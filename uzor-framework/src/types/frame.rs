//! Frame output and cadence vocabulary: what the host should paint, when the
//! loop should wake next, and the region / timer identities the CadenceEngine
//! schedules.

use smallvec::SmallVec;
use uzor::render::{InvalidateBits, RetainedScope};
use uzor::Rect;

use crate::types::ids::{OverlaySlot, RegionId, Seconds, WindowId};
use crate::types::window::SizePx;

/// One window paint the host must perform this tick.
///
/// Produced by the CadenceEngine in the publish phase (due regions plus
/// retained-cache invalidations); consumed by the host, which forwards the
/// invalidations to its render state and calls the runtime's paint once per
/// region. Hosts never compute "due" themselves.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameRequest {
    /// The window to paint.
    pub window: WindowId,
    /// Surface size, physical pixels.
    pub size: SizePx,
    /// Device pixel ratio.
    pub dpr: f64,
    /// Clear colour as `0xRRGGBBAA`.
    pub background: u32,
    /// Regions to repaint; empty = the whole window as one region.
    pub regions: SmallVec<[RegionPlan; 2]>,
    /// Retained-cache invalidations to apply before painting.
    pub invalidations: SmallVec<[(RetainedScope, InvalidateBits); 4]>,
}

/// One region to repaint inside a [`FrameRequest`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegionPlan {
    /// The app-declared region, or `None` for the implicit whole-window region.
    pub id: Option<RegionId>,
    /// Region rect, window-local logical pixels.
    pub rect: Rect,
}

/// When the host loop should run the next tick.
///
/// Produced in the publish phase; the host maps it to `ControlFlow` (native)
/// or RAF / `setTimeout` (web).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Wake {
    /// Sleep until the next input or handle dispatch.
    #[default]
    Idle,
    /// Run the next frame as soon as possible (animation in flight).
    Immediate,
    /// Wake at this host-clock instant (earliest deadline or capped region).
    At(Seconds),
}

/// One paint region an app declares for a window.
///
/// Produced by the app (`CadenceCmd::SetRegions`); owned by the
/// CadenceEngine and echoed in [`CadenceView`](crate::CadenceView).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegionSpec {
    /// Region identity, unique within the window.
    pub id: RegionId,
    /// Region rect, window-local logical pixels.
    pub rect: Rect,
    /// Repaint cadence: 0 = only when dirty, otherwise frames per second.
    pub target_fps: u32,
    /// Needs a repaint now.
    pub dirty: bool,
}

/// Who armed a deadline on the CadenceEngine's wheel; re-arming the same
/// owner replaces its previous deadline.
///
/// The kernel converts fired owners: `App(n)` becomes `Intent::Timer(n)`,
/// `OverlayAutoClose` closes that overlay, `Tooltip` shows / hides a tooltip.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TimerOwner {
    /// An app timer (`CadenceCmd::WakeAt`) with the app's token.
    App(u64),
    /// Auto-close deadline of one open overlay.
    OverlayAutoClose {
        /// The overlay's window.
        win: WindowId,
        /// The overlay instance.
        slot: OverlaySlot,
    },
    /// Tooltip delay of one window.
    Tooltip {
        /// The window.
        win: WindowId,
    },
}

/// The paintable surface of one window, as the WindowEngine knows it.
///
/// Produced from the WindowEngine's view (open, non-minimized windows only)
/// and handed by the kernel to the CadenceEngine, which copies size and DPR
/// into each [`FrameRequest`]. A window without a surface gets no frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowSurface {
    /// The window.
    pub win: WindowId,
    /// Inner size, physical pixels.
    pub size: SizePx,
    /// Device pixel ratio.
    pub dpr: f64,
}
