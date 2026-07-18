//! [`build_morph`]/[`sample`] — Phase 3 kinetics: the resize-morph case
//! (same paragraph, two different `max_width`s) linearly interpolated
//! per glyph.
//!
//! ## Identity match
//!
//! The design doc's §4 Phase 3 names the rule as "identity-match by
//! (run_index, byte-offset-within-run) since content is identical, only
//! position changes." This module implements that rule's *intent*
//! (position-in-the-run identity, width-independent) via a scheme that
//! isn't literally "sum `cluster.len()` in byte order" — see the
//! divergence note below for why the literal reading has a cascade bug —
//! but is equivalent for every glyph that both layouts actually share.
//! [`glyph_keys`] walks each layout's `glyphs` in order and, per
//! `run_index`, tags every **non-whitespace** cluster with its own 1-based
//! ordinal among that run's non-whitespace clusters ("word ordinal"), and
//! every **whitespace** cluster with `(word ordinal of the last
//! non-whitespace cluster seen, 1-based position within this whitespace
//! run)`. Word ordinals never change with `max_width` — no non-whitespace
//! cluster is ever dropped by wrapping (see divergence note) — so this
//! key is exactly as width-independent as the doc intends, and reduces
//! the mismatch-detection surface to only the whitespace clusters that
//! can actually differ.
//!
//! ## Divergence from the design doc: literal byte-offset accumulation
//! cascades, so this module doesn't use it
//!
//! `crate::layout::greedy::pack_lines` trims leading/trailing whitespace
//! atoms at each line's wrap boundary, and *which* whitespace atoms get
//! trimmed depends on where the lines happen to break — which differs
//! between two different `max_width`s. Naively keying every glyph by
//! `(run_index, cumulative cluster.len() summed over the glyphs actually
//! present in this layout)` breaks the moment two layouts disagree on
//! even one trimmed whitespace cluster: every glyph *after* that point in
//! the run then accumulates a different running total in the two
//! layouts, so the key mismatches for the rest of the run, not just the
//! one trimmed cluster (confirmed empirically — the first implementation
//! of this module used literal byte-offset accumulation and produced a
//! visibly garbled second half of the resize-morph demo's proof PNG at
//! `t = 1.0`, glyphs of different words glued together). The word-ordinal
//! scheme above is immune to this: dropping a whitespace cluster only
//! ever desyncs *that one gap's own key*, never anything after it, because
//! word ordinals are computed purely from a count of non-whitespace
//! clusters, which wrapping never drops.
//!
//! The doc's own Phase 3 note says the resize-morph case needs "no fade
//! ... nothing appears/disappears" — true for the vast majority of
//! glyphs (every non-whitespace cluster, plus every whitespace cluster
//! that isn't sitting exactly on a wrap boundary) but not literally all
//! of them, for the reason above. Rather than treat the boundary case as
//! a panic or a silent drop, this phase reuses the
//! [`GlyphState::opacity`] field the doc's own sketch already declares:
//! an unmatched glyph fades in/out at its own fixed position instead of
//! flying to/from nowhere.
//!
//! This same fade mechanism is also what keeps [`build_morph`] from ever
//! panicking on two layouts of genuinely *different* text (see
//! `build_morph_on_mismatched_text_does_not_panic` below) — every glyph
//! in `from` with no `to` partner fades out, every glyph in `to` with no
//! `from` partner fades in. This is a graceful degradation, not the
//! general fuzzy-match-and-reflow PowerPoint-Morph engine the doc
//! explicitly defers past Phase 3.

use std::collections::{HashMap, HashSet};

use uzor::ui::animation::math::timeline::Animatable;

use crate::layout::{GlyphLayout, ParagraphLayout};

use super::GlyphState;

/// A prepared, sampleable transition between two [`ParagraphLayout`]s of
/// the same logical paragraph (Phase 3 scope: same runs/text, different
/// `max_width`).
///
/// Pure data (design law 3: stateless layout over borrowed/owned spans)
/// — [`build_morph`] computes this once; [`sample`]/[`sample_layout`]
/// read it any number of times at any `t`.
#[derive(Debug, Clone)]
pub struct MorphTransition {
    from: ParagraphLayout,
    to: ParagraphLayout,
    /// `(from_glyph_index, to_glyph_index)` pairs sharing one identity key.
    matched: Vec<(usize, usize)>,
    /// Indices into `from.glyphs` with no partner in `to` — fades out.
    from_only: Vec<usize>,
    /// Indices into `to.glyphs` with no partner in `from` — fades in.
    to_only: Vec<usize>,
}

impl MorphTransition {
    /// Number of glyphs present in both `from` and `to` under the same
    /// identity key.
    pub fn matched_count(&self) -> usize {
        self.matched.len()
    }

    /// Number of `from`-side glyphs with no `to`-side partner (fade out
    /// toward `t = 1.0`).
    pub fn from_only_count(&self) -> usize {
        self.from_only.len()
    }

    /// Number of `to`-side glyphs with no `from`-side partner (fade in
    /// toward `t = 1.0`).
    pub fn to_only_count(&self) -> usize {
        self.to_only.len()
    }
}

/// `(run_index, word_ordinal, gap_sub_index)` identity key for every
/// glyph in `layout`, in `layout.glyphs` order — see this module's doc
/// comment for why this shape (word-ordinal-anchored, not raw byte
/// accumulation) is the width-independent, non-cascading choice.
///
/// `word_ordinal` is the 1-based ordinal of the last non-whitespace
/// cluster seen in this run (0 before the run's first word). A
/// non-whitespace glyph's own key uses its *own* (just-incremented)
/// ordinal with `gap_sub_index = 0`; a whitespace glyph's key uses the
/// *preceding* word's ordinal with `gap_sub_index` counting `1, 2, ...`
/// across a run of consecutive whitespace clusters (reset to `0` by the
/// next non-whitespace glyph).
fn glyph_keys(layout: &ParagraphLayout) -> Vec<(usize, usize, usize)> {
    let mut word_ordinal: HashMap<usize, usize> = HashMap::new();
    let mut gap_index: HashMap<usize, usize> = HashMap::new();

    layout
        .glyphs
        .iter()
        .map(|g| {
            if is_whitespace_cluster(&g.cluster) {
                let word = *word_ordinal.get(&g.run_index).unwrap_or(&0);
                let gap = gap_index.entry(g.run_index).or_insert(0);
                *gap += 1;
                (g.run_index, word, *gap)
            } else {
                let word = word_ordinal.entry(g.run_index).or_insert(0);
                *word += 1;
                gap_index.insert(g.run_index, 0);
                (g.run_index, *word, 0)
            }
        })
        .collect()
}

/// `true` for a non-empty cluster made entirely of whitespace characters
/// — mirrors `crate::layout::greedy`'s own (private) whitespace test.
fn is_whitespace_cluster(cluster: &str) -> bool {
    !cluster.is_empty() && cluster.chars().all(char::is_whitespace)
}

/// Build a [`MorphTransition`] between `from` and `to` — see this
/// module's doc comment for the identity-match rule and the mismatch
/// fallback. Never panics, regardless of how unrelated `from`/`to` are
/// (no fallible API surface on the hot path — design doc §3.2).
pub fn build_morph(from: &ParagraphLayout, to: &ParagraphLayout) -> MorphTransition {
    let from_keys = glyph_keys(from);
    let to_keys = glyph_keys(to);

    let mut to_index_by_key: HashMap<(usize, usize, usize), usize> = HashMap::with_capacity(to_keys.len());
    for (idx, key) in to_keys.iter().enumerate() {
        to_index_by_key.insert(*key, idx);
    }

    let mut matched = Vec::new();
    let mut from_only = Vec::new();
    let mut matched_to: HashSet<usize> = HashSet::new();

    for (from_idx, key) in from_keys.iter().enumerate() {
        match to_index_by_key.get(key) {
            Some(&to_idx) => {
                matched.push((from_idx, to_idx));
                matched_to.insert(to_idx);
            }
            None => from_only.push(from_idx),
        }
    }

    let to_only: Vec<usize> = (0..to.glyphs.len()).filter(|idx| !matched_to.contains(idx)).collect();

    MorphTransition { from: from.clone(), to: to.clone(), matched, from_only, to_only }
}

/// A glyph's at-rest (`t`-independent) kinetic state: full opacity, unit
/// scale, zero rotation, positioned exactly where `glyph` says.
fn at_rest(glyph: &GlyphLayout) -> GlyphState {
    GlyphState { ch: glyph.cluster.clone(), pos: (glyph.x, glyph.y), opacity: 1.0, scale: 1.0, rotation: 0.0 }
}

/// Sample `morph` at `t` (clamped to `[0, 1]`): matched glyphs linearly
/// interpolate position (opacity held at `1.0` throughout — the doc's
/// "no fade needed" case); unmatched glyphs fade in/out at their own
/// fixed position (see module doc). `t = 0.0` reproduces `morph`'s `from`
/// glyphs' positions exactly; `t = 1.0` reproduces its `to` glyphs'
/// positions exactly.
pub fn sample(morph: &MorphTransition, t: f64) -> Vec<GlyphState> {
    let t = t.clamp(0.0, 1.0);
    let mut states = Vec::with_capacity(morph.matched.len() + morph.from_only.len() + morph.to_only.len());

    for &(from_idx, to_idx) in &morph.matched {
        let from_state = at_rest(&morph.from.glyphs[from_idx]);
        let to_state = at_rest(&morph.to.glyphs[to_idx]);
        states.push(from_state.lerp(&to_state, t));
    }

    for &from_idx in &morph.from_only {
        let from_state = at_rest(&morph.from.glyphs[from_idx]);
        let mut to_state = from_state.clone();
        to_state.opacity = 0.0;
        states.push(from_state.lerp(&to_state, t));
    }

    for &to_idx in &morph.to_only {
        let to_state = at_rest(&morph.to.glyphs[to_idx]);
        let mut from_state = to_state.clone();
        from_state.opacity = 0.0;
        states.push(from_state.lerp(&to_state, t));
    }

    states
}

/// Convenience not in the design doc's own sketch: reconstitutes a full
/// [`ParagraphLayout`] at `t` so the sampled morph can paint through the
/// existing [`crate::draw::draw_paragraph`] primitive unchanged (design
/// law 6 — no new drawing primitive for text). [`GlyphState`] alone
/// carries no font/color/advance/width, so this pairs [`sample`]'s
/// interpolated `(x, y)`/`opacity` with each glyph's own (unchanging —
/// Phase 3 never touches font/color) metadata: matched glyphs borrow
/// `to`'s font/advance/width/line_index/run_index, `from_only`/`to_only`
/// glyphs borrow their own side's. Fade `opacity` is baked into the
/// glyph's color alpha channel ([`GlyphLayout::color`]'s existing
/// `0xRRGGBBAA` packing — no new per-glyph paint field either), replacing
/// any run-level color/`None` with `base_rgb` at the sampled alpha.
pub fn sample_layout(morph: &MorphTransition, t: f64, base_rgb: u32) -> ParagraphLayout {
    let t = t.clamp(0.0, 1.0);
    let base_rgb = base_rgb & 0xffff_ff00;

    let mut glyphs = Vec::with_capacity(morph.matched.len() + morph.from_only.len() + morph.to_only.len());

    for &(from_idx, to_idx) in &morph.matched {
        let from_glyph = &morph.from.glyphs[from_idx];
        let to_glyph = &morph.to.glyphs[to_idx];
        let sampled = at_rest(from_glyph).lerp(&at_rest(to_glyph), t);
        glyphs.push(faded_glyph(to_glyph, sampled, base_rgb));
    }
    for &from_idx in &morph.from_only {
        let from_glyph = &morph.from.glyphs[from_idx];
        let from_state = at_rest(from_glyph);
        let mut to_state = from_state.clone();
        to_state.opacity = 0.0;
        let sampled = from_state.lerp(&to_state, t);
        glyphs.push(faded_glyph(from_glyph, sampled, base_rgb));
    }
    for &to_idx in &morph.to_only {
        let to_glyph = &morph.to.glyphs[to_idx];
        let to_state = at_rest(to_glyph);
        let mut from_state = to_state.clone();
        from_state.opacity = 0.0;
        let sampled = from_state.lerp(&to_state, t);
        glyphs.push(faded_glyph(to_glyph, sampled, base_rgb));
    }

    let width = morph.from.width.lerp(&morph.to.width, t);
    let height = morph.from.height.lerp(&morph.to.height, t);

    // Typography-gap WAVE 2: decoration spans are NOT sampled through the
    // morph — `GlyphState` carries no decoration/run-index-stable identity
    // to interpolate a span's own x-extent against (the SAME "no fade for
    // spans" gap this module's own `lines`/`boxes: Vec::new()` above
    // already documents for line/box geometry), a documented, minor scope
    // limit rather than a silent drop of a feature this phase never wired.
    ParagraphLayout { glyphs, lines: Vec::new(), boxes: Vec::new(), decorations: Vec::new(), width, height }
}

/// Clone `source`'s non-positional fields onto `sampled`'s interpolated
/// position, packing `sampled.opacity` into the alpha byte of `base_rgb`.
fn faded_glyph(source: &GlyphLayout, sampled: GlyphState, base_rgb: u32) -> GlyphLayout {
    let alpha = (sampled.opacity.clamp(0.0, 1.0) * 255.0).round() as u32;
    GlyphLayout {
        cluster: sampled.ch,
        run_index: source.run_index,
        line_index: source.line_index,
        x: sampled.pos.0,
        y: sampled.pos.1,
        advance: source.advance,
        width: source.width,
        font: source.font,
        color: Some(base_rgb | alpha),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use uzor::fonts::FontFamily;
    use uzor_export::{render_to_png, ExportSpec};

    use super::*;
    use crate::layout::layout_text;
    use crate::model::FontSpec;
    use crate::shape::CosmicShaper;

    const LONG_SENTENCE: &str = "The quick brown fox jumps over the lazy dog \
        and then keeps running further down the road without stopping for a \
        very long time indeed";

    /// Two widths that both exceed the fixture's natural (unwrapped)
    /// extent produce byte-identical single-line layouts — the doc's own
    /// literal Phase 3 gate ("preserves glyph count ... every glyph has a
    /// match, no `None`s") in its cleanest form: zero unmatched glyphs on
    /// either side.
    #[test]
    fn build_morph_between_two_unwrapped_widths_matches_every_glyph() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let shaper = CosmicShaper::headless();

        let from = layout_text(LONG_SENTENCE, &font, 4000.0, &shaper);
        let to = layout_text(LONG_SENTENCE, &font, 6000.0, &shaper);
        assert_eq!(from.lines.len(), 1);
        assert_eq!(to.lines.len(), 1);

        let morph = build_morph(&from, &to);
        assert_eq!(morph.matched_count(), from.glyphs.len());
        assert_eq!(morph.matched_count(), to.glyphs.len());
        assert_eq!(morph.from_only_count(), 0, "no glyph should be from-only when nothing wraps differently");
        assert_eq!(morph.to_only_count(), 0, "no glyph should be to-only when nothing wraps differently");
    }

    /// `sample(0)` reproduces every matched glyph's `from` position
    /// exactly; `sample(1)` reproduces its `to` position exactly — holds
    /// even when `from`/`to` wrap to a different number of lines (the
    /// realistic resize-morph case), matched glyphs included.
    #[test]
    fn sample_at_zero_and_one_reproduces_the_endpoints_exactly() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let shaper = CosmicShaper::headless();

        let from = layout_text(LONG_SENTENCE, &font, 150.0, &shaper);
        let to = layout_text(LONG_SENTENCE, &font, 550.0, &shaper);
        assert!(from.lines.len() > to.lines.len(), "fixture must actually rewrap between the two widths");

        let morph = build_morph(&from, &to);
        assert!(morph.matched_count() > 0);

        let at0 = sample(&morph, 0.0);
        let at1 = sample(&morph, 1.0);

        for (i, &(from_idx, _)) in morph_matched(&morph).iter().enumerate() {
            let expected = &from.glyphs[from_idx];
            assert_eq!(at0[i].pos, (expected.x, expected.y));
            assert_eq!(at0[i].opacity, 1.0);
        }
        for (i, &(_, to_idx)) in morph_matched(&morph).iter().enumerate() {
            let expected = &to.glyphs[to_idx];
            assert_eq!(at1[i].pos, (expected.x, expected.y));
            assert_eq!(at1[i].opacity, 1.0);
        }
    }

    /// Exposes `MorphTransition::matched` to this test module only,
    /// without making the field itself `pub(crate)` on the real struct —
    /// rebuilt here from the public counts + `sample`'s own known
    /// ordering (matched pairs first, in `from` order) would be fragile,
    /// so instead re-derive matched pairs the same way `build_morph`
    /// does, purely for test assertions.
    fn morph_matched(morph: &MorphTransition) -> Vec<(usize, usize)> {
        let from_keys = glyph_keys(&morph.from);
        let to_keys = glyph_keys(&morph.to);
        let mut to_index_by_key: HashMap<(usize, usize, usize), usize> = HashMap::with_capacity(to_keys.len());
        for (idx, key) in to_keys.iter().enumerate() {
            to_index_by_key.insert(*key, idx);
        }
        from_keys
            .iter()
            .enumerate()
            .filter_map(|(from_idx, key)| to_index_by_key.get(key).map(|&to_idx| (from_idx, to_idx)))
            .collect()
    }

    /// At the midpoint, every matched glyph's position lies within the
    /// convex hull of its two endpoints (never outside, never NaN), and
    /// at least some glyphs actually moved (fixture rewraps, so this
    /// isn't a vacuous check). Unmatched glyphs sit exactly halfway
    /// between full and zero opacity.
    #[test]
    fn sample_at_half_stays_between_the_endpoints_and_never_produces_nan() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let shaper = CosmicShaper::headless();

        let from = layout_text(LONG_SENTENCE, &font, 150.0, &shaper);
        let to = layout_text(LONG_SENTENCE, &font, 550.0, &shaper);
        let morph = build_morph(&from, &to);

        let mid = sample(&morph, 0.5);
        assert_eq!(mid.len(), morph.matched_count() + morph.from_only_count() + morph.to_only_count());

        let mut moved_count = 0;
        for state in &mid {
            assert!(state.pos.0.is_finite() && state.pos.1.is_finite(), "no NaN/inf in sampled position");
            assert!((0.0..=1.0).contains(&state.opacity), "opacity must stay in [0,1], got {}", state.opacity);
        }

        for (idx, &(from_idx, to_idx)) in morph_matched(&morph).iter().enumerate() {
            let a = &from.glyphs[from_idx];
            let b = &to.glyphs[to_idx];
            let sampled_pos = mid[idx].pos;
            let (lo_x, hi_x) = (a.x.min(b.x), a.x.max(b.x));
            let (lo_y, hi_y) = (a.y.min(b.y), a.y.max(b.y));
            assert!(sampled_pos.0 >= lo_x - 1e-6 && sampled_pos.0 <= hi_x + 1e-6);
            assert!(sampled_pos.1 >= lo_y - 1e-6 && sampled_pos.1 <= hi_y + 1e-6);
            if (a.x - b.x).abs() > 1e-6 || (a.y - b.y).abs() > 1e-6 {
                moved_count += 1;
            }
        }
        assert!(moved_count > 0, "fixture must produce at least some real glyph movement, not just a static fixture");
    }

    /// `build_morph` on two layouts of *completely different* text must
    /// not panic — every `from` glyph becomes `from_only` (fades out),
    /// every `to` glyph becomes `to_only` (fades in). This is the
    /// graceful fallback this phase implements instead of building the
    /// design doc's explicitly-deferred general fuzzy-match engine.
    #[test]
    fn build_morph_on_mismatched_text_does_not_panic() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let shaper = CosmicShaper::headless();

        let from = layout_text("Hello world, a short greeting.", &font, 1000.0, &shaper);
        let to = layout_text(
            "A totally different, unrelated sentence about something else entirely.",
            &font,
            1000.0,
            &shaper,
        );

        let morph = build_morph(&from, &to);
        assert_eq!(morph.matched_count() + morph.from_only_count(), from.glyphs.len());
        assert_eq!(morph.matched_count() + morph.to_only_count(), to.glyphs.len());

        let at0 = sample(&morph, 0.0);
        let at1 = sample(&morph, 1.0);
        let mid = sample(&morph, 0.5);
        assert_eq!(at0.len(), mid.len());
        assert_eq!(at1.len(), mid.len());
        for state in mid.iter().chain(at0.iter()).chain(at1.iter()) {
            assert!(state.pos.0.is_finite() && state.pos.1.is_finite());
            assert!((0.0..=1.0).contains(&state.opacity));
        }
    }

    /// `sample_layout` reconstitutes a paintable [`ParagraphLayout`] —
    /// same glyph count as `sample`, valid `width`/`height`, no NaN.
    #[test]
    fn sample_layout_produces_a_valid_layout_with_matching_glyph_count() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let shaper = CosmicShaper::headless();

        let from = layout_text(LONG_SENTENCE, &font, 150.0, &shaper);
        let to = layout_text(LONG_SENTENCE, &font, 550.0, &shaper);
        let morph = build_morph(&from, &to);

        let mid_layout = sample_layout(&morph, 0.5, 0x111111ff);
        assert_eq!(mid_layout.glyphs.len(), morph.matched_count() + morph.from_only_count() + morph.to_only_count());
        assert!(mid_layout.width.is_finite() && mid_layout.height.is_finite());
        for g in &mid_layout.glyphs {
            assert!(g.x.is_finite() && g.y.is_finite());
            assert!(g.color.is_some(), "sample_layout must always bake an explicit color for fade support");
        }
    }

    // ── Headless proof strip (design law 8: one screenshot is never proof) ──

    const WIDTH: u32 = 600;
    const HEIGHT: u32 = 400;
    const MARGIN: f64 = 20.0;
    const NARROW_WIDTH: f64 = 160.0;
    const WIDE_WIDTH: f64 = 560.0;

    /// Fixed seeded sample paragraph — no lorem-ipsum RNG (design law 8).
    const PROOF_PARAGRAPH: &str = "The quick brown fox jumps over the lazy dog and \
        then keeps running further down the road without stopping, a fixed seeded \
        sample paragraph morphing between a narrow and a wide column for every \
        uzor-text Phase 3 kinetics proof render.";

    fn out_dir() -> PathBuf {
        // Fixed path — `uzor/out/` is the shared human-eyeball drop point
        // for every headless proof render in this workspace (matches
        // `crate::draw`'s own proof tests).
        PathBuf::from(r"C:\Users\VA PC\CODING\ML_TRADING\nemo\uzor\out")
    }

    fn write_proof_png(name: &str, bytes: &[u8]) {
        let dir = out_dir();
        std::fs::create_dir_all(&dir).expect("create uzor/out/ proof directory");
        std::fs::write(dir.join(name), bytes).expect("write proof PNG");
    }

    fn decoded_png_dims(bytes: &[u8]) -> (u32, u32) {
        let decoder = png::Decoder::new(bytes);
        let reader = decoder.read_info().expect("valid PNG header");
        let info = reader.info();
        (info.width, info.height)
    }

    /// Renders `t = 0.0 / 0.5 / 1.0` of a narrow<->wide resize-morph to
    /// `uzor/out/text_p3_morph_t{0,05,1}.png` via the exact same
    /// `sample_layout` + `draw_paragraph` path the demo uses.
    #[test]
    fn morph_proof_strip_renders_t0_t05_t1_to_valid_pngs() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let shaper = CosmicShaper::headless();

        let narrow = layout_text(PROOF_PARAGRAPH, &font, NARROW_WIDTH, &shaper);
        let wide = layout_text(PROOF_PARAGRAPH, &font, WIDE_WIDTH, &shaper);
        assert!(narrow.lines.len() > wide.lines.len(), "fixture must actually rewrap between the two widths");

        let morph = build_morph(&narrow, &wide);

        let spec = ExportSpec { width_px: WIDTH, height_px: HEIGHT, dpr: 1.0, background: Some([255, 255, 255, 255]) };
        for (t, name) in [(0.0, "text_p3_morph_t0.png"), (0.5, "text_p3_morph_t05.png"), (1.0, "text_p3_morph_t1.png")] {
            let sampled = sample_layout(&morph, t, 0x111111_ff);
            let bytes = render_to_png(&spec, |ctx| {
                crate::draw::draw_paragraph(ctx, (MARGIN, MARGIN), &sampled, "#111111", false);
            })
            .unwrap_or_else(|e| panic!("morph proof render at t={t} should succeed: {e}"));

            assert_eq!(decoded_png_dims(&bytes), (WIDTH, HEIGHT));
            write_proof_png(name, &bytes);
        }
    }
}
