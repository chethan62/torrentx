// ─── RSS types ──────────────────────────────────────────────────────────────
use crate::jackett::{dedupe_key, urlenc};
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
        Self {
            name: "New Feed".into(),
            indexer: "all".into(),
            query: String::new(),
            category: String::new(),
            enabled: true,
            auto_refresh: true,
        }
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
    /// Feed-level `<guid>` — the stable identity Torznab intends for dedupe
    /// (falls back to normalized title when absent).
    pub(crate) guid: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FeedStatus {
    Idle,
    Loading,
    Ok,
    Error,
}

/// How long to wait before re-fetching this feed.
///
/// A failing indexer used to be retried on every tick forever (and at a short
/// interval the status pill oscillated Loading→Error). Failures now double the
/// wait, capped at 30 minutes, so a dead feed costs a handful of requests a day
/// instead of one per interval. Success resets the counter.
pub(crate) fn refresh_delay(interval: std::time::Duration, failures: u32) -> std::time::Duration {
    let factor = 1u32 << failures.min(3); // ×1, ×2, ×4, ×8
    interval
        .saturating_mul(factor)
        .min(std::time::Duration::from_secs(1800))
}

pub(crate) struct RssFeedState {
    pub(crate) config: RssFeedConfig,
    pub(crate) items: Vec<RssItem>,
    pub(crate) status: FeedStatus,
    pub(crate) error: Option<String>,
    /// Consecutive failed fetches — drives the backoff in `refresh_delay`.
    pub(crate) failures: u32,
}

impl RssFeedState {
    /// Fold a fetch result into the feed: success clears the error and the
    /// backoff, failure records both.
    pub(crate) fn apply_result(&mut self, res: Result<Vec<RssItem>, String>) {
        match res {
            Ok(items) => {
                self.items = dedupe_items(items);
                self.status = FeedStatus::Ok;
                self.error = None;
                self.failures = 0;
            }
            Err(e) => {
                self.status = FeedStatus::Error;
                self.error = Some(e);
                self.failures = self.failures.saturating_add(1);
            }
        }
    }

    pub(crate) fn new(config: RssFeedConfig) -> Self {
        Self {
            config,
            items: vec![],
            status: FeedStatus::Idle,
            error: None,
            failures: 0,
        }
    }
}

pub(crate) fn build_rss_url(base: &str, key: &str, cfg: &RssFeedConfig) -> String {
    let indexer = if cfg.indexer.trim().is_empty() {
        "all"
    } else {
        cfg.indexer.trim()
    };
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

// ─── RSS XML parser ────────────────────────────────────────────────────────

/// Reject bodies that are not a Torznab/RSS feed before parsing them.
///
/// `quick_xml` is a pull parser: it happily reads an HTML login page or a
/// truncated document to EOF and yields zero items, so a proxy error page used to
/// look like a healthy feed with "0 items" (green pill, no error) — the user had
/// no way to tell "nothing new" from "your API key is wrong".
fn check_feed_body(body: &str) -> Result<(), String> {
    let head = body.trim_start();
    if !head.starts_with('<') || !(body.contains("<rss") || body.contains("<channel")) {
        return Err(
            "Jackett did not return an RSS feed — check the URL and API key in Settings".into(),
        );
    }
    // Feed-shaped but not well-formed: an <item> we never closed (truncated
    // response, cut-off connection) parses as zero items instead of failing.
    if body.contains("<item") && !body.contains("</item>") {
        return Err("RSS feed is malformed (truncated item) — try Refresh".into());
    }
    Ok(())
}

pub(crate) fn parse_torznab_xml(xml: &str) -> Result<Vec<RssItem>, String> {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    let mut reader = Reader::from_str(xml);
    // NOTE: no trim_text here — it would trim each Text chunk independently
    // and eat the spaces around entity splits ("&amp;" boundary); the
    // accumulator below is trimmed once per element instead.
    let mut items: Vec<RssItem> = vec![];
    let mut cur: Option<RssItem> = None;
    let mut buf = Vec::new();
    let mut in_item = false;
    let mut cur_tag = String::new();
    // Text accumulator — newer quick-xml splits Text events around entity
    // boundaries ("…&amp; More" → 2+ events), so fields must be assembled
    // across chunks and applied on the element's End event.
    let mut text_acc = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag = tag_name(e.name().as_ref());
                if tag == "item" {
                    in_item = true;
                    cur = Some(RssItem::default());
                } else if in_item {
                    if tag == "enclosure" {
                        for attr in e.attributes().flatten() {
                            let k = tag_name(attr.key.as_ref());
                            if k == "url"
                                && let Ok(v) =
                                    attr.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                && let Some(ref mut item) = cur
                                && item.link.is_none()
                            {
                                item.link = Some(v.to_string());
                            }
                        }
                    }
                    cur_tag = tag;
                    text_acc.clear();
                }
            }
            Ok(Event::Empty(ref e)) if in_item => {
                let tag = tag_name(e.name().as_ref());
                match tag.as_str() {
                    "enclosure" => {
                        for attr in e.attributes().flatten() {
                            let k = tag_name(attr.key.as_ref());
                            if k == "url" {
                                if let Ok(v) =
                                    attr.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                    && let Some(ref mut item) = cur
                                    && item.link.is_none()
                                {
                                    item.link = Some(v.to_string());
                                }
                            } else if k == "length"
                                && let Ok(v) =
                                    attr.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                && let Some(ref mut item) = cur
                                && item.size.is_none()
                            {
                                item.size = v.parse().ok();
                            }
                        }
                    }
                    t if t.contains(":attr") || t == "attr" => {
                        let mut name = String::new();
                        let mut val = String::new();
                        for attr in e.attributes().flatten() {
                            let k = tag_name(attr.key.as_ref());
                            if let Ok(v) = attr.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                            {
                                match k.as_str() {
                                    "name" => name = v.to_string(),
                                    "value" => val = v.to_string(),
                                    _ => {}
                                }
                            }
                        }
                        if let Some(ref mut item) = cur {
                            match name.as_str() {
                                "seeders" => item.seeders = val.parse().ok(),
                                "peers" | "leechers" => item.leechers = val.parse().ok(),
                                "magneturl" => item.magnet = Some(val),
                                "size" if item.size.is_none() => {
                                    item.size = val.parse().ok();
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) if in_item => {
                // quick-xml ≥0.37 splits Text events around entity boundaries;
                // chunks are already unescaped by the reader, so just append.
                // 0.42 replaced `decode()` (fallible, byte-based) with the
                // infallible str-based `xml10_content()` (EOL normalization only).
                text_acc.push_str(&e.xml10_content());
            }
            Ok(Event::CData(ref e)) if in_item => {
                // CDATA is its own event, not Text. Trackers that wrap titles in
                // `<![CDATA[…]]>` had every item dropped as "no title".
                text_acc.push_str(&e.clone().into_inner());
            }
            Ok(Event::GeneralRef(ref e)) if in_item => {
                // `&name;` / `&#nnn;` arrive as their own event. Resolve via
                // the escape-html feature (full HTML5 set, so &ldquo; etc.
                // render); unknown refs stay literal instead of vanishing.
                // 0.42: BytesRef content is str-based and infallible (was
                // fallible byte-based `decode()` in ≤0.41).
                let name = e.xml10_content();
                let replacement: Option<String> = if let Some(num) =
                    name.strip_prefix("#x").or_else(|| name.strip_prefix("#X"))
                {
                    u32::from_str_radix(num, 16)
                        .ok()
                        .and_then(char::from_u32)
                        .map(String::from)
                } else if let Some(num) = name.strip_prefix('#') {
                    num.parse::<u32>()
                        .ok()
                        .and_then(char::from_u32)
                        .map(String::from)
                } else {
                    quick_xml::escape::resolve_predefined_entity(&name)
                        .map(str::to_string)
                        .or_else(|| Some(format!("&{name};")))
                };
                if let Some(s) = replacement {
                    text_acc.push_str(&s);
                }
            }
            Ok(Event::End(ref e)) => {
                let tag = tag_name(e.name().as_ref());
                if tag == "item" {
                    in_item = false;
                    cur_tag = String::new();
                    if let Some(item) = cur.take()
                        && !item.title.is_empty()
                    {
                        items.push(item);
                    }
                } else if in_item {
                    // Apply the accumulated text of the element that just closed.
                    let t = text_acc.trim().to_string();
                    if !t.is_empty()
                        && let Some(ref mut item) = cur
                    {
                        match cur_tag.as_str() {
                            "title" => item.title = t,
                            "link" if item.link.is_none() => {
                                item.link = Some(t);
                            }
                            "pubdate" | "pubDate" => item.pub_date = Some(t),
                            "size" if item.size.is_none() => {
                                item.size = t.parse().ok();
                            }
                            "jackettindexer" => item.tracker = Some(t),
                            "guid" if item.guid.is_none() => item.guid = Some(t),
                            "category" => item.category = Some(t),
                            _ => {}
                        }
                    }
                    cur_tag = String::new();
                    text_acc.clear();
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(format!(
                    "XML error at byte {}: {e}",
                    reader.buffer_position()
                ));
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(items)
}

/// Keep one copy of each release — the best-seeded one — in feed order.
///
/// Identity is the feed's `<guid>` when it has one, else the same
/// content-marker key the search results use. Keying on `normalize` alone (the
/// first four words) merged different episodes and different films that share
/// an opening, so the RSS list lost rows the search list kept.
pub(crate) fn dedupe_items(items: Vec<RssItem>) -> Vec<RssItem> {
    let mut best: std::collections::HashMap<String, usize> = Default::default();
    for (i, it) in items.iter().enumerate() {
        let key = it.guid.clone().unwrap_or_else(|| dedupe_key(&it.title));
        match best.get(&key) {
            Some(&j) if items[j].seeders.unwrap_or(0) >= it.seeders.unwrap_or(0) => {}
            _ => {
                best.insert(key, i);
            }
        }
    }
    let mut keep: Vec<usize> = best.into_values().collect();
    keep.sort_unstable();
    keep.into_iter().map(|i| items[i].clone()).collect()
}

pub(crate) fn tag_name(raw: &str) -> String {
    raw.to_lowercase()
}

pub(crate) fn fetch_rss(url: &str, timeout: u64) -> Result<Vec<RssItem>, String> {
    let resp = crate::jackett::shared_client()
        .get(url)
        .timeout(Duration::from_secs(timeout))
        .send()
        .map_err(|e| {
            if e.is_connect() {
                "Cannot reach Jackett. Is it running?".into()
            } else if e.is_timeout() {
                format!("Timed out after {timeout}s")
            } else {
                // Never surface the URL: it carries the API key.
                format!("Network error: {}", e.without_url())
            }
        })?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }
    let body = resp
        .text()
        .map_err(|e| format!("Read error: {}", e.without_url()))?;
    check_feed_body(&body)?;
    parse_torznab_xml(&body)
}

pub(crate) fn start_rss_fetch(
    base_url: String,
    api_key: String,
    feed_cfg: RssFeedConfig,
    timeout: u64,
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
    use super::{RssFeedConfig, build_rss_url, parse_torznab_xml};

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
      <guid isPermaLink="false">urn:btih:abc123</guid>
    </item>
  </channel>
</rss>"#;

    /// A dead feed must back off instead of being retried on every tick.
    #[test]
    fn failures_double_the_delay_and_success_resets_it() {
        use super::{FeedStatus, RssFeedConfig, RssFeedState, refresh_delay};
        use std::time::Duration;

        let base = Duration::from_secs(60);
        assert_eq!(refresh_delay(base, 0), base);
        assert_eq!(refresh_delay(base, 1), Duration::from_secs(120));
        assert_eq!(refresh_delay(base, 2), Duration::from_secs(240));
        assert_eq!(refresh_delay(base, 3), Duration::from_secs(480));
        // The exponent stops growing (×8 max) …
        assert_eq!(refresh_delay(base, 9), Duration::from_secs(480));
        // … and the absolute cap holds even for a long base interval.
        assert_eq!(
            refresh_delay(Duration::from_secs(900), 3),
            Duration::from_secs(1800)
        );

        let mut f = RssFeedState::new(RssFeedConfig::new_default());
        f.apply_result(Err("HTTP 500".into()));
        f.apply_result(Err("HTTP 500".into()));
        assert_eq!(f.failures, 2);
        assert_eq!(f.status, FeedStatus::Error);
        f.apply_result(Ok(vec![]));
        assert_eq!(f.failures, 0, "a success clears the backoff");
        assert_eq!(f.status, FeedStatus::Ok);
        assert!(f.error.is_none());
    }

    /// A feed that failed to fetch must not look like a healthy empty feed.
    /// quick_xml reads an HTML login page to EOF and returns zero items, so the
    /// shape check is what separates "nothing new" from "your key is wrong".
    #[test]
    fn only_feed_shaped_bodies_are_accepted() {
        use super::check_feed_body;

        // A proxy / login wall.
        assert!(check_feed_body("<html><body>login</body></html>").is_err());
        assert!(check_feed_body("{\"error\":\"upstream\"}").is_err());
        // Truncated mid-item (connection cut).
        assert!(check_feed_body("<rss><channel><item><title>Half").is_err());
        // A genuinely empty feed is NOT an error.
        assert!(
            check_feed_body("<rss version=\"2.0\"><channel><title>x</title></channel></rss>")
                .is_ok()
        );
        // And a normal feed passes.
        assert!(
            check_feed_body("<rss><channel><item><title>t</title></item></channel></rss>").is_ok()
        );
    }

    #[test]
    fn parses_full_item() {
        let items = parse_torznab_xml(SAMPLE).unwrap();
        assert_eq!(items.len(), 1);
        let it = &items[0];
        assert_eq!(it.title, "Ubuntu 24.04 ISO");
        assert_eq!(
            it.link.as_deref(),
            Some("http://tracker/download/1.torrent")
        );
        assert_eq!(it.size, Some(5_242_880)); // from enclosure length
        assert_eq!(it.seeders, Some(42));
        assert_eq!(it.leechers, Some(10));
        assert_eq!(it.magnet.as_deref(), Some("magnet:?xt=urn:btih:abc123"));
        assert_eq!(it.category.as_deref(), Some("PC"));
        assert_eq!(it.tracker.as_deref(), Some("mytracker"));
        assert!(it.pub_date.is_some());
        assert_eq!(it.guid.as_deref(), Some("urn:btih:abc123"));
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
    fn parses_cdata_title_and_link() {
        // Trackers commonly wrap the title in CDATA. quick-xml emits that as its
        // own `Event::CData`, not `Event::Text`, so a parser that only reads Text
        // dropped the whole item ("ignores items without title").
        let xml = r#"<rss><channel><item>
            <title><![CDATA[Dune 2024 1080p & More]]></title>
            <guid>x1</guid>
        </item></channel></rss>"#;
        let items = parse_torznab_xml(xml).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Dune 2024 1080p & More");
    }

    #[test]
    fn dedupe_keeps_episodes_apart_and_best_seeded_copy() {
        use super::{RssItem, dedupe_items};
        let it = |t: &str, s: u32, g: Option<&str>| RssItem {
            title: t.into(),
            seeders: Some(s),
            guid: g.map(String::from),
            ..RssItem::default()
        };
        // No guid: two encodings of the same episode collapse to the better
        // seeded copy, while the neighbouring episode stays a separate row.
        let out = dedupe_items(vec![
            it("Show S02E07 1080p WEBRip", 5, None),
            it("Show S02E08 1080p WEBRip", 9, None),
            it("Show S02E07 2160p BluRay", 50, None),
        ]);
        assert_eq!(
            out.len(),
            2,
            "{:?}",
            out.iter().map(|i| &i.title).collect::<Vec<_>>()
        );
        assert!(out.iter().any(|i| i.title == "Show S02E08 1080p WEBRip"));
        assert!(out.iter().any(|i| i.title == "Show S02E07 2160p BluRay"));
        // Two different films whose first four words match: the old
        // `normalize` key merged them; content markers keep them apart.
        let out = dedupe_items(vec![
            it(
                "One Piece Episode of East Blue (2017) 1080p BRRip x264",
                12,
                None,
            ),
            it("One Piece Episode of Sabo (2015) 1080p BRRip x264", 4, None),
        ]);
        assert_eq!(
            out.len(),
            2,
            "{:?}",
            out.iter().map(|i| &i.title).collect::<Vec<_>>()
        );
        // Same guid → one copy, the better seeded one.
        let out = dedupe_items(vec![
            it("Same release", 1, Some("g1")),
            it("Same release", 7, Some("g1")),
        ]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].seeders, Some(7));
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
        assert!(
            parse_torznab_xml("<rss><channel><item>")
                .unwrap()
                .is_empty()
        );
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

    #[test]
    fn html_named_entities_in_title_are_decoded() {
        // Torznab titles carry HTML entities (&ldquo; &rdquo; &amp; &nbsp;).
        // quick-xml 0.41 emits them as separate GeneralRef events; the
        // escape-html feature resolves the full HTML5 named-entity set.
        // Regression: literal "&ldquo;Sylvia&rdquo;" was rendered.
        let xml = r#"<rss><channel><item>
            <title>Linux Mint 18.3 &ldquo;Sylvia&rdquo; KDE (64-bit) &amp; More</title>
        </item></channel></rss>"#;
        let items = parse_torznab_xml(xml).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].title,
            "Linux Mint 18.3 \u{201C}Sylvia\u{201D} KDE (64-bit) & More"
        );
    }

    #[test]
    fn numeric_refs_resolve_and_unknown_entities_stay_literal() {
        let xml = r#"<rss><channel><item>
            <title>A &#65;&#x42;C &#160;D &notanentity; E</title>
        </item></channel></rss>"#;
        let items = parse_torznab_xml(xml).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "A ABC \u{a0}D &notanentity; E");
    }

    #[test]
    fn double_escaped_entity_decodes_exactly_one_level() {
        // Captured from a live Jackett feed: a tracker double-escaped its own
        // ampersand, so the wire format carried `&amp;amp;`. One decode yields
        // the literal text "&amp;", and entities must NOT be resolved
        // recursively — a second pass would corrupt titles that legitimately
        // contain entity-looking text.
        let xml = r#"<rss><channel><item>
            <title>Linux Command Line (Hands-On) Ubuntu Linux &amp;amp; Linux Scripting</title>
        </item></channel></rss>"#;
        let items = parse_torznab_xml(xml).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].title,
            "Linux Command Line (Hands-On) Ubuntu Linux &amp; Linux Scripting"
        );
    }

    #[test]
    fn entity_followed_by_text_keeps_both() {
        // Also from a live feed, where the tracker truncated its own title
        // mid-field: `+5&amp;a` decodes to "+5&a". The dangling text after the
        // reference must survive instead of being dropped or merged wrongly.
        let xml = r#"<rss><channel><item>
            <title>The Official Ubuntu Book (7th Edition) 2012 - Prentice Hall +5&amp;a</title>
        </item></channel></rss>"#;
        let items = parse_torznab_xml(xml).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].title,
            "The Official Ubuntu Book (7th Edition) 2012 - Prentice Hall +5&a"
        );
    }
}
