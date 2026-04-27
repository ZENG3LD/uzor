//! Button rendering — ported from `mlc/chart/src/ui/widgets/button.rs` and
//! `mlc/chart/src/ui/toolbar_core.rs` / `toolbar_render.rs`.
//!
//! # Base button
//! `draw_button` — bg → active border → icon → text. Used by all standard
//! button variants.
//!
//! # Toolbar variants (sections 2-11 of button-full.md)
//! One function per mlc `ToolbarItem` variant plus the panel-toolbar
//! orchestrator (`draw_panel_toolbar`).

use crate::render::{RenderContext, TextAlign, TextBaseline};
use crate::types::{IconId, Rect, WidgetState};

use super::state::SplitButtonHoverZone;
use super::theme::ButtonTheme;

use super::settings::ButtonSettings;
use super::style::DropdownMenuRowStyle;

// ─── Per-frame rendering inputs ─────────────────────────────────────────────

/// What the caller hands to `draw_button` each frame.
pub struct ButtonView<'a> {
    /// Optional icon (drawn left of text, vertically centered).
    pub icon: Option<&'a IconId>,
    /// Optional label.
    pub text: Option<&'a str>,
    /// `true` if this is a Toggle/Checkbox/Tab in its "ON" state.
    pub active: bool,
    /// Disabled fields render in disabled colours and ignore hover/press.
    pub disabled: bool,
    /// Per-instance override for the active-border stroke.
    ///
    /// `Some(true)` forces the border on; `Some(false)` suppresses it even
    /// when `ButtonStyle::show_active_border()` returns `true`.
    /// `None` defers to `ButtonStyle::show_active_border()` (style-level default).
    ///
    /// Mirrors mlc `ButtonConfig::active_border` which is per-instance.
    pub active_border: Option<bool>,
}

/// What the renderer returns. `clicked`/`hovered`/`pressed` are derived
/// from `WidgetState` so the caller can react without re-querying the
/// coordinator.
#[derive(Debug, Default, Clone, Copy)]
pub struct ButtonResult {
    pub clicked: bool,
    pub hovered: bool,
    pub pressed: bool,
}

// ─── Public render entry point ──────────────────────────────────────────────

/// Render the button (background, optional active border, optional icon,
/// optional text). Returns interaction flags. Caller plugs in icon
/// rendering via `draw_icon` (closure can ignore the call if no icon).
pub fn draw_button<F>(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    state: WidgetState,
    view: &ButtonView<'_>,
    settings: &ButtonSettings,
    draw_icon: F,
) -> ButtonResult
where
    F: FnOnce(&mut dyn RenderContext, &IconId, Rect, &str),
{
    let style = settings.style.as_ref();
    let theme = settings.theme.as_ref();

    let effective = if view.disabled {
        WidgetState::Disabled
    } else {
        state
    };

    // ─── Colour selection by state ──────────────────────────────────────────
    let (bg, text_color) = match effective {
        WidgetState::Disabled => (theme.button_bg_disabled(),  theme.button_text_disabled()),
        WidgetState::Pressed  => (theme.button_bg_pressed(),   theme.button_text_hover()),
        WidgetState::Hovered  => (theme.button_bg_hover(),     theme.button_text_hover()),
        WidgetState::Active | WidgetState::Toggled => {
            // Explicit active/toggled states mirror mlc's Normal+active=true path.
            (theme.button_accent(), theme.button_text_hover())
        }
        WidgetState::Normal => {
            if view.active {
                // mlc: Normal + active=true → accent bg, text_hover text.
                (theme.button_accent(), theme.button_text_hover())
            } else {
                (theme.button_bg_normal(), theme.button_text_normal())
            }
        }
    };

    let radius = style.radius();

    // Background
    ctx.set_fill_color(bg);
    ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, radius);

    // Active border — per-instance flag (view.active_border) takes priority;
    // falls back to style-level default (show_active_border()).
    // Stroke color mirrors mlc: theme.accent (= button_accent()).
    let show_border = view.active_border.unwrap_or_else(|| style.show_active_border());
    if view.active && show_border {
        ctx.set_stroke_color(theme.button_accent());
        ctx.set_stroke_width(style.border_width());
        ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, radius);
    }

    // ─── Content layout ────────────────────────────────────────────────────
    // mlc uses min(padding_x, padding_y) as a single inset — keep that
    // behaviour for parity with existing visuals.
    let padding = style.padding_x().min(style.padding_y());
    let content_rect = Rect::new(
        rect.x + padding,
        rect.y + padding,
        rect.width  - padding * 2.0,
        rect.height - padding * 2.0,
    );

    let mut text_x = content_rect.x;

    if let Some(icon) = view.icon {
        let icon_size = style.icon_size();
        let icon_rect = Rect::new(
            content_rect.x,
            content_rect.y + content_rect.height / 2.0 - icon_size / 2.0,
            icon_size,
            icon_size,
        );
        draw_icon(ctx, icon, icon_rect, text_color);
        text_x = icon_rect.x + icon_rect.width + style.gap();
    }

    if let Some(text) = view.text {
        ctx.set_font(&format!("{}px sans-serif", style.font_size()));
        ctx.set_fill_color(text_color);
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Middle);
        ctx.fill_text(text, text_x, rect.y + rect.height / 2.0);
    }

    ButtonResult {
        clicked: matches!(effective, WidgetState::Pressed),
        hovered: effective.is_hovered(),
        pressed: effective.is_pressed(),
    }
}

// =============================================================================
// Toolbar variant view structs
// =============================================================================

/// Per-instance data for `draw_toolbar_color_button`.
pub struct ColorButtonView<'a> {
    pub icon:    &'a IconId,
    /// CSS color string for the swatch bar (e.g. `"#ff0000"`).
    pub color:   &'a str,
    pub active:  bool,
    pub hovered: bool,
}

/// Per-instance data for `draw_toolbar_line_width_button`.
pub struct LineWidthButtonView {
    /// Line thickness in logical pixels (clamped 1-4 during render).
    pub width:   u32,
    pub active:  bool,
    pub hovered: bool,
}

/// Per-instance data for `draw_toolbar_dropdown_trigger`.
pub struct DropdownTriggerView<'a> {
    pub icon:         Option<&'a IconId>,
    pub text:         Option<&'a str>,
    pub active:       bool,
    pub hovered:      bool,
    /// Render the filled-triangle chevron at the right edge.
    pub show_chevron: bool,
}

/// Per-instance data for `draw_toolbar_split_icon_button`.
pub struct SplitIconButtonView<'a> {
    pub icon:      &'a IconId,
    pub active:    bool,
    /// Which sub-zone the pointer is currently in.
    pub hover_zone: SplitButtonHoverZone,
}

/// Per-instance data for `draw_toolbar_split_line_width_button`.
pub struct SplitLineWidthButtonView {
    pub width:     u32,
    pub active:    bool,
    /// Which sub-zone the pointer is currently in.
    pub hover_zone: SplitButtonHoverZone,
}

// ClockView and LabelView have been extracted to atomic::clock and atomic::item.
// Backward-compat re-exports live in button/mod.rs.

/// One hit-rect entry returned by toolbar render functions.
#[derive(Debug, Clone)]
pub struct ToolbarHitRect {
    /// Item identifier — mirrors `ToolbarItem::id`.
    pub id:   String,
    pub rect: Rect,
}

/// Result of `draw_panel_toolbar` — caller maps pointer events to items.
#[derive(Debug, Default, Clone)]
pub struct PanelToolbarResult {
    pub item_rects: Vec<ToolbarHitRect>,
}

// =============================================================================
// Toolbar internal helpers
// =============================================================================

/// Shared: pick icon/text colour by (hovered, active) tuple.
/// Mirrors mlc `pick_color()` in `toolbar_core.rs` and `toolbar_render.rs`.
#[inline]
fn toolbar_pick_color<'t>(
    is_hovered: bool,
    is_active:  bool,
    theme:      &'t dyn ButtonTheme,
) -> &'t str {
    if is_active       { theme.toolbar_item_text_active() }
    else if is_hovered { theme.toolbar_item_text_hover()  }
    else               { theme.toolbar_item_text()        }
}

/// Shared: draw hover/active rounded rect.
/// Mirrors mlc `draw_item_bg()` from `toolbar_render.rs`.
#[inline]
fn toolbar_draw_item_bg(
    ctx:        &mut dyn RenderContext,
    rect:       Rect,
    is_hovered: bool,
    is_active:  bool,
    theme:      &dyn ButtonTheme,
) {
    if is_active {
        ctx.draw_active_rounded_rect(
            rect.x, rect.y, rect.width, rect.height,
            4.0, theme.toolbar_item_bg_active(),
        );
    } else if is_hovered {
        ctx.draw_hover_rounded_rect(
            rect.x, rect.y, rect.width, rect.height,
            4.0, theme.toolbar_item_bg_hover(),
        );
    }
}

/// Draw the tiny (2.5px half, 1.2px stroke) downward chevron used by
/// `SplitIconButton` and `SplitLineWidthButton`.
/// Mirrors mlc inline chevron in `draw_toolbar_with_icons`.
#[inline]
fn toolbar_draw_tiny_chevron(
    ctx:   &mut dyn RenderContext,
    cx:    f64,
    cy:    f64,
    color: &str,
) {
    let cs = 2.5_f64;
    ctx.set_stroke_color(color);
    ctx.set_stroke_width(1.2);
    ctx.set_line_dash(&[]);
    ctx.begin_path();
    ctx.move_to(cx - cs, cy - cs / 2.0);
    ctx.line_to(cx,      cy + cs / 2.0);
    ctx.line_to(cx + cs, cy - cs / 2.0);
    ctx.stroke();
}

/// Draw the filled-triangle dropdown chevron used by `Dropdown` items in
/// `toolbar_core.rs`.
#[inline]
fn toolbar_draw_filled_chevron(
    ctx:        &mut dyn RenderContext,
    item_rect:  Rect,
    color:      &str,
) {
    let chevron_x    = item_rect.right() - 10.0;
    let chevron_y    = item_rect.center_y();
    let chevron_size = 4.0_f64;

    ctx.set_fill_color(color);
    ctx.begin_path();
    ctx.move_to(chevron_x - chevron_size, chevron_y - chevron_size / 2.0);
    ctx.line_to(chevron_x,               chevron_y + chevron_size / 2.0);
    ctx.line_to(chevron_x + chevron_size, chevron_y - chevron_size / 2.0);
    ctx.close_path();
    ctx.fill();
}

/// Draw the stroked chevron used by `toolbar_render.rs` panel-toolbar
/// dropdowns.  Half = 3px, stroke 1.5.
#[inline]
fn panel_toolbar_draw_chevron(
    ctx:       &mut dyn RenderContext,
    item_rect: Rect,
    color:     &str,
) {
    let cx   = item_rect.right() - 8.0;
    let cy   = item_rect.center_y();
    let half = 3.0_f64;
    ctx.set_stroke_color(color);
    ctx.set_stroke_width(1.5);
    ctx.set_line_dash(&[]);
    ctx.begin_path();
    ctx.move_to(cx - half, cy - 1.5);
    ctx.line_to(cx,        cy + 1.5);
    ctx.line_to(cx + half, cy - 1.5);
    ctx.stroke();
}

/// Draw label text as used in `toolbar_render.rs`:
/// `font "11px sans-serif"`, `center_y + 4.0` baseline (not Middle).
/// `has_icon` shifts text right by `item_rect.height + 2.0`.
#[inline]
fn panel_toolbar_draw_label(
    ctx:       &mut dyn RenderContext,
    text:      &str,
    item_rect: Rect,
    color:     &str,
    has_icon:  bool,
) {
    ctx.set_fill_color(color);
    ctx.set_font("11px sans-serif");
    let x = if has_icon {
        item_rect.x + item_rect.height + 2.0
    } else {
        item_rect.center_x()
    };
    ctx.fill_text(text, x, item_rect.center_y() + 4.0);
}

/// Draw the line+number content shared by `LineWidthButton` and
/// `SplitLineWidthButton`.
#[inline]
fn toolbar_draw_line_and_number(
    ctx:       &mut dyn RenderContext,
    item_rect: Rect,
    width:     u32,
    color:     &str,
) {
    let thickness = (width as f64).clamp(1.0, 4.0);
    ctx.set_stroke_color(color);
    ctx.set_stroke_width(thickness);
    ctx.set_line_dash(&[]);
    ctx.begin_path();
    ctx.move_to(item_rect.x + 4.0,  item_rect.center_y());
    ctx.line_to(item_rect.x + 16.0, item_rect.center_y());
    ctx.stroke();

    ctx.set_font("12px sans-serif");
    ctx.set_fill_color(color);
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(&format!("{width}"), item_rect.x + 20.0, item_rect.center_y());
}

// =============================================================================
// Toolbar variant #2 — IconButton
// =============================================================================

/// Render a toolbar icon-only button.
///
/// Ports `ToolbarItem::IconButton` from `toolbar_core.rs` / `draw_toolbar_with_icons`.
///
/// # Sidebar active visual
/// When `is_sidebar` is `true` and `active` is set, draws a full-width
/// highlight (`item_rect` already full-width) plus a 3 px left-edge accent bar
/// via `ctx.draw_sidebar_active_item`.
///
/// # Arguments
/// - `item_rect` — pre-computed pixel rect for this item (caller handles layout).
/// - `is_sidebar` — `true` for vertical sidebar toolbars that use the
///   left-accent-bar active style.
/// - `draw_icon` — caller-supplied closure: `(ctx, icon, icon_rect, color)`.
pub fn draw_toolbar_icon_button<F>(
    ctx:       &mut dyn RenderContext,
    item_rect: Rect,
    icon:      &IconId,
    active:    bool,
    disabled:  bool,
    hovered:   bool,
    is_sidebar: bool,
    icon_size:  f64,
    theme:     &dyn ButtonTheme,
    draw_icon: F,
)
where
    F: FnOnce(&mut dyn RenderContext, &IconId, Rect, &str),
{
    let color = if disabled {
        theme.toolbar_item_text()
    } else if active {
        if is_sidebar {
            ctx.draw_sidebar_active_item(
                item_rect.x, item_rect.y, item_rect.width, item_rect.height,
                theme.toolbar_accent(), theme.toolbar_item_bg_active(), 3.0,
            );
        } else {
            ctx.draw_active_rounded_rect(
                item_rect.x, item_rect.y, item_rect.width, item_rect.height,
                4.0, theme.toolbar_item_bg_active(),
            );
        }
        theme.toolbar_item_text_active()
    } else if hovered {
        ctx.draw_hover_rounded_rect(
            item_rect.x, item_rect.y, item_rect.width, item_rect.height,
            4.0, theme.toolbar_item_bg_hover(),
        );
        theme.toolbar_item_text_hover()
    } else {
        theme.toolbar_item_text()
    };

    let icon_rect = Rect::new(
        item_rect.center_x() - icon_size / 2.0,
        item_rect.center_y() - icon_size / 2.0,
        icon_size,
        icon_size,
    );
    draw_icon(ctx, icon, icon_rect, color);
}

// =============================================================================
// Toolbar variant #3 — Button (text and/or icon)
// =============================================================================

/// Render a toolbar button that may carry an icon, a text label, or both.
///
/// Ports `ToolbarItem::Button` from `toolbar_core.rs` / `draw_toolbar_with_icons`.
///
/// Text layout:
/// - Icon present → text at `item_rect.x + icon_size + 4.0`, Left-aligned.
/// - Icon absent  → text at `item_rect.center_x()`, Center-aligned.
pub fn draw_toolbar_button<F>(
    ctx:       &mut dyn RenderContext,
    item_rect: Rect,
    icon:      Option<&IconId>,
    text:      Option<&str>,
    active:    bool,
    disabled:  bool,
    hovered:   bool,
    icon_size:  f64,
    theme:     &dyn ButtonTheme,
    draw_icon: F,
)
where
    F: FnOnce(&mut dyn RenderContext, &IconId, Rect, &str),
{
    let color = if disabled {
        theme.toolbar_item_text()
    } else if active {
        ctx.draw_active_rounded_rect(
            item_rect.x, item_rect.y, item_rect.width, item_rect.height,
            4.0, theme.toolbar_item_bg_active(),
        );
        theme.toolbar_item_text_active()
    } else if hovered {
        ctx.draw_hover_rounded_rect(
            item_rect.x, item_rect.y, item_rect.width, item_rect.height,
            4.0, theme.toolbar_item_bg_hover(),
        );
        theme.toolbar_item_text_hover()
    } else {
        theme.toolbar_item_text()
    };

    if let Some(ic) = icon {
        let icon_rect = Rect::new(
            item_rect.center_x() - icon_size / 2.0,
            item_rect.center_y() - icon_size / 2.0,
            icon_size,
            icon_size,
        );
        draw_icon(ctx, ic, icon_rect, color);
    }

    if let Some(label) = text {
        let (text_x, align) = if icon.is_some() {
            (item_rect.x + icon_size + 4.0, TextAlign::Left)
        } else {
            (item_rect.center_x(), TextAlign::Center)
        };
        ctx.set_font("13px sans-serif");
        ctx.set_fill_color(color);
        ctx.set_text_align(align);
        ctx.set_text_baseline(TextBaseline::Middle);
        ctx.fill_text(label, text_x, item_rect.center_y());
    }
}

// =============================================================================
// Toolbar variant #4 — Dropdown Trigger
// =============================================================================

/// Render a toolbar dropdown trigger (icon, optional text, optional filled
/// down-triangle chevron).
///
/// Ports `ToolbarItem::Dropdown` from `toolbar_core.rs` / `draw_toolbar_with_icons`.
///
/// Differences from `draw_toolbar_button`:
/// - Icon is offset at `item_rect.x + 4.0` (not centred).
/// - Text is always Left-aligned at `x + icon_size + 8.0`.
/// - Optional filled-triangle chevron at the right edge.
/// - No `disabled` state.
pub fn draw_toolbar_dropdown_trigger<F>(
    ctx:       &mut dyn RenderContext,
    item_rect: Rect,
    view:      &DropdownTriggerView<'_>,
    icon_size:  f64,
    theme:     &dyn ButtonTheme,
    draw_icon: F,
)
where
    F: FnOnce(&mut dyn RenderContext, &IconId, Rect, &str),
{
    let color = toolbar_pick_color(view.hovered, view.active, theme);

    if view.active {
        ctx.draw_active_rounded_rect(
            item_rect.x, item_rect.y, item_rect.width, item_rect.height,
            4.0, theme.toolbar_item_bg_active(),
        );
    } else if view.hovered {
        ctx.draw_hover_rounded_rect(
            item_rect.x, item_rect.y, item_rect.width, item_rect.height,
            4.0, theme.toolbar_item_bg_hover(),
        );
    }

    if let Some(ic) = view.icon {
        let icon_rect = Rect::new(
            item_rect.x + 4.0,
            item_rect.center_y() - icon_size / 2.0,
            icon_size,
            icon_size,
        );
        draw_icon(ctx, ic, icon_rect, color);
    }

    if let Some(label) = view.text {
        let text_x = item_rect.x + icon_size + 8.0;
        ctx.set_font("13px sans-serif");
        ctx.set_fill_color(color);
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Middle);
        ctx.fill_text(label, text_x, item_rect.center_y());
    }

    if view.show_chevron {
        toolbar_draw_filled_chevron(ctx, item_rect, color);
    }
}

// =============================================================================
// Toolbar variant #5 — ColorButton
// =============================================================================

/// Render a toolbar color button: icon (top-biased) + 3 px color swatch bar
/// at the bottom.
///
/// Ports `ToolbarItem::ColorButton` from `toolbar_core.rs` / `draw_toolbar_with_icons`.
///
/// Color bar: `fill_rect(x+4, bottom-6, width-8, 3)` in `view.color`.
pub fn draw_toolbar_color_button<F>(
    ctx:       &mut dyn RenderContext,
    item_rect: Rect,
    view:      &ColorButtonView<'_>,
    icon_size:  f64,
    theme:     &dyn ButtonTheme,
    draw_icon: F,
)
where
    F: FnOnce(&mut dyn RenderContext, &IconId, Rect, &str),
{
    let color = toolbar_pick_color(view.hovered, view.active, theme);
    toolbar_draw_item_bg(ctx, item_rect, view.hovered, view.active, theme);

    let icon_rect = Rect::new(
        item_rect.center_x() - icon_size / 2.0,
        item_rect.y + 2.0,          // top-biased, not centred
        icon_size,
        icon_size,
    );
    draw_icon(ctx, view.icon, icon_rect, color);

    // 3 px color swatch bar at the bottom
    ctx.set_fill_color(view.color);
    ctx.fill_rect(
        item_rect.x + 4.0,
        item_rect.bottom() - 6.0,
        item_rect.width - 8.0,
        3.0,
    );
}

// =============================================================================
// Toolbar variant #6 — LineWidthButton
// =============================================================================

/// Render a toolbar line-width button: short line segment + numeric label.
///
/// Ports `ToolbarItem::LineWidthButton` from `toolbar_core.rs`.
///
/// Layout (all relative to `item_rect.x`):
/// - Line: `(x+4, center_y) → (x+16, center_y)`, stroke = `width.clamp(1,4)`.
/// - Number: `"{width}"`, font `"12px sans-serif"`, at `(x+20, center_y)`,
///   Left+Middle.
pub fn draw_toolbar_line_width_button(
    ctx:       &mut dyn RenderContext,
    item_rect: Rect,
    view:      &LineWidthButtonView,
    theme:     &dyn ButtonTheme,
) {
    let color = toolbar_pick_color(view.hovered, view.active, theme);
    toolbar_draw_item_bg(ctx, item_rect, view.hovered, view.active, theme);
    toolbar_draw_line_and_number(ctx, item_rect, view.width, color);
}

// =============================================================================
// Toolbar variant #7 — SplitIconButton
// =============================================================================

/// Render a split icon button: main area (icon) + 10 px chevron trigger.
///
/// Ports `ToolbarItem::SplitIconButton` from `toolbar_core.rs`.
///
/// Returns `(main_rect, chevron_rect)` so the caller can register two
/// independent hit-rects (IDs: `id` and `"{id}_menu"`).
///
/// Background covers the full compound rect.  Either half being hovered
/// highlights the entire compound rect.
pub fn draw_toolbar_split_icon_button<F>(
    ctx:        &mut dyn RenderContext,
    item_rect:  Rect,
    view:       &SplitIconButtonView<'_>,
    icon_size:   f64,
    theme:      &dyn ButtonTheme,
    draw_icon:  F,
) -> (Rect, Rect)
where
    F: FnOnce(&mut dyn RenderContext, &IconId, Rect, &str),
{
    const CHEVRON_W: f64 = 10.0;
    let main_w     = item_rect.width - CHEVRON_W;
    let main_rect  = Rect::new(item_rect.x,           item_rect.y, main_w,    item_rect.height);
    let chev_rect  = Rect::new(item_rect.x + main_w,  item_rect.y, CHEVRON_W, item_rect.height);

    let is_any_hovered = view.hover_zone != SplitButtonHoverZone::None;
    let color = toolbar_pick_color(is_any_hovered, view.active, theme);

    // Background covers both halves as one rounded rect
    if view.active {
        ctx.draw_active_rounded_rect(
            item_rect.x, item_rect.y, item_rect.width, item_rect.height,
            4.0, theme.toolbar_item_bg_active(),
        );
    } else if is_any_hovered {
        ctx.draw_hover_rounded_rect(
            item_rect.x, item_rect.y, item_rect.width, item_rect.height,
            4.0, theme.toolbar_item_bg_hover(),
        );
    }

    // Icon in the main area
    let effective_icon_size = icon_size.min(main_w - 2.0);
    let icon_rect = Rect::new(
        main_rect.center_x() - effective_icon_size / 2.0,
        main_rect.center_y() - effective_icon_size / 2.0,
        effective_icon_size,
        effective_icon_size,
    );
    draw_icon(ctx, view.icon, icon_rect, color);

    // Tiny chevron in the chevron area
    toolbar_draw_tiny_chevron(ctx, chev_rect.center_x(), chev_rect.center_y(), color);

    (main_rect, chev_rect)
}

// =============================================================================
// Toolbar variant #8 — SplitLineWidthButton
// =============================================================================

/// Render a split line-width button: main area (line+number) + 10 px chevron.
///
/// Ports `ToolbarItem::SplitLineWidthButton` from `toolbar_core.rs`.
///
/// The `item_rect` supplied by the caller should be `46 px` wide
/// (`main_w=36 + CHEVRON_W=10`).
///
/// Returns `(main_rect, chevron_rect)`.
pub fn draw_toolbar_split_line_width_button(
    ctx:       &mut dyn RenderContext,
    item_rect: Rect,
    view:      &SplitLineWidthButtonView,
    theme:     &dyn ButtonTheme,
) -> (Rect, Rect) {
    const CHEVRON_W: f64 = 10.0;
    let main_w     = item_rect.width - CHEVRON_W;
    let main_rect  = Rect::new(item_rect.x,          item_rect.y, main_w,    item_rect.height);
    let chev_rect  = Rect::new(item_rect.x + main_w, item_rect.y, CHEVRON_W, item_rect.height);

    let is_any_hovered = view.hover_zone != SplitButtonHoverZone::None;
    let color = toolbar_pick_color(is_any_hovered, view.active, theme);

    // Background covers both halves
    if view.active {
        ctx.draw_active_rounded_rect(
            item_rect.x, item_rect.y, item_rect.width, item_rect.height,
            4.0, theme.toolbar_item_bg_active(),
        );
    } else if is_any_hovered {
        ctx.draw_hover_rounded_rect(
            item_rect.x, item_rect.y, item_rect.width, item_rect.height,
            4.0, theme.toolbar_item_bg_hover(),
        );
    }

    // Line + number in the main area
    toolbar_draw_line_and_number(ctx, main_rect, view.width, color);

    // Tiny chevron in the chevron area
    toolbar_draw_tiny_chevron(ctx, chev_rect.center_x(), chev_rect.center_y(), color);

    (main_rect, chev_rect)
}

// draw_toolbar_clock extracted to atomic::clock::draw_clock.
// draw_toolbar_label extracted to atomic::item::draw_item (Label variant).
// Backward-compat re-exports live in button/mod.rs.

// =============================================================================
// Toolbar variant #11 — draw_panel_toolbar
// =============================================================================

/// A single item in a panel toolbar passed to `draw_panel_toolbar`.
///
/// Each variant carries the data needed to render one logical toolbar item.
/// The caller builds this slice from its own data model (e.g. `PanelToolbarDef`).
pub enum PanelToolbarItem<'a> {
    /// Vertical or horizontal separator line.
    Separator,
    /// Flexible gap that takes a fixed number of pixels.
    Spacer(f64),
    /// Icon-only toggle / action button.
    IconButton {
        id:      &'a str,
        icon:    &'a IconId,
        active:  bool,
        hovered: bool,
    },
    /// Button with optional icon and/or label.
    Button {
        id:        &'a str,
        icon:      Option<&'a IconId>,
        text:      Option<&'a str>,
        active:    bool,
        hovered:   bool,
        min_width: f64,
    },
    /// Dropdown trigger.
    Dropdown {
        id:           &'a str,
        icon:         Option<&'a IconId>,
        text:         Option<&'a str>,
        active:       bool,
        hovered:      bool,
        /// When `true` the stroked-chevron (`toolbar_render.rs` style) is drawn.
        show_chevron: bool,
        min_width:    f64,
    },
    /// Color icon + 3 px swatch bar.
    ColorButton {
        id:      &'a str,
        icon:    &'a IconId,
        color:   &'a str,
        active:  bool,
        hovered: bool,
    },
    /// Line segment + number.
    LineWidthButton {
        id:      &'a str,
        width:   u32,
        active:  bool,
        hovered: bool,
    },
    /// Split: icon + chevron.
    SplitIconButton {
        id:         &'a str,
        icon:       &'a IconId,
        active:     bool,
        hover_zone: SplitButtonHoverZone,
    },
    /// Split: line+number + chevron.
    SplitLineWidthButton {
        id:         &'a str,
        width:      u32,
        active:     bool,
        hover_zone: SplitButtonHoverZone,
    },
    /// Clock display.
    Clock {
        id:      &'a str,
        time:    &'a str,
        hovered: bool,
    },
    /// Non-interactive label.
    Label {
        id:   &'a str,
        text: &'a str,
    },
}

/// Toolbar orientation for `draw_panel_toolbar`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelToolbarOrientation {
    Horizontal,
    Vertical,
}

/// Render the panel-level toolbar background, optional separator line, and
/// all toolbar items, returning hit-rects for all interactive items.
///
/// Ports `render_panel_toolbar` from `toolbar_render.rs`.
///
/// # Layout
/// Items are laid out sequentially along the primary axis starting at
/// `padding` from the toolbar edge.  Each item advances by its natural width
/// plus `spacing`.
///
/// # Arguments
/// - `rect`        — pixel bounds of the entire toolbar.
/// - `items`       — ordered slice of items to render.
/// - `orientation` — determines layout axis.
/// - `item_size`   — default square size for icon buttons (mlc default 28 px).
/// - `icon_size`   — icon side length (mlc default 16 px).
/// - `spacing`     — gap between items (mlc default 2 px).
/// - `padding`     — toolbar edge padding (mlc default 4 px).
/// - `draw_icon`   — caller-supplied icon renderer.
#[allow(clippy::too_many_arguments)]
pub fn draw_panel_toolbar<'a, F>(
    ctx:         &mut dyn RenderContext,
    rect:        Rect,
    items:       &[PanelToolbarItem<'a>],
    orientation: PanelToolbarOrientation,
    item_size:   f64,
    icon_size:   f64,
    spacing:     f64,
    padding:     f64,
    theme:       &dyn ButtonTheme,
    mut draw_icon: F,
) -> PanelToolbarResult
where
    F: FnMut(&mut dyn RenderContext, &IconId, Rect, &str),
{
    let mut result = PanelToolbarResult::default();

    // Draw toolbar background
    ctx.set_fill_color(theme.toolbar_background());
    ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);

    let is_vertical = orientation == PanelToolbarOrientation::Vertical;

    let mut pos = if is_vertical {
        rect.y + padding
    } else {
        rect.x + padding
    };

    for item in items {
        match item {
            PanelToolbarItem::Separator => {
                // Draw separator with 20% margin — mirrors `draw_separator` in toolbar_render.rs
                ctx.set_stroke_color(theme.toolbar_separator());
                ctx.set_stroke_width(1.0);
                ctx.set_line_dash(&[]);
                ctx.begin_path();
                if is_vertical {
                    let margin = rect.width * 0.2;
                    ctx.move_to(rect.x + margin,         pos + 3.0);
                    ctx.line_to(rect.right() - margin,   pos + 3.0);
                } else {
                    let margin = rect.height * 0.2;
                    ctx.move_to(pos + 3.0, rect.y + margin);
                    ctx.line_to(pos + 3.0, rect.bottom() - margin);
                }
                ctx.stroke();
                pos += 6.0; // separator thickness + gap
            }

            PanelToolbarItem::Spacer(w) => {
                pos += w;
            }

            PanelToolbarItem::IconButton { id, icon, active, hovered } => {
                let item_rect = make_panel_item_rect(is_vertical, rect, pos, item_size, padding);
                let color = toolbar_pick_color(*hovered, *active, theme);

                toolbar_draw_item_bg(ctx, item_rect, *hovered, *active, theme);
                render_icon_centered(ctx, icon, item_rect, icon_size, color, &mut draw_icon);

                push_hit_rect(&mut result, id, item_rect);
                pos += item_size + spacing;
            }

            PanelToolbarItem::Button { id, icon, text, active, hovered, min_width } => {
                let natural_w = if *min_width > 0.0 {
                    *min_width
                } else if text.is_some() {
                    item_size * 2.5
                } else {
                    item_size
                };
                let item_rect = make_panel_item_rect(is_vertical, rect, pos, natural_w, padding);
                let color = toolbar_pick_color(*hovered, *active, theme);

                toolbar_draw_item_bg(ctx, item_rect, *hovered, *active, theme);

                if let Some(ic) = icon {
                    render_icon_centered(ctx, ic, item_rect, icon_size, color, &mut draw_icon);
                }
                if let Some(label) = text {
                    panel_toolbar_draw_label(ctx, label, item_rect, color, icon.is_some());
                }

                push_hit_rect(&mut result, id, item_rect);
                pos += (if is_vertical { item_size } else { natural_w }) + spacing;
            }

            PanelToolbarItem::Dropdown { id, icon, text, active, hovered, show_chevron, min_width } => {
                let natural_w = if *min_width > 0.0 {
                    *min_width
                } else if text.is_some() {
                    item_size * 2.5
                } else {
                    item_size
                };
                let item_rect = make_panel_item_rect(is_vertical, rect, pos, natural_w, padding);
                let color = toolbar_pick_color(*hovered, *active, theme);

                toolbar_draw_item_bg(ctx, item_rect, *hovered, *active, theme);

                if let Some(ic) = icon {
                    render_icon_centered(ctx, ic, item_rect, icon_size, color, &mut draw_icon);
                }
                if let Some(label) = text {
                    panel_toolbar_draw_label(ctx, label, item_rect, color, icon.is_some());
                }
                if *show_chevron && text.is_some() {
                    // stroked chevron style from toolbar_render.rs
                    panel_toolbar_draw_chevron(ctx, item_rect, color);
                }

                push_hit_rect(&mut result, id, item_rect);
                pos += (if is_vertical { item_size } else { natural_w }) + spacing;
            }

            PanelToolbarItem::ColorButton { id, icon, color: swatch_color, active, hovered } => {
                let item_rect = make_panel_item_rect(is_vertical, rect, pos, item_size, padding);
                let text_color = toolbar_pick_color(*hovered, *active, theme);

                toolbar_draw_item_bg(ctx, item_rect, *hovered, *active, theme);

                let icon_rect = Rect::new(
                    item_rect.center_x() - icon_size / 2.0,
                    item_rect.y + 2.0,
                    icon_size,
                    icon_size,
                );
                draw_icon(ctx, icon, icon_rect, text_color);

                ctx.set_fill_color(swatch_color);
                ctx.fill_rect(
                    item_rect.x + 4.0,
                    item_rect.bottom() - 6.0,
                    item_rect.width - 8.0,
                    3.0,
                );

                push_hit_rect(&mut result, id, item_rect);
                pos += item_size + spacing;
            }

            PanelToolbarItem::LineWidthButton { id, width, active, hovered } => {
                let item_w    = 36.0_f64;
                let item_rect = make_panel_item_rect(is_vertical, rect, pos, item_w, padding);
                let color     = toolbar_pick_color(*hovered, *active, theme);

                toolbar_draw_item_bg(ctx, item_rect, *hovered, *active, theme);
                toolbar_draw_line_and_number(ctx, item_rect, *width, color);

                push_hit_rect(&mut result, id, item_rect);
                pos += item_w + spacing;
            }

            PanelToolbarItem::SplitIconButton { id, icon, active, hover_zone } => {
                const CHEVRON_W: f64 = 10.0;
                let main_w    = item_size;
                let total_w   = main_w + CHEVRON_W;
                let item_rect = make_panel_item_rect(is_vertical, rect, pos, total_w, padding);
                let main_rect = Rect::new(item_rect.x,          item_rect.y, main_w,    item_rect.height);
                let chev_rect = Rect::new(item_rect.x + main_w, item_rect.y, CHEVRON_W, item_rect.height);

                let is_any_hovered = *hover_zone != SplitButtonHoverZone::None;
                let color = toolbar_pick_color(is_any_hovered, *active, theme);

                if *active {
                    ctx.draw_active_rounded_rect(
                        item_rect.x, item_rect.y, item_rect.width, item_rect.height,
                        4.0, theme.toolbar_item_bg_active(),
                    );
                } else if is_any_hovered {
                    ctx.draw_hover_rounded_rect(
                        item_rect.x, item_rect.y, item_rect.width, item_rect.height,
                        4.0, theme.toolbar_item_bg_hover(),
                    );
                }

                let eff_icon_size = icon_size.min(main_w - 2.0);
                let icon_rect = Rect::new(
                    main_rect.center_x() - eff_icon_size / 2.0,
                    main_rect.center_y() - eff_icon_size / 2.0,
                    eff_icon_size,
                    eff_icon_size,
                );
                draw_icon(ctx, icon, icon_rect, color);
                toolbar_draw_tiny_chevron(ctx, chev_rect.center_x(), chev_rect.center_y(), color);

                push_hit_rect(&mut result, id, main_rect);
                push_hit_rect(&mut result, &format!("{id}_menu"), chev_rect);
                pos += total_w + spacing;
            }

            PanelToolbarItem::SplitLineWidthButton { id, width, active, hover_zone } => {
                const CHEVRON_W: f64 = 10.0;
                let main_w    = 36.0_f64;
                let total_w   = main_w + CHEVRON_W;
                let item_rect = make_panel_item_rect(is_vertical, rect, pos, total_w, padding);
                let main_rect = Rect::new(item_rect.x,          item_rect.y, main_w,    item_rect.height);
                let chev_rect = Rect::new(item_rect.x + main_w, item_rect.y, CHEVRON_W, item_rect.height);

                let is_any_hovered = *hover_zone != SplitButtonHoverZone::None;
                let color = toolbar_pick_color(is_any_hovered, *active, theme);

                if *active {
                    ctx.draw_active_rounded_rect(
                        item_rect.x, item_rect.y, item_rect.width, item_rect.height,
                        4.0, theme.toolbar_item_bg_active(),
                    );
                } else if is_any_hovered {
                    ctx.draw_hover_rounded_rect(
                        item_rect.x, item_rect.y, item_rect.width, item_rect.height,
                        4.0, theme.toolbar_item_bg_hover(),
                    );
                }

                toolbar_draw_line_and_number(ctx, main_rect, *width, color);
                toolbar_draw_tiny_chevron(ctx, chev_rect.center_x(), chev_rect.center_y(), color);

                push_hit_rect(&mut result, id, main_rect);
                push_hit_rect(&mut result, &format!("{id}_menu"), chev_rect);
                pos += total_w + spacing;
            }

            PanelToolbarItem::Clock { id, time, hovered } => {
                let clock_w   = 140.0_f64;
                let item_rect = if is_vertical {
                    Rect::new(rect.x, pos, rect.width, item_size)
                } else {
                    Rect::new(pos, rect.y, clock_w, rect.height)
                };

                if *hovered {
                    ctx.set_fill_color(theme.toolbar_item_bg_hover());
                    ctx.fill_rounded_rect(
                        item_rect.x,
                        item_rect.y + 2.0,
                        item_rect.width,
                        item_rect.height - 4.0,
                        4.0,
                    );
                }

                ctx.set_font("13px monospace");
                ctx.set_fill_color(theme.clock_text());
                ctx.set_text_align(TextAlign::Right);
                ctx.set_text_baseline(TextBaseline::Middle);
                ctx.fill_text(time, item_rect.right() - 8.0, item_rect.center_y());

                push_hit_rect(&mut result, id, item_rect);
                pos += clock_w + spacing;
            }

            PanelToolbarItem::Label { id, text } => {
                ctx.set_font("13px sans-serif");
                let text_w    = ctx.measure_text(text);
                let item_w    = text_w + 8.0;
                let item_rect = if is_vertical {
                    Rect::new(rect.x, pos, rect.width, item_size)
                } else {
                    Rect::new(pos, rect.y, item_w, rect.height)
                };

                ctx.set_fill_color(theme.toolbar_label_text());
                ctx.set_text_align(TextAlign::Left);
                ctx.set_text_baseline(TextBaseline::Middle);
                ctx.fill_text(text, item_rect.x + 4.0, item_rect.center_y());

                push_hit_rect(&mut result, id, item_rect);
                pos += item_w + spacing;
            }
        }
    }

    result
}

// =============================================================================
// Private layout helpers for draw_panel_toolbar
// =============================================================================

/// Build the pixel rect for one panel-toolbar item given its position on the
/// primary axis.  Mirrors `make_item_rect` in `toolbar_render.rs`.
fn make_panel_item_rect(
    is_vertical: bool,
    toolbar_rect: Rect,
    pos:          f64,
    size:         f64,
    padding:      f64,
) -> Rect {
    if is_vertical {
        let inset = padding * 0.5;
        Rect::new(toolbar_rect.x + inset, pos, toolbar_rect.width - inset * 2.0, size)
    } else {
        let inset = padding * 0.5;
        Rect::new(pos, toolbar_rect.y + inset, size, toolbar_rect.height - inset * 2.0)
    }
}

/// Render an icon centered in `item_rect`.
fn render_icon_centered<F>(
    ctx:       &mut dyn RenderContext,
    icon:      &IconId,
    item_rect: Rect,
    icon_size:  f64,
    color:     &str,
    draw_icon: &mut F,
)
where
    F: FnMut(&mut dyn RenderContext, &IconId, Rect, &str),
{
    let ir = Rect::new(
        item_rect.center_x() - icon_size / 2.0,
        item_rect.center_y() - icon_size / 2.0,
        icon_size,
        icon_size,
    );
    draw_icon(ctx, icon, ir, color);
}

/// Push a `ToolbarHitRect` into the result.
fn push_hit_rect(result: &mut PanelToolbarResult, id: &str, rect: Rect) {
    result.item_rects.push(ToolbarHitRect { id: id.to_string(), rect });
}

// =============================================================================
// Modal action button variants (sections 12-18)
// =============================================================================

// ─── Section 12 — Primary/Accent Button ─────────────────────────────────────

/// Per-instance data for `draw_primary_button`.
pub struct PrimaryButtonView<'a> {
    pub text:    &'a str,
    pub hovered: bool,
}

/// Render a primary/accent action button (OK, Save, Create).
///
/// Ports section 12 from button-full.md (`chart_settings.rs` OK/Save pattern).
///
/// Background: `button_primary_bg` / `button_primary_bg_hover`.
/// Text: white (`#ffffff`), bold, 12 px, centred.
/// Geometry: flat rect by default (caller passes `PrimaryButtonStyle` or own style).
///
/// # Arguments
/// - `radius` — corner radius; pass `0.0` for flat (chart_settings) or `4.0` for
///   rounded (preset_name_input). Passed explicitly so callers don't need a
///   second style struct just for this.
pub fn draw_primary_button(
    ctx:     &mut dyn RenderContext,
    rect:    Rect,
    view:    &PrimaryButtonView<'_>,
    radius:  f64,
    theme:   &dyn ButtonTheme,
) -> ButtonResult {
    let bg = if view.hovered {
        theme.button_primary_bg_hover()
    } else {
        theme.button_primary_bg()
    };

    ctx.set_fill_color(bg);
    ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, radius);

    ctx.set_fill_color("#ffffff");
    ctx.set_font("bold 12px sans-serif");
    ctx.set_text_align(TextAlign::Center);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(view.text, rect.center_x(), rect.center_y());

    ButtonResult {
        clicked: false,
        hovered: view.hovered,
        pressed: false,
    }
}

// ─── Section 13 — GhostOutline Button ───────────────────────────────────────

/// Per-instance data for `draw_ghost_outline_button`.
pub struct GhostOutlineButtonView<'a> {
    pub text:    &'a str,
    pub hovered: bool,
}

/// Render a ghost-outline button (Cancel, Template, Отмена).
///
/// Ports section 13 from button-full.md (`chart_settings.rs` Cancel pattern).
///
/// Idle: no fill, 1 px `toolbar_separator` stroke, `toolbar_item_text` text.
/// Hover: `toolbar_item_bg_hover` fill, `toolbar_item_text` stroke, `toolbar_item_text_hover` text.
///
/// # Arguments
/// - `radius` — 0.0 flat (chart_settings) or 4.0 rounded (preset_name / alert_settings).
pub fn draw_ghost_outline_button(
    ctx:     &mut dyn RenderContext,
    rect:    Rect,
    view:    &GhostOutlineButtonView<'_>,
    radius:  f64,
    theme:   &dyn ButtonTheme,
) -> ButtonResult {
    // Hover fill
    if view.hovered {
        ctx.set_fill_color(theme.toolbar_item_bg_hover());
        ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, radius);
    }

    // Stroke border — color shifts on hover
    let border_color = if view.hovered {
        theme.toolbar_item_text()
    } else {
        theme.toolbar_separator()
    };
    ctx.set_stroke_color(border_color);
    ctx.set_stroke_width(1.0);
    ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, radius);

    // Text
    let text_color = if view.hovered {
        theme.toolbar_item_text_hover()
    } else {
        theme.toolbar_item_text()
    };
    ctx.set_fill_color(text_color);
    ctx.set_font("13px sans-serif");
    ctx.set_text_align(TextAlign::Center);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(view.text, rect.center_x(), rect.center_y());

    ButtonResult {
        clicked: false,
        hovered: view.hovered,
        pressed: false,
    }
}

// ─── Sections 14/15 — Danger Button ─────────────────────────────────────────

/// Selects between the LogOut and Delete visual variants of the danger button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DangerVariant {
    /// profile_manager LogOut: semi-transparent fill, no explicit border stroke,
    /// icon + text. Height 30.
    LogOut,
    /// user_settings Delete: semi-transparent fill + border stroke, text-only.
    /// Height 22. Caller should pass a compact rect.
    Delete,
}

/// Per-instance data for `draw_danger_button`.
pub struct DangerButtonView<'a> {
    pub text:    &'a str,
    pub hovered: bool,
    pub variant: DangerVariant,
    /// Optional icon drawn to the left of the text (used in LogOut variant).
    pub icon:    Option<&'a IconId>,
}

/// Render a danger (destructive action) button.
///
/// Ports sections 14 (`LogOut`) and 15 (`Delete`) from button-full.md.
///
/// `LogOut`: semi-transparent red fill, no border, icon+text at 14px icon / bold 11px text.
/// `Delete`: semi-transparent red fill + 1px red border, text-only 11px.
///
/// # Arguments
/// - `radius` — typically 4.0 for LogOut, 3.0 for Delete.
/// - `draw_icon` — icon renderer closure (ignored when `view.icon` is `None`).
pub fn draw_danger_button<F>(
    ctx:       &mut dyn RenderContext,
    rect:      Rect,
    view:      &DangerButtonView<'_>,
    radius:    f64,
    theme:     &dyn ButtonTheme,
    draw_icon: F,
) -> ButtonResult
where
    F: FnOnce(&mut dyn RenderContext, &IconId, Rect, &str),
{
    // Background
    let bg = if view.hovered {
        theme.button_danger_bg_hover()
    } else {
        theme.button_danger_bg()
    };
    ctx.set_fill_color(bg);
    ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, radius);

    // Border — only for Delete variant
    if view.variant == DangerVariant::Delete {
        let border = if view.hovered {
            theme.button_danger_border_hover()
        } else {
            theme.button_danger_border()
        };
        ctx.set_stroke_color(border);
        ctx.set_stroke_width(1.0);
        ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, radius);
    }

    let text_color = theme.button_danger_text();

    // Icon (LogOut)
    let text_x = if let Some(icon) = view.icon {
        let icon_size = 14.0_f64;
        let icon_rect = Rect::new(
            rect.x + 10.0,
            rect.center_y() - icon_size / 2.0,
            icon_size,
            icon_size,
        );
        draw_icon(ctx, icon, icon_rect, text_color);
        icon_rect.x + icon_rect.width + 6.0
    } else {
        // Delete: no icon — consume the closure without calling it
        let _ = draw_icon;
        rect.center_x()
    };

    // Text
    let (align, font) = if view.icon.is_some() {
        (TextAlign::Left, "bold 11px sans-serif")
    } else {
        (TextAlign::Center, "11px sans-serif")
    };
    ctx.set_fill_color(text_color);
    ctx.set_font(font);
    ctx.set_text_align(align);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(view.text, text_x, rect.center_y());

    ButtonResult {
        clicked: false,
        hovered: view.hovered,
        pressed: false,
    }
}

// ─── Section 16 — SecondaryNeutral Button ────────────────────────────────────

/// Per-instance data for `draw_secondary_neutral_button`.
pub struct SecondaryNeutralButtonView<'a> {
    pub text:    &'a str,
    pub hovered: bool,
}

/// Render a secondary-neutral action button (Open Dashboard, Show Welcome Wizard,
/// Rename, Avatar).
///
/// Ports section 16 from button-full.md (`user_settings.rs` neutral pattern).
///
/// Idle:  `toolbar_item_bg_hover` fill + `toolbar_separator` stroke.
/// Hover: `button_secondary_hover_bg` fill + `toolbar_separator` stroke.
/// Text:  `button_secondary_text_muted` idle → `button_secondary_text` hover.
///
/// # Arguments
/// - `radius` — 4.0 for full-width buttons, 3.0 for compact row buttons.
pub fn draw_secondary_neutral_button(
    ctx:     &mut dyn RenderContext,
    rect:    Rect,
    view:    &SecondaryNeutralButtonView<'_>,
    radius:  f64,
    font:    &str,
    theme:   &dyn ButtonTheme,
) -> ButtonResult {
    // Fill
    let bg = if view.hovered {
        theme.button_secondary_hover_bg()
    } else {
        theme.toolbar_item_bg_hover()
    };
    ctx.set_fill_color(bg);
    ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, radius);

    // Border
    ctx.set_stroke_color(theme.toolbar_separator());
    ctx.set_stroke_width(1.0);
    ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, radius);

    // Text
    let text_color = if view.hovered {
        theme.button_secondary_text()
    } else {
        theme.button_secondary_text_muted()
    };
    ctx.set_fill_color(text_color);
    ctx.set_font(font);
    ctx.set_text_align(TextAlign::Center);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(view.text, rect.center_x(), rect.center_y());

    ButtonResult {
        clicked: false,
        hovered: view.hovered,
        pressed: false,
    }
}

// ─── Section 17 — SignIn Button ───────────────────────────────────────────────

/// Per-instance data for `draw_signin_button`.
pub struct SignInButtonView<'a> {
    pub text:    &'a str,
    pub hovered: bool,
    /// Optional icon drawn to the left (profile_manager uses Icon::LogIn, 16x16).
    pub icon:    Option<&'a IconId>,
}

/// Render the SignIn button from `profile_manager.rs` left-column.
///
/// Ports section 17 from button-full.md.
///
/// Uses `button_utility_bg` / `button_utility_bg_hover` (distinct from
/// `toolbar_item_bg_hover`). Icon at `rect.x + 10`, text bold 11px, accent text
/// colour (`toolbar_item_text_active`).
///
/// # Arguments
/// - `draw_icon` — icon renderer closure (ignored if `view.icon` is `None`).
pub fn draw_signin_button<F>(
    ctx:       &mut dyn RenderContext,
    rect:      Rect,
    view:      &SignInButtonView<'_>,
    theme:     &dyn ButtonTheme,
    draw_icon: F,
) -> ButtonResult
where
    F: FnOnce(&mut dyn RenderContext, &IconId, Rect, &str),
{
    let bg = if view.hovered {
        theme.button_utility_bg_hover()
    } else {
        theme.button_utility_bg()
    };
    ctx.set_fill_color(bg);
    ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, 4.0);

    let text_color = theme.toolbar_item_text_active();

    let text_x = if let Some(icon) = view.icon {
        let icon_size = 16.0_f64;
        let icon_rect = Rect::new(
            rect.x + 10.0,
            rect.center_y() - icon_size / 2.0,
            icon_size,
            icon_size,
        );
        draw_icon(ctx, icon, icon_rect, text_color);
        icon_rect.x + icon_rect.width + 6.0
    } else {
        let _ = draw_icon;
        rect.center_x()
    };

    let align = if view.icon.is_some() { TextAlign::Left } else { TextAlign::Center };
    ctx.set_fill_color(text_color);
    ctx.set_font("bold 11px sans-serif");
    ctx.set_text_align(align);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(view.text, text_x, rect.center_y());

    ButtonResult {
        clicked: false,
        hovered: view.hovered,
        pressed: false,
    }
}

// ─── Section 19 — SidebarTab Button ──────────────────────────────────────────

/// Per-instance data for `draw_sidebar_tab_button`.
pub struct SidebarTabView<'a> {
    pub icon:    &'a IconId,
    /// `true` → 3 px accent bar + active-bg fill; `false` → no background.
    pub active:  bool,
}

/// Render a vertical sidebar tab button: icon centred, active state via
/// `draw_sidebar_active_item` (3 px left accent bar + filled rect).
///
/// Ports section 19 from button-full.md (`chart_settings.rs`, `indicator_settings.rs`,
/// `user_settings.rs` left sidebar tab pattern).
///
/// No hover state — sidebar tabs are active-or-normal only (mlc source: no hover
/// highlight on sidebar tab items).
///
/// # Arguments
/// - `item_rect`  — full-width rect for this tab cell (caller handles layout).
/// - `icon_size`  — icon side length; mlc default `20.0`.
/// - `draw_icon`  — caller-supplied closure: `(ctx, icon, icon_rect, color)`.
pub fn draw_sidebar_tab_button<F>(
    ctx:       &mut dyn RenderContext,
    item_rect: Rect,
    view:      &SidebarTabView<'_>,
    icon_size: f64,
    theme:     &dyn ButtonTheme,
    draw_icon: F,
)
where
    F: FnOnce(&mut dyn RenderContext, &IconId, Rect, &str),
{
    let icon_color = if view.active {
        ctx.draw_sidebar_active_item(
            item_rect.x, item_rect.y, item_rect.width, item_rect.height,
            theme.toolbar_accent(), theme.toolbar_item_bg_active(), 3.0,
        );
        theme.toolbar_item_text_active()
    } else {
        theme.toolbar_item_text()
    };

    let icon_rect = Rect::new(
        item_rect.center_x() - icon_size / 2.0,
        item_rect.center_y() - icon_size / 2.0,
        icon_size,
        icon_size,
    );
    draw_icon(ctx, view.icon, icon_rect, icon_color);
}

// ─── Section 20 — HorizontalTab Button ───────────────────────────────────────

/// Visual style for the active-state highlight of a horizontal tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HorizontalTabActiveStyle {
    /// Full rect fill (watchlist_modal / alert_settings pattern).
    FillRect,
    /// Bottom underline bar (primitive_settings `draw_active_rect` pattern).
    /// Underline height defaults to `2.0 px`.
    Underline,
}

/// Per-instance data for `draw_horizontal_tab_button`.
pub struct HorizontalTabView<'a> {
    pub label:   &'a str,
    pub active:  bool,
    pub hovered: bool,
}

/// Render a flat horizontal tab button: text centred, active/hover state as
/// either a full-rect fill or a bottom underline bar.
///
/// Ports section 20 from button-full.md (`watchlist_modal.rs`, `alert_settings.rs`,
/// `primitive_settings.rs` tab patterns).
///
/// # Arguments
/// - `item_rect`    — pre-computed pixel rect for this tab cell (caller handles layout).
/// - `active_style` — [`HorizontalTabActiveStyle::FillRect`] for watchlist/alert style;
///                    [`HorizontalTabActiveStyle::Underline`] for primitive_settings style.
/// - `font`         — font string (mlc default `"12px sans-serif"`).
/// - `theme`        — colour source.
///
/// # Active visual
/// - `FillRect`:  `toolbar_item_bg_active` fill over the full rect; text uses
///   `toolbar_item_text_active`.
/// - `Underline`: `toolbar_accent` fill over a 2 px strip at the bottom of the
///   rect; text uses `toolbar_item_text_active`.
///
/// # Hover visual (both styles)
/// `toolbar_item_bg_hover` fill; text uses `toolbar_item_text_hover`.
/// Hover is suppressed when `active` is `true`.
pub fn draw_horizontal_tab_button(
    ctx:          &mut dyn RenderContext,
    item_rect:    Rect,
    view:         &HorizontalTabView<'_>,
    active_style: HorizontalTabActiveStyle,
    font:         &str,
    theme:        &dyn ButtonTheme,
) {
    let text_color = if view.active {
        match active_style {
            HorizontalTabActiveStyle::FillRect => {
                ctx.set_fill_color(theme.toolbar_item_bg_active());
                ctx.fill_rect(item_rect.x, item_rect.y, item_rect.width, item_rect.height);
            }
            HorizontalTabActiveStyle::Underline => {
                const UNDERLINE_H: f64 = 2.0;
                ctx.set_fill_color(theme.toolbar_accent());
                ctx.fill_rect(
                    item_rect.x,
                    item_rect.bottom() - UNDERLINE_H,
                    item_rect.width,
                    UNDERLINE_H,
                );
            }
        }
        theme.toolbar_item_text_active()
    } else if view.hovered {
        ctx.set_fill_color(theme.toolbar_item_bg_hover());
        ctx.fill_rect(item_rect.x, item_rect.y, item_rect.width, item_rect.height);
        theme.toolbar_item_text_hover()
    } else {
        theme.toolbar_item_text()
    };

    ctx.set_fill_color(text_color);
    ctx.set_font(font);
    ctx.set_text_align(TextAlign::Center);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(view.label, item_rect.center_x(), item_rect.center_y());
}

// ─── Section 18 — Utility Button ─────────────────────────────────────────────

/// Per-instance data for `draw_utility_button`.
pub struct UtilityButtonView<'a> {
    pub text:    &'a str,
    pub hovered: bool,
    /// When `true` text is drawn in `toolbar_item_text`; when `false` in
    /// `toolbar_item_text_muted` (mirrors "+ Create New Profile" idle state).
    pub prominent: bool,
}

/// Render a utility action button (Run Setup Wizard, Create New Profile, etc.).
///
/// Ports section 18 from button-full.md (`profile_manager.rs` utility/create pattern).
///
/// Background: `button_utility_bg` / `button_utility_bg_hover`.
/// Text: `toolbar_item_text` (hover or `prominent=true`) else `toolbar_item_text_muted`.
/// No border stroke.
///
/// # Arguments
/// - `font` — e.g. `"11px sans-serif"` (wizard) or `"bold 13px sans-serif"` (Create).
pub fn draw_utility_button(
    ctx:   &mut dyn RenderContext,
    rect:  Rect,
    view:  &UtilityButtonView<'_>,
    font:  &str,
    theme: &dyn ButtonTheme,
) -> ButtonResult {
    let bg = if view.hovered {
        theme.button_utility_bg_hover()
    } else {
        theme.button_utility_bg()
    };
    ctx.set_fill_color(bg);
    ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, 4.0);

    let text_color = if view.hovered || view.prominent {
        theme.toolbar_item_text()
    } else {
        theme.toolbar_item_text()  // mlc item_text_muted — falls back to item_text in DefaultButtonTheme
    };
    ctx.set_fill_color(text_color);
    ctx.set_font(font);
    ctx.set_text_align(TextAlign::Center);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(view.text, rect.center_x(), rect.center_y());

    ButtonResult {
        clicked: false,
        hovered: view.hovered,
        pressed: false,
    }
}

// =============================================================================
// DropdownMenuRow (section 38)
// =============================================================================

/// Per-instance data for `draw_dropdown_menu_row`.
///
/// Represents a single item rendered inside an open dropdown menu.
pub struct DropdownMenuRowView<'a> {
    /// Row label text.
    pub label: &'a str,
    /// Optional leading icon (drawn before the label).
    pub icon: Option<&'a IconId>,
    /// `true` when this item is the currently selected value.
    pub selected: bool,
    /// `true` when the pointer is over this row.
    pub hovered: bool,
    /// `true` draws a separator line below this row.
    pub separator_after: bool,
}

/// Render one row inside an open dropdown menu.
///
/// Ports section 38 from button-full.md
/// (`alert_settings.rs` / `chart_settings.rs` template list).
///
/// # Visual
/// - Hover: `dropdown_menu_row_bg_hover` fill (with 1 px x-inset).
/// - Selected: `dropdown_menu_row_bg_selected` fill (overrides hover).
/// - Text: `dropdown_menu_row_text` normal, `dropdown_menu_row_text_selected` when selected.
/// - Optional icon drawn 16×16 left of label.
/// - Optional separator line (`dropdown_menu_separator`) at the bottom of the row.
///
/// # Arguments
/// - `rect`    — full pixel rect for this row (caller provides layout).
/// - `icon_size` — side length for the optional icon in pixels.
pub fn draw_dropdown_menu_row<F>(
    ctx:       &mut dyn RenderContext,
    rect:      Rect,
    view:      &DropdownMenuRowView<'_>,
    font:      &str,
    icon_size: f64,
    style:     &dyn DropdownMenuRowStyle,
    theme:     &dyn ButtonTheme,
    draw_icon: F,
)
where
    F: FnOnce(&mut dyn RenderContext, &IconId, Rect, &str),
{
    let ix = style.highlight_inset_x();
    let r  = style.radius();

    // ── Row highlight ──────────────────────────────────────────────────────────
    let highlight_bg = if view.selected {
        Some(theme.dropdown_menu_row_bg_selected())
    } else if view.hovered {
        Some(theme.dropdown_menu_row_bg_hover())
    } else {
        None
    };

    if let Some(bg) = highlight_bg {
        ctx.set_fill_color(bg);
        if r > 0.0 {
            ctx.fill_rounded_rect(
                rect.x + ix, rect.y,
                rect.width - ix * 2.0, rect.height,
                r,
            );
        } else {
            ctx.fill_rect(
                rect.x + ix, rect.y,
                rect.width - ix * 2.0, rect.height,
            );
        }
    }

    // ── Optional leading icon ─────────────────────────────────────────────────
    let text_color = if view.selected {
        theme.dropdown_menu_row_text_selected()
    } else {
        theme.dropdown_menu_row_text()
    };

    let mut text_x = rect.x + style.text_padding_x();

    if let Some(icon) = view.icon {
        let icon_rect = Rect::new(
            text_x,
            rect.center_y() - icon_size / 2.0,
            icon_size,
            icon_size,
        );
        draw_icon(ctx, icon, icon_rect, text_color);
        text_x += icon_size + 6.0;
    }

    // ── Label text ────────────────────────────────────────────────────────────
    ctx.set_font(font);
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.set_fill_color(text_color);
    ctx.fill_text(view.label, text_x, rect.center_y());

    // ── Separator ─────────────────────────────────────────────────────────────
    if view.separator_after {
        let sep_y = rect.y + rect.height - style.separator_height();
        ctx.set_stroke_color(theme.dropdown_menu_separator());
        ctx.set_stroke_width(style.separator_height());
        ctx.set_line_dash(&[]);
        ctx.begin_path();
        ctx.move_to(rect.x,                 sep_y);
        ctx.line_to(rect.x + rect.width,    sep_y);
        ctx.stroke();
    }
}

// draw_close_button extracted to atomic::close_button::draw_close_button.
// draw_scroll_chevron_button extracted to atomic::scroll_chevron::draw_scroll_chevron.
// Backward-compat re-exports live in button/mod.rs.
