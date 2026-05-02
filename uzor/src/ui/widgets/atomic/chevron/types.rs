//! Chevron atomic — type definitions.

/// Direction the chevron points. Independent of state — caller picks the
/// active direction each frame (so an `ExpandToggle` site swaps Right ↔ Down
/// based on its `expanded` flag at the call site).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChevronDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Semantic role of the chevron. Drives sane defaults for visibility,
/// placement and hit-area policies; each is still individually overrideable
/// on the `ChevronView`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChevronUseCase {
    /// Pure button — back arrow, breadcrumb step, generic icon button.
    /// Default: visible always, hit area = whole host (set by composite).
    PureButton,
    /// Step the host's pixel-scroll offset by one notch on click.
    /// Default: visible only while there's room to scroll, dedicated rect
    /// reserved on the overflowing edge.
    PixelScrollStep,
    /// Advance one page (item-step) through a finite paged set.
    /// Default: visible only while there's another page in this direction.
    PageStep,
    /// Opens a dropdown menu attached to the host.
    /// Default: visible always; inline corner of host; own hit zone.
    DropdownTrigger,
    /// Indicates a submenu is reachable from the host row.
    /// Default: right-pointing, decorative; row click handles activation.
    SubmenuTrigger,
    /// Tree-node expand/collapse. Direction (Right / Down) is chosen by the
    /// caller from its `expanded` flag.
    ExpandToggle,
    /// Panel-collapse toggle (hide whole panel into header).
    PanelHideToggle,
    /// Decorative-only affordance — host owns the click. The chevron renders
    /// without a hit zone of its own.
    Affordance,
    /// Pure glyph used as an icon inside another row (e.g. "Move up" menu
    /// item). No interactivity intrinsic.
    IconGlyph,
}

/// When is the chevron visible?
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisibilityPolicy {
    /// Always shown.
    Always,
    /// Shown when there is content to advance to in this direction. Caller
    /// sets `has_more` each frame from its scroll offset / page index.
    WhenOverflow { has_more: bool },
    /// Shown only while the host is hovered. Useful for revealing extra
    /// controls without adding visual noise.
    OnHover,
}

/// How the chevron occupies space within its host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlacementPolicy {
    /// The chevron owns its rect outright — caller passes the full rect.
    Standalone,
    /// The chevron lives inside the leading or trailing corner of the host
    /// rect; host stays the click target unless `HitAreaPolicy` says otherwise.
    InlineCorner { trailing: bool },
    /// The chevron eats `size` pixels off the host's main axis when visible.
    /// Caller does the actual layout reservation; this variant is purely
    /// declarative for tooling / introspection.
    LayoutEdge,
    /// Float over the host without affecting layout. Z-order handled by caller.
    Overlay,
}

/// What rect the click should hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HitAreaPolicy {
    /// Hit area = the visual rect.
    Visual,
    /// No own hit area — the host owns the click.
    None,
    /// Hit area is the visual rect inflated by `padding` pixels on each side.
    Inflated { padding: f64 },
}

/// Drawing primitive used to paint the chevron.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChevronVisualKind {
    /// Three-segment stroked V shape (the legacy `scroll_chevron` look).
    Stroked,
    /// Filled equilateral triangle (split-button trigger / panel toggle look).
    Filled,
    /// Unicode glyph (e.g. `▶` `▼`). Rendered through atomic Text-style
    /// fill_text; takes `font_size` from style.
    Glyph,
    /// SVG icon by id. Rendered through the host's icon system; the atomic
    /// only reserves the rect.
    Icon,
}

/// Per-frame data handed to `draw_chevron`.
pub struct ChevronView {
    pub direction:   ChevronDirection,
    pub use_case:    ChevronUseCase,
    pub visibility:  VisibilityPolicy,
    pub placement:   PlacementPolicy,
    pub hit_area:    HitAreaPolicy,
    pub visual_kind: ChevronVisualKind,

    // Interaction state — kept inline because the atomic itself is
    // stateless; the coordinator owns the persistent half.
    pub hovered:  bool,
    pub pressed:  bool,
    pub disabled: bool,
    /// "Open" / "expanded" — drives accent colour, not direction.
    pub active:   bool,

    /// Override the glyph string when `visual_kind == Glyph`. `None` ⇒ use
    /// the default glyph for `direction` (▲▼◀▶).
    pub glyph_override: Option<&'static str>,
}

impl Default for ChevronView {
    fn default() -> Self {
        Self {
            direction:   ChevronDirection::Down,
            use_case:    ChevronUseCase::PureButton,
            visibility:  VisibilityPolicy::Always,
            placement:   PlacementPolicy::Standalone,
            hit_area:    HitAreaPolicy::Visual,
            visual_kind: ChevronVisualKind::Stroked,
            hovered:     false,
            pressed:     false,
            disabled:    false,
            active:      false,
            glyph_override: None,
        }
    }
}

impl ChevronView {
    /// Should this chevron actually render this frame?
    pub fn should_render(&self) -> bool {
        match self.visibility {
            VisibilityPolicy::Always => true,
            VisibilityPolicy::WhenOverflow { has_more } => has_more,
            VisibilityPolicy::OnHover => self.hovered,
        }
    }
}
