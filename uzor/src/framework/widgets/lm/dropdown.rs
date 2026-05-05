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

use crate::core::types::Rect;
use crate::layout::docking::DockPanel;
use crate::layout::{DropdownHandle, DropdownNode, LayoutManager, LayoutNodeId, StyleManager};
use crate::render::RenderContext;
use crate::types::{OverflowMode, SizeMode};
use crate::ui::widgets::composite::dropdown::input::register_layout_manager_dropdown;
use crate::ui::widgets::composite::dropdown::settings::DropdownSettings;
use crate::ui::widgets::composite::dropdown::style::DefaultDropdownStyle;
use crate::ui::widgets::composite::dropdown::theme::{DefaultDropdownTheme, DropdownTheme};
use crate::ui::widgets::composite::dropdown::types::{
    DropdownItem, DropdownRenderKind, DropdownView, DropdownViewKind, SubmenuWidth,
};

// =============================================================================
// StyledDropdownTheme
// =============================================================================

struct StyledDropdownTheme {
    bg:               String,
    border:           String,
    item_bg_hover:    String,
    item_bg_selected: String,
    item_text:        String,
    accent:           String,
    fallback:         DefaultDropdownTheme,
}

impl StyledDropdownTheme {
    fn from_styles(s: &StyleManager) -> Self {
        let accent     = s.color_or_owned("accent",    "#2962ff");
        let accent_dim = s.color_or_owned("accent_dim","rgba(41,98,255,0.15)");
        Self {
            bg:               s.color_or_owned("surface",       "#1e222d"),
            border:           s.color_or_owned("border_strong", "#363a45"),
            item_bg_hover:    s.color_or_owned("surface_raised","#2a2e39"),
            item_bg_selected: accent_dim,
            item_text:        s.color_or_owned("fg_1",          "#d1d4dc"),
            accent,
            fallback:         DefaultDropdownTheme,
        }
    }
}

impl DropdownTheme for StyledDropdownTheme {
    fn bg(&self)                    -> &str { &self.bg }
    fn border(&self)                -> &str { &self.border }
    fn shadow(&self)                -> &str { self.fallback.shadow() }
    fn item_bg_normal(&self)        -> &str { &self.bg }
    fn item_bg_hover(&self)         -> &str { &self.item_bg_hover }
    fn item_bg_selected(&self)      -> &str { &self.item_bg_selected }
    fn item_bg_danger_hover(&self)  -> &str { self.fallback.item_bg_danger_hover() }
    fn item_text(&self)             -> &str { &self.item_text }
    fn item_text_hover(&self)       -> &str { self.fallback.item_text_hover() }
    fn item_text_disabled(&self)    -> &str { self.fallback.item_text_disabled() }
    fn item_text_danger(&self)      -> &str { self.fallback.item_text_danger() }
    fn header_text(&self)           -> &str { self.fallback.header_text() }
    fn header_border(&self)         -> &str { &self.border }
    fn separator(&self)             -> &str { &self.border }
    fn shortcut_text(&self)         -> &str { self.fallback.shortcut_text() }
    fn caret_color(&self)           -> &str { self.fallback.caret_color() }
    fn toggle_on(&self)             -> &str { &self.accent }
    fn toggle_off(&self)            -> &str { self.fallback.toggle_off() }
    fn toggle_thumb(&self)          -> &str { self.fallback.toggle_thumb() }
    fn trigger_bg(&self)            -> &str { &self.bg }
    fn trigger_bg_hover(&self)      -> &str { &self.item_bg_hover }
    fn trigger_border(&self)        -> &str { &self.border }
    fn trigger_text(&self)          -> &str { &self.item_text }
    fn trigger_arrow(&self)         -> &str { self.fallback.trigger_arrow() }
    fn checkbox_border(&self)       -> &str { self.fallback.checkbox_border() }
    fn checkbox_checked(&self)      -> &str { &self.accent }
    fn cell_bg_hover(&self)         -> &str { &self.item_bg_hover }
    fn cell_border(&self)           -> &str { &self.border }
}

fn dropdown_settings_from_styles(s: &StyleManager) -> DropdownSettings {
    DropdownSettings {
        theme: Box::new(StyledDropdownTheme::from_styles(s)),
        style: Box::<DefaultDropdownStyle>::default(),
    }
}

/// Chainable builder for a dropdown overlay.
pub struct DropdownBuilder<'a> {
    handle:           &'a DropdownHandle,
    parent:           LayoutNodeId,
    slot_id:          Option<&'a str>,
    overlay_rect:     Option<Rect>,
    anchor:           Option<Rect>,
    /// Pending widget-id lookup — resolved at `.build()` via coord.
    anchor_widget_id: Option<&'a str>,
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
            anchor_widget_id:  None,
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

    /// Auto-anchor to a registered widget by id — at `.build()` time the
    /// builder looks up the widget's rect via the input coordinator and
    /// uses it as the anchor.  Equivalent to calling `.anchor(rect)` with
    /// the rect resolved from `coord.widget_rect(id)`.
    pub fn anchor_to(mut self, widget_id: &'a str) -> Self {
        self.anchor_widget_id = Some(widget_id);
        self
    }

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

        // Resolve anchor: explicit `.anchor(...)` wins; otherwise look up
        // the widget rect via coord using `.anchor_to(id)`.
        let resolved_anchor: Option<Rect> = self.anchor.or_else(|| {
            self.anchor_widget_id.and_then(|wid| {
                layout.ctx().input.widget_rect(&crate::types::unsafe_widget_id(wid))
            })
        });

        let overlay_rect = self.overlay_rect.unwrap_or_else(|| {
            let (x, y) = self.position_override
                .or_else(|| resolved_anchor.map(|a| (a.x, a.y + a.height)))
                .unwrap_or((0.0, 0.0));
            let (w, h) = match self.size_mode {
                SizeMode::Fixed(w, h) => (w, h),
                _                     => (200.0, 240.0),
            };
            Rect::new(x, y, w, h)
        });

        let mut view = DropdownView {
            anchor:            resolved_anchor,
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

        let settings = self.settings.unwrap_or_else(|| dropdown_settings_from_styles(layout.styles()));

        register_layout_manager_dropdown(
            layout,
            render,
            self.parent,
            &slot_id,
            self.handle,
            overlay_rect,
            resolved_anchor,
            &mut view,
            &settings,
            self.kind,
        )
    }
}
