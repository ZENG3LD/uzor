//! Text widget render function.

use crate::input::text::selection::{highlight_rects_single, TextSelection};
use crate::input::text::SelectOverride;
use crate::render::{RenderContext, TextAlign, TextBaseline};
use crate::types::Rect;

use super::settings::TextSettings;
use super::types::{TextOverflow, TextView};

/// Measure the natural width of a text view.
///
/// `w = padding_left + text.len() * 7.0 + padding_right` (mlc convention,
/// matches `chrome::tab_width` fallback). Height is caller-supplied — text
/// fills whatever rect it's given. For row-height defaults, use the parent
/// composite's `item_height` / equivalent.
pub fn measure_text_width(view: &TextView<'_>, settings: &TextSettings) -> f64 {
    let style = settings.style.as_ref();
    style.padding_left() + view.text.len() as f64 * 7.0 + style.padding_right()
}

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
    draw_text_with_selection(ctx, rect, view, settings, None);
}

/// [`draw_text`] plus an optional selection highlight: when `selection` is
/// non-empty and `settings.select` is not [`SelectOverride::Off`], fills
/// the selected chars' boxes (full rect height) in
/// `theme.selection_color()` under the glyphs.
///
/// The selection is in chars of `view.text`; with `Ellipsis` overflow it is
/// clamped to the chars actually drawn. `None` (or an empty selection)
/// issues exactly the calls [`draw_text`] issues. Who owns the selection
/// and whether this label may have one is the caller's business (see
/// [`TextSettings::is_selectable`]).
pub fn draw_text_with_selection(
    ctx:       &mut dyn RenderContext,
    rect:      Rect,
    view:      &TextView<'_>,
    settings:  &TextSettings,
    selection: Option<&TextSelection>,
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

    let highlight = selection
        .filter(|s| !s.is_empty())
        .filter(|_| settings.select != SelectOverride::Off)
        .map(|sel| Highlight { sel, x, align: view.align, rect, fill: theme.selection_color(), text_color: color });

    match view.overflow {
        TextOverflow::Clip | TextOverflow::Wrap => {
            // This atomic widget stays a single-line label by design — real
            // multi-line paragraph wrap lives in `uzor-text` (see
            // `docs/uzor-engines/uzor_text_arc2_design.md` Q2), not here. `Wrap`
            // falls back to clip until/unless a later arc wires this widget
            // to consume a `uzor-text` layout.
            if let Some(h) = &highlight {
                h.fill(ctx, view.text, view.text.chars().count());
            }
            ctx.fill_text(view.text, x, y);
        }
        TextOverflow::Ellipsis => {
            let max_w = rect.width - pad_l - pad_r;
            let full_w = ctx.measure_text(view.text);
            if full_w <= max_w {
                if let Some(h) = &highlight {
                    h.fill(ctx, view.text, view.text.chars().count());
                }
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
                            if let Some(h) = &highlight {
                                h.fill(ctx, &candidate, take);
                            }
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

/// A selection highlight to fill under one drawn string.
struct Highlight<'a> {
    sel:        &'a TextSelection,
    /// The `x` passed to `fill_text`.
    x:          f64,
    align:      TextAlign,
    rect:       Rect,
    fill:       &'a str,
    text_color: &'a str,
}

impl Highlight<'_> {
    /// Fill the rects for the selection (clamped to the first `selectable`
    /// chars of `drawn`) and put the text fill colour back.
    fn fill(&self, ctx: &mut dyn RenderContext, drawn: &str, selectable: usize) {
        let sel = self.sel.clamped(selectable);
        if sel.is_empty() {
            return;
        }
        let boundaries = drawn_boundaries(ctx, drawn, self.x, self.align);
        let rects = highlight_rects_single(&sel, &boundaries, self.rect.y, self.rect.height);
        if rects.is_empty() {
            return;
        }
        ctx.set_fill_color(self.fill);
        for r in rects {
            ctx.fill_rect(r.x, r.y, r.width, r.height);
        }
        ctx.set_fill_color(self.text_color);
    }
}

/// Char boundary x positions of `drawn` as `fill_text(drawn, x, _)` with
/// `align` lays it out, from prefix widths in the current font.
fn drawn_boundaries(ctx: &mut dyn RenderContext, drawn: &str, x: f64, align: TextAlign) -> Vec<f64> {
    let total = ctx.measure_text(drawn);
    let left = match align {
        TextAlign::Left   => x,
        TextAlign::Center => x - total / 2.0,
        TextAlign::Right  => x - total,
    };
    let mut out = Vec::with_capacity(drawn.chars().count() + 1);
    out.push(left);
    for (b, ch) in drawn.char_indices() {
        out.push(left + ctx.measure_text(&drawn[..b + ch.len_utf8()]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{DrawOp, RecordingRenderContext};

    fn record(view: &TextView<'_>, settings: &TextSettings, sel: Option<&TextSelection>) -> Vec<DrawOp> {
        let mut ctx = RecordingRenderContext::with_char_width(10.0);
        draw_text_with_selection(&mut ctx, Rect::new(0.0, 0.0, 200.0, 20.0), view, settings, sel);
        ctx.ops
    }

    fn plain(view: &TextView<'_>, settings: &TextSettings) -> Vec<DrawOp> {
        let mut ctx = RecordingRenderContext::with_char_width(10.0);
        draw_text(&mut ctx, Rect::new(0.0, 0.0, 200.0, 20.0), view, settings);
        ctx.ops
    }

    fn fill_rects(ops: &[DrawOp]) -> Vec<(f64, f64, f64, f64)> {
        ops.iter()
            .filter_map(|op| match op {
                DrawOp::FillRect { x, y, w, h } => Some((*x, *y, *w, *h)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn no_or_empty_selection_draws_exactly_what_draw_text_draws() {
        let settings = TextSettings::default();
        for overflow in [TextOverflow::Clip, TextOverflow::Wrap, TextOverflow::Ellipsis] {
            for align in [TextAlign::Left, TextAlign::Center, TextAlign::Right] {
                let view = TextView { text: "привет world", align, overflow, ..Default::default() };
                let want = plain(&view, &settings);
                assert!(fill_rects(&want).is_empty());
                assert_eq!(record(&view, &settings, None), want);
                assert_eq!(record(&view, &settings, Some(&TextSelection::caret(3))), want);
            }
        }
    }

    #[test]
    fn selection_fills_under_the_glyphs_and_restores_the_text_colour() {
        let settings = TextSettings::default();
        let view = TextView { text: "hello", ..Default::default() };
        let ops = record(&view, &settings, Some(&TextSelection::new(4, 1)));
        let pad = settings.style.padding_left();
        assert_eq!(fill_rects(&ops), vec![(pad + 10.0, 0.0, 30.0, 20.0)]);
        let rect_at = ops.iter().position(|o| matches!(o, DrawOp::FillRect { .. })).unwrap();
        let text_at = ops.iter().position(|o| matches!(o, DrawOp::FillText { .. })).unwrap();
        assert!(rect_at < text_at, "highlight is under the glyphs");
        assert_eq!(ops[rect_at - 1], DrawOp::FillColor(settings.theme.selection_color().to_string()));
        let text_colour = settings.theme.text_color().to_string();
        assert_eq!(ops[rect_at + 1], DrawOp::FillColor(text_colour));
    }

    #[test]
    fn centred_and_right_aligned_selection_follow_the_text() {
        let settings = TextSettings::default();
        let centred = TextView { text: "abcd", align: TextAlign::Center, ..Default::default() };
        // centre x = 100, text 40 wide → starts at 80
        assert_eq!(fill_rects(&record(&centred, &settings, Some(&TextSelection::new(0, 2)))), vec![(80.0, 0.0, 20.0, 20.0)]);
        let right = TextView { text: "abcd", align: TextAlign::Right, ..Default::default() };
        let end = 200.0 - settings.style.padding_right();
        assert_eq!(fill_rects(&record(&right, &settings, Some(&TextSelection::all("abcd")))), vec![(end - 40.0, 0.0, 40.0, 20.0)]);
    }

    #[test]
    fn select_off_suppresses_the_highlight() {
        let settings = TextSettings::default().with_select(SelectOverride::Off);
        let view = TextView { text: "hello", ..Default::default() };
        let ops = record(&view, &settings, Some(&TextSelection::all("hello")));
        assert!(fill_rects(&ops).is_empty());
        assert_eq!(ops, plain(&view, &settings));
    }

    #[test]
    fn ellipsis_clamps_the_selection_to_the_drawn_chars() {
        let settings = TextSettings::default();
        let pad = settings.style.padding_left() + settings.style.padding_right();
        // 30 chars at 10 px never fit in 200 px: some prefix + "…" is drawn
        let text = "abcdefghijklmnopqrstuvwxyz0123";
        let view = TextView { text, overflow: TextOverflow::Ellipsis, ..Default::default() };
        let ops = record(&view, &settings, Some(&TextSelection::all(text)));
        let drawn = ops
            .iter()
            .find_map(|o| match o {
                DrawOp::FillText { text, .. } => Some(text.clone()),
                _ => None,
            })
            .unwrap();
        let kept = drawn.chars().count() - 1;
        assert!(((kept + 1) as f64) * 10.0 <= 200.0 - pad);
        let rects = fill_rects(&ops);
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0].2, kept as f64 * 10.0, "the ellipsis itself is not highlighted");
    }
}
