//! Tooltip colour palette.

pub trait TooltipTheme {
    fn bg(&self)     -> &str;
    fn border(&self) -> &str;
    fn text(&self)   -> &str;
    /// Drop-shadow colour.  Only used when `TooltipStyle::has_shadow()` returns `true`.
    /// Matches mlc hardcoded `"#00000060"`.
    fn shadow(&self) -> &str { "#00000060" }
}

// =============================================================================
// Token-contract implementation
// =============================================================================
//
// `DefaultTooltipTheme`, `ChromeTooltipTheme`, `CrosshairTooltipTheme` (three
// literal-colour prototype impls) were deleted in H1 Brief 7b —
// `crate::tokens::theme::TokenTheme` is now the one `TooltipTheme`
// implementation ships, backed by
// `crate::ui::widgets::atomic::tooltip::tokens::TooltipTokens` (see
// `docs/uzor/plans/h1-token-contract-design-2026-09-24.md` §3).
//
// The companion doc's own design kept Chrome/Crosshair as distinct wrapper
// types (`TokenChromeTooltipTheme`/`TokenCrosshairTooltipTheme`) on the
// assumption MLC uses both concurrently on screen. Grepping the whole
// uzor-next tree (uzor-examples, uzor-desktop, `framework::widgets::lm`,
// every composite renderer including `chrome`) found ZERO call sites for
// either constructor path today: `TooltipSettings::chrome()`/`.toolbar()`/
// `.crosshair()` and `TooltipPreset::for_chrome`/`for_toolbar`/
// `for_crosshair` only ever called each other, with no root caller anywhere
// in the tree — dead code by the house rule ("if code is unused, delete
// it"), not yet-wired MLC-side API (MLC does not consume uzor's token
// contract before the W1 switch, main design doc §4). Deleted along with
// the two theme structs (`settings.rs`, this brief);
// `ChromeTooltipStyle`/`CrosshairTooltipStyle` (geometry — H1 is
// colour-only) stay exported for whenever a real caller needs them. A4's
// `ChromeTooltipTheme::border == ""` "no border" sentinel question is moot —
// the struct it lived on is gone.
