//! Winit → framework input mapper (design §2.3).
//!
//! One [`WinitMapper`] per window. Reuses the leaf tables in
//! `uzor_window_desktop::event_mapper` (`map_key_code`, `map_modifiers`,
//! `map_mouse_button`, `map_ime_event`) and stamps modifiers on pointer
//! events itself. Coordinates leave this mapper in window-local logical
//! pixels.

use smallvec::SmallVec;
use winit::event::{
    ElementState, Ime, KeyEvent, MouseScrollDelta, TouchPhase,
    WindowEvent,
};
use winit::keyboard::PhysicalKey;
use uzor::input::ModifierKeys;
use uzor::platform::ImeEvent;
use uzor_window_desktop::event_mapper::{
    map_ime_event, map_key_code, map_modifiers, map_mouse_button, sanitize_scale,
};

use crate::types::bus::{
    DropInput, DroppedFile, ImeInput, InputEvent, KeyInput, KeyState, PointerInput, TouchInput,
    WheelDelta, WheelInput, WindowInput,
};
use crate::types::window::{Point, SizePx};

/// Per-window winit → [`InputEvent`] translator.
#[derive(Debug)]
pub struct WinitMapper {
    scale: f64,
    last_cursor: Point,
    mods: ModifierKeys,
}

impl WinitMapper {
    /// A mapper for a window whose current scale factor is `scale`.
    pub fn new(scale: f64) -> Self {
        Self {
            scale: sanitize_scale(scale),
            last_cursor: Point::new(0.0, 0.0),
            mods: ModifierKeys::default(),
        }
    }

    /// Current device-pixel ratio used for logical conversion.
    pub fn scale(&self) -> f64 {
        self.scale
    }

    /// Last known cursor position in logical pixels.
    pub fn last_cursor(&self) -> Point {
        self.last_cursor
    }

    /// Current modifier keys (from the last `ModifiersChanged`).
    pub fn mods(&self) -> ModifierKeys {
        self.mods
    }

    /// Translate one winit window event. Empty when the event is host-only
    /// (redraw request) or not yet mapped.
    pub fn map(&mut self, ev: &WindowEvent) -> SmallVec<[InputEvent; 2]> {
        let mut out = SmallVec::new();
        match ev {
            WindowEvent::Resized(size) => {
                out.push(InputEvent::Window(WindowInput::Resized {
                    size: SizePx::new(size.width, size.height),
                    dpr: self.scale,
                }));
            }
            WindowEvent::Moved(pos) => {
                out.push(InputEvent::Window(WindowInput::Moved {
                    position: (pos.x, pos.y),
                }));
            }
            WindowEvent::Focused(focused) => {
                out.push(InputEvent::Window(WindowInput::Focused(*focused)));
            }
            WindowEvent::Occluded(occ) => {
                out.push(InputEvent::Window(WindowInput::Occluded(*occ)));
            }
            WindowEvent::CloseRequested => {
                out.push(InputEvent::Window(WindowInput::CloseRequested));
            }
            WindowEvent::Destroyed => {
                out.push(InputEvent::Window(WindowInput::Destroyed));
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                // Host emits a real Resized with the window's current size.
                self.scale = sanitize_scale(*scale_factor);
            }
            WindowEvent::ThemeChanged(theme) => {
                let dark = matches!(theme, winit::window::Theme::Dark);
                out.push(InputEvent::Window(WindowInput::ThemeChanged { dark }));
            }
            WindowEvent::CursorEntered { .. } => {
                out.push(InputEvent::Pointer(PointerInput::Entered));
            }
            WindowEvent::CursorLeft { .. } => {
                out.push(InputEvent::Pointer(PointerInput::Left));
            }
            WindowEvent::CursorMoved { position, .. } => {
                let pos = Point::new(position.x / self.scale, position.y / self.scale);
                self.last_cursor = pos;
                out.push(InputEvent::Pointer(PointerInput::Moved {
                    pos,
                    mods: self.mods,
                }));
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let button = map_mouse_button(*button);
                let pos = self.last_cursor;
                let mods = self.mods;
                match state {
                    ElementState::Pressed => {
                        out.push(InputEvent::Pointer(PointerInput::Down { pos, button, mods }));
                    }
                    ElementState::Released => {
                        out.push(InputEvent::Pointer(PointerInput::Up { pos, button, mods }));
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (delta, pos) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (
                        WheelDelta::Lines {
                            x: f64::from(*x),
                            y: f64::from(*y),
                        },
                        self.last_cursor,
                    ),
                    MouseScrollDelta::PixelDelta(p) => (
                        WheelDelta::Pixels {
                            x: p.x / self.scale,
                            y: p.y / self.scale,
                        },
                        self.last_cursor,
                    ),
                };
                out.push(InputEvent::Wheel(WheelInput {
                    pos,
                    delta,
                    mods: self.mods,
                }));
            }
            WindowEvent::Touch(touch) => {
                let pos = Point::new(touch.location.x / self.scale, touch.location.y / self.scale);
                let id = touch.id;
                let ev = match touch.phase {
                    TouchPhase::Started => TouchInput::Start { id, pos },
                    TouchPhase::Moved => TouchInput::Move { id, pos },
                    TouchPhase::Ended => TouchInput::End { id, pos },
                    TouchPhase::Cancelled => TouchInput::Cancel { id },
                };
                out.push(InputEvent::Touch(ev));
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let Some(k) = map_key(event, self.mods) {
                    out.push(InputEvent::Key(k));
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.mods = map_modifiers(modifiers);
            }
            WindowEvent::Ime(ime) => {
                out.push(InputEvent::Ime(map_ime(ime)));
            }
            WindowEvent::DroppedFile(path) => {
                let name = path
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                out.push(InputEvent::Drop(DropInput::Dropped(DroppedFile {
                    name,
                    path: Some(path.clone()),
                    bytes: None,
                })));
            }
            WindowEvent::HoveredFile(path) => {
                let name = path
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                out.push(InputEvent::Drop(DropInput::Hovered(DroppedFile {
                    name,
                    path: Some(path.clone()),
                    bytes: None,
                })));
            }
            WindowEvent::HoveredFileCancelled => {
                out.push(InputEvent::Drop(DropInput::Cancelled));
            }
            // RedrawRequested and gesture events are host-only.
            _ => {}
        }
        out
    }
}

fn map_key(event: &KeyEvent, mods: ModifierKeys) -> Option<KeyInput> {
    let code = match &event.physical_key {
        PhysicalKey::Code(c) => map_key_code(*c),
        PhysicalKey::Unidentified(_) => uzor::input::KeyCode::Unknown,
    };
    let state = match event.state {
        ElementState::Pressed if event.repeat => KeyState::Repeat,
        ElementState::Pressed => KeyState::Down,
        ElementState::Released => KeyState::Up,
    };
    Some(KeyInput {
        code,
        text: event.text.as_ref().map(|s| s.to_string()),
        state,
        mods,
    })
}

fn map_ime(ime: &Ime) -> ImeInput {
    match map_ime_event(ime) {
        ImeEvent::Enabled => ImeInput::Enabled,
        ImeEvent::Preedit(text, cursor) => ImeInput::Preedit { text, cursor },
        ImeEvent::Commit(text) => ImeInput::Commit(text),
        ImeEvent::Disabled => ImeInput::Disabled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::dpi::{PhysicalPosition, PhysicalSize};
    use winit::event::MouseButton as WB;
    use winit::event::MouseButton as WinitMouseButton;

    #[test]
    fn pointer_move_is_logical_and_stamps_mods() {
        let mut m = WinitMapper::new(2.0);
        // Shift via ModifiersChanged is exercised through map_modifiers tables;
        // here we set mods by calling map on a ModifiersChanged-equivalent path.
        m.mods.shift = true;
        let ev = WindowEvent::CursorMoved {
            device_id: winit::event::DeviceId::dummy(),
            position: PhysicalPosition::new(100.0, 40.0),
        };
        let out = m.map(&ev);
        assert_eq!(out.len(), 1);
        match &out[0] {
            InputEvent::Pointer(PointerInput::Moved { pos, mods }) => {
                assert!((pos.x - 50.0).abs() < 1e-9);
                assert!((pos.y - 20.0).abs() < 1e-9);
                assert!(mods.shift);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn mouse_down_uses_last_cursor() {
        let mut m = WinitMapper::new(1.0);
        let _ = m.map(&WindowEvent::CursorMoved {
            device_id: winit::event::DeviceId::dummy(),
            position: PhysicalPosition::new(10.0, 20.0),
        });
        let out = m.map(&WindowEvent::MouseInput {
            device_id: winit::event::DeviceId::dummy(),
            state: ElementState::Pressed,
            button: WB::Left,
        });
        match &out[0] {
            InputEvent::Pointer(PointerInput::Down { pos, button, .. }) => {
                assert_eq!(*pos, Point::new(10.0, 20.0));
                assert_eq!(*button, uzor::input::MouseButton::Left);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn resize_and_close_are_window_inputs() {
        let mut m = WinitMapper::new(1.5);
        let out = m.map(&WindowEvent::Resized(PhysicalSize::new(800, 600)));
        match &out[0] {
            InputEvent::Window(WindowInput::Resized { size, dpr }) => {
                assert_eq!(*size, SizePx::new(800, 600));
                assert!((*dpr - 1.5).abs() < 1e-9);
            }
            other => panic!("unexpected {other:?}"),
        }
        let out = m.map(&WindowEvent::CloseRequested);
        assert!(matches!(out[0], InputEvent::Window(WindowInput::CloseRequested)));
    }

    #[test]
    fn wheel_pixel_delta_is_logical() {
        let mut m = WinitMapper::new(2.0);
        let out = m.map(&WindowEvent::MouseWheel {
            device_id: winit::event::DeviceId::dummy(),
            delta: MouseScrollDelta::PixelDelta(PhysicalPosition::new(4.0, -8.0)),
            phase: TouchPhase::Moved,
        });
        match &out[0] {
            InputEvent::Wheel(WheelInput {
                delta: WheelDelta::Pixels { x, y },
                ..
            }) => {
                assert!((*x - 2.0).abs() < 1e-9);
                assert!((*y - -4.0).abs() < 1e-9);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn key_tables_are_reused() {
        assert_eq!(
            map_key_code(winit::keyboard::KeyCode::KeyA),
            uzor::input::KeyCode::A
        );
        assert_eq!(
            map_mouse_button(WinitMouseButton::Right),
            uzor::input::MouseButton::Right
        );
    }
}
