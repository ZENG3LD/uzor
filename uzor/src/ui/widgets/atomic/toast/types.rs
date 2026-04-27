//! Toast variant catalog. Layout (rect) is layout-layer concern.

use crate::input::Sense;
use crate::ui::widgets::WidgetCapabilities;

/// Visual severity — uzor extends beyond mlc's single variant.
/// mlc has one visual (Info/blue); Success/Warning/Error are uzor additions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastSeverity {
    Info,
    Success,
    Warning,
    Error,
}

/// A toast notification instance.
///
/// Mirrors mlc's `ToastNotification` struct exactly, plus `severity` (uzor extension).
/// `title` and `timestamp_ms` come from mlc; `severity` and the four constructors
/// are uzor additions.
#[derive(Debug, Clone, PartialEq)]
pub struct ToastType {
    /// Short heading shown above the message (mlc field).
    pub title: Option<String>,
    /// Body text.
    pub message: String,
    /// Unix epoch milliseconds when this toast was created (mlc field).
    pub timestamp_ms: u64,
    /// Display duration in milliseconds.
    pub duration_ms: u64,
    /// Visual severity — controls accent colour.
    pub severity: ToastSeverity,
}

impl WidgetCapabilities for ToastType {
    fn sense(&self) -> Sense {
        Sense::HOVER
    }
}

impl ToastType {
    // ── Severity constructors (uzor) ─────────────────────────────────────────

    pub fn info(message: impl Into<String>) -> Self {
        Self {
            title: None,
            message: message.into(),
            timestamp_ms: 0,
            duration_ms: 3_000,
            severity: ToastSeverity::Info,
        }
    }

    pub fn success(message: impl Into<String>) -> Self {
        Self {
            title: None,
            message: message.into(),
            timestamp_ms: 0,
            duration_ms: 3_000,
            severity: ToastSeverity::Success,
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            title: None,
            message: message.into(),
            timestamp_ms: 0,
            duration_ms: 4_000,
            severity: ToastSeverity::Warning,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            title: None,
            message: message.into(),
            timestamp_ms: 0,
            duration_ms: 5_000,
            severity: ToastSeverity::Error,
        }
    }

    // ── mlc use-case constructors ────────────────────────────────────────────

    /// Alert fired by the delivery engine — 5 000 ms (mlc hardcoded value).
    pub fn alert_fired(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: Some(title.into()),
            message: message.into(),
            timestamp_ms: 0,
            duration_ms: 5_000,
            severity: ToastSeverity::Info,
        }
    }

    /// OTA update available — 8 000 ms (mlc hardcoded value).
    pub fn ota_update(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: Some(title.into()),
            message: message.into(),
            timestamp_ms: 0,
            duration_ms: 8_000,
            severity: ToastSeverity::Info,
        }
    }

    // ── Lifecycle helpers (mirroring mlc's `ToastNotification` methods) ──────

    /// Returns `true` if the toast has fully expired.
    pub fn is_expired(&self, now_ms: u64) -> bool {
        now_ms > self.timestamp_ms + self.duration_ms
    }

    /// Remaining display fraction: `1.0` = just appeared, `0.0` = fully expired.
    /// Mirrors mlc `remaining_fraction` exactly.
    pub fn remaining_fraction(&self, now_ms: u64) -> f64 {
        if now_ms >= self.timestamp_ms + self.duration_ms {
            return 0.0;
        }
        if now_ms <= self.timestamp_ms {
            return 1.0;
        }
        let elapsed = now_ms - self.timestamp_ms;
        1.0 - (elapsed as f64 / self.duration_ms as f64)
    }
}
