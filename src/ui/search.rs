//! search drawing methods.

use super::*;

use crate::app::App;


impl App {

    pub(crate) fn draw_search(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, state: &SearchState) {
        let fs = self.cfg.font_size;
        let busy = *state == SearchState::Searching;
        ui.add_space(10.0);
        let mut bar_rect = egui::Rect::NOTHING;

        // Search input
        ui.horizontal_wrapped(|ui| {
            ui.add_space(12.0);
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .id(egui::Id::new("q"))
                    .desired_width((ui.available_width() - 310.0).max(160.0))
                    .hint_text("Search torrents — movies, shows, games, software, anime…")
                    .font(FontId::proportional(fs + 2.0))
            );
            bar_rect = resp.rect;
            if resp.gained_focus() && !self.cfg.history.is_empty() { self.show_hist = true; }
            if resp.changed() && self.query.is_empty() { self.show_hist = false; }
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) { self.do_search(); }

            ui.add_space(6.0);
            egui::ComboBox::from_id_salt("cat_cb")
                .selected_text(RichText::new(&self.cat).font(FontId::proportional(fs)))
                .width(115.0)
                .show_ui(ui, |ui| {
                    for &c in CATS {
                        ui.selectable_value(&mut self.cat, c.into(),
                            RichText::new(c).font(FontId::proportional(fs)));
                    }
                });

            // Indexer picker (from Jackett's configured indexers)
            ui.add_space(6.0);
            egui::ComboBox::from_id_salt("idx_cb")
                .selected_text(RichText::new(&self.indexer).font(FontId::proportional(fs)))
                .width(130.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.indexer, "All".into(),
                        RichText::new("All").font(FontId::proportional(fs)));
                    for idx in &self.indexers {
                        ui.selectable_value(&mut self.indexer, idx.clone(),
                            RichText::new(idx).font(FontId::proportional(fs)));
                    }
                })
                .response
                .on_hover_text("Search one indexer instead of all");

            ui.add_space(6.0);
            if ui.add_enabled(!busy,
                egui::Button::new(
                    RichText::new(if busy { "  Scanning…  " } else { "    Search    " })
                        .font(FontId::proportional(fs)).strong().color(Color32::WHITE))
                    .fill(if busy { rgb(6,100,130) } else { self.pal.accent })
                    .corner_radius(6.0).min_size(Vec2::new(0.0, 36.0))
            ).clicked() { self.do_search(); }

            if !self.query.is_empty()
                && ui.add(egui::Button::new(svg_image(SvgIcon::Close, 14.0, self.pal.sub))
                    .fill(tint(self.pal.sub, 12))
                    .stroke(Stroke::new(1.0_f32, self.pal.border))
                    .corner_radius(6.0).min_size(Vec2::new(0.0, 36.0)))
                    .on_hover_text("Clear search").clicked() {
                    self.query.clear(); self.show_hist = false;
                }
        });

        // History dropdown
        self.draw_history_dropdown(ctx, bar_rect, fs);

        ui.add_space(8.0);

        // Filter bar
        self.draw_filter_bar(ui, fs);

        ui.add_space(8.0);

        // State-dependent content
        match state {
            SearchState::Idle => self.draw_idle(ui),
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
                            self.show_settings = true;
                        }
                    });
            }
            SearchState::Done => {
                // Fire a desktop notification on transition (once per search).
                if !self.notified {
                    self.notified = true;
                    self.notify_search_done();
                }
                let raw = self.all_results();
                let sorted = self.filtered(&raw);
                let total = sorted.len();

                // Clamp selected index after filtering (page-local index space)
                let page_n = self.page_slice(&sorted).len();
                self.selected = self.selected.filter(|&i| i < page_n);
                if self.selected.is_none() {
                    self.detail_open = false;
                }

                if total == 0 {
                    ui.add_space(40.0);
                    ui.vertical_centered(|ui| {
                        lbl(ui, "No results match your filters", self.pal.sub, 17.0);
                        if !raw.is_empty() {
                            lbl(ui, &format!("{} results hidden by filters", raw.len()),
                                self.pal.dim, fs);
                        }
                    });
                    return;
                }

                let max_p = self.max_pages(total);
                if self.page >= max_p { self.page = max_p.saturating_sub(1); }
                let pg = self.page;
                let page_s = self.page_slice(&sorted).to_vec();
                let page_n = page_s.len();

                // Stats bar
                ui.horizontal_wrapped(|ui| {
                    ui.add_space(12.0);
                    let active: usize = sorted.iter().filter(|r| r.seeders.unwrap_or(0) > 0).count();
                    let seeds: u32 = sorted.iter().map(|r| r.seeders.unwrap_or(0)).sum();
                    let trackers: std::collections::HashSet<_> =
                        sorted.iter().filter_map(|r| r.tracker.as_deref()).collect();
                    lbl(ui, &format!("Showing {page_n} of {total}  ·  {active} active  ·  \
                                      {seeds} seeds  ·  {} trackers", trackers.len()),
                        self.pal.sub, fs - 1.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(12.0);
                        let sc = sorted.clone();
                        if outline_btn(ui, "Export CSV", self.pal.sub) {
                            self.export_csv(&sc);
                            self.toast("Exported to Downloads ✓", self.pal.green);
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
                                let sel = self.f_text == *cat;
                                egui::Frame::NONE
                                    .fill(tint(*col, if sel { 50 } else { 20 })).corner_radius(10.0)
                                    .stroke(Stroke::new(
                                        if sel { 1.5_f32 } else { 1.0_f32 },
                                        tint(*col, if sel { 200 } else { 80 })))
                                    .inner_margin(egui::Margin::symmetric(7, 2))
                                    .show(ui, |ui| {
                                        if ui.add(egui::Label::new(
                                            RichText::new(format!("{cat}  {count}"))
                                                .font(FontId::proportional(11.0)).color(*col)
                                        ).sense(egui::Sense::click()))
                                            .on_hover_text("Click to filter by category").clicked() {
                                            if self.f_text == *cat { self.f_text.clear(); }
                                            else { self.f_text = cat.clone(); }
                                            // Category filter changed the result set: drop batch selections.
                                            self.sel_set.clear(); self.sel_mode = false;
                                            self.page = 0;
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
                    self.selected = Some(self.selected.map_or(0, |s| (s + 1).min(page_n.saturating_sub(1))));
                    self.detail_open = true;
                }
                if !typing && ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                    self.selected = Some(self.selected.map_or(0, |s| s.saturating_sub(1)));
                    self.detail_open = true;
                }
                if !typing && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if let Some(i) = self.selected {
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
                if !typing && ui.input(|i| i.key_pressed(egui::Key::D))
                    && self.selected.is_some() { self.detail_open = !self.detail_open; }
                if !typing && ui.input(|i| i.key_pressed(egui::Key::F)) {
                    if let Some(i) = self.selected {
                        if let Some(r) = page_s.get(i).cloned() { self.add_fav(&r); }
                    }
                }
                if !typing && ui.input(|i| i.key_pressed(egui::Key::M)) {
                    if let Some(i) = self.selected {
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
                let base = if self.cfg.page_size == 0 { 0 } else { pg * self.cfg.page_size };
                self.draw_results_table(ui, &page_s, base);
            }
        }
    }

    /// Bottom pagination bar (Prev / page numbers / Next). Shown only when
    /// there's more than one page. Mutates `self.page` and clears selection.
    fn draw_pagination(&mut self, ui: &mut egui::Ui, max_p: usize, pg: usize, fs: f32) {
        if max_p <= 1 { return; }
        egui::Panel::bottom("pages")
            .default_size(34.0)
            .frame(egui::Frame::NONE.fill(self.pal.bg)
                .stroke(Stroke::new(1.0_f32, self.pal.border))
                .inner_margin(egui::Margin::symmetric(PANEL_MARGIN_X, 5)))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(pg > 0,
                        egui::Button::new(RichText::new("← Prev")
                            .font(FontId::proportional(fs - 1.0)).color(self.pal.sub))
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::new(1.0_f32, self.pal.border)).corner_radius(4.0)
                    ).clicked() { self.page -= 1; self.selected = None; }
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
                        if ui.add(egui::Button::selectable(on,
                            RichText::new(format!("{}", p + 1))
                                .font(FontId::proportional(fs - 1.0))
                                .color(if on { self.pal.accent } else { self.pal.sub })
                        )).clicked() { self.page = p; self.selected = None; }
                    }
                    ui.add_space(6.0);
                    if ui.add_enabled(pg + 1 < max_p,
                        egui::Button::new(RichText::new("Next →")
                            .font(FontId::proportional(fs - 1.0)).color(self.pal.sub))
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::new(1.0_f32, self.pal.border)).corner_radius(4.0)
                    ).clicked() { self.page += 1; self.selected = None; }
                    lbl(ui, &format!("  Page {} of {max_p}", pg + 1), self.pal.dim, fs - 1.0);
                });
            });
    }

    pub(crate) fn draw_detail_panel(&mut self, ui: &mut egui::Ui) {
        if !self.detail_open || self.tab != Tab::Search { return; }
        let state = self.cur_state();
        if state != SearchState::Done { return; }
        let raw = self.all_results();
        let sorted = self.filtered(&raw);
        let page_s = self.page_slice(&sorted);
        if let Some(idx) = self.selected {
            if let Some(r) = page_s.get(idx).cloned() {
                // Frame-level right panel (reserves space — table shrinks,
                // no overlap). Must be added BEFORE CentralPanel.
                let w = self.detail_width.clamp(240.0, 520.0);
                egui::Panel::right("detail_pnl")
                    .resizable(true)
                    .default_size(w)
                    .size_range(220.0..=640.0)
                    .frame(egui::Frame::NONE
                        .fill(self.pal.surface)
                        .stroke(Stroke::new(1.0_f32, self.pal.border))
                        .inner_margin(egui::Margin::symmetric(PANEL_MARGIN_X, 8)))
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| self.draw_detail(ui, &r));
                    });
            }
        }
    }
    pub(crate) fn draw_history_dropdown(&mut self, ctx: &egui::Context, bar_rect: egui::Rect, fs: f32) {
        if self.show_hist && !self.cfg.history.is_empty() {
            let pos = egui::pos2(bar_rect.min.x, bar_rect.max.y + 4.0);
            let w = bar_rect.width();
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
                            ui.set_width(w.max(280.0));
                            ui.horizontal_wrapped(|ui| {
                                lbl(ui, "Recent searches", self.pal.dim, 11.0);
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.add(egui::Button::new(
                                        RichText::new("clear all").size(11.0).color(self.pal.dim))
                                        .fill(Color32::TRANSPARENT).frame(false)).clicked() {
                                        clear_all = true;
                                    }
                                });
                            });
                            ui.add_space(4.0);
                            for h in hist.iter().take(10) {
                                ui.horizontal_wrapped(|ui| {
                                    if ui.add(egui::Button::new(
                                        RichText::new(h.as_str()).font(FontId::proportional(fs))
                                            .color(self.pal.text))
                                        .fill(Color32::TRANSPARENT).frame(false)
                                        .min_size(egui::vec2(w.max(280.0) - 50.0, 26.0))
                                    ).clicked() { clicked = Some(h.clone()); }
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.add(egui::Button::new(svg_image(SvgIcon::Close, 10.0, self.pal.dim))
                                            .fill(Color32::TRANSPARENT).frame(false)
                                            .min_size(egui::vec2(18.0, 18.0))
                                        ).on_hover_text("Remove").clicked() {
                                            deleted = Some(h.clone());
                                        }
                                    });
                                });
                            }
                        });
                });
            if let Some(h) = clicked { self.query = h; self.show_hist = false; self.do_search(); }
            if let Some(h) = deleted { self.cfg.history.retain(|x| x != &h); save_cfg(&self.cfg); }
            if clear_all { self.cfg.history.clear(); save_cfg(&self.cfg); self.show_hist = false; }
        }
    }

    pub(crate) fn draw_filter_bar(&mut self, ui: &mut egui::Ui, fs: f32) {
        egui::Frame::NONE
            .fill(self.pal.surface).corner_radius(PANEL_RADIUS)
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
                    filter_changed |= ui.add(egui::TextEdit::singleline(&mut self.f_text)
                        .desired_width(FILTER_TEXT_W).hint_text("within results")
                        .font(FontId::proportional(fs))).changed();
                    ui.add_space(8.0);
                    lbl(ui, "Seeds ≥", self.pal.dim, fs);
                    filter_changed |= ui.add(egui::TextEdit::singleline(&mut self.f_seed)
                        .desired_width(FILTER_NUM_W).hint_text("0").font(FontId::proportional(fs))).changed();
                    ui.add_space(8.0);
                    lbl(ui, "Max GB", self.pal.dim, fs);
                    filter_changed |= ui.add(egui::TextEdit::singleline(&mut self.f_size)
                        .desired_width(FILTER_NUM_W).hint_text("∞").font(FontId::proportional(fs))).changed();
                    ui.add_space(8.0);
                    lbl(ui, "Year ≥", self.pal.dim, fs);
                    filter_changed |= ui.add(egui::TextEdit::singleline(&mut self.f_year)
                        .desired_width(FILTER_YEAR_W).hint_text("any").font(FontId::proportional(fs))).changed();
                    ui.add_space(8.0);
                    lbl(ui, "Tracker", self.pal.dim, fs);
                    filter_changed |= ui.add(egui::TextEdit::singleline(&mut self.f_trk)
                        .desired_width(FILTER_TRK_W).hint_text("any").font(FontId::proportional(fs))).changed();

                    if filter_changed && self.sel_mode {
                        self.sel_set.clear(); self.sel_mode = false;
                        self.toast("Selection cleared (filters changed)", self.pal.yellow);
                    }

                    let dirty = !self.f_text.is_empty() || !self.f_seed.is_empty()
                        || !self.f_size.is_empty() || !self.f_year.is_empty()
                        || !self.f_trk.is_empty() || self.f_hlth != Hlth::All;
                    if dirty {
                        ui.add_space(8.0);
                        if outline_icon_btn(ui, SvgIcon::Close, "Reset", self.pal.red) {
                            self.f_text.clear(); self.f_seed.clear(); self.f_size.clear();
                            self.f_year.clear(); self.f_trk.clear(); self.f_hlth = Hlth::All;
                            self.page = 0;
                            self.sel_set.clear(); self.sel_mode = false;
                        }
                    }
                });
                ui.add_space(5.0);
                // Row 2 — select mode + health + sort
                ui.horizontal_wrapped(|ui| {
                    // Batch select toggle (vector checkbox, no glyphs)
                    if v_checkbox(ui, self.sel_mode, "Select", self.pal.accent).clicked() {
                        self.sel_mode = !self.sel_mode;
                        self.sel_set.clear();
                        self.detail_open = false;
                    }
                    if self.sel_mode {
                        let n = self.sel_set.len();
                        if n > 0 && ui.add(egui::Button::new(
                            RichText::new(format!("⧉ Copy {n} magnet{}", if n == 1 { "" } else { "s" }))
                                .font(FontId::proportional(fs - 1.0)).color(self.pal.green))
                            .fill(tint(self.pal.green, 14))
                            .stroke(Stroke::new(1.0_f32, tint(self.pal.green, 60)))
                            .corner_radius(4.0)
                        ).clicked() {
                            self.copy_selected_magnets(ui);
                        }
                        ui.add_space(4.0);
                        // Select all visible / clear (no glyphs)
                        let all_checked = !self.sel_set.is_empty();
                        if v_checkbox(ui, all_checked, "All", self.pal.accent).on_hover_text("Select all results on this page").clicked() {
                            let raw = self.all_results();
                            let sorted = self.filtered(&raw);
                            if self.cfg.page_size == 0 {
                                self.sel_set = (0..sorted.len()).collect();
                            } else {
                                let base = self.page * self.cfg.page_size;
                                let end = (base + self.cfg.page_size).min(sorted.len());
                                self.sel_set = (base..end).collect();
                            }
                        }
                        if !self.sel_set.is_empty() && ui.add(egui::Button::new(
                            RichText::new("  Clear").font(FontId::proportional(fs - 1.0)).color(self.pal.sub))
                            .fill(Color32::TRANSPARENT).stroke(Stroke::new(1.0_f32, self.pal.border))
                            .corner_radius(4.0)
                        ).clicked() {
                            self.sel_set.clear();
                        }
                        ui.add_space(4.0);
                    }
                    ui.add_space(6.0);
                    lbl(ui, "Health", self.pal.dim, fs);
                    ui.add_space(4.0);
                    for hf in [Hlth::All, Hlth::Hot, Hlth::Good, Hlth::Slow, Hlth::Dead] {
                        let on = self.f_hlth == hf;
                        if ui.add(egui::Button::selectable(on,
                            RichText::new(hf.label()).font(FontId::proportional(fs - 1.0))
                                .color(if on { self.pal.accent } else { self.pal.sub })
                        )).clicked() {
                            if self.f_hlth != hf {
                                self.f_hlth = hf;
                                self.sel_set.clear(); self.sel_mode = false; // filter changed
                            }
                            self.page = 0;
                        }
                        ui.add_space(2.0);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let d_lbl = if self.s_dir == SortDir::Desc { "▼ DESC" } else { "▲ ASC" };
                        if ui.add(egui::Button::new(
                            RichText::new(d_lbl).font(FontId::proportional(fs - 1.0)).color(self.pal.accent))
                            .fill(tint(self.pal.accent, 18))
                            .stroke(Stroke::new(1.0_f32, tint(self.pal.accent, 60))).corner_radius(4.0)
                        ).on_hover_text("Toggle sort direction").clicked() {
                            self.s_dir = if self.s_dir == SortDir::Desc { SortDir::Asc } else { SortDir::Desc };
                            self.page = 0;
                        }
                        ui.add_space(6.0);
                        lbl(ui, "Sort:", self.pal.dim, fs);
                        ui.add_space(4.0);
                        for (l, col) in [("Date", SortCol::Date), ("Size", SortCol::Size),
                                         ("Leech", SortCol::Leech), ("Seeds", SortCol::Seeds),
                                         ("Ratio", SortCol::Ratio), ("Tracker", SortCol::Tracker),
                                         ("Name", SortCol::Name)] {
                            let on = self.s_col == col;
                            let txt = if on {
                                if self.s_dir == SortDir::Desc { format!("{l}▼") } else { format!("{l}▲") }
                            } else { l.to_string() };
                            if ui.add(egui::Button::selectable(on,
                                RichText::new(&txt).font(FontId::proportional(fs - 1.0))
                                    .color(if on { self.pal.accent } else { self.pal.sub })
                            )).clicked() {
                                if self.s_col == col {
                                    self.s_dir = if self.s_dir == SortDir::Desc { SortDir::Asc } else { SortDir::Desc };
                                } else { self.s_col = col; self.s_dir = SortDir::Desc; }
                                self.page = 0;
                            }
                            ui.add_space(2.0);
                        }
                    });
                });
            });
    }

    pub(crate) fn draw_results_table(&mut self, ui: &mut egui::Ui, page_s: &[TorrentResult], base: usize) {
        let mut actions: Vec<(usize, &'static str)> = vec![];
        let pal = self.pal.clone();
        let s_col = self.s_col.clone();
        let s_dir = self.s_dir.clone();
        let rh = self.cfg.row_height;
        let fsz = self.cfg.font_size;
        let cfg = self.cfg.clone();
        let sel = self.selected;
        let det_open = self.detail_open;

        let mut new_sort: Option<(SortCol, bool)> = None;

        // Table header helper
        let hdr = |l: &str, col: &SortCol| {
            let on = &s_col == col;
            let arr = if on { if s_dir == SortDir::Desc { "▼" } else { "▲" } } else { "" };
            RichText::new(format!("{l}{arr}")).font(FontId::proportional(fsz))
                .color(if on { pal.accent } else { pal.sub }).strong()
        };

        // Visible columns in user-configured order
        let cols: Vec<TableCol> = self.cfg.col_order.iter()
            .filter_map(|n| TableCol::from_name(n))
            .filter(|c| match c {
                TableCol::Name | TableCol::Seeds => true,
                TableCol::Tracker => cfg.col_tracker,
                TableCol::Size => cfg.col_size,
                TableCol::Leech => cfg.col_leech,
                TableCol::Ratio => cfg.col_ratio,
                TableCol::Health => cfg.col_health,
                TableCol::Date => cfg.col_date,
            })
            .collect();

        let mut tb = TableBuilder::new(ui)
            .striped(false)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center));
        // The Name column is the flexible one (absorbs window slack); all
        // other data columns are fixed-width. Actions is a normal resizable
        // column — NOT remainder — because egui pins a trailing remainder
        // column to fill all leftover space, which makes its separator
        // undraggable once moved (the stuck-drag bug).
        for c in &cols {
            if *c == TableCol::Name {
                tb = tb.column(Column::remainder().at_least(295.0));
            } else {
                tb = tb.column(Column::initial(c.width()).at_least(44.0));
            }
        }
        tb = tb.column(Column::initial(210.0)); // Actions always (resizable)
        tb
            .header(30.0, |mut header| {
                for c in &cols {
                    let sortcol = match c {
                        TableCol::Name => SortCol::Name,
                        TableCol::Tracker => SortCol::Tracker,
                        TableCol::Size => SortCol::Size,
                        TableCol::Seeds => SortCol::Seeds,
                        TableCol::Leech => SortCol::Leech,
                        TableCol::Ratio => SortCol::Ratio,
                        TableCol::Date => SortCol::Date,
                        TableCol::Health => SortCol::Seeds, // non-sortable; reuse Seeds
                    };
                    header.col(|ui| {
                        if *c == TableCol::Health {
                            ui.label(RichText::new("Health").font(FontId::proportional(fsz)).color(pal.sub).strong());
                        } else if ui.add(egui::Label::new(hdr(c.label(), &sortcol.clone())).sense(egui::Sense::click())).clicked() {
                            new_sort = Some((sortcol.clone(), s_col == sortcol));
                        }
                    });
                }
                header.col(|ui| {
                    ui.label(RichText::new("Actions").font(FontId::proportional(fsz)).color(pal.sub).strong());
                });
            })
            .body(|mut body| {
                for (i, r) in page_s.iter().enumerate() {
                    let gi = base + i; // global index into the filtered results
                    let is_sel = sel == Some(i);
                    let is_hov = self.hovered == Some(i);
                    let seed = r.seeders.unwrap_or(0);
                    let leech = r.peers.unwrap_or(0);
                    let bg = if is_sel { pal.row_sel }
                             else if is_hov { pal.row_hov }
                             else if i % 2 == 0 { pal.row_odd }
                             else { pal.row_even };

                    body.row(rh, |mut row| {
                        for c in &cols {
                            row.col(|ui| {
                                ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                                match c {
                                    TableCol::Name => {
                                        // Full-cell click layer — clicking ANYWHERE
                                        // in the Name cell (not just the text)
                                        // selects the row. Double-click opens magnet.
                                        let cell_id = egui::Id::new(("namecell", gi));
                                        let cell_resp = ui.interact(ui.max_rect(), cell_id, egui::Sense::click());
                                        if cell_resp.double_clicked() {
                                            if let Some(m) = r.magnet_uri.as_deref() {
                                                if is_magnet(m) {
                                                    let _ = open::that(m);
                                                    self.toast("Opening in torrent client…", self.pal.accent);
                                                } else {
                                                    self.toast("Invalid magnet link", self.pal.yellow);
                                                }
                                            } else {
                                                self.toast("No magnet link", self.pal.yellow);
                                            }
                                        } else if cell_resp.clicked() {
                                            if self.sel_mode {
                                                if !self.sel_set.insert(gi) { self.sel_set.remove(&gi); }
                                            } else {
                                                actions.push((i, "select"));
                                            }
                                        }
                                        if cell_resp.hovered() {
                                            self.hovered = Some(i);
                                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                        }
                                        // Draw the content (non-interactive label)
                                        ui.horizontal(|ui| {
                                            ui.add_space(6.0);
                                            if self.sel_mode {
                                                let checked = self.sel_set.contains(&gi);
                                                if v_checkbox(ui, checked, "", self.pal.accent).clicked() {
                                                    if checked { self.sel_set.remove(&gi); } else { self.sel_set.insert(gi); }
                                                }
                                                ui.add_space(4.0);
                                            }
                                            ui.add(egui::Label::new(
                                                RichText::new(&r.title).font(FontId::proportional(fsz))
                                                    .color(if is_sel { pal.accent } else { pal.text })
                                            ).truncate());
                                        });
                                        if rh >= 40.0 {
                                            let cat = r.category_desc.as_deref().unwrap_or("Other");
                                            ui.add(egui::Label::new(RichText::new(cat)
                                                .font(FontId::proportional(fsz - 2.5))
                                                .color(cat_col(cat))).truncate());
                                        }
                                    }
                                    TableCol::Tracker | TableCol::Size | TableCol::Seeds
                                    | TableCol::Leech | TableCol::Ratio | TableCol::Health
                                    | TableCol::Date => {
                                        draw_cell_content(ui, c, r, seed, leech, fsz, &pal);
                                        // Click anywhere in these cells selects the row
                                        // (the Name cell handles its own label clicks).
                                        let cell_id = egui::Id::new(("rowcell", gi, c.label()));
                                        let cell_resp = ui.interact(ui.max_rect(), cell_id, egui::Sense::click());
                                        if cell_resp.clicked() && !self.sel_mode {
                                            actions.push((i, "select"));
                                        }
                                        if cell_resp.hovered() {
                                            self.hovered = Some(i);
                                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                        }
                                    }
                                }
                            });
                        }
                        // Actions — fixed (always-visible) icon buttons, right-aligned.
                        row.col(|ui| {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.add_space(4.0);
                                ui.spacing_mut().item_spacing.x = 5.0;
                                let has_mag = r.magnet_uri.as_deref().map(is_magnet).unwrap_or(false);
                                let has_link = r.link.is_some();
                                // All actions always visible (no hover-reveal).
                                if has_mag
                                    && svg_btn(ui, SvgIcon::Magnet, "Open in torrent client", pal.accent) { actions.push((i, "mag")); }
                                if has_mag
                                    && svg_btn(ui, SvgIcon::Copy, "Copy magnet link", pal.sub) { actions.push((i, "copy")); }
                                if has_link
                                    && svg_btn(ui, SvgIcon::Download, "Download .torrent", pal.green) { actions.push((i, "dl")); }
                                if svg_btn(ui, SvgIcon::Star, "Add to Favorites (F)", pal.yellow) { actions.push((i, "fav")); }
                                if svg_btn(ui, SvgIcon::Info,
                                    "Detail panel (D)",
                                    if is_sel && det_open { pal.accent } else { pal.sub }) { actions.push((i, "info")); }
                                let cell_id = egui::Id::new(("rowhov", gi));
                                let hover_resp = ui.interact(ui.max_rect(), cell_id, egui::Sense::hover());
                                if hover_resp.hovered() {
                                    self.hovered = Some(i);
                                }
                            });
                        });
                    });
                }
            });

        if let Some((col, same)) = new_sort {
            if same {
                self.s_dir = if self.s_dir == SortDir::Desc { SortDir::Asc } else { SortDir::Desc };
            } else { self.s_col = col; self.s_dir = SortDir::Desc; }
            self.page = 0;
        }

        // Process actions
        for (i, action) in actions {
            if action == "hover" { continue; } // already handled
            if let Some(r) = page_s.get(i).cloned() {
                match action {
                    "select" => {
                        if self.selected == Some(i) && self.detail_open {
                            self.selected = None; self.detail_open = false;
                        } else { self.selected = Some(i); self.detail_open = true; }
                    }
                    "mag" => { if let Some(m) = &r.magnet_uri { let _ = open::that(m); self.toast("Opening magnet…", self.pal.accent); } }
                    "copy" => { if let Some(m) = &r.magnet_uri { ui.ctx().copy_text(m.clone()); self.toast("Magnet copied ✓", self.pal.green); } }
                    "dl" => { if let Some(l) = &r.link { let _ = open::that(l); self.toast("Downloading…", self.pal.green); } }
                    "fav" => { self.add_fav(&r); }
                    "info" => {
                        // Idempotent open: clicking Info always opens the detail
                        // panel for this row. (The row's own click may have already
                        // set selected+detail_open — don't toggle it closed here,
                        // or the button would flash-open/close in the same frame.)
                        self.selected = Some(i); self.detail_open = true;
                    }
                    "web" => { if let Some(d) = &r.details { let _ = open::that(d); } }
                    _ => {}
                }
            }
        }

        // Clear hover when mouse leaves the table area
        if let Some(hover_pos) = ui.ctx().pointer_hover_pos() {
            if !ui.min_rect().contains(hover_pos) {
                self.hovered = None;
            }
        } else {
            self.hovered = None;
        }
    }


    // ─── Idle / welcome ────────────────────────────────────────────────────

    pub(crate) fn draw_idle(&mut self, ui: &mut egui::Ui) {
        let fs = self.cfg.font_size;
        ui.add_space(50.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("TorrentX")
                .font(FontId::proportional(40.0)).strong().color(tint(self.pal.accent, 90)));
            ui.add_space(6.0);
            lbl(ui, "Search all your Jackett indexers in one shot", self.pal.sub, fs + 1.0);
            ui.add_space(3.0);
            lbl(ui, "Movies  ·  TV  ·  Music  ·  Games  ·  Software  ·  Anime  ·  Books",
                self.pal.dim, fs - 1.0);
            ui.add_space(32.0);

            if !self.cfg.history.is_empty() {
                lbl(ui, "Recent searches", self.pal.dim, fs - 1.0);
                ui.add_space(10.0);
                let hist: Vec<String> = self.cfg.history.iter().take(12).cloned().collect();
                let mut clicked: Option<String> = None;
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                    for h in &hist {
                        if ui.add(egui::Button::new(
                            RichText::new(h.as_str()).font(FontId::proportional(fs)).color(self.pal.sub))
                            .fill(self.pal.surface)
                            .stroke(Stroke::new(1.0_f32, self.pal.border))
                            .corner_radius(14.0).min_size(egui::vec2(0.0, 28.0))
                        ).clicked() { clicked = Some(h.clone()); }
                    }
                });
                if let Some(h) = clicked { self.query = h; self.do_search(); }
            } else {
                lbl(ui, "Try searching:", self.pal.dim, fs - 1.0);
                ui.add_space(10.0);
                let suggestions = ["Linux Mint", "Ubuntu 24.04", "Blender", "GIMP", "Inkscape"];
                let mut clicked: Option<&str> = None;
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                    for s in &suggestions {
                        if ui.add(egui::Button::new(
                            RichText::new(*s).font(FontId::proportional(fs)).color(self.pal.dim))
                            .fill(self.pal.surface)
                            .stroke(Stroke::new(1.0_f32, tint(self.pal.border, 140)))
                            .corner_radius(14.0).min_size(egui::vec2(0.0, 28.0))
                        ).clicked() { clicked = Some(s); }
                    }
                });
                if let Some(s) = clicked { self.query = s.to_string(); self.do_search(); }

                ui.add_space(32.0);
                egui::Frame::NONE
                    .fill(tint(self.pal.accent, 12)).corner_radius(10.0)
                    .stroke(Stroke::new(1.0_f32, tint(self.pal.accent, 50)))
                    .inner_margin(egui::Margin::symmetric(24, 16))
                    .show(ui, |ui| {
                        ui.set_max_width(480.0);
                        ui.label(RichText::new("First time?")
                            .font(FontId::proportional(fs + 1.0)).color(self.pal.accent).strong());
                        ui.add_space(6.0);
                        lbl(ui, "1. Make sure Jackett is running  (localhost:9117)", self.pal.sub, fs - 1.0);
                        lbl(ui, "2. Click ⚙ Settings and paste your API key", self.pal.sub, fs - 1.0);
                        lbl(ui, "3. Search for anything!", self.pal.sub, fs - 1.0);
                        ui.add_space(10.0);
                        if outline_btn(ui, "Open Settings", self.pal.accent) {
                            self.show_settings = true;
                        }
                    });
            }
        });
    }

    // ─── Detail panel ──────────────────────────────────────────────────────

    pub(crate) fn draw_detail(&mut self, ui: &mut egui::Ui, r: &TorrentResult) {
        let fs = self.cfg.font_size;
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            lbl(ui, "Details", self.pal.text, fs + 2.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                if ui.add(egui::Button::new(svg_image(SvgIcon::Close, 14.0, self.pal.sub))
                    .fill(Color32::TRANSPARENT).corner_radius(4.0))
                    .on_hover_text("Close").clicked() {
                    self.detail_open = false; self.selected = None;
                }
            });
        });
        ui.separator();
        ui.add_space(8.0);

        egui::ScrollArea::vertical().id_salt("det_scr").show(ui, |ui| {
            ui.add(egui::Label::new(
                RichText::new(&r.title).font(FontId::proportional(fs)).color(self.pal.text).strong()
            ).wrap());
            ui.add_space(8.0);

            let cat = r.category_desc.as_deref().unwrap_or("Unknown");
            egui::Frame::NONE
                .fill(tint(cat_col(cat), 25)).corner_radius(PANEL_RADIUS)
                .inner_margin(egui::Margin::symmetric(8, 3))
                .show(ui, |ui| {
                    ui.label(RichText::new(cat).font(FontId::proportional(fs - 1.0)).color(cat_col(cat)));
                });
            ui.add_space(12.0);

            // Use grid for aligned details
            egui::Grid::new("detail_grid")
                .num_columns(2)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    let seed = r.seeders.unwrap_or(0);
                    let leech = r.peers.unwrap_or(0);

                    if let Some(t) = &r.tracker { grid_row(ui, "Tracker", t, self.pal.sub, &self.pal, fs); }
                    if let Some(s) = r.size { grid_row(ui, "Size", &fmt_size(s), self.pal.sub, &self.pal, fs); }
                    grid_row(ui, "Seeders", &seed.to_string(), seed_col(seed), &self.pal, fs);
                    grid_row(ui, "Leechers", &leech.to_string(), self.pal.red, &self.pal, fs);
                    let ratio = if leech > 0 { format!("{:.2}", seed as f64 / leech as f64) } else { "∞".into() };
                    grid_row(ui, "Ratio", &ratio, self.pal.sub, &self.pal, fs);
                    grid_row(ui, "Health", hlth_lbl(seed), seed_col(seed), &self.pal, fs);
                    if let Some(d) = &r.publish_date { grid_row(ui, "Published", &time_ago(d), self.pal.dim, &self.pal, fs); }
                });

            let seed = r.seeders.unwrap_or(0);
            let leech = r.peers.unwrap_or(0);
            let tot = (seed + leech) as f32;
            if tot > 0.0 {
                let ratio_value = if leech > 0 {
                    format!("{:.2}", seed as f64 / leech as f64)
                } else {
                    "∞".into()
                };
                let pct = (seed as f32 / tot).clamp(0.0, 1.0);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("Ratio: {}  ", ratio_value))
                        .font(FontId::proportional(fs - 1.0)).color(self.pal.sub));
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width() - 60.0, 8.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 4.0, self.pal.border);
                    let mut filled = rect;
                    filled.max.x = rect.min.x + rect.width() * pct;
                    ui.painter().rect_filled(filled, 4.0, seed_col(seed));
                });
                ui.add_space(2.0);
                lbl(ui, &format!("{:.0}% seeded", pct * 100.0), self.pal.dim, fs - 2.0);
            }

            // Show magnet link (truncated) if present
            if let Some(mag) = &r.magnet_uri {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Magnet:").font(FontId::proportional(fs-1.5)).color(self.pal.dim));
                    let truncated = if mag.len() > 60 {
                        format!("{}…", &mag[..57])
                    } else {
                        mag.clone()
                    };
                    let resp = ui.add(egui::Label::new(
                        RichText::new(truncated).font(FontId::monospace(fs-2.0)).color(self.pal.sub))
                        .sense(egui::Sense::click()));
                    if resp.on_hover_text("Click to copy full magnet").clicked() {
                        ui.ctx().copy_text(mag.clone());
                        self.toast("Magnet copied ✓", self.pal.green);
                    }
                });
            }

            ui.add_space(16.0);
            lbl(ui, "Actions", self.pal.dim, fs - 1.0);
            ui.add_space(6.0);

            if let Some(mag) = r.magnet_uri.clone() {
                let mc = mag.clone();
                if wide_icon_btn(ui, SvgIcon::Magnet, "Open Magnet", self.pal.accent) {
                    let _ = open::that(mag); self.toast("Opening magnet…", self.pal.accent);
                }
                ui.add_space(4.0);
                if wide_icon_btn(ui, SvgIcon::Copy, "Copy Magnet Link", self.pal.sub) {
                    ui.ctx().copy_text(mc);
                    self.toast("Copied ✓", self.pal.green);
                }
                ui.add_space(4.0);
            }
            if let Some(link) = r.link.clone() {
                if wide_icon_btn(ui, SvgIcon::Download, "Download .torrent", self.pal.green) {
                    let _ = open::that(link); self.toast("Downloading…", self.pal.green);
                }
                ui.add_space(4.0);
            }
            if let Some(det) = r.details.clone() {
                if wide_icon_btn(ui, SvgIcon::Web, "Open in Browser", self.pal.sub) { let _ = open::that(det); }
                ui.add_space(4.0);
            }
            let r2 = r.clone();
            if wide_icon_btn(ui, SvgIcon::Star, "Add to Favorites", self.pal.yellow) { self.add_fav(&r2); }
        });
    }

    // ─── Favorites tab ─────────────────────────────────────────────────────

}

