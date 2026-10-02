//! The [`App`] contract and the narrow hook contexts (design §6.3).
//!
//! An app implements [`App`] once. The kernel calls its hooks with contexts
//! that reach exactly one frame's worth of on-screen truth and nothing
//! else: no engine, no kernel, no runtime, no inbox (ban F8). Effects a
//! hook asks for are declared through [`HookOps`] and applied by the next
//! settle pass; intents arrive through [`App::intent`] with an
//! [`IntentCx`] that answers with commands.

use uzor::app_context::StateRegistry;
use uzor::input::{InputCoordinator, LayerId};
use uzor::layout::docking::LeafId;
use uzor::render::RenderContext;
use uzor::tokens::Tokens;
use uzor::{Rect, WidgetId};

use crate::types::command::{AppCommand, ChromeModel};
use crate::types::ids::{Seconds, WindowId};
use crate::types::intent::Intent;
use crate::types::overlay_model::OverlayModel;
use crate::types::snapshot::VisualSnapshot;
use crate::types::spec::Spec;

/// The application: view-model declarations, content hooks and the intent
/// sink. One value drives any number of windows.
///
/// All hooks run on the runtime's thread inside the tick (`intent`,
/// `shutdown`) or the compose phase (`chrome_model`, `overlay_model`,
/// `panel`, `overlay_body`). Business state stays behind the app's own
/// back-office handle; nothing back-office crosses into this crate
/// (design §6.5).
pub trait App: 'static {
    /// The app's vocabulary (panels, overlays, actions).
    type Spec: Spec;

    /// Startup: enqueue the initial windows, keymap bindings and default
    /// layout as commands. Runs once before the first tick.
    fn init(&mut self, cx: &mut InitCx<'_, Self::Spec>) {
        let _ = cx;
    }

    /// What the kernel draws as this window's chrome strip, declared per
    /// frame. Compose diffs it against the layout engine's model and
    /// applies a `SetChromeModel` on change (the engine stays the single
    /// writer; hit-testing reads the same model the kernel draws).
    fn chrome_model(&self, win: WindowId) -> ChromeModel {
        let _ = win;
        ChromeModel::default()
    }

    /// What the kernel draws around one open overlay's body, declared per
    /// frame. The body itself is drawn by [`App::overlay_body`].
    fn overlay_model(&self, win: WindowId, id: <Self::Spec as Spec>::Overlay) -> OverlayModel;

    /// Draw one visible dock leaf's content with library widget functions,
    /// once per frame per leaf. The kernel gives the content rect (inside
    /// the leaf's frame and tab strip) and a scoped registration layer.
    fn panel(&mut self, cx: &mut PanelCx<'_, Self::Spec>);

    /// Draw one open overlay's body into the rect the frame leaves, once
    /// per frame per open overlay.
    fn overlay_body(&mut self, cx: &mut OverlayCx<'_, Self::Spec>);

    /// One typed intent produced by the tick, in production order. Answer
    /// with visual effects via [`IntentCx::command`]; send business effects
    /// through the app's own back-office handle.
    fn intent(&mut self, intent: Intent<Self::Spec>, cx: &mut IntentCx<'_, Self::Spec>);

    /// The runtime is shutting down (last window closed or
    /// `AppCommand::Shutdown`).
    fn shutdown(&mut self) {}
}

/// Startup door: commands only.
pub struct InitCx<'a, S: Spec> {
    pub(crate) out: &'a mut Vec<AppCommand<S>>,
}

impl<'a, S: Spec> InitCx<'a, S> {
    /// Enqueue a command, applied from the first tick on.
    pub fn command(&mut self, cmd: AppCommand<S>) {
        self.out.push(cmd);
    }
}

/// Intent hook door: the published snapshot (read-only) plus commands.
pub struct IntentCx<'a, S: Spec> {
    /// The snapshot published at the end of this tick (already includes
    /// every change the intent reports).
    pub view: &'a VisualSnapshot<S>,
    pub(crate) out: &'a mut Vec<AppCommand<S>>,
}

impl<'a, S: Spec> IntentCx<'a, S> {
    /// Enqueue a command, applied in the next settle pass of this tick.
    pub fn command(&mut self, cmd: AppCommand<S>) {
        self.out.push(cmd);
    }
}

/// Effects a content hook may declare; nothing else crosses (design §6.3).
pub enum HookOp<S: Spec> {
    /// Repaint this window next frame (animation, live data).
    RequestFrame,
    /// Tell the OS where the text caret is (IME candidate window).
    ImeArea(Rect),
    /// A visual command, applied in the next settle pass.
    Command(AppCommand<S>),
}

/// The sink a hook writes its declared effects into.
pub struct HookOps<S: Spec> {
    pub(crate) ops: Vec<HookOp<S>>,
}

impl<S: Spec> Default for HookOps<S> {
    fn default() -> Self {
        Self { ops: Vec::new() }
    }
}

impl<S: Spec> HookOps<S> {
    /// Repaint this window next frame.
    pub fn request_frame(&mut self) {
        self.ops.push(HookOp::RequestFrame);
    }

    /// Report the text caret rect (window-local logical pixels) to the OS.
    pub fn ime_area(&mut self, rect: Rect) {
        self.ops.push(HookOp::ImeArea(rect));
    }

    /// Enqueue a visual command for the next settle pass.
    pub fn command(&mut self, cmd: AppCommand<S>) {
        self.ops.push(HookOp::Command(cmd));
    }
}

/// Wall-clock facts of this frame; both values come from the host clock
/// the runtime was given, never read inside the crate.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameTime {
    /// Host clock of the tick.
    pub now: Seconds,
    /// Seconds since the previous painted frame of this window.
    pub dt: f64,
}

/// Read-only per-window truth of this frame, built at the start of the
/// compose phase (design §5.2, read path a).
///
/// Owned, not borrowed: the compose phase mutates the input engine
/// (registrations) while hooks hold the view, so the engine views cannot
/// be lent out; the copied state is exactly what the engines reported at
/// phase start, which is the same-tick truth the design asks the view to
/// be. Clicks are the last evaluated frame's (a release is evaluated by
/// the `EndFrame` of the compose that saw it and read back from the next
/// compose on — one frame of latency, like every retained-registration
/// immediate-mode UI).
pub struct VisualView<S: Spec> {
    pub(crate) hovered: Option<WidgetId>,
    pub(crate) pressed: Option<WidgetId>,
    pub(crate) focused: Option<WidgetId>,
    pub(crate) clicked: Vec<(WidgetId, u8)>,
    pub(crate) scroll: (f64, f64),
    pub(crate) captured: bool,
    pub(crate) open_overlays: Vec<S::Overlay>,
}

// Manual impl: a derive would demand `S: Default` of the marker type.
impl<S: Spec> Default for VisualView<S> {
    fn default() -> Self {
        Self {
            hovered: None,
            pressed: None,
            focused: None,
            clicked: Vec::new(),
            scroll: (0.0, 0.0),
            captured: false,
            open_overlays: Vec::new(),
        }
    }
}

impl<S: Spec> VisualView<S> {
    /// The widget under the pointer, if any.
    pub fn hovered(&self) -> Option<&WidgetId> {
        self.hovered.as_ref()
    }

    /// `id` is under the pointer.
    pub fn is_hovered(&self, id: &WidgetId) -> bool {
        self.hovered.as_ref() == Some(id)
    }

    /// The widget being pressed, if any.
    pub fn pressed(&self) -> Option<&WidgetId> {
        self.pressed.as_ref()
    }

    /// `id` is being pressed.
    pub fn is_pressed(&self, id: &WidgetId) -> bool {
        self.pressed.as_ref() == Some(id)
    }

    /// The widget with keyboard focus, if any.
    pub fn focused(&self) -> Option<&WidgetId> {
        self.focused.as_ref()
    }

    /// `id` has keyboard focus.
    pub fn is_focused(&self, id: &WidgetId) -> bool {
        self.focused.as_ref() == Some(id)
    }

    /// `id` was clicked in the last evaluated frame (single, double or
    /// triple — see [`VisualView::click_count`]).
    pub fn clicked(&self, id: &WidgetId) -> bool {
        self.click_count(id) > 0
    }

    /// The click count `id` was last clicked with (0 = not clicked,
    /// 1 single, 2 double, 3 triple).
    pub fn click_count(&self, id: &WidgetId) -> u8 {
        self.clicked
            .iter()
            .find_map(|(w, n)| (w == id).then_some(*n))
            .unwrap_or(0)
    }

    /// Every widget clicked in the last evaluated frame, with counts.
    pub fn clicks(&self) -> &[(WidgetId, u8)] {
        &self.clicked
    }

    /// Scroll delta accumulated for this frame, in lines.
    pub fn scroll(&self) -> (f64, f64) {
        self.scroll
    }

    /// The pointer is captured by a widget or an engine.
    pub fn captured(&self) -> bool {
        self.captured
    }

    /// This overlay is open in the window.
    pub fn overlay_open(&self, id: S::Overlay) -> bool {
        self.open_overlays.contains(&id)
    }

    /// Every open overlay of the window, bottom to top.
    pub fn open_overlays(&self) -> &[S::Overlay] {
        &self.open_overlays
    }
}

/// Widget registration face a hook draws against: the window's coordinator
/// (library L1 `register_*` fns take it), the per-widget typed state store
/// and the layer the hook's widgets register into.
///
/// The coordinator's state-mutating driver methods are deliberately not
/// reachable here (one writer of focus / hover / capture, design §6.3);
/// lib brief L6 makes that mechanical.
pub struct Widgets<'a> {
    /// The window's input coordinator (widget registration only).
    pub coord: &'a mut InputCoordinator,
    /// Per-widget persistent typed state (scroll offsets, expand flags).
    pub states: &'a mut StateRegistry,
    /// The layer this hook's widgets register into (panel-local or the
    /// overlay's own).
    pub layer: LayerId,
}

/// One dock leaf's content hook context (design §6.3).
pub struct PanelCx<'a, S: Spec> {
    /// The window being composed.
    pub window: WindowId,
    /// The leaf being drawn.
    pub leaf: LeafId,
    /// The app's own panel value (the active tab of the leaf).
    pub panel: &'a S::Panel,
    /// The panel's tab index inside the leaf.
    pub index: usize,
    /// Content rect, window-local logical pixels (inside the leaf frame
    /// and its tab strip).
    pub rect: Rect,
    /// Device pixel ratio of the window.
    pub dpr: f64,
    /// Active theme tokens.
    pub tokens: &'a Tokens,
    /// The paint target.
    pub render: &'a mut dyn RenderContext,
    /// Widget registration face.
    pub widgets: Widgets<'a>,
    /// This frame's input truth of the window.
    pub view: &'a VisualView<S>,
    /// Frame clock.
    pub time: FrameTime,
    /// Declared effects out.
    pub out: &'a mut HookOps<S>,
}

/// One open overlay's body hook context (design §6.3); same reach as
/// [`PanelCx`], plus the overlay identity and the body rect the kernel's
/// frame leaves.
pub struct OverlayCx<'a, S: Spec> {
    /// The window being composed.
    pub window: WindowId,
    /// The overlay being drawn.
    pub id: S::Overlay,
    /// Body rect, window-local logical pixels (inside the kernel-drawn
    /// frame).
    pub body: Rect,
    /// Content rect the body may register widgets into (equals `body`).
    pub rect: Rect,
    /// Device pixel ratio of the window.
    pub dpr: f64,
    /// Active theme tokens.
    pub tokens: &'a Tokens,
    /// The paint target.
    pub render: &'a mut dyn RenderContext,
    /// Widget registration face (the overlay's own layer).
    pub widgets: Widgets<'a>,
    /// This frame's input truth of the window.
    pub view: &'a VisualView<S>,
    /// Frame clock.
    pub time: FrameTime,
    /// Declared effects out.
    pub out: &'a mut HookOps<S>,
}
