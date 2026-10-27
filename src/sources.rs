//! Native sources — search that does not need Jackett.
//!
//! Both endpoints answer JSON with everything the table draws (title, size,
//! seeders, leechers, hash), so each source is a request and a serde parse. No
//! scraping, no extra process, no second config.
//!
//! `apibay.org` is The Pirate Bay's public API; `api.knaben.org` is a meta
//! search over ~700 trackers. They are offered *next to* Jackett's indexers,
//! not instead of them, and appear in the same picker because a native source is
//! just one more id in the fan-out list.

use crate::jackett::{TorrentResult, shared_client, urlenc};
use std::time::Duration;

/// Native ids, in picker order (first is the fan-out's fallback).
pub(crate) const NATIVE: [&str; 2] = ["apibay", "knaben"];

pub(crate) fn is_native(id: &str) -> bool {
    NATIVE.iter().any(|n| n.eq_ignore_ascii_case(id))
}

/// Native ids as owned strings — the seed value for `App::net.indexers`, so the
/// picker is populated (and searching works) before Jackett is ever reached.
pub(crate) fn native_ids() -> Vec<String> {
    NATIVE.iter().map(|s| s.to_string()).collect()
}

/// Native sources first, then Jackett's configured indexers (which may repeat
/// one of ours only if the user added an indexer with the same id — deduped).
pub(crate) fn with_native(configured: &[String]) -> Vec<String> {
    let mut out = native_ids();
    for c in configured {
        if !out.iter().any(|o| o == c) {
            out.push(c.clone());
        }
    }
    out
}

/// Trackers attached to every magnet we synthesise: apibay returns only a bare
/// info hash, and a magnet with no tracker relies on DHT alone.
const TRACKERS: [&str; 6] = [
    "udp://tracker.opentrackr.org:1337/announce",
    "udp://open.stealth.si:80/announce",
    "udp://tracker.torrent.eu.org:451/announce",
    "udp://explodie.org:6969/announce",
    "udp://tracker.openbittorrent.com:80/announce",
    "udp://tracker.dler.org:6969/announce",
];

fn magnet(hash: &str, name: &str) -> String {
    let mut m = format!("magnet:?xt=urn:btih:{hash}&dn={}", urlenc(name));
    for t in TRACKERS {
        m.push_str("&tr=");
        m.push_str(&urlenc(t));
    }
    m
}

/// The picker's category label → the apibay category to *request*. This is the
/// coarse group, never a leaf id: apibay's `205` is TV shows only, so asking for
/// it made "TV" + `1080p` return nothing while 57 of the video hits were HD TV
/// shows (`208`). The group is fetched and `matches_cat` does the split.
/// Anime has no TPB category — video only — so it stays All rather than lying.
pub(crate) fn apibay_cat(cat: &str) -> &'static str {
    match cat {
        "Movies" | "TV" => "200",
        "Music" => "100",
        "PC Games" => "400",
        "Software" => "300",
        "Books" => "601",
        "XXX" => "500",
        _ => "0",
    }
}

/// Does a row's own category belong in the picker's bucket? Used where the
/// source has no category parameter (knaben publishes categories only in the
/// response, and its `categories` filter takes undocumented numeric ids).
/// The leading group is what matches: "Movies / HD" is a movie, "PC / Games" is
/// a game but "PC / Unix" is software.
pub(crate) fn matches_cat(cat: &str, desc: Option<&str>) -> bool {
    let Some(desc) = desc else {
        // No category from the source: only "All" can honestly show it.
        return cat == "All";
    };
    let d = desc.to_ascii_lowercase();
    let (group, sub) = d.split_once(" / ").unwrap_or((d.as_str(), ""));
    match cat {
        "All" => true,
        "Movies" => group == "movies",
        "TV" => group == "tv",
        "Music" => group == "audio",
        "Anime" => group == "anime",
        "Books" => group == "books",
        "XXX" => group == "xxx",
        "PC Games" => group == "pc" && sub.starts_with("game"),
        "Software" => group == "pc" && !sub.starts_with("game"),
        // An unknown label filters nothing: hiding every row would look like a
        // broken search rather than an unmapped category.
        _ => true,
    }
}

/// Query one native source. Blocking; the caller runs it on the fan-out thread.
pub(crate) fn fetch(
    id: &str,
    query: &str,
    cat: &str,
    timeout: u64,
) -> Result<Vec<TorrentResult>, String> {
    let budget = Duration::from_secs(timeout.min(30));
    match id {
        "apibay" => {
            let url = format!(
                "https://apibay.org/q.php?q={}&cat={}",
                urlenc(query),
                apibay_cat(cat)
            );
            let body = shared_client()
                .get(&url)
                .timeout(budget)
                .send()
                .map_err(|e| format!("apibay: {}", net_err(e, timeout)))
                .and_then(|r| {
                    r.error_for_status()
                        .map_err(|e| format!("apibay: {}", net_err(e, timeout)))
                })
                .and_then(|r| {
                    r.text()
                        .map_err(|e| format!("apibay: {}", net_err(e, timeout)))
                })?;
            Ok(parse_apibay(&body)
                .into_iter()
                .filter(|r| matches_cat(cat, r.category_desc.as_deref()))
                .collect())
        }
        "knaben" => {
            let url = "https://api.knaben.org/v1";
            let body = shared_client()
                .post(url)
                .timeout(budget)
                .json(&serde_json::json!({
                    "query": query,
                    "searchType": "100%",
                    "orderBy": "seeders",
                    "orderDirection": "desc",
                    "from": 0,
                    "size": 100,
                    "hideUnsafe": false,
                }))
                .send()
                .map_err(|e| format!("knaben: {}", net_err(e, timeout)))
                .and_then(|r| {
                    r.error_for_status()
                        .map_err(|e| format!("knaben: {}", net_err(e, timeout)))
                })
                .and_then(|r| {
                    r.text()
                        .map_err(|e| format!("knaben: {}", net_err(e, timeout)))
                })?;
            let rows = parse_knaben(&body)?;
            // knaben publishes each hit's category but its request filter wants
            // numeric ids it never documents, so the bucket is applied here.
            Ok(rows
                .into_iter()
                .filter(|r| matches_cat(cat, r.category_desc.as_deref()))
                .collect())
        }
        other => Err(format!("unknown source {other}")),
    }
}

/// One message per failure mode; never the URL (a query can be private).
fn net_err(e: reqwest::Error, timeout: u64) -> String {
    if e.is_timeout() {
        format!("timed out after {timeout}s")
    } else if e.is_decode() {
        "unexpected response".to_string()
    } else {
        e.without_url().to_string()
    }
}

// ─── apibay ────────────────────────────────────────────────────────────────
// Everything arrives as strings, and "no results" is a single row with id "0".

/// TPB's numeric category → the same label shape knaben uses, so one filter
/// (`matches_cat`) serves both sources. Group 200 is *all* video, so the sub-id
/// is what separates a film from an episode — without this, "Movies" on apibay
/// listed TV shows.
fn apibay_label(cat: &str) -> &'static str {
    match cat.parse::<u32>().ok() {
        Some(205) | Some(208) => "TV",
        Some(200..=299) => "Movies",
        Some(100..=199) => "Audio",
        Some(400..=499) => "PC / Games",
        Some(300..=399) => "PC",
        Some(500..=599) => "XXX",
        Some(600..=699) => "Books",
        _ => "Other",
    }
}

#[derive(serde::Deserialize)]
struct ApibayRow {
    #[serde(default)]
    category: String,
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    info_hash: String,
    #[serde(default)]
    leechers: String,
    #[serde(default)]
    seeders: String,
    #[serde(default)]
    size: String,
    #[serde(default)]
    added: String,
}

pub(crate) fn parse_apibay(body: &str) -> Vec<TorrentResult> {
    let Ok(rows) = serde_json::from_str::<Vec<ApibayRow>>(body) else {
        return vec![];
    };
    rows.into_iter()
        .filter(|r| r.id != "0" && !r.info_hash.is_empty())
        .map(|r| TorrentResult {
            title: r.name.clone(),
            tracker: Some("The Pirate Bay".into()),
            category_desc: Some(apibay_label(&r.category).to_string()),
            size: r.size.parse::<u64>().ok(),
            seeders: r.seeders.parse::<u32>().ok(),
            peers: r.leechers.parse::<u32>().ok(),
            publish_date: r
                .added
                .parse::<i64>()
                .ok()
                .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
                .map(|d| d.to_rfc2822()),
            magnet_uri: Some(magnet(&r.info_hash, &r.name)),
            link: Some(format!("https://apibay.org/t.php?id={}", r.id)),
            ..Default::default()
        })
        .collect()
}

// ─── knaben ────────────────────────────────────────────────────────────────
// A meta search: each hit names the tracker it came from and already carries a
// magnet. `seeders` is the tracker's count, `liveSeeders` knaben's own crawl.

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct KnabenHit {
    #[serde(default)]
    title: String,
    #[serde(default)]
    hash: String,
    #[serde(default)]
    seeders: Option<u32>,
    #[serde(default)]
    peers: Option<u32>,
    #[serde(default)]
    live_seeders: Option<u32>,
    #[serde(default)]
    live_peers: Option<u32>,
    #[serde(default)]
    bytes: Option<u64>,
    #[serde(default)]
    date: Option<String>,
    #[serde(default)]
    details: Option<String>,
    #[serde(default)]
    magnet_url: Option<String>,
    #[serde(default)]
    tracker: Option<String>,
    #[serde(default)]
    category: Option<String>,
}

#[derive(serde::Deserialize)]
struct KnabenResp {
    #[serde(default)]
    hits: Vec<KnabenHit>,
}

pub(crate) fn parse_knaben(body: &str) -> Result<Vec<TorrentResult>, String> {
    let resp: KnabenResp =
        serde_json::from_str(body).map_err(|e| format!("knaben: unexpected response ({e})"))?;
    Ok(resp
        .hits
        .into_iter()
        .filter(|h| !h.title.is_empty())
        .map(|h| {
            // Tracker-seeded counts first; knaben's own crawl only as a fallback,
            // so a row never claims more than its tracker reported.
            //
            // A zero from knaben is "we have no count", not "the swarm is empty":
            // its API serialises a missing tracker count as 0 (seen on nearly
            // every hit, even when asked to sort by seeders). Passing that 0 on
            // badged live torrents DEAD and hid them from every band but All, so
            // a non-positive count becomes None — unknown, which is the truth.
            let seeders = h
                .seeders
                .filter(|&s| s > 0)
                .or(h.live_seeders.filter(|&s| s > 0));
            let peers = h
                .peers
                .filter(|&p| p > 0)
                .or(h.live_peers.filter(|&p| p > 0));
            TorrentResult {
                magnet_uri: h
                    .magnet_url
                    .or_else(|| (!h.hash.is_empty()).then(|| magnet(&h.hash, &h.title))),
                title: h.title,
                tracker: Some(h.tracker.unwrap_or_else(|| "Knaben".into())),
                category_desc: h.category,
                size: h.bytes,
                seeders,
                peers,
                publish_date: h.date,
                details: h.details,
                ..Default::default()
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apibay_maps_rows_and_drops_the_no_results_sentinel() {
        // apibay answers "no results" with one row whose id is 0 — parsing that as
        // a torrent was a classic bug in clients that trusted the array length.
        let body = r#"[
          {"id":"59191690","name":"Ubuntu 22.04 LTS","info_hash":"2C6B6858D61DA9543D4231A71DB4B1C9264B0685",
           "leechers":"5","seeders":"37","size":"3654957056","num_files":"1","username":"rjaa","added":"1652877231","status":"vip","category":"303"},
          {"id":"0","name":"No results returned","info_hash":"0000000000000000000000000000000000000000",
           "leechers":"0","seeders":"0","size":"0","added":"0"}
        ]"#;
        let rows = parse_apibay(body);
        assert_eq!(rows.len(), 1, "the sentinel row must not become a result");
        let r = &rows[0];
        assert_eq!(r.title, "Ubuntu 22.04 LTS");
        assert_eq!(r.seeders, Some(37));
        assert_eq!(r.peers, Some(5));
        assert_eq!(r.size, Some(3_654_957_056));
        assert_eq!(r.tracker.as_deref(), Some("The Pirate Bay"));
        let m = r.magnet_uri.as_deref().unwrap_or("");
        assert!(m.starts_with("magnet:?xt=urn:btih:2C6B6858"), "{m}");
        assert!(m.contains("&tr=udp%3A%2F%2F"), "magnet needs trackers: {m}");
        assert!(r.publish_date.is_some(), "unix `added` must become a date");
        // TPB's numeric category becomes a label, so the table and the picker can
        // read it (it used to be dropped and the column stayed blank).
        assert_eq!(r.category_desc.as_deref(), Some("PC"));
    }

    #[test]
    fn knaben_prefers_tracker_seeders_over_its_own_crawl() {
        let body = r#"{"hits":[
          {"title":"ubuntu","hash":"9A72DA7F86D4B050E69F8B10CBC549B769A0144D","seeders":12,"peers":3,
           "liveSeeders":0,"livePeers":2,"bytes":690371952,"date":"2011-12-20T23:00:00+00:00",
           "details":"https://knaben.xyz/x","magnetUrl":"magnet:?xt=urn:btih:9A72","tracker":"The Pirate Bay",
           "category":"PC / Unix"},
          {"title":"only-live","hash":"AA","liveSeeders":7,"bytes":10},
          {"title":"knaben-has-no-count","hash":"BB","seeders":0,"peers":0,"liveSeeders":0,"livePeers":0,"bytes":10}
        ]}"#;
        let rows = parse_knaben(body).expect("valid body");
        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows[0].seeders,
            Some(12),
            "tracker count wins over liveSeeders"
        );
        assert_eq!(rows[0].peers, Some(3));
        assert_eq!(rows[0].size, Some(690_371_952));
        assert_eq!(rows[0].category_desc.as_deref(), Some("PC / Unix"));
        assert_eq!(
            rows[0].magnet_uri.as_deref(),
            Some("magnet:?xt=urn:btih:9A72")
        );
        // A hit with no tracker counts still shows the crawl's number.
        assert_eq!(rows[1].seeders, Some(7));
        // knaben's 0 means "no count", not "empty swarm": unknown, so it stays out
        // of DEAD and renders as — (the health chips treat None that way).
        assert_eq!(rows[2].seeders, None, "knaben's 0 must become unknown");
        assert_eq!(rows[2].peers, None);
        // …and one with no magnet gets one synthesised from the hash.
        assert!(
            rows[1]
                .magnet_uri
                .as_deref()
                .unwrap_or("")
                .starts_with("magnet:?xt=urn:btih:AA")
        );
    }

    #[test]
    fn the_category_picker_narrows_a_native_source() {
        // apibay takes a real category, so the request itself is narrowed.
        assert_eq!(apibay_cat("All"), "0");
        assert_eq!(apibay_cat("Movies"), "200");
        // Both video buckets request the group; `205` alone is TV shows only and
        // left HD TV shows (208) out of a "TV" search entirely.
        assert_eq!(apibay_cat("TV"), "200");
        assert_eq!(apibay_cat("Music"), "100");
        assert_eq!(apibay_cat("PC Games"), "400");
        assert_eq!(apibay_cat("Software"), "300");
        assert_eq!(apibay_cat("Books"), "601");
        assert_eq!(apibay_cat("XXX"), "500");
        // TPB has no anime category; All beats claiming a bucket it cannot fill.
        assert_eq!(apibay_cat("Anime"), "0");

        // apibay's numeric category becomes the same label shape knaben uses —
        // group 200 is all video, so 205/208 must not land in Movies.
        assert_eq!(apibay_label("201"), "Movies");
        assert_eq!(apibay_label("207"), "Movies");
        assert_eq!(apibay_label("205"), "TV");
        assert_eq!(apibay_label("208"), "TV");
        assert_eq!(apibay_label("101"), "Audio");
        assert_eq!(apibay_label("303"), "PC");
        assert_eq!(apibay_label("401"), "PC / Games");
        assert_eq!(apibay_label("501"), "XXX");
        assert_eq!(apibay_label("601"), "Books");
        assert_eq!(apibay_label("999"), "Other");
        assert_eq!(apibay_label(""), "Other");
        // …so the request-narrowed video group is split correctly by the filter.
        assert!(matches_cat("Movies", Some(apibay_label("207"))));
        assert!(!matches_cat("Movies", Some(apibay_label("205"))));
        assert!(matches_cat("TV", Some(apibay_label("208"))));

        // knaben reports its own category per hit, so the bucket is applied to
        // the parsed rows. The leading group decides, not the sub-category.
        assert!(matches_cat("Movies", Some("Movies / HD")));
        assert!(matches_cat("Movies", Some("Movies / Other")));
        assert!(!matches_cat("Movies", Some("TV / HD")));
        assert!(matches_cat("TV", Some("TV")));
        assert!(matches_cat("Music", Some("Audio / MP3")));
        assert!(matches_cat("Anime", Some("Anime / Subbed")));
        assert!(matches_cat("Books", Some("Books / EBooks")));
        assert!(matches_cat("XXX", Some("XXX / Video")));
        // "PC / Games" is a game; "PC / Unix" is software — one group, two buckets.
        assert!(matches_cat("PC Games", Some("PC / Games")));
        assert!(!matches_cat("Software", Some("PC / Games")));
        assert!(matches_cat("Software", Some("PC / Unix")));
        assert!(matches_cat("Software", Some("PC")));
        // Everything is in All; an uncategorised row is only in All.
        assert!(matches_cat("All", Some("Other / Misc")));
        assert!(matches_cat("All", None));
        assert!(!matches_cat("Movies", None));
        // An unmapped label must not empty the table.
        assert!(matches_cat("Nonsense", Some("Movies")));
    }

    #[test]
    fn native_ids_survive_the_merge_with_jackett() {
        let jackett = vec!["1337x".to_string(), "apibay".to_string()];
        let merged = with_native(&jackett);
        assert_eq!(merged, vec!["apibay", "knaben", "1337x"]);
        assert!(
            is_native("KNABEN"),
            "the picker id must match case-insensitively"
        );
        assert!(!is_native("1337x"));
    }
}
