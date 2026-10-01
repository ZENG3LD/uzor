//! InputEngine: focus scopes, pointer capture, cook, hover / press / click,
//! text and IME, caret blink (design §3.4).
//!
//! The one writer of per-window interaction state. Each window has one
//! library [`InputCoordinator`] as its storage (registrations, hit index,
//! [`TextFieldStore`], widget state), one
//! library [`CookState`] for event-level cooking, the pointer accumulator
//! the coordinator's next frame is fed from, the focus-scope stack, the
//! pointer capture and the caret-blink phase. Pure and deterministic: every
//! time-dependent op carries the host clock.
//!
//! ## Coordinator access and the future driver trait
//!
//! The coordinator's state-mutating methods (`begin_frame`, `end_frame`,
//! `set_focus`, `clear_focus`, `focus_next` / `focus_prev`,
//! `focus_text_field`, `grab_pointer` / `release_pointer`, `on_char`,
//! `on_key`, `text_fields_mut`) are called only from this file. Lib brief L6
//! moves them behind `CoordinatorDriver` (ban F7 keeps that trait inside
//! this engine and the kernel); until then they are plain inherent calls.
//! [`InputEngine::registrar`] is the compose-phase door for widget
//! registration; after L6 the `&mut InputCoordinator` it returns can no
//! longer reach those methods.
//!
//! ## Pointer (routing, design §4.3)
//!
//! - [`InputOp::Pointer`] is step 1: the engine's `CookState` counts clicks
//!   (single / double / triple within `DOUBLE_CLICK_WINDOW_S` and
//!   `MULTI_CLICK_MAX_DIST_PX`) and arms drags past `DRAG_THRESHOLD_PX`,
//!   and the answer says whether a [`Capture`] owns the event (step 2). A
//!   capture bypasses every hit test until the button is released (or the
//!   OS cancels the gesture); the release itself still goes to the owner.
//! - [`InputOp::Deliver`] is step 5: an event nothing above claimed enters
//!   the coordinator's next-frame input (press / release edges; positions
//!   always follow the pointer). Claimed presses never reach content.
//! - [`InputOp::BeginFrame`] feeds the accumulated input to the coordinator
//!   with `InputState::time` stamped from the host clock, so the
//!   coordinator's own cook (fed in `end_frame`) reports real
//!   `double_clicked` / `triple_clicked`. [`InputOp::EndFrame`] evaluates
//!   the frame's registrations into hover and this frame's clicks.
//!
//! ## Focus scopes (H2 §3)
//!
//! A stack of `{ owner, members, saved focus }`. Pushing saves and clears
//! the current focus; while a scope is active, focus can only move to one
//! of its members (`FocusOp::Set` outside is a no-op, Tab / Shift+Tab cycle
//! the members registered this frame in list order); popping the active
//! scope with `restore` puts the saved focus back (a saved text field
//! keeps its engagement), without `restore` clears it. Popping a scope
//! below the top hands its saved focus to the scope above it. A click
//! that focuses a text field outside the active scope is reverted at
//! `EndFrame`, so focus is always inside the active scope. With no scope,
//! Tab cycles every focusable widget (the coordinator's global order).
//!
//! ## Text and IME
//!
//! Key step 1 ([`InputOp::Key`]): with a focused text field, its editing
//! chords (characters, Backspace / Delete, arrows with Shift / Ctrl word
//! movement, Home / End, Ctrl+A / C / X / V, Enter) are consumed before any
//! keymap binding; Tab / Shift+Tab move focus. Everything else is passed on
//! ([`InputEffect::KeyPassed`]); Escape is never consumed here, so an
//! overlay can close on it (step 2). IME preedit goes to
//! `TextFieldStore::preedit_set` (an overlay span, never in the buffer);
//! commit to `ime_commit`. Copy / cut report [`InputEffect::CopyText`]
//! (masked fields never copy); paste asks the host with
//! [`InputEffect::NeedPaste`] under an engine-minted ticket (at or above
//! [`INPUT_TICKET_BASE`]) and inserts the [`InputOp::Clipboard`] answer if
//! the same field is still focused. The IME caret area and the IME-allowed
//! flag are reported only when they change.
//!
//! ## Caret blink
//!
//! While a text field is focused and engaged (the user clicked or typed
//! into it) and the window has OS focus, the engine keeps one deadline
//! armed at the next blink edge ([`InputEffect::CaretDeadline`]; the kernel
//! arms `TimerOwner::CaretBlink { win }`). Visibility is the store's
//! `cursor_visible(now_ms)` (500 ms phases) anchored at the last caret
//! change: any edit, caret move, selection, preedit or focus change
//! restarts the phase visible. At each edge ([`InputEngine::tick`]) the
//! engine flips visibility and asks for a repaint of only that field
//! ([`InputEffect::InvalidateField`] with the field's last drawn rect; the
//! kernel maps it to the region containing it). Blur, loss of window
//! focus, un-engagement or the field being unregistered disarms it.
//!
//! ## Plain-text selection (T2)
//!
//! One selection owner per window, held here: selectable plain text has no
//! store (unlike text fields), so the app reports each selectable widget's
//! content and line geometry every frame through
//! [`InputEngine::update_selectable`] (the compose-phase door, mirroring
//! `TextFieldStore::update_field`; not revisioned — the change is evaluated
//! at [`InputOp::EndFrame`]). A widget not reported this frame stops being
//! selectable and its selection drops. The owner state is a small
//! `Vec<(WidgetId, TextSelection)>` — usually one entry; a panel / window
//! Ctrl+A escalation selects several widgets at once.
//!
//! A press on a selectable widget records the char under the pointer as
//! the drag anchor; no pointer capture is taken — the cook state tracks
//! the press window-wide, exactly the mechanism text fields use, and
//! `char_at_point` clamps out-of-rect positions to the line ends. While
//! the press is armed (or its release just ended the drag) the selection
//! extends anchor → current pointer char every frame, so the highlight is
//! live during the drag. A release that was not a drag and lands on the
//! same selectable widget is a click: the selection collapses / expands to
//! word / line by the press sequence's click count (the engine's own
//! `CookState`, not the coordinator's click reporting — a label registers
//! `Sense::HOVER` + `select`, no `click` bit, and must not start
//! swallowing clicks). A press anywhere else clears the selection.
//! Escape and focus changes do not clear it (browser-like).
//!
//! Keys, no focused text field: Ctrl+C with a non-empty selection reports
//! [`InputEffect::CopyText`] (the same conduction as a field copy — the
//! kernel enqueues `WindowCommand::ClipboardWrite`); with nothing selected
//! the key routes on. Ctrl+A selects all with escalation
//! widget → panel → window: the panel step takes every selectable whose
//! rect is inside the nearest ancestor registered as `Panel` /
//! `BlackboxPanel` (via `widget_parent`), the window step every selectable
//! in the window; multi-widget selections copy in reading order
//! (top-left, newline-joined). The escalation level resets on any
//! pointer-driven selection.
//!
//! Every selection change repaints through the existing
//! [`InputEffect::InvalidateField`] with the widget's rect, and is
//! revisioned (the map is observable through
//! [`InputEngineView::selections`]).
//!
//! Out of scope (recorded for the owner): word-granularity drag after a
//! double click, Shift+click extend, drag-and-drop of selected text.
//!
//! ## Revision
//!
//! Bumped exactly when observable state changes: the window set, focus,
//! scopes, capture, hover, pressed, this frame's clicks, the focused
//! field's caret (text, cursor, selection, preedit, engagement), caret
//! visibility and the plain-text selections. Pointer positions, cook
//! bookkeeping, IME bookkeeping and a pending paste are not revisioned
//! (they never appear in the snapshot).

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use smallvec::SmallVec;
use uzor::input::core::cook::CookState;
use uzor::input::text::selection::{char_at_point, SelectionLine, TextSelection};
use uzor::input::{
    InputCoordinator, InputState, KeyCode, KeyPress, ModifierKeys, MouseButton, TextAction,
    TextFieldStore, WidgetKind,
};
use uzor::{Rect, WidgetId};

use crate::types::bus::{
    ClipboardResult, ImeInput, KeyInput, KeyState, PointerInput, WheelDelta, WheelInput,
};
use crate::types::ids::{Revision, ScopeId, Seconds, Ticket, WindowId};
use crate::types::intent::TextIntent;
use crate::types::ops::{
    Capture, CookedPointer, FocusOp, InputEffect, InputOp, PointerTarget, ScopeOwner,
};
use crate::types::snapshot::InputView;

/// Effects of one InputEngine op or tick.
pub type InputEffects = SmallVec<[InputEffect; 2]>;

/// First clipboard ticket the engine mints for paste. App tickets stay
/// below it, so a `ClipboardResult` at or above it belongs to this engine.
pub const INPUT_TICKET_BASE: u64 = 1 << 63;

/// One caret blink phase (visible or hidden), the store's 500 ms.
pub const CARET_BLINK_INTERVAL: Seconds = Seconds(0.5);

const CARET_BLINK_MS: u64 = 500;

/// Logical pixels per wheel line, for pixel-unit wheel deltas (the same
/// factor the desktop input bridge uses); the coordinator counts lines.
const WHEEL_PIXELS_PER_LINE: f64 = 20.0;

// ---------------------------------------------------------------------------
// Per-window state
// ---------------------------------------------------------------------------

/// Pointer input gathered between two frames; becomes the coordinator's
/// `InputState` at `BeginFrame`.
#[derive(Clone, Debug, Default)]
struct PointerAccum {
    pos: Option<(f64, f64)>,
    prev_pos: Option<(f64, f64)>,
    button_down: Option<MouseButton>,
    clicked: Option<MouseButton>,
    mods: ModifierKeys,
    scroll: (f64, f64),
}

/// The focus the engine restores when a scope closes.
#[derive(Clone, Debug, PartialEq, Eq)]
struct SavedFocus {
    id: WidgetId,
    /// The saved widget was an engaged text field.
    engaged: bool,
}

#[derive(Clone, Debug)]
struct FocusScope {
    owner: ScopeOwner,
    members: Vec<WidgetId>,
    saved: Option<SavedFocus>,
}

/// Plain selectable text reported for one frame through the compose door
/// [`InputEngine::update_selectable`].
#[derive(Clone, Debug)]
struct SelectableText {
    text: String,
    lines: Vec<SelectionLine>,
}

/// A press that began on a selectable widget: the selection drag anchor.
#[derive(Clone, Debug)]
struct PressSel {
    widget: WidgetId,
    anchor: usize,
}

/// Where a Ctrl+A escalation stands for its widget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EscLevel {
    /// Last select-all covered only the widget itself.
    Widget,
    /// … every selectable inside the nearest panel ancestor.
    Panel,
    /// … every selectable in the window.
    Window,
}

/// Everything about the focused text field whose change restarts the
/// blink phase (geometry is deliberately not part of it).
#[derive(Clone, Debug, PartialEq, Eq)]
struct CaretKey {
    field: WidgetId,
    engaged: bool,
    text: String,
    cursor: usize,
    selection: Option<usize>,
    preedit: String,
    preedit_cursor: usize,
}

/// The engine's own focus record: the widget the view reports.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Focused {
    id: WidgetId,
    /// Focused through the text store (a registered text field).
    field: bool,
}

struct PerWindowInput {
    coord: InputCoordinator,
    /// Per-widget persistent typed state (scroll offsets, expand flags);
    /// the compose-phase door for the app hook's `Widgets` face (design
    /// §3.4 names it here; F3 had no consumer, F7 does).
    states: uzor::app_context::StateRegistry,
    cook: CookState,
    pointer: PointerAccum,
    capture: Option<Capture>,
    scopes: Vec<FocusScope>,
    focus: Option<Focused>,
    hover: Option<WidgetId>,
    pressed: Option<WidgetId>,
    clicked: SmallVec<[(WidgetId, u8); 2]>,
    os_focused: bool,
    caret: Option<CaretKey>,
    blink_anchor_ms: u64,
    caret_visible: bool,
    armed: Option<Seconds>,
    ime_area: Option<Rect>,
    ime_allowed: bool,
    pending_paste: Option<(Ticket, WidgetId)>,
    // Plain-text selection (T2). `selectables` is last completed frame's
    // reported set (read by pointer ops); the compose door fills
    // `reported` and EndFrame swaps them.
    selectables: HashMap<WidgetId, SelectableText>,
    reported: HashMap<WidgetId, SelectableText>,
    selections: Vec<(WidgetId, TextSelection)>,
    press_sel: Option<PressSel>,
    drag_ended: bool,
    escalation: Option<(WidgetId, EscLevel)>,
}

impl PerWindowInput {
    fn new() -> Self {
        Self {
            coord: InputCoordinator::new(),
            states: uzor::app_context::StateRegistry::new(),
            cook: CookState::default(),
            pointer: PointerAccum::default(),
            capture: None,
            scopes: Vec::new(),
            focus: None,
            hover: None,
            pressed: None,
            clicked: SmallVec::new(),
            // A window the host just created is assumed focused until the
            // host says otherwise (not every host sends the initial echo).
            os_focused: true,
            caret: None,
            blink_anchor_ms: 0,
            caret_visible: false,
            armed: None,
            ime_area: None,
            ime_allowed: false,
            pending_paste: None,
            selectables: HashMap::new(),
            reported: HashMap::new(),
            selections: Vec::new(),
            press_sel: None,
            drag_ended: false,
            escalation: None,
        }
    }

    // -- focus ----------------------------------------------------------------

    /// `id` may take focus: no scope is active, or it is a member of the
    /// active one.
    fn allowed(&self, id: &WidgetId) -> bool {
        self.scopes.last().is_none_or(|s| s.members.contains(id))
    }

    /// Focus `id` in the coordinator (text fields through the store, which
    /// also syncs widget focus; anything else blurs the store). No guard.
    fn focus_raw(&mut self, id: WidgetId) {
        let field = self.coord.text_fields().has_field(&id);
        if field {
            self.coord.focus_text_field(&id);
        } else {
            self.coord.set_focus(id.clone());
        }
        self.focus = Some(Focused { id, field });
    }

    fn clear_raw(&mut self) -> bool {
        let had = self.focus.is_some() || self.coord.text_fields().focused().is_some();
        self.coord.clear_focus();
        self.focus = None;
        had
    }

    fn focused_id(&self) -> Option<&WidgetId> {
        self.focus.as_ref().map(|f| &f.id)
    }

    /// Tab order candidates: the active scope's members registered this
    /// frame, in list order.
    fn scoped_candidates(&self) -> Option<Vec<WidgetId>> {
        let scope = self.scopes.last()?;
        Some(
            scope
                .members
                .iter()
                .filter(|m| self.coord.widget_rect(m).is_some())
                .cloned()
                .collect(),
        )
    }

    /// Tab / Shift+Tab. `None` = there was nothing to move to.
    fn cycle(&mut self, forward: bool) -> Option<bool> {
        let before = self.focus.clone();
        match self.scoped_candidates() {
            Some(list) => {
                if list.is_empty() {
                    return None;
                }
                let n = list.len();
                let at = self
                    .focused_id()
                    .and_then(|f| list.iter().position(|m| m == f));
                let next = match (at, forward) {
                    (Some(i), true) => (i + 1) % n,
                    (None, true) => 0,
                    (Some(i), false) => (i + n - 1) % n,
                    (None, false) => n - 1,
                };
                if self.focused_id() != Some(&list[next]) {
                    self.focus_raw(list[next].clone());
                }
            }
            None => {
                if forward {
                    self.coord.focus_next();
                } else {
                    self.coord.focus_prev();
                }
                let target = self.coord.focused_widget().cloned()?;
                if self.focused_id() != Some(&target) || !self.store_matches_focus() {
                    self.focus_raw(target);
                }
            }
        }
        Some(self.focus != before)
    }

    /// The text store's focus agrees with the engine's record.
    fn store_matches_focus(&self) -> bool {
        let store = self.coord.text_fields().focused();
        match &self.focus {
            Some(f) if f.field => store == Some(&f.id),
            _ => store.is_none(),
        }
    }

    fn focus_op(
        &mut self,
        win: WindowId,
        op: FocusOp,
        next_scope: &mut u64,
        fx: &mut InputEffects,
    ) -> bool {
        match op {
            FocusOp::Set(id) => {
                if !self.allowed(&id) || self.focused_id() == Some(&id) {
                    return false;
                }
                self.focus_raw(id);
                true
            }
            FocusOp::Clear => self.clear_raw(),
            FocusOp::Next => self.cycle(true).unwrap_or(false),
            FocusOp::Prev => self.cycle(false).unwrap_or(false),
            FocusOp::PushScope { owner, members } => {
                if self.scopes.iter().any(|s| s.owner == owner) {
                    return self.set_members(&owner, members);
                }
                let saved = self.focus.as_ref().map(|f| SavedFocus {
                    engaged: f.field && self.coord.text_fields().is_engaged(&f.id),
                    id: f.id.clone(),
                });
                self.clear_raw();
                self.scopes.push(FocusScope {
                    owner: owner.clone(),
                    members,
                    saved,
                });
                *next_scope += 1;
                fx.push(InputEffect::ScopePushed {
                    win,
                    owner,
                    scope: ScopeId(*next_scope),
                });
                true
            }
            FocusOp::SetMembers { owner, members } => self.set_members(&owner, members),
            FocusOp::PopScope { owner, restore } => self.pop_scope(&owner, restore),
        }
    }

    fn set_members(&mut self, owner: &ScopeOwner, members: Vec<WidgetId>) -> bool {
        let Some(i) = self.scopes.iter().position(|s| &s.owner == owner) else {
            return false;
        };
        if self.scopes[i].members == members {
            return false;
        }
        self.scopes[i].members = members;
        let active = i + 1 == self.scopes.len();
        if active {
            if let Some(f) = self.focused_id().cloned() {
                if !self.allowed(&f) {
                    self.clear_raw();
                }
            }
        }
        true
    }

    fn pop_scope(&mut self, owner: &ScopeOwner, restore: bool) -> bool {
        let Some(i) = self.scopes.iter().position(|s| &s.owner == owner) else {
            return false;
        };
        let scope = self.scopes.remove(i);
        if i < self.scopes.len() {
            // A scope below the top closed: the scope above saved a focus
            // inside it, which is gone; it inherits the closed scope's.
            self.scopes[i].saved = scope.saved;
            return true;
        }
        self.clear_raw();
        if let (true, Some(saved)) = (restore, scope.saved) {
            if self.allowed(&saved.id) {
                self.focus_raw(saved.id.clone());
                if saved.engaged {
                    self.coord.text_fields_mut().engage(&saved.id);
                }
            }
        }
        true
    }

    /// After `end_frame`: accept a click-focus into a text field, revert
    /// one outside the active scope, and forget a field that disappeared.
    fn reconcile_focus(&mut self) -> bool {
        let store = self.coord.text_fields().focused().cloned();
        match (store, self.focus.clone()) {
            (Some(s), Some(f)) if f.field && s == f.id => false,
            (Some(s), _) if self.allowed(&s) => {
                self.coord.set_focus(s.clone());
                self.focus = Some(Focused { id: s, field: true });
                true
            }
            (Some(_), prev) => {
                // Clicked into a field the active scope does not contain.
                self.coord.text_fields_mut().blur();
                if let Some(f) = prev.filter(|f| f.field) {
                    let engaged = self.coord.text_fields().is_engaged(&f.id);
                    self.coord.focus_text_field(&f.id);
                    if engaged {
                        self.coord.text_fields_mut().engage(&f.id);
                    }
                }
                false
            }
            (None, Some(f)) if f.field => {
                // The focused field was unregistered or blurred.
                self.coord.clear_focus();
                self.focus = None;
                true
            }
            (None, _) => false,
        }
    }

    // -- pointer --------------------------------------------------------------

    fn set_capture(&mut self, capture: Option<Capture>) -> bool {
        if self.capture == capture {
            return false;
        }
        if let Some(Capture::Widget(_)) = &self.capture {
            self.coord.release_pointer();
        }
        if let Some(Capture::Widget(id)) = &capture {
            self.coord.grab_pointer(id.clone());
        }
        self.capture = capture;
        true
    }

    fn cook(&mut self, win: WindowId, now: Seconds, input: PointerInput) -> (bool, CookedPointer) {
        let t = now.get();
        let target = match &self.capture {
            Some(c) => PointerTarget::Captured(c.clone()),
            None => PointerTarget::HitTest,
        };
        let mut drag_started = false;
        let mut ended_drag = false;
        let mut changed = false;
        match input {
            PointerInput::Moved { pos, mods } => {
                self.pointer.pos = Some((pos.x, pos.y));
                self.pointer.mods = mods;
                drag_started = self.cook.motion(pos.x, pos.y).drag_started;
            }
            PointerInput::Down { pos, button, mods } => {
                self.pointer.pos = Some((pos.x, pos.y));
                self.pointer.mods = mods;
                // A second button during a held press does not restart it.
                if self.cook.press_origin.is_none() {
                    self.cook.press(pos.x, pos.y, button, t);
                }
            }
            PointerInput::Up { pos, button, mods } => {
                self.pointer.pos = Some((pos.x, pos.y));
                self.pointer.mods = mods;
                if self.cook.press_origin.map(|o| o.button) == Some(button) {
                    ended_drag = self.cook.release(pos.x, pos.y, button, t);
                    self.drag_ended |= ended_drag;
                    changed = self.set_capture(None);
                }
            }
            PointerInput::Entered | PointerInput::RawDelta { .. } => {}
            PointerInput::Left => self.pointer.pos = None,
            PointerInput::Cancelled => {
                self.cook.press_origin = None;
                self.cook.drag_armed = false;
                self.pointer.button_down = None;
                changed = self.set_capture(None);
                changed |= self.pressed.take().is_some();
                self.press_sel = None;
                self.drag_ended = false;
            }
        }
        let cooked = CookedPointer {
            win,
            input,
            click_count: self.cook.click_count,
            drag_armed: self.cook.drag_armed,
            drag_started,
            ended_drag,
            target,
        };
        (changed, cooked)
    }

    /// The content widget a pointer at `pos` goes to: the capturing widget,
    /// else the topmost hit of last frame's registrations.
    fn content_target(&self, pos: Option<(f64, f64)>) -> Option<WidgetId> {
        if let Some(Capture::Widget(id)) = &self.capture {
            return Some(id.clone());
        }
        let (x, y) = pos?;
        self.coord.hit_test_now(x, y)
    }

    fn deliver(
        &mut self,
        win: WindowId,
        input: PointerInput,
        fx: &mut InputEffects,
    ) -> (bool, Option<WidgetId>) {
        match input {
            PointerInput::Moved { pos, .. } => {
                self.pointer.pos = Some((pos.x, pos.y));
                (false, self.content_target(self.pointer.pos))
            }
            PointerInput::Down { pos, button, .. } => {
                self.pointer.pos = Some((pos.x, pos.y));
                self.pointer.button_down = Some(button);
                let target = self.content_target(self.pointer.pos);
                let changed = self.pressed != target;
                self.pressed.clone_from(&target);
                let sel_changed = self.press_down(win, target.as_ref(), pos.x, pos.y, fx);
                (changed | sel_changed, target)
            }
            PointerInput::Up { pos, button, .. } => {
                self.pointer.pos = Some((pos.x, pos.y));
                self.pointer.button_down = None;
                self.pointer.clicked = Some(button);
                let target = self.content_target(self.pointer.pos);
                let released = self.pressed.take().is_some();
                let sel_changed = self.release_select(win, target.as_ref(), fx);
                (released | sel_changed, target)
            }
            PointerInput::Entered
            | PointerInput::Left
            | PointerInput::Cancelled
            | PointerInput::RawDelta { .. } => (false, None),
        }
    }

    /// A release on the same selectable widget the press began on, and
    /// the press never crossed the drag threshold: a click. The selection
    /// collapses / expands to word / line by the click count.
    fn release_select(
        &mut self,
        win: WindowId,
        target: Option<&WidgetId>,
        fx: &mut InputEffects,
    ) -> bool {
        if self.drag_ended {
            return false;
        }
        let Some(p) = self.press_sel.clone() else { return false };
        if target != Some(&p.widget) {
            return false;
        }
        let Some(st) = self.selectables.get(&p.widget) else { return false };
        let sel = TextSelection::for_click_count(&st.text, p.anchor, self.cook.click_count.max(1));
        let changed = self.set_selection(win, &p.widget, sel, fx);
        self.escalation = Some((p.widget, EscLevel::Widget));
        changed
    }

    fn wheel(&mut self, input: WheelInput) -> Option<WidgetId> {
        let (dx, dy) = match input.delta {
            WheelDelta::Lines { x, y } => (x, y),
            WheelDelta::Pixels { x, y } => (x / WHEEL_PIXELS_PER_LINE, y / WHEEL_PIXELS_PER_LINE),
        };
        self.pointer.pos = Some((input.pos.x, input.pos.y));
        self.pointer.mods = input.mods;
        self.pointer.scroll.0 += dx;
        self.pointer.scroll.1 += dy;
        if let Some(Capture::Widget(id)) = &self.capture {
            return Some(id.clone());
        }
        self.coord.process_scroll(input.pos.x, input.pos.y)
    }

    // -- plain-text selection (T2) -------------------------------------------

    fn sel_get(&self, widget: &WidgetId) -> Option<TextSelection> {
        self.selections.iter().find(|(w, _)| w == widget).map(|(_, s)| *s)
    }

    fn sel_set(&mut self, widget: WidgetId, sel: TextSelection) -> bool {
        if let Some(slot) = self.selections.iter_mut().find(|(w, _)| *w == widget) {
            if slot.1 == sel {
                return false;
            }
            slot.1 = sel;
            return true;
        }
        self.selections.push((widget, sel));
        true
    }

    fn sel_remove(&mut self, widget: &WidgetId) -> bool {
        let before = self.selections.len();
        self.selections.retain(|(w, _)| w != widget);
        self.selections.len() != before
    }

    /// Set one widget's selection, repainting it on change.
    fn set_selection(
        &mut self,
        win: WindowId,
        widget: &WidgetId,
        sel: TextSelection,
        fx: &mut InputEffects,
    ) -> bool {
        if !self.sel_set(widget.clone(), sel) {
            return false;
        }
        fx.push(InputEffect::InvalidateField {
            win,
            rect: self.coord.widget_rect(widget),
        });
        true
    }

    /// Drop every selection, repainting on change.
    fn clear_selections(&mut self, win: WindowId, fx: &mut InputEffects) -> bool {
        if self.selections.is_empty() {
            return false;
        }
        for (w, _) in &self.selections {
            fx.push(InputEffect::InvalidateField {
                win,
                rect: self.coord.widget_rect(w),
            });
        }
        self.selections.clear();
        self.escalation = None;
        true
    }

    /// Selectable widget ids in reading order (top-left), for multi-widget
    /// select-all and copy concatenation.
    fn selectable_ids_by_row(&self) -> Vec<WidgetId> {
        let mut ids: Vec<(f64, f64, WidgetId)> = self
            .selectables
            .keys()
            .map(|w| {
                let r = self
                    .coord
                    .widget_rect(w)
                    .unwrap_or_else(|| Rect::new(0.0, 0.0, 0.0, 0.0));
                (r.y, r.x, w.clone())
            })
            .collect();
        ids.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.total_cmp(&b.1)));
        ids.into_iter().map(|(_, _, w)| w).collect()
    }

    /// The selected text of every selected widget, reading order, newline
    /// joined; `None` when nothing (or only carets) is selected.
    fn selected_text_concat(&self) -> Option<String> {
        if self.selections.is_empty() {
            return None;
        }
        let mut parts: Vec<String> = Vec::new();
        for w in self.selectable_ids_by_row() {
            if let (Some(sel), Some(st)) = (self.sel_get(&w), self.selectables.get(&w)) {
                let s = sel.selected_text(&st.text);
                if !s.is_empty() {
                    parts.push(s.to_owned());
                }
            }
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join("\n"))
        }
    }

    /// The rect of the nearest ancestor registered as `Panel` /
    /// `BlackboxPanel`, if any — the Ctrl+A escalation boundary.
    fn nearest_panel_rect(&self, widget: &WidgetId) -> Option<Rect> {
        let mut cur = self.coord.widget_parent(widget);
        while let Some(p) = cur {
            if matches!(
                self.coord.widget_kind(&p),
                Some(WidgetKind::Panel | WidgetKind::BlackboxPanel)
            ) {
                return self.coord.widget_rect(&p);
            }
            cur = self.coord.widget_parent(&p);
        }
        None
    }

    /// Select-all of every selectable whose rect is inside `rect`.
    fn select_in_rect(&mut self, win: WindowId, rect: &Rect, fx: &mut InputEffects) -> bool {
        let mut changed = false;
        for w in self.selectable_ids_by_row() {
            if let Some(r) = self.coord.widget_rect(&w) {
                let inside = r.x >= rect.x
                    && r.y >= rect.y
                    && r.x + r.width <= rect.x + rect.width
                    && r.y + r.height <= rect.y + rect.height;
                if inside {
                    if let Some(st) = self.selectables.get(&w) {
                        let sel = TextSelection::all(&st.text);
                        changed |= self.set_selection(win, &w, sel, fx);
                    }
                }
            }
        }
        changed
    }

    /// Select-all of every selectable in the window.
    fn select_everything(&mut self, win: WindowId, fx: &mut InputEffects) -> bool {
        let mut changed = false;
        for w in self.selectable_ids_by_row() {
            if let Some(st) = self.selectables.get(&w) {
                let sel = TextSelection::all(&st.text);
                changed |= self.set_selection(win, &w, sel, fx);
            }
        }
        changed
    }

    /// A press landed on `target` (`None` = empty space): anything selected
    /// that is not the target clears; a selectable target starts a press.
    fn press_down(
        &mut self,
        win: WindowId,
        target: Option<&WidgetId>,
        x: f64,
        y: f64,
        fx: &mut InputEffects,
    ) -> bool {
        let mut changed = false;
        if !target.is_some_and(|t| self.sel_get(t).is_some()) {
            changed |= self.clear_selections(win, fx);
        }
        self.press_sel = None;
        if let Some(t) = target {
            if let Some(st) = self.selectables.get(t) {
                let anchor = char_at_point(&st.lines, x, y);
                self.press_sel = Some(PressSel {
                    widget: t.clone(),
                    anchor,
                });
            }
        }
        changed
    }

    /// Selection bookkeeping at EndFrame: live drag extension, click
    /// selection, press-session cleanup, stale-widget pruning.
    fn selection_frame(&mut self, win: WindowId, fx: &mut InputEffects) -> bool {
        let mut changed = false;

        // Live drag: the press is armed past the threshold (or its release
        // just ended the drag) — extend the selection to the current
        // pointer char every frame, so the highlight is live.
        if let Some(p) = self.press_sel.clone() {
            if self.cook.drag_armed || self.drag_ended {
                if let (Some(st), Some((x, y))) =
                    (self.selectables.get(&p.widget), self.pointer.pos)
                {
                    let sel = TextSelection::new(p.anchor, char_at_point(&st.lines, x, y));
                    changed |= self.set_selection(win, &p.widget, sel, fx);
                }
            }
        }

        // Click selection happens at the release (Deliver Up), not here —
        // a label has no `click` sense, so the coordinator never reports
        // its clicks.

        // The press session ends when the button is up.
        if self.pointer.button_down.is_none() {
            self.press_sel = None;
            self.drag_ended = false;
        }

        // Prune: only widgets reported this frame stay selectable; their
        // selections drop with them.
        let fresh = std::mem::take(&mut self.reported);
        let gone: Vec<WidgetId> = self
            .selections
            .iter()
            .map(|(w, _)| w.clone())
            .filter(|w| !fresh.contains_key(w))
            .collect();
        if !gone.is_empty() {
            for w in &gone {
                fx.push(InputEffect::InvalidateField {
                    win,
                    rect: self.coord.widget_rect(w),
                });
                self.sel_remove(w);
            }
            if let Some((w, _)) = &self.escalation {
                if gone.contains(w) {
                    self.escalation = None;
                }
            }
            changed = true;
        }
        self.selectables = fresh;

        changed
    }

    /// Ctrl+A with no focused text field: select all, escalating widget →
    /// panel → window across repeated presses. `Some(changed)` = consumed;
    /// `None` = no selectable context (the key routes on).
    fn ctrl_a(&mut self, win: WindowId, fx: &mut InputEffects) -> Option<bool> {
        if let Some((w, level)) = self.escalation.clone() {
            let next = match level {
                EscLevel::Widget => match self.nearest_panel_rect(&w) {
                    Some(rect) => {
                        self.select_in_rect(win, &rect, fx);
                        EscLevel::Panel
                    }
                    None => {
                        self.select_everything(win, fx);
                        EscLevel::Window
                    }
                },
                EscLevel::Panel => {
                    self.select_everything(win, fx);
                    EscLevel::Window
                }
                EscLevel::Window => return Some(false),
            };
            self.escalation = Some((w, next));
            return Some(true);
        }
        // First press: the hovered selectable, else the single selection.
        let start = self
            .hover
            .clone()
            .filter(|h| self.selectables.contains_key(h))
            .or_else(|| (self.selections.len() == 1).then(|| self.selections[0].0.clone()));
        let Some(w) = start else { return None };
        let Some(st) = self.selectables.get(&w) else { return None };
        let sel = TextSelection::all(&st.text);
        let changed = self.set_selection(win, &w, sel, fx);
        self.escalation = Some((w, EscLevel::Widget));
        Some(changed)
    }

    // -- frame ----------------------------------------------------------------

    fn begin_frame(&mut self, now: Seconds) -> bool {
        let mut state = InputState::new();
        state.pointer.pos = self.pointer.pos;
        state.pointer.prev_pos = self.pointer.prev_pos;
        state.pointer.button_down = self.pointer.button_down;
        state.pointer.clicked = self.pointer.clicked.take();
        state.modifiers = self.pointer.mods;
        state.scroll_delta = std::mem::take(&mut self.pointer.scroll);
        state.time = now.get();
        self.pointer.prev_pos = self.pointer.pos;
        self.coord.begin_frame(state);
        let had_clicks = !self.clicked.is_empty();
        self.clicked.clear();
        had_clicks
    }

    fn end_frame(&mut self, win: WindowId, fx: &mut InputEffects) -> bool {
        let mut hover = None;
        let mut clicked = SmallVec::new();
        for (id, r) in self.coord.end_frame() {
            if r.hovered {
                hover = Some(id.clone());
            }
            if r.clicked {
                let count = if r.triple_clicked {
                    3
                } else if r.double_clicked {
                    2
                } else {
                    1
                };
                clicked.push((id, count));
            }
        }
        let mut changed = hover != self.hover || clicked != self.clicked;
        self.hover = hover;
        self.clicked = clicked;
        changed |= self.selection_frame(win, fx);
        changed |= self.reconcile_focus();
        changed
    }

    // -- text -----------------------------------------------------------------

    /// The focused text field, when the store and the record agree.
    fn text_focus(&self) -> Option<WidgetId> {
        let f = self.focus.as_ref().filter(|f| f.field)?;
        self.coord
            .text_fields()
            .is_focused(&f.id)
            .then(|| f.id.clone())
    }

    fn key(
        &mut self,
        win: WindowId,
        input: KeyInput,
        next_ticket: &mut u64,
        fx: &mut InputEffects,
    ) -> bool {
        if input.state == KeyState::Up {
            fx.push(InputEffect::KeyPassed { win, key: input });
            return false;
        }
        let m = input.mods;
        if input.code == KeyCode::Tab && !(m.ctrl || m.alt || m.meta) {
            return match self.cycle(!m.shift) {
                Some(changed) => changed,
                None => {
                    fx.push(InputEffect::KeyPassed { win, key: input });
                    false
                }
            };
        }
        let field = self.text_focus();
        let edit = editing_chord(&input);
        let (field, edit) = match (field, edit) {
            (Some(f), Some(e)) => (f, e),
            // No focused text field: a plain-text selection takes the copy
            // / select-all chords; with nothing selected they route on.
            (None, Some(Edit::Copy)) => {
                if let Some(text) = self.selected_text_concat() {
                    fx.push(InputEffect::CopyText { win, text });
                } else {
                    fx.push(InputEffect::KeyPassed { win, key: input });
                }
                return false;
            }
            (None, Some(Edit::Key(KeyPress::SelectAll))) => {
                // `None` = no selectable context: the key routes on.
                return match self.ctrl_a(win, fx) {
                    Some(changed) => changed,
                    None => {
                        fx.push(InputEffect::KeyPassed { win, key: input });
                        false
                    }
                };
            }
            _ => {
                fx.push(InputEffect::KeyPassed { win, key: input });
                return false;
            }
        };
        let text_before = self.coord.text_fields().text(&field).to_owned();
        let changed_text = |action: &TextAction| matches!(action, TextAction::TextChanged(_));
        let mut changed = false;
        match edit {
            Edit::Key(kp) => changed = changed_text(&self.coord.on_key(kp)),
            Edit::Char(c) => changed = changed_text(&self.coord.on_char(c)),
            Edit::Text(s) => {
                for c in s.chars() {
                    changed |= changed_text(&self.coord.on_char(c));
                }
            }
            Edit::Submit => {
                if let TextAction::Commit(_) = self.coord.on_char('\r') {
                    fx.push(InputEffect::Text(TextIntent::Submitted {
                        win,
                        field: field.clone(),
                    }));
                }
            }
            Edit::Copy => {
                if !self.masked(&field) {
                    if let Some(text) = self.coord.text_fields().copy_selection() {
                        fx.push(InputEffect::CopyText { win, text });
                    }
                }
            }
            Edit::Cut => {
                if !self.masked(&field) {
                    if let Some(text) = self.coord.text_fields_mut().cut_selection(&field) {
                        fx.push(InputEffect::CopyText { win, text });
                    }
                }
            }
            Edit::Paste => {
                let ticket = Ticket(INPUT_TICKET_BASE.saturating_add(*next_ticket));
                *next_ticket = next_ticket.saturating_add(1);
                self.pending_paste = Some((ticket, field.clone()));
                fx.push(InputEffect::NeedPaste { win, ticket });
            }
        }
        if changed || self.coord.text_fields().text(&field) != text_before {
            fx.push(InputEffect::Text(TextIntent::Changed { win, field }));
        }
        // Text and caret changes are revisioned by `settle`.
        false
    }

    fn masked(&self, field: &WidgetId) -> bool {
        self.coord
            .text_fields()
            .field_state(field)
            .is_some_and(|s| s.config.masked)
    }

    fn ime(&mut self, win: WindowId, input: ImeInput, fx: &mut InputEffects) {
        let Some(field) = self.text_focus() else {
            return;
        };
        match input {
            ImeInput::Enabled => {}
            ImeInput::Preedit { text, cursor } => {
                let chars = |b: usize| text.get(..b).map(|s| s.chars().count());
                let at = cursor
                    .and_then(|(b, _)| chars(b))
                    .unwrap_or_else(|| text.chars().count());
                self.coord.text_fields_mut().preedit_set(&field, text, at);
            }
            ImeInput::Commit(text) => {
                let action = self.coord.text_fields_mut().ime_commit(&field, text);
                if let TextAction::TextChanged(_) = action {
                    fx.push(InputEffect::Text(TextIntent::Changed { win, field }));
                }
            }
            ImeInput::Disabled => {
                self.coord
                    .text_fields_mut()
                    .preedit_set(&field, String::new(), 0);
            }
        }
    }

    fn clipboard(&mut self, win: WindowId, result: ClipboardResult, fx: &mut InputEffects) {
        let Some((ticket, field)) = self.pending_paste.clone() else {
            return;
        };
        if ticket != result.ticket {
            return;
        }
        self.pending_paste = None;
        let (Some(text), Some(focused)) = (result.text, self.text_focus()) else {
            return;
        };
        if focused != field || text.is_empty() {
            return;
        }
        if let TextAction::TextChanged(_) = self.coord.on_key(KeyPress::Paste(text)) {
            fx.push(InputEffect::Text(TextIntent::Changed { win, field }));
        }
    }

    // -- caret ----------------------------------------------------------------

    fn caret_key(&self) -> Option<CaretKey> {
        let store = self.coord.text_fields();
        let field = store.focused()?;
        let s = store.field_state(field)?;
        Some(CaretKey {
            field: field.clone(),
            engaged: s.engaged,
            text: s.text.clone(),
            cursor: s.cursor,
            selection: s.selection_start,
            preedit: s.preedit.clone(),
            preedit_cursor: s.preedit_cursor,
        })
    }

    fn field_rect(&self, field: &WidgetId) -> Option<Rect> {
        let (x, y, w, h) = self.coord.text_fields().field_state(field)?.last_rect?;
        Some(Rect::new(x, y, w, h))
    }

    /// The caret's rect: the char boundary at the cursor, full field height.
    fn caret_rect(&self, field: &WidgetId) -> Option<Rect> {
        let s = self.coord.text_fields().field_state(field)?;
        let (x, y, _, h) = s.last_rect?;
        let cx = s.last_char_positions.get(s.cursor).copied().unwrap_or(x);
        Some(Rect::new(cx, y, 1.0, h))
    }

    /// Re-derive caret blink, repaint, IME area and IME-allowed state after
    /// any op. Returns whether observable state changed.
    fn settle(&mut self, win: WindowId, now: Seconds, fx: &mut InputEffects) -> bool {
        let now_ms = millis(now);
        let key = self.caret_key();
        let key_changed = key != self.caret;
        if key_changed {
            if let Some(old) = &self.caret {
                if key.as_ref().map(|k| &k.field) != Some(&old.field) {
                    fx.push(InputEffect::InvalidateField {
                        win,
                        rect: self.field_rect(&old.field),
                    });
                }
            }
            self.caret = key;
            self.blink_anchor_ms = now_ms;
        }
        self.coord
            .text_fields_mut()
            .set_blink_time(self.blink_anchor_ms);
        let active = self.os_focused && self.caret.as_ref().is_some_and(|k| k.engaged);
        let visible = active && self.coord.text_fields().cursor_visible(now_ms);
        let visible_changed = visible != self.caret_visible;
        self.caret_visible = visible;
        if key_changed || visible_changed {
            if let Some(k) = &self.caret {
                fx.push(InputEffect::InvalidateField {
                    win,
                    rect: self.field_rect(&k.field),
                });
            }
        }
        let at = active.then(|| next_blink_edge(self.blink_anchor_ms, now_ms));
        if at != self.armed {
            self.armed = at;
            fx.push(InputEffect::CaretDeadline { win, at });
        }
        let field = self.caret.as_ref().map(|k| k.field.clone());
        let allowed = field.is_some();
        if allowed != self.ime_allowed {
            self.ime_allowed = allowed;
            fx.push(InputEffect::ImeAllowed { win, allowed });
        }
        let area = field.and_then(|f| self.caret_rect(&f));
        if area != self.ime_area {
            self.ime_area = area;
            if let Some(rect) = area {
                fx.push(InputEffect::ImeArea { win, rect });
            }
        }
        key_changed || visible_changed
    }
}

/// Host clock in whole milliseconds (the store's blink unit); negative and
/// NaN instants count as 0.
fn millis(t: Seconds) -> u64 {
    // `as` saturates: NaN -> 0, beyond u64::MAX -> u64::MAX.
    (t.get() * 1000.0).round().max(0.0) as u64
}

/// The first blink edge strictly after `now_ms` for a phase anchored at
/// `anchor_ms`.
fn next_blink_edge(anchor_ms: u64, now_ms: u64) -> Seconds {
    let phases = now_ms.saturating_sub(anchor_ms) / CARET_BLINK_MS + 1;
    let edge = anchor_ms.saturating_add(phases.saturating_mul(CARET_BLINK_MS));
    Seconds(edge as f64 / 1000.0)
}

/// What a key does to a focused text field.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Edit {
    Key(KeyPress),
    Char(char),
    Text(String),
    Submit,
    Copy,
    Cut,
    Paste,
}

/// The editing chord a key press is for a focused text field, or `None`
/// when the field does not take it (it routes on to overlays and keymap).
fn editing_chord(input: &KeyInput) -> Option<Edit> {
    let m = input.mods;
    let cmd = m.ctrl || m.meta;
    let edit = match input.code {
        KeyCode::A if cmd => Edit::Key(KeyPress::SelectAll),
        KeyCode::C if cmd => Edit::Copy,
        KeyCode::X if cmd => Edit::Cut,
        KeyCode::V if cmd => Edit::Paste,
        KeyCode::ArrowLeft if cmd => Edit::Key(KeyPress::WordLeft),
        KeyCode::ArrowRight if cmd => Edit::Key(KeyPress::WordRight),
        KeyCode::Backspace if cmd => Edit::Key(KeyPress::DeleteWordBack),
        KeyCode::Delete if cmd => Edit::Key(KeyPress::DeleteWordForward),
        KeyCode::ArrowLeft if m.shift => Edit::Key(KeyPress::ShiftLeft),
        KeyCode::ArrowRight if m.shift => Edit::Key(KeyPress::ShiftRight),
        KeyCode::Home if m.shift => Edit::Key(KeyPress::ShiftHome),
        KeyCode::End if m.shift => Edit::Key(KeyPress::ShiftEnd),
        KeyCode::ArrowLeft => Edit::Key(KeyPress::ArrowLeft),
        KeyCode::ArrowRight => Edit::Key(KeyPress::ArrowRight),
        KeyCode::Home => Edit::Key(KeyPress::Home),
        KeyCode::End => Edit::Key(KeyPress::End),
        KeyCode::Backspace => Edit::Char('\x08'),
        KeyCode::Delete => Edit::Key(KeyPress::Delete),
        KeyCode::Enter => Edit::Submit,
        // Other Ctrl / Cmd chords belong to the keymap; Ctrl+Alt is AltGr
        // on some layouts and does produce text.
        _ if cmd && !(m.ctrl && m.alt) => return None,
        _ => {
            let text: String = input
                .text
                .as_deref()?
                .chars()
                .filter(|c| !c.is_control())
                .collect();
            if text.is_empty() {
                return None;
            }
            match text.chars().count() {
                1 => Edit::Char(text.chars().next()?),
                _ => Edit::Text(text),
            }
        }
    };
    Some(edit)
}

// ---------------------------------------------------------------------------
// InputEngine
// ---------------------------------------------------------------------------

/// The InputEngine: see the module docs.
#[derive(Default)]
pub struct InputEngine {
    windows: BTreeMap<WindowId, PerWindowInput>,
    next_scope: u64,
    next_ticket: u64,
    rev: Revision,
}

impl fmt::Debug for InputEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InputEngine")
            .field("windows", &self.windows.keys().collect::<Vec<_>>())
            .field("rev", &self.rev)
            .finish()
    }
}

impl InputEngine {
    /// An engine with no windows.
    pub fn new() -> Self {
        Self::default()
    }

    /// The only mutation door. Bumps the revision iff observable state
    /// changed (see the module docs).
    pub fn apply(&mut self, op: InputOp) -> InputEffects {
        let mut fx = InputEffects::new();
        let Self {
            windows,
            next_scope,
            next_ticket,
            rev,
        } = self;
        let changed = match op {
            InputOp::Open(win) => match windows.entry(win) {
                std::collections::btree_map::Entry::Occupied(_) => false,
                std::collections::btree_map::Entry::Vacant(slot) => {
                    slot.insert(PerWindowInput::new());
                    true
                }
            },
            InputOp::Close(win) => windows.remove(&win).is_some(),
            InputOp::WindowFocus { win, focused, now } => match windows.get_mut(&win) {
                Some(w) => {
                    w.os_focused = focused;
                    w.settle(win, now, &mut fx)
                }
                None => false,
            },
            InputOp::Pointer { win, now, input } => match windows.get_mut(&win) {
                Some(w) => {
                    let (changed, cooked) = w.cook(win, now, input);
                    fx.push(InputEffect::Pointer(cooked));
                    changed
                }
                None => false,
            },
            InputOp::Deliver { win, input } => match windows.get_mut(&win) {
                Some(w) => {
                    let (changed, target) = w.deliver(win, input, &mut fx);
                    fx.push(InputEffect::Content { win, target });
                    changed
                }
                None => false,
            },
            InputOp::Wheel { win, input } => {
                if let Some(w) = windows.get_mut(&win) {
                    let target = w.wheel(input);
                    fx.push(InputEffect::Content { win, target });
                }
                false
            }
            InputOp::Key { win, now, input } => match windows.get_mut(&win) {
                Some(w) => {
                    let changed = w.key(win, input, next_ticket, &mut fx);
                    w.settle(win, now, &mut fx) | changed
                }
                None => false,
            },
            InputOp::Ime { win, now, input } => match windows.get_mut(&win) {
                Some(w) => {
                    w.ime(win, input, &mut fx);
                    w.settle(win, now, &mut fx)
                }
                None => false,
            },
            InputOp::Clipboard { win, now, result } => match windows.get_mut(&win) {
                Some(w) => {
                    w.clipboard(win, result, &mut fx);
                    w.settle(win, now, &mut fx)
                }
                None => false,
            },
            InputOp::Focus { win, now, op } => match windows.get_mut(&win) {
                Some(w) => {
                    let changed = w.focus_op(win, op, next_scope, &mut fx);
                    w.settle(win, now, &mut fx) | changed
                }
                None => false,
            },
            InputOp::SetCapture { win, capture } => windows
                .get_mut(&win)
                .is_some_and(|w| w.set_capture(capture)),
            InputOp::BeginFrame { win, now } => {
                windows.get_mut(&win).is_some_and(|w| w.begin_frame(now))
            }
            InputOp::EndFrame { win, now } => match windows.get_mut(&win) {
                Some(w) => {
                    let changed = w.end_frame(win, &mut fx);
                    w.settle(win, now, &mut fx) | changed
                }
                None => false,
            },
        };
        if changed {
            rev.bump();
        }
        fx
    }

    /// Caret blink edges: every window whose armed edge is due at `now`
    /// flips its caret, asks for a repaint of that field and re-arms the
    /// next edge. The kernel calls it when `TimerOwner::CaretBlink` fires
    /// (calling it at any other time is harmless).
    pub fn tick(&mut self, now: Seconds) -> InputEffects {
        let mut fx = InputEffects::new();
        let mut changed = false;
        for (win, w) in &mut self.windows {
            if w.armed.is_some_and(|at| at.get() <= now.get()) {
                changed |= w.settle(*win, now, &mut fx);
            }
        }
        if changed {
            self.rev.bump();
        }
        fx
    }

    /// The compose-phase door for widget registration (`register*`,
    /// `register_text_field`, `update_field`, `mark_seen`, `unregister`,
    /// layers). Not revisioned: what registration changes is evaluated by
    /// the following [`InputOp::EndFrame`]. Lib brief L6 narrows what this
    /// reference can reach (see the module docs).
    pub fn registrar(&mut self, win: WindowId) -> Option<&mut InputCoordinator> {
        self.windows.get_mut(&win).map(|w| &mut w.coord)
    }

    /// The compose-phase door for the per-widget typed state store (the
    /// `Widgets` face of the app hook contexts). Not revisioned: the store
    /// holds no on-screen truth of its own.
    pub fn states_mut(&mut self, win: WindowId) -> Option<&mut uzor::app_context::StateRegistry> {
        self.windows.get_mut(&win).map(|w| &mut w.states)
    }

    /// Compose-phase door (T2): report one selectable plain-text widget's
    /// content and line geometry for this frame — the input the selection
    /// owner resolves presses / drags against, mirroring
    /// `TextFieldStore::update_field`. A widget not reported in a frame
    /// stops being selectable and loses its selection. Not revisioned: the
    /// change is evaluated by the following [`InputOp::EndFrame`].
    pub fn update_selectable(
        &mut self,
        win: WindowId,
        widget: WidgetId,
        text: impl Into<String>,
        lines: Vec<SelectionLine>,
    ) {
        if let Some(w) = self.windows.get_mut(&win) {
            w.reported.insert(
                widget,
                SelectableText {
                    text: text.into(),
                    lines,
                },
            );
        }
    }

    /// Bumped on every observable state change.
    pub fn revision(&self) -> Revision {
        self.rev
    }

    /// Read-only projection.
    pub fn view(&self) -> InputEngineView<'_> {
        InputEngineView { e: self }
    }
}

/// Read-only projection of the [`InputEngine`].
#[derive(Clone, Copy, Debug)]
pub struct InputEngineView<'a> {
    e: &'a InputEngine,
}

impl<'a> InputEngineView<'a> {
    fn w(&self, win: WindowId) -> Option<&'a PerWindowInput> {
        self.e.windows.get(&win)
    }

    /// Tracked windows, ascending.
    pub fn ids(&self) -> impl Iterator<Item = WindowId> + 'a {
        self.e.windows.keys().copied()
    }

    /// The published input state of one window.
    pub fn window(&self, win: WindowId) -> Option<InputView> {
        self.w(win).map(|w| InputView {
            hovered: w.hover.clone(),
            pressed: w.pressed.clone(),
            focused: w.focused_id().cloned(),
            captured: w.capture.is_some(),
            scope_depth: w.scopes.len(),
        })
    }

    /// The widget with keyboard focus.
    pub fn focused(&self, win: WindowId) -> Option<&'a WidgetId> {
        self.w(win)?.focused_id()
    }

    /// The owner of the active (innermost) focus scope.
    pub fn active_scope(&self, win: WindowId) -> Option<&'a ScopeOwner> {
        self.w(win)?.scopes.last().map(|s| &s.owner)
    }

    /// The pointer capture.
    pub fn capture(&self, win: WindowId) -> Option<&'a Capture> {
        self.w(win)?.capture.as_ref()
    }

    /// The event-level cook state (click count, drag arm).
    pub fn cook(&self, win: WindowId) -> Option<&'a CookState> {
        self.w(win).map(|w| &w.cook)
    }

    /// Widgets clicked in the last evaluated frame, with their click count
    /// (1 single, 2 double, 3 triple).
    pub fn clicked(&self, win: WindowId) -> &'a [(WidgetId, u8)] {
        self.w(win).map_or(&[], |w| w.clicked.as_slice())
    }

    /// The caret of the focused field is in its visible phase (and blinking
    /// is active).
    pub fn caret_visible(&self, win: WindowId) -> bool {
        self.w(win).is_some_and(|w| w.caret_visible)
    }

    /// The armed caret blink edge.
    pub fn caret_deadline(&self, win: WindowId) -> Option<Seconds> {
        self.w(win)?.armed
    }

    /// The window's text fields (text, cursor, selection, preedit).
    pub fn text_fields(&self, win: WindowId) -> Option<&'a TextFieldStore> {
        self.w(win).map(|w| w.coord.text_fields())
    }

    /// The window's plain-text selections (widget → selection), the T2
    /// per-window owner state. The app reads this at compose time to draw
    /// highlights through the lib's `draw_text_with_selection`.
    pub fn selections(&self, win: WindowId) -> Option<&'a [(WidgetId, TextSelection)]> {
        self.w(win).map(|w| w.selections.as_slice())
    }

    /// The window's coordinator, read-only (hit tests, widget state).
    pub fn coordinator(&self, win: WindowId) -> Option<&'a InputCoordinator> {
        self.w(win).map(|w| &w.coord)
    }

    /// Scroll accumulated since the last `BeginFrame`, in lines. The
    /// compose phase reads it into the hook view before `BeginFrame`
    /// moves it into the coordinator's frame input.
    pub fn pending_scroll(&self, win: WindowId) -> (f64, f64) {
        self.w(win).map_or((0.0, 0.0), |w| w.pointer.scroll)
    }
}

#[cfg(test)]
mod tests;
