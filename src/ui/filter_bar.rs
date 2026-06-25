use eframe::egui::{self, FontId, RichText, Stroke};
use crate::app::App;
use crate::types::{HealthFilter, SortCol, SortDir};
use crate::ui::components::{lbl, outline_btn};

pub fn draw(app: &mut App, ui: &mut egui::Ui) {
    let fs = app.cfg.font_size;

    egui::Frame::NONE
        .fill(app.pal.surface)
        .corner_radius(8.0)
        .stroke(Stroke::new(1.0, app.pal.border))
        .inner_margin(egui::Margin::symmetric(12, 7))
        .outer_margin(egui::Margin::symmetric(12, 0))
        .show(ui, |ui| {
            // ── Row 1: text filters ────────────────────────────────────
            ui.horizontal(|ui| {
                lbl(ui, "Filter", app.pal.dim, fs);
                ui.add_space(3.0);
                ui.add(egui::TextEdit::singleline(&mut app.filters.text)
                    .desired_width(115.0).hint_text("within results")
                    .font(FontId::proportional(fs)));
                ui.add_space(8.0);

                lbl(ui, "Seeds ≥", app.pal.dim, fs);
                ui.add(egui::TextEdit::singleline(&mut app.filters.min_seeds)
                    .desired_width(38.0).hint_text("0").font(FontId::proportional(fs)));
                ui.add_space(8.0);

                lbl(ui, "Max GB", app.pal.dim, fs);
                ui.add(egui::TextEdit::singleline(&mut app.filters.max_gb)
                    .desired_width(38.0).hint_text("∞").font(FontId::proportional(fs)));
                ui.add_space(8.0);

                lbl(ui, "Year ≥", app.pal.dim, fs);
                ui.add(egui::TextEdit::singleline(&mut app.filters.min_year)
                    .desired_width(44.0).hint_text("any").font(FontId::proportional(fs)));
                ui.add_space(8.0);

                lbl(ui, "Tracker", app.pal.dim, fs);
                ui.add(egui::TextEdit::singleline(&mut app.filters.tracker)
                    .desired_width(86.0).hint_text("any").font(FontId::proportional(fs)));

                if app.filters.is_dirty() {
                    ui.add_space(8.0);
                    let red = app.pal.red;
                    if outline_btn(ui, "✕ Reset", red) {
                        app.filters.reset();
                        app.page = 0;
                    }
                }
            });
            ui.add_space(5.0);

            // ── Row 2: health + sort ───────────────────────────────────
            ui.horizontal(|ui| {
                lbl(ui, "Health", app.pal.dim, fs);
                ui.add_space(4.0);
                for hf in [HealthFilter::All, HealthFilter::Hot, HealthFilter::Good,
                           HealthFilter::Slow, HealthFilter::Dead] {
                    let on = app.filters.health == hf;
                    if ui.add(egui::Button::selectable(on,
                        RichText::new(hf.label()).font(FontId::proportional(fs - 1.0))
                            .color(if on { app.pal.accent } else { app.pal.sub })
                    )).clicked() { app.filters.health = hf; app.page = 0; }
                    ui.add_space(2.0);
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Sort direction toggle
                    let d_lbl = if app.sort_dir == SortDir::Desc { "▼ DESC" } else { "▲ ASC" };
                    let accent = app.pal.accent;
                    if ui.add(egui::Button::new(
                        RichText::new(d_lbl).font(FontId::proportional(fs - 1.0)).color(accent))
                        .fill(crate::theme::tint(accent, 18))
                        .stroke(Stroke::new(1.0, crate::theme::tint(accent, 60)))
                        .corner_radius(4.0)
                    ).on_hover_text("Toggle sort direction").clicked() {
                        app.sort_dir = if app.sort_dir == SortDir::Desc { SortDir::Asc } else { SortDir::Desc };
                        app.page = 0;
                    }
                    ui.add_space(6.0);
                    lbl(ui, "Sort:", app.pal.dim, fs);
                    ui.add_space(4.0);

                    for (l, col) in [
                        ("Date",    SortCol::Date),
                        ("Size",    SortCol::Size),
                        ("Ratio",   SortCol::Ratio),
                        ("Leech",   SortCol::Leech),
                        ("Seeds",   SortCol::Seeds),
                        ("Tracker", SortCol::Tracker),
                        ("Name",    SortCol::Name),
                    ] {
                        let on = app.sort_col == col;
                        let txt = if on {
                            if app.sort_dir == SortDir::Desc { format!("{l}▼") } else { format!("{l}▲") }
                        } else { l.to_string() };
                        if ui.add(egui::Button::selectable(on,
                            RichText::new(&txt).font(FontId::proportional(fs - 1.0))
                                .color(if on { app.pal.accent } else { app.pal.sub })
                        )).clicked() {
                            if app.sort_col == col {
                                app.sort_dir = if app.sort_dir == SortDir::Desc { SortDir::Asc } else { SortDir::Desc };
                            } else {
                                app.sort_col = col;
                                app.sort_dir = SortDir::Desc;
                            }
                            app.page = 0;
                        }
                        ui.add_space(2.0);
                    }
                });
            });
        });
}
