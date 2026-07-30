//! Opt-in frame correlation and panic diagnostics shared by every renderer.
//!
//! The runtime owns the outer frame span. Applications and backend crates can
//! query [`current_frame_id`] and emit stages without depending on each other.

use std::backtrace::Backtrace;
use std::cell::RefCell;
use std::fmt::{self, Write as _};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Once, OnceLock};
use std::time::Instant;

const MAX_STAGE_EVENTS: usize = 256;
const MAX_COMPONENT_BYTES: usize = 64;
const MAX_STAGE_BYTES: usize = 96;
const MAX_DETAILS_BYTES: usize = 512;
const MAX_SUMMARY_DETAILS_BYTES: usize = 4_096;

#[derive(Clone)]
struct FrameContext {
    id: u64,
    started: Instant,
    profile: Option<Arc<Mutex<FrameBuffer>>>,
}

struct StageEvent {
    elapsed_us: u128,
    component: String,
    stage: String,
    details: String,
}

struct FrameBuffer {
    events: Vec<StageEvent>,
    dropped: usize,
}

impl Default for FrameBuffer {
    fn default() -> Self {
        Self {
            events: Vec::with_capacity(MAX_STAGE_EVENTS),
            dropped: 0,
        }
    }
}

impl FrameBuffer {
    fn record(&mut self, event: StageEvent) {
        if self.events.len() < MAX_STAGE_EVENTS {
            self.events.push(event);
        } else {
            self.dropped = self.dropped.saturating_add(1);
            self.events[MAX_STAGE_EVENTS - 1] = event;
        }
    }
}

thread_local! {
    static ACTIVE_FRAME: RefCell<Option<FrameContext>> = const { RefCell::new(None) };
}

static NEXT_FRAME_ID: AtomicU64 = AtomicU64::new(1);
static INSTALL_PANIC_HOOK: Once = Once::new();
static PROFILE_ENABLED: OnceLock<bool> = OnceLock::new();

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
        let enabled = profile_enabled();
        let context = FrameContext {
            id: NEXT_FRAME_ID.fetch_add(1, Ordering::Relaxed),
            started: Instant::now(),
            profile: enabled.then(|| Arc::new(Mutex::new(FrameBuffer::default()))),
        };
        if enabled {
            install_panic_backtrace_hook();
        }
        let previous = ACTIVE_FRAME.with(|active| active.replace(Some(context.clone())));
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
            flush(&self.context);
        }
        ACTIVE_FRAME.with(|active| {
            active.replace(self.previous.take());
        });
    }
}

/// Whether frame profiling was explicitly requested for this process.
pub fn profile_enabled() -> bool {
    cached_profile_enabled(&PROFILE_ENABLED, || {
        std::env::var_os("UZOR_PROFILE_FRAMES").is_some()
    })
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
            if let Some(profile) = context.profile.as_ref() {
                let event = StageEvent {
                    elapsed_us: context.started.elapsed().as_micros(),
                    component: bounded(component.to_owned(), MAX_COMPONENT_BYTES),
                    stage: bounded(stage.to_owned(), MAX_STAGE_BYTES),
                    details: bounded(
                        details.to_string(),
                        if stage == "frame_summary" {
                            MAX_SUMMARY_DETAILS_BYTES
                        } else {
                            MAX_DETAILS_BYTES
                        },
                    ),
                };
                lock_buffer(profile).record(event);
            }
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

fn cached_profile_enabled(
    cache: &OnceLock<bool>,
    read_environment: impl FnOnce() -> bool,
) -> bool {
    *cache.get_or_init(read_environment)
}

fn bounded(mut value: String, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value;
    }

    let mut end = max_bytes.saturating_sub(3);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value.truncate(end);
    value.push_str("...");
    value
}

fn lock_buffer(profile: &Mutex<FrameBuffer>) -> std::sync::MutexGuard<'_, FrameBuffer> {
    match profile.lock() {
        Ok(buffer) => buffer,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn flush(context: &FrameContext) {
    let Some(profile) = context.profile.as_ref() else {
        return;
    };
    let elapsed_us = context.started.elapsed().as_micros();
    let output = {
        let buffer = lock_buffer(profile);
        format_frame_batch(context.id, elapsed_us, &buffer)
    };

    eprint!("{output}");
}

fn format_frame_batch(frame_id: u64, elapsed_us: u128, buffer: &FrameBuffer) -> String {
    let mut output =
        String::with_capacity(256 + buffer.events.len().saturating_mul(160));
    append_profile_line(
        &mut output,
        frame_id,
        0,
        "runtime",
        "frame_begin",
        "",
    );
    for event in &buffer.events {
        append_profile_line(
            &mut output,
            frame_id,
            event.elapsed_us,
            &event.component,
            &event.stage,
            &event.details,
        );
    }
    append_profile_line(
        &mut output,
        frame_id,
        elapsed_us,
        "runtime",
        "frame_summary",
        &format!(
            "recorded_stages={} dropped_stages={}",
            buffer.events.len(),
            buffer.dropped,
        ),
    );
    append_profile_line(
        &mut output,
        frame_id,
        elapsed_us,
        "runtime",
        "frame_end",
        "",
    );
    output
}

fn append_profile_line(
    output: &mut String,
    frame_id: u64,
    elapsed_us: u128,
    component: &str,
    stage: &str,
    details: &str,
) {
    writeln!(
        output,
        "[uzor-frame-profile] frame_id={} elapsed_us={} component={} stage={} {}",
        frame_id,
        elapsed_us,
        component,
        stage,
        details,
    )
    .expect("writing a frame profile into a String cannot fail");
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

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::sync::OnceLock;

    use super::{
        bounded, cached_profile_enabled, current_frame_id, format_frame_batch, FrameBuffer,
        FrameProfileGuard, StageEvent, MAX_DETAILS_BYTES, MAX_STAGE_EVENTS,
        MAX_SUMMARY_DETAILS_BYTES,
    };

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

    #[test]
    fn profile_enablement_is_read_once() {
        let cache = OnceLock::new();
        let reads = Cell::new(0);

        assert!(cached_profile_enabled(&cache, || {
            reads.set(reads.get() + 1);
            true
        }));
        assert!(cached_profile_enabled(&cache, || {
            reads.set(reads.get() + 1);
            false
        }));
        assert_eq!(reads.get(), 1);
    }

    #[test]
    fn frame_batch_is_bounded_and_keeps_latest_stage() {
        let mut buffer = FrameBuffer::default();
        for index in 0..MAX_STAGE_EVENTS + 3 {
            buffer.record(StageEvent {
                elapsed_us: index as u128,
                component: "urx_native_wgpu".to_owned(),
                stage: format!("stage_{index}"),
                details: String::new(),
            });
        }

        let output = format_frame_batch(42, 999, &buffer);
        assert_eq!(buffer.events.len(), MAX_STAGE_EVENTS);
        assert_eq!(buffer.dropped, 3);
        assert!(output.contains(
            "[uzor-frame-profile] frame_id=42 elapsed_us=0 component=runtime stage=frame_begin",
        ));
        assert!(output.contains("component=urx_native_wgpu stage=stage_0"));
        assert!(output.contains(&format!(
            "component=urx_native_wgpu stage=stage_{}",
            MAX_STAGE_EVENTS + 2,
        )));
        assert!(output.contains(
            "component=runtime stage=frame_summary recorded_stages=256 dropped_stages=3",
        ));
        assert!(output.ends_with(
            "frame_id=42 elapsed_us=999 component=runtime stage=frame_end \n",
        ));
    }

    #[test]
    fn summary_detail_budget_preserves_structured_renderer_metrics() {
        let details = "x".repeat(MAX_DETAILS_BYTES + 1);
        assert!(bounded(details.clone(), MAX_DETAILS_BYTES).ends_with("..."));
        assert_eq!(
            bounded(details.clone(), MAX_SUMMARY_DETAILS_BYTES),
            details,
        );
    }
}
