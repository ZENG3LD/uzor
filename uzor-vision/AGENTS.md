# uzor-vision — agent contract

GPU-first computer vision frontend. First consumer is `uzor-vectorize`
(PNG→SVG). Later consumers (detectors, masks, tracking) take **labels**,
not SVG. Do not vendor OpenCV / vtracer / SAM weights without a supply-
chain review.

## Device

- Default: HighPerformance wgpu adapter.
- No adapter / `--cpu`: CPU bilateral + CPU SLIC.
- `--gpu`: require the adapter; on failure log and fall back.

## Pipeline

1. 3×3 median (CPU, cheap).
2. Bilateral denoise — GPU compute, CPU fallback.
3. SLIC superpixels — GPU assign + CPU center update, CPU fallback.
4. Adjacent same-paint merge (hue barrier: cream is not a solvent).

Output: denoised RGB + label map + per-label fill (interior median is
the tracer's job).

## Do not

- Do not add OpenCV, vtracer, potrace, scipy.
- Do not put SVG / resvg here.
- Do not special-case a scene.
