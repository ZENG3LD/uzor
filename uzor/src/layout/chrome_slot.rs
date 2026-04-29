/// Chrome strip slot — the system-level titlebar/menubar at the top of the window.
///
/// Managed internally by `LayoutManager`; app developers configure it via
/// `layout_manager.chrome_mut()` but do not add/remove it at runtime.
#[derive(Debug, Clone)]
pub struct ChromeSlot {
    /// Preferred height in logical pixels (default 32 px).
    pub height: f32,
    /// Whether the chrome strip is currently visible.
    /// When `false`, chrome contributes zero height to layout.
    pub visible: bool,
}

impl Default for ChromeSlot {
    fn default() -> Self {
        Self { height: 32.0, visible: true }
    }
}
