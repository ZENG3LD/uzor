//! Scrollbar settings bundle — style + theme in one place.

use super::style::{
    CompactScrollbarStyle, ScrollbarStyle, SignalScrollbarStyle, StandardScrollbarStyle,
};
use super::theme::{DefaultScrollbarTheme, LightScrollbarTheme, ScrollbarTheme};

/// Combined scrollbar configuration passed to `draw_scrollbar`.
pub struct ScrollbarSettings {
    pub theme: Box<dyn ScrollbarTheme>,
    pub style: Box<dyn ScrollbarStyle>,
}

impl Default for ScrollbarSettings {
    fn default() -> Self {
        Self::standard()
    }
}

impl ScrollbarSettings {
    /// Standard scrollbar (sidebar, indicator-settings, scrollable-container).
    /// 8 px / 30 px min / r=4 / dark theme.
    pub fn standard() -> Self {
        Self {
            theme: Box::<DefaultScrollbarTheme>::default(),
            style: Box::new(StandardScrollbarStyle),
        }
    }

    /// Compact scrollbar (profile-manager).
    /// 4 px / 24 px min / r=2 / always opaque / dark theme.
    pub fn compact() -> Self {
        Self {
            theme: Box::<DefaultScrollbarTheme>::default(),
            style: Box::new(CompactScrollbarStyle),
        }
    }

    /// Signal-group scrollbar.
    /// 6 px / 16 px min / r=0 / draws track bg / dark theme.
    pub fn signal() -> Self {
        Self {
            theme: Box::<DefaultScrollbarTheme>::default(),
            style: Box::new(SignalScrollbarStyle),
        }
    }

    /// Standard scrollbar with light theme.
    pub fn standard_light() -> Self {
        Self {
            theme: Box::<LightScrollbarTheme>::default(),
            style: Box::new(StandardScrollbarStyle),
        }
    }
}
