//! [`TokenSet`] (loaded, alias/modifier-resolved, not yet validated for
//! completeness) and [`Tokens`] (the widget-consumable, fully validated
//! result of [`TokenSet::resolve`]) — see the H1 token contract design doc
//! §2. Also the `app.*` typed surface ([`AppTokens`], [`TokenValue`]) and
//! the `component.*` override surface ([`ComponentOverrides`], [`ColorSpec`],
//! [`Modifier`]) — coordinator amendment A1: `app.*` is resolved, typed, and
//! never exposes `serde_json::Value` publicly; `component.*` stays symbolic
//! (an alias + optional modifier) because no widget's own default-alias
//! table exists yet to resolve it against (that lands per-widget from H1
//! Brief 4 onward).

use std::collections::HashMap;

use crate::tokens::color::ColorValue;
use crate::tokens::dtcg::TokenLoadError;
use crate::tokens::geometry::GeometryRoles;
use crate::tokens::semantic::{Role, SemanticRoles};

/// A loaded token file: every `color.*`/`geometry.*` role actually defined
/// (alias chains already followed, modifiers already applied), the fully
/// resolved `app.*` namespace, and the still-symbolic `component.*`
/// overrides. Built only by [`crate::tokens::loader::load_token_set`].
///
/// Deliberately NOT yet validated for completeness — a file may omit
/// required roles, and `TokenSet` will still hold whatever it did define.
/// [`Self::resolve`] is where "does this file define every required role"
/// is checked, so that check has exactly one place to live.
#[derive(Clone, Debug)]
pub struct TokenSet {
    pub(crate) roles: HashMap<Role, ColorValue>,
    pub(crate) geometry: HashMap<&'static str, f32>,
    pub(crate) app: AppTokens,
    pub(crate) components: ComponentOverrides,
    pub(crate) name: Option<String>,
}

impl TokenSet {
    /// Validates that every required role/geometry field is present and
    /// assembles the final, widget-consumable [`Tokens`]. The one place a
    /// missing required token becomes [`TokenLoadError::MissingRequired`].
    pub fn resolve(&self) -> Result<Tokens, TokenLoadError> {
        let required_role = |role: Role| -> Result<ColorValue, TokenLoadError> {
            self.roles.get(&role).cloned().ok_or_else(|| TokenLoadError::MissingRequired {
                path: format!("color.{}", role.path()),
            })
        };
        let semantic = SemanticRoles {
            surface_app_chrome: required_role(Role::SurfaceAppChrome)?,
            surface_control_idle: required_role(Role::SurfaceControlIdle)?,
            surface_control_hover: required_role(Role::SurfaceControlHover)?,
            surface_control_active: required_role(Role::SurfaceControlActive)?,
            surface_floating: required_role(Role::SurfaceFloating)?,
            surface_header: required_role(Role::SurfaceHeader)?,
            surface_panel: required_role(Role::SurfacePanel)?,
            surface_row_alt: self.roles.get(&Role::SurfaceRowAlt).cloned(),
            text_primary: required_role(Role::TextPrimary)?,
            text_secondary: required_role(Role::TextSecondary)?,
            text_muted: required_role(Role::TextMuted)?,
            text_disabled: required_role(Role::TextDisabled)?,
            text_on_accent: required_role(Role::TextOnAccent)?,
            border_subtle: required_role(Role::BorderSubtle)?,
            border_default: required_role(Role::BorderDefault)?,
            border_strong: required_role(Role::BorderStrong)?,
            accent_default: required_role(Role::AccentDefault)?,
            accent_hover: required_role(Role::AccentHover)?,
            accent_pressed: required_role(Role::AccentPressed)?,
            status_success: required_role(Role::StatusSuccess)?,
            status_success_bg: required_role(Role::StatusSuccessBg)?,
            status_danger: required_role(Role::StatusDanger)?,
            status_danger_bg: required_role(Role::StatusDangerBg)?,
            status_warning: required_role(Role::StatusWarning)?,
            status_warning_bg: required_role(Role::StatusWarningBg)?,
            status_info: required_role(Role::StatusInfo)?,
            status_info_bg: required_role(Role::StatusInfoBg)?,
            selection: required_role(Role::Selection)?,
            focus_ring: required_role(Role::FocusRing)?,
            backdrop_dim: required_role(Role::BackdropDim)?,
            backdrop_full: required_role(Role::BackdropFull)?,
            shadow_default: required_role(Role::ShadowDefault)?,
        };

        let required_radius = |key: &'static str| -> Result<f32, TokenLoadError> {
            self.geometry
                .get(key)
                .copied()
                .ok_or_else(|| TokenLoadError::MissingRequired { path: format!("geometry.{key}") })
        };
        let geometry = GeometryRoles {
            radius_sm: required_radius("radius.sm")?,
            radius_md: required_radius("radius.md")?,
            radius_lg: required_radius("radius.lg")?,
        };

        let button = crate::ui::widgets::atomic::button::tokens::ButtonTokens::resolve(
            &semantic,
            &self.components,
        );
        let checkbox = crate::ui::widgets::atomic::checkbox::tokens::CheckboxTokens::resolve(
            &semantic,
            &self.components,
        );
        let chevron = crate::ui::widgets::atomic::chevron::tokens::ChevronTokens::resolve(
            &semantic,
            &self.components,
        );
        let clock = crate::ui::widgets::atomic::clock::tokens::ClockTokens::resolve(
            &semantic,
            &self.components,
        );
        let close_button = crate::ui::widgets::atomic::close_button::tokens::CloseButtonTokens::resolve(
            &semantic,
            &self.components,
        );

        Ok(Tokens {
            semantic,
            geometry,
            name: self.name.clone(),
            app: self.app.clone(),
            components: self.components.clone(),
            button,
            checkbox,
            chevron,
            clock,
            close_button,
        })
    }
}

/// The fully validated, widget-consumable result of [`TokenSet::resolve`].
/// `name` replaces `StyleManager::active_preset` (H1 §3) — `None` for a
/// token set loaded without a `$name` metadata key. Per-widget component
/// token fields (`button`, growing by one per converted widget) stay
/// private — the only public read path is through a widget's own theme
/// trait, implemented by [`crate::tokens::theme::TokenTheme`], never a
/// generic stringly getter.
#[derive(Clone, Debug)]
pub struct Tokens {
    pub semantic: SemanticRoles,
    pub geometry: GeometryRoles,
    pub name: Option<String>,
    pub app: AppTokens,
    pub components: ComponentOverrides,
    button: crate::ui::widgets::atomic::button::tokens::ButtonTokens,
    checkbox: crate::ui::widgets::atomic::checkbox::tokens::CheckboxTokens,
    chevron: crate::ui::widgets::atomic::chevron::tokens::ChevronTokens,
    clock: crate::ui::widgets::atomic::clock::tokens::ClockTokens,
    close_button: crate::ui::widgets::atomic::close_button::tokens::CloseButtonTokens,
}

impl Tokens {
    /// This widget's resolved component tokens (H1 §3). `pub(crate)` — read
    /// through `ButtonTheme`'s trait methods via
    /// [`crate::tokens::theme::TokenTheme`], not directly.
    pub(crate) fn button(&self) -> &crate::ui::widgets::atomic::button::tokens::ButtonTokens {
        &self.button
    }

    /// This widget's resolved component tokens (H1 §3). `pub(crate)` — read
    /// through `CheckboxTheme`'s trait methods via
    /// [`crate::tokens::theme::TokenTheme`], not directly.
    pub(crate) fn checkbox(&self) -> &crate::ui::widgets::atomic::checkbox::tokens::CheckboxTokens {
        &self.checkbox
    }

    /// This widget's resolved component tokens (H1 §3). `pub(crate)` — read
    /// through `ChevronTheme`'s trait methods via
    /// [`crate::tokens::theme::TokenTheme`], not directly.
    pub(crate) fn chevron(&self) -> &crate::ui::widgets::atomic::chevron::tokens::ChevronTokens {
        &self.chevron
    }

    /// This widget's resolved component tokens (H1 §3). `pub(crate)` — read
    /// through `ClockTheme`'s trait methods via
    /// [`crate::tokens::theme::TokenTheme`], not directly.
    pub(crate) fn clock(&self) -> &crate::ui::widgets::atomic::clock::tokens::ClockTokens {
        &self.clock
    }

    /// This widget's resolved component tokens (H1 §3). `pub(crate)` — read
    /// through `CloseButtonTheme`'s trait methods via
    /// [`crate::tokens::theme::TokenTheme`], not directly.
    pub(crate) fn close_button(&self) -> &crate::ui::widgets::atomic::close_button::tokens::CloseButtonTokens {
        &self.close_button
    }
}

/// A resolved `app.*` value — MLC's escape hatch namespace (D4). uzor
/// resolves aliases/modifiers into one of these three kinds but never
/// interprets the path itself.
#[derive(Clone, Debug, PartialEq)]
pub enum TokenValue {
    Color(ColorValue),
    /// A `$type: "dimension"` numeric value (e.g. a spacing/size token).
    Dimension(f32),
    /// A plain `$type: "number"` (or type-less) numeric value.
    Number(f64),
}

impl TokenValue {
    fn kind(&self) -> &'static str {
        match self {
            TokenValue::Color(_) => "color",
            TokenValue::Dimension(_) => "dimension",
            TokenValue::Number(_) => "number",
        }
    }
}

/// The resolved `app.*` namespace: sub-path (with the leading `app.`
/// stripped, e.g. `"chart.candle_up"`) to its resolved [`TokenValue`].
/// Typed getters name the offending path on a miss — no
/// `serde_json::Value` anywhere in this type (coordinator amendment A1).
#[derive(Clone, Debug, Default)]
pub struct AppTokens {
    pub(crate) values: HashMap<String, TokenValue>,
}

impl AppTokens {
    /// Looks up `path` (without the `app.` prefix) expecting a colour.
    pub fn color(&self, path: &str) -> Result<&ColorValue, TokenLoadError> {
        match self.values.get(path) {
            Some(TokenValue::Color(c)) => Ok(c),
            Some(other) => Err(TokenLoadError::WrongType {
                path: format!("app.{path}"),
                expected: "color",
                found: other.kind().to_string(),
            }),
            None => Err(TokenLoadError::AppTokenMissing { path: format!("app.{path}") }),
        }
    }

    /// Looks up `path` (without the `app.` prefix) expecting a dimension.
    pub fn dimension(&self, path: &str) -> Result<f32, TokenLoadError> {
        match self.values.get(path) {
            Some(TokenValue::Dimension(d)) => Ok(*d),
            Some(other) => Err(TokenLoadError::WrongType {
                path: format!("app.{path}"),
                expected: "dimension",
                found: other.kind().to_string(),
            }),
            None => Err(TokenLoadError::AppTokenMissing { path: format!("app.{path}") }),
        }
    }

    /// Looks up `path` (without the `app.` prefix) expecting a plain number.
    pub fn number(&self, path: &str) -> Result<f64, TokenLoadError> {
        match self.values.get(path) {
            Some(TokenValue::Number(n)) => Ok(*n),
            Some(other) => Err(TokenLoadError::WrongType {
                path: format!("app.{path}"),
                expected: "number",
                found: other.kind().to_string(),
            }),
            None => Err(TokenLoadError::AppTokenMissing { path: format!("app.{path}") }),
        }
    }
}

/// An override value as authored for one `component.<widget>.<key>` entry:
/// either a semantic-role alias (with an optional [`Modifier`]) or a
/// hardcoded literal colour — the same two shapes the DTCG `$value`/
/// `$extensions` pair can express (H1 §3). Stays symbolic (not resolved to
/// a final [`ColorValue`]) until a widget's own default-alias table exists
/// to resolve it against ([`Self::resolve`], consumed from H1 Brief 4).
#[derive(Clone, Debug, PartialEq)]
pub enum ColorSpec {
    Alias(Role, Option<Modifier>),
    Literal(ColorValue),
}

/// A modifier applied to an aliased [`ColorSpec`] once its target role is
/// known.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Modifier {
    Alpha(f32),
    Mix(Role, f32),
}

impl ColorSpec {
    /// Resolves this override against a fully-resolved semantic-role set.
    pub fn resolve(&self, roles: &SemanticRoles) -> ColorValue {
        match self {
            ColorSpec::Literal(v) => v.clone(),
            ColorSpec::Alias(role, modifier) => {
                let base = role_value(roles, *role);
                match modifier {
                    Some(Modifier::Alpha(a)) => base.with_alpha(*a),
                    Some(Modifier::Mix(other, t)) => base.mix(&role_value(roles, *other), *t),
                    None => base,
                }
            }
        }
    }
}

/// [`Role`] to [`SemanticRoles`] field lookup used only by [`ColorSpec`]
/// resolution. Deliberately not a method on `SemanticRoles` itself — that
/// type's own doc comment fixes it as never exposing a runtime `get(Role)`
/// lookup surface (H1 Brief 1); callers of `SemanticRoles` still always use
/// the named field directly.
fn role_value(roles: &SemanticRoles, role: Role) -> ColorValue {
    match role {
        Role::SurfaceAppChrome => roles.surface_app_chrome.clone(),
        Role::SurfaceControlIdle => roles.surface_control_idle.clone(),
        Role::SurfaceControlHover => roles.surface_control_hover.clone(),
        Role::SurfaceControlActive => roles.surface_control_active.clone(),
        Role::SurfaceFloating => roles.surface_floating.clone(),
        Role::SurfaceHeader => roles.surface_header.clone(),
        Role::SurfacePanel => roles.surface_panel.clone(),
        Role::SurfaceRowAlt => roles.surface_row_alt.clone().unwrap_or(ColorValue::Transparent),
        Role::TextPrimary => roles.text_primary.clone(),
        Role::TextSecondary => roles.text_secondary.clone(),
        Role::TextMuted => roles.text_muted.clone(),
        Role::TextDisabled => roles.text_disabled.clone(),
        Role::TextOnAccent => roles.text_on_accent.clone(),
        Role::BorderSubtle => roles.border_subtle.clone(),
        Role::BorderDefault => roles.border_default.clone(),
        Role::BorderStrong => roles.border_strong.clone(),
        Role::AccentDefault => roles.accent_default.clone(),
        Role::AccentHover => roles.accent_hover.clone(),
        Role::AccentPressed => roles.accent_pressed.clone(),
        Role::StatusSuccess => roles.status_success.clone(),
        Role::StatusSuccessBg => roles.status_success_bg.clone(),
        Role::StatusDanger => roles.status_danger.clone(),
        Role::StatusDangerBg => roles.status_danger_bg.clone(),
        Role::StatusWarning => roles.status_warning.clone(),
        Role::StatusWarningBg => roles.status_warning_bg.clone(),
        Role::StatusInfo => roles.status_info.clone(),
        Role::StatusInfoBg => roles.status_info_bg.clone(),
        Role::Selection => roles.selection.clone(),
        Role::FocusRing => roles.focus_ring.clone(),
        Role::BackdropDim => roles.backdrop_dim.clone(),
        Role::BackdropFull => roles.backdrop_full.clone(),
        Role::ShadowDefault => roles.shadow_default.clone(),
    }
}

/// `component.*` overrides from one loaded token file: widget name to its
/// key-to-[`ColorSpec`] map. Only ever contains entries for keys a widget
/// has registered via [`crate::tokens::component_keys`] — the loader
/// rejects anything else as [`TokenLoadError::UnknownPath`], so this type
/// never needs to represent a rejected/unknown entry.
#[derive(Clone, Debug, Default)]
pub struct ComponentOverrides {
    pub(crate) widgets: HashMap<String, HashMap<String, ColorSpec>>,
}

impl ComponentOverrides {
    /// The override authored for `widget`'s `key` component token, if the
    /// file defined one.
    pub fn get(&self, widget: &str, key: &str) -> Option<&ColorSpec> {
        self.widgets.get(widget)?.get(key)
    }
}
