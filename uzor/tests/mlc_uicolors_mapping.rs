//! D7 mapping proof (H1 token contract design §5): uzor's 4 generic
//! built-in token sets must reproduce MLC's `UIColors` fields exactly, per
//! the `UIColors` → token path table in the design doc's §4.
//!
//! Source of truth is MLC's `UIColors` AT A COMMIT, never MLC's working
//! copy: `git -C mylittlechart show <commit>:crates/mlc-core/src/theme/preset.rs`,
//! commit `05b0e1cf6776e9ea8941337bbffbdada65ee7728` (`main`, read
//! 2026-09-24). `UIColors` struct: `preset.rs:872-907`. Constructors:
//! `UITheme::dark()` `preset.rs:989`, `::light()` `:1067`,
//! `::high_contrast()` `:1145`, `::high_contrast_mono()` `:1223`. Every
//! literal transcribed into this file's test bodies was read directly from
//! that commit's text — all 4 constructors' `UIColors { .. }` literals
//! matched the H1 design doc's §1 table exactly; no JSON correction was
//! needed against this commit.
//!
//! Comparisons are parsed [`Rgba`], never raw strings — a casing difference
//! (`#FEFFEE` vs `#feffee`) must not fail this test.
//!
//! ## `UIColors` fields NOT asserted here, and why (§4)
//! - `button_hover_stroke` — maps to `component.button.button_border_hover`,
//!   a per-widget component-token override, not a [`SemanticRoles`] field;
//!   no widget carries that override table yet (lands starting Brief 4).
//! - `button_active_stroke` — maps to `color.border.strong`, but that
//!   correspondence only holds in `high_contrast_mono` (asserted there,
//!   below). In dark/light/high_contrast, MLC's value is the literal string
//!   `"transparent"` (not a role-comparable colour), while `border.strong`
//!   there aliases `border.default` (a real colour) — no field-level
//!   equality exists to assert for those 3 presets.
//! - `border` — dead: byte-identical to `toolbar_bg` in every preset, never
//!   promoted to a role (§1's `border.dead` note).
//! - `bubble_user_bg` — stays in the `app.*` namespace (D4); no
//!   `SemanticRoles` field represents it (no chat-bubble widget among H1's
//!   30 converted widgets).
//!
//! ## The accepted collapse (`high_contrast_mono` only)
//! `divider` = `#444444` there while `toolbar_divider`/`ui_border` =
//! `#333333` — a 1-of-3 outlier against the majority. §1/§4 collapse
//! `border.default` onto the `#333333` majority; the MLC coordinator
//! accepted this exact collapse (H1 design doc, coordinator review). The
//! `high_contrast_mono` test below asserts `toolbar_divider`/`ui_border`
//! against `border.default` and deliberately does NOT assert `divider`
//! (asserting it would be a false equality).

use uzor::tokens::{BuiltinSet, ColorValue, Rgba, Tokens};

/// Extracts the [`Rgba`] a resolved [`ColorValue`] renders. Every role this
/// test touches resolves to `ColorValue::Solid` (no `Transparent`/
/// `Sentinel` among the mapped fields) — a non-`Solid` value here is itself
/// a test failure, not an input to tolerate.
fn solid(role_name: &str, value: &ColorValue) -> Rgba {
    match value {
        ColorValue::Solid(rgba) => *rgba,
        other => panic!("{role_name}: expected a solid colour, found {other:?}"),
    }
}

/// Asserts one `UIColors` field reproduces its §4-mapped `SemanticRoles`
/// value, comparing parsed [`Rgba`] (never raw strings).
fn assert_mapped(role_name: &str, role_value: &ColorValue, mlc_field: &str, mlc_hex: &str) {
    let token_rgba = solid(role_name, role_value);
    let mlc_rgba = Rgba::from_hex(mlc_hex)
        .unwrap_or_else(|e| panic!("{mlc_field}: MLC literal {mlc_hex:?} failed to parse: {e}"));
    assert_eq!(
        token_rgba, mlc_rgba,
        "UIColors.{mlc_field} -> semantic.{role_name}: token {token_rgba:?} != MLC {mlc_rgba:?} (from {mlc_hex:?})"
    );
}

#[test]
fn dark_reproduces_mlc_uicolors_dark() {
    let t = Tokens::builtin(BuiltinSet::Dark);
    let s = &t.semantic;
    assert_mapped("surface_app_chrome", &s.surface_app_chrome, "toolbar_bg", "#131722");
    assert_mapped("surface_app_chrome", &s.surface_app_chrome, "status_bar_bg", "#131722");
    assert_mapped("surface_control_idle", &s.surface_control_idle, "button_bg", "#1e222d");
    assert_mapped("surface_control_hover", &s.surface_control_hover, "button_bg_hover", "#2a2e39");
    assert_mapped("surface_control_active", &s.surface_control_active, "button_bg_active", "#2962ff");
    assert_mapped("surface_floating", &s.surface_floating, "dropdown_bg", "#1e222d");
    assert_mapped("text_primary", &s.text_primary, "text_primary", "#d1d4dc");
    assert_mapped("text_secondary", &s.text_secondary, "text_secondary", "#b2b5be");
    assert_mapped("text_muted", &s.text_muted, "text_muted", "#787b86");
    assert_mapped("text_on_accent", &s.text_on_accent, "text_active", "#d1d4dc");
    assert_mapped("border_subtle", &s.border_subtle, "border_light", "#2a2e39");
    assert_mapped("border_default", &s.border_default, "divider", "#363a45");
    assert_mapped("border_default", &s.border_default, "toolbar_divider", "#363a45");
    assert_mapped("border_default", &s.border_default, "ui_border", "#363a45");
    assert_mapped("accent_default", &s.accent_default, "accent", "#2962ff");
    assert_mapped("accent_hover", &s.accent_hover, "accent_hover", "#1e53e4");
    assert_mapped("status_success", &s.status_success, "success", "#26a69a");
    assert_mapped("status_danger", &s.status_danger, "danger", "#f23645");
    assert_mapped("status_warning", &s.status_warning, "warning", "#ff9800");
    assert_mapped("selection", &s.selection, "text_selection", "#2962ff55");
    assert_eq!(t.geometry.radius_md, 4.0, "UIColors.button_rounding -> geometry.radius.md");
}

#[test]
fn light_reproduces_mlc_uicolors_light() {
    let t = Tokens::builtin(BuiltinSet::Light);
    let s = &t.semantic;
    assert_mapped("surface_app_chrome", &s.surface_app_chrome, "toolbar_bg", "#f8f9fa");
    assert_mapped("surface_app_chrome", &s.surface_app_chrome, "status_bar_bg", "#f8f9fa");
    assert_mapped("surface_control_idle", &s.surface_control_idle, "button_bg", "#e9ecef");
    assert_mapped("surface_control_hover", &s.surface_control_hover, "button_bg_hover", "#dee2e6");
    assert_mapped("surface_control_active", &s.surface_control_active, "button_bg_active", "#4a90d9");
    assert_mapped("surface_floating", &s.surface_floating, "dropdown_bg", "#ffffff");
    assert_mapped("text_primary", &s.text_primary, "text_primary", "#131722");
    assert_mapped("text_secondary", &s.text_secondary, "text_secondary", "#434651");
    assert_mapped("text_muted", &s.text_muted, "text_muted", "#787b86");
    assert_mapped("text_on_accent", &s.text_on_accent, "text_active", "#131722");
    assert_mapped("border_subtle", &s.border_subtle, "border_light", "#e9ecef");
    assert_mapped("border_default", &s.border_default, "divider", "#dee2e6");
    assert_mapped("border_default", &s.border_default, "toolbar_divider", "#dee2e6");
    assert_mapped("border_default", &s.border_default, "ui_border", "#dee2e6");
    assert_mapped("accent_default", &s.accent_default, "accent", "#4a90d9");
    assert_mapped("accent_hover", &s.accent_hover, "accent_hover", "#3a7bc8");
    assert_mapped("status_success", &s.status_success, "success", "#26a69a");
    assert_mapped("status_danger", &s.status_danger, "danger", "#f23645");
    assert_mapped("status_warning", &s.status_warning, "warning", "#ff9800");
    assert_mapped("selection", &s.selection, "text_selection", "#4a90d940");
    assert_eq!(t.geometry.radius_md, 4.0, "UIColors.button_rounding -> geometry.radius.md");
}

#[test]
fn high_contrast_reproduces_mlc_uicolors_high_contrast() {
    let t = Tokens::builtin(BuiltinSet::HighContrast);
    let s = &t.semantic;
    assert_mapped("surface_app_chrome", &s.surface_app_chrome, "toolbar_bg", "#000000");
    assert_mapped("surface_app_chrome", &s.surface_app_chrome, "status_bar_bg", "#000000");
    assert_mapped("surface_control_idle", &s.surface_control_idle, "button_bg", "#1a1a1a");
    assert_mapped("surface_control_hover", &s.surface_control_hover, "button_bg_hover", "#333333");
    assert_mapped("surface_control_active", &s.surface_control_active, "button_bg_active", "#0066ff");
    assert_mapped("surface_floating", &s.surface_floating, "dropdown_bg", "#000000");
    assert_mapped("text_primary", &s.text_primary, "text_primary", "#ffffff");
    assert_mapped("text_secondary", &s.text_secondary, "text_secondary", "#cccccc");
    assert_mapped("text_muted", &s.text_muted, "text_muted", "#999999");
    assert_mapped("text_on_accent", &s.text_on_accent, "text_active", "#ffffff");
    assert_mapped("border_subtle", &s.border_subtle, "border_light", "#666666");
    assert_mapped("border_default", &s.border_default, "divider", "#ffffff");
    assert_mapped("border_default", &s.border_default, "toolbar_divider", "#ffffff");
    assert_mapped("border_default", &s.border_default, "ui_border", "#ffffff");
    assert_mapped("accent_default", &s.accent_default, "accent", "#0066ff");
    assert_mapped("accent_hover", &s.accent_hover, "accent_hover", "#0055dd");
    assert_mapped("status_success", &s.status_success, "success", "#00ff00");
    assert_mapped("status_danger", &s.status_danger, "danger", "#ff0000");
    assert_mapped("status_warning", &s.status_warning, "warning", "#ffff00");
    assert_mapped("selection", &s.selection, "text_selection", "#0066ffaa");
    assert_eq!(t.geometry.radius_md, 4.0, "UIColors.button_rounding -> geometry.radius.md");
}

#[test]
fn high_contrast_mono_reproduces_mlc_uicolors_high_contrast_mono() {
    let t = Tokens::builtin(BuiltinSet::HighContrastMono);
    let s = &t.semantic;
    assert_mapped("surface_app_chrome", &s.surface_app_chrome, "toolbar_bg", "#000000");
    assert_mapped("surface_app_chrome", &s.surface_app_chrome, "status_bar_bg", "#000000");
    assert_mapped("surface_control_idle", &s.surface_control_idle, "button_bg", "#111111");
    assert_mapped("surface_control_hover", &s.surface_control_hover, "button_bg_hover", "#222222");
    assert_mapped("surface_control_active", &s.surface_control_active, "button_bg_active", "#999999");
    assert_mapped("surface_floating", &s.surface_floating, "dropdown_bg", "#111111");
    assert_mapped("text_primary", &s.text_primary, "text_primary", "#ffffff");
    assert_mapped("text_secondary", &s.text_secondary, "text_secondary", "#cccccc");
    assert_mapped("text_muted", &s.text_muted, "text_muted", "#888888");
    assert_mapped("text_on_accent", &s.text_on_accent, "text_active", "#ffffff");
    assert_mapped("border_subtle", &s.border_subtle, "border_light", "#333333");
    // `divider` (#444444) is the accepted 1-of-3 outlier collapse — NOT
    // asserted here (see module header). `toolbar_divider`/`ui_border`
    // (both #333333) are the majority this role's value follows.
    assert_mapped("border_default", &s.border_default, "toolbar_divider", "#333333");
    assert_mapped("border_default", &s.border_default, "ui_border", "#333333");
    // The one preset where `button_active_stroke` has a real (non-
    // "transparent") value — it maps to `border.strong` here (see module
    // header for why this is skipped in the other 3 presets).
    assert_mapped("border_strong", &s.border_strong, "button_active_stroke", "#ffffff");
    assert_mapped("accent_default", &s.accent_default, "accent", "#999999");
    assert_mapped("accent_hover", &s.accent_hover, "accent_hover", "#bbbbbb");
    assert_mapped("status_success", &s.status_success, "success", "#cccccc");
    assert_mapped("status_danger", &s.status_danger, "danger", "#ffffff");
    assert_mapped("status_warning", &s.status_warning, "warning", "#aaaaaa");
    assert_mapped("selection", &s.selection, "text_selection", "#99999955");
    assert_eq!(t.geometry.radius_md, 0.0, "UIColors.button_rounding -> geometry.radius.md");
}
