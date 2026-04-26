//! Web backend for uzor using WebAssembly
//!
//! This crate provides the web platform implementation for uzor,
//! supporting browsers via WebAssembly (WASM).

#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    HtmlCanvasElement, Window, Document, Event, MouseEvent, KeyboardEvent,
    WheelEvent, TouchEvent, CompositionEvent,
};

use uzor::input::events::KeyCode;
use uzor::input::state::{ModifierKeys, MouseButton};
use uzor::platform::{
    backends::PlatformBackend,
    types::{PlatformError, WindowId, SystemIntegration},
    ImeEvent, PlatformEvent, SystemTheme, WindowConfig,
};
use uzor::input::cursor::CursorIcon;

pub use uzor;

// =============================================================================
// WebPlatform - Main Platform Backend
// =============================================================================

/// Web platform backend for uzor
///
/// This struct handles all browser integration including canvas management,
/// event handling, clipboard operations, and system integration.
#[derive(Clone)]
pub struct WebPlatform {
    state: Rc<RefCell<WebPlatformState>>,
}

struct WebPlatformState {
    window: Window,
    document: Document,
    canvas: HtmlCanvasElement,
    window_id: WindowId,
    config: WindowConfig,
    event_queue: VecDeque<PlatformEvent>,
    scale_factor: f64,
    cursor_icon: CursorIcon,
    cursor_visible: bool,
    ime_position: (f64, f64),
    ime_allowed: bool,
    // Event listener closures (kept alive)
    _listeners: Vec<EventListener>,
}

struct EventListener {
    _closure: Closure<dyn FnMut(Event)>,
}

impl WebPlatform {
    /// Create a new WebPlatform from a canvas element ID
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No window object is available
    /// - No document object is available
    /// - Canvas element with the given ID is not found
    /// - Canvas element is not an HTMLCanvasElement
    pub fn new(canvas_id: &str) -> Result<Self, String> {
        // Get window and document
        let window = web_sys::window()
            .ok_or_else(|| "No window object available".to_string())?;

        let document = window
            .document()
            .ok_or_else(|| "No document object available".to_string())?;

        // Get canvas element
        let canvas = document
            .get_element_by_id(canvas_id)
            .ok_or_else(|| format!("Canvas element '{}' not found", canvas_id))?
            .dyn_into::<HtmlCanvasElement>()
            .map_err(|_| format!("Element '{}' is not a canvas", canvas_id))?;

        // Get device pixel ratio
        let scale_factor = window.device_pixel_ratio();

        // Create initial config from canvas size
        let width = canvas.client_width() as u32;
        let height = canvas.client_height() as u32;
        let config = WindowConfig {
            title: "Web Canvas".to_string(),
            width,
            height,
            ..WindowConfig::default()
        };

        let state = Rc::new(RefCell::new(WebPlatformState {
            window,
            document,
            canvas,
            window_id: WindowId::new(),
            config,
            event_queue: VecDeque::new(),
            scale_factor,
            cursor_icon: CursorIcon::Default,
            cursor_visible: true,
            ime_position: (0.0, 0.0),
            ime_allowed: false,
            _listeners: Vec::new(),
        }));

        // Setup event listeners
        Self::setup_event_listeners(&state)?;

        Ok(Self { state })
    }

    /// Get the underlying canvas element
    pub fn canvas(&self) -> HtmlCanvasElement {
        self.state.borrow().canvas.clone()
    }

    fn setup_event_listeners(state: &Rc<RefCell<WebPlatformState>>) -> Result<(), String> {
        let mut state_mut = state.borrow_mut();
        let canvas = state_mut.canvas.clone();
        let canvas_target = canvas.clone().dyn_into::<web_sys::EventTarget>()
            .map_err(|_| "Canvas is not an EventTarget")?;

        // Mouse events
        Self::add_mouse_listener(&mut state_mut, &canvas_target, "mousedown", state)?;
        Self::add_mouse_listener(&mut state_mut, &canvas_target, "mousemove", state)?;
        Self::add_mouse_listener(&mut state_mut, &canvas_target, "mouseup", state)?;
        Self::add_mouse_listener(&mut state_mut, &canvas_target, "mouseenter", state)?;
        Self::add_mouse_listener(&mut state_mut, &canvas_target, "mouseleave", state)?;

        // Wheel events
        Self::add_wheel_listener(&mut state_mut, &canvas_target, state)?;

        // Touch events
        Self::add_touch_listener(&mut state_mut, &canvas_target, "touchstart", state)?;
        Self::add_touch_listener(&mut state_mut, &canvas_target, "touchmove", state)?;
        Self::add_touch_listener(&mut state_mut, &canvas_target, "touchend", state)?;
        Self::add_touch_listener(&mut state_mut, &canvas_target, "touchcancel", state)?;

        // Keyboard events
        Self::add_keyboard_listener(&mut state_mut, &canvas_target, "keydown", state)?;
        Self::add_keyboard_listener(&mut state_mut, &canvas_target, "keyup", state)?;

        // Focus events
        Self::add_focus_listener(&mut state_mut, &canvas_target, state)?;

        // IME events
        Self::add_ime_listener(&mut state_mut, &canvas_target, "compositionstart", state)?;
        Self::add_ime_listener(&mut state_mut, &canvas_target, "compositionupdate", state)?;
        Self::add_ime_listener(&mut state_mut, &canvas_target, "compositionend", state)?;

        Ok(())
    }

    fn add_mouse_listener(
        state_mut: &mut WebPlatformState,
        target: &web_sys::EventTarget,
        event_type: &str,
        state_ref: &Rc<RefCell<WebPlatformState>>,
    ) -> Result<(), String> {
        let state_clone = state_ref.clone();
        let event_type_str = event_type.to_string();

        let closure = Closure::wrap(Box::new(move |event: Event| {
            if let Ok(mouse_event) = event.dyn_into::<MouseEvent>() {
                let mut state = state_clone.borrow_mut();
                let platform_event = Self::map_mouse_event(&event_type_str, &mouse_event);
                if let Some(evt) = platform_event {
                    state.event_queue.push_back(evt);
                }
            }
        }) as Box<dyn FnMut(Event)>);

        target.add_event_listener_with_callback(event_type, closure.as_ref().unchecked_ref())
            .map_err(|_| format!("Failed to add {} listener", event_type))?;

        state_mut._listeners.push(EventListener { _closure: closure });
        Ok(())
    }

    fn add_wheel_listener(
        state_mut: &mut WebPlatformState,
        target: &web_sys::EventTarget,
        state_ref: &Rc<RefCell<WebPlatformState>>,
    ) -> Result<(), String> {
        let state_clone = state_ref.clone();

        let closure = Closure::wrap(Box::new(move |event: Event| {
            event.prevent_default();

            if let Ok(wheel_event) = event.dyn_into::<WheelEvent>() {
                let mut state = state_clone.borrow_mut();
                let dx = wheel_event.delta_x();
                let dy = wheel_event.delta_y();

                state.event_queue.push_back(PlatformEvent::Scroll {
                    dx: -dx,
                    dy: -dy
                });
            }
        }) as Box<dyn FnMut(Event)>);

        target.add_event_listener_with_callback("wheel", closure.as_ref().unchecked_ref())
            .map_err(|_| "Failed to add wheel listener")?;

        state_mut._listeners.push(EventListener { _closure: closure });
        Ok(())
    }

    fn add_touch_listener(
        state_mut: &mut WebPlatformState,
        target: &web_sys::EventTarget,
        event_type: &str,
        state_ref: &Rc<RefCell<WebPlatformState>>,
    ) -> Result<(), String> {
        let state_clone = state_ref.clone();
        let event_type_str = event_type.to_string();

        let closure = Closure::wrap(Box::new(move |event: Event| {
            event.prevent_default();

            if let Ok(touch_event) = event.dyn_into::<TouchEvent>() {
                let mut state = state_clone.borrow_mut();
                let events = Self::map_touch_event(&event_type_str, &touch_event);
                for evt in events {
                    state.event_queue.push_back(evt);
                }
            }
        }) as Box<dyn FnMut(Event)>);

        target.add_event_listener_with_callback(event_type, closure.as_ref().unchecked_ref())
            .map_err(|_| format!("Failed to add {} listener", event_type))?;

        state_mut._listeners.push(EventListener { _closure: closure });
        Ok(())
    }

    fn add_keyboard_listener(
        state_mut: &mut WebPlatformState,
        target: &web_sys::EventTarget,
        event_type: &str,
        state_ref: &Rc<RefCell<WebPlatformState>>,
    ) -> Result<(), String> {
        let state_clone = state_ref.clone();
        let event_type_str = event_type.to_string();

        let closure = Closure::wrap(Box::new(move |event: Event| {
            if let Ok(keyboard_event) = event.dyn_into::<KeyboardEvent>() {
                let mut state = state_clone.borrow_mut();
                let platform_event = Self::map_keyboard_event(&event_type_str, &keyboard_event);
                if let Some(evt) = platform_event {
                    state.event_queue.push_back(evt);
                }
            }
        }) as Box<dyn FnMut(Event)>);

        target.add_event_listener_with_callback(event_type, closure.as_ref().unchecked_ref())
            .map_err(|_| format!("Failed to add {} listener", event_type))?;

        state_mut._listeners.push(EventListener { _closure: closure });
        Ok(())
    }

    fn add_focus_listener(
        state_mut: &mut WebPlatformState,
        target: &web_sys::EventTarget,
        state_ref: &Rc<RefCell<WebPlatformState>>,
    ) -> Result<(), String> {
        let state_clone = state_ref.clone();

        let focus_closure = Closure::wrap(Box::new(move |_event: Event| {
            let mut state = state_clone.borrow_mut();
            state.event_queue.push_back(PlatformEvent::WindowFocused(true));
        }) as Box<dyn FnMut(Event)>);

        target.add_event_listener_with_callback("focus", focus_closure.as_ref().unchecked_ref())
            .map_err(|_| "Failed to add focus listener")?;

        state_mut._listeners.push(EventListener { _closure: focus_closure });

        let state_clone2 = state_ref.clone();
        let blur_closure = Closure::wrap(Box::new(move |_event: Event| {
            let mut state = state_clone2.borrow_mut();
            state.event_queue.push_back(PlatformEvent::WindowFocused(false));
        }) as Box<dyn FnMut(Event)>);

        target.add_event_listener_with_callback("blur", blur_closure.as_ref().unchecked_ref())
            .map_err(|_| "Failed to add blur listener")?;

        state_mut._listeners.push(EventListener { _closure: blur_closure });

        Ok(())
    }

    fn add_ime_listener(
        state_mut: &mut WebPlatformState,
        target: &web_sys::EventTarget,
        event_type: &str,
        state_ref: &Rc<RefCell<WebPlatformState>>,
    ) -> Result<(), String> {
        let state_clone = state_ref.clone();
        let event_type_str = event_type.to_string();

        let closure = Closure::wrap(Box::new(move |event: Event| {
            if let Ok(composition_event) = event.dyn_into::<CompositionEvent>() {
                let mut state = state_clone.borrow_mut();
                let ime_event = Self::map_ime_event(&event_type_str, &composition_event);
                if let Some(evt) = ime_event {
                    state.event_queue.push_back(PlatformEvent::Ime(evt));
                }
            }
        }) as Box<dyn FnMut(Event)>);

        target.add_event_listener_with_callback(event_type, closure.as_ref().unchecked_ref())
            .map_err(|_| format!("Failed to add {} listener", event_type))?;

        state_mut._listeners.push(EventListener { _closure: closure });
        Ok(())
    }

    fn map_mouse_event(event_type: &str, event: &MouseEvent) -> Option<PlatformEvent> {
        let x = event.offset_x() as f64;
        let y = event.offset_y() as f64;
        let button = Self::map_mouse_button(event.button());

        match event_type {
            "mousedown" => Some(PlatformEvent::PointerDown { x, y, button }),
            "mouseup" => Some(PlatformEvent::PointerUp { x, y, button }),
            "mousemove" => Some(PlatformEvent::PointerMoved { x, y }),
            "mouseenter" => Some(PlatformEvent::PointerEntered),
            "mouseleave" => Some(PlatformEvent::PointerLeft),
            _ => None,
        }
    }

    fn map_touch_event(event_type: &str, event: &TouchEvent) -> Vec<PlatformEvent> {
        let mut events = Vec::new();

        match event_type {
            "touchstart" => {
                let touches = event.changed_touches();
                for i in 0..touches.length() {
                    if let Some(touch) = touches.item(i) {
                        events.push(PlatformEvent::TouchStart {
                            id: touch.identifier() as u64,
                            x: touch.client_x() as f64,
                            y: touch.client_y() as f64,
                        });
                    }
                }
            }
            "touchmove" => {
                let touches = event.changed_touches();
                for i in 0..touches.length() {
                    if let Some(touch) = touches.item(i) {
                        events.push(PlatformEvent::TouchMove {
                            id: touch.identifier() as u64,
                            x: touch.client_x() as f64,
                            y: touch.client_y() as f64,
                        });
                    }
                }
            }
            "touchend" => {
                let touches = event.changed_touches();
                for i in 0..touches.length() {
                    if let Some(touch) = touches.item(i) {
                        events.push(PlatformEvent::TouchEnd {
                            id: touch.identifier() as u64,
                            x: touch.client_x() as f64,
                            y: touch.client_y() as f64,
                        });
                    }
                }
            }
            "touchcancel" => {
                let touches = event.changed_touches();
                for i in 0..touches.length() {
                    if let Some(touch) = touches.item(i) {
                        events.push(PlatformEvent::TouchCancel {
                            id: touch.identifier() as u64,
                        });
                    }
                }
            }
            _ => {}
        }

        events
    }

    fn map_keyboard_event(event_type: &str, event: &KeyboardEvent) -> Option<PlatformEvent> {
        let key = Self::map_keycode(&event.code());
        let modifiers = Self::get_modifiers(event);

        match event_type {
            "keydown" => Some(PlatformEvent::KeyDown { key, modifiers }),
            "keyup" => Some(PlatformEvent::KeyUp { key, modifiers }),
            _ => None,
        }
    }

    fn map_ime_event(event_type: &str, event: &CompositionEvent) -> Option<ImeEvent> {
        match event_type {
            "compositionstart" => Some(ImeEvent::Enabled),
            "compositionupdate" => {
                let data = event.data().unwrap_or_default();
                Some(ImeEvent::Preedit(data, None))
            }
            "compositionend" => {
                let data = event.data().unwrap_or_default();
                Some(ImeEvent::Commit(data))
            }
            _ => None,
        }
    }

    fn map_mouse_button(button: i16) -> MouseButton {
        match button {
            0 => MouseButton::Left,
            1 => MouseButton::Middle,
            2 => MouseButton::Right,
            _ => MouseButton::Left,
        }
    }

    fn get_modifiers(event: &KeyboardEvent) -> ModifierKeys {
        ModifierKeys {
            shift: event.shift_key(),
            ctrl: event.ctrl_key(),
            alt: event.alt_key(),
            meta: event.meta_key(),
        }
    }

    fn map_keycode(code: &str) -> KeyCode {
        match code {
            // Letters
            "KeyA" => KeyCode::A,
            "KeyB" => KeyCode::B,
            "KeyC" => KeyCode::C,
            "KeyD" => KeyCode::D,
            "KeyE" => KeyCode::E,
            "KeyF" => KeyCode::F,
            "KeyG" => KeyCode::G,
            "KeyH" => KeyCode::H,
            "KeyI" => KeyCode::I,
            "KeyJ" => KeyCode::J,
            "KeyK" => KeyCode::K,
            "KeyL" => KeyCode::L,
            "KeyM" => KeyCode::M,
            "KeyN" => KeyCode::N,
            "KeyO" => KeyCode::O,
            "KeyP" => KeyCode::P,
            "KeyQ" => KeyCode::Q,
            "KeyR" => KeyCode::R,
            "KeyS" => KeyCode::S,
            "KeyT" => KeyCode::T,
            "KeyU" => KeyCode::U,
            "KeyV" => KeyCode::V,
            "KeyW" => KeyCode::W,
            "KeyX" => KeyCode::X,
            "KeyY" => KeyCode::Y,
            "KeyZ" => KeyCode::Z,

            // Numbers
            "Digit0" => KeyCode::Num0,
            "Digit1" => KeyCode::Num1,
            "Digit2" => KeyCode::Num2,
            "Digit3" => KeyCode::Num3,
            "Digit4" => KeyCode::Num4,
            "Digit5" => KeyCode::Num5,
            "Digit6" => KeyCode::Num6,
            "Digit7" => KeyCode::Num7,
            "Digit8" => KeyCode::Num8,
            "Digit9" => KeyCode::Num9,

            // Special keys
            "Enter" => KeyCode::Enter,
            "Escape" => KeyCode::Escape,
            "Backspace" => KeyCode::Backspace,
            "Tab" => KeyCode::Tab,
            "Space" => KeyCode::Space,

            // Arrow keys
            "ArrowLeft" => KeyCode::ArrowLeft,
            "ArrowRight" => KeyCode::ArrowRight,
            "ArrowUp" => KeyCode::ArrowUp,
            "ArrowDown" => KeyCode::ArrowDown,

            // Function keys
            "F1" => KeyCode::F1,
            "F2" => KeyCode::F2,
            "F3" => KeyCode::F3,
            "F4" => KeyCode::F4,
            "F5" => KeyCode::F5,
            "F6" => KeyCode::F6,
            "F7" => KeyCode::F7,
            "F8" => KeyCode::F8,
            "F9" => KeyCode::F9,
            "F10" => KeyCode::F10,
            "F11" => KeyCode::F11,
            "F12" => KeyCode::F12,

            // Other
            "Delete" => KeyCode::Delete,
            "Home" => KeyCode::Home,
            "End" => KeyCode::End,
            "PageUp" => KeyCode::PageUp,
            "PageDown" => KeyCode::PageDown,

            _ => KeyCode::Unknown,
        }
    }

    fn cursor_icon_to_css(icon: CursorIcon) -> &'static str {
        icon.css_name()
    }
}

// =============================================================================
// Send + Sync Implementation (WASM is single-threaded)
// =============================================================================

// SAFETY: WebPlatform is only used in single-threaded WASM contexts.
// JavaScript/WASM doesn't have threads that would make Rc unsafe.
unsafe impl Send for WebPlatform {}
unsafe impl Sync for WebPlatform {}

// =============================================================================
// Trait Implementations
// =============================================================================

impl PlatformBackend for WebPlatform {
    fn name(&self) -> &'static str {
        todo!("not yet implemented for this platform")
    }

    fn create_window(&mut self, config: WindowConfig) -> Result<WindowId, PlatformError> {
        let mut state = self.state.borrow_mut();

        // Update canvas size
        let canvas = &state.canvas;
        canvas.set_width((config.width as f64 * state.scale_factor) as u32);
        canvas.set_height((config.height as f64 * state.scale_factor) as u32);

        // Update title (if document title)
        state.document.set_title(&config.title);

        state.config = config;
        state.event_queue.push_back(PlatformEvent::WindowCreated);

        Ok(state.window_id)
    }

    fn close_window(&mut self, _window_id: WindowId) -> Result<(), PlatformError> {
        let mut state = self.state.borrow_mut();
        state.event_queue.push_back(PlatformEvent::WindowDestroyed);
        Ok(())
    }

    fn primary_window(&self) -> Option<WindowId> {
        todo!("not yet implemented for this platform")
    }

    fn poll_events(&mut self) -> Vec<PlatformEvent> {
        todo!("not yet implemented for this platform")
    }

    fn request_redraw(&self, _id: WindowId) {
        // No-op for now: web redraws are driven by requestAnimationFrame
    }
}

impl SystemIntegration for WebPlatform {
    fn get_clipboard(&self) -> Option<String> {
        todo!("not yet implemented for this platform")
    }

    fn set_clipboard(&self, _text: &str) {
        todo!("not yet implemented for this platform")
    }

    fn get_system_theme(&self) -> Option<SystemTheme> {
        let state = self.state.borrow();

        // Use matchMedia to detect dark mode
        if let Ok(Some(media_query)) = state.window.match_media("(prefers-color-scheme: dark)") {
            if media_query.matches() {
                return Some(SystemTheme::Dark);
            }
        }

        Some(SystemTheme::Light)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keycode_mapping() {
        assert_eq!(WebPlatform::map_keycode("KeyA"), KeyCode::A);
        assert_eq!(WebPlatform::map_keycode("Digit5"), KeyCode::Num5);
        assert_eq!(WebPlatform::map_keycode("Enter"), KeyCode::Enter);
        assert_eq!(WebPlatform::map_keycode("ArrowLeft"), KeyCode::ArrowLeft);
        assert_eq!(WebPlatform::map_keycode("Unknown"), KeyCode::Unknown);
    }

    #[test]
    fn test_mouse_button_mapping() {
        assert_eq!(WebPlatform::map_mouse_button(0), MouseButton::Left);
        assert_eq!(WebPlatform::map_mouse_button(1), MouseButton::Middle);
        assert_eq!(WebPlatform::map_mouse_button(2), MouseButton::Right);
    }

    #[test]
    fn test_cursor_icon_css() {
        assert_eq!(WebPlatform::cursor_icon_to_css(CursorIcon::Default), "default");
        assert_eq!(WebPlatform::cursor_icon_to_css(CursorIcon::PointingHand), "pointer");
        assert_eq!(WebPlatform::cursor_icon_to_css(CursorIcon::Text), "text");
        assert_eq!(WebPlatform::cursor_icon_to_css(CursorIcon::Grab), "grab");
        assert_eq!(WebPlatform::cursor_icon_to_css(CursorIcon::ResizeVertical), "ns-resize");
    }
}
