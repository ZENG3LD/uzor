//! Dock and chrome paint for compose (brief F11, design §9.4).
//!
//! F7 filled the chrome strip and overlay frames with flat token boxes.
//! Goldens need the strip the hit-test already classifies (`draw_chrome`),
//! the dock's headers, tab bars, splitters, a drag ghost with its drop
//! zone, a pin mark, and a focus ring. Still no render backend: paint goes
//! through [`RenderContext`] only.

use uzor::layout::docking::{DockPanel, DropZone, PanelRect, SeparatorState};
use uzor::render::RenderContext;
use uzor::tokens::TokenTheme;
use uzor::tokens::Tokens;
use uzor::widgets::composite::chrome::{
    draw_chrome, ChromeRenderKind, ChromeSettings, ChromeTabConfig, ChromeView, DefaultChromeStyle,
};
use uzor::Rect;

use crate::engine::layout::{SessionKind, WindowLayoutView};
use crate::types::command::ChromeKind;

/// Paint the window chrome strip with the library chrome composite, so the
/// pixels match the hit zones in `engine/layout/chrome.rs`.
pub(super) fn paint_chrome<P: DockPanel>(
    render: &mut dyn RenderContext,
    wv: WindowLayoutView<'_, P>,
    tokens: &Tokens,
) {
    let Some(rect) = wv.solved().chrome else {
        return;
    };
    let model = wv.chrome_model();
    let tabs: Vec<ChromeTabConfig<'_>> = model
        .tabs
        .iter()
        .map(|t| ChromeTabConfig {
            id: t.label.as_str(),
            label: t.label.as_str(),
            icon: None,
            color_tag: None,
            closable: t.closable,
            active: false,
        })
        .collect();
    let active = tabs.first().map(|t| t.id);
    let b = &model.buttons;
    let view = ChromeView {
        tabs: &tabs,
        active_tab_id: active,
        show_new_tab_btn: b.show_new_tab_btn,
        show_menu_btn: b.show_menu_btn,
        show_new_window_btn: b.show_new_window_btn,
        show_close_window_btn: b.show_close_window_btn,
        is_maximized: false,
        menu_left: b.menu_left,
        show_maximize: b.show_maximize,
        cursor_x: 0.0,
        cursor_y: 0.0,
        time_ms: 0.0,
    };
    let settings = ChromeSettings {
        theme: Box::new(TokenTheme::from_tokens(tokens)),
        style: Box::new(DefaultChromeStyle),
    };
    let kind = match model.kind {
        ChromeKind::Default => ChromeRenderKind::Default,
        ChromeKind::Minimal => ChromeRenderKind::Minimal,
        ChromeKind::WindowControlsOnly => ChromeRenderKind::WindowControlsOnly,
    };
    draw_chrome(render, rect, wv.chrome_state(), &view, &settings, &kind);
}

/// Headers, tab bars, splitters, the live drop zone and the drag ghost.
pub(super) fn paint_dock<P: DockPanel>(
    render: &mut dyn RenderContext,
    wv: WindowLayoutView<'_, P>,
    tokens: &Tokens,
) {
    let dock = wv.dock();
    let header = tokens.semantic.surface_header.to_css();
    let text = tokens.semantic.text_primary.to_css();
    let border = tokens.semantic.border_default.to_css();
    let accent = tokens.semantic.accent_default.to_css();
    let dragging = match wv.session() {
        Some(SessionKind::Splitter { sep, .. }) => Some(sep),
        _ => None,
    };

    for (id, hdr) in dock.panel_headers() {
        let (x, y, w, h) = xywh(*hdr);
        render.set_fill_color(&header);
        render.fill_rect(x, y, w, h);
        if let Some(leaf) = dock.tree().leaf(*id) {
            if let Some(panel) = leaf.active_panel() {
                render.set_fill_color(&text);
                render.set_font("12px sans-serif");
                render.set_text_align(uzor::render::TextAlign::Left);
                render.set_text_baseline(uzor::render::TextBaseline::Middle);
                render.fill_text(panel.title(), x + 8.0, y + h * 0.5);
                if panel.pin().locks_tear_off() {
                    // `Pin::User { pinned: true }` (and `System`): a mark in
                    // the header so the golden can see the lock.
                    render.set_fill_color(&accent);
                    render.fill_rect(x + w - 16.0, y + (h - 8.0) * 0.5, 8.0, 8.0);
                }
            }
        }
    }

    for bar in dock.tab_bars() {
        let (x, y, w, h) = xywh(bar.rect);
        render.set_fill_color(&header);
        render.fill_rect(x, y, w, h);
        for tab in &bar.tabs {
            let (tx, ty, tw, th) = xywh(tab.rect);
            if tab.is_active {
                render.set_fill_color(&tokens.semantic.surface_panel.to_css());
                render.fill_rect(tx, ty, tw, th);
                render.set_fill_color(&accent);
                render.fill_rect(tx, ty + th - 2.0, tw, 2.0);
            }
            render.set_fill_color(&text);
            render.set_font("12px sans-serif");
            render.set_text_align(uzor::render::TextAlign::Left);
            render.set_text_baseline(uzor::render::TextBaseline::Middle);
            render.fill_text(&tab.title, tx + 8.0, ty + th * 0.5);
        }
    }

    for (index, sep) in dock.separators().iter().enumerate() {
        let (pos, start, len) = (sep.position as f64, sep.start as f64, sep.length as f64);
        let hot = dragging == Some(index) || sep.state == SeparatorState::Dragging;
        let t = if hot { 4.0 } else { 2.0 };
        let (x, y, w, h) = match sep.orientation {
            uzor::layout::docking::SeparatorOrientation::Vertical => {
                (pos - t * 0.5, start, t, len)
            }
            uzor::layout::docking::SeparatorOrientation::Horizontal => {
                (start, pos - t * 0.5, len, t)
            }
        };
        render.set_fill_color(if hot { &accent } else { &border });
        render.fill_rect(x, y, w, h);
    }

    if let Some(drag) = dock.panel_drag_state() {
        if let (Some(target), Some(zone)) = (drag.target_leaf_id, drag.drop_zone) {
            if let Some(rect) = dock.panel_rects().get(&target).copied() {
                let preview = zone_preview(zone, rect);
                let (x, y, w, h) = xywh(preview);
                render.set_fill_color_alpha(&accent, 0.35);
                render.fill_rect(x, y, w, h);
            }
        }
        let ghost = Rect::new(drag.current_x as f64, drag.current_y as f64, 140.0, 22.0);
        render.set_fill_color(&tokens.semantic.surface_floating.to_css());
        render.fill_rect(ghost.x, ghost.y, ghost.width, ghost.height);
        render.set_fill_color(&border);
        render.fill_rect(ghost.x, ghost.y, ghost.width, 1.0);
        if let Some(leaf) = dock.tree().leaf(drag.dragged_leaf_id) {
            if let Some(panel) = leaf.active_panel() {
                render.set_fill_color(&text);
                render.set_font("12px sans-serif");
                render.set_text_align(uzor::render::TextAlign::Left);
                render.set_text_baseline(uzor::render::TextBaseline::Middle);
                render.fill_text(panel.title(), ghost.x + 8.0, ghost.y + ghost.height * 0.5);
            }
        }
    }
}

/// A 2 px ring in `focus_ring` around the focused widget.
pub(super) fn paint_focus_ring(render: &mut dyn RenderContext, tokens: &Tokens, rect: Option<Rect>) {
    let Some(rect) = rect else {
        return;
    };
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return;
    }
    let color = tokens.semantic.focus_ring.to_css();
    render.set_fill_color(&color);
    let t = 2.0;
    render.fill_rect(rect.x - t, rect.y - t, rect.width + t * 2.0, t);
    render.fill_rect(rect.x - t, rect.y + rect.height, rect.width + t * 2.0, t);
    render.fill_rect(rect.x - t, rect.y, t, rect.height);
    render.fill_rect(rect.x + rect.width, rect.y, t, rect.height);
}

fn xywh(r: PanelRect) -> (f64, f64, f64, f64) {
    (r.x as f64, r.y as f64, r.width as f64, r.height as f64)
}

fn zone_preview(zone: DropZone, rect: PanelRect) -> PanelRect {
    zone.preview_rect(rect)
}
