//! [`PropertyState`] — Google Slides' 3-state resolution algebra (design
//! doc §5), adopted directly: a style property at any level of a
//! `master -> layout -> instance` delta chain is either an explicit value
//! (`Rendered`), explicitly cleared (`NotRendered` — a real, TERMINAL
//! answer, not a synonym for "unset"), or deferred to whatever the parent
//! level resolves to (`Inherit`).
//!
//! `Inherit` is illegal on a parentless (master/root) level — there is no
//! parent left to defer to. [`RootPropertyState`] enforces this AT
//! CONSTRUCTION TIME (design doc §7 P2 risk note: "a documented
//! illegal/panic-worthy state ... not a silent fallback to some
//! default"), rather than leaving it an unreachable-but-representable
//! enum value that some call site could still quietly construct.

/// One level's own contribution to a resolved property (design doc §5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyState<T> {
    /// An explicit value, set at THIS level.
    Rendered(T),
    /// Explicitly cleared AT THIS LEVEL — a real, terminal resolution
    /// (design doc §7 P2 risk note: "`NotRendered` != unset"). Resolution
    /// stops here; it never falls through toward the root the way
    /// `Inherit` does.
    NotRendered,
    /// Defer to the previous (more general) level in the chain. Legal on
    /// a layout/instance level; illegal on the root/master level — see
    /// [`RootPropertyState`].
    Inherit,
}

/// What [`resolve_property_chain`] settles on once every `Inherit` has
/// been walked past.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedProperty<T> {
    Value(T),
    NotRendered,
}

impl<T> ResolvedProperty<T> {
    /// `Some(value)` for [`ResolvedProperty::Value`], `None` for
    /// [`ResolvedProperty::NotRendered`] — the common case where a caller
    /// just wants an `Option` to fall back on with its own default.
    pub fn value(self) -> Option<T> {
        match self {
            ResolvedProperty::Value(v) => Some(v),
            ResolvedProperty::NotRendered => None,
        }
    }
}

/// A [`PropertyState`] at the ROOT (master) level of a resolution chain —
/// has no parent to defer to, so [`PropertyState::Inherit`] there is
/// nonsensical. [`RootPropertyState::new`] panics immediately if handed
/// one (design doc §7 P2 risk note), rather than silently treating it as
/// some default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootPropertyState<T>(PropertyState<T>);

impl<T> RootPropertyState<T> {
    /// # Panics
    /// If `state` is [`PropertyState::Inherit`] — a master-level node has
    /// no parent to defer to, so this is a genuine construction-time
    /// error, never a state this crate silently degrades from.
    pub fn new(state: PropertyState<T>) -> Self {
        assert!(
            !matches!(state, PropertyState::Inherit),
            "PropertyState::Inherit is illegal on a parentless (master-level) node — it has no parent to defer to"
        );
        Self(state)
    }

    pub fn get(&self) -> &PropertyState<T> {
        &self.0
    }
}

/// Resolve a `root (master) -> ... -> leaf (instance)` property chain:
/// walk `chain` from the LEAF backward, stopping at the first
/// `Rendered`/`NotRendered`; every `Inherit` in between defers one level
/// further toward the root. `root` can never itself be `Inherit`
/// (enforced at [`RootPropertyState::new`]), so this function always
/// terminates.
pub fn resolve_property_chain<T: Clone>(root: &RootPropertyState<T>, chain: &[PropertyState<T>]) -> ResolvedProperty<T> {
    for state in chain.iter().rev() {
        match state {
            PropertyState::Rendered(v) => return ResolvedProperty::Value(v.clone()),
            PropertyState::NotRendered => return ResolvedProperty::NotRendered,
            PropertyState::Inherit => continue,
        }
    }
    match root.get() {
        PropertyState::Rendered(v) => ResolvedProperty::Value(v.clone()),
        PropertyState::NotRendered => ResolvedProperty::NotRendered,
        PropertyState::Inherit => unreachable!("RootPropertyState::new already rejects Inherit"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(state: PropertyState<u32>) -> RootPropertyState<u32> {
        RootPropertyState::new(state)
    }

    /// Table-driven per design doc §7 P2's own gate: instance beats
    /// layout, layout beats master, `Inherit` walks all the way up, and
    /// `NotRendered` is a real terminal answer distinct from `Inherit`.
    #[test]
    fn tri_state_resolution_table() {
        struct Case {
            name: &'static str,
            master: PropertyState<u32>,
            chain: Vec<PropertyState<u32>>,
            expected: ResolvedProperty<u32>,
        }

        let cases = [
            Case {
                name: "instance Set beats layout Set",
                master: PropertyState::Rendered(1),
                chain: vec![PropertyState::Rendered(2), PropertyState::Rendered(3)],
                expected: ResolvedProperty::Value(3),
            },
            Case {
                name: "layout Set beats master when instance Inherits",
                master: PropertyState::Rendered(1),
                chain: vec![PropertyState::Rendered(2), PropertyState::Inherit],
                expected: ResolvedProperty::Value(2),
            },
            Case {
                name: "Inherit walks all the way up to master when every level between defers",
                master: PropertyState::Rendered(1),
                chain: vec![PropertyState::Inherit, PropertyState::Inherit],
                expected: ResolvedProperty::Value(1),
            },
            Case {
                name: "an empty chain resolves directly to the root",
                master: PropertyState::Rendered(9),
                chain: vec![],
                expected: ResolvedProperty::Value(9),
            },
            Case {
                name: "NotRendered is a terminal answer distinct from Inherit — never collapses back to master's value",
                master: PropertyState::Rendered(1),
                chain: vec![PropertyState::NotRendered, PropertyState::Inherit],
                expected: ResolvedProperty::NotRendered,
            },
            Case {
                name: "every level inheriting falls through to a NotRendered root",
                master: PropertyState::NotRendered,
                chain: vec![PropertyState::Inherit],
                expected: ResolvedProperty::NotRendered,
            },
        ];

        for case in cases {
            let resolved = resolve_property_chain(&root(case.master), &case.chain);
            assert_eq!(resolved, case.expected, "case failed: {}", case.name);
        }
    }

    #[test]
    #[should_panic(expected = "illegal on a parentless")]
    fn inherit_on_a_root_level_node_panics_at_construction() {
        let _ = RootPropertyState::new(PropertyState::<u32>::Inherit);
    }

    #[test]
    fn resolved_property_value_helper_converts_to_option() {
        assert_eq!(ResolvedProperty::Value(5).value(), Some(5));
        assert_eq!(ResolvedProperty::<u32>::NotRendered.value(), None);
    }
}
