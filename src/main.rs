#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(dead_code)]

use eframe::egui::{self, Color32, FontId, RichText, Stroke, Vec2, Visuals};
use egui_extras::{Column, TableBuilder};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// ─── Constants ─────────────────────────────────────────────────────────────

const MARGIN_DEFAULT: f32 = 12.0;
const ROW_HEIGHT_COMPACT: f32 = 32.0;
const ROW_HEIGHT_NORMAL: f32 = 44.0;
const ROW_HEIGHT_ROOMY: f32 = 56.0;

const COL_NAME_WIDTH: f32 = 295.0;
const COL_TRACKER_WIDTH: f32 = 88.0;
const COL_SIZE_WIDTH: f32 = 76.0;
const COL_SEEDS_WIDTH: f32 = 66.0;
const COL_LEECH_WIDTH: f32 = 66.0;
const COL_RATIO_WIDTH: f32 = 58.0;
const COL_HEALTH_WIDTH: f32 = 78.0;
const COL_DATE_WIDTH: f32 = 88.0;

const CATS: &[&str] = &["All", "Movies", "TV", "Music", "PC Games", "Software", "Anime", "Books", "XXX"];
const SPIN: &[&str] = &["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"];

// ─── Jackett types ─────────────────────────────────────────────────────────

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "PascalCase")]
struct JackettResponse {
    #[serde(default)]
    results: Vec<TorrentResult>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
struct TorrentResult {
    #[serde(default)]
    title: String,
    tracker: Option<String>,
    category_desc: Option<String>,
    size: Option<u64>,
    seeders: Option<u32>,
    peers: Option<u32>,
    publish_date: Option<String>,
    magnet_uri: Option<String>,
    link: Option<String>,
    details: Option<String>,
}

// ─── Themes ────────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
enum Theme {
    // Dark
    TokyoNight, Cyberpunk, Midnight, OneDark, CatppuccinMocha,
    Dracula, RosePine, Monokai, Kanagawa, Everforest,
    MaterialOcean, Oxocarbon, Ayu, Nord, Gruvbox, SolarizedDark,
    // Light
    Light, GruvboxLight, CatppuccinLatte,
}

impl Theme {
    fn all() -> &'static [Theme] {
        use Theme::*;
        &[TokyoNight, Cyberpunk, Midnight, OneDark, CatppuccinMocha,
          Dracula, RosePine, Monokai, Kanagawa, Everforest,
          MaterialOcean, Oxocarbon, Ayu, Nord, Gruvbox, SolarizedDark,
          Light, GruvboxLight, CatppuccinLatte]
    }

    fn name(&self) -> &'static str {
        use Theme::*;
        match self {
            TokyoNight => "Tokyo Night",
            Cyberpunk => "Cyberpunk",
            Midnight => "Midnight",
            OneDark => "One Dark",
            CatppuccinMocha => "Catppuccin Mocha",
            Dracula => "Dracula",
            RosePine => "Rose Pine",
            Monokai => "Monokai",
            Kanagawa => "Kanagawa",
            Everforest => "Everforest",
            MaterialOcean => "Material Ocean",
            Oxocarbon => "Oxocarbon",
            Ayu => "Ayu Dark",
            Nord => "Nord",
            Gruvbox => "Gruvbox",
            SolarizedDark => "Solarized Dark",
            Light => "Light",
            GruvboxLight => "Gruvbox Light",
            CatppuccinLatte => "Catppuccin Latte",
        }
    }

    fn is_light(&self) -> bool {
        matches!(self, Theme::Light | Theme::GruvboxLight | Theme::CatppuccinLatte)
    }

    fn accent_color(&self) -> Color32 {
        use Theme::*;
        match self {
            TokyoNight => rgb(122, 162, 247),
            Cyberpunk => rgb(6, 182, 212),
            Midnight => rgb(192, 132, 252),
            OneDark => rgb(97, 175, 239),
            CatppuccinMocha => rgb(203, 166, 247),
            Dracula => rgb(189, 147, 249),
            RosePine => rgb(196, 167, 231),
            Monokai => rgb(166, 226, 46),
            Kanagawa => rgb(127, 180, 202),
            Everforest => rgb(131, 192, 146),
            MaterialOcean => rgb(130, 170, 255),
            Oxocarbon => rgb(120, 190, 255),
            Ayu => rgb(255, 182, 109),
            Nord => rgb(136, 192, 208),
            Gruvbox => rgb(214, 153, 33),
            SolarizedDark => rgb(42, 161, 152),
            Light => rgb(59, 130, 246),
            GruvboxLight => rgb(121, 116, 14),
            CatppuccinLatte => rgb(30, 102, 245),
        }
    }
}

// ─── Palette ───────────────────────────────────────────────────────────────

#[derive(Clone)]
struct Pal {
    bg: Color32, surface: Color32, surface2: Color32, hdr: Color32,
    accent: Color32, text: Color32, sub: Color32, dim: Color32,
    green: Color32, red: Color32, yellow: Color32, border: Color32,
    row_odd: Color32, row_even: Color32, row_sel: Color32, row_hov: Color32,
    light: bool,
}

fn rgb(r: u8, g: u8, b: u8) -> Color32 { Color32::from_rgb(r, g, b) }
fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color32 { Color32::from_rgba_unmultiplied(r, g, b, a) }
fn tint(c: Color32, a: u8) -> Color32 { rgba(c.r(), c.g(), c.b(), a) }

impl Pal {
    fn from(t: &Theme) -> Self {
        use Theme::*;
        match t {
            TokyoNight => Self {
                bg: rgb(26,27,38),    surface: rgb(36,40,59),   surface2: rgb(41,46,66),
                hdr: rgb(20,21,32),   accent: rgb(122,162,247),
                text: rgb(192,202,245), sub: rgb(169,177,214),  dim: rgb(86,95,137),
                green: rgb(158,206,106), red: rgb(247,118,142), yellow: rgb(224,175,104),
                border: rgb(41,46,66),
                row_odd: rgba(36,40,59,255), row_even: rgba(30,34,53,255),
                row_sel: rgba(122,162,247,55), row_hov: rgba(122,162,247,18), light: false,
            },
            Cyberpunk => Self {
                bg: rgb(10,14,28),    surface: rgb(16,24,48),   surface2: rgb(24,36,64),
                hdr: rgb(8,12,22),    accent: rgb(6,182,212),
                text: rgb(226,232,240), sub: rgb(148,163,184),  dim: rgb(71,85,105),
                green: rgb(34,197,94), red: rgb(239,68,68),     yellow: rgb(245,158,11),
                border: rgb(30,41,59),
                row_odd: rgba(16,24,48,255), row_even: rgba(20,30,58,255),
                row_sel: rgba(6,182,212,55), row_hov: rgba(6,182,212,18), light: false,
            },
            Midnight => Self {
                bg: rgb(5,5,16),      surface: rgb(12,12,26),   surface2: rgb(18,18,36),
                hdr: rgb(4,4,14),     accent: rgb(192,132,252),
                text: rgb(226,232,240), sub: rgb(148,163,184),  dim: rgb(60,60,90),
                green: rgb(52,211,153), red: rgb(248,113,113),  yellow: rgb(251,191,36),
                border: rgb(20,20,40),
                row_odd: rgba(12,12,26,255), row_even: rgba(8,8,20,255),
                row_sel: rgba(192,132,252,55), row_hov: rgba(192,132,252,15), light: false,
            },
            OneDark => Self {
                bg: rgb(40,44,52),    surface: rgb(33,37,43),   surface2: rgb(50,56,66),
                hdr: rgb(26,29,35),   accent: rgb(97,175,239),
                text: rgb(171,178,191), sub: rgb(130,137,151),  dim: rgb(92,99,112),
                green: rgb(152,195,121), red: rgb(224,108,117), yellow: rgb(229,192,123),
                border: rgb(60,68,80),
                row_odd: rgba(33,37,43,255), row_even: rgba(40,44,52,255),
                row_sel: rgba(97,175,239,55), row_hov: rgba(97,175,239,15), light: false,
            },
            CatppuccinMocha => Self {
                bg: rgb(30,30,46),    surface: rgb(24,24,37),   surface2: rgb(49,50,68),
                hdr: rgb(17,17,27),   accent: rgb(203,166,247),
                text: rgb(205,214,244), sub: rgb(166,173,200),  dim: rgb(108,112,134),
                green: rgb(166,227,161), red: rgb(243,139,168), yellow: rgb(249,226,175),
                border: rgb(49,50,68),
                row_odd: rgba(24,24,37,255), row_even: rgba(30,30,46,255),
                row_sel: rgba(203,166,247,55), row_hov: rgba(203,166,247,15), light: false,
            },
            Dracula => Self {
                bg: rgb(40,42,54),    surface: rgb(50,53,68),   surface2: rgb(60,63,80),
                hdr: rgb(30,32,44),   accent: rgb(189,147,249),
                text: rgb(248,248,242), sub: rgb(139,155,180),  dim: rgb(80,90,110),
                green: rgb(80,250,123), red: rgb(255,85,85),    yellow: rgb(241,250,140),
                border: rgb(60,63,80),
                row_odd: rgba(50,53,68,255), row_even: rgba(44,47,62,255),
                row_sel: rgba(189,147,249,55), row_hov: rgba(189,147,249,15), light: false,
            },
            RosePine => Self {
                bg: rgb(25,23,36),    surface: rgb(31,29,46),   surface2: rgb(64,61,82),
                hdr: rgb(18,17,26),   accent: rgb(196,167,231),
                text: rgb(224,222,244), sub: rgb(144,140,170),  dim: rgb(86,82,110),
                green: rgb(156,207,216), red: rgb(235,111,146), yellow: rgb(246,193,119),
                border: rgb(64,61,82),
                row_odd: rgba(31,29,46,255), row_even: rgba(25,23,36,255),
                row_sel: rgba(196,167,231,55), row_hov: rgba(196,167,231,15), light: false,
            },
            Monokai => Self {
                bg: rgb(39,40,34),    surface: rgb(47,49,40),   surface2: rgb(61,62,50),
                hdr: rgb(30,31,26),   accent: rgb(166,226,46),
                text: rgb(248,248,242), sub: rgb(200,200,190),  dim: rgb(117,113,94),
                green: rgb(166,226,46), red: rgb(249,38,114),   yellow: rgb(230,219,116),
                border: rgb(73,72,62),
                row_odd: rgba(47,49,40,255), row_even: rgba(39,40,34,255),
                row_sel: rgba(166,226,46,50), row_hov: rgba(166,226,46,15), light: false,
            },
            Kanagawa => Self {
                bg: rgb(22,22,30),    surface: rgb(31,31,40),   surface2: rgb(42,42,58),
                hdr: rgb(15,15,24),   accent: rgb(127,180,202),
                text: rgb(220,215,186), sub: rgb(150,147,127),  dim: rgb(84,84,109),
                green: rgb(118,185,0), red: rgb(195,64,67),     yellow: rgb(220,180,70),
                border: rgb(54,54,74),
                row_odd: rgba(31,31,40,255), row_even: rgba(22,22,30,255),
                row_sel: rgba(127,180,202,55), row_hov: rgba(127,180,202,15), light: false,
            },
            Everforest => Self {
                bg: rgb(45,53,59),    surface: rgb(52,61,70),   surface2: rgb(60,73,79),
                hdr: rgb(35,43,46),   accent: rgb(131,192,146),
                text: rgb(211,198,170), sub: rgb(157,153,136),  dim: rgb(105,103,95),
                green: rgb(131,192,146), red: rgb(230,126,128), yellow: rgb(219,188,127),
                border: rgb(74,82,90),
                row_odd: rgba(52,61,70,255), row_even: rgba(45,53,59,255),
                row_sel: rgba(131,192,146,55), row_hov: rgba(131,192,146,15), light: false,
            },
            MaterialOcean => Self {
                bg: rgb(15,17,26),    surface: rgb(13,14,22),   surface2: rgb(30,34,54),
                hdr: rgb(10,11,18),   accent: rgb(130,170,255),
                text: rgb(198,212,254), sub: rgb(137,148,184),  dim: rgb(72,82,113),
                green: rgb(195,232,141), red: rgb(255,85,114),  yellow: rgb(255,203,107),
                border: rgb(30,34,54),
                row_odd: rgba(13,14,22,255), row_even: rgba(15,17,26,255),
                row_sel: rgba(130,170,255,55), row_hov: rgba(130,170,255,15), light: false,
            },
            Oxocarbon => Self {
                bg: rgb(15,15,15),    surface: rgb(22,22,22),   surface2: rgb(32,32,32),
                hdr: rgb(10,10,10),   accent: rgb(120,190,255),
                text: rgb(244,244,244), sub: rgb(180,180,180),  dim: rgb(100,100,100),
                green: rgb(66,190,101), red: rgb(255,84,80),    yellow: rgb(243,196,0),
                border: rgb(45,45,45),
                row_odd: rgba(22,22,22,255), row_even: rgba(15,15,15,255),
                row_sel: rgba(120,190,255,55), row_hov: rgba(120,190,255,15), light: false,
            },
            Ayu => Self {
                bg: rgb(15,20,25),    surface: rgb(20,27,33),   surface2: rgb(26,34,44),
                hdr: rgb(11,15,20),   accent: rgb(255,182,109),
                text: rgb(203,215,232), sub: rgb(139,155,175),  dim: rgb(75,90,112),
                green: rgb(166,213,146), red: rgb(245,110,110), yellow: rgb(255,182,109),
                border: rgb(33,43,54),
                row_odd: rgba(20,27,33,255), row_even: rgba(15,20,25,255),
                row_sel: rgba(255,182,109,50), row_hov: rgba(255,182,109,15), light: false,
            },
            Nord => Self {
                bg: rgb(46,52,64),    surface: rgb(59,66,82),   surface2: rgb(67,76,94),
                hdr: rgb(36,42,54),   accent: rgb(136,192,208),
                text: rgb(236,239,244), sub: rgb(144,153,166),  dim: rgb(76,86,106),
                green: rgb(163,190,140), red: rgb(191,97,106),  yellow: rgb(235,203,139),
                border: rgb(67,76,94),
                row_odd: rgba(59,66,82,255), row_even: rgba(52,60,76,255),
                row_sel: rgba(136,192,208,55), row_hov: rgba(136,192,208,15), light: false,
            },
            Gruvbox => Self {
                bg: rgb(40,40,40),    surface: rgb(60,56,54),   surface2: rgb(80,73,69),
                hdr: rgb(29,32,33),   accent: rgb(214,153,33),
                text: rgb(235,219,178), sub: rgb(168,153,132),  dim: rgb(102,92,84),
                green: rgb(184,187,38), red: rgb(251,73,52),    yellow: rgb(250,189,47),
                border: rgb(80,73,69),
                row_odd: rgba(60,56,54,255), row_even: rgba(54,50,48,255),
                row_sel: rgba(214,153,33,55), row_hov: rgba(214,153,33,15), light: false,
            },
            SolarizedDark => Self {
                bg: rgb(0,43,54),     surface: rgb(7,54,66),    surface2: rgb(0,60,80),
                hdr: rgb(0,32,42),    accent: rgb(42,161,152),
                text: rgb(131,148,150), sub: rgb(101,123,131),  dim: rgb(55,83,98),
                green: rgb(133,153,0), red: rgb(220,50,47),     yellow: rgb(181,137,0),
                border: rgb(0,60,80),
                row_odd: rgba(7,54,66,255), row_even: rgba(0,48,60,255),
                row_sel: rgba(42,161,152,55), row_hov: rgba(42,161,152,15), light: false,
            },
            Light => Self {
                bg: rgb(249,250,251), surface: rgb(243,244,246), surface2: rgb(229,231,235),
                hdr: rgb(255,255,255), accent: rgb(59,130,246),
                text: rgb(17,24,39),  sub: rgb(75,85,99),        dim: rgb(156,163,175),
                green: rgb(22,163,74), red: rgb(220,38,38),      yellow: rgb(217,119,6),
                border: rgb(209,213,219),
                row_odd: rgba(255,255,255,255), row_even: rgba(249,250,251,255),
                row_sel: rgba(59,130,246,40), row_hov: rgba(59,130,246,12), light: true,
            },
            GruvboxLight => Self {
                bg: rgb(251,241,199), surface: rgb(242,229,188), surface2: rgb(213,196,161),
                hdr: rgb(255,248,212), accent: rgb(121,116,14),
                text: rgb(60,56,54),  sub: rgb(102,92,84),       dim: rgb(168,153,132),
                green: rgb(121,116,14), red: rgb(157,0,6),       yellow: rgb(181,118,20),
                border: rgb(213,196,161),
                row_odd: rgba(255,248,212,255), row_even: rgba(251,241,199,255),
                row_sel: rgba(121,116,14,45),   row_hov: rgba(121,116,14,12), light: true,
            },
            CatppuccinLatte => Self {
                bg: rgb(239,241,245), surface: rgb(230,233,239), surface2: rgb(204,208,218),
                hdr: rgb(255,255,255), accent: rgb(30,102,245),
                text: rgb(76,79,105), sub: rgb(100,104,132),     dim: rgb(156,160,176),
                green: rgb(64,160,43), red: rgb(210,15,57),      yellow: rgb(223,142,29),
                border: rgb(188,192,204),
                row_odd: rgba(255,255,255,255), row_even: rgba(239,241,245,255),
                row_sel: rgba(30,102,245,40),   row_hov: rgba(30,102,245,10), light: true,
            },
        }
    }
}

// ─── Config ────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone)]
struct Config {
    jackett_url: String,
    api_key: String,
    history: Vec<String>,
    favorites: Vec<Favorite>,
    theme: Theme,
    timeout_secs: u64,
    dedupe: bool,
    page_size: usize,
    row_height: f32,
    font_size: f32,
    show_cat_bar: bool,
    col_tracker: bool,
    col_size: bool,
    col_leech: bool,
    col_ratio: bool,
    col_health: bool,
    col_date: bool,
    #[serde(default)]
    rss_feeds: Vec<RssFeedConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            jackett_url: "http://localhost:9117".into(),
            api_key: String::new(),
            history: vec![],
            favorites: vec![],
            theme: Theme::TokyoNight,
            timeout_secs: 45,
            dedupe: false,
            page_size: 50,
            row_height: ROW_HEIGHT_NORMAL,
            font_size: 14.0,
            show_cat_bar: true,
            col_tracker: true, col_size: true, col_leech: true,
            col_ratio: true, col_health: true, col_date: true,
            rss_feeds: vec![],
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct Favorite {
    title: String,
    magnet: Option<String>,
    link: Option<String>,
    tracker: Option<String>,
    size: Option<u64>,
    seeders: Option<u32>,
    #[serde(default)]
    saved_at: String,
}

fn cfg_path() -> std::path::PathBuf {
    let d = dirs_next::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("torrentx");
    let _ = fs::create_dir_all(&d);
    d.join("config.json")
}

fn load_cfg() -> Config {
    fs::read_to_string(cfg_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_cfg(c: &Config) {
    if let Ok(j) = serde_json::to_string_pretty(c) {
        let _ = fs::write(cfg_path(), j);
    }
}

// ─── RSS types ──────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct RssFeedConfig {
    name: String,
    indexer: String,
    query: String,
    category: String,
    enabled: bool,
    #[serde(default)]
    auto_refresh: bool,
}

impl RssFeedConfig {
    fn new_default() -> Self {
        Self { name: "New Feed".into(), indexer: "all".into(), query: String::new(),
               category: String::new(), enabled: true, auto_refresh: true }
    }
}

#[derive(Clone, Debug, Default)]
struct RssItem {
    title: String,
    link: Option<String>,
    magnet: Option<String>,
    pub_date: Option<String>,
    size: Option<u64>,
    seeders: Option<u32>,
    leechers: Option<u32>,
    tracker: Option<String>,
    category: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
enum FeedStatus { Idle, Loading, Ok, Error }

struct RssFeedState {
    config: RssFeedConfig,
    items: Vec<RssItem>,
    status: FeedStatus,
    error: Option<String>,
}

impl RssFeedState {
    fn new(config: RssFeedConfig) -> Self {
        Self { config, items: vec![], status: FeedStatus::Idle, error: None }
    }
    fn status_icon(&self) -> &'static str {
        match self.status {
            FeedStatus::Idle => "○", FeedStatus::Loading => "⟳",
            FeedStatus::Ok => "●", FeedStatus::Error => "✕",
        }
    }
}

fn build_rss_url(base: &str, key: &str, cfg: &RssFeedConfig) -> String {
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

fn parse_torznab_xml(xml: &str) -> Result<Vec<RssItem>, String> {
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
            Ok(Event::Empty(ref e)) => {
                if in_item {
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
                                    "size" => { if item.size.is_none() { item.size = val.parse().ok(); } }
                                    _ => {}
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
                                    "title" => item.title = t,
                                    "link" => { if item.link.is_none() { item.link = Some(t); } }
                                    "pubdate" | "pubDate" => item.pub_date = Some(t),
                                    "size" => { if item.size.is_none() { item.size = t.parse().ok(); } }
                                    "jackettindexer" => item.tracker = Some(t),
                                    "category" => item.category = Some(t),
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

fn tag_name(raw: &[u8]) -> String {
    std::str::from_utf8(raw).unwrap_or("").to_lowercase()
}

fn fetch_rss(url: &str, timeout: u64) -> Result<Vec<RssItem>, String> {
    let client = Client::builder().timeout(Duration::from_secs(timeout)).build()
        .map_err(|e| format!("Client error: {e}"))?;
    let resp = client.get(url).send().map_err(|e| {
        if e.is_connect() { "Cannot reach Jackett. Is it running?".into() }
        else if e.is_timeout() { format!("Timed out after {timeout}s") }
        else { format!("Network error: {e}") }
    })?;
    if !resp.status().is_success() { return Err(format!("HTTP {}", resp.status().as_u16())); }
    let body = resp.text().map_err(|e| format!("Read error: {e}"))?;
    parse_torznab_xml(&body)
}

fn start_rss_fetch(
    base_url: String, api_key: String, feed_cfg: RssFeedConfig, timeout: u64,
    feed_idx: usize,
    tx: std::sync::mpsc::SyncSender<(usize, Result<Vec<RssItem>, String>)>,
) {
    thread::spawn(move || {
        let url = build_rss_url(&base_url, &api_key, &feed_cfg);
        let result = fetch_rss(&url, timeout);
        let _ = tx.send((feed_idx, result));
    });
}

// ─── App state types ───────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
enum SortCol { Name, Tracker, Size, Seeds, Leech, Date }

#[derive(Clone, PartialEq)]
enum SortDir { Asc, Desc }

#[derive(Clone, PartialEq)]
enum Tab { Search, Favorites, Rss, About }

#[derive(Clone, PartialEq)]
enum SearchState { Idle, Searching, Done, Error(String) }

#[derive(Clone, PartialEq)]
enum Hlth { All, Hot, Good, Slow, Dead }

impl Hlth {
    fn label(&self) -> &'static str {
        match self {
            Hlth::All => "All",
            Hlth::Hot => "HOT",
            Hlth::Good => "GOOD",
            Hlth::Slow => "SLOW",
            Hlth::Dead => "DEAD",
        }
    }
    fn ok(&self, s: u32) -> bool {
        match self {
            Hlth::All => true,
            Hlth::Hot => s > 500,
            Hlth::Good => (101..=500).contains(&s),
            Hlth::Slow => (11..=100).contains(&s),
            Hlth::Dead => s <= 10,
        }
    }
}

#[derive(Clone)]
struct Toast { msg: String, ttl: f32, col: Color32 }

// ─── App ───────────────────────────────────────────────────────────────────

struct App {
    cfg: Config,
    pal: Pal,
    // search
    query: String,
    cat: String,
    // filters
    f_text: String,
    f_seed: String,
    f_size: String,
    f_year: String,
    f_trk: String,
    f_hlth: Hlth,
    // sort
    s_col: SortCol,
    s_dir: SortDir,
    // async search
    results: Arc<Mutex<Vec<TorrentResult>>>,
    state: Arc<Mutex<SearchState>>,
    count: Arc<Mutex<usize>>,
    // ui
    tab: Tab,
    show_settings: bool,
    key_vis: bool,
    selected: Option<usize>,
    detail_open: bool,
    detail_width: f32,
    show_hist: bool,
    page: usize,
    last_query: String,
    toasts: Vec<Toast>,
    hovered: Option<usize>,
    fav_search: String,
    // RSS
    rss_feeds: Vec<RssFeedState>,
    rss_selected: usize,
    rss_detail: Option<usize>,
    rss_filter: String,
    rss_add_mode: bool,
    rss_edit_idx: Option<usize>,
    rss_new_cfg: RssFeedConfig,
    // timing / spinner
    t_start: Option<Instant>,
    t_done: Option<f64>,
    spin_i: usize,
    spin_t: f32,
}

impl Default for App {
    fn default() -> Self {
        let cfg = load_cfg();
        let pal = Pal::from(&cfg.theme);
        let feeds: Vec<RssFeedState> = cfg.rss_feeds.iter().map(|c| RssFeedState::new(c.clone())).collect();
        Self {
            cfg, pal,
            query: String::new(), cat: "All".into(),
            f_text: String::new(), f_seed: String::new(),
            f_size: String::new(), f_year: String::new(),
            f_trk: String::new(), f_hlth: Hlth::All,
            s_col: SortCol::Seeds, s_dir: SortDir::Desc,
            results: Arc::new(Mutex::new(vec![])),
            state: Arc::new(Mutex::new(SearchState::Idle)),
            count: Arc::new(Mutex::new(0)),
            tab: Tab::Search, show_settings: false, key_vis: false,
            selected: None, detail_open: false, detail_width: 295.0, show_hist: false,
            page: 0, last_query: String::new(), toasts: vec![],
            hovered: None, fav_search: String::new(),
            rss_feeds: feeds,
            rss_selected: 0, rss_detail: None, rss_filter: String::new(),
            rss_add_mode: false, rss_edit_idx: None,
            rss_new_cfg: RssFeedConfig::new_default(),
            t_start: None, t_done: None, spin_i: 0, spin_t: 0.0,
        }
    }
}

// ─── Pure helpers ──────────────────────────────────────────────────────────

fn fmt_size(b: u64) -> String {
    let b = b as f64;
    if b >= 1_073_741_824.0 { format!("{:.2} GB", b / 1_073_741_824.0) }
    else if b >= 1_048_576.0 { format!("{:.0} MB", b / 1_048_576.0) }
    else { format!("{:.0} KB", b / 1_024.0) }
}

fn time_ago(s: &str) -> String {
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

fn pub_year(s: &str) -> u32 {
    chrono::DateTime::parse_from_rfc3339(s)
        .or_else(|_| chrono::DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%z"))
        .ok()
        .and_then(|dt| dt.format("%Y").to_string().parse::<u32>().ok())
        .unwrap_or(0)
}

fn seed_col(s: u32) -> Color32 {
    if s > 500 { rgb(34,197,94) } else if s > 100 { rgb(74,222,128) }
    else if s > 10 { rgb(245,158,11) } else if s > 0 { rgb(249,115,22) }
    else { rgb(239,68,68) }
}

fn hlth_lbl(s: u32) -> &'static str {
    if s > 500 {"HOT"} else if s > 100 {"GOOD"}
    else if s > 10 {"SLOW"} else if s > 0 {"DYING"} else {"DEAD"}
}

fn cat_col(cat: &str) -> Color32 {
    match cat.split('/').next().unwrap_or("").trim() {
        "Movies" => rgb(245,158,11), "TV" => rgb(59,130,246),
        "Music" => rgb(16,185,129), "Games" => rgb(139,92,246),
        "Software" => rgb(6,182,212), "Anime" => rgb(236,72,153),
        "Books" => rgb(249,115,22), _ => rgb(100,116,139),
    }
}

fn urlenc(s: &str) -> String {
    s.chars().map(|c| match c {
        'A'..='Z'|'a'..='z'|'0'..='9'|'-'|'_'|'.'|'~' => c.to_string(),
        ' ' => "+".into(),
        c => format!("%{:02X}", c as u32),
    }).collect()
}

fn normalize(t: &str) -> String {
    let stop = ["1080p","720p","480p","4k","bluray","bdrip","webrip",
                "x264","x265","hevc","10bit","hdr","yify","yts","rarbg",
                "mkv","mp4","avi","remux"];
    let mut s = t.to_lowercase();
    for w in &stop { s = s.replace(w, " "); }
    s.split_whitespace().take(4).collect::<Vec<_>>().join(" ")
}

fn now_str() -> String { chrono::Utc::now().format("%Y-%m-%d %H:%M").to_string() }

fn set_err(st: &Arc<Mutex<SearchState>>, msg: String) {
    if let Ok(mut s) = st.lock() { *s = SearchState::Error(msg); }
}

// ─── Small UI buttons ──────────────────────────────────────────────────────

fn act_btn(ui: &mut egui::Ui, label: &str, tip: &str, color: Color32) -> bool {
    ui.add(
        egui::Button::new(RichText::new(label).size(11.5).color(color))
            .fill(tint(color, 18))
            .stroke(Stroke::new(1.0, tint(color, 70)))
            .rounding(5.0)
            .min_size(Vec2::new(0.0, 25.0))
    ).on_hover_text(tip).clicked()
}

fn status_pill(ui: &mut egui::Ui, label: &str, col: Color32) {
    egui::Frame::none().fill(tint(col, 20)).rounding(6.0)
        .inner_margin(egui::Margin::symmetric(6.0, 2.0))
        .show(ui, |ui| { ui.label(RichText::new(label).font(FontId::proportional(10.5)).color(col)); });
}

fn wide_btn(ui: &mut egui::Ui, label: &str, color: Color32) -> bool {
    let w = ui.available_width().max(200.0);
    ui.add(
        egui::Button::new(RichText::new(label).font(FontId::proportional(13.0)).color(color))
            .fill(tint(color, 18))
            .stroke(Stroke::new(1.0, tint(color, 80)))
            .rounding(6.0)
            .min_size(Vec2::new(w, 34.0))
    ).clicked()
}

fn outline_btn(ui: &mut egui::Ui, label: &str, color: Color32) -> bool {
    ui.add(
        egui::Button::new(RichText::new(label).font(FontId::proportional(12.0)).color(color))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, tint(color, 80)))
            .rounding(4.0)
    ).clicked()
}

fn lbl(ui: &mut egui::Ui, text: &str, color: Color32, fs: f32) {
    ui.label(RichText::new(text).font(FontId::proportional(fs)).color(color));
}

// ─── Search thread ─────────────────────────────────────────────────────────

fn start_search(
    url: String, key: String, query: String, cat: String, timeout: u64,
    results: Arc<Mutex<Vec<TorrentResult>>>,
    state: Arc<Mutex<SearchState>>,
    count: Arc<Mutex<usize>>,
) {
    thread::spawn(move || {
        if let Ok(mut s) = state.lock() { *s = SearchState::Searching; }
        let mut ep = format!(
            "{}/api/v2.0/indexers/all/results?apikey={}&Query={}",
            url.trim_end_matches('/'), urlenc(&key), urlenc(&query)
        );
        if cat != "All" { ep.push_str(&format!("&Category[]={}", urlenc(&cat))); }

        let client = match Client::builder().timeout(Duration::from_secs(timeout)).build() {
            Ok(c) => c,
            Err(e) => { set_err(&state, format!("Client error: {e}")); return; }
        };
        match client.get(&ep).send() {
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
        }
    });
}

// ─── App methods ───────────────────────────────────────────────────────────

impl App {
    fn do_search(&mut self) {
        let q = self.query.trim().to_string();
        if q.is_empty() { return; }
        if self.cfg.api_key.trim().is_empty() {
            set_err(&self.state, "No API key — open Settings and paste your Jackett API key.".into());
            self.show_settings = true;
            return;
        }
        self.cfg.history.retain(|h| h != &q);
        self.cfg.history.insert(0, q.clone());
        self.cfg.history.truncate(20);
        save_cfg(&self.cfg);
        self.selected = None; self.detail_open = false;
        self.show_hist = false; self.page = 0;
        self.last_query = q.clone(); self.f_text.clear();
        self.hovered = None; self.t_start = Some(Instant::now()); self.t_done = None;
        if let Ok(mut r) = self.results.lock() { r.clear(); }
        if let Ok(mut c) = self.count.lock() { *c = 0; }
        start_search(
            self.cfg.jackett_url.clone(), self.cfg.api_key.clone(),
            q, self.cat.clone(), self.cfg.timeout_secs,
            Arc::clone(&self.results), Arc::clone(&self.state), Arc::clone(&self.count),
        );
    }

    fn add_fav(&mut self, r: &TorrentResult) {
        if self.cfg.favorites.iter().any(|f| f.title == r.title) {
            self.toast("Already in Favorites", self.pal.yellow); return;
        }
        self.cfg.favorites.push(Favorite {
            title: r.title.clone(), magnet: r.magnet_uri.clone(), link: r.link.clone(),
            tracker: r.tracker.clone(), size: r.size, seeders: r.seeders, saved_at: now_str(),
        });
        save_cfg(&self.cfg);
        self.toast("Saved to Favorites ★", self.pal.yellow);
    }

    fn toast(&mut self, msg: &str, col: Color32) {
        self.toasts.retain(|t| t.msg != msg);
        self.toasts.push(Toast { msg: msg.into(), ttl: 3.0, col });
    }

    fn set_theme(&mut self, t: Theme) {
        self.cfg.theme = t; self.pal = Pal::from(&self.cfg.theme); save_cfg(&self.cfg);
    }

    // ── RSS helpers ───────────────────────────────────────────────────────

    fn sync_rss_configs(&mut self) {
        self.cfg.rss_feeds = self.rss_feeds.iter().map(|f| f.config.clone()).collect();
        save_cfg(&self.cfg);
    }

    fn refresh_feed(&mut self, idx: usize) {
        if idx >= self.rss_feeds.len() { return; }
        self.rss_feeds[idx].status = FeedStatus::Loading;
        self.rss_feeds[idx].error = None;
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let base = self.cfg.jackett_url.clone();
        let key = self.cfg.api_key.clone();
        let cfg = self.rss_feeds[idx].config.clone();
        let to = self.cfg.timeout_secs;
        start_rss_fetch(base, key, cfg, to, idx, tx.clone());
        // ponytail: poll in next frame
        drop(tx); // signal no more sends
        std::thread::spawn(move || {
            if let Ok((idx, result)) = rx.recv() {
                // Results handled in poll_rss()
                let _ = (idx, result);
            }
        });
    }

    fn refresh_all_feeds(&mut self) {
        for i in 0..self.rss_feeds.len() {
            if self.rss_feeds[i].config.enabled { self.refresh_feed(i); }
        }
    }

    fn poll_rss(&mut self) {
        // ponytail: simple sync poll — in a real app use channels
        // Check if any feed is loading and poll results
        for i in 0..self.rss_feeds.len() {
            if self.rss_feeds[i].status == FeedStatus::Loading {
                // Start async fetch
                let base = self.cfg.jackett_url.clone();
                let key = self.cfg.api_key.clone();
                let cfg = self.rss_feeds[i].config.clone();
                let to = self.cfg.timeout_secs;
                let url = build_rss_url(&base, &key, &cfg);
                match fetch_rss(&url, to) {
                    Ok(items) => {
                        self.rss_feeds[i].items = items;
                        self.rss_feeds[i].status = FeedStatus::Ok;
                        self.rss_feeds[i].error = None;
                    }
                    Err(e) => {
                        self.rss_feeds[i].status = FeedStatus::Error;
                        self.rss_feeds[i].error = Some(e);
                    }
                }
            }
        }
    }

    fn add_fav_from_rss(&mut self, item: &RssItem) {
        let now = chrono::Local::now().format("%Y-%m-%d").to_string();
        self.cfg.favorites.push(Favorite {
            title: item.title.clone(), magnet: item.magnet.clone(),
            link: item.link.clone(), tracker: item.tracker.clone(),
            size: item.size, seeders: item.seeders, saved_at: now,
        });
        save_cfg(&self.cfg);
        self.toast("Saved to Favorites ★", self.pal.yellow);
    }

    fn cur_state(&self) -> SearchState {
        self.state.lock().map(|g| g.clone()).unwrap_or(SearchState::Idle)
    }
    fn all_results(&self) -> Vec<TorrentResult> {
        self.results.lock().map(|g| g.clone()).unwrap_or_default()
    }
    fn total_count(&self) -> usize {
        self.count.lock().map(|g| *g).unwrap_or(0)
    }

    fn filtered(&self, raw: &[TorrentResult]) -> Vec<TorrentResult> {
        let min_s: u32 = self.f_seed.parse().unwrap_or(0);
        let max_b: u64 = self.f_size.parse::<f64>().unwrap_or(0.0) as u64 * 1_073_741_824;
        let min_y: u32 = self.f_year.parse().unwrap_or(0);
        let trk = self.f_trk.to_lowercase();
        let txt = self.f_text.to_lowercase();
        let mut seen = std::collections::HashSet::new();

        let mut out: Vec<_> = raw.iter().filter(|r| {
            let s = r.seeders.unwrap_or(0);
            if s < min_s { return false; }
            if max_b > 0 && r.size.unwrap_or(0) > max_b { return false; }
            if min_y > 0 && r.publish_date.as_deref().map(pub_year).unwrap_or(0) < min_y { return false; }
            if !trk.is_empty() && !r.tracker.as_deref().unwrap_or("").to_lowercase().contains(&trk) { return false; }
            if !txt.is_empty() {
                let hay = format!("{} {} {}",
                    r.title.to_lowercase(),
                    r.tracker.as_deref().unwrap_or("").to_lowercase(),
                    r.category_desc.as_deref().unwrap_or("").to_lowercase());
                if !hay.contains(&txt) { return false; }
            }
            if !self.f_hlth.ok(s) { return false; }
            if self.cfg.dedupe && !seen.insert(normalize(&r.title)) { return false; }
            true
        }).cloned().collect();

        out.sort_by(|a, b| {
            let c = match self.s_col {
                SortCol::Seeds => b.seeders.unwrap_or(0).cmp(&a.seeders.unwrap_or(0)),
                SortCol::Leech => b.peers.unwrap_or(0).cmp(&a.peers.unwrap_or(0)),
                SortCol::Size => b.size.unwrap_or(0).cmp(&a.size.unwrap_or(0)),
                SortCol::Name => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
                SortCol::Tracker => a.tracker.as_deref().unwrap_or("").to_lowercase()
                                     .cmp(&b.tracker.as_deref().unwrap_or("").to_lowercase()),
                SortCol::Date => b.publish_date.as_deref().unwrap_or("")
                                     .cmp(a.publish_date.as_deref().unwrap_or("")),
            };
            if self.s_dir == SortDir::Asc { c.reverse() } else { c }
        });
        out
    }

    fn max_pages(&self, n: usize) -> usize {
        if self.cfg.page_size == 0 || n == 0 { return 1; }
        (n + self.cfg.page_size - 1) / self.cfg.page_size
    }
    fn page_slice<'a>(&self, v: &'a [TorrentResult]) -> &'a [TorrentResult] {
        if self.cfg.page_size == 0 { return v; }
        let s = self.page * self.cfg.page_size;
        if s >= v.len() { return &[]; }
        &v[s..(s + self.cfg.page_size).min(v.len())]
    }

    fn cat_chips(results: &[TorrentResult]) -> Vec<(String, usize, Color32)> {
        let mut map: std::collections::BTreeMap<String, usize> = Default::default();
        for r in results {
            let c = r.category_desc.as_deref()
                .and_then(|c| c.split('/').next())
                .unwrap_or("Other").trim().to_string();
            *map.entry(c).or_insert(0) += 1;
        }
        let mut v: Vec<_> = map.into_iter().map(|(k, n)| { let col = cat_col(&k); (k, n, col) }).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        v.truncate(7);
        v
    }

    fn export_csv(&self, rows: &[TorrentResult]) {
        let path = dirs_next::download_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(format!("torrentx_{}.csv",
                self.last_query.replace(' ', "_").replace('/', "-")));
        let mut out = "Title,Tracker,Category,Size,Seeders,Leechers,Date\n".to_string();
        for r in rows {
            out.push_str(&format!(
                "\"{}\",\"{}\",\"{}\",\"{}\",{},{},\"{}\"\n",
                r.title.replace('"', "'"),
                r.tracker.as_deref().unwrap_or(""),
                r.category_desc.as_deref().unwrap_or(""),
                r.size.map(fmt_size).unwrap_or_default(),
                r.seeders.unwrap_or(0), r.peers.unwrap_or(0),
                r.publish_date.as_deref().map(time_ago).unwrap_or_default(),
            ));
        }
        if fs::write(&path, out).is_ok() { let _ = open::that(&path); }
    }

    fn apply_theme(&self, ctx: &egui::Context) {
        let p = &self.pal;
        let mut vis = if p.light { Visuals::light() } else { Visuals::dark() };
        vis.panel_fill = p.bg;
        vis.window_fill = p.bg;
        vis.faint_bg_color = p.surface;
        vis.extreme_bg_color = p.hdr;
        vis.widgets.noninteractive.bg_fill = p.surface;
        vis.widgets.inactive.bg_fill = p.surface;
        vis.widgets.hovered.bg_fill = p.surface2;
        vis.widgets.active.bg_fill = p.accent;
        vis.selection.bg_fill = tint(p.accent, 50);
        vis.override_text_color = Some(p.text);
        vis.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.dim);
        vis.widgets.inactive.fg_stroke = Stroke::new(1.0, p.sub);
        vis.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
        vis.widgets.inactive.bg_stroke = Stroke::new(1.0, p.border);
        let rn = egui::Rounding::same(6.0);
        vis.widgets.noninteractive.rounding = rn;
        vis.widgets.inactive.rounding = rn;
        vis.widgets.hovered.rounding = rn;
        vis.widgets.active.rounding = rn;
        ctx.set_visuals(vis);
    }
}

// ─── eframe::App main loop ─────────────────────────────────────────────────

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.apply_theme(ctx);
        let state = self.cur_state();

        // Spinner tick
        if state == SearchState::Searching {
            ctx.request_repaint_after(Duration::from_millis(80));
            let dt = ctx.input(|i| i.unstable_dt).clamp(0.0, 0.1);
            self.spin_t += dt;
            if self.spin_t > 0.1 { self.spin_t = 0.0; self.spin_i = (self.spin_i + 1) % SPIN.len(); }
        }
        if matches!(state, SearchState::Done | SearchState::Error(_)) {
            if let Some(t) = self.t_start.take() { self.t_done = Some(t.elapsed().as_secs_f64()); }
        }

        // Toast decay
        let dt = ctx.input(|i| i.unstable_dt).clamp(0.0, 0.1);
        self.toasts.retain_mut(|t| { t.ttl -= dt; t.ttl > 0.0 });

        // Global shortcuts
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::F)) {
            ctx.memory_mut(|m| m.request_focus(egui::Id::new("q")));
        }
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::R)) { self.do_search(); }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.detail_open { self.detail_open = false; } else { self.query.clear(); self.show_hist = false; }
        }
        // Copy magnet from detail panel with Ctrl+C
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::C) && self.detail_open) {
            if let Some(idx) = self.selected {
                let raw = self.all_results();
                let sorted = self.filtered(&raw);
                let page_s = self.page_slice(&sorted);
                if let Some(r) = page_s.get(idx) {
                    if let Some(m) = &r.magnet_uri {
                        ctx.output_mut(|o| o.copied_text = m.clone());
                        self.toast("Magnet copied ✓", self.pal.green);
                    }
                }
            }
        }

        // ── Status bar ───────────────────────────────────────────────────
        egui::TopBottomPanel::bottom("sb")
            .exact_height(26.0)
            .frame(egui::Frame::none()
                .fill(self.pal.hdr).stroke(Stroke::new(1.0, self.pal.border))
                .inner_margin(egui::Margin::symmetric(12.0, 4.0)))
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    match &state {
                        SearchState::Idle => {
                            lbl(ui, "Ready — type a query and press Search", self.pal.dim, 12.0);
                        }
                        SearchState::Searching => {
                            let sp = SPIN[self.spin_i];
                            let el = self.t_start.as_ref()
                                .map(|t| format!("  {:.1}s", t.elapsed().as_secs_f64()))
                                .unwrap_or_default();
                            lbl(ui, &format!("{sp} Searching \"{}\"{}", self.last_query, el), self.pal.accent, 12.0);
                        }
                        SearchState::Done => {
                            let n = self.total_count();
                            let e = self.t_done.map(|e| format!("  ({:.1}s)", e)).unwrap_or_default();
                            lbl(ui, &format!("✓ {n} results for \"{}\"{}", self.last_query, e), self.pal.green, 12.0);
                        }
                        SearchState::Error(e) => {
                            lbl(ui, &format!("✕ {}", e.lines().next().unwrap_or(e)), self.pal.red, 12.0);
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        lbl(ui, "Ctrl+F  Ctrl+R  ↑↓  Enter  D=detail  F=fav  M=magnet  Esc=close",
                            self.pal.dim, 10.5);
                    });
                });
            });

        // ── Header ───────────────────────────────────────────────────────
        self.draw_header(ctx);

        // ── RSS polling ───────────────────────────────────────────────
        self.poll_rss();

        // ── Settings panel ───────────────────────────────────────────────
        if self.show_settings {
            egui::TopBottomPanel::top("settings")
                .frame(egui::Frame::none()
                    .fill(self.pal.hdr).stroke(Stroke::new(1.0, self.pal.border))
                    .inner_margin(egui::Margin::symmetric(14.0, 8.0)))
                .show(ctx, |ui| {
                    self.draw_settings_panel(ui);
                });
        }

        // ── Central panel ────────────────────────────────────────────────
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(self.pal.bg))
            .show(ctx, |ui| {
                match self.tab.clone() {
                    Tab::Search => self.draw_search(ui, ctx, &state),
                    Tab::Favorites => self.draw_favorites(ui),
                    Tab::Rss => self.draw_rss(ui, ctx),
                    Tab::About => self.draw_about(ui),
                }
            });

        // Detail panel (top-level SidePanel::right, resizable)
        self.draw_detail_panel(ctx);

        self.draw_toasts(ctx);
    }
}

// ─── Header ────────────────────────────────────────────────────────────────

impl App {
    fn draw_header(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("hdr")
            .exact_height(52.0)
            .frame(egui::Frame::none()
                .fill(self.pal.surface).stroke(Stroke::new(1.0, self.pal.border)))
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(MARGIN_DEFAULT + 2.0);
                    // Logo
                    ui.label(RichText::new("Torrent").font(FontId::monospace(16.0)).strong().color(self.pal.text));
                    ui.label(RichText::new("X").font(FontId::monospace(16.0)).strong().color(self.pal.accent));
                    egui::Frame::none()
                        .fill(tint(self.pal.accent, 28)).rounding(10.0)
                        .inner_margin(egui::Margin::symmetric(5.0, 1.0))
                        .show(ui, |ui| {
                            ui.label(RichText::new("v6").size(10.0).color(self.pal.accent));
                        });
                    ui.add_space(14.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // Tabs
                    for (label, tab) in [("🔍", Tab::Search), ("★", Tab::Favorites), ("📡", Tab::Rss), ("ℹ", Tab::About)] {
                        let active = self.tab == tab;
                        let badge = if tab == Tab::Favorites && !self.cfg.favorites.is_empty() {
                            format!(" {}", self.cfg.favorites.len())
                        } else { String::new() };
                        if ui.add(egui::Button::new(
                            RichText::new(format!("{label}{badge}")).font(FontId::proportional(14.0))
                                .color(if active { self.pal.accent } else { self.pal.sub }))
                            .fill(if active { tint(self.pal.accent, 22) } else { Color32::TRANSPARENT })
                            .stroke(Stroke::new(if active { 1.0 } else { 0.0 }, self.pal.accent))
                            .rounding(6.0).min_size(Vec2::new(0.0, 30.0))
                        ).clicked() {
                            self.tab = tab;
                            self.detail_open = false;
                            self.selected = None;
                        }
                        ui.add_space(2.0);
                    }

                    // Right side controls
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(12.0);
                        let sa = self.show_settings;
                        if ui.add(egui::Button::new(
                            RichText::new("⚙ Settings").size(13.0)
                                .color(if sa { self.pal.accent } else { self.pal.sub }))
                            .fill(if sa { tint(self.pal.accent, 22) } else { Color32::TRANSPARENT })
                            .stroke(Stroke::new(1.0, if sa { self.pal.accent } else { self.pal.border }))
                            .rounding(6.0).min_size(Vec2::new(0.0, 30.0))
                        ).clicked() { self.show_settings = !self.show_settings; }
                        ui.add_space(10.0);

                        // Theme picker
                        let ac = self.cfg.theme.accent_color();
                        egui::ComboBox::from_id_source("theme_cb")
                            .selected_text(RichText::new(self.cfg.theme.name())
                                .font(FontId::proportional(13.0)).color(ac))
                            .width(155.0)
                            .show_ui(ui, |ui| {
                                ui.label(RichText::new("─ Dark ─").size(10.0).color(self.pal.dim));
                                for t in Theme::all().iter().filter(|t| !t.is_light()) {
                                    let col = t.accent_color();
                                    let on = &self.cfg.theme == t;
                                    if ui.add(egui::SelectableLabel::new(on,
                                        RichText::new(format!("  {}", t.name()))
                                            .font(FontId::proportional(13.0)).color(col)
                                    )).clicked() { self.set_theme(t.clone()); }
                                }
                                ui.add_space(3.0);
                                ui.label(RichText::new("─ Light ─").size(10.0).color(self.pal.dim));
                                for t in Theme::all().iter().filter(|t| t.is_light()) {
                                    let col = t.accent_color();
                                    let on = &self.cfg.theme == t;
                                    if ui.add(egui::SelectableLabel::new(on,
                                        RichText::new(format!("  {}", t.name()))
                                            .font(FontId::proportional(13.0)).color(col)
                                    )).clicked() { self.set_theme(t.clone()); }
                                }
                            });
                        ui.add_space(10.0);
                        let n = self.total_count();
                        if n > 0 { lbl(ui, &format!("{n} results"), self.pal.dim, 12.0); }
                    });
                });
            });
    }
}

// ─── Settings panel ────────────────────────────────────────────────────────

impl App {
    fn draw_settings_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_space(MARGIN_DEFAULT);
            ui.vertical(|ui| {
                // Row 1 — Connection
                ui.horizontal(|ui| {
                    lbl(ui, "CONNECTION", self.pal.dim, 10.0);
                    ui.add_space(6.0);
                    lbl(ui, "Jackett URL", self.pal.sub, 12.0);
                    ui.add(egui::TextEdit::singleline(&mut self.cfg.jackett_url)
                        .desired_width(172.0).font(FontId::monospace(12.0)));
                    ui.add_space(6.0);
                    lbl(ui, "API Key", self.pal.sub, 12.0);
                    ui.add(egui::TextEdit::singleline(&mut self.cfg.api_key)
                        .desired_width(210.0).password(!self.key_vis)
                        .hint_text("from Jackett dashboard (top-right)")
                        .font(FontId::monospace(12.0)));
                    if ui.small_button(if self.key_vis { "hide" } else { "show" }).clicked() {
                        self.key_vis = !self.key_vis;
                    }
                    ui.add_space(6.0);
                    lbl(ui, "Timeout", self.pal.sub, 12.0);
                    let mut ts = self.cfg.timeout_secs.to_string();
                    if ui.add(egui::TextEdit::singleline(&mut ts)
                        .desired_width(30.0).font(FontId::monospace(12.0))).changed() {
                        if let Ok(v) = ts.parse::<u64>() { self.cfg.timeout_secs = v.clamp(5, 120); }
                    }
                    lbl(ui, "s", self.pal.dim, 11.0);
                });
                ui.add_space(5.0);

                // Row 2 — Display
                ui.horizontal(|ui| {
                    lbl(ui, "DISPLAY", self.pal.dim, 10.0);
                    ui.add_space(6.0);
                    lbl(ui, "Rows", self.pal.sub, 12.0);
                    for (l, h) in [("Compact", ROW_HEIGHT_COMPACT), ("Normal", ROW_HEIGHT_NORMAL), ("Roomy", ROW_HEIGHT_ROOMY)] {
                        let on = (self.cfg.row_height - h).abs() < 1.0;
                        if ui.add(egui::SelectableLabel::new(on,
                            RichText::new(l).font(FontId::proportional(12.0))
                        )).clicked() { self.cfg.row_height = h; save_cfg(&self.cfg); }
                    }
                    ui.add_space(8.0);
                    lbl(ui, "Font", self.pal.sub, 12.0);
                    for (l, sz) in [("S", 12.0f32), ("M", 14.0), ("L", 16.0)] {
                        let on = (self.cfg.font_size - sz).abs() < 0.5;
                        if ui.add(egui::SelectableLabel::new(on,
                            RichText::new(l).font(FontId::proportional(12.0))
                        )).clicked() { self.cfg.font_size = sz; save_cfg(&self.cfg); }
                    }
                    ui.add_space(8.0);
                    lbl(ui, "Page", self.pal.sub, 12.0);
                    for (l, ps) in [("25", 25usize), ("50", 50), ("100", 100), ("All", 0)] {
                        let on = self.cfg.page_size == ps;
                        if ui.add(egui::SelectableLabel::new(on,
                            RichText::new(l).font(FontId::proportional(12.0))
                        )).clicked() { self.cfg.page_size = ps; self.page = 0; save_cfg(&self.cfg); }
                    }
                    ui.add_space(8.0);
                    if ui.add(egui::SelectableLabel::new(self.cfg.dedupe,
                        RichText::new("Dedupe").font(FontId::proportional(12.0))
                    )).on_hover_text("Merge near-duplicate titles across trackers").clicked() {
                        self.cfg.dedupe = !self.cfg.dedupe; save_cfg(&self.cfg);
                    }
                    ui.add_space(4.0);
                    if ui.add(egui::SelectableLabel::new(self.cfg.show_cat_bar,
                        RichText::new("Cat bar").font(FontId::proportional(12.0))
                    )).on_hover_text("Show category breakdown chips").clicked() {
                        self.cfg.show_cat_bar = !self.cfg.show_cat_bar; save_cfg(&self.cfg);
                    }
                });
                ui.add_space(5.0);

                // Row 3 — Columns
                ui.horizontal(|ui| {
                    lbl(ui, "COLUMNS", self.pal.dim, 10.0);
                    ui.add_space(6.0);
                    let mut col_changed = false;
                    for (label, val) in [
                        ("Tracker", &mut self.cfg.col_tracker),
                        ("Size", &mut self.cfg.col_size),
                        ("Leech", &mut self.cfg.col_leech),
                        ("Ratio", &mut self.cfg.col_ratio),
                        ("Health", &mut self.cfg.col_health),
                        ("Date", &mut self.cfg.col_date),
                    ] {
                        let on = *val;
                        if ui.add(egui::SelectableLabel::new(on,
                            RichText::new(label).font(FontId::proportional(12.0))
                                .color(if on { self.pal.accent } else { self.pal.dim })
                        )).clicked() { *val = !*val; col_changed = true; }
                        ui.add_space(2.0);
                    }
                    if col_changed { save_cfg(&self.cfg); }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new(
                            RichText::new("Save").font(FontId::proportional(12.0)).color(self.pal.green))
                            .fill(tint(self.pal.green, 18))
                            .stroke(Stroke::new(1.0, tint(self.pal.green, 80))).rounding(4.0)
                        ).clicked() {
                            save_cfg(&self.cfg);
                            self.toast("Settings saved ✓", self.pal.green);
                        }
                    });
                });
            });
        });
    }
}

// ─── Search tab ────────────────────────────────────────────────────────────

impl App {
    fn draw_search(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, state: &SearchState) {
        let fs = self.cfg.font_size;
        let busy = *state == SearchState::Searching;
        ui.add_space(10.0);
        let mut bar_rect = egui::Rect::NOTHING;

        // Search input
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .id(egui::Id::new("q"))
                    .desired_width(ui.available_width() - 310.0)
                    .hint_text("Search torrents — movies, shows, games, software, anime…")
                    .font(FontId::proportional(fs + 2.0))
            );
            bar_rect = resp.rect;
            if resp.gained_focus() && !self.cfg.history.is_empty() { self.show_hist = true; }
            if resp.changed() && self.query.is_empty() { self.show_hist = false; }
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) { self.do_search(); }

            ui.add_space(6.0);
            egui::ComboBox::from_id_source("cat_cb")
                .selected_text(RichText::new(&self.cat).font(FontId::proportional(fs)))
                .width(115.0)
                .show_ui(ui, |ui| {
                    for &c in CATS {
                        ui.selectable_value(&mut self.cat, c.into(),
                            RichText::new(c).font(FontId::proportional(fs)));
                    }
                });

            ui.add_space(6.0);
            if ui.add_enabled(!busy,
                egui::Button::new(
                    RichText::new(if busy { "  Scanning…  " } else { "    Search    " })
                        .font(FontId::proportional(fs)).strong().color(Color32::WHITE))
                    .fill(if busy { rgb(6,100,130) } else { self.pal.accent })
                    .rounding(6.0).min_size(Vec2::new(0.0, 36.0))
            ).clicked() { self.do_search(); }

            if !self.query.is_empty() {
                if ui.add(egui::Button::new(RichText::new("✕").size(13.0).color(self.pal.sub))
                    .fill(Color32::TRANSPARENT).rounding(4.0)).on_hover_text("Clear").clicked() {
                    self.query.clear(); self.show_hist = false;
                }
            }
        });

        // History dropdown
        self.draw_history_dropdown(ctx, bar_rect, fs);

        ui.add_space(8.0);

        // Filter bar
        self.draw_filter_bar(ui, fs);

        ui.add_space(8.0);

        // State-dependent content
        match state {
            SearchState::Idle => self.draw_idle(ui),
            SearchState::Searching => {
                ui.add_space(70.0);
                ui.vertical_centered(|ui| {
                    ui.spinner();
                    ui.add_space(12.0);
                    lbl(ui, "Scanning all Jackett indexers…", self.pal.sub, 16.0);
                    ui.add_space(4.0);
                    lbl(ui, "This usually takes 10–30 seconds", self.pal.dim, fs);
                });
            }
            SearchState::Error(err) => {
                ui.add_space(10.0);
                egui::Frame::none()
                    .fill(tint(self.pal.red, 10))
                    .stroke(Stroke::new(1.0, tint(self.pal.red, 70)))
                    .rounding(8.0)
                    .inner_margin(egui::Margin::symmetric(20.0, 14.0))
                    .outer_margin(egui::Margin::symmetric(12.0, 0.0))
                    .show(ui, |ui| {
                        for line in err.lines() {
                            lbl(ui, line, self.pal.red, fs);
                        }
                        ui.add_space(8.0);
                        if outline_btn(ui, "Open Settings", self.pal.accent) {
                            self.show_settings = true;
                        }
                    });
            }
            SearchState::Done => {
                let raw = self.all_results();
                let sorted = self.filtered(&raw);
                let total = sorted.len();

                // Clamp selected index after filtering
                self.selected = self.selected.filter(|&i| i < total);
                if self.selected.is_none() {
                    self.detail_open = false;
                }

                if total == 0 {
                    ui.add_space(40.0);
                    ui.vertical_centered(|ui| {
                        lbl(ui, "No results match your filters", self.pal.sub, 17.0);
                        if !raw.is_empty() {
                            lbl(ui, &format!("{} results hidden by filters", raw.len()),
                                self.pal.dim, fs);
                        }
                    });
                    return;
                }

                let pg = self.page;
                let max_p = self.max_pages(total);
                let page_s = self.page_slice(&sorted).to_vec();
                let page_n = page_s.len();

                // Stats bar
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    let active: usize = sorted.iter().filter(|r| r.seeders.unwrap_or(0) > 0).count();
                    let seeds: u32 = sorted.iter().map(|r| r.seeders.unwrap_or(0)).sum();
                    let trackers: std::collections::HashSet<_> =
                        sorted.iter().filter_map(|r| r.tracker.as_deref()).collect();
                    lbl(ui, &format!("Showing {page_n} of {total}  ·  {active} active  ·  \
                                      {seeds} seeds  ·  {} trackers", trackers.len()),
                        self.pal.sub, fs - 1.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(12.0);
                        let sc = sorted.clone();
                        if outline_btn(ui, "Export CSV", self.pal.sub) {
                            self.export_csv(&sc);
                            self.toast("Exported to Downloads ✓", self.pal.green);
                        }
                    });
                });

                // Category chips
                if self.cfg.show_cat_bar {
                    let chips = App::cat_chips(&sorted);
                    if !chips.is_empty() {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            for (cat, count, col) in &chips {
                                let sel = self.f_text == *cat;
                                egui::Frame::none()
                                    .fill(tint(*col, if sel { 50 } else { 20 })).rounding(10.0)
                                    .stroke(Stroke::new(
                                        if sel { 1.5 } else { 1.0 },
                                        tint(*col, if sel { 200 } else { 80 })))
                                    .inner_margin(egui::Margin::symmetric(7.0, 2.0))
                                    .show(ui, |ui| {
                                        if ui.add(egui::Label::new(
                                            RichText::new(format!("{cat}  {count}"))
                                                .font(FontId::proportional(11.0)).color(*col)
                                        ).sense(egui::Sense::click()))
                                            .on_hover_text("Click to filter by category").clicked() {
                                            if self.f_text == *cat { self.f_text.clear(); }
                                            else { self.f_text = cat.clone(); }
                                        }
                                    });
                                ui.add_space(3.0);
                            }
                        });
                    }
                }
                ui.add_space(4.0);

                // Keyboard navigation (only when no text input is focused)
                let typing = ui.ctx().wants_keyboard_input();
                if !typing && ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                    self.selected = Some(self.selected.map_or(0, |s| (s + 1).min(page_n.saturating_sub(1))));
                    self.detail_open = true;
                }
                if !typing && ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                    self.selected = Some(self.selected.map_or(0, |s| s.saturating_sub(1)));
                    self.detail_open = true;
                }
                if !typing && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if let Some(i) = self.selected {
                        if let Some(r) = page_s.get(i) {
                            if let Some(m) = &r.magnet_uri {
                                let _ = open::that(m);
                                self.toast("Opening magnet…", self.pal.accent);
                            }
                        }
                    }
                }
                if !typing && ui.input(|i| i.key_pressed(egui::Key::D)) {
                    if self.selected.is_some() { self.detail_open = !self.detail_open; }
                }
                if !typing && ui.input(|i| i.key_pressed(egui::Key::F)) {
                    if let Some(i) = self.selected {
                        if let Some(r) = page_s.get(i).cloned() { self.add_fav(&r); }
                    }
                }
                if !typing && ui.input(|i| i.key_pressed(egui::Key::M)) {
                    if let Some(i) = self.selected {
                        if let Some(r) = page_s.get(i) {
                            if let Some(m) = &r.magnet_uri {
                                let _ = open::that(m);
                                self.toast("Opening magnet…", self.pal.accent);
                            }
                        }
                    }
                }

                // Pagination
                if max_p > 1 {
                    egui::TopBottomPanel::bottom("pages")
                        .exact_height(34.0)
                        .frame(egui::Frame::none().fill(self.pal.bg)
                            .stroke(Stroke::new(1.0, self.pal.border))
                            .inner_margin(egui::Margin::symmetric(12.0, 5.0)))
                        .show_inside(ui, |ui| {
                            ui.horizontal(|ui| {
                                if ui.add_enabled(pg > 0,
                                    egui::Button::new(RichText::new("← Prev")
                                        .font(FontId::proportional(fs - 1.0)).color(self.pal.sub))
                                    .fill(Color32::TRANSPARENT)
                                    .stroke(Stroke::new(1.0, self.pal.border)).rounding(4.0)
                                ).clicked() { self.page -= 1; self.selected = None; }
                                ui.add_space(6.0);
                                for p in 0..max_p {
                                    let near = p == 0 || p == max_p - 1 || p.abs_diff(pg) <= 2;
                                    if !near {
                                        if p == 1 || p == max_p - 2 {
                                            lbl(ui, "…", self.pal.dim, fs - 1.0);
                                        }
                                        continue;
                                    }
                                    let on = p == pg;
                                    if ui.add(egui::SelectableLabel::new(on,
                                        RichText::new(format!("{}", p + 1))
                                            .font(FontId::proportional(fs - 1.0))
                                            .color(if on { self.pal.accent } else { self.pal.sub })
                                    )).clicked() { self.page = p; self.selected = None; }
                                }
                                ui.add_space(6.0);
                                if ui.add_enabled(pg + 1 < max_p,
                                    egui::Button::new(RichText::new("Next →")
                                        .font(FontId::proportional(fs - 1.0)).color(self.pal.sub))
                                    .fill(Color32::TRANSPARENT)
                                    .stroke(Stroke::new(1.0, self.pal.border)).rounding(4.0)
                                ).clicked() { self.page += 1; self.selected = None; }
                                lbl(ui, &format!("  Page {} of {max_p}", pg + 1), self.pal.dim, fs - 1.0);
                            });
                        });
                }

                // Results table
                self.draw_results_table(ui, &page_s);
            }
        }
    }

    fn draw_detail_panel(&mut self, ctx: &egui::Context) {
        if !self.detail_open || self.tab != Tab::Search { return; }
        let state = self.cur_state();
        if state != SearchState::Done { return; }
        let raw = self.all_results();
        let sorted = self.filtered(&raw);
        let page_s = self.page_slice(&sorted);
        if let Some(idx) = self.selected {
            if let Some(r) = page_s.get(idx).cloned() {
                egui::SidePanel::right("detail_pnl")
                    .resizable(true).default_width(self.detail_width).min_width(240.0)
                    .frame(egui::Frame::none()
                        .fill(self.pal.surface)
                        .stroke(Stroke::new(1.0, self.pal.border))
                        .inner_margin(egui::Margin::symmetric(12.0, 8.0)))
                    .show(ctx, |ui| { self.draw_detail(ui, &r); });
            }
        }
    }
    fn draw_history_dropdown(&mut self, ctx: &egui::Context, bar_rect: egui::Rect, fs: f32) {
        if self.show_hist && !self.cfg.history.is_empty() {
            let pos = egui::pos2(bar_rect.min.x, bar_rect.max.y + 4.0);
            let w = bar_rect.width();
            let hist = self.cfg.history.clone();
            let mut clicked: Option<String> = None;
            let mut deleted: Option<String> = None;
            let mut clear_all = false;

            egui::Area::new(egui::Id::new("hist_dd"))
                .fixed_pos(pos)
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::none()
                        .fill(self.pal.surface)
                        .rounding(8.0)
                        .stroke(Stroke::new(1.0, self.pal.accent))
                        .shadow(egui::epaint::Shadow {
                            offset: [0.0, 4.0].into(),
                            blur: 12.0,
                            spread: 0.0,
                            color: rgba(0, 0, 0, 70),
                        })
                        .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                        .show(ui, |ui| {
                            ui.set_width(w.max(280.0));
                            ui.horizontal(|ui| {
                                lbl(ui, "Recent searches", self.pal.dim, 11.0);
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.add(egui::Button::new(
                                        RichText::new("clear all").size(11.0).color(self.pal.dim))
                                        .fill(Color32::TRANSPARENT).frame(false)).clicked() {
                                        clear_all = true;
                                    }
                                });
                            });
                            ui.add_space(4.0);
                            for h in hist.iter().take(10) {
                                ui.horizontal(|ui| {
                                    if ui.add(egui::Button::new(
                                        RichText::new(h.as_str()).font(FontId::proportional(fs))
                                            .color(self.pal.text))
                                        .fill(Color32::TRANSPARENT).frame(false)
                                        .min_size(egui::vec2(w.max(280.0) - 50.0, 26.0))
                                    ).clicked() { clicked = Some(h.clone()); }
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.add(egui::Button::new(
                                            RichText::new("✕").size(10.0).color(self.pal.dim))
                                            .fill(Color32::TRANSPARENT).frame(false)
                                            .min_size(egui::vec2(18.0, 18.0))
                                        ).on_hover_text("Remove").clicked() {
                                            deleted = Some(h.clone());
                                        }
                                    });
                                });
                            }
                        });
                });
            if let Some(h) = clicked { self.query = h; self.show_hist = false; self.do_search(); }
            if let Some(h) = deleted { self.cfg.history.retain(|x| x != &h); save_cfg(&self.cfg); }
            if clear_all { self.cfg.history.clear(); save_cfg(&self.cfg); self.show_hist = false; }
        }
    }

    fn draw_filter_bar(&mut self, ui: &mut egui::Ui, fs: f32) {
        egui::Frame::none()
            .fill(self.pal.surface).rounding(8.0)
            .stroke(Stroke::new(1.0, self.pal.border))
            .inner_margin(egui::Margin::symmetric(12.0, 7.0))
            .outer_margin(egui::Margin::symmetric(12.0, 0.0))
            .show(ui, |ui| {
                // Row 1
                ui.horizontal(|ui| {
                    lbl(ui, "Filter", self.pal.dim, fs);
                    ui.add_space(3.0);
                    ui.add(egui::TextEdit::singleline(&mut self.f_text)
                        .desired_width(115.0).hint_text("within results")
                        .font(FontId::proportional(fs)));
                    ui.add_space(8.0);
                    lbl(ui, "Seeds ≥", self.pal.dim, fs);
                    ui.add(egui::TextEdit::singleline(&mut self.f_seed)
                        .desired_width(38.0).hint_text("0").font(FontId::proportional(fs)));
                    ui.add_space(8.0);
                    lbl(ui, "Max GB", self.pal.dim, fs);
                    ui.add(egui::TextEdit::singleline(&mut self.f_size)
                        .desired_width(38.0).hint_text("∞").font(FontId::proportional(fs)));
                    ui.add_space(8.0);
                    lbl(ui, "Year ≥", self.pal.dim, fs);
                    ui.add(egui::TextEdit::singleline(&mut self.f_year)
                        .desired_width(44.0).hint_text("any").font(FontId::proportional(fs)));
                    ui.add_space(8.0);
                    lbl(ui, "Tracker", self.pal.dim, fs);
                    ui.add(egui::TextEdit::singleline(&mut self.f_trk)
                        .desired_width(86.0).hint_text("any").font(FontId::proportional(fs)));

                    let dirty = !self.f_text.is_empty() || !self.f_seed.is_empty()
                        || !self.f_size.is_empty() || !self.f_year.is_empty()
                        || !self.f_trk.is_empty() || self.f_hlth != Hlth::All;
                    if dirty {
                        ui.add_space(8.0);
                        if outline_btn(ui, "✕ Reset", self.pal.red) {
                            self.f_text.clear(); self.f_seed.clear(); self.f_size.clear();
                            self.f_year.clear(); self.f_trk.clear(); self.f_hlth = Hlth::All;
                            self.page = 0;
                        }
                    }
                });
                ui.add_space(5.0);
                // Row 2 — health + sort
                ui.horizontal(|ui| {
                    lbl(ui, "Health", self.pal.dim, fs);
                    ui.add_space(4.0);
                    for hf in [Hlth::All, Hlth::Hot, Hlth::Good, Hlth::Slow, Hlth::Dead] {
                        let on = self.f_hlth == hf;
                        if ui.add(egui::SelectableLabel::new(on,
                            RichText::new(hf.label()).font(FontId::proportional(fs - 1.0))
                                .color(if on { self.pal.accent } else { self.pal.sub })
                        )).clicked() { self.f_hlth = hf; self.page = 0; }
                        ui.add_space(2.0);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let d_lbl = if self.s_dir == SortDir::Desc { "▼ DESC" } else { "▲ ASC" };
                        if ui.add(egui::Button::new(
                            RichText::new(d_lbl).font(FontId::proportional(fs - 1.0)).color(self.pal.accent))
                            .fill(tint(self.pal.accent, 18))
                            .stroke(Stroke::new(1.0, tint(self.pal.accent, 60))).rounding(4.0)
                        ).on_hover_text("Toggle sort direction").clicked() {
                            self.s_dir = if self.s_dir == SortDir::Desc { SortDir::Asc } else { SortDir::Desc };
                            self.page = 0;
                        }
                        ui.add_space(6.0);
                        lbl(ui, "Sort:", self.pal.dim, fs);
                        ui.add_space(4.0);
                        for (l, col) in [("Date", SortCol::Date), ("Size", SortCol::Size),
                                         ("Leech", SortCol::Leech), ("Seeds", SortCol::Seeds),
                                         ("Tracker", SortCol::Tracker), ("Name", SortCol::Name)] {
                            let on = self.s_col == col;
                            let txt = if on {
                                if self.s_dir == SortDir::Desc { format!("{l}▼") } else { format!("{l}▲") }
                            } else { l.to_string() };
                            if ui.add(egui::SelectableLabel::new(on,
                                RichText::new(&txt).font(FontId::proportional(fs - 1.0))
                                    .color(if on { self.pal.accent } else { self.pal.sub })
                            )).clicked() {
                                if self.s_col == col {
                                    self.s_dir = if self.s_dir == SortDir::Desc { SortDir::Asc } else { SortDir::Desc };
                                } else { self.s_col = col; self.s_dir = SortDir::Desc; }
                                self.page = 0;
                            }
                            ui.add_space(2.0);
                        }
                    });
                });
            });
    }

    fn draw_results_table(&mut self, ui: &mut egui::Ui, page_s: &[TorrentResult]) {
        let mut actions: Vec<(usize, &'static str)> = vec![];
        let pal = self.pal.clone();
        let s_col = self.s_col.clone();
        let s_dir = self.s_dir.clone();
        let rh = self.cfg.row_height;
        let fsz = self.cfg.font_size;
        let cfg = self.cfg.clone();
        let sel = self.selected;
        let det_open = self.detail_open;

        let mut new_sort: Option<(SortCol, bool)> = None;

        // Table header helper
        let hdr = |l: &str, col: &SortCol| {
            let on = &s_col == col;
            let arr = if on { if s_dir == SortDir::Desc { "▼" } else { "▲" } } else { "" };
            RichText::new(format!("{l}{arr}")).font(FontId::proportional(fsz))
                .color(if on { pal.accent } else { pal.sub }).strong()
        };

        TableBuilder::new(ui)
            .striped(false)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::initial(COL_NAME_WIDTH).at_least(160.0).clip(true))
            .columns(if cfg.col_tracker { Column::initial(COL_TRACKER_WIDTH).at_least(55.0) } else { Column::remainder() }, 1)
            .columns(if cfg.col_size { Column::initial(COL_SIZE_WIDTH).at_least(50.0) } else { Column::remainder() }, 1)
            .column(Column::initial(COL_SEEDS_WIDTH).at_least(44.0)) // Seeds always
            .columns(if cfg.col_leech { Column::initial(COL_LEECH_WIDTH).at_least(44.0) } else { Column::remainder() }, 1)
            .columns(if cfg.col_ratio { Column::initial(COL_RATIO_WIDTH).at_least(44.0) } else { Column::remainder() }, 1)
            .columns(if cfg.col_health { Column::initial(COL_HEALTH_WIDTH).at_least(50.0) } else { Column::remainder() }, 1)
            .columns(if cfg.col_date { Column::initial(COL_DATE_WIDTH).at_least(60.0) } else { Column::remainder() }, 1)
            .column(Column::remainder().at_least(160.0)) // Actions always
            .header(30.0, |mut header| {
                header.col(|ui| {
                    if ui.add(egui::Label::new(hdr("Name", &SortCol::Name)).sense(egui::Sense::click())).clicked() {
                        new_sort = Some((SortCol::Name, s_col == SortCol::Name));
                    }
                });
                if cfg.col_tracker {
                    header.col(|ui| {
                        if ui.add(egui::Label::new(hdr("Tracker", &SortCol::Tracker)).sense(egui::Sense::click())).clicked() {
                            new_sort = Some((SortCol::Tracker, s_col == SortCol::Tracker));
                        }
                    });
                }
                if cfg.col_size {
                    header.col(|ui| {
                        if ui.add(egui::Label::new(hdr("Size", &SortCol::Size)).sense(egui::Sense::click())).clicked() {
                            new_sort = Some((SortCol::Size, s_col == SortCol::Size));
                        }
                    });
                }
                header.col(|ui| {
                    if ui.add(egui::Label::new(hdr("Seeds", &SortCol::Seeds)).sense(egui::Sense::click())).clicked() {
                        new_sort = Some((SortCol::Seeds, s_col == SortCol::Seeds));
                    }
                });
                if cfg.col_leech {
                    header.col(|ui| {
                        if ui.add(egui::Label::new(hdr("Leech", &SortCol::Leech)).sense(egui::Sense::click())).clicked() {
                            new_sort = Some((SortCol::Leech, s_col == SortCol::Leech));
                        }
                    });
                }
                if cfg.col_ratio {
                    header.col(|ui| {
                        ui.label(RichText::new("Ratio").font(FontId::proportional(fsz)).color(pal.sub).strong());
                    });
                }
                if cfg.col_health {
                    header.col(|ui| {
                        ui.label(RichText::new("Health").font(FontId::proportional(fsz)).color(pal.sub).strong());
                    });
                }
                if cfg.col_date {
                    header.col(|ui| {
                        if ui.add(egui::Label::new(hdr("Date", &SortCol::Date)).sense(egui::Sense::click())).clicked() {
                            new_sort = Some((SortCol::Date, s_col == SortCol::Date));
                        }
                    });
                }
                header.col(|ui| {
                    ui.label(RichText::new("Actions").font(FontId::proportional(fsz)).color(pal.sub).strong());
                });
            })
            .body(|mut body| {
                for (i, r) in page_s.iter().enumerate() {
                    let is_sel = sel == Some(i);
                    let is_hov = self.hovered == Some(i);
                    let seed = r.seeders.unwrap_or(0);
                    let leech = r.peers.unwrap_or(0);
                    let bg = if is_sel { pal.row_sel }
                             else if is_hov { pal.row_hov }
                             else if i % 2 == 0 { pal.row_odd }
                             else { pal.row_even };

                    body.row(rh, |mut row| {
                        // Name
                        row.col(|ui| {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                            let resp = ui.add(egui::Label::new(
                                RichText::new(&r.title).font(FontId::proportional(fsz))
                                    .color(if is_sel { pal.accent } else { pal.text })
                            ).truncate(true).sense(egui::Sense::click()));
                            if resp.clicked() { actions.push((i, "select")); }
                            if resp.hovered() {
                                self.hovered = Some(i);
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if rh >= 40.0 {
                                let cat = r.category_desc.as_deref().unwrap_or("Other");
                                ui.add(egui::Label::new(RichText::new(cat)
                                    .font(FontId::proportional(fsz - 2.5))
                                    .color(cat_col(cat))).truncate(true));
                            }
                        });
                        // Tracker
                        if cfg.col_tracker {
                            row.col(|ui| {
                                ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                                ui.add(egui::Label::new(RichText::new(
                                    r.tracker.as_deref().unwrap_or("—"))
                                    .font(FontId::proportional(fsz - 1.0)).color(pal.sub)).truncate(true));
                            });
                        }
                        // Size
                        if cfg.col_size {
                            row.col(|ui| {
                                ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                                ui.label(RichText::new(r.size.map(fmt_size).unwrap_or_else(||"—".into()))
                                    .font(FontId::proportional(fsz)).color(pal.sub));
                            });
                        }
                        // Seeds
                        row.col(|ui| {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                            ui.label(RichText::new(seed.to_string())
                                .font(FontId::proportional(fsz)).color(seed_col(seed)).strong());
                        });
                        // Leechers
                        if cfg.col_leech {
                            row.col(|ui| {
                                ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                                ui.label(RichText::new(leech.to_string())
                                    .font(FontId::proportional(fsz)).color(pal.red));
                            });
                        }
                        // Ratio bar
                        if cfg.col_ratio {
                            row.col(|ui| {
                                ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                                let tot = (seed + leech) as f32;
                                if tot > 0.0 {
                                    let pct = (seed as f32 / tot).clamp(0.0, 1.0);
                                    let rect = ui.available_rect_before_wrap();
                                    let bar = egui::Rect::from_min_size(
                                        rect.min + Vec2::new(2.0, (rect.height() - 7.0) / 2.0),
                                        Vec2::new((rect.width() - 4.0).max(8.0), 7.0));
                                    ui.painter().rect_filled(bar, 3.0, pal.border);
                                    let mut filled = bar;
                                    filled.max.x = bar.min.x + bar.width() * pct;
                                    ui.painter().rect_filled(filled, 3.0, seed_col(seed));
                                    ui.allocate_rect(bar, egui::Sense::hover())
                                        .on_hover_text(format!("{:.0}% seeded", pct * 100.0));
                                } else {
                                    ui.label(RichText::new("—")
                                        .font(FontId::proportional(fsz - 1.0)).color(pal.dim));
                                }
                            });
                        }
                        // Health
                        if cfg.col_health {
                            row.col(|ui| {
                                ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                                let dot = if seed > 10 { "●" } else { "○" };
                                ui.label(RichText::new(format!("{dot} {}", hlth_lbl(seed)))
                                    .font(FontId::proportional(fsz - 1.0)).strong().color(seed_col(seed)));
                            });
                        }
                        // Date
                        if cfg.col_date {
                            row.col(|ui| {
                                ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                                let d = r.publish_date.as_deref()
                                    .map(time_ago).unwrap_or_else(||"—".into());
                                ui.label(RichText::new(d)
                                    .font(FontId::proportional(fsz)).color(pal.dim));
                            });
                        }
                        // Actions
                        row.col(|ui| {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                            ui.horizontal(|ui| {
                                ui.add_space(2.0);
                                if r.magnet_uri.is_some() {
                                    if act_btn(ui, "Mag", "Open in torrent client", pal.accent) { actions.push((i, "mag")); }
                                    if act_btn(ui, "Copy", "Copy magnet link", pal.sub) { actions.push((i, "copy")); }
                                }
                                if r.link.is_some() {
                                    if act_btn(ui, "DL", "Download .torrent", pal.green) { actions.push((i, "dl")); }
                                }
                                if act_btn(ui, "Fav", "Add to Favorites (F)", pal.yellow) { actions.push((i, "fav")); }
                                if act_btn(ui, "Info", "Detail panel (D)",
                                    if is_sel && det_open { pal.accent } else { pal.dim }) { actions.push((i, "info")); }
                                if r.details.is_some() {
                                    if act_btn(ui, "Web", "Open in browser", pal.dim) { actions.push((i, "web")); }
                                }
                            });
                        });
                    });
                }
            });

        if let Some((col, same)) = new_sort {
            if same {
                self.s_dir = if self.s_dir == SortDir::Desc { SortDir::Asc } else { SortDir::Desc };
            } else { self.s_col = col; self.s_dir = SortDir::Desc; }
            self.page = 0;
        }

        // Process actions
        for (i, action) in actions {
            if action == "hover" { continue; } // already handled
            if let Some(r) = page_s.get(i).cloned() {
                match action {
                    "select" => {
                        if self.selected == Some(i) && self.detail_open {
                            self.selected = None; self.detail_open = false;
                        } else { self.selected = Some(i); self.detail_open = true; }
                    }
                    "mag" => { if let Some(m) = &r.magnet_uri { let _ = open::that(m); self.toast("Opening magnet…", self.pal.accent); } }
                    "copy" => { if let Some(m) = &r.magnet_uri { ui.output_mut(|o| o.copied_text = m.clone()); self.toast("Magnet copied ✓", self.pal.green); } }
                    "dl" => { if let Some(l) = &r.link { let _ = open::that(l); self.toast("Downloading…", self.pal.green); } }
                    "fav" => { self.add_fav(&r); }
                    "info" => {
                        if self.selected == Some(i) && self.detail_open {
                            self.detail_open = false; self.selected = None;
                        } else { self.selected = Some(i); self.detail_open = true; }
                    }
                    "web" => { if let Some(d) = &r.details { let _ = open::that(d); } }
                    _ => {}
                }
            }
        }

        // Clear hover when mouse leaves the table area
        if let Some(hover_pos) = ui.ctx().pointer_hover_pos() {
            if !ui.min_rect().contains(hover_pos) {
                self.hovered = None;
            }
        } else {
            self.hovered = None;
        }
    }

    // ─── Idle / welcome ────────────────────────────────────────────────────

    fn draw_idle(&mut self, ui: &mut egui::Ui) {
        let fs = self.cfg.font_size;
        ui.add_space(50.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("TorrentX")
                .font(FontId::proportional(40.0)).strong().color(tint(self.pal.accent, 90)));
            ui.add_space(6.0);
            lbl(ui, "Search all your Jackett indexers in one shot", self.pal.sub, fs + 1.0);
            ui.add_space(3.0);
            lbl(ui, "Movies  ·  TV  ·  Music  ·  Games  ·  Software  ·  Anime  ·  Books",
                self.pal.dim, fs - 1.0);
            ui.add_space(32.0);

            if !self.cfg.history.is_empty() {
                lbl(ui, "Recent searches", self.pal.dim, fs - 1.0);
                ui.add_space(10.0);
                let hist: Vec<String> = self.cfg.history.iter().take(12).cloned().collect();
                let mut clicked: Option<String> = None;
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                    for h in &hist {
                        if ui.add(egui::Button::new(
                            RichText::new(h.as_str()).font(FontId::proportional(fs)).color(self.pal.sub))
                            .fill(self.pal.surface)
                            .stroke(Stroke::new(1.0, self.pal.border))
                            .rounding(14.0).min_size(egui::vec2(0.0, 28.0))
                        ).clicked() { clicked = Some(h.clone()); }
                    }
                });
                if let Some(h) = clicked { self.query = h; self.do_search(); }
            } else {
                lbl(ui, "Try searching:", self.pal.dim, fs - 1.0);
                ui.add_space(10.0);
                let suggestions = ["Linux Mint", "Ubuntu 24.04", "Blender", "GIMP", "Inkscape"];
                let mut clicked: Option<&str> = None;
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                    for s in &suggestions {
                        if ui.add(egui::Button::new(
                            RichText::new(*s).font(FontId::proportional(fs)).color(self.pal.dim))
                            .fill(self.pal.surface)
                            .stroke(Stroke::new(1.0, tint(self.pal.border, 140)))
                            .rounding(14.0).min_size(egui::vec2(0.0, 28.0))
                        ).clicked() { clicked = Some(s); }
                    }
                });
                if let Some(s) = clicked { self.query = s.to_string(); self.do_search(); }

                ui.add_space(32.0);
                egui::Frame::none()
                    .fill(tint(self.pal.accent, 12)).rounding(10.0)
                    .stroke(Stroke::new(1.0, tint(self.pal.accent, 50)))
                    .inner_margin(egui::Margin::symmetric(24.0, 16.0))
                    .show(ui, |ui| {
                        ui.set_max_width(480.0);
                        ui.label(RichText::new("First time?")
                            .font(FontId::proportional(fs + 1.0)).color(self.pal.accent).strong());
                        ui.add_space(6.0);
                        lbl(ui, "1. Make sure Jackett is running  (localhost:9117)", self.pal.sub, fs - 1.0);
                        lbl(ui, "2. Click ⚙ Settings and paste your API key", self.pal.sub, fs - 1.0);
                        lbl(ui, "3. Search for anything!", self.pal.sub, fs - 1.0);
                        ui.add_space(10.0);
                        if outline_btn(ui, "Open Settings", self.pal.accent) {
                            self.show_settings = true;
                        }
                    });
            }
        });
    }

    // ─── Detail panel ──────────────────────────────────────────────────────

    fn draw_detail(&mut self, ui: &mut egui::Ui, r: &TorrentResult) {
        let fs = self.cfg.font_size;
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            lbl(ui, "Details", self.pal.text, fs + 2.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                if ui.add(egui::Button::new(RichText::new("✕").size(14.0).color(self.pal.sub))
                    .fill(Color32::TRANSPARENT).rounding(4.0))
                    .on_hover_text("Close").clicked() {
                    self.detail_open = false; self.selected = None;
                }
            });
        });
        ui.separator();
        ui.add_space(8.0);

        egui::ScrollArea::vertical().id_source("det_scr").show(ui, |ui| {
            ui.add(egui::Label::new(
                RichText::new(&r.title).font(FontId::proportional(fs)).color(self.pal.text).strong()
            ).wrap(true));
            ui.add_space(8.0);

            let cat = r.category_desc.as_deref().unwrap_or("Unknown");
            egui::Frame::none()
                .fill(tint(cat_col(cat), 25)).rounding(8.0)
                .inner_margin(egui::Margin::symmetric(8.0, 3.0))
                .show(ui, |ui| {
                    ui.label(RichText::new(cat).font(FontId::proportional(fs - 1.0)).color(cat_col(cat)));
                });
            ui.add_space(12.0);

            // Use grid for aligned details
            egui::Grid::new("detail_grid")
                .num_columns(2)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    let seed = r.seeders.unwrap_or(0);
                    let leech = r.peers.unwrap_or(0);

                    if let Some(t) = &r.tracker { grid_row(ui, "Tracker", t, self.pal.sub, &self.pal, fs); }
                    if let Some(s) = r.size { grid_row(ui, "Size", &fmt_size(s), self.pal.sub, &self.pal, fs); }
                    grid_row(ui, "Seeders", &seed.to_string(), seed_col(seed), &self.pal, fs);
                    grid_row(ui, "Leechers", &leech.to_string(), self.pal.red, &self.pal, fs);
                    let ratio = if leech > 0 { format!("{:.2}", seed as f64 / leech as f64) } else { "∞".into() };
                    grid_row(ui, "Ratio", &ratio, self.pal.sub, &self.pal, fs);
                    grid_row(ui, "Health", hlth_lbl(seed), seed_col(seed), &self.pal, fs);
                    if let Some(d) = &r.publish_date { grid_row(ui, "Published", &time_ago(d), self.pal.dim, &self.pal, fs); }
                });

            let seed = r.seeders.unwrap_or(0);
            let leech = r.peers.unwrap_or(0);
            let tot = (seed + leech) as f32;
            if tot > 0.0 {
                let ratio_value = if leech > 0 {
                    format!("{:.2}", seed as f64 / leech as f64)
                } else {
                    "∞".into()
                };
                let pct = (seed as f32 / tot).clamp(0.0, 1.0);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("Ratio: {}  ", ratio_value))
                        .font(FontId::proportional(fs - 1.0)).color(self.pal.sub));
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width() - 60.0, 8.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 4.0, self.pal.border);
                    let mut filled = rect;
                    filled.max.x = rect.min.x + rect.width() * pct;
                    ui.painter().rect_filled(filled, 4.0, seed_col(seed));
                });
                ui.add_space(2.0);
                lbl(ui, &format!("{:.0}% seeded", pct * 100.0), self.pal.dim, fs - 2.0);
            }

            // Show magnet link (truncated) if present
            if let Some(mag) = &r.magnet_uri {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Magnet:").font(FontId::proportional(fs-1.5)).color(self.pal.dim));
                    let truncated = if mag.len() > 60 {
                        format!("{}…", &mag[..57])
                    } else {
                        mag.clone()
                    };
                    let resp = ui.add(egui::Label::new(
                        RichText::new(truncated).font(FontId::monospace(fs-2.0)).color(self.pal.sub))
                        .sense(egui::Sense::click()));
                    if resp.on_hover_text("Click to copy full magnet").clicked() {
                        ui.output_mut(|o| o.copied_text = mag.clone());
                        self.toast("Magnet copied ✓", self.pal.green);
                    }
                });
            }

            ui.add_space(16.0);
            lbl(ui, "Actions", self.pal.dim, fs - 1.0);
            ui.add_space(6.0);

            if let Some(mag) = r.magnet_uri.clone() {
                let mc = mag.clone();
                if wide_btn(ui, "▶  Open Magnet", self.pal.accent) {
                    let _ = open::that(mag); self.toast("Opening magnet…", self.pal.accent);
                }
                ui.add_space(4.0);
                if wide_btn(ui, "⎘  Copy Magnet Link", self.pal.sub) {
                    ui.output_mut(|o| o.copied_text = mc);
                    self.toast("Copied ✓", self.pal.green);
                }
                ui.add_space(4.0);
            }
            if let Some(link) = r.link.clone() {
                if wide_btn(ui, "↓  Download .torrent", self.pal.green) {
                    let _ = open::that(link); self.toast("Downloading…", self.pal.green);
                }
                ui.add_space(4.0);
            }
            if let Some(det) = r.details.clone() {
                if wide_btn(ui, "🌐  Open in Browser", self.pal.sub) { let _ = open::that(det); }
                ui.add_space(4.0);
            }
            let r2 = r.clone();
            if wide_btn(ui, "★  Add to Favorites", self.pal.yellow) { self.add_fav(&r2); }
        });
    }

    // ─── Favorites tab ─────────────────────────────────────────────────────

    fn draw_favorites(&mut self, ui: &mut egui::Ui) {
        let fs = self.cfg.font_size;
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label(RichText::new(format!("Favorites  ({})", self.cfg.favorites.len()))
                .font(FontId::proportional(18.0)).color(self.pal.text).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(14.0);
                if !self.cfg.favorites.is_empty() {
                    if outline_btn(ui, "Clear all", self.pal.red) {
                        self.cfg.favorites.clear(); save_cfg(&self.cfg);
                    }
                }
            });
        });

        if self.cfg.favorites.is_empty() {
            ui.add_space(60.0);
            ui.vertical_centered(|ui| {
                lbl(ui, "No favorites yet", self.pal.sub, 20.0);
                ui.add_space(6.0);
                lbl(ui, "Click Fav on any result, or press F when a row is selected",
                    self.pal.dim, fs);
            });
            return;
        }

        // Search box
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            lbl(ui, "Search:", self.pal.dim, fs);
            ui.add_space(4.0);
            ui.add(egui::TextEdit::singleline(&mut self.fav_search)
                .desired_width(220.0).hint_text("filter favorites…")
                .font(FontId::proportional(fs)));
            if !self.fav_search.is_empty() {
                if ui.add(egui::Button::new(RichText::new("✕").size(12.0).color(self.pal.sub))
                    .fill(Color32::TRANSPARENT).frame(false)).clicked() {
                    self.fav_search.clear();
                }
            }
        });
        ui.add_space(8.0);

        let mut remove: Option<usize> = None;
        let mut open_mag: Option<String> = None;
        let mut open_link: Option<String> = None;
        let fq = self.fav_search.to_lowercase();

        egui::ScrollArea::vertical().show(ui, |ui| {
            let favs = self.cfg.favorites.clone();
            let mut row_i = 0usize;
            for (i, fav) in favs.iter().enumerate() {
                if !fq.is_empty()
                    && !fav.title.to_lowercase().contains(&fq)
                    && !fav.tracker.as_deref().unwrap_or("").to_lowercase().contains(&fq)
                { continue; }
                row_i += 1;
                let bg = if row_i % 2 == 0 { self.pal.row_odd } else { self.pal.row_even };
                egui::Frame::none()
                    .fill(bg).inner_margin(egui::Margin::symmetric(16.0, 10.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.set_min_width(ui.available_width() - 130.0);
                                ui.add(egui::Label::new(
                                    RichText::new(&fav.title).font(FontId::proportional(fs))
                                        .color(self.pal.text)).truncate(true));
                                ui.horizontal(|ui| {
                                    if let Some(t) = &fav.tracker {
                                        lbl(ui, t, self.pal.sub, fs - 1.5);
                                    }
                                    if let Some(s) = fav.size {
                                        lbl(ui, &format!("·  {}", fmt_size(s)), self.pal.dim, fs - 1.5);
                                    }
                                    if let Some(s) = fav.seeders {
                                        lbl(ui, &format!("·  {} seeds", s), seed_col(s), fs - 1.5);
                                    }
                                    if !fav.saved_at.is_empty() {
                                        lbl(ui, &format!("·  saved {}", fav.saved_at),
                                            self.pal.dim, fs - 2.0);
                                    }
                                });
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if act_btn(ui, "Del", "Remove", self.pal.red) { remove = Some(i); }
                                if fav.link.is_some() {
                                    if act_btn(ui, "DL", "Download .torrent", self.pal.green) { open_link = fav.link.clone(); }
                                }
                                if fav.magnet.is_some() {
                                    if act_btn(ui, "Mag", "Open magnet", self.pal.accent) { open_mag = fav.magnet.clone(); }
                                }
                            });
                        });
                    });
                ui.separator();
            }
        });

        if let Some(i) = remove { self.cfg.favorites.remove(i); save_cfg(&self.cfg); }
        if let Some(m) = open_mag { let _ = open::that(m); self.toast("Opening magnet…", self.pal.accent); }
        if let Some(l) = open_link { let _ = open::that(l); self.toast("Downloading…", self.pal.green); }
    }

    // ─── RSS Tab ────────────────────────────────────────────────────────

    fn draw_rss(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        let pal = self.pal.clone();
        let fs = self.cfg.font_size;

        if self.rss_feeds.is_empty() && !self.rss_add_mode {
            ui.add_space(60.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("📡").size(42.0));
                ui.add_space(12.0);
                lbl(ui, "No RSS Feeds yet", pal.sub, 18.0);
                ui.add_space(6.0);
                lbl(ui, "Add Jackett Torznab feeds to auto-refresh torrents", pal.dim, fs);
                ui.add_space(20.0);
                egui::Frame::none()
                    .fill(tint(pal.accent, 12)).rounding(10.0)
                    .stroke(Stroke::new(1.0, tint(pal.accent, 50)))
                    .inner_margin(egui::Margin::symmetric(24.0, 16.0))
                    .show(ui, |ui| {
                        ui.set_max_width(420.0);
                        lbl(ui, "How Torznab RSS works", pal.accent, fs);
                        ui.add_space(6.0);
                        lbl(ui, "Each indexer in Jackett exposes a Torznab API.", pal.sub, fs - 1.0);
                        lbl(ui, "You can search any indexer and get live results", pal.sub, fs - 1.0);
                        lbl(ui, "as an auto-refreshed feed — no browser needed.", pal.sub, fs - 1.0);
                        ui.add_space(12.0);
                        if outline_btn(ui, "+ Add Feed", pal.accent) { self.rss_add_mode = true; }
                    });
            });
            return;
        }

        ui.horizontal_top(|ui| {
            egui::SidePanel::left("rss_sidebar")
                .resizable(true).default_width(220.0).min_width(160.0)
                .frame(egui::Frame::none().fill(pal.surface).stroke(Stroke::new(1.0, pal.border)))
                .show_inside(ui, |ui| { self.draw_rss_sidebar(ui); });

            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(pal.bg))
                .show_inside(ui, |ui| {
                    if self.rss_add_mode { self.draw_rss_form(ui, None); }
                    else if let Some(idx) = self.rss_edit_idx { self.draw_rss_form(ui, Some(idx)); }
                    else { self.draw_rss_items(ui); }
                });
        });
    }

    fn draw_rss_sidebar(&mut self, ui: &mut egui::Ui) {
        let pal = self.pal.clone(); let fs = self.cfg.font_size;
        egui::Frame::none()
            .fill(pal.hdr).stroke(Stroke::new(1.0, pal.border))
            .inner_margin(egui::Margin::symmetric(10.0, 8.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    lbl(ui, "RSS Feeds", pal.accent, fs);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let sub = pal.sub;
                        if ui.add(egui::Button::new(RichText::new("⟳ All").font(FontId::proportional(fs - 1.5)).color(sub))
                            .fill(Color32::TRANSPARENT).stroke(Stroke::new(1.0, pal.border)).rounding(4.0)
                        ).on_hover_text("Refresh all feeds").clicked() { self.refresh_all_feeds(); }
                    });
                });
                ui.add_space(4.0);
                ui.add(egui::TextEdit::singleline(&mut self.rss_filter)
                    .desired_width(ui.available_width()).hint_text("Filter feeds…").font(FontId::proportional(fs)));
            });

        egui::ScrollArea::vertical().id_source("rss_feed_list").show(ui, |ui| {
            let filter = self.rss_filter.to_lowercase();
            let len = self.rss_feeds.len();
            let mut sel: Option<usize> = None;
            let mut del: Option<usize> = None;
            let mut ed: Option<usize> = None;
            let mut refr: Option<usize> = None;

            for i in 0..len {
                let name = self.rss_feeds[i].config.name.clone();
                let n = self.rss_feeds[i].items.len();
                let st = self.rss_feeds[i].status.clone();
                let en = self.rss_feeds[i].config.enabled;
                if !filter.is_empty() && !name.to_lowercase().contains(&filter) { continue; }

                let is_sel = self.rss_selected == i && !self.rss_add_mode && self.rss_edit_idx.is_none();
                let bg = if is_sel { tint(pal.accent, 22) } else { Color32::TRANSPARENT };

                egui::Frame::none().fill(bg).rounding(6.0)
                    .inner_margin(egui::Margin::symmetric(10.0, 7.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (dc, dot) = match st {
                                FeedStatus::Ok => (pal.green, "●"), FeedStatus::Loading => (pal.accent, "⟳"),
                                FeedStatus::Error => (pal.red, "✕"), FeedStatus::Idle => (pal.dim, "○"),
                            };
                            lbl(ui, dot, dc, fs - 2.0);
                            ui.add_space(4.0);
                            let nc = if en { pal.text } else { pal.dim };
                            if ui.add(egui::Label::new(RichText::new(&name).font(FontId::proportional(fs - 0.5)).color(nc))
                                .truncate(true).sense(egui::Sense::click())).clicked() { sel = Some(i); }
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if n > 0 {
                                    egui::Frame::none().fill(tint(pal.accent, 25)).rounding(8.0)
                                        .inner_margin(egui::Margin::symmetric(5.0, 1.0))
                                        .show(ui, |ui| { ui.label(RichText::new(n.to_string()).font(FontId::proportional(fs - 3.0)).color(pal.accent)); });
                                }
                            });
                        });
                        if is_sel {
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                if act_btn(ui, "⟳", "Refresh", pal.accent) { refr = Some(i); }
                                if act_btn(ui, "Edit", "Edit feed", pal.sub) { ed = Some(i); }
                                if act_btn(ui, "✕", "Delete feed", pal.red) { del = Some(i); }
                                let ec = if en { pal.green } else { pal.dim };
                                let el = if en { "On" } else { "Off" };
                                if act_btn(ui, el, "Toggle enabled", ec) {
                                    self.rss_feeds[i].config.enabled = !en; self.sync_rss_configs();
                                }
                            });
                        }
                    });
            }
            if let Some(i) = sel  { self.rss_selected = i; self.rss_add_mode = false; self.rss_edit_idx = None; self.rss_detail = None; }
            if let Some(i) = refr { self.refresh_feed(i); }
            if let Some(i) = del  { self.rss_feeds.remove(i); if self.rss_selected >= self.rss_feeds.len() && !self.rss_feeds.is_empty() { self.rss_selected = self.rss_feeds.len() - 1; } self.sync_rss_configs(); }
            if let Some(i) = ed   { self.rss_edit_idx = Some(i); self.rss_add_mode = false; }
        });

        ui.add_space(8.0);
        egui::Frame::none().inner_margin(egui::Margin::symmetric(10.0, 6.0)).show(ui, |ui| {
            if wide_btn(ui, "+ Add Feed", pal.accent) { self.rss_add_mode = true; self.rss_edit_idx = None; self.rss_new_cfg = RssFeedConfig::new_default(); }
        });
    }

    fn draw_rss_items(&mut self, ui: &mut egui::Ui) {
        let pal = self.pal.clone(); let fs = self.cfg.font_size; let rh = self.cfg.row_height;
        let sel = self.rss_selected;
        if self.rss_feeds.is_empty() || sel >= self.rss_feeds.len() { return; }

        let name = self.rss_feeds[sel].config.name.clone();
        let status = self.rss_feeds[sel].status.clone();
        let items = self.rss_feeds[sel].items.clone();
        let err = self.rss_feeds[sel].error.clone();

        egui::Frame::none().fill(pal.surface).stroke(Stroke::new(1.0, pal.border))
            .inner_margin(egui::Margin::symmetric(14.0, 8.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    lbl(ui, &name, pal.accent, fs + 1.0); ui.add_space(8.0);
                    let (dc, dl) = match status {
                        FeedStatus::Ok => (pal.green, "● OK"), FeedStatus::Loading => (pal.accent, "⟳ Loading"),
                        FeedStatus::Error => (pal.red, "✕ Error"), FeedStatus::Idle => (pal.dim, "○ Idle"),
                    };
                    status_pill(ui, dl, dc);
                    lbl(ui, &format!("  {} items", items.len()), pal.dim, fs - 1.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if outline_btn(ui, "⟳ Refresh", pal.accent) { self.refresh_feed(sel); }
                        ui.add_space(6.0);
                        if outline_btn(ui, "Edit Feed", pal.sub) { self.rss_edit_idx = Some(sel); }
                    });
                });
                if let Some(e) = &err { ui.add_space(4.0); lbl(ui, &format!("Error: {e}"), pal.red, fs - 1.0); }
            });

        if status == FeedStatus::Loading && items.is_empty() {
            ui.add_space(40.0); ui.vertical_centered(|ui| { ui.spinner(); ui.add_space(10.0); lbl(ui, "Fetching Torznab feed…", pal.sub, fs); });
            return;
        }
        if items.is_empty() {
            ui.add_space(40.0); ui.vertical_centered(|ui| { lbl(ui, "No items yet", pal.dim, fs + 2.0); ui.add_space(8.0); lbl(ui, "Click Refresh to fetch the latest torrents", pal.sub, fs); });
            return;
        }

        if let Some(di) = self.rss_detail {
            if let Some(item) = items.get(di).cloned() {
                egui::SidePanel::right("rss_detail_pnl")
                    .resizable(true).default_width(280.0).min_width(220.0)
                    .frame(egui::Frame::none().fill(pal.surface).stroke(Stroke::new(1.0, pal.border)))
                    .show_inside(ui, |ui| { self.draw_rss_item_detail(ui, &item); });
            }
        }

        use egui_extras::{Column, TableBuilder};
        let mut actions: Vec<(usize, &'static str)> = vec![];
        ui.add_space(2.0);
        TableBuilder::new(ui).striped(false).resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::remainder().at_least(180.0).clip(true))
            .column(Column::initial(80.0).at_least(50.0))
            .column(Column::initial(60.0).at_least(44.0))
            .column(Column::initial(60.0).at_least(44.0))
            .column(Column::initial(80.0).at_least(60.0))
            .column(Column::initial(180.0).at_least(120.0))
            .header(28.0, |mut hdr| {
                for label in ["Title", "Tracker", "Size", "Seeds", "Date", "Actions"] {
                    hdr.col(|ui| { ui.label(RichText::new(label).font(FontId::proportional(fs - 1.0)).color(pal.sub).strong()); });
                }
            })
            .body(|mut body| {
                for (i, item) in items.iter().enumerate() {
                    let is_sel = self.rss_detail == Some(i);
                    let bg = if is_sel { tint(pal.accent, 20) } else if i % 2 == 0 { pal.row_odd } else { pal.row_even };
                    body.row(rh, |mut row| {
                        row.col(|ui| {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                            let resp = ui.add(egui::Label::new(RichText::new(&item.title).font(FontId::proportional(fs)).color(if is_sel { pal.accent } else { pal.text }))
                                .truncate(true).sense(egui::Sense::click()));
                            if resp.clicked() { actions.push((i, "detail")); }
                        });
                        row.col(|ui| {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                            ui.add(egui::Label::new(RichText::new(item.tracker.as_deref().unwrap_or("—")).font(FontId::proportional(fs - 1.0)).color(pal.sub)).truncate(true));
                        });
                        row.col(|ui| {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                            ui.label(RichText::new(item.size.map(fmt_size).unwrap_or_else(|| "—".into())).font(FontId::proportional(fs)).color(pal.sub));
                        });
                        row.col(|ui| {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                            let s = item.seeders.unwrap_or(0);
                            ui.label(RichText::new(s.to_string()).font(FontId::proportional(fs)).color(seed_col(s)).strong());
                        });
                        row.col(|ui| {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                            let d = item.pub_date.as_deref().map(time_ago).unwrap_or_else(|| "—".into());
                            ui.label(RichText::new(d).font(FontId::proportional(fs)).color(pal.dim));
                        });
                        row.col(|ui| {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                            ui.horizontal(|ui| {
                                if item.magnet.is_some() {
                                    if act_btn(ui, "Mag", "Open magnet", pal.accent) { actions.push((i, "mag")); }
                                    if act_btn(ui, "Copy", "Copy magnet link", pal.sub) { actions.push((i, "copy")); }
                                }
                                if item.link.is_some() { if act_btn(ui, "DL", "Download .torrent", pal.green) { actions.push((i, "dl")); } }
                                if act_btn(ui, "★", "Add to Favorites", pal.yellow) { actions.push((i, "fav")); }
                            });
                        });
                    });
                }
            });

        for (i, action) in actions {
            if let Some(item) = items.get(i).cloned() {
                match action {
                    "detail" => { self.rss_detail = if self.rss_detail == Some(i) { None } else { Some(i) }; }
                    "mag" => { if let Some(m) = &item.magnet { let _ = open::that(m); self.toast("Opening magnet…", pal.accent); } }
                    "copy" => { if let Some(m) = &item.magnet { ui.output_mut(|o| o.copied_text = m.clone()); self.toast("Magnet copied ✓", pal.green); } }
                    "dl" => { if let Some(l) = &item.link { let _ = open::that(l); } }
                    "fav" => { self.add_fav_from_rss(&item); }
                    _ => {}
                }
            }
        }
    }

    fn draw_rss_item_detail(&mut self, ui: &mut egui::Ui, item: &RssItem) {
        let pal = self.pal.clone(); let fs = self.cfg.font_size;
        ui.add_space(10.0);
        egui::Frame::none().inner_margin(egui::Margin::symmetric(12.0, 0.0)).show(ui, |ui| {
            ui.add(egui::Label::new(RichText::new(&item.title).font(FontId::proportional(fs)).color(pal.text).strong()).wrap(true));
            ui.add_space(12.0);
            egui::Grid::new("rss_item_grid").num_columns(2).spacing([8.0, 5.0]).show(ui, |ui| {
                if let Some(t) = &item.tracker { grid_row(ui, "Tracker", t, pal.accent, &pal, fs); }
                if let Some(s) = item.size { grid_row(ui, "Size", &fmt_size(s), pal.text, &pal, fs); }
                if let Some(s) = item.seeders { grid_row(ui, "Seeders", &s.to_string(), seed_col(s), &pal, fs); }
                if let Some(l) = item.leechers { grid_row(ui, "Leechers", &l.to_string(), pal.red, &pal, fs); }
                if let Some(d) = &item.pub_date { grid_row(ui, "Published", &time_ago(d), pal.text, &pal, fs); }
                if let Some(c) = &item.category { grid_row(ui, "Category", c, pal.text, &pal, fs); }
            });
            ui.add_space(12.0);
            if item.magnet.is_some() {
                if wide_btn(ui, "⚡  Open Magnet", pal.accent) { if let Some(m) = &item.magnet { let _ = open::that(m); } }
                ui.add_space(4.0);
                if wide_btn(ui, "⎘  Copy Magnet", pal.sub) { if let Some(m) = &item.magnet { ui.output_mut(|o| o.copied_text = m.clone()); self.toast("Copied ✓", pal.green); } }
                ui.add_space(4.0);
            }
            if item.link.is_some() { if wide_btn(ui, "↓  Download .torrent", pal.green) { if let Some(l) = &item.link { let _ = open::that(l); } } ui.add_space(4.0); }
            if wide_btn(ui, "★  Save to Favorites", pal.yellow) { let it = item.clone(); self.add_fav_from_rss(&it); }
        });
    }

    fn draw_rss_form(&mut self, ui: &mut egui::Ui, edit_idx: Option<usize>) {
        let pal = self.pal.clone(); let fs = self.cfg.font_size;
        let is_edit = edit_idx.is_some();

        if is_edit && self.rss_new_cfg.name.is_empty() {
            if let Some(idx) = edit_idx {
                if let Some(feed) = self.rss_feeds.get(idx) { self.rss_new_cfg = feed.config.clone(); }
            }
        }

        let title = if is_edit { "Edit Feed" } else { "Add New RSS Feed" };
        ui.add_space(20.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(500.0);
            lbl(ui, title, pal.accent, fs + 3.0); ui.add_space(4.0);
            lbl(ui, "Connects to a Jackett Torznab indexer endpoint", pal.dim, fs - 1.0);
            ui.add_space(20.0);

            ui.horizontal(|ui| { ui.add_space(4.0); lbl(ui, "Name:", pal.dim, fs); ui.add_space(4.0);
                ui.add(egui::TextEdit::singleline(&mut self.rss_new_cfg.name)
                    .desired_width(260.0).hint_text("My Feed").font(FontId::proportional(fs))); });
            ui.add_space(6.0);
            ui.horizontal(|ui| { ui.add_space(4.0); lbl(ui, "Indexer:", pal.dim, fs); ui.add_space(4.0);
                ui.add(egui::TextEdit::singleline(&mut self.rss_new_cfg.indexer)
                    .desired_width(260.0).hint_text("all (Jackett slug)").font(FontId::proportional(fs))); });
            ui.add_space(6.0);
            ui.horizontal(|ui| { ui.add_space(4.0); lbl(ui, "Query:", pal.dim, fs); ui.add_space(4.0);
                ui.add(egui::TextEdit::singleline(&mut self.rss_new_cfg.query)
                    .desired_width(260.0).hint_text("empty = latest torrents").font(FontId::proportional(fs))); });
            ui.add_space(6.0);
            ui.horizontal(|ui| { ui.add_space(4.0); lbl(ui, "Category:", pal.dim, fs); ui.add_space(4.0);
                ui.add(egui::TextEdit::singleline(&mut self.rss_new_cfg.category)
                    .desired_width(260.0).hint_text("Torznab cat numbers").font(FontId::proportional(fs))); });
            ui.add_space(10.0);

            ui.horizontal(|ui| {
                ui.checkbox(&mut self.rss_new_cfg.enabled, "");
                lbl(ui, "Enabled", pal.text, fs);
                ui.add_space(20.0);
                ui.checkbox(&mut self.rss_new_cfg.auto_refresh, "");
                lbl(ui, "Auto-refresh", pal.text, fs);
            });
            ui.add_space(16.0);

            ui.horizontal(|ui| {
                if outline_btn(ui, "Cancel", pal.red) { self.rss_add_mode = false; self.rss_edit_idx = None; }
                ui.add_space(12.0);
                if wide_btn(ui, "Save", pal.accent) {
                    if is_edit {
                        if let Some(idx) = edit_idx { self.rss_feeds[idx].config = self.rss_new_cfg.clone(); }
                    } else {
                        self.rss_feeds.push(RssFeedState::new(self.rss_new_cfg.clone()));
                        self.rss_selected = self.rss_feeds.len() - 1;
                    }
                    self.sync_rss_configs();
                    self.rss_add_mode = false; self.rss_edit_idx = None;
                    self.rss_new_cfg = RssFeedConfig::new_default();
                }
            });
        });
    }

    // ─── About tab ─────────────────────────────────────────────────────────

    fn draw_about(&self, ui: &mut egui::Ui) {
        let fs = self.cfg.font_size;
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("TorrentX")
                    .font(FontId::proportional(30.0)).color(self.pal.text).strong());
                ui.label(RichText::new("v17.0").font(FontId::proportional(fs)).color(self.pal.accent));
                ui.add_space(4.0);
                lbl(ui, "Native Rust + egui torrent search GUI powered by Jackett",
                    self.pal.sub, fs + 1.0);

                ui.add_space(24.0);
                for (k, v) in [
                    ("Language", "Rust 2021 edition"),
                    ("GUI", "egui 0.27 + egui_extras"),
                    ("Rendering", "GPU via wgpu / OpenGL (eframe)"),
                    ("HTTP", "reqwest (blocking)"),
                    ("Config", "~/.config/torrentx/config.json"),
                ] {
                    ui.horizontal(|ui| {
                        ui.add_space(ui.available_width() * 0.15);
                        lbl(ui, &format!("{k:<18}"), self.pal.dim, fs);
                        lbl(ui, v, self.pal.sub, fs);
                    });
                    ui.add_space(2.0);
                }

                // Theme swatches
                ui.add_space(24.0);
                lbl(ui, "19 Themes", self.pal.accent, fs + 1.0);
                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                    ui.add_space(40.0);
                    for t in Theme::all() {
                        let col = t.accent_color();
                        let active = &self.cfg.theme == t;
                        egui::Frame::none()
                            .fill(tint(col, if active { 45 } else { 20 })).rounding(6.0)
                            .stroke(Stroke::new(
                                if active { 2.0 } else { 1.0 },
                                tint(col, if active { 220 } else { 90 })))
                            .inner_margin(egui::Margin::symmetric(9.0, 4.0))
                            .show(ui, |ui| {
                                ui.label(RichText::new(t.name())
                                    .font(FontId::proportional(fs - 1.5)).color(col));
                            });
                    }
                });

                // Features
                ui.add_space(24.0);
                lbl(ui, "Features", self.pal.accent, fs + 1.0);
                ui.add_space(8.0);
                for f in [
                    "Search all Jackett indexers simultaneously",
                    "19 themes — 16 dark, 3 light — instant switching",
                    "Toggle columns: Tracker, Size, Leech, Ratio, Health, Date",
                    "Row density: Compact / Normal / Roomy",
                    "Font size: Small / Medium / Large",
                    "Filter by text, seeds, size, year, tracker, health status",
                    "Sort by Name, Tracker, Size, Seeds, Leechers, Date",
                    "Hover highlight per theme + selected row highlight",
                    "Animated spinner with elapsed time",
                    "Clickable category chips to filter by category",
                    "Search history with per-item delete",
                    "Favorites with search filter, timestamps, persistent storage",
                    "Detail side panel with seeder/leecher ratio bar",
                    "Deduplication across trackers",
                    "Export filtered results to CSV",
                    "Pagination: 25 / 50 / 100 / All per page",
                    "Keyboard nav: ↑↓ Enter D F M Ctrl+F Ctrl+R Esc",
                ] {
                    lbl(ui, &format!("  ·  {f}"), self.pal.sub, fs - 1.0);
                    ui.add_space(1.0);
                }

                // Shortcuts
                ui.add_space(24.0);
                lbl(ui, "Keyboard Shortcuts", self.pal.accent, fs + 1.0);
                ui.add_space(10.0);
                for (k, v) in [
                    ("↑ / ↓", "Navigate result rows"),
                    ("Enter", "Open magnet for selected row"),
                    ("D", "Toggle detail panel"),
                    ("F", "Add selected to Favorites"),
                    ("M", "Open magnet for selected row"),
                    ("Esc", "Close detail panel / clear search"),
                    ("Ctrl+F", "Focus search bar"),
                    ("Ctrl+R", "Re-run last search"),
                ] {
                    ui.horizontal(|ui| {
                        ui.add_space(ui.available_width() * 0.12);
                        ui.add(egui::Button::new(
                            RichText::new(k).font(FontId::proportional(fs)).color(self.pal.accent))
                            .fill(self.pal.surface)
                            .stroke(Stroke::new(1.0, self.pal.border)).rounding(4.0));
                        ui.add_space(8.0);
                        lbl(ui, v, self.pal.sub, fs);
                    });
                    ui.add_space(4.0);
                }

                ui.add_space(20.0);
                lbl(ui, "github.com/chethan62/torrentx", self.pal.dim, fs - 1.0);
            });
        });
    }

    // ─── Toast notifications ───────────────────────────────────────────────

    fn draw_toasts(&self, ctx: &egui::Context) {
        if self.toasts.is_empty() { return; }
        let scr = ctx.screen_rect();
        let mut y = scr.max.y - 54.0;
        for toast in self.toasts.iter().rev() {
            let a = ((toast.ttl.min(0.4) / 0.4) * 230.0) as u8;
            egui::Area::new(egui::Id::new(format!("toast_{}", toast.msg)))
                .fixed_pos([scr.max.x - 310.0, y])
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::none()
                        .fill(tint(self.pal.surface, a))
                        .stroke(Stroke::new(1.5, tint(toast.col, a)))
                        .rounding(8.0)
                        .inner_margin(egui::Margin::symmetric(14.0, 9.0))
                        .shadow(egui::epaint::Shadow {
                            offset: [0.0, 2.0].into(),
                            blur: 8.0,
                            spread: 0.0,
                            color: rgba(0, 0, 0, 80),
                        })
                        .show(ui, |ui| {
                            ui.label(RichText::new(&toast.msg)
                                .font(FontId::proportional(13.5)).color(tint(toast.col, a)));
                        });
                });
            y -= 46.0;
        }
    }
}

// ─── Helper for detail grid ─────────────────────────────────────────────────

fn grid_row(ui: &mut egui::Ui, label: &str, value: &str, color: Color32, p: &Pal, fs: f32) {
    ui.label(RichText::new(format!("{label}:")).font(FontId::proportional(fs - 1.5)).color(p.dim));
    ui.label(RichText::new(value).font(FontId::proportional(fs - 1.0)).color(color));
    ui.end_row();
}

// ─── Entry point ────────────────────────────────────────────────────────────

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "TorrentX",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("TorrentX")
                .with_inner_size([1300.0, 800.0])
                .with_min_inner_size([900.0, 560.0]),
            ..Default::default()
        },
        Box::new(|_cc| Box::new(App::default())),
    )
}