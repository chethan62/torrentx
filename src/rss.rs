use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::utils::urlenc;

// ─── Config (serialised to disk) ──────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct RssFeedConfig {
    pub name:         String,
    pub indexer:      String,   // "all" or a specific indexer slug
    pub query:        String,   // empty = latest torrents
    pub category:     String,   // Torznab category numbers, e.g. "2000,2010"
    pub enabled:      bool,
    pub auto_refresh: bool,
}

impl RssFeedConfig {
    pub fn new_default() -> Self {
        Self {
            name:         "New Feed".into(),
            indexer:      "all".into(),
            query:        String::new(),
            category:     String::new(),
            enabled:      true,
            auto_refresh: true,
        }
    }
}

// ─── Item (runtime only) ──────────────────────────────────────────────────

#[derive(Clone, Debug, Default)]
pub struct RssItem {
    pub title:    String,
    pub link:     Option<String>,
    pub magnet:   Option<String>,
    pub pub_date: Option<String>,
    pub size:     Option<u64>,
    pub seeders:  Option<u32>,
    pub leechers: Option<u32>,
    pub tracker:  Option<String>,
    pub category: Option<String>,
    pub guid:     Option<String>,
}

// ─── Feed state (runtime only) ────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum FeedStatus { Idle, Loading, Ok, Error }

pub struct RssFeedState {
    pub config:       RssFeedConfig,
    pub items:        Vec<RssItem>,
    pub status:       FeedStatus,
    pub last_fetched: Option<Instant>,
    pub error:        Option<String>,
}

impl RssFeedState {
    pub fn new(config: RssFeedConfig) -> Self {
        Self { config, items: vec![], status: FeedStatus::Idle, last_fetched: None, error: None }
    }
    pub fn status_icon(&self) -> &'static str {
        match self.status {
            FeedStatus::Idle    => "○",
            FeedStatus::Loading => "⟳",
            FeedStatus::Ok      => "●",
            FeedStatus::Error   => "✕",
        }
    }
}

// ─── URL builder ─────────────────────────────────────────────────────────

pub fn build_rss_url(base: &str, key: &str, cfg: &RssFeedConfig) -> String {
    let indexer = if cfg.indexer.trim().is_empty() { "all" } else { cfg.indexer.trim() };
    let mut url = format!(
        "{}/api/v2.0/indexers/{}/results/torznab/api?apikey={}&t=search&q={}",
        base.trim_end_matches('/'),
        indexer,
        key,
        urlenc(&cfg.query),
    );
    if !cfg.category.trim().is_empty() {
        url.push_str(&format!("&cat={}", cfg.category.trim()));
    }
    url
}

// ─── Async fetch ─────────────────────────────────────────────────────────

pub fn start_rss_fetch(
    base_url: String,
    api_key:  String,
    feed_cfg: RssFeedConfig,
    timeout:  u64,
    feed_idx: usize,
    tx:       mpsc::SyncSender<(usize, Result<Vec<RssItem>, String>)>,
) {
    thread::spawn(move || {
        let url = build_rss_url(&base_url, &api_key, &feed_cfg);
        let result = fetch_and_parse(&url, timeout);
        let _ = tx.send((feed_idx, result));
    });
}

fn fetch_and_parse(url: &str, timeout: u64) -> Result<Vec<RssItem>, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(timeout))
        .build()
        .map_err(|e| format!("Client error: {e}"))?;

    let resp = client.get(url).send()
        .map_err(|e| if e.is_connect() {
            "Cannot reach Jackett. Is it running?".into()
        } else if e.is_timeout() {
            format!("Timed out after {timeout}s")
        } else {
            format!("Network error: {e}")
        })?;

    let status = resp.status();
    if !status.is_success() {
        return Err(format!("HTTP {}", status.as_u16()));
    }

    let body = resp.text().map_err(|e| format!("Read error: {e}"))?;
    parse_torznab_xml(&body)
}

// ─── Torznab XML parser ──────────────────────────────────────────────────
//
// Handles the RSS 2.0 + torznab namespace format that Jackett emits.
// Key fields:
//   <title>, <link>, <pubDate>, <size>, <guid>, <jackettindexer>
//   <enclosure url="..." />
//   <torznab:attr name="seeders"   value="N"/>
//   <torznab:attr name="peers"     value="N"/>
//   <torznab:attr name="magneturl" value="magnet:?..."/>
//   <torznab:attr name="size"      value="N"/>

pub fn parse_torznab_xml(xml: &str) -> Result<Vec<RssItem>, String> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);

    let mut items: Vec<RssItem>  = vec![];
    let mut cur: Option<RssItem> = None;
    let mut buf      = Vec::new();
    let mut in_item  = false;
    let mut cur_tag  = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag = tag_name(e.name().as_ref());
                if tag == "item" {
                    in_item = true;
                    cur     = Some(RssItem::default());
                } else if in_item {
                    // capture attribute on start elements too (e.g. <enclosure url="...">)
                    if tag == "enclosure" {
                        for attr in e.attributes().flatten() {
                            let k = tag_name(attr.key.as_ref());
                            if k == "url" {
                                if let Ok(v) = attr.unescape_value() {
                                    if let Some(ref mut item) = cur {
                                        if item.link.is_none() { item.link = Some(v.to_string()); }
                                    }
                                }
                            }
                        }
                    }
                    cur_tag = tag;
                }
            }
            Ok(Event::Empty(ref e)) => {
                if in_item {
                    let tag = tag_name(e.name().as_ref());
                    match tag.as_str() {
                        "enclosure" => {
                            for attr in e.attributes().flatten() {
                                let k = tag_name(attr.key.as_ref());
                                if k == "url" {
                                    if let Ok(v) = attr.unescape_value() {
                                        if let Some(ref mut item) = cur {
                                            if item.link.is_none() {
                                                item.link = Some(v.to_string());
                                            }
                                        }
                                    }
                                } else if k == "length" {
                                    if let Ok(v) = attr.unescape_value() {
                                        if let Some(ref mut item) = cur {
                                            if item.size.is_none() {
                                                item.size = v.parse().ok();
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        t if t.ends_with(":attr") || t == "torznab:attr" || t == "attr" => {
                            let mut name = String::new();
                            let mut val  = String::new();
                            for attr in e.attributes().flatten() {
                                let k = tag_name(attr.key.as_ref());
                                if let Ok(v) = attr.unescape_value() {
                                    match k.as_str() {
                                        "name"  => name = v.to_string(),
                                        "value" => val  = v.to_string(),
                                        _ => {}
                                    }
                                }
                            }
                            if let Some(ref mut item) = cur {
                                match name.as_str() {
                                    "seeders"   => item.seeders  = val.parse().ok(),
                                    "peers" | "leechers" => item.leechers = val.parse().ok(),
                                    "magneturl" => item.magnet   = Some(val),
                                    "size"      => { if item.size.is_none() { item.size = val.parse().ok(); } }
                                    _           => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_item {
                    if let Ok(text) = e.unescape() {
                        let t = text.trim().to_string();
                        if !t.is_empty() {
                            if let Some(ref mut item) = cur {
                                match cur_tag.as_str() {
                                    "title"           => item.title    = t,
                                    "link"            => { if item.link.is_none() { item.link = Some(t); } }
                                    "pubdate" | "pubDate" => item.pub_date  = Some(t),
                                    "size"            => { if item.size.is_none() { item.size = t.parse().ok(); } }
                                    "guid"            => item.guid     = Some(t),
                                    "jackettindexer"  => item.tracker  = Some(t),
                                    "category"        => item.category = Some(t),
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let tag = tag_name(e.name().as_ref());
                if tag == "item" {
                    in_item   = false;
                    cur_tag   = String::new();
                    if let Some(item) = cur.take() {
                        if !item.title.is_empty() { items.push(item); }
                    }
                } else if in_item {
                    cur_tag = String::new();
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML error at byte {}: {e}", reader.buffer_position())),
            _ => {}
        }
        buf.clear();
    }

    Ok(items)
}

fn tag_name(raw: &[u8]) -> String {
    // Strip namespace prefix: "torznab:attr" → "attr" (but we match full name too)
    let s = std::str::from_utf8(raw).unwrap_or("").to_lowercase();
    s
}
