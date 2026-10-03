//! Phases 5-6 — Compose and Responses (design §4.2, §4.5 draw order,
//! §6.3 hook contexts).
//!
//! One compose paints one window: the truth snapshot (`VisualView`) is
//! built BEFORE `BeginFrame` clears the last frame's clicks, the app
//! declares its `ChromeModel` (the layout engine stays the single writer —
//! the kernel diffs and applies on change), widgets register between
//! `BeginFrame` and `EndFrame`, and phase 6 turns this frame's clicks on
//! overlay parts into overlay ops and drains the hooks' declared effects.
//!
//! F7 painted the chrome strip as a flat token box. F11 draws that strip
//! with `draw_chrome` and paints dock headers, tab bars, splitters, the
//! drag ghost and the focus ring (`furniture`). Overlay frames stay plain
//! boxes; the app's `overlay_body` draws the body.

use uzor::input::driver::CoordinatorDriver;
use uzor::input::{LayerId, Sense};
use uzor::render::{InvalidateBits, RenderContext};

use crate::handle::{App, FrameTime, HookOp, HookOps, OverlayCx, PanelCx, VisualView, Widgets};
use crate::types::command::{AppCommand, LayoutCmd};
use crate::types::ids::{Seconds, WindowId};
use crate::types::ops::{FocusOp, InputOp, LayoutOp, OverlayOp, ScopeOwner, WindowOp};
use crate::types::overlay_model::OverlayModel;
use crate::types::spec::Spec;
use crate::types::window::{Point, WindowCommand};

use super::{Kernel, Phase};

/// What a compose hands back to the runtime: the commands the window's
/// hooks declared (design §6.3 — applied by the next settle pass's drain).
/// Intents produced in phase 6 go straight into the kernel outbox.
pub(crate) struct ComposeOut<S: Spec> {
    /// Commands declared by the window's hooks, in declaration order.
    pub commands: Vec<AppCommand<S>>,
}

impl<S: Spec> Kernel<S> {
    /// Phases 5-6 for one window. `render` paints at the window's own
    /// scale; time comes from the runtime, never from a clock.
    pub(crate) fn compose(
        &mut self,
        now: Seconds,
        win: WindowId,
        app: &mut impl App<Spec = S>,
        render: &mut dyn RenderContext,
    ) -> ComposeOut<S> {
        self.clock.begin(Phase::Compose);
        let mut out = ComposeOut {
            commands: Vec::new(),
        };
        let mut hook_ops: Vec<HookOp<S>> = Vec::new();

        let Some(dpr) = self.windows.view().window(win).map(|w| w.geometry().scale) else {
            self.clock.idle();
            return out;
        };
        let tokens = self.windows.view().tokens().clone();

        // The frame's truth, built BEFORE `BeginFrame` clears the last
        // frame's clicks (the one-frame-latency immediate-mode read).
        let view = self.visual_view(win);

        // ChromeModel duality: the app declares per frame, the layout
        // engine is the single writer; apply only on change.
        let declared = app.chrome_model(win);
        let same = self
            .layout
            .view()
            .window(win)
            .is_some_and(|wv| wv.chrome_model().same_as(&declared));
        if !same {
            let fx = self.layout.apply(LayoutOp::Cmd(LayoutCmd::SetChromeModel {
                win,
                model: declared,
            }));
            self.conduct_layout(now, fx);
        }

        let time = FrameTime {
            now,
            dt: now.get() - self.last_frame.get(&win).copied().unwrap_or(now).get(),
        };

        // `BeginFrame`: the cooked pointer input enters the coordinator.
        let fx = self.input.apply(InputOp::BeginFrame { win, now });
        self.conduct_input(now, fx);

        // -- dock leaves (main layer), chrome strip above them -----------
        {
            let lview = self.layout.view();
            if let Some(wv) = lview.window(win) {
                // The chrome strip (plain paint; the lib `draw_chrome`
                // mapping lands with F11 goldens). Chrome hit-testing is
                // the layout engine's own — nothing registers here.
                // Chrome strip: the same composite the hit-test classifies.
                super::furniture::paint_chrome(render, wv, &tokens);

                let leaves = wv.dock_view().leaves;
                for leaf_view in &leaves {
                    if leaf_view.hidden {
                        continue;
                    }
                    let Some(leaf) = wv.dock().tree().leaf(leaf_view.leaf) else {
                        continue;
                    };
                    let Some(panel) = leaf.active_panel() else {
                        continue;
                    };
                    let mut hooks = HookOps::default();
                    {
                        let Some((coord, states)) = self.input.compose_parts(win) else {
                            break;
                        };
                        let mut cx = PanelCx {
                            window: win,
                            leaf: leaf_view.leaf,
                            panel,
                            index: leaf.active_tab,
                            rect: leaf_view.rect,
                            dpr,
                            tokens: &tokens,
                            render: &mut *render,
                            widgets: Widgets {
                                coord,
                                states,
                                layer: LayerId::main(),
                            },
                            view: &view,
                            time,
                            out: &mut hooks,
                        };
                        app.panel(&mut cx);
                    }
                    hook_ops.append(&mut hooks.ops);
                }
                // Floating panel bodies do not compose in F7: their rects
                // and tabs publish (snapshot `floating`), the engine drags
                // and snaps them, but the `DockState` exposes no floating
                // panel values to hand to `App::panel` (recorded gap).
                super::furniture::paint_dock(render, wv, &tokens);
            }
        }

        // -- overlays, bottom to top -------------------------------------
        let overlays = self.overlays.view().snapshot(win);
        for (z, ov) in overlays.iter().enumerate() {
            let model = app.overlay_model(win, ov.id);
            let layer = overlay_layer(ov.slot);
            let mut hooks = HookOps::default();
            {
                let Some((coord, states)) = self.input.compose_parts(win) else {
                    break;
                };
                // Layers are per-frame in the coordinator: the stack is
                // rebuilt every compose, in snapshot order, with the modal
                // flag driving the hit-test barrier.
                coord.push_layer(layer.clone(), 1 + z as u32, ov.modal);

                // Modal backdrop (dimming the world is the frame's job).
                if let OverlayModel::Modal(m) = &model {
                    if m.backdrop {
                        let viewport = self
                            .windows
                            .view()
                            .window(win)
                            .map_or_else(Default::default, |w| w.geometry().viewport);
                        render.set_fill_color_alpha("#000000", 0.35);
                        render.fill_rect(0.0, 0.0, viewport.width, viewport.height);
                    }
                }

                // The frame itself: a click-catching registration (inside
                // clicks must not fall through to the dock) and a plain
                // token-colored box.
                coord.register_on_layer(ov_host_id(ov.slot), ov.rect, Sense::CLICK, &layer);
                let css = tokens.semantic.surface_floating.to_css();
                render.set_fill_color(&css);
                render.fill_rect(ov.rect.x, ov.rect.y, ov.rect.width, ov.rect.height);

                let mut cx = OverlayCx {
                    window: win,
                    id: ov.id,
                    // F7 plain frame: the body fills the frame (the lib
                    // `body_rect` of a header / tab / footer frame is F8).
                    body: ov.rect,
                    rect: ov.rect,
                    dpr,
                    tokens: &tokens,
                    render: &mut *render,
                    widgets: Widgets {
                        coord,
                        states,
                        layer,
                    },
                    view: &view,
                    time,
                    out: &mut hooks,
                };
                app.overlay_body(&mut cx);
            }
            hook_ops.append(&mut hooks.ops);
        }

        // Focus ring over whatever registered this frame (the view's focus
        // is the last evaluated one; a field focused by the previous tick
        // has re-registered above).
        {
            let ring = view.focused.as_ref().and_then(|id| {
                self.input
                    .view()
                    .coordinator(win)
                    .and_then(|c| c.widget_rect(id))
            });
            super::furniture::paint_focus_ring(render, &tokens, ring);
        }

        // `EndFrame`: this frame's registrations evaluate (hover, clicks).
        let fx = self.input.apply(InputOp::EndFrame { win, now });
        self.conduct_input(now, fx);

        // Scope members: an overlay opens its scope with an empty member
        // list (blocking all focus); once its body composed, the members
        // are this frame's focus-wanting registrations on its layer.
        for ov in &overlays {
            if ov.scope_depth == 0 {
                continue;
            }
            let members = self
                .input
                .view()
                .coordinator(win)
                .map(|c| c.focusable_ids(Some(&overlay_layer(ov.slot))))
                .unwrap_or_default();
            let fx = self.input.apply(InputOp::Focus {
                win,
                now,
                op: FocusOp::SetMembers {
                    owner: ScopeOwner::Overlay(ov.slot),
                    members,
                },
            });
            self.conduct_input(now, fx);
        }

        // -- phase 6: responses -------------------------------------------
        // This frame's clicks on overlay parts resolve through the
        // window's dispatcher (frame buttons, body composites); every
        // other click stays in the view for the app to read next frame.
        let clicked: Vec<_> = self.input.view().clicked(win).to_vec();
        if !clicked.is_empty() {
            let cursor = self
                .input
                .view()
                .coordinator(win)
                .and_then(|c| c.pointer_pos())
                .map_or_else(Point::default, |(x, y)| Point::new(x, y));
            for (widget, _count) in clicked {
                let fx = self.overlays.apply(OverlayOp::Click {
                    win,
                    widget,
                    cursor,
                });
                self.conduct_overlay(now, fx);
            }
        }

        // The hooks' declared effects (design §6.3): repaint requests
        // invalidate, IME areas go to the host queue, commands travel back
        // to the runtime for the next settle pass's drain.
        for op in hook_ops {
            match op {
                HookOp::RequestFrame => self.invalidate(win, None, InvalidateBits::ALL),
                HookOp::ImeArea(rect) => {
                    let fx = self.windows.apply(WindowOp::Enqueue {
                        win,
                        cmd: WindowCommand::SetImeCursorArea(rect),
                    });
                    self.conduct_window_all(now, fx);
                }
                HookOp::Command(cmd) => out.commands.push(cmd),
            }
        }

        self.last_frame.insert(win, now);
        self.clock.idle();
        out
    }

    /// The hook-read truth of a window at compose start (design §5.2):
    /// hover / press / focus / capture from the input engine's evaluated
    /// state, the last frame's clicks and the pending scroll, the open
    /// overlay stack bottom to top.
    fn visual_view(&self, win: WindowId) -> VisualView<S> {
        let iview = self.input.view();
        let input = iview.window(win).unwrap_or_default();
        let mut view = VisualView::<S>::default();
        view.hovered = input.hovered;
        view.pressed = input.pressed;
        view.focused = input.focused;
        view.captured = input.captured;
        view.clicked = iview.clicked(win).to_vec();
        view.scroll = iview.pending_scroll(win);
        view.open_overlays = self
            .overlays
            .view()
            .snapshot(win)
            .iter()
            .map(|o| o.id)
            .collect();
        view
    }
}

/// The coordinator layer of an overlay instance (`overlay:{slot}` — the
/// name the scope-members collection and the body registrations share).
fn overlay_layer(slot: crate::types::ids::OverlaySlot) -> LayerId {
    LayerId::new(&format!("overlay:{}", slot.0))
}

/// The frame's widget id (`overlay-{slot}` — the prefix the window's
/// `ClickDispatcher` resolves `{host}:close` / `{host}:tab:*` under).
fn ov_host_id(slot: crate::types::ids::OverlaySlot) -> uzor::WidgetId {
    uzor::WidgetId::from(format!("overlay-{}", slot.0))
}
