//! Whether plain (non-input) text can be selected — the app-wide default
//! and the per-widget override.
//!
//! Plain text is **not** selectable by default. An app turns it on for every
//! label with one switch, [`TextPolicy::selectable_by_default`]; one widget
//! opts in with [`Sense::select`] (or [`SelectOverride::On`] on its
//! settings) and opts out with [`SelectOverride::Off`]. The rule is the same
//! on native and browser hosts: uzor paints text itself, there is no
//! platform selection on either.
//!
//! # Override shape
//!
//! The opt-in is the [`Sense::select`] bit, because that is what the input
//! layer reads for every other capability and any widget — built-in or
//! custom — can set it without new settings. The opt-out cannot be a second
//! sense bit: senses combine with OR (`union`), and an "off" that another
//! OR can switch back on is not an off. So the opt-out lives where the
//! policy is resolved — a tri-state [`SelectOverride`] on the text widget's
//! settings, defaulting to [`SelectOverride::Inherit`]. The smallest shape
//! that expresses "follow the app", "always" and "never" is exactly these
//! three values. [`resolve_sense`] folds the result back into the sense the
//! widget registers, so the selection owner only ever reads `sense.select`.
//!
//! Text inputs are outside this rule: a field always selects its own text
//! (its selection lives in the text store).

use crate::input::core::sense::Sense;

/// App-wide text policy. The framework owns one per app; the library only
/// defines it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextPolicy {
    /// Plain text is selectable unless a widget opts out. Default `false`:
    /// labels are not selectable unless a widget opts in.
    pub selectable_by_default: bool,
}

impl TextPolicy {
    /// Plain text not selectable unless a widget opts in (the default).
    pub const NOT_SELECTABLE: TextPolicy = TextPolicy { selectable_by_default: false };
    /// Plain text selectable unless a widget opts out.
    pub const SELECTABLE: TextPolicy = TextPolicy { selectable_by_default: true };
}

/// A text widget's own say on selection, on its settings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SelectOverride {
    /// Follow the app's [`TextPolicy`] (or the widget's `Sense::select`).
    #[default]
    Inherit,
    /// Always selectable.
    On,
    /// Never selectable, whatever the policy or sense says.
    Off,
}

/// Whether a plain-text widget registering `sense`, with `over` on its
/// settings, is selectable under `policy`.
///
/// | `over`    | `sense.select` | `policy.selectable_by_default` | result |
/// |-----------|----------------|--------------------------------|--------|
/// | `Off`     | any            | any                            | false  |
/// | `On`      | any            | any                            | true   |
/// | `Inherit` | true           | any                            | true   |
/// | `Inherit` | false          | b                              | b      |
pub fn is_selectable(sense: Sense, over: SelectOverride, policy: TextPolicy) -> bool {
    match over {
        SelectOverride::Off => false,
        SelectOverride::On => true,
        SelectOverride::Inherit => sense.select || policy.selectable_by_default,
    }
}

/// `sense` with its `select` bit set to [`is_selectable`]'s answer — the
/// sense a text widget registers, so the selection owner reads one bit.
/// With the default policy and `Inherit`, `sense` comes back unchanged.
pub fn resolve_sense(sense: Sense, over: SelectOverride, policy: TextPolicy) -> Sense {
    let select = is_selectable(sense, over, policy);
    let mut out = sense;
    out.select = select;
    if select {
        out.hover = true;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_is_not_selectable() {
        assert_eq!(TextPolicy::default(), TextPolicy::NOT_SELECTABLE);
        assert!(!TextPolicy::default().selectable_by_default);
        assert_eq!(SelectOverride::default(), SelectOverride::Inherit);
        assert!(!is_selectable(Sense::HOVER, SelectOverride::default(), TextPolicy::default()));
    }

    #[test]
    fn is_selectable_truth_table() {
        let plain = Sense::HOVER;
        let opted = Sense::HOVER.with_select();
        for (sense, over, policy, want) in [
            (plain, SelectOverride::Inherit, TextPolicy::NOT_SELECTABLE, false),
            (plain, SelectOverride::Inherit, TextPolicy::SELECTABLE, true),
            (opted, SelectOverride::Inherit, TextPolicy::NOT_SELECTABLE, true),
            (opted, SelectOverride::Inherit, TextPolicy::SELECTABLE, true),
            (plain, SelectOverride::On, TextPolicy::NOT_SELECTABLE, true),
            (plain, SelectOverride::On, TextPolicy::SELECTABLE, true),
            (opted, SelectOverride::On, TextPolicy::NOT_SELECTABLE, true),
            (opted, SelectOverride::On, TextPolicy::SELECTABLE, true),
            (plain, SelectOverride::Off, TextPolicy::NOT_SELECTABLE, false),
            (plain, SelectOverride::Off, TextPolicy::SELECTABLE, false),
            (opted, SelectOverride::Off, TextPolicy::NOT_SELECTABLE, false),
            (opted, SelectOverride::Off, TextPolicy::SELECTABLE, false),
        ] {
            assert_eq!(is_selectable(sense, over, policy), want, "{over:?} select={} {policy:?}", sense.select);
        }
    }

    #[test]
    fn resolve_sense_sets_only_the_select_bit() {
        assert_eq!(resolve_sense(Sense::HOVER, SelectOverride::Inherit, TextPolicy::default()), Sense::HOVER);
        assert_eq!(
            resolve_sense(Sense::HOVER, SelectOverride::Inherit, TextPolicy::SELECTABLE),
            Sense::HOVER.with_select()
        );
        let off = resolve_sense(Sense::HOVER.with_select(), SelectOverride::Off, TextPolicy::SELECTABLE);
        assert!(!off.select);
        assert!(off.hover);
        let on = resolve_sense(Sense::NONE, SelectOverride::On, TextPolicy::default());
        assert!(on.select && on.hover);
    }
}
