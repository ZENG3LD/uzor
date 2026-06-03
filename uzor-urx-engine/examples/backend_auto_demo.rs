//! Demonstrates `Backend::auto(WorkloadHint)` heuristic chooser.
//!
//! Tries a matrix of realistic consumer workloads against the
//! `Backend::auto` heuristic and prints which backend each one picks.
//! Pure CPU — no GPU device needed, no real rendering. The point is
//! to show consumers how `Backend::auto` is meant to be wired into
//! their own apps.
//!
//! Run with default features (wgpu-backend only — Hybrid disabled):
//!   cargo run -p uzor-urx-engine --example backend_auto_demo --release
//!
//! Run with hybrid-backend ALSO enabled to see Hybrid wins:
//!   cargo run -p uzor-urx-engine --example backend_auto_demo --release \
//!     --features hybrid-backend
//!
//! Note: `Backend::auto` resolves available variants at compile time via
//! cfg(feature = ...). With only wgpu-backend, retained-mode workloads
//! that would prefer Hybrid fall through to Wgpu instead.

use uzor_urx_engine::engine::{Backend, WorkloadHint};

fn main() {
    let scenarios: &[(&str, WorkloadHint)] = &[
        ("headless / no GPU",
         WorkloadHint { gpu_available: false, ..Default::default() }),

        ("tiny window, no animation",
         WorkloadHint {
             region_count: 2, total_pixels: 50_000,
             gpu_available: true, ..Default::default()
         }),

        ("dashboard, 32 panels static",
         WorkloadHint {
             region_count: 32, total_pixels: 1_920 * 1_080,
             retained: true, gpu_available: true,
             ..Default::default()
         }),

        ("chart, high-frequency animation, large area",
         WorkloadHint {
             region_count: 8, total_pixels: 2_000_000,
             high_hz: true, gpu_available: true,
             ..Default::default()
         }),

        ("apple-silicon unified memory, mixed scene",
         WorkloadHint {
             region_count: 16, total_pixels: 1_500_000,
             gpu_available: true, unified_memory: true,
             ..Default::default()
         }),

        ("retained mostly-static + occasional update",
         WorkloadHint {
             region_count: 64, total_pixels: 1_920 * 1_080,
             retained: true, high_hz: false,
             gpu_available: true,
             ..Default::default()
         }),
    ];

    println!("┌────────────────────────────────────────────────────┬──────────┐");
    println!("│ Scenario                                           │ Backend  │");
    println!("├────────────────────────────────────────────────────┼──────────┤");
    for (label, hint) in scenarios {
        let backend = Backend::auto(*hint);
        println!("│ {:<50} │ {:<8} │", label, format!("{:?}", backend));
    }
    println!("└────────────────────────────────────────────────────┴──────────┘");
    println!();
    println!("Heuristic summary (engine.rs Backend::auto):");
    println!("  - no gpu                           → Cpu");
    println!("  - region_count<4 && pixels<100k    → Cpu (overhead-bound)");
    println!("  - retained && !high_hz             → Hybrid (cache once, composite N)");
    println!("  - unified_memory                   → Hybrid (zero-copy upload)");
    println!("  - high_hz && pixels>1M             → Wgpu (instanced direct GPU)");
    println!("  - default with gpu                 → Hybrid (safe general-purpose)");
}
