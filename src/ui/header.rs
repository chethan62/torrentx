use eframe::egui::{self, Color32, FontId, RichText, Stroke, Vec2};
use crate::app::App;
use crate::theme::{Theme, tint};
use crate::types::Tab;

pub fn draw(app: &mut App, ctx: &egui::Context) {
    egui::TopBottomPanel::top("header")
        .exact_height(52.0)
        .frame(egui::Frame::none()
            .fill(app.pal.surface)
            .stroke(Stroke::new(1.0, app.pal.border)))
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.add_space(14.0);

                // Logo
                ui.label(RichText::new("Torrent")
                    .font(FontId::monospace(16.0)).strong().color(app.pal.text));
                ui.label(RichText::new("X")
                    .font(FontId::monospace(16.0)).strong().color(app.pal.accent));
                egui::Frame::none()
                    .fill(tint(app.pal.accent, 28)).rounding(10.0)
                    .inner_margin(egui::Margin::symmetric(5.0, 1.0))
                    .show(ui, |ui| {
                        ui.label(RichText::new("v7").size(10.0).color(app.pal.accent));
                    });

                ui.add_space(14.0);
                ui.separator();
                ui.add_space(8.0);

                // Tabs
                let rss_badge = if !app.rss_feeds.is_empty() {
                    let total: usize = app.rss_feeds.iter().map(|f| f.items.len()).sum();
                    if total > 0 { format!(" {}", total) } else { String::new() }
                } else { String::new() };

                let tabs: &[(&str, Tab, String)] = &[
                    ("Search",    Tab::Search,    String::new()),
                    ("RSS Feeds", Tab::Rss,       rss_badge),
                    ("Favorites", Tab::Favorites, if !app.cfg.favorites.is_empty() {
                        format!(" {}", app.cfg.favorites.len()) } else { String::new() }),
                    ("About",     Tab::About,     String::new()),
                ];

                for (label, tab, badge) in tabs {
                    let active = &app.tab == tab;
                    if ui.add(egui::Button::new(
                        RichText::new(format!("{label}{badge}"))
                            .font(FontId::proportional(14.0))
                            .color(if active { app.pal.accent } else { app.pal.sub }))
                        .fill(if active { tint(app.pal.accent, 22) } else { Color32::TRANSPARENT })
                        .stroke(Stroke::new(if active { 1.0 } else { 0.0 }, app.pal.accent))
                        .rounding(6.0).min_size(Vec2::new(0.0, 30.0))
                    ).clicked() {
                        app.tab = tab.clone();
                        app.detail_open  = false;
                        app.selected     = None;
                        app.rss_detail   = None;
                        app.rss_add_mode = false;
                    }
                    ui.add_space(2.0);
                }

                // Right side
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(12.0);

                    // Settings toggle
                    let sa = app.show_settings;
                    if ui.add(egui::Button::new(
                        RichText::new("⚙ Settings").size(13.0)
                            .color(if sa { app.pal.accent } else { app.pal.sub }))
                        .fill(if sa { tint(app.pal.accent, 22) } else { Color32::TRANSPARENT })
                        .stroke(Stroke::new(1.0, if sa { app.pal.accent } else { app.pal.border }))
                        .rounding(6.0).min_size(Vec2::new(0.0, 30.0))
                    ).clicked() { app.show_settings = !app.show_settings; }
                    ui.add_space(10.0);

                    // Theme picker
                    let ac = app.cfg.theme.accent();
                    egui::ComboBox::from_id_source("theme_picker")
                        .selected_text(RichText::new(app.cfg.theme.name())
                            .font(FontId::proportional(13.0)).color(ac))
                        .width(158.0)
                        .show_ui(ui, |ui| {
                            ui.label(RichText::new("── Dark ──").size(10.0).color(app.pal.dim));
                            for t in Theme::all().iter().filter(|t| !t.is_light()) {
                                let col = t.accent();
                                let on  = &app.cfg.theme == t;
                                if ui.add(egui::SelectableLabel::new(on,
                                    RichText::new(format!("  {}", t.name()))
                                        .font(FontId::proportional(13.0)).color(col)
                                )).clicked() { app.set_theme(t.clone()); }
                            }
                            ui.add_space(3.0);
                            ui.label(RichText::new("── Light ──").size(10.0).color(app.pal.dim));
                            for t in Theme::all().iter().filter(|t| t.is_light()) {
                                let col = t.accent();
                                let on  = &app.cfg.theme == t;
                                if ui.add(egui::SelectableLabel::new(on,
                                    RichText::new(format!("  {}", t.name()))
                                        .font(FontId::proportional(13.0)).color(col)
                                )).clicked() { app.set_theme(t.clone()); }
                            }
                        });
                    ui.add_space(10.0);

                    // Result count
                    let n = app.total_count();
                    if n > 0 {
                        ui.label(RichText::new(format!("{n} results"))
                            .font(FontId::proportional(12.0)).color(app.pal.dim));
                    }
                });
            });
        });
}
