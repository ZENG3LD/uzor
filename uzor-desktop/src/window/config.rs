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
    pub decorations: bool,
    /// Optional window icon.
    pub icon: Option<winit::window::Icon>,
    /// Restore saved geometry from a previous session.
    pub restore_geom: Option<WindowGeom>,
    /// Cascade position offset.
    pub cascade_from: Option<winit::window::WindowId>,
    /// When `false` the window is created invisible until first GPU frame.
    pub start_visible: bool,
    /// Override automatic backend detection. Takes precedence over
    /// [`Self::render_family`] entirely — an explicit backend skips
    /// family resolution (owner decision 2026-07-24).
    pub backend_hint: Option<uzor_render_hub::RenderBackend>,
    /// Coarse render family [`create_window`](super::creation::create_window)
    /// resolves autodetect against when `backend_hint` is `None` — the
    /// `UZOR_RENDER_FAMILY` env var (case-insensitive `vello`/`urx`), if
    /// set and valid, still overrides this field; see
    /// `uzor_render_hub::resolve_render_family_from_process_env`.
    /// Defaults to [`uzor_render_hub::RenderFamily::default`] (`Vello`) —
    /// no default flip, ever.
    pub render_family: uzor_render_hub::RenderFamily,
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
            render_family: uzor_render_hub::RenderFamily::default(),
        }
    }
}
