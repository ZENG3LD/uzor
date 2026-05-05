//! Centralised style/colour/size/texture registry on `LayoutManager`.
//!
//! Apps configure global look-and-feel by pushing values into this manager;
//! `lm::*` builders read from it when no per-instance `Settings` were
//! supplied via `.settings(...)`. Widget Settings remain the
//! highest-priority override — StyleManager is the *default supplier*.

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureKind {
    Solid,
    Glass,        // semi-transparent (alpha ~0.7)
    Frosted,      // backdrop-blur (renderer support TBD)
    Custom(u32),  // app-defined slot id
}

impl Default for TextureKind {
    fn default() -> Self {
        TextureKind::Solid
    }
}

/// Three string-keyed dictionaries: colors, sizes, textures.
#[derive(Debug, Clone)]
pub struct StyleManager {
    colors:   HashMap<String, String>,
    sizes:    HashMap<String, f64>,
    textures: HashMap<String, TextureKind>,
    /// Name passed to the most recent `apply_named` call (or `None`
    /// when only `apply` was used or the style is at default).  Apps
    /// compare against this string to know which preset is active —
    /// see `lm::button(...).active(layout.styles().active_preset() ==
    /// Some("mirage_dark"))`.
    active_preset: Option<String>,
}

impl Default for StyleManager {
    fn default() -> Self {
        let mut sm = Self {
            colors:   HashMap::new(),
            sizes:    HashMap::new(),
            textures: HashMap::new(),
            active_preset: None,
        };
        // Mirage default palette (mirrors tokens.toml in uzor-framework).
        sm.set_color("surface_0",      "#08090B");
        sm.set_color("surface",        "#0E0E11");
        sm.set_color("surface_raised", "#1C1D23");
        sm.set_color("fg_0",           "#F4F4F5");
        sm.set_color("fg_1",           "#D7D8DB");
        sm.set_color("fg_2",           "#878B91");
        sm.set_color("fg_3",           "#555860");
        sm.set_color("accent",         "#FBB26A");
        sm.set_color("accent_hover",   "#F9A04E");
        sm.set_color("accent_dim",     "rgba(251,178,106,0.12)");
        sm.set_color("border",         "rgba(255,255,255,0.06)");
        sm.set_color("border_strong",  "rgba(255,255,255,0.12)");
        sm.set_color("ok",             "#5CB87A");
        sm.set_color("warn",           "#E8A838");
        sm.set_color("error",          "#EB5757");

        // Default sizes.
        sm.set_size("button_radius",    6.0);
        sm.set_size("button_padding",   8.0);
        sm.set_size("button_font_size", 13.0);
        sm.set_size("chrome_height",    30.0);
        sm.set_size("modal_radius",     8.0);
        sm.set_size("popup_radius",     6.0);
        sm.set_size("toolbar_height",   40.0);

        // Default textures.
        sm.set_texture("button_bg", TextureKind::Solid);
        sm.set_texture("modal_bg",  TextureKind::Solid);
        sm.set_texture("chrome_bg", TextureKind::Solid);
        sm.set_texture("popup_bg",  TextureKind::Solid);

        sm
    }
}

impl StyleManager {
    pub fn new() -> Self {
        Self::default()
    }

    // ── colors ────────────────────────────────────────────────────────────────

    pub fn color(&self, key: &str) -> Option<&str> {
        self.colors.get(key).map(|s| s.as_str())
    }

    pub fn color_or<'a>(&'a self, key: &str, fallback: &'a str) -> &'a str {
        self.colors.get(key).map(|s| s.as_str()).unwrap_or(fallback)
    }

    /// Like `color_or` but returns an owned `String`, needed when the fallback
    /// is itself a temporary expression (e.g. another `color_or` call).
    pub fn color_or_owned(&self, key: &str, fallback: &str) -> String {
        self.colors.get(key).map(|s| s.clone()).unwrap_or_else(|| fallback.to_string())
    }

    pub fn set_color(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.colors.insert(key.into(), value.into());
    }

    // ── sizes ─────────────────────────────────────────────────────────────────

    pub fn size(&self, key: &str) -> Option<f64> {
        self.sizes.get(key).copied()
    }

    pub fn size_or(&self, key: &str, fallback: f64) -> f64 {
        self.size(key).unwrap_or(fallback)
    }

    pub fn set_size(&mut self, key: impl Into<String>, value: f64) {
        self.sizes.insert(key.into(), value);
    }

    // ── textures ──────────────────────────────────────────────────────────────

    pub fn texture(&self, key: &str) -> TextureKind {
        self.textures.get(key).copied().unwrap_or(TextureKind::Solid)
    }

    pub fn set_texture(&mut self, key: impl Into<String>, value: TextureKind) {
        self.textures.insert(key.into(), value);
    }

    // ── bulk apply ────────────────────────────────────────────────────────────

    pub fn apply<P: Preset + ?Sized>(&mut self, preset: &P) {
        preset.apply_to(self);
        self.active_preset = None;
    }

    /// Apply a preset and tag it with a name so apps can detect the
    /// currently active preset by string compare.  Triggers no log
    /// write itself — callers (LM/app) push to the agent log.
    pub fn apply_named<P: Preset + ?Sized>(&mut self, preset: &P, name: impl Into<String>) {
        preset.apply_to(self);
        self.active_preset = Some(name.into());
    }

    /// Name of the currently active preset, or `None` when apps used
    /// the unnamed [`apply`] / set state by hand.
    pub fn active_preset(&self) -> Option<&str> {
        self.active_preset.as_deref()
    }
}

/// Trait for bundles of style overrides (e.g. "Mirage Dark", "Win11 Light").
pub trait Preset {
    fn apply_to(&self, sm: &mut StyleManager);
}

// =============================================================================
// Built-in presets
// =============================================================================

/// Mirage Dark preset (default palette).
pub struct MirageDarkPreset;

impl Preset for MirageDarkPreset {
    fn apply_to(&self, sm: &mut StyleManager) {
        sm.set_color("surface_0",      "#08090B");
        sm.set_color("surface",        "#0E0E11");
        sm.set_color("surface_raised", "#1C1D23");
        sm.set_color("fg_0",           "#F4F4F5");
        sm.set_color("fg_1",           "#D7D8DB");
        sm.set_color("fg_2",           "#878B91");
        sm.set_color("fg_3",           "#555860");
        sm.set_color("accent",         "#FBB26A");
        sm.set_color("accent_hover",   "#F9A04E");
        sm.set_color("accent_dim",     "rgba(251,178,106,0.12)");
        sm.set_color("border",         "rgba(255,255,255,0.06)");
        sm.set_color("border_strong",  "rgba(255,255,255,0.12)");
        sm.set_color("ok",             "#5CB87A");
        sm.set_color("warn",           "#E8A838");
        sm.set_color("error",          "#EB5757");
    }
}

/// Mirage Light preset.
pub struct MirageLightPreset;

impl Preset for MirageLightPreset {
    fn apply_to(&self, sm: &mut StyleManager) {
        // Surface ladder: dark→light, panel-on-canvas distinction kept.
        sm.set_color("surface_0",      "#E4E4E0");
        sm.set_color("surface",        "#F0F0EC");
        sm.set_color("surface_raised", "#FFFFFF");
        // Foreground tones: high-contrast on light background.
        sm.set_color("fg_0",           "#0A0A0A");
        sm.set_color("fg_1",           "#1F1F1F");
        sm.set_color("fg_2",           "#4A4A4A");
        sm.set_color("fg_3",           "#7A7A7A");
        // Accent — darker amber so text on white reads.
        sm.set_color("accent",         "#B85F00");
        sm.set_color("accent_hover",   "#9A4F00");
        sm.set_color("accent_dim",     "rgba(184,95,0,0.16)");
        sm.set_color("border",         "rgba(0,0,0,0.12)");
        sm.set_color("border_strong",  "rgba(0,0,0,0.22)");
        sm.set_color("ok",             "#1F6F30");
        sm.set_color("warn",           "#8C5208");
        sm.set_color("error",          "#A82828");
    }
}
