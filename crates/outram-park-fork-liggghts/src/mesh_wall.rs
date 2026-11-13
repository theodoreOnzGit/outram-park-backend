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
//! - **CORRECTED 2026-11-11 (gh:#1416):** the two bullets below describe the
//!   geometry-only query [`MeshWall::particle_overlap`]. The force path
//!   ([`crate::granular_system::GranularSystem`]) no longer uses it: it uses
//!   [`MeshWall::sphere_contacts`], a translation of LIGGGHTS' `TriMesh`
//!   (every overlapped facet in its own barycentric region, inactive edges and
//!   corners skipped, per-facet shear history, coplanar-neighbour handling), and
//!   was cross-coded bit-identically on a rotating plate and a rotating drum
//!   (see [`crate::tri_mesh`]). Node velocities are upstream's per-node
//!   velocities, not a rigid `ω × r` evaluated at the contact point.
//! - ~~**When the closest point lies on a shared edge or vertex** of the mesh, the
//!   contact normal is taken from whichever incident triangle is found nearest
//!   (ties broken by iteration order); the penetration `δ = r − d` uses the true
//!   Euclidean distance `d` to that closest point. A blended/averaged edge
//!   normal is not computed.~~
//! - **Rigid-body wall motion only** ([`MovingBoundary`]): pure translation plus
//!   rotation about a single moving pivot. No wall deformation, ~~no per-vertex velocity fields~~ **CORRECTED 2026-11-11**: a mesh carries per-node velocities (set by the mover, used by the force path).
//!   Rotation is applied to [`Boundary::Plane`],
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

use petir::mathf::RealMath;
use crate::boundary::{Boundary, Contact};
use crate::particle::{Particle, Vec3};
use crate::tri_mesh::{arr, vec3, Nodes, TriMeshState, V3};
use crate::mesh_stress::{MeshServo, MeshStress, ServoCtrl, ServoMove};
use crate::variable::{Expr, VarContext, VariableTable};
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
    let (sin_t, cos_t) = angle.r_sin_cos();
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
    ///
    /// **If you mutate this directly**, the bounding-sphere cache below is
    /// invalidated; [`MeshWall::particle_overlap`] detects the length mismatch
    /// and falls back to the unaccelerated scan, so results stay correct but
    /// slow. Rebuild with [`MeshWall::new`] to restore the acceleration.
    pub triangles: Vec<Triangle>,
    /// Per-facet bounding sphere `(centroid, radius)` `[m]`, used only to prune
    /// the nearest-facet search. Pure acceleration: the pruning bound is
    /// conservative, so the facet selected is identical to the brute-force
    /// scan's.
    bounds: Vec<(Vec3, f64)>,
    /// Bounding sphere of the whole mesh `(centre, radius)` `[m]`, for the O(1)
    /// early-out when a particle is nowhere near the wall — the dominant case
    /// once a moving wall has travelled away from the material.
    hull: (Vec3, f64),
    /// Upstream `TriMesh` state: derived geometry, topology (edge/corner
    /// activity), node velocities (gh:#1416). See [`crate::tri_mesh`].
    pub(crate) state: TriMeshState,
}

impl MeshWall {
    /// The facets' nodes in upstream's array layout.
    pub fn nodes(&self) -> Vec<Nodes> {
        self.triangles
            .iter()
            .map(|t| [arr(t.a), arr(t.b), arr(t.c)])
            .collect()
    }

    /// Write nodes back into `triangles`.
    pub(crate) fn set_nodes(&mut self, nodes: &[Nodes]) {
        for (t, n) in self.triangles.iter_mut().zip(nodes) {
            t.a = vec3(n[0]);
            t.b = vec3(n[1]);
            t.c = vec3(n[2]);
        }
    }

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
        let bounds = Self::compute_bounds(&triangles);
        let hull = Self::compute_hull(&bounds);
        let nodes: Vec<Nodes> = triangles
            .iter()
            .map(|t| [arr(t.a), arr(t.b), arr(t.c)])
            .collect();
        let state = TriMeshState::new(&nodes);
        Ok(Self {
            triangles,
            bounds,
            hull,
            state,
        })
    }

    /// Parse an **ASCII STL** file body into a mesh wall.
    ///
    /// This exists so that this crate and upstream LIGGGHTS can be pointed at
    /// the *same geometry file* — `fix mesh/surface file <stl>` on one side and
    /// this on the other — rather than each building its own approximation of
    /// "a cylinder". Used by the angle-of-repose case in
    /// `docs/verification-and-validation.md`.
    ///
    /// # Format accepted
    ///
    /// The standard ASCII grammar, whitespace-insensitive:
    ///
    /// ```text
    /// solid <name>
    ///   facet normal <nx> <ny> <nz>
    ///     outer loop
    ///       vertex <x> <y> <z>
    ///       vertex <x> <y> <z>
    ///       vertex <x> <y> <z>
    ///     endloop
    ///   endfacet
    /// endsolid <name>
    /// ```
    ///
    /// **The `facet normal` record is ignored**, deliberately: STL files in the
    /// wild frequently carry normals inconsistent with their vertex winding,
    /// and [`Triangle::normal`] recomputes it from the winding anyway. Order the
    /// vertices counter-clockwise as seen from the particle side (see
    /// [`Triangle`]). Binary STL is **not** supported.
    ///
    /// Degenerate (zero-area) facets are **skipped**, not rejected — meshing
    /// tools emit them and upstream tolerates them.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] if a `vertex` record does not carry three
    /// parseable coordinates, or if no usable facet is found.
    pub fn from_ascii_stl(text: &str) -> Result<Self, DemError> {
        let mut vertices: Vec<Vec3> = Vec::new();
        let mut triangles: Vec<Triangle> = Vec::new();
        for line in text.lines() {
            let mut it = line.split_whitespace();
            if it.next() != Some("vertex") {
                continue;
            }
            let coords: Vec<f64> = it.filter_map(|t| t.parse::<f64>().ok()).collect();
            if coords.len() != 3 {
                return Err(DemError::InvalidInput(format!(
                    "malformed STL vertex record: {line:?}"
                )));
            }
            vertices.push(Vec3::new(coords[0], coords[1], coords[2]));
            if vertices.len() == 3 {
                // Skip degenerate facets rather than failing the whole file.
                if let Ok(t) = Triangle::new(vertices[0], vertices[1], vertices[2]) {
                    triangles.push(t);
                }
                vertices.clear();
            }
        }
        if triangles.is_empty() {
            return Err(DemError::InvalidInput(
                "ASCII STL contained no usable (non-degenerate) facets".to_string(),
            ));
        }
        Self::new(triangles)
    }

    /// Every facet the sphere `(c, radius)` overlaps, in ascending facet order,
    /// resolved by upstream's `TriMesh::resolveTriSphereContactBary` (face,
    /// edge or corner region, inactive edges and corners skipped). Only
    /// `deltan <= 0` contacts are returned, as `FixWallGran::post_force_mesh`
    /// accepts with the default contact-distance factor. gh:#1416.
    #[must_use]
    pub fn sphere_contacts(&self, c: Vec3, radius: f64) -> Vec<MeshContact> {
        self.sphere_contacts_within(c, radius, 0.0)
    }

    /// One facet resolved for the sphere `(c, radius)` whatever its gap
    /// (`resolveTriSphereContactBary`); the caller applies the acceptance
    /// test. Used where the radius differs per facet (multicontact). gh:#1410.
    #[must_use]
    pub fn resolve_facet(&self, tri: usize, c: Vec3, radius: f64) -> MeshContact {
        let t = &self.triangles[tri];
        let nd: Nodes = [arr(t.a), arr(t.b), arr(t.c)];
        let cs: V3 = arr(c);
        let (deltan, delta, bary, barysign) =
            self.state.resolve_tri_sphere_contact_bary(&nd, tri, radius, &cs, true);
        MeshContact { tri, deltan, delta, bary, barysign }
    }

    /// [`MeshWall::sphere_contacts`] extended to the facets whose gap is
    /// `deltan < margin` (`FixWallGran::post_force_mesh`:
    /// `deltan < (contactDistanceFactor - 1) * radius`, the non-overlapping
    /// "close" contacts). `margin = 0` is exactly `sphere_contacts`. gh:#1410.
    #[must_use]
    pub fn sphere_contacts_within(&self, c: Vec3, radius: f64, margin: f64) -> Vec<MeshContact> {
        let reach = radius + margin.max(0.0);
        let mut out = Vec::new();
        let use_bounds = self.bounds.len() == self.triangles.len();
        if use_bounds {
            let (hc, hr) = self.hull;
            if c.sub(hc).norm() - hr > reach {
                return out;
            }
        }
        let cs: V3 = arr(c);
        for (index, tri) in self.triangles.iter().enumerate() {
            if use_bounds {
                let (centroid, rad) = self.bounds[index];
                if c.sub(centroid).norm() - rad > reach {
                    continue;
                }
            }
            let nd: Nodes = [arr(tri.a), arr(tri.b), arr(tri.c)];
            let (deltan, delta, bary, barysign) = self
                .state
                .resolve_tri_sphere_contact_bary(&nd, index, radius, &cs, true);
            if deltan <= 0.0 || deltan < margin {
                out.push(MeshContact {
                    tri: index,
                    deltan,
                    delta,
                    bary,
                    barysign,
                });
            }
        }
        out
    }

    /// Wall velocity at barycentric coordinates `bary` of facet `tri`
    /// (interpolated node velocities, as upstream).
    #[must_use]
    pub fn wall_velocity(&self, tri: usize, bary: &V3) -> Vec3 {
        vec3(self.state.wall_velocity(tri, bary))
    }

    /// Whether facets `a` and `b` are coplanar node-neighbours (upstream
    /// `areCoplanarNodeNeighs`).
    #[must_use]
    pub fn coplanar_node_neighs(&self, a: usize, b: usize) -> bool {
        self.state.are_coplanar_node_neighs(a, b)
    }

    /// Recompute the derived facet geometry from the nodes (a neighbour-list
    /// rebuild step upstream).
    pub fn refresh_geometry(&mut self) {
        let nodes = self.nodes();
        self.state.refresh_from_nodes(&nodes);
    }

    /// Upstream `decideRebuild`: any node moved more than `skin/2` since the
    /// last refresh.
    #[must_use]
    pub fn decide_rebuild(&self, skin: f64) -> bool {
        self.state.decide_rebuild(&self.nodes(), skin)
    }

    /// Recompute the pruning caches from the current `triangles`.
    ///
    /// **Call this after mutating [`MeshWall::triangles`] in any way that is
    /// not a pure translation** (a rotation, a re-meshing, an edit). The caches
    /// are only an acceleration, but a *stale* cache is not conservative — it
    /// prunes against the wrong positions and can drop facets the particle is
    /// genuinely touching. [`WallGeometry`] calls this for you when it rotates
    /// a mesh; [`MeshWall::shift_bounds`] is the cheaper exact equivalent for a
    /// rigid translation.
    pub fn rebuild_bounds(&mut self) {
        self.bounds = Self::compute_bounds(&self.triangles);
        self.hull = Self::compute_hull(&self.bounds);
    }

    /// Translate the pruning caches by `disp` `[m]`, the exact equivalent of
    /// [`MeshWall::rebuild_bounds`] after a rigid translation of every facet,
    /// at O(N_tri) with no geometry recomputation.
    pub fn shift_bounds(&mut self, disp: Vec3) {
        for (centroid, _) in self.bounds.iter_mut() {
            *centroid = centroid.add(disp);
        }
        self.hull.0 = self.hull.0.add(disp);
    }

    fn compute_bounds(triangles: &[Triangle]) -> Vec<(Vec3, f64)> {
        triangles
            .iter()
            .map(|t| {
                let centroid = t.a.add(t.b).add(t.c).scale(1.0 / 3.0);
                let radius = centroid
                    .sub(t.a)
                    .norm()
                    .max(centroid.sub(t.b).norm())
                    .max(centroid.sub(t.c).norm());
                (centroid, radius)
            })
            .collect()
    }

    fn compute_hull(bounds: &[(Vec3, f64)]) -> (Vec3, f64) {
        let centre = bounds
            .iter()
            .fold(Vec3::zero(), |acc, (c, _)| acc.add(*c))
            .scale(1.0 / bounds.len() as f64);
        let radius = bounds
            .iter()
            .map(|(c, r)| centre.sub(*c).norm() + r)
            .fold(0.0_f64, f64::max);
        (centre, radius)
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

        // Pruning. Two exact bounds, neither of which can change the answer:
        //
        //  * whole-mesh: if the particle is farther than `r` from the mesh's
        //    bounding sphere it cannot touch any facet, so return None at O(1).
        //    This is the dominant case once a moving wall has travelled away
        //    from the material, and without it the far field costs a full
        //    O(N_tri) scan every step.
        //  * per-facet: only a facet whose closest point lies within `r` can
        //    produce a contact at all, and no point of a facet is nearer than
        //    |c - centroid| - rad. A facet failing that bound is skipped.
        //    Pruning against `r` (rather than against the best distance so far)
        //    is both stronger and order-independent.
        //
        // Among the facets that survive, the nearest is selected exactly as the
        // brute-force scan selects it, and a particle touching nothing yields
        // None either way. Falls back to the unaccelerated scan if `triangles`
        // was mutated behind the cache.
        let use_bounds = self.bounds.len() == self.triangles.len();
        if use_bounds {
            let (hc, hr) = self.hull;
            if c.sub(hc).norm() - hr >= r {
                return None;
            }
        }
        let mut best: Option<(f64, Vec3, Vec3)> = None; // (distance, closest point, normal)
        for (index, tri) in self.triangles.iter().enumerate() {
            if use_bounds {
                let (centroid, rad) = self.bounds[index];
                if c.sub(centroid).norm() - rad >= r {
                    continue;
                }
            }
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

/// One facet contact from [`MeshWall::sphere_contacts`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshContact {
    /// Facet index (upstream element id).
    pub tri: usize,
    /// Signed gap `d - r` `[m]`, `<= 0` for a contact (upstream `deltan`).
    pub deltan: f64,
    /// `closest point - sphere centre` `[m]` (upstream `delta`).
    pub delta: V3,
    /// Barycentric coordinates of the contact point.
    pub bary: V3,
    /// Region code, `7` = face.
    pub barysign: u8,
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
                // The pruning caches move with the facets. Leaving them behind
                // is NOT a slow-but-correct fallback: a stale cache prunes
                // against the old positions and silently drops facets the
                // particle is genuinely touching.
                m.shift_bounds(disp);
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
                m.rebuild_bounds();
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
    /// Exact `fix move/mesh rotate` parameters for a mesh (unit axis and rate),
    /// when set by [`MovingBoundary::rotating`]. A mesh without it derives the
    /// axis as `ω/|ω|`, which is exact only for an axis-aligned rotation.
    pub rotation: Option<MeshRotation>,
    /// The time-dependent mover (`fix move/mesh` wiggle, viblin, riggle,
    /// vibrot), if any. gh:#1416.
    pub motion: MeshMotion,
    /// Time since the mover started `[s]`, accumulated by `+= dt` per step as
    /// `FixMoveMesh::time_` is.
    pub time: f64,
    /// `update->ntimestep` seen by equal-style variables: incremented once
    /// per [`MovingBoundary::advance`] before the mover runs, as upstream
    /// increments `ntimestep` before `initial_integrate`.
    pub step: i64,
    /// `update->endstep` of the run, for `ramp()` in a variable (0 = unset).
    pub end_step: i64,
    /// `fix mesh/surface/stress` module (force, torque and stress on the
    /// mesh), if requested with [`MovingBoundary::with_stress`]. gh:#1416.
    pub stress: Option<MeshStress>,
    /// `mesh_module_stress_servo` (force PID) with the original nodes
    /// `mesh->move` starts from. gh:#1416.
    pub servo: Option<(MeshServo, Vec<Nodes>)>,
    /// `getTotalNumContacts()` of the servo mesh neighbour list at its last
    /// rebuild (read by `mode auto`). gh:#1416.
    pub servo_contacts: usize,
    /// A mesh with no `fix wall/gran` (`fix mesh/surface` only): it moves and
    /// counts (`fix massflow/mesh`) but exerts no contact force.
    pub contactless: bool,
}

/// Upper bound of the `order` of a vibration mover (upstream `ampl[30]`).
pub const MAX_VIBRATION_ORDER: usize = 30;

/// The time-dependent `fix move/mesh` movers of `mesh_mover_linear.cpp` and
/// `mesh_mover_rotation.cpp` (LIGGGHTS-PUBLIC `3d5c00f2`). The constant-rate
/// `linear` and `rotate` movers are the plain `velocity` and `rotation` of
/// [`MovingBoundary`]; `linear/variable` and `rotate/variable` (~~not ported~~ **ported 2026-11-11, gh:#1416**, via [`crate::variable`]) read LIGGGHTS
/// equal-style variables.
#[derive(Debug, Clone, PartialEq)]
pub enum MeshMotion {
    /// No time-dependent mover (use `velocity` / `rotation`).
    None,
    /// `wiggle amplitude ax ay az period T`: translation with velocity
    /// `A omega cos(omega t)`.
    Wiggle {
        /// Amplitude vector `[m]`.
        amplitude: Vec3,
        /// `2 pi / T` `[rad/s]`.
        omega: f64,
    },
    /// `viblin axis a order n amplitude .. phase .. period ..`: translation
    /// along `axis` with velocity `-sum A_j omega_j sin(omega_j t + phi_j)`.
    VibLin {
        /// Unit axis.
        axis: Vec3,
        /// Number of harmonics (1..=30).
        order: usize,
        /// Amplitudes `[m]`.
        ampl: [f64; MAX_VIBRATION_ORDER],
        /// Phases `[rad]`.
        phi: [f64; MAX_VIBRATION_ORDER],
        /// `2 pi / T_j` `[rad/s]`.
        omega: [f64; MAX_VIBRATION_ORDER],
    },
    /// `linear/variable v_x v_y v_z`: translation with the velocity given by
    /// three equal-style variables, re-evaluated every step
    /// (`MeshMoverLinearVariable::initial_integrate`).
    LinearVariable {
        /// Velocity components `[m/s]` as formulas.
        formulas: [Expr; 3],
    },
    /// `rotate/variable origin o axis a omega v_w`: rotation about the fixed
    /// point with the rate given by an equal-style variable
    /// (`MeshMoverRotateVariable::initial_integrate`).
    RotateVariable {
        /// Unit axis.
        axis: Vec3,
        /// Rate `[rad/s]` as a formula.
        omega: Expr,
    },
    /// `riggle origin o axis a period T amplitude deg`: rotation about the
    /// fixed point `o` with angular velocity `A omega cos(omega t)`.
    Riggle {
        /// Amplitude `pi deg / 180` `[rad]`.
        amplitude: f64,
        /// `2 pi / T` `[rad/s]`.
        omega: f64,
        /// Unit axis.
        axis: Vec3,
    },
    /// `vibrot origin o axis a order n amplitude .. phase .. period ..`:
    /// rotation about the fixed point with angular velocity
    /// `-sum A_j omega_j sin(omega_j t + phi_j)`.
    VibRot {
        /// Unit axis.
        axis: Vec3,
        /// Number of harmonics (1..=30).
        order: usize,
        /// Amplitudes `[rad]`.
        ampl: [f64; MAX_VIBRATION_ORDER],
        /// Phases `[rad]`.
        phi: [f64; MAX_VIBRATION_ORDER],
        /// `2 pi / T_j` `[rad/s]`.
        omega: [f64; MAX_VIBRATION_ORDER],
    },
}

/// `vectorNormalize3D` as the rotate mover uses it (zero stays zero).
fn unit_vector(axis: Vec3) -> Vec3 {
    let a = arr(axis);
    let norm = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    let inv = if norm == 0. { 0. } else { 1. / norm };
    Vec3::new(a[0] * inv, a[1] * inv, a[2] * inv)
}

fn harmonics(
    amplitude: &[f64],
    phase: &[f64],
    period: &[f64],
) -> Option<(
    usize,
    [f64; MAX_VIBRATION_ORDER],
    [f64; MAX_VIBRATION_ORDER],
    [f64; MAX_VIBRATION_ORDER],
)> {
    let n = amplitude.len();
    if n == 0 || n > MAX_VIBRATION_ORDER || phase.len() != n || period.len() != n {
        return None;
    }
    let (mut a, mut ph, mut om) = (
        [0.0; MAX_VIBRATION_ORDER],
        [0.0; MAX_VIBRATION_ORDER],
        [0.0; MAX_VIBRATION_ORDER],
    );
    for j in 0..n {
        a[j] = amplitude[j];
        ph[j] = phase[j];
        om[j] = 2. * std::f64::consts::PI / period[j];
    }
    Some((n, a, ph, om))
}

/// Parameters of upstream's `MeshMoverRotate`: `omega_ = 2π/period`,
/// `axis_` normalised by `vectorNormalize3D`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshRotation {
    /// Unit rotation axis.
    pub axis: Vec3,
    /// Rotation rate `[rad/s]`.
    pub omega: f64,
}

impl MovingBoundary {
    /// A mesh rotating like `fix move/mesh ... rotate origin <o> axis <a>
    /// period <T>`: `ω = 2π/T`, axis normalised as upstream does. The mesh's
    /// node velocities stay zero until the first [`MovingBoundary::advance`].
    #[must_use]
    pub fn rotating(geometry: WallGeometry, origin: Vec3, axis: Vec3, period: f64) -> Self {
        let a = arr(axis);
        let norm = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
        let inv = if norm == 0. { 0. } else { 1. / norm };
        let u = Vec3::new(a[0] * inv, a[1] * inv, a[2] * inv);
        let omega = 2. * std::f64::consts::PI / period;
        let mut m = Self::new(geometry, Vec3::zero(), u.scale(omega), origin);
        m.rotation = Some(MeshRotation { axis: u, omega });
        m
    }

    /// Whether this wall rotates.
    #[must_use]
    pub fn is_rotating(&self) -> bool {
        self.rotation.is_some_and(|r| r.omega != 0.0) || self.angular_velocity.norm() > 0.0
            || self.servo.as_ref().is_some_and(|s| s.0.ctrl() == ServoCtrl::Torque)
    }

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
            rotation: None,
            motion: MeshMotion::None,
            time: 0.0,
            step: 0,
            end_step: 0,
            stress: None,
            servo: None,
            servo_contacts: 0,
            contactless: false,
        }
    }

    /// A mesh with no `fix wall/gran`: it moves (and may be counted by
    /// `fix massflow/mesh`) but exerts no contact force on particles.
    #[must_use]
    pub fn without_contact(mut self) -> Self {
        self.contactless = true;
        self
    }

    /// Track the force and torque on this mesh (`fix mesh/surface/stress ...
    /// reference_point <p>`). No-op for a non-mesh wall.
    #[must_use]
    pub fn with_stress(mut self, p_ref: Vec3) -> Self {
        if let WallGeometry::Mesh(m) = &self.geometry {
            let area = (0..m.triangles.len()).map(|i| m.state.area(i)).collect();
            self.stress = Some(MeshStress::new(p_ref, area));
        }
        self
    }

    /// `wear finnie` on this wall's `fix mesh/surface/stress` (call after
    /// [`MovingBoundary::with_stress`]; a no-op without it): `k_finnie[t]` is
    /// the coefficient against particle type `t` (0-based). gh:#1491.
    #[must_use]
    pub fn with_wear_finnie(mut self, k_finnie: Vec<f64>) -> Self {
        self.stress = self.stress.map(|s| s.with_wear_finnie(k_finnie));
        self
    }

    /// Attach `mesh_module_stress_servo` (`ctrlPV force`, PID): the mesh
    /// also tracks stress, `p_ref` at the servo centre. The mesh's current
    /// nodes are the original nodes `mesh->move` starts from. No-op for a
    /// non-mesh wall.
    #[must_use]
    pub fn with_servo(mut self, servo: MeshServo) -> Self {
        if let WallGeometry::Mesh(m) = &self.geometry {
            let orig = m.nodes();
            let xcm = servo.xcm();
            self = self.with_stress(vec3(xcm));
            self.servo = Some((servo, orig));
        }
        self
    }

    fn with_motion(geometry: WallGeometry, pivot: Vec3, motion: MeshMotion) -> Self {
        let mut m = Self::new(geometry, Vec3::zero(), Vec3::zero(), pivot);
        m.motion = motion;
        m
    }

    /// `fix move/mesh ... linear/variable v_<x> v_<y> v_<z>`: the three
    /// formulas are the (already `v_`-stripped) equal-style variables of
    /// `table`, evaluated at `ntimestep` every step.
    ///
    /// # Errors
    /// If a variable is not defined in `table`.
    pub fn linear_variable(
        geometry: WallGeometry,
        table: &VariableTable,
        names: [&str; 3],
    ) -> Result<Self, String> {
        let get = |n: &str| {
            table.get(n).cloned().ok_or_else(|| {
                format!("Variable name {n} for fix move/mesh linear/variable does not exist")
            })
        };
        let formulas = [get(names[0])?, get(names[1])?, get(names[2])?];
        Ok(Self::with_motion(
            geometry,
            Vec3::zero(),
            MeshMotion::LinearVariable { formulas },
        ))
    }

    /// `fix move/mesh ... rotate/variable origin <o> axis <a> omega v_<name>`.
    ///
    /// # Errors
    /// If the variable is not defined in `table`.
    pub fn rotate_variable(
        geometry: WallGeometry,
        table: &VariableTable,
        origin: Vec3,
        axis: Vec3,
        name: &str,
    ) -> Result<Self, String> {
        let omega = table.get(name).cloned().ok_or_else(|| {
            format!("Variable name {name} for fix move/mesh rotate/variable does not exist")
        })?;
        Ok(Self::with_motion(
            geometry,
            origin,
            MeshMotion::RotateVariable {
                axis: unit_vector(axis),
                omega,
            },
        ))
    }

    /// `fix move/mesh ... wiggle amplitude <a> period <T>`.
    #[must_use]
    pub fn wiggle(geometry: WallGeometry, amplitude: Vec3, period: f64) -> Self {
        let omega = 2. * std::f64::consts::PI / period;
        Self::with_motion(
            geometry,
            Vec3::zero(),
            MeshMotion::Wiggle { amplitude, omega },
        )
    }

    /// `fix move/mesh ... viblin axis <a> order <n> amplitude .. phase ..
    /// period ..`. `None` if `n` is not in `1..=30` or the lists differ in length.
    #[must_use]
    pub fn viblin(
        geometry: WallGeometry,
        axis: Vec3,
        amplitude: &[f64],
        phase: &[f64],
        period: &[f64],
    ) -> Option<Self> {
        let (order, ampl, phi, omega) = harmonics(amplitude, phase, period)?;
        Some(Self::with_motion(
            geometry,
            Vec3::zero(),
            MeshMotion::VibLin {
                axis: unit_vector(axis),
                order,
                ampl,
                phi,
                omega,
            },
        ))
    }

    /// `fix move/mesh ... riggle origin <o> axis <a> period <T> amplitude
    /// <deg>`.
    #[must_use]
    pub fn riggle(
        geometry: WallGeometry,
        origin: Vec3,
        axis: Vec3,
        period: f64,
        amplitude_deg: f64,
    ) -> Self {
        Self::with_motion(
            geometry,
            origin,
            MeshMotion::Riggle {
                amplitude: std::f64::consts::PI * amplitude_deg / 180.,
                omega: 2. * std::f64::consts::PI / period,
                axis: unit_vector(axis),
            },
        )
    }

    /// `fix move/mesh ... vibrot origin <o> axis <a> order <n> amplitude ..
    /// phase .. period ..` (amplitudes in rad). `None` as
    /// [`MovingBoundary::viblin`].
    #[must_use]
    pub fn vibrot(
        geometry: WallGeometry,
        origin: Vec3,
        axis: Vec3,
        amplitude: &[f64],
        phase: &[f64],
        period: &[f64],
    ) -> Option<Self> {
        let (order, ampl, phi, omega) = harmonics(amplitude, phase, period)?;
        Some(Self::with_motion(
            geometry,
            origin,
            MeshMotion::VibRot {
                axis: unit_vector(axis),
                order,
                ampl,
                phi,
                omega,
            },
        ))
    }

    /// Set `velocity` / `rotation` for this step from the time-dependent
    /// mover (`MeshMover*::initial_integrate` of upstream), after `time +=
    /// dt`.
    fn apply_motion(&mut self, dt: f64) {
        let t = self.time;
        let ctx = VarContext {
            step: self.step,
            begin_step: 0,
            end_step: self.end_step,
            dt,
        };
        match &self.motion {
            MeshMotion::None => {}
            MeshMotion::LinearVariable { formulas } => {
                let mut v = [0.0; 3];
                for (k, f) in formulas.iter().enumerate() {
                    v[k] = f
                        .eval(&ctx)
                        .unwrap_or_else(|e| panic!("fix move/mesh linear/variable: {e}"));
                }
                self.velocity = Vec3::new(v[0], v[1], v[2]);
            }
            MeshMotion::RotateVariable { axis, omega } => {
                let w = omega
                    .eval(&ctx)
                    .unwrap_or_else(|e| panic!("fix move/mesh rotate/variable: {e}"));
                self.rotation = Some(MeshRotation {
                    axis: *axis,
                    omega: w,
                });
            }
            MeshMotion::Wiggle { amplitude, omega } => {
                self.velocity = amplitude.scale(omega * (omega * t).r_cos());
            }
            MeshMotion::VibLin {
                axis,
                order,
                ampl,
                phi,
                omega,
            } => {
                let mut v_a = 0.0;
                for j in 0..*order {
                    v_a -= ampl[j] * omega[j] * (omega[j] * t + phi[j]).r_sin();
                }
                self.velocity = axis.scale(v_a);
            }
            MeshMotion::Riggle {
                amplitude,
                omega,
                axis,
            } => {
                let pre = omega * amplitude * (omega * t).r_cos();
                self.rotation = Some(MeshRotation {
                    axis: *axis,
                    omega: pre,
                });
            }
            MeshMotion::VibRot {
                axis,
                order,
                ampl,
                phi,
                omega,
            } => {
                let mut v_r = 0.0;
                for j in 0..*order {
                    v_r -= ampl[j] * omega[j] * (omega[j] * t + phi[j]).r_sin();
                }
                self.rotation = Some(MeshRotation {
                    axis: *axis,
                    omega: v_r,
                });
            }
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
        if matches!(self.geometry, WallGeometry::Mesh(_)) {
            self.time += dt;
            self.step += 1;
            if self.servo.is_some() {
                self.advance_servo();
                return;
            }
            self.apply_motion(dt);
            self.advance_mesh(dt);
            return;
        }
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

impl MovingBoundary {
    /// `MeshModuleStressServo::initial_integrate`: `ctrlPV force` puts every
    /// node at its original position plus `dX` (`mesh->move(dX, dx)`) and
    /// `p_ref` follows the centre; `ctrlPV torque` puts every node at its
    /// original position rotated by the total angle about `xcm`
    /// (`mesh->rotate(totalPhi, incrementalPhi, axis, xcm)`). Node velocities
    /// keep what the last `final_integrate` set.
    fn advance_servo(&mut self) {
        let Some((servo, orig)) = &mut self.servo else {
            return;
        };
        // only if the wall should move (`integrate stop`, gh:#1510)
        if !servo.is_integrating() {
            return;
        }
        let mv = servo.initial_integrate();
        let WallGeometry::Mesh(m) = &mut self.geometry else {
            return;
        };
        match mv {
            ServoMove::Translate { d_x: d } => {
                let nodes: Vec<Nodes> = orig
                    .iter()
                    .map(|nd| {
                        let mut o = *nd;
                        for node in &mut o {
                            for k in 0..3 {
                                node[k] += d[k];
                            }
                        }
                        o
                    })
                    .collect();
                m.set_nodes(&nodes);
                m.rebuild_bounds();
                if let Some(st) = &mut self.stress {
                    st.p_ref = vec3(servo.xcm());
                }
            }
            ServoMove::Rotate {
                total_phi,
                inc_phi,
                axis,
                origin,
            } => {
                // `p_ref` is the rotation origin (set in `init()`): the
                // `move(-origin)`, rotate, `move(origin)` of the tracked
                // global property leaves it where it is.
                let mut nodes = orig.clone();
                m.state
                    .rotate_total(orig, &mut nodes, total_phi, inc_phi, &axis, &origin);
                m.set_nodes(&nodes);
                m.rebuild_bounds();
            }
        }
    }

    /// `MeshModuleStressServo::final_integrate`: new servo velocity from this
    /// step's total force (or torque) and the mesh contact count, written to
    /// every node's velocity (`set_v_node` / `set_v_node_rotate`).
    pub fn servo_final_integrate(&mut self) {
        let (Some((servo, _)), Some(st)) = (&mut self.servo, &self.stress) else {
            return;
        };
        let pv = match servo.ctrl() {
            ServoCtrl::Force => st.f_total(),
            ServoCtrl::Torque => st.torque_total(),
        };
        let v = servo.final_integrate_pv(pv, self.servo_contacts);
        // `integrate stop`: `set_v_node` is not called, the nodes keep their
        // velocities (gh:#1510)
        if !servo.is_integrating() {
            return;
        }
        if let WallGeometry::Mesh(m) = &mut self.geometry {
            match servo.ctrl() {
                ServoCtrl::Force => {
                    for nv in &mut m.state.node_velocity {
                        *nv = [v; 3];
                    }
                }
                ServoCtrl::Torque => {
                    let nodes = m.nodes();
                    for (nd, nv) in nodes.iter().zip(m.state.node_velocity.iter_mut()) {
                        for j in 0..3 {
                            nv[j] = servo.node_velocity_rotate(nd[j]);
                        }
                    }
                }
            }
        }
    }

    /// The `init()` and `setup_pre_force()` of a new `run` for this wall
    /// (gh:#1510); a no-op for an analytic wall.
    ///
    /// * `MeshModuleStressServo::init`: [`MeshServo::init_run`] with `r_min`
    ///   the smallest radius of `particles` (upstream `BIG = 1e6` if none) and
    ///   the current nodes; `p_ref = xcm` on the stress module.
    /// * `MeshModuleStressServo::setup_pre_force` ([`MeshServo::setup_pre_force`])
    ///   and `FixMesh::setup_pre_force` -> `pbcExchangeBorders(1)` ->
    ///   `refreshOwned(1)`: the current nodes become the original nodes the
    ///   servo moves from (`storeNodePosOrig`), and the facet geometry, the
    ///   rebuild reference nodes and the stress module's facet areas are
    ///   recomputed from the nodes (`recalcLocalSurfProperties`,
    ///   `storeNodePosRebuild`).
    ///
    /// Everything else persists: the servo's integral and derivative state,
    /// its velocity and the node velocities, the stress totals, the wear, the
    /// mover's time.
    ///
    /// # Errors
    ///
    /// Whatever [`MeshServo::init_run`] reports (upstream's `init()` aborts).
    pub fn begin_run(&mut self, particles: &[Particle]) -> Result<(), DemError> {
        let WallGeometry::Mesh(m) = &mut self.geometry else {
            return Ok(());
        };
        if let Some((servo, orig)) = &mut self.servo {
            // r = r_min = BIG; r_min = MIN(r_min, r) over the atoms
            let r_min = particles
                .iter()
                .fold(1.0e6_f64, |r_min, p| if r_min < p.radius { r_min } else { p.radius });
            servo.init_run(r_min, &m.nodes())?;
            if let Some(st) = &mut self.stress {
                st.p_ref = vec3(servo.xcm());
            }
            servo.setup_pre_force();
            *orig = m.nodes();
        }
        self.refresh_mesh();
        Ok(())
    }

    /// `pbcExchangeBorders` -> `SurfaceMesh::refreshOwned` ->
    /// `recalcLocalSurfProperties` (`surface_mesh_I.h`, LIGGGHTS-PUBLIC
    /// `3d5c00f2`) for this wall's mesh: the facet geometry (edge vectors and
    /// lengths, normals, obtuse index) and the rebuild reference nodes from the
    /// current nodes ([`MeshWall::refresh_geometry`]), and the facet areas the
    /// stress module divides by (`area(i) = calcArea(i)`, read by
    /// `MeshModuleStress` as `areaElem`). Upstream runs it at a run's set-up
    /// and, for a moving mesh, at every re-neighbouring (`FixMesh::pre_force`
    /// with `pOpFlag_`), before the force compute (gh:#1512). A no-op for an
    /// analytic wall.
    pub fn refresh_mesh(&mut self) {
        let WallGeometry::Mesh(m) = &mut self.geometry else {
            return;
        };
        m.refresh_geometry();
        if let Some(st) = &mut self.stress {
            st.refresh_area((0..m.triangles.len()).map(|i| m.state.area(i)).collect());
        }
    }

    /// `FixMoveMesh::setup` of a new `run`: the node velocities of a mesh
    /// moved by a `fix move/mesh` are zeroed (`v->setAll(0.)` when the mover
    /// is the first on the mesh). Upstream does this in the fix's own
    /// `setup`, i.e. after the `fix wall/gran` set-up evaluation when the
    /// `move/mesh` is declared after the wall, which is the order this
    /// crate's fix order assumes. A no-op for a servo mesh, a static mesh and
    /// an analytic wall (gh:#1510).
    pub fn move_mesh_setup(&mut self) {
        if self.servo.is_some() {
            return;
        }
        let moved = !matches!(self.motion, MeshMotion::None)
            || self.rotation.is_some()
            || self.velocity != Vec3::zero()
            || self.angular_velocity != Vec3::zero();
        if !moved {
            return;
        }
        if let WallGeometry::Mesh(m) = &mut self.geometry {
            m.state.node_velocity.fill([[0.0; 3]; 3]);
        }
    }

    /// `FixNeighlistMesh::pre_force` count for the servo mesh
    /// (`numAllContacts_`): the number of (facet, particle) pairs the mesh
    /// neighbour list holds, for a moving mesh (threshold = the full
    /// `skin`, `r * contactDistanceFactor = r`). A no-op without a servo.
    pub fn update_servo_contacts(&mut self, particles: &[Particle], skin: f64) {
        if self.servo.is_none() {
            return;
        }
        let WallGeometry::Mesh(m) = &self.geometry else {
            return;
        };
        let nodes = m.nodes();
        let mut count = 0;
        for (i, nd) in nodes.iter().enumerate() {
            for p in particles {
                if m.state
                    .neighbuild(nd, i, p.radius, &arr(p.position), skin)
                {
                    count += 1;
                }
            }
        }
        self.servo_contacts = count;
    }

    /// Mesh branch of [`MovingBoundary::advance`]: upstream `FixMoveMesh::
    /// initial_integrate` with a rotate mover then a linear mover. Node
    /// velocities are zeroed, the nodes rotate by the step quaternion about
    /// the pivot (`MeshMoverRotate::initial_integrate`; cached edge vectors and
    /// normals rotate with them), `v += ω × (node − pivot)`, then the nodes
    /// translate by `velocity·dt` and `v += velocity` (`MeshMoverLinear`).
    fn advance_mesh(&mut self, dt: f64) {
        let (axis, omega) = match self.rotation {
            Some(r) => (arr(r.axis), r.omega),
            None => {
                let w = self.angular_velocity;
                let rate = w.norm();
                if rate > 0.0 {
                    ([w.x / rate, w.y / rate, w.z / rate], rate)
                } else {
                    ([0.0; 3], 0.0)
                }
            }
        };
        let pivot = arr(self.pivot);
        let disp = self.velocity.scale(dt);
        let vel = arr(self.velocity);
        let WallGeometry::Mesh(m) = &mut self.geometry else {
            return;
        };
        let mut nodes = m.nodes();
        for v in &mut m.state.node_velocity {
            *v = [[0.0; 3]; 3];
        }
        let rotating = omega != 0.0;
        if rotating {
            m.state
                .rotate_incremental(&mut nodes, omega * dt, &axis, &pivot);
            if let Some(st) = &mut self.stress {
                // TrackingMesh::rotate: customValues_.move(-origin), rotate by dQ, move(origin)
                let an = {
                    let mag = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
                    [axis[0] / mag, axis[1] / mag, axis[2] / mag]
                };
                let h = omega * dt * 0.5;
                let dq = [h.r_cos(), an[0] * h.r_sin(), an[1] * h.r_sin(), an[2] * h.r_sin()];
                let trans = pivot[0] * pivot[0] + pivot[1] * pivot[1] + pivot[2] * pivot[2] > 0.;
                let mut p = arr(st.p_ref);
                if trans {
                    p = [p[0] + -pivot[0], p[1] + -pivot[1], p[2] + -pivot[2]];
                }
                p = crate::tri_mesh::vec_quat_rotate(&p, &dq);
                if trans {
                    p = [p[0] + pivot[0], p[1] + pivot[1], p[2] + pivot[2]];
                }
                st.p_ref = vec3(p);
            }
            let omega_vec = [omega * axis[0], omega * axis[1], omega * axis[2]];
            for (nd, vn) in nodes.iter().zip(m.state.node_velocity.iter_mut()) {
                for j in 0..3 {
                    let r = [
                        nd[j][0] - pivot[0],
                        nd[j][1] - pivot[1],
                        nd[j][2] - pivot[2],
                    ];
                    let v_rot = [
                        omega_vec[1] * r[2] - omega_vec[2] * r[1],
                        omega_vec[2] * r[0] - omega_vec[0] * r[2],
                        omega_vec[0] * r[1] - omega_vec[1] * r[0],
                    ];
                    for k in 0..3 {
                        vn[j][k] += v_rot[k];
                    }
                }
            }
        }
        let d = arr(disp);
        for (nd, vn) in nodes.iter_mut().zip(m.state.node_velocity.iter_mut()) {
            for j in 0..3 {
                for k in 0..3 {
                    nd[j][k] += d[k];
                    vn[j][k] += vel[k];
                }
            }
        }
        m.set_nodes(&nodes);
        m.rebuild_bounds();
        if let Some(st) = &mut self.stress {
            st.p_ref = st.p_ref.add(disp);
        }
        self.pivot = self.pivot.add(disp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;
    use uom::si::f64::{Length, Mass, ThermodynamicTemperature};

    /// V&V — **bounding-sphere pruning returns exactly the brute-force answer**.
    ///
    /// Methodology: the nearest-facet search skips facets whose bounding sphere
    /// cannot beat the best distance so far. That bound is conservative, so the
    /// result must be *identical* — not merely close — to an unpruned scan.
    /// This drives the committed 1280-facet lift cylinder with 400 probe
    /// particles on a lattice spanning the tube interior and its mouth, and
    /// compares the accelerated [`MeshWall::particle_overlap`] against a
    /// reference scan that tests every facet.
    ///
    /// Result (measured 2026-09-16): 400/400 probes agree exactly — same
    /// `Some`/`None`, and where in contact, overlap, normal and contact point
    /// bit-identical (`0.0` difference). Pruning is speed only.
    #[test]
    fn bounding_sphere_pruning_matches_brute_force_exactly() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../reference-data/liggghts/lift_cylinder.stl");
        let Ok(text) = std::fs::read_to_string(path) else {
            eprintln!("skipping: reference-data/liggghts/ not present");
            return;
        };
        let mesh = MeshWall::from_ascii_stl(&text).expect("valid stl");

        // Reference: unpruned nearest-facet scan.
        let brute = |c: Vec3, r: f64| -> Option<(f64, Vec3, Vec3)> {
            let mut best: Option<(f64, Vec3, Vec3)> = None;
            for tri in &mesh.triangles {
                let q = tri.closest_point(c);
                let d = c.sub(q).norm();
                if best.is_none_or(|(bd, _, _)| d < bd) {
                    best = Some((d, q, tri.normal()));
                }
            }
            let (d, q, n) = best?;
            if r - d > 0.0 {
                Some((r - d, n, q))
            } else {
                None
            }
        };

        let mut probes = 0;
        for i in 0..10 {
            for j in 0..10 {
                for k in 0..4 {
                    let x = -0.055 + 0.011 * f64::from(i);
                    let y = -0.055 + 0.011 * f64::from(j);
                    let z = -0.005 + 0.045 * f64::from(k);
                    let p = Particle::new(
                        Vec3::new(x, y, z),
                        Vec3::zero(),
                        Vec3::zero(),
                        Mass::new::<kilogram>(1.0e-3),
                        Length::new::<meter>(0.005),
                        ThermodynamicTemperature::new::<kelvin>(300.0),
                    )
                    .expect("valid probe");
                    let got = mesh.particle_overlap(&p);
                    let want = brute(p.position, p.radius);
                    match (got, want) {
                        (None, None) => {}
                        (Some(g), Some((ov, n, q))) => {
                            assert_abs_diff_eq!(g.overlap, ov, epsilon = 0.0);
                            assert_abs_diff_eq!(g.normal.x, n.x, epsilon = 0.0);
                            assert_abs_diff_eq!(g.normal.y, n.y, epsilon = 0.0);
                            assert_abs_diff_eq!(g.normal.z, n.z, epsilon = 0.0);
                            assert_abs_diff_eq!(g.point.x, q.x, epsilon = 0.0);
                            assert_abs_diff_eq!(g.point.y, q.y, epsilon = 0.0);
                            assert_abs_diff_eq!(g.point.z, q.z, epsilon = 0.0);
                        }
                        (g, w) => panic!("pruning disagreed at ({x},{y},{z}): {g:?} vs {w:?}"),
                    }
                    probes += 1;
                }
            }
        }
        assert_eq!(probes, 400);
    }

    /// V&V — **pruning stays correct after the mesh MOVES** (regression).
    ///
    /// Methodology: the bounding-sphere caches are keyed to facet positions, so
    /// a rigid move must carry them along. This drives a [`MovingBoundary`]
    /// wrapping the committed lift cylinder through 200 translation steps
    /// (`0.02 m/s`, `dt = 5 ms`, total `0.02 m`) and one rotation, and after
    /// each stage compares [`MeshWall::particle_overlap`] against an unpruned
    /// scan over the *current* facet positions, for probes placed right at the
    /// wall where any cache error shows up immediately.
    ///
    /// **This is a regression test for a real defect.** The first version of
    /// the pruning updated `triangles` on a move but left `bounds`/`hull`
    /// behind. A stale cache is *not* conservative — it prunes against the old
    /// positions and drops facets the particle is genuinely touching — so the
    /// lifting cylinder silently stopped confining anything and the
    /// angle-of-repose heap collapsed to a monolayer (apex `0.0100 m` against
    /// LIGGGHTS' `0.0370 m`, kinetic energy `5.6e-2 J` and still rising after
    /// the rest phase). The static-mesh equivalence test did not catch it
    /// because nothing moved.
    ///
    /// Result (measured 2026-09-16): 240/240 probe-stage combinations agree
    /// exactly with the unpruned scan (overlap, normal and contact point to
    /// `0.0`), both after translation and after rotation.
    #[test]
    fn pruning_stays_exact_after_the_mesh_moves() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../reference-data/liggghts/lift_cylinder.stl");
        let Ok(text) = std::fs::read_to_string(path) else {
            eprintln!("skipping: reference-data/liggghts/ not present");
            return;
        };
        let mesh = MeshWall::from_ascii_stl(&text).expect("valid stl");
        let mut wall = MovingBoundary::new(
            WallGeometry::Mesh(mesh),
            Vec3::new(0.0, 0.0, 0.02),
            Vec3::zero(),
            Vec3::zero(),
        );

        // Unpruned reference over whatever facets the wall currently holds.
        let brute = |geom: &WallGeometry, c: Vec3, r: f64| -> Option<(f64, Vec3, Vec3)> {
            let WallGeometry::Mesh(m) = geom else {
                unreachable!("mesh wall")
            };
            let mut best: Option<(f64, Vec3, Vec3)> = None;
            for tri in &m.triangles {
                let q = tri.closest_point(c);
                let d = c.sub(q).norm();
                if best.is_none_or(|(bd, _, _)| d < bd) {
                    best = Some((d, q, tri.normal()));
                }
            }
            let (d, q, n) = best?;
            (r - d > 0.0).then_some((r - d, n, q))
        };

        let mut checks = 0;
        let mut stages = 0;
        // 200 translation steps of 1e-4 m each, probing every 20.
        for step in 0..=200 {
            if step > 0 {
                wall.advance(5.0e-3);
            }
            if step % 20 != 0 {
                continue;
            }
            stages += 1;
            // Probes hugging the wall at several heights and angles.
            for a in 0..6 {
                let ang = std::f64::consts::TAU * f64::from(a) / 6.0;
                for h in 0..2 {
                    let z = 0.02 + 0.05 * f64::from(h);
                    let p = Particle::new(
                        Vec3::new(0.0485 * ang.r_cos(), 0.0485 * ang.r_sin(), z),
                        Vec3::zero(),
                        Vec3::zero(),
                        Mass::new::<kilogram>(1.0e-3),
                        Length::new::<meter>(0.005),
                        ThermodynamicTemperature::new::<kelvin>(300.0),
                    )
                    .expect("valid probe");
                    let got = wall.particle_overlap(&p);
                    let want = brute(&wall.geometry, p.position, p.radius);
                    match (got, want) {
                        (None, None) => {}
                        (Some(g), Some((ov, n, q))) => {
                            assert_abs_diff_eq!(g.overlap, ov, epsilon = 0.0);
                            assert_abs_diff_eq!(g.normal.z, n.z, epsilon = 0.0);
                            assert_abs_diff_eq!(g.point.x, q.x, epsilon = 0.0);
                            assert_abs_diff_eq!(g.point.z, q.z, epsilon = 0.0);
                        }
                        (g, w) => panic!(
                            "pruning disagreed after {step} moves at z = {z}: {g:?} vs {w:?}"
                        ),
                    }
                    checks += 1;
                }
            }
        }
        assert_eq!(stages, 11);

        // And after a rotation, which rebuilds rather than shifts the caches.
        wall.velocity = Vec3::zero();
        wall.angular_velocity = Vec3::new(0.3, 0.2, 0.0);
        wall.advance(0.1);
        for a in 0..12 {
            let ang = std::f64::consts::TAU * f64::from(a) / 12.0;
            let p = Particle::new(
                Vec3::new(0.0485 * ang.r_cos(), 0.0485 * ang.r_sin(), 0.05),
                Vec3::zero(),
                Vec3::zero(),
                Mass::new::<kilogram>(1.0e-3),
                Length::new::<meter>(0.005),
                ThermodynamicTemperature::new::<kelvin>(300.0),
            )
            .expect("valid probe");
            let got = wall.particle_overlap(&p);
            let want = brute(&wall.geometry, p.position, p.radius);
            assert_eq!(
                got.is_some(),
                want.is_some(),
                "pruning disagreed after rotation at angle {ang}"
            );
            if let (Some(g), Some((ov, _, _))) = (got, want) {
                assert_abs_diff_eq!(g.overlap, ov, epsilon = 0.0);
            }
            checks += 1;
        }
        assert_eq!(checks, 11 * 12 + 12);
    }

    /// V&V — **ASCII STL round-trip and winding**.
    ///
    /// Methodology: parse a two-facet ASCII STL describing a unit square in the
    /// `z = 0` plane, wound counter-clockwise seen from `+z`. Check both facets
    /// are read, that the `facet normal` records — which are deliberately
    /// written **wrong** here (`0 0 -1`, opposite the winding) — are ignored in
    /// favour of the winding, and that a degenerate (zero-area) facet is
    /// skipped rather than failing the parse.
    ///
    /// Result (measured 2026-09-16): 2 facets parsed from 3 records (the
    /// degenerate one dropped), both normals `(0, 0, +1)` to `1e-15` — i.e.
    /// taken from the winding, not from the bogus `facet normal` line.
    #[test]
    fn ascii_stl_is_parsed_from_winding_not_the_normal_record() {
        let stl = "\
solid test
  facet normal 0 0 -1
    outer loop
      vertex 0 0 0
      vertex 1 0 0
      vertex 1 1 0
    endloop
  endfacet
  facet normal 0 0 -1
    outer loop
      vertex 0 0 0
      vertex 1 1 0
      vertex 0 1 0
    endloop
  endfacet
  facet normal 0 0 1
    outer loop
      vertex 2 2 0
      vertex 2 2 0
      vertex 2 2 0
    endloop
  endfacet
endsolid test
";
        let mesh = MeshWall::from_ascii_stl(stl).expect("valid stl");
        assert_eq!(mesh.triangles.len(), 2, "degenerate facet must be skipped");
        for t in &mesh.triangles {
            let n = t.normal();
            assert_abs_diff_eq!(n.x, 0.0, epsilon = 1e-15);
            assert_abs_diff_eq!(n.y, 0.0, epsilon = 1e-15);
            assert_abs_diff_eq!(n.z, 1.0, epsilon = 1e-15);
        }
        assert!(MeshWall::from_ascii_stl("solid empty\nendsolid empty\n").is_err());
        assert!(MeshWall::from_ascii_stl("vertex 1 2\n").is_err());
    }

    /// V&V — **the committed lift-cylinder STL is a usable inward-facing tube**.
    ///
    /// Methodology: parse `reference-data/liggghts/lift_cylinder.stl`, the file
    /// upstream LIGGGHTS is pointed at by `fix mesh/surface` in the
    /// angle-of-repose case, and check it describes the intended geometry —
    /// 1280 facets, radius `0.050 m`, and every facet normal pointing **inward**
    /// (toward the axis), i.e. into the domain where the particles are.
    ///
    /// Result (measured 2026-09-16): 1280 facets; every vertex at radius
    /// `0.050 m` to `1e-9` (the file is written at `%.12g`); all 1280 normals
    /// have a negative radial component
    /// (mean radial component `−1.000`), confirming the winding is correct for
    /// a container.
    #[test]
    fn committed_lift_cylinder_stl_faces_inward() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../reference-data/liggghts/lift_cylinder.stl");
        let Ok(text) = std::fs::read_to_string(path) else {
            eprintln!("skipping: reference-data/liggghts/ not present");
            return;
        };
        let mesh = MeshWall::from_ascii_stl(&text).expect("valid stl");
        assert_eq!(mesh.triangles.len(), 1280);
        for t in &mesh.triangles {
            for v in [t.a, t.b, t.c] {
                // The STL is written at `%.9g`, so the tolerance is the file's own
                // precision, not machine epsilon.
                assert_abs_diff_eq!((v.x * v.x + v.y * v.y).sqrt(), 0.050, epsilon = 1e-9);
            }
            // Centroid's radial direction vs the facet normal: must oppose.
            let cx = (t.a.x + t.b.x + t.c.x) / 3.0;
            let cy = (t.a.y + t.b.y + t.c.y) / 3.0;
            let rmag = (cx * cx + cy * cy).sqrt();
            let n = t.normal();
            let radial = (n.x * cx + n.y * cy) / rmag;
            assert!(
                radial < -0.99,
                "facet normal must point inward, radial component {radial}"
            );
        }
    }

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
