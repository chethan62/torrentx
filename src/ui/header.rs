//! header drawing methods.

use super::*;

use crate::app::App;

impl App {
    pub(crate) fn draw_header(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("hdr")
            .default_size(52.0)
            .frame(
                egui::Frame::NONE
                    .fill(self.pal.surface)
                    .stroke(Stroke::new(1.0_f32, self.pal.border)),
            )
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(MARGIN_DEFAULT + 2.0);
                    // Logo + tabs wrap onto a second line at narrow widths so
                    // the right-side controls never get clipped (overflow fix).
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        // Logo
                        ui.label(
                            RichText::new("Torrent")
                                .font(FontId::monospace(16.0))
                                .strong()
                                .color(self.pal.text),
                        );
                        ui.label(
                            RichText::new("X")
                                .font(FontId::monospace(16.0))
                                .strong()
                                .color(self.pal.accent),
                        );
                        egui::Frame::NONE
                            .fill(tint(self.pal.accent, 28))
                            .corner_radius(10.0)
                            .inner_margin(egui::Margin::symmetric(5, 1))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(env!("CARGO_PKG_VERSION"))
                                        .size(10.0)
                                        .color(self.pal.accent),
                                );
                            });
                        ui.add_space(14.0);
                        ui.separator();
                        ui.add_space(8.0);

                        // Tabs (SVG icon + label)
                        for (icon, label, tip, tab) in [
                            (SvgIcon::Search, "Search", "Search torrents", Tab::Search),
                            (
                                SvgIcon::Bookmark,
                                "Favorites",
                                "Saved torrents",
                                Tab::Favorites,
                            ),
                            (SvgIcon::Rss, "RSS", "RSS feed reader", Tab::Rss),
                            (SvgIcon::Info, "About", "About TorrentX", Tab::About),
                        ] {
                            let active = self.ui.tab == tab;
                            let badge = if tab == Tab::Favorites && !self.cfg.favorites.is_empty() {
                                format!(" {}", self.cfg.favorites.len())
                            } else if tab == Tab::Search {
                                let n = self.search.count.lock().map(|c| *c).unwrap_or(0);
                                if n > 0 {
                                    format!(" {n}")
                                } else {
                                    String::new()
                                }
                            } else {
                                String::new()
                            };
                            let col = if active {
                                self.pal.accent
                            } else {
                                self.pal.sub
                            };
                            let clicked = ui
                                .horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 5.0;
                                    svg_icon(ui, icon, 15.0, col);
                                    ui.add(
                                        egui::Button::new(
                                            RichText::new(format!("{label}{badge}"))
                                                .font(FontId::proportional(14.0))
                                                .color(col),
                                        )
                                        .fill(if active {
                                            tint(self.pal.accent, 22)
                                        } else {
                                            Color32::TRANSPARENT
                                        })
                                        .stroke(Stroke::new(
                                            if active { 1.0_f32 } else { 0.0_f32 },
                                            self.pal.accent,
                                        ))
                                        .corner_radius(6.0)
                                        .min_size(Vec2::new(0.0, 30.0)),
                                    )
                                    .on_hover_text(tip)
                                    .clicked()
                                })
                                .inner;
                            if clicked && self.ui.tab != tab {
                                if tab != Tab::Favorites {
                                    self.ui.fav_search.clear();
                                }
                                self.ui.tab = tab;
                                self.ui.detail_open = false;
                                self.ui.selected = None;
                                // Clear cross-tab hover/selection state so a
                                // stale row highlight never bleeds into the
                                // newly shown tab's table.
                                self.ui.hovered = None;
                                self.rss.rss_detail = None;
                            }
                            ui.add_space(2.0);
                        }
                    }); // end horizontal_wrapped (logo + tabs wrap at narrow widths)

                    // Right side controls
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(12.0);

                        // Jackett connection status dot
                        if let Some(ok) = self.net.jackett_ok {
                            let (col, tip) = if ok {
                                (self.pal.green, "Jackett connected")
                            } else {
                                (self.pal.red, "Jackett unreachable — check Settings")
                            };
                            ui.add(svg_image(SvgIcon::Circle, 12.0, col))
                                .on_hover_text(tip);
                            ui.add_space(6.0);
                        } else {
                            ui.add(svg_image(SvgIcon::CircleDot, 12.0, self.pal.dim))
                                .on_hover_text("Checking Jackett…");
                            ui.add_space(6.0);
                        }

                        let sa = self.ui.show_settings;
                        let set_col = if sa { self.pal.accent } else { self.pal.sub };
                        if ui
                            .horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 5.0;
                                svg_icon(ui, SvgIcon::Settings, 14.0, set_col);
                                ui.add(
                                    egui::Button::new(
                                        RichText::new("Settings").size(13.0).color(set_col),
                                    )
                                    .fill(if sa {
                                        tint(self.pal.accent, 22)
                                    } else {
                                        Color32::TRANSPARENT
                                    })
                                    .stroke(Stroke::new(
                                        1.0_f32,
                                        if sa { self.pal.accent } else { self.pal.border },
                                    ))
                                    .corner_radius(6.0)
                                    .min_size(Vec2::new(0.0, 30.0)),
                                )
                                .clicked()
                            })
                            .inner
                        {
                            self.ui.show_settings = !self.ui.show_settings;
                        }
                        ui.add_space(10.0);

                        // Theme picker — width adapts to available space so it
                        // never pushes past the window edge at narrow widths.
                        let ac = self.cfg.theme.accent_color();
                        let theme_w = ui.available_width().clamp(110.0, 155.0);
                        egui::ComboBox::from_id_salt("theme_cb")
                            .selected_text(
                                RichText::new(self.cfg.theme.name())
                                    .font(FontId::proportional(13.0))
                                    .color(ac),
                            )
                            .width(theme_w)
                            .show_ui(ui, |ui| {
                                ui.label(RichText::new("─ Dark ─").size(10.0).color(self.pal.dim));
                                for t in Theme::all().iter().filter(|t| !t.is_light()) {
                                    let col = t.accent_color();
                                    let on = &self.cfg.theme == t;
                                    if ui
                                        .add(egui::Button::selectable(
                                            on,
                                            RichText::new(format!("  {}", t.name()))
                                                .font(FontId::proportional(13.0))
                                                .color(col),
                                        ))
                                        .clicked()
                                    {
                                        self.set_theme(t.clone());
                                    }
                                }
                                ui.add_space(3.0);
                                ui.label(RichText::new("─ Light ─").size(10.0).color(self.pal.dim));
                                for t in Theme::all().iter().filter(|t| t.is_light()) {
                                    let col = t.accent_color();
                                    let on = &self.cfg.theme == t;
                                    if ui
                                        .add(egui::Button::selectable(
                                            on,
                                            RichText::new(format!("  {}", t.name()))
                                                .font(FontId::proportional(13.0))
                                                .color(col),
                                        ))
                                        .clicked()
                                    {
                                        self.set_theme(t.clone());
                                    }
                                }
                            });
                        ui.add_space(10.0);
                    });
                });
            });
    }
}

// ─── Settings panel ────────────────────────────────────────────────────────

impl App {
    pub(crate) fn draw_settings_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(MARGIN_DEFAULT);
            ui.vertical(|ui| {
                // Row 1 — Connection
                ui.horizontal_wrapped(|ui| {
                    lbl(ui, "CONNECTION", self.pal.dim, 10.0);
                    ui.add_space(6.0);
                    lbl(ui, "Jackett URL", self.pal.sub, 12.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.cfg.jackett_url)
                            .desired_width(
                                ui.available_width().clamp(SETTINGS_URL_MIN, SETTINGS_URL_W),
                            )
                            .font(FontId::monospace(12.0)),
                    );
                    ui.add_space(6.0);
                    lbl(ui, "API Key", self.pal.sub, 12.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.cfg.api_key)
                            .desired_width(
                                ui.available_width().clamp(SETTINGS_KEY_MIN, SETTINGS_KEY_W),
                            )
                            .password(!self.ui.key_vis)
                            .hint_text("from Jackett dashboard (top-right)")
                            .font(FontId::monospace(12.0)),
                    );
                    if ui
                        .small_button(if self.ui.key_vis { "hide" } else { "show" })
                        .clicked()
                    {
                        self.ui.key_vis = !self.ui.key_vis;
                    }
                    ui.add_space(6.0);
                    lbl(ui, "Timeout", self.pal.sub, 12.0);
                    let mut ts = self.cfg.timeout_secs.to_string();
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut ts)
                                .desired_width(30.0)
                                .font(FontId::monospace(12.0)),
                        )
                        .changed()
                    {
                        if let Ok(v) = ts.parse::<u64>() {
                            self.cfg.timeout_secs = v.clamp(5, 120);
                        }
                    }
                    lbl(ui, "s", self.pal.dim, 11.0);
                    ui.add_space(8.0);
                    lbl(ui, "RSS", self.pal.sub, 12.0);
                    let mut rs = self.cfg.rss_refresh_secs.to_string();
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut rs)
                                .desired_width(SETTINGS_SMALL_W)
                                .font(FontId::monospace(12.0))
                                .hint_text("600"),
                        )
                        .changed()
                    {
                        if let Ok(v) = rs.parse::<u64>() {
                            self.cfg.rss_refresh_secs = v.clamp(0, 86_400);
                        }
                    }
                    lbl(ui, "s", self.pal.dim, 11.0);
                    ui.add_space(2.0);
                    ui.add(svg_image(SvgIcon::Info, 12.0, self.pal.dim))
                        .on_hover_text(
                            "RSS auto-refresh interval (seconds). 0 disables auto-refresh.",
                        );
                });
                ui.add_space(5.0);

                // Row 2 — Display
                ui.horizontal_wrapped(|ui| {
                    lbl(ui, "DISPLAY", self.pal.dim, 10.0);
                    ui.add_space(6.0);
                    lbl(ui, "Rows", self.pal.sub, 12.0);
                    for (l, h) in [
                        ("Compact", ROW_HEIGHT_COMPACT),
                        ("Normal", ROW_HEIGHT_NORMAL),
                        ("Roomy", ROW_HEIGHT_ROOMY),
                    ] {
                        let on = (self.cfg.row_height - h).abs() < 1.0;
                        if ui
                            .add(egui::Button::selectable(
                                on,
                                RichText::new(l).font(FontId::proportional(12.0)),
                            ))
                            .clicked()
                        {
                            self.cfg.row_height = h;
                            save_cfg(&self.cfg);
                        }
                    }
                    ui.add_space(8.0);
                    lbl(ui, "Font", self.pal.sub, 12.0);
                    for (l, sz) in [("S", 12.0f32), ("M", 14.0), ("L", 16.0)] {
                        let on = (self.cfg.font_size - sz).abs() < 0.5;
                        if ui
                            .add(egui::Button::selectable(
                                on,
                                RichText::new(l).font(FontId::proportional(12.0)),
                            ))
                            .clicked()
                        {
                            self.cfg.font_size = sz;
                            save_cfg(&self.cfg);
                        }
                    }
                    ui.add_space(8.0);
                    lbl(ui, "Page", self.pal.sub, 12.0);
                    for (l, ps) in [("25", 25usize), ("50", 50), ("100", 100), ("All", 0)] {
                        let on = self.cfg.page_size == ps;
                        if ui
                            .add(egui::Button::selectable(
                                on,
                                RichText::new(l).font(FontId::proportional(12.0)),
                            ))
                            .clicked()
                        {
                            self.cfg.page_size = ps;
                            self.search.page = 0;
                            save_cfg(&self.cfg);
                        }
                    }
                    ui.add_space(8.0);
                    if ui
                        .add(egui::Button::selectable(
                            self.cfg.dedupe,
                            RichText::new("Dedupe").font(FontId::proportional(12.0)),
                        ))
                        .on_hover_text("Merge near-duplicate titles across trackers")
                        .clicked()
                    {
                        self.cfg.dedupe = !self.cfg.dedupe;
                        save_cfg(&self.cfg);
                    }
                    ui.add_space(4.0);
                    if ui
                        .add(egui::Button::selectable(
                            self.cfg.show_cat_bar,
                            RichText::new("Cat bar").font(FontId::proportional(12.0)),
                        ))
                        .on_hover_text("Show category breakdown chips")
                        .clicked()
                    {
                        self.cfg.show_cat_bar = !self.cfg.show_cat_bar;
                        save_cfg(&self.cfg);
                    }
                    ui.add_space(8.0);
                    // Custom accent color
                    lbl(ui, "Accent", self.pal.sub, 12.0);
                    let cur = self
                        .cfg
                        .accent
                        .map(|[r, g, b]| rgb(r, g, b))
                        .unwrap_or(self.pal.accent);
                    if ui
                        .add(
                            egui::Button::new("")
                                .fill(cur)
                                .min_size(egui::vec2(18.0, 18.0))
                                .stroke(Stroke::new(1.0_f32, self.pal.border)),
                        )
                        .on_hover_text("Custom accent color (overrides theme)")
                        .clicked()
                    {
                        self.ui.show_color_picker = !self.ui.show_color_picker;
                    }
                    if self.cfg.accent.is_some() {
                        ui.add_space(4.0);
                        if ui
                            .add(
                                egui::Button::new(svg_image(SvgIcon::Close, 11.0, self.pal.sub))
                                    .min_size(egui::vec2(18.0, 18.0))
                                    .stroke(Stroke::new(1.0_f32, self.pal.border)),
                            )
                            .on_hover_text("Reset to theme accent")
                            .clicked()
                        {
                            self.cfg.accent = None;
                            self.pal = Pal::from(&self.cfg.theme, None);
                            save_cfg(&self.cfg);
                        }
                    }
                    // Color picker popup
                    if self.ui.show_color_picker {
                        let mut col: [f32; 3] = self
                            .cfg
                            .accent
                            .map(|[r, g, b]| [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0])
                            .unwrap_or([
                                self.pal.accent.r() as f32 / 255.0,
                                self.pal.accent.g() as f32 / 255.0,
                                self.pal.accent.b() as f32 / 255.0,
                            ]);
                        let mut close = false;
                        egui::Window::new("Accent color")
                            .collapsible(false)
                            .resizable(false)
                            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-30.0, 60.0))
                            .show(ui.ctx(), |ui| {
                                if ui.color_edit_button_rgb(&mut col).changed()
                                    || ui
                                        .add(egui::Slider::new(&mut col[2], 0.0..=1.0).text(""))
                                        .changed()
                                {
                                    self.cfg.accent = Some([
                                        (col[0] * 255.0).round() as u8,
                                        (col[1] * 255.0).round() as u8,
                                        (col[2] * 255.0).round() as u8,
                                    ]);
                                    self.pal = Pal::from(&self.cfg.theme, self.cfg.accent);
                                }
                                ui.add_space(6.0);
                                if ui.button("Done").clicked() {
                                    close = true;
                                }
                            });
                        if close {
                            self.ui.show_color_picker = false;
                            save_cfg(&self.cfg);
                        }
                    }
                });
                ui.add_space(5.0);

                // Row 3 — Columns (toggle which columns are shown)
                ui.horizontal_wrapped(|ui| {
                    lbl(ui, "COLUMNS", self.pal.dim, 10.0);
                    ui.add_space(6.0);
                    let mut col_changed = false;
                    for (label, val) in [
                        ("Tracker", &mut self.cfg.col_tracker),
                        ("Size", &mut self.cfg.col_size),
                        ("Leech", &mut self.cfg.col_leech),
                        ("Ratio", &mut self.cfg.col_ratio),
                        ("Health", &mut self.cfg.col_health),
                        ("Date", &mut self.cfg.col_date),
                    ] {
                        let on = *val;
                        if ui
                            .add(egui::Button::selectable(
                                on,
                                RichText::new(label)
                                    .font(FontId::proportional(12.0))
                                    .color(if on { self.pal.accent } else { self.pal.dim }),
                            ))
                            .clicked()
                        {
                            *val = !*val;
                            col_changed = true;
                        }
                        ui.add_space(2.0);
                    }
                    if col_changed {
                        save_cfg(&self.cfg);
                    }
                });
                ui.add_space(5.0);

                // Row 4 — Column order (dedicated row: each column chip + SVG
                // up/down arrows; no longer nested inside the COLUMNS toggles,
                // which made the chips and arrows wrap together and scramble).
                ui.horizontal_wrapped(|ui| {
                    lbl(ui, "ORDER", self.pal.dim, 10.0);
                    ui.add_space(6.0);
                    let mut moved: Option<(usize, isize)> = None;
                    for (idx, name) in self.cfg.col_order.clone().iter().enumerate() {
                        // Each column is ONE atomic horizontal unit (chip + gap
                        // + vertical arrows) so horizontal_wrapped never splits
                        // it and every unit shares the same row height — this is
                        // what keeps the whole ORDER row on a straight line.
                        let can_up = idx > 0;
                        let can_down = idx + 1 < self.cfg.col_order.len();
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(2.0, 0.0);
                            let lbl_txt = RichText::new(name.as_str())
                                .font(FontId::proportional(11.5))
                                .color(if idx == 0 {
                                    self.pal.accent
                                } else {
                                    self.pal.sub
                                });
                            egui::Frame::NONE
                                .fill(if idx == 0 {
                                    tint(self.pal.accent, 14)
                                } else {
                                    self.pal.surface
                                })
                                .stroke(Stroke::new(1.0_f32, self.pal.border))
                                .corner_radius(4.0)
                                .inner_margin(egui::Margin::symmetric(7, 3))
                                .show(ui, |ui| {
                                    // Vertically center the label against the
                                    // 18px arrow stack so every unit aligns.
                                    ui.vertical_centered(|ui| {
                                        ui.add_space(2.0);
                                        ui.label(lbl_txt);
                                        ui.add_space(2.0);
                                    });
                                });
                            if let Some(d) = reorder_control(
                                ui,
                                self.pal.sub,
                                can_up,
                                can_down,
                                "Move left",
                                "Move right",
                            ) {
                                moved = Some((idx, d));
                            }
                        });
                        ui.add_space(2.0);
                    }
                    if let Some((idx, d)) = moved {
                        let j = (idx as isize + d) as usize;
                        let name = self.cfg.col_order.remove(idx);
                        self.cfg.col_order.insert(j, name);
                        save_cfg(&self.cfg);
                    }
                    // Save sits inline right after the Order chips — NOT
                    // right-aligned to the full panel width, which left a huge
                    // empty void between the chips and the orphaned button on
                    // wide monitors.
                    ui.add_space(10.0);
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("Save")
                                    .font(FontId::proportional(12.0))
                                    .color(self.pal.green),
                            )
                            .fill(tint(self.pal.green, 18))
                            .stroke(Stroke::new(1.0_f32, tint(self.pal.green, 80)))
                            .corner_radius(4.0),
                        )
                        .clicked()
                    {
                        if let Some(err) =
                            crate::jackett::validate_jackett_url(&self.cfg.jackett_url)
                        {
                            self.toast(&format!("Jackett URL invalid: {err}"), self.pal.red);
                        } else {
                            save_cfg(&self.cfg);
                            self.toast("Settings saved", self.pal.green);
                        }
                    }
                });
            });
        });
    }
}

// ─── Search tab ────────────────────────────────────────────────────────────

impl App {}
