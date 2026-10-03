//! System clipboard via arboard (design §5.4 `ClipboardRead` / `ClipboardWrite`).

use arboard::Clipboard;

use crate::types::bus::ClipboardResult;
use crate::types::ids::Ticket;

/// Host-owned clipboard. Created once; OS errors become `None` / warn logs.
pub struct NativeClipboard {
    inner: Option<Clipboard>,
}

impl NativeClipboard {
    /// Open the system clipboard; `None` inside when the OS denies access.
    pub fn new() -> Self {
        Self {
            inner: Clipboard::new()
                .map_err(|e| {
                    log::warn!("clipboard open failed: {e}");
                    e
                })
                .ok(),
        }
    }

    /// Write text; OS errors are logged and ignored (executor contract).
    pub fn write(&mut self, text: &str) {
        let Some(cb) = self.inner.as_mut() else {
            return;
        };
        if let Err(e) = cb.set_text(text.to_owned()) {
            log::warn!("clipboard write failed: {e}");
        }
    }

    /// Read text for a `ClipboardRead` ticket.
    pub fn read(&mut self, ticket: Ticket) -> ClipboardResult {
        let text = self.inner.as_mut().and_then(|cb| match cb.get_text() {
            Ok(t) if t.is_empty() => None,
            Ok(t) => Some(t),
            Err(e) => {
                log::warn!("clipboard read failed: {e}");
                None
            }
        });
        ClipboardResult { ticket, text }
    }
}

impl Default for NativeClipboard {
    fn default() -> Self {
        Self::new()
    }
}
