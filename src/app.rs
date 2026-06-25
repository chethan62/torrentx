use std::collections::HashSet;
use std::fs;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use eframe::egui;

use crate::config::{Config, Favorite, load_cfg, save_cfg};
use crate::rss::{FeedStatus, RssFeedConfig, RssFeedState, RssItem, start_rss_fetch};
use crate::search::start_search;
use crate::theme::Pal;
use crate::types::*;
use crate::utils::*;
use crate::ui;

// ─── App ───────────────────────────────────────────────────────────────────

pub struct App {
    // ── Persistent config ─────────────────────────────
    pub cfg: Config,
    pub pal: Pal,

    // ── Search state ──────────────────────────────────
    pub query:       String,
    pub search_cat:  String,
    pub results:     Arc<Mutex<Vec<TorrentResult>>>,
    pub search_state: Arc<Mutex<SearchState>>,
    pub result_count: Arc<Mutex<usize>>,
    pub last_query:  String,

    // ── Filters + sort ────────────────────────────────
    pub filters: FilterState,
    pub sort_col: SortCol,
    pub sort_dir: SortDir,

    // ── UI state ──────────────────────────────────────
    pub tab:          Tab,
    pub show_settings: bool,
    pub key_vis:      bool,
    pub selected:     Option<usize>,
    pub detail_open:  bool,
    pub show_hist:    bool,
    pub page:         usize,
    pub hovered:      Option<usize>,
    pub toasts:       Vec<Toast>,
    pub fav_search:   String,

    // ── RSS state ─────────────────────────────────────
    pub rss_feeds:         Vec<RssFeedState>,
    pub rss_tx:            mpsc::SyncSender<(usize, Result<Vec<RssItem>, String>)>,
    pub rss_rx:            mpsc::Receiver<(usize, Result<Vec<RssItem>, String>)>,
    pub rss_selected:      usize,
    pub rss_filter:        String,
    pub rss_detail:        Option<usize>,
    pub rss_add_mode:      bool,
    pub rss_new_cfg:       RssFeedConfig,
    pub rss_edit_idx:      Option<usize>,

    // ── Spinner / timing ──────────────────────────────
    pub t_start: Option<Instant>,
    pub t_done:  Option<f64>,
    pub spin_i:  usize,
    pub spin_t:  f32,
}

impl Default for App {
    fn default() -> Self {
        let cfg = load_cfg();
        let pal = Pal::from(&cfg.theme);
        let rss_feeds = cfg.rss_feeds.iter().map(|c| RssFeedState::new(c.clone())).collect();
        let (rss_tx, rss_rx) = mpsc::sync_channel(128);
        Self {
            cfg, pal, rss_feeds, rss_tx, rss_rx,
            query:        String::new(),
            search_cat:   "All".into(),
            results:      Arc::new(Mutex::new(vec![])),
            search_state: Arc::new(Mutex::new(SearchState::Idle)),
            result_count: Arc::new(Mutex::new(0)),
            last_query:   String::new(),
            filters:      FilterState::default(),
            sort_col:     SortCol::Seeds,
            sort_dir:     SortDir::Desc,
            tab:          Tab::Search,
            show_settings: false,
            key_vis:      false,
            selected:     None,
            detail_open:  false,
            show_hist:    false,
            page:         0,
            hovered:      None,
            toasts:       vec![],
            fav_search:   String::new(),
            rss_selected: 0,
            rss_filter:   String::new(),
            rss_detail:   None,
            rss_add_mode: false,
            rss_new_cfg:  RssFeedConfig::new_default(),
            rss_edit_idx: None,
            t_start: None,
            t_done:  None,
            spin_i:  0,
            spin_t:  0.0,
        }
    }
}

// ─── Core app methods ──────────────────────────────────────────────────────

impl App {
    // ── Search ────────────────────────────────────────

    pub fn do_search(&mut self) {
        let q = self.query.trim().to_string();
        if q.is_empty() { return; }
        if self.cfg.api_key.trim().is_empty() {
            self.set_search_state(SearchState::Error(
                "No API key — open Settings and paste your Jackett API key.".into(),
            ));
            self.show_settings = true;
            return;
        }
        self.cfg.history.retain(|h| h != &q);
        self.cfg.history.insert(0, q.clone());
        self.cfg.history.truncate(20);
        save_cfg(&self.cfg);

        self.selected    = None;
        self.detail_open = false;
        self.show_hist   = false;
        self.page        = 0;
        self.last_query  = q.clone();
        self.hovered     = None;
        self.t_start     = Some(Instant::now());
        self.t_done      = None;
        self.filters.reset();

        if let Ok(mut r) = self.results.lock()      { r.clear(); }
        if let Ok(mut c) = self.result_count.lock() { *c = 0; }

        start_search(
            self.cfg.jackett_url.clone(),
            self.cfg.api_key.clone(),
            q,
            self.search_cat.clone(),
            self.cfg.timeout_secs,
            Arc::clone(&self.results),
            Arc::clone(&self.search_state),
            Arc::clone(&self.result_count),
        );
    }

    fn set_search_state(&self, s: SearchState) {
        if let Ok(mut g) = self.search_state.lock() { *g = s; }
    }

    pub fn cur_search_state(&self) -> SearchState {
        self.search_state.lock().map(|g| g.clone()).unwrap_or(SearchState::Idle)
    }

    pub fn all_results(&self) -> Vec<TorrentResult> {
        self.results.lock().map(|g| g.clone()).unwrap_or_default()
    }

    pub fn total_count(&self) -> usize {
        self.result_count.lock().map(|g| *g).unwrap_or(0)
    }

    // ── Filtering / sorting ───────────────────────────

    pub fn filtered(&self, raw: &[TorrentResult]) -> Vec<TorrentResult> {
        let min_s: u32 = self.filters.min_seeds.parse().unwrap_or(0);
        let max_b: u64 = self.filters.max_gb.parse::<f64>().unwrap_or(0.0) as u64 * 1_073_741_824;
        let min_y: u32 = self.filters.min_year.parse().unwrap_or(0);
        let trk   = self.filters.tracker.to_lowercase();
        let txt   = self.filters.text.to_lowercase();
        let mut seen: HashSet<String> = HashSet::new();

        let mut out: Vec<_> = raw.iter().filter(|r| {
            let s = r.seeders.unwrap_or(0);
            if s < min_s { return false; }
            if max_b > 0 && r.size.unwrap_or(0) > max_b { return false; }
            if min_y > 0 {
                let yr = r.publish_date.as_deref().map(pub_year).unwrap_or(0);
                if yr < min_y { return false; }
            }
            if !trk.is_empty() && !r.tracker.as_deref().unwrap_or("").to_lowercase().contains(&trk) {
                return false;
            }
            if !txt.is_empty() {
                let hay = format!("{} {} {}",
                    r.title.to_lowercase(),
                    r.tracker.as_deref().unwrap_or("").to_lowercase(),
                    r.category_desc.as_deref().unwrap_or("").to_lowercase());
                if !hay.contains(&txt) { return false; }
            }
            if !self.filters.health.matches(s) { return false; }
            if self.cfg.dedupe && !seen.insert(normalize_title(&r.title)) { return false; }
            true
        }).cloned().collect();

        out.sort_by(|a, b| {
            let c = match self.sort_col {
                SortCol::Seeds   => b.seeders.unwrap_or(0).cmp(&a.seeders.unwrap_or(0)),
                SortCol::Leech   => b.peers.unwrap_or(0).cmp(&a.peers.unwrap_or(0)),
                SortCol::Size    => b.size.unwrap_or(0).cmp(&a.size.unwrap_or(0)),
                SortCol::Name    => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
                SortCol::Tracker => a.tracker.as_deref().unwrap_or("").to_lowercase()
                                    .cmp(&b.tracker.as_deref().unwrap_or("").to_lowercase()),
                SortCol::Date    => b.publish_date.as_deref().unwrap_or("")
                                    .cmp(a.publish_date.as_deref().unwrap_or("")),
            };
            if self.sort_dir == SortDir::Asc { c.reverse() } else { c }
        });
        out
    }

    pub fn max_pages(&self, n: usize) -> usize {
        if self.cfg.page_size == 0 || n == 0 { return 1; }
        (n + self.cfg.page_size - 1) / self.cfg.page_size
    }

    pub fn page_slice<'a>(&self, v: &'a [TorrentResult]) -> &'a [TorrentResult] {
        if self.cfg.page_size == 0 { return v; }
        let s = self.page * self.cfg.page_size;
        if s >= v.len() { return &[]; }
        &v[s..(s + self.cfg.page_size).min(v.len())]
    }

    pub fn cat_chips(results: &[TorrentResult]) -> Vec<(String, usize, eframe::egui::Color32)> {
        let mut map: std::collections::BTreeMap<String, usize> = Default::default();
        for r in results {
            let c = r.category_desc.as_deref()
                .and_then(|c| c.split('/').next())
                .unwrap_or("Other").trim().to_string();
            *map.entry(c).or_insert(0) += 1;
        }
        let mut v: Vec<_> = map.into_iter()
            .map(|(k, n)| { let col = cat_color(&k); (k, n, col) })
            .collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        v.truncate(7);
        v
    }

    // ── Favorites ─────────────────────────────────────

    pub fn add_fav(&mut self, r: &TorrentResult) {
        if self.cfg.favorites.iter().any(|f| f.title == r.title) {
            self.toast("Already in Favorites", self.pal.yellow);
            return;
        }
        self.cfg.favorites.push(Favorite {
            title: r.title.clone(), magnet: r.magnet_uri.clone(), link: r.link.clone(),
            tracker: r.tracker.clone(), size: r.size, seeders: r.seeders,
            saved_at: now_str(),
        });
        save_cfg(&self.cfg);
        self.toast("Saved to Favorites ★", self.pal.yellow);
    }

    pub fn add_fav_from_rss(&mut self, item: &RssItem) {
        if self.cfg.favorites.iter().any(|f| f.title == item.title) {
            self.toast("Already in Favorites", self.pal.yellow);
            return;
        }
        self.cfg.favorites.push(Favorite {
            title: item.title.clone(), magnet: item.magnet.clone(), link: item.link.clone(),
            tracker: item.tracker.clone(), size: item.size, seeders: item.seeders,
            saved_at: now_str(),
        });
        save_cfg(&self.cfg);
        self.toast("Saved to Favorites ★", self.pal.yellow);
    }

    // ── RSS ───────────────────────────────────────────

    pub fn sync_rss_configs(&mut self) {
        self.cfg.rss_feeds = self.rss_feeds.iter().map(|s| s.config.clone()).collect();
        save_cfg(&self.cfg);
    }

    pub fn refresh_feed(&mut self, idx: usize) {
        if let Some(feed) = self.rss_feeds.get_mut(idx) {
            feed.status = FeedStatus::Loading;
            let cfg = feed.config.clone();
            start_rss_fetch(
                self.cfg.jackett_url.clone(),
                self.cfg.api_key.clone(),
                cfg,
                self.cfg.timeout_secs,
                idx,
                self.rss_tx.clone(),
            );
        }
    }

    pub fn refresh_all_feeds(&mut self) {
        let len = self.rss_feeds.len();
        for i in 0..len {
            if self.rss_feeds[i].config.enabled {
                self.refresh_feed(i);
            }
        }
    }

    fn poll_rss_results(&mut self) {
        while let Ok((idx, result)) = self.rss_rx.try_recv() {
            if let Some(feed) = self.rss_feeds.get_mut(idx) {
                let (toast_msg, toast_ok) = match result {
                    Ok(items) => {
                        let n = items.len();
                        let msg = format!("✓ {} — {} items", feed.config.name, n);
                        feed.items        = items;
                        feed.status       = FeedStatus::Ok;
                        feed.last_fetched = Some(Instant::now());
                        feed.error        = None;
                        (msg, true)
                    }
                    Err(e) => {
                        let msg = format!("✕ {} — {}", feed.config.name, &e[..e.len().min(40)]);
                        feed.status       = FeedStatus::Error;
                        feed.error        = Some(e);
                        feed.last_fetched = Some(Instant::now());
                        (msg, false)
                    }
                };
                let col = if toast_ok { self.pal.green } else { self.pal.red };
                self.toast(&toast_msg, col);
            }
        }
    }

    fn auto_refresh_feeds(&mut self) {
        let interval = Duration::from_secs(self.cfg.rss_refresh_min.max(1) * 60);
        let now      = Instant::now();
        let mut to_refresh: Vec<(usize, RssFeedConfig)> = vec![];

        for (i, feed) in self.rss_feeds.iter_mut().enumerate() {
            if !feed.config.enabled || !feed.config.auto_refresh { continue; }
            if feed.status == FeedStatus::Loading { continue; }
            let stale = match feed.last_fetched {
                None    => true,
                Some(t) => now.duration_since(t) > interval,
            };
            if stale {
                feed.status = FeedStatus::Loading;
                to_refresh.push((i, feed.config.clone()));
            }
        }

        for (idx, cfg) in to_refresh {
            start_rss_fetch(
                self.cfg.jackett_url.clone(),
                self.cfg.api_key.clone(),
                cfg,
                self.cfg.timeout_secs,
                idx,
                self.rss_tx.clone(),
            );
        }
    }

    // ── Theme ─────────────────────────────────────────

    pub fn set_theme(&mut self, t: crate::theme::Theme) {
        self.cfg.theme = t;
        self.pal       = Pal::from(&self.cfg.theme);
        save_cfg(&self.cfg);
    }

    // ── Toast ─────────────────────────────────────────

    pub fn toast(&mut self, msg: &str, col: eframe::egui::Color32) {
        self.toasts.retain(|t| t.msg != msg);
        self.toasts.push(Toast { msg: msg.into(), ttl: 3.0, col });
    }

    // ── Export ────────────────────────────────────────

    pub fn export_csv(&self, rows: &[TorrentResult]) {
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
}

// ─── eframe::App ──────────────────────────────────────────────────────────

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.pal.apply_to_ctx(ctx);
        let state = self.cur_search_state();

        // ── Spinner animation ──────────────────────────
        if state == SearchState::Searching {
            ctx.request_repaint_after(Duration::from_millis(80));
            let dt = ctx.input(|i| i.unstable_dt).clamp(0.0, 0.1);
            self.spin_t += dt;
            if self.spin_t > 0.1 {
                self.spin_t = 0.0;
                self.spin_i = (self.spin_i + 1) % SPINNER_FRAMES.len();
            }
        }
        if matches!(state, SearchState::Done | SearchState::Error(_)) {
            if let Some(t) = self.t_start.take() { self.t_done = Some(t.elapsed().as_secs_f64()); }
        }

        // ── RSS polling + auto-refresh ─────────────────
        self.poll_rss_results();
        self.auto_refresh_feeds();
        if self.rss_feeds.iter().any(|f| f.status == FeedStatus::Loading) {
            ctx.request_repaint_after(Duration::from_millis(400));
        }

        // ── Toast decay ───────────────────────────────
        let dt = ctx.input(|i| i.unstable_dt).clamp(0.0, 0.5);
        self.toasts.retain_mut(|t| { t.ttl -= dt; t.ttl > 0.0 });

        // ── Global keyboard shortcuts ─────────────────
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::F)) {
            ctx.memory_mut(|m| m.request_focus(egui::Id::new("search_query")));
        }
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::R)) {
            self.do_search();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.detail_open { self.detail_open = false; }
            else if self.rss_detail.is_some() { self.rss_detail = None; }
            else if self.rss_add_mode { self.rss_add_mode = false; }
            else { self.query.clear(); self.show_hist = false; }
        }

        // ── Draw ──────────────────────────────────────
        ui::status_bar::draw(self, ctx, &state);
        ui::header::draw(self, ctx);

        if self.show_settings {
            ui::settings::draw(self, ctx);
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(self.pal.bg))
            .show(ctx, |ui| {
                match self.tab.clone() {
                    Tab::Search    => ui::search_tab::draw(self, ui, ctx, &state),
                    Tab::Rss       => ui::rss_tab::draw(self, ui, ctx),
                    Tab::Favorites => ui::favorites_tab::draw(self, ui),
                    Tab::About     => ui::about_tab::draw(self, ui),
                }
            });

        ui::toasts::draw(self, ctx);
    }
}
