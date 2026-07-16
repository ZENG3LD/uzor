//! `BlackboxAgentSurface` impl for [`GraphEngine`] — state (node count,
//! visible count, selected, camera, alpha/hot) and actions (`select_node`,
//! `pin_node`/`unpin_node`, `set_camera`, `fit_view`), so the engine is
//! driveable/screenshot-verifiable headlessly via `uzor-agent-api`
//! without the app needing to write any of this itself.

use serde_json::{json, Value};

use uzor::layout::agent::{AgentAction, AgentActionReply, AgentWidget, BlackboxAgentSurface};
use uzor::types::Rect;

use crate::engine::GraphEngine;
use crate::graph::NodeIndex;
use crate::layout::Layout;

impl<N, E, L> BlackboxAgentSurface for GraphEngine<N, E, L>
where
    N: Send + 'static,
    E: Send + 'static,
    L: Layout + Send + 'static,
{
    fn agent_slot_id(&self) -> &str {
        &self.agent_slot_id
    }

    fn agent_kind(&self) -> &str {
        "graph"
    }

    fn list_agent_widgets(&self) -> Vec<AgentWidget> {
        self.visible_nodes()
            .iter()
            .filter_map(|&id| {
                let facts = self.node_facts(id)?;
                let (sx, sy) = self.camera.world_to_screen((facts.position.0 as f64, facts.position.1 as f64), self.canvas_rect());
                let r = self.camera.node_screen_radius(self.graph.get_node(id)?.radius);
                Some(AgentWidget {
                    sub_id: format!("node:{}", id.index()),
                    kind: "node".to_owned(),
                    rect: Rect::new(sx - r, sy - r, r * 2.0, r * 2.0),
                    label: Some(facts.label.to_owned()),
                    meta: json!({
                        "category": facts.category,
                        "degree": facts.degree,
                        "pinned": facts.pinned,
                    }),
                })
            })
            .collect()
    }

    fn agent_state(&self) -> Value {
        let selected = self.selected_facts().map(|f| {
            json!({
                "index": f.index.index(),
                "label": f.label,
                "category": f.category,
                "degree": f.degree,
                "pinned": f.pinned,
            })
        });
        json!({
            "node_count": self.graph.node_count(),
            "edge_count": self.graph.edge_count(),
            "visible_node_count": self.visible_nodes().len(),
            "selected": selected,
            "hovered": self.hovered.map(|id| id.index()),
            "camera": {
                "pan_x": self.camera.pan_x,
                "pan_y": self.camera.pan_y,
                "zoom": self.camera.zoom,
            },
            "alpha": self.last_tick().alpha,
            "hot": self.is_hot(),
        })
    }

    fn apply_agent_action(&mut self, action: AgentAction) -> AgentActionReply {
        match action.name.as_str() {
            "select_node" => {
                let Some(node) = resolve_node(self, &action) else {
                    return AgentActionReply::err("select_node requires args.index (u32) or args.label (string)");
                };
                self.select(node);
                AgentActionReply::ok_with_log(json!({ "selected": node.index() }))
            }
            "clear_selection" => {
                self.clear_selection();
                AgentActionReply::ok_with_log(json!({ "selected": Value::Null }))
            }
            "pin_node" => {
                let Some(node) = resolve_node(self, &action) else {
                    return AgentActionReply::err("pin_node requires args.index (u32) or args.label (string)");
                };
                self.pin_node(node);
                AgentActionReply::ok_with_log(json!({ "pinned": node.index() }))
            }
            "unpin_node" => {
                let Some(node) = resolve_node(self, &action) else {
                    return AgentActionReply::err("unpin_node requires args.index (u32) or args.label (string)");
                };
                self.unpin_node(node);
                AgentActionReply::ok_with_log(json!({ "unpinned": node.index() }))
            }
            "set_camera" => {
                if let Some(x) = action.args.get("pan_x").and_then(Value::as_f64) {
                    self.camera.pan_x = x;
                }
                if let Some(y) = action.args.get("pan_y").and_then(Value::as_f64) {
                    self.camera.pan_y = y;
                }
                if let Some(z) = action.args.get("zoom").and_then(Value::as_f64) {
                    self.camera.zoom = z.clamp(crate::camera::ZOOM_MIN, crate::camera::ZOOM_MAX);
                }
                self.mark_dirty();
                AgentActionReply::ok_with_log(json!({
                    "pan_x": self.camera.pan_x,
                    "pan_y": self.camera.pan_y,
                    "zoom": self.camera.zoom,
                }))
            }
            "fit_view" => {
                self.fit_view();
                AgentActionReply::ok_with_log(json!({
                    "pan_x": self.camera.pan_x,
                    "pan_y": self.camera.pan_y,
                    "zoom": self.camera.zoom,
                }))
            }
            other => AgentActionReply::err(format!("unknown action {other:?}")),
        }
    }
}

fn resolve_node<N, E, L: Layout>(engine: &GraphEngine<N, E, L>, action: &AgentAction) -> Option<NodeIndex> {
    if let Some(idx) = action.args.get("index").and_then(Value::as_u64) {
        let node = NodeIndex(idx as u32);
        return (node.index() < engine.graph.node_count()).then_some(node);
    }
    if let Some(label) = action.args.get("label").and_then(Value::as_str) {
        return engine.graph.find_by_label(label);
    }
    None
}
