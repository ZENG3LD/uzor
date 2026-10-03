//! Widget-catalog dashboard on the `uzor-framework` [`App`] (brief E1).
//!
//! One window: a Settings leaf and the four painting cells from the old
//! `l4/dashboard.rs`, drawn with [`view!`]. The window strip is
//! [`App::chrome_model`] (its new-window button is `WindowIntent::NewWindowRequested`).
//! `<chrome>` is also in the settings tree — that tag is part of the M1
//! surface — but the macro paints it at the measured origin. It does not
//! forward the flex rect, and M1 is not changed here.
//!
//! Left on the old door, so not in this crate: `uzor-desktop` render
//! control (backend, vsync, MSAA, fps limit), the tree-debug blackbox,
//! tray and the png icon, and per-cell `RenderRegion` cadence. `panel`
//! runs for every visible leaf of a frame, so the four cells share the
//! window clock. Their titles keep the old cadence names.

use std::cell::Cell;
use std::collections::BTreeMap;

use uzor::layout::docking::DockPanel;
use uzor::tokens::{BuiltinSet, Tokens};
use uzor::{CornerStyle, Rect};

use uzor_framework::view;
use uzor_framework::{
    App, AppCommand, ChromeModel, ChromeTab, DockIntent, DockTarget, Intent, IntentCx, LayoutCmd,
    LeafView, OverlayCx, OverlayModel, PanelCx, PanelHome, RuntimeConfig, SizePx, Spec, SplitDir,
    ThemeCmd, WindowCmd, WindowId, WindowIntent, WindowSpec,
};

const ID_SETTINGS: &str = "settings";
const ID_DIRTY: &str = "paint:r0_dirty";
const ID_30: &str = "paint:r1_30fps";
const ID_120: &str = "paint:r2_120fps";
const ID_UNCAP: &str = "paint:r3_uncap";

/// Dock header is 24px and is painted over the leaf rect. Content starts
/// below it.
const CONTENT_TOP: f64 = 32.0;
const CONTENT_X: f64 = 8.0;
const COL_GAP: f64 = 6.0;
const COL_PAD: f64 = 8.0;

// Settings column, top to bottom. `theme_button_center` solves the same
// list; the button row is index 4.
const CHROME_ROW: f64 = 32.0;
const TITLE_ROW: f64 = 20.0;
const SEP_ROW: f64 = 1.0;
const LABEL_ROW: f64 = 16.0;
const BUTTON_ROW: f64 = 28.0;
const CHECK_ROW: f64 = 24.0;
const STATUS_ROW: f64 = 18.0;
#[cfg(test)]
const BUTTON_INDEX: usize = 4;

/// The five dock leaves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DashPanel {
    /// Theme, checkbox, toggle.
    Settings,
    /// Old dirty-driven cell.
    PaintDirty,
    /// Old 30 fps cell.
    Paint30,
    /// Old 120 fps cell.
    Paint120,
    /// Old uncapped cell.
    PaintUncap,
}

impl DashPanel {
    fn from_type_id(type_id: &str) -> Option<Self> {
        Some(match type_id {
            ID_SETTINGS => Self::Settings,
            ID_DIRTY => Self::PaintDirty,
            ID_30 => Self::Paint30,
            ID_120 => Self::Paint120,
            ID_UNCAP => Self::PaintUncap,
            _ => return None,
        })
    }

    fn paint_index(self) -> Option<usize> {
        Some(match self {
            Self::PaintDirty => 0,
            Self::Paint30 => 1,
            Self::Paint120 => 2,
            Self::PaintUncap => 3,
            Self::Settings => return None,
        })
    }

    fn blurb(self) -> &'static str {
        match self {
            Self::PaintDirty => "dirty-driven",
            Self::Paint30 => "30 fps",
            Self::Paint120 => "120 fps",
            Self::PaintUncap => "uncapped",
            Self::Settings => "",
        }
    }

    fn title_id(self) -> &'static str {
        match self {
            Self::PaintDirty => "paint:r0_dirty:title",
            Self::Paint30 => "paint:r1_30fps:title",
            Self::Paint120 => "paint:r2_120fps:title",
            Self::PaintUncap => "paint:r3_uncap:title",
            Self::Settings => "settings:title",
        }
    }

    fn note_id(self) -> &'static str {
        match self {
            Self::PaintDirty => "paint:r0_dirty:note",
            Self::Paint30 => "paint:r1_30fps:note",
            Self::Paint120 => "paint:r2_120fps:note",
            Self::PaintUncap => "paint:r3_uncap:note",
            Self::Settings => "settings:note",
        }
    }

    fn sep_id(self) -> &'static str {
        match self {
            Self::PaintDirty => "paint:r0_dirty:sep",
            Self::Paint30 => "paint:r1_30fps:sep",
            Self::Paint120 => "paint:r2_120fps:sep",
            Self::PaintUncap => "paint:r3_uncap:sep",
            Self::Settings => "settings:sep-cell",
        }
    }
}

impl DockPanel for DashPanel {
    fn title(&self) -> &str {
        match self {
            Self::Settings => "Settings",
            Self::PaintDirty => "fps = 0 (dirty)",
            Self::Paint30 => "fps = 30",
            Self::Paint120 => "fps = 120",
            Self::PaintUncap => "uncapped",
        }
    }

    fn type_id(&self) -> &'static str {
        match self {
            Self::Settings => ID_SETTINGS,
            Self::PaintDirty => ID_DIRTY,
            Self::Paint30 => ID_30,
            Self::Paint120 => ID_120,
            Self::PaintUncap => ID_UNCAP,
        }
    }

    fn min_size(&self) -> (f32, f32) {
        match self {
            Self::Settings => (200.0, 160.0),
            _ => (120.0, 80.0),
        }
    }

    fn closable(&self) -> bool {
        false
    }
}

/// No overlays. Modal / popup / dropdown / context-menu tags are a
/// compile error after M1; this dashboard does not open any.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DashOverlay {}

/// No keymap. Theme is the two settings buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DashAction {}

/// App vocabulary.
#[derive(Clone, Copy, Debug, Default)]
pub struct DashSpec;

impl Spec for DashSpec {
    type Panel = DashPanel;
    type Overlay = DashOverlay;
    type Action = DashAction;

    fn decode_panel(_home: PanelHome, type_id: &str) -> Option<DashPanel> {
        DashPanel::from_type_id(type_id)
    }
}

/// The dashboard.
pub struct DashboardApp {
    dark: bool,
    show_counts: bool,
    live: bool,
    counts: [u64; 4],
    /// 0 = settings only, then one split per step, 4 = grid complete.
    phase: BTreeMap<WindowId, u8>,
    extra: u32,
}

impl Default for DashboardApp {
    fn default() -> Self {
        Self::new()
    }
}

impl DashboardApp {
    /// Dark theme, rebuild counters visible, live repaint off.
    pub fn new() -> Self {
        Self {
            dark: true,
            show_counts: true,
            live: false,
            counts: [0; 4],
            phase: BTreeMap::new(),
            extra: 0,
        }
    }

    /// One undecorated window, dark tokens. Same size as the old L4 bin.
    pub fn runtime_config() -> RuntimeConfig {
        RuntimeConfig {
            windows: vec![main_window()],
            tokens: Tokens::builtin(BuiltinSet::Dark),
            dark: true,
            ..RuntimeConfig::default()
        }
    }

    /// `true` while the dark token set is the one the buttons last asked for.
    pub fn is_dark(&self) -> bool {
        self.dark
    }

    fn paint_settings(&mut self, cx: &mut PanelCx<'_, DashSpec>) {
        let dark = self.dark;
        let dt_ms = cx.time.dt * 1000.0;
        let status = format!(
            "{}   {:.1} ms",
            if dark { "theme dark" } else { "theme light" },
            dt_ms
        );
        let theme_click: Cell<Option<bool>> = Cell::new(None);
        let show_counts = &mut self.show_counts;
        let live = &mut self.live;
        let body = content_body(cx.rect);
        view! {
            <col rect={body} gap={COL_GAP} pad={COL_PAD}>
                <row flex=0.0 size={CHROME_ROW}>
                    <chrome show_new_window=true />
                </row>
                <text flex=0.0 size={TITLE_ROW} id="settings:title" text="Settings" />
                <separator flex=0.0 size={SEP_ROW} id="settings:sep0" />
                <text flex=0.0 size={LABEL_ROW} id="settings:theme_lbl" text="Theme" />
                <row flex=0.0 size={BUTTON_ROW} gap={COL_GAP}>
                    <button id="settings:theme_dark" text="Dark" active={dark} on_click={|| theme_click.set(Some(true))} />
                    <button id="settings:theme_light" text="Light" active={!dark} on_click={|| theme_click.set(Some(false))} />
                </row>
                <separator flex=0.0 size={SEP_ROW} id="settings:sep1" />
                <checkbox flex=0.0 size={CHECK_ROW} id="settings:counts" bind={show_counts} label="Rebuild counters" />
                <toggle flex=0.0 size={CHECK_ROW} id="settings:live" bind={live} label="Live repaint" />
                <text flex=1.0 size={STATUS_ROW} id="settings:status" text={status.as_str()} />
            </col>
        }
        if let Some(dark) = theme_click.get() {
            if dark != self.dark {
                self.dark = dark;
                let set = if dark {
                    BuiltinSet::Dark
                } else {
                    BuiltinSet::Light
                };
                cx.out
                    .command(AppCommand::Theme(ThemeCmd::SetTokens(Tokens::builtin(set))));
                cx.out.command(AppCommand::Theme(ThemeCmd::SetDark(dark)));
            }
        }
        if self.live {
            cx.out.request_frame();
        }
    }

    fn paint_cell(&mut self, cx: &mut PanelCx<'_, DashSpec>, panel: DashPanel) {
        let Some(idx) = panel.paint_index() else {
            return;
        };
        self.counts[idx] = self.counts[idx].wrapping_add(1);
        let count = self.counts[idx];
        let line = if self.show_counts {
            format!("{}   {count}", panel.title())
        } else {
            panel.title().to_string()
        };
        let body = content_body(cx.rect);
        let title_id = panel.title_id();
        let sep_id = panel.sep_id();
        let note_id = panel.note_id();
        let note = panel.blurb();
        view! {
            <col rect={body} gap={COL_GAP} pad={COL_PAD}>
                <text flex=0.0 size={TITLE_ROW} id={title_id} text={line.as_str()} />
                <separator flex=0.0 size={SEP_ROW} id={sep_id} />
                <text flex=1.0 size={LABEL_ROW} id={note_id} text={note} />
            </col>
        }
    }

    fn on_layout(&mut self, win: WindowId, cx: &mut IntentCx<'_, DashSpec>) {
        let Some(phase) = self.phase.get(&win).copied() else {
            return;
        };
        let leaves = cx
            .view
            .window(win)
            .map(|w| w.dock.leaves.clone())
            .unwrap_or_default();
        let has = |id: &str| leaf_with(&leaves, id).is_some();
        let mut split = |type_id: &str, dir: SplitDir, panel: DashPanel| -> bool {
            let Some(leaf) = leaf_with(&leaves, type_id) else {
                return false;
            };
            cx.command(AppCommand::Layout(LayoutCmd::Split {
                win,
                leaf,
                dir,
                panel,
            }));
            true
        };
        let next = match phase {
            0 if has(ID_SETTINGS) && !has(ID_DIRTY) => {
                split(ID_SETTINGS, SplitDir::Right, DashPanel::PaintDirty).then_some(1)
            }
            1 if has(ID_DIRTY) && !has(ID_120) => {
                split(ID_DIRTY, SplitDir::Down, DashPanel::Paint120).then_some(2)
            }
            2 if has(ID_DIRTY) && !has(ID_30) => {
                split(ID_DIRTY, SplitDir::Right, DashPanel::Paint30).then_some(3)
            }
            3 if has(ID_120) && !has(ID_UNCAP) => {
                split(ID_120, SplitDir::Right, DashPanel::PaintUncap).then_some(4)
            }
            _ => None,
        };
        if let Some(next) = next {
            self.phase.insert(win, next);
        }
    }
}

impl App for DashboardApp {
    type Spec = DashSpec;

    fn chrome_model(&self, _win: WindowId) -> ChromeModel {
        ChromeModel {
            tabs: vec![ChromeTab {
                label: "Dashboard".into(),
                closable: false,
            }],
            ..ChromeModel::default()
        }
    }

    fn overlay_model(&self, _win: WindowId, id: DashOverlay) -> OverlayModel {
        match id {}
    }

    fn panel(&mut self, cx: &mut PanelCx<'_, DashSpec>) {
        match *cx.panel {
            DashPanel::Settings => self.paint_settings(cx),
            other => self.paint_cell(cx, other),
        }
    }

    fn overlay_body(&mut self, cx: &mut OverlayCx<'_, DashSpec>) {
        match cx.id {}
    }

    fn intent(&mut self, intent: Intent<DashSpec>, cx: &mut IntentCx<'_, DashSpec>) {
        match intent {
            Intent::Window(WindowIntent::Opened { win, .. }) => {
                if self.phase.insert(win, 0).is_none() {
                    cx.command(AppCommand::Layout(LayoutCmd::SetChrome {
                        win,
                        visible: true,
                        height: 32.0,
                    }));
                    cx.command(AppCommand::Layout(LayoutCmd::OpenPanel {
                        win,
                        panel: DashPanel::Settings,
                        at: DockTarget::Root,
                    }));
                }
            }
            Intent::Window(WindowIntent::NewWindowRequested { .. }) => {
                self.extra = self.extra.wrapping_add(1);
                let spec = extra_window(self.extra);
                cx.command(AppCommand::Window(WindowCmd::Open(spec)));
            }
            Intent::Window(WindowIntent::Closed { win }) => {
                self.phase.remove(&win);
            }
            Intent::Dock(DockIntent::LayoutChanged { win, .. }) => self.on_layout(win, cx),
            Intent::Action(action) => match action {},
            Intent::Overlay(_)
            | Intent::Window(_)
            | Intent::Dock(_)
            | Intent::Text(_)
            | Intent::Drop(_)
            | Intent::Timer(_)
            | Intent::Screenshot { .. }
            | Intent::Tray(_)
            | Intent::Unhandled(_) => {}
        }
    }
}

fn main_window() -> WindowSpec {
    let mut spec = WindowSpec::new("main", "uzor — L4 Dashboard", SizePx::new(1400, 900));
    spec.min_inner_size = Some(SizePx::new(900, 600));
    spec.decorations = false;
    spec.corner_style = CornerStyle::Rounded;
    spec.border_color = Some(0xFB_B2_6A);
    spec
}

fn extra_window(n: u32) -> WindowSpec {
    let mut spec = WindowSpec::new(format!("extra-{n}"), "uzor — extra", SizePx::new(560, 420));
    spec.min_inner_size = Some(SizePx::new(420, 320));
    spec.decorations = false;
    spec.corner_style = CornerStyle::Rounded;
    spec
}

fn content_body(panel: Rect) -> Rect {
    Rect::new(
        panel.x + CONTENT_X,
        panel.y + CONTENT_TOP,
        (panel.width - CONTENT_X * 2.0).max(1.0),
        (panel.height - CONTENT_TOP - CONTENT_X).max(1.0),
    )
}

fn leaf_with(leaves: &[LeafView], type_id: &str) -> Option<uzor::layout::docking::LeafId> {
    leaves.iter().find_map(|leaf| {
        leaf.panels
            .iter()
            .any(|p| p.type_id == type_id)
            .then_some(leaf.leaf)
    })
}

/// Column children in the same order as `paint_settings`.
#[cfg(test)]
fn settings_children() -> [uzor_framework::flex::FlexChild; 9] {
    use uzor_framework::flex::FlexChild;
    let fixed = |basis| FlexChild { basis, flex: 0.0 };
    [
        fixed(CHROME_ROW),
        fixed(TITLE_ROW),
        fixed(SEP_ROW),
        fixed(LABEL_ROW),
        fixed(BUTTON_ROW),
        fixed(SEP_ROW),
        fixed(CHECK_ROW),
        fixed(CHECK_ROW),
        FlexChild {
            basis: STATUS_ROW,
            flex: 1.0,
        },
    ]
}

/// Center of the Dark (`light == false`) or Light button, in the same
/// coordinates `paint_settings` gives `view!`.
#[cfg(test)]
fn theme_button_center(panel: Rect, light: bool) -> (f64, f64) {
    use uzor_framework::flex::{FlexChild, FlexDir};
    let body = content_body(panel);
    let col = uzor_framework::flex::flex_solve(
        body,
        FlexDir::Col,
        COL_GAP,
        COL_PAD,
        &settings_children(),
    );
    let row = col[BUTTON_INDEX];
    let buttons = uzor_framework::flex::flex_solve(
        row,
        FlexDir::Row,
        COL_GAP,
        0.0,
        &[
            FlexChild {
                basis: 0.0,
                flex: 1.0,
            },
            FlexChild {
                basis: 0.0,
                flex: 1.0,
            },
        ],
    );
    let r = buttons[usize::from(light)];
    (r.x + r.width * 0.5, r.y + r.height * 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    use uzor::input::{ModifierKeys, MouseButton};
    use uzor_framework::{HeadlessHost, InputEvent, Point, PointerInput};

    fn panel_ids(host: &HeadlessHost<DashSpec, DashboardApp>) -> Vec<&'static str> {
        let snap = host.snapshot();
        let mut ids = Vec::new();
        for win in &snap.windows {
            for leaf in &win.dock.leaves {
                for panel in &leaf.panels {
                    ids.push(panel.type_id);
                }
            }
        }
        ids
    }

    fn settings_rect(host: &HeadlessHost<DashSpec, DashboardApp>) -> Rect {
        let snap = host.snapshot();
        let win = snap.windows.first().expect("window");
        win.dock
            .leaves
            .iter()
            .find(|leaf| leaf.panels.iter().any(|p| p.type_id == ID_SETTINGS))
            .expect("settings leaf")
            .rect
    }

    fn open_settled() -> HeadlessHost<DashSpec, DashboardApp> {
        let (mut host, _) = HeadlessHost::new(DashboardApp::new(), DashboardApp::runtime_config());
        for _ in 0..8 {
            host.advance(0.02);
            if panel_ids(&host).len() >= 5 {
                break;
            }
        }
        host
    }

    fn click(host: &mut HeadlessHost<DashSpec, DashboardApp>, x: f64, y: f64) {
        let win = host.snapshot().windows[0].id;
        host.input(
            win,
            InputEvent::Pointer(PointerInput::Down {
                pos: Point::new(x, y),
                button: MouseButton::Left,
                mods: ModifierKeys::default(),
            }),
        );
        host.advance(0.02);
        host.input(
            win,
            InputEvent::Pointer(PointerInput::Up {
                pos: Point::new(x, y),
                button: MouseButton::Left,
                mods: ModifierKeys::default(),
            }),
        );
        host.advance(0.02);
        // The frame that reports the click to the hook, then the tick that
        // drains the ThemeCmd the hook queued.
        host.advance(0.02);
        host.advance(0.02);
    }

    #[test]
    fn dashboard_opens_settings_and_paint_grid() {
        let host = open_settled();
        let mut ids = panel_ids(&host);
        ids.sort_unstable();
        let mut expect = vec![ID_120, ID_30, ID_DIRTY, ID_SETTINGS, ID_UNCAP];
        expect.sort_unstable();
        assert_eq!(ids, expect, "dock leaves after settle");
        let rect = settings_rect(&host);
        assert!(rect.width > 100.0 && rect.height > 100.0, "{rect:?}");
        assert!(host.runtime().app().is_dark());
    }

    #[test]
    fn dashboard_theme_buttons_switch_token_set() {
        let mut host = open_settled();
        let before = host.snapshot().theme_rev;
        let rect = settings_rect(&host);
        let (x, y) = theme_button_center(rect, true);
        assert!(
            y > rect.y + 24.0,
            "light button {y} overlaps the header of {rect:?}"
        );
        click(&mut host, x, y);
        assert!(
            !host.runtime().app().is_dark(),
            "light button at ({x}, {y}) rect {rect:?}"
        );
        assert!(host.snapshot().theme_rev > before);

        let rect = settings_rect(&host);
        let (x, y) = theme_button_center(rect, false);
        click(&mut host, x, y);
        assert!(host.runtime().app().is_dark());
    }
}
