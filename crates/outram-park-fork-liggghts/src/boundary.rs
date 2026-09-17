// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Independent Rust implementation — reimplemented clean-room from public
// computational-geometry and DEM literature (signed-distance functions,
// point/primitive distance geometry). NOT derived from GPL-2.0
// LIGGGHTS/LAMMPS source. See the crate NOTICE.
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

//! Phase 3 — **Boundaries** (bead `op-t3l.3`).
//!
//! Geometric domain boundaries a DEM particle can collide with: an infinite
//! [`Boundary::Plane`], a one-sided half-space [`Boundary::Wall`], an
//! axis-aligned [`Boundary::Box`] container, and an infinite
//! [`Boundary::Cylinder`] container. Each primitive answers two purely
//! **geometric** questions:
//!
//! - [`Boundary::signed_distance`] — the signed perpendicular distance from a
//!   query point to the boundary surface `[m]`.
//! - [`Boundary::particle_overlap`] — whether a sphere of radius `r` penetrates
//!   the boundary and, if so, by how much (penetration depth `δ`) and along
//!   which contact normal.
//!
//! This module is **geometry only**. It computes the overlap `δ` and the
//! contact normal that a contact-force law would consume — it does **not**
//! compute forces. The force models (Hooke, Hertz) live in Phase 2's
//! [`crate::contact`] module; the [`Contact`] returned here is exactly the
//! geometric hand-off a force model needs. This is an independent
//! implementation from standard signed-distance geometry, **not** derived from
//! LIGGGHTS/LAMMPS source (see the crate `NOTICE`).
//!
//! # Sign conventions (read once, applies to every variant)
//!
//! All lengths are in metres `[m]`. Let `q` be a query point, `p` a particle of
//! radius `r` centred at `c = p.position`.
//!
//! - **`signed_distance(q)`** is **positive when `q` is in the open domain**
//!   (the free region where particle centres are meant to live) and **negative**
//!   once `q` has crossed the boundary surface into the forbidden region; its
//!   magnitude is the perpendicular distance to the nearest surface. For the
//!   *container* primitives ([`Boundary::Wall`], [`Boundary::Box`],
//!   [`Boundary::Cylinder`]) the domain is unambiguous. The two-sided
//!   [`Boundary::Plane`] is the one exception: it has no "inside", so its
//!   `signed_distance` is the signed offset `(q − point) · n̂`, positive on the
//!   `+n̂` side (see that variant's docs).
//!
//! - **`particle_overlap(p)`** returns `Some(`[`Contact`]`)` exactly when the
//!   sphere surface protrudes past the boundary, i.e. when the penetration depth
//!
//!   ```text
//!     δ = r − s > 0,
//!   ```
//!
//!   where `s` is the perpendicular distance from the centre `c` to the surface
//!   (for the container primitives `s = signed_distance(c)`; for the two-sided
//!   `Plane`, `s = |signed_distance(c)|`). The [`Contact::normal`] `n̂_c` is a
//!   **unit vector pointing from the boundary surface into the domain** — i.e.
//!   toward the particle centre while the particle is still inside the domain —
//!   so a repulsive penalty force `F = k · δ · n̂_c` pushes the particle back
//!   into the domain. The [`Contact::point`] lies on the boundary surface,
//!   `c − n̂_c · (r − δ)` (the foot of the perpendicular from `c`).
//!
//! # Honest scope (Phase 3)
//!
//! This module implements a **verified geometric foundation only** — it has
//! **not** been validated against a DEM reference code. Concretely it provides:
//!
//! - **infinite planes** and **infinite half-space walls** (a `Wall` is a
//!   half-space plane — see [`Boundary::Wall`]); no finite/bounded planar
//!   patches,
//! - **axis-aligned** boxes only (no rotated/oriented boxes),
//! - **infinite** cylinders only (no capped/finite cylinders, no cones),
//! - **no meshed / triangulated (STL) walls**,
//! - **static boundaries only** — no moving/rotating walls, so a contact
//!   carries no wall velocity and the overlap is purely positional.
//!
//! For a container primitive (`Box`, `Cylinder`) whose particle straddles a
//! corner or edge, only the **single nearest face/surface** contact is
//! returned; simultaneous multi-face contact is not resolved (documented per
//! variant). The tests below verify each primitive's signed distance and overlap
//! against hand-computed geometry; no cross-code benchmark comparison has been
//! run — that is the later human validation step in this bead's Definition of
//! Done.
//!
//! # References (public literature / geometry — NOT LAMMPS/LIGGGHTS source)
//!
//! - C. Ericson, *Real-Time Collision Detection* (Morgan Kaufmann, 2005) —
//!   point-to-plane / point-to-AABB / point-to-cylinder distance geometry.
//! - P. J. Schneider and D. H. Eberly, *Geometric Tools for Computer Graphics*
//!   (Morgan Kaufmann, 2003) — signed-distance and closest-point formulas.
//! - I. Quílez, "Distance functions" (analytic signed-distance functions,
//!   public reference), <https://iquilezles.org/articles/distfunctions/> — the
//!   exact axis-aligned-box signed distance used here.
//! - P. A. Cundall and O. D. L. Strack, "A discrete numerical model for
//!   granular assemblies," *Géotechnique* **29**(1), 47–65 (1979) — the DEM
//!   soft-sphere overlap `δ`.
//! - T. Pöschel and T. Schwager, *Computational Granular Dynamics: Models and
//!   Algorithms* (Springer, 2005) — particle–wall overlap and contact-normal
//!   conventions.

use crate::particle::{Particle, Vec3};
use crate::DemError;

/// Normalise a vector to unit length.
///
/// Returns `v / ‖v‖`. The public constructors ([`Boundary::plane`],
/// [`Boundary::wall`], [`Boundary::cylinder`]) reject a zero-length direction
/// before it reaches here, and the query methods call this defensively so a
/// directly field-constructed [`Boundary`] with a non-unit normal still yields
/// correct results. Passing a zero vector yields a non-finite result (the
/// caller is responsible for avoiding that).
#[inline]
fn unit(v: Vec3) -> Vec3 {
    v.scale(1.0 / v.norm())
}

/// A single geometric particle–boundary contact: the hand-off a contact-force
/// law consumes.
///
/// This carries **only geometry** — penetration depth and directions, no force.
/// It is produced by [`Boundary::particle_overlap`] and would be fed to a Phase 2
/// force model ([`crate::contact`]) to obtain the actual normal force.
///
/// # Fields and units
///
/// | Field | Quantity | SI unit |
/// |---|---|---|
/// | `overlap` | penetration depth `δ = r − s` | `[m]` |
/// | `normal` | unit contact normal, boundary surface → domain (toward the particle centre) | dimensionless |
/// | `point` | contact point on the boundary surface (foot of the perpendicular from the centre) | `[m]` |
///
/// `overlap` is strictly positive for every `Contact` that
/// [`Boundary::particle_overlap`] returns (a non-penetrating particle yields
/// `None`, not a zero-overlap `Contact`). `normal` is a unit vector; applying a
/// repulsive force `F = k · overlap · normal` pushes the particle back into the
/// domain.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Contact {
    /// Penetration depth `δ = r − s > 0` `[m]`, where `s` is the perpendicular
    /// distance from the particle centre to the boundary surface.
    pub overlap: f64,
    /// Unit contact normal, pointing **from the boundary surface into the
    /// domain** — toward the particle centre while the particle is inside.
    /// Dimensionless.
    pub normal: Vec3,
    /// Contact point on the boundary surface `[m]`: the foot of the
    /// perpendicular dropped from the particle centre, `c − normal · (r − δ)`.
    pub point: Vec3,
}

/// A geometric domain boundary a DEM particle can collide with.
///
/// Enum dispatch (no `Box<dyn>`), per the workspace design rules: the set of
/// boundary primitives is closed and known at compile time, so adding a variant
/// forces every `match` to handle it.
///
/// Construct via the validated constructors ([`Boundary::plane`],
/// [`Boundary::wall`], [`Boundary::aabb`], [`Boundary::cylinder`]), which check
/// physical validity (non-zero normals/axes, `min < max`, positive radius) and
/// normalise directions. The fields are public for pattern matching and direct
/// construction; the query methods normalise defensively, but direct
/// construction skips the validity checks.
///
/// All positions and lengths are in metres `[m]`. See the module-level "Sign
/// conventions" note for the shared meaning of `signed_distance` and the
/// contact normal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Boundary {
    /// An **infinite, two-sided** plane (a thin planar divider).
    ///
    /// The plane passes through `point` `[m]` with unit outward-reference
    /// `normal` `n̂`. Being two-sided, it has no "inside": its
    /// [`Boundary::signed_distance`] is the signed offset `(q − point) · n̂`
    /// (positive on the `+n̂` side, negative on the `−n̂` side, zero on the
    /// surface), and a particle overlaps it from **either** side. The contact
    /// normal flips to point from the plane toward whichever side the particle
    /// centre is on. Use this for an internal splitter plate; for a one-sided
    /// solid barrier (a floor) use [`Boundary::Wall`] instead.
    Plane {
        /// A point the plane passes through `[m]`.
        point: Vec3,
        /// Unit plane normal (dimensionless); `+n̂` labels the positive side.
        normal: Vec3,
    },
    /// A **one-sided half-space wall**: solid material fills the closed
    /// half-space on the `−normal` side, and the open domain is the `+normal`
    /// side.
    ///
    /// The wall face passes through `point` `[m]` with unit `normal` `n̂`
    /// pointing **out of the wall, into the domain** (toward the particles). Its
    /// [`Boundary::signed_distance`] is `(q − point) · n̂` = the perpendicular
    /// distance into the domain (negative once inside the wall material). Unlike
    /// [`Boundary::Plane`], a `Wall` only ever pushes along its fixed outward
    /// normal `+n̂`: a particle overlaps when its centre is within `r` of the
    /// face (including having passed through into the material), and the contact
    /// normal is always `n̂`. This is the standard DEM floor/wall half-space.
    Wall {
        /// A point on the wall face `[m]`.
        point: Vec3,
        /// Unit outward normal (dimensionless), from the wall into the domain.
        normal: Vec3,
    },
    /// An **axis-aligned box** container; the open domain is the box interior.
    ///
    /// `min` and `max` `[m]` are the lower and upper corners, with
    /// `min.x < max.x`, `min.y < max.y`, `min.z < max.z`. Particles live inside;
    /// [`Boundary::signed_distance`] is positive in the interior (equal to the
    /// distance to the nearest face) and negative outside. Overlap is resolved
    /// against the **single nearest interior face** (its inward normal is one of
    /// `±x`, `±y`, `±z`); simultaneous two/three-face corner contact is not
    /// resolved — the nearest (deepest-penetration) face is returned.
    Box {
        /// Lower corner `[m]` (`min.i < max.i` on every axis `i`).
        min: Vec3,
        /// Upper corner `[m]` (`max.i > min.i` on every axis `i`).
        max: Vec3,
    },
    /// An **infinite circular cylinder** container; the open domain is the
    /// cylinder interior.
    ///
    /// The axis passes through `axis_point` `[m]` along unit direction
    /// `axis_dir`, and the cylindrical wall sits at radial distance `radius`
    /// `[m] > 0` from the axis. Particles live inside;
    /// [`Boundary::signed_distance`] is `radius − ρ` where `ρ` is the query
    /// point's perpendicular distance from the axis (positive inside, negative
    /// outside). The contact normal points radially inward (toward the axis).
    /// The cylinder is infinite along its axis (no end caps). A particle whose
    /// centre lies exactly on the axis has no defined radial direction and
    /// yields `None` from [`Boundary::particle_overlap`].
    Cylinder {
        /// A point on the cylinder axis `[m]`.
        axis_point: Vec3,
        /// Unit axis direction (dimensionless).
        axis_dir: Vec3,
        /// Cylinder radius `[m]`, strictly positive.
        radius: f64,
    },
}

impl Boundary {
    /// Construct an infinite two-sided [`Boundary::Plane`] through `point` with
    /// `normal` (any non-zero length; stored normalised to a unit vector).
    ///
    /// # Errors
    ///
    /// Returns [`DemError::InvalidInput`] if `normal` has (near-)zero length, as
    /// a plane orientation is then undefined.
    pub fn plane(point: Vec3, normal: Vec3) -> Result<Self, DemError> {
        let n = normal.norm();
        if n > 0.0 {
            Ok(Self::Plane {
                point,
                normal: normal.scale(1.0 / n),
            })
        } else {
            Err(DemError::InvalidInput(
                "plane normal must have non-zero length".to_string(),
            ))
        }
    }

    /// Construct a one-sided half-space [`Boundary::Wall`] whose face passes
    /// through `point` with outward `normal` (any non-zero length; stored
    /// normalised). The `+normal` side is the open domain; the `−normal` side is
    /// solid wall material.
    ///
    /// # Errors
    ///
    /// Returns [`DemError::InvalidInput`] if `normal` has (near-)zero length.
    pub fn wall(point: Vec3, normal: Vec3) -> Result<Self, DemError> {
        let n = normal.norm();
        if n > 0.0 {
            Ok(Self::Wall {
                point,
                normal: normal.scale(1.0 / n),
            })
        } else {
            Err(DemError::InvalidInput(
                "wall normal must have non-zero length".to_string(),
            ))
        }
    }

    /// Construct an axis-aligned [`Boundary::Box`] with lower corner `min` and
    /// upper corner `max` `[m]`.
    ///
    /// # Errors
    ///
    /// Returns [`DemError::InvalidInput`] unless `min.i < max.i` on **every**
    /// axis (a degenerate or inverted box has no interior).
    pub fn aabb(min: Vec3, max: Vec3) -> Result<Self, DemError> {
        if min.x < max.x && min.y < max.y && min.z < max.z {
            Ok(Self::Box { min, max })
        } else {
            Err(DemError::InvalidInput(format!(
                "box requires min < max on every axis, got min=({}, {}, {}) max=({}, {}, {})",
                min.x, min.y, min.z, max.x, max.y, max.z
            )))
        }
    }

    /// Construct an infinite [`Boundary::Cylinder`] with the given `axis_point`,
    /// `axis_dir` (any non-zero length; stored normalised), and `radius` `[m]`.
    ///
    /// # Errors
    ///
    /// Returns [`DemError::InvalidInput`] if `axis_dir` has (near-)zero length
    /// or if `radius` is not strictly positive.
    pub fn cylinder(axis_point: Vec3, axis_dir: Vec3, radius: f64) -> Result<Self, DemError> {
        // `> 0.0` (not `>= `) rejects zero length and also a NaN norm, so no
        // non-finite direction slips through.
        let a = axis_dir.norm();
        if a > 0.0 {
            if radius > 0.0 {
                Ok(Self::Cylinder {
                    axis_point,
                    axis_dir: axis_dir.scale(1.0 / a),
                    radius,
                })
            } else {
                Err(DemError::InvalidInput(format!(
                    "cylinder radius must be strictly positive, got {radius} m"
                )))
            }
        } else {
            Err(DemError::InvalidInput(
                "cylinder axis direction must have non-zero length".to_string(),
            ))
        }
    }

    /// Signed perpendicular distance from `point` to this boundary's surface
    /// `[m]`.
    ///
    /// Positive when `point` is in the open domain, negative once it has crossed
    /// the surface; magnitude is the perpendicular distance to the nearest
    /// surface. See the module-level "Sign conventions" note — in particular the
    /// two-sided [`Boundary::Plane`] returns the signed offset `(point − p)· n̂`
    /// (positive on the `+n̂` side), which has no "inside/outside" meaning.
    #[must_use]
    pub fn signed_distance(&self, point: Vec3) -> f64 {
        match *self {
            Self::Plane { point: p, normal } => {
                // Signed offset along the unit normal: (q − p) · n̂.
                point.sub(p).dot(unit(normal))
            }
            Self::Wall { point: p, normal } => {
                // Perpendicular distance into the domain (+n̂ side); < 0 inside
                // the wall material.
                point.sub(p).dot(unit(normal))
            }
            Self::Box { min, max } => {
                // Exact axis-aligned-box signed distance (Quílez), negated so the
                // interior is positive. For an interior point this equals the
                // distance to the nearest face.
                let cx = 0.5 * (min.x + max.x);
                let cy = 0.5 * (min.y + max.y);
                let cz = 0.5 * (min.z + max.z);
                let hx = 0.5 * (max.x - min.x);
                let hy = 0.5 * (max.y - min.y);
                let hz = 0.5 * (max.z - min.z);
                let dx = (point.x - cx).abs() - hx;
                let dy = (point.y - cy).abs() - hy;
                let dz = (point.z - cz).abs() - hz;
                let outside = Vec3::new(dx.max(0.0), dy.max(0.0), dz.max(0.0)).norm();
                let inside = dx.max(dy).max(dz).min(0.0);
                // outside + inside is the standard SDF (negative interior);
                // negate for the positive-interior convention.
                -(outside + inside)
            }
            Self::Cylinder {
                axis_point,
                axis_dir,
                radius,
            } => {
                let a = unit(axis_dir);
                let w = point.sub(axis_point);
                let axial = w.dot(a);
                let radial = w.sub(a.scale(axial));
                radius - radial.norm()
            }
        }
    }

    /// Geometric overlap of particle `p` (a sphere of radius `r = p.radius`
    /// centred at `c = p.position`) with this boundary.
    ///
    /// Returns `Some(`[`Contact`]`)` with penetration depth `δ = r − s > 0` and
    /// the inward contact normal when the sphere protrudes past the surface,
    /// else `None`. Here `s` is the perpendicular distance from `c` to the
    /// surface — `s = signed_distance(c)` for the container primitives, and
    /// `s = |signed_distance(c)|` for the two-sided [`Boundary::Plane`]. See the
    /// module-level "Sign conventions" note. This is geometry only; the returned
    /// `Contact` is fed to a Phase 2 force model — no force is computed here.
    ///
    /// Units: `[m]` throughout (the normal is dimensionless).
    #[must_use]
    pub fn particle_overlap(&self, p: &Particle) -> Option<Contact> {
        let c = p.position;
        let r = p.radius;
        match *self {
            Self::Plane { point, normal } => {
                let n = unit(normal);
                let signed = c.sub(point).dot(n); // may be either sign (two-sided)
                let s = signed.abs(); // perpendicular distance to the plane
                let delta = r - s;
                // `> 0.0` (not `>= `) also yields None for a NaN depth.
                if delta > 0.0 {
                    // Normal points from the plane toward the particle centre's side.
                    let nc = if signed >= 0.0 { n } else { n.scale(-1.0) };
                    Some(Contact {
                        overlap: delta,
                        normal: nc,
                        point: c.sub(nc.scale(s)),
                    })
                } else {
                    None
                }
            }
            Self::Wall { point, normal } => {
                let n = unit(normal);
                // Perpendicular distance into the domain (may be negative if the
                // centre has crossed into the wall material).
                let s = c.sub(point).dot(n);
                let delta = r - s;
                if delta > 0.0 {
                    // One-sided: the normal is always the fixed outward +n̂.
                    Some(Contact {
                        overlap: delta,
                        normal: n,
                        point: c.sub(n.scale(s)),
                    })
                } else {
                    None
                }
            }
            Self::Box { min, max } => {
                // Distance from the centre to each of the six faces, positive on
                // the interior side of that face, paired with that face's inward
                // normal. The smallest distance is the nearest face (deepest
                // penetration); corner/edge multi-face contact is not resolved.
                let faces = [
                    (c.x - min.x, Vec3::new(1.0, 0.0, 0.0)),
                    (max.x - c.x, Vec3::new(-1.0, 0.0, 0.0)),
                    (c.y - min.y, Vec3::new(0.0, 1.0, 0.0)),
                    (max.y - c.y, Vec3::new(0.0, -1.0, 0.0)),
                    (c.z - min.z, Vec3::new(0.0, 0.0, 1.0)),
                    (max.z - c.z, Vec3::new(0.0, 0.0, -1.0)),
                ];
                let (s, nc) = faces
                    .iter()
                    .copied()
                    .reduce(|a, b| if b.0 < a.0 { b } else { a })
                    .expect("six faces");
                let delta = r - s;
                if delta > 0.0 {
                    Some(Contact {
                        overlap: delta,
                        normal: nc,
                        point: c.sub(nc.scale(s)),
                    })
                } else {
                    None
                }
            }
            Self::Cylinder {
                axis_point,
                axis_dir,
                radius,
            } => {
                let a = unit(axis_dir);
                let w = c.sub(axis_point);
                let axial = w.dot(a);
                let radial = w.sub(a.scale(axial));
                let rho = radial.norm();
                if rho == 0.0 {
                    // Centre on the axis: the radial contact direction is
                    // undefined, so no well-defined contact is reported.
                    return None;
                }
                let s = radius - rho; // perpendicular distance to the wall
                let delta = r - s;
                if delta > 0.0 {
                    // Inward normal points from the wall toward the axis: −ρ̂.
                    let nc = radial.scale(-1.0 / rho);
                    Some(Contact {
                        overlap: delta,
                        normal: nc,
                        point: c.sub(nc.scale(s)),
                    })
                } else {
                    None
                }
            }
        }
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

    /// Helper: a valid particle centred at `center` with radius `radius_m`.
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

    /// V&V — **plane signed distance at known points**.
    ///
    /// Methodology: an infinite plane through the origin with unit normal
    /// `n̂ = +ẑ`. The signed distance is `(q − 0) · ẑ = q.z`. Evaluate at a point
    /// on the `+n̂` side `q = (7, −4, 2)` (expect `+2`), on the `−n̂` side
    /// `q = (1, 1, −3)` (expect `−3`), and on the surface `q = (5, 6, 0)`
    /// (expect `0`). The non-`z` coordinates must not affect the result. Pass
    /// criterion: exact to `1e-15`.
    ///
    /// Result (measured 2026-07-23): `+2.0`, `−3.0`, `0.0` reproduced to
    /// `< 1e-15` (round-off). A deliberately non-unit input normal `(0,0,4)`
    /// gave the same numbers, confirming the defensive normalisation.
    #[test]
    fn plane_signed_distance_known_points() {
        let plane = Boundary::plane(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).unwrap();
        assert_abs_diff_eq!(
            plane.signed_distance(Vec3::new(7.0, -4.0, 2.0)),
            2.0,
            epsilon = 1e-15
        );
        assert_abs_diff_eq!(
            plane.signed_distance(Vec3::new(1.0, 1.0, -3.0)),
            -3.0,
            epsilon = 1e-15
        );
        assert_abs_diff_eq!(
            plane.signed_distance(Vec3::new(5.0, 6.0, 0.0)),
            0.0,
            epsilon = 1e-15
        );
        // Non-unit normal is normalised defensively.
        let plane2 = Boundary::Plane {
            point: Vec3::zero(),
            normal: Vec3::new(0.0, 0.0, 4.0),
        };
        assert_abs_diff_eq!(
            plane2.signed_distance(Vec3::new(0.0, 0.0, 2.0)),
            2.0,
            epsilon = 1e-15
        );
    }

    /// V&V — **wall (half-space) signed distance at known points**.
    ///
    /// Methodology: a floor wall with face through `(0, 0, 0)` and outward
    /// normal `n̂ = +ẑ` (open domain above, solid material below). Signed
    /// distance `= q.z`: a point in the domain `q = (2, 3, 0.5)` gives `+0.5`; a
    /// point inside the material `q = (0, 0, −0.25)` gives `−0.25`; on the face
    /// `q = (9, 9, 0)` gives `0`. Pass criterion: exact to `1e-15`.
    ///
    /// Result (measured 2026-07-23): `+0.5`, `−0.25`, `0.0` reproduced to
    /// `< 1e-15` (round-off).
    #[test]
    fn wall_signed_distance_known_points() {
        let wall = Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).unwrap();
        assert_abs_diff_eq!(
            wall.signed_distance(Vec3::new(2.0, 3.0, 0.5)),
            0.5,
            epsilon = 1e-15
        );
        assert_abs_diff_eq!(
            wall.signed_distance(Vec3::new(0.0, 0.0, -0.25)),
            -0.25,
            epsilon = 1e-15
        );
        assert_abs_diff_eq!(
            wall.signed_distance(Vec3::new(9.0, 9.0, 0.0)),
            0.0,
            epsilon = 1e-15
        );
    }

    /// V&V — **axis-aligned box signed distance at known points**.
    ///
    /// Methodology: a box `[0,0,0]–[2,2,2]` (interior = domain, positive-inside
    /// convention). Check: centre `(1,1,1)` → nearest face is `1.0` away, expect
    /// `+1.0`; near a face `(0.2, 1, 1)` → `+0.2`; on a face `(0, 1, 1)` → `0.0`;
    /// outside along one axis `(3, 1, 1)` → `−1.0`; outside past a corner
    /// `(3, 3, 1)` → Euclidean corner distance `sqrt(1²+1²) = √2 ≈ −1.41421356`.
    /// Pass criterion: exact to `1e-12`.
    ///
    /// Result (measured 2026-07-23): `+1.0`, `+0.2`, `0.0`, `−1.0`,
    /// `−1.4142135623…` reproduced to `< 1e-12`, matching the exact
    /// axis-aligned-box signed-distance formula (including the corner Euclidean
    /// term).
    #[test]
    fn box_signed_distance_known_points() {
        let b = Boundary::aabb(Vec3::zero(), Vec3::new(2.0, 2.0, 2.0)).unwrap();
        assert_abs_diff_eq!(
            b.signed_distance(Vec3::new(1.0, 1.0, 1.0)),
            1.0,
            epsilon = 1e-12
        );
        assert_abs_diff_eq!(
            b.signed_distance(Vec3::new(0.2, 1.0, 1.0)),
            0.2,
            epsilon = 1e-12
        );
        assert_abs_diff_eq!(
            b.signed_distance(Vec3::new(0.0, 1.0, 1.0)),
            0.0,
            epsilon = 1e-12
        );
        assert_abs_diff_eq!(
            b.signed_distance(Vec3::new(3.0, 1.0, 1.0)),
            -1.0,
            epsilon = 1e-12
        );
        assert_abs_diff_eq!(
            b.signed_distance(Vec3::new(3.0, 3.0, 1.0)),
            -std::f64::consts::SQRT_2,
            epsilon = 1e-12
        );
    }

    /// V&V — **infinite cylinder signed distance at known points**.
    ///
    /// Methodology: a cylinder about the z-axis through the origin with radius
    /// `R = 1.0` (interior = domain). Signed distance `= R − ρ`, `ρ` the radial
    /// distance from the axis. On the axis-parallel line at `ρ = 0.5`,
    /// `q = (0.5, 0, 4)` → `+0.5` (the axial coordinate must not matter); outside
    /// at `q = (2, 0, 0)` (`ρ = 2`) → `−1.0`; on the wall `q = (0, 1, 0)`
    /// (`ρ = 1`) → `0.0`; a diagonal interior point `q = (0.6, 0.8, 3)`
    /// (`ρ = 1.0`) → `0.0`. Pass criterion: exact to `1e-12`.
    ///
    /// Result (measured 2026-07-23): `+0.5`, `−1.0`, `0.0`, `0.0` reproduced to
    /// `< 1e-12` — the `(0.6, 0.8)` point sits exactly on the unit circle, so its
    /// signed distance is zero, confirming the radial projection removes the
    /// axial component correctly.
    #[test]
    fn cylinder_signed_distance_known_points() {
        let cyl = Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), 1.0).unwrap();
        assert_abs_diff_eq!(
            cyl.signed_distance(Vec3::new(0.5, 0.0, 4.0)),
            0.5,
            epsilon = 1e-12
        );
        assert_abs_diff_eq!(
            cyl.signed_distance(Vec3::new(2.0, 0.0, 0.0)),
            -1.0,
            epsilon = 1e-12
        );
        assert_abs_diff_eq!(
            cyl.signed_distance(Vec3::new(0.0, 1.0, 0.0)),
            0.0,
            epsilon = 1e-12
        );
        assert_abs_diff_eq!(
            cyl.signed_distance(Vec3::new(0.6, 0.8, 3.0)),
            0.0,
            epsilon = 1e-12
        );
    }

    /// V&V — **sphere straddling a two-sided plane: overlap and outward
    /// normal**.
    ///
    /// Methodology: an infinite plane through the origin, normal `+ẑ`. A sphere
    /// of radius `r = 0.5` centred at `c = (0, 0, 0.3)` straddles the plane on
    /// the `+ẑ` side: perpendicular distance `s = 0.3`, so overlap
    /// `δ = r − s = 0.2`, contact normal `+ẑ` (toward the centre's side), and
    /// contact point `c − n̂·s = (0, 0, 0)` on the plane. A mirror centre
    /// `c = (0, 0, −0.3)` must give the same `δ = 0.2` but flipped normal `−ẑ`
    /// (the two-sided plane repels from either face). A far sphere
    /// `c = (0, 0, 0.9)`, `r = 0.5` (`s = 0.9 > r`) yields no contact. Pass
    /// criterion: `δ` and the normal/point components exact to `1e-12`.
    ///
    /// Result (measured 2026-07-23): `+ẑ` case `δ = 0.2`, `n̂_c = (0,0,1)`,
    /// contact point `(0,0,0)`; `−ẑ` case `δ = 0.2`, `n̂_c = (0,0,−1)`, contact
    /// point `(0,0,0)`; far case `None`. All reproduced to `< 1e-12`.
    #[test]
    fn sphere_straddling_plane_overlap_and_normal() {
        let plane = Boundary::plane(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).unwrap();

        let c = plane
            .particle_overlap(&particle_at(Vec3::new(0.0, 0.0, 0.3), 0.5))
            .unwrap();
        assert_abs_diff_eq!(c.overlap, 0.2, epsilon = 1e-12);
        assert_abs_diff_eq!(c.normal.z, 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(c.normal.x, 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(c.normal.y, 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(c.point.z, 0.0, epsilon = 1e-12);

        // Mirror side: same overlap, flipped normal.
        let c2 = plane
            .particle_overlap(&particle_at(Vec3::new(0.0, 0.0, -0.3), 0.5))
            .unwrap();
        assert_abs_diff_eq!(c2.overlap, 0.2, epsilon = 1e-12);
        assert_abs_diff_eq!(c2.normal.z, -1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(c2.point.z, 0.0, epsilon = 1e-12);

        // Too far: no contact.
        assert!(plane
            .particle_overlap(&particle_at(Vec3::new(0.0, 0.0, 0.9), 0.5))
            .is_none());
    }

    /// V&V — **one-sided wall: contact from the front only**.
    ///
    /// Methodology: a floor wall through the origin, outward normal `+ẑ`. A
    /// sphere `r = 0.5` centred at `c = (0, 0, 0.4)` in the domain penetrates:
    /// `s = 0.4`, `δ = 0.1`, normal fixed at `+ẑ`, contact point `(0, 0, 0)`. A
    /// sphere well above the floor `c = (0, 0, 1.0)`, `r = 0.5` (`s = 1.0 > r`)
    /// does not touch → `None`. A sphere whose centre has sunk into the material
    /// `c = (0, 0, −0.2)`, `r = 0.5` gives `δ = r − s = 0.5 − (−0.2) = 0.7` with
    /// the same `+ẑ` normal (the one-sided wall pushes it back out). Pass
    /// criterion: exact to `1e-12`.
    ///
    /// Result (measured 2026-07-23): front case `δ = 0.1`, `n̂_c = (0,0,1)`,
    /// point `(0,0,0)`; far case `None`; sunk case `δ = 0.7`, `n̂_c = (0,0,1)`.
    /// All reproduced to `< 1e-12`.
    #[test]
    fn wall_one_sided_overlap() {
        let wall = Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).unwrap();

        let c = wall
            .particle_overlap(&particle_at(Vec3::new(0.0, 0.0, 0.4), 0.5))
            .unwrap();
        assert_abs_diff_eq!(c.overlap, 0.1, epsilon = 1e-12);
        assert_abs_diff_eq!(c.normal.z, 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(c.point.z, 0.0, epsilon = 1e-12);

        assert!(wall
            .particle_overlap(&particle_at(Vec3::new(0.0, 0.0, 1.0), 0.5))
            .is_none());

        let sunk = wall
            .particle_overlap(&particle_at(Vec3::new(0.0, 0.0, -0.2), 0.5))
            .unwrap();
        assert_abs_diff_eq!(sunk.overlap, 0.7, epsilon = 1e-12);
        assert_abs_diff_eq!(sunk.normal.z, 1.0, epsilon = 1e-12);
    }

    /// V&V — **particle inside an axis-aligned box, near one face**.
    ///
    /// Methodology: a box `[0,0,0]–[10,10,10]`. A sphere `r = 0.5` centred at
    /// `c = (0.3, 5, 5)` is nearest the low-x face (distance `0.3`), so
    /// `δ = r − 0.3 = 0.2`, inward normal `+x̂ = (1,0,0)`, contact point on that
    /// face `(0, 5, 5)`. A well-centred sphere `c = (5,5,5)`, `r = 0.5` (nearest
    /// face `5 > r`) reports no contact. Pass criterion: exact to `1e-12`.
    ///
    /// Result (measured 2026-07-23): near-face case `δ = 0.2`,
    /// `n̂_c = (1,0,0)`, contact point `(0,5,5)`; centred case `None`. All
    /// reproduced to `< 1e-12`.
    #[test]
    fn particle_inside_box_near_face() {
        let b = Boundary::aabb(Vec3::zero(), Vec3::new(10.0, 10.0, 10.0)).unwrap();

        let c = b
            .particle_overlap(&particle_at(Vec3::new(0.3, 5.0, 5.0), 0.5))
            .unwrap();
        assert_abs_diff_eq!(c.overlap, 0.2, epsilon = 1e-12);
        assert_abs_diff_eq!(c.normal.x, 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(c.normal.y, 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(c.normal.z, 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(c.point.x, 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(c.point.y, 5.0, epsilon = 1e-12);
        assert_abs_diff_eq!(c.point.z, 5.0, epsilon = 1e-12);

        assert!(b
            .particle_overlap(&particle_at(Vec3::new(5.0, 5.0, 5.0), 0.5))
            .is_none());
    }

    /// V&V — **particle inside vs. outside the cylinder radius**.
    ///
    /// Methodology: a cylinder about the z-axis through the origin, radius
    /// `R = 2.0`. A sphere `r = 0.5` centred at `c = (1.6, 0, 0)` (`ρ = 1.6`)
    /// nears the wall: `s = R − ρ = 0.4`, `δ = r − s = 0.1`, inward normal
    /// `−ρ̂ = (−1, 0, 0)`, contact point on the wall `(2.0, 0, 0)`. A sphere well
    /// inside `c = (1.0, 0, 0)` (`ρ = 1.0`, `s = 1.0 > r`) reports no contact.
    /// Pass criterion: exact to `1e-12`.
    ///
    /// Result (measured 2026-07-23): near-wall case `δ = 0.1`,
    /// `n̂_c = (−1,0,0)`, contact point `(2,0,0)`; deep-inside case `None`. All
    /// reproduced to `< 1e-12`.
    #[test]
    fn particle_inside_outside_cylinder_radius() {
        let cyl = Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), 2.0).unwrap();

        let c = cyl
            .particle_overlap(&particle_at(Vec3::new(1.6, 0.0, 0.0), 0.5))
            .unwrap();
        assert_abs_diff_eq!(c.overlap, 0.1, epsilon = 1e-12);
        assert_abs_diff_eq!(c.normal.x, -1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(c.normal.y, 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(c.normal.z, 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(c.point.x, 2.0, epsilon = 1e-12);

        assert!(cyl
            .particle_overlap(&particle_at(Vec3::new(1.0, 0.0, 0.0), 0.5))
            .is_none());

        // Degenerate: centre exactly on the axis → no defined radial normal.
        assert!(cyl
            .particle_overlap(&particle_at(Vec3::zero(), 0.5))
            .is_none());
    }

    /// Constructor input validation: zero-length normals/axes, an inverted box,
    /// and a non-positive cylinder radius must all be rejected with
    /// [`DemError::InvalidInput`], while valid inputs succeed.
    #[test]
    fn constructors_reject_invalid_inputs() {
        // Zero-length plane / wall / cylinder-axis directions.
        assert!(Boundary::plane(Vec3::zero(), Vec3::zero()).is_err());
        assert!(Boundary::wall(Vec3::zero(), Vec3::zero()).is_err());
        assert!(Boundary::cylinder(Vec3::zero(), Vec3::zero(), 1.0).is_err());
        // Inverted / degenerate box.
        assert!(Boundary::aabb(Vec3::new(1.0, 0.0, 0.0), Vec3::zero()).is_err());
        assert!(Boundary::aabb(Vec3::zero(), Vec3::new(0.0, 1.0, 1.0)).is_err()); // equal on x
                                                                                  // Non-positive cylinder radius.
        assert!(Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), 0.0).is_err());
        assert!(Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), -2.0).is_err());
        // Valid inputs succeed.
        assert!(Boundary::plane(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).is_ok());
        assert!(Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).is_ok());
        assert!(Boundary::aabb(Vec3::zero(), Vec3::new(1.0, 1.0, 1.0)).is_ok());
        assert!(Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), 1.0).is_ok());
    }
}
