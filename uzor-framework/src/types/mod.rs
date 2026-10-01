//! ROLE types: pure data definitions shared by every other module.
//!
//! Owns nothing. May import `std`, `serde` and `uzor` leaf types only
//! (geometry, input enums, docking ids, tokens, cadence types); never another
//! framework module, never an OS or UI toolkit (bans F1-F5, F9).
//!
//! | Module | Contents |
//! |---|---|
//! | [`ids`] | `Copy` identity and scalar newtypes: windows, revisions, time, tickets |
//! | [`anim`] | animator keys, expand kinds and the animation policy |
//! | [`bus`] | the one input envelope hosts push, and its per-kind payloads |
//! | [`command`] | [`AppCommand`](command::AppCommand) and the per-domain command vocabularies |
//! | [`intent`] | typed intents the kernel delivers to the app |
//! | [`snapshot`] | the published [`VisualSnapshot`](snapshot::VisualSnapshot) and its per-window views |
//! | [`window`] | [`WindowCommand`](window::WindowCommand), window spec / geometry, cursor / IME enums |
//! | [`frame`] | frame requests, region plans, loop wake-up, window surfaces |
//! | [`layout_blob`] | opaque serialized dock layout and its codec error |
//! | [`ops`] | engine operations and effects (kernel-facing, not app-facing) |
//! | [`overlay_model`] | app-declared overlay frames the kernel draws around bodies |
//! | [`spec`] | the app's vocabulary trait [`Spec`](spec::Spec) |
//! | [`error`] | [`FrameworkError`](error::FrameworkError) |

pub mod anim;
pub mod bus;
pub mod command;
pub mod error;
pub mod frame;
pub mod ids;
pub mod intent;
pub mod layout_blob;
pub mod ops;
pub mod overlay_model;
pub mod snapshot;
pub mod spec;
pub mod window;
