//! ContextMenu type definitions — per-frame view data and render kind enum.
//!
//! Ported from the mlc deep audit in `context-menu-deep.md`.
//! Two structurally-distinct render kinds: `Default` (icons + blur) and
//! `Minimal` (chrome-style: no icons, no blur).

use super::settings::ContextMenuSettings;
use super::state::ContextMenuState;
use crate::render::RenderContext;
use crate::types::Rect;

// ---------------------------------------------------------------------------
// ContextMenuItem
// ---------------------------------------------------------------------------

/// A single row in the context menu.
pub struct ContextMenuItem<'a> {
    /// Stable action identifier — returned to caller on activation.
    pub action: &'a str,

    /// Display label text.
    pub label: &'a str,

    /// Optional icon identifier string (resolved to an icon by the renderer).
    /// Relevant for `Default` kind; ignored by `Minimal`.
    pub icon: Option<&'a str>,

    /// Whether this item renders in danger style (red text).
    pub danger: bool,

    /// Whether a separator line is drawn after this item.
    pub separator_after: bool,

    /// Whether this item is enabled (disabled items show dimmed text, no hover).
    pub enabled: bool,
}

// ---------------------------------------------------------------------------
// ContextMenuView
// ---------------------------------------------------------------------------

/// Per-frame data handed to `register_*_context_menu`.
pub struct ContextMenuView<'a> {
    /// Ordered item rows.
    pub items: &'a [ContextMenuItem<'a>],

    /// Opaque identifier of the object that was right-clicked.
    /// Stored in `ContextMenuState.target_id`; caller interprets semantics.
    pub target_id: Option<&'a str>,

    /// Optional title row shown above the items (rare — chrome menus omit this).
    pub title: Option<&'a str>,
}

// ---------------------------------------------------------------------------
// ContextMenuRenderKind
// ---------------------------------------------------------------------------

/// Layout and rendering strategy for the context menu.
pub enum ContextMenuRenderKind<'a> {
    /// Chart drawing tools style:
    /// icon column, separators, blur background, 32 px items, ~180 px wide.
    Default,

    /// Chrome / minimal style:
    /// no icons, no separators, no blur, 28 px items, ~160 px wide.
    Minimal,

    /// Escape hatch — caller supplies full draw closure.
    /// The composite provides NO frame; the closure draws everything.
    Custom(
        Box<
            dyn Fn(
                    &mut dyn RenderContext,
                    Rect,
                    &ContextMenuState,
                    &ContextMenuView<'_>,
                    &ContextMenuSettings,
                ) + 'a,
        >,
    ),
}
