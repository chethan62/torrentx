use eframe::egui::{self, FontId, RichText};
use crate::app::App;
use crate::theme::tint;

pub fn draw(app: &mut App, ctx: &egui::Context) {
    if app.toasts.is_empty() { return; }

    let screen = ctx.content_rect();
    let mut y   = screen.max.y - 16.0;

    // Render newest toast on top (reversed)
    let toasts: Vec<_> = app.toasts.iter().rev().cloned().collect();

    for toast in &toasts {
        let alpha = ((toast.ttl / 0.4).clamp(0.0, 1.0) * 255.0) as u8;
        let bg    = tint(toast.col, (28.0 * alpha as f32 / 255.0) as u8);
        let col   = egui::Color32::from_rgba_unmultiplied(
            toast.col.r(), toast.col.g(), toast.col.b(), alpha);
        let border = tint(toast.col, (90.0 * alpha as f32 / 255.0) as u8);

        // Measure text height (approx)
        let h = 38.0_f32;
        y -= h + 6.0;

        let w    = 320.0_f32;
        let x    = screen.max.x - w - 16.0;
        let pos  = egui::pos2(x, y);

        egui::Area::new(egui::Id::new(format!("toast_{}", toast.msg)))
            .fixed_pos(pos)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(bg)
                    .corner_radius(8.0)
                    .stroke(egui::Stroke::new(1.0, border))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 3],
                        blur: 12,
                        spread: 0,
                        color: crate::theme::rgba(0, 0, 0, 70),
                    })
                    .inner_margin(egui::Margin::symmetric(14, 9))
                    .show(ui, |ui| {
                        ui.set_min_width(w);
                        ui.label(RichText::new(&toast.msg)
                            .font(FontId::proportional(13.0))
                            .color(col));
                    });
            });
    }
}
