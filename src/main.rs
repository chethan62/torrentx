#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]


// ─── Modules ────────────────────────────────────────────────────────
mod app;
mod config;
mod jackett;
mod rss;
mod themes;
mod ui;

use app::App;

use jackett::{SearchState, Tab};
use themes::{tint, Pal};

use eframe::egui::{self, Color32, FontId, RichText, Stroke, Vec2};
use std::time::Duration;

// ─── Constants ─────────────────────────────────────────────────────────────

pub(crate) const MARGIN_DEFAULT: f32 = 12.0;


pub(crate) const CATS: &[&str] = &["All", "Movies", "TV", "Music", "PC Games", "Software", "Anime", "Books", "XXX"];
pub(crate) const SPIN: &[&str] = &["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"];

/// CSV field escaping: wrap in quotes, double any embedded quotes.
pub(crate) fn csv_esc(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}


// ─── Small UI buttons ──────────────────────────────────────────────────────

pub(crate) fn act_btn(ui: &mut egui::Ui, label: &str, tip: &str, color: Color32) -> bool {
    ui.add(
        egui::Button::new(RichText::new(label).size(11.5).color(color))
            .fill(tint(color, 18))
            .stroke(Stroke::new(1.0_f32, tint(color, 70)))
            .corner_radius(5.0)
            .min_size(Vec2::new(0.0, 25.0))
    ).on_hover_text(tip).clicked()
}

pub(crate) fn status_pill(ui: &mut egui::Ui, label: &str, col: Color32) {
    egui::Frame::NONE.fill(tint(col, 20)).corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(6, 2))
        .show(ui, |ui| { ui.label(RichText::new(label).font(FontId::proportional(10.5)).color(col)); });
}

pub(crate) fn wide_btn(ui: &mut egui::Ui, label: &str, color: Color32) -> bool {
    let w = ui.available_width().max(200.0);
    ui.add(
        egui::Button::new(RichText::new(label).font(FontId::proportional(13.0)).color(color))
            .fill(tint(color, 18))
            .stroke(Stroke::new(1.0_f32, tint(color, 80)))
            .corner_radius(6.0)
            .min_size(Vec2::new(w, 34.0))
    ).clicked()
}

pub(crate) fn outline_btn(ui: &mut egui::Ui, label: &str, color: Color32) -> bool {
    ui.add(
        egui::Button::new(RichText::new(label).font(FontId::proportional(12.0)).color(color))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0_f32, tint(color, 80)))
            .corner_radius(4.0)
    ).clicked()
}

pub(crate) fn lbl(ui: &mut egui::Ui, text: &str, color: Color32, fs: f32) {
    ui.label(RichText::new(text).font(FontId::proportional(fs)).color(color));
}

/// A labeled single-line input: `Label: [input]` on one row.
pub(crate) fn labeled_input(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    width: f32,
    hint: &str,
    fs: f32,
    dim: Color32,
) {
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        lbl(ui, label, dim, fs);
        ui.add_space(4.0);
        ui.add(egui::TextEdit::singleline(value)
            .desired_width(width).hint_text(hint).font(FontId::proportional(fs)));
    });
}

// ─── App methods ───────────────────────────────────────────────────────────

// ─── (logic methods moved to app.rs) ──────────────────────────────
// ─── eframe::App main loop ─────────────────────────────────────────────────

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.apply_theme(&ctx);
        let state = self.cur_state();

        // Minimize-to-tray: intercept window close (unless tray "Quit" was used).
        if ctx.input(|i| i.viewport().close_requested())
            && !QUIT.load(std::sync::atomic::Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
        // Tray "Quit" → exit the whole app.
        if QUIT.load(std::sync::atomic::Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        // Tray "Show / Hide" → toggle window visibility.
        if TOGGLE_VIS.swap(false, std::sync::atomic::Ordering::SeqCst) {
            let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(!focused));
            if focused { ctx.request_repaint(); }
        }

        // Fetch indexer list once (background, so UI never blocks)
        if self.indexers.is_empty() && !self.indexers_loading && !self.cfg.api_key.is_empty() {
            let url = self.cfg.jackett_url.clone();
            let key = self.cfg.api_key.clone();
            let handle = self.indexers_handle.clone();
            self.indexers_loading = true;
            std::thread::spawn(move || {
                let list = jackett::fetch_indexers(&url, &key);
                let _ = handle.send(list);
            });
        }
        // Drain indexer fetch result
        if self.indexers_loading {
            if let Ok(list) = self.indexers_rx.try_recv() {
                self.jackett_ok = Some(list.is_some()); // None = Jackett unreachable
                if let Some(l) = list {
                    self.indexers = l;
                }
                self.indexers_loading = false;
            }
        }

        // Check for updates once at startup (background); show a toast when found
        if !self.update_checked {
            self.update_checked = true;
            let cur = env!("CARGO_PKG_VERSION").to_string();
            let update_tx = self.update_tx.clone();
            std::thread::spawn(move || {
                let new = jackett::check_update(&cur);
                let _ = update_tx.send(new);
            });
        }
        if let Ok(Some(new)) = self.update_rx.try_recv() {
            self.toast(&format!("Update available: {new}"), self.pal.accent);
        }

        // Spinner tick
        if state == SearchState::Searching {
            ctx.request_repaint_after(Duration::from_millis(80));
            let dt = ctx.input(|i| i.unstable_dt).clamp(0.0, 0.1);
            self.spin_t += dt;
            if self.spin_t > 0.1 { self.spin_t = 0.0; self.spin_i = (self.spin_i + 1) % SPIN.len(); }
        }
        if matches!(state, SearchState::Done | SearchState::Error(_)) {
            if let Some(t) = self.t_start.take() { self.t_done = Some(t.elapsed().as_secs_f64()); }
        }

        // Toast decay
        let dt = ctx.input(|i| i.unstable_dt).clamp(0.0, 0.1);
        self.toasts.retain_mut(|t| { t.ttl -= dt; t.ttl > 0.0 });

        // Global shortcuts
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::F)) {
            ctx.memory_mut(|m| m.request_focus(egui::Id::new("q")));
        }
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::R)) { self.do_search(); }
        // Ctrl+A — select all visible results (batch mode auto-enables)
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::A)) {
            let raw = self.all_results();
            let sorted = self.filtered(&raw);
            if !sorted.is_empty() {
                self.sel_mode = true;
                let page_s = self.page_slice(&sorted);
                let base = self.page * self.cfg.page_size;
                self.sel_set = (0..page_s.len()).map(|i| base + i).collect();
                self.toast(&format!("Selected {} results", page_s.len()), self.pal.green);
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.detail_open { self.detail_open = false; } else { self.query.clear(); self.show_hist = false; }
        }
        // Copy magnet from detail panel with Ctrl+C
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::C) && self.detail_open) {
            if let Some(idx) = self.selected {
                let raw = self.all_results();
                let sorted = self.filtered(&raw);
                let page_s = self.page_slice(&sorted);
                if let Some(r) = page_s.get(idx) {
                    if let Some(m) = &r.magnet_uri {
                        ui.ctx().copy_text(m.clone());
                        self.toast("Magnet copied ✓", self.pal.green);
                    }
                }
            }
        }

        // ── Status bar ───────────────────────────────────────────────────
        egui::Panel::bottom("sb")
            .default_size(26.0)
            .frame(egui::Frame::NONE
                .fill(self.pal.hdr).stroke(Stroke::new(1.0_f32, self.pal.border))
                .inner_margin(egui::Margin::symmetric(12, 4)))
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    match &state {
                        SearchState::Idle => {
                            lbl(ui, "Ready — type a query and press Search", self.pal.dim, 12.0);
                        }
                        SearchState::Searching => {
                            let sp = SPIN[self.spin_i];
                            let el = self.t_start.as_ref()
                                .map(|t| format!("  {:.1}s", t.elapsed().as_secs_f64()))
                                .unwrap_or_default();
                            lbl(ui, &format!("{sp} Searching \"{}\"{}", self.last_query, el), self.pal.accent, 12.0);
                        }
                        SearchState::Done => {
                            let n = self.total_count();
                            let e = self.t_done.map(|e| format!("  ({:.1}s)", e)).unwrap_or_default();
                            lbl(ui, &format!("✓ {n} results for \"{}\"{}", self.last_query, e), self.pal.green, 12.0);
                        }
                        SearchState::Error(e) => {
                            lbl(ui, &format!("✕ {}", e.lines().next().unwrap_or(e)), self.pal.red, 12.0);
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        lbl(ui, "↑↓ Enter  D  F  M  Esc  Ctrl+F  Ctrl+R",
                            self.pal.dim, 10.5);
                    });
                });
            });

        // ── Header ───────────────────────────────────────────────────────
        self.draw_header(ui);

        // ── RSS polling ───────────────────────────────────────────────
        self.poll_rss();
        self.auto_refresh_feeds();

        // ── Settings panel ───────────────────────────────────────────────
        if self.show_settings {
            egui::Panel::top("settings")
                .frame(egui::Frame::NONE
                    .fill(self.pal.hdr).stroke(Stroke::new(1.0_f32, self.pal.border))
                    .inner_margin(egui::Margin::symmetric(14, 8)))
                .show(ui, |ui| {
                    self.draw_settings_panel(ui);
                });
        }

        // ── Central panel ────────────────────────────────────────────────
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(self.pal.bg))
            .show(ui, |ui| {
                match self.tab.clone() {
                    Tab::Search => self.draw_search(ui, &ctx, &state),
                    Tab::Favorites => self.draw_favorites(ui),
                    Tab::Rss => self.draw_rss(ui, &ctx),
                    Tab::About => self.draw_about(ui),
                }
            });

        // Detail panel (top-level Panel::right, resizable)
        self.draw_detail_panel(ui);

        self.draw_toasts(&ctx);
    }
}

// ─── (UI draw methods moved to ui.rs) ──────────────────────────────
// ─── Helper for detail grid ─────────────────────────────────────────────────

pub(crate) fn grid_row(ui: &mut egui::Ui, label: &str, value: &str, color: Color32, p: &Pal, fs: f32) {
    ui.label(RichText::new(format!("{label}:")).font(FontId::proportional(fs - 1.5)).color(p.dim));
    ui.label(RichText::new(value).font(FontId::proportional(fs - 1.0)).color(color));
    ui.end_row();
}

// ─── Entry point ────────────────────────────────────────────────────────────

fn native_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("TorrentX")
            .with_inner_size([1300.0, 800.0])
            .with_min_inner_size([900.0, 560.0]),
        ..Default::default()
    }
}

type AppCreator = Box<dyn FnOnce(&eframe::CreationContext) -> Result<Box<dyn eframe::App>, Box<dyn std::error::Error + Send + Sync>>>;

fn app_creator() -> AppCreator {
    Box::new(|_cc| Ok(Box::new(App::default()) as Box<dyn eframe::App>))
}

/// Shared quit flag: set when the tray "Quit" is clicked, so the app exits
/// even from the tray (which runs outside the egui event loop).
static QUIT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Set when the tray "Show / Hide" is clicked; the app toggles window visibility.
static TOGGLE_VIS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Create the system tray icon + menu (Show/Hide, Quit) on a dedicated GTK
/// thread (tray-icon requires a GTK event loop on Linux). Returns immediately;
/// the tray lives for the app's lifetime. Failure is non-fatal.
fn setup_tray() {
    #[cfg(target_os = "linux")]
    {
        std::thread::spawn(|| {
            use tray_icon::{menu::{Menu, MenuItem, MenuEvent}, Icon, TrayIconBuilder};
            if gtk::init().is_err() { return; }

            // A tiny 16x16 TorrentX-ish icon (blue "T" on dark).
            let mut rgba = Vec::with_capacity(16 * 16 * 4);
            for y in 0..16 {
                for x in 0..16 {
                    let in_t = (4..=11).contains(&x) && (3..=12).contains(&y) && !((6..=9).contains(&x) && (6..=9).contains(&y));
                    if in_t { rgba.extend_from_slice(&[122, 162, 247, 255]); }
                    else { rgba.extend_from_slice(&[26, 27, 38, 255]); }
                }
            }
            let Ok(icon) = Icon::from_rgba(rgba, 16, 16) else { return };

            let show = MenuItem::new("Show / Hide", true, None);
            let quit = MenuItem::new("Quit", true, None);
            let menu = Menu::new();
            if menu.append_items(&[&show, &quit]).is_err() { return; }

            if TrayIconBuilder::new()
                .with_menu(Box::new(menu))
                .with_tooltip("TorrentX")
                .with_icon(icon)
                .build().is_err() { return; }

            // Menu events arrive on this thread's channel; signal the app.
            let show_id = show.id().clone();
            let quit_id = quit.id().clone();
            std::thread::spawn(move || {
                while let Ok(ev) = MenuEvent::receiver().recv() {
                    if ev.id == quit_id {
                        QUIT.store(true, std::sync::atomic::Ordering::SeqCst);
                        break;
                    }
                    if ev.id == show_id {
                        TOGGLE_VIS.store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                }
            });

            // Run the GTK event loop (blocks; keeps the tray alive).
            gtk::main();
        });
    }
}

fn main() -> eframe::Result<()> {
    // Parse CLI flags before any GUI/config work.
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "TorrentX {} — native Jackett torrent-search GUI\n\n\
             USAGE:\n  torrentx [OPTIONS]\n\n\
             OPTIONS:\n  \
             --config <path>   Use an alternate config file (default: ~/.config/torrentx/config.json)\n  \
             -h, --help        Print this help and exit\n  \
             -V, --version     Print the version and exit\n",
            env!("CARGO_PKG_VERSION")
        );
        return Ok(());
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("TorrentX {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    // Optional --config <path> override (before any config is loaded).
    if let Some(i) = args.iter().position(|a| a == "--config") {
        if let Some(p) = args.get(i + 1) {
            config::set_config_override(std::path::PathBuf::from(p));
        }
    }

    // System tray on a dedicated GTK thread (optional; non-fatal if unavailable).
    setup_tray();

    // Try the normal (GPU-accelerated GL) run first.
    match eframe::run_native("TorrentX", native_options(), app_creator()) {
        Ok(()) => Ok(()),
        Err(e) => {
            eprintln!("GPU/GL init failed ({e}); retrying with software rendering…");
            // Retry with Mesa's software GL — fixes black windows on machines
            // where the GPU driver/context can't initialize (VMs, NVIDIA+Wayland quirks).
            std::env::set_var("LIBGL_ALWAYS_SOFTWARE", "1");
            std::env::set_var("GALLIUM_DRIVER", "llvmpipe");
            eframe::run_native("TorrentX", native_options(), app_creator())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::csv_esc;

    #[test]
    fn csv_esc_quotes_and_doubles_embedded_quotes() {
        assert_eq!(csv_esc("plain"), "\"plain\"");
        assert_eq!(csv_esc("has \"quotes\""), "\"has \"\"quotes\"\"\"");
        assert_eq!(csv_esc(""), "\"\"");
        // Commas are harmless inside quotes.
        assert_eq!(csv_esc("a,b,c"), "\"a,b,c\"");
    }
}