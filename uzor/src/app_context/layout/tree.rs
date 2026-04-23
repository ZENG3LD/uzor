use std::collections::HashMap;
use crate::types::rect::Rect;
use crate::types::state::WidgetId;
use super::types::*;

#[derive(Clone, Debug)]
pub struct LayoutTree {
    pub root: LayoutNode,
    pub computed: HashMap<WidgetId, LayoutComputed>,
}

impl LayoutTree {
    pub fn new(root: LayoutNode) -> Self {
        Self {
            root,
            computed: HashMap::new(),
        }
    }

    /// Compute layout for the entire tree given a viewport size
    pub fn compute(&mut self, viewport: Rect) {
        self.computed.clear();
        
        let mut ctx = LayoutContext {
            computed: &mut self.computed,
        };

        // Initial pass: layout the root
        layout_node_at(&self.root, viewport, &mut ctx, 0, None);
    }

    pub fn get_rect(&self, id: &WidgetId) -> Option<Rect> {
        self.computed.get(id).map(|c| c.rect)
    }

    pub fn get_computed(&self, id: &WidgetId) -> Option<&LayoutComputed> {
        self.computed.get(id)
    }
}

struct LayoutContext<'a> {
    computed: &'a mut HashMap<WidgetId, LayoutComputed>,
}

fn layout_node_at(
    node: &LayoutNode,
    target_rect: Rect,
    ctx: &mut LayoutContext,
    parent_z: i32,
    parent_clip: Option<Rect>,
) {
    // 1. Apply margins to get the border box
    let margin = &node.style.margin;
    let x = target_rect.min_x() + margin.left + node.style.offset_x;
    let y = target_rect.min_y() + margin.top + node.style.offset_y;
    
    // 2. Determine actual size (if specific size requested vs target_rect provided by parent)
    // If parent passed a specific rect, we generally respect it (it already calculated flex).
    // If we have Fix size, we enforce it? 
    // For this "minimal" engine, we assume the parent did the flex math and gave us our box.
    
    let width = target_rect.width() - margin.width();
    let height = target_rect.height() - margin.height();
    
    let final_rect = Rect::new(x, y, width.max(0.0), height.max(0.0));
    
    // 3. Compute Content Rect (minus padding)
    let padding = &node.style.padding;
    let content_x = x + padding.left;
    let content_y = y + padding.top;
    let content_w = (width - padding.width()).max(0.0);
    let content_h = (height - padding.height()).max(0.0);
    let content_rect = Rect::new(content_x, content_y, content_w, content_h);
    
    // 4. Handle Clipping
    let mut current_clip = parent_clip;
    if node.flags.contains(LayoutFlags::CLIP_CONTENT) {
        current_clip = match current_clip {
            Some(parent) => Some(parent.intersect(content_rect)), // Intersection logic needed in WidgetRect
            None => Some(content_rect),
        };
    }
    
    // 5. Z-Index
    let z_order = parent_z + node.style.z_index;
    
    // 6. Store computed
    ctx.computed.insert(node.id.clone(), LayoutComputed {
        rect: final_rect,
        content_rect,
        clip_rect: current_clip,
        z_order,
    });
    
    // 7. Layout Children
    match node.style.display {
        Display::Flex => layout_flex(node, content_rect, ctx, z_order, current_clip),
        Display::Stack => layout_stack(node, content_rect, ctx, z_order, current_clip),
        Display::None => {}, // Skip children
        Display::Grid => layout_flex(node, content_rect, ctx, z_order, current_clip), // Fallback
    }
}

fn layout_flex(
    node: &LayoutNode,
    content_rect: Rect,
    ctx: &mut LayoutContext,
    z: i32,
    clip: Option<Rect>,
) {
    let dir = node.style.direction;
    let gap = node.style.gap;
    
    // Simplified Flex:
    // 1. Count fixed vs fill children
    // 2. Allocate fixed
    // 3. Distribute remaining to fill
    
    let mut total_fixed = 0.0;
    let mut fill_count = 0;
    
    for child in &node.children {
        if child.style.display == Display::None || child.style.position == Position::Absolute { continue; }
        
        let size_spec = match dir {
            FlexDirection::Row => child.style.width,
            FlexDirection::Column => child.style.height,
        };
        
        match size_spec {
            SizeSpec::Fix(v) => total_fixed += v,
            SizeSpec::Pct(p) => {
                let basis = match dir {
                    FlexDirection::Row => content_rect.width(),
                    FlexDirection::Column => content_rect.height(),
                };
                total_fixed += basis * p;
            },
            SizeSpec::Fill => fill_count += 1,
            SizeSpec::Content => {
                // To support Content properly we need a measure pass. 
                // For now treat as Fill or Min? Let's treat as "Fit content" -> requires measure.
                // Minimal hack: treat as Fix(0) but let it expand?
                // Let's assume Fix(0) for V1
            }
        }
    }
    
    // Add gaps
    let visible_children = node.children.iter().filter(|c| c.style.display != Display::None && c.style.position != Position::Absolute).count();
    let total_gap = if visible_children > 1 { (visible_children - 1) as f64 * gap } else { 0.0 };
    total_fixed += total_gap;
    
    // Calculate flexible unit
    let available_space = match dir {
        FlexDirection::Row => content_rect.width(),
        FlexDirection::Column => content_rect.height(),
    };
    
    let remaining = (available_space - total_fixed).max(0.0);
    let flex_unit = if fill_count > 0 { remaining / fill_count as f64 } else { 0.0 };
    
    // Layout pass
    let mut cursor = match dir {
        FlexDirection::Row => content_rect.min_x(),
        FlexDirection::Column => content_rect.min_y(),
    };
    
    for child in &node.children {
        if child.style.display == Display::None { continue; }
        
        // Handle Absolute Children separately
        if child.style.position == Position::Absolute {
            // Absolute is relative to parent content rect
             // TODO: Support top/left/right/bottom constraints
             // For now just give it parent content rect? Or 0 size?
             // Let's assume Absolute takes 0 size flow and is placed at top-left of content
             layout_node_at(child, content_rect, ctx, z + 100, clip);
             continue;
        }

        let (child_w, child_h) = match dir {
            FlexDirection::Row => {
                let w = match child.style.width {
                    SizeSpec::Fix(v) => v,
                    SizeSpec::Pct(p) => content_rect.width() * p,
                    SizeSpec::Fill => flex_unit,
                    SizeSpec::Content => 100.0, // Placeholder
                };
                let h = match child.style.height {
                    SizeSpec::Fix(v) => v,
                    SizeSpec::Pct(p) => content_rect.height() * p,
                    SizeSpec::Fill => content_rect.height(),
                    SizeSpec::Content => content_rect.height(), // Stretch cross axis by default
                };
                (w, h)
            },
            FlexDirection::Column => {
                let h = match child.style.height {
                    SizeSpec::Fix(v) => v,
                    SizeSpec::Pct(p) => content_rect.height() * p,
                    SizeSpec::Fill => flex_unit,
                    SizeSpec::Content => 30.0, // Placeholder
                };
                let w = match child.style.width {
                    SizeSpec::Fix(v) => v,
                    SizeSpec::Pct(p) => content_rect.width() * p,
                    SizeSpec::Fill => content_rect.width(),
                    SizeSpec::Content => content_rect.width(), // Stretch cross axis by default
                };
                (w, h)
            }
        };
        
        let child_x = match dir {
            FlexDirection::Row => cursor,
            FlexDirection::Column => content_rect.min_x(),
        };
        let child_y = match dir {
            FlexDirection::Row => content_rect.min_y(),
            FlexDirection::Column => cursor,
        };
        
        let child_rect = Rect::new(child_x, child_y, child_w, child_h);
        layout_node_at(child, child_rect, ctx, z, clip);
        
        // Advance cursor
        match dir {
            FlexDirection::Row => cursor += child_w + gap,
            FlexDirection::Column => cursor += child_h + gap,
        }
    }
}

fn layout_stack(
    node: &LayoutNode,
    content_rect: Rect,
    ctx: &mut LayoutContext,
    z: i32,
    clip: Option<Rect>,
) {
    // Stack: all children get the full content rect
    for (i, child) in node.children.iter().enumerate() {
        if child.style.display == Display::None { continue; }
        
        // Z-index increases for each child in stack unless specified
        let child_z = z + child.style.z_index + (i as i32);
        
        layout_node_at(child, content_rect, ctx, child_z, clip);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flex_column_layout() {
        // Root: VBox
        // - Header: Fix(50)
        // - Content: Fill
        
        let header = LayoutNode::new("header")
            .with_style(LayoutStyle {
                height: SizeSpec::Fix(50.0),
                width: SizeSpec::Fill,
                ..Default::default()
            });

        let content = LayoutNode::new("content")
            .with_style(LayoutStyle {
                height: SizeSpec::Fill,
                width: SizeSpec::Fill,
                ..Default::default()
            });

        let root = LayoutNode::new("root")
            .with_style(LayoutStyle {
                display: Display::Flex,
                direction: FlexDirection::Column,
                width: SizeSpec::Fix(200.0),
                height: SizeSpec::Fix(200.0),
                ..Default::default()
            })
            .with_child(header)
            .with_child(content);

        let mut tree = LayoutTree::new(root);
        let viewport = Rect::new(0.0, 0.0, 200.0, 200.0);
        
        tree.compute(viewport);

        // Check Root
        let root_rect = tree.get_rect(&WidgetId::new("root")).unwrap();
        assert_eq!(root_rect.width, 200.0);
        assert_eq!(root_rect.height, 200.0);

        // Check Header
        let header_rect = tree.get_rect(&WidgetId::new("header")).unwrap();
        assert_eq!(header_rect.y, 0.0);
        assert_eq!(header_rect.height, 50.0);
        assert_eq!(header_rect.width, 200.0);

        // Check Content
        let content_rect = tree.get_rect(&WidgetId::new("content")).unwrap();
        assert_eq!(content_rect.y, 50.0);
        assert_eq!(content_rect.height, 150.0); // 200 - 50
        assert_eq!(content_rect.width, 200.0);
    }

    #[test]
    fn test_flex_row_layout_with_gap() {
        // Root: HBox, gap=10
        // - Left: Fix(50)
        // - Right: Fill

        let left = LayoutNode::new("left")
            .with_style(LayoutStyle {
                width: SizeSpec::Fix(50.0),
                height: SizeSpec::Fill,
                ..Default::default()
            });

        let right = LayoutNode::new("right")
            .with_style(LayoutStyle {
                width: SizeSpec::Fill,
                height: SizeSpec::Fill,
                ..Default::default()
            });

        let root = LayoutNode::new("root")
            .with_style(LayoutStyle {
                display: Display::Flex,
                direction: FlexDirection::Row,
                gap: 10.0,
                width: SizeSpec::Fix(200.0),
                height: SizeSpec::Fix(100.0),
                ..Default::default()
            })
            .with_child(left)
            .with_child(right);

        let mut tree = LayoutTree::new(root);
        let viewport = Rect::new(0.0, 0.0, 200.0, 100.0);
        
        tree.compute(viewport);

        let left_rect = tree.get_rect(&WidgetId::new("left")).unwrap();
        assert_eq!(left_rect.x, 0.0);
        assert_eq!(left_rect.width, 50.0);

        let right_rect = tree.get_rect(&WidgetId::new("right")).unwrap();
        assert_eq!(right_rect.x, 60.0); // 50 + 10 gap
        assert_eq!(right_rect.width, 140.0); // 200 - 50 - 10
    }
}
