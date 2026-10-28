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
    // Leading padding is the caller's cell inset (see CELL_PAD_X in results.rs).
    match c {
        TableCol::Tracker => {
            // Grouped by infohash: "Tracker +2" and the rest on hover. The count
            // is worth a column cell of its own only when it is not 1.
            let own = r.tracker.as_deref().unwrap_or("—");
            let text = if r.also_on.is_empty() {
                own.to_string()
            } else {
                format!("{} +{}", own, r.also_on.len())
            };
            let resp = ui.add(
                egui::Label::new(
                    RichText::new(text)
                        .font(FontId::proportional(fsz - 1.0))
                        .color(if r.also_on.is_empty() {
                            pal.sub
                        } else {
                            pal.accent
                        }),
                )
                .truncate(),
            );
            if !r.also_on.is_empty() {
                resp.on_hover_text(format!("Also on: {}", r.also_on.join(", ")));
            }
        }
        TableCol::Size => {
            ui.label(
                RichText::new(r.size.map(fmt_size).unwrap_or_else(|| "—".into()))
                    .font(FontId::monospace(fsz - 0.5))
                    .color(pal.sub),
            );
        }
        TableCol::Seeds => {
            // An indexer that publishes no seeder count (btdig, any DHT index) is
            // unknown, not dead: "0" would claim the swarm is empty.
            let (txt, col) = match r.seeders {
                Some(s) => (s.to_string(), seed_col(s)),
                None => ("—".into(), pal.dim),
            };
            ui.label(
                RichText::new(txt)
                    .font(FontId::monospace(fsz - 0.5))
                    .color(col)
                    .strong(),
            );
        }
        TableCol::Leech => {
            ui.label(
                RichText::new(match r.peers {
                    Some(p) => p.to_string(),
                    None => "—".into(),
                })
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
            let Some(seed) = r.seeders else {
                ui.label(
                    RichText::new("—")
                        .font(FontId::proportional(fsz - 1.0))
                        .color(pal.dim),
                );
                return;
            };
            let dot = if seed > 10 {
                SvgIcon::CircleDot
            } else {
                SvgIcon::Circle
            };
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                svg_icon(ui, dot, ICON_XS, seed_col(seed));
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

/// Content rect inside a row band: the band minus `inset`, never smaller than
/// 1x1. An unclamped `shrink2` on a band narrower or shorter than the insets
/// yields a negative rect that silently draws nothing (a row that looks empty).
pub(crate) fn band_inner(band: egui::Rect, inset: egui::Vec2) -> egui::Rect {
    let inner = band.shrink2(inset);
    egui::Rect::from_min_max(inner.min, inner.max.max(inner.min + egui::vec2(1.0, 1.0)))
}

#[cfg(test)]
mod band_tests {
    use super::*;

    /// Bands exactly as small as their insets must still yield a positive rect:
    /// `shrink2` alone returns zero/negative and the child Ui draws nothing.
    #[test]
    fn a_tiny_band_still_yields_a_drawable_inner_rect() {
        // RSS feed row (10,7) and Favorites band (16,10)
        for inset in [egui::vec2(10.0, 7.0), egui::vec2(16.0, 10.0)] {
            for (w, h) in [(200.0, 40.0), (20.0, 14.0), (12.0, 14.0), (4.0, 4.0)] {
                let band = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(w, h));
                let inner = band_inner(band, inset);
                assert!(
                    inner.width() >= 1.0 && inner.height() >= 1.0,
                    "band {w}x{h} inset {inset:?} -> {}x{}",
                    inner.width(),
                    inner.height()
                );
            }
        }
    }
}
