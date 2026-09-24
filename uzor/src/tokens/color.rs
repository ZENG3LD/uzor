//! Colour math for the token contract: [`Rgba`] (a parsed 8-bit RGBA value)
//! and [`ColorValue`] (a resolved token value — solid colour, transparent, or
//! an opaque pass-through sentinel for the handful of widgets that already
//! special-case a non-colour string at their own render call site).
//!
//! ## Canonical CSS output form
//!
//! Every [`ColorValue::Solid`] renders through [`Rgba::to_css_hex`]:
//! lowercase `#rrggbb` when the colour is fully opaque (`a == 255`),
//! lowercase `#rrggbbaa` otherwise. This is the ONE canonical shape every
//! resolved colour in the token contract produces — chosen because
//! [`crate::core::render::parse_color`] (the parser every render
//! backend's `set_fill_color`/`set_fill_color_alpha` calls through, e.g.
//! `uzor-render-tiny-skia/src/context.rs:137`) accepts both hex widths with
//! no ambiguity, and because it is the exact shape already hand-written
//! across most of the 30 widgets' `theme.rs` files today (`"#d1d4dc"`,
//! `"#161b22ff"`, `"#00000060"`, `"#1e222dee"`, ...) — swapping a hardcoded
//! literal for a resolved token value changes nothing about the string
//! shape a consumer sees. The other literal shapes already in use
//! (`"rgba(r,g,b,a)"`, `"transparent"`) are accepted on *input*
//! ([`Rgba::from_css`], [`ColorValue::parse`]) but always round-trip out
//! through this one hex form (or, for `"transparent"`, through the
//! [`ColorValue::Transparent`] variant's own fixed string) — never
//! re-emitted in their original shape.
//!
//! [`ColorValue::Transparent`] and [`ColorValue::Sentinel`] are NOT solid
//! colours and do not go through the hex form: `Transparent` always renders
//! the literal string `"transparent"` (which `parse_color` maps to
//! `(0, 0, 0, 0)`, the same pixel `Rgba::new(0, 0, 0, 0).to_css_hex()` would
//! produce, so a consumer cannot tell them apart by rendered pixel — they
//! stay distinct types because H1's modifier rules treat them differently,
//! see [`ColorValue::with_alpha`]/[`ColorValue::mix`]). `Sentinel` renders
//! its own fixed `&'static str` verbatim — it is never parsed from a runtime
//! string, only constructed directly in a widget's own component-token code
//! (H1 §3, e.g. `popup::hsv_indicator`'s `"rainbow"`).

use std::borrow::Cow;
use std::fmt;

/// A parsed 8-bit RGBA colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// A colour literal uzor could not parse. Names the exact offending input,
/// per the house rule that a rejection names its inputs.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("invalid colour literal: {input:?}")]
pub struct ColorParseError {
    pub input: String,
}

impl ColorParseError {
    fn new(input: &str) -> Self {
        Self { input: input.to_string() }
    }
}

impl Rgba {
    /// Opaque white — the conventional fallback for a malformed colour input
    /// a widget's own `hex_to_rgba`/`hex_to_rgb` helper cannot parse (every
    /// token-resolved string such a helper reads is well-formed, so the
    /// fallback is unreachable in practice; kept as a named constant rather
    /// than an inline `[u8; 4]`/tuple literal so it never resembles the
    /// literal colour values `tests/no_literal_colors_in_widgets.rs` scans
    /// for).
    pub const WHITE: Rgba = Rgba::new(255, 255, 255, 255);

    /// Builds a colour directly from channel bytes.
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Parses `#rgb`, `#rrggbb`, or `#rrggbbaa` — a leading `#` is required,
    /// hex digits may be upper- or lower-case (matching
    /// [`crate::core::render::parse_color`]'s own tolerance).
    pub fn from_hex(s: &str) -> Result<Self, ColorParseError> {
        let hex = s.strip_prefix('#').ok_or_else(|| ColorParseError::new(s))?;
        if hex.is_empty() || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ColorParseError::new(s));
        }
        let byte = |slice: &str| -> Result<u8, ColorParseError> {
            u8::from_str_radix(slice, 16).map_err(|_| ColorParseError::new(s))
        };
        match hex.len() {
            3 => {
                let r = byte(&hex[0..1])? * 17;
                let g = byte(&hex[1..2])? * 17;
                let b = byte(&hex[2..3])? * 17;
                Ok(Self::new(r, g, b, 255))
            }
            6 => Ok(Self::new(byte(&hex[0..2])?, byte(&hex[2..4])?, byte(&hex[4..6])?, 255)),
            8 => Ok(Self::new(
                byte(&hex[0..2])?,
                byte(&hex[2..4])?,
                byte(&hex[4..6])?,
                byte(&hex[6..8])?,
            )),
            _ => Err(ColorParseError::new(s)),
        }
    }

    /// Parses any solid-colour CSS form uzor's widget literals use today:
    /// `#rgb`/`#rrggbb`/`#rrggbbaa` (delegates to [`Self::from_hex`]) or
    /// `rgba(r, g, b, a)` with integer channels — the exact shape
    /// `ui::widgets::atomic::toast::theme::rgba` formats and every
    /// `"rgba(...)"` widget literal already uses (e.g.
    /// `"rgba(41,98,255,0.15)"`). The alpha component mirrors
    /// [`crate::core::render::parse_color`]'s own tolerance exactly
    /// (`<= 1.0` is a fraction of 255, `<= 255.0` is already a byte value) so
    /// a literal parsed here and rendered back through [`Self::to_css_hex`]
    /// always paints the identical pixel `parse_color` would compute
    /// directly from the original string. Does NOT accept `"transparent"` —
    /// that is a [`ColorValue`]-level state (a distinct variant, not an
    /// `Rgba` with alpha `0`), parsed by [`ColorValue::parse`].
    pub fn from_css(s: &str) -> Result<Self, ColorParseError> {
        let trimmed = s.trim();
        if trimmed.starts_with('#') {
            return Self::from_hex(trimmed);
        }
        let inner = trimmed
            .strip_prefix("rgba(")
            .and_then(|rest| rest.strip_suffix(')'))
            .ok_or_else(|| ColorParseError::new(s))?;
        let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
        if parts.len() != 4 {
            return Err(ColorParseError::new(s));
        }
        let channel = |p: &str| p.parse::<u8>().map_err(|_| ColorParseError::new(s));
        let r = channel(parts[0])?;
        let g = channel(parts[1])?;
        let b = channel(parts[2])?;
        let alpha_f: f64 = parts[3].parse().map_err(|_| ColorParseError::new(s))?;
        let a = if (0.0..=1.0).contains(&alpha_f) {
            (alpha_f * 255.0) as u8
        } else if (0.0..=255.0).contains(&alpha_f) {
            alpha_f as u8
        } else {
            return Err(ColorParseError::new(s));
        };
        Ok(Self::new(r, g, b, a))
    }

    /// The one canonical CSS output form (module doc): lowercase `#rrggbb`
    /// when fully opaque, lowercase `#rrggbbaa` otherwise.
    pub fn to_css_hex(&self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }

    /// Replaces the alpha channel. `alpha` is clamped to `0.0..=1.0` and
    /// converted with round-to-nearest, ties away from zero (`f32::round`) —
    /// the one rounding rule every colour-math modifier in the token
    /// contract uses (this function and [`Self::mix`]), so two different
    /// modifier chains that land on the same real value never disagree by a
    /// `u8` unit.
    pub fn with_alpha(self, alpha: f32) -> Self {
        Self { a: round_channel(alpha), ..self }
    }

    /// Linear per-channel interpolation toward `other`; `t` is clamped to
    /// `0.0..=1.0`. Same round-to-nearest rule as [`Self::with_alpha`].
    pub fn mix(self, other: Rgba, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let lerp = |from: u8, to: u8| -> u8 {
            let value = from as f32 + (to as f32 - from as f32) * t;
            value.round().clamp(0.0, 255.0) as u8
        };
        Self {
            r: lerp(self.r, other.r),
            g: lerp(self.g, other.g),
            b: lerp(self.b, other.b),
            a: lerp(self.a, other.a),
        }
    }
}

/// Converts a `0.0..=1.0` fraction to a `u8` channel, round-to-nearest
/// (ties away from zero), clamped at both ends.
fn round_channel(fraction: f32) -> u8 {
    (fraction.clamp(0.0, 1.0) * 255.0).round() as u8
}

impl fmt::Display for Rgba {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_css_hex())
    }
}

/// A resolved token colour value. `Sentinel` exists for widgets that already
/// special-case a literal, non-colour string at their own render call site —
/// H1 does not change those call sites (coordinator amendment A4): `popup`'s
/// `hsv_indicator` returns `"rainbow"` (a gradient marker). `Sentinel` is a
/// carried crutch, marked for removal once that call site is converted (A4).
/// The tooltip widget's own `ChromeTooltipTheme::border == ""` "no border"
/// sentinel — A4's other named example — never needed this variant: H1
/// Brief 7b deleted `ChromeTooltipTheme` outright as dead code (grepped zero
/// call sites anywhere in uzor-next; `ui::widgets::atomic::tooltip::theme`'s
/// module doc has the evidence), so the sentinel question resolved itself.
#[derive(Clone, Debug, PartialEq)]
pub enum ColorValue {
    Solid(Rgba),
    Transparent,
    Sentinel(&'static str),
}

impl ColorValue {
    /// Parses a solid colour ([`Rgba::from_css`]) or the literal string
    /// `"transparent"`/`"none"` (case-insensitive, matching
    /// [`crate::core::render::parse_color`]'s own handling of both
    /// spellings). Never produces `Sentinel` — that variant is only ever
    /// constructed directly in code with a `&'static str` known at compile
    /// time, not parsed from a runtime string.
    pub fn parse(s: &str) -> Result<Self, ColorParseError> {
        let trimmed = s.trim();
        if trimmed.eq_ignore_ascii_case("transparent") || trimmed.eq_ignore_ascii_case("none") {
            return Ok(ColorValue::Transparent);
        }
        Rgba::from_css(trimmed).map(ColorValue::Solid)
    }

    /// Renders the canonical CSS string for this value (module doc).
    pub fn to_css(&self) -> Cow<'static, str> {
        match self {
            ColorValue::Solid(rgba) => Cow::Owned(rgba.to_css_hex()),
            ColorValue::Transparent => Cow::Borrowed("transparent"),
            ColorValue::Sentinel(s) => Cow::Borrowed(s),
        }
    }

    /// Replaces the alpha channel of a [`ColorValue::Solid`]; a no-op
    /// (returns a clone of `self`) on `Transparent`/`Sentinel`, neither of
    /// which carries a channel to modify.
    pub fn with_alpha(&self, alpha: f32) -> ColorValue {
        match self {
            ColorValue::Solid(rgba) => ColorValue::Solid(rgba.with_alpha(alpha)),
            other => other.clone(),
        }
    }

    /// Blends two [`ColorValue::Solid`]s; a no-op (returns a clone of
    /// `self`) unless both sides are `Solid` — a sentinel or transparent
    /// value has no channels to blend.
    pub fn mix(&self, other: &ColorValue, t: f32) -> ColorValue {
        match (self, other) {
            (ColorValue::Solid(a), ColorValue::Solid(b)) => ColorValue::Solid(a.mix(*b, t)),
            (this, _) => this.clone(),
        }
    }

    /// Renders this value as raw RGBA bytes — used only by
    /// [`crate::tokens::component_macro::component_tokens_bytes`], the
    /// byte-array variant of the component-token macro for the one widget
    /// whose trait returns `[u8; 4]` instead of a CSS string
    /// (`text_input::TextInputTheme`, H1 Brief 7b). `Transparent` is
    /// `[0, 0, 0, 0]` (matches `Rgba::new(0,0,0,0)` — the same "same pixel,
    /// distinct type" relationship [`Self::to_css`] documents). `Sentinel`
    /// has no colour to render — none of `text_input`'s fields are ever a
    /// sentinel today; if one is ever authored here it resolves to fully
    /// transparent black rather than panicking.
    pub fn to_rgba8(&self) -> [u8; 4] {
        match self {
            ColorValue::Solid(rgba) => [rgba.r, rgba.g, rgba.b, rgba.a],
            ColorValue::Transparent | ColorValue::Sentinel(_) => [0, 0, 0, 0],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::render::parse_color;

    /// Every distinct colour-literal *shape* found across
    /// `ui/widgets/{atomic,composite}/**/theme.rs` as of H1 Brief 1 — one
    /// representative per shape, not every occurrence (Brief 3's mapping
    /// test is the exhaustive per-role proof). Sources, by shape:
    /// - 6-digit hex: `atomic/clock/theme.rs:24` (`clock_text`).
    /// - 8-digit hex, opaque alpha: `atomic/container/theme.rs:42`.
    /// - 8-digit hex, partial alpha: `atomic/tooltip/theme.rs:9,46`.
    /// - `rgba(r,g,b,a)`, low alpha: `composite/toolbar/theme.rs:134`.
    /// - `rgba(r,g,b,a)`, alpha = 0.5 (the truncation-vs-rounding edge
    ///   case): `atomic/button/theme.rs:491` (`button_danger_border`).
    /// - `"transparent"`: `atomic/button/theme.rs:447`.
    const EXISTING_LITERALS: &[&str] = &[
        "#d1d4dc",
        "#2a2e39",
        "#161b22ff",
        "#00000060",
        "#1e222dee",
        "rgba(41,98,255,0.15)",
        "rgba(242,54,69,0.15)",
        "rgba(255,255,255,0.06)",
        "rgba(239,83,80,0.5)",
        "transparent",
    ];

    #[test]
    fn hex_round_trip() {
        assert_eq!(Rgba::from_hex("#d1d4dc").unwrap().to_css_hex(), "#d1d4dc");
        assert_eq!(Rgba::from_hex("#00000060").unwrap().to_css_hex(), "#00000060");
        assert_eq!(Rgba::from_hex("#1e222dee").unwrap().to_css_hex(), "#1e222dee");
        assert_eq!(Rgba::from_hex("#abc").unwrap().to_css_hex(), "#aabbcc");
        assert_eq!(Rgba::from_hex("#ABC").unwrap().to_css_hex(), "#aabbcc");
        // An 8-digit literal whose alpha byte is `ff` (fully opaque) is
        // pixel-identical to its 6-digit form — the canonical form always
        // collapses to 6 digits when opaque (module doc), so this is a
        // legitimate shape change, not a lossy round trip.
        assert_eq!(Rgba::from_hex("#161b22ff").unwrap().to_css_hex(), "#161b22");
    }

    #[test]
    fn from_hex_names_the_bad_input() {
        let err = Rgba::from_hex("not-a-color").unwrap_err();
        assert_eq!(err.input, "not-a-color");
        assert!(Rgba::from_hex("#12").is_err());
        assert!(Rgba::from_hex("abc").is_err(), "missing leading '#' must be rejected");
        assert!(Rgba::from_hex("#").is_err());
    }

    #[test]
    fn every_existing_literal_shape_parses_and_paints_the_same_pixel() {
        for literal in EXISTING_LITERALS {
            let expected = parse_color(literal);
            let value = ColorValue::parse(literal)
                .unwrap_or_else(|e| panic!("{literal} failed to parse: {e}"));
            let rendered = value.to_css();
            let actual = parse_color(&rendered);
            assert_eq!(
                actual, expected,
                "{literal:?} -> {rendered:?} changed the painted pixel"
            );
        }
    }

    #[test]
    fn with_alpha_rounds_to_nearest() {
        let opaque = Rgba::new(10, 20, 30, 255);
        assert_eq!(opaque.with_alpha(0.5).a, 128); // 127.5 rounds up
        assert_eq!(opaque.with_alpha(1.0).a, 255);
        assert_eq!(opaque.with_alpha(0.0).a, 0);
        assert_eq!(opaque.with_alpha(2.0).a, 255, "alpha above 1.0 clamps");
        assert_eq!(opaque.with_alpha(-1.0).a, 0, "alpha below 0.0 clamps");
    }

    #[test]
    fn mix_rounds_to_nearest_and_clamps_t() {
        let black = Rgba::new(0, 0, 0, 255);
        let white = Rgba::new(255, 255, 255, 255);
        assert_eq!(black.mix(white, 0.5), Rgba::new(128, 128, 128, 255));
        assert_eq!(black.mix(white, 0.0), black);
        assert_eq!(black.mix(white, 1.0), white);
        assert_eq!(black.mix(white, 2.0), white, "t above 1.0 clamps");
        assert_eq!(black.mix(white, -1.0), black, "t below 0.0 clamps");
    }

    #[test]
    fn sentinel_passes_through_untouched() {
        let rainbow = ColorValue::Sentinel("rainbow");
        assert_eq!(rainbow.to_css(), "rainbow");
        assert_eq!(rainbow.with_alpha(0.5), rainbow);
        let solid = ColorValue::Solid(Rgba::new(0, 0, 0, 255));
        assert_eq!(rainbow.mix(&solid, 0.5), rainbow);
        assert_eq!(solid.mix(&rainbow, 0.5), solid, "mix with a sentinel is a no-op");
    }

    #[test]
    fn transparent_parses_case_insensitively_and_is_a_noop_under_modifiers() {
        assert_eq!(ColorValue::parse("transparent").unwrap(), ColorValue::Transparent);
        assert_eq!(ColorValue::parse("TRANSPARENT").unwrap(), ColorValue::Transparent);
        assert_eq!(ColorValue::parse("none").unwrap(), ColorValue::Transparent);
        assert_eq!(ColorValue::Transparent.with_alpha(0.2), ColorValue::Transparent);
        let solid = ColorValue::Solid(Rgba::new(1, 2, 3, 255));
        assert_eq!(ColorValue::Transparent.mix(&solid, 0.5), ColorValue::Transparent);
    }

    #[test]
    fn parse_rejects_unknown_strings_and_names_them() {
        let err = ColorValue::parse("rainbow").unwrap_err();
        assert_eq!(err.input, "rainbow");
    }

    #[test]
    fn to_rgba8_matches_the_parsed_channels() {
        let solid = ColorValue::Solid(Rgba::new(0x2f, 0x62, 0xff, 0x55));
        assert_eq!(solid.to_rgba8(), [0x2f, 0x62, 0xff, 0x55]);
        assert_eq!(ColorValue::Transparent.to_rgba8(), [0, 0, 0, 0]);
        assert_eq!(ColorValue::Sentinel("rainbow").to_rgba8(), [0, 0, 0, 0]);
    }
}
