//! Sidebar render entry point and per-kind layout pipelines.
//!
//! # API convention
//!
//! - `register_input_coordinator_sidebar` — registers the composite + all child
//!   hit-rects with an `InputCoordinator`.  **No drawing.**
//! - `register_context_manager_sidebar`   — convenience wrapper: takes a
//!   `ContextManager`, registers, and draws in one call.
//!
//! # Draw order (non-Custom kinds)
//!
//! 1. Background + border
//! 2. Header strip (icon + title + action buttons)
//! 3. Header bottom divider
//! 4. Tab strip (WithTypeSelector only)
//! 5. (body drawn by caller after composite call)
//! 6. Scrollbar (if `view.effective_show_scrollbar()`)
//! 7. Resize edge handle (Right/Left/WithTypeSelector — sidebar variant stroke)

use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::render::{RenderContext, TextAlign, TextBaseline};
use crate::types::{Rect, WidgetId, CompositeId};
use crate::ui::widgets::atomic::scrollbar::render::{
    draw_scrollbar_standard, ScrollbarVisualState,
};

use super::settings::SidebarSettings;
use super::state::SidebarState;
use super::style::BackgroundFill;
use super::types::{SidebarRenderKind, SidebarView};

// ---------------------------------------------------------------------------
// Internal layout struct
// ---------------------------------------------------------------------------

struct SidebarLayout {
    /// Full frame rect (same as outer `rect`).
    frame: Rect,
    /// Header strip (40 px fixed).
    header: Rect,
    /// Tab strip (WithTypeSelector only; zero height otherwise).
    tab_strip: Rect,
    /// Body area available to the body closure.
    body: Rect,
    /// Scrollbar column (zero width if scrollbar disabled).
    scrollbar: Rect,
    /// Resize edge hit zone (zero width for Embedded / Custom).
    resize_zone: Rect,
    /// 1 px border line rect.
    border_line: Rect,
}

// ---------------------------------------------------------------------------
// Public API — register (InputCoordinator)
// ---------------------------------------------------------------------------

/// Register the sidebar composite and all its child hit-rects.
///
/// **No drawing happens here.**  Use when you need explicit z-order control.
///
/// Returns the [`CompositeId`] assigned to the sidebar composite.
pub fn register_input_coordinator_sidebar(
    coord:    &mut InputCoordinator,
    id:       impl Into<WidgetId>,
    rect:     Rect,
    state:    &SidebarState,
    view:     &SidebarView<'_>,
    settings: &SidebarSettings,
    kind:     &SidebarRenderKind,
    layer:    &LayerId,
) -> CompositeId {
    let sidebar_id = coord.register_composite(id, WidgetKind::Sidebar, rect, Sense::CLICK, layer);

    if let SidebarRenderKind::Custom(_) = kind {
        return sidebar_id;
    }

    let layout = compute_layout(rect, state, view, settings, kind);

    // --- Header action buttons ---
    for (i, action) in view.header.actions.iter().enumerate() {
        let btn_size  = 24.0_f64;
        let btn_gap   = 8.0_f64;
        let pad_right = 12.0_f64;
        let btn_x = layout.header.x + layout.header.width
            - pad_right
            - btn_size * (i + 1) as f64
            - btn_gap * i as f64;
        let btn_y = layout.header.y + (layout.header.height - btn_size) / 2.0;
        coord.register_child(
            &sidebar_id,
            format!("{}:action:{}", sidebar_id.0.0, action.id),
            WidgetKind::Button,
            Rect::new(btn_x, btn_y, btn_size, btn_size),
            Sense::CLICK | Sense::HOVER,
        );
    }

    // --- Tab strip (WithTypeSelector) ---
    if let SidebarRenderKind::WithTypeSelector = kind {
        let tab_count = view.tabs.len();
        if tab_count > 0 && layout.tab_strip.height > 0.0 {
            let tab_w = layout.tab_strip.width / tab_count as f64;
            for i in 0..tab_count {
                let tab_rect = Rect::new(
                    layout.tab_strip.x + i as f64 * tab_w,
                    layout.tab_strip.y,
                    tab_w,
                    layout.tab_strip.height,
                );
                coord.register_child(
                    &sidebar_id,
                    format!("{}:tab:{}", sidebar_id.0.0, i),
                    WidgetKind::Button,
                    tab_rect,
                    Sense::CLICK | Sense::HOVER,
                );
            }
        }
    }

    // --- Resize zone ---
    if layout.resize_zone.width > 0.0 && layout.resize_zone.height > 0.0 {
        coord.register_child(
            &sidebar_id,
            format!("{}:resize", sidebar_id.0.0),
            WidgetKind::Separator,
            layout.resize_zone,
            Sense::DRAG,
        );
    }

    // --- Scrollbar handle + track ---
    if view.effective_show_scrollbar() && layout.scrollbar.width > 0.0 {
        // Inflated hit zone: ±5 px on x-axis (matches mlc scrollbar_handle inflation).
        let scroll = state
            .scroll_per_panel
            .get(view.active_tab.unwrap_or("default"));
        let scroll_offset = scroll.map(|s| s.offset).unwrap_or(0.0);
        let viewport_h = layout.body.height;
        let content_h  = view.content_height;

        if content_h > viewport_h {
            let visible_ratio = (viewport_h / content_h).clamp(0.0, 1.0);
            let max_scroll    = (content_h - viewport_h).max(0.0);
            let scroll_ratio  = if max_scroll > 0.0 { (scroll_offset / max_scroll).clamp(0.0, 1.0) } else { 0.0 };
            let thumb_len     = (layout.scrollbar.height * visible_ratio).max(24.0).min(layout.scrollbar.height);
            let available     = (layout.scrollbar.height - thumb_len).max(0.0);
            let thumb_y       = layout.scrollbar.y + scroll_ratio * available;
            let thumb_rect    = Rect::new(layout.scrollbar.x, thumb_y, layout.scrollbar.width, thumb_len);
            let inflated      = Rect::new(
                thumb_rect.x - 5.0,
                thumb_rect.y,
                thumb_rect.width + 10.0,
                thumb_rect.height,
            );

            coord.register_child(
                &sidebar_id,
                format!("{}:scrollbar_handle", sidebar_id.0.0),
                WidgetKind::ScrollbarHandle,
                inflated,
                Sense::DRAG,
            );
            coord.register_child(
                &sidebar_id,
                format!("{}:scrollbar_track", sidebar_id.0.0),
                WidgetKind::ScrollbarTrack,
                layout.scrollbar,
                Sense::CLICK,
            );
        }
    }

    // --- Viewport scroll zone ---
    coord.register_child(
        &sidebar_id,
        format!("{}:viewport", sidebar_id.0.0),
        WidgetKind::Custom,
        layout.body,
        Sense::SCROLL,
    );

    // --- Chevron pager hit zones (OverflowMode::Chevrons) -------------------
    // Two 16-px overlay strips at the top and bottom of the body region.
    // Visible only when content overflows; each chevron pages the body
    // scroll offset by one viewport.
    if matches!(view.overflow, crate::types::OverflowMode::Chevrons) {
        let content_h  = view.content_height;
        let viewport_h = layout.body.height;
        if content_h > viewport_h && viewport_h > 0.0 {
            let panel_key   = view.active_tab.unwrap_or("default");
            let cur_offset  = state.scroll_per_panel.get(panel_key).map(|s| s.offset).unwrap_or(0.0);
            let max_offset  = (content_h - viewport_h).max(0.0);
            let strip_h     = 16.0_f64;
            if cur_offset > 0.0 {
                coord.register_child(
                    &sidebar_id,
                    format!("{}:chevron_up", sidebar_id.0.0),
                    WidgetKind::ScrollChevron,
                    Rect::new(layout.body.x, layout.body.y, layout.body.width, strip_h),
                    Sense::CLICK | Sense::HOVER,
                );
            }
            if cur_offset < max_offset {
                coord.register_child(
                    &sidebar_id,
                    format!("{}:chevron_down", sidebar_id.0.0),
                    WidgetKind::ScrollChevron,
                    Rect::new(layout.body.x, layout.body.y + layout.body.height - strip_h, layout.body.width, strip_h),
                    Sense::CLICK | Sense::HOVER,
                );
            }
        }
    }

    sidebar_id
}

// ---------------------------------------------------------------------------
// Public API — convenience wrapper (ContextManager)
// ---------------------------------------------------------------------------

/// Register + draw a sidebar in one call using a `ContextManager`.
///
/// # Arguments
/// - `ctx_mgr`  — context manager (`coord` extracted as `&mut ctx_mgr.input`).
/// - `render`   — render context.
/// - `id`       — stable widget id.
/// - `rect`     — bounding rect (full sidebar area).
/// - `state`    — mutable sidebar state.
/// - `view`     — per-frame data (header, body closure, etc.).
/// - `settings` — theme + style configuration.
/// - `kind`     — selects the layout pipeline.
/// - `layer`    — coordinator layer.
///
/// Returns the `WidgetId` assigned to the sidebar composite.
pub fn register_context_manager_sidebar(
    ctx_mgr:  &mut crate::app_context::ContextManager,
    render:   &mut dyn RenderContext,
    id:       impl Into<WidgetId>,
    rect:     Rect,
    state:    &mut SidebarState,
    view:     &mut SidebarView<'_>,
    settings: &SidebarSettings,
    kind:     &SidebarRenderKind,
    layer:    &LayerId,
) -> CompositeId {
    let coord = &mut ctx_mgr.input;
    let sidebar_id =
        register_input_coordinator_sidebar(coord, id, rect, state, view, settings, kind, layer);
    draw_sidebar_with_coord(render, rect, coord, state, view, settings, kind);
    sidebar_id
}

// ---------------------------------------------------------------------------
// Internal draw pipeline
// ---------------------------------------------------------------------------

fn draw_sidebar_with_coord(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    _coord:    &mut InputCoordinator,
    state:    &mut SidebarState,
    view:     &mut SidebarView<'_>,
    settings: &SidebarSettings,
    kind:     &SidebarRenderKind,
) {
    if let SidebarRenderKind::Custom(f) = kind {
        f(ctx, rect, view, settings);
        return;
    }

    let theme  = settings.theme.as_ref();
    let style  = settings.style.as_ref();
    let layout = compute_layout(rect, state, view, settings, kind);

    // --- 1. Background -------------------------------------------------------
    match style.background_fill() {
        BackgroundFill::Solid => {
            ctx.set_fill_color(theme.bg());
            ctx.fill_rect(layout.frame.x, layout.frame.y, layout.frame.width, layout.frame.height);
        }
        BackgroundFill::Glass { blur_radius: _ } => {
            ctx.draw_blur_background(layout.frame.x, layout.frame.y, layout.frame.width, layout.frame.height);
            ctx.set_fill_color(theme.bg());
            ctx.fill_rect(layout.frame.x, layout.frame.y, layout.frame.width, layout.frame.height);
        }
    }

    // --- 2. Frame borders ----------------------------------------------------
    // Per-side configuration via `style.borders()`. If the style returns the
    // all-None sentinel (legacy default) we fall back to "inner edge only"
    // — the previous single-line behaviour — so existing presets keep
    // looking the same.
    let raw_borders = style.borders();
    let resolved_borders = if raw_borders.top.is_none()
        && raw_borders.right.is_none()
        && raw_borders.bottom.is_none()
        && raw_borders.left.is_none()
    {
        let s = Some(super::style::BorderStroke::default());
        match kind {
            SidebarRenderKind::Left => super::style::BorderConfig {
                top: None, right: s, bottom: None, left: None,
            },
            SidebarRenderKind::Top => super::style::BorderConfig {
                top: None, right: None, bottom: s, left: None,
            },
            SidebarRenderKind::Bottom => super::style::BorderConfig {
                top: s, right: None, bottom: None, left: None,
            },
            SidebarRenderKind::Embedded => super::style::BorderConfig::none(),
            _ => super::style::BorderConfig {
                top: None, right: None, bottom: None, left: s,
            },
        }
    } else {
        raw_borders
    };

    let f = layout.frame;
    let theme_border = theme.border();
    let mut paint_stroke = |x: f64, y: f64, w: f64, h: f64, stroke: super::style::BorderStroke| {
        if w <= 0.0 || h <= 0.0 || stroke.width <= 0.0 || stroke.opacity <= 0.0 {
            return;
        }
        ctx.save();
        ctx.set_global_alpha(stroke.opacity);
        ctx.set_fill_color(theme_border);
        ctx.fill_rect(x, y, w, h);
        ctx.restore();
    };
    if let Some(s) = resolved_borders.top    { paint_stroke(f.x, f.y, f.width, s.width, s); }
    if let Some(s) = resolved_borders.bottom { paint_stroke(f.x, f.y + f.height - s.width, f.width, s.width, s); }
    if let Some(s) = resolved_borders.left   { paint_stroke(f.x, f.y, s.width, f.height, s); }
    if let Some(s) = resolved_borders.right  { paint_stroke(f.x + f.width - s.width, f.y, s.width, f.height, s); }

    // --- 3. Header background + icon + title ---------------------------------
    ctx.set_fill_color(theme.header_bg());
    ctx.fill_rect(layout.header.x, layout.header.y, layout.header.width, layout.header.height);

    let pad       = style.padding();
    let icon_size = 18.0_f64;
    let icon_pad  = 12.0_f64;
    let title_off = 36.0_f64; // icon_pad + icon_size + gap (≈6 px)

    // Icon
    if view.header.icon.is_some() {
        // Icon rendering delegated to caller via draw_icon callback.
        // Here we just reserve the rect; the caller's body closure can draw it.
        // Rendering inline as a colored rect placeholder at icon position:
        let icon_rect = Rect::new(
            layout.header.x + icon_pad,
            layout.header.y + (layout.header.height - icon_size) / 2.0,
            icon_size,
            icon_size,
        );
        let _ = icon_rect; // icon_rect registered visually; actual draw is caller's job
    }

    // Title
    let title_x = if view.header.icon.is_some() {
        layout.header.x + title_off
    } else {
        layout.header.x + pad
    };
    ctx.set_fill_color(theme.header_text());
    ctx.set_font("13px sans-serif");
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(
        view.header.title,
        title_x,
        layout.header.y + layout.header.height / 2.0,
    );

    // Action buttons (icon-only, right-to-left)
    let btn_size  = 24.0_f64;
    let btn_gap   = 8.0_f64;
    let pad_right = 12.0_f64;
    for (i, action) in view.header.actions.iter().enumerate() {
        let btn_x = layout.header.x + layout.header.width
            - pad_right
            - btn_size * (i + 1) as f64
            - btn_gap * i as f64;
        let btn_y = layout.header.y + (layout.header.height - btn_size) / 2.0;
        let hovered = state.header_action_hovered.as_deref() == Some(action.id);

        if hovered {
            ctx.set_fill_color("rgba(255,255,255,0.08)");
            ctx.fill_rounded_rect(btn_x, btn_y, btn_size, btn_size, 4.0);
        }

        let icon_color = if hovered {
            theme.action_icon_hover()
        } else {
            theme.action_icon_normal()
        };
        // Icon drawn as a placeholder colored square for now;
        // actual SVG drawing uses caller-provided callback.
        ctx.set_fill_color(icon_color);
        let icon_inner = 16.0_f64;
        let ix = btn_x + (btn_size - icon_inner) / 2.0;
        let iy = btn_y + (btn_size - icon_inner) / 2.0;
        let _ = (ix, iy); // icon draw point; real SVG render by caller
    }

    // --- 4. Header bottom divider --------------------------------------------
    {
        let cfg = style.header_divider();
        // Legacy `show_header_divider() == false` overrides the new config.
        if cfg.visible && style.show_header_divider() && cfg.opacity > 0.0 && cfg.width > 0.0 {
            let len_frac = cfg.length_frac.clamp(0.0, 1.0);
            let line_w   = layout.header.width * len_frac;
            let line_x   = layout.header.x + (layout.header.width - line_w) / 2.0;
            ctx.save();
            ctx.set_global_alpha(cfg.opacity.clamp(0.0, 1.0));
            ctx.set_fill_color(theme.divider());
            ctx.fill_rect(
                line_x,
                layout.header.y + layout.header.height - cfg.width,
                line_w,
                cfg.width,
            );
            ctx.restore();
        }
    }

    // --- 5. Tab strip (WithTypeSelector) -------------------------------------
    if let SidebarRenderKind::WithTypeSelector = kind {
        draw_tab_strip(ctx, &layout, view, state, settings);
    }

    // --- 6. (body drawn by caller after composite call) ----------------------

    // --- 7. Scrollbar --------------------------------------------------------
    if view.effective_show_scrollbar() && layout.scrollbar.width > 0.0 {
        let panel_key = view.active_tab.unwrap_or("default");
        let scroll_offset = state
            .scroll_per_panel
            .get(panel_key)
            .map(|s| s.offset)
            .unwrap_or(0.0);
        let scb_state = if state.scroll_per_panel
            .get(panel_key)
            .map(|s| s.is_dragging)
            .unwrap_or(false)
        {
            ScrollbarVisualState::Dragging
        } else {
            ScrollbarVisualState::Active
        };
        draw_scrollbar_standard(
            ctx,
            layout.scrollbar,
            view.content_height,
            layout.body.height,
            scroll_offset,
            scb_state,
            None,
        );
    }

    // --- 7b. Chevron pager (OverflowMode::Chevrons) -------------------------
    if matches!(view.overflow, crate::types::OverflowMode::Chevrons) {
        let content_h  = view.content_height;
        let viewport_h = layout.body.height;
        if content_h > viewport_h && viewport_h > 0.0 {
            let panel_key  = view.active_tab.unwrap_or("default");
            let cur_offset = state.scroll_per_panel.get(panel_key).map(|s| s.offset).unwrap_or(0.0);
            let max_offset = (content_h - viewport_h).max(0.0);
            let strip_h    = 16.0_f64;

            use crate::ui::widgets::atomic::chevron::render::draw_chevron;
            use crate::ui::widgets::atomic::chevron::settings::ChevronSettings;
            use crate::ui::widgets::atomic::chevron::types::{
                ChevronDirection, ChevronUseCase, ChevronView, ChevronVisualKind,
                HitAreaPolicy, PlacementPolicy, VisibilityPolicy,
            };

            let cs = ChevronSettings::default();
            // Top strip — visible only if there's content above.
            if cur_offset > 0.0 {
                let r = Rect::new(layout.body.x, layout.body.y, layout.body.width, strip_h);
                // Faint backdrop so the strip reads as a control, not a row.
                ctx.set_fill_color("rgba(20,22,28,0.85)");
                ctx.fill_rect(r.x, r.y, r.width, r.height);
                draw_chevron(ctx, r, &ChevronView {
                    direction: ChevronDirection::Up,
                    use_case: ChevronUseCase::PageStep,
                    visibility: VisibilityPolicy::Always,
                    placement: PlacementPolicy::Overlay,
                    hit_area: HitAreaPolicy::Visual,
                    visual_kind: ChevronVisualKind::Stroked,
                    ..ChevronView::default()
                }, &cs);
            }
            // Bottom strip — visible only if there's content below.
            if cur_offset < max_offset {
                let r = Rect::new(layout.body.x, layout.body.y + layout.body.height - strip_h, layout.body.width, strip_h);
                ctx.set_fill_color("rgba(20,22,28,0.85)");
                ctx.fill_rect(r.x, r.y, r.width, r.height);
                draw_chevron(ctx, r, &ChevronView {
                    direction: ChevronDirection::Down,
                    use_case: ChevronUseCase::PageStep,
                    visibility: VisibilityPolicy::Always,
                    placement: PlacementPolicy::Overlay,
                    hit_area: HitAreaPolicy::Visual,
                    visual_kind: ChevronVisualKind::Stroked,
                    ..ChevronView::default()
                }, &cs);
            }
        }
    }

    // --- 8. Resize edge visual — only highlights while actively resizing ----
    // Border line itself was already drawn in step 2 at the standard divider
    // colour. We only repaint it with the accent colour while the user is
    // dragging the handle; otherwise it blends in with the regular UI border.
    let shows_resize = matches!(
        kind,
        SidebarRenderKind::Right
            | SidebarRenderKind::Left
            | SidebarRenderKind::Top
            | SidebarRenderKind::Bottom
            | SidebarRenderKind::WithTypeSelector
    );
    if shows_resize && state.resize_dragging {
        let bl = layout.border_line;
        if bl.width > 0.0 && bl.height > 0.0 {
            let stripe_w = 2.0_f64;
            ctx.set_fill_color(settings.theme.tab_accent());
            if bl.width >= bl.height {
                ctx.fill_rect(bl.x, bl.y - (stripe_w - bl.height) / 2.0, bl.width, stripe_w);
            } else {
                ctx.fill_rect(bl.x - (stripe_w - bl.width) / 2.0, bl.y, stripe_w, bl.height);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tab strip drawing helper
// ---------------------------------------------------------------------------

fn draw_tab_strip(
    ctx:      &mut dyn RenderContext,
    layout:   &SidebarLayout,
    view:     &SidebarView<'_>,
    state:    &SidebarState,
    settings: &SidebarSettings,
) {
    let theme     = settings.theme.as_ref();
    let tab_count = view.tabs.len();
    if tab_count == 0 || layout.tab_strip.height <= 0.0 {
        return;
    }

    let tab_w = layout.tab_strip.width / tab_count as f64;

    // Tab strip background (same as sidebar bg — no separate fill needed)
    // Draw each tab
    for (i, tab) in view.tabs.iter().enumerate() {
        let tab_rect = Rect::new(
            layout.tab_strip.x + i as f64 * tab_w,
            layout.tab_strip.y,
            tab_w,
            layout.tab_strip.height,
        );

        let is_active = view.active_tab == Some(tab.id)
            || state.active_tab.as_deref() == Some(tab.id);

        // Background
        if is_active {
            ctx.set_fill_color(theme.tab_bg_active());
            ctx.fill_rect(tab_rect.x, tab_rect.y, tab_rect.width, tab_rect.height);
        }

        // Label
        let text_color = if is_active {
            theme.tab_text_active()
        } else {
            theme.tab_text_inactive()
        };
        ctx.set_fill_color(text_color);
        ctx.set_font("12px sans-serif");
        ctx.set_text_align(TextAlign::Center);
        ctx.set_text_baseline(TextBaseline::Middle);
        ctx.fill_text(
            tab.label,
            tab_rect.x + tab_rect.width / 2.0,
            tab_rect.y + tab_rect.height / 2.0,
        );

        // Active underline (2 px bottom accent)
        if is_active {
            ctx.set_fill_color(theme.tab_accent());
            ctx.fill_rect(
                tab_rect.x,
                tab_rect.y + tab_rect.height - 2.0,
                tab_rect.width,
                2.0,
            );
        }
    }

    // Tab strip bottom border
    ctx.set_fill_color(settings.theme.divider());
    ctx.fill_rect(
        layout.tab_strip.x,
        layout.tab_strip.y + layout.tab_strip.height - 1.0,
        layout.tab_strip.width,
        1.0,
    );
}

// ---------------------------------------------------------------------------
// Layout computation
// ---------------------------------------------------------------------------

/// The body region as the caller should treat it: scroll-clipped rect plus
/// the y-coordinate at which to anchor the first row of content.
///
/// Returned by [`body_viewport`]. The composite has already applied the
/// scroll offset to `content_origin_y`, so the caller draws relative to it
/// without thinking about scroll. Drawing outside `clip_rect` is hidden by
/// the composite-supplied `ctx.save() / clip_rect / restore` pair (see
/// [`begin_body`] / [`end_body`]).
#[derive(Clone, Copy, Debug)]
pub struct SidebarBodyViewport {
    /// Clip rectangle — the caller must keep all body content inside this.
    pub clip_rect: Rect,
    /// Y coordinate of the first row of scrollable content (already shifted
    /// by `state.scroll_per_panel[panel_key].offset`).
    pub content_origin_y: f64,
    /// Scroll offset that has been applied (informational; useful for
    /// callers that compute child rects relative to scroll).
    pub scroll_offset: f64,
    /// Visible viewport height (= `clip_rect.height`).
    pub viewport_height: f64,
}

/// Compute the body viewport — header / tab-strip / scrollbar are excluded
/// from `clip_rect`, and `content_origin_y` already has scroll applied.
pub fn body_viewport(
    frame_rect: Rect,
    state:      &SidebarState,
    view:       &SidebarView<'_>,
    settings:   &SidebarSettings,
    kind:       &SidebarRenderKind,
) -> SidebarBodyViewport {
    let layout = compute_layout(frame_rect, state, view, settings, kind);
    let panel_key = view.active_tab.unwrap_or("default");
    let scroll_offset = state.scroll_per_panel.get(panel_key).map(|s| s.offset).unwrap_or(0.0);
    SidebarBodyViewport {
        clip_rect: layout.body,
        content_origin_y: layout.body.y - scroll_offset,
        scroll_offset,
        viewport_height: layout.body.height,
    }
}

/// Push a scroll-aware clip rect for the sidebar body. Pair with
/// [`end_body`] to restore the render context. Returns the same viewport
/// info as [`body_viewport`] for convenience.
pub fn begin_body(
    ctx:        &mut dyn RenderContext,
    frame_rect: Rect,
    state:      &SidebarState,
    view:       &SidebarView<'_>,
    settings:   &SidebarSettings,
    kind:       &SidebarRenderKind,
) -> SidebarBodyViewport {
    let vp = body_viewport(frame_rect, state, view, settings, kind);
    ctx.save();
    ctx.clip_rect(vp.clip_rect.x, vp.clip_rect.y, vp.clip_rect.width, vp.clip_rect.height);
    vp
}

/// Restore the render context after a [`begin_body`] call.
pub fn end_body(ctx: &mut dyn RenderContext) {
    ctx.restore();
}

/// Measure the natural width of a sidebar (`style.default_width()`) and the
/// chrome overhead height (header + optional tab strip).
///
/// Sidebar body height is layout-driven, not content-driven — caller adds the
/// available body height to `chrome_h` to get total height.
///
/// Returns `(default_w, chrome_h)`.
pub fn measure(
    settings: &SidebarSettings,
    kind:     &SidebarRenderKind,
) -> (f64, f64) {
    let style = settings.style.as_ref();
    let header_h    = style.header_height();
    let tab_strip_h = match kind {
        SidebarRenderKind::WithTypeSelector => style.tab_strip_height(),
        _ => 0.0,
    };
    (style.default_width(), header_h + tab_strip_h)
}

fn compute_layout(
    rect:     Rect,
    _state:   &SidebarState,
    view:     &SidebarView<'_>,
    settings: &SidebarSettings,
    kind:     &SidebarRenderKind,
) -> SidebarLayout {
    let style = settings.style.as_ref();

    let header_h     = style.header_height();
    let tab_strip_h  = match kind {
        SidebarRenderKind::WithTypeSelector => style.tab_strip_height(),
        _ => 0.0,
    };
    let scrollbar_w  = if view.effective_show_scrollbar() { style.scrollbar_width() } else { 0.0 };
    let resize_w     = match kind {
        SidebarRenderKind::Embedded | SidebarRenderKind::Custom(_) => 0.0,
        _ => style.resize_zone_width(),
    };
    let border_w     = style.border_width();

    let frame = rect;

    let header = Rect::new(frame.x, frame.y, frame.width, header_h);

    let tab_strip = if tab_strip_h > 0.0 {
        Rect::new(frame.x, frame.y + header_h, frame.width, tab_strip_h)
    } else {
        Rect::default()
    };

    // Body starts after header + tab strip; right edge reserved for scrollbar
    let body_y = frame.y + header_h + tab_strip_h;
    let body_h = (frame.height - header_h - tab_strip_h).max(0.0);
    let body_w = frame.width - scrollbar_w;
    let body   = Rect::new(frame.x, body_y, body_w, body_h);

    let scrollbar = if scrollbar_w > 0.0 {
        Rect::new(frame.x + frame.width - scrollbar_w, body_y, scrollbar_w, body_h)
    } else {
        Rect::default()
    };

    // Resize zone: centered on the relevant border edge.
    // The handle sits OUTSIDE the sidebar frame (half its width straddles into
    // the dock area) so dragging from the visible separator line works.
    let resize_zone = if resize_w > 0.0 {
        match kind {
            SidebarRenderKind::Left => {
                // Right edge of a left sidebar
                Rect::new(
                    frame.x + frame.width - resize_w / 2.0,
                    frame.y,
                    resize_w,
                    frame.height,
                )
            }
            SidebarRenderKind::Top => {
                // Bottom edge of a top sidebar
                Rect::new(
                    frame.x,
                    frame.y + frame.height - resize_w / 2.0,
                    frame.width,
                    resize_w,
                )
            }
            SidebarRenderKind::Bottom => {
                // Top edge of a bottom sidebar
                Rect::new(
                    frame.x,
                    frame.y - resize_w / 2.0,
                    frame.width,
                    resize_w,
                )
            }
            _ => {
                // Left edge of a right sidebar (Right / WithTypeSelector / Custom)
                Rect::new(
                    frame.x - resize_w / 2.0,
                    frame.y,
                    resize_w,
                    frame.height,
                )
            }
        }
    } else {
        Rect::default()
    };

    // Border line (1 px visual) on the inner edge that faces the dock area.
    let border_line = if border_w > 0.0 {
        match kind {
            SidebarRenderKind::Left => {
                Rect::new(frame.x + frame.width - border_w, frame.y, border_w, frame.height)
            }
            SidebarRenderKind::Top => {
                Rect::new(frame.x, frame.y + frame.height - border_w, frame.width, border_w)
            }
            SidebarRenderKind::Bottom => {
                Rect::new(frame.x, frame.y, frame.width, border_w)
            }
            _ => {
                Rect::new(frame.x, frame.y, border_w, frame.height)
            }
        }
    } else {
        Rect::default()
    };

    SidebarLayout {
        frame,
        header,
        tab_strip,
        body,
        scrollbar,
        resize_zone,
        border_line,
    }
}
