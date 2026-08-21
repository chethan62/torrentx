// ─── RSS types ──────────────────────────────────────────────────────────────
use crate::jackett::urlenc;
use serde::{Deserialize, Serialize};
use std::thread;
use std::time::Duration;
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub(crate) struct RssFeedConfig {
    pub(crate) name: String,
    pub(crate) indexer: String,
    pub(crate) query: String,
    pub(crate) category: String,
    pub(crate) enabled: bool,
    #[serde(default)]
    pub(crate) auto_refresh: bool,
}

impl RssFeedConfig {
    pub(crate) fn new_default() -> Self {
        Self { name: "New Feed".into(), indexer: "all".into(), query: String::new(),
               category: String::new(), enabled: true, auto_refresh: true }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RssItem {
    pub(crate) title: String,
    pub(crate) link: Option<String>,
    pub(crate) magnet: Option<String>,
    pub(crate) pub_date: Option<String>,
    pub(crate) size: Option<u64>,
    pub(crate) seeders: Option<u32>,
    pub(crate) leechers: Option<u32>,
    pub(crate) tracker: Option<String>,
    pub(crate) category: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FeedStatus { Idle, Loading, Ok, Error }

pub(crate) struct RssFeedState {
    pub(crate) config: RssFeedConfig,
    pub(crate) items: Vec<RssItem>,
    pub(crate) status: FeedStatus,
    pub(crate) error: Option<String>,
}

impl RssFeedState {
    pub(crate) fn new(config: RssFeedConfig) -> Self {
        Self { config, items: vec![], status: FeedStatus::Idle, error: None }
    }
}

pub(crate) fn build_rss_url(base: &str, key: &str, cfg: &RssFeedConfig) -> String {
    let indexer = if cfg.indexer.trim().is_empty() { "all" } else { cfg.indexer.trim() };
    let mut url = format!(
        "{}/api/v2.0/indexers/{}/results/torznab/api?apikey={}&t=search&q={}",
        base.trim_end_matches('/'), indexer, key, urlenc(&cfg.query),
    );
    if !cfg.category.trim().is_empty() {
        url.push_str(&format!("&cat={}", cfg.category.trim()));
    }
    url
}

// ─── RSS XML parser ────────────────────────────────────────────────────────

pub(crate) fn parse_torznab_xml(xml: &str) -> Result<Vec<RssItem>, String> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut items: Vec<RssItem> = vec![];
    let mut cur: Option<RssItem> = None;
    let mut buf = Vec::new();
    let mut in_item = false;
    let mut cur_tag = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag = tag_name(e.name().as_ref());
                if tag == "item" { in_item = true; cur = Some(RssItem::default()); }
                else if in_item {
                    if tag == "enclosure" {
                        for attr in e.attributes().flatten() {
                            let k = tag_name(attr.key.as_ref());
                            if k == "url" {
                                if let Ok(v) = attr.unescape_value() {
                                    if let Some(ref mut item) = cur { if item.link.is_none() { item.link = Some(v.to_string()); } }
                                }
                            }
                        }
                    }
                    cur_tag = tag;
                }
            }
            Ok(Event::Empty(ref e))
                if in_item => {
                    let tag = tag_name(e.name().as_ref());
                    match tag.as_str() {
                        "enclosure" => {
                            for attr in e.attributes().flatten() {
                                let k = tag_name(attr.key.as_ref());
                                if k == "url" {
                                    if let Ok(v) = attr.unescape_value() {
                                        if let Some(ref mut item) = cur { if item.link.is_none() { item.link = Some(v.to_string()); } }
                                    }
                                } else if k == "length" {
                                    if let Ok(v) = attr.unescape_value() {
                                        if let Some(ref mut item) = cur { if item.size.is_none() { item.size = v.parse().ok(); } }
                                    }
                                }
                            }
                        }
                        t if t.contains(":attr") || t == "attr" => {
                            let mut name = String::new(); let mut val = String::new();
                            for attr in e.attributes().flatten() {
                                let k = tag_name(attr.key.as_ref());
                                if let Ok(v) = attr.unescape_value() {
                                    match k.as_str() { "name" => name = v.to_string(), "value" => val = v.to_string(), _ => {} }
                                }
                            }
                            if let Some(ref mut item) = cur {
                                match name.as_str() {
                                    "seeders" => item.seeders = val.parse().ok(),
                                    "peers" | "leechers" => item.leechers = val.parse().ok(),
                                    "magneturl" => item.magnet = Some(val),
                                    "size" if item.size.is_none() => { item.size = val.parse().ok(); }
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
            Ok(Event::Text(ref e))
                if in_item => {
                    if let Ok(text) = e.unescape() {
                        let t = text.trim().to_string();
                        if !t.is_empty() {
                            if let Some(ref mut item) = cur {
                                match cur_tag.as_str() {
                                    "title" => item.title = t,
                                    "link" if item.link.is_none() => { item.link = Some(t); }
                                    "pubdate" | "pubDate" => item.pub_date = Some(t),
                                    "size" if item.size.is_none() => { item.size = t.parse().ok(); }
                                    "jackettindexer" => item.tracker = Some(t),
                                    "category" => item.category = Some(t),
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            Ok(Event::End(ref e)) => {
                let tag = tag_name(e.name().as_ref());
                if tag == "item" {
                    in_item = false; cur_tag = String::new();
                    if let Some(item) = cur.take() { if !item.title.is_empty() { items.push(item); } }
                } else if in_item { cur_tag = String::new(); }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML error at byte {}: {e}", reader.buffer_position())),
            _ => {}
        }
        buf.clear();
    }
    Ok(items)
}

pub(crate) fn tag_name(raw: &[u8]) -> String {
    std::str::from_utf8(raw).unwrap_or("").to_lowercase()
}

pub(crate) fn fetch_rss(url: &str, timeout: u64) -> Result<Vec<RssItem>, String> {
    let resp = crate::jackett::shared_client()
        .get(url)
        .timeout(Duration::from_secs(timeout))
        .send()
        .map_err(|e| {
            if e.is_connect() { "Cannot reach Jackett. Is it running?".into() }
            else if e.is_timeout() { format!("Timed out after {timeout}s") }
            else { format!("Network error: {e}") }
        })?;
    if !resp.status().is_success() { return Err(format!("HTTP {}", resp.status().as_u16())); }
    let body = resp.text().map_err(|e| format!("Read error: {e}"))?;
    parse_torznab_xml(&body)
}

pub(crate) fn start_rss_fetch(
    base_url: String, api_key: String, feed_cfg: RssFeedConfig, timeout: u64,
    feed_idx: usize,
    tx: std::sync::mpsc::Sender<(usize, Result<Vec<RssItem>, String>)>,
) {
    thread::spawn(move || {
        let url = build_rss_url(&base_url, &api_key, &feed_cfg);
        let result = fetch_rss(&url, timeout);
        let _ = tx.send((feed_idx, result));
    });
}

#[cfg(test)]
mod tests {
    use super::{build_rss_url, parse_torznab_xml, RssFeedConfig};

    const SAMPLE: &str = r#"<?xml version="1.0"?>
<rss version="2.0" xmlns:torznab="http://torznab.com/schemas/2015/feed">
  <channel>
    <title>Test Feed</title>
    <item>
      <title>Ubuntu 24.04 ISO</title>
      <link>http://tracker/download/1.torrent</link>
      <enclosure url="http://tracker/download/1.torrent" length="5242880" type="application/x-bittorrent"/>
      <category>PC</category>
      <torznab:attr name="seeders" value="42"/>
      <torznab:attr name="peers" value="10"/>
      <torznab:attr name="magneturl" value="magnet:?xt=urn:btih:abc123"/>
      <torznab:attr name="size" value="7340032"/>
      <pubDate>Wed, 21 Aug 2024 10:00:00 +0000</pubDate>
      <jackettindexer>mytracker</jackettindexer>
    </item>
  </channel>
</rss>"#;

    #[test]
    fn parses_full_item() {
        let items = parse_torznab_xml(SAMPLE).unwrap();
        assert_eq!(items.len(), 1);
        let it = &items[0];
        assert_eq!(it.title, "Ubuntu 24.04 ISO");
        assert_eq!(it.link.as_deref(), Some("http://tracker/download/1.torrent"));
        assert_eq!(it.size, Some(5_242_880)); // from enclosure length
        assert_eq!(it.seeders, Some(42));
        assert_eq!(it.leechers, Some(10));
        assert_eq!(it.magnet.as_deref(), Some("magnet:?xt=urn:btih:abc123"));
        assert_eq!(it.category.as_deref(), Some("PC"));
        assert_eq!(it.tracker.as_deref(), Some("mytracker"));
        assert!(it.pub_date.is_some());
    }

    #[test]
    fn parses_text_fallback_for_title_and_category() {
        let xml = r#"<rss><channel><item>
            <title>Fallback Title</title>
            <category>TV</category>
        </item></channel></rss>"#;
        let items = parse_torznab_xml(xml).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Fallback Title");
        assert_eq!(items[0].category.as_deref(), Some("TV"));
    }

    #[test]
    fn ignores_items_without_title() {
        let xml = r#"<rss><channel>
            <item><link>http://x/1</link></item>
            <item><title>Real</title></item>
        </channel></rss>"#;
        let items = parse_torznab_xml(xml).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Real");
    }

    #[test]
    fn truncated_xml_returns_empty_not_error() {
        // Quick-xml streams; an unclosed doc parses as "no complete items",
        // which the feed UI treats as an empty feed — not a hard error.
        assert!(parse_torznab_xml("<rss><channel><item>").unwrap().is_empty());
        assert!(parse_torznab_xml("").unwrap().is_empty());
    }

    #[test]
    fn build_url_uses_all_indexer_and_category() {
        let cfg = RssFeedConfig {
            indexer: "mytracker".into(),
            query: "ubuntu iso".into(),
            category: "2000".into(),
            ..RssFeedConfig::new_default()
        };
        let url = build_rss_url("http://localhost:9117/", "KEY123", &cfg);
        assert!(url.contains("/indexers/mytracker/results/torznab/api"));
        assert!(url.contains("apikey=KEY123"));
        assert!(url.contains("q=ubuntu+iso"));
        assert!(url.contains("&cat=2000"));
    }

    #[test]
    fn build_url_defaults_to_all_indexer_and_no_cat() {
        let cfg = RssFeedConfig::new_default();
        let url = build_rss_url("http://localhost:9117", "K", &cfg);
        assert!(url.contains("/indexers/all/results/torznab/api"));
        assert!(!url.contains("&cat="));
    }
}

