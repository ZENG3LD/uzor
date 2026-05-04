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
}

impl Default for StyleManager {
    fn default() -> Self {
        let mut sm = Self {
            colors:   HashMap::new(),
            sizes:    HashMap::new(),
            textures: HashMap::new(),
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
        sm.set_color("surface_0",      "#EBEBEB");
        sm.set_color("surface",        "#F7F7F4");
        sm.set_color("surface_raised", "#FFFFFF");
        sm.set_color("fg_0",           "#1a1a1a");
        sm.set_color("fg_1",           "#3a3a3a");
        sm.set_color("fg_2",           "#6b6b6b");
        sm.set_color("fg_3",           "#9a9a9a");
        sm.set_color("accent",         "#D07000");
        sm.set_color("accent_hover",   "#B86000");
        sm.set_color("accent_dim",     "rgba(208,112,0,0.12)");
        sm.set_color("border",         "rgba(0,0,0,0.08)");
        sm.set_color("border_strong",  "rgba(0,0,0,0.16)");
        sm.set_color("ok",             "#2A7A3A");
        sm.set_color("warn",           "#A06010");
        sm.set_color("error",          "#C03030");
    }
}
