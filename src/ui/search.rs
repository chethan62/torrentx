//! search drawing methods.

use super::*;

use crate::app::App;

/// Width of the Search button. Fixed, so the field's width is derived rather than
/// guessed from a chrome total that grows every time a control is added.
const SEARCH_BTN_W: f32 = 132.0;
/// One height per row: every control in a row must be the same box, or the row
/// looks hand-assembled (the Search button used to stand 4 px taller than the
/// field it sits next to).
const ROW1_H: f32 = 36.0;
const ROW2_H: f32 = 28.0;
/// Both combos in row 2 share this width, so the labels line up in a grid.
const COMBO_W: f32 = 120.0;
/// Phone: both pickers share one row, so each box gives up 20 px. 96 still
/// fits the longest common label in this list ("The Pirate Bay" is clipped by
/// the combo's own ellipsis, and the tooltip keeps it readable).
const COMBO_W_NARROW: f32 = 96.0;

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
        // Row 1: query field + Search. The button has a fixed width and the field
        // takes the rest, so this row cannot wrap — the old `available - 505`
        // constant broke as soon as a control was added beside the field and the
        // button jumped onto a line of its own (it rendered at x=5, y=125).
        let bar_rect = self.search_query_row(ui, fs, busy);

        // Row 2: which indexers a search reaches, and the window to change that.
        self.search_filter_row(ui, fs);

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
        self.search_state_area(ui, state, view, fs);
    }

    /// Row 1: the query field and the Search button. Returns the field's rect
    /// (the history dropdown is positioned against it).
    fn search_query_row(&mut self, ui: &mut egui::Ui, fs: f32, busy: bool) -> egui::Rect {
        let mut bar_rect = egui::Rect::NOTHING;
        // Phone width: the long placeholder truncates mid-word, and a 132 px
        // button eats the room the field needs, so both give way.
        let tight = narrow(ui.available_width());
        let btn_w = if tight { 96.0 } else { SEARCH_BTN_W };
        let hint = if tight {
            "Search torrents…"
        } else {
            "Search torrents — movies, shows, games, software, anime…"
        };
        ui.horizontal(|ui| {
            // Every gap in these two rows is an explicit token: egui would add its
            // own item_spacing around each add_space, so the same SP4 produced 12 px
            // in one place and 20 px in another.
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.spacing_mut().interact_size.y = ROW1_H;
            ui.add_space(SP4);
            let field_w = (ui.available_width() - btn_w - SP3 - SP4).max(140.0);
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.search.query)
                    .id(egui::Id::unique("q"))
                    .desired_width(field_w)
                    .hint_text(hint)
                    .font(FontId::proportional(fs + 2.0))
                    // Room for the in-field clear button (always reserved, or the
                    // field would change height the moment text appears).
                    .margin(egui::Margin {
                        left: SP3 as i8,
                        right: (ICON_MD + SP3) as i8,
                        top: SP2 as i8,
                        bottom: SP2 as i8,
                    }),
            );
            bar_rect = resp.rect;
            if resp.gained_focus() && !self.cfg.history.is_empty() {
                self.ui.show_hist = true;
            }
            if resp.changed() && self.search.query.is_empty() {
                self.ui.show_hist = false;
            }
            if resp.lost_focus() {
                // Dismiss the popup when the field loses focus — otherwise it stays
                // over the filter bar, where its rows are topmost and a click meant
                // for a filter silently re-runs a history entry. The exception is a
                // click INSIDE the popup: focusing away is what that click does, and
                // hiding here in the same frame ate it (picking a past search did
                // nothing). The popup reports its own rect one frame ahead.
                let over_popup = ui
                    .input(|i| i.pointer.interact_pos())
                    .zip(self.ui.hist_rect)
                    .is_some_and(|(p, r)| r.contains(p));
                if !over_popup {
                    self.ui.show_hist = false;
                }
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.do_search();
                }
            }

            // Clear sits INSIDE the field's right edge, the way every search box
            // does it. After the Search button it overflowed the row (field width
            // is `available - 505`) and wrapped onto a second line.
            if !self.search.query.is_empty() {
                let box_side = ICON_MD + SP2;
                let clear_rect = egui::Rect::from_center_size(
                    egui::pos2(
                        resp.rect.right() - SP3 - box_side * 0.5,
                        resp.rect.center().y,
                    ),
                    Vec2::splat(box_side),
                );
                let hit = ui
                    .put(
                        clear_rect,
                        egui::Button::new(svg_image(SvgIcon::Close, ICON_XS, self.pal.sub))
                            .fill(Color32::TRANSPARENT)
                            .frame(false)
                            .min_size(Vec2::splat(box_side)),
                    )
                    .on_hover_text("Clear search (Esc)");
                if hit.clicked() {
                    self.clear_search();
                }
            }

            ui.add_space(SP4);
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
                    .min_size(Vec2::new(btn_w, ROW1_H)),
                )
                .clicked()
            {
                self.do_search();
            }
        });
        bar_rect
    }

    /// Row 2: category/indexer scope, the indexer picker, and the window that
    /// changes which indexers a search reaches.
    fn search_filter_row(&mut self, ui: &mut egui::Ui, fs: f32) {
        // Every gap in this row is explicit: egui adds its own item_spacing
        // around each add_space, so the same token rendered as 8 px in one
        // place and 16 px in another.
        if narrow(ui.available_width()) {
            // Phone width: the two pickers share a row at 96 px each (they
            // need ~470 px together at full width, which is how the indexer
            // picker ended up off-screen), and the indexer chip gets the row
            // below on its own.
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.spacing_mut().interact_size.y = ROW2_H;
                ui.add_space(SP4);
                self.cat_picker(ui, fs, COMBO_W_NARROW);
                ui.add_space(SP3);
                self.idx_picker(ui, fs, COMBO_W_NARROW);
            });
            // The indexer chip shares the Filters row on a phone (it is
            // drawn by `draw_filter_bar` there): its own row cost 50 px of a
            // 520 px screen, which is where the results table went.
        } else {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.spacing_mut().interact_size.y = ROW2_H;
                ui.add_space(SP4);
                self.cat_picker(ui, fs, COMBO_W);
                ui.add_space(SP4);
                self.idx_picker(ui, fs, COMBO_W);
                ui.add_space(SP4);
                self.indexers_chip(ui, fs);
            });
        }
    }

    /// Category picker, label and box as one atom.
    fn cat_picker(&mut self, ui: &mut egui::Ui, fs: f32, w: f32) {
        // Label + picker are ONE atom in the wrapping row: as separate
        // items the label could wrap to the line below its own picker.
        ui.horizontal(|ui| {
            lbl(ui, "Category", self.pal.dim, 11.0);
            ui.add_space(SP2);
            egui::ComboBox::from_id_salt("cat_cb")
                .selected_text(RichText::new(&self.search.cat).font(FontId::proportional(fs)))
                .width(w)
                .show_ui(ui, |ui| {
                    for &c in CATS {
                        ui.selectable_value(
                            &mut self.search.cat,
                            c.into(),
                            RichText::new(c).font(FontId::proportional(fs)),
                        );
                    }
                });
        });
    }

    /// Indexer picker (from Jackett's configured indexers).
    fn idx_picker(&mut self, ui: &mut egui::Ui, fs: f32, w: f32) {
        ui.horizontal(|ui| {
            lbl(ui, "Indexer", self.pal.dim, 11.0);
            ui.add_space(SP2);
            egui::ComboBox::from_id_salt("idx_cb")
                .selected_text(RichText::new(&self.net.indexer).font(FontId::proportional(fs)))
                .width(w)
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
        });
    }

    /// Which indexers the "All" fan-out covers, plus the catalogue. A compact
    /// "8/9" chip opens a small window: a menu popup closes on every click,
    /// which made the checkboxes and the filter field unusable.
    pub(crate) fn indexers_chip(&mut self, ui: &mut egui::Ui, fs: f32) {
        // Which indexers the "All" fan-out covers, plus the catalogue. A
        // compact "8/9" chip opens a small window: a menu popup closes on
        // every click, which made the checkboxes and the filter field unusable.
        let all_idx = self.net.indexers.clone();
        if !all_idx.is_empty() {
            ui.add_space(SP4);
            let on = all_idx
                .iter()
                .filter(|i| !self.cfg.disabled_indexers.iter().any(|d| d == *i))
                .count();
            // A labelled button, not a bare "9/9": the count alone never said
            // that indexers can be enabled or added here.
            let all_on = on == all_idx.len();
            let label = RichText::new(format!("Indexers  {on}/{}", all_idx.len()))
                .font(FontId::proportional(fs))
                .color(if all_on {
                    self.pal.text
                } else {
                    self.pal.yellow
                });
            let btn = egui::Button::new(label)
                .fill(self.pal.surface2)
                .stroke(egui::Stroke::new(
                    1.0,
                    if all_on {
                        self.pal.border
                    } else {
                        self.pal.yellow
                    },
                ))
                .corner_radius(R_MD)
                .min_size(Vec2::new(0.0, ROW2_H));
            if ui
                .add(btn)
                .on_hover_text(
                    "Enable or disable indexers, add one in Jackett, browse the catalogue",
                )
                .clicked()
            {
                self.net.idx_open = !self.net.idx_open;
            }
            let mut open = self.net.idx_open;
            let mut esc = false;
            if open {
                egui::Window::new("Indexers")
                    .id(egui::Id::unique("idx_win"))
                    .open(&mut open)
                    .collapsible(false)
                    .resizable(false)
                    .default_width(260.0)
                    .show(ui.ctx(), |ui| {
                        // Esc closes this before it reaches "clear search":
                        // it is the topmost surface while it is open.
                        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            esc = true;
                        }
                        if ui
                            .button(RichText::new("Enable all").font(FontId::proportional(fs)))
                            .clicked()
                        {
                            self.cfg.disabled_indexers.clear();
                            let _ = crate::config::save_cfg(&self.cfg);
                        }
                        ui.separator();
                        if ui
                            .button(
                                RichText::new("Add indexer in Jackett…")
                                    .font(FontId::proportional(fs)),
                            )
                            .on_hover_text(
                                "Jackett owns indexer credentials; its API is login-gated",
                            )
                            .clicked()
                        {
                            let base = self
                                .cfg
                                .jackett_url
                                .trim()
                                .trim_end_matches('/')
                                .to_string();
                            let _ = crate::safe_open(format!("{base}/UI/Dashboard"));
                        }
                        if ui
                            .button(RichText::new("Refresh list").font(FontId::proportional(fs)))
                            .clicked()
                        {
                            self.refresh_indexers();
                        }
                        // Catalogue: indexers Jackett knows but has not configured.
                        // The API cannot add one (every write needs Jackett's cookie
                        // session), so a click copies the id and opens the dashboard.
                        let cat = crate::jackett::IndexerCatalogue {
                            configured: all_idx.clone(),
                            all: self.net.catalogue.clone(),
                        };
                        let pending = cat.unconfigured().count();
                        if pending > 0 {
                            ui.separator();
                            ui.label(
                                RichText::new(format!("Add from catalogue ({pending})"))
                                    .font(FontId::proportional(fs - 1.0))
                                    .color(self.pal.dim),
                            );
                            // Group chips: with 635 entries the real question is
                            // "which one is worth configuring", not "how do I spell it".
                            for (gi, (label, ids)) in
                                crate::jackett::CATALOGUE_GROUPS.iter().enumerate()
                            {
                                let entries = crate::jackett::group_entries(&cat, ids);
                                if entries.is_empty() {
                                    continue;
                                }
                                let selected = self.net.cat_group == Some(gi);
                                let text = RichText::new(format!("{label} {}", entries.len()))
                                    .font(FontId::proportional(fs - 1.0))
                                    .color(if selected { self.pal.bg } else { self.pal.text });
                                let btn = egui::Button::new(text)
                                    .fill(if selected {
                                        self.pal.accent
                                    } else {
                                        self.pal.surface2
                                    })
                                    .corner_radius(R_MD);
                                if ui.add(btn).clicked() {
                                    self.net.cat_group = if selected { None } else { Some(gi) };
                                    self.net.cat_query.clear();
                                }
                            }
                            ui.add_space(SP2);
                            ui.add(
                                egui::TextEdit::singleline(&mut self.net.cat_query)
                                    .hint_text("filter…")
                                    .desired_width(230.0),
                            );
                            // A group narrows the list; the filter box narrows it further.
                            let q = self.net.cat_query.trim().to_lowercase();
                            let mut hits: Vec<String> = match self
                                .net
                                .cat_group
                                .and_then(|gi| crate::jackett::CATALOGUE_GROUPS.get(gi))
                            {
                                Some((_, ids)) => crate::jackett::group_entries(&cat, ids)
                                    .into_iter()
                                    .filter(|id| q.is_empty() || id.to_lowercase().contains(&q))
                                    .take(8)
                                    .cloned()
                                    .collect(),
                                None => {
                                    crate::jackett::catalogue_matches(&cat, &self.net.cat_query, 8)
                                        .into_iter()
                                        .cloned()
                                        .collect()
                                }
                            };
                            hits.retain(|h| !h.is_empty());
                            for id in &hits {
                                if ui
                                    .button(RichText::new(id).font(FontId::proportional(fs)))
                                    .on_hover_text("Copy the name and open Jackett")
                                    .clicked()
                                {
                                    ui.ctx().copy_text(id.clone());
                                    let base = self
                                        .cfg
                                        .jackett_url
                                        .trim()
                                        .trim_end_matches('/')
                                        .to_string();
                                    let _ = crate::safe_open(format!("{base}/UI/Dashboard"));
                                }
                            }
                            if self.net.cat_group
                                == crate::jackett::CATALOGUE_GROUPS
                                    .iter()
                                    .position(|(l, _)| *l == "Indian / Desi")
                            {
                                ui.label(
                                    RichText::new(
                                        "Jackett has no Telugu/Tamil/Malayalam indexer — \
                                     those releases come through the general trackers",
                                    )
                                    .font(FontId::proportional(fs - 1.5))
                                    .color(self.pal.dim),
                                );
                            }
                            if hits.len() == 8 {
                                ui.label(
                                    RichText::new("type to narrow the list")
                                        .font(FontId::proportional(fs - 1.0))
                                        .color(self.pal.dim),
                                );
                            }
                        }
                        ui.separator();
                        for idx in &all_idx {
                            let mut enabled = !self.cfg.disabled_indexers.iter().any(|d| d == idx);
                            if ui
                                .checkbox(
                                    &mut enabled,
                                    RichText::new(idx).font(FontId::proportional(fs)),
                                )
                                .changed()
                            {
                                if enabled {
                                    self.cfg.disabled_indexers.retain(|d| d != idx);
                                } else if !self.cfg.disabled_indexers.contains(idx) {
                                    self.cfg.disabled_indexers.push(idx.clone());
                                }
                                let _ = crate::config::save_cfg(&self.cfg);
                            }
                        }
                    });
                self.net.idx_open = open && !esc;
            }
        }
    }

    /// The state-dependent body: idle screen, live results while searching,
    /// done summary, error banner. Faded as one block.
    fn search_state_area(
        &mut self,
        ui: &mut egui::Ui,
        state: &SearchState,
        view: Option<&crate::app::ResultsView>,
        fs: f32,
    ) {
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
                    // The filter dropped the selected row: abandon its listing too.
                    self.close_detail();
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
                    // Phone: the four-field summary wrapped and left
                    // "· 5 trackers" orphaned on its own line, so the two
                    // fields a phone reader scans for stay on one line.
                    let summary = if narrow(ui.available_width()) {
                        format!("Showing {page_n} of {total}  ·  {seeds} seeds")
                    } else {
                        format!(
                            "Showing {page_n} of {total}  ·  {active} active  ·  \
                                      {seeds} seeds  ·  {} trackers",
                            trackers.len()
                        )
                    };
                    lbl(ui, &summary, self.pal.sub, fs - 1.0);
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
                        // Phone: one scrollable row instead of a wrapped
                        // block. Wrapped, the last chip ("Movies 5") ran off
                        // the right edge and the row cost two lines of a
                        // 520 px screen; a side-scrolling strip is the
                        // pattern a phone expects for filter chips.
                        let tight = narrow(ui.available_width());
                        let draw = |ui: &mut egui::Ui, me: &mut Self| {
                            ui.add_space(SP4);
                            for (cat, count, col) in &chips {
                                let sel = me.search.f_text == *cat;
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
                                            egui::Id::unique(("chip", cat)),
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
                                            if me.search.f_text == *cat {
                                                me.search.f_text.clear();
                                            } else {
                                                me.search.f_text = cat.clone();
                                            }
                                            // Category filter changed the result set: drop batch selections.
                                            me.ui.sel_set.clear();
                                            me.ui.sel_mode = false;
                                            me.search.page = 0;
                                        }
                                    });
                                ui.add_space(SP1);
                            }
                        };
                        if tight {
                            egui::ScrollArea::horizontal()
                                .id_salt("cat_chips_scroll")
                                .max_height(34.0)
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| draw(ui, self));
                                });
                        } else {
                            ui.horizontal_wrapped(|ui| draw(ui, self));
                        }
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
                if !typing
                    && ui.input(|i| i.key_pressed(egui::Key::Enter))
                    && let Some(i) = self.ui.selected
                    && let Some(r) = page_s.get(i)
                    && let Some(m) = &r.magnet_uri
                {
                    if is_magnet(m) {
                        let _ = safe_open(m);
                        self.toast("Opening magnet…", self.pal.accent);
                    } else {
                        self.toast("No valid magnet link", self.pal.yellow);
                    }
                }
                if !typing
                    && ui.input(|i| i.key_pressed(egui::Key::D))
                    && self.ui.selected.is_some()
                {
                    self.ui.detail_open = !self.ui.detail_open;
                }
                if !typing
                    && ui.input(|i| i.key_pressed(egui::Key::F))
                    && let Some(i) = self.ui.selected
                    && let Some(r) = page_s.get(i).cloned()
                {
                    self.add_fav(&r);
                }
                if !typing
                    && ui.input(|i| i.key_pressed(egui::Key::M))
                    && let Some(i) = self.ui.selected
                    && let Some(r) = page_s.get(i)
                    && let Some(m) = &r.magnet_uri
                {
                    if is_magnet(m) {
                        let _ = safe_open(m);
                        self.toast("Opening magnet…", self.pal.accent);
                    } else {
                        self.toast("No valid magnet link", self.pal.yellow);
                    }
                }
                if !typing
                    && ui.input(|i| i.key_pressed(egui::Key::P))
                    && let Some(i) = self.ui.selected
                    && let Some(r) = page_s.get(i)
                    && let Some(m) = &r.magnet_uri
                {
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
        if self.ui.detail_open
            && let Some(idx) = self.ui.selected
            && let Some(r) = page_s.get(idx).cloned()
        {
            self.ui.detail_row = Some(r.clone());
        }
        let Some(r) = self.ui.detail_row.clone() else {
            return;
        };
        // Wide: a frame-level right panel — it reserves space, so the table
        // shrinks beside it instead of being covered.
        let room = ui.available_width();
        let frame = egui::Frame::NONE
            .fill(self.pal.surface)
            .stroke(Stroke::new(1.0_f32, self.pal.border))
            .inner_margin(margin(PANEL_MARGIN_X as f32, SP3));
        if narrow(room) {
            // Phone: an overlay window over the content area instead of a side
            // panel. A right panel in a 327 pt window is either a sliver (it
            // kept a 61 pt width that ignored every size passed in) or wider
            // than the screen; the overlay is the phone pattern anyway — the
            // table waits behind it and the header's X closes it.
            let rect = ui.available_rect_before_wrap();
            egui::Window::new("detail_overlay")
                .id(egui::Id::unique("detail_overlay"))
                .title_bar(false)
                .collapsible(false)
                .resizable(false)
                .fixed_rect(rect)
                .frame(frame)
                .show(ui.ctx(), |ui| self.detail_panel_body(ui, &r));
        } else {
            egui::Panel::right("detail_pnl")
                .resizable(true)
                .default_size(self.ui.detail_width.clamp(240.0, 520.0))
                .size_range(220.0..=640.0)
                .frame(frame)
                .show(ui, |ui| self.detail_panel_body(ui, &r));
        }
    }

    /// Fade + slide in, then the panel's content. Shared by the desktop side
    /// panel and the phone overlay so the two cannot drift apart.
    fn detail_panel_body(&mut self, ui: &mut egui::Ui, r: &crate::jackett::TorrentResult) {
        let anim = self.ui.detail_anim;
        let eased = (self.tokens.easing)(anim);
        ui.set_opacity(eased);
        ui.add_space((1.0 - eased) * 8.0); // slight downward settle
        egui::ScrollArea::vertical().show(ui, |ui| self.draw_detail(ui, r));
    }
}
