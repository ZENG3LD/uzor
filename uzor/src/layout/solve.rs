use crate::core::types::Rect;
use super::tree::LayoutTree;
use super::chrome_slot::ChromeSlot;
use super::edge_panels::EdgePanels;
use super::types::{EdgeSide, LayoutSolved};

/// Run the macro layout pass: chrome → edges → dock area.
///
/// Mutates `tree` to store the computed rect for each system node.
/// Does **not** solve the dock subtree, floating window positions, or overlay
/// rects — those are delegated to `PanelDockingManager` and `OverlayStack`.
///
/// # Algorithm
///
/// 1. Chrome consumes from the top (if visible).
/// 2. Top edge slots consume from the top, stacked in `order` ascending.
/// 3. Bottom edge slots consume from the bottom, stacked in `order` ascending.
/// 4. Left edge slots consume from the left, stacked in `order` ascending.
/// 5. Right edge slots consume from the right, stacked in `order` ascending.
/// 6. The remaining rect is the dock area (= floating area, z-above dock).
pub fn solve_layout(
    window: Rect,
    chrome: &ChromeSlot,
    edges: &EdgePanels,
    tree: &mut LayoutTree,
) -> LayoutSolved {
    let mut remaining = window;
    let mut solved = LayoutSolved::default();

    // ------------------------------------------------------------------
    // 1. Chrome
    // ------------------------------------------------------------------
    if chrome.visible && chrome.height > 0.0 {
        let h = chrome.height as f64;
        let chrome_rect = Rect::new(remaining.x, remaining.y, remaining.width, h);
        remaining.y += h;
        remaining.height = (remaining.height - h).max(0.0);
        tree.set_rect(tree.chrome_id(), chrome_rect);
        solved.chrome = Some(chrome_rect);
    } else {
        tree.set_rect(tree.chrome_id(), Rect::new(remaining.x, remaining.y, remaining.width, 0.0));
    }

    // ------------------------------------------------------------------
    // 2. Top edge slots
    // ------------------------------------------------------------------
    {
        let top_slots: Vec<_> = edges.slots_for(EdgeSide::Top).collect();
        let mut slot_rects = Vec::with_capacity(top_slots.len());
        for slot in &top_slots {
            let h = slot.thickness as f64;
            let r = Rect::new(remaining.x, remaining.y, remaining.width, h);
            remaining.y += h;
            remaining.height = (remaining.height - h).max(0.0);
            slot_rects.push(r);
        }
        let combined_h: f64 = slot_rects.iter().map(|r| r.height).sum();
        tree.set_rect(
            tree.edge_id(EdgeSide::Top),
            Rect::new(window.x, solved.chrome.map_or(window.y, |c| c.y + c.height), window.width, combined_h),
        );
        solved.edges.top = slot_rects;
    }

    // ------------------------------------------------------------------
    // 3. Bottom edge slots
    // ------------------------------------------------------------------
    {
        let bot_slots: Vec<_> = edges.slots_for(EdgeSide::Bottom).collect();
        let mut slot_rects = Vec::with_capacity(bot_slots.len());
        for slot in &bot_slots {
            let h = slot.thickness as f64;
            let r = Rect::new(
                remaining.x,
                remaining.y + remaining.height - h,
                remaining.width,
                h,
            );
            remaining.height = (remaining.height - h).max(0.0);
            slot_rects.push(r);
        }
        let combined_h: f64 = slot_rects.iter().map(|r| r.height).sum();
        tree.set_rect(
            tree.edge_id(EdgeSide::Bottom),
            Rect::new(window.x, window.y + window.height - combined_h, window.width, combined_h),
        );
        solved.edges.bottom = slot_rects;
    }

    // ------------------------------------------------------------------
    // 4. Left edge slots
    // ------------------------------------------------------------------
    {
        let left_slots: Vec<_> = edges.slots_for(EdgeSide::Left).collect();
        let mut slot_rects = Vec::with_capacity(left_slots.len());
        for slot in &left_slots {
            let w = slot.thickness as f64;
            let r = Rect::new(remaining.x, remaining.y, w, remaining.height);
            remaining.x += w;
            remaining.width = (remaining.width - w).max(0.0);
            slot_rects.push(r);
        }
        let combined_w: f64 = slot_rects.iter().map(|r| r.width).sum();
        tree.set_rect(
            tree.edge_id(EdgeSide::Left),
            Rect::new(window.x, remaining.y - remaining.height, combined_w, remaining.height),
        );
        solved.edges.left = slot_rects;
    }

    // ------------------------------------------------------------------
    // 5. Right edge slots
    // ------------------------------------------------------------------
    {
        let right_slots: Vec<_> = edges.slots_for(EdgeSide::Right).collect();
        let mut slot_rects = Vec::with_capacity(right_slots.len());
        for slot in &right_slots {
            let w = slot.thickness as f64;
            let r = Rect::new(
                remaining.x + remaining.width - w,
                remaining.y,
                w,
                remaining.height,
            );
            remaining.width = (remaining.width - w).max(0.0);
            slot_rects.push(r);
        }
        let combined_w: f64 = slot_rects.iter().map(|r| r.width).sum();
        tree.set_rect(
            tree.edge_id(EdgeSide::Right),
            Rect::new(window.x + window.width - combined_w, remaining.y, combined_w, remaining.height),
        );
        solved.edges.right = slot_rects;
    }

    // ------------------------------------------------------------------
    // 6. Dock area = floating area = remaining
    // ------------------------------------------------------------------
    let dock_area = remaining;
    tree.set_rect(tree.dock_root_id(), dock_area);
    tree.set_rect(tree.floating_id(), dock_area);

    solved.dock_area = dock_area;
    solved.floating_area = dock_area;

    solved
}
