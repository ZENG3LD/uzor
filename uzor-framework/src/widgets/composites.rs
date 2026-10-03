//! The composite half of the `lm` set. Each call registers the composite
//! with the hook's coordinator and paints it with the library draw
//! function. Opening an overlay is still an [`crate::OverlayCmd`] issued
//! by the app; these functions draw a frame the kernel (or the hook) asks
//! for. Blackbox panels are not here (design §7.2: the app `panel` hook
//! replaces that slot).

use uzor::ui::widgets::composite::chrome::{
    draw_chrome, register_input_coordinator_chrome, ChromeRenderKind, ChromeSettings, ChromeState,
    ChromeView,
};
use uzor::ui::widgets::composite::context_menu::{
    draw_context_menu, register_input_coordinator_context_menu, ContextMenuRenderKind,
    ContextMenuSettings, ContextMenuState, ContextMenuView,
};
use uzor::ui::widgets::composite::dropdown::{
    draw_dropdown, register_input_coordinator_dropdown, DropdownRenderKind, DropdownSettings,
    DropdownState, DropdownView,
};
use uzor::ui::widgets::composite::modal::render::draw_modal;
use uzor::ui::widgets::composite::modal::{
    register_input_coordinator_modal, ModalRenderKind, ModalSettings, ModalState, ModalView,
};
use uzor::ui::widgets::composite::panel::{
    draw_panel, register_input_coordinator_panel, PanelRenderKind, PanelSettings, PanelState,
    PanelView,
};
use uzor::ui::widgets::composite::popup::render::draw_popup;
use uzor::ui::widgets::composite::popup::{
    register_input_coordinator_popup, PopupRenderKind, PopupSettings, PopupState, PopupView,
};
use uzor::ui::widgets::composite::sidebar::{
    draw_sidebar, register_input_coordinator_sidebar, SidebarRenderKind, SidebarSettings,
    SidebarState, SidebarView,
};
use uzor::ui::widgets::composite::toolbar::render::draw_toolbar;
use uzor::ui::widgets::composite::toolbar::{
    register_input_coordinator_toolbar, ToolbarRenderKind, ToolbarSettings, ToolbarState,
    ToolbarView,
};
use uzor::Rect;
use uzor::WidgetId;

use super::host::ContentCx;

/// Register and draw a window chrome strip.
pub fn chrome<'a, C: ContentCx<'a>>(
    cx: &mut C,
    id: impl Into<WidgetId>,
    rect: Rect,
    state: &ChromeState,
    view: &ChromeView<'_>,
    settings: &ChromeSettings,
    kind: &ChromeRenderKind,
) {
    let id = id.into();
    cx.with_paint(|render, widgets, _visual, _tokens| {
        chrome_paint(render, widgets, id, rect, state, view, settings, kind);
    });
}

pub(crate) fn chrome_paint(
    render: &mut dyn uzor::render::RenderContext,
    widgets: &mut crate::handle::Widgets<'_>,
    id: WidgetId,
    rect: Rect,
    state: &ChromeState,
    view: &ChromeView<'_>,
    settings: &ChromeSettings,
    kind: &ChromeRenderKind,
) {
    let _cid = register_input_coordinator_chrome(
        widgets.coord,
        id,
        rect,
        state,
        view,
        settings,
        kind,
        &widgets.layer,
    );
    draw_chrome(render, rect, state, view, settings, kind);
}

/// Register and draw a modal frame. `state.position == (0, 0)` keeps `rect`.
pub fn modal<'a, C: ContentCx<'a>>(
    cx: &mut C,
    id: impl Into<WidgetId>,
    rect: Rect,
    state: &mut ModalState,
    view: &mut ModalView<'_>,
    settings: &ModalSettings,
    kind: &ModalRenderKind,
) {
    let id = id.into();
    cx.with_paint(|render, widgets, _visual, _tokens| {
        let _cid = register_input_coordinator_modal(
            widgets.coord,
            id,
            rect,
            state,
            view,
            settings,
            kind,
            &widgets.layer,
        );
        draw_modal(render, rect, state, view, settings, kind);
    });
}

/// Register and draw a dropdown panel.
pub fn dropdown<'a, C: ContentCx<'a>>(
    cx: &mut C,
    id: impl Into<WidgetId>,
    rect: Rect,
    state: &DropdownState,
    view: &DropdownView<'_>,
    settings: &DropdownSettings,
    kind: DropdownRenderKind,
) {
    let id = id.into();
    cx.with_paint(|render, widgets, _visual, _tokens| {
        let _cid = register_input_coordinator_dropdown(
            widgets.coord,
            id,
            rect,
            state,
            view,
            settings,
            kind,
            &widgets.layer,
        );
        draw_dropdown(render, rect, state, view, settings, kind);
    });
}

/// Register and draw a context menu. The frame origin is `state.x/y`.
pub fn context_menu<'a, C: ContentCx<'a>>(
    cx: &mut C,
    id: impl Into<WidgetId>,
    state: &ContextMenuState,
    view: &ContextMenuView<'_>,
    settings: &ContextMenuSettings,
    kind: &ContextMenuRenderKind<'_>,
) {
    let id = id.into();
    cx.with_paint(|render, widgets, _visual, _tokens| {
        let _cid = register_input_coordinator_context_menu(
            widgets.coord,
            id,
            state,
            view,
            settings,
            kind,
            &widgets.layer,
        );
        draw_context_menu(render, state, view, settings, kind);
    });
}

/// Register and draw a popup frame.
pub fn popup<'a, C: ContentCx<'a>>(
    cx: &mut C,
    id: impl Into<WidgetId>,
    rect: Rect,
    state: &mut PopupState,
    view: &mut PopupView<'_>,
    settings: &PopupSettings,
    kind: PopupRenderKind,
) {
    let id = id.into();
    cx.with_paint(|render, widgets, _visual, _tokens| {
        let _cid = register_input_coordinator_popup(
            widgets.coord,
            id,
            rect,
            state,
            view,
            settings,
            kind,
            &widgets.layer,
        );
        draw_popup(render, rect, state, view, settings, kind);
    });
}

/// Register and draw a panel frame (header / columns / body / footer per
/// `kind`). `state` is the caller's persistent scroll and sort.
pub fn panel<'a, C: ContentCx<'a>>(
    cx: &mut C,
    id: impl Into<WidgetId>,
    rect: Rect,
    state: &mut PanelState,
    view: &mut PanelView<'_>,
    settings: &PanelSettings,
    kind: &PanelRenderKind,
) {
    let id = id.into();
    cx.with_paint(|render, widgets, _visual, _tokens| {
        let cid = {
            register_input_coordinator_panel(
                widgets.coord,
                id,
                rect,
                state,
                view,
                settings,
                kind,
                &widgets.layer,
            )
        };
        draw_panel(
            render,
            rect,
            widgets.coord,
            &cid,
            state,
            view,
            settings,
            kind,
        );
    });
}

/// Register and draw an edge sidebar.
pub fn sidebar<'a, C: ContentCx<'a>>(
    cx: &mut C,
    id: impl Into<WidgetId>,
    rect: Rect,
    state: &mut SidebarState,
    view: &mut SidebarView<'_>,
    settings: &SidebarSettings,
    kind: &SidebarRenderKind,
) {
    let id = id.into();
    cx.with_paint(|render, widgets, _visual, _tokens| {
        let _cid = register_input_coordinator_sidebar(
            widgets.coord,
            id,
            rect,
            state,
            view,
            settings,
            kind,
            &widgets.layer,
        );
        draw_sidebar(render, rect, state, view, settings, kind);
    });
}

/// Register and draw a toolbar strip.
pub fn toolbar<'a, C: ContentCx<'a>>(
    cx: &mut C,
    id: impl Into<WidgetId>,
    rect: Rect,
    state: &ToolbarState,
    view: &ToolbarView<'_>,
    settings: &ToolbarSettings,
    kind: &ToolbarRenderKind,
) {
    let id = id.into();
    cx.with_paint(|render, widgets, _visual, _tokens| {
        let _cid = register_input_coordinator_toolbar(
            widgets.coord,
            id,
            rect,
            state,
            view,
            settings,
            kind,
            &widgets.layer,
        );
        draw_toolbar(render, rect, state, view, settings, kind);
    });
}
