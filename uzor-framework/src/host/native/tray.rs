//! System tray icon + menu (moved from `uzor-desktop`, design §7.3).
//!
//! Menu selections become [`HostEvent::Tray`](crate::HostEvent::Tray) via the
//! native host's about_to_wait drain — never a second input path.

use std::collections::HashMap;

use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use uzor::RgbaIcon;

use crate::types::ids::TrayItemId;

/// Errors from tray construction / icon updates.
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

/// High-level tray events drained by [`TrayHandle::next_event`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayEvent {
    /// A context menu item was selected.
    MenuClick(TrayItemId),
    /// Left single-click on the tray icon.
    LeftClick,
    /// Right click on the tray icon.
    RightClick,
    /// Left double-click on the tray icon.
    DoubleClick,
}

/// One tray menu entry declared at startup.
#[derive(Clone, Debug)]
pub struct TrayMenuItem {
    /// Stable id delivered as [`HostEvent::Tray`](crate::HostEvent::Tray).
    pub id: TrayItemId,
    /// Visible label.
    pub label: String,
    /// Whether the item is enabled.
    pub enabled: bool,
}

/// Spec for a system-tray icon (native-only `RuntimeConfig` field).
#[derive(Clone, Debug, Default)]
pub struct TraySpec {
    /// Hover tooltip.
    pub tooltip: Option<String>,
    /// Optional RGBA icon; the host may fill from the window icon when absent.
    pub icon: Option<RgbaIcon>,
    /// Context-menu entries.
    pub items: Vec<TrayMenuItem>,
}

/// Fluent builder for a system tray icon + context menu.
#[derive(Default)]
pub struct TrayBuilder {
    icon: Option<RgbaIcon>,
    tooltip: Option<String>,
    items: Vec<TrayMenuItem>,
}

impl TrayBuilder {
    /// Empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the tray icon.
    pub fn icon(mut self, icon: RgbaIcon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Set the hover tooltip.
    pub fn tooltip(mut self, t: impl Into<String>) -> Self {
        self.tooltip = Some(t.into());
        self
    }

    /// Add an enabled menu item.
    pub fn menu_item(mut self, id: TrayItemId, label: impl Into<String>) -> Self {
        self.items.push(TrayMenuItem {
            id,
            label: label.into(),
            enabled: true,
        });
        self
    }

    /// Add a disabled menu item.
    pub fn menu_item_disabled(mut self, id: TrayItemId, label: impl Into<String>) -> Self {
        self.items.push(TrayMenuItem {
            id,
            label: label.into(),
            enabled: false,
        });
        self
    }

    /// Build from a [`TraySpec`].
    pub fn from_spec(spec: TraySpec) -> Self {
        Self {
            icon: spec.icon,
            tooltip: spec.tooltip,
            items: spec.items,
        }
    }

    /// Create the OS tray icon. Must run on the main thread after the event
    /// loop has started.
    pub fn build(self) -> Result<TrayHandle, TrayError> {
        let menu = Menu::new();
        let mut id_map: HashMap<tray_icon::menu::MenuId, TrayItemId> = HashMap::new();

        for item in &self.items {
            let mi = MenuItem::new(&item.label, item.enabled, None);
            id_map.insert(mi.id().clone(), item.id);
            menu.append(&mi)
                .map_err(|e| TrayError::Build(e.to_string()))?;
        }

        let mut builder = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false);

        if let Some(tooltip) = &self.tooltip {
            builder = builder.with_tooltip(tooltip);
        }

        let icon_dims = self.icon.as_ref().map(|i| (i.width, i.height));

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

/// Live system tray handle. Drop removes the icon.
pub struct TrayHandle {
    tray: TrayIcon,
    id_map: HashMap<tray_icon::menu::MenuId, TrayItemId>,
    icon_dims: (u32, u32),
}

impl TrayHandle {
    /// Replace the tray icon.
    pub fn set_icon(&mut self, icon: RgbaIcon) -> Result<(), TrayError> {
        self.icon_dims = (icon.width, icon.height);
        let os_icon = Icon::from_rgba(icon.pixels, icon.width, icon.height)
            .map_err(|e| TrayError::Icon(e.to_string()))?;
        self.tray
            .set_icon(Some(os_icon))
            .map_err(|e| TrayError::Icon(e.to_string()))
    }

    /// Update the tooltip.
    pub fn set_tooltip(&mut self, text: &str) {
        let _ = self.tray.set_tooltip(Some(text));
    }

    /// Drain one pending tray event (non-blocking).
    pub fn next_event(&self) -> Option<TrayEvent> {
        if let Ok(menu_ev) = MenuEvent::receiver().try_recv() {
            if let Some(id) = self.id_map.get(&menu_ev.id) {
                return Some(TrayEvent::MenuClick(*id));
            }
        }
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

    /// Remembered icon dimensions.
    pub fn icon_dims(&self) -> (u32, u32) {
        self.icon_dims
    }
}
