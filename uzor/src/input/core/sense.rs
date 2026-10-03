//! Sense flags for widget interaction detection
//!
//! The [`Sense`] type determines what kinds of user interactions a widget
//! will detect and respond to.

/// What interactions a widget is sensitive to
///
/// Using `CLICK_AND_DRAG` introduces latency to distinguish click vs drag intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Sense {
    /// Widget responds to clicks
    pub click: bool,
    /// Widget responds to drags
    pub drag: bool,
    /// Widget tracks hover state
    pub hover: bool,
    /// Widget can receive keyboard focus
    pub focus: bool,
    /// Widget responds to scroll wheel / touchpad scroll
    pub scroll: bool,
    /// Widget accepts text input (text field)
    pub text: bool,
    /// Widget responds to right-clicks (context menus)
    pub right_click: bool,
    /// Widget responds to double-clicks
    pub double_click: bool,
    /// Widget receives non-text keyboard events (arrows, escape, shortcuts) when focused
    pub keyboard: bool,
    /// Widget takes IME composition (preedit / commit) while focused
    pub ime: bool,
    /// Widget is a drop target for file / drag-and-drop payloads
    pub drop: bool,
    /// Widget responds to multi-finger gestures (pinch, rotate, two-finger pan)
    pub gesture: bool,
    /// Widget responds to raw touch contacts
    pub touch: bool,
    /// Widget's (non-editable) text can be selected with the pointer
    /// (drag, double / triple click) and copied.
    ///
    /// Off in every named sense except [`Sense::ALL`]: plain text is not
    /// selectable unless the widget opts in, or the app-wide
    /// [`TextPolicy`](crate::input::text::TextPolicy) turns it on — see
    /// [`is_selectable`](crate::input::text::is_selectable). Text inputs
    /// (`text`) own their selection in the text store and do not need it.
    pub select: bool,
}

// Predefined constants
impl Sense {
    /// No interactions at all
    pub const NONE: Sense = Sense {
        click: false,
        drag: false,
        hover: false,
        focus: false,
        scroll: false,
        text: false,
        right_click: false,
        double_click: false,
        keyboard: false,
        ime: false,
        drop: false,
        gesture: false,
        touch: false,
        select: false,
    };

    /// Only hover detection
    pub const HOVER: Sense = Sense {
        click: false,
        drag: false,
        hover: true,
        focus: false,
        scroll: false,
        text: false,
        right_click: false,
        double_click: false,
        keyboard: false,
        ime: false,
        drop: false,
        gesture: false,
        touch: false,
        select: false,
    };

    /// Click and hover (for buttons, checkboxes)
    pub const CLICK: Sense = Sense {
        click: true,
        drag: false,
        hover: true,
        focus: false,
        scroll: false,
        text: false,
        right_click: false,
        double_click: false,
        keyboard: false,
        ime: false,
        drop: false,
        gesture: false,
        touch: false,
        select: false,
    };

    /// Drag and hover (for sliders, scrollbars)
    pub const DRAG: Sense = Sense {
        click: false,
        drag: true,
        hover: true,
        focus: false,
        scroll: false,
        text: false,
        right_click: false,
        double_click: false,
        keyboard: false,
        ime: false,
        drop: false,
        gesture: false,
        touch: false,
        select: false,
    };

    /// Both click and drag (introduces latency)
    pub const CLICK_AND_DRAG: Sense = Sense {
        click: true,
        drag: true,
        hover: true,
        focus: false,
        scroll: false,
        text: false,
        right_click: false,
        double_click: false,
        keyboard: false,
        ime: false,
        drop: false,
        gesture: false,
        touch: false,
        select: false,
    };

    /// Can receive keyboard focus but no mouse interaction
    pub const FOCUSABLE: Sense = Sense {
        click: false,
        drag: false,
        hover: true,
        focus: true,
        scroll: false,
        text: false,
        right_click: false,
        double_click: false,
        keyboard: false,
        ime: false,
        drop: false,
        gesture: false,
        touch: false,
        select: false,
    };

    /// Scroll-sensitive (for scrollable container viewports)
    pub const SCROLL: Sense = Sense {
        click: false,
        drag: false,
        hover: true,
        focus: false,
        scroll: true,
        text: false,
        right_click: false,
        double_click: false,
        keyboard: false,
        ime: false,
        drop: false,
        gesture: false,
        touch: false,
        select: false,
    };

    /// Full interaction - click, drag, hover, focus, scroll, right_click,
    /// double_click, keyboard, ime, drop, gesture, touch, select (not text)
    pub const ALL: Sense = Sense {
        click: true,
        drag: true,
        hover: true,
        focus: true,
        scroll: true,
        text: false,
        right_click: true,
        double_click: true,
        keyboard: true,
        ime: true,
        drop: true,
        gesture: true,
        touch: true,
        select: true,
    };

    /// Text input — click, drag, hover, focus, and text
    pub const TEXT_INPUT: Sense = Sense {
        click: true,
        drag: true,
        hover: true,
        focus: true,
        scroll: false,
        text: true,
        right_click: false,
        double_click: false,
        keyboard: false,
        ime: false,
        drop: false,
        gesture: false,
        touch: false,
        select: false,
    };

    /// Right-click and hover (for context menus)
    pub const RIGHT_CLICK: Sense = Sense {
        click: false,
        drag: false,
        hover: true,
        focus: false,
        scroll: false,
        text: false,
        right_click: true,
        double_click: false,
        keyboard: false,
        ime: false,
        drop: false,
        gesture: false,
        touch: false,
        select: false,
    };

    /// Double-click and hover
    pub const DOUBLE_CLICK: Sense = Sense {
        click: false,
        drag: false,
        hover: true,
        focus: false,
        scroll: false,
        text: false,
        right_click: false,
        double_click: true,
        keyboard: false,
        ime: false,
        drop: false,
        gesture: false,
        touch: false,
        select: false,
    };

    /// Keyboard events when focused (arrows, escape, shortcuts), includes hover and focus
    pub const KEYBOARD: Sense = Sense {
        click: false,
        drag: false,
        hover: true,
        focus: true,
        scroll: false,
        text: false,
        right_click: false,
        double_click: false,
        keyboard: true,
        ime: false,
        drop: false,
        gesture: false,
        touch: false,
        select: false,
    };
}

// Constructor methods
impl Sense {
    /// Create empty sense (no interactions)
    #[inline]
    pub fn none() -> Self {
        Self::NONE
    }

    /// Create hover-only sense
    #[inline]
    pub fn hover() -> Self {
        Self::HOVER
    }

    /// Create click sense (includes hover)
    #[inline]
    pub fn click() -> Self {
        Self::CLICK
    }

    /// Create drag sense (includes hover)
    #[inline]
    pub fn drag() -> Self {
        Self::DRAG
    }

    /// Create click and drag sense (includes hover, introduces latency)
    #[inline]
    pub fn click_and_drag() -> Self {
        Self::CLICK_AND_DRAG
    }

    /// Create focusable sense (for keyboard navigation)
    #[inline]
    pub fn focusable() -> Self {
        Self::FOCUSABLE
    }

    /// Create scroll-sensitive sense (includes hover)
    #[inline]
    pub fn scroll() -> Self {
        Self::SCROLL
    }

    /// Create full interaction sense
    #[inline]
    pub fn all() -> Self {
        Self::ALL
    }

    /// Create text input sense (click, drag, hover, focus, text)
    #[inline]
    pub fn text_input() -> Self {
        Self::TEXT_INPUT
    }
}

// Combination methods
impl Sense {
    /// Union of two senses (OR)
    #[inline]
    pub fn union(self, other: Sense) -> Sense {
        Sense {
            click: self.click || other.click,
            drag: self.drag || other.drag,
            hover: self.hover || other.hover,
            focus: self.focus || other.focus,
            scroll: self.scroll || other.scroll,
            text: self.text || other.text,
            right_click: self.right_click || other.right_click,
            double_click: self.double_click || other.double_click,
            keyboard: self.keyboard || other.keyboard,
            ime: self.ime || other.ime,
            drop: self.drop || other.drop,
            gesture: self.gesture || other.gesture,
            touch: self.touch || other.touch,
            select: self.select || other.select,
        }
    }

    /// Intersection of two senses (AND)
    #[inline]
    pub fn intersection(self, other: Sense) -> Sense {
        Sense {
            click: self.click && other.click,
            drag: self.drag && other.drag,
            hover: self.hover && other.hover,
            focus: self.focus && other.focus,
            scroll: self.scroll && other.scroll,
            text: self.text && other.text,
            right_click: self.right_click && other.right_click,
            double_click: self.double_click && other.double_click,
            keyboard: self.keyboard && other.keyboard,
            ime: self.ime && other.ime,
            drop: self.drop && other.drop,
            gesture: self.gesture && other.gesture,
            touch: self.touch && other.touch,
            select: self.select && other.select,
        }
    }

    /// Add click sensing (also adds hover)
    #[inline]
    pub fn with_click(mut self) -> Self {
        self.click = true;
        self.hover = true;
        self
    }

    /// Add drag sensing (also adds hover)
    #[inline]
    pub fn with_drag(mut self) -> Self {
        self.drag = true;
        self.hover = true;
        self
    }

    /// Add focus capability
    #[inline]
    pub fn with_focus(mut self) -> Self {
        self.focus = true;
        self
    }

    /// Add scroll sensing (also adds hover)
    #[inline]
    pub fn with_scroll(mut self) -> Self {
        self.scroll = true;
        self.hover = true;
        self
    }

    /// Add text input capability (also adds focus and hover)
    #[inline]
    pub fn with_text(mut self) -> Self {
        self.text = true;
        self.focus = true;
        self.hover = true;
        self
    }

    /// Add right-click sensing (also adds hover)
    #[inline]
    pub fn with_right_click(mut self) -> Self {
        self.right_click = true;
        self.hover = true;
        self
    }

    /// Add double-click sensing (also adds hover)
    #[inline]
    pub fn with_double_click(mut self) -> Self {
        self.double_click = true;
        self.hover = true;
        self
    }

    /// Add keyboard event sensing (also adds focus and hover)
    #[inline]
    pub fn with_keyboard(mut self) -> Self {
        self.keyboard = true;
        self.focus = true;
        self.hover = true;
        self
    }
    /// Add IME composition sensing (also adds focus and hover — IME text
    /// only ever reaches the focused widget)
    #[inline]
    pub fn with_ime(mut self) -> Self {
        self.ime = true;
        self.focus = true;
        self.hover = true;
        self
    }

    /// Add drop-target sensing (also adds hover)
    #[inline]
    pub fn with_drop(mut self) -> Self {
        self.drop = true;
        self.hover = true;
        self
    }

    /// Add multi-finger gesture sensing (also adds hover)
    #[inline]
    pub fn with_gesture(mut self) -> Self {
        self.gesture = true;
        self.hover = true;
        self
    }

    /// Add raw touch sensing (also adds hover)
    #[inline]
    pub fn with_touch(mut self) -> Self {
        self.touch = true;
        self.hover = true;
        self
    }

    /// Opt this widget's text into pointer selection (also adds hover —
    /// the selection drag starts from a press over the widget).
    #[inline]
    pub fn with_select(mut self) -> Self {
        self.select = true;
        self.hover = true;
        self
    }
}

// Query methods
impl Sense {
    /// Check if any interaction is sensed (everything except bare hover)
    #[inline]
    pub fn interactive(&self) -> bool {
        self.click
            || self.drag
            || self.focus
            || self.scroll
            || self.text
            || self.right_click
            || self.double_click
            || self.keyboard
            || self.ime
            || self.drop
            || self.gesture
            || self.touch
            || self.select
    }

    /// Check if both click and drag are sensed (has latency)
    #[inline]
    pub fn has_click_and_drag(&self) -> bool {
        self.click && self.drag
    }

    /// Check if widget is purely visual (no interactions)
    #[inline]
    pub fn is_passive(&self) -> bool {
        !self.click
            && !self.drag
            && !self.focus
            && !self.scroll
            && !self.text
            && !self.right_click
            && !self.double_click
            && !self.keyboard
            && !self.ime
            && !self.drop
            && !self.gesture
            && !self.touch
            && !self.select
    }
}

impl std::ops::BitOr for Sense {
    type Output = Sense;

    /// Combine two senses using the `|` operator (equivalent to union)
    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

impl std::ops::BitOrAssign for Sense {
    /// Combine this sense with another using `|=`
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        *self = self.union(rhs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_predefined_constants() {
        assert!(!Sense::NONE.click);
        assert!(!Sense::NONE.drag);
        assert!(!Sense::NONE.hover);
        assert!(!Sense::NONE.focus);
        assert!(!Sense::NONE.scroll);
        assert!(!Sense::NONE.text);
        assert!(!Sense::NONE.right_click);
        assert!(!Sense::NONE.double_click);
        assert!(!Sense::NONE.keyboard);

        assert!(!Sense::HOVER.click);
        assert!(Sense::HOVER.hover);
        assert!(!Sense::HOVER.scroll);
        assert!(!Sense::HOVER.text);
        assert!(!Sense::HOVER.right_click);
        assert!(!Sense::HOVER.double_click);
        assert!(!Sense::HOVER.keyboard);

        assert!(Sense::CLICK.click);
        assert!(!Sense::CLICK.drag);
        assert!(Sense::CLICK.hover);
        assert!(!Sense::CLICK.scroll);
        assert!(!Sense::CLICK.text);
        assert!(!Sense::CLICK.right_click);
        assert!(!Sense::CLICK.double_click);
        assert!(!Sense::CLICK.keyboard);

        assert!(!Sense::DRAG.click);
        assert!(Sense::DRAG.drag);
        assert!(Sense::DRAG.hover);
        assert!(!Sense::DRAG.scroll);
        assert!(!Sense::DRAG.text);
        assert!(!Sense::DRAG.right_click);
        assert!(!Sense::DRAG.double_click);
        assert!(!Sense::DRAG.keyboard);

        assert!(Sense::CLICK_AND_DRAG.click);
        assert!(Sense::CLICK_AND_DRAG.drag);
        assert!(Sense::CLICK_AND_DRAG.hover);
        assert!(!Sense::CLICK_AND_DRAG.scroll);
        assert!(!Sense::CLICK_AND_DRAG.text);
        assert!(!Sense::CLICK_AND_DRAG.right_click);
        assert!(!Sense::CLICK_AND_DRAG.double_click);
        assert!(!Sense::CLICK_AND_DRAG.keyboard);

        assert!(!Sense::FOCUSABLE.click);
        assert!(Sense::FOCUSABLE.hover);
        assert!(Sense::FOCUSABLE.focus);
        assert!(!Sense::FOCUSABLE.scroll);
        assert!(!Sense::FOCUSABLE.text);
        assert!(!Sense::FOCUSABLE.right_click);
        assert!(!Sense::FOCUSABLE.double_click);
        assert!(!Sense::FOCUSABLE.keyboard);

        assert!(!Sense::SCROLL.click);
        assert!(!Sense::SCROLL.drag);
        assert!(Sense::SCROLL.hover);
        assert!(!Sense::SCROLL.focus);
        assert!(Sense::SCROLL.scroll);
        assert!(!Sense::SCROLL.text);
        assert!(!Sense::SCROLL.right_click);
        assert!(!Sense::SCROLL.double_click);
        assert!(!Sense::SCROLL.keyboard);

        assert!(Sense::ALL.click);
        assert!(Sense::ALL.drag);
        assert!(Sense::ALL.hover);
        assert!(Sense::ALL.focus);
        assert!(Sense::ALL.scroll);
        assert!(!Sense::ALL.text);
        assert!(Sense::ALL.right_click);
        assert!(Sense::ALL.double_click);
        assert!(Sense::ALL.keyboard);
    }

    #[test]
    fn test_new_constants() {
        // RIGHT_CLICK
        assert!(!Sense::RIGHT_CLICK.click);
        assert!(!Sense::RIGHT_CLICK.drag);
        assert!(Sense::RIGHT_CLICK.hover);
        assert!(!Sense::RIGHT_CLICK.focus);
        assert!(!Sense::RIGHT_CLICK.scroll);
        assert!(!Sense::RIGHT_CLICK.text);
        assert!(Sense::RIGHT_CLICK.right_click);
        assert!(!Sense::RIGHT_CLICK.double_click);
        assert!(!Sense::RIGHT_CLICK.keyboard);
        assert!(Sense::RIGHT_CLICK.interactive());
        assert!(!Sense::RIGHT_CLICK.is_passive());

        // DOUBLE_CLICK
        assert!(!Sense::DOUBLE_CLICK.click);
        assert!(!Sense::DOUBLE_CLICK.drag);
        assert!(Sense::DOUBLE_CLICK.hover);
        assert!(!Sense::DOUBLE_CLICK.focus);
        assert!(!Sense::DOUBLE_CLICK.scroll);
        assert!(!Sense::DOUBLE_CLICK.text);
        assert!(!Sense::DOUBLE_CLICK.right_click);
        assert!(Sense::DOUBLE_CLICK.double_click);
        assert!(!Sense::DOUBLE_CLICK.keyboard);
        assert!(Sense::DOUBLE_CLICK.interactive());
        assert!(!Sense::DOUBLE_CLICK.is_passive());

        // KEYBOARD
        assert!(!Sense::KEYBOARD.click);
        assert!(!Sense::KEYBOARD.drag);
        assert!(Sense::KEYBOARD.hover);
        assert!(Sense::KEYBOARD.focus);
        assert!(!Sense::KEYBOARD.scroll);
        assert!(!Sense::KEYBOARD.text);
        assert!(!Sense::KEYBOARD.right_click);
        assert!(!Sense::KEYBOARD.double_click);
        assert!(Sense::KEYBOARD.keyboard);
        assert!(Sense::KEYBOARD.interactive());
        assert!(!Sense::KEYBOARD.is_passive());
    }

    #[test]
    fn test_text_input_constant() {
        assert!(Sense::TEXT_INPUT.click);
        assert!(Sense::TEXT_INPUT.drag);
        assert!(Sense::TEXT_INPUT.hover);
        assert!(Sense::TEXT_INPUT.focus);
        assert!(!Sense::TEXT_INPUT.scroll);
        assert!(Sense::TEXT_INPUT.text);
        assert!(!Sense::TEXT_INPUT.right_click);
        assert!(!Sense::TEXT_INPUT.double_click);
        assert!(!Sense::TEXT_INPUT.keyboard);
        assert_eq!(Sense::text_input(), Sense::TEXT_INPUT);
        assert!(Sense::TEXT_INPUT.interactive());
        assert!(!Sense::TEXT_INPUT.is_passive());
    }

    #[test]
    fn test_constructor_methods() {
        assert_eq!(Sense::none(), Sense::NONE);
        assert_eq!(Sense::hover(), Sense::HOVER);
        assert_eq!(Sense::click(), Sense::CLICK);
        assert_eq!(Sense::drag(), Sense::DRAG);
        assert_eq!(Sense::click_and_drag(), Sense::CLICK_AND_DRAG);
        assert_eq!(Sense::focusable(), Sense::FOCUSABLE);
        assert_eq!(Sense::scroll(), Sense::SCROLL);
        assert_eq!(Sense::all(), Sense::ALL);
    }

    #[test]
    fn test_union() {
        let click = Sense::click();
        let drag = Sense::drag();
        let combined = click.union(drag);

        assert!(combined.click);
        assert!(combined.drag);
        assert!(combined.hover);
        assert!(!combined.focus);
        assert!(!combined.scroll);
        assert!(!combined.right_click);
        assert!(!combined.double_click);
        assert!(!combined.keyboard);
        assert_eq!(combined, Sense::CLICK_AND_DRAG);

        let with_scroll = Sense::click().union(Sense::scroll());
        assert!(with_scroll.click);
        assert!(with_scroll.scroll);
        assert!(with_scroll.hover);

        let with_rc = Sense::click().union(Sense::RIGHT_CLICK);
        assert!(with_rc.click);
        assert!(with_rc.right_click);
        assert!(with_rc.hover);
    }

    #[test]
    fn test_intersection() {
        let click_and_drag = Sense::CLICK_AND_DRAG;
        let click = Sense::click();
        let common = click_and_drag.intersection(click);

        assert!(common.click);
        assert!(!common.drag);
        assert!(common.hover);
        assert!(!common.focus);
        assert!(!common.scroll);
        assert!(!common.right_click);
        assert!(!common.double_click);
        assert!(!common.keyboard);

        let all_and_scroll = Sense::ALL.intersection(Sense::SCROLL);
        assert!(!all_and_scroll.click);
        assert!(!all_and_scroll.drag);
        assert!(all_and_scroll.hover);
        assert!(!all_and_scroll.focus);
        assert!(all_and_scroll.scroll);
        assert!(!all_and_scroll.right_click);
        assert!(!all_and_scroll.double_click);
        assert!(!all_and_scroll.keyboard);
    }

    #[test]
    fn test_with_methods() {
        let sense = Sense::none().with_click();
        assert!(sense.click);
        assert!(sense.hover);
        assert!(!sense.drag);

        let sense = Sense::none().with_drag();
        assert!(sense.drag);
        assert!(sense.hover);
        assert!(!sense.click);

        let sense = Sense::click().with_focus();
        assert!(sense.click);
        assert!(sense.focus);
        assert!(sense.hover);

        let sense = Sense::none().with_scroll();
        assert!(sense.scroll);
        assert!(sense.hover);
        assert!(!sense.click);
        assert!(!sense.drag);
        assert!(!sense.focus);

        let sense = Sense::none().with_click().with_drag().with_focus().with_scroll();
        assert!(sense.click);
        assert!(sense.drag);
        assert!(sense.hover);
        assert!(sense.focus);
        assert!(sense.scroll);
        assert!(!sense.text);
        assert!(!sense.right_click);
        assert!(!sense.double_click);
        assert!(!sense.keyboard);
    }

    #[test]
    fn test_new_with_methods() {
        let sense = Sense::none().with_right_click();
        assert!(sense.right_click);
        assert!(sense.hover);
        assert!(!sense.click);
        assert!(!sense.drag);
        assert!(!sense.focus);
        assert!(!sense.double_click);
        assert!(!sense.keyboard);
        assert_eq!(sense, Sense::RIGHT_CLICK);

        let sense = Sense::none().with_double_click();
        assert!(sense.double_click);
        assert!(sense.hover);
        assert!(!sense.click);
        assert!(!sense.drag);
        assert!(!sense.focus);
        assert!(!sense.right_click);
        assert!(!sense.keyboard);
        assert_eq!(sense, Sense::DOUBLE_CLICK);

        let sense = Sense::none().with_keyboard();
        assert!(sense.keyboard);
        assert!(sense.focus);
        assert!(sense.hover);
        assert!(!sense.click);
        assert!(!sense.drag);
        assert!(!sense.right_click);
        assert!(!sense.double_click);
        assert_eq!(sense, Sense::KEYBOARD);

        // chaining
        let sense = Sense::none().with_click().with_right_click().with_double_click().with_keyboard();
        assert!(sense.click);
        assert!(sense.right_click);
        assert!(sense.double_click);
        assert!(sense.keyboard);
        assert!(sense.focus);
        assert!(sense.hover);
        assert!(sense.interactive());
        assert!(!sense.is_passive());
    }

    #[test]
    fn test_query_methods() {
        assert!(Sense::click().interactive());
        assert!(Sense::drag().interactive());
        assert!(Sense::focusable().interactive());
        assert!(Sense::scroll().interactive());
        assert!(Sense::RIGHT_CLICK.interactive());
        assert!(Sense::DOUBLE_CLICK.interactive());
        assert!(Sense::KEYBOARD.interactive());
        assert!(!Sense::hover().interactive());
        assert!(!Sense::none().interactive());

        assert!(Sense::CLICK_AND_DRAG.has_click_and_drag());
        assert!(Sense::ALL.has_click_and_drag());
        assert!(!Sense::click().has_click_and_drag());
        assert!(!Sense::drag().has_click_and_drag());

        assert!(Sense::none().is_passive());
        assert!(Sense::hover().is_passive());
        assert!(!Sense::click().is_passive());
        assert!(!Sense::drag().is_passive());
        assert!(!Sense::focusable().is_passive());
        assert!(!Sense::scroll().is_passive());
        assert!(!Sense::RIGHT_CLICK.is_passive());
        assert!(!Sense::DOUBLE_CLICK.is_passive());
        assert!(!Sense::KEYBOARD.is_passive());
    }

    #[test]
    fn test_bitor_operator() {
        let combined = Sense::click() | Sense::drag();
        assert_eq!(combined, Sense::CLICK_AND_DRAG);

        let mut sense = Sense::click();
        sense |= Sense::drag();
        assert_eq!(sense, Sense::CLICK_AND_DRAG);

        let complex = Sense::click() | Sense::drag() | Sense::focusable();
        assert!(complex.click);
        assert!(complex.drag);
        assert!(complex.focus);
        assert!(complex.hover);
        assert!(!complex.scroll);

        let with_scroll = Sense::click() | Sense::scroll();
        assert!(with_scroll.click);
        assert!(with_scroll.scroll);
        assert!(with_scroll.hover);

        let with_new = Sense::RIGHT_CLICK | Sense::DOUBLE_CLICK | Sense::KEYBOARD;
        assert!(with_new.right_click);
        assert!(with_new.double_click);
        assert!(with_new.keyboard);
        assert!(with_new.hover);
        assert!(with_new.focus);
    }

    #[test]
    fn test_default() {
        let default_sense = Sense::default();
        assert_eq!(default_sense, Sense::NONE);
    }

    #[test]
    fn test_eq_and_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(Sense::click());
        set.insert(Sense::click());
        set.insert(Sense::drag());
        set.insert(Sense::scroll());
        set.insert(Sense::RIGHT_CLICK);
        set.insert(Sense::DOUBLE_CLICK);
        set.insert(Sense::KEYBOARD);

        assert_eq!(set.len(), 6);
        assert!(set.contains(&Sense::click()));
        assert!(set.contains(&Sense::drag()));
        assert!(set.contains(&Sense::scroll()));
        assert!(set.contains(&Sense::RIGHT_CLICK));
        assert!(set.contains(&Sense::DOUBLE_CLICK));
        assert!(set.contains(&Sense::KEYBOARD));
    }

    #[test]
    fn test_clone_and_copy() {
        let original = Sense::click();
        let cloned = original.clone();
        let copied = original;

        assert_eq!(original, cloned);
        assert_eq!(original, copied);
    }

    #[test]
    fn new_bits_are_off_in_named_senses_and_on_in_all() {
        for sense in [Sense::NONE, Sense::HOVER, Sense::CLICK, Sense::DRAG, Sense::TEXT_INPUT, Sense::KEYBOARD] {
            assert!(!sense.ime && !sense.drop && !sense.gesture && !sense.touch, "{sense:?}");
        }
        let all = Sense::ALL;
        assert!(all.ime && all.drop && all.gesture && all.touch);
    }

    #[test]
    fn new_builders_set_their_bit_and_hover_and_count_as_interactive() {
        let ime = Sense::NONE.with_ime();
        assert!(ime.ime && ime.focus && ime.hover);
        for sense in [Sense::NONE.with_drop(), Sense::NONE.with_gesture(), Sense::NONE.with_touch()] {
            assert!(sense.hover);
            assert!(sense.interactive());
            assert!(!sense.is_passive());
        }
        assert!(Sense::NONE.with_drop().drop);
        assert!(Sense::NONE.with_gesture().gesture);
        assert!(Sense::NONE.with_touch().touch);
    }

    #[test]
    fn union_and_intersection_carry_the_new_bits() {
        let a = Sense::NONE.with_touch().with_ime();
        let b = Sense::NONE.with_touch().with_drop();
        let u = a.union(b);
        assert!(u.touch && u.ime && u.drop && !u.gesture);
        let i = a.intersection(b);
        assert!(i.touch && !i.ime && !i.drop);
    }

    #[test]
    fn select_bit_is_off_in_named_senses_and_on_in_all() {
        for sense in [
            Sense::NONE, Sense::HOVER, Sense::CLICK, Sense::DRAG, Sense::CLICK_AND_DRAG,
            Sense::FOCUSABLE, Sense::SCROLL, Sense::TEXT_INPUT, Sense::RIGHT_CLICK,
            Sense::DOUBLE_CLICK, Sense::KEYBOARD, Sense::default(),
        ] {
            assert!(!sense.select, "{sense:?}");
        }
        assert!(Sense::ALL.select);
    }

    #[test]
    fn with_select_sets_the_bit_and_hover_and_counts_as_interactive() {
        let s = Sense::NONE.with_select();
        assert!(s.select && s.hover);
        assert!(!s.click && !s.drag && !s.focus && !s.text);
        assert!(s.interactive());
        assert!(!s.is_passive());
        // a hover-only label stays passive until it opts in
        assert!(Sense::HOVER.is_passive());
        assert!(!Sense::HOVER.with_select().is_passive());
    }

    #[test]
    fn union_and_intersection_carry_the_select_bit() {
        let a = Sense::HOVER.with_select();
        let b = Sense::CLICK;
        assert!(a.union(b).select);
        assert!((b | a).select);
        assert!(!a.intersection(b).select);
        assert!(a.intersection(Sense::ALL).select);
    }
}
