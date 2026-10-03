//! LayoutEngine part 2 tests (design §9.2 "Expand", "Drag-out"; brief F6
//! done-when list): expand origin invariants, inner inset, gutter drop,
//! drag-out with scripted outer-rect echoes, dwell, degradation, and the
//! exactly-one hit sweep with gutters.

use super::*;
use crate::types::anim::ExpandKind;
use crate::types::command::{DragOutChrome, DragOutPolicy};
use crate::types::window::{SizePx, WindowSpec};

/// The main window's outer rect (physical screen px, scale 1).
fn origin() -> Rect {
    Rect::new(100.0, 50.0, 1000.0, 600.0)
}

fn outer(e: &mut E, win: WindowId, rect: Rect, now: f64) -> LayoutEffects {
    e.apply(LayoutOp::OuterRect {
        win,
        now: Seconds(now),
        rect,
        scale: 1.0,
    })
}

/// `a | b` in a multi-window host whose outer rect is known.
fn expand_engine(policy: LayoutPolicy) -> (E, LeafId, LeafId) {
    let mut e = engine_with(policy);
    outer(&mut e, W, origin(), 0.5);
    let (a, b) = two_columns(&mut e, pn("a"), pn("b"));
    (e, a, b)
}

/// Start a live header drag of `b` (its header is at y 0..24, x 500..1000).
fn drag_b(e: &mut E) {
    down(e, 700.0, 12.0);
    mv(e, 700.0, 60.0);
    assert!(matches!(
        e.view().window(W).expect("W").session(),
        Some(SessionKind::PanelDrag { .. })
    ));
}

fn value(e: &mut E, kind: ExpandKind, t: f64) -> LayoutEffects {
    e.apply(LayoutOp::ExpandValue { win: W, kind, t })
}

fn outer_cmds(fx: &LayoutEffects) -> Vec<Rect> {
    fx.iter()
        .filter_map(|f| match f {
            LayoutEffect::Window {
                cmd: WindowCommand::SetOuterRect { position, size },
                ..
            } => Some(Rect::new(
                position.0 as f64,
                position.1 as f64,
                size.width as f64,
                size.height as f64,
            )),
            _ => None,
        })
        .collect()
}

fn targets(fx: &LayoutEffects) -> Vec<(ExpandKind, Option<EdgeSide>)> {
    fx.iter()
        .filter_map(|f| match f {
            LayoutEffect::ExpandTarget { kind, side, .. } => Some((*kind, *side)),
            _ => None,
        })
        .collect()
}

fn done(fx: &LayoutEffects) -> Vec<ExpandKind> {
    fx.iter()
        .filter_map(|f| match f {
            LayoutEffect::ExpandDone { kind, .. } => Some(*kind),
            _ => None,
        })
        .collect()
}

fn expand_of(e: &E) -> Option<ExpandState> {
    e.view().window(W).expect("W").expand()
}

fn outer_policy() -> LayoutPolicy {
    LayoutPolicy {
        left_top_expand: ExpandKind::Outer,
        ..LayoutPolicy::default()
    }
}

/// Sweep `t` 0 → 1 → 0 through `ExpandValue` (with the gutter released
/// half way), checking every sent rect with `check(t, rect)`; returns the
/// rects sent on the way up and the effects of the final `t = 0`.
fn sweep(e: &mut E, kind: ExpandKind, check: impl Fn(f64, Rect)) -> LayoutEffects {
    let mut last = origin();
    for i in 0..=20 {
        let t = i as f64 / 20.0;
        let fx = value(e, kind, t);
        for r in outer_cmds(&fx) {
            check(t, r);
            last = r;
        }
    }
    assert_eq!(expand_of(e).map(|x| x.t()), Some(1.0));
    let _ = last;
    // Leave the gutter (and the viewport): retract.
    let fx = mv(e, 600.0, 5000.0);
    assert_eq!(targets(&fx), vec![(kind, None)]);
    for i in (1..20).rev() {
        let t = i as f64 / 20.0;
        let fx = value(e, kind, t);
        for r in outer_cmds(&fx) {
            check(t, r);
        }
        assert!(done(&fx).is_empty(), "not done before t = 0");
    }
    value(e, kind, 0.0)
}

// ---------------------------------------------------------------------------
// Expand
// ---------------------------------------------------------------------------

#[test]
fn outer_left_expand_keeps_the_right_edge_fixed_and_retracts_to_the_exact_origin() {
    let (mut e, _, _) = expand_engine(outer_policy());
    drag_b(&mut e);
    let fx = mv(&mut e, 3.0, 300.0);
    assert_eq!(
        e.hit(W, Point::new(3.0, 300.0)),
        LayoutHit::EdgeGutter(EdgeSide::Left)
    );
    assert_eq!(
        targets(&fx),
        vec![(ExpandKind::Outer, Some(EdgeSide::Left))]
    );
    let x = expand_of(&e).expect("latched");
    assert_eq!(x.target().origin(), origin());
    assert_eq!(x.target().thickness_px(), 480.0);
    let o = origin();
    let fx = sweep(&mut e, ExpandKind::Outer, |t, r| {
        let extent = (480.0 * t).round();
        assert_eq!(r.x + r.width, o.x + o.width, "right edge fixed at t {t}");
        assert_eq!(
            r.x,
            o.x - extent,
            "x decreases exactly as width grows at t {t}"
        );
        assert_eq!((r.y, r.height), (o.y, o.height));
    });
    assert_eq!(outer_cmds(&fx), vec![origin()], "exact original rect");
    assert_eq!(done(&fx), vec![ExpandKind::Outer]);
    assert_eq!(expand_of(&e), None);
    assert_eq!(e.view().window(W).expect("W").content_rect(), vp());
}

#[test]
fn outer_left_expand_shifts_the_content_by_the_extent() {
    let (mut e, _, _) = expand_engine(outer_policy());
    drag_b(&mut e);
    mv(&mut e, 3.0, 300.0);
    let fx = value(&mut e, ExpandKind::Outer, 0.5);
    assert_eq!(
        outer_cmds(&fx),
        vec![Rect::new(-140.0, 50.0, 1240.0, 600.0)]
    );
    // The host echoes the grown viewport; the content keeps its screen place.
    e.apply(LayoutOp::Solve {
        win: W,
        viewport: Rect::new(0.0, 0.0, 1240.0, 600.0),
    });
    let w = e.view().window(W).expect("W");
    assert_eq!(w.expand().map(|x| x.inset()), Some(Point::new(240.0, 0.0)));
    assert_eq!(w.content_rect(), Rect::new(240.0, 0.0, 1000.0, 600.0));
    assert_eq!(w.dock_area(), Rect::new(240.0, 0.0, 1000.0, 600.0));
    // The whole grown strip stays the latched gutter.
    assert_eq!(
        e.hit(W, Point::new(120.0, 300.0)),
        LayoutHit::EdgeGutter(EdgeSide::Left)
    );
}

#[test]
fn outer_top_expand_keeps_the_bottom_edge_fixed_and_retracts_to_the_exact_origin() {
    let (mut e, _, _) = expand_engine(outer_policy());
    drag_b(&mut e);
    let fx = mv(&mut e, 300.0, 3.0);
    assert_eq!(targets(&fx), vec![(ExpandKind::Outer, Some(EdgeSide::Top))]);
    let o = origin();
    let fx = sweep(&mut e, ExpandKind::Outer, |t, r| {
        let extent = (480.0 * t).round();
        assert_eq!(r.y + r.height, o.y + o.height, "bottom edge fixed at t {t}");
        assert_eq!(
            r.y,
            o.y - extent,
            "y decreases exactly as height grows at t {t}"
        );
        assert_eq!((r.x, r.width), (o.x, o.width));
    });
    assert_eq!(outer_cmds(&fx), vec![origin()]);
    assert_eq!(done(&fx), vec![ExpandKind::Outer]);
    assert_eq!(expand_of(&e), None);
}

#[test]
fn right_and_bottom_expand_grow_toward_the_gutter() {
    for (p, side) in [
        (Point::new(995.0, 300.0), EdgeSide::Right),
        (Point::new(300.0, 595.0), EdgeSide::Bottom),
    ] {
        let (mut e, _, _) = expand_engine(LayoutPolicy::default());
        drag_b(&mut e);
        let fx = mv(&mut e, p.x, p.y);
        assert_eq!(targets(&fx), vec![(ExpandKind::Outer, Some(side))]);
        let o = origin();
        let fx = sweep(&mut e, ExpandKind::Outer, |t, r| {
            let extent = (480.0 * t).round();
            assert_eq!((r.x, r.y), (o.x, o.y), "position fixed");
            match side {
                EdgeSide::Right => assert_eq!((r.width, r.height), (o.width + extent, o.height)),
                _ => assert_eq!((r.width, r.height), (o.width, o.height + extent)),
            }
        });
        assert_eq!(outer_cmds(&fx), vec![origin()]);
        assert_eq!(e.view().window(W).expect("W").expand(), None, "{side:?}");
    }
}

#[test]
fn inner_expand_keeps_the_position_and_shifts_the_content_by_the_inset() {
    // Default policy: Left / Top latch the inner expand.
    for (p, side) in [
        (Point::new(3.0, 300.0), EdgeSide::Left),
        (Point::new(300.0, 3.0), EdgeSide::Top),
    ] {
        let (mut e, _, _) = expand_engine(LayoutPolicy::default());
        drag_b(&mut e);
        let fx = mv(&mut e, p.x, p.y);
        assert_eq!(targets(&fx), vec![(ExpandKind::Inner, Some(side))]);
        // An outer value is not this expand's.
        assert!(value(&mut e, ExpandKind::Outer, 0.5).is_empty());
        let fx = value(&mut e, ExpandKind::Inner, 0.5);
        let o = origin();
        let grown = match side {
            EdgeSide::Left => Rect::new(o.x, o.y, o.width + 240.0, o.height),
            _ => Rect::new(o.x, o.y, o.width, o.height + 240.0),
        };
        assert_eq!(outer_cmds(&fx), vec![grown], "origin never shifts");
        let w = e.view().window(W).expect("W");
        let inset = match side {
            EdgeSide::Left => Point::new(240.0, 0.0),
            _ => Point::new(0.0, 240.0),
        };
        assert_eq!(w.expand().map(|x| x.inset()), Some(inset));
        assert_eq!(w.dock_area().x, inset.x);
        assert_eq!(w.dock_area().y, inset.y);
        assert_eq!((w.dock_area().width, w.dock_area().height), (1000.0, 600.0));
        let fx = sweep(&mut e, ExpandKind::Inner, |_, r| {
            assert_eq!((r.x, r.y), (o.x, o.y), "origin never shifts");
        });
        assert_eq!(outer_cmds(&fx), vec![origin()]);
        assert_eq!(done(&fx), vec![ExpandKind::Inner]);
        let w = e.view().window(W).expect("W");
        assert_eq!(w.dock_area(), vp());
    }
}

#[test]
fn release_in_the_gutter_grows_fully_and_docks_the_panel_into_the_strip() {
    let (mut e, a, b) = expand_engine(LayoutPolicy::default());
    drag_b(&mut e);
    mv(&mut e, 995.0, 300.0);
    value(&mut e, ExpandKind::Outer, 0.4);
    let fx = up(&mut e, 996.0, 300.0);
    assert_eq!(
        outer_cmds(&fx),
        vec![Rect::new(100.0, 50.0, 1480.0, 600.0)],
        "full extent at once"
    );
    assert_eq!(done(&fx), vec![ExpandKind::Outer]);
    let moved = intents(&fx);
    let [DockIntent::PanelMoved { from, to, zone }] = moved.as_slice() else {
        panic!("{moved:?}");
    };
    assert_eq!((*from, *zone), ((W, b), DropZone::Right));
    assert_eq!(to.0, W);
    assert_eq!(captures(&fx), vec![false]);
    assert_eq!(expand_of(&e), None);
    // The host echoes the grown window: the new leaf is exactly the strip.
    e.apply(LayoutOp::Solve {
        win: W,
        viewport: Rect::new(0.0, 0.0, 1480.0, 600.0),
    });
    assert_eq!(tabs(&e), vec![vec!["a"], vec!["b"]]);
    let strip = leaf_rect(&e, to.1);
    assert!((strip.width - 480.0).abs() < 0.5, "{strip:?}");
    assert!((leaf_rect(&e, a).width - 1000.0).abs() < 0.5);
    // A later value for the finished expand does nothing.
    assert!(value(&mut e, ExpandKind::Outer, 0.2).is_empty());
}

#[test]
fn a_second_side_waits_until_the_retract_is_over() {
    let (mut e, _, _) = expand_engine(LayoutPolicy::default());
    drag_b(&mut e);
    mv(&mut e, 995.0, 300.0);
    value(&mut e, ExpandKind::Outer, 0.5);
    // Straight into the bottom gutter: the right expand cools, nothing new
    // latches while it retracts.
    let fx = mv(&mut e, 300.0, 595.0);
    assert_eq!(targets(&fx), vec![(ExpandKind::Outer, None)]);
    assert_eq!(
        expand_of(&e).map(|x| x.target().side()),
        Some(EdgeSide::Right)
    );
    // Back to the right: it heats again.
    let fx = mv(&mut e, 997.0, 300.0);
    assert_eq!(
        targets(&fx),
        vec![(ExpandKind::Outer, Some(EdgeSide::Right))]
    );
    // Cancel: cools and retracts to the exact origin.
    let fx = ptr(&mut e, DockPointer::Cancel);
    assert_eq!(targets(&fx), vec![(ExpandKind::Outer, None)]);
    let fx = value(&mut e, ExpandKind::Outer, 0.0);
    assert_eq!(outer_cmds(&fx), vec![origin()]);
    // Now the bottom may latch.
    drag_b(&mut e);
    let fx = mv(&mut e, 300.0, 595.0);
    assert_eq!(
        targets(&fx),
        vec![(ExpandKind::Outer, Some(EdgeSide::Bottom))]
    );
}

#[test]
fn expand_never_latches_without_multi_window_or_an_outer_rect() {
    // Web caps.
    let mut e = E::with_policy(decode, LayoutPolicy::default());
    e.apply(LayoutOp::Open {
        win: W,
        caps: HostCaps {
            multi_window: false,
            ..os_caps()
        },
        viewport: vp(),
    });
    outer(&mut e, W, origin(), 0.5);
    two_columns(&mut e, pn("a"), pn("b"));
    drag_b(&mut e);
    for p in [(3.0, 300.0), (995.0, 300.0), (300.0, 3.0), (300.0, 595.0)] {
        assert!(targets(&mv(&mut e, p.0, p.1)).is_empty());
        assert!(!matches!(
            e.hit(W, Point::new(p.0, p.1)),
            LayoutHit::EdgeGutter(_)
        ));
    }
    assert_eq!(expand_of(&e), None);
    assert!(targets(&up(&mut e, 995.0, 300.0)).is_empty());

    // Multi-window host that never echoed an outer rect.
    let mut e = engine();
    two_columns(&mut e, pn("a"), pn("b"));
    drag_b(&mut e);
    assert!(targets(&mv(&mut e, 995.0, 300.0)).is_empty());
    assert_eq!(expand_of(&e), None);
}

#[test]
fn gutters_exist_only_during_a_live_panel_drag() {
    let (mut e, _, b) = expand_engine(LayoutPolicy::default());
    assert_eq!(e.hit(W, Point::new(995.0, 300.0)), LayoutHit::PanelBody(b));
    down(&mut e, 700.0, 12.0); // pressed, not dragging yet
    assert_eq!(e.hit(W, Point::new(995.0, 300.0)), LayoutHit::PanelBody(b));
    mv(&mut e, 700.0, 60.0);
    assert_eq!(
        e.hit(W, Point::new(995.0, 300.0)),
        LayoutHit::EdgeGutter(EdgeSide::Right)
    );
    ptr(&mut e, DockPointer::Cancel);
    assert_eq!(e.hit(W, Point::new(995.0, 300.0)), LayoutHit::PanelBody(b));
}

#[test]
fn expand_revision_bumps_exactly_on_change() {
    let (mut e, _, _) = expand_engine(LayoutPolicy::default());
    drag_b(&mut e);
    let r = e.revision();
    mv(&mut e, 995.0, 300.0);
    assert_eq!(e.revision(), r.next(), "latch");
    let r = e.revision();
    value(&mut e, ExpandKind::Inner, 0.5);
    outer(&mut e, W, origin(), 0.6);
    assert_eq!(e.revision(), r, "foreign kind and same echo change nothing");
    value(&mut e, ExpandKind::Outer, 0.5);
    assert_eq!(e.revision(), r.next());
    let r = e.revision();
    value(&mut e, ExpandKind::Outer, 0.5);
    assert_eq!(e.revision(), r, "same value");
}

#[test]
fn gutter_sweep_gives_exactly_one_consistent_hit_per_point() {
    let (mut e, _, b) = furnished();
    outer(&mut e, W, origin(), 0.5);
    // Live drag of b's header (at y 32..56 below the chrome).
    down(&mut e, 800.0, 40.0);
    mv(&mut e, 800.0, 100.0);
    assert_eq!(
        e.view().window(W).expect("W").session(),
        Some(SessionKind::PanelDrag { leaf: b })
    );
    let check = |e: &E, latched: bool| {
        let w = win(e);
        let policy = LayoutPolicy::default();
        let mut sides = std::collections::HashSet::new();
        let mut y = -3.0;
        while y < 605.0 {
            let mut x = -3.0;
            while x < 1005.0 {
                let p = Point::new(x, y);
                let hit = e.hit(W, p);
                assert_eq!(hit, e.hit(W, p));
                assert_eq!(hit, e.view().hit(W, p));
                let g = expand::gutter(w, &policy, p, true);
                match hit {
                    LayoutHit::EdgeGutter(side) => {
                        assert_eq!(g, Some(side), "{p:?}");
                        sides.insert(side);
                    }
                    // A gutter point is never anything else, even the bezel.
                    other => assert_eq!(g, None, "{other:?} at {p:?}"),
                }
                if !rects::contains(w.viewport, p) {
                    assert_eq!(hit, LayoutHit::None);
                }
                x += 7.0;
            }
            y += 7.0;
        }
        if latched {
            assert!(!sides.is_empty());
        } else {
            assert_eq!(sides.len(), 4, "{sides:?}");
        }
    };
    check(&e, false);
    // The chrome and the toolbar widen the Top / Left bands.
    assert_eq!(
        e.hit(W, Point::new(500.0, 20.0)),
        LayoutHit::EdgeGutter(EdgeSide::Top)
    );
    assert_eq!(
        e.hit(W, Point::new(30.0, 300.0)),
        LayoutHit::EdgeGutter(EdgeSide::Left)
    );
    assert_eq!(
        e.hit(W, Point::new(800.0, 40.0)),
        LayoutHit::PanelHeader(b),
        "just below the bands the dock answers"
    );
    // Latched and half grown: the strip is the gutter, the content moved.
    mv(&mut e, 998.0, 300.0);
    value(&mut e, ExpandKind::Outer, 0.25);
    e.apply(LayoutOp::Solve {
        win: W,
        viewport: Rect::new(0.0, 0.0, 1120.0, 600.0),
    });
    assert_eq!(
        e.hit(W, Point::new(1100.0, 300.0)),
        LayoutHit::EdgeGutter(EdgeSide::Right)
    );
    check(&e, true);
}

// ---------------------------------------------------------------------------
// Drag-out
// ---------------------------------------------------------------------------

const M: WindowId = WindowId(3);
const M2: WindowId = WindowId(4);

fn dragout_policy(chrome: DragOutChrome) -> LayoutPolicy {
    LayoutPolicy {
        drag_out: DragOutPolicy::Enabled { chrome },
        ..LayoutPolicy::default()
    }
}

/// The sibling main window W2 at screen (1300, 50), 800 x 600, one leaf `c`.
fn open_sibling(e: &mut E) -> LeafId {
    e.apply(LayoutOp::Open {
        win: W2,
        caps: os_caps(),
        viewport: Rect::new(0.0, 0.0, 800.0, 600.0),
    });
    outer(e, W2, Rect::new(1300.0, 50.0, 800.0, 600.0), 0.5);
    cmd(
        e,
        LayoutCmd::OpenPanel {
            win: W2,
            panel: pn("c"),
            at: DockTarget::Root,
        },
    );
    e.view().window(W2).expect("W2").dock_view().leaves[0].leaf
}

fn spawns(fx: &LayoutEffects) -> Vec<(WindowId, WindowSpec)> {
    fx.iter()
        .filter_map(|f| match f {
            LayoutEffect::SpawnWindow { src, spec } => Some((*src, spec.clone())),
            _ => None,
        })
        .collect()
}

fn window_cmds(fx: &LayoutEffects, target: WindowId) -> Vec<String> {
    fx.iter()
        .filter_map(|f| match f {
            LayoutEffect::Window { win, cmd } if *win == target => Some(format!("{cmd:?}")),
            _ => None,
        })
        .collect()
}

fn tabs_of(e: &E, w: WindowId) -> Vec<Vec<&'static str>> {
    e.view()
        .window(w)
        .expect("window")
        .dock_view()
        .leaves
        .iter()
        .map(|l| l.panels.iter().map(|p| p.type_id).collect())
        .collect()
}

/// Drag `b` out of W to window-local (1150, 300) = screen (1250, 350), in
/// the gap between W and W2; the kernel's spawn / adopt / create round trip;
/// the micro-window's first outer-rect echo at the spec position.
fn drag_out_b(e: &mut E) -> (WindowSpec, Rect) {
    drag_b(e);
    mv(e, 1150.0, 300.0);
    let fx = up(e, 1150.0, 300.0);
    let sp = spawns(&fx);
    assert_eq!(sp.len(), 1, "{fx:?}");
    assert_eq!(sp[0].0, W);
    assert!(intents(&fx).is_empty());
    let spec = sp[0].1.clone();
    let fx = e.apply(LayoutOp::AdoptPanel { win: M });
    assert_eq!(
        intents(&fx),
        vec![DockIntent::PanelDraggedOut {
            from: (W, leaf_b(e)),
            to: M
        }]
    );
    e.apply(LayoutOp::Open {
        win: M,
        caps: os_caps(),
        viewport: Rect::new(
            0.0,
            0.0,
            spec.inner_size.width as f64,
            spec.inner_size.height as f64,
        ),
    });
    let pos = spec.position.expect("placed under the cursor");
    let rect = Rect::new(
        pos.0 as f64,
        pos.1 as f64,
        spec.inner_size.width as f64,
        spec.inner_size.height as f64,
    );
    outer(e, M, rect, 2.0);
    (spec, rect)
}

fn leaf_b(e: &E) -> LeafId {
    e.view().drag_out().expect("session").leaf()
}

#[test]
fn drag_out_spawns_a_micro_window_and_dwell_docks_it_back() {
    let (mut e, _, _) = expand_engine(dragout_policy(DragOutChrome::None));
    let c = open_sibling(&mut e);
    let (spec, rect) = drag_out_b(&mut e);

    // The spec: sized to the panel (500 x 600 leaf, capped at 92 % of the
    // source viewport height), no decorations, cursor on the grab point.
    assert!(!spec.decorations);
    assert_eq!(spec.inner_size, SizePx::new(500, 552));
    assert_eq!(spec.title, "b");
    let header = win(&e).dock.tab_bar_height() as f64;
    assert_eq!(
        spec.position,
        Some((1250 - 60, (350.0 - header / 2.0).round() as i32))
    );
    assert_eq!(tabs(&e), vec![vec!["a"]], "b left W");
    assert_eq!(tabs_of(&e, M), vec![vec!["b"]], "b is in the micro-window");
    let mw = e.view().window(M).expect("M");
    assert_eq!(mw.micro(), Some(DragOutChrome::None));
    let view = e.view().drag_out().expect("session");
    assert_eq!(
        (view.src(), view.micro(), view.placed()),
        (W, Some(M), true)
    );

    // The first echo (at the spawn point) is not a move. Moved over W2 at
    // 2.0, but the next tick comes 0.8 s later: not fresh, no dwell.
    let over_w2 = |r: Rect, dx: f64| Rect::new(1500.0 + dx, 200.0, r.width, r.height);
    outer(&mut e, M, over_w2(rect, 0.0), 2.0);
    e.tick(Seconds(2.8));
    assert_eq!(
        e.view().drag_out().and_then(|d| d.dwell()),
        None,
        "no fresh move"
    );

    // A fresh move over W2 starts the dwell ...
    outer(&mut e, M, over_w2(rect, 5.0), 3.0);
    e.tick(Seconds(3.25));
    assert_eq!(
        e.view().drag_out().and_then(|d| d.dwell()),
        Some((W2, Seconds(3.25)))
    );
    assert_eq!(e.view().next_deadline(), Some(Seconds(4.25)));
    // ... runs while the user holds still ...
    let fx = e.tick(Seconds(4.0));
    assert!(intents(&fx).is_empty());
    // ... and completes: b docks into c's leaf, the micro-window hides, closes.
    let fx = e.tick(Seconds(4.25));
    let moved: Vec<DockIntent> = intents(&fx)
        .into_iter()
        .filter(|i| !matches!(i, DockIntent::LayoutChanged { .. }))
        .collect();
    let [DockIntent::PanelMoved { from, to, zone }] = moved.as_slice() else {
        panic!("{moved:?}");
    };
    assert_eq!((from.0, *to, *zone), (M, (W2, c), DropZone::Center));
    assert_eq!(
        window_cmds(&fx, M),
        vec!["SetVisible(false)".to_string(), "Close".to_string()]
    );
    assert_eq!(tabs_of(&e, W2), vec![vec!["c", "b"]]);
    let c_view = e.view().window(W2).expect("W2").dock_view();
    assert_eq!(c_view.leaves[0].active_tab, 1, "the docked panel is active");
    assert_eq!(e.view().drag_out(), None);
    assert!(tabs_of(&e, M).is_empty());
    e.apply(LayoutOp::Close(M));
    assert!(e.view().window(M).is_none());
}

#[test]
fn a_parked_micro_window_never_merges_by_itself() {
    let (mut e, _, _) = expand_engine(dragout_policy(DragOutChrome::UzorChrome));
    open_sibling(&mut e);
    let (spec, rect) = drag_out_b(&mut e);
    // UzorChrome: the chrome strip is shown and sized in.
    assert!(!spec.decorations);
    assert_eq!(spec.inner_size, SizePx::new(500, 552));
    let mw = e.view().window(M).expect("M");
    assert!(mw.chrome_view().visible);
    // Move over W2 at 3.0, dwell starts at 3.1, but the next tick comes only
    // after the move went stale (> 1.25 s): the dwell is dropped.
    outer(
        &mut e,
        M,
        Rect::new(1500.0, 200.0, rect.width, rect.height),
        3.0,
    );
    e.tick(Seconds(3.1));
    assert!(e.view().drag_out().and_then(|d| d.dwell()).is_some());
    let fx = e.tick(Seconds(4.3));
    assert!(intents(&fx).is_empty());
    assert_eq!(e.view().drag_out().and_then(|d| d.dwell()), None);
    assert_eq!(tabs_of(&e, M), vec![vec!["b"]]);
    // Leaving every sibling clears a running dwell too.
    outer(
        &mut e,
        M,
        Rect::new(1500.0, 210.0, rect.width, rect.height),
        5.0,
    );
    e.tick(Seconds(5.1));
    assert!(e.view().drag_out().and_then(|d| d.dwell()).is_some());
    outer(
        &mut e,
        M,
        Rect::new(4000.0, 4000.0, rect.width, rect.height),
        5.2,
    );
    e.tick(Seconds(5.3));
    assert_eq!(e.view().drag_out().and_then(|d| d.dwell()), None);
}

#[test]
fn chrome_less_micro_window_follows_its_header_and_that_move_is_fresh() {
    let (mut e, _, _) = expand_engine(dragout_policy(DragOutChrome::None));
    open_sibling(&mut e);
    let (_, rect) = drag_out_b(&mut e);
    let mp = |e: &mut E, event: DockPointer, now: f64| {
        e.apply(LayoutOp::Pointer {
            win: M,
            now: Seconds(now),
            event,
        })
    };
    // Press the panel header in the micro-window: it moves the OS window.
    let at = Point::new(100.0, 10.0);
    assert!(matches!(e.hit(M, at), LayoutHit::PanelHeader(_)));
    let fx = mp(
        &mut e,
        DockPointer::Down {
            pos: at,
            button: MouseButton::Left,
        },
        3.0,
    );
    assert_eq!(captures(&fx), vec![true]);
    assert_eq!(
        e.view().window(M).expect("M").session(),
        Some(SessionKind::MicroMove)
    );
    // Move by (+310, -140): the window follows, the cursor stays on `at`,
    // so its screen point lands over W2.
    let fx = mp(&mut e, DockPointer::Move(Point::new(410.0, -130.0)), 3.2);
    let moved = Rect::new(rect.x + 310.0, rect.y - 140.0, rect.width, rect.height);
    assert_eq!(outer_cmds(&fx), vec![moved]);
    assert_eq!(e.view().window(M).expect("M").outer_rect(), Some(moved));
    e.tick(Seconds(3.3));
    let screen = Point::new(moved.x + at.x, moved.y + at.y);
    assert!(screen.x >= 1300.0, "{screen:?}");
    assert_eq!(
        e.view().drag_out().and_then(|d| d.dwell()),
        Some((W2, Seconds(3.3)))
    );
    // Nothing tears off a micro-window and it has no gutters.
    let fx = mp(&mut e, DockPointer::Up(Point::new(410.0, -130.0)), 3.4);
    assert_eq!(captures(&fx), vec![false]);
    let fx = e.tick(Seconds(4.3));
    assert_eq!(
        window_cmds(&fx, M).last().map(String::as_str),
        Some("Close")
    );
}

#[test]
fn a_micro_window_is_never_a_drop_target() {
    let (mut e, a, _) = expand_engine(dragout_policy(DragOutChrome::Os));
    cmd(
        &mut e,
        LayoutCmd::OpenPanel {
            win: W,
            panel: pn("d"),
            at: DockTarget::Leaf(a, DropZone::Down),
        },
    );
    let (spec, rect) = drag_out_b(&mut e);
    assert!(spec.decorations, "Os chrome");
    // Park the micro-window somewhere empty.
    let parked = Rect::new(3000.0, 100.0, rect.width, rect.height);
    outer(&mut e, M, parked, 2.5);

    // A second drag-out released over the micro-window's screen rect: it is
    // not a target, a new micro-window is requested instead.
    let d_leaf = leaf_of(&e, &["d"]);
    let hdr = leaf_rect(&e, d_leaf);
    down(&mut e, hdr.x + 50.0, hdr.y + 10.0);
    mv(&mut e, hdr.x + 50.0, hdr.y + 60.0);
    let local = Point::new(3000.0 + 50.0 - 100.0, 100.0 + 50.0 - 50.0);
    mv(&mut e, local.x, local.y);
    let fx = up(&mut e, local.x, local.y);
    assert_eq!(spawns(&fx).len(), 1, "{fx:?}");
    assert_eq!(
        tabs_of(&e, M),
        vec![vec!["b"]],
        "the micro-window is untouched"
    );
    e.apply(LayoutOp::AdoptPanel { win: M2 });
    e.apply(LayoutOp::Open {
        win: M2,
        caps: os_caps(),
        viewport: Rect::new(0.0, 0.0, 320.0, 240.0),
    });
    assert_eq!(tabs_of(&e, M2), vec![vec!["d"]]);

    // Moving M2 over the micro-window M never starts a dwell.
    outer(&mut e, M2, Rect::new(2990.0, 100.0, 320.0, 240.0), 3.0);
    outer(&mut e, M2, Rect::new(3000.0, 100.0, 320.0, 240.0), 3.1);
    e.tick(Seconds(3.2));
    assert_eq!(e.view().drag_out().and_then(|d| d.dwell()), None);
    let fx = e.tick(Seconds(4.5));
    assert!(intents(&fx).is_empty());
}

#[test]
fn release_outside_over_a_sibling_docks_there_at_once() {
    let (mut e, _, b) = expand_engine(dragout_policy(DragOutChrome::UzorChrome));
    let c = open_sibling(&mut e);
    drag_b(&mut e);
    // Window-local (1500, 300) = screen (1600, 350): inside W2's leaf c.
    mv(&mut e, 1500.0, 300.0);
    let fx = up(&mut e, 1500.0, 300.0);
    assert!(spawns(&fx).is_empty());
    assert_eq!(
        intents(&fx),
        vec![DockIntent::PanelMoved {
            from: (W, b),
            to: (W2, c),
            zone: DropZone::Center
        }]
    );
    assert_eq!(tabs(&e), vec![vec!["a"]]);
    assert_eq!(tabs_of(&e, W2), vec![vec!["c", "b"]]);
    assert_eq!(e.view().drag_out(), None);
}

#[test]
fn disabled_policy_and_web_caps_degrade_to_an_in_window_float() {
    let check = |mut e: E| {
        outer(&mut e, W, origin(), 0.5);
        let (_, b) = two_columns(&mut e, pn("a"), pn("b"));
        drag_b(&mut e);
        mv(&mut e, 1150.0, 300.0);
        let fx = up(&mut e, 1150.0, 300.0);
        assert!(spawns(&fx).is_empty());
        assert_eq!(
            intents(&fx),
            vec![DockIntent::PanelTornOff {
                win: W,
                leaf: b,
                index: 0
            }]
        );
        let view = e.view().window(W).expect("W").dock_view();
        assert_eq!(view.floating.len(), 1);
        assert_eq!(e.view().drag_out(), None);
    };
    // Disabled (the default).
    check(engine());
    // Enabled on a single-window host.
    let mut e = E::with_policy(decode, dragout_policy(DragOutChrome::None));
    e.apply(LayoutOp::Open {
        win: W,
        caps: HostCaps {
            multi_window: false,
            ..os_caps()
        },
        viewport: vp(),
    });
    check(e);
}

#[test]
fn drag_out_revision_bumps_once_per_change() {
    let (mut e, _, _) = expand_engine(dragout_policy(DragOutChrome::None));
    open_sibling(&mut e);
    let (_, rect) = drag_out_b(&mut e);
    let r = e.revision();
    e.tick(Seconds(2.2));
    assert_eq!(e.revision(), r, "nothing to do");
    outer(
        &mut e,
        M,
        Rect::new(1500.0, 200.0, rect.width, rect.height),
        3.0,
    );
    assert_eq!(e.revision(), r.next(), "the echo moved the window");
    let r = e.revision();
    e.tick(Seconds(3.1));
    assert_eq!(e.revision(), r.next(), "dwell started");
    let r = e.revision();
    e.tick(Seconds(3.5));
    assert_eq!(e.revision(), r, "dwell running");
    e.tick(Seconds(4.2));
    assert_eq!(e.revision(), r.next(), "merge: one bump for both windows");
}
