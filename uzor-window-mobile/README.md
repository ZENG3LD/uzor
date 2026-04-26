# uzor-mobile

Mobile platform backend for uzor supporting iOS and Android.

## Overview

This crate provides the mobile platform implementation for uzor, enabling touch-first applications on iOS and Android devices. It implements all platform traits with mobile-specific features like:

- **Touch Input**: Multi-touch support with gesture recognition
- **Virtual Keyboard**: IME integration for text input
- **Haptic Feedback**: Tactile feedback for UI interactions
- **Safe Areas**: Proper handling of notches, home indicators, and system bars
- **Screen Orientation**: Detect and respond to orientation changes
- **Native Integration**: Clipboard, URL opening, theme detection

## Architecture

```
uzor-mobile/
├── lib.rs          # Main MobilePlatform struct + trait implementations
├── android.rs      # Android-specific backend (NDK + JNI)
├── ios.rs          # iOS-specific backend (UIKit + Objective-C)
└── common.rs       # Shared gesture recognition utilities
```

### Platform Abstraction

The crate uses conditional compilation to provide a unified API across platforms:

```rust
#[cfg(target_os = "android")]
use android::AndroidBackend;

#[cfg(target_os = "ios")]
use ios::IosBackend;
```

When compiled for non-mobile platforms (e.g., during desktop development), a stub backend is used that compiles but returns placeholder values.

## Usage

### Basic Setup

```rust
use uzor_mobile::MobilePlatform;
use uzor_core::platform::{PlatformBackend, WindowConfig};

fn main() {
    let mut platform = MobilePlatform::new().unwrap();

    let window = platform.create_window(
        WindowConfig::new("My Mobile App")
    ).unwrap();

    // Event loop
    loop {
        while let Some(event) = platform.poll_event() {
            handle_event(event);
        }
    }
}
```

### Touch Event Handling

```rust
use uzor_core::platform::PlatformEvent;

fn handle_event(event: PlatformEvent) {
    match event {
        PlatformEvent::TouchStart { id, x, y } => {
            println!("Touch {} started at ({}, {})", id, x, y);
        }
        PlatformEvent::TouchMove { id, x, y } => {
            println!("Touch {} moved to ({}, {})", id, x, y);
        }
        PlatformEvent::TouchEnd { id, x, y } => {
            println!("Touch {} ended at ({}, {})", id, x, y);
        }
        _ => {}
    }
}
```

### Mobile-Specific Features

#### Safe Area Insets

```rust
let (top, right, bottom, left) = platform.safe_area_insets();
println!("Top inset (notch/status bar): {}", top);
println!("Bottom inset (home indicator): {}", bottom);
```

#### Haptic Feedback

```rust
use uzor_mobile::HapticStyle;

// Light tap feedback
platform.haptic_feedback(HapticStyle::Light);

// Success feedback
platform.haptic_feedback(HapticStyle::Success);

// Error feedback
platform.haptic_feedback(HapticStyle::Error);
```

#### Screen Orientation

```rust
use uzor_mobile::ScreenOrientation;

match platform.orientation() {
    ScreenOrientation::Portrait => {
        // Adjust UI for portrait
    }
    ScreenOrientation::Landscape => {
        // Adjust UI for landscape
    }
    _ => {}
}
```

#### Virtual Keyboard (IME)

```rust
use uzor_core::platform::ImeSupport;

// Show keyboard when text field is focused
platform.set_ime_allowed(true);
platform.set_ime_position(cursor_x, cursor_y);

// Hide keyboard when done
platform.set_ime_allowed(false);
```

## Implemented Traits

### PlatformBackend

Core window and event management:

- `capabilities()` - Returns mobile capabilities
- `create_window()` - Creates the app window (single window on mobile)
- `poll_event()` - Gets platform events
- `request_redraw()` - Requests screen redraw
- `window_size()` - Gets screen dimensions
- `scale_factor()` - Gets device pixel ratio

### Clipboard

Native clipboard integration:

- `get_text()` - Read clipboard text
- `set_text()` - Write clipboard text

### SystemIntegration

System-level operations:

- `open_url()` - Opens URL in browser
- `get_system_theme()` - Gets light/dark mode
- `get_scale_factor()` - Gets display scaling

### CursorManagement

Stub implementation (mobile devices don't have cursors):

- `set_cursor()` - No-op (tracks state for compatibility)
- `set_cursor_visible()` - No-op
- `set_cursor_locked()` - Returns NotSupported error

### ImeSupport

Virtual keyboard integration:

- `set_ime_position()` - Sets keyboard position hint
- `set_ime_allowed()` - Shows/hides virtual keyboard
- `set_ime_cursor_area()` - Sets text field cursor area

### RenderSurface

Surface for rendering:

- `raw_window_handle()` - Returns None (platform-specific access needed)
- `surface_size()` - Gets render surface dimensions
- `surface_scale_factor()` - Gets surface scaling

## Platform Capabilities

Mobile platforms have these characteristics:

```rust
PlatformCapabilities {
    has_clipboard: true,
    has_file_dialogs: true,
    has_system_theme: true,
    has_touch: true,
    has_mouse: false,          // No cursor
    has_keyboard: true,         // Virtual keyboard
    has_file_drop: false,       // Not supported
    has_cursor_management: false,
    has_ime: true,
    max_touch_points: 10,
}
```

## Feature Flags

### `android`

Enables Android-specific functionality:

```toml
[dependencies]
uzor-mobile = { version = "0.1.0", features = ["android"] }
```

Provides:
- Android NDK integration
- JNI bindings for Java APIs
- Android-specific clipboard, theme detection, etc.

### `ios`

Enables iOS-specific functionality:

```toml
[dependencies]
uzor-mobile = { version = "0.1.0", features = ["ios"] }
```

Provides:
- UIKit integration
- Objective-C bindings
- iOS-specific clipboard, theme detection, etc.

## Current Implementation Status

### ✅ Complete

- Platform trait implementations
- Gesture recognition framework
- Feature flag structure
- Stub backend (compiles on desktop)
- Comprehensive tests

### 🚧 Stub Implementations

The following require native integration:

#### Android (via NDK + JNI)
- [ ] Get screen size from Display API
- [ ] Get density from DisplayMetrics
- [ ] Window insets from WindowInsets API
- [ ] Haptic feedback via Vibrator
- [ ] Touch event processing from native activity
- [ ] Clipboard via ClipboardManager
- [ ] URL opening via Intent.ACTION_VIEW
- [ ] Theme detection via Configuration
- [ ] Virtual keyboard via InputMethodManager

#### iOS (via UIKit + Objective-C)
- [ ] Get screen size from UIScreen
- [ ] Get scale from UIScreen.scale
- [ ] Safe area insets from UIWindow
- [ ] Haptic feedback via UIImpactFeedbackGenerator
- [ ] Touch event processing from UIApplication
- [ ] Clipboard via UIPasteboard
- [ ] URL opening via UIApplication.openURL
- [ ] Theme detection via UITraitCollection
- [ ] Virtual keyboard via UITextField

### Next Steps

1. **Android Native Integration**
   - Setup NDK activity lifecycle
   - Implement JNI wrappers for Java APIs
   - Handle touch events from native activity

2. **iOS Native Integration**
   - Setup UIKit integration
   - Implement Objective-C bindings
   - Handle touch events from UIApplication

3. **Rendering Integration**
   - Provide CAMetalLayer access (iOS)
   - Provide ANativeWindow access (Android)
   - Integrate with wgpu/Vello rendering

## Gesture Recognition

The `common` module provides a gesture recognizer for high-level touch gestures:

```rust
use uzor_mobile::common::{GestureRecognizer, GestureEvent};

let mut recognizer = GestureRecognizer::new();

// Feed touch events
recognizer.touch_start(0, 100.0, 200.0);
recognizer.touch_move(0, 200.0, 210.0);
if let Some(gesture) = recognizer.touch_end(0, 200.0, 210.0) {
    match gesture {
        GestureEvent::Tap { x, y } => {
            println!("Tapped at ({}, {})", x, y);
        }
        GestureEvent::Swipe { direction, distance } => {
            println!("Swiped {:?} for {}", direction, distance);
        }
        GestureEvent::Pinch { scale, center_x, center_y } => {
            println!("Pinched with scale {}", scale);
        }
        _ => {}
    }
}
```

Supported gestures:
- **Tap**: Quick touch and release
- **Long Press**: Touch held for duration
- **Swipe**: Fast directional movement
- **Pinch**: Two-finger zoom in/out
- **Rotation**: Two-finger rotation (TODO)

## Development Notes

### Compiling for Desktop

During development, you can compile on desktop platforms. The stub backend will be used:

```bash
cargo check  # Uses StubBackend
cargo test   # Tests run with stub
```

### Compiling for Android

Requires Android NDK and cargo-mobile2:

```bash
# Install cargo-mobile2
cargo install --git https://github.com/tauri-apps/cargo-mobile2

# Build for Android
cargo mobile android build --release
```

### Compiling for iOS

Requires Xcode and cargo-mobile2:

```bash
# Build for iOS
cargo mobile ios build --release
```

## Testing

Run tests:

```bash
cargo test
```

Test with feature flags:

```bash
cargo test --features android
cargo test --features ios
```

## Related Crates

- `uzor-core` - Platform-agnostic uzor core
- `uzor-desktop` - Desktop platform backend (winit)
- `uzor-web` - Web platform backend (WASM)

## References

- [Android NDK Guide](https://developer.android.com/ndk/guides)
- [iOS UIKit Documentation](https://developer.apple.com/documentation/uikit)
- [cargo-mobile2](https://github.com/tauri-apps/cargo-mobile2)
- [Touch Events Specification](https://www.w3.org/TR/touch-events/)
