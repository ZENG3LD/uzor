//! ROLE engine: one single writer per on-screen domain.
//!
//! Every engine owns its storage in private fields and has the same outer
//! shape (design §3.0), which keeps the future kernel a conductor with no
//! domain logic:
//!
//! | Method | Meaning |
//! |---|---|
//! | `apply(op) -> SmallVec<[Effect; 2]>` | the only mutation door; `op` is the engine's closed op enum from [`types::ops`](crate::types::ops) |
//! | `tick(now, ..) -> SmallVec<[Effect; 2]>` | where time advances state; `now` is an argument, never read from a clock |
//! | `revision() -> Revision` | bumped exactly when the engine's state changes |
//! | `view() -> XEngineView<'_>` | read-only projection for publish and for other engines' ticks |
//!
//! Engines never import each other, the kernel, the handle, the runtime or a
//! host; cross-engine reads are immutable views passed as arguments and
//! cross-engine writes never happen (the kernel conducts effects into ops).
//! Engines are pure, synchronous and deterministic.
//!
//! | Module | Engine | Owns |
//! |---|---|---|
//! | [`windows`] | [`WindowEngine`](windows::WindowEngine) | per-window OS-facing state, theme tokens, the output command queue |
//! | [`cadence`] | [`CadenceEngine`](cadence::CadenceEngine) | tick rates, regions, invalidation bits, the [`DeadlineWheel`](cadence::DeadlineWheel) |
//! | [`anim`] | [`AnimationEngine`](anim::AnimationEngine) | typed linear animators (expand, overlay fade) |
//! | [`input`] | [`InputEngine`](input::InputEngine) | per-window coordinator, cook, focus scopes, capture, hover / press / click, text / IME, caret blink |
//! | [`keymap`] | [`KeymapEngine`](keymap::KeymapEngine) | key bindings per scope (global / overlay / focused widget) |
//! | [`overlays`] | [`OverlayEngine`](overlays::OverlayEngine) | per-window overlay stacks (Z order, modal shield, outside / Escape dismiss, scopes) and composite widget states |
//!
//! ## Visibility
//!
//! The engines are `pub` only until the kernel exists (brief F7): with no
//! in-crate caller yet, `pub(crate)` items would be dead code. F7 narrows
//! this module and its items to `pub(crate)` so no engine is reachable from
//! outside the kernel (design §4.6, invariant 3). Nothing here is
//! re-exported from the crate root.

pub mod anim;
pub mod cadence;
pub mod input;
pub mod keymap;
pub mod overlays;
pub mod windows;
