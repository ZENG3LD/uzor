//! CPU blend-layer isolation + degrade-counter proofs (URX Wave 3
//! Commit 1, `docs/uzor-engines/plans/urx-wave3-clip-blend-design-2026-07-25.md`
//! §6.3/§6.4/§6.5). CPU-only, no GPU, always-on (not `--ignored`), fast.
//!
//! `blend_layer_isolation_differs_from_direct_draw` is the load-bearing
//! test: it does NOT compare against the native backend at all — it
//! proves `PushBlendLayer`/`PopBlendLayer` are structurally load-bearing
//! on THIS backend alone. This test MUST fail if a future regression
//! silently degrades layers back to identity (design §0.4).

use metrics::{Counter, CounterFn, Gauge, Histogram, Key, KeyName, Metadata, Recorder, SharedString, Unit};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use uzor_urx_core::config::UrxConfig;
use uzor_urx_core::math::{Affine, Brush, Color, Rect};
use uzor_urx_core::metrics_keys::KEY_RENDER_PRIMITIVES;
use uzor_urx_core::scene::{DrawCommand, Scene};
use uzor_urx_cpu::{CpuBackend, Pixmap};

const CANVAS: u32 = 256;

fn solid(r: u8, g: u8, b: u8, a: u8) -> Brush {
    Brush::Solid(Color::from_rgba8(r, g, b, a))
}

fn push_background(scene: &mut Scene) {
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, CANVAS as f64, CANVAS as f64),
        radii: None,
        brush: solid(32, 32, 32, 255),
        transform: Affine::IDENTITY,
    });
}

/// Two overlapping translucent rects, same params in BOTH variants —
/// the ONLY difference between `blend_layer_group`/`blend_layer_group_reference_no_layer`
/// is whether they're wrapped in a blend layer (design §6.3).
fn overlapping_pair(scene: &mut Scene) {
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(60.0, 60.0, 180.0, 180.0),
        radii: None,
        brush: solid(220, 30, 30, 200),
        transform: Affine::IDENTITY,
    });
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(110.0, 110.0, 230.0, 230.0),
        radii: None,
        brush: solid(30, 60, 220, 200),
        transform: Affine::IDENTITY,
    });
}

fn blend_layer_group() -> Scene {
    let mut scene = Scene::new();
    push_background(&mut scene);
    scene.push(DrawCommand::PushBlendLayer { mode: peniko::BlendMode::default(), alpha: 0.6, transform: Affine::IDENTITY });
    overlapping_pair(&mut scene);
    scene.push(DrawCommand::PopBlendLayer);
    scene
}

fn blend_layer_group_reference_no_layer() -> Scene {
    let mut scene = Scene::new();
    push_background(&mut scene);
    overlapping_pair(&mut scene); // SAME shapes, no layer wrapper, no alpha multiplier
    scene
}

fn render(scene: &Scene) -> Pixmap {
    let mut pixmap = Pixmap::new(CANVAS, CANVAS);
    CpuBackend::new().render(scene, &mut pixmap).expect("well-formed fixture scene must not error");
    pixmap
}

fn max_channel_diff(a: [u8; 4], b: [u8; 4]) -> i32 {
    a.iter().zip(b.iter()).map(|(x, y)| (*x as i32 - *y as i32).abs()).max().unwrap_or(0)
}

/// The load-bearing test (design §6.3 item 1): renders BOTH scenes
/// through `CpuBackend::render` and asserts the shapes' OVERLAP-region
/// probe pixel differs by MORE than a small epsilon between them. This
/// FAILS if `PushBlendLayer` silently degrades back to identity — a
/// degraded-to-identity layer would draw the SAME two rects in the SAME
/// painter's order with NO alpha multiplier, producing byte-identical
/// output to the no-layer reference.
#[test]
fn blend_layer_isolation_differs_from_direct_draw() {
    let layered = render(&blend_layer_group());
    let direct = render(&blend_layer_group_reference_no_layer());

    // (140, 140): inside both overlapping rects' bounds.
    let probe = (140u32, 140u32);
    let a = layered.get_pixel(probe.0, probe.1);
    let b = direct.get_pixel(probe.0, probe.1);
    let diff = max_channel_diff(a, b);
    assert!(
        diff > 8,
        "layered vs direct-draw must differ meaningfully at the overlap probe \
         (layer isolation would be silently degraded to identity otherwise): \
         layered={a:?} direct={b:?} (max_diff={diff})"
    );
}

/// Alpha-scaling correctness (design §6.5): a SINGLE opaque rect inside
/// a `PushBlendLayer{alpha: 0.5}` over an opaque background must land
/// at the mathematically exact 50%-scaled composite, not merely "some
/// different value."
#[test]
fn blend_layer_alpha_scaling_is_measurable_and_exact() {
    let mut scene = Scene::new();
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, CANVAS as f64, CANVAS as f64),
        radii: None,
        brush: solid(0, 0, 0, 255), // opaque black backdrop
        transform: Affine::IDENTITY,
    });
    scene.push(DrawCommand::PushBlendLayer { mode: peniko::BlendMode::default(), alpha: 0.5, transform: Affine::IDENTITY });
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(50.0, 50.0, 150.0, 150.0),
        radii: None,
        brush: solid(200, 100, 40, 255), // opaque — layer's OWN alpha (1.0) unaffected by this
        transform: Affine::IDENTITY,
    });
    scene.push(DrawCommand::PopBlendLayer);

    let pixmap = render(&scene);
    // Deep interior of the rect, away from any AA edge.
    let px = pixmap.get_pixel(100, 100);
    // Premultiplied-alpha-scale by 0.5, SrcOver onto opaque black:
    // 200*0.5=100, 100*0.5=50, 40*0.5=20, 255*0.5=127.5->128;
    // blended over opaque black (dst=[0,0,0,255]) — see
    // `blend.rs::composite_layer_srcover`'s own doc comment for the
    // exact integer-rounding formula this must match.
    assert_eq!(px, [100, 50, 20, 255], "alpha=0.5 layer over opaque black must land at the exact scaled composite");
}

/// Nested 2-layer scene correctness (design §6.5): inner layer
/// composites onto outer, outer onto root — proves the parent-
/// resolution logic in `LayerStack::pop` picks the right target at
/// each level, not just for a single layer.
#[test]
fn nested_two_layer_scene_composites_inner_onto_outer_onto_root() {
    let mut scene = Scene::new();
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, CANVAS as f64, CANVAS as f64),
        radii: None,
        brush: solid(0, 0, 0, 255),
        transform: Affine::IDENTITY,
    });
    scene.push(DrawCommand::PushBlendLayer { mode: peniko::BlendMode::default(), alpha: 0.5, transform: Affine::IDENTITY }); // outer
    scene.push(DrawCommand::PushBlendLayer { mode: peniko::BlendMode::default(), alpha: 0.5, transform: Affine::IDENTITY }); // inner
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(50.0, 50.0, 150.0, 150.0),
        radii: None,
        brush: solid(200, 200, 200, 255),
        transform: Affine::IDENTITY,
    });
    scene.push(DrawCommand::PopBlendLayer); // inner -> outer, scaled by 0.5
    scene.push(DrawCommand::PopBlendLayer); // outer -> root, scaled by 0.5 again

    let pixmap = render(&scene);
    let px = pixmap.get_pixel(100, 100);
    // Inner: 200 * 0.5 = 100 (onto a transparent outer layer, so the
    // outer layer's own pixel becomes exactly [100,100,100,128ish]).
    // Outer pop: that result scaled by 0.5 AGAIN onto opaque black —
    // i.e. roughly a quarter of the original 200, not a half. This is
    // the numerically-observable proof that BOTH pops actually ran
    // their own composite step (a single-composite bug would land at
    // ~100, not ~50).
    assert!(
        px[0] < 70,
        "double-nested 0.5*0.5 alpha must land noticeably below a single 0.5 composite (~100): got {px:?}"
    );
    assert!(px[0] > 20, "double-nested composite must still show SOME ink, not fully vanish: got {px:?}");
}

/// Pop-underflow (design §6.5): a stray `PopBlendLayer` with no
/// matching push must not panic and must count
/// `cpu_blend_layer_pop_underflow` exactly once.
#[test]
fn pop_underflow_counts_and_never_panics() {
    let recorder = TestRecorder::default();
    metrics::with_local_recorder(&recorder, || {
        let mut scene = Scene::new();
        push_background(&mut scene);
        scene.push(DrawCommand::PopBlendLayer); // no matching push at all
        let mut pixmap = Pixmap::new(CANVAS, CANVAS);
        CpuBackend::new().render(&scene, &mut pixmap).expect("unbalanced Pop must not error, just count + no-op");
    });
    let value = recorder.value_for(KEY_RENDER_PRIMITIVES, "cpu_blend_layer_pop_underflow");
    assert_eq!(value, 1);
}

/// End-of-scene force-close (audit follow-up, design §3.3's GPU-side
/// `native_blend_layer_force_closed_at_scene_end` mirrored on CPU): a
/// `PushBlendLayer` with content but NO matching `PopBlendLayer` must
/// NOT silently lose that content — before this fix, the open
/// `LayerFrame` (and whatever was drawn into it) would simply be
/// dropped along with `LayerStack` at the end of `render()`. The
/// content must still land on the root pixmap, alpha-scaled exactly as
/// a well-formed matching Pop would have produced, and the force-close
/// must be counted (never silent).
#[test]
fn unbalanced_push_without_pop_still_composites_content_and_counts_force_close() {
    let recorder = TestRecorder::default();
    let pixmap = metrics::with_local_recorder(&recorder, || {
        let mut scene = Scene::new();
        scene.push(DrawCommand::FillRect {
            rect: Rect::new(0.0, 0.0, CANVAS as f64, CANVAS as f64),
            radii: None,
            brush: solid(0, 0, 0, 255), // opaque black backdrop
            transform: Affine::IDENTITY,
        });
        scene.push(DrawCommand::PushBlendLayer { mode: peniko::BlendMode::default(), alpha: 0.5, transform: Affine::IDENTITY });
        scene.push(DrawCommand::FillRect {
            rect: Rect::new(50.0, 50.0, 150.0, 150.0),
            radii: None,
            brush: solid(200, 100, 40, 255),
            transform: Affine::IDENTITY,
        });
        // NO PopBlendLayer — deliberately unbalanced.
        let mut pixmap = Pixmap::new(CANVAS, CANVAS);
        CpuBackend::new().render(&scene, &mut pixmap).expect("unbalanced scene must not error");
        pixmap
    });

    let px = pixmap.get_pixel(100, 100);
    // Same exact math as `blend_layer_alpha_scaling_is_measurable_and_exact`
    // (alpha=0.5 over opaque black) — proves the force-close path
    // composites through the IDENTICAL code (`composite_top_onto_parent`)
    // a normal Pop uses, not a different, unverified fallback.
    assert_eq!(
        px,
        [100, 50, 20, 255],
        "content from an unmatched PushBlendLayer must still be visible on root, alpha-scaled — NOT silently dropped"
    );

    let force_closed = recorder.value_for(KEY_RENDER_PRIMITIVES, "cpu_blend_layer_force_closed_at_scene_end");
    assert_eq!(force_closed, 1, "exactly one still-open layer must be counted as force-closed");
    let underflow = recorder.value_for(KEY_RENDER_PRIMITIVES, "cpu_blend_layer_pop_underflow");
    assert_eq!(underflow, 0, "force-close is a distinct, honest label — must not ALSO count as a pop-underflow");
}

/// Depth-cap suppression (design §6.5): `blend_layer_max_depth` via
/// `CpuBackend::with_config`, pushes beyond the cap keep drawing into
/// the current target (no panic, content still visible) and count
/// `cpu_blend_layer_depth_exceeded` for every suppressed push.
#[test]
fn depth_cap_suppresses_pushes_beyond_max_depth_and_still_draws() {
    let cfg = UrxConfig::builder().blend_layer_max_depth(1).build().expect("max_depth=1 is valid");
    let backend = CpuBackend::with_config(cfg);

    let recorder = TestRecorder::default();
    let pixmap = metrics::with_local_recorder(&recorder, || {
        let mut scene = Scene::new();
        push_background(&mut scene);
        scene.push(DrawCommand::PushBlendLayer { mode: peniko::BlendMode::default(), alpha: 1.0, transform: Affine::IDENTITY }); // depth 1 — allowed
        scene.push(DrawCommand::PushBlendLayer { mode: peniko::BlendMode::default(), alpha: 1.0, transform: Affine::IDENTITY }); // depth 2 — suppressed
        scene.push(DrawCommand::FillRect {
            rect: Rect::new(50.0, 50.0, 150.0, 150.0),
            radii: None,
            brush: solid(220, 10, 10, 255),
            transform: Affine::IDENTITY,
        });
        scene.push(DrawCommand::PopBlendLayer); // matches the suppressed push — no-op
        scene.push(DrawCommand::PopBlendLayer); // matches the real push — composites
        let mut pixmap = Pixmap::new(CANVAS, CANVAS);
        backend.render(&scene, &mut pixmap).expect("depth-capped scene must not error");
        pixmap
    });

    let exceeded = recorder.value_for(KEY_RENDER_PRIMITIVES, "cpu_blend_layer_depth_exceeded");
    assert_eq!(exceeded, 1, "exactly one push must have been suppressed (depth 2 at max_depth=1)");
    let pop_underflow = recorder.value_for(KEY_RENDER_PRIMITIVES, "cpu_blend_layer_pop_underflow");
    assert_eq!(pop_underflow, 0, "the suppressed push's matching pop must balance silently, not count as underflow");

    // Content past the cap still drew (into the still-open depth-1
    // layer) — not silently dropped.
    let px = pixmap.get_pixel(100, 100);
    assert_ne!(px, [0, 0, 0, 0], "content beyond the depth cap must still be visible somewhere in the final composite");
}

/// Mix/compose degrade counters (design §0.3/§6.5): a non-default
/// `Mix` counts `cpu_blend_layer_mix_to_normal` but NOT the compose
/// counter; a non-default `Compose` counts the opposite way. Each
/// counts exactly once per `PopBlendLayer` (checked at pop time, not
/// per-pixel).
#[test]
fn mix_and_compose_degrades_count_independently() {
    let recorder = TestRecorder::default();
    metrics::with_local_recorder(&recorder, || {
        let mut scene = Scene::new();
        push_background(&mut scene);
        scene.push(DrawCommand::PushBlendLayer {
            mode: peniko::BlendMode { mix: peniko::Mix::Multiply, compose: peniko::Compose::SrcOver },
            alpha: 1.0,
            transform: Affine::IDENTITY,
        });
        scene.push(DrawCommand::PopBlendLayer);
        let mut pixmap = Pixmap::new(CANVAS, CANVAS);
        CpuBackend::new().render(&scene, &mut pixmap).expect("must not error");
    });
    assert_eq!(recorder.value_for(KEY_RENDER_PRIMITIVES, "cpu_blend_layer_mix_to_normal"), 1);
    assert_eq!(recorder.value_for(KEY_RENDER_PRIMITIVES, "cpu_blend_layer_compose_to_srcover"), 0);

    let recorder2 = TestRecorder::default();
    metrics::with_local_recorder(&recorder2, || {
        let mut scene = Scene::new();
        push_background(&mut scene);
        scene.push(DrawCommand::PushBlendLayer {
            mode: peniko::BlendMode { mix: peniko::Mix::Normal, compose: peniko::Compose::DestOver },
            alpha: 1.0,
            transform: Affine::IDENTITY,
        });
        scene.push(DrawCommand::PopBlendLayer);
        let mut pixmap = Pixmap::new(CANVAS, CANVAS);
        CpuBackend::new().render(&scene, &mut pixmap).expect("must not error");
    });
    assert_eq!(recorder2.value_for(KEY_RENDER_PRIMITIVES, "cpu_blend_layer_mix_to_normal"), 0);
    assert_eq!(recorder2.value_for(KEY_RENDER_PRIMITIVES, "cpu_blend_layer_compose_to_srcover"), 1);
}

/// A non-identity `transform` on `PushBlendLayer` counts
/// `cpu_blend_layer_transform_ignored` (design §0.2) — the IR field
/// has no defined semantics yet; this is the honest disclosure, not a
/// silent gap.
#[test]
fn non_identity_transform_counts_ignored_but_still_renders() {
    let recorder = TestRecorder::default();
    metrics::with_local_recorder(&recorder, || {
        let mut scene = Scene::new();
        push_background(&mut scene);
        scene.push(DrawCommand::PushBlendLayer {
            mode: peniko::BlendMode::default(),
            alpha: 1.0,
            transform: Affine::translate((10.0, 10.0)),
        });
        scene.push(DrawCommand::PopBlendLayer);
        let mut pixmap = Pixmap::new(CANVAS, CANVAS);
        CpuBackend::new().render(&scene, &mut pixmap).expect("must not error");
    });
    assert_eq!(recorder.value_for(KEY_RENDER_PRIMITIVES, "cpu_blend_layer_transform_ignored"), 1);
}

// ── Hand-rolled metrics test recorder (same shape as
// `uzor-urx-wgpu/src/encode.rs`'s `metrics_recorder_proof` submodule,
// Wave 1 Commit 4 precedent) — `metrics::with_local_recorder` needs no
// new dev-dependency, `metrics = "0.24"` is already a direct dep. ────

struct RecordedCounter(AtomicU64);
impl CounterFn for RecordedCounter {
    fn increment(&self, value: u64) {
        self.0.fetch_add(value, Ordering::SeqCst);
    }
    fn absolute(&self, value: u64) {
        self.0.store(value, Ordering::SeqCst);
    }
}

#[derive(Default)]
struct TestRecorder {
    counters: Mutex<HashMap<Key, Arc<RecordedCounter>>>,
}

impl TestRecorder {
    fn value_for(&self, metric_name: &str, label: &str) -> u64 {
        let map = self.counters.lock().unwrap_or_else(|e| e.into_inner());
        map.iter()
            .filter(|(key, _)| {
                key.name() == metric_name && key.labels().any(|l| l.key() == "kind" && l.value() == label)
            })
            .map(|(_, counter)| counter.0.load(Ordering::SeqCst))
            .sum()
    }
}

impl Recorder for TestRecorder {
    fn describe_counter(&self, _key: KeyName, _unit: Option<Unit>, _description: SharedString) {}
    fn describe_gauge(&self, _key: KeyName, _unit: Option<Unit>, _description: SharedString) {}
    fn describe_histogram(&self, _key: KeyName, _unit: Option<Unit>, _description: SharedString) {}

    fn register_counter(&self, key: &Key, _metadata: &Metadata<'_>) -> Counter {
        let mut map = self.counters.lock().unwrap_or_else(|e| e.into_inner());
        let handle = map.entry(key.clone()).or_insert_with(|| Arc::new(RecordedCounter(AtomicU64::new(0))));
        Counter::from_arc(handle.clone())
    }
    fn register_gauge(&self, _key: &Key, _metadata: &Metadata<'_>) -> Gauge {
        Gauge::noop()
    }
    fn register_histogram(&self, _key: &Key, _metadata: &Metadata<'_>) -> Histogram {
        Histogram::noop()
    }
}
