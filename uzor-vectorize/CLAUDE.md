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

median-cut → k-means refine → merge similar → 3×3 majority on the index
map → two-tier speckle absorb → 4-connected blobs → pixel-corner outlines
→ corner-preserving RDP → SVG paths, large-area first.

## Open work

k-means starves rare colors. Next: protect small saturated bins, residual
pass vs quant, then GIF frames.
