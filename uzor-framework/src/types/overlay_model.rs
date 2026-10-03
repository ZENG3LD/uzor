//! App-declared overlay frames: the view models [`App::overlay_model`]
//! returns per open overlay per frame (design §6.3-6.4).
//!
//! The kernel draws the frame these models describe (title, tabs, close
//! button, menu rows, backdrop) with the library composite widgets; the app
//! draws only the body. The models are pure owned data — the mapping onto
//! the library composites' per-frame view types lives in the kernel's
//! compose phase, so `types/` keeps its leaf-only import rule.
//!
//! [`App::overlay_model`]: crate::handle::App::overlay_model

/// What the kernel draws around one open overlay's body.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum OverlayModel {
    /// A modal dialog frame (title, optional tabs, optional footer buttons,
    /// close button, dim backdrop).
    Modal(ModalModel),
    /// A popup frame (border and shadow only).
    Popup,
    /// A dropdown list frame with rows the kernel draws and hit-registers.
    Dropdown(DropdownModel),
    /// A context menu frame with rows the kernel draws and hit-registers.
    ContextMenu(ContextMenuModel),
    /// A tooltip: one text line, no hit zones (pointer-transparent).
    Tooltip(TooltipModel),
    /// The kernel draws nothing and registers nothing; the app's
    /// `overlay_body` paints the whole rect (colour pickers and other
    /// custom composites).
    #[default]
    Custom,
}

/// A modal dialog frame declaration.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModalModel {
    /// Title in the header; empty = no header row unless tabs or footer
    /// demand one.
    pub title: String,
    /// Tab labels, left to right; more than zero selects the tabbed frame.
    pub tabs: Vec<String>,
    /// Active tab index (drawn highlighted; clicks arrive as
    /// `OverlayIntent::ModalTab`).
    pub active_tab: usize,
    /// Footer action buttons, left to right; empty = no footer. Activating
    /// one closes the modal with `CloseCause::Item` (library semantics).
    pub footer: Vec<String>,
    /// Show the close button (Esc and the dismissal policies are
    /// unaffected).
    pub closable: bool,
    /// Register resize handles along the frame border.
    pub resizable: bool,
    /// Dim everything behind the dialog (default for modals).
    pub backdrop: bool,
}

impl ModalModel {
    /// A plain titled dialog: close button, dim backdrop, no tabs or footer.
    pub fn titled(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            closable: true,
            backdrop: true,
            ..Self::default()
        }
    }
}

/// A dropdown list frame declaration.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DropdownModel {
    /// Row labels, top to bottom. Activating row `i` arrives as
    /// `OverlayIntent::DropdownItem` with the item widget id
    /// (`{host}:item:{i}`).
    pub items: Vec<String>,
    /// Row drawn as current (check-marked by the theme), if any.
    pub selected: Option<usize>,
}

/// A context menu frame declaration.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ContextMenuModel {
    /// Rows, top to bottom.
    pub items: Vec<ContextMenuEntry>,
    /// Optional title row above the items.
    pub title: Option<String>,
}

/// One context menu row.
#[derive(Clone, Debug, PartialEq)]
pub enum ContextMenuEntry {
    /// An actionable row; activating it arrives as
    /// `OverlayIntent::ContextMenuItem` with the row index.
    Item {
        /// Display label.
        label: String,
        /// Greyed out, not clickable.
        disabled: bool,
    },
    /// A separator line.
    Separator,
}

/// A tooltip frame declaration.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TooltipModel {
    /// The text to show.
    pub text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titled_modal_defaults() {
        let m = ModalModel::titled("Settings");
        assert_eq!(m.title, "Settings");
        assert!(m.closable && m.backdrop);
        assert!(m.tabs.is_empty() && m.footer.is_empty() && !m.resizable);
        assert_eq!(OverlayModel::default(), OverlayModel::Custom);
    }
}
