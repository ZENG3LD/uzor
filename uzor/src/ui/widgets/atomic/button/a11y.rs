//! Button's per-frame accessibility node.

use crate::a11y::{A11yAction, A11yNode, A11yRole};
use crate::types::{Rect, WidgetId};

use super::render::ButtonView;

/// This button's accessibility node for the current frame. Pure — no
/// coordinator/tree coupling, mirrors `draw_button`'s own shape (per-frame
/// view in, value out). The caller pushes the result into its own
/// `A11yTree` the same frame it calls `draw_button`.
pub fn node_for(id: impl Into<WidgetId>, rect: Rect, view: &ButtonView<'_>, focused: bool) -> A11yNode {
    let name = view.text.unwrap_or_default().to_owned();
    A11yNode::new(id, A11yRole::Button, name, rect)
        .with_disabled(view.disabled)
        .with_focused(focused)
        .with_action(A11yAction::Click)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect() -> Rect {
        Rect::new(10.0, 10.0, 100.0, 32.0)
    }

    fn view(text: Option<&str>, disabled: bool) -> ButtonView<'_> {
        ButtonView {
            icon: None,
            text,
            active: false,
            disabled,
            active_border: None,
            hover_chevron: None,
        }
    }

    #[test]
    fn node_for_reports_role_name_bounds_action_and_state() {
        let disabled_view = view(Some("Save"), false);
        let node = node_for("save-btn", rect(), &disabled_view, true);
        assert_eq!(node.role, A11yRole::Button);
        assert_eq!(node.name, "Save");
        assert_eq!(node.bounds, rect());
        assert!(node.actions.contains(&A11yAction::Click));
        assert!(!node.disabled);
        assert!(node.focused);

        let empty_view = view(None, true);
        let icon_only = node_for("icon-btn", rect(), &empty_view, false);
        assert_eq!(icon_only.name, "");
        assert!(icon_only.disabled);
        assert!(!icon_only.focused);
    }
}
