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

GPU bilateral (wgpu, CPU fallback) → **Felzenszwalb on the 4-graph**
(tau=80, hue barrier) → adjacent same-paint merge → speckle absorb →
interior-median fill → pixel-corner paths, `fill-rule="nonzero"`.
`--median-cut` is the old global palette. `--cpu` skips GPU.

## Open work

More vision stages on GPU (labels, not just denoise). GIF.
