//! Named chrome the framework ships. Apps pick entries; they do not
//! invent a second hit vocabulary for the same slots.

/// Kind of chrome piece. Ready entries have paint + input in this crate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChromeKind {
    OverlayCard,
    OverlayHost,
    HitLayer,
    Keymap,
    SidebarRail,
    DockSplit,
}

/// One catalog row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CatalogEntry {
    pub id: &'static str,
    pub kind: ChromeKind,
    /// Paint + input live here. `false` = reserved slot, not built.
    pub ready: bool,
}

/// Everything the shell layer currently names.
pub const CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        id: "overlay-card",
        kind: ChromeKind::OverlayCard,
        ready: true,
    },
    CatalogEntry {
        id: "overlay-host",
        kind: ChromeKind::OverlayHost,
        ready: true,
    },
    CatalogEntry {
        id: "hit-layer",
        kind: ChromeKind::HitLayer,
        ready: true,
    },
    CatalogEntry {
        id: "keymap",
        kind: ChromeKind::Keymap,
        ready: true,
    },
    CatalogEntry {
        id: "sidebar-rail",
        kind: ChromeKind::SidebarRail,
        ready: false,
    },
    CatalogEntry {
        id: "dock-split",
        kind: ChromeKind::DockSplit,
        ready: false,
    },
];

pub fn lookup(id: &str) -> Option<&'static CatalogEntry> {
    CATALOG.iter().find(|e| e.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_entries_are_named() {
        assert!(CATALOG.iter().any(|e| e.id == "overlay-card" && e.ready));
        assert!(lookup("keymap").is_some_and(|e| e.ready));
        assert!(lookup("dock-split").is_some_and(|e| !e.ready));
        assert!(lookup("nope").is_none());
    }
}
