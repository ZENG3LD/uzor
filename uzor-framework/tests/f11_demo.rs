//! Brief F11: the demo runs headless, and §9.4 scenes match stored PNGs.
//!
//! Goldens are synthetic widget renders (tiny-skia CPU context), same idea
//! as the F8 widget goldens. `UZOR_BLESS=1` writes `tests/goldens/demo/*.png`.
//! Nothing here is a photograph of a desktop.

#[path = "../examples/demo/mod.rs"]
mod demo;

use std::path::PathBuf;

use demo::{
    id_note, id_settings_note, navigator_hits, DemoApp, DemoOverlay, DemoPanel,
};
use tiny_skia::Pixmap;
use uzor::input::{KeyCode, ModifierKeys, MouseButton};
use uzor::layout::docking::DropZone;
use uzor::tokens::{BuiltinSet, TokenTheme, Tokens};
use uzor::ui::widgets::composite::chrome::{
    self, ChromeRenderKind, ChromeSettings, ChromeState, ChromeTabConfig, ChromeView,
    DefaultChromeStyle,
};
use uzor::Rect;
use uzor_framework::{
    AppCommand, DockTarget, FrameRequest, HeadlessHost, InputEvent, KeyInput, KeyState, LayoutCmd,
    PointerInput, SizePx, WindowSpec,
};
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

fn boot(extra_pin_window: bool) -> (HeadlessHost<demo::DemoSpec, DemoApp>, uzor_framework::Handle<demo::DemoSpec>) {
    let mut cfg = DemoApp::runtime_config();
    if extra_pin_window {
        cfg.windows
            .push(WindowSpec::new("pin", "Pinned", SizePx::new(280, 180)));
    }
    let (mut host, handle) = HeadlessHost::new(DemoApp::new(), cfg);
    for _ in 0..8 {
        host.advance(0.016);
    }
    (host, handle)
}

fn window_id(host: &HeadlessHost<demo::DemoSpec, DemoApp>, key: &str) -> uzor_framework::WindowId {
    host.snapshot()
        .windows
        .iter()
        .find(|w| w.key.as_str() == key)
        .unwrap_or_else(|| panic!("window {key} missing"))
        .id
}

fn leaf_rect(host: &HeadlessHost<demo::DemoSpec, DemoApp>, key: &str, type_id: &str) -> Rect {
    let snap = host.snapshot();
    let win = snap
        .windows
        .iter()
        .find(|w| w.key.as_str() == key)
        .unwrap_or_else(|| panic!("window {key}"));
    win.dock
        .leaves
        .iter()
        .find(|l| l.panels.iter().any(|p| p.type_id == type_id))
        .unwrap_or_else(|| panic!("leaf {type_id} in {key}"))
        .rect
}

fn paint(host: &mut HeadlessHost<demo::DemoSpec, DemoApp>, key: &str) -> TinySkiaCpuRenderContext {
    let snap = host.snapshot();
    let win = snap
        .windows
        .iter()
        .find(|w| w.key.as_str() == key)
        .unwrap_or_else(|| panic!("window {key}"));
    let w = win.geometry.viewport.width.round().max(1.0) as u32;
    let h = win.geometry.viewport.height.round().max(1.0) as u32;
    let id = win.id;
    drop(snap);
    let mut ctx = TinySkiaCpuRenderContext::new(w, h, 1.0);
    let tokens = Tokens::builtin(if host.runtime().app().is_dark() {
        BuiltinSet::Dark
    } else {
        BuiltinSet::Light
    });
    let [r, g, b, a] = tokens.semantic.surface_panel.to_rgba8();
    ctx.clear(tiny_skia::Color::from_rgba8(r, g, b, a));
    let req = FrameRequest {
        window: id,
        size: SizePx::new(w, h),
        dpr: 1.0,
        background: 0,
        regions: Default::default(),
        invalidations: Default::default(),
    };
    host.runtime_mut().paint(&req, &mut ctx);
    ctx
}

fn assert_pixmap(name: &str, width: u32, height: u32, pixels: &[u8]) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/goldens/demo")
        .join(format!("{name}.png"));
    if std::env::var("UZOR_BLESS").ok().as_deref() == Some("1") {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).unwrap();
        }
        let pm = Pixmap::from_vec(pixels.to_vec(), tiny_skia::IntSize::from_wh(width, height).unwrap())
            .unwrap_or_else(|| panic!("{name}: pixmap"));
        pm.save_png(&path).unwrap();
        return;
    }
    let golden = Pixmap::load_png(&path).unwrap_or_else(|err| panic!("load {}: {err}", path.display()));
    assert_eq!(golden.width(), width, "{name} width");
    assert_eq!(golden.height(), height, "{name} height");
    if golden.data() != pixels {
        let differ = golden.data().iter().zip(pixels).filter(|(a, b)| a != b).count();
        panic!("{name}: {differ} bytes differ from {}", path.display());
    }
}

fn assert_ctx(name: &str, ctx: &TinySkiaCpuRenderContext) {
    assert_pixmap(name, ctx.width(), ctx.height(), ctx.pixels());
}

fn pointer(
    host: &mut HeadlessHost<demo::DemoSpec, DemoApp>,
    win: uzor_framework::WindowId,
    ev: PointerInput,
) {
    host.input(win, InputEvent::Pointer(ev));
}

fn click(host: &mut HeadlessHost<demo::DemoSpec, DemoApp>, win: uzor_framework::WindowId, rect: Rect) {
    let pos = uzor_framework::Point::new(rect.x + rect.width * 0.5, rect.y + rect.height * 0.5);
    pointer(
        host,
        win,
        PointerInput::Down {
            pos,
            button: MouseButton::Left,
            mods: ModifierKeys::default(),
        },
    );
    host.advance(0.016);
    pointer(
        host,
        win,
        PointerInput::Up {
            pos,
            button: MouseButton::Left,
            mods: ModifierKeys::default(),
        },
    );
    host.advance(0.016);
    host.advance(0.016);
}

#[test]
fn demo_settles_two_windows_three_panels_and_a_blob() {
    let (host, _) = boot(false);
    let snap = host.snapshot();
    assert_eq!(snap.windows.len(), 2, "native-caps host opens both windows");
    let main = snap.windows.iter().find(|w| w.key.as_str() == "main").unwrap();
    let kinds: Vec<&str> = main
        .dock
        .leaves
        .iter()
        .filter_map(|l| l.panels.first().map(|p| p.type_id))
        .collect();
    assert!(kinds.contains(&"nav"), "{kinds:?}");
    assert!(kinds.contains(&"editor"), "{kinds:?}");
    assert!(kinds.contains(&"inspector"), "{kinds:?}");
    assert!(main.dock.separators.len() >= 2, "splitters: {}", main.dock.separators.len());
    assert!(
        host.runtime().app().office().contains_key("main"),
        "in-memory back office stored a layout blob"
    );
    assert!(main.chrome.visible, "chrome strip is on");
}

#[test]
fn ctrl_k_opens_palette_and_esc_closes_it() {
    let (mut host, _) = boot(false);
    let win = window_id(&host, "main");
    host.input(
        win,
        InputEvent::Key(KeyInput {
            code: KeyCode::K,
            text: None,
            state: KeyState::Down,
            mods: ModifierKeys {
                ctrl: true,
                ..ModifierKeys::default()
            },
        }),
    );
    host.advance(0.016);
    host.advance(0.016);
    let open = host
        .snapshot()
        .window(win)
        .unwrap()
        .overlays
        .iter()
        .any(|o| o.id == DemoOverlay::Palette);
    assert!(open, "Ctrl+K opens the palette");
    host.input(
        win,
        InputEvent::Key(KeyInput {
            code: KeyCode::Escape,
            text: None,
            state: KeyState::Down,
            mods: ModifierKeys::default(),
        }),
    );
    host.advance(0.016);
    host.advance(0.016);
    let still = host
        .snapshot()
        .window(win)
        .unwrap()
        .overlays
        .iter()
        .any(|o| o.id == DemoOverlay::Palette);
    assert!(!still, "Esc dismisses the palette");
}

#[test]
fn golden_initial_layout_and_tokens() {
    let (mut host, _) = boot(false);
    assert_ctx("initial_layout", &paint(&mut host, "main"));
    let win = window_id(&host, "main");
    let nav = leaf_rect(&host, "main", "nav");
    let rev = host.snapshot().theme_rev;
    click(&mut host, win, navigator_hits(nav).theme);
    // Click is read on a later compose; the theme command applies the tick after that.
    for _ in 0..3 {
        host.advance(0.016);
    }
    assert!(
        host.snapshot().theme_rev > rev,
        "theme button must switch tokens"
    );
    assert!(!host.runtime().app().is_dark());
    assert!(!host.runtime().app().office().is_empty());
    assert_ctx("tokens_light", &paint(&mut host, "main"));
}

#[test]
fn golden_modal_focus_ring() {
    let (mut host, _) = boot(false);
    let win = window_id(&host, "main");
    let nav = leaf_rect(&host, "main", "nav");
    click(&mut host, win, navigator_hits(nav).settings);
    // Click is read a frame late, the open command applies on the next
    // tick, the body then asks for focus, and that applies one tick later.
    for _ in 0..8 {
        host.advance(0.016);
    }
    let focused = host.snapshot().window(win).unwrap().input.focused.clone();
    assert_eq!(focused.as_ref(), Some(&id_settings_note()), "{focused:?}");
    assert!(host
        .snapshot()
        .window(win)
        .unwrap()
        .overlays
        .iter()
        .any(|o| o.id == DemoOverlay::Settings && o.modal));
    assert_ctx("modal_focus_ring", &paint(&mut host, "main"));
}

#[test]
fn golden_splitter_mid_drag() {
    let (mut host, _) = boot(false);
    let win = window_id(&host, "main");
    let before = host.snapshot().window(win).unwrap().dock.separators[0].rect;
    let pos = uzor_framework::Point::new(before.x + before.width * 0.5, before.y + 20.0);
    pointer(
        &mut host,
        win,
        PointerInput::Down {
            pos,
            button: MouseButton::Left,
            mods: ModifierKeys::default(),
        },
    );
    host.advance(0.016);
    pointer(
        &mut host,
        win,
        PointerInput::Moved {
            pos: uzor_framework::Point::new(pos.x + 36.0, pos.y),
            mods: ModifierKeys::default(),
        },
    );
    host.advance(0.016);
    let after = host.snapshot().window(win).unwrap().dock.separators[0].rect;
    assert_ne!(before, after, "splitter did not move");
    assert_ctx("splitter_mid_drag", &paint(&mut host, "main"));
}

#[test]
fn golden_drag_ghost() {
    let (mut host, _) = boot(false);
    let win = window_id(&host, "main");
    let nav = leaf_rect(&host, "main", "nav");
    let editor = leaf_rect(&host, "main", "editor");
    let pos = uzor_framework::Point::new(nav.x + 24.0, nav.y + 8.0);
    pointer(
        &mut host,
        win,
        PointerInput::Down {
            pos,
            button: MouseButton::Left,
            mods: ModifierKeys::default(),
        },
    );
    host.advance(0.016);
    pointer(
        &mut host,
        win,
        PointerInput::Moved {
            pos: uzor_framework::Point::new(
                editor.x + editor.width * 0.5,
                editor.y + editor.height * 0.5,
            ),
            mods: ModifierKeys::default(),
        },
    );
    host.advance(0.016);
    assert!(
        host.snapshot().window(win).unwrap().input.captured,
        "panel drag should capture the pointer"
    );
    assert_ctx("drag_ghost", &paint(&mut host, "main"));
}

#[test]
fn golden_tabs_and_pin() {
    let (mut host, handle) = boot(true);
    assert_ctx("tabs_1", &paint(&mut host, "side"));
    assert_ctx("pin_user", &paint(&mut host, "pin"));
    let side = window_id(&host, "side");
    let leaf = host.snapshot().window(side).unwrap().dock.leaves[0].leaf;
    let mut stack = vec![DemoPanel::Editor];
    stack.extend([
        DemoPanel::Navigator,
        DemoPanel::Editor,
        DemoPanel::Navigator,
        DemoPanel::Editor,
    ]);
    for (n, panel) in stack.into_iter().enumerate() {
        handle
            .dispatch(AppCommand::Layout(LayoutCmd::OpenPanel {
                win: side,
                panel,
                at: DockTarget::Leaf(leaf, DropZone::Center),
            }))
            .unwrap_or_else(|e| panic!("dispatch: {e}"));
        host.advance(0.016);
        host.advance(0.016);
        let tabs = host.snapshot().window(side).unwrap().dock.leaves[0]
            .panels
            .len();
        if n == 0 {
            assert!(tabs >= 2, "two tabs, got {tabs}");
            assert_ctx("tabs_2", &paint(&mut host, "side"));
        }
    }
    let tabs = host.snapshot().window(side).unwrap().dock.leaves[0].panels.len();
    assert!(tabs >= 6, "many tabs, got {tabs}");
    assert_ctx("tabs_many", &paint(&mut host, "side"));
}

#[test]
fn editor_field_takes_a_character() {
    let (mut host, _) = boot(false);
    let win = window_id(&host, "main");
    let editor = leaf_rect(&host, "main", "editor");
    click(&mut host, win, navigator_hits(editor).note);
    host.input(
        win,
        InputEvent::Key(KeyInput {
            code: KeyCode::A,
            text: Some("a".into()),
            state: KeyState::Down,
            mods: ModifierKeys::default(),
        }),
    );
    host.advance(0.016);
    let focused = host.snapshot().window(win).unwrap().input.focused.clone();
    assert_eq!(focused.as_ref(), Some(&id_note()), "{focused:?}");
}

fn chrome_strip(set: BuiltinSet, hovered_tab: Option<usize>, close_hovered: bool) -> TinySkiaCpuRenderContext {
    const TAB_IDS: &[&str] = &["demo", "editor", "inspector"];
    let tabs = [
        ChromeTabConfig {
            id: "demo",
            label: "Demo",
            icon: None,
            color_tag: None,
            closable: false,
            active: false,
        },
        ChromeTabConfig {
            id: "editor",
            label: "Editor",
            icon: None,
            color_tag: None,
            closable: true,
            active: false,
        },
        ChromeTabConfig {
            id: "inspector",
            label: "Inspector",
            icon: None,
            color_tag: None,
            closable: true,
            active: false,
        },
    ];
    let view = ChromeView {
        tabs: &tabs,
        active_tab_id: Some("demo"),
        show_new_tab_btn: false,
        show_menu_btn: true,
        show_new_window_btn: false,
        show_close_window_btn: true,
        is_maximized: false,
        menu_left: false,
        show_maximize: true,
        cursor_x: 0.0,
        cursor_y: 0.0,
        time_ms: 0.0,
    };
    let settings = ChromeSettings {
        theme: Box::new(TokenTheme::new(Tokens::builtin(set))),
        style: Box::<DefaultChromeStyle>::default(),
    };
    let kind = ChromeRenderKind::Default;
    let mut state = ChromeState::new();
    state.sync_tabs(TAB_IDS);
    if let Some(i) = hovered_tab {
        if let Some(ts) = state.tabs_state.get_mut(i) {
            ts.hovered = true;
        }
    }
    if close_hovered {
        state.hovered = chrome::ChromeHit::CloseBtn;
    }
    let (w, h) = chrome::measure(&view, &state, &settings, &kind);
    let mut ctx = TinySkiaCpuRenderContext::new(w.ceil().max(1.0) as u32, h.ceil().max(1.0) as u32, 1.0);
    let tokens = Tokens::builtin(set);
    let [r, g, b, a] = tokens.semantic.surface_app_chrome.to_rgba8();
    ctx.clear(tiny_skia::Color::from_rgba8(r, g, b, a));
    uzor::ui::widgets::composite::chrome::draw_chrome(
        &mut ctx,
        Rect::new(0.0, 0.0, w, h),
        &state,
        &view,
        &settings,
        &kind,
    );
    ctx
}

#[test]
fn golden_chrome_button_states() {
    use tiny_skia::{PixmapPaint, Transform};
    for (set, file) in [
        (BuiltinSet::Dark, "chrome_buttons_dark"),
        (BuiltinSet::Light, "chrome_buttons_light"),
    ] {
        let a = chrome_strip(set, None, false);
        let b = chrome_strip(set, Some(1), false);
        let c = chrome_strip(set, None, true);
        let w = a.width().max(b.width()).max(c.width());
        let h = a.height() + b.height() + c.height();
        let mut out = Pixmap::new(w, h).unwrap();
        let paint = PixmapPaint::default();
        out.draw_pixmap(0, 0, a.pixmap().as_ref(), &paint, Transform::identity(), None);
        out.draw_pixmap(0, a.height() as i32, b.pixmap().as_ref(), &paint, Transform::identity(), None);
        out.draw_pixmap(
            0,
            (a.height() + b.height()) as i32,
            c.pixmap().as_ref(),
            &paint,
            Transform::identity(),
            None,
        );
        assert_pixmap(file, out.width(), out.height(), out.data());
    }
}
