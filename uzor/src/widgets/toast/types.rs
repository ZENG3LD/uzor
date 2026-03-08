//! Toast type definitions - semantic toast notification variants

/// Main toast type enum covering all toast notification variants
#[derive(Debug, Clone, PartialEq)]
pub enum ToastType {
    /// Informational toast message
    Info {
        message: String,
        duration_ms: u32,
        position: (f64, f64),
        width: f64,
        height: f64,
    },

    /// Success confirmation toast
    Success {
        message: String,
        duration_ms: u32,
        position: (f64, f64),
        width: f64,
        height: f64,
    },

    /// Warning toast message
    Warning {
        message: String,
        duration_ms: u32,
        position: (f64, f64),
        width: f64,
        height: f64,
    },

    /// Error notification toast
    Error {
        message: String,
        duration_ms: u32,
        position: (f64, f64),
        width: f64,
        height: f64,
    },
}

impl ToastType {
    pub fn info(message: impl Into<String>, x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::Info {
            message: message.into(),
            duration_ms: 3000,
            position: (x, y),
            width,
            height,
        }
    }

    pub fn success(message: impl Into<String>, x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::Success {
            message: message.into(),
            duration_ms: 3000,
            position: (x, y),
            width,
            height,
        }
    }

    pub fn warning(message: impl Into<String>, x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::Warning {
            message: message.into(),
            duration_ms: 4000,
            position: (x, y),
            width,
            height,
        }
    }

    pub fn error(message: impl Into<String>, x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::Error {
            message: message.into(),
            duration_ms: 5000,
            position: (x, y),
            width,
            height,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::Info { message, .. } => message,
            Self::Success { message, .. } => message,
            Self::Warning { message, .. } => message,
            Self::Error { message, .. } => message,
        }
    }

    pub fn duration_ms(&self) -> u32 {
        match self {
            Self::Info { duration_ms, .. } => *duration_ms,
            Self::Success { duration_ms, .. } => *duration_ms,
            Self::Warning { duration_ms, .. } => *duration_ms,
            Self::Error { duration_ms, .. } => *duration_ms,
        }
    }

    pub fn position(&self) -> (f64, f64) {
        match self {
            Self::Info { position, .. } => *position,
            Self::Success { position, .. } => *position,
            Self::Warning { position, .. } => *position,
            Self::Error { position, .. } => *position,
        }
    }

    pub fn width(&self) -> f64 {
        match self {
            Self::Info { width, .. } => *width,
            Self::Success { width, .. } => *width,
            Self::Warning { width, .. } => *width,
            Self::Error { width, .. } => *width,
        }
    }

    pub fn height(&self) -> f64 {
        match self {
            Self::Info { height, .. } => *height,
            Self::Success { height, .. } => *height,
            Self::Warning { height, .. } => *height,
            Self::Error { height, .. } => *height,
        }
    }
}
