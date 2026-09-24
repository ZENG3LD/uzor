//! Modal colour palette trait; token-contract default implementation lives
//! in `crate::tokens::theme::TokenTheme`, backed by `super::tokens`.

/// Colour tokens for the modal composite.
///
/// Implement this trait on your app theme struct to plug in custom colours.
pub trait ModalTheme {
    // --- Frame ---

    /// Modal background fill.  Default alias: `surface.floating`.
    fn bg(&self) -> &str;

    /// Frame border (1 px stroke).  Default alias: `border.default`.
    fn border(&self) -> &str;

    /// Shadow rect fill.  Default alias: `shadow.default`.
    fn shadow(&self) -> &str;

    // --- Header ---

    /// Header zone background.  Default alias: `surface.header`.
    fn header_bg(&self) -> &str;

    /// Header title text colour.  Default alias: `text.on_accent`.
    fn header_text(&self) -> &str;

    /// Header bottom separator line.  Default alias: `border.default`.
    fn divider(&self) -> &str;

    // --- Footer ---

    /// Footer zone background.  Default alias: `surface.floating`.
    fn footer_bg(&self) -> &str;

    /// Footer top separator line.  Default alias: `border.default`.
    fn footer_border(&self) -> &str;

    // --- Close button ---

    /// Close-X icon colour in idle state.  Default alias: `text.secondary`.
    fn close_icon(&self) -> &str;

    /// Close-X icon colour on hover.  Default alias: `text.on_accent`.
    fn close_icon_hover(&self) -> &str;

    // --- Backdrop ---

    /// Backdrop dim fill (rgba string) used for `BackdropKind::Dim`.
    /// Default alias: `backdrop.dim`.
    fn backdrop_dim(&self) -> &str;

    /// Backdrop fill colour for `BackdropKind::FullBlock`.
    /// Default alias: `backdrop.full` (= `surface.app_chrome`).
    fn backdrop_full(&self) -> &str;

    // --- Sidebar (SideTabs) ---

    /// Sidebar strip background.  Default alias: `surface.floating`.
    fn sidebar_bg(&self) -> &str;

    /// Sidebar right-edge separator.  Default alias: `border.default`.
    fn sidebar_border(&self) -> &str;

    // --- Tab strip ---

    /// Active tab text colour.  Default alias: `text.on_accent`.
    fn tab_text_active(&self) -> &str;

    /// Inactive tab text colour.  Default alias: `text.muted`.
    fn tab_text_inactive(&self) -> &str;

    /// Active tab underline / sidebar left-border accent.  Default alias:
    /// `accent.default`.
    fn tab_accent(&self) -> &str;

    /// Active tab background highlight.  Default alias: `accent.default`
    /// + alpha(0.12).
    fn tab_bg_active(&self) -> &str;

    /// Hovered tab background.  Default alias: `surface.control.hover`.
    fn tab_bg_hover(&self) -> &str;

    // --- Wizard ---

    /// Inactive page-dot colour.  Default alias: `border.default`.
    fn wizard_dot_inactive(&self) -> &str;

    /// Active page-dot colour.  Default alias: `accent.default`.
    fn wizard_dot_active(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Token-contract implementation
// ---------------------------------------------------------------------------
//
// `DefaultModalTheme` (a literal-colour prototype impl) was deleted in H1
// Brief 8a — `crate::tokens::theme::TokenTheme` is now the one `ModalTheme`
// implementation this crate ships, backed by
// `crate::ui::widgets::composite::modal::tokens::ModalTokens` (see
// `docs/uzor/plans/h1-token-contract-design-2026-09-24.md` §3).
