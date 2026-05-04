//! Extension trait that adds `.run()` onto `uzor::framework::AppBuilder<A, P>`.
//!
//! Import [`AppRun`] to gain `.run()` on any `AppBuilder`:
//!
//! ```rust,ignore
//! use uzor::framework::AppBuilder;
//! use uzor_desktop::AppRun as _;
//!
//! // Backend autodetected — no .backend() or .surface_factory() needed.
//! AppBuilder::new(MyApp::new())
//!     .title("my app")
//!     .size(1280, 720)
//!     .run()
//!     .expect("runtime error");
//! ```

use uzor::docking::panels::DockPanel;
use uzor::framework::app::{App, AppConfig};
use uzor::framework::builder::AppBuilder;
use uzor_render_hub::{RenderBackend, RenderSurfaceFactory};

use crate::manager::{Manager, ManagerError};

// ── AppRun ────────────────────────────────────────────────────────────────────

/// Extension trait that adds `.run()` onto [`AppBuilder`].
///
/// This is the primary entry point for running an uzor desktop app.
pub trait AppRun {
    /// Consume the builder and run the application.
    ///
    /// Blocks until all windows close (on native desktop targets).
    fn run(self) -> Result<(), ManagerError>;
}

impl<A, P> AppRun for AppBuilder<A, P>
where
    A: App<P>,
    P: DockPanel + Default + Clone + 'static,
{
    fn run(self) -> Result<(), ManagerError> {
        let built = self.build().map_err(ManagerError::Build)?;
        // Single-instance guard lives for the duration of the event loop.
        let _single_instance_guard = built
            .config
            .single_instance
            .as_deref()
            .map(crate::utils::single_instance::single_instance);
        Manager::from_built(built).run()
    }
}

// ── run_closure ───────────────────────────────────────────────────────────────

/// Quick prototype helper — build and run an app from a closure.
///
/// The closure receives `&mut LayoutManager<P>` and `&mut dyn RenderContext`.
///
/// # Example
///
/// ```rust,ignore
/// uzor_desktop::run_closure::<uzor::framework::app::NoPanel, _>(
///     |layout, render| { /* draw something */ },
///     AppConfig::default(),
///     RenderBackend::VelloGpu,
///     Box::new(uzor_render_hub::VelloGpuSurfaceFactory::new()),
/// ).expect("runtime error");
/// ```
pub fn run_closure<P, F>(
    ui: F,
    config: AppConfig,
    backend: RenderBackend,
    factory: Box<dyn RenderSurfaceFactory>,
) -> Result<(), ManagerError>
where
    P: DockPanel + Default + Clone + Send + Sync + 'static,
    F: FnMut(&mut uzor::layout::LayoutManager<P>, &mut dyn uzor::render::RenderContext)
        + 'static,
{
    use uzor::framework::app::ClosureApp;
    let mut mgr = Manager::new(
        ClosureApp::<P, F>::new(ui),
        config,
        backend,
        None,
    );
    mgr.set_surface_factory(factory);
    // Queue a default window from config defaults.
    use uzor::framework::multi_window::{WindowKey, WindowSpec};
    mgr.queue_window_spec(
        WindowSpec::new(WindowKey::new("main"), "uzor")
            .size(1280, 720),
    );
    mgr.run()
}
