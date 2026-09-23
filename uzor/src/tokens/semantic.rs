//! Semantic colour roles: the fixed, typed contract every token set resolves
//! into (H1 token contract design, §1 "Semantic role list"). One field per
//! role, never a string-keyed map.

use crate::tokens::color::ColorValue;

/// The fixed semantic role set. `Option<ColorValue>` only for a role with no
/// built-in value yet (`surface_row_alt` — reserved, no consumer among the
/// H1 widgets; list/table widgets land in H3).
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticRoles {
    pub surface_app_chrome: ColorValue,
    pub surface_control_idle: ColorValue,
    pub surface_control_hover: ColorValue,
    pub surface_control_active: ColorValue,
    pub surface_floating: ColorValue,
    pub surface_header: ColorValue,
    pub surface_panel: ColorValue,
    pub surface_row_alt: Option<ColorValue>,
    pub text_primary: ColorValue,
    pub text_secondary: ColorValue,
    pub text_muted: ColorValue,
    pub text_disabled: ColorValue,
    pub text_on_accent: ColorValue,
    pub border_subtle: ColorValue,
    pub border_default: ColorValue,
    pub border_strong: ColorValue,
    pub accent_default: ColorValue,
    pub accent_hover: ColorValue,
    pub accent_pressed: ColorValue,
    pub status_success: ColorValue,
    pub status_success_bg: ColorValue,
    pub status_danger: ColorValue,
    pub status_danger_bg: ColorValue,
    pub status_warning: ColorValue,
    pub status_warning_bg: ColorValue,
    pub status_info: ColorValue,
    pub status_info_bg: ColorValue,
    pub selection: ColorValue,
    pub focus_ring: ColorValue,
    pub backdrop_dim: ColorValue,
    pub backdrop_full: ColorValue,
    pub shadow_default: ColorValue,
}

/// Alias-resolution key for one [`SemanticRoles`] field — one variant per
/// field, used only while the DTCG loader (Brief 2) walks `{color.a.b.c}`
/// references in a token file. Never a runtime lookup surface: there is no
/// `SemanticRoles::get(Role) -> ColorValue` method, callers always use the
/// named field directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    SurfaceAppChrome,
    SurfaceControlIdle,
    SurfaceControlHover,
    SurfaceControlActive,
    SurfaceFloating,
    SurfaceHeader,
    SurfacePanel,
    SurfaceRowAlt,
    TextPrimary,
    TextSecondary,
    TextMuted,
    TextDisabled,
    TextOnAccent,
    BorderSubtle,
    BorderDefault,
    BorderStrong,
    AccentDefault,
    AccentHover,
    AccentPressed,
    StatusSuccess,
    StatusSuccessBg,
    StatusDanger,
    StatusDangerBg,
    StatusWarning,
    StatusWarningBg,
    StatusInfo,
    StatusInfoBg,
    Selection,
    FocusRing,
    BackdropDim,
    BackdropFull,
    ShadowDefault,
}

impl Role {
    /// Every `Role` variant, one entry per [`SemanticRoles`] field — used
    /// for exhaustive iteration in tests and by [`Self::from_path`].
    pub const ALL: &'static [Role] = &[
        Role::SurfaceAppChrome,
        Role::SurfaceControlIdle,
        Role::SurfaceControlHover,
        Role::SurfaceControlActive,
        Role::SurfaceFloating,
        Role::SurfaceHeader,
        Role::SurfacePanel,
        Role::SurfaceRowAlt,
        Role::TextPrimary,
        Role::TextSecondary,
        Role::TextMuted,
        Role::TextDisabled,
        Role::TextOnAccent,
        Role::BorderSubtle,
        Role::BorderDefault,
        Role::BorderStrong,
        Role::AccentDefault,
        Role::AccentHover,
        Role::AccentPressed,
        Role::StatusSuccess,
        Role::StatusSuccessBg,
        Role::StatusDanger,
        Role::StatusDangerBg,
        Role::StatusWarning,
        Role::StatusWarningBg,
        Role::StatusInfo,
        Role::StatusInfoBg,
        Role::Selection,
        Role::FocusRing,
        Role::BackdropDim,
        Role::BackdropFull,
        Role::ShadowDefault,
    ];

    /// The dotted DTCG path a `{color.<path>}` alias resolves through, e.g.
    /// `Role::SurfaceControlHover` ↔ `"surface.control.hover"`. This is a
    /// semantic dot-join of role-group segments, NOT a mechanical `_` → `.`
    /// replace of the whole field name — `StatusSuccessBg`'s path is
    /// `"status.success_bg"` (two segments: group `status`, role
    /// `success_bg`), not `"status.success.bg"`.
    pub fn path(self) -> &'static str {
        match self {
            Role::SurfaceAppChrome => "surface.app_chrome",
            Role::SurfaceControlIdle => "surface.control.idle",
            Role::SurfaceControlHover => "surface.control.hover",
            Role::SurfaceControlActive => "surface.control.active",
            Role::SurfaceFloating => "surface.floating",
            Role::SurfaceHeader => "surface.header",
            Role::SurfacePanel => "surface.panel",
            Role::SurfaceRowAlt => "surface.row_alt",
            Role::TextPrimary => "text.primary",
            Role::TextSecondary => "text.secondary",
            Role::TextMuted => "text.muted",
            Role::TextDisabled => "text.disabled",
            Role::TextOnAccent => "text.on_accent",
            Role::BorderSubtle => "border.subtle",
            Role::BorderDefault => "border.default",
            Role::BorderStrong => "border.strong",
            Role::AccentDefault => "accent.default",
            Role::AccentHover => "accent.hover",
            Role::AccentPressed => "accent.pressed",
            Role::StatusSuccess => "status.success",
            Role::StatusSuccessBg => "status.success_bg",
            Role::StatusDanger => "status.danger",
            Role::StatusDangerBg => "status.danger_bg",
            Role::StatusWarning => "status.warning",
            Role::StatusWarningBg => "status.warning_bg",
            Role::StatusInfo => "status.info",
            Role::StatusInfoBg => "status.info_bg",
            Role::Selection => "selection",
            Role::FocusRing => "focus_ring",
            Role::BackdropDim => "backdrop.dim",
            Role::BackdropFull => "backdrop.full",
            Role::ShadowDefault => "shadow.default",
        }
    }

    /// The inverse of [`Self::path`]. `None` for an unrecognised path — the
    /// DTCG loader (Brief 2) turns that into a named
    /// `TokenLoadError::UnknownPath`.
    pub fn from_path(path: &str) -> Option<Role> {
        Role::ALL.iter().copied().find(|role| role.path() == path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_role_round_trips_through_its_path() {
        for role in Role::ALL {
            let path = role.path();
            assert_eq!(
                Role::from_path(path),
                Some(*role),
                "path {path:?} did not round-trip back to {role:?}"
            );
        }
    }

    #[test]
    fn path_list_has_no_duplicates() {
        let mut seen = HashSet::new();
        for role in Role::ALL {
            assert!(seen.insert(role.path()), "duplicate token path: {}", role.path());
        }
        assert_eq!(seen.len(), Role::ALL.len());
    }

    #[test]
    fn from_path_rejects_unknown_path() {
        assert_eq!(Role::from_path("surface.control.bogus"), None);
        assert_eq!(Role::from_path(""), None);
    }

    #[test]
    fn semantic_roles_constructs_with_every_field_set() {
        let placeholder = ColorValue::Transparent;
        let roles = SemanticRoles {
            surface_app_chrome: placeholder.clone(),
            surface_control_idle: placeholder.clone(),
            surface_control_hover: placeholder.clone(),
            surface_control_active: placeholder.clone(),
            surface_floating: placeholder.clone(),
            surface_header: placeholder.clone(),
            surface_panel: placeholder.clone(),
            surface_row_alt: None,
            text_primary: placeholder.clone(),
            text_secondary: placeholder.clone(),
            text_muted: placeholder.clone(),
            text_disabled: placeholder.clone(),
            text_on_accent: placeholder.clone(),
            border_subtle: placeholder.clone(),
            border_default: placeholder.clone(),
            border_strong: placeholder.clone(),
            accent_default: placeholder.clone(),
            accent_hover: placeholder.clone(),
            accent_pressed: placeholder.clone(),
            status_success: placeholder.clone(),
            status_success_bg: placeholder.clone(),
            status_danger: placeholder.clone(),
            status_danger_bg: placeholder.clone(),
            status_warning: placeholder.clone(),
            status_warning_bg: placeholder.clone(),
            status_info: placeholder.clone(),
            status_info_bg: placeholder.clone(),
            selection: placeholder.clone(),
            focus_ring: placeholder.clone(),
            backdrop_dim: placeholder.clone(),
            backdrop_full: placeholder.clone(),
            shadow_default: placeholder,
        };
        assert_eq!(roles.surface_app_chrome, ColorValue::Transparent);
        assert_eq!(roles.surface_row_alt, None);
    }
}
