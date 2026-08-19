// ─── Config ────────────────────────────────────────────────────────────────
use crate::rss::RssFeedConfig;
use crate::themes::Theme;
use serde::{Deserialize, Serialize};
use std::fs;
pub(crate) const ROW_HEIGHT_COMPACT: f32 = 32.0;
pub(crate) const ROW_HEIGHT_NORMAL: f32 = 44.0;
pub(crate) const ROW_HEIGHT_ROOMY: f32 = 56.0;

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct Config {
    pub(crate) jackett_url: String,
    pub(crate) api_key: String,
    pub(crate) history: Vec<String>,
    pub(crate) favorites: Vec<Favorite>,
    pub(crate) theme: Theme,
    pub(crate) timeout_secs: u64,
    pub(crate) dedupe: bool,
    pub(crate) page_size: usize,
    pub(crate) row_height: f32,
    pub(crate) font_size: f32,
    pub(crate) show_cat_bar: bool,
    pub(crate) col_tracker: bool,
    pub(crate) col_size: bool,
    pub(crate) col_leech: bool,
    pub(crate) col_ratio: bool,
    pub(crate) col_health: bool,
    pub(crate) col_date: bool,
    #[serde(default)]
    pub(crate) rss_feeds: Vec<RssFeedConfig>,
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
pub(crate) struct Favorite {
    pub(crate) title: String,
    pub(crate) magnet: Option<String>,
    pub(crate) link: Option<String>,
    pub(crate) tracker: Option<String>,
    pub(crate) size: Option<u64>,
    pub(crate) seeders: Option<u32>,
    #[serde(default)]
    pub(crate) saved_at: String,
}

pub(crate) fn cfg_path() -> std::path::PathBuf {
    let d = dirs_next::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("torrentx");
    let _ = fs::create_dir_all(&d);
    d.join("config.json")
}

pub(crate) fn load_cfg() -> Config {
    fs::read_to_string(cfg_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub(crate) fn save_cfg(c: &Config) {
    if let Ok(j) = serde_json::to_string_pretty(c) {
        let _ = fs::write(cfg_path(), j);
    }
}

