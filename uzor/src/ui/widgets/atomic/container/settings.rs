use super::style::{ContainerStyle, DefaultContainerStyle};
use super::theme::{ContainerTheme, DefaultContainerTheme};

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
