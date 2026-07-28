//! Domain-neutral, optional visual semantics for individual graph elements.
//!
//! These values affect paint construction only. They never replace
//! [`crate::graph::NodeIndex`] / [`crate::graph::EdgeIndex`] identity and
//! do not participate in layout, selection, hover, or picking.

/// A semantic marker layered around a node's ordinary circle/sphere.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeMarker {
    /// Two concentric rings, suitable for a root or other primary node.
    DoubleRing,
    /// A distinct enclosing boundary.
    Boundary,
    /// An open/frontier affordance.
    Frontier,
    /// A warning affordance.
    Warning,
}

/// Optional edge dashing.
#[derive(Clone, Debug, PartialEq)]
pub enum DashPattern {
    /// Renderer-appropriate ordinary dashed styling.
    Dashed,
    /// Alternating on/off lengths in render units.
    Pattern(Vec<f32>),
}

impl DashPattern {
    /// A finite, positive alternating pattern, or `None` when the custom
    /// pattern is invalid/empty and should degrade safely to a solid edge.
    pub(crate) fn resolved(&self) -> Option<Vec<f32>> {
        let values = match self {
            Self::Dashed => vec![8.0, 5.0],
            Self::Pattern(values) => values.clone(),
        };
        if values.is_empty() || values.iter().any(|value| !value.is_finite() || *value <= 0.0) {
            None
        } else {
            Some(values)
        }
    }
}

/// Optional paint overrides for one node.
///
/// `None` fields inherit the category palette and renderer defaults.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodeVisualStyle {
    /// CSS-style fill/tint color (normally `#rrggbb`).
    pub fill: Option<String>,
    /// CSS-style outline/marker color.
    pub outline: Option<String>,
    /// Opacity override, clamped to `0..=1` by renderers.
    pub alpha: Option<f32>,
    /// 2D outline width in pixels. 3D marker meshes keep their shared,
    /// renderer-owned thickness.
    pub outline_width: Option<f32>,
    /// Optional semantic marker layered without changing node identity.
    pub marker: Option<NodeMarker>,
}

/// Optional paint overrides for one edge.
///
/// `None` fields inherit the global edge theme, including the existing
/// weight-to-width behavior in 3D.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EdgeVisualStyle {
    /// CSS-style stroke/tint color (normally `#rrggbb`).
    pub tint: Option<String>,
    /// Opacity override, clamped to `0..=1` by renderers.
    pub alpha: Option<f32>,
    /// Desired stroke width in pixels. The 3D renderer converts this to
    /// an instance multiplier against its 1.75px default base width.
    pub width: Option<f32>,
    /// Optional dashed construction.
    pub dash: Option<DashPattern>,
    /// Signed lateral displacement from the endpoint-to-endpoint segment.
    ///
    /// The 2D renderer interprets this in screen pixels. The 3D renderer
    /// interprets it in world units along a deterministic perpendicular.
    /// `None` preserves the endpoint geometry exactly.
    pub lateral_offset: Option<f32>,
    /// Draw an arrowhead at the `to` endpoint.
    pub directed: bool,
}
