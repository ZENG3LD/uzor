//! Text widget render function.

use crate::render::{RenderContext, TextAlign, TextBaseline};
use crate::types::Rect;

use super::settings::TextSettings;
use super::types::{TextOverflow, TextView};

/// Draw the text inside `rect` using `view` + `settings`.
///
/// Saves and restores the `RenderContext` text state (font, align, baseline,
/// fill color, clip) so callers don't have to manage those manually.
pub fn draw_text(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    view:     &TextView<'_>,
    settings: &TextSettings,
) {
    let style = settings.style.as_ref();
    let theme = settings.theme.as_ref();

    let color = view.color.unwrap_or_else(|| {
        if view.hovered { theme.text_color_hover() } else { theme.text_color() }
    });
    let font = view.font.unwrap_or_else(|| style.font());

    ctx.save();
    ctx.clip_rect(rect.x, rect.y, rect.width, rect.height);
    ctx.set_fill_color(color);
    ctx.set_font(font);
    ctx.set_text_align(view.align);
    ctx.set_text_baseline(view.baseline);

    let pad_l = style.padding_left();
    let pad_r = style.padding_right();

    let x = match view.align {
        TextAlign::Left   => rect.x + pad_l,
        TextAlign::Center => rect.x + rect.width / 2.0,
        TextAlign::Right  => rect.x + rect.width - pad_r,
    };
    let y = match view.baseline {
        TextBaseline::Top        => rect.y,
        TextBaseline::Middle     => rect.y + rect.height / 2.0,
        TextBaseline::Bottom     => rect.y + rect.height,
        TextBaseline::Alphabetic => rect.y + rect.height / 2.0,
    };

    match view.overflow {
        TextOverflow::Clip | TextOverflow::Wrap => {
            // Wrap not yet implemented — falls back to clip.
            ctx.fill_text(view.text, x, y);
        }
        TextOverflow::Ellipsis => {
            let max_w = rect.width - pad_l - pad_r;
            let full_w = ctx.measure_text(view.text);
            if full_w <= max_w {
                ctx.fill_text(view.text, x, y);
            } else {
                let ell = "\u{2026}"; // "…"
                let ell_w = ctx.measure_text(ell);
                if ell_w < max_w {
                    let chars: Vec<char> = view.text.chars().collect();
                    let mut take = chars.len();
                    while take > 0 {
                        take -= 1;
                        let candidate: String =
                            chars[..take].iter().collect::<String>() + ell;
                        if ctx.measure_text(&candidate) <= max_w {
                            ctx.fill_text(&candidate, x, y);
                            break;
                        }
                    }
                }
                // If even "…" alone doesn't fit, draw nothing — rect too narrow.
            }
        }
    }

    ctx.restore();
}
