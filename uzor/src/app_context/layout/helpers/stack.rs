use crate::layout::types::*;
use crate::types::state::WidgetId;

pub struct Stack;
impl Stack {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(id: impl Into<WidgetId>) -> LayoutNode {
        LayoutNode::new(id)
            .with_style(LayoutStyle {
                display: Display::Stack,
                ..Default::default()
            })
    }
}
