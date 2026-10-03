//! Chainable builders, the `view!` surface, retargeted from
//! `&mut LayoutManager` to a [`ContentCx`] (a [`PanelCx`] or an
//! [`OverlayCx`]).
//!
//! `.build(cx)` registers and draws. There is no layout-tree parent and no
//! string handle: the id is a [`WidgetId`], and an overlay opens with an
//! [`crate::OverlayCmd`] rather than `layout.add_modal`. Blackbox is not a
//! builder (design §7.2).

use uzor::render::{TextAlign, TextBaseline};
use uzor::ui::widgets::atomic::button::render::HoverChevronSpec;
use uzor::ui::widgets::atomic::button::{ButtonSettings, ButtonView};
use uzor::ui::widgets::atomic::checkbox::{CheckboxRenderKind, CheckboxSettings, CheckboxView};
use uzor::ui::widgets::atomic::chevron::{
    ChevronDirection, ChevronSettings, ChevronUseCase, ChevronView, ChevronVisualKind,
    DefaultChevronStyle, HitAreaPolicy, PlacementPolicy, VisibilityPolicy,
};
use uzor::ui::widgets::atomic::separator::{
    DefaultSeparatorStyle, SeparatorKind, SeparatorOrientation, SeparatorSettings, SeparatorType,
    SeparatorView,
};
use uzor::ui::widgets::atomic::text::{TextOverflow, TextSettings, TextView};
use uzor::ui::widgets::atomic::toggle::{ToggleRenderKind, ToggleSettings, ToggleView};
use uzor::ui::widgets::atomic::tooltip::{
    DefaultTooltipStyle, TooltipConfig, TooltipPosition, TooltipSettings,
};
use uzor::ui::widgets::composite::chrome::{
    measure as measure_chrome, ChromeRenderKind, ChromeSettings, ChromeState, ChromeTabConfig,
    ChromeView, DefaultChromeStyle,
};
use uzor::Rect;
use uzor::WidgetId;

use super::atomics::{
    paint_checkbox, paint_chevron, paint_separator, paint_text, paint_toggle, paint_tooltip,
};
use super::button::{button_settings, paint_button};
use super::host::ContentCx;
use super::paint::theme_of;

// -- button -----------------------------------------------------------------

/// Chainable button. Terminal call is [`ButtonBuilder::build`].
pub struct ButtonBuilder<'a> {
    id: WidgetId,
    rect: Rect,
    text: Option<&'a str>,
    icon: Option<&'a uzor::IconId>,
    active: bool,
    disabled: bool,
    active_border: Option<bool>,
    hover_chevron: Option<HoverChevronSpec>,
    settings: Option<ButtonSettings>,
    on_click: Option<Box<dyn FnOnce() + 'a>>,
    bind_count: Option<&'a mut u32>,
}

/// Start a button at `id` + `rect`.
pub fn button<'a>(id: impl Into<WidgetId>, rect: Rect) -> ButtonBuilder<'a> {
    ButtonBuilder {
        id: id.into(),
        rect,
        text: None,
        icon: None,
        active: false,
        disabled: false,
        active_border: None,
        hover_chevron: None,
        settings: None,
        on_click: None,
        bind_count: None,
    }
}

impl<'a> ButtonBuilder<'a> {
    /// Label.
    pub fn text(mut self, t: &'a str) -> Self {
        self.text = Some(t);
        self
    }
    /// Icon drawn left of the label.
    pub fn icon(mut self, i: &'a uzor::IconId) -> Self {
        self.icon = Some(i);
        self
    }
    /// Drawn as active (accent fill) when not hovered or pressed.
    pub fn active(mut self, on: bool) -> Self {
        self.active = on;
        self
    }
    /// Disabled colours; clicks are ignored.
    pub fn disabled(mut self, on: bool) -> Self {
        self.disabled = on;
        self
    }
    /// Per-instance active-border override.
    pub fn active_border(mut self, b: Option<bool>) -> Self {
        self.active_border = b;
        self
    }
    /// Hover-revealed trailing chevron.
    pub fn hover_chevron(mut self, c: HoverChevronSpec) -> Self {
        self.hover_chevron = Some(c);
        self
    }
    /// Full settings. Wins over the token-derived default.
    pub fn settings(mut self, s: ButtonSettings) -> Self {
        self.settings = Some(s);
        self
    }
    /// Invoked when this id was clicked and the button is not disabled.
    pub fn on_click(mut self, f: impl FnOnce() + 'a) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
    /// Incremented on each click (ignored when disabled).
    pub fn bind_count(mut self, n: &'a mut u32) -> Self {
        self.bind_count = Some(n);
        self
    }

    /// Register and draw into `cx`.
    pub fn build<'c, C: ContentCx<'c>>(self, cx: &mut C) {
        let theme = theme_of(cx);
        cx.with_paint(|render, widgets, visual, tokens| {
            if !self.disabled && visual.clicked(&self.id) {
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
            let settings = self.settings.unwrap_or_else(|| {
                let mut s = button_settings(tokens);
                s.theme = Box::new(theme);
                s
            });
            paint_button(
                render, widgets, visual, self.id, self.rect, &view, &settings,
            );
        });
    }
}

// -- text -------------------------------------------------------------------

/// Chainable text label.
pub struct TextBuilder<'a> {
    id: WidgetId,
    rect: Rect,
    text: &'a str,
    color: Option<&'a str>,
    align: TextAlign,
    baseline: TextBaseline,
    font: Option<&'a str>,
    overflow: TextOverflow,
    hovered: bool,
    settings: Option<TextSettings>,
}

/// Start a text label.
pub fn text<'a>(id: impl Into<WidgetId>, rect: Rect, text: &'a str) -> TextBuilder<'a> {
    TextBuilder {
        id: id.into(),
        rect,
        text,
        color: None,
        align: TextAlign::Left,
        baseline: TextBaseline::Middle,
        font: None,
        overflow: TextOverflow::Clip,
        hovered: false,
        settings: None,
    }
}

impl<'a> TextBuilder<'a> {
    /// CSS colour override. `None` uses the theme.
    pub fn color(mut self, c: &'a str) -> Self {
        self.color = Some(c);
        self
    }
    /// Horizontal alignment.
    pub fn align(mut self, a: TextAlign) -> Self {
        self.align = a;
        self
    }
    /// Vertical alignment.
    pub fn baseline(mut self, b: TextBaseline) -> Self {
        self.baseline = b;
        self
    }
    /// Font shorthand override.
    pub fn font(mut self, f: &'a str) -> Self {
        self.font = Some(f);
        self
    }
    /// Overflow policy.
    pub fn overflow(mut self, o: TextOverflow) -> Self {
        self.overflow = o;
        self
    }
    /// Force the hover colour.
    pub fn hovered(mut self, on: bool) -> Self {
        self.hovered = on;
        self
    }
    /// Full settings.
    pub fn settings(mut self, s: TextSettings) -> Self {
        self.settings = Some(s);
        self
    }

    /// Register and draw into `cx`.
    pub fn build<'c, C: ContentCx<'c>>(self, cx: &mut C) {
        let explicit = self.settings.is_some();
        let theme = theme_of(cx);
        cx.with_paint(|render, widgets, _visual, _tokens| {
            let view = TextView {
                text: self.text,
                color: self.color,
                align: self.align,
                baseline: self.baseline,
                font: self.font,
                overflow: self.overflow,
                hovered: self.hovered,
            };
            let mut settings = self.settings.unwrap_or_default();
            if !explicit {
                settings.theme = Box::new(theme);
            }
            paint_text(render, widgets, self.id, self.rect, &view, &settings);
        });
    }
}

// -- checkbox ---------------------------------------------------------------

/// Chainable checkbox.
pub struct CheckboxBuilder<'a> {
    id: WidgetId,
    rect: Rect,
    checked: bool,
    bind: Option<&'a mut bool>,
    label: Option<&'a str>,
    kind: CheckboxRenderKind<'a>,
    font: &'a str,
    settings: Option<CheckboxSettings>,
}

/// Start a checkbox.
pub fn checkbox<'a>(id: impl Into<WidgetId>, rect: Rect) -> CheckboxBuilder<'a> {
    CheckboxBuilder {
        id: id.into(),
        rect,
        checked: false,
        bind: None,
        label: None,
        kind: CheckboxRenderKind::Standard,
        font: "13px sans-serif",
        settings: None,
    }
}

impl<'a> CheckboxBuilder<'a> {
    /// Checked, ignored when [`CheckboxBuilder::bind`] is set.
    pub fn checked(mut self, on: bool) -> Self {
        self.checked = on;
        self
    }
    /// Read `*flag` for paint and flip it when the box was clicked.
    pub fn bind(mut self, flag: &'a mut bool) -> Self {
        self.bind = Some(flag);
        self
    }
    /// Label drawn beside the box.
    pub fn label(mut self, l: &'a str) -> Self {
        self.label = Some(l);
        self
    }
    /// Render kind.
    pub fn kind(mut self, k: CheckboxRenderKind<'a>) -> Self {
        self.kind = k;
        self
    }
    /// Label font shorthand.
    pub fn font(mut self, f: &'a str) -> Self {
        self.font = f;
        self
    }
    /// Full settings.
    pub fn settings(mut self, s: CheckboxSettings) -> Self {
        self.settings = Some(s);
        self
    }

    /// Register and draw into `cx`.
    pub fn build<'c, C: ContentCx<'c>>(self, cx: &mut C) {
        let explicit = self.settings.is_some();
        let theme = theme_of(cx);
        cx.with_paint(|render, widgets, visual, _tokens| {
            let checked = if let Some(flag) = self.bind {
                if visual.clicked(&self.id) {
                    *flag = !*flag;
                }
                *flag
            } else {
                self.checked
            };
            let view = CheckboxView {
                checked,
                label: self.label,
            };
            let mut settings = self.settings.unwrap_or_default();
            if !explicit {
                settings.theme = Box::new(theme);
            }
            paint_checkbox(
                render, widgets, visual, self.id, self.rect, &view, &settings, &self.kind,
                self.font,
            );
        });
    }
}

// -- toggle -----------------------------------------------------------------

/// Chainable toggle.
pub struct ToggleBuilder<'a> {
    id: WidgetId,
    rect: Rect,
    toggled: bool,
    bind: Option<&'a mut bool>,
    label: Option<&'a str>,
    disabled: bool,
    kind: ToggleRenderKind<'a>,
    settings: Option<ToggleSettings>,
}

/// Start a toggle.
pub fn toggle<'a>(id: impl Into<WidgetId>, rect: Rect) -> ToggleBuilder<'a> {
    ToggleBuilder {
        id: id.into(),
        rect,
        toggled: false,
        bind: None,
        label: None,
        disabled: false,
        kind: ToggleRenderKind::Switch,
        settings: None,
    }
}

impl<'a> ToggleBuilder<'a> {
    /// On, ignored when [`ToggleBuilder::bind`] is set.
    pub fn toggled(mut self, on: bool) -> Self {
        self.toggled = on;
        self
    }
    /// Read `*flag` for paint and flip it when clicked and not disabled.
    pub fn bind(mut self, flag: &'a mut bool) -> Self {
        self.bind = Some(flag);
        self
    }
    /// Label.
    pub fn label(mut self, l: &'a str) -> Self {
        self.label = Some(l);
        self
    }
    /// Disabled colours; bind does not flip.
    pub fn disabled(mut self, on: bool) -> Self {
        self.disabled = on;
        self
    }
    /// Render kind.
    pub fn kind(mut self, k: ToggleRenderKind<'a>) -> Self {
        self.kind = k;
        self
    }
    /// Full settings.
    pub fn settings(mut self, s: ToggleSettings) -> Self {
        self.settings = Some(s);
        self
    }

    /// Register and draw into `cx`.
    pub fn build<'c, C: ContentCx<'c>>(self, cx: &mut C) {
        let explicit = self.settings.is_some();
        let theme = theme_of(cx);
        cx.with_paint(|render, widgets, visual, _tokens| {
            let toggled = if let Some(flag) = self.bind {
                if visual.clicked(&self.id) && !self.disabled {
                    *flag = !*flag;
                }
                *flag
            } else {
                self.toggled
            };
            let view = ToggleView {
                toggled,
                label: self.label,
                disabled: self.disabled,
            };
            let mut settings = self.settings.unwrap_or_default();
            if !explicit {
                settings.theme = Box::new(theme);
            }
            paint_toggle(
                render, widgets, visual, self.id, self.rect, &view, &settings, &self.kind,
            );
        });
    }
}

// -- separator --------------------------------------------------------------

/// Chainable separator.
pub struct SeparatorBuilder {
    id: WidgetId,
    rect: Rect,
    kind: SeparatorKind,
    sep_type: SeparatorType,
    hovered: bool,
    dragging: bool,
    settings: Option<SeparatorSettings>,
}

/// Start a horizontal divider.
pub fn separator(id: impl Into<WidgetId>, rect: Rect) -> SeparatorBuilder {
    SeparatorBuilder {
        id: id.into(),
        rect,
        kind: SeparatorKind::Divider,
        sep_type: SeparatorType::Divider {
            orientation: SeparatorOrientation::Horizontal,
        },
        hovered: false,
        dragging: false,
        settings: None,
    }
}

impl SeparatorBuilder {
    /// Hit sense.
    pub fn kind(mut self, k: SeparatorKind) -> Self {
        self.kind = k;
        self
    }
    /// Paint kind.
    pub fn sep_type(mut self, t: SeparatorType) -> Self {
        self.sep_type = t;
        self
    }
    /// Hover colour.
    pub fn hovered(mut self, on: bool) -> Self {
        self.hovered = on;
        self
    }
    /// Drag colour.
    pub fn dragging(mut self, on: bool) -> Self {
        self.dragging = on;
        self
    }
    /// Full settings.
    pub fn settings(mut self, s: SeparatorSettings) -> Self {
        self.settings = Some(s);
        self
    }

    /// Register and draw into `cx`.
    pub fn build<'c, C: ContentCx<'c>>(self, cx: &mut C) {
        let theme = theme_of(cx);
        cx.with_paint(|render, widgets, _visual, _tokens| {
            let view = SeparatorView {
                kind: self.sep_type,
                hovered: self.hovered,
                dragging: self.dragging,
            };
            let settings = match self.settings {
                Some(settings) => settings,
                None => SeparatorSettings {
                    theme: Box::new(theme),
                    style: Box::new(DefaultSeparatorStyle),
                },
            };
            paint_separator(
                render, widgets, self.id, self.rect, self.kind, &view, &settings,
            );
        });
    }
}

// -- chevron ----------------------------------------------------------------

/// Chainable chevron.
pub struct ChevronBuilder<'a> {
    id: WidgetId,
    rect: Rect,
    view: ChevronView,
    settings: Option<ChevronSettings>,
    on_click: Option<Box<dyn FnOnce() + 'a>>,
}

/// Start a down-pointing chevron.
pub fn chevron<'a>(id: impl Into<WidgetId>, rect: Rect) -> ChevronBuilder<'a> {
    ChevronBuilder {
        id: id.into(),
        rect,
        view: ChevronView {
            direction: ChevronDirection::Down,
            use_case: ChevronUseCase::PureButton,
            visibility: VisibilityPolicy::Always,
            placement: PlacementPolicy::Standalone,
            hit_area: HitAreaPolicy::Visual,
            visual_kind: ChevronVisualKind::Stroked,
            hovered: false,
            pressed: false,
            disabled: false,
            active: false,
            glyph_override: None,
        },
        settings: None,
        on_click: None,
    }
}

impl<'a> ChevronBuilder<'a> {
    /// Direction the glyph points.
    pub fn direction(mut self, d: ChevronDirection) -> Self {
        self.view.direction = d;
        self
    }
    /// Hover flag (paint).
    pub fn hovered(mut self, on: bool) -> Self {
        self.view.hovered = on;
        self
    }
    /// Pressed flag (paint).
    pub fn pressed(mut self, on: bool) -> Self {
        self.view.pressed = on;
        self
    }
    /// Disabled flag (paint, and clicks are ignored).
    pub fn disabled(mut self, on: bool) -> Self {
        self.view.disabled = on;
        self
    }
    /// Invoked when this id was clicked and the chevron is not disabled.
    pub fn on_click(mut self, f: impl FnOnce() + 'a) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
    /// Full settings.
    pub fn settings(mut self, s: ChevronSettings) -> Self {
        self.settings = Some(s);
        self
    }

    /// Register and draw into `cx`.
    pub fn build<'c, C: ContentCx<'c>>(self, cx: &mut C) {
        let theme = theme_of(cx);
        cx.with_paint(|render, widgets, visual, _tokens| {
            if !self.view.disabled && visual.clicked(&self.id) {
                if let Some(cb) = self.on_click {
                    cb();
                }
            }
            let settings = match self.settings {
                Some(settings) => settings,
                None => ChevronSettings {
                    theme: Box::new(theme),
                    style: Box::new(DefaultChevronStyle),
                },
            };
            paint_chevron(render, widgets, self.id, self.rect, &self.view, &settings);
        });
    }
}

// -- tooltip ----------------------------------------------------------------

/// Chainable tooltip.
pub struct TooltipBuilder {
    id: WidgetId,
    rect: Rect,
    text: String,
    anchor: Rect,
    position: TooltipPosition,
    alpha: f64,
    settings: Option<TooltipSettings>,
}

/// Start a tooltip.
pub fn tooltip(id: impl Into<WidgetId>, rect: Rect) -> TooltipBuilder {
    TooltipBuilder {
        id: id.into(),
        rect,
        text: String::new(),
        anchor: Rect::new(0.0, 0.0, 0.0, 0.0),
        position: TooltipPosition::Above,
        alpha: 1.0,
        settings: None,
    }
}

impl TooltipBuilder {
    /// Primary label.
    pub fn text(mut self, t: impl Into<String>) -> Self {
        self.text = t.into();
        self
    }
    /// Anchor rect the bubble sits against.
    pub fn anchor(mut self, r: Rect) -> Self {
        self.anchor = r;
        self
    }
    /// Preferred side of the anchor.
    pub fn position(mut self, p: TooltipPosition) -> Self {
        self.position = p;
        self
    }
    /// Opacity, 0.0..=1.0.
    pub fn alpha(mut self, a: f64) -> Self {
        self.alpha = a;
        self
    }
    /// Full settings.
    pub fn settings(mut self, s: TooltipSettings) -> Self {
        self.settings = Some(s);
        self
    }

    /// Register and draw into `cx`.
    pub fn build<'c, C: ContentCx<'c>>(self, cx: &mut C) {
        let theme = theme_of(cx);
        cx.with_paint(|render, widgets, _visual, _tokens| {
            let config = TooltipConfig {
                text: self.text,
                lines: None,
                anchor: self.anchor,
                position: self.position,
            };
            let settings = match self.settings {
                Some(settings) => settings,
                None => TooltipSettings {
                    theme: Box::new(theme),
                    style: Box::new(DefaultTooltipStyle),
                },
            };
            paint_tooltip(
                render, widgets, self.id, self.rect, &config, self.alpha, &settings,
            );
        });
    }
}

// -- chrome -----------------------------------------------------------------

/// Chainable window chrome strip.
pub struct ChromeBuilder<'a> {
    id: &'a str,
    rect: Option<Rect>,
    tabs: &'a [ChromeTabConfig<'a>],
    active_tab_id: Option<&'a str>,
    show_new_tab_btn: bool,
    show_menu_btn: bool,
    show_new_window_btn: bool,
    show_close_window_btn: bool,
    is_maximized: bool,
    menu_left: bool,
    show_maximize: bool,
    cursor_x: f64,
    cursor_y: f64,
    time_ms: f64,
    kind: ChromeRenderKind,
    settings: Option<ChromeSettings>,
}

/// Start a chrome strip with widget id `"chrome"`.
pub fn chrome<'a>() -> ChromeBuilder<'a> {
    ChromeBuilder {
        id: "chrome",
        rect: None,
        tabs: &[],
        active_tab_id: None,
        show_new_tab_btn: false,
        show_menu_btn: false,
        show_new_window_btn: false,
        show_close_window_btn: false,
        is_maximized: false,
        menu_left: false,
        show_maximize: true,
        cursor_x: 0.0,
        cursor_y: 0.0,
        time_ms: 0.0,
        kind: ChromeRenderKind::Default,
        settings: None,
    }
}

impl<'a> ChromeBuilder<'a> {
    /// Widget id (default `"chrome"`).
    pub fn widget_id(mut self, id: &'a str) -> Self {
        self.id = id;
        self
    }
    /// Strip rect. Default is the measured natural size at the origin.
    pub fn rect(mut self, r: Rect) -> Self {
        self.rect = Some(r);
        self
    }
    /// Tabs, left to right.
    pub fn tabs(mut self, ts: &'a [ChromeTabConfig<'a>]) -> Self {
        self.tabs = ts;
        self
    }
    /// Id of the active tab.
    pub fn active_tab(mut self, id: &'a str) -> Self {
        self.active_tab_id = Some(id);
        self
    }
    /// New-tab button.
    pub fn show_new_tab_btn(mut self, on: bool) -> Self {
        self.show_new_tab_btn = on;
        self
    }
    /// Menu button.
    pub fn show_menu_btn(mut self, on: bool) -> Self {
        self.show_menu_btn = on;
        self
    }
    /// New-window button.
    pub fn show_new_window_btn(mut self, on: bool) -> Self {
        self.show_new_window_btn = on;
        self
    }
    /// Cursor, window-local logical pixels.
    pub fn cursor(mut self, pos: (f64, f64)) -> Self {
        self.cursor_x = pos.0;
        self.cursor_y = pos.1;
        self
    }
    /// Frame clock in milliseconds, for the chrome clock slot.
    pub fn time_ms(mut self, t: f64) -> Self {
        self.time_ms = t;
        self
    }
    /// Render kind. A `Custom` kind is drawn by its closure; the measured
    /// default rect is then `0×0` unless [`ChromeBuilder::rect`] is set.
    pub fn kind(mut self, k: ChromeRenderKind) -> Self {
        self.kind = k;
        self
    }

    /// Register and draw into `cx`.
    pub fn build<'c, C: ContentCx<'c>>(self, cx: &mut C) {
        let theme = theme_of(cx);
        cx.with_paint(|render, widgets, _visual, _tokens| {
            let view = ChromeView {
                tabs: self.tabs,
                active_tab_id: self.active_tab_id,
                show_new_tab_btn: self.show_new_tab_btn,
                show_menu_btn: self.show_menu_btn,
                show_new_window_btn: self.show_new_window_btn,
                show_close_window_btn: self.show_close_window_btn,
                is_maximized: self.is_maximized,
                menu_left: self.menu_left,
                show_maximize: self.show_maximize,
                cursor_x: self.cursor_x,
                cursor_y: self.cursor_y,
                time_ms: self.time_ms,
            };
            let settings = self.settings.unwrap_or_else(|| ChromeSettings {
                theme: Box::new(theme),
                style: Box::<DefaultChromeStyle>::default(),
            });
            let mut state = ChromeState::new();
            let ids: Vec<&str> = self.tabs.iter().map(|t| t.id).collect();
            state.sync_tabs(&ids);
            let rect = self.rect.unwrap_or_else(|| {
                let (w, h) = measure_chrome(&view, &state, &settings, &self.kind);
                Rect::new(0.0, 0.0, w, h)
            });
            super::composites::chrome_paint(
                render,
                widgets,
                WidgetId::from(self.id),
                rect,
                &state,
                &view,
                &settings,
                &self.kind,
            );
        });
    }
}
