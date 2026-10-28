//! header drawing methods.

use super::*;

use crate::app::App;
use crate::lock_or_recover;

/// Write a reordered run of movable columns back into the slots those columns
/// already occupy, leaving the pinned ones (Name, Seeds) and any unknown names
/// exactly where they were. Keeps `col_order` a permutation instead of letting
/// the popup's local list overwrite the saved one.
pub(crate) fn apply_col_order(saved: &[String], movable: &[TableCol]) -> Vec<String> {
    let mut it = movable.iter();
    saved
        .iter()
        .map(|n| match TableCol::from_name(n) {
            Some(TableCol::Name) | Some(TableCol::Seeds) | None => n.clone(),
            Some(_) => it
                .next()
                .map(|c| c.label().to_string())
                .unwrap_or_else(|| n.clone()),
        })
        .collect()
}

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
                // The wrapped logo row and the right-to-left control block
                // cannot share one line once the window is phone-width: the
                // right-hand layout was left ~0 px and the tab labels clipped
                // to "arch". Below NARROW_W they get a row each.
                if narrow(ui.available_width()) {
                    self.header_narrow(ui);
                } else {
                    self.header_wide(ui);
                }
            });
    }

    /// Wide: logo, tabs and the right-hand controls share one row.
    fn header_wide(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            ui.add_space(MARGIN_DEFAULT + 2.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                self.header_logo(ui);
                ui.add_space(SP5);
                ui.separator();
                ui.add_space(SP3);
                self.header_tabs(ui);
            });
            self.header_right(ui);
        });
    }

    /// Narrow: the controls take the first row, the tabs the second, so every
    /// label keeps its full text and each control keeps a tappable width.
    fn header_narrow(&mut self, ui: &mut egui::Ui) {
        ui.add_space(SP1);
        ui.horizontal(|ui| {
            self.header_logo(ui);
            self.header_right(ui);
        });
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.add_space(SP2);
            self.header_tabs(ui);
        });
        ui.add_space(SP1);
    }

    /// Wordmark + version pill.
    fn header_logo(&mut self, ui: &mut egui::Ui) {
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
            .corner_radius(R_LG)
            .inner_margin(margin(SP2, SP1))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(env!("CARGO_PKG_VERSION"))
                        .size(11.0)
                        .color(self.pal.accent),
                );
            });
    }

    /// The four tabs, shared by both header shapes.
    fn header_tabs(&mut self, ui: &mut egui::Ui) {
        // Tabs (SVG icon + label)
        for (icon, label, tip, tab) in [
            (SvgIcon::Search, "Search", "Search torrents", Tab::Search),
            (SvgIcon::Star, "Favorites", "Saved torrents", Tab::Favorites),
            (SvgIcon::Rss, "RSS", "RSS feed reader", Tab::Rss),
            (SvgIcon::Info, "About", "About TorrentX", Tab::About),
        ] {
            let active = self.ui.tab == tab;
            let badge = if tab == Tab::Favorites && !self.cfg.favorites.is_empty() {
                format!(" {}", self.cfg.favorites.len())
            } else if tab == Tab::Search {
                let n = *lock_or_recover(&self.search.count);
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
            // One widget for icon + label: clicking the
            // symbol switches tabs too, not just the text.
            let clicked = icon_text_tab(
                ui,
                icon,
                &format!("{label}{badge}"),
                col,
                active,
                self.pal.accent,
            )
            .on_hover_text(tip)
            .clicked();
            if clicked && self.ui.tab != tab {
                if tab != Tab::Favorites {
                    self.ui.fav_search.clear();
                }
                self.set_tab(tab);
                self.close_detail();
                // Clear cross-tab hover/selection state so a
                // stale row highlight never bleeds into the
                // newly shown tab's table.
                self.ui.hovered = None;
                self.rss.rss_detail = None;
            }
            ui.add_space(SP1);
        }
    }

    /// Theme picker, Settings and the Jackett status dot, right-aligned.
    fn header_right(&mut self, ui: &mut egui::Ui) {
        // Right side controls
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(SP4);

            // Jackett connection status dot
            if let Some(ok) = self.net.jackett_ok {
                let (col, tip) = if ok {
                    (self.pal.green, "Jackett connected")
                } else {
                    (self.pal.red, "Jackett unreachable — check Settings")
                };
                ui.add(svg_image(SvgIcon::Circle, ICON_SM, col))
                    .on_hover_text(tip);
                ui.add_space(SP3);
            } else {
                ui.add(svg_image(SvgIcon::CircleDot, ICON_SM, self.pal.dim))
                    .on_hover_text("Checking Jackett…");
                ui.add_space(SP3);
            }

            let sa = self.ui.show_settings;
            let set_col = if sa { self.pal.accent } else { self.pal.sub };
            // Same one-widget pattern as the header tabs: the gear is
            // inside the button, so the pill frames icon + label together
            // instead of floating beside a text-only button.
            if icon_text_tab(
                ui,
                SvgIcon::Settings,
                "Settings",
                set_col,
                sa,
                self.pal.accent,
            )
            .clicked()
            {
                self.set_settings_open(!self.ui.show_settings);
            }
            ui.add_space(SP4);

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
                    ui.label(RichText::new("─ Dark ─").size(11.0).color(self.pal.dim));
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
                    ui.add_space(SP1);
                    ui.label(RichText::new("─ Light ─").size(11.0).color(self.pal.dim));
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
            ui.add_space(SP4);
        });
    }
}

// ─── Settings panel ────────────────────────────────────────────────────────

impl App {
    pub(crate) fn draw_settings_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(MARGIN_DEFAULT);
            ui.vertical(|ui| {
                self.settings_connection(ui);
                self.settings_display(ui);
                self.settings_toggles(ui);
                self.settings_columns(ui);
                self.settings_save_row(ui);
            });
        });
    }

    /// Connection: Jackett URL + API key, timeout, RSS interval and player,
    /// plus the Start/Open Jackett actions.
    fn settings_connection(&mut self, ui: &mut egui::Ui) {
        // Sections read as headings over their controls (an inline
        // heading made every row start at a different x).
        lbl(ui, "CONNECTION", self.pal.sub, 12.0);
        ui.add_space(SP2);
        ui.horizontal_wrapped(|ui| {
            ui.add_space(SP1);
            lbl(ui, "Jackett URL", self.pal.sub, 12.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.cfg.jackett_url)
                    .desired_width(ui.available_width().clamp(SETTINGS_URL_MIN, SETTINGS_URL_W))
                    .font(FontId::monospace(12.0)),
            );
            ui.add_space(SP3);
            lbl(ui, "API Key", self.pal.sub, 12.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.cfg.api_key)
                    .desired_width(ui.available_width().clamp(SETTINGS_KEY_MIN, SETTINGS_KEY_W))
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
            ui.add_space(SP3);
            // Run a local Jackett instead of asking the user to install and
            // start one themselves. Not bundled: 48MB + GPL-2.0 against an
            // MIT 18MB app, and an AppImage payload is read-only.
            let have_bin = crate::jackett::find_bin().is_some();
            ui.horizontal(|ui| {
                let busy = crate::jackett::MANAGED_BUSY.load(std::sync::atomic::Ordering::SeqCst);
                let label = if busy {
                    "Starting Jackett…"
                } else if have_bin {
                    "Start Jackett"
                } else {
                    "Install Jackett (48 MB)"
                };
                if ui
                    .add_enabled(
                        !busy,
                        egui::Button::new(RichText::new(label).size(11.5))
                            .min_size(egui::vec2(0.0, 25.0)),
                    )
                    .on_hover_text(if have_bin {
                        "Starts the Jackett on this machine (localhost only)"
                    } else {
                        "Downloads Jackett once into ~/.local/share/torrentx"
                    })
                    .clicked()
                {
                    crate::jackett::start_local_async(
                        self.cfg.jackett_url.clone(),
                        self.cfg.api_key.clone(),
                    );
                }
                if ui
                    .add(
                        egui::Button::new(RichText::new("Open Jackett").size(11.5))
                            .min_size(egui::vec2(0.0, 25.0)),
                    )
                    .on_hover_text("Add or change indexers in Jackett's own page")
                    .clicked()
                {
                    let _ = crate::safe_open(self.cfg.jackett_url.clone());
                }
            });
            // Worker result: adopt the url/key, or report why not.
            let done = lock_or_recover(&crate::jackett::MANAGED).take();
            if let Some(res) = done {
                match res {
                    Ok((url, key)) => {
                        self.cfg.jackett_url = url;
                        if !key.is_empty() {
                            self.cfg.api_key = key;
                        }
                        let _ = save_cfg(&self.cfg);
                        self.toast(
                            "Jackett is running — add indexers, then search",
                            self.pal.green,
                        );
                    }
                    Err(e) => self.toast(&format!("Jackett: {e}"), self.pal.red),
                }
            }
            ui.add_space(SP3);
            lbl(ui, "Timeout", self.pal.sub, 12.0);
            let te = ui
                .add(
                    egui::TextEdit::singleline(&mut self.ui.timeout_buf)
                        .desired_width(30.0)
                        .font(FontId::monospace(12.0)),
                )
                .on_hover_text(
                    "Per request, 5-120 s. An All search gives each indexer at most \
                     20 s — one slow tracker must not become the search duration.",
                );
            if te.lost_focus() {
                // Commit once, on focus loss: clamping every keystroke and
                // writing the result back into the buffer made "10"
                // impossible to type (it became 50).
                if let Ok(v) = self.ui.timeout_buf.trim().parse::<u64>() {
                    self.cfg.timeout_secs = v.clamp(5, 120);
                }
                self.ui.timeout_buf = self.cfg.timeout_secs.to_string();
            }
            lbl(ui, "s", self.pal.dim, 11.0);
            ui.add_space(SP3);
            lbl(ui, "RSS", self.pal.sub, 12.0);
            let rs_te = ui.add(
                egui::TextEdit::singleline(&mut self.ui.rss_secs_buf)
                    .desired_width(SETTINGS_SMALL_W)
                    .font(FontId::monospace(12.0))
                    .hint_text("600"),
            );
            if rs_te.lost_focus() {
                if let Ok(v) = self.ui.rss_secs_buf.trim().parse::<u64>() {
                    self.cfg.rss_refresh_secs = v.clamp(0, 86_400);
                }
                self.ui.rss_secs_buf = self.cfg.rss_refresh_secs.to_string();
            }
            lbl(ui, "s", self.pal.dim, 11.0);
            ui.add_space(SP1);
            ui.add(svg_image(SvgIcon::Info, ICON_SM, self.pal.dim))
                .on_hover_text("RSS auto-refresh interval (seconds). 0 disables auto-refresh.");
            ui.add_space(SP3);
            lbl(ui, "PLAY WITH", self.pal.dim, 11.0);
            egui::ComboBox::from_id_salt("player_cb")
                .selected_text(self.cfg.player.clone())
                .width(64.0)
                .show_ui(ui, |ui| {
                    for p in crate::player::PLAYERS {
                        if ui
                            .selectable_value(&mut self.cfg.player, p.to_string(), p)
                            .changed()
                        {
                            let _ = save_cfg(&self.cfg);
                        }
                    }
                });
        });
        ui.add_space(SP2);
    }

    /// Display: theme, row height, font size and the UI scale.
    fn settings_display(&mut self, ui: &mut egui::Ui) {
        // Row 2 — Display
        lbl(ui, "DISPLAY", self.pal.sub, 12.0);
        ui.add_space(SP2);
        ui.horizontal_wrapped(|ui| {
            ui.add_space(SP1);
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
                    let _ = save_cfg(&self.cfg);
                }
            }
            ui.add_space(SP3);
            // One control for "everything is too small". A segmented picker,
            // not a slider: it is the vocabulary Rows and Page already use,
            // and every step is one click away (Ctrl +/- still fine-tunes).
            // A `Slider` here was inert — 75 px of track that ignored both
            // clicks and drags while the buttons beside it worked.
            lbl(ui, "Scale", self.pal.sub, 12.0);
            for (l, v) in [
                ("80%", 0.8f32),
                ("90%", 0.9),
                ("100%", 1.0),
                ("110%", 1.1),
                ("125%", 1.25),
                ("150%", 1.5),
            ] {
                let on = (self.cfg.ui_scale - v).abs() < 0.01;
                if ui
                    .add(egui::Button::selectable(
                        on,
                        RichText::new(l).font(FontId::proportional(12.0)),
                    ))
                    .on_hover_text("Scales text, rows and icons together (Ctrl +/-)")
                    .clicked()
                {
                    self.cfg.ui_scale = v;
                    let _ = save_cfg(&self.cfg);
                }
            }
            ui.add_space(SP3);
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
                    let _ = save_cfg(&self.cfg);
                }
            }
        });
    }

    /// Behaviour toggles: updates check, autostart, player, colour picker.
    fn settings_toggles(&mut self, ui: &mut egui::Ui) {
        // The toggles get their own line: seven groups on one row read as a
        // dense wall at any width and wrapped unpredictably.
        ui.horizontal_wrapped(|ui| {
            ui.add_space(SP1);
            if ui
                .add(egui::Button::selectable(
                    self.cfg.check_updates,
                    RichText::new("Updates").font(FontId::proportional(12.0)),
                ))
                .on_hover_text("Check GitHub for a newer release at startup")
                .clicked()
            {
                self.cfg.check_updates = !self.cfg.check_updates;
                let _ = save_cfg(&self.cfg);
            }
            // XDG autostart is a Linux mechanism. Elsewhere the pill
            // would be a control that silently does nothing (macOS needs a
            // LaunchAgent, Windows a Startup-folder link), so it is absent
            // rather than lying — and neither platform ships a build.
            #[cfg(target_os = "linux")]
            {
                ui.add_space(SP2);
                if ui
                    .add(egui::Button::selectable(
                        crate::autostart_on(),
                        RichText::new("At login").font(FontId::proportional(12.0)),
                    ))
                    .on_hover_text("Start TorrentX at login so RSS feeds keep polling")
                    .clicked()
                {
                    crate::set_autostart(!crate::autostart_on());
                }
            }
            ui.add_space(SP2);
            if ui
                .add(egui::Button::selectable(
                    self.cfg.show_cat_bar,
                    RichText::new("Cat bar").font(FontId::proportional(12.0)),
                ))
                .on_hover_text("Show category breakdown chips")
                .clicked()
            {
                self.cfg.show_cat_bar = !self.cfg.show_cat_bar;
                let _ = save_cfg(&self.cfg);
            }
            ui.add_space(SP3);
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
                ui.add_space(SP2);
                if ui
                    .add(
                        egui::Button::new(svg_image(SvgIcon::Close, ICON_SM, self.pal.sub))
                            .min_size(egui::vec2(18.0, 18.0))
                            .stroke(Stroke::new(1.0_f32, self.pal.border)),
                    )
                    .on_hover_text("Reset to theme accent")
                    .clicked()
                {
                    self.cfg.accent = None;
                    self.pal = Pal::from(&self.cfg.theme, None);
                    let _ = save_cfg(&self.cfg);
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
                            // Persist on change, not only on "Done": closing
                            // the picker by switching tabs or the Settings
                            // toggle used to discard the colour.
                            let _ = save_cfg(&self.cfg);
                        }
                        ui.add_space(SP3);
                        if ui.button("Done").clicked() {
                            close = true;
                        }
                    });
                if close {
                    self.ui.show_color_picker = false;
                    let _ = save_cfg(&self.cfg);
                }
            }
        });
        ui.add_space(SP2);
    }

    /// Columns: visibility chips and the up/down reordering.
    fn settings_columns(&mut self, ui: &mut egui::Ui) {
        // Row 3 — Columns. The chips toggle visibility; the arrows move a
        // column, because `cfg.col_order` already drives the table and had
        // no way to be edited. Name and Seeds stay pinned: Name owns the
        // row's full-cell click layer and Seeds anchors the sort arrow.
        lbl(ui, "COLUMNS", self.pal.sub, 12.0);
        ui.add_space(SP2);
        let mut order: Vec<TableCol> = self
            .cfg
            .col_order
            .iter()
            .filter_map(|n| TableCol::from_name(n))
            .filter(|c| !matches!(c, TableCol::Name | TableCol::Seeds))
            .collect();
        let mut col_changed = false;
        ui.horizontal_wrapped(|ui| {
            ui.add_space(SP1);
            let mut i = 0;
            while i < order.len() {
                let c = order[i];
                let flag = match c {
                    TableCol::Tracker => &mut self.cfg.col_tracker,
                    TableCol::Size => &mut self.cfg.col_size,
                    TableCol::Leech => &mut self.cfg.col_leech,
                    TableCol::Ratio => &mut self.cfg.col_ratio,
                    TableCol::Health => &mut self.cfg.col_health,
                    TableCol::Date => &mut self.cfg.col_date,
                    TableCol::Name | TableCol::Seeds => unreachable!(),
                };
                let on = *flag;
                if ui
                    .add(egui::Button::selectable(
                        on,
                        RichText::new(c.label())
                            .font(FontId::proportional(12.0))
                            .color(if on { self.pal.accent } else { self.pal.dim }),
                    ))
                    .clicked()
                {
                    *flag = !on;
                    col_changed = true;
                }
                let arrow = |ui: &mut egui::Ui, icon: SvgIcon, live: bool| {
                    ui.add_enabled(
                        live,
                        egui::Button::image(svg_image(
                            icon,
                            11.0,
                            if live { self.pal.sub } else { self.pal.dim },
                        ))
                        .frame(false),
                    )
                    .clicked()
                };
                if arrow(ui, SvgIcon::ChevronLeft, i > 0) {
                    order.swap(i, i - 1);
                    col_changed = true;
                }
                if arrow(ui, SvgIcon::ChevronRight, i + 1 < order.len()) {
                    order.swap(i, i + 1);
                    col_changed = true;
                }
                ui.add_space(SP1);
                i += 1;
            }
            if col_changed {
                self.cfg.col_order = apply_col_order(&self.cfg.col_order, &order);
                let _ = save_cfg(&self.cfg);
            }
        });
        ui.add_space(SP2);
    }

    /// Save: validates what will actually be stored, right-aligned.
    fn settings_save_row(&mut self, ui: &mut egui::Ui) {
        // Save button — right-aligned, no dead void on wide panels.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(MARGIN_DEFAULT);
            if ui
                .add(
                    egui::Button::new(
                        RichText::new("Save")
                            .font(FontId::proportional(12.0))
                            .color(self.pal.green),
                    )
                    .fill(tint(self.pal.green, 18))
                    .stroke(Stroke::new(1.0_f32, tint(self.pal.green, 80)))
                    .corner_radius(R_SM),
                )
                .clicked()
            {
                // Validate what will actually be STORED: the validator trims,
                // so a padded URL used to pass validation and then fail every
                // request. Same for the API key.
                let url = self.cfg.jackett_url.trim().to_string();
                if let Some(err) = crate::jackett::validate_jackett_url(&url) {
                    self.toast(&format!("Jackett URL invalid: {err}"), self.pal.red);
                } else {
                    self.cfg.jackett_url = url;
                    self.cfg.api_key = self.cfg.api_key.trim().to_string();
                    // Report the real outcome — the old code toasted success
                    // even when the write failed (read-only dir, full disk).
                    match save_cfg(&self.cfg) {
                        Ok(()) => self.toast("Settings saved", self.pal.green),
                        Err(e) => {
                            self.toast(&format!("Could not save settings: {e}"), self.pal.red)
                        }
                    }
                }
            }
        });
    }
}

// ─── Search tab ────────────────────────────────────────────────────────────

impl App {}

#[cfg(test)]
mod order_tests {
    use super::apply_col_order;
    use crate::jackett::TableCol;

    fn names(v: &[TableCol]) -> Vec<String> {
        v.iter().map(|c| c.label().to_string()).collect()
    }

    /// A swap inside the movable run must land back in the same slots, so the
    /// pinned columns never drift and no name is lost or duplicated.
    #[test]
    fn reordering_rewrites_only_the_movable_slots() {
        let saved = vec![
            "Name", "Tracker", "Size", "Seeds", "Leech", "Ratio", "Health", "Date",
        ]
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();

        let mut movable: Vec<TableCol> = saved
            .iter()
            .filter_map(|n| TableCol::from_name(n))
            .filter(|c| !matches!(c, TableCol::Name | TableCol::Seeds))
            .collect();
        // Move Date to the front of the movable run (arrow presses, in effect).
        let last = movable.len() - 1;
        for i in (0..last).rev() {
            movable.swap(i, i + 1);
        }
        assert_eq!(names(&movable)[0], "Date");

        let out = apply_col_order(&saved, &movable);
        assert_eq!(out.len(), saved.len(), "no column may be lost");
        assert_eq!(out[0], "Name", "Name is pinned to the front");
        assert_eq!(out[3], "Seeds", "Seeds keeps its slot");
        assert_eq!(out[1], "Date", "the reordered run fills the old slots");
        let mut sorted = out.clone();
        sorted.sort();
        let mut expect = saved.clone();
        expect.sort();
        assert_eq!(sorted, expect, "still a permutation of the saved order");
    }
}
