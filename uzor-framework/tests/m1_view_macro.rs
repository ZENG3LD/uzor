//! M1: `view!` expands against `uzor_framework::{widgets::lm, flex}`.

use uzor::app_context::StateRegistry;
use uzor::input::{InputCoordinator, LayerId};
use uzor::layout::docking::{DockPanel, LeafId};
use uzor::testing::NullRenderContext;
use uzor::tokens::{BuiltinSet, Tokens};
use uzor::{Rect, WidgetId};

use uzor_framework::handle::{FrameTime, HookOps, PanelCx, VisualView, Widgets};
use uzor_framework::types::ids::WindowId;
use uzor_framework::types::spec::{PanelHome, Spec};
use uzor_framework::view;

#[derive(Clone, Debug)]
struct P(&'static str);

impl DockPanel for P {
    fn title(&self) -> &str {
        self.0
    }
    fn type_id(&self) -> &'static str {
        "m1"
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
        Some(P("m1"))
    }
}

fn compose(body: impl FnOnce(&mut PanelCx<'_, Demo>)) {
    let tokens = Tokens::builtin(BuiltinSet::Dark);
    let mut coord = InputCoordinator::new();
    let mut states = StateRegistry::new();
    let panel = P("m1");
    let mut hooks = HookOps::<Demo>::default();
    let visual = VisualView::<Demo>::default();
    let mut render = NullRenderContext;
    let mut cx = PanelCx {
        window: WindowId(1),
        leaf: LeafId(0),
        panel: &panel,
        index: 0,
        rect: Rect::new(0.0, 0.0, 320.0, 240.0),
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
    body(&mut cx);
    let _ = (Ov::None, Act::None);
}

#[test]
fn view_macro_col_of_atoms_builds() {
    compose(|cx| {
        let body = Rect::new(0.0, 0.0, 200.0, 160.0);
        let mut dark = true;
        let mut power = false;
        let mut clicks = 0_u32;
        view! {
            <col rect={body} gap=4 pad=2>
                <button text="Save" size=36.0 bind_count={&mut clicks} />
                <text text="Hello" size=24.0 />
                <checkbox bind={&mut dark} label="Dark" size=28.0 />
                <toggle bind={&mut power} label="Power" size=28.0 />
                <separator size=2.0 />
            </col>
        }
        let _ = WidgetId::from("v::col[0]::button[0]");
        assert!(dark);
        assert!(!power);
        assert_eq!(clicks, 0);
    });
}

#[test]
fn view_macro_row_and_chrome_build() {
    compose(|cx| {
        let body = Rect::new(0.0, 0.0, 400.0, 48.0);
        view! {
            <row rect={body} gap=0>
                <chrome show_new_window=true size=48.0 />
            </row>
        }
    });
}

#[test]
fn view_macro_nested_flex_builds() {
    compose(|cx| {
        let body = Rect::new(10.0, 10.0, 300.0, 200.0);
        view! {
            <col rect={body} gap=8 pad=4>
                <row gap=4 size=40.0>
                    <button text="A" />
                    <button text="B" />
                </row>
                <text text="footer" size=24.0 />
            </col>
        }
    });
}
