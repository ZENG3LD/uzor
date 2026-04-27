//! Tab settings bundle — theme + all variant style presets.

use super::style::{
    ChromeTabStyle, DefaultTabStyle, ModalHorizontalTabStyle, ModalSidebarTabStyle,
    TagsTabsSidebarTabStyle, TabStyle,
};
use super::theme::{DefaultTabTheme, TabTheme};

/// All configuration needed to render any tab variant.
///
/// - `theme`           — color palette (shared across all variants).
/// - `style`           — generic `TabStyle` for use with `draw_tab` (generic dispatcher).
/// - `chrome`          — preset for `draw_chrome_tab`.
/// - `modal_sidebar`   — preset for `draw_modal_sidebar_tab`.
/// - `modal_horizontal`— preset for `draw_modal_horizontal_tab`.
/// - `tags_sidebar`    — preset for `draw_tags_tabs_sidebar_tab`.
pub struct TabSettings {
    pub theme: Box<dyn TabTheme>,
    pub style: Box<dyn TabStyle>,

    // Per-variant presets (boxed so callers can supply custom impls).
    pub chrome: ChromeTabStyle,
    pub modal_sidebar: ModalSidebarTabStyle,
    pub modal_horizontal: ModalHorizontalTabStyle,
    pub tags_sidebar: TagsTabsSidebarTabStyle,
}

impl Default for TabSettings {
    fn default() -> Self {
        Self {
            theme: Box::<DefaultTabTheme>::default(),
            style: Box::new(DefaultTabStyle),
            chrome: ChromeTabStyle::default(),
            modal_sidebar: ModalSidebarTabStyle::default(),
            modal_horizontal: ModalHorizontalTabStyle::default(),
            tags_sidebar: TagsTabsSidebarTabStyle::default(),
        }
    }
}
