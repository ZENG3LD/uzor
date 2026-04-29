//! Tests for the three modal registration levels:
//! L1 — `register_input_coordinator_modal` (InputCoordinator)
//! L2 — `register_context_manager_modal`   (ContextManager)
//! L3 — `register_layout_manager_modal`    (LayoutManager)

use crate::docking::panels::DockPanel;
use crate::input::{InputCoordinator, WidgetKind};
use crate::input::core::coordinator::LayerId;
use crate::layout::{LayoutManager, OverlayEntry, OverlayKind};
use crate::render::{RenderContext, TextAlign, TextBaseline};
use crate::types::{Rect, WidgetId};

use super::input::{register_input_coordinator_modal, register_layout_manager_modal};
use super::render::register_context_manager_modal;
use super::settings::ModalSettings;
use super::state::ModalState;
use super::types::{BackdropKind, ModalRenderKind, ModalView};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect::new(x, y, w, h)
}

/// Minimal no-op render context for tests.  All draw calls are discarded.
struct NoopRender;

impl RenderContext for NoopRender {
    fn dpr(&self) -> f64 { 1.0 }

    fn set_stroke_color(&mut self, _color: &str) {}
    fn set_stroke_width(&mut self, _width: f64) {}
    fn set_line_dash(&mut self, _pattern: &[f64]) {}
    fn set_line_cap(&mut self, _cap: &str) {}
    fn set_line_join(&mut self, _join: &str) {}

    fn set_fill_color(&mut self, _color: &str) {}
    fn set_global_alpha(&mut self, _alpha: f64) {}

    fn begin_path(&mut self) {}
    fn move_to(&mut self, _x: f64, _y: f64) {}
    fn line_to(&mut self, _x: f64, _y: f64) {}
    fn close_path(&mut self) {}
    fn rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) {}
    fn arc(&mut self, _cx: f64, _cy: f64, _r: f64, _s: f64, _e: f64) {}
    fn ellipse(&mut self, _cx: f64, _cy: f64, _rx: f64, _ry: f64, _rot: f64, _s: f64, _e: f64) {}
    fn quadratic_curve_to(&mut self, _cpx: f64, _cpy: f64, _x: f64, _y: f64) {}
    fn bezier_curve_to(&mut self, _cp1x: f64, _cp1y: f64, _cp2x: f64, _cp2y: f64, _x: f64, _y: f64) {}

    fn stroke(&mut self) {}
    fn fill(&mut self) {}
    fn clip(&mut self) {}

    fn stroke_rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) {}
    fn fill_rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) {}

    fn set_font(&mut self, _font: &str) {}
    fn set_text_align(&mut self, _align: TextAlign) {}
    fn set_text_baseline(&mut self, _baseline: TextBaseline) {}
    fn fill_text(&mut self, _text: &str, _x: f64, _y: f64) {}
    fn stroke_text(&mut self, _text: &str, _x: f64, _y: f64) {}
    fn measure_text(&self, _text: &str) -> f64 { 0.0 }

    fn save(&mut self) {}
    fn restore(&mut self) {}
    fn translate(&mut self, _x: f64, _y: f64) {}
    fn rotate(&mut self, _angle: f64) {}
    fn scale(&mut self, _x: f64, _y: f64) {}
}

/// Minimal DockPanel for LayoutManager<P>.
#[derive(Clone, Debug)]
struct DummyPanel;

impl DockPanel for DummyPanel {
    fn title(&self) -> &str { "dummy" }
    fn type_id(&self) -> &'static str { "dummy" }
}

/// Build a minimal `ModalView` whose body closure does nothing.
fn plain_view() -> ModalView<'static> {
    ModalView {
        title: None,
        tabs: &[],
        footer_buttons: &[],
        wizard_pages: &[],
        backdrop: BackdropKind::None,
        body: Box::new(|_render, _rect, _coord| {}),
    }
}

// ---------------------------------------------------------------------------
// Test 1 — L1: InputCoordinator
// ---------------------------------------------------------------------------

/// Calling `register_input_coordinator_modal` must register the modal composite
/// in the coordinator and return a WidgetId of kind Modal.
#[test]
fn modal_l1_registers_in_input_coordinator() {
    let mut coord    = InputCoordinator::new();
    let     state    = ModalState::default();
    let     view     = plain_view();
    let     settings = ModalSettings::default();
    let     kind     = ModalRenderKind::Plain;
    let     layer    = LayerId::modal();
    let     modal_rect = rect(100.0, 100.0, 400.0, 300.0);

    let modal_id: WidgetId = register_input_coordinator_modal(
        &mut coord,
        "test-modal-l1",
        modal_rect,
        &state,
        &view,
        &settings,
        &kind,
        &layer,
    );

    // The composite must have been registered.
    assert_eq!(
        coord.widget_kind(&modal_id),
        Some(WidgetKind::Modal),
        "modal composite must be registered with kind Modal",
    );

    // The rect stored must match what we passed in.
    let stored = coord.widget_rect(&modal_id)
        .expect("registered modal must have a rect");
    assert_eq!(stored, modal_rect, "stored rect must equal the rect passed to registration");

    // Idempotency: a second call (different id) also succeeds without panic.
    let _ = register_input_coordinator_modal(
        &mut coord,
        "test-modal-l1-b",
        rect(200.0, 200.0, 300.0, 200.0),
        &state,
        &view,
        &settings,
        &kind,
        &layer,
    );
}

// ---------------------------------------------------------------------------
// Test 2 — L2: ContextManager (via register_context_manager_modal)
// ---------------------------------------------------------------------------

/// Calling `register_context_manager_modal` must wire the modal through the
/// context manager's embedded InputCoordinator.
#[test]
fn modal_l2_registers_via_context_manager() {
    use crate::app_context::ContextManager;
    use crate::app_context::layout::types::LayoutNode;

    let mut ctx      = ContextManager::new(LayoutNode::new("test-root"));
    let mut render   = NoopRender;
    let mut state    = ModalState::default();
    let mut view     = plain_view();
    let     settings = ModalSettings::default();
    let     kind     = ModalRenderKind::WithHeader;
    let     layer    = LayerId::modal();
    let     modal_rect = rect(50.0, 50.0, 600.0, 400.0);

    register_context_manager_modal(
        &mut ctx,
        &mut render,
        "test-modal-l2",
        modal_rect,
        &mut state,
        &mut view,
        &settings,
        &kind,
        &layer,
    );

    // The widget must be visible via ctx.input.
    let id = WidgetId::new("test-modal-l2");
    assert_eq!(
        ctx.input.widget_kind(&id),
        Some(WidgetKind::Modal),
        "L2 must forward the modal registration to ctx.input",
    );

    // WithHeader adds close + drag children — verify at least one child is registered.
    let close_id = WidgetId::new("test-modal-l2:close");
    assert_eq!(
        ctx.input.widget_kind(&close_id),
        Some(WidgetKind::CloseButton),
        "WithHeader modal must register a CloseButton child",
    );
}

// ---------------------------------------------------------------------------
// Test 3 — L3: LayoutManager (via register_layout_manager_modal)
// ---------------------------------------------------------------------------

/// `register_layout_manager_modal` resolves the rect from the overlay stack and
/// forwards to L2.  Returns `Some(())` when the slot exists, `None` otherwise.
#[test]
fn modal_l3_resolves_rect_from_layout_manager() {
    let mut layout = LayoutManager::<DummyPanel>::new();
    let mut render = NoopRender;
    let mut state  = ModalState::default();
    let     settings = ModalSettings::default();
    let     kind   = ModalRenderKind::Plain;
    let     layer  = LayerId::modal();

    // Solve first so the layout is initialised (not strictly required for
    // overlay lookup, but mirrors realistic usage).
    layout.solve(rect(0.0, 0.0, 1920.0, 1080.0));

    // Push the overlay slot.
    let overlay_rect = rect(100.0, 100.0, 400.0, 300.0);
    layout.push_overlay(OverlayEntry {
        id:     "test-modal-l3".to_string(),
        kind:   OverlayKind::Modal,
        rect:   overlay_rect,
        anchor: None,
    });

    // Confirm the overlay rect is visible through the layout manager.
    assert_eq!(
        layout.rect_for_overlay("test-modal-l3"),
        Some(overlay_rect),
        "overlay rect must be resolvable before L3 call",
    );

    // --- Happy path: slot exists → L3 must succeed.
    let result = {
        let mut view = plain_view();
        register_layout_manager_modal(
            &mut layout,
            &mut render,
            "test-modal-l3",
            "modal-widget-l3",
            &mut state,
            &mut view,
            &settings,
            &kind,
            &layer,
        )
    };
    assert!(
        result.is_some(),
        "L3 must return Some(()) when the overlay slot exists",
    );

    // Verify the widget was also registered inside the embedded ContextManager.
    let id = WidgetId::new("modal-widget-l3");
    assert_eq!(
        layout.ctx().input.widget_kind(&id),
        Some(WidgetKind::Modal),
        "L3 must propagate registration into the embedded ContextManager",
    );

    // --- Missing-slot path: unknown id → L3 must return None without panic.
    let missing = {
        let mut view = plain_view();
        register_layout_manager_modal(
            &mut layout,
            &mut render,
            "this-slot-does-not-exist",
            "modal-widget-missing",
            &mut state,
            &mut view,
            &settings,
            &kind,
            &layer,
        )
    };
    assert!(
        missing.is_none(),
        "L3 must return None when the overlay slot is absent",
    );
}
