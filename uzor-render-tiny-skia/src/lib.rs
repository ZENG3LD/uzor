//! CPU software rasterizer backend using `tiny-skia` + `fontdue`.
//!
//! Provides a pure-Rust, zero-GPU rendering context that implements the
//! [`uzor::render::RenderContext`] trait via [`TinySkiaCpuRenderContext`].

mod context;

pub use context::TinySkiaCpuRenderContext;

// ---------------------------------------------------------------------------
// Phase 5 text_to_path spot-checks
// ---------------------------------------------------------------------------

#[cfg(test)]
mod text_to_path_tests {
    use uzor::render::TextMetrics;
    use super::TinySkiaCpuRenderContext;

    fn ctx() -> TinySkiaCpuRenderContext {
        TinySkiaCpuRenderContext::new(1, 1, 1.0)
    }

    /// Non-empty input → non-empty path starting with "M".
    #[test]
    fn hello_returns_path_starting_with_m() {
        let d = ctx().text_to_path("HELLO", "bold 48px sans-serif");
        assert!(!d.is_empty(), "text_to_path('HELLO') should return non-empty path");
        assert!(d.starts_with('M'), "path should start with M, got: {:?}", &d[..d.len().min(20)]);
    }

    /// Empty text → empty string (no panic).
    #[test]
    fn empty_text_returns_empty_string() {
        let d = ctx().text_to_path("", "16px sans-serif");
        assert!(d.is_empty(), "empty text should return empty path");
    }

    /// Path must end with "Z" (all subpaths closed).
    #[test]
    fn path_ends_with_z() {
        let d = ctx().text_to_path("Hi", "16px sans-serif");
        assert!(!d.is_empty(), "non-empty input should produce a path");
        assert!(d.ends_with('Z'), "path should end with Z, got: {:?}", &d[d.len().saturating_sub(20)..]);
    }

    /// Cache deduplication: two calls return identical strings.
    #[test]
    fn cache_returns_same_string() {
        let c = ctx();
        let d1 = c.text_to_path("AB", "24px sans-serif");
        let d2 = c.text_to_path("AB", "24px sans-serif");
        assert_eq!(d1, d2, "cached call should return identical path");
    }

    /// Different inputs produce different paths.
    #[test]
    fn different_text_different_path() {
        let c = ctx();
        let d_a = c.text_to_path("A", "24px sans-serif");
        let d_b = c.text_to_path("B", "24px sans-serif");
        assert_ne!(d_a, d_b, "different glyphs should produce different paths");
    }
}

// ---------------------------------------------------------------------------
// Phase 4 cluster-shaping spot-checks
// ---------------------------------------------------------------------------

#[cfg(test)]
mod shaper_tests {
    use uzor::render::TextMetrics;
    use super::TinySkiaCpuRenderContext;

    fn ctx() -> TinySkiaCpuRenderContext {
        TinySkiaCpuRenderContext::new(1, 1, 1.0)
    }

    const FONT: &str = "16px sans-serif";

    /// "Hello" → 5 clusters, x_offset strictly increasing.
    #[test]
    fn hello_five_clusters() {
        let glyphs = ctx().measure_text_glyphs("Hello", FONT);
        assert_eq!(glyphs.len(), 5, "expected 5 clusters for 'Hello', got {:?}", glyphs);
        for (i, g) in glyphs.iter().enumerate() {
            let expected = ["H", "e", "l", "l", "o"][i];
            assert_eq!(g.cluster, expected, "cluster[{i}] mismatch");
        }
        // x_offsets must be non-decreasing
        for w in glyphs.windows(2) {
            assert!(
                w[1].x_offset >= w[0].x_offset,
                "x_offset not increasing: {:?} >= {:?}", w[0].x_offset, w[1].x_offset
            );
        }
    }

    /// "café" → 4 clusters, with 'é' as a single cluster (not 'e' + combining).
    #[test]
    fn cafe_four_clusters() {
        let glyphs = ctx().measure_text_glyphs("caf\u{00e9}", FONT);
        assert_eq!(glyphs.len(), 4, "expected 4 clusters for 'café', got {:?}", glyphs);
        assert_eq!(glyphs[3].cluster, "\u{00e9}", "last cluster should be é (U+00E9)");
    }

    /// "hi 👋" → 4 clusters: 'h', 'i', ' ', and the wave emoji as one cluster.
    #[test]
    fn emoji_one_cluster() {
        let text = "hi \u{1F44B}"; // U+1F44B = waving hand
        let glyphs = ctx().measure_text_glyphs(text, FONT);
        assert_eq!(glyphs.len(), 4, "expected 4 clusters for 'hi 👋', got {:?}", glyphs);
        assert_eq!(glyphs[3].cluster, "\u{1F44B}", "emoji should be one cluster");
    }

    /// Empty string → empty Vec.
    #[test]
    fn empty_string_empty_vec() {
        let glyphs = ctx().measure_text_glyphs("", FONT);
        assert!(glyphs.is_empty(), "empty string should return empty Vec");
    }
}
