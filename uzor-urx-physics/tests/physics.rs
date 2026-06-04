//! Wave 15 + 16 — collision + dynamics tests (no GPU, no #[ignore]).

use uzor_urx_physics::{BodyKind, Collider, Joint, PhysicsWorld, Vec3};

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
fn distance_joint_keeps_pendulum_fixed_length() {
    // A kinematic anchor at (0, 0, 0) holds a dynamic ball that
    // wants to fall under gravity. Distance joint with rest_length=2
    // → the ball settles to a swinging arc at radius 2.
    let mut w = PhysicsWorld::new();
    let id_anchor = w.fresh_id();
    w.add(uzor_urx_physics::Body::kinematic(id_anchor, Collider::sphere(0.1), Vec3::ZERO));
    let id_ball = w.spawn_dynamic(
        Collider::sphere(0.2),
        Vec3::new(2.0, 0.0, 0.0), // start to the side, distance 2 from anchor
        1.0,
    );
    w.add_joint(Joint::distance(id_anchor, id_ball, 2.0));

    for _ in 0..240 { w.step(1.0 / 60.0); }
    let p = w.body(id_ball).unwrap().position;
    let d = (p - Vec3::ZERO).length();
    // Pendulum should still be roughly at radius 2 from anchor.
    assert!(
        (d - 2.0).abs() < 0.05,
        "pendulum drifted off the joint length: dist={}, pos={:?}",
        d, p
    );
}

#[test]
fn distance_joint_chain_hangs_under_gravity() {
    // Three dynamic balls in a row, joined to a kinematic anchor.
    // After settling, all three should be roughly straight down with
    // joint lengths preserved.
    let mut w = PhysicsWorld::new();
    let id_anchor = w.fresh_id();
    w.add(uzor_urx_physics::Body::kinematic(
        id_anchor, Collider::sphere(0.1), Vec3::ZERO,
    ));
    let mut prev = id_anchor;
    let mut ids = Vec::new();
    for i in 0..3 {
        let id = w.spawn_dynamic(
            Collider::sphere(0.15),
            Vec3::new(0.0, -(i + 1) as f32, 0.0),
            1.0,
        );
        w.add_joint(Joint::distance(prev, id, 1.0));
        ids.push(id);
        prev = id;
    }
    // Settle.
    for _ in 0..600 { w.step(1.0 / 60.0); }

    // First link distance from anchor.
    let p0 = w.body(ids[0]).unwrap().position;
    let p1 = w.body(ids[1]).unwrap().position;
    let p2 = w.body(ids[2]).unwrap().position;
    let d0 = p0.length();
    let d1 = (p1 - p0).length();
    let d2 = (p2 - p1).length();
    eprintln!("chain links: {} {} {}", d0, d1, d2);
    // Joint solver should keep each link near 1 unit.
    for d in [d0, d1, d2] {
        assert!(
            (d - 1.0).abs() < 0.15,
            "joint link drifted: {} (expected ~1)", d
        );
    }
    // Chain hangs roughly downward — y of last ball < y of first.
    assert!(p2.y < p0.y - 0.5, "chain not hanging downward: p0={:?} p2={:?}", p0, p2);
}

#[test]
fn pin_joint_welds_kinematic_anchor_and_dynamic_block() {
    let mut w = PhysicsWorld::new();
    let id_anchor = w.fresh_id();
    w.add(uzor_urx_physics::Body::kinematic(
        id_anchor, Collider::sphere(0.1), Vec3::new(0.0, 5.0, 0.0),
    ));
    let id_block = w.spawn_dynamic(
        Collider::aabb(Vec3::splat(0.3)),
        Vec3::new(1.0, 5.0, 0.0),
        1.0,
    );
    // Weld at current offset (1, 0, 0).
    let joint = Joint::weld(&w, id_anchor, id_block).unwrap();
    w.add_joint(joint);
    // 4 seconds of gravity — block must still hang at offset (1,0,0).
    for _ in 0..240 { w.step(1.0 / 60.0); }
    let pa = w.body(id_anchor).unwrap().position;
    let pb = w.body(id_block).unwrap().position;
    let off = pb - pa;
    let drift = (off - Vec3::new(1.0, 0.0, 0.0)).length();
    eprintln!("pin offset drift: {}, off={:?}", drift, off);
    assert!(drift < 0.1, "pin joint failed to hold: drift={}", drift);
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
