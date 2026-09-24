//! Atomic widget builders.
//!
//! Atomics are simpler than composites — each builder wraps a single
//! `register_layout_manager_*` call from `uzor::ui::widgets::atomic::*`.
//! Most atomics need only an id, a rect, and a label; defaults handle the
//! rest.
//!
//! For atomics not covered here call the raw `register_layout_manager_*`
//! re-export via [`super::raw`] directly.

use crate::core::types::Rect;
use crate::layout::docking::DockPanel;
use crate::layout::{LayoutManager, LayoutNodeId};
use crate::render::RenderContext;
use crate::types::{WidgetId, WidgetState};

use crate::ui::widgets::atomic::button::theme::ButtonTheme;
use crate::ui::widgets::atomic::button::style::{ButtonStyle, DefaultButtonStyle};
use crate::ui::widgets::atomic::button::settings::ButtonSettings;
use crate::tokens::{Tokens, TokenTheme};

/// Builds `ButtonSettings` from the layout's live token set (H1 Brief 9,
/// §3 "StyleManager — removed") — replaces the pre-H1 `StyledButtonTheme`/
/// `StyledButtonStyle` bridge that hand-copied a handful of `StyleManager`
/// keys and fell back to a dark `TokenTheme` for the rest. `TokenTheme`
/// covers every `ButtonTheme` method uniformly from whatever token set is
/// currently active, so there is no fallback split to maintain any more.
fn button_settings_from_tokens(tokens: &std::sync::Arc<Tokens>) -> ButtonSettings {
    ButtonSettings {
        theme: Box::new(TokenTheme::new(tokens.clone())),
        style: Box::new(DefaultButtonStyle),
    }
}

// =============================================================================
// Button
// =============================================================================

use crate::ui::widgets::atomic::button::input::register_layout_manager_button;
use crate::ui::widgets::atomic::button::render::{ButtonView, HoverChevronSpec};
use crate::types::IconId;

/// Chainable builder for an atomic button.
///
/// Reactive options:
/// - `.on_click(|| ...)` — closure invoked when the widget is clicked in the
///   current frame.  No need to handle the click via `App::on_*`.
/// - `.bind_count(&mut u32)` — increments a counter per click (for "increment"
///   style demos).
pub struct ButtonBuilder<'a> {
    id:             WidgetId,
    rect:           Rect,
    parent:         LayoutNodeId,
    text:           Option<&'a str>,
    icon:           Option<&'a IconId>,
    active:         bool,
    disabled:       bool,
    active_border:  Option<bool>,
    hover_chevron:  Option<HoverChevronSpec>,
    widget_state:   Option<WidgetState>,
    settings:       Option<ButtonSettings>,
    /// Override only the colour-token bundle.
    theme_override: Option<Box<dyn ButtonTheme>>,
    /// Override only the geometry bundle.
    style_override: Option<Box<dyn ButtonStyle>>,
    on_click:       Option<Box<dyn FnOnce() + 'a>>,
    bind_count:     Option<&'a mut u32>,
}

/// Entry point: build a button at the given id + rect.
pub fn button<'a>(id: impl Into<WidgetId>, rect: Rect) -> ButtonBuilder<'a> {
    ButtonBuilder {
        id: id.into(),
        rect,
        parent:         LayoutNodeId::ROOT,
        text:           None,
        icon:           None,
        active:         false,
        disabled:       false,
        active_border:  None,
        hover_chevron:  None,
        widget_state:   None,
        settings:       None,
        theme_override: None,
        style_override: None,
        on_click:       None,
        bind_count:     None,
    }
}

impl<'a> ButtonBuilder<'a> {
    pub fn parent(mut self, p: LayoutNodeId) -> Self { self.parent = p; self }
    pub fn text(mut self, t: &'a str) -> Self { self.text = Some(t); self }
    pub fn icon(mut self, i: &'a IconId) -> Self { self.icon = Some(i); self }
    pub fn active(mut self, on: bool) -> Self { self.active = on; self }
    pub fn disabled(mut self, on: bool) -> Self { self.disabled = on; self }

    /// Per-instance override for the active-border stroke.
    /// `Some(true)` forces the border on, `Some(false)` suppresses it,
    /// `None` defers to the style-level default.
    pub fn active_border(mut self, b: Option<bool>) -> Self { self.active_border = b; self }

    /// Hover-revealed chevron — paints in the trailing corner only while the
    /// button is hovered.  Used as a "this opens a dropdown" hint.
    pub fn hover_chevron(mut self, c: HoverChevronSpec) -> Self {
        self.hover_chevron = Some(c);
        self
    }

    pub fn state(mut self, s: WidgetState) -> Self { self.widget_state = Some(s); self }
    pub fn settings(mut self, s: ButtonSettings) -> Self { self.settings = Some(s); self }

    /// Override only the button theme (colour tokens).
    pub fn theme(mut self, t: Box<dyn ButtonTheme>) -> Self {
        self.theme_override = Some(t);
        self
    }

    /// Override only the button style (geometry — radius, padding, font size …).
    pub fn style(mut self, s: Box<dyn ButtonStyle>) -> Self {
        self.style_override = Some(s);
        self
    }

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
            active_border: self.active_border,
            hover_chevron: self.hover_chevron,
        };
        let mut settings = self.settings.unwrap_or_else(|| button_settings_from_tokens(layout.tokens()));
        if let Some(t) = self.theme_override { settings.theme = t; }
        if let Some(s) = self.style_override { settings.style = s; }
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

use crate::ui::widgets::atomic::text::input::register_layout_manager_text;
use crate::ui::widgets::atomic::text::settings::TextSettings;
use crate::ui::widgets::atomic::text::style::{DefaultTextStyle, TextStyle};
use crate::ui::widgets::atomic::text::theme::TextTheme;
use crate::ui::widgets::atomic::text::types::{TextOverflow, TextView};
use crate::render::{TextAlign, TextBaseline};

fn text_settings_from_tokens(tokens: &std::sync::Arc<Tokens>) -> TextSettings {
    TextSettings {
        theme: Box::new(TokenTheme::new(tokens.clone())),
        style: Box::new(DefaultTextStyle),
    }
}

/// Chainable builder for a text label.
pub struct TextBuilder<'a> {
    id:       WidgetId,
    rect:     Rect,
    parent:   LayoutNodeId,
    text:     &'a str,
    color:    Option<&'a str>,
    align:    TextAlign,
    baseline: TextBaseline,
    font:     Option<&'a str>,
    overflow: TextOverflow,
    hovered:  bool,
    settings: Option<TextSettings>,
    /// Override only the colour-token bundle.
    theme_override: Option<Box<dyn TextTheme>>,
    /// Override only the geometry bundle.
    style_override: Option<Box<dyn TextStyle>>,
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
        font:     None,
        overflow: TextOverflow::Clip,
        hovered:  false,
        settings: None,
        theme_override: None,
        style_override: None,
    }
}

impl<'a> TextBuilder<'a> {
    pub fn parent(mut self, p: LayoutNodeId) -> Self { self.parent = p; self }
    pub fn color(mut self, c: &'a str) -> Self { self.color = Some(c); self }
    pub fn align(mut self, a: TextAlign) -> Self { self.align = a; self }
    pub fn baseline(mut self, b: TextBaseline) -> Self { self.baseline = b; self }

    /// Optional font CSS-shorthand override (e.g. `"13px Roboto"`).
    /// `None` defers to `style.font()`.
    pub fn font(mut self, f: &'a str) -> Self { self.font = Some(f); self }

    pub fn overflow(mut self, o: TextOverflow) -> Self { self.overflow = o; self }
    pub fn hovered(mut self, on: bool) -> Self { self.hovered = on; self }
    pub fn settings(mut self, s: TextSettings) -> Self { self.settings = Some(s); self }

    /// Override only the text theme (colour tokens).
    pub fn theme(mut self, t: Box<dyn TextTheme>) -> Self {
        self.theme_override = Some(t);
        self
    }

    /// Override only the text style (geometry — font shorthand).
    pub fn style(mut self, s: Box<dyn TextStyle>) -> Self {
        self.style_override = Some(s);
        self
    }

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
            font:     self.font,
            overflow: self.overflow,
            hovered:  self.hovered,
        };
        let mut settings = self.settings.unwrap_or_else(|| text_settings_from_tokens(layout.tokens()));
        if let Some(t) = self.theme_override { settings.theme = t; }
        if let Some(s) = self.style_override { settings.style = s; }
        register_layout_manager_text(
            layout, render, self.parent, self.id, self.rect,
            WidgetState::Normal, &view, &settings,
        );
    }
}

// =============================================================================
// Checkbox
// =============================================================================

use crate::ui::widgets::atomic::checkbox::input::register_layout_manager_checkbox;
use crate::ui::widgets::atomic::checkbox::settings::CheckboxSettings;
use crate::ui::widgets::atomic::checkbox::style::CheckboxStyle;
use crate::ui::widgets::atomic::checkbox::theme::CheckboxTheme;
use crate::ui::widgets::atomic::checkbox::types::{CheckboxRenderKind, CheckboxView};

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
    /// Override only the colour-token bundle.
    theme_override: Option<Box<dyn CheckboxTheme>>,
    /// Override only the geometry bundle.
    style_override: Option<Box<dyn CheckboxStyle>>,
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
        theme_override: None,
        style_override: None,
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

    /// Override only the checkbox theme (colour tokens).
    pub fn theme(mut self, t: Box<dyn CheckboxTheme>) -> Self {
        self.theme_override = Some(t);
        self
    }

    /// Override only the checkbox style (geometry — size, radius, label gap …).
    pub fn style(mut self, s: Box<dyn CheckboxStyle>) -> Self {
        self.style_override = Some(s);
        self
    }

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
        let mut settings = self.settings.unwrap_or_default();
        if let Some(t) = self.theme_override { settings.theme = t; }
        if let Some(s) = self.style_override { settings.style = s; }
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

use crate::ui::widgets::atomic::toggle::input::register_layout_manager_toggle;
use crate::ui::widgets::atomic::toggle::settings::ToggleSettings;
use crate::ui::widgets::atomic::toggle::style::{ToggleIconStyle, ToggleSwitchStyle};
use crate::ui::widgets::atomic::toggle::theme::ToggleTheme;
use crate::ui::widgets::atomic::toggle::types::{ToggleRenderKind, ToggleView};

pub struct ToggleBuilder<'a> {
    id:       WidgetId,
    rect:     Rect,
    parent:   LayoutNodeId,
    toggled:  bool,
    bind:     Option<&'a mut bool>,
    label:    Option<&'a str>,
    disabled: bool,
    settings: Option<ToggleSettings>,
    /// Override only the colour-token bundle.
    theme_override:        Option<Box<dyn ToggleTheme>>,
    /// Override only the switch-track/thumb geometry.
    switch_style_override: Option<Box<dyn ToggleSwitchStyle>>,
    /// Override only the icon-swap geometry.
    icon_style_override:   Option<Box<dyn ToggleIconStyle>>,
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
        theme_override:        None,
        switch_style_override: None,
        icon_style_override:   None,
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

    /// Override only the toggle theme (colour tokens).
    pub fn theme(mut self, t: Box<dyn ToggleTheme>) -> Self {
        self.theme_override = Some(t);
        self
    }

    /// Override only the switch-track / thumb geometry (`Switch` / `SwitchWide`).
    pub fn switch_style(mut self, s: Box<dyn ToggleSwitchStyle>) -> Self {
        self.switch_style_override = Some(s);
        self
    }

    /// Override only the icon-swap geometry (`IconSwap`).
    pub fn icon_style(mut self, s: Box<dyn ToggleIconStyle>) -> Self {
        self.icon_style_override = Some(s);
        self
    }

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
        let mut settings = self.settings.unwrap_or_default();
        if let Some(t) = self.theme_override         { settings.theme = t; }
        if let Some(s) = self.switch_style_override  { settings.switch_style = s; }
        if let Some(s) = self.icon_style_override    { settings.icon_style = s; }
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

use crate::ui::widgets::atomic::separator::input::register_layout_manager_separator;
use crate::ui::widgets::atomic::separator::settings::SeparatorSettings;
use crate::ui::widgets::atomic::separator::input::SeparatorKind;
use crate::ui::widgets::atomic::separator::style::SeparatorStyle;
use crate::ui::widgets::atomic::separator::theme::SeparatorTheme;
use crate::ui::widgets::atomic::separator::types::{SeparatorOrientation, SeparatorType};
use crate::ui::widgets::atomic::separator::render::SeparatorView;

pub struct SeparatorBuilder {
    id:          WidgetId,
    rect:        Rect,
    parent:      LayoutNodeId,
    kind:        SeparatorKind,
    sep_type:    SeparatorType,
    hovered:     bool,
    dragging:    bool,
    settings:    Option<SeparatorSettings>,
    /// Override only the colour-token bundle.
    theme_override: Option<Box<dyn SeparatorTheme>>,
    /// Override only the geometry bundle.
    style_override: Option<Box<dyn SeparatorStyle>>,
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
        theme_override: None,
        style_override: None,
    }
}

impl SeparatorBuilder {
    pub fn parent(mut self, p: LayoutNodeId) -> Self { self.parent = p; self }
    pub fn kind(mut self, k: SeparatorKind) -> Self { self.kind = k; self }
    pub fn sep_type(mut self, t: SeparatorType) -> Self { self.sep_type = t; self }
    pub fn hovered(mut self, on: bool) -> Self { self.hovered = on; self }
    pub fn dragging(mut self, on: bool) -> Self { self.dragging = on; self }
    pub fn settings(mut self, s: SeparatorSettings) -> Self { self.settings = Some(s); self }

    /// Override only the separator theme (colour tokens).
    pub fn theme(mut self, t: Box<dyn SeparatorTheme>) -> Self {
        self.theme_override = Some(t);
        self
    }

    /// Override only the separator style (geometry).
    pub fn style(mut self, s: Box<dyn SeparatorStyle>) -> Self {
        self.style_override = Some(s);
        self
    }

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
        let mut settings = self.settings.unwrap_or_default();
        if let Some(t) = self.theme_override { settings.theme = t; }
        if let Some(s) = self.style_override { settings.style = s; }
        register_layout_manager_separator(
            layout, render, self.parent, self.id, self.rect, self.kind, &view, &settings,
        );
    }
}

// =============================================================================
// Chevron
// =============================================================================

use crate::ui::widgets::atomic::chevron::input::register_layout_manager_chevron;
use crate::ui::widgets::atomic::chevron::settings::ChevronSettings;
use crate::ui::widgets::atomic::chevron::style::ChevronStyle;
use crate::ui::widgets::atomic::chevron::theme::ChevronTheme;
use crate::ui::widgets::atomic::chevron::types::{
    ChevronDirection, ChevronUseCase, ChevronView, ChevronVisualKind,
    HitAreaPolicy, PlacementPolicy, VisibilityPolicy,
};

/// Chainable builder for an atomic chevron.
///
/// Reactive: pass `.on_click(|| ...)` to handle click without writing an
/// `App::on_*` callback.  Hit area / visibility policy / direction are all
/// override-able; defaults match `ChevronUseCase::PureButton`.
pub struct ChevronBuilder<'a> {
    id:             WidgetId,
    rect:           Rect,
    parent:         LayoutNodeId,
    direction:      ChevronDirection,
    use_case:       ChevronUseCase,
    visibility:     VisibilityPolicy,
    placement:      PlacementPolicy,
    hit_area:       HitAreaPolicy,
    visual_kind:    ChevronVisualKind,
    hovered:        bool,
    pressed:        bool,
    disabled:       bool,
    active:         bool,
    glyph_override: Option<&'static str>,
    settings:       Option<ChevronSettings>,
    theme_override: Option<Box<dyn ChevronTheme>>,
    style_override: Option<Box<dyn ChevronStyle>>,
    on_click:       Option<Box<dyn FnOnce() + 'a>>,
}

/// Entry point: build a chevron at the given id + rect.
pub fn chevron<'a>(id: impl Into<WidgetId>, rect: Rect) -> ChevronBuilder<'a> {
    ChevronBuilder {
        id: id.into(),
        rect,
        parent:         LayoutNodeId::ROOT,
        direction:      ChevronDirection::Down,
        use_case:       ChevronUseCase::PureButton,
        visibility:     VisibilityPolicy::Always,
        placement:      PlacementPolicy::Standalone,
        hit_area:       HitAreaPolicy::Visual,
        visual_kind:    ChevronVisualKind::Stroked,
        hovered:        false,
        pressed:        false,
        disabled:       false,
        active:         false,
        glyph_override: None,
        settings:       None,
        theme_override: None,
        style_override: None,
        on_click:       None,
    }
}

impl<'a> ChevronBuilder<'a> {
    pub fn parent(mut self, p: LayoutNodeId) -> Self { self.parent = p; self }
    pub fn direction(mut self, d: ChevronDirection) -> Self { self.direction = d; self }
    pub fn use_case(mut self, u: ChevronUseCase) -> Self { self.use_case = u; self }
    pub fn visibility(mut self, v: VisibilityPolicy) -> Self { self.visibility = v; self }
    pub fn placement(mut self, p: PlacementPolicy) -> Self { self.placement = p; self }
    pub fn hit_area(mut self, h: HitAreaPolicy) -> Self { self.hit_area = h; self }
    pub fn visual_kind(mut self, k: ChevronVisualKind) -> Self { self.visual_kind = k; self }
    pub fn hovered(mut self, on: bool) -> Self { self.hovered = on; self }
    pub fn pressed(mut self, on: bool) -> Self { self.pressed = on; self }
    pub fn disabled(mut self, on: bool) -> Self { self.disabled = on; self }
    pub fn active(mut self, on: bool) -> Self { self.active = on; self }

    /// Override the glyph string when `visual_kind == Glyph`.  `None` defers
    /// to the per-direction default (▲▼◀▶).
    pub fn glyph_override(mut self, g: &'static str) -> Self {
        self.glyph_override = Some(g);
        self
    }

    pub fn settings(mut self, s: ChevronSettings) -> Self { self.settings = Some(s); self }

    /// Override only the chevron theme (colour tokens).
    pub fn theme(mut self, t: Box<dyn ChevronTheme>) -> Self {
        self.theme_override = Some(t);
        self
    }

    /// Override only the chevron style (geometry — stroke width, glyph font …).
    pub fn style(mut self, s: Box<dyn ChevronStyle>) -> Self {
        self.style_override = Some(s);
        self
    }

    /// Reactive on-click closure — invoked at `.build()` if the chevron was
    /// clicked this frame and is not disabled.
    pub fn on_click(mut self, f: impl FnOnce() + 'a) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }

    pub fn build<P: DockPanel>(
        self,
        layout: &mut LayoutManager<P>,
        render: &mut dyn RenderContext,
    ) {
        if !self.disabled && layout.was_clicked(&self.id) {
            if let Some(cb) = self.on_click {
                cb();
            }
        }

        let view = ChevronView {
            direction:      self.direction,
            use_case:       self.use_case,
            visibility:     self.visibility,
            placement:      self.placement,
            hit_area:       self.hit_area,
            visual_kind:    self.visual_kind,
            hovered:        self.hovered,
            pressed:        self.pressed,
            disabled:       self.disabled,
            active:         self.active,
            glyph_override: self.glyph_override,
        };
        let mut settings = self.settings.unwrap_or_default();
        if let Some(t) = self.theme_override { settings.theme = t; }
        if let Some(s) = self.style_override { settings.style = s; }
        register_layout_manager_chevron(
            layout, render, self.parent, self.id, self.rect, &view, &settings,
        );
    }
}

// =============================================================================
// Tooltip
// =============================================================================

use crate::ui::widgets::atomic::tooltip::input::register_layout_manager_tooltip;
use crate::ui::widgets::atomic::tooltip::settings::TooltipSettings;
use crate::ui::widgets::atomic::tooltip::style::TooltipStyle;
use crate::ui::widgets::atomic::tooltip::theme::TooltipTheme;
use crate::ui::widgets::atomic::tooltip::types::{TooltipConfig, TooltipPosition};

/// Chainable builder for an atomic tooltip overlay.
///
/// `text` is the primary single-line label.  `lines` enables the multi-line
/// crosshair variant.  `anchor` is the rect of the widget that triggered the
/// tooltip (screen coords); `position` picks placement relative to it.
pub struct TooltipBuilder<'a> {
    id:       WidgetId,
    rect:     Rect,
    parent:   LayoutNodeId,
    text:     String,
    lines:    Option<Vec<String>>,
    anchor:   Rect,
    position: TooltipPosition,
    alpha:    f64,
    settings: Option<TooltipSettings>,
    theme_override: Option<Box<dyn TooltipTheme>>,
    style_override: Option<Box<dyn TooltipStyle>>,
    /// Borrow-marker so the builder lifetime stays consistent with sibling
    /// atomics even though no `&'a` field is currently exposed.
    _phantom: std::marker::PhantomData<&'a ()>,
}

/// Entry point: build a tooltip at the given id + rect.
pub fn tooltip<'a>(id: impl Into<WidgetId>, rect: Rect) -> TooltipBuilder<'a> {
    TooltipBuilder {
        id: id.into(),
        rect,
        parent:   LayoutNodeId::ROOT,
        text:     String::new(),
        lines:    None,
        anchor:   Rect::new(0.0, 0.0, 0.0, 0.0),
        position: TooltipPosition::Above,
        alpha:    1.0,
        settings: None,
        theme_override: None,
        style_override: None,
        _phantom: std::marker::PhantomData,
    }
}

impl<'a> TooltipBuilder<'a> {
    pub fn parent(mut self, p: LayoutNodeId) -> Self { self.parent = p; self }

    /// Primary label.  For the multi-line crosshair variant set `text` to the
    /// first line and pass remaining lines through `.lines(...)`.
    pub fn text(mut self, t: impl Into<String>) -> Self { self.text = t.into(); self }

    /// Additional lines for the crosshair variant.
    pub fn lines(mut self, ls: Vec<String>) -> Self { self.lines = Some(ls); self }

    /// Rect of the widget that triggered the tooltip (screen coords) — used
    /// to position the bubble.
    pub fn anchor(mut self, r: Rect) -> Self { self.anchor = r; self }

    /// Preferred placement relative to the anchor.
    pub fn position(mut self, p: TooltipPosition) -> Self { self.position = p; self }

    /// Fade-in / fade-out alpha (0.0..=1.0).  Caller animates over time.
    pub fn alpha(mut self, a: f64) -> Self { self.alpha = a; self }

    pub fn settings(mut self, s: TooltipSettings) -> Self { self.settings = Some(s); self }

    /// Override only the tooltip theme (colour tokens).
    pub fn theme(mut self, t: Box<dyn TooltipTheme>) -> Self {
        self.theme_override = Some(t);
        self
    }

    /// Override only the tooltip style (geometry — padding, radius, font …).
    pub fn style(mut self, s: Box<dyn TooltipStyle>) -> Self {
        self.style_override = Some(s);
        self
    }

    pub fn build<P: DockPanel>(
        self,
        layout: &mut LayoutManager<P>,
        render: &mut dyn RenderContext,
    ) {
        let cfg = TooltipConfig {
            text:     self.text,
            lines:    self.lines,
            anchor:   self.anchor,
            position: self.position,
        };
        let mut settings = self.settings.unwrap_or_default();
        if let Some(t) = self.theme_override { settings.theme = t; }
        if let Some(s) = self.style_override { settings.style = s; }
        register_layout_manager_tooltip(
            layout, render, self.parent, self.id, self.rect, &cfg, self.alpha, &settings,
        );
    }
}
