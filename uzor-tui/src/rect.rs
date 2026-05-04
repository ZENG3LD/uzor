//! TUI rectangle in terminal cell coordinates (u16).

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    pub const ZERO: Rect = Rect { x: 0, y: 0, width: 0, height: 0 };

    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self { x, y, width, height }
    }

    pub const fn area(&self) -> u32 {
        (self.width as u32) * (self.height as u32)
    }

    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    pub const fn left(&self) -> u16 { self.x }
    pub const fn top(&self) -> u16 { self.y }
    pub const fn right(&self) -> u16 { self.x.saturating_add(self.width) }
    pub const fn bottom(&self) -> u16 { self.y.saturating_add(self.height) }

    /// Shrink by 1 cell on each side (content area inside a border).
    pub fn inner(&self) -> Rect {
        if self.width < 2 || self.height < 2 {
            return Rect::ZERO;
        }
        Rect {
            x: self.x.saturating_add(1),
            y: self.y.saturating_add(1),
            width: self.width.saturating_sub(2),
            height: self.height.saturating_sub(2),
        }
    }

    /// Shrink by arbitrary margin on each side.
    pub fn inset(&self, top: u16, right: u16, bottom: u16, left: u16) -> Rect {
        let x = self.x.saturating_add(left);
        let y = self.y.saturating_add(top);
        let w = self.width.saturating_sub(left.saturating_add(right));
        let h = self.height.saturating_sub(top.saturating_add(bottom));
        Rect::new(x, y, w, h)
    }

    /// Intersection of two rects.
    pub fn intersect(&self, other: Rect) -> Rect {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let r = self.right().min(other.right());
        let b = self.bottom().min(other.bottom());
        if x >= r || y >= b {
            Rect::ZERO
        } else {
            Rect::new(x, y, r - x, b - y)
        }
    }

    /// Does this rect contain the point (col, row)?
    pub fn contains(&self, col: u16, row: u16) -> bool {
        col >= self.x && col < self.right() && row >= self.y && row < self.bottom()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_and_area() {
        let r = Rect::new(0, 0, 10, 5);
        assert_eq!(r.area(), 50);
    }

    #[test]
    fn test_edges() {
        let r = Rect::new(2, 3, 10, 5);
        assert_eq!(r.left(), 2);
        assert_eq!(r.top(), 3);
        assert_eq!(r.right(), 12);
        assert_eq!(r.bottom(), 8);
    }

    #[test]
    fn test_inner() {
        let r = Rect::new(0, 0, 10, 5);
        let i = r.inner();
        assert_eq!(i, Rect::new(1, 1, 8, 3));
    }

    #[test]
    fn test_inner_too_small() {
        let r = Rect::new(0, 0, 1, 1);
        assert_eq!(r.inner(), Rect::ZERO);
    }

    #[test]
    fn test_inset() {
        let r = Rect::new(0, 0, 20, 10);
        let i = r.inset(1, 2, 1, 2);
        assert_eq!(i, Rect::new(2, 1, 16, 8));
    }

    #[test]
    fn test_intersect() {
        let a = Rect::new(0, 0, 10, 10);
        let b = Rect::new(5, 5, 10, 10);
        assert_eq!(a.intersect(b), Rect::new(5, 5, 5, 5));
    }

    #[test]
    fn test_intersect_no_overlap() {
        let a = Rect::new(0, 0, 5, 5);
        let b = Rect::new(10, 10, 5, 5);
        assert_eq!(a.intersect(b), Rect::ZERO);
    }

    #[test]
    fn test_contains() {
        let r = Rect::new(2, 3, 10, 5);
        assert!(r.contains(2, 3));
        assert!(r.contains(11, 7));
        assert!(!r.contains(12, 3)); // right edge exclusive
        assert!(!r.contains(2, 8));  // bottom edge exclusive
        assert!(!r.contains(1, 3));  // left of rect
    }

    #[test]
    fn test_is_empty() {
        assert!(Rect::ZERO.is_empty());
        assert!(Rect::new(0, 0, 0, 5).is_empty());
        assert!(!Rect::new(0, 0, 1, 1).is_empty());
    }
}
