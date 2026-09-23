//! A single accessibility node.

use crate::types::{Rect, WidgetId};
use super::{A11yAction, A11yRole};

/// One accessibility node, 1:1 shaped after AccessKit's `Node`: role,
/// label/name, bounds, actions, toggled/checked/value/disabled/focused
/// state, and children — carrying uzor's own [`WidgetId`] instead of an
/// AccessKit `NodeId` so L0 never depends on the `accesskit` crate.
#[derive(Clone, Debug, PartialEq)]
pub struct A11yNode {
    pub id: WidgetId,
    pub role: A11yRole,
    pub name: String,
    pub bounds: Rect,
    pub actions: Vec<A11yAction>,
    pub toggled: Option<bool>,
    pub checked: Option<bool>,
    pub value: Option<String>,
    pub disabled: bool,
    pub focused: bool,
    pub children: Vec<WidgetId>,
}

impl A11yNode {
    /// Construct a new node with no actions/state — use the `with_*`
    /// builders to fill in the rest.
    pub fn new(id: impl Into<WidgetId>, role: A11yRole, name: impl Into<String>, bounds: Rect) -> Self {
        Self {
            id: id.into(),
            role,
            name: name.into(),
            bounds,
            actions: Vec::new(),
            toggled: None,
            checked: None,
            value: None,
            disabled: false,
            focused: false,
            children: Vec::new(),
        }
    }

    /// Add a supported action (e.g. `Click`).
    pub fn with_action(mut self, action: A11yAction) -> Self {
        self.actions.push(action);
        self
    }

    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn with_focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn with_toggled(mut self, toggled: bool) -> Self {
        self.toggled = Some(toggled);
        self
    }

    pub fn with_checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn with_children(mut self, children: Vec<WidgetId>) -> Self {
        self.children = children;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect() -> Rect {
        Rect::new(0.0, 0.0, 100.0, 40.0)
    }

    #[test]
    fn new_sets_base_fields_with_empty_state() {
        let node = A11yNode::new("save-btn", A11yRole::Button, "Save", rect());
        assert_eq!(node.id, WidgetId::from("save-btn"));
        assert_eq!(node.role, A11yRole::Button);
        assert_eq!(node.name, "Save");
        assert_eq!(node.bounds, rect());
        assert!(node.actions.is_empty());
        assert_eq!(node.toggled, None);
        assert_eq!(node.checked, None);
        assert_eq!(node.value, None);
        assert!(!node.disabled);
        assert!(!node.focused);
        assert!(node.children.is_empty());
    }

    #[test]
    fn with_action_appends_in_order() {
        let node = A11yNode::new("field", A11yRole::TextInput, "Name", rect())
            .with_action(A11yAction::Focus)
            .with_action(A11yAction::SetValue);
        assert_eq!(node.actions, vec![A11yAction::Focus, A11yAction::SetValue]);
    }

    #[test]
    fn with_disabled_and_focused_set_flags() {
        let node = A11yNode::new("btn", A11yRole::Button, "Save", rect())
            .with_disabled(true)
            .with_focused(true);
        assert!(node.disabled);
        assert!(node.focused);
    }

    #[test]
    fn with_toggled_and_checked_wrap_in_some() {
        let node = A11yNode::new("switch", A11yRole::Switch, "Dark mode", rect())
            .with_toggled(true)
            .with_checked(false);
        assert_eq!(node.toggled, Some(true));
        assert_eq!(node.checked, Some(false));
    }

    #[test]
    fn with_value_sets_value() {
        let node = A11yNode::new("slider", A11yRole::Slider, "Volume", rect())
            .with_value("50%");
        assert_eq!(node.value.as_deref(), Some("50%"));
    }

    #[test]
    fn with_children_sets_child_list() {
        let children = vec![WidgetId::from("item-1"), WidgetId::from("item-2")];
        let node = A11yNode::new("list", A11yRole::Group, "Items", rect())
            .with_children(children.clone());
        assert_eq!(node.children, children);
    }
}
