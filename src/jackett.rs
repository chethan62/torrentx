// ─── Jackett types ─────────────────────────────────────────────────────────
use crate::themes::rgb;
use eframe::egui::Color32;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct JackettResponse {
    #[serde(default)]
    pub(crate) results: Vec<TorrentResult>,
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
    pub(crate) fn width(&self) -> f32 {
        match self {
            TableCol::Name => 295.0,
            TableCol::Tracker => 84.0,
            TableCol::Size => 72.0,
            TableCol::Seeds => 62.0,
            TableCol::Leech => 62.0,
            TableCol::Ratio => 56.0,
            TableCol::Health => 72.0,
            TableCol::Date => 84.0,
        }
    }
}

#[derive(Clone, PartialEq)]
pub(crate) enum SearchState {
    Idle,
    Searching,
    Done,
    Error(String),
}

#[derive(Clone, PartialEq)]
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
    if let Ok(mut s) = st.lock() {
        *s = SearchState::Error(msg);
    }
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

/// Keep one row per normalised title — the best-seeded one.
///
/// Deduping with a seen-set while filtering kept whichever tracker Jackett listed
/// first, so the surviving row (the one the user downloads) could be a 1-seeder
/// copy while a 900-seeder copy was discarded. Ties keep the earlier row, so feed
/// order still breaks equal-seed cases.
pub(crate) fn dedupe_best_seeded(rows: Vec<TorrentResult>) -> Vec<TorrentResult> {
    let mut best: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (i, r) in rows.iter().enumerate() {
        let key = normalize(&r.title);
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
        if low.len() >= 3 {
            if let Some(i) = MONTHS.iter().position(|m| low.starts_with(m)) {
                mon = i as u32 + 1;
                continue;
            }
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
pub(crate) fn fetch_indexers(url: &str, key: &str) -> Option<Vec<String>> {
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
    parse_indexers_xml(&body)
}

/// Pull configured indexer ids out of a Jackett `t=indexers` Torznab XML body.
/// Pure and unit-testable without a live server.
fn parse_indexers_xml(body: &str) -> Option<Vec<String>> {
    use quick_xml::events::Event;
    use quick_xml::Reader;
    let mut reader = Reader::from_str(body);
    reader.config_mut().trim_text(true);
    let mut out = vec![];
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
                if configured {
                    if let Some(id) = id {
                        if !id.is_empty() {
                            out.push(id);
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
        buf.clear();
    }
    out.sort();
    out.dedup();
    Some(out)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn start_search(
    url: String,
    key: String,
    query: String,
    cat: String,
    indexer: String,
    timeout: u64,
    results: Arc<Mutex<Arc<Vec<TorrentResult>>>>,
    state: Arc<Mutex<SearchState>>,
    count: Arc<Mutex<usize>>,
    epoch: Arc<AtomicU64>,
    gen: u64,
) {
    thread::spawn(move || {
        // Epoch guard: a newer search invalidates this one — never touch state,
        // so a slow old response can't overwrite fresh results (search race).
        if epoch.load(Ordering::Relaxed) != gen {
            return;
        }
        let live = || epoch.load(Ordering::Relaxed) == gen;
        if let Ok(mut s) = state.lock() {
            *s = SearchState::Searching;
        }
        let idx = if indexer.is_empty() || indexer == "All" {
            "all"
        } else {
            indexer.as_str()
        };
        let mut ep = format!(
            "{}/api/v2.0/indexers/{}/results?apikey={}&Query={}",
            url.trim_end_matches('/'),
            urlenc(idx),
            urlenc(&key),
            urlenc(&query)
        );
        if cat != "All" {
            if let Some(id) = category_id(&cat) {
                ep.push_str(&format!("&Category[]={}", id));
            }
        }

        match shared_client()
            .get(&ep)
            .timeout(Duration::from_secs(timeout))
            .send()
        {
            Ok(resp) => {
                let st = resp.status();
                if st.is_success() {
                    match resp.json::<JackettResponse>() {
                        Ok(data) => {
                            if !live() {
                                return;
                            }
                            let n = data.results.len();
                            if let Ok(mut r) = results.lock() {
                                *r = Arc::new(data.results);
                            }
                            if let Ok(mut c) = count.lock() {
                                *c = n;
                            }
                            if let Ok(mut s) = state.lock() {
                                *s = SearchState::Done;
                            }
                        }
                        Err(e) => {
                            if live() {
                                set_err(&state, format!("Parse error: {e}"))
                            }
                        }
                    }
                } else if live() {
                    set_err(
                        &state,
                        match st.as_u16() {
                            401 => "Invalid API key — open Settings to update it.".into(),
                            403 => "Forbidden — check Jackett permissions.".into(),
                            404 => "Jackett endpoint not found — verify URL in Settings.".into(),
                            500 => "Jackett internal error — check Jackett logs.".into(),
                            n => format!("HTTP {n} from Jackett"),
                        },
                    );
                }
            }
            Err(e) => {
                if live() {
                    set_err(
                        &state,
                        if e.is_connect() {
                            format!(
                                "Cannot reach Jackett at {url}\nSettings → Start Jackett, or: sudo systemctl start jackett"
                            )
                        } else if e.is_timeout() {
                            format!("Timed out after {timeout}s — increase timeout in Settings")
                        } else {
                            // Strip the URL: reqwest's Display appends
                            // " for url (…)" and the query string carries the
                            // Jackett API key, which must not reach the UI.
                            format!("Network error: {}", e.without_url())
                        },
                    )
                }
            }
        };
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
    *JACKETT_CHILD.lock().unwrap() = Some(child);

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
    *MANAGED.lock().unwrap() = None;
    thread::spawn(move || {
        let r = start_local(&url, &key, 60);
        *MANAGED.lock().unwrap() = Some(r);
        MANAGED_BUSY.store(false, Ordering::SeqCst);
        crate::wake_ui();
    });
}

/// Stop the Jackett we started (no-op if we didn't start one). Called on exit.
pub(crate) fn stop_local() {
    if let Some(mut c) = JACKETT_CHILD.lock().unwrap().take() {
        let _ = c.kill();
        let _ = c.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        asset_prefix, category_id, dedupe_best_seeded, find_bin, fmt_size, free_port, hlth_lbl,
        is_magnet, managed_config, managed_root, normalize, parse_indexers_xml, parse_latest_tag,
        port_open, pub_date_key, pub_year, read_server_config, start_local, stop_local,
        truncate_magnet, urlenc, user_config, validate_jackett_url, Hlth, Tab, TorrentResult,
    };
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
        std::env::set_var("XDG_CONFIG_HOME", &xdg);
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
        let ids = parse_indexers_xml(xml).unwrap();
        // Only configured ones, sorted+deduped; missing-id ones skipped.
        assert_eq!(ids, vec!["rarbg", "yts"]);
        // Forgiving parser: malformed XML yields empty list, not an error.
        assert!(parse_indexers_xml("<broken>").unwrap().is_empty());
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
