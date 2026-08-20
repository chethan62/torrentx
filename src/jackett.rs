// ─── Jackett types ─────────────────────────────────────────────────────────
use crate::themes::rgb;
use eframe::egui::Color32;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct JackettResponse {
    #[serde(default)]
    pub(crate) results: Vec<TorrentResult>,
}

#[derive(Deserialize, Debug, Clone)]
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
pub(crate) enum SortCol { Name, Tracker, Size, Seeds, Leech, Ratio, Date }

#[derive(Clone, PartialEq)]
pub(crate) enum SortDir { Asc, Desc }

#[derive(Clone, PartialEq)]
pub(crate) enum Tab { Search, Favorites, Rss, About }

/// Results-table columns (order is user-configurable via `Config::col_order`).
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum TableCol {
    Name, Tracker, Size, Seeds, Leech, Ratio, Health, Date,
}

impl TableCol {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            TableCol::Name => "Name", TableCol::Tracker => "Tracker",
            TableCol::Size => "Size", TableCol::Seeds => "Seeds",
            TableCol::Leech => "Leech", TableCol::Ratio => "Ratio",
            TableCol::Health => "Health", TableCol::Date => "Date",
        }
    }
    pub(crate) fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            "Name" => TableCol::Name, "Tracker" => TableCol::Tracker,
            "Size" => TableCol::Size, "Seeds" => TableCol::Seeds,
            "Leech" => TableCol::Leech, "Ratio" => TableCol::Ratio,
            "Health" => TableCol::Health, "Date" => TableCol::Date,
            _ => return None,
        })
    }
    pub(crate) fn width(&self) -> f32 {
        match self {
            TableCol::Name => 295.0, TableCol::Tracker => 88.0,
            TableCol::Size => 76.0, TableCol::Seeds => 66.0,
            TableCol::Leech => 66.0, TableCol::Ratio => 58.0,
            TableCol::Health => 78.0, TableCol::Date => 88.0,
        }
    }
}

#[derive(Clone, PartialEq)]
pub(crate) enum SearchState { Idle, Searching, Done, Error(String) }

#[derive(Clone, PartialEq)]
pub(crate) enum Hlth { All, Hot, Good, Slow, Dead }

impl Hlth {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Hlth::All => "All",
            Hlth::Hot => "HOT",
            Hlth::Good => "GOOD",
            Hlth::Slow => "SLOW",
            Hlth::Dead => "DEAD",
        }
    }
    pub(crate) fn ok(&self, s: u32) -> bool {
        match self {
            Hlth::All => true,
            Hlth::Hot => s > 500,
            Hlth::Good => (101..=500).contains(&s),
            Hlth::Slow => (11..=100).contains(&s),
            Hlth::Dead => s <= 10,
        }
    }
}
// ─── Pure helpers ──────────────────────────────────────────────────────────

pub(crate) fn fmt_size(b: u64) -> String {
    let b = b as f64;
    if b >= 1_073_741_824.0 { format!("{:.2} GB", b / 1_073_741_824.0) }
    else if b >= 1_048_576.0 { format!("{:.0} MB", b / 1_048_576.0) }
    else if b >= 1_024.0 { format!("{:.0} KB", b / 1_024.0) }
    else { format!("{b} B") }
}

pub(crate) fn time_ago(s: &str) -> String {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s)
        .or_else(|_| chrono::DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%z"))
    {
        let secs = chrono::Utc::now()
            .signed_duration_since(dt.with_timezone(&chrono::Utc))
            .num_seconds().max(0);
        return if secs < 3600 { format!("{}m ago", secs / 60) }
               else if secs < 86400 { format!("{}h ago", secs / 3600) }
               else if secs < 604800 { format!("{}d ago", secs / 86400) }
               else { dt.format("%Y-%m-%d").to_string() };
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

pub(crate) fn seed_col(s: u32) -> Color32 {
    if s > 500 { rgb(34,197,94) } else if s > 100 { rgb(74,222,128) }
    else if s > 10 { rgb(245,158,11) } else if s > 0 { rgb(249,115,22) }
    else { rgb(239,68,68) }
}

pub(crate) fn hlth_lbl(s: u32) -> &'static str {
    if s > 500 {"HOT"} else if s > 100 {"GOOD"}
    else if s > 10 {"SLOW"} else if s > 0 {"DYING"} else {"DEAD"}
}

pub(crate) fn cat_col(cat: &str) -> Color32 {
    match cat.split('/').next().unwrap_or("").trim() {
        "Movies" => rgb(245,158,11), "TV" => rgb(59,130,246),
        "Music" => rgb(16,185,129), "Games" => rgb(139,92,246),
        "Software" => rgb(6,182,212), "Anime" => rgb(236,72,153),
        "Books" => rgb(249,115,22), _ => rgb(100,116,139),
    }
}

pub(crate) fn urlenc(s: &str) -> String {
    s.chars().map(|c| match c {
        'A'..='Z'|'a'..='z'|'0'..='9'|'-'|'_'|'.'|'~' => c.to_string(),
        ' ' => "+".into(),
        c => format!("%{:02X}", c as u32),
    }).collect()
}

pub(crate) fn normalize(t: &str) -> String {
    let stop = ["1080p","720p","480p","4k","bluray","bdrip","webrip",
                "x264","x265","hevc","10bit","hdr","yify","yts","rarbg",
                "mkv","mp4","avi","remux"];
    let mut s = t.to_lowercase();
    for w in &stop { s = s.replace(w, " "); }
    s.split_whitespace().take(4).collect::<Vec<_>>().join(" ")
}

pub(crate) fn now_str() -> String { chrono::Utc::now().format("%Y-%m-%d %H:%M").to_string() }

pub(crate) fn set_err(st: &Arc<Mutex<SearchState>>, msg: String) {
    if let Ok(mut s) = st.lock() { *s = SearchState::Error(msg); }
}

// ─── Search thread ─────────────────────────────────────────────────────────

/// Shared HTTP client, built once. Avoids re-handshaking per request.
pub(crate) fn shared_client() -> &'static Client {
    static CLIENT: std::sync::OnceLock<Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("failed to build HTTP client")
    })
}

/// Check GitHub releases for a newer version. Returns the latest release tag
/// (e.g. "v17.0.0") or None on failure / no newer version.
pub(crate) fn check_update(current: &str) -> Option<String> {
    let ep = "https://api.github.com/repos/chethan62/torrentx/releases/latest";
    let resp = shared_client()
        .get(ep)
        .header("User-Agent", "TorrentX")
        .timeout(Duration::from_secs(10))
        .send()
        .ok()?;
    if !resp.status().is_success() { return None; }
    let body = resp.text().ok()?;
    // Parse "tag_name":"vX.Y.Z" (char-safe: avoid byte slicing on multibyte)
    let tag = body.find("\"tag_name\":\"")?;
    let rest = &body[tag + "\"tag_name\":\"".len()..];
    let latest = rest.split('"').next()?;
    let latest_trim = latest.trim_start_matches('v');
    let cur_trim = current.trim_start_matches('v');
    // Compare dotted versions
    let parse = |s: &str| -> Vec<u64> {
        s.split('.').filter_map(|p| p.parse::<u64>().ok()).collect()
    };
    let (l, c) = (parse(latest_trim), parse(cur_trim));
    let newer = l.iter().zip(c.iter()).find(|(a, b)| a != b)
        .map(|(a, b)| a > b)
        .unwrap_or(l.len() > c.len());
    if newer { Some(latest.to_string()) } else { None }
}

/// Validate a magnet link: must start with `magnet:?xt=urn:btih:` and carry
/// a 32- or 40-char hex/base32 info-hash. Rejects empty / malformed strings.
pub(crate) fn is_magnet(s: &str) -> bool {
    let s = s.trim();
    if !s.starts_with("magnet:?xt=urn:btih:") { return false; }
    // Grab the xt=urn:btih:<hash> value (may be followed by &dn=... etc.)
    let rest = &s["magnet:?xt=urn:btih:".len()..];
    let hash = rest.split('&').next().unwrap_or("");
    let hash = hash.trim_end_matches(';');
    match hash.len() {
        40 => hash.chars().all(|c| c.is_ascii_hexdigit()),
        32 => hash.chars().all(|c| c.is_ascii_alphanumeric()), // base32
        _ => false,
    }
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
        url.trim_end_matches('/'), key
    );
    let resp = shared_client().get(&ep).timeout(Duration::from_secs(15)).send().ok()?;
    let body = resp.text().ok()?;
    // Parse `<indexer id="..." configured="true"><title>...</title>` entries
    let mut out = vec![];
    let mut pos = 0;
    while let Some(start) = body[pos..].find("<indexer ") {
        let seg = &body[pos + start..];
        // Extract id="..." — safe against attribute order / malformed XML
        if let Some(i0) = seg.find("id=\"") {
            let i0 = i0 + 4;
            if let Some(i1) = seg[i0..].find('"') {
                let id = &seg[i0..i0 + i1];
                let configured = seg.contains("configured=\"true\"");
                if configured && !id.is_empty() { out.push(id.to_string()); }
            }
        }
        pos += start + 10;
    }
    out.sort();
    out.dedup();
    Some(out)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn start_search(
    url: String, key: String, query: String, cat: String, indexer: String, timeout: u64,
    results: Arc<Mutex<Vec<TorrentResult>>>,
    state: Arc<Mutex<SearchState>>,
    count: Arc<Mutex<usize>>,
) {
    thread::spawn(move || {
        if let Ok(mut s) = state.lock() { *s = SearchState::Searching; }
        let idx = if indexer.is_empty() || indexer == "All" { "all" } else { indexer.as_str() };
        let mut ep = format!(
            "{}/api/v2.0/indexers/{}/results?apikey={}&Query={}",
            url.trim_end_matches('/'), urlenc(idx), urlenc(&key), urlenc(&query)
        );
        if cat != "All" {
            if let Some(id) = category_id(&cat) {
                ep.push_str(&format!("&Category[]={}", id));
            }
        }

        match shared_client().get(&ep).timeout(Duration::from_secs(timeout)).send() {
            Ok(resp) => {
                let st = resp.status();
                if st.is_success() {
                    match resp.json::<JackettResponse>() {
                        Ok(data) => {
                            let n = data.results.len();
                            if let Ok(mut r) = results.lock() { *r = data.results; }
                            if let Ok(mut c) = count.lock() { *c = n; }
                            if let Ok(mut s) = state.lock() { *s = SearchState::Done; }
                        }
                        Err(e) => set_err(&state, format!("Parse error: {e}")),
                    }
                } else {
                    set_err(&state, match st.as_u16() {
                        401 => "Invalid API key — open Settings to update it.".into(),
                        403 => "Forbidden — check Jackett permissions.".into(),
                        404 => "Jackett endpoint not found — verify URL in Settings.".into(),
                        500 => "Jackett internal error — check Jackett logs.".into(),
                        n => format!("HTTP {n} from Jackett"),
                    });
                }
            }
            Err(e) => set_err(&state, if e.is_connect() {
                format!("Cannot reach Jackett at {url}\nRun: sudo systemctl start jackett")
            } else if e.is_timeout() {
                format!("Timed out after {timeout}s — increase timeout in Settings")
            } else {
                format!("Network error: {e}")
            }),
        };
    });
}


#[cfg(test)]
mod tests {
    use super::{category_id, is_magnet};

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
}
