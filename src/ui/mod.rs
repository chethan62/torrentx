//! UI drawing — split into per-screen modules. Shared imports, constants,

//! and helper widgets re-exported here so submodules can `use super::*;`.

mod about;
mod detail;
mod favorites;
mod filters;
mod header;
mod history;
mod idle;
mod paging;
mod results;
mod rss;
mod search;
mod tokens;
mod widgets;

pub(crate) use crate::config::{ROW_HEIGHT_COMPACT, ROW_HEIGHT_NORMAL, ROW_HEIGHT_ROOMY, save_cfg};
pub(crate) use crate::jackett::{
    Hlth, SearchState, SortCol, SortDir, Tab, TableCol, TorrentResult, cat_col, fmt_size, hlth_lbl,
    is_magnet, seed_col, time_ago,
};
pub(crate) use crate::rss::{FeedStatus, RssFeedConfig, RssFeedState, RssItem};
pub(crate) use crate::themes::{Pal, Theme, rgb, rgba, tint};
pub(crate) use crate::{
    CATS, MARGIN_DEFAULT, SvgIcon, act_btn, grid_row, icon_text_btn, icon_text_tab, labeled_input,
    lbl, outline_btn, outline_icon_btn, safe_open, status_icon_pill, svg_btn, svg_icon, svg_image,
    v_checkbox, wide_btn, wide_icon_btn,
};
pub(crate) use eframe::egui::{self, Color32, FontId, RichText, Stroke, Vec2};
pub(crate) use egui_extras::{Column, TableBuilder};
pub(crate) use tokens::*;

// ─── UI tuning constants ───────────────────────────────────────────────────
/// Filter-bar input widths (px), in row order.
pub(crate) const FILTER_TEXT_W: f32 = 115.0; // "within results"
pub(crate) const FILTER_NUM_W: f32 = 38.0; // Seeds ≥ / Max GB
pub(crate) const FILTER_YEAR_W: f32 = 44.0; // Year ≥
pub(crate) const FILTER_TRK_W: f32 = 86.0; // Tracker
/// Settings-panel input widths (px). URL/Key shrink with the window so they
/// never clip at narrow widths (min ensures they stay usable).
pub(crate) const SETTINGS_URL_W: f32 = 172.0;
pub(crate) const SETTINGS_KEY_W: f32 = 210.0;
pub(crate) const SETTINGS_URL_MIN: f32 = 120.0;
pub(crate) const SETTINGS_KEY_MIN: f32 = 130.0;
pub(crate) const SETTINGS_SMALL_W: f32 = 40.0;
/// RSS form input width (px).
pub(crate) const RSS_FORM_W: f32 = 260.0;
/// Favorites search input width (px).
pub(crate) const FAV_SEARCH_W: f32 = 220.0;
/// Below this the header, search bar and filter cells stack instead of sharing
/// a row. One breakpoint, in one place, so the header, the search bar and the
/// filter cells cannot disagree about when to reflow.
///
/// It is the width the five filter cells actually need (measured ~720 px of
/// available space) plus slack: the cells are label+field pairs that must not
/// split, so they cannot wrap on their own, and a breakpoint below their real
/// width would let the row run off the right edge. A phone in portrait sits far
/// below it; a desktop window sits far above it.
pub(crate) const NARROW_W: f32 = 760.0;

/// True when the window is phone-width (or a very narrow desktop window).
pub(crate) fn narrow(width: f32) -> bool {
    width < NARROW_W
}

/// Standard panel corner radius (px).
pub(crate) const PANEL_RADIUS: f32 = R_MD;
/// Standard panel side margin (px, i8 for egui Margin).
pub(crate) const PANEL_MARGIN_X: i8 = 12;

// Re-export the shared cell renderer so `search.rs` (and any submodule) can
// reach it via `use super::*;`.
pub(crate) use widgets::{band_inner, draw_cell_content};

// ─── The frame: `eframe::App::ui` ─────────────────────────────────────────
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::{
    QUIT, SHOW_REQ, START_MINIMIZED, TOGGLE_VIS, UI_CTX, WINDOW_HIDDEN, jackett, lock_or_recover,
    next_visible, wake_ui, wayland_session,
};

// Moved out of `main.rs`, which now only wires the app up. The trait impl stays
// here because everything it draws lives in this module tree.
use crate::app::App;

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // Publish the context once so the tray thread can wake this loop when a
        // tray item is activated (see `wake_ui`).
        let _ = UI_CTX.set(ctx.clone());
        self.apply_theme(&ctx);
        self.apply_ui_scale(&ctx);
        let state = self.cur_state();

        // --minimized. `with_visible(false)` alone does not hold on X11 (winit
        // maps the window during init), so hide it again on the first frame —
        // the same lever the tray's Show/Hide uses. On Wayland hiding is
        // impossible, so minimize instead (the taskbar restores it).
        if START_MINIMIZED.swap(false, std::sync::atomic::Ordering::SeqCst) {
            if wayland_session() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            } else {
                WINDOW_HIDDEN.store(true, std::sync::atomic::Ordering::SeqCst);
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
        }

        // A second launch asked us to show the window (the escape hatch for
        // --minimized that does not depend on the tray icon being visible).
        if SHOW_REQ.swap(false, std::sync::atomic::Ordering::SeqCst) && !wayland_session() {
            WINDOW_HIDDEN.store(false, std::sync::atomic::Ordering::SeqCst);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }

        // Tray "Quit" → exit the whole app.
        if QUIT.load(std::sync::atomic::Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        // Tray "Show / Hide" (X11) / "Minimize" (Wayland) → window visibility.
        if TOGGLE_VIS.swap(false, std::sync::atomic::Ordering::SeqCst) {
            if wayland_session() {
                // Wayland gives a client exactly one window-state lever: minimize.
                // `set_visible` is a no-op ("Not possible on Wayland"),
                // `set_minimized(false)` and `focus_window()` are ignored, and an
                // xdg-activation request does not unminimize on KWin (all verified
                // on KDE Wayland). So this is deliberately one-way — the menu item
                // says "Minimize" there, and restoring is the compositor's job
                // (panel / taskbar click).
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            } else {
                // X11: a hidden window is restorable, so keep true show/hide —
                // driven by our own record of the state, never by focus.
                let was_hidden = WINDOW_HIDDEN.fetch_xor(true, std::sync::atomic::Ordering::SeqCst);
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(next_visible(was_hidden)));
                ctx.request_repaint();
            }
        }

        // Fetch indexer list (background, so UI never blocks). Re-fetches when
        // the Jackett URL/key changes, and retries a minute after a failed
        // attempt — e.g. when Jackett is still booting at app start.
        let creds = (self.cfg.jackett_url.clone(), self.cfg.api_key.clone());
        let tried = self.net.indexers_fetched_for.as_ref() == Some(&creds);
        let retry_due = self
            .net
            .indexers_retry_at
            .is_none_or(|t| Instant::now() >= t);
        // `catalogue`, not `indexers`: the list is seeded with the native sources
        // (apibay, knaben) before Jackett is ever reached, so "indexers is empty"
        // stopped being a question about Jackett and the catalogue was never
        // fetched at startup — the picker showed 2/2 until "Refresh list".
        if self.net.catalogue.is_empty()
            && !self.net.indexers_loading
            && !self.cfg.api_key.is_empty()
            && (!tried || retry_due)
        {
            self.net.indexers_fetched_for = Some(creds);
            let url = self.cfg.jackett_url.clone();
            let key = self.cfg.api_key.clone();
            let handle = self.net.indexers_handle.clone();
            self.net.indexers_loading = true;
            std::thread::spawn(move || {
                // Disk cache first: 2.3 MB of XML per launch is pure waste when
                // Jackett's indexer set has not changed. "Refresh list" in the
                // indexer window calls `fetch_indexers` directly, bypassing this.
                let list = jackett::cached_indexers(&url, &key)
                    .or_else(|| jackett::fetch_indexers(&url, &key));
                let _ = handle.send(list);
                // Wake the parked UI loop: otherwise the Jackett status dot and the
                // indexer list stay stale until the user happens to move the mouse.
                wake_ui();
            });
        }
        // Drain indexer fetch result
        if self.net.indexers_loading
            && let Ok(list) = self.net.indexers_rx.try_recv()
        {
            self.net.jackett_ok = Some(list.is_some()); // None = Jackett unreachable
            match list {
                Some(cat) => {
                    self.net.indexers = crate::sources::with_native(&cat.configured);
                    self.net.catalogue = cat.all;
                    self.net.indexers_retry_at = None;
                }
                None => {
                    // Unreachable — back off before retrying.
                    self.net.indexers_retry_at = Some(Instant::now() + Duration::from_secs(60));
                }
            }
            self.net.indexers_loading = false;
        }

        // Check for updates once at startup (background); show a toast when
        // found. Opt-out via Settings (check_updates).
        if !self.net.update_checked {
            self.net.update_checked = true;
            if self.cfg.check_updates {
                let cur = env!("CARGO_PKG_VERSION").to_string();
                let update_tx = self.net.update_tx.clone();
                std::thread::spawn(move || {
                    let new = jackett::check_update(&cur);
                    let _ = update_tx.send(new);
                    // Wake the parked loop so the "update available" toast actually
                    // renders instead of waiting for the next input event.
                    wake_ui();
                });
            }
        }
        if let Ok(Some(new)) = self.net.update_rx.try_recv() {
            self.toast(&format!("Update available: {new}"), self.pal.accent);
        }

        // Spinner tick — keeps the Searching state repainting so the elapsed
        // timer updates live. (The Braille spinner was replaced by an SVG
        // Refresh icon; spin_i/t removed.)
        if state == SearchState::Searching {
            ctx.request_repaint_after(Duration::from_millis(80));

            // Watchdog: if the search thread never completes (dead thread,
            // hung connection), force Error so the spinner stops and the UI
            // recovers instead of spinning at 12fps forever.
            if let Some(t) = self.ui.t_start {
                let budget = Duration::from_secs(self.cfg.timeout_secs.max(10) + 15);
                if t.elapsed() > budget {
                    jackett::set_err(
                        &self.search.state,
                        format!(
                            "Search timed out after {}s — check Jackett is running",
                            budget.as_secs()
                        ),
                    );
                    self.ui.t_start = None;
                    ctx.request_repaint();
                }
            }
        }
        if matches!(state, SearchState::Done | SearchState::Error(_)) {
            if let Some(t) = self.ui.t_start.take() {
                self.ui.t_done = Some(t.elapsed().as_secs_f64());
            }
            // One dedupe pass when the search settles, so the badge, the status
            // line and the table all report the same number.
            self.finalize_results();
        }

        // Toast decay + animation progress
        let dt = ctx.input(|i| i.unstable_dt).clamp(0.0, 0.1);
        self.ui.toasts.retain_mut(|t| {
            t.ttl -= dt;
            // Animate in (0.15s) and out (handled by fade in draw_toasts)
            t.anim_progress = (t.anim_progress + dt / 0.15).min(1.0);
            t.ttl > 0.0
        });
        // Keep painting while a toast lives: its TTL and fade only advance on
        // frames, and the loop parks when nothing requests a repaint — otherwise a
        // toast freezes on screen and covers the UI until the next input event.
        if !self.ui.toasts.is_empty() {
            ctx.request_repaint();
        }

        // Persist window size (throttled to one write per 3s while resizing)
        // so the next launch reopens at this size.
        let vr = ctx.input(|i| i.viewport_rect());
        if vr.width() >= 820.0 && vr.height() >= 560.0 {
            let size = [vr.width(), vr.height()];
            if self.cfg.win_size != Some(size)
                && self
                    .ui
                    .win_save_at
                    .is_none_or(|t| t.elapsed() >= Duration::from_secs(3))
            {
                self.ui.win_save_at = Some(Instant::now());
                self.cfg.win_size = Some(size);
                let _ = save_cfg(&self.cfg);
            }
        }

        // Search state transition animation
        if self.ui.prev_search_state != state {
            self.ui.prev_search_state = state.clone();
            self.ui.search_state_anim = 0.0;
        } else {
            self.ui.search_state_anim =
                (self.ui.search_state_anim + dt * 1000.0 / MOTION_MS as f32).min(1.0);
        }

        // Table content animation — reset when the page changes (page turns,
        // filters/sort changed → page was reset to 0 by the caller)
        if self.ui.last_table_page != self.search.page {
            self.ui.last_table_page = self.search.page;
            self.ui.table_anim = 0.0;
        } else {
            self.ui.table_anim = (self.ui.table_anim + dt / 0.25).min(1.0);
        }

        // Detail panel open/close animation — drive toward the target state.
        // Both the search detail panel (detail_open) and the RSS item panel
        // (rss_detail) share this one clock; whichever is open drives it.
        let target = if self.ui.detail_open || self.rss.rss_detail.is_some() {
            1.0
        } else {
            0.0
        };
        let speed = if self.ui.detail_open {
            1000.0 / MOTION_MS as f32
        } else {
            1.0 / 0.15
        };
        let delta = speed * dt;
        self.ui.detail_anim = if self.ui.detail_anim < target {
            (self.ui.detail_anim + delta).min(target)
        } else {
            (self.ui.detail_anim - delta).max(target)
        };
        // The loop parks when idle, so a fade that starts from an Escape must be
        // driven to the end by itself; and once it is over, the panel content is
        // dropped instead of being drawn at opacity 0 forever (it also kept the
        // table narrowed by its width).
        if (self.ui.detail_anim - target).abs() > f32::EPSILON {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        } else if target == 0.0 && self.rss.rss_detail.is_none() && self.ui.tab == Tab::Search {
            self.ui.detail_row = None;
        }
        self.ui.prev_detail_open = self.ui.detail_open;

        // Filter+sort the results once per frame; every consumer below
        // (batch handlers, search tab, detail panel) shares this view.
        // Partial results are shown while Searching: the fan-out fills the list
        // indexer by indexer, so page 1 is usable before the last tracker answers.
        let view = if matches!(state, SearchState::Done | SearchState::Searching)
            && self.total_count() > 0
        {
            Some(self.compute_view())
        } else {
            None
        };

        // Global shortcuts
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::F)) {
            ctx.memory_mut(|m| m.request_focus(egui::Id::unique("q")));
        }
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::R)) {
            self.do_search();
        }
        // Ctrl+A — select all visible results (batch mode auto-enables).
        // Skipped while a TextEdit owns the keyboard, so select-all still
        // works inside the query/filters/settings inputs.
        let editing = ctx.memory(|m| m.focused().is_some());
        if !editing
            && ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::A))
            && let Some(v) = &view
            && !v.sorted.is_empty()
        {
            self.ui.sel_mode = true;
            let page_s = self.page_slice(&v.sorted);
            let base = self.search.page * self.cfg.page_size;
            self.ui.sel_set = (0..page_s.len()).map(|i| base + i).collect();
            self.toast(
                &format!("Selected {} results", page_s.len()),
                self.pal.green,
            );
        }
        // Ctrl +/- zoom the whole interface, Ctrl+0 resets — the same gesture as a
        // browser, so the size control is reachable without opening Settings.
        if !editing {
            // Count the key events, not `key_pressed`: several presses can land in
            // one frame (a fast double tap, or a held key), and a bool collapsed
            // them into a single step.
            let (up, down, reset) = ctx.input(|i| {
                let (mut up, mut down, mut reset) = (0i32, 0i32, false);
                for e in &i.events {
                    if let egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } = e
                    {
                        if !(modifiers.ctrl || modifiers.command) {
                            continue;
                        }
                        match key {
                            egui::Key::Equals | egui::Key::Plus => up += 1,
                            egui::Key::Minus => down += 1,
                            egui::Key::Num0 => reset = true,
                            _ => {}
                        }
                    }
                }
                (up, down, reset)
            });
            if up != 0 || down != 0 {
                self.cfg.ui_scale =
                    crate::config::clamp_scale(self.cfg.ui_scale + 0.05 * (up - down) as f32);
                let _ = save_cfg(&self.cfg);
            } else if reset && self.cfg.ui_scale != 1.0 {
                self.cfg.ui_scale = 1.0;
                let _ = save_cfg(&self.cfg);
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.ui.detail_open {
                self.close_detail();
            } else if self.rss.rss_detail.is_some() {
                // The RSS tab keeps its own detail index: Esc must close that panel
                // instead of silently clearing the (hidden) search query.
                self.rss.rss_detail = None;
                self.ui.detail_row = None;
            } else if ctx.memory(|m| m.focused()).is_none() {
                // Only clear the query when no text field owns the keyboard.
                self.clear_search();
            }
        }
        // Copy magnet from detail panel with Ctrl+C — only when no TextEdit
        // is focused (don't clobber the clipboard while copying typed text).
        if self.ui.detail_open
            && !editing
            && ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::C))
            && let (Some(idx), Some(v)) = (self.ui.selected, &view)
        {
            let page_s = self.page_slice(&v.sorted);
            if let Some(r) = page_s.get(idx)
                && let Some(m) = &r.magnet_uri
            {
                ui.ctx().copy_text(m.clone());
                self.toast("Magnet copied", self.pal.green);
            }
        }

        // ── Status bar ───────────────────────────────────────────────────
        egui::Panel::bottom("sb")
            .default_size(26.0)
            .frame(
                egui::Frame::NONE
                    .fill(self.pal.hdr)
                    .stroke(Stroke::new(1.0_f32, self.pal.border))
                    .inner_margin(margin(SP4, SP2)),
            )
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    match &state {
                        SearchState::Idle => {
                            lbl(
                                ui,
                                "Ready — type a query and press Search",
                                self.pal.dim,
                                12.0,
                            );
                        }
                        SearchState::Searching => {
                            let el = self
                                .ui
                                .t_start
                                .as_ref()
                                .map(|t| format!("  {:.1}s", t.elapsed().as_secs_f64()))
                                .unwrap_or_default();
                            // SVG Refresh icon — the Braille-dot spinner (⣾…)
                            // tofus on fonts without Braille; vector icon is
                            // consistent with the Check/Close status icons.
                            svg_icon(ui, SvgIcon::Refresh, ICON_SM, self.pal.accent);
                            let n = self.total_count();
                            // Rows can all be on screen while one slow tracker
                            // still holds the search open; say which, or the wait
                            // reads as a hang (measured: 8 indexers in ≤1.2 s, 1 in
                            // 12-18 s).
                            let (done, asked) = (
                                self.search.answered.load(Ordering::Relaxed),
                                self.search.targets.load(Ordering::Relaxed),
                            );
                            let idx_note = if asked > 1 && done < asked {
                                format!("  ·  {done}/{asked} indexers")
                            } else {
                                String::new()
                            };
                            let failed =
                                crate::jackett::failed_note(&lock_or_recover(&self.search.failed));
                            lbl(
                                ui,
                                &format!(
                                    "Searching \"{}\" — {} so far{}{}{failed}",
                                    self.search.last_query, n, el, idx_note
                                ),
                                self.pal.accent,
                                12.0,
                            );
                        }
                        SearchState::Done => {
                            let n = self.total_count();
                            let e = self
                                .ui
                                .t_done
                                .map(|e| format!("  ({:.1}s)", e))
                                .unwrap_or_default();
                            svg_icon(ui, SvgIcon::Check, ICON_SM, self.pal.green);
                            // The search settles on the rows it has; a straggler may
                            // still be appending, so keep counting it out loud.
                            let (done, asked) = (
                                self.search.answered.load(Ordering::Relaxed),
                                self.search.targets.load(Ordering::Relaxed),
                            );
                            let idx_note = if asked > 1 && done < asked {
                                format!("  ·  {}/{asked} indexers", done)
                            } else {
                                String::new()
                            };
                            let failed =
                                crate::jackett::failed_note(&lock_or_recover(&self.search.failed));
                            lbl(
                                ui,
                                &format!(
                                    "{n} results for \"{}\"{}{idx_note}{failed}",
                                    self.search.last_query, e
                                ),
                                self.pal.green,
                                12.0,
                            );
                        }
                        SearchState::Error(e) => {
                            svg_icon(ui, SvgIcon::Close, ICON_SM, self.pal.red);
                            lbl(ui, e.lines().next().unwrap_or(e), self.pal.red, 12.0);
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        lbl(
                            ui,
                            // The list is a hint, but it has to be a true one:
                            // every shortcut named here is bound, and Ctrl +/-/0
                            // is the one people reach for when text looks small.
                            "Keys: Arrows · Enter · D · F · M · Esc · Ctrl+F · Ctrl+R · Ctrl +/-",
                            self.pal.dim,
                            11.5,
                        );
                    });
                });
            });

        // ── Header ───────────────────────────────────────────────────────
        self.draw_header(ui);

        // Drop row-indexed selection whenever the view changes underneath it
        // (sort, filter, page, page size, new result set): the highlight, the
        // detail panel and the batch set would otherwise target a different
        // torrent than the one the user picked.
        let sig = self.view_signature();
        if self.ui.view_sig.is_some_and(|prev| prev != sig) {
            self.clear_row_selection();
        }
        self.ui.view_sig = Some(sig);

        // ── RSS polling ───────────────────────────────────────────────
        self.poll_rss();
        self.poll_pick();
        // Wake again when the next auto-refresh is due; without this the loop
        // stays parked while the app is idle and the timer never fires.
        if let Some(after) = self.auto_refresh_feeds() {
            ctx.request_repaint_after(after);
        }

        // ── Settings ─────────────────────────────────────────────────────
        // A floating Preferences window: configuration is not part of the
        // search flow, so it must not push the search bar and results down
        // (the old top panel reflowed the whole workspace every time it
        // opened). Fixed width keeps its three rows on one line each.
        if self.ui.show_settings {
            let mut open = true;
            egui::Window::new("Settings")
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                // Wide enough that each section keeps one line at the default
                // font size; the height follows the content up to a cap.
                .default_width(1000.0)
                .min_width(720.0)
                .max_height(760.0)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .frame(
                    egui::Frame::NONE
                        .fill(self.pal.hdr)
                        .corner_radius(R_LG)
                        .stroke(Stroke::new(1.0_f32, self.pal.border))
                        .shadow(egui::epaint::Shadow {
                            offset: [0, 8],
                            blur: 24,
                            spread: 0,
                            color: egui::Color32::from_black_alpha(90),
                        })
                        .inner_margin(margin(SP4, SP4)),
                )
                .show(&ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("settings_scroll")
                        .max_height(540.0)
                        .show(ui, |ui| self.draw_settings_panel(ui));
                });
            if !open {
                self.set_settings_open(false);
            }
        }

        // ── Detail panels — frame-level right panels ─────────────────────
        // Added BEFORE CentralPanel so egui reserves the right strip and the
        // table shrinks to fit (no overlap). A plain Panel::right at frame
        // level is the correct egui pattern.
        self.draw_detail_panel(ui, view.as_ref());
        self.draw_rss_detail_panel(ui);

        // ── Central panel ────────────────────────────────────────────────
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(self.pal.bg))
            .show(ui, |ui| match self.ui.tab.clone() {
                Tab::Search => self.draw_search(ui, &ctx, &state, view.as_ref()),
                Tab::Favorites => self.draw_favorites(ui),
                Tab::Rss => self.draw_rss(ui, &ctx),
                Tab::About => self.draw_about(ui),
            });

        self.draw_toasts(&ctx);
    }
}
