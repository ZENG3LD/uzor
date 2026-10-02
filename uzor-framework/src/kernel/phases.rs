//! Phases 1, 2 and 4 of the frame step (design §4.2): command drain,
//! window / host lifecycle, and the engine clocks.

use crate::types::bus::{HostEvent, InputEnvelope, InputEvent};
use crate::types::command::{AppCommand, Domain};
use crate::types::ids::Seconds;
use crate::types::intent::Intent;
use crate::types::ops::{CadenceOp, FocusOp, InputOp, KeymapOp, LayoutOp, OverlayOp, WindowOp};
use crate::types::spec::Spec;

use super::Kernel;

impl<S: Spec> Kernel<S> {
    /// Phase 1 — Drain: every app command is applied to EXACTLY the engine
    /// its variant names (`AppCommand::target()` is total), commands first,
    /// so a command issued in the last intent callback is visible to this
    /// tick's routing.
    pub(super) fn drain(&mut self, now: Seconds, cmds: &mut Vec<AppCommand<S>>) {
        for cmd in cmds.drain(..) {
            match cmd.target() {
                Domain::Window => {
                    let op = match cmd {
                        AppCommand::Window(c) => WindowOp::Cmd(c),
                        AppCommand::Theme(c) => WindowOp::Theme(c),
                        AppCommand::Render(c) => WindowOp::Render(c),
                        AppCommand::Clipboard(c) => WindowOp::Clipboard(c),
                        AppCommand::Screenshot { win, ticket } => {
                            WindowOp::Screenshot { win, ticket }
                        }
                        AppCommand::Shutdown => WindowOp::Shutdown,
                        other => unreachable!("Domain::Window target, got {other:?}"),
                    };
                    let fx = self.windows.apply(op);
                    self.conduct_window_all(now, fx);
                }
                Domain::Layout => {
                    let AppCommand::Layout(c) = cmd else {
                        unreachable!("Domain::Layout target")
                    };
                    let fx = self.layout.apply(LayoutOp::Cmd(c));
                    self.conduct_layout(now, fx);
                }
                Domain::Overlay => {
                    let AppCommand::Overlay(c) = cmd else {
                        unreachable!("Domain::Overlay target")
                    };
                    let fx = self.overlays.apply(OverlayOp::Cmd { cmd: c, now });
                    self.conduct_overlay(now, fx);
                }
                Domain::Input => {
                    let AppCommand::Focus(c) = cmd else {
                        unreachable!("Domain::Input target")
                    };
                    let (win, op) = FocusOp::from_cmd(c);
                    let fx = self.input.apply(InputOp::Focus { win, now, op });
                    self.conduct_input(now, fx);
                }
                Domain::Keymap => {
                    let AppCommand::Keymap(c) = cmd else {
                        unreachable!("Domain::Keymap target")
                    };
                    let fx = self.keymap.apply(KeymapOp::Cmd(c));
                    // KeymapEffect is uninhabited.
                    let _ = fx;
                }
                Domain::Cadence => {
                    let AppCommand::Cadence(c) = cmd else {
                        unreachable!("Domain::Cadence target")
                    };
                    let fx = self.cadence.apply(CadenceOp::Cmd(c));
                    self.conduct_cadence(now, fx);
                }
            }
        }
    }

    /// Phase 2 — Lifecycle: window echoes land in the WindowEngine before
    /// any pointer event of the same tick is routed against stale rects;
    /// host facts become intents or window ops.
    pub(super) fn lifecycle(&mut self, now: Seconds, inputs: &[InputEnvelope]) {
        for env in inputs.iter() {
            match &env.event {
                InputEvent::Window(input) => {
                    let fx = self.windows.apply(WindowOp::Lifecycle {
                        win: env.window,
                        input: *input,
                    });
                    self.conduct_window_all(now, fx);
                    // The outer-rect echoes also feed layout (expand origin,
                    // drag-out projection, dwell fresh-move). The rect is
                    // read back from the WindowEngine after the echo: the
                    // Moved echo carries no size, Created no scale update.
                    if feeds_outer_rect(*input) {
                        let read = self.windows.view().window(env.window).map(|w| {
                            let g = w.geometry();
                            (
                                uzor::Rect::new(
                                    g.position.map_or(0.0, |(x, _)| x as f64),
                                    g.position.map_or(0.0, |(_, y)| y as f64),
                                    g.outer_size.width as f64,
                                    g.outer_size.height as f64,
                                ),
                                g.scale,
                            )
                        });
                        if let Some((rect, scale)) = read {
                            let fx = self.layout.apply(LayoutOp::OuterRect {
                                win: env.window,
                                now: env.t,
                                rect,
                                scale,
                            });
                            self.conduct_layout(now, fx);
                        }
                    }
                }
                InputEvent::Host(host) => match host {
                    HostEvent::Screenshot { ticket, png } => {
                        self.outbox.intents.push(Intent::Screenshot {
                            ticket: *ticket,
                            png: png.clone(),
                        });
                    }
                    HostEvent::Tray { id } => {
                        self.outbox.intents.push(Intent::Tray(*id));
                    }
                    HostEvent::RenderInfo(info) => {
                        let fx = self.windows.apply(WindowOp::RenderInfo(info.clone()));
                        self.conduct_window_all(now, fx);
                    }
                },
                _ => {}
            }
        }
    }

    /// Phase 4 — Engines: the only phase where time advances state.
    /// 4a cadence wheel, 4b animators, 4c layout tick, 4d reconcile.
    pub(super) fn engines(&mut self, now: Seconds) {
        // 4a: due deadlines -> conduct (overlay auto-close, app timers,
        // caret blink edges).
        let fx = self.cadence.tick(now);
        self.conduct_cadence(now, fx);

        // 4b: animator steps -> conduct (expand values resize the OS
        // window; fades repaint).
        let fx = self.anim.tick(now);
        self.conduct_anim(now, fx);

        // 4c: layout tick (snap-back, dwell, expand apply, coalesced
        // `LayoutChanged`).
        let fx = self.layout.tick(now);
        self.conduct_layout(now, fx);

        // 4d: reconcile — the input engine drops focus whose scope or
        // widget is gone and settles caret / selection state.
        let fx = self.input.tick(now);
        self.conduct_input(now, fx);
    }
}

/// Whether the echo moves the window's outer rect (design §3.8: expand
/// origin, drag-out projection and dwell read these).
fn feeds_outer_rect(input: crate::types::bus::WindowInput) -> bool {
    use crate::types::bus::WindowInput as W;
    matches!(
        input,
        W::Created { .. } | W::Moved { .. } | W::Resized { .. } | W::OuterRect { .. }
    )
}
