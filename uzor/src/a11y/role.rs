//! Semantic accessibility role.

/// Semantic role of an accessibility node — modeled 1:1 on a subset of
/// AccessKit's own `accesskit::Role` enum (accesskit 0.19-shaped, from
/// memory of its public docs — NOT fetched fresh in this pass; re-verify
/// exact upstream member names against whichever `accesskit` version is
/// pinned before writing the H7 adapter that maps this onto
/// `accesskit::Node`). No `accesskit` dependency here — this type is pure
/// uzor and must stay that way per the master plan's H0/H7 split.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum A11yRole {
    Button,
    CheckBox,
    RadioButton,
    Switch,
    Slider,
    TextInput,
    Tab,
    TabList,
    MenuItem,
    ListItem,
    Group,
    Dialog,
    Tooltip,
    Unknown,
}
