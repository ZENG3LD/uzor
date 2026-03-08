//! Drop zone detection system for panel placement
//!
//! Implements VSCode-style 5-zone detection algorithm for drag-and-drop:
//! - **Center**: Add as tab to existing container
//! - **Left/Right/Up/Down**: Split container in that direction
//!
//! # Algorithm
//!
//! Uses two thresholds:
//! - **edge_threshold** (10%): Distance from edge to activate edge zones
//! - **split_threshold** (33%): Distance from edge to choose direction
//!
//! # Example
//!
//! ```
//! use uzor_panels::{DropZoneDetector, DropZone, PanelRect};
//!
//! let detector = DropZoneDetector::new();
//! let rect = PanelRect::new(0.0, 0.0, 100.0, 100.0);
//!
//! // Center point -> Center zone
//! assert_eq!(detector.detect(50.0, 50.0, rect), DropZone::Center);
//!
//! // Left edge -> Left zone
//! assert_eq!(detector.detect(5.0, 50.0, rect), DropZone::Left);
//! ```

use super::PanelRect;

// =============================================================================
// Drop Zone Types
// =============================================================================

/// Drop zone for panel placement (5 zones)
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum DropZone {
    /// Add as tab to existing container
    Center,
    /// Split left (new panel on left)
    Left,
    /// Split right (new panel on right)
    Right,
    /// Split top (new panel on top)
    Up,
    /// Split bottom (new panel on bottom)
    Down,
}

impl DropZone {
    /// Get preview rect for this drop zone
    /// Returns where the panel will be placed
    pub fn preview_rect(&self, container_rect: PanelRect) -> PanelRect {
        match self {
            DropZone::Center => container_rect,
            DropZone::Left => PanelRect {
                x: container_rect.x,
                y: container_rect.y,
                width: container_rect.width * 0.5,
                height: container_rect.height,
            },
            DropZone::Right => PanelRect {
                x: container_rect.x + container_rect.width * 0.5,
                y: container_rect.y,
                width: container_rect.width * 0.5,
                height: container_rect.height,
            },
            DropZone::Up => PanelRect {
                x: container_rect.x,
                y: container_rect.y,
                width: container_rect.width,
                height: container_rect.height * 0.5,
            },
            DropZone::Down => PanelRect {
                x: container_rect.x,
                y: container_rect.y + container_rect.height * 0.5,
                width: container_rect.width,
                height: container_rect.height * 0.5,
            },
        }
    }
}

/// Drop zone detector using VSCode algorithm
///
/// Uses two thresholds:
/// - **edge_threshold** (10%): Distance from edge to activate edge zones
/// - **split_threshold** (33%): Distance from edge to choose direction
pub struct DropZoneDetector {
    edge_threshold: f32,
    split_threshold: f32,
}

impl DropZoneDetector {
    /// Create new detector with default thresholds
    pub fn new() -> Self {
        Self {
            edge_threshold: 0.10,
            split_threshold: 0.33,
        }
    }

    /// Create detector with custom thresholds
    pub fn with_thresholds(edge_threshold: f32, split_threshold: f32) -> Self {
        Self {
            edge_threshold,
            split_threshold,
        }
    }

    /// Detect drop zone from mouse position relative to container
    ///
    /// # Arguments
    /// - `x`, `y`: Mouse position (absolute coordinates)
    /// - `rect`: Container rectangle
    ///
    /// # Algorithm
    /// 1. Check if inside center zone (10% margin from edges)
    /// 2. If outside center, determine direction using 33% thresholds
    pub fn detect(&self, x: f32, y: f32, rect: PanelRect) -> DropZone {
        let width = rect.width;
        let height = rect.height;

        // Convert absolute position to relative
        let rel_x = x - rect.x;
        let rel_y = y - rect.y;

        // Edge activation thresholds (10% from borders)
        let edge_x = width * self.edge_threshold;
        let edge_y = height * self.edge_threshold;

        // Check center zone first
        if rel_x > edge_x && rel_x < width - edge_x && rel_y > edge_y && rel_y < height - edge_y {
            return DropZone::Center;
        }

        // Split thresholds (33% for left/right/up/down choice)
        let split_x = width * self.split_threshold;

        // Determine direction based on position
        if rel_x < split_x {
            DropZone::Left
        } else if rel_x > width - split_x {
            DropZone::Right
        } else if rel_y < height / 2.0 {
            DropZone::Up
        } else {
            DropZone::Down
        }
    }
}

impl Default for DropZoneDetector {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Compass Zone (for rendering compass widget)
// =============================================================================

/// Compass zone for rendering drop zone indicators
///
/// Same as DropZone but used specifically for compass widget rendering.
/// Keeps UI rendering separate from state machine logic.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum CompassZone {
    /// Center zone indicator
    Center,
    /// Left zone indicator
    Left,
    /// Right zone indicator
    Right,
    /// Up zone indicator
    Up,
    /// Down zone indicator
    Down,
}

impl From<DropZone> for CompassZone {
    fn from(zone: DropZone) -> Self {
        match zone {
            DropZone::Center => CompassZone::Center,
            DropZone::Left => CompassZone::Left,
            DropZone::Right => CompassZone::Right,
            DropZone::Up => CompassZone::Up,
            DropZone::Down => CompassZone::Down,
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drop_zone_center() {
        let detector = DropZoneDetector::new();
        let rect = PanelRect::new(0.0, 0.0, 100.0, 100.0);

        // Center point should be Center zone
        let zone = detector.detect(50.0, 50.0, rect);
        assert_eq!(zone, DropZone::Center);
    }

    #[test]
    fn test_drop_zone_edges() {
        let detector = DropZoneDetector::new();
        let rect = PanelRect::new(0.0, 0.0, 100.0, 100.0);

        // Left edge (within 33% threshold)
        let zone = detector.detect(5.0, 50.0, rect);
        assert_eq!(zone, DropZone::Left);

        // Right edge
        let zone = detector.detect(95.0, 50.0, rect);
        assert_eq!(zone, DropZone::Right);

        // Top edge (middle 33%)
        let zone = detector.detect(50.0, 5.0, rect);
        assert_eq!(zone, DropZone::Up);

        // Bottom edge
        let zone = detector.detect(50.0, 95.0, rect);
        assert_eq!(zone, DropZone::Down);
    }

    #[test]
    fn test_drop_zone_preview_rect() {
        let rect = PanelRect::new(0.0, 0.0, 100.0, 100.0);

        let preview = DropZone::Left.preview_rect(rect);
        assert_eq!(preview.width, 50.0);
        assert_eq!(preview.height, 100.0);
        assert_eq!(preview.x, 0.0);

        let preview = DropZone::Up.preview_rect(rect);
        assert_eq!(preview.width, 100.0);
        assert_eq!(preview.height, 50.0);
        assert_eq!(preview.y, 0.0);

        let preview = DropZone::Center.preview_rect(rect);
        assert_eq!(preview, rect);
    }

    #[test]
    fn test_compass_zone_conversion() {
        assert_eq!(CompassZone::from(DropZone::Center), CompassZone::Center);
        assert_eq!(CompassZone::from(DropZone::Left), CompassZone::Left);
        assert_eq!(CompassZone::from(DropZone::Right), CompassZone::Right);
        assert_eq!(CompassZone::from(DropZone::Up), CompassZone::Up);
        assert_eq!(CompassZone::from(DropZone::Down), CompassZone::Down);
    }
}
