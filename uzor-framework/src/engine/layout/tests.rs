//! LayoutEngine tests (design §9.2 "Splitter policy", "Dock tab merge",
//! "Layout blob"; brief F5 done-when list). Driven through `apply` / `tick`
//! / `hit` the way the kernel will drive them.

use super::*;
use crate::types::command::{ChromeKind, ChromeTab, SplitDir, SplitterPolicy};
use crate::types::ids::Ticket;
use crate::types::layout_blob::{LayoutBlob, LayoutCodecError};
use uzor::input::MouseButton;
use uzor::layout::docking::{
    BranchId, DropZone, Pin, SeparatorOrientation, WindowLayout as Preset,
};
use uzor::layout::{EdgePlacement, EdgeSlot};
use uzor::widgets::composite::chrome::{ChromeAction, ChromeHit, ChromeLayoutConfig};

const W: WindowId = WindowId(1);
const W2: WindowId = WindowId(2);

#[derive(Clone, Debug)]
struct Pn {
    kind: &'static str,
    min: (f32, f32),
    pin: Pin,
    closable: bool,
}

impl DockPanel for Pn {
    fn title(&self) -> &str {
        self.kind
    }
    fn type_id(&self) -> &'static str {
        self.kind
    }
    fn min_size(&self) -> (f32, f32) {
        self.min
    }
    fn closable(&self) -> bool {
        self.closable
    }
    fn pin(&self) -> Pin {
        self.pin
    }
}

fn pn(kind: &'static str) -> Pn {
    Pn {
        kind,
        min: (0.0, 0.0),
        pin: Pin::Free,
        closable: true,
    }
}

fn pn_min(kind: &'static str, min: f32) -> Pn {
    Pn {
        min: (min, min),
        ..pn(kind)
    }
}

fn pinned(kind: &'static str) -> Pn {
    Pn {
        pin: Pin::System,
        ..pn(kind)
    }
}

fn fixed(kind: &'static str) -> Pn {
    Pn {
        closable: false,
        ..pn(kind)
    }
}

/// The test app's panel factory: knows `a`..`e` and `grid`, refuses the rest.
fn decode(_home: PanelHome, type_id: &str) -> Option<Pn> {
    let kind = match type_id {
        "a" => "a",
        "b" => "b",
        "c" => "c",
        "d" => "d",
        "e" => "e",
        _ => return None,
    };
    Some(pn(kind))
}

type E = LayoutEngine<Pn>;

fn vp() -> Rect {
    Rect::new(0.0, 0.0, 1000.0, 600.0)
}

/// Caps with an OS resize border (no uzor bezel).
fn os_caps() -> HostCaps {
    HostCaps {
        multi_window: true,
        os_resize_bezel: true,
        clipboard_async: false,
    }
}

fn engine_with(policy: LayoutPolicy) -> E {
    let mut e = E::with_policy(decode, policy);
    e.apply(LayoutOp::Open {
        win: W,
        caps: os_caps(),
        viewport: vp(),
    });
    e
}

fn engine() -> E {
    engine_with(LayoutPolicy::default())
}

fn cmd(e: &mut E, c: LayoutCmd<Pn>) -> LayoutEffects {
    e.apply(LayoutOp::Cmd(c))
}

fn ptr(e: &mut E, event: DockPointer) -> LayoutEffects {
    e.apply(LayoutOp::Pointer {
        win: W,
        now: Seconds(1.0),
        event,
    })
}

fn down(e: &mut E, x: f64, y: f64) -> LayoutEffects {
    ptr(
        e,
        DockPointer::Down {
            pos: Point::new(x, y),
            button: MouseButton::Left,
        },
    )
}

fn mv(e: &mut E, x: f64, y: f64) -> LayoutEffects {
    ptr(e, DockPointer::Move(Point::new(x, y)))
}

fn up(e: &mut E, x: f64, y: f64) -> LayoutEffects {
    ptr(e, DockPointer::Up(Point::new(x, y)))
}

fn win(e: &E) -> &WindowLayout<Pn> {
    e.windows.get(&W).expect("window W")
}

fn win_mut(e: &mut E) -> &mut WindowLayout<Pn> {
    e.windows.get_mut(&W).expect("window W")
}

fn intents(fx: &LayoutEffects) -> Vec<DockIntent> {
    fx.iter()
        .filter_map(|f| match f {
            LayoutEffect::Intent(i) => Some(i.clone()),
            _ => None,
        })
        .collect()
}

fn captures(fx: &LayoutEffects) -> Vec<bool> {
    fx.iter()
        .filter_map(|f| match f {
            LayoutEffect::Capture { hold, .. } => Some(*hold),
            _ => None,
        })
        .collect()
}

/// The leaf whose tabs are exactly `kinds`.
fn leaf_of(e: &E, kinds: &[&str]) -> LeafId {
    let view = e.view().window(W).expect("W").dock_view();
    view.leaves
        .iter()
        .find(|l| l.panels.iter().map(|p| p.type_id).collect::<Vec<_>>() == kinds)
        .map(|l| l.leaf)
        .expect("leaf with these tabs")
}

fn tabs(e: &E) -> Vec<Vec<&'static str>> {
    e.view()
        .window(W)
        .expect("W")
        .dock_view()
        .leaves
        .iter()
        .map(|l| l.panels.iter().map(|p| p.type_id).collect())
        .collect()
}

/// `a | b` side by side (each half of the 1000 px viewport).
fn two_columns(e: &mut E, a: Pn, b: Pn) -> (LeafId, LeafId) {
    cmd(
        e,
        LayoutCmd::OpenPanel {
            win: W,
            panel: a.clone(),
            at: DockTarget::Root,
        },
    );
    let la = leaf_of(e, &[a.kind]);
    cmd(
        e,
        LayoutCmd::Split {
            win: W,
            leaf: la,
            dir: SplitDir::Right,
            panel: b.clone(),
        },
    );
    (la, leaf_of(e, &[b.kind]))
}

fn sep_x(e: &E, sep: usize) -> f64 {
    win(e).dock.separators()[sep].position as f64
}

fn leaf_rect(e: &E, leaf: LeafId) -> Rect {
    rects::from_panel(win(e).dock.panel_rects()[&leaf])
}

// ---------------------------------------------------------------------------
// Splitters
// ---------------------------------------------------------------------------

/// Press the vertical splitter of `a | b`, drag it to each x, release.
fn drag_splitter(e: &mut E, xs: &[f64]) -> Vec<LayoutEffects> {
    let x0 = sep_x(e, 0);
    assert_eq!(
        e.hit(W, Point::new(x0, 300.0)),
        LayoutHit::Splitter {
            sep: 0,
            orientation: SeparatorOrientation::Vertical
        }
    );
    let mut out = vec![down(e, x0, 300.0)];
    for &x in xs {
        out.push(mv(e, x, 300.0));
    }
    out.push(up(e, *xs.last().unwrap_or(&x0), 300.0));
    out
}

fn assert_widths_fill(e: &E, a: LeafId, b: LeafId) {
    let (ra, rb) = (leaf_rect(e, a), leaf_rect(e, b));
    assert!((ra.width + rb.width - 1000.0).abs() < 0.01, "{ra:?} {rb:?}");
}

#[test]
fn splitter_cascade_is_the_default_and_follows_the_pointer() {
    let mut e = engine();
    let (a, b) = two_columns(&mut e, pn_min("a", 100.0), pn_min("b", 100.0));
    assert_eq!(win(&e).dock.splitter_policy(), SplitterPolicy::Cascade);
    assert_eq!(sep_x(&e, 0), 500.0);

    let fx = drag_splitter(&mut e, &[600.0]);
    assert_eq!(captures(&fx[0]), vec![true], "press captures the pointer");
    assert!(win(&e).session.is_none());
    assert_eq!(captures(&fx[2]), vec![false], "release frees it");
    assert!((sep_x(&e, 0) - 600.0).abs() < 0.01);

    // Past the neighbour's minimum: Cascade stops at it, never rejects.
    drag_splitter(&mut e, &[950.0]);
    assert!((sep_x(&e, 0) - 900.0).abs() < 0.01, "{}", sep_x(&e, 0));
    assert_widths_fill(&e, a, b);
    assert!(win(&e).dock.snap_animations().is_empty());
}

#[test]
fn splitter_keeps_the_grab_offset() {
    let mut e = engine();
    two_columns(&mut e, pn("a"), pn("b"));
    // Grab 3 px right of the line: the line ends 3 px left of the pointer.
    down(&mut e, 503.0, 300.0);
    mv(&mut e, 703.0, 300.0);
    assert!((sep_x(&e, 0) - 700.0).abs() < 0.01);
    up(&mut e, 703.0, 300.0);
}

#[test]
fn splitter_clamp_stops_at_min_frac() {
    let mut e = engine_with(LayoutPolicy {
        splitter: SplitterPolicy::Clamp { min_frac: 0.2 },
        ..LayoutPolicy::default()
    });
    let (a, b) = two_columns(&mut e, pn_min("a", 100.0), pn_min("b", 100.0));
    drag_splitter(&mut e, &[950.0]);
    assert!((sep_x(&e, 0) - 800.0).abs() < 0.01, "{}", sep_x(&e, 0));
    drag_splitter(&mut e, &[10.0]);
    assert!((sep_x(&e, 0) - 200.0).abs() < 0.01, "{}", sep_x(&e, 0));
    assert_widths_fill(&e, a, b);
    assert!(
        win(&e).dock.snap_animations().is_empty(),
        "Clamp never snaps back"
    );
}

#[test]
fn splitter_reject_snap_back_freezes_the_drag_and_tick_settles_it() {
    let mut e = engine_with(LayoutPolicy {
        splitter: SplitterPolicy::RejectSnapBack,
        ..LayoutPolicy::default()
    });
    let (a, b) = two_columns(&mut e, pn_min("a", 100.0), pn_min("b", 100.0));
    down(&mut e, 500.0, 300.0);
    mv(&mut e, 700.0, 300.0);
    assert!((sep_x(&e, 0) - 700.0).abs() < 0.01, "a legal move moves");

    // b would be 50 px < 100: refused whole, one snap-back queued.
    mv(&mut e, 950.0, 300.0);
    assert!((sep_x(&e, 0) - 700.0).abs() < 0.01);
    assert_eq!(win(&e).dock.snap_animations().len(), 1);
    assert_eq!(
        e.view().window(W).expect("W").session(),
        Some(SessionKind::Splitter {
            sep: 0,
            frozen: true
        })
    );
    // Frozen until release: later legal moves are ignored.
    mv(&mut e, 750.0, 300.0);
    assert!((sep_x(&e, 0) - 700.0).abs() < 0.01);
    assert_eq!(
        win(&e).dock.snap_animations().len(),
        1,
        "one snap-back per drag"
    );
    up(&mut e, 750.0, 300.0);

    // `tick` advances the spring until it settles; every moving tick bumps
    // the revision and invalidates, the settled engine is quiet.
    let mut t = 1.0;
    let mut ticks = 0;
    while !win(&e).dock.snap_animations().is_empty() {
        t += 1.0 / 60.0;
        let rev = e.revision();
        let fx = e.tick(Seconds(t));
        assert!(e.revision() > rev);
        assert!(fx
            .iter()
            .any(|f| matches!(f, LayoutEffect::Invalidate { bits, .. } if *bits == InvalidateBits::GEOMETRY)));
        ticks += 1;
        assert!(ticks < 600, "snap-back never settled");
    }
    let rev = e.revision();
    e.tick(Seconds(t + 0.1));
    assert_eq!(e.revision(), rev);
    assert!(
        (sep_x(&e, 0) - 700.0).abs() < 0.01,
        "ratios stay at the last legal move"
    );
    assert_widths_fill(&e, a, b);
}

#[test]
fn corner_drag_moves_both_grid_lines() {
    let mut e = engine();
    for k in ["a", "b", "c", "d"] {
        win_mut(&mut e).dock.tree_mut().add_leaf(pn(k));
    }
    cmd(
        &mut e,
        LayoutCmd::SetGrid {
            win: W,
            branch: BranchId(0),
            rows: 2,
            cols: 2,
        },
    );
    let hit = e.hit(W, Point::new(500.0, 300.0));
    let LayoutHit::Corner {
        vertical,
        horizontal,
    } = hit
    else {
        panic!("expected a corner, got {hit:?}");
    };
    down(&mut e, 500.0, 300.0);
    mv(&mut e, 600.0, 200.0);
    up(&mut e, 600.0, 200.0);
    let seps = win(&e).dock.separators();
    assert!((seps[vertical].position - 600.0).abs() < 0.01);
    assert!((seps[horizontal].position - 200.0).abs() < 0.01);
    // A point on one line away from the crossing is that line, typed.
    assert!(matches!(
        e.hit(W, Point::new(600.0, 50.0)),
        LayoutHit::Splitter {
            orientation: SeparatorOrientation::Vertical,
            ..
        }
    ));
}

// ---------------------------------------------------------------------------
// Tabs
// ---------------------------------------------------------------------------

#[test]
fn tab_merge_center_joins_the_stack_without_a_ghost_leaf() {
    let mut e = engine();
    let (a, b) = two_columns(&mut e, pn("a"), pn("b"));
    cmd(
        &mut e,
        LayoutCmd::OpenPanel {
            win: W,
            panel: pn("c"),
            at: DockTarget::Leaf(a, DropZone::Center),
        },
    );
    assert_eq!(tabs(&e), vec![vec!["a", "c"], vec!["b"]]);
    assert_eq!(
        e.hit(W, Point::new(300.0, 12.0)),
        LayoutHit::Tab { leaf: a, tab: 1 }
    );

    down(&mut e, 300.0, 12.0);
    mv(&mut e, 300.0, 80.0); // leaves the bar: tear-off
    assert_eq!(
        e.view().window(W).expect("W").session(),
        Some(SessionKind::PanelDrag { leaf: a })
    );
    mv(&mut e, 750.0, 300.0); // centre of b's body
    let fx = up(&mut e, 750.0, 300.0);
    assert_eq!(
        intents(&fx),
        vec![DockIntent::PanelMoved {
            from: (W, a),
            to: (W, b),
            zone: DropZone::Center
        }]
    );
    assert_eq!(tabs(&e), vec![vec!["a"], vec!["b", "c"]], "no ghost leaf");
    let bl = e
        .view()
        .window(W)
        .expect("W")
        .dock_view()
        .leaves
        .into_iter()
        .find(|l| l.leaf == b)
        .expect("b");
    assert_eq!(bl.active_tab, 1, "the dropped tab is active");
    assert_eq!(captures(&fx), vec![false]);
}

#[test]
fn leaf_drag_center_merges_and_removes_the_source_leaf() {
    let mut e = engine();
    let (a, b) = two_columns(&mut e, pn("a"), pn("b"));
    assert_eq!(e.hit(W, Point::new(700.0, 12.0)), LayoutHit::PanelHeader(b));
    down(&mut e, 700.0, 12.0);
    mv(&mut e, 700.0, 60.0);
    mv(&mut e, 250.0, 300.0);
    let fx = up(&mut e, 250.0, 300.0);
    assert_eq!(
        intents(&fx),
        vec![DockIntent::PanelMoved {
            from: (W, b),
            to: (W, a),
            zone: DropZone::Center
        }]
    );
    assert_eq!(tabs(&e), vec![vec!["a", "b"]]);
}

#[test]
fn leaf_dropped_nowhere_floats_in_window() {
    let mut e = engine();
    let (_, b) = two_columns(&mut e, pn("a"), pn("b"));
    down(&mut e, 700.0, 12.0);
    mv(&mut e, 700.0, 60.0);
    mv(&mut e, 1500.0, 900.0); // outside every target
    let fx = up(&mut e, 1500.0, 900.0);
    assert_eq!(
        intents(&fx),
        vec![DockIntent::PanelTornOff {
            win: W,
            leaf: b,
            index: 0
        }]
    );
    let view = e.view().window(W).expect("W").dock_view();
    assert_eq!(view.leaves.len(), 1);
    assert_eq!(view.floating.len(), 1);
    let r = view.floating[0].rect;
    assert!(
        r.x >= 0.0 && r.x + r.width <= 1000.0 && r.y + r.height <= 600.0,
        "{r:?}"
    );
}

#[test]
fn tab_activate_and_close_intents() {
    let mut e = engine();
    cmd(
        &mut e,
        LayoutCmd::OpenPanel {
            win: W,
            panel: pn("a"),
            at: DockTarget::Root,
        },
    );
    let a = leaf_of(&e, &["a"]);
    for k in [pn("b"), fixed("c")] {
        cmd(
            &mut e,
            LayoutCmd::OpenPanel {
                win: W,
                panel: k,
                at: DockTarget::Leaf(a, DropZone::Center),
            },
        );
    }
    // Three chips of 333.3 px; `c` (added last) is active.
    let fx = down(&mut e, 100.0, 12.0);
    assert_eq!(
        intents(&fx),
        vec![DockIntent::TabActivated {
            win: W,
            leaf: a,
            index: 0
        }]
    );
    up(&mut e, 100.0, 12.0);
    let fx = down(&mut e, 100.0, 12.0);
    assert!(intents(&fx).is_empty(), "already active: no intent");
    up(&mut e, 100.0, 12.0);

    // Close button of chip 1: acts on release over the same button.
    let close = win(&e).dock.tab_bars()[0].tabs[1].close_rect;
    let (cx, cy) = ((close.x + 7.0) as f64, (close.y + 7.0) as f64);
    assert_eq!(
        e.hit(W, Point::new(cx, cy)),
        LayoutHit::TabClose { leaf: a, tab: 1 }
    );
    down(&mut e, cx, cy);
    let fx = up(&mut e, cx + 200.0, cy); // released elsewhere: nothing
    assert!(intents(&fx).is_empty());
    assert_eq!(tabs(&e), vec![vec!["a", "b", "c"]]);
    down(&mut e, cx, cy);
    let fx = up(&mut e, cx, cy);
    assert_eq!(
        intents(&fx),
        vec![DockIntent::PanelClosed {
            win: W,
            leaf: a,
            index: 1,
            type_id: "b"
        }]
    );
    assert_eq!(tabs(&e), vec![vec!["a", "c"]]);

    // A panel that is not closable has no close zone: its chip is hit.
    let close = win(&e).dock.tab_bars()[0].tabs[1].close_rect;
    let (cx, cy) = ((close.x + 7.0) as f64, (close.y + 7.0) as f64);
    assert_eq!(
        e.hit(W, Point::new(cx, cy)),
        LayoutHit::Tab { leaf: a, tab: 1 }
    );
}

#[test]
fn tab_reorder_inside_the_bar() {
    let mut e = engine();
    cmd(
        &mut e,
        LayoutCmd::OpenPanel {
            win: W,
            panel: pn("a"),
            at: DockTarget::Root,
        },
    );
    let a = leaf_of(&e, &["a"]);
    cmd(
        &mut e,
        LayoutCmd::OpenPanel {
            win: W,
            panel: pn("b"),
            at: DockTarget::Leaf(a, DropZone::Center),
        },
    );
    down(&mut e, 100.0, 12.0); // chip 0 of two 500 px chips
    mv(&mut e, 900.0, 12.0);
    assert_eq!(
        e.view().window(W).expect("W").session(),
        Some(SessionKind::TabReorder { leaf: a })
    );
    let fx = up(&mut e, 900.0, 12.0);
    assert!(intents(&fx).is_empty());
    assert_eq!(tabs(&e), vec![vec!["b", "a"]]);
}

#[test]
fn tab_new_button_requests_a_panel() {
    let mut e = engine_with(LayoutPolicy {
        tab_new_button: true,
        ..LayoutPolicy::default()
    });
    let (a, b) = two_columns(&mut e, pn("a"), pn("b"));
    cmd(
        &mut e,
        LayoutCmd::OpenPanel {
            win: W,
            panel: pn("c"),
            at: DockTarget::Leaf(a, DropZone::Center),
        },
    );
    // Multi-tab bar: chips leave room for the "+".
    assert_eq!(
        e.hit(W, Point::new(490.0, 12.0)),
        LayoutHit::TabNew { leaf: a }
    );
    assert_eq!(
        e.hit(W, Point::new(470.0, 12.0)),
        LayoutHit::TabClose { leaf: a, tab: 1 }
    );
    // Single-tab header: "+" at its right end.
    assert_eq!(
        e.hit(W, Point::new(990.0, 12.0)),
        LayoutHit::TabNew { leaf: b }
    );
    down(&mut e, 990.0, 12.0);
    let fx = up(&mut e, 990.0, 12.0);
    assert_eq!(
        intents(&fx),
        vec![DockIntent::NewPanelRequested { win: W, leaf: b }]
    );
}

// ---------------------------------------------------------------------------
// Pin
// ---------------------------------------------------------------------------

#[test]
fn pinned_panel_never_starts_a_tear_off() {
    let mut e = engine();
    let (a, b) = two_columns(&mut e, pn("a"), pinned("p"));
    // Header drag of the pinned leaf: refused, inert until release (the
    // press itself only activates the leaf).
    down(&mut e, 700.0, 12.0);
    let before = blob::WireDock::capture(&win(&e).dock);
    mv(&mut e, 700.0, 60.0);
    assert_eq!(
        e.view().window(W).expect("W").session(),
        Some(SessionKind::Refused)
    );
    assert!(win(&e).dock.panel_drag_state().is_none());
    mv(&mut e, 250.0, 300.0);
    let fx = up(&mut e, 250.0, 300.0);
    assert!(intents(&fx).is_empty());
    assert_eq!(
        blob::WireDock::capture(&win(&e).dock),
        before,
        "tree unchanged"
    );

    // A pinned tab in a stack cannot be torn off by its chip either.
    cmd(
        &mut e,
        LayoutCmd::OpenPanel {
            win: W,
            panel: pinned("q"),
            at: DockTarget::Leaf(a, DropZone::Center),
        },
    );
    assert_eq!(
        e.hit(W, Point::new(300.0, 12.0)),
        LayoutHit::Tab { leaf: a, tab: 1 }
    );
    down(&mut e, 300.0, 12.0);
    mv(&mut e, 300.0, 80.0);
    assert_eq!(
        e.view().window(W).expect("W").session(),
        Some(SessionKind::Refused)
    );
    mv(&mut e, 750.0, 300.0);
    let fx = up(&mut e, 750.0, 300.0);
    assert!(intents(&fx).is_empty());
    assert_eq!(tabs(&e), vec![vec!["a", "q"], vec!["p"]]);

    // The same gesture on a free panel does tear off (control).
    let _ = b;
    down(&mut e, 100.0, 12.0);
    mv(&mut e, 100.0, 80.0);
    mv(&mut e, 750.0, 300.0);
    let fx = up(&mut e, 750.0, 300.0);
    assert!(matches!(intents(&fx)[..], [DockIntent::PanelMoved { .. }]));
}

// ---------------------------------------------------------------------------
// Coalescing and revisions
// ---------------------------------------------------------------------------

#[test]
fn layout_changed_is_coalesced_once_per_window_per_tick() {
    let mut e = engine();
    e.apply(LayoutOp::Open {
        win: W2,
        caps: os_caps(),
        viewport: vp(),
    });
    let (a, _) = two_columns(&mut e, pn("a"), pn("b"));
    let branch = win(&e)
        .dock
        .tree()
        .find_parent_of_leaf(a)
        .map(|b| b.id)
        .expect("parent");
    cmd(
        &mut e,
        LayoutCmd::SetRatios {
            win: W,
            branch,
            ratios: vec![0.3, 0.7],
        },
    );
    cmd(
        &mut e,
        LayoutCmd::OpenPanel {
            win: W2,
            panel: pn("c"),
            at: DockTarget::Root,
        },
    );
    // Intents never come from the ops themselves.
    let fx = e.tick(Seconds(2.0));
    let got = intents(&fx);
    let w1_rev = e.view().window(W).expect("W").dock_rev();
    let w2_rev = e.view().window(W2).expect("W2").dock_rev();
    assert_eq!(
        got,
        vec![
            DockIntent::LayoutChanged {
                win: W,
                dock_rev: w1_rev
            },
            DockIntent::LayoutChanged {
                win: W2,
                dock_rev: w2_rev
            },
        ]
    );
    assert_eq!(w1_rev, Revision(3), "open, split, ratios");
    assert!(intents(&e.tick(Seconds(3.0))).is_empty(), "nothing new");

    // A drag of many moves is one change per tick.
    let x = sep_x(&e, 0);
    down(&mut e, x, 300.0);
    for i in 1..20 {
        mv(&mut e, x + i as f64 * 5.0, 300.0);
    }
    up(&mut e, x + 95.0, 300.0);
    assert_eq!(intents(&e.tick(Seconds(4.0))).len(), 1);
}

#[test]
fn revision_bumps_exactly_on_change() {
    let mut e = engine();
    let (a, b) = two_columns(&mut e, pn("a"), pn("b"));
    let r0 = e.revision();

    // No-ops.
    e.apply(LayoutOp::Solve {
        win: W,
        viewport: vp(),
    });
    cmd(
        &mut e,
        LayoutCmd::ActivateTab {
            win: W,
            leaf: a,
            index: 0,
        },
    );
    cmd(
        &mut e,
        LayoutCmd::SetRatios {
            win: W,
            branch: BranchId(0),
            ratios: vec![f64::NAN],
        },
    );
    mv(&mut e, 10.0, 10.0);
    up(&mut e, 10.0, 10.0);
    ptr(&mut e, DockPointer::Cancel);
    let _ = e.hit(W, Point::new(700.0, 300.0));
    cmd(
        &mut e,
        LayoutCmd::RequestBlob {
            win: W,
            ticket: Ticket(1),
        },
    );
    cmd(&mut e, LayoutCmd::SetPolicy(LayoutPolicy::default()));
    cmd(
        &mut e,
        LayoutCmd::ClosePanel {
            win: W,
            leaf: LeafId(999),
            index: 0,
        },
    );
    e.tick(Seconds(5.0));
    assert_eq!(e.revision(), r0, "nothing changed, nothing bumped");

    // Each real change: exactly +1.
    e.apply(LayoutOp::Solve {
        win: W,
        viewport: Rect::new(0.0, 0.0, 900.0, 600.0),
    });
    assert_eq!(e.revision(), r0.next());
    let x = sep_x(&e, 0);
    down(&mut e, x, 300.0); // a session starts: observable
    assert_eq!(e.revision(), r0.next().next());
    let before = e.revision();
    mv(&mut e, 300.0, 300.0);
    assert_eq!(e.revision(), before.next());
    up(&mut e, 300.0, 300.0);
    assert_eq!(e.revision(), before.next().next());
    let before = e.revision();
    cmd(
        &mut e,
        LayoutCmd::SetChrome {
            win: W,
            visible: true,
            height: 32.0,
        },
    );
    assert_eq!(e.revision(), before.next());
    // A policy change that re-lays every window out is still one bump.
    let before = e.revision();
    cmd(
        &mut e,
        LayoutCmd::SetPolicy(LayoutPolicy {
            tab_new_button: true,
            ..LayoutPolicy::default()
        }),
    );
    assert_eq!(e.revision(), before.next());
    let _ = b;
}

// ---------------------------------------------------------------------------
// Layout blob
// ---------------------------------------------------------------------------

fn request_blob(e: &mut E, w: WindowId) -> LayoutBlob {
    let fx = cmd(
        e,
        LayoutCmd::RequestBlob {
            win: w,
            ticket: Ticket(7),
        },
    );
    match &intents(&fx)[..] {
        [DockIntent::LayoutBlob { win, ticket, blob }] => {
            assert_eq!((*win, *ticket), (w, Ticket(7)));
            blob.clone()
        }
        other => panic!("expected a blob, got {other:?}"),
    }
}

fn restore(e: &mut E, w: WindowId, blob: LayoutBlob) -> Vec<DockIntent> {
    intents(&cmd(e, LayoutCmd::Restore { win: w, blob }))
}

/// Blob of W restores into W (after scrambling it) and into a fresh W2 with
/// the same structure.
fn round_trip(e: &mut E) {
    let wire = blob::WireDock::capture(&win(e).dock);
    let view = e.view().window(W).expect("W").dock_view();
    let blob = request_blob(e, W);

    cmd(
        e,
        LayoutCmd::SetPreset {
            win: W,
            preset: Preset::Single,
        },
    );
    let first = view.leaves[0].leaf;
    cmd(
        e,
        LayoutCmd::ClosePanel {
            win: W,
            leaf: first,
            index: 0,
        },
    );
    assert_ne!(blob::WireDock::capture(&win(e).dock), wire);

    assert_eq!(
        restore(e, W, blob.clone()),
        vec![DockIntent::LayoutRestored { win: W }]
    );
    assert_eq!(
        blob::WireDock::capture(&win(e).dock),
        wire,
        "structure equal"
    );
    let back = e.view().window(W).expect("W").dock_view();
    assert_eq!(back.leaves, view.leaves, "same leaves, rects, tabs");
    assert_eq!(back.floating, view.floating);

    e.apply(LayoutOp::Open {
        win: W2,
        caps: os_caps(),
        viewport: vp(),
    });
    assert_eq!(
        restore(e, W2, blob),
        vec![DockIntent::LayoutRestored { win: W2 }]
    );
    let w2 = e.windows.get(&W2).expect("W2");
    assert_eq!(blob::WireDock::capture(&w2.dock), wire);
}

#[test]
fn blob_round_trip_preset_tree() {
    let mut e = engine();
    for k in ["a", "b", "c"] {
        win_mut(&mut e).dock.tree_mut().add_leaf(pn(k));
    }
    cmd(
        &mut e,
        LayoutCmd::SetPreset {
            win: W,
            preset: Preset::ThreeRows,
        },
    );
    cmd(
        &mut e,
        LayoutCmd::SetRatios {
            win: W,
            branch: BranchId(0),
            ratios: vec![0.5, 0.3, 0.2],
        },
    );
    let leaf = leaf_of(&e, &["b"]);
    cmd(
        &mut e,
        LayoutCmd::OpenPanel {
            win: W,
            panel: pn("d"),
            at: DockTarget::Leaf(leaf, DropZone::Center),
        },
    );
    cmd(
        &mut e,
        LayoutCmd::ActivateTab {
            win: W,
            leaf,
            index: 0,
        },
    );
    round_trip(&mut e);
    let root = win(&e).dock.tree().root();
    assert_eq!(root.layout, Preset::ThreeRows);
    assert_eq!(root.proportions, vec![0.5, 0.3, 0.2]);
    assert_eq!(win(&e).dock.tree().active_leaf_id(), Some(leaf));
}

#[test]
fn blob_round_trip_grid_tree() {
    let mut e = engine();
    for k in ["a", "b", "c", "d", "e", "a"] {
        win_mut(&mut e).dock.tree_mut().add_leaf(pn(k));
    }
    cmd(
        &mut e,
        LayoutCmd::SetGrid {
            win: W,
            branch: BranchId(0),
            rows: 2,
            cols: 3,
        },
    );
    // Move one grid line so the ratios are not the defaults.
    let line = win(&e)
        .dock
        .separators()
        .iter()
        .position(|s| s.grid_line.is_some() && s.orientation == SeparatorOrientation::Vertical)
        .expect("a vertical grid line");
    let x = sep_x(&e, line);
    down(&mut e, x, 150.0);
    mv(&mut e, x + 60.0, 150.0);
    up(&mut e, x + 60.0, 150.0);
    let grid = win(&e).dock.tree().root().grid.clone().expect("grid");
    round_trip(&mut e);
    assert_eq!(win(&e).dock.tree().root().grid.clone(), Some(grid));
}

#[test]
fn blob_round_trip_non_magnetic_branch() {
    let mut e = engine();
    for k in ["a", "b"] {
        win_mut(&mut e).dock.tree_mut().add_leaf(pn(k));
    }
    if let Some(root) = win_mut(&mut e).dock.tree_mut().find_branch_mut(BranchId(0)) {
        root.magnetic = false;
    }
    // Direct test-side mutation: let the engine observe and solve it.
    e.apply(LayoutOp::Solve {
        win: W,
        viewport: vp(),
    });
    round_trip(&mut e);
    assert!(
        !win(&e).dock.tree().root().magnetic,
        "magnetic false survives"
    );
}

#[test]
fn blob_round_trip_floating_window() {
    let mut e = engine();
    let (a, _) = two_columns(&mut e, pn("a"), pn("b"));
    cmd(
        &mut e,
        LayoutCmd::OpenPanel {
            win: W,
            panel: pn("c"),
            at: DockTarget::Leaf(a, DropZone::Center),
        },
    );
    cmd(
        &mut e,
        LayoutCmd::Float {
            win: W,
            leaf: a,
            index: 1,
            rect: Rect::new(100.0, 120.0, 320.0, 200.0),
        },
    );
    cmd(
        &mut e,
        LayoutCmd::OpenPanel {
            win: W,
            panel: pn("d"),
            at: DockTarget::Floating(Rect::new(400.0, 300.0, 200.0, 150.0)),
        },
    );
    let fl = e.view().window(W).expect("W").dock_view().floating;
    assert_eq!(fl.len(), 2);
    assert_eq!(fl[0].rect, Rect::new(100.0, 120.0, 320.0, 200.0));
    round_trip(&mut e);
    let fl2 = e.view().window(W).expect("W").dock_view().floating;
    assert_eq!(fl2, fl, "ids, rects, tabs");
}

#[test]
fn blob_version_mismatch_and_bad_bytes_fail_typed_and_change_nothing() {
    let mut e = engine();
    two_columns(&mut e, pn("a"), pn("b"));
    let good = request_blob(&mut e, W);
    e.tick(Seconds(8.0));
    let wire = blob::WireDock::capture(&win(&e).dock);
    let rev = e.revision();
    let dock_rev = e.view().window(W).expect("W").dock_rev();

    // Same body, version 2.
    let mut bytes = good.as_bytes().to_vec();
    assert_eq!(bytes[0], BLOB_VERSION as u8, "u16 varint 1 is one byte");
    bytes[0] = 2;
    assert_eq!(
        restore(&mut e, W, LayoutBlob::from_bytes(bytes)),
        vec![DockIntent::LayoutRestoreFailed {
            win: W,
            error: LayoutCodecError::Version {
                found: 2,
                supported: BLOB_VERSION
            }
        }]
    );

    let bad = |e: &mut E, bytes: Vec<u8>| {
        let got = restore(e, W, LayoutBlob::from_bytes(bytes));
        assert!(
            matches!(
                &got[..],
                [DockIntent::LayoutRestoreFailed {
                    error: LayoutCodecError::Malformed(_),
                    ..
                }]
            ),
            "{got:?}"
        );
    };
    bad(&mut e, Vec::new());
    bad(&mut e, vec![1, 0xff, 0xff, 0xff]);
    let mut truncated = good.as_bytes().to_vec();
    truncated.truncate(truncated.len() / 2);
    bad(&mut e, truncated);
    let mut trailing = good.as_bytes().to_vec();
    trailing.push(0);
    bad(&mut e, trailing);

    // A cycle (a branch listing itself) is refused before the library
    // could recurse into it.
    let cyclic = wire.with_root_cycle();
    let bytes = blob::encode(&cyclic, None).expect("encode").into_bytes();
    bad(&mut e, bytes);

    assert_eq!(blob::WireDock::capture(&win(&e).dock), wire);
    assert_eq!(e.revision(), rev, "a failed restore changes nothing");
    assert_eq!(e.view().window(W).expect("W").dock_rev(), dock_rev);
    assert!(intents(&e.tick(Seconds(9.0))).is_empty());
}

#[test]
fn blob_panel_the_app_cannot_decode_fails_with_its_home() {
    let mut e = engine();
    two_columns(&mut e, pn("a"), pn("zz")); // `decode` does not know `zz`
    let blob = request_blob(&mut e, W);
    let zz = leaf_of(&e, &["zz"]);
    let wire = blob::WireDock::capture(&win(&e).dock);
    assert_eq!(
        restore(&mut e, W, blob),
        vec![DockIntent::LayoutRestoreFailed {
            win: W,
            error: LayoutCodecError::Panel {
                home: PanelHome::Leaf(zz),
                type_id: "zz".into()
            }
        }]
    );
    assert_eq!(blob::WireDock::capture(&win(&e).dock), wire);

    // Same for a floating window's tab.
    let mut e = engine();
    two_columns(&mut e, pn("a"), pn("b"));
    cmd(
        &mut e,
        LayoutCmd::OpenPanel {
            win: W,
            panel: pn("zz"),
            at: DockTarget::Floating(Rect::new(10.0, 10.0, 200.0, 200.0)),
        },
    );
    let blob = request_blob(&mut e, W);
    let got = restore(&mut e, W, blob);
    assert!(
        matches!(
            &got[..],
            [DockIntent::LayoutRestoreFailed {
                error: LayoutCodecError::Panel {
                    home: PanelHome::Floating(_),
                    ..
                },
                ..
            }]
        ),
        "{got:?}"
    );
}

// ---------------------------------------------------------------------------
// Typed hit-test over a solved window
// ---------------------------------------------------------------------------

/// A borderless 1000 x 600 window: bezel, chrome, a left toolbar, `a+c | b`
/// docked, one floating window.
fn furnished() -> (E, LeafId, LeafId) {
    let mut e = E::new(decode);
    e.apply(LayoutOp::Open {
        win: W,
        caps: HostCaps {
            os_resize_bezel: false,
            ..os_caps()
        },
        viewport: vp(),
    });
    cmd(
        &mut e,
        LayoutCmd::SetChrome {
            win: W,
            visible: true,
            height: 32.0,
        },
    );
    cmd(
        &mut e,
        LayoutCmd::SetChromeModel {
            win: W,
            model: ChromeModel {
                kind: ChromeKind::Default,
                buttons: ChromeLayoutConfig::default(),
                tabs: vec![ChromeTab {
                    label: "Main".into(),
                    closable: false,
                }],
            },
        },
    );
    cmd(
        &mut e,
        LayoutCmd::AddEdgeSlot {
            win: W,
            slot: EdgeSlot {
                id: "tools".into(),
                side: EdgeSide::Left,
                thickness: 40.0,
                visible: true,
                order: 0,
                placement: EdgePlacement::Compress,
            },
        },
    );
    let (a, b) = two_columns(&mut e, pn("a"), pn("b"));
    cmd(
        &mut e,
        LayoutCmd::OpenPanel {
            win: W,
            panel: pn("c"),
            at: DockTarget::Leaf(a, DropZone::Center),
        },
    );
    cmd(
        &mut e,
        LayoutCmd::OpenPanel {
            win: W,
            panel: pn("d"),
            at: DockTarget::Floating(Rect::new(600.0, 300.0, 240.0, 160.0)),
        },
    );
    (e, a, b)
}

#[test]
fn hit_test_names_each_part_of_a_solved_window() {
    let (e, a, b) = furnished();
    let h = |x: f64, y: f64| e.hit(W, Point::new(x, y));
    let area = e.view().window(W).expect("W").dock_area();
    assert_eq!(area, Rect::new(40.0, 32.0, 960.0, 568.0));

    // Bezel (no OS border): edges and corners.
    assert_eq!(h(2.0, 300.0), LayoutHit::Bezel(ResizeDirection::West));
    assert_eq!(h(998.0, 300.0), LayoutHit::Bezel(ResizeDirection::East));
    assert_eq!(h(500.0, 598.0), LayoutHit::Bezel(ResizeDirection::South));
    assert_eq!(h(500.0, 1.0), LayoutHit::Bezel(ResizeDirection::North));
    assert_eq!(h(1.0, 1.0), LayoutHit::Bezel(ResizeDirection::NorthWest));
    assert_eq!(
        h(999.0, 599.0),
        LayoutHit::Bezel(ResizeDirection::SouthEast)
    );
    // Chrome: window controls, the tab, the drag zone.
    assert_eq!(h(980.0, 16.0), LayoutHit::Chrome(ChromeHit::CloseBtn));
    assert_eq!(h(940.0, 16.0), LayoutHit::Chrome(ChromeHit::MaxBtn));
    assert_eq!(h(20.0, 16.0), LayoutHit::Chrome(ChromeHit::Tab(0)));
    assert_eq!(h(500.0, 16.0), LayoutHit::Chrome(ChromeHit::Drag));
    // The chrome's own border zone is not a resize zone.
    assert_eq!(h(500.0, 30.0), LayoutHit::Chrome(ChromeHit::Drag));
    // The toolbar is content.
    assert_eq!(h(20.0, 300.0), LayoutHit::None);
    // Dock.
    let x = sep_x(&e, 0);
    assert_eq!(x, 520.0);
    assert_eq!(
        h(x + 2.0, 400.0),
        LayoutHit::Splitter {
            sep: 0,
            orientation: SeparatorOrientation::Vertical
        }
    );
    assert_eq!(h(100.0, 40.0), LayoutHit::Tab { leaf: a, tab: 0 });
    assert_eq!(h(300.0, 40.0), LayoutHit::Tab { leaf: a, tab: 1 });
    let close = win(&e).dock.tab_bars()[0].tabs[1].close_rect;
    assert_eq!(
        h(close.x as f64 + 5.0, close.y as f64 + 5.0),
        LayoutHit::TabClose { leaf: a, tab: 1 }
    );
    assert_eq!(h(800.0, 40.0), LayoutHit::PanelHeader(b));
    assert_eq!(h(300.0, 300.0), LayoutHit::PanelBody(a));
    assert_eq!(h(900.0, 200.0), LayoutHit::PanelBody(b));
    // Floating window over b.
    let fid = e.view().window(W).expect("W").dock_view().floating[0].id;
    assert_eq!(h(700.0, 310.0), LayoutHit::FloatingHeader(fid));
    assert_eq!(h(700.0, 400.0), LayoutHit::FloatingBody(fid));
    assert_eq!(h(826.0, 310.0), LayoutHit::FloatingClose(fid));
    assert_eq!(
        h(839.0, 400.0),
        LayoutHit::FloatingResize {
            id: fid,
            dir: ResizeDirection::East
        }
    );
    // Outside.
    assert_eq!(h(-5.0, 10.0), LayoutHit::None);
    assert_eq!(h(1000.0, 10.0), LayoutHit::None);
    // Unknown window.
    assert_eq!(e.hit(W2, Point::new(10.0, 10.0)), LayoutHit::None);
}

#[test]
fn hit_test_gives_exactly_one_consistent_hit_per_point() {
    let (e, _, _) = furnished();
    let w = win(&e);
    let dock = &w.dock;
    let policy = LayoutPolicy::default();
    let mut kinds = std::collections::HashSet::new();
    let mut y = -3.0;
    while y < 605.0 {
        let mut x = -3.0;
        while x < 1005.0 {
            let p = Point::new(x, y);
            let hit = e.hit(W, p);
            // A pure function of the state: same answer twice, same answer
            // through the view.
            assert_eq!(hit, e.hit(W, p));
            assert_eq!(hit, e.view().hit(W, p));
            let (fx, fy) = (x as f32, y as f32);
            // The hit's own zone contains the point.
            let ok = match hit {
                LayoutHit::None => {
                    !rects::contains(vp(), p)
                        || rects::in_edge_slot(&w.solved, p)
                        || !rects::contains(w.solved.dock_area, p)
                }
                LayoutHit::Bezel(_) => chrome::bezel(vp(), policy.bezel_px, p).is_some(),
                LayoutHit::Chrome(_) => w.solved.chrome.is_some_and(|c| rects::contains(c, p)),
                LayoutHit::Splitter { sep, .. } => dock.separators()[sep].hit_test(fx, fy),
                LayoutHit::Corner {
                    vertical,
                    horizontal,
                } => {
                    dock.separators()[vertical].hit_test(fx, fy)
                        || dock.separators()[horizontal].hit_test(fx, fy)
                        || dock.corners().iter().any(|c| c.hit_test(fx, fy, 6.0))
                }
                LayoutHit::Tab { leaf, tab } => dock
                    .tab_bars()
                    .iter()
                    .any(|b| b.container_id == leaf && rects::contains_panel(&b.tabs[tab].rect, p)),
                LayoutHit::TabClose { leaf, tab } => dock.tab_bars().iter().any(|b| {
                    b.container_id == leaf && rects::contains_panel(&b.tabs[tab].close_rect, p)
                }),
                LayoutHit::TabNew { .. } | LayoutHit::EdgeGutter(_) => false,
                LayoutHit::PanelHeader(leaf) => {
                    rects::contains_panel(&dock.panel_headers()[&leaf], p)
                }
                LayoutHit::PanelBody(leaf) => rects::contains_panel(&dock.panel_rects()[&leaf], p),
                LayoutHit::FloatingHeader(id)
                | LayoutHit::FloatingBody(id)
                | LayoutHit::FloatingClose(id)
                | LayoutHit::FloatingResize { id, .. } => dock
                    .floating_windows()
                    .iter()
                    .any(|f| f.id == id && rects::contains(rects::from_panel(f.rect()), p)),
            };
            assert!(ok, "{hit:?} at {p:?}");
            // Precedence: the bezel beats everything, the chrome beats the
            // dock, a floating window beats what it covers.
            if chrome::bezel(vp(), policy.bezel_px, p).is_some() && rects::contains(vp(), p) {
                assert!(matches!(hit, LayoutHit::Bezel(_)), "{hit:?} at {p:?}");
            }
            kinds.insert(std::mem::discriminant(&hit));
            x += 7.0;
        }
        y += 7.0;
    }
    // The grid met bezel, chrome, none, splitter, tab, header, body and
    // floating parts.
    assert!(kinds.len() >= 9, "{}", kinds.len());
}

// ---------------------------------------------------------------------------
// Chrome / bezel presses
// ---------------------------------------------------------------------------

#[test]
fn chrome_and_bezel_presses_become_window_commands_and_actions() {
    let (mut e, _, _) = furnished();
    // Drag zone: the OS takes the gesture at once, no capture.
    let fx = down(&mut e, 500.0, 16.0);
    assert!(fx.iter().any(|f| matches!(
        f,
        LayoutEffect::Window {
            cmd: WindowCommand::DragWindow,
            ..
        }
    )));
    assert!(captures(&fx).is_empty());
    // Bezel.
    let fx = down(&mut e, 2.0, 300.0);
    assert!(fx.iter().any(|f| matches!(
        f,
        LayoutEffect::Window {
            cmd: WindowCommand::DragResizeWindow(ResizeDirection::West),
            ..
        }
    )));
    // A button acts on release over itself.
    let fx = down(&mut e, 980.0, 16.0);
    assert_eq!(captures(&fx), vec![true]);
    let fx = up(&mut e, 980.0, 16.0);
    assert!(fx.iter().any(|f| matches!(
        f,
        LayoutEffect::Chrome {
            action: ChromeAction::CloseApp,
            ..
        }
    )));
    down(&mut e, 980.0, 16.0);
    let fx = up(&mut e, 500.0, 16.0);
    assert!(!fx.iter().any(|f| matches!(f, LayoutEffect::Chrome { .. })));
    assert_eq!(captures(&fx), vec![false]);
}

#[test]
fn floating_window_moves_and_resizes_inside_the_dock_area() {
    let (mut e, _, _) = furnished();
    let fid = e.view().window(W).expect("W").dock_view().floating[0].id;
    down(&mut e, 700.0, 310.0);
    mv(&mut e, 400.0, 110.0);
    up(&mut e, 400.0, 110.0);
    let r = e.view().window(W).expect("W").dock_view().floating[0].rect;
    assert_eq!(r, Rect::new(300.0, 100.0, 240.0, 160.0));
    // Past the dock area: clamped.
    down(&mut e, 400.0, 110.0);
    mv(&mut e, -500.0, -500.0);
    up(&mut e, -500.0, -500.0);
    let r = e.view().window(W).expect("W").dock_view().floating[0].rect;
    assert_eq!((r.x, r.y), (40.0, 32.0));
    // Resize from the east border, never below the minimum width.
    assert_eq!(
        e.hit(W, Point::new(279.0, 100.0)),
        LayoutHit::FloatingResize {
            id: fid,
            dir: ResizeDirection::East
        }
    );
    down(&mut e, 279.0, 100.0);
    mv(&mut e, 339.0, 100.0);
    let r = e.view().window(W).expect("W").dock_view().floating[0].rect;
    assert_eq!(r.width, 300.0);
    mv(&mut e, -400.0, 100.0);
    up(&mut e, -400.0, 100.0);
    let r = e.view().window(W).expect("W").dock_view().floating[0].rect;
    assert_eq!(r.width, 120.0);
    // Close button.
    let close = (r.x + r.width - 14.0, r.y + 10.0);
    assert_eq!(
        e.hit(W, Point::new(close.0, close.1)),
        LayoutHit::FloatingClose(fid)
    );
    down(&mut e, close.0, close.1);
    up(&mut e, close.0, close.1);
    assert!(e
        .view()
        .window(W)
        .expect("W")
        .dock_view()
        .floating
        .is_empty());
}

#[test]
fn cancel_undoes_a_reorder_and_a_drag() {
    let mut e = engine();
    let (a, _) = two_columns(&mut e, pn("a"), pn("b"));
    cmd(
        &mut e,
        LayoutCmd::OpenPanel {
            win: W,
            panel: pn("c"),
            at: DockTarget::Leaf(a, DropZone::Center),
        },
    );
    down(&mut e, 50.0, 12.0); // activates chip 0 (not undone by cancel)
    let wire = blob::WireDock::capture(&win(&e).dock);
    mv(&mut e, 400.0, 12.0);
    let fx = ptr(&mut e, DockPointer::Cancel);
    assert_eq!(captures(&fx), vec![false]);
    assert_eq!(blob::WireDock::capture(&win(&e).dock), wire);
    down(&mut e, 700.0, 12.0); // activates leaf b
    let wire = blob::WireDock::capture(&win(&e).dock);
    mv(&mut e, 700.0, 60.0);
    mv(&mut e, 250.0, 300.0);
    ptr(&mut e, DockPointer::Cancel);
    assert_eq!(blob::WireDock::capture(&win(&e).dock), wire);
    assert!(win(&e).dock.panel_drag_state().is_none());
    assert!(win(&e).session.is_none());
}

#[test]
fn claims_press_and_cursor() {
    assert!(LayoutHit::PanelHeader(LeafId(1)).claims_press());
    assert!(!LayoutHit::PanelBody(LeafId(1)).claims_press());
    assert!(!LayoutHit::None.claims_press());
    assert_eq!(
        LayoutHit::Splitter {
            sep: 0,
            orientation: SeparatorOrientation::Vertical
        }
        .cursor(),
        Some(uzor::CursorIcon::ResizeColumn)
    );
    assert_eq!(LayoutHit::FloatingBody(FloatingWindowId(1)).cursor(), None);
}

#[test]
fn closed_and_unknown_windows_are_ignored() {
    let mut e = engine();
    let fx = cmd(
        &mut e,
        LayoutCmd::OpenPanel {
            win: W2,
            panel: pn("a"),
            at: DockTarget::Root,
        },
    );
    assert!(fx.is_empty());
    let rev = e.revision();
    e.apply(LayoutOp::Close(W));
    assert_eq!(e.revision(), rev.next());
    assert!(e.view().window(W).is_none());
    assert!(e.tick(Seconds(1.0)).is_empty());
}

mod expand_dragout;
