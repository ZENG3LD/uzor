//! Chrome **slot-driven** layout — alternative to `draw_chrome`.
//!
//! `draw_chrome` (in `render.rs`) hard-codes the order of zones
//! (left = tabs, right = window controls, etc.).  Real apps need
//! arbitrary slot configuration: tabs/menu/search/undo-redo/toolbar
//! placed in any of the three zones.  This module provides exactly
//! that — a pure paint surface driven by [`ChromeLayout`].
//!
//! All slots dispatch into existing pure paint fns
//! (`l0::atomic::*`, `l0::toolbar::draw_toolbar`, helpers below) —
//! no new state machines, no event handlers.  Embedders (tessera,
//! …) own interaction state keyed by the [`ChromeHitPath`] this
//! module returns from [`chrome_layout_hit_test`].

use crate::core::types::Rect;
use crate::core::types::state::WidgetState;
use crate::render::{RenderContext, TextAlign, TextBaseline};

use super::settings::ChromeSettings;
use super::state::ChromeState;
use super::types::ChromeTabConfig;
use crate::ui::widgets::atomic::text_input::{
    render::draw_input,
    settings::TextInputSettings,
    render::InputView,
    types::InputType,
    state::TextFieldConfig,
};

// ---------------------------------------------------------------------------
// ChromeLayout
// ---------------------------------------------------------------------------

/// Slot-driven chrome layout descriptor.  Each zone is an ordered
/// list of [`Slot`]s.  `left` flows left→right starting at `rect.x`,
/// `right` flows right→left starting at `rect.x + rect.w`, `center`
/// fills whatever gap remains.
///
/// The configurator is **declarative** — caller fills it once per
/// frame.  Position is decided by which `Vec` a slot lives in.
#[derive(Default)]
pub struct ChromeLayout<'a> {
    pub left:   Vec<Slot<'a>>,
    pub center: Vec<Slot<'a>>,
    pub right:  Vec<Slot<'a>>,
}

impl<'a> ChromeLayout<'a> {
    pub fn new() -> Self { Self::default() }

    pub fn left  (mut self, slots: Vec<Slot<'a>>) -> Self { self.left   = slots; self }
    pub fn center(mut self, slots: Vec<Slot<'a>>) -> Self { self.center = slots; self }
    pub fn right (mut self, slots: Vec<Slot<'a>>) -> Self { self.right  = slots; self }
}

/// One slot in a chrome zone.  Each variant owns enough data for
/// pure paint + hit-test; persistent interaction state (hover /
/// pressed / focused / text store) lives in the embedder.
pub enum Slot<'a> {
    /// Tab strip + optional `+` new-tab button.  Tabs themselves
    /// are app state — caller passes them through here.
    Tabs(TabsConfig<'a>),

    /// Burger / gear icon — opens an app-defined menu.  Standalone.
    Menu,

    /// Min / max / close-app trio.  Always present when chrome
    /// exists — owned by tessera's window domain.
    WindowControls,

    /// Multi-window pair: spawn-new-window + close-this-window.
    /// Always travels as a pair; turn on/off as one unit.
    MultiWindow,

    /// Inline search field.  Owned `TextFieldStore` lives in the
    /// embedder's atomics map; this carries only render-time view.
    Search(SearchConfig<'a>),

    /// Undo / redo bool pair.  Always presented together.
    UndoRedo,

    /// Update beacon — small badge that lights up when an OTA is
    /// available.  Standalone.
    UpdateBeacon,

    /// App-defined toolbar — items list (buttons, dropdowns,
    /// separators, …).  Forwarded to `l0::toolbar::draw_toolbar`.
    Toolbar(ToolbarSlotConfig<'a>),
}

/// Tab-strip data — what the slot needs each frame.
pub struct TabsConfig<'a> {
    pub tabs:          &'a [ChromeTabConfig<'a>],
    pub active_tab_id: Option<&'a str>,
    pub show_new_btn:  bool,
}

/// Search slot data — visible text + cursor + interaction flags.
/// Caller owns the `TextFieldStore` and feeds the relevant fields.
pub struct SearchConfig<'a> {
    pub text:        &'a str,
    pub placeholder: &'a str,
    pub cursor:      usize,
    pub selection:   Option<(usize, usize)>,
    pub focused:     bool,
    pub disabled:    bool,
    /// Visual width in pixels.  When `None` the slot expands to
    /// fill the centre zone.
    pub width:       Option<f64>,
}

/// Toolbar slot data — the embedder builds a `ToolbarView` and a
/// `ToolbarState` and hands them in.  `ToolbarRenderKind::Inline`
/// is the default rendering kind for chrome-embedded toolbars.
pub struct ToolbarSlotConfig<'a> {
    pub view:     &'a super::super::toolbar::types::ToolbarView<'a>,
    pub state:    &'a super::super::toolbar::state::ToolbarState,
    pub settings: &'a super::super::toolbar::settings::ToolbarSettings,
    /// Pixel width.  `None` = natural width via `measure_horizontal`.
    pub width:    Option<f64>,
}

// ---------------------------------------------------------------------------
// ChromeHitPath
// ---------------------------------------------------------------------------

/// Result of [`chrome_layout_hit_test`].  The embedder converts
/// this into its own `AtomicPath` (container + "left[i]" + atomic).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChromeHitPath {
    pub zone:        ChromeZone,
    pub slot_index:  usize,
    /// The atomic id within the slot (e.g. `"tab:0"`, `"min_btn"`,
    /// `"undo_btn"`, `"toolbar.btn:save"`, `""` for whole-slot).
    pub atomic_id:   String,
    pub kind:        ChromeHitKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChromeZone {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChromeHitKind {
    Background,
    Tab,
    TabClose,
    NewTab,
    Menu,
    MinBtn,
    MaxBtn,
    CloseAppBtn,
    NewWindowBtn,
    CloseWindowBtn,
    Search,
    UndoBtn,
    RedoBtn,
    BeaconBtn,
    ToolbarItem,
    Drag,
}

// ---------------------------------------------------------------------------
// Layout walker
// ---------------------------------------------------------------------------

/// Per-slot allocated rectangle.  Built by the walker; used by
/// both render and hit-test paths.
struct SlotPlacement {
    zone:       ChromeZone,
    slot_index: usize,
    rect:       Rect,
}

/// Walk left + right zones, then centre.  Returns `(placements,
/// background_rect)`.  Centre slots share the remaining space
/// equally; in practice chrome carries one centre slot at most.
fn walk(rect: Rect, layout: &ChromeLayout<'_>, settings: &ChromeSettings) -> Vec<SlotPlacement> {
    let mut out = Vec::with_capacity(layout.left.len() + layout.center.len() + layout.right.len());

    let mut left_cursor  = rect.x;
    let mut right_cursor = rect.x + rect.width;

    for (i, slot) in layout.left.iter().enumerate() {
        let w = slot_width(slot, rect.height, settings);
        let r = Rect { x: left_cursor, y: rect.y, width: w, height: rect.height };
        out.push(SlotPlacement { zone: ChromeZone::Left, slot_index: i, rect: r });
        left_cursor += w;
    }

    for (i, slot) in layout.right.iter().enumerate() {
        let w = slot_width(slot, rect.height, settings);
        right_cursor -= w;
        let r = Rect { x: right_cursor, y: rect.y, width: w, height: rect.height };
        out.push(SlotPlacement { zone: ChromeZone::Right, slot_index: i, rect: r });
    }

    let centre_x      = left_cursor;
    let centre_width  = (right_cursor - left_cursor).max(0.0);
    let centre_count  = layout.center.len().max(1);
    let centre_each   = centre_width / centre_count as f64;

    for (i, _slot) in layout.center.iter().enumerate() {
        let r = Rect {
            x:      centre_x + centre_each * i as f64,
            y:      rect.y,
            width:  centre_each,
            height: rect.height,
        };
        out.push(SlotPlacement { zone: ChromeZone::Center, slot_index: i, rect: r });
    }

    out
}

/// Reserved width for a slot.  Tabs / centre-search return 0 —
/// they take whatever zone gives them.  Bool pairs and standalone
/// icons return their fixed pixel column.
fn slot_width(slot: &Slot<'_>, height: f64, _settings: &ChromeSettings) -> f64 {
    match slot {
        Slot::Tabs(cfg)        => tabs_width(cfg, height),
        Slot::Menu             => 36.0,
        Slot::WindowControls   => 46.0 * 3.0,                 // min + max + close
        Slot::MultiWindow      => 36.0 * 2.0,                 // new-window + close-this
        Slot::Search(cfg)      => cfg.width.unwrap_or(0.0),   // 0 = "fill the zone"
        Slot::UndoRedo         => 36.0 * 2.0,
        Slot::UpdateBeacon     => 36.0,
        Slot::Toolbar(cfg)     => cfg.width.unwrap_or_else(|| {
            super::super::toolbar::render::measure_horizontal(cfg.view, cfg.settings).0
        }),
    }
}

fn tabs_width(cfg: &TabsConfig<'_>, _height: f64) -> f64 {
    const TAB_W:   f64 = 180.0;
    const NEW_W:   f64 = 28.0;
    let n = cfg.tabs.len() as f64;
    n * TAB_W + if cfg.show_new_btn { NEW_W } else { 0.0 }
}

// ---------------------------------------------------------------------------
// draw_chrome_layout
// ---------------------------------------------------------------------------

/// Paint a slot-driven chrome.  Each slot dispatches to its own
/// pure paint helper.  Caller decides theme / style by populating
/// `settings`.
pub fn draw_chrome_layout(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    state:    &ChromeState,
    settings: &ChromeSettings,
    layout:   &ChromeLayout<'_>,
) {
    if rect.width <= 0.0 || rect.height <= 0.0 { return; }

    // Background fill.
    let theme = settings.theme.as_ref();
    ctx.set_fill_color(theme.background());
    ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);

    // Bottom separator.
    if settings.style.show_bottom_border() {
        ctx.set_fill_color(theme.separator());
        ctx.fill_rect(rect.x, rect.y + rect.height - 1.0, rect.width, 1.0);
    }

    // Walk + dispatch.
    let placements = walk(rect, layout, settings);
    for p in &placements {
        let slot = match p.zone {
            ChromeZone::Left   => &layout.left[p.slot_index],
            ChromeZone::Center => &layout.center[p.slot_index],
            ChromeZone::Right  => &layout.right[p.slot_index],
        };
        draw_slot(ctx, p.rect, slot, state, settings);
    }
}

fn draw_slot(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    slot:     &Slot<'_>,
    state:    &ChromeState,
    settings: &ChromeSettings,
) {
    match slot {
        Slot::Tabs(cfg)           => draw_tabs(ctx, rect, cfg, state, settings),
        Slot::Menu                => draw_icon_btn(ctx, rect, "≡", false, settings),
        Slot::WindowControls      => draw_window_controls(ctx, rect, state, settings),
        Slot::MultiWindow         => draw_multi_window(ctx, rect, state, settings),
        Slot::Search(cfg)         => draw_search(ctx, rect, cfg, settings),
        Slot::UndoRedo            => draw_undo_redo(ctx, rect, state, settings),
        Slot::UpdateBeacon        => draw_icon_btn(ctx, rect, "●", false, settings),
        Slot::Toolbar(cfg)        => {
            super::super::toolbar::render::draw_toolbar(
                ctx, rect, cfg.state, cfg.view, cfg.settings,
                &super::super::toolbar::types::ToolbarRenderKind::Inline,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Per-slot paint
// ---------------------------------------------------------------------------

fn draw_tabs(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    cfg:      &TabsConfig<'_>,
    state:    &ChromeState,
    settings: &ChromeSettings,
) {
    let theme = settings.theme.as_ref();
    let style = settings.style.as_ref();
    let tab_w = if cfg.tabs.is_empty() { 0.0 } else {
        let avail = rect.width - if cfg.show_new_btn { 28.0 } else { 0.0 };
        (avail / cfg.tabs.len() as f64).min(180.0).max(80.0)
    };

    let mut x = rect.x;
    for (i, tab) in cfg.tabs.iter().enumerate() {
        let active   = cfg.active_tab_id == Some(tab.id);
        let hovered  = state.tabs_state.get(i).map(|t| t.hovered).unwrap_or(false);
        let bg = if active   { theme.tab_bg_active() }
                 else if hovered { theme.tab_bg_hover() }
                 else        { theme.tab_bg_normal() };
        let tx = if active   { theme.tab_text_active() }
                 else if hovered { theme.tab_text_hover() }
                 else        { theme.tab_text_normal() };

        ctx.set_fill_color(bg);
        ctx.fill_rect(x, rect.y, tab_w, rect.height);

        // Label.
        ctx.set_fill_color(tx);
        ctx.set_font("12px sans-serif");
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Middle);
        ctx.fill_text(
            tab.label,
            x + style.tab_padding_h(),
            rect.y + rect.height / 2.0,
        );

        // Active accent line.
        if active {
            ctx.set_fill_color(theme.tab_accent());
            ctx.fill_rect(
                x,
                rect.y + rect.height - style.tab_accent_height(),
                tab_w,
                style.tab_accent_height(),
            );
        }

        // Close × on closable tabs.
        if tab.closable {
            let cs = style.tab_close_size();
            let cx = x + tab_w - cs - 4.0;
            let cy = rect.y + (rect.height - cs) / 2.0;
            ctx.set_fill_color(tx);
            ctx.set_font("14px sans-serif");
            ctx.set_text_align(TextAlign::Center);
            ctx.set_text_baseline(TextBaseline::Middle);
            ctx.fill_text("×", cx + cs / 2.0, cy + cs / 2.0);
        }

        x += tab_w;
    }

    if cfg.show_new_btn {
        ctx.set_fill_color(theme.icon_normal());
        ctx.set_font("16px sans-serif");
        ctx.set_text_align(TextAlign::Center);
        ctx.set_text_baseline(TextBaseline::Middle);
        ctx.fill_text("+", x + 14.0, rect.y + rect.height / 2.0);
    }
}

fn draw_icon_btn(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    glyph:    &str,
    pressed:  bool,
    settings: &ChromeSettings,
) {
    let theme = settings.theme.as_ref();
    let bg = if pressed { theme.button_hover() } else { theme.background() };
    ctx.set_fill_color(bg);
    ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);

    ctx.set_fill_color(theme.icon_normal());
    ctx.set_font("14px sans-serif");
    ctx.set_text_align(TextAlign::Center);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(glyph, rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
}

fn draw_window_controls(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    state:    &ChromeState,
    settings: &ChromeSettings,
) {
    let theme = settings.theme.as_ref();
    let bw    = rect.width / 3.0;

    let min_r   = Rect { x: rect.x,            y: rect.y, width: bw, height: rect.height };
    let max_r   = Rect { x: rect.x + bw,       y: rect.y, width: bw, height: rect.height };
    let close_r = Rect { x: rect.x + bw * 2.0, y: rect.y, width: bw, height: rect.height };

    use super::types::ChromeHit;
    let min_hover   = matches!(state.hovered, ChromeHit::MinBtn);
    let max_hover   = matches!(state.hovered, ChromeHit::MaxBtn);
    let close_hover = matches!(state.hovered, ChromeHit::CloseBtn);

    if min_hover   {
        ctx.set_fill_color(theme.button_hover());
        ctx.fill_rect(min_r.x, min_r.y, min_r.width, min_r.height);
    }
    if max_hover   {
        ctx.set_fill_color(theme.button_hover());
        ctx.fill_rect(max_r.x, max_r.y, max_r.width, max_r.height);
    }
    if close_hover {
        ctx.set_fill_color(theme.close_hover());
        ctx.fill_rect(close_r.x, close_r.y, close_r.width, close_r.height);
    }

    ctx.set_text_align(TextAlign::Center);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.set_font("12px sans-serif");

    // Min: "—"
    ctx.set_fill_color(theme.icon_normal());
    ctx.fill_text("—", min_r.x + min_r.width / 2.0, min_r.y + min_r.height / 2.0);

    // Max / restore: "□" / "❐"
    let max_glyph = if state.is_maximized { "❐" } else { "□" };
    ctx.fill_text(max_glyph, max_r.x + max_r.width / 2.0, max_r.y + max_r.height / 2.0);

    // Close: "×"
    ctx.set_font("14px sans-serif");
    let close_color = if close_hover { theme.icon_hover() } else { theme.icon_normal() };
    ctx.set_fill_color(close_color);
    ctx.fill_text("×", close_r.x + close_r.width / 2.0, close_r.y + close_r.height / 2.0);
}

fn draw_multi_window(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    state:    &ChromeState,
    settings: &ChromeSettings,
) {
    let bw  = rect.width / 2.0;
    let new_r   = Rect { x: rect.x,      y: rect.y, width: bw, height: rect.height };
    let close_r = Rect { x: rect.x + bw, y: rect.y, width: bw, height: rect.height };

    use super::types::ChromeHit;
    let new_hov   = matches!(state.hovered, ChromeHit::NewWindowBtn);
    let close_hov = matches!(state.hovered, ChromeHit::CloseWindowBtn);

    draw_icon_btn(ctx, new_r,   "⊞", new_hov,   settings);
    draw_icon_btn(ctx, close_r, "⊠", close_hov, settings);
}

fn draw_undo_redo(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    _state:   &ChromeState,
    settings: &ChromeSettings,
) {
    let bw  = rect.width / 2.0;
    let undo_r = Rect { x: rect.x,      y: rect.y, width: bw, height: rect.height };
    let redo_r = Rect { x: rect.x + bw, y: rect.y, width: bw, height: rect.height };
    draw_icon_btn(ctx, undo_r, "↶", false, settings);
    draw_icon_btn(ctx, redo_r, "↷", false, settings);
}

fn draw_search(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    cfg:      &SearchConfig<'_>,
    _settings: &ChromeSettings,
) {
    // Embedded text-input: chrome's settings carry chrome theme/style;
    // the search slot uses default text-input settings here.  Apps
    // that need a custom search style can paint the slot themselves
    // (Toolbar slot escape hatch) and skip Slot::Search.
    let ti_settings = TextInputSettings::with_config(TextFieldConfig::search());
    let view = InputView {
        text:        cfg.text,
        placeholder: cfg.placeholder,
        cursor:      cfg.cursor,
        selection:   cfg.selection,
        focused:     cfg.focused,
        disabled:    cfg.disabled,
        input_type:  InputType::Search,
    };
    let st = if cfg.disabled       { WidgetState::Disabled }
             else if cfg.focused   { WidgetState::Pressed  }
             else                  { WidgetState::Normal   };
    let _ = draw_input(ctx, rect, st, &view, &ti_settings);
}

// ---------------------------------------------------------------------------
// chrome_layout_hit_test
// ---------------------------------------------------------------------------

/// Hit-test against a [`ChromeLayout`].  Walks the same zones as
/// [`draw_chrome_layout`] and returns the first slot whose
/// sub-rect contains the point — with the slot's internal hit
/// resolved (which tab, which window-control button, etc.).
pub fn chrome_layout_hit_test(
    rect:     Rect,
    state:    &ChromeState,
    settings: &ChromeSettings,
    layout:   &ChromeLayout<'_>,
    point:    (f64, f64),
) -> Option<ChromeHitPath> {
    let (px, py) = point;
    if px < rect.x || px > rect.x + rect.width
        || py < rect.y || py > rect.y + rect.height {
        return None;
    }

    let placements = walk(rect, layout, settings);
    for p in &placements {
        let r = p.rect;
        if px < r.x || px >= r.x + r.width { continue; }
        let slot = match p.zone {
            ChromeZone::Left   => &layout.left[p.slot_index],
            ChromeZone::Center => &layout.center[p.slot_index],
            ChromeZone::Right  => &layout.right[p.slot_index],
        };
        if let Some((kind, atomic_id)) = slot_hit(slot, r, state, settings, px) {
            return Some(ChromeHitPath {
                zone: p.zone, slot_index: p.slot_index, atomic_id, kind,
            });
        }
    }
    None
}

fn slot_hit(
    slot:     &Slot<'_>,
    rect:     Rect,
    _state:   &ChromeState,
    _settings: &ChromeSettings,
    px:       f64,
) -> Option<(ChromeHitKind, String)> {
    match slot {
        Slot::Tabs(cfg)        => tabs_hit(cfg, rect, px),
        Slot::Menu             => Some((ChromeHitKind::Menu, String::new())),
        Slot::WindowControls   => Some(window_controls_hit(rect, px)),
        Slot::MultiWindow      => Some(multi_window_hit(rect, px)),
        Slot::Search(_)        => Some((ChromeHitKind::Search, String::new())),
        Slot::UndoRedo         => Some(undo_redo_hit(rect, px)),
        Slot::UpdateBeacon     => Some((ChromeHitKind::BeaconBtn, String::new())),
        Slot::Toolbar(_)       => Some((ChromeHitKind::ToolbarItem, String::new())),
    }
}

fn tabs_hit(cfg: &TabsConfig<'_>, rect: Rect, px: f64) -> Option<(ChromeHitKind, String)> {
    if cfg.tabs.is_empty() && !cfg.show_new_btn { return None; }
    let avail = rect.width - if cfg.show_new_btn { 28.0 } else { 0.0 };
    let tab_w = if cfg.tabs.is_empty() { 0.0 }
                else { (avail / cfg.tabs.len() as f64).min(180.0).max(80.0) };
    let strip_end = rect.x + cfg.tabs.len() as f64 * tab_w;
    if px < strip_end {
        let i = ((px - rect.x) / tab_w).floor() as usize;
        let i = i.min(cfg.tabs.len().saturating_sub(1));
        let tab = &cfg.tabs[i];
        let close_x = rect.x + (i as f64 + 1.0) * tab_w - 18.0 - 4.0;
        if tab.closable && px >= close_x && px < close_x + 18.0 {
            return Some((ChromeHitKind::TabClose, format!("tab:{}", i)));
        }
        return Some((ChromeHitKind::Tab, format!("tab:{}", i)));
    }
    if cfg.show_new_btn && px >= strip_end && px < strip_end + 28.0 {
        return Some((ChromeHitKind::NewTab, String::from("new_tab")));
    }
    None
}

fn window_controls_hit(rect: Rect, px: f64) -> (ChromeHitKind, String) {
    let bw = rect.width / 3.0;
    let local = px - rect.x;
    if local < bw          { (ChromeHitKind::MinBtn,      String::from("min_btn")) }
    else if local < bw*2.0 { (ChromeHitKind::MaxBtn,      String::from("max_btn")) }
    else                   { (ChromeHitKind::CloseAppBtn, String::from("close_btn")) }
}

fn multi_window_hit(rect: Rect, px: f64) -> (ChromeHitKind, String) {
    let bw = rect.width / 2.0;
    if px < rect.x + bw { (ChromeHitKind::NewWindowBtn,   String::from("new_window_btn"))   }
    else                { (ChromeHitKind::CloseWindowBtn, String::from("close_window_btn")) }
}

fn undo_redo_hit(rect: Rect, px: f64) -> (ChromeHitKind, String) {
    let bw = rect.width / 2.0;
    if px < rect.x + bw { (ChromeHitKind::UndoBtn, String::from("undo_btn")) }
    else                { (ChromeHitKind::RedoBtn, String::from("redo_btn")) }
}
