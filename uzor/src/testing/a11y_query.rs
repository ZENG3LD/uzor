//! Minimal AccessKit-shaped query surface for tests.

use crate::a11y::{A11yNode, A11yRole, A11yTree};

/// Failure modes for [`A11yQuery::find_by_role_name`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum A11yQueryError {
    /// No node with the given role/name exists in the tree.
    NotFound { role: A11yRole, name: String },
    /// More than one node with the given role/name exists — the query is
    /// ambiguous.
    Ambiguous { role: A11yRole, name: String, count: usize },
}

impl std::fmt::Display for A11yQueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { role, name } => {
                write!(f, "no {role:?} node named {name:?} found in the a11y tree")
            }
            Self::Ambiguous { role, name, count } => write!(
                f,
                "{count} {role:?} nodes named {name:?} found in the a11y tree — expected exactly one"
            ),
        }
    }
}

impl std::error::Error for A11yQueryError {}

/// Minimal AccessKit-shaped query surface over one frame's [`A11yTree`] —
/// find-by-role+name, then let the test's own `assert!`/`assert_eq!` read
/// the returned node's fields directly. Kept deliberately thinner than a
/// full widget-tree traversal DSL — H0 only needs a flat lookup; extend
/// when a second consumer needs more.
pub struct A11yQuery<'a> {
    tree: &'a A11yTree,
}

impl<'a> A11yQuery<'a> {
    /// Wrap a frame's [`A11yTree`] for querying.
    pub fn new(tree: &'a A11yTree) -> Self {
        Self { tree }
    }

    /// Find the single node with the given `role` and exact `name`.
    pub fn find_by_role_name(&self, role: A11yRole, name: &str) -> Result<&'a A11yNode, A11yQueryError> {
        let mut matches = self
            .tree
            .nodes()
            .iter()
            .filter(|node| node.role == role && node.name == name);
        let first = matches
            .next()
            .ok_or_else(|| A11yQueryError::NotFound { role, name: name.to_owned() })?;
        let extra = matches.count();
        if extra > 0 {
            return Err(A11yQueryError::Ambiguous {
                role,
                name: name.to_owned(),
                count: extra + 1,
            });
        }
        Ok(first)
    }

    /// All nodes with the given `role`.
    pub fn all_by_role(&self, role: A11yRole) -> Vec<&'a A11yNode> {
        self.tree.nodes().iter().filter(|node| node.role == role).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Rect;

    fn rect() -> Rect {
        Rect::new(0.0, 0.0, 10.0, 10.0)
    }

    #[test]
    fn find_by_role_name_finds_the_only_match() {
        let mut tree = A11yTree::new();
        tree.push(A11yNode::new("save", A11yRole::Button, "Save", rect()));
        let query = A11yQuery::new(&tree);
        let found = query
            .find_by_role_name(A11yRole::Button, "Save")
            .expect("must find the button");
        assert_eq!(found.name, "Save");
    }

    #[test]
    fn find_by_role_name_reports_not_found() {
        let tree = A11yTree::new();
        let query = A11yQuery::new(&tree);
        let err = query.find_by_role_name(A11yRole::Button, "Save").unwrap_err();
        assert_eq!(
            err,
            A11yQueryError::NotFound { role: A11yRole::Button, name: "Save".to_owned() }
        );
    }

    #[test]
    fn find_by_role_name_reports_ambiguous() {
        let mut tree = A11yTree::new();
        tree.push(A11yNode::new("save-1", A11yRole::Button, "Save", rect()));
        tree.push(A11yNode::new("save-2", A11yRole::Button, "Save", rect()));
        let query = A11yQuery::new(&tree);
        let err = query.find_by_role_name(A11yRole::Button, "Save").unwrap_err();
        assert_eq!(
            err,
            A11yQueryError::Ambiguous { role: A11yRole::Button, name: "Save".to_owned(), count: 2 }
        );
    }

    #[test]
    fn find_by_role_name_does_not_match_across_roles() {
        let mut tree = A11yTree::new();
        tree.push(A11yNode::new("save", A11yRole::TextInput, "Save", rect()));
        let query = A11yQuery::new(&tree);
        assert!(query.find_by_role_name(A11yRole::Button, "Save").is_err());
    }

    #[test]
    fn all_by_role_filters_by_role_only() {
        let mut tree = A11yTree::new();
        tree.push(A11yNode::new("save", A11yRole::Button, "Save", rect()));
        tree.push(A11yNode::new("cancel", A11yRole::Button, "Cancel", rect()));
        tree.push(A11yNode::new("email", A11yRole::TextInput, "Email", rect()));
        let query = A11yQuery::new(&tree);
        let buttons = query.all_by_role(A11yRole::Button);
        assert_eq!(buttons.len(), 2);
        assert!(buttons.iter().all(|node| node.role == A11yRole::Button));
    }
}
