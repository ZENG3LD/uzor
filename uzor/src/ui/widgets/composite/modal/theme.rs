//! Modal colour palette trait and default dark-theme implementation.
//!
//! All colour values ported from the mlc audit in `modal-deep.md` §7 and
//! `render_modal_frame_only` in `mylittlechart/crates/chart/src/layout/modals/`.

/// Colour tokens for the modal composite.
///
/// Implement this trait on your app theme struct to plug in custom colours.
pub trait ModalTheme {
    // --- Frame ---

    /// Modal background fill.  Default: `#1e222d`.
    fn bg(&self) -> &str;

    /// Frame border (1 px stroke).  Default: `#363a45`.
    fn border(&self) -> &str;

    /// Shadow rect fill.  Default: `rgba(0,0,0,0.4)`.
    fn shadow(&self) -> &str;

    // --- Header ---

    /// Header zone background.  Default: same as `bg` (`#1e222d`).
    fn header_bg(&self) -> &str;

    /// Header title text colour.  Default: `#ffffff`.
    fn header_text(&self) -> &str;

    /// Header bottom separator line.  Default: `#363a45`.
    fn divider(&self) -> &str;

    // --- Footer ---

    /// Footer zone background.  Default: same as `bg`.
    fn footer_bg(&self) -> &str;

    /// Footer top separator line.  Default: `#363a45`.
    fn footer_border(&self) -> &str;

    // --- Close button ---

    /// Close-X icon colour in idle state.  Default: `#9598a1`.
    fn close_icon(&self) -> &str;

    /// Close-X icon colour on hover.  Default: `#ffffff`.
    fn close_icon_hover(&self) -> &str;

    // --- Backdrop ---

    /// Backdrop dim fill (rgba string) used for `BackdropKind::Dim`.
    /// Default: `rgba(0,0,0,0.45)`.
    fn backdrop_dim(&self) -> &str;

    /// Backdrop fill colour for `BackdropKind::FullBlock`.
    /// Default: `#131722` (near-black, matches mlc WelcomeWizard background).
    fn backdrop_full(&self) -> &str;

    // --- Sidebar (SideTabs) ---

    /// Sidebar strip background.  Default: `#1e222d` (same as frame bg).
    fn sidebar_bg(&self) -> &str;

    /// Sidebar right-edge separator.  Default: `#363a45`.
    fn sidebar_border(&self) -> &str;

    // --- Tab strip ---

    /// Active tab text colour.  Default: `#ffffff`.
    fn tab_text_active(&self) -> &str;

    /// Inactive tab text colour.  Default: `#787b86`.
    fn tab_text_inactive(&self) -> &str;

    /// Active tab underline / sidebar left-border accent.  Default: `#2962ff`.
    fn tab_accent(&self) -> &str;

    /// Active tab background highlight.  Default: `rgba(41,98,255,0.12)`.
    fn tab_bg_active(&self) -> &str;

    /// Hovered tab background.  Default: `rgba(255,255,255,0.06)`.
    fn tab_bg_hover(&self) -> &str;

    // --- Wizard ---

    /// Inactive page-dot colour.  Default: `#363a45`.
    fn wizard_dot_inactive(&self) -> &str;

    /// Active page-dot colour.  Default: `#2962ff`.
    fn wizard_dot_active(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Default dark theme
// ---------------------------------------------------------------------------

/// Default dark-theme implementation.
///
/// Values sourced from the mlc audit (§7, `modal-deep.md`) and
/// `render_modal_frame_only`.
#[derive(Default)]
pub struct DefaultModalTheme;

impl ModalTheme for DefaultModalTheme {
    // Frame
    fn bg(&self)     -> &str { "#1e222d" }
    fn border(&self) -> &str { "#363a45" }
    fn shadow(&self) -> &str { "rgba(0,0,0,0.4)" }

    // Header
    fn header_bg(&self)   -> &str { "#1e222d" }
    fn header_text(&self) -> &str { "#ffffff" }
    fn divider(&self)     -> &str { "#363a45" }

    // Footer
    fn footer_bg(&self)     -> &str { "#1e222d" }
    fn footer_border(&self) -> &str { "#363a45" }

    // Close button
    fn close_icon(&self)       -> &str { "#9598a1" }
    fn close_icon_hover(&self) -> &str { "#ffffff" }

    // Backdrop
    fn backdrop_dim(&self)  -> &str { "rgba(0,0,0,0.45)" }
    fn backdrop_full(&self) -> &str { "#131722" }

    // Sidebar
    fn sidebar_bg(&self)     -> &str { "#1e222d" }
    fn sidebar_border(&self) -> &str { "#363a45" }

    // Tab strip
    fn tab_text_active(&self)   -> &str { "#ffffff" }
    fn tab_text_inactive(&self) -> &str { "#787b86" }
    fn tab_accent(&self)        -> &str { "#2962ff" }
    fn tab_bg_active(&self)     -> &str { "rgba(41,98,255,0.12)" }
    fn tab_bg_hover(&self)      -> &str { "rgba(255,255,255,0.06)" }

    // Wizard dots
    fn wizard_dot_inactive(&self) -> &str { "#363a45" }
    fn wizard_dot_active(&self)   -> &str { "#2962ff" }
}
