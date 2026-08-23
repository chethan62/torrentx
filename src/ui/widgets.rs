//! Shared cell-content rendering (free function, not a method).

use super::*;

pub(crate) fn draw_cell_content(
    ui: &mut egui::Ui,
    c: &TableCol,
    r: &TorrentResult,
    seed: u32,
    leech: u32,
    fsz: f32,
    pal: &Pal,
) {
    // Not flush against the cell's left edge — consistent leading padding.
    ui.add_space(4.0);
    match c {
        TableCol::Tracker => {
            ui.add(
                egui::Label::new(
                    RichText::new(r.tracker.as_deref().unwrap_or("—"))
                        .font(FontId::proportional(fsz - 1.0))
                        .color(pal.sub),
                )
                .truncate(),
            );
        }
        TableCol::Size => {
            ui.label(
                RichText::new(r.size.map(fmt_size).unwrap_or_else(|| "—".into()))
                    .font(FontId::monospace(fsz - 0.5))
                    .color(pal.sub),
            );
        }
        TableCol::Seeds => {
            ui.label(
                RichText::new(seed.to_string())
                    .font(FontId::monospace(fsz - 0.5))
                    .color(seed_col(seed))
                    .strong(),
            );
        }
        TableCol::Leech => {
            ui.label(
                RichText::new(leech.to_string())
                    .font(FontId::monospace(fsz - 0.5))
                    .color(pal.red),
            );
        }
        TableCol::Ratio => {
            let tot = (seed + leech) as f32;
            if tot > 0.0 {
                let pct = (seed as f32 / tot).clamp(0.0, 1.0);
                let rect = ui.available_rect_before_wrap();
                let bar = egui::Rect::from_min_size(
                    rect.min + Vec2::new(2.0, (rect.height() - 7.0) / 2.0),
                    Vec2::new((rect.width() - 4.0).max(8.0), 7.0),
                );
                ui.painter().rect_filled(bar, 3.0, pal.border);
                let mut filled = bar;
                filled.max.x = bar.min.x + bar.width() * pct;
                ui.painter().rect_filled(filled, 3.0, seed_col(seed));
                ui.allocate_rect(bar, egui::Sense::hover())
                    .on_hover_text(format!("{:.0}% seeded", pct * 100.0));
            } else {
                ui.label(
                    RichText::new("—")
                        .font(FontId::proportional(fsz - 1.0))
                        .color(pal.dim),
                );
            }
        }
        TableCol::Health => {
            let dot = if seed > 10 {
                SvgIcon::CircleDot
            } else {
                SvgIcon::Circle
            };
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                svg_icon(ui, dot, 8.0, seed_col(seed));
                ui.label(
                    RichText::new(hlth_lbl(seed))
                        .font(FontId::proportional(fsz - 1.0))
                        .strong()
                        .color(seed_col(seed)),
                );
            });
        }
        TableCol::Date => {
            let d = r
                .publish_date
                .as_deref()
                .map(time_ago)
                .unwrap_or_else(|| "—".into());
            ui.label(
                RichText::new(d)
                    .font(FontId::monospace(fsz - 0.5))
                    .color(pal.dim),
            );
        }
        TableCol::Name => {} // handled inline (interaction)
    }
}
