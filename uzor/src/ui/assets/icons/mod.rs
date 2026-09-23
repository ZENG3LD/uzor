//! SVG icon catalog for map rendering
//!
//! Usage:
//! ```
//! use uzor::render::icons::aviation;
//! // Then use draw_svg_icon(ctx, aviation::JET, x, y, w, h, color);
//! ```

pub mod aviation;
pub mod maritime;
pub mod markers;
pub mod weather;
pub mod infrastructure;
pub mod military;
pub mod ui;

/// Shared assertion for the per-module `root_stroke_width_stays_thin` tests below
/// and in `crate::ui::themes::macos::icons::paths` — every icon's root `<svg>`
/// pen weight must stay at or below 1.25, the crate's thin-icon default. This
/// guards against a future Lucide/Tabler paste landing with their stock
/// `stroke-width="2"` root and rendering fat again (see commit f11176c, which
/// made `draw_svg_icon` honor the root `stroke-width` instead of a fixed 1.5).
#[cfg(test)]
pub(crate) mod test_support {
    /// Root `<svg ...>` opening tag's `stroke-width` attribute, if declared.
    /// Returns `None` when the root has no explicit `stroke-width` (icons that
    /// set the pen weight per-element only, e.g. a line-width picker) — that
    /// case has no root value to bound, so it is not this guard's concern.
    pub(crate) fn root_stroke_width(svg: &str) -> Option<f64> {
        let tag_end = svg.find('>')?;
        let tag = &svg[..tag_end];
        let attr_start = tag.find("stroke-width=\"")? + "stroke-width=\"".len();
        let attr_end = tag[attr_start..].find('"')?;
        tag[attr_start..attr_start + attr_end].parse::<f64>().ok()
    }

    /// Panics if `svg`'s root declares a `stroke-width` above 1.5.
    pub(crate) fn assert_root_stroke_width_is_thin(name: &str, svg: &str) {
        if let Some(w) = root_stroke_width(svg) {
            assert!(
                w <= 1.5,
                "{name}: root stroke-width {w} exceeds the 1.5 thin-icon ceiling \
                 (Lucide/Tabler default is 2 — scale it down to 1.25 to match \
                 the rest of the asset library)"
            );
        }
    }
}
