//! search drawing methods (split out for readability).

use super::*;

use crate::app::App;

impl App {
    pub(crate) fn draw_filter_bar(
        &mut self,
        ui: &mut egui::Ui,
        fs: f32,
        view: Option<&crate::app::ResultsView>,
    ) {
        egui::Frame::NONE
            .fill(self.pal.surface)
            .corner_radius(PANEL_RADIUS)
            .stroke(Stroke::new(1.0_f32, self.pal.border))
            .inner_margin(margin(PANEL_MARGIN_X as f32, SP3))
            .outer_margin(margin(PANEL_MARGIN_X as f32, 0.0))
            .show(ui, |ui| {
                // Row 1
                ui.horizontal_wrapped(|ui| {
                    // Track whether any filter input changed this frame so we can
                    // drop batch selections whose indices went stale. TextEdit
                    // responses report .changed() — no per-frame string clones.
                    let mut filter_changed = false;
                    lbl(ui, "Filter", self.pal.dim, fs);
                    ui.add_space(SP1);
                    filter_changed |= ui
                        .add(
                            egui::TextEdit::singleline(&mut self.search.f_text)
                                .desired_width(FILTER_TEXT_W)
                                .hint_text("within results")
                                .font(FontId::proportional(fs)),
                        )
                        .changed();
                    ui.add_space(SP3);
                    lbl(ui, "Seeds ≥", self.pal.dim, fs);
                    filter_changed |= ui
                        .add(
                            egui::TextEdit::singleline(&mut self.search.f_seed)
                                .desired_width(FILTER_NUM_W)
                                .hint_text("0")
                                .font(FontId::proportional(fs)),
                        )
                        .changed();
                    ui.add_space(SP3);
                    lbl(ui, "Max GB", self.pal.dim, fs);
                    filter_changed |= ui
                        .add(
                            egui::TextEdit::singleline(&mut self.search.f_size)
                                .desired_width(FILTER_NUM_W)
                                .hint_text("∞")
                                .font(FontId::proportional(fs)),
                        )
                        .changed();
                    ui.add_space(SP3);
                    lbl(ui, "Year ≥", self.pal.dim, fs);
                    filter_changed |= ui
                        .add(
                            egui::TextEdit::singleline(&mut self.search.f_year)
                                .desired_width(FILTER_YEAR_W)
                                .hint_text("any")
                                .font(FontId::proportional(fs)),
                        )
                        .changed();
                    ui.add_space(SP3);
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
                        ui.add_space(SP3);
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
                ui.add_space(SP2);
                // Row 2 — select mode + health + sort
                ui.horizontal_wrapped(|ui| {
                    lbl(ui, "Health", self.pal.dim, fs);
                    ui.add_space(SP2);
                    for hf in [
                        Hlth::All,
                        Hlth::Hot,
                        Hlth::Good,
                        Hlth::Slow,
                        Hlth::Dying,
                        Hlth::Dead,
                    ] {
                        let on = self.search.f_hlth == hf;
                        let chip_col = match hf {
                            Hlth::All => self.pal.sub,
                            Hlth::Hot | Hlth::Good => self.pal.green,
                            Hlth::Slow | Hlth::Dying => self.pal.yellow,
                            Hlth::Dead => self.pal.red,
                        };
                        if ui
                            .add(egui::Button::selectable(
                                on,
                                RichText::new(hf.label())
                                    .font(FontId::proportional(fs - 1.0))
                                    .color(if on { self.pal.accent } else { chip_col }),
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
                        ui.add_space(SP1);
                    }
                    // Batch selection is an action group, not a filter: park it at
                    // the right edge so the row is balanced and the Health chips
                    // stay next to the other filters.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.horizontal(|ui| {
                            // Batch select toggle (vector checkbox, no glyphs)
                            if v_checkbox(ui, self.ui.sel_mode, "Select", self.pal.accent).clicked()
                            {
                                self.ui.sel_mode = !self.ui.sel_mode;
                                self.ui.sel_set.clear();
                                self.ui.detail_open = false;
                            }
                            if self.ui.sel_mode {
                                let n = self.ui.sel_set.len();
                                let copy_label =
                                    format!("Copy {n} magnet{}", if n == 1 { "" } else { "s" });
                                if n > 0
                                    && icon_text_btn(
                                        ui,
                                        SvgIcon::Copy,
                                        &copy_label,
                                        self.pal.green,
                                        true,
                                    )
                                    && let Some(v) = view
                                {
                                    self.copy_selected_magnets(ui, &v.sorted);
                                }
                                ui.add_space(SP2);
                                // Select all visible / clear (no glyphs)
                                let all_checked = !self.ui.sel_set.is_empty();
                                if v_checkbox(ui, all_checked, "All", self.pal.accent)
                                    .on_hover_text("Select all results on this page")
                                    .clicked()
                                    && let Some(v) = view
                                {
                                    if self.cfg.page_size == 0 {
                                        self.ui.sel_set = (0..v.sorted.len()).collect();
                                    } else {
                                        let base = self.search.page * self.cfg.page_size;
                                        let end = (base + self.cfg.page_size).min(v.sorted.len());
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
                                            .corner_radius(R_SM),
                                        )
                                        .clicked()
                                {
                                    self.ui.sel_set.clear();
                                }
                                ui.add_space(SP2);
                            }
                        });
                    });
                });
            });
    }
}
