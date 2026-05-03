//! Atomic widget builders.
//!
//! Atomics are simpler than composites — each builder wraps a single
//! `register_layout_manager_*` call from `uzor::ui::widgets::atomic::*`.
//! Most atomics need only an id, a rect, and a label; defaults handle the
//! rest.
//!
//! For atomics not covered here (rare ones — `chevron`, `tooltip`,
//! `shape_selector`, etc.) call the raw `register_layout_manager_*` re-export
//! via [`super::raw`] directly.

use uzor::core::types::Rect;
use uzor::docking::panels::DockPanel;
use uzor::layout::{LayoutManager, LayoutNodeId};
use uzor::render::RenderContext;
use uzor::types::{WidgetId, WidgetState};

// =============================================================================
// Button
// =============================================================================

use uzor::ui::widgets::atomic::button::input::register_layout_manager_button;
use uzor::ui::widgets::atomic::button::render::ButtonView;
use uzor::ui::widgets::atomic::button::settings::ButtonSettings;
use uzor::types::IconId;

/// Chainable builder for an atomic button.
///
/// Reactive options:
/// - `.on_click(|| ...)` — closure invoked when the widget is clicked in the
///   current frame.  No need to handle the click via `App::on_*`.
/// - `.bind_count(&mut u32)` — increments a counter per click (for "increment"
///   style demos).
pub struct ButtonBuilder<'a> {
    id:            WidgetId,
    rect:          Rect,
    parent:        LayoutNodeId,
    text:          Option<&'a str>,
    icon:          Option<&'a IconId>,
    active:        bool,
    disabled:      bool,
    widget_state:  Option<WidgetState>,
    settings:      Option<ButtonSettings>,
    on_click:      Option<Box<dyn FnOnce() + 'a>>,
    bind_count:    Option<&'a mut u32>,
}

/// Entry point: build a button at the given id + rect.
pub fn button<'a>(id: impl Into<WidgetId>, rect: Rect) -> ButtonBuilder<'a> {
    ButtonBuilder {
        id: id.into(),
        rect,
        parent:       LayoutNodeId::ROOT,
        text:         None,
        icon:         None,
        active:       false,
        disabled:     false,
        widget_state: None,
        settings:     None,
        on_click:     None,
        bind_count:   None,
    }
}

impl<'a> ButtonBuilder<'a> {
    pub fn parent(mut self, p: LayoutNodeId) -> Self { self.parent = p; self }
    pub fn text(mut self, t: &'a str) -> Self { self.text = Some(t); self }
    pub fn icon(mut self, i: &'a IconId) -> Self { self.icon = Some(i); self }
    pub fn active(mut self, on: bool) -> Self { self.active = on; self }
    pub fn disabled(mut self, on: bool) -> Self { self.disabled = on; self }
    pub fn state(mut self, s: WidgetState) -> Self { self.widget_state = Some(s); self }
    pub fn settings(mut self, s: ButtonSettings) -> Self { self.settings = Some(s); self }

    /// Reactive on-click closure.  Invoked at `.build()` if the widget was
    /// clicked this frame.  Replaces the need for an `App::on_*` callback.
    pub fn on_click(mut self, f: impl FnOnce() + 'a) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }

    /// Reactive counter — increments on each click.  Useful for click-counter
    /// demos and toolbar action telemetry.
    pub fn bind_count(mut self, n: &'a mut u32) -> Self { self.bind_count = Some(n); self }

    pub fn build<P: DockPanel>(
        self,
        layout: &mut LayoutManager<P>,
        render: &mut dyn RenderContext,
    ) {
        // Invoke reactive callbacks if clicked this frame (and not disabled).
        if !self.disabled && layout.was_clicked(&self.id) {
            if let Some(cb) = self.on_click {
                cb();
            }
            if let Some(n) = self.bind_count {
                *n = n.wrapping_add(1);
            }
        }

        let view = ButtonView {
            icon: self.icon,
            text: self.text,
            active: self.active,
            disabled: self.disabled,
            active_border: None,
            hover_chevron: None,
        };
        let settings = self.settings.unwrap_or_default();
        let ws = self.widget_state.unwrap_or_else(|| {
            // Pull live state from coordinator if available.
            layout.ctx().input.widget_state(&self.id)
        });
        register_layout_manager_button(
            layout, render, self.parent, self.id, self.rect, ws, &view, &settings,
        );
    }
}

// =============================================================================
// Text
// =============================================================================

use uzor::ui::widgets::atomic::text::input::register_layout_manager_text;
use uzor::ui::widgets::atomic::text::settings::TextSettings;
use uzor::ui::widgets::atomic::text::types::{TextOverflow, TextView};
use uzor::render::{TextAlign, TextBaseline};

/// Chainable builder for a text label.
pub struct TextBuilder<'a> {
    id:       WidgetId,
    rect:     Rect,
    parent:   LayoutNodeId,
    text:     &'a str,
    color:    Option<&'a str>,
    align:    TextAlign,
    baseline: TextBaseline,
    overflow: TextOverflow,
    settings: Option<TextSettings>,
}

/// Entry point: build a text label at the given id + rect.
pub fn text<'a>(id: impl Into<WidgetId>, rect: Rect, text: &'a str) -> TextBuilder<'a> {
    TextBuilder {
        id: id.into(),
        rect,
        parent:   LayoutNodeId::ROOT,
        text,
        color:    None,
        align:    TextAlign::Left,
        baseline: TextBaseline::Middle,
        overflow: TextOverflow::Clip,
        settings: None,
    }
}

impl<'a> TextBuilder<'a> {
    pub fn parent(mut self, p: LayoutNodeId) -> Self { self.parent = p; self }
    pub fn color(mut self, c: &'a str) -> Self { self.color = Some(c); self }
    pub fn align(mut self, a: TextAlign) -> Self { self.align = a; self }
    pub fn baseline(mut self, b: TextBaseline) -> Self { self.baseline = b; self }
    pub fn overflow(mut self, o: TextOverflow) -> Self { self.overflow = o; self }
    pub fn settings(mut self, s: TextSettings) -> Self { self.settings = Some(s); self }

    pub fn build<P: DockPanel>(
        self,
        layout: &mut LayoutManager<P>,
        render: &mut dyn RenderContext,
    ) {
        let view = TextView {
            text:     self.text,
            color:    self.color,
            align:    self.align,
            baseline: self.baseline,
            font:     None,
            overflow: self.overflow,
            hovered:  false,
        };
        let settings = self.settings.unwrap_or_default();
        register_layout_manager_text(
            layout, render, self.parent, self.id, self.rect,
            WidgetState::Normal, &view, &settings,
        );
    }
}

// =============================================================================
// Checkbox
// =============================================================================

use uzor::ui::widgets::atomic::checkbox::input::register_layout_manager_checkbox;
use uzor::ui::widgets::atomic::checkbox::settings::CheckboxSettings;
use uzor::ui::widgets::atomic::checkbox::types::{CheckboxRenderKind, CheckboxView};

/// Chainable builder for a checkbox.
///
/// Two ways to wire state:
/// - `.checked(bool)` + handle click yourself in `App::on_unhandled_click`.
/// - `.bind(&mut bool)` — reactive: builder reads the value for paint AND
///   toggles it on click in the same frame.  The app does not write a click
///   handler.
pub struct CheckboxBuilder<'a> {
    id:       WidgetId,
    rect:     Rect,
    parent:   LayoutNodeId,
    checked:  bool,
    bind:     Option<&'a mut bool>,
    label:    Option<&'a str>,
    settings: Option<CheckboxSettings>,
    kind:     Option<CheckboxRenderKind<'a>>,
    font:     &'a str,
}

pub fn checkbox<'a>(id: impl Into<WidgetId>, rect: Rect) -> CheckboxBuilder<'a> {
    CheckboxBuilder {
        id: id.into(),
        rect,
        parent:   LayoutNodeId::ROOT,
        checked:  false,
        bind:     None,
        label:    None,
        settings: None,
        kind:     None,
        font:     "13px sans-serif",
    }
}

impl<'a> CheckboxBuilder<'a> {
    pub fn parent(mut self, p: LayoutNodeId) -> Self { self.parent = p; self }
    pub fn checked(mut self, on: bool) -> Self { self.checked = on; self }

    /// Reactive binding: the builder reads `*flag` for paint AND, if the
    /// widget is clicked this frame, flips `*flag = !*flag` before paint.
    /// The app does not need any click handler for this checkbox.
    pub fn bind(mut self, flag: &'a mut bool) -> Self { self.bind = Some(flag); self }

    pub fn label(mut self, l: &'a str) -> Self { self.label = Some(l); self }
    pub fn settings(mut self, s: CheckboxSettings) -> Self { self.settings = Some(s); self }
    pub fn kind(mut self, k: CheckboxRenderKind<'a>) -> Self { self.kind = Some(k); self }
    pub fn font(mut self, f: &'a str) -> Self { self.font = f; self }

    pub fn build<P: DockPanel>(
        self,
        layout: &mut LayoutManager<P>,
        render: &mut dyn RenderContext,
    ) {
        // Resolve checked state: bind takes priority; toggle on click.
        let checked = if let Some(flag) = self.bind {
            if layout.was_clicked(&self.id) {
                *flag = !*flag;
            }
            *flag
        } else {
            self.checked
        };

        let view = CheckboxView {
            checked,
            label:   self.label,
        };
        let settings = self.settings.unwrap_or_default();
        let kind     = self.kind.unwrap_or(CheckboxRenderKind::Standard);
        let ws = layout.ctx().input.widget_state(&self.id);
        register_layout_manager_checkbox(
            layout, render, self.parent, self.id, self.rect, ws, &view, &settings, &kind, self.font,
        );
    }
}

// =============================================================================
// Toggle
// =============================================================================

use uzor::ui::widgets::atomic::toggle::input::register_layout_manager_toggle;
use uzor::ui::widgets::atomic::toggle::settings::ToggleSettings;
use uzor::ui::widgets::atomic::toggle::types::{ToggleRenderKind, ToggleView};

pub struct ToggleBuilder<'a> {
    id:       WidgetId,
    rect:     Rect,
    parent:   LayoutNodeId,
    toggled:  bool,
    bind:     Option<&'a mut bool>,
    label:    Option<&'a str>,
    disabled: bool,
    settings: Option<ToggleSettings>,
    kind:     Option<ToggleRenderKind<'a>>,
}

pub fn toggle<'a>(id: impl Into<WidgetId>, rect: Rect) -> ToggleBuilder<'a> {
    ToggleBuilder {
        id: id.into(),
        rect,
        parent:   LayoutNodeId::ROOT,
        toggled:  false,
        bind:     None,
        label:    None,
        disabled: false,
        settings: None,
        kind:     None,
    }
}

impl<'a> ToggleBuilder<'a> {
    pub fn parent(mut self, p: LayoutNodeId) -> Self { self.parent = p; self }
    pub fn toggled(mut self, on: bool) -> Self { self.toggled = on; self }

    /// Reactive binding — see [`CheckboxBuilder::bind`].
    pub fn bind(mut self, flag: &'a mut bool) -> Self { self.bind = Some(flag); self }

    pub fn label(mut self, l: &'a str) -> Self { self.label = Some(l); self }
    pub fn disabled(mut self, on: bool) -> Self { self.disabled = on; self }
    pub fn settings(mut self, s: ToggleSettings) -> Self { self.settings = Some(s); self }
    pub fn kind(mut self, k: ToggleRenderKind<'a>) -> Self { self.kind = Some(k); self }

    pub fn build<P: DockPanel>(
        self,
        layout: &mut LayoutManager<P>,
        render: &mut dyn RenderContext,
    ) {
        let toggled = if let Some(flag) = self.bind {
            if layout.was_clicked(&self.id) && !self.disabled {
                *flag = !*flag;
            }
            *flag
        } else {
            self.toggled
        };

        let view = ToggleView {
            toggled,
            label:    self.label,
            disabled: self.disabled,
        };
        let settings = self.settings.unwrap_or_default();
        let kind     = self.kind.unwrap_or(ToggleRenderKind::Switch);
        let ws = layout.ctx().input.widget_state(&self.id);
        register_layout_manager_toggle(
            layout, render, self.parent, self.id, self.rect, ws, &view, &settings, &kind,
        );
    }
}

// =============================================================================
// Separator
// =============================================================================

use uzor::ui::widgets::atomic::separator::input::register_layout_manager_separator;
use uzor::ui::widgets::atomic::separator::settings::SeparatorSettings;
use uzor::ui::widgets::atomic::separator::input::SeparatorKind;
use uzor::ui::widgets::atomic::separator::types::{SeparatorOrientation, SeparatorType};
use uzor::ui::widgets::atomic::separator::render::SeparatorView;

pub struct SeparatorBuilder {
    id:          WidgetId,
    rect:        Rect,
    parent:      LayoutNodeId,
    kind:        SeparatorKind,
    sep_type:    SeparatorType,
    hovered:     bool,
    dragging:    bool,
    settings:    Option<SeparatorSettings>,
}

pub fn separator(id: impl Into<WidgetId>, rect: Rect) -> SeparatorBuilder {
    SeparatorBuilder {
        id: id.into(),
        rect,
        parent:   LayoutNodeId::ROOT,
        kind:     SeparatorKind::Divider,
        sep_type: SeparatorType::Divider { orientation: SeparatorOrientation::Horizontal },
        hovered:  false,
        dragging: false,
        settings: None,
    }
}

impl SeparatorBuilder {
    pub fn parent(mut self, p: LayoutNodeId) -> Self { self.parent = p; self }
    pub fn kind(mut self, k: SeparatorKind) -> Self { self.kind = k; self }
    pub fn sep_type(mut self, t: SeparatorType) -> Self { self.sep_type = t; self }
    pub fn hovered(mut self, on: bool) -> Self { self.hovered = on; self }
    pub fn dragging(mut self, on: bool) -> Self { self.dragging = on; self }
    pub fn settings(mut self, s: SeparatorSettings) -> Self { self.settings = Some(s); self }

    pub fn build<P: DockPanel>(
        self,
        layout: &mut LayoutManager<P>,
        render: &mut dyn RenderContext,
    ) {
        let view = SeparatorView {
            kind:     self.sep_type,
            hovered:  self.hovered,
            dragging: self.dragging,
        };
        let settings = self.settings.unwrap_or_default();
        register_layout_manager_separator(
            layout, render, self.parent, self.id, self.rect, self.kind, &view, &settings,
        );
    }
}
