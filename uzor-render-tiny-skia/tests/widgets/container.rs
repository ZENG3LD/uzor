//! Golden grid (H1 Brief 10a-2): container × 4 built-in sets × its
//! toolbar-slot / panel-slot variants plus a section header. `container`
//! itself is non-interactive (each `ContainerType` dispatches to a
//! dedicated draw fn with no hover/pressed/disabled axis — `theme.rs`
//! module doc: "Two parallel mlc theme families are bridged here"), so the
//! state axis tested here is the `Panel` role (toolbar/sidebar/status_bar,
//! `draw_panel_container`) plus `Section` (`draw_section_container`, the
//! header-strip variant).
//!
//! NOTE: `ContainerTokens`' own module doc + `builtin.rs`'s
//! `surface_header_and_panel_alias_surface_floating_in_every_set` test both
//! confirm `section_header_bg` (`surface.header`) deliberately aliases `bg`
//! (`surface.floating`) in every built-in set — this is an intentional H1
//! token-contract consolidation, not a bug. Its consequence for this
//! golden: the "section" cell's header strip is colour-identical to its own
//! body in all 4 sets, so no header boundary is visible in any of them.
//! This is a genuine (if already-decided) token-driven readability
//! limitation of the `Section` container variant — flagged per this
//! brief's review instructions, not fixed here (never a token change from a
//! test).

#![cfg(feature = "golden")]

use uzor::tokens::{BuiltinSet, TokenTheme, Tokens};
use uzor::types::Rect;
use uzor::ui::widgets::atomic::container::{
    self, ContainerTheme, PanelContainerStyle, PanelRole, SectionContainerStyle,
};
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

use super::support::{self, ReviewSheet};

#[derive(Clone, Copy)]
enum Variant {
    Toolbar,
    Sidebar,
    StatusBar,
    Section,
}

fn rect() -> Rect {
    Rect::new(8.0, 8.0, 184.0, 64.0)
}

fn render(set: BuiltinSet, variant: Variant) -> TinySkiaCpuRenderContext {
    let mut ctx = support::canvas(200, 80, set);
    let theme: Box<dyn ContainerTheme> = Box::new(TokenTheme::new(Tokens::builtin(set)));
    match variant {
        Variant::Toolbar => container::draw_panel_container(
            &mut ctx,
            rect(),
            theme.as_ref(),
            &PanelContainerStyle::default(),
            PanelRole::Toolbar,
        ),
        Variant::Sidebar => container::draw_panel_container(
            &mut ctx,
            rect(),
            theme.as_ref(),
            &PanelContainerStyle::default(),
            PanelRole::Sidebar,
        ),
        Variant::StatusBar => container::draw_panel_container(
            &mut ctx,
            rect(),
            theme.as_ref(),
            &PanelContainerStyle::default(),
            PanelRole::StatusBar,
        ),
        Variant::Section => container::draw_section_container(
            &mut ctx,
            rect(),
            theme.as_ref(),
            &SectionContainerStyle::default(),
        ),
    }
    ctx
}

/// `(state name, Variant)` — container's toolbar-slot/panel-slot variants
/// plus a section header (module doc).
const STATES: &[(&str, Variant)] = &[
    ("toolbar", Variant::Toolbar),
    ("sidebar", Variant::Sidebar),
    ("status_bar", Variant::StatusBar),
    ("section", Variant::Section),
];

#[test]
fn container_matches_golden_grid() {
    let mut sheet = ReviewSheet::new();
    support::for_each_set(|set, _tokens| {
        for &(state_name, variant) in STATES {
            let ctx = render(set, variant);
            support::golden("container", set, state_name, &ctx).expect("container golden mismatch");
            sheet.add(set, state_name, &ctx);
        }
    });
    sheet.save("container");
}
