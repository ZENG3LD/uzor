//! Tests for the three modal registration levels:
//! L1 — `register_input_coordinator_modal` (InputCoordinator)
//! L2 — `register_context_manager_modal`   (ContextManager)
//! L3 — `register_layout_manager_modal`    (LayoutManager)

use crate::layout::docking::DockPanel;
use crate::input::{InputCoordinator, WidgetKind};
use crate::input::core::coordinator::LayerId;
use crate::layout::{LayoutManager, LayoutNodeId, OverlayEntry, OverlayKind};
use crate::testing::NullRenderContext;
use crate::types::{Rect, WidgetId, CompositeId};

use super::input::{register_input_coordinator_modal, register_layout_manager_modal};
use super::render::register_context_manager_modal;
use super::settings::ModalSettings;
use super::state::ModalState;
use super::types::{BackdropKind, ModalRenderKind, ModalView};
use crate::layout::ModalHandle;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect::new(x, y, w, h)
}


/// Minimal DockPanel for LayoutManager<P>.
#[derive(Clone, Debug)]
struct DummyPanel;

impl DockPanel for DummyPanel {
    fn title(&self) -> &str { "dummy" }
    fn type_id(&self) -> &'static str { "dummy" }
}

/// Build a minimal `ModalView` for tests.
fn plain_view() -> ModalView<'static> {
    ModalView {
        title: None,
        tabs: &[],
        footer_buttons: &[],
        wizard_pages: &[],
        backdrop: BackdropKind::None,
        overflow: crate::types::OverflowMode::Clip,
        resizable: false,
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
    let mut state    = ModalState::default();
    let     view     = plain_view();
    let     settings = ModalSettings::default();
    let     kind     = ModalRenderKind::Plain;
    let     layer    = LayerId::modal();
    let     modal_rect = rect(100.0, 100.0, 400.0, 300.0);

    let modal_id: CompositeId = register_input_coordinator_modal(
        &mut coord,
        "test-modal-l1",
        modal_rect,
        &mut state,
        &view,
        &settings,
        &kind,
        &layer,
    );

    // The composite must have been registered.
    assert_eq!(
        coord.widget_kind(modal_id.as_widget_id()),
        Some(WidgetKind::Modal),
        "modal composite must be registered with kind Modal",
    );

    // The rect stored must match what we passed in.
    let stored = coord.widget_rect(modal_id.as_widget_id())
        .expect("registered modal must have a rect");
    assert_eq!(stored, modal_rect, "stored rect must equal the rect passed to registration");

    // Idempotency: a second call (different id) also succeeds without panic.
    let _ = register_input_coordinator_modal(
        &mut coord,
        "test-modal-l1-b",
        rect(200.0, 200.0, 300.0, 200.0),
        &mut state,
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
    let mut render   = NullRenderContext;
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
#[ignore = "requires LayoutManager::set_current_window thread-local setup; \
            covered end-to-end by integration tests"]
fn modal_l3_resolves_rect_from_layout_manager() {
    let mut layout = LayoutManager::<DummyPanel>::new();
    let mut render = NullRenderContext;
    let     settings = ModalSettings::default();
    let     kind   = ModalRenderKind::Plain;
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
    let handle_l3 = ModalHandle { id: WidgetId::new("modal-widget-l3") };
    let result = {
        let mut view = plain_view();
        register_layout_manager_modal(
            &mut layout,
            &mut render,
            LayoutNodeId::ROOT,
            "test-modal-l3",
            &handle_l3,
            overlay_rect,
            None,
            &mut view,
            &settings,
            &kind,
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
    let handle_missing = ModalHandle { id: WidgetId::new("modal-widget-missing") };
    let missing = {
        let mut view = plain_view();
        register_layout_manager_modal(
            &mut layout,
            &mut render,
            LayoutNodeId::ROOT,
            "this-slot-does-not-exist",
            &handle_missing,
            rect(0.0, 0.0, 0.0, 0.0),
            None,
            &mut view,
            &settings,
            &kind,
        )
    };
    assert!(
        missing.is_none(),
        "L3 must return None when the overlay slot is absent",
    );
}
