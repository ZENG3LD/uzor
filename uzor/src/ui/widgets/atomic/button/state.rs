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
