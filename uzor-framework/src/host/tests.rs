//! The §9.2 acceptance list, headless: every test drives the public host
//! surface only (`HeadlessHost`, `Handle`, commands and input events), so
//! the same scripts run unchanged against the native and web hosts later.

use std::sync::Arc;

use uzor::input::{KeyCode, ModifierKeys, MouseButton, Sense, TextFieldConfig};
use uzor::layout::OverlayKind;
use uzor::layout::docking::DockPanel;
use uzor::render::{InvalidateBits, TickRate};
use uzor::{Rect, WidgetId};

use crate::handle::{App, IntentCx, OverlayCx, PanelCx};
use crate::runtime::{MAX_SETTLE, RuntimeConfig};
use crate::types::bus::{ImeInput, InputEvent, KeyInput, PointerInput};
use crate::types::command::{
    AppCommand, Binding, CadenceCmd, DockTarget, DragOutChrome, DragOutPolicy, FocusCmd, KeymapCmd,
    KeymapScope, LayoutCmd, LayoutPolicy, OverlayCmd, OverlayPolicy, OverlaySize, SplitDir,
};
use crate::types::frame::Wake;
use crate::types::ids::{Seconds, Ticket, WindowId};
use crate::types::intent::{CloseCause, DockIntent, Intent, OverlayIntent, TextIntent};
use crate::types::layout_blob::LayoutBlob;
use crate::types::overlay_model::OverlayModel;
use crate::types::spec::{PanelHome, Spec};
use crate::types::window::{Point, SizePx, WindowCommand};
use crate::Handle;

use super::HeadlessHost as Host;

// -- test app ---------------------------------------------------------------

#[derive(Clone, Debug)]
struct P(&'static str);

impl DockPanel for P {
    fn title(&self) -> &str {
        self.0
    }

    fn type_id(&self) -> &'static str {
        "p"
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Ov {
    Modal,
    Second,
    Menu,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Act {
    Save,
    Quit,
}

struct TS;

impl Spec for TS {
    type Panel = P;
    type Overlay = Ov;
    type Action = Act;

    fn decode_panel(_home: PanelHome, _type_id: &str) -> Option<P> {
        Some(P("restored"))
    }
}

/// One text field the app reports per frame (initial text set once).
struct Field {
    id: WidgetId,
    rect: Rect,
    initial: String,
    initialized: bool,
}

/// The recording test app: widgets are registered declaratively, intents
/// and observed clicks land in plain vectors.
#[derive(Default)]
struct TestApp {
    intents: Vec<Intent<TS>>,
    panel_widgets: Vec<(WidgetId, Rect, Sense)>,
    overlay_widgets: Vec<(WidgetId, Rect, Sense)>,
    field: Option<Field>,
    /// Clicks the panel hook observed through `VisualView`, with counts.
    seen_clicks: Vec<(WidgetId, u8)>,
    /// The field text the panel hook last observed in the store.
    seen_text: Option<String>,
    /// Answered to EVERY intent (the settle test's feedback loop).
    answer: Option<AppCommand<TS>>,
    request_frame: bool,
}

impl App for TestApp {
    type Spec = TS;

    fn overlay_model(&self, _win: WindowId, _id: Ov) -> OverlayModel {
        OverlayModel::default()
    }

    fn panel(&mut self, cx: &mut PanelCx<'_, TS>) {
        for (id, rect, sense) in &self.panel_widgets {
            cx.widgets
                .coord
                .register_on_layer(id.clone(), *rect, *sense, &cx.widgets.layer);
        }
        if let Some(field) = &mut self.field {
            cx.widgets.coord.register_text_field(
                field.id.clone(),
                field.rect,
                TextFieldConfig::text(),
            );
            if !field.initialized {
                cx.widgets
                    .coord
                    .text_fields_mut()
                    .set_text(&field.id, &field.initial);
                field.initialized = true;
            }
            let n = cx
                .widgets
                .coord
                .text_fields()
                .text(&field.id)
                .chars()
                .count();
            let r = field.rect;
            cx.widgets.coord.text_fields_mut().update_field(
                &field.id,
                (r.x, r.y, r.width, r.height),
                (0..=n).map(|i| r.x + i as f64 * 8.0).collect(),
            );
            self.seen_text = Some(cx.widgets.coord.text_fields().text(&field.id).to_string());
        }
        self.seen_clicks.extend(cx.view.clicks().iter().cloned());
        if self.request_frame {
            cx.out.request_frame();
        }
    }

    fn overlay_body(&mut self, cx: &mut OverlayCx<'_, TS>) {
        for (id, rect, sense) in &self.overlay_widgets {
            cx.widgets
                .coord
                .register_on_layer(id.clone(), *rect, *sense, &cx.widgets.layer);
        }
    }

    fn intent(&mut self, intent: Intent<TS>, cx: &mut IntentCx<'_, TS>) {
        self.intents.push(intent);
        if let Some(cmd) = &self.answer {
            cx.command(cmd.clone());
        }
    }
}

// -- helpers ----------------------------------------------------------------

fn wid(name: &str) -> WidgetId {
    WidgetId::from(name)
}

fn mods_ctrl() -> ModifierKeys {
    ModifierKeys {
        ctrl: true,
        ..ModifierKeys::default()
    }
}

fn host() -> (Host<TS, TestApp>, Handle<TS>) {
    Host::new(TestApp::default(), RuntimeConfig::default())
}

fn open_main(h: &mut Host<TS, TestApp>) -> WindowId {
    h.open_window(SizePx::new(800, 600), 1.0)
}

/// A panel with one leaf; returns the window.
fn open_with_panel(h: &mut Host<TS, TestApp>, handle: &Handle<TS>) -> WindowId {
    let win = open_main(h);
    handle
        .dispatch(AppCommand::Layout(LayoutCmd::OpenPanel {
            win,
            panel: P("main"),
            at: DockTarget::Root,
        }))
        .expect("dispatch");
    h.advance(0.017);
    h.advance(0.017);
    win
}

fn down(h: &mut Host<TS, TestApp>, win: WindowId, x: f64, y: f64) {
    h.input(
        win,
        InputEvent::Pointer(PointerInput::Down {
            pos: Point::new(x, y),
            button: MouseButton::Left,
            mods: ModifierKeys::default(),
        }),
    );
}

fn up(h: &mut Host<TS, TestApp>, win: WindowId, x: f64, y: f64) {
    h.input(
        win,
        InputEvent::Pointer(PointerInput::Up {
            pos: Point::new(x, y),
            button: MouseButton::Left,
            mods: ModifierKeys::default(),
        }),
    );
}

fn mov(h: &mut Host<TS, TestApp>, win: WindowId, x: f64, y: f64) {
    h.input(
        win,
        InputEvent::Pointer(PointerInput::Moved {
            pos: Point::new(x, y),
            mods: ModifierKeys::default(),
        }),
    );
}

/// A full click cycle: press, release, the frame that evaluates the
/// click, the frame whose view reports it to the hooks. Every step is
/// 0.017 s: past the 60 Hz cap, so each advance paints exactly one frame.
fn click(h: &mut Host<TS, TestApp>, win: WindowId, x: f64, y: f64) {
    down(h, win, x, y);
    h.advance(0.017);
    up(h, win, x, y);
    h.advance(0.017);
    h.advance(0.017);
}

fn key(h: &mut Host<TS, TestApp>, win: WindowId, code: KeyCode, mods: ModifierKeys, text: Option<&str>) {
    h.input(
        win,
        InputEvent::Key(KeyInput {
            code,
            text: text.map(str::to_string),
            state: crate::types::bus::KeyState::Down,
            mods,
        }),
    );
    h.advance(0.017);
    h.input(
        win,
        InputEvent::Key(KeyInput {
            code,
            text: None,
            state: crate::types::bus::KeyState::Up,
            mods,
        }),
    );
    h.advance(0.017);
    h.advance(0.017);
}

fn open_overlay(h: &mut Host<TS, TestApp>, handle: &Handle<TS>, win: WindowId, id: Ov, kind: OverlayKind, policy: OverlayPolicy) {
    handle
        .dispatch(AppCommand::Overlay(OverlayCmd::Open {
            win,
            id,
            kind,
            anchor: Some(Rect::new(100.0, 100.0, 10.0, 10.0)),
            size: OverlaySize::Fixed {
                width: 200.0,
                height: 120.0,
            },
            policy: Some(policy),
        }))
        .expect("dispatch");
    h.advance(0.017);
    h.advance(0.017);
}

fn policy(modal: bool, outside: bool, escape: bool, restore: bool) -> OverlayPolicy {
    OverlayPolicy {
        modal,
        dismiss_on_outside: outside,
        dismiss_on_escape: escape,
        restore_focus: restore,
        auto_close_after: None,
        keymap_scope: false,
    }
}

fn overlay_closed(app: &TestApp, id: Ov) -> Option<CloseCause> {
    app.intents.iter().rev().find_map(|i| match i {
        Intent::Overlay(OverlayIntent::Closed { id: got, cause, .. }) if *got == id => Some(*cause),
        _ => None,
    })
}

fn has_unhandled_pointer(app: &TestApp) -> bool {
    app.intents
        .iter()
        .any(|i| matches!(i, Intent::Unhandled(crate::types::intent::UnhandledInput::Pointer { .. })))
}

fn focused(h: &Host<TS, TestApp>, win: WindowId) -> Option<WidgetId> {
    h.snapshot().window(win).and_then(|w| w.input.focused.clone())
}

// -- §9.2: overlay routing ---------------------------------------------------

/// A click under a modal never reaches the content widget, and is not
/// reported unhandled either: the shield consumes it (design §4.3).
#[test]
fn modal_shield_blocks_content_clicks() {
    let (mut h, handle) = host();
    let win = open_with_panel(&mut h, &handle);
    h.runtime_mut().app_mut().panel_widgets.push((
        wid("btn"),
        Rect::new(10.0, 100.0, 60.0, 24.0),
        Sense::CLICK,
    ));
    open_overlay(&mut h, &handle, win, Ov::Modal, OverlayKind::Modal, policy(true, false, true, true));

    click(&mut h, win, 20.0, 112.0);

    let app = h.runtime().app();
    assert!(
        !app.seen_clicks.iter().any(|(id, _)| *id == wid("btn")),
        "the modal shield leaked a click to content: {:?}",
        app.seen_clicks
    );
    assert!(
        !has_unhandled_pointer(app),
        "a shielded click is consumed, not unhandled: {:?}",
        app.intents
    );
    assert!(h.snapshot().window(win).is_some_and(|w| w.overlays.len() == 1));
}

/// Pointer-up outside a dismissible overlay closes it with
/// `CloseCause::Outside`; the click is consumed (design §4.3).
#[test]
fn outside_click_dismisses_dropdown() {
    let (mut h, handle) = host();
    let win = open_with_panel(&mut h, &handle);
    h.runtime_mut().app_mut().panel_widgets.push((
        wid("btn"),
        Rect::new(500.0, 400.0, 60.0, 24.0),
        Sense::CLICK,
    ));
    open_overlay(&mut h, &handle, win, Ov::Menu, OverlayKind::Dropdown, policy(false, true, true, true));

    click(&mut h, win, 520.0, 410.0);

    assert_eq!(overlay_closed(h.runtime().app(), Ov::Menu), Some(CloseCause::Outside));
    assert!(h.snapshot().window(win).is_some_and(|w| w.overlays.is_empty()));
    assert!(
        !h.runtime().app().seen_clicks.iter().any(|(id, _)| *id == wid("btn")),
        "the dismissing click must be consumed, not delivered"
    );
}

/// Escape closes exactly the top overlay; the one below survives the
/// first press and closes on the second (design §4.3 key precedence).
#[test]
fn escape_closes_overlays_top_first() {
    let (mut h, handle) = host();
    let win = open_with_panel(&mut h, &handle);
    open_overlay(&mut h, &handle, win, Ov::Modal, OverlayKind::Modal, policy(true, true, true, true));
    open_overlay(&mut h, &handle, win, Ov::Second, OverlayKind::Modal, policy(true, true, true, true));
    assert!(h.snapshot().window(win).is_some_and(|w| w.overlays.len() == 2));

    key(&mut h, win, KeyCode::Escape, ModifierKeys::default(), None);
    assert_eq!(overlay_closed(h.runtime().app(), Ov::Second), Some(CloseCause::Escape));
    assert_eq!(overlay_closed(h.runtime().app(), Ov::Modal), None, "one Escape, one close");
    assert!(h.snapshot().window(win).is_some_and(|w| w.overlays.len() == 1));

    key(&mut h, win, KeyCode::Escape, ModifierKeys::default(), None);
    assert_eq!(overlay_closed(h.runtime().app(), Ov::Modal), Some(CloseCause::Escape));
    assert!(h.snapshot().window(win).is_some_and(|w| w.overlays.is_empty()));
}

/// Focus saved at scope push returns when the overlay closes with a
/// restoring cause (design §4.3, `restore_focus`).
#[test]
fn focus_restores_after_overlay_scope() {
    let (mut h, handle) = host();
    let win = open_with_panel(&mut h, &handle);
    h.runtime_mut().app_mut().panel_widgets.push((
        wid("fld"),
        Rect::new(10.0, 100.0, 60.0, 24.0),
        Sense::CLICK.union(Sense::FOCUSABLE),
    ));
    h.advance(0.017);
    click(&mut h, win, 20.0, 112.0);
    assert_eq!(focused(&h, win), Some(wid("fld")), "click focuses a FOCUSABLE widget");

    open_overlay(&mut h, &handle, win, Ov::Modal, OverlayKind::Modal, policy(true, true, true, true));
    key(&mut h, win, KeyCode::Escape, ModifierKeys::default(), None);

    assert_eq!(overlay_closed(h.runtime().app(), Ov::Modal), Some(CloseCause::Escape));
    assert_eq!(focused(&h, win), Some(wid("fld")), "the scope's saved focus must return");
}

// -- §9.2: keymap ------------------------------------------------------------

/// Precedence: focused widget > top overlay > global, and a modal shields
/// globals unless the binding opts out (`through_modal`).
#[test]
fn keymap_precedence_focused_global_modal() {
    let (mut h, handle) = host();
    let win = open_with_panel(&mut h, &handle);
    h.runtime_mut().app_mut().panel_widgets.push((
        wid("fld"),
        Rect::new(10.0, 100.0, 60.0, 24.0),
        Sense::CLICK.union(Sense::FOCUSABLE),
    ));
    let chord = uzor::input::KeyboardShortcut::new(mods_ctrl(), KeyCode::S);
    handle
        .dispatch(AppCommand::Keymap(KeymapCmd::Bind {
            scope: KeymapScope::Global,
            binding: Binding::new(chord.clone(), Act::Save),
        }))
        .expect("dispatch");
    handle
        .dispatch(AppCommand::Keymap(KeymapCmd::Bind {
            scope: KeymapScope::Focused(wid("fld")),
            binding: Binding::new(chord.clone(), Act::Quit),
        }))
        .expect("dispatch");
    h.advance(0.017);

    // Focused widget wins over global.
    click(&mut h, win, 20.0, 112.0);
    assert_eq!(focused(&h, win), Some(wid("fld")));
    key(&mut h, win, KeyCode::S, mods_ctrl(), None);
    assert!(
        h.runtime().app().intents.iter().any(|i| matches!(i, Intent::Action(Act::Quit))),
        "the focused binding must win: {:?}",
        h.runtime().app().intents
    );

    // Blur: the global resolves.
    handle
        .dispatch(AppCommand::Focus(FocusCmd::Clear { win }))
        .expect("dispatch");
    h.advance(0.017);
    key(&mut h, win, KeyCode::S, mods_ctrl(), None);
    assert!(
        h.runtime().app().intents.iter().any(|i| matches!(i, Intent::Action(Act::Save))),
        "the global binding must resolve once blurred"
    );

    // A modal shields the global (no `through_modal`).
    open_overlay(&mut h, &handle, win, Ov::Modal, OverlayKind::Modal, policy(true, true, true, true));
    let before = h.runtime().app().intents.len();
    key(&mut h, win, KeyCode::S, mods_ctrl(), None);
    assert!(
        !h.runtime().app().intents[before..]
            .iter()
            .any(|i| matches!(i, Intent::Action(_))),
        "an open modal shields global bindings: {:?}",
        &h.runtime().app().intents[before..]
    );
}

// -- §9.2: cook, capture, hops ------------------------------------------------

/// The cook counts a fast second press as a double click (design §4.3
/// routing step 1).
#[test]
fn click_cook_counts_double_click() {
    let (mut h, handle) = host();
    let win = open_with_panel(&mut h, &handle);
    h.runtime_mut().app_mut().panel_widgets.push((
        wid("btn"),
        Rect::new(10.0, 100.0, 60.0, 24.0),
        Sense::CLICK,
    ));
    h.advance(0.017);

    click(&mut h, win, 20.0, 112.0);
    click(&mut h, win, 20.0, 112.0);

    let app = h.runtime().app();
    assert!(
        app.seen_clicks.iter().any(|(id, n)| *id == wid("btn") && *n == 2),
        "expected a double click in {:?}",
        app.seen_clicks
    );
}

/// A drag capture holds the pointer for the widget even far outside its
/// rect, and releases on pointer-up (design §4.3 routing step 2).
#[test]
fn widget_capture_survives_leaving_the_rect() {
    let (mut h, handle) = host();
    let win = open_with_panel(&mut h, &handle);
    h.runtime_mut().app_mut().panel_widgets.push((
        wid("drag"),
        Rect::new(10.0, 100.0, 60.0, 24.0),
        Sense::CLICK_AND_DRAG,
    ));
    h.advance(0.017);

    down(&mut h, win, 20.0, 112.0);
    h.advance(0.017);
    mov(&mut h, win, 30.0, 112.0);
    h.advance(0.017);
    mov(&mut h, win, 700.0, 500.0);
    h.advance(0.017);
    assert!(
        h.snapshot().window(win).is_some_and(|w| w.input.captured),
        "the drag must hold the capture outside the rect"
    );

    up(&mut h, win, 700.0, 500.0);
    h.advance(0.017);
    assert!(
        h.snapshot().window(win).is_some_and(|w| !w.input.captured),
        "pointer-up ends the capture"
    );
}

/// §4.4: no routed event crossed more than three module boundaries
/// (push_input -> route -> engine), ever.
#[test]
fn route_hops_stay_within_three() {
    let (mut h, handle) = host();
    let win = open_with_panel(&mut h, &handle);
    h.runtime_mut().app_mut().panel_widgets.push((
        wid("btn"),
        Rect::new(10.0, 100.0, 60.0, 24.0),
        Sense::CLICK,
    ));
    open_overlay(&mut h, &handle, win, Ov::Modal, OverlayKind::Modal, policy(true, true, true, true));
    click(&mut h, win, 20.0, 112.0); // shielded by the modal
    key(&mut h, win, KeyCode::Escape, ModifierKeys::default(), None);
    click(&mut h, win, 20.0, 112.0); // reaches content
    key(&mut h, win, KeyCode::S, mods_ctrl(), None); // unhandled key

    let max = h.runtime().kernel().trace_max();
    assert!(
        max + 1 <= 3,
        "the longest route was {} hops (+1 for push_input): {:?}",
        max + 1,
        h.runtime().kernel().last_trace()
    );
}

// -- §9.2: publish --------------------------------------------------------------

/// §4.6: a quiet tick publishes nothing — observers keep the same Arc.
#[test]
fn publish_keeps_the_same_arc_when_nothing_changes() {
    let (mut h, handle) = host();
    let _win = open_with_panel(&mut h, &handle);
    let before = h.snapshot();
    h.advance(0.0);
    h.advance(0.0);
    let after = h.snapshot();
    assert!(
        Arc::ptr_eq(&before, &after),
        "a quiet tick must not republish"
    );
    assert_eq!(before.revision, after.revision);
}

/// §4.6: any number of engine changes inside one pass bump the published
/// revision exactly once.
#[test]
fn publish_bumps_revision_once_per_pass() {
    let (mut h, handle) = host();
    let win = open_main(&mut h);
    let r0 = h.snapshot().revision;
    for name in ["a", "b"] {
        handle
            .dispatch(AppCommand::Layout(LayoutCmd::OpenPanel {
                win,
                panel: P(name),
                at: DockTarget::Root,
            }))
            .expect("dispatch");
    }
    // No frame is due at the same instant, so the paint bookkeeping
    // (regions marked painted revision the cadence engine) stays out of
    // the measurement: only the change pass publishes.
    h.advance(0.0);
    let r1 = h.snapshot().revision;
    assert_eq!(
        r1.get(),
        r0.get() + 1,
        "two panels in one pass must be a single bump ({r0:?} -> {r1:?})"
    );
}

// -- §9.2: settle ---------------------------------------------------------------

/// §2.5: an app answering every intent with an already-due timer gets
/// exactly `MAX_SETTLE` timers per tick and the runtime stays live
/// (`Wake::Immediate`) instead of spinning forever.
#[test]
fn settle_loop_is_bounded() {
    let app = TestApp {
        answer: Some(AppCommand::Cadence(CadenceCmd::WakeAt {
            token: 1,
            at: Seconds::ZERO,
        })),
        ..TestApp::default()
    };
    let (mut h, _handle) = Host::new(app, RuntimeConfig::default());
    let win = open_main(&mut h);
    h.runtime_mut().app_mut().intents.clear();

    down(&mut h, win, 700.0, 500.0); // unhandled press -> answer -> storm
    let wake = h.advance(0.017).wake;
    let timers = h
        .runtime()
        .app()
        .intents
        .iter()
        .filter(|i| matches!(i, Intent::Timer(1)))
        .count();
    assert_eq!(timers, MAX_SETTLE, "one timer per settle pass, then stop");
    assert!(
        matches!(wake, Wake::Immediate),
        "the unanswered timer keeps the runtime hot: {wake:?}"
    );
}

// -- §9.2: cadence / wake ---------------------------------------------------------

/// §6.2: a capped window frames at its rate and schedules `Wake::At`;
/// `Dirty` sleeps until an explicit invalidation.
#[test]
fn cadence_cap_and_dirty() {
    let (mut h, handle) = host();
    let win = open_main(&mut h);
    handle
        .dispatch(AppCommand::Cadence(CadenceCmd::SetTickRate {
            win,
            rate: TickRate::Capped(10),
        }))
        .expect("dispatch");
    h.advance(0.0);

    assert!(
        !h.advance(0.05).frames.iter().any(|f| f.window == win),
        "0.05 s < the 0.1 s slot: no frame"
    );
    let (framed, wake) = {
        let out = h.advance(0.06);
        (out.frames.iter().any(|f| f.window == win), out.wake)
    };
    assert!(framed, "0.11 s crossed the slot: frame");
    assert!(
        matches!(wake, Wake::At(_)),
        "a capped window schedules its next slot: {wake:?}"
    );

    handle
        .dispatch(AppCommand::Cadence(CadenceCmd::SetTickRate {
            win,
            rate: TickRate::Dirty,
        }))
        .expect("dispatch");
    h.advance(0.0);
    assert!(
        h.advance(0.5).frames.is_empty(),
        "dirty and quiet: no frame, however long the step"
    );
    handle
        .dispatch(AppCommand::Cadence(CadenceCmd::Invalidate {
            win,
            region: None,
            bits: InvalidateBits::MATERIAL,
        }))
        .expect("dispatch");
    assert!(
        h.advance(0.0).frames.iter().any(|f| f.window == win),
        "an invalidation wakes a dirty window"
    );
}

// -- §9.2: determinism ------------------------------------------------------------

/// §9.2: two hosts driven by the identical script publish identical
/// snapshots, revision included.
#[test]
fn determinism_same_script_same_snapshot() {
    fn script() -> Host<TS, TestApp> {
        let (mut h, handle) = host();
        let win = open_with_panel(&mut h, &handle);
        h.runtime_mut().app_mut().panel_widgets.push((
            wid("btn"),
            Rect::new(10.0, 100.0, 60.0, 24.0),
            Sense::CLICK.union(Sense::FOCUSABLE),
        ));
        h.advance(0.017);
        click(&mut h, win, 20.0, 112.0);
        key(&mut h, win, KeyCode::S, mods_ctrl(), None);
        open_overlay(&mut h, &handle, win, Ov::Menu, OverlayKind::Dropdown, policy(false, true, true, true));
        click(&mut h, win, 400.0, 300.0); // dismisses the dropdown
        h.advance(0.017);
        h
    }

    let a = script();
    let b = script();
    assert_eq!(
        *a.snapshot(),
        *b.snapshot(),
        "identical scripts must publish identical snapshots"
    );
}

// -- §9.2: dock interactions --------------------------------------------------------

/// Split one leaf into two columns; returns (left leaf id, separator rect).
fn split_two_columns(h: &mut Host<TS, TestApp>, handle: &Handle<TS>, win: WindowId) -> (uzor::layout::docking::LeafId, Rect) {
    let leaf = h.snapshot().window(win).expect("window").dock.leaves[0].leaf;
    handle
        .dispatch(AppCommand::Layout(LayoutCmd::Split {
            win,
            leaf,
            dir: SplitDir::Right,
            panel: P("b"),
        }))
        .expect("dispatch");
    h.advance(0.017);
    h.advance(0.017);
    let snap = h.snapshot();
    let dock = &snap.window(win).expect("window").dock;
    assert_eq!(dock.leaves.len(), 2, "the split made a second leaf");
    assert_eq!(dock.separators.len(), 1, "one separator between them");
    (leaf, dock.separators[0].rect)
}

/// §9.2: the separator follows the drag and clamps at the neighbour's
/// minimum — slamming past it changes nothing more.
#[test]
fn splitter_drag_moves_and_clamps() {
    let (mut h, handle) = host();
    let win = open_with_panel(&mut h, &handle);
    let (left_leaf, sep) = split_two_columns(&mut h, &handle, win);
    let left_width = |h: &Host<TS, TestApp>| {
        h.snapshot()
            .window(win)
            .and_then(|w| w.dock.leaves.iter().find(|l| l.leaf == left_leaf).map(|l| l.rect.width))
            .expect("the left leaf survives")
    };
    let before = left_width(&h);

    let cx = sep.x + sep.width / 2.0;
    let cy = sep.y + sep.height / 2.0;
    down(&mut h, win, cx, cy);
    h.advance(0.017);
    mov(&mut h, win, cx + 150.0, cy);
    h.advance(0.017);
    up(&mut h, win, cx + 150.0, cy);
    h.advance(0.017);
    let grown = left_width(&h);
    assert!(grown > before, "the separator follows the drag: {before} -> {grown}");

    // Slam far past the neighbour's minimum, twice: the second slam is a no-op.
    for _ in 0..2 {
        let from = left_width(&h);
        down(&mut h, win, cx + from - before, cy);
        h.advance(0.017);
        mov(&mut h, win, cx + from - before + 2000.0, cy);
        h.advance(0.017);
        up(&mut h, win, cx + from - before + 2000.0, cy);
        h.advance(0.017);
    }
    assert_eq!(
        left_width(&h),
        left_width(&h),
        "clamped at the minimum (idempotent)"
    );
    let clamped = left_width(&h);
    down(&mut h, win, cx + clamped - before, cy);
    h.advance(0.017);
    mov(&mut h, win, cx + clamped - before + 2000.0, cy);
    h.advance(0.017);
    up(&mut h, win, cx + clamped - before + 2000.0, cy);
    h.advance(0.017);
    assert_eq!(left_width(&h), clamped, "the clamp holds");
}

/// §9.2: dragging a tab onto another leaf's body merges them into tabs.
#[test]
fn dock_tab_merge_via_drag() {
    let (mut h, handle) = host();
    let win = open_with_panel(&mut h, &handle);
    let (left_leaf, _sep) = split_two_columns(&mut h, &handle, win);
    let (from, to) = {
        let snap = h.snapshot();
        let dock = &snap.window(win).expect("window").dock;
        let from = dock.leaves.iter().find(|l| l.leaf != left_leaf).expect("right leaf").rect;
        let to = dock.leaves.iter().find(|l| l.leaf == left_leaf).expect("left leaf").rect;
        (from, to)
    };

    // b's single-tab header is the strip at the leaf top; drop at a's body centre.
    down(&mut h, win, from.x + from.width / 2.0, from.y + 12.0);
    h.advance(0.017);
    mov(&mut h, win, to.x + to.width / 2.0, to.y + to.height / 2.0);
    h.advance(0.017);
    up(&mut h, win, to.x + to.width / 2.0, to.y + to.height / 2.0);
    h.advance(0.017);
    h.advance(0.017);

    let snap = h.snapshot();
    let dock = &snap.window(win).expect("window").dock;
    assert_eq!(dock.leaves.len(), 1, "the drop merged the leaves: {:?}", dock.leaves);
    assert_eq!(dock.leaves[0].panels.len(), 2, "two tabs in the survivor");
}

/// §9.2: dragging a tab into the right gutter expands the window to the
/// right exactly once.
#[test]
fn expand_right_gutter_exactly_once() {
    let (mut h, handle) = host();
    let win = open_with_panel(&mut h, &handle);
    let gutter_x = 796.0; // 800-wide window: the right gutter is 792..800.

    down(&mut h, win, 700.0, 12.0); // the panel's tab header
    h.advance(0.017);
    mov(&mut h, win, gutter_x, 300.0);
    h.advance(0.017);
    h.advance(1.0); // the dwell completes inside the gutter
    up(&mut h, win, gutter_x, 300.0);
    h.advance(0.017);
    h.advance(0.5);

    // The expansion is one widened outer rect (the release retracts: the
    // drop was not committed). Counting every SetOuterRect would mistake
    // the retract for a second expand.
    let expansions = h
        .command_log()
        .iter()
        .filter(|(_, c)| matches!(c, WindowCommand::SetOuterRect { size, .. } if size.width > 800))
        .count();
    assert_eq!(expansions, 1, "one gutter dwell expands exactly once");
}

/// §9.2: dragging a tab out of the window spawns a new OS window holding
/// the panel.
#[test]
fn drag_out_spawns_window() {
    let (mut h, handle) = host();
    let win = open_with_panel(&mut h, &handle);
    // Tear-off floats in-window by default; window spawn is opt-in.
    handle
        .dispatch(AppCommand::Layout(LayoutCmd::SetPolicy(LayoutPolicy {
            drag_out: DragOutPolicy::Enabled {
                chrome: DragOutChrome::Os,
            },
            ..LayoutPolicy::default()
        })))
        .expect("dispatch");
    h.advance(0.017);
    let (left_leaf, _sep) = split_two_columns(&mut h, &handle, win);
    let from = {
        let snap = h.snapshot();
        let dock = &snap.window(win).expect("window").dock;
        dock.leaves
            .iter()
            .find(|l| l.leaf != left_leaf)
            .expect("right leaf")
            .rect
    };

    down(&mut h, win, from.x + from.width / 2.0, from.y + 12.0);
    h.advance(0.017);
    mov(&mut h, win, 700.0, 60.0);
    h.advance(0.017);
    mov(&mut h, win, 1150.0, 300.0); // past the 800-wide viewport
    h.advance(0.017);
    up(&mut h, win, 1150.0, 300.0);
    h.advance(0.017);
    h.advance(0.017);

    assert!(
        h.command_log().iter().any(|(_, c)| matches!(c, WindowCommand::Spawn(_))),
        "the tear-off asked the host for a window"
    );
    assert_eq!(
        h.snapshot().windows.len(),
        2,
        "the torn-off panel lives in its own window"
    );
    assert!(
        h.snapshot().windows.iter().all(|w| w.id == win || !w.dock.leaves.is_empty()),
        "the new window holds a docked panel"
    );
}

// -- §9.2: layout blob --------------------------------------------------------------

/// §9.2: `RequestBlob` answers with a blob; after a mutation `Restore`
/// brings the captured structure back.
#[test]
fn layout_blob_roundtrip() {
    let (mut h, handle) = host();
    let win = open_with_panel(&mut h, &handle);
    handle
        .dispatch(AppCommand::Layout(LayoutCmd::OpenPanel {
            win,
            panel: P("b"),
            at: DockTarget::Root,
        }))
        .expect("dispatch");
    h.advance(0.017);
    let panel_count = |h: &Host<TS, TestApp>| {
        h.snapshot()
            .window(win)
            .map(|w| w.dock.leaves.iter().map(|l| l.panels.len()).sum::<usize>())
            .unwrap_or(0)
    };
    assert_eq!(panel_count(&h), 2);

    handle
        .dispatch(AppCommand::Layout(LayoutCmd::RequestBlob {
            win,
            ticket: Ticket(7),
        }))
        .expect("dispatch");
    h.advance(0.017);
    let blob = h
        .runtime()
        .app()
        .intents
        .iter()
        .find_map(|i| match i {
            Intent::Dock(DockIntent::LayoutBlob { ticket, blob, .. }) if *ticket == Ticket(7) => {
                Some(LayoutBlob::from_bytes(blob.as_bytes().to_vec()))
            }
            _ => None,
        })
        .expect("the blob intent answers the request");

    let leaf = h.snapshot().window(win).expect("window").dock.leaves[0].leaf;
    handle
        .dispatch(AppCommand::Layout(LayoutCmd::ClosePanel { win, leaf, index: 1 }))
        .expect("dispatch");
    h.advance(0.017);
    assert_eq!(panel_count(&h), 1, "the close mutated the layout");

    handle
        .dispatch(AppCommand::Layout(LayoutCmd::Restore { win, blob }))
        .expect("dispatch");
    h.advance(0.017);
    assert!(
        h.runtime().app().intents.iter().any(|i| matches!(i, Intent::Dock(DockIntent::LayoutRestored { .. }))),
        "restore reports success: {:?}",
        h.runtime().app().intents
    );
    assert_eq!(panel_count(&h), 2, "the captured structure is back");
}

// -- §9.2: text, IME, clipboard -------------------------------------------------------

/// §9.2: click focuses the field, typing edits, preedit stays out of the
/// buffer while commit lands, Ctrl+A/C copies, Enter submits.
#[test]
fn text_ime_clipboard_flow() {
    let (mut h, handle) = host();
    let win = open_with_panel(&mut h, &handle);
    h.runtime_mut().app_mut().field = Some(Field {
        id: wid("field"),
        rect: Rect::new(10.0, 100.0, 200.0, 24.0),
        initial: "hi".to_string(),
        initialized: false,
    });
    h.advance(0.017); // registers and initialises the field

    click(&mut h, win, 30.0, 112.0); // past "hi" (2 glyphs x 8 px): caret at the end
    assert_eq!(focused(&h, win), Some(wid("field")), "click focuses the field");

    key(&mut h, win, KeyCode::A, ModifierKeys::default(), Some("a"));
    assert!(
        h.runtime().app().intents.iter().any(|i| matches!(
            i,
            Intent::Text(TextIntent::Changed { field, .. }) if *field == wid("field")
        )),
        "typing reports Changed: {:?}",
        h.runtime().app().intents
    );
    assert_eq!(h.runtime().app().seen_text.as_deref(), Some("hia"));

    h.input(
        win,
        InputEvent::Ime(ImeInput::Preedit {
            text: "\u{301}".to_string(),
            cursor: None,
        }),
    );
    h.advance(0.017);
    h.advance(0.017);
    assert_eq!(
        h.runtime().app().seen_text.as_deref(),
        Some("hia"),
        "preedit never enters the buffer"
    );
    h.input(win, InputEvent::Ime(ImeInput::Commit("!".to_string())));
    h.advance(0.017);
    h.advance(0.017);
    assert_eq!(h.runtime().app().seen_text.as_deref(), Some("hia!"));

    key(&mut h, win, KeyCode::A, mods_ctrl(), None); // select all
    h.input(
        win,
        InputEvent::Key(KeyInput {
            code: KeyCode::C,
            text: None,
            state: crate::types::bus::KeyState::Down,
            mods: mods_ctrl(),
        }),
    );
    let copied = h
        .advance(0.017)
        .window_commands
        .iter()
        .any(|(_, c)| matches!(c, WindowCommand::ClipboardWrite(t) if t == "hia!"));
    assert!(copied, "Ctrl+C writes the selection to the clipboard");

    key(&mut h, win, KeyCode::Enter, ModifierKeys::default(), None);
    assert!(
        h.runtime().app().intents.iter().any(|i| matches!(
            i,
            Intent::Text(TextIntent::Submitted { field, .. }) if *field == wid("field")
        )),
        "Enter submits: {:?}",
        h.runtime().app().intents
    );
}







