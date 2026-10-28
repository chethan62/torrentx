// ─── Jackett types ─────────────────────────────────────────────────────────
use crate::lock_or_recover;
use crate::themes::rgb;
use eframe::egui::Color32;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct JackettResponse {
    #[serde(default)]
    pub(crate) results: Vec<TorrentResult>,
    /// The aggregate `results?Query=` route answers 200 even when an indexer
    /// failed; the failure is only visible here. Ignoring it made a Cloudflare
    /// block look exactly like "no results" (this is how 1337x stayed silent).
    #[serde(default)]
    pub(crate) indexers: Vec<JackettIndexerStatus>,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct JackettIndexerStatus {
    // Jackett sends `"Error": null` for a healthy indexer; `#[serde(default)]`
    // only covers a MISSING field, so a plain String failed the whole parse and
    // every successful indexer came back as an error.
    #[serde(rename = "ID", default)]
    pub(crate) id: Option<String>,
    #[serde(default)]
    pub(crate) error: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct TorrentResult {
    #[serde(default)]
    pub(crate) title: String,
    pub(crate) tracker: Option<String>,
    pub(crate) category_desc: Option<String>,
    pub(crate) size: Option<u64>,
    pub(crate) seeders: Option<u32>,
    pub(crate) peers: Option<u32>,
    pub(crate) publish_date: Option<String>,
    pub(crate) magnet_uri: Option<String>,
    pub(crate) link: Option<String>,
    pub(crate) details: Option<String>,
    /// Other trackers that carry the same infohash. Derived by
    /// [`group_by_infohash`], never parsed from a feed, so it is skipped in both
    /// directions of serde.
    #[serde(skip)]
    pub(crate) also_on: Vec<String>,
}

// ─── App state types ───────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
pub(crate) enum SortCol {
    Name,
    Tracker,
    Size,
    Seeds,
    Leech,
    Ratio,
    Date,
}

#[derive(Clone, PartialEq)]
pub(crate) enum SortDir {
    Asc,
    Desc,
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) enum Tab {
    Search,
    Favorites,
    Rss,
    About,
}

impl Tab {
    /// Stable config-file key for this tab (persisted as `last_tab`).
    pub(crate) fn key(&self) -> &'static str {
        match self {
            Tab::Search => "Search",
            Tab::Favorites => "Favorites",
            Tab::Rss => "Rss",
            Tab::About => "About",
        }
    }
    /// Inverse of `key`; unknown strings fall back to Search (via the caller).
    pub(crate) fn from_key(s: &str) -> Option<Tab> {
        Some(match s {
            "Search" => Tab::Search,
            "Favorites" => Tab::Favorites,
            "Rss" => Tab::Rss,
            "About" => Tab::About,
            _ => return None,
        })
    }
}

/// Results-table columns (order is user-configurable via `Config::col_order`).
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum TableCol {
    Name,
    Tracker,
    Size,
    Seeds,
    Leech,
    Ratio,
    Health,
    Date,
}

impl TableCol {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            TableCol::Name => "Name",
            TableCol::Tracker => "Tracker",
            TableCol::Size => "Size",
            TableCol::Seeds => "Seeds",
            TableCol::Leech => "Leech",
            TableCol::Ratio => "Ratio",
            TableCol::Health => "Health",
            TableCol::Date => "Date",
        }
    }
    pub(crate) fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            "Name" => TableCol::Name,
            "Tracker" => TableCol::Tracker,
            "Size" => TableCol::Size,
            "Seeds" => TableCol::Seeds,
            "Leech" => TableCol::Leech,
            "Ratio" => TableCol::Ratio,
            "Health" => TableCol::Health,
            "Date" => TableCol::Date,
            _ => return None,
        })
    }
    /// Stable key for persisting per-column settings: unlike the index it
    /// survives a column toggle or a reorder.
    pub(crate) fn key(&self) -> &'static str {
        match self {
            TableCol::Name => "name",
            TableCol::Tracker => "tracker",
            TableCol::Size => "size",
            TableCol::Seeds => "seeds",
            TableCol::Leech => "leech",
            TableCol::Ratio => "ratio",
            TableCol::Health => "health",
            TableCol::Date => "date",
        }
    }

    pub(crate) fn width(&self) -> f32 {
        match self {
            TableCol::Name => 295.0,
            TableCol::Tracker => 104.0,
            TableCol::Size => 72.0,
            TableCol::Seeds => 62.0,
            TableCol::Leech => 62.0,
            TableCol::Ratio => 56.0,
            TableCol::Health => 72.0,
            TableCol::Date => 84.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SearchState {
    Idle,
    Searching,
    Done,
    Error(String),
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) enum Hlth {
    All,
    Hot,
    Good,
    Slow,
    /// 1..=10 seeds — the band the row badge calls DYING. The chip was named
    /// DEAD while selecting this band too, so filtering "DEAD" listed rows
    /// labelled DYING.
    Dying,
    Dead,
}

impl Hlth {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Hlth::All => "All",
            Hlth::Hot => "HOT",
            Hlth::Good => "GOOD",
            Hlth::Slow => "SLOW",
            Hlth::Dying => "DYING",
            Hlth::Dead => "DEAD",
        }
    }
    /// The bands must partition exactly as `hlth_lbl` (the row badge) does — a chip
    /// must select precisely the rows it is named after. Enforced by
    /// `health_chips_match_the_row_badge_bands`.
    pub(crate) fn ok(&self, s: u32) -> bool {
        match self {
            Hlth::All => true,
            Hlth::Hot => s > 500,
            Hlth::Good => (101..=500).contains(&s),
            Hlth::Slow => (11..=100).contains(&s),
            Hlth::Dying => (1..=10).contains(&s),
            Hlth::Dead => s == 0,
        }
    }

    /// Same bands, but for a source that publishes no seeder count at all
    /// (btdig and other DHT indexes): unknown belongs under `All` only, never
    /// under DEAD — "we do not know" is not "the swarm is empty".
    pub(crate) fn ok_opt(&self, s: Option<u32>) -> bool {
        match s {
            Some(v) => self.ok(v),
            None => matches!(self, Hlth::All),
        }
    }
}
// ─── Pure helpers ──────────────────────────────────────────────────────────

pub(crate) fn fmt_size(b: u64) -> String {
    if b >= 1_073_741_824 {
        format!("{:.2} GB", b as f64 / 1_073_741_824.0)
    } else if b >= 1_048_576 {
        format!("{:.0} MB", b as f64 / 1_048_576.0)
    } else if b >= 1_024 {
        format!("{:.0} KB", b as f64 / 1_024.0)
    } else {
        format!("{b} B")
    }
}

pub(crate) fn time_ago(s: &str) -> String {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s)
        // Jackett's torznab pubDate is RFC 2822 ("Tue, 07 May 2024 10:20:30
        // +0000"); without this the RSS list printed the chopped "Tue, 07 Ma".
        .or_else(|_| chrono::DateTime::parse_from_rfc2822(s))
        .or_else(|_| chrono::DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%z"))
    {
        let secs = chrono::Utc::now()
            .signed_duration_since(dt.with_timezone(&chrono::Utc))
            .num_seconds()
            .max(0);
        return if secs < 3600 {
            format!("{}m ago", secs / 60)
        } else if secs < 86400 {
            format!("{}h ago", secs / 3600)
        } else if secs < 604800 {
            format!("{}d ago", secs / 86400)
        } else {
            dt.format("%Y-%m-%d").to_string()
        };
    }
    s.get(..10).unwrap_or("?").to_string()
}

pub(crate) fn pub_year(s: &str) -> u32 {
    chrono::DateTime::parse_from_rfc3339(s)
        .or_else(|_| chrono::DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%z"))
        .ok()
        .and_then(|dt| dt.format("%Y").to_string().parse::<u32>().ok())
        .unwrap_or(0)
}

/// Truncate a magnet link for display, cutting on a UTF-8 char boundary.
/// Magnets can carry non-ASCII in the `dn=` display name; byte-slicing
/// `&s[..57]` panics on a mid-char cut. Returns the string un-changed when
/// it's short enough. Pure + testable.
pub(crate) fn truncate_magnet(mag: &str, max_bytes: usize) -> String {
    if mag.len() <= max_bytes {
        return mag.to_string();
    }
    // Find the last char boundary at or before max_bytes.
    let cut = mag
        .char_indices()
        .map(|(i, _)| i)
        .take_while(|&i| i <= max_bytes)
        .last()
        .unwrap_or(0);
    format!("{}…", &mag[..cut])
}

pub(crate) fn seed_col(s: u32) -> Color32 {
    if s > 500 {
        rgb(34, 197, 94)
    } else if s > 100 {
        rgb(74, 222, 128)
    } else if s > 10 {
        rgb(245, 158, 11)
    } else if s > 0 {
        rgb(249, 115, 22)
    } else {
        rgb(239, 68, 68)
    }
}

pub(crate) fn hlth_lbl(s: u32) -> &'static str {
    if s > 500 {
        "HOT"
    } else if s > 100 {
        "GOOD"
    } else if s > 10 {
        "SLOW"
    } else if s > 0 {
        "DYING"
    } else {
        "DEAD"
    }
}

pub(crate) fn cat_col(cat: &str) -> Color32 {
    match cat.split('/').next().unwrap_or("").trim() {
        "Movies" => rgb(245, 158, 11),
        "TV" => rgb(59, 130, 246),
        "Music" => rgb(16, 185, 129),
        "Games" => rgb(139, 92, 246),
        "Software" => rgb(6, 182, 212),
        "Anime" => rgb(236, 72, 153),
        "Books" => rgb(249, 115, 22),
        _ => rgb(100, 116, 139),
    }
}

pub(crate) fn urlenc(s: &str) -> String {
    // Percent-encode UTF-8 bytes (not code points) so non-ASCII queries
    // reach Jackett correctly. Space → '+' (application/x-www-form-urlencoded).
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub(crate) fn normalize(t: &str) -> String {
    let stop = [
        "2160p", "1080p", "720p", "480p", "4k", "uhd", "bluray", "bdrip", "webrip", "webdl",
        "x264", "x265", "hevc", "10bit", "hdr", "dolby", "yify", "yts", "rarbg", "mkv", "mp4",
        "avi", "remux",
    ];
    let mut s = t.to_lowercase();
    for w in &stop {
        s = s.replace(w, " ");
    }
    s.split_whitespace().take(4).collect::<Vec<_>>().join(" ")
}

pub(crate) fn now_str() -> String {
    chrono::Utc::now().format("%Y-%m-%d %H:%M").to_string()
}

pub(crate) fn set_err(st: &Arc<Mutex<SearchState>>, msg: String) {
    *lock_or_recover(st) = SearchState::Error(msg);
}

// ─── Search thread ─────────────────────────────────────────────────────────

/// Shared HTTP client, built once. Avoids re-handshaking per request.
/// Falls back to a bare default client if a custom-builder client fails
/// (can't happen in practice, but never panic on startup over this).
pub(crate) fn shared_client() -> &'static Client {
    static CLIENT: std::sync::OnceLock<Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .unwrap_or_else(|_| Client::new())
    })
}

/// GitHub repo to check for updates (override with `TORRENTX_UPDATE_REPO`).
const UPDATE_REPO: &str = "chethan62/torrentx";

/// Check GitHub releases for a newer version. Returns the latest release tag
/// (e.g. "v17.0.0") or None on failure / no newer version.
pub(crate) fn check_update(current: &str) -> Option<String> {
    let repo = std::env::var("TORRENTX_UPDATE_REPO").unwrap_or_else(|_| UPDATE_REPO.to_string());
    let ep = format!("https://api.github.com/repos/{repo}/releases/latest");
    let resp = shared_client()
        .get(ep)
        .header("User-Agent", "TorrentX")
        .timeout(Duration::from_secs(10))
        .send()
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body = resp.text().ok()?;
    let latest = parse_latest_tag(&body)?;
    let latest_trim = latest.trim_start_matches('v');
    let cur_trim = current.trim_start_matches('v');
    // Compare dotted versions
    let parse =
        |s: &str| -> Vec<u64> { s.split('.').filter_map(|p| p.parse::<u64>().ok()).collect() };
    let (l, c) = (parse(latest_trim), parse(cur_trim));
    let newer = l
        .iter()
        .zip(c.iter())
        .find(|(a, b)| a != b)
        .map(|(a, b)| a > b)
        .unwrap_or(l.len() > c.len());
    if newer {
        Some(latest.to_string())
    } else {
        None
    }
}

/// Pull `tag_name` out of a GitHub "latest release" JSON body via serde_json.
/// Split out so the network call (`check_update`) and the parsing are separable
/// and unit-testable without a live server.
fn parse_latest_tag(body: &str) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct Latest {
        tag_name: String,
    }
    let l: Latest = serde_json::from_str(body).ok()?;
    Some(l.tag_name)
}

/// Numbers that mean "encoding", not "content" — they differ between two copies of
/// the SAME release, so they must not separate them.
const NOT_CONTENT: [&str; 10] = [
    "264", "265", "720", "1080", "2160", "480", "576", "10", "12", "8",
];

/// Content markers: episode/season/part/year numbers found anywhere in the title.
fn content_markers(t: &str) -> Vec<String> {
    let mut m: Vec<String> = t
        .to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|tok| {
            if tok.chars().all(|c| c.is_ascii_digit()) && (2..=4).contains(&tok.len()) {
                return !NOT_CONTENT.contains(tok);
            }
            // s02e07 / ep1178 / 1x02 style episode markers
            let b = tok.as_bytes();
            (b.len() >= 4 && b[0] == b's' && tok.contains('e'))
                || tok
                    .strip_prefix("ep")
                    .is_some_and(|r| r.parse::<u32>().is_ok())
                || (tok.contains('x') && tok.split('x').all(|p| p.parse::<u32>().is_ok()))
        })
        .map(String::from)
        .collect();
    m.sort();
    m.dedup();
    m
}

/// Dedupe key: `normalize` (which strips quality noise) PLUS the content markers.
///
/// `normalize` alone keys on the first four words, which hid real results: over live
/// Jackett data it merged `Episode of East Blue (2017)`, `Episode of Sabo (2015)` and
/// `Episode of Skypiea (2018)` into one row, merged episodes 1177 and 1178, and merged
/// Ubuntu 2022 with 2023 — 12 and 10 wrong merges in two 400-item samples. With the
/// markers appended those samples have none, while same-release-different-encode rows
/// still collapse.
pub(crate) fn dedupe_key(t: &str) -> String {
    let marks = content_markers(t);
    if marks.is_empty() {
        normalize(t)
    } else {
        format!("{}|{}", normalize(t), marks.join(","))
    }
}

/// Keep one row per title — the best-seeded one.
///
/// Deduping with a seen-set while filtering kept whichever tracker Jackett listed
/// first, so the surviving row (the one the user downloads) could be a 1-seeder
/// copy while a 900-seeder copy was discarded. Ties keep the earlier row, so feed
/// order still breaks equal-seed cases.
pub(crate) fn dedupe_best_seeded(rows: Vec<TorrentResult>) -> Vec<TorrentResult> {
    let mut best: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (i, r) in rows.iter().enumerate() {
        let key = dedupe_key(&r.title);
        match best.get(&key) {
            Some(&j) if rows[j].seeders.unwrap_or(0) >= r.seeders.unwrap_or(0) => {}
            _ => {
                best.insert(key, i);
            }
        }
    }
    let mut keep: Vec<usize> = best.into_values().collect();
    keep.sort_unstable();
    keep.into_iter().map(|i| rows[i].clone()).collect()
}

/// The `btih` value out of a magnet link, lowercased — "this is the same
/// torrent, whatever tracker listed it".
///
/// Base32 is left as-is: indexers that emit base32 all emit the same base32, so
/// it still compares equal, and decoding 32 chars to hex buys nothing here.
pub(crate) fn info_hash(magnet: &str) -> Option<String> {
    let rest = magnet.split("xt=urn:btih:").nth(1)?;
    let v = rest.split('&').next().unwrap_or_default().trim();
    (!v.is_empty()).then(|| v.to_ascii_lowercase())
}

/// One row per torrent, however many trackers carry it: the best-seeded copy
/// survives and keeps the other trackers in `also_on`.
///
/// Name-based dedupe cannot merge "Show.S01.1080p-GRP" listed by two trackers
/// when one of them appends its own tag, and it *must not* merge two different
/// torrents that happen to share a name. The infohash is exact on both counts.
/// Rows with no magnet (or no `btih`) pass through untouched.
pub(crate) fn group_by_infohash(rows: Vec<TorrentResult>) -> Vec<TorrentResult> {
    let mut best: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut trackers: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    let mut dropped: std::collections::HashSet<usize> = std::collections::HashSet::new();
    for (i, r) in rows.iter().enumerate() {
        let Some(h) = r.magnet_uri.as_deref().and_then(info_hash) else {
            continue;
        };
        let name = r.tracker.clone().unwrap_or_else(|| "—".to_string());
        let seen = trackers.entry(h.clone()).or_default();
        if !seen.contains(&name) {
            seen.push(name);
        }
        match best.get(&h) {
            Some(&j) if rows[j].seeders.unwrap_or(0) >= r.seeders.unwrap_or(0) => {
                dropped.insert(i);
            }
            Some(&j) => {
                // A better-seeded copy replaces the one seen earlier: drop `j`,
                // keep `i`. (Dropping `i` here kept the first-seen row and threw
                // the best-seeded one away — the test below caught exactly that.)
                dropped.insert(j);
                best.insert(h, i);
            }
            None => {
                best.insert(h, i);
            }
        }
    }
    let mut out = Vec::with_capacity(rows.len());
    for (i, mut r) in rows.into_iter().enumerate() {
        if dropped.contains(&i) {
            continue;
        }
        if let Some(h) = r.magnet_uri.as_deref().and_then(info_hash) {
            let own = r.tracker.clone().unwrap_or_default();
            r.also_on = trackers
                .get(&h)
                .map(|v| v.iter().filter(|t| **t != own).cloned().collect())
                .unwrap_or_default();
        }
        out.push(r);
    }
    out
}

/// Sortable key for the date formats feeds actually emit: RFC 3339
/// (`2024-05-01T10:20:30+00:00`), a bare `2024-05-01`, and RFC 2822/RFC 1123
/// (`Tue, 07 May 2024 10:20:30 +0000`, which is what Jackett's torznab `pubDate`
/// usually is). Sorting the raw strings was wrong even within one format, since
/// "07 May" compares before "12 Apr" lexically although April comes first.
pub(crate) fn pub_date_key(s: &str) -> (u32, u32, u32, u32, u32, u32) {
    const MONTHS: [&str; 12] = [
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ];
    let s = s.trim();
    // ISO / RFC 3339: year is already the leading field.
    let b = s.as_bytes();
    if s.len() >= 10 && b[4] == b'-' && b[7] == b'-' {
        let n = |a: usize, z: usize| s.get(a..z).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
        return (n(0, 4), n(5, 7), n(8, 10), n(11, 13), n(14, 16), n(17, 19));
    }
    // RFC 2822 / RFC 1123: fields in any order, weekday optional.
    let (mut day, mut mon, mut year) = (0u32, 0u32, 0u32);
    let (mut hh, mut mm, mut ss) = (0u32, 0u32, 0u32);
    for tok in s.split([' ', ',']).filter(|t| !t.is_empty()) {
        let low = tok.to_ascii_lowercase();
        if low.len() >= 3
            && let Some(i) = MONTHS.iter().position(|m| low.starts_with(m))
        {
            mon = i as u32 + 1;
            continue;
        }
        if let Some((h, rest)) = tok.split_once(':') {
            let (m, sec) = rest.split_once(':').unwrap_or((rest, "0"));
            hh = h.parse().unwrap_or(0);
            mm = m.parse().unwrap_or(0);
            ss = sec.parse().unwrap_or(0);
            continue;
        }
        if let Ok(v) = tok.parse::<u32>() {
            if v > 999 {
                year = v; // 4-digit year
            } else if day == 0 {
                day = v; // leading day-of-month; the offset ("+0000") is skipped
            }
        }
    }
    (year, mon, day, hh, mm, ss)
}

/// Validate a Jackett base URL: http/https scheme with a real host.
/// Returns an error message, or None if the URL is acceptable.
pub(crate) fn validate_jackett_url(s: &str) -> Option<&'static str> {
    let t = s.trim();
    if t.is_empty() {
        return Some("URL is empty");
    }
    let lower = t.to_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Some("URL must start with http:// or https://");
    }
    // The scheme alone is not a Jackett address. A host is required, and it must
    // be checked on the raw string: `url::Url::parse("http:///api")` normalizes
    // to a URL that *has* a host, so parsing lets exactly the cases we are
    // guarding against through.
    let after_scheme = t.split_once("://").map(|(_, rest)| rest).unwrap_or("");
    let authority = after_scheme.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority.split(':').next().unwrap_or("");
    if host.is_empty() {
        return Some("URL must include a host, e.g. http://localhost:9117");
    }
    None
}

/// Validate a magnet link: it must carry an `xt=urn:btih:` (or BitTorrent v2
/// `xt=urn:btmh:`) exact-topic parameter with a plausible hash.
///
/// The parameter may appear anywhere in the query. Real feeds send
/// `magnet:?dn=…&xt=urn:btih:…` as well, which an exact-prefix check rejected —
/// hiding the magnet/copy buttons for a perfectly valid link.
pub(crate) fn is_magnet(s: &str) -> bool {
    let s = s.trim();
    let Some(query) = s.strip_prefix("magnet:?") else {
        return false;
    };
    for param in query.split('&') {
        let Some(v) = param.strip_prefix("xt=") else {
            continue;
        };
        if let Some(hash) = v.strip_prefix("urn:btih:") {
            let hash = hash.trim_end_matches(';');
            return match hash.len() {
                40 => hash.chars().all(|c| c.is_ascii_hexdigit()),
                32 => hash.chars().all(|c| c.is_ascii_alphanumeric()), // base32
                _ => false,
            };
        }
        // BitTorrent v2 hash, e.g. urn:btmh:1220<64 hex chars>.
        if let Some(mh) = v.strip_prefix("urn:btmh:") {
            return mh.len() >= 40 && mh.chars().all(|c| c.is_ascii_alphanumeric());
        }
    }
    false
}

/// Map UI category labels to Jackett/Torznab numeric category IDs.
/// The API expects numbers (2000=Movies, 5000=TV, …), not English labels.
pub(crate) fn category_id(label: &str) -> Option<&'static str> {
    Some(match label {
        "Movies" => "2000",
        "TV" => "5000",
        "Music" => "3000",
        "PC Games" => "4050",
        "Software" => "4000",
        "Anime" => "5070",
        "Books" => "7000",
        "XXX" => "6000",
        _ => return None,
    })
}

/// Fetch the list of *configured* Jackett indexers (id slugs).
/// Uses the Torznab `t=indexers` endpoint. Returns `None` if Jackett is
/// unreachable, `Some(list)` (possibly empty) on success.
/// Jackett's whole indexer list, plus which of them are configured. The
/// unconfigured ones cannot be added through the API (every write on
/// `/api/v2.0/indexers*` is behind Jackett's cookie session), so the app shows
/// them as a catalogue to search and name, and the adding happens in Jackett.
#[derive(Clone, Debug, Default)]
pub(crate) struct IndexerCatalogue {
    pub(crate) configured: Vec<String>,
    pub(crate) all: Vec<String>,
}

impl IndexerCatalogue {
    /// Indexers Jackett knows but has not configured, in the order Jackett sent.
    pub(crate) fn unconfigured(&self) -> impl Iterator<Item = &String> {
        self.all.iter().filter(|id| !self.configured.contains(id))
    }
}

/// How long a cached `t=indexers` body is trusted. Jackett's indexer set changes
/// only when the user edits it, and the indexer window has a "Refresh list"
/// button that forces a fetch, so a day is generous and saves 2.3 MB per launch.
const INDEXER_CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// Where the catalogue body is cached. Named by a hash of url+key so another
/// Jackett (or another API key) cannot serve a stale list — and so the key never
/// appears in a path.
fn indexer_cache_path(url: &str, key: &str) -> std::path::PathBuf {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325; // FNV-1a, 64 bit
    for b in url.bytes().chain(std::iter::once(b'|')).chain(key.bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    dirs_next::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("torrentx")
        .join(format!("indexers-{h:016x}.xml"))
}

fn cache_fresh(age: Duration) -> bool {
    age <= INDEXER_CACHE_TTL
}

/// The cached catalogue, if there is a fresh one. `None` means "go to the
/// network" (missing, stale or unparseable) — never an error the UI has to show.
pub(crate) fn cached_indexers(url: &str, key: &str) -> Option<IndexerCatalogue> {
    let path = indexer_cache_path(url, key);
    let meta = std::fs::metadata(&path).ok()?;
    let age = meta.modified().ok()?.elapsed().ok()?;
    if !cache_fresh(age) {
        return None;
    }
    let body = std::fs::read_to_string(&path).ok()?;
    parse_indexers_xml(&body)
}

fn store_indexers(url: &str, key: &str, body: &str) {
    let path = indexer_cache_path(url, key);
    if let Some(dir) = path.parent()
        && std::fs::create_dir_all(dir).is_err()
    {
        return;
    }
    let tmp = crate::unique_tmp(&path);
    if std::fs::write(&tmp, body).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

pub(crate) fn fetch_indexers(url: &str, key: &str) -> Option<IndexerCatalogue> {
    let ep = format!(
        "{}/api/v2.0/indexers/all/results/torznab/api?apikey={}&t=indexers",
        url.trim_end_matches('/'),
        key
    );
    let resp = shared_client()
        .get(&ep)
        .timeout(Duration::from_secs(15))
        .send()
        .ok()?;
    let body = resp.text().ok()?;
    let cat = parse_indexers_xml(&body)?;
    store_indexers(url, key, &body);
    Some(cat)
}

/// Curated groups for the catalogue. Only ids Jackett actually ships are listed
/// here; `catalogue_matches` intersects them with what the server returns, so a
/// removed indexer simply disappears instead of advertising a dead name.
///
/// There is no Telugu/Tamil/Malayalam indexer in Jackett's catalogue — the only
/// desi definitions are `desigaane` and `desitorrents-api`; Indian-language
/// releases otherwise arrive through the general trackers (1337x, knaben, TPB).
pub(crate) const CATALOGUE_GROUPS: &[(&str, &[&str])] = &[
    (
        "Popular",
        &[
            "1337x",
            "thepiratebay",
            "yts",
            "eztv",
            "knaben",
            "limetorrents",
            "torrentgalaxyclone",
            "kickasstorrents-to",
            "kickasstorrents-ws",
            "mejortorrent",
        ],
    ),
    (
        "Anime",
        &[
            "nyaasi",
            "animetosho-xyz",
            "animebytes",
            "bakabt",
            "acgrip",
            "tokyotosho",
            "animez",
            "sukebeinyaasi",
        ],
    ),
    ("Indian / Desi", &["desigaane", "desitorrents-api"]),
    ("Games", &["gazellegamesapi", "bitgamer"]),
    (
        "Books / Audio",
        &["epublibre", "audiobookbay", "myanonamouse", "bibliotik"],
    ),
    ("Music", &["redacted", "orpheus", "dicmusic"]),
];

/// Unconfigured ids in one group, in the group's own order.
pub(crate) fn group_entries<'a>(cat: &'a IndexerCatalogue, group: &[&'a str]) -> Vec<&'a String> {
    group
        .iter()
        .filter_map(|id| cat.all.iter().find(|a| a.as_str() == *id))
        .filter(|id| !cat.configured.contains(id))
        .collect()
}

/// Catalogue entries matching `query` that are not configured yet, capped.
/// Pure and unit-testable: the popup calls this every frame.
pub(crate) fn catalogue_matches<'a>(
    cat: &'a IndexerCatalogue,
    query: &str,
    limit: usize,
) -> Vec<&'a String> {
    let q = query.trim().to_lowercase();
    cat.unconfigured()
        .filter(|id| q.is_empty() || id.to_lowercase().contains(&q))
        .take(limit)
        .collect()
}

/// Pull every indexer id out of a Jackett `t=indexers` Torznab XML body, noting
/// which are configured. Pure and unit-testable without a live server.
fn parse_indexers_xml(body: &str) -> Option<IndexerCatalogue> {
    use quick_xml::Reader;
    use quick_xml::events::Event;
    let mut reader = Reader::from_str(body);
    reader.config_mut().trim_text(true);
    let mut cat = IndexerCatalogue::default();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                // 0.42: QName/attribute keys borrow as `&str` (was `&[u8]` in ≤0.41).
                let tag = e.name().as_ref().to_lowercase();
                if tag != "indexer" {
                    continue;
                }
                let mut id = None;
                let mut configured = false;
                for attr in e.attributes().flatten() {
                    let k = attr.key.as_ref().to_lowercase();
                    if let Ok(v) = attr.normalized_value(quick_xml::XmlVersion::Implicit1_0) {
                        match k.as_str() {
                            "id" => id = Some(v.to_string()),
                            "configured" => configured = v == "true",
                            _ => {}
                        }
                    }
                }
                let Some(id) = id.filter(|i| !i.is_empty()) else {
                    continue;
                };
                if configured {
                    cat.configured.push(id.clone());
                }
                cat.all.push(id);
            }
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
        buf.clear();
    }
    cat.configured.sort();
    cat.configured.dedup();
    cat.all.dedup();
    Some(cat)
}

/// Which indexers a search fans out to. Picking one indexer searches only it;
/// "All" (or a list not fetched yet) fans out over every configured indexer, so
/// the table fills as each tracker answers instead of waiting for the slowest
/// one. Pure and tested.
pub(crate) fn search_targets(
    indexer: &str,
    configured: &[String],
    disabled: &[String],
) -> Vec<String> {
    if !indexer.is_empty() && indexer != "All" {
        return vec![indexer.to_string()];
    }
    if configured.is_empty() {
        return vec!["all".to_string()];
    }
    let mut targets: Vec<String> = configured
        .iter()
        .filter(|c| !disabled.iter().any(|d| d == *c))
        .cloned()
        .collect();
    // Never fan out over nothing: a config that disables everything (or a
    // disabled list naming indexers that no longer exist) would otherwise leave
    // a search with no targets at all. Keeping the first configured indexer is
    // the smallest honest fallback.
    if targets.is_empty() {
        targets.push(configured[0].clone());
    }
    targets
}

/// Jackett results endpoint for one indexer.
fn search_url(url: &str, key: &str, query: &str, cat: &str, idx: &str) -> String {
    let mut ep = format!(
        "{}/api/v2.0/indexers/{}/results?apikey={}&Query={}",
        url.trim_end_matches('/'),
        urlenc(idx),
        urlenc(key),
        urlenc(query)
    );
    if cat != "All"
        && let Some(id) = category_id(cat)
    {
        ep.push_str(&format!("&Category[]={}", id));
    }
    ep
}

/// Message for a transport failure. The URL is stripped: reqwest appends
/// " for url (…)" and the query string carries the Jackett API key.
fn request_error(e: reqwest::Error, timeout: u64, url: &str) -> String {
    if e.is_connect() {
        format!(
            "Cannot reach Jackett at {url}\nSettings → Start Jackett, or: sudo systemctl start jackett"
        )
    } else if e.is_timeout() {
        format!("Timed out after {timeout}s — increase timeout in Settings")
    } else {
        format!("Network error: {}", e.without_url())
    }
}

/// Jackett's per-indexer error text is a .NET exception, and its inner message can
/// carry the request URL — which includes `apikey=`. Keep one short line, no key.
pub(crate) fn scrub_indexer_error(raw: &str) -> String {
    let first = raw.lines().next().unwrap_or("").trim();
    let cleaned = match first.find(" for url") {
        Some(i) => &first[..i],
        None => first,
    };
    let mut s = cleaned.replace("\r", " ");
    if let Some(i) = s.find("apikey=") {
        let mut end = s[i..].find(['&', ')', ' ']).unwrap_or(s.len() - i);
        end += i;
        s.replace_range(i..end, "apikey=***");
    }
    s.chars().take(140).collect()
}

fn status_error(st: reqwest::StatusCode) -> String {
    match st.as_u16() {
        401 => "Invalid API key — open Settings to update it.".into(),
        403 => "Forbidden — check Jackett permissions.".into(),
        404 => "Jackett endpoint not found — verify URL in Settings.".into(),
        500 => "Jackett internal error — check Jackett logs.".into(),
        n => format!("HTTP {n} from Jackett"),
    }
}

/// Append rows to the shared, published-once results vec.
fn append_results(
    results: &Arc<Mutex<Arc<Vec<TorrentResult>>>>,
    count: &Arc<Mutex<usize>>,
    mut add: Vec<TorrentResult>,
    dedupe: bool,
) {
    let total = {
        let mut r = lock_or_recover(results);
        let mut v = (**r).clone();
        v.append(&mut add);
        if dedupe {
            v = group_by_infohash(v);
            v = dedupe_best_seeded(v);
        }
        let n = v.len();
        *r = Arc::new(v);
        n
    };
    *lock_or_recover(count) = total;
}

/// One search's connection parameters, cloned per fan-out worker.
/// Ceiling for one indexer in an "All" search.
///
/// Measured against a live Jackett: 8 of 9 configured indexers answer in ≤1.2 s
/// and one (anisource) takes 12–18 s while returning nothing for TV/movie queries.
/// The fan-out joins every worker before reporting Done, so without a cap the
/// slowest tracker IS the search duration — and the user's own timeout (up to
/// 120 s) was the worst case. A single-indexer search still honours the setting.
const INDEXER_DEADLINE_SECS: u64 = 20;

/// How long the fan-out waits before reporting Done on the rows it already has.
/// The quick indexers answer in ~1-2 s; this is the grace for a merely slow one.
const EARLY_DONE_SECS: u64 = 3;

/// The per-indexer deadline used by the "All" fan-out.
pub(crate) fn indexer_timeout(base: u64) -> u64 {
    base.min(INDEXER_DEADLINE_SECS)
}

#[derive(Clone)]
struct SearchCtx {
    url: String,
    key: String,
    query: String,
    cat: String,
    timeout: u64,
    /// Keep the stored set deduped as rows arrive. With the early-Done watchdog a
    /// straggler can append AFTER the settle pass, and an undeduped late row would
    /// make the badge disagree with the table.
    dedupe: bool,
}

/// Status-line suffix naming the indexers that failed this search, capped so a
/// total outage cannot fill the status bar with 600 names.
pub(crate) fn failed_note(failed: &[String]) -> String {
    if failed.is_empty() {
        return String::new();
    }
    let mut sorted: Vec<&str> = failed.iter().map(String::as_str).collect();
    sorted.sort_unstable();
    let n = sorted.len();
    let shown: Vec<&str> = sorted.iter().take(3).copied().collect();
    let more = if n > shown.len() {
        format!(" +{}", n - shown.len())
    } else {
        String::new()
    };
    format!("  ·  {n} failed: {}{more}", shown.join(", "))
}

/// Phone-width status bar: the count only. The named list is what pushes the
/// line past 360 px, and a label clipped mid-word reads as a broken app.
pub(crate) fn failed_note_short(failed: &[String]) -> String {
    if failed.is_empty() {
        String::new()
    } else {
        format!("  ·  {} failed", failed.len())
    }
}

/// Fetch one indexer and append its rows. Err is a message for the UI.
fn fetch_into(
    ctx: &SearchCtx,
    idx: &str,
    results: &Arc<Mutex<Arc<Vec<TorrentResult>>>>,
    count: &Arc<Mutex<usize>>,
) -> Result<usize, String> {
    // A user-defined JSON indexer: one request + a field map, no Jackett. It is
    // checked first so a definition can replace a built-in by id.
    if let Some(def) = crate::indexers::find(idx) {
        let rows = crate::indexers::fetch(&def, &ctx.query, &ctx.cat, ctx.timeout)?;
        let n = rows.len();
        append_results(results, count, rows, ctx.dedupe);
        crate::wake_ui();
        return Ok(n);
    }
    // A native source (apibay, knaben) has no Jackett behind it: same fan-out,
    // same append/dedupe/wake path, just a different endpoint.
    if crate::sources::is_native(idx) {
        let rows = crate::sources::fetch(idx, &ctx.query, &ctx.cat, ctx.timeout)?;
        let n = rows.len();
        append_results(results, count, rows, ctx.dedupe);
        crate::wake_ui();
        return Ok(n);
    }
    let ep = search_url(&ctx.url, &ctx.key, &ctx.query, &ctx.cat, idx);
    let resp = shared_client()
        .get(&ep)
        .timeout(Duration::from_secs(ctx.timeout))
        .send()
        .map_err(|e| request_error(e, ctx.timeout, &ctx.url))?;
    let st = resp.status();
    if !st.is_success() {
        return Err(status_error(st));
    }
    let data = resp.json::<JackettResponse>().map_err(|e| {
        // reqwest's Display appends " for url (…?apikey=…)" — the Jackett API key
        // must never reach the screen, a screenshot, or a pasted bug report.
        // Without the URL the rest ("expected value at line 1 column 1") still
        // says what happened, and the hint names the usual cause.
        format!(
            "Unexpected response from Jackett ({}) — check the URL in Settings",
            e.without_url()
        )
    })?;
    let n = data.results.len();
    if n == 0
        && let Some(bad) = data
            .indexers
            .iter()
            .find(|i| i.error.as_deref().is_some_and(|e| !e.trim().is_empty()))
    {
        let msg = scrub_indexer_error(bad.error.as_deref().unwrap_or(""));
        let id = bad.id.as_deref().unwrap_or("");
        return Err(if id.is_empty() || id == idx {
            msg
        } else {
            format!("{id}: {msg}")
        });
    }
    append_results(results, count, data.results, ctx.dedupe);
    // Wake the parked UI loop so the rows paint now, not on the next input.
    crate::wake_ui();
    Ok(n)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn start_search(
    url: String,
    key: String,
    query: String,
    cat: String,
    targets: Vec<String>,
    timeout: u64,
    results: Arc<Mutex<Arc<Vec<TorrentResult>>>>,
    state: Arc<Mutex<SearchState>>,
    count: Arc<Mutex<usize>>,
    epoch: Arc<AtomicU64>,
    answered: Arc<AtomicUsize>,
    failed: Arc<Mutex<Vec<String>>>,
    dedupe: bool,
    generation: u64,
) {
    thread::spawn(move || {
        // Epoch guard: a newer search invalidates this one — never touch state,
        // so a slow old response can't overwrite fresh results (search race).
        if epoch.load(Ordering::Relaxed) != generation {
            return;
        }
        let live = || epoch.load(Ordering::Relaxed) == generation;
        *lock_or_recover(&state) = SearchState::Searching;

        let ctx = SearchCtx {
            url: url.clone(),
            key: key.clone(),
            query: query.clone(),
            cat: cat.clone(),
            timeout,
            dedupe,
        };

        // One indexer: the plain request, same as before.
        if targets.len() <= 1 {
            let idx = targets.first().map(String::as_str).unwrap_or("all");
            match fetch_into(&ctx, idx, &results, &count) {
                Ok(_) if live() => {
                    *lock_or_recover(&state) = SearchState::Done;
                }
                Ok(_) => {}
                Err(e) if live() => set_err(&state, e),
                Err(_) => {}
            }
            answered.fetch_add(1, Ordering::Relaxed);
            crate::wake_ui();
            return;
        }

        // Fan out: every indexer on its own thread. Each appends its rows and
        // wakes the UI, so page 1 appears while slower trackers are still going.
        // Breadth search: one slow tracker must not become the search duration.
        let ctx = SearchCtx {
            timeout: indexer_timeout(ctx.timeout),
            ..ctx
        };
        // Early Done. Every worker wakes the UI as it appends, so the table is
        // usable as soon as the quick indexers answer; waiting for the slowest one
        // only kept the Search button disabled and the timer running (measured:
        // 8/9 in 2.5 s, the last at 12.65 s).
        {
            let state = Arc::clone(&state);
            let count = Arc::clone(&count);
            let epoch = Arc::clone(&epoch);
            thread::spawn(move || {
                thread::sleep(Duration::from_secs(EARLY_DONE_SECS));
                if epoch.load(Ordering::Relaxed) == generation
                    && *lock_or_recover(&count) > 0
                    && matches!(*lock_or_recover(&state), SearchState::Searching)
                {
                    *lock_or_recover(&state) = SearchState::Done;
                    crate::wake_ui();
                }
            });
        }

        let mut handles = Vec::new();
        for idx in targets {
            let ctx = ctx.clone();
            let results = Arc::clone(&results);
            let count = Arc::clone(&count);
            let answered = Arc::clone(&answered);
            handles.push(thread::spawn(move || {
                let r = fetch_into(&ctx, &idx, &results, &count);
                answered.fetch_add(1, Ordering::Relaxed);
                crate::wake_ui();
                (idx, r)
            }));
        }
        let (mut ok_count, mut first_err) = (0usize, None::<String>);
        for h in handles {
            match h.join() {
                Ok((_, Ok(_))) => ok_count += 1,
                Ok((idx, Err(e))) => {
                    // Keep the id: "2 indexers failed" without names is the same
                    // silent failure in a new costume.
                    let mut f = lock_or_recover(&failed);
                    if !f.contains(&idx) {
                        f.push(idx);
                    }
                    drop(f);
                    first_err.get_or_insert(e);
                }
                Err(_) => {}
            }
        }
        if live() {
            if ok_count > 0 {
                *lock_or_recover(&state) = SearchState::Done;
            } else {
                set_err(
                    &state,
                    first_err.unwrap_or_else(|| "No indexer answered".to_string()),
                );
            }
        }
        crate::wake_ui();
    });
}

// ─── Managed Jackett (start one, or install a copy once) ────────────────────
//
// Deliberately NOT bundled: Jackett is 48MB packed / 116MB unpacked and GPL-2.0
// (this app is MIT), and an AppImage payload is a read-only squashfs it could
// never update itself from. Instead we run a local copy the same way the user
// would, on localhost only.

/// Result of the last start/install attempt, polled (and taken) by the Settings
/// panel: `Ok((url, api_key))` or `Err(message)`.
pub(crate) static MANAGED: Mutex<Option<Result<(String, String), String>>> = Mutex::new(None);
/// True while a start/install worker is running.
pub(crate) static MANAGED_BUSY: AtomicBool = AtomicBool::new(false);
/// The Jackett we started, so exit can stop it (we only kill our own).
static JACKETT_CHILD: Mutex<Option<std::process::Child>> = Mutex::new(None);

fn managed_root() -> std::path::PathBuf {
    dirs_next::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("torrentx")
}
/// Jackett's own config: the user's existing one if present (it holds their
/// indexers), else a private one under our data dir.
fn user_config() -> std::path::PathBuf {
    dirs_next::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("Jackett/ServerConfig.json")
}
fn managed_config() -> std::path::PathBuf {
    managed_root().join("Jackett/ServerConfig.json")
}

/// A local Jackett binary: a copy we installed, or a system install.
pub(crate) fn find_bin() -> Option<std::path::PathBuf> {
    [
        managed_root().join("Jackett/jackett"),
        std::path::PathBuf::from("/opt/Jackett/jackett"),
        std::path::PathBuf::from("/usr/lib/jackett/jackett"),
        std::path::PathBuf::from("/usr/share/jackett/jackett"),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

/// `(port, api_key)` out of a Jackett ServerConfig.json.
pub(crate) fn read_server_config(path: &std::path::Path) -> Option<(u16, String)> {
    let txt = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&txt).ok()?;
    let port = u16::try_from(v.get("Port")?.as_u64()?).ok()?;
    let key = v.get("APIKey")?.as_str()?.to_string();
    (!key.is_empty()).then_some((port, key))
}

/// Jackett's release asset prefix for this architecture.
pub(crate) fn asset_prefix(arch: &str) -> Option<&'static str> {
    match arch {
        "x86_64" => Some("LinuxAMDx64"),
        "aarch64" => Some("LinuxARM64"),
        _ => None,
    }
}

/// A free localhost port.
/// ponytail: bind-then-release races with anything grabbing the port in the gap;
/// fine on a single-user desktop, and Jackett reports a failed bind which we surface.
pub(crate) fn free_port() -> Option<u16> {
    std::net::TcpListener::bind("127.0.0.1:0")
        .ok()
        .and_then(|l| l.local_addr().ok())
        .map(|a| a.port())
}

fn port_open(host: &str, port: u16) -> bool {
    let Ok(addr) = format!("{host}:{port}").parse() else {
        return false;
    };
    std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(400)).is_ok()
}

/// Download the official tarball and unpack it into our data dir.
fn install() -> Result<std::path::PathBuf, String> {
    let want = asset_prefix(std::env::consts::ARCH).ok_or_else(|| {
        format!(
            "no Jackett build for {} — install Jackett yourself",
            std::env::consts::ARCH
        )
    })?;
    let root = managed_root();
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let tar = root.join("jackett.tar.gz");

    let body = shared_client()
        .get("https://api.github.com/repos/Jackett/Jackett/releases/latest")
        .header("User-Agent", "torrentx")
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("release lookup failed: {}", e.without_url()))?
        .text()
        .map_err(|e| e.to_string())?;
    let json: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let url = json["assets"]
        .as_array()
        .and_then(|a| {
            a.iter().find_map(|x| {
                let n = x["name"].as_str()?;
                n.starts_with(want)
                    .then(|| x["browser_download_url"].as_str().map(String::from))
                    .flatten()
            })
        })
        .ok_or_else(|| format!("no {want} asset in the latest Jackett release"))?;

    let mut resp = shared_client().get(url).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("download failed: HTTP {}", resp.status()));
    }
    let mut f = std::fs::File::create(&tar).map_err(|e| e.to_string())?;
    std::io::copy(&mut resp, &mut f).map_err(|e| e.to_string())?; // streamed, ~48MB
    drop(f);

    let ok = std::process::Command::new("tar")
        .args(["xzf", &tar.to_string_lossy(), "-C", &root.to_string_lossy()])
        .status()
        .map_err(|e| format!("tar: {e}"))?
        .success();
    let _ = std::fs::remove_file(&tar);
    if !ok {
        return Err("could not unpack the Jackett archive".into());
    }
    find_bin().ok_or_else(|| "Jackett unpacked but no binary found".into())
}

/// Start a local Jackett and return `(url, api_key)`. Blocking (~48MB download /
/// ~10s start) so call it from a worker thread, via [`start_local_async`].
pub(crate) fn start_local(
    cfg_url: &str,
    cfg_key: &str,
    timeout_secs: u64,
) -> Result<(String, String), String> {
    let base = url::Url::parse(cfg_url).map_err(|e| format!("bad Jackett URL: {e}"))?;
    let host = base.host_str().unwrap_or("127.0.0.1").to_string();
    let cur_port = base.port_or_known_default().unwrap_or(9117);
    if port_open(&host, cur_port) {
        return Ok((cfg_url.to_string(), cfg_key.to_string())); // already running
    }

    let bin = find_bin().map_or_else(install, Ok)?;

    // Reuse the user's real Jackett config when it exists: it already has their
    // indexers. Otherwise give Jackett a private config dir (on Linux that is what
    // XDG_CONFIG_HOME selects — `--DataFolder` is explicitly not for Unix).
    let reuse = read_server_config(&user_config());
    let (env_cfg, port) = match &reuse {
        Some((p, _)) => (None, *p),
        None => (
            Some(managed_root()),
            free_port().ok_or("no free local port")?,
        ),
    };
    let cfg_path = if env_cfg.is_some() {
        managed_config()
    } else {
        user_config()
    };
    // Something already listening on that port IS the instance — e.g. the user's own
    // Jackett on the port from their ServerConfig — so there is nothing to start.
    let read_key = || {
        read_server_config(&cfg_path)
            .map(|(_, k)| k)
            .unwrap_or_else(|| cfg_key.to_string())
    };
    if port_open(&host, port) {
        return Ok((format!("http://{host}:{port}"), read_key()));
    }

    let mut cmd = std::process::Command::new(&bin);
    cmd.args(["--NoRestart", "--ListenPrivate"])
        .arg("--Port")
        .arg(port.to_string())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    if let Some(dir) = &env_cfg {
        // Jackett writes <XDG_CONFIG_HOME>/Jackett, so the parent has to exist —
        // don't rely on Jackett creating the whole chain on a fresh machine.
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        cmd.env("XDG_CONFIG_HOME", dir);
    }
    let child = cmd
        .spawn()
        .map_err(|e| format!("cannot start Jackett ({}): {e}", bin.display()))?;
    *lock_or_recover(&JACKETT_CHILD) = Some(child);

    let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs.max(15));
    while std::time::Instant::now() < deadline {
        if port_open(&host, port) {
            break;
        }
        thread::sleep(Duration::from_millis(500));
    }
    if !port_open(&host, port) {
        return Err(format!("Jackett did not come up on port {port}"));
    }

    // Fresh instances generate their key on first start.
    let key = read_key();
    Ok((format!("http://{host}:{port}"), key))
}

/// Run [`start_local`] on a worker thread; the Settings panel polls `MANAGED`.
pub(crate) fn start_local_async(url: String, key: String) {
    if MANAGED_BUSY.swap(true, Ordering::SeqCst) {
        return; // one at a time
    }
    *lock_or_recover(&MANAGED) = None;
    thread::spawn(move || {
        let r = start_local(&url, &key, 60);
        *lock_or_recover(&MANAGED) = Some(r);
        MANAGED_BUSY.store(false, Ordering::SeqCst);
        crate::wake_ui();
    });
}

/// Stop the Jackett we started (no-op if we didn't start one). Called on exit.
pub(crate) fn stop_local() {
    if let Some(mut c) = lock_or_recover(&JACKETT_CHILD).take() {
        let _ = c.kill();
        let _ = c.wait();
    }
}

#[cfg(test)]
mod deadline_tests {
    use super::indexer_timeout;

    /// Measured: one indexer took 12-18 s while the rest answered in <=1.2 s, and
    /// the fan-out waits for all of them. The cap is what bounds the search.
    #[test]
    fn the_all_search_caps_each_indexer_and_keeps_a_smaller_setting() {
        assert_eq!(indexer_timeout(120), 20);
        assert_eq!(indexer_timeout(45), 20);
        assert_eq!(indexer_timeout(5), 5, "a smaller timeout is still honoured");
    }
}

#[cfg(test)]
mod append_tests {
    use super::*;

    /// The early-Done watchdog lets a straggler append AFTER the settle pass, so
    /// the stored set has to dedupe as it grows — otherwise the badge and the
    /// table disagree, and the best-seeded copy can be the one that is dropped.
    #[test]
    fn appending_with_dedupe_keeps_the_best_seeded_and_the_count_in_step() {
        let results: Arc<Mutex<Arc<Vec<TorrentResult>>>> =
            Arc::new(Mutex::new(Arc::new(Vec::new())));
        let count = Arc::new(Mutex::new(0usize));
        let row = |seeds: u32| TorrentResult {
            title: "Slow Horses S06E06 1080p".into(),
            seeders: Some(seeds),
            ..Default::default()
        };
        append_results(&results, &count, vec![row(5), row(900)], true);
        assert_eq!(lock_or_recover(&results).len(), 1, "one release, one row");
        assert_eq!(*lock_or_recover(&count), 1, "count follows the rows");
        assert_eq!(lock_or_recover(&results)[0].seeders, Some(900));
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CATALOGUE_GROUPS, Hlth, INDEXER_CACHE_TTL, IndexerCatalogue, JackettResponse, SearchState,
        Tab, TorrentResult, asset_prefix, cache_fresh, catalogue_matches, category_id,
        dedupe_best_seeded, dedupe_key, failed_note, find_bin, fmt_size, free_port, group_entries,
        hlth_lbl, indexer_cache_path, is_magnet, managed_config, managed_root, normalize,
        parse_indexers_xml, parse_latest_tag, port_open, pub_date_key, pub_year,
        read_server_config, scrub_indexer_error, search_targets, set_err, start_local, stop_local,
        time_ago, truncate_magnet, urlenc, user_config, validate_jackett_url,
    };
    use crate::lock_or_recover;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn tab_key_round_trip() {
        for t in [Tab::Search, Tab::Favorites, Tab::Rss, Tab::About] {
            assert_eq!(Tab::from_key(t.key()), Some(t));
        }
        assert_eq!(Tab::from_key("Nonsense"), None);
    }

    #[test]
    fn category_mapping() {
        assert_eq!(category_id("Movies"), Some("2000"));
        assert_eq!(category_id("TV"), Some("5000"));
        assert_eq!(category_id("Music"), Some("3000"));
        assert_eq!(category_id("PC Games"), Some("4050"));
        assert_eq!(category_id("Software"), Some("4000"));
        assert_eq!(category_id("Anime"), Some("5070"));
        assert_eq!(category_id("Books"), Some("7000"));
        assert_eq!(category_id("XXX"), Some("6000"));
        assert_eq!(category_id("All"), None);
        assert_eq!(category_id("Nonsense"), None);
    }

    #[test]
    fn magnet_validation() {
        // Valid: 40-char hex info-hash
        let ok = "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567&dn=ubuntu.iso";
        assert!(is_magnet(ok));
        // Valid: 32-char base32
        let b32 = "magnet:?xt=urn:btih:JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP";
        assert!(is_magnet(b32));
        // Invalid: not a magnet / short hash / wrong scheme
        assert!(!is_magnet(""));
        assert!(!is_magnet("magnet:?xt=urn:btih:1234"));
        assert!(!is_magnet("http://example.com/file.torrent"));
        assert!(!is_magnet("magnet:?xt=urn:sha1:deadbeef"));
    }

    #[test]
    fn size_formatting() {
        assert_eq!(fmt_size(0), "0 B");
        assert_eq!(fmt_size(512), "512 B");
        assert_eq!(fmt_size(1_024), "1 KB");
        assert_eq!(fmt_size(1_048_576), "1 MB");
        assert_eq!(fmt_size(5_784_123_904), "5.39 GB");
    }

    #[test]
    fn normalize_strips_quality_and_case() {
        // Quality tags and file extensions are removed; case is lowered.
        assert_eq!(normalize("Ubuntu 22.04 1080p BluRay x264"), "ubuntu 22.04");
        // 2160p / UHD 4K releases dedupe with their 1080p counterparts.
        assert_eq!(normalize("Dune 2024 2160p UHD WEBRip"), "dune 2024");
        // Stops after 4 words.
        assert_eq!(
            normalize("one two three four five six"),
            "one two three four"
        );
        assert_eq!(normalize(""), "");
    }

    #[test]
    fn urlenc_basics() {
        assert_eq!(urlenc("hello world"), "hello+world");
        assert_eq!(urlenc("ubuntu-22.04"), "ubuntu-22.04");
        assert_eq!(urlenc("café"), "caf%C3%A9");
        assert_eq!(urlenc("a b/c"), "a+b%2Fc");
    }

    #[test]
    fn pub_year_extracts_year() {
        assert_eq!(pub_year("2024-05-01T10:00:00+00:00"), 2024);
        assert_eq!(pub_year("1999-01-01T00:00:00Z"), 1999);
        assert_eq!(pub_year("garbage"), 0);
        assert_eq!(pub_year(""), 0);
    }

    #[test]
    fn parses_latest_release_tag() {
        // Real GitHub "latest release" shape (with unrelated fields).
        let body = r#"{"url":"https://api.github.com/repos/x/y/releases/1",
            "tag_name":"v18.2.0","name":"Release","draft":false}"#;
        assert_eq!(parse_latest_tag(body).as_deref(), Some("v18.2.0"));
        assert_eq!(parse_latest_tag("not json"), None);
        assert_eq!(parse_latest_tag(r#"{"tag_name":42}"#), None);
    }

    #[test]
    fn is_magnet_accepts_any_xt_position_and_v2_hashes() {
        let h40 = "a".repeat(40);
        // Canonical form, and the reordered form real feeds send.
        assert!(is_magnet(&format!("magnet:?xt=urn:btih:{h40}")));
        assert!(is_magnet(&format!(
            "magnet:?dn=Name&xt=urn:btih:{h40}&tr=udp://t"
        )));
        // Trailing semicolon is tolerated, as before.
        assert!(is_magnet(&format!("magnet:?xt=urn:btih:{h40};")));
        // 32-char base32 info-hash.
        assert!(is_magnet(&format!(
            "magnet:?xt=urn:btih:{}",
            "A".repeat(32)
        )));
        // BitTorrent v2 multihash.
        assert!(is_magnet(&format!(
            "magnet:?xt=urn:btmh:1220{}",
            "b".repeat(64)
        )));
        // Still rejected: wrong hash length, bad chars, no xt, not a magnet.
        assert!(!is_magnet("magnet:?xt=urn:btih:tooshort"));
        assert!(!is_magnet(&format!(
            "magnet:?xt=urn:btih:{}",
            "z".repeat(40)
        )));
        assert!(!is_magnet("magnet:?dn=NoXt"));
        assert!(!is_magnet("http://example.invalid/x.torrent"));
        assert!(!is_magnet(""));
    }

    #[test]
    fn validates_jackett_url_requires_a_host() {
        // Scheme alone is not an address — these used to pass Save and then fail
        // every request.
        assert_eq!(
            validate_jackett_url("http://"),
            Some("URL must include a host, e.g. http://localhost:9117")
        );
        assert_eq!(
            validate_jackett_url("http:///api"),
            Some("URL must include a host, e.g. http://localhost:9117")
        );
        assert_eq!(
            validate_jackett_url("http://:9117"),
            Some("URL must include a host, e.g. http://localhost:9117")
        );
        // Whitespace is tolerated around a real host.
        assert_eq!(validate_jackett_url("  http://localhost:9117  "), None);
    }

    fn hashed(title: &str, tracker: &str, hash: &str, seeds: u32) -> TorrentResult {
        TorrentResult {
            title: title.into(),
            tracker: Some(tracker.into()),
            seeders: Some(seeds),
            peers: Some(1),
            size: Some(1),
            magnet_uri: Some(format!("magnet:?xt=urn:btih:{hash}&dn=x")),
            ..Default::default()
        }
    }

    #[test]
    fn the_same_infohash_from_three_trackers_is_one_row() {
        let rows = vec![
            hashed("Show S01 1080p-GRP", "apibay", "aa11", 5),
            hashed("Show.S01.1080p-GRP", "knaben", "AA11", 900),
            hashed("Show S01 1080p GRP", "other", "aa11", 40),
        ];
        let out = super::group_by_infohash(rows);
        assert_eq!(out.len(), 1, "one torrent, three listings");
        // The most-seeded copy survives — that is the one worth downloading.
        assert_eq!(out[0].tracker.as_deref(), Some("knaben"));
        assert_eq!(out[0].also_on.len(), 2);
        assert!(out[0].also_on.contains(&"apibay".to_string()));
        assert!(out[0].also_on.contains(&"other".to_string()));
    }

    #[test]
    fn different_infohashes_with_the_same_title_stay_separate() {
        // The exact case name-based dedupe gets wrong: two different torrents,
        // one title.
        let rows = vec![
            hashed("Big Buck Bunny 1080p", "a", "bb22", 10),
            hashed("Big Buck Bunny 1080p", "b", "cc33", 20),
        ];
        let out = super::group_by_infohash(rows);
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|r| r.also_on.is_empty()));
    }

    #[test]
    fn rows_without_a_magnet_or_hash_pass_through() {
        let rows = vec![
            TorrentResult {
                title: "No magnet".into(),
                tracker: Some("a".into()),
                ..Default::default()
            },
            TorrentResult {
                title: "Link only".into(),
                tracker: Some("b".into()),
                magnet_uri: Some("magnet:?dn=no-hash".into()),
                ..Default::default()
            },
        ];
        let out = super::group_by_infohash(rows);
        assert_eq!(out.len(), 2, "nothing to group on, nothing merged");
        assert!(out.iter().all(|r| r.also_on.is_empty()));
    }

    #[test]
    fn info_hash_parses_the_btih_field() {
        assert_eq!(
            super::info_hash("magnet:?xt=urn:btih:ABCDEF&dn=x").as_deref(),
            Some("abcdef")
        );
        // base32 stays base32, just lowercased: equal values still compare equal.
        assert_eq!(
            super::info_hash("magnet:?xt=urn:btih:abcdefghijklmnopqrstuvwxyz234567").as_deref(),
            Some("abcdefghijklmnopqrstuvwxyz234567")
        );
        assert_eq!(super::info_hash("magnet:?dn=x"), None);
        assert_eq!(super::info_hash("https://example.org/x.torrent"), None);
        assert_eq!(super::info_hash("magnet:?xt=urn:btih:&dn=x"), None);
    }

    #[test]
    fn dedupe_keeps_the_best_seeded_copy() {
        let row = |title: &str, seeds: u32, tracker: &str| TorrentResult {
            title: title.into(),
            seeders: Some(seeds),
            tracker: Some(tracker.into()),
            ..Default::default()
        };
        // Same title once normalised (case/whitespace folded) → one row survives,
        // and it must be the 900-seeder copy, not whichever came first.
        let out = dedupe_best_seeded(vec![
            row("Ubuntu Linux", 2, "1337x"),
            row("ubuntu  linux", 900, "thepiratebay"),
            row("Other Thing", 5, "yts"),
        ]);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].seeders, Some(900));
        assert_eq!(out[0].tracker.as_deref(), Some("thepiratebay"));
        assert_eq!(out[1].title, "Other Thing");
        // Feed order is preserved, and a tie keeps the earlier row.
        let tied = dedupe_best_seeded(vec![row("X", 7, "a"), row("X", 7, "b")]);
        assert_eq!(tied.len(), 1);
        assert_eq!(tied[0].tracker.as_deref(), Some("a"));
        assert!(dedupe_best_seeded(vec![]).is_empty());
    }

    #[test]
    fn unknown_seeders_are_not_dead() {
        // btdig publishes no seeder count: such a row is unknown, so it must stay
        // out of every band chip (especially DEAD — "we do not know" is not
        // "empty swarm") and remain visible under All.
        assert!(Hlth::All.ok_opt(None));
        for chip in [Hlth::Hot, Hlth::Good, Hlth::Slow, Hlth::Dying, Hlth::Dead] {
            assert!(
                !chip.ok_opt(None),
                "{} claimed an unknown row",
                chip.label()
            );
        }
        // A real zero still lands in DEAD; known values still follow the bands.
        assert!(Hlth::Dead.ok_opt(Some(0)));
        assert!(Hlth::Hot.ok_opt(Some(501)));
    }

    #[test]
    fn health_chips_match_the_row_badge_bands() {
        // The chip a user clicks must select exactly the rows it is named after.
        // Hlth::Dead used to cover 1..=10 too, while the row badge called those
        // DYING — so filtering "DEAD" listed rows labelled DYING.
        let chips = [Hlth::Hot, Hlth::Good, Hlth::Slow, Hlth::Dying, Hlth::Dead];
        for s in [0u32, 1, 5, 10, 11, 100, 101, 500, 501, 10_000] {
            let hits: Vec<&str> = chips
                .iter()
                .filter(|c| c.ok(s))
                .map(|c| c.label())
                .collect();
            assert_eq!(
                hits.len(),
                1,
                "{s} seeds must match exactly one chip: {hits:?}"
            );
            assert_eq!(hits[0], hlth_lbl(s), "badge/chip disagree at {s} seeds");
        }
    }

    #[test]
    fn pub_date_key_orders_across_formats() {
        // RFC 2822 — the format torznab pubDate usually uses. Lexical comparison
        // puts "07 May" before "12 Apr", which is backwards.
        let apr = pub_date_key("Fri, 12 Apr 2024 10:00:00 +0000");
        let may = pub_date_key("Tue, 07 May 2024 10:00:00 +0000");
        assert!(apr < may, "April must sort before May");
        assert_eq!(may, (2024, 5, 7, 10, 0, 0));
        // Same instant in RFC 3339 must order identically, so mixed feeds sort
        // coherently rather than in two independent blocks.
        let may_iso = pub_date_key("2024-05-07T10:00:00+00:00");
        assert_eq!(may, may_iso);
        // Within one day, the time breaks the tie.
        assert!(pub_date_key("2024-05-07T09:00:00Z") < pub_date_key("2024-05-07T17:30:00Z"));
        // A bare date still yields a usable (zeroed-time) key, and unparseable
        // input sorts first instead of panicking.
        assert_eq!(pub_date_key("2024-05-07"), (2024, 5, 7, 0, 0, 0));
        assert_eq!(pub_date_key("garbage"), (0, 0, 0, 0, 0, 0));
        assert_eq!(pub_date_key(""), (0, 0, 0, 0, 0, 0));
    }

    #[test]
    fn reads_jackett_server_config() {
        let p = std::env::temp_dir().join("tx_jackett_serverconfig_test.json");
        std::fs::write(&p, r#"{"Port":9117,"APIKey":"abc123","CacheEnabled":true}"#).unwrap();
        assert_eq!(read_server_config(&p), Some((9117, "abc123".to_string())));
        // An empty key must read as absent, so the caller falls back rather than
        // saving "" over a good key.
        std::fs::write(&p, r#"{"Port":9117,"APIKey":""}"#).unwrap();
        assert_eq!(read_server_config(&p), None);
        std::fs::write(&p, "not json at all").unwrap();
        assert_eq!(read_server_config(&p), None);
        assert_eq!(
            read_server_config(std::path::Path::new("/nonexistent/x.json")),
            None
        );
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn maps_arch_to_jackett_asset() {
        assert_eq!(asset_prefix("x86_64"), Some("LinuxAMDx64"));
        assert_eq!(asset_prefix("aarch64"), Some("LinuxARM64"));
        assert_eq!(asset_prefix("riscv64"), None); // refuse rather than fetch a wrong build
    }

    /// One request per indexer is what makes results appear progressively; a
    /// single chosen indexer must stay a single request.
    #[test]
    fn search_targets_split_all_into_each_indexer() {
        let configured = vec!["1337x".to_string(), "yts".to_string()];
        let none: Vec<String> = vec![];
        assert_eq!(search_targets("All", &configured, &none), configured);
        assert_eq!(search_targets("", &configured, &none), configured);
        assert_eq!(search_targets("yts", &configured, &none), vec!["yts"]);
        // Nothing fetched yet: Jackett's own aggregate endpoint.
        assert_eq!(search_targets("All", &[], &none), vec!["all"]);
    }

    /// A switched-off indexer is skipped by the "All" fan-out, and a list that
    /// disables everything still leaves one target instead of none.
    #[test]
    fn disabled_indexers_are_skipped_but_never_all_of_them() {
        let configured = vec!["1337x".to_string(), "yts".to_string(), "tpb".to_string()];
        let off = vec!["yts".to_string()];
        assert_eq!(
            search_targets("All", &configured, &off),
            vec!["1337x", "tpb"]
        );

        let all_off = configured.clone();
        assert_eq!(search_targets("All", &configured, &all_off), vec!["1337x"]);

        // A stale name that no longer exists must not match anything.
        let stale = vec!["gone".to_string()];
        assert_eq!(search_targets("All", &configured, &stale), configured);

        // Picking one indexer explicitly still works even if it is switched off.
        assert_eq!(search_targets("yts", &configured, &off), vec!["yts"]);
    }

    /// The fan-out must publish each indexer's rows as they arrive: the fast
    /// tracker's result is visible while the slow one is still in flight.
    /// A poisoned state lock used to make `set_err` a silent no-op: the error
    /// was dropped and the UI kept showing "Searching" forever. It must land.
    #[test]
    fn set_err_still_records_the_failure_on_a_poisoned_lock() {
        use std::sync::{Arc, Mutex};

        let st = Arc::new(Mutex::new(SearchState::Idle));
        let s2 = Arc::clone(&st);
        let _ = std::thread::spawn(move || {
            let _g = s2.lock().unwrap();
            panic!("worker dies holding the state lock");
        })
        .join();
        assert!(st.lock().is_err(), "poisoned");

        set_err(&st, "boom".into());

        let guard = lock_or_recover(&st);
        assert!(
            matches!(&*guard, SearchState::Error(m) if m == "boom"),
            "state stayed {guard:?}"
        );
    }

    /// A worker that panicked while holding the state lock poisons it. The GUI
    /// must keep working: `lock_or_recover` returns the data instead of panicking.
    #[test]
    fn lock_or_recover_survives_a_poisoned_mutex() {
        use std::sync::{Arc, Mutex};

        let state = Arc::new(Mutex::new(7u32));
        let s2 = Arc::clone(&state);
        let _ = std::thread::spawn(move || {
            let mut g = s2.lock().unwrap();
            *g = 9;
            panic!("holder dies with the lock held");
        })
        .join(); // the panic is expected

        assert!(state.lock().is_err(), "the mutex must actually be poisoned");
        assert_eq!(*lock_or_recover(&state), 9, "recovered value is readable");
        *lock_or_recover(&state) = 11;
        assert_eq!(*lock_or_recover(&state), 11, "and writable");
    }

    /// The error message must not carry the API key. reqwest's Display embeds the
    /// full request URL (query string and all), which put `apikey=…` on screen in
    /// the fault-injection pass whenever Jackett answered with HTML or bad JSON.
    #[test]
    fn a_parse_error_never_leaks_the_api_key() {
        use super::{SearchCtx, fetch_into};
        use std::io::Write;
        use std::net::TcpListener;
        use std::sync::{Arc, Mutex};

        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            use std::io::Read;
            if let Ok((mut s, _)) = l.accept() {
                let mut req = [0u8; 1024];
                let _ = s.read(&mut req);
                let body = "<html>not json</html>";
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = s.write_all(head.as_bytes());
                let _ = s.write_all(body.as_bytes());
                let _ = s.flush();
            }
        });

        let ctx = SearchCtx {
            url: format!("http://127.0.0.1:{port}"),
            key: "SECRETKEY123".into(),
            query: "iso".into(),
            cat: "All".into(),
            timeout: 5,
            dedupe: false,
        };
        let results = Arc::new(Mutex::new(Arc::new(vec![])));
        let count = Arc::new(Mutex::new(0usize));
        let err = fetch_into(&ctx, "alpha", &results, &count).unwrap_err();

        assert!(err.contains("Unexpected response from Jackett"), "{err}");
        assert!(!err.contains("SECRETKEY123"), "the API key leaked: {err}");
        assert!(!err.contains("apikey"), "the query string leaked: {err}");
    }

    /// End to end through the real fetch path: one torrent listed by two
    /// indexers must arrive as one row that names the other indexer.
    #[test]
    fn one_infohash_from_two_indexers_arrives_as_one_row() {
        use super::{SearchCtx, fetch_into};
        use std::net::TcpListener;
        use std::sync::{Arc, Mutex};

        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            for _ in 0..2 {
                let Ok((mut s, _)) = l.accept() else { return };
                let mut buf = [0u8; 2048];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                // Same torrent, same infohash, two indexers, different seed counts.
                let seeds = if req.contains("/beta/") { 900 } else { 5 };
                let tracker = if req.contains("/beta/") {
                    "beta"
                } else {
                    "alpha"
                };
                let body = format!(
                    r#"{{"Results":[{{"Title":"Show S01 1080p-GRP","Tracker":"{tracker}","Seeders":{seeds},"Peers":3,"Size":1000,"MagnetUri":"magnet:?xt=urn:btih:aa11&dn=x"}}],"Indexers":[]}}"#
                );
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = s.write_all(head.as_bytes());
                let _ = s.write_all(body.as_bytes());
                let _ = s.flush();
            }
        });

        let ctx = SearchCtx {
            url: format!("http://127.0.0.1:{port}"),
            key: "k".into(),
            query: "show".into(),
            cat: "All".into(),
            timeout: 5,
            dedupe: true,
        };
        let results = Arc::new(Mutex::new(Arc::new(vec![])));
        let count = Arc::new(Mutex::new(0usize));
        fetch_into(&ctx, "alpha", &results, &count).unwrap();
        fetch_into(&ctx, "beta", &results, &count).unwrap();

        let rows = (**lock_or_recover(&results)).clone();
        assert_eq!(rows.len(), 1, "one torrent → one row");
        assert_eq!(rows[0].tracker.as_deref(), Some("beta"), "best-seeded wins");
        assert_eq!(rows[0].also_on, vec!["alpha".to_string()]);
        assert_eq!(
            *lock_or_recover(&count),
            1,
            "the count agrees with the table"
        );
    }

    /// The aggregate route reports a failed indexer inside a 200 body. Reading
    /// only `Results` turned a Cloudflare block into "0 results", and the app had
    /// no way to say which tracker was dead.
    #[test]
    fn a_failed_indexer_inside_a_200_body_becomes_an_error() {
        let body = r#"{"Results":[],"Indexers":[{"ID":"1337x","Name":"1337x","Status":1,"Results":0,"Error":"Jackett.Common.IndexerException: Exception (1337x): Connection reset by peer for url (http://localhost:9117/api/v2.0/indexers/1337x/results?apikey=SECRETKEY&Query=x) ---> inner"}]}"#;
        let parsed: JackettResponse = serde_json::from_str(body).unwrap();
        assert!(parsed.results.is_empty());
        assert_eq!(parsed.indexers.len(), 1);
        let msg = scrub_indexer_error(parsed.indexers[0].error.as_deref().unwrap_or(""));
        assert!(msg.contains("Connection reset by peer"), "{msg}");
        assert!(!msg.contains("SECRETKEY"), "the API key leaked: {msg}");
        assert!(!msg.contains("apikey="), "the query string leaked: {msg}");
        assert!(msg.len() <= 140);
    }

    /// `"Error": null` is what Jackett sends for a healthy indexer. Typing the
    /// field as String made serde reject the whole body, so every working indexer
    /// looked broken — the regression that turned a good search into an error.
    #[test]
    fn a_null_indexer_error_still_parses() {
        let body = r#"{"Results":[{"Title":"x","Seeders":5}],"Indexers":[{"ID":"therarbg","Name":"TheRARBG","Status":2,"Results":11,"Error":null}]}"#;
        let parsed: JackettResponse = serde_json::from_str(body).unwrap();
        assert_eq!(parsed.results.len(), 1);
        assert_eq!(parsed.indexers[0].id.as_deref(), Some("therarbg"));
        assert!(parsed.indexers[0].error.is_none());
    }

    /// An indexer that answers with an error must be named in the status line.
    /// Without this, a Cloudflare-blocked tracker is indistinguishable from one
    /// that simply had no results — the failure that hid 1337x/eztv for days.
    #[test]
    fn fan_out_records_which_indexer_failed() {
        use super::start_search;
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::sync::atomic::{AtomicU64, AtomicUsize};
        use std::sync::{Arc, Mutex};
        use std::time::{Duration, Instant};

        let srv = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = srv.local_addr().unwrap();
        std::thread::spawn(move || {
            for _ in 0..2 {
                let Ok((mut s, _)) = srv.accept() else { return };
                let mut buf = [0u8; 2048];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                std::thread::spawn(move || {
                    let resp = if req.contains("/indexers/slow/") {
                        let body = r#"{"error":"blocked"}"#;
                        format!(
                            "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            body.len(),
                            body
                        )
                    } else {
                        let body = r#"{"Results":[{"Title":"x","Seeders":5}]}"#;
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            body.len(),
                            body
                        )
                    };
                    let _ = s.write_all(resp.as_bytes());
                });
            }
        });

        let failed: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(vec![]));
        start_search(
            format!("http://{addr}"),
            "key".to_string(),
            "q".to_string(),
            "All".to_string(),
            vec!["fast".to_string(), "slow".to_string()],
            5,
            Arc::new(Mutex::new(Arc::new(Vec::new()))),
            Arc::new(Mutex::new(super::SearchState::Idle)),
            Arc::new(Mutex::new(0usize)),
            Arc::new(AtomicU64::new(1)),
            Arc::new(AtomicUsize::new(0)),
            Arc::clone(&failed),
            false,
            1,
        );

        let t0 = Instant::now();
        while Instant::now() - t0 < Duration::from_secs(5) && failed.lock().unwrap().is_empty() {
            std::thread::sleep(Duration::from_millis(20));
        }
        let f = failed.lock().unwrap().clone();
        assert_eq!(
            f,
            vec!["slow".to_string()],
            "failed indexers were not named"
        );
    }

    #[test]
    fn fan_out_publishes_the_fast_indexer_before_the_slow_one() {
        use super::{SearchState, start_search};
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::sync::atomic::AtomicU64;
        use std::sync::{Arc, Mutex};
        use std::time::{Duration, Instant};

        let srv = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = srv.local_addr().unwrap();
        std::thread::spawn(move || {
            for _ in 0..2 {
                let Ok((mut s, _)) = srv.accept() else { return };
                let mut buf = [0u8; 2048];
                let n = s.read(&mut buf).unwrap_or(0);
                let slow = String::from_utf8_lossy(&buf[..n]).contains("/indexers/slow/");
                std::thread::spawn(move || {
                    if slow {
                        std::thread::sleep(Duration::from_millis(400));
                    }
                    let body = r#"{"Results":[{"Title":"x","Seeders":5}]}"#;
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = s.write_all(resp.as_bytes());
                });
            }
        });

        let results: Arc<Mutex<Arc<Vec<TorrentResult>>>> =
            Arc::new(Mutex::new(Arc::new(Vec::new())));
        let state = Arc::new(Mutex::new(SearchState::Idle));
        let count = Arc::new(Mutex::new(0usize));
        let epoch = Arc::new(AtomicU64::new(1));
        let answered = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        start_search(
            format!("http://{addr}"),
            "key".to_string(),
            "q".to_string(),
            "All".to_string(),
            vec!["fast".to_string(), "slow".to_string()],
            5,
            Arc::clone(&results),
            Arc::clone(&state),
            Arc::clone(&count),
            Arc::clone(&epoch),
            Arc::clone(&answered),
            Arc::new(Mutex::new(Vec::new())),
            false,
            1,
        );

        let wait = |n: usize| {
            let t0 = Instant::now();
            while Instant::now() - t0 < Duration::from_secs(5) {
                if *count.lock().unwrap() >= n {
                    return true;
                }
                thread::sleep(Duration::from_millis(20));
            }
            false
        };
        assert!(wait(1), "the fast indexer's rows must arrive first");
        assert_eq!(
            answered.load(std::sync::atomic::Ordering::Relaxed),
            1,
            "only the fast indexer has answered while the slow one is in flight"
        );
        // One row, not both, and still Searching: the slow indexer is in flight.
        // This PAIR is the ordering proof — a batched implementation would
        // publish both rows together, so the first observation would already be
        // count == 2 with state Done. No wall-clock bound: a 300 ms assertion
        // against a 400 ms server failed on a loaded CI runner at 662 ms while
        // the behaviour was correct.
        assert_eq!(*count.lock().unwrap(), 1);
        assert_eq!(*state.lock().unwrap(), SearchState::Searching);
        assert!(wait(2), "both indexers must land");
        // Then WAIT for Done instead of reading it once: the last row is
        // appended by its worker, but the state flips only after the parent
        // thread joins every handle, so `count == 2` can be observed a few
        // microseconds before `Done` (measured: a CI run asserting immediately
        // read `Searching`). Polling an eventual state is deterministic; the
        // ordering proof above is the `count == 1 && Searching` pair.
        let wait_done = || {
            let t0 = Instant::now();
            while Instant::now() - t0 < Duration::from_secs(5) {
                if matches!(*lock_or_recover(&state), SearchState::Done) {
                    return true;
                }
                thread::sleep(Duration::from_millis(20));
            }
            false
        };
        assert!(wait_done(), "the search must reach Done, not Error or hang");
        assert_eq!(results.lock().unwrap().len(), 2);
    }

    /// Jackett's torznab `pubDate` is RFC 2822 ("Tue, 07 May 2024 10:20:30 +0000").
    /// `time_ago` only knew RFC 3339, so it fell through to `.get(..10)` and the
    /// RSS list showed the chopped string "Tue, 07 Ma".
    #[test]
    fn time_ago_parses_rfc2822() {
        assert_eq!(time_ago("Tue, 07 May 2024 10:20:30 +0000"), "2024-05-07");
        // RFC 3339 must keep working.
        assert_eq!(time_ago("2024-05-07T10:20:30+00:00"), "2024-05-07");
    }

    /// Regression from live Jackett data. `normalize()` alone keys on the first four
    /// words, which merged all three of these films into a single row (11 more such
    /// merges in that sample, and 10 in an Ubuntu sample — including 2022 vs 2023).
    #[test]
    fn dedupe_key_keeps_different_episodes_apart() {
        let k = dedupe_key;
        // Three different films whose first four words match ("one piece episode of").
        let east_blue = k(
            "One Piece Episode of East Blue Luffy to 4-nin no Nakama no Daiboken (2017) 1080p BRRip 5.1 x264 -YTS",
        );
        let sabo = k(
            "One Piece Episode of Sabo Bond of Three Brothers, A Miraculous Reunion and an Inherited Will (2015) 1080p BRRip 5.1 x264 -YTS",
        );
        let skypiea = k("One Piece Episode of Skypiea (2018) 1080p BRRip x264 -YTS");
        assert_ne!(east_blue, sabo);
        assert_ne!(sabo, skypiea);
        // Adjacent episodes of a long runner (the tracker's own numbering).
        assert_ne!(
            k("[Erai-raws] One Piece - 1177 [1080p CR WEBRip HEVC AAC][MultiSub][4AFFFCF0]"),
            k("[Erai-raws] One Piece - 1178 [1080p CR WEBRip HEVC AAC][MultiSub][5B00D1D2]")
        );
        // Season/episode form.
        assert_ne!(
            k("ONE PIECE 2023 S02E03 1080p WEBRip HDR10 10bit DDP5 1 HEVC-d3g"),
            k("ONE PIECE 2023 S02E07 1080p WEBRip HDR10 10bit DDP5 1 HEVC-d3g")
        );
        // Different OS releases.
        assert_ne!(
            k("ubuntu-22.04.3-desktop-amd64.iso"),
            k("ubuntu-23.10-desktop-amd64.iso")
        );
    }

    /// The other half of the deal: the same release in different encodings must STILL
    /// collapse, or dedupe stops doing anything useful.
    #[test]
    fn dedupe_key_still_merges_the_same_release() {
        let k = dedupe_key;
        assert_eq!(
            k("ONE PIECE 2023 S02E03 1080p WEBRip HDR10 10bit DDP5 1 HEVC-d3g"),
            k("ONE PIECE 2023 S02E03 720p HEVC x265-MeGusta")
        );
        // Resolutions and codec numbers are encoding, not content.
        assert_eq!(
            k("[ToonsHub] One Piece EP1178 1080p BILI WEB-DL AAC2.0 H.265 (Multi-Subs)"),
            k("[ToonsHub] One Piece EP1178 1080p BILI WEB-DL AAC2.0 H.264 (Multi-Subs)")
        );
        // Titles with no numbers at all behave exactly as before.
        assert_eq!(k("Ubuntu Linux"), k("ubuntu  linux"));
    }

    #[test]
    fn free_port_is_actually_free() {
        let p = free_port().expect("should find a free port");
        assert!(p > 0);
        std::net::TcpListener::bind(("127.0.0.1", p)).expect("port must be bindable");
    }

    /// The only test that starts a real Jackett. Ignored by default (needs a local
    /// Jackett binary and ~10s); run with `cargo test -- --ignored`.
    ///
    /// Isolation matters: XDG_CONFIG_HOME is redirected to a temp dir, so
    /// `user_config()` does not exist and the managed branch runs — spawning an
    /// isolated instance on a free port instead of colliding with the Jackett the
    /// user already has on their own port.
    #[test]
    #[ignore = "starts a real Jackett; run explicitly with --ignored"]
    fn starts_a_real_jackett_isolated() {
        if find_bin().is_none() {
            eprintln!("skipped: no local Jackett binary");
            return;
        }
        let xdg = std::env::temp_dir().join("tx_jackett_e2e_xdg");
        let _ = std::fs::remove_dir_all(&xdg);
        std::fs::create_dir_all(&xdg).unwrap();
        // SAFETY: test-only, set before the managed-Jackett code under test reads it.
        unsafe { std::env::set_var("XDG_CONFIG_HOME", &xdg) };
        // Leave the machine as found: only clean up a managed dir this test created.
        let pre_existing = managed_root().exists();
        assert!(
            read_server_config(&user_config()).is_none(),
            "redirect failed — refusing to spawn against the user's real Jackett config"
        );

        // Port 9 (discard) is never Jackett's, so the "already running" check misses.
        let (url, key) = start_local("http://127.0.0.1:9", "", 60).expect("start");
        let port: u16 = url.rsplit(':').next().unwrap().parse().unwrap();
        assert_ne!(
            port, 9117,
            "must not have taken over the user's port: {url}"
        );
        assert!(
            port_open("127.0.0.1", port),
            "Jackett should be listening: {url}"
        );
        assert!(!key.is_empty(), "a generated API key was expected");
        // The key we hand back is the one Jackett wrote.
        assert_eq!(
            read_server_config(&managed_config()).map(|(_, k)| k),
            Some(key)
        );

        stop_local();
        for _ in 0..20 {
            if !port_open("127.0.0.1", port) {
                break;
            }
            thread::sleep(Duration::from_millis(300));
        }
        assert!(
            !port_open("127.0.0.1", port),
            "stop_local must stop what it started"
        );
        let _ = std::fs::remove_dir_all(&xdg);
        if !pre_existing {
            let _ = std::fs::remove_dir_all(managed_root());
        }
    }

    #[test]
    fn validates_jackett_url_scheme() {
        assert_eq!(validate_jackett_url("http://localhost:9117"), None);
        assert_eq!(validate_jackett_url("https://jackett.example.com"), None);
        assert_eq!(
            validate_jackett_url("localhost:9117"),
            Some("URL must start with http:// or https://")
        );
        assert_eq!(
            validate_jackett_url("ftp://host"),
            Some("URL must start with http:// or https://")
        );
        assert_eq!(validate_jackett_url(""), Some("URL is empty"));
        assert_eq!(validate_jackett_url("   "), Some("URL is empty"));
    }

    #[test]
    fn parses_indexer_list() {
        let xml = r#"<?xml version="1.0"?>
<indexers>
  <indexer id="yts" configured="true"><title>YTS</title></indexer>
  <indexer id="thepiratebay" configured="false"><title>TPB</title></indexer>
  <indexer id="rarbg" configured="true"/>
  <indexer configured="true"><title>No ID</title></indexer>
</indexers>"#;
        let cat = parse_indexers_xml(xml).unwrap();
        // Configured ones sorted+deduped; the id-less entry is skipped.
        assert_eq!(cat.configured, vec!["rarbg", "yts"]);
        // Everything Jackett knows, configured or not, in document order.
        assert_eq!(cat.all, vec!["yts", "thepiratebay", "rarbg"]);
        assert_eq!(cat.unconfigured().collect::<Vec<_>>(), vec!["thepiratebay"]);
        // Forgiving parser: malformed XML yields an empty catalogue, not an error.
        assert!(parse_indexers_xml("<broken>").unwrap().all.is_empty());
    }

    /// The status line names what failed, but caps the list: a whole-outage
    /// search must not print 600 ids into one label.
    #[test]
    fn failed_note_names_up_to_three_indexers() {
        assert_eq!(failed_note(&[]), "");
        assert_eq!(
            failed_note(&["eztv".into(), "1337x".into()]),
            "  ·  2 failed: 1337x, eztv"
        );
        assert_eq!(super::failed_note_short(&[]), "");
        assert_eq!(
            super::failed_note_short(&["eztv".into(), "1337x".into()]),
            "  ·  2 failed"
        );
        assert_eq!(
            failed_note(&["d".into(), "c".into(), "b".into(), "a".into(), "e".into()]),
            "  ·  5 failed: a, b, c +2"
        );
    }

    /// The catalogue cache is keyed by url+key, never by a name that could leak
    /// the key, and staleness is a plain age comparison.
    #[test]
    fn indexer_cache_is_keyed_by_url_and_key_and_expires() {
        let a = indexer_cache_path("http://localhost:9117", "KEYONE");
        let b = indexer_cache_path("http://localhost:9117", "KEYTWO");
        let c = indexer_cache_path("http://localhost:9117", "KEYONE");
        assert_eq!(a, c, "same server+key must map to the same file");
        assert_ne!(a, b, "another key must not reuse the cache");
        let name = a.file_name().unwrap().to_string_lossy().to_string();
        assert!(
            !name.contains("KEYONE"),
            "the API key must not appear in the path"
        );

        assert!(cache_fresh(Duration::from_secs(60)));
        assert!(cache_fresh(INDEXER_CACHE_TTL));
        assert!(!cache_fresh(INDEXER_CACHE_TTL + Duration::from_secs(1)));
    }

    /// Groups only advertise ids Jackett actually returns, and never ones already
    /// configured — the Indian group is exactly desigaane + desitorrents-api because
    /// Jackett ships no Telugu/Tamil/Malayalam definition at all.
    #[test]
    fn group_entries_intersect_the_catalogue_and_skip_configured() {
        let cat = IndexerCatalogue {
            configured: vec!["1337x".into()],
            all: vec![
                "1337x".into(),
                "thepiratebay".into(),
                "knaben".into(),
                "nyaasi".into(),
                "desigaane".into(),
                "unlisted-in-any-group".into(),
            ],
        };
        let popular = group_entries(&cat, CATALOGUE_GROUPS[0].1);
        let popular: Vec<&str> = popular.iter().map(|s| s.as_str()).collect();
        assert_eq!(popular, vec!["thepiratebay", "knaben"]); // 1337x configured, yts absent
        let desi: Vec<&str> = group_entries(&cat, CATALOGUE_GROUPS[2].1)
            .iter()
            .map(|s| s.as_str())
            .collect();
        assert_eq!(desi, vec!["desigaane"]); // desitorrents-api not in this catalogue
        assert!(group_entries(&cat, &["not-in-this-catalogue"]).is_empty());
    }

    /// The catalogue popup filters on what is *not* configured and caps the rows.
    #[test]
    fn catalogue_matches_filters_unconfigured_and_caps() {
        let cat = IndexerCatalogue {
            configured: vec!["yts".into()],
            all: vec![
                "yts".into(),
                "1337x".into(),
                "piratebay1337".into(),
                "eztv".into(),
            ],
        };
        let hits: Vec<&str> = catalogue_matches(&cat, "1337", 10)
            .into_iter()
            .map(|s| s.as_str())
            .collect();
        assert_eq!(hits, vec!["1337x", "piratebay1337"]); // configured `yts` never listed
        assert_eq!(catalogue_matches(&cat, "", 2).len(), 2); // capped
        assert!(catalogue_matches(&cat, "no-such-indexer", 10).is_empty());
    }

    #[test]
    fn truncate_magnet_cuts_on_char_boundary() {
        // Short → unchanged (no ellipsis).
        assert_eq!(
            truncate_magnet("magnet:?xt=urn:btih:abcd", 100),
            "magnet:?xt=urn:btih:abcd"
        );
        // ASCII: cuts at byte 57, then appends the 3-byte ellipsis.
        let long = "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567&dn=abcdefghijklmnopqrstuvwxyz";
        let t = truncate_magnet(long, 57);
        assert!(t.ends_with('…'));
        // The kept body is the 0..57 prefix (57 bytes), plus ellipsis (3).
        assert_eq!(t.len(), 60);
        let body = t.trim_end_matches('…');
        assert!(long.starts_with(body));
        assert!(body.is_char_boundary(body.len()));
    }

    #[test]
    fn truncate_magnet_handles_multibyte_without_panic() {
        // dn= with multibyte UTF-8 (Japanese). Byte-slicing [..57] would panic
        // on a mid-char boundary; char_indices must not.
        let s =
            "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567&dn=日本語タイトルテスト";
        let t = truncate_magnet(s, 57);
        // Result must be valid UTF-8 (didn't panic) and end with the ellipsis.
        assert!(t.ends_with('…'));
        // The body (before ellipsis) must cut on a char boundary.
        let body = t.trim_end_matches('…');
        assert!(body.is_char_boundary(body.len()));
        // Re-slice the body must not panic.
        let _ = &body[..body.len()];
    }
}
