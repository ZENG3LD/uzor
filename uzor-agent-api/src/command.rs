//! Write-side command vocabulary.
//!
//! HTTP `POST` endpoints decode JSON into a [`Command`], hand it to the
//! window manager (via [`super::AgentControl::dispatch`]), and wait on
//! the returned oneshot for the [`CommandReply`].  The window manager
//! drains the queue on the next tick so commands always apply on the
//! winit / event-loop thread.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Command {
    // ── synthetic input ─────────────────────────────────────────────
    /// Inject a pointer-move event at `(x, y)` in the named window.
    InjectHover { window: String, x: f64, y: f64 },
    /// Inject a pointer-down then pointer-up (full click) at `(x, y)`.
    InjectClick { window: String, x: f64, y: f64, button: MouseButton },
    /// Inject a wheel scroll (`dx`, `dy` in logical pixels).
    InjectScroll { window: String, dx: f64, dy: f64 },

    // ── window lifecycle ────────────────────────────────────────────
    /// Spawn a new window.  `key` must be unique.
    SpawnWindow {
        key: String,
        title: String,
        width: u32,
        height: u32,
        background: Option<u32>,
        decorations: Option<bool>,
    },
    /// Close the named window.
    CloseWindow { key: String },

    // ── direct LM ops (incremental — added as needed) ───────────────
    /// Promote / demote a sync-tagged node.  `mode` is one of
    /// `"synced"`, `"sometimes_alone"`, `"sometimes_group"`,
    /// `"standalone"`.  When `mode == "sometimes_group"`, `group_id`
    /// must be `Some(u64)`.
    SetSyncMode {
        node_id: String,
        mode: String,
        group_id: Option<u64>,
    },
    /// Apply a built-in style preset.  Known names: `"mirage_dark"`,
    /// `"mirage_light"`.
    ApplyStylePreset { name: String },
}

#[derive(Debug, Clone, Copy, Deserialize)]
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
