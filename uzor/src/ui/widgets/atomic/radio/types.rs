//! Radio widget types — views, config, and render-kind enum.

/// One option in a radio group.
pub struct RadioOption<'a> {
    /// Primary label (13 px).
    pub label: &'a str,
    /// Optional secondary description (11 px, muted). Pass `""` to omit.
    pub description: &'a str,
    /// `true` when the pointer is hovering over this row.
    pub hovered: bool,
}

/// Per-frame data for the `Group` render kind.
pub struct RadioGroupView<'a> {
    /// Ordered list of radio options.
    pub options: &'a [RadioOption<'a>],
    /// Index of the currently selected option.
    pub selected: usize,
}

/// Per-frame data for the `Pair` render kind.
pub struct RadioPairView<'a> {
    /// Label for the left radio option.
    pub left_label: &'a str,
    /// Label for the right radio option.
    pub right_label: &'a str,
    /// `true` when the left option is selected; `false` selects the right.
    pub selected_left: bool,
}

/// Per-frame data for a single `Dot` (circle only, no label).
pub struct RadioDotView {
    /// `true` when this option is selected.
    pub selected: bool,
}

/// Shape variant for the `Dot` render kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DotShape {
    /// Standard circular dot (default).
    Circle,
    /// Square dot.
    Square,
    /// Pill (wide rounded rect) dot.
    Pill,
    /// Star-shaped dot.
    Star,
}

/// Static configuration for a radio widget.
#[derive(Debug, Clone)]
pub struct RadioConfig {
    /// Font string for labels (e.g. `"13px sans-serif"`).
    pub label_font: String,
}

impl Default for RadioConfig {
    fn default() -> Self {
        Self {
            label_font: "13px sans-serif".to_string(),
        }
    }
}

/// Selects the visual variant used by `draw_radio`.
///
/// `Custom` is an escape hatch for app-supplied renderers.
pub enum RadioRenderKind<'a> {
    /// Canonical vertical list of radio rows (section 35).
    /// Args: x, y, width forwarded to the renderer.
    Group {
        x: f64,
        y: f64,
        width: f64,
        view: RadioGroupView<'a>,
    },
    /// Two inline radio buttons side-by-side (sections 36-37).
    Pair {
        /// `false` = solid fill when active (section 36).
        /// `true`  = outer ring + inner dot when active (section 37).
        use_ring_dot: bool,
        x: f64,
        cy: f64,
        between_gap: f64,
        view: RadioPairView<'a>,
    },
    /// Single inline circle dot (section 37 — dot-only, row drawn by parent).
    Dot {
        shape: DotShape,
        cx: f64,
        cy: f64,
        view: RadioDotView,
    },
    /// Caller-supplied renderer.
    Custom(Box<dyn Fn(&mut dyn crate::render::RenderContext, crate::types::Rect, crate::types::WidgetState, &super::settings::RadioSettings) + 'a>),
}
