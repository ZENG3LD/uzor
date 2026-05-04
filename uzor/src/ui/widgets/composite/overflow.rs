//! Shared overflow helpers for composite containers.
//!
//! A composite's body has a *natural* size (`content_w × content_h`) and an
//! *available* rect (the layout-solver gave it). When natural > available,
//! the composite picks one of three reactions:
//!
//! 1. **Chevrons** — register four paging arrows on the overflowing edges.
//!    The user pages by step. Used by toolbars, modal bodies, dropdowns.
//! 2. **Scrollbar** — register a track + draggable handle on the overflowing
//!    axis. The user scrolls continuously. Used by sidebars, long modal bodies.
//! 3. **Compress** — return a scale factor the composite can apply to its
//!    children so they fit. No registration; pure math.
//!
//! All three operate on the same shared state (`BodyScrollState`) so a
//! composite can switch between modes without rewriting its draw path.
//!
//! Helpers do nothing when the body actually fits (`content <= rect`); the
//! composite never has to short-circuit by itself.

use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::input::core::coordinator::LayerId;
use crate::render::RenderContext;
use crate::types::{CompositeId, Rect};

// =============================================================================
// Shared scroll/content state
// =============================================================================

/// Per-composite scroll + content extents. Composites embed this in their
/// own state struct so all three helpers can read/write a single source of
/// truth.
#[derive(Debug, Clone, Copy, Default)]
pub struct BodyScrollState {
    /// Horizontal scroll offset in pixels. `0.0` = leftmost.
    pub offset_x: f64,
    /// Vertical scroll offset in pixels. `0.0` = topmost.
    pub offset_y: f64,
    /// Natural content width in pixels.
    pub content_w: f64,
    /// Natural content height in pixels.
    pub content_h: f64,
}

impl BodyScrollState {
    /// Clamp scroll offsets so they never expose blank space past content
    /// boundaries. Call after the composite updates `content_*` and after
    /// any user-driven offset change.
    pub fn clamp(&mut self, body_w: f64, body_h: f64) {
        let max_x = (self.content_w - body_w).max(0.0);
        let max_y = (self.content_h - body_h).max(0.0);
        if self.offset_x > max_x { self.offset_x = max_x; }
        if self.offset_y > max_y { self.offset_y = max_y; }
        if self.offset_x < 0.0   { self.offset_x = 0.0;   }
        if self.offset_y < 0.0   { self.offset_y = 0.0;   }
    }

    /// Does the content overflow either axis?
    pub fn overflows(&self, body_w: f64, body_h: f64) -> Overflow {
        Overflow {
            horizontal: self.content_w > body_w + 0.5,
            vertical:   self.content_h > body_h + 0.5,
        }
    }
}

/// Per-axis overflow flags returned by [`BodyScrollState::overflows`].
#[derive(Debug, Clone, Copy, Default)]
pub struct Overflow {
    pub horizontal: bool,
    pub vertical:   bool,
}

impl Overflow {
    /// Either axis overflows.
    pub fn any(self) -> bool { self.horizontal || self.vertical }
}

// =============================================================================
// 1. Chevrons (paging) helper
// =============================================================================

/// Width of the chevron strip painted on each overflowing edge.
pub const CHEVRON_STRIP: f64 = 26.0;

/// How much one chevron click pages the body.
pub const CHEVRON_STEP_PX: f64 = 80.0;

/// Register chevron hit-zones on the four edges of `body` whose axis
/// overflows. No-op when the content fits.
///
/// IDs use the convention `"{host_id}:chevron_{up|down|left|right}"` so
/// composites can route `DispatchEvent::ChevronStepRequested` back to
/// their step handler.
pub fn register_chevrons_helper(
    coord:    &mut InputCoordinator,
    host_id:  &CompositeId,
    body:     Rect,
    state:    &BodyScrollState,
    layer:    &LayerId,
) {
    if body.width <= 0.0 || body.height <= 0.0 { return; }
    let _ = layer; // composites already register children under their own layer
    let o = state.overflows(body.width, body.height);
    if !o.any() { return; }

    // Carve corners off vertical strips when both axes overflow so the
    // four hit-zones don't overlap.
    let inset_x = if o.horizontal { CHEVRON_STRIP } else { 0.0 };
    let inset_y = if o.vertical   { CHEVRON_STRIP } else { 0.0 };

    if o.vertical {
        let w = (body.width - inset_x * 2.0).max(0.0);
        let up = Rect::new(body.x + inset_x, body.y, w, CHEVRON_STRIP);
        let dn = Rect::new(body.x + inset_x, body.y + body.height - CHEVRON_STRIP, w, CHEVRON_STRIP);
        coord.register_child(host_id, format!("{}:chevron_up",   host_id.0.0),
            WidgetKind::Button, up, Sense::CLICK | Sense::HOVER);
        coord.register_child(host_id, format!("{}:chevron_down", host_id.0.0),
            WidgetKind::Button, dn, Sense::CLICK | Sense::HOVER);
    }
    if o.horizontal {
        let h = (body.height - inset_y * 2.0).max(0.0);
        let lf = Rect::new(body.x, body.y + inset_y, CHEVRON_STRIP, h);
        let rt = Rect::new(body.x + body.width - CHEVRON_STRIP, body.y + inset_y, CHEVRON_STRIP, h);
        coord.register_child(host_id, format!("{}:chevron_left",  host_id.0.0),
            WidgetKind::Button, lf, Sense::CLICK | Sense::HOVER);
        coord.register_child(host_id, format!("{}:chevron_right", host_id.0.0),
            WidgetKind::Button, rt, Sense::CLICK | Sense::HOVER);
    }
}

/// Draw chevron arrows on overflowing edges. Call AFTER the body content so
/// the strips sit on top.
///
/// `bg_color` — fill behind the arrow (usually the composite's body bg
/// to mask scrolled content). `arrow_color` — stroke colour.
pub fn draw_chevrons_helper(
    ctx:         &mut dyn RenderContext,
    body:        Rect,
    state:       &BodyScrollState,
    bg_color:    &str,
    arrow_color: &str,
) {
    use crate::ui::widgets::atomic::chevron::{
        draw_chevron,
        settings::ChevronSettings,
        types::{ChevronDirection, ChevronUseCase, ChevronView, ChevronVisualKind,
                HitAreaPolicy, PlacementPolicy, VisibilityPolicy},
    };
    if body.width <= 0.0 || body.height <= 0.0 { return; }
    let o = state.overflows(body.width, body.height);
    if !o.any() { return; }

    let chev_settings = ChevronSettings::default();
    let _ = arrow_color; // reserved for future custom theming via ChevronSettings

    if o.vertical {
        let max_v = (state.content_h - body.height).max(0.0);
        let has_back = state.offset_y > 0.5;
        let has_fwd  = state.offset_y < max_v - 0.5;
        let up = Rect::new(body.x, body.y, body.width, CHEVRON_STRIP);
        let dn = Rect::new(body.x, body.y + body.height - CHEVRON_STRIP, body.width, CHEVRON_STRIP);
        ctx.set_fill_color(bg_color);
        ctx.fill_rect(up.x, up.y, up.width, up.height);
        ctx.fill_rect(dn.x, dn.y, dn.width, dn.height);
        let v_up = ChevronView { direction: ChevronDirection::Up,   use_case: ChevronUseCase::PixelScrollStep,
            visibility: VisibilityPolicy::WhenOverflow { has_more: has_back },
            placement: PlacementPolicy::Overlay, hit_area: HitAreaPolicy::Visual,
            visual_kind: ChevronVisualKind::Stroked, ..Default::default() };
        let v_dn = ChevronView { direction: ChevronDirection::Down, use_case: ChevronUseCase::PixelScrollStep,
            visibility: VisibilityPolicy::WhenOverflow { has_more: has_fwd },
            placement: PlacementPolicy::Overlay, hit_area: HitAreaPolicy::Visual,
            visual_kind: ChevronVisualKind::Stroked, ..Default::default() };
        draw_chevron(ctx, up, &v_up, &chev_settings);
        draw_chevron(ctx, dn, &v_dn, &chev_settings);
    }
    if o.horizontal {
        let max_h = (state.content_w - body.width).max(0.0);
        let has_back = state.offset_x > 0.5;
        let has_fwd  = state.offset_x < max_h - 0.5;
        let lf = Rect::new(body.x, body.y, CHEVRON_STRIP, body.height);
        let rt = Rect::new(body.x + body.width - CHEVRON_STRIP, body.y, CHEVRON_STRIP, body.height);
        ctx.set_fill_color(bg_color);
        ctx.fill_rect(lf.x, lf.y, lf.width, lf.height);
        ctx.fill_rect(rt.x, rt.y, rt.width, rt.height);
        let v_lf = ChevronView { direction: ChevronDirection::Left,  use_case: ChevronUseCase::PixelScrollStep,
            visibility: VisibilityPolicy::WhenOverflow { has_more: has_back },
            placement: PlacementPolicy::Overlay, hit_area: HitAreaPolicy::Visual,
            visual_kind: ChevronVisualKind::Stroked, ..Default::default() };
        let v_rt = ChevronView { direction: ChevronDirection::Right, use_case: ChevronUseCase::PixelScrollStep,
            visibility: VisibilityPolicy::WhenOverflow { has_more: has_fwd },
            placement: PlacementPolicy::Overlay, hit_area: HitAreaPolicy::Visual,
            visual_kind: ChevronVisualKind::Stroked, ..Default::default() };
        draw_chevron(ctx, lf, &v_lf, &chev_settings);
        draw_chevron(ctx, rt, &v_rt, &chev_settings);
    }
}

/// Apply a chevron-step delta to the scroll state. Composites call this on
/// `DispatchEvent::ChevronStepRequested`.
pub fn step_chevron(state: &mut BodyScrollState, axis: ChevronAxis, body_w: f64, body_h: f64) {
    match axis {
        ChevronAxis::Up    => state.offset_y -= CHEVRON_STEP_PX,
        ChevronAxis::Down  => state.offset_y += CHEVRON_STEP_PX,
        ChevronAxis::Left  => state.offset_x -= CHEVRON_STEP_PX,
        ChevronAxis::Right => state.offset_x += CHEVRON_STEP_PX,
    }
    state.clamp(body_w, body_h);
}

/// Direction passed to [`step_chevron`].
#[derive(Debug, Clone, Copy)]
pub enum ChevronAxis { Up, Down, Left, Right }

// =============================================================================
// 2. Scrollbar helper
// =============================================================================

/// Visual width/height of the scrollbar track.
pub const SCROLLBAR_THICKNESS: f64 = 8.0;

/// Which axis the scrollbar lives on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollAxis { Vertical, Horizontal }

/// Register a scrollbar track + drag handle on `body`. No-op if the chosen
/// axis doesn't overflow.
///
/// Track id: `"{host_id}:scrollbar_track"`, handle id: `"{host_id}:scrollbar_handle"`.
pub fn register_scrollbar_helper(
    coord:    &mut InputCoordinator,
    host_id:  &CompositeId,
    body:     Rect,
    state:    &BodyScrollState,
    axis:     ScrollAxis,
    layer:    &LayerId,
) -> Option<Rect> {
    if body.width <= 0.0 || body.height <= 0.0 { return None; }
    let _ = layer;
    let o = state.overflows(body.width, body.height);
    let track = match axis {
        ScrollAxis::Vertical => {
            if !o.vertical { return None; }
            Rect::new(body.x + body.width - SCROLLBAR_THICKNESS, body.y, SCROLLBAR_THICKNESS, body.height)
        }
        ScrollAxis::Horizontal => {
            if !o.horizontal { return None; }
            Rect::new(body.x, body.y + body.height - SCROLLBAR_THICKNESS, body.width, SCROLLBAR_THICKNESS)
        }
    };
    coord.register_child(host_id, format!("{}:scrollbar_track",  host_id.0.0),
        WidgetKind::ScrollbarTrack, track, Sense::CLICK);
    coord.register_child(host_id, format!("{}:scrollbar_handle", host_id.0.0),
        WidgetKind::ScrollbarHandle, track, Sense::DRAG | Sense::HOVER);
    Some(track)
}

/// Draw scrollbar track + thumb. Call AFTER body content.
///
/// `track_color` / `thumb_color` — caller supplies palette tokens.
pub fn draw_scrollbar_helper(
    ctx:         &mut dyn RenderContext,
    body:        Rect,
    state:       &BodyScrollState,
    axis:        ScrollAxis,
    track_color: &str,
    thumb_color: &str,
) {
    if body.width <= 0.0 || body.height <= 0.0 { return; }
    let o = state.overflows(body.width, body.height);
    match axis {
        ScrollAxis::Vertical => {
            if !o.vertical { return; }
            let track_x = body.x + body.width - SCROLLBAR_THICKNESS;
            let track_y = body.y;
            let track_h = body.height;
            let max_off = (state.content_h - body.height).max(0.0);
            let visible = (body.height / state.content_h).clamp(0.0, 1.0);
            let thumb_h = (track_h * visible).max(20.0);
            let thumb_y = track_y + (track_h - thumb_h) * (state.offset_y / max_off.max(1.0));
            ctx.set_fill_color(track_color);
            ctx.fill_rect(track_x, track_y, SCROLLBAR_THICKNESS, track_h);
            ctx.set_fill_color(thumb_color);
            ctx.fill_rect(track_x, thumb_y, SCROLLBAR_THICKNESS, thumb_h);
        }
        ScrollAxis::Horizontal => {
            if !o.horizontal { return; }
            let track_x = body.x;
            let track_y = body.y + body.height - SCROLLBAR_THICKNESS;
            let track_w = body.width;
            let max_off = (state.content_w - body.width).max(0.0);
            let visible = (body.width / state.content_w).clamp(0.0, 1.0);
            let thumb_w = (track_w * visible).max(20.0);
            let thumb_x = track_x + (track_w - thumb_w) * (state.offset_x / max_off.max(1.0));
            ctx.set_fill_color(track_color);
            ctx.fill_rect(track_x, track_y, track_w, SCROLLBAR_THICKNESS);
            ctx.set_fill_color(thumb_color);
            ctx.fill_rect(thumb_x, track_y, thumb_w, SCROLLBAR_THICKNESS);
        }
    }
}

// =============================================================================
// 3. Compress helper
// =============================================================================

/// Per-axis scale factors (0.0..1.0) for compressing children when content
/// is wider/taller than the available rect.
#[derive(Debug, Clone, Copy, Default)]
pub struct CompressFactor {
    pub sx: f64,
    pub sy: f64,
}

impl CompressFactor {
    /// Identity (no compression).
    pub fn one() -> Self { Self { sx: 1.0, sy: 1.0 } }
    /// True when the factor is below 1.0 on either axis.
    pub fn compresses(&self) -> bool { self.sx < 1.0 || self.sy < 1.0 }
}

/// Compute the scale factor a composite should apply to its children when
/// it cannot fit them at natural size. `min_factor` clamps the squeeze so
/// content stays legible (e.g. 0.5 = never go below 50%).
///
/// Pure math — does not register or draw anything. The composite picks
/// up the factors and multiplies its child rects.
pub fn compute_compress_factor(content_w: f64, content_h: f64, body: Rect, min_factor: f64) -> CompressFactor {
    if body.width <= 0.0 || body.height <= 0.0 || content_w <= 0.0 || content_h <= 0.0 {
        return CompressFactor::one();
    }
    let sx = if content_w > body.width  { (body.width  / content_w).max(min_factor) } else { 1.0 };
    let sy = if content_h > body.height { (body.height / content_h).max(min_factor) } else { 1.0 };
    CompressFactor { sx, sy }
}
