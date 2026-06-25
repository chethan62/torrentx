use eframe::egui::{self, FontId, RichText, Stroke, Vec2};
use crate::app::App;
use crate::types::TorrentResult;
use crate::utils::{fmt_size, health_label, seed_color, time_ago};
use crate::ui::components::{grid_row, wide_btn};
use crate::theme::tint;

pub fn draw(app: &mut App, ui: &mut egui::Ui, r: &TorrentResult) {
    // Paint full background including drag handle area
    let surface = app.pal.surface;
    ui.painter().rect_filled(ui.max_rect().expand(4.0), 0.0, surface);
    let pal = app.pal.clone();
    let fs  = app.cfg.font_size;
    let seed  = r.seeders.unwrap_or(0);
    let leech = r.peers.unwrap_or(0);

    egui::ScrollArea::vertical()
        .id_salt("detail_scroll")
        .auto_shrink([false; 2])
        .show(ui, |ui| {

    ui.add_space(10.0);

    // ── Title ──────────────────────────────────────────────────────────────
    egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(12, 0))
        .show(ui, |ui| {
            ui.add(egui::Label::new(
                RichText::new(&r.title)
                    .font(FontId::proportional(fs - 0.5))
                    .color(pal.text).strong()
            ).wrap());
        });

    ui.add_space(10.0);

    // ── Seed/leech ratio bar ───────────────────────────────────────────────
    egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(12, 0))
        .show(ui, |ui| {
            let tot = (seed + leech) as f32;
            if tot > 0.0 {
                let pct  = (seed as f32 / tot).clamp(0.0, 1.0);
                let w    = ui.available_width();
                let (_, bar) = ui.allocate_space(Vec2::new(w, 8.0));
                let p = ui.painter();
                p.rect_filled(bar, 4.0, pal.border);
                let mut filled = bar;
                filled.max.x = bar.min.x + bar.width() * pct;
                p.rect_filled(filled, 4.0, seed_color(seed));
                ui.add_space(4.0);
            }

            // Seed / leech labels
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("▲ {} seeds", seed))
                    .font(FontId::proportional(fs)).color(seed_color(seed)).strong());
                ui.add_space(12.0);
                ui.label(RichText::new(format!("▼ {} leechers", leech))
                    .font(FontId::proportional(fs)).color(pal.red));
                ui.add_space(8.0);
                // health pill
                egui::Frame::NONE
                    .fill(tint(seed_color(seed), 25)).corner_radius(10.0)
                    .inner_margin(egui::Margin::symmetric(6, 2))
                    .show(ui, |ui| {
                        ui.label(RichText::new(health_label(seed))
                            .font(FontId::proportional(fs - 2.0))
                            .color(seed_color(seed)).strong());
                    });
            });
        });

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);

    // ── Metadata grid ─────────────────────────────────────────────────────
    egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(12, 0))
        .show(ui, |ui| {
            egui::Grid::new("detail_grid")
                .num_columns(2)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
                    if let Some(t) = &r.tracker {
                        grid_row(ui, "Tracker",  t, pal.accent, pal.dim, fs);
                    }
                    if let Some(c) = &r.category_desc {
                        grid_row(ui, "Category", c, pal.text, pal.dim, fs);
                    }
                    if let Some(sz) = r.size {
                        grid_row(ui, "Size", &fmt_size(sz), pal.text, pal.dim, fs);
                    }
                    if let Some(d) = r.publish_date.as_deref() {
                        grid_row(ui, "Published", &time_ago(d), pal.text, pal.dim, fs);
                    }
                    if tot_nonzero(seed, leech) {
                        let tot = (seed + leech) as f32;
                        grid_row(ui, "Ratio",
                            &format!("{:.2}", if leech > 0 { seed as f32 / leech as f32 } else { f32::INFINITY }),
                            pal.text, pal.dim, fs);
                        grid_row(ui, "Swarm",
                            &format!("{:.0}% seeded", seed as f32 / tot * 100.0),
                            pal.text, pal.dim, fs);
                    }
                });
        });

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

    // ── Actions ───────────────────────────────────────────────────────────
    egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(12, 0))
        .show(ui, |ui| {
            if r.magnet_uri.is_some() {
                let accent = pal.accent;
                if wide_btn(ui, "⚡  Open Magnet", accent) {
                    if let Some(m) = &r.magnet_uri {
                        let _ = open::that(m);
                        let a = app.pal.accent;
                        app.toast("Opening in torrent client…", a);
                    }
                }
                ui.add_space(5.0);
                let sub = pal.sub;
                if wide_btn(ui, "⎘  Copy Magnet Link", sub) {
                    if let Some(m) = &r.magnet_uri {
                        ui.ctx().output_mut(|o| o.commands.push(egui::OutputCommand::CopyText(m.clone().to_string())));
                        let g = app.pal.green;
                        app.toast("Magnet link copied ✓", g);
                    }
                }
                ui.add_space(5.0);
            }

            if r.link.is_some() {
                let green = pal.green;
                if wide_btn(ui, "↓  Download .torrent", green) {
                    if let Some(l) = &r.link { let _ = open::that(l); }
                }
                ui.add_space(5.0);
            }

            let yellow = pal.yellow;
            if wide_btn(ui, "★  Save to Favorites", yellow) {
                let rc = r.clone();
                app.add_fav(&rc);
            }
            ui.add_space(5.0);

            if app.cfg.qbit_enabled && r.magnet_uri.is_some() {
                if wide_btn(ui, "📥  Send to qBittorrent", pal.accent) {
                    if let Some(m) = &r.magnet_uri {
                        if app.send_to_qbit(m) {
                            app.toast("Sent to qBittorrent ✓", pal.green);
                        } else {
                            app.toast("Failed to send — is qBit running?", pal.red);
                        }
                    }
                }
                ui.add_space(5.0);
            }

            if r.details.is_some() {
                let dim = pal.dim;
                if wide_btn(ui, "⊙  Open Detail Page", dim) {
                    if let Some(d) = &r.details { let _ = open::that(d); }
                }
            }
        });

    ui.add_space(10.0);

    // ── Magnet raw preview ────────────────────────────────────────────────
    if let Some(mag) = &r.magnet_uri {
        ui.separator();
        ui.add_space(6.0);
        egui::Frame::NONE
            .inner_margin(egui::Margin::symmetric(12, 0))
            .show(ui, |ui| {
                ui.label(RichText::new("Magnet URI").font(FontId::proportional(fs - 2.5)).color(pal.dim));
                ui.add_space(3.0);
                let preview = if mag.len() > 200 { format!("{}…", &mag[..200]) } else { mag.clone() };
                egui::Frame::NONE
                    .fill(pal.hdr).corner_radius(6.0)
                    .stroke(Stroke::new(1.0, pal.border))
                    .inner_margin(egui::Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        ui.add(egui::Label::new(
                            RichText::new(&preview)
                                .font(FontId::monospace(fs - 3.5))
                                .color(pal.dim)
                        ).wrap());
                    });
            });
    }

    }); // end ScrollArea
}

fn tot_nonzero(seed: u32, leech: u32) -> bool { seed + leech > 0 }
