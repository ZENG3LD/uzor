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
use crate::engine::{FilterSpec, GraphEngine, SelectMode, DEFAULT_FIT_PADDING_PX, DEFAULT_TRANSITION_MS};
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
            "selection": selection_json(self),
            "hovered": self.hovered.map(|id| id.index()),
            "hover": hover,
            "camera": camera_json(self),
            "alpha": self.last_tick().alpha,
            "hot": self.is_hot(),
            "layout": layout_mode,
            "clusters": clusters,
            "collapsed_clusters": self.clusters.collapsed_ids(),
            "forces": self.force_params().map(|p| force_params_json(&p)),
            "labels": {
                "density": self.label_density(),
                "drawn_last_frame": self.labels_drawn_last_frame(),
            },
            // Wave 2.6 — see `GraphEngine::{local_root, filter}`.
            "local_root": self.local_root().map(|(node, depth)| json!({ "index": node.index(), "depth": depth })),
            "filter": self.filter().map(filter_json),
            // Wave 2.5 — whether an animated `zoom_to_fit`/`zoom_to_node`
            // move is still in flight; `camera` above already reports the
            // live pan/zoom every frame of that animation.
            "camera_transitioning": self.camera_transitioning(),
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
            // Wave 2.4 — combine an explicit index list into the current
            // multi-selection per `args.mode` (drives the SAME
            // `apply_selection` pipeline `box_select`/the pointer path
            // use). `mode` defaults to `"replace"` when omitted.
            "select_nodes" => {
                let Some(indices) = action.args.get("indices").and_then(Value::as_array) else {
                    return AgentActionReply::err("select_nodes requires args.indices (array of u32 node indices)");
                };
                let mode = match parse_select_mode(&action) {
                    Ok(m) => m,
                    Err(e) => return AgentActionReply::err(e),
                };
                let mut nodes = Vec::with_capacity(indices.len());
                for v in indices {
                    let Some(idx) = v.as_u64() else {
                        return AgentActionReply::err("select_nodes args.indices entries must all be u32");
                    };
                    let node = NodeIndex(idx as u32);
                    if node.index() >= self.graph.node_count() {
                        return AgentActionReply::err(format!("select_nodes index {idx} out of range"));
                    }
                    nodes.push(node);
                }
                self.apply_selection(nodes, mode);
                AgentActionReply::ok_with_log(json!({ "selection": selection_json(self) }))
            }
            // Wave 2.4 — screen-space rectangle box-select, driving the
            // EXACT same pipeline a real Shift/Ctrl+Shift/Alt+Shift+drag
            // does (see `GraphEngine::box_select`), so it's
            // screenshot-verifiable headlessly.
            "box_select" => {
                let coord = |k: &str| action.args.get(k).and_then(Value::as_f64);
                let (Some(x0), Some(y0), Some(x1), Some(y1)) = (coord("x0"), coord("y0"), coord("x1"), coord("y1")) else {
                    return AgentActionReply::err("box_select requires args.x0/y0/x1/y1 (f64 screen coords)");
                };
                let mode = match parse_select_mode(&action) {
                    Ok(m) => m,
                    Err(e) => return AgentActionReply::err(e),
                };
                self.box_select((x0, y0), (x1, y1), mode);
                AgentActionReply::ok_with_log(json!({ "selection": selection_json(self) }))
            }
            // Wave 2.4 — the interactive-grouping path: define a cluster
            // over the CURRENT selection and collapse it in one step.
            "collapse_selection" => match self.collapse_selection() {
                Some(id) => AgentActionReply::ok_with_log(json!({ "collapsed": id.0 })),
                None => AgentActionReply::err("collapse_selection requires a non-empty selection"),
            },
            "pin_selection" => {
                self.pin_selection();
                AgentActionReply::ok_with_log(json!({ "selection": selection_json(self) }))
            }
            "unpin_selection" => {
                self.unpin_selection();
                AgentActionReply::ok_with_log(json!({ "selection": selection_json(self) }))
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
                AgentActionReply::ok_with_log(camera_json(self))
            }
            "fit_view" => {
                self.fit_view();
                AgentActionReply::ok_with_log(camera_json(self))
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
            // Wave 2.3 label LOD — engine setter is `set_label_density`;
            // exposed as its own action (not folded into `set_forces`,
            // which is force-model-specific) so a density change is
            // screenshot-verifiable headlessly.
            "set_labels" => {
                let Some(density) = action.args.get("density").and_then(Value::as_f64) else {
                    return AgentActionReply::err("set_labels requires args.density (f64, labels per 100px cell at zoom 1.0)");
                };
                self.set_label_density(density);
                AgentActionReply::ok_with_log(json!({ "labels": { "density": self.label_density() } }))
            }
            // Wave 2.5 navigation — animated zoom-to-fit over whichever
            // nodes are currently effective (local-subgraph ∩ filter).
            "zoom_to_fit" => {
                let duration_ms = action.args.get("duration_ms").and_then(Value::as_f64).unwrap_or(DEFAULT_TRANSITION_MS);
                let padding = action.args.get("padding").and_then(Value::as_f64).unwrap_or(DEFAULT_FIT_PADDING_PX);
                self.zoom_to_fit(duration_ms, padding);
                AgentActionReply::ok_with_log(camera_json(self))
            }
            // Wave 2.5 navigation — animated pan(+zoom) to center a node.
            "zoom_to_node" => {
                let Some(node) = resolve_node(self, &action) else {
                    return AgentActionReply::err("zoom_to_node requires args.index (u32) or args.label (string)");
                };
                let duration_ms = action.args.get("duration_ms").and_then(Value::as_f64).unwrap_or(DEFAULT_TRANSITION_MS);
                let target_zoom = action.args.get("zoom").and_then(Value::as_f64);
                self.zoom_to_node(node, duration_ms, target_zoom);
                AgentActionReply::ok_with_log(camera_json(self))
            }
            // Wave 2.6 local subgraph — `{}`/`{"index": null}` restores
            // the full view (same "nothing means clear" convention
            // `hover_node`/`select_node` already use).
            "set_local_root" => {
                let index_arg = action.args.get("index");
                let explicit_clear = matches!(index_arg, Some(Value::Null))
                    || (index_arg.is_none() && action.args.get("label").is_none());
                if explicit_clear {
                    self.set_local_root(None, None);
                    return AgentActionReply::ok_with_log(json!({ "local_root": Value::Null }));
                }
                let Some(node) = resolve_node(self, &action) else {
                    return AgentActionReply::err("set_local_root requires args.index (u32), args.label (string), or {} / null to clear");
                };
                let depth = action.args.get("depth").and_then(Value::as_u64).map(|d| d as u8);
                self.set_local_root(Some(node), depth);
                AgentActionReply::ok_with_log(json!({
                    "local_root": self.local_root().map(|(n, d)| json!({ "index": n.index(), "depth": d })),
                }))
            }
            // Wave 2.6 query filter — an args object with none of the 3
            // recognized clauses (`{}`) clears the filter, same
            // convention `hover_node`'s bare `{}` uses.
            "set_filter" => {
                let has_clause = ["label_substring", "categories", "min_degree"].iter().any(|k| action.args.get(*k).is_some());
                if !has_clause {
                    self.set_filter(None);
                    return AgentActionReply::ok_with_log(json!({ "filter": Value::Null }));
                }
                let label_substring = action.args.get("label_substring").and_then(Value::as_str).map(str::to_owned);
                let categories = action.args.get("categories").and_then(Value::as_array).map(|arr| {
                    arr.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect::<Vec<String>>()
                });
                let min_degree = action.args.get("min_degree").and_then(Value::as_u64).map(|v| v as u32);
                let spec = FilterSpec { label_substring, categories, min_degree };
                self.set_filter(Some(spec));
                AgentActionReply::ok_with_log(json!({ "filter": self.filter().map(filter_json) }))
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

/// Parse `args.mode` (`"replace"`/`"union"`/`"diff"`) for `select_nodes`/
/// `box_select` — strings only at THIS agent JSON boundary (the Rust API
/// itself, `GraphEngine::{apply_selection,box_select}`, always takes the
/// typed [`SelectMode`] enum). Missing `mode` defaults to
/// [`SelectMode::Replace`]; an unrecognized string is an error, not a
/// silent fallback.
fn parse_select_mode(action: &AgentAction) -> Result<SelectMode, String> {
    match action.args.get("mode").and_then(Value::as_str) {
        None | Some("replace") => Ok(SelectMode::Replace),
        Some("union") => Ok(SelectMode::Union),
        Some("diff") => Ok(SelectMode::Diff),
        Some(other) => Err(format!("unknown select mode {other:?} (expected \"replace\"|\"union\"|\"diff\")")),
    }
}

/// `agent_state`'s `selection` field shape, shared with every selection-
/// mutating action's reply so a caller reads back the exact same JSON
/// either way. `indices` is capped at 50 (task gate) — `count` always
/// reports the TRUE selection size even when `indices` is truncated.
fn selection_json<N, E, L: Layout>(engine: &GraphEngine<N, E, L>) -> Value {
    const MAX_REPORTED_INDICES: usize = 50;
    let indices: Vec<u32> = engine.selection.iter().take(MAX_REPORTED_INDICES).map(|n| n.0).collect();
    json!({
        "count": engine.selection.len(),
        "indices": indices,
        "collapsed_group": engine.selection_collapsed_group().map(|id| id.0),
    })
}

/// Camera pan/zoom snapshot — shared by `agent_state`'s `camera` field and
/// the `set_camera`/`fit_view`/`zoom_to_fit`/`zoom_to_node` action
/// replies, so a caller reads back the exact same JSON shape either way.
fn camera_json<N, E, L: Layout>(engine: &GraphEngine<N, E, L>) -> Value {
    json!({
        "pan_x": engine.camera.pan_x,
        "pan_y": engine.camera.pan_y,
        "zoom": engine.camera.zoom,
    })
}

/// [`FilterSpec`] snapshot as JSON — shared by `agent_state`'s `filter`
/// field and the `set_filter` action's reply (Wave 2.6).
fn filter_json(filter: &FilterSpec) -> Value {
    json!({
        "label_substring": filter.label_substring,
        "categories": filter.categories,
        "min_degree": filter.min_degree,
    })
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
    fn set_labels_action_updates_density_and_agent_state_reports_it() {
        let (graph, _) = ring_graph(3);
        let mut engine: GraphEngine<(), (), ForceDirectedLayout> = GraphEngine::new(graph, ForceDirectedLayout::default());

        assert_eq!(engine.agent_state()["labels"]["density"], json!(crate::label_grid::DEFAULT_LABEL_DENSITY));
        assert_eq!(engine.agent_state()["labels"]["drawn_last_frame"], json!(0));

        let reply = engine.apply_agent_action(action("set_labels", json!({ "density": 3.0 })));
        assert!(reply.ok);
        assert_eq!(engine.label_density(), 3.0);
        assert_eq!(engine.agent_state()["labels"]["density"], json!(3.0));

        let bad = engine.apply_agent_action(action("set_labels", json!({})));
        assert!(!bad.ok, "set_labels without args.density must be an error reply, not a silent no-op");
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

    // ── W2.4 selection model (agent surface) ────────────────────────────────

    #[test]
    fn select_nodes_action_supports_replace_union_and_diff_modes() {
        let (graph, ids) = ring_graph(4);
        let mut engine: GraphEngine<(), (), ForceDirectedLayout> = GraphEngine::new(graph, ForceDirectedLayout::default());

        let reply = engine.apply_agent_action(action("select_nodes", json!({ "indices": [ids[0].index(), ids[1].index()] })));
        assert!(reply.ok);
        assert_eq!(engine.selection, [ids[0], ids[1]].into_iter().collect());
        assert_eq!(engine.agent_state()["selection"]["count"], json!(2));

        let union = engine.apply_agent_action(action("select_nodes", json!({ "indices": [ids[2].index()], "mode": "union" })));
        assert!(union.ok);
        assert_eq!(engine.selection, [ids[0], ids[1], ids[2]].into_iter().collect());

        let diff = engine.apply_agent_action(action("select_nodes", json!({ "indices": [ids[0].index()], "mode": "diff" })));
        assert!(diff.ok);
        assert_eq!(engine.selection, [ids[1], ids[2]].into_iter().collect());

        let bad_mode = engine.apply_agent_action(action("select_nodes", json!({ "indices": [ids[0].index()], "mode": "intersect" })));
        assert!(!bad_mode.ok, "an unrecognized mode string is an error reply, not a silent fallback");

        let out_of_range = engine.apply_agent_action(action("select_nodes", json!({ "indices": [999] })));
        assert!(!out_of_range.ok);

        let missing_indices = engine.apply_agent_action(action("select_nodes", json!({})));
        assert!(!missing_indices.ok);
    }

    #[test]
    fn box_select_action_selects_nodes_within_the_screen_rect() {
        use uzor_export::{render_to_png, ExportSpec};

        let mut graph = Graph::new();
        let a = graph.push_node((), "a", "x", 4.0);
        let b = graph.push_node((), "b", "x", 4.0);
        let c = graph.push_node((), "c", "x", 4.0);
        let mut engine: GraphEngine<(), (), ForceDirectedLayout> = GraphEngine::new(graph, ForceDirectedLayout::default());
        engine.set_canvas_rect(Rect::new(0.0, 0.0, 800.0, 600.0));
        engine.seed_positions(&[(0.0, 0.0), (50.0, 0.0), (500.0, 500.0)]);

        // Populate the visible/pick candidate set the same way a real
        // frame would — `draw()`'s internal `refresh_visible` is private
        // to `engine.rs`; a headless render is the public way to trigger
        // it from this module's own test scope.
        let spec = ExportSpec { width_px: 800, height_px: 600, dpr: 1.0, background: None };
        render_to_png(&spec, |ctx| engine.draw(ctx)).expect("headless render must succeed");

        let reply = engine.apply_agent_action(action("box_select", json!({ "x0": -10.0, "y0": -10.0, "x1": 60.0, "y1": 10.0 })));
        assert!(reply.ok);
        assert_eq!(engine.selection, [a, b].into_iter().collect());
        assert_eq!(engine.agent_state()["selection"]["count"], json!(2));
        let reported: Vec<u32> =
            engine.agent_state()["selection"]["indices"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as u32).collect();
        assert_eq!(reported, vec![a.0, b.0]);

        // Union mode adds `c` on top without dropping a/b.
        let union = engine.apply_agent_action(action(
            "box_select",
            json!({ "x0": 490.0, "y0": 490.0, "x1": 510.0, "y1": 510.0, "mode": "union" }),
        ));
        assert!(union.ok);
        assert_eq!(engine.selection, [a, b, c].into_iter().collect());

        let missing_coord = engine.apply_agent_action(action("box_select", json!({ "x0": 0.0, "y0": 0.0, "x1": 10.0 })));
        assert!(!missing_coord.ok, "box_select requires all 4 corner coords");
    }

    #[test]
    fn collapse_selection_pin_selection_and_unpin_selection_actions_round_trip() {
        let (graph, ids) = ring_graph(4);
        let mut engine: GraphEngine<(), (), ForceDirectedLayout> = GraphEngine::new(graph, ForceDirectedLayout::default());

        let empty = engine.apply_agent_action(action("collapse_selection", json!({})));
        assert!(!empty.ok, "collapse_selection with nothing selected is an error reply");

        engine.apply_agent_action(action("select_nodes", json!({ "indices": [ids[0].index(), ids[1].index()] })));

        let pin = engine.apply_agent_action(action("pin_selection", json!({})));
        assert!(pin.ok);
        assert!(engine.is_pinned(ids[0]));
        assert!(engine.is_pinned(ids[1]));

        let unpin = engine.apply_agent_action(action("unpin_selection", json!({})));
        assert!(unpin.ok);
        assert!(!engine.is_pinned(ids[0]));
        assert!(!engine.is_pinned(ids[1]));

        let collapse = engine.apply_agent_action(action("collapse_selection", json!({})));
        assert!(collapse.ok);
        let group_id = collapse.log_payload.as_ref().and_then(|d| d.get("collapsed")).and_then(Value::as_u64).expect("collapse reply carries the new GroupId");
        assert_eq!(engine.agent_state()["selection"]["collapsed_group"], json!(group_id));
    }

    // ── W2.5/W2.6 agent surface (zoom_to_fit, zoom_to_node, set_local_root, set_filter) ─

    #[test]
    fn zoom_to_fit_action_starts_an_animated_transition_reported_via_camera_transitioning() {
        let mut graph = Graph::new();
        graph.push_node((), "a", "x", 4.0);
        graph.push_node((), "b", "x", 4.0);
        let mut engine: GraphEngine<(), (), ForceDirectedLayout> = GraphEngine::new(graph, ForceDirectedLayout::default());
        engine.set_canvas_rect(Rect::new(0.0, 0.0, 800.0, 600.0));
        engine.seed_positions(&[(-50.0, 0.0), (50.0, 0.0)]);

        assert!(!engine.agent_state()["camera_transitioning"].as_bool().unwrap());
        let reply = engine.apply_agent_action(action("zoom_to_fit", json!({ "duration_ms": 300.0, "padding": 20.0 })));
        assert!(reply.ok);
        assert!(engine.camera_transitioning());
        assert!(engine.agent_state()["camera_transitioning"].as_bool().unwrap());

        for _ in 0..40 {
            engine.tick(1.0 / 60.0);
        }
        assert!(!engine.camera_transitioning(), "a 300ms transition must finish within 40 ticks at 60fps");
    }

    #[test]
    fn zoom_to_node_action_resolves_by_index_and_animates_the_camera_toward_it() {
        let (graph, ids) = ring_graph(3);
        let mut engine: GraphEngine<(), (), ForceDirectedLayout> = GraphEngine::new(graph, ForceDirectedLayout::default());
        engine.set_canvas_rect(Rect::new(0.0, 0.0, 800.0, 600.0));
        engine.seed_positions(&[(120.0, 0.0), (0.0, 120.0), (-120.0, -120.0)]);

        let reply = engine.apply_agent_action(action("zoom_to_node", json!({ "index": ids[0].index(), "duration_ms": 100.0, "zoom": 2.0 })));
        assert!(reply.ok);
        assert!(engine.camera_transitioning());

        for _ in 0..20 {
            engine.tick(1.0 / 60.0);
        }
        assert!(!engine.camera_transitioning());
        assert!((engine.agent_state()["camera"]["zoom"].as_f64().unwrap() - 2.0).abs() < 1e-6);

        let bad = engine.apply_agent_action(action("zoom_to_node", json!({})));
        assert!(!bad.ok, "zoom_to_node requires an index or label");
    }

    #[test]
    fn set_local_root_action_reports_local_root_in_agent_state_and_clears_on_null() {
        let (graph, ids) = ring_graph(4);
        let mut engine: GraphEngine<(), (), ForceDirectedLayout> = GraphEngine::new(graph, ForceDirectedLayout::default());
        engine.set_canvas_rect(Rect::new(0.0, 0.0, 800.0, 600.0));
        engine.seed_positions(&[(0.0, 0.0), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)]);

        assert_eq!(engine.agent_state()["local_root"], Value::Null);

        let reply = engine.apply_agent_action(action("set_local_root", json!({ "index": ids[0].index(), "depth": 1 })));
        assert!(reply.ok);
        assert_eq!(
            engine.agent_state()["local_root"],
            json!({ "index": ids[0].index(), "depth": 1 })
        );

        let cleared = engine.apply_agent_action(action("set_local_root", json!({ "index": Value::Null })));
        assert!(cleared.ok);
        assert_eq!(engine.agent_state()["local_root"], Value::Null);

        let bad = engine.apply_agent_action(action("set_local_root", json!({ "index": 999 })));
        assert!(!bad.ok, "an out-of-range index is an error reply, not a silent clear");
    }

    #[test]
    fn set_filter_action_reports_filter_in_agent_state_and_clears_on_empty_args() {
        let (graph, _ids) = ring_graph(4);
        let mut engine: GraphEngine<(), (), ForceDirectedLayout> = GraphEngine::new(graph, ForceDirectedLayout::default());

        assert_eq!(engine.agent_state()["filter"], Value::Null);

        let reply = engine.apply_agent_action(action("set_filter", json!({ "min_degree": 2, "categories": ["x"] })));
        assert!(reply.ok);
        let filter_state = engine.agent_state()["filter"].clone();
        assert_eq!(filter_state["min_degree"], json!(2));
        assert_eq!(filter_state["categories"], json!(["x"]));
        assert_eq!(filter_state["label_substring"], Value::Null);

        let cleared = engine.apply_agent_action(action("set_filter", json!({})));
        assert!(cleared.ok);
        assert_eq!(engine.agent_state()["filter"], Value::Null);
    }
}
