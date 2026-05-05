//! Layout-tree debug panel — a full blackbox with agent control.
//!
//! Lives inside the dashboard as a dock leaf.  Renders the live LM
//! tree (synced root + every attached branch) with sync-mode pills.
//! Two toggles in the top-right corner let the user filter what to
//! show:
//!
//! - `show_synced_root` — hides the green "synced root" section.
//! - `show_standalone`  — hides red Standalone rows.
//!
//! Both are controllable from the agent via:
//!
//! - `POST /blackbox/tree-debug/click_widget {sub_id:"toggle:synced_root"}`
//! - `POST /blackbox/tree-debug/action {name:"set_filter", args:{...}}`
//!
//! The agent reads internal state via
//! `GET /blackbox/tree-debug/state` and the published mini-widget list
//! via `GET /blackbox/tree-debug/widgets`.

use std::sync::{Arc, Mutex};

use serde_json::json;

use uzor::core::types::Rect;
use uzor::engine::render::RenderContext;
use uzor::framework::multi_window::WindowKey;
use uzor::layout::agent::{
    AgentAction, AgentActionReply, AgentWidget, BlackboxAgentSurface,
};
use uzor::layout::docking::DockPanel;
use uzor::layout::sync::{node_ids, SyncMode};
use uzor::layout::LayoutManager;

// ── State ────────────────────────────────────────────────────────────

/// Long-lived state for the tree-debug blackbox.  Owned by the host
/// app and shared with LM through `Arc<Mutex<TreeDebugState>>` so the
/// agent surface can be locked from any thread.
#[derive(Debug, Clone)]
pub struct TreeDebugState {
    pub show_synced_root: bool,
    pub show_standalone:  bool,
    /// Slot id this instance was registered with.  Used by
    /// `BlackboxAgentSurface::agent_slot_id`.
    pub slot_id: String,
    /// Cached toggle rects (filled in by the renderer each frame).
    pub toggle_rects: Vec<(String, Rect)>,
}

impl Default for TreeDebugState {
    fn default() -> Self {
        Self {
            show_synced_root: true,
            show_standalone:  true,
            slot_id: "tree-debug".to_owned(),
            toggle_rects: Vec::new(),
        }
    }
}

// ── BlackboxAgentSurface impl ───────────────────────────────────────

impl BlackboxAgentSurface for TreeDebugState {
    fn agent_slot_id(&self) -> &str {
        &self.slot_id
    }

    fn agent_kind(&self) -> &str {
        "tree-debug"
    }

    fn list_agent_widgets(&self) -> Vec<AgentWidget> {
        let mut out = Vec::with_capacity(self.toggle_rects.len());
        for (sub_id, rect) in &self.toggle_rects {
            let (label, checked) = match sub_id.as_str() {
                "toggle:synced_root" => ("Show synced root", self.show_synced_root),
                "toggle:standalone"  => ("Show standalone",  self.show_standalone),
                _ => ("?", false),
            };
            out.push(AgentWidget {
                sub_id: sub_id.clone(),
                kind: "toggle".to_owned(),
                rect: *rect,
                label: Some(label.to_owned()),
                meta: json!({ "checked": checked }),
            });
        }
        out
    }

    fn agent_state(&self) -> serde_json::Value {
        json!({
            "show_synced_root": self.show_synced_root,
            "show_standalone":  self.show_standalone,
        })
    }

    fn apply_agent_action(&mut self, action: AgentAction) -> AgentActionReply {
        match action.name.as_str() {
            "toggle_synced_root" => {
                self.show_synced_root = !self.show_synced_root;
                AgentActionReply::ok_with_log(json!({
                    "show_synced_root": self.show_synced_root
                }))
            }
            "toggle_standalone" => {
                self.show_standalone = !self.show_standalone;
                AgentActionReply::ok_with_log(json!({
                    "show_standalone": self.show_standalone
                }))
            }
            "set_filter" => {
                if let Some(v) = action.args.get("show_synced_root").and_then(|v| v.as_bool()) {
                    self.show_synced_root = v;
                }
                if let Some(v) = action.args.get("show_standalone").and_then(|v| v.as_bool()) {
                    self.show_standalone = v;
                }
                AgentActionReply::ok_with_log(json!({
                    "show_synced_root": self.show_synced_root,
                    "show_standalone":  self.show_standalone,
                }))
            }
            other => AgentActionReply::err(format!("unknown action {:?}", other)),
        }
    }
}

// ── Per-frame render entry-point ────────────────────────────────────

/// Render the panel inside `rect`.  Updates `state.toggle_rects` so
/// `BlackboxAgentSurface::list_agent_widgets` returns current rects.
///
/// Click handling is done by the host (l4-dashboard) — it polls
/// `layout.was_clicked(...)` for synthetic widget ids registered by
/// this renderer (`tree-debug:toggle:synced_root` / `:standalone`)
/// and forwards into [`TreeDebugState::apply_agent_action`].  This
/// means *both* a human click and an agent click come through the
/// same code path.
pub fn render_layout_tree<P: DockPanel>(
    rect:   Rect,
    state:  &mut TreeDebugState,
    layout: &LayoutManager<P>,
    render: &mut dyn RenderContext,
) {
    // Clip everything to our cell rect — neighbour cells must not bleed
    // through after a dock-separator drag shrinks us.
    render.save();
    render.clip_rect(rect.x, rect.y, rect.width, rect.height);

    // Background
    let bg = layout.styles().color_or_owned("surface", "#16171D");
    render.set_fill_color(bg.as_str());
    render.fill_rect(rect.x, rect.y, rect.width, rect.height);

    // Title
    render.set_fill_color("#E6E6EA");
    render.set_font("bold 14px sans-serif");
    render.fill_text("LayoutManager — live tree", rect.x + 12.0, rect.y + 18.0);

    // ── Toggles in the top-right corner ─────────────────────────────
    let toggle_w = 28.0_f64;
    let toggle_h = 16.0_f64;
    let toggle_gap = 8.0_f64;

    let synced_x = rect.x + rect.width - 12.0 - toggle_w;
    let synced_y = rect.y + 8.0;
    draw_toggle(render, synced_x, synced_y, toggle_w, toggle_h, state.show_synced_root, "synced");

    let standalone_x = rect.x + rect.width - 12.0 - toggle_w * 2.0 - toggle_gap;
    let standalone_y = synced_y;
    draw_toggle(render, standalone_x, standalone_y, toggle_w, toggle_h, state.show_standalone, "alone");

    // Publish toggle rects so `list_agent_widgets` can hand them to the agent.
    state.toggle_rects.clear();
    state.toggle_rects.push((
        "toggle:synced_root".to_owned(),
        Rect::new(synced_x, synced_y, toggle_w, toggle_h),
    ));
    state.toggle_rects.push((
        "toggle:standalone".to_owned(),
        Rect::new(standalone_x, standalone_y, toggle_w, toggle_h),
    ));

    let mut cy = rect.y + 38.0;
    let line_h = 18.0_f64;
    let indent = 16.0_f64;

    if state.show_synced_root {
        draw_label(render, rect.x + 12.0, &mut cy, line_h, "synced root", "#9AA0AC");
        for id in [
            node_ids::STYLES,
            node_ids::Z_LAYERS,
            node_ids::FRAME_TIME,
            node_ids::PANEL_TYPES,
        ] {
            let mode = layout.sync_registry().get(id);
            draw_node(render, rect.x + 12.0 + indent, &mut cy, line_h, id, mode, None);
        }
        cy += 6.0;
    }

    let keys: Vec<WindowKey> = layout.window_keys().cloned().collect();
    let current = layout.current_window().cloned();

    draw_label(
        render,
        rect.x + 12.0,
        &mut cy,
        line_h,
        &format!("windows ({})", keys.len()),
        "#9AA0AC",
    );

    for key in &keys {
        let prefix = if Some(key) == current.as_ref() { "▶ " } else { "  " };
        draw_label(
            render,
            rect.x + 12.0 + indent,
            &mut cy,
            line_h,
            &format!("{prefix}window: {}", key.as_str()),
            "#C9CDD8",
        );

        for (id, hint) in window_branch_rows(layout, key) {
            let mode = layout.sync_registry().get(id);
            if !state.show_standalone && matches!(mode, SyncMode::Standalone) {
                continue;
            }
            draw_node(
                render,
                rect.x + 12.0 + indent * 2.0,
                &mut cy,
                line_h,
                id,
                mode,
                hint.as_deref(),
            );
            if cy > rect.y + rect.height - 28.0 {
                return;
            }
        }
        cy += 4.0;
    }

    // Legend
    let legend_y = rect.y + rect.height - 22.0;
    let mut lx = rect.x + 12.0;
    for (label, mode) in [
        ("synced",          SyncMode::Synced),
        ("sometimes·alone", SyncMode::Sometimes(None)),
        ("standalone",      SyncMode::Standalone),
    ] {
        let css = rgba_css(mode.color());
        render.set_fill_color(&css);
        render.fill_rounded_rect(lx, legend_y, 12.0, 12.0, 3.0);
        render.set_fill_color("#C9CDD8");
        render.set_font("11px sans-serif");
        render.fill_text(label, lx + 18.0, legend_y + 10.0);
        lx += render.measure_text(label) + 44.0;
    }

    render.restore();
}

// ── Construction helpers ────────────────────────────────────────────

/// Build a fresh shared state for the panel and register it as a
/// blackbox agent surface so HTTP `/blackbox/tree-debug/...` routes
/// resolve.
pub fn register<P: DockPanel>(
    layout: &mut LayoutManager<P>,
    slot_id: impl Into<String>,
) -> Arc<Mutex<TreeDebugState>> {
    let mut state = TreeDebugState::default();
    state.slot_id = slot_id.into();
    let id = state.slot_id.clone();
    let arc = Arc::new(Mutex::new(state));
    layout.register_blackbox_agent(id, arc.clone());
    arc
}

// ── small drawing helpers ───────────────────────────────────────────

fn draw_toggle(
    render: &mut dyn RenderContext,
    x: f64, y: f64, w: f64, h: f64,
    on: bool,
    short_label: &str,
) {
    let track_bg = if on { "#5CB87A" } else { "#3A3D45" };
    render.set_fill_color(track_bg);
    render.fill_rounded_rect(x, y, w, h, h / 2.0);

    let knob_size = h - 4.0;
    let knob_x = if on { x + w - knob_size - 2.0 } else { x + 2.0 };
    let knob_y = y + 2.0;
    render.set_fill_color("#E6E6EA");
    render.fill_rounded_rect(knob_x, knob_y, knob_size, knob_size, knob_size / 2.0);

    // Tiny label below
    render.set_fill_color("#9AA0AC");
    render.set_font("9px sans-serif");
    render.fill_text(short_label, x, y + h + 9.0);
}

fn rgba_css(c: [f32; 4]) -> String {
    format!(
        "rgba({},{},{},{:.2})",
        (c[0] * 255.0) as u8,
        (c[1] * 255.0) as u8,
        (c[2] * 255.0) as u8,
        c[3]
    )
}

fn draw_label(
    render: &mut dyn RenderContext,
    x: f64,
    cy: &mut f64,
    line_h: f64,
    text: &str,
    color: &str,
) {
    render.set_fill_color(color);
    render.set_font("12px sans-serif");
    render.fill_text(text, x, *cy);
    *cy += line_h;
}

fn draw_node(
    render: &mut dyn RenderContext,
    x: f64,
    cy: &mut f64,
    line_h: f64,
    node_id: &str,
    mode: SyncMode,
    hint: Option<&str>,
) {
    let pill_w = 92.0;
    let pill_h = 14.0;
    let pill_y = *cy - 11.0;

    let css = rgba_css(mode.color());
    render.set_fill_color(&css);
    render.fill_rounded_rect(x, pill_y, pill_w, pill_h, 3.0);
    render.set_fill_color("#0E0F12");
    render.set_font("bold 10px sans-serif");
    render.fill_text(mode.label(), x + 4.0, pill_y + 11.0);

    render.set_fill_color("#D6D9E2");
    render.set_font("12px sans-serif");
    let label = match hint {
        Some(h) if !h.is_empty() => format!("{node_id}    ({h})"),
        _ => node_id.to_string(),
    };
    render.fill_text(&label, x + pill_w + 8.0, *cy);
    *cy += line_h;
}

fn window_branch_rows<P: DockPanel>(
    layout: &LayoutManager<P>,
    key: &WindowKey,
) -> Vec<(&'static str, Option<String>)> {
    let Some(branch) = layout.window(key) else { return Vec::new() };

    let dock_leaves = branch.dock.tree().leaves().len();
    let edge_count = branch.edges.iter().count();
    let overlay_count = branch.overlays.entries().len();
    let modal_count = branch.modals.len();
    let popup_count = branch.popups.len();
    let dropdown_count = branch.dropdowns.len();
    let toolbar_count = branch.toolbars.len();
    let sidebar_count = branch.sidebars.len();
    let context_menu_count = branch.context_menus.len();
    let hovered = branch.last_hovered.as_ref().map(|w| w.as_str().to_owned());
    let pointer = branch.last_pointer_pos.map(|(x, y)| format!("({x:.0},{y:.0})"));

    vec![
        (node_ids::CHROME_CFG,    Some(format!("visible={}", branch.chrome.visible))),
        (node_ids::EDGES,         Some(format!("{edge_count} slots"))),
        (node_ids::DOCK_TREE,     Some(format!("{dock_leaves} leaves"))),
        (node_ids::LAYOUT_TREE,   None),
        (node_ids::OVERLAYS,      Some(format!("{overlay_count} entries"))),
        (node_ids::POINTER_STATE, Some(format!(
            "hover={} at={}",
            hovered.unwrap_or_else(|| "—".into()),
            pointer.unwrap_or_else(|| "—".into()),
        ))),
        (node_ids::MODALS,        Some(format!("{modal_count}"))),
        (node_ids::POPUPS,        Some(format!("{popup_count}"))),
        (node_ids::DROPDOWNS,     Some(format!("{dropdown_count}"))),
        (node_ids::TOOLBARS,      Some(format!("{toolbar_count}"))),
        (node_ids::SIDEBARS,      Some(format!("{sidebar_count}"))),
        (node_ids::CONTEXT_MENUS, Some(format!("{context_menu_count}"))),
    ]
}
