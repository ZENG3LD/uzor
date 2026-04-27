//! Container rendering — 6 dedicated functions, one per visual pattern.
//!
//! Design rule: functions are NOT merged into a single dispatcher.
//! Each carries distinct visual semantics and may diverge further as mlc evolves.

use crate::render::RenderContext;
use crate::types::Rect;

use super::style::{
    BorderedContainerStyle, CardContainerStyle, ClippingContainerStyle, ContainerStyle,
    PanelContainerStyle, PlainContainerStyle, SectionContainerStyle,
};
use super::theme::ContainerTheme;
use super::types::{ContainerType, PanelRole};

// ---------------------------------------------------------------------------
// 1. Plain — fill_rect bg, no border
// ---------------------------------------------------------------------------

/// Draw a plain rectangle background with no border or rounding.
///
/// mlc equivalent: all 9 trading panel outermost fills
/// (`fill_rect` with `theme.panel_bg`).
pub fn draw_plain_container(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    theme: &dyn ContainerTheme,
    _style: &PlainContainerStyle,
) {
    ctx.set_fill_color(theme.bg());
    ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);
}

// ---------------------------------------------------------------------------
// 2. Bordered — fill_rounded_rect bg + 1px stroke border
// ---------------------------------------------------------------------------

/// Draw a rounded-rect background with a 1px border.
///
/// mlc equivalent: dropdown menus in modal settings
/// (`fill_rounded_rect` + `stroke_rounded_rect`, radius=4.0, separator color).
pub fn draw_bordered_container(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    theme: &dyn ContainerTheme,
    style: &BorderedContainerStyle,
) {
    ctx.set_fill_color(theme.bg());
    ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, style.radius());

    ctx.set_stroke_color(theme.border());
    ctx.set_stroke_width(style.border_width());
    ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, style.radius());
}

// ---------------------------------------------------------------------------
// 3. Card — bg + drop shadow + rounded corners
// ---------------------------------------------------------------------------

/// Draw a card: shadow rect → background → border.
///
/// NOTE: In mlc, blur (`draw_blur_background`) is exclusive to the composite
/// `Popup` and `ModalFrame` widgets. This atomic Card renders shadow-only
/// (alpha-filled rect offset). Callers needing backdrop blur must use
/// `crate::ui::widgets::composite::popup`.
///
/// mlc popup defaults: shadow_offset=(2,4), radius=4.0, shadow rgba(0,0,0,0.4).
pub fn draw_card_container(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    theme: &dyn ContainerTheme,
    style: &CardContainerStyle,
) {
    // Shadow — filled rect drawn behind the card, offset by (dx, dy).
    let (sx, sy) = style.shadow_offset();
    ctx.set_fill_color_alpha(theme.card_shadow_color(), style.shadow_alpha());
    ctx.fill_rounded_rect(
        rect.x + sx,
        rect.y + sy,
        rect.width,
        rect.height,
        style.radius(),
    );
    ctx.reset_alpha();

    // Background.
    ctx.set_fill_color(theme.bg());
    if style.radius() > 0.0 {
        ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, style.radius());
    } else {
        ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);
    }

    // Border.
    ctx.set_stroke_color(theme.border());
    ctx.set_stroke_width(1.0);
    if style.radius() > 0.0 {
        ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, style.radius());
    } else {
        ctx.stroke_rect(rect.x, rect.y, rect.width, rect.height);
    }
}

// ---------------------------------------------------------------------------
// 4. Clipping — bg + ctx.save / clip_rect / restore
// ---------------------------------------------------------------------------

/// Begin a clipping container: saves render state and installs a clip rect.
///
/// **Must be paired with `end_clipping_container`** — draw children in between.
///
/// mlc equivalent: `ScrollableContainer::begin()` — `ctx.save()` + `ctx.clip()`.
/// No background is drawn here; the caller owns that fill (matching mlc behaviour).
pub fn begin_clipping_container(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    style: &ClippingContainerStyle,
) {
    if style.clipping {
        ctx.save();
        ctx.clip_rect(rect.x, rect.y, rect.width, rect.height);
    }
}

/// End a clipping container: restores saved render state.
///
/// Call after all children have been drawn.
pub fn end_clipping_container(ctx: &mut dyn RenderContext, style: &ClippingContainerStyle) {
    if style.clipping {
        ctx.restore();
    }
}

// ---------------------------------------------------------------------------
// 5. Section — header strip + body bg + optional border
// ---------------------------------------------------------------------------

/// Draw a section container: body background, then header strip on top.
///
/// mlc equivalent: trading panels with a column/title header strip
/// (dom.rs, order_entry.rs, position_manager.rs, trade_log.rs, etc.).
/// Drawing order matches mlc: `panel_bg` first, then `header_bg` on top.
pub fn draw_section_container(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    theme: &dyn ContainerTheme,
    style: &SectionContainerStyle,
) {
    // Body background.
    ctx.set_fill_color(theme.bg());
    ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);

    // Header strip — drawn on top of body bg.
    let header_h = style.header_height.min(rect.height);
    ctx.set_fill_color(theme.section_header_bg());
    ctx.fill_rect(rect.x, rect.y, rect.width, header_h);
}

// ---------------------------------------------------------------------------
// 6. Panel — toolbar / sidebar / status-bar with PanelTheme bridge
// ---------------------------------------------------------------------------

/// Draw a panel container using the `PanelTheme` bridge color slots.
///
/// mlc `panels_render.rs` bridges `RuntimeTheme` into `PanelTheme`; toolbar/sidebar
/// panels use `toolbar_bg` → `panel_bg`, with a 1px bottom/side border.
///
/// Border placement by role:
/// - `Toolbar` — 1px bottom border (separates toolbar from chart area)
/// - `Sidebar` — 1px right border (separates sidebar from content)
/// - `StatusBar` — 1px top border (separates status from chart area)
pub fn draw_panel_container(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    theme: &dyn ContainerTheme,
    style: &PanelContainerStyle,
    role: PanelRole,
) {
    let _ = style; // padding is caller-managed layout; container draws bg only

    // Body fill.
    ctx.set_fill_color(theme.panel_bg());
    ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);

    // 1px separator line on the edge facing content.
    ctx.set_fill_color(theme.panel_border());
    match role {
        PanelRole::Toolbar => {
            // Bottom edge.
            ctx.fill_rect(rect.x, rect.y + rect.height - 1.0, rect.width, 1.0);
        }
        PanelRole::Sidebar => {
            // Right edge.
            ctx.fill_rect(rect.x + rect.width - 1.0, rect.y, 1.0, rect.height);
        }
        PanelRole::StatusBar => {
            // Top edge.
            ctx.fill_rect(rect.x, rect.y, rect.width, 1.0);
        }
    }
}

// ---------------------------------------------------------------------------
// Top-level dispatcher kept for simple callers
// ---------------------------------------------------------------------------

/// View descriptor passed to `draw_container`.
pub struct ContainerView {
    pub kind: ContainerType,
    pub border: bool,
}

/// Generic dispatcher — delegates to the appropriate typed render fn.
///
/// For call sites that hold a `ContainerType` value and want to avoid a manual
/// `match`. Typed fns above should be preferred when the variant is known
/// statically.
pub fn draw_container(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    view: &ContainerView,
    theme: &dyn ContainerTheme,
    style: &dyn ContainerStyle,
) {
    match view.kind {
        ContainerType::Plain => {
            ctx.set_fill_color(theme.bg());
            ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);
        }

        ContainerType::Bordered => {
            ctx.set_fill_color(theme.bg());
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, style.radius());
            if view.border {
                ctx.set_stroke_color(theme.border());
                ctx.set_stroke_width(style.border_width());
                ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, style.radius());
            }
        }

        ContainerType::Card => {
            let (sx, sy) = style.shadow_offset();
            ctx.set_fill_color_alpha(theme.card_shadow_color(), style.shadow_alpha());
            ctx.fill_rounded_rect(
                rect.x + sx,
                rect.y + sy,
                rect.width,
                rect.height,
                style.radius(),
            );
            ctx.reset_alpha();

            ctx.set_fill_color(theme.bg());
            if style.radius() > 0.0 {
                ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, style.radius());
            } else {
                ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);
            }

            if view.border {
                ctx.set_stroke_color(theme.border());
                ctx.set_stroke_width(style.border_width());
                if style.radius() > 0.0 {
                    ctx.stroke_rounded_rect(
                        rect.x,
                        rect.y,
                        rect.width,
                        rect.height,
                        style.radius(),
                    );
                } else {
                    ctx.stroke_rect(rect.x, rect.y, rect.width, rect.height);
                }
            }
        }

        ContainerType::Clip => {
            ctx.save();
            ctx.clip_rect(rect.x, rect.y, rect.width, rect.height);
            // Caller draws children; must call ctx.restore() when done.
        }

        ContainerType::Section => {
            ctx.set_fill_color(theme.bg());
            ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);

            // Default header height from the generic style padding field
            // (SectionContainerStyle stores it in body_padding — the dispatcher
            // falls back to a sensible default since dyn ContainerStyle doesn't
            // expose header_height).
            let header_h: f64 = 24.0;
            ctx.set_fill_color(theme.section_header_bg());
            ctx.fill_rect(rect.x, rect.y, rect.width, header_h.min(rect.height));
        }

        ContainerType::Panel(role) => {
            ctx.set_fill_color(theme.panel_bg());
            ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);

            ctx.set_fill_color(theme.panel_border());
            match role {
                PanelRole::Toolbar => {
                    ctx.fill_rect(rect.x, rect.y + rect.height - 1.0, rect.width, 1.0);
                }
                PanelRole::Sidebar => {
                    ctx.fill_rect(rect.x + rect.width - 1.0, rect.y, 1.0, rect.height);
                }
                PanelRole::StatusBar => {
                    ctx.fill_rect(rect.x, rect.y, rect.width, 1.0);
                }
            }
        }
    }
}
