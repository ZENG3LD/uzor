//! [`PlaceholderKind`]/[`PlaceholderSlot`] — a named slot in a
//! [`super::SlideMaster`]/[`super::SlideLayout`] that an instance fills
//! with ANY block kind (design doc §5): a deliberate improvement over
//! BOTH OOXML (a restricted placeholder-type set) and Google Slides
//! (Shape only) — see the design doc's own note.

/// The semantic role a placeholder slot plays — closed enum, matches the
/// design doc's own list exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlaceholderKind {
    Title,
    Body,
    Media,
    Chart,
    Table,
    Caption,
}

/// One placeholder slot in a [`super::SlideMaster`]/[`super::SlideLayout`]
/// — identified by `(kind, index)` so a master can declare more than one
/// slot of the same kind (e.g. two `Media` slots on a two-up layout).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlaceholderSlot {
    pub kind: PlaceholderKind,
    pub index: u32,
}

impl PlaceholderSlot {
    pub fn new(kind: PlaceholderKind, index: u32) -> Self {
        Self { kind, index }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_of_the_same_kind_are_distinguished_by_index() {
        let a = PlaceholderSlot::new(PlaceholderKind::Media, 0);
        let b = PlaceholderSlot::new(PlaceholderKind::Media, 1);
        assert_ne!(a, b);
        assert_eq!(a.kind, b.kind);
    }

    #[test]
    fn slots_of_different_kinds_at_the_same_index_are_distinct() {
        let a = PlaceholderSlot::new(PlaceholderKind::Title, 0);
        let b = PlaceholderSlot::new(PlaceholderKind::Body, 0);
        assert_ne!(a, b);
    }
}
