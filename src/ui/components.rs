use eframe::egui::{self, Color32, FontId, RichText, Stroke, Vec2};
use crate::theme::tint;

/// Small labelled action button used in table rows.
pub fn act_btn(ui: &mut egui::Ui, label: &str, tooltip: &str, col: egui::Color32) -> bool {
    ui.add(
        egui::Button::new(
            egui::RichText::new(label)
                .size(11.5)
                .color(col)
        )
        .fill(crate::theme::tint(col, 18))
        .stroke(egui::Stroke::new(1.0, crate::theme::tint(col, 60)))
        .corner_radius(6.0)
        .min_size(egui::vec2(28.0, 22.0))
    )
    .on_hover_text(tooltip)
    .clicked()
}

/// Full-width action button used in side panels.
pub fn wide_btn(ui: &mut egui::Ui, label: &str, color: Color32) -> bool {
    let w = ui.available_width().max(200.0);
    ui.add(
        egui::Button::new(RichText::new(label).font(FontId::proportional(13.0)).color(color))
            .fill(tint(color, 18))
            .stroke(Stroke::new(1.0, tint(color, 80)))
            .corner_radius(6.0)
            .min_size(Vec2::new(w, 34.0)),
    ).clicked()
}

/// Transparent outline button.
pub fn outline_btn(ui: &mut egui::Ui, label: &str, color: Color32) -> bool {
    ui.add(
        egui::Button::new(RichText::new(label).font(FontId::proportional(12.0)).color(color))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, tint(color, 80)))
            .corner_radius(4.0),
    ).clicked()
}

/// Convenience colored label.
pub fn lbl(ui: &mut egui::Ui, text: &str, color: Color32, fs: f32) {
    ui.label(RichText::new(text).font(FontId::proportional(fs)).color(color));
}

/// Two-column grid row used in detail panels.
pub fn grid_row(
    ui: &mut egui::Ui, label: &str, value: &str,
    color: Color32, dim: Color32, fs: f32,
) {
    ui.label(RichText::new(format!("{label}:")).font(FontId::proportional(fs - 1.5)).color(dim));
    ui.label(RichText::new(value).font(FontId::proportional(fs - 1.0)).color(color));
    ui.end_row();
}

/// Chip / badge label (non-interactive).
pub fn chip(ui: &mut egui::Ui, text: &str, color: Color32, selected: bool) {
    eframe::egui::Frame::NONE
        .fill(tint(color, if selected { 50 } else { 20 }))
        .corner_radius(10.0)
        .stroke(Stroke::new(if selected { 1.5 } else { 1.0 }, tint(color, if selected { 200 } else { 80 })))
        .inner_margin(eframe::egui::Margin::symmetric(7, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text).font(FontId::proportional(11.0)).color(color));
        });
}

/// Pill-style status indicator.
pub fn status_pill(ui: &mut egui::Ui, text: &str, color: Color32) {
    eframe::egui::Frame::NONE
        .fill(tint(color, 28))
        .corner_radius(10.0)
        .stroke(Stroke::new(1.0, tint(color, 100)))
        .inner_margin(eframe::egui::Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text).font(FontId::proportional(11.0)).color(color));
        });
}
