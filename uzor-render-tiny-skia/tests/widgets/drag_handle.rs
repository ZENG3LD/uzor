//! Golden grid (H1 Brief 10a-2): drag_handle × 4 built-in sets × its own
//! real state set. `DragHandleView` (module doc: "No visual state beyond
//! the rect — the drag handle is purely a hit zone") carries no
//! interaction flags at all, so the only axis that changes what gets
//! painted is `DragHandleRenderKind` itself: `Invisible` (no-op — pure hit
//! zone, renders nothing) and `GripDots` (the 2×3 dot grid). `Custom` is an
//! app-supplied escape hatch, out of scope here (same reasoning as every
//! other golden in this suite).

#![cfg(feature = "golden")]

use uzor::tokens::{BuiltinSet, TokenTheme, Tokens};
use uzor::types::Rect;
use uzor::ui::widgets::atomic::drag_handle::{self, DragHandleRenderKind, DragHandleSettings, DragHandleView};
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

use super::support::{self, ReviewSheet};

fn rect() -> Rect {
    Rect::new(4.0, 4.0, 24.0, 24.0)
}

fn render(set: BuiltinSet, kind: &DragHandleRenderKind) -> TinySkiaCpuRenderContext {
    let mut ctx = support::canvas(32, 32, set);
    let settings = DragHandleSettings::default().with_theme(Box::new(TokenTheme::new(Tokens::builtin(set))));
    let view = DragHandleView { rect: rect() };
    drag_handle::draw_drag_handle(&mut ctx, rect(), &view, &settings, kind);
    ctx
}

/// `DragHandleRenderKind`'s two built-in variants — drag_handle's entire
/// real state set (module doc).
const STATE_NAMES: &[&str] = &["invisible", "grip_dots"];

fn kind_for(name: &str) -> DragHandleRenderKind {
    match name {
        "invisible" => DragHandleRenderKind::Invisible,
        _ => DragHandleRenderKind::GripDots,
    }
}

#[test]
fn drag_handle_matches_golden_grid() {
    let mut sheet = ReviewSheet::new();
    support::for_each_set(|set, _tokens| {
        for &state_name in STATE_NAMES {
            let kind = kind_for(state_name);
            let ctx = render(set, &kind);
            support::golden("drag_handle", set, state_name, &ctx).expect("drag_handle golden mismatch");
            sheet.add(set, state_name, &ctx);
        }
    });
    sheet.save("drag_handle");
}
