//! Golden grid (H1 Brief 10a-4): toast × 4 built-in sets × its own real
//! severity set. `ToastSeverity` (`Info`/`Success`/`Warning`/`Error`) is the
//! widget's entire visual axis — `draw_toast_at` has no hover/disabled
//! branch at all. One cell per severity, all at full opacity: `alpha_for`
//! (render.rs) is `1.0` whenever `remaining_fraction(now_ms) >= 0.2`, and
//! `remaining_fraction` returns `1.0` for any `now_ms <= timestamp_ms` — so
//! fixing `now_ms = 0` against the default `timestamp_ms = 0` a toast is
//! stamped with (`ToastType::info`/`success`/`warning`/`error`) always lands
//! on full opacity deterministically, never a wall-clock sample.
//!
//! `hc_mono`'s cells are expected fully grayscale here too — that set's
//! status-role tokens (which `bg_for` resolves per severity) are gray by
//! design, not a rendering defect.

#![cfg(feature = "golden")]

use uzor::tokens::{BuiltinSet, TokenTheme, Tokens};
use uzor::ui::widgets::atomic::toast::{self, ToastEntry, ToastSeverity, ToastType};
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

use super::support::{self, ReviewSheet};

/// Fixed animation time: `now_ms == timestamp_ms == 0` for every cell, so
/// `alpha_for` always resolves to `1.0` (module doc) — no wall-clock sample.
const NOW_MS: u64 = 0;

fn toast_for(severity: ToastSeverity) -> ToastType {
    match severity {
        ToastSeverity::Info => ToastType::info("Saved successfully"),
        ToastSeverity::Success => ToastType::success("Changes applied"),
        ToastSeverity::Warning => ToastType::warning("Check your input"),
        ToastSeverity::Error => ToastType::error("Something went wrong"),
    }
}

/// `(state name, severity)` — named after `ToastSeverity`'s own variants,
/// the widget's entire visual axis (module doc).
const STATES: &[(&str, ToastSeverity)] = &[
    ("info", ToastSeverity::Info),
    ("success", ToastSeverity::Success),
    ("warning", ToastSeverity::Warning),
    ("error", ToastSeverity::Error),
];

fn render(set: BuiltinSet, severity: ToastSeverity) -> TinySkiaCpuRenderContext {
    let mut ctx = support::canvas(340, 84, set);
    let theme = TokenTheme::new(Tokens::builtin(set));
    let entry = ToastEntry { toast: toast_for(severity) };
    toast::draw_toast_at(&mut ctx, 8.0, 8.0, &entry, &theme, NOW_MS);
    ctx
}

#[test]
fn toast_matches_golden_grid() {
    let mut sheet = ReviewSheet::new();
    support::for_each_set(|set, _tokens| {
        for &(state_name, severity) in STATES {
            let ctx = render(set, severity);
            support::golden("toast", set, state_name, &ctx).expect("toast golden mismatch");
            sheet.add(set, state_name, &ctx);
        }
    });
    sheet.save("toast");
}
