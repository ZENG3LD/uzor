//! Append-only event log of agent-visible state transitions.
//!
//! `LayoutManager` owns one ring buffer.  Anyone holding `&mut
//! LayoutManager` can push entries — LM internals, the app layer, and
//! blackbox-panel handlers all write into the same feed.  Entries
//! carry a free-form `category` string so callers organise their
//! breadcrumbs by namespace:
//!
//! - `lm.*`    — LM internals (click, dispatch, style, sync_mode,
//!               window_attach, overlay_toggle …)
//! - `app.*`   — app code (theme_changed, symbol_changed,
//!               selection_changed …)
//! - `chart.*`, `dom.*`, etc — blackbox-panel handlers
//!
//! Agents poll `GET /log?since=<seq>&prefix=<category-prefix>` and
//! see one merged story without resorting to screenshots.

use serde::Serialize;
use serde_json::Value;
use std::collections::VecDeque;

/// Capacity of the ring buffer.  Tuned high enough for long agent
/// sessions but low enough that the JSON `/log` payload stays small.
pub const DEFAULT_LOG_CAPACITY: usize = 4096;

#[derive(Debug, Clone, Serialize)]
pub struct AgentLogEntry {
    /// Strictly increasing sequence number.  Pass back in `?since=`
    /// to skip already-seen entries.
    pub seq: u64,
    /// Milliseconds since runtime start.
    pub ts_ms: f64,
    /// Window key the event affected, when applicable.
    pub window: Option<String>,
    /// Free-form namespace.  Convention: dotted lowercase, e.g.
    /// `"lm.click"`, `"app.theme.changed"`, `"chart.crosshair"`.
    pub category: String,
    /// Arbitrary JSON payload — shape is per-category.
    pub payload: Value,
}

#[derive(Debug, Clone)]
pub struct AgentLog {
    entries: VecDeque<AgentLogEntry>,
    next_seq: u64,
    capacity: usize,
}

impl AgentLog {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(capacity),
            next_seq: 1,
            capacity,
        }
    }

    /// Append one entry.  Caller supplies `ts_ms`, `window`, the
    /// category, and the payload.  We tag with `seq` and trim to
    /// capacity.
    pub fn push(
        &mut self,
        ts_ms: f64,
        window: Option<String>,
        category: impl Into<String>,
        payload: Value,
    ) -> u64 {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        if self.entries.len() == self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(AgentLogEntry {
            seq,
            ts_ms,
            window,
            category: category.into(),
            payload,
        });
        seq
    }

    /// Highest seq currently visible.
    pub fn head_seq(&self) -> u64 {
        self.entries.back().map(|e| e.seq).unwrap_or(0)
    }

    /// Drain entries with `seq > since`, capped at `limit`, optionally
    /// filtered by category prefix.
    pub fn since(
        &self,
        since: u64,
        limit: usize,
        prefix: Option<&str>,
    ) -> Vec<AgentLogEntry> {
        self.entries
            .iter()
            .filter(|e| e.seq > since)
            .filter(|e| match prefix {
                Some(p) => e.category.starts_with(p),
                None => true,
            })
            .take(limit)
            .cloned()
            .collect()
    }

    /// Tail of the log, last `n` entries.
    pub fn tail(&self, n: usize) -> Vec<AgentLogEntry> {
        let len = self.entries.len();
        let start = len.saturating_sub(n);
        self.entries.iter().skip(start).cloned().collect()
    }

    /// Snapshot of every entry currently in the buffer.  Used by
    /// platform layers that mirror the log into a separate
    /// `Arc<RwLock<Vec<AgentLogEntry>>>` for the HTTP shim.
    pub fn snapshot(&self) -> Vec<AgentLogEntry> {
        self.entries.iter().cloned().collect()
    }
}

impl Default for AgentLog {
    fn default() -> Self {
        Self::new(DEFAULT_LOG_CAPACITY)
    }
}
