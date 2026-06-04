//! Wave 15 + 16 — collision + dynamics tests (no GPU, no #[ignore]).

use uzor_urx_physics::{BodyKind, Collider, PhysicsWorld, Vec3};

#[test]
fn aabb_overlap_detected() {
    let mut w = PhysicsWorld::new();
    w.gravity = Vec3::ZERO;
    let a = w.spawn_static(Collider::aabb(Vec3::splat(1.0)), Vec3::ZERO);
    let b = w.spawn_static(Collider::aabb(Vec3::splat(1.0)), Vec3::new(1.5, 0.0, 0.0));
    let cs = w.find_contacts();
    assert_eq!(cs.len(), 1);
    let c = cs[0];
    assert_eq!((c.a, c.b), (a, b));
    // overlap = 2 - 1.5 = 0.5 along +X
    assert!((c.normal.x - 1.0).abs() < 1e-3);
    assert!((c.depth - 0.5).abs() < 1e-3);
}

#[test]
fn sphere_overlap_detected() {
    let mut w = PhysicsWorld::new();
    w.gravity = Vec3::ZERO;
    w.spawn_static(Collider::sphere(1.0), Vec3::ZERO);
    w.spawn_static(Collider::sphere(1.0), Vec3::new(1.5, 0.0, 0.0));
    let cs = w.find_contacts();
    assert_eq!(cs.len(), 1);
    let c = cs[0];
    assert!((c.depth - 0.5).abs() < 1e-3);
    assert!((c.normal.x - 1.0).abs() < 1e-3);
}

#[test]
fn separated_pair_emits_no_contact() {
    let mut w = PhysicsWorld::new();
    w.gravity = Vec3::ZERO;
    w.spawn_static(Collider::sphere(0.5), Vec3::ZERO);
    w.spawn_static(Collider::sphere(0.5), Vec3::new(2.0, 0.0, 0.0));
    assert!(w.find_contacts().is_empty());
}

#[test]
fn aabb_sphere_contact_normal_points_outward() {
    let mut w = PhysicsWorld::new();
    w.gravity = Vec3::ZERO;
    let a = w.spawn_static(Collider::aabb(Vec3::splat(1.0)), Vec3::ZERO);
    let b = w.spawn_static(Collider::sphere(0.5), Vec3::new(1.3, 0.0, 0.0));
    let cs = w.find_contacts();
    assert_eq!(cs.len(), 1);
    let c = cs[0];
    assert_eq!((c.a, c.b), (a, b));
    // closest point on aabb to sphere centre = (1, 0, 0); dist = 0.3;
    // depth = 0.5 - 0.3 = 0.2; normal points from AABB to sphere = +X
    assert!((c.normal.x - 1.0).abs() < 1e-3);
    assert!((c.depth - 0.2).abs() < 1e-3);
}

#[test]
fn gravity_pulls_dynamic_body_down() {
    let mut w = PhysicsWorld::new();
    let id = w.spawn_dynamic(Collider::sphere(0.5), Vec3::new(0.0, 5.0, 0.0), 1.0);
    for _ in 0..60 { w.step(1.0 / 60.0); }
    let y = w.body(id).unwrap().position.y;
    // After 1 sec of gravity -9.81, free-fall would land at 5 - 4.905 ≈ 0.1.
    assert!(y < 0.5, "should have fallen ~5 units: y={}", y);
}

#[test]
fn ball_bounces_off_static_floor() {
    let mut w = PhysicsWorld::new();
    // Floor: big static AABB whose TOP edge is exactly y=0.
    w.spawn_static(
        Collider::aabb(Vec3::new(10.0, 0.5, 10.0)),
        Vec3::new(0.0, -0.5, 0.0),
    );
    let ball = w.spawn_dynamic(Collider::sphere(0.5), Vec3::new(0.0, 5.0, 0.0), 1.0);
    if let Some(b) = w.body_mut(ball) { b.restitution = 0.8; }

    // 4 seconds of sim — ball should fall, bounce, come to rest above y=0.
    let mut max_y_after_bounce = 0.0f32;
    let mut bounced = false;
    let mut min_y = 1000.0f32;
    for s in 0..240 {
        w.step(1.0 / 60.0);
        let p = w.body(ball).unwrap().position;
        if p.y < min_y { min_y = p.y; }
        if s > 30 && p.y > max_y_after_bounce { max_y_after_bounce = p.y; }
        if s > 30 && p.y > 0.5 { bounced = true; }
    }
    assert!(bounced, "ball did not bounce — peaked only at {}", max_y_after_bounce);
    // Final position must not have tunneled below the floor.
    assert!(min_y > -0.5, "ball tunneled through floor: min_y={}", min_y);
}

#[test]
fn kinematic_body_doesnt_fall_but_collides() {
    let mut w = PhysicsWorld::new();
    let id = w.fresh_id();
    w.add(uzor_urx_physics::Body::kinematic(id, Collider::sphere(0.5), Vec3::new(0.0, 5.0, 0.0)));
    for _ in 0..60 { w.step(1.0 / 60.0); }
    let y = w.body(id).unwrap().position.y;
    assert!((y - 5.0).abs() < 1e-3, "kinematic body should not fall: y={}", y);
    assert_eq!(w.body(id).unwrap().kind, BodyKind::Kinematic);
}

#[test]
fn dynamic_dynamic_collision_pushes_both_apart() {
    let mut w = PhysicsWorld::new();
    w.gravity = Vec3::ZERO;
    let a = w.spawn_dynamic(Collider::sphere(0.5), Vec3::new(-0.3, 0.0, 0.0), 1.0);
    let b = w.spawn_dynamic(Collider::sphere(0.5), Vec3::new(0.3, 0.0, 0.0), 1.0);
    // Move them at each other.
    w.body_mut(a).unwrap().velocity = Vec3::new(1.0, 0.0, 0.0);
    w.body_mut(b).unwrap().velocity = Vec3::new(-1.0, 0.0, 0.0);
    for _ in 0..30 { w.step(1.0 / 60.0); }
    let pa = w.body(a).unwrap().position;
    let pb = w.body(b).unwrap().position;
    let va = w.body(a).unwrap().velocity;
    let vb = w.body(b).unwrap().velocity;
    // Each must have been pushed APART (velocities flipped).
    assert!(va.x < 0.0, "a should rebound to -X: va={:?}", va);
    assert!(vb.x > 0.0, "b should rebound to +X: vb={:?}", vb);
    // Centres must not overlap (distance ≥ r1+r2 ≈ 1.0).
    let dist = (pb - pa).length();
    assert!(dist >= 0.95, "bodies still overlap: dist={}", dist);
}
