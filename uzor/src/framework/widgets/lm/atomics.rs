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

use crate::core::types::Rect;
use crate::layout::docking::DockPanel;
use crate::layout::{LayoutManager, LayoutNodeId, StyleManager};
use crate::render::RenderContext;
use crate::types::{WidgetId, WidgetState};

// =============================================================================
// StyledButtonTheme — reads from StyleManager, delegates rest to Default
// =============================================================================

use crate::ui::widgets::atomic::button::theme::{ButtonTheme, DefaultButtonTheme};
use crate::ui::widgets::atomic::button::style::{ButtonStyle, DefaultButtonStyle};
use crate::ui::widgets::atomic::button::settings::ButtonSettings;

struct StyledButtonTheme {
    bg_normal:               String,
    bg_hover:                String,
    bg_active:               String,
    bg_pressed:              String,
    bg_disabled:             String,
    text_normal:             String,
    text_hover:              String,
    text_active:             String,
    text_disabled:           String,
    icon_normal:             String,
    icon_hover:              String,
    icon_active:             String,
    icon_disabled:           String,
    border_normal:           String,
    border_hover:            String,
    border_focused:          String,
    accent:                  String,
    danger:                  String,
    success:                 String,
    warning:                 String,
    toolbar_item_bg_hover:   String,
    toolbar_item_bg_active:  String,
    toolbar_item_text:       String,
    toolbar_item_text_hover: String,
    fallback:                DefaultButtonTheme,
}

impl StyledButtonTheme {
    fn from_styles(s: &StyleManager) -> Self {
        let accent     = s.color_or_owned("accent",         "#2962ff");
        let accent_dim = s.color_or_owned("accent_dim",     "rgba(41,98,255,0.15)");
        let surface    = s.color_or_owned("surface",        "transparent");
        let surface_r  = s.color_or_owned("surface_raised", "#2a2e39");
        let fg_0       = s.color_or_owned("fg_0",           "#ffffff");
        let fg_1       = s.color_or_owned("fg_1",           "#d1d5db");
        let fg_2       = s.color_or_owned("fg_2",           "#878B91");
        let fg_3       = s.color_or_owned("fg_3",           "#555860");
        let border_c   = s.color_or_owned("border",         "rgba(255,255,255,0.06)");
        let error_c    = s.color_or_owned("error",          "#ef5350");
        let ok_c       = s.color_or_owned("ok",             "#10b981");
        let warn_c     = s.color_or_owned("warn",           "#f59e0b");
        Self {
            bg_normal:               "transparent".into(),     // button text-on-surface by default
            bg_hover:                accent_dim.clone(),
            bg_active:               accent.clone(),
            bg_pressed:              accent.clone(),
            bg_disabled:             surface.clone(),
            text_normal:             fg_1.clone(),
            text_hover:              fg_0.clone(),
            text_active:             fg_0.clone(),
            text_disabled:           fg_3.clone(),
            icon_normal:             fg_2.clone(),
            icon_hover:              fg_0.clone(),
            icon_active:             fg_0.clone(),
            icon_disabled:           fg_3.clone(),
            border_normal:           border_c.clone(),
            border_hover:            border_c.clone(),
            border_focused:          accent.clone(),
            accent:                  accent.clone(),
            danger:                  error_c,
            success:                 ok_c,
            warning:                 warn_c,
            toolbar_item_bg_hover:   surface_r.clone(),
            toolbar_item_bg_active:  accent.clone(),
            toolbar_item_text:       fg_1,
            toolbar_item_text_hover: fg_0,
            fallback:                DefaultButtonTheme,
        }
    }
}

impl ButtonTheme for StyledButtonTheme {
    fn button_bg_normal(&self)   -> &str { &self.bg_normal }
    fn button_bg_hover(&self)    -> &str { &self.bg_hover }
    fn button_bg_pressed(&self)  -> &str { &self.bg_pressed }
    fn button_bg_active(&self)   -> &str { &self.bg_active }
    fn button_bg_disabled(&self) -> &str { &self.bg_disabled }

    fn button_text_normal(&self)   -> &str { &self.text_normal }
    fn button_text_hover(&self)    -> &str { &self.text_hover }
    fn button_text_active(&self)   -> &str { &self.text_active }
    fn button_text_disabled(&self) -> &str { &self.text_disabled }

    fn button_icon_normal(&self)   -> &str { &self.icon_normal }
    fn button_icon_hover(&self)    -> &str { &self.icon_hover }
    fn button_icon_active(&self)   -> &str { &self.icon_active }
    fn button_icon_disabled(&self) -> &str { &self.icon_disabled }

    fn button_border_normal(&self)  -> &str { &self.border_normal }
    fn button_border_hover(&self)   -> &str { &self.border_hover }
    fn button_border_focused(&self) -> &str { &self.border_focused }

    fn button_accent(&self)   -> &str { &self.accent }
    fn button_danger(&self)   -> &str { &self.danger }
    fn button_success(&self)  -> &str { &self.success }
    fn button_warning(&self)  -> &str { &self.warning }

    fn toolbar_item_bg_hover(&self)    -> &str { &self.toolbar_item_bg_hover }
    fn toolbar_item_bg_active(&self)   -> &str { &self.toolbar_item_bg_active }
    fn toolbar_item_text(&self)        -> &str { &self.toolbar_item_text }
    fn toolbar_item_text_hover(&self)  -> &str { &self.toolbar_item_text_hover }
    fn toolbar_item_text_active(&self) -> &str { self.fallback.toolbar_item_text_active() }
    fn toolbar_separator(&self)        -> &str { self.fallback.toolbar_separator() }
    fn toolbar_background(&self)       -> &str { self.fallback.toolbar_background() }
    fn toolbar_accent(&self)           -> &str { &self.accent }

    fn button_primary_bg(&self)           -> &str { &self.accent }
    fn button_primary_bg_hover(&self)     -> &str { self.fallback.button_primary_bg_hover() }
    fn button_danger_bg(&self)            -> &str { self.fallback.button_danger_bg() }
    fn button_danger_bg_hover(&self)      -> &str { self.fallback.button_danger_bg_hover() }
    fn button_danger_border(&self)        -> &str { self.fallback.button_danger_border() }
    fn button_danger_border_hover(&self)  -> &str { self.fallback.button_danger_border_hover() }
    fn button_danger_text(&self)          -> &str { &self.danger }
    fn button_secondary_hover_bg(&self)   -> &str { self.fallback.button_secondary_hover_bg() }
    fn button_secondary_text_muted(&self) -> &str { self.fallback.button_secondary_text_muted() }
    fn button_secondary_text(&self)       -> &str { self.fallback.button_secondary_text() }
    fn button_ghost_idle_bg(&self)        -> &str { self.fallback.button_ghost_idle_bg() }
    fn button_utility_bg(&self)           -> &str { self.fallback.button_utility_bg() }
    fn button_utility_bg_hover(&self)     -> &str { self.fallback.button_utility_bg_hover() }
}

struct StyledButtonStyle {
    radius:    f64,
    padding_x: f64,
    font_size: f64,
    fallback:  DefaultButtonStyle,
}

impl StyledButtonStyle {
    fn from_styles(s: &StyleManager) -> Self {
        Self {
            radius:    s.size_or("button_radius",    4.0),
            padding_x: s.size_or("button_padding",   8.0),
            font_size: s.size_or("button_font_size", 13.0),
            fallback:  DefaultButtonStyle,
        }
    }
}

impl ButtonStyle for StyledButtonStyle {
    fn radius(&self)             -> f64  { self.radius }
    fn padding_x(&self)          -> f64  { self.padding_x }
    fn padding_y(&self)          -> f64  { self.fallback.padding_y() }
    fn icon_size(&self)          -> f64  { self.fallback.icon_size() }
    fn font_size(&self)          -> f64  { self.font_size }
    fn gap(&self)                -> f64  { self.fallback.gap() }
    fn border_width(&self)       -> f64  { self.fallback.border_width() }
    fn show_active_border(&self) -> bool { self.fallback.show_active_border() }
}

fn button_settings_from_styles(s: &StyleManager) -> ButtonSettings {
    ButtonSettings {
        theme: Box::new(StyledButtonTheme::from_styles(s)),
        style: Box::new(StyledButtonStyle::from_styles(s)),
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
        let mut settings = self.settings.unwrap_or_else(|| button_settings_from_styles(layout.styles()));
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
use crate::ui::widgets::atomic::text::theme::{DefaultTextTheme, TextTheme};
use crate::ui::widgets::atomic::text::types::{TextOverflow, TextView};
use crate::render::{TextAlign, TextBaseline};

struct StyledTextTheme {
    color:       String,
    color_hover: String,
}

impl StyledTextTheme {
    fn from_styles(s: &StyleManager) -> Self {
        Self {
            color:       s.color_or_owned("fg_1", "#d1d4dc"),
            color_hover: s.color_or_owned("fg_0", "#ffffff"),
        }
    }
}

impl TextTheme for StyledTextTheme {
    fn text_color(&self)       -> &str { &self.color }
    fn text_color_hover(&self) -> &str { &self.color_hover }
}

fn text_settings_from_styles(s: &StyleManager) -> TextSettings {
    TextSettings {
        theme: Box::new(StyledTextTheme::from_styles(s)),
        style: Box::new(DefaultTextStyle),
    }
}

#[allow(dead_code)]
fn _suppress_default_text_theme_unused(_t: &DefaultTextTheme) {}

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
        let mut settings = self.settings.unwrap_or_else(|| text_settings_from_styles(layout.styles()));
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
