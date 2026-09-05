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

3×3 per-channel median denoise → Heckbert median-cut on pixels (largest
luminance-weighted range; **box membership kept**, no nearest-centroid)
→ hue-safe similar-color merge → two-tier speckle absorb → median snap
of each bin onto the denoised RGB → 4-connected blobs → pixel-corner
outlines → corner-preserving RDP.

k-means and 3×3 majority are opt-in. Both steal or eat rare hues / thin
black. Population unique-color cuts (Pillow's heap) looked right on
memphis mint in a Python replica and then wrecked gold coins on
flat-vector in Rust — do not bring them back without a fixture on all
three tiles.

## Open work

Memphis mint rays. Felzenszwalb (`--tau`) leaks through AA — next is
spatial neighbour-merge, owned, not a vtracer dep. GIF after poster-like
tiles sit at the quant ceiling.
