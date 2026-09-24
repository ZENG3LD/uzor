//! Integration-test binary entry point: one `mod` per widget under test,
//! all compiled into this single `tests/widgets` binary (Cargo only
//! auto-discovers files directly under `tests/`, so `tests/widgets/*.rs`
//! are plain modules pulled in here, not separate test binaries — keeps
//! future widgets from multiplying the number of heavy test targets).

// This file is itself the crate root of the `widgets` integration test
// binary, so plain `mod button;` would look for a sibling
// `tests/button.rs` — the explicit `#[path]` is what actually places the
// submodule under `tests/widgets/`, matching the file layout above.
#[path = "widgets/support.rs"]
mod support;

#[path = "widgets/button.rs"]
mod button;
