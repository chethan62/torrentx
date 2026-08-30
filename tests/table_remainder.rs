//! Offscreen render test: proves that a resizable remainder column locks its
//! width (leaving dead space after the last fixed column), while a
//! non-resizable remainder re-fills every frame.
//!
//! Run with:  cargo test --test table_remainder -- --nocapture
use egui::{CentralPanel, Color32, Context, FontId, RawInput, RichText, Vec2};
use egui_extras::{Column, TableBuilder};

fn render_table(ui: &mut egui::Ui, name_resizable: bool) -> f32 {
    // Columns mirror the app: Name remainder + 7 fixed (Tracker..Date) + Actions(190)
    let fixed: [f32; 7] = [84.0, 62.0, 72.0, 62.0, 56.0, 72.0, 84.0];
    let name_col = Column::remainder().at_least(120.0);
    let name_col = if name_resizable {
        name_col.resizable(true)
    } else {
        name_col.resizable(false)
    };
    let mut tb_full = TableBuilder::new(ui)
        .striped(false)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center));
    tb_full = tb_full.column(name_col);
    for w in fixed {
        tb_full = tb_full.column(Column::initial(w).at_least(36.0));
    }
    tb_full = tb_full.column(Column::initial(190.0).at_least(150.0)); // Actions

    let mut actions_right = 0.0f32;
    tb_full
        .header(30.0, |mut header| {
            for label in [
                "Name", "Tracker", "Seeds", "Size", "Leech", "Ratio", "Health", "Date",
            ] {
                header.col(|ui| {
                    ui.label(RichText::new(label).font(FontId::proportional(12.0)));
                });
            }
            header.col(|ui| {
                ui.label(RichText::new("Actions").font(FontId::proportional(12.0)));
            });
        })
        .body(|mut body| {
            for row in 0..20 {
                body.row(30.0, |mut r| {
                    for _ in 0..8 {
                        r.col(|ui| {
                            ui.label(
                                RichText::new(format!("cell {row}"))
                                    .font(FontId::proportional(12.0)),
                            );
                        });
                    }
                    r.col(|ui| {
                        actions_right = ui.max_rect().right();
                    });
                });
            }
        });
    actions_right
}

/// Render two frames on the same context (table state persists between them):
/// first at 1100px wide, then at 1920px wide. Returns the Actions column's
/// right edge on the wide frame.
fn wide_frame_actions_right(name_resizable: bool) -> f32 {
    let ctx = Context::default();
    let mut actions_right = 0.0f32;

    let input1 = RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            Vec2::new(1100.0, 800.0),
        )),
        ..Default::default()
    };
    let mut out1 = ctx.run_ui(input1, |ui| {
        CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::from_rgb(26, 27, 38)))
            .show(ui, |ui| {
                ui.add_space(10.0);
                let _ = render_table(ui, name_resizable);
            });
    });
    out1.textures_delta.clear();

    let input2 = RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            Vec2::new(1920.0, 800.0),
        )),
        ..Default::default()
    };
    let mut out2 = ctx.run_ui(input2, |ui| {
        CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::from_rgb(26, 27, 38)))
            .show(ui, |ui| {
                ui.add_space(10.0);
                actions_right = render_table(ui, name_resizable);
            });
    });
    out2.textures_delta.clear();
    actions_right
}

#[test]
fn resizable_remainder_leaves_dead_space_after_actions() {
    let right = wide_frame_actions_right(true);
    println!("resizable remainder: Actions right edge at 1920px window = {right:.0}");
    // The resizable remainder locks its first-frame width → table ends early.
    // We can't assert the exact value (egui may size it differently), but we
    // assert the BUG shape: it does NOT reach the window width.
    assert!(
        right < 1920.0 - 50.0,
        "expected dead space (table ended at {right:.0}), but table filled the window"
    );
}

#[test]
fn non_resizable_remainder_fills_window() {
    let right = wide_frame_actions_right(false);
    println!("non-resizable remainder: Actions right edge at 1920px window = {right:.0}");
    assert!(
        right >= 1920.0 - 2.0,
        "non-resizable remainder table did NOT fill the window: Actions right edge {right:.0} < 1920"
    );
}
