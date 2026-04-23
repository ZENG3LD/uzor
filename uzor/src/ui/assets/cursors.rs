//! Cursor icon types (CSS cursor specification)

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum CursorIcon {
    #[default]
    Default,
    None,
    ContextMenu,
    Help,
    PointingHand,
    Progress,
    Wait,
    Cell,
    Crosshair,
    Text,
    VerticalText,
    Alias,
    Copy,
    Move,
    NoDrop,
    NotAllowed,
    Grab,
    Grabbing,
    AllScroll,
    ResizeHorizontal,
    ResizeVertical,
    ResizeNeSw,
    ResizeNwSe,
    ResizeEast,
    ResizeWest,
    ResizeNorth,
    ResizeSouth,
    ResizeNorthEast,
    ResizeNorthWest,
    ResizeSouthEast,
    ResizeSouthWest,
    ResizeColumn,
    ResizeRow,
    ZoomIn,
    ZoomOut,
}

impl CursorIcon {
    pub fn is_resize(&self) -> bool {
        matches!(
            self,
            CursorIcon::AllScroll
                | CursorIcon::ResizeHorizontal
                | CursorIcon::ResizeVertical
                | CursorIcon::ResizeNeSw
                | CursorIcon::ResizeNwSe
                | CursorIcon::ResizeEast
                | CursorIcon::ResizeWest
                | CursorIcon::ResizeNorth
                | CursorIcon::ResizeSouth
                | CursorIcon::ResizeNorthEast
                | CursorIcon::ResizeNorthWest
                | CursorIcon::ResizeSouthEast
                | CursorIcon::ResizeSouthWest
                | CursorIcon::ResizeColumn
                | CursorIcon::ResizeRow
        )
    }

    pub fn is_drag(&self) -> bool {
        matches!(self, CursorIcon::Grab | CursorIcon::Grabbing | CursorIcon::Move)
    }

    pub fn css_name(&self) -> &'static str {
        match self {
            CursorIcon::Default => "default",
            CursorIcon::None => "none",
            CursorIcon::ContextMenu => "context-menu",
            CursorIcon::Help => "help",
            CursorIcon::PointingHand => "pointer",
            CursorIcon::Progress => "progress",
            CursorIcon::Wait => "wait",
            CursorIcon::Cell => "cell",
            CursorIcon::Crosshair => "crosshair",
            CursorIcon::Text => "text",
            CursorIcon::VerticalText => "vertical-text",
            CursorIcon::Alias => "alias",
            CursorIcon::Copy => "copy",
            CursorIcon::Move => "move",
            CursorIcon::NoDrop => "no-drop",
            CursorIcon::NotAllowed => "not-allowed",
            CursorIcon::Grab => "grab",
            CursorIcon::Grabbing => "grabbing",
            CursorIcon::AllScroll => "all-scroll",
            CursorIcon::ResizeHorizontal => "ew-resize",
            CursorIcon::ResizeVertical => "ns-resize",
            CursorIcon::ResizeNeSw => "nesw-resize",
            CursorIcon::ResizeNwSe => "nwse-resize",
            CursorIcon::ResizeEast => "e-resize",
            CursorIcon::ResizeWest => "w-resize",
            CursorIcon::ResizeNorth => "n-resize",
            CursorIcon::ResizeSouth => "s-resize",
            CursorIcon::ResizeNorthEast => "ne-resize",
            CursorIcon::ResizeNorthWest => "nw-resize",
            CursorIcon::ResizeSouthEast => "se-resize",
            CursorIcon::ResizeSouthWest => "sw-resize",
            CursorIcon::ResizeColumn => "col-resize",
            CursorIcon::ResizeRow => "row-resize",
            CursorIcon::ZoomIn => "zoom-in",
            CursorIcon::ZoomOut => "zoom-out",
        }
    }
}
