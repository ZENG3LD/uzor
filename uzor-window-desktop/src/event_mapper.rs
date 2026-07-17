//! Event mapping from winit to uzor platform events

use winit::event::{
    ElementState, Ime, KeyEvent, Modifiers, MouseButton as WinitMouseButton, MouseScrollDelta, TouchPhase,
    WindowEvent,
};
use winit::keyboard::{KeyCode as WinitKeyCode, PhysicalKey};

use uzor::input::events::KeyCode;
use uzor::input::state::{ModifierKeys, MouseButton};
use uzor::platform::{ImeEvent, PlatformEvent};

/// Maps winit events to platform events.
///
/// Stateful: tracks the window's current scale factor and the last known
/// cursor position (in LOGICAL pixels) so that:
///
/// - `MouseInput` (winit reports no coordinates of its own for button
///   press/release) is stamped with the real last-known cursor position
///   instead of `(0.0, 0.0)`.
/// - Every pointer/touch/scroll coordinate is normalized from winit's
///   PHYSICAL pixels into the LOGICAL pixel space the rest of uzor (layout,
///   widgets, `window_rect()`) operates in.
///
/// One instance per window — cursor position and scale factor are
/// per-window state.
pub struct EventMapper {
    /// Device pixel ratio (HiDPI scale factor) of the window this mapper
    /// tracks. Kept in sync via `WindowEvent::ScaleFactorChanged`.
    scale_factor: f64,
    /// Last known cursor position, in LOGICAL pixels. Stays `(0.0, 0.0)`
    /// until the first `CursorMoved` arrives — a `MouseInput` with no prior
    /// move stamps that tracked default, which is the correct behavior for
    /// the never-moved edge case.
    last_cursor: (f64, f64),
}

impl EventMapper {
    /// Construct a mapper for a window with the given initial scale factor.
    pub fn new(scale_factor: f64) -> Self {
        Self {
            scale_factor: sanitize_scale(scale_factor),
            last_cursor: (0.0, 0.0),
        }
    }

    /// Map a winit `WindowEvent` to a `PlatformEvent`, updating the
    /// mapper's internal cursor/scale tracking as a side effect.
    pub fn map_window_event(&mut self, event: &WindowEvent) -> Option<PlatformEvent> {
        match event {
            WindowEvent::Resized(size) => Some(PlatformEvent::WindowResized {
                width: size.width,
                height: size.height,
            }),

            WindowEvent::Moved(position) => Some(PlatformEvent::WindowMoved {
                x: position.x,
                y: position.y,
            }),

            WindowEvent::Focused(focused) => Some(PlatformEvent::WindowFocused(*focused)),

            WindowEvent::CursorEntered { .. } => Some(PlatformEvent::PointerEntered),

            WindowEvent::CursorLeft { .. } => Some(PlatformEvent::PointerLeft),

            WindowEvent::CursorMoved { position, .. } => {
                let x = position.x / self.scale_factor;
                let y = position.y / self.scale_factor;
                self.last_cursor = (x, y);
                Some(PlatformEvent::PointerMoved { x, y })
            }

            WindowEvent::MouseInput { state, button, .. } => {
                let mapped_button = map_mouse_button(*button);
                let (x, y) = self.last_cursor;
                match state {
                    ElementState::Pressed => Some(PlatformEvent::PointerDown {
                        x,
                        y,
                        button: mapped_button,
                    }),
                    ElementState::Released => Some(PlatformEvent::PointerUp {
                        x,
                        y,
                        button: mapped_button,
                    }),
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (*x as f64 * 20.0, *y as f64 * 20.0),
                    MouseScrollDelta::PixelDelta(pos) => {
                        (pos.x / self.scale_factor, pos.y / self.scale_factor)
                    }
                };
                Some(PlatformEvent::Scroll { dx, dy })
            }

            WindowEvent::Touch(touch) => {
                let x = touch.location.x / self.scale_factor;
                let y = touch.location.y / self.scale_factor;
                match touch.phase {
                    TouchPhase::Started => Some(PlatformEvent::TouchStart { id: touch.id, x, y }),
                    TouchPhase::Moved => Some(PlatformEvent::TouchMove { id: touch.id, x, y }),
                    TouchPhase::Ended => Some(PlatformEvent::TouchEnd { id: touch.id, x, y }),
                    TouchPhase::Cancelled => Some(PlatformEvent::TouchCancel { id: touch.id }),
                }
            }

            WindowEvent::KeyboardInput { event, .. } => {
                map_keyboard_event(event)
            }

            WindowEvent::ModifiersChanged(modifiers) => {
                Some(PlatformEvent::ModifiersChanged {
                    modifiers: map_modifiers(modifiers),
                })
            }

            WindowEvent::Ime(ime) => Some(PlatformEvent::Ime(map_ime_event(ime))),

            WindowEvent::DroppedFile(path) => Some(PlatformEvent::FileDropped {
                path: path.clone(),
            }),

            WindowEvent::HoveredFile(path) => Some(PlatformEvent::FileHovered {
                path: path.clone(),
            }),

            WindowEvent::HoveredFileCancelled => Some(PlatformEvent::FileCancelled),

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.scale_factor = sanitize_scale(*scale_factor);
                Some(PlatformEvent::ScaleFactorChanged {
                    scale: *scale_factor,
                })
            }

            WindowEvent::ThemeChanged(theme) => {
                let dark_mode = matches!(theme, winit::window::Theme::Dark);
                Some(PlatformEvent::ThemeChanged { dark_mode })
            }

            WindowEvent::RedrawRequested => Some(PlatformEvent::RedrawRequested),

            // Events we don't map
            WindowEvent::CloseRequested
            | WindowEvent::Destroyed
            | WindowEvent::ActivationTokenDone { .. }
            | WindowEvent::AxisMotion { .. }
            | WindowEvent::Occluded(_)
            | WindowEvent::TouchpadPressure { .. }
            | WindowEvent::PinchGesture { .. }
            | WindowEvent::PanGesture { .. }
            | WindowEvent::DoubleTapGesture { .. }
            | WindowEvent::RotationGesture { .. } => None,
        }
    }
}

/// Clamp a winit-reported scale factor to a sane positive value.
///
/// Winit always returns a positive finite scale factor in practice, but a
/// mapper constructed from a stale/placeholder value (or a future platform
/// quirk) must not divide pointer coordinates by zero or `NaN`.
fn sanitize_scale(scale: f64) -> f64 {
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

/// Map winit mouse button to uzor mouse button
fn map_mouse_button(button: WinitMouseButton) -> MouseButton {
    match button {
        WinitMouseButton::Left => MouseButton::Left,
        WinitMouseButton::Right => MouseButton::Right,
        WinitMouseButton::Middle => MouseButton::Middle,
        _ => MouseButton::Left, // Fallback for extra buttons
    }
}

/// Map winit keyboard event to platform event
fn map_keyboard_event(event: &KeyEvent) -> Option<PlatformEvent> {
    let key_code = match &event.physical_key {
        PhysicalKey::Code(code) => map_key_code(*code),
        PhysicalKey::Unidentified(_) => KeyCode::Unknown,
    };

    // Note: Modifiers need to be tracked separately via ModifiersChanged event
    // winit 0.30 doesn't include modifiers in KeyEvent
    let modifiers = ModifierKeys::default();

    match event.state {
        ElementState::Pressed => Some(PlatformEvent::KeyDown {
            key: key_code,
            modifiers,
        }),
        ElementState::Released => Some(PlatformEvent::KeyUp {
            key: key_code,
            modifiers,
        }),
    }
}

/// Map winit key code to uzor key code
fn map_key_code(key: WinitKeyCode) -> KeyCode {
    match key {
        // Letters
        WinitKeyCode::KeyA => KeyCode::A,
        WinitKeyCode::KeyB => KeyCode::B,
        WinitKeyCode::KeyC => KeyCode::C,
        WinitKeyCode::KeyD => KeyCode::D,
        WinitKeyCode::KeyE => KeyCode::E,
        WinitKeyCode::KeyF => KeyCode::F,
        WinitKeyCode::KeyG => KeyCode::G,
        WinitKeyCode::KeyH => KeyCode::H,
        WinitKeyCode::KeyI => KeyCode::I,
        WinitKeyCode::KeyJ => KeyCode::J,
        WinitKeyCode::KeyK => KeyCode::K,
        WinitKeyCode::KeyL => KeyCode::L,
        WinitKeyCode::KeyM => KeyCode::M,
        WinitKeyCode::KeyN => KeyCode::N,
        WinitKeyCode::KeyO => KeyCode::O,
        WinitKeyCode::KeyP => KeyCode::P,
        WinitKeyCode::KeyQ => KeyCode::Q,
        WinitKeyCode::KeyR => KeyCode::R,
        WinitKeyCode::KeyS => KeyCode::S,
        WinitKeyCode::KeyT => KeyCode::T,
        WinitKeyCode::KeyU => KeyCode::U,
        WinitKeyCode::KeyV => KeyCode::V,
        WinitKeyCode::KeyW => KeyCode::W,
        WinitKeyCode::KeyX => KeyCode::X,
        WinitKeyCode::KeyY => KeyCode::Y,
        WinitKeyCode::KeyZ => KeyCode::Z,

        // Numbers
        WinitKeyCode::Digit0 => KeyCode::Num0,
        WinitKeyCode::Digit1 => KeyCode::Num1,
        WinitKeyCode::Digit2 => KeyCode::Num2,
        WinitKeyCode::Digit3 => KeyCode::Num3,
        WinitKeyCode::Digit4 => KeyCode::Num4,
        WinitKeyCode::Digit5 => KeyCode::Num5,
        WinitKeyCode::Digit6 => KeyCode::Num6,
        WinitKeyCode::Digit7 => KeyCode::Num7,
        WinitKeyCode::Digit8 => KeyCode::Num8,
        WinitKeyCode::Digit9 => KeyCode::Num9,

        // Function keys
        WinitKeyCode::F1 => KeyCode::F1,
        WinitKeyCode::F2 => KeyCode::F2,
        WinitKeyCode::F3 => KeyCode::F3,
        WinitKeyCode::F4 => KeyCode::F4,
        WinitKeyCode::F5 => KeyCode::F5,
        WinitKeyCode::F6 => KeyCode::F6,
        WinitKeyCode::F7 => KeyCode::F7,
        WinitKeyCode::F8 => KeyCode::F8,
        WinitKeyCode::F9 => KeyCode::F9,
        WinitKeyCode::F10 => KeyCode::F10,
        WinitKeyCode::F11 => KeyCode::F11,
        WinitKeyCode::F12 => KeyCode::F12,

        // Navigation
        WinitKeyCode::ArrowUp => KeyCode::ArrowUp,
        WinitKeyCode::ArrowDown => KeyCode::ArrowDown,
        WinitKeyCode::ArrowLeft => KeyCode::ArrowLeft,
        WinitKeyCode::ArrowRight => KeyCode::ArrowRight,
        WinitKeyCode::Home => KeyCode::Home,
        WinitKeyCode::End => KeyCode::End,
        WinitKeyCode::PageUp => KeyCode::PageUp,
        WinitKeyCode::PageDown => KeyCode::PageDown,

        // Editing
        WinitKeyCode::Backspace => KeyCode::Backspace,
        WinitKeyCode::Delete => KeyCode::Delete,
        WinitKeyCode::Insert => KeyCode::Insert,
        WinitKeyCode::Enter => KeyCode::Enter,
        WinitKeyCode::Tab => KeyCode::Tab,
        WinitKeyCode::Space => KeyCode::Space,
        WinitKeyCode::Escape => KeyCode::Escape,

        // Symbols
        WinitKeyCode::Equal => KeyCode::Plus,
        WinitKeyCode::Minus => KeyCode::Minus,
        WinitKeyCode::BracketLeft => KeyCode::BracketLeft,
        WinitKeyCode::BracketRight => KeyCode::BracketRight,

        // All other keys map to Unknown
        _ => KeyCode::Unknown,
    }
}

/// Map winit modifiers to uzor modifiers
fn map_modifiers(modifiers: &Modifiers) -> ModifierKeys {
    ModifierKeys {
        shift: modifiers.state().shift_key(),
        ctrl: modifiers.state().control_key(),
        alt: modifiers.state().alt_key(),
        meta: modifiers.state().super_key(),
    }
}

/// Map winit IME event to uzor IME event
fn map_ime_event(ime: &Ime) -> ImeEvent {
    match ime {
        Ime::Enabled => ImeEvent::Enabled,
        Ime::Preedit(text, cursor) => {
            ImeEvent::Preedit(text.clone(), *cursor)
        }
        Ime::Commit(text) => ImeEvent::Commit(text.clone()),
        Ime::Disabled => ImeEvent::Disabled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mouse_button_mapping() {
        assert_eq!(
            map_mouse_button(WinitMouseButton::Left),
            MouseButton::Left
        );
        assert_eq!(
            map_mouse_button(WinitMouseButton::Right),
            MouseButton::Right
        );
        assert_eq!(
            map_mouse_button(WinitMouseButton::Middle),
            MouseButton::Middle
        );
    }

    #[test]
    fn test_key_code_mapping() {
        assert_eq!(map_key_code(WinitKeyCode::KeyA), KeyCode::A);
        assert_eq!(map_key_code(WinitKeyCode::Digit0), KeyCode::Num0);
        assert_eq!(map_key_code(WinitKeyCode::F1), KeyCode::F1);
        assert_eq!(map_key_code(WinitKeyCode::ArrowUp), KeyCode::ArrowUp);
        assert_eq!(map_key_code(WinitKeyCode::Enter), KeyCode::Enter);
        assert_eq!(map_key_code(WinitKeyCode::Space), KeyCode::Space);
    }

    #[test]
    fn test_modifiers_mapping() {
        let modifiers = Modifiers::default();
        let mapped = map_modifiers(&modifiers);
        assert!(!mapped.shift);
        assert!(!mapped.ctrl);
        assert!(!mapped.alt);
        assert!(!mapped.meta);
    }

    fn cursor_moved(x: f64, y: f64) -> WindowEvent {
        WindowEvent::CursorMoved {
            device_id: winit::event::DeviceId::dummy(),
            position: winit::dpi::PhysicalPosition::new(x, y),
        }
    }

    fn mouse_input(state: ElementState) -> WindowEvent {
        WindowEvent::MouseInput {
            device_id: winit::event::DeviceId::dummy(),
            state,
            button: WinitMouseButton::Left,
        }
    }

    /// Gate (a): Down/Up carry the last Moved position, not (0, 0).
    #[test]
    fn test_pointer_down_up_carry_last_moved_position() {
        let mut mapper = EventMapper::new(1.0);

        match mapper.map_window_event(&cursor_moved(123.0, 456.0)) {
            Some(PlatformEvent::PointerMoved { x, y }) => {
                assert_eq!((x, y), (123.0, 456.0));
            }
            other => panic!("expected PointerMoved, got {other:?}"),
        }

        match mapper.map_window_event(&mouse_input(ElementState::Pressed)) {
            Some(PlatformEvent::PointerDown { x, y, button }) => {
                assert_eq!((x, y), (123.0, 456.0));
                assert_eq!(button, MouseButton::Left);
            }
            other => panic!("expected PointerDown, got {other:?}"),
        }

        match mapper.map_window_event(&mouse_input(ElementState::Released)) {
            Some(PlatformEvent::PointerUp { x, y, .. }) => {
                assert_eq!((x, y), (123.0, 456.0));
            }
            other => panic!("expected PointerUp, got {other:?}"),
        }
    }

    /// Gate (b): coordinates are logical (physical / scale_factor) at
    /// dpr 1.0 / 1.25 / 2.0 — covers Moved, the Down/Up stamp it feeds, and
    /// Touch.
    #[test]
    fn test_pointer_and_touch_coords_are_logical_at_various_scale_factors() {
        for scale in [1.0, 1.25, 2.0] {
            let mut mapper = EventMapper::new(scale);
            let (px, py) = (250.0, 500.0);
            let expected = (px / scale, py / scale);

            match mapper.map_window_event(&cursor_moved(px, py)) {
                Some(PlatformEvent::PointerMoved { x, y }) => {
                    assert_eq!((x, y), expected, "PointerMoved at scale {scale}");
                }
                other => panic!("expected PointerMoved, got {other:?}"),
            }

            match mapper.map_window_event(&mouse_input(ElementState::Pressed)) {
                Some(PlatformEvent::PointerDown { x, y, .. }) => {
                    assert_eq!((x, y), expected, "PointerDown at scale {scale}");
                }
                other => panic!("expected PointerDown, got {other:?}"),
            }

            let touch = WindowEvent::Touch(winit::event::Touch {
                device_id: winit::event::DeviceId::dummy(),
                phase: TouchPhase::Started,
                location: winit::dpi::PhysicalPosition::new(px, py),
                force: None,
                id: 7,
            });
            match mapper.map_window_event(&touch) {
                Some(PlatformEvent::TouchStart { x, y, id }) => {
                    assert_eq!((x, y), expected, "TouchStart at scale {scale}");
                    assert_eq!(id, 7);
                }
                other => panic!("expected TouchStart, got {other:?}"),
            }
        }
    }

    /// Gate (c): a Down with no prior Move stamps the tracked default
    /// (0.0, 0.0) — acceptable ONLY in this never-moved case.
    #[test]
    fn test_pointer_down_without_prior_move_stamps_default() {
        let mut mapper = EventMapper::new(1.0);

        match mapper.map_window_event(&mouse_input(ElementState::Pressed)) {
            Some(PlatformEvent::PointerDown { x, y, .. }) => {
                assert_eq!((x, y), (0.0, 0.0));
            }
            other => panic!("expected PointerDown, got {other:?}"),
        }
    }
}
