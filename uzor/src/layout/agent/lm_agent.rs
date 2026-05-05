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
    AgentSnapshot, BranchSnapshot, ClickSnap, DockNodeSnap, NodeSyncSnapshot, RectSnap, RootSnapshot,
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
                    tick_count: b.tick_count,
                    tick_rate: b.tick_rate.label(),
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
                    dock_tree: build_dock_tree_snap(b),
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
                style_preset: layout.styles().active_preset().map(|s| s.to_owned()),
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
                        label: w.label.clone(),
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

    /// Push an `AgentCommand` log entry recording that the WM passed
    /// us a write command and whether it succeeded.
    pub fn log_command(
        layout: &mut LayoutManager<P>,
        cmd: &Command,
        reply: &CommandReply,
    ) {
        let window = match cmd {
            Command::InjectHover { window, .. }
            | Command::InjectClick { window, .. }
            | Command::InjectScroll { window, .. }
            | Command::ClickWidget { window, .. }
            | Command::HoverWidget { window, .. }
            | Command::OpenModal { window, .. }
            | Command::CloseModal { window, .. }
            | Command::OpenPopup { window, .. }
            | Command::ClosePopup { window, .. }
            | Command::OpenDropdown { window, .. }
            | Command::CloseDropdown { window, .. }
            | Command::ToggleSidebar { window, .. } => Some(window.clone()),
            Command::SpawnWindow { key, .. } | Command::CloseWindow { key } => {
                Some(key.clone())
            }
            Command::BlackboxClickWidget { window, .. } => Some(window.clone()),
            Command::LogPush { window, .. } => window.clone(),
            Command::SetTickRate { window, .. } => Some(window.clone()),
            Command::ResizePanelEdge { window, .. }
            | Command::DragDockSeparator { window, .. }
            | Command::SetPanelRect { window, .. } => Some(window.clone()),
            Command::SetSyncMode { .. } | Command::ApplyStylePreset { .. } => None,
        };
        let ts = layout.frame_time_ms;
        layout.agent_log.push(
            ts,
            window,
            "lm.agent_command",
            serde_json::json!({
                "command": format!("{:?}", cmd),
                "ok": reply.ok,
                "message": reply.message,
            }),
        );
    }

    /// Apply a command that LM understands.  Returns:
    /// - `Some(reply)` — command was handled (success or LM-level error).
    /// - `None`        — command needs platform-side handling
    ///                   (screenshot, real OS spawn, real input event).
    ///                   The WM should look at it and apply itself.
    pub fn try_apply(layout: &mut LayoutManager<P>, cmd: &Command) -> Option<CommandReply> {
        let result = Self::try_apply_inner(layout, cmd);
        if let Some(reply) = result.as_ref() {
            Self::log_command(layout, cmd, reply);
        }
        result
    }

    fn try_apply_inner(layout: &mut LayoutManager<P>, cmd: &Command) -> Option<CommandReply> {
        use crate::layout::window::WindowKey;
        match cmd {
            // ── pure read-from-snapshot inputs (still on WM side) ───
            Command::InjectHover { .. }
            | Command::InjectClick { .. }
            | Command::InjectScroll { .. }
            | Command::SpawnWindow { .. }
            | Command::CloseWindow { .. } => None,

            Command::LogPush { category, payload, window } => {
                let ts = layout.frame_time_ms;
                layout.agent_log.push(ts, window.clone(), category.clone(), payload.clone());
                Some(CommandReply::ok())
            }

            Command::SetTickRate { window, mode, fps } => {
                let key = WindowKey::new(window.clone());
                let new_rate = match mode.as_str() {
                    "dirty"    => crate::render::TickRate::Dirty,
                    "uncapped" => crate::render::TickRate::Uncapped,
                    "capped"   => match fps {
                        Some(f) if *f > 0 => crate::render::TickRate::Capped(*f),
                        _ => return Some(CommandReply::err(
                            "capped requires `fps` > 0",
                        )),
                    },
                    other => return Some(CommandReply::err(format!(
                        "unknown tick mode {:?}", other,
                    ))),
                };
                let Some(slot) = layout.window_mut(&key) else {
                    return Some(CommandReply::err(format!("unknown window {:?}", window)));
                };
                slot.tick_rate = new_rate;
                let ts = layout.frame_time_ms;
                layout.agent_log.push(
                    ts,
                    Some(window.clone()),
                    "lm.tick_rate",
                    serde_json::json!({ "mode": mode, "fps": fps }),
                );
                Some(CommandReply::ok())
            }

            // ── blackbox sub-widget click ───────────────────────────
            Command::BlackboxClickWidget { window, slot_id, sub_id } => {
                let key = WindowKey::new(window.clone());
                if !layout.window_keys().any(|k| k == &key) {
                    return Some(CommandReply::err(format!("unknown window {:?}", window)));
                }
                let surface = match layout.find_blackbox_agent(slot_id) {
                    Some(s) => s,
                    None => return Some(CommandReply::err(format!(
                        "no blackbox registered with slot_id {:?}", slot_id
                    ))),
                };
                let rect = match surface.lock() {
                    Ok(g) => g.resolve_click_widget(sub_id),
                    Err(_) => return Some(CommandReply::err("blackbox lock poisoned")),
                };
                let rect = match rect {
                    Some(r) => r,
                    None => return Some(CommandReply::err(format!(
                        "blackbox {:?} has no widget {:?}", slot_id, sub_id
                    ))),
                };
                let cx = rect.x + rect.width / 2.0;
                let cy = rect.y + rect.height / 2.0;
                layout.set_current_window(key);
                layout.on_pointer_move(cx, cy);
                layout.on_pointer_down(cx, cy);
                let _ = layout.on_pointer_up(cx, cy);
                let ts = layout.frame_time_ms;
                layout.agent_log.push(
                    ts,
                    Some(window.clone()),
                    format!("{}.click_widget", slot_id),
                    serde_json::json!({ "sub_id": sub_id, "x": cx, "y": cy }),
                );
                Some(CommandReply::ok())
            }

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
                Self::log_overlay(layout, window, "modal", modal_id, true);
                Some(CommandReply::ok())
            }
            Command::CloseModal { window, modal_id } => {
                if let Some(reply) = Self::route(layout, window) { return Some(reply); }
                let id = WidgetId::new(modal_id.clone());
                layout.modals_map_mut().remove(&id);
                Self::log_overlay(layout, window, "modal", modal_id, false);
                Some(CommandReply::ok())
            }
            Command::OpenPopup { window, popup_id } => {
                if let Some(reply) = Self::route(layout, window) { return Some(reply); }
                let h = layout.add_popup(popup_id);
                let _ = layout.popup_mut(&h);
                Self::log_overlay(layout, window, "popup", popup_id, true);
                Some(CommandReply::ok())
            }
            Command::ClosePopup { window, popup_id } => {
                if let Some(reply) = Self::route(layout, window) { return Some(reply); }
                let id = WidgetId::new(popup_id.clone());
                layout.popups_map_mut().remove(&id);
                Self::log_overlay(layout, window, "popup", popup_id, false);
                Some(CommandReply::ok())
            }
            Command::OpenDropdown { window, dropdown_id } => {
                if let Some(reply) = Self::route(layout, window) { return Some(reply); }
                let h = layout.add_dropdown(dropdown_id);
                layout.dropdown_mut(&h).open = true;
                Self::log_overlay(layout, window, "dropdown", dropdown_id, true);
                Some(CommandReply::ok())
            }
            Command::CloseDropdown { window, dropdown_id } => {
                if let Some(reply) = Self::route(layout, window) { return Some(reply); }
                let h = layout.add_dropdown(dropdown_id);
                layout.dropdown_mut(&h).close();
                Self::log_overlay(layout, window, "dropdown", dropdown_id, false);
                Some(CommandReply::ok())
            }
            Command::ToggleSidebar { window, sidebar_id } => {
                if let Some(reply) = Self::route(layout, window) { return Some(reply); }
                let h = layout.add_sidebar(sidebar_id);
                layout.sidebar_mut(&h).toggle_collapse();
                let open_now = !layout.sidebar(&h).is_collapsed;
                Self::log_overlay(layout, window, "sidebar", sidebar_id, open_now);
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
                let ts = layout.frame_time_ms;
                layout.agent_log.push(
                    ts,
                    None,
                    "lm.sync_mode",
                    serde_json::json!({
                        "node_id": node_id,
                        "mode": mode,
                        "group_id": group_id,
                    }),
                );
                Some(CommandReply::ok())
            }
            Command::ApplyStylePreset { name } => {
                let ok = match name.as_str() {
                    "mirage_dark"  => { layout.apply_style_preset(&MirageDarkPreset,  "mirage_dark");  true }
                    "mirage_light" => { layout.apply_style_preset(&MirageLightPreset, "mirage_light"); true }
                    _ => false,
                };
                if !ok {
                    return Some(CommandReply::err(format!("unknown preset {:?}", name)));
                }
                Some(CommandReply::ok())
            }

            // ── resize / drag ───────────────────────────────────────
            Command::ResizePanelEdge { window, panel_id, edge, delta_px } => {
                let key = WindowKey::new(window.clone());
                if !layout.window_keys().any(|k| k == &key) {
                    return Some(CommandReply::err(format!("unknown window {:?}", window)));
                }
                let parsed = match edge.as_str() {
                    "n" | "N" => crate::layout::ResizeEdge::N,
                    "s" | "S" => crate::layout::ResizeEdge::S,
                    "e" | "E" => crate::layout::ResizeEdge::E,
                    "w" | "W" => crate::layout::ResizeEdge::W,
                    other => return Some(CommandReply::err(format!(
                        "edge must be n/s/e/w, got {:?}", other,
                    ))),
                };
                layout.set_current_window(key);
                let sep_idx = match layout.resize_handle_to_separator(panel_id, parsed) {
                    Some(i) => i,
                    None => return Some(CommandReply::err(format!(
                        "panel {:?} has no resizable {:?} edge (window border or unknown panel)",
                        panel_id, edge,
                    ))),
                };
                let win_rect = layout.last_window().unwrap_or(crate::types::Rect::new(0.0,0.0,1.0,1.0));
                let ok = layout.panels_mut().drag_separator(
                    sep_idx,
                    *delta_px as f32,
                    win_rect.width  as f32,
                    win_rect.height as f32,
                );
                if !ok {
                    return Some(CommandReply::err("drag_separator returned false"));
                }
                Some(CommandReply::ok())
            }

            Command::DragDockSeparator { window, sep_idx, delta_px } => {
                let key = WindowKey::new(window.clone());
                if !layout.window_keys().any(|k| k == &key) {
                    return Some(CommandReply::err(format!("unknown window {:?}", window)));
                }
                layout.set_current_window(key);
                let win_rect = layout.last_window().unwrap_or(crate::types::Rect::new(0.0,0.0,1.0,1.0));
                let ok = layout.panels_mut().drag_separator(
                    *sep_idx,
                    *delta_px as f32,
                    win_rect.width  as f32,
                    win_rect.height as f32,
                );
                if !ok {
                    return Some(CommandReply::err(format!(
                        "no separator at index {}", sep_idx,
                    )));
                }
                Some(CommandReply::ok())
            }

            Command::SetPanelRect { window, panel_id, .. } => {
                // Free-floating panels not implemented yet — placeholder
                // returning a clear error so callers know it's pending.
                let _ = (window, panel_id);
                Some(CommandReply::err(
                    "SetPanelRect is reserved for free-floating panels — not implemented yet",
                ))
            }
        }
    }

    fn log_overlay(
        layout: &mut LayoutManager<P>,
        window: &str,
        slot: &'static str,
        id: &str,
        open: bool,
    ) {
        let ts = layout.frame_time_ms;
        layout.agent_log.push(
            ts,
            Some(window.to_owned()),
            format!("lm.overlay.{}", slot),
            serde_json::json!({ "id": id, "open": open }),
        );
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

fn build_dock_tree_snap<P: DockPanel>(
    b: &crate::layout::branch::WindowBranch<P>,
) -> DockNodeSnap {
    let dock = &b.dock;
    let win  = b.rect;
    snap_node(
        &crate::layout::docking::PanelNode::Branch(dock.tree().root().clone()),
        Rect::new(0.0, 0.0, win.width, win.height),
        b,
    )
}

fn snap_node<P: DockPanel>(
    node:   &crate::layout::docking::PanelNode<P>,
    parent: Rect,
    b:      &crate::layout::branch::WindowBranch<P>,
) -> DockNodeSnap {
    use crate::layout::docking::PanelNode;
    match node {
        PanelNode::Leaf(l) => {
            // Resolve actual rect via dock state's solved map; falls
            // back to parent if not solved this frame.
            let rect = b.dock.panel_rects().get(&l.id)
                .map(|pr| Rect::new(pr.x as f64, pr.y as f64, pr.width as f64, pr.height as f64))
                .unwrap_or(parent);
            DockNodeSnap::Leaf {
                leaf_id:  l.id.0,
                panel_id: l.panels.first().map(|p| p.type_id().to_string()),
                rect:     rect_to_snap(rect),
            }
        }
        PanelNode::Branch(br) => {
            // Compute child rects so each child gets its real rect (the
            // top-level call passes the window rect; the recursion pipes
            // each computed child rect down).
            let child_rects = crate::layout::docking::DockingTree::<P>
                ::compute_child_rects(br, crate::layout::docking::PanelRect::new(
                    parent.x as f32, parent.y as f32,
                    parent.width as f32, parent.height as f32,
                ));
            let children: Vec<DockNodeSnap> = br.children.iter()
                .zip(child_rects.iter())
                .map(|(c, cr)| {
                    let cr = Rect::new(cr.x as f64, cr.y as f64, cr.width as f64, cr.height as f64);
                    snap_node(c, cr, b)
                })
                .collect();
            DockNodeSnap::Branch {
                branch_id:   br.id.0,
                layout:      format!("{:?}", br.layout).to_lowercase(),
                proportions: br.proportions.clone(),
                children,
            }
        }
    }
}
