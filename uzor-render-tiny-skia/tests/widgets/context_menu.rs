//! Golden grid (H1 Brief 10a-composites): context_menu × 4 built-in sets ×
//! its `Default` kind (icon column + separators + blur background, module
//! doc table). Fixed 5-item menu covering every row style the composite
//! draws differently: a plain enabled item, a separator after an item, a
//! disabled item (dimmed text, no hover per `ContextMenuItem::enabled` doc),
//! and a danger item (red text / red hover bg). `ContextMenuState::
//! hovered_index` is the widget's whole interactive axis — `draw_context_menu`
//! never reads a clock or an `InputCoordinator` — so "plain" / "item_hovered"
//! / "danger_hovered" are a fixed `hovered_index` value per cell, never a
//! live pointer sample.

#![cfg(feature = "golden")]

use uzor::tokens::{BuiltinSet, Tokens, TokenTheme};
use uzor::ui::widgets::composite::context_menu::{
    self, ContextMenuItem, ContextMenuRenderKind, ContextMenuSettings, ContextMenuState,
    ContextMenuView, DefaultContextMenuStyle,
};
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

use super::support::{self, ReviewSheet};

/// Canvas margin around the measured menu panel — wide enough to clear the
/// frame's own `shadow_offset` (`DefaultContextMenuStyle::shadow_offset` =
/// `(3.0, 3.0)`) so the shadow is never clipped at the canvas edge.
const MARGIN: f64 = 16.0;
const ORIGIN: f64 = 8.0;

/// Fixed 5-item menu: a plain item, a separator, an enabled item, a disabled
/// item, and a danger item — every row style `draw_items` (render.rs) draws
/// differently (module doc).
fn items() -> Vec<ContextMenuItem<'static>> {
    vec![
        ContextMenuItem {
            action: "copy",
            label: "Copy",
            icon: None,
            danger: false,
            separator_after: false,
            enabled: true,
        },
        ContextMenuItem {
            action: "paste",
            label: "Paste",
            icon: None,
            danger: false,
            separator_after: true,
            enabled: true,
        },
        ContextMenuItem {
            action: "rename",
            label: "Rename",
            icon: None,
            danger: false,
            separator_after: false,
            enabled: true,
        },
        ContextMenuItem {
            action: "archive",
            label: "Archived",
            icon: None,
            danger: false,
            separator_after: false,
            enabled: false,
        },
        ContextMenuItem {
            action: "delete",
            label: "Delete",
            icon: None,
            danger: true,
            separator_after: false,
            enabled: true,
        },
    ]
}

/// `(state name, hovered item index)` — `ContextMenuState::hovered_index` is
/// the widget's whole interactive axis for a fixed-content golden (module
/// doc). Index 2 ("Rename") and 4 ("Delete") are both `enabled: true` rows,
/// the only kind `draw_items` ever highlights on hover.
const STATES: &[(&str, Option<usize>)] =
    &[("plain", None), ("item_hovered", Some(2)), ("danger_hovered", Some(4))];

fn render(set: BuiltinSet, hovered_index: Option<usize>) -> TinySkiaCpuRenderContext {
    let items = items();
    let view = ContextMenuView { items: &items, target_id: None, title: None };
    let settings = ContextMenuSettings {
        theme: Box::new(TokenTheme::new(Tokens::builtin(set))),
        style: Box::<DefaultContextMenuStyle>::default(),
    };
    let kind = ContextMenuRenderKind::Default;
    let (menu_w, menu_h) = context_menu::measure(&view, &settings, &kind);

    let mut ctx = support::canvas((menu_w + MARGIN).ceil() as u32, (menu_h + MARGIN).ceil() as u32, set);
    let state = ContextMenuState {
        is_open: true,
        x: ORIGIN,
        y: ORIGIN,
        target_id: None,
        hovered_index,
        primed_index: None,
    };
    context_menu::draw_context_menu(&mut ctx, &state, &view, &settings, &kind);
    ctx
}

#[test]
fn context_menu_matches_golden_grid() {
    let mut sheet = ReviewSheet::new();
    support::for_each_set(|set, _tokens| {
        for &(state_name, hovered_index) in STATES {
            let ctx = render(set, hovered_index);
            support::golden("context_menu", set, state_name, &ctx)
                .expect("context_menu golden mismatch");
            sheet.add(set, state_name, &ctx);
        }
    });
    sheet.save("context_menu");
}
