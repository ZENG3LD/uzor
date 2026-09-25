//! Golden grid (H1 Brief 10a-composites): blackbox_panel × 4 built-in sets ×
//! its `WithHeaderBorder` kind — the "header + body frame" composition
//! (module doc: "Header strip (title + close-X chrome) above body rect" +
//! "1 px border around full rect", `render.rs` module doc). `BlackboxView::
//! title` is the widget's whole meaningful visual axis for a fixed-content
//! golden: a titled header and an untitled header (blank strip, no text) are
//! the "1-2 states" this grid needs — the body closure itself never varies
//! (a small centred square painted in the panel's own `divider` colour,
//! standing in for arbitrary caller-drawn canvas content — `BlackboxView::
//! body` never receives `&mut InputCoordinator`, module doc). Deterministic:
//! no wall clock, no randomness.

#![cfg(feature = "golden")]

use uzor::render::RenderContext;
use uzor::tokens::{BuiltinSet, Tokens, TokenTheme};
use uzor::types::Rect;
use uzor::ui::widgets::composite::blackbox_panel::{
    self, BlackboxEventResult, BlackboxPanelSettings, BlackboxRenderKind, BlackboxTheme,
    BlackboxView, DefaultBlackboxStyle,
};
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

use super::support::{self, ReviewSheet};

fn panel_rect() -> Rect {
    Rect::new(8.0, 8.0, 224.0, 144.0)
}

/// `(state name, title)` — `BlackboxView::title` is the widget's whole
/// meaningful visual axis for a fixed header+border composition (module doc).
const STATES: &[(&str, Option<&str>)] = &[("titled", Some("Chart View")), ("untitled", None)];

fn render(set: BuiltinSet, title: Option<&str>) -> TinySkiaCpuRenderContext {
    let mut ctx = support::canvas(240, 160, set);
    let theme = TokenTheme::new(Tokens::builtin(set));
    // Placeholder body content: a centred square in the panel's own divider
    // colour, standing in for arbitrary caller-drawn canvas content.
    let placeholder = theme.divider().to_string();
    let settings = BlackboxPanelSettings {
        theme: Box::new(theme),
        style: Box::<DefaultBlackboxStyle>::default(),
    };
    let mut view = BlackboxView {
        title,
        body: Box::new(move |body_ctx: &mut dyn RenderContext, body_rect: Rect| {
            let size = 40.0;
            let x = body_rect.x + (body_rect.width - size) / 2.0;
            let y = body_rect.y + (body_rect.height - size) / 2.0;
            body_ctx.set_fill_color(&placeholder);
            body_ctx.fill_rect(x, y, size, size);
        }),
        handle_event: Box::new(|_event| BlackboxEventResult::NotConsumed),
        sense: uzor::input::Sense::CLICK,
    };
    blackbox_panel::draw_blackbox(
        &mut ctx,
        panel_rect(),
        &mut view,
        &settings,
        &BlackboxRenderKind::WithHeaderBorder,
    );
    ctx
}

#[test]
fn blackbox_panel_matches_golden_grid() {
    let mut sheet = ReviewSheet::new();
    support::for_each_set(|set, _tokens| {
        for &(state_name, title) in STATES {
            let ctx = render(set, title);
            support::golden("blackbox_panel", set, state_name, &ctx)
                .expect("blackbox_panel golden mismatch");
            sheet.add(set, state_name, &ctx);
        }
    });
    sheet.save("blackbox_panel");
}
