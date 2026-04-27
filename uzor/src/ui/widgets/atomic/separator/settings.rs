use super::style::{
    DefaultSeparatorStyle, ModalSectionDividerStyle, SeparatorStyle, SidebarSeparatorStyle,
    SplitPanelSeparatorStyle, SubPaneSeparatorStyle,
};
use super::theme::{DefaultSeparatorTheme, SeparatorTheme};

/// Bundle of theme + style for a separator widget.
///
/// Use `SeparatorSettings::default()` for the generic 1 px resize handle.
/// Use the variant constructors for mlc-specific presets.
pub struct SeparatorSettings {
    pub theme: Box<dyn SeparatorTheme>,
    pub style: Box<dyn SeparatorStyle>,
}

impl Default for SeparatorSettings {
    fn default() -> Self {
        Self {
            theme: Box::<DefaultSeparatorTheme>::default(),
            style: Box::new(DefaultSeparatorStyle),
        }
    }
}

impl SeparatorSettings {
    /// Sub-pane separator (1 px visual / ±6 px hit / 80 px drag min).
    pub fn sub_pane() -> Self {
        Self {
            theme: Box::<DefaultSeparatorTheme>::default(),
            style: Box::new(SubPaneSeparatorStyle),
        }
    }

    /// Split-panel separator (2 px idle / 4 px hover-drag / 8 px hit zone).
    ///
    /// `active` = hovered or dragging.
    pub fn split_panel(active: bool) -> Self {
        Self {
            theme: Box::<DefaultSeparatorTheme>::default(),
            style: Box::new(SplitPanelSeparatorStyle { active }),
        }
    }

    /// Sidebar separator (1 px visual / 8 px hit zone / min width 280 px).
    pub fn sidebar() -> Self {
        Self {
            theme: Box::<DefaultSeparatorTheme>::default(),
            style: Box::new(SidebarSeparatorStyle),
        }
    }

    /// Modal section divider (1 px stroke, non-interactive).
    pub fn modal_divider() -> Self {
        Self {
            theme: Box::<DefaultSeparatorTheme>::default(),
            style: Box::new(ModalSectionDividerStyle),
        }
    }
}
