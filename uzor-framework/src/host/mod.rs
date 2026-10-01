//! ROLE host/* (shell): the loops that drive the runtime (design §9).
//! Hosts own the OS event loop, the paint surface and the clock; they see
//! the [`Runtime`](crate::Runtime) only (ban F10). The headless host is
//! the deterministic test host: it compiles without features, runs every
//! §9.2 script, and paints into a recording context.

mod headless;

pub use headless::HeadlessHost;
