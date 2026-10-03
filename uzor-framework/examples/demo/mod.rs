//! One [`DemoApp`] for the native and web entries (design §9.3, brief F11).
//!
//! No `cfg`: both entry points construct [`DemoApp::new`] and
//! [`DemoApp::runtime_config`]. The app drives two windows on a multi-window
//! host, a dock of three panel kinds with splitters, tab tear-off and
//! drag-out (policy), one overlay of each kind, Ctrl+K / Esc, a text field
//! registered for IME, layout save / restore through an in-memory stand-in
//! for the back office, expand gutters (policy) and a theme switch.

use std::collections::{BTreeMap, BTreeSet};

use uzor::input::keyboard::KeyboardShortcut;
use uzor::input::{KeyCode, ModifierKeys, TextFieldConfig};
use uzor::layout::docking::{DockPanel, Pin};
use uzor::layout::OverlayKind;
use uzor::render::{TextAlign, TextBaseline};
use uzor::tokens::{BuiltinSet, TokenTheme, Tokens};
use uzor::ui::widgets::atomic::button::{ButtonSettings, ButtonView};
use uzor::ui::widgets::atomic::text::{TextOverflow, TextSettings, TextView};
use uzor::ui::widgets::atomic::text_input::render::{draw_input, InputView};
use uzor::ui::widgets::atomic::text_input::settings::TextInputSettings;
use uzor::ui::widgets::atomic::text_input::types::InputType;
use uzor::{Rect, WidgetId, WidgetState};
use uzor_framework::widgets::{button, text};
use uzor_framework::{
    App, AppCommand, Binding, ChromeModel, ChromeTab, ContextMenuEntry, ContextMenuModel,
    DockTarget, DragOutChrome, DragOutPolicy, DropdownModel, FocusCmd, Intent, IntentCx, KeymapCmd,
    KeymapScope, LayoutBlob, LayoutCmd, LayoutPolicy, ModalModel, OverlayCmd, OverlayCx,
    OverlayModel, OverlaySize, PanelCx, PanelHome, RuntimeConfig, SizePx, Spec, SplitDir, ThemeCmd,
    Ticket, TooltipModel, WindowId, WindowSpec,
};

/// Canvas id the web entry binds.
#[allow(dead_code)]
pub const CANVAS_ID: &str = "uzor-demo";

/// The three dock panel kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DemoPanel {
    /// Tree of commands (settings, theme, layout).
    Navigator,
    /// The text field.
    Editor,
    /// Pinned (`Pin::User { pinned: true }`): it does not tear off.
    Inspector,
}

impl DockPanel for DemoPanel {
    fn title(&self) -> &str {
        match self {
            DemoPanel::Navigator => "Navigator",
            DemoPanel::Editor => "Editor",
            DemoPanel::Inspector => "Inspector",
        }
    }

    fn type_id(&self) -> &'static str {
        match self {
            DemoPanel::Navigator => "nav",
            DemoPanel::Editor => "editor",
            DemoPanel::Inspector => "inspector",
        }
    }

    fn min_size(&self) -> (f32, f32) {
        (96.0, 64.0)
    }

    fn pin(&self) -> Pin {
        match self {
            DemoPanel::Inspector => Pin::User { pinned: true },
            _ => Pin::Free,
        }
    }
}

/// One overlay of each kind the kernel knows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DemoOverlay {
    /// Settings modal (text field, focus ring).
    Settings,
    /// Context menu.
    Menu,
    /// Ctrl+K command palette.
    Palette,
    /// Hover tooltip on the navigator.
    Tooltip,
}

/// Keymap targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum DemoAction {
    /// Open the palette.
    Palette,
    /// Toggle dark / light tokens.
    Theme,
    /// Ask the kernel for a layout blob and keep it in memory.
    SaveLayout,
    /// Restore the blob kept by [`DemoApp::office`].
    RestoreLayout,
    /// Close the top overlay (Esc).
    Dismiss,
}

pub struct DemoSpec;

impl Spec for DemoSpec {
    type Panel = DemoPanel;
    type Overlay = DemoOverlay;
    type Action = DemoAction;

    fn decode_panel(_home: PanelHome, type_id: &str) -> Option<DemoPanel> {
        match type_id {
            "nav" => Some(DemoPanel::Navigator),
            "editor" => Some(DemoPanel::Editor),
            "inspector" => Some(DemoPanel::Inspector),
            _ => None,
        }
    }
}

/// Settings button.
pub fn id_settings() -> WidgetId {
    WidgetId::from("demo-settings")
}
/// Menu button.
pub fn id_menu() -> WidgetId {
    WidgetId::from("demo-menu")
}
/// Theme switch.
pub fn id_theme() -> WidgetId {
    WidgetId::from("demo-theme")
}
/// Save layout.
pub fn id_save() -> WidgetId {
    WidgetId::from("demo-save")
}
/// Restore layout.
pub fn id_restore() -> WidgetId {
    WidgetId::from("demo-restore")
}
/// Tooltip anchor.
pub fn id_tip() -> WidgetId {
    WidgetId::from("demo-tip")
}
/// Editor text field (IME).
pub fn id_note() -> WidgetId {
    WidgetId::from("demo-note")
}
/// Settings-modal text field.
pub fn id_settings_note() -> WidgetId {
    WidgetId::from("demo-settings-note")
}

/// Content rect inside a leaf (below the header the kernel paints).
pub fn content_body(panel: Rect) -> Rect {
    Rect::new(
        panel.x + 8.0,
        panel.y + 32.0,
        (panel.width - 16.0).max(1.0),
        (panel.height - 40.0).max(1.0),
    )
}

/// Navigator button rects in the same coordinates `panel` draws.
pub struct DemoHits {
    /// Settings.
    pub settings: Rect,
    /// Context menu.
    pub menu: Rect,
    /// Theme.
    pub theme: Rect,
    /// Save.
    pub save: Rect,
    /// Restore.
    pub restore: Rect,
    /// Tooltip anchor.
    pub tip: Rect,
    /// Editor text field.
    pub note: Rect,
}

/// Hit rects for one leaf's content.
pub fn navigator_hits(panel: Rect) -> DemoHits {
    let b = content_body(panel);
    let w = b.width.min(160.0).max(1.0);
    let h = 22.0;
    let g = 4.0;
    let at = |row: f64| Rect::new(b.x, b.y + row * (h + g), w, h);
    DemoHits {
        settings: at(0.0),
        menu: at(1.0),
        theme: at(2.0),
        save: at(3.0),
        restore: at(4.0),
        tip: at(5.0),
        note: Rect::new(b.x, b.y, b.width.min(280.0), 28.0),
    }
}

/// Text field inside a settings modal body.
pub fn settings_field(body: Rect) -> Rect {
    Rect::new(
        body.x + 12.0,
        body.y + 12.0,
        (body.width - 24.0).max(40.0),
        32.0,
    )
}

/// The demo. [`DemoApp::office`] is the in-memory back-office stand-in
/// (layout blobs keyed by window label).
pub struct DemoApp {
    keys: BTreeMap<WindowId, String>,
    mains: BTreeSet<WindowId>,
    phase: BTreeMap<WindowId, u8>,
    blob_asked: BTreeSet<WindowId>,
    office: BTreeMap<String, LayoutBlob>,
    dark: bool,
}

impl Default for DemoApp {
    fn default() -> Self {
        Self::new()
    }
}

impl DemoApp {
    /// Empty office, dark theme.
    pub fn new() -> Self {
        Self {
            keys: BTreeMap::new(),
            mains: BTreeSet::new(),
            phase: BTreeMap::new(),
            blob_asked: BTreeSet::new(),
            office: BTreeMap::new(),
            dark: true,
        }
    }

    /// Two windows, drag-out enabled, expand gutters on, dark tokens.
    ///
    /// A single-canvas host ignores the second spawn (`multi_window: false`);
    /// the app still asks for both.
    pub fn runtime_config() -> RuntimeConfig {
        RuntimeConfig {
            windows: vec![
                WindowSpec::new("main", "Uzor demo", SizePx::new(480, 320)),
                WindowSpec::new("side", "Uzor demo — side", SizePx::new(280, 200)),
            ],
            tokens: Tokens::builtin(BuiltinSet::Dark),
            dark: true,
            layout: LayoutPolicy {
                drag_out: DragOutPolicy::Enabled {
                    chrome: DragOutChrome::UzorChrome,
                },
                edge_expand_px: 16.0,
                expand_thickness_px: 160.0,
                tab_new_button: true,
                ..LayoutPolicy::default()
            },
            ..RuntimeConfig::default()
        }
    }

    /// Blobs stored for the back office, keyed by window label.
    #[allow(dead_code)]
    pub fn office(&self) -> &BTreeMap<String, LayoutBlob> {
        &self.office
    }

    /// `true` while the demo is on the dark token set.
    #[allow(dead_code)]
    pub fn is_dark(&self) -> bool {
        self.dark
    }

    fn theme_set(&self) -> BuiltinSet {
        if self.dark {
            BuiltinSet::Dark
        } else {
            BuiltinSet::Light
        }
    }
}

trait CmdSink {
    fn cmd(&mut self, cmd: AppCommand<DemoSpec>);
}

impl CmdSink for IntentCx<'_, DemoSpec> {
    fn cmd(&mut self, cmd: AppCommand<DemoSpec>) {
        self.command(cmd);
    }
}

struct HookSink<'a>(&'a mut uzor_framework::HookOps<DemoSpec>);

impl CmdSink for HookSink<'_> {
    fn cmd(&mut self, cmd: AppCommand<DemoSpec>) {
        self.0.command(cmd);
    }
}

impl DemoApp {
    fn toggle_theme<C: CmdSink>(&mut self, cx: &mut C) {
        self.dark = !self.dark;
        cx.cmd(AppCommand::Theme(ThemeCmd::SetTokens(Tokens::builtin(
            self.theme_set(),
        ))));
        cx.cmd(AppCommand::Theme(ThemeCmd::SetDark(self.dark)));
    }

    fn save<C: CmdSink>(&mut self, cx: &mut C, win: WindowId) {
        cx.cmd(AppCommand::Layout(LayoutCmd::RequestBlob {
            win,
            ticket: Ticket(7),
        }));
    }

    fn restore<C: CmdSink>(&self, cx: &mut C, win: WindowId) {
        let Some(key) = self.keys.get(&win) else {
            return;
        };
        let Some(blob) = self.office.get(key).cloned() else {
            return;
        };
        cx.cmd(AppCommand::Layout(LayoutCmd::Restore { win, blob }));
    }

    fn on_layout_changed(&mut self, win: WindowId, cx: &mut IntentCx<'_, DemoSpec>) {
        if !self.mains.contains(&win) {
            return;
        }
        let phase = self.phase.get(&win).copied().unwrap_or(0);
        let leaves = cx
            .view
            .window(win)
            .map(|w| w.dock.leaves.clone())
            .unwrap_or_default();
        match phase {
            0 if leaves.len() == 1 => {
                cx.command(AppCommand::Layout(LayoutCmd::Split {
                    win,
                    leaf: leaves[0].leaf,
                    dir: SplitDir::Right,
                    panel: DemoPanel::Editor,
                }));
                self.phase.insert(win, 1);
            }
            1 if leaves.len() >= 2 => {
                let leaf = leaves
                    .iter()
                    .find(|l| l.panels.iter().any(|p| p.type_id == "editor"))
                    .map(|l| l.leaf)
                    .unwrap_or(leaves[leaves.len() - 1].leaf);
                cx.command(AppCommand::Layout(LayoutCmd::Split {
                    win,
                    leaf,
                    dir: SplitDir::Down,
                    panel: DemoPanel::Inspector,
                }));
                self.phase.insert(win, 2);
            }
            _ => {}
        }
        let phase = self.phase.get(&win).copied().unwrap_or(0);
        if phase == 2 && leaves.len() >= 3 && self.blob_asked.insert(win) {
            cx.command(AppCommand::Layout(LayoutCmd::RequestBlob {
                win,
                ticket: Ticket(1),
            }));
        }
    }
}

impl App for DemoApp {
    type Spec = DemoSpec;

    fn init(&mut self, cx: &mut uzor_framework::InitCx<'_, Self::Spec>) {
        let ctrl_k = KeyboardShortcut::new(
            ModifierKeys {
                ctrl: true,
                ..ModifierKeys::default()
            },
            KeyCode::K,
        );
        cx.command(AppCommand::Keymap(KeymapCmd::Bind {
            scope: KeymapScope::Global,
            binding: Binding::new(ctrl_k, DemoAction::Palette),
        }));
        cx.command(AppCommand::Keymap(KeymapCmd::Bind {
            scope: KeymapScope::Global,
            binding: Binding::new(KeyboardShortcut::key(KeyCode::Escape), DemoAction::Dismiss),
        }));
    }

    fn chrome_model(&self, _win: WindowId) -> ChromeModel {
        ChromeModel {
            tabs: vec![ChromeTab {
                label: "Demo".into(),
                closable: false,
            }],
            ..ChromeModel::default()
        }
    }

    fn overlay_model(&self, _win: WindowId, id: DemoOverlay) -> OverlayModel {
        match id {
            DemoOverlay::Settings => OverlayModel::Modal(ModalModel::titled("Settings")),
            DemoOverlay::Menu => OverlayModel::ContextMenu(ContextMenuModel {
                title: Some("Panel".into()),
                items: vec![
                    ContextMenuEntry::Item {
                        label: "Inspector pinned".into(),
                        disabled: true,
                    },
                    ContextMenuEntry::Separator,
                    ContextMenuEntry::Item {
                        label: "Close".into(),
                        disabled: false,
                    },
                ],
            }),
            DemoOverlay::Palette => OverlayModel::Dropdown(DropdownModel {
                items: vec![
                    "Theme".into(),
                    "Save layout".into(),
                    "Restore layout".into(),
                ],
                selected: None,
            }),
            DemoOverlay::Tooltip => OverlayModel::Tooltip(TooltipModel {
                text: "Drag a header into the gutter to expand".into(),
            }),
        }
    }

    fn panel(&mut self, cx: &mut PanelCx<'_, Self::Spec>) {
        match *cx.panel {
            DemoPanel::Navigator => self.paint_navigator(cx),
            DemoPanel::Editor => {
                let hits = navigator_hits(cx.rect);
                paint_field(cx, id_note(), hits.note, false);
            }
            DemoPanel::Inspector => {
                label(cx, content_body(cx.rect), "Pinned");
            }
        }
    }

    fn overlay_body(&mut self, cx: &mut OverlayCx<'_, Self::Spec>) {
        match cx.id {
            DemoOverlay::Settings => {
                let rect = settings_field(cx.rect);
                paint_field(cx, id_settings_note(), rect, true);
            }
            DemoOverlay::Menu => label(cx, cx.rect, "Inspector is pinned"),
            DemoOverlay::Palette => self.paint_palette(cx),
            DemoOverlay::Tooltip => {
                label(cx, cx.rect, "Drag a header into the gutter to expand");
            }
        }
    }

    fn intent(&mut self, intent: Intent<Self::Spec>, cx: &mut IntentCx<'_, Self::Spec>) {
        match intent {
            Intent::Window(uzor_framework::WindowIntent::Opened { win, key }) => {
                self.keys.insert(win, key.as_str().to_string());
                cx.command(AppCommand::Layout(LayoutCmd::SetChrome {
                    win,
                    visible: true,
                    height: 32.0,
                }));
                let panel = match key.as_str() {
                    "main" => {
                        self.mains.insert(win);
                        self.phase.insert(win, 0);
                        DemoPanel::Navigator
                    }
                    "pin" => DemoPanel::Inspector,
                    _ => DemoPanel::Navigator,
                };
                cx.command(AppCommand::Layout(LayoutCmd::OpenPanel {
                    win,
                    panel,
                    at: DockTarget::Root,
                }));
            }
            Intent::Dock(uzor_framework::DockIntent::LayoutChanged { win, .. }) => {
                self.on_layout_changed(win, cx);
            }
            Intent::Dock(uzor_framework::DockIntent::LayoutBlob { win, blob, .. }) => {
                if let Some(key) = self.keys.get(&win).cloned() {
                    self.office.insert(key, blob);
                }
            }
            Intent::Action(DemoAction::Palette) => {
                if let Some(win) = self.mains.iter().next().copied() {
                    cx.command(open_overlay(
                        win,
                        DemoOverlay::Palette,
                        OverlayKind::Dropdown,
                        None,
                        220.0,
                        140.0,
                    ));
                }
            }
            Intent::Action(DemoAction::Theme) => self.toggle_theme(cx),
            Intent::Action(DemoAction::SaveLayout) => {
                if let Some(win) = self.mains.iter().next().copied() {
                    self.save(cx, win);
                }
            }
            Intent::Action(DemoAction::RestoreLayout) => {
                if let Some(win) = self.mains.iter().next().copied() {
                    self.restore(cx, win);
                }
            }
            Intent::Action(DemoAction::Dismiss) => {
                let wins: Vec<WindowId> = self.keys.keys().copied().collect();
                for win in wins {
                    cx.command(AppCommand::Overlay(OverlayCmd::CloseTop { win }));
                }
            }
            _ => {}
        }
    }
}

impl DemoApp {
    fn paint_navigator(&mut self, cx: &mut PanelCx<'_, DemoSpec>) {
        let hits = navigator_hits(cx.rect);
        let win = cx.window;
        if cx.view.clicked(&id_settings()) {
            cx.out.command(open_overlay(
                win,
                DemoOverlay::Settings,
                OverlayKind::Modal,
                None,
                300.0,
                180.0,
            ));
        }
        if cx.view.clicked(&id_menu()) {
            cx.out.command(open_overlay(
                win,
                DemoOverlay::Menu,
                OverlayKind::ContextMenu,
                Some(hits.menu),
                180.0,
                88.0,
            ));
        }
        if cx.view.clicked(&id_theme()) {
            let mut sink = HookSink(cx.out);
            self.toggle_theme(&mut sink);
        }
        if cx.view.clicked(&id_save()) {
            let mut sink = HookSink(cx.out);
            self.save(&mut sink, win);
        }
        if cx.view.clicked(&id_restore()) {
            let mut sink = HookSink(cx.out);
            self.restore(&mut sink, win);
        }
        if cx.view.is_hovered(&id_tip()) {
            if !cx.view.overlay_open(DemoOverlay::Tooltip) {
                cx.out.command(open_overlay(
                    win,
                    DemoOverlay::Tooltip,
                    OverlayKind::Tooltip,
                    Some(hits.tip),
                    260.0,
                    36.0,
                ));
            }
        } else if cx.view.overlay_open(DemoOverlay::Tooltip) {
            cx.out.command(close_overlay(win, DemoOverlay::Tooltip));
        }
        paint_btn(cx, id_settings(), hits.settings, "Settings");
        paint_btn(cx, id_menu(), hits.menu, "Menu");
        paint_btn(
            cx,
            id_theme(),
            hits.theme,
            if self.dark { "Light" } else { "Dark" },
        );
        paint_btn(cx, id_save(), hits.save, "Save");
        paint_btn(cx, id_restore(), hits.restore, "Restore");
        paint_btn(cx, id_tip(), hits.tip, "Expand");
        let cap = content_body(cx.rect);
        label(
            cx,
            Rect::new(cap.x, cap.y + 160.0, cap.width, 20.0),
            "Ctrl+K palette",
        );
    }

    fn paint_palette(&mut self, cx: &mut OverlayCx<'_, DemoSpec>) {
        let (theme, save, restore) = palette_hits(cx.rect);
        let win = cx.window;
        if cx.view.clicked(&id_palette_theme()) {
            let mut sink = HookSink(cx.out);
            self.toggle_theme(&mut sink);
            sink.cmd(close_overlay(win, DemoOverlay::Palette));
        }
        if cx.view.clicked(&id_palette_save()) {
            let mut sink = HookSink(cx.out);
            self.save(&mut sink, win);
            sink.cmd(close_overlay(win, DemoOverlay::Palette));
        }
        if cx.view.clicked(&id_palette_restore()) {
            let mut sink = HookSink(cx.out);
            self.restore(&mut sink, win);
            sink.cmd(close_overlay(win, DemoOverlay::Palette));
        }
        paint_btn(cx, id_palette_theme(), theme, "Theme");
        paint_btn(cx, id_palette_save(), save, "Save layout");
        paint_btn(cx, id_palette_restore(), restore, "Restore");
    }
}

fn palette_hits(body: Rect) -> (Rect, Rect, Rect) {
    let w = (body.width - 24.0).max(40.0);
    let row = |i: f64| Rect::new(body.x + 12.0, body.y + 8.0 + i * 32.0, w, 28.0);
    (row(0.0), row(1.0), row(2.0))
}

fn id_palette_theme() -> WidgetId {
    WidgetId::from("demo-palette-theme")
}
fn id_palette_save() -> WidgetId {
    WidgetId::from("demo-palette-save")
}
fn id_palette_restore() -> WidgetId {
    WidgetId::from("demo-palette-restore")
}

fn open_overlay(
    win: WindowId,
    id: DemoOverlay,
    kind: OverlayKind,
    anchor: Option<Rect>,
    width: f64,
    height: f64,
) -> AppCommand<DemoSpec> {
    AppCommand::Overlay(OverlayCmd::Open {
        win,
        id,
        kind,
        anchor,
        size: OverlaySize::Fixed { width, height },
        policy: None,
    })
}

fn close_overlay(win: WindowId, id: DemoOverlay) -> AppCommand<DemoSpec> {
    AppCommand::Overlay(OverlayCmd::Close { win, id })
}

trait PaintCx<'a> {
    fn paint_btn(&mut self, id: WidgetId, rect: Rect, label_text: &str);
    fn paint_label(&mut self, rect: Rect, label_text: &str);
}

fn button_settings(tokens: &Tokens) -> ButtonSettings {
    ButtonSettings::default().with_theme(Box::new(TokenTheme::from_tokens(tokens)))
}

fn text_settings(tokens: &Tokens) -> TextSettings {
    TextSettings::default().with_theme(Box::new(TokenTheme::from_tokens(tokens)))
}

impl<'a> PaintCx<'a> for PanelCx<'a, DemoSpec> {
    fn paint_btn(&mut self, id: WidgetId, rect: Rect, label_text: &str) {
        let settings = button_settings(self.tokens);
        let view = ButtonView {
            icon: None,
            text: Some(label_text),
            active: false,
            disabled: false,
            active_border: None,
            hover_chevron: None,
        };
        button(self, id, rect, &view, &settings);
    }
    fn paint_label(&mut self, rect: Rect, label_text: &str) {
        let settings = text_settings(self.tokens);
        let view = TextView {
            text: label_text,
            align: TextAlign::Left,
            baseline: TextBaseline::Middle,
            color: None,
            font: None,
            overflow: TextOverflow::Clip,
            hovered: false,
        };
        text(self, WidgetId::from("demo-caption"), rect, &view, &settings);
    }
}

impl<'a> PaintCx<'a> for OverlayCx<'a, DemoSpec> {
    fn paint_btn(&mut self, id: WidgetId, rect: Rect, label_text: &str) {
        let settings = button_settings(self.tokens);
        let view = ButtonView {
            icon: None,
            text: Some(label_text),
            active: false,
            disabled: false,
            active_border: None,
            hover_chevron: None,
        };
        button(self, id, rect, &view, &settings);
    }
    fn paint_label(&mut self, rect: Rect, label_text: &str) {
        let settings = text_settings(self.tokens);
        let view = TextView {
            text: label_text,
            align: TextAlign::Left,
            baseline: TextBaseline::Middle,
            color: None,
            font: None,
            overflow: TextOverflow::Clip,
            hovered: false,
        };
        text(self, WidgetId::from("demo-caption"), rect, &view, &settings);
    }
}

fn paint_btn<'a, C: PaintCx<'a>>(cx: &mut C, id: WidgetId, rect: Rect, label_text: &str) {
    cx.paint_btn(id, rect, label_text);
}

fn label<'a, C: PaintCx<'a>>(cx: &mut C, rect: Rect, label_text: &str) {
    cx.paint_label(rect, label_text);
}

trait FieldCx<'a> {
    fn win(&self) -> WindowId;
    fn tokens(&self) -> &Tokens;
    fn focused(&self, id: &WidgetId) -> bool;
    fn hovered(&self, id: &WidgetId) -> bool;
    fn layer(&self) -> uzor::LayerId;
    fn command(&mut self, cmd: AppCommand<DemoSpec>);
    fn ime_area(&mut self, rect: Rect);
    fn with_coord<R>(&mut self, f: impl FnOnce(&mut uzor::input::InputCoordinator) -> R) -> R;
    fn with_render<R>(&mut self, f: impl FnOnce(&mut dyn uzor::render::RenderContext) -> R) -> R;
}

impl<'a> FieldCx<'a> for PanelCx<'a, DemoSpec> {
    fn win(&self) -> WindowId {
        self.window
    }
    fn tokens(&self) -> &Tokens {
        self.tokens
    }
    fn focused(&self, id: &WidgetId) -> bool {
        self.view.is_focused(id)
    }
    fn hovered(&self, id: &WidgetId) -> bool {
        self.view.is_hovered(id)
    }
    fn layer(&self) -> uzor::LayerId {
        self.widgets.layer.clone()
    }
    fn command(&mut self, cmd: AppCommand<DemoSpec>) {
        self.out.command(cmd);
    }
    fn ime_area(&mut self, rect: Rect) {
        self.out.ime_area(rect);
    }
    fn with_coord<R>(&mut self, f: impl FnOnce(&mut uzor::input::InputCoordinator) -> R) -> R {
        f(self.widgets.coord)
    }
    fn with_render<R>(&mut self, f: impl FnOnce(&mut dyn uzor::render::RenderContext) -> R) -> R {
        f(self.render)
    }
}

impl<'a> FieldCx<'a> for OverlayCx<'a, DemoSpec> {
    fn win(&self) -> WindowId {
        self.window
    }
    fn tokens(&self) -> &Tokens {
        self.tokens
    }
    fn focused(&self, id: &WidgetId) -> bool {
        self.view.is_focused(id)
    }
    fn hovered(&self, id: &WidgetId) -> bool {
        self.view.is_hovered(id)
    }
    fn layer(&self) -> uzor::LayerId {
        self.widgets.layer.clone()
    }
    fn command(&mut self, cmd: AppCommand<DemoSpec>) {
        self.out.command(cmd);
    }
    fn ime_area(&mut self, rect: Rect) {
        self.out.ime_area(rect);
    }
    fn with_coord<R>(&mut self, f: impl FnOnce(&mut uzor::input::InputCoordinator) -> R) -> R {
        f(self.widgets.coord)
    }
    fn with_render<R>(&mut self, f: impl FnOnce(&mut dyn uzor::render::RenderContext) -> R) -> R {
        f(self.render)
    }
}

fn paint_field<'a, C: FieldCx<'a>>(cx: &mut C, id: WidgetId, rect: Rect, focus_if_needed: bool) {
    if focus_if_needed && !cx.focused(&id) {
        let win = cx.win();
        cx.command(AppCommand::Focus(FocusCmd::Focus { win, id: id.clone() }));
    }
    let layer = cx.layer();
    cx.with_coord(|coord| {
        coord.set_default_layer(Some(layer));
        coord.register_text_field(id.clone(), rect, TextFieldConfig::text());
        coord.set_default_layer(None);
    });
    let (text, preedit, cursor, selection) = cx.with_coord(|coord| match coord.text_fields().field_state(&id) {
        Some(s) => (s.text.clone(), s.preedit.clone(), s.cursor, s.selection_range()),
        None => (String::new(), String::new(), 0, None),
    });
    let shown = splice_preedit(&text, cursor, &preedit);
    let draw_cursor = cursor + preedit.chars().count();
    let focused = cx.focused(&id);
    let hovered = cx.hovered(&id);
    let settings = TextInputSettings::with_config(widget_field_config())
        .with_theme(Box::new(TokenTheme::from_tokens(cx.tokens())));
    let view = InputView {
        text: &shown,
        placeholder: "Note",
        cursor: draw_cursor,
        selection,
        focused,
        disabled: false,
        input_type: InputType::Text,
    };
    let state = if hovered {
        WidgetState::Hovered
    } else {
        WidgetState::Normal
    };
    let positions = cx.with_render(|render| {
        draw_input(render, rect, state, &view, &settings).char_x_positions
    });
    cx.with_coord(|coord| {
        coord.text_fields_mut().update_field(
            &id,
            (rect.x, rect.y, rect.width, rect.height),
            positions,
        );
    });
    if focused {
        cx.ime_area(rect);
    }
}

fn splice_preedit(text: &str, cursor: usize, preedit: &str) -> String {
    if preedit.is_empty() {
        return text.to_string();
    }
    let at = text
        .char_indices()
        .nth(cursor)
        .map(|(b, _)| b)
        .unwrap_or(text.len());
    let mut shown = String::with_capacity(text.len() + preedit.len());
    shown.push_str(&text[..at]);
    shown.push_str(preedit);
    shown.push_str(&text[at..]);
    shown
}

fn widget_field_config() -> uzor::ui::widgets::atomic::text_input::state::TextFieldConfig {
    uzor::ui::widgets::atomic::text_input::state::TextFieldConfig::text()
}
