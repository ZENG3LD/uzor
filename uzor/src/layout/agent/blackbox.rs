//! Agent surface trait every blackbox panel implements to expose
//! itself to external agents.
//!
//! ## What blackbox publishes
//!
//! - **Mini widget tree** — clickable hot-zones inside the blackbox
//!   body (toolbar buttons, separators, indicator chips).  Each one
//!   has a stable `sub_id` so the agent can address it.
//! - **Internal state snapshot** — domain data the blackbox owns
//!   (chart symbol, crosshair position, drawing tools, …).  Free-form
//!   JSON, the agent doesn't need to know the schema.
//! - **Typed actions** — semantic operations the agent invokes
//!   (`set_symbol`, `add_indicator`, `scroll_to_timestamp`, …).
//!
//! ## What LM sees
//!
//! Only the blackbox **rect** (it placed it in the dock) and the
//! agent log feed (the blackbox writes breadcrumbs through
//! `LayoutManager::agent_log_push`).  LM is a *router* for HTTP
//! requests addressed by `slot_id`, not a registry that merges
//! blackbox internals into its own widget tree.
//!
//! ## Threading
//!
//! The HTTP shim runs on a separate tokio thread.  The blackbox
//! handler lives on the winit thread.  Registry stores
//! `Arc<Mutex<dyn BlackboxAgentSurface>>` so HTTP handlers can
//! lock briefly to read state / dispatch actions.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::types::Rect;

// ── Mini-widget published by a blackbox ─────────────────────────────

/// One agent-addressable hot-zone inside a blackbox body.
///
/// `sub_id` is local to the blackbox.  The HTTP layer prepends
/// `<slot_id>:` so external agents see a stable global id like
/// `"chart-btc:btn:zoom_in"`.
#[derive(Debug, Clone, Serialize)]
pub struct AgentWidget {
    pub sub_id: String,
    /// Free-form classification: `"button"`, `"separator"`, `"toggle"`,
    /// `"indicator"`, `"tab"`, etc.  No semantics — purely advisory
    /// for agents that want to filter.
    pub kind: String,
    /// Screen-space rect (after the blackbox `rect` is solved).
    pub rect: Rect,
    /// Optional human-readable label (`"Zoom In"`, `"Crosshair"`).
    pub label: Option<String>,
    /// Free-form metadata (e.g. `{"checked": true}` for toggles).
    #[serde(default)]
    pub meta: Value,
}

// ── Action / reply ──────────────────────────────────────────────────

/// Typed operation routed to a blackbox via
/// `POST /blackbox/<slot>/action`.
#[derive(Debug, Clone, Deserialize)]
pub struct AgentAction {
    /// Action name agreed between the blackbox and its agent users
    /// (e.g. `"set_symbol"`, `"toggle_crosshair"`, `"add_ma"`).
    pub name: String,
    /// Free-form arguments.
    #[serde(default)]
    pub args: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentActionReply {
    pub ok: bool,
    pub message: Option<String>,
    /// Optional payload merged by the framework into a
    /// `<slot_id>.<action.name>` agent-log entry so external observers
    /// can replay cause-and-effect.  `None` means "don't write a log
    /// entry beyond the standard one".
    pub log_payload: Option<Value>,
}

impl AgentActionReply {
    pub fn ok() -> Self {
        Self { ok: true, message: None, log_payload: None }
    }
    pub fn ok_with_log(payload: Value) -> Self {
        Self { ok: true, message: None, log_payload: Some(payload) }
    }
    pub fn err(msg: impl Into<String>) -> Self {
        Self { ok: false, message: Some(msg.into()), log_payload: None }
    }
}

// ── The trait blackbox panels implement ─────────────────────────────

/// Implemented on the long-lived state behind a blackbox panel
/// (e.g. `ChartHandler`, `DomState`, `DrawingPanel`).
///
/// Required for any blackbox that wants agent control.  Blackboxes
/// without this trait still render and accept input normally — they
/// just don't participate in the agent control plane beyond the
/// coarse "click in my rect at (x, y)" path.
pub trait BlackboxAgentSurface: Send + 'static {
    /// Stable, unique identifier the agent uses to address this
    /// blackbox over HTTP (`/blackbox/<slot_id>/...`).
    fn agent_slot_id(&self) -> &str;

    /// Free-form name agents may use to disambiguate similar slots
    /// (e.g. `"chart"`, `"dom"`, `"footprint"`).  Defaults to the
    /// slot id.
    fn agent_kind(&self) -> &str { self.agent_slot_id() }

    /// Mini widget list the agent can iterate / click on.  Returns
    /// rects in **screen-space** (already offset by the blackbox's
    /// own rect) so `/lm/click_widget`-style synthetic clicks land
    /// in the right place.
    fn list_agent_widgets(&self) -> Vec<AgentWidget> { Vec::new() }

    /// Snapshot of internal state for `GET /blackbox/<slot>/state`.
    /// Free-form — chart returns `{symbol, crosshair, …}`, DOM
    /// returns `{symbol, levels, depth}`, etc.
    fn agent_state(&self) -> Value { Value::Null }

    /// Apply a typed semantic action.  Default rejects everything.
    fn apply_agent_action(&mut self, _action: AgentAction) -> AgentActionReply {
        AgentActionReply::err("blackbox does not implement any actions")
    }

    /// Resolve a `sub_id` to a synthetic click.  Default returns the
    /// rect of the matching mini-widget (if any) so the framework
    /// can emit a coordinate click.  Override only if the blackbox
    /// wants to bypass the coordinate path.
    fn resolve_click_widget(&self, sub_id: &str) -> Option<Rect> {
        self.list_agent_widgets()
            .into_iter()
            .find(|w| w.sub_id == sub_id)
            .map(|w| w.rect)
    }
}
