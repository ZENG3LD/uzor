# uzor-vectorize

Flat-color PNG/JPEG → SVG. Pair of `uzor-icon` (SVG → PNG). Workspace
member, `publish = false`. Full contract: `AGENTS.md` in this directory.

## Do

- Build and run `uzor-vectorize` from `nemo/uzor`. Release binary.
- After every algorithm change, re-trace the three Pig Slayer tiles and
  **look at** preview / quant / diff / source. MAE is a secondary signal.
- Treat lost distinct hues as a bug even when vs-source MAE improved.

## Do not

- Do not bring back the Python tracer.
- Do not add `vtracer` / `potrace` / `scipy` / new unreviewed deps.
  Owned median-cut + contours only (supply-chain policy).
- Do not `cargo test` the whole uzor workspace for this crate.
- Do not rustfmt / `cargo fmt`.

## Current pipeline

3×3 median denoise → **Felzenszwalb on the 4-graph** (tau=80, hue barrier
so cream is not a universal solvent) → speckle absorb → interior-median
fill → 4-connected blobs → pixel-corner paths, `fill-rule="nonzero"`,
large-first stack. `--median-cut` is the old global palette.

## Open work

Collapse adjacent same-paint regions (SVG is 2–3k paths). GPU bilateral
as the reusable vision denoise. Then GIF.
