//! Sticky chevron — atomic overlay widget that attaches to one edge or
//! corner of a host rect. Owns its own rect, its own hit zone, and is
//! drawn ON TOP of the host so clicks go to the chevron, not the host.

use crate::types::Rect;
use crate::ui::widgets::atomic::chevron::types::{ChevronDirection, ChevronVisualKind};

/// Where on the host rect the chevron sticks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StickyAnchor {
    /// Top edge, centered horizontally.
    N,
    /// Bottom edge, centered.
    S,
    /// Left edge, centered vertically.
    W,
    /// Right edge, centered.
    E,
    /// Top-left corner.
    NW,
    /// Top-right corner.
    NE,
    /// Bottom-left corner.
    SW,
    /// Bottom-right corner.
    SE,
}

/// When is the chevron visible?
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StickyVisibility {
    /// Always visible (e.g. permanent split-button chevron).
    Always,
    /// Visible only while the host or the chevron itself is hovered.
    OnHostHover,
    /// Visible only while the chevron itself is hovered.
    OnSelfHover,
}

/// Per-instance spec for a sticky chevron.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StickyChevronSpec {
    /// Direction the chevron arrow points (independent of anchor).
    pub direction:  ChevronDirection,
    /// Side of the chevron's square rect in pixels.
    pub size:       f64,
    /// Gap between the chevron rect and the corresponding host edge / corner.
    /// `0.0` (default) = the chevron sits flush against the edge.
    pub inset:      f64,
    /// Which edge / corner of the host to attach to.
    pub anchor:     StickyAnchor,
    /// When the chevron paints + when its hit zone is registered.
    pub visibility: StickyVisibility,
    /// Drawing primitive (Stroked / Filled / Glyph / Icon — passed to the
    /// underlying chevron atomic).
    pub visual:     ChevronVisualKind,
    /// Whether the chevron paints its own hover state when the cursor is
    /// over its rect. Default `true` (legacy behaviour). Set `false` for
    /// purely-decorative chevrons that visually inherit the host's state
    /// — they still register a hit zone (so click events work) but ignore
    /// `chev_state.is_hovered()` when drawing.
    pub hover_visual: bool,
    /// Whether the chevron child registers as `Sense::CLICK | HOVER`
    /// (`true`, default) or `Sense::NONE` (`false`). Set `false` for
    /// pure-decoration chevrons — they're still real coord widgets (so
    /// they participate in the tree) but never claim hover/click; the
    /// host alone collects the row's events.
    pub interactive: bool,
}

impl Default for StickyChevronSpec {
    fn default() -> Self {
        Self {
            direction:  ChevronDirection::Down,
            size:       16.0,
            inset:      0.0,
            anchor:     StickyAnchor::E,
            visibility: StickyVisibility::Always,
            visual:     ChevronVisualKind::Stroked,
            hover_visual: true,
            interactive: true,
        }
    }
}

/// Compute the chevron's screen rect given a host rect and a spec.
pub fn place_sticky_chevron(host: Rect, spec: &StickyChevronSpec) -> Rect {
    let s = spec.size;
    let i = spec.inset;
    use StickyAnchor::*;
    match spec.anchor {
        N  => Rect::new(host.x + (host.width - s) / 2.0, host.y + i, s, s),
        S  => Rect::new(host.x + (host.width - s) / 2.0, host.y + host.height - s - i, s, s),
        W  => Rect::new(host.x + i, host.y + (host.height - s) / 2.0, s, s),
        E  => Rect::new(host.x + host.width - s - i, host.y + (host.height - s) / 2.0, s, s),
        NW => Rect::new(host.x + i, host.y + i, s, s),
        NE => Rect::new(host.x + host.width - s - i, host.y + i, s, s),
        SW => Rect::new(host.x + i, host.y + host.height - s - i, s, s),
        SE => Rect::new(host.x + host.width - s - i, host.y + host.height - s - i, s, s),
    }
}
