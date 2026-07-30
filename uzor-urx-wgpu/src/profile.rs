use std::fmt;

pub(crate) struct FrameGuard {
    id: u64,
    enabled: bool,
    fallback: Option<uzor::diagnostics::FrameProfileGuard>,
}

impl FrameGuard {
    pub(crate) fn enter() -> Self {
        let fallback = if uzor::diagnostics::current_frame_id().is_none() {
            Some(uzor::diagnostics::FrameProfileGuard::enter())
        } else {
            None
        };
        let id = uzor::diagnostics::current_frame_id()
            .expect("runtime or renderer fallback must establish frame correlation");
        let enabled = uzor::diagnostics::profile_enabled();
        if enabled {
            stage(
                "render_begin",
                format_args!(
                    "thread={:?} correlation_source={}",
                    std::thread::current().id(),
                    if fallback.is_some() { "renderer_fallback" } else { "runtime" },
                ),
            );
        }
        Self { id, enabled, fallback }
    }

    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    pub(crate) fn enabled(&self) -> bool {
        self.enabled
    }
}

impl Drop for FrameGuard {
    fn drop(&mut self) {
        if self.enabled {
            stage("render_scope_end", format_args!(""));
        }
        let _ = self.fallback.take();
    }
}

pub(crate) fn enabled() -> bool {
    uzor::diagnostics::profile_enabled()
}

pub(crate) fn stage(stage: &'static str, details: fmt::Arguments<'_>) {
    uzor::diagnostics::stage("urx_native_wgpu", stage, details);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_guard_reuses_runtime_frame_correlation() {
        let runtime = uzor::diagnostics::FrameProfileGuard::enter();
        let renderer = FrameGuard::enter();

        assert_eq!(renderer.id(), runtime.id());
        assert!(renderer.fallback.is_none());
    }
}
