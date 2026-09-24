//! `TokenTheme`: the one type that implements every widget colour-theme
//! trait converted to the token contract, by reading its resolved,
//! pre-rendered [`Tokens`] fields (H1 token contract design §3). Cloning is
//! an `Arc` bump — O(1), no allocation, no per-paint recomputation.
//!
//! `ButtonTheme` is the only trait implemented as of H1 Brief 4 (the worked
//! example the remaining 29 widgets copy); each later brief adds one more
//! `impl <Widget>Theme for TokenTheme` block here, same one-line-per-method
//! shape.

use std::sync::Arc;

use crate::tokens::set::Tokens;
use crate::ui::widgets::atomic::button::theme::ButtonTheme;
use crate::ui::widgets::atomic::checkbox::theme::CheckboxTheme;
use crate::ui::widgets::atomic::chevron::theme::ChevronTheme;
use crate::ui::widgets::atomic::clock::theme::ClockTheme;

/// Holds a resolved, shared token set and implements every converted
/// widget's colour-theme trait by borrowing its matching pre-rendered
/// [`Tokens`] field — never a per-call string lookup.
#[derive(Clone)]
pub struct TokenTheme(Arc<Tokens>);

impl TokenTheme {
    /// Wraps an already-resolved token set (e.g. [`Tokens::builtin`]).
    pub fn new(tokens: Arc<Tokens>) -> Self {
        Self(tokens)
    }

    /// The underlying resolved token set.
    pub fn tokens(&self) -> &Tokens {
        &self.0
    }
}

impl ButtonTheme for TokenTheme {
    fn button_bg_normal(&self) -> &str { &self.0.button().button_bg_normal }
    fn button_bg_hover(&self) -> &str { &self.0.button().button_bg_hover }
    fn button_bg_pressed(&self) -> &str { &self.0.button().button_bg_pressed }
    fn button_bg_active(&self) -> &str { &self.0.button().button_bg_active }
    fn button_bg_disabled(&self) -> &str { &self.0.button().button_bg_disabled }

    fn button_text_normal(&self) -> &str { &self.0.button().button_text_normal }
    fn button_text_hover(&self) -> &str { &self.0.button().button_text_hover }
    fn button_text_active(&self) -> &str { &self.0.button().button_text_active }
    fn button_text_disabled(&self) -> &str { &self.0.button().button_text_disabled }

    fn button_icon_normal(&self) -> &str { &self.0.button().button_icon_normal }
    fn button_icon_hover(&self) -> &str { &self.0.button().button_icon_hover }
    fn button_icon_active(&self) -> &str { &self.0.button().button_icon_active }
    fn button_icon_disabled(&self) -> &str { &self.0.button().button_icon_disabled }

    fn button_border_normal(&self) -> &str { &self.0.button().button_border_normal }
    fn button_border_hover(&self) -> &str { &self.0.button().button_border_hover }
    fn button_border_focused(&self) -> &str { &self.0.button().button_border_focused }

    fn button_accent(&self) -> &str { &self.0.button().button_accent }
    fn button_danger(&self) -> &str { &self.0.button().button_danger }
    fn button_success(&self) -> &str { &self.0.button().button_success }
    fn button_warning(&self) -> &str { &self.0.button().button_warning }

    fn toolbar_item_bg_hover(&self) -> &str { &self.0.button().toolbar_item_bg_hover }
    fn toolbar_item_bg_active(&self) -> &str { &self.0.button().toolbar_item_bg_active }
    fn toolbar_item_text(&self) -> &str { &self.0.button().toolbar_item_text }
    fn toolbar_item_text_hover(&self) -> &str { &self.0.button().toolbar_item_text_hover }
    fn toolbar_item_text_active(&self) -> &str { &self.0.button().toolbar_item_text_active }
    fn toolbar_separator(&self) -> &str { &self.0.button().toolbar_separator }
    fn toolbar_background(&self) -> &str { &self.0.button().toolbar_background }
    fn toolbar_accent(&self) -> &str { &self.0.button().toolbar_accent }

    fn button_primary_bg(&self) -> &str { &self.0.button().button_primary_bg }
    fn button_primary_bg_hover(&self) -> &str { &self.0.button().button_primary_bg_hover }
    fn button_danger_bg(&self) -> &str { &self.0.button().button_danger_bg }
    fn button_danger_bg_hover(&self) -> &str { &self.0.button().button_danger_bg_hover }
    fn button_danger_border(&self) -> &str { &self.0.button().button_danger_border }
    fn button_danger_border_hover(&self) -> &str { &self.0.button().button_danger_border_hover }
    fn button_danger_text(&self) -> &str { &self.0.button().button_danger_text }
    fn button_secondary_hover_bg(&self) -> &str { &self.0.button().button_secondary_hover_bg }
    fn button_secondary_text_muted(&self) -> &str { &self.0.button().button_secondary_text_muted }
    fn button_secondary_text(&self) -> &str { &self.0.button().button_secondary_text }
    fn button_ghost_idle_bg(&self) -> &str { &self.0.button().button_ghost_idle_bg }
    fn button_utility_bg(&self) -> &str { &self.0.button().button_utility_bg }
    fn button_utility_bg_hover(&self) -> &str { &self.0.button().button_utility_bg_hover }

    fn transparency_checker_a(&self) -> &str { &self.0.button().transparency_checker_a }
    fn transparency_checker_b(&self) -> &str { &self.0.button().transparency_checker_b }
    fn dropdown_menu_row_bg_normal(&self) -> &str { &self.0.button().dropdown_menu_row_bg_normal }
}

impl CheckboxTheme for TokenTheme {
    fn checkbox_bg_checked(&self) -> &str { &self.0.checkbox().checkbox_bg_checked }
    fn checkbox_bg_unchecked(&self) -> &str { &self.0.checkbox().checkbox_bg_unchecked }
    fn checkbox_border(&self) -> &str { &self.0.checkbox().checkbox_border }
    fn checkbox_checkmark(&self) -> &str { &self.0.checkbox().checkbox_checkmark }
    fn checkbox_notification_inner(&self) -> &str { &self.0.checkbox().checkbox_notification_inner }
    fn checkbox_label_text(&self) -> &str { &self.0.checkbox().checkbox_label_text }
}

impl ChevronTheme for TokenTheme {
    fn color(&self) -> &str { &self.0.chevron().color }
    fn color_hover(&self) -> &str { &self.0.chevron().color_hover }
    fn color_pressed(&self) -> &str { &self.0.chevron().color_pressed }
    fn color_disabled(&self) -> &str { &self.0.chevron().color_disabled }
    fn color_active(&self) -> &str { &self.0.chevron().color_active }
    fn bg_hover(&self) -> &str { &self.0.chevron().bg_hover }
}

impl ClockTheme for TokenTheme {
    fn clock_text(&self) -> &str { &self.0.clock().clock_text }
    fn clock_bg_hover(&self) -> &str { &self.0.clock().clock_bg_hover }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::BuiltinSet;

    #[test]
    fn token_theme_reads_through_to_resolved_button_tokens() {
        let tokens = Tokens::builtin(BuiltinSet::Dark);
        let theme = TokenTheme::new(Arc::clone(&tokens));
        assert_eq!(theme.button_bg_hover(), tokens.button().button_bg_hover);
        assert_eq!(theme.button_accent(), "#2962ff");
        assert_eq!(theme.transparency_checker_a(), "#ffffff");
    }

    #[test]
    fn clone_shares_the_same_arc() {
        let theme = TokenTheme::new(Tokens::builtin(BuiltinSet::Dark));
        let cloned = theme.clone();
        assert!(Arc::ptr_eq(&theme.0, &cloned.0));
    }
}
