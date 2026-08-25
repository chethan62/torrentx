// ─── App state and logic ───────────────────────────────────────────────────
// The App struct owns all UI + async state. This module holds the struct,
// its Default, and the pure logic methods (search, filtering, favorites,
// RSS refresh, theme). UI drawing lives in `ui.rs`; the eframe entry point
// lives in `main.rs`.

use crate::config::{save_cfg, Config, Favorite};
use crate::jackett::{
    cat_col, fmt_size, is_magnet, normalize, now_str, pub_year, set_err, start_search, time_ago,
    Hlth, SearchState, SortCol, SortDir, TorrentResult,
};
use crate::rss::{start_rss_fetch, FeedStatus, RssFeedConfig, RssItem};
use crate::themes::{tint, Pal, Theme, VisualTokens};
use crate::{csv_esc, csv_safe};

use eframe::egui::{self, Color32, Stroke, Visuals};
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Load the embedded Noto Sans CJK JP subset so Japanese/Korean/Chinese
/// torrent titles render instead of showing □□□□. The font is embedded in
/// the binary (include_bytes!) — egui can't load system .ttc collections
/// (ab_glyph can't select a face index), so we embed a single-face .otf
/// subset (CJK ideographs + kana + punctuation). Runs once.
pub(crate) fn load_cjk_fonts(ctx: &egui::Context) {
    use std::sync::OnceLock;
    static LOADED: OnceLock<bool> = OnceLock::new();
    if LOADED.get().is_some() {
        return;
    }
    let data: &'static [u8] = include_bytes!("../assets/NotoSansCJKjp-subset.otf");
    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert("cjk".to_owned(), egui::FontData::from_static(data).into());
    // NOTE: Phosphor.ttf removed — all icons are Lucide SVG (no font glyphs).
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        if let Some(list) = fonts.families.get_mut(&family) {
            if !list.contains(&"cjk".to_owned()) {
                list.push("cjk".to_owned());
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

/// The main application state: config + theme, then four focused sub-structs
/// (search, network, ui, rss). Each sub-struct groups the fields that change
/// together so individual UI/logic modules can borrow only their slice.
pub(crate) struct App {
    pub(crate) cfg: Config,
    pub(crate) pal: Pal,
    pub(crate) tokens: VisualTokens,
    pub(crate) search: SearchUi,
    pub(crate) net: NetState,
    pub(crate) ui: UiState,
    pub(crate) rss: RssUi,
}

/// Search + filter + sort + results + pagination state.
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
    pub(crate) results: Arc<Mutex<Vec<TorrentResult>>>,
    pub(crate) state: Arc<Mutex<SearchState>>,
    pub(crate) count: Arc<Mutex<usize>>,
    /// Monotonic search counter; bumped per search so stale threads can
    /// detect they were superseded and discard their response.
    pub(crate) epoch: Arc<AtomicU64>,
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
            results: Arc::new(Mutex::new(vec![])),
            state: Arc::new(Mutex::new(SearchState::Idle)),
            count: Arc::new(Mutex::new(0)),
            epoch: Arc::new(AtomicU64::new(0)),
            last_query: String::new(),
            page: 0,
        }
    }
}

/// Indexer list + app-update check (background network state).
pub(crate) struct NetState {
    pub(crate) indexers: Vec<String>,
    pub(crate) indexer: String,
    pub(crate) indexers_loading: bool,
    pub(crate) indexers_handle: std::sync::mpsc::Sender<Option<Vec<String>>>,
    pub(crate) indexers_rx: std::sync::mpsc::Receiver<Option<Vec<String>>>,
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
        let (indexers_handle, indexers_rx) = std::sync::mpsc::channel::<Option<Vec<String>>>();
        let (update_tx, update_rx) = std::sync::mpsc::channel::<Option<String>>();
        Self {
            indexers: vec![],
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
    pub(crate) toasts: Vec<Toast>,
    pub(crate) hovered: Option<usize>,
    pub(crate) fav_search: String,
    pub(crate) sel_mode: bool,
    pub(crate) sel_set: std::collections::HashSet<usize>,
    pub(crate) show_color_picker: bool,
    pub(crate) t_start: Option<Instant>,
    pub(crate) t_done: Option<f64>,
    pub(crate) notified: bool,
    /// Animation progress for row hover (0.0 = not hovering, 1.0 = fully hovered)
    pub(crate) row_hover_anim: f32,
    /// Previously hovered row index for animation
    pub(crate) prev_hovered: Option<usize>,
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
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            tab: crate::jackett::Tab::Search,
            show_settings: false,
            key_vis: false,
            selected: None,
            detail_open: false,
            detail_width: 295.0,
            show_hist: false,
            toasts: vec![],
            hovered: None,
            fav_search: String::new(),
            sel_mode: false,
            sel_set: std::collections::HashSet::new(),
            show_color_picker: false,
            t_start: None,
            t_done: None,
            notified: false,
            row_hover_anim: 0.0,
            prev_hovered: None,
            search_state_anim: 1.0, // Start fully visible
            prev_search_state: SearchState::Idle,
            table_anim: 1.0, // Start fully visible
            last_table_page: 0,
            detail_anim: 0.0, // Detail panel starts hidden
            prev_detail_open: false,
            detail_row: None,
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
        let cfg = crate::config::load_cfg();
        let pal = Pal::from(&cfg.theme, cfg.accent);
        let tokens = VisualTokens::default();
        let rss = RssUi::new(&cfg.rss_feeds);
        Self {
            cfg,
            pal,
            tokens,
            search: SearchUi::default(),
            net: NetState::default(),
            ui: UiState::default(),
            rss,
        }
    }
}
impl App {
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
            self.ui.show_settings = true;
            return;
        }
        self.cfg.history.retain(|h| h != &q);
        self.cfg.history.insert(0, q.clone());
        self.cfg.history.truncate(20);
        save_cfg(&self.cfg);
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
        if let Ok(mut r) = self.search.results.lock() {
            r.clear();
        }
        if let Ok(mut c) = self.search.count.lock() {
            *c = 0;
        }
        let gen = self.search.epoch.fetch_add(1, Ordering::Relaxed) + 1;
        start_search(
            self.cfg.jackett_url.clone(),
            self.cfg.api_key.clone(),
            q,
            self.search.cat.clone(),
            self.net.indexer.clone(),
            self.cfg.timeout_secs,
            Arc::clone(&self.search.results),
            Arc::clone(&self.search.state),
            Arc::clone(&self.search.count),
            Arc::clone(&self.search.epoch),
            gen,
        );
    }

    /// Copy all magnet links from the batch-selected rows (one per line).
    pub(crate) fn copy_selected_magnets(&mut self, ui: &egui::Ui) {
        let raw = self.all_results();
        let sorted = self.filtered(&raw);
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

    pub(crate) fn add_fav(&mut self, r: &TorrentResult) {
        if self.cfg.favorites.iter().any(|f| f.title == r.title) {
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
        save_cfg(&self.cfg);
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
        let n = self.search.count.lock().map(|c| *c).unwrap_or(0);
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
        save_cfg(&self.cfg);
    }

    // ── RSS helpers ───────────────────────────────────────────────────────

    pub(crate) fn sync_rss_configs(&mut self) {
        self.cfg.rss_feeds = self
            .rss
            .rss_feeds
            .iter()
            .map(|f| f.config.clone())
            .collect();
        save_cfg(&self.cfg);
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
            match result {
                Ok(items) => {
                    // Dedupe by normalized title (same logic as main search):
                    // sort by seeders desc so the best copy wins, then keep first.
                    let mut items = items;
                    items.sort_by_key(|a| std::cmp::Reverse(a.seeders.unwrap_or(0)));
                    let mut seen = std::collections::HashSet::new();
                    items.retain(|it| seen.insert(normalize(&it.title)));
                    self.rss.rss_feeds[idx].items = items;
                    self.rss.rss_feeds[idx].status = FeedStatus::Ok;
                    self.rss.rss_feeds[idx].error = None;
                }
                Err(e) => {
                    self.rss.rss_feeds[idx].status = FeedStatus::Error;
                    self.rss.rss_feeds[idx].error = Some(e);
                }
            }
        }
    }

    /// Auto-refresh enabled feeds whose `auto_refresh` flag is set.
    /// Re-checks every `cfg.rss_refresh_secs`; skips feeds already loading.
    pub(crate) fn auto_refresh_feeds(&mut self) {
        // Keep timestamps in sync with the feed list (add/remove).
        while self.rss.rss_last_refresh.len() < self.rss.rss_feeds.len() {
            self.rss.rss_last_refresh.push(Instant::now());
        }
        self.rss.rss_last_refresh.truncate(self.rss.rss_feeds.len());
        let interval = self.cfg.rss_refresh_secs;
        for i in 0..self.rss.rss_feeds.len() {
            let cfg = self.rss.rss_feeds[i].config.clone();
            if !cfg.enabled || !cfg.auto_refresh {
                continue;
            }
            if self.rss.rss_feeds[i].status == FeedStatus::Loading {
                continue;
            }
            if interval == 0 {
                continue;
            }
            let due = self.rss.rss_last_refresh[i].elapsed() >= Duration::from_secs(interval);
            if due {
                self.refresh_feed(i);
            }
        }
    }

    pub(crate) fn add_fav_from_rss(&mut self, item: &RssItem) {
        if self.cfg.favorites.iter().any(|f| f.title == item.title) {
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
        save_cfg(&self.cfg);
        self.toast("Saved to Favorites", self.pal.yellow);
    }

    pub(crate) fn cur_state(&self) -> SearchState {
        self.search
            .state
            .lock()
            .map(|g| g.clone())
            .unwrap_or(SearchState::Idle)
    }
    pub(crate) fn all_results(&self) -> Vec<TorrentResult> {
        self.search
            .results
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }
    pub(crate) fn total_count(&self) -> usize {
        self.search.count.lock().map(|g| *g).unwrap_or(0)
    }

    pub(crate) fn filtered(&self, raw: &[TorrentResult]) -> Vec<TorrentResult> {
        let min_s: u32 = self.search.f_seed.parse().unwrap_or(0);
        let max_b: u64 = self.search.f_size.parse::<f64>().unwrap_or(0.0).max(0.0) as u64;
        let max_b = max_b.saturating_mul(1_073_741_824);
        let min_y: u32 = self.search.f_year.parse().unwrap_or(0);
        let trk = self.search.f_trk.to_lowercase();
        let txt = self.search.f_text.to_lowercase();
        let mut seen = std::collections::HashSet::new();

        let mut out: Vec<_> = raw
            .iter()
            .filter(|r| {
                let s = r.seeders.unwrap_or(0);
                if s < min_s {
                    return false;
                }
                if max_b > 0 && r.size.unwrap_or(0) > max_b {
                    return false;
                }
                if min_y > 0 && r.publish_date.as_deref().map(pub_year).unwrap_or(0) < min_y {
                    return false;
                }
                if !trk.is_empty()
                    && !r
                        .tracker
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&trk)
                {
                    return false;
                }
                if !txt.is_empty() {
                    let hay = format!(
                        "{} {} {}",
                        r.title.to_lowercase(),
                        r.tracker.as_deref().unwrap_or("").to_lowercase(),
                        r.category_desc.as_deref().unwrap_or("").to_lowercase()
                    );
                    if !hay.contains(&txt) {
                        return false;
                    }
                }
                if !self.search.f_hlth.ok(s) {
                    return false;
                }
                if self.cfg.dedupe && !seen.insert(normalize(&r.title)) {
                    return false;
                }
                true
            })
            .cloned()
            .collect();

        out.sort_by(|a, b| {
            let c = match self.search.s_col {
                SortCol::Seeds => b.seeders.unwrap_or(0).cmp(&a.seeders.unwrap_or(0)),
                SortCol::Leech => b.peers.unwrap_or(0).cmp(&a.peers.unwrap_or(0)),
                SortCol::Ratio => {
                    let r = |x: &TorrentResult| {
                        let s = x.seeders.unwrap_or(0) as f64;
                        let l = x.peers.unwrap_or(0) as f64;
                        if l > 0.0 {
                            s / l
                        } else {
                            f64::INFINITY
                        }
                    };
                    r(b).partial_cmp(&r(a)).unwrap_or(std::cmp::Ordering::Equal)
                }
                SortCol::Size => b.size.unwrap_or(0).cmp(&a.size.unwrap_or(0)),
                SortCol::Name => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
                SortCol::Tracker => a
                    .tracker
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .cmp(&b.tracker.as_deref().unwrap_or("").to_lowercase()),
                SortCol::Date => b
                    .publish_date
                    .as_deref()
                    .unwrap_or("")
                    .cmp(a.publish_date.as_deref().unwrap_or("")),
            };
            if self.search.s_dir == SortDir::Asc {
                c.reverse()
            } else {
                c
            }
        });
        out
    }

    pub(crate) fn max_pages(&self, n: usize) -> usize {
        if self.cfg.page_size == 0 || n == 0 {
            return 1;
        }
        n.div_ceil(self.cfg.page_size)
    }
    pub(crate) fn page_slice<'a>(&self, v: &'a [TorrentResult]) -> &'a [TorrentResult] {
        if self.cfg.page_size == 0 {
            return v;
        }
        let s = self.search.page * self.cfg.page_size;
        if s >= v.len() {
            return &[];
        }
        &v[s..(s + self.cfg.page_size).min(v.len())]
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

    pub(crate) fn export_csv(&self, rows: &[TorrentResult]) {
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
                csv_esc(&r.publish_date.as_deref().map(time_ago).unwrap_or_default()),
            ));
        }
        if fs::write(&path, out).is_ok() {
            let _ = open::that(&path);
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
        let rn = egui::CornerRadius::same(6);
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
