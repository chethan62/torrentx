use eframe::egui::{self, FontId, RichText, Stroke};
use crate::app::App;
use crate::config::{save_cfg, ROW_H_COMPACT, ROW_H_NORMAL, ROW_H_ROOMY};
use crate::theme::tint;
use crate::ui::components::lbl;

pub fn draw(app: &mut App, ctx: &egui::Context) {
    egui::Panel::top("settings_panel")
        .frame(egui::Frame::NONE
            .fill(app.pal.hdr)
            .stroke(Stroke::new(1.0, app.pal.border))
            .inner_margin(egui::Margin::symmetric(14, 8)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    // ── Row 1: Connection ──────────────────────────────
                    ui.horizontal(|ui| {
                        lbl(ui, "CONNECTION", app.pal.dim, 10.0);
                        ui.add_space(6.0);
                        lbl(ui, "Jackett URL", app.pal.sub, 12.0);
                        ui.add(egui::TextEdit::singleline(&mut app.cfg.jackett_url)
                            .desired_width(172.0).font(FontId::monospace(12.0)));
                        ui.add_space(6.0);
                        lbl(ui, "API Key", app.pal.sub, 12.0);
                        ui.add(egui::TextEdit::singleline(&mut app.cfg.api_key)
                            .desired_width(210.0)
                            .password(!app.key_vis)
                            .hint_text("from Jackett dashboard (top-right)")
                            .font(FontId::monospace(12.0)));
                        if ui.small_button(if app.key_vis { "hide" } else { "show" }).clicked() {
                            app.key_vis = !app.key_vis;
                        }
                        ui.add_space(6.0);
                        lbl(ui, "Timeout", app.pal.sub, 12.0);
                        let mut ts = app.cfg.timeout_secs.to_string();
                        if ui.add(egui::TextEdit::singleline(&mut ts)
                            .desired_width(30.0).font(FontId::monospace(12.0))).changed() {
                            if let Ok(v) = ts.parse::<u64>() { app.cfg.timeout_secs = v.clamp(5, 120); }
                        }
                        lbl(ui, "s", app.pal.dim, 11.0);
                        ui.add_space(10.0);
                        lbl(ui, "RSS refresh", app.pal.sub, 12.0);
                        let mut rm = app.cfg.rss_refresh_min.to_string();
                        if ui.add(egui::TextEdit::singleline(&mut rm)
                            .desired_width(30.0).font(FontId::monospace(12.0))).changed() {
                            if let Ok(v) = rm.parse::<u64>() { app.cfg.rss_refresh_min = v.clamp(1, 1440); }
                        }
                        lbl(ui, "min", app.pal.dim, 11.0);
                    });
                    ui.add_space(5.0);

                    // ── Row 2: Display ──────────────────────────────────
                    ui.horizontal(|ui| {
                        lbl(ui, "DISPLAY", app.pal.dim, 10.0);
                        ui.add_space(6.0);

                        lbl(ui, "Rows", app.pal.sub, 12.0);
                        for (l, h) in [("Compact", ROW_H_COMPACT), ("Normal", ROW_H_NORMAL), ("Roomy", ROW_H_ROOMY)] {
                            let on = (app.cfg.row_height - h).abs() < 1.0;
                            if ui.add(egui::Button::selectable(on,
                                RichText::new(l).font(FontId::proportional(12.0))
                            )).clicked() { app.cfg.row_height = h; save_cfg(&app.cfg); }
                        }
                        ui.add_space(8.0);

                        lbl(ui, "Font", app.pal.sub, 12.0);
                        for (l, sz) in [("S", 12.0f32), ("M", 14.0), ("L", 16.0)] {
                            let on = (app.cfg.font_size - sz).abs() < 0.5;
                            if ui.add(egui::Button::selectable(on,
                                RichText::new(l).font(FontId::proportional(12.0))
                            )).clicked() { app.cfg.font_size = sz; save_cfg(&app.cfg); }
                        }
                        ui.add_space(8.0);

                        lbl(ui, "Page", app.pal.sub, 12.0);
                        for (l, ps) in [("25", 25usize), ("50", 50), ("100", 100), ("All", 0)] {
                            let on = app.cfg.page_size == ps;
                            if ui.add(egui::Button::selectable(on,
                                RichText::new(l).font(FontId::proportional(12.0))
                            )).clicked() { app.cfg.page_size = ps; app.page = 0; save_cfg(&app.cfg); }
                        }
                        ui.add_space(8.0);

                        let dedupe = app.cfg.dedupe;
                        if ui.add(egui::Button::selectable(dedupe,
                            RichText::new("Dedupe").font(FontId::proportional(12.0))
                        )).on_hover_text("Merge near-duplicate titles").clicked() {
                            app.cfg.dedupe = !dedupe; save_cfg(&app.cfg);
                        }
                        ui.add_space(4.0);

                        let catbar = app.cfg.show_cat_bar;
                        if ui.add(egui::Button::selectable(catbar,
                            RichText::new("Cat bar").font(FontId::proportional(12.0))
                        )).on_hover_text("Show category breakdown chips").clicked() {
                            app.cfg.show_cat_bar = !catbar; save_cfg(&app.cfg);
                        }
                    });
                    ui.add_space(5.0);

                    // ── Row 3: qBittorrent Integration ────────────────
                    ui.horizontal(|ui| {
                        lbl(ui, "QBITTORRENT", app.pal.dim, 10.0);
                        ui.add_space(6.0);
                        ui.checkbox(&mut app.cfg.qbit_enabled, "Enabled");
                        if app.cfg.qbit_enabled {
                            ui.add_space(6.0);
                            lbl(ui, "URL", app.pal.sub, 12.0);
                            ui.add(egui::TextEdit::singleline(&mut app.cfg.qbit_url)
                                .desired_width(260.0)
                                .hint_text("http://localhost:8080")
                                .font(FontId::monospace(12.0)));
                            ui.add_space(6.0);
                            lbl(ui, "User", app.pal.sub, 12.0);
                            ui.add(egui::TextEdit::singleline(&mut app.cfg.qbit_user)
                                .desired_width(90.0)
                                .hint_text("admin")
                                .font(FontId::monospace(12.0)));
                            ui.add_space(6.0);
                            lbl(ui, "Pass", app.pal.sub, 12.0);
                            ui.add(egui::TextEdit::singleline(&mut app.cfg.qbit_pass)
                                .desired_width(100.0)
                                .password(true)
                                .font(FontId::monospace(12.0)));
                        }
                        save_cfg(&app.cfg);
                    });
                    ui.add_space(5.0);

                    // ── Row 4: Columns ──────────────────────────────────
                    ui.horizontal(|ui| {
                        lbl(ui, "COLUMNS", app.pal.dim, 10.0);
                        ui.add_space(6.0);

                        let cols: &mut [(&str, &mut bool)] = &mut [
                            ("Tracker", &mut app.cfg.col_tracker),
                            ("Size",    &mut app.cfg.col_size),
                            ("Leech",   &mut app.cfg.col_leech),
                            ("Ratio",   &mut app.cfg.col_ratio),
                            ("Health",  &mut app.cfg.col_health),
                            ("Date",    &mut app.cfg.col_date),
                        ];
                        let mut changed = false;
                        for (label, val) in cols.iter_mut() {
                            let on = **val;
                            if ui.add(egui::Button::selectable(on,
                                RichText::new(*label).font(FontId::proportional(12.0))
                                    .color(if on { app.pal.accent } else { app.pal.dim })
                            )).clicked() { **val = !on; changed = true; }
                            ui.add_space(2.0);
                        }
                        if changed { save_cfg(&app.cfg); }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(egui::Button::new(
                                RichText::new("Save").font(FontId::proportional(12.0)).color(app.pal.green))
                                .fill(tint(app.pal.green, 18))
                                .stroke(Stroke::new(1.0, tint(app.pal.green, 80)))
                                .corner_radius(4.0)
                            ).clicked() {
                                save_cfg(&app.cfg);
                                let green = app.pal.green;
                                app.toast("Settings saved ✓", green);
                            }
                        });
                    });
                });
            });
        });
}
