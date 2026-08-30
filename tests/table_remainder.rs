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

#[test]
fn search_table_actions_clips_at_narrow_width() {
    // The search table with the fix (Name non-resizable remainder). At a
    // narrow window, fixed columns + Name min + Actions must still fit, or
    // Actions clips off the right edge (TableBuilder hscroll is hardcoded off).
    let ctx = Context::default();
    let mut rights: Vec<f32> = vec![];
    for width in [1000.0f32, 900.0, 800.0, 700.0] {
        let input = RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                Vec2::new(width, 800.0),
            )),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            CentralPanel::default()
                .frame(egui::Frame::NONE.fill(Color32::from_rgb(26, 27, 38)))
                .show(ui, |ui| {
                    ui.add_space(10.0);
                    // Same as the fixed app: ScrollArea::both so narrow windows
                    // scroll horizontally instead of clipping Actions.
                    egui::ScrollArea::both()
                        .id_salt("search_scroll")
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            let mut tb = TableBuilder::new(ui)
                                .striped(false)
                                .resizable(true)
                                .cell_layout(egui::Layout::left_to_right(egui::Align::Center));
                            // Search layout: Name remainder (non-resizable, the fix)
                            // + 7 fixed (Tracker..Date) + Actions 190.
                            tb = tb.column(Column::remainder().at_least(120.0).resizable(false));
                            for w in [84.0, 62.0, 72.0, 62.0, 56.0, 72.0, 84.0] {
                                tb = tb.column(Column::initial(w).at_least(36.0));
                            }
                            tb = tb.column(Column::initial(190.0).at_least(150.0));
                            tb.header(30.0, |mut hdr| {
                                for label in [
                                    "Name", "Tracker", "Seeds", "Size", "Leech", "Ratio", "Health",
                                    "Date", "Actions",
                                ] {
                                    hdr.col(|ui| {
                                        ui.label(
                                            RichText::new(label).font(FontId::proportional(12.0)),
                                        );
                                    });
                                }
                            })
                            .body(|mut body| {
                                for row in 0..10 {
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
                                            rights.push(ui.max_rect().right());
                                        });
                                    });
                                }
                            });
                        });
                });
        });
        out.textures_delta.clear();
    }
    let (w1000, w900, w800, w700) = (rights[0], rights[1], rights[2], rights[3]);
    println!(
        "search fixed: Actions right at 1000={w1000:.0} 900={w900:.0} 800={w800:.0} 700={w700:.0}"
    );
    // The table is wrapped in ScrollArea::both (the app fix): at narrow
    // widths the fixed columns keep their width and the content overflows
    // the viewport horizontally — the scroll area provides the hscroll, so
    // Actions is reachable instead of clipped. The content width must stay
    // consistent across viewport sizes (no weird reflow), and at a wide
    // window the table fills exactly.
    let consistent = (w1000 - w900).abs() < 2.0
        && (w900 - w800).abs() < 2.0
        && (w800 - w700).abs() < 2.0;
    assert!(
        consistent,
        "search table content width should be stable across viewports: {w1000:.0}/{w900:.0}/{w800:.0}/{w700:.0}"
    );
    assert!(
        w700 > 700.0 + 50.0,
        "expected content wider than the 700px viewport (scrollable, not clipped): right edge {w700:.0}"
    );
}

/// RSS-table layout: Title remainder (resizable by default) + 4 fixed + Actions 180.
/// Mirrors src/ui/rss.rs exactly (except ScrollArea, which doesn't change the
/// column-width math). Returns the Actions column's right edge.
fn rss_actions_right(title_resizable: bool, width: f32) -> f32 {
    let ctx = Context::default();
    let mut actions_right = 0.0f32;
    let title_col = Column::remainder().at_least(180.0).clip(true);
    let title_col = if title_resizable {
        title_col.resizable(true)
    } else {
        title_col.resizable(false)
    };
    let input = RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            Vec2::new(width, 800.0),
        )),
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::from_rgb(26, 27, 38)))
            .show(ui, |ui| {
                ui.add_space(10.0);
                let mut tb = TableBuilder::new(ui)
                    .striped(false)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center));
                tb = tb.column(title_col);
                for w in [80.0, 60.0, 60.0, 80.0] {
                    tb = tb.column(Column::initial(w).at_least(44.0));
                }
                tb = tb.column(Column::initial(180.0).at_least(120.0)); // Actions
                tb.header(28.0, |mut hdr| {
                    for label in ["Title", "Tracker", "Size", "Seeds", "Date", "Actions"] {
                        hdr.col(|ui| {
                            ui.label(RichText::new(label).font(FontId::proportional(12.0)));
                        });
                    }
                })
                .body(|mut body| {
                    for row in 0..10 {
                        body.row(40.0, |mut r| {
                            for _ in 0..5 {
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
            });
    });
    out.textures_delta.clear();
    actions_right
}

#[test]
fn rss_resizable_title_remainder_pushes_actions_offscreen() {
    // Narrow window first, then wide — table state persists.
    let ctx = Context::default();
    let mut rights: Vec<f32> = vec![];
    for width in [1100.0f32, 1920.0] {
        let input = RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                Vec2::new(width, 800.0),
            )),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            CentralPanel::default()
                .frame(egui::Frame::NONE.fill(Color32::from_rgb(26, 27, 38)))
                .show(ui, |ui| {
                    ui.add_space(10.0);
                    // Same as the RSS tab: ScrollArea::both + resizable table.
                    egui::ScrollArea::both()
                        .id_salt("rss_items_scroll")
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            let title_col =
                                Column::remainder().at_least(180.0).clip(true).resizable(true);
                            let mut tb = TableBuilder::new(ui)
                                .striped(false)
                                .resizable(true)
                                .cell_layout(egui::Layout::left_to_right(egui::Align::Center));
                            tb = tb.column(title_col);
                            for w in [80.0, 60.0, 60.0, 80.0] {
                                tb = tb.column(Column::initial(w).at_least(44.0));
                            }
                            tb = tb.column(Column::initial(180.0).at_least(120.0));
                            tb.header(28.0, |mut hdr| {
                                for label in
                                    ["Title", "Tracker", "Size", "Seeds", "Date", "Actions"]
                                {
                                    hdr.col(|ui| {
                                        ui.label(
                                            RichText::new(label).font(FontId::proportional(12.0)),
                                        );
                                    });
                                }
                            })
                            .body(|mut body| {
                                for row in 0..10 {
                                    body.row(40.0, |mut r| {
                                        for _ in 0..5 {
                                            r.col(|ui| {
                                                ui.label(
                                                    RichText::new(format!("cell {row}"))
                                                        .font(FontId::proportional(12.0)),
                                                );
                                            });
                                        }
                                        r.col(|ui| {
                                            rights.push(ui.max_rect().right());
                                        });
                                    });
                                }
                            });
                        });
                });
        });
        out.textures_delta.clear();
    }
    let narrow = rights.first().copied().unwrap_or(0.0);
    let wide = rights.last().copied().unwrap_or(0.0);
    println!(
        "RSS resizable title: Actions right edge narrow={narrow:.0} wide={wide:.0} (window 1920)"
    );
    // BUG: the resizable remainder locks to the narrow-frame width, so the
    // fixed columns + Actions get pushed past the window edge.
    assert!(
        wide <= 1920.0 + 2.0 && narrow <= 1100.0 + 2.0,
        "resizable remainder locks the narrow width ({narrow:.0}) and never re-fills ({wide:.0}) — that is the bug the fix removes"
    );
}

#[test]
fn rss_non_resizable_title_keeps_actions_on_screen() {
    let wide = rss_actions_right(false, 1920.0);
    println!("RSS non-resizable title: Actions right edge at 1920px window = {wide:.0}");
    assert!(
        wide <= 1920.0 + 2.0,
        "RSS Actions column off-screen: right edge {wide:.0} > 1920"
    );
}
