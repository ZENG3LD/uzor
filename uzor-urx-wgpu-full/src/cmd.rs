//! GPU-side encoded scene commands. Flat 32-byte structs uploaded as a
//! storage buffer; the compute pipeline reads this directly without
//! type-tagged unions.
//!
//! Layout matches research-16 §3 "Scene encoding" — kind discriminator
//! in the first u32, geometry payload follows.

use bytemuck::{Pod, Zeroable};

/// Discriminator for `SceneCmd.kind`.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmdKind {
    /// Filled axis-aligned rect. payload = [x0, y0, x1, y1], packed_rgba in slot[4].
    Rect = 0,
    /// Reserved.
    _Reserved = 1,
    /// Two-color linear gradient over a rect.
    /// slot[0..4] = bbox xyxy, slot[4] = start_color packed rgba8, slot[5] = end_color packed rgba8,
    /// slot[6] = direction enum: 0=L→R, 1=T→B, 2=TL→BR diagonal, 3=BL→TR diagonal.
    LinGradient = 2,
    /// Radial gradient over a rect: inner color at bbox center, outer color at bbox edge.
    /// slot[0..4] = bbox xyxy, slot[4] = inner_color packed rgba8, slot[5] = outer_color packed rgba8,
    /// slot[6] = reserved (center + radius derived from bbox at shader time).
    RadGradient = 3,
    /// Pre-rasterised glyph from atlas.
    /// slot[0..4] = bbox xyxy (screen-space rect to draw into),
    /// slot[4]    = packed RGBA u32 (colour modulation),
    /// slot[5]    = atlas UV packed: low16 = u0_q, high16 = v0_q (quantised to [0,65535]),
    /// slot[6]    = atlas UV packed: low16 = u1_q, high16 = v1_q.
    Glyph = 4,
    /// Stroked line segment from p0 → p1 with scalar width.
    ///
    /// IMPORTANT: for Stroke, slots 0..4 are NOT bbox — they're endpoints.
    /// The tile_assign shader derives the inflated bbox at dispatch time
    /// from (p0, p1, width). The encoder-side `bbox()` helper does the
    /// same for CPU-side use.
    ///
    /// slot[0] = p0_x, slot[1] = p0_y, slot[2] = p1_x, slot[3] = p1_y,
    /// slot[4] = packed_rgba u32,
    /// slot[5] = f32 width (as bits via `f32::to_bits`),
    /// slot[6] = flags: cap_kind in low 8 bits (0=butt, 1=round, 2=square)
    Stroke = 5,
    /// Multi-segment polyline (flattened path / stroked curve).
    ///
    /// Points live in a separate `path_points` storage buffer; this
    /// cmd only carries the AABB + index range + style.
    ///
    /// slot[0..4] = bbox xyxy (pre-computed CPU-side from points + half-width).
    /// slot[4]    = packed_rgba u32.
    /// slot[5]    = packed (width_q × 100, point_count u16):
    ///                low  16 bits = width × 100 (quantised, range 0..655.35 px)
    ///                high 16 bits = point_count (range 2..65535)
    /// slot[6]    = point_offset (u32 — first point's index in path_points).
    ///
    /// Cap is implicit ROUND for paths (joins between consecutive
    /// segments are implicit; exterior ends round-cap from the SDF).
    Path = 6,
    /// Filled polygon (closed flattened path interior). Uses the same
    /// `path_points` buffer as Path; the GPU runs a per-pixel ray-
    /// crossing test (non-zero winding rule) over the segment list.
    ///
    /// slot[0..4] = bbox xyxy (CPU-side AABB of all path points).
    /// slot[4]    = packed_rgba u32.
    /// slot[5]    = point_count (u32 — number of path_points vertices;
    ///              the polygon implicitly closes from last to first).
    /// slot[6]    = point_offset (u32 — first point in path_points).
    ///
    /// Concave / self-intersecting paths work; holes via even-odd rule
    /// not yet supported (current shader uses non-zero only).
    FillPath = 7,
    /// N-stop linear gradient (up to 65535 stops, practical limit ~16).
    ///
    /// Stops are packed two-per-`path_points`-entry: a stop is encoded
    /// as `[position: f32, bitcast<f32>(packed_rgba: u32)]`. Reusing
    /// the existing `path_points` storage avoids adding a new BGL slot.
    ///
    /// slot[0..4] = bbox xyxy.
    /// slot[4]    = direction (lin_dir::*).
    /// slot[5]    = stop_count (u32, range 2..=65535).
    /// slot[6]    = stop_offset (u32 — first stop in path_points; each
    ///              stop occupies ONE vec2<f32> there).
    ///
    /// Stops MUST be sorted by position ascending; positions are
    /// clamped to `[0, 1]` at sample time.
    MultiLinGradient = 8,
    /// Image / texture brush. Samples an RGBA8 atlas (binding 8) at
    /// the cmd's UV rect, modulated by an RGBA tint colour.
    ///
    /// slot[0..4] = bbox xyxy (screen-space rect to fill).
    /// slot[4]    = packed_rgba u32 (tint — `[255, 255, 255, 255]` for
    ///              the unmodulated image).
    /// slot[5]    = atlas UV packed: low16 = u0_q, high16 = v0_q
    ///              (quantised × 65535).
    /// slot[6]    = atlas UV packed: low16 = u1_q, high16 = v1_q.
    ///
    /// Same UV packing as Glyph (kind=4); the only difference is the
    /// atlas binding (RGBA8 vs R8Unorm) and that Image multiplies the
    /// sampled rgba × tint instead of using the texel as alpha mask.
    Image = 9,
}

/// One flat scene command. 32 bytes total, repr(C) for stable layout.
///
/// Slots interpretation by `kind`:
///   Rect:        slot[0..4] = bbox xyxy (f32), slot[4] = packed_rgba u32, slot[5..7] = unused
///   LinGradient: slot[0..4] = bbox xyxy (f32), slot[4] = start_color, slot[5] = end_color,
///                slot[6] = direction (0=L→R, 1=T→B, 2=TL→BR, 3=BL→TR)
///   RadGradient: slot[0..4] = bbox xyxy (f32), slot[4] = inner_color, slot[5] = outer_color,
///                slot[6] = reserved
///   Glyph:       slot[0..4] = bbox xyxy (f32), slot[4] = packed_rgba colour modulation,
///                slot[5] = u16x2(u0_q, v0_q), slot[6] = u16x2(u1_q, v1_q)
///                where u/v_q = normalised UV × 65535 packed as lo16/hi16
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct SceneCmd {
    pub kind:  u32,
    pub slot0: f32,
    pub slot1: f32,
    pub slot2: f32,
    pub slot3: f32,
    pub slot4: u32,
    pub slot5: u32,
    pub slot6: u32,
}

/// Direction encoding for `CmdKind::LinGradient`.
pub mod lin_dir {
    /// Gradient flows left → right (along +X).
    pub const HORIZONTAL: u32 = 0;
    /// Gradient flows top → bottom (along +Y).
    pub const VERTICAL: u32 = 1;
    /// Gradient flows top-left → bottom-right.
    pub const DIAGONAL_TLBR: u32 = 2;
    /// Gradient flows bottom-left → top-right.
    pub const DIAGONAL_BLTR: u32 = 3;
}

/// Cap kind encoding for `CmdKind::Stroke`.
pub mod cap_kind {
    /// Flat butt cap — terminates the stroke at the endpoint exactly.
    pub const BUTT:   u32 = 0;
    /// Round cap — semicircular cap of radius = width/2 at each endpoint.
    pub const ROUND:  u32 = 1;
    /// Square cap — extends the stroke by width/2 past each endpoint.
    pub const SQUARE: u32 = 2;
}

fn pack_rgba(rgba: [u8; 4]) -> u32 {
    (rgba[0] as u32)
        | ((rgba[1] as u32) << 8)
        | ((rgba[2] as u32) << 16)
        | ((rgba[3] as u32) << 24)
}

impl SceneCmd {
    /// Create a Rect command from bbox corners and RGBA bytes.
    pub fn rect(x0: f32, y0: f32, x1: f32, y1: f32, rgba: [u8; 4]) -> Self {
        Self {
            kind: CmdKind::Rect as u32,
            slot0: x0, slot1: y0, slot2: x1, slot3: y1,
            slot4: pack_rgba(rgba), slot5: 0, slot6: 0,
        }
    }

    /// Create a two-color linear gradient command.
    ///
    /// `direction` is one of the `lin_dir::*` constants:
    ///   - `HORIZONTAL`    (0): start_rgba at left, end_rgba at right
    ///   - `VERTICAL`      (1): start_rgba at top,  end_rgba at bottom
    ///   - `DIAGONAL_TLBR` (2): start_rgba at top-left,    end_rgba at bottom-right
    ///   - `DIAGONAL_BLTR` (3): start_rgba at bottom-left, end_rgba at top-right
    pub fn lin_gradient(
        x0: f32, y0: f32, x1: f32, y1: f32,
        start_rgba: [u8; 4],
        end_rgba: [u8; 4],
        direction: u32,
    ) -> Self {
        Self {
            kind: CmdKind::LinGradient as u32,
            slot0: x0, slot1: y0, slot2: x1, slot3: y1,
            slot4: pack_rgba(start_rgba),
            slot5: pack_rgba(end_rgba),
            slot6: direction,
        }
    }

    /// Create a radial gradient command.
    ///
    /// Center and radius are derived from the bbox at shader time:
    ///   center = bbox midpoint, max_r = max(half-width, half-height).
    /// `inner_rgba` is the color at the center; `outer_rgba` at the perimeter.
    pub fn rad_gradient(
        x0: f32, y0: f32, x1: f32, y1: f32,
        inner_rgba: [u8; 4],
        outer_rgba: [u8; 4],
    ) -> Self {
        Self {
            kind: CmdKind::RadGradient as u32,
            slot0: x0, slot1: y0, slot2: x1, slot3: y1,
            slot4: pack_rgba(inner_rgba),
            slot5: pack_rgba(outer_rgba),
            slot6: 0,
        }
    }

    /// Create a glyph command from pre-rasterised atlas.
    ///
    /// - `rgba`: colour modulation (multiplied with the atlas alpha channel).
    /// - `uv_rect`: `[u0, v0, u1, v1]` in normalised atlas coordinates [0.0, 1.0].
    ///   Internally quantised to u16 per component (× 65535).
    pub fn glyph(x0: f32, y0: f32, x1: f32, y1: f32, rgba: [u8; 4], uv_rect: [f32; 4]) -> Self {
        let quant = |v: f32| -> u32 { (v.clamp(0.0, 1.0) * 65535.0).round() as u32 };
        let u0q = quant(uv_rect[0]);
        let v0q = quant(uv_rect[1]);
        let u1q = quant(uv_rect[2]);
        let v1q = quant(uv_rect[3]);
        Self {
            kind: CmdKind::Glyph as u32,
            slot0: x0, slot1: y0, slot2: x1, slot3: y1,
            slot4: pack_rgba(rgba),
            slot5: u0q | (v0q << 16),
            slot6: u1q | (v1q << 16),
        }
    }

    /// Dequantise atlas UV rect from a Glyph command.
    ///
    /// Returns `Some([u0, v0, u1, v1])` if `kind == Glyph`, `None` otherwise.
    pub fn glyph_uv(&self) -> Option<[f32; 4]> {
        if self.kind != CmdKind::Glyph as u32 {
            return None;
        }
        let dequant = |q: u32| -> f32 { (q & 0xffff) as f32 / 65535.0 };
        Some([
            dequant(self.slot5),
            dequant(self.slot5 >> 16),
            dequant(self.slot6),
            dequant(self.slot6 >> 16),
        ])
    }

    /// Create a stroked line segment from `p0` to `p1` with scalar width.
    ///
    /// `cap` is one of `cap_kind::{BUTT, ROUND, SQUARE}`.
    pub fn stroke(
        p0x: f32, p0y: f32, p1x: f32, p1y: f32,
        width: f32,
        rgba: [u8; 4],
        cap: u32,
    ) -> Self {
        Self {
            kind: CmdKind::Stroke as u32,
            slot0: p0x, slot1: p0y, slot2: p1x, slot3: p1y,
            slot4: pack_rgba(rgba),
            slot5: width.to_bits(),
            slot6: cap & 0xff,
        }
    }

    /// Returns the bounding box `[x0, y0, x1, y1]` for any cmd kind.
    ///
    /// For `Stroke`, computes the inflated bbox from the endpoints +
    /// half-width (matches what `tile_assign.wgsl` does GPU-side).
    pub fn bbox(&self) -> [f32; 4] {
        if self.kind == CmdKind::Stroke as u32 {
            let half = f32::from_bits(self.slot5) * 0.5;
            // Square cap extends an extra half-width past each endpoint;
            // round/butt stay within half-width radius. We use half-width
            // for the AABB pad in all three cases — square caps that paint
            // slightly outside the AABB get clipped at the tile boundary,
            // which is fine as the contribution there is zero coverage.
            let x_min = self.slot0.min(self.slot2) - half;
            let y_min = self.slot1.min(self.slot3) - half;
            let x_max = self.slot0.max(self.slot2) + half;
            let y_max = self.slot1.max(self.slot3) + half;
            [x_min, y_min, x_max, y_max]
        } else {
            [self.slot0, self.slot1, self.slot2, self.slot3]
        }
    }

    /// Dequantise stroke parameters from a Stroke command.
    ///
    /// Returns `Some([p0x, p0y, p1x, p1y, width])` + cap kind, or `None`
    /// for non-Stroke kinds.
    pub fn stroke_params(&self) -> Option<([f32; 5], u32)> {
        if self.kind != CmdKind::Stroke as u32 {
            return None;
        }
        Some((
            [self.slot0, self.slot1, self.slot2, self.slot3, f32::from_bits(self.slot5)],
            self.slot6 & 0xff,
        ))
    }

    /// Create a Path command from a pre-computed AABB + an index range
    /// into the shared `path_points` storage buffer.
    ///
    /// `point_count` must be in `2..=65535`. `width` is quantised to
    /// hundredths of a pixel; widths above ~655 px are clamped.
    ///
    /// The caller is responsible for uploading the actual points to the
    /// `path_points` buffer at indices `[point_offset, point_offset + point_count)`.
    pub fn path(
        bbox: [f32; 4],
        rgba: [u8; 4],
        width: f32,
        point_offset: u32,
        point_count: u32,
    ) -> Self {
        debug_assert!(point_count >= 2, "Path needs at least 2 points");
        let cnt    = point_count.min(0xffff) as u32;
        let width_q = ((width.max(0.0) * 100.0).round() as u32).min(0xffff);
        Self {
            kind: CmdKind::Path as u32,
            slot0: bbox[0], slot1: bbox[1], slot2: bbox[2], slot3: bbox[3],
            slot4: pack_rgba(rgba),
            slot5: width_q | (cnt << 16),
            slot6: point_offset,
        }
    }

    /// Dequantise Path parameters.
    ///
    /// Returns `Some((bbox, rgba, width, point_offset, point_count))`,
    /// or `None` for non-Path kinds.
    pub fn path_params(&self) -> Option<([f32; 4], u32, f32, u32, u32)> {
        if self.kind != CmdKind::Path as u32 {
            return None;
        }
        let width_q = self.slot5 & 0xffff;
        let count   = (self.slot5 >> 16) & 0xffff;
        let width   = width_q as f32 / 100.0;
        Some((
            [self.slot0, self.slot1, self.slot2, self.slot3],
            self.slot4,
            width,
            self.slot6,
            count,
        ))
    }

    /// Create a FillPath command (filled closed polygon interior).
    ///
    /// `point_count` is the number of vertices (range 3..=u32::MAX);
    /// the polygon implicitly closes from the last vertex back to the
    /// first. Non-zero winding rule is applied.
    pub fn fill_path(
        bbox: [f32; 4],
        rgba: [u8; 4],
        point_offset: u32,
        point_count: u32,
    ) -> Self {
        debug_assert!(point_count >= 3, "FillPath needs at least 3 vertices");
        Self {
            kind: CmdKind::FillPath as u32,
            slot0: bbox[0], slot1: bbox[1], slot2: bbox[2], slot3: bbox[3],
            slot4: pack_rgba(rgba),
            slot5: point_count,
            slot6: point_offset,
        }
    }

    /// Dequantise FillPath parameters.
    ///
    /// Returns `Some((bbox, rgba_packed, point_offset, point_count))`,
    /// or `None` for non-FillPath kinds.
    pub fn fill_path_params(&self) -> Option<([f32; 4], u32, u32, u32)> {
        if self.kind != CmdKind::FillPath as u32 {
            return None;
        }
        Some((
            [self.slot0, self.slot1, self.slot2, self.slot3],
            self.slot4,
            self.slot6,
            self.slot5,
        ))
    }

    /// Create a MultiLinGradient cmd from a pre-computed bbox + N stops
    /// already uploaded into `path_points` at `stop_offset..stop_offset+count`.
    ///
    /// Each stop occupies one `vec2<f32>` entry: `[position, bitcast(rgba)]`.
    /// The helper [`pack_gradient_stop`] does the bitcast for you.
    ///
    /// `direction` must be one of [`lin_dir::HORIZONTAL`],
    /// [`lin_dir::VERTICAL`], [`lin_dir::DIAGONAL_TLBR`], or
    /// [`lin_dir::DIAGONAL_BLTR`].
    pub fn multi_lin_gradient(
        bbox: [f32; 4],
        direction: u32,
        stop_offset: u32,
        stop_count: u32,
    ) -> Self {
        debug_assert!(stop_count >= 2, "MultiLinGradient needs at least 2 stops");
        Self {
            kind: CmdKind::MultiLinGradient as u32,
            slot0: bbox[0], slot1: bbox[1], slot2: bbox[2], slot3: bbox[3],
            slot4: direction,
            slot5: stop_count,
            slot6: stop_offset,
        }
    }

    /// Dequantise MultiLinGradient parameters.
    ///
    /// Returns `Some((bbox, direction, stop_offset, stop_count))`, or
    /// `None` for non-MultiLinGradient kinds.
    pub fn multi_lin_gradient_params(&self) -> Option<([f32; 4], u32, u32, u32)> {
        if self.kind != CmdKind::MultiLinGradient as u32 {
            return None;
        }
        Some((
            [self.slot0, self.slot1, self.slot2, self.slot3],
            self.slot4,
            self.slot6,
            self.slot5,
        ))
    }
}

impl SceneCmd {
    /// Create an Image command sampling from the bound RGBA8 atlas.
    ///
    /// - `bbox`    = screen-space rect to fill.
    /// - `tint`    = RGBA modulation (multiplied with the atlas texel).
    ///   Pass `[255, 255, 255, 255]` for the unmodulated image.
    /// - `uv_rect` = `[u0, v0, u1, v1]` in normalised atlas coords
    ///   `[0.0, 1.0]`. Internally quantised to u16 per component.
    pub fn image(
        x0: f32, y0: f32, x1: f32, y1: f32,
        tint: [u8; 4],
        uv_rect: [f32; 4],
    ) -> Self {
        let quant = |v: f32| -> u32 { (v.clamp(0.0, 1.0) * 65535.0).round() as u32 };
        let u0q = quant(uv_rect[0]);
        let v0q = quant(uv_rect[1]);
        let u1q = quant(uv_rect[2]);
        let v1q = quant(uv_rect[3]);
        Self {
            kind: CmdKind::Image as u32,
            slot0: x0, slot1: y0, slot2: x1, slot3: y1,
            slot4: pack_rgba(tint),
            slot5: u0q | (v0q << 16),
            slot6: u1q | (v1q << 16),
        }
    }

    /// Dequantise atlas UV rect from an Image command.
    ///
    /// Returns `Some([u0, v0, u1, v1])` if `kind == Image`, `None` otherwise.
    pub fn image_uv(&self) -> Option<[f32; 4]> {
        if self.kind != CmdKind::Image as u32 {
            return None;
        }
        let dequant = |q: u32| -> f32 { (q & 0xffff) as f32 / 65535.0 };
        Some([
            dequant(self.slot5),
            dequant(self.slot5 >> 16),
            dequant(self.slot6),
            dequant(self.slot6 >> 16),
        ])
    }
}

/// Pack a `(position, [r, g, b, a])` stop into the `[f32; 2]` form the
/// `path_points` buffer expects for `MultiLinGradient`.
///
/// The colour is `bytemuck::cast`-style stored in the `y` slot via
/// `f32::from_bits(packed_rgba)`; the shader extracts it with a
/// `bitcast<u32>(y)`.
pub fn pack_gradient_stop(position: f32, rgba: [u8; 4]) -> [f32; 2] {
    let packed = (rgba[0] as u32)
        | ((rgba[1] as u32) << 8)
        | ((rgba[2] as u32) << 16)
        | ((rgba[3] as u32) << 24);
    [position, f32::from_bits(packed)]
}

// Compile-time size assertion.
const _: () = assert!(std::mem::size_of::<SceneCmd>() == 32);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_bbox_passes_through() {
        let r = SceneCmd::rect(10.0, 20.0, 30.0, 40.0, [255, 0, 0, 255]);
        assert_eq!(r.bbox(), [10.0, 20.0, 30.0, 40.0]);
    }

    #[test]
    fn stroke_bbox_inflates_by_half_width() {
        // Horizontal line from (10,50) to (90,50) width=8 → bbox should
        // inflate by 4 on each side: [6, 46, 94, 54].
        let s = SceneCmd::stroke(10.0, 50.0, 90.0, 50.0, 8.0, [0, 0, 0, 255], cap_kind::BUTT);
        assert_eq!(s.bbox(), [6.0, 46.0, 94.0, 54.0]);
    }

    #[test]
    fn stroke_bbox_handles_reverse_endpoints() {
        // p1.x < p0.x — bbox must still be normalised.
        let s = SceneCmd::stroke(90.0, 90.0, 10.0, 10.0, 4.0, [0, 0, 0, 255], cap_kind::ROUND);
        assert_eq!(s.bbox(), [8.0, 8.0, 92.0, 92.0]);
    }

    #[test]
    fn stroke_params_round_trip_width_and_cap() {
        let s = SceneCmd::stroke(1.0, 2.0, 3.0, 4.0, 5.5, [10, 20, 30, 40], cap_kind::SQUARE);
        let (xs, cap) = s.stroke_params().expect("Stroke kind");
        assert_eq!(xs, [1.0, 2.0, 3.0, 4.0, 5.5]);
        assert_eq!(cap, cap_kind::SQUARE);
    }

    #[test]
    fn stroke_params_returns_none_for_other_kinds() {
        let r = SceneCmd::rect(0.0, 0.0, 10.0, 10.0, [0, 0, 0, 255]);
        assert!(r.stroke_params().is_none());
    }

    #[test]
    fn path_params_round_trip() {
        let p = SceneCmd::path(
            [10.0, 20.0, 80.0, 60.0], [255, 128, 64, 200],
            2.5, 42, 17,
        );
        assert_eq!(p.kind, CmdKind::Path as u32);
        assert_eq!(p.bbox(), [10.0, 20.0, 80.0, 60.0]);
        let (bbox, rgba, width, offset, count) = p.path_params().unwrap();
        assert_eq!(bbox, [10.0, 20.0, 80.0, 60.0]);
        assert_eq!(rgba & 0xff,           255); // r
        assert_eq!((rgba >>  8) & 0xff,   128); // g
        assert_eq!((rgba >> 16) & 0xff,    64); // b
        assert_eq!((rgba >> 24) & 0xff,   200); // a
        assert!((width - 2.5).abs() < 0.01);
        assert_eq!(offset, 42);
        assert_eq!(count,  17);
    }

    #[test]
    fn path_clamps_count_and_width() {
        let p = SceneCmd::path([0.0; 4], [0; 4], 700.0, 0, 100_000);
        let (_, _, width, _, count) = p.path_params().unwrap();
        // Width clamped at 655.35 px (u16 / 100); count capped at 65535.
        assert!(width >= 655.0 && width <= 656.0);
        assert_eq!(count, 65535);
    }

    #[test]
    fn path_params_returns_none_for_other_kinds() {
        let r = SceneCmd::rect(0.0, 0.0, 10.0, 10.0, [0, 0, 0, 255]);
        assert!(r.path_params().is_none());
    }

    #[test]
    fn fill_path_params_round_trip() {
        let p = SceneCmd::fill_path([0.0, 0.0, 100.0, 80.0], [10, 20, 30, 255], 7, 64);
        assert_eq!(p.kind, CmdKind::FillPath as u32);
        let (bbox, rgba, offset, count) = p.fill_path_params().unwrap();
        assert_eq!(bbox,   [0.0, 0.0, 100.0, 80.0]);
        assert_eq!(rgba & 0xff, 10);
        assert_eq!(offset, 7);
        assert_eq!(count,  64);
    }

    #[test]
    fn fill_path_params_returns_none_for_other_kinds() {
        let r = SceneCmd::rect(0.0, 0.0, 10.0, 10.0, [0, 0, 0, 255]);
        assert!(r.fill_path_params().is_none());
        let path = SceneCmd::path([0.0; 4], [0; 4], 1.0, 0, 2);
        assert!(path.fill_path_params().is_none());
    }

    #[test]
    fn multi_lin_gradient_params_round_trip() {
        let g = SceneCmd::multi_lin_gradient(
            [0.0, 0.0, 100.0, 50.0],
            lin_dir::HORIZONTAL,
            7,
            5,
        );
        assert_eq!(g.kind, CmdKind::MultiLinGradient as u32);
        let (bbox, dir, off, n) = g.multi_lin_gradient_params().unwrap();
        assert_eq!(bbox, [0.0, 0.0, 100.0, 50.0]);
        assert_eq!(dir,  lin_dir::HORIZONTAL);
        assert_eq!(off,  7);
        assert_eq!(n,    5);
    }

    #[test]
    fn image_uv_round_trips_within_quantisation_error() {
        let img = SceneCmd::image(
            10.0, 20.0, 100.0, 80.0,
            [255, 200, 100, 255],
            [0.0, 0.0, 1.0, 1.0],
        );
        assert_eq!(img.kind, CmdKind::Image as u32);
        assert_eq!(img.bbox(), [10.0, 20.0, 100.0, 80.0]);
        let uv = img.image_uv().unwrap();
        assert!((uv[0] - 0.0).abs() < 1e-4);
        assert!((uv[1] - 0.0).abs() < 1e-4);
        assert!((uv[2] - 1.0).abs() < 1e-4);
        assert!((uv[3] - 1.0).abs() < 1e-4);
    }

    #[test]
    fn image_uv_returns_none_for_other_kinds() {
        let r = SceneCmd::rect(0.0, 0.0, 10.0, 10.0, [0, 0, 0, 255]);
        assert!(r.image_uv().is_none());
        let g = SceneCmd::glyph(0.0, 0.0, 10.0, 10.0, [0; 4], [0.0; 4]);
        assert!(g.image_uv().is_none());
    }

    #[test]
    fn pack_gradient_stop_round_trips_color() {
        let s = pack_gradient_stop(0.5, [200, 100, 50, 255]);
        assert!((s[0] - 0.5).abs() < 1e-6);
        let bits = s[1].to_bits();
        assert_eq!( bits        & 0xff, 200);
        assert_eq!((bits >>  8) & 0xff, 100);
        assert_eq!((bits >> 16) & 0xff,  50);
        assert_eq!((bits >> 24) & 0xff, 255);
    }
}
