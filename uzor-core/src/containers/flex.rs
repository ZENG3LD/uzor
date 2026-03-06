use crate::layout::types::*;
use crate::types::state::WidgetId;

pub struct VBox;
impl VBox {
    pub fn new(id: impl Into<WidgetId>) -> LayoutNode {
        LayoutNode::new(id)
            .with_style(LayoutStyle {
                display: Display::Flex,
                direction: FlexDirection::Column,
                ..Default::default()
            })
    }
}

pub struct HBox;
impl HBox {
    pub fn new(id: impl Into<WidgetId>) -> LayoutNode {
        LayoutNode::new(id)
            .with_style(LayoutStyle {
                display: Display::Flex,
                direction: FlexDirection::Row,
                ..Default::default()
            })
    }
}
