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
    /// Widget accepts text input
    pub text: bool,
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
    };

    /// Only hover detection
    pub const HOVER: Sense = Sense {
        click: false,
        drag: false,
        hover: true,
        focus: false,
        scroll: false,
        text: false,
    };

    /// Click and hover (for buttons, checkboxes)
    pub const CLICK: Sense = Sense {
        click: true,
        drag: false,
        hover: true,
        focus: false,
        scroll: false,
        text: false,
    };

    /// Drag and hover (for sliders, scrollbars)
    pub const DRAG: Sense = Sense {
        click: false,
        drag: true,
        hover: true,
        focus: false,
        scroll: false,
        text: false,
    };

    /// Both click and drag (introduces latency)
    pub const CLICK_AND_DRAG: Sense = Sense {
        click: true,
        drag: true,
        hover: true,
        focus: false,
        scroll: false,
        text: false,
    };

    /// Can receive keyboard focus but no mouse interaction
    pub const FOCUSABLE: Sense = Sense {
        click: false,
        drag: false,
        hover: true,
        focus: true,
        scroll: false,
        text: false,
    };

    /// Scroll-sensitive (for scrollable container viewports)
    pub const SCROLL: Sense = Sense {
        click: false,
        drag: false,
        hover: true,
        focus: false,
        scroll: true,
        text: false,
    };

    /// Full interaction - click, drag, hover, focus, scroll
    pub const ALL: Sense = Sense {
        click: true,
        drag: true,
        hover: true,
        focus: true,
        scroll: true,
        text: false,
    };

    /// Text input - click, drag, hover, focus, text
    pub const TEXT_INPUT: Sense = Sense {
        click: true,
        drag: true,
        hover: true,
        focus: true,
        scroll: false,
        text: true,
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

    /// Add text input capability (also enables focus and hover)
    #[inline]
    pub fn with_text(mut self) -> Self {
        self.text = true;
        self.focus = true;
        self.hover = true;
        self
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
}

// Query methods
impl Sense {
    /// Check if any interaction is sensed (click, drag, focus, scroll, or text)
    #[inline]
    pub fn interactive(&self) -> bool {
        self.click || self.drag || self.focus || self.scroll || self.text
    }

    /// Check if both click and drag are sensed (has latency)
    #[inline]
    pub fn has_click_and_drag(&self) -> bool {
        self.click && self.drag
    }

    /// Check if widget is purely visual (no interactions)
    #[inline]
    pub fn is_passive(&self) -> bool {
        !self.click && !self.drag && !self.focus && !self.scroll && !self.text
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

        assert!(!Sense::HOVER.click);
        assert!(Sense::HOVER.hover);
        assert!(!Sense::HOVER.scroll);
        assert!(!Sense::HOVER.text);

        assert!(Sense::CLICK.click);
        assert!(!Sense::CLICK.drag);
        assert!(Sense::CLICK.hover);
        assert!(!Sense::CLICK.scroll);
        assert!(!Sense::CLICK.text);

        assert!(!Sense::DRAG.click);
        assert!(Sense::DRAG.drag);
        assert!(Sense::DRAG.hover);
        assert!(!Sense::DRAG.scroll);
        assert!(!Sense::DRAG.text);

        assert!(Sense::CLICK_AND_DRAG.click);
        assert!(Sense::CLICK_AND_DRAG.drag);
        assert!(Sense::CLICK_AND_DRAG.hover);
        assert!(!Sense::CLICK_AND_DRAG.scroll);
        assert!(!Sense::CLICK_AND_DRAG.text);

        assert!(!Sense::FOCUSABLE.click);
        assert!(Sense::FOCUSABLE.hover);
        assert!(Sense::FOCUSABLE.focus);
        assert!(!Sense::FOCUSABLE.scroll);
        assert!(!Sense::FOCUSABLE.text);

        assert!(!Sense::SCROLL.click);
        assert!(!Sense::SCROLL.drag);
        assert!(Sense::SCROLL.hover);
        assert!(!Sense::SCROLL.focus);
        assert!(Sense::SCROLL.scroll);
        assert!(!Sense::SCROLL.text);

        assert!(Sense::ALL.click);
        assert!(Sense::ALL.drag);
        assert!(Sense::ALL.hover);
        assert!(Sense::ALL.focus);
        assert!(Sense::ALL.scroll);
        assert!(!Sense::ALL.text);
    }

    #[test]
    fn test_text_input_constant() {
        assert!(Sense::TEXT_INPUT.click);
        assert!(Sense::TEXT_INPUT.drag);
        assert!(Sense::TEXT_INPUT.hover);
        assert!(Sense::TEXT_INPUT.focus);
        assert!(!Sense::TEXT_INPUT.scroll);
        assert!(Sense::TEXT_INPUT.text);
        assert_eq!(Sense::text_input(), Sense::TEXT_INPUT);
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
        assert!(!combined.text);
        assert_eq!(combined, Sense::CLICK_AND_DRAG);

        let with_scroll = Sense::click().union(Sense::scroll());
        assert!(with_scroll.click);
        assert!(with_scroll.scroll);
        assert!(with_scroll.hover);
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
        assert!(!common.text);

        let all_and_scroll = Sense::ALL.intersection(Sense::SCROLL);
        assert!(!all_and_scroll.click);
        assert!(!all_and_scroll.drag);
        assert!(all_and_scroll.hover);
        assert!(!all_and_scroll.focus);
        assert!(all_and_scroll.scroll);
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

        let sense = Sense::none().with_text();
        assert!(sense.text);
        assert!(sense.focus);
        assert!(sense.hover);
    }

    #[test]
    fn test_query_methods() {
        assert!(Sense::click().interactive());
        assert!(Sense::drag().interactive());
        assert!(Sense::focusable().interactive());
        assert!(Sense::scroll().interactive());
        assert!(Sense::text_input().interactive());
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
        assert!(!Sense::text_input().is_passive());
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

        assert_eq!(set.len(), 3);
        assert!(set.contains(&Sense::click()));
        assert!(set.contains(&Sense::drag()));
        assert!(set.contains(&Sense::scroll()));
    }

    #[test]
    fn test_clone_and_copy() {
        let original = Sense::click();
        let cloned = original.clone();
        let copied = original;

        assert_eq!(original, cloned);
        assert_eq!(original, copied);
    }
}
