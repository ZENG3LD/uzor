use crate::layout::types::*;
use crate::types::state::WidgetId;

pub struct ScrollView;

impl ScrollView {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(id: impl Into<WidgetId>) -> LayoutNode {
        let mut flags = LayoutFlags::CLIP_CONTENT;
        flags.insert(LayoutFlags::SCROLL_Y);
        
        LayoutNode::new(id)
            .with_style(LayoutStyle {
                display: Display::Flex, // Wrapper acts as flex container for content
                direction: FlexDirection::Column,
                ..Default::default()
            })
            .with_flags(flags)
    }
}

pub struct Modal;
impl Modal {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(id: impl Into<WidgetId>) -> LayoutNode {
        // Modal is usually a full-screen overlay Stack
        LayoutNode::new(id)
            .with_style(LayoutStyle {
                display: Display::Stack,
                position: Position::Absolute,
                width: SizeSpec::Fill,
                height: SizeSpec::Fill,
                z_index: 1000, // High Z-index
                ..Default::default()
            })
            .with_kind(LayoutKind::Overlay)
    }
}
