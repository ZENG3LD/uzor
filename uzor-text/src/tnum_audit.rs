//! Typography track T1 (2026-07-25): tabular-figures (`tnum`) measurement.
//!
//! **Test-only, zero production surface** — this module exists purely to
//! answer, empirically, a question this crate's own `CLAUDE.md`
//! (Typography-gap WAVE 2 §5) had already logged but never actually
//! checked: *"cosmic-text 0.12.1's public API has no seam to inject `tnum`
//! — but do the fonts this workspace actually SHIPS even need it?"* Per
//! this task's own brief ("measure before building... if already equal,
//! say so plainly and do not build anything; if they differ, report by how
//! much and stop there"), the answer below is a measurement, not new
//! machinery.
//!
//! Measures the raw `hmtx` advance width of every ASCII digit glyph
//! (`'0'..='9'`) directly from each embedded font FILE via [`ttf_parser`] —
//! deliberately bypassing `uzor::shaper`/cosmic-text's `FontFamily`
//! selection entirely, since `uzor::shaper::parse_css_font`'s own
//! `resolve_family` only recognizes 3 logical families (Roboto/PT Root
//! UI/JetBrains Mono) and has no seam to select a specific STYLE FILE
//! (Roboto Bold vs Italic vs BoldItalic) or DejaVu Sans at all (see
//! `uzor/src/ui/assets/fonts/fonts.rs`'s own `resolve_family` — "dejavu" is
//! not a recognized keyword, so a CSS family string naming it would
//! silently resolve to Roboto instead). Reading `hmtx` directly per FILE
//! sidesteps that gap and is also the more DIRECT answer to "does this
//! font's own default figures have equal advances" — the digit advance
//! recorded in `hmtx` is a per-glyph fact of the font file itself; a plain
//! per-codepoint shape (no ligature, since digits never ligate with each
//! other) never differs from it unless a `tnum`/`pnum`-class GSUB
//! substitution is explicitly requested (the exact feature cosmic-text
//! 0.12.1 has no seam to request), so reading it directly (once, per file)
//! answers the *default* (`tnum`-OFF) question exactly, at every render
//! size (an advance is a font-unit value scaled linearly by `size_px /
//! units_per_em` — the equal/unequal verdict is scale-invariant; measuring
//! it at 16px AND 32px below is a direct demonstration of that, not a
//! second independent fact).
//!
//! ## Verdict (measured, NOT assumed — the numbers are the point)
//!
//! **Mixed, and genuinely mixed — not a clean "non-issue across the
//! board."** Roboto (all 4 style files), DejaVu Sans, and JetBrains Mono
//! (the monospace control — tabular by definition, every glyph shares one
//! advance) all already have EQUAL-width default digits — `tnum` is a
//! real NON-ISSUE for any document set in one of those. **PT Root UI (the
//! variable font, default instance) does NOT** — its digit `'1'` measures
//! `350` font units against `550`-`610` for the others (upm 1000), a
//! ~30-43% narrower advance, a real ~4px gap at 16px / ~8px at 32px (see
//! `digit_width_audit_table`'s printed numbers for the exact per-digit
//! set). This is NOT a "user-supplied proportional-figure font" case —
//! PT Root UI is one of only 3 [`uzor::fonts::FontFamily`] variants
//! `FontSpec`/`uzor::shaper` can select at all, so any first-party document
//! that chooses PT Root UI for numeric/tabular content (financial tables,
//! KPI figures) inherits this gap TODAY. Per this task's own instruction
//! ("if they differ, report by how much and stop there — the fix is a
//! separate decision, not this task"), the number is reported and closed
//! here; no `tnum` plumbing is built by this pass.

#[cfg(test)]
mod tests {
    use ttf_parser::Face;

    /// Every embedded font FILE this task named — `(label, bytes)`. Exactly
    /// the 8 files the task brief enumerates (DejaVu Sans; Roboto Regular/
    /// Bold/Italic/BoldItalic; JetBrains Mono Regular/Bold; PT Root UI VF).
    fn embedded_fonts() -> [(&'static str, &'static [u8]); 8] {
        [
            ("DejaVuSans", uzor_fonts::DEJAVU_SANS),
            ("Roboto-Regular", uzor_fonts::ROBOTO_REGULAR),
            ("Roboto-Bold", uzor_fonts::ROBOTO_BOLD),
            ("Roboto-Italic", uzor_fonts::ROBOTO_ITALIC),
            ("Roboto-BoldItalic", uzor_fonts::ROBOTO_BOLD_ITALIC),
            ("JetBrainsMono-Regular", uzor_fonts::JETBRAINS_MONO_REGULAR),
            ("JetBrainsMono-Bold", uzor_fonts::JETBRAINS_MONO_BOLD),
            ("PTRootUI-VF", uzor_fonts::PT_ROOT_UI_VF),
        ]
    }

    /// `(raw hmtx advance per digit '0'..='9', units_per_em)` for one font
    /// file — default instance (no variation-axis coordinates applied),
    /// no shaping features involved (a plain `cmap` codepoint lookup +
    /// `hmtx` read).
    fn digit_advances(bytes: &[u8]) -> (Vec<u16>, u16) {
        let face = Face::parse(bytes, 0).expect("embedded font must parse");
        let upm = face.units_per_em();
        let advances: Vec<u16> = ('0'..='9')
            .map(|c| {
                let gid = face.glyph_index(c).unwrap_or_else(|| panic!("font must contain digit '{c}'"));
                face.glyph_hor_advance(gid).unwrap_or_else(|| panic!("digit '{c}' must have a horizontal advance"))
            })
            .collect();
        (advances, upm)
    }

    fn all_equal(advances: &[u16]) -> bool {
        advances.windows(2).all(|w| w[0] == w[1])
    }

    /// Scaled px advance at `size_px` — linear from the raw font-unit
    /// value, matching how every shaper in this workspace scales `hmtx`
    /// (no hinting-driven per-size rounding at this measurement layer).
    fn to_px(units: u16, upm: u16, size_px: f64) -> f64 {
        units as f64 * size_px / upm as f64
    }

    /// T1 measurement table: every embedded font's raw digit advances
    /// (font units) + scaled px advances at 16px and 32px, one line per
    /// font. Run with `--nocapture` to reproduce the table this task's own
    /// report/handoff quotes verbatim.
    #[test]
    fn digit_width_audit_table() {
        for (label, bytes) in embedded_fonts() {
            let (advances, upm) = digit_advances(bytes);
            let tabular = all_equal(&advances);
            let px16: Vec<f64> = advances.iter().map(|&u| (to_px(u, upm, 16.0) * 100.0).round() / 100.0).collect();
            let px32: Vec<f64> = advances.iter().map(|&u| (to_px(u, upm, 32.0) * 100.0).round() / 100.0).collect();
            println!("{label:22} upm={upm:5} tabular={tabular:5} units(0-9)={advances:?} px16={px16:?} px32={px32:?}");
        }
    }

    /// Control: JetBrains Mono is monospace, so EVERY glyph (not just
    /// digits) shares one advance — tabular by definition, both bundled
    /// weights. The ground truth this module's own methodology is checked
    /// against.
    #[test]
    fn jetbrains_mono_digits_are_tabular_by_definition_both_weights() {
        let (regular, _) = digit_advances(uzor_fonts::JETBRAINS_MONO_REGULAR);
        let (bold, _) = digit_advances(uzor_fonts::JETBRAINS_MONO_BOLD);
        assert!(all_equal(&regular), "JetBrains Mono Regular digits must be tabular (monospace)");
        assert!(all_equal(&bold), "JetBrains Mono Bold digits must be tabular (monospace)");
    }

    /// T1 verdict (non-issue half), locked in as a regression guard: Roboto
    /// (all 4 style files) and DejaVu Sans already have equal-width default
    /// digits — measured directly, never assumed. A future font swap that
    /// silently makes this false for any of them fails this test, forcing
    /// an explicit acknowledgment rather than a silent tabular-alignment
    /// regression in a first-party document that relies on it.
    #[test]
    fn roboto_and_dejavu_sans_already_have_tabular_default_digits() {
        let already_tabular = [
            ("DejaVuSans", uzor_fonts::DEJAVU_SANS),
            ("Roboto-Regular", uzor_fonts::ROBOTO_REGULAR),
            ("Roboto-Bold", uzor_fonts::ROBOTO_BOLD),
            ("Roboto-Italic", uzor_fonts::ROBOTO_ITALIC),
            ("Roboto-BoldItalic", uzor_fonts::ROBOTO_BOLD_ITALIC),
        ];
        for (label, bytes) in already_tabular {
            let (advances, _) = digit_advances(bytes);
            assert!(all_equal(&advances), "{label}: expected already-tabular default digits (T1 measurement), got {advances:?}");
        }
    }

    /// T1 verdict (real-gap half), locked in as a regression guard the
    /// OTHER direction: PT Root UI's variable-font default instance does
    /// NOT have equal-width digits — `'1'` (index 1) is measurably the
    /// narrowest, a genuine ~30%+ advance spread against the widest digit,
    /// not a rounding artifact. PT Root UI is one of only 3
    /// [`uzor::fonts::FontFamily`] variants this workspace's own shaper can
    /// select — this is a real, currently-live gap for first-party content
    /// choosing that family for numeric text, reported here per this
    /// task's own "report by how much and stop there" instruction (no
    /// `tnum` plumbing is built by this pass).
    #[test]
    fn pt_root_ui_vf_default_digits_are_not_tabular_a_real_measured_gap() {
        let (advances, upm) = digit_advances(uzor_fonts::PT_ROOT_UI_VF);
        assert!(!all_equal(&advances), "PT Root UI VF: expected a NON-tabular default (T1 measurement), got uniformly {advances:?}");

        let widest = *advances.iter().max().expect("10 digits");
        let narrowest = *advances.iter().min().expect("10 digits");
        assert_eq!(advances[1], narrowest, "digit '1' must be the narrowest — the classic proportional-figures tell, got advances={advances:?}");
        let spread_fraction = (widest - narrowest) as f64 / widest as f64;
        assert!(
            spread_fraction > 0.25,
            "the widest/narrowest digit spread must be a REAL gap (>25% of the widest digit's own advance), got {:.1}% (upm={upm})",
            spread_fraction * 100.0
        );
    }

    /// The size-invariance claim this module's own doc comment makes: the
    /// tabular/non-tabular VERDICT for a font must be identical whether
    /// read in raw font units or scaled to any render size (16px/32px) —
    /// proves measuring once in font units and reasoning about it at every
    /// size (rather than re-measuring per size) is sound, not an assumption.
    /// Exercised against BOTH the tabular fonts and the one non-tabular
    /// font (PT Root UI) — the invariance must hold either way.
    #[test]
    fn tabular_verdict_is_identical_across_render_sizes() {
        for (label, bytes) in embedded_fonts() {
            let (advances, upm) = digit_advances(bytes);
            let units_verdict = all_equal(&advances);
            let px16: Vec<f64> = advances.iter().map(|&u| to_px(u, upm, 16.0)).collect();
            let px32: Vec<f64> = advances.iter().map(|&u| to_px(u, upm, 32.0)).collect();
            let px16_verdict = px16.windows(2).all(|w| (w[0] - w[1]).abs() < 1e-9);
            let px32_verdict = px32.windows(2).all(|w| (w[0] - w[1]).abs() < 1e-9);
            assert_eq!(units_verdict, px16_verdict, "{label}: 16px verdict must match the font-unit verdict");
            assert_eq!(units_verdict, px32_verdict, "{label}: 32px verdict must match the font-unit verdict");
        }
    }
}
