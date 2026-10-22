//! Design tokens: the only place spacing/radius/icon/motion numbers live.
//! Rule: 4 px grid for space, 3 radii, 4 icon sizes. Enforced by
//! `scripts/ui-standards.sh` (no numeric literals elsewhere in `src/ui`).

/// Vertical/horizontal rhythm. Use the nearest step, never a literal.
pub(crate) const SP1: f32 = 2.0; // hairline (inside a badge)
pub(crate) const SP2: f32 = 4.0; // label to its control
pub(crate) const SP3: f32 = 8.0; // items in a group
pub(crate) const SP4: f32 = 12.0; // groups in a row
pub(crate) const SP5: f32 = 16.0; // sections
pub(crate) const SP6: f32 = 24.0; // page padding
pub(crate) const SP7: f32 = 32.0; // empty-state block

/// Corner radii: inputs/buttons, panels, dialogs.
pub(crate) const R_SM: f32 = 4.0;
pub(crate) const R_MD: f32 = 8.0;
pub(crate) const R_LG: f32 = 12.0;

/// Icon box sizes. Stroke is 2.0 in every SVG (see assets/icons/README.md).
pub(crate) const ICON_XS: f32 = 10.0; // inline status glyph
pub(crate) const ICON_SM: f32 = 12.0; // button glyph beside text
pub(crate) const ICON_MD: f32 = 14.0; // toolbar / row action
pub(crate) const ICON_LG: f32 = 16.0; // header, empty state

/// Table cell inset — text must not touch the cell edge.
pub(crate) const CELL_PAD_X: f32 = 10.0;
/// Row hover/selection transition.
pub(crate) const MOTION_MS: u64 = 120;

/// `Margin::symmetric` from the space scale (egui margins are i8).
pub(crate) fn margin(x: f32, y: f32) -> eframe::egui::Margin {
    eframe::egui::Margin::symmetric(x as i8, y as i8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn space_scale_is_a_4px_grid() {
        for v in [SP1, SP2, SP3, SP4, SP5, SP6, SP7] {
            if v == SP1 {
                continue; // hairline step is the intentional exception
            }
            assert_eq!(v % 4.0, 0.0, "{v} is off the 4px grid");
        }
    }

    #[test]
    fn icon_sizes_are_ordered_and_smaller_than_their_row() {
        assert!(ICON_XS < ICON_SM && ICON_SM < ICON_MD && ICON_MD < ICON_LG);
        assert!(ICON_LG <= 16.0, "icons larger than 16px crowd a 28px row");
    }
}
