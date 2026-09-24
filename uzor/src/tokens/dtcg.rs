//! Raw DTCG (Design Tokens Community Group) file parsing: the JSON shape
//! before alias resolution — `$value`/`$type` leaves, `{alias}` references,
//! and uzor's own modifier extensions (`$extensions."uzor.alpha"` /
//! `"uzor.mix"`). See the H1 token contract design doc §2 for the full file
//! shape and the loader's resolution order.
//!
//! This module only understands the file's *shape*: which top-level groups
//! exist (`color`, `geometry`, `component`, `app`), which nodes are groups
//! vs. leaves, and how a leaf's `$value`/`$type`/`$extensions` parse into
//! [`RawValue`]/[`ModifierRaw`]. It does NOT resolve aliases or apply
//! modifiers — that DFS lives in [`crate::tokens::loader`], which consumes
//! [`TokenFileRaw`]'s flattened, dotted-path maps.

use std::collections::BTreeMap;

/// Every way uzor's token loader can reject a file. Every variant names the
/// offending path (and, where relevant, the offending value) — the house
/// rule that a rejection names its inputs.
#[derive(Debug, thiserror::Error)]
pub enum TokenLoadError {
    /// The input was not valid JSON at all.
    #[error("invalid JSON: {0}")]
    Parse(#[from] serde_json::Error),

    /// A path exists in the file but matches no semantic role, geometry
    /// role, registered component key, or accepted structural shape.
    /// Covers: an unrecognised top-level group name, a `color.*`/
    /// `geometry.*` leaf whose sub-path isn't a known role, and a
    /// `component.<widget>.<key>` entry whose widget hasn't registered that
    /// key (or hasn't registered at all — see
    /// [`crate::tokens::component_keys`]).
    #[error("{path}: unknown token path")]
    UnknownPath { path: String },

    /// `{alias}` does not name any token actually present in the file.
    #[error("{path}: alias {alias:?} does not resolve to any known token")]
    BadAlias { path: String, alias: String },

    /// Following `{alias}` references from `path` eventually loops back to
    /// itself. `cycle` lists every path in the loop, in visit order.
    #[error("{path}: alias cycle: {cycle}")]
    AliasCycle { path: String, cycle: String },

    /// A value resolved to a different kind than its declared `$type` (or
    /// than the shape its own group requires — e.g. `color.*` must resolve
    /// to a colour, `geometry.*` to a number).
    #[error("{path}: expected {expected}, found {found}")]
    WrongType { path: String, expected: &'static str, found: String },

    /// A literal `$value` string failed colour parsing.
    #[error("{path}: invalid colour value {value:?}: {source}")]
    InvalidColor {
        path: String,
        value: String,
        #[source]
        source: crate::tokens::color::ColorParseError,
    },

    /// The token set omits a role/geometry field every token set must
    /// define (`surface.row_alt` is the one optional semantic role).
    #[error("{path}: token set does not define this required token")]
    MissingRequired { path: String },

    /// A `color.*`/`geometry.*` token aliased into `app.*`. Direction rule
    /// (H1 Brief 2, coordinator A1 follow-up): `app.*` may alias
    /// `color.*`/`geometry.*`, never the reverse.
    #[error("{path}: color/geometry tokens must not alias app.* (found {alias:?})")]
    CoreAliasIntoApp { path: String, alias: String },

    /// [`crate::tokens::set::AppTokens`] getter miss: no value was resolved
    /// at this `app.*` sub-path.
    #[error("{path}: no app token at this path")]
    AppTokenMissing { path: String },
}

/// One node in the raw token tree, before we know whether it (or its
/// descendants) are aliases: a leaf (`$value`/`$type`/`$extensions`) or a
/// nested group of further nodes.
#[derive(Debug, Clone)]
pub(crate) enum TokenNodeRaw {
    Group(BTreeMap<String, TokenNodeRaw>),
    Leaf(TokenLeafRaw),
}

/// A single `$value`/`$type`/`$extensions` leaf, before alias resolution.
#[derive(Debug, Clone)]
pub(crate) struct TokenLeafRaw {
    pub value: RawValue,
    pub type_hint: Option<String>,
    pub modifier: Option<ModifierRaw>,
}

/// The `$value` payload, before we know whether it is an alias.
#[derive(Debug, Clone)]
pub(crate) enum RawValue {
    /// `"{a.b.c}"` — the braces stripped, `"a.b.c"` kept as the raw
    /// referenced path.
    Alias(String),
    /// Any other JSON string, e.g. `"#131722"` or `"transparent"`.
    String(String),
    /// A raw JSON number, e.g. `4.0`.
    Number(f64),
}

/// A parsed `$extensions` modifier.
#[derive(Debug, Clone)]
pub(crate) enum ModifierRaw {
    Alpha(f32),
    /// `with` is the referenced path with its braces already stripped, same
    /// as [`RawValue::Alias`].
    Mix { with: String, t: f32 },
}

/// The whole parsed file: `color.*`/`geometry.*`/`app.*` leaves flattened
/// into one dotted-path map (alias references cross these three groups
/// freely, so one shared map is what the loader's DFS walks), plus
/// `component.<widget>.<key>` entries kept separately since they resolve
/// without a DFS (§2, `ColorSpec` is one hop, not a chain).
#[derive(Debug, Clone, Default)]
pub(crate) struct TokenFileRaw {
    pub name: Option<String>,
    pub leaves: BTreeMap<String, TokenLeafRaw>,
    pub component: BTreeMap<(String, String), TokenLeafRaw>,
}

impl TokenFileRaw {
    /// Parses a DTCG-shaped JSON token file. Structural validation only —
    /// alias references are recorded as-is ([`RawValue::Alias`]), not yet
    /// resolved or checked for existence.
    pub(crate) fn parse(json: &str) -> Result<Self, TokenLoadError> {
        let value: serde_json::Value = serde_json::from_str(json)?;
        let root = value.as_object().ok_or_else(|| TokenLoadError::WrongType {
            path: "<root>".to_string(),
            expected: "object",
            found: describe_kind(&value),
        })?;

        let mut name = None;
        let mut leaves = BTreeMap::new();
        let mut component = BTreeMap::new();

        for (key, val) in root {
            if let Some(meta) = key.strip_prefix('$') {
                if meta == "name" {
                    let s = val.as_str().ok_or_else(|| TokenLoadError::WrongType {
                        path: "$name".to_string(),
                        expected: "string",
                        found: describe_kind(val),
                    })?;
                    name = Some(s.to_string());
                }
                // Other `$`-prefixed top-level keys are DTCG metadata this
                // loader doesn't use yet — spec-legal, silently ignored,
                // same tolerance as an unrecognised `$extensions` key.
                continue;
            }
            match key.as_str() {
                "color" => flatten("color", node_from_json("color", val)?, &mut leaves),
                "geometry" => flatten("geometry", node_from_json("geometry", val)?, &mut leaves),
                "app" => flatten("app", node_from_json("app", val)?, &mut leaves),
                "component" => parse_component(val, &mut component)?,
                _ => return Err(TokenLoadError::UnknownPath { path: key.clone() }),
            }
        }

        Ok(Self { name, leaves, component })
    }
}

/// Classifies one JSON value as a leaf (has `$value`) or a group (any other
/// object), recursing into groups. `path` is the full dotted path so far,
/// used only for error messages here — [`flatten`] reconstructs the same
/// paths from the returned tree's own key structure.
fn node_from_json(path: &str, value: &serde_json::Value) -> Result<TokenNodeRaw, TokenLoadError> {
    let obj = value.as_object().ok_or_else(|| TokenLoadError::WrongType {
        path: path.to_string(),
        expected: "object",
        found: describe_kind(value),
    })?;
    if obj.contains_key("$value") {
        Ok(TokenNodeRaw::Leaf(parse_leaf_obj(path, obj)?))
    } else {
        let mut children = BTreeMap::new();
        for (key, val) in obj {
            let child_path = format!("{path}.{key}");
            children.insert(key.clone(), node_from_json(&child_path, val)?);
        }
        Ok(TokenNodeRaw::Group(children))
    }
}

/// Walks a [`TokenNodeRaw`] tree, inserting every leaf into `out` keyed by
/// its full dotted path (`prefix` plus each group segment on the way down).
fn flatten(prefix: &str, node: TokenNodeRaw, out: &mut BTreeMap<String, TokenLeafRaw>) {
    match node {
        TokenNodeRaw::Leaf(leaf) => {
            out.insert(prefix.to_string(), leaf);
        }
        TokenNodeRaw::Group(children) => {
            for (key, child) in children {
                let path = format!("{prefix}.{key}");
                flatten(&path, child, out);
            }
        }
    }
}

/// Parses the `component` top-level group: exactly two levels
/// (`component.<widget>.<key>`), each `<key>` a leaf. Registry validation
/// (does `<widget>` even exist, does it declare `<key>`) happens later in
/// [`crate::tokens::loader`] — this function only checks the JSON shape.
fn parse_component(
    value: &serde_json::Value,
    out: &mut BTreeMap<(String, String), TokenLeafRaw>,
) -> Result<(), TokenLoadError> {
    let widgets = value.as_object().ok_or_else(|| TokenLoadError::WrongType {
        path: "component".to_string(),
        expected: "object",
        found: describe_kind(value),
    })?;
    for (widget, keys_value) in widgets {
        let keys = keys_value
            .as_object()
            .ok_or_else(|| TokenLoadError::UnknownPath { path: format!("component.{widget}") })?;
        for (key, leaf_value) in keys {
            let path = format!("component.{widget}.{key}");
            let obj = leaf_value
                .as_object()
                .filter(|o| o.contains_key("$value"))
                .ok_or_else(|| TokenLoadError::UnknownPath { path: path.clone() })?;
            let leaf = parse_leaf_obj(&path, obj)?;
            out.insert((widget.clone(), key.clone()), leaf);
        }
    }
    Ok(())
}

/// Parses one `{ "$value": ..., "$type": ..., "$extensions": {...} }`
/// object into a [`TokenLeafRaw`].
fn parse_leaf_obj(
    path: &str,
    obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<TokenLeafRaw, TokenLoadError> {
    // `obj` is only ever handed to this function after a caller has already
    // confirmed `$value` is present (`node_from_json`, `parse_component`),
    // so this lookup always succeeds; the fallback still names the path
    // rather than panicking if that invariant ever changes.
    let value_field = obj
        .get("$value")
        .ok_or_else(|| TokenLoadError::UnknownPath { path: path.to_string() })?;
    let type_hint = match obj.get("$type") {
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        Some(other) => {
            return Err(TokenLoadError::WrongType {
                path: path.to_string(),
                expected: "string",
                found: describe_kind(other),
            });
        }
        None => None,
    };
    let value = match value_field {
        serde_json::Value::String(s) => match s.strip_prefix('{').and_then(|r| r.strip_suffix('}')) {
            Some(inner) => RawValue::Alias(inner.to_string()),
            None => RawValue::String(s.clone()),
        },
        serde_json::Value::Number(n) => {
            let n = n.as_f64().ok_or_else(|| TokenLoadError::WrongType {
                path: path.to_string(),
                expected: "finite number",
                found: n.to_string(),
            })?;
            RawValue::Number(n)
        }
        other => {
            return Err(TokenLoadError::WrongType {
                path: path.to_string(),
                expected: "string or number",
                found: describe_kind(other),
            });
        }
    };
    let modifier = match obj.get("$extensions") {
        Some(serde_json::Value::Object(ext)) => parse_modifier(path, ext)?,
        Some(other) => {
            return Err(TokenLoadError::WrongType {
                path: path.to_string(),
                expected: "object",
                found: describe_kind(other),
            });
        }
        None => None,
    };
    Ok(TokenLeafRaw { value, type_hint, modifier })
}

/// Parses `$extensions."uzor.alpha"` / `$extensions."uzor.mix"`. At most one
/// is expected per leaf; if both are present, `uzor.alpha` wins (a file
/// authoring both on one leaf is unusual enough that silently preferring
/// one is acceptable — neither is silently dropped, both are spec-legal
/// `$extensions` entries).
fn parse_modifier(
    path: &str,
    ext: &serde_json::Map<String, serde_json::Value>,
) -> Result<Option<ModifierRaw>, TokenLoadError> {
    if let Some(alpha) = ext.get("uzor.alpha") {
        let a = alpha.as_f64().ok_or_else(|| TokenLoadError::WrongType {
            path: path.to_string(),
            expected: "number",
            found: describe_kind(alpha),
        })?;
        return Ok(Some(ModifierRaw::Alpha(a as f32)));
    }
    if let Some(mix) = ext.get("uzor.mix") {
        let obj = mix.as_object().ok_or_else(|| TokenLoadError::WrongType {
            path: path.to_string(),
            expected: "object",
            found: describe_kind(mix),
        })?;
        let with_raw = obj.get("with").and_then(|v| v.as_str()).ok_or_else(|| TokenLoadError::WrongType {
            path: path.to_string(),
            expected: "string 'with'",
            found: "missing 'with'".to_string(),
        })?;
        let with = with_raw
            .strip_prefix('{')
            .and_then(|r| r.strip_suffix('}'))
            .unwrap_or(with_raw)
            .to_string();
        let t = obj.get("t").and_then(|v| v.as_f64()).ok_or_else(|| TokenLoadError::WrongType {
            path: path.to_string(),
            expected: "number 't'",
            found: "missing 't'".to_string(),
        })?;
        return Ok(Some(ModifierRaw::Mix { with, t: t as f32 }));
    }
    Ok(None)
}

/// Human-readable JSON value kind, for `WrongType`/`UnknownPath` messages.
fn describe_kind(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(_) => "boolean".to_string(),
        serde_json::Value::Number(n) => format!("number ({n})"),
        serde_json::Value::String(s) => format!("string {s:?}"),
        serde_json::Value::Array(_) => "array".to_string(),
        serde_json::Value::Object(_) => "object".to_string(),
    }
}
