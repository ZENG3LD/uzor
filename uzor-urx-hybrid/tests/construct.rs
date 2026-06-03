//! Compile-time + construction tests. Full GPU test needs a live wgpu
//! device which is too heavy for `cargo test` here; smoke verifies
//! via a live binary in the URX smoke rig (Phase 7.5).

use uzor_urx_hybrid::HybridBackend;

#[test]
fn backend_constructs_empty() {
    let b = HybridBackend::new();
    assert_eq!(b.region_count(), 0);
    assert_eq!(b.region_bytes(), 0);
}

#[test]
fn quad_instance_is_pod() {
    use bytemuck::Pod;
    fn assert_pod<T: Pod>() {}
    assert_pod::<uzor_urx_hybrid::composite::QuadInstance>();
    assert_pod::<uzor_urx_hybrid::composite::ScreenUniform>();
}
