//! L3 — `LayoutManager` declarative API.
//!
//! Two layers:
//!
//! - **Chainable typed builders** (this module's submodules) — entry-points
//!   like `lm::modal(handle).title("Settings").build(layout, render)`.
//!   Hide `View` / `Settings` / `Kind` / parent / overlay-rect parameters
//!   under sensible defaults; expose them as opt-in chainable methods.
//!
//! - [`raw`] module — raw `register_layout_manager_*` re-exports under short
//!   `build_*` names.  Use as escape hatch when a chainable builder doesn't
//!   expose every option.
//!
//! L4 framework apps should prefer the chainable builders; only fall back to
//! [`raw`] when needed.

pub mod atomics;
pub mod blackbox;
pub mod chrome;
pub mod context_menu;
pub mod dropdown;
pub mod modal;
pub mod panel;
pub mod popup;
pub mod raw;
pub mod sidebar;
pub mod toolbar;

// ── Composite entry-point re-exports ─────────────────────────────────────────

pub use blackbox::{blackbox, stub_panel, BlackboxBuilder};
pub use chrome::{chrome, ChromeBuilder};
pub use context_menu::{context_menu, ContextMenuBuilder};
pub use dropdown::{dropdown, DropdownBuilder};
pub use modal::{modal, ModalBuilder};
pub use panel::{panel, PanelBuilder};
pub use popup::{popup, PopupBuilder};
pub use sidebar::{sidebar, SidebarBuilder};
pub use toolbar::{toolbar, ToolbarBuilder};

// ── Atomic entry-point re-exports ────────────────────────────────────────────

pub use atomics::{
    button, checkbox, separator, text, toggle,
    ButtonBuilder, CheckboxBuilder, SeparatorBuilder, TextBuilder, ToggleBuilder,
};

// ── Raw escape-hatch ─────────────────────────────────────────────────────────

pub use raw::*;
