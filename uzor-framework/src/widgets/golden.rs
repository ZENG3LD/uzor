//! Compose the demo widgets into a tiny-skia CPU context and compare the
//! pixels to the committed widget goldens (brief F8). Those PNGs live in
//! `uzor-render-tiny-skia`; this crate does not store images.

use std::path::PathBuf;

use tiny_skia::{Color, Pixmap};
use uzor::app_context::StateRegistry;
use uzor::input::{InputCoordinator, LayerId};
use uzor::layout::docking::{DockPanel, LeafId};
use uzor::render::{TextAlign, TextBaseline};
use uzor::testing::NullRenderContext;
use uzor::tokens::{BuiltinSet, TokenTheme, Tokens};
use uzor::ui::widgets::atomic::button::{ButtonSettings, ButtonView};
use uzor::ui::widgets::atomic::checkbox::{CheckboxRenderKind, CheckboxSettings, CheckboxView};
use uzor::ui::widgets::atomic::chevron::{ChevronSettings, ChevronView, DefaultChevronStyle};
use uzor::ui::widgets::atomic::separator::{
    DefaultSeparatorStyle, SeparatorKind, SeparatorSettings, SeparatorType, SeparatorView,
};
use uzor::ui::widgets::atomic::text::{TextOverflow, TextSettings, TextView};
use uzor::ui::widgets::atomic::toggle::{ToggleRenderKind, ToggleSettings, ToggleView};
use uzor::ui::widgets::atomic::tooltip::{DefaultTooltipStyle, TooltipConfig, TooltipSettings};
use uzor::ui::widgets::composite::chrome::{
    self, ChromeRenderKind, ChromeSettings, ChromeState, ChromeTabConfig, ChromeView,
    DefaultChromeStyle,
};
use uzor::{Rect, WidgetId};
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

use crate::handle::{FrameTime, HookOps, PanelCx, VisualView, Widgets};
use crate::types::ids::WindowId;
use crate::types::spec::{PanelHome, Spec};

use super::lm;

#[derive(Clone, Debug)]
struct P(&'static str);

impl DockPanel for P {
    fn title(&self) -> &str {
        self.0
    }
    fn type_id(&self) -> &'static str {
        "demo"
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Ov {
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Act {
    None,
}

struct Demo;

impl Spec for Demo {
    type Panel = P;
    type Overlay = Ov;
    type Action = Act;
    fn decode_panel(_home: PanelHome, _type_id: &str) -> Option<P> {
        Some(P("demo"))
    }
}

fn _spec_tags() -> (Ov, Act) {
    (Ov::None, Act::None)
}

fn set_name(set: BuiltinSet) -> &'static str {
    match set {
        BuiltinSet::Dark => "dark",
        BuiltinSet::Light => "light",
        BuiltinSet::HighContrast => "high_contrast",
        BuiltinSet::HighContrastMono => "high_contrast_mono",
    }
}

fn canvas(width: u32, height: u32, set: BuiltinSet) -> TinySkiaCpuRenderContext {
    let tokens = Tokens::builtin(set);
    let mut ctx = TinySkiaCpuRenderContext::new(width, height, 1.0);
    let [r, g, b, a] = tokens.semantic.surface_app_chrome.to_rgba8();
    ctx.clear(Color::from_rgba8(r, g, b, a));
    ctx
}

fn assert_golden(name: &str, ctx: &TinySkiaCpuRenderContext) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../uzor-render-tiny-skia/tests/goldens")
        .join(format!("{name}.png"));
    let golden =
        Pixmap::load_png(&path).unwrap_or_else(|err| panic!("load {}: {err}", path.display()));
    assert_eq!(golden.width(), ctx.width(), "{name} width");
    assert_eq!(golden.height(), ctx.height(), "{name} height");
    let actual = ctx.pixels();
    let expected = golden.data();
    if expected != actual {
        let differ = expected.iter().zip(actual).filter(|(a, b)| a != b).count();
        panic!("{name}: {differ} bytes differ from {}", path.display());
    }
}

fn compose(
    set: BuiltinSet,
    visual: &VisualView<Demo>,
    ctx: &mut TinySkiaCpuRenderContext,
    body: impl FnOnce(&mut PanelCx<'_, Demo>),
) {
    let tokens = Tokens::builtin(set);
    let mut coord = InputCoordinator::new();
    let mut states = StateRegistry::new();
    let panel = P("demo");
    let mut hooks = HookOps::<Demo>::default();
    let mut cx = PanelCx {
        window: WindowId(1),
        leaf: LeafId(0),
        panel: &panel,
        index: 0,
        rect: Rect::new(0.0, 0.0, ctx.width() as f64, ctx.height() as f64),
        dpr: 1.0,
        tokens: tokens.as_ref(),
        render: ctx,
        widgets: Widgets {
            coord: &mut coord,
            states: &mut states,
            layer: LayerId::main(),
        },
        view: visual,
        time: FrameTime::default(),
        out: &mut hooks,
    };
    body(&mut cx);
    let _ = _spec_tags();
}

fn theme(set: BuiltinSet) -> TokenTheme {
    TokenTheme::new(Tokens::builtin(set))
}

#[test]
fn button_grid_matches_golden() {
    const LABEL: &str = "Save";
    let rect = Rect::new(20.0, 20.0, 120.0, 36.0);
    let id = WidgetId::from("demo-button");
    let states: &[(&str, bool, bool, bool)] = &[
        ("default", false, false, false),
        ("hover", true, false, false),
        ("pressed", false, true, false),
        ("disabled", false, false, true),
    ];
    for &set in BuiltinSet::ALL {
        for &(name, hovered, pressed, disabled) in states {
            let mut visual = VisualView::<Demo>::default();
            if hovered {
                visual.hovered = Some(id.clone());
            }
            if pressed {
                visual.pressed = Some(id.clone());
            }
            let mut ctx = canvas(160, 76, set);
            let settings = ButtonSettings::default().with_theme(Box::new(theme(set)));
            let view = ButtonView {
                icon: None,
                text: Some(LABEL),
                active: false,
                disabled,
                active_border: None,
                hover_chevron: None,
            };
            compose(set, &visual, &mut ctx, |cx| {
                super::button(cx, id.clone(), rect, &view, &settings);
            });
            assert_golden(&format!("button/{}/{name}", set_name(set)), &ctx);
        }
    }
}

#[test]
fn text_grid_matches_golden() {
    let rect = Rect::new(4.0, 4.0, 112.0, 24.0);
    let id = WidgetId::from("demo-text");
    for &set in BuiltinSet::ALL {
        for &(name, hovered) in &[("normal", false), ("hover", true)] {
            let mut ctx = canvas(120, 32, set);
            let settings = TextSettings::default().with_theme(Box::new(theme(set)));
            let view = TextView {
                text: "Sample label",
                align: TextAlign::Left,
                baseline: TextBaseline::Middle,
                color: None,
                font: None,
                overflow: TextOverflow::Clip,
                hovered,
            };
            let visual = VisualView::<Demo>::default();
            compose(set, &visual, &mut ctx, |cx| {
                super::text(cx, id.clone(), rect, &view, &settings);
            });
            assert_golden(&format!("text/{}/{name}", set_name(set)), &ctx);
        }
    }
}

#[test]
fn checkbox_grid_matches_golden() {
    let rect = Rect::new(20.0, 20.0, 16.0, 16.0);
    let id = WidgetId::from("demo-checkbox");
    let visual = VisualView::<Demo>::default();
    for &set in BuiltinSet::ALL {
        for &(name, checked) in &[("unchecked", false), ("checked", true)] {
            let mut ctx = canvas(160, 56, set);
            let settings = CheckboxSettings::default().with_theme(Box::new(theme(set)));
            let view = CheckboxView {
                checked,
                label: Some("Label"),
            };
            compose(set, &visual, &mut ctx, |cx| {
                super::checkbox(
                    cx,
                    id.clone(),
                    rect,
                    &view,
                    &settings,
                    &CheckboxRenderKind::Standard,
                    "13px sans-serif",
                );
            });
            assert_golden(&format!("checkbox/{}/{name}", set_name(set)), &ctx);
        }
    }
}

#[test]
fn toggle_grid_matches_golden() {
    let rect = Rect::new(8.0, 9.0, 44.0, 22.0);
    let id = WidgetId::from("demo-toggle");
    let visual = VisualView::<Demo>::default();
    let states: &[(&str, bool, bool)] = &[
        ("off", false, false),
        ("on", true, false),
        ("disabled", false, true),
    ];
    for &set in BuiltinSet::ALL {
        for &(name, toggled, disabled) in states {
            let mut ctx = canvas(160, 40, set);
            let settings = ToggleSettings {
                theme: Box::new(theme(set)),
                ..ToggleSettings::default()
            };
            let view = ToggleView {
                toggled,
                label: Some("Enable"),
                disabled,
            };
            compose(set, &visual, &mut ctx, |cx| {
                super::toggle(
                    cx,
                    id.clone(),
                    rect,
                    &view,
                    &settings,
                    &ToggleRenderKind::Switch,
                );
            });
            assert_golden(&format!("toggle/{}/{name}", set_name(set)), &ctx);
        }
    }
}

#[test]
fn separator_grid_matches_golden() {
    let rect = Rect::new(4.0, 4.0, 120.0, 16.0);
    let id = WidgetId::from("demo-separator");
    let visual = VisualView::<Demo>::default();
    for &set in BuiltinSet::ALL {
        for &(name, handle, hovered, dragging) in &[
            ("line", false, false, false),
            ("handle_hover", true, true, false),
            ("handle_active", true, false, true),
        ] {
            let mut ctx = canvas(128, 24, set);
            let settings = SeparatorSettings {
                theme: Box::new(theme(set)),
                style: Box::new(DefaultSeparatorStyle),
            };
            let view = SeparatorView {
                kind: if handle {
                    SeparatorType::horizontal_resize()
                } else {
                    SeparatorType::horizontal_divider()
                },
                hovered,
                dragging,
            };
            let kind = if handle {
                SeparatorKind::ResizeHandle
            } else {
                SeparatorKind::Divider
            };
            compose(set, &visual, &mut ctx, |cx| {
                super::separator(cx, id.clone(), rect, kind, &view, &settings);
            });
            assert_golden(&format!("separator/{}/{name}", set_name(set)), &ctx);
        }
    }
}

#[test]
fn chevron_grid_matches_golden() {
    let rect = Rect::new(16.0, 16.0, 32.0, 32.0);
    let id = WidgetId::from("demo-chevron");
    let visual = VisualView::<Demo>::default();
    let states: &[(&str, bool, bool, bool)] = &[
        ("normal", false, false, false),
        ("hover", true, false, false),
        ("pressed", true, true, false),
        ("disabled", false, false, true),
    ];
    for &set in BuiltinSet::ALL {
        for &(name, hovered, pressed, disabled) in states {
            let mut ctx = canvas(64, 64, set);
            let settings = ChevronSettings {
                theme: Box::new(theme(set)),
                style: Box::new(DefaultChevronStyle),
            };
            let view = ChevronView {
                hovered,
                pressed,
                disabled,
                ..ChevronView::default()
            };
            compose(set, &visual, &mut ctx, |cx| {
                super::chevron(cx, id.clone(), rect, &view, &settings);
            });
            assert_golden(&format!("chevron/{}/{name}", set_name(set)), &ctx);
        }
    }
}

#[test]
fn tooltip_grid_matches_golden() {
    let rect = Rect::new(8.0, 8.0, 120.0, 24.0);
    let id = WidgetId::from("demo-tooltip");
    let visual = VisualView::<Demo>::default();
    let anchor = Rect::new(0.0, 0.0, 0.0, 0.0);
    for &set in BuiltinSet::ALL {
        let mut ctx = canvas(136, 40, set);
        let settings = TooltipSettings {
            theme: Box::new(theme(set)),
            style: Box::new(DefaultTooltipStyle),
        };
        let config = TooltipConfig::below("Tooltip text", anchor);
        compose(set, &visual, &mut ctx, |cx| {
            super::tooltip(cx, id.clone(), rect, &config, 1.0, &settings);
        });
        assert_golden(&format!("tooltip/{}/default", set_name(set)), &ctx);
    }
}

#[test]
fn chrome_grid_matches_golden() {
    const TAB_IDS: &[&str] = &["overview", "chart", "trades"];
    let visual = VisualView::<Demo>::default();
    let states: &[(&str, Option<usize>, bool)] = &[
        ("normal", None, false),
        ("tab_hovered", Some(1), false),
        ("close_hovered", None, true),
    ];
    for &set in BuiltinSet::ALL {
        for &(name, hovered_tab, close_hovered) in states {
            let tabs = vec![
                ChromeTabConfig {
                    id: "overview",
                    label: "Overview",
                    icon: None,
                    color_tag: None,
                    closable: false,
                    active: false,
                },
                ChromeTabConfig {
                    id: "chart",
                    label: "Chart",
                    icon: None,
                    color_tag: None,
                    closable: false,
                    active: false,
                },
                ChromeTabConfig {
                    id: "trades",
                    label: "Trades",
                    icon: None,
                    color_tag: None,
                    closable: false,
                    active: false,
                },
            ];
            let view = ChromeView {
                tabs: &tabs,
                active_tab_id: Some("overview"),
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
            };
            let settings = ChromeSettings {
                theme: Box::new(theme(set)),
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
            let mut ctx = canvas(w.ceil() as u32, h.ceil() as u32, set);
            let rect = Rect::new(0.0, 0.0, w, h);
            compose(set, &visual, &mut ctx, |cx| {
                super::chrome(cx, "demo-chrome", rect, &state, &view, &settings, &kind);
            });
            assert_golden(&format!("chrome/{}/{name}", set_name(set)), &ctx);
        }
    }
}

#[test]
fn button_builder_bind_count_sees_last_frame_click() {
    let id = WidgetId::from("demo-button");
    let rect = Rect::new(0.0, 0.0, 40.0, 20.0);
    let mut visual = VisualView::<Demo>::default();
    visual.clicked = vec![(id.clone(), 1)];
    let tokens = Tokens::builtin(BuiltinSet::Dark);
    let mut coord = InputCoordinator::new();
    let mut states = StateRegistry::new();
    let panel = P("demo");
    let mut hooks = HookOps::<Demo>::default();
    let mut render = NullRenderContext;
    let mut n = 0u32;
    let mut cx = PanelCx {
        window: WindowId(1),
        leaf: LeafId(0),
        panel: &panel,
        index: 0,
        rect,
        dpr: 1.0,
        tokens: tokens.as_ref(),
        render: &mut render,
        widgets: Widgets {
            coord: &mut coord,
            states: &mut states,
            layer: LayerId::main(),
        },
        view: &visual,
        time: FrameTime::default(),
        out: &mut hooks,
    };
    lm::button(id, rect)
        .text("Save")
        .bind_count(&mut n)
        .build(&mut cx);
    assert_eq!(n, 1);
}
