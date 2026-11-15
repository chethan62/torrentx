//! Class-driven button visuals.
//!
//! `egui` 0.37 ships a widget-style provider (`Context::replace_widget_theme`) that
//! resolves a widget's style from its *classes* and state. One provider here
//! replaces the `.fill()` / `.stroke()` / `.corner_radius()` block that every call
//! site was re-specifying: a button says *what it is* (`primary`, `chip`, `ghost`)
//! and the palette + tokens decide how that looks.
//!
//! Left inline on purpose: buttons whose paint is per-item state rather than
//! style — the theme swatch (its fill *is* the colour it sets), the indexer chip's
//! warning border, and the two fade tints. A class for each would be one call site
//! wearing a constant.
//!
//! Ceiling: `ButtonStyle` carries one `min_size`, so per-row sizes (the Search
//! button's computed width) stay at the call site. The provider owns paint.
#![allow(dead_code)] // converted a page at a time

use eframe::egui::class::HasClasses as _;
use eframe::egui::theme::{DefaultStyle, StyleProvider};
use eframe::egui::widget_style::{
    AtomLayoutStyle, ButtonStyle, StyleArgs, TextVisuals, WidgetState,
};
use eframe::egui::{Button, Color32, CornerRadius, FontId, Frame, Margin, Stroke};

use crate::themes::Pal;
use crate::ui::{R_LG, R_MD, R_SM, SP2, SP3, margin, tint};

/// The accent-filled call to action (Search).
pub(crate) const PRIMARY: &str = "tx::primary";
/// The same button while it is busy: accent, muted.
pub(crate) const PRIMARY_SOFT: &str = "tx::soft";
/// No chrome at all: tab labels, menu rows, empty-state shortcuts.
pub(crate) const GHOST: &str = "tx::ghost";
/// A glyph that fills in on hover and is otherwise invisible (the search ×).
pub(crate) const ICON_GHOST: &str = "tx::icon-ghost";
/// Quiet bordered row: settings entries, dialog buttons.
pub(crate) const SURFACE: &str = "tx::surface";
/// Bordered but unfilled: secondary actions that must not compete.
pub(crate) const OUTLINE: &str = "tx::outline";
/// A pill/segment: category groups. Pair with `Button::CLASS_SELECTED`.
pub(crate) const CHIP: &str = "tx::chip";
/// Confirming action (Save): green, tinted, bordered.
pub(crate) const TONE_GREEN: &str = "tx::green";
/// Radius overrides: a pill (12 px) or a key cap (4 px) is the same paint at a
/// different corner, so this is a utility class, not a variant of each class.
pub(crate) const RADIUS_SM: &str = "tx::r-sm";
pub(crate) const RADIUS_LG: &str = "tx::r-lg";

pub(crate) struct ButtonTheme {
    pub(crate) pal: Pal,
    pub(crate) fs: f32,
}

impl StyleProvider<ButtonStyle> for ButtonTheme {
    fn style(&mut self, args: &StyleArgs<'_>) -> ButtonStyle {
        let pal = &self.pal;
        // Mirror egui's own default padding, so a converted site keeps its size;
        // the primary button is the one that sets a wider, deliberate padding.
        let pad: Margin = args.style.spacing.button_padding.into();
        let hovered = matches!(args.state, WidgetState::Hovered | WidgetState::Active);

        // The hand-written sites inherited egui's 1 px `bg_stroke`; keeping it is
        // what makes this sweep pixel-neutral instead of "almost".
        let edge = Stroke::new(1.0_f32, pal.border);
        let (fill, stroke, text, corner, pad) = if args.has_class(PRIMARY) {
            (pal.accent, edge, Color32::WHITE, R_MD, margin(SP3, SP2))
        } else if args.has_class(PRIMARY_SOFT) {
            (
                tint(pal.accent, 190),
                edge,
                Color32::WHITE,
                R_MD,
                margin(SP3, SP2),
            )
        } else if args.has_class(ICON_GHOST) {
            let (fill, stroke) = if hovered {
                (pal.surface2, edge)
            } else {
                (Color32::TRANSPARENT, Stroke::NONE)
            };
            // No inner margin: this glyph lives in a rect the caller already
            // sized (the in-field clear button), so padding would grow the box.
            (fill, stroke, pal.sub, R_SM, Margin::ZERO)
        } else if args.has_class(GHOST) {
            (Color32::TRANSPARENT, Stroke::NONE, pal.sub, R_SM, pad)
        } else if args.has_class(CHIP) {
            if args.classes.has_class(&Button::CLASS_SELECTED) {
                (pal.accent, edge, pal.bg, R_MD, pad)
            } else {
                (pal.surface2, edge, pal.text, R_MD, pad)
            }
        } else if args.has_class(SURFACE) {
            (
                pal.surface,
                Stroke::new(1.0_f32, pal.border),
                pal.sub,
                R_MD,
                pad,
            )
        } else if args.has_class(OUTLINE) {
            (
                Color32::TRANSPARENT,
                Stroke::new(1.0_f32, pal.border),
                pal.sub,
                R_SM,
                pad,
            )
        } else if args.has_class(TONE_GREEN) {
            (
                tint(pal.green, 18),
                Stroke::new(1.0_f32, tint(pal.green, 80)),
                pal.green,
                R_SM,
                pad,
            )
        } else {
            // Anything not classified here keeps egui's own look: a ComboBox is
            // rendered through ButtonStyle too, and overriding its arm turned the
            // Category/Indexer pickers from their theme fill into a plain button.
            let mut default = DefaultStyle;
            return default.style(args);
        };

        let corner = if args.has_class(RADIUS_SM) {
            R_SM
        } else if args.has_class(RADIUS_LG) {
            R_LG
        } else {
            corner
        };

        ButtonStyle {
            atom_layout: AtomLayoutStyle {
                gap: SP2,
                frame: Frame::new()
                    .fill(fill)
                    .stroke(stroke)
                    .corner_radius(CornerRadius::same(corner as u8))
                    .inner_margin(pad),
                text_style: TextVisuals {
                    font_id: FontId::proportional(self.fs),
                    color: text,
                },
                ..Default::default()
            },
        }
    }
}
