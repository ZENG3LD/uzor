//! Accessibility actions a node can expose to an assistive-technology client.

/// An action an accessibility node supports (invoked by an AT client, e.g. a
/// screen reader). Mirrors the subset of `accesskit::Action` relevant to L0
/// widgets today — see [`super::A11yRole`]'s own doc comment for the same
/// "no `accesskit` dependency here" note.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum A11yAction {
    Click,
    Focus,
    Increment,
    Decrement,
    SetValue,
    Expand,
    Collapse,
}
