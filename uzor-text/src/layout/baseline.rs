//! Mixed-run baseline pass — resolves one shared ascent/descent (and
//! therefore one shared baseline) for a visual line that may contain runs
//! of different font sizes and/or an [`crate::model::InlineBox`] taller
//! than the surrounding text.
//!
//! Applied by [`crate::layout::layout_paragraph`] after wrap (line
//! membership is already decided), before final `y` assignment on each
//! `GlyphLayout` — design doc §4 Phase 2: "per-line baseline resolution
//! across mixed-size runs (max-ascent/max-descent per line ...), applied
//! after wrap, before final `y` assignment."

/// Resolved vertical metrics for one visual line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LineMetrics {
    /// Distance from the line's top to its one shared baseline.
    pub ascent: f64,
    /// Distance from the shared baseline to the line's bottom.
    pub descent: f64,
    /// Total line height: `ascent + descent`, or the paragraph's
    /// `line_height` override when one is set.
    pub height: f64,
}

/// Fold per-atom `(ascent, descent)` pairs into one shared [`LineMetrics`]
/// — "max-ascent/max-descent per line" (design doc §4 Phase 2).
///
/// `line_height_override` (`Paragraph::line_height`) replaces the natural
/// `ascent + descent` sum outright when `Some` ("line height = max over
/// runs, or explicit override").
pub(crate) fn resolve_line_metrics<I>(metrics: I, line_height_override: Option<f64>) -> LineMetrics
where
    I: IntoIterator<Item = (f64, f64)>,
{
    let (mut ascent, mut descent) = (0.0_f64, 0.0_f64);
    for (a, d) in metrics {
        ascent = ascent.max(a);
        descent = descent.max(d);
    }
    let height = line_height_override.unwrap_or(ascent + descent);
    LineMetrics { ascent, descent, height }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_to_the_max_ascent_and_max_descent_across_runs() {
        let metrics = resolve_line_metrics([(12.0, 4.0), (20.0, 3.0), (10.0, 6.0)], None);
        assert_eq!(metrics.ascent, 20.0);
        assert_eq!(metrics.descent, 6.0);
        assert_eq!(metrics.height, 26.0);
    }

    #[test]
    fn empty_input_folds_to_zero() {
        let metrics = resolve_line_metrics(std::iter::empty(), None);
        assert_eq!(metrics.ascent, 0.0);
        assert_eq!(metrics.descent, 0.0);
        assert_eq!(metrics.height, 0.0);
    }

    #[test]
    fn explicit_override_replaces_the_natural_sum_outright() {
        let metrics = resolve_line_metrics([(12.0, 4.0), (20.0, 3.0)], Some(50.0));
        assert_eq!(metrics.ascent, 20.0);
        assert_eq!(metrics.descent, 4.0);
        assert_eq!(metrics.height, 50.0, "override replaces the natural 24.0 sum outright");
    }
}
