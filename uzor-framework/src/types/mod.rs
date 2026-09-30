//! ROLE types: pure data definitions shared by every other module.
//!
//! Owns nothing. May import `std`, `serde` and `uzor` leaf types only
//! (geometry, input enums, docking ids, tokens, cadence types); never another
//! framework module, never an OS or UI toolkit (bans F1-F5, F9).
//!
//! | Module | Contents |
//! |---|---|
//! | [`ids`] | `Copy` identity and scalar newtypes: windows, revisions, time, tickets |
//! | [`bus`] | the one input envelope hosts push, and its per-kind payloads |
//! | [`command`] | [`AppCommand`](command::AppCommand) and the per-domain command vocabularies |
//! | [`intent`] | typed intents the kernel delivers to the app |
//! | [`snapshot`] | the published [`VisualSnapshot`](snapshot::VisualSnapshot) and its per-window views |
//! | [`window`] | [`WindowCommand`](window::WindowCommand), window spec / geometry, cursor / IME enums |
//! | [`frame`] | frame requests, region plans, loop wake-up |
//! | [`layout_blob`] | opaque serialized dock layout and its codec error |
//! | [`spec`] | the app's vocabulary trait [`Spec`](spec::Spec) |
//! | [`error`] | [`FrameworkError`](error::FrameworkError) |

pub mod bus;
pub mod command;
pub mod error;
pub mod frame;
pub mod ids;
pub mod intent;
pub mod layout_blob;
pub mod snapshot;
pub mod spec;
pub mod window;
