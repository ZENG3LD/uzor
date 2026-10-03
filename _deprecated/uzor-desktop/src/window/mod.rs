//! Window infrastructure: winit creation, vello surface + renderer, multi-window
//! manager.

pub mod config;
pub mod creation;
pub mod manager;
pub mod state;

pub use config::{WindowConfig, WindowGeom};
pub use creation::{create_window, WindowCreateError};
pub use manager::WindowManager;
pub use state::WindowState;
