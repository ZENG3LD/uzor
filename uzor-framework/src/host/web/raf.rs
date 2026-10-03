//! The one RAF driver (design §2.4).
//!
//! DOM listeners enqueue; this loop is the only caller of [`Runtime::tick`]
//! and [`Runtime::paint`]. An idle page does not keep a frame requested.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::{Document, Element, HtmlCanvasElement, HtmlElement};

use super::executor::{self, WebFollowUp, WebSurface};
use super::listeners::{self, device_pixel_ratio, Queue, RafWake};
use super::paint::paint_frame;
use super::translate::physical_size;
use super::WEB_CAPS;
use crate::runtime::{Runtime, RuntimeConfig};
use crate::types::bus::{ClipboardResult, InputEnvelope, InputEvent, WindowInput};
use crate::types::error::FrameworkError;
use crate::types::frame::Wake;
use crate::types::ids::{Seconds, Ticket, WindowId};
use crate::types::window::{CursorMode, FullscreenMode, WindowCommand, WindowSpec};
use crate::{App, Handle, Spec, Waker};
use uzor::CursorIcon;

/// `Send` wrapper around the single-threaded RAF arm.
///
/// `wasm32-unknown-unknown` has no threads. The waker type is `Send` because
/// [`Handle`] is; the closure only schedules `requestAnimationFrame` on the
/// browser thread. Same precedent as `uzor_window_web::SendSyncCanvas`.
struct Arm(Rc<dyn Fn()>);

unsafe impl Send for Arm {}
unsafe impl Sync for Arm {}

/// Page-lifetime driver. Held alive by the waker cycle (the handle's waker
/// points back at this driver) plus the RAF closure.
struct RafDriver<S: Spec, A: App<Spec = S>> {
    runtime: Runtime<S, A>,
    _handle: Handle<S>,
    queue: Queue,
    canvas: HtmlCanvasElement,
    document: Document,
    wake: Option<RafWake>,
    raf_cb: Option<Closure<dyn FnMut(f64)>>,
    timeout_cb: Option<Closure<dyn FnMut()>>,
    raf: Option<i32>,
    timeout: Option<i32>,
    stopped: bool,
    needs_tick: bool,
    in_frame: Arc<AtomicBool>,
    kicked: Arc<AtomicBool>,
    cursor_css: String,
    cursor_hidden: bool,
    _listeners: Option<listeners::Listeners>,
}

/// Monotonic page clock, seconds. Same origin as the RAF timestamp.
pub(super) fn now_seconds() -> Seconds {
    let ms = web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or(0.0);
    Seconds(if ms.is_finite() { ms / 1000.0 } else { 0.0 })
}

/// Run `app` against the canvas element `canvas_id`.
///
/// Returns after the first frame is scheduled. The driver stays alive for
/// the life of the page; this is not a blocking desktop loop.
pub fn run_web<S, A>(canvas_id: &str, app: A, cfg: RuntimeConfig) -> Result<(), FrameworkError>
where
    S: Spec,
    A: App<Spec = S>,
{
    let window =
        web_sys::window().ok_or_else(|| FrameworkError::Host("no window object".into()))?;
    let document = window
        .document()
        .ok_or_else(|| FrameworkError::Host("no document object".into()))?;
    let canvas = document
        .get_element_by_id(canvas_id)
        .ok_or_else(|| FrameworkError::Host(format!("element '{canvas_id}' not found")))?
        .dyn_into::<HtmlCanvasElement>()
        .map_err(|_| FrameworkError::Host(format!("element '{canvas_id}' is not a canvas")))?;

    let _ = canvas.set_attribute("tabindex", "0");
    let html: &HtmlElement = canvas.unchecked_ref();
    let _ = html.style().set_property("touch-action", "none");
    let _ = html.style().set_property("outline", "none");

    let kicked = Arc::new(AtomicBool::new(false));
    let in_frame = Arc::new(AtomicBool::new(false));
    let once: Arc<OnceLock<Arm>> = Arc::new(OnceLock::new());
    let waker = {
        let kicked = Arc::clone(&kicked);
        let in_frame = Arc::clone(&in_frame);
        let once = Arc::clone(&once);
        Waker::new(move || {
            kicked.store(true, Ordering::Release);
            if in_frame.load(Ordering::Acquire) {
                return;
            }
            if let Some(arm) = once.get() {
                (arm.0)();
            }
        })
    };

    let (runtime, handle) = Runtime::new(app, cfg, waker);
    let queue = Queue::new();
    let driver = Rc::new(RefCell::new(RafDriver {
        runtime,
        _handle: handle,
        queue: queue.clone(),
        canvas: canvas.clone(),
        document,
        wake: None,
        raf_cb: None,
        timeout_cb: None,
        raf: None,
        timeout: None,
        stopped: false,
        needs_tick: false,
        in_frame: Arc::clone(&in_frame),
        kicked: Arc::clone(&kicked),
        cursor_css: "default".into(),
        cursor_hidden: false,
        _listeners: None,
    }));

    {
        let d = Rc::clone(&driver);
        let cb = Closure::wrap(Box::new(move |ts: f64| {
            if let Ok(mut guard) = d.try_borrow_mut() {
                guard.on_frame(ts);
            } else {
                log::warn!("raf callback re-entered; frame skipped");
            }
        }) as Box<dyn FnMut(f64)>);
        driver.borrow_mut().raf_cb = Some(cb);
    }
    {
        let d = Rc::clone(&driver);
        let cb = Closure::wrap(Box::new(move || {
            if let Ok(mut guard) = d.try_borrow_mut() {
                let now = now_seconds();
                guard.timeout = None;
                guard
                    .queue
                    .push_raw(InputEvent::Timer(crate::types::bus::TimerWake { now }));
                guard.ensure_scheduled();
            }
        }) as Box<dyn FnMut()>);
        driver.borrow_mut().timeout_cb = Some(cb);
    }

    let arm_driver = Rc::clone(&driver);
    let arm = Arm(Rc::new(move || {
        if let Ok(mut guard) = arm_driver.try_borrow_mut() {
            guard.ensure_scheduled();
        }
    }) as Rc<dyn Fn()>);
    let _ = once.set(arm);

    let wake = RafWake::new(Rc::new({
        let d = Rc::clone(&driver);
        move || {
            if let Ok(mut guard) = d.try_borrow_mut() {
                guard.kicked.store(true, Ordering::Release);
                if !guard.in_frame.load(Ordering::Acquire) {
                    guard.ensure_scheduled();
                }
            }
        }
    }) as Rc<dyn Fn()>);
    driver.borrow_mut().wake = Some(wake.clone());

    let listeners = listeners::install(&canvas, queue, wake).map_err(FrameworkError::from)?;
    driver.borrow_mut()._listeners = Some(listeners);
    driver.borrow_mut().ensure_scheduled();
    Ok(())
}

/// Same as [`run_web`]. The demo entry (brief F11) calls this name.
pub fn start_web<S, A>(canvas_id: &str, app: A, cfg: RuntimeConfig) -> Result<(), FrameworkError>
where
    S: Spec,
    A: App<Spec = S>,
{
    run_web(canvas_id, app, cfg)
}

impl<S: Spec, A: App<Spec = S>> RafDriver<S, A> {
    fn on_frame(&mut self, now_ms: f64) {
        self.raf = None;
        if self.stopped {
            return;
        }
        self.in_frame.store(true, Ordering::Release);
        let now = if now_ms.is_finite() {
            Seconds(now_ms / 1000.0)
        } else {
            now_seconds()
        };
        while let Some(env) = self.queue.pop() {
            self.runtime.push_input(env);
        }
        let out = self.runtime.tick(now);
        self.execute_all(out.window_commands);
        for req in &out.frames {
            if let Err(err) = paint_frame(&mut self.runtime, &self.canvas, req) {
                log::warn!("paint: {err}");
            }
        }
        let kicked = {
            // Drop the in-frame flag before reading `kicked`, so a waker that
            // runs after this point schedules the RAF itself.
            self.in_frame.store(false, Ordering::Release);
            self.kicked.swap(false, Ordering::AcqRel)
        };
        if self.stopped || self.runtime.should_exit() {
            self.stop();
            return;
        }
        let force = self.needs_tick || kicked;
        self.needs_tick = false;
        if force {
            self.ensure_scheduled();
            return;
        }
        match out.wake {
            Wake::Immediate => self.ensure_scheduled(),
            Wake::At(t) => self.arm_timeout(t),
            Wake::Idle => self.disarm(),
        }
    }

    fn execute_all(&mut self, cmds: Vec<(WindowId, WindowCommand)>) {
        for (id, cmd) in cmds {
            self.execute_one(id, cmd);
        }
    }

    fn execute_one(&mut self, id: WindowId, cmd: WindowCommand) {
        let canvas = self.canvas.clone();
        let document = self.document.clone();
        let raw_delta = self.queue.raw_delta_flag();
        let redraw = Cell::new(false);
        let follow = {
            let mut surface = DomSurface {
                canvas: &canvas,
                document: &document,
                raw_delta: &raw_delta,
                redraw: &redraw,
                cursor_css: &mut self.cursor_css,
                cursor_hidden: &mut self.cursor_hidden,
            };
            executor::execute(&mut surface, cmd)
        };
        if redraw.get() {
            self.needs_tick = true;
        }
        let Some(follow) = follow else {
            return;
        };
        match follow {
            WebFollowUp::ClipboardWrite(text) => self.write_clipboard(&text),
            WebFollowUp::ClipboardRead(ticket) => self.read_clipboard(ticket),
            WebFollowUp::Spawn(spec) => self.bind_spawn(id, spec),
            WebFollowUp::Close => self.close_window(id),
            WebFollowUp::ExitApp => self.stop(),
            WebFollowUp::OuterRectEcho { position, size } => {
                self.push_runtime(
                    id,
                    InputEvent::Window(WindowInput::OuterRect { position, size }),
                );
            }
        }
    }

    fn bind_spawn(&mut self, id: WindowId, spec: WindowSpec) {
        if self.queue.window().is_some() {
            log::warn!(
                "web host is single-canvas (multi_window: false); ignoring spawn '{}'",
                spec.title
            );
            return;
        }
        self.queue.bind_window(id);
        self.document.set_title(&spec.title);
        let (size, dpr) = physical_size(
            self.canvas.client_width(),
            self.canvas.client_height(),
            device_pixel_ratio(),
        );
        self.push_runtime(
            id,
            InputEvent::Window(WindowInput::Created {
                size,
                dpr,
                position: None,
                caps: WEB_CAPS,
            }),
        );
    }

    fn close_window(&mut self, id: WindowId) {
        if self.queue.window() != Some(id) {
            return;
        }
        self.push_runtime(id, InputEvent::Window(WindowInput::Destroyed));
        self.queue.clear_window();
        self.stop();
    }

    fn push_runtime(&mut self, window: WindowId, event: InputEvent) {
        self.runtime.push_input(InputEnvelope {
            window,
            t: now_seconds(),
            event,
        });
        self.needs_tick = true;
    }

    fn write_clipboard(&mut self, text: &str) {
        *self.queue.copy_buf().borrow_mut() = Some(text.to_string());
        let Some(window) = web_sys::window() else {
            return;
        };
        let clipboard = window.navigator().clipboard();
        let _ = clipboard.write_text(text);
    }

    fn read_clipboard(&mut self, ticket: Ticket) {
        let Some(window_id) = self.queue.window() else {
            return;
        };
        if let Some(text) = self.queue.paste_buf().borrow_mut().take() {
            self.push_runtime(
                window_id,
                InputEvent::Clipboard(ClipboardResult {
                    ticket,
                    text: Some(text),
                }),
            );
            return;
        }
        self.queue.pending_reads().borrow_mut().push_back(ticket);
        let Some(win) = web_sys::window() else {
            self.finish_read(window_id, ticket, None);
            return;
        };
        let promise = win.navigator().clipboard().read_text();
        let queue = self.queue.clone();
        let wake = self.wake.clone();
        let on_ok = Closure::once(move |value: wasm_bindgen::JsValue| {
            let text = value.as_string().filter(|s| !s.is_empty());
            deliver_read(&queue, &wake, ticket, text);
        });
        let queue_err = self.queue.clone();
        let wake_err = self.wake.clone();
        let on_err = Closure::once(move |_err: wasm_bindgen::JsValue| {
            deliver_read(&queue_err, &wake_err, ticket, None);
        });
        let _ = promise.then2(&on_ok, &on_err);
        on_ok.forget();
        on_err.forget();
    }

    fn finish_read(&mut self, window: WindowId, ticket: Ticket, text: Option<String>) {
        let pending_reads = self.queue.pending_reads();
        let mut pending = pending_reads.borrow_mut();
        if let Some(pos) = pending.iter().position(|t| *t == ticket) {
            pending.remove(pos);
        }
        drop(pending);
        self.push_runtime(
            window,
            InputEvent::Clipboard(ClipboardResult { ticket, text }),
        );
    }

    fn ensure_scheduled(&mut self) {
        if self.stopped || self.raf.is_some() {
            return;
        }
        self.cancel_timeout();
        let Some(window) = web_sys::window() else {
            return;
        };
        let Some(cb) = self.raf_cb.as_ref() else {
            return;
        };
        let function: &js_sys::Function = cb.as_ref().unchecked_ref();
        match window.request_animation_frame(function) {
            Ok(id) => self.raf = Some(id),
            Err(err) => log::warn!("requestAnimationFrame failed: {err:?}"),
        }
    }

    fn arm_timeout(&mut self, at: Seconds) {
        if self.stopped {
            return;
        }
        self.cancel_raf();
        let now = now_seconds().get();
        let delay = (at.get() - now) * 1000.0;
        if !delay.is_finite() || delay <= 0.0 {
            self.ensure_scheduled();
            return;
        }
        let delay_ms = delay.min(60_000_000.0).ceil() as i32;
        self.cancel_timeout();
        let Some(window) = web_sys::window() else {
            return;
        };
        let Some(cb) = self.timeout_cb.as_ref() else {
            return;
        };
        let function: &js_sys::Function = cb.as_ref().unchecked_ref();
        match window.set_timeout_with_callback_and_timeout_and_arguments_0(function, delay_ms) {
            Ok(id) => self.timeout = Some(id),
            Err(err) => log::warn!("setTimeout failed: {err:?}"),
        }
    }

    fn disarm(&mut self) {
        self.cancel_raf();
        self.cancel_timeout();
    }

    fn stop(&mut self) {
        self.stopped = true;
        self.disarm();
    }

    fn cancel_raf(&mut self) {
        if let Some(id) = self.raf.take() {
            if let Some(window) = web_sys::window() {
                window.cancel_animation_frame(id).ok();
            }
        }
    }

    fn cancel_timeout(&mut self) {
        if let Some(id) = self.timeout.take() {
            if let Some(window) = web_sys::window() {
                window.clear_timeout_with_handle(id);
            }
        }
    }
}

fn deliver_read(queue: &Queue, wake: &Option<RafWake>, ticket: Ticket, text: Option<String>) {
    let pending_reads = queue.pending_reads();
    let mut pending = pending_reads.borrow_mut();
    let Some(pos) = pending.iter().position(|t| *t == ticket) else {
        return;
    };
    pending.remove(pos);
    drop(pending);
    queue.push_raw(InputEvent::Clipboard(ClipboardResult { ticket, text }));
    if let Some(wake) = wake {
        wake.request();
    }
}

struct DomSurface<'a> {
    canvas: &'a HtmlCanvasElement,
    document: &'a Document,
    raw_delta: &'a Cell<bool>,
    redraw: &'a Cell<bool>,
    cursor_css: &'a mut String,
    cursor_hidden: &'a mut bool,
}

impl DomSurface<'_> {
    fn apply_cursor(&self) {
        let css = if *self.cursor_hidden {
            "none"
        } else {
            self.cursor_css.as_str()
        };
        let html: &HtmlElement = self.canvas.unchecked_ref();
        let _ = html.style().set_property("cursor", css);
    }
}

impl WebSurface for DomSurface<'_> {
    fn set_title(&mut self, title: &str) {
        self.document.set_title(title);
    }
    fn set_cursor(&mut self, icon: CursorIcon) {
        *self.cursor_css = icon.css_name().to_string();
        if !*self.cursor_hidden {
            self.apply_cursor();
        }
    }
    fn set_cursor_visible(&mut self, on: bool) {
        *self.cursor_hidden = !on;
        self.apply_cursor();
    }
    fn set_cursor_mode(&mut self, mode: CursorMode) {
        let el: &Element = self.canvas.unchecked_ref();
        match mode {
            CursorMode::LockedHidden => {
                self.raw_delta.set(true);
                let _ = el.request_pointer_lock();
            }
            // `Confined` has no browser API; degrade to a free cursor.
            CursorMode::Normal | CursorMode::Confined => {
                self.raw_delta.set(false);
                let _ = self.document.exit_pointer_lock();
            }
        }
    }
    fn set_fullscreen(&mut self, mode: FullscreenMode) {
        match mode {
            FullscreenMode::Off => {
                let _ = self.document.exit_fullscreen();
            }
            FullscreenMode::Borderless => {
                let el: &Element = self.canvas.unchecked_ref();
                let _ = el.request_fullscreen();
            }
        }
    }
    fn focus(&mut self) {
        let html: &HtmlElement = self.canvas.unchecked_ref();
        let _ = html.focus();
    }
    fn request_redraw(&mut self) {
        self.redraw.set(true);
    }
}
