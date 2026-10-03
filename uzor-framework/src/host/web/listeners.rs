//! DOM listeners (design §2.4).
//!
//! Closures only enqueue [`InputEnvelope`]s and call [`RafWake::request`].
//! They never touch the [`Runtime`](crate::Runtime). The event list matches
//! `uzor-window-web`'s `WebWindowProvider` (pointer capture, wheel, keys,
//! touch, focus, resize, clipboard) plus composition events, file drop,
//! `ResizeObserver`, `prefers-color-scheme`, `visibilitychange` and
//! `beforeunload` (design §2.2 web producers).

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{
    Document, DragEvent, Element, Event, EventTarget, HtmlCanvasElement, KeyboardEvent, MouseEvent,
    WheelEvent,
};

use super::raf::now_seconds;
use super::translate::{
    canvas_local, composition_from_dom, key_from_dom, mods, physical_size, pointer_from_dom,
    raw_delta, touch_from_local, wheel_from_css_px,
};
use super::WebHostError;
use crate::types::bus::{
    ClipboardResult, DropInput, DroppedFile, InputEnvelope, InputEvent, WindowInput,
};
use crate::types::ids::{Seconds, Ticket, WindowId};
use uzor_window_web::normalize_wheel_delta_to_css_px;

/// Idempotent "please run a frame" handle. Shared by every DOM closure.
#[derive(Clone)]
pub struct RafWake {
    arm: Rc<dyn Fn()>,
}

impl RafWake {
    pub(super) fn new(arm: Rc<dyn Fn()>) -> Self {
        Self { arm }
    }

    /// Schedule one frame if one is not already armed.
    pub fn request(&self) {
        (self.arm)();
    }
}

/// Single-thread input queue (ban F5 allows `RefCell` in `host/*`).
///
/// Events that arrive before the canvas is bound to a [`WindowId`] are held
/// and stamped on bind. Pre-bind resizes are dropped: `Created` carries the
/// initial size.
#[derive(Clone)]
pub struct Queue {
    events: Rc<RefCell<VecDeque<InputEnvelope>>>,
    early: Rc<RefCell<Vec<(Seconds, InputEvent)>>>,
    window: Rc<Cell<Option<WindowId>>>,
    raw_delta: Rc<Cell<bool>>,
    pending_reads: Rc<RefCell<VecDeque<Ticket>>>,
    paste_buf: Rc<RefCell<Option<String>>>,
    copy_buf: Rc<RefCell<Option<String>>>,
}

impl Queue {
    pub(super) fn new() -> Self {
        Self {
            events: Rc::new(RefCell::new(VecDeque::new())),
            early: Rc::new(RefCell::new(Vec::new())),
            window: Rc::new(Cell::new(None)),
            raw_delta: Rc::new(Cell::new(false)),
            pending_reads: Rc::new(RefCell::new(VecDeque::new())),
            paste_buf: Rc::new(RefCell::new(None)),
            copy_buf: Rc::new(RefCell::new(None)),
        }
    }

    /// The bound window, if the first `Spawn` has been applied.
    pub fn window(&self) -> Option<WindowId> {
        self.window.get()
    }

    pub(super) fn raw_delta_flag(&self) -> Rc<Cell<bool>> {
        Rc::clone(&self.raw_delta)
    }

    pub(super) fn paste_buf(&self) -> Rc<RefCell<Option<String>>> {
        Rc::clone(&self.paste_buf)
    }

    pub(super) fn copy_buf(&self) -> Rc<RefCell<Option<String>>> {
        Rc::clone(&self.copy_buf)
    }

    pub(super) fn pending_reads(&self) -> Rc<RefCell<VecDeque<Ticket>>> {
        Rc::clone(&self.pending_reads)
    }

    /// Enqueue one event. Unbound windows buffer it.
    pub fn push_raw(&self, event: InputEvent) {
        let t = now_seconds();
        if let Some(window) = self.window.get() {
            self.events
                .borrow_mut()
                .push_back(InputEnvelope { window, t, event });
        } else {
            self.early.borrow_mut().push((t, event));
        }
    }

    /// Pop the oldest stamped envelope.
    pub fn pop(&self) -> Option<InputEnvelope> {
        self.events.borrow_mut().pop_front()
    }

    /// Bind the canvas to `id` and stamp buffered events.
    pub fn bind_window(&self, id: WindowId) {
        self.window.set(Some(id));
        let early = std::mem::take(&mut *self.early.borrow_mut());
        for (t, event) in early {
            if matches!(event, InputEvent::Window(WindowInput::Resized { .. })) {
                continue;
            }
            self.events.borrow_mut().push_back(InputEnvelope {
                window: id,
                t,
                event,
            });
        }
    }

    pub(super) fn clear_window(&self) {
        self.window.set(None);
    }
}

/// Closures and observers kept alive for the page.
pub struct Listeners {
    _closures: Vec<Closure<dyn FnMut(Event)>>,
    _resize: Option<web_sys::ResizeObserver>,
    _media: Vec<web_sys::MediaQueryList>,
}

/// Install the DOM listeners. Failures forget already-registered closures so
/// dropping them cannot abort the wasm module.
pub fn install(
    canvas: &HtmlCanvasElement,
    q: Queue,
    wake: RafWake,
) -> Result<Listeners, WebHostError> {
    let mut closures = Vec::new();
    let window = web_sys::window().ok_or_else(|| WebHostError::new("no window object"))?;
    let document = window
        .document()
        .ok_or_else(|| WebHostError::new("no document object"))?;

    let canvas_target: &EventTarget = canvas.unchecked_ref();
    let window_target: &EventTarget = window.unchecked_ref();

    if let Err(err) = install_pointers(canvas, canvas_target, &q, &wake, &mut closures) {
        forget_all(closures);
        return Err(err);
    }
    if let Err(err) = install_wheel(canvas, canvas_target, &q, &wake, &mut closures) {
        forget_all(closures);
        return Err(err);
    }
    let composing = Rc::new(Cell::new(false));
    if let Err(err) = install_keys(window_target, &q, &wake, &composing, &mut closures) {
        forget_all(closures);
        return Err(err);
    }
    if let Err(err) = install_composition(window_target, &q, &wake, &composing, &mut closures) {
        forget_all(closures);
        return Err(err);
    }
    if let Err(err) = install_touch(canvas, canvas_target, &q, &wake, &mut closures) {
        forget_all(closures);
        return Err(err);
    }
    if let Err(err) = install_focus(window_target, &q, &wake, &mut closures) {
        forget_all(closures);
        return Err(err);
    }
    let resize = match install_resize(canvas, window_target, &q, &wake, &mut closures) {
        Ok(obs) => obs,
        Err(err) => {
            forget_all(closures);
            return Err(err);
        }
    };
    let media = match install_theme_and_dpr(&window, &q, &wake, &mut closures, canvas) {
        Ok(list) => list,
        Err(err) => {
            forget_all(closures);
            return Err(err);
        }
    };
    if let Err(err) = install_visibility(&document, &q, &wake, &mut closures) {
        forget_all(closures);
        return Err(err);
    }
    if let Err(err) = install_before_unload(window_target, &q, &wake, &mut closures) {
        forget_all(closures);
        return Err(err);
    }
    if let Err(err) = install_clipboard(&document, &q, &wake, &mut closures) {
        forget_all(closures);
        return Err(err);
    }
    if let Err(err) = install_drop(canvas_target, &q, &wake, &mut closures) {
        forget_all(closures);
        return Err(err);
    }

    Ok(Listeners {
        _closures: closures,
        _resize: resize,
        _media: media,
    })
}

fn forget_all(closures: Vec<Closure<dyn FnMut(Event)>>) {
    for c in closures {
        c.forget();
    }
}

fn host_err(context: &str, err: JsValue) -> WebHostError {
    WebHostError::new(format!("{context}: {err:?}"))
}

fn listen(
    target: &EventTarget,
    ty: &str,
    closure: &Closure<dyn FnMut(Event)>,
    passive: bool,
) -> Result<(), WebHostError> {
    if passive {
        target
            .add_event_listener_with_callback(ty, closure.as_ref().unchecked_ref())
            .map_err(|e| host_err(ty, e))
    } else {
        let opts = web_sys::AddEventListenerOptions::new();
        opts.set_passive(false);
        target
            .add_event_listener_with_callback_and_add_event_listener_options(
                ty,
                closure.as_ref().unchecked_ref(),
                &opts,
            )
            .map_err(|e| host_err(ty, e))
    }
}

fn event_mods(ev: &MouseEvent) -> uzor::input::ModifierKeys {
    mods(ev.shift_key(), ev.ctrl_key(), ev.alt_key(), ev.meta_key())
}

fn install_pointers(
    canvas: &HtmlCanvasElement,
    target: &EventTarget,
    q: &Queue,
    wake: &RafWake,
    out: &mut Vec<Closure<dyn FnMut(Event)>>,
) -> Result<(), WebHostError> {
    for kind in [
        "pointerdown",
        "pointermove",
        "pointerup",
        "pointercancel",
        "pointerenter",
        "pointerleave",
    ] {
        let q = q.clone();
        let wake = wake.clone();
        let canvas = canvas.clone();
        let kind_owned = kind.to_string();
        let closure = Closure::wrap(Box::new(move |raw: Event| {
            if kind_owned == "pointerdown" {
                if let Ok(pid) = js_sys::Reflect::get(raw.as_ref(), &JsValue::from_str("pointerId"))
                {
                    if let Some(id) = pid.as_f64() {
                        let el: &Element = canvas.unchecked_ref();
                        let _ = el.set_pointer_capture(id as i32);
                    }
                }
            }
            let Ok(ev) = raw.dyn_into::<MouseEvent>() else {
                return;
            };
            let input = if q.raw_delta.get() && kind_owned == "pointermove" {
                Some(raw_delta(ev.movement_x() as f64, ev.movement_y() as f64))
            } else {
                pointer_from_dom(
                    &kind_owned,
                    ev.offset_x() as f64,
                    ev.offset_y() as f64,
                    ev.button(),
                    event_mods(&ev),
                )
            };
            if let Some(input) = input {
                q.push_raw(input);
                wake.request();
            }
        }) as Box<dyn FnMut(Event)>);
        listen(target, &kind, &closure, false)?;
        out.push(closure);
    }

    let closure = Closure::wrap(Box::new(move |raw: Event| {
        raw.prevent_default();
    }) as Box<dyn FnMut(Event)>);
    listen(target, "contextmenu", &closure, false)?;
    out.push(closure);
    Ok(())
}

fn install_wheel(
    canvas: &HtmlCanvasElement,
    target: &EventTarget,
    q: &Queue,
    wake: &RafWake,
    out: &mut Vec<Closure<dyn FnMut(Event)>>,
) -> Result<(), WebHostError> {
    let q = q.clone();
    let wake = wake.clone();
    let canvas = canvas.clone();
    let closure = Closure::wrap(Box::new(move |raw: Event| {
        raw.prevent_default();
        let Ok(ev) = raw.dyn_into::<WheelEvent>() else {
            return;
        };
        let (dx, dy) = normalize_wheel_delta_to_css_px(&ev, &canvas);
        q.push_raw(wheel_from_css_px(
            ev.offset_x() as f64,
            ev.offset_y() as f64,
            dx,
            dy,
            event_mods(&ev),
        ));
        wake.request();
    }) as Box<dyn FnMut(Event)>);
    listen(target, "wheel", &closure, false)?;
    out.push(closure);
    Ok(())
}

fn install_keys(
    target: &EventTarget,
    q: &Queue,
    wake: &RafWake,
    composing: &Rc<Cell<bool>>,
    out: &mut Vec<Closure<dyn FnMut(Event)>>,
) -> Result<(), WebHostError> {
    for kind in ["keydown", "keyup"] {
        let q = q.clone();
        let wake = wake.clone();
        let composing = Rc::clone(composing);
        let kind_owned = kind.to_string();
        let closure = Closure::wrap(Box::new(move |raw: Event| {
            let Ok(ev) = raw.dyn_into::<KeyboardEvent>() else {
                return;
            };
            if kind_owned == "keydown" && (ev.ctrl_key() || ev.meta_key()) {
                match ev.code().as_str() {
                    "KeyA" | "KeyS" | "KeyF" | "KeyZ" | "KeyY" => ev.prevent_default(),
                    _ => {}
                }
            }
            if kind_owned == "keydown" && ev.code() == "Tab" {
                ev.prevent_default();
            }
            let allow_text = !composing.get();
            if let Some(input) = key_from_dom(
                &kind_owned,
                &ev.code(),
                &ev.key(),
                ev.repeat(),
                mods(ev.shift_key(), ev.ctrl_key(), ev.alt_key(), ev.meta_key()),
                allow_text,
            ) {
                q.push_raw(input);
                wake.request();
            }
        }) as Box<dyn FnMut(Event)>);
        listen(target, kind, &closure, false)?;
        out.push(closure);
    }
    Ok(())
}

fn install_composition(
    target: &EventTarget,
    q: &Queue,
    wake: &RafWake,
    composing: &Rc<Cell<bool>>,
    out: &mut Vec<Closure<dyn FnMut(Event)>>,
) -> Result<(), WebHostError> {
    for kind in ["compositionstart", "compositionupdate", "compositionend"] {
        let q = q.clone();
        let wake = wake.clone();
        let composing = Rc::clone(composing);
        let kind_owned = kind.to_string();
        let closure = Closure::wrap(Box::new(move |raw: Event| {
            let data = raw
                .dyn_ref::<web_sys::CompositionEvent>()
                .map(|ev| ev.data().unwrap_or_default())
                .unwrap_or_default();
            match kind_owned.as_str() {
                "compositionstart" => composing.set(true),
                "compositionend" => composing.set(false),
                _ => {}
            }
            if let Some(input) = composition_from_dom(&kind_owned, &data) {
                q.push_raw(input);
                wake.request();
            }
        }) as Box<dyn FnMut(Event)>);
        listen(target, kind, &closure, true)?;
        out.push(closure);
    }
    Ok(())
}

fn install_touch(
    canvas: &HtmlCanvasElement,
    target: &EventTarget,
    q: &Queue,
    wake: &RafWake,
    out: &mut Vec<Closure<dyn FnMut(Event)>>,
) -> Result<(), WebHostError> {
    for kind in ["touchstart", "touchmove", "touchend", "touchcancel"] {
        let q = q.clone();
        let wake = wake.clone();
        let canvas = canvas.clone();
        let kind_owned = kind.to_string();
        let closure = Closure::wrap(Box::new(move |raw: Event| {
            raw.prevent_default();
            let Ok(ev) = raw.dyn_into::<web_sys::TouchEvent>() else {
                return;
            };
            let rect = canvas.get_bounding_client_rect();
            let touches = ev.changed_touches();
            let mut any = false;
            for i in 0..touches.length() {
                let Some(t) = touches.item(i) else { continue };
                let pos = canvas_local(
                    t.client_x() as f64,
                    t.client_y() as f64,
                    rect.left(),
                    rect.top(),
                );
                if let Some(input) =
                    touch_from_local(&kind_owned, t.identifier() as u64, pos.x, pos.y)
                {
                    q.push_raw(input);
                    any = true;
                }
            }
            if any {
                wake.request();
            }
        }) as Box<dyn FnMut(Event)>);
        listen(target, kind, &closure, false)?;
        out.push(closure);
    }
    Ok(())
}

fn install_focus(
    target: &EventTarget,
    q: &Queue,
    wake: &RafWake,
    out: &mut Vec<Closure<dyn FnMut(Event)>>,
) -> Result<(), WebHostError> {
    let last = Rc::new(Cell::new(None));
    for (kind, focused) in [("focus", true), ("blur", false)] {
        let q = q.clone();
        let wake = wake.clone();
        let last = Rc::clone(&last);
        let closure = Closure::wrap(Box::new(move |_raw: Event| {
            if last.get() == Some(focused) {
                return;
            }
            last.set(Some(focused));
            q.push_raw(InputEvent::Window(WindowInput::Focused(focused)));
            wake.request();
        }) as Box<dyn FnMut(Event)>);
        listen(target, kind, &closure, true)?;
        out.push(closure);
    }
    Ok(())
}

fn install_resize(
    canvas: &HtmlCanvasElement,
    window_target: &EventTarget,
    q: &Queue,
    wake: &RafWake,
    out: &mut Vec<Closure<dyn FnMut(Event)>>,
) -> Result<Option<web_sys::ResizeObserver>, WebHostError> {
    let last: Rc<Cell<Option<(u32, u32, u64)>>> = Rc::new(Cell::new(None));

    let q1 = q.clone();
    let wake1 = wake.clone();
    let canvas1 = canvas.clone();
    let last1 = Rc::clone(&last);
    let on_window = Closure::wrap(Box::new(move |_raw: Event| {
        maybe_resize(&canvas1, &last1, &q1, &wake1);
    }) as Box<dyn FnMut(Event)>);
    listen(window_target, "resize", &on_window, true)?;
    out.push(on_window);

    let q2 = q.clone();
    let wake2 = wake.clone();
    let canvas2 = canvas.clone();
    let last2 = Rc::clone(&last);
    let cb = Closure::wrap(Box::new(
        move |_entries: js_sys::Array, _obs: web_sys::ResizeObserver| {
            maybe_resize(&canvas2, &last2, &q2, &wake2);
        },
    )
        as Box<dyn FnMut(js_sys::Array, web_sys::ResizeObserver)>);
    let observer = web_sys::ResizeObserver::new(cb.as_ref().unchecked_ref())
        .map_err(|e| host_err("ResizeObserver", e))?;
    let el: &Element = canvas.unchecked_ref();
    observer.observe(el);
    // The observer callback is not an `FnMut(Event)`. Leak it for the page:
    // the observer holds it, and `Listeners` holds the observer.
    cb.forget();
    Ok(Some(observer))
}

fn maybe_resize(
    canvas: &HtmlCanvasElement,
    last: &Cell<Option<(u32, u32, u64)>>,
    q: &Queue,
    wake: &RafWake,
) {
    if q.window().is_none() {
        return;
    }
    let (size, dpr) = physical_size(
        canvas.client_width(),
        canvas.client_height(),
        device_pixel_ratio(),
    );
    let key = (size.width, size.height, dpr.to_bits());
    if last.get() == Some(key) {
        return;
    }
    last.set(Some(key));
    q.push_raw(InputEvent::Window(WindowInput::Resized { size, dpr }));
    wake.request();
}

pub(super) fn device_pixel_ratio() -> f64 {
    web_sys::window()
        .map(|w| w.device_pixel_ratio())
        .unwrap_or(1.0)
}

fn install_theme_and_dpr(
    window: &web_sys::Window,
    q: &Queue,
    wake: &RafWake,
    out: &mut Vec<Closure<dyn FnMut(Event)>>,
    canvas: &HtmlCanvasElement,
) -> Result<Vec<web_sys::MediaQueryList>, WebHostError> {
    let mut held = Vec::new();
    if let Some(mql) = match_media(window, "(prefers-color-scheme: dark)") {
        let q = q.clone();
        let wake = wake.clone();
        let closure = Closure::wrap(Box::new(move |_raw: Event| {
            let dark = web_sys::window()
                .and_then(|w| match_media(&w, "(prefers-color-scheme: dark)"))
                .map(|m| m.matches())
                .unwrap_or(false);
            q.push_raw(InputEvent::Window(WindowInput::ThemeChanged { dark }));
            wake.request();
        }) as Box<dyn FnMut(Event)>);
        listen(mql.unchecked_ref(), "change", &closure, true)?;
        out.push(closure);
        held.push(mql);
    }
    let dpr = device_pixel_ratio();
    let query = format!("(resolution: {dpr}dppx)");
    if let Some(mql) = match_media(window, &query) {
        let q = q.clone();
        let wake = wake.clone();
        let canvas = canvas.clone();
        let last = Rc::new(Cell::new(None));
        let closure = Closure::wrap(Box::new(move |_raw: Event| {
            maybe_resize(&canvas, &last, &q, &wake);
        }) as Box<dyn FnMut(Event)>);
        listen(mql.unchecked_ref(), "change", &closure, true)?;
        out.push(closure);
        held.push(mql);
    }
    Ok(held)
}

fn match_media(window: &web_sys::Window, query: &str) -> Option<web_sys::MediaQueryList> {
    window.match_media(query).ok().flatten()
}

fn install_visibility(
    document: &Document,
    q: &Queue,
    wake: &RafWake,
    out: &mut Vec<Closure<dyn FnMut(Event)>>,
) -> Result<(), WebHostError> {
    let q = q.clone();
    let wake = wake.clone();
    let closure = Closure::wrap(Box::new(move |_raw: Event| {
        let hidden = web_sys::window()
            .and_then(|w| w.document())
            .map(|d| d.visibility_state() == web_sys::VisibilityState::Hidden)
            .unwrap_or(false);
        q.push_raw(InputEvent::Window(WindowInput::Occluded(hidden)));
        wake.request();
    }) as Box<dyn FnMut(Event)>);
    listen(document.unchecked_ref(), "visibilitychange", &closure, true)?;
    out.push(closure);
    Ok(())
}

fn install_before_unload(
    target: &EventTarget,
    q: &Queue,
    wake: &RafWake,
    out: &mut Vec<Closure<dyn FnMut(Event)>>,
) -> Result<(), WebHostError> {
    let q = q.clone();
    let wake = wake.clone();
    let closure = Closure::wrap(Box::new(move |_raw: Event| {
        q.push_raw(InputEvent::Window(WindowInput::CloseRequested));
        wake.request();
    }) as Box<dyn FnMut(Event)>);
    listen(target, "beforeunload", &closure, true)?;
    out.push(closure);
    Ok(())
}

fn install_clipboard(
    document: &Document,
    q: &Queue,
    wake: &RafWake,
    out: &mut Vec<Closure<dyn FnMut(Event)>>,
) -> Result<(), WebHostError> {
    let target: &EventTarget = document.unchecked_ref();

    let q1 = q.clone();
    let wake1 = wake.clone();
    let paste = Closure::wrap(Box::new(move |raw: Event| {
        let Ok(ev) = raw.dyn_into::<web_sys::ClipboardEvent>() else {
            return;
        };
        let Some(text) = ev.clipboard_data().and_then(|dt| dt.get_data("text").ok()) else {
            return;
        };
        if text.is_empty() {
            return;
        }
        ev.prevent_default();
        if let Some(ticket) = q1.pending_reads.borrow_mut().pop_front() {
            q1.push_raw(InputEvent::Clipboard(ClipboardResult {
                ticket,
                text: Some(text),
            }));
        } else {
            *q1.paste_buf.borrow_mut() = Some(text);
        }
        wake1.request();
    }) as Box<dyn FnMut(Event)>);
    listen(target, "paste", &paste, false)?;
    out.push(paste);

    let q2 = q.clone();
    let copy = Closure::wrap(Box::new(move |raw: Event| {
        let Ok(ev) = raw.dyn_into::<web_sys::ClipboardEvent>() else {
            return;
        };
        let Some(text) = q2.copy_buf.borrow().clone() else {
            return;
        };
        if let Some(dt) = ev.clipboard_data() {
            if dt.set_data("text/plain", &text).is_ok() {
                ev.prevent_default();
            }
        }
    }) as Box<dyn FnMut(Event)>);
    listen(target, "copy", &copy, false)?;
    out.push(copy);

    let q3 = q.clone();
    let cut = Closure::wrap(Box::new(move |raw: Event| {
        let Ok(ev) = raw.dyn_into::<web_sys::ClipboardEvent>() else {
            return;
        };
        let Some(text) = q3.copy_buf.borrow().clone() else {
            return;
        };
        if let Some(dt) = ev.clipboard_data() {
            if dt.set_data("text/plain", &text).is_ok() {
                ev.prevent_default();
            }
        }
    }) as Box<dyn FnMut(Event)>);
    listen(target, "cut", &cut, false)?;
    out.push(cut);
    Ok(())
}

fn install_drop(
    target: &EventTarget,
    q: &Queue,
    wake: &RafWake,
    out: &mut Vec<Closure<dyn FnMut(Event)>>,
) -> Result<(), WebHostError> {
    let hovering = Rc::new(Cell::new(false));

    let q_over = q.clone();
    let wake_over = wake.clone();
    let hovering_over = Rc::clone(&hovering);
    let dragover = Closure::wrap(Box::new(move |raw: Event| {
        raw.prevent_default();
        let Ok(ev) = raw.dyn_into::<DragEvent>() else {
            return;
        };
        if hovering_over.replace(true) {
            return;
        }
        let name = drag_name(&ev).unwrap_or_default();
        q_over.push_raw(InputEvent::Drop(DropInput::Hovered(DroppedFile {
            name,
            path: None,
            bytes: None,
        })));
        wake_over.request();
    }) as Box<dyn FnMut(Event)>);
    listen(target, "dragover", &dragover, false)?;
    out.push(dragover);

    let q_leave = q.clone();
    let wake_leave = wake.clone();
    let hovering_leave = Rc::clone(&hovering);
    let dragleave = Closure::wrap(Box::new(move |_raw: Event| {
        if !hovering_leave.replace(false) {
            return;
        }
        q_leave.push_raw(InputEvent::Drop(DropInput::Cancelled));
        wake_leave.request();
    }) as Box<dyn FnMut(Event)>);
    listen(target, "dragleave", &dragleave, true)?;
    out.push(dragleave);

    let q_drop = q.clone();
    let wake_drop = wake.clone();
    let hovering_drop = Rc::clone(&hovering);
    let drop = Closure::wrap(Box::new(move |raw: Event| {
        raw.prevent_default();
        hovering_drop.set(false);
        let Ok(ev) = raw.dyn_into::<DragEvent>() else {
            return;
        };
        let Some(files) = ev.data_transfer().and_then(|dt| dt.files()) else {
            q_drop.push_raw(InputEvent::Drop(DropInput::Cancelled));
            wake_drop.request();
            return;
        };
        if files.length() == 0 {
            q_drop.push_raw(InputEvent::Drop(DropInput::Cancelled));
            wake_drop.request();
            return;
        }
        for i in 0..files.length() {
            if let Some(file) = files.item(i) {
                read_dropped_file(&q_drop, &wake_drop, file);
            }
        }
    }) as Box<dyn FnMut(Event)>);
    listen(target, "drop", &drop, false)?;
    out.push(drop);
    Ok(())
}

fn drag_name(ev: &DragEvent) -> Option<String> {
    let files = ev.data_transfer()?.files()?;
    let file = files.item(0)?;
    Some(file.name())
}

fn read_dropped_file(q: &Queue, wake: &RafWake, file: web_sys::File) {
    let name = file.name();
    let reader = match web_sys::FileReader::new() {
        Ok(reader) => reader,
        Err(_) => {
            q.push_raw(dropped(name, None));
            wake.request();
            return;
        }
    };
    let q2 = q.clone();
    let wake2 = wake.clone();
    let name2 = name.clone();
    let reader_for_cb = reader.clone();
    let closure = Closure::once(move |_ev: JsValue| {
        let bytes = reader_for_cb.result().ok().and_then(|value| {
            let buf = value.dyn_into::<js_sys::ArrayBuffer>().ok()?;
            let arr = js_sys::Uint8Array::new(&buf);
            let mut bytes = vec![0u8; arr.length() as usize];
            arr.copy_to(&mut bytes[..]);
            Some(Arc::<[u8]>::from(bytes))
        });
        q2.push_raw(dropped(name2, bytes));
        wake2.request();
    });
    let function: &js_sys::Function = closure.as_ref().unchecked_ref();
    let _ = reader.set_onload(Some(function));
    let blob: &web_sys::Blob = file.unchecked_ref();
    if reader.read_as_array_buffer(blob).is_err() {
        q.push_raw(dropped(name, None));
        wake.request();
    }
    closure.forget();
}

fn dropped(name: String, bytes: Option<Arc<[u8]>>) -> InputEvent {
    InputEvent::Drop(DropInput::Dropped(DroppedFile {
        name,
        path: None,
        bytes,
    }))
}
