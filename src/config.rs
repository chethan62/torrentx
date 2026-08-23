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
    /// RSS auto-refresh interval in seconds (0 = never auto-refresh).
    #[serde(default = "default_rss_refresh")]
    pub(crate) rss_refresh_secs: u64,
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
            col_tracker: true,
            col_size: true,
            col_leech: true,
            col_ratio: true,
            col_health: true,
            col_date: true,
            rss_feeds: vec![],
            rss_refresh_secs: default_rss_refresh(),
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
    let path = cfg_path();
    let raw = fs::read_to_string(&path).ok();
    let mut c: Config = match &raw {
        Some(s) => match serde_json::from_str(s) {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "torrentx: failed to parse {} ({e}); using defaults",
                    path.display()
                );
                Config::default()
            }
        },
        None => {
            // Warn only for an explicit --config that's missing/unreadable, not
            // the normal first-run (no config yet) case.
            if CONFIG_OVERRIDE.get().is_some() {
                eprintln!(
                    "torrentx: config file {} not found; using defaults",
                    path.display()
                );
            }
            Config::default()
        }
    };
    // Heal old configs: empty col_order (pre-column-reorder) → default order.
    c = heal_col_order(c);
    c
}

/// Heal a config that's missing the column-order field (old configs).
/// Pure and testable: returns the config with a default col_order if empty.
pub(crate) fn heal_col_order(mut c: Config) -> Config {
    if c.col_order.is_empty() {
        c.col_order = default_col_order();
    }
    c
}

/// The canonical column order, used as the default and to heal old configs.
pub(crate) fn default_col_order() -> Vec<String> {
    vec![
        "Name".into(),
        "Tracker".into(),
        "Size".into(),
        "Seeds".into(),
        "Leech".into(),
        "Ratio".into(),
        "Health".into(),
        "Date".into(),
    ]
}

/// Default RSS auto-refresh interval (10 minutes, in seconds).
fn default_rss_refresh() -> u64 {
    600
}

pub(crate) fn save_cfg(c: &Config) {
    if let Ok(j) = serde_json::to_string_pretty(c) {
        let p = cfg_path();
        let _ = fs::write(&p, j);
        // Config holds the Jackett API key — keep it private to this user.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&p, fs::Permissions::from_mode(0o600));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{default_col_order, heal_col_order, Config};

    #[test]
    fn heal_fills_empty_col_order() {
        let mut c = Config::default();
        c.col_order.clear();
        let healed = heal_col_order(c);
        assert_eq!(healed.col_order, default_col_order());
        assert_eq!(healed.col_order.len(), 8);
        assert_eq!(healed.col_order[0], "Name");
    }

    #[test]
    fn heal_preserves_custom_order() {
        let healed = heal_col_order(Config {
            col_order: vec!["Seeds".into(), "Name".into()],
            ..Config::default()
        });
        assert_eq!(healed.col_order, vec!["Seeds", "Name"]);
    }

    #[test]
    fn default_config_has_full_col_order() {
        assert_eq!(Config::default().col_order, default_col_order());
    }
}
