//! Button persistent state.
//!
//! Most buttons are stateless — interaction is driven by `WidgetState`
//! the coordinator gives the renderer (Normal/Hovered/Pressed/Disabled).
//! Toggle / Checkbox / Tab variants additionally need a persistent
//! `toggled`/`checked`/`active` bit; that lives inside the variant data
//! itself (see `types.rs`) so each instance is self-describing.
//!
//! This module is reserved for future state that doesn't naturally fit
//! into the variant data — e.g. press-and-hold timers for repeat-fire
//! buttons, color-swatch popup open flags, etc.

/// Placeholder. Concrete fields added as concrete need arises.
#[derive(Debug, Default, Clone)]
pub struct ButtonState;

/// Which half of a split button the pointer is currently over.
///
/// Used by `draw_toolbar_split_icon_button` and
/// `draw_toolbar_split_line_width_button` to track per-instance hover state
/// so the caller can register two separate hit-rects and report them back.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SplitButtonHoverZone {
    /// Pointer is not over either half.
    #[default]
    None,
    /// Pointer is over the main (left) area.
    Main,
    /// Pointer is over the chevron (right) dropdown trigger.
    Chevron,
}
