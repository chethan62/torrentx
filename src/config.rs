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
    /// Custom accent color (RGB) overriding the theme default; None = theme default.
    #[serde(default)]
    pub(crate) accent: Option<[u8; 3]>,
    /// Column display order (names of TableCol), left to right.
    #[serde(default)]
    pub(crate) col_order: Vec<String>,
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
            accent: None,
            col_order: default_col_order(),
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

/// Optional config-file override set by `--config <path>` (parsed in main()).
static CONFIG_OVERRIDE: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

pub(crate) fn set_config_override(p: std::path::PathBuf) {
    let _ = CONFIG_OVERRIDE.set(p);
}

pub(crate) fn cfg_path() -> std::path::PathBuf {
    if let Some(p) = CONFIG_OVERRIDE.get() {
        return p.clone();
    }
    let d = dirs_next::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("torrentx");
    let _ = fs::create_dir_all(&d);
    d.join("config.json")
}

pub(crate) fn load_cfg() -> Config {
    let mut c: Config = fs::read_to_string(cfg_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    // Heal old configs: empty col_order (pre-column-reorder) → default order.
    if c.col_order.is_empty() {
        c.col_order = default_col_order();
    }
    c
}

/// The canonical column order, used as the default and to heal old configs.
pub(crate) fn default_col_order() -> Vec<String> {
    vec![
        "Name".into(), "Tracker".into(), "Size".into(), "Seeds".into(),
        "Leech".into(), "Ratio".into(), "Health".into(), "Date".into(),
    ]
}

pub(crate) fn save_cfg(c: &Config) {
    if let Ok(j) = serde_json::to_string_pretty(c) {
        let _ = fs::write(cfg_path(), j);
    }
}

