//! `word/numbering.xml` — two fixed `abstractNum` definitions (bullet,
//! decimal) plus one fresh `numId` per authored `:::List` block, so every
//! list restarts numbering at 1 on its own (Word restarts per distinct
//! `numId`), matching `press.rs`'s own `push_list` always-starts-at-1
//! behavior.

/// `abstractNumId` for the bullet list definition.
const BULLET_ABSTRACT_ID: u32 = 0;
/// `abstractNumId` for the decimal (numbered) list definition.
const DECIMAL_ABSTRACT_ID: u32 = 1;
/// Left indent of the list text and the hanging indent of the marker, twips.
const LIST_LEFT_TWIPS: i64 = 720;
const LIST_HANGING_TWIPS: i64 = 360;
/// Distance from the text column to the list marker, in points: a border on
/// a list paragraph is drawn at the marker, not at the text.
pub(crate) const LIST_MARKER_OFFSET_PT: f64 = (LIST_LEFT_TWIPS - LIST_HANGING_TWIPS) as f64 / 20.0;

pub(crate) struct NumberingRegistry {
    next_num_id: u32,
    /// `(numId, ordered)` in allocation order — `to_numbering_xml` emits
    /// one `<w:num>` binding per entry.
    allocations: Vec<(u32, bool)>,
}

impl NumberingRegistry {
    /// Pre-seeds the 2 fixed `abstractNumId` defs (0 = bullet, 1 =
    /// decimal); no `numId` bindings exist yet.
    pub(crate) fn new() -> Self {
        Self { next_num_id: 1, allocations: Vec::new() }
    }

    /// One fresh `numId` per authored list block, bound to the right
    /// `abstractNumId`.
    pub(crate) fn allocate(&mut self, ordered: bool) -> u32 {
        let num_id = self.next_num_id;
        self.next_num_id += 1;
        self.allocations.push((num_id, ordered));
        num_id
    }

    /// The whole `word/numbering.xml` body.
    pub(crate) fn to_numbering_xml(&self) -> String {
        let mut xml = String::new();
        xml.push_str(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#);
        xml.push_str(r#"<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#);
        xml.push_str(&abstract_num_xml(BULLET_ABSTRACT_ID, "bullet", "\u{2022}"));
        xml.push_str(&abstract_num_xml(DECIMAL_ABSTRACT_ID, "decimal", "%1."));
        for (num_id, ordered) in &self.allocations {
            let abstract_id = if *ordered { DECIMAL_ABSTRACT_ID } else { BULLET_ABSTRACT_ID };
            // `startOverride` makes Word restart the count for this list; without
            // it Word continues the numbering of an earlier `w:num` that shares
            // the same `abstractNum`.
            xml.push_str(&format!(
                r#"<w:num w:numId="{num_id}"><w:abstractNumId w:val="{abstract_id}"/><w:lvlOverride w:ilvl="0"><w:startOverride w:val="1"/></w:lvlOverride></w:num>"#
            ));
        }
        xml.push_str("</w:numbering>");
        xml
    }
}

fn abstract_num_xml(abstract_id: u32, num_fmt: &str, lvl_text: &str) -> String {
    format!(
        r#"<w:abstractNum w:abstractNumId="{abstract_id}"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="{num_fmt}"/><w:lvlText w:val="{lvl_text}"/><w:lvlJc w:val="left"/><w:pPr><w:ind w:left="{LIST_LEFT_TWIPS}" w:hanging="{LIST_HANGING_TWIPS}"/></w:pPr></w:lvl></w:abstractNum>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocations_of_the_same_ordered_value_get_distinct_num_ids_same_abstract_id() {
        let mut reg = NumberingRegistry::new();
        let a = reg.allocate(false);
        let b = reg.allocate(false);
        assert_ne!(a, b);
        let xml = reg.to_numbering_xml();
        assert!(xml.contains(&format!(r#"<w:num w:numId="{a}"><w:abstractNumId w:val="{BULLET_ABSTRACT_ID}"/>"#)));
        assert!(xml.contains(&format!(r#"<w:num w:numId="{b}"><w:abstractNumId w:val="{BULLET_ABSTRACT_ID}"/>"#)));
    }

    #[test]
    fn every_list_restarts_its_own_numbering() {
        let mut reg = NumberingRegistry::new();
        reg.allocate(true);
        reg.allocate(true);
        let xml = reg.to_numbering_xml();
        assert_eq!(xml.matches(r#"<w:startOverride w:val="1"/>"#).count(), 2);
    }

    #[test]
    fn bullet_and_decimal_bind_to_different_abstract_ids() {
        let mut reg = NumberingRegistry::new();
        let bullet_id = reg.allocate(false);
        let decimal_id = reg.allocate(true);
        let xml = reg.to_numbering_xml();
        assert!(xml.contains(&format!(r#"<w:num w:numId="{bullet_id}"><w:abstractNumId w:val="{BULLET_ABSTRACT_ID}"/>"#)));
        assert!(xml.contains(&format!(r#"<w:num w:numId="{decimal_id}"><w:abstractNumId w:val="{DECIMAL_ABSTRACT_ID}"/>"#)));
    }
}
