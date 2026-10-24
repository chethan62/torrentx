// ─── App state and logic ───────────────────────────────────────────────────
// The App struct owns all UI + async state. This module holds the struct,
// its Default, and the pure logic methods (search, filtering, favorites,
// RSS refresh, theme). UI drawing lives in `ui.rs`; the eframe entry point
// lives in `main.rs`.

use crate::config::{Config, Favorite, save_cfg};
use crate::jackett::{
    Hlth, SearchState, SortCol, SortDir, TorrentResult, cat_col, dedupe_best_seeded, fmt_size,
    is_magnet, now_str, pub_date_key, pub_year, set_err, start_search, time_ago,
};
use crate::lock_or_recover;
use crate::rss::{FeedStatus, RssFeedConfig, RssItem, start_rss_fetch};
use crate::themes::{Pal, Theme, VisualTokens, tint};
use crate::{csv_esc, csv_safe, jackett::Tab};

use eframe::egui::{self, Color32, Stroke, Visuals};
use std::fs;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Load the embedded CJK subsets so Japanese/Chinese (Han + kana) and Korean
/// (Hangul) torrent titles render instead of □□□□.
///
/// Two single-face subsets, not one merged file: the JP face keeps Japanese
/// glyph forms for Han, and the KR face supplies Hangul, which the JP face does
/// not contain at all (Noto ships one face per region). Both are embedded
/// (`include_bytes!`) because egui's ab_glyph cannot pick a face out of a system
/// `.ttc` collection. egui falls back per character in family order, so a
/// mixed-script title draws from both. Runs once.
/// The OFL notice for the embedded fonts. Compiled in as well as shipped in the
/// repo/AppImage: the licence must accompany the fonts wherever they go, and the
/// release artifacts are a bare binary and an AppImage. The About tab shows it
/// (OFL condition 2 — the notice must be viewable by the user).
pub(crate) const FONT_LICENSE: &str = include_str!("../assets/NotoSansCJKjp-subset.LICENSE.txt");

/// The ISC notice for the bundled Lucide icons. ISC requires the notice in all
/// copies, and the release artefacts are a bare binary and an AppImage, so it is
/// compiled in as well as shipped at usr/share/licenses/torrentx/Lucide-ISC.txt.
pub(crate) const ICON_LICENSE: &str = include_str!("../assets/Lucide.ISC.LICENSE.txt");

#[cfg(test)]
mod licence_tests {
    use super::{FONT_LICENSE, ICON_LICENSE};

    /// Both notices are `include_str!`ed, so a rename or an emptied file would
    /// otherwise only surface as a blank section in About.
    #[test]
    fn bundled_notices_are_embedded_with_their_terms() {
        assert!(ICON_LICENSE.contains("ISC License"), "icon notice missing");
        assert!(
            ICON_LICENSE.contains("lucide-static 1.61.0"),
            "icon notice lost its provenance line"
        );
        assert!(
            FONT_LICENSE.contains("SIL Open Font License"),
            "font notice missing"
        );
    }
}

pub(crate) fn load_cjk_fonts(ctx: &egui::Context) {
    use std::sync::OnceLock;
    static LOADED: OnceLock<bool> = OnceLock::new();
    if LOADED.get().is_some() {
        return;
    }
    let jp: &'static [u8] = include_bytes!("../assets/NotoSansCJKjp-subset.otf");
    let kr: &'static [u8] = include_bytes!("../assets/NotoSansKR-subset.otf");
    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert("cjk".to_owned(), egui::FontData::from_static(jp).into());
    fonts
        .font_data
        .insert("hangul".to_owned(), egui::FontData::from_static(kr).into());
    // NOTE: Phosphor.ttf removed — all icons are Lucide SVG (no font glyphs).
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        if let Some(list) = fonts.families.get_mut(&family) {
            for name in ["cjk", "hangul"] {
                if !list.iter().any(|f| f == name) {
                    list.push(name.to_owned());
                }
            }
        }
    }
    ctx.set_fonts(fonts);
    let _ = LOADED.set(true);
}

/// A transient toast notification (message + color + ttl in seconds).
#[derive(Clone)]
pub(crate) struct Toast {
    pub(crate) msg: String,
    pub(crate) ttl: f32,
    pub(crate) col: Color32,
    pub(crate) anim_progress: f32, // 0.0 = hidden, 1.0 = fully visible
}

/// The filtered+sorted result set for one frame. Computed once in the main
/// loop and passed down to every drawer/handler that needs results.
pub(crate) struct ResultsView {
    /// Number of raw (unfiltered) results.
    pub(crate) raw_total: usize,
    /// Filtered + sorted rows, shared with `ViewCache` so an unchanged frame
    /// hands out the same allocation instead of rebuilding it.
    pub(crate) sorted: Arc<Vec<TorrentResult>>,
}

/// Memoises one filtered+sorted view. The key covers the filters, the sort, the
/// page size and the identity of the result set, so a cache hit is only possible
/// when the visible rows would be identical.
#[derive(Default)]
pub(crate) struct ViewCache {
    key: Option<u64>,
    /// The result set the cached rows were built from. Held (not just its
    /// address) so the allocation cannot be freed and reused — a recycled
    /// pointer would make a different result set look like a cache hit.
    input: Option<Arc<Vec<TorrentResult>>>,
    rows: Arc<Vec<TorrentResult>>,
}

impl ViewCache {
    /// Cached rows for this input + key, running `build` only on a miss.
    pub(crate) fn get(
        &mut self,
        key: u64,
        input: &Arc<Vec<TorrentResult>>,
        build: impl FnOnce() -> Vec<TorrentResult>,
    ) -> Arc<Vec<TorrentResult>> {
        let hit = self.key == Some(key)
            && self
                .input
                .as_ref()
                .is_some_and(|prev| Arc::ptr_eq(prev, input));
        if !hit {
            self.rows = Arc::new(build());
            self.key = Some(key);
            self.input = Some(Arc::clone(input));
        }
        Arc::clone(&self.rows)
    }
}

/// The main application state: config + theme, then four focused sub-structs
/// (search, network, ui, rss). Each sub-struct groups the fields that change
/// together so individual UI/logic modules can borrow only their slice.
pub(crate) struct App {
    pub(crate) cfg: Config,
    /// The window's own pixels-per-point, captured on the first frame. The UI
    /// scale multiplies THIS, never the live value — feeding `pixels_per_point()`
    /// back into `set_pixels_per_point` compounds every frame.
    pub(crate) base_ppp: f32,
    pub(crate) pal: Pal,
    pub(crate) tokens: VisualTokens,
    pub(crate) search: SearchUi,
    pub(crate) view_cache: ViewCache,
    pub(crate) net: NetState,
    pub(crate) ui: UiState,
    pub(crate) rss: RssUi,
}

/// Search + filter + sort + results + pagination state.
/// The result filters as parsed values. Built once per frame from `SearchUi`
/// and consumed by the pure predicate below, so the filtering rules can be
/// tested without an `App` (which needs a live egui context).
#[derive(Clone, Debug)]
pub(crate) struct FilterQuery {
    pub(crate) min_seed: u32,
    pub(crate) max_bytes: u64,
    pub(crate) min_year: u32,
    pub(crate) tracker: String,
    pub(crate) text: String,
    pub(crate) health: Hlth,
}

impl FilterQuery {
    pub(crate) fn from_search(s: &SearchUi) -> Self {
        let max_gb: f64 = s.f_size.parse::<f64>().unwrap_or(0.0).max(0.0);
        Self {
            min_seed: s.f_seed.parse().unwrap_or(0),
            max_bytes: (max_gb as u64).saturating_mul(1_073_741_824),
            min_year: s.f_year.parse().unwrap_or(0),
            tracker: s.f_trk.to_lowercase(),
            text: s.f_text.to_lowercase(),
            health: s.f_hlth.clone(),
        }
    }

    /// No filter active — every row passes. Test-only: the app always builds
    /// the query from the live filter fields.
    #[cfg(test)]
    pub(crate) fn none() -> Self {
        Self {
            min_seed: 0,
            max_bytes: 0,
            min_year: 0,
            tracker: String::new(),
            text: String::new(),
            health: Hlth::All,
        }
    }
}

/// Does one row survive the filters? Pure.
pub(crate) fn matches_filters(r: &TorrentResult, q: &FilterQuery) -> bool {
    let s = r.seeders.unwrap_or(0);
    if s < q.min_seed {
        return false;
    }
    if q.max_bytes > 0 && r.size.unwrap_or(0) > q.max_bytes {
        return false;
    }
    if q.min_year > 0 && r.publish_date.as_deref().map(pub_year).unwrap_or(0) < q.min_year {
        return false;
    }
    if !q.tracker.is_empty()
        && !r
            .tracker
            .as_deref()
            .unwrap_or("")
            .to_lowercase()
            .contains(&q.tracker)
    {
        return false;
    }
    if !q.text.is_empty() {
        let hay = format!(
            "{} {} {}",
            r.title.to_lowercase(),
            r.tracker.as_deref().unwrap_or("").to_lowercase(),
            r.category_desc.as_deref().unwrap_or("").to_lowercase()
        );
        if !hay.contains(&q.text) {
            return false;
        }
    }
    q.health.ok(s)
}

/// Sort in place. Descending for every column, reversed for `Asc`; dates compare
/// by parsed time (RFC 2822 strings do not sort lexically). Pure.
pub(crate) fn sort_rows(rows: &mut [TorrentResult], col: &SortCol, dir: &SortDir) {
    rows.sort_by(|a, b| {
        let c = match col {
            SortCol::Seeds => b.seeders.unwrap_or(0).cmp(&a.seeders.unwrap_or(0)),
            SortCol::Leech => b.peers.unwrap_or(0).cmp(&a.peers.unwrap_or(0)),
            SortCol::Ratio => {
                let r = |x: &TorrentResult| {
                    let s = x.seeders.unwrap_or(0) as f64;
                    let l = x.peers.unwrap_or(0) as f64;
                    if l > 0.0 { s / l } else { f64::INFINITY }
                };
                r(b).partial_cmp(&r(a)).unwrap_or(std::cmp::Ordering::Equal)
            }
            SortCol::Size => b.size.unwrap_or(0).cmp(&a.size.unwrap_or(0)),
            SortCol::Name => b.title.to_lowercase().cmp(&a.title.to_lowercase()),
            SortCol::Tracker => b
                .tracker
                .as_deref()
                .unwrap_or("")
                .to_lowercase()
                .cmp(&a.tracker.as_deref().unwrap_or("").to_lowercase()),
            SortCol::Date => pub_date_key(b.publish_date.as_deref().unwrap_or(""))
                .cmp(&pub_date_key(a.publish_date.as_deref().unwrap_or(""))),
        };
        if *dir == SortDir::Asc { c.reverse() } else { c }
    });
}

/// Page count for `n` rows. `page_size == 0` means "All" (one page), and an
/// empty result set is still one (empty) page.
pub(crate) fn page_count(n: usize, page_size: usize) -> usize {
    if page_size == 0 || n == 0 {
        return 1;
    }
    n.div_ceil(page_size)
}

/// Half-open slice bounds for `page`, clamped so a stale page index after a
/// filter change yields an empty slice instead of a panic.
pub(crate) fn page_bounds(n: usize, page_size: usize, page: usize) -> (usize, usize) {
    if page_size == 0 {
        return (0, n);
    }
    let s = page.saturating_mul(page_size);
    if s >= n {
        return (0, 0);
    }
    (s, (s + page_size).min(n))
}

#[cfg(test)]
mod view_tests {
    use super::*;

    fn row(
        title: &str,
        tracker: &str,
        seeds: u32,
        peers: u32,
        size: u64,
        date: &str,
        cat: &str,
    ) -> TorrentResult {
        TorrentResult {
            title: title.to_string(),
            tracker: Some(tracker.to_string()),
            category_desc: Some(cat.to_string()),
            size: Some(size),
            seeders: Some(seeds),
            peers: Some(peers),
            publish_date: Some(date.to_string()),
            magnet_uri: Some(format!("magnet:?xt=urn:btih:{}", "a".repeat(40))),
            link: None,
            details: None,
        }
    }

    fn gb(n: u64) -> u64 {
        n * 1_073_741_824
    }

    #[test]
    fn filters_apply_each_field_and_combine() {
        let rows = [
            row(
                "Sintel 1080p",
                "The Pirate Bay",
                900,
                10,
                gb(4),
                "2026-09-14T00:00:00Z",
                "Movies",
            ),
            row(
                "Sintel 720p",
                "1337x",
                40,
                2,
                gb(1),
                "Mon, 07 May 2018 00:00:00 +0000",
                "Movies",
            ),
            row(
                "Ubuntu ISO",
                "YTS",
                0,
                0,
                gb(9),
                "2019-01-02T00:00:00Z",
                "Software",
            ),
        ];

        let mut q = FilterQuery::none();
        assert_eq!(rows.iter().filter(|r| matches_filters(r, &q)).count(), 3);

        q.min_seed = 41; // drops the 40- and 0-seed rows
        assert_eq!(rows.iter().filter(|r| matches_filters(r, &q)).count(), 1);

        q = FilterQuery::none();
        q.max_bytes = gb(2); // only the 1 GB row survives
        let got: Vec<_> = rows.iter().filter(|r| matches_filters(r, &q)).collect();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].title, "Sintel 720p");

        q = FilterQuery::none();
        q.min_year = 2020; // RFC 2822 and ISO dates both parse
        let got: Vec<_> = rows.iter().filter(|r| matches_filters(r, &q)).collect();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].title, "Sintel 1080p");

        q = FilterQuery::none();
        q.tracker = "pirate".into(); // case-insensitive substring
        assert_eq!(rows.iter().filter(|r| matches_filters(r, &q)).count(), 1);

        q = FilterQuery::none();
        q.text = "software".into(); // matches category_desc, not just the title
        assert_eq!(rows.iter().filter(|r| matches_filters(r, &q)).count(), 1);

        q = FilterQuery::none();
        q.health = Hlth::Hot; // > 500 seeds
        assert_eq!(rows.iter().filter(|r| matches_filters(r, &q)).count(), 1);
        q.health = Hlth::Dead; // exactly 0
        assert_eq!(rows.iter().filter(|r| matches_filters(r, &q)).count(), 1);
    }

    #[test]
    fn sort_orders_by_column_and_direction() {
        let mut rows = vec![
            row(
                "beta 2020",
                "TrackerB",
                10,
                0,
                gb(1),
                "12 Apr 2018",
                "Movies",
            ),
            row(
                "Alpha 2021",
                "TrackerA",
                500,
                50,
                gb(4),
                "07 May 2018",
                "Movies",
            ),
            row(
                "gamma 2019",
                "TrackerC",
                100,
                10,
                gb(2),
                "2026-01-01T00:00:00Z",
                "Movies",
            ),
        ];

        sort_rows(&mut rows, &SortCol::Seeds, &SortDir::Desc);
        assert_eq!(rows[0].seeders, Some(500));
        sort_rows(&mut rows, &SortCol::Seeds, &SortDir::Asc);
        assert_eq!(rows[0].seeders, Some(10));

        // Dates compare by parsed time: 07 May 2018 is newer than 12 Apr 2018.
        sort_rows(&mut rows, &SortCol::Date, &SortDir::Desc);
        assert_eq!(
            rows[0].publish_date.as_deref(),
            Some("2026-01-01T00:00:00Z")
        );
        assert_eq!(rows[1].publish_date.as_deref(), Some("07 May 2018"));
        assert_eq!(rows[2].publish_date.as_deref(), Some("12 Apr 2018"));

        // Name sort is case-insensitive.
        sort_rows(&mut rows, &SortCol::Name, &SortDir::Asc);
        assert_eq!(rows[0].title, "Alpha 2021");

        // Ratio with zero leechers is infinite, so it sorts first descending.
        sort_rows(&mut rows, &SortCol::Ratio, &SortDir::Desc);
        assert_eq!(rows[0].title, "beta 2020");
    }

    /// Not an assertion — a measurement, run with `--nocapture`:
    /// `cargo test --bin torrentx bench_view -- --nocapture --ignored`
    #[test]
    #[ignore]
    fn bench_view() {
        for n in [178usize, 1000, 5000] {
            let rows: Vec<_> = (0..n)
                .map(|i| {
                    row(
                        &format!("Sintel 2010 cut {i:04} 1080p BluRay x264 DTS-HD MA 5.1"),
                        ["The Pirate Bay", "1337x", "YTS"][i % 3],
                        (i % 900) as u32,
                        (i % 30) as u32,
                        gb(1 + (i % 8) as u64),
                        "2026-09-14T00:00:00Z",
                        "Movies/HD",
                    )
                })
                .collect();
            let q = FilterQuery::none();
            let iters = 50;
            let t = std::time::Instant::now();
            let mut total = 0usize;
            for _ in 0..iters {
                let mut out: Vec<_> = rows
                    .iter()
                    .filter(|r| matches_filters(r, &q))
                    .cloned()
                    .collect();
                sort_rows(&mut out, &SortCol::Seeds, &SortDir::Desc);
                total += out.len();
            }
            let per = t.elapsed().as_secs_f64() * 1000.0 / iters as f64;
            println!(
                "{n:>5} rows: {per:.3} ms per view ({} rows kept)",
                total / iters
            );
        }
    }

    #[test]
    fn view_cache_rebuilds_only_when_the_key_changes() {
        let mut cache = ViewCache::default();
        let mut builds = 0;
        let set1 = Arc::new(vec![row(
            "a",
            "t",
            1,
            0,
            gb(1),
            "2026-01-01T00:00:00Z",
            "Movies",
        )]);
        let set2 = Arc::new(vec![row(
            "b",
            "t",
            2,
            0,
            gb(1),
            "2026-01-01T00:00:00Z",
            "Movies",
        )]);

        let a = cache.get(7, &set1, || {
            builds += 1;
            vec![row("a", "t", 1, 0, gb(1), "2026-01-01T00:00:00Z", "Movies")]
        });
        let b = cache.get(7, &set1, || {
            builds += 1;
            vec![row("x", "t", 9, 0, gb(1), "2026-01-01T00:00:00Z", "Movies")]
        });
        assert_eq!(builds, 1, "same key + same result set must not rebuild");
        assert!(Arc::ptr_eq(&a, &b), "hit returns the same allocation");
        assert_eq!(b[0].title, "a");

        // Same key, different result set: a new search whose row count happens
        // to match must still rebuild (this is the stale-view bug).
        let c = cache.get(7, &set2, || {
            builds += 1;
            vec![row("b", "t", 2, 0, gb(1), "2026-01-01T00:00:00Z", "Movies")]
        });
        assert_eq!(builds, 2, "a different result set rebuilds");
        assert_eq!(c[0].title, "b");
        assert!(!Arc::ptr_eq(&b, &c));

        let d = cache.get(8, &set2, || {
            builds += 1;
            vec![row("c", "t", 3, 0, gb(1), "2026-01-01T00:00:00Z", "Movies")]
        });
        assert_eq!(builds, 3, "a new key rebuilds");
        assert_eq!(d[0].title, "c");
    }

    #[test]
    fn paging_bounds_cover_all_and_empty_cases() {
        assert_eq!(page_count(0, 25), 1, "empty result set is one page");
        assert_eq!(page_count(25, 25), 1);
        assert_eq!(page_count(26, 25), 2);
        assert_eq!(page_count(30, 0), 1, "page_size 0 means All");
        assert_eq!(page_count(1000, 100), 10);

        assert_eq!(page_bounds(30, 0, 0), (0, 30), "All shows everything");
        assert_eq!(page_bounds(30, 25, 0), (0, 25));
        assert_eq!(page_bounds(30, 25, 1), (25, 30), "last page is partial");
        assert_eq!(page_bounds(30, 25, 2), (0, 0), "stale page index is empty");
        assert_eq!(page_bounds(0, 25, 0), (0, 0));
    }
}

pub(crate) struct SearchUi {
    pub(crate) query: String,
    pub(crate) cat: String,
    pub(crate) f_text: String,
    pub(crate) f_seed: String,
    pub(crate) f_size: String,
    pub(crate) f_year: String,
    pub(crate) f_trk: String,
    pub(crate) f_hlth: Hlth,
    pub(crate) s_col: SortCol,
    pub(crate) s_dir: SortDir,
    pub(crate) results: Arc<Mutex<Arc<Vec<TorrentResult>>>>,
    pub(crate) state: Arc<Mutex<SearchState>>,
    pub(crate) count: Arc<Mutex<usize>>,
    /// Monotonic search counter; bumped per search so stale threads can
    /// detect they were superseded and discard their response.
    pub(crate) epoch: Arc<AtomicU64>,
    /// Indexers answered / asked in the current fan-out. Drives the
    /// "3/9 indexers" part of the status line: the rows can all be on screen
    /// while one slow tracker still holds the search open.
    pub(crate) answered: Arc<AtomicUsize>,
    pub(crate) targets: Arc<AtomicUsize>,
    /// Indexers that answered with an error in the current fan-out, by id. Shown
    /// in the status line: a tracker that cannot be reached used to vanish into
    /// the answered/targets count and look like "no results".
    pub(crate) failed: Arc<std::sync::Mutex<Vec<String>>>,
    pub(crate) last_query: String,
    pub(crate) page: usize,
}

impl Default for SearchUi {
    fn default() -> Self {
        Self {
            query: String::new(),
            cat: "All".into(),
            f_text: String::new(),
            f_seed: String::new(),
            f_size: String::new(),
            f_year: String::new(),
            f_trk: String::new(),
            f_hlth: Hlth::All,
            s_col: SortCol::Seeds,
            s_dir: SortDir::Desc,
            // Arc-shared so per-frame reads are refcount bumps, not deep copies.
            results: Arc::new(Mutex::new(Arc::new(vec![]))),
            state: Arc::new(Mutex::new(SearchState::Idle)),
            count: Arc::new(Mutex::new(0)),
            epoch: Arc::new(AtomicU64::new(0)),
            answered: Arc::new(AtomicUsize::new(0)),
            targets: Arc::new(AtomicUsize::new(0)),
            failed: Arc::new(std::sync::Mutex::new(Vec::new())),
            last_query: String::new(),
            page: 0,
        }
    }
}

/// Indexer list + app-update check (background network state).
pub(crate) struct NetState {
    pub(crate) indexers: Vec<String>,
    /// Every indexer Jackett knows (configured or not) — the "add from
    /// catalogue" list in the indexer popup.
    pub(crate) catalogue: Vec<String>,
    /// Filter text for that list.
    pub(crate) cat_query: String,
    /// Is the indexer window (enable/disable + catalogue) open?
    pub(crate) idx_open: bool,
    /// Selected catalogue group (index into `jackett::CATALOGUE_GROUPS`), if any.
    pub(crate) cat_group: Option<usize>,
    pub(crate) indexer: String,
    pub(crate) indexers_loading: bool,
    pub(crate) indexers_handle: std::sync::mpsc::Sender<Option<crate::jackett::IndexerCatalogue>>,
    pub(crate) indexers_rx: std::sync::mpsc::Receiver<Option<crate::jackett::IndexerCatalogue>>,
    /// (jackett_url, api_key) of the last indexer fetch attempt — lets the UI
    /// loop re-fetch automatically when settings change instead of only once.
    pub(crate) indexers_fetched_for: Option<(String, String)>,
    /// Earliest time to retry after a failed fetch (Jackett may still be
    /// booting); None = retry allowed immediately.
    pub(crate) indexers_retry_at: Option<Instant>,
    pub(crate) update_checked: bool,
    pub(crate) update_tx: std::sync::mpsc::Sender<Option<String>>,
    pub(crate) update_rx: std::sync::mpsc::Receiver<Option<String>>,
    pub(crate) jackett_ok: Option<bool>,
}

impl Default for NetState {
    fn default() -> Self {
        let (indexers_handle, indexers_rx) =
            std::sync::mpsc::channel::<Option<crate::jackett::IndexerCatalogue>>();
        let (update_tx, update_rx) = std::sync::mpsc::channel::<Option<String>>();
        Self {
            indexers: vec![],
            catalogue: vec![],
            cat_query: String::new(),
            idx_open: false,
            cat_group: None,
            indexer: "All".into(),
            indexers_loading: false,
            indexers_handle,
            indexers_rx,
            indexers_fetched_for: None,
            indexers_retry_at: None,
            update_checked: false,
            update_tx,
            update_rx,
            jackett_ok: None,
        }
    }
}

/// UI chrome state: tabs, panels, selection, batch mode, toasts, spinner.
pub(crate) struct UiState {
    pub(crate) tab: crate::jackett::Tab,
    pub(crate) show_settings: bool,
    pub(crate) key_vis: bool,
    pub(crate) selected: Option<usize>,
    pub(crate) detail_open: bool,
    pub(crate) detail_width: f32,
    pub(crate) show_hist: bool,
    /// Screen rect of the recent-searches dropdown from the last frame. The
    /// field asks it before hiding the popup on focus loss: a click ON the popup
    /// is itself a focus loss, and hiding it in that same frame swallowed the
    /// click (nothing happened when a past search was picked).
    pub(crate) hist_rect: Option<egui::Rect>,
    /// Column widths for the results table, in the order the columns are shown.
    /// The table is built from these (`Column::exact`), so a drag changes exactly
    /// one column and the view scrolls horizontally instead of squeezing a
    /// neighbour — egui_extras' own widths live in a private TableState that
    /// cannot be read back to size a wrapping ScrollArea.
    pub(crate) col_w: Vec<f32>,
    /// Signature of the visible column set; when it changes the widths reset.
    pub(crate) col_w_sig: Option<u64>,
    /// (column index, pointer x at drag start, width at drag start)
    pub(crate) col_drag: Option<(usize, f32, f32)>,
    pub(crate) toasts: Vec<Toast>,
    pub(crate) hovered: Option<usize>,
    pub(crate) fav_search: String,
    pub(crate) sel_mode: bool,
    pub(crate) sel_set: std::collections::HashSet<usize>,
    pub(crate) show_color_picker: bool,
    pub(crate) t_start: Option<Instant>,
    pub(crate) t_done: Option<f64>,
    pub(crate) notified: bool,
    /// Edit buffers for the Settings numeric fields. They cannot be rebuilt from
    /// the (clamped) config every frame: each keystroke was clamped and fed back
    /// into the widget, so typing "10" landed on 50 and "100" on 120.
    pub(crate) timeout_buf: String,
    pub(crate) rss_secs_buf: String,
    /// Signature of everything that decides which torrent sits at which row.
    /// When it changes, row-indexed selection state is dropped (see
    /// `view_signature` / `clear_row_selection`).
    pub(crate) view_sig: Option<u64>,
    /// Animation progress for search state transitions
    pub(crate) search_state_anim: f32,
    /// Previous search state for transition animation
    pub(crate) prev_search_state: SearchState,
    /// Animation progress for table content changes (page/filter/sort)
    pub(crate) table_anim: f32,
    /// Last drawn page — bump table_anim when it changes
    pub(crate) last_table_page: usize,
    /// Animation progress for the detail panel open/close (0=hidden, 1=open)
    pub(crate) detail_anim: f32,
    /// Previous detail_open value — drive detail_anim on change
    pub(crate) prev_detail_open: bool,
    /// Cached detail row — survives `selected` being cleared on close so the
    /// fade-out animation has content to render.
    pub(crate) detail_row: Option<TorrentResult>,
    /// Last window-size config write (throttles resize persistence).
    pub(crate) win_save_at: Option<Instant>,
    /// File list for the result whose picker is open (multi-file torrents).
    pub(crate) pick: Option<PickFiles>,
    pub(crate) pick_tx: std::sync::mpsc::Sender<Result<Vec<crate::player::FileEntry>, String>>,
    pub(crate) pick_rx: std::sync::mpsc::Receiver<Result<Vec<crate::player::FileEntry>, String>>,
}

/// File list for one result, so a season pack can play the episode you picked.
#[derive(Clone, Default)]
pub(crate) struct PickFiles {
    /// Result title the list belongs to.
    pub(crate) key: String,
    pub(crate) files: Vec<crate::player::FileEntry>,
    /// Chosen file; `None` keeps webtorrent's default (the largest file).
    pub(crate) sel: Option<usize>,
    pub(crate) loading: bool,
    pub(crate) err: Option<String>,
    /// Case-insensitive filter for long season packs.
    pub(crate) filter: String,
    /// Whether the chooser window is open (the list outlives it).
    pub(crate) window: bool,
}

impl Default for UiState {
    fn default() -> Self {
        let (pick_tx, pick_rx) = std::sync::mpsc::channel();
        Self {
            tab: crate::jackett::Tab::Search,
            show_settings: false,
            key_vis: false,
            selected: None,
            detail_open: false,
            detail_width: 295.0,
            show_hist: false,
            hist_rect: None,
            col_w: Vec::new(),
            col_w_sig: None,
            col_drag: None,
            toasts: vec![],
            hovered: None,
            fav_search: String::new(),
            sel_mode: false,
            sel_set: std::collections::HashSet::new(),
            show_color_picker: false,
            t_start: None,
            t_done: None,
            notified: false,
            search_state_anim: 1.0, // Start fully visible
            prev_search_state: SearchState::Idle,
            table_anim: 1.0, // Start fully visible
            last_table_page: 0,
            detail_anim: 0.0, // Detail panel starts hidden
            prev_detail_open: false,
            detail_row: None,
            view_sig: None,
            timeout_buf: String::new(),
            rss_secs_buf: String::new(),
            // Grace period: don't persist size until 3s after launch, so the
            // pre-restore default never clobbers the saved window size.
            win_save_at: Some(Instant::now()),
            pick: None,
            pick_tx,
            pick_rx,
        }
    }
}

/// RSS feed list + detail + add/edit form state.
pub(crate) struct RssUi {
    pub(crate) rss_feeds: Vec<crate::rss::RssFeedState>,
    pub(crate) rss_last_refresh: Vec<Instant>,
    pub(crate) rss_tx: std::sync::mpsc::Sender<(usize, Result<Vec<RssItem>, String>)>,
    pub(crate) rss_rx: std::sync::mpsc::Receiver<(usize, Result<Vec<RssItem>, String>)>,
    pub(crate) rss_selected: usize,
    pub(crate) rss_detail: Option<usize>,
    pub(crate) rss_filter: String,
    pub(crate) rss_add_mode: bool,
    pub(crate) rss_edit_idx: Option<usize>,
    pub(crate) rss_new_cfg: RssFeedConfig,
}

impl RssUi {
    pub(crate) fn new(feeds: &[RssFeedConfig]) -> Self {
        let (rss_tx, rss_rx) = std::sync::mpsc::channel();
        let n = feeds.len();
        Self {
            rss_feeds: feeds
                .iter()
                .map(|c| crate::rss::RssFeedState::new(c.clone()))
                .collect(),
            rss_last_refresh: vec![Instant::now(); n],
            rss_tx,
            rss_rx,
            rss_selected: 0,
            rss_detail: None,
            rss_filter: String::new(),
            rss_add_mode: false,
            rss_edit_idx: None,
            rss_new_cfg: crate::rss::RssFeedConfig::new_default(),
        }
    }
}

impl Default for App {
    fn default() -> Self {
        // A crash or a kill leaves last session's streamed pieces on disk; they
        // are never reused (webtorrent re-verifies them, which then stalls Play).
        crate::player::prune_stream_cache();
        let cfg = crate::config::load_cfg();
        let pal = Pal::from(&cfg.theme, cfg.accent);
        let tokens = VisualTokens::default();
        let rss = RssUi::new(&cfg.rss_feeds);
        // Restore the last active tab (persisted across sessions).
        let tab = cfg
            .last_tab
            .as_deref()
            .and_then(Tab::from_key)
            .unwrap_or(Tab::Search);
        let ui = UiState {
            tab,
            ..UiState::default()
        };
        let mut app = Self {
            cfg,
            pal,
            tokens,
            base_ppp: 0.0,
            search: SearchUi::default(),
            view_cache: ViewCache::default(),
            net: NetState::default(),
            ui,
            rss,
        };
        // Report an unreadable config in the UI: stderr is invisible in a GUI
        // launch and the user needs to know their settings were reset (and that a
        // backup was kept).
        if let Some(w) = crate::config::CONFIG_LOAD_WARNING.get() {
            let (msg, col) = (w.clone(), app.pal.yellow);
            app.toast(&msg, col);
        }
        app
    }
}

impl App {
    /// Re-fetch Jackett's configured indexers. Jackett has no API for adding one
    /// (`/api/v2.0/indexers/*` is UI-session gated — a DELETE or POST there
    /// redirects to /UI/Login even with a valid API key), so the app links to
    /// Jackett's own dashboard and then reloads the list from here.
    pub(crate) fn refresh_indexers(&mut self) {
        if self.net.indexers_loading || self.cfg.api_key.is_empty() {
            return;
        }
        let (url, key, handle) = (
            self.cfg.jackett_url.clone(),
            self.cfg.api_key.clone(),
            self.net.indexers_handle.clone(),
        );
        self.net.indexers_fetched_for = Some((url.clone(), key.clone()));
        self.net.indexers_loading = true;
        std::thread::spawn(move || {
            let list = crate::jackett::fetch_indexers(&url, &key);
            let _ = handle.send(list);
            crate::wake_ui();
        });
    }

    /// Switch tabs, persisting the choice so the next launch reopens here.
    pub(crate) fn set_tab(&mut self, tab: Tab) {
        self.cfg.last_tab = Some(tab.key().to_string());
        self.ui.tab = tab;
        let _ = save_cfg(&self.cfg);
    }

    /// Empty the search box and put the results back to the welcome screen.
    ///
    /// Clearing only the text left the table and the status line still describing
    /// the previous query ("12 results for \"iso\"") under an empty field, and an
    /// in-flight fan-out could land its rows afterwards. The epoch bump is what
    /// stops that: a worker whose epoch is stale drops its batch.
    pub(crate) fn clear_search(&mut self) {
        self.search.query.clear();
        self.ui.show_hist = false;
        self.ui.hist_rect = None;
        self.ui.selected = None;
        self.ui.detail_open = false;
        self.ui.detail_row = None;
        self.ui.sel_set.clear();
        self.ui.sel_mode = false;
        self.ui.hovered = None;
        self.search.last_query.clear();
        self.search.f_text.clear();
        self.search.page = 0;
        self.ui.t_start = None;
        self.ui.t_done = None;
        self.search.epoch.fetch_add(1, Ordering::Relaxed);
        *lock_or_recover(&self.search.results) = Arc::new(vec![]);
        *lock_or_recover(&self.search.count) = 0;
        *lock_or_recover(&self.search.state) = SearchState::Idle;
    }

    pub(crate) fn do_search(&mut self) {
        let q = self.search.query.trim().to_string();
        if q.is_empty() {
            return;
        }
        if self.cfg.api_key.trim().is_empty() {
            set_err(
                &self.search.state,
                "No API key — open Settings and paste your Jackett API key.".into(),
            );
            self.set_settings_open(true);
            return;
        }
        self.cfg.history.retain(|h| h != &q);
        self.cfg.history.insert(0, q.clone());
        self.cfg.history.truncate(20);
        let _ = save_cfg(&self.cfg);
        self.ui.selected = None;
        self.ui.detail_open = false;
        self.ui.detail_row = None; // drop cached detail from previous search
        self.ui.show_hist = false;
        self.search.page = 0;
        self.ui.sel_set.clear();
        self.ui.sel_mode = false;
        self.search.last_query = q.clone();
        self.search.f_text.clear();
        self.ui.hovered = None;
        self.ui.t_start = Some(Instant::now());
        self.ui.t_done = None;
        self.ui.notified = false;
        *lock_or_recover(&self.search.results) = Arc::new(vec![]);
        *lock_or_recover(&self.search.count) = 0;
        let generation = self.search.epoch.fetch_add(1, Ordering::Relaxed) + 1;
        self.search.answered.store(0, Ordering::Relaxed);
        // Fan out per indexer so results land as each tracker answers.
        let targets = crate::jackett::search_targets(
            &self.net.indexer,
            &self.net.indexers,
            &self.cfg.disabled_indexers,
        );
        self.search.targets.store(targets.len(), Ordering::Relaxed);
        lock_or_recover(&self.search.failed).clear();
        start_search(
            self.cfg.jackett_url.clone(),
            self.cfg.api_key.clone(),
            q,
            self.search.cat.clone(),
            targets,
            self.cfg.timeout_secs,
            Arc::clone(&self.search.results),
            Arc::clone(&self.search.state),
            Arc::clone(&self.search.count),
            Arc::clone(&self.search.epoch),
            Arc::clone(&self.search.answered),
            Arc::clone(&self.search.failed),
            self.cfg.dedupe,
            generation,
        );
    }

    /// Copy all magnet links from the batch-selected rows (one per line).
    /// Takes the frame's shared `ResultsView` instead of recomputing it.
    pub(crate) fn copy_selected_magnets(&mut self, ui: &egui::Ui, sorted: &[TorrentResult]) {
        let magnets: Vec<&str> = self
            .ui
            .sel_set
            .iter()
            .filter_map(|&i| sorted.get(i))
            .filter_map(|r| r.magnet_uri.as_deref())
            .filter(|m| is_magnet(m))
            .collect();
        if magnets.is_empty() {
            self.toast("No valid magnets selected", self.pal.yellow);
            return;
        }
        let text = magnets.join("\n");
        ui.ctx().copy_text(text);
        self.toast(
            &format!(
                "Copied {} magnet{}",
                magnets.len(),
                if magnets.len() == 1 { "" } else { "s" }
            ),
            self.pal.green,
        );
        self.ui.sel_mode = false;
        self.ui.sel_set.clear();
    }

    /// Whether a torrent with this title is already saved. The results table
    /// shows a filled star and a "Remove" tooltip when it is.
    pub(crate) fn is_fav(&self, title: &str) -> bool {
        self.cfg.favorites.iter().any(|f| f.title == title)
    }

    pub(crate) fn add_fav(&mut self, r: &TorrentResult) {
        if self.is_fav(&r.title) {
            self.toast("Already in Favorites", self.pal.yellow);
            return;
        }
        self.cfg.favorites.push(Favorite {
            title: r.title.clone(),
            magnet: r.magnet_uri.clone(),
            link: r.link.clone(),
            tracker: r.tracker.clone(),
            size: r.size,
            seeders: r.seeders,
            saved_at: now_str(),
        });
        let _ = save_cfg(&self.cfg);
        self.toast("Saved to Favorites", self.pal.yellow);
    }

    pub(crate) fn toast(&mut self, msg: &str, col: Color32) {
        self.ui.toasts.retain(|t| t.msg != msg);
        self.ui.toasts.push(Toast {
            msg: msg.into(),
            ttl: 2.5,
            col,
            anim_progress: 0.0,
        });
    }

    /// Fire a desktop notification when a search completes while the window
    /// is hidden (minimized to tray). No-op on failure / missing daemon.
    pub(crate) fn notify_search_done(&mut self) {
        let n = *lock_or_recover(&self.search.count);
        let q = self.search.last_query.clone();
        if let Ok(h) = notify_rust::Notification::new()
            .summary("TorrentX — search complete")
            .body(&format!("{n} results for \u{201c}{q}\u{201d}"))
            .appname("TorrentX")
            .timeout(notify_rust::Timeout::Milliseconds(5000))
            .show()
        {
            let _ = h;
        }
    }

    pub(crate) fn set_theme(&mut self, t: Theme) {
        self.cfg.theme = t;
        self.pal = Pal::from(&self.cfg.theme, self.cfg.accent);
        let _ = save_cfg(&self.cfg);
    }

    // ── RSS helpers ───────────────────────────────────────────────────────

    pub(crate) fn sync_rss_configs(&mut self) {
        self.cfg.rss_feeds = self
            .rss
            .rss_feeds
            .iter()
            .map(|f| f.config.clone())
            .collect();
        let _ = save_cfg(&self.cfg);
    }

    pub(crate) fn refresh_feed(&mut self, idx: usize) {
        if idx >= self.rss.rss_feeds.len() {
            return;
        }
        if self.rss.rss_feeds[idx].status == FeedStatus::Loading {
            return;
        } // already in flight
        self.rss.rss_feeds[idx].status = FeedStatus::Loading;
        self.rss.rss_feeds[idx].error = None;
        if self.rss.rss_last_refresh.len() == self.rss.rss_feeds.len() {
            self.rss.rss_last_refresh[idx] = Instant::now();
        }
        let tx = self.rss.rss_tx.clone();
        let base = self.cfg.jackett_url.clone();
        let key = self.cfg.api_key.clone();
        let cfg = self.rss.rss_feeds[idx].config.clone();
        let to = self.cfg.timeout_secs;
        start_rss_fetch(base, key, cfg, to, idx, tx);
    }

    pub(crate) fn refresh_all_feeds(&mut self) {
        for i in 0..self.rss.rss_feeds.len() {
            if self.rss.rss_feeds[i].config.enabled {
                self.refresh_feed(i);
            }
        }
    }

    pub(crate) fn poll_rss(&mut self) {
        // Drain completed background fetches (non-blocking; never touch the network on the UI thread)
        while let Ok((idx, result)) = self.rss.rss_rx.try_recv() {
            if idx >= self.rss.rss_feeds.len() {
                continue;
            }
            // Dedupe per release (guid, else the content-marker key), and let
            // the state own the failure counter that drives the backoff.
            self.rss.rss_feeds[idx].apply_result(result);
        }
    }

    /// Load the file list for one result (background; polled by `poll_pick`).
    /// Re-clicking the same row reuses nothing: a torrent's list can change
    /// between attempts, and this is a user-initiated action.
    pub(crate) fn pick_files(&mut self, title: &str, magnet: &str, link: Option<&str>) {
        self.ui.pick = Some(PickFiles {
            key: title.to_string(),
            loading: true,
            // Open the chooser as soon as the list arrives.
            window: true,
            ..Default::default()
        });
        crate::player::list_files(
            magnet.to_string(),
            link.map(str::to_string),
            self.ui.pick_tx.clone(),
        );
    }

    /// Drain a finished file listing into the picker.
    pub(crate) fn poll_pick(&mut self) {
        while let Ok(res) = self.ui.pick_rx.try_recv() {
            let Some(p) = self.ui.pick.as_mut() else {
                continue;
            };
            p.loading = false;
            match res {
                Ok(files) => {
                    p.files = files;
                    p.err = None;
                }
                Err(e) => p.err = Some(e),
            }
        }
    }

    /// The file index chosen for `title`, if its list is loaded. `None` keeps
    /// webtorrent's default (largest file).
    pub(crate) fn picked_file(&self, title: &str) -> Option<usize> {
        let p = self.ui.pick.as_ref().filter(|p| p.key == title)?;
        p.sel.and_then(|i| p.files.get(i)).map(|f| f.idx)
    }

    /// Auto-refresh enabled feeds whose `auto_refresh` flag is set.
    /// Re-checks every `cfg.rss_refresh_secs`; skips feeds already loading.
    ///
    /// Returns how long until this needs to run again, or `None` when there is
    /// nothing to wait for. The caller must turn that into a wake
    /// (`ctx.request_repaint_after`): the UI loop is otherwise parked while the
    /// app sits idle, so a timer only ever checked from the paint loop would
    /// never fire — feeds refreshed only while the user was interacting.
    pub(crate) fn auto_refresh_feeds(&mut self) -> Option<Duration> {
        // Keep timestamps in sync with the feed list (add/remove).
        while self.rss.rss_last_refresh.len() < self.rss.rss_feeds.len() {
            self.rss.rss_last_refresh.push(Instant::now());
        }
        self.rss.rss_last_refresh.truncate(self.rss.rss_feeds.len());
        let interval = self.cfg.rss_refresh_secs;
        if interval == 0 {
            return None;
        }
        let interval = Duration::from_secs(interval);
        // Shortest wait wins, so one wake serves every feed.
        let mut wake_in: Option<Duration> = None;
        let sooner = |d: Duration, cur: Option<Duration>| Some(cur.map_or(d, |c| c.min(d)));
        for i in 0..self.rss.rss_feeds.len() {
            let cfg = self.rss.rss_feeds[i].config.clone();
            if !cfg.enabled || !cfg.auto_refresh {
                continue;
            }
            if self.rss.rss_feeds[i].status == FeedStatus::Loading {
                // Fetch in flight: come back soon to drain the result.
                wake_in = sooner(Duration::from_millis(500), wake_in);
                continue;
            }
            // A feed that keeps failing is retried with a growing delay.
            let due = crate::rss::refresh_delay(interval, self.rss.rss_feeds[i].failures);
            let elapsed = self.rss.rss_last_refresh[i].elapsed();
            if elapsed >= due {
                self.refresh_feed(i);
                // The timestamp was just reset by refresh_feed.
                wake_in = sooner(due, wake_in);
            } else {
                wake_in = sooner(due - elapsed, wake_in);
            }
        }
        wake_in
    }

    pub(crate) fn add_fav_from_rss(&mut self, item: &RssItem) {
        if self.is_fav(&item.title) {
            self.toast("Already in Favorites", self.pal.yellow);
            return;
        }
        self.cfg.favorites.push(Favorite {
            title: item.title.clone(),
            magnet: item.magnet.clone(),
            link: item.link.clone(),
            tracker: item.tracker.clone(),
            size: item.size,
            seeders: item.seeders,
            saved_at: now_str(),
        });
        let _ = save_cfg(&self.cfg);
        self.toast("Saved to Favorites", self.pal.yellow);
    }

    pub(crate) fn cur_state(&self) -> SearchState {
        lock_or_recover(&self.search.state).clone()
    }
    /// Cheap snapshot of the current results (Arc refcount bump — no deep
    /// copy). The vec inside is never mutated after publication.
    pub(crate) fn all_results(&self) -> Arc<Vec<TorrentResult>> {
        lock_or_recover(&self.search.results).clone()
    }
    /// Collapse cross-tracker duplicates **into the stored results** once a
    /// search finishes. Doing it per-frame in `compute_view` left every counter
    /// (header badge, status line) reporting the pre-dedupe number while the
    /// table showed fewer rows — the user saw "8 results" over a 7-row table.
    pub(crate) fn finalize_results(&self) {
        if !self.cfg.dedupe {
            return;
        }
        let mut g = lock_or_recover(&self.search.results);
        let deduped = dedupe_best_seeded((**g).clone());
        if deduped.len() == g.len() {
            return;
        }
        *g = std::sync::Arc::new(deduped);
        let n = g.len();
        drop(g);
        *lock_or_recover(&self.search.count) = n;
    }

    pub(crate) fn total_count(&self) -> usize {
        *lock_or_recover(&self.search.count)
    }

    /// Filter+sort the current results ONCE per frame. Every consumer in the
    /// frame (search tab, detail panel, batch handlers) shares this view —
    /// previously each recomputed its own clone+sort, up to 4× per frame.
    pub(crate) fn compute_view(&mut self) -> ResultsView {
        let raw = self.all_results();
        let raw_total = raw.len();
        // Filters + sort + page size + result count, mixed with the identity of
        // the result set itself: same count but different rows must not hit.
        let key = self.view_signature();
        let q = FilterQuery::from_search(&self.search);
        let (col, dir) = (self.search.s_col.clone(), self.search.s_dir.clone());
        let sorted = self.view_cache.get(key, &raw, || {
            let mut out: Vec<_> = raw
                .iter()
                .filter(|r| matches_filters(r, &q))
                .cloned()
                .collect();
            sort_rows(&mut out, &col, &dir);
            out
        });
        ResultsView { raw_total, sorted }
    }

    pub(crate) fn max_pages(&self, n: usize) -> usize {
        page_count(n, self.cfg.page_size)
    }

    pub(crate) fn page_slice<'a>(&self, v: &'a [TorrentResult]) -> &'a [TorrentResult] {
        let (s, e) = page_bounds(v.len(), self.cfg.page_size, self.search.page);
        &v[s..e]
    }

    pub(crate) fn cat_chips(results: &[TorrentResult]) -> Vec<(String, usize, Color32)> {
        let mut map: std::collections::BTreeMap<String, usize> = Default::default();
        for r in results {
            let c = r
                .category_desc
                .as_deref()
                .and_then(|c| c.split('/').next())
                .unwrap_or("Other")
                .trim()
                .to_string();
            *map.entry(c).or_insert(0) += 1;
        }
        let mut v: Vec<_> = map
            .into_iter()
            .map(|(k, n)| {
                let col = cat_col(&k);
                (k, n, col)
            })
            .collect();
        v.sort_by_key(|b| std::cmp::Reverse(b.1));
        v.truncate(7);
        v
    }

    /// Cheap signature of everything that decides which torrent sits at which row
    /// (filters, sort, page, page size, result count). Compared once per frame so
    /// row-indexed selection can be dropped when the view changes underneath it.
    pub(crate) fn view_signature(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.search.cat.hash(&mut h);
        self.search.f_text.hash(&mut h);
        self.search.f_seed.hash(&mut h);
        self.search.f_size.hash(&mut h);
        self.search.f_year.hash(&mut h);
        self.search.f_trk.hash(&mut h);
        (self.search.f_hlth.clone() as u8).hash(&mut h);
        (self.search.s_col.clone() as u8).hash(&mut h);
        (self.search.s_dir.clone() as u8).hash(&mut h);
        self.search.page.hash(&mut h);
        self.cfg.page_size.hash(&mut h);
        lock_or_recover(&self.search.count).hash(&mut h);
        h.finish()
    }

    /// Drop row-keyed selection state. `selected`, `sel_set` and the detail panel
    /// all address rows by index, so after any reorder/reslice they would point at
    /// a different torrent — the highlight moves, F/Enter/M act on the wrong item,
    /// and a batch copy would export torrents the user never ticked.
    pub(crate) fn clear_row_selection(&mut self) {
        self.ui.selected = None;
        self.ui.detail_open = false;
        self.ui.detail_row = None;
        self.ui.sel_set.clear();
        self.ui.hovered = None;
    }

    /// Open or close the Settings panel, seeding the numeric edit buffers from the
    /// config. They cannot be derived per frame (see `timeout_buf`).
    pub(crate) fn set_settings_open(&mut self, open: bool) {
        self.ui.show_settings = open;
        if open {
            self.ui.timeout_buf = self.cfg.timeout_secs.to_string();
            self.ui.rss_secs_buf = self.cfg.rss_refresh_secs.to_string();
        }
    }

    pub(crate) fn export_csv(&mut self, rows: &[TorrentResult]) {
        let path = dirs_next::download_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(format!(
                "torrentx_{}.csv",
                self.search.last_query.replace(' ', "_").replace('/', "-")
            ));
        let mut out = "Title,Tracker,Category,Size,Seeders,Leechers,Date\n".to_string();
        for r in rows {
            out.push_str(&format!(
                "{},{},{},{},{},{},{}\n",
                csv_safe(&r.title),
                csv_safe(r.tracker.as_deref().unwrap_or("")),
                csv_safe(r.category_desc.as_deref().unwrap_or("")),
                csv_esc(&r.size.map(fmt_size).unwrap_or_default()),
                r.seeders.unwrap_or(0),
                r.peers.unwrap_or(0),
                csv_safe(&r.publish_date.as_deref().map(time_ago).unwrap_or_default()),
            ));
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        match fs::write(&path, out) {
            Ok(()) => {
                let _ = open::that(&path);
                self.toast(&format!("Exported {name}"), self.pal.green);
            }
            Err(e) => self.toast(&format!("Export failed: {e}"), self.pal.red),
        }
    }

    pub(crate) fn apply_theme(&self, ctx: &egui::Context) {
        load_cjk_fonts(ctx); // once (OnceLock inside)
        let p = &self.pal;
        let mut vis = if p.light {
            Visuals::light()
        } else {
            Visuals::dark()
        };
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
        vis.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.dim);
        vis.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, p.sub);
        vis.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border);
        vis.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, p.border);
        // Focus/hover states: a focused text field draws its frame with
        // `selection.stroke`, so this is the one accent focus ring everywhere;
        // hover stays a plain border so it does not compete with focus.
        vis.selection.stroke = Stroke::new(1.0_f32, p.accent);
        vis.widgets.active.bg_stroke = Stroke::new(1.0_f32, p.accent);
        vis.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.border);
        let rn = egui::CornerRadius::same(crate::ui::R_MD as u8);
        vis.widgets.noninteractive.corner_radius = rn;
        vis.widgets.inactive.corner_radius = rn;
        vis.widgets.hovered.corner_radius = rn;
        vis.widgets.active.corner_radius = rn;
        ctx.set_visuals(vis);
        // Labels must NOT be text-selectable: with the egui default
        // `selectable_labels = true`, every Label silently senses click+drag
        // over its text galley, which sits ON TOP of our row `ui.interact`
        // click layers and swallows clicks on text. Disable it so row clicks
        // land everywhere (we never need in-table text selection).
        ctx.all_styles_mut(|s| s.interaction.selectable_labels = false);
    }
}

#[cfg(test)]
mod font_tests {
    const JP: &[u8] = include_bytes!("../assets/NotoSansCJKjp-subset.otf");
    const KR: &[u8] = include_bytes!("../assets/NotoSansKR-subset.otf");

    fn font(bytes: &[u8], what: &str) -> ab_glyph::FontVec {
        ab_glyph::FontVec::try_from_vec(bytes.to_vec())
            .unwrap_or_else(|e| panic!("{what} must parse as a single-face font: {e}"))
    }

    /// The README promises CJK titles render; ab_glyph silently draws .notdef
    /// (tofu) for a missing glyph. So assert coverage per script: a regenerated
    /// subset that drops kana, Han or Hangul fails here instead of shipping.
    #[test]
    fn bundled_fonts_cover_the_promised_scripts() {
        use ab_glyph::Font;
        let jp = font(JP, "JP subset");
        let kr = font(KR, "KR subset");
        for c in "日本語のアニメ".chars() {
            assert_ne!(jp.glyph_id(c).0, 0, "JP subset lacks {c:?}");
        }
        for c in "中文电影".chars() {
            assert_ne!(jp.glyph_id(c).0, 0, "JP subset lacks Han {c:?}");
        }
        for c in "한글드라마".chars() {
            assert_ne!(kr.glyph_id(c).0, 0, "KR subset lacks {c:?}");
        }
        // Fullwidth punctuation and CJK punctuation appear in real titles.
        for c in "：！？（）".chars() {
            assert_ne!(jp.glyph_id(c).0, 0, "JP subset lacks punctuation {c:?}");
        }
        // Disjoint faces: Hangul must come from KR, Han from JP. If they ever
        // overlap, a broken glyph in one face could silently mask the other.
        assert_eq!(jp.glyph_id('한').0, 0, "Hangul must come from the KR face");
        assert_eq!(kr.glyph_id('日').0, 0, "Han must come from the JP face");
    }
}

#[cfg(test)]
mod clear_tests {
    use super::*;

    /// Clearing must undo the whole search, not just the text: the table and the
    /// status line used to keep describing the old query, and a fan-out still in
    /// flight could publish its rows afterwards.
    #[test]
    fn clearing_drops_the_results_and_invalidates_in_flight_workers() {
        let mut app = App::default();
        app.search.query = "iso".into();
        app.search.last_query = "iso".into();
        *lock_or_recover(&app.search.count) = 7;
        *lock_or_recover(&app.search.state) = SearchState::Searching;
        let epoch = app.search.epoch.load(Ordering::Relaxed);

        app.clear_search();

        assert!(app.search.query.is_empty());
        assert!(app.search.last_query.is_empty());
        assert_eq!(*lock_or_recover(&app.search.count), 0);
        assert!(lock_or_recover(&app.search.results).is_empty());
        assert!(matches!(
            *lock_or_recover(&app.search.state),
            SearchState::Idle
        ));
        assert!(
            app.search.epoch.load(Ordering::Relaxed) > epoch,
            "a worker from before the clear must be stale now"
        );
    }
}
