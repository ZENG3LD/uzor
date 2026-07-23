//! `uzor-examples`'s shared library target.
//!
//! Every `[[bin]]` in this package auto-links this crate's own `lib`
//! target for free (same-package rule) — that's the only reason this
//! file exists: `uzor-examples` previously had zero `[lib]` target
//! (eight `[[bin]]`s only), so bins couldn't share modules without one,
//! and a whole new crate for a small comparator utility would be the
//! kind of ceremony this project's own doctrine rejects
//! (`urx-wave6-autodetect-cutover-design-2026-07-25.md` §4.1).
//!
//! Currently exposes exactly one thing: [`parity_harness`], the
//! screenshot-diff comparator Wave 6's app-level parity gate uses.
//! Per-fixture driving code stays colocated inside each reference
//! demo's own bin file as a `#[cfg(test)] mod screenshot_diff` (see
//! `l4/figures_demo.rs`) — this lib only owns the shared, generic
//! rendering/comparison glue.

pub mod parity_harness;
