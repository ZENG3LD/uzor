//! Small serde-able mirrors of `uzor-figures`' own typed enums — typed
//! things stay typed (no stringly config bags), only the enum's own
//! serde discriminant is a string, same as any ordinary Rust enum's wire
//! representation.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use uzor_figures::{Annotation, BarMode, LegendPosition, NumberFormat, PointRadius, WaterfallKind};

/// Mirrors [`uzor_figures::Annotation`] field-for-field — that type is
/// already all owned (`String`/`f64`/`Option<String>`), so this is a
/// direct serde-able copy, not a lossy approximation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SpecAnnotation {
    /// Horizontal dashed reference line at a fixed Y domain value.
    HLine {
        value: f64,
        #[serde(default)]
        color: Option<String>,
        #[serde(default)]
        label: Option<String>,
    },
    /// Vertical dashed reference line at a fixed X domain value.
    VLine {
        value: f64,
        #[serde(default)]
        color: Option<String>,
        #[serde(default)]
        label: Option<String>,
    },
    /// Shaded band over a Y domain interval `[low, high]`.
    HBand {
        low: f64,
        high: f64,
        #[serde(default)]
        color: Option<String>,
        #[serde(default)]
        label: Option<String>,
    },
    /// Free-text callout anchored at one `(x, y)` domain point.
    Callout { x: f64, y: f64, text: String },
}

impl SpecAnnotation {
    /// Build the real `uzor-figures` [`Annotation`] this spec describes.
    pub fn to_annotation(&self) -> Annotation {
        match self {
            SpecAnnotation::HLine { value, color, label } => Annotation::HLine { value: *value, color: color.clone(), label: label.clone() },
            SpecAnnotation::VLine { value, color, label } => Annotation::VLine { value: *value, color: color.clone(), label: label.clone() },
            SpecAnnotation::HBand { low, high, color, label } => {
                Annotation::HBand { low: *low, high: *high, color: color.clone(), label: label.clone() }
            }
            SpecAnnotation::Callout { x, y, text } => Annotation::Callout { x: *x, y: *y, text: text.clone() },
        }
    }
}

/// Build the real `Vec<Annotation>` a figure's own `.with_annotations(..)`
/// builder expects.
pub fn to_annotations(specs: &[SpecAnnotation]) -> Vec<Annotation> {
    specs.iter().map(SpecAnnotation::to_annotation).collect()
}

/// Mirrors [`uzor_figures::LegendPosition`] (a plain data-less enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecLegendPosition {
    Top,
    Bottom,
    Right,
}

impl From<SpecLegendPosition> for LegendPosition {
    fn from(value: SpecLegendPosition) -> Self {
        match value {
            SpecLegendPosition::Top => LegendPosition::Top,
            SpecLegendPosition::Bottom => LegendPosition::Bottom,
            SpecLegendPosition::Right => LegendPosition::Right,
        }
    }
}

/// Mirrors [`uzor_figures::BarMode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecBarMode {
    Grouped,
    Stacked,
}

impl From<SpecBarMode> for BarMode {
    fn from(value: SpecBarMode) -> Self {
        match value {
            SpecBarMode::Grouped => BarMode::Grouped,
            SpecBarMode::Stacked => BarMode::Stacked,
        }
    }
}

/// Mirrors [`uzor_figures::WaterfallKind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecWaterfallKind {
    Delta,
    Subtotal,
    Total,
}

impl From<SpecWaterfallKind> for WaterfallKind {
    fn from(value: SpecWaterfallKind) -> Self {
        match value {
            SpecWaterfallKind::Delta => WaterfallKind::Delta,
            SpecWaterfallKind::Subtotal => WaterfallKind::Subtotal,
            SpecWaterfallKind::Total => WaterfallKind::Total,
        }
    }
}

/// Mirrors [`uzor_figures::PointRadius`] — internally tagged on `mode`.
/// Nested inside `ScatterSpec::radius: Option<SpecPointRadius>`, so this
/// is just one field's own typed enum shape, not a second top-level
/// dispatch string (the registry's own one-dispatch-level doctrine is
/// about `FigureSpec::kind`, not every nested Rust enum's own serde
/// discriminant).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum SpecPointRadius {
    /// Every point draws at the same radius.
    Fixed { radius: f64 },
    /// Radius linearly interpolated between `min_radius`/`max_radius`
    /// across the point set's own value extent.
    ValueMapped { min_radius: f64, max_radius: f64 },
}

impl From<SpecPointRadius> for PointRadius {
    fn from(value: SpecPointRadius) -> Self {
        match value {
            SpecPointRadius::Fixed { radius } => PointRadius::Fixed(radius),
            SpecPointRadius::ValueMapped { min_radius, max_radius } => PointRadius::ValueMapped { min_radius, max_radius },
        }
    }
}

/// Mirrors [`uzor_figures::NumberFormat`]. `Currency` carries an owned
/// `String` symbol (JSON has no `&'static str` concept) — see
/// [`leak_currency_symbol`]'s own doc comment for how this bridges to
/// `NumberFormat::Currency(&'static str)` without an unbounded leak.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SpecNumberFormat {
    Plain,
    Thousands,
    Si,
    Percent,
    Currency { symbol: String },
}

impl From<&SpecNumberFormat> for NumberFormat {
    fn from(value: &SpecNumberFormat) -> Self {
        match value {
            SpecNumberFormat::Plain => NumberFormat::Plain,
            SpecNumberFormat::Thousands => NumberFormat::Thousands,
            SpecNumberFormat::Si => NumberFormat::Si,
            SpecNumberFormat::Percent => NumberFormat::Percent,
            SpecNumberFormat::Currency { symbol } => NumberFormat::Currency(leak_currency_symbol(symbol)),
        }
    }
}

fn currency_symbol_interner() -> &'static Mutex<HashSet<&'static str>> {
    static INTERNER: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    INTERNER.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Resolve `symbol` to a process-wide-interned `&'static str` for
/// [`uzor_figures::NumberFormat::Currency`]'s own `&'static str` field —
/// `NumberFormat` is a small `Copy` type over a caller-supplied CONSTANT
/// string (see that type's own doc comment: "one fixed constant for the
/// whole scale's lifetime"), a real API shape this crate cannot change
/// (`uzor-figures` is read-only reference for this task, per the task's
/// own scope).
///
/// **Documented trade-off, not a silent unbounded leak**: this crate's
/// own specs are AUTHORED CONTENT (report figures, dashboards) — a real
/// deployment sees a small, bounded set of DISTINCT currency symbols
/// (`"$"`, `"€"`, `"£"`, ...) over a process's whole lifetime, never one
/// unique symbol per render call. A small process-wide interner
/// (`Mutex<HashSet<&'static str>>`) deduplicates by VALUE before leaking,
/// so re-parsing/re-rendering the SAME symbol string many times (e.g. a
/// report generator looping over many cases, all in USD) leaks at most
/// ONE allocation per distinct symbol ever seen, never one per call.
fn leak_currency_symbol(symbol: &str) -> &'static str {
    let interner = currency_symbol_interner();
    let mut guard = interner.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(&existing) = guard.iter().find(|&&s| s == symbol) {
        return existing;
    }
    let leaked: &'static str = Box::leak(symbol.to_owned().into_boxed_str());
    guard.insert(leaked);
    leaked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annotation_round_trips_every_variant_through_the_real_uzor_figures_type() {
        let specs = [
            SpecAnnotation::HLine { value: 1.0, color: Some("#fff".to_owned()), label: None },
            SpecAnnotation::VLine { value: 2.0, color: None, label: Some("mark".to_owned()) },
            SpecAnnotation::HBand { low: 1.0, high: 2.0, color: None, label: None },
            SpecAnnotation::Callout { x: 1.0, y: 2.0, text: "note".to_owned() },
        ];
        for spec in specs {
            let annotation = spec.to_annotation();
            let ok = match (&spec, &annotation) {
                (SpecAnnotation::HLine { value, .. }, Annotation::HLine { value: v2, .. }) => value == v2,
                (SpecAnnotation::VLine { value, .. }, Annotation::VLine { value: v2, .. }) => value == v2,
                (SpecAnnotation::HBand { low, high, .. }, Annotation::HBand { low: l2, high: h2, .. }) => low == l2 && high == h2,
                (SpecAnnotation::Callout { x, y, text }, Annotation::Callout { x: x2, y: y2, text: t2 }) => x == x2 && y == y2 && text == t2,
                _ => false,
            };
            assert!(ok, "conversion must preserve the same variant and fields: {spec:?} -> {annotation:?}");
        }
    }

    #[test]
    fn currency_symbol_interner_dedupes_the_same_symbol() {
        let a = leak_currency_symbol("$");
        let b = leak_currency_symbol("$");
        assert!(std::ptr::eq(a, b), "the same symbol string must resolve to the SAME leaked allocation, not a new one per call");
    }

    #[test]
    fn distinct_symbols_get_distinct_allocations() {
        let usd = leak_currency_symbol("USD-TEST-MARKER");
        let eur = leak_currency_symbol("EUR-TEST-MARKER");
        assert_ne!(usd, eur);
    }
}
