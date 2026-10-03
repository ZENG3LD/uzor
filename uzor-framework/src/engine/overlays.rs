//! OverlayEngine: per-window overlay stacks, their policies and the
//! composite widget states inside them (design §3.3).
//!
//! The one writer of what is open on top of a window's content. Z order,
//! the modal shield, outside / Escape dismiss and the focus / keymap scope
//! lifetimes are decided here and nowhere else; the kernel only conducts the
//! effects (§3.8) and never re-derives any of it.
//!
//! ## Z order (the one rule)
//!
//! Each window's stack is kept sorted bottom → top by one key:
//!
//! 1. **modal band** — a modal starts a new band above everything open when
//!    it opened; an overlay opened while `k` modals are open joins band `k`.
//!    So whatever a modal opens (its dropdowns, context menus, tooltips) is
//!    above it, and whatever was open before it is below it and shielded;
//! 2. inside a band the band's modal is the base, then
//!    [`ZLayerTable::z_for`] of the kind (the library table);
//! 3. then insertion order (a re-open counts as a new insertion).
//!
//! Callers cannot place an overlay: there is no z argument, no "bring to
//! front", no index. The only questions they can ask are
//! [`OverlayEngine::hit`] / [`OverlayEngine::topmost_at`] and the verdicts
//! of [`OverlayOp::Intercept`], which share one private walk. Closing a
//! modal first closes everything above it (newest first), so bands stay
//! contiguous and a modal's children never outlive it.
//!
//! Tooltips are pointer- and key-transparent: they are drawn in the stack
//! but are never hit, never "the top overlay", open no focus scope.
//!
//! ## Pointer (routing step 3, design §4.3)
//!
//! Every [`Intercepted::Pointer`] / [`Intercepted::Wheel`] gets exactly one
//! [`OverlayEffect::Pointer`] verdict ([`PointerRoute`]):
//!
//! - **Modal shield.** Walking top → bottom, the first overlay whose rect
//!   contains the point takes it ([`PointerRoute::Overlay`]); reaching a
//!   modal that does not contain it stops the walk with
//!   [`PointerRoute::Consumed`] — nothing below the modal (lower overlays,
//!   dock, chrome, splitters, content) sees it.
//! - **Outside dismiss.** Only the TOP overlay is considered. A press
//!   outside it, when its policy has `dismiss_on_outside`, is consumed and
//!   remembered; the release of that same press, still outside and with the
//!   same overlay on top, closes it ([`CloseCause::Outside`]) and is
//!   consumed. The release of any consumed press is consumed.
//! - **Press origin rule.** A press that started inside an overlay never
//!   dismisses it, wherever it is released; its release is routed to that
//!   overlay (so dragging out of a menu and letting go is harmless). A press
//!   outside that is released inside does not dismiss either.
//! - Any button counts (a right press outside a context menu closes it
//!   like a left one).
//! - Moves and wheel steps follow the hit walk. `Entered`, `Left`,
//!   `Cancelled` and `RawDelta` carry no position and pass (`Cancelled`
//!   forgets the press).
//!
//! ## Keys (routing key step 2)
//!
//! Every [`Intercepted::Key`] gets exactly one [`OverlayEffect::Key`]
//! verdict ([`KeyRoute`]). An Escape press walks top → bottom: the first
//! overlay with `dismiss_on_escape` closes ([`CloseCause::Escape`]) and the
//! key is consumed; a modal without it stops the walk. Otherwise the key
//! passes with the ONLY values the kernel may feed to the keymap: the top
//! overlay (when it opened a keymap scope) and whether a modal is open
//! (global bindings then resolve only with `through_modal`).
//!
//! ## Scopes, deadlines, fades
//!
//! [`OverlayEffect::Opened`] carries the focus scope to push
//! (`ScopeOwner::Overlay(slot)`), the modal flag, the keymap-scope flag and
//! the absolute auto-close deadline; [`OverlayEffect::Closed`] carries the
//! same scope, the cause and `restore_focus`. Several closes of one op
//! (a modal and what it opened, a closed window) are reported newest first,
//! so scopes pop in reverse order of opening. The
//! kernel arms the deadline on the cadence wheel and feeds the fired owner
//! back as [`OverlayOp::Fire`] (stale slots are ignored); it drives
//! `AnimKey::OverlayFade` from `Opened` / `Closed` (no separate effect).
//!
//! ## Bodies and clicks
//!
//! Modal / popup / dropdown / context-menu overlays hold the library
//! composite state; the engine is its only writer. Each instance has a host
//! widget id ([`OverlayEntryView::host_id`], `overlay-{slot}`) that compose
//! uses for the body's widgets, and the window's
//! [`ClickDispatcher`] is rebuilt from the stack with the library's
//! `register_*_dispatch` fns on every stack change. [`OverlayOp::Click`]
//! resolves a clicked widget through it: close button / footer →
//! `CloseCause::Item`; modal tab → [`OverlayIntent::ModalTab`]; dropdown
//! item → [`OverlayIntent::DropdownItem`] + close; context-menu item →
//! [`OverlayIntent::ContextMenuItem`] + close; body parts (chevrons, body
//! scrollbar, resize handles, submenu toggle) → the library
//! `consume_event`. Colour pickers are [`OverlayBody::Custom`] (no library
//! composite state); modal wizard next / back are not handled here (the
//! page count lives in the app's model).
//!
//! ## Geometry
//!
//! `Open` places the overlay below its anchor (left-aligned), or centred in
//! the known viewport without one; `OverlaySize::Auto` starts empty until
//! [`OverlayOp::Resize`] reports the measured size. [`OverlayOp::Reclamp`]
//! (after every solve) stores the viewport and clamps every rect inside it
//! with the library's `clamp_to_viewport`; opens, moves and resizes clamp
//! too.
//!
//! ## Revision
//!
//! Bumped exactly when observable state changes: an overlay opens, closes
//! or is replaced, a rect or the viewport changes, or a body state changes.
//! Routing verdicts that change nothing and the remembered press origin are
//! not revisioned.

use std::collections::BTreeMap;
use std::fmt;

use smallvec::SmallVec;
use uzor::input::KeyCode;
use uzor::layout::{
    ClickDispatcher, ContextMenuHandle, DispatchEvent, DropdownHandle, ModalHandle, OverlayKind,
    OverlayStack, PopupHandle, ZLayerTable,
};
use uzor::render::InvalidateBits;
use uzor::ui::widgets::composite::context_menu::input::register_context_menu_dispatch;
use uzor::ui::widgets::composite::context_menu::ContextMenuState;
use uzor::ui::widgets::composite::dropdown::input::{
    self as dropdown_input, register_dropdown_dispatch,
};
use uzor::ui::widgets::composite::dropdown::DropdownState;
use uzor::ui::widgets::composite::modal::input::{self as modal_input, register_modal_dispatch};
use uzor::ui::widgets::composite::modal::ModalState;
use uzor::ui::widgets::composite::popup::input::{self as popup_input, register_popup_dispatch};
use uzor::ui::widgets::composite::popup::PopupState;
use uzor::{Rect, WidgetId};

use crate::types::bus::{KeyInput, KeyState, PointerInput};
use crate::types::command::{OverlayCmd, OverlayPolicy, OverlaySize};
use crate::types::ids::{OverlaySlot, Revision, Seconds, WindowId};
use crate::types::intent::{CloseCause, OverlayIntent};
use crate::types::ops::{
    Intercepted, KeyRoute, OverlayEffect, OverlayOp, PointerRoute, ScopeOwner,
};
use crate::types::snapshot::OverlayView;
use crate::types::window::Point;

/// Effects of one OverlayEngine op.
pub type OverlayEffects<O> = SmallVec<[OverlayEffect<O>; 2]>;

/// The state inside one open overlay: the library composite state for the
/// kinds that have one.
#[derive(Clone, Debug)]
pub enum OverlayBody {
    /// A modal dialog.
    Modal(ModalState),
    /// A popup panel.
    Popup(PopupState),
    /// A dropdown menu.
    Dropdown(DropdownState),
    /// A context menu.
    ContextMenu(ContextMenuState),
    /// A tooltip (no state).
    Tooltip,
    /// A kind without a library composite state (colour picker).
    Custom,
}

impl OverlayBody {
    fn for_kind(kind: OverlayKind, rect: Rect, anchor: Option<Rect>) -> Self {
        match kind {
            OverlayKind::Modal => Self::Modal(ModalState {
                position: (rect.x, rect.y),
                ..ModalState::default()
            }),
            OverlayKind::Popup => {
                let mut s = PopupState::default();
                s.open_at((rect.x, rect.y));
                Self::Popup(s)
            }
            OverlayKind::Dropdown => {
                let mut s = DropdownState::default();
                match anchor {
                    Some(a) => s.open_below(a, 0.0),
                    None => s.open_at(rect.x, rect.y),
                }
                Self::Dropdown(s)
            }
            OverlayKind::ContextMenu => {
                let mut s = ContextMenuState::default();
                s.open_raw(rect.x, rect.y, None);
                Self::ContextMenu(s)
            }
            OverlayKind::Tooltip => Self::Tooltip,
            OverlayKind::ColorPicker => Self::Custom,
        }
    }

    /// Keep the composite's own origin equal to the engine rect's.
    fn set_origin(&mut self, x: f64, y: f64) {
        match self {
            Self::Modal(s) => s.position = (x, y),
            Self::Popup(s) => s.position = (x, y),
            Self::Dropdown(s) => {
                s.origin = (x, y);
                s.open_position_override = Some((x, y));
            }
            Self::ContextMenu(s) => {
                s.x = x;
                s.y = y;
            }
            Self::Tooltip | Self::Custom => {}
        }
    }
}

/// The sort key of an entry (see the module docs, "Z order"). Field order
/// is the comparison order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ZKey {
    band: u32,
    /// `false` for the band's modal base, `true` for everything above it.
    raised: bool,
    z: i32,
    seq: u64,
}

/// One open overlay instance.
#[derive(Clone, Debug)]
// Fields surface through the OverlayEntryView doors below.
#[allow(dead_code)]
struct Entry<O> {
    id: O,
    slot: OverlaySlot,
    kind: OverlayKind,
    key: ZKey,
    rect: Rect,
    anchor: Option<Rect>,
    size: OverlaySize,
    policy: OverlayPolicy,
    body: OverlayBody,
    host: WidgetId,
    opened: Seconds,
}

impl<O> Entry<O> {
    /// Never hit, never the top overlay, no focus scope.
    fn transparent(&self) -> bool {
        self.kind == OverlayKind::Tooltip
    }

    fn scope(&self) -> Option<ScopeOwner> {
        (!self.transparent()).then_some(ScopeOwner::Overlay(self.slot))
    }

    fn contains(&self, p: Point) -> bool {
        self.rect.contains(p.x, p.y)
    }

    /// The clicked widget is one of this overlay's parts (`{host}:...`).
    fn owns(&self, widget: &WidgetId) -> bool {
        widget
            .as_str()
            .strip_prefix(self.host.as_str())
            .is_some_and(|rest| rest.starts_with(':'))
    }
}

/// Where the current press of a window started, relative to its overlays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Press {
    /// Outside the top overlay, which had `dismiss_on_outside`.
    Outside(OverlaySlot),
    /// Inside this overlay.
    Inside(OverlaySlot),
}

/// One window's stack.
#[derive(Clone)]
struct Stack<O> {
    /// Sorted bottom → top by `ZKey`.
    entries: Vec<Entry<O>>,
    viewport: Option<Rect>,
    dispatch: ClickDispatcher,
    press: Option<Press>,
}

impl<O> Default for Stack<O> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            viewport: None,
            dispatch: ClickDispatcher::new(),
            press: None,
        }
    }
}

impl<O: fmt::Debug> fmt::Debug for Stack<O> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Stack")
            .field("entries", &self.entries)
            .field("viewport", &self.viewport)
            .field("press", &self.press)
            .finish_non_exhaustive()
    }
}

/// What is under a point of a window, after Z order and the modal shield.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayHit<O> {
    /// This overlay is the topmost one containing the point.
    Overlay {
        /// App identity.
        id: O,
        /// The instance.
        slot: OverlaySlot,
        /// Its kind.
        kind: OverlayKind,
    },
    /// No overlay above it contains the point and this modal does not
    /// either: nothing below it may take the point.
    Shielded {
        /// The shielding modal's identity.
        id: O,
        /// The instance.
        slot: OverlaySlot,
    },
    /// No overlay claims the point (layout zones and content may).
    None,
}

impl<O> OverlayHit<O> {
    fn route(self) -> PointerRoute<O> {
        match self {
            Self::Overlay { id, slot, .. } => PointerRoute::Overlay { id, slot },
            Self::Shielded { .. } => PointerRoute::Consumed,
            Self::None => PointerRoute::Pass,
        }
    }
}

/// The OverlayEngine: see the module docs. `O` is the app's overlay
/// identity ([`Spec::Overlay`](crate::Spec::Overlay)).
#[derive(Clone, Debug)]
pub struct OverlayEngine<O> {
    windows: BTreeMap<WindowId, Stack<O>>,
    z: ZLayerTable,
    next_slot: u64,
    next_seq: u64,
    rev: Revision,
}

impl<O> Default for OverlayEngine<O> {
    fn default() -> Self {
        Self::with_z(ZLayerTable::default())
    }
}

impl<O> OverlayEngine<O> {
    /// An engine with no overlays and the library's default z table.
    pub fn new() -> Self {
        Self::default()
    }

    /// An engine with no overlays and a custom z table (fixed for the
    /// engine's life, so the order of open overlays never changes under
    /// them).
    pub fn with_z(z: ZLayerTable) -> Self {
        Self {
            windows: BTreeMap::new(),
            z,
            next_slot: 1,
            next_seq: 0,
            rev: Revision::ZERO,
        }
    }
}

impl<O: Copy + Eq> OverlayEngine<O> {
    /// The only mutation door. Bumps the revision iff state changed.
    pub fn apply(&mut self, op: OverlayOp<O>) -> OverlayEffects<O> {
        let mut fx = OverlayEffects::new();
        let changed = match op {
            OverlayOp::Cmd { cmd, now } => self.cmd(cmd, now, &mut fx),
            OverlayOp::Resize {
                win,
                id,
                width,
                height,
            } => self.resize(win, id, width, height, &mut fx),
            OverlayOp::Intercept { win, event } => self.intercept(win, event, &mut fx),
            OverlayOp::Click {
                win,
                widget,
                cursor,
            } => self.click(win, &widget, cursor, &mut fx),
            OverlayOp::Captured { win, slot, input } => self.captured(win, slot, input, &mut fx),
            OverlayOp::Reclamp { win, viewport } => self.reclamp(win, viewport, &mut fx),
            OverlayOp::Fire { win, slot } => self.windows.get_mut(&win).is_some_and(|s| {
                match s.entries.iter().position(|e| e.slot == slot) {
                    Some(i) => {
                        close_at(win, s, i, CloseCause::Timer, &mut fx);
                        true
                    }
                    None => false,
                }
            }),
            OverlayOp::CloseWindow(win) => match self.windows.remove(&win) {
                Some(mut s) => {
                    let had = !s.entries.is_empty();
                    s.entries.sort_by_key(|e| std::cmp::Reverse(e.key.seq));
                    for e in &s.entries {
                        fx.push(closed(win, e, CloseCause::WindowClosed));
                    }
                    had || s.viewport.is_some()
                }
                None => false,
            },
        };
        if changed {
            self.rev.bump();
        }
        fx
    }

    /// What is under `p` in `win` after Z order and the modal shield. The
    /// same walk decides every pointer verdict; routing never re-derives Z.
    // Read doors for the native/web host briefs (F8+) - unused in-crate until then.
    #[allow(dead_code)]
    pub fn hit(&self, win: WindowId, p: Point) -> OverlayHit<O> {
        self.windows
            .get(&win)
            .map_or(OverlayHit::None, |s| hit_walk(s, p))
    }

    /// The topmost overlay that takes `p` (the [`OverlayHit::Overlay`] case
    /// of [`Self::hit`]); `None` when the point is shielded or free.
    // Read doors for the native/web host briefs (F8+) - unused in-crate until then.
    #[allow(dead_code)]
    pub fn topmost_at(&self, win: WindowId, p: Point) -> Option<OverlayEntryView<'_, O>> {
        let s = self.windows.get(&win)?;
        match hit_walk(s, p) {
            OverlayHit::Overlay { slot, .. } => s
                .entries
                .iter()
                .find(|e| e.slot == slot)
                .map(|e| OverlayEntryView { e }),
            _ => None,
        }
    }

    /// The compose-phase door for one entry's composite state: the
    /// library's registration fns take `&mut` (e.g. a draggable modal
    /// resolves its moved frame). Not revisioned — what registration
    /// changes is evaluated by the input engine's following `EndFrame`,
    /// and a body state change surfaces through the composite's own
    /// `consume_event` path (which reports effects).
    // Read doors for the native/web host briefs (F8+) - unused in-crate until then.
    #[allow(dead_code)]
    pub fn body_state_mut(&mut self, win: WindowId, slot: OverlaySlot) -> Option<&mut OverlayBody> {
        let s = self.windows.get_mut(&win)?;
        s.entries
            .iter_mut()
            .find(|e| e.slot == slot)
            .map(|e| &mut e.body)
    }

    /// Bumped on every observable state change.
    pub fn revision(&self) -> Revision {
        self.rev
    }

    /// Read-only projection.
    pub fn view(&self) -> OverlayEngineView<'_, O> {
        OverlayEngineView { e: self }
    }

    fn cmd(&mut self, cmd: OverlayCmd<O>, now: Seconds, fx: &mut OverlayEffects<O>) -> bool {
        match cmd {
            OverlayCmd::Open {
                win,
                id,
                kind,
                anchor,
                size,
                policy,
            } => {
                self.open(win, id, kind, anchor, size, policy, now, fx);
                true
            }
            OverlayCmd::Close { win, id } => self.close_id(win, id, CloseCause::Command, fx),
            OverlayCmd::CloseTop { win } => {
                self.windows
                    .get_mut(&win)
                    .is_some_and(|s| match top_index(s) {
                        Some(i) => {
                            close_at(win, s, i, CloseCause::Command, fx);
                            true
                        }
                        None => false,
                    })
            }
            OverlayCmd::Toggle {
                win,
                id,
                kind,
                anchor,
                size,
                policy,
            } => {
                if !self.close_id(win, id, CloseCause::Command, fx) {
                    self.open(win, id, kind, anchor, size, policy, now, fx);
                }
                true
            }
            OverlayCmd::Move { win, id, to } => {
                let Some(s) = self.windows.get_mut(&win) else {
                    return false;
                };
                let viewport = s.viewport;
                let Some(e) = s.entries.iter_mut().find(|e| e.id == id) else {
                    return false;
                };
                let moved = Rect::new(to.x, to.y, e.rect.width, e.rect.height);
                set_rect(win, e, moved, viewport, fx)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn open(
        &mut self,
        win: WindowId,
        id: O,
        kind: OverlayKind,
        anchor: Option<Rect>,
        size: OverlaySize,
        policy: Option<OverlayPolicy>,
        now: Seconds,
        fx: &mut OverlayEffects<O>,
    ) {
        let policy = policy.unwrap_or(OverlayPolicy::for_kind(kind));
        let s = self.windows.entry(win).or_default();
        if let Some(i) = s.entries.iter().position(|e| e.id == id) {
            close_at(win, s, i, CloseCause::Replaced, fx);
        }
        let modals = s.entries.iter().filter(|e| e.policy.modal).count() as u32;
        let (band, raised) = if policy.modal {
            (modals + 1, false)
        } else {
            (modals, true)
        };
        let key = ZKey {
            band,
            raised,
            z: self.z.z_for(kind),
            seq: self.next_seq,
        };
        self.next_seq += 1;
        let slot = OverlaySlot(self.next_slot);
        self.next_slot += 1;
        let rect = place(anchor, size, s.viewport);
        let entry = Entry {
            id,
            slot,
            kind,
            key,
            rect,
            anchor,
            size,
            policy,
            body: OverlayBody::for_kind(kind, rect, anchor),
            host: WidgetId::from(format!("overlay-{}", slot.0)),
            opened: now,
        };
        fx.push(OverlayEffect::Opened {
            win,
            id,
            slot,
            kind,
            scope: entry.scope(),
            modal: policy.modal,
            keymap_scope: policy.keymap_scope,
            auto_close_at: policy.auto_close_after.map(|d| now.after(d)),
        });
        let at = s.entries.partition_point(|e| e.key < key);
        s.entries.insert(at, entry);
        rebuild_dispatch(s);
    }

    fn close_id(
        &mut self,
        win: WindowId,
        id: O,
        cause: CloseCause,
        fx: &mut OverlayEffects<O>,
    ) -> bool {
        let Some(s) = self.windows.get_mut(&win) else {
            return false;
        };
        match s.entries.iter().position(|e| e.id == id) {
            Some(i) => {
                close_at(win, s, i, cause, fx);
                true
            }
            None => false,
        }
    }

    fn resize(
        &mut self,
        win: WindowId,
        id: O,
        width: f64,
        height: f64,
        fx: &mut OverlayEffects<O>,
    ) -> bool {
        let Some(s) = self.windows.get_mut(&win) else {
            return false;
        };
        let viewport = s.viewport;
        let Some(e) = s.entries.iter_mut().find(|e| e.id == id) else {
            return false;
        };
        let r = Rect::new(e.rect.x, e.rect.y, finite(width), finite(height));
        set_rect(win, e, r, viewport, fx)
    }

    fn reclamp(&mut self, win: WindowId, viewport: Rect, fx: &mut OverlayEffects<O>) -> bool {
        let s = self.windows.entry(win).or_default();
        let mut changed = s.viewport != Some(viewport);
        s.viewport = Some(viewport);
        let mut geometry = false;
        for e in &mut s.entries {
            let r = OverlayStack::clamp_to_viewport(e.rect, viewport);
            if r != e.rect {
                e.rect = r;
                e.body.set_origin(r.x, r.y);
                geometry = true;
            }
        }
        if geometry {
            changed = true;
            fx.push(OverlayEffect::Invalidate {
                win,
                bits: InvalidateBits::GEOMETRY,
            });
        }
        changed
    }

    fn intercept(&mut self, win: WindowId, event: Intercepted, fx: &mut OverlayEffects<O>) -> bool {
        let Some(s) = self.windows.get_mut(&win) else {
            match event {
                Intercepted::Key(_) => fx.push(OverlayEffect::Key {
                    win,
                    route: KeyRoute::Pass {
                        top_overlay: None,
                        modal_open: false,
                    },
                }),
                _ => fx.push(OverlayEffect::Pointer {
                    win,
                    route: PointerRoute::Pass,
                }),
            }
            return false;
        };
        match event {
            Intercepted::Pointer(input) => {
                let (route, changed) = pointer(win, s, input, fx);
                fx.push(OverlayEffect::Pointer { win, route });
                changed
            }
            Intercepted::Wheel(w) => {
                let route = hit_walk(s, w.pos).route();
                fx.push(OverlayEffect::Pointer { win, route });
                false
            }
            Intercepted::Key(k) => {
                let changed = escape(win, s, &k, fx);
                let route = if changed {
                    KeyRoute::Consumed
                } else {
                    key_pass(s)
                };
                fx.push(OverlayEffect::Key { win, route });
                changed
            }
        }
    }

    fn click(
        &mut self,
        win: WindowId,
        widget: &WidgetId,
        cursor: Point,
        fx: &mut OverlayEffects<O>,
    ) -> bool {
        let Some(s) = self.windows.get_mut(&win) else {
            return false;
        };
        let Some(ev) = s.dispatch.dispatch(widget) else {
            return false;
        };
        let Some(i) = s.entries.iter().position(|e| e.owns(widget)) else {
            return false;
        };
        let viewport = s.viewport;
        let e = &mut s.entries[i];
        let (id, slot) = (e.id, e.slot);
        match ev {
            DispatchEvent::ModalCloseRequested(_) => {
                close_at(win, s, i, CloseCause::Item, fx);
                true
            }
            DispatchEvent::ModalTabClicked { index, .. } => {
                fx.push(OverlayEffect::Intent(OverlayIntent::ModalTab {
                    win,
                    id,
                    index,
                }));
                match &mut e.body {
                    OverlayBody::Modal(m) if m.active_tab != index => {
                        m.set_active_tab(index);
                        fx.push(OverlayEffect::Invalidate {
                            win,
                            bits: InvalidateBits::MATERIAL,
                        });
                        true
                    }
                    _ => false,
                }
            }
            DispatchEvent::DropdownItemClicked { item_id, .. } => {
                if let OverlayBody::Dropdown(d) = &mut e.body {
                    d.select(item_id.clone());
                }
                fx.push(OverlayEffect::Intent(OverlayIntent::DropdownItem {
                    win,
                    id,
                    item: WidgetId::from(item_id),
                }));
                close_at(win, s, i, CloseCause::Item, fx);
                true
            }
            DispatchEvent::ContextMenuItemClicked { item_index, .. } => {
                fx.push(OverlayEffect::Intent(OverlayIntent::ContextMenuItem {
                    win,
                    id,
                    index: item_index,
                }));
                close_at(win, s, i, CloseCause::Item, fx);
                true
            }
            DispatchEvent::ModalWizardNext(_)
            | DispatchEvent::ModalWizardBack(_)
            | DispatchEvent::Unhandled(_) => false,
            ev => {
                let starts_drag = matches!(
                    ev,
                    DispatchEvent::ResizeHandleDragStarted { .. }
                        | DispatchEvent::ScrollbarThumbDragStarted { .. }
                );
                let vp = viewport.unwrap_or(e.rect);
                let (c, r, v) = ((cursor.x, cursor.y), e.rect, (vp.width, vp.height));
                let host = &e.host;
                let rest = match &mut e.body {
                    OverlayBody::Modal(m) => modal_input::consume_event(
                        ev,
                        m,
                        host,
                        modal_input::ConsumeEventCtx {
                            cursor: c,
                            frame_rect: r,
                            viewport: v,
                        },
                    ),
                    OverlayBody::Popup(p) => popup_input::consume_event(
                        ev,
                        p,
                        host,
                        popup_input::ConsumeEventCtx {
                            cursor: c,
                            frame_rect: r,
                            viewport: v,
                        },
                    ),
                    OverlayBody::Dropdown(d) => dropdown_input::consume_event(
                        ev,
                        d,
                        host,
                        dropdown_input::ConsumeEventCtx {
                            cursor: c,
                            frame_rect: r,
                            viewport: v,
                        },
                    ),
                    OverlayBody::ContextMenu(_) | OverlayBody::Tooltip | OverlayBody::Custom => {
                        Some(ev)
                    }
                };
                if rest.is_some() {
                    return false;
                }
                fx.push(OverlayEffect::Invalidate {
                    win,
                    bits: InvalidateBits::MATERIAL,
                });
                if starts_drag {
                    fx.push(OverlayEffect::Capture {
                        win,
                        slot: Some(slot),
                    });
                }
                true
            }
        }
    }

    fn captured(
        &mut self,
        win: WindowId,
        slot: OverlaySlot,
        input: PointerInput,
        fx: &mut OverlayEffects<O>,
    ) -> bool {
        let Some(s) = self.windows.get_mut(&win) else {
            return false;
        };
        let viewport = s.viewport;
        let Some(e) = s.entries.iter_mut().find(|e| e.slot == slot) else {
            return false;
        };
        match input {
            PointerInput::Moved { pos, .. } => {
                let (resized, scrolled) = match &mut e.body {
                    OverlayBody::Modal(m) => {
                        let scrolling = m.scroll.is_dragging;
                        m.update_resize((pos.x, pos.y));
                        if scrolling {
                            m.update_body_scroll_drag(pos.y);
                        }
                        (m.resize_drag.and(m.resized_rect), scrolling)
                    }
                    OverlayBody::Popup(p) => {
                        let scrolling = p.scroll.is_dragging;
                        p.update_resize((pos.x, pos.y));
                        if scrolling {
                            p.update_body_scroll_drag(pos.y);
                        }
                        (p.resize_drag.and(p.resized_rect), scrolling)
                    }
                    _ => (None, false),
                };
                let mut changed = scrolled;
                if scrolled {
                    fx.push(OverlayEffect::Invalidate {
                        win,
                        bits: InvalidateBits::MATERIAL,
                    });
                }
                if let Some(r) = resized {
                    changed |= set_rect(win, e, r, viewport, fx);
                }
                changed
            }
            PointerInput::Up { .. } | PointerInput::Cancelled => {
                let was = match &mut e.body {
                    OverlayBody::Modal(m) => {
                        let was = m.resize_drag.is_some() || m.scroll.is_dragging;
                        m.end_resize();
                        m.end_body_scroll_drag();
                        was
                    }
                    OverlayBody::Popup(p) => {
                        let was = p.is_dragging_any();
                        p.end_resize();
                        p.end_body_scroll_drag();
                        was
                    }
                    _ => false,
                };
                fx.push(OverlayEffect::Capture { win, slot: None });
                was
            }
            _ => false,
        }
    }
}

/// Pointer verdict (see the module docs); returns `(route, state changed)`.
fn pointer<O: Copy + Eq>(
    win: WindowId,
    s: &mut Stack<O>,
    input: PointerInput,
    fx: &mut OverlayEffects<O>,
) -> (PointerRoute<O>, bool) {
    match input {
        PointerInput::Down { pos, .. } => {
            s.press = None;
            if let Some(t) = top_index(s) {
                let e = &s.entries[t];
                if e.policy.dismiss_on_outside && !e.contains(pos) {
                    s.press = Some(Press::Outside(e.slot));
                    return (PointerRoute::Consumed, false);
                }
            }
            let hit = hit_walk(s, pos);
            if let OverlayHit::Overlay { slot, .. } = hit {
                s.press = Some(Press::Inside(slot));
            }
            (hit.route(), false)
        }
        PointerInput::Up { pos, .. } => match s.press.take() {
            Some(Press::Outside(slot)) => {
                let dismiss = top_index(s).filter(|&t| {
                    let e = &s.entries[t];
                    e.slot == slot && !e.contains(pos)
                });
                match dismiss {
                    Some(t) => {
                        close_at(win, s, t, CloseCause::Outside, fx);
                        (PointerRoute::Consumed, true)
                    }
                    None => (PointerRoute::Consumed, false),
                }
            }
            Some(Press::Inside(slot)) => match s.entries.iter().find(|e| e.slot == slot) {
                Some(e) => (PointerRoute::Overlay { id: e.id, slot }, false),
                None => (hit_walk(s, pos).route(), false),
            },
            None => (hit_walk(s, pos).route(), false),
        },
        PointerInput::Moved { pos, .. } => (hit_walk(s, pos).route(), false),
        PointerInput::Cancelled => {
            s.press = None;
            (PointerRoute::Pass, false)
        }
        PointerInput::Entered | PointerInput::Left | PointerInput::RawDelta { .. } => {
            (PointerRoute::Pass, false)
        }
    }
}

/// Escape step: closes the first overlay (top → bottom) with
/// `dismiss_on_escape`, stopping at a modal without it. `true` if one closed.
fn escape<O: Copy + Eq>(
    win: WindowId,
    s: &mut Stack<O>,
    k: &KeyInput,
    fx: &mut OverlayEffects<O>,
) -> bool {
    if k.code != KeyCode::Escape || k.state != KeyState::Down {
        return false;
    }
    let mut target = None;
    for (i, e) in s.entries.iter().enumerate().rev() {
        if e.policy.dismiss_on_escape {
            target = Some(i);
            break;
        }
        if e.policy.modal {
            break;
        }
    }
    match target {
        Some(i) => {
            close_at(win, s, i, CloseCause::Escape, fx);
            true
        }
        None => false,
    }
}

/// The keymap context of a window: top overlay (with a keymap scope) and
/// the modal flag.
fn key_pass<O: Copy>(s: &Stack<O>) -> KeyRoute<O> {
    let top_overlay = top_index(s)
        .map(|i| &s.entries[i])
        .filter(|e| e.policy.keymap_scope)
        .map(|e| e.id);
    KeyRoute::Pass {
        top_overlay,
        modal_open: s.entries.iter().any(|e| e.policy.modal),
    }
}

/// The one Z walk: top → bottom, skip transparent entries, first rect
/// containing `p` wins, a modal not containing it shields.
fn hit_walk<O: Copy>(s: &Stack<O>, p: Point) -> OverlayHit<O> {
    for e in s.entries.iter().rev().filter(|e| !e.transparent()) {
        if e.contains(p) {
            return OverlayHit::Overlay {
                id: e.id,
                slot: e.slot,
                kind: e.kind,
            };
        }
        if e.policy.modal {
            return OverlayHit::Shielded {
                id: e.id,
                slot: e.slot,
            };
        }
    }
    OverlayHit::None
}

/// Index of the top overlay (topmost non-transparent entry).
fn top_index<O>(s: &Stack<O>) -> Option<usize> {
    s.entries.iter().rposition(|e| !e.transparent())
}

/// Close entry `i`, reporting each close; rebuilds the dispatcher. A modal
/// first closes everything above it (what it opened), newest first, so the
/// focus scopes of one op pop in reverse order of opening.
fn close_at<O: Copy>(
    win: WindowId,
    s: &mut Stack<O>,
    i: usize,
    cause: CloseCause,
    fx: &mut OverlayEffects<O>,
) {
    if s.entries[i].policy.modal {
        let mut above = s.entries.split_off(i + 1);
        above.sort_by_key(|e| std::cmp::Reverse(e.key.seq));
        for e in above {
            forget_press(s, e.slot);
            fx.push(closed(win, &e, cause));
        }
    }
    let e = s.entries.remove(i);
    forget_press(s, e.slot);
    fx.push(closed(win, &e, cause));
    rebuild_dispatch(s);
}

fn forget_press<O>(s: &mut Stack<O>, slot: OverlaySlot) {
    if matches!(s.press, Some(Press::Outside(p) | Press::Inside(p)) if p == slot) {
        s.press = None;
    }
}

fn closed<O: Copy>(win: WindowId, e: &Entry<O>, cause: CloseCause) -> OverlayEffect<O> {
    OverlayEffect::Closed {
        win,
        id: e.id,
        slot: e.slot,
        scope: e.scope(),
        cause,
        restore_focus: e.policy.restore_focus,
        auto_close: e.policy.auto_close_after.is_some(),
    }
}

/// Re-register every body's click patterns, bottom → top.
fn rebuild_dispatch<O>(s: &mut Stack<O>) {
    s.dispatch.clear();
    for e in &s.entries {
        match e.body {
            OverlayBody::Modal(_) => {
                register_modal_dispatch(&mut s.dispatch, &ModalHandle::new(e.host.clone()), true)
            }
            OverlayBody::Popup(_) => {
                register_popup_dispatch(&mut s.dispatch, &PopupHandle::new(e.host.clone()))
            }
            OverlayBody::Dropdown(_) => {
                register_dropdown_dispatch(&mut s.dispatch, &DropdownHandle::new(e.host.clone()))
            }
            OverlayBody::ContextMenu(_) => register_context_menu_dispatch(
                &mut s.dispatch,
                &ContextMenuHandle::new(e.host.clone()),
            ),
            OverlayBody::Tooltip | OverlayBody::Custom => {}
        }
    }
}

/// Set an entry's rect (clamped to the viewport when known); reports
/// `GEOMETRY` and `true` iff it changed.
fn set_rect<O>(
    win: WindowId,
    e: &mut Entry<O>,
    r: Rect,
    viewport: Option<Rect>,
    fx: &mut OverlayEffects<O>,
) -> bool {
    let r = viewport.map_or(r, |v| OverlayStack::clamp_to_viewport(r, v));
    if r == e.rect {
        return false;
    }
    e.rect = r;
    e.body.set_origin(r.x, r.y);
    fx.push(OverlayEffect::Invalidate {
        win,
        bits: InvalidateBits::GEOMETRY,
    });
    true
}

/// Initial rect: below the anchor (left-aligned), else centred in the
/// viewport, else at the origin; clamped to the viewport when known.
fn place(anchor: Option<Rect>, size: OverlaySize, viewport: Option<Rect>) -> Rect {
    let (w, h) = match size {
        OverlaySize::Fixed { width, height } => (finite(width), finite(height)),
        OverlaySize::Auto => (0.0, 0.0),
    };
    let (x, y) = match (anchor, viewport) {
        (Some(a), _) => (a.x, a.y + a.height),
        (None, Some(v)) => (v.x + (v.width - w) / 2.0, v.y + (v.height - h) / 2.0),
        (None, None) => (0.0, 0.0),
    };
    let r = Rect::new(x, y, w, h);
    viewport.map_or(r, |v| OverlayStack::clamp_to_viewport(r, v))
}

/// A non-negative finite length (anything else is `0`).
fn finite(v: f64) -> f64 {
    if v.is_finite() {
        v.max(0.0)
    } else {
        0.0
    }
}

/// Read-only projection of the [`OverlayEngine`].
#[derive(Debug)]
pub struct OverlayEngineView<'a, O> {
    e: &'a OverlayEngine<O>,
}

impl<O> Clone for OverlayEngineView<'_, O> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<O> Copy for OverlayEngineView<'_, O> {}

// Read doors for the native/web host briefs (F8+) - unused in-crate until then.
#[allow(dead_code)]
impl<'a, O: Copy + Eq> OverlayEngineView<'a, O> {
    /// The window's overlays, bottom → top (draw order).
    pub fn stack(&self, win: WindowId) -> impl Iterator<Item = OverlayEntryView<'a, O>> + 'a {
        self.e
            .windows
            .get(&win)
            .into_iter()
            .flat_map(|s| s.entries.iter().map(|e| OverlayEntryView { e }))
    }

    /// Number of open overlays in the window.
    pub fn len(&self, win: WindowId) -> usize {
        self.e.windows.get(&win).map_or(0, |s| s.entries.len())
    }

    /// No overlay is open in the window.
    pub fn is_empty(&self, win: WindowId) -> bool {
        self.len(win) == 0
    }

    /// The open instance of an app overlay.
    pub fn get(&self, win: WindowId, id: O) -> Option<OverlayEntryView<'a, O>> {
        let s = self.e.windows.get(&win)?;
        s.entries
            .iter()
            .find(|e| e.id == id)
            .map(|e| OverlayEntryView { e })
    }

    /// The top overlay (topmost that is not pointer-transparent).
    pub fn top(&self, win: WindowId) -> Option<OverlayEntryView<'a, O>> {
        let s = self.e.windows.get(&win)?;
        top_index(s).map(|i| OverlayEntryView { e: &s.entries[i] })
    }

    /// A modal is open in the window.
    pub fn modal_open(&self, win: WindowId) -> bool {
        self.e
            .windows
            .get(&win)
            .is_some_and(|s| s.entries.iter().any(|e| e.policy.modal))
    }

    /// The keymap context of the window, as a key verdict would carry it.
    pub fn key_context(&self, win: WindowId) -> KeyRoute<O> {
        self.e.windows.get(&win).map_or(
            KeyRoute::Pass {
                top_overlay: None,
                modal_open: false,
            },
            key_pass,
        )
    }

    /// Same as [`OverlayEngine::hit`].
    pub fn hit(&self, win: WindowId, p: Point) -> OverlayHit<O> {
        self.e.hit(win, p)
    }

    /// Same as [`OverlayEngine::topmost_at`].
    pub fn topmost_at(&self, win: WindowId, p: Point) -> Option<OverlayEntryView<'a, O>> {
        self.e.topmost_at(win, p)
    }

    /// The viewport of the last [`OverlayOp::Reclamp`].
    pub fn viewport(&self, win: WindowId) -> Option<Rect> {
        self.e.windows.get(&win).and_then(|s| s.viewport)
    }

    /// The window's click table for overlay bodies (compose / tests).
    pub fn dispatcher(&self, win: WindowId) -> Option<&'a ClickDispatcher> {
        self.e.windows.get(&win).map(|s| &s.dispatch)
    }

    /// The z table the stack is ordered by.
    pub fn z_table(&self) -> &'a ZLayerTable {
        &self.e.z
    }

    /// The snapshot rows of the window, bottom → top. `scope_depth` is the
    /// 1-based position of the overlay's focus scope among the open scoped
    /// overlays in opening order (the InputEngine's scope stack when every
    /// scope comes from an overlay), `0` without a scope.
    pub fn snapshot(&self, win: WindowId) -> Vec<OverlayView<O>> {
        let Some(s) = self.e.windows.get(&win) else {
            return Vec::new();
        };
        let mut scoped: Vec<(u64, OverlaySlot)> = s
            .entries
            .iter()
            .filter(|e| !e.transparent())
            .map(|e| (e.key.seq, e.slot))
            .collect();
        scoped.sort_unstable();
        s.entries
            .iter()
            .map(|e| OverlayView {
                id: e.id,
                slot: e.slot,
                kind: e.kind,
                rect: e.rect,
                modal: e.policy.modal,
                scope_depth: if e.transparent() {
                    0
                } else {
                    scoped
                        .iter()
                        .position(|&(_, sl)| sl == e.slot)
                        .map_or(0, |p| p + 1)
                },
            })
            .collect()
    }
}

/// Read-only view of one open overlay.
#[derive(Debug)]
// Read doors for the native/web host briefs (F8+) - unused in-crate until then.
#[allow(dead_code)]
pub struct OverlayEntryView<'a, O> {
    e: &'a Entry<O>,
}

impl<O> Clone for OverlayEntryView<'_, O> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<O> Copy for OverlayEntryView<'_, O> {}

// Read doors for the native/web host briefs (F8+) - unused in-crate until then.
#[allow(dead_code)]
impl<'a, O: Copy> OverlayEntryView<'a, O> {
    /// App identity.
    pub fn id(&self) -> O {
        self.e.id
    }

    /// Instance slot.
    pub fn slot(&self) -> OverlaySlot {
        self.e.slot
    }

    /// Composite kind.
    pub fn kind(&self) -> OverlayKind {
        self.e.kind
    }

    /// Rect, window-local logical pixels.
    pub fn rect(&self) -> Rect {
        self.e.rect
    }

    /// Anchor it opened at.
    pub fn anchor(&self) -> Option<Rect> {
        self.e.anchor
    }

    /// Requested size.
    pub fn size(&self) -> OverlaySize {
        self.e.size
    }

    /// Effective policy.
    pub fn policy(&self) -> OverlayPolicy {
        self.e.policy
    }

    /// The library composite state inside it.
    pub fn body(&self) -> &'a OverlayBody {
        &self.e.body
    }

    /// Host widget id of its body (`overlay-{slot}`); the body's widgets are
    /// `{host}:...`.
    pub fn host_id(&self) -> &'a WidgetId {
        &self.e.host
    }

    /// Host clock when it opened.
    pub fn opened_at(&self) -> Seconds {
        self.e.opened
    }

    /// Its kind's z value in the engine's table.
    pub fn z(&self) -> i32 {
        self.e.key.z
    }

    /// Its modal band (number of modals at or below it).
    pub fn band(&self) -> u32 {
        self.e.key.band
    }

    /// Pointer / key transparent (tooltips).
    pub fn is_transparent(&self) -> bool {
        self.e.transparent()
    }
}

#[cfg(test)]
mod tests;
