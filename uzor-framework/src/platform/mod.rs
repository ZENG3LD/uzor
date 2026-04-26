//! Platform-specific helpers (win32 cursor capture, DWM border colour, etc.)

#[cfg(target_os = "windows")]
pub mod win32;
