//! Tab type definitions.

/// Selects which visual variant to render.
///
/// Callers that want a single-dispatch entry point pass a `TabKind` alongside
/// `TabConfig` to `draw_tab`, which delegates to the appropriate dedicated
/// `draw_*_tab` function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabKind {
    /// Browser-style tab strip with a close × and a 2px bottom accent line.
    /// Geometry: CHROME_HEIGHT=32, TAB_PADDING_H=12, TAB_CLOSE_SIZE=16.
    #[default]
    Chrome,

    /// Icon-only vertical sidebar tab (modal settings, search overlay category
    /// filter). Geometry: width=48, button_height=44/40, 3px left accent bar.
    ModalSidebar,

    /// Text-label horizontal tab row with a full filled-bg active state and no
    /// accent bar. Geometry: height=32, padding_h=12, gap=2.
    ModalHorizontal,

    /// Text-only pill with a rounded-rect background. Geometry: width=80,
    /// item_height=40, pill_radius=4.
    TagsTabsSidebar,
}

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
    /// When `true` the caller should compute width from the label text rather
    /// than relying on a fixed rect width (used by Chrome and ModalHorizontal).
    pub intrinsic_width: bool,
}

impl TabConfig {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            active: false,
            closable: false,
            icon: None,
            intrinsic_width: false,
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

    pub fn intrinsic(mut self) -> Self {
        self.intrinsic_width = true;
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
