//! The §9.2 acceptance list, headless: every test drives the public host
//! surface only (`HeadlessHost`, `Handle`, commands and input events), so
//! the same scripts run unchanged against the native and web hosts later.

use std::sync::Arc;

use uzor::input::{KeyCode, ModifierKeys, MouseButton, Sense, TextFieldConfig};
use uzor::layout::OverlayKind;
use uzor::layout::docking::DockPanel;
use uzor::{Rect, WidgetId};

use crate::handle::{App, IntentCx, OverlayCx, PanelCx};
use crate::runtime::RuntimeConfig;
use crate::types::bus::{ClipboardResult, ImeInput, InputEvent, KeyInput, PointerInput};
use crate::types::command::{
    AppCommand, Binding, FocusCmd, KeymapCmd, KeymapScope, LayoutCmd, OverlayCmd, OverlayPolicy,
    OverlaySize, WindowCmd,
};
use crate::types::frame::Wake;
use crate::types::ids::{Seconds, WindowId};
use crate::types::intent::{CloseCause, DockIntent, Intent, OverlayIntent, TextIntent};
use crate::types::overlay_model::OverlayModel;
use crate::types::spec::{PanelHome, Spec};
use crate::types::window::{Point, SizePx};
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
            at: crate::types::command::DockTarget::Root,
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
