//! Container settings bundle — theme + style boxed together.

use super::style::{ContainerStyle, DefaultContainerStyle};
use super::theme::{ContainerTheme, DefaultContainerTheme};

/// Bundles a theme and style for a container widget.
///
/// Use `ContainerSettings::default()` for a generic dark-mode card appearance,
/// or construct with typed style presets from `super::style`.
pub struct ContainerSettings {
    pub theme: Box<dyn ContainerTheme>,
    pub style: Box<dyn ContainerStyle>,
}

impl Default for ContainerSettings {
    fn default() -> Self {
        Self {
            theme: Box::<DefaultContainerTheme>::default(),
            style: Box::new(DefaultContainerStyle),
        }
    }
}
