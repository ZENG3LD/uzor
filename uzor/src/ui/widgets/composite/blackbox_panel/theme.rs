//! BlackboxPanel colour palette trait and default dark-theme implementation.

// ---------------------------------------------------------------------------
// BlackboxTheme trait
// ---------------------------------------------------------------------------

/// Colour tokens for the blackbox panel composite.
///
/// Implement on your app theme struct to plug in custom colours.
pub trait BlackboxTheme {
    /// Panel body background fill.
    ///
    /// Default: `#1a1d28` (dark chart background).
    fn bg(&self) -> &str;

    /// 1 px border colour (`WithBorder` / `WithHeaderBorder` kinds).
    ///
    /// Default: `#363a45`.
    fn border(&self) -> &str;

    /// Header strip background.
    ///
    /// Default: `#1e222d`.
    fn header_bg(&self) -> &str;

    /// Header title text colour.
    ///
    /// Default: `#ffffff`.
    fn header_text(&self) -> &str;

    /// 1 px divider line between header and body.
    ///
    /// Default: `#363a45`.
    fn divider(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Default dark theme
// ---------------------------------------------------------------------------

/// Default dark-theme implementation matching mlc blackbox panel colours.
#[derive(Default)]
pub struct DefaultBlackboxTheme;

impl BlackboxTheme for DefaultBlackboxTheme {
    fn bg(&self)          -> &str { "#1a1d28" }
    fn border(&self)      -> &str { "#363a45" }
    fn header_bg(&self)   -> &str { "#1e222d" }
    fn header_text(&self) -> &str { "#ffffff"  }
    fn divider(&self)     -> &str { "#363a45" }
}
