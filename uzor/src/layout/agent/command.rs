//! Write-side command vocabulary.
//!
//! HTTP shims (`uzor-agent-api`) decode JSON into [`Command`] and pass
//! it to the platform window manager which forwards to either
//! [`super::LmAgent`] or its own override (e.g. screenshot).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Command {
    // ── synthetic input (pixel mode) ────────────────────────────────
    /// Pointer-move at `(x, y)` in the named window.
    InjectHover { window: String, x: f64, y: f64 },
    /// Full pointer-down + pointer-up at `(x, y)`.
    InjectClick { window: String, x: f64, y: f64, button: MouseButton },
    /// Wheel scroll (`dx`, `dy` in logical pixels).
    InjectScroll { window: String, dx: f64, dy: f64 },

    // ── semantic / direct LM ops ────────────────────────────────────
    /// Resolve `widget_id` to its rect via the branch's layout tree
    /// and synthesise a click at its centre.  Detect-friendly: agents
    /// don't track pixel positions.
    ClickWidget { window: String, widget_id: String },
    /// Hover over a widget by id.
    HoverWidget { window: String, widget_id: String },

    /// Open / close composite-state slots by id.
    OpenModal      { window: String, modal_id: String },
    CloseModal     { window: String, modal_id: String },
    OpenPopup      { window: String, popup_id: String },
    ClosePopup     { window: String, popup_id: String },
    OpenDropdown   { window: String, dropdown_id: String },
    CloseDropdown  { window: String, dropdown_id: String },
    ToggleSidebar  { window: String, sidebar_id: String },

    // ── window lifecycle ────────────────────────────────────────────
    SpawnWindow {
        key: String,
        title: String,
        width: u32,
        height: u32,
        background: Option<u32>,
        decorations: Option<bool>,
    },
    CloseWindow { key: String },

    // ── LM-root ops ─────────────────────────────────────────────────
    SetSyncMode {
        node_id: String,
        mode: String,        // "synced" / "sometimes_alone" / "sometimes_group" / "standalone"
        group_id: Option<u64>,
    },
    ApplyStylePreset { name: String },

    // ── blackbox routing ────────────────────────────────────────────
    /// Synthetic click on a mini-widget published by a blackbox.
    BlackboxClickWidget { window: String, slot_id: String, sub_id: String },

    /// Free-form agent-log push.  Used by the HTTP shim to record
    /// `<slot>.<action>` entries after a blackbox action returns
    /// with `log_payload`, and by external tooling that wants to
    /// drop breadcrumbs into the merged feed.
    LogPush {
        category: String,
        #[serde(default)]
        payload: serde_json::Value,
        #[serde(default)]
        window: Option<String>,
    },

    /// Change the baseline tick rate of a window.  `mode` is
    /// `"dirty"`, `"capped"`, or `"uncapped"`.  When `mode == "capped"`,
    /// `fps` is required.
    SetTickRate {
        window: String,
        mode: String,
        #[serde(default)]
        fps: Option<u32>,
    },
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommandReply {
    pub ok: bool,
    pub message: Option<String>,
}

impl CommandReply {
    pub fn ok() -> Self {
        Self { ok: true, message: None }
    }
    pub fn err(msg: impl Into<String>) -> Self {
        Self { ok: false, message: Some(msg.into()) }
    }
}
