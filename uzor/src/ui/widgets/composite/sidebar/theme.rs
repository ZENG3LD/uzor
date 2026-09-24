//! Sidebar colour palette trait; token-contract default implementation
//! lives in `crate::tokens::theme::TokenTheme`, backed by `super::tokens`.

/// Colour tokens for the sidebar composite.
///
/// Implement on your app theme struct to plug in custom colours.
pub trait SidebarTheme {
    // --- Frame ---

    /// Sidebar body and header background fill.  Default: `#1e222d`.
    fn bg(&self) -> &str;

    /// Left/right border line (1 px separator between chart area and sidebar).
    /// Default: `#363a45`.
    fn border(&self) -> &str;

    // --- Header ---

    /// Header zone background (usually same as `bg`).  Default: `#1e222d`.
    fn header_bg(&self) -> &str;

    /// Header title text.  Default: `#ffffff`.
    fn header_text(&self) -> &str;

    /// Header icon colour (normal).  Default: `#9598a1`.
    fn header_icon(&self) -> &str;

    /// Header bottom divider line.  Default: `#363a45`.
    fn divider(&self) -> &str;

    // --- Action buttons ---

    /// Icon colour for header action buttons in idle state.  Default: `#9598a1`.
    fn action_icon_normal(&self) -> &str;

    /// Icon colour for header action buttons on hover.  Default: `#ffffff`.
    fn action_icon_hover(&self) -> &str;

    // --- Scrollbar (delegated) ---

    /// Scrollbar thumb colour in idle state.  Default: `#363a45`.
    fn scrollbar_thumb(&self) -> &str;

    /// Scrollbar thumb colour on hover / drag.  Default: `#787b86`.
    fn scrollbar_thumb_active(&self) -> &str;

    // --- Tab strip (WithTypeSelector) ---

    /// Active tab text colour.  Default: `#ffffff`.
    fn tab_text_active(&self) -> &str;

    /// Inactive tab text colour.  Default: `#787b86`.
    fn tab_text_inactive(&self) -> &str;

    /// Active tab underline accent.  Default: `#2962ff`.
    fn tab_accent(&self) -> &str;

    /// Active tab background highlight.  Default: `rgba(41,98,255,0.12)`.
    fn tab_bg_active(&self) -> &str;

    /// Hovered tab background.  Default: `rgba(255,255,255,0.06)`.
    fn tab_bg_hover(&self) -> &str;

    // --- render.rs generic highlights (no companion-table row; H1 Brief 8b) ---

    /// Background highlight painted behind a header action button while
    /// hovered.  Default alias: `surface.control.hover`.
    fn action_bg_hover(&self) -> &str;

    /// Faint backdrop painted behind an overflow chevron strip so it reads
    /// as a control, not a body row.  Default alias: `surface.app_chrome`
    /// + alpha(0.85).
    fn chevron_strip_bg(&self) -> &str;

    // --- Generic body content (`input::SidebarBodyBuilder`; H1 Brief 8b) ---

    /// Generic accent for body-content controls (radio-button dot,
    /// action-button fill).  Default alias: `accent.default`.
    fn accent(&self) -> &str;

    /// Text painted on an accent-filled generic control (selected radio
    /// label, action-button label).  Default alias: `text.on_accent`.
    fn on_accent_text(&self) -> &str;

    /// Primary body text for generic content rows (inactive panel-list
    /// title).  Default alias: `text.primary`.
    fn content_text(&self) -> &str;

    /// De-emphasized body text (unselected radio label).  Default alias:
    /// `text.secondary`.
    fn content_muted_text(&self) -> &str;

    /// All-caps section heading (e.g. `"NEW PANEL"`).  Default alias:
    /// `text.muted`.
    fn section_header_text(&self) -> &str;

    /// Muted sub-label (e.g. `"Type:"`, `"Split:"`).  Default alias:
    /// `text.secondary`.
    fn sub_label_text(&self) -> &str;

    /// Unselected radio-dot fill.  Default alias: `surface.control.hover`.
    fn radio_dot_inactive(&self) -> &str;

    /// Active dock-panel-list row background.  Default alias:
    /// `accent.default` + alpha(0.18).
    fn panel_row_bg_active(&self) -> &str;

    /// Inactive dock-panel-list row background.  Default alias:
    /// `surface.control.hover`.
    fn panel_row_bg_inactive(&self) -> &str;

    /// Dock-panel-list row close-glyph colour.  Default alias:
    /// `status.danger` + alpha(0.5).
    fn panel_close_icon(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Token-contract implementation
// ---------------------------------------------------------------------------
//
// `DefaultSidebarTheme` (a literal-colour prototype impl) was deleted in H1
// Brief 8b — `crate::tokens::theme::TokenTheme` is now the one
// `SidebarTheme` implementation this crate ships, backed by
// `crate::ui::widgets::composite::sidebar::tokens::SidebarTokens` (see
// `docs/uzor/plans/h1-token-contract-design-2026-09-24.md` §3).
