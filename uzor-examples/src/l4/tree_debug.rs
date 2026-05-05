//! # L4 — Layout-tree debug visualiser
//!
//! Spawns two windows.  Each window draws the *live* state of the
//! [`LayoutManager`] as a tree, with a coloured pill on every row
//! showing that node's [`SyncMode`]:
//!
//! - **green** — Synced (single instance on the LM root)
//! - **amber** — Sometimes(None)        — opt-in shareable, currently alone
//! - **blue**  — Sometimes(Some(group)) — opt-in shareable, in a group
//! - **red**   — Standalone             — never shareable
//!
//! Both windows render the same data, so the two visualisers reading
//! the same `LayoutManager` are themselves the proof that branches are
//! isolated: only the *current* window's `pointer_state` row updates as
//! the cursor moves.
//!
//! ```sh
//! cargo run -p uzor-examples --bin l4-tree-debug
//! ```

use uzor::framework::app::{App, NoPanel};
use uzor::framework::builder::AppBuilder;
use uzor::framework::multi_window::{WindowCtx, WindowKey, WindowSpec};
use uzor::layout::sync::{node_ids, SyncMode};
use uzor::layout::LayoutManager;
use uzor_desktop::AppRun as _;

// ── App ──────────────────────────────────────────────────────────────────────

struct TreeDebugApp;

impl App<NoPanel> for TreeDebugApp {
    fn ui(&mut self, win: &mut WindowCtx<'_, NoPanel>) {
        let rect   = win.rect;
        let render = &mut *win.render;
        let layout = &mut *win.layout;

        // Background
        let bg = layout.styles().color_or_owned("surface", "#16171D");
        render.set_fill_color(bg.as_str());
        render.fill_rect(rect.x, rect.y, rect.width, rect.height);

        // Title
        render.set_fill_color("#E6E6EA");
        render.set_font("bold 16px sans-serif");
        render.fill_text("LayoutManager — live tree", rect.x + 12.0, rect.y + 24.0);

        let mut cy = rect.y + 48.0;
        let line_h = 18.0_f64;
        let indent = 16.0_f64;

        // ── synced root ─────────────────────────────────────────────────
        draw_label(render, rect.x + 12.0, &mut cy, line_h, "synced root", "#9AA0AC");
        for id in [
            node_ids::STYLES,
            node_ids::Z_LAYERS,
            node_ids::FRAME_TIME,
            node_ids::PANEL_TYPES,
        ] {
            let mode = layout.sync_registry().get(id);
            draw_node(render, rect.x + 12.0 + indent, &mut cy, line_h, id, mode, None);
        }

        cy += 8.0;

        // ── windows ─────────────────────────────────────────────────────
        let keys: Vec<WindowKey> = layout.window_keys().cloned().collect();
        let current = layout.current_window().cloned();

        draw_label(
            render,
            rect.x + 12.0,
            &mut cy,
            line_h,
            &format!("windows ({})", keys.len()),
            "#9AA0AC",
        );

        for key in &keys {
            let prefix = if Some(key) == current.as_ref() { "▶ " } else { "  " };
            draw_label(
                render,
                rect.x + 12.0 + indent,
                &mut cy,
                line_h,
                &format!("{prefix}window: {}", key.as_str()),
                "#C9CDD8",
            );

            for (id, hint) in window_branch_rows(layout, key) {
                let mode = layout.sync_registry().get(id);
                draw_node(
                    render,
                    rect.x + 12.0 + indent * 2.0,
                    &mut cy,
                    line_h,
                    id,
                    mode,
                    hint.as_deref(),
                );
            }
            cy += 4.0;
        }

        // Legend
        let legend_y = rect.y + rect.height - 24.0;
        let mut lx = rect.x + 12.0;
        for (label, mode) in [
            ("synced",          SyncMode::Synced),
            ("sometimes·alone", SyncMode::Sometimes(None)),
            ("standalone",      SyncMode::Standalone),
        ] {
            let css = rgba_css(mode.color());
            render.set_fill_color(&css);
            render.fill_rounded_rect(lx, legend_y, 14.0, 14.0, 3.0);
            render.set_fill_color("#C9CDD8");
            render.set_font("11px sans-serif");
            render.fill_text(label, lx + 20.0, legend_y + 11.0);
            lx += render.measure_text(label) + 50.0;
        }
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn rgba_css(c: [f32; 4]) -> String {
    format!(
        "rgba({},{},{},{:.2})",
        (c[0] * 255.0) as u8,
        (c[1] * 255.0) as u8,
        (c[2] * 255.0) as u8,
        c[3]
    )
}

fn draw_label(
    render: &mut dyn uzor::engine::render::RenderContext,
    x: f64,
    cy: &mut f64,
    line_h: f64,
    text: &str,
    color: &str,
) {
    render.set_fill_color(color);
    render.set_font("12px sans-serif");
    render.fill_text(text, x, *cy);
    *cy += line_h;
}

fn draw_node(
    render: &mut dyn uzor::engine::render::RenderContext,
    x: f64,
    cy: &mut f64,
    line_h: f64,
    node_id: &str,
    mode: SyncMode,
    hint: Option<&str>,
) {
    let pill_w = 92.0;
    let pill_h = 14.0;
    let pill_y = *cy - 11.0;

    let css = rgba_css(mode.color());
    render.set_fill_color(&css);
    render.fill_rounded_rect(x, pill_y, pill_w, pill_h, 3.0);
    render.set_fill_color("#0E0F12");
    render.set_font("bold 10px sans-serif");
    render.fill_text(mode.label(), x + 4.0, pill_y + 11.0);

    render.set_fill_color("#D6D9E2");
    render.set_font("12px sans-serif");
    let label = match hint {
        Some(h) if !h.is_empty() => format!("{node_id}    ({h})"),
        _ => node_id.to_string(),
    };
    render.fill_text(&label, x + pill_w + 8.0, *cy);
    *cy += line_h;
}

fn window_branch_rows(
    layout: &LayoutManager<NoPanel>,
    key: &WindowKey,
) -> Vec<(&'static str, Option<String>)> {
    let Some(branch) = layout.window(key) else { return Vec::new() };

    let dock_leaves = branch.dock.tree().leaves().len();
    let edge_count = branch.edges.iter().count();
    let overlay_count = branch.overlays.entries().len();
    let modal_count = branch.modals.len();
    let popup_count = branch.popups.len();
    let dropdown_count = branch.dropdowns.len();
    let toolbar_count = branch.toolbars.len();
    let sidebar_count = branch.sidebars.len();
    let context_menu_count = branch.context_menus.len();
    let hovered = branch.last_hovered.as_ref().map(|w| w.as_str().to_owned());
    let pointer = branch.last_pointer_pos.map(|(x, y)| format!("({x:.0},{y:.0})"));

    vec![
        (node_ids::CHROME_CFG,    Some(format!("visible={}", branch.chrome.visible))),
        (node_ids::EDGES,         Some(format!("{edge_count} slots"))),
        (node_ids::DOCK_TREE,     Some(format!("{dock_leaves} leaves"))),
        (node_ids::LAYOUT_TREE,   None),
        (node_ids::OVERLAYS,      Some(format!("{overlay_count} entries"))),
        (node_ids::POINTER_STATE, Some(format!(
            "hover={} at={}",
            hovered.unwrap_or_else(|| "—".into()),
            pointer.unwrap_or_else(|| "—".into()),
        ))),
        (node_ids::MODALS,        Some(format!("{modal_count}"))),
        (node_ids::POPUPS,        Some(format!("{popup_count}"))),
        (node_ids::DROPDOWNS,     Some(format!("{dropdown_count}"))),
        (node_ids::TOOLBARS,      Some(format!("{toolbar_count}"))),
        (node_ids::SIDEBARS,      Some(format!("{sidebar_count}"))),
        (node_ids::CONTEXT_MENUS, Some(format!("{context_menu_count}"))),
    ]
}

// ── main ─────────────────────────────────────────────────────────────────────

fn main() -> Result<(), Box<dyn std::error::Error>> {
    AppBuilder::new(TreeDebugApp)
        .title("uzor tree-debug · main")
        .size(820, 640)
        .decorations(true)
        .background(0xFF16171D)
        .window(
            WindowSpec::new(WindowKey::new("side"), "uzor tree-debug · side")
                .size(820, 640)
                .background(0xFF16171D),
        )
        .run()?;

    Ok(())
}
