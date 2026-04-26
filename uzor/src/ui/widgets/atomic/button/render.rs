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
use super::style::{
    CheckboxStyle, CloseButtonStyle, ColorSwatchStyle, DropdownFieldStyle, DropdownMenuRowStyle,
    FillToggleStyle, RadioPairStyle, RadioStyle, ScrollChevronStyle, SelectorButtonStyle,
    SplitDropdownStyle, ToggleSwitchStyle,
};
use super::types::ChevronDirection;

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

/// Per-instance data for `draw_toolbar_clock`.
pub struct ClockView<'a> {
    pub time:    &'a str,
    pub hovered: bool,
}

/// Per-instance data for `draw_toolbar_label`.
pub struct LabelView<'a> {
    pub text: &'a str,
}

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

// =============================================================================
// Toolbar variant #9 — Clock (hover-only)
// =============================================================================

/// Render a clock item: monospace time string, right-aligned.
///
/// Ports `ToolbarItem::Clock` from `toolbar_core.rs`.
///
/// Hover: draws `item_bg_hover` rounded rect with a 2 px vertical inset
/// (`y+2`, `height-4`).  No active state.
///
/// Fixed width used by the caller to size `item_rect`: `140 px`.
pub fn draw_toolbar_clock(
    ctx:       &mut dyn RenderContext,
    item_rect: Rect,
    view:      &ClockView<'_>,
    theme:     &dyn ButtonTheme,
) {
    if view.hovered {
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
    ctx.fill_text(view.time, item_rect.right() - 8.0, item_rect.center_y());
}

// =============================================================================
// Toolbar variant #10 — Label (non-interactive text)
// =============================================================================

/// Render a non-interactive toolbar label.
///
/// Ports `ToolbarItem::Label` from `toolbar_core.rs`.
///
/// Width = `ctx.measure_text(text) + 8.0` — the caller must pre-measure if
/// it needs the width for layout; this function only draws.
///
/// A hit-rect is still registered by the caller (mlc does so for potential
/// click use).
pub fn draw_toolbar_label(
    ctx:       &mut dyn RenderContext,
    item_rect: Rect,
    view:      &LabelView<'_>,
    theme:     &dyn ButtonTheme,
) {
    ctx.set_font("13px sans-serif");
    ctx.set_fill_color(theme.toolbar_label_text());
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(view.text, item_rect.x + 4.0, item_rect.center_y());
}

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

// =============================================================================
// Checkbox variants (sections 21-24)
// =============================================================================

/// Per-instance data for checkbox render functions.
pub struct CheckboxView<'a> {
    /// Whether the checkbox is in its checked/enabled state.
    pub checked: bool,
    /// Optional label drawn to the right of the box (caller supplies font+color).
    /// When `Some`, the render fn fills the text; when `None` only the box is drawn.
    pub label: Option<&'a str>,
}

/// Render a standard checkbox box with checkmark (sections 21-23).
///
/// Covers all three mlc variants that share the same checkmark-path visual:
/// - Section 21 (`draw_checkbox` in `chart_settings.rs`) — pass `StandardCheckboxStyle`
/// - Section 22 (`indicator_settings.rs` inline) — pass `VisibilityCheckboxStyle`
/// - Section 23 (`primitive_settings.rs` render_level_item) — pass `LevelVisibilityCheckboxStyle`
///
/// The style controls size, border radius, and checkmark anchor inset.
/// The theme controls fill/border/checkmark colors.
///
/// # Layout
/// `rect` is the **box** rect (top-left origin of the square).
/// Label (if any) is drawn at `rect.x + rect.width + style.label_gap()`, vertically centred.
/// Font for the label must be set by the caller **before** calling this fn, or pass `font`
/// explicitly — the function sets `font` only when `view.label` is `Some`.
///
/// # Arguments
/// - `rect`  — box rect; `rect.width == rect.height == style.size()` by convention.
/// - `font`  — font string for the label (e.g. `"13px sans-serif"`); ignored if `view.label == None`.
pub fn draw_checkbox_standard(
    ctx:   &mut dyn RenderContext,
    rect:  Rect,
    view:  &CheckboxView<'_>,
    font:  &str,
    style: &dyn CheckboxStyle,
    theme: &dyn ButtonTheme,
) {
    let r = style.radius();

    // Background fill
    let bg = if view.checked {
        theme.checkbox_bg_checked()
    } else {
        theme.checkbox_bg_unchecked()
    };
    ctx.set_fill_color(bg);
    ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);

    // Border stroke
    ctx.set_stroke_color(theme.checkbox_border());
    ctx.set_stroke_width(style.border_width());
    ctx.set_line_dash(&[]);
    ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);

    // Checkmark path
    if view.checked {
        let inset = style.checkmark_inset();
        ctx.set_stroke_color(theme.checkbox_checkmark());
        ctx.set_stroke_width(style.checkmark_width());
        ctx.set_line_dash(&[]);
        ctx.begin_path();
        ctx.move_to(rect.x + 3.0,              rect.y + rect.height / 2.0);
        ctx.line_to(rect.x + 6.0,              rect.y + rect.height - inset);
        ctx.line_to(rect.x + rect.width - 3.0, rect.y + inset);
        ctx.stroke();
    }

    // Label
    if let Some(label) = view.label {
        ctx.set_font(font);
        ctx.set_fill_color(theme.toolbar_item_text());
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Middle);
        ctx.fill_text(label, rect.x + rect.width + style.label_gap(), rect.y + rect.height / 2.0);
    }
}

/// Render a notification-style checkbox (section 24 — `draw_toggle` in `alert_settings.rs`).
///
/// Visual difference from standard: no filled outer background, stroke-only outer square,
/// enabled state shows a filled inner rect (`size-6` × `size-6` inset by 3px, radius 1.0)
/// instead of a checkmark path.
///
/// # Arguments
/// - `rect`  — box rect; width and height should equal `style.size()`.
/// - `font`  — label font string; ignored when `view.label == None`.
pub fn draw_checkbox_notification(
    ctx:   &mut dyn RenderContext,
    rect:  Rect,
    view:  &CheckboxView<'_>,
    font:  &str,
    style: &dyn CheckboxStyle,
    theme: &dyn ButtonTheme,
) {
    let r = style.radius();

    // Outer square — stroke only (no fill)
    ctx.set_stroke_color(theme.checkbox_border());
    ctx.set_stroke_width(style.border_width());
    ctx.set_line_dash(&[]);
    ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);

    // Inner filled rect when enabled
    if view.checked {
        let inset  = 3.0_f64;
        let inner_x = rect.x + inset;
        let inner_y = rect.y + inset;
        let inner_w = rect.width  - inset * 2.0;
        let inner_h = rect.height - inset * 2.0;
        ctx.set_fill_color(theme.checkbox_notification_inner());
        ctx.fill_rounded_rect(inner_x, inner_y, inner_w, inner_h, 1.0);
    }

    // Label
    if let Some(label) = view.label {
        ctx.set_font(font);
        ctx.set_fill_color(theme.toolbar_item_text());
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Middle);
        ctx.fill_text(label, rect.x + rect.width + style.label_gap(), rect.y + rect.height / 2.0);
    }
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
// Color swatch variants (sections 27-30) and fill toggle (section 31)
// =============================================================================

/// Per-instance data for `draw_color_swatch`.
///
/// Covers variants 27, 28, 29 (with `show_transparency = true`), and 30.
pub struct ColorSwatchView<'a> {
    /// RGBA color of the swatch.  `[r, g, b, a]` each 0-255.
    pub color: [u8; 4],
    /// Whether the pointer is currently over this swatch.
    pub hovered: bool,
    /// Whether the color picker for this swatch is open (selected state).
    pub selected: bool,
    /// When `true` draws a two-tile checkerboard behind the color fill so
    /// that semi-transparent colors are visually legible.
    /// Set this for the appearance-tab variant (section 29).
    pub show_transparency: bool,
    /// Optional CSS-color override for the border.  `None` uses theme default.
    /// Pass `Some("rgba(0,0,0,0.4)")` to match the "muted_color" used in
    /// section 29 without exposing a new theme slot.
    pub border_color_override: Option<&'a str>,
}

/// Per-instance data for `draw_fill_toggle`.
pub struct FillToggleView {
    /// `true` → fill is enabled; shows color fill + active border.
    /// `false` → fill is disabled; shows toolbar bg + diagonal strikethrough.
    pub filled: bool,
    /// RGBA fill color displayed when `filled = true`.  `[r, g, b, a]` 0-255.
    pub color: [u8; 4],
    /// When `true` applies a semi-transparent dark overlay (disabled state).
    pub disabled: bool,
}

// =============================================================================
// Toggle switch variants (sections 25-26)
// =============================================================================

/// Per-instance data for `draw_toggle_switch`.
///
/// Covers both mlc variants (section 25 — Bool param, section 26 — signals
/// enable/disable). The visual difference between the two is controlled
/// entirely by passing the matching `ToggleSwitchStyle` implementation.
pub struct ToggleSwitchView<'a> {
    /// `true` → ON state (thumb right, accent track fill).
    /// `false` → OFF state (thumb left, grey track fill).
    pub toggled: bool,
    /// Optional label drawn to the right of the track.
    pub label: Option<&'a str>,
    /// When `true` renders with `toggle_disabled_overlay` wash.
    pub disabled: bool,
}

/// Render an iOS-style toggle switch: pill track + circular thumb.
///
/// Ports sections 25 and 26 from button-full.md (`indicator_settings.rs`).
///
/// # Visual
/// - Track: `fill_rounded_rect` with `border-radius = track_height / 2` (full pill).
///   ON → `toggle_track_on` (accent), OFF → `toggle_track_off` (grey).
/// - Thumb: filled circle (`fill_rounded_rect` with `radius = thumb_radius`).
///   White (`toggle_thumb_on` / `toggle_thumb_off`).
///   ON  → thumb center at `track_x + track_width  - thumb_radius - thumb_padding`.
///   OFF → thumb center at `track_x + thumb_radius + thumb_padding`.
/// - Disabled overlay: semi-transparent fill over the full track.
///
/// # Arguments
/// - `rect`  — top-left origin of the **track** (caller handles row layout).
///             `rect.width` and `rect.height` are ignored; dimensions come from
///             `style.track_width()` / `style.track_height()`.
/// - `font`  — label font string (e.g. `"13px sans-serif"`); ignored when
///             `view.label == None`.
pub fn draw_toggle_switch(
    ctx:   &mut dyn RenderContext,
    rect:  Rect,
    view:  &ToggleSwitchView<'_>,
    font:  &str,
    style: &dyn ToggleSwitchStyle,
    theme: &dyn ButtonTheme,
) {
    let tw = style.track_width();
    let th = style.track_height();
    let tr = th / 2.0;
    let kr = style.thumb_radius();
    let kp = style.thumb_padding();

    // ── Track ──────────────────────────────────────────────────────────────────
    let track_color = if view.toggled {
        theme.toggle_track_on()
    } else {
        theme.toggle_track_off()
    };
    ctx.set_fill_color(track_color);
    ctx.fill_rounded_rect(rect.x, rect.y, tw, th, tr);

    // ── Thumb ──────────────────────────────────────────────────────────────────
    let thumb_color = if view.toggled {
        theme.toggle_thumb_on()
    } else {
        theme.toggle_thumb_off()
    };
    let thumb_center_x = if view.toggled {
        rect.x + tw - kr - kp
    } else {
        rect.x + kr + kp
    };
    let thumb_center_y = rect.y + th / 2.0;

    ctx.set_fill_color(thumb_color);
    ctx.fill_rounded_rect(
        thumb_center_x - kr,
        thumb_center_y - kr,
        kr * 2.0,
        kr * 2.0,
        kr,
    );

    // ── Disabled overlay ───────────────────────────────────────────────────────
    if view.disabled {
        ctx.set_fill_color(theme.toggle_disabled_overlay());
        ctx.fill_rounded_rect(rect.x, rect.y, tw, th, tr);
    }

    // ── Label ──────────────────────────────────────────────────────────────────
    if let Some(label) = view.label {
        let label_color = if view.disabled {
            theme.button_text_disabled()
        } else {
            theme.toolbar_item_text()
        };
        ctx.set_font(font);
        ctx.set_fill_color(label_color);
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Middle);
        ctx.fill_text(label, rect.x + tw + style.label_gap(), rect.y + th / 2.0);
    }
}

// =============================================================================
// Color swatch render functions (sections 27-30)
// =============================================================================

/// Convert a `[r, g, b, a]` byte tuple to a CSS `rgba(…)` string.
#[inline]
fn rgba_css(c: [u8; 4]) -> String {
    let alpha = c[3] as f64 / 255.0;
    format!("rgba({},{},{},{:.3})", c[0], c[1], c[2], alpha)
}

/// Render a color swatch button.
///
/// Covers sections 27-30 from button-full.md.
///
/// # Visual layers (bottom → top)
/// 1. **Hover expand** (optional): `color_swatch_hover_outline` filled rect expanded
///    by `style.hover_expand()` on each side.  Only drawn when `view.hovered` or
///    `view.selected` and `style.hover_expand() > 0.0`.
/// 2. **Checkerboard** (optional, `view.show_transparency = true`): two 9×9 tiles
///    alternating `transparency_checker_a` / `transparency_checker_b`.  Fills the
///    same `rect` as the swatch.
/// 3. **Color fill**: `rgba_css(view.color)` filled into `rect`.
/// 4. **Border stroke**: `color_swatch_border` idle, `color_swatch_selected_border`
///    when `view.selected`.  Width = `style.border_width()` idle /
///    `style.selected_border_width()` selected.  `border_color_override` overrides
///    the color for both states when set.
///
/// # Arguments
/// - `rect`  — pixel rect for the swatch square.  Width and height come from the
///   caller (e.g., `style.swatch_size()` square, or `16 × row_height - 8` for
///   primitive variant).  The function draws exactly into this rect.
/// - `style` — geometry: size hints, radius, border widths, hover expand.
/// - `theme` — color slots.
pub fn draw_color_swatch(
    ctx:   &mut dyn RenderContext,
    rect:  Rect,
    view:  &ColorSwatchView<'_>,
    style: &dyn ColorSwatchStyle,
    theme: &dyn ButtonTheme,
) {
    let r = style.radius();

    // ── 1. Hover expand rect ───────────────────────────────────────────────────
    let expand = style.hover_expand();
    if expand > 0.0 && (view.hovered || view.selected) {
        ctx.set_fill_color(theme.color_swatch_hover_outline());
        ctx.fill_rect(
            rect.x      - expand,
            rect.y      - expand,
            rect.width  + expand * 2.0,
            rect.height + expand * 2.0,
        );
    }

    // ── 2. Checkerboard background ─────────────────────────────────────────────
    if view.show_transparency {
        let tile = style.checker_tile_size();
        // White base
        ctx.set_fill_color(theme.transparency_checker_a());
        ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);
        // Two dark tiles at top-left and bottom-right
        ctx.set_fill_color(theme.transparency_checker_b());
        ctx.fill_rect(rect.x,        rect.y,        tile, tile);
        ctx.fill_rect(rect.x + tile, rect.y + tile, tile, tile);
    }

    // ── 3. Color fill ──────────────────────────────────────────────────────────
    let color_css = rgba_css(view.color);
    ctx.set_fill_color(&color_css);
    if r > 0.0 {
        ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);
    } else {
        ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);
    }

    // ── 4. Border stroke ───────────────────────────────────────────────────────
    let (border_color, border_w) = if view.selected {
        let c = view.border_color_override
            .unwrap_or_else(|| theme.color_swatch_selected_border());
        (c, style.selected_border_width())
    } else {
        let c = view.border_color_override
            .unwrap_or_else(|| theme.color_swatch_border());
        (c, style.border_width())
    };
    ctx.set_stroke_color(border_color);
    ctx.set_stroke_width(border_w);
    ctx.set_line_dash(&[]);
    if r > 0.0 {
        ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);
    } else {
        ctx.stroke_rect(rect.x, rect.y, rect.width, rect.height);
    }
}

// =============================================================================
// Fill toggle render function (section 31)
// =============================================================================

/// Render a fill-toggle button.
///
/// Ports section 31 from button-full.md (`primitive_settings.rs` level fill button).
///
/// # Visual
/// - Base: `toolbar_background` rounded rect (always drawn as background).
/// - When `filled = true`: color fill (with alpha from `view.color[3]`) + active border
///   (`fill_toggle_active_border`).
/// - When `filled = false`: idle border (`color_swatch_border`) + diagonal strikethrough
///   line (`fill_toggle_off_pattern_color`).
/// - When `disabled = true`: semi-transparent overlay (`toggle_disabled_overlay`).
///
/// # Arguments
/// - `rect`  — pixel rect for the toggle square (caller handles layout).
/// - `style` — geometry: radius, border width.
/// - `theme` — color slots.
pub fn draw_fill_toggle(
    ctx:   &mut dyn RenderContext,
    rect:  Rect,
    view:  &FillToggleView,
    style: &dyn FillToggleStyle,
    theme: &dyn ButtonTheme,
) {
    let r = style.radius();

    // ── Base background ────────────────────────────────────────────────────────
    ctx.set_fill_color(theme.toolbar_background());
    ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);

    // ── Fill when enabled ──────────────────────────────────────────────────────
    if view.filled {
        let color_css = rgba_css(view.color);
        ctx.set_fill_color(&color_css);
        ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);
    }

    // ── Border ─────────────────────────────────────────────────────────────────
    let border_color = if view.filled {
        theme.fill_toggle_active_border()
    } else {
        theme.color_swatch_border()
    };
    ctx.set_stroke_color(border_color);
    ctx.set_stroke_width(style.border_width());
    ctx.set_line_dash(&[]);
    ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);

    // ── Diagonal strikethrough when fill disabled ──────────────────────────────
    if !view.filled {
        ctx.set_stroke_color(theme.fill_toggle_off_pattern_color());
        ctx.set_stroke_width(1.0);
        ctx.set_line_dash(&[]);
        ctx.begin_path();
        ctx.move_to(rect.x + 2.0,            rect.y + rect.height - 2.0);
        ctx.line_to(rect.x + rect.width - 2.0, rect.y + 2.0);
        ctx.stroke();
    }

    // ── Disabled overlay ───────────────────────────────────────────────────────
    if view.disabled {
        ctx.set_fill_color(theme.toggle_disabled_overlay());
        ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);
    }
}

// =============================================================================
// SplitDropdown (section 32)
// =============================================================================

/// Per-instance data for `draw_split_dropdown`.
///
/// Represents a settings dropdown with two clickable zones:
/// - Left (text) area: cycles through values when clicked if `cycle_on_click`.
/// - Right (chevron) area: opens the dropdown menu.
pub struct SplitDropdownView<'a> {
    /// Current value label displayed in the text zone.
    pub current_label: &'a str,
    /// `true` if clicking the text zone cycles the value.
    pub cycle_on_click: bool,
    /// Which zone (if any) the pointer is currently over.
    pub hovered_zone: SplitButtonHoverZone,
    /// `true` when the dropdown menu is currently open.
    pub open: bool,
}

/// Render a split dropdown trigger.
///
/// Ports section 32 from button-full.md (`chart_settings.rs draw_split_dropdown`).
///
/// # Visual
/// ```text
/// ┌──────────────────────┬────┐
/// │ current_label        │ ▼  │
/// └──────────────────────┴────┘
///  ← text_width ────────→← chevron_width →
/// ```
/// - Background: `toolbar_background` rounded rect with `dropdown_field_border` stroke.
/// - Vertical separator line at `x + text_width`.
/// - Filled triangle chevron centered in the chevron zone.
/// - Hover state: `dropdown_field_bg_hover` on the hovered zone only.
///
/// # Returns
/// `(text_rect, chevron_rect)` — the two independent hit-test rects.
/// Caller registers each as a separate atomic widget.
pub fn draw_split_dropdown(
    ctx:   &mut dyn RenderContext,
    rect:  Rect,
    view:  &SplitDropdownView<'_>,
    font:  &str,
    style: &dyn SplitDropdownStyle,
    theme: &dyn ButtonTheme,
) -> (Rect, Rect) {
    let cw = style.chevron_width();
    let text_w = rect.width - cw;

    let text_rect  = Rect::new(rect.x,           rect.y, text_w, rect.height);
    let chev_rect  = Rect::new(rect.x + text_w,  rect.y, cw,     rect.height);

    let r = style.radius();
    let bw = style.border_width();

    // ── Base background ────────────────────────────────────────────────────────
    ctx.set_fill_color(theme.dropdown_field_bg());
    ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);

    // ── Hover highlight on the active zone ────────────────────────────────────
    match view.hovered_zone {
        SplitButtonHoverZone::Main => {
            ctx.set_fill_color(theme.dropdown_field_bg_hover());
            // Left zone covers only the text area; rounded only on the left side.
            // Approximate with fill_rounded_rect clipped to text_w.
            ctx.save();
            ctx.clip_rect(rect.x, rect.y, text_w, rect.height);
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);
            ctx.restore();
        }
        SplitButtonHoverZone::Chevron => {
            ctx.set_fill_color(theme.dropdown_field_bg_hover());
            ctx.save();
            ctx.clip_rect(rect.x + text_w, rect.y, cw, rect.height);
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);
            ctx.restore();
        }
        SplitButtonHoverZone::None => {}
    }

    // ── Border ─────────────────────────────────────────────────────────────────
    ctx.set_stroke_color(theme.dropdown_field_border());
    ctx.set_stroke_width(bw);
    ctx.set_line_dash(&[]);
    ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);

    // ── Label ─────────────────────────────────────────────────────────────────
    ctx.set_font(font);
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.set_fill_color(theme.dropdown_field_text());
    ctx.fill_text(view.current_label, rect.x + style.text_padding_x(), rect.center_y());

    // ── Vertical separator ────────────────────────────────────────────────────
    ctx.set_stroke_color(theme.dropdown_field_border());
    ctx.set_stroke_width(bw);
    ctx.set_line_dash(&[]);
    ctx.begin_path();
    ctx.move_to(rect.x + text_w, rect.y);
    ctx.line_to(rect.x + text_w, rect.y + rect.height);
    ctx.stroke();

    // ── Filled triangle chevron ───────────────────────────────────────────────
    let arrow_cx = chev_rect.center_x();
    let arrow_cy = chev_rect.center_y();
    // Downward pointing triangle: half_base=3, height=6
    let hb = 3.0_f64;
    let ah = 3.0_f64;
    ctx.set_fill_color(theme.dropdown_chevron_color());
    ctx.begin_path();
    ctx.move_to(arrow_cx - hb, arrow_cy - ah);
    ctx.line_to(arrow_cx + hb, arrow_cy - ah);
    ctx.line_to(arrow_cx,      arrow_cy + ah);
    ctx.close_path();
    ctx.fill();

    (text_rect, chev_rect)
}

// =============================================================================
// DropdownField (section 33)
// =============================================================================

/// Per-instance data for `draw_dropdown_field`.
///
/// Single-zone trigger styled as a form input (like a text input box)
/// with a chevron icon on the right.
pub struct DropdownFieldView<'a> {
    /// Current value label displayed inside the field.
    pub current_label: &'a str,
    /// `true` when the dropdown menu is currently open (open state styling).
    pub open: bool,
    /// `true` when the pointer is over this field.
    pub hovered: bool,
}

/// Render a dropdown field trigger.
///
/// Ports section 33 from button-full.md (`alert_settings.rs draw_dropdown_field`).
///
/// # Visual
/// ```text
/// ┌─────────────────────────╮
/// │ current_label      ↓   │
/// └─────────────────────────╯
/// ```
/// - Background: `dropdown_field_bg` idle, `dropdown_field_bg_hover` on hover/open.
/// - Border: `dropdown_field_border`.
/// - Chevron SVG icon (12×12) pinned to the right, drawn as a filled triangle.
///
/// # Returns
/// The full `rect` as the single hit-test rect (no split zones).
pub fn draw_dropdown_field(
    ctx:   &mut dyn RenderContext,
    rect:  Rect,
    view:  &DropdownFieldView<'_>,
    font:  &str,
    style: &dyn DropdownFieldStyle,
    theme: &dyn ButtonTheme,
) -> Rect {
    let r   = style.radius();
    let bw  = style.border_width();
    let cs  = style.chevron_size();
    let cmr = style.chevron_margin_right();

    // ── Background ─────────────────────────────────────────────────────────────
    let bg = if view.hovered || view.open {
        theme.dropdown_field_bg_hover()
    } else {
        theme.dropdown_field_bg()
    };
    ctx.set_fill_color(bg);
    ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);

    // ── Border ─────────────────────────────────────────────────────────────────
    ctx.set_stroke_color(theme.dropdown_field_border());
    ctx.set_stroke_width(bw);
    ctx.set_line_dash(&[]);
    ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);

    // ── Label ─────────────────────────────────────────────────────────────────
    ctx.set_font(font);
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.set_fill_color(theme.dropdown_field_text());
    ctx.fill_text(view.current_label, rect.x + style.text_padding_x(), rect.center_y());

    // ── Chevron (filled triangle, 12×12 bounding box) ────────────────────────
    let arrow_x = rect.x + rect.width - cs - cmr;
    let arrow_cy = rect.center_y();
    // Center the downward triangle in the cs×cs bounding box.
    let hb = cs / 4.0;          // half-base ≈ 3 px for cs=12
    let ah = cs / 4.0;          // height    ≈ 3 px for cs=12
    let acx = arrow_x + cs / 2.0;
    ctx.set_fill_color(theme.dropdown_chevron_color());
    ctx.begin_path();
    ctx.move_to(acx - hb, arrow_cy - ah);
    ctx.line_to(acx + hb, arrow_cy - ah);
    ctx.line_to(acx,      arrow_cy + ah);
    ctx.close_path();
    ctx.fill();

    rect
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

// =============================================================================
// Selector-style buttons (sections 34, 39, 40)
// =============================================================================

// ─── Section 34 — ShapeSelector Button ───────────────────────────────────────

/// Per-instance data for `draw_shape_selector_button`.
///
/// A small square toggle button that contains a rendered shape icon.
/// The caller draws the shape itself via the `draw_shape` closure.
pub struct ShapeSelectorView<'a> {
    /// `true` when this shape is the currently selected one.
    pub selected: bool,
    /// `true` when the pointer is hovering over this button.
    pub hovered: bool,
    /// Optional label rendered below the button.
    pub label: Option<&'a str>,
}

/// Render a shape selector button (section 34 — signals tab shape row).
///
/// Ports from `indicator_settings.rs` shape button loop.
///
/// # Visual
/// - Background: `toolbar_item_bg_active` when selected, `toolbar_item_bg_hover` on hover,
///   `toolbar_background` otherwise.
/// - Border: `selector_selected_border` (accent) when selected,
///   `selector_hover_border` on hover, `selector_idle_border` otherwise.
/// - Shape content: drawn by the `draw_shape` closure into an inset rect
///   (`3 px` inset on each side).
/// - Label: drawn below the button in `selector_label_text` at `label_font_size`.
///
/// # Arguments
/// - `rect`       — button bounds (typically `style.width() × style.height()` square).
/// - `draw_shape` — closure that renders the shape icon;
///                  signature: `(ctx, inner_rect, color)`.
pub fn draw_shape_selector_button<F>(
    ctx:        &mut dyn RenderContext,
    rect:       Rect,
    view:       &ShapeSelectorView<'_>,
    font:       &str,
    style:      &dyn SelectorButtonStyle,
    theme:      &dyn ButtonTheme,
    draw_shape: F,
)
where
    F: FnOnce(&mut dyn RenderContext, Rect, &str),
{
    let r = style.radius();

    // ── Background ─────────────────────────────────────────────────────────────
    let bg = if view.selected {
        theme.toolbar_item_bg_active()
    } else if view.hovered {
        theme.toolbar_item_bg_hover()
    } else {
        theme.toolbar_background()
    };
    ctx.set_fill_color(bg);
    ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);

    // ── Border ─────────────────────────────────────────────────────────────────
    let (border_color, border_w) = if view.selected {
        (theme.selector_selected_border(), style.selected_border_width())
    } else if view.hovered {
        (theme.selector_hover_border(), style.border_width())
    } else {
        (theme.selector_idle_border(), style.border_width())
    };
    ctx.set_stroke_color(border_color);
    ctx.set_stroke_width(border_w);
    ctx.set_line_dash(&[]);
    ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);

    // ── Shape content (inset by 3 px) ──────────────────────────────────────────
    let inset = 3.0_f64;
    let inner = Rect::new(
        rect.x + inset,
        rect.y + inset,
        (rect.width  - inset * 2.0).max(0.0),
        (rect.height - inset * 2.0).max(0.0),
    );
    let shape_color = if view.selected {
        theme.toolbar_item_text_active()
    } else {
        theme.toolbar_item_text()
    };
    draw_shape(ctx, inner, shape_color);

    // ── Label ─────────────────────────────────────────────────────────────────
    if let Some(label) = view.label {
        ctx.set_font(font);
        ctx.set_fill_color(theme.selector_label_text());
        ctx.set_text_align(TextAlign::Center);
        ctx.set_text_baseline(TextBaseline::Top);
        ctx.fill_text(
            label,
            rect.center_x(),
            rect.y + rect.height + style.label_gap(),
        );
    }
}

// ─── Section 39 — ThemePreset Button ─────────────────────────────────────────

/// Per-instance data for `draw_theme_preset_button`.
///
/// An appearance-tab button that shows a color swatch preview on the left
/// and a theme name on the right.  Ports section 39 from button-full.md.
pub struct ThemePresetView<'a> {
    /// Theme name label drawn to the right of the swatch.
    pub label: &'a str,
    /// Preview color displayed as an 18×18 square swatch.
    /// CSS color string (e.g. `"#1e222d"` or `"rgba(30,34,45,1)"`).
    pub preview_color: &'a str,
    /// Muted border color drawn around the swatch square.
    /// Matches mlc `muted_color` (e.g. `"rgba(0,0,0,0.3)"`).
    pub swatch_border_color: &'a str,
    /// `true` when this is the currently active theme preset.
    pub selected: bool,
    /// `true` when the pointer is hovering over this button.
    pub hovered: bool,
}

/// Render a theme preset selector button (section 39 — appearance tab).
///
/// Ports from `chart_settings.rs` theme button pattern.
///
/// # Visual
/// - Background: `toolbar_item_bg_active` when selected, `toolbar_item_bg_hover` on hover,
///   `toolbar_background` otherwise.
/// - Border outline: `selector_selected_border` when selected,
///   `selector_hover_border` on hover, `selector_idle_border` otherwise.
/// - Swatch square (18×18) at `(rect.x + 6, rect.center_y - 9)` with
///   `view.swatch_border_color` stroke.
/// - Theme name text at `rect.x + 30`, vertically centered.
pub fn draw_theme_preset_button(
    ctx:   &mut dyn RenderContext,
    rect:  Rect,
    view:  &ThemePresetView<'_>,
    font:  &str,
    style: &dyn SelectorButtonStyle,
    theme: &dyn ButtonTheme,
) {
    let r = style.radius();

    // ── Background ─────────────────────────────────────────────────────────────
    let bg = if view.selected {
        theme.toolbar_item_bg_active()
    } else if view.hovered {
        theme.toolbar_item_bg_hover()
    } else {
        theme.toolbar_background()
    };
    ctx.set_fill_color(bg);
    ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);

    // ── Selection / hover border ───────────────────────────────────────────────
    let (border_color, border_w) = if view.selected {
        (theme.selector_selected_border(), style.selected_border_width())
    } else if view.hovered {
        (theme.selector_hover_border(), style.border_width())
    } else {
        (theme.selector_idle_border(), style.border_width())
    };
    ctx.set_stroke_color(border_color);
    ctx.set_stroke_width(border_w);
    ctx.set_line_dash(&[]);
    ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);

    // ── Preview color swatch (18×18) ───────────────────────────────────────────
    const SWATCH_SIZE: f64 = 18.0;
    let swatch_x = rect.x + 6.0;
    let swatch_y = rect.center_y() - SWATCH_SIZE / 2.0;

    ctx.set_fill_color(view.preview_color);
    ctx.fill_rect(swatch_x, swatch_y, SWATCH_SIZE, SWATCH_SIZE);

    ctx.set_stroke_color(view.swatch_border_color);
    ctx.set_stroke_width(1.0);
    ctx.set_line_dash(&[]);
    ctx.stroke_rect(swatch_x, swatch_y, SWATCH_SIZE, SWATCH_SIZE);

    // ── Theme name text ────────────────────────────────────────────────────────
    let text_color = if view.selected {
        theme.toolbar_item_text_active()
    } else {
        theme.selector_label_text()
    };
    ctx.set_font(font);
    ctx.set_fill_color(text_color);
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(view.label, rect.x + 30.0, rect.center_y());
}

// ─── Section 40 — UIStyle Button ─────────────────────────────────────────────

/// Per-instance data for `draw_ui_style_button`.
///
/// A text-only selector button for UI style choices (e.g. "Dark", "Light",
/// "High contrast").  Ports section 40 from button-full.md.
pub struct UIStyleView<'a> {
    /// Label text displayed inside the button.
    pub label: &'a str,
    /// `true` when this is the currently active UI style.
    pub selected: bool,
    /// `true` when the pointer is hovering over this button.
    pub hovered: bool,
}

/// Render a UI style selector button (section 40 — appearance tab).
///
/// Ports from `chart_settings.rs` `draw_button` text-only active pattern.
///
/// # Visual
/// - Background: `toolbar_item_bg_active` when selected, `toolbar_item_bg_hover` on hover,
///   `toolbar_background` otherwise.
/// - Border: `selector_selected_border` when selected,
///   `selector_hover_border` on hover, `selector_idle_border` otherwise.
/// - Label text centered inside the button rect.
pub fn draw_ui_style_button(
    ctx:   &mut dyn RenderContext,
    rect:  Rect,
    view:  &UIStyleView<'_>,
    font:  &str,
    style: &dyn SelectorButtonStyle,
    theme: &dyn ButtonTheme,
) {
    let r = style.radius();

    // ── Background ─────────────────────────────────────────────────────────────
    let bg = if view.selected {
        theme.toolbar_item_bg_active()
    } else if view.hovered {
        theme.toolbar_item_bg_hover()
    } else {
        theme.toolbar_background()
    };
    ctx.set_fill_color(bg);
    ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);

    // ── Border ─────────────────────────────────────────────────────────────────
    let (border_color, border_w) = if view.selected {
        (theme.selector_selected_border(), style.selected_border_width())
    } else if view.hovered {
        (theme.selector_hover_border(), style.border_width())
    } else {
        (theme.selector_idle_border(), style.border_width())
    };
    ctx.set_stroke_color(border_color);
    ctx.set_stroke_width(border_w);
    ctx.set_line_dash(&[]);
    ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);

    // ── Label text ─────────────────────────────────────────────────────────────
    let text_color = if view.selected {
        theme.toolbar_item_text_active()
    } else {
        theme.selector_label_text()
    };
    ctx.set_font(font);
    ctx.set_fill_color(text_color);
    ctx.set_text_align(TextAlign::Center);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(view.label, rect.center_x(), rect.center_y());
}

// =============================================================================
// Radio variants (sections 35-37)
// =============================================================================

// ─── Section 35 — RadioGroup ─────────────────────────────────────────────────

/// One option in a `RadioGroupView`.
pub struct RadioOption<'a> {
    /// Primary label (13 px).
    pub label: &'a str,
    /// Optional secondary description (11 px, muted).  Pass `""` to omit.
    pub description: &'a str,
    /// `true` when the pointer is hovering over this row.
    pub hovered: bool,
}

/// Per-frame data for `draw_radio_group`.
pub struct RadioGroupView<'a> {
    /// Ordered list of radio options.
    pub options: &'a [RadioOption<'a>],
    /// Index of the currently selected option.
    pub selected: usize,
}

/// Render a vertical stack of radio rows (section 35 — canonical group).
///
/// Ports `draw_radio_group` from `mlc/chart/src/ui/widgets/radio_group.rs`.
///
/// # Layout (per row, from top-left `(x, current_y)`)
/// - Hover background: full-width rounded rect when `hovered`.
/// - Outer ring circle at `(x + circle_offset_x, current_y + circle_offset_y)`.
/// - Inner fill dot (selected only), same center, `inner_radius`.
/// - Label at `(x + label_offset_x, current_y + label_offset_y)`.
/// - Description at `(x + label_offset_x, current_y + desc_offset_y)`.
///
/// # Arguments
/// - `x`, `y`   — top-left origin of the group.
/// - `width`    — row width (for hover rect and layout).
pub fn draw_radio_group(
    ctx:   &mut dyn RenderContext,
    x:     f64,
    y:     f64,
    width: f64,
    view:  &RadioGroupView<'_>,
    style: &dyn RadioStyle,
    theme: &dyn ButtonTheme,
) {
    use std::f64::consts::TAU;

    let mut current_y = y;

    for (i, opt) in view.options.iter().enumerate() {
        let is_selected = i == view.selected;

        // ── Hover background ──────────────────────────────────────────────────
        if opt.hovered {
            ctx.set_fill_color(theme.radio_row_bg_hover());
            ctx.fill_rounded_rect(x, current_y, width, style.row_height(), style.row_corner_radius());
        }

        // ── Outer ring ────────────────────────────────────────────────────────
        let circle_cx = x + style.circle_offset_x();
        let circle_cy = current_y + style.circle_offset_y();

        ctx.begin_path();
        ctx.arc(circle_cx, circle_cy, style.outer_radius(), 0.0, TAU);
        ctx.set_stroke_color(if is_selected {
            theme.radio_outer_border_selected()
        } else {
            theme.radio_outer_border()
        });
        ctx.set_stroke_width(style.ring_stroke_width());
        ctx.set_line_dash(&[]);
        ctx.stroke();

        // ── Inner dot (selected only) ─────────────────────────────────────────
        if is_selected {
            ctx.begin_path();
            ctx.arc(circle_cx, circle_cy, style.inner_radius(), 0.0, TAU);
            ctx.set_fill_color(theme.radio_inner_dot());
            ctx.fill();
        }

        // ── Label ─────────────────────────────────────────────────────────────
        ctx.set_fill_color(if is_selected {
            theme.radio_label_text_selected()
        } else {
            theme.radio_label_text()
        });
        ctx.set_font(&format!("{}px sans-serif", style.label_font_size()));
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Top);
        ctx.fill_text(opt.label, x + style.label_offset_x(), current_y + style.label_offset_y());

        // ── Description (if provided) ─────────────────────────────────────────
        if !opt.description.is_empty() {
            ctx.set_fill_color(theme.radio_description_text());
            ctx.set_font(&format!("{}px sans-serif", style.desc_font_size()));
            ctx.fill_text(opt.description, x + style.label_offset_x(), current_y + style.desc_offset_y());
        }

        current_y += style.row_height() + style.gap();
    }
}

// ─── Sections 36/37 — RadioPair / RadioDot ───────────────────────────────────

/// Per-frame data for `draw_radio_pair` (sections 36 and 37).
///
/// Renders two inline radio entries side-by-side.  Each entry is a circle
/// plus an optional label.  Section 36 (profile_manager): solid filled dot
/// when active, stroke-only when inactive, label to the right.
/// Section 37 (user_settings): outer ring + inner dot when active.
pub struct RadioPairView<'a> {
    /// Label for the left radio option.
    pub left_label: &'a str,
    /// Label for the right radio option.
    pub right_label: &'a str,
    /// `true` when the left option is selected; `false` selects the right.
    pub selected_left: bool,
}

/// Per-frame data for `draw_radio_dot` (section 37 — inline dot only).
///
/// Used when the parent row already renders the label; only the circle is
/// drawn here.
pub struct RadioDotView {
    /// `true` when this option is selected.
    pub selected: bool,
}

/// Render two inline radio buttons (sections 36 and 37).
///
/// Ports both `profile_manager.rs` (section 36) and `user_settings.rs`
/// (section 37) radio-pair patterns into one function.  Pass `use_ring_dot`
/// to choose the visual variant:
///
/// - `false` (section 36): active = solid filled circle in accent color;
///   inactive = stroke-only circle in muted text color.
/// - `true`  (section 37): active = outer ring (accent stroke) + inner dot
///   (accent fill); inactive = outer ring only (separator color stroke).
///
/// Each entry lays out left-to-right: circle then label, separated by
/// `style.label_gap()`.  The two entries are separated by `between_gap`.
///
/// # Arguments
/// - `x`, `cy` — left edge and vertical center of the pair row.
/// - `between_gap` — pixel gap between the right edge of the left label and
///   the circle of the right option.
/// - `use_ring_dot` — `false` = section 36 solid fill variant;
///   `true` = section 37 ring+dot variant.
#[allow(clippy::too_many_arguments)]
pub fn draw_radio_pair(
    ctx:          &mut dyn RenderContext,
    x:            f64,
    cy:           f64,
    between_gap:  f64,
    view:         &RadioPairView<'_>,
    style:        &dyn RadioPairStyle,
    theme:        &dyn ButtonTheme,
    use_ring_dot: bool,
) {
    use std::f64::consts::TAU;

    let r        = style.radio_radius();
    let sw       = style.ring_stroke_width();
    let font_str = format!("{}px sans-serif", style.label_font_size());

    // Helper: draw one radio entry.
    // Returns the x-coordinate just past the label end (for chaining).
    let draw_entry = |ctx: &mut dyn RenderContext, ex: f64, label: &str, is_active: bool| -> f64 {
        let ccx = ex + r;

        if use_ring_dot {
            // Section 37: outer ring + inner fill dot
            ctx.begin_path();
            ctx.arc(ccx, cy, r, 0.0, TAU);
            ctx.set_stroke_color(if is_active {
                theme.radio_outer_border_selected()
            } else {
                theme.radio_outer_border()
            });
            ctx.set_stroke_width(sw);
            ctx.set_line_dash(&[]);
            ctx.stroke();

            if is_active {
                ctx.begin_path();
                ctx.arc(ccx, cy, style.inner_dot_radius(), 0.0, TAU);
                ctx.set_fill_color(theme.radio_inner_dot());
                ctx.fill();
            }
        } else {
            // Section 36: solid fill when active, stroke-only when inactive
            ctx.begin_path();
            ctx.arc(ccx, cy, r, 0.0, TAU);
            if is_active {
                ctx.set_fill_color(theme.radio_inner_dot());
                ctx.fill();
            } else {
                ctx.set_stroke_color(theme.radio_label_text());
                ctx.set_stroke_width(sw);
                ctx.set_line_dash(&[]);
                ctx.stroke();
            }
        }

        // Label
        let label_x = ccx + r + style.label_gap();
        ctx.set_font(&font_str);
        ctx.set_fill_color(if is_active {
            theme.radio_label_text_selected()
        } else {
            theme.radio_label_text()
        });
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Middle);
        ctx.fill_text(label, label_x, cy);

        // Approximate end x — caller controls between_gap so exact measure is
        // not required here; we return a rough advance.
        label_x + (label.len() as f64 * style.label_font_size() * 0.6)
    };

    let left_end = draw_entry(ctx, x, view.left_label, view.selected_left);
    draw_entry(ctx, left_end + between_gap, view.right_label, !view.selected_left);
}

/// Render a single inline radio dot (section 37 — user_settings profile list).
///
/// Draws only the circle at `(cx, cy)`.  The parent row is responsible for
/// rendering the row background and label text.
///
/// Visual:
/// - Selected: outer ring (accent stroke, `ring_stroke_width`) + inner fill
///   dot (`inner_dot_radius`).
/// - Unselected: outer ring only (separator color stroke).
pub fn draw_radio_dot(
    ctx:   &mut dyn RenderContext,
    cx:    f64,
    cy:    f64,
    view:  &RadioDotView,
    style: &dyn RadioPairStyle,
    theme: &dyn ButtonTheme,
) {
    use std::f64::consts::TAU;

    let r  = style.radio_radius();
    let sw = style.ring_stroke_width();

    // Outer ring
    ctx.begin_path();
    ctx.arc(cx, cy, r, 0.0, TAU);
    ctx.set_stroke_color(if view.selected {
        theme.radio_outer_border_selected()
    } else {
        theme.radio_outer_border()
    });
    ctx.set_stroke_width(sw);
    ctx.set_line_dash(&[]);
    ctx.stroke();

    // Inner dot (selected only)
    if view.selected {
        ctx.begin_path();
        ctx.arc(cx, cy, style.inner_dot_radius(), 0.0, TAU);
        ctx.set_fill_color(theme.radio_inner_dot());
        ctx.fill();
    }
}

// =============================================================================
// Section 41 — CloseButton (X icon)
// =============================================================================

/// Per-instance data for `draw_close_button`.
pub struct CloseButtonView {
    /// Whether the pointer is over the button.
    pub hovered: bool,
}

/// Render a close button — square hit target with an X glyph (section 41).
///
/// Ported from mlc modal patterns:
/// - Idle: X drawn in `close_button_x_color` (muted) with no background.
/// - Hovered: X brightens to `close_button_x_color_hover`; a rounded-rect
///   background fill (`toolbar_item_bg_hover`) is drawn behind the X —
///   mirrors `profile_manager` hover fill.
///
/// # Arguments
/// - `rect`  — pixel bounds (caller sizes to `style.size() × style.size()`).
/// - `view`  — per-frame state.
/// - `style` — geometry (size, stroke, inset).
/// - `theme` — color slots.
///
/// Returns `ButtonResult` with `hovered` set from `view.hovered`.
pub fn draw_close_button(
    ctx:   &mut dyn RenderContext,
    rect:  Rect,
    view:  &CloseButtonView,
    style: &dyn CloseButtonStyle,
    theme: &dyn ButtonTheme,
) -> ButtonResult {
    // Hover background fill — mirrors profile_manager pattern
    if view.hovered {
        ctx.set_fill_color(theme.toolbar_item_bg_hover());
        ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, style.hover_bg_radius());
    }

    let color = if view.hovered {
        theme.close_button_x_color_hover()
    } else {
        theme.close_button_x_color()
    };

    let inset = style.x_inset();
    let x1 = rect.x + inset;
    let y1 = rect.y + inset;
    let x2 = rect.x + rect.width  - inset;
    let y2 = rect.y + rect.height - inset;

    ctx.set_stroke_color(color);
    ctx.set_stroke_width(style.x_stroke_width());
    ctx.set_line_dash(&[]);

    // First arm: top-left → bottom-right
    ctx.begin_path();
    ctx.move_to(x1, y1);
    ctx.line_to(x2, y2);
    ctx.stroke();

    // Second arm: top-right → bottom-left
    ctx.begin_path();
    ctx.move_to(x2, y1);
    ctx.line_to(x1, y2);
    ctx.stroke();

    ButtonResult {
        clicked: false,
        hovered: view.hovered,
        pressed: false,
    }
}

// =============================================================================
// Section 42 — ScrollChevron (toolbar overflow navigation)
// =============================================================================

/// Per-instance data for `draw_scroll_chevron_button`.
pub struct ScrollChevronView {
    /// Which way the chevron points.
    pub direction: ChevronDirection,
    /// Whether the pointer is over the button.
    pub hovered: bool,
    /// Whether there are no more items to scroll to in this direction.
    pub disabled: bool,
}

/// Render a scroll-chevron button — compact triangle for toolbar overflow
/// navigation (section 42).
///
/// Ported from `mlc/toolbar_core.rs` `draw_toolbar_with_icons`.
/// `chevron_size = 16.0`; left/right chevrons appear when items overflow.
///
/// Visual:
/// - Idle: chevron glyph in `scroll_chevron_color`.
/// - Hovered: chevron in `scroll_chevron_color_hover` + `toolbar_item_bg_hover`
///   rounded-rect fill.
/// - Disabled: chevron in `scroll_chevron_color_disabled`, no hover fill.
///
/// # Arguments
/// - `rect`  — pixel bounds (caller sizes to `style.size() × style.size()`).
/// - `view`  — per-frame state.
/// - `style` — geometry (size, stroke, inset).
/// - `theme` — color slots.
pub fn draw_scroll_chevron_button(
    ctx:   &mut dyn RenderContext,
    rect:  Rect,
    view:  &ScrollChevronView,
    style: &dyn ScrollChevronStyle,
    theme: &dyn ButtonTheme,
) -> ButtonResult {
    let color = if view.disabled {
        theme.scroll_chevron_color_disabled()
    } else if view.hovered {
        theme.scroll_chevron_color_hover()
    } else {
        theme.scroll_chevron_color()
    };

    // Hover background fill (not drawn when disabled)
    if view.hovered && !view.disabled {
        ctx.set_fill_color(theme.toolbar_item_bg_hover());
        ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, style.hover_bg_radius());
    }

    // Draw chevron as two stroked lines meeting at the tip
    let inset = style.chevron_inset();
    let cx = rect.center_x();
    let cy = rect.center_y();
    let half = (rect.width.min(rect.height) / 2.0 - inset).max(2.0);

    ctx.set_stroke_color(color);
    ctx.set_stroke_width(style.chevron_thickness());
    ctx.set_line_dash(&[]);
    ctx.begin_path();

    match view.direction {
        ChevronDirection::Left => {
            // tip on left, opening on right
            ctx.move_to(cx + half * 0.5, cy - half);
            ctx.line_to(cx - half * 0.5, cy);
            ctx.line_to(cx + half * 0.5, cy + half);
        }
        ChevronDirection::Right => {
            // tip on right, opening on left
            ctx.move_to(cx - half * 0.5, cy - half);
            ctx.line_to(cx + half * 0.5, cy);
            ctx.line_to(cx - half * 0.5, cy + half);
        }
        ChevronDirection::Up => {
            // tip on top, opening on bottom
            ctx.move_to(cx - half, cy + half * 0.5);
            ctx.line_to(cx, cy - half * 0.5);
            ctx.line_to(cx + half, cy + half * 0.5);
        }
        ChevronDirection::Down => {
            // tip on bottom, opening on top
            ctx.move_to(cx - half, cy - half * 0.5);
            ctx.line_to(cx, cy + half * 0.5);
            ctx.line_to(cx + half, cy - half * 0.5);
        }
    }

    ctx.stroke();

    ButtonResult {
        clicked: false,
        hovered: view.hovered && !view.disabled,
        pressed: false,
    }
}
