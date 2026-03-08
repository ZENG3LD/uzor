//! Panel toolbar definitions
//!
//! Panel crates use these types to describe their toolbar layout.
//! The panel renders the toolbar itself using RenderContext + these definitions.

use serde::{Serialize, Deserialize};

/// Icon identifier for toolbar buttons (string-based, maps to SVG icon registry)
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolbarIconId(pub String);

impl ToolbarIconId {
    pub fn new(name: &str) -> Self {
        Self(name.to_string())
    }

    pub fn name(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ToolbarIconId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for ToolbarIconId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// A single toolbar item
#[derive(Clone, Debug)]
pub enum ToolbarItemDef {
    /// Button with icon and/or text
    Button {
        id: &'static str,
        icon: Option<ToolbarIconId>,
        text: Option<String>,
        active: bool,
        disabled: bool,
        min_width: f64,
    },
    /// Icon-only button
    IconButton {
        id: &'static str,
        icon: ToolbarIconId,
        active: bool,
        disabled: bool,
    },
    /// Dropdown button (panel handles the popup rendering)
    Dropdown {
        id: &'static str,
        icon: Option<ToolbarIconId>,
        text: Option<String>,
        active: bool,
        show_chevron: bool,
        items: Vec<DropdownItemDef>,
        /// If true, first click activates last tool, second click opens dropdown
        quick_select: bool,
        /// Grid layout columns for dropdown (None = vertical list)
        grid_columns: Option<u8>,
        min_width: f64,
    },
    /// Visual separator
    Separator,
    /// Flexible spacer
    Spacer,
}

impl ToolbarItemDef {
    pub fn button(id: &'static str) -> Self {
        Self::Button {
            id,
            icon: None,
            text: None,
            active: false,
            disabled: false,
            min_width: 0.0,
        }
    }

    pub fn icon_button(id: &'static str, icon: impl Into<ToolbarIconId>) -> Self {
        Self::IconButton {
            id,
            icon: icon.into(),
            active: false,
            disabled: false,
        }
    }

    pub fn dropdown(id: &'static str, items: Vec<DropdownItemDef>) -> Self {
        Self::Dropdown {
            id,
            icon: None,
            text: None,
            active: false,
            show_chevron: true,
            items,
            quick_select: false,
            grid_columns: None,
            min_width: 0.0,
        }
    }

    pub fn quick_select(id: &'static str, items: Vec<DropdownItemDef>) -> Self {
        Self::Dropdown {
            id,
            icon: None,
            text: None,
            active: false,
            show_chevron: false,
            items,
            quick_select: true,
            grid_columns: None,
            min_width: 0.0,
        }
    }

    pub fn with_icon(mut self, icon: impl Into<ToolbarIconId>) -> Self {
        match &mut self {
            Self::Button { icon: ref mut i, .. } => *i = Some(icon.into()),
            Self::Dropdown { icon: ref mut i, .. } => *i = Some(icon.into()),
            _ => {}
        }
        self
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        match &mut self {
            Self::Button { text: ref mut t, .. } => *t = Some(text.into()),
            Self::Dropdown { text: ref mut t, .. } => *t = Some(text.into()),
            _ => {}
        }
        self
    }

    pub fn with_active(mut self, active: bool) -> Self {
        match &mut self {
            Self::Button { active: ref mut a, .. } => *a = active,
            Self::IconButton { active: ref mut a, .. } => *a = active,
            Self::Dropdown { active: ref mut a, .. } => *a = active,
            _ => {}
        }
        self
    }

    pub fn with_min_width(mut self, w: f64) -> Self {
        match &mut self {
            Self::Button { min_width: ref mut mw, .. } => *mw = w,
            Self::Dropdown { min_width: ref mut mw, .. } => *mw = w,
            _ => {}
        }
        self
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Button { id, .. } => id,
            Self::IconButton { id, .. } => id,
            Self::Dropdown { id, .. } => id,
            Self::Separator | Self::Spacer => "",
        }
    }
}

/// Dropdown menu item
#[derive(Clone, Debug)]
pub enum DropdownItemDef {
    /// Clickable action item
    Action {
        id: String,
        label: String,
        icon: Option<ToolbarIconId>,
        shortcut: Option<String>,
    },
    /// Submenu (nested dropdown)
    Submenu {
        id: String,
        label: String,
        icon: Option<ToolbarIconId>,
        items: Vec<DropdownItemDef>,
        grid_columns: Option<u8>,
    },
    /// Section header text
    Header {
        label: String,
    },
    /// Visual separator
    Separator,
}

impl DropdownItemDef {
    pub fn action(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::Action {
            id: id.into(),
            label: label.into(),
            icon: None,
            shortcut: None,
        }
    }

    pub fn with_icon(mut self, icon: impl Into<ToolbarIconId>) -> Self {
        match &mut self {
            Self::Action { icon: ref mut i, .. } => *i = Some(icon.into()),
            Self::Submenu { icon: ref mut i, .. } => *i = Some(icon.into()),
            _ => {}
        }
        self
    }

    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        if let Self::Action { shortcut: ref mut s, .. } = &mut self {
            *s = Some(shortcut.into());
        }
        self
    }
}

/// A group of toolbar items with optional separator
#[derive(Clone, Debug)]
pub struct ToolbarSectionDef {
    pub items: Vec<ToolbarItemDef>,
    pub show_separator: bool,
    /// Section alignment (Start = left/top, End = right/bottom)
    pub align: SectionAlign,
}

/// Section alignment within toolbar
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SectionAlign {
    #[default]
    Start,
    End,
}

impl ToolbarSectionDef {
    pub fn new(items: Vec<ToolbarItemDef>) -> Self {
        Self {
            items,
            show_separator: false,
            align: SectionAlign::Start,
        }
    }

    pub fn with_separator(mut self) -> Self {
        self.show_separator = true;
        self
    }

    pub fn align_end(mut self) -> Self {
        self.align = SectionAlign::End;
        self
    }
}

/// Toolbar orientation
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolbarOrientation {
    #[default]
    Horizontal,
    Vertical,
}

/// Complete toolbar definition for a panel
#[derive(Clone, Debug)]
pub struct PanelToolbarDef {
    pub sections: Vec<ToolbarSectionDef>,
    pub orientation: ToolbarOrientation,
    /// Height (horizontal) or width (vertical) of the toolbar
    pub size: f64,
    /// Individual item size (button height/width)
    pub item_size: f64,
    /// Icon size within items
    pub icon_size: f64,
    /// Spacing between items
    pub spacing: f64,
    /// Padding at toolbar edges
    pub padding: f64,
}

impl Default for PanelToolbarDef {
    fn default() -> Self {
        Self {
            sections: Vec::new(),
            orientation: ToolbarOrientation::Horizontal,
            size: 32.0,
            item_size: 28.0,
            icon_size: 16.0,
            spacing: 2.0,
            padding: 4.0,
        }
    }
}

impl PanelToolbarDef {
    pub fn horizontal(sections: Vec<ToolbarSectionDef>) -> Self {
        Self {
            sections,
            orientation: ToolbarOrientation::Horizontal,
            ..Default::default()
        }
    }

    pub fn vertical(sections: Vec<ToolbarSectionDef>) -> Self {
        Self {
            sections,
            orientation: ToolbarOrientation::Vertical,
            size: 40.0,
            item_size: 32.0,
            icon_size: 18.0,
            spacing: 2.0,
            padding: 4.0,
        }
    }

    pub fn with_size(mut self, size: f64) -> Self {
        self.size = size;
        self
    }
}
