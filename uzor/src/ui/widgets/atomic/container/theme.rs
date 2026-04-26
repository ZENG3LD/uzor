//! Container colour palette.

pub trait ContainerTheme {
    fn bg(&self)     -> &str;
    fn border(&self) -> &str;
    fn shadow(&self) -> &str;
}

#[derive(Default)]
pub struct DefaultContainerTheme;

impl ContainerTheme for DefaultContainerTheme {
    fn bg(&self)     -> &str { "#1e1e1e" }
    fn border(&self) -> &str { "#3a3a3a" }
    fn shadow(&self) -> &str { "#000000" }
}
