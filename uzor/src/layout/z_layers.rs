use super::types::OverlayKind;

/// Z-layer table: maps overlay kinds to draw-order integers.
///
/// Higher values render on top. All values are configurable at runtime via
/// `set`; defaults follow the uzor z-order model.
#[derive(Debug, Clone)]
pub struct ZLayerTable {
    /// Z for docked panel content.
    pub dock: i32,
    /// Z for floating dock windows.
    pub floating: i32,
    /// Z for `OverlayKind::Dropdown`.
    pub dropdown: i32,
    /// Z for `OverlayKind::Popup`.
    pub popup: i32,
    /// Z for `OverlayKind::Modal`.
    pub modal: i32,
    /// Z for `OverlayKind::ContextMenu`.
    pub context_menu: i32,
    /// Z for `OverlayKind::ColorPicker`.
    pub color_picker: i32,
    /// Z for `OverlayKind::Tooltip`.
    pub tooltip: i32,
}

impl Default for ZLayerTable {
    fn default() -> Self {
        Self {
            dock: 0,
            floating: 1,
            dropdown: 2,
            popup: 3,
            modal: 4,
            context_menu: 5,
            color_picker: 6,
            tooltip: 7,
        }
    }
}

impl ZLayerTable {
    /// Return the z value for the given overlay kind.
    pub fn z_for(&self, kind: OverlayKind) -> i32 {
        match kind {
            OverlayKind::Dropdown => self.dropdown,
            OverlayKind::Popup => self.popup,
            OverlayKind::Modal => self.modal,
            OverlayKind::ContextMenu => self.context_menu,
            OverlayKind::ColorPicker => self.color_picker,
            OverlayKind::Tooltip => self.tooltip,
        }
    }

    /// Override the z value for a given overlay kind.
    pub fn set(&mut self, kind: OverlayKind, z: i32) {
        match kind {
            OverlayKind::Dropdown => self.dropdown = z,
            OverlayKind::Popup => self.popup = z,
            OverlayKind::Modal => self.modal = z,
            OverlayKind::ContextMenu => self.context_menu = z,
            OverlayKind::ColorPicker => self.color_picker = z,
            OverlayKind::Tooltip => self.tooltip = z,
        }
    }
}
