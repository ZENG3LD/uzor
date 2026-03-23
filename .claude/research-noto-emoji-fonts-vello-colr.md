# Noto Emoji Fonts for Rust GPU Renderer — Research Report

**Date:** 2026-03-23
**Stack:** skrifa + peniko + vello + cosmic_text

---

## TL;DR — Recommendation

**Use `NotoColorEmoji.ttf` (CBDT bitmap, 10.2 MB) from the official googlefonts repo.**

Do NOT use `Noto-COLRv1.ttf`. cosmic_text (the shaping/layout layer) does not support COLR v1 as of March 2026, causing silent blank rendering with no fallback. `NotoColorEmoji.ttf` uses CBDT/CBLC bitmap tables which cosmic_text handles correctly.

Keep `NotoEmoji-Regular.ttf` (858 KB monochrome) as a slim fallback option if binary size is a hard constraint.

---

## Font Format Taxonomy (Critical)

The Google Noto project ships **three distinct formats** in the same repo:

| Filename | Format | Tables | Size | Renderer compat |
|---|---|---|---|---|
| `NotoColorEmoji.ttf` | CBDT/CBLC bitmaps | CBDT, CBLC | 10.2 MB | **Works** in cosmic_text, Android, Chrome |
| `Noto-COLRv1.ttf` | Vector COLRv1 only | COLR v1, CPAL | ~2–3 MB | **BROKEN** in cosmic_text (blank glyph, no fallback) |
| `NotoColorEmoji_WindowsCompatible.ttf` | CBDT/CBLC bitmaps | Same as main | 10.2 MB | Identical to main, Win10 tweak |

### Why `Noto-COLRv1.ttf` Is Broken in This Stack

cosmic_text's shaping pipeline does not implement the COLR v1 table reader. When it encounters a COLRv1-only font, it successfully locates the glyph (so cursor advance is correct), fails to decode the color layer tree, and draws nothing — producing invisible blank spaces with no error and no fallback. This was confirmed as an open, unresolved bug in cosmic-epoch issue #2546 (November 2025) and cosmic-text issue #310.

---

## Font 1: NotoColorEmoji.ttf (Color, CBDT Bitmaps)

### Download URL (Raw, direct)
```
https://github.com/googlefonts/noto-emoji/raw/refs/heads/main/fonts/NotoColorEmoji.ttf
```

### Versioned / Windows-compatible variant
```
https://github.com/googlefonts/noto-emoji/raw/refs/heads/main/fonts/NotoColorEmoji_WindowsCompatible.ttf
```

### Metadata
- **File size:** 10.2 MB
- **Format:** CBDT/CBLC (color bitmaps at multiple resolutions)
- **Coverage:** Full Unicode emoji set (Unicode 17.0 as of v2.051, Sep 2025)
- **License:** Apache 2.0
- **Latest release:** v2.051
- **Repository:** https://github.com/googlefonts/noto-emoji

### Embedding Considerations
- 10.2 MB is large for a binary-embedded asset. Consider shipping as a sidecar file loaded at runtime, or compressing with `include_bytes!` + decompression on first use.
- CBDT stores raster bitmaps at fixed resolutions (typically 128×128 px). Looks sharp at target size, pixelated if scaled far above that.
- No system font registration needed — can be loaded directly into cosmic_text's FontSystem via `font_system.db_mut().load_font_data(bytes)`.

---

## Font 2: NotoEmoji-Regular.ttf (Monochrome / Black Outline)

### Download URLs
Official Google Fonts / noto-emoji repo (static Regular weight):
```
https://github.com/googlefonts/noto-emoji/raw/refs/heads/main/fonts/NotoEmoji-Regular.ttf
```

AUR-mirrored source (confirmed working, v1.1.0):
```
https://github.com/zjaco13/Noto-Emoji-Monochrome/raw/b80db438fe644bd25e0032661ab66fa72f2af0e2/fonts/NotoEmoji-Regular.ttf
```

Variable font (all weights in one file):
```
https://github.com/zjaco13/Noto-Emoji-Monochrome/raw/b80db438fe644bd25e0032661ab66fa72f2af0e2/fonts/NotoEmoji-VariableFont_wght.ttf
```

### Metadata
- **File size:** ~858 KB (static Regular), ~3 MB for full 5-weight ZIP
- **Format:** Standard TrueType outlines — no color tables, no bitmaps
- **Coverage:** ~25% of Unicode emoji (not the full set — many emoji missing)
- **Rendering:** Pure black vector glyphs; will render in the app's current text color
- **License:** SIL Open Font License 1.1
- **Repository:** https://github.com/googlefonts/noto-emoji (fonts/ dir)

### Embedding Considerations
- Very small at 858 KB — safe to embed directly with `include_bytes!`.
- Standard outline font — works with any renderer without special color table support.
- Limitation: only covers ~25% of emoji codepoints. Many common emoji will show tofu boxes if this is the only fallback.
- Good as a supplemental fallback after `NotoColorEmoji.ttf`, not as the primary emoji font.

---

## Vello COLR v1 Support Analysis

### What Vello Itself Supports

Vello added color glyph rendering in v0.3.0 (2024-10-04) via PR #615/#641. The vello issue #536 confirms:

- **COLRv1:** Supported in vello's own glyph renderer
- **CBDT/SBIX bitmaps:** Not yet implemented in vello (marked as follow-up work)

Vello 0.4.1 patched "incorrect COLR Emoji rendering" that particularly affected Windows users.

### The Stack Problem: vello vs cosmic_text

This is the key architectural distinction:

```
cosmic_text (shaping, layout, font selection)
    ↓
produces GlyphRun with resolved glyph IDs + positions
    ↓
vello (rendering, draws glyph pixels)
```

**cosmic_text** handles font selection, Unicode shaping, and glyph ID resolution. It is cosmic_text — not vello — that reads COLR tables to produce color layer data for the renderer. Since cosmic_text does not implement COLR v1, the color layer data never reaches vello, so vello's own COLRv1 support is irrelevant for this pipeline.

**Summary of COLR v1 support by component:**

| Component | COLRv1 Support | Status |
|---|---|---|
| `vello` (renderer) | Yes | Working since 0.3.0, fixed in 0.4.1 |
| `skrifa` (font parsing lib) | Yes | Implements COLR v1 table spec |
| `cosmic_text` (shaping layer) | **No** | Open bug, no fix as of March 2026 |
| `swash` (used by cosmic_text) | Limited | Supports ligatures/color but not COLRv1 |

### Practical Result

If using cosmic_text for text shaping (which this renderer does), COLRv1 fonts produce blank glyphs. The correct font to embed is `NotoColorEmoji.ttf` (CBDT bitmaps), NOT `Noto-COLRv1.ttf`.

If in the future cosmic_text gains COLRv1 support, `Noto-COLRv1.ttf` can be swapped in for a significantly smaller binary (~3 MB vs 10.2 MB) with vector quality at all scales.

---

## Decision Matrix

| Criterion | NotoColorEmoji.ttf (CBDT) | NotoEmoji-Regular.ttf (monochrome) |
|---|---|---|
| Works with cosmic_text | Yes | Yes |
| Works with COLRv1 | N/A (uses bitmaps) | N/A (monochrome) |
| Emoji coverage | Full (Unicode 17.0) | ~25% only |
| File size | 10.2 MB | 858 KB |
| Color | Yes (color bitmaps) | No (black outlines only) |
| Scale quality | Fixed-res bitmaps | Perfect vector scaling |
| Embed as `include_bytes!` | Not recommended (10 MB binary bloat) | Safe |
| Ship as sidecar asset | Recommended | Optional |

---

## Recommended Implementation Strategy

### Primary approach (full emoji support)
1. Ship `NotoColorEmoji.ttf` as a bundled asset (not `include_bytes!` — too large).
2. Load at runtime: `font_system.db_mut().load_font_data(std::fs::read("assets/NotoColorEmoji.ttf")?);`
3. Set as fallback font in cosmic_text's font database after system fonts.

### Fallback approach (minimal binary size)
1. Embed `NotoEmoji-Regular.ttf` via `include_bytes!` (858 KB adds ~860 KB to binary).
2. Accept ~75% emoji coverage gap.
3. Upgrade to color version later via asset download.

### Future-proof note
If/when cosmic_text implements COLRv1 (tracked in pop-os/cosmic-epoch#2546 and pop-os/cosmic-text#310), switch to `Noto-COLRv1.ttf` which is vector-only and significantly smaller.

---

## Direct Download Commands

```bash
# Color emoji (CBDT bitmaps, full coverage) — ship as asset
curl -L "https://github.com/googlefonts/noto-emoji/raw/refs/heads/main/fonts/NotoColorEmoji.ttf" \
     -o assets/NotoColorEmoji.ttf

# Monochrome (embed-safe size, partial coverage)
curl -L "https://github.com/googlefonts/noto-emoji/raw/refs/heads/main/fonts/NotoEmoji-Regular.ttf" \
     -o assets/NotoEmoji-Regular.ttf

# COLRv1 (DO NOT USE until cosmic_text fixes COLRv1 support)
# curl -L "https://github.com/googlefonts/noto-emoji/raw/refs/heads/main/fonts/Noto-COLRv1.ttf" \
#      -o assets/Noto-COLRv1.ttf
```

---

## Sources

- [googlefonts/noto-emoji — Main Repository](https://github.com/googlefonts/noto-emoji)
- [NotoColorEmoji.ttf blob page (size confirmation)](https://github.com/googlefonts/noto-emoji/blob/main/fonts/NotoColorEmoji.ttf)
- [Noto Color Emoji — Google Fonts specimen](https://fonts.google.com/noto/specimen/Noto+Color+Emoji)
- [Noto Emoji (monochrome) — Google Fonts specimen](https://fonts.google.com/noto/specimen/Noto+Emoji)
- [cosmic-epoch issue #2546 — COLRv1 blank spaces in Cosmic apps (open, Nov 2025)](https://github.com/pop-os/cosmic-epoch/issues/2546)
- [cosmic-text issue #310 — Emoji rendering yields empty image](https://github.com/pop-os/cosmic-text/issues/310)
- [linebender/vello — Main Repository](https://github.com/linebender/vello)
- [vello issue #536 — Trouble drawing emojis (COLR support confirmed in vello)](https://github.com/linebender/vello/issues/536)
- [Fedora wiki — Changes/Use COLR for Noto Color Emoji](https://fedoraproject.org/wiki/Changes/Use_COLR_for_Noto_Color_Emoji)
- [AUR package ttf-noto-emoji-monochrome](https://aur.archlinux.org/packages/ttf-noto-emoji-monochrome)
- [Noto Emoji Monochrome mirror repo (zjaco13)](https://github.com/zjaco13/Noto-Emoji-Monochrome)
- [pop-os/cosmic-text — GitHub](https://github.com/pop-os/cosmic-text)
- [Chrome DevBlog — COLRv1 Color Gradient Vector Fonts](https://developer.chrome.com/blog/colrv1-fonts)
