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
| `tau` | 80 | Felzenszwalb k/size. `--median-cut` (`tau=0`) is the old global palette |

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
5. Re-trace the fixture (six tiles, see below). Do not tune for one scene.

Do not ship a “better MAE” that washed out a color the source still has.

## Agnostic engine, fixture is not the product

This crate traces **any** flat-color PNG/JPEG. Pig Slayer tiles are a
regression fixture. Do not add scene-specific palettes, colour names, or
per-tile knobs.

Fixture (re-trace after every algorithm change, look at the sheets):

`PS/pig-slayer/assets/tiles-graphic/{01-flat-vector,07-print-noblood,12-memphis,02-sticker,09-neobrutalist,11-pixel}.jpg`

## Known open delta (continue here)

Default is **spatial Felzenszwalb** (owned rewrite of IJCV 2004), not a
global palette. Global median-cut is `--median-cut`. Do not vendor vtracer.

Hue barrier: two chromatic flats with opponent-hue ≳30° never merge, even
across a 1px JPEG ramp. Cream is not a universal solvent.

| tile | vs source MAE | close | visible | vs quant close | notes |
|---|---|---|---|---|---|
| neobrutalist | 2.53 | 93.8% | 3.5% | 99.4% | auto enough |
| memphis | 5.16 | 87.3% | 6.8% | 98.9% | mint rays exact (was cyan) |
| flat | 4.47 | 87.6% | 6.7% | 98.8% | gold/floral kept |
| print | 6.28 | 87.3% | 7.2% | 98.6% | |
| sticker | 3.91 | 91.4% | 5.0% | 99.5% | |
| pixel | 11.08 | 71.6% | 16.8% | 97.5% | dense burst; still not a poster |

SVG is fat (~2–3k paths) because same-fill neighbours are not yet
collapsed spatially. Next: neighbour merge of identical paint, then GPU
bilateral (`quantize::bilateral` is the CPU stand-in).

100% vs JPEG with hard fills is not reachable (1px AA). 100% vs quant is
the engine ceiling; poster-like tiles sit ~99%.

- Thin black outlines still fragment on print linocut texture.
- Residual pass vs quant not started.
- GIF after still-image on poster-like tiles; pixel-burst is a different job.

Agent loop after auto: skill `uzor-vectorize` (hand-finish fill hexes / holes,
do not chase AA).

## Layout

```
uzor-vectorize/
  src/lib.rs           orchestrate + resvg roundtrip + parity stats
  src/quantize.rs      median-cut, k-means, similar-color merge
  src/segment.rs       Felzenszwalb (opt-in --tau)
  src/region.rs        4-connected labels, majority snap, speckle absorb
  src/contour.rs       pixel-boundary loops, corner-preserving simplify
  src/svg.rs           path `d` + document
  src/bin/uzor-vectorize.rs
```

No `uzor` engine dependency. Same `image` / `resvg` / `tiny-skia` family
as `uzor-icon`.
