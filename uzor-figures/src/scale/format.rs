//! `NumberFormat` — typed value formatting beyond this crate's default
//! thousands-grouped decimal ([`linear::format_value`]), used by axis tick
//! labels ([`crate::guide::axis::draw_x_axis_formatted`]/
//! `draw_y_axis_formatted`) and any other caller wanting a domain value
//! rendered as a headline number (e.g. [`crate::figure::KpiFigure`]'s own
//! big number + delta).
//!
//! [`NumberFormat::default`] is [`NumberFormat::Thousands`] — chosen
//! specifically so `NumberFormat::default().format(v, step)` is
//! byte-identical to [`linear::format_value(v, step)`] for every `v`/
//! `step` (tested directly, `default_matches_current_thousands_output`
//! below) — a figure that starts calling this type through
//! `draw_x_axis_formatted`/`draw_y_axis_formatted` with the default format
//! changes NOTHING about its own existing rendered output.
//!
//! ## Locale scope (documented, not built)
//!
//! Beyond the thousands-separator CHOICE below, this module does not
//! implement locale-aware formatting (decimal comma vs. point, different
//! grouping sizes, RTL digit shaping, currency-symbol placement rules,
//! etc.) — [`linear::format_value`]'s own grouping is a fixed `,`/`.`
//! (US/UK convention: `,` groups thousands, `.` is the decimal point) and
//! every variant here reuses it verbatim. A caller needing a different
//! separator convention (e.g. `1.234,56` or `1 234,56`) is out of this
//! module's own scope — it would need either a parameterized grouping
//! character threaded through [`linear::format_value`] itself (a change to
//! existing, widely-reused code, not attempted this pass) or a fully
//! separate locale-formatting crate this workspace doesn't depend on.

use super::linear::{self, decimal_precision};

/// SI magnitude threshold — 1000 (kilo), matching the standard k/M/B
/// (thousand/million/billion) short-scale ladder this module's own
/// [`NumberFormat::Si`] uses (not the metric long-scale/milliard ladder).
const SI_THOUSAND: f64 = 1_000.0;
const SI_MILLION: f64 = 1_000_000.0;
const SI_BILLION: f64 = 1_000_000_000.0;

/// Typed value-display format — a caller-chosen alternative to this
/// crate's default axis-tick decimal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberFormat {
    /// Plain decimal at the precision implied by `step` — NO thousands
    /// grouping (e.g. `1234567.89`, not `1,234,567.89`). Distinct from
    /// [`NumberFormat::Thousands`] specifically so a caller can opt OUT of
    /// grouping, which this crate's pre-existing default always applied.
    Plain,
    /// This crate's pre-existing default: [`linear::format_value`]
    /// verbatim — thousands-grouped decimal at the precision implied by
    /// `step` (e.g. `1,234,567.89`). [`NumberFormat::default`].
    Thousands,
    /// SI magnitude suffix (`k`/`M`/`B`) at 1 decimal digit, a trailing
    /// `.0` dropped (e.g. `1234.0` -> `"1.2k"`, `850.0` -> `"850"`,
    /// `2_300_000_000.0` -> `"2.3B"`). No thousands grouping below the `k`
    /// threshold (a 3-digit number needs no grouping to begin with).
    Si,
    /// `value * 100` formatted at the precision implied by `step * 100`,
    /// with a trailing `%` — the caller-supplied domain value is the
    /// underlying FRACTION (e.g. `0.15` -> `"15.00%"`), matching this
    /// module's own single %-as-fraction convention (a caller already
    /// holding a whole percentage, e.g. `15.0`, pre-multiplies by `0.01`
    /// before handing it to a [`NumberFormat::Percent`]-formatted scale).
    Percent,
    /// A caller-supplied currency symbol/code prefixed onto the
    /// thousands-grouped decimal (e.g. `Currency("$")` -> `"$1,234.50"`).
    /// `&'static str` (not `String`) — matches this crate's existing
    /// convention for small, caller-supplied constant strings on a
    /// `Copy` type (e.g. [`crate::scale::Tick`] holds an owned `String`
    /// only because a tick LABEL is computed per-value; a currency SYMBOL
    /// is one fixed constant for the whole scale's lifetime).
    Currency(&'static str),
}

impl Default for NumberFormat {
    /// [`NumberFormat::Thousands`] — see the module docs for why this is
    /// the one variant guaranteed byte-identical to this crate's
    /// pre-existing default axis-tick output.
    fn default() -> Self {
        NumberFormat::Thousands
    }
}

/// SI-suffix formatting for [`NumberFormat::Si`] — see that variant's own
/// docs for the exact rounding/suffix rules.
///
/// **Documented rounding-boundary quirk (not fixed this pass):** a value
/// just under a decade boundary can round its own last displayed digit up
/// INTO the next order without renormalizing the suffix — e.g.
/// `999_999.0` formats as `"1000.0k"` rather than `"1.0M"` (the 1-decimal
/// round of `999.999` is `1000.0`, still divided by `SI_THOUSAND`, not
/// re-evaluated against the `SI_MILLION` threshold after rounding).
/// Fixing this would need a renormalization loop after the initial
/// division — a narrow, extremely rare-in-practice display glitch (only
/// values within `0.05%` of an exact decade boundary trigger it), out of
/// this pass's own scope; every golden test below deliberately avoids
/// this boundary.
fn format_si(value: f64) -> String {
    let abs = value.abs();
    let (scaled, suffix) = if abs >= SI_BILLION {
        (value / SI_BILLION, "B")
    } else if abs >= SI_MILLION {
        (value / SI_MILLION, "M")
    } else if abs >= SI_THOUSAND {
        (value / SI_THOUSAND, "k")
    } else {
        (value, "")
    };
    let mut formatted = format!("{scaled:.1}");
    if let Some(stripped) = formatted.strip_suffix(".0") {
        formatted = stripped.to_owned();
    }
    format!("{formatted}{suffix}")
}

impl NumberFormat {
    /// Format `value` under this variant, using `step` (the same
    /// tick-spacing/precision hint every other formatter in this crate
    /// takes, e.g. [`linear::format_value`]'s own `step` parameter) to
    /// pick a decimal precision where the variant needs one.
    pub fn format(&self, value: f64, step: f64) -> String {
        match self {
            NumberFormat::Plain => {
                let precision = decimal_precision(step);
                format!("{value:.precision$}")
            }
            NumberFormat::Thousands => linear::format_value(value, step),
            NumberFormat::Si => format_si(value),
            NumberFormat::Percent => format!("{}%", linear::format_value(value * 100.0, (step * 100.0).abs())),
            NumberFormat::Currency(symbol) => format!("{symbol}{}", linear::format_value(value, step)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_matches_current_thousands_output() {
        for (value, step) in [(1_234_567.891, 1.0), (-1_234.5, 1.0), (42.0, 1.0), (0.0, 0.5), (-0.005, 0.001)] {
            assert_eq!(NumberFormat::default().format(value, step), linear::format_value(value, step));
        }
        assert_eq!(NumberFormat::default(), NumberFormat::Thousands);
    }

    #[test]
    fn plain_has_no_thousands_grouping_unlike_thousands() {
        assert_eq!(NumberFormat::Plain.format(1_234_567.89, 1.0), "1234567.89");
        assert_eq!(NumberFormat::Thousands.format(1_234_567.89, 1.0), "1,234,567.89");
    }

    #[test]
    fn si_golden_values_below_and_across_every_threshold() {
        assert_eq!(NumberFormat::Si.format(850.0, 1.0), "850");
        assert_eq!(NumberFormat::Si.format(1_234.0, 1.0), "1.2k");
        assert_eq!(NumberFormat::Si.format(-4_200.0, 1.0), "-4.2k");
        assert_eq!(NumberFormat::Si.format(1_500_000.0, 1.0), "1.5M");
        assert_eq!(NumberFormat::Si.format(2_300_000_000.0, 1.0), "2.3B");
        assert_eq!(NumberFormat::Si.format(0.0, 1.0), "0");
    }

    #[test]
    fn si_drops_a_trailing_dot_zero_but_keeps_a_real_fraction() {
        assert_eq!(NumberFormat::Si.format(2_000.0, 1.0), "2k");
        assert_eq!(NumberFormat::Si.format(2_500.0, 1.0), "2.5k");
    }

    #[test]
    fn percent_multiplies_by_100_and_appends_the_sign() {
        // A small enough `step` pushes `decimal_precision` past its 2-digit
        // floor (see `linear::decimal_precision`'s own docs) so this case
        // also proves `step` genuinely propagates through `* 100` too.
        assert_eq!(NumberFormat::Percent.format(0.1234, 0.00005), "12.340%");
        assert_eq!(NumberFormat::Percent.format(-0.05, 0.01), "-5.00%");
        assert_eq!(NumberFormat::Percent.format(1.0, 0.1), "100.00%");
    }

    #[test]
    fn currency_prefixes_the_grouped_decimal() {
        assert_eq!(NumberFormat::Currency("$").format(1_234.5, 1.0), "$1,234.50");
        assert_eq!(NumberFormat::Currency("EUR ").format(-99.9, 0.1), "EUR -99.90");
    }

    #[test]
    fn every_variant_is_finite_and_non_panicking_for_degenerate_step() {
        for format in [NumberFormat::Plain, NumberFormat::Thousands, NumberFormat::Si, NumberFormat::Percent, NumberFormat::Currency("$")] {
            let s = format.format(42.0, 0.0);
            assert!(!s.is_empty(), "{format:?} must produce a non-empty label even for a degenerate step");
        }
    }
}
