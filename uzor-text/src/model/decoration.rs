//! [`TextDecoration`]/[`VerticalAlign`] — typed per-run paint attributes
//! (typography-gap WAVE 2). Both default to "nothing" so every pre-wave
//! [`crate::model::StyledRun`] caller is unaffected.

/// Underline/strikethrough flags for one [`crate::model::StyledRun`].
/// Default: neither (`TextDecoration::NONE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextDecoration {
    pub underline: bool,
    pub strikethrough: bool,
}

impl TextDecoration {
    /// No decoration — same as [`Default::default`], named for use in a
    /// `const` context (`Default::default()` is not `const fn`).
    pub const NONE: Self = Self { underline: false, strikethrough: false };

    /// Underline only.
    pub const fn underline() -> Self {
        Self { underline: true, strikethrough: false }
    }

    /// Strikethrough only.
    pub const fn strikethrough() -> Self {
        Self { underline: false, strikethrough: true }
    }

    /// `true` when neither flag is set — [`crate::layout::paragraph`]'s
    /// decoration-span builder uses this to skip a run without emitting a
    /// dangling in-progress span.
    pub const fn is_none(&self) -> bool {
        !self.underline && !self.strikethrough
    }
}

/// Baseline shift + scale for one [`crate::model::StyledRun`] — sub/
/// superscript. Default: [`VerticalAlign::Baseline`] (no shift, no scale).
///
/// Shift/scale ratios are **sane fixed fallbacks**, not font-derived
/// (`superscriptYOffset`/`subscriptYOffset`/`superscriptYSize` in a real
/// font's OS/2 table) — `uzor-text`'s own dependency law forbids reaching
/// into `uzor-export`'s TTF metrics reader from production code (that
/// reader is `uzor-export`-only, and `uzor-text` may only depend on it as a
/// dev-dependency — see this crate's `CLAUDE.md`), so there is no in-crate
/// path to a real font's own script metrics. The ratios below match the
/// common CSS/OpenType convention closely enough to read correctly in every
/// embedded font this workspace ships.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VerticalAlign {
    #[default]
    Baseline,
    /// Raised above the baseline by [`SUPERSCRIPT_SHIFT_EM`] em, shaped at
    /// [`SCRIPT_SCALE`] of the run's own `font.size_px`.
    Super,
    /// Lowered below the baseline by [`SUBSCRIPT_SHIFT_EM`] em, shaped at
    /// [`SCRIPT_SCALE`] of the run's own `font.size_px`.
    Sub,
}

/// How much smaller a [`VerticalAlign::Super`]/[`VerticalAlign::Sub`] run
/// shapes relative to its own nominal `font.size_px` — applied to the
/// FONT SIZE used for shaping (a genuinely smaller glyph, not a squashed
/// paint-time transform), matching real subscript/superscript typography.
pub const SCRIPT_SCALE: f64 = 0.65;

/// Upward baseline shift for [`VerticalAlign::Super`], in em (fraction of
/// the run's own unscaled `font.size_px`).
pub const SUPERSCRIPT_SHIFT_EM: f64 = 0.58;

/// Downward baseline shift for [`VerticalAlign::Sub`], in em (fraction of
/// the run's own unscaled `font.size_px`).
pub const SUBSCRIPT_SHIFT_EM: f64 = 0.21;

impl VerticalAlign {
    /// The font actually used to SHAPE a run under this vertical-align
    /// (design law 1: [`crate::layout::glyph_layout::GlyphLayout::font`]
    /// always carries the font glyphs were genuinely shaped/measured at,
    /// never the run's own nominal font, so a downstream painter never
    /// requests a mismatched size).
    pub(crate) fn shape_font(self, font: crate::model::FontSpec) -> crate::model::FontSpec {
        match self {
            VerticalAlign::Baseline => font,
            VerticalAlign::Super | VerticalAlign::Sub => {
                crate::model::FontSpec { size_px: font.size_px * SCRIPT_SCALE, ..font }
            }
        }
    }

    /// Baseline-relative y shift (paragraph y grows downward — a NEGATIVE
    /// shift raises the glyph, a POSITIVE shift lowers it), computed from
    /// `nominal_size_px` (the run's own UNSCALED `font.size_px` — em-ratios
    /// are defined against the nominal size, not the already-shrunk shape
    /// font, matching standard OpenType script-metric convention).
    pub(crate) fn baseline_shift(self, nominal_size_px: f64) -> f64 {
        match self {
            VerticalAlign::Baseline => 0.0,
            VerticalAlign::Super => -(SUPERSCRIPT_SHIFT_EM * nominal_size_px),
            VerticalAlign::Sub => SUBSCRIPT_SHIFT_EM * nominal_size_px,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_decoration_default_is_none() {
        let d = TextDecoration::default();
        assert!(d.is_none());
        assert_eq!(d, TextDecoration::NONE);
    }

    #[test]
    fn text_decoration_constructors_set_exactly_one_flag() {
        assert_eq!(TextDecoration::underline(), TextDecoration { underline: true, strikethrough: false });
        assert_eq!(TextDecoration::strikethrough(), TextDecoration { underline: false, strikethrough: true });
        assert!(!TextDecoration::underline().is_none());
    }

    #[test]
    fn vertical_align_default_is_baseline_with_no_shift_and_nominal_font() {
        let font = crate::model::FontSpec::new(uzor::fonts::FontFamily::Roboto, 16.0);
        assert_eq!(VerticalAlign::default(), VerticalAlign::Baseline);
        assert_eq!(VerticalAlign::Baseline.baseline_shift(16.0), 0.0);
        assert_eq!(VerticalAlign::Baseline.shape_font(font), font);
    }

    #[test]
    fn superscript_shifts_up_and_shrinks_the_shape_font() {
        let font = crate::model::FontSpec::new(uzor::fonts::FontFamily::Roboto, 20.0);
        assert!(VerticalAlign::Super.baseline_shift(20.0) < 0.0, "superscript must raise (negative y shift)");
        let shaped = VerticalAlign::Super.shape_font(font);
        assert!((shaped.size_px - 20.0 * SCRIPT_SCALE).abs() < 1e-9);
    }

    #[test]
    fn subscript_shifts_down_and_shrinks_the_shape_font() {
        let font = crate::model::FontSpec::new(uzor::fonts::FontFamily::Roboto, 20.0);
        assert!(VerticalAlign::Sub.baseline_shift(20.0) > 0.0, "subscript must lower (positive y shift)");
        let shaped = VerticalAlign::Sub.shape_font(font);
        assert!((shaped.size_px - 20.0 * SCRIPT_SCALE).abs() < 1e-9);
    }
}
