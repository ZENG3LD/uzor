//! Constraint-based layout splitting.

use crate::rect::Rect;

/// Direction for layout splitting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Horizontal,
    Vertical,
}

/// Size constraint for a layout segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Constraint {
    /// Fixed number of cells.
    Fixed(u16),
    /// Percentage of available space (0-100).
    Percentage(u16),
    /// Minimum number of cells (fills remaining space, at least this much).
    Min(u16),
    /// Maximum number of cells (fills remaining space, at most this much).
    Max(u16),
    /// Proportional share: num/den of available space.
    Ratio(u16, u16),
}

/// Split a rect into sub-rects according to constraints.
///
/// Two-pass algorithm:
/// 1. Resolve Fixed and Percentage constraints first.
/// 2. Distribute remaining space among Min/Max/Ratio constraints.
pub fn split(area: Rect, direction: Direction, constraints: &[Constraint]) -> Vec<Rect> {
    if constraints.is_empty() || area.is_empty() {
        return vec![area; constraints.len().max(1)];
    }

    let total = match direction {
        Direction::Horizontal => area.width as u32,
        Direction::Vertical => area.height as u32,
    };

    // First pass: resolve deterministic sizes
    let mut sizes: Vec<Option<u32>> = vec![None; constraints.len()];
    let mut used: u32 = 0;

    for (i, c) in constraints.iter().enumerate() {
        match c {
            Constraint::Fixed(n) => {
                let s = (*n as u32).min(total.saturating_sub(used));
                sizes[i] = Some(s);
                used += s;
            }
            Constraint::Percentage(p) => {
                let s = (total * (*p as u32).min(100) / 100).min(total.saturating_sub(used));
                sizes[i] = Some(s);
                used += s;
            }
            _ => {} // resolved in second pass
        }
    }

    // Second pass: distribute remaining space
    let remaining = total.saturating_sub(used);
    let flexible: Vec<usize> = sizes.iter().enumerate()
        .filter(|(_, s)| s.is_none())
        .map(|(i, _)| i)
        .collect();

    if !flexible.is_empty() {
        // Calculate total ratio weight
        let total_weight: u32 = flexible.iter().map(|&i| {
            match constraints[i] {
                Constraint::Ratio(num, _den) => num as u32,
                _ => 1, // Min and Max get weight 1
            }
        }).sum();

        let total_weight = total_weight.max(1);
        let mut distributed: u32 = 0;

        for (idx, &i) in flexible.iter().enumerate() {
            let weight = match constraints[i] {
                Constraint::Ratio(num, _den) => num as u32,
                _ => 1,
            };

            let share = if idx == flexible.len() - 1 {
                // Last flexible gets all remaining to avoid rounding errors
                remaining.saturating_sub(distributed)
            } else {
                remaining * weight / total_weight
            };

            let clamped = match constraints[i] {
                Constraint::Min(min) => share.max(min as u32),
                Constraint::Max(max) => share.min(max as u32),
                Constraint::Ratio(num, den) => {
                    if den == 0 { 0 } else { (remaining * num as u32) / den as u32 }
                }
                _ => share,
            };

            // Don't exceed remaining
            let final_size = clamped.min(remaining.saturating_sub(distributed));
            sizes[i] = Some(final_size);
            distributed += final_size;
        }
    }

    // Fill any still-None with 0
    let sizes: Vec<u16> = sizes.iter().map(|s| s.unwrap_or(0) as u16).collect();

    // Build rects
    let mut result = Vec::with_capacity(constraints.len());
    let mut offset = match direction {
        Direction::Horizontal => area.x,
        Direction::Vertical => area.y,
    };

    for size in &sizes {
        let r = match direction {
            Direction::Horizontal => Rect::new(offset, area.y, *size, area.height),
            Direction::Vertical => Rect::new(area.x, offset, area.width, *size),
        };
        result.push(r);
        offset = offset.saturating_add(*size);
    }

    result
}

/// Split a rect into N equal parts.
pub fn split_equal(area: Rect, direction: Direction, n: u16) -> Vec<Rect> {
    if n == 0 {
        return vec![];
    }
    let constraints: Vec<Constraint> = (0..n).map(|_| Constraint::Ratio(1, n)).collect();
    split(area, direction, &constraints)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_fixed() {
        let area = Rect::new(0, 0, 100, 10);
        let rects = split(area, Direction::Horizontal, &[
            Constraint::Fixed(20),
            Constraint::Fixed(30),
            Constraint::Fixed(50),
        ]);
        assert_eq!(rects.len(), 3);
        assert_eq!(rects[0], Rect::new(0, 0, 20, 10));
        assert_eq!(rects[1], Rect::new(20, 0, 30, 10));
        assert_eq!(rects[2], Rect::new(50, 0, 50, 10));
    }

    #[test]
    fn test_split_vertical() {
        let area = Rect::new(0, 0, 80, 24);
        let rects = split(area, Direction::Vertical, &[
            Constraint::Fixed(1),
            Constraint::Min(0),
            Constraint::Fixed(1),
        ]);
        assert_eq!(rects.len(), 3);
        assert_eq!(rects[0], Rect::new(0, 0, 80, 1));
        assert_eq!(rects[1], Rect::new(0, 1, 80, 22));
        assert_eq!(rects[2], Rect::new(0, 23, 80, 1));
    }

    #[test]
    fn test_split_percentage() {
        let area = Rect::new(0, 0, 100, 10);
        let rects = split(area, Direction::Horizontal, &[
            Constraint::Percentage(30),
            Constraint::Percentage(70),
        ]);
        assert_eq!(rects[0].width, 30);
        assert_eq!(rects[1].width, 70);
    }

    #[test]
    fn test_split_equal() {
        let area = Rect::new(0, 0, 90, 10);
        let rects = split_equal(area, Direction::Horizontal, 3);
        assert_eq!(rects.len(), 3);
        assert_eq!(rects[0].width, 30);
        assert_eq!(rects[1].width, 30);
        assert_eq!(rects[2].width, 30);
    }

    #[test]
    fn test_split_empty() {
        let area = Rect::ZERO;
        let rects = split(area, Direction::Horizontal, &[Constraint::Fixed(10)]);
        assert_eq!(rects.len(), 1);
    }

    #[test]
    fn test_split_mixed() {
        let area = Rect::new(0, 0, 80, 24);
        let rects = split(area, Direction::Vertical, &[
            Constraint::Fixed(3),    // header
            Constraint::Min(0),      // body (fill)
            Constraint::Fixed(1),    // status bar
        ]);
        assert_eq!(rects[0].height, 3);
        assert_eq!(rects[1].height, 20);
        assert_eq!(rects[2].height, 1);
    }
}
