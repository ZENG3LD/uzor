//! Container variant catalog. Layout (rect) is layout-layer concern.
//!
//! Note: a *composite* scrollable container (Container + Scrollbar) is
//! a separate widget that lives in `composite/scroll_container/`. This
//! atomic container only manages the bg/border/clip; scrollbar
//! composition happens at the composite level.

use crate::input::Sense;
use crate::ui::widgets::WidgetCapabilities;

/// Sub-types for `ContainerType::Panel` — maps to mlc toolbar/sidebar/status roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelRole {
    /// Full-width horizontal toolbar (mlc `ToolbarTheme::background`).
    Toolbar,
    /// Vertical side panel or settings sidebar.
    Sidebar,
    /// Slim horizontal status/footer bar.
    StatusBar,
}

/// Container variant — determines which render function is invoked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerType {
    /// Plain — fill_rect bg, no border, no radius.
    ///
    /// Maps to all 9 mlc trading panel outermost fills (e.g. dom.rs, footprint.rs).
    Plain,

    /// Bordered — fill_rounded_rect bg + 1px stroke border.
    ///
    /// Maps to mlc dropdown menus (`indicator_settings.rs`, `chart_settings.rs`,
    /// `alert_settings.rs`) with radius=4.0, separator color border.
    Bordered,

    /// Card — bg + drop shadow (no blur) + rounded corners.
    ///
    /// NOTE: In mlc, blur+shadow is exclusive to `Popup` and `ModalFrame` composite
    /// widgets. This atomic Card variant implements shadow-only (no blur) for
    /// completeness. Callers needing blur should use the composite popup widget.
    Card,

    /// Clipping — bg + ctx.save / clip_rect / restore.
    ///
    /// Maps to mlc `ScrollableContainer::begin()`/`end()` pattern. Callers draw
    /// children between `draw_clipping_container` and `end_clipping_container`.
    Clip,

    /// Section — header strip + body bg + border.
    ///
    /// Maps to mlc panels that draw a `header_bg` strip on top of `panel_bg`
    /// (dom.rs, order_entry.rs, position_manager.rs, trade_log.rs, etc.).
    Section,

    /// Panel — toolbar/sidebar/status-bar style container.
    ///
    /// Uses `PanelTheme` bridge (mlc `panels_render.rs` pattern).
    Panel(PanelRole),
}

impl WidgetCapabilities for ContainerType {
    fn sense(&self) -> Sense {
        Sense::NONE
    }
}
