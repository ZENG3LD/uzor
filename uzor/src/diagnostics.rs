//! Opt-in frame correlation and panic diagnostics shared by every renderer.
//!
//! The runtime owns the outer frame span. Applications and backend crates can
//! query [`current_frame_id`] and emit stages without depending on each other.

use std::backtrace::Backtrace;
use std::cell::RefCell;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Once;
use std::time::Instant;

#[derive(Clone)]
struct FrameContext {
    id: u64,
    started: Instant,
}

thread_local! {
    static ACTIVE_FRAME: RefCell<Option<FrameContext>> = const { RefCell::new(None) };
}

static NEXT_FRAME_ID: AtomicU64 = AtomicU64::new(1);
static INSTALL_PANIC_HOOK: Once = Once::new();

/// Restores the previous frame context when the runtime finishes one tick.
#[must_use]
pub struct FrameProfileGuard {
    context: FrameContext,
    previous: Option<FrameContext>,
    enabled: bool,
}

impl FrameProfileGuard {
    /// Start one correlated application-build and renderer-submit span.
    pub fn enter() -> Self {
        let context = FrameContext {
            id: NEXT_FRAME_ID.fetch_add(1, Ordering::Relaxed),
            started: Instant::now(),
        };
        let enabled = profile_enabled();
        if enabled {
            install_panic_backtrace_hook();
        }
        let previous = ACTIVE_FRAME.with(|active| active.replace(Some(context.clone())));
        if enabled {
            emit(&context, "runtime", "frame_begin", format_args!(""));
        }
        Self {
            context,
            previous,
            enabled,
        }
    }

    /// Correlation identifier assigned to this runtime frame.
    pub fn id(&self) -> u64 {
        self.context.id
    }
}

impl Drop for FrameProfileGuard {
    fn drop(&mut self) {
        if self.enabled {
            emit(&self.context, "runtime", "frame_end", format_args!(""));
        }
        ACTIVE_FRAME.with(|active| {
            active.replace(self.previous.take());
        });
    }
}

/// Whether frame profiling was explicitly requested for this process.
pub fn profile_enabled() -> bool {
    std::env::var_os("UZOR_PROFILE_FRAMES").is_some()
}

/// ID of the frame currently built or submitted on this thread.
pub fn current_frame_id() -> Option<u64> {
    ACTIVE_FRAME.with(|active| active.borrow().as_ref().map(|context| context.id))
}

/// Emit a machine-comparable stage marker for the current frame.
pub fn stage(component: &str, stage: &str, details: fmt::Arguments<'_>) {
    if !profile_enabled() {
        return;
    }
    ACTIVE_FRAME.with(|active| {
        if let Some(context) = active.borrow().as_ref() {
            emit(context, component, stage, details);
        } else {
            eprintln!(
                "[uzor-frame-profile] frame_id=none elapsed_us=none component={} stage={} {}",
                component,
                stage,
                details,
            );
        }
    });
}

/// Install an opt-in panic hook that records the active frame and a forced
/// backtrace before delegating to the process's existing hook.
pub fn install_panic_backtrace_hook() {
    INSTALL_PANIC_HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let frame_id = current_frame_id()
                .map(|id| id.to_string())
                .unwrap_or_else(|| "none".to_owned());
            eprintln!(
                "[uzor-frame-panic] frame_id={} thread={:?} panic={}",
                frame_id,
                std::thread::current().id(),
                info,
            );
            eprintln!(
                "[uzor-frame-panic-backtrace] frame_id={}\n{}",
                frame_id,
                Backtrace::force_capture(),
            );
            previous(info);
        }));
    });
}

fn emit(
    context: &FrameContext,
    component: &str,
    stage: &str,
    details: fmt::Arguments<'_>,
) {
    eprintln!(
        "[uzor-frame-profile] frame_id={} elapsed_us={} component={} stage={} {}",
        context.id,
        context.started.elapsed().as_micros(),
        component,
        stage,
        details,
    );
}

#[cfg(test)]
mod tests {
    use super::{current_frame_id, FrameProfileGuard};

    #[test]
    fn nested_frame_guard_restores_previous_correlation() {
        assert_eq!(current_frame_id(), None);
        let outer = FrameProfileGuard::enter();
        assert_eq!(current_frame_id(), Some(outer.id()));
        {
            let inner = FrameProfileGuard::enter();
            assert_eq!(current_frame_id(), Some(inner.id()));
            assert_ne!(inner.id(), outer.id());
        }
        assert_eq!(current_frame_id(), Some(outer.id()));
        drop(outer);
        assert_eq!(current_frame_id(), None);
    }
}
