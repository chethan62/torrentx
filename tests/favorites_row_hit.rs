//! Regression test: the Favorites row click layer must be scoped to its own row.
//!
//! Taking `ui.max_rect()` at the scroll-area level — i.e. OUTSIDE the row's
//! `Frame` — hands every row the same whole-viewport rect, and egui settles a hit
//! on the last registered widget. Every click therefore acted on the LAST
//! favourite: it opened the wrong magnet (a real, external action) and made the
//! per-row magnet/download/Remove buttons dead on every row but the last.
//!
//! The app takes the rect INSIDE the row's Frame (see `ui/favorites.rs`), which
//! also matches the RSS sidebar pattern. These tests pin both halves of that:
//! the per-row geometry, and that a click resolves to the row under the pointer.

use egui::{PointerButton, Pos2, RawInput, Rect};

/// Runs one offscreen frame over a two-row list.
///
/// `in_frame = true` mirrors the app (rect taken inside the row's Frame);
/// `false` reproduces the old bug (rect taken at the scroll-area level).
/// Returns each row's registered rect and which row's layer fired.
fn run(
    ctx: &egui::Context,
    pos: Pos2,
    press: bool,
    release: bool,
    in_frame: bool,
) -> (Vec<Rect>, Option<usize>) {
    let mut rects: Vec<Rect> = vec![];
    let mut clicked: Option<usize> = None;
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
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(400.0, 300.0))),
            events,
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for i in 0..2 {
                        egui::Frame::NONE
                            .inner_margin(egui::Margin::symmetric(8, 8))
                            .show(ui, |ui| {
                                if in_frame {
                                    let r = ui.interact(
                                        ui.max_rect(),
                                        egui::Id::new(("row", i)),
                                        egui::Sense::click(),
                                    );
                                    if r.clicked() {
                                        clicked = Some(i);
                                    }
                                    rects.push(r.rect);
                                }
                                ui.label(format!("row {i}"));
                            });
                        if !in_frame {
                            let r = ui.interact(
                                ui.max_rect(),
                                egui::Id::new(("row", i)),
                                egui::Sense::click(),
                            );
                            if r.clicked() {
                                clicked = Some(i);
                            }
                            rects.push(r.rect);
                        }
                    }
                });
            });
        },
    );
    let mut full_output = full_output;
    full_output.textures_delta.clear();
    (rects, clicked)
}

/// The bug's mechanism: each row's layer must start at that row's top. With the
/// old pattern every rect was the viewport, so both rows reported the same top.
#[test]
fn each_row_layer_is_scoped_to_its_own_row() {
    let ctx = egui::Context::default();
    let (rects, _) = run(&ctx, egui::pos2(5.0, 5.0), false, false, true);
    assert_eq!(rects.len(), 2);
    assert_ne!(rects[0], rects[1], "row layers must not share one rect");
    assert!(
        rects[0].top() < rects[1].top(),
        "row 1's layer must start below row 0's (got {:?} then {:?})",
        rects[0],
        rects[1]
    );
}

/// The user-visible bug: clicking the first row acted on the second one.
#[test]
fn click_on_first_row_acts_on_the_first_row() {
    let ctx = egui::Context::default();
    let (rects, _) = run(&ctx, egui::pos2(5.0, 5.0), false, false, true);
    // Just inside row 0, above row 1's top (the layer extends to the viewport
    // bottom, so its centre would sit inside row 1's layer).
    let p = egui::pos2(rects[0].center().x, rects[0].top() + 2.0);
    run(&ctx, p, false, false, true); // hover
    run(&ctx, p, true, false, true); // press
    let (_, clicked) = run(&ctx, p, false, true, true); // release
    assert_eq!(
        clicked,
        Some(0),
        "a click on the first favourite must act on the first favourite"
    );
}

/// The other direction, so the test cannot pass by simply preferring row 0.
#[test]
fn click_on_second_row_acts_on_the_second_row() {
    let ctx = egui::Context::default();
    let (rects, _) = run(&ctx, egui::pos2(5.0, 5.0), false, false, true);
    let p = egui::pos2(rects[1].center().x, rects[1].top() + 2.0);
    run(&ctx, p, false, false, true);
    run(&ctx, p, true, false, true);
    let (_, clicked) = run(&ctx, p, false, true, true);
    assert_eq!(clicked, Some(1));
}
