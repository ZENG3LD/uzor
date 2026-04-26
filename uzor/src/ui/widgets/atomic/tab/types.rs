//! Tab type definitions.

/// Configuration for a single tab in a tab strip.
#[derive(Debug, Clone)]
pub struct TabConfig {
    /// Unique identifier for this tab (used for routing events).
    pub id: String,
    /// Display label.
    pub label: String,
    /// Whether this tab is the currently selected one.
    pub active: bool,
    /// Whether the tab displays a close button.
    pub closable: bool,
    /// Optional icon name / path (rendered by the backend).
    pub icon: Option<String>,
}

impl TabConfig {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            active: false,
            closable: false,
            icon: None,
        }
    }

    pub fn active(mut self) -> Self {
        self.active = true;
        self
    }

    pub fn closable(mut self) -> Self {
        self.closable = true;
        self
    }

    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}

/// Events produced by a tab during a frame.
#[derive(Debug, Clone, Default)]
pub struct TabResponse {
    /// Tab body was clicked (select).
    pub clicked: bool,
    /// Close button inside the tab was clicked.
    pub close_clicked: bool,
    /// Pointer is currently over the tab body.
    pub hovered: bool,
}
