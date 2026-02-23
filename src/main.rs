#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui::{self, Color32, FontId, RichText, Stroke, Vec2, Visuals};
use egui_extras::{Column, TableBuilder};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// ─── Jackett API ──────────────────────────────────────────────────────────────

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

// ─── Themes ───────────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
enum Theme {
    Cyberpunk, Nord, Dracula, Gruvbox, TokyoNight, SolarizedDark, Midnight, Light,
    CatppuccinMocha, OneDark, RosePine, Everforest, Ayu, Kanagawa,
    // v6
    Monokai, MaterialOcean, CatppuccinLatte, Oxocarbon, GruvboxLight,
}

impl Theme {
    fn all() -> &'static [Theme] {
        use Theme::*;
        &[TokyoNight, Cyberpunk, Midnight, OneDark, CatppuccinMocha,
          Dracula, RosePine, Monokai, Kanagawa, Everforest, MaterialOcean,
          Oxocarbon, Ayu, Nord, Gruvbox, SolarizedDark,
          Light, GruvboxLight, CatppuccinLatte]
    }
    fn name(&self) -> &'static str {
        use Theme::*;
        match self {
            Cyberpunk       => "Cyberpunk",
            Nord            => "Nord",
            Dracula         => "Dracula",
            Gruvbox         => "Gruvbox",
            TokyoNight      => "Tokyo Night",
            SolarizedDark   => "Solarized",
            Light           => "Light",
            Midnight        => "Midnight",
            CatppuccinMocha => "Catppuccin",
            OneDark         => "One Dark",
            RosePine        => "Rose Pine",
            Everforest      => "Everforest",
            Ayu             => "Ayu Dark",
            Kanagawa        => "Kanagawa",
            Monokai         => "Monokai",
            MaterialOcean   => "Material Ocean",
            CatppuccinLatte => "Catppuccin Latte",
            Oxocarbon       => "Oxocarbon",
            GruvboxLight    => "Gruvbox Light",
        }
    }
    fn is_light_theme(&self) -> bool {
        matches!(self, Theme::Light | Theme::GruvboxLight | Theme::CatppuccinLatte)
    }
    fn preview_color(&self) -> Color32 {
        use Theme::*;
        match self {
            Cyberpunk       => rgb(6,182,212),
            Nord            => rgb(136,192,208),
            Dracula         => rgb(189,147,249),
            Gruvbox         => rgb(184,187,38),
            TokyoNight      => rgb(122,162,247),
            SolarizedDark   => rgb(42,161,152),
            Light           => rgb(59,130,246),
            Midnight        => rgb(192,132,252),
            CatppuccinMocha => rgb(203,166,247),
            OneDark         => rgb(97,175,239),
            RosePine        => rgb(196,167,231),
            Everforest      => rgb(167,192,128),
            Ayu             => rgb(255,182,109),
            Kanagawa        => rgb(127,180,202),
            Monokai         => rgb(166,226,46),
            MaterialOcean   => rgb(130,170,255),
            CatppuccinLatte => rgb(30,102,245),
            Oxocarbon       => rgb(120,190,255),
            GruvboxLight    => rgb(121,116,14),
        }
    }
}

#[derive(Clone)]
struct Pal {
    bg: Color32, surface: Color32, surface2: Color32, header_bg: Color32,
    accent: Color32, text: Color32, subtext: Color32, dim: Color32,
    green: Color32, red: Color32, yellow: Color32, border: Color32,
    row_odd: Color32, row_even: Color32, row_sel: Color32, row_hover: Color32,
    is_light: bool,
}

fn rgb(r: u8, g: u8, b: u8) -> Color32 { Color32::from_rgb(r, g, b) }
fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color32 { Color32::from_rgba_unmultiplied(r, g, b, a) }
fn tint(c: Color32, a: u8) -> Color32 { rgba(c.r(), c.g(), c.b(), a) }

impl Pal {
    fn from(t: &Theme) -> Self {
        use Theme::*;
        match t {
            Cyberpunk => Self {
                bg: rgb(10,14,28), surface: rgb(16,24,48), surface2: rgb(24,36,64),
                header_bg: rgb(8,12,22), accent: rgb(6,182,212),
                text: rgb(226,232,240), subtext: rgb(148,163,184), dim: rgb(71,85,105),
                green: rgb(34,197,94), red: rgb(239,68,68), yellow: rgb(245,158,11),
                border: rgb(30,41,59),
                row_odd: rgba(16,24,48,255), row_even: rgba(20,30,58,255),
                row_sel: rgba(6,182,212,40), row_hover: rgba(6,182,212,15), is_light: false,
            },
            Nord => Self {
                bg: rgb(46,52,64), surface: rgb(59,66,82), surface2: rgb(67,76,94),
                header_bg: rgb(36,42,54), accent: rgb(136,192,208),
                text: rgb(236,239,244), subtext: rgb(144,153,166), dim: rgb(76,86,106),
                green: rgb(163,190,140), red: rgb(191,97,106), yellow: rgb(235,203,139),
                border: rgb(67,76,94),
                row_odd: rgba(59,66,82,255), row_even: rgba(52,60,76,255),
                row_sel: rgba(136,192,208,50), row_hover: rgba(136,192,208,15), is_light: false,
            },
            Dracula => Self {
                bg: rgb(40,42,54), surface: rgb(50,53,68), surface2: rgb(60,63,80),
                header_bg: rgb(30,32,44), accent: rgb(189,147,249),
                text: rgb(248,248,242), subtext: rgb(139,155,180), dim: rgb(80,90,110),
                green: rgb(80,250,123), red: rgb(255,85,85), yellow: rgb(241,250,140),
                border: rgb(60,63,80),
                row_odd: rgba(50,53,68,255), row_even: rgba(44,47,62,255),
                row_sel: rgba(189,147,249,50), row_hover: rgba(189,147,249,15), is_light: false,
            },
            Gruvbox => Self {
                bg: rgb(40,40,40), surface: rgb(60,56,54), surface2: rgb(80,73,69),
                header_bg: rgb(29,32,33), accent: rgb(184,187,38),
                text: rgb(235,219,178), subtext: rgb(168,153,132), dim: rgb(102,92,84),
                green: rgb(184,187,38), red: rgb(251,73,52), yellow: rgb(250,189,47),
                border: rgb(80,73,69),
                row_odd: rgba(60,56,54,255), row_even: rgba(54,50,48,255),
                row_sel: rgba(184,187,38,50), row_hover: rgba(214,153,33,12), is_light: false,
            },
            TokyoNight => Self {
                bg: rgb(26,27,38), surface: rgb(36,40,59), surface2: rgb(41,46,66),
                header_bg: rgb(20,21,32), accent: rgb(122,162,247),
                text: rgb(192,202,245), subtext: rgb(169,177,214), dim: rgb(86,95,137),
                green: rgb(158,206,106), red: rgb(247,118,142), yellow: rgb(224,175,104),
                border: rgb(41,46,66),
                row_odd: rgba(36,40,59,255), row_even: rgba(30,34,53,255),
                row_sel: rgba(122,162,247,50), row_hover: rgba(122,162,247,15), is_light: false,
            },
            SolarizedDark => Self {
                bg: rgb(0,43,54), surface: rgb(7,54,66), surface2: rgb(0,60,80),
                header_bg: rgb(0,32,42), accent: rgb(42,161,152),
                text: rgb(131,148,150), subtext: rgb(101,123,131), dim: rgb(55,83,98),
                green: rgb(133,153,0), red: rgb(220,50,47), yellow: rgb(181,137,0),
                border: rgb(0,60,80),
                row_odd: rgba(7,54,66,255), row_even: rgba(0,48,60,255),
                row_sel: rgba(42,161,152,50), row_hover: rgba(42,161,152,15), is_light: false,
            },
            Light => Self {
                bg: rgb(249,250,251), surface: rgb(243,244,246), surface2: rgb(229,231,235),
                header_bg: rgb(243,244,246), accent: rgb(59,130,246),
                text: rgb(17,24,39), subtext: rgb(75,85,99), dim: rgb(156,163,175),
                green: rgb(22,163,74), red: rgb(220,38,38), yellow: rgb(217,119,6),
                border: rgb(209,213,219),
                row_odd: rgba(255,255,255,255), row_even: rgba(249,250,251,255),
                row_sel: rgba(59,130,246,30), row_hover: rgba(59,130,246,10), is_light: true,
            },
            Midnight => Self {
                bg: rgb(5,5,16), surface: rgb(12,12,26), surface2: rgb(18,18,36),
                header_bg: rgb(4,4,14), accent: rgb(192,132,252),
                text: rgb(226,232,240), subtext: rgb(148,163,184), dim: rgb(60,60,90),
                green: rgb(52,211,153), red: rgb(248,113,113), yellow: rgb(251,191,36),
                border: rgb(20,20,40),
                row_odd: rgba(12,12,26,255), row_even: rgba(8,8,20,255),
                row_sel: rgba(192,132,252,40), row_hover: rgba(192,132,252,12), is_light: false,
            },
            // ── New themes ──
            CatppuccinMocha => Self {
                bg: rgb(30,30,46), surface: rgb(24,24,37), surface2: rgb(36,36,54),
                header_bg: rgb(20,20,32), accent: rgb(203,166,247),
                text: rgb(205,214,244), subtext: rgb(166,173,200), dim: rgb(108,112,134),
                green: rgb(166,227,161), red: rgb(243,139,168), yellow: rgb(249,226,175),
                border: rgb(49,50,68),
                row_odd: rgba(24,24,37,255), row_even: rgba(30,30,46,255),
                row_sel: rgba(203,166,247,45), row_hover: rgba(203,166,247,12), is_light: false,
            },
            OneDark => Self {
                bg: rgb(40,44,52), surface: rgb(33,37,43), surface2: rgb(50,56,66),
                header_bg: rgb(26,29,35), accent: rgb(97,175,239),
                text: rgb(171,178,191), subtext: rgb(130,137,151), dim: rgb(92,99,112),
                green: rgb(152,195,121), red: rgb(224,108,117), yellow: rgb(229,192,123),
                border: rgb(60,68,80),
                row_odd: rgba(33,37,43,255), row_even: rgba(40,44,52,255),
                row_sel: rgba(97,175,239,45), row_hover: rgba(97,175,239,12), is_light: false,
            },
            RosePine => Self {
                bg: rgb(25,23,36), surface: rgb(31,29,46), surface2: rgb(38,35,58),
                header_bg: rgb(18,17,26), accent: rgb(196,167,231),
                text: rgb(224,222,244), subtext: rgb(144,140,170), dim: rgb(86,82,110),
                green: rgb(156,207,216), red: rgb(235,111,146), yellow: rgb(246,193,119),
                border: rgb(64,61,82),
                row_odd: rgba(31,29,46,255), row_even: rgba(25,23,36,255),
                row_sel: rgba(196,167,231,45), row_hover: rgba(196,167,231,12), is_light: false,
            },
            Everforest => Self {
                bg: rgb(45,53,59), surface: rgb(52,61,70), surface2: rgb(60,73,79),
                header_bg: rgb(35,43,46), accent: rgb(167,192,128),
                text: rgb(211,198,170), subtext: rgb(157,153,136), dim: rgb(105,103,95),
                green: rgb(167,192,128), red: rgb(230,126,128), yellow: rgb(219,188,127),
                border: rgb(74,82,90),
                row_odd: rgba(52,61,70,255), row_even: rgba(45,53,59,255),
                row_sel: rgba(167,192,128,45), row_hover: rgba(131,192,146,12), is_light: false,
            },
            Ayu => Self {
                bg: rgb(15,20,25), surface: rgb(20,27,33), surface2: rgb(26,34,44),
                header_bg: rgb(11,15,20), accent: rgb(255,182,109),
                text: rgb(203,215,232), subtext: rgb(139,155,175), dim: rgb(75,90,112),
                green: rgb(166,213,146), red: rgb(245,110,110), yellow: rgb(255,182,109),
                border: rgb(33,43,54),
                row_odd: rgba(20,27,33,255), row_even: rgba(15,20,25,255),
                row_sel: rgba(255,182,109,40), row_hover: rgba(255,182,109,12), is_light: false,
            },
            Kanagawa => Self {
                bg: rgb(22,22,30), surface: rgb(31,31,40), surface2: rgb(42,42,58),
                header_bg: rgb(15,15,24), accent: rgb(127,180,202),
                text: rgb(220,215,186), subtext: rgb(150,147,127), dim: rgb(84,84,109),
                green: rgb(118,185,0), red: rgb(195,64,67), yellow: rgb(220,180,70),
                border: rgb(54,54,74),
                row_odd: rgba(31,31,40,255), row_even: rgba(22,22,30,255),
                row_sel: rgba(127,180,202,45), row_hover: rgba(127,180,202,12), is_light: false,
            },
            Monokai => Self {
                bg: rgb(39,40,34), surface: rgb(47,49,40), surface2: rgb(61,62,50),
                header_bg: rgb(30,31,26), accent: rgb(166,226,46),
                text: rgb(248,248,242), subtext: rgb(200,200,190), dim: rgb(117,113,94),
                green: rgb(166,226,46), red: rgb(249,38,114), yellow: rgb(230,219,116),
                border: rgb(73,72,62),
                row_odd: rgba(47,49,40,255), row_even: rgba(39,40,34,255),
                row_sel: rgba(166,226,46,40), row_hover: rgba(166,226,46,12), is_light: false,
            },
            MaterialOcean => Self {
                bg: rgb(15,17,26), surface: rgb(13,14,22), surface2: rgb(30,34,54),
                header_bg: rgb(10,11,18), accent: rgb(130,170,255),
                text: rgb(198,212,254), subtext: rgb(137,148,184), dim: rgb(72,82,113),
                green: rgb(195,232,141), red: rgb(255,85,114), yellow: rgb(255,203,107),
                border: rgb(30,34,54),
                row_odd: rgba(13,14,22,255), row_even: rgba(15,17,26,255),
                row_sel: rgba(130,170,255,45), row_hover: rgba(130,170,255,12), is_light: false,
            },
            CatppuccinLatte => Self {
                bg: rgb(239,241,245), surface: rgb(230,233,239), surface2: rgb(204,208,218),
                header_bg: rgb(255,255,255), accent: rgb(30,102,245),
                text: rgb(76,79,105), subtext: rgb(100,104,132), dim: rgb(156,160,176),
                green: rgb(64,160,43), red: rgb(210,15,57), yellow: rgb(223,142,29),
                border: rgb(188,192,204),
                row_odd: rgba(255,255,255,255), row_even: rgba(239,241,245,255),
                row_sel: rgba(30,102,245,35), row_hover: rgba(30,102,245,8), is_light: true,
            },
            Oxocarbon => Self {
                bg: rgb(15,15,15), surface: rgb(22,22,22), surface2: rgb(32,32,32),
                header_bg: rgb(10,10,10), accent: rgb(120,190,255),
                text: rgb(244,244,244), subtext: rgb(180,180,180), dim: rgb(100,100,100),
                green: rgb(66,190,101), red: rgb(255,84,80), yellow: rgb(243,196,0),
                border: rgb(45,45,45),
                row_odd: rgba(22,22,22,255), row_even: rgba(15,15,15,255),
                row_sel: rgba(120,190,255,40), row_hover: rgba(120,190,255,10), is_light: false,
            },
            GruvboxLight => Self {
                bg: rgb(251,241,199), surface: rgb(242,229,188), surface2: rgb(213,196,161),
                header_bg: rgb(255,248,212), accent: rgb(121,116,14),
                text: rgb(60,56,54), subtext: rgb(102,92,84), dim: rgb(168,153,132),
                green: rgb(121,116,14), red: rgb(157,0,6), yellow: rgb(181,118,20),
                border: rgb(213,196,161),
                row_odd: rgba(255,248,212,255), row_even: rgba(251,241,199,255),
                row_sel: rgba(121,116,14,35), row_hover: rgba(121,116,14,8), is_light: true,
            },
        }
    }
}

// ─── Config ───────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone)]
struct Config {
    jackett_url:    String,
    api_key:        String,
    search_history: Vec<String>,
    favorites:      Vec<Favorite>,
    theme:          Theme,
    timeout_secs:   u64,
    dedupe:         bool,
    page_size:      usize,
    // New in v5
    row_height:     f32,
    font_size:      f32,
    show_cat_bar:   bool,
    col_size:       bool,
    col_leech:      bool,
    col_ratio:      bool,
    col_health:     bool,
    col_date:       bool,
    col_tracker:    bool,
}

#[derive(Serialize, Deserialize, Clone)]
struct Favorite {
    title: String, magnet: Option<String>, link: Option<String>,
    tracker: Option<String>, size: Option<u64>, seeders: Option<u32>,
    #[serde(default)]
    saved_at: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            jackett_url: "http://localhost:9117".into(),
            api_key: String::new(),
            search_history: vec![],
            favorites: vec![],
            theme: Theme::Cyberpunk,
            timeout_secs: 45,
            dedupe: false,
            page_size: 50,
            row_height: 44.0,
            font_size: 14.0,
            show_cat_bar: true,
            col_size: true, col_leech: true, col_ratio: true,
            col_health: true, col_date: true, col_tracker: true,
        }
    }
}

fn config_path() -> std::path::PathBuf {
    let dir = dirs_next::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("torrentx");
    let _ = fs::create_dir_all(&dir);
    dir.join("config.json")
}

fn load_config() -> Config {
    fs::read_to_string(config_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_config(cfg: &Config) {
    if let Ok(j) = serde_json::to_string_pretty(cfg) { let _ = fs::write(config_path(), j); }
}

// ─── State enums ──────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
enum SortCol { Name, Tracker, Size, Seeders, Leechers, Date }

#[derive(Clone, PartialEq)]
enum SortDir { Asc, Desc }

#[derive(Clone, PartialEq)]
enum Tab { Search, Favorites, About }

#[derive(Clone, PartialEq)]
enum SearchState { Idle, Searching, Done, Error(String) }

#[derive(Clone, PartialEq)]
enum HealthFilter { All, Hot, Good, Slow, Dead }

impl HealthFilter {
    fn label(&self) -> &'static str {
        match self { Self::All=>"All", Self::Hot=>"HOT", Self::Good=>"GOOD",
                     Self::Slow=>"SLOW", Self::Dead=>"DEAD" }
    }
    fn matches(&self, s: u32) -> bool {
        match self {
            Self::All  => true,
            Self::Hot  => s > 500,
            Self::Good => (101..=500).contains(&s),
            Self::Slow => (11..=100).contains(&s),
            Self::Dead => s <= 10,
        }
    }
}

#[derive(Clone)]
struct Toast { msg: String, timer: f32, color: Color32 }

// ─── App ──────────────────────────────────────────────────────────────────────

struct App {
    cfg: Config,
    pal: Pal,
    query:     String,
    category:  String,
    min_seed:  String,
    max_gb:    String,
    min_year:  String,
    trk_filt:  String,
    txt_filt:  String,
    hlth_filt: HealthFilter,
    sort_col:  SortCol,
    sort_dir:  SortDir,
    results:   Arc<Mutex<Vec<TorrentResult>>>,
    state:     Arc<Mutex<SearchState>>,
    count:     Arc<Mutex<usize>>,
    tab:          Tab,
    show_settings: bool,
    api_key_vis:   bool,
    selected:      Option<usize>,
    detail_open:   bool,
    show_hist:     bool,
    page:          usize,
    last_query:    String,
    toasts:        Vec<Toast>,
    // New in v5
    search_start:  Option<Instant>,
    last_elapsed:  Option<f64>,
    spin_frame:    usize,
    spin_tick:     f32,
    // v6
    fav_search:    String,
    hovered_row:   Option<usize>,
}

const CATEGORIES: &[&str] = &[
    "All","Movies","TV","Music","PC Games","Software","Anime","Books","XXX",
];

impl Default for App {
    fn default() -> Self {
        let cfg = load_config();
        let pal = Pal::from(&cfg.theme);
        Self {
            pal, cfg,
            query: String::new(), category: "All".into(),
            min_seed: String::new(), max_gb: String::new(),
            min_year: String::new(), trk_filt: String::new(), txt_filt: String::new(),
            hlth_filt: HealthFilter::All,
            sort_col: SortCol::Seeders, sort_dir: SortDir::Desc,
            results: Arc::new(Mutex::new(vec![])),
            state:   Arc::new(Mutex::new(SearchState::Idle)),
            count:   Arc::new(Mutex::new(0)),
            tab: Tab::Search, show_settings: false, api_key_vis: false,
            selected: None, detail_open: false, show_hist: false,
            page: 0, last_query: String::new(), toasts: vec![],
            search_start: None, last_elapsed: None,
            spin_frame: 0, spin_tick: 0.0,
            fav_search: String::new(), hovered_row: None,
        }
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn fmt_bytes(b: u64) -> String {
    const G: f64 = 1_073_741_824.0; const M: f64 = 1_048_576.0; const K: f64 = 1_024.0;
    let b = b as f64;
    if b >= G { format!("{:.2} GB", b/G) }
    else if b >= M { format!("{:.0} MB", b/M) }
    else if b >= K { format!("{:.0} KB", b/K) }
    else { format!("{} B", b as u64) }
}

fn time_ago(s: &str) -> String {
    let dt = chrono::DateTime::parse_from_rfc3339(s)
        .or_else(|_| chrono::DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%z"));
    if let Ok(dt) = dt {
        let secs = chrono::Utc::now()
            .signed_duration_since(dt.with_timezone(&chrono::Utc))
            .num_seconds().max(0);
        return if secs < 3600   { format!("{}m ago", secs/60) }
               else if secs < 86400  { format!("{}h ago", secs/3600) }
               else if secs < 604800 { format!("{}d ago", secs/86400) }
               else { dt.format("%Y-%m-%d").to_string() };
    }
    s.get(..10).unwrap_or("?").to_string()
}

fn seed_color(s: u32) -> Color32 {
    if s > 500 { rgb(34,197,94) } else if s > 100 { rgb(74,222,128) }
    else if s > 10 { rgb(245,158,11) } else if s > 0 { rgb(239,130,68) }
    else { rgb(239,68,68) }
}

fn health_label(s: u32) -> &'static str {
    if s > 500 { "HOT" } else if s > 100 { "GOOD" }
    else if s > 10 { "SLOW" } else if s > 0 { "DYING" } else { "DEAD" }
}

fn extract_year(s: &str) -> Option<u32> {
    chrono::DateTime::parse_from_rfc3339(s)
        .or_else(|_| chrono::DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%z"))
        .ok()
        .map(|dt| dt.format("%Y").to_string().parse::<u32>().unwrap_or(0))
}

fn cat_color(cat: &str) -> Color32 {
    match cat.split('/').next().unwrap_or("").trim() {
        "Movies"   => rgb(245,158,11),  "TV"       => rgb(59,130,246),
        "Music"    => rgb(16,185,129),  "Games"    => rgb(139,92,246),
        "Software" => rgb(6,182,212),   "Anime"    => rgb(236,72,153),
        "Books"    => rgb(249,115,22),  _          => rgb(100,116,139),
    }
}

fn urlenc(s: &str) -> String {
    s.chars().map(|c| match c {
        'A'..='Z'|'a'..='z'|'0'..='9'|'-'|'_'|'.'|'~' => c.to_string(),
        ' ' => "+".into(), c => format!("%{:02X}", c as u32),
    }).collect()
}

fn normalize(t: &str) -> String {
    let strip = ["1080p","720p","480p","4k","bluray","bdrip","webrip","x264","x265",
                 "hevc","10bit","hdr","yify","yts","rarbg","mkv","mp4","avi","remux"];
    let mut s = t.to_lowercase();
    for tok in &strip { s = s.replace(tok, " "); }
    s.split_whitespace().take(4).collect::<Vec<_>>().join(" ")
}

fn today_str() -> String { chrono::Utc::now().format("%Y-%m-%d %H:%M").to_string() }

const SPIN_FRAMES: &[&str] = &["⣾","⣽","⣻","⢿","⡿","⣟","⣯","⣷"];

// ─── Search thread ────────────────────────────────────────────────────────────

fn start_search(
    url: String, key: String, query: String, cat: String, timeout: u64,
    results: Arc<Mutex<Vec<TorrentResult>>>,
    state:   Arc<Mutex<SearchState>>,
    count:   Arc<Mutex<usize>>,
) {
    thread::spawn(move || {
        if let Ok(mut s) = state.lock() { *s = SearchState::Searching; }
        let mut endpoint = format!(
            "{}/api/v2.0/indexers/all/results?apikey={}&Query={}",
            url.trim_end_matches('/'), urlenc(&key), urlenc(&query)
        );
        if cat != "All" { endpoint.push_str(&format!("&Category[]={}", urlenc(&cat))); }
        let client = match Client::builder().timeout(Duration::from_secs(timeout)).build() {
            Ok(c) => c, Err(e) => { set_err(&state, format!("Build error: {e}")); return; }
        };
        match client.get(&endpoint).send() {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    match resp.json::<JackettResponse>() {
                        Ok(data) => {
                            let n = data.results.len();
                            if let Ok(mut r) = results.lock() { *r = data.results; }
                            if let Ok(mut c) = count.lock()   { *c = n; }
                            if let Ok(mut s) = state.lock()   { *s = SearchState::Done; }
                        }
                        Err(e) => set_err(&state, format!("Parse error: {e}")),
                    }
                } else {
                    set_err(&state, match status.as_u16() {
                        401 => "Invalid API key — open Settings to update it.".into(),
                        403 => "Forbidden — check Jackett permissions.".into(),
                        404 => "Jackett endpoint not found — check the URL.".into(),
                        500 => "Jackett internal error — check Jackett logs.".into(),
                        s   => format!("HTTP {s} from Jackett"),
                    });
                }
            }
            Err(e) => set_err(&state, if e.is_connect() {
                format!("Cannot reach Jackett at {url}\nsudo systemctl start jackett")
            } else if e.is_timeout() {
                format!("Timed out after {timeout}s — increase timeout in Settings")
            } else { format!("Network error: {e}") }),
        }
    });
}

fn set_err(state: &Arc<Mutex<SearchState>>, msg: String) {
    if let Ok(mut s) = state.lock() { *s = SearchState::Error(msg); }
}

// ─── App logic ────────────────────────────────────────────────────────────────

impl App {
    fn search(&mut self) {
        let q = self.query.trim().to_string();
        if q.is_empty() { return; }
        if self.cfg.api_key.trim().is_empty() {
            set_err(&self.state, "Enter your Jackett API key in Settings (gear icon).".into()); return;
        }
        self.cfg.search_history.retain(|h| h != &q);
        self.cfg.search_history.insert(0, q.clone());
        self.cfg.search_history.truncate(20);
        save_config(&self.cfg);
        self.selected = None; self.detail_open = false;
        self.show_hist = false; self.page = 0; self.last_query = q.clone();
        self.txt_filt.clear(); self.search_start = Some(Instant::now());
        self.last_elapsed = None;
        if let Ok(mut r) = self.results.lock() { r.clear(); }
        if let Ok(mut c) = self.count.lock()   { *c = 0; }
        start_search(
            self.cfg.jackett_url.clone(), self.cfg.api_key.clone(),
            q, self.category.clone(), self.cfg.timeout_secs,
            Arc::clone(&self.results), Arc::clone(&self.state), Arc::clone(&self.count),
        );
    }

    fn save_fav(&mut self, r: &TorrentResult) {
        if self.cfg.favorites.iter().any(|f| f.title == r.title) {
            self.toast("Already in favorites", self.pal.subtext); return;
        }
        self.cfg.favorites.push(Favorite {
            title: r.title.clone(), magnet: r.magnet_uri.clone(), link: r.link.clone(),
            tracker: r.tracker.clone(), size: r.size, seeders: r.seeders,
            saved_at: today_str(),
        });
        save_config(&self.cfg);
        self.toast("Saved to Favorites", self.pal.yellow);
    }

    fn toast(&mut self, msg: &str, color: Color32) {
        self.toasts.retain(|t| t.msg != msg);
        self.toasts.push(Toast { msg: msg.into(), timer: 3.0, color });
    }

    fn set_theme(&mut self, t: Theme) {
        self.cfg.theme = t;
        self.pal = Pal::from(&self.cfg.theme);
        save_config(&self.cfg);
    }

    fn get_state(&self) -> SearchState {
        self.state.lock().map(|g| g.clone()).unwrap_or(SearchState::Idle)
    }
    fn get_results(&self) -> Vec<TorrentResult> {
        self.results.lock().map(|g| g.clone()).unwrap_or_default()
    }
    fn get_count(&self) -> usize {
        self.count.lock().map(|g| *g).unwrap_or(0)
    }

    fn export_csv(&self, rows: &[TorrentResult]) {
        let path = dirs_next::download_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(format!("torrentx_{}.csv", self.last_query.replace(' ', "_")));
        let mut out = "Title,Tracker,Category,Size,Seeders,Leechers,Date\n".to_string();
        for r in rows {
            out.push_str(&format!(
                "\"{}\",\"{}\",\"{}\",\"{}\",{},{},\"{}\"\n",
                r.title.replace('"', "'"),
                r.tracker.as_deref().unwrap_or(""),
                r.category_desc.as_deref().unwrap_or(""),
                r.size.map(fmt_bytes).unwrap_or_default(),
                r.seeders.unwrap_or(0), r.peers.unwrap_or(0),
                r.publish_date.as_deref().map(time_ago).unwrap_or_default(),
            ));
        }
        if fs::write(&path, out).is_ok() { let _ = open::that(&path); }
    }

    fn filtered_sorted(&self, raw: &[TorrentResult]) -> Vec<TorrentResult> {
        let min_s: u32  = self.min_seed.parse().unwrap_or(0);
        let max_b: u64  = self.max_gb.parse::<f64>().unwrap_or(0.0) as u64 * 1_073_741_824;
        let min_yr: u32 = self.min_year.parse().unwrap_or(0);
        let tf = self.trk_filt.to_lowercase();
        let tx = self.txt_filt.to_lowercase();
        let mut seen = std::collections::HashSet::new();

        let mut out: Vec<_> = raw.iter().filter(|r| {
            let s = r.seeders.unwrap_or(0);
            if s < min_s { return false; }
            if max_b > 0 { if r.size.unwrap_or(0) > max_b { return false; } }
            if min_yr > 0 {
                let yr = r.publish_date.as_deref()
                    .and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok())
                    .map(|dt| dt.format("%Y").to_string().parse::<u32>().unwrap_or(0))
                    .unwrap_or(0);
                if yr < min_yr { return false; }
            }
            if !tf.is_empty() && !r.tracker.as_deref().unwrap_or("").to_lowercase().contains(&tf) { return false; }
            if !tx.is_empty() {
                let hay = format!("{} {} {}", r.title.to_lowercase(),
                    r.tracker.as_deref().unwrap_or("").to_lowercase(),
                    r.category_desc.as_deref().unwrap_or("").to_lowercase());
                if !hay.contains(&tx) { return false; }
            }
            if !self.hlth_filt.matches(s) { return false; }
            if self.cfg.dedupe && !seen.insert(normalize(&r.title)) { return false; }
            true
        }).cloned().collect();

        out.sort_by(|a, b| {
            let cmp = match self.sort_col {
                SortCol::Seeders  => b.seeders.unwrap_or(0).cmp(&a.seeders.unwrap_or(0)),
                SortCol::Leechers => b.peers.unwrap_or(0).cmp(&a.peers.unwrap_or(0)),
                SortCol::Size     => b.size.unwrap_or(0).cmp(&a.size.unwrap_or(0)),
                SortCol::Name     => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
                SortCol::Tracker  => a.tracker.as_deref().unwrap_or("").to_lowercase().cmp(&b.tracker.as_deref().unwrap_or("").to_lowercase()),
                SortCol::Date     => b.publish_date.as_deref().unwrap_or("")
                                      .cmp(a.publish_date.as_deref().unwrap_or("")),
            };
            if self.sort_dir == SortDir::Asc { cmp.reverse() } else { cmp }
        });
        out
    }

    fn max_pages(&self, total: usize) -> usize {
        if self.cfg.page_size == 0 || total == 0 { return 1; }
        (total + self.cfg.page_size - 1) / self.cfg.page_size
    }

    fn page_slice<'a>(&self, sorted: &'a [TorrentResult]) -> &'a [TorrentResult] {
        if self.cfg.page_size == 0 { return sorted; }
        let start = self.page * self.cfg.page_size;
        if start >= sorted.len() { return &[]; }
        &sorted[start..(start + self.cfg.page_size).min(sorted.len())]
    }

    /// Count results by top-level category
    fn cat_breakdown(results: &[TorrentResult]) -> Vec<(String, usize)> {
        let mut map: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for r in results {
            let cat = r.category_desc.as_deref()
                .and_then(|c| c.split('/').next())
                .unwrap_or("Other")
                .trim()
                .to_string();
            *map.entry(cat).or_insert(0) += 1;
        }
        let mut v: Vec<_> = map.into_iter().collect();
        v.sort_by(|a,b| b.1.cmp(&a.1));
        v.truncate(6);
        v
    }
}

// ─── egui main loop ───────────────────────────────────────────────────────────

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.apply_theme(ctx);

        let state = self.get_state();

        // Spinner animation
        if state == SearchState::Searching {
            ctx.request_repaint_after(Duration::from_millis(80));
            let dt = ctx.input(|i| i.unstable_dt).clamp(0.0, 0.1);
            self.spin_tick += dt;
            if self.spin_tick > 0.1 {
                self.spin_tick = 0.0;
                self.spin_frame = (self.spin_frame + 1) % SPIN_FRAMES.len();
            }
        }

        // Record elapsed when search finishes
        if state == SearchState::Done || matches!(state, SearchState::Error(_)) {
            if let Some(start) = self.search_start.take() {
                self.last_elapsed = Some(start.elapsed().as_secs_f64());
            }
        }

        // Tick toasts
        let dt = ctx.input(|i| i.unstable_dt).clamp(0.0, 0.1);
        self.toasts.retain_mut(|t| { t.timer -= dt; t.timer > 0.0 });

        // Global shortcuts
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::F)) {
            ctx.memory_mut(|m| m.request_focus(egui::Id::new("q")));
        }
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::R)) { self.search(); }

        // ── Status bar ──
        egui::TopBottomPanel::bottom("status")
            .exact_height(28.0)
            .frame(egui::Frame::none()
                .fill(self.pal.header_bg)
                .inner_margin(egui::Margin::symmetric(12.0, 4.0))
                .stroke(Stroke::new(1.0, self.pal.border)))
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    match &state {
                        SearchState::Idle => {
                            ui.label(RichText::new("○ Ready").font(FontId::proportional(13.0)).color(self.pal.dim));
                        }
                        SearchState::Searching => {
                            let spin = SPIN_FRAMES[self.spin_frame];
                            ui.label(RichText::new(format!("{spin} Scanning indexers…")).font(FontId::proportional(13.0)).color(self.pal.accent));
                            if let Some(start) = &self.search_start {
                                let secs = start.elapsed().as_secs_f64();
                                ui.label(RichText::new(format!("  {:.1}s", secs)).font(FontId::proportional(12.0)).color(self.pal.dim));
                            }
                        }
                        SearchState::Done => {
                            let n = self.get_count();
                            let elapsed = self.last_elapsed.map(|e| format!("  ({:.1}s)", e)).unwrap_or_default();
                            ui.label(RichText::new(format!("● {n} results for \"{}\"{elapsed}", self.last_query))
                                .font(FontId::proportional(13.0)).color(self.pal.green));
                        }
                        SearchState::Error(e) => {
                            ui.label(RichText::new(format!("● {}", e.lines().next().unwrap_or(e)))
                                .font(FontId::proportional(13.0)).color(self.pal.red));
                        }
                    };
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new("Ctrl+F  Ctrl+R  Up/Dn  Enter=magnet  D=detail  F=fav  M=mag")
                            .font(FontId::proportional(12.0)).color(self.pal.dim));
                    });
                });
            });

        // ── Header ──
        egui::TopBottomPanel::top("header")
            .exact_height(58.0)
            .frame(egui::Frame::none()
                .fill(self.pal.surface)
                .stroke(Stroke::new(1.0, self.pal.border)))
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui.label(RichText::new("Torrent").font(FontId::monospace(17.0)).strong().color(self.pal.text));
                    ui.label(RichText::new("X").font(FontId::monospace(17.0)).strong().color(self.pal.accent));
                    ui.add_space(4.0);
                    // version badge
                    egui::Frame::none()
                        .fill(tint(self.pal.accent, 25))
                        .rounding(8.0)
                        .inner_margin(egui::Margin { left: 5.0, right: 5.0, top: 2.0, bottom: 2.0 })
                        .show(ui, |ui| {
                            ui.label(RichText::new("v5").font(FontId::proportional(11.0)).color(self.pal.accent));
                        });
                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(8.0);

                    for (lbl, t) in [("Search", Tab::Search),("Favorites", Tab::Favorites),("About", Tab::About)] {
                        let active = self.tab == t;
                        let count_badge = if t == Tab::Favorites && !self.cfg.favorites.is_empty() {
                            format!(" ({})", self.cfg.favorites.len())
                        } else { String::new() };
                        let label = format!("{lbl}{count_badge}");
                        if ui.add(egui::Button::new(
                            RichText::new(&label).font(FontId::proportional(15.0))
                                .color(if active { self.pal.accent } else { self.pal.subtext }))
                            .fill(if active { tint(self.pal.accent, 20) } else { Color32::TRANSPARENT })
                            .stroke(Stroke::new(if active {1.0} else {0.0}, self.pal.accent))
                            .rounding(6.0).min_size(Vec2::new(0.0, 36.0))
                        ).clicked() { self.tab = t; }
                        ui.add_space(2.0);
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(12.0);
                        let sa = self.show_settings;
                        if ui.add(egui::Button::new(
                            RichText::new("Settings").size(14.0)
                                .color(if sa { self.pal.accent } else { self.pal.subtext }))
                            .fill(if sa { tint(self.pal.accent, 20) } else { Color32::TRANSPARENT })
                            .stroke(Stroke::new(1.0, if sa { self.pal.accent } else { self.pal.border }))
                            .rounding(6.0).min_size(Vec2::new(0.0, 32.0))
                        ).on_hover_text("Open settings panel").clicked() {
                            self.show_settings = !self.show_settings;
                        }
                        ui.add_space(8.0);
                        // Theme picker (compact colored dot + name)
                        let dot_color = self.cfg.theme.preview_color();
                        egui::ComboBox::from_id_source("theme")
                            .selected_text(
                                RichText::new(self.cfg.theme.name())
                                    .font(FontId::proportional(14.0))
                                    .color(dot_color)
                            )
                            .width(152.0)
                            .show_ui(ui, |ui| {
                                ui.label(RichText::new("-- Dark --").font(FontId::proportional(11.0)).color(self.pal.dim));
                                for t in Theme::all().iter().filter(|t| !t.is_light_theme()) {
                                    let active = &self.cfg.theme == t;
                                    let col = t.preview_color();
                                    if ui.add(egui::SelectableLabel::new(active,
                                        RichText::new(format!("  {}", t.name()))
                                            .font(FontId::proportional(13.0)).color(col)
                                    )).clicked() { self.set_theme(t.clone()); }
                                }
                                ui.add_space(2.0);
                                ui.label(RichText::new("-- Light --").font(FontId::proportional(11.0)).color(self.pal.dim));
                                for t in Theme::all().iter().filter(|t| t.is_light_theme()) {
                                    let active = &self.cfg.theme == t;
                                    let col = t.preview_color();
                                    if ui.add(egui::SelectableLabel::new(active,
                                        RichText::new(format!("  {}", t.name()))
                                            .font(FontId::proportional(13.0)).color(col)
                                    )).clicked() { self.set_theme(t.clone()); }
                                }
                            });
                        ui.add_space(10.0);
                        let n = self.get_count();
                        if n > 0 {
                            ui.label(RichText::new(format!("{n} results"))
                                .font(FontId::proportional(13.0)).color(self.pal.dim));
                        }
                    });
                });
            });

        // ── Settings panel ──
        if self.show_settings {
            egui::TopBottomPanel::top("settings")
                .frame(egui::Frame::none()
                    .fill(self.pal.header_bg)
                    .stroke(Stroke::new(1.0, self.pal.border))
                    .inner_margin(egui::Margin { left: 16.0, right: 16.0, top: 8.0, bottom: 8.0 }))
                .show(ctx, |ui| {
                    // Row 1: connection
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("CONNECTION").font(FontId::proportional(11.0)).color(self.pal.dim).strong());
                        ui.add_space(8.0);
                        ui.label(RichText::new("Jackett URL").font(FontId::proportional(13.0)).color(self.pal.subtext));
                        ui.add(egui::TextEdit::singleline(&mut self.cfg.jackett_url)
                            .desired_width(180.0).font(FontId::monospace(13.0)));
                        ui.add_space(8.0);
                        ui.label(RichText::new("API Key").font(FontId::proportional(13.0)).color(self.pal.subtext));
                        ui.add(egui::TextEdit::singleline(&mut self.cfg.api_key)
                            .desired_width(230.0).password(!self.api_key_vis)
                            .hint_text("from Jackett dashboard top-right")
                            .font(FontId::monospace(13.0)));
                        if ui.small_button(if self.api_key_vis {"hide"} else {"show"}).clicked() {
                            self.api_key_vis = !self.api_key_vis;
                        }
                        ui.add_space(8.0);
                        ui.label(RichText::new("Timeout").font(FontId::proportional(13.0)).color(self.pal.subtext));
                        let mut ts = self.cfg.timeout_secs.to_string();
                        if ui.add(egui::TextEdit::singleline(&mut ts)
                            .desired_width(36.0).font(FontId::monospace(13.0))).changed() {
                            if let Ok(v) = ts.parse::<u64>() { self.cfg.timeout_secs = v.clamp(5, 120); }
                        }
                        ui.label(RichText::new("s").font(FontId::proportional(13.0)).color(self.pal.dim));
                    });
                    ui.add_space(5.0);
                    // Row 2: display options
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("DISPLAY").font(FontId::proportional(11.0)).color(self.pal.dim).strong());
                        ui.add_space(8.0);

                        ui.label(RichText::new("Row height").font(FontId::proportional(13.0)).color(self.pal.subtext));
                        for (lbl, h) in [("Compact", 32.0f32), ("Normal", 44.0), ("Roomy", 56.0)] {
                            let active = (self.cfg.row_height - h).abs() < 1.0;
                            if ui.add(egui::SelectableLabel::new(active,
                                RichText::new(lbl).font(FontId::proportional(13.0))
                            )).clicked() { self.cfg.row_height = h; }
                        }
                        ui.add_space(12.0);

                        ui.label(RichText::new("Font").font(FontId::proportional(13.0)).color(self.pal.subtext));
                        for (lbl, sz) in [("S", 12.0f32), ("M", 14.0), ("L", 16.0)] {
                            let active = (self.cfg.font_size - sz).abs() < 0.5;
                            if ui.add(egui::SelectableLabel::new(active,
                                RichText::new(lbl).font(FontId::proportional(13.0))
                            )).clicked() { self.cfg.font_size = sz; }
                        }
                        ui.add_space(12.0);

                        ui.label(RichText::new("Page").font(FontId::proportional(13.0)).color(self.pal.subtext));
                        for ps in [25usize, 50, 100, 0] {
                            let lbl = if ps == 0 { "All" } else if ps == 25 { "25" } else if ps == 50 { "50" } else { "100" };
                            let active = self.cfg.page_size == ps;
                            if ui.add(egui::SelectableLabel::new(active,
                                RichText::new(lbl).font(FontId::proportional(13.0))
                            )).clicked() { self.cfg.page_size = ps; self.page = 0; }
                        }
                        ui.add_space(12.0);

                        if ui.add(egui::SelectableLabel::new(self.cfg.dedupe,
                            RichText::new("Dedupe").font(FontId::proportional(13.0))
                        )).on_hover_text("Collapse near-duplicate titles from different trackers").clicked() {
                            self.cfg.dedupe = !self.cfg.dedupe;
                        }
                        ui.add_space(6.0);
                        if ui.add(egui::SelectableLabel::new(self.cfg.show_cat_bar,
                            RichText::new("Cat bar").font(FontId::proportional(13.0))
                        )).on_hover_text("Show category breakdown after search").clicked() {
                            self.cfg.show_cat_bar = !self.cfg.show_cat_bar;
                        }
                    });
                    ui.add_space(4.0);
                    // Row 3: Columns
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("COLUMNS").font(FontId::proportional(11.0)).color(self.pal.dim).strong());
                        ui.add_space(6.0);
                        for (lbl, cur) in [
                            ("Tracker", self.cfg.col_tracker),
                            ("Size",    self.cfg.col_size),
                            ("Leech",   self.cfg.col_leech),
                            ("Ratio",   self.cfg.col_ratio),
                            ("Health",  self.cfg.col_health),
                            ("Date",    self.cfg.col_date),
                        ] {
                            if ui.add(egui::SelectableLabel::new(cur,
                                RichText::new(lbl).font(FontId::proportional(13.0))
                                    .color(if cur { self.pal.accent } else { self.pal.dim })
                            )).clicked() {
                                match lbl {
                                    "Tracker" => self.cfg.col_tracker = !self.cfg.col_tracker,
                                    "Size"    => self.cfg.col_size    = !self.cfg.col_size,
                                    "Leech"   => self.cfg.col_leech   = !self.cfg.col_leech,
                                    "Ratio"   => self.cfg.col_ratio   = !self.cfg.col_ratio,
                                    "Health"  => self.cfg.col_health  = !self.cfg.col_health,
                                    "Date"    => self.cfg.col_date    = !self.cfg.col_date,
                                    _ => {}
                                }
                            }
                            ui.add_space(2.0);
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(egui::Button::new(
                                RichText::new("Save").font(FontId::proportional(13.0)).color(self.pal.green))
                                .fill(tint(self.pal.green, 18)).stroke(Stroke::new(1.0, tint(self.pal.green, 80))).rounding(4.0)
                            ).clicked() {
                                save_config(&self.cfg);
                                self.toast("Settings saved", self.pal.green);
                            }
                        });
                    });
                });
        }

        // ── Central ──
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(self.pal.bg))
            .show(ctx, |ui| {
                match self.tab.clone() {
                    Tab::Search    => self.draw_search(ui, ctx, &state),
                    Tab::Favorites => self.draw_favorites(ui),
                    Tab::About     => self.draw_about(ui),
                }
            });

        self.draw_toasts(ctx);
    }
}

// ─── Search tab ───────────────────────────────────────────────────────────────

impl App {
    fn draw_search(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, state: &SearchState) {
        let fs = self.cfg.font_size;
        ui.add_space(10.0);
        let searching = *state == SearchState::Searching;
        let mut search_bar_rect = egui::Rect::NOTHING;

        // ── Search bar ──
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .id(egui::Id::new("q"))
                    .desired_width(ui.available_width() - 330.0)
                    .hint_text("Search torrents…  movies, shows, games, software, anime")
                    .font(FontId::proportional(fs + 2.0))
            );
            search_bar_rect = resp.rect;
            if resp.gained_focus() && !self.cfg.search_history.is_empty() { self.show_hist = true; }
            if resp.changed() && self.query.is_empty() { self.show_hist = false; }
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) { self.search(); }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) { self.query.clear(); self.show_hist = false; }

            ui.add_space(6.0);
            egui::ComboBox::from_id_source("cat")
                .selected_text(RichText::new(&self.category).font(FontId::proportional(fs)))
                .width(120.0)
                .show_ui(ui, |ui| {
                    for &cat in CATEGORIES {
                        ui.selectable_value(&mut self.category, cat.into(),
                            RichText::new(cat).font(FontId::proportional(fs)));
                    }
                });

            ui.add_space(6.0);
            if ui.add_enabled(!searching,
                egui::Button::new(RichText::new(if searching {"Scanning…"} else {"  Search  "})
                    .font(FontId::proportional(fs + 1.0)).strong().color(Color32::WHITE))
                .fill(if searching { rgb(6,100,130) } else { rgb(8,145,178) })
                .rounding(6.0).min_size(Vec2::new(110.0, 38.0))
            ).clicked() { self.search(); }

            if !self.query.is_empty() {
                if ui.add(egui::Button::new(RichText::new("x").color(self.pal.subtext))
                    .fill(Color32::TRANSPARENT).rounding(4.0)).on_hover_text("Clear").clicked() {
                    self.query.clear(); self.show_hist = false;
                }
            }
        });

        // ── History dropdown ──
        if self.show_hist && !self.cfg.search_history.is_empty() {
            let pos = egui::pos2(search_bar_rect.min.x, search_bar_rect.max.y + 4.0);
            let w   = search_bar_rect.width();
            let hist = self.cfg.search_history.clone();
            let mut clicked: Option<String> = None;
            let mut clear = false;
            egui::Area::new(egui::Id::new("hist_dd"))
                .fixed_pos(pos).order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::none()
                        .fill(self.pal.surface).rounding(8.0)
                        .stroke(Stroke::new(1.0, self.pal.border))
                        .inner_margin(egui::Margin::symmetric(12.0, 10.0))
                        .show(ui, |ui| {
                            ui.set_width(w);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Recent").font(FontId::proportional(12.0)).color(self.pal.dim));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.add(egui::Button::new(RichText::new("clear").font(FontId::proportional(12.0)).color(self.pal.dim))
                                        .fill(Color32::TRANSPARENT).frame(false)).clicked() { clear = true; }
                                });
                            });
                            ui.add_space(4.0);
                            let mut delete_h: Option<String> = None;
                            for h in hist.iter().take(8) {
                                ui.horizontal(|ui| {
                                    if ui.add(egui::Button::new(
                                        RichText::new(h.as_str()).font(FontId::proportional(fs)).color(self.pal.text)
                                    ).fill(Color32::TRANSPARENT).frame(false)
                                     .min_size(egui::vec2(w - 44.0, 26.0))).clicked() {
                                        clicked = Some(h.clone());
                                    }
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.add(egui::Button::new(RichText::new("x").size(11.0).color(self.pal.dim))
                                            .fill(Color32::TRANSPARENT).frame(false)
                                            .min_size(egui::vec2(18.0, 18.0))
                                        ).on_hover_text("Remove").clicked() {
                                            delete_h = Some(h.clone());
                                        }
                                    });
                                });
                            }
                            if let Some(h) = delete_h {
                                self.cfg.search_history.retain(|x| x != &h);
                                save_config(&self.cfg);
                            }
                        });
                });
            if let Some(h) = clicked { self.query = h; self.show_hist = false; self.search(); }
            if clear { self.cfg.search_history.clear(); save_config(&self.cfg); self.show_hist = false; }
        }

        ui.add_space(8.0);

        // ── Filter bar (2 rows) ──
        egui::Frame::none()
            .fill(self.pal.surface).rounding(8.0)
            .stroke(Stroke::new(1.0, self.pal.border))
            .inner_margin(egui::Margin { left: 12.0, right: 12.0, top: 8.0, bottom: 8.0 })
            .outer_margin(egui::Margin::symmetric(12.0, 0.0))
            .show(ui, |ui| {
                // Row 1: filters
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Filter:").font(FontId::proportional(fs - 1.0)).color(self.pal.dim));
                    ui.add_space(4.0);
                    ui.add(egui::TextEdit::singleline(&mut self.txt_filt)
                        .desired_width(130.0).hint_text("within results").font(FontId::proportional(fs)));
                    ui.add_space(8.0);
                    ui.label(RichText::new("Min seeds").font(FontId::proportional(fs - 1.0)).color(self.pal.dim));
                    ui.add_space(3.0);
                    ui.add(egui::TextEdit::singleline(&mut self.min_seed)
                        .desired_width(46.0).hint_text("0").font(FontId::proportional(fs)));
                    ui.add_space(8.0);
                    ui.label(RichText::new("Max GB").font(FontId::proportional(fs - 1.0)).color(self.pal.dim));
                    ui.add_space(3.0);
                    ui.add(egui::TextEdit::singleline(&mut self.max_gb)
                        .desired_width(46.0).hint_text("inf").font(FontId::proportional(fs)));
                    ui.add_space(8.0);
                    ui.label(RichText::new("Year >=").font(FontId::proportional(fs - 1.0)).color(self.pal.dim));
                    ui.add_space(3.0);
                    ui.add(egui::TextEdit::singleline(&mut self.min_year)
                        .desired_width(46.0).hint_text("any").font(FontId::proportional(fs)));
                    ui.add_space(8.0);
                    ui.label(RichText::new("Tracker").font(FontId::proportional(fs - 1.0)).color(self.pal.dim));
                    ui.add_space(3.0);
                    ui.add(egui::TextEdit::singleline(&mut self.trk_filt)
                        .desired_width(100.0).hint_text("any").font(FontId::proportional(fs)));
                    let dirty = !self.txt_filt.is_empty() || !self.min_seed.is_empty()
                        || !self.max_gb.is_empty() || !self.trk_filt.is_empty()
                        || !self.min_year.is_empty() || self.hlth_filt != HealthFilter::All;
                    if dirty {
                        ui.add_space(8.0);
                        if ui.add(egui::Button::new(RichText::new("Reset filters").font(FontId::proportional(fs - 1.0)).color(self.pal.red))
                            .fill(Color32::TRANSPARENT).rounding(4.0)).clicked() {
                            self.txt_filt.clear(); self.min_seed.clear();
                            self.max_gb.clear(); self.trk_filt.clear(); self.min_year.clear();
                            self.hlth_filt = HealthFilter::All; self.page = 0;
                        }
                    }
                });
                ui.add_space(6.0);
                // Row 2: health chips + sort
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Health:").font(FontId::proportional(fs - 1.0)).color(self.pal.dim));
                    ui.add_space(4.0);
                    for hf in [HealthFilter::All,HealthFilter::Hot,HealthFilter::Good,HealthFilter::Slow,HealthFilter::Dead] {
                        let active = self.hlth_filt == hf;
                        if ui.add(egui::SelectableLabel::new(active,
                            RichText::new(hf.label()).font(FontId::proportional(fs - 1.0))
                                .color(if active { self.pal.accent } else { self.pal.subtext })
                        )).clicked() { self.hlth_filt = hf; self.page = 0; }
                        ui.add_space(2.0);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let dir_lbl = if self.sort_dir == SortDir::Desc { " DESC " } else { " ASC  " };
                        if ui.add(egui::Button::new(
                            RichText::new(dir_lbl).font(FontId::proportional(fs - 1.0)).strong().color(self.pal.accent))
                            .fill(tint(self.pal.accent, 22)).stroke(Stroke::new(1.0, tint(self.pal.accent, 70))).rounding(4.0)
                        ).on_hover_text("Toggle sort direction").clicked() {
                            self.sort_dir = if self.sort_dir == SortDir::Desc { SortDir::Asc } else { SortDir::Desc };
                            self.page = 0;
                        }
                        ui.add_space(8.0);
                        ui.label(RichText::new("Sort:").font(FontId::proportional(fs - 1.0)).color(self.pal.dim));
                        ui.add_space(4.0);
                        for (lbl, col) in [("Date",SortCol::Date),("Size",SortCol::Size),
                                           ("Leech",SortCol::Leechers),("Seeds",SortCol::Seeders),
                                           ("Tracker",SortCol::Tracker),("Name",SortCol::Name)] {
                            let active = self.sort_col == col;
                            let ls = if active {
                                if self.sort_dir == SortDir::Desc { format!("{lbl}v") } else { format!("{lbl}^") }
                            } else { lbl.to_string() };
                            if ui.add(egui::SelectableLabel::new(active,
                                RichText::new(&ls).font(FontId::proportional(fs - 1.0))
                                    .color(if active { self.pal.accent } else { self.pal.subtext })
                            )).clicked() {
                                if self.sort_col == col {
                                    self.sort_dir = if self.sort_dir==SortDir::Desc { SortDir::Asc } else { SortDir::Desc };
                                } else { self.sort_col = col; self.sort_dir = SortDir::Desc; }
                                self.page = 0;
                            }
                            ui.add_space(2.0);
                        }
                    });
                });
            });

        ui.add_space(8.0);

        // ── Content ──
        match state {
            SearchState::Idle    => self.draw_idle(ui),
            SearchState::Searching => {
                ui.add_space(80.0);
                ui.vertical_centered(|ui| {
                    ui.spinner();
                    ui.add_space(16.0);
                    ui.label(RichText::new("Scanning all Jackett indexers…").font(FontId::proportional(17.0)).color(self.pal.subtext));
                    ui.add_space(4.0);
                    ui.label(RichText::new("This may take 10–30 seconds depending on how many trackers you have configured")
                        .font(FontId::proportional(13.0)).color(self.pal.dim));
                });
            }
            SearchState::Error(err) => {
                ui.add_space(12.0);
                egui::Frame::none()
                    .fill(tint(self.pal.red, 15)).stroke(Stroke::new(1.0, tint(self.pal.red, 80)))
                    .rounding(8.0).inner_margin(egui::Margin::symmetric(20.0, 14.0))
                    .outer_margin(egui::Margin::symmetric(12.0, 0.0))
                    .show(ui, |ui| {
                        for line in err.lines() {
                            ui.label(RichText::new(line).font(FontId::proportional(fs)).color(self.pal.red));
                        }
                        ui.add_space(8.0);
                        if ui.add(egui::Button::new(RichText::new("Open Settings").font(FontId::proportional(fs - 1.0)).color(self.pal.accent))
                            .fill(tint(self.pal.accent, 20)).stroke(Stroke::new(1.0, tint(self.pal.accent, 60))).rounding(4.0)
                        ).clicked() { self.show_settings = true; }
                    });
            }
            SearchState::Done => {
                let raw    = self.get_results();
                let sorted = self.filtered_sorted(&raw);
                let total  = sorted.len();

                if total == 0 {
                    ui.add_space(40.0);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("No results match your filters.").font(FontId::proportional(17.0)).color(self.pal.subtext));
                        if !raw.is_empty() {
                            ui.label(RichText::new(format!("{} results hidden by filters", raw.len())).font(FontId::proportional(fs)).color(self.pal.dim));
                        }
                    });
                    return;
                }

                let pg     = self.page;
                let max_p  = self.max_pages(total);
                let page_s = self.page_slice(&sorted).to_vec();
                let page_n = page_s.len();

                // Stats + category bar
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    let active: usize = sorted.iter().filter(|r| r.seeders.unwrap_or(0)>0).count();
                    let total_seed: u32 = sorted.iter().map(|r| r.seeders.unwrap_or(0)).sum();
                    let trackers: std::collections::HashSet<_> = sorted.iter()
                        .filter_map(|r| r.tracker.as_deref()).collect();
                    ui.label(RichText::new(
                        format!("{page_n} of {total}  ·  {active} active  ·  {} seeds  ·  {} trackers",
                            total_seed, trackers.len())
                    ).font(FontId::proportional(fs - 1.0)).color(self.pal.subtext));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(12.0);
                        let sc = sorted.clone();
                        if ui.add(egui::Button::new(
                            RichText::new("Export CSV").font(FontId::proportional(fs - 1.0)).color(self.pal.subtext))
                            .fill(Color32::TRANSPARENT).stroke(Stroke::new(1.0, self.pal.border)).rounding(4.0)
                        ).clicked() { self.export_csv(&sc); self.toast("Exported to Downloads", self.pal.green); }
                    });
                });

                // Category breakdown bar
                if self.cfg.show_cat_bar {
                    let breakdown = App::cat_breakdown(&sorted);
                    if !breakdown.is_empty() {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            for (cat, count) in &breakdown {
                                let col = cat_color(cat);
                                egui::Frame::none()
                                    .fill(tint(col, 25)).rounding(10.0)
                                    .inner_margin(egui::Margin { left: 7.0, right: 7.0, top: 2.0, bottom: 2.0 })
                                    .show(ui, |ui| {
                                        ui.label(RichText::new(format!("{cat} {count}"))
                                            .font(FontId::proportional(11.0)).color(col));
                                    });
                                ui.add_space(3.0);
                            }
                        });
                    }
                }

                ui.add_space(4.0);


                // Pagination (bottom)
                egui::TopBottomPanel::bottom("pagination")
                    .exact_height(if max_p > 1 { 38.0 } else { 0.0 })
                    .frame(egui::Frame::none().fill(self.pal.bg)
                        .inner_margin(egui::Margin::symmetric(12.0, 6.0))
                        .stroke(Stroke::new(1.0, self.pal.border)))
                    .show_inside(ui, |ui| {
                        if max_p > 1 {
                            ui.horizontal(|ui| {
                                if ui.add_enabled(pg > 0,
                                    egui::Button::new(RichText::new("Prev").font(FontId::proportional(fs - 1.0)).color(self.pal.subtext))
                                    .fill(Color32::TRANSPARENT).stroke(Stroke::new(1.0, self.pal.border)).rounding(4.0)
                                ).clicked() { self.page -= 1; self.selected = None; }
                                ui.add_space(6.0);
                                for p in 0..max_p {
                                    let show = p == 0 || p == max_p-1 || p.abs_diff(pg) <= 2;
                                    if !show { if p == 1 || p == max_p-2 {
                                        ui.label(RichText::new("…").font(FontId::proportional(fs - 1.0)).color(self.pal.dim)); }
                                        continue;
                                    }
                                    let active = p == pg;
                                    if ui.add(egui::SelectableLabel::new(active,
                                        RichText::new(format!("{}", p+1)).font(FontId::proportional(fs - 1.0))
                                            .color(if active { self.pal.accent } else { self.pal.subtext })
                                    )).clicked() { self.page = p; self.selected = None; }
                                }
                                ui.add_space(6.0);
                                if ui.add_enabled(pg+1 < max_p,
                                    egui::Button::new(RichText::new("Next").font(FontId::proportional(fs - 1.0)).color(self.pal.subtext))
                                    .fill(Color32::TRANSPARENT).stroke(Stroke::new(1.0, self.pal.border)).rounding(4.0)
                                ).clicked() { self.page += 1; self.selected = None; }
                                ui.label(RichText::new(format!("Page {}/{max_p}", pg+1)).font(FontId::proportional(fs - 1.0)).color(self.pal.dim));
                            });
                        }
                    });

                // Detail panel
                if self.detail_open {
                    if let Some(idx) = self.selected {
                        if let Some(r) = page_s.get(idx).cloned() {
                            egui::SidePanel::right("detail")
                                .resizable(true).default_width(290.0).min_width(240.0)
                                .frame(egui::Frame::none().fill(self.pal.surface).stroke(Stroke::new(1.0, self.pal.border)))
                                .show_inside(ui, |ui| { self.draw_detail(ui, &r); });
                        }
                    }
                }

                // ── Keyboard shortcuts for result rows ──
                if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                    self.selected = Some(self.selected.map_or(0, |s| (s+1).min(page_n.saturating_sub(1))));
                    self.detail_open = self.selected.is_some();
                }
                if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                    self.selected = Some(self.selected.map_or(0, |s| s.saturating_sub(1)));
                    self.detail_open = self.selected.is_some();
                }
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if let Some(idx) = self.selected {
                        if let Some(r) = page_s.get(idx) {
                            if let Some(m) = &r.magnet_uri { let _ = open::that(m); self.toast("Opening magnet…", self.pal.accent); }
                        }
                    }
                }
                if ui.input(|i| i.key_pressed(egui::Key::D)) {
                    if self.selected.is_some() { self.detail_open = !self.detail_open; }
                }
                if ui.input(|i| i.key_pressed(egui::Key::F)) {
                    if let Some(idx) = self.selected {
                        if let Some(r) = page_s.get(idx).cloned() { self.save_fav(&r); }
                    }
                }
                if ui.input(|i| i.key_pressed(egui::Key::M)) {
                    if let Some(idx) = self.selected {
                        if let Some(r) = page_s.get(idx) {
                            if let Some(m) = &r.magnet_uri { let _ = open::that(m); self.toast("Opening…", self.pal.accent); }
                        }
                    }
                }

                // ── Table ──
                let mut actions: Vec<(usize, &'static str)> = vec![];
                let pal = self.pal.clone();
                let sort_col = self.sort_col.clone();
                let sort_dir = self.sort_dir.clone();
                let rh  = self.cfg.row_height;
                let fsz = self.cfg.font_size;
                let cfg = self.cfg.clone();

                let table_area = ui.available_rect_before_wrap();
                let mut table_ui = ui.child_ui(table_area, egui::Layout::top_down(egui::Align::LEFT));
                egui::ScrollArea::horizontal().id_source("t_scroll").show(&mut table_ui, |ui| {
                    let mut tb = TableBuilder::new(ui)
                        .striped(false).resizable(true)
                        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                        .column(Column::initial(300.0).at_least(160.0).clip(true));  // Name always
                    if cfg.col_tracker { tb = tb.column(Column::initial(88.0).at_least(55.0)); }
                    if cfg.col_size    { tb = tb.column(Column::initial(76.0).at_least(50.0)); }
                    tb = tb.column(Column::initial(68.0).at_least(44.0));             // Seeds always
                    if cfg.col_leech  { tb = tb.column(Column::initial(68.0).at_least(44.0)); }
                    if cfg.col_ratio  { tb = tb.column(Column::initial(58.0).at_least(44.0)); }
                    if cfg.col_health { tb = tb.column(Column::initial(80.0).at_least(50.0)); }
                    if cfg.col_date   { tb = tb.column(Column::initial(88.0).at_least(60.0)); }
                    let tb = tb.column(Column::remainder().at_least(160.0));          // Actions always

                    let mut new_sort: Option<(SortCol, bool)> = None;
                    let hdr = |lbl: &str, col: &SortCol| {
                        let active = &sort_col == col;
                        let arrow  = if active { if sort_dir == SortDir::Desc {" v"} else {" ^"} } else {""};
                        RichText::new(format!("{lbl}{arrow}")).font(FontId::proportional(fsz))
                            .color(if active { pal.accent } else { pal.subtext }).strong()
                    };

                    tb.header(30.0, |mut h| {
                        h.col(|ui| { if ui.add(egui::Button::new(hdr("Name",&SortCol::Name)).fill(Color32::TRANSPARENT).frame(false)).clicked() { new_sort=Some((SortCol::Name,sort_col==SortCol::Name)); } });
                        if cfg.col_tracker { h.col(|ui| { if ui.add(egui::Button::new(hdr("Tracker",&SortCol::Tracker)).fill(Color32::TRANSPARENT).frame(false)).clicked() { new_sort=Some((SortCol::Tracker,sort_col==SortCol::Tracker)); } }); }
                        if cfg.col_size { h.col(|ui| { if ui.add(egui::Button::new(hdr("Size",&SortCol::Size)).fill(Color32::TRANSPARENT).frame(false)).clicked() { new_sort=Some((SortCol::Size,sort_col==SortCol::Size)); } }); }
                        h.col(|ui| { if ui.add(egui::Button::new(hdr("Seeds",&SortCol::Seeders)).fill(Color32::TRANSPARENT).frame(false)).clicked() { new_sort=Some((SortCol::Seeders,sort_col==SortCol::Seeders)); } });
                        if cfg.col_leech { h.col(|ui| { if ui.add(egui::Button::new(hdr("Leech",&SortCol::Leechers)).fill(Color32::TRANSPARENT).frame(false)).clicked() { new_sort=Some((SortCol::Leechers,sort_col==SortCol::Leechers)); } }); }
                        if cfg.col_ratio  { h.col(|ui| { ui.label(RichText::new("Ratio").font(FontId::proportional(fsz)).color(pal.subtext).strong()); }); }
                        if cfg.col_health { h.col(|ui| { ui.label(RichText::new("Health").font(FontId::proportional(fsz)).color(pal.subtext).strong()); }); }
                        if cfg.col_date { h.col(|ui| { if ui.add(egui::Button::new(hdr("Date",&SortCol::Date)).fill(Color32::TRANSPARENT).frame(false)).clicked() { new_sort=Some((SortCol::Date,sort_col==SortCol::Date)); } }); }
                        h.col(|ui| { ui.label(RichText::new("Actions").font(FontId::proportional(fsz)).color(pal.subtext).strong()); });
                    })
                    .body(|mut body| {
                        for (i, r) in page_s.iter().enumerate() {
                            let sel   = self.selected == Some(i);
                            let hov   = self.hovered_row == Some(i);
                            let seed  = r.seeders.unwrap_or(0);
                            let leech = r.peers.unwrap_or(0);
                            let bg = if sel      { pal.row_sel }
                                     else if hov { pal.row_hover }
                                     else if i % 2 == 0 { pal.row_odd }
                                     else { pal.row_even };

                            body.row(rh, |mut row| {
                                // Name
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                                    let resp = ui.add(egui::Label::new(
                                        RichText::new(&r.title).font(FontId::proportional(fsz))
                                            .color(if sel { pal.accent } else { pal.text })
                                    ).truncate(true).sense(egui::Sense::click()));
                                    if resp.clicked()  { actions.push((i,"select")); }
                                    if resp.hovered()  {
                                        actions.push((i,"hover"));
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    if rh >= 40.0 {
                                        ui.horizontal(|ui| {
                                            let cat = r.category_desc.as_deref().unwrap_or("Other");
                                            ui.add(egui::Label::new(RichText::new(cat).font(FontId::proportional(fsz-2.0)).color(cat_color(cat))).truncate(true));
                                        });
                                    }
                                });
                                // Tracker (conditional)
                                if cfg.col_tracker {
                                    row.col(|ui| {
                                        ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                                        let trk = r.tracker.as_deref().unwrap_or("—");
                                        ui.add(egui::Label::new(RichText::new(trk).font(FontId::proportional(fsz-1.0)).color(pal.subtext)).truncate(true));
                                    });
                                }
                                // Size (conditional)
                                if cfg.col_size {
                                    row.col(|ui| {
                                        ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                                        ui.label(RichText::new(r.size.map(fmt_bytes).unwrap_or("—".into())).font(FontId::proportional(fsz)).color(pal.subtext));
                                    });
                                }
                                // Seeds (always)
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                                    ui.label(RichText::new(seed.to_string()).font(FontId::proportional(fsz)).color(seed_color(seed)).strong());
                                });
                                // Leechers (conditional)
                                if cfg.col_leech {
                                    row.col(|ui| {
                                        ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                                        ui.label(RichText::new(leech.to_string()).font(FontId::proportional(fsz)).color(pal.red));
                                    });
                                }
                                // Ratio bar (conditional)
                                if cfg.col_ratio {
                                    row.col(|ui| {
                                        ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                                        let total = (seed + leech) as f32;
                                        if total > 0.0 {
                                            let pct  = (seed as f32 / total).clamp(0.0, 1.0);
                                            let rect = ui.available_rect_before_wrap();
                                            let bar  = egui::Rect::from_min_size(
                                                rect.min + Vec2::new(2.0, (rect.height()-7.0)/2.0),
                                                Vec2::new((rect.width()-4.0).max(8.0), 7.0));
                                            ui.painter().rect_filled(bar, 4.0, pal.border);
                                            let mut filled = bar; filled.max.x = bar.min.x + bar.width()*pct;
                                            ui.painter().rect_filled(filled, 4.0, seed_color(seed));
                                            ui.allocate_rect(bar, egui::Sense::hover())
                                                .on_hover_text(format!("{:.0}% seeded", pct*100.0));
                                        } else {
                                            ui.label(RichText::new("—").font(FontId::proportional(fsz-1.0)).color(pal.dim));
                                        }
                                    });
                                }
                                // Health (conditional)
                                if cfg.col_health {
                                    row.col(|ui| {
                                        ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                                        let hd = if seed > 10 {"●"} else {"○"};
                                        ui.label(RichText::new(format!("{hd} {}", health_label(seed))).font(FontId::proportional(fsz-1.0)).strong().color(seed_color(seed)));
                                    });
                                }
                                // Date (conditional)
                                if cfg.col_date {
                                    row.col(|ui| {
                                        ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                                        let ds = r.publish_date.as_deref().map(time_ago).unwrap_or("—".into());
                                        ui.label(RichText::new(ds).font(FontId::proportional(fsz)).color(pal.dim));
                                    });
                                }
                                // Actions (always)
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                                    ui.horizontal(|ui| {
                                        ui.add_space(2.0);
                                        if r.magnet_uri.is_some() {
                                            if act_btn(ui,"Mag","Open magnet in torrent client", pal.accent)  { actions.push((i,"mag")); }
                                            if act_btn(ui,"Copy","Copy magnet link",pal.subtext) { actions.push((i,"copy")); }
                                        }
                                        if r.link.is_some()    { if act_btn(ui,"DL","Download .torrent", pal.green)   { actions.push((i,"dl")); } }
                                        if act_btn(ui,"Fav","Save to favorites (F)", pal.yellow) { actions.push((i,"fav")); }
                                        if act_btn(ui,"Info","Detail panel (D)", if sel && self.detail_open { pal.accent } else { pal.dim }) { actions.push((i,"info")); }
                                        if r.details.is_some() { if act_btn(ui,"Web","Open in browser", pal.dim) { actions.push((i,"web")); } }
                                    });
                                });
                            });
                        }
                    });

                    if let Some((col, same)) = new_sort {
                        if same { self.sort_dir = if self.sort_dir==SortDir::Desc { SortDir::Asc } else { SortDir::Desc }; }
                        else { self.sort_col = col; self.sort_dir = SortDir::Desc; }
                        self.page = 0;
                    }
                });

                // Process actions + hover
                let mut new_hover: Option<usize> = None;
                for (i, action) in actions {
                    if action == "hover" { new_hover = Some(i); continue; }
                    if let Some(r) = page_s.get(i).cloned() {
                        match action {
                            "select" => { if self.selected==Some(i) { self.selected=None; self.detail_open=false; } else { self.selected=Some(i); self.detail_open=true; } }
                            "mag"  => { if let Some(m) = &r.magnet_uri { let _ = open::that(m); self.toast("Opening magnet…", self.pal.accent); } }
                            "copy" => { if let Some(m) = &r.magnet_uri { ui.output_mut(|o| o.copied_text=m.clone()); self.toast("Magnet copied!", self.pal.green); } }
                            "dl"   => { if let Some(l) = &r.link  { let _ = open::that(l); self.toast("Downloading…", self.pal.green); } }
                            "fav"  => { self.save_fav(&r); }
                            "info" => { if self.selected==Some(i) && self.detail_open { self.detail_open=false; self.selected=None; } else { self.selected=Some(i); self.detail_open=true; } }
                            "web"  => { if let Some(d) = &r.details { let _ = open::that(d); } }
                            _ => {}
                        }
                    }
                }
                self.hovered_row = new_hover;
            }
        }
        let _ = ctx;
    }

    fn draw_idle(&mut self, ui: &mut egui::Ui) {
        let fs = self.cfg.font_size;
        ui.add_space(50.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("Torrent X").font(FontId::proportional(32.0)).color(tint(self.pal.accent, 60)).strong());
            ui.add_space(6.0);
            ui.label(RichText::new("Search across all your configured Jackett indexers")
                .font(FontId::proportional(fs + 2.0)).color(self.pal.subtext));
            ui.add_space(4.0);
            ui.label(RichText::new("Movies  ·  TV  ·  Music  ·  Games  ·  Software  ·  Anime  ·  Books")
                .font(FontId::proportional(fs)).color(self.pal.dim));

            if !self.cfg.search_history.is_empty() {
                ui.add_space(28.0);
                ui.label(RichText::new("Recent searches").font(FontId::proportional(fs - 1.0)).color(self.pal.dim));
                ui.add_space(10.0);
                let hist: Vec<String> = self.cfg.search_history.iter().take(10).cloned().collect();
                let mut clicked_h: Option<String> = None;
                // Centered chips
                let chip_sp = 8.0;
                let avail_w = ui.available_width();
                let total_w: f32 = hist.iter().map(|h| h.len() as f32 * 8.5 + 28.0 + chip_sp).sum::<f32>() - chip_sp;
                let left_pad = ((avail_w - total_w.min(avail_w - 32.0)) * 0.5).max(0.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(chip_sp, chip_sp);
                    ui.add_space(left_pad);
                    for h in &hist {
                        if ui.add(egui::Button::new(RichText::new(h.as_str()).font(FontId::proportional(fs)).color(self.pal.subtext))
                            .fill(self.pal.surface).stroke(Stroke::new(1.0, self.pal.border))
                            .rounding(16.0).min_size(egui::vec2(0.0, 30.0))
                        ).clicked() { clicked_h = Some(h.clone()); }
                    }
                });
                if let Some(h) = clicked_h { self.query = h; self.search(); }
            } else {
                // Quick-start suggestions for new users
                ui.add_space(28.0);
                ui.label(RichText::new("Try searching for…").font(FontId::proportional(fs - 1.0)).color(self.pal.dim));
                ui.add_space(10.0);
                let suggestions = ["Linux Mint", "Ubuntu", "Blender", "Inkscape", "GIMP"];
                let mut clicked_s: Option<&str> = None;
                let chip_sp = 8.0;
                let avail_w = ui.available_width();
                let total_w: f32 = suggestions.iter().map(|h| h.len() as f32 * 8.5 + 28.0 + chip_sp).sum::<f32>() - chip_sp;
                let left_pad = ((avail_w - total_w.min(avail_w - 32.0)) * 0.5).max(0.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(chip_sp, chip_sp);
                    ui.add_space(left_pad);
                    for s in &suggestions {
                        if ui.add(egui::Button::new(RichText::new(*s).font(FontId::proportional(fs)).color(self.pal.dim))
                            .fill(self.pal.surface).stroke(Stroke::new(1.0, tint(self.pal.border, 180)))
                            .rounding(16.0).min_size(egui::vec2(0.0, 28.0))
                        ).clicked() { clicked_s = Some(s); }
                    }
                });
                if let Some(s) = clicked_s { self.query = s.to_string(); self.search(); }
            }

            ui.add_space(36.0);
            // Quickstart tip
            egui::Frame::none()
                .fill(tint(self.pal.accent, 12)).rounding(8.0)
                .stroke(Stroke::new(1.0, tint(self.pal.accent, 40)))
                .inner_margin(egui::Margin::symmetric(20.0, 12.0))
                .show(ui, |ui| {
                    ui.set_max_width(480.0);
                    ui.label(RichText::new("First time?").font(FontId::proportional(fs)).color(self.pal.accent).strong());
                    ui.add_space(4.0);
                    ui.label(RichText::new("Open Settings, paste your Jackett API key (found at localhost:9117 top-right), then search!")
                        .font(FontId::proportional(fs - 1.0)).color(self.pal.subtext));
                    ui.add_space(6.0);
                    if ui.add(egui::Button::new(RichText::new("Open Settings").font(FontId::proportional(fs - 1.0)).color(self.pal.accent))
                        .fill(tint(self.pal.accent, 20)).stroke(Stroke::new(1.0, tint(self.pal.accent, 60))).rounding(4.0)
                    ).clicked() { self.show_settings = true; }
                });
        });
    }

    // ─── Detail panel ────────────────────────────────────────────────────────

    fn draw_detail(&mut self, ui: &mut egui::Ui, r: &TorrentResult) {
        let fs = self.cfg.font_size;
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.label(RichText::new("Details").font(FontId::proportional(fs + 2.0)).color(self.pal.subtext).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                if ui.add(egui::Button::new(RichText::new("x").color(self.pal.dim))
                    .fill(Color32::TRANSPARENT).rounding(4.0)).clicked() {
                    self.detail_open = false; self.selected = None;
                }
            });
        });
        ui.add_space(4.0); ui.separator(); ui.add_space(10.0);

        egui::ScrollArea::vertical().id_source("det_scr").show(ui, |ui| {
            ui.add_space(4.0);
            ui.add(egui::Label::new(RichText::new(&r.title).font(FontId::proportional(fs)).color(self.pal.text).strong()).wrap(true));
            ui.add_space(12.0);

            let cat = r.category_desc.as_deref().unwrap_or("?");
            drow(ui, "Category",  cat, cat_color(cat), &self.pal, fs);
            if let Some(t) = &r.tracker  { drow(ui, "Tracker",   t, self.pal.subtext, &self.pal, fs); }
            if let Some(s) = r.size      { drow(ui, "Size",  &fmt_bytes(s), self.pal.subtext, &self.pal, fs); }
            let s = r.seeders.unwrap_or(0);
            let l = r.peers.unwrap_or(0);
            drow(ui, "Seeders",  &s.to_string(), seed_color(s), &self.pal, fs);
            drow(ui, "Leechers", &l.to_string(), self.pal.red,  &self.pal, fs);
            let ratio = if l > 0 { format!("{:.2}", s as f64/l as f64) } else { "inf".into() };
            drow(ui, "Ratio",    &ratio, self.pal.subtext, &self.pal, fs);
            drow(ui, "Health",   health_label(s), seed_color(s), &self.pal, fs);
            if let Some(d) = &r.publish_date { drow(ui, "Published", &time_ago(d), self.pal.dim, &self.pal, fs); }

            ui.add_space(14.0);
            ui.label(RichText::new("Actions").font(FontId::proportional(fs - 1.0)).color(self.pal.dim).strong());
            ui.add_space(6.0);

            if let Some(mag) = r.magnet_uri.clone() {
                let mc = mag.clone();
                if fbtn(ui, "Open Magnet", self.pal.accent) { let _ = open::that(mag); self.toast("Opening magnet…", self.pal.accent); }
                ui.add_space(3.0);
                if fbtn(ui, "Copy Magnet", self.pal.subtext) {
                    ui.output_mut(|o| o.copied_text = mc);
                    self.toast("Copied to clipboard!", self.pal.subtext);
                }
                ui.add_space(3.0);
            }
            if let Some(link) = r.link.clone() {
                if fbtn(ui, "Download .torrent", self.pal.green) { let _ = open::that(link); }
                ui.add_space(3.0);
            }
            if let Some(det) = r.details.clone() {
                if fbtn(ui, "Open in Browser", self.pal.subtext) { let _ = open::that(det); }
                ui.add_space(3.0);
            }
            let r2 = r.clone();
            if fbtn(ui, "Add to Favorites", self.pal.yellow) { self.save_fav(&r2); }
        });
    }

    // ─── Favorites tab ───────────────────────────────────────────────────────

    fn draw_favorites(&mut self, ui: &mut egui::Ui) {
        let fs = self.cfg.font_size;
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            ui.label(RichText::new(format!("Favorites  ({})", self.cfg.favorites.len()))
                .font(FontId::proportional(17.0)).color(self.pal.text).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(16.0);
                if !self.cfg.favorites.is_empty() {
                    if ui.add(egui::Button::new(RichText::new("Clear all").font(FontId::proportional(fs - 1.0)).color(self.pal.red))
                        .fill(Color32::TRANSPARENT).rounding(4.0)).clicked() {
                        self.cfg.favorites.clear(); save_config(&self.cfg);
                    }
                }
            });
        });
        ui.add_space(10.0);

        if self.cfg.favorites.is_empty() {
            ui.add_space(60.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("No favorites yet").font(FontId::proportional(18.0)).color(self.pal.subtext));
                ui.add_space(4.0);
                ui.label(RichText::new("Click 'Star' on any result to save it here")
                    .font(FontId::proportional(fs)).color(self.pal.dim));
            });
            return;
        }

        // Search within favorites
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label(RichText::new("Search:").font(FontId::proportional(fs - 1.0)).color(self.pal.dim));
            ui.add_space(4.0);
            ui.add(egui::TextEdit::singleline(&mut self.fav_search)
                .desired_width(220.0).hint_text("filter favorites…").font(FontId::proportional(fs)));
            if !self.fav_search.is_empty() {
                if ui.add(egui::Button::new(RichText::new("x").size(12.0).color(self.pal.subtext))
                    .fill(Color32::TRANSPARENT).frame(false)).clicked() {
                    self.fav_search.clear();
                }
            }
        });
        ui.add_space(8.0);
        let mut remove: Option<usize> = None;
        let mut omag:   Option<String> = None;
        let mut olink:  Option<String> = None;
        let fq = self.fav_search.to_lowercase();

        egui::ScrollArea::vertical().show(ui, |ui| {
            let favs = self.cfg.favorites.clone();
            let mut vis_i = 0usize;
            for (i, fav) in favs.iter().enumerate() {
                if !fq.is_empty() && !fav.title.to_lowercase().contains(&fq)
                    && !fav.tracker.as_deref().unwrap_or("").to_lowercase().contains(&fq) { continue; }
                vis_i += 1;
                let bg = if vis_i % 2 == 0 { self.pal.row_odd } else { self.pal.row_even };
                egui::Frame::none().fill(bg).inner_margin(egui::Margin::symmetric(16.0, 10.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.set_min_width(ui.available_width() - 130.0);
                                ui.add(egui::Label::new(RichText::new(&fav.title).font(FontId::proportional(fs)).color(self.pal.text)).truncate(true));
                                ui.horizontal(|ui| {
                                    if let Some(t) = &fav.tracker {
                                        ui.label(RichText::new(t).font(FontId::proportional(fs - 1.0)).color(self.pal.subtext));
                                    }
                                    if let Some(s) = fav.size {
                                        ui.label(RichText::new(format!("· {}", fmt_bytes(s))).font(FontId::proportional(fs - 1.0)).color(self.pal.dim));
                                    }
                                    if let Some(s) = fav.seeders {
                                        ui.label(RichText::new(format!("· {} seeds", s)).font(FontId::proportional(fs - 1.0)).color(seed_color(s)));
                                    }
                                    if !fav.saved_at.is_empty() {
                                        ui.label(RichText::new(format!("· saved {}", fav.saved_at)).font(FontId::proportional(fs - 2.0)).color(self.pal.dim));
                                    }
                                });
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if act_btn(ui, "Del",  "Remove from favorites", self.pal.red)   { remove = Some(i); }
                                if fav.link.is_some()   { if act_btn(ui,"DL","Download .torrent", self.pal.green)  { olink = fav.link.clone(); } }
                                if fav.magnet.is_some() { if act_btn(ui,"Mag","Open magnet",self.pal.accent) { omag = fav.magnet.clone(); } }
                            });
                        });
                    });
                ui.separator();
            }
        });

        if let Some(i) = remove { self.cfg.favorites.remove(i); save_config(&self.cfg); }
        if let Some(m) = omag   { let _ = open::that(m); self.toast("Opening magnet…", self.pal.accent); }
        if let Some(l) = olink  { let _ = open::that(l); self.toast("Downloading…", self.pal.green); }
    }

    // ─── About tab ───────────────────────────────────────────────────────────

    fn draw_about(&self, ui: &mut egui::Ui) {
        let fs = self.cfg.font_size;
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("TorrentX").font(FontId::proportional(28.0)).color(self.pal.text).strong());
                ui.label(RichText::new("v6.0").font(FontId::proportional(fs)).color(self.pal.accent));
                ui.add_space(4.0);
                ui.label(RichText::new("Native Rust + egui torrent search GUI powered by Jackett")
                    .font(FontId::proportional(fs + 1.0)).color(self.pal.subtext));

                ui.add_space(24.0);
                for (k,v) in [("Language","Rust 2021 edition"),("GUI","egui 0.27 + egui_extras"),
                              ("Rendering","GPU (wgpu/Vulkan or OpenGL)"),
                              ("HTTP","reqwest blocking"),("Config","~/.config/torrentx/config.json")] {
                    ui.horizontal(|ui| {
                        ui.add_space(ui.available_width() * 0.2);
                        ui.label(RichText::new(format!("{k:<18}")).font(FontId::proportional(fs)).color(self.pal.dim));
                        ui.label(RichText::new(v).font(FontId::proportional(fs)).color(self.pal.subtext));
                    });
                    ui.add_space(3.0);
                }

                // Theme swatches
                ui.add_space(24.0);
                ui.label(RichText::new("Themes").font(FontId::proportional(fs + 1.0)).color(self.pal.accent).strong());
                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                    ui.add_space(60.0);
                    for t in Theme::all() {
                        let col = t.preview_color();
                        let active = &self.cfg.theme == t;
                        egui::Frame::none()
                            .fill(tint(col, if active { 40 } else { 20 }))
                            .rounding(6.0)
                            .stroke(Stroke::new(if active {2.0} else {1.0}, tint(col, if active {220} else {100})))
                            .inner_margin(egui::Margin { left: 10.0, right: 10.0, top: 5.0, bottom: 5.0 })
                            .show(ui, |ui| {
                                ui.label(RichText::new(t.name()).font(FontId::proportional(fs - 1.0)).color(col));
                            });
                    }
                });

                ui.add_space(24.0);
                ui.label(RichText::new("Features").font(FontId::proportional(fs + 1.0)).color(self.pal.accent).strong());
                ui.add_space(8.0);
                for f in [
                    "Real-time search via Jackett API (all indexers at once)",
                    "13 themes including Catppuccin, One Dark, Rose Pine, Kanagawa, Ayu",
                    "Row density: Compact / Normal / Roomy",
                    "Font size: Small / Medium / Large",
                    "Animated spinner + elapsed time during search",
                    "Category breakdown bar after search results",
                    "Clickable column headers to sort with direction indicator",
                    "Filter by text, min seeds, max size, tracker, health",
                    "Deduplication across trackers",
                    "Search history with one-click re-search",
                    "Favorites with save date, persistent across restarts",
                    "Detail side panel with all actions per result",
                    "Copy magnet to clipboard",
                    "Export results as CSV to Downloads",
                    "Pagination (25 / 50 / 100 / All)",
                    "Keyboard navigation: Up/Down, Enter=open magnet, Ctrl+R=re-search",
                ] {
                    ui.label(RichText::new(format!("  · {f}")).font(FontId::proportional(fs - 1.0)).color(self.pal.subtext));
                    ui.add_space(2.0);
                }

                ui.add_space(20.0);
                ui.label(RichText::new("Keyboard Shortcuts").font(FontId::proportional(fs + 1.0)).color(self.pal.accent).strong());
                ui.add_space(8.0);
                for (k,v) in [("Enter","Search / open magnet when row selected"),
                              ("D","Toggle detail panel for selected row"),
                              ("F","Save selected row to favorites"),
                              ("M","Open magnet for selected row"),
                              ("Escape","Clear search input"),
                              ("Ctrl+F","Focus search bar from anywhere"),
                              ("Ctrl+R","Re-run last search"),
                              ("Up / Down","Navigate result rows")] {
                    ui.horizontal(|ui| {
                        ui.add_space(ui.available_width() * 0.2);
                        ui.add(egui::Button::new(RichText::new(k).font(FontId::proportional(fs)).color(self.pal.accent))
                            .fill(self.pal.surface).stroke(Stroke::new(1.0, self.pal.border)).rounding(4.0));
                        ui.add_space(8.0);
                        ui.label(RichText::new(v).font(FontId::proportional(fs)).color(self.pal.subtext));
                    });
                    ui.add_space(4.0);
                }
            });
        });
    }

    // ─── Toasts ──────────────────────────────────────────────────────────────

    fn draw_toasts(&self, ctx: &egui::Context) {
        if self.toasts.is_empty() { return; }
        let scr = ctx.screen_rect();
        let mut y = scr.max.y - 56.0;
        for toast in self.toasts.iter().rev() {
            let alpha = ((toast.timer.min(0.4) / 0.4) * 230.0) as u8;
            egui::Area::new(egui::Id::new(format!("t_{}", toast.msg)))
                .fixed_pos([scr.max.x - 300.0, y])
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::none()
                        .fill(tint(self.pal.surface, alpha))
                        .stroke(Stroke::new(1.5, tint(toast.color, alpha)))
                        .rounding(8.0).inner_margin(egui::Margin::symmetric(14.0, 9.0))
                        .show(ui, |ui| {
                            ui.label(RichText::new(&toast.msg)
                                .font(FontId::proportional(14.0)).color(tint(toast.color, alpha)));
                        });
                });
            y -= 46.0;
        }
    }

    // ─── Theme ───────────────────────────────────────────────────────────────

    fn apply_theme(&self, ctx: &egui::Context) {
        let p = &self.pal;
        let mut vis = if p.is_light { Visuals::light() } else { Visuals::dark() };
        vis.panel_fill  = p.bg;
        vis.window_fill = p.bg;
        vis.faint_bg_color    = p.surface;
        vis.extreme_bg_color  = p.header_bg;
        vis.widgets.noninteractive.bg_fill = p.surface;
        vis.widgets.inactive.bg_fill       = p.surface;
        vis.widgets.hovered.bg_fill        = p.surface2;
        vis.widgets.active.bg_fill         = p.accent;
        vis.selection.bg_fill              = tint(p.accent, 50);
        vis.override_text_color            = Some(p.text);
        vis.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.dim);
        vis.widgets.inactive.fg_stroke       = Stroke::new(1.0, p.subtext);
        vis.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
        vis.widgets.inactive.bg_stroke       = Stroke::new(1.0, p.border);
        let r = egui::Rounding::same(6.0);
        vis.widgets.noninteractive.rounding = r;
        vis.widgets.inactive.rounding       = r;
        vis.widgets.hovered.rounding        = r;
        vis.widgets.active.rounding         = r;
        ctx.set_visuals(vis);
    }
}

// ─── UI helpers ───────────────────────────────────────────────────────────────

/// Small section label (e.g. "CONNECTION")
fn lbl_sec(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.label(RichText::new(text).font(FontId::proportional(10.0)).color(color).strong());
    ui.add_space(6.0);
}

/// Inline field label
fn slbl(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.label(RichText::new(text).font(FontId::proportional(12.0)).color(color));
}

/// Small outline button that returns true if clicked
fn sml_btn(label: &str, color: Color32) -> egui::Button<'static> {
    egui::Button::new(RichText::new(label.to_string()).font(FontId::proportional(12.0)).color(color))
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::new(1.0, tint(color, 80)))
        .rounding(4.0)
}

fn act_btn(ui: &mut egui::Ui, label: &str, tip: &str, color: Color32) -> bool {
    ui.add(
        egui::Button::new(RichText::new(label).size(12.0).color(color))
            .fill(tint(color, 18))
            .stroke(Stroke::new(1.0, tint(color, 70)))
            .rounding(5.0)
            .min_size(Vec2::new(0.0, 26.0))
    ).on_hover_text(tip).clicked()
}

fn fbtn(ui: &mut egui::Ui, label: &str, color: Color32) -> bool {
    ui.add(
        egui::Button::new(RichText::new(label).font(FontId::proportional(14.0)).color(color))
            .fill(tint(color, 18)).stroke(Stroke::new(1.0, tint(color, 80)))
            .rounding(6.0).min_size(Vec2::new(220.0, 34.0))
    ).clicked()
}

fn drow(ui: &mut egui::Ui, label: &str, value: &str, color: Color32, p: &Pal, fs: f32) {
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        ui.label(RichText::new(format!("{label:<13}")).font(FontId::proportional(fs - 1.0)).color(p.dim));
        ui.add(egui::Label::new(RichText::new(value).font(FontId::proportional(fs - 1.0)).color(color)).truncate(true));
    });
    ui.add_space(2.0);
}

// ─── Entry point ─────────────────────────────────────────────────────────────

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
