use eframe::egui::{self, Stroke};
use crate::app::App;
use crate::types::{SearchState, SPINNER_FRAMES};
use crate::ui::components::lbl;

pub fn draw(app: &mut App, ctx: &egui::Context, state: &SearchState) {
    egui::TopBottomPanel::bottom("status_bar")
        .exact_height(26.0)
        .frame(egui::Frame::none()
            .fill(app.pal.hdr)
            .stroke(Stroke::new(1.0, app.pal.border))
            .inner_margin(egui::Margin::symmetric(12.0, 4.0)))
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                match state {
                    SearchState::Idle => {
                        lbl(ui, "Ready — type a query and press Search", app.pal.dim, 12.0);
                    }
                    SearchState::Searching => {
                        let sp = SPINNER_FRAMES[app.spin_i];
                        let elapsed = app.t_start.as_ref()
                            .map(|t| format!("  {:.1}s", t.elapsed().as_secs_f64()))
                            .unwrap_or_default();
                        lbl(ui, &format!("{sp} Searching \"{}\"{}",
                            app.last_query, elapsed), app.pal.accent, 12.0);
                    }
                    SearchState::Done => {
                        let n = app.total_count();
                        let elapsed = app.t_done.map(|e| format!("  ({:.1}s)", e)).unwrap_or_default();
                        lbl(ui, &format!("✓ {n} results for \"{}\"{}",
                            app.last_query, elapsed), app.pal.green, 12.0);
                    }
                    SearchState::Error(e) => {
                        lbl(ui, &format!("✕ {}", e.lines().next().unwrap_or(e)),
                            app.pal.red, 12.0);
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    lbl(ui, "Ctrl+F  Ctrl+R  ↑↓  Enter  D=detail  F=fav  M=magnet  Esc",
                        app.pal.dim, 10.5);
                });
            });
        });
}
