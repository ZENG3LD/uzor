//! Modal L1 registration tests (brief C1).
//!
//! L2 / L3 registration doors were deleted in C1. Overlay routing coverage
//! lives in `uzor-framework`'s overlay engine suite.

use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, WidgetKind};
use crate::types::{CompositeId, Rect};

use super::input::register_input_coordinator_modal;
use super::settings::ModalSettings;
use super::state::ModalState;
use super::types::{BackdropKind, ModalRenderKind, ModalView};

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect::new(x, y, w, h)
}

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

/// Calling `register_input_coordinator_modal` must register the modal composite
/// in the coordinator and return a WidgetId of kind Modal.
#[test]
fn modal_l1_registers_in_input_coordinator() {
    let mut coord = InputCoordinator::new();
    let mut state = ModalState::default();
    let view = plain_view();
    let settings = ModalSettings::default();
    let kind = ModalRenderKind::Plain;
    let layer = LayerId::modal();
    let modal_rect = rect(100.0, 100.0, 400.0, 300.0);

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

    assert_eq!(
        coord.widget_kind(modal_id.as_widget_id()),
        Some(WidgetKind::Modal),
        "modal composite must be registered with kind Modal",
    );

    let stored = coord
        .widget_rect(modal_id.as_widget_id())
        .expect("registered modal must have a rect");
    assert_eq!(
        stored, modal_rect,
        "stored rect must equal the rect passed to registration"
    );

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
