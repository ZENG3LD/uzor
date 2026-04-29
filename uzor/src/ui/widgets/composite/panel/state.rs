//! Panel persistent state.
//!
//! `PanelState` is flat — fields irrelevant to the active `PanelRenderKind`
//! are simply never touched.

use crate::core::types::state::ScrollState;

// ---------------------------------------------------------------------------
// PanelState
// ---------------------------------------------------------------------------

/// All per-panel-instance persistent state.
#[derive(Debug, Clone)]
pub struct PanelState {
    // --- Scroll ---

    /// Scroll state (offset + drag tracking).
    pub scroll: ScrollState,

    // --- Sort (column-header click) ---

    /// Id of the currently sorted column.  `None` = unsorted.
    pub sort_column: Option<String>,

    /// `true` = ascending order, `false` = descending.
    pub sort_ascending: bool,

    // --- Filter ---

    /// Active filter string.  `None` = no filter applied.
    pub active_filter: Option<String>,

    // --- Hover tracking ---

    /// Id of the header action button the pointer is currently hovering over.
    pub hovered_action: Option<String>,

    /// Id of the column-header cell the pointer is currently hovering over.
    pub hovered_column: Option<String>,
}

impl Default for PanelState {
    fn default() -> Self {
        Self {
            scroll: ScrollState::default(),
            sort_column: None,
            sort_ascending: true,
            active_filter: None,
            hovered_action: None,
            hovered_column: None,
        }
    }
}

impl PanelState {
    // -------------------------------------------------------------------------
    // Sort helpers
    // -------------------------------------------------------------------------

    /// Toggle sort on `column_id`.
    ///
    /// - If `column_id` is already the sort column → flip `sort_ascending`.
    /// - Otherwise → set `sort_column = Some(column_id)`, `sort_ascending = true`.
    pub fn toggle_sort(&mut self, column_id: impl Into<String>) {
        let id = column_id.into();
        if self.sort_column.as_deref() == Some(&id) {
            self.sort_ascending = !self.sort_ascending;
        } else {
            self.sort_column = Some(id);
            self.sort_ascending = true;
        }
    }

    // -------------------------------------------------------------------------
    // Filter helpers
    // -------------------------------------------------------------------------

    /// Set the active filter string.  Pass `None` to clear.
    pub fn set_filter(&mut self, filter: Option<impl Into<String>>) {
        self.active_filter = filter.map(Into::into);
    }
}
