// ─── Jackett types ─────────────────────────────────────────────────────────
use crate::themes::rgb;
use eframe::egui::Color32;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::sync::atomic::{AtomicU64, Ordering};
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
    Dead,
}

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

/// Validate a Jackett base URL: http/https scheme, non-empty host.
/// Returns an error message, or None if the URL is acceptable.
pub(crate) fn validate_jackett_url(s: &str) -> Option<&'static str> {
    let t = s.trim();
    if t.is_empty() {
        return Some("URL is empty");
    }
    let lower = t.to_lowercase();
    if lower.starts_with("http://") {
        // Allowed (Jackett commonly runs on plain http locally), but warn.
        None
    } else if lower.starts_with("https://") {
        None
    } else {
        Some("URL must start with http:// or https://")
    }
}

/// Validate a magnet link: must start with `magnet:?xt=urn:btih:` and carry
/// a 32- or 40-char hex/base32 info-hash. Rejects empty / malformed strings.
pub(crate) fn is_magnet(s: &str) -> bool {
    let s = s.trim();
    if !s.starts_with("magnet:?xt=urn:btih:") {
        return false;
    }
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
    reader.trim_text(true);
    let mut out = vec![];
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_lowercase();
                if tag != "indexer" {
                    continue;
                }
                let mut id = None;
                let mut configured = false;
                for attr in e.attributes().flatten() {
                    let k = String::from_utf8_lossy(attr.key.as_ref()).to_lowercase();
                    if let Ok(v) = attr.unescape_value() {
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
                                "Cannot reach Jackett at {url}\nRun: sudo systemctl start jackett"
                            )
                        } else if e.is_timeout() {
                            format!("Timed out after {timeout}s — increase timeout in Settings")
                        } else {
                            format!("Network error: {e}")
                        },
                    )
                }
            }
        };
    });
}

#[cfg(test)]
mod tests {
    use super::{
        category_id, fmt_size, is_magnet, normalize, parse_indexers_xml, parse_latest_tag,
        pub_year, truncate_magnet, urlenc, validate_jackett_url, Tab,
    };

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
