//! Splitter and corner drags (design §3.2.2). The hit is typed
//! (`LayoutHit::Splitter { sep, .. }` / `Corner`), the constraint
//! behaviour is the library's: every move calls
//! `DockState::drag_separator` under the window's `SplitterPolicy`
//! (configured from `LayoutPolicy::splitter`).

use uzor::layout::docking::{DockPanel, SeparatorOrientation};

use crate::types::command::{LayoutPolicy, SplitterPolicy};
use crate::types::window::Point;

use super::{rects, DockState, SessionKind, WindowLayout};

/// Separator moves smaller than this (logical px) are ignored.
const MIN_STEP_PX: f64 = 1e-3;

/// What a splitter session drags.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Target {
    Line(usize),
    Corner { vertical: usize, horizontal: usize },
}

/// A live splitter / corner drag.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SplitDrag {
    target: Target,
    /// Pointer minus line position at press, per axis (x for the vertical
    /// line, y for the horizontal one), so the line keeps its grab point.
    grab: (f64, f64),
    /// A move was refused under `RejectSnapBack`: the rest of the drag is
    /// ignored until release.
    frozen: bool,
}

impl SplitDrag {
    pub(super) fn kind(&self) -> SessionKind {
        match self.target {
            Target::Line(sep) => SessionKind::Splitter {
                sep,
                frozen: self.frozen,
            },
            Target::Corner {
                vertical,
                horizontal,
            } => SessionKind::Corner {
                vertical,
                horizontal,
                frozen: self.frozen,
            },
        }
    }
}

fn axis(p: Point, o: SeparatorOrientation) -> f64 {
    match o {
        SeparatorOrientation::Vertical => p.x,
        SeparatorOrientation::Horizontal => p.y,
    }
}

/// Start dragging separator `sep` from `pos`.
pub(super) fn start_line<P: DockPanel>(
    dock: &DockState<P>,
    sep: usize,
    pos: Point,
) -> Option<SplitDrag> {
    let s = dock.separators().get(sep)?;
    let g = axis(pos, s.orientation) - s.position as f64;
    Some(SplitDrag {
        target: Target::Line(sep),
        grab: (g, g),
        frozen: false,
    })
}

/// Start dragging the crossing of `vertical` and `horizontal` from `pos`.
pub(super) fn start_corner<P: DockPanel>(
    dock: &DockState<P>,
    vertical: usize,
    horizontal: usize,
    pos: Point,
) -> Option<SplitDrag> {
    let v = dock.separators().get(vertical)?;
    let h = dock.separators().get(horizontal)?;
    Some(SplitDrag {
        target: Target::Corner {
            vertical,
            horizontal,
        },
        grab: (pos.x - v.position as f64, pos.y - h.position as f64),
        frozen: false,
    })
}

enum Step {
    Moved,
    Rejected,
    Still,
}

/// Move separator `sep` so its line sits at `line` (logical px along its
/// axis), then re-solve so the next move reads fresh positions.
fn move_to<P: DockPanel>(w: &mut WindowLayout<P>, sep: usize, line: f64) -> Step {
    let Some(s) = w.dock.separators().get(sep) else {
        return Step::Still;
    };
    let delta = line - s.position as f64;
    if delta.abs() < MIN_STEP_PX {
        return Step::Still;
    }
    let area = w.dock.layout_area();
    let snaps = w.dock.snap_animations().len();
    let ok = w
        .dock
        .drag_separator(sep, delta as f32, area.width, area.height);
    rects::solve(w);
    if ok {
        Step::Moved
    } else if w.dock.snap_animations().len() > snaps {
        Step::Rejected
    } else {
        Step::Still
    }
}

/// One pointer move of a splitter session.
pub(super) fn drag<P: DockPanel>(
    w: &mut WindowLayout<P>,
    mut d: SplitDrag,
    pos: Point,
    policy: &LayoutPolicy,
) -> SplitDrag {
    if d.frozen {
        return d;
    }
    let steps = match d.target {
        Target::Line(sep) => {
            let Some(o) = w.dock.separators().get(sep).map(|s| s.orientation) else {
                return d;
            };
            vec![move_to(w, sep, axis(pos, o) - d.grab.0)]
        }
        Target::Corner {
            vertical,
            horizontal,
        } => vec![
            move_to(w, vertical, pos.x - d.grab.0),
            move_to(w, horizontal, pos.y - d.grab.1),
        ],
    };
    if policy.splitter == SplitterPolicy::RejectSnapBack
        && steps.iter().any(|s| matches!(s, Step::Rejected))
    {
        d.frozen = true;
    }
    d
}

/// Snap-back springs as comparable values (separator, offset).
pub(super) fn snap_obs<P: DockPanel>(dock: &DockState<P>) -> Vec<(usize, f32)> {
    dock.snap_animations()
        .iter()
        .map(|a| (a.separator_idx, a.offset()))
        .collect()
}
