//! [`load_token_set`]: turns a raw, DTCG-shaped JSON file
//! ([`crate::tokens::dtcg::TokenFileRaw`]) into a [`TokenSet`] — alias
//! references followed by depth-first search with cycle detection,
//! modifiers applied once the alias chain bottoms out, `component.*`
//! entries checked against each widget's own registered key list. See the
//! H1 token contract design doc §2 for the resolution order this module
//! implements.

use std::collections::{BTreeMap, HashMap};

use crate::tokens::color::ColorValue;
use crate::tokens::dtcg::{ModifierRaw, RawValue, TokenFileRaw, TokenLeafRaw, TokenLoadError};
use crate::tokens::geometry::GEOMETRY_KEYS;
use crate::tokens::semantic::Role;
use crate::tokens::set::{AppTokens, ColorSpec, ComponentOverrides, Modifier, TokenSet, TokenValue};

/// Loads and resolves a DTCG-shaped JSON token file. Aliases are followed
/// (cross-group: an `app.*` entry may alias `color.*`/`geometry.*`) and
/// modifiers are applied once the alias chain bottoms out — but the result
/// is not yet checked for completeness; call [`TokenSet::resolve`] for that.
pub fn load_token_set(json: &str) -> Result<TokenSet, TokenLoadError> {
    let file = TokenFileRaw::parse(json)?;

    for (widget, key) in file.component.keys() {
        let allowed =
            crate::tokens::component_keys(widget).map(|keys| keys.contains(&key.as_str())).unwrap_or(false);
        if !allowed {
            return Err(TokenLoadError::UnknownPath { path: format!("component.{widget}.{key}") });
        }
    }

    let mut cache: HashMap<String, ResolvedRaw> = HashMap::new();
    let mut roles: HashMap<Role, ColorValue> = HashMap::new();
    let mut geometry: HashMap<&'static str, f32> = HashMap::new();
    let mut app_values: HashMap<String, TokenValue> = HashMap::new();

    for path in file.leaves.keys() {
        if let Some(sub) = path.strip_prefix("color.") {
            let role = Role::from_path(sub).ok_or_else(|| TokenLoadError::UnknownPath { path: path.clone() })?;
            let mut visiting = Vec::new();
            match resolve_path(path, &file.leaves, &mut cache, &mut visiting)? {
                ResolvedRaw::Color(c) => {
                    roles.insert(role, c);
                }
                ResolvedRaw::Number(n) => {
                    return Err(TokenLoadError::WrongType {
                        path: path.clone(),
                        expected: "color",
                        found: format!("number ({n})"),
                    });
                }
            }
        } else if let Some(sub) = path.strip_prefix("geometry.") {
            let key = GEOMETRY_KEYS
                .iter()
                .copied()
                .find(|k| *k == sub)
                .ok_or_else(|| TokenLoadError::UnknownPath { path: path.clone() })?;
            let mut visiting = Vec::new();
            match resolve_path(path, &file.leaves, &mut cache, &mut visiting)? {
                ResolvedRaw::Number(n) => {
                    geometry.insert(key, n as f32);
                }
                ResolvedRaw::Color(_) => {
                    return Err(TokenLoadError::WrongType {
                        path: path.clone(),
                        expected: "number",
                        found: "color".to_string(),
                    });
                }
            }
        } else if let Some(sub) = path.strip_prefix("app.") {
            let mut visiting = Vec::new();
            let resolved = resolve_path(path, &file.leaves, &mut cache, &mut visiting)?;
            let leaf = &file.leaves[path];
            let value = classify_app_value(path, leaf.type_hint.as_deref(), resolved)?;
            app_values.insert(sub.to_string(), value);
        } else {
            // `TokenFileRaw::parse` only ever inserts leaves under
            // "color."/"geometry."/"app." prefixes (component entries live
            // in `file.component`, not `file.leaves`) — this arm exists so
            // adding a fourth flattened group later fails loudly here
            // instead of silently dropping its entries.
            return Err(TokenLoadError::UnknownPath { path: path.clone() });
        }
    }

    Ok(TokenSet {
        roles,
        geometry,
        app: AppTokens { values: app_values },
        components: build_component_overrides(&file.component)?,
        name: file.name,
    })
}

/// A leaf's value once alias chains are followed and modifiers applied —
/// not yet boxed into the public [`TokenValue`]/[`ColorValue`] surface,
/// since geometry numbers and `app.*` numbers share this shape during
/// resolution.
#[derive(Clone, Debug)]
enum ResolvedRaw {
    Color(ColorValue),
    Number(f64),
}

/// Depth-first alias resolution for one `color.*`/`geometry.*`/`app.*` leaf.
/// `visiting` is the current DFS stack (by path) — a path already on it
/// means the alias chain looped back on itself ([`TokenLoadError::AliasCycle`],
/// naming the whole loop). `cache` memoises already-resolved paths so a
/// value referenced by multiple aliases is only walked once.
fn resolve_path(
    path: &str,
    leaves: &BTreeMap<String, TokenLeafRaw>,
    cache: &mut HashMap<String, ResolvedRaw>,
    visiting: &mut Vec<String>,
) -> Result<ResolvedRaw, TokenLoadError> {
    if let Some(v) = cache.get(path) {
        return Ok(v.clone());
    }
    if let Some(pos) = visiting.iter().position(|p| p == path) {
        let mut cycle = visiting[pos..].to_vec();
        cycle.push(path.to_string());
        return Err(TokenLoadError::AliasCycle { path: path.to_string(), cycle: cycle.join(" -> ") });
    }
    // Every path this function is called with already exists in `leaves`:
    // top-level callers only pass paths they found by iterating
    // `leaves.keys()`, and alias/mix targets are existence-checked with
    // `BadAlias` before recursing. This lookup still names the path rather
    // than panicking if that invariant is ever broken.
    let leaf = leaves
        .get(path)
        .ok_or_else(|| TokenLoadError::BadAlias { path: path.to_string(), alias: path.to_string() })?;

    visiting.push(path.to_string());

    let base = match &leaf.value {
        RawValue::Alias(target) => {
            enforce_alias_direction(path, target)?;
            if !leaves.contains_key(target) {
                return Err(TokenLoadError::BadAlias { path: path.to_string(), alias: target.clone() });
            }
            resolve_path(target, leaves, cache, visiting)?
        }
        RawValue::String(s) => ResolvedRaw::Color(ColorValue::parse(s).map_err(|e| TokenLoadError::InvalidColor {
            path: path.to_string(),
            value: s.clone(),
            source: e,
        })?),
        RawValue::Number(n) => ResolvedRaw::Number(*n),
    };

    let resolved = match &leaf.modifier {
        None => base,
        Some(ModifierRaw::Alpha(alpha)) => match base {
            ResolvedRaw::Color(c) => ResolvedRaw::Color(c.with_alpha(*alpha)),
            ResolvedRaw::Number(_) => {
                return Err(TokenLoadError::WrongType {
                    path: path.to_string(),
                    expected: "color",
                    found: "number".to_string(),
                });
            }
        },
        Some(ModifierRaw::Mix { with, t }) => {
            enforce_alias_direction(path, with)?;
            if !leaves.contains_key(with) {
                return Err(TokenLoadError::BadAlias { path: path.to_string(), alias: with.clone() });
            }
            let mix_target = resolve_path(with, leaves, cache, visiting)?;
            match (base, mix_target) {
                (ResolvedRaw::Color(a), ResolvedRaw::Color(b)) => ResolvedRaw::Color(a.mix(&b, *t)),
                _ => {
                    return Err(TokenLoadError::WrongType {
                        path: path.to_string(),
                        expected: "color",
                        found: "number".to_string(),
                    });
                }
            }
        }
    };

    if let Some(hint) = leaf.type_hint.as_deref() {
        let found_kind = match &resolved {
            ResolvedRaw::Color(_) => "color",
            ResolvedRaw::Number(_) => "number",
        };
        let expected_static = match hint {
            "color" => "color",
            "number" => "number",
            "dimension" => "dimension",
            other => {
                return Err(TokenLoadError::WrongType {
                    path: path.to_string(),
                    expected: "color, number, or dimension",
                    found: format!("unknown $type {other:?}"),
                });
            }
        };
        let ok = match expected_static {
            "color" => found_kind == "color",
            _ => found_kind == "number",
        };
        if !ok {
            return Err(TokenLoadError::WrongType {
                path: path.to_string(),
                expected: expected_static,
                found: found_kind.to_string(),
            });
        }
    }

    visiting.pop();
    cache.insert(path.to_string(), resolved.clone());
    Ok(resolved)
}

/// `app.*` may alias `color.*`/`geometry.*`; the reverse is an error
/// (coordinator A1 follow-up: core tokens never depend on the
/// consumer-owned `app.*` namespace).
fn enforce_alias_direction(referring: &str, target: &str) -> Result<(), TokenLoadError> {
    let referring_is_core = referring.starts_with("color.") || referring.starts_with("geometry.");
    if referring_is_core && target.starts_with("app.") {
        return Err(TokenLoadError::CoreAliasIntoApp { path: referring.to_string(), alias: target.to_string() });
    }
    Ok(())
}

/// Boxes a resolved `app.*` value into the typed [`TokenValue`] the file's
/// own `$type` hint asks for (`"dimension"` vs. the type-less/`"number"`
/// default — both resolve to the same numeric [`ResolvedRaw::Number`], only
/// the declared `$type` tells them apart).
fn classify_app_value(
    path: &str,
    hint: Option<&str>,
    resolved: ResolvedRaw,
) -> Result<TokenValue, TokenLoadError> {
    match (hint, resolved) {
        (Some("color"), ResolvedRaw::Color(c)) => Ok(TokenValue::Color(c)),
        (Some("color"), ResolvedRaw::Number(n)) => Err(TokenLoadError::WrongType {
            path: path.to_string(),
            expected: "color",
            found: format!("number ({n})"),
        }),
        (Some("dimension"), ResolvedRaw::Number(n)) => Ok(TokenValue::Dimension(n as f32)),
        (Some("dimension"), ResolvedRaw::Color(_)) => Err(TokenLoadError::WrongType {
            path: path.to_string(),
            expected: "dimension",
            found: "color".to_string(),
        }),
        (Some("number"), ResolvedRaw::Number(n)) => Ok(TokenValue::Number(n)),
        (Some("number"), ResolvedRaw::Color(_)) => Err(TokenLoadError::WrongType {
            path: path.to_string(),
            expected: "number",
            found: "color".to_string(),
        }),
        (Some(other), _) => Err(TokenLoadError::WrongType {
            path: path.to_string(),
            expected: "color, number, or dimension",
            found: format!("unknown $type {other:?}"),
        }),
        (None, ResolvedRaw::Color(c)) => Ok(TokenValue::Color(c)),
        (None, ResolvedRaw::Number(n)) => Ok(TokenValue::Number(n)),
    }
}

/// Builds `component.*` overrides — one hop, not a DFS: an alias must name
/// a `color.*` role directly ([`ColorSpec::Alias`] holds a [`Role`], not a
/// path), so there is no chain to walk.
fn build_component_overrides(
    component: &BTreeMap<(String, String), TokenLeafRaw>,
) -> Result<ComponentOverrides, TokenLoadError> {
    let mut widgets: HashMap<String, HashMap<String, ColorSpec>> = HashMap::new();
    for ((widget, key), leaf) in component {
        let path = format!("component.{widget}.{key}");
        let spec = build_color_spec(&path, leaf)?;
        widgets.entry(widget.clone()).or_default().insert(key.clone(), spec);
    }
    Ok(ComponentOverrides { widgets })
}

/// Builds one [`ColorSpec`] from its raw leaf.
fn build_color_spec(path: &str, leaf: &TokenLeafRaw) -> Result<ColorSpec, TokenLoadError> {
    match &leaf.value {
        RawValue::Alias(target) => {
            let role = role_from_alias_path(path, target)?;
            let modifier = match &leaf.modifier {
                None => None,
                Some(ModifierRaw::Alpha(a)) => Some(Modifier::Alpha(*a)),
                Some(ModifierRaw::Mix { with, t }) => Some(Modifier::Mix(role_from_alias_path(path, with)?, *t)),
            };
            Ok(ColorSpec::Alias(role, modifier))
        }
        RawValue::String(s) => {
            if leaf.modifier.is_some() {
                // A literal is already the final value — a modifier only
                // makes sense applied to an aliased role (H1 §3's
                // `ColorSpec` has no "literal + pending modifier" shape).
                // Author the modified colour directly instead.
                return Err(TokenLoadError::WrongType {
                    path: path.to_string(),
                    expected: "alias (modifiers apply only to alias references)",
                    found: "literal value with a modifier".to_string(),
                });
            }
            let color = ColorValue::parse(s).map_err(|e| TokenLoadError::InvalidColor {
                path: path.to_string(),
                value: s.clone(),
                source: e,
            })?;
            Ok(ColorSpec::Literal(color))
        }
        RawValue::Number(n) => Err(TokenLoadError::WrongType {
            path: path.to_string(),
            expected: "color",
            found: format!("number ({n})"),
        }),
    }
}

/// Parses a `"color.<role path>"` alias reference into its [`Role`] — the
/// only alias shape a `component.*` entry may use (H1 §3: component tokens
/// alias a semantic role, never `geometry.*`/`app.*`/another component).
fn role_from_alias_path(referring_path: &str, alias: &str) -> Result<Role, TokenLoadError> {
    alias
        .strip_prefix("color.")
        .and_then(Role::from_path)
        .ok_or_else(|| TokenLoadError::BadAlias { path: referring_path.to_string(), alias: alias.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::color::Rgba;

    /// A minimal but complete fixture: every required `color.*`/
    /// `geometry.*` role, one alias, one alpha modifier, one mix modifier,
    /// and one typed `app.*` entry of each kind. No `component.*` section —
    /// `component_keys` is an empty registry today (H1 Brief 2 scope), so
    /// any `component.*` entry would make every fixture-based test fail
    /// with `UnknownPath`; [`fixture_with_component`] covers that path on
    /// its own.
    fn fixture() -> String {
        r##"{
                "$name": "test-dark",
                "color": {
                    "surface": {
                        "app_chrome": { "$type": "color", "$value": "#131722" },
                        "control": {
                            "idle": { "$type": "color", "$value": "#1e222d" },
                            "hover": { "$type": "color", "$value": "#2a2e39" },
                            "active": { "$type": "color", "$value": "#2962ff" }
                        },
                        "floating": { "$type": "color", "$value": "#1e222d" },
                        "header": { "$type": "color", "$value": "{color.surface.floating}" },
                        "panel": { "$type": "color", "$value": "{color.surface.floating}" }
                    },
                    "text": {
                        "primary": { "$type": "color", "$value": "#d1d4dc" },
                        "secondary": { "$type": "color", "$value": "#b2b5be" },
                        "muted": { "$type": "color", "$value": "#787b86" },
                        "disabled": { "$type": "color", "$value": "#6a6d78" },
                        "on_accent": { "$type": "color", "$value": "{color.text.primary}" }
                    },
                    "border": {
                        "subtle": { "$type": "color", "$value": "#2a2e39" },
                        "default": { "$type": "color", "$value": "#363a45" },
                        "strong": { "$type": "color", "$value": "{color.border.default}" }
                    },
                    "accent": {
                        "default": { "$type": "color", "$value": "#2962ff" },
                        "hover": { "$type": "color", "$value": "#1e53e4" },
                        "pressed": { "$type": "color", "$value": "{color.accent.hover}" }
                    },
                    "status": {
                        "success": { "$type": "color", "$value": "#26a69a" },
                        "success_bg": {
                            "$type": "color",
                            "$value": "{color.status.success}",
                            "$extensions": { "uzor.alpha": 0.15 }
                        },
                        "danger": { "$type": "color", "$value": "#f23645" },
                        "danger_bg": {
                            "$type": "color",
                            "$value": "{color.status.danger}",
                            "$extensions": { "uzor.alpha": 0.15 }
                        },
                        "warning": { "$type": "color", "$value": "#ff9800" },
                        "warning_bg": {
                            "$type": "color",
                            "$value": "{color.status.warning}",
                            "$extensions": { "uzor.alpha": 0.15 }
                        },
                        "info": { "$type": "color", "$value": "{color.accent.default}" },
                        "info_bg": {
                            "$type": "color",
                            "$value": "{color.status.info}",
                            "$extensions": { "uzor.alpha": 0.15 }
                        }
                    },
                    "selection": { "$type": "color", "$value": "#2962ff55" },
                    "focus_ring": { "$type": "color", "$value": "{color.accent.default}" },
                    "backdrop": {
                        "dim": { "$type": "color", "$value": "rgba(0,0,0,0.45)" },
                        "full": { "$type": "color", "$value": "{color.surface.app_chrome}" }
                    },
                    "shadow": {
                        "default": {
                            "$type": "color",
                            "$value": "{color.status.danger}",
                            "$extensions": { "uzor.mix": { "with": "{color.surface.app_chrome}", "t": 0.5 } }
                        }
                    }
                },
                "geometry": {
                    "radius": {
                        "sm": { "$type": "dimension", "$value": 2.0 },
                        "md": { "$type": "dimension", "$value": 4.0 },
                        "lg": { "$type": "dimension", "$value": 8.0 }
                    }
                },
                "app": {
                    "chart": {
                        "candle_up": { "$type": "color", "$value": "#26a69a" },
                        "gap": { "$type": "dimension", "$value": 3.0 },
                        "z_index": { "$type": "number", "$value": 42.0 },
                        "accent_alias": { "$type": "color", "$value": "{color.accent.default}" }
                    }
                }
            }"##
        .to_string()
    }

    /// [`fixture`] plus one `component.<widget>.<key>` entry — used only by
    /// the "unregistered widget" test, since `component_keys` is an empty
    /// registry today and would reject it in every other test.
    fn fixture_with_component(widget: &str, key: &str) -> String {
        let mut value: serde_json::Value =
            serde_json::from_str(&fixture()).expect("fixture() must itself be valid JSON");
        let component = serde_json::json!({
            widget: {
                key: { "$type": "color", "$value": "{color.surface.control.hover}" }
            }
        });
        value
            .as_object_mut()
            .expect("fixture() must parse to a JSON object")
            .insert("component".to_string(), component);
        value.to_string()
    }

    #[test]
    fn parses_and_resolves_the_fixture() {
        let json = fixture();
        let set = load_token_set(&json).unwrap_or_else(|e| panic!("load failed: {e}"));
        let tokens = set.resolve().unwrap_or_else(|e| panic!("resolve failed: {e}"));
        assert_eq!(tokens.name.as_deref(), Some("test-dark"));
        assert_eq!(tokens.semantic.surface_app_chrome, ColorValue::Solid(Rgba::from_hex("#131722").unwrap()));
        assert_eq!(tokens.geometry.radius_md, 4.0);
    }

    #[test]
    fn alias_resolves_across_groups() {
        let json = fixture();
        let tokens = load_token_set(&json).unwrap().resolve().unwrap();
        // surface.header aliases surface.floating.
        assert_eq!(tokens.semantic.surface_header, tokens.semantic.surface_floating);
        // status.info aliases accent.default, and app.chart.accent_alias
        // aliases color.accent.default too — both must agree.
        assert_eq!(tokens.semantic.status_info, tokens.semantic.accent_default);
        assert_eq!(tokens.app.color("chart.accent_alias").unwrap(), &tokens.semantic.accent_default);
    }

    #[test]
    fn alpha_and_mix_modifiers_apply_after_alias_resolution() {
        let json = fixture();
        let tokens = load_token_set(&json).unwrap().resolve().unwrap();
        let danger = tokens.semantic.status_danger.clone();
        assert_eq!(tokens.semantic.status_danger_bg, danger.with_alpha(0.15));
        let chrome = tokens.semantic.surface_app_chrome.clone();
        assert_eq!(tokens.semantic.shadow_default, danger.mix(&chrome, 0.5));
    }

    #[test]
    fn alias_cycle_is_caught_and_names_the_full_loop() {
        let json = r##"{
            "color": {
                "surface": {
                    "app_chrome": { "$type": "color", "$value": "{color.surface.floating}" },
                    "control": {
                        "idle": { "$type": "color", "$value": "#1e222d" },
                        "hover": { "$type": "color", "$value": "#2a2e39" },
                        "active": { "$type": "color", "$value": "#2962ff" }
                    },
                    "floating": { "$type": "color", "$value": "{color.surface.app_chrome}" },
                    "header": { "$type": "color", "$value": "#1e222d" },
                    "panel": { "$type": "color", "$value": "#1e222d" }
                }
            }
        }"##;
        let err = load_token_set(json).unwrap_err();
        match err {
            TokenLoadError::AliasCycle { cycle, .. } => {
                assert!(cycle.contains("color.surface.app_chrome"));
                assert!(cycle.contains("color.surface.floating"));
            }
            other => panic!("expected AliasCycle, got {other}"),
        }
    }

    #[test]
    fn unknown_top_level_path_is_named() {
        let json = r##"{ "bogus": { "x": { "$type": "color", "$value": "#000000" } } }"##;
        let err = load_token_set(json).unwrap_err();
        match err {
            TokenLoadError::UnknownPath { path } => assert_eq!(path, "bogus"),
            other => panic!("expected UnknownPath, got {other}"),
        }
    }

    #[test]
    fn unknown_role_path_is_named() {
        let json = r##"{
            "color": {
                "surface": {
                    "bogus": { "$type": "color", "$value": "#000000" }
                }
            }
        }"##;
        let err = load_token_set(json).unwrap_err();
        match err {
            TokenLoadError::UnknownPath { path } => assert_eq!(path, "color.surface.bogus"),
            other => panic!("expected UnknownPath, got {other}"),
        }
    }

    #[test]
    fn wrong_type_is_caught_and_named() {
        let json = r##"{
            "app": {
                "test": { "bogus": { "$type": "color", "$value": 4.0 } }
            }
        }"##;
        let err = load_token_set(json).unwrap_err();
        match err {
            TokenLoadError::WrongType { path, expected, .. } => {
                assert_eq!(path, "app.test.bogus");
                assert_eq!(expected, "color");
            }
            other => panic!("expected WrongType, got {other}"),
        }
    }

    #[test]
    fn missing_required_role_is_named() {
        let json = r##"{
            "color": {
                "surface": {
                    "app_chrome": { "$type": "color", "$value": "#131722" }
                }
            }
        }"##;
        let set = load_token_set(json).unwrap_or_else(|e| panic!("load failed: {e}"));
        let err = set.resolve().unwrap_err();
        match err {
            TokenLoadError::MissingRequired { path } => assert!(path.starts_with("color.")),
            other => panic!("expected MissingRequired, got {other}"),
        }
    }

    #[test]
    fn surface_row_alt_stays_optional_when_absent() {
        let json = fixture();
        let tokens = load_token_set(&json).unwrap().resolve().unwrap();
        assert_eq!(tokens.semantic.surface_row_alt, None);
    }

    #[test]
    fn core_token_may_not_alias_app() {
        let json = r##"{
            "color": {
                "surface": {
                    "app_chrome": { "$type": "color", "$value": "{app.chart.candle_up}" },
                    "control": {
                        "idle": { "$type": "color", "$value": "#1e222d" },
                        "hover": { "$type": "color", "$value": "#2a2e39" },
                        "active": { "$type": "color", "$value": "#2962ff" }
                    },
                    "floating": { "$type": "color", "$value": "#1e222d" },
                    "header": { "$type": "color", "$value": "#1e222d" },
                    "panel": { "$type": "color", "$value": "#1e222d" }
                }
            },
            "app": {
                "chart": { "candle_up": { "$type": "color", "$value": "#26a69a" } }
            }
        }"##;
        let err = load_token_set(json).unwrap_err();
        assert!(matches!(err, TokenLoadError::CoreAliasIntoApp { .. }), "got {err}");
    }

    #[test]
    fn app_may_alias_color() {
        let json = fixture();
        let tokens = load_token_set(&json).unwrap().resolve().unwrap();
        assert_eq!(tokens.app.color("chart.candle_up").unwrap(), &tokens.semantic.status_success);
    }

    #[test]
    fn app_getters_hit_missing_and_mistyped() {
        let json = fixture();
        let tokens = load_token_set(&json).unwrap().resolve().unwrap();
        assert!(tokens.app.color("chart.candle_up").is_ok());
        assert_eq!(tokens.app.dimension("chart.gap").unwrap(), 3.0);
        assert_eq!(tokens.app.number("chart.z_index").unwrap(), 42.0);

        match tokens.app.color("chart.nonexistent").unwrap_err() {
            TokenLoadError::AppTokenMissing { path } => assert_eq!(path, "app.chart.nonexistent"),
            other => panic!("expected AppTokenMissing, got {other}"),
        }
        match tokens.app.number("chart.candle_up").unwrap_err() {
            TokenLoadError::WrongType { path, expected, .. } => {
                assert_eq!(path, "app.chart.candle_up");
                assert_eq!(expected, "number");
            }
            other => panic!("expected WrongType, got {other}"),
        }
    }

    #[test]
    fn component_entry_for_an_unregistered_widget_is_rejected() {
        let json = fixture_with_component("button", "bg_hover");
        let err = load_token_set(&json).unwrap_err();
        match err {
            TokenLoadError::UnknownPath { path } => assert_eq!(path, "component.button.bg_hover"),
            other => panic!("expected UnknownPath, got {other}"),
        }
    }
}
