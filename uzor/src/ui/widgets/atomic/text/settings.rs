//! Text widget settings — bundles theme + style.

use super::style::{DefaultTextStyle, TextStyle};
use super::theme::TextTheme;
use crate::input::text::{resolve_sense, SelectOverride, TextPolicy};
use crate::input::Sense;
use crate::tokens::{BuiltinSet, Tokens, TokenTheme};

/// Bundles theme and style for a Text widget instance.
pub struct TextSettings {
    pub theme: Box<dyn TextTheme>,
    pub style: Box<dyn TextStyle>,
    /// This label's say on pointer selection. Default
    /// [`SelectOverride::Inherit`] — follow the app's [`TextPolicy`], whose
    /// default is "not selectable". [`SelectOverride::Off`] also suppresses
    /// the selection highlight in
    /// [`draw_text_with_selection`](super::render::draw_text_with_selection).
    pub select: SelectOverride,
}

impl Default for TextSettings {
    fn default() -> Self {
        Self {
            theme: Box::new(TokenTheme::new(Tokens::builtin(BuiltinSet::Dark))),
            style: Box::new(DefaultTextStyle),
            select: SelectOverride::Inherit,
        }
    }
}

impl TextSettings {
    /// Override the theme.
    pub fn with_theme(mut self, theme: Box<dyn TextTheme>) -> Self {
        self.theme = theme;
        self
    }

    /// Override the style.
    pub fn with_style(mut self, style: Box<dyn TextStyle>) -> Self {
        self.style = style;
        self
    }

    /// Override the selection behaviour.
    pub fn with_select(mut self, select: SelectOverride) -> Self {
        self.select = select;
        self
    }

    /// Whether this label is selectable under `policy` (see
    /// [`crate::input::text::is_selectable`]).
    pub fn is_selectable(&self, policy: TextPolicy) -> bool {
        self.sense(policy).select
    }

    /// The sense this label registers under `policy`: `Sense::HOVER`, plus
    /// `select` when it resolves selectable. With the default policy and
    /// `Inherit` this is exactly `Sense::HOVER`, what the registration
    /// helpers use today.
    pub fn sense(&self, policy: TextPolicy) -> Sense {
        resolve_sense(Sense::HOVER, self.select, policy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_label_is_not_selectable_and_registers_plain_hover() {
        let s = TextSettings::default();
        assert_eq!(s.select, SelectOverride::Inherit);
        assert_eq!(s.sense(TextPolicy::default()), Sense::HOVER);
        assert!(!s.is_selectable(TextPolicy::default()));
        assert!(s.is_selectable(TextPolicy::SELECTABLE));
    }

    #[test]
    fn override_wins_over_the_policy() {
        let off = TextSettings::default().with_select(SelectOverride::Off);
        assert!(!off.is_selectable(TextPolicy::SELECTABLE));
        let on = TextSettings::default().with_select(SelectOverride::On);
        assert!(on.is_selectable(TextPolicy::NOT_SELECTABLE));
        assert!(on.sense(TextPolicy::NOT_SELECTABLE).select);
    }
}
