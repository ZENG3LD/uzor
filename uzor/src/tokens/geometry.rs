//! Non-colour semantic roles: the corner-radius scale (H1 token contract
//! design, §1 "Geometry" table). `radius_sm`/`radius_lg` are an algorithmic
//! scale step off `radius_md` — the one geometry field MLC's `UIColors`
//! actually ships (`button_rounding`) — not independently-sourced values.

/// The fixed geometry role set. Always fully populated (no `Option`s) —
/// every built-in set defines a `radius_md`, so the scale steps derived from
/// it are always well-defined.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeometryRoles {
    pub radius_sm: f32,
    pub radius_md: f32,
    pub radius_lg: f32,
}

impl GeometryRoles {
    /// Builds the scale from just the base radius: `radius_sm` is half of
    /// `radius_md`, `radius_lg` is double — the fixed multiplier the token
    /// contract design accepts as the Radix-style scale-step treatment for
    /// geometry (unlike colour, where D1 forbids synthesizing a value).
    pub fn from_radius_md(radius_md: f32) -> Self {
        Self { radius_sm: radius_md * 0.5, radius_md, radius_lg: radius_md * 2.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_radius_md_derives_the_fixed_scale_steps() {
        let roles = GeometryRoles::from_radius_md(4.0);
        assert_eq!(roles, GeometryRoles { radius_sm: 2.0, radius_md: 4.0, radius_lg: 8.0 });
    }

    #[test]
    fn from_radius_md_handles_the_high_contrast_mono_zero_case() {
        let roles = GeometryRoles::from_radius_md(0.0);
        assert_eq!(roles, GeometryRoles { radius_sm: 0.0, radius_md: 0.0, radius_lg: 0.0 });
    }

    #[test]
    fn geometry_roles_constructs_with_explicit_fields() {
        let roles = GeometryRoles { radius_sm: 1.0, radius_md: 2.0, radius_lg: 4.0 };
        assert_eq!(roles.radius_sm, 1.0);
        assert_eq!(roles.radius_md, 2.0);
        assert_eq!(roles.radius_lg, 4.0);
    }
}
