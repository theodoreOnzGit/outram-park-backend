// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Independent Rust implementation — reimplemented clean-room from public
// computational-geometry and DEM literature (closest-point-on-triangle,
// rigid-body kinematics, signed-distance functions). NOT derived from
// GPL-2.0 LIGGGHTS/LAMMPS source. See the crate NOTICE.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! Phase 3 (extension) — **Triangulated (mesh) & moving walls** (bead
//! `op-t3l.3` follow-up).
//!
//! The analytic boundaries in [`crate::boundary`] cover infinite planes,
//! half-space walls, axis-aligned boxes, and infinite cylinders. This module
//! adds the two extensions those primitives cannot express:
//!
//! - a **triangulated (STL-style) surface** — [`MeshWall`], a `Vec` of flat
//!   [`Triangle`] faces — so arbitrary complex wall geometry (hoppers, chutes,
//!   impellers, imported CAD/STL meshes) can collide with particles;
//! - **moving / rotating walls** — [`MovingBoundary`], a rigid-body wrapper that
//!   carries a translational velocity and an angular velocity about a pivot,
//!   advances the wrapped geometry in time, and reports the **local surface
//!   velocity** at a contact point.
//!
//! Like [`crate::boundary`], this module is **geometry + kinematics only**. It
//! answers "does the sphere penetrate, by how much (`δ`), along which normal,
//! and how fast is the wall surface moving there?" — it does **not** compute
//! contact forces. The [`Contact`] it returns and the surface velocity it
//! reports are exactly the hand-off a contact-force + wall-friction law (Phase 2
//! [`crate::contact`]) consumes: the friction force needs the particle velocity
//! *relative to the moving wall surface*, `v_rel = v_particle − v_surface`, and
//! [`MovingBoundary::surface_velocity`] supplies the `v_surface` term. The
//! force computation itself stays in the contact model.
//!
//! # Contact convention (shared with [`crate::boundary`])
//!
//! A [`Contact`] (reused from [`crate::boundary::Contact`]) carries the
//! penetration depth `δ = r − d > 0` `[m]` (with `d` the distance from the
//! particle centre to the wall surface), a **unit** contact `normal` pointing
//! from the wall surface into the domain (toward the particle centre for a
//! particle on the outward side), and the `point` on the wall surface. A
//! repulsive penalty force `F = k · δ · normal` then pushes the particle off the
//! wall.
//!
//! # Winding / outward-normal convention
//!
//! Each [`Triangle`] `{a, b, c}` has an outward unit normal
//! `n̂ = normalize((b − a) × (c − a))`. Order the vertices **counter-clockwise
//! as seen from the domain (particle) side**, exactly as the STL format
//! requires, so `n̂` points out of the solid, into the free region where
//! particles live. A mesh wall is treated as one-sided: contact is meaningful
//! for particles approaching from the outward (`+n̂`) side.
//!
//! # Units
//!
//! Distances `[m]`, translational velocity `[m/s]`, angular velocity `[rad/s]`
//! (all `f64` in SI base units, matching [`crate::particle`]). Vertices,
//! contact points, pivots are positions `[m]`; the contact normal is
//! dimensionless.
//!
//! # Honest scope (this extension)
//!
//! A **verified geometric + kinematic foundation only** — it has **not** been
//! validated against a DEM reference code (that is the later human validation
//! step in this bead's Definition of Done; the tests below check hand-computed
//! geometry and closed-form rigid-body kinematics). Concretely:
//!
//! - **Flat triangles only** — no curved/higher-order surface patches (Bézier,
//!   NURBS, subdivision); a curved wall must be pre-tessellated into triangles
//!   by the caller.
//! - **No self-collision** and no mesh-consistency checks: the triangles are
//!   assumed to form a sensible, non-self-intersecting surface. Overlapping or
//!   inconsistent-winding triangles are not detected.
//! - **Nearest-triangle search is O(N_tri) brute force** — every query tests
//!   every triangle. There is **no BVH / spatial acceleration structure yet**;
//!   this is adequate for small meshes and for verification, not for large
//!   production meshes (a BVH is future work).
//! - **One-sided contact.** The reported normal is the nearest triangle's stored
//!   outward normal; correct behaviour assumes the particle is on the outward
//!   side. A particle that has tunnelled fully behind a thin single-triangle
//!   sheet is not specially handled.
//! - **When the closest point lies on a shared edge or vertex** of the mesh, the
//!   contact normal is taken from whichever incident triangle is found nearest
//!   (ties broken by iteration order); the penetration `δ = r − d` uses the true
//!   Euclidean distance `d` to that closest point. A blended/averaged edge
//!   normal is not computed.
//! - **Rigid-body wall motion only** ([`MovingBoundary`]): pure translation plus
//!   rotation about a single moving pivot. No wall deformation, no per-vertex
//!   velocity fields. Rotation is applied to [`Boundary::Plane`],
//!   [`Boundary::Wall`], [`Boundary::Cylinder`], and [`MeshWall`] geometry; an
//!   **axis-aligned [`Boundary::Box`] is translated but not rotated** (a rotated
//!   AABB is no longer axis-aligned — wrap a [`MeshWall`] to rotate a
//!   box-shaped container).
//!
//! # References (public literature — NOT LAMMPS/LIGGGHTS source)
//!
//! - C. Ericson, *Real-Time Collision Detection* (Morgan Kaufmann, 2005),
//!   **§5.1.5 "Closest Point on Triangle to Point"** — the Voronoi-region
//!   barycentric closest-point algorithm used in [`Triangle::closest_point`].
//! - P. J. Schneider and D. H. Eberly, *Geometric Tools for Computer Graphics*
//!   (Morgan Kaufmann, 2003) — point/triangle distance geometry.
//! - O. Rodrigues, "Des lois géométriques qui régissent les déplacements d'un
//!   système solide…," *J. Math. Pures Appl.* **5**, 380–440 (1840) — the
//!   axis–angle rotation ("Rodrigues") formula used in [`MovingBoundary::advance`].
//! - H. Goldstein, C. Poole, J. Safko, *Classical Mechanics*, 3rd ed.
//!   (Addison-Wesley, 2002) — rigid-body kinematics, the surface-velocity
//!   relation `v = v_cm + ω × r`.
//! - T. Pöschel and T. Schwager, *Computational Granular Dynamics: Models and
//!   Algorithms* (Springer, 2005) — particle–wall overlap, contact-normal
//!   conventions, and relative-velocity handling for moving walls.
//! - J. Chen, A. B. Yu, et al. — the standard DEM triangulated-wall (STL)
//!   treatment in the granular literature (closest-point-on-facet contact
//!   detection); this is an independent reimplementation of that public method.

use crate::boundary::{Boundary, Contact};
use crate::particle::{Particle, Vec3};
use crate::DemError;

/// Normalise a vector to unit length: `v / ‖v‖`.
///
/// Callers pass a vector already known to be non-zero (triangle normals are
/// validated at construction; rotation axes are checked before use), so this is
/// an infallible helper. Passing a zero vector yields a non-finite result.
#[inline]
fn unit(v: Vec3) -> Vec3 {
    v.scale(1.0 / v.norm())
}

/// Rotate vector `v` about the **unit** axis `axis` by `angle` `[rad]`, using
/// the Rodrigues rotation formula.
///
/// `v_rot = v·cosθ + (axis × v)·sinθ + axis·(axis · v)·(1 − cosθ)`.
///
/// `axis` must be a unit vector and `v` any vector in the same frame; the result
/// carries the same unit as `v`. Used by [`MovingBoundary::advance`] to rotate
/// wall geometry rigidly about its pivot.
#[inline]
fn rotate_vector(v: Vec3, axis: Vec3, angle: f64) -> Vec3 {
    let (sin_t, cos_t) = angle.sin_cos();
    let cross = axis.cross(v);
    let dot = axis.dot(v);
    v.scale(cos_t)
        .add(cross.scale(sin_t))
        .add(axis.scale(dot * (1.0 - cos_t)))
}

/// A single flat triangular facet `{a, b, c}` of a triangulated wall surface.
///
/// # Fields and units
///
/// | Field | Quantity | SI unit |
/// |---|---|---|
/// | `a`, `b`, `c` | the three vertex positions | `[m]` |
///
/// # Outward normal and winding
///
/// The outward unit normal is `n̂ = normalize((b − a) × (c − a))` (see
/// [`Triangle::normal`]). Order the vertices **counter-clockwise as seen from
/// the domain (particle) side** so `n̂` points out of the solid toward the
/// particles — the same right-hand winding the STL file format uses. The three
/// vertices must not be collinear (a zero-area triangle has no defined normal);
/// [`Triangle::new`] enforces this.
///
/// `Copy` and stored inline (no heap allocation); a [`MeshWall`] owns a `Vec` of
/// these by value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Triangle {
    /// First vertex `[m]`.
    pub a: Vec3,
    /// Second vertex `[m]`.
    pub b: Vec3,
    /// Third vertex `[m]`.
    pub c: Vec3,
}

impl Triangle {
    /// Construct a validated triangle from three vertices `[m]`.
    ///
    /// The vertices must be **counter-clockwise as seen from the domain side**
    /// (see the type and module docs) so the outward normal points toward the
    /// particles.
    ///
    /// # Errors
    ///
    /// Returns [`DemError::InvalidInput`] if the three vertices are collinear (or
    /// coincident) — i.e. the edge cross product `(b − a) × (c − a)` has
    /// (near-)zero length — since a degenerate, zero-area triangle has no
    /// defined outward normal.
    pub fn new(a: Vec3, b: Vec3, c: Vec3) -> Result<Self, DemError> {
        let cross = b.sub(a).cross(c.sub(a));
        // `> 0.0` (not `>=`) also rejects a NaN, so no non-finite normal slips
        // through. The magnitude of the edge cross product is twice the area.
        if cross.norm() > 0.0 {
            Ok(Self { a, b, c })
        } else {
            Err(DemError::InvalidInput(
                "triangle vertices must not be collinear (zero-area facet has no normal)"
                    .to_string(),
            ))
        }
    }

    /// The outward **unit** normal `n̂ = normalize((b − a) × (c − a))`
    /// (dimensionless).
    ///
    /// Points out of the solid, into the domain, for the counter-clockwise
    /// winding described on the type. A directly field-constructed degenerate
    /// (collinear) triangle yields a non-finite result; use [`Triangle::new`] to
    /// reject that up front.
    #[must_use]
    pub fn normal(&self) -> Vec3 {
        unit(self.b.sub(self.a).cross(self.c.sub(self.a)))
    }

    /// The point on this triangle (interior, edge, or vertex) closest to `p`
    /// `[m]`, by the Voronoi-region barycentric method of Ericson,
    /// *Real-Time Collision Detection* (2005), **§5.1.5**.
    ///
    /// The algorithm classifies `p` against the triangle's seven Voronoi
    /// regions (three vertices, three edges, one face) using signed dot products,
    /// then returns the exact closest point:
    ///
    /// - a **vertex** if `p` projects outside two adjacent edges,
    /// - a point on an **edge** if `p` projects outside one edge only,
    /// - the perpendicular foot inside the **face** otherwise.
    ///
    /// It is branch-exact (no iteration) and returns a point in `[m]`. This is an
    /// independent reimplementation of the published textbook algorithm — not
    /// copied from any collision-detection library source.
    #[must_use]
    pub fn closest_point(&self, p: Vec3) -> Vec3 {
        let a = self.a;
        let b = self.b;
        let c = self.c;
        let ab = b.sub(a);
        let ac = c.sub(a);
        let ap = p.sub(a);

        let d1 = ab.dot(ap);
        let d2 = ac.dot(ap);
        // Vertex region A: barycentric (1, 0, 0).
        if d1 <= 0.0 && d2 <= 0.0 {
            return a;
        }

        let bp = p.sub(b);
        let d3 = ab.dot(bp);
        let d4 = ac.dot(bp);
        // Vertex region B: barycentric (0, 1, 0).
        if d3 >= 0.0 && d4 <= d3 {
            return b;
        }

        // Edge region AB: barycentric (1 − v, v, 0).
        let vc = d1 * d4 - d3 * d2;
        if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
            let v = d1 / (d1 - d3);
            return a.add(ab.scale(v));
        }

        let cp = p.sub(c);
        let d5 = ab.dot(cp);
        let d6 = ac.dot(cp);
        // Vertex region C: barycentric (0, 0, 1).
        if d6 >= 0.0 && d5 <= d6 {
            return c;
        }

        // Edge region AC: barycentric (1 − w, 0, w).
        let vb = d5 * d2 - d1 * d6;
        if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
            let w = d2 / (d2 - d6);
            return a.add(ac.scale(w));
        }

        // Edge region BC: barycentric (0, 1 − w, w).
        let va = d3 * d6 - d5 * d4;
        if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
            let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
            return b.add(c.sub(b).scale(w));
        }

        // Face region: barycentric (1 − v − w, v, w) via the barycentric
        // coordinates of the perpendicular projection.
        let denom = 1.0 / (va + vb + vc);
        let v = vb * denom;
        let w = vc * denom;
        a.add(ab.scale(v)).add(ac.scale(w))
    }
}

/// A **triangulated (STL-style) wall**: a surface made of flat [`Triangle`]
/// facets.
///
/// Owns its facets by value in a `Vec` (indexed by `usize`), per the workspace
/// design rules — no trait objects, no lifetimes. Build one from any triangle
/// soup; the facets are assumed to share the winding convention on [`Triangle`]
/// so their outward normals all point into the domain.
///
/// [`MeshWall::particle_overlap`] finds, over **all** triangles (O(N_tri) brute
/// force — see the module "Honest scope"), the facet whose closest point is
/// nearest the particle centre, and reports the contact there.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshWall {
    /// The triangular facets `[m]`. Assumed consistently wound (outward normals
    /// point into the domain) and to form a sensible surface; see the module
    /// "Honest scope" for what is *not* checked.
    pub triangles: Vec<Triangle>,
}

impl MeshWall {
    /// Construct a mesh wall from a non-empty list of facets.
    ///
    /// # Errors
    ///
    /// Returns [`DemError::InvalidInput`] if `triangles` is empty (an empty mesh
    /// is not a surface). Individual facet validity (non-degeneracy) is the
    /// caller's responsibility — build each facet with [`Triangle::new`] to have
    /// it checked.
    pub fn new(triangles: Vec<Triangle>) -> Result<Self, DemError> {
        if triangles.is_empty() {
            return Err(DemError::InvalidInput(
                "mesh wall must contain at least one triangle".to_string(),
            ));
        }
        Ok(Self { triangles })
    }

    /// Geometric overlap of particle `p` (sphere of radius `r = p.radius` centred
    /// at `c = p.position`) with the nearest facet of this mesh.
    ///
    /// Over all triangles, finds the one whose [`Triangle::closest_point`] to `c`
    /// is at the smallest Euclidean distance `d`. If the sphere reaches that
    /// facet — penetration `δ = r − d > 0` — returns `Some(`[`Contact`]`)` with:
    ///
    /// - `overlap = δ` `[m]`,
    /// - `normal` = the nearest facet's outward unit normal (the surface → domain
    ///   direction the wall pushes along),
    /// - `point` = the closest point on that facet `[m]` (the contact point on
    ///   the mesh surface).
    ///
    /// Returns `None` if no facet is within `r`, or if the mesh has no triangles.
    /// This is geometry only — no force is computed (see the module docs). See
    /// the module "Honest scope" for the edge/vertex and one-sidedness caveats.
    #[must_use]
    pub fn particle_overlap(&self, p: &Particle) -> Option<Contact> {
        let c = p.position;
        let r = p.radius;

        let mut best: Option<(f64, Vec3, Vec3)> = None; // (distance, closest point, normal)
        for tri in &self.triangles {
            let q = tri.closest_point(c);
            let dist = c.sub(q).norm();
            let closer = match best {
                Some((best_dist, _, _)) => dist < best_dist,
                None => true,
            };
            if closer {
                best = Some((dist, q, tri.normal()));
            }
        }

        let (dist, point, normal) = best?;
        let delta = r - dist;
        // `> 0.0` (not `>=`) also yields None for a NaN depth.
        if delta > 0.0 {
            Some(Contact {
                overlap: delta,
                normal,
                point,
            })
        } else {
            None
        }
    }
}

/// The wall geometry a [`MovingBoundary`] carries: either an analytic
/// [`Boundary`] or a triangulated [`MeshWall`].
///
/// Enum dispatch (no `Box<dyn>`), per the workspace design rules — the set of
/// wall geometries is closed and known at compile time. This lets one
/// [`MovingBoundary`] type wrap *any* wall shape without trait objects.
#[derive(Debug, Clone, PartialEq)]
pub enum WallGeometry {
    /// An analytic boundary primitive (plane, half-space wall, box, cylinder).
    Analytic(Boundary),
    /// A triangulated (STL-style) mesh wall.
    Mesh(MeshWall),
}

impl WallGeometry {
    /// Geometric overlap of particle `p` with the wrapped geometry, delegating to
    /// [`Boundary::particle_overlap`] or [`MeshWall::particle_overlap`].
    ///
    /// Returns the static-geometry [`Contact`] (penetration `δ`, normal, point)
    /// in the wall's *current* pose. The wall's surface velocity is a separate
    /// query on [`MovingBoundary`]; see [`MovingBoundary::surface_velocity`].
    #[must_use]
    pub fn particle_overlap(&self, p: &Particle) -> Option<Contact> {
        match self {
            WallGeometry::Analytic(b) => b.particle_overlap(p),
            WallGeometry::Mesh(m) => m.particle_overlap(p),
        }
    }

    /// Translate the geometry rigidly by displacement `disp` `[m]`.
    fn translate(&mut self, disp: Vec3) {
        match self {
            WallGeometry::Analytic(b) => match b {
                Boundary::Plane { point, .. } | Boundary::Wall { point, .. } => {
                    *point = point.add(disp);
                }
                Boundary::Box { min, max } => {
                    *min = min.add(disp);
                    *max = max.add(disp);
                }
                Boundary::Cylinder { axis_point, .. } => {
                    *axis_point = axis_point.add(disp);
                }
            },
            WallGeometry::Mesh(m) => {
                for tri in m.triangles.iter_mut() {
                    tri.a = tri.a.add(disp);
                    tri.b = tri.b.add(disp);
                    tri.c = tri.c.add(disp);
                }
            }
        }
    }

    /// Rotate the geometry rigidly about `pivot` `[m]` by `angle` `[rad]` around
    /// the **unit** `axis`.
    ///
    /// Reference points are rotated about the pivot; direction vectors (plane /
    /// wall normals, cylinder axis) are rotated in place. An axis-aligned
    /// [`Boundary::Box`] is intentionally **not** rotated (a rotated AABB is no
    /// longer axis-aligned) — see the module "Honest scope".
    fn rotate(&mut self, pivot: Vec3, axis: Vec3, angle: f64) {
        let rotate_point = |point: Vec3| pivot.add(rotate_vector(point.sub(pivot), axis, angle));
        let rotate_dir = |dir: Vec3| rotate_vector(dir, axis, angle);
        match self {
            WallGeometry::Analytic(b) => match b {
                Boundary::Plane { point, normal } | Boundary::Wall { point, normal } => {
                    *point = rotate_point(*point);
                    *normal = rotate_dir(*normal);
                }
                Boundary::Cylinder {
                    axis_point,
                    axis_dir,
                    ..
                } => {
                    *axis_point = rotate_point(*axis_point);
                    *axis_dir = rotate_dir(*axis_dir);
                }
                // Axis-aligned box cannot rotate and stay axis-aligned: left
                // as-is by design (documented in the module "Honest scope").
                Boundary::Box { .. } => {}
            },
            WallGeometry::Mesh(m) => {
                for tri in m.triangles.iter_mut() {
                    tri.a = rotate_point(tri.a);
                    tri.b = rotate_point(tri.b);
                    tri.c = rotate_point(tri.c);
                }
            }
        }
    }
}

/// A **moving / rotating rigid wall**: a [`WallGeometry`] plus its rigid-body
/// kinematic state (translational velocity, angular velocity about a pivot).
///
/// # Purpose
///
/// This supplies the two things a moving-wall contact needs beyond static
/// geometry:
///
/// 1. [`MovingBoundary::advance`] moves the geometry forward one time step so the
///    next overlap query sees the wall in its new pose;
/// 2. [`MovingBoundary::surface_velocity`] gives the wall's material velocity at
///    a contact point, so the caller can form the particle-relative velocity
///    `v_rel = v_particle − v_surface` that a wall-friction / damping law
///    consumes.
///
/// **Force computation stays in the contact model.** This type provides geometry
/// (via [`MovingBoundary::particle_overlap`]) and kinematics only — no force.
///
/// # Fields and units
///
/// | Field | Quantity | SI unit |
/// |---|---|---|
/// | `geometry` | the wall shape (analytic or mesh) | positions `[m]` |
/// | `velocity` | translational velocity of the pivot / body | `[m/s]` |
/// | `angular_velocity` | angular velocity `ω` about the pivot | `[rad/s]` |
/// | `pivot` | the point the rotation is taken about | `[m]` |
///
/// The angular-velocity vector's direction is the rotation axis and its
/// magnitude the rotation rate (right-hand rule). A purely translating wall has
/// `angular_velocity = 0`; a wall spinning about a fixed axis has
/// `velocity = 0`.
#[derive(Debug, Clone, PartialEq)]
pub struct MovingBoundary {
    /// The wall geometry, in its current pose.
    pub geometry: WallGeometry,
    /// Translational velocity of the rigid body `[m/s]`.
    pub velocity: Vec3,
    /// Angular velocity `ω` about [`MovingBoundary::pivot`] `[rad/s]` (direction
    /// = axis, magnitude = rate).
    pub angular_velocity: Vec3,
    /// The pivot point rotation is taken about `[m]`. Translates with the body
    /// under [`MovingBoundary::advance`].
    pub pivot: Vec3,
}

impl MovingBoundary {
    /// Wrap `geometry` as a rigid wall with translational `velocity` `[m/s]`,
    /// `angular_velocity` `[rad/s]` about `pivot` `[m]`.
    ///
    /// No validation is needed: any velocity/pivot is physically meaningful (a
    /// zero angular velocity gives pure translation; a zero translational
    /// velocity gives pure rotation about the pivot).
    #[must_use]
    pub fn new(
        geometry: WallGeometry,
        velocity: Vec3,
        angular_velocity: Vec3,
        pivot: Vec3,
    ) -> Self {
        Self {
            geometry,
            velocity,
            angular_velocity,
            pivot,
        }
    }

    /// Material velocity of the wall surface at world point `point` `[m]`,
    /// `[m/s]`.
    ///
    /// For a rigid body, the velocity of the material point currently at `point`
    /// is
    ///
    /// `v_surface = velocity + ω × (point − pivot)`
    ///
    /// (Goldstein, *Classical Mechanics*, rigid-body kinematics). The caller
    /// forms the particle-relative surface velocity `v_rel = v_particle −
    /// v_surface` for the wall-friction law — this method returns only the
    /// `v_surface` term, no force.
    ///
    /// `point` is typically a [`Contact::point`] from
    /// [`MovingBoundary::particle_overlap`], but any world point is valid.
    #[must_use]
    pub fn surface_velocity(&self, point: Vec3) -> Vec3 {
        self.velocity
            .add(self.angular_velocity.cross(point.sub(self.pivot)))
    }

    /// Geometric overlap of particle `p` with the wall in its **current** pose,
    /// delegating to [`WallGeometry::particle_overlap`].
    ///
    /// Returns the static-geometry [`Contact`] (penetration `δ`, contact normal,
    /// contact point). Pair the contact point with
    /// [`MovingBoundary::surface_velocity`] to get the wall velocity there. No
    /// force is computed.
    #[must_use]
    pub fn particle_overlap(&self, p: &Particle) -> Option<Contact> {
        self.geometry.particle_overlap(p)
    }

    /// Advance the wall one time step `dt` `[s]`: rotate the geometry about the
    /// pivot by `ω·dt`, then translate it (and the pivot) by `velocity·dt`.
    ///
    /// # Scheme
    ///
    /// The rigid update applied to every geometry reference point `x` is
    ///
    /// `x(t+dt) = pivot + R(ω, |ω|·dt)·(x − pivot) + velocity·dt`,
    ///
    /// with `R` the Rodrigues axis–angle rotation about the unit axis `ω/|ω|` by
    /// angle `|ω|·dt`, and the pivot itself advances to `pivot + velocity·dt`.
    /// Direction vectors (normals, cylinder axis) are rotated but not translated.
    /// When `ω = 0` the rotation is skipped (pure translation). An axis-aligned
    /// [`Boundary::Box`] is translated but not rotated (see the module "Honest
    /// scope").
    ///
    /// This is a **first-order explicit** pose update: for a constant `ω` the
    /// single-step rotation angle `|ω|·dt` is exact, so a constant-rate spin is
    /// reproduced exactly per step (as the rotation test confirms); for a
    /// time-varying `ω` the caller must keep `dt` small. It advances geometry
    /// only — the wall's `velocity`/`angular_velocity` are unchanged (a rigid
    /// wall is kinematically prescribed here, not force-driven).
    pub fn advance(&mut self, dt: f64) {
        let omega = self.angular_velocity;
        let rate = omega.norm();
        if rate > 0.0 {
            let axis = omega.scale(1.0 / rate);
            let angle = rate * dt;
            self.geometry.rotate(self.pivot, axis, angle);
        }
        let disp = self.velocity.scale(dt);
        self.geometry.translate(disp);
        self.pivot = self.pivot.add(disp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;
    use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
    use uom::si::length::meter;
    use uom::si::mass::kilogram;
    use uom::si::thermodynamic_temperature::kelvin;

    /// Helper: a valid particle centred at `center` `[m]` with radius `radius_m`.
    fn particle_at(center: Vec3, radius_m: f64) -> Particle {
        Particle::new(
            center,
            Vec3::zero(),
            Vec3::zero(),
            Mass::new::<kilogram>(1.0),
            Length::new::<meter>(radius_m),
            ThermodynamicTemperature::new::<kelvin>(300.0),
        )
        .expect("valid particle")
    }

    /// The canonical right-triangle used in the closest-point tests: `a = (0,0,0)`,
    /// `b = (1,0,0)`, `c = (0,1,0)` in the `z = 0` plane, outward normal `+ẑ`.
    fn unit_triangle() -> Triangle {
        Triangle::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        )
        .expect("non-degenerate triangle")
    }

    /// V&V — **closest point on triangle: face, edge, and vertex regions**
    /// (Ericson §5.1.5).
    ///
    /// Methodology: use the right triangle `a=(0,0,0)`, `b=(1,0,0)`, `c=(0,1,0)`
    /// in the `z=0` plane (outward normal `+ẑ`) and query three points, one per
    /// Voronoi-region class, each with a hand-computed closest point:
    ///
    /// - **Face region** — `p = (0.2, 0.2, 0.7)`. The perpendicular foot
    ///   `(0.2, 0.2, 0)` lies inside the triangle (`0.2 + 0.2 = 0.4 ≤ 1`), so the
    ///   closest point is `(0.2, 0.2, 0)`, distance `0.7`.
    /// - **Edge region (AB)** — `p = (0.5, −0.3, 0.4)`. Its projection
    ///   `(0.5, −0.3, 0)` is outside the triangle (`y < 0`); the closest point is
    ///   the foot on edge AB, `(0.5, 0, 0)`, distance
    ///   `√(0.3² + 0.4²) = 0.5`.
    /// - **Vertex region (C)** — `p = (−0.3, 1.4, 0.5)`. It projects beyond
    ///   vertex `c`, so the closest point is `c = (0, 1, 0)`, distance
    ///   `√(0.3² + 0.4² + 0.5²) = √0.5 ≈ 0.70710678`.
    ///
    /// Pass criterion: closest-point components exact to `1e-12`.
    ///
    /// Result (measured 2026-07-24): closest points `(0.2, 0.2, 0)`,
    /// `(0.5, 0, 0)`, `(0, 1, 0)` reproduced to `< 1e-12`; the corresponding
    /// distances `0.7`, `0.5`, `0.70710678…` match the hand-computed values,
    /// confirming the face, edge, and vertex branches of the §5.1.5 algorithm.
    #[test]
    fn closest_point_on_triangle_face_edge_vertex() {
        let tri = unit_triangle();

        // Face region.
        let q_face = tri.closest_point(Vec3::new(0.2, 0.2, 0.7));
        assert_abs_diff_eq!(q_face.x, 0.2, epsilon = 1e-12);
        assert_abs_diff_eq!(q_face.y, 0.2, epsilon = 1e-12);
        assert_abs_diff_eq!(q_face.z, 0.0, epsilon = 1e-12);

        // Edge region AB.
        let q_edge = tri.closest_point(Vec3::new(0.5, -0.3, 0.4));
        assert_abs_diff_eq!(q_edge.x, 0.5, epsilon = 1e-12);
        assert_abs_diff_eq!(q_edge.y, 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(q_edge.z, 0.0, epsilon = 1e-12);

        // Vertex region C.
        let q_vert = tri.closest_point(Vec3::new(-0.3, 1.4, 0.5));
        assert_abs_diff_eq!(q_vert.x, 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(q_vert.y, 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(q_vert.z, 0.0, epsilon = 1e-12);
    }

    /// V&V — **triangle outward normal from winding**.
    ///
    /// Methodology: for the counter-clockwise right triangle `a=(0,0,0)`,
    /// `b=(1,0,0)`, `c=(0,1,0)`, the normal is
    /// `normalize((b−a)×(c−a)) = normalize((1,0,0)×(0,1,0)) = (0,0,1)`. Pass
    /// criterion: components exact to `1e-15`.
    ///
    /// Result (measured 2026-07-24): `(0, 0, 1)` to `< 1e-15`; a degenerate
    /// collinear triple is rejected by [`Triangle::new`].
    #[test]
    fn triangle_normal_and_degeneracy() {
        let n = unit_triangle().normal();
        assert_abs_diff_eq!(n.x, 0.0, epsilon = 1e-15);
        assert_abs_diff_eq!(n.y, 0.0, epsilon = 1e-15);
        assert_abs_diff_eq!(n.z, 1.0, epsilon = 1e-15);

        // Collinear vertices → no defined normal → rejected.
        assert!(Triangle::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
        )
        .is_err());
        // Empty mesh → rejected.
        assert!(MeshWall::new(Vec::new()).is_err());
    }

    /// V&V — **sphere penetrating a single triangle: overlap depth and normal**.
    ///
    /// Methodology: the right triangle `a=(0,0,0)`, `b=(1,0,0)`, `c=(0,1,0)`
    /// (outward normal `+ẑ`) as a one-facet [`MeshWall`]. A sphere of radius
    /// `r = 0.5` centred at `c = (0.2, 0.2, 0.3)` sits above the face interior:
    /// closest point `(0.2, 0.2, 0)`, distance `d = 0.3`, so overlap
    /// `δ = r − d = 0.2`, contact normal `+ẑ`, contact point `(0.2, 0.2, 0)`. A
    /// sphere held clear at `c = (0.2, 0.2, 0.9)`, `r = 0.5` (`d = 0.9 > r`)
    /// reports no contact. Pass criterion: `δ`, normal, and point exact to
    /// `1e-12`.
    ///
    /// Result (measured 2026-07-24): penetrating case `δ = 0.2`,
    /// `n̂ = (0, 0, 1)`, contact point `(0.2, 0.2, 0)`; clear case `None`. All to
    /// `< 1e-12`, matching the hand-computed `δ = r − d`.
    #[test]
    fn sphere_penetrating_single_triangle() {
        let mesh = MeshWall::new(vec![unit_triangle()]).unwrap();

        let contact = mesh
            .particle_overlap(&particle_at(Vec3::new(0.2, 0.2, 0.3), 0.5))
            .expect("penetration");
        assert_abs_diff_eq!(contact.overlap, 0.2, epsilon = 1e-12);
        assert_abs_diff_eq!(contact.normal.x, 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(contact.normal.y, 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(contact.normal.z, 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(contact.point.x, 0.2, epsilon = 1e-12);
        assert_abs_diff_eq!(contact.point.y, 0.2, epsilon = 1e-12);
        assert_abs_diff_eq!(contact.point.z, 0.0, epsilon = 1e-12);

        // Held clear of the facet: no contact.
        assert!(mesh
            .particle_overlap(&particle_at(Vec3::new(0.2, 0.2, 0.9), 0.5))
            .is_none());
    }

    /// V&V — **two-triangle quad wall matches the analytic plane**.
    ///
    /// Methodology: tessellate the square `[−1, 1] × [−1, 1]` in the `z = 0`
    /// plane into two triangles wound so both outward normals are `+ẑ`
    /// (`t1 = (−1,−1,0),(1,−1,0),(1,1,0)`; `t2 = (−1,−1,0),(1,1,0),(−1,1,0)`), and
    /// compare a centred sphere's [`MeshWall`] overlap against the analytic
    /// [`Boundary::Plane`] through the origin with normal `+ẑ`. For a sphere
    /// `r = 0.5` centred at `c = (0, 0, 0.4)`: distance `d = 0.4`, overlap
    /// `δ = 0.1`, normal `+ẑ`, contact point `(0, 0, 0)`. The mesh centre projects
    /// onto the shared diagonal, whose closest point is `(0, 0, 0)` on either
    /// facet — identical to the plane. Pass criterion: mesh and plane agree, and
    /// both hit the hand-computed values, to `1e-12`.
    ///
    /// Result (measured 2026-07-24): plane and mesh both give `δ = 0.1`,
    /// `n̂ = (0, 0, 1)`, contact point `(0, 0, 0)`, agreeing to `< 1e-12`. This
    /// confirms a flat triangulated wall reproduces the analytic half-plane for a
    /// centred contact.
    #[test]
    fn two_triangle_quad_matches_plane() {
        let t1 = Triangle::new(
            Vec3::new(-1.0, -1.0, 0.0),
            Vec3::new(1.0, -1.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
        )
        .unwrap();
        let t2 = Triangle::new(
            Vec3::new(-1.0, -1.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(-1.0, 1.0, 0.0),
        )
        .unwrap();
        // Both facets wound for a +ẑ outward normal.
        assert_abs_diff_eq!(t1.normal().z, 1.0, epsilon = 1e-15);
        assert_abs_diff_eq!(t2.normal().z, 1.0, epsilon = 1e-15);

        let mesh = MeshWall::new(vec![t1, t2]).unwrap();
        let plane = Boundary::plane(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).unwrap();

        let p = particle_at(Vec3::new(0.0, 0.0, 0.4), 0.5);
        let cm = mesh.particle_overlap(&p).expect("mesh contact");
        let cp = plane.particle_overlap(&p).expect("plane contact");

        // Mesh matches the hand-computed values …
        assert_abs_diff_eq!(cm.overlap, 0.1, epsilon = 1e-12);
        assert_abs_diff_eq!(cm.normal.z, 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(cm.point.x, 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(cm.point.y, 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(cm.point.z, 0.0, epsilon = 1e-12);
        // … and matches the analytic plane.
        assert_abs_diff_eq!(cm.overlap, cp.overlap, epsilon = 1e-12);
        assert_abs_diff_eq!(cm.normal.x, cp.normal.x, epsilon = 1e-12);
        assert_abs_diff_eq!(cm.normal.y, cp.normal.y, epsilon = 1e-12);
        assert_abs_diff_eq!(cm.normal.z, cp.normal.z, epsilon = 1e-12);
        assert_abs_diff_eq!(cm.point.z, cp.point.z, epsilon = 1e-12);
    }

    /// V&V — **translating wall: surface velocity equals the translational
    /// velocity**.
    ///
    /// Methodology: a moving wall (any geometry) with translational velocity
    /// `v = (1, 2, 3) m/s` and **zero** angular velocity. For a rigid body the
    /// material velocity is `v_surface = v + ω × r = v` everywhere, independent
    /// of the query point. Query two distinct points; both must return
    /// `(1, 2, 3)`. Pass criterion: exact to `1e-15`.
    ///
    /// Result (measured 2026-07-24): both `(4, 5, 6)` and `(−2, 0, 7)` return
    /// `v_surface = (1, 2, 3) m/s` to `< 1e-15`, confirming a pure translation is
    /// spatially uniform.
    #[test]
    fn translating_wall_surface_velocity_equals_velocity() {
        let plane = Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mb = MovingBoundary::new(
            WallGeometry::Analytic(plane),
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::zero(),
            Vec3::zero(),
        );

        for q in [Vec3::new(4.0, 5.0, 6.0), Vec3::new(-2.0, 0.0, 7.0)] {
            let v = mb.surface_velocity(q);
            assert_abs_diff_eq!(v.x, 1.0, epsilon = 1e-15);
            assert_abs_diff_eq!(v.y, 2.0, epsilon = 1e-15);
            assert_abs_diff_eq!(v.z, 3.0, epsilon = 1e-15);
        }
    }

    /// V&V — **rotating wall: surface velocity equals `ω × r`**.
    ///
    /// Methodology: a wall spinning about the z-axis through the origin,
    /// `ω = (0, 0, 2) rad/s`, zero translational velocity, pivot at the origin.
    /// The rigid-body surface velocity is `v = ω × (point − pivot)`. At
    /// `point = (3, 0, 0)` (`r = (3, 0, 0)`),
    /// `ω × r = (0,0,2) × (3,0,0) = (0, 6, 0) m/s`. With a nonzero pivot
    /// `pivot = (1, 0, 0)` and the same point, `r = (2, 0, 0)` and
    /// `ω × r = (0, 4, 0) m/s`. Pass criterion: exact to `1e-15`, and the direct
    /// `ω × r` evaluation must agree.
    ///
    /// Result (measured 2026-07-24): origin-pivot case `v = (0, 6, 0)`,
    /// shifted-pivot case `v = (0, 4, 0)`, both to `< 1e-15` and matching a direct
    /// cross-product evaluation.
    #[test]
    fn rotating_wall_surface_velocity_equals_omega_cross_r() {
        let mesh = MeshWall::new(vec![unit_triangle()]).unwrap();
        let omega = Vec3::new(0.0, 0.0, 2.0);

        // Pivot at the origin.
        let mb = MovingBoundary::new(
            WallGeometry::Mesh(mesh.clone()),
            Vec3::zero(),
            omega,
            Vec3::zero(),
        );
        let point = Vec3::new(3.0, 0.0, 0.0);
        let v = mb.surface_velocity(point);
        let expected = omega.cross(point.sub(Vec3::zero()));
        assert_abs_diff_eq!(v.x, expected.x, epsilon = 1e-15);
        assert_abs_diff_eq!(v.y, expected.y, epsilon = 1e-15);
        assert_abs_diff_eq!(v.z, expected.z, epsilon = 1e-15);
        assert_abs_diff_eq!(v.x, 0.0, epsilon = 1e-15);
        assert_abs_diff_eq!(v.y, 6.0, epsilon = 1e-15);
        assert_abs_diff_eq!(v.z, 0.0, epsilon = 1e-15);

        // Shifted pivot: r = point − pivot = (2, 0, 0), ω × r = (0, 4, 0).
        let pivot = Vec3::new(1.0, 0.0, 0.0);
        let mb2 = MovingBoundary::new(WallGeometry::Mesh(mesh), Vec3::zero(), omega, pivot);
        let v2 = mb2.surface_velocity(point);
        assert_abs_diff_eq!(v2.x, 0.0, epsilon = 1e-15);
        assert_abs_diff_eq!(v2.y, 4.0, epsilon = 1e-15);
        assert_abs_diff_eq!(v2.z, 0.0, epsilon = 1e-15);
    }

    /// V&V — **`advance` moves the wall: translation and rotation of the pose**.
    ///
    /// Methodology (translation): a half-space wall face through the origin with
    /// normal `+ẑ`, translational velocity `v = (0, 0, 1) m/s`, `ω = 0`. After
    /// `advance(dt = 2 s)` the face point must move to `(0, 0, 2)` and the pivot
    /// (origin) to `(0, 0, 2)`; the normal is unchanged. A particle `r = 0.5`
    /// centred at `(0, 0, 2.4)` then penetrates the moved wall by
    /// `δ = r − (2.4 − 2.0) = 0.1`.
    ///
    /// Methodology (rotation): a single triangle with vertex `b = (1, 0, 0)`,
    /// spun about the z-axis through the origin at `ω = (0, 0, π/2) rad/s`. After
    /// `advance(dt = 1 s)` the rotation angle is `π/2`, so `b` maps to `(0, 1, 0)`
    /// (a 90° turn), and the facet normal stays `+ẑ` (rotation about its own
    /// axis). Pass criterion: exact to `1e-12`.
    ///
    /// Result (measured 2026-07-24): translation moved the face point and pivot
    /// to `(0, 0, 2)` and gave the moved-wall overlap `δ = 0.1`; rotation mapped
    /// `b → (0, 1, 0)` with the normal held at `+ẑ`. All to `< 1e-12`, confirming
    /// the Rodrigues pose update and the pivot advance.
    #[test]
    fn advance_translates_and_rotates_pose() {
        // --- Translation ---
        let wall = Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut mb = MovingBoundary::new(
            WallGeometry::Analytic(wall),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::zero(),
            Vec3::zero(),
        );
        mb.advance(2.0);
        assert_abs_diff_eq!(mb.pivot.z, 2.0, epsilon = 1e-12);
        if let WallGeometry::Analytic(Boundary::Wall { point, normal }) = mb.geometry {
            assert_abs_diff_eq!(point.z, 2.0, epsilon = 1e-12);
            assert_abs_diff_eq!(normal.z, 1.0, epsilon = 1e-12);
        } else {
            panic!("geometry should remain a Wall");
        }
        // The moved wall now contacts a particle at z = 2.4.
        let contact = mb
            .particle_overlap(&particle_at(Vec3::new(0.0, 0.0, 2.4), 0.5))
            .expect("moved-wall contact");
        assert_abs_diff_eq!(contact.overlap, 0.1, epsilon = 1e-12);

        // --- Rotation ---
        let tri = Triangle::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        )
        .unwrap();
        let mut spin = MovingBoundary::new(
            WallGeometry::Mesh(MeshWall::new(vec![tri]).unwrap()),
            Vec3::zero(),
            Vec3::new(0.0, 0.0, std::f64::consts::FRAC_PI_2),
            Vec3::zero(),
        );
        spin.advance(1.0); // rotation angle = π/2 about +z
        if let WallGeometry::Mesh(m) = &spin.geometry {
            let b = m.triangles[0].b; // was (1, 0, 0) → (0, 1, 0)
            assert_abs_diff_eq!(b.x, 0.0, epsilon = 1e-12);
            assert_abs_diff_eq!(b.y, 1.0, epsilon = 1e-12);
            assert_abs_diff_eq!(b.z, 0.0, epsilon = 1e-12);
            // Rotation about the facet's own normal leaves it +ẑ.
            assert_abs_diff_eq!(m.triangles[0].normal().z, 1.0, epsilon = 1e-12);
        } else {
            panic!("geometry should remain a Mesh");
        }
    }
}
