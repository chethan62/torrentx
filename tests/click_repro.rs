//! Headless repro: does a click on TEXT inside a full-cell interact layer register?

use egui::{PointerButton, Pos2, RawInput};

fn run_frame(ctx: &egui::Context, pos: Pos2, press: bool, release: bool) -> bool {
    // Returns whether the cell interact layer saw a click THIS frame.
    let mut clicked = false;
    let mut events = vec![egui::Event::PointerMoved(pos)];
    if press {
        events.push(egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::default(),
        });
    }
    if release {
        events.push(egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::default(),
        });
    }
    let full_output = ctx.run_ui(
        RawInput {
            screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(400.0, 300.0))),
            events,
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let cell = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(200.0, 30.0));
                let _ = ui.allocate_rect(cell, egui::Sense::hover());
                // REPRO STACK:
                let cell_resp = ui.interact(cell, egui::Id::new("cell"), egui::Sense::click());
                clicked = cell_resp.clicked();
                // Text drawn on top at the same position:
                ui.horizontal(|ui| {
                    ui.add_space(6.0);
                    ui.add(egui::Label::new("Clickable tracker name").truncate());
                });
            });
        },
    );
    // Consume textures from the pass so the drop assert doesn't fire:
    let mut full_output = full_output; // shadow
    full_output.textures_delta.clear();
    clicked
}

#[test]
fn click_on_text_registers() {
    let ctx = egui::Context::default();
    let text_pos = egui::pos2(60.0, 25.0); // ON the text
    run_frame(&ctx, text_pos, false, false); // hover
    run_frame(&ctx, text_pos, true, false); // press
    let clicked = run_frame(&ctx, text_pos, false, true); // release
    assert!(clicked, "click on text should register on the interact layer");
}

#[test]
fn click_on_empty_cell_registers() {
    let ctx = egui::Context::default();
    let empty_pos = egui::pos2(200.0, 25.0); // empty space
    run_frame(&ctx, empty_pos, false, false);
    run_frame(&ctx, empty_pos, true, false);
    let clicked = run_frame(&ctx, empty_pos, false, true);
    assert!(clicked, "click on empty cell should register too");
}
