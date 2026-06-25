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
    ClipboardEvent, Document, Event, HtmlCanvasElement, KeyboardEvent, MouseEvent, TouchEvent,
    WheelEvent,
};

use uzor::core::types::Rect;
use uzor::input::core::event_processor::PlatformEvent;
use uzor::input::keyboard::events::KeyCode;
use uzor::input::pointer::state::{ModifierKeys, MouseButton};

use uzor::layout::window::{RawHandle, WindowProvider};

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
    /// Producer called from inside the DOM `copy` / `cut` listeners to
    /// synchronously read the text the embedder wants to put on the
    /// system clipboard.  Returning `None` lets the browser fall through
    /// to its default copy/cut behaviour (no-op on a canvas).
    ///
    /// Set via [`WebWindowProvider::set_copy_provider`].  Defaults to
    /// `None`, in which case Ctrl+C / Ctrl+X are silent on this canvas.
    copy_provider: Rc<RefCell<Option<Box<dyn FnMut() -> Option<String>>>>>,
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
        let copy_provider: Rc<RefCell<Option<Box<dyn FnMut() -> Option<String>>>>>
            = Rc::new(RefCell::new(None));
        // Tracks the last keyboard-modifier bits we observed.  Browser DOM does
        // not surface a dedicated `modifierschange` event the way winit does on
        // desktop, so we synthesize one from `keydown`/`keyup` whenever the
        // modifier mask changes — otherwise downstream consumers (e.g.
        // mlc-input-shell's Ctrl+A/S/F/Z/Y branch which gates on
        // `state.modifiers.ctrl`) never see the modifier flip and the
        // shortcut silently dies.
        let last_modifiers: Rc<RefCell<ModifierKeys>> =
            Rc::new(RefCell::new(ModifierKeys::default()));
        let mut listeners = Vec::new();

        let canvas_target = canvas
            .clone()
            .dyn_into::<web_sys::EventTarget>()
            .map_err(|_| "canvas is not an EventTarget")?;

        // ── Pointer events ────────────────────────────────────────────────────
        //
        // Use pointer-events (PointerEvent extends MouseEvent) instead of
        // mouse-events so we can call `setPointerCapture` on pointerdown.
        // Without capture, mousemove/mouseup are only dispatched while the
        // cursor is over the canvas — once a drag carries the cursor off the
        // canvas (e.g. dragging a separator past the chart edge, or panning
        // out of the window), the drag freezes and `mouseup` never arrives,
        // leaving `mouse_pressed` stuck `true`. With capture, every
        // pointermove/pointerup is delivered to the canvas for the lifetime
        // of the gesture even if the cursor leaves the element.
        //
        // PointerEvent inherits `button()`, `offset_x()`, `offset_y()` from
        // MouseEvent, so the existing `map_mouse_event` helper still applies.

        for event_type in &[
            "pointerdown",
            "pointermove",
            "pointerup",
            "pointercancel",
            "pointerenter",
            "pointerleave",
        ] {
            let pending_clone = pending.clone();
            let canvas_for_capture = canvas.clone();
            let et = (*event_type).to_string();
            let closure = Closure::wrap(Box::new(move |raw: Event| {
                // Capture the pointer on pointerdown so that subsequent
                // pointermove / pointerup (and pointercancel) fire on the
                // canvas even if the cursor leaves it.
                if et == "pointerdown" {
                    // PointerEvent.pointerId via Reflect (web-sys' PointerEvent
                    // type isn't enabled in the default feature set used here).
                    if let Ok(pid) = js_sys::Reflect::get(raw.as_ref(), &"pointerId".into()) {
                        if let Some(id) = pid.as_f64() {
                            let _ = canvas_for_capture.set_pointer_capture(id as i32);
                        }
                    }
                }
                if let Ok(ev) = raw.dyn_into::<MouseEvent>() {
                    if let Some(p) = map_pointer_event(&et, &ev) {
                        pending_clone.borrow_mut().push(p);
                    }
                }
            }) as Box<dyn FnMut(Event)>);
            canvas_target
                .add_event_listener_with_callback(event_type, closure.as_ref().unchecked_ref())
                .map_err(|_| format!("failed to add {} listener", event_type))?;
            listeners.push(Listener { _closure: closure });
        }

        // ── contextmenu — prevent the browser's right-click menu ──────────────
        //
        // Without preventDefault the native context menu appears on right-click,
        // hijacking the gesture before our PointerDown(Right) reaches
        // `chart.on_right_click()`. Suppress it on the canvas.

        {
            let closure = Closure::wrap(Box::new(move |raw: Event| {
                raw.prevent_default();
            }) as Box<dyn FnMut(Event)>);
            canvas_target
                .add_event_listener_with_callback("contextmenu", closure.as_ref().unchecked_ref())
                .map_err(|_| "failed to add contextmenu listener")?;
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
            let last_mods_clone = last_modifiers.clone();
            let et = (*event_type).to_string();
            let closure = Closure::wrap(Box::new(move |raw: Event| {
                if let Ok(ev) = raw.dyn_into::<KeyboardEvent>() {
                    // Block browser-default behaviour for the editing shortcuts
                    // we route into TextFieldStore ourselves.  Without this:
                    //   Ctrl+A  → browser "select all on page" steals selection
                    //   Ctrl+S  → browser "save page" dialog
                    //   Ctrl+F  → browser find bar
                    //   Ctrl+Z  → no-op (canvas isn't editable) but may bubble
                    //   Ctrl+Y  → reopen-closed-tab in Firefox
                    // Ctrl+C / Ctrl+X / Ctrl+V are handled by the dedicated
                    // copy/cut/paste listeners which already preventDefault.
                    if et == "keydown" && (ev.ctrl_key() || ev.meta_key()) {
                        match ev.code().as_str() {
                            "KeyA" | "KeyS" | "KeyF" | "KeyZ" | "KeyY" => {
                                ev.prevent_default();
                            }
                            _ => {}
                        }
                    }
                    let mut pending = pending_clone.borrow_mut();
                    // Synthesize a `ModifiersChanged` event whenever the
                    // modifier mask differs from the last keyboard event we
                    // observed.  Browsers never fire a dedicated
                    // modifier-change event, but downstream consumers (e.g.
                    // shells that gate Ctrl-shortcuts on a tracked
                    // `state.modifiers.ctrl` flag) expect winit-style
                    // `ModifiersChanged` deltas.  Emitting before the
                    // KeyDown/KeyUp guarantees the consumer's modifier state
                    // is up-to-date by the time the key arm runs.
                    let current_mods = ModifierKeys {
                        shift: ev.shift_key(),
                        ctrl:  ev.ctrl_key(),
                        alt:   ev.alt_key(),
                        meta:  ev.meta_key(),
                    };
                    {
                        let mut last = last_mods_clone.borrow_mut();
                        if *last != current_mods {
                            *last = current_mods;
                            pending.push(PlatformEvent::ModifiersChanged {
                                modifiers: current_mods,
                            });
                        }
                    }
                    if let Some(p) = map_keyboard_event(&et, &ev) {
                        pending.push(p);
                    }
                    // On keydown also emit `TextInput { text: ev.key() }` for
                    // printable single-character keys.  Mirrors the
                    // winit-Ime::Commit pathway on native: KeyboardInput +
                    // Ime::Commit are both delivered, the input dispatcher
                    // routes shortcuts (Ctrl+A/V/Z…) off the KeyDown and text
                    // off the IME commit.  Without this `keydown` only carries
                    // the positional KeyCode (KeyA, KeyB, …) which lacks
                    // layout info, so non-Latin keyboards and Shift'd symbols
                    // can never reach text fields.
                    if et == "keydown" {
                        let key = ev.key();
                        // Filter out modifier-only chords and named-key strings
                        // (Escape, Enter, ArrowLeft, F1, …) — those are >1 char.
                        // Only emit when `ev.key()` is exactly one Unicode
                        // scalar AND no Ctrl/Meta (those are shortcuts).
                        if !ev.ctrl_key() && !ev.meta_key() {
                            let mut chars = key.chars();
                            if let (Some(ch), None) = (chars.next(), chars.next()) {
                                if !ch.is_control() {
                                    pending.push(PlatformEvent::TextInput {
                                        text: ch.to_string(),
                                    });
                                }
                            }
                        }
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

        // ── Clipboard events ──────────────────────────────────────────────────
        //
        // Browser security only delivers synchronous clipboard text inside
        // user-gesture-triggered `paste` / `copy` / `cut` events.  Listen on
        // `document` so the gesture is captured anywhere on the page (the
        // canvas may not have focus at the moment Ctrl+V is hit).
        if let Some(doc_for_clip) = web_sys::window().and_then(|w| w.document()) {
            let doc_target = doc_for_clip.dyn_into::<web_sys::EventTarget>()
                .map_err(|_| "document is not EventTarget")?;

            // paste → translate clipboardData.text into PlatformEvent::ClipboardPaste.
            {
                let pending_clone = pending.clone();
                let closure = Closure::wrap(Box::new(move |raw: Event| {
                    if let Ok(ev) = raw.dyn_into::<ClipboardEvent>() {
                        if let Some(dt) = ev.clipboard_data() {
                            if let Ok(text) = dt.get_data("text") {
                                if !text.is_empty() {
                                    pending_clone.borrow_mut()
                                        .push(PlatformEvent::ClipboardPaste { text });
                                    ev.prevent_default();
                                }
                            }
                        }
                    }
                }) as Box<dyn FnMut(Event)>);
                doc_target
                    .add_event_listener_with_callback("paste", closure.as_ref().unchecked_ref())
                    .map_err(|_| "failed to add paste listener")?;
                listeners.push(Listener { _closure: closure });
            }

            // copy → ask copy_provider for the text to put on the clipboard,
            // write it via clipboardData.setData, suppress the default behaviour.
            {
                let provider = copy_provider.clone();
                let closure = Closure::wrap(Box::new(move |raw: Event| {
                    if let Ok(ev) = raw.clone().dyn_into::<ClipboardEvent>() {
                        let text_opt = provider.borrow_mut().as_mut().and_then(|f| f());
                        if let Some(text) = text_opt {
                            if let Some(dt) = ev.clipboard_data() {
                                let _ = dt.set_data("text/plain", &text);
                                ev.prevent_default();
                            }
                        }
                    }
                }) as Box<dyn FnMut(Event)>);
                doc_target
                    .add_event_listener_with_callback("copy", closure.as_ref().unchecked_ref())
                    .map_err(|_| "failed to add copy listener")?;
                listeners.push(Listener { _closure: closure });
            }

            // cut → copy the selection AND fire ClipboardCut so the embedder
            // deletes the selection from the focused text field.
            {
                let provider = copy_provider.clone();
                let pending_clone = pending.clone();
                let closure = Closure::wrap(Box::new(move |raw: Event| {
                    if let Ok(ev) = raw.clone().dyn_into::<ClipboardEvent>() {
                        let text_opt = provider.borrow_mut().as_mut().and_then(|f| f());
                        if let Some(text) = text_opt {
                            if let Some(dt) = ev.clipboard_data() {
                                let _ = dt.set_data("text/plain", &text);
                                ev.prevent_default();
                                pending_clone.borrow_mut().push(PlatformEvent::ClipboardCut);
                            }
                        }
                    }
                }) as Box<dyn FnMut(Event)>);
                doc_target
                    .add_event_listener_with_callback("cut", closure.as_ref().unchecked_ref())
                    .map_err(|_| "failed to add cut listener")?;
                listeners.push(Listener { _closure: closure });
            }
        }

        Ok(Self {
            canvas,
            pending,
            should_close,
            copy_provider,
            _listeners: listeners,
        })
    }

    /// Register a closure that returns the text the embedder wants placed on
    /// the system clipboard when the user triggers a `copy` or `cut` gesture
    /// (Ctrl+C / Ctrl+X, the Edit menu, or right-click Copy).
    ///
    /// The closure is called synchronously inside the DOM `copy` / `cut`
    /// event listener — that is the only context in which `clipboardData`
    /// is writable from JavaScript.  Returning `None` lets the browser's
    /// default action run (which is typically a no-op on a canvas).
    ///
    /// Replaces any previously-registered provider.
    pub fn set_copy_provider<F>(&self, provider: F)
    where
        F: FnMut() -> Option<String> + 'static,
    {
        *self.copy_provider.borrow_mut() = Some(Box::new(provider));
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

/// Map a DOM pointer event (PointerEvent extends MouseEvent) to a
/// platform-neutral [`PlatformEvent`]. We listen on `pointer*` rather than
/// `mouse*` so the canvas can call `setPointerCapture` on pointerdown and
/// keep receiving move/up events even when the cursor leaves the canvas
/// during a drag.
fn map_pointer_event(event_type: &str, ev: &MouseEvent) -> Option<PlatformEvent> {
    let x = ev.offset_x() as f64;
    let y = ev.offset_y() as f64;
    let button = map_mouse_button(ev.button());
    match event_type {
        "pointerdown"   => Some(PlatformEvent::PointerDown { x, y, button }),
        "pointerup"     => Some(PlatformEvent::PointerUp   { x, y, button }),
        // pointercancel: browser aborted the gesture (e.g. system gesture,
        // OS captured the input). Treat as release so we don't leave
        // `mouse_pressed` stuck `true`.
        "pointercancel" => Some(PlatformEvent::PointerUp   { x, y, button }),
        "pointermove"   => Some(PlatformEvent::PointerMoved { x, y }),
        "pointerenter"  => Some(PlatformEvent::PointerEntered),
        "pointerleave"  => Some(PlatformEvent::PointerLeft),
        _               => None,
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

