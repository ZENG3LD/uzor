//! [`WebWindowProvider`] — implements [`WindowProvider`] over a DOM canvas.
//!
//! This is the canonical web window provider for uzor.  It wraps a
//! `web_sys::HtmlCanvasElement`, attaches DOM event listeners (mouse, wheel,
//! keyboard, touch, focus, resize), and translates them into uzor
//! [`PlatformEvent`]s buffered for the next frame's [`poll_events`] call.
//!
//! # Usage
//!
//! ```rust,ignore
//! // In your wasm_bindgen entry-point:
//! let provider = WebWindowProvider::from_id("canvas")?;
//! AppBuilder::new(MyApp)
//!     .backend(RenderBackend::Canvas2d)
//!     .surface_factory(Box::new(Canvas2dSurfaceFactory::new()))
//!     .window_provider(Box::new(provider))
//!     .run()
//!     .ok();
//! ```

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    Document, Event, HtmlCanvasElement, KeyboardEvent, MouseEvent, TouchEvent, WheelEvent,
};

use uzor::core::types::Rect;
use uzor::input::core::event_processor::PlatformEvent;
use uzor::input::keyboard::events::KeyCode;
use uzor::input::pointer::state::{ModifierKeys, MouseButton};

use uzor_window_hub::lifecycle::{RawHandle, WindowProvider};

// ── SendSyncCanvas ────────────────────────────────────────────────────────────

/// Thin `Send + Sync` wrapper around `HtmlCanvasElement` for use inside
/// [`RawHandle::Canvas`].
///
/// `HtmlCanvasElement` is `!Send + !Sync` because it contains raw JS object
/// references.  On `wasm32` WASM execution is always single-threaded, so
/// wrapping in this newtype and implementing `Send + Sync` is safe.
///
/// The `Canvas2dSurfaceFactory` in `uzor-render-hub` downcasts the
/// `Box<dyn Any + Send + Sync>` back to `SendSyncCanvas` to extract the canvas.
pub struct SendSyncCanvas(pub HtmlCanvasElement);

// SAFETY: wasm32 is always single-threaded — no concurrent access is possible.
unsafe impl Send for SendSyncCanvas {}
unsafe impl Sync for SendSyncCanvas {}

// ── EventListener keepalive ───────────────────────────────────────────────────

/// Keeps a boxed `Closure` alive for the duration of the event listener.
struct Listener {
    _closure: Closure<dyn FnMut(Event)>,
}

// ── WebWindowProvider ─────────────────────────────────────────────────────────

/// Web window provider that wraps an HTML canvas element.
///
/// Implements [`WindowProvider`] so it can be used directly with
/// `uzor_framework::AppBuilder`.  Event listeners are attached at construction
/// time and kept alive via the `listeners` field.
///
/// # Send + Sync
///
/// WASM is single-threaded.  `Rc<RefCell<…>>` and `HtmlCanvasElement` are not
/// `Send`, but we need the framework trait bounds to compile.  We declare the
/// impls `unsafe` with a safety note that this type is always used from the
/// single WASM thread.
pub struct WebWindowProvider {
    canvas: HtmlCanvasElement,
    pending: Rc<RefCell<Vec<PlatformEvent>>>,
    should_close: Rc<RefCell<bool>>,
    /// Kept alive so DOM listeners are not dropped.
    _listeners: Vec<Listener>,
}

// SAFETY: WASM is always single-threaded — there are no other threads that
// could race on the Rc / HtmlCanvasElement values.
unsafe impl Send for WebWindowProvider {}
unsafe impl Sync for WebWindowProvider {}

impl WebWindowProvider {
    /// Create a provider from a canvas element ID.
    ///
    /// Looks up `document.getElementById(canvas_id)`, downcasts to
    /// `HtmlCanvasElement`, attaches event listeners, and returns the provider.
    ///
    /// # Errors
    ///
    /// Returns an error string if the window / document is unavailable, the
    /// element is not found, or the element is not a canvas.
    pub fn from_id(canvas_id: &str) -> Result<Self, String> {
        let window =
            web_sys::window().ok_or_else(|| "no window object".to_string())?;
        let document: Document = window
            .document()
            .ok_or_else(|| "no document object".to_string())?;
        let canvas = document
            .get_element_by_id(canvas_id)
            .ok_or_else(|| format!("element '{}' not found", canvas_id))?
            .dyn_into::<HtmlCanvasElement>()
            .map_err(|_| format!("element '{}' is not a canvas", canvas_id))?;
        Self::from_canvas(canvas)
    }

    /// Create a provider from an existing `HtmlCanvasElement`.
    ///
    /// # Errors
    ///
    /// Returns an error string if attaching an event listener fails.
    pub fn from_canvas(canvas: HtmlCanvasElement) -> Result<Self, String> {
        let pending: Rc<RefCell<Vec<PlatformEvent>>> = Rc::new(RefCell::new(Vec::new()));
        let should_close: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
        let mut listeners = Vec::new();

        let canvas_target = canvas
            .clone()
            .dyn_into::<web_sys::EventTarget>()
            .map_err(|_| "canvas is not an EventTarget")?;

        // ── Mouse events ──────────────────────────────────────────────────────

        for event_type in &["mousedown", "mousemove", "mouseup", "mouseenter", "mouseleave"] {
            let pending_clone = pending.clone();
            let et = (*event_type).to_string();
            let closure = Closure::wrap(Box::new(move |raw: Event| {
                if let Ok(ev) = raw.dyn_into::<MouseEvent>() {
                    if let Some(p) = map_mouse_event(&et, &ev) {
                        pending_clone.borrow_mut().push(p);
                    }
                }
            }) as Box<dyn FnMut(Event)>);
            canvas_target
                .add_event_listener_with_callback(event_type, closure.as_ref().unchecked_ref())
                .map_err(|_| format!("failed to add {} listener", event_type))?;
            listeners.push(Listener { _closure: closure });
        }

        // ── Wheel event ───────────────────────────────────────────────────────

        {
            let pending_clone = pending.clone();
            let closure = Closure::wrap(Box::new(move |raw: Event| {
                raw.prevent_default();
                if let Ok(ev) = raw.dyn_into::<WheelEvent>() {
                    pending_clone.borrow_mut().push(PlatformEvent::Scroll {
                        dx: -ev.delta_x(),
                        dy: -ev.delta_y(),
                    });
                }
            }) as Box<dyn FnMut(Event)>);
            canvas_target
                .add_event_listener_with_callback("wheel", closure.as_ref().unchecked_ref())
                .map_err(|_| "failed to add wheel listener")?;
            listeners.push(Listener { _closure: closure });
        }

        // ── Keyboard events ───────────────────────────────────────────────────

        for event_type in &["keydown", "keyup"] {
            let pending_clone = pending.clone();
            let et = (*event_type).to_string();
            let closure = Closure::wrap(Box::new(move |raw: Event| {
                if let Ok(ev) = raw.dyn_into::<KeyboardEvent>() {
                    if let Some(p) = map_keyboard_event(&et, &ev) {
                        pending_clone.borrow_mut().push(p);
                    }
                }
            }) as Box<dyn FnMut(Event)>);
            canvas_target
                .add_event_listener_with_callback(event_type, closure.as_ref().unchecked_ref())
                .map_err(|_| format!("failed to add {} listener", event_type))?;
            listeners.push(Listener { _closure: closure });
        }

        // ── Touch events ──────────────────────────────────────────────────────

        for event_type in &["touchstart", "touchmove", "touchend", "touchcancel"] {
            let pending_clone = pending.clone();
            let et = (*event_type).to_string();
            let closure = Closure::wrap(Box::new(move |raw: Event| {
                raw.prevent_default();
                if let Ok(ev) = raw.dyn_into::<TouchEvent>() {
                    let evs = map_touch_event(&et, &ev);
                    pending_clone.borrow_mut().extend(evs);
                }
            }) as Box<dyn FnMut(Event)>);
            canvas_target
                .add_event_listener_with_callback(event_type, closure.as_ref().unchecked_ref())
                .map_err(|_| format!("failed to add {} listener", event_type))?;
            listeners.push(Listener { _closure: closure });
        }

        // ── Focus events ──────────────────────────────────────────────────────

        {
            let p = pending.clone();
            let closure = Closure::wrap(Box::new(move |_raw: Event| {
                p.borrow_mut().push(PlatformEvent::WindowFocused(true));
            }) as Box<dyn FnMut(Event)>);
            canvas_target
                .add_event_listener_with_callback("focus", closure.as_ref().unchecked_ref())
                .map_err(|_| "failed to add focus listener")?;
            listeners.push(Listener { _closure: closure });
        }

        {
            let p = pending.clone();
            let closure = Closure::wrap(Box::new(move |_raw: Event| {
                p.borrow_mut().push(PlatformEvent::WindowFocused(false));
            }) as Box<dyn FnMut(Event)>);
            canvas_target
                .add_event_listener_with_callback("blur", closure.as_ref().unchecked_ref())
                .map_err(|_| "failed to add blur listener")?;
            listeners.push(Listener { _closure: closure });
        }

        // ── Resize observation via window resize event ────────────────────────

        // Listen on the global window rather than the canvas — the canvas size
        // is driven by CSS, not by the resize event directly.
        if let Some(win) = web_sys::window() {
            let pending_clone = pending.clone();
            let canvas_clone = canvas.clone();
            let win_target = win
                .dyn_into::<web_sys::EventTarget>()
                .map_err(|_| "window is not EventTarget")?;
            let closure = Closure::wrap(Box::new(move |_raw: Event| {
                let w = canvas_clone.client_width() as u32;
                let h = canvas_clone.client_height() as u32;
                pending_clone
                    .borrow_mut()
                    .push(PlatformEvent::WindowResized { width: w, height: h });
            }) as Box<dyn FnMut(Event)>);
            win_target
                .add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref())
                .map_err(|_| "failed to add resize listener")?;
            listeners.push(Listener { _closure: closure });
        }

        Ok(Self {
            canvas,
            pending,
            should_close,
            _listeners: listeners,
        })
    }
}

impl WindowProvider for WebWindowProvider {
    /// Drain buffered DOM events translated to [`PlatformEvent`]s.
    fn poll_events(&mut self) -> Vec<PlatformEvent> {
        std::mem::take(&mut *self.pending.borrow_mut())
    }

    /// Logical rect of the canvas (`client_width / client_height`, origin 0,0).
    fn window_rect(&self) -> Rect {
        let w = self.canvas.client_width() as f64;
        let h = self.canvas.client_height() as f64;
        Rect::new(0.0, 0.0, w, h)
    }

    /// Device pixel ratio from `window.devicePixelRatio`.
    fn scale_factor(&self) -> f64 {
        web_sys::window()
            .map(|w| w.device_pixel_ratio())
            .unwrap_or(1.0)
    }

    /// No-op — the RAF callback in `AppBuilder::run()` drives redraws.
    fn request_redraw(&mut self) {}

    /// `true` once an explicit close signal has been issued.
    fn should_close(&self) -> bool {
        *self.should_close.borrow()
    }

    /// Returns a [`RawHandle::Canvas`] wrapping the `HtmlCanvasElement`.
    ///
    /// The `Canvas2dSurfaceFactory` downcasts the inner payload back to
    /// [`SendSyncCanvas`] via [`std::any::Any`] and clones the inner element.
    fn raw_window_handle(&self) -> Option<RawHandle> {
        let boxed: Box<dyn std::any::Any + Send + Sync> =
            Box::new(SendSyncCanvas(self.canvas.clone()));
        Some(RawHandle::Canvas(boxed))
    }
}

// ── Event mapping helpers ─────────────────────────────────────────────────────

fn map_mouse_event(event_type: &str, ev: &MouseEvent) -> Option<PlatformEvent> {
    let x = ev.offset_x() as f64;
    let y = ev.offset_y() as f64;
    let button = map_mouse_button(ev.button());
    match event_type {
        "mousedown"  => Some(PlatformEvent::PointerDown { x, y, button }),
        "mouseup"    => Some(PlatformEvent::PointerUp   { x, y, button }),
        "mousemove"  => Some(PlatformEvent::PointerMoved { x, y }),
        "mouseenter" => Some(PlatformEvent::PointerEntered),
        "mouseleave" => Some(PlatformEvent::PointerLeft),
        _            => None,
    }
}

fn map_mouse_button(b: i16) -> MouseButton {
    match b {
        0 => MouseButton::Left,
        1 => MouseButton::Middle,
        2 => MouseButton::Right,
        _ => MouseButton::Left,
    }
}

fn map_keyboard_event(event_type: &str, ev: &KeyboardEvent) -> Option<PlatformEvent> {
    let key = map_keycode(&ev.code());
    let modifiers = ModifierKeys {
        shift: ev.shift_key(),
        ctrl:  ev.ctrl_key(),
        alt:   ev.alt_key(),
        meta:  ev.meta_key(),
    };
    match event_type {
        "keydown" => Some(PlatformEvent::KeyDown { key, modifiers }),
        "keyup"   => Some(PlatformEvent::KeyUp   { key, modifiers }),
        _         => None,
    }
}

fn map_touch_event(event_type: &str, ev: &TouchEvent) -> Vec<PlatformEvent> {
    let mut out = Vec::new();
    let touches = ev.changed_touches();
    match event_type {
        "touchstart" => {
            for i in 0..touches.length() {
                if let Some(t) = touches.item(i) {
                    out.push(PlatformEvent::TouchStart {
                        id: t.identifier() as u64,
                        x: t.client_x() as f64,
                        y: t.client_y() as f64,
                    });
                }
            }
        }
        "touchmove" => {
            for i in 0..touches.length() {
                if let Some(t) = touches.item(i) {
                    out.push(PlatformEvent::TouchMove {
                        id: t.identifier() as u64,
                        x: t.client_x() as f64,
                        y: t.client_y() as f64,
                    });
                }
            }
        }
        "touchend" => {
            for i in 0..touches.length() {
                if let Some(t) = touches.item(i) {
                    out.push(PlatformEvent::TouchEnd {
                        id: t.identifier() as u64,
                        x: t.client_x() as f64,
                        y: t.client_y() as f64,
                    });
                }
            }
        }
        "touchcancel" => {
            for i in 0..touches.length() {
                if let Some(t) = touches.item(i) {
                    out.push(PlatformEvent::TouchCancel {
                        id: t.identifier() as u64,
                    });
                }
            }
        }
        _ => {}
    }
    out
}

fn map_keycode(code: &str) -> KeyCode {
    match code {
        "KeyA" => KeyCode::A, "KeyB" => KeyCode::B, "KeyC" => KeyCode::C,
        "KeyD" => KeyCode::D, "KeyE" => KeyCode::E, "KeyF" => KeyCode::F,
        "KeyG" => KeyCode::G, "KeyH" => KeyCode::H, "KeyI" => KeyCode::I,
        "KeyJ" => KeyCode::J, "KeyK" => KeyCode::K, "KeyL" => KeyCode::L,
        "KeyM" => KeyCode::M, "KeyN" => KeyCode::N, "KeyO" => KeyCode::O,
        "KeyP" => KeyCode::P, "KeyQ" => KeyCode::Q, "KeyR" => KeyCode::R,
        "KeyS" => KeyCode::S, "KeyT" => KeyCode::T, "KeyU" => KeyCode::U,
        "KeyV" => KeyCode::V, "KeyW" => KeyCode::W, "KeyX" => KeyCode::X,
        "KeyY" => KeyCode::Y, "KeyZ" => KeyCode::Z,
        "Digit0" => KeyCode::Num0, "Digit1" => KeyCode::Num1,
        "Digit2" => KeyCode::Num2, "Digit3" => KeyCode::Num3,
        "Digit4" => KeyCode::Num4, "Digit5" => KeyCode::Num5,
        "Digit6" => KeyCode::Num6, "Digit7" => KeyCode::Num7,
        "Digit8" => KeyCode::Num8, "Digit9" => KeyCode::Num9,
        "Enter"      => KeyCode::Enter,
        "Escape"     => KeyCode::Escape,
        "Backspace"  => KeyCode::Backspace,
        "Tab"        => KeyCode::Tab,
        "Space"      => KeyCode::Space,
        "ArrowLeft"  => KeyCode::ArrowLeft,
        "ArrowRight" => KeyCode::ArrowRight,
        "ArrowUp"    => KeyCode::ArrowUp,
        "ArrowDown"  => KeyCode::ArrowDown,
        "F1"  => KeyCode::F1,  "F2"  => KeyCode::F2,
        "F3"  => KeyCode::F3,  "F4"  => KeyCode::F4,
        "F5"  => KeyCode::F5,  "F6"  => KeyCode::F6,
        "F7"  => KeyCode::F7,  "F8"  => KeyCode::F8,
        "F9"  => KeyCode::F9,  "F10" => KeyCode::F10,
        "F11" => KeyCode::F11, "F12" => KeyCode::F12,
        "Delete"   => KeyCode::Delete,
        "Home"     => KeyCode::Home,
        "End"      => KeyCode::End,
        "PageUp"   => KeyCode::PageUp,
        "PageDown" => KeyCode::PageDown,
        _ => KeyCode::Unknown,
    }
}

