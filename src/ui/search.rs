//! search drawing methods.

use super::*;

use crate::app::App;

impl App {
    pub(crate) fn draw_search(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        state: &SearchState,
        view: Option<&crate::app::ResultsView>,
    ) {
        let fs = self.cfg.font_size;
        let busy = *state == SearchState::Searching;
        ui.add_space(SP4);
        let mut bar_rect = egui::Rect::NOTHING;

        // Search input
        ui.horizontal_wrapped(|ui| {
            ui.add_space(SP4);
            // Field shrinks with the window; floor at 120 so the combos +
            // Search button can wrap onto a second row instead of clipping.
            // Combos + labels + button take ~470px; the field takes the rest so
            // the cluster ends at the window edge instead of stranding it mid-row.
            // Measured widths: Category label+combo ~165, Indexer label+combo
            // ~162, Search button ~130, gaps ~24 -> 480 plus slack, so the row
            // keeps one line and the button ends at the window edge.
            let field_w = (ui.available_width() - 505.0).max(140.0);
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
            if resp.lost_focus() {
                // Dismiss the recent-searches popup when the field loses focus:
                // otherwise it stays on screen over the filter bar, and its entry
                // rows are topmost there — so a click meant for a filter silently
                // re-runs that history entry instead.
                self.ui.show_hist = false;
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.do_search();
                }
            }

            ui.add_space(SP3);
            lbl(ui, "Category", self.pal.dim, 10.0);
            ui.add_space(SP1);
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
            ui.add_space(SP3);
            lbl(ui, "Indexer", self.pal.dim, 10.0);
            ui.add_space(SP1);
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

            ui.add_space(SP3);
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
                    .corner_radius(R_MD)
                    .min_size(Vec2::new(0.0, 36.0)),
                )
                .clicked()
            {
                self.do_search();
            }

            if !self.search.query.is_empty()
                && ui
                    .add(
                        egui::Button::new(svg_image(SvgIcon::Close, ICON_MD, self.pal.sub))
                            .fill(tint(self.pal.sub, 12))
                            .stroke(Stroke::new(1.0_f32, self.pal.border))
                            .corner_radius(R_MD)
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

        ui.add_space(SP3);

        // Filter bar
        self.draw_filter_bar(ui, fs, view);

        ui.add_space(SP3);

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
                match view.filter(|v| !v.sorted.is_empty()) {
                    // Results arrive one indexer at a time: show and page them
                    // immediately instead of hiding the table behind a spinner.
                    Some(v) => {
                        // No banner row: the status bar already names the query and
                        // the live count, and skipping it avoids a layout jump when
                        // the last indexer lands.
                        let max_p = self.max_pages(v.sorted.len());
                        let pg = self.search.page.min(max_p.saturating_sub(1));
                        let page_s = self.page_slice(&v.sorted);
                        let base = if self.cfg.page_size == 0 {
                            0
                        } else {
                            pg * self.cfg.page_size
                        };
                        self.draw_pagination(ui, max_p, pg, fs);
                        self.draw_results_table(ui, page_s, base);
                    }
                    None => {
                        ui.add_space(SP7 * 2.0);
                        ui.vertical_centered(|ui| {
                            ui.spinner();
                            ui.add_space(SP4);
                            lbl(ui, "Scanning Jackett indexers…", self.pal.sub, 16.0);
                            ui.add_space(SP2);
                            lbl(
                                ui,
                                "Results appear as each tracker answers",
                                self.pal.dim,
                                fs,
                            );
                        });
                    }
                }
            }
            SearchState::Error(err) => {
                ui.add_space(SP4);
                egui::Frame::NONE
                    .fill(tint(self.pal.red, 10))
                    .stroke(Stroke::new(1.0_f32, tint(self.pal.red, 70)))
                    .corner_radius(PANEL_RADIUS)
                    .inner_margin(margin(SP6, SP5))
                    .outer_margin(margin(PANEL_MARGIN_X as f32, 0.0))
                    .show(ui, |ui| {
                        for line in err.lines() {
                            lbl(ui, line, self.pal.red, fs);
                        }
                        ui.add_space(SP3);
                        if outline_btn(ui, "Open Settings", self.pal.accent) {
                            self.set_settings_open(true);
                        }
                    });
            }
            SearchState::Done => {
                let Some(view) = view else {
                    return;
                };
                let sorted = &view.sorted;
                // Fire a desktop notification when the search completes while the
                // window is NOT focused — that is the only case where it is useful
                // (the helper's contract), and unconditional firing spams the
                // desktop on every search.
                if !self.ui.notified {
                    self.ui.notified = true;
                    if !ui.input(|i| i.viewport().focused.unwrap_or(true)) {
                        self.notify_search_done();
                    }
                }
                let total = sorted.len();

                // Clamp selected index after filtering (page-local index space)
                let page_n = self.page_slice(sorted).len();
                self.ui.selected = self.ui.selected.filter(|&i| i < page_n);
                if self.ui.selected.is_none() {
                    self.ui.detail_open = false;
                }

                if total == 0 {
                    ui.add_space(SP7);
                    ui.vertical_centered(|ui| {
                        lbl(ui, "No results match your filters", self.pal.sub, 17.0);
                        if view.raw_total > 0 {
                            lbl(
                                ui,
                                &format!("{} results hidden by filters", view.raw_total),
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
                let page_s = self.page_slice(sorted);
                let page_n = page_s.len();

                // Stats bar
                ui.horizontal_wrapped(|ui| {
                    ui.add_space(SP4);
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
                        ui.add_space(SP4);
                        if outline_btn(ui, "Export CSV", self.pal.sub) {
                            self.export_csv(sorted);
                        }
                    });
                });

                // Category chips
                if self.cfg.show_cat_bar {
                    let chips = App::cat_chips(sorted);
                    if !chips.is_empty() {
                        ui.add_space(SP2);
                        ui.horizontal_wrapped(|ui| {
                            ui.add_space(SP4);
                            for (cat, count, col) in &chips {
                                let sel = self.search.f_text == *cat;
                                egui::Frame::NONE
                                    .fill(tint(*col, if sel { 50 } else { 20 }))
                                    .corner_radius(R_LG)
                                    .stroke(Stroke::new(
                                        if sel { 1.5_f32 } else { 1.0_f32 },
                                        tint(*col, if sel { 200 } else { 80 }),
                                    ))
                                    .inner_margin(margin(SP3, SP1))
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
                                ui.add_space(SP1);
                            }
                        });
                    }
                }
                ui.add_space(SP2);

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
                                    let _ = safe_open(m);
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
                                    let _ = safe_open(m);
                                    self.toast("Opening magnet…", self.pal.accent);
                                } else {
                                    self.toast("No valid magnet link", self.pal.yellow);
                                }
                            }
                        }
                    }
                }
                if !typing && ui.input(|i| i.key_pressed(egui::Key::P)) {
                    if let Some(i) = self.ui.selected {
                        if let Some(r) = page_s.get(i) {
                            if let Some(m) = &r.magnet_uri {
                                match crate::player::start_play(
                                    m,
                                    r.link.as_deref(),
                                    &self.cfg.player,
                                    self.picked_file(&r.title),
                                ) {
                                    Ok(()) => self.toast("Starting stream…", self.pal.accent),
                                    Err(e) => self.toast(&e, self.pal.red),
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
                self.draw_results_table(ui, page_s, base);
            }
        }
    }

    /// Bottom pagination bar (Prev / page numbers / Next). Shown only when
    /// there's more than one page. Mutates `self.search.page` and clears selection.
    pub(crate) fn draw_detail_panel(
        &mut self,
        ui: &mut egui::Ui,
        view: Option<&crate::app::ResultsView>,
    ) {
        // Render while the panel is open OR still animating closed
        // (detail_anim drives the fade; the flag flips instantly on close).
        if !self.ui.detail_open && self.ui.detail_anim <= 0.0 {
            return;
        }
        if self.ui.tab != Tab::Search {
            return;
        }
        let Some(view) = view else {
            return;
        };
        let page_s = self.page_slice(&view.sorted);
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
                    .inner_margin(margin(PANEL_MARGIN_X as f32, SP3)),
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
}
