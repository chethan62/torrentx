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

