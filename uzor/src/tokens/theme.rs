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
use crate::ui::widgets::atomic::close_button::theme::CloseButtonTheme;
use crate::ui::widgets::atomic::color_swatch::theme::ColorSwatchTheme;
use crate::ui::widgets::atomic::drag_handle::theme::DragHandleTheme;
use crate::ui::widgets::atomic::dropdown_trigger::theme::DropdownTriggerTheme;
use crate::ui::widgets::atomic::item::theme::ItemTheme;
use crate::ui::widgets::atomic::radio::theme::RadioTheme;
use crate::ui::widgets::atomic::scrollbar::theme::ScrollbarTheme;
use crate::ui::widgets::atomic::container::theme::ContainerTheme;
use crate::ui::widgets::atomic::scroll_chevron::theme::ScrollChevronTheme;
use crate::ui::widgets::atomic::separator::theme::SeparatorTheme;
use crate::ui::widgets::atomic::text::theme::TextTheme;
use crate::ui::widgets::atomic::tab::theme::TabTheme;
use crate::ui::widgets::atomic::slider::theme::SliderTheme;

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

impl CloseButtonTheme for TokenTheme {
    fn close_button_x_color(&self) -> &str { &self.0.close_button().close_button_x_color }
    fn close_button_x_color_hover(&self) -> &str { &self.0.close_button().close_button_x_color_hover }
    fn close_button_bg_hover(&self) -> &str { &self.0.close_button().close_button_bg_hover }
}

impl ColorSwatchTheme for TokenTheme {
    fn color_swatch_border(&self) -> &str { &self.0.color_swatch().color_swatch_border }
    fn color_swatch_hover_outline(&self) -> &str { &self.0.color_swatch().color_swatch_hover_outline }
    fn color_swatch_selected_border(&self) -> &str { &self.0.color_swatch().color_swatch_selected_border }
    fn transparency_checker_a(&self) -> &str { &self.0.color_swatch().transparency_checker_a }
    fn transparency_checker_b(&self) -> &str { &self.0.color_swatch().transparency_checker_b }
    fn fill_toggle_active_border(&self) -> &str { &self.0.color_swatch().fill_toggle_active_border }
    fn fill_toggle_off_pattern_color(&self) -> &str { &self.0.color_swatch().fill_toggle_off_pattern_color }
    fn fill_toggle_background(&self) -> &str { &self.0.color_swatch().fill_toggle_background }
    fn fill_toggle_disabled_overlay(&self) -> &str { &self.0.color_swatch().fill_toggle_disabled_overlay }
}

impl DragHandleTheme for TokenTheme {
    fn grip_dots_color(&self) -> &str { &self.0.drag_handle().grip_dots_color }
}

impl DropdownTriggerTheme for TokenTheme {
    fn dropdown_field_bg(&self) -> &str { &self.0.dropdown_trigger().dropdown_field_bg }
    fn dropdown_field_bg_hover(&self) -> &str { &self.0.dropdown_trigger().dropdown_field_bg_hover }
    fn dropdown_field_border(&self) -> &str { &self.0.dropdown_trigger().dropdown_field_border }
    fn dropdown_field_text(&self) -> &str { &self.0.dropdown_trigger().dropdown_field_text }
    fn dropdown_chevron_color(&self) -> &str { &self.0.dropdown_trigger().dropdown_chevron_color }
}

impl ItemTheme for TokenTheme {
    fn item_text(&self) -> &str { &self.0.item().item_text }
}

impl RadioTheme for TokenTheme {
    fn radio_outer_border(&self) -> &str { &self.0.radio().radio_outer_border }
    fn radio_outer_border_selected(&self) -> &str { &self.0.radio().radio_outer_border_selected }
    fn radio_inner_dot(&self) -> &str { &self.0.radio().radio_inner_dot }
    fn radio_disabled_overlay(&self) -> &str { &self.0.radio().radio_disabled_overlay }
    fn radio_row_bg_hover(&self) -> &str { &self.0.radio().radio_row_bg_hover }
    fn radio_label_text(&self) -> &str { &self.0.radio().radio_label_text }
    fn radio_label_text_selected(&self) -> &str { &self.0.radio().radio_label_text_selected }
    fn radio_description_text(&self) -> &str { &self.0.radio().radio_description_text }
}

impl ScrollbarTheme for TokenTheme {
    fn thumb_normal(&self) -> &str { &self.0.scrollbar().thumb_normal }
    fn thumb_hover(&self) -> &str { &self.0.scrollbar().thumb_hover }
    fn thumb_active(&self) -> &str { &self.0.scrollbar().thumb_active }
    fn track_bg(&self) -> &str { &self.0.scrollbar().track_bg }
}

impl ContainerTheme for TokenTheme {
    fn bg(&self) -> &str { &self.0.container().bg }
    fn border(&self) -> &str { &self.0.container().border }
    fn shadow(&self) -> &str { &self.0.container().shadow }
    fn card_shadow_color(&self) -> &str { &self.0.container().card_shadow_color }
    fn section_header_bg(&self) -> &str { &self.0.container().section_header_bg }
    fn section_header_text(&self) -> &str { &self.0.container().section_header_text }
    fn panel_bg(&self) -> &str { &self.0.container().panel_bg }
    fn panel_border(&self) -> &str { &self.0.container().panel_border }
}

impl ScrollChevronTheme for TokenTheme {
    fn scroll_chevron_color(&self) -> &str { &self.0.scroll_chevron().scroll_chevron_color }
    fn scroll_chevron_color_hover(&self) -> &str { &self.0.scroll_chevron().scroll_chevron_color_hover }
    fn scroll_chevron_color_disabled(&self) -> &str { &self.0.scroll_chevron().scroll_chevron_color_disabled }
    fn scroll_chevron_bg_hover(&self) -> &str { &self.0.scroll_chevron().scroll_chevron_bg_hover }
}

impl SeparatorTheme for TokenTheme {
    fn line(&self) -> &str { &self.0.separator().line }
    fn handle_hover(&self) -> &str { &self.0.separator().handle_hover }
    fn handle_active(&self) -> &str { &self.0.separator().handle_active }
    fn pane_handle_idle(&self) -> &str { &self.0.separator().pane_handle_idle }
    fn pane_handle_hover(&self) -> &str { &self.0.separator().pane_handle_hover }
    fn sidebar_separator(&self) -> &str { &self.0.separator().sidebar_separator }
    fn modal_divider(&self) -> &str { &self.0.separator().modal_divider }
}

impl TextTheme for TokenTheme {
    fn text_color(&self) -> &str { &self.0.text().text_color }
    fn text_color_hover(&self) -> &str { &self.0.text().text_color_hover }
}

// `chrome_bottom_accent`, `chrome_hover_line`, `sidebar_left_accent`,
// `sidebar_bg_active`, and `tags_pill_bg_active` are NOT overridden here —
// each is a pure delegator whose default body resolves to the exact same
// value as its target once the fields below are aliased (see
// `ui::widgets::atomic::tab::tokens`'s module doc for the per-method proof),
// so `TabTheme`'s own default body runs unmodified on `TokenTheme` too.
impl TabTheme for TokenTheme {
    fn bg_normal(&self) -> &str { &self.0.tab().bg_normal }
    fn bg_hover(&self) -> &str { &self.0.tab().bg_hover }
    fn bg_active(&self) -> &str { &self.0.tab().bg_active }
    fn text_normal(&self) -> &str { &self.0.tab().text_normal }
    fn text_active(&self) -> &str { &self.0.tab().text_active }
    fn accent(&self) -> &str { &self.0.tab().accent }
    fn close_normal(&self) -> &str { &self.0.tab().close_normal }
    fn close_hover(&self) -> &str { &self.0.tab().close_hover }
    fn tags_pill_bg_hover(&self) -> &str { &self.0.tab().tags_pill_bg_hover }
}

impl SliderTheme for TokenTheme {
    fn track_empty(&self) -> &str { &self.0.slider().track_empty }
    fn accent(&self) -> &str { &self.0.slider().accent }
    fn text_normal(&self) -> &str { &self.0.slider().text_normal }
    fn text_disabled(&self) -> &str { &self.0.slider().text_disabled }
    fn input_bg(&self) -> &str { &self.0.slider().input_bg }
    fn input_border_normal(&self) -> &str { &self.0.slider().input_border_normal }
    fn input_border_focused(&self) -> &str { &self.0.slider().input_border_focused }
    fn toolbar_track_empty(&self) -> &str { &self.0.slider().toolbar_track_empty }
    fn toolbar_track_filled(&self) -> &str { &self.0.slider().toolbar_track_filled }
    fn toolbar_handle(&self) -> &str { &self.0.slider().toolbar_handle }
    fn input_bg_disabled(&self) -> &str { &self.0.slider().input_bg_disabled }
    fn input_placeholder(&self) -> &str { &self.0.slider().input_placeholder }
    fn input_selection(&self) -> &str { &self.0.slider().input_selection }
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
        // `ButtonTheme::transparency_checker_a` and
        // `ColorSwatchTheme::transparency_checker_a` are two distinct traits
        // both implemented on `TokenTheme` — disambiguate explicitly.
        assert_eq!(ButtonTheme::transparency_checker_a(&theme), "#ffffff");
    }

    #[test]
    fn clone_shares_the_same_arc() {
        let theme = TokenTheme::new(Tokens::builtin(BuiltinSet::Dark));
        let cloned = theme.clone();
        assert!(Arc::ptr_eq(&theme.0, &cloned.0));
    }

    /// Proves the tab-token design decision (`tab/tokens.rs`'s module doc):
    /// the 5 default-body methods `TokenTheme` does NOT override still
    /// resolve correctly by falling through to `TabTheme`'s own default
    /// body, landing on the exact same value as their delegation target.
    #[test]
    fn tab_theme_pure_delegators_resolve_through_their_target() {
        let theme = TokenTheme::new(Tokens::builtin(BuiltinSet::Dark));
        assert_eq!(TabTheme::chrome_bottom_accent(&theme), TabTheme::accent(&theme));
        assert_eq!(TabTheme::chrome_hover_line(&theme), TabTheme::bg_hover(&theme));
        assert_eq!(TabTheme::sidebar_left_accent(&theme), TabTheme::accent(&theme));
        assert_eq!(TabTheme::sidebar_bg_active(&theme), TabTheme::bg_active(&theme));
        assert_eq!(TabTheme::tags_pill_bg_active(&theme), TabTheme::accent(&theme));
        // `tags_pill_bg_hover` is the one default-body method that does NOT
        // delegate on `TokenTheme` — its own alias (`text.secondary`)
        // diverges from `text_normal`'s (`text.muted`).
        assert_ne!(TabTheme::tags_pill_bg_hover(&theme), TabTheme::text_normal(&theme));
    }
}
