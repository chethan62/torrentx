//! Class-driven button visuals.
//!
//! Spike: `egui` 0.37 ships a widget-style provider (`Context::replace_widget_theme`)
//! that resolves a widget's style from its *classes* and state. One provider here
//! replaces the `.fill()` / `.corner_radius()` / text-colour block that every call
//! site was re-specifying, so a button says *what it is* (`primary`, `ghost`) and
//! the palette + tokens decide how that looks.
//!
//! Ceiling: `ButtonStyle` carries one `min_size`, so per-row sizes (the Search
//! button's computed width) stay at the call site. The provider owns paint.
#![allow(dead_code)] // the sweep converts call sites one at a time

use eframe::egui::theme::StyleProvider;
use eframe::egui::widget_style::{AtomLayoutStyle, ButtonStyle, StyleArgs, TextVisuals};
use eframe::egui::{Color32, FontId, Frame};

use crate::themes::Pal;
use crate::ui::{R_MD, SP2, SP3, margin, tint};

/// The accent-filled call to action (Search, Refresh).
pub(crate) const PRIMARY: &str = "tx::primary";
/// The same button while it is busy: accent, muted.
pub(crate) const PRIMARY_SOFT: &str = "tx::soft";
/// Quiet buttons: tab chrome, row actions, in-field glyphs.
pub(crate) const GHOST: &str = "tx::ghost";

pub(crate) struct ButtonTheme {
    pub(crate) pal: Pal,
    pub(crate) fs: f32,
}

impl StyleProvider<ButtonStyle> for ButtonTheme {
    fn style(&mut self, args: &StyleArgs<'_>) -> ButtonStyle {
        let pal = &self.pal;
        let (fill, text) = if args.has_class(PRIMARY) {
            (pal.accent, Color32::WHITE)
        } else if args.has_class(PRIMARY_SOFT) {
            (tint(pal.accent, 190), Color32::WHITE)
        } else if args.has_class(GHOST) {
            (Color32::TRANSPARENT, pal.sub)
        } else {
            (pal.surface, pal.sub)
        };
        ButtonStyle {
            atom_layout: AtomLayoutStyle {
                gap: SP2,
                frame: Frame::new()
                    .fill(fill)
                    .corner_radius(R_MD)
                    .inner_margin(margin(SP3, SP2)),
                text_style: TextVisuals {
                    font_id: FontId::proportional(self.fs),
                    color: text,
                },
                ..Default::default()
            },
        }
    }
}
