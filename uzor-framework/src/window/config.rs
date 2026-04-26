//! Window configuration supplied by the app before window creation.

/// Saved window geometry for session restore.
#[derive(Debug, Clone, Copy)]
pub struct WindowGeom {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// App-supplied settings for creating a new winit window.
pub struct WindowConfig {
    /// Window title bar text.
    pub title: String,
    /// Logical pixel size `(width, height)` for the initial window dimensions.
    pub initial_size: (u32, u32),
    /// Minimum logical pixel size `(width, height)`.  `None` = no minimum.
    pub min_size: Option<(u32, u32)>,
    /// Initial window position in physical pixels.  `None` = OS default.
    pub initial_position: Option<(i32, i32)>,
    /// Whether to show OS-native window decorations (title bar, border).
    ///
    /// `false` means chromeless — the app draws its own window controls.
    pub decorations: bool,
    /// Optional window icon.
    pub icon: Option<winit::window::Icon>,
    /// Restore saved geometry from a previous session (position + size in
    /// physical pixels).  Overrides `initial_size` and `initial_position`.
    pub restore_geom: Option<WindowGeom>,
    /// Cascade position: offset by +30 physical px from the outer position of
    /// this existing window.
    pub cascade_from: Option<winit::window::WindowId>,
    /// When `false` the window is created invisible and made visible only
    /// after the first GPU frame is presented.  Eliminates the white-flash.
    pub start_visible: bool,
    /// Override automatic backend detection.  `None` = auto-detect from wgpu
    /// adapter capabilities.
    pub backend_hint: Option<uzor_render_hub::RenderBackend>,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "uzor".to_string(),
            initial_size: (1200, 800),
            min_size: Some((400, 300)),
            initial_position: None,
            decorations: false,
            icon: None,
            restore_geom: None,
            cascade_from: None,
            start_visible: false,
            backend_hint: None,
        }
    }
}
