//! search drawing methods.

use super::*;

use crate::app::App;

impl App {
    pub(crate) fn draw_search(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        state: &SearchState,
    ) {
        let fs = self.cfg.font_size;
        let busy = *state == SearchState::Searching;
        ui.add_space(10.0);
        let mut bar_rect = egui::Rect::NOTHING;

        // Search input
        ui.horizontal_wrapped(|ui| {
            ui.add_space(12.0);
            // Field shrinks with the window; floor at 120 so the combos +
            // Search button can wrap onto a second row instead of clipping.
            let field_w = (ui.available_width() - 320.0).clamp(120.0, 700.0);
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.search.query)
                    .id(egui::Id::new("q"))
                    .desired_width(field_w)
                    .hint_text("Search torrents — movies, shows, games, software, anime…")
                    .font(FontId::proportional(fs + 2.0)),
            );
            bar_rect = resp.rect;
            if resp.gained_focus() && !self.cfg.history.is_empty() {
                self.ui.show_hist = true;
            }
            if resp.changed() && self.search.query.is_empty() {
                self.ui.show_hist = false;
            }
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.do_search();
            }

            ui.add_space(6.0);
            egui::ComboBox::from_id_salt("cat_cb")
                .selected_text(RichText::new(&self.search.cat).font(FontId::proportional(fs)))
                .width(115.0)
                .show_ui(ui, |ui| {
                    for &c in CATS {
                        ui.selectable_value(
                            &mut self.search.cat,
                            c.into(),
                            RichText::new(c).font(FontId::proportional(fs)),
                        );
                    }
                });

            // Indexer picker (from Jackett's configured indexers)
            ui.add_space(6.0);
            egui::ComboBox::from_id_salt("idx_cb")
                .selected_text(RichText::new(&self.net.indexer).font(FontId::proportional(fs)))
                .width(130.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.net.indexer,
                        "All".into(),
                        RichText::new("All").font(FontId::proportional(fs)),
                    );
                    for idx in &self.net.indexers {
                        ui.selectable_value(
                            &mut self.net.indexer,
                            idx.clone(),
                            RichText::new(idx).font(FontId::proportional(fs)),
                        );
                    }
                })
                .response
                .on_hover_text("Search one indexer instead of all");

            ui.add_space(6.0);
            if ui
                .add_enabled(
                    !busy,
                    egui::Button::new(
                        RichText::new(if busy {
                            "  Scanning…  "
                        } else {
                            "    Search    "
                        })
                        .font(FontId::proportional(fs))
                        .strong()
                        .color(Color32::WHITE),
                    )
                    .fill(if busy {
                        tint(self.pal.accent, 190)
                    } else {
                        self.pal.accent
                    })
                    .corner_radius(6.0)
                    .min_size(Vec2::new(0.0, 36.0)),
                )
                .clicked()
            {
                self.do_search();
            }

            if !self.search.query.is_empty()
                && ui
                    .add(
                        egui::Button::new(svg_image(SvgIcon::Close, 14.0, self.pal.sub))
                            .fill(tint(self.pal.sub, 12))
                            .stroke(Stroke::new(1.0_f32, self.pal.border))
                            .corner_radius(6.0)
                            .min_size(Vec2::new(0.0, 36.0)),
                    )
                    .on_hover_text("Clear search")
                    .clicked()
            {
                self.search.query.clear();
                self.ui.show_hist = false;
            }
        });

        // History dropdown
        self.draw_history_dropdown(ctx, bar_rect, fs);

        ui.add_space(8.0);

        // Filter bar
        self.draw_filter_bar(ui, fs);

        ui.add_space(8.0);

        // State-dependent content
        // Fade in the whole state block (match is the last thing drawn in
        // this function, so opacity can't leak to later widgets). Covers all
        // four states including Done. eased(1.0)=1.0 → no-op at rest.
        let search_anim = self.ui.search_state_anim;
        let eased = (self.tokens.easing)(search_anim);
        ui.set_opacity(eased);

        match state {
            SearchState::Idle => {
                self.draw_idle(ui);
            }
            SearchState::Searching => {
                ui.add_space(70.0);
                ui.vertical_centered(|ui| {
                    ui.spinner();
                    ui.add_space(12.0);
                    lbl(ui, "Scanning all Jackett indexers…", self.pal.sub, 16.0);
                    ui.add_space(4.0);
                    lbl(ui, "This usually takes 10–30 seconds", self.pal.dim, fs);
                });
            }
            SearchState::Error(err) => {
                ui.add_space(10.0);
                egui::Frame::NONE
                    .fill(tint(self.pal.red, 10))
                    .stroke(Stroke::new(1.0_f32, tint(self.pal.red, 70)))
                    .corner_radius(PANEL_RADIUS)
                    .inner_margin(egui::Margin::symmetric(20, 14))
                    .outer_margin(egui::Margin::symmetric(PANEL_MARGIN_X, 0))
                    .show(ui, |ui| {
                        for line in err.lines() {
                            lbl(ui, line, self.pal.red, fs);
                        }
                        ui.add_space(8.0);
                        if outline_btn(ui, "Open Settings", self.pal.accent) {
                            self.ui.show_settings = true;
                        }
                    });
            }
            SearchState::Done => {
                // Fire a desktop notification on transition (once per search).
                if !self.ui.notified {
                    self.ui.notified = true;
                    self.notify_search_done();
                }
                let raw = self.all_results();
                let sorted = self.filtered(&raw);
                let total = sorted.len();

                // Clamp selected index after filtering (page-local index space)
                let page_n = self.page_slice(&sorted).len();
                self.ui.selected = self.ui.selected.filter(|&i| i < page_n);
                if self.ui.selected.is_none() {
                    self.ui.detail_open = false;
                }

                if total == 0 {
                    ui.add_space(40.0);
                    ui.vertical_centered(|ui| {
                        lbl(ui, "No results match your filters", self.pal.sub, 17.0);
                        if !raw.is_empty() {
                            lbl(
                                ui,
                                &format!("{} results hidden by filters", raw.len()),
                                self.pal.dim,
                                fs,
                            );
                        }
                    });
                    return;
                }

                let max_p = self.max_pages(total);
                if self.search.page >= max_p {
                    self.search.page = max_p.saturating_sub(1);
                }
                let pg = self.search.page;
                let page_s = self.page_slice(&sorted).to_vec();
                let page_n = page_s.len();

                // Stats bar
                ui.horizontal_wrapped(|ui| {
                    ui.add_space(12.0);
                    let active: usize =
                        sorted.iter().filter(|r| r.seeders.unwrap_or(0) > 0).count();
                    let seeds: u32 = sorted.iter().map(|r| r.seeders.unwrap_or(0)).sum();
                    let trackers: std::collections::HashSet<_> =
                        sorted.iter().filter_map(|r| r.tracker.as_deref()).collect();
                    lbl(
                        ui,
                        &format!(
                            "Showing {page_n} of {total}  ·  {active} active  ·  \
                                      {seeds} seeds  ·  {} trackers",
                            trackers.len()
                        ),
                        self.pal.sub,
                        fs - 1.0,
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(12.0);
                        let sc = sorted.clone();
                        if outline_btn(ui, "Export CSV", self.pal.sub) {
                            self.export_csv(&sc);
                            self.toast("Exported to Downloads", self.pal.green);
                        }
                    });
                });

                // Category chips
                if self.cfg.show_cat_bar {
                    let chips = App::cat_chips(&sorted);
                    if !chips.is_empty() {
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.add_space(12.0);
                            for (cat, count, col) in &chips {
                                let sel = self.search.f_text == *cat;
                                egui::Frame::NONE
                                    .fill(tint(*col, if sel { 50 } else { 20 }))
                                    .corner_radius(10.0)
                                    .stroke(Stroke::new(
                                        if sel { 1.5_f32 } else { 1.0_f32 },
                                        tint(*col, if sel { 200 } else { 80 }),
                                    ))
                                    .inner_margin(egui::Margin::symmetric(7, 2))
                                    .show(ui, |ui| {
                                        // Whole-chip click (not just the text).
                                        let chip_resp = ui.interact(
                                            ui.max_rect(),
                                            egui::Id::new(("chip", cat)),
                                            egui::Sense::click(),
                                        );
                                        ui.label(
                                            RichText::new(format!("{cat}  {count}"))
                                                .font(FontId::proportional(11.0))
                                                .color(*col),
                                        );
                                        if chip_resp.hovered() {
                                            ui.ctx()
                                                .set_cursor_icon(egui::CursorIcon::PointingHand);
                                        }
                                        if chip_resp.clicked() {
                                            if self.search.f_text == *cat {
                                                self.search.f_text.clear();
                                            } else {
                                                self.search.f_text = cat.clone();
                                            }
                                            // Category filter changed the result set: drop batch selections.
                                            self.ui.sel_set.clear();
                                            self.ui.sel_mode = false;
                                            self.search.page = 0;
                                        }
                                    });
                                ui.add_space(3.0);
                            }
                        });
                    }
                }
                ui.add_space(4.0);

                // Keyboard navigation (only when no text input is focused)
                let typing = ui.ctx().egui_wants_keyboard_input();
                if !typing && ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                    self.ui.selected = Some(
                        self.ui
                            .selected
                            .map_or(0, |s| (s + 1).min(page_n.saturating_sub(1))),
                    );
                    self.ui.detail_open = true;
                }
                if !typing && ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                    self.ui.selected = Some(self.ui.selected.map_or(0, |s| s.saturating_sub(1)));
                    self.ui.detail_open = true;
                }
                if !typing && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if let Some(i) = self.ui.selected {
                        if let Some(r) = page_s.get(i) {
                            if let Some(m) = &r.magnet_uri {
                                if is_magnet(m) {
                                    let _ = open::that(m);
                                    self.toast("Opening magnet…", self.pal.accent);
                                } else {
                                    self.toast("No valid magnet link", self.pal.yellow);
                                }
                            }
                        }
                    }
                }
                if !typing
                    && ui.input(|i| i.key_pressed(egui::Key::D))
                    && self.ui.selected.is_some()
                {
                    self.ui.detail_open = !self.ui.detail_open;
                }
                if !typing && ui.input(|i| i.key_pressed(egui::Key::F)) {
                    if let Some(i) = self.ui.selected {
                        if let Some(r) = page_s.get(i).cloned() {
                            self.add_fav(&r);
                        }
                    }
                }
                if !typing && ui.input(|i| i.key_pressed(egui::Key::M)) {
                    if let Some(i) = self.ui.selected {
                        if let Some(r) = page_s.get(i) {
                            if let Some(m) = &r.magnet_uri {
                                if is_magnet(m) {
                                    let _ = open::that(m);
                                    self.toast("Opening magnet…", self.pal.accent);
                                } else {
                                    self.toast("No valid magnet link", self.pal.yellow);
                                }
                            }
                        }
                    }
                }

                // Pagination
                self.draw_pagination(ui, max_p, pg, fs);

                // Results table
                let base = if self.cfg.page_size == 0 {
                    0
                } else {
                    pg * self.cfg.page_size
                };
                self.draw_results_table(ui, &page_s, base);
            }
        }
    }

    /// Bottom pagination bar (Prev / page numbers / Next). Shown only when
    /// there's more than one page. Mutates `self.search.page` and clears selection.
    fn draw_pagination(&mut self, ui: &mut egui::Ui, max_p: usize, pg: usize, fs: f32) {
        if max_p <= 1 {
            return;
        }
        egui::Panel::bottom("pages")
            .default_size(34.0)
            .frame(
                egui::Frame::NONE
                    .fill(self.pal.bg)
                    .stroke(Stroke::new(1.0_f32, self.pal.border))
                    .inner_margin(egui::Margin::symmetric(PANEL_MARGIN_X, 5)),
            )
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if icon_text_btn(ui, SvgIcon::ChevronLeft, "Prev", self.pal.sub, pg > 0) {
                        self.search.page -= 1;
                        self.ui.selected = None;
                        self.ui.detail_open = false; // page content changed; drop stale detail
                        self.ui.detail_row = None;
                    }
                    ui.add_space(6.0);
                    for p in 0..max_p {
                        let near = p == 0 || p == max_p - 1 || p.abs_diff(pg) <= 2;
                        if !near {
                            if p == 1 || p == max_p - 2 {
                                lbl(ui, "…", self.pal.dim, fs - 1.0);
                            }
                            continue;
                        }
                        let on = p == pg;
                        if ui
                            .add(egui::Button::selectable(
                                on,
                                RichText::new(format!("{}", p + 1))
                                    .font(FontId::proportional(fs - 1.0))
                                    .color(if on { self.pal.accent } else { self.pal.sub }),
                            ))
                            .clicked()
                        {
                            self.search.page = p;
                            self.ui.selected = None;
                            self.ui.detail_open = false;
                            self.ui.detail_row = None;
                        }
                    }
                    ui.add_space(6.0);
                    if icon_text_btn(
                        ui,
                        SvgIcon::ChevronRight,
                        "Next",
                        self.pal.sub,
                        pg + 1 < max_p,
                    ) {
                        self.search.page += 1;
                        self.ui.selected = None;
                        self.ui.detail_open = false;
                        self.ui.detail_row = None;
                    }
                    lbl(
                        ui,
                        &format!("  Page {} of {max_p}", pg + 1),
                        self.pal.dim,
                        fs - 1.0,
                    );
                });
            });
    }

    pub(crate) fn draw_detail_panel(&mut self, ui: &mut egui::Ui) {
        // Render while the panel is open OR still animating closed
        // (detail_anim drives the fade; the flag flips instantly on close).
        if !self.ui.detail_open && self.ui.detail_anim <= 0.0 {
            return;
        }
        if self.ui.tab != Tab::Search {
            return;
        }
        let state = self.cur_state();
        if state != SearchState::Done {
            return;
        }
        let raw = self.all_results();
        let sorted = self.filtered(&raw);
        let page_s = self.page_slice(&sorted);
        // Cache the selected row so the fade-out animation still has content
        // after `selected` is cleared on close.
        if self.ui.detail_open {
            if let Some(idx) = self.ui.selected {
                if let Some(r) = page_s.get(idx).cloned() {
                    self.ui.detail_row = Some(r.clone());
                }
            }
        }
        let Some(r) = self.ui.detail_row.clone() else {
            return;
        };
        // Frame-level right panel (reserves space — table shrinks,
        // no overlap). Must be added BEFORE CentralPanel.
        let w = self.ui.detail_width.clamp(240.0, 520.0);
        egui::Panel::right("detail_pnl")
            .resizable(true)
            .default_size(w)
            .size_range(220.0..=640.0)
            .frame(
                egui::Frame::NONE
                    .fill(self.pal.surface)
                    .stroke(Stroke::new(1.0_f32, self.pal.border))
                    .inner_margin(egui::Margin::symmetric(PANEL_MARGIN_X, 8)),
            )
            .show(ui, |ui| {
                // Fade + slide in the panel content (from the right edge).
                // detail_anim is driven in main.rs toward 0/1 on open/close.
                let anim = self.ui.detail_anim;
                let eased = (self.tokens.easing)(anim);
                ui.set_opacity(eased);
                ui.add_space((1.0 - eased) * 8.0); // slight downward settle
                egui::ScrollArea::vertical().show(ui, |ui| self.draw_detail(ui, &r));
            });
    }
    pub(crate) fn draw_history_dropdown(
        &mut self,
        ctx: &egui::Context,
        bar_rect: egui::Rect,
        fs: f32,
    ) {
        if self.ui.show_hist && !self.cfg.history.is_empty() {
            let scr = ctx.input(|i| i.viewport_rect());
            // Clamp the dropdown to the viewport: never start left of the
            // window and never extend past the right edge (overflow bug at
            // narrow widths / when the search bar is scrolled).
            let w = bar_rect.width().max(280.0).min(scr.width() - 16.0);
            let x = bar_rect.min.x.clamp(8.0, (scr.max.x - w - 8.0).max(8.0));
            let pos = egui::pos2(x, bar_rect.max.y + 4.0);
            let hist = self.cfg.history.clone();
            let mut clicked: Option<String> = None;
            let mut deleted: Option<String> = None;
            let mut clear_all = false;

            egui::Area::new(egui::Id::new("hist_dd"))
                .fixed_pos(pos)
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::NONE
                        .fill(self.pal.surface)
                        .corner_radius(PANEL_RADIUS)
                        .stroke(Stroke::new(1.0_f32, self.pal.accent))
                        .shadow(egui::epaint::Shadow {
                            offset: [0, 4],
                            blur: 12,
                            spread: 0,
                            color: rgba(0, 0, 0, 70),
                        })
                        .inner_margin(egui::Margin::symmetric(10, 8))
                        .show(ui, |ui| {
                            ui.set_width(w);
                            ui.horizontal_wrapped(|ui| {
                                lbl(ui, "Recent searches", self.pal.dim, 11.0);
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .add(
                                                egui::Button::new(
                                                    RichText::new("clear all")
                                                        .size(11.0)
                                                        .color(self.pal.dim),
                                                )
                                                .fill(Color32::TRANSPARENT)
                                                .frame(false),
                                            )
                                            .clicked()
                                        {
                                            clear_all = true;
                                        }
                                    },
                                );
                            });
                            ui.add_space(4.0);
                            for h in hist.iter().take(10) {
                                ui.horizontal_wrapped(|ui| {
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                RichText::new(h.as_str())
                                                    .font(FontId::proportional(fs))
                                                    .color(self.pal.text),
                                            )
                                            .fill(Color32::TRANSPARENT)
                                            .frame(false)
                                            .min_size(egui::vec2((w - 50.0).max(120.0), 26.0)),
                                        )
                                        .clicked()
                                    {
                                        clicked = Some(h.clone());
                                    }
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui
                                                .add(
                                                    egui::Button::new(svg_image(
                                                        SvgIcon::Close,
                                                        10.0,
                                                        self.pal.dim,
                                                    ))
                                                    .fill(Color32::TRANSPARENT)
                                                    .frame(false)
                                                    .min_size(egui::vec2(18.0, 18.0)),
                                                )
                                                .on_hover_text("Remove")
                                                .clicked()
                                            {
                                                deleted = Some(h.clone());
                                            }
                                        },
                                    );
                                });
                            }
                        });
                });
            if let Some(h) = clicked {
                self.search.query = h;
                self.ui.show_hist = false;
                self.do_search();
            }
            if let Some(h) = deleted {
                self.cfg.history.retain(|x| x != &h);
                save_cfg(&self.cfg);
            }
            if clear_all {
                self.cfg.history.clear();
                save_cfg(&self.cfg);
                self.ui.show_hist = false;
            }
        }
    }

    pub(crate) fn draw_filter_bar(&mut self, ui: &mut egui::Ui, fs: f32) {
        egui::Frame::NONE
            .fill(self.pal.surface)
            .corner_radius(PANEL_RADIUS)
            .stroke(Stroke::new(1.0_f32, self.pal.border))
            .inner_margin(egui::Margin::symmetric(PANEL_MARGIN_X, 7))
            .outer_margin(egui::Margin::symmetric(PANEL_MARGIN_X, 0))
            .show(ui, |ui| {
                // Row 1
                ui.horizontal_wrapped(|ui| {
                    // Track whether any filter input changed this frame so we can
                    // drop batch selections whose indices went stale. TextEdit
                    // responses report .changed() — no per-frame string clones.
                    let mut filter_changed = false;
                    lbl(ui, "Filter", self.pal.dim, fs);
                    ui.add_space(3.0);
                    filter_changed |= ui
                        .add(
                            egui::TextEdit::singleline(&mut self.search.f_text)
                                .desired_width(FILTER_TEXT_W)
                                .hint_text("within results")
                                .font(FontId::proportional(fs)),
                        )
                        .changed();
                    ui.add_space(8.0);
                    lbl(ui, "Seeds ≥", self.pal.dim, fs);
                    filter_changed |= ui
                        .add(
                            egui::TextEdit::singleline(&mut self.search.f_seed)
                                .desired_width(FILTER_NUM_W)
                                .hint_text("0")
                                .font(FontId::proportional(fs)),
                        )
                        .changed();
                    ui.add_space(8.0);
                    lbl(ui, "Max GB", self.pal.dim, fs);
                    filter_changed |= ui
                        .add(
                            egui::TextEdit::singleline(&mut self.search.f_size)
                                .desired_width(FILTER_NUM_W)
                                .hint_text("∞")
                                .font(FontId::proportional(fs)),
                        )
                        .changed();
                    ui.add_space(8.0);
                    lbl(ui, "Year ≥", self.pal.dim, fs);
                    filter_changed |= ui
                        .add(
                            egui::TextEdit::singleline(&mut self.search.f_year)
                                .desired_width(FILTER_YEAR_W)
                                .hint_text("any")
                                .font(FontId::proportional(fs)),
                        )
                        .changed();
                    ui.add_space(8.0);
                    lbl(ui, "Tracker", self.pal.dim, fs);
                    filter_changed |= ui
                        .add(
                            egui::TextEdit::singleline(&mut self.search.f_trk)
                                .desired_width(FILTER_TRK_W)
                                .hint_text("any")
                                .font(FontId::proportional(fs)),
                        )
                        .changed();

                    if filter_changed && self.ui.sel_mode {
                        self.ui.sel_set.clear();
                        self.ui.sel_mode = false;
                        self.toast("Selection cleared (filters changed)", self.pal.yellow);
                    }

                    let dirty = !self.search.f_text.is_empty()
                        || !self.search.f_seed.is_empty()
                        || !self.search.f_size.is_empty()
                        || !self.search.f_year.is_empty()
                        || !self.search.f_trk.is_empty()
                        || self.search.f_hlth != Hlth::All;
                    if dirty {
                        ui.add_space(8.0);
                        if outline_icon_btn(ui, SvgIcon::Close, "Reset", self.pal.red) {
                            self.search.f_text.clear();
                            self.search.f_seed.clear();
                            self.search.f_size.clear();
                            self.search.f_year.clear();
                            self.search.f_trk.clear();
                            self.search.f_hlth = Hlth::All;
                            self.search.page = 0;
                            self.ui.sel_set.clear();
                            self.ui.sel_mode = false;
                        }
                    }
                });
                ui.add_space(5.0);
                // Row 2 — select mode + health + sort
                ui.horizontal_wrapped(|ui| {
                    // Batch select toggle (vector checkbox, no glyphs)
                    if v_checkbox(ui, self.ui.sel_mode, "Select", self.pal.accent).clicked() {
                        self.ui.sel_mode = !self.ui.sel_mode;
                        self.ui.sel_set.clear();
                        self.ui.detail_open = false;
                    }
                    if self.ui.sel_mode {
                        let n = self.ui.sel_set.len();
                        let copy_label =
                            format!("Copy {n} magnet{}", if n == 1 { "" } else { "s" });
                        if n > 0
                            && icon_text_btn(ui, SvgIcon::Copy, &copy_label, self.pal.green, true)
                        {
                            self.copy_selected_magnets(ui);
                        }
                        ui.add_space(4.0);
                        // Select all visible / clear (no glyphs)
                        let all_checked = !self.ui.sel_set.is_empty();
                        if v_checkbox(ui, all_checked, "All", self.pal.accent)
                            .on_hover_text("Select all results on this page")
                            .clicked()
                        {
                            let raw = self.all_results();
                            let sorted = self.filtered(&raw);
                            if self.cfg.page_size == 0 {
                                self.ui.sel_set = (0..sorted.len()).collect();
                            } else {
                                let base = self.search.page * self.cfg.page_size;
                                let end = (base + self.cfg.page_size).min(sorted.len());
                                self.ui.sel_set = (base..end).collect();
                            }
                        }
                        if !self.ui.sel_set.is_empty()
                            && ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("  Clear")
                                            .font(FontId::proportional(fs - 1.0))
                                            .color(self.pal.sub),
                                    )
                                    .fill(Color32::TRANSPARENT)
                                    .stroke(Stroke::new(1.0_f32, self.pal.border))
                                    .corner_radius(4.0),
                                )
                                .clicked()
                        {
                            self.ui.sel_set.clear();
                        }
                        ui.add_space(4.0);
                    }
                    ui.add_space(6.0);
                    lbl(ui, "Health", self.pal.dim, fs);
                    ui.add_space(4.0);
                    for hf in [Hlth::All, Hlth::Hot, Hlth::Good, Hlth::Slow, Hlth::Dead] {
                        let on = self.search.f_hlth == hf;
                        if ui
                            .add(egui::Button::selectable(
                                on,
                                RichText::new(hf.label())
                                    .font(FontId::proportional(fs - 1.0))
                                    .color(if on { self.pal.accent } else { self.pal.sub }),
                            ))
                            .clicked()
                        {
                            if self.search.f_hlth != hf {
                                self.search.f_hlth = hf;
                                self.ui.sel_set.clear();
                                self.ui.sel_mode = false; // filter changed
                            }
                            self.search.page = 0;
                        }
                        ui.add_space(2.0);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let desc = self.search.s_dir == SortDir::Desc;
                        let d_lbl = if desc { "DESC" } else { "ASC" };
                        let arrow = if desc {
                            SvgIcon::ArrowDown
                        } else {
                            SvgIcon::ArrowUp
                        };
                        let clicked = ui
                            .horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 3.0;
                                svg_icon(ui, arrow, 10.0, self.pal.accent);
                                ui.add(
                                    egui::Button::new(
                                        RichText::new(d_lbl)
                                            .font(FontId::proportional(fs - 1.0))
                                            .color(self.pal.accent),
                                    )
                                    .fill(Color32::TRANSPARENT)
                                    .frame(false),
                                )
                                .clicked()
                            })
                            .inner;
                        if clicked {
                            self.search.s_dir = if desc { SortDir::Asc } else { SortDir::Desc };
                            self.search.page = 0;
                        }
                        ui.add_space(6.0);
                        lbl(ui, "Sort:", self.pal.dim, fs);
                        ui.add_space(4.0);
                        for (l, col) in [
                            ("Date", SortCol::Date),
                            ("Size", SortCol::Size),
                            ("Leech", SortCol::Leech),
                            ("Seeds", SortCol::Seeds),
                            ("Ratio", SortCol::Ratio),
                            ("Tracker", SortCol::Tracker),
                            ("Name", SortCol::Name),
                        ] {
                            let on = self.search.s_col == col;
                            let col_c = if on { self.pal.accent } else { self.pal.sub };
                            let clicked = ui
                                .horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 3.0;
                                    let hit = ui
                                        .add(egui::Button::selectable(
                                            on,
                                            RichText::new(l)
                                                .font(FontId::proportional(fs - 1.0))
                                                .color(col_c),
                                        ))
                                        .clicked();
                                    if on {
                                        let arrow = if self.search.s_dir == SortDir::Desc {
                                            SvgIcon::ArrowDown
                                        } else {
                                            SvgIcon::ArrowUp
                                        };
                                        svg_icon(ui, arrow, 9.0, self.pal.accent);
                                    }
                                    hit
                                })
                                .inner;
                            if clicked {
                                if self.search.s_col == col {
                                    self.search.s_dir = if self.search.s_dir == SortDir::Desc {
                                        SortDir::Asc
                                    } else {
                                        SortDir::Desc
                                    };
                                } else {
                                    self.search.s_col = col;
                                    self.search.s_dir = SortDir::Desc;
                                }
                                self.search.page = 0;
                            }
                            ui.add_space(2.0);
                        }
                    });
                });
            });
    }
}
