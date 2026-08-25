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
use std::time::{Duration, Instant};

// ─── Constants ─────────────────────────────────────────────────────────────

pub(crate) const MARGIN_DEFAULT: f32 = 12.0;

pub(crate) const CATS: &[&str] = &[
    "All", "Movies", "TV", "Music", "PC Games", "Software", "Anime", "Books", "XXX",
];

/// CSV field escaping: wrap in quotes, double any embedded quotes.
pub(crate) fn csv_esc(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// Open a URL/URI only if its scheme is safe. Indexer-supplied strings are
/// untrusted — never hand file:// or arbitrary schemes to xdg-open.
pub(crate) fn safe_open(s: impl AsRef<str>) -> bool {
    let t = s.as_ref().trim();
    let ok = ["http://", "https://", "magnet:"]
        .iter()
        .any(|p| t.starts_with(p));
    if ok {
        open::that(t).is_ok()
    } else {
        false
    }
}

/// CSV field escaping + formula-injection guard: spreadsheet apps execute
/// cells that begin with = + - @ (tab/CR too). Indexer-supplied titles are
/// untrusted, so prefix such payloads with `'`. The payload sits at index 1
/// (index 0 is the opening quote added by `csv_esc`).
pub(crate) fn csv_safe(s: &str) -> String {
    let mut s = csv_esc(s);
    if matches!(
        s.as_bytes().get(1),
        Some(b'=' | b'+' | b'-' | b'@' | b'\t' | b'\r')
    ) {
        s.insert(1, '\'');
    }
    s
}

// ─── Small UI buttons ──────────────────────────────────────────────────────

pub(crate) fn act_btn(ui: &mut egui::Ui, label: &str, tip: &str, color: Color32) -> bool {
    ui.add(
        egui::Button::new(RichText::new(label).size(11.5).color(color))
            .fill(tint(color, 18))
            .stroke(Stroke::new(1.0_f32, tint(color, 70)))
            .corner_radius(5.0)
            .min_size(Vec2::new(0.0, 25.0)),
    )
    .on_hover_text(tip)
    .clicked()
}

/// A high-quality SVG icon button — renders an embedded Lucide SVG via
/// egui's image loader (resvg), tinted to the theme color. Professional
/// icon design, vector-crisp, no glyphs, no tofu. egui caches the
/// rasterized image by URI. Returns true when clicked.
pub(crate) fn svg_btn(ui: &mut egui::Ui, icon: SvgIcon, tip: &str, color: Color32) -> bool {
    let size = egui::vec2(32.0, 28.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let rounding = egui::CornerRadius::same(5);
    let painter = ui.painter_at(rect);

    // Press feedback: shrink the hit surface slightly while held (tactile
    // confirmation the click registered — interface-design motion rule).
    let press_scale = if resp.is_pointer_button_down_on() {
        0.94
    } else {
        1.0
    };
    let rect = if press_scale < 1.0 {
        egui::Rect::from_center_size(rect.center(), rect.size() * press_scale)
    } else {
        rect
    };

    if resp.hovered() || resp.clicked() {
        painter.rect_filled(rect, rounding, tint(color, 26));
    } else {
        painter.rect_filled(rect, rounding, tint(color, 14));
    }
    painter.rect_stroke(
        rect.shrink(0.5),
        rounding,
        Stroke::new(1.0, tint(color, 70)),
        egui::StrokeKind::Outside,
    );

    // Draw the SVG tinted to the theme color (icon area centered in button).
    // Lucide uses stroke="currentColor"; resvg renders that black, and egui's
    // tint multiplies (black × color = black → invisible on dark themes). So
    // rewrite currentColor→white so tint() yields the actual theme color.
    let svg_white = icon.svg().replace("currentColor", "#fff");
    let icon_size = 16.0;
    let icon_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(icon_size, icon_size));
    let img = egui::Image::from_bytes(icon.uri(), svg_white.into_bytes())
        .tint(color)
        .fit_to_exact_size(egui::vec2(icon_size, icon_size));
    ui.put(icon_rect, img);

    resp.on_hover_text(tip).clicked()
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum SvgIcon {
    Magnet,
    Copy,
    Download,
    Star,
    Info,
    Web,
    Search,
    Rss,
    Settings,
    Close,
    Refresh,
    Bookmark,
    ArrowUp,
    ArrowDown,
    Circle,
    CircleDot,
    ChevronLeft,
    ChevronRight,
    Check,
}

impl SvgIcon {
    fn uri(&self) -> &'static str {
        match self {
            SvgIcon::Magnet => "bytes://tx/magnet.svg",
            SvgIcon::Copy => "bytes://tx/copy.svg",
            SvgIcon::Download => "bytes://tx/download.svg",
            SvgIcon::Star => "bytes://tx/star.svg",
            SvgIcon::Info => "bytes://tx/info.svg",
            SvgIcon::Web => "bytes://tx/web.svg",
            SvgIcon::Search => "bytes://tx/search.svg",
            SvgIcon::Rss => "bytes://tx/rss.svg",
            SvgIcon::Settings => "bytes://tx/settings.svg",
            SvgIcon::Close => "bytes://tx/x.svg",
            SvgIcon::Refresh => "bytes://tx/refresh-cw.svg",
            SvgIcon::Bookmark => "bytes://tx/bookmark.svg",
            SvgIcon::ArrowUp => "bytes://tx/arrow-up.svg",
            SvgIcon::ArrowDown => "bytes://tx/arrow-down.svg",
            SvgIcon::Circle => "bytes://tx/circle.svg",
            SvgIcon::CircleDot => "bytes://tx/circle-dot.svg",
            SvgIcon::ChevronLeft => "bytes://tx/chevron-left.svg",
            SvgIcon::ChevronRight => "bytes://tx/chevron-right.svg",
            SvgIcon::Check => "bytes://tx/check.svg",
        }
    }
    fn svg(&self) -> &'static str {
        match self {
            SvgIcon::Magnet => include_str!("../assets/icons/magnet.svg"),
            SvgIcon::Copy => include_str!("../assets/icons/copy.svg"),
            SvgIcon::Download => include_str!("../assets/icons/download.svg"),
            SvgIcon::Star => include_str!("../assets/icons/star.svg"),
            SvgIcon::Info => include_str!("../assets/icons/info.svg"),
            SvgIcon::Web => include_str!("../assets/icons/web.svg"),
            SvgIcon::Search => include_str!("../assets/icons/search.svg"),
            SvgIcon::Rss => include_str!("../assets/icons/rss.svg"),
            SvgIcon::Settings => include_str!("../assets/icons/settings.svg"),
            SvgIcon::Close => include_str!("../assets/icons/x.svg"),
            SvgIcon::Refresh => include_str!("../assets/icons/refresh-cw.svg"),
            SvgIcon::Bookmark => include_str!("../assets/icons/bookmark.svg"),
            SvgIcon::ArrowUp => include_str!("../assets/icons/arrow-up.svg"),
            SvgIcon::ArrowDown => include_str!("../assets/icons/arrow-down.svg"),
            SvgIcon::Circle => include_str!("../assets/icons/circle.svg"),
            SvgIcon::CircleDot => include_str!("../assets/icons/circle-dot.svg"),
            SvgIcon::ChevronLeft => include_str!("../assets/icons/chevron-left.svg"),
            SvgIcon::ChevronRight => include_str!("../assets/icons/chevron-right.svg"),
            SvgIcon::Check => include_str!("../assets/icons/check.svg"),
        }
    }
}

/// Inline SVG icon (not a button) — renders the tinted icon at `size` px
/// into the current Ui. For headers, labels, status dots, etc.
pub(crate) fn svg_icon(ui: &mut egui::Ui, icon: SvgIcon, size: f32, color: Color32) {
    ui.add(svg_image(icon, size, color));
}

/// Returns an egui::Image widget for an SVG icon (tinted, sized). Usable
/// inside `Button::new(...)` or `ui.add(...)`.
pub(crate) fn svg_image(icon: SvgIcon, size: f32, color: Color32) -> egui::Image<'static> {
    let svg_white = icon.svg().replace("currentColor", "#fff");
    egui::Image::from_bytes(icon.uri(), svg_white.into_bytes())
        .tint(color)
        .fit_to_exact_size(egui::vec2(size, size))
}

/// A high-quality vector checkbox — drawn with the Painter (rounded rect +
/// check mark), NO font glyphs. Single allocation, whole-rect clickable.
/// Uses the accent color when checked. Returns the Response.
pub(crate) fn v_checkbox(
    ui: &mut egui::Ui,
    checked: bool,
    label: &str,
    accent: Color32,
) -> egui::Response {
    let icon_size = 15.0;
    let spacing = ui.spacing().item_spacing.x;

    // Measure label
    let font_id = egui::TextStyle::Body.resolve(ui.style());
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font_id, ui.visuals().text_color());

    // Single allocation for the whole row (icon + spacing + text)
    let desired = egui::vec2(
        icon_size + spacing + galley.size().x,
        icon_size.max(galley.size().y) + 2.0,
    );
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    if ui.is_rect_visible(rect) {
        let p = ui.painter_at(rect);
        // Box
        let box_rect = egui::Rect::from_min_size(
            egui::pos2(rect.left(), rect.center().y - icon_size / 2.0),
            egui::vec2(icon_size, icon_size),
        );
        let rounding = egui::CornerRadius::same(4);
        if checked {
            p.rect_filled(box_rect, rounding, accent);
            // Check mark (two lines); contrast against the accent fill
            let check_col = if ui.visuals().dark_mode {
                Color32::from_rgb(12, 12, 16)
            } else {
                Color32::WHITE
            };
            let ck = egui::Stroke::new(2.2, check_col);
            let cc = box_rect.center();
            p.line_segment(
                [
                    egui::pos2(cc.x - 3.5, cc.y),
                    egui::pos2(cc.x - 0.8, cc.y + 3.0),
                ],
                ck,
            );
            p.line_segment(
                [
                    egui::pos2(cc.x - 0.8, cc.y + 3.0),
                    egui::pos2(cc.x + 4.0, cc.y - 3.5),
                ],
                ck,
            );
        } else {
            p.rect_filled(box_rect, rounding, Color32::TRANSPARENT);
            p.rect_stroke(
                box_rect,
                rounding,
                egui::Stroke::new(1.3, ui.visuals().text_color().gamma_multiply(0.6)),
                egui::StrokeKind::Inside,
            );
        }
        // Label
        let text_pos = egui::pos2(
            rect.left() + icon_size + spacing,
            rect.center().y - galley.size().y / 2.0,
        );
        p.galley(text_pos, galley, ui.visuals().text_color());
    }
    response
}

/// Compact icon + text pill. The complete surface is one native button,
/// so icon and label share the same hover/focus/click target.
pub(crate) fn icon_text_btn(
    ui: &mut egui::Ui,
    icon: SvgIcon,
    label: &str,
    color: Color32,
    enabled: bool,
) -> bool {
    ui.add_enabled(
        enabled,
        egui::Button::image_and_text(
            svg_image(icon, 12.0, color),
            RichText::new(label)
                .font(FontId::proportional(12.0))
                .color(color),
        )
        .fill(tint(color, 14))
        .stroke(Stroke::new(1.0, tint(color, 70)))
        .corner_radius(5.0)
        .min_size(Vec2::new(0.0, 28.0)),
    )
    .clicked()
}

/// Non-interactive status pill with a tofu-proof SVG icon.
pub(crate) fn status_icon_pill(ui: &mut egui::Ui, icon: SvgIcon, label: &str, col: Color32) {
    egui::Frame::NONE
        .fill(tint(col, 20))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                svg_icon(ui, icon, 9.0, col);
                ui.label(
                    RichText::new(label)
                        .font(FontId::proportional(10.5))
                        .color(col),
                );
            });
        });
}

pub(crate) fn wide_btn(ui: &mut egui::Ui, label: &str, color: Color32) -> bool {
    let w = ui.available_width().max(200.0);
    ui.add(
        egui::Button::new(
            RichText::new(label)
                .font(FontId::proportional(13.0))
                .color(color),
        )
        .fill(tint(color, 18))
        .stroke(Stroke::new(1.0_f32, tint(color, 80)))
        .corner_radius(6.0)
        .min_size(Vec2::new(w, 34.0)),
    )
    .clicked()
}

pub(crate) fn outline_btn(ui: &mut egui::Ui, label: &str, color: Color32) -> bool {
    ui.add(
        egui::Button::new(
            RichText::new(label)
                .font(FontId::proportional(12.0))
                .color(color),
        )
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::new(1.0_f32, tint(color, 80)))
        .corner_radius(4.0),
    )
    .clicked()
}

/// Wide button with an SVG icon + text label (professional look, no glyphs).
/// Built with allocate + painter + horizontal layout so icon and text never
/// overlap and the whole button is clickable.
pub(crate) fn wide_icon_btn(ui: &mut egui::Ui, icon: SvgIcon, label: &str, color: Color32) -> bool {
    let w = ui.available_width().max(200.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 34.0), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        let fill = if resp.hovered() {
            tint(color, 26)
        } else {
            tint(color, 18)
        };
        painter.rect_filled(rect, egui::CornerRadius::same(6), fill);
        painter.rect_stroke(
            rect.shrink(0.5),
            egui::CornerRadius::same(6),
            Stroke::new(1.0, tint(color, 80)),
            egui::StrokeKind::Outside,
        );
    }
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    child.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        svg_icon(ui, icon, 15.0, color);
        ui.label(
            RichText::new(label)
                .font(FontId::proportional(13.0))
                .color(color),
        );
    });
    resp.clicked()
}

/// Outline button with an SVG icon + text label (professional, no glyphs).
pub(crate) fn outline_icon_btn(
    ui: &mut egui::Ui,
    icon: SvgIcon,
    label: &str,
    color: Color32,
) -> bool {
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width().min(160.0), 26.0),
        egui::Sense::click(),
    );
    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        painter.rect_stroke(
            rect.shrink(0.5),
            egui::CornerRadius::same(4),
            Stroke::new(1.0, tint(color, 80)),
            egui::StrokeKind::Outside,
        );
    }
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    child.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        svg_icon(ui, icon, 13.0, color);
        ui.label(
            RichText::new(label)
                .font(FontId::proportional(12.0))
                .color(color),
        );
    });
    resp.clicked()
}

pub(crate) fn lbl(ui: &mut egui::Ui, text: &str, color: Color32, fs: f32) {
    ui.label(
        RichText::new(text)
            .font(FontId::proportional(fs))
            .color(color),
    );
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
        ui.add(
            egui::TextEdit::singleline(value)
                .desired_width(width)
                .hint_text(hint)
                .font(FontId::proportional(fs)),
        );
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

        // Tray "Quit" → exit the whole app.
        if QUIT.load(std::sync::atomic::Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        // Tray "Show / Hide" → toggle window visibility.
        if TOGGLE_VIS.swap(false, std::sync::atomic::Ordering::SeqCst) {
            let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(!focused));
            if focused {
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
        if self.net.indexers.is_empty()
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
                let list = jackett::fetch_indexers(&url, &key);
                let _ = handle.send(list);
            });
        }
        // Drain indexer fetch result
        if self.net.indexers_loading {
            if let Ok(list) = self.net.indexers_rx.try_recv() {
                self.net.jackett_ok = Some(list.is_some()); // None = Jackett unreachable
                match list {
                    Some(l) => {
                        self.net.indexers = l;
                        self.net.indexers_retry_at = None;
                    }
                    None => {
                        // Unreachable — back off before retrying.
                        self.net.indexers_retry_at = Some(Instant::now() + Duration::from_secs(60));
                    }
                }
                self.net.indexers_loading = false;
            }
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
        }

        // Toast decay + animation progress
        let dt = ctx.input(|i| i.unstable_dt).clamp(0.0, 0.1);
        self.ui.toasts.retain_mut(|t| {
            t.ttl -= dt;
            // Animate in (0.15s) and out (handled by fade in draw_toasts)
            t.anim_progress = (t.anim_progress + dt / 0.15).min(1.0);
            t.ttl > 0.0
        });

        // Row hover animation (smooth transition between rows)
        if self.ui.hovered != self.ui.prev_hovered {
            self.ui.prev_hovered = self.ui.hovered;
            self.ui.row_hover_anim = 0.0;
        } else if self.ui.hovered.is_some() {
            self.ui.row_hover_anim = (self.ui.row_hover_anim + dt / 0.1).min(1.0);
        } else {
            self.ui.row_hover_anim = (self.ui.row_hover_anim - dt / 0.1).max(0.0);
        }

        // Search state transition animation
        if self.ui.prev_search_state != state {
            self.ui.prev_search_state = state.clone();
            self.ui.search_state_anim = 0.0;
        } else {
            self.ui.search_state_anim = (self.ui.search_state_anim + dt / 0.2).min(1.0);
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
            1.0 / 0.2
        } else {
            1.0 / 0.15
        };
        let delta = speed * dt;
        self.ui.detail_anim = if self.ui.detail_anim < target {
            (self.ui.detail_anim + delta).min(target)
        } else {
            (self.ui.detail_anim - delta).max(target)
        };
        self.ui.prev_detail_open = self.ui.detail_open;

        // Filter+sort the results once per frame; every consumer below
        // (batch handlers, search tab, detail panel) shares this view.
        let view = if state == SearchState::Done {
            Some(self.compute_view())
        } else {
            None
        };

        // Global shortcuts
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::F)) {
            ctx.memory_mut(|m| m.request_focus(egui::Id::new("q")));
        }
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::R)) {
            self.do_search();
        }
        // Ctrl+A — select all visible results (batch mode auto-enables).
        // Skipped while a TextEdit owns the keyboard, so select-all still
        // works inside the query/filters/settings inputs.
        let editing = ctx.memory(|m| m.focused().is_some());
        if !editing && ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::A)) {
            if let Some(v) = &view {
                if !v.sorted.is_empty() {
                    self.ui.sel_mode = true;
                    let page_s = self.page_slice(&v.sorted);
                    let base = self.search.page * self.cfg.page_size;
                    self.ui.sel_set = (0..page_s.len()).map(|i| base + i).collect();
                    self.toast(
                        &format!("Selected {} results", page_s.len()),
                        self.pal.green,
                    );
                }
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.ui.detail_open {
                self.ui.detail_open = false;
            } else {
                self.search.query.clear();
                self.ui.show_hist = false;
            }
        }
        // Copy magnet from detail panel with Ctrl+C — only when no TextEdit
        // is focused (don't clobber the clipboard while copying typed text).
        if self.ui.detail_open
            && !editing
            && ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::C))
        {
            if let (Some(idx), Some(v)) = (self.ui.selected, &view) {
                let page_s = self.page_slice(&v.sorted);
                if let Some(r) = page_s.get(idx) {
                    if let Some(m) = &r.magnet_uri {
                        ui.ctx().copy_text(m.clone());
                        self.toast("Magnet copied", self.pal.green);
                    }
                }
            }
        }

        // ── Status bar ───────────────────────────────────────────────────
        egui::Panel::bottom("sb")
            .default_size(26.0)
            .frame(
                egui::Frame::NONE
                    .fill(self.pal.hdr)
                    .stroke(Stroke::new(1.0_f32, self.pal.border))
                    .inner_margin(egui::Margin::symmetric(12, 4)),
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
                            svg_icon(ui, SvgIcon::Refresh, 12.0, self.pal.accent);
                            lbl(
                                ui,
                                &format!("Searching \"{}\"{}", self.search.last_query, el),
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
                            svg_icon(ui, SvgIcon::Check, 12.0, self.pal.green);
                            lbl(
                                ui,
                                &format!("{n} results for \"{}\"{}", self.search.last_query, e),
                                self.pal.green,
                                12.0,
                            );
                        }
                        SearchState::Error(e) => {
                            svg_icon(ui, SvgIcon::Close, 12.0, self.pal.red);
                            lbl(ui, e.lines().next().unwrap_or(e), self.pal.red, 12.0);
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        lbl(
                            ui,
                            "Keys: Arrows · Enter · D · F · M · Esc · Ctrl+F · Ctrl+R",
                            self.pal.dim,
                            10.5,
                        );
                    });
                });
            });

        // ── Header ───────────────────────────────────────────────────────
        self.draw_header(ui);

        // ── RSS polling ───────────────────────────────────────────────
        self.poll_rss();
        self.auto_refresh_feeds();

        // ── Settings panel ───────────────────────────────────────────────
        if self.ui.show_settings {
            egui::Panel::top("settings")
                .frame(
                    egui::Frame::NONE
                        .fill(self.pal.hdr)
                        .stroke(Stroke::new(1.0_f32, self.pal.border))
                        .inner_margin(egui::Margin::symmetric(14, 8)),
                )
                .show(ui, |ui| {
                    self.draw_settings_panel(ui);
                });
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

// ─── (UI draw methods moved to ui.rs) ──────────────────────────────
// ─── Helper for detail grid ─────────────────────────────────────────────────

pub(crate) fn grid_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &str,
    color: Color32,
    p: &Pal,
    fs: f32,
) {
    ui.label(
        RichText::new(format!("{label}:"))
            .font(FontId::proportional(fs - 1.5))
            .color(p.dim),
    );
    ui.label(
        RichText::new(value)
            .font(FontId::proportional(fs - 1.0))
            .color(color),
    );
    ui.end_row();
}

// ─── Entry point ────────────────────────────────────────────────────────────

fn native_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("TorrentX")
            .with_inner_size([1300.0, 800.0])
            // Min width tuned so the header right-side controls + search bar
            // still fit without clipping on small screens (1280x720 laptops).
            .with_min_inner_size([820.0, 560.0]),
        ..Default::default()
    }
}

type AppCreator = Box<
    dyn FnOnce(
        &eframe::CreationContext,
    ) -> Result<Box<dyn eframe::App>, Box<dyn std::error::Error + Send + Sync>>,
>;

fn app_creator() -> AppCreator {
    Box::new(|cc| {
        // SVG icon support (egui_extras svg feature → resvg). Required for
        // the Lucide SVG action buttons.
        egui_extras::install_image_loaders(&cc.egui_ctx);
        Ok(Box::new(App::default()) as Box<dyn eframe::App>)
    })
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
            use tray_icon::{
                menu::{Menu, MenuEvent, MenuItem},
                Icon, TrayIconBuilder,
            };
            if gtk::init().is_err() {
                return;
            }

            // A tiny 16x16 TorrentX-ish icon (blue "T" on dark).
            let mut rgba = Vec::with_capacity(16 * 16 * 4);
            for y in 0..16 {
                for x in 0..16 {
                    let in_t = (4..=11).contains(&x)
                        && (3..=12).contains(&y)
                        && !((6..=9).contains(&x) && (6..=9).contains(&y));
                    if in_t {
                        rgba.extend_from_slice(&[122, 162, 247, 255]);
                    } else {
                        rgba.extend_from_slice(&[26, 27, 38, 255]);
                    }
                }
            }
            let Ok(icon) = Icon::from_rgba(rgba, 16, 16) else {
                return;
            };

            let show = MenuItem::new("Show / Hide", true, None);
            let quit = MenuItem::new("Quit", true, None);
            let menu = Menu::new();
            if menu.append_items(&[&show, &quit]).is_err() {
                return;
            }

            if TrayIconBuilder::new()
                .with_menu(Box::new(menu))
                .with_tooltip("TorrentX")
                .with_icon(icon)
                .build()
                .is_err()
            {
                return;
            }

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

    // Try the normal (GPU-accelerated) run first.
    match eframe::run_native("TorrentX", native_options(), app_creator()) {
        Ok(()) => Ok(()),
        Err(e) => {
            eprintln!("GPU init failed ({e}); retrying with software rendering…");
            // Retry with a software-friendly renderer. These env vars are read
            // by egui-wgpu/Mesa at adapter-selection time — i.e. INSIDE the
            // run_native call below — so setting them here is effective:
            // WGPU_BACKEND=opengl + LIBGL_ALWAYS_SOFTWARE steer wgpu onto Mesa's
            // software GL (llvmpipe), fixing black windows on machines where
            // hardware Vulkan/GL can't initialize (VMs, NVIDIA+Wayland quirks).
            std::env::set_var("WGPU_BACKEND", "opengl");
            std::env::set_var("WGPU_POWER_PREF", "low");
            std::env::set_var("LIBGL_ALWAYS_SOFTWARE", "1");
            std::env::set_var("GALLIUM_DRIVER", "llvmpipe");
            eframe::run_native("TorrentX", native_options(), app_creator())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{csv_esc, csv_safe};

    #[test]
    fn csv_esc_quotes_and_doubles_embedded_quotes() {
        assert_eq!(csv_esc("plain"), "\"plain\"");
        assert_eq!(csv_esc("has \"quotes\""), "\"has \"\"quotes\"\"\"");
        assert_eq!(csv_esc(""), "\"\"");
        // Commas are harmless inside quotes.
        assert_eq!(csv_esc("a,b,c"), "\"a,b,c\"");
    }

    #[test]
    fn csv_safe_neutralizes_formula_injection() {
        // Benign values pass through unchanged.
        assert_eq!(csv_safe("Ubuntu 24.04"), "\"Ubuntu 24.04\"");
        assert_eq!(csv_safe(""), "\"\"");
        // Formula-leading characters get a `'` prefix (after the quote).
        assert_eq!(csv_safe("=cmd|' /C calc'!A0"), "\"'=cmd|' /C calc'!A0\"");
        assert_eq!(csv_safe("+1+1"), "\"'+1+1\"");
        assert_eq!(csv_safe("-2"), "\"'-2\"");
        assert_eq!(csv_safe("@SUM(1)"), "\"'@SUM(1)\"");
        assert_eq!(csv_safe("\tTab"), "\"'\tTab\"");
    }
}
