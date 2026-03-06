//! Layout calculation utilities

use uzor_core::types::rect::Rect;

/// Stack rects vertically with spacing
pub fn stack_vertical(container: Rect, item_height: f64, spacing: f64, count: usize) -> Vec<Rect> {
    let mut rects = Vec::with_capacity(count);
    let mut y = container.y;

    for _ in 0..count {
        rects.push(Rect::new(container.x, y, container.width, item_height));
        y += item_height + spacing;
    }

    rects
}

/// Stack rects horizontally with spacing
pub fn stack_horizontal(container: Rect, item_width: f64, spacing: f64, count: usize) -> Vec<Rect> {
    let mut rects = Vec::with_capacity(count);
    let mut x = container.x;

    for _ in 0..count {
        rects.push(Rect::new(x, container.y, item_width, container.height));
        x += item_width + spacing;
    }

    rects
}

/// Create a grid layout
pub fn grid_layout(container: Rect, cols: usize, rows: usize, spacing: f64) -> Vec<Rect> {
    let mut rects = Vec::with_capacity(cols * rows);

    let cell_width = (container.width - spacing * (cols - 1) as f64) / cols as f64;
    let cell_height = (container.height - spacing * (rows - 1) as f64) / rows as f64;

    for row in 0..rows {
        for col in 0..cols {
            let x = container.x + col as f64 * (cell_width + spacing);
            let y = container.y + row as f64 * (cell_height + spacing);
            rects.push(Rect::new(x, y, cell_width, cell_height));
        }
    }

    rects
}

/// Distribute space evenly among items
pub fn distribute_space(container: Rect, item_count: usize) -> Vec<Rect> {
    let item_width = container.width / item_count as f64;
    let mut rects = Vec::with_capacity(item_count);

    for i in 0..item_count {
        let x = container.x + i as f64 * item_width;
        rects.push(Rect::new(x, container.y, item_width, container.height));
    }

    rects
}
