//! Golden grid (H1 Brief 10a-2): item × 4 built-in sets × its own real
//! variant set. `item` is non-interactive (`Sense::NONE`, module doc) — it
//! has no hover/disabled states — so the only real state axis is the
//! `ItemStyle` preset: `DefaultItemStyle` ("normal", 13px font, 4px
//! icon/text gap) vs `ToolbarItemStyle` ("toolbar", 11px font, 2px gap).
//! Rendered via `ItemRenderKind::TextIcon` (icon + label together) so both
//! the font-size and gap differences are visible; the icon closure paints a
//! plain filled square in `theme.item_text()` (this suite has no real icon
//! atlas to draw from — a coloured placeholder box is enough to prove
//! geometry and colour, same approach `button`'s golden takes with its own
//! icon closure).

#![cfg(feature = "golden")]

use uzor::tokens::{BuiltinSet, TokenTheme, Tokens};
use uzor::types::{IconId, Rect, WidgetState};
use uzor::ui::widgets::atomic::item::{self, ItemRenderKind, ItemSettings, ItemView, ToolbarItemStyle};
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

use super::support::{self, ReviewSheet};

fn rect() -> Rect {
    Rect::new(4.0, 4.0, 120.0, 24.0)
}

fn render(set: BuiltinSet, toolbar: bool) -> TinySkiaCpuRenderContext {
    let mut ctx = support::canvas(128, 32, set);
    let theme = Box::new(TokenTheme::new(Tokens::builtin(set)));
    let settings = if toolbar {
        ItemSettings::default().with_theme(theme).with_style(Box::new(ToolbarItemStyle))
    } else {
        ItemSettings::default().with_theme(theme)
    };
    let icon = IconId::new("demo");
    let view = ItemView { label: Some("Label"), icon: Some(&icon), svg: None };
    item::draw_item(
        &mut ctx,
        rect(),
        WidgetState::Normal,
        &view,
        &settings,
        &ItemRenderKind::TextIcon,
        |ctx, _icon, icon_rect, color| {
            ctx.set_fill_color(color);
            ctx.fill_rect(icon_rect.x, icon_rect.y, icon_rect.width, icon_rect.height);
        },
        |_, _, _, _| {},
    );
    ctx
}

/// `(state name, toolbar)` — item's entire real style-variant set (module doc).
const STATES: &[(&str, bool)] = &[("normal", false), ("toolbar", true)];

#[test]
fn item_matches_golden_grid() {
    let mut sheet = ReviewSheet::new();
    support::for_each_set(|set, _tokens| {
        for &(state_name, toolbar) in STATES {
            let ctx = render(set, toolbar);
            support::golden("item", set, state_name, &ctx).expect("item golden mismatch");
            sheet.add(set, state_name, &ctx);
        }
    });
    sheet.save("item");
}
