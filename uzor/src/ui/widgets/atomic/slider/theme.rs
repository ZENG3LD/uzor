//! Slider colour palette.
//!
//! Tokens cover the centralised renderer (variants 1.1–1.4) plus the
//! toolbar-style line-width variant (1.5).

pub trait SliderTheme {
    // ── Track tokens ─────────────────────────────────────────────────────────

    /// Empty (right) portion of the track (mlc `border_normal = "#363a45"`).
    fn track_empty(&self) -> &str;

    /// Filled (left) portion of the track + hover halo (mlc `accent = "#2196F3"`).
    fn accent(&self) -> &str;

    // ── Handle tokens ────────────────────────────────────────────────────────

    /// Handle fill + label text (mlc `text_normal = "#d1d4dc"`).
    fn text_normal(&self) -> &str;

    /// Handle fill when disabled.
    fn text_disabled(&self) -> &str;

    /// Handle border colour (distinct from accent when overriding). Defaults to `accent`.
    fn handle_border(&self) -> &str {
        self.accent()
    }

    // ── Input box tokens (variants 1.1 / 1.2 / 1.4) ─────────────────────────

    /// Input box background (mlc `bg_normal = "#2a2e39"`).
    fn input_bg(&self) -> &str;

    /// Input box border when not editing.
    fn input_border_normal(&self) -> &str;

    /// Input box border when editing (mlc `border_focused = "#2196F3"`).
    fn input_border_focused(&self) -> &str;

    /// Input box text colour (same as `text_normal`).
    fn input_text(&self) -> &str {
        self.text_normal()
    }

    // ── Toolbar-style tokens (variant 1.5 line-width slider) ─────────────────

    /// Track empty colour when using toolbar style.
    fn toolbar_track_empty(&self) -> &str;

    /// Filled portion when using toolbar style.
    fn toolbar_track_filled(&self) -> &str;

    /// Handle + label colour when using toolbar style.
    fn toolbar_handle(&self) -> &str;

    // ── Numeric-input sub-theme tokens (H1 Brief 7a additions) ──────────────
    //
    // `render.rs`'s `make_input_settings` builds a `TextInputTheme` for the
    // inline numeric value box (variants 1.1 / 1.2 / 1.4). Its `bg`/
    // `border_n`/`border_f`/`text` fields already round-trip through
    // `input_bg`/`input_border_normal`/`input_border_focused`/`text_normal`
    // above via `hex_to_rgba`; these 3 cover the remaining fields that used
    // to be `[u8; 4]` literals, never reachable through this trait at all.

    /// Input box background in the disabled state (mlc `bg_disabled =
    /// "#232323"`, corrected onto `surface.control.idle` — same convention
    /// `text_input::TextInputTheme::bg_disabled` uses).
    fn input_bg_disabled(&self) -> &str;

    /// Input box placeholder text colour (mlc `placeholder = "#787b86"` —
    /// same value `text_disabled` already resolves to, `text.muted`, same
    /// convention `text_input::TextInputTheme::placeholder` uses).
    fn input_placeholder(&self) -> &str;

    /// Input box text-selection highlight (mlc `selection =
    /// "#2196f380"`-equivalent, corrected onto the shared `selection` role —
    /// same Windows/Material-blue-style correction
    /// `text_input::TextInputTheme::selection` uses).
    fn input_selection(&self) -> &str;
}

// =============================================================================
// Token-contract implementation
// =============================================================================
//
// `DefaultSliderTheme` (a literal-colour prototype impl) was deleted in H1
// Brief 7a — `crate::tokens::theme::TokenTheme` is now the one
// `SliderTheme` implementation ships, backed by
// `crate::ui::widgets::atomic::slider::tokens::SliderTokens` (see
// `docs/uzor/plans/h1-token-contract-design-2026-09-24.md` §3).
