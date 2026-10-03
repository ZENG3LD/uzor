//! Minimal native host smoke: open two windows (F9 done-condition).
//!
//! ```text
//! cargo run -p uzor-framework --release --example native_two_windows --features native
//! ```

use uzor::layout::docking::DockPanel;
use uzor::tokens::{BuiltinSet, Tokens};
use uzor_framework::host::native::run_native_default;
use uzor_framework::{
    App, Intent, IntentCx, OverlayCx, OverlayModel, PanelCx, PanelHome, RuntimeConfig, SizePx,
    Spec, WindowId, WindowSpec,
};

#[derive(Clone, Debug)]
struct DemoPanel;

impl DockPanel for DemoPanel {
    fn title(&self) -> &str {
        "Demo"
    }
    fn type_id(&self) -> &'static str {
        "demo"
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum DemoOverlay {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum DemoAction {}

struct DemoSpec;

impl Spec for DemoSpec {
    type Panel = DemoPanel;
    type Overlay = DemoOverlay;
    type Action = DemoAction;

    fn decode_panel(_home: PanelHome, _type_id: &str) -> Option<Self::Panel> {
        Some(DemoPanel)
    }
}

struct DemoApp;

impl App for DemoApp {
    type Spec = DemoSpec;

    fn overlay_model(&self, _win: WindowId, _id: DemoOverlay) -> OverlayModel {
        OverlayModel::default()
    }

    fn panel(&mut self, _cx: &mut PanelCx<'_, Self::Spec>) {}

    fn overlay_body(&mut self, _cx: &mut OverlayCx<'_, Self::Spec>) {}

    fn intent(&mut self, _intent: Intent<Self::Spec>, _cx: &mut IntentCx<'_, Self::Spec>) {}
}

fn main() -> Result<(), uzor_framework::FrameworkError> {
    let cfg = RuntimeConfig {
        windows: vec![
            WindowSpec::new("main", "Uzor F9 — window A", SizePx::new(640, 480)),
            WindowSpec::new("aux", "Uzor F9 — window B", SizePx::new(480, 360)),
        ],
        tokens: Tokens::builtin(BuiltinSet::Dark),
        dark: true,
        ..RuntimeConfig::default()
    };
    run_native_default(DemoApp, cfg)
}
