//! WindowEngine: windows, theme tokens and the output command queue
//! (design §3.1).
//!
//! The one writer of per-window OS-facing state (geometry, caps, title,
//! cursor, IME, decorations, lifecycle status), of the app-wide theme
//! (tokens + dark flag) and of the host-bound [`WindowCommand`] queue. Every
//! host command passes through [`WindowEngine::apply`], so the output
//! vocabulary has one writer and one drain point ([`WindowEngine::drain`]).
//!
//! ## Queue semantics
//!
//! - Commands are queued as `(WindowId, WindowCommand)` in production order
//!   and moved out exactly once by [`WindowEngine::drain`].
//! - State-setting commands (title, cursor, cursor mode / visibility, IME,
//!   decorations, resizable, visible, fullscreen, min / max inner size,
//!   corner style, border colour, theme hint) are mirrored in the window's
//!   state; a command that would set the value the window already has is
//!   dropped (idempotent state commands are deduplicated against state).
//! - `RequestRedraw` is coalesced when the window's last queued command is
//!   already `RequestRedraw` (the previous kernel's `request_redraw`).
//! - Commands for unknown or closing windows are dropped; a window's queued
//!   commands are discarded when the host reports it destroyed.
//! - Queueing and draining are not state changes: only mirrored state moves
//!   the revision.
//!
//! ## Lifecycle
//!
//! `Create` allocates the next [`WindowId`] (sequential, never reused),
//! records the window as [`WindowStatus::Pending`] and enqueues
//! `WindowCommand::Spawn`; the host's `WindowInput::Created` echo opens it.
//! A user close request follows the [`ClosePolicy`]: `Auto` enqueues
//! `WindowCommand::Close` at once, `AskApp` reports
//! [`WindowEffect::CloseRequested`] and waits for the app's `WindowCmd::Close`.
//! A closing window is [`WindowStatus::Closing`] until the host's `Destroyed`
//! echo removes it ([`WindowEffect::Closed`]).

use std::collections::BTreeMap;
use std::sync::Arc;

use smallvec::SmallVec;
use uzor::layout::window::WindowKey;
use uzor::tokens::Tokens;
use uzor::{CursorIcon, Rect};

use crate::types::bus::{HostCaps, RenderInfo, WindowInput};
use crate::types::command::{ClipboardCmd, ThemeCmd, WindowCmd};
use crate::types::frame::WindowSurface;
use crate::types::ids::{Revision, WindowId};
use crate::types::ops::{WindowEffect, WindowOp};
use crate::types::window::{
    ClosePolicy, CursorMode, FullscreenMode, ImePurpose, SizePx, ThemeHint, WindowCommand,
    WindowGeometry, WindowSpec,
};

/// Effects of one WindowEngine op.
pub type WindowEffects = SmallVec<[WindowEffect; 2]>;

/// Where a window is in its lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WindowStatus {
    /// Id allocated and `Spawn` enqueued; the host has not echoed `Created`.
    Pending,
    /// The host created the window.
    Open,
    /// `Close` enqueued; waiting for the host's `Destroyed` echo.
    Closing,
}

/// Per-window OS-facing state (private; read through [`WindowRead`]).
#[derive(Clone, Debug, PartialEq)]
struct WindowState {
    status: WindowStatus,
    /// Live mirror of the spawn spec: key, title, decorations, resizable,
    /// visible, min inner size, corner style, border colour, tick rate.
    spec: WindowSpec,
    inner_size: SizePx,
    geometry: WindowGeometry,
    caps: HostCaps,
    fullscreen: FullscreenMode,
    max_inner_size: Option<SizePx>,
    cursor: CursorIcon,
    cursor_visible: bool,
    cursor_mode: CursorMode,
    ime_allowed: bool,
    ime_purpose: ImePurpose,
    ime_area: Option<Rect>,
    theme_hint: Option<ThemeHint>,
}

impl WindowState {
    fn new(spec: WindowSpec) -> Self {
        let inner_size = spec.inner_size;
        let mut geometry = WindowGeometry {
            position: spec.position,
            ..WindowGeometry::default()
        };
        geometry.viewport = logical_viewport(inner_size, geometry.scale);
        geometry.outer_size = inner_size;
        Self {
            status: WindowStatus::Pending,
            spec,
            inner_size,
            geometry,
            caps: HostCaps::default(),
            fullscreen: FullscreenMode::Off,
            max_inner_size: None,
            cursor: CursorIcon::default(),
            cursor_visible: true,
            cursor_mode: CursorMode::Normal,
            ime_allowed: false,
            ime_purpose: ImePurpose::Normal,
            ime_area: None,
            theme_hint: None,
        }
    }

    /// Set inner size and DPR; `true` when the logical viewport or the
    /// scale changed.
    fn set_surface(&mut self, size: SizePx, dpr: f64) -> bool {
        let dpr = sanitize_dpr(dpr);
        let viewport = logical_viewport(size, dpr);
        let moved = viewport != self.geometry.viewport || dpr != self.geometry.scale;
        self.inner_size = size;
        self.geometry.scale = dpr;
        self.geometry.viewport = viewport;
        moved
    }
}

/// A non-finite or non-positive device pixel ratio is treated as `1.0`.
fn sanitize_dpr(dpr: f64) -> f64 {
    if dpr.is_finite() && dpr > 0.0 {
        dpr
    } else {
        1.0
    }
}

/// Window-local logical viewport for an inner size in physical pixels.
fn logical_viewport(size: SizePx, dpr: f64) -> Rect {
    Rect::new(
        0.0,
        0.0,
        f64::from(size.width) / dpr,
        f64::from(size.height) / dpr,
    )
}

/// How a queued command relates to mirrored window state.
enum Mirror {
    /// The command sets state to a new value: queue it.
    Changed,
    /// The command sets the value the window already has: drop it.
    Same,
    /// The command carries no mirrored state: queue it.
    Stateless,
}

fn set_if<T: PartialEq>(slot: &mut T, value: T) -> Mirror {
    if *slot == value {
        Mirror::Same
    } else {
        *slot = value;
        Mirror::Changed
    }
}

/// Mirror a state-setting command into the window's state.
fn mirror(st: &mut WindowState, cmd: &WindowCommand) -> Mirror {
    match cmd {
        WindowCommand::SetTitle(t) => {
            if st.spec.title == *t {
                Mirror::Same
            } else {
                st.spec.title.clone_from(t);
                Mirror::Changed
            }
        }
        WindowCommand::SetDecorations(b) => set_if(&mut st.spec.decorations, *b),
        WindowCommand::SetCornerStyle(c) => set_if(&mut st.spec.corner_style, *c),
        WindowCommand::SetBorderColor(c) => set_if(&mut st.spec.border_color, *c),
        WindowCommand::SetTheme(h) => set_if(&mut st.theme_hint, *h),
        WindowCommand::SetVisible(b) => set_if(&mut st.spec.visible, *b),
        WindowCommand::SetFullscreen(m) => set_if(&mut st.fullscreen, *m),
        WindowCommand::SetResizable(b) => set_if(&mut st.spec.resizable, *b),
        WindowCommand::SetMinInnerSize(s) => set_if(&mut st.spec.min_inner_size, *s),
        WindowCommand::SetMaxInnerSize(s) => set_if(&mut st.max_inner_size, *s),
        WindowCommand::SetCursor(i) => set_if(&mut st.cursor, *i),
        WindowCommand::SetCursorVisible(b) => set_if(&mut st.cursor_visible, *b),
        WindowCommand::SetCursorMode(m) => set_if(&mut st.cursor_mode, *m),
        WindowCommand::SetImeAllowed(b) => set_if(&mut st.ime_allowed, *b),
        WindowCommand::SetImeCursorArea(r) => set_if(&mut st.ime_area, Some(*r)),
        WindowCommand::SetImePurpose(p) => set_if(&mut st.ime_purpose, *p),
        _ => Mirror::Stateless,
    }
}

const fn theme_hint(dark: bool) -> ThemeHint {
    if dark {
        ThemeHint::Dark
    } else {
        ThemeHint::Light
    }
}

/// The WindowEngine: see the module docs.
#[derive(Debug)]
pub struct WindowEngine {
    windows: BTreeMap<WindowId, WindowState>,
    /// Next id to allocate; `None` once `u32` is exhausted.
    next: Option<WindowId>,
    tokens: Arc<Tokens>,
    dark: bool,
    os_dark: Option<bool>,
    theme_rev: Revision,
    close: ClosePolicy,
    exit_requested: bool,
    render: RenderInfo,
    queue: Vec<(WindowId, WindowCommand)>,
    rev: Revision,
}

impl WindowEngine {
    /// An engine with no windows, the given app-wide theme and close policy.
    pub fn new(tokens: Arc<Tokens>, dark: bool, close: ClosePolicy) -> Self {
        Self {
            windows: BTreeMap::new(),
            next: Some(WindowId::FIRST),
            tokens,
            dark,
            os_dark: None,
            theme_rev: Revision::ZERO,
            close,
            exit_requested: false,
            render: RenderInfo::default(),
            queue: Vec::new(),
            rev: Revision::ZERO,
        }
    }

    /// The only mutation door. Bumps the revision iff state changed.
    pub fn apply(&mut self, op: WindowOp) -> WindowEffects {
        let mut fx = WindowEffects::new();
        let changed = match op {
            WindowOp::Create(spec) => self.create(spec, &mut fx),
            WindowOp::Lifecycle { win, input } => self.lifecycle(win, input, &mut fx),
            WindowOp::Cmd(cmd) => self.window_cmd(cmd, &mut fx),
            WindowOp::Theme(cmd) => self.theme(cmd, &mut fx),
            WindowOp::Render(cmd) => self.push_primary(WindowCommand::Render(cmd)),
            WindowOp::Clipboard(ClipboardCmd::Write(text)) => {
                self.push_primary(WindowCommand::ClipboardWrite(text))
            }
            WindowOp::Screenshot { win, ticket } => {
                self.push(win, WindowCommand::Screenshot(ticket))
            }
            WindowOp::Shutdown => self.shutdown(),
            WindowOp::Enqueue { win, cmd } => self.push(win, cmd),
            WindowOp::RenderInfo(info) => {
                if self.render == info {
                    false
                } else {
                    self.render = info;
                    true
                }
            }
        };
        if changed {
            self.rev.bump();
        }
        fx
    }

    /// Move every queued host command out, in production order. A second
    /// call without new ops returns an empty list.
    pub fn drain(&mut self) -> Vec<(WindowId, WindowCommand)> {
        std::mem::take(&mut self.queue)
    }

    /// Bumped on every state change (windows, theme, exit flag, render facts).
    pub fn revision(&self) -> Revision {
        self.rev
    }

    /// Read-only projection.
    pub fn view(&self) -> WindowEngineView<'_> {
        WindowEngineView { e: self }
    }

    // -- ops ----------------------------------------------------------------

    fn create(&mut self, spec: WindowSpec, fx: &mut WindowEffects) -> bool {
        let Some(id) = self.next else {
            return false;
        };
        self.next = id.next();
        self.windows.insert(id, WindowState::new(spec.clone()));
        self.queue.push((id, WindowCommand::Spawn(spec)));
        // OS decorations follow the app-wide theme from the first frame.
        self.push(id, WindowCommand::SetTheme(Some(theme_hint(self.dark))));
        fx.push(WindowEffect::Spawned(id));
        true
    }

    fn lifecycle(&mut self, win: WindowId, input: WindowInput, fx: &mut WindowEffects) -> bool {
        if let WindowInput::ThemeChanged { dark } = input {
            if !self.windows.contains_key(&win) || self.os_dark == Some(dark) {
                return false;
            }
            self.os_dark = Some(dark);
            return true;
        }
        if let WindowInput::Destroyed = input {
            if self.windows.remove(&win).is_none() {
                return false;
            }
            self.queue.retain(|(w, _)| *w != win);
            fx.push(WindowEffect::Closed(win));
            return true;
        }
        let close = self.close;
        let Some(st) = self.windows.get_mut(&win) else {
            return false;
        };
        let before = st.clone();
        match input {
            WindowInput::Created {
                size,
                dpr,
                position,
                caps,
            } => {
                let geometry_moved = st.set_surface(size, dpr);
                st.geometry.outer_size = size;
                if position.is_some() {
                    st.geometry.position = position;
                }
                st.caps = caps;
                if st.status == WindowStatus::Pending {
                    st.status = WindowStatus::Open;
                    fx.push(WindowEffect::Created(win));
                } else if geometry_moved {
                    fx.push(WindowEffect::GeometryChanged(win));
                }
            }
            WindowInput::Resized { size, dpr } => {
                if st.set_surface(size, dpr) {
                    fx.push(WindowEffect::GeometryChanged(win));
                }
            }
            WindowInput::Moved { position } => st.geometry.position = Some(position),
            WindowInput::Focused(focused) => {
                if st.geometry.focused != focused {
                    st.geometry.focused = focused;
                    fx.push(WindowEffect::FocusChanged { win, focused });
                }
            }
            WindowInput::Occluded(b) => st.geometry.occluded = b,
            WindowInput::Minimized(b) => st.geometry.minimized = b,
            WindowInput::Maximized(b) => st.geometry.maximized = b,
            WindowInput::CloseRequested => match close {
                ClosePolicy::AskApp => fx.push(WindowEffect::CloseRequested(win)),
                ClosePolicy::Auto => {
                    if st.status != WindowStatus::Closing {
                        st.status = WindowStatus::Closing;
                        self.queue.push((win, WindowCommand::Close));
                    }
                }
            },
            WindowInput::OuterRect { position, size } => {
                st.geometry.position = Some(position);
                st.geometry.outer_size = size;
            }
            // Handled above.
            WindowInput::ThemeChanged { .. } | WindowInput::Destroyed => {}
        }
        *st != before
    }

    fn window_cmd(&mut self, cmd: WindowCmd, fx: &mut WindowEffects) -> bool {
        match cmd {
            WindowCmd::Open(spec) => self.create(spec, fx),
            WindowCmd::Close(win) => self.push(win, WindowCommand::Close),
            WindowCmd::SetTitle { win, title } => self.push(win, WindowCommand::SetTitle(title)),
            WindowCmd::SetIcon { win, icon } => self.push(win, WindowCommand::SetIcon(icon)),
            WindowCmd::Minimize { win, minimized } => {
                self.push(win, WindowCommand::SetMinimized(minimized))
            }
            WindowCmd::Maximize { win, maximized } => {
                self.push(win, WindowCommand::SetMaximized(maximized))
            }
            WindowCmd::Fullscreen { win, mode } => {
                self.push(win, WindowCommand::SetFullscreen(mode))
            }
            WindowCmd::SetOuterRect {
                win,
                position,
                size,
            } => self.push(win, WindowCommand::SetOuterRect { position, size }),
            WindowCmd::Focus(win) => self.push(win, WindowCommand::FocusWindow),
            WindowCmd::SetCursorMode { win, mode } => {
                self.push(win, WindowCommand::SetCursorMode(mode))
            }
        }
    }

    fn theme(&mut self, cmd: ThemeCmd, fx: &mut WindowEffects) -> bool {
        match cmd {
            ThemeCmd::SetTokens(tokens) => {
                if Arc::ptr_eq(&self.tokens, &tokens) {
                    return false;
                }
                self.tokens = tokens;
            }
            ThemeCmd::SetDark(dark) => {
                if self.dark == dark {
                    return false;
                }
                self.dark = dark;
                let ids: Vec<WindowId> = self.windows.keys().copied().collect();
                for win in ids {
                    self.push(win, WindowCommand::SetTheme(Some(theme_hint(dark))));
                }
            }
        }
        self.theme_rev.bump();
        fx.push(WindowEffect::ThemeChanged);
        true
    }

    fn shutdown(&mut self) -> bool {
        if self.exit_requested {
            return false;
        }
        self.exit_requested = true;
        self.push_primary(WindowCommand::ExitApp);
        true
    }

    /// Queue a command for a window, mirroring / deduplicating state
    /// commands. Returns `true` iff window state changed.
    fn push(&mut self, win: WindowId, cmd: WindowCommand) -> bool {
        let Some(st) = self.windows.get_mut(&win) else {
            return false;
        };
        if st.status == WindowStatus::Closing {
            return false;
        }
        match cmd {
            // `Create` is the only spawn door: a spawn needs a fresh id.
            WindowCommand::Spawn(_) => false,
            WindowCommand::Close => {
                st.status = WindowStatus::Closing;
                self.queue.push((win, WindowCommand::Close));
                true
            }
            WindowCommand::RequestRedraw => {
                let last = self.queue.iter().rev().find(|(w, _)| *w == win);
                if !matches!(last, Some((_, WindowCommand::RequestRedraw))) {
                    self.queue.push((win, WindowCommand::RequestRedraw));
                }
                false
            }
            cmd => match mirror(st, &cmd) {
                Mirror::Same => false,
                Mirror::Changed => {
                    self.queue.push((win, cmd));
                    true
                }
                Mirror::Stateless => {
                    self.queue.push((win, cmd));
                    false
                }
            },
        }
    }

    /// Queue an app-wide command on the primary window, if any.
    fn push_primary(&mut self, cmd: WindowCommand) -> bool {
        match self.view().primary() {
            Some(win) => self.push(win, cmd),
            None => false,
        }
    }
}

/// Read-only projection of the [`WindowEngine`].
#[derive(Clone, Copy, Debug)]
pub struct WindowEngineView<'a> {
    e: &'a WindowEngine,
}

// Read doors for the native/web host briefs (F8+) - unused in-crate until then.
#[allow(dead_code)]
impl<'a> WindowEngineView<'a> {
    /// Every known window id (pending, open and closing), ascending.
    pub fn ids(&self) -> impl Iterator<Item = WindowId> + 'a {
        self.e.windows.keys().copied()
    }

    /// Every known window, ascending by id.
    pub fn windows(&self) -> impl Iterator<Item = WindowRead<'a>> + 'a {
        self.e
            .windows
            .iter()
            .map(|(id, st)| WindowRead { id: *id, st })
    }

    /// One window, if known.
    pub fn window(&self, win: WindowId) -> Option<WindowRead<'a>> {
        self.e
            .windows
            .get(&win)
            .map(|st| WindowRead { id: win, st })
    }

    /// The window app-wide host commands go to: the focused open window,
    /// else the open window with the lowest id.
    pub fn primary(&self) -> Option<WindowId> {
        let open = || {
            self.e
                .windows
                .iter()
                .filter(|(_, st)| st.status == WindowStatus::Open)
        };
        open()
            .find(|(_, st)| st.geometry.focused)
            .or_else(|| open().next())
            .map(|(id, _)| *id)
    }

    /// Paintable surfaces: open, non-minimized windows, ascending by id.
    pub fn surfaces(&self) -> Vec<WindowSurface> {
        self.e
            .windows
            .iter()
            .filter(|(_, st)| st.status == WindowStatus::Open && !st.geometry.minimized)
            .map(|(id, st)| WindowSurface {
                win: *id,
                size: st.inner_size,
                dpr: st.geometry.scale,
            })
            .collect()
    }

    /// The app-wide design tokens.
    pub fn tokens(&self) -> &'a Arc<Tokens> {
        &self.e.tokens
    }

    /// The app-wide dark flag.
    pub fn dark(&self) -> bool {
        self.e.dark
    }

    /// The OS theme as last reported by a host, if ever.
    pub fn os_dark(&self) -> Option<bool> {
        self.e.os_dark
    }

    /// Revision of the theme (tokens + dark flag) alone.
    pub fn theme_rev(&self) -> Revision {
        self.e.theme_rev
    }

    /// The close policy.
    pub fn close_policy(&self) -> ClosePolicy {
        self.e.close
    }

    /// The app asked to shut down.
    pub fn exit_requested(&self) -> bool {
        self.e.exit_requested
    }

    /// Render facts last reported by the host.
    pub fn render_info(&self) -> &'a RenderInfo {
        &self.e.render
    }

    /// Commands queued and not yet drained, in order.
    pub fn queued(&self) -> &'a [(WindowId, WindowCommand)] {
        &self.e.queue
    }
}

/// Read access to one window of the [`WindowEngine`].
#[derive(Clone, Copy, Debug)]
// Read doors for the native/web host briefs (F8+) - unused in-crate until then.
#[allow(dead_code)]
pub struct WindowRead<'a> {
    id: WindowId,
    st: &'a WindowState,
}

// Read doors for the native/web host briefs (F8+) - unused in-crate until then.
#[allow(dead_code)]
impl<'a> WindowRead<'a> {
    /// The window id.
    pub fn id(&self) -> WindowId {
        self.id
    }

    /// Lifecycle status.
    pub fn status(&self) -> WindowStatus {
        self.st.status
    }

    /// Stable app label.
    pub fn key(&self) -> &'a WindowKey {
        &self.st.spec.key
    }

    /// Current OS title.
    pub fn title(&self) -> &'a str {
        &self.st.spec.title
    }

    /// Live mirror of the spawn spec (title, decorations, resizable,
    /// visible, min inner size, corner style, border colour, tick rate).
    pub fn spec(&self) -> &'a WindowSpec {
        &self.st.spec
    }

    /// Geometry and OS state flags.
    pub fn geometry(&self) -> &'a WindowGeometry {
        &self.st.geometry
    }

    /// Inner size, physical pixels.
    pub fn inner_size(&self) -> SizePx {
        self.st.inner_size
    }

    /// Host capabilities (default until the `Created` echo).
    pub fn caps(&self) -> HostCaps {
        self.st.caps
    }

    /// Fullscreen mode last requested.
    pub fn fullscreen(&self) -> FullscreenMode {
        self.st.fullscreen
    }

    /// Maximum inner size last requested.
    pub fn max_inner_size(&self) -> Option<SizePx> {
        self.st.max_inner_size
    }

    /// Cursor icon last requested.
    pub fn cursor(&self) -> CursorIcon {
        self.st.cursor
    }

    /// Cursor visibility last requested.
    pub fn cursor_visible(&self) -> bool {
        self.st.cursor_visible
    }

    /// Cursor grab mode last requested.
    pub fn cursor_mode(&self) -> CursorMode {
        self.st.cursor_mode
    }

    /// OS input method enabled.
    pub fn ime_allowed(&self) -> bool {
        self.st.ime_allowed
    }

    /// IME purpose hint.
    pub fn ime_purpose(&self) -> ImePurpose {
        self.st.ime_purpose
    }

    /// IME caret area, window-local logical pixels.
    pub fn ime_area(&self) -> Option<Rect> {
        self.st.ime_area
    }

    /// Theme hint sent for OS decorations.
    pub fn theme_hint(&self) -> Option<ThemeHint> {
        self.st.theme_hint
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ids::Ticket;
    use crate::types::window::RenderCmd;
    use uzor::tokens::BuiltinSet;

    const W1: WindowId = WindowId(1);
    const W2: WindowId = WindowId(2);

    fn engine(close: ClosePolicy) -> WindowEngine {
        WindowEngine::new(Tokens::builtin(BuiltinSet::Light), false, close)
    }

    fn spec(key: &str) -> WindowSpec {
        WindowSpec::new(key, key, SizePx::new(800, 600))
    }

    fn created(size: SizePx, dpr: f64) -> WindowInput {
        WindowInput::Created {
            size,
            dpr,
            position: Some((10, 20)),
            caps: HostCaps {
                multi_window: true,
                ..HostCaps::default()
            },
        }
    }

    fn life(win: WindowId, input: WindowInput) -> WindowOp {
        WindowOp::Lifecycle { win, input }
    }

    /// An engine with windows 1 and 2 open and the queue drained.
    fn two_open(close: ClosePolicy) -> WindowEngine {
        let mut e = engine(close);
        e.apply(WindowOp::Create(spec("a")));
        e.apply(WindowOp::Create(spec("b")));
        e.apply(life(W1, created(SizePx::new(800, 600), 1.0)));
        e.apply(life(W2, created(SizePx::new(800, 600), 1.0)));
        e.drain();
        e
    }

    fn kinds(q: &[(WindowId, WindowCommand)]) -> Vec<(u32, String)> {
        q.iter()
            .map(|(w, c)| {
                let s = format!("{c:?}");
                let head = s.split(['(', ' ', '{']).next().unwrap_or("").to_string();
                (w.get(), head)
            })
            .collect()
    }

    #[test]
    fn create_allocates_sequential_ids_and_spawns() {
        let mut e = engine(ClosePolicy::Auto);
        assert_eq!(e.revision(), Revision::ZERO);
        let fx = e.apply(WindowOp::Create(spec("a")));
        assert_eq!(fx.as_slice(), &[WindowEffect::Spawned(W1)]);
        let fx = e.apply(WindowOp::Cmd(WindowCmd::Open(spec("b"))));
        assert_eq!(fx.as_slice(), &[WindowEffect::Spawned(W2)]);
        assert_eq!(e.revision(), Revision(2));
        let v = e.view();
        assert_eq!(
            v.window(W1).map(|w| w.status()),
            Some(WindowStatus::Pending)
        );
        assert_eq!(v.window(W2).map(|w| w.key().as_str()), Some("b"));
        // Pending windows have no surface and are not primary.
        assert!(v.surfaces().is_empty());
        assert_eq!(v.primary(), None);
        let q = e.drain();
        assert_eq!(
            kinds(&q),
            vec![
                (1, "Spawn".into()),
                (1, "SetTheme".into()),
                (2, "Spawn".into()),
                (2, "SetTheme".into()),
            ]
        );
        assert!(matches!(
            q[1].1,
            WindowCommand::SetTheme(Some(ThemeHint::Light))
        ));
    }

    #[test]
    fn ids_are_never_reused_or_wrapped() {
        let mut e = engine(ClosePolicy::Auto);
        e.next = Some(WindowId(u32::MAX));
        let fx = e.apply(WindowOp::Create(spec("last")));
        assert_eq!(fx.as_slice(), &[WindowEffect::Spawned(WindowId(u32::MAX))]);
        let rev = e.revision();
        let fx = e.apply(WindowOp::Create(spec("none")));
        assert!(fx.is_empty());
        assert_eq!(e.revision(), rev);
        assert_eq!(e.view().ids().count(), 1);
    }

    #[test]
    fn created_echo_opens_the_window() {
        let mut e = engine(ClosePolicy::Auto);
        e.apply(WindowOp::Create(spec("a")));
        let rev = e.revision();
        let fx = e.apply(life(W1, created(SizePx::new(1600, 1200), 2.0)));
        assert_eq!(fx.as_slice(), &[WindowEffect::Created(W1)]);
        assert_eq!(e.revision(), rev.next());
        let v = e.view();
        let w = v
            .window(W1)
            .map(|w| (w.status(), *w.geometry(), w.caps(), w.inner_size()));
        let Some((status, g, caps, inner)) = w else {
            panic!("window 1 missing");
        };
        assert_eq!(status, WindowStatus::Open);
        assert_eq!(g.viewport, Rect::new(0.0, 0.0, 800.0, 600.0));
        assert_eq!(g.scale, 2.0);
        assert_eq!(g.position, Some((10, 20)));
        assert!(caps.multi_window);
        assert_eq!(inner, SizePx::new(1600, 1200));
        assert_eq!(
            v.surfaces(),
            vec![WindowSurface {
                win: W1,
                size: SizePx::new(1600, 1200),
                dpr: 2.0
            }]
        );
        // An identical duplicate echo changes nothing.
        let rev = e.revision();
        let fx = e.apply(life(W1, created(SizePx::new(1600, 1200), 2.0)));
        assert!(fx.is_empty());
        assert_eq!(e.revision(), rev);
        // A duplicate echo with a new size reports a geometry change.
        let fx = e.apply(life(W1, created(SizePx::new(1000, 1200), 2.0)));
        assert_eq!(fx.as_slice(), &[WindowEffect::GeometryChanged(W1)]);
    }

    #[test]
    fn resize_and_dpr_changes() {
        let mut e = two_open(ClosePolicy::Auto);
        let rev = e.revision();
        // Same size and DPR: no change, no bump.
        let fx = e.apply(life(
            W1,
            WindowInput::Resized {
                size: SizePx::new(800, 600),
                dpr: 1.0,
            },
        ));
        assert!(fx.is_empty());
        assert_eq!(e.revision(), rev);
        // New size.
        let fx = e.apply(life(
            W1,
            WindowInput::Resized {
                size: SizePx::new(1000, 500),
                dpr: 1.0,
            },
        ));
        assert_eq!(fx.as_slice(), &[WindowEffect::GeometryChanged(W1)]);
        assert_eq!(e.revision(), rev.next());
        // DPR-only change: same physical size, new logical viewport.
        let fx = e.apply(life(
            W1,
            WindowInput::Resized {
                size: SizePx::new(1000, 500),
                dpr: 2.0,
            },
        ));
        assert_eq!(fx.as_slice(), &[WindowEffect::GeometryChanged(W1)]);
        let g = e.view().window(W1).map(|w| *w.geometry());
        assert_eq!(
            g.map(|g| g.viewport),
            Some(Rect::new(0.0, 0.0, 500.0, 250.0))
        );
        assert_eq!(g.map(|g| g.scale), Some(2.0));
        // Invalid DPR is treated as 1.0.
        e.apply(life(
            W1,
            WindowInput::Resized {
                size: SizePx::new(1000, 500),
                dpr: f64::NAN,
            },
        ));
        assert_eq!(e.view().window(W1).map(|w| w.geometry().scale), Some(1.0));
        // Moving is a state change but not a geometry (viewport) change.
        let rev = e.revision();
        let fx = e.apply(life(W1, WindowInput::Moved { position: (5, 6) }));
        assert!(fx.is_empty());
        assert_eq!(e.revision(), rev.next());
        let fx = e.apply(life(W1, WindowInput::Moved { position: (5, 6) }));
        assert!(fx.is_empty());
        assert_eq!(e.revision(), rev.next());
        // Flags.
        e.apply(life(W1, WindowInput::Maximized(true)));
        e.apply(life(W1, WindowInput::Occluded(true)));
        e.apply(life(
            W1,
            WindowInput::OuterRect {
                position: (1, 2),
                size: SizePx::new(1010, 530),
            },
        ));
        let g = e.view().window(W1).map(|w| *w.geometry());
        assert_eq!(g.map(|g| (g.maximized, g.occluded)), Some((true, true)));
        assert_eq!(g.map(|g| g.outer_size), Some(SizePx::new(1010, 530)));
        assert_eq!(g.and_then(|g| g.position), Some((1, 2)));
        assert_eq!(e.revision(), rev.next().next().next().next());
    }

    #[test]
    fn focus_changes_report_once() {
        let mut e = two_open(ClosePolicy::Auto);
        let rev = e.revision();
        let fx = e.apply(life(W2, WindowInput::Focused(true)));
        assert_eq!(
            fx.as_slice(),
            &[WindowEffect::FocusChanged {
                win: W2,
                focused: true
            }]
        );
        assert_eq!(e.revision(), rev.next());
        let fx = e.apply(life(W2, WindowInput::Focused(true)));
        assert!(fx.is_empty());
        assert_eq!(e.revision(), rev.next());
        assert_eq!(e.view().primary(), Some(W2));
        e.apply(life(W2, WindowInput::Focused(false)));
        assert_eq!(e.view().primary(), Some(W1));
    }

    #[test]
    fn close_auto_policy() {
        let mut e = two_open(ClosePolicy::Auto);
        let rev = e.revision();
        let fx = e.apply(life(W1, WindowInput::CloseRequested));
        assert!(fx.is_empty());
        assert_eq!(e.revision(), rev.next());
        assert_eq!(
            e.view().window(W1).map(|w| w.status()),
            Some(WindowStatus::Closing)
        );
        // A second request enqueues nothing more; commands to a closing
        // window are dropped.
        e.apply(life(W1, WindowInput::CloseRequested));
        e.apply(WindowOp::Cmd(WindowCmd::SetTitle {
            win: W1,
            title: "late".into(),
        }));
        assert_eq!(kinds(&e.drain()), vec![(1, "Close".into())]);
        // A closing window is neither primary nor paintable.
        assert_eq!(e.view().primary(), Some(W2));
        assert_eq!(e.view().surfaces().len(), 1);
        let fx = e.apply(life(W1, WindowInput::Destroyed));
        assert_eq!(fx.as_slice(), &[WindowEffect::Closed(W1)]);
        assert!(e.view().window(W1).is_none());
        // Echoes for a destroyed window are ignored.
        let rev = e.revision();
        let fx = e.apply(life(W1, WindowInput::Destroyed));
        assert!(fx.is_empty());
        let fx = e.apply(life(W1, WindowInput::Focused(true)));
        assert!(fx.is_empty());
        assert_eq!(e.revision(), rev);
    }

    #[test]
    fn close_ask_app_policy() {
        let mut e = two_open(ClosePolicy::AskApp);
        let rev = e.revision();
        let fx = e.apply(life(W1, WindowInput::CloseRequested));
        assert_eq!(fx.as_slice(), &[WindowEffect::CloseRequested(W1)]);
        // Asking changes no state and queues nothing.
        assert_eq!(e.revision(), rev);
        assert!(e.drain().is_empty());
        assert_eq!(
            e.view().window(W1).map(|w| w.status()),
            Some(WindowStatus::Open)
        );
        // The app answers.
        e.apply(WindowOp::Cmd(WindowCmd::Close(W1)));
        e.apply(WindowOp::Cmd(WindowCmd::Close(W1)));
        assert_eq!(e.revision(), rev.next());
        assert_eq!(kinds(&e.drain()), vec![(1, "Close".into())]);
        let fx = e.apply(life(W1, WindowInput::Destroyed));
        assert_eq!(fx.as_slice(), &[WindowEffect::Closed(W1)]);
    }

    #[test]
    fn destroyed_discards_queued_commands_of_that_window() {
        let mut e = two_open(ClosePolicy::Auto);
        e.apply(WindowOp::Enqueue {
            win: W1,
            cmd: WindowCommand::RequestRedraw,
        });
        e.apply(WindowOp::Enqueue {
            win: W2,
            cmd: WindowCommand::RequestRedraw,
        });
        e.apply(life(W1, WindowInput::Destroyed));
        assert_eq!(kinds(&e.drain()), vec![(2, "RequestRedraw".into())]);
    }

    #[test]
    fn unknown_window_ops_are_ignored() {
        let mut e = two_open(ClosePolicy::Auto);
        let rev = e.revision();
        let ghost = WindowId(99);
        assert!(e
            .apply(life(ghost, created(SizePx::new(1, 1), 1.0)))
            .is_empty());
        assert!(e
            .apply(life(ghost, WindowInput::ThemeChanged { dark: true }))
            .is_empty());
        e.apply(WindowOp::Cmd(WindowCmd::SetTitle {
            win: ghost,
            title: "x".into(),
        }));
        e.apply(WindowOp::Screenshot {
            win: ghost,
            ticket: Ticket(1),
        });
        assert_eq!(e.revision(), rev);
        assert!(e.drain().is_empty());
        assert!(e.view().window(ghost).is_none());
    }

    #[test]
    fn queue_order_drain_and_dedup() {
        let mut e = two_open(ClosePolicy::Auto);
        let rev = e.revision();
        let enq = |e: &mut WindowEngine, win, cmd| {
            e.apply(WindowOp::Enqueue { win, cmd });
        };
        // Same title as the spec: dropped, no state change.
        e.apply(WindowOp::Cmd(WindowCmd::SetTitle {
            win: W1,
            title: "a".into(),
        }));
        assert_eq!(e.revision(), rev);
        // New title: queued, state mirrored.
        e.apply(WindowOp::Cmd(WindowCmd::SetTitle {
            win: W1,
            title: "A".into(),
        }));
        assert_eq!(e.revision(), rev.next());
        assert_eq!(e.view().window(W1).map(|w| w.title()), Some("A"));
        // Initial cursor is Default: dropped. Pointer: queued once.
        enq(&mut e, W1, WindowCommand::SetCursor(CursorIcon::Default));
        enq(
            &mut e,
            W1,
            WindowCommand::SetCursor(CursorIcon::PointingHand),
        );
        enq(
            &mut e,
            W1,
            WindowCommand::SetCursor(CursorIcon::PointingHand),
        );
        // Redraw coalesced while it is the window's last command, even with
        // another window's command in between.
        enq(&mut e, W1, WindowCommand::RequestRedraw);
        enq(&mut e, W2, WindowCommand::DragWindow);
        enq(&mut e, W1, WindowCommand::RequestRedraw);
        // Stateless commands always queue.
        enq(&mut e, W1, WindowCommand::ClipboardWrite("x".into()));
        enq(&mut e, W1, WindowCommand::RequestRedraw);
        // IME area mirrored.
        let area = Rect::new(1.0, 2.0, 3.0, 4.0);
        enq(&mut e, W2, WindowCommand::SetImeCursorArea(area));
        enq(&mut e, W2, WindowCommand::SetImeCursorArea(area));
        // Spawn through Enqueue is refused (Create is the spawn door).
        enq(&mut e, W2, WindowCommand::Spawn(spec("z")));
        assert_eq!(e.view().window(W2).and_then(|w| w.ime_area()), Some(area));
        assert_eq!(e.view().queued().len(), 7);
        let q = e.drain();
        assert_eq!(
            kinds(&q),
            vec![
                (1, "SetTitle".into()),
                (1, "SetCursor".into()),
                (1, "RequestRedraw".into()),
                (2, "DragWindow".into()),
                (1, "ClipboardWrite".into()),
                (1, "RequestRedraw".into()),
                (2, "SetImeCursorArea".into()),
            ]
        );
        // Moved out once.
        assert!(e.drain().is_empty());
        assert!(e.view().queued().is_empty());
        // title, cursor, ime area = three state changes; draining is not one.
        assert_eq!(e.revision(), Revision(rev.get() + 3));
    }

    #[test]
    fn app_window_cmds_map_to_host_commands() {
        let mut e = two_open(ClosePolicy::Auto);
        let cmds = [
            WindowCmd::SetIcon {
                win: W1,
                icon: None,
            },
            WindowCmd::Minimize {
                win: W1,
                minimized: true,
            },
            WindowCmd::Maximize {
                win: W1,
                maximized: true,
            },
            WindowCmd::Fullscreen {
                win: W1,
                mode: FullscreenMode::Borderless,
            },
            WindowCmd::Fullscreen {
                win: W1,
                mode: FullscreenMode::Borderless,
            },
            WindowCmd::SetOuterRect {
                win: W1,
                position: (0, 0),
                size: SizePx::new(10, 10),
            },
            WindowCmd::Focus(W1),
            WindowCmd::SetCursorMode {
                win: W1,
                mode: CursorMode::Confined,
            },
        ];
        for c in cmds {
            e.apply(WindowOp::Cmd(c));
        }
        assert_eq!(
            kinds(&e.drain()),
            vec![
                (1, "SetIcon".into()),
                (1, "SetMinimized".into()),
                (1, "SetMaximized".into()),
                (1, "SetFullscreen".into()),
                (1, "SetOuterRect".into()),
                (1, "FocusWindow".into()),
                (1, "SetCursorMode".into()),
            ]
        );
        let w = e
            .view()
            .window(W1)
            .map(|w| (w.fullscreen(), w.cursor_mode()));
        assert_eq!(w, Some((FullscreenMode::Borderless, CursorMode::Confined)));
    }

    #[test]
    fn theme_tokens_and_dark_flag() {
        let mut e = two_open(ClosePolicy::Auto);
        let rev = e.revision();
        let v0 = e.view().theme_rev();
        // Same Arc: no change.
        let same = Arc::clone(e.view().tokens());
        assert!(e
            .apply(WindowOp::Theme(ThemeCmd::SetTokens(same)))
            .is_empty());
        assert_eq!((e.revision(), e.view().theme_rev()), (rev, v0));
        // New tokens.
        let dark_tokens = Tokens::builtin(BuiltinSet::Dark);
        let fx = e.apply(WindowOp::Theme(ThemeCmd::SetTokens(Arc::clone(
            &dark_tokens,
        ))));
        assert_eq!(fx.as_slice(), &[WindowEffect::ThemeChanged]);
        assert!(Arc::ptr_eq(e.view().tokens(), &dark_tokens));
        assert_eq!(e.view().theme_rev(), v0.next());
        assert_eq!(e.revision(), rev.next());
        assert!(e.drain().is_empty());
        // Dark flag: unchanged value is a no-op.
        assert!(e
            .apply(WindowOp::Theme(ThemeCmd::SetDark(false)))
            .is_empty());
        let fx = e.apply(WindowOp::Theme(ThemeCmd::SetDark(true)));
        assert_eq!(fx.as_slice(), &[WindowEffect::ThemeChanged]);
        assert!(e.view().dark());
        assert_eq!(e.view().theme_rev(), v0.next().next());
        let q = e.drain();
        assert_eq!(
            kinds(&q),
            vec![(1, "SetTheme".into()), (2, "SetTheme".into())]
        );
        assert!(q
            .iter()
            .all(|(_, c)| matches!(c, WindowCommand::SetTheme(Some(ThemeHint::Dark)))));
        assert!(e.apply(WindowOp::Theme(ThemeCmd::SetDark(true))).is_empty());
        // OS theme echo is recorded, once.
        let rev = e.revision();
        e.apply(life(W1, WindowInput::ThemeChanged { dark: true }));
        e.apply(life(W2, WindowInput::ThemeChanged { dark: true }));
        assert_eq!(e.view().os_dark(), Some(true));
        assert_eq!(e.revision(), rev.next());
    }

    #[test]
    fn app_wide_commands_go_to_the_primary_window() {
        let mut e = engine(ClosePolicy::Auto);
        // No window: dropped; the exit flag is still recorded.
        e.apply(WindowOp::Render(RenderCmd::SetVsync(true)));
        e.apply(WindowOp::Shutdown);
        assert!(e.drain().is_empty());
        assert!(e.view().exit_requested());
        let mut e = two_open(ClosePolicy::Auto);
        e.apply(WindowOp::Render(RenderCmd::SetVsync(true)));
        e.apply(life(W2, WindowInput::Focused(true)));
        e.apply(WindowOp::Clipboard(ClipboardCmd::Write("t".into())));
        e.apply(WindowOp::Screenshot {
            win: W1,
            ticket: Ticket(3),
        });
        let rev = e.revision();
        e.apply(WindowOp::Shutdown);
        e.apply(WindowOp::Shutdown);
        assert!(e.view().exit_requested());
        assert_eq!(e.revision(), rev.next());
        assert_eq!(
            kinds(&e.drain()),
            vec![
                (1, "Render".into()),
                (2, "ClipboardWrite".into()),
                (1, "Screenshot".into()),
                (2, "ExitApp".into()),
            ]
        );
    }

    #[test]
    fn minimized_windows_have_no_surface() {
        let mut e = two_open(ClosePolicy::Auto);
        e.apply(life(W1, WindowInput::Minimized(true)));
        let wins: Vec<WindowId> = e.view().surfaces().iter().map(|s| s.win).collect();
        assert_eq!(wins, vec![W2]);
    }

    #[test]
    fn render_info_is_revisioned_by_value() {
        let mut e = engine(ClosePolicy::Auto);
        let info = RenderInfo {
            frame_count: 3,
            ..RenderInfo::default()
        };
        e.apply(WindowOp::RenderInfo(info.clone()));
        assert_eq!(e.revision(), Revision(1));
        e.apply(WindowOp::RenderInfo(info));
        assert_eq!(e.revision(), Revision(1));
        assert_eq!(e.view().render_info().frame_count, 3);
    }
}
