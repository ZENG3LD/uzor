//! Per-window host state: winit handle, render state, mapper (design §7.3).

use std::sync::Arc;

use uzor::{CornerStyle, CursorIcon, Rect, ResizeDirection, RgbaIcon};
use uzor_render_hub::WindowRenderState;
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use winit::window::{
    CursorGrabMode, Fullscreen, Icon, ResizeDirection as WinitResizeDirection, Window,
    WindowId as WinitWindowId,
};

use super::executor::OsWindow;
use super::mapper::WinitMapper;
use super::win32;
use crate::types::ids::WindowId;
use crate::types::window::{
    Attention, CursorMode, FullscreenMode, ImePurpose, Point, SizePx, ThemeHint, WindowSpec,
};

/// One OS window owned by the native host.
pub struct NativeWindow {
    /// Framework window id.
    pub id: WindowId,
    /// App-facing key from the spawn spec.
    pub key: String,
    /// winit window.
    pub window: Arc<Window>,
    /// Hub render state for this surface.
    pub render_state: WindowRenderState,
    /// Per-window input mapper.
    pub mapper: WinitMapper,
    /// Last known decorations flag (for borderless-resize install).
    pub decorations: bool,
}

impl NativeWindow {
    /// winit window id.
    pub fn winit_id(&self) -> WinitWindowId {
        self.window.id()
    }

    /// Borrow as an [`OsWindow`] executor target.
    pub fn as_os(&mut self) -> WinitOsWindow<'_> {
        WinitOsWindow {
            window: &self.window,
            decorations: &mut self.decorations,
        }
    }
}

/// Live [`OsWindow`] over a winit `Window`.
pub struct WinitOsWindow<'a> {
    window: &'a Arc<Window>,
    decorations: &'a mut bool,
}

impl OsWindow for WinitOsWindow<'_> {
    fn set_title(&mut self, title: &str) {
        self.window.set_title(title);
    }

    fn set_icon(&mut self, icon: Option<&RgbaIcon>) {
        let os = icon.and_then(|r| Icon::from_rgba(r.pixels.clone(), r.width, r.height).ok());
        self.window.set_window_icon(os);
    }

    fn set_decorations(&mut self, on: bool) {
        self.window.set_decorations(on);
        *self.decorations = on;
        if !on {
            win32::install_borderless_resize(self.window);
        }
    }

    fn set_corner_style(&mut self, style: CornerStyle) {
        win32::set_corner_style(self.window, style);
    }

    fn set_border_color(&mut self, color: Option<u32>) {
        win32::set_border_color(self.window, color);
    }

    fn set_theme(&mut self, theme: Option<ThemeHint>) {
        let t = theme.map(|h| match h {
            ThemeHint::Light => winit::window::Theme::Light,
            ThemeHint::Dark => winit::window::Theme::Dark,
        });
        self.window.set_theme(t);
    }

    fn set_visible(&mut self, on: bool) {
        self.window.set_visible(on);
    }

    fn set_minimized(&mut self, on: bool) {
        self.window.set_minimized(on);
    }

    fn set_maximized(&mut self, on: bool) {
        self.window.set_maximized(on);
    }

    fn set_fullscreen(&mut self, mode: FullscreenMode) {
        let fs = match mode {
            FullscreenMode::Off => None,
            FullscreenMode::Borderless => Some(Fullscreen::Borderless(None)),
        };
        self.window.set_fullscreen(fs);
    }

    fn set_resizable(&mut self, on: bool) {
        self.window.set_resizable(on);
    }

    fn set_min_inner_size(&mut self, size: Option<SizePx>) {
        let s = size.map(|s| LogicalSize::new(f64::from(s.width), f64::from(s.height)));
        self.window.set_min_inner_size(s);
    }

    fn set_max_inner_size(&mut self, size: Option<SizePx>) {
        let s = size.map(|s| LogicalSize::new(f64::from(s.width), f64::from(s.height)));
        self.window.set_max_inner_size(s);
    }

    fn set_outer_rect(&mut self, position: (i32, i32), size: SizePx) -> (i32, i32, SizePx) {
        self.window
            .set_outer_position(PhysicalPosition::new(position.0, position.1));
        let _ = self
            .window
            .request_inner_size(PhysicalSize::new(size.width, size.height));
        (position.0, position.1, size)
    }

    fn request_inner_size(&mut self, size: SizePx) {
        let _ = self
            .window
            .request_inner_size(PhysicalSize::new(size.width, size.height));
    }

    fn drag_window(&mut self) {
        if let Err(e) = self.window.drag_window() {
            log::warn!("drag_window failed: {e}");
        }
    }

    fn drag_resize_window(&mut self, dir: ResizeDirection) {
        let wd = map_resize_dir(dir);
        if let Err(e) = self.window.drag_resize_window(wd) {
            log::warn!("drag_resize_window failed: {e}");
        }
    }

    fn show_window_menu(&mut self, at: (i32, i32)) {
        let _ = self.window.show_window_menu(PhysicalPosition::new(at.0, at.1));
    }

    fn set_cursor(&mut self, icon: CursorIcon) {
        self.window.set_cursor(map_cursor(icon));
    }

    fn set_cursor_visible(&mut self, on: bool) {
        self.window.set_cursor_visible(on);
    }

    fn set_cursor_mode(&mut self, mode: CursorMode) {
        let grab = match mode {
            CursorMode::Normal => CursorGrabMode::None,
            CursorMode::Confined => CursorGrabMode::Confined,
            CursorMode::LockedHidden => CursorGrabMode::Locked,
        };
        if let Err(e) = self.window.set_cursor_grab(grab) {
            if matches!(mode, CursorMode::LockedHidden) {
                // Fall back to confined when locked is unsupported.
                if let Err(e2) = self.window.set_cursor_grab(CursorGrabMode::Confined) {
                    log::warn!("set_cursor_grab failed: {e}; fallback: {e2}");
                }
            } else {
                log::warn!("set_cursor_grab failed: {e}");
            }
        }
        self.window
            .set_cursor_visible(!matches!(mode, CursorMode::LockedHidden));
    }

    fn set_cursor_position(&mut self, pos: Point) {
        if let Err(e) = self
            .window
            .set_cursor_position(LogicalPosition::new(pos.x, pos.y))
        {
            log::warn!("set_cursor_position failed: {e}");
        }
    }

    fn set_ime_allowed(&mut self, on: bool) {
        self.window.set_ime_allowed(on);
    }

    fn set_ime_cursor_area(&mut self, area: Rect) {
        self.window.set_ime_cursor_area(
            LogicalPosition::new(area.x, area.y),
            LogicalSize::new(area.width, area.height),
        );
    }

    fn set_ime_purpose(&mut self, purpose: ImePurpose) {
        let p = match purpose {
            ImePurpose::Normal => winit::window::ImePurpose::Normal,
            ImePurpose::Password => winit::window::ImePurpose::Password,
            ImePurpose::Terminal => winit::window::ImePurpose::Terminal,
        };
        self.window.set_ime_purpose(p);
    }

    fn focus_window(&mut self) {
        self.window.focus_window();
    }

    fn request_attention(&mut self, kind: Option<Attention>) {
        use winit::window::UserAttentionType;
        let t = kind.map(|k| match k {
            Attention::Critical => UserAttentionType::Critical,
            Attention::Informational => UserAttentionType::Informational,
        });
        self.window.request_user_attention(t);
    }

    fn request_redraw(&mut self) {
        self.window.request_redraw();
    }
}

fn map_resize_dir(dir: ResizeDirection) -> WinitResizeDirection {
    match dir {
        ResizeDirection::North => WinitResizeDirection::North,
        ResizeDirection::South => WinitResizeDirection::South,
        ResizeDirection::East => WinitResizeDirection::East,
        ResizeDirection::West => WinitResizeDirection::West,
        ResizeDirection::NorthEast => WinitResizeDirection::NorthEast,
        ResizeDirection::NorthWest => WinitResizeDirection::NorthWest,
        ResizeDirection::SouthEast => WinitResizeDirection::SouthEast,
        ResizeDirection::SouthWest => WinitResizeDirection::SouthWest,
    }
}

fn map_cursor(icon: CursorIcon) -> winit::window::CursorIcon {
    use winit::window::CursorIcon as W;
    match icon {
        CursorIcon::Default => W::Default,
        CursorIcon::None => W::Default,
        CursorIcon::ContextMenu => W::ContextMenu,
        CursorIcon::Help => W::Help,
        CursorIcon::PointingHand => W::Pointer,
        CursorIcon::Progress => W::Progress,
        CursorIcon::Wait => W::Wait,
        CursorIcon::Cell => W::Cell,
        CursorIcon::Crosshair => W::Crosshair,
        CursorIcon::Text => W::Text,
        CursorIcon::VerticalText => W::VerticalText,
        CursorIcon::Alias => W::Alias,
        CursorIcon::Copy => W::Copy,
        CursorIcon::Move => W::Move,
        CursorIcon::NoDrop => W::NoDrop,
        CursorIcon::NotAllowed => W::NotAllowed,
        CursorIcon::Grab => W::Grab,
        CursorIcon::Grabbing => W::Grabbing,
        CursorIcon::AllScroll => W::AllScroll,
        CursorIcon::ResizeHorizontal => W::EwResize,
        CursorIcon::ResizeVertical => W::NsResize,
        CursorIcon::ResizeNeSw => W::NeswResize,
        CursorIcon::ResizeNwSe => W::NwseResize,
        CursorIcon::ResizeEast => W::EResize,
        CursorIcon::ResizeWest => W::WResize,
        CursorIcon::ResizeNorth => W::NResize,
        CursorIcon::ResizeSouth => W::SResize,
        CursorIcon::ResizeNorthEast => W::NeResize,
        CursorIcon::ResizeNorthWest => W::NwResize,
        CursorIcon::ResizeSouthEast => W::SeResize,
        CursorIcon::ResizeSouthWest => W::SwResize,
        CursorIcon::ResizeColumn => W::ColResize,
        CursorIcon::ResizeRow => W::RowResize,
        CursorIcon::ZoomIn => W::ZoomIn,
        CursorIcon::ZoomOut => W::ZoomOut,
    }
}

/// Start-up helpers shared with window_create.
pub fn apply_spawn_style(window: &Window, spec: &WindowSpec) {
    if spec.corner_style != CornerStyle::Default {
        win32::set_corner_style(window, spec.corner_style);
    }
    if spec.border_color.is_some() {
        win32::set_border_color(window, spec.border_color);
    }
    if !spec.decorations {
        win32::install_borderless_resize(window);
    }
}
