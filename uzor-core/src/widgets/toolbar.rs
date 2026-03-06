//! Toolbar widget configuration and geometry
//!
//! Provides toolbar configuration and geometry calculation
//! for headless architecture.

use crate::types::{Rect, WidgetState};
use serde::{Deserialize, Serialize};

/// Toolbar orientation
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolbarOrientation {
    #[default]
    Horizontal,
    Vertical,
}

/// Toolbar item configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ToolbarItem {
    /// Button with text
    Button {
        id: String,
        text: Option<String>,
        active: bool,
        disabled: bool,
        min_width: f64,
    },
    /// Visual separator
    Separator,
    /// Flexible spacer
    Spacer,
    /// Label (non-interactive text)
    Label {
        id: String,
        text: String,
    },
}

impl ToolbarItem {
    pub fn button(id: &str, text: &str) -> Self {
        Self::Button {
            id: id.to_string(),
            text: Some(text.to_string()),
            active: false,
            disabled: false,
            min_width: 0.0,
        }
    }

    pub fn id(&self) -> Option<&str> {
        match self {
            Self::Button { id, .. } => Some(id),
            Self::Label { id, .. } => Some(id),
            Self::Separator | Self::Spacer => None,
        }
    }
}

/// Section alignment
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SectionAlign {
    #[default]
    Start,
    End,
}

/// Toolbar section configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolbarSection {
    pub items: Vec<ToolbarItem>,
    pub show_separator: bool,
    pub align: SectionAlign,
}

impl ToolbarSection {
    pub fn new(items: Vec<ToolbarItem>) -> Self {
        Self {
            items,
            show_separator: false,
            align: SectionAlign::Start,
        }
    }
}

/// Toolbar configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolbarConfig {
    pub sections: Vec<ToolbarSection>,
    pub orientation: ToolbarOrientation,
    pub item_size: f64,
    pub spacing: f64,
    pub padding: f64,
}

impl Default for ToolbarConfig {
    fn default() -> Self {
        Self {
            sections: Vec::new(),
            orientation: ToolbarOrientation::Horizontal,
            item_size: 32.0,
            spacing: 4.0,
            padding: 8.0,
        }
    }
}

/// Toolbar interaction response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ToolbarResponse {
    pub clicked: Option<String>,
    pub hovered: Option<String>,
    pub item_geometries: Vec<ToolbarItemGeometry>,
}

/// Geometry and state for a single toolbar item
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolbarItemGeometry {
    pub id: Option<String>,
    pub rect: Rect,
    pub state: WidgetState,
    pub item_type: ToolbarItemType,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ToolbarItemType {
    Button,
    Separator,
    Spacer,
    Label,
}

// TODO: Implement headless toolbar geometry calculation and interaction detection