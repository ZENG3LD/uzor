# uzor-vectorize — agent contract

PNG/JPEG → SVG tracer for **flat-color** graphics. Inverse of `uzor-icon`
(that crate rasterizes SVG; this one vectorizes a raster).

This is an uzor workspace member (`publish = false` until the API and
parity numbers stabilize). Not a Python script, not a nemo-root tool.

## Why it exists

Agents generate raster illustrations (Imagine, screenshots) and then need
a real SVG that stays sharp at any size, plus later GIF frames that do not
smear. JPEG/PNG anti-alias and JPEG ringing destroy naive traces. This
crate owns that conversion so every harness uses one engine.

Do **not** revive `PS/pig-slayer/tools/raster_vector.py` — it was a
prototype and has been deleted.

## How to run

From `nemo/uzor`:

```
cargo build -p uzor-vectorize --bin uzor-vectorize --release
target/release/uzor-vectorize.exe parity <in.png|jpg> -o out.svg --preview out.png --diff out-diff.png
```

`parity` writes: SVG, resvg preview PNG, quantized reconstruction, heatmap,
and a 2×2 `*-sheet.png`. Read the sheet. Do not declare quality from MAE
alone.

Library: `vectorize_path` / `vectorize_rgb` → `SvgDocument`. Rasterize with
`rasterize_svg` (resvg, same path as `uzor-icon`).

## Knobs (`VectorizeOptions`)

| field | default | meaning |
|---|---|---|
| `colors` | 48 | median-cut bins (no k-means unless `--kmeans-iters`) |
| `merge` | 14 | RGB distance to collapse similar palette entries; chromatic hues that differ by ≳30° in opponent space are never merged (mint must not chain into cyan) |
| `min_area` | 32 | drop / absorb blobs smaller than this (px) |
| `epsilon` | 0.4 | path simplify; 0 = pixel stairs |
| `absorb_dist` | 48 | max RGB distance when eating speckles into a neighbor |
| `kmeans_iters` | 0 | off. k-means reassignment steals gold/floral/mint into large fills |
| `majority` | false | 3×3 label majority. Off: it ate thin black outlines |

Two-tier absorb: area < 16 always eaten (JPEG dirt); 16..min_area only if
the neighbor is within `absorb_dist` (gold must not fall into pink).

`parity` also writes `*-sheet.png` (source | svg / quant | heatmap). Heatmap
is luma + red only where max-channel error > 8, so a 5-level fill shift
does not paint the whole sheet.

## Parity — two ceilings, do not mix them

- **vs quant** — SVG raster vs the quantized source. Engine correctness.
  ~98% pixels within 8 on print/memphis; lower on busy flats. Residual is
  dropped `min_area` crumbs, evenodd on complex blobs, resvg edge coverage.
- **vs JPEG/PNG source** — what the owner sees. Remaining ~7–9% “visible”
  is almost all 1px anti-alias around every edge. Hard SVG fills cannot
  reproduce JPEG coverage without turning the fringe into dirt.

A drop in vs-source MAE that also loses a distinct hue (gold coins, floral
red) is a **regression**, even if the number looks finer.

## Debug loop (mandatory)

1. Trace.
2. Open `preview`, `*-quant.png`, `*-diff.png`, and the source. Look.
3. Name the failure (lost hue, hole in a fill, broken outline, AA halo).
4. Change one knob or one pipeline stage.
5. Re-trace the same three Pig Slayer tiles:
   `PS/pig-slayer/assets/tiles-graphic/{01-flat-vector,07-print-noblood,12-memphis}.jpg`

Do not ship a “better MAE” that washed out a color the source still has.

## Known open delta (continue here)

Last Python prototype (Pillow `MEDIANCUT`, no k-means): print MAE 6.93 /
close 85.1%; flat 4.94 / 85.1%; memphis 6.25 / 80.2%. Rust as of this
pipeline (3×3 denoise, range median-cut, hue-safe merge, median snap):

| tile | vs source MAE | close | visible | vs quant close |
|---|---|---|---|---|
| print | 6.23 | 86.4% | 7.7% | 97.4% |
| flat | 5.45 | 84.7% | 7.9% | 97.5% |
| memphis | 7.77 | 71.5% | 17.7% | 97.7% |

Print is past Python. Flat is within a point. Memphis mint sunburst rays
still collapse into cyan — Pillow's unique-color population cut kept
them; a naïve port of that cut spent the palette on cream JPEG noise and
turned flat-vector gold coins red. Do not revive that cut.

- Thin black outlines still fragment on print linocut texture.
- No residual pass yet (rasterize SVG, vectorize leftover error blobs).
- GIF frame stacking is out of scope until memphis mint is back.

## Layout

```
uzor-vectorize/
  src/lib.rs           orchestrate + resvg roundtrip + parity stats
  src/quantize.rs      median-cut, k-means, similar-color merge
  src/region.rs        4-connected labels, majority snap, speckle absorb
  src/contour.rs       pixel-boundary loops, corner-preserving simplify
  src/svg.rs           path `d` + document
  src/bin/uzor-vectorize.rs
```

No `uzor` engine dependency. Same `image` / `resvg` / `tiny-skia` family
as `uzor-icon`.
