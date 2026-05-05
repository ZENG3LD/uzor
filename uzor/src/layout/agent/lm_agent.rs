//! [`LmAgent`] — default implementations of every agent op routable
//! purely through `LayoutManager`.
//!
//! Window managers call these helpers from their own command-drain
//! pass.  The functions all take `&LayoutManager<P>` for reads or
//! `&mut LayoutManager<P>` for writes — never own state of their own.
//!
//! Operations that *cannot* be performed through LM alone (screenshot,
//! GPU readback, OS-level window spawn / close, real synthetic OS
//! input) live on the WM side; this module simply ignores those
//! commands and the WM handles them before / after delegating here.

use crate::core::types::Rect;
use crate::layout::docking::DockPanel;
use crate::layout::sync::{SyncGroupId, SyncMode};
use crate::layout::{LayoutManager, LayoutNode as TreeNode, MirageDarkPreset, MirageLightPreset};
use crate::types::WidgetId;

use super::command::{Command, CommandReply};
use super::snapshot::{
    AgentSnapshot, BranchSnapshot, ClickSnap, NodeSyncSnapshot, RectSnap, RootSnapshot,
    WidgetSnapshot,
};

/// Bundle of pure helpers; not constructed at runtime.
pub struct LmAgent<P: DockPanel>(std::marker::PhantomData<P>);

impl<P: DockPanel> LmAgent<P> {
    // ── snapshot builders ────────────────────────────────────────────

    pub fn build_snapshot(
        layout: &LayoutManager<P>,
        fps_ema: f32,
        frame_count: u64,
        frame_time_ms: f64,
    ) -> AgentSnapshot {
        let current_window = layout.current_window().map(|k| k.as_str().to_owned());
        let window_count = layout.window_keys().count();

        let windows: Vec<BranchSnapshot> = layout
            .window_keys()
            .filter_map(|key| {
                let b = layout.window(key)?;
                Some(BranchSnapshot {
                    key: key.as_str().to_owned(),
                    rect: rect_to_snap(b.rect),
                    initialised: b.initialised,
                    chrome_visible: b.chrome.visible,
                    edge_count: b.edges.iter().count(),
                    dock_leaves: b.dock.tree().leaves().len(),
                    overlay_count: b.overlays.entries().len(),
                    modal_count: b.modals.len(),
                    popup_count: b.popups.len(),
                    dropdown_count: b.dropdowns.len(),
                    toolbar_count: b.toolbars.len(),
                    sidebar_count: b.sidebars.len(),
                    context_menu_count: b.context_menus.len(),
                    hovered_widget: b.last_hovered.as_ref().map(|w| w.as_str().to_owned()),
                    pressed_widget: b.last_pressed.as_ref().map(|w| w.as_str().to_owned()),
                    last_click: b.last_click.as_ref().map(|(id, (x, y))| ClickSnap {
                        widget: id.as_str().to_owned(),
                        pos: [*x, *y],
                    }),
                    pointer_pos: b.last_pointer_pos.map(|(x, y)| [x, y]),
                })
            })
            .collect();

        let sync_nodes: Vec<NodeSyncSnapshot> = layout
            .sync_registry()
            .iter()
            .map(|(node_id, mode)| {
                let (label, group_id) = match mode {
                    SyncMode::Synced => ("synced", None),
                    SyncMode::Sometimes(None) => ("sometimes_alone", None),
                    SyncMode::Sometimes(Some(g)) => ("sometimes_group", Some(g.0)),
                    SyncMode::Standalone => ("standalone", None),
                };
                NodeSyncSnapshot {
                    node_id: node_id.to_string(),
                    mode: label.to_string(),
                    group_id,
                }
            })
            .collect();

        AgentSnapshot {
            root: RootSnapshot {
                current_window,
                window_count,
                style_preset: None,
            },
            windows,
            sync_nodes,
            frame_time_ms,
            frame_count,
            fps_ema,
        }
    }

    /// Build a flat widget list across every attached window.
    pub fn build_widget_list(layout: &LayoutManager<P>) -> Vec<WidgetSnapshot> {
        let mut out = Vec::new();
        let keys: Vec<_> = layout.window_keys().cloned().collect();
        for key in keys {
            let Some(b) = layout.window(&key) else { continue };
            for entry in b.tree.entries() {
                if let TreeNode::Widget(w) = &entry.node {
                    out.push(WidgetSnapshot {
                        window: key.as_str().to_owned(),
                        id: w.id.as_str().to_owned(),
                        kind: format!("{:?}", w.kind),
                        rect: rect_to_snap(entry.rect),
                        layer: String::new(),
                    });
                }
            }
        }
        out
    }

    /// Resolve `(window, widget_id)` to its current rect by walking the
    /// branch tree.  Returns `None` if either is missing.
    pub fn widget_rect(
        layout: &LayoutManager<P>,
        window: &str,
        widget_id: &str,
    ) -> Option<Rect> {
        let key = crate::layout::window::WindowKey::new(window);
        let b = layout.window(&key)?;
        for entry in b.tree.entries() {
            if let TreeNode::Widget(w) = &entry.node {
                if w.id.as_str() == widget_id {
                    return Some(entry.rect);
                }
            }
        }
        None
    }

    // ── command application (those LM can answer alone) ─────────────

    /// Apply a command that LM understands.  Returns:
    /// - `Some(reply)` — command was handled (success or LM-level error).
    /// - `None`        — command needs platform-side handling
    ///                   (screenshot, real OS spawn, real input event).
    ///                   The WM should look at it and apply itself.
    pub fn try_apply(layout: &mut LayoutManager<P>, cmd: &Command) -> Option<CommandReply> {
        use crate::layout::window::WindowKey;
        match cmd {
            // ── pure read-from-snapshot inputs (still on WM side) ───
            Command::InjectHover { .. }
            | Command::InjectClick { .. }
            | Command::InjectScroll { .. }
            | Command::SpawnWindow { .. }
            | Command::CloseWindow { .. } => None,

            // ── semantic widget hits ────────────────────────────────
            Command::ClickWidget { window, widget_id } => {
                let key = WindowKey::new(window.clone());
                if !layout.window_keys().any(|k| k == &key) {
                    return Some(CommandReply::err(format!("unknown window {:?}", window)));
                }
                let Some(rect) = Self::widget_rect(layout, window, widget_id) else {
                    return Some(CommandReply::err(format!("unknown widget {:?}", widget_id)));
                };
                let cx = rect.x + rect.width / 2.0;
                let cy = rect.y + rect.height / 2.0;
                layout.set_current_window(key);
                layout.on_pointer_move(cx, cy);
                layout.on_pointer_down(cx, cy);
                let _ = layout.on_pointer_up(cx, cy);
                Some(CommandReply::ok())
            }
            Command::HoverWidget { window, widget_id } => {
                let key = WindowKey::new(window.clone());
                if !layout.window_keys().any(|k| k == &key) {
                    return Some(CommandReply::err(format!("unknown window {:?}", window)));
                }
                let Some(rect) = Self::widget_rect(layout, window, widget_id) else {
                    return Some(CommandReply::err(format!("unknown widget {:?}", widget_id)));
                };
                let cx = rect.x + rect.width / 2.0;
                let cy = rect.y + rect.height / 2.0;
                layout.set_current_window(key);
                layout.on_pointer_move(cx, cy);
                Some(CommandReply::ok())
            }

            // ── modal / popup / dropdown / sidebar toggles ──────────
            Command::OpenModal { window, modal_id } => {
                if let Some(reply) = Self::route(layout, window) { return Some(reply); }
                let h = layout.add_modal(modal_id);
                let _ = layout.modal_mut(&h);
                // `add_modal` ensures the state slot exists; rendering
                // is what makes it visible — apps drive that themselves.
                // For now we simply ensure the slot exists.
                Some(CommandReply::ok())
            }
            Command::CloseModal { window, modal_id } => {
                if let Some(reply) = Self::route(layout, window) { return Some(reply); }
                // We can't directly "close" without app cooperation.
                // Mark the entry by removing its frame from the dispatch
                // registry next frame.  Simplest correct behaviour:
                // remove the persistent state map entry.
                let id = WidgetId::new(modal_id.clone());
                layout.modals_map_mut().remove(&id);
                Some(CommandReply::ok())
            }
            Command::OpenPopup { window, popup_id } => {
                if let Some(reply) = Self::route(layout, window) { return Some(reply); }
                let h = layout.add_popup(popup_id);
                let _ = layout.popup_mut(&h);
                Some(CommandReply::ok())
            }
            Command::ClosePopup { window, popup_id } => {
                if let Some(reply) = Self::route(layout, window) { return Some(reply); }
                let id = WidgetId::new(popup_id.clone());
                layout.popups_map_mut().remove(&id);
                Some(CommandReply::ok())
            }
            Command::OpenDropdown { window, dropdown_id } => {
                if let Some(reply) = Self::route(layout, window) { return Some(reply); }
                let h = layout.add_dropdown(dropdown_id);
                layout.dropdown_mut(&h).open = true;
                Some(CommandReply::ok())
            }
            Command::CloseDropdown { window, dropdown_id } => {
                if let Some(reply) = Self::route(layout, window) { return Some(reply); }
                let h = layout.add_dropdown(dropdown_id);
                layout.dropdown_mut(&h).close();
                Some(CommandReply::ok())
            }
            Command::ToggleSidebar { window, sidebar_id } => {
                if let Some(reply) = Self::route(layout, window) { return Some(reply); }
                let h = layout.add_sidebar(sidebar_id);
                layout.sidebar_mut(&h).toggle_collapse();
                Some(CommandReply::ok())
            }

            // ── LM-root ops ─────────────────────────────────────────
            Command::SetSyncMode { node_id, mode, group_id } => {
                let target = match mode.as_str() {
                    "synced" => SyncMode::Synced,
                    "sometimes_alone" => SyncMode::Sometimes(None),
                    "sometimes_group" => match group_id {
                        Some(g) => SyncMode::Sometimes(Some(SyncGroupId(*g))),
                        None => return Some(CommandReply::err(
                            "sometimes_group requires group_id",
                        )),
                    },
                    "standalone" => SyncMode::Standalone,
                    other => return Some(CommandReply::err(format!(
                        "unknown sync mode {:?}", other
                    ))),
                };
                let leaked: &'static str = Box::leak(node_id.clone().into_boxed_str());
                layout.sync_registry_mut().set(leaked, target);
                Some(CommandReply::ok())
            }
            Command::ApplyStylePreset { name } => match name.as_str() {
                "mirage_dark"  => { layout.styles_mut().apply(&MirageDarkPreset);  Some(CommandReply::ok()) }
                "mirage_light" => { layout.styles_mut().apply(&MirageLightPreset); Some(CommandReply::ok()) }
                other => Some(CommandReply::err(format!("unknown preset {:?}", other))),
            },
        }
    }

    /// Common helper: validate window key and route LM to it.  Returns
    /// `Some(err_reply)` if the window is unknown, `None` on success.
    fn route(layout: &mut LayoutManager<P>, window: &str) -> Option<CommandReply> {
        let key = crate::layout::window::WindowKey::new(window);
        if !layout.window_keys().any(|k| k == &key) {
            return Some(CommandReply::err(format!("unknown window {:?}", window)));
        }
        layout.set_current_window(key);
        None
    }
}

fn rect_to_snap(r: Rect) -> RectSnap {
    RectSnap { x: r.x, y: r.y, w: r.width, h: r.height }
}
