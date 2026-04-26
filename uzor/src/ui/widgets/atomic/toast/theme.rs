//! Toast colour palette.

use super::types::ToastSeverity;

pub trait ToastTheme {
    fn bg_info(&self)    -> &str;
    fn bg_success(&self) -> &str;
    fn bg_warning(&self) -> &str;
    fn bg_error(&self)   -> &str;
    fn text(&self)       -> &str;

    fn bg_for(&self, sev: ToastSeverity) -> &str {
        match sev {
            ToastSeverity::Info    => self.bg_info(),
            ToastSeverity::Success => self.bg_success(),
            ToastSeverity::Warning => self.bg_warning(),
            ToastSeverity::Error   => self.bg_error(),
        }
    }
}

#[derive(Default)]
pub struct DefaultToastTheme;

impl ToastTheme for DefaultToastTheme {
    fn bg_info(&self)    -> &str { "#1e64c8" }
    fn bg_success(&self) -> &str { "#28b428" }
    fn bg_warning(&self) -> &str { "#ffb400" }
    fn bg_error(&self)   -> &str { "#dc3232" }
    fn text(&self)       -> &str { "#ffffff" }
}
