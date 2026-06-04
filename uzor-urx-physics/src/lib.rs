//! # uzor-urx-physics
//!
//! Lightweight 3D physics for the URX render family — Wave 15+16:
//!
//! - **Collider**: AABB (axis-aligned box) or Sphere.
//! - **Body**: id, position, velocity, mass, restitution, kind
//!   (Dynamic / Static / Kinematic).
//! - **PhysicsWorld**: stores bodies, runs Verlet integration with
//!   gravity, and emits `Contact` events for every overlapping pair.
//!
//! Out of scope (later waves): joints, friction, continuous-collision
//! resolution, mesh colliders, sleeping bodies. The point of this
//! crate is to be ENOUGH for URX 3D demos (falling boxes, ball-on-
//! plane, particle-vs-box) without pulling in `rapier3d`'s 100k LOC.
//!
//! No dependency on `uzor-urx-3d`: that's the consumer's job — pass
//! `Body::position` into the matching `Node::with_translation` each
//! frame.

pub use glam::Vec3;

#[derive(Debug, Clone, Copy)]
pub enum Collider {
    /// Axis-aligned box centred at the body's position with the given
    /// half-extents on each axis.
    Aabb { half_extents: Vec3 },
    /// Sphere centred at the body's position.
    Sphere { radius: f32 },
}

impl Collider {
    pub fn aabb(half_extents: Vec3) -> Self { Self::Aabb { half_extents } }
    pub fn sphere(radius: f32) -> Self { Self::Sphere { radius } }
}

/// Body kind controls how the integrator treats the body.
///
/// - `Static`: never moves. Gravity ignored. Other bodies bounce off.
/// - `Kinematic`: moves but only by direct script (no gravity / impulses
///   applied automatically). Treated as INFINITE mass during contact.
/// - `Dynamic`: gravity + impulses apply; obeys mass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyKind { Dynamic, Static, Kinematic }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BodyId(pub u32);

#[derive(Debug, Clone)]
pub struct Body {
    pub id: BodyId,
    pub kind: BodyKind,
    pub collider: Collider,
    pub position: Vec3,
    pub velocity: Vec3,
    /// Inverse mass (0.0 means infinite — for Static / Kinematic).
    pub inv_mass: f32,
    /// Bounce coefficient in [0, 1]. 0 = inelastic, 1 = elastic.
    pub restitution: f32,
    /// Linear damping per second (velocity *= exp(-damping * dt)).
    pub damping: f32,
}

impl Body {
    pub fn dynamic(id: BodyId, collider: Collider, position: Vec3, mass: f32) -> Self {
        Self {
            id, kind: BodyKind::Dynamic, collider, position,
            velocity: Vec3::ZERO,
            inv_mass: if mass > 1e-6 { 1.0 / mass } else { 0.0 },
            restitution: 0.4,
            damping: 0.05,
        }
    }
    pub fn r#static(id: BodyId, collider: Collider, position: Vec3) -> Self {
        Self {
            id, kind: BodyKind::Static, collider, position,
            velocity: Vec3::ZERO, inv_mass: 0.0, restitution: 0.6, damping: 0.0,
        }
    }
    pub fn kinematic(id: BodyId, collider: Collider, position: Vec3) -> Self {
        Self {
            id, kind: BodyKind::Kinematic, collider, position,
            velocity: Vec3::ZERO, inv_mass: 0.0, restitution: 0.4, damping: 0.0,
        }
    }
}

/// Wave 17 — joint between two bodies.
///
/// All joints are constraint-only — they generate impulses that
/// preserve a relationship every frame. No rotational state means we
/// can't simulate a true 1-DOF hinge axis (that needs body rotation),
/// so HingeAxis is approximated by a `Pin` joint at the hinge point.
#[derive(Debug, Clone, Copy)]
pub enum Joint {
    /// Keep `a` and `b` at exactly `rest_length` apart along the line
    /// connecting their centres. Rod / chain link.
    Distance {
        a: BodyId,
        b: BodyId,
        rest_length: f32,
    },
    /// Pin two bodies together at their CURRENT centre offset.
    /// Useful for welding kinematic and dynamic bodies, or building
    /// rigid compound shapes from primitives. Acts as a ball joint
    /// (3-axis translation locked, no rotation lock since bodies
    /// don't carry rotation in this physics world).
    Pin {
        a: BodyId,
        b: BodyId,
        /// Target world-space offset of b relative to a — captured at
        /// `Pin::weld` time.
        offset: Vec3,
    },
}

impl Joint {
    pub fn distance(a: BodyId, b: BodyId, rest_length: f32) -> Self {
        Self::Distance { a, b, rest_length: rest_length.max(0.0) }
    }
    /// Capture the current world-space offset between `a` and `b` and
    /// freeze it as the pin target. (Bodies must already be in their
    /// pinned configuration when you call this.)
    pub fn weld(world: &PhysicsWorld, a: BodyId, b: BodyId) -> Option<Self> {
        let pa = world.body(a)?.position;
        let pb = world.body(b)?.position;
        Some(Self::Pin { a, b, offset: pb - pa })
    }
}

/// Overlap event — `a` < `b` always (lower-id first) to avoid dupes.
#[derive(Debug, Clone, Copy)]
pub struct Contact {
    pub a: BodyId,
    pub b: BodyId,
    /// World-space contact normal pointing from `a` to `b`.
    pub normal: Vec3,
    /// Penetration depth — positive means overlapping.
    pub depth: f32,
}

pub struct PhysicsWorld {
    pub gravity: Vec3,
    bodies: Vec<Body>,
    next_id: u32,
    joints: Vec<Joint>,
    /// Sequential-impulse solver iterations per step. More = stiffer
    /// joints under load, more CPU.
    pub joint_iterations: u32,
}

impl Default for PhysicsWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl PhysicsWorld {
    pub fn new() -> Self {
        Self {
            gravity: Vec3::new(0.0, -9.81, 0.0),
            bodies: Vec::new(),
            next_id: 0,
            joints: Vec::new(),
            joint_iterations: 8,
        }
    }

    pub fn add_joint(&mut self, j: Joint) { self.joints.push(j); }
    pub fn joints(&self) -> &[Joint] { &self.joints }
    pub fn clear_joints(&mut self) { self.joints.clear(); }

    pub fn fresh_id(&mut self) -> BodyId {
        let id = BodyId(self.next_id);
        self.next_id += 1;
        id
    }

    pub fn add(&mut self, body: Body) -> BodyId {
        let id = body.id;
        self.bodies.push(body);
        id
    }

    /// Convenience: allocate id + insert a dynamic body in one call.
    pub fn spawn_dynamic(&mut self, collider: Collider, position: Vec3, mass: f32) -> BodyId {
        let id = self.fresh_id();
        self.add(Body::dynamic(id, collider, position, mass));
        id
    }

    pub fn spawn_static(&mut self, collider: Collider, position: Vec3) -> BodyId {
        let id = self.fresh_id();
        self.add(Body::r#static(id, collider, position));
        id
    }

    pub fn bodies(&self) -> &[Body] { &self.bodies }
    pub fn bodies_mut(&mut self) -> &mut [Body] { &mut self.bodies }

    pub fn body(&self, id: BodyId) -> Option<&Body> {
        self.bodies.iter().find(|b| b.id == id)
    }
    pub fn body_mut(&mut self, id: BodyId) -> Option<&mut Body> {
        self.bodies.iter_mut().find(|b| b.id == id)
    }

    /// Step the simulation by `dt` seconds.
    ///
    /// 1. Apply gravity to dynamic bodies.
    /// 2. Integrate position (semi-implicit Euler).
    /// 3. Find all overlapping pairs.
    /// 4. Resolve each contact (position correction + impulse).
    /// 5. Return the (post-resolution) contact list.
    pub fn step(&mut self, dt: f32) -> Vec<Contact> {
        for b in &mut self.bodies {
            if b.kind == BodyKind::Dynamic {
                b.velocity += self.gravity * dt;
                // Apply linear damping.
                let damp = (-b.damping * dt).exp();
                b.velocity *= damp;
            }
            if b.kind != BodyKind::Static {
                let v = b.velocity;
                b.position += v * dt;
            }
        }

        let contacts = self.find_contacts();
        for c in &contacts {
            self.resolve_contact(*c);
        }

        // Wave 17 — Sequential Impulse joint solver. Each iteration
        // walks every joint and projects its constraint, accumulating
        // toward stable rest. 8 iterations default is a good balance
        // for chains up to ~6 links.
        for _ in 0..self.joint_iterations {
            for j in 0..self.joints.len() {
                let joint = self.joints[j];
                self.resolve_joint(joint);
            }
        }

        contacts
    }

    fn resolve_joint(&mut self, j: Joint) {
        match j {
            Joint::Distance { a, b, rest_length } => {
                let ia = self.bodies.iter().position(|x| x.id == a);
                let ib = self.bodies.iter().position(|x| x.id == b);
                let (ia, ib) = match (ia, ib) { (Some(a), Some(b)) => (a, b), _ => return };
                let pa = self.bodies[ia].position;
                let pb = self.bodies[ib].position;
                let d = pb - pa;
                let dist = d.length();
                if dist < 1e-6 { return; }
                let n = d / dist;
                let c = dist - rest_length;
                let inv_a = self.bodies[ia].inv_mass;
                let inv_b = self.bodies[ib].inv_mass;
                let inv_sum = inv_a + inv_b;
                if inv_sum < 1e-6 { return; }

                // Position correction.
                let push = n * (c / inv_sum);
                if self.bodies[ia].kind != BodyKind::Static {
                    let p = push * inv_a;
                    self.bodies[ia].position += p;
                }
                if self.bodies[ib].kind != BodyKind::Static {
                    let p = push * inv_b;
                    self.bodies[ib].position -= p;
                }

                // Velocity correction along the constraint axis.
                let rv = self.bodies[ib].velocity - self.bodies[ia].velocity;
                let v_along = rv.dot(n);
                let lambda = -v_along / inv_sum;
                let impulse = n * lambda;
                if self.bodies[ia].kind == BodyKind::Dynamic {
                    let dv = impulse * inv_a;
                    self.bodies[ia].velocity -= dv;
                }
                if self.bodies[ib].kind == BodyKind::Dynamic {
                    let dv = impulse * inv_b;
                    self.bodies[ib].velocity += dv;
                }
            }
            Joint::Pin { a, b, offset } => {
                let ia = self.bodies.iter().position(|x| x.id == a);
                let ib = self.bodies.iter().position(|x| x.id == b);
                let (ia, ib) = match (ia, ib) { (Some(a), Some(b)) => (a, b), _ => return };
                let pa = self.bodies[ia].position;
                let pb = self.bodies[ib].position;
                let target = pa + offset;
                let err = pb - target;
                let err_len = err.length();
                if err_len < 1e-6 { return; }
                let n = err / err_len;
                let inv_a = self.bodies[ia].inv_mass;
                let inv_b = self.bodies[ib].inv_mass;
                let inv_sum = inv_a + inv_b;
                if inv_sum < 1e-6 { return; }

                // Position correction — move bodies until b - a == offset.
                let push = n * (err_len / inv_sum);
                if self.bodies[ia].kind != BodyKind::Static {
                    let p = push * inv_a;
                    self.bodies[ia].position += p;
                }
                if self.bodies[ib].kind != BodyKind::Static {
                    let p = push * inv_b;
                    self.bodies[ib].position -= p;
                }

                // Velocity match — bodies should drift together along
                // any axis (full 3-DOF lock for a pin/ball joint).
                let rv = self.bodies[ib].velocity - self.bodies[ia].velocity;
                let rv_len = rv.length();
                if rv_len < 1e-6 { return; }
                let dir = rv / rv_len;
                let lambda = -rv_len / inv_sum;
                let impulse = dir * lambda;
                if self.bodies[ia].kind == BodyKind::Dynamic {
                    let dv = impulse * inv_a;
                    self.bodies[ia].velocity -= dv;
                }
                if self.bodies[ib].kind == BodyKind::Dynamic {
                    let dv = impulse * inv_b;
                    self.bodies[ib].velocity += dv;
                }
            }
        }
    }

    /// Run only the collision-detection pass without integrating.
    /// Useful when the consumer drives positions externally and only
    /// wants overlap events.
    pub fn find_contacts(&self) -> Vec<Contact> {
        let mut out = Vec::new();
        let n = self.bodies.len();
        for i in 0..n {
            for j in (i + 1)..n {
                if let Some(c) = contact_between(&self.bodies[i], &self.bodies[j]) {
                    out.push(c);
                }
            }
        }
        out
    }

    fn resolve_contact(&mut self, c: Contact) {
        // Find indices.
        let ia = self.bodies.iter().position(|b| b.id == c.a);
        let ib = self.bodies.iter().position(|b| b.id == c.b);
        let (ia, ib) = match (ia, ib) { (Some(a), Some(b)) => (a, b), _ => return };

        let inv_a = self.bodies[ia].inv_mass;
        let inv_b = self.bodies[ib].inv_mass;
        let inv_sum = inv_a + inv_b;
        if inv_sum < 1e-6 { return; } // both infinite-mass, no impulse

        // Position correction (push apart by `depth` along the normal,
        // weighted by inverse mass).
        let correction = c.normal * (c.depth / inv_sum);
        if self.bodies[ia].kind != BodyKind::Static {
            let push = correction * inv_a;
            self.bodies[ia].position -= push;
        }
        if self.bodies[ib].kind != BodyKind::Static {
            let push = correction * inv_b;
            self.bodies[ib].position += push;
        }

        // Velocity impulse.
        let rv = self.bodies[ib].velocity - self.bodies[ia].velocity;
        let rel_vel_normal = rv.dot(c.normal);
        if rel_vel_normal > 0.0 { return; } // separating

        let e = self.bodies[ia].restitution.min(self.bodies[ib].restitution);
        let j = -(1.0 + e) * rel_vel_normal / inv_sum;
        let impulse = c.normal * j;
        if self.bodies[ia].kind == BodyKind::Dynamic {
            let dv = impulse * inv_a;
            self.bodies[ia].velocity -= dv;
        }
        if self.bodies[ib].kind == BodyKind::Dynamic {
            let dv = impulse * inv_b;
            self.bodies[ib].velocity += dv;
        }
    }
}

fn contact_between(a: &Body, b: &Body) -> Option<Contact> {
    let (id_a, id_b) = (a.id, b.id);
    match (a.collider, b.collider) {
        (Collider::Aabb { half_extents: ha }, Collider::Aabb { half_extents: hb }) => {
            aabb_aabb(a.position, ha, b.position, hb).map(|(n, d)| Contact { a: id_a, b: id_b, normal: n, depth: d })
        }
        (Collider::Sphere { radius: ra }, Collider::Sphere { radius: rb }) => {
            sphere_sphere(a.position, ra, b.position, rb).map(|(n, d)| Contact { a: id_a, b: id_b, normal: n, depth: d })
        }
        (Collider::Aabb { half_extents }, Collider::Sphere { radius }) => {
            aabb_sphere(a.position, half_extents, b.position, radius).map(|(n, d)| Contact { a: id_a, b: id_b, normal: n, depth: d })
        }
        (Collider::Sphere { radius }, Collider::Aabb { half_extents }) => {
            aabb_sphere(b.position, half_extents, a.position, radius).map(|(n, d)| Contact { a: id_a, b: id_b, normal: -n, depth: d })
        }
    }
}

fn aabb_aabb(pa: Vec3, ha: Vec3, pb: Vec3, hb: Vec3) -> Option<(Vec3, f32)> {
    let d = pb - pa;
    let overlap = ha + hb - d.abs();
    if overlap.x <= 0.0 || overlap.y <= 0.0 || overlap.z <= 0.0 { return None; }
    // Resolve along axis of LEAST overlap.
    let (axis, depth) = if overlap.x < overlap.y && overlap.x < overlap.z {
        (Vec3::new(d.x.signum(), 0.0, 0.0), overlap.x)
    } else if overlap.y < overlap.z {
        (Vec3::new(0.0, d.y.signum(), 0.0), overlap.y)
    } else {
        (Vec3::new(0.0, 0.0, d.z.signum()), overlap.z)
    };
    Some((axis, depth))
}

fn sphere_sphere(pa: Vec3, ra: f32, pb: Vec3, rb: f32) -> Option<(Vec3, f32)> {
    let d = pb - pa;
    let dist = d.length();
    let r = ra + rb;
    if dist >= r { return None; }
    let n = if dist > 1e-6 { d / dist } else { Vec3::Y };
    Some((n, r - dist))
}

fn aabb_sphere(pa: Vec3, ha: Vec3, pb: Vec3, rb: f32) -> Option<(Vec3, f32)> {
    // Find closest point on the AABB to the sphere centre.
    let min = pa - ha;
    let max = pa + ha;
    let closest = pb.clamp(min, max);
    let d = pb - closest;
    let dist = d.length();
    if dist >= rb { return None; }
    let n = if dist > 1e-6 { d / dist } else {
        // Sphere centre inside AABB — push along the axis where it's
        // closest to a face.
        let to_min = pb - min;
        let to_max = max - pb;
        let axes = [
            (to_min.x, Vec3::NEG_X), (to_max.x, Vec3::X),
            (to_min.y, Vec3::NEG_Y), (to_max.y, Vec3::Y),
            (to_min.z, Vec3::NEG_Z), (to_max.z, Vec3::Z),
        ];
        axes.iter().fold((f32::INFINITY, Vec3::Y), |acc, &(v, a)| {
            if v < acc.0 { (v, a) } else { acc }
        }).1
    };
    Some((n, rb - dist))
}
