//! uzor-macos — macOS-style themes and widgets for the uzor UI framework
//!
//! Provides pixel-perfect macOS Ventura/Sonoma styling including:
//! - 8 appearance modes (Light, Dark, Vibrant, Accessible variants)
//! - 70+ semantic color tokens
//! - 12-level typography scale
//! - Widget renderers: Button, Menu, Checkbox, Radio, Switch, Input, Dialog, Progress, Tabs, Traffic Lights
//! - Animation presets: spring physics, modal transitions, dock magnification
//! - Effects: multi-layer shadows, gradient approximation

pub mod colors;
pub mod typography;
pub mod themes;
pub mod widgets;
pub mod animations;
pub mod icons;
pub mod effects;
pub mod presets;

// Re-export commonly used types
pub use colors::{AppearanceMode, WidgetState, ColorPalette};
pub use presets::ventura::VenturaPreset;
