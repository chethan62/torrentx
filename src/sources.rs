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

/// Query one native source. Blocking; the caller runs it on the fan-out thread.
pub(crate) fn fetch(id: &str, query: &str, timeout: u64) -> Result<Vec<TorrentResult>, String> {
    let budget = Duration::from_secs(timeout.min(30));
    match id {
        "apibay" => {
            let url = format!("https://apibay.org/q.php?q={}&cat=0", urlenc(query));
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
            Ok(parse_apibay(&body))
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
            parse_knaben(&body)
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

#[derive(serde::Deserialize)]
struct ApibayRow {
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
            let seeders = h.seeders.or(h.live_seeders);
            let peers = h.peers.or(h.live_peers);
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
           "leechers":"5","seeders":"37","size":"3654957056","num_files":"1","username":"rjaa","added":"1652877231","status":"vip"},
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
    }

    #[test]
    fn knaben_prefers_tracker_seeders_over_its_own_crawl() {
        let body = r#"{"hits":[
          {"title":"ubuntu","hash":"9A72DA7F86D4B050E69F8B10CBC549B769A0144D","seeders":12,"peers":3,
           "liveSeeders":0,"livePeers":2,"bytes":690371952,"date":"2011-12-20T23:00:00+00:00",
           "details":"https://knaben.xyz/x","magnetUrl":"magnet:?xt=urn:btih:9A72","tracker":"The Pirate Bay",
           "category":"PC / Unix"},
          {"title":"only-live","hash":"AA","liveSeeders":7,"bytes":10}
        ]}"#;
        let rows = parse_knaben(body).expect("valid body");
        assert_eq!(rows.len(), 2);
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
