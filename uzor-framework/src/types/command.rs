//! The app's command vocabulary: one closed enum, one engine per variant.
//!
//! [`AppCommand`] is what the app sends in (through the handle, the init
//! context, intent contexts and hook outputs). The kernel's drain phase applies
//! each command to exactly the engine [`AppCommand::target`] names. There are
//! no string modes and no payload bags: every command is a typed variant.

use std::fmt;
use std::sync::Arc;

use uzor::input::KeyboardShortcut;
use uzor::layout::docking::{BranchId, DropZone, FloatingWindowId, LeafId, WindowLayout};
use uzor::layout::{EdgeSide, EdgeSlot, OverlayKind, SlotId};
use uzor::render::{InvalidateBits, TickRate};
use uzor::tokens::Tokens;
use uzor::widgets::composite::chrome::ChromeHit;
use uzor::{Rect, ResizeDirection, RgbaIcon, WidgetId};

use crate::types::frame::RegionSpec;
use crate::types::ids::{RegionId, Seconds, Ticket, WindowId};
use crate::types::layout_blob::LayoutBlob;
use crate::types::spec::Spec;
use crate::types::window::{CursorMode, FullscreenMode, Point, RenderCmd, SizePx, WindowSpec};

// ---------------------------------------------------------------------------
// AppCommand
// ---------------------------------------------------------------------------

/// Every visual command the app can issue.
///
/// Produced by the app; consumed by the kernel's drain phase, which applies it
/// to the single engine named by [`AppCommand::target`].
pub enum AppCommand<S: Spec> {
    /// Window lifecycle and OS properties.
    Window(WindowCmd),
    /// Chrome, edge slots, dock tree, layout persistence.
    Layout(LayoutCmd<S::Panel>),
    /// Open / close / move overlays.
    Overlay(OverlayCmd<S::Overlay>),
    /// Keyboard focus inside a window.
    Focus(FocusCmd),
    /// Key bindings.
    Keymap(KeymapCmd<S::Overlay, S::Action>),
    /// Tick rate, regions, invalidation, app timers.
    Cadence(CadenceCmd),
    /// App-wide visual palette.
    Theme(ThemeCmd),
    /// Render-backend control (forwarded to the host).
    Render(RenderCmd),
    /// System clipboard writes.
    Clipboard(ClipboardCmd),
    /// Capture a window; the PNG arrives as `Intent::Screenshot`.
    Screenshot {
        /// Window to capture.
        win: WindowId,
        /// Correlation ticket echoed in the intent.
        ticket: Ticket,
    },
    /// Stop the runtime and leave the host loop.
    Shutdown,
}

/// The engine domain a command is applied to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Domain {
    /// WindowEngine (windows, theme, host command queue).
    Window,
    /// LayoutEngine.
    Layout,
    /// OverlayEngine.
    Overlay,
    /// InputEngine (focus, capture, text).
    Input,
    /// KeymapEngine.
    Keymap,
    /// CadenceEngine.
    Cadence,
}

impl<S: Spec> AppCommand<S> {
    /// The one engine this command is applied to. Exhaustive by construction:
    /// adding a variant without a domain does not compile.
    pub fn target(&self) -> Domain {
        match self {
            AppCommand::Window(_) => Domain::Window,
            AppCommand::Layout(_) => Domain::Layout,
            AppCommand::Overlay(_) => Domain::Overlay,
            AppCommand::Focus(_) => Domain::Input,
            AppCommand::Keymap(_) => Domain::Keymap,
            AppCommand::Cadence(_) => Domain::Cadence,
            AppCommand::Theme(_) => Domain::Window,
            AppCommand::Render(_) => Domain::Window,
            AppCommand::Clipboard(_) => Domain::Window,
            AppCommand::Screenshot { .. } => Domain::Window,
            AppCommand::Shutdown => Domain::Window,
        }
    }
}

// Manual impls: a derive would demand `S: Clone` / `S: Debug` of the marker
// type itself; only its associated types need the bounds (given by `Spec`).
impl<S: Spec> Clone for AppCommand<S> {
    fn clone(&self) -> Self {
        match self {
            AppCommand::Window(c) => AppCommand::Window(c.clone()),
            AppCommand::Layout(c) => AppCommand::Layout(c.clone()),
            AppCommand::Overlay(c) => AppCommand::Overlay(*c),
            AppCommand::Focus(c) => AppCommand::Focus(c.clone()),
            AppCommand::Keymap(c) => AppCommand::Keymap(c.clone()),
            AppCommand::Cadence(c) => AppCommand::Cadence(c.clone()),
            AppCommand::Theme(c) => AppCommand::Theme(c.clone()),
            AppCommand::Render(c) => AppCommand::Render(*c),
            AppCommand::Clipboard(c) => AppCommand::Clipboard(c.clone()),
            AppCommand::Screenshot { win, ticket } => AppCommand::Screenshot {
                win: *win,
                ticket: *ticket,
            },
            AppCommand::Shutdown => AppCommand::Shutdown,
        }
    }
}

impl<S: Spec> fmt::Debug for AppCommand<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppCommand::Window(c) => f.debug_tuple("Window").field(c).finish(),
            AppCommand::Layout(c) => f.debug_tuple("Layout").field(c).finish(),
            AppCommand::Overlay(c) => f.debug_tuple("Overlay").field(c).finish(),
            AppCommand::Focus(c) => f.debug_tuple("Focus").field(c).finish(),
            AppCommand::Keymap(c) => f.debug_tuple("Keymap").field(c).finish(),
            AppCommand::Cadence(c) => f.debug_tuple("Cadence").field(c).finish(),
            AppCommand::Theme(c) => f.debug_tuple("Theme").field(c).finish(),
            AppCommand::Render(c) => f.debug_tuple("Render").field(c).finish(),
            AppCommand::Clipboard(c) => f.debug_tuple("Clipboard").field(c).finish(),
            AppCommand::Screenshot { win, ticket } => f
                .debug_struct("Screenshot")
                .field("win", win)
                .field("ticket", ticket)
                .finish(),
            AppCommand::Shutdown => f.write_str("Shutdown"),
        }
    }
}

// ---------------------------------------------------------------------------
// Window domain
// ---------------------------------------------------------------------------

/// Window commands from the app; applied by the WindowEngine, which enqueues
/// the matching [`WindowCommand`](crate::WindowCommand)s for the host.
#[derive(Clone, Debug)]
pub enum WindowCmd {
    /// Open a new window.
    Open(WindowSpec),
    /// Close a window (also the answer to `WindowIntent::CloseRequested`
    /// under an ask-the-app close policy).
    Close(WindowId),
    /// Set the OS title.
    SetTitle {
        /// Target window.
        win: WindowId,
        /// New title.
        title: String,
    },
    /// Set or clear the window icon.
    SetIcon {
        /// Target window.
        win: WindowId,
        /// New icon, or the default.
        icon: Option<RgbaIcon>,
    },
    /// Minimize or restore.
    Minimize {
        /// Target window.
        win: WindowId,
        /// `true` = minimize.
        minimized: bool,
    },
    /// Maximize or restore.
    Maximize {
        /// Target window.
        win: WindowId,
        /// `true` = maximize.
        maximized: bool,
    },
    /// Enter or leave fullscreen.
    Fullscreen {
        /// Target window.
        win: WindowId,
        /// Requested mode.
        mode: FullscreenMode,
    },
    /// Atomic move + resize.
    SetOuterRect {
        /// Target window.
        win: WindowId,
        /// Outer position, physical screen pixels.
        position: (i32, i32),
        /// Outer size, physical pixels.
        size: SizePx,
    },
    /// Bring a window to the front and focus it.
    Focus(WindowId),
    /// Cursor grab / lock mode.
    SetCursorMode {
        /// Target window.
        win: WindowId,
        /// Requested mode.
        mode: CursorMode,
    },
}

/// App-wide visual palette commands; applied by the WindowEngine (the one
/// owner of theme tokens).
#[derive(Clone, Debug)]
pub enum ThemeCmd {
    /// Replace the design tokens every window draws with.
    SetTokens(Arc<Tokens>),
    /// Switch between the dark and light variant.
    SetDark(bool),
}

/// System clipboard commands; applied by the WindowEngine as a host command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClipboardCmd {
    /// Write text to the clipboard.
    Write(String),
}

// ---------------------------------------------------------------------------
// Layout domain
// ---------------------------------------------------------------------------

/// Dock / chrome / edge-slot commands; applied by the LayoutEngine.
/// `P` is the app's panel type ([`Spec::Panel`]).
#[derive(Clone, Debug)]
pub enum LayoutCmd<P> {
    /// Show or hide the chrome strip and set its height (logical px).
    SetChrome {
        /// Target window.
        win: WindowId,
        /// Chrome visible.
        visible: bool,
        /// Strip height, logical pixels.
        height: f32,
    },
    /// Add an edge slot (toolbar, sidebar, status bar).
    AddEdgeSlot {
        /// Target window.
        win: WindowId,
        /// The slot.
        slot: EdgeSlot,
    },
    /// Remove an edge slot by id.
    RemoveEdgeSlot {
        /// Target window.
        win: WindowId,
        /// Slot id.
        id: SlotId,
    },
    /// Put a panel somewhere in the dock.
    OpenPanel {
        /// Target window.
        win: WindowId,
        /// The app's panel value.
        panel: P,
        /// Where it goes.
        at: DockTarget,
    },
    /// Close one tab of a leaf.
    ClosePanel {
        /// Target window.
        win: WindowId,
        /// The leaf.
        leaf: LeafId,
        /// Tab index inside the leaf.
        index: usize,
    },
    /// Activate one tab of a leaf.
    ActivateTab {
        /// Target window.
        win: WindowId,
        /// The leaf.
        leaf: LeafId,
        /// Tab index inside the leaf.
        index: usize,
    },
    /// Split a leaf and put a new panel on one side.
    Split {
        /// Target window.
        win: WindowId,
        /// The leaf to split.
        leaf: LeafId,
        /// Side the new panel goes to.
        dir: SplitDir,
        /// The new panel.
        panel: P,
    },
    /// Turn one tab into an in-window floating window.
    Float {
        /// Target window.
        win: WindowId,
        /// The leaf.
        leaf: LeafId,
        /// Tab index inside the leaf.
        index: usize,
        /// Floating rect, window-local logical pixels.
        rect: Rect,
    },
    /// Rebuild the dock from a lib preset.
    SetPreset {
        /// Target window.
        win: WindowId,
        /// The preset.
        preset: WindowLayout,
    },
    /// Set the child ratios of one branch.
    SetRatios {
        /// Target window.
        win: WindowId,
        /// The branch.
        branch: BranchId,
        /// One ratio per child, summing to 1.
        ratios: Vec<f64>,
    },
    /// Replace the layout policy (all windows).
    SetPolicy(LayoutPolicy),
    /// Ask for the window's layout blob; it arrives as `DockIntent::LayoutBlob`.
    RequestBlob {
        /// Target window.
        win: WindowId,
        /// Correlation ticket.
        ticket: Ticket,
    },
    /// Restore a previously produced blob.
    Restore {
        /// Target window.
        win: WindowId,
        /// The stored blob.
        blob: LayoutBlob,
    },
}

/// Where [`LayoutCmd::OpenPanel`] puts a panel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DockTarget {
    /// The root of the dock (new tab of the root leaf, or the first leaf).
    Root,
    /// Next to / into a leaf, by drop zone.
    Leaf(LeafId, DropZone),
    /// A new in-window floating window at this rect.
    Floating(Rect),
    /// The root of another window's dock.
    Window(WindowId),
}

/// Side of a leaf where [`LayoutCmd::Split`] places the new panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SplitDir {
    /// New panel left of the leaf.
    Left,
    /// New panel right of the leaf.
    Right,
    /// New panel above the leaf.
    Up,
    /// New panel below the leaf.
    Down,
}

/// Layout behaviour knobs; owned by the LayoutEngine, replaced by
/// [`LayoutCmd::SetPolicy`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutPolicy {
    /// Splitter constraint behaviour.
    pub splitter: SplitterPolicy,
    /// Whether and how panels can be dragged out into their own window.
    pub drag_out: DragOutPolicy,
    /// Width of the edge gutter that triggers an outer expand, logical px.
    pub edge_expand_px: f64,
    /// Width of the uzor resize bezel, logical px; used only when the host
    /// has no OS resize border.
    pub bezel_px: f64,
}

impl Default for LayoutPolicy {
    fn default() -> Self {
        Self {
            splitter: SplitterPolicy::Cascade,
            drag_out: DragOutPolicy::Disabled,
            edge_expand_px: 8.0,
            bezel_px: 6.0,
        }
    }
}

/// What a splitter drag does when it would violate a panel's minimum size —
/// the library's own policy type, applied by the LayoutEngine to each
/// window's `DockState`.
pub use uzor::layout::docking::SplitterPolicy;

/// Chrome a dragged-out panel window gets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DragOutChrome {
    /// OS decorations.
    Os,
    /// The uzor chrome strip.
    UzorChrome,
    /// No decorations at all.
    None,
}

/// Whether panels can be dragged out of the window into a micro-window.
/// With `Disabled` (or a single-window host) a torn-off panel floats
/// in-window instead.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DragOutPolicy {
    /// Tear-off floats inside the window.
    #[default]
    Disabled,
    /// Tear-off leaving the viewport spawns a window.
    Enabled {
        /// Chrome of the spawned window.
        chrome: DragOutChrome,
    },
}

/// What a window-level pointer position hits; a pure query result of the
/// LayoutEngine, consumed by the kernel's pointer routing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LayoutHit {
    /// A chrome strip element.
    Chrome(ChromeHit),
    /// The uzor resize bezel (only without an OS resize border).
    Bezel(ResizeDirection),
    /// A splitter between two dock children.
    Splitter {
        /// Separator index in the solved dock.
        sep: usize,
    },
    /// A corner where two splitters meet.
    Corner {
        /// First separator index.
        a: usize,
        /// Second separator index.
        b: usize,
    },
    /// A tab in a leaf's tab bar.
    TabBar {
        /// The leaf.
        leaf: LeafId,
        /// Tab index.
        tab: usize,
    },
    /// A panel's header strip.
    PanelHeader(LeafId),
    /// A panel's content area.
    PanelBody(LeafId),
    /// A floating window's header.
    FloatingHeader(FloatingWindowId),
    /// A floating window's content area.
    FloatingBody(FloatingWindowId),
    /// An expand gutter at a window edge.
    EdgeGutter(EdgeSide),
    /// Nothing layout-owned.
    None,
}

// ---------------------------------------------------------------------------
// Overlay domain
// ---------------------------------------------------------------------------

/// Overlay commands; applied by the OverlayEngine. `O` is the app's overlay
/// identity type ([`Spec::Overlay`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OverlayCmd<O> {
    /// Open (or re-open, replacing) an overlay.
    Open {
        /// Target window.
        win: WindowId,
        /// App overlay identity.
        id: O,
        /// Composite kind.
        kind: OverlayKind,
        /// Anchor rect (dropdown button, context-menu point), if any.
        anchor: Option<Rect>,
        /// Requested size.
        size: OverlaySize,
        /// Behaviour; `None` = the per-kind default.
        policy: Option<OverlayPolicy>,
    },
    /// Close an overlay by identity.
    Close {
        /// Target window.
        win: WindowId,
        /// App overlay identity.
        id: O,
    },
    /// Close the topmost overlay.
    CloseTop {
        /// Target window.
        win: WindowId,
    },
    /// Close the overlay if open, otherwise open it.
    Toggle {
        /// Target window.
        win: WindowId,
        /// App overlay identity.
        id: O,
        /// Composite kind (used when opening).
        kind: OverlayKind,
        /// Anchor rect (used when opening).
        anchor: Option<Rect>,
        /// Requested size (used when opening).
        size: OverlaySize,
        /// Behaviour (used when opening); `None` = the per-kind default.
        policy: Option<OverlayPolicy>,
    },
    /// Move an open overlay's origin.
    Move {
        /// Target window.
        win: WindowId,
        /// App overlay identity.
        id: O,
        /// New top-left corner, window-local logical pixels.
        to: Point,
    },
}

/// Requested overlay size.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum OverlaySize {
    /// The overlay's model decides (content / minimum size).
    #[default]
    Auto,
    /// Fixed size in logical pixels.
    Fixed {
        /// Width, logical pixels.
        width: f64,
        /// Height, logical pixels.
        height: f64,
    },
}

/// How an overlay behaves; enforced by the OverlayEngine.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OverlayPolicy {
    /// Blocks pointer, keys and focus for everything it covers.
    pub modal: bool,
    /// Pointer-up outside the top overlay closes it; the click is consumed.
    pub dismiss_on_outside: bool,
    /// Escape closes it.
    pub dismiss_on_escape: bool,
    /// Focus returns to the saved widget on close.
    pub restore_focus: bool,
    /// Close automatically after this long.
    pub auto_close_after: Option<Seconds>,
    /// Opens its own keymap scope (for `KeymapScope::Overlay` bindings).
    pub keymap_scope: bool,
}

impl OverlayPolicy {
    /// The per-kind default: modals are modal and close on Escape; popups,
    /// dropdowns, context menus and colour pickers close on outside click and
    /// Escape; tooltips have no behaviour. Every kind except tooltips
    /// restores focus and opens a keymap scope.
    pub const fn for_kind(kind: OverlayKind) -> Self {
        let none = Self {
            modal: false,
            dismiss_on_outside: false,
            dismiss_on_escape: false,
            restore_focus: false,
            auto_close_after: None,
            keymap_scope: false,
        };
        match kind {
            OverlayKind::Modal => Self {
                modal: true,
                dismiss_on_escape: true,
                restore_focus: true,
                keymap_scope: true,
                ..none
            },
            OverlayKind::Popup
            | OverlayKind::Dropdown
            | OverlayKind::ContextMenu
            | OverlayKind::ColorPicker => Self {
                dismiss_on_outside: true,
                dismiss_on_escape: true,
                restore_focus: true,
                keymap_scope: true,
                ..none
            },
            OverlayKind::Tooltip => none,
        }
    }
}

// ---------------------------------------------------------------------------
// Input / keymap domains
// ---------------------------------------------------------------------------

/// Keyboard focus commands; applied by the InputEngine. Focus scopes are not
/// commandable: they open and close only with overlays.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FocusCmd {
    /// Focus a widget (a no-op when it lies outside the active scope).
    Focus {
        /// Target window.
        win: WindowId,
        /// Widget to focus.
        id: WidgetId,
    },
    /// Clear focus.
    Clear {
        /// Target window.
        win: WindowId,
    },
    /// Move focus to the next widget of the active scope.
    Next {
        /// Target window.
        win: WindowId,
    },
    /// Move focus to the previous widget of the active scope.
    Prev {
        /// Target window.
        win: WindowId,
    },
}

/// The scope a key binding lives in. Precedence on resolve:
/// focused widget, then top overlay, then global.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum KeymapScope<O> {
    /// Always active (unless a modal blocks it).
    Global,
    /// Active while this overlay is open and on top.
    Overlay(O),
    /// Active while this widget has focus.
    Focused(WidgetId),
}

/// One key binding: chord to app action.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Binding<A> {
    /// Modifiers + key.
    pub chord: KeyboardShortcut,
    /// App action delivered as `Intent::Action`.
    pub action: A,
    /// Resolves even while a modal is open.
    pub through_modal: bool,
}

impl<A> Binding<A> {
    /// A binding that a modal blocks.
    pub fn new(chord: KeyboardShortcut, action: A) -> Self {
        Self {
            chord,
            action,
            through_modal: false,
        }
    }
}

/// Key-binding commands; applied by the KeymapEngine. Bindings change only
/// through these, never by a per-frame rebuild.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeymapCmd<O, A> {
    /// Add a binding; within a scope the last registered wins.
    Bind {
        /// Scope of the binding.
        scope: KeymapScope<O>,
        /// The binding.
        binding: Binding<A>,
    },
    /// Remove the binding of a chord in a scope.
    Unbind {
        /// Scope of the binding.
        scope: KeymapScope<O>,
        /// The chord.
        chord: KeyboardShortcut,
    },
    /// Remove every binding of a scope.
    Clear(KeymapScope<O>),
}

// ---------------------------------------------------------------------------
// Cadence domain
// ---------------------------------------------------------------------------

/// Repaint cadence and timer commands; applied by the CadenceEngine.
#[derive(Clone, Debug, PartialEq)]
pub enum CadenceCmd {
    /// Set a window's base tick rate.
    SetTickRate {
        /// Target window.
        win: WindowId,
        /// New rate.
        rate: TickRate,
    },
    /// Replace a window's paint regions.
    SetRegions {
        /// Target window.
        win: WindowId,
        /// The regions.
        regions: Vec<RegionSpec>,
    },
    /// Mark a region (or the whole window) as needing a repaint.
    Invalidate {
        /// Target window.
        win: WindowId,
        /// Region, or `None` for the whole window.
        region: Option<RegionId>,
        /// What changed.
        bits: InvalidateBits,
    },
    /// Arm an app timer; it fires as `Intent::Timer(token)`. Re-arming the
    /// same token replaces the previous deadline.
    WakeAt {
        /// App-chosen timer token.
        token: u64,
        /// Host-clock instant.
        at: Seconds,
    },
    /// Disarm an app timer.
    Cancel {
        /// App-chosen timer token.
        token: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor::input::{KeyCode, ModifierKeys};
    use uzor::layout::docking::DockPanel;

    #[derive(Clone, Debug)]
    struct TestPanel;
    impl DockPanel for TestPanel {
        fn title(&self) -> &str {
            "test"
        }
        fn type_id(&self) -> &'static str {
            "test"
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    enum TestOverlay {
        Settings,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    enum TestAction {
        Palette,
    }

    struct TestSpec;
    impl Spec for TestSpec {
        type Panel = TestPanel;
        type Overlay = TestOverlay;
        type Action = TestAction;
        fn decode_panel(_leaf: LeafId, type_id: &str) -> Option<TestPanel> {
            (type_id == "test").then_some(TestPanel)
        }
    }

    const W: WindowId = WindowId(1);

    fn chord() -> KeyboardShortcut {
        KeyboardShortcut::new(
            ModifierKeys {
                ctrl: true,
                ..ModifierKeys::default()
            },
            KeyCode::K,
        )
    }

    /// One command per variant, in declaration order.
    fn one_of_each() -> Vec<(AppCommand<TestSpec>, Domain)> {
        vec![
            (AppCommand::Window(WindowCmd::Focus(W)), Domain::Window),
            (
                AppCommand::Layout(LayoutCmd::RequestBlob {
                    win: W,
                    ticket: Ticket(1),
                }),
                Domain::Layout,
            ),
            (
                AppCommand::Overlay(OverlayCmd::CloseTop { win: W }),
                Domain::Overlay,
            ),
            (AppCommand::Focus(FocusCmd::Clear { win: W }), Domain::Input),
            (
                AppCommand::Keymap(KeymapCmd::Bind {
                    scope: KeymapScope::Overlay(TestOverlay::Settings),
                    binding: Binding::new(chord(), TestAction::Palette),
                }),
                Domain::Keymap,
            ),
            (
                AppCommand::Cadence(CadenceCmd::Cancel { token: 7 }),
                Domain::Cadence,
            ),
            (AppCommand::Theme(ThemeCmd::SetDark(true)), Domain::Window),
            (
                AppCommand::Render(RenderCmd::SetVsync(false)),
                Domain::Window,
            ),
            (
                AppCommand::Clipboard(ClipboardCmd::Write("x".into())),
                Domain::Window,
            ),
            (
                AppCommand::Screenshot {
                    win: W,
                    ticket: Ticket(2),
                },
                Domain::Window,
            ),
            (AppCommand::Shutdown, Domain::Window),
        ]
    }

    #[test]
    fn target_maps_every_variant_to_its_engine() {
        let all = one_of_each();
        assert_eq!(all.len(), 11, "one entry per AppCommand variant");
        for (cmd, domain) in &all {
            assert_eq!(cmd.target(), *domain, "{cmd:?}");
            assert_eq!(cmd.clone().target(), *domain);
        }
    }

    #[test]
    fn command_clone_and_debug_do_not_need_spec_bounds() {
        let cmd: AppCommand<TestSpec> = AppCommand::Layout(LayoutCmd::OpenPanel {
            win: W,
            panel: TestPanel,
            at: DockTarget::Root,
        });
        let text = format!("{:?}", cmd.clone());
        assert!(text.starts_with("Layout(OpenPanel"), "{text}");
        assert_eq!(
            format!("{:?}", AppCommand::<TestSpec>::Shutdown),
            "Shutdown"
        );
    }

    #[test]
    fn spec_decode_panel_is_the_factory() {
        assert!(TestSpec::decode_panel(LeafId(1), "test").is_some());
        assert!(TestSpec::decode_panel(LeafId(1), "other").is_none());
    }

    #[test]
    fn overlay_policy_per_kind_defaults() {
        let modal = OverlayPolicy::for_kind(OverlayKind::Modal);
        assert!(modal.modal && modal.dismiss_on_escape && !modal.dismiss_on_outside);
        let dd = OverlayPolicy::for_kind(OverlayKind::Dropdown);
        assert!(!dd.modal && dd.dismiss_on_outside && dd.dismiss_on_escape && dd.restore_focus);
        assert_eq!(
            OverlayPolicy::for_kind(OverlayKind::Tooltip),
            OverlayPolicy::default()
        );
    }

    #[test]
    fn layout_policy_default_is_todays_behaviour() {
        let p = LayoutPolicy::default();
        assert_eq!(p.splitter, SplitterPolicy::Cascade);
        assert_eq!(p.drag_out, DragOutPolicy::Disabled);
        assert_eq!(p.bezel_px, 6.0);
    }

    fn assert_send<T: Send>() {}
    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn commands_cross_threads() {
        assert_send::<AppCommand<TestSpec>>();
        assert_send_sync::<AppCommand<TestSpec>>();
    }
}
