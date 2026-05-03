//! `DropdownBuilder` — chainable, default-friendly wrapper around
//! `register_layout_manager_dropdown`.
//!
//! Usage:
//! ```ignore
//! let h = layout.add_dropdown("file-menu");
//! lm::dropdown(&h)
//!     .items(&items)
//!     .anchor(button_rect)
//!     .build(&mut layout, &mut render);
//! ```

use uzor::core::types::Rect;
use uzor::docking::panels::DockPanel;
use uzor::layout::{DropdownHandle, DropdownNode, LayoutManager, LayoutNodeId};
use uzor::render::RenderContext;
use uzor::types::{OverflowMode, SizeMode};
use uzor::ui::widgets::composite::dropdown::input::register_layout_manager_dropdown;
use uzor::ui::widgets::composite::dropdown::settings::DropdownSettings;
use uzor::ui::widgets::composite::dropdown::types::{
    DropdownItem, DropdownRenderKind, DropdownView, DropdownViewKind, SubmenuWidth,
};

/// Chainable builder for a dropdown overlay.
pub struct DropdownBuilder<'a> {
    handle:           &'a DropdownHandle,
    parent:           LayoutNodeId,
    slot_id:          Option<&'a str>,
    overlay_rect:     Option<Rect>,
    anchor:           Option<Rect>,
    position_override:Option<(f64, f64)>,
    open:             bool,
    items:            &'a [DropdownItem<'a>],
    hovered_id:       Option<&'a str>,
    submenu:          Option<(&'a str, &'a [DropdownItem<'a>])>,
    submenu_hovered:  Option<&'a str>,
    submenu_width:    SubmenuWidth,
    size_mode:        SizeMode,
    overflow:         OverflowMode,
    settings:         Option<DropdownSettings>,
    kind:             DropdownRenderKind,
}

/// Entry point: start a `DropdownBuilder` for the given handle.
pub fn dropdown<'a>(handle: &'a DropdownHandle) -> DropdownBuilder<'a> {
    DropdownBuilder::new(handle)
}

impl<'a> DropdownBuilder<'a> {
    /// New builder with all fields at default.
    pub fn new(handle: &'a DropdownHandle) -> Self {
        Self {
            handle,
            parent:            LayoutNodeId::ROOT,
            slot_id:           None,
            overlay_rect:      None,
            anchor:            None,
            position_override: None,
            open:              true,
            items:             &[],
            hovered_id:        None,
            submenu:           None,
            submenu_hovered:   None,
            submenu_width:     SubmenuWidth::default(),
            size_mode:         SizeMode::default(),
            overflow:          OverflowMode::Clip,
            settings:          None,
            kind:              DropdownRenderKind::Flat,
        }
    }

    /// Override the parent layout node (default `LayoutNodeId::ROOT`).
    pub fn parent(mut self, p: LayoutNodeId) -> Self { self.parent = p; self }

    /// Override the overlay slot id (default = handle id).
    pub fn slot_id(mut self, s: &'a str) -> Self { self.slot_id = Some(s); self }

    /// Anchor rect of the trigger button (re-positioning on viewport resize).
    pub fn anchor(mut self, r: Rect) -> Self { self.anchor = Some(r); self }

    /// Explicit screen-space origin override (takes priority over anchor).
    pub fn origin(mut self, o: (f64, f64)) -> Self { self.position_override = Some(o); self }

    /// Explicit overlay rect override.
    pub fn rect(mut self, r: Rect) -> Self { self.overlay_rect = Some(r); self }

    /// Whether the dropdown is currently open (default `true`).
    pub fn open(mut self, on: bool) -> Self { self.open = on; self }

    /// Item rows (default empty).
    pub fn items(mut self, items: &'a [DropdownItem<'a>]) -> Self { self.items = items; self }

    /// Currently-hovered item id (default `None`).
    pub fn hovered_id(mut self, id: &'a str) -> Self { self.hovered_id = Some(id); self }

    /// Open submenu data: `(trigger_item_id, submenu_items)`.
    pub fn submenu(mut self, trigger_id: &'a str, items: &'a [DropdownItem<'a>]) -> Self {
        self.submenu = Some((trigger_id, items));
        self
    }

    /// Currently-hovered submenu item id (default `None`).
    pub fn submenu_hovered(mut self, id: &'a str) -> Self { self.submenu_hovered = Some(id); self }

    /// Submenu width strategy (`Auto` or `InheritParent`).
    pub fn submenu_width(mut self, w: SubmenuWidth) -> Self { self.submenu_width = w; self }

    /// Sizing mode (`AutoFit` measures content, `Fixed(w,h)` pins).
    pub fn size_mode(mut self, m: SizeMode) -> Self { self.size_mode = m; self }

    /// Body overflow strategy (default `Clip`).
    pub fn overflow(mut self, m: OverflowMode) -> Self { self.overflow = m; self }

    /// Override visual settings (default `DropdownSettings::default()`).
    pub fn settings(mut self, s: DropdownSettings) -> Self { self.settings = Some(s); self }

    /// Override render kind (default `DropdownRenderKind::Flat`).
    pub fn kind(mut self, k: DropdownRenderKind) -> Self { self.kind = k; self }

    /// Terminal call — register and draw the dropdown panel.
    pub fn build<P: DockPanel>(
        self,
        layout: &mut LayoutManager<P>,
        render: &mut dyn RenderContext,
    ) -> Option<DropdownNode> {
        let slot_id = self.slot_id
            .map(str::to_owned)
            .unwrap_or_else(|| self.handle.id_str().to_string());

        let overlay_rect = self.overlay_rect.unwrap_or_else(|| {
            let (x, y) = self.position_override
                .or_else(|| self.anchor.map(|a| (a.x, a.y + a.height)))
                .unwrap_or((0.0, 0.0));
            let (w, h) = match self.size_mode {
                SizeMode::Fixed(w, h) => (w, h),
                _                     => (200.0, 240.0),
            };
            Rect::new(x, y, w, h)
        });

        let mut view = DropdownView {
            anchor:            self.anchor,
            position_override: self.position_override,
            open:              self.open,
            kind:              DropdownViewKind::Flat {
                items:              self.items,
                hovered_id:         self.hovered_id,
                submenu_items:      self.submenu,
                submenu_hovered_id: self.submenu_hovered,
            },
            size_mode:    self.size_mode,
            overflow:     self.overflow,
            submenu_width:self.submenu_width,
        };

        let settings = self.settings.unwrap_or_default();

        register_layout_manager_dropdown(
            layout,
            render,
            self.parent,
            &slot_id,
            self.handle,
            overlay_rect,
            self.anchor,
            &mut view,
            &settings,
            self.kind,
        )
    }
}
