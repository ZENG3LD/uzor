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

`uzor-vision` GPU bilateral + SLIC → adjacent merge → absorb →
interior-median → nonzero paths. `--fz` / `--median-cut` / `--cpu`.

## Open work

More GPU stages in `uzor-vision` (not in this crate). GIF.
