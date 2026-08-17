//! Shell framework on top of the cell engine.
//!
//! Data (card slots, catalog, keymap) + functions (paint, hit, host).
//! A new app mounts this. Existing apps stay on their own chrome.

pub mod card;
pub mod catalog;
pub mod hit;
pub mod host;
pub mod input;
pub mod paint;
pub mod theme;

pub use card::{Card, CardButton, CardFooter, CardSkin, CardTab, FooterRule};
pub use catalog::{lookup, CatalogEntry, ChromeKind, CATALOG};
pub use hit::{Hit, HitLayer, HitRegion};
pub use host::OverlayHost;
pub use input::{Command, Event, Key, Keymap, Mouse, MouseKind};
pub use paint::{paint_card, PaintedCard};
pub use theme::ShellTheme;
