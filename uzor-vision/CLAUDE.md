# uzor-vision

GPU-first CV frontend for the uzor workspace. `publish = false`.
Contract: `AGENTS.md`.

## Do

- New vision stages go here, GPU first, CPU fallback.
- `uzor-vectorize` traces the label map. It does not own the GPU.

## Do not

- Do not rustfmt. Do not `cargo test` the whole uzor workspace.
