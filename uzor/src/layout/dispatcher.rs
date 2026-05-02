//! Dispatch table for L3 LayoutManager.
//!
//! Composites and the app register patterns against `WidgetId`s; when a
//! click resolves to a `WidgetId`, the dispatcher walks the table and emits
//! a high-level [`DispatchEvent`]. The app matches on the enum instead of
//! parsing raw id strings.
//!
//! # Why this lives in `layout`, not in app code
//!
//! - `InputCoordinator` (L1) returns a `WidgetId` from `process_click`.
//! - `LayoutManager` (L3) owns the coord and knows which composites it
//!   registered. So it's the right place to decide what a hit on
//!   `"modal-widget:close"` semantically means.
//! - The app no longer writes `if id_str == "modal-widget:close" { ... }`
//!   500 times. It matches on `DispatchEvent::ModalCloseRequested`.
//!
//! # How patterns work
//!
//! Two flavours:
//! - **Exact**: `"modal-widget:close"` — fires only on that exact id.
//! - **Prefix**: `"dd-help-widget:item:"` — fires on any id that starts with
//!   it; the suffix after the prefix is passed to the event constructor.
//!
//! Exact patterns win over prefix patterns when both could match the same
//! id. Within the same flavour, **last-registered wins** — the app can
//! override a composite's default handler by registering its own pattern
//! after `register_layout_manager_*`.

use crate::types::WidgetId;

/// High-level events surfaced to the app after a click is dispatched.
///
/// Composites populate the table with their own patterns and the
/// corresponding event constructor; the app matches on this enum.
///
/// # Adding a new event kind
///
/// 1. Add a variant here.
/// 2. Add a matching `EventBuilder` variant in [`EventBuilder`].
/// 3. Register the pattern from your composite's `register_*` helper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchEvent {
    /// User clicked a registered widget that has no semantic handler.
    /// The app may still react via raw id matching if it wants.
    Unhandled(WidgetId),

    /// User clicked the close-X / a footer button on a modal.
    /// Carries the modal composite's WidgetId so multi-modal apps
    /// can identify which one.
    ModalCloseRequested(WidgetId),

    /// User clicked a TopTabs / SideTabs tab inside a modal.
    /// `index` is the tab index registered with the composite.
    ModalTabClicked { modal_id: WidgetId, index: usize },

    /// User clicked the Wizard "Next" button.
    ModalWizardNext(WidgetId),

    /// User clicked the Wizard "Back" button.
    ModalWizardBack(WidgetId),

    /// User clicked an item inside a dropdown.
    /// `dropdown_id` — composite WidgetId; `item_id` — application-defined
    /// id string the caller used when building the items list.
    DropdownItemClicked { dropdown_id: WidgetId, item_id: String },

    /// User clicked an item inside a top-level toolbar.
    ToolbarItemClicked { toolbar_id: WidgetId, item_id: String },

    /// User clicked a chrome tab.
    ChromeTabClicked { tab_index: usize },

    /// User clicked an item in a context menu (semantic shortcut for the
    /// common shape of ctx-menu hits).
    ContextMenuItemClicked { menu_id: WidgetId, item_index: usize },

    /// User clicked the scrollbar **track** (jump to that position).
    /// `track_id` lets the app distinguish between multiple scrollbars.
    ScrollbarTrackClicked { track_id: WidgetId },

    /// User started dragging the scrollbar **thumb** (mouse-down on it).
    /// The app should call atomic-scrollbar `start_thumb_drag` on its state
    /// and follow up with `update_thumb_drag` on every mouse-move while the
    /// drag is live.
    ScrollbarThumbDragStarted { thumb_id: WidgetId },

    /// User clicked a navigation chevron — request to advance content one
    /// step in `direction`. Used by overflow-mode `Chevrons` for sidebars,
    /// toolbars, modals, popups and similar containers.
    /// `chevron_id` lets the app distinguish multiple chevron sites.
    ChevronStepRequested {
        chevron_id: WidgetId,
        direction:  super::ChevronStepDirection,
    },

    /// User started dragging a resize handle on a composite (toolbar /
    /// modal / popup / sidebar). The app should capture initial geometry
    /// and consume subsequent mouse-move events to drive the resize.
    ResizeHandleDragStarted {
        /// The composite that owns the handle (e.g. modal / toolbar id).
        host_id: WidgetId,
        /// Which edge / corner is being grabbed.
        edge:    ResizeEdge,
    },
}

/// Edges and corners a resize handle can be attached to. Used by
/// `DispatchEvent::ResizeHandleDragStarted` so the app knows which
/// dimension(s) to scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResizeEdge {
    /// Top edge — vertical drag, grows / shrinks from the top.
    N,
    /// Bottom edge — vertical drag, grows / shrinks from the bottom.
    S,
    /// Left edge — horizontal drag, grows / shrinks from the left.
    W,
    /// Right edge — horizontal drag, grows / shrinks from the right.
    E,
    /// Top-left corner — both axes.
    NW,
    /// Top-right corner — both axes.
    NE,
    /// Bottom-left corner — both axes.
    SW,
    /// Bottom-right corner — both axes.
    SE,
}

/// Direction passed inside `DispatchEvent::ChevronStepRequested`. Mirrors
/// the atomic chevron's directions; isolated from the atomic so the
/// dispatcher module has no cyclical dep on the widget tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChevronStepDirection {
    Up,
    Down,
    Left,
    Right,
}

/// How to construct a [`DispatchEvent`] when a pattern matches.
///
/// Each variant captures whatever extra context the composite supplied at
/// registration time (the composite's own `WidgetId`, sometimes a parsed
/// integer index, etc.).
#[derive(Clone)]
pub enum EventBuilder {
    /// Fires `ModalCloseRequested(modal_id)` on match.
    ModalClose { modal_id: WidgetId },

    /// Fires `ModalTabClicked { modal_id, index }` — `index` parsed from the
    /// suffix after the prefix as a `usize`. If parse fails, falls through.
    ModalTabFromSuffix { modal_id: WidgetId },

    /// Fires `ModalWizardNext(modal_id)`.
    ModalWizardNext { modal_id: WidgetId },

    /// Fires `ModalWizardBack(modal_id)`.
    ModalWizardBack { modal_id: WidgetId },

    /// Fires `DropdownItemClicked { dropdown_id, item_id = suffix }`.
    DropdownItem { dropdown_id: WidgetId },

    /// Fires `ToolbarItemClicked { toolbar_id, item_id = suffix }`.
    ToolbarItem { toolbar_id: WidgetId },

    /// Fires `ChromeTabClicked { tab_index = parsed suffix }`.
    ChromeTabFromSuffix,

    /// Fires `ContextMenuItemClicked { menu_id, item_index = parsed suffix }`.
    ContextMenuItem { menu_id: WidgetId },

    /// Fires `ScrollbarTrackClicked { track_id }` when the user clicks the
    /// track of a scrollbar (i.e. jump-to-position).
    ScrollbarTrack { track_id: WidgetId },

    /// Fires `ScrollbarThumbDragStarted { thumb_id }` when the user
    /// mouse-downs on a scrollbar thumb.
    ScrollbarThumb { thumb_id: WidgetId },

    /// Fires `ChevronStepRequested { chevron_id, direction }` — used by
    /// overflow-mode `Chevrons` paging strips.
    ChevronStep { chevron_id: WidgetId, direction: super::ChevronStepDirection },

    /// Fires `ResizeHandleDragStarted { host_id, edge }` when a resize
    /// handle is grabbed on the composite identified by `host_id`.
    ResizeHandle { host_id: WidgetId, edge: super::ResizeEdge },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Match {
    Exact,
    Prefix,
}

#[derive(Clone)]
struct Entry {
    pattern: String,
    kind: Match,
    builder: EventBuilder,
}

/// Dispatch table embedded in [`LayoutManager`](super::manager::LayoutManager).
///
/// Composites add patterns at `register_*` time; the app may add overrides.
/// Patterns are matched in **registration order, exact-first** — the first
/// exact match wins, then the first prefix match wins. Adding a more
/// specific pattern after a generic one will take precedence (since `Match`
/// is checked exact-first regardless of registration order).
#[derive(Default, Clone)]
pub struct ClickDispatcher {
    entries: Vec<Entry>,
}

impl ClickDispatcher {
    /// Create an empty dispatcher.
    pub fn new() -> Self {
        Self::default()
    }

    /// Forget every registered pattern.
    ///
    /// `LayoutManager` calls this at the start of each frame so composites
    /// can re-register their handlers cleanly. App-level handlers must also
    /// be re-added every frame (same model as the rest of the immediate-mode
    /// composite registration).
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Register an exact-id pattern.
    ///
    /// The handler fires only when `clicked_id == pattern`.
    pub fn on_exact(&mut self, pattern: impl Into<String>, builder: EventBuilder) {
        self.entries.push(Entry {
            pattern: pattern.into(),
            kind: Match::Exact,
            builder,
        });
    }

    /// Register a prefix pattern.
    ///
    /// The handler fires when `clicked_id.starts_with(prefix)`. The portion
    /// after `prefix` is the "suffix" passed to the event constructor.
    pub fn on_prefix(&mut self, prefix: impl Into<String>, builder: EventBuilder) {
        self.entries.push(Entry {
            pattern: prefix.into(),
            kind: Match::Prefix,
            builder,
        });
    }

    /// Resolve a clicked WidgetId to a semantic event.
    ///
    /// Returns `Some(event)` if any pattern matched. Returns `None` when no
    /// pattern matched — caller may treat that as a miss (close menus on
    /// outside-click, etc.).
    pub fn dispatch(&self, clicked: &WidgetId) -> Option<DispatchEvent> {
        let id = clicked.0.as_str();

        // Exact patterns win over prefix patterns regardless of registration
        // order. This means a composite's "{id}:close" exact handler
        // beats a generic "{id}:" prefix handler an app might add later.
        for entry in &self.entries {
            if entry.kind == Match::Exact && entry.pattern == id {
                return Some(build(&entry.builder, id, &entry.pattern));
            }
        }
        for entry in &self.entries {
            if entry.kind == Match::Prefix && id.starts_with(&entry.pattern) {
                return Some(build(&entry.builder, id, &entry.pattern));
            }
        }
        None
    }
}

/// Run an [`EventBuilder`] for a given clicked id and the pattern that matched.
fn build(builder: &EventBuilder, id: &str, pattern: &str) -> DispatchEvent {
    let suffix = || id.strip_prefix(pattern).unwrap_or("").to_owned();
    match builder {
        EventBuilder::ModalClose { modal_id } => {
            DispatchEvent::ModalCloseRequested(modal_id.clone())
        }
        EventBuilder::ModalTabFromSuffix { modal_id } => {
            match suffix().parse::<usize>() {
                Ok(index) => DispatchEvent::ModalTabClicked { modal_id: modal_id.clone(), index },
                Err(_)    => DispatchEvent::Unhandled(WidgetId::new(id)),
            }
        }
        EventBuilder::ModalWizardNext { modal_id } => {
            DispatchEvent::ModalWizardNext(modal_id.clone())
        }
        EventBuilder::ModalWizardBack { modal_id } => {
            DispatchEvent::ModalWizardBack(modal_id.clone())
        }
        EventBuilder::DropdownItem { dropdown_id } => {
            DispatchEvent::DropdownItemClicked {
                dropdown_id: dropdown_id.clone(),
                item_id: suffix(),
            }
        }
        EventBuilder::ToolbarItem { toolbar_id } => {
            DispatchEvent::ToolbarItemClicked {
                toolbar_id: toolbar_id.clone(),
                item_id: suffix(),
            }
        }
        EventBuilder::ChromeTabFromSuffix => {
            match suffix().parse::<usize>() {
                Ok(tab_index) => DispatchEvent::ChromeTabClicked { tab_index },
                Err(_)        => DispatchEvent::Unhandled(WidgetId::new(id)),
            }
        }
        EventBuilder::ContextMenuItem { menu_id } => {
            match suffix().parse::<usize>() {
                Ok(item_index) => DispatchEvent::ContextMenuItemClicked {
                    menu_id: menu_id.clone(),
                    item_index,
                },
                Err(_) => DispatchEvent::Unhandled(WidgetId::new(id)),
            }
        }
        EventBuilder::ScrollbarTrack { track_id } => {
            DispatchEvent::ScrollbarTrackClicked { track_id: track_id.clone() }
        }
        EventBuilder::ScrollbarThumb { thumb_id } => {
            DispatchEvent::ScrollbarThumbDragStarted { thumb_id: thumb_id.clone() }
        }
        EventBuilder::ChevronStep { chevron_id, direction } => {
            DispatchEvent::ChevronStepRequested {
                chevron_id: chevron_id.clone(),
                direction: *direction,
            }
        }
        EventBuilder::ResizeHandle { host_id, edge } => {
            DispatchEvent::ResizeHandleDragStarted {
                host_id: host_id.clone(),
                edge: *edge,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_beats_prefix_regardless_of_order() {
        let mut d = ClickDispatcher::new();
        d.on_prefix(
            "m:",
            EventBuilder::DropdownItem { dropdown_id: WidgetId::new("m") },
        );
        d.on_exact(
            "m:close",
            EventBuilder::ModalClose { modal_id: WidgetId::new("m") },
        );

        let ev = d.dispatch(&WidgetId::new("m:close")).unwrap();
        assert_eq!(ev, DispatchEvent::ModalCloseRequested(WidgetId::new("m")));
    }

    #[test]
    fn prefix_passes_suffix() {
        let mut d = ClickDispatcher::new();
        d.on_prefix(
            "dd:item:",
            EventBuilder::DropdownItem { dropdown_id: WidgetId::new("dd") },
        );

        let ev = d.dispatch(&WidgetId::new("dd:item:save")).unwrap();
        assert_eq!(
            ev,
            DispatchEvent::DropdownItemClicked {
                dropdown_id: WidgetId::new("dd"),
                item_id: "save".to_string(),
            },
        );
    }

    #[test]
    fn miss_returns_none() {
        let d = ClickDispatcher::new();
        assert_eq!(d.dispatch(&WidgetId::new("nope")), None);
    }

    #[test]
    fn parse_error_falls_to_unhandled() {
        let mut d = ClickDispatcher::new();
        d.on_prefix(
            "m:tab:",
            EventBuilder::ModalTabFromSuffix { modal_id: WidgetId::new("m") },
        );

        let ev = d.dispatch(&WidgetId::new("m:tab:notanumber")).unwrap();
        assert_eq!(ev, DispatchEvent::Unhandled(WidgetId::new("m:tab:notanumber")));
    }
}
