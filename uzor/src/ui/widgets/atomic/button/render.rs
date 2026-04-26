//! Button rendering — ported from `mlc/chart/src/ui/widgets/button.rs`.
//!
//! Pure math: bg → optional active border → icon → text. Variant-specific
//! flourishes (chevron for `Dropdown`, color square for `ColorSwatch`,
//! checkmark for `Checkbox`, etc.) are added in subsequent passes via
//! variant-specific helpers; this base is the shared rect+icon+text code.

use crate::render::{RenderContext, TextAlign, TextBaseline};
use crate::types::{IconId, Rect, WidgetState};

use super::settings::ButtonSettings;

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
            (theme.button_bg_active(), theme.button_text_active())
        }
        WidgetState::Normal => {
            if view.active {
                (theme.button_bg_active(), theme.button_text_active())
            } else {
                (theme.button_bg_normal(), theme.button_text_normal())
            }
        }
    };

    let radius = style.radius();

    // Background
    ctx.set_fill_color(bg);
    ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, radius);

    // Active border (opt-in via style)
    if view.active && style.show_active_border() {
        // No dedicated `button_border_active` slot — fall back to focused border.
        ctx.set_stroke_color(theme.button_border_focused());
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
