//! Chrome strip and resize bezel: typed hits and what a press / click on
//! them means. The chrome part under a point comes from the library
//! composite's own hit-test (`chrome_hit_test`) run against the window's
//! [`ChromeModel`], so the zones match what the composite draws.

use uzor::layout::docking::DockPanel;
use uzor::widgets::composite::chrome::{
    chrome_hit_test, handle_chrome_action, ChromeAction, ChromeHit, ChromeRenderKind,
    ChromeSettings, ChromeTabConfig, ChromeView,
};
use uzor::{Rect, ResizeDirection};

use crate::types::command::{ChromeKind, ChromeModel, LayoutHit};
use crate::types::ids::WindowId;
use crate::types::ops::LayoutEffect;
use crate::types::window::{Point, WindowCommand};

use super::{rects, LayoutEffects, Session, WindowLayout};

/// The observable chrome state of a window.
#[derive(PartialEq)]
pub(super) struct ChromeObs {
    visible: bool,
    height: f32,
    rect: Option<Rect>,
    model: ModelObs,
}

/// [`ChromeModel`] compared field by field (the library flags type has no
/// `PartialEq`).
struct ModelObs(ChromeModel);

impl PartialEq for ModelObs {
    fn eq(&self, other: &Self) -> bool {
        self.0.same_as(&other.0)
    }
}

impl ChromeObs {
    pub(super) fn of<P: DockPanel>(w: &WindowLayout<P>) -> Self {
        Self {
            visible: w.chrome_slot.visible,
            height: w.chrome_slot.height,
            rect: w.solved.chrome,
            model: ModelObs(w.chrome.clone()),
        }
    }
}

/// The uzor resize bezel: a `px`-wide band along the viewport border.
pub(super) fn bezel(vp: Rect, px: f64, p: Point) -> Option<ResizeDirection> {
    if px.is_nan() || px <= 0.0 {
        return None;
    }
    let left = p.x < vp.x + px;
    let right = p.x >= vp.x + vp.width - px;
    let top = p.y < vp.y + px;
    let bottom = p.y >= vp.y + vp.height - px;
    match (top, bottom, left, right) {
        (true, _, true, _) => Some(ResizeDirection::NorthWest),
        (true, _, _, true) => Some(ResizeDirection::NorthEast),
        (_, true, true, _) => Some(ResizeDirection::SouthWest),
        (_, true, _, true) => Some(ResizeDirection::SouthEast),
        (true, _, _, _) => Some(ResizeDirection::North),
        (_, true, _, _) => Some(ResizeDirection::South),
        (_, _, true, _) => Some(ResizeDirection::West),
        (_, _, _, true) => Some(ResizeDirection::East),
        _ => None,
    }
}

/// The chrome part under `p`, or `None` when `p` is not in the chrome
/// strip. Points in the composite's border zone are classified as the part
/// just inside it (the bezel owns window resize).
pub(super) fn hit<P: DockPanel>(w: &WindowLayout<P>, p: Point) -> Option<LayoutHit> {
    let rect = w.solved.chrome?;
    if !rects::contains(rect, p) {
        return None;
    }
    let settings = ChromeSettings::default();
    let style = settings.style.as_ref();
    let border = style.border_zone();
    let strip_h = style.chrome_height();
    let tabs: Vec<ChromeTabConfig<'_>> = w
        .chrome
        .tabs
        .iter()
        .map(|t| ChromeTabConfig {
            id: &t.label,
            label: &t.label,
            icon: None,
            color_tag: None,
            closable: t.closable,
            active: false,
        })
        .collect();
    let b = w.chrome.buttons;
    let view = ChromeView {
        tabs: &tabs,
        active_tab_id: None,
        show_new_tab_btn: b.show_new_tab_btn,
        show_menu_btn: b.show_menu_btn,
        show_new_window_btn: b.show_new_window_btn,
        show_close_window_btn: b.show_close_window_btn,
        is_maximized: false,
        menu_left: b.menu_left,
        show_maximize: b.show_maximize,
        cursor_x: p.x,
        cursor_y: p.y,
        time_ms: 0.0,
    };
    let kind = render_kind(w.chrome.kind);
    let test =
        |x: f64, y: f64| chrome_hit_test(&w.chrome_state, &view, &settings, &kind, rect, (x, y));
    let mut part = test(p.x, p.y);
    if is_resize(part) {
        // Nudge into the strip interior, past the border zone.
        let eps = 0.5;
        let x = p.x.clamp(
            rect.x + border + eps,
            (rect.x + rect.width - border - eps).max(rect.x),
        );
        let y = p.y.clamp(
            rect.y + border + eps,
            (rect.y + strip_h - border - eps).max(rect.y),
        );
        part = test(x, y);
        if is_resize(part) {
            part = ChromeHit::None;
        }
    }
    Some(LayoutHit::Chrome(part))
}

fn render_kind(kind: ChromeKind) -> ChromeRenderKind {
    match kind {
        ChromeKind::Default => ChromeRenderKind::Default,
        ChromeKind::Minimal => ChromeRenderKind::Minimal,
        ChromeKind::WindowControlsOnly => ChromeRenderKind::WindowControlsOnly,
    }
}

fn is_resize(h: ChromeHit) -> bool {
    matches!(
        h,
        ChromeHit::ResizeCorner(_)
            | ChromeHit::ResizeTop
            | ChromeHit::ResizeBottom
            | ChromeHit::ResizeLeft
            | ChromeHit::ResizeRight
    )
}

/// A primary press on a chrome part: the drag zone hands the gesture to the
/// OS at once; buttons and tabs start a click session (they act on
/// release); inert chrome does nothing.
pub(super) fn press(win: WindowId, part: ChromeHit, fx: &mut LayoutEffects) -> Option<Session> {
    match handle_chrome_action(part) {
        ChromeAction::WindowDragStart => {
            fx.push(LayoutEffect::Window {
                win,
                cmd: WindowCommand::DragWindow,
            });
            None
        }
        ChromeAction::None | ChromeAction::BeginResize(_) => None,
        _ => Some(Session::Click(LayoutHit::Chrome(part))),
    }
}

/// A completed click on a chrome part (the kernel maps the action).
pub(super) fn click(win: WindowId, part: ChromeHit, fx: &mut LayoutEffects) {
    let action = handle_chrome_action(part);
    if !matches!(
        action,
        ChromeAction::None | ChromeAction::WindowDragStart | ChromeAction::BeginResize(_)
    ) {
        fx.push(LayoutEffect::Chrome { win, action });
    }
}
