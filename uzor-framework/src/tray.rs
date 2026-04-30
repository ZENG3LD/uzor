//! System tray icon + context menu wrapper around the `tray-icon` crate.
//!
//! Saves consumers ~80 LOC of boilerplate per app — the same pattern is already
//! duplicated in `zengeld-rlt`, `atol-ecommerce`, and others.
//!
//! # Quick start
//!
//! ```no_run
//! use uzor_framework::tray::{TrayBuilder, TrayEvent};
//! use uzor_window_hub::RgbaIcon;
//!
//! let rgba = vec![0u8; 32 * 32 * 4]; // your icon pixels
//! let icon = RgbaIcon::from_rgba(32, 32, rgba);
//!
//! let mut tray = TrayBuilder::new()
//!     .icon(icon)
//!     .tooltip("My App")
//!     .menu_item("show", "Show window")
//!     .menu_item("quit", "Quit")
//!     .build()
//!     .expect("tray creation failed");
//!
//! // In event loop:
//! while let Some(ev) = tray.next_event() {
//!     match ev {
//!         TrayEvent::MenuClick(id) if id == "quit" => std::process::exit(0),
//!         TrayEvent::LeftClick => { /* show window */ }
//!         _ => {}
//!     }
//! }
//! ```

use std::collections::HashMap;

use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use uzor_window_hub::RgbaIcon;

// ── TrayError ─────────────────────────────────────────────────────────────────

/// Errors produced by tray-related operations.
#[derive(Debug)]
pub enum TrayError {
    /// Tray icon or menu construction failed.
    Build(String),
    /// Icon conversion / update failed.
    Icon(String),
}

impl std::fmt::Display for TrayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrayError::Build(s) => write!(f, "tray build error: {s}"),
            TrayError::Icon(s) => write!(f, "tray icon error: {s}"),
        }
    }
}

impl std::error::Error for TrayError {}

// ── TrayEvent ─────────────────────────────────────────────────────────────────

/// High-level tray events delivered by [`TrayHandle::next_event`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayEvent {
    /// A context menu item was clicked; carries the string id passed to
    /// [`TrayBuilder::menu_item`].
    MenuClick(String),
    /// Left mouse button single-click on the tray icon.
    LeftClick,
    /// Right mouse button click on the tray icon.
    RightClick,
    /// Left mouse button double-click on the tray icon.
    DoubleClick,
}

// ── TrayBuilder ───────────────────────────────────────────────────────────────

/// Fluent builder for a system tray icon + context menu.
pub struct TrayBuilder {
    icon: Option<RgbaIcon>,
    tooltip: Option<String>,
    /// `(our_id, label, enabled)`
    items: Vec<(String, String, bool)>,
}

impl Default for TrayBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TrayBuilder {
    /// Create a new builder with no icon, no tooltip, and an empty menu.
    pub fn new() -> Self {
        Self {
            icon: None,
            tooltip: None,
            items: Vec::new(),
        }
    }

    /// Set the tray icon.
    pub fn icon(mut self, icon: RgbaIcon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Set the tooltip shown when hovering the tray icon.
    pub fn tooltip(mut self, t: impl Into<String>) -> Self {
        self.tooltip = Some(t.into());
        self
    }

    /// Add an enabled context-menu item.
    ///
    /// `id` is the string returned in [`TrayEvent::MenuClick`] when this item
    /// is clicked. `label` is the visible text.
    pub fn menu_item(mut self, id: impl Into<String>, label: impl Into<String>) -> Self {
        self.items.push((id.into(), label.into(), true));
        self
    }

    /// Add a disabled (greyed-out) context-menu item.
    pub fn menu_item_disabled(
        mut self,
        id: impl Into<String>,
        label: impl Into<String>,
    ) -> Self {
        self.items.push((id.into(), label.into(), false));
        self
    }

    /// Consume the builder and create the OS tray icon.
    ///
    /// Must be called on the **main thread** after the event loop has started.
    ///
    /// # Errors
    ///
    /// Returns [`TrayError::Build`] if the OS tray icon or menu cannot be
    /// created (e.g. no system tray support on the current platform).
    pub fn build(self) -> Result<TrayHandle, TrayError> {
        // Build the context menu and keep a mapping from MenuId → our string id.
        let menu = Menu::new();
        let mut id_map: HashMap<tray_icon::menu::MenuId, String> = HashMap::new();

        for (our_id, label, enabled) in &self.items {
            let item = MenuItem::new(label, *enabled, None);
            id_map.insert(item.id().clone(), our_id.clone());
            menu.append(&item).map_err(|e| TrayError::Build(e.to_string()))?;
        }

        // Build tray icon itself.
        let mut builder = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false);

        if let Some(tooltip) = &self.tooltip {
            builder = builder.with_tooltip(tooltip);
        }

        let icon_dims: Option<(u32, u32)> = self.icon.as_ref().map(|i| (i.width, i.height));

        if let Some(rgba) = self.icon {
            let icon = Icon::from_rgba(rgba.pixels, rgba.width, rgba.height)
                .map_err(|e| TrayError::Build(e.to_string()))?;
            builder = builder.with_icon(icon);
        }

        let tray = builder
            .build()
            .map_err(|e| TrayError::Build(e.to_string()))?;

        Ok(TrayHandle {
            tray,
            id_map,
            icon_dims: icon_dims.unwrap_or((32, 32)),
        })
    }
}

// ── TrayHandle ────────────────────────────────────────────────────────────────

/// A live system tray icon + menu handle.
///
/// Drop to remove the tray icon from the OS tray area.
pub struct TrayHandle {
    tray: TrayIcon,
    /// Maps OS `MenuId` → caller-provided string id.
    id_map: HashMap<tray_icon::menu::MenuId, String>,
    /// Width × height remembered for icon replacement.
    icon_dims: (u32, u32),
}

impl TrayHandle {
    /// Replace the current tray icon.
    ///
    /// # Errors
    ///
    /// Returns [`TrayError::Icon`] if the pixel data is invalid or the OS
    /// rejects the icon.
    pub fn set_icon(&mut self, icon: RgbaIcon) -> Result<(), TrayError> {
        self.icon_dims = (icon.width, icon.height);
        let os_icon = Icon::from_rgba(icon.pixels, icon.width, icon.height)
            .map_err(|e| TrayError::Icon(e.to_string()))?;
        self.tray
            .set_icon(Some(os_icon))
            .map_err(|e| TrayError::Icon(e.to_string()))
    }

    /// Update the tooltip text.
    pub fn set_tooltip(&mut self, text: &str) {
        let _ = self.tray.set_tooltip(Some(text));
    }

    /// Drain one pending tray event (non-blocking).
    ///
    /// Returns `None` when there are no pending events. Call in the main-thread
    /// event loop (e.g. in `about_to_wait` or after `RedrawRequested`).
    ///
    /// Both `tray_icon` OS events and `muda` menu events are checked; the first
    /// available one is returned.
    pub fn next_event(&self) -> Option<TrayEvent> {
        // Check menu events first (they carry more information).
        if let Ok(menu_ev) = MenuEvent::receiver().try_recv() {
            if let Some(our_id) = self.id_map.get(&menu_ev.id) {
                return Some(TrayEvent::MenuClick(our_id.clone()));
            }
        }

        // Then check raw tray icon events.
        if let Ok(tray_ev) = TrayIconEvent::receiver().try_recv() {
            match tray_ev {
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => return Some(TrayEvent::LeftClick),

                TrayIconEvent::Click {
                    button: MouseButton::Right,
                    button_state: MouseButtonState::Up,
                    ..
                } => return Some(TrayEvent::RightClick),

                TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                } => return Some(TrayEvent::DoubleClick),

                _ => {}
            }
        }

        None
    }

    /// Returns the remembered icon dimensions `(width, height)` in pixels.
    pub fn icon_dims(&self) -> (u32, u32) {
        self.icon_dims
    }
}
