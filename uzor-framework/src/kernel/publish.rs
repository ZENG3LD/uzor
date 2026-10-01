//! Phase 7 — Publish (design §4.6): the snapshot leaves the kernel only
//! when some engine revision moved (invariant 1), bumped exactly once per
//! publish (invariant 2), assembled window by window in `WindowId` order
//! (invariant 4). The frame requests and the wake are the cadence
//! engine's, built against a cross-engine view assembled here.

use uzor::tokens::{ColorValue, Tokens};

use crate::engine::cadence::CadenceTickView;
use crate::types::frame::{FrameRequest, Wake};
use crate::types::ids::{Seconds, WindowId};
use crate::types::snapshot::{CadenceView, ChromeView, DockView, InputView, VisualSnapshot, WindowView};
use crate::types::spec::Spec;
use crate::types::window::WindowCommand;

use super::{Kernel, Phase};

/// What phase 7 produced (design §2.5's `TickOutput` payload, minus the
/// intents — the runtime drains those from the outbox itself).
pub(crate) struct PublishOut<S: Spec> {
    /// The new snapshot, `Some` iff an engine revision moved since the
    /// last publish (the handle keeps serving the previous `Arc`
    /// otherwise — §4.6 invariant 1).
    pub snapshot: Option<VisualSnapshot<S>>,
    /// Host-bound window commands, drained in order.
    pub window_commands: Vec<(WindowId, WindowCommand)>,
    /// Due frame requests, ascending by window id; producing them marks
    /// the regions painted.
    pub frames: Vec<FrameRequest>,
    /// When the runtime should tick again.
    pub wake: Wake,
}

impl<S: Spec> Kernel<S> {
    /// Phase 7 — publish. Runs once per tick (and once per settle pass):
    /// the revision compare is cheap, the assembly happens only on change.
    pub(crate) fn publish(&mut self, now: Seconds) -> PublishOut<S> {
        self.clock.begin(Phase::Publish);

        let seen = self.engine_revisions();
        let moved = seen != self.last_seen;
        let snapshot = if moved {
            self.last_seen = seen;
            self.revision.bump();
            Some(self.assemble(now))
        } else {
            None
        };

        let window_commands = self.windows.drain();
        let surfaces = self.windows.view().surfaces();
        let background = clear_color(self.windows.view().tokens());
        let animating = !self.anim.view().animating_windows().is_empty();
        let tick_view = CadenceTickView::new(&surfaces, background, animating);
        let frames = self.cadence.frames(now, &tick_view);
        let wake = self.cadence.next_wake(now, &tick_view);

        self.clock.idle();
        PublishOut {
            snapshot,
            window_commands,
            frames,
            wake,
        }
    }

    /// The snapshot of this publish: one view per window, ordered by
    /// `WindowId`, identities only (never the app's panel values).
    fn assemble(&self, now: Seconds) -> VisualSnapshot<S> {
        let wview = self.windows.view();
        let mut windows = Vec::new();
        for win in wview.ids() {
            let Some(wread) = wview.window(win) else { continue };
            let lview = self.layout.view().window(win);
            windows.push(WindowView {
                id: win,
                key: wread.key().clone(),
                geometry: *wread.geometry(),
                focused: wread.geometry().focused,
                cursor: wread.cursor(),
                chrome: lview.as_ref().map_or_else(ChromeView::default, |wv| wv.chrome_view()),
                edges: lview.as_ref().map_or_else(Vec::new, |wv| wv.edge_views()),
                dock: lview.as_ref().map_or_else(DockView::default, |wv| wv.dock_view()),
                overlays: self.overlays.view().snapshot(win),
                input: self.input.view().window(win).unwrap_or_else(InputView::default),
                cadence: self
                    .cadence
                    .view()
                    .window(win)
                    .unwrap_or_else(CadenceView::default),
            });
        }
        VisualSnapshot {
            revision: self.revision,
            at: now,
            windows,
            render: wview.render_info().clone(),
            theme_rev: wview.theme_rev(),
        }
    }
}

/// The frame clear colour: the theme's app-chrome surface packed as
/// `0xRRGGBBAA` (design §2.5's `FrameRequest::background`).
fn clear_color(tokens: &Tokens) -> u32 {
    match &tokens.semantic.surface_app_chrome {
        ColorValue::Solid(c) => u32::from_be_bytes([c.r, c.g, c.b, c.a]),
        _ => 0x0000_00FF,
    }
}
