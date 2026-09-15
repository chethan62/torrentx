//! Regression: a scroll-list row's full-row click layer must cover its OWN band.
//!
//! `ui.max_rect()` is not the row rect: inside a Frame inside a ScrollArea it is the
//! AVAILABLE rect, so every row's layer stretched to the bottom of the viewport. All
//! layers then overlapped, egui settles a hit on the LAST one registered, and so:
//!   - clicking feed 1 selected the last feed (the Favorites list opened the last
//!     favourite's magnet the same way), and
//!   - the row's own icon buttons (Refresh/Edit/Delete, Magnet/Download/Remove)
//!     never fired unless the row happened to be the last one.
//!
//! The fix allocates the row band up front, which (a) makes the layers disjoint and
//! (b) registers the row BELOW its own children, so the buttons still win.
//!
//! Both patterns are exercised: `fixed` must pass all three expectations, and `buggy`
//! must fail them — a test that cannot fail proves nothing.

use egui::{Pos2, Rect, Vec2};

#[derive(Default)]
struct Outcome {
    bands: Vec<(usize, Rect)>,
    row_clicked: Option<usize>,
    button_clicked: Option<usize>,
}

fn run(ctx: &egui::Context, click: Option<Pos2>, pattern: &str) -> Outcome {
    let mut out = Outcome::default();
    let mut events = vec![];
    if let Some(pos) = click {
        events.push(egui::Event::PointerMoved(pos));
        for pressed in [true, false] {
            events.push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            });
        }
    }
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(420.0, 300.0))),
        events,
        ..Default::default()
    };
    let mut output = ctx.run_ui(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("list")
                .max_height(ui.available_height())
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for i in 0..3 {
                        if pattern == "fixed" {
                            // The shipped pattern: allocate the band, then draw into it
                            // with a child ui. `new_child` (not `scope_builder`) is what
                            // keeps the next row's band exactly adjacent — a scope also
                            // consumes space in the parent, so bands overlapped by 1px.
                            let (band, resp) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), 26.0),
                                egui::Sense::click(),
                            );
                            out.bands.push((i, band));
                            if resp.clicked() {
                                out.row_clicked = Some(i);
                            }
                            {
                                let mut content = ui.new_child(
                                    egui::UiBuilder::new()
                                        .max_rect(band.shrink2(Vec2::new(10.0, 4.0)))
                                        .layout(egui::Layout::top_down(egui::Align::Min)),
                                );
                                let ui = &mut content;
                                ui.horizontal(|ui| {
                                    if ui.button("x").clicked() {
                                        out.button_clicked = Some(i);
                                    }
                                    ui.label(format!("row {i}"));
                                });
                            }
                        } else {
                            // The pre-fix pattern: interact inside the Frame with max_rect.
                            egui::Frame::NONE
                                .inner_margin(egui::Margin::symmetric(10, 4))
                                .show(ui, |ui| {
                                    let resp = ui.interact(
                                        ui.max_rect(),
                                        egui::Id::new(("row", i)),
                                        egui::Sense::click(),
                                    );
                                    out.bands.push((i, resp.rect));
                                    if resp.clicked() {
                                        out.row_clicked = Some(i);
                                    }
                                    ui.horizontal(|ui| {
                                        if ui.button("x").clicked() {
                                            out.button_clicked = Some(i);
                                        }
                                        ui.label(format!("row {i}"));
                                    });
                                });
                        }
                    }
                });
        });
    });
    output.textures_delta.clear();
    out
}

/// Point inside row 1's band, on the far right (blank space, not the button).
fn blank_point(o: &Outcome) -> Pos2 {
    let band = o.bands[1].1;
    Pos2::new(band.max.x - 20.0, band.center().y)
}

/// Point over row 1's button (left edge of its band).
fn button_point(o: &Outcome) -> Pos2 {
    let band = o.bands[1].1;
    Pos2::new(band.min.x + 14.0, band.center().y)
}

#[test]
fn fixed_pattern_is_disjoint_and_routes_to_the_right_row() {
    let ctx = egui::Context::default();
    let layout = run(&ctx, None, "fixed");

    // 1. Bands must not overlap: overlapping layers are what misroute clicks.
    for a in 0..layout.bands.len() {
        for b in (a + 1)..layout.bands.len() {
            assert!(
                !layout.bands[a].1.intersects(layout.bands[b].1),
                "row {a} band {:?} overlaps row {b} band {:?}",
                layout.bands[a].1,
                layout.bands[b].1
            );
        }
    }

    // 2. Clicking row 1's blank area acts on row 1.
    let pos = blank_point(&layout);
    let clicked = run(&ctx, Some(pos), "fixed");
    assert_eq!(
        clicked.row_clicked,
        Some(1),
        "a click on row 1 must act on row 1, not another row"
    );
    assert_eq!(clicked.button_clicked, None, "blank space is not a button");

    // 3. And the row's own button still wins its click.
    let pos = button_point(&layout);
    let on_btn = run(&ctx, Some(pos), "fixed");
    assert_eq!(
        on_btn.button_clicked,
        Some(1),
        "the row's own button must receive the click"
    );
}

#[test]
fn buggy_pattern_is_detected_by_the_same_expectations() {
    let ctx = egui::Context::default();
    let layout = run(&ctx, None, "buggy");

    let overlapping = layout.bands.iter().enumerate().any(|(a, (_, ra))| {
        layout
            .bands
            .iter()
            .skip(a + 1)
            .any(|(_, rb)| ra.intersects(*rb))
    });
    assert!(
        overlapping,
        "the pre-fix pattern is expected to produce overlapping bands — \
         if this is no longer true, this test (and its comment) is stale"
    );

    let pos = blank_point(&layout);
    let clicked = run(&ctx, Some(pos), "buggy");
    assert_ne!(
        clicked.row_clicked,
        Some(1),
        "pre-fix, row 1's blank area did NOT act on row 1 — that is the bug this \
         regression test exists for"
    );
}
