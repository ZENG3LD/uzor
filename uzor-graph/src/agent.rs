//! `BlackboxAgentSurface` impl for [`GraphEngine`] — state (node count,
//! visible count, selected, camera, alpha/hot, layout mode, clusters)
//! and actions (`select_node`, `pin_node`/`unpin_node`, `set_camera`,
//! `fit_view`, `collapse`, `expand`, `set_layout`), so the engine is
//! driveable/screenshot-verifiable headlessly via `uzor-agent-api`
//! without the app needing to write any of this itself.

use serde_json::{json, Value};

use uzor::layout::agent::{AgentAction, AgentActionReply, AgentWidget, BlackboxAgentSurface};
use uzor::types::Rect;

use crate::cluster::GroupId;
use crate::engine::GraphEngine;
use crate::graph::NodeIndex;
use crate::layout::{ForceParams, GraphLayoutMode, Layout, LayoutKind};

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
        let layout_mode = layout_mode_name(&self.layout);
        let clusters: Vec<Value> = self
            .clusters
            .iter()
            .map(|(id, c)| json!({ "id": id.0, "member_count": c.member_count(), "collapsed": c.is_collapsed() }))
            .collect();
        let hover = self.hovered.and_then(|id| self.node_facts(id)).map(|f| {
            json!({
                "index": f.index.index(),
                "label": f.label,
            })
        });
        json!({
            "node_count": self.graph.node_count(),
            "edge_count": self.graph.edge_count(),
            "visible_node_count": self.visible_nodes().len(),
            "selected": selected,
            "hovered": self.hovered.map(|id| id.index()),
            "hover": hover,
            "camera": {
                "pan_x": self.camera.pan_x,
                "pan_y": self.camera.pan_y,
                "zoom": self.camera.zoom,
            },
            "alpha": self.last_tick().alpha,
            "hot": self.is_hot(),
            "layout": layout_mode,
            "clusters": clusters,
            "collapsed_clusters": self.clusters.collapsed_ids(),
            "forces": self.force_params().map(|p| force_params_json(&p)),
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
            // Wave 2.2 — drives the exact same `set_hovered` path
            // `on_pointer_moved`'s picking uses, so the reducer-style
            // neighbor-highlight + hover card are screenshot-verifiable
            // headlessly. `{}`/`{"index": null}` clears the hover; an
            // out-of-range `index` or an unknown `label` is an error, not
            // a silent clear (distinguishes "caller meant to clear" from
            // "caller made a typo").
            "hover_node" => {
                let index_arg = action.args.get("index");
                let explicit_clear = matches!(index_arg, Some(Value::Null))
                    || (index_arg.is_none() && action.args.get("label").is_none());
                if explicit_clear {
                    self.set_hovered(None);
                    return AgentActionReply::ok_with_log(json!({ "hover": Value::Null }));
                }
                let Some(node) = resolve_node(self, &action) else {
                    return AgentActionReply::err("hover_node requires args.index (u32), args.label (string), or {} / null to clear");
                };
                self.set_hovered(Some(node));
                AgentActionReply::ok_with_log(json!({ "hover": { "index": node.index() } }))
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
            "collapse" => {
                let Some(id) = resolve_cluster(&action) else {
                    return AgentActionReply::err("collapse requires args.cluster (u32 GroupId)");
                };
                if self.collapse_cluster(id) {
                    AgentActionReply::ok_with_log(json!({ "collapsed": id.0 }))
                } else {
                    AgentActionReply::err(format!("cluster {} not found or already collapsed", id.0))
                }
            }
            "expand" => {
                let Some(id) = resolve_cluster(&action) else {
                    return AgentActionReply::err("expand requires args.cluster (u32 GroupId)");
                };
                if self.expand_cluster(id) {
                    AgentActionReply::ok_with_log(json!({ "expanded": id.0 }))
                } else {
                    AgentActionReply::err(format!("cluster {} not found or not collapsed", id.0))
                }
            }
            // Wave 2.1 owner order — "хочу иметь возможность изменять
            // силу притяжения". Partial update: only keys present in
            // `args` change, everything else carries over from the
            // engine's current `force_params()` snapshot. Requires the
            // active layout to have a force model (bare
            // `ForceDirectedLayout`, or `GraphLayoutMode` — see
            // `GraphEngine::force_params`'s doc comment).
            "set_forces" => {
                let Some(mut params) = self.force_params() else {
                    return AgentActionReply::err(
                        "set_forces requires GraphEngine<_, _, ForceDirectedLayout> or GraphEngine<_, _, GraphLayoutMode>",
                    );
                };
                if let Some(v) = action.args.get("charge").and_then(Value::as_f64) {
                    params.charge_strength = v as f32;
                }
                if let Some(v) = action.args.get("link_strength").and_then(Value::as_f64) {
                    params.link_strength = v as f32;
                }
                if let Some(v) = action.args.get("link_distance").and_then(Value::as_f64) {
                    params.link_distance = v as f32;
                }
                if let Some(v) = action.args.get("center_gravity").and_then(Value::as_f64) {
                    params.center_strength = v as f32;
                }
                if let Some(v) = action.args.get("center_x").and_then(Value::as_f64) {
                    params.center.0 = v as f32;
                }
                if let Some(v) = action.args.get("center_y").and_then(Value::as_f64) {
                    params.center.1 = v as f32;
                }
                if let Some(v) = action.args.get("velocity_decay").and_then(Value::as_f64) {
                    params.velocity_decay = v as f32;
                }
                if let Some(v) = action.args.get("alpha_decay").and_then(Value::as_f64) {
                    params.alpha_decay = v as f32;
                }
                if let Some(v) = action.args.get("alpha_min").and_then(Value::as_f64) {
                    params.alpha_min = v as f32;
                }
                if let Some(v) = action.args.get("theta").and_then(Value::as_f64) {
                    params.theta = v as f32;
                }
                if let Some(v) = action.args.get("collision").and_then(Value::as_bool) {
                    params.collision = v;
                }
                if let Some(v) = action.args.get("collision_strength").and_then(Value::as_f64) {
                    params.collision_strength = v as f32;
                }
                if let Some(v) = action.args.get("brute_force_threshold").and_then(Value::as_u64) {
                    params.brute_force_threshold = v as usize;
                }
                self.set_force_params(params);
                AgentActionReply::ok_with_log(json!({ "forces": force_params_json(&params) }))
            }
            "set_layout" => {
                let Some(mode) = action.args.get("mode").and_then(Value::as_str) else {
                    return AgentActionReply::err("set_layout requires args.mode (\"force\"|\"hierarchical\"|\"radial\")");
                };
                let kind = match mode {
                    "force" => LayoutKind::Force,
                    "hierarchical" => LayoutKind::Hierarchical,
                    "radial" => LayoutKind::Radial,
                    other => return AgentActionReply::err(format!("unknown layout mode {other:?}")),
                };
                // `GraphLayoutMode` is one `Layout` impl among several this
                // engine can be generic over (`L`) — a runtime `set_layout`
                // action only makes sense when `L` actually IS the
                // dispatcher, so this downcasts rather than assuming.
                let layout_any: &mut dyn std::any::Any = &mut self.layout;
                let Some(dispatch) = layout_any.downcast_mut::<GraphLayoutMode>() else {
                    return AgentActionReply::err("set_layout requires GraphEngine<_, _, GraphLayoutMode>");
                };
                dispatch.set_kind(kind);
                self.mark_dirty();
                AgentActionReply::ok_with_log(json!({ "layout": mode }))
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

fn resolve_cluster(action: &AgentAction) -> Option<GroupId> {
    action.args.get("cluster").and_then(Value::as_u64).map(|v| GroupId(v as u32))
}

/// Full [`ForceParams`] snapshot as JSON — shared by `agent_state`'s
/// `forces` field and the `set_forces` action's reply, so both report
/// the exact same key names a caller would use to partially update them.
fn force_params_json(p: &ForceParams) -> Value {
    json!({
        "charge": p.charge_strength,
        "link_strength": p.link_strength,
        "link_distance": p.link_distance,
        "center_gravity": p.center_strength,
        "center_x": p.center.0,
        "center_y": p.center.1,
        "velocity_decay": p.velocity_decay,
        "alpha_decay": p.alpha_decay,
        "alpha_min": p.alpha_min,
        "theta": p.theta,
        "collision": p.collision,
        "collision_strength": p.collision_strength,
        "brute_force_threshold": p.brute_force_threshold,
    })
}

/// `Some("force"|"hierarchical"|"radial")` when `L` is the runtime
/// [`GraphLayoutMode`] dispatcher, `None` for any other concrete
/// `Layout` (e.g. a bare `ForceDirectedLayout`) — same downcast
/// approach as the `set_layout` action, read-only.
fn layout_mode_name<L: Layout + 'static>(layout: &L) -> Option<&'static str> {
    let layout_any: &dyn std::any::Any = layout;
    layout_any.downcast_ref::<GraphLayoutMode>().map(|m| match m.kind() {
        LayoutKind::Force => "force",
        LayoutKind::Hierarchical => "hierarchical",
        LayoutKind::Radial => "radial",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Graph;
    use crate::layout::ForceDirectedLayout;

    fn action(name: &str, args: Value) -> AgentAction {
        AgentAction { name: name.to_owned(), args }
    }

    fn ring_graph(n: u32) -> (Graph<(), ()>, Vec<NodeIndex>) {
        let mut graph = Graph::new();
        let ids: Vec<NodeIndex> = (0..n).map(|i| graph.push_node((), format!("n{i}"), "x", 4.0)).collect();
        for i in 0..ids.len() {
            graph.push_edge(ids[i], ids[(i + 1) % ids.len()], 1.0, ());
        }
        (graph, ids)
    }

    #[test]
    fn layout_mode_name_is_none_for_a_bare_force_directed_layout() {
        let (graph, _) = ring_graph(3);
        let engine: GraphEngine<(), (), ForceDirectedLayout> = GraphEngine::new(graph, ForceDirectedLayout::default());
        assert_eq!(engine.agent_state().get("layout").and_then(Value::as_str), None);
    }

    #[test]
    fn set_layout_action_switches_mode_and_shows_up_in_agent_state() {
        let (graph, _) = ring_graph(3);
        let mut engine: GraphEngine<(), (), GraphLayoutMode> = GraphEngine::new(graph, GraphLayoutMode::default());
        assert_eq!(engine.agent_state()["layout"], json!("force"));

        let reply = engine.apply_agent_action(action("set_layout", json!({ "mode": "radial" })));
        assert!(reply.ok);
        assert_eq!(engine.agent_state()["layout"], json!("radial"));

        let bad = engine.apply_agent_action(action("set_layout", json!({ "mode": "not_a_mode" })));
        assert!(!bad.ok);
    }

    #[test]
    fn collapse_and_expand_actions_round_trip_through_agent_state() {
        let (graph, ids) = ring_graph(4);
        let mut engine: GraphEngine<(), (), ForceDirectedLayout> = GraphEngine::new(graph, ForceDirectedLayout::default());
        let id = engine.define_cluster(ids).expect("non-empty cluster");

        let collapse_reply = engine.apply_agent_action(action("collapse", json!({ "cluster": id.0 })));
        assert!(collapse_reply.ok);
        assert_eq!(engine.agent_state()["collapsed_clusters"], json!([id.0]));

        // Collapsing an already-collapsed cluster is an error reply, not a panic.
        let again = engine.apply_agent_action(action("collapse", json!({ "cluster": id.0 })));
        assert!(!again.ok);

        let expand_reply = engine.apply_agent_action(action("expand", json!({ "cluster": id.0 })));
        assert!(expand_reply.ok);
        assert_eq!(engine.agent_state()["collapsed_clusters"], json!([] as [u32; 0]));
    }

    fn settle(engine: &mut GraphEngine<(), (), ForceDirectedLayout>) {
        for _ in 0..3000 {
            if engine.tick(1.0 / 60.0).settled {
                break;
            }
        }
    }

    fn two_node_chain() -> (Graph<(), ()>, NodeIndex, NodeIndex) {
        let mut graph = Graph::new();
        let a = graph.push_node((), "a", "x", 4.0);
        let b = graph.push_node((), "b", "x", 4.0);
        graph.push_edge(a, b, 1.0, ());
        (graph, a, b)
    }

    #[test]
    fn set_forces_action_partially_updates_params_and_grows_the_settled_edge_length() {
        let (graph, a, b) = two_node_chain();
        let mut engine: GraphEngine<(), (), ForceDirectedLayout> = GraphEngine::new(graph, ForceDirectedLayout::default());
        engine.seed_positions(&[(-20.0, 0.0), (20.0, 0.0)]);
        settle(&mut engine);

        let edge_len = |e: &GraphEngine<(), (), ForceDirectedLayout>| {
            let dx = e.particles[b.index()].x - e.particles[a.index()].x;
            let dy = e.particles[b.index()].y - e.particles[a.index()].y;
            (dx * dx + dy * dy).sqrt()
        };
        let baseline_dist = edge_len(&engine);

        let before = engine.force_params().expect("bare ForceDirectedLayout has force params");
        let bigger_link_distance = before.link_distance * 4.0;

        let reply = engine.apply_agent_action(action("set_forces", json!({ "link_distance": bigger_link_distance })));
        assert!(reply.ok);
        let after = engine.force_params().expect("still force-capable after set_forces");
        assert_eq!(after.link_distance, bigger_link_distance);
        // Untouched keys must survive the partial update.
        assert_eq!(after.charge_strength, before.charge_strength);
        assert_eq!(after.velocity_decay, before.velocity_decay);

        settle(&mut engine);
        let grown_dist = edge_len(&engine);
        assert!(
            grown_dist > baseline_dist * 1.2,
            "growing link_distance {} -> {} must grow the settled edge length: {baseline_dist} -> {grown_dist}",
            before.link_distance,
            bigger_link_distance
        );

        let forces = engine.agent_state()["forces"].clone();
        assert_eq!(forces["link_distance"].as_f64().unwrap() as f32, bigger_link_distance);
    }

    #[test]
    fn hover_node_action_drives_the_same_reducer_pipeline_pointer_hover_uses() {
        let (graph, ids) = ring_graph(4);
        let mut engine: GraphEngine<(), (), ForceDirectedLayout> = GraphEngine::new(graph, ForceDirectedLayout::default());

        let reply = engine.apply_agent_action(action("hover_node", json!({ "index": ids[0].index() })));
        assert!(reply.ok);
        assert_eq!(engine.agent_state()["hover"]["index"], json!(ids[0].index()));
        assert_eq!(engine.agent_state()["hover"]["label"], json!("n0"));
        assert!(engine.focus.is_active());
        assert!(engine.focus.is_selected(u64::from(ids[0])));
        assert!(engine.focus.is_selected(u64::from(ids[1])), "ring neighbor of node 0 must be highlighted too");

        // `{}` (no index, no label) clears the hover — same convention
        // `select_node`/`clear_selection` already use for "nothing".
        let cleared = engine.apply_agent_action(action("hover_node", json!({})));
        assert!(cleared.ok);
        assert_eq!(engine.agent_state()["hover"], Value::Null);
        assert!(!engine.focus.is_active());

        // Explicit `null` is the same as `{}`.
        engine.apply_agent_action(action("hover_node", json!({ "index": ids[2].index() })));
        assert!(engine.focus.is_active());
        let cleared_explicit = engine.apply_agent_action(action("hover_node", json!({ "index": Value::Null })));
        assert!(cleared_explicit.ok);
        assert!(!engine.focus.is_active());

        // An out-of-range index is an error reply, not a silent clear.
        let bad = engine.apply_agent_action(action("hover_node", json!({ "index": 999 })));
        assert!(!bad.ok);
    }
}
