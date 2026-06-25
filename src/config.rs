use serde::{Deserialize, Serialize};
use std::fs;

use crate::rss::RssFeedConfig;
use crate::theme::Theme;

pub const ROW_H_COMPACT: f32 = 32.0;
pub const ROW_H_NORMAL:  f32 = 44.0;
pub const ROW_H_ROOMY:   f32 = 56.0;
pub const MARGIN:        f32 = 12.0;

// ─── Column width constants ────────────────────────────────────────────────

pub const COL_NAME:    f32 = 295.0;
pub const COL_TRACKER: f32 =  88.0;
pub const COL_SIZE:    f32 =  76.0;
pub const COL_SEEDS:   f32 =  66.0;
pub const COL_LEECH:   f32 =  66.0;
pub const COL_RATIO:   f32 =  58.0;
pub const COL_HEALTH:  f32 =  78.0;
pub const COL_DATE:    f32 =  88.0;

// ─── Config ────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    pub jackett_url:     String,
    pub api_key:         String,
    pub history:         Vec<String>,
    pub favorites:       Vec<Favorite>,
    pub rss_feeds:       Vec<RssFeedConfig>,
    pub theme:           Theme,
    pub timeout_secs:    u64,
    pub dedupe:          bool,
    pub page_size:       usize,
    pub row_height:      f32,
    pub font_size:       f32,
    pub show_cat_bar:    bool,
    pub rss_refresh_min: u64,
    // column toggles
    pub col_tracker: bool,
    pub col_size:    bool,
    pub col_leech:   bool,
    pub col_ratio:   bool,
    pub col_health:  bool,
    pub col_date:    bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            jackett_url:     "http://localhost:9117".into(),
            api_key:         String::new(),
            history:         vec![],
            favorites:       vec![],
            rss_feeds:       vec![],
            theme:           Theme::TokyoNight,
            timeout_secs:    45,
            dedupe:          false,
            page_size:       50,
            row_height:      ROW_H_NORMAL,
            font_size:       14.0,
            show_cat_bar:    true,
            rss_refresh_min: 30,
            col_tracker: true,
            col_size:    true,
            col_leech:   true,
            col_ratio:   true,
            col_health:  true,
            col_date:    true,
        }
    }
}

// ─── Favorite ──────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone)]
pub struct Favorite {
    pub title:    String,
    pub magnet:   Option<String>,
    pub link:     Option<String>,
    pub tracker:  Option<String>,
    pub size:     Option<u64>,
    pub seeders:  Option<u32>,
    #[serde(default)]
    pub saved_at: String,
}

// ─── I/O ───────────────────────────────────────────────────────────────────

pub fn cfg_path() -> std::path::PathBuf {
    let d = dirs_next::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("torrentx");
    let _ = fs::create_dir_all(&d);
    d.join("config.json")
}

pub fn load_cfg() -> Config {
    fs::read_to_string(cfg_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_cfg(c: &Config) {
    if let Ok(j) = serde_json::to_string_pretty(c) {
        let _ = fs::write(cfg_path(), j);
    }
}
