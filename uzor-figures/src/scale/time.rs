//! `TimeScale` — continuous UTC-timestamp domain with calendar-aware tick
//! generation.
//!
//! Ported and generalized from `mylittlechart`'s `TimeScale`
//! (`mlc-core/src/chart/types/time_scale.rs`, full file 1-1899) per
//! `nemo/docs/uzor-engines/mlc_harvest_inventory.md` §2. That source is
//! **bar-index** X-axis math (a `TimeTick` carries a `bar_idx`, resolved
//! through a `Viewport`'s pixel-per-bar zoom) built around an explicit
//! port of TradingView Lightweight Charts' `TickMarkWeight` hierarchy
//! (source comment, its own lines 138-140). This module harvests ONLY the
//! weight system + calendar utilities + label-format table — never the
//! bar/viewport machinery, which has no meaning for a scale mapping
//! continuous timestamps directly (design brief: "port the WEIGHT SYSTEM,
//! not the bar machinery").
//!
//! ## Unit
//!
//! The domain (`min_ts`/`max_ts`, every [`Tick::value`]) is **Unix
//! seconds (UTC), as `f64`** — matching the mlc source's own base
//! representation (`Bar::timestamp: i64`, whole seconds; the source only
//! scales to milliseconds transiently inside `weight_by_time`'s
//! sub-minute divisor table, which this port does not carry — see below).
//! Calendar math floors/ceils to whole seconds internally; sub-second
//! precision survives `map`/`invert` (plain linear interpolation) but has
//! no dedicated tick tier.
//!
//! ## What was ported vs. dropped
//!
//! Ported near-verbatim: [`TickMarkWeight`] (same 13 variants, same
//! discriminant values, same `is_major`/`is_medium`), the from-scratch
//! calendar utilities ([`timestamp_to_date`]/`date_to_timestamp`/
//! `is_leap_year`/`days_in_month` — source lines 37-129, "no external
//! dependencies"), and the per-weight label format table (source's
//! `format_time_by_weight`, lines 1313-1347).
//!
//! Deliberately dropped (bar/app machinery, not weight-system math):
//! - `weight_by_time`/`fill_weights`/`is_non_uniform` — these classify a
//!   weight by comparing ADJACENT BARS' timestamps (source lines
//!   198-275). A continuous scale has no bars to compare; [`boundary_weight`]
//!   replaces them with the direct generalization — classify a single
//!   instant by the coarsest calendar boundary it lands on exactly. Any
//!   candidate tick this module generates always sits exactly on such a
//!   boundary (by construction), so per-tick classification this way is
//!   equivalent to the source's per-bar comparison, without needing bars.
//! - The `TICKS_{28,29,30,31}_STEP{1..5}` day-of-month lookup tables +
//!   `get_tick_days` (source lines 402-462). Those exist ONLY to keep
//!   PIXEL spacing uniform across variable-length months when X is
//!   bar-index (source's `Viewport::bar_to_x_f64` is not proportional to
//!   real elapsed time for non-uniform bar families). This scale's `map`
//!   is a direct linear function of real elapsed time, so real calendar
//!   day/month/year boundaries are ALREADY pixel-correct — no lookup
//!   table needed. This is a genuine simplification enabled by the
//!   different (continuous-time, not bar-index) domain model, not a
//!   dropped feature.
//! - `generate_intraday_ticks`/`generate_daily_ticks`/
//!   `generate_calendar_grid_ticks`/`select_ticks_with_spacing`'s
//!   viewport/bar-index-driven cadence dispatch (`day_width_px`
//!   thresholds against a pixel budget) — replaced by
//!   [`TimeScale::ticks`]'s own cadence ladder, driven by `target_count`
//!   directly (there is no pixel viewport at the `Scale` trait level).
//! - `format_time_by_weight_with_settings`/`TimeFormatSettings`/
//!   `DateFormat`/day-of-week-prefix/12h-AM-PM — locale/display
//!   PREFERENCE formatting, not weight-system math. Per the crate's own
//!   scope ("no trading vocabulary", UTC-only), this scale ships one
//!   canonical UTC/English label per weight tier (matching
//!   `scale::linear`'s single canonical `format_value`) — a consumer
//!   wanting a different locale/12h format re-derives from
//!   [`timestamp_to_date`] + [`boundary_weight`] itself. Localization is
//!   explicitly a consumer concern.
//! - Generalized addition (not in the source at all): a real
//!   **year-boundary tick tier**. The source's own daily/monthly path
//!   caps out at "show only 1st-of-month labels" once 3+ months are
//!   visible (source `generate_daily_ticks`, `visible_month_count >= 3`)
//!   and never actually walks year boundaries in its calendar-grid path
//!   (only `weight_by_time`'s per-bar classification can ever produce
//!   `TickMarkWeight::Year`, and only for the non-uniform-bar family).
//!   That's a fine simplification for mlc's charts (a bounded max
//!   zoom-out), but this scale must remain sensible over multi-decade
//!   domains — see [`TimeScale::ticks`]'s year regime, which reuses
//!   [`crate::scale::linear::nice_step`]'s `[2, 2.5, 2]` ladder over a
//!   year count (a year axis behaves enough like a linear one to reuse
//!   that exact "nice" math, generalized off the price axis it was
//!   already generalized off).
//! - The month-cadence ladder (`[1, 2, 3, 6, 12]` months) additionally
//!   generalizes the source's flat "every month" cadence into real
//!   quarter/half-year tick STEPS (not weight tiers — `TickMarkWeight`
//!   has no `Quarter`/`HalfYear` variant, matching the source; a
//!   6-months-apart tick still classifies as `Month` unless it also
//!   happens to be January 1st, which classifies `Year`).

use super::{Scale, Tick, TickPriority};

// =============================================================================
// Time constants (seconds)
// =============================================================================

const MINUTE: i64 = 60;
const HOUR: i64 = 3_600;
const DAY: i64 = 86_400;

// =============================================================================
// Calendar utilities — ported near-verbatim from
// `mlc-core/src/chart/types/time_scale.rs:37-129` ("no external
// dependencies" calendar system).
// =============================================================================

const DAYS_IN_MONTH: [i32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn days_in_month(year: i32, month: i32) -> i32 {
    if month == 2 && is_leap_year(year) {
        29
    } else {
        DAYS_IN_MONTH[(month - 1) as usize]
    }
}

/// Convert a Unix timestamp (UTC seconds) to date components.
/// Returns `(year, month 1-12, day 1-31, hour 0-23, minute 0-59, second 0-59)`.
pub fn timestamp_to_date(ts: i64) -> (i32, i32, i32, i32, i32, i32) {
    let time_of_day = ts.rem_euclid(DAY);
    let hour = (time_of_day / HOUR) as i32;
    let minute = ((time_of_day % HOUR) / MINUTE) as i32;
    let second = (time_of_day % MINUTE) as i32;

    let mut days = ts.div_euclid(DAY);
    let mut year = 1970_i32;

    if days >= 0 {
        loop {
            let days_in_year = if is_leap_year(year) { 366 } else { 365 };
            if days < days_in_year as i64 {
                break;
            }
            days -= days_in_year as i64;
            year += 1;
        }
    } else {
        loop {
            year -= 1;
            let days_in_year = if is_leap_year(year) { 366 } else { 365 };
            days += days_in_year as i64;
            if days >= 0 {
                break;
            }
        }
    }

    let mut month = 1_i32;
    loop {
        let dim = days_in_month(year, month) as i64;
        if days < dim {
            break;
        }
        days -= dim;
        month += 1;
    }

    let day = days as i32 + 1;
    (year, month, day, hour, minute, second)
}

/// Convert date components (UTC) to a Unix timestamp in seconds.
/// `month`: 1-12, `day`: 1-31.
fn date_to_timestamp(year: i32, month: i32, day: i32, hour: i32, minute: i32, second: i32) -> i64 {
    let mut days: i64 = 0;

    if year >= 1970 {
        for y in 1970..year {
            days += if is_leap_year(y) { 366 } else { 365 };
        }
    } else {
        for y in year..1970 {
            days -= if is_leap_year(y) { 366 } else { 365 };
        }
    }

    for m in 1..month {
        days += days_in_month(year, m) as i64;
    }
    days += (day - 1) as i64;

    days * DAY + hour as i64 * HOUR + minute as i64 * MINUTE + second as i64
}

fn month_index(year: i32, month: i32) -> i64 {
    year as i64 * 12 + (month as i64 - 1)
}

// =============================================================================
// Tick mark weight — ported verbatim from
// `mlc-core/src/chart/types/time_scale.rs:141-183`.
// =============================================================================

/// Hierarchical tick-mark weight: `Year` (70) down to `LessThanSecond` (1).
/// Higher weight = coarser calendar boundary = more visually important
/// (bigger font / brighter color, a consumer's styling concern).
///
/// Variants mirror TradingView Lightweight Charts' `TickMarkWeight` (see
/// module docs) so the same weight hierarchy applies whether a tick came
/// from a bar-indexed chart or this continuous scale.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
#[repr(u8)]
pub enum TickMarkWeight {
    /// Sub-second resolution. Never produced by [`boundary_weight`] (this
    /// scale's domain floors to whole seconds) — kept for API parity with
    /// the source hierarchy and the format table's match exhaustiveness.
    LessThanSecond = 1,
    /// Sub-minute granularity.
    #[default]
    Second = 5,
    /// 1-minute boundaries.
    Minute1 = 10,
    /// 5-minute boundaries.
    Minute5 = 15,
    /// 30-minute boundaries.
    Minute30 = 20,
    /// Hour boundaries.
    Hour = 30,
    /// 3-hour boundaries.
    Hour3 = 31,
    /// 4-hour boundaries.
    Hour4 = 35,
    /// 6-hour boundaries.
    Hour6 = 36,
    /// 12-hour boundaries.
    Hour12 = 37,
    /// Midnight-UTC day boundaries.
    Day = 50,
    /// 1st-of-month boundaries.
    Month = 60,
    /// January 1st boundaries.
    Year = 70,
}

impl TickMarkWeight {
    /// Major tier (Year/Month) — brighter/bolder styling.
    pub fn is_major(&self) -> bool {
        matches!(self, TickMarkWeight::Year | TickMarkWeight::Month)
    }

    /// Medium tier (Day) — between major and minor styling.
    pub fn is_medium(&self) -> bool {
        matches!(self, TickMarkWeight::Day)
    }
}

/// Classify a single UTC instant (seconds) by the COARSEST calendar
/// boundary it lands on exactly — the direct generalization of the
/// source's `weight_by_time` (which compared two adjacent bars) for a
/// domain with no bars to compare: every tick this module generates sits
/// exactly on one of these boundaries by construction, so classifying it
/// in isolation is equivalent.
///
/// Exposed as a standalone, cheap accessor (not stored per [`Tick`], which
/// stays the shared, scale-agnostic `{value, label}` shape every scale in
/// this crate returns) — a consumer styling major/minor gridlines calls
/// `boundary_weight(tick.value as i64)` for the same tier info the source
/// baked into its own `TimeTick::weight` field.
pub fn boundary_weight(ts_secs: i64) -> TickMarkWeight {
    let (_, month, day, hour, minute, second) = timestamp_to_date(ts_secs);
    let midnight = hour == 0 && minute == 0 && second == 0;
    if month == 1 && day == 1 && midnight {
        return TickMarkWeight::Year;
    }
    if day == 1 && midnight {
        return TickMarkWeight::Month;
    }
    if midnight {
        return TickMarkWeight::Day;
    }
    if ts_secs.rem_euclid(12 * HOUR) == 0 {
        return TickMarkWeight::Hour12;
    }
    if ts_secs.rem_euclid(6 * HOUR) == 0 {
        return TickMarkWeight::Hour6;
    }
    if ts_secs.rem_euclid(4 * HOUR) == 0 {
        return TickMarkWeight::Hour4;
    }
    if ts_secs.rem_euclid(3 * HOUR) == 0 {
        return TickMarkWeight::Hour3;
    }
    if ts_secs.rem_euclid(HOUR) == 0 {
        return TickMarkWeight::Hour;
    }
    if ts_secs.rem_euclid(30 * MINUTE) == 0 {
        return TickMarkWeight::Minute30;
    }
    if ts_secs.rem_euclid(5 * MINUTE) == 0 {
        return TickMarkWeight::Minute5;
    }
    if ts_secs.rem_euclid(MINUTE) == 0 {
        return TickMarkWeight::Minute1;
    }
    TickMarkWeight::Second
}

// =============================================================================
// Label formatting — ported from `format_time_by_weight`
// (`mlc-core/src/chart/types/time_scale.rs:1313-1347`), locale/settings
// variant dropped (see module docs).
// =============================================================================

const MONTH_NAMES: [&str; 12] =
    ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// Format a UTC instant (seconds) as a label appropriate for `weight`.
pub fn format_by_weight(ts_secs: i64, weight: TickMarkWeight) -> String {
    let (year, month, day, hour, minute, second) = timestamp_to_date(ts_secs);
    match weight {
        TickMarkWeight::Year => format!("{year}"),
        TickMarkWeight::Month => MONTH_NAMES[(month - 1) as usize].to_owned(),
        TickMarkWeight::Day => format!("{day} {}", MONTH_NAMES[(month - 1) as usize]),
        TickMarkWeight::Hour12
        | TickMarkWeight::Hour6
        | TickMarkWeight::Hour4
        | TickMarkWeight::Hour3
        | TickMarkWeight::Hour
        | TickMarkWeight::Minute30
        | TickMarkWeight::Minute5
        | TickMarkWeight::Minute1 => format!("{hour:02}:{minute:02}"),
        TickMarkWeight::Second | TickMarkWeight::LessThanSecond => {
            format!("{hour:02}:{minute:02}:{second:02}")
        }
    }
}

// =============================================================================
// Cadence ladders driving `TimeScale::ticks`'s regime selection.
// =============================================================================

/// Fixed-cadence tiers (seconds), ascending, from 1 second up to 21 days.
/// Restates the source's `day_width_px`-threshold cadence ladder
/// (`generate_intraday_ticks`) as an explicit seconds-per-tick menu (no
/// pixel viewport to threshold against at the `Scale` trait level), and
/// extends it with day/week multiples (the source's own
/// day-of-month lookup tables are dropped — see module docs) before
/// handing off to the month ladder.
const SUB_MONTH_STEPS_SECS: &[i64] = &[
    1,
    2,
    5,
    10,
    15,
    30,
    MINUTE,
    5 * MINUTE,
    15 * MINUTE,
    30 * MINUTE,
    HOUR,
    3 * HOUR,
    6 * HOUR,
    12 * HOUR,
    DAY,
    2 * DAY,
    3 * DAY,
    5 * DAY,
    7 * DAY,
    10 * DAY,
    14 * DAY,
    21 * DAY,
];

/// Calendar-month cadence tiers: month, quarter, half-year, year-via-months.
const MONTH_STEPS: &[i64] = &[1, 2, 3, 6, 12];

/// How much a candidate cadence's resulting tick count may exceed
/// `target_count` before it's accepted — scanned finest-to-coarsest, so
/// this is "don't reject a candidate just for being a little over the
/// target," not a hard cap.
fn fits_budget(count: i64, target: i64) -> bool {
    count <= target * 2
}

fn walk_seconds_ticks(min_ts: f64, max_ts: f64, min_secs: i64, max_secs: i64, step: i64) -> Vec<Tick> {
    let mut t = min_secs.div_euclid(step) * step;
    if t < min_secs {
        t += step;
    }
    let mut out = Vec::new();
    while t <= max_secs {
        let value = t as f64;
        if value >= min_ts && value <= max_ts {
            let weight = boundary_weight(t);
            out.push(Tick { value, label: format_by_weight(t, weight) });
        }
        t += step;
    }
    out
}

fn walk_months_ticks(min_ts: f64, max_ts: f64, min_secs: i64, max_secs: i64, step_months: i64) -> Vec<Tick> {
    let (y0, m0, ..) = timestamp_to_date(min_secs);
    let (y1, m1, ..) = timestamp_to_date(max_secs);
    let idx0 = month_index(y0, m0);
    let idx1 = month_index(y1, m1);

    let mut idx = idx0.div_euclid(step_months) * step_months;
    if idx < idx0 {
        idx += step_months;
    }

    let mut out = Vec::new();
    while idx <= idx1 {
        let year = idx.div_euclid(12) as i32;
        let month = (idx.rem_euclid(12) + 1) as i32;
        let ts = date_to_timestamp(year, month, 1, 0, 0, 0);
        let value = ts as f64;
        if value >= min_ts && value <= max_ts {
            let weight = boundary_weight(ts);
            out.push(Tick { value, label: format_by_weight(ts, weight) });
        }
        idx += step_months;
    }
    out
}

fn walk_years_ticks(min_ts: f64, max_ts: f64, min_secs: i64, max_secs: i64, step_years: i64) -> Vec<Tick> {
    let (y0, ..) = timestamp_to_date(min_secs);
    let (y1, ..) = timestamp_to_date(max_secs);
    let step_years = step_years.max(1);

    let mut year = (y0 as i64).div_euclid(step_years) * step_years;
    if year < y0 as i64 {
        year += step_years;
    }

    let mut out = Vec::new();
    while year <= y1 as i64 {
        let ts = date_to_timestamp(year as i32, 1, 1, 0, 0, 0);
        let value = ts as f64;
        if value >= min_ts && value <= max_ts {
            let weight = boundary_weight(ts);
            out.push(Tick { value, label: format_by_weight(ts, weight) });
        }
        year += step_years;
    }
    out
}

// =============================================================================
// TimeScale
// =============================================================================

/// Continuous UTC-time domain: `[min_ts, max_ts]` in Unix seconds (`f64`).
/// `map`/`invert` are plain linear interpolation over that range (same
/// shape as [`super::LinearScale`]); [`Scale::ticks`] is calendar-aware,
/// picking whichever cadence (seconds/minutes/hours, calendar days,
/// calendar months, or calendar years) best fits `target_count` — see the
/// module docs for the full ladder and what was dropped from the mlc
/// source it was ported from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeScale {
    pub min_ts: f64,
    pub max_ts: f64,
}

impl TimeScale {
    pub fn new(min_ts: f64, max_ts: f64) -> Self {
        Self { min_ts, max_ts }
    }

    fn range(&self) -> f64 {
        self.max_ts - self.min_ts
    }
}

impl Scale for TimeScale {
    fn domain(&self) -> (f64, f64) {
        (self.min_ts, self.max_ts)
    }

    fn map(&self, v: f64) -> f64 {
        let range = self.range();
        if range.abs() < f64::EPSILON {
            return 0.5;
        }
        (v - self.min_ts) / range
    }

    fn invert(&self, t: f64) -> f64 {
        self.min_ts + t * self.range()
    }

    fn ticks(&self, target_count: usize) -> Vec<Tick> {
        let (min_ts, max_ts) = (self.min_ts, self.max_ts);

        if !min_ts.is_finite() || !max_ts.is_finite() || max_ts <= min_ts {
            let anchor = if min_ts.is_finite() { min_ts } else { 0.0 };
            let secs = anchor.floor() as i64;
            let weight = boundary_weight(secs);
            return vec![Tick { value: anchor, label: format_by_weight(secs, weight) }];
        }

        let target = target_count.max(1) as i64;
        let min_secs = min_ts.floor() as i64;
        let max_secs = max_ts.ceil() as i64;
        let span_secs = (max_secs - min_secs).max(1);

        for &step in SUB_MONTH_STEPS_SECS {
            let count = span_secs / step + 1;
            if fits_budget(count, target) {
                return walk_seconds_ticks(min_ts, max_ts, min_secs, max_secs, step);
            }
        }

        let (y0, m0, ..) = timestamp_to_date(min_secs);
        let (y1, m1, ..) = timestamp_to_date(max_secs);
        let span_months = month_index(y1, m1) - month_index(y0, m0);

        for &step in MONTH_STEPS {
            let count = span_months / step + 1;
            if fits_budget(count, target) {
                return walk_months_ticks(min_ts, max_ts, min_secs, max_secs, step);
            }
        }

        // Fallback: calendar years, "nice" step reused from the linear
        // scale's own `[2, 2.5, 2]` ladder (see module docs) — a year
        // count behaves enough like a plain linear quantity for that math
        // to apply directly.
        let span_years = ((y1 - y0) as i64).max(1) as f64;
        let step_years = super::linear::nice_step(span_years, target as f64).round().max(1.0) as i64;
        walk_years_ticks(min_ts, max_ts, min_secs, max_secs, step_years)
    }

    /// Full "day month hour:minute" calendar label at whole-minute
    /// precision, regardless of tick weight — overrides [`Scale`]'s
    /// default numeric-step formatter (which would otherwise print a raw
    /// Unix-second number for this scale). Unlike [`format_by_weight`]
    /// (used for AXIS ticks, which intentionally coarsen to e.g. just a
    /// month name once zoomed out), a hover/tooltip/crosshair label always
    /// wants full precision for the exact instant under the cursor.
    fn format_value(&self, v: f64) -> String {
        let secs = v.floor() as i64;
        let (_, month, day, hour, minute, _second) = timestamp_to_date(secs);
        format!("{day} {} {hour:02}:{minute:02}", MONTH_NAMES[(month - 1) as usize])
    }

    /// Real per-tick calendar-boundary classification — see [`Scale::
    /// tick_weight`]'s own doc comment for why this hook exists on the
    /// trait at all (a generic axis/grid caller styling major/minor ticks
    /// without downcasting).
    fn tick_weight(&self, v: f64) -> Option<TickMarkWeight> {
        Some(boundary_weight(v.floor() as i64))
    }

    /// Reuses the SAME [`boundary_weight`] classification
    /// [`TimeScale::tick_weight`] already computes — a Year/Month
    /// boundary is [`TickPriority::Major`] (protected from
    /// `guide::axis`'s label-collision skip the same way it's already
    /// styled distinctly under the `_weighted` entry points), every
    /// coarser-than-Day tick (Day/Hour/Minute/Second) is
    /// [`TickPriority::Minor`] — the generalization this item's own
    /// defect fix asked for: "every scale declares tick importance, not
    /// just TimeScale" now literally includes `TimeScale` itself in the
    /// SAME priority-aware skip every other scale opts into, so a real
    /// Year/Month boundary can no longer lose a label-collision to an
    /// adjacent Day tick even on this figure's own DEFAULT (unweighted)
    /// axis draw.
    fn tick_priority(&self, v: f64) -> TickPriority {
        if boundary_weight(v.floor() as i64).is_major() {
            TickPriority::Major
        } else {
            TickPriority::Minor
        }
    }

    /// A plain `TimeScale::new(min_ts, max_ts)` — see [`Scale::windowed`]'s
    /// own doc comment for the seam this serves. This is the concrete
    /// case the harvest doc's own domain-vs-bar-index distinction is
    /// about: a [`crate::interact::viewport::Viewport`] windows this
    /// scale in real UTC SECONDS, never a bar/array position.
    fn windowed(&self, min: f64, max: f64) -> Option<Box<dyn Scale>> {
        Some(Box::new(TimeScale::new(min, max)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(year: i32, month: i32, day: i32, hour: i32, minute: i32, second: i32) -> f64 {
        date_to_timestamp(year, month, day, hour, minute, second) as f64
    }

    #[test]
    fn tick_weight_ordering_matches_source_hierarchy() {
        assert!(TickMarkWeight::Year > TickMarkWeight::Month);
        assert!(TickMarkWeight::Month > TickMarkWeight::Day);
        assert!(TickMarkWeight::Day > TickMarkWeight::Hour12);
        assert!(TickMarkWeight::Hour12 > TickMarkWeight::Hour);
        assert!(TickMarkWeight::Hour > TickMarkWeight::Minute1);
        assert!(TickMarkWeight::Minute1 > TickMarkWeight::Second);
    }

    #[test]
    fn weight_classification_matches_source() {
        assert!(TickMarkWeight::Year.is_major());
        assert!(TickMarkWeight::Month.is_major());
        assert!(!TickMarkWeight::Day.is_major());
        assert!(TickMarkWeight::Day.is_medium());
        assert!(!TickMarkWeight::Hour.is_medium());
    }

    #[test]
    fn calendar_round_trip_accuracy() {
        let feb_28_2023 = date_to_timestamp(2023, 2, 28, 12, 0, 0);
        let mar_1_2023 = date_to_timestamp(2023, 3, 1, 12, 0, 0);
        assert_eq!(timestamp_to_date(feb_28_2023), (2023, 2, 28, 12, 0, 0));
        assert_eq!(timestamp_to_date(mar_1_2023), (2023, 3, 1, 12, 0, 0));

        // Leap day exists in 2024.
        let feb_29_2024 = date_to_timestamp(2024, 2, 29, 0, 0, 0);
        assert_eq!(timestamp_to_date(feb_29_2024), (2024, 2, 29, 0, 0, 0));
    }

    #[test]
    fn boundary_weight_classifies_calendar_tiers() {
        let jan1 = date_to_timestamp(2024, 1, 1, 0, 0, 0);
        let jul1 = date_to_timestamp(2024, 7, 1, 0, 0, 0);
        let day15 = date_to_timestamp(2024, 3, 15, 0, 0, 0);
        let noon = date_to_timestamp(2024, 3, 15, 12, 0, 0);
        let half_hour = date_to_timestamp(2024, 3, 15, 12, 30, 0);
        assert_eq!(boundary_weight(jan1), TickMarkWeight::Year);
        assert_eq!(boundary_weight(jul1), TickMarkWeight::Month);
        assert_eq!(boundary_weight(day15), TickMarkWeight::Day);
        assert_eq!(boundary_weight(noon), TickMarkWeight::Hour12);
        assert_eq!(boundary_weight(half_hour), TickMarkWeight::Minute30);
    }

    #[test]
    fn format_table_matches_weight_tier() {
        let t = date_to_timestamp(2023, 1, 2, 3, 4, 5);
        assert_eq!(format_by_weight(t, TickMarkWeight::Year), "2023");
        assert_eq!(format_by_weight(t, TickMarkWeight::Month), "Jan");
        assert_eq!(format_by_weight(t, TickMarkWeight::Day), "2 Jan");
        assert_eq!(format_by_weight(t, TickMarkWeight::Hour), "03:04");
        assert_eq!(format_by_weight(t, TickMarkWeight::Second), "03:04:05");
    }

    // ── Tick-ladder tests (per weight regime) ──────────────────────────

    #[test]
    fn ticks_over_two_hours_pick_intraday_cadence() {
        let start = ts(2024, 6, 1, 8, 0, 0);
        let end = ts(2024, 6, 1, 10, 0, 0);
        let scale = TimeScale::new(start, end);
        let ticks = scale.ticks(6);

        assert!(!ticks.is_empty());
        assert!(ticks.len() <= 6 * 3, "tick count should stay near the target, got {}", ticks.len());
        for w in ticks.windows(2) {
            assert!(w[1].value > w[0].value, "ticks must be strictly increasing");
        }
        for t in &ticks {
            assert!(t.value >= start && t.value <= end, "tick must fall inside the domain");
            let weight = boundary_weight(t.value as i64);
            assert!(
                weight <= TickMarkWeight::Hour12,
                "a 2-hour span must not produce coarser-than-half-day ticks, got {weight:?}"
            );
        }
    }

    #[test]
    fn ticks_over_three_days_pick_hour_scale_cadence() {
        // Deliberately NOT anchored on the 1st of a month — June 10-13
        // avoids the (correct, but test-confusing) case where a boundary
        // tick coincides with a real month/year start and legitimately
        // outranks Day (see `boundary_weight_classifies_calendar_tiers`
        // and the weight-monotonicity test for that behavior on purpose).
        let start = ts(2024, 6, 10, 0, 0, 0);
        let end = ts(2024, 6, 13, 0, 0, 0);
        let scale = TimeScale::new(start, end);
        let ticks = scale.ticks(6);

        assert!(!ticks.is_empty());
        for w in ticks.windows(2) {
            assert!(w[1].value > w[0].value);
        }
        for t in &ticks {
            assert!(t.value >= start && t.value <= end);
        }
        // A 3-day span at a target of 6 should land somewhere in the
        // hour-to-day tier, never dropping all the way to Month/Year.
        assert!(ticks.iter().any(|t| boundary_weight(t.value as i64) <= TickMarkWeight::Day));
        assert!(ticks.iter().all(|t| boundary_weight(t.value as i64) <= TickMarkWeight::Day));
    }

    #[test]
    fn ticks_over_two_months_pick_day_scale_cadence() {
        let start = ts(2024, 1, 1, 0, 0, 0);
        let end = ts(2024, 3, 2, 0, 0, 0);
        let scale = TimeScale::new(start, end);
        let ticks = scale.ticks(6);

        assert!(!ticks.is_empty());
        for w in ticks.windows(2) {
            assert!(w[1].value > w[0].value);
        }
        for t in &ticks {
            assert!(t.value >= start && t.value <= end);
        }
        // ~2 months should resolve at day or (at coarsest) month
        // granularity, never a bare year tick.
        assert!(ticks.iter().all(|t| boundary_weight(t.value as i64) <= TickMarkWeight::Month));
    }

    #[test]
    fn ticks_over_three_years_include_year_boundaries() {
        let start = ts(2022, 1, 1, 0, 0, 0);
        let end = ts(2025, 1, 1, 0, 0, 0);
        let scale = TimeScale::new(start, end);
        let ticks = scale.ticks(6);

        assert!(!ticks.is_empty());
        for w in ticks.windows(2) {
            assert!(w[1].value > w[0].value);
        }
        for t in &ticks {
            assert!(t.value >= start && t.value <= end);
        }
        assert!(
            ticks.iter().any(|t| boundary_weight(t.value as i64) == TickMarkWeight::Year),
            "a 3-year span must surface at least one Year-weight tick, got {ticks:?}"
        );
    }

    #[test]
    fn year_boundary_outweighs_month_boundary_in_the_same_window() {
        // Same fixture as the 3-year ladder test: a half-year cadence over
        // 3 years mixes January-1st (Year) ticks with mid-year (Month)
        // ticks in ONE generated set — direct proof of weight
        // monotonicity on real output, not just on hand-picked timestamps.
        let start = ts(2022, 1, 1, 0, 0, 0);
        let end = ts(2025, 1, 1, 0, 0, 0);
        let scale = TimeScale::new(start, end);
        let ticks = scale.ticks(6);

        let max_weight = ticks.iter().map(|t| boundary_weight(t.value as i64)).max();
        let month_tick_exists = ticks.iter().any(|t| boundary_weight(t.value as i64) == TickMarkWeight::Month);

        assert_eq!(max_weight, Some(TickMarkWeight::Year));
        assert!(month_tick_exists, "expected a mixed-weight window, got {ticks:?}");
        assert!(TickMarkWeight::Year > TickMarkWeight::Month);
    }

    // ── Round-trip / edge cases ─────────────────────────────────────────

    #[test]
    fn map_and_invert_round_trip() {
        let start = ts(2020, 1, 1, 0, 0, 0);
        let end = ts(2024, 1, 1, 0, 0, 0);
        let scale = TimeScale::new(start, end);
        for v in [start, start + 1.0, (start + end) / 2.0, end - 1.0, end] {
            let t = scale.map(v);
            let back = scale.invert(t);
            assert!((back - v).abs() < 1e-6, "round trip drifted: {v} -> {back}");
        }
    }

    #[test]
    fn zero_span_domain_does_not_panic() {
        let point = ts(2024, 5, 17, 12, 0, 0);
        let scale = TimeScale::new(point, point);
        assert_eq!(scale.map(point), 0.5);
        let ticks = scale.ticks(5);
        assert_eq!(ticks.len(), 1);
        assert_eq!(ticks[0].value, point);
    }

    #[test]
    fn non_finite_domain_does_not_panic() {
        let scale = TimeScale::new(f64::NAN, f64::NAN);
        let ticks = scale.ticks(5);
        assert_eq!(ticks.len(), 1);
        assert!(ticks[0].value.is_finite());
    }

    #[test]
    fn format_value_gives_a_full_calendar_label_not_a_raw_number() {
        let t = ts(2024, 3, 15, 9, 5, 0);
        let scale = TimeScale::new(t - 3600.0, t + 3600.0);
        assert_eq!(scale.format_value(t), "15 Mar 09:05");
    }

    #[test]
    fn scale_trait_tick_weight_matches_boundary_weight_directly() {
        // `Scale::tick_weight` is the generic hook `guide::axis`/
        // `guide::grid`'s weighted entry points consult through `&dyn
        // Scale` — must agree with `boundary_weight` called directly.
        let jan1 = ts(2024, 1, 1, 0, 0, 0);
        let scale: &dyn Scale = &TimeScale::new(jan1 - 3600.0, jan1 + 3600.0);
        assert_eq!(scale.tick_weight(jan1), Some(boundary_weight(jan1 as i64)));
        assert_eq!(scale.tick_weight(jan1), Some(TickMarkWeight::Year));
    }

    #[test]
    fn tick_priority_marks_year_and_month_boundaries_major_everything_else_minor() {
        let scale = TimeScale::new(1_704_067_200.0, 1_704_067_200.0 + 62.0 * 86_400.0);
        let jan1_2024 = ts(2024, 1, 1, 0, 0, 0);
        let feb1_2024 = ts(2024, 2, 1, 0, 0, 0);
        let jan2_2024 = ts(2024, 1, 2, 0, 0, 0);
        assert_eq!(scale.tick_priority(jan1_2024), TickPriority::Major, "a Year boundary must report Major priority");
        assert_eq!(scale.tick_priority(feb1_2024), TickPriority::Major, "a Month boundary must report Major priority");
        assert_eq!(scale.tick_priority(jan2_2024), TickPriority::Minor, "an ordinary Day boundary must report Minor priority");
    }

    #[test]
    fn windowed_rebuilds_a_time_scale_over_the_given_unix_second_bounds() {
        let jan1_2024 = ts(2024, 1, 1, 0, 0, 0);
        let scale = TimeScale::new(jan1_2024, jan1_2024 + 90.0 * 86_400.0);
        let window_start = jan1_2024 + 10.0 * 86_400.0;
        let window_end = jan1_2024 + 20.0 * 86_400.0;
        let windowed = scale.windowed(window_start, window_end).expect("TimeScale supports windowing");
        assert_eq!(windowed.domain(), (window_start, window_end));
    }
}
