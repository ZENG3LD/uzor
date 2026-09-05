# uzor-vision — agent contract

GPU-first computer vision frontend. First consumer is `uzor-vectorize`
(PNG→SVG). Later consumers (detectors, masks, tracking) take **labels**,
not SVG. Do not vendor OpenCV / vtracer / SAM weights without a supply-
chain review.

## Device (three modes, not a one-way move)

| mode | denoise | SLIC |
|---|---|---|
| `--cpu` | CPU | CPU |
| `--hybrid` (default) | GPU bilateral | GPU assign, CPU centers |
| `--gpu` | GPU bilateral | assign+accum+div on device, one readback |

No adapter → CPU. `--gpu` failure → hybrid, then CPU.

## Pipeline

1. 3×3 median (CPU).
2. Bilateral.
3. SLIC.
4. Adjacent same-paint merge (hue barrier).

Output: denoised RGB + label map + per-label fill (interior median is
the tracer's job).

## Do not

- Do not add OpenCV, vtracer, potrace, scipy.
- Do not put SVG / resvg here.
- Do not special-case a scene.
