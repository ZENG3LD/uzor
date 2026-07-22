//! Typed parse/serialize failures for [`crate::spec::parse_spec`]/
//! [`crate::spec::spec_to_json`] — never a panic on malformed input.

use std::fmt;

/// Parsing/serialization failures for this crate's spec-IR — every
/// variant is a real, distinguishable failure mode (never a panic, never
/// a single opaque string).
#[derive(Debug)]
pub enum RegistryError {
    /// The input wasn't even syntactically valid JSON.
    InvalidJson(serde_json::Error),
    /// The input parsed as JSON but its top-level value isn't a JSON
    /// object (e.g. a bare array or string) — [`crate::spec::FigureSpec`]'s
    /// own wire shape is always `{"kind": ..., "data": ...}`.
    NotAnObject,
    /// The top-level object has no `"kind"` string field.
    MissingKind,
    /// `"kind"` is present but not one of this registry's known figure
    /// kinds — the ONE string-keyed dispatch level this crate allows
    /// (owner doctrine); carries the unrecognized value verbatim.
    UnknownKind(String),
    /// `"kind"` is recognized but the object has no `"data"` field.
    /// Carries the recognized `kind` string.
    MissingData(String),
    /// `"kind"`/`"data"` are both present, but `data` doesn't match that
    /// kind's own typed field shape (wrong type, missing required field,
    /// etc.) — carries the `kind` string plus `serde_json`'s own message.
    MalformedField { kind: String, message: String },
    /// [`crate::spec::spec_to_json`]'s own serialization failed. Should
    /// not happen for any [`crate::spec::FigureSpec`] this crate's own
    /// `render`/`spec` modules construct (every field is a plain
    /// serializable type) — kept as a typed `Result` variant rather than
    /// an `unwrap()`/`panic!()` so a caller that hand-assembles a
    /// `FigureSpec` some other way (e.g. with a non-finite `f64` —
    /// `serde_json` itself rejects `NaN`/`inf`) still gets a real error.
    Serialize(serde_json::Error),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryError::InvalidJson(e) => write!(f, "invalid JSON: {e}"),
            RegistryError::NotAnObject => {
                write!(f, "top-level JSON value must be an object with \"kind\"/\"data\" fields")
            }
            RegistryError::MissingKind => write!(f, "missing required \"kind\" string field"),
            RegistryError::UnknownKind(kind) => write!(f, "unknown figure kind {kind:?}"),
            RegistryError::MissingData(kind) => {
                write!(f, "figure kind {kind:?} is recognized but the object has no \"data\" field")
            }
            RegistryError::MalformedField { kind, message } => {
                write!(f, "malformed field in {kind:?} spec: {message}")
            }
            RegistryError::Serialize(e) => write!(f, "spec serialization failed: {e}"),
        }
    }
}

impl std::error::Error for RegistryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RegistryError::InvalidJson(e) | RegistryError::Serialize(e) => Some(e),
            RegistryError::NotAnObject
            | RegistryError::MissingKind
            | RegistryError::UnknownKind(_)
            | RegistryError::MissingData(_)
            | RegistryError::MalformedField { .. } => None,
        }
    }
}
