//! Text input colour palette.
//!
//! Geometry lives in `style.rs`; this trait is colour-only.

/// Colour trait — overridable by callers via custom `impl`.
pub trait TextInputTheme {
    fn bg_normal(&self)        -> [u8; 4];
    fn bg_disabled(&self)      -> [u8; 4];
    fn border_normal(&self)    -> [u8; 4];
    fn border_hover(&self)     -> [u8; 4];
    fn border_focused(&self)   -> [u8; 4];
    fn text_normal(&self)      -> [u8; 4];
    fn text_disabled(&self)    -> [u8; 4];
    fn placeholder(&self)      -> [u8; 4];
    fn selection(&self)        -> [u8; 4];
    fn cursor(&self)           -> [u8; 4];
}

/// Dark default theme — values copied from mlc.
pub struct DefaultTextInputTheme;

impl Default for DefaultTextInputTheme {
    fn default() -> Self {
        Self
    }
}

impl TextInputTheme for DefaultTextInputTheme {
    fn bg_normal(&self)      -> [u8; 4] { [45, 45, 45, 255] }
    fn bg_disabled(&self)    -> [u8; 4] { [35, 35, 35, 255] }
    fn border_normal(&self)  -> [u8; 4] { [80, 80, 80, 255] }
    fn border_hover(&self)   -> [u8; 4] { [110, 110, 110, 255] }
    fn border_focused(&self) -> [u8; 4] { [0, 120, 215, 255] }
    fn text_normal(&self)    -> [u8; 4] { [255, 255, 255, 255] }
    fn text_disabled(&self)  -> [u8; 4] { [128, 128, 128, 255] }
    fn placeholder(&self)    -> [u8; 4] { [128, 128, 128, 255] }
    fn selection(&self)      -> [u8; 4] { [0, 120, 215, 128] }
    fn cursor(&self)         -> [u8; 4] { [255, 255, 255, 255] }
}
