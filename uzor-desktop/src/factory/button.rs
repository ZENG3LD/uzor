//! Button factory rendering for desktop
//!
//! Adapted from terminal button factory to use RenderContext

use super::{RenderContext, TextAlign, TextBaseline};
use uzor_core::types::{Rect, WidgetState, IconId};
use uzor_core::widgets::button::types::{
    ButtonType,
    ActionVariant, ToggleVariant, CheckboxVariant, TabVariant, ColorSwatchVariant, DropdownVariant,
    ButtonStyle,
};
use uzor_core::widgets::button::theme::ButtonTheme;
use uzor_core::widgets::button::defaults::*;

/// Main entry point for rendering a button with default settings
pub fn render_default<F>(
    ctx: &mut dyn RenderContext,
    button: &ButtonType,
    rect: Rect,
    state: WidgetState,
    theme: &dyn ButtonTheme,
    draw_icon: F,
) where
    F: Fn(&mut dyn RenderContext, &IconId, Rect, &str),
{
    match button {
        ButtonType::Action { variant, .. } => render_action(ctx, variant, rect, state, theme, draw_icon),
        ButtonType::Toggle { variant, .. } => render_toggle(ctx, variant, rect, state, theme, draw_icon),
        ButtonType::Checkbox { variant, .. } => render_checkbox(ctx, variant, rect, state, theme),
        ButtonType::Tab { variant, .. } => render_tab(ctx, variant, rect, state, theme, draw_icon),
        ButtonType::ColorSwatch { variant, .. } => render_colorswatch(ctx, variant, rect, state, theme, draw_icon),
        ButtonType::Dropdown { variant, .. } => render_dropdown(ctx, variant, rect, state, theme, draw_icon),
    }
}

// =============================================================================
// Action Variants
// =============================================================================

fn render_action<F>(
    ctx: &mut dyn RenderContext,
    variant: &ActionVariant,
    rect: Rect,
    state: WidgetState,
    theme: &dyn ButtonTheme,
    draw_icon: F,
) where
    F: Fn(&mut dyn RenderContext, &IconId, Rect, &str),
{
    match variant {
        ActionVariant::IconOnly { icon, disabled } => {
            let defaults = IconOnlyDefaults::default();
            let effective_state = if *disabled { WidgetState::Disabled } else { state };

            let icon_color = match effective_state {
                WidgetState::Normal => theme.button_icon_normal(),
                WidgetState::Hovered => theme.button_icon_hover(),
                WidgetState::Pressed => theme.button_icon_active(),
                WidgetState::Active | WidgetState::Toggled => theme.button_icon_active(),
                WidgetState::Disabled => theme.button_icon_disabled(),
            };

            // Optional hover background
            if matches!(effective_state, WidgetState::Hovered | WidgetState::Pressed) {
                ctx.set_fill_color(theme.button_bg_hover());
                ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, defaults.hover_bg_radius);
            }

            // Draw icon centered
            let icon_rect = Rect::new(
                rect.center_x() - defaults.icon_size / 2.0,
                rect.center_y() - defaults.icon_size / 2.0,
                defaults.icon_size,
                defaults.icon_size,
            );
            draw_icon(ctx, icon, icon_rect, icon_color);
        }

        ActionVariant::Text { text, style, disabled } => {
            let defaults = TextDefaults::default();
            let effective_state = if *disabled { WidgetState::Disabled } else { state };

            let (bg_color, text_color, border_color) = match effective_state {
                WidgetState::Disabled => (
                    theme.button_bg_disabled(),
                    theme.button_text_disabled(),
                    theme.button_border_normal(),
                ),
                WidgetState::Pressed | WidgetState::Active => (
                    theme.button_bg_pressed(),
                    theme.button_text_active(),
                    theme.button_border_focused(),
                ),
                WidgetState::Hovered => (
                    theme.button_bg_hover(),
                    theme.button_text_hover(),
                    theme.button_border_hover(),
                ),
                WidgetState::Normal | WidgetState::Toggled => match style {
                    ButtonStyle::Primary => (
                        theme.button_accent(),
                        theme.button_text_active(),
                        theme.button_accent(),
                    ),
                    ButtonStyle::Danger => (
                        theme.button_bg_normal(),
                        theme.button_danger(),
                        theme.button_danger(),
                    ),
                    ButtonStyle::Ghost => (
                        "transparent",
                        theme.button_text_normal(),
                        "transparent",
                    ),
                    ButtonStyle::Default => (
                        theme.button_bg_normal(),
                        theme.button_text_normal(),
                        theme.button_border_normal(),
                    ),
                },
            };

            // Draw background
            ctx.set_fill_color(bg_color);
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, defaults.border_radius);

            // Draw border
            if border_color != "transparent" {
                ctx.set_stroke_color(border_color);
                ctx.set_stroke_width(defaults.border_width);
                ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, defaults.border_radius);
            }

            // Draw text centered
            ctx.set_fill_color(text_color);
            ctx.set_font(&format!("{}px sans-serif", defaults.font_size));
            ctx.set_text_align(TextAlign::Center);
            ctx.set_text_baseline(TextBaseline::Middle);
            ctx.fill_text(text, rect.center_x(), rect.center_y());
        }

        ActionVariant::IconText { icon, text, style, disabled } => {
            let defaults = IconTextDefaults::default();
            let effective_state = if *disabled { WidgetState::Disabled } else { state };

            let (bg_color, text_color, icon_color) = match effective_state {
                WidgetState::Disabled => (
                    theme.button_bg_disabled(),
                    theme.button_text_disabled(),
                    theme.button_icon_disabled(),
                ),
                WidgetState::Pressed | WidgetState::Active => (
                    theme.button_bg_pressed(),
                    theme.button_text_active(),
                    theme.button_icon_active(),
                ),
                WidgetState::Hovered => match style {
                    ButtonStyle::Primary => (
                        theme.button_accent(),
                        theme.button_text_active(),
                        theme.button_icon_active(),
                    ),
                    _ => (
                        theme.button_bg_hover(),
                        theme.button_text_hover(),
                        theme.button_icon_hover(),
                    ),
                },
                WidgetState::Normal | WidgetState::Toggled => match style {
                    ButtonStyle::Primary => (
                        theme.button_accent(),
                        theme.button_text_active(),
                        theme.button_icon_active(),
                    ),
                    _ => (
                        theme.button_bg_normal(),
                        theme.button_text_normal(),
                        theme.button_icon_normal(),
                    ),
                },
            };

            // Draw background
            ctx.set_fill_color(bg_color);
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, defaults.border_radius);

            // Calculate layout for icon + text
            let content_width = defaults.icon_size + defaults.icon_text_gap + (text.len() as f64 * defaults.font_size * 0.6);
            let start_x = rect.center_x() - content_width / 2.0;

            // Draw icon
            let icon_rect = Rect::new(
                start_x,
                rect.center_y() - defaults.icon_size / 2.0,
                defaults.icon_size,
                defaults.icon_size,
            );
            draw_icon(ctx, icon, icon_rect, icon_color);

            // Draw text
            ctx.set_fill_color(text_color);
            ctx.set_font(&format!("{}px sans-serif", defaults.font_size));
            ctx.set_text_align(TextAlign::Left);
            ctx.set_text_baseline(TextBaseline::Middle);
            ctx.fill_text(text, start_x + defaults.icon_size + defaults.icon_text_gap, rect.center_y());
        }

        _ => {
            // Stub for other action variants
            ctx.set_fill_color(theme.button_bg_normal());
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, 4.0);
        }
    }
}

// =============================================================================
// Toggle Variants
// =============================================================================

fn render_toggle<F>(
    ctx: &mut dyn RenderContext,
    variant: &ToggleVariant,
    rect: Rect,
    state: WidgetState,
    theme: &dyn ButtonTheme,
    draw_icon: F,
) where
    F: Fn(&mut dyn RenderContext, &IconId, Rect, &str),
{
    match variant {
        ToggleVariant::IconSwap { icon_off, icon_on, toggled } => {
            let defaults = IconSwapDefaults::default();

            let icon_color = match state {
                WidgetState::Disabled => theme.button_icon_disabled(),
                WidgetState::Pressed | WidgetState::Active => theme.button_icon_active(),
                WidgetState::Hovered => theme.button_icon_hover(),
                WidgetState::Normal | WidgetState::Toggled => theme.button_icon_normal(),
            };

            let icon = if *toggled { icon_on } else { icon_off };

            let icon_rect = Rect::new(
                rect.center_x() - defaults.icon_size / 2.0,
                rect.center_y() - defaults.icon_size / 2.0,
                defaults.icon_size,
                defaults.icon_size,
            );
            draw_icon(ctx, icon, icon_rect, icon_color);
        }

        _ => {
            // Stub for other toggle variants
            ctx.set_fill_color(theme.button_bg_normal());
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, 4.0);
        }
    }
}

// =============================================================================
// Checkbox Variants
// =============================================================================

fn render_checkbox(
    ctx: &mut dyn RenderContext,
    variant: &CheckboxVariant,
    rect: Rect,
    state: WidgetState,
    theme: &dyn ButtonTheme,
) {
    match variant {
        CheckboxVariant::Standard { checked } => {
            let defaults = CheckboxDefaults::default();

            let (bg_color, border_color, check_color) = if *checked {
                match state {
                    WidgetState::Hovered => (
                        theme.button_accent(),
                        theme.button_accent(),
                        theme.button_text_active(),
                    ),
                    WidgetState::Disabled => (
                        theme.button_bg_disabled(),
                        theme.button_border_normal(),
                        theme.button_text_disabled(),
                    ),
                    _ => (
                        theme.button_accent(),
                        theme.button_accent(),
                        theme.button_text_active(),
                    ),
                }
            } else {
                match state {
                    WidgetState::Hovered => (
                        theme.button_bg_hover(),
                        theme.button_border_hover(),
                        theme.button_text_active(),
                    ),
                    WidgetState::Disabled => (
                        theme.button_bg_disabled(),
                        theme.button_border_normal(),
                        theme.button_text_disabled(),
                    ),
                    _ => (
                        "transparent",
                        theme.button_border_normal(),
                        theme.button_text_active(),
                    ),
                }
            };

            let checkbox_x = rect.center_x() - defaults.checkbox_size / 2.0;
            let checkbox_y = rect.center_y() - defaults.checkbox_size / 2.0;

            ctx.set_fill_color(bg_color);
            ctx.fill_rounded_rect(checkbox_x, checkbox_y, defaults.checkbox_size, defaults.checkbox_size, defaults.border_radius);

            ctx.set_stroke_color(border_color);
            ctx.set_stroke_width(defaults.border_width);
            ctx.stroke_rounded_rect(checkbox_x, checkbox_y, defaults.checkbox_size, defaults.checkbox_size, defaults.border_radius);

            if *checked {
                ctx.set_stroke_color(check_color);
                ctx.set_stroke_width(2.0);
                let check_x = checkbox_x + 4.0;
                let check_y = checkbox_y + 8.0;
                ctx.begin_path();
                ctx.move_to(check_x, check_y);
                ctx.line_to(check_x + 3.0, check_y + 3.0);
                ctx.line_to(check_x + 8.0, check_y - 4.0);
                ctx.stroke();
            }
        }

        _ => {
            // Stub for other checkbox variants
            ctx.set_fill_color(theme.button_bg_normal());
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, 4.0);
        }
    }
}

// =============================================================================
// Tab Variants
// =============================================================================

fn render_tab<F>(
    ctx: &mut dyn RenderContext,
    variant: &TabVariant,
    rect: Rect,
    state: WidgetState,
    theme: &dyn ButtonTheme,
    draw_icon: F,
) where
    F: Fn(&mut dyn RenderContext, &IconId, Rect, &str),
{
    match variant {
        TabVariant::Vertical { label, icon, active } => {
            let defaults = VerticalTabDefaults::default();

            let (bg_color, content_color) = if *active {
                (theme.button_bg_active(), theme.button_icon_active())
            } else {
                match state {
                    WidgetState::Hovered => (theme.button_bg_hover(), theme.button_icon_hover()),
                    _ => ("transparent", theme.button_icon_normal()),
                }
            };

            ctx.set_fill_color(bg_color);
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, 0.0);

            if *active {
                ctx.set_fill_color(theme.button_accent());
                ctx.fill_rounded_rect(rect.x, rect.y, defaults.active_bar_width, rect.height, 0.0);
            }

            if let Some(ref icon_id) = icon {
                let icon_rect = Rect::new(
                    rect.center_x() - defaults.icon_size / 2.0,
                    rect.center_y() - defaults.icon_size / 2.0,
                    defaults.icon_size,
                    defaults.icon_size,
                );
                draw_icon(ctx, icon_id, icon_rect, content_color);
            }

            if let Some(ref text) = label {
                ctx.set_fill_color(content_color);
                ctx.set_font(&format!("{}px sans-serif", 10.0));
                ctx.set_text_align(TextAlign::Center);
                ctx.set_text_baseline(TextBaseline::Top);
                ctx.fill_text(text, rect.center_x(), rect.center_y() + defaults.icon_size / 2.0 + 4.0);
            }
        }

        _ => {
            // Stub for horizontal tabs
            ctx.set_fill_color(theme.button_bg_normal());
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, 4.0);
        }
    }
}

// =============================================================================
// ColorSwatch Variants
// =============================================================================

fn render_colorswatch<F>(
    ctx: &mut dyn RenderContext,
    variant: &ColorSwatchVariant,
    rect: Rect,
    state: WidgetState,
    theme: &dyn ButtonTheme,
    _draw_icon: F,
) where
    F: Fn(&mut dyn RenderContext, &IconId, Rect, &str),
{
    match variant {
        ColorSwatchVariant::Square { color } => {
            let defaults = ColorSwatchSquareDefaults::default();

            let border_color = match state {
                WidgetState::Pressed => theme.button_accent(),
                WidgetState::Hovered => theme.button_border_hover(),
                _ => theme.button_border_normal(),
            };

            let swatch_x = rect.center_x() - defaults.swatch_size / 2.0;
            let swatch_y = rect.center_y() - defaults.swatch_size / 2.0;

            ctx.set_fill_color(color);
            ctx.fill_rounded_rect(swatch_x, swatch_y, defaults.swatch_size, defaults.swatch_size, defaults.border_radius);

            ctx.set_stroke_color(border_color);
            ctx.set_stroke_width(if matches!(state, WidgetState::Pressed | WidgetState::Hovered) { 2.0 } else { defaults.border_width });
            ctx.stroke_rounded_rect(swatch_x, swatch_y, defaults.swatch_size, defaults.swatch_size, defaults.border_radius);
        }

        _ => {
            // Stub for other color swatch variants
            ctx.set_fill_color(theme.button_bg_normal());
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, 4.0);
        }
    }
}

// =============================================================================
// Dropdown Variants
// =============================================================================

fn render_dropdown<F>(
    ctx: &mut dyn RenderContext,
    variant: &DropdownVariant,
    rect: Rect,
    state: WidgetState,
    theme: &dyn ButtonTheme,
    draw_icon: F,
) where
    F: Fn(&mut dyn RenderContext, &IconId, Rect, &str),
{
    match variant {
        DropdownVariant::TextChevron { current_label, .. } => {
            let defaults = DropdownTextChevronDefaults::default();

            let (bg_color, text_color, border_color) = match state {
                WidgetState::Pressed => (
                    theme.button_bg_pressed(),
                    theme.button_text_active(),
                    theme.button_border_focused(),
                ),
                WidgetState::Hovered => (
                    theme.button_bg_hover(),
                    theme.button_text_hover(),
                    theme.button_border_hover(),
                ),
                _ => (
                    theme.button_bg_normal(),
                    theme.button_text_normal(),
                    theme.button_border_normal(),
                ),
            };

            ctx.set_fill_color(bg_color);
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, defaults.border_radius);

            ctx.set_stroke_color(border_color);
            ctx.set_stroke_width(defaults.border_width);
            ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, defaults.border_radius);

            ctx.set_fill_color(text_color);
            ctx.set_font(&format!("{}px sans-serif", 13.0));
            ctx.set_text_align(TextAlign::Left);
            ctx.set_text_baseline(TextBaseline::Middle);
            ctx.fill_text(current_label, rect.x + defaults.text_padding_x, rect.center_y());

            let chevron_icon = IconId::new("chevron_down");
            let chevron_rect = Rect::new(
                rect.x + rect.width - defaults.chevron_area_width + (defaults.chevron_area_width - defaults.chevron_size) / 2.0,
                rect.center_y() - defaults.chevron_size / 2.0,
                defaults.chevron_size,
                defaults.chevron_size,
            );
            draw_icon(ctx, &chevron_icon, chevron_rect, text_color);
        }

        _ => {
            // Stub for other dropdown variants
            ctx.set_fill_color(theme.button_bg_normal());
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, 4.0);
        }
    }
}
