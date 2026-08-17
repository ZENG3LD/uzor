//! Last-registered region wins. Same rule both apps already use.

use crate::rect::Rect;

/// Hit kind the shell understands. Apps map these to their own commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    OverlayDrag,
    OverlayClose,
    TopTab(u8),
    SideTab(u8),
    FooterButton(u8),
    Body,
    Custom(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HitRegion {
    pub rect: Rect,
    pub hit: Hit,
}

#[derive(Clone, Debug, Default)]
pub struct HitLayer {
    regions: Vec<HitRegion>,
}

impl HitLayer {
    pub fn new() -> Self {
        Self {
            regions: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.regions.clear();
    }

    pub fn push(&mut self, rect: Rect, hit: Hit) {
        if !rect.is_empty() {
            self.regions.push(HitRegion { rect, hit });
        }
    }

    /// Topmost (last) region under the point.
    pub fn hit(&self, x: u16, y: u16) -> Option<Hit> {
        self.regions
            .iter()
            .rev()
            .find(|r| r.rect.contains(x, y))
            .map(|r| r.hit)
    }

    pub fn regions(&self) -> &[HitRegion] {
        &self.regions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_region_wins() {
        let mut layer = HitLayer::new();
        layer.push(Rect::new(0, 0, 10, 5), Hit::Body);
        layer.push(Rect::new(0, 0, 3, 1), Hit::OverlayClose);
        assert_eq!(layer.hit(1, 0), Some(Hit::OverlayClose));
        assert_eq!(layer.hit(5, 2), Some(Hit::Body));
        assert_eq!(layer.hit(20, 0), None);
    }
}
