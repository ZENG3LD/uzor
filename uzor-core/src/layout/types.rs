use crate::types::rect::WidgetRect;
use crate::types::state::WidgetId;

/// Layout display mode
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Display {
    /// Flexbox layout (row or column)
    #[default]
    Flex,
    /// Stack layout (z-index overlay)
    Stack,
    /// Grid layout (simplified)
    Grid,
    /// Hidden (not participating in layout)
    None,
}

/// Flex direction
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FlexDirection {
    #[default]
    Row,
    Column,
}

/// Alignment of items along the cross axis
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AlignItems {
    #[default]
    Stretch,
    Start,
    End,
    Center,
}

/// Justification of content along the main axis
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum JustifyContent {
    #[default]
    Start,
    End,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// Positioning type
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Position {
    /// Relative to flow
    #[default]
    Relative,
    /// Absolute to parent's content box
    Absolute,
}

/// Size specification
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SizeSpec {
    /// Fixed size in pixels
    Fix(f64),
    /// Percentage of parent size (0.0 - 1.0)
    Pct(f64),
    /// Fill available space (flex grow)
    Fill,
    /// Size to content
    Content,
}

impl Default for SizeSpec {
    fn default() -> Self {
        Self::Content
    }
}

/// Insets (padding, margin)
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Insets {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl Insets {
    pub fn all(val: f64) -> Self {
        Self { top: val, right: val, bottom: val, left: val }
    }

    pub fn symmetric(v: f64, h: f64) -> Self {
        Self { top: v, right: h, bottom: v, left: h }
    }
    
    pub fn width(&self) -> f64 { self.left + self.right }
    pub fn height(&self) -> f64 { self.top + self.bottom }
}

/// Layout style properties
#[derive(Clone, Debug, Default)]
pub struct LayoutStyle {
    pub display: Display,
    pub direction: FlexDirection,
    pub align_items: AlignItems,
    pub justify_content: JustifyContent,
    pub position: Position,
    
    pub gap: f64,
    pub padding: Insets,
    pub margin: Insets,
    
    pub width: SizeSpec,
    pub height: SizeSpec,
    pub min_width: Option<f64>,
    pub max_width: Option<f64>,
    pub min_height: Option<f64>,
    pub max_height: Option<f64>,
    
    pub offset_x: f64,
    pub offset_y: f64,
    pub z_index: i32,
}

/// Kind of layout node
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LayoutKind {
    #[default]
    Container,
    Widget,
    Overlay,
}

/// A node in the layout definition tree
#[derive(Clone, Debug)]
pub struct LayoutNode {
    pub id: WidgetId,
    pub kind: LayoutKind,
    pub style: LayoutStyle,
    pub children: Vec<LayoutNode>,
    pub flags: LayoutFlags,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LayoutFlags(u32);

impl LayoutFlags {
    pub const NONE: Self = Self(0);
    pub const CLIP_CONTENT: Self = Self(1 << 0);
    pub const SCROLL_Y: Self = Self(1 << 1);
    pub const SCROLL_X: Self = Self(1 << 2);
    pub const IS_ROOT: Self = Self(1 << 3);

    pub fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
    
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }
}

impl std::ops::BitOr for LayoutFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl LayoutNode {
    pub fn new(id: impl Into<WidgetId>) -> Self {
        Self {
            id: id.into(),
            kind: LayoutKind::Container,
            style: LayoutStyle::default(),
            children: Vec::new(),
            flags: LayoutFlags::NONE,
        }
    }

    pub fn with_style(mut self, style: LayoutStyle) -> Self {
        self.style = style;
        self
    }
    
    pub fn with_child(mut self, child: LayoutNode) -> Self {
        self.children.push(child);
        self
    }

    pub fn with_children(mut self, children: Vec<LayoutNode>) -> Self {
        self.children = children;
        self
    }
    
    pub fn with_kind(mut self, kind: LayoutKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn with_flags(mut self, flags: LayoutFlags) -> Self {
        self.flags = flags;
        self
    }
}

/// Computed layout result for a node
#[derive(Clone, Debug)]
pub struct LayoutComputed {
    /// Final absolute position and size
    pub rect: WidgetRect,
    /// Content area (inner rect minus padding/border)
    pub content_rect: WidgetRect,
    /// Visible clipping area (intersection of all parent clips)
    pub clip_rect: Option<WidgetRect>,
    /// Z-index stack level
    pub z_order: i32,
}
