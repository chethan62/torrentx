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
    /// Merge near-duplicate titles across trackers (the same release usually shows
    /// up from two indexers). ON by default: two identical rows waste a visit every
    /// time, and the merge keeps the best-seeded copy.
    #[serde(default = "default_true")]
    pub(crate) dedupe: bool,
    /// Indexers the user switched off. A search fans out over every other
    /// configured indexer; the name is kept so the choice survives restarts.
    #[serde(default)]
    pub(crate) disabled_indexers: Vec<String>,
    /// Results-table column widths keyed by column name (`"name"`, `"seeds"`, …).
    /// Keyed, not positional: hiding a column or reordering them must not throw
    /// the other widths away. Missing keys use the built-in width.
    #[serde(default)]
    pub(crate) col_widths: std::collections::BTreeMap<String, f32>,
    pub(crate) page_size: usize,
    pub(crate) row_height: f32,
    pub(crate) font_size: f32,
    /// Whole-interface scale (text, rows, icons) applied as pixels-per-point.
    /// 1.0 = native. `font_size` alone left the chrome — tabs, chips, headers —
    /// at its fixed literals, so raising it made the body text collide with a
    /// toolbar that never grew.
    #[serde(default = "default_ui_scale")]
    pub(crate) ui_scale: f32,
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
    /// Check GitHub for a newer release at startup (opt-out for privacy).
    #[serde(default = "default_true")]
    pub(crate) check_updates: bool,
    /// Player driven by direct play ("mpv" | "vlc"). Matched against
    /// `player::PLAYERS`; never interpolated into a command line.
    #[serde(default = "default_player")]
    pub(crate) player: String,
    /// Tab shown at startup ("Search" | "Favorites" | "Rss" | "About").
    #[serde(default)]
    pub(crate) last_tab: Option<String>,
    /// Last window size [w, h], restored at startup.
    #[serde(default)]
    pub(crate) win_size: Option<[f32; 2]>,
    /// Anonymous per-install identifier (UUID v4), generated on first run.
    /// Handy when triaging bug reports; never sent anywhere by the app.
    #[serde(default)]
    pub(crate) install_id: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            jackett_url: "http://localhost:9117".into(),
            api_key: String::new(),
            history: vec![],
            favorites: vec![],
            theme: Theme::TokyoNight,
            timeout_secs: 30,
            dedupe: true,
            disabled_indexers: Vec::new(),
            col_widths: std::collections::BTreeMap::new(),
            page_size: 50,
            row_height: ROW_HEIGHT_NORMAL,
            font_size: 14.0,
            ui_scale: 1.0,
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
            check_updates: true,
            player: default_player(),
            last_tab: None,
            win_size: None,
            install_id: String::new(),
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

/// Set when a config file existed but could not be parsed, so the UI can tell the
/// user their settings were reset and where the backup lives. stderr is invisible
/// in a GUI launch, and silently replacing a config loses favourites, history and
/// the API key.
pub(crate) static CONFIG_LOAD_WARNING: std::sync::OnceLock<String> = std::sync::OnceLock::new();

pub(crate) fn set_config_override(p: std::path::PathBuf) {
    let _ = CONFIG_OVERRIDE.set(p);
}

pub(crate) fn cfg_path() -> std::path::PathBuf {
    if let Some(p) = CONFIG_OVERRIDE.get() {
        return p.clone();
    }
    // Android has no XDG config dir, and the process cwd is "/" (unwritable),
    // so the config goes in the app's private storage instead.
    #[cfg(target_os = "android")]
    let base = crate::android_data_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    #[cfg(not(target_os = "android"))]
    let base = dirs_next::config_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let d = base.join("torrentx");
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
                // Never silently destroy the user's settings. Keep a copy of the
                // unparseable file first, so favourites, history and the Jackett
                // API key stay recoverable, and leave a note for the UI.
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                // Append to the file name so the backup is recognisable as one of
                // THIS config: `with_extension` would replace ".json", producing
                // "config.corrupt-123" instead of "config.json.corrupt-123".
                let mut name = path
                    .file_name()
                    .unwrap_or_else(|| std::ffi::OsStr::new("config.json"))
                    .to_os_string();
                name.push(format!(".corrupt-{ts}"));
                let backup = path.with_file_name(name);
                eprintln!(
                    "torrentx: failed to parse {} ({e}); using defaults",
                    path.display()
                );
                let note = if fs::copy(&path, &backup).is_ok() {
                    format!(
                        "Settings unreadable — kept a backup at {}",
                        backup.display()
                    )
                } else {
                    "Settings unreadable — using defaults".to_string()
                };
                let _ = CONFIG_LOAD_WARNING.set(note);
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
    // Stored widths are clamped when rendered, so a stale out-of-range value (an
    // old build stretched Name to 1143) made the first drag jump to the clamp
    // instead of following the pointer. Clamp once, on load, so what is saved is
    // what is drawn.
    c = heal_col_widths(c);
    // First run: mint the anonymous per-install GUID and persist it.
    if c.install_id.is_empty() {
        c.install_id = uuid::Uuid::new_v4().to_string();
        let _ = save_cfg(&c);
    }
    c
}

/// Bring stored column widths inside the same bounds the table renders with.
/// Pure and testable.
pub(crate) fn heal_col_widths(mut c: Config) -> Config {
    c.col_widths
        .values_mut()
        .for_each(|w| *w = clamp_col_width(*w));
    c
}

/// Heal a config whose column order is missing or incomplete (old configs, hand
/// edits). Pure and testable.
///
/// An EMPTY list gets the default order; a PARTIAL list gets the missing columns
/// appended. Without that, a column absent from the stored order could never be
/// displayed again — its Settings toggle would flip and nothing would appear.
pub(crate) fn heal_col_order(mut c: Config) -> Config {
    let known = default_col_order();
    if c.col_order.is_empty() {
        c.col_order = known;
        return c;
    }
    for name in &known {
        if !c.col_order.iter().any(|n| n.eq_ignore_ascii_case(name)) {
            c.col_order.push(name.clone());
        }
    }
    // Keep only real columns, and only once each.
    let mut seen = std::collections::HashSet::new();
    c.col_order
        .retain(|n| known.iter().any(|k| k == n) && seen.insert(n.clone()));
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
pub(crate) const UI_SCALE_MIN: f32 = 0.8;
pub(crate) const UI_SCALE_MAX: f32 = 1.6;

/// Clamp a persisted column width to the drag range. A hand-edited config cannot
/// produce a NaN or a zero-width column, which egui_extras panics on when
/// `at_least` ends up above the width.
pub(crate) fn clamp_col_width(w: f32) -> f32 {
    if w.is_finite() {
        w.clamp(60.0, 900.0)
    } else {
        120.0
    }
}

/// Clamp + snap a scale to the 5% steps the slider and the shortcuts use.
pub(crate) fn clamp_scale(v: f32) -> f32 {
    if !v.is_finite() {
        return 1.0;
    }
    let snapped = (v.clamp(UI_SCALE_MIN, UI_SCALE_MAX) * 20.0).round() / 20.0;
    snapped.clamp(UI_SCALE_MIN, UI_SCALE_MAX)
}

fn default_ui_scale() -> f32 {
    1.0
}

fn default_rss_refresh() -> u64 {
    600
}

fn default_true() -> bool {
    true
}

fn default_player() -> String {
    "mpv".into()
}

pub(crate) fn save_cfg(c: &Config) -> Result<(), String> {
    save_to(&cfg_path(), c)
}

/// Durable atomic save: temp file, fsync, rename, fsync the directory. Split
/// from `save_cfg` so the durability behaviour is testable without the global
/// config path.
fn save_to(p: &std::path::Path, c: &Config) -> Result<(), String> {
    let j = serde_json::to_string_pretty(c).map_err(|e| format!("serialize config: {e}"))?;
    // Write a sibling temp file and rename over the config: a plain fs::write
    // truncates first, so an interrupted save (kill, power loss) would leave an
    // unparseable config behind — which used to be replaced by defaults.
    let tmp = p.with_extension("json.tmp");
    // Write through a File so the bytes can be fsync'd BEFORE the rename: on a
    // crash the renamed name must never resolve to an empty file (ext4 with
    // data=writeback will do exactly that without the barrier), and the rename
    // itself is only durable once the directory is synced.
    let written = fs::File::create(&tmp).and_then(|mut f| {
        use std::io::Write;
        f.write_all(j.as_bytes())?;
        f.sync_all()
    });
    if let Err(e) = written {
        let msg = format!("write {}: {e}", tmp.display());
        eprintln!("torrentx: failed to save config: {msg}");
        return Err(msg);
    }
    // Config holds the Jackett API key — keep it private to this user.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
    }
    if let Err(e) = fs::rename(&tmp, p) {
        let _ = fs::remove_file(&tmp);
        let msg = format!("save {}: {e}", p.display());
        eprintln!("torrentx: failed to save config: {msg}");
        return Err(msg);
    }
    // Make the NAME durable, not just the contents. Best effort: a failure here
    // does not invalidate the saved config, it only weakens crash durability.
    #[cfg(unix)]
    if let Some(dir) = p.parent() {
        let _ = fs::File::open(dir).and_then(|d| d.sync_all());
    }
    Ok(())
}

#[cfg(test)]
mod col_width_tests {
    use super::*;

    /// Widths are keyed by column name so hiding or reordering a column does not
    /// throw the others away, and a config predating the field still parses.
    #[test]
    fn widths_are_keyed_and_a_config_without_them_still_parses() {
        let mut c = Config::default();
        c.col_widths.insert("name".into(), 300.0);
        c.col_widths.insert("seeds".into(), 80.0);
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(
            serde_json::from_str::<Config>(&json).unwrap().col_widths,
            c.col_widths
        );

        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        v.as_object_mut().unwrap().remove("col_widths");
        let old: Config = serde_json::from_str(&v.to_string()).unwrap();
        assert!(old.col_widths.is_empty());
    }

    #[test]
    fn clamp_kills_non_finite_values_and_holds_the_range() {
        assert_eq!(clamp_col_width(f32::NAN), 120.0);
        assert_eq!(clamp_col_width(5.0), 60.0);
        assert_eq!(clamp_col_width(2000.0), 900.0);
        assert_eq!(clamp_col_width(200.0), 200.0);
    }
}

#[cfg(test)]
mod dedupe_default_tests {
    use super::*;

    /// The same release comes back from two indexers, so an off-by-default dedupe
    /// meant the table showed it twice on every search.
    #[test]
    fn a_fresh_config_dedupes_and_a_missing_field_means_on() {
        assert!(Config::default().dedupe, "new installs dedupe");
        assert!(default_true());
    }
}

#[cfg(test)]
mod scale_tests {
    use super::*;

    /// The slider and the Ctrl +/- shortcuts both go through this, so an out-of-
    /// range or NaN value can never reach pixels_per_point.
    #[test]
    fn ui_scale_clamps_and_snaps_to_five_percent() {
        assert_eq!(clamp_scale(1.0), 1.0);
        assert_eq!(clamp_scale(5.0), UI_SCALE_MAX);
        assert_eq!(clamp_scale(0.1), UI_SCALE_MIN);
        assert_eq!(clamp_scale(1.234), 1.25);
        assert_eq!(clamp_scale(f32::NAN), 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::{Config, default_col_order, heal_col_order};

    /// The config holds the API key and the user's favourites: a save must be
    /// all-or-nothing (temp + fsync + rename), private, and leave no debris.
    #[test]
    fn save_is_atomic_private_and_round_trips() {
        use super::{Config, save_to};

        let dir = std::env::temp_dir().join(format!("tx-cfg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");

        let mut c = Config {
            api_key: "SECRET".to_string(),
            history: vec!["ubuntu".to_string()],
            ..Default::default()
        };
        save_to(&path, &c).expect("save");

        let text = std::fs::read_to_string(&path).unwrap();
        let back: Config = serde_json::from_str(&text).unwrap();
        assert_eq!(back.api_key, "SECRET");
        assert_eq!(back.history, ["ubuntu"]);

        // No temp file left behind.
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n != "config.json")
            .collect();
        assert!(leftovers.is_empty(), "leftover files: {leftovers:?}");

        // The key is user-private.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "config mode is {mode:o}");
        }

        // A second save replaces the file rather than appending.
        c.history.push("debian".to_string());
        save_to(&path, &c).expect("resave");
        let back: Config = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(back.history, ["ubuntu", "debian"]);

        let _ = std::fs::remove_dir_all(&dir);
    }

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
    fn heal_preserves_custom_order_and_appends_missing() {
        // A stored order naming only some columns keeps its own prefix order and
        // gets the rest appended. Previously a partial list hid the unnamed columns
        // for good: their Settings toggles flipped and nothing ever appeared.
        let healed = heal_col_order(Config {
            col_order: vec!["Seeds".into(), "Name".into()],
            ..Config::default()
        });
        assert_eq!(&healed.col_order[..2], &["Seeds", "Name"]);
        assert_eq!(healed.col_order.len(), default_col_order().len());
        for c in default_col_order() {
            assert!(healed.col_order.contains(&c), "{c} must stay reachable");
        }
    }

    #[test]
    fn heal_drops_unknown_columns_and_duplicates() {
        let healed = heal_col_order(Config {
            col_order: vec!["Name".into(), "Nope".into(), "Name".into(), "Seeds".into()],
            ..Config::default()
        });
        assert_eq!(&healed.col_order[..2], &["Name", "Seeds"]);
        assert!(!healed.col_order.iter().any(|n| n == "Nope"));
        assert_eq!(healed.col_order.len(), default_col_order().len());
    }

    #[test]
    fn default_config_has_full_col_order() {
        assert_eq!(Config::default().col_order, default_col_order());
    }

    #[test]
    fn config_round_trips_new_fields() {
        let c = Config {
            check_updates: false,
            last_tab: Some("Rss".into()),
            win_size: Some([1280.0, 720.0]),
            ..Config::default()
        };
        let j = serde_json::to_string(&c).unwrap();
        let back: Config = serde_json::from_str(&j).unwrap();
        assert!(!back.check_updates);
        assert_eq!(back.last_tab.as_deref(), Some("Rss"));
        assert_eq!(back.win_size, Some([1280.0, 720.0]));
    }

    #[test]
    fn config_defaults_apply_for_missing_fields() {
        // An old config file without the newer fields still loads cleanly.
        let old: Config =
            serde_json::from_str(r#"{"jackett_url":"http://x","api_key":"k","history":[],"favorites":[],"theme":"TokyoNight","timeout_secs":45,"dedupe":false,"page_size":50,"row_height":44.0,"font_size":14.0,"show_cat_bar":true,"col_tracker":true,"col_size":true,"col_leech":true,"col_ratio":true,"col_health":true,"col_date":true}"#).unwrap();
        assert!(old.check_updates); // serde default
        assert_eq!(old.player, "mpv"); // serde default
        assert_eq!(old.win_size, None);
        assert_eq!(old.last_tab, None);
    }

    /// A config written by an older build can name a column that no longer
    /// exists and omit the ones added since; healing must fix both without
    /// duplicating anything.
    #[test]
    fn heal_drops_unknown_columns_and_appends_missing_ones_once() {
        let c = Config {
            col_order: vec![
                "Tracker".to_string(),
                "GoneColumn".to_string(),
                "Tracker".to_string(), // duplicate
                "Seeds".to_string(),
            ],
            ..Default::default()
        };
        let healed = heal_col_order(c);
        let known = default_col_order();

        assert!(
            !healed.col_order.iter().any(|n| n == "GoneColumn"),
            "unknown column survived: {:?}",
            healed.col_order
        );
        let mut seen = std::collections::HashSet::new();
        for n in &healed.col_order {
            assert!(seen.insert(n.clone()), "duplicate column {n}");
        }
        // Every known column is present exactly once.
        assert_eq!(healed.col_order.len(), known.len());
        for k in &known {
            assert!(healed.col_order.contains(k), "missing {k}");
        }
        // The user's surviving order is kept at the front.
        assert_eq!(healed.col_order[0], "Tracker");
        assert_eq!(healed.col_order[1], "Seeds");
    }
}

#[cfg(test)]
mod col_width_heal_tests {
    use super::{Config, heal_col_widths};

    /// A width saved by a build that stretched the Name column (1143) must load
    /// as the clamp, or the first drag of that divider jumps instead of tracking.
    #[test]
    fn stale_widths_load_inside_the_render_bounds() {
        let mut c = Config::default();
        c.col_widths.insert("name".into(), 1143.4688);
        c.col_widths.insert("size".into(), f32::NAN);
        let healed = heal_col_widths(c);
        assert_eq!(healed.col_widths["name"], 900.0);
        assert_eq!(healed.col_widths["size"], 120.0);
    }
}
