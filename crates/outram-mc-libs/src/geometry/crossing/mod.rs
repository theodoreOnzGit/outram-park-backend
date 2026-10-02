// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from OpenMC (src/surface.cpp, src/geometry.cpp, src/particle.cpp;
// MIT licence, see LICENSE.openmc). Split out of this crate's former
// `geometry/surface.rs` and `geometry/geometry.rs` on 2026-10-02 (GitHub
// issue #486).
//
// This file is part of OUTRAM PARK. GPL-3.0-only; see LICENSE.

//! **Transport-state work on the CSG types `outram-blender` owns.**
//!
//! Since GitHub issue #486 (2026-10-02) the CSG description and its pure
//! navigation kernel live in [`outram_blender::csg`]. What needs this crate's
//! random-number stream, scattering kinematics, materials or lattice
//! acceleration stays here, as extension traits (static dispatch, no `dyn`)
//! implemented on the moved types. They are re-exported from the old module
//! paths and from the prelude, so a `use outram_mc_libs::prelude::*` caller
//! sees no change; a caller importing the type by path brings the trait in
//! with one more `use`.
//!
//! | trait | on | methods |
//! |---|---|---|
//! | [`SurfaceKindExt`] | [`SurfaceKind`] | `diffuse_reflect`, `sphere_centre_radius`, `overlaps_voxel` |
//! | [`GeometryExt`] | [`Geometry`] | `cross_surface`, `cross_surface_in_frame`, `distance_out_of_level`, `sigma_t_at`, `validate_boundary_conditions` |

use outram_blender::csg::cell::{HalfSpaceSense, SurfaceToken};
use outram_blender::csg::geometry::{Geometry, GeometryPath};
use outram_blender::csg::position::{stream, Direction, Position};
use outram_blender::csg::surface::BoundaryType;
use crate::material::material::Material;
use crate::material::nuclide::Nuclide;
use outram_blender::csg::surface::SurfaceKind;

/// Surface methods that stay in `outram-mc-libs` (GitHub #486): the white
/// boundary's cosine-law re-emission, which draws from this crate's RNG and
/// scattering kinematics, and the two virtual-lattice helpers.
pub trait SurfaceKindExt {
    /// **Diffuse** (white) reflection of `u` off this surface at `r`: re-emit
    /// into the half-space the particle came from with a **cosine-law** polar
    /// distribution about the normal and a uniform azimuth.
    ///
    /// Ported from `Surface::diffuse_reflect` (`src/surface.cpp:144`) at OpenMC
    /// `afa7a14`. Upstream, in full:
    ///
    /// ```text
    /// Direction n = this->normal(r);  n /= n.norm();
    /// const double projection = n.dot(u);
    /// const double mu = (projection >= 0.0) ? -std::sqrt(prn(seed))
    ///                                       :  std::sqrt(prn(seed));
    /// u = rotate_angle(n, mu, nullptr, seed);
    /// return u / u.norm();
    /// ```
    ///
    /// # Why `sqrt`, and why the sign flips on the projection
    ///
    /// The cosine (Lambert) law has `p(mu) = 2 mu` on `[0, 1]`, so `F(mu) = mu^2`
    /// and the inverse transform is `mu = sqrt(xi)`. That is the whole of the
    /// sampling; there is no rejection.
    ///
    /// The sign is chosen so the particle goes **back into the cell it came
    /// from**. `projection = n . u` is positive when the incident direction
    /// already points along the outward normal, and in that case the outgoing
    /// cosine about `n` must be negative. Getting this backwards does not crash:
    /// it emits the particle out of the problem, which reads as leakage and is
    /// the same silent-wrong-answer shape this whole boundary condition exists
    /// to remove.
    ///
    /// # This is NOT the Wigner-Seitz white boundary
    ///
    /// `examples/lump_self_shielding_scan.rs` has its own `CellBoundary::White`,
    /// which re-enters at a **uniformly random point** on the sphere with a
    /// cosine-distributed inward direction. That is the lattice-cell (Wigner-
    /// Seitz) closure and it is a *different* condition: it randomises position
    /// as well as direction, which is what destroys the impact-parameter memory
    /// that made specular reflection wrong by +39-50 % there. This one keeps the
    /// crossing point and randomises only the direction, exactly as upstream's
    /// surface `WhiteBC` does. Checked and deliberately not unified: they are
    /// two conditions that share a name, not one condition implemented twice.
    fn diffuse_reflect(&self, r: Position, u: Direction, seed: &mut u64) -> Direction;
    /// Centre `[x0, y0, z0]` (cm) and radius (cm), for surfaces that have them.
    ///
    /// `Some` for [`Sphere`] only; `None` for every other variant. Mirrors
    /// `Surface::get_center` / `Surface::get_radius` in the vendored
    /// `liangjg/openmc` `virtual_lattice` fork
    /// (`include/openmc/surface.h:88`, `src/surface.cpp:762`), where the base
    /// class returns an empty vector and only `SurfaceSphere` overrides them.
    ///
    /// Used by [`crate::geometry::virtual_lattice`] to place TRISO kernels in
    /// voxels and to test point containment.
    fn sphere_centre_radius(&self) -> Option<([f64; 3], f64)>;
    /// Does this surface overlap the axis-aligned voxel of half-extent
    /// `pitch/2` centred at `centre` (both cm)?
    ///
    /// Ported from `Surface::triso_in_mesh` in the vendored `liangjg/openmc`
    /// `virtual_lattice` fork (`src/surface.cpp`). Used by
    /// [`crate::geometry::virtual_lattice::VirtualLattice::build`] to decide
    /// bucket membership.
    ///
    /// # Only `Sphere` is implemented — and that matches upstream
    ///
    /// Upstream declares `triso_in_mesh` as a virtual on the base `Surface` and
    /// overrides it on all 15 concrete types, but **14 of those 15 overrides
    /// are five-line `return false;` stubs**. Only `SurfaceSphere`
    /// (`src/surface.cpp:724`, 42 lines) carries real logic. The arms below are
    /// written out one per variant rather than collapsed into a wildcard so the
    /// correspondence with upstream stays one-to-one and diffable, and so
    /// adding a variant forces a decision here.
    ///
    /// The practical consequence: a virtual lattice only ever accelerates
    /// spheres. Handing it a cylinder or plane registers that surface in no
    /// voxel, and a traversal will never report it — see
    /// [`crate::geometry::virtual_lattice::BuildReport::unregistered`], which
    /// makes the omission observable rather than silent.
    ///
    /// # Sphere test
    ///
    /// Standard sphere-versus-AABB check: accumulate the squared distance from
    /// the centre to the box along each axis (zero on axes where the centre
    /// lies within the slab), and compare against the radius.
    ///
    /// Upstream writes `sqrt(dis_x + dis_y + dis_z) < radius_`; this port
    /// compares `d2 < r*r` instead. The two are identical for non-negative
    /// operands, and the squared form avoids a `sqrt` in a build loop that runs
    /// 27 times per TRISO particle. The comparison stays **strict**, as
    /// upstream: a sphere exactly tangent to a voxel face is *not* registered
    /// in it.
    fn overlaps_voxel(&self, centre: [f64; 3], pitch: [f64; 3]) -> bool;
}

impl SurfaceKindExt for SurfaceKind {
    #[inline]
    fn diffuse_reflect(&self, r: Position, u: Direction, seed: &mut u64) -> Direction {
        let n = self.normal(r);
        // `normal` already returns a unit vector for every surface in this enum,
        // but upstream re-normalises here and the cost is one sqrt on a path
        // taken once per boundary crossing -- keep the guard rather than rely on
        // every one of fifteen `normal` impls staying exactly unit forever.
        let norm = (n.u * n.u + n.v * n.v + n.w * n.w).sqrt();
        let n = Direction::new(n.u / norm, n.v / norm, n.w / norm);

        let projection = n.u * u.u + n.v * u.v + n.w * u.w;
        let xi = crate::rng::lcg::prn(seed);
        let mu = if projection >= 0.0 {
            -xi.sqrt()
        } else {
            xi.sqrt()
        };

        let out = crate::physics::scatter::rotate_direction(n, mu, seed);
        let norm = (out.u * out.u + out.v * out.v + out.w * out.w).sqrt();
        Direction::new(out.u / norm, out.v / norm, out.w / norm)
    }

    #[inline]
    fn sphere_centre_radius(&self) -> Option<([f64; 3], f64)> {
        match self {
            Self::Sphere(s) => Some(([s.x0, s.y0, s.z0], s.r)),
            _ => None,
        }
    }

    #[inline]
    fn overlaps_voxel(&self, centre: [f64; 3], pitch: [f64; 3]) -> bool {
        match self {
            Self::Sphere(s) => {
                let c = [s.x0, s.y0, s.z0];
                let mut d2 = 0.0;
                for d in 0..3 {
                    let lo = centre[d] - pitch[d] / 2.0;
                    let hi = centre[d] + pitch[d] / 2.0;
                    let gap = if c[d] < lo {
                        lo - c[d]
                    } else if c[d] > hi {
                        c[d] - hi
                    } else {
                        0.0
                    };
                    d2 += gap * gap;
                }
                d2 < s.r * s.r
            }
            // The 14 upstream stubs. Not "unimplemented here" — upstream
            // returns false for all of these too (see the doc comment above).
            Self::XPlane(_) => false,
            Self::YPlane(_) => false,
            Self::ZPlane(_) => false,
            Self::Plane(_) => false,
            Self::XCylinder(_) => false,
            Self::YCylinder(_) => false,
            Self::ZCylinder(_) => false,
            Self::XCone(_) => false,
            Self::YCone(_) => false,
            Self::ZCone(_) => false,
            Self::Quadric(_) => false,
            Self::XTorus(_) => false,
            Self::YTorus(_) => false,
            Self::ZTorus(_) => false,
        }
    }
}

/// Post-crossing state returned by [`Geometry::cross_surface`].
///
/// Feed `r`, `u` and `on_surface` straight back into the transport loop's next
/// [`Geometry::locate`] call; `alive` is `false` only for a vacuum (leak)
/// crossing, where the other fields are the escape state.
#[derive(Debug, Clone, Copy)]
pub struct SurfaceCrossing {
    /// Position \[cm\] just across the surface (nudged off it — see
    /// [`Geometry::cross_surface`]).
    pub r: Position,
    /// Outgoing unit direction — reflected for a reflective surface, unchanged
    /// for a transmissive one.
    pub u: Direction,
    /// `false` if the crossing killed the particle (vacuum boundary = leak).
    pub alive: bool,
    /// The surface just crossed and **which side of it the particle is now on**.
    /// [`SurfaceToken::NONE`] for a vacuum crossing (the history is over).
    pub on_surface: SurfaceToken,
}

/// Which side of `surf` a particle leaving along `u_out` from the crossing point
/// `r` ends up on.
///
/// Purely geometric, and independent of any cell's region definition: the
/// outward normal points in the direction `evaluate` increases, so a particle
/// travelling with `u_out · n > 0` is heading into the positive (outside)
/// half-space. This is how OpenMC signs its surface token when the region is not
/// a simple intersection (`Region::distance_complex`, `src/cell.cpp:1013`), and
/// it is used here for every crossing because it needs no assumption about how
/// the region was written.
#[inline]
fn outgoing_side(surf: &SurfaceKind, r: Position, u_out: Direction, i_surf: usize) -> SurfaceToken {
    let n = surf.normal(r);
    let dot = u_out.u * n.u + u_out.v * n.v + u_out.w * n.w;
    let sense = if dot > 0.0 {
        HalfSpaceSense::Outside
    } else {
        HalfSpaceSense::Inside
    };
    SurfaceToken::on(i_surf, sense)
}

/// Geometry methods that stay in `outram-mc-libs` (GitHub #486): crossing a
/// surface (boundary conditions, nudging, corner and diffuse reflection),
/// leaving a tracking region, the cross-section lookup and the pre-run
/// boundary-condition check.
pub trait GeometryExt {
    /// **Distance to leave the region at coordinate level `level`** — the
    /// extent of a delta-tracked region, which [`Geometry::distance_to_boundary`]
    /// cannot give.
    ///
    /// NEW WORK, no OpenMC counterpart (`bn:op-867c.4`).
    ///
    /// # Why the ordinary query is the wrong one
    ///
    /// `distance_to_boundary` takes the minimum over **every** level, so inside
    /// a pebble bed it returns the next TRISO or pebble surface — metres inside
    /// the region, and of no interest to a delta tracker, which is precisely
    /// the machinery that exists to *not* stop at those surfaces. What a delta
    /// flight must stop at is the boundary of the region that chose delta
    /// tracking, i.e. the cell at [`GeometryPath::tracking_level`].
    ///
    /// # What it returns
    ///
    /// The distance along the ray at which the particle leaves that cell,
    /// counting both its bounding surfaces and — if that level is a lattice
    /// tile — the tile edge. `f64::INFINITY` if it does not leave.
    ///
    /// Levels **deeper** than `level` are deliberately ignored: they are inside
    /// the region. Levels **shallower** are also ignored, because leaving an
    /// ancestor implies leaving this cell, and that crossing will be found on
    /// the next `locate` anyway. Including them would truncate delta flights at
    /// boundaries the tracker should stream straight through.
    ///
    /// Returns `INFINITY` for an out-of-range `level` rather than panicking: a
    /// too-*large* exit distance is caught by the enclosing tracker on the next
    /// locate, whereas a panic would take down a run over a stale index.
    fn distance_out_of_level(&self, path: &GeometryPath, level: usize) -> f64;
    /// Total macroscopic cross section of the cell a point is in — a convenience
    /// for delta-tracking majorant lookups. `None` if the point is lost (outside
    /// the geometry) or in a void cell.
    ///
    /// `materials`/`nuclides` are the global arrays the leaf material indexes into.
    fn sigma_t_at(
        &self,
        r: Position,
        u: Direction,
        e: f64,
        materials: &[Material],
        nuclides: &[Nuclide],
    ) -> Option<f64>;
    /// Reject boundary conditions this crate cannot honestly transport, **before**
    /// a run starts.
    ///
    /// # Why this exists (GitHub #259)
    ///
    /// Until 2026-09-22 `White` and `Periodic` both fell through to the
    /// specular-reflection arm of [`Self::cross_surface`] under a comment
    /// reading *"approximated as reflective (documented gap)"*. That is not an
    /// approximation in the useful sense: a periodic lattice without mirror
    /// symmetry -- a rotated hex assembly, a checkerboard, an off-centre rod
    /// bank -- has a genuinely different answer under the two conditions, and
    /// the run completed and returned a plausible `k` either way. It is the same
    /// failure shape as GitHub #187, where a specularly reflective sphere
    /// conserved impact parameter and produced a flat +39-50 % offset that read
    /// as a physics result.
    ///
    /// `White` is now implemented ([`SurfaceKind::diffuse_reflect`]), so it
    /// passes. `Periodic` is not, so it is refused here rather than aliased.
    ///
    /// # What it does not check
    ///
    /// Only the boundary conditions. It is not a geometry validator: it says
    /// nothing about whether surfaces close a region, whether cells overlap, or
    /// whether a lattice is filled.
    ///
    /// # Errors
    ///
    /// Returns the index and a description of the first surface carrying an
    /// unimplemented boundary condition.
    fn validate_boundary_conditions(&self) -> Result<(), String>;
    /// Apply a surface crossing to a global position/direction and return the
    /// post-crossing state.
    ///
    /// The particle is assumed already streamed to the surface at global `r`.
    /// A **reflective** surface reflects `u` about its outward normal; a
    /// **vacuum** surface kills the particle (leak); a transmissive crossing
    /// passes through unchanged. The returned position is nudged a hair
    /// **across the surface, along its normal**, and the returned
    /// [`SurfaceCrossing::on_surface`] records which side the particle ended up
    /// on, so the next [`Geometry::locate`] is unambiguous.
    ///
    /// # Why the outgoing side must be recorded, not re-derived
    ///
    /// After the crossing the particle sits (to within round-off) *on* the
    /// surface, where the sign of `Surface::evaluate` is decided by rounding
    /// rather than by geometry — worst at **grazing incidence on a curved
    /// surface**, where the tangential step dominates. A membership test that
    /// re-evaluates that sign can put the particle back in the cell it was
    /// leaving; the next `distance_to_boundary` then finds no forward surface
    /// (the one it sits on is suppressed as coincident), so the particle streams
    /// to infinity and the history leaks. On a concentric-shell pebble that lost
    /// 85 % of source neutrons (GitHub #168). The outgoing side is known exactly
    /// here — it is the sign of `u_out · n` — so it is recorded and carried,
    /// exactly as OpenMC carries its signed surface token
    /// (`src/particle.cpp:344`, and `surface() = -surface()` on reflection at
    /// `:795`).
    ///
    /// The nudge is a second, independent guard belonging to *this* crate's
    /// tracker (OpenMC does not nudge): it is taken along the surface **normal**
    /// so `evaluate` changes sign no matter how tangent `u_out` is, plus a step
    /// along `u_out` so a grazing particle makes tangential progress and does not
    /// re-hit the same point. See `nudge_across`.
    ///
    /// Mirrors the boundary-condition dispatch in `Particle::cross_surface`
    /// (`src/particle.cpp:659`), reduced to the vacuum/reflective/transmissive
    /// cases this crate implements.
    fn cross_surface(
        &self,
        i_surf: usize,
        r: Position,
        u: Direction,
        seed: &mut u64,
    ) -> SurfaceCrossing;
    /// Apply a boundary condition **in the coordinate frame the surface actually
    /// lives in** — the only correct way to cross a surface inside a lattice or
    /// a filled universe.
    ///
    /// # Why [`Self::cross_surface`] alone is not enough
    ///
    /// `cross_surface` evaluates `surf.normal(r)` at the position it is handed.
    /// A surface declared inside a nested universe is defined about **that
    /// universe's origin**, so handing it a *global* position computes the
    /// normal about the wrong centre. For a plane the normal is constant and
    /// nothing goes wrong, which is why root-universe geometries never showed
    /// it; for a sphere the normal is radial and the error is total.
    ///
    /// The consequence is not a crash. `nudge_across` pushes the particle
    /// `1e-9` along that wrong normal, so it can land back on the side it came
    /// from, and the following `locate` then reports the cell it was leaving.
    /// The whole next flight segment is attributed to the wrong material.
    ///
    /// **Measured on the 3x3x3 TRISO lattice (2026-09-14): 37.4 % of kernel
    /// sphere crossings mis-assigned the next segment; 0.0 % of the
    /// root-universe box-plane crossings.** That perfect separation by frame is
    /// what identified this. It is invisible without a material contrast — the
    /// wrong cell holds the same material — and invisible to a static point
    /// check, because that never crosses anything.
    ///
    /// # How the conversion is exact
    ///
    /// Nested frames in this crate are **pure translations** (no rotation), as
    /// [`Geometry::distance_to_boundary`] already relies on. So the global frame
    /// and level `coord_level` differ by a constant offset and a direction
    /// needs no transformation at all.
    ///
    /// That offset is read from [`Coord::offset`], which `locate` accumulates
    /// on the way down. It is **not** recovered as
    /// `levels[0].r - levels[coord_level].r`: that subtraction is catastrophic
    /// cancellation, and the ~1e-16 error it leaves is amplified to `1e-9` by
    /// the `dot == 0.0` branch in `nudge_across` — see [`Coord::offset`] and
    /// `tests/cell_translation.rs`.
    ///
    /// `r_global` is the crossing point in global coordinates — i.e. after
    /// streaming to the boundary, not the position the path was located at.
    fn cross_surface_in_frame(
        &self,
        i_surf: usize,
        path: &GeometryPath,
        coord_level: usize,
        r_global: Position,
        u: Direction,
        seed: &mut u64,
    ) -> SurfaceCrossing;
}

impl GeometryExt for Geometry {
    fn distance_out_of_level(&self, path: &GeometryPath, level: usize) -> f64 {
        let Some(coord) = path.levels.get(level) else {
            return f64::INFINITY;
        };
        let cell = &self.cells[coord.cell];
        let (d_surf, _) =
            cell.distance_to_boundary(coord.r, coord.u, &self.surfaces, path.on_surface);
        let mut best = d_surf;
        if let Some(l_idx) = coord.lattice {
            let d_tile = self.lattices[l_idx]
                .distance(coord.r, coord.u, coord.lattice_index)
                .0;
            if d_tile < best {
                best = d_tile;
            }
        }
        best
    }

    fn sigma_t_at(
        &self,
        r: Position,
        u: Direction,
        e: f64,
        materials: &[Material],
        nuclides: &[Nuclide],
    ) -> Option<f64> {
        let path = self.locate(r, u, SurfaceToken::NONE)?;
        let m = path.material?;
        Some(materials[m].macro_xs_total(e, nuclides))
    }

    fn validate_boundary_conditions(&self) -> Result<(), String> {
        for (i, surf) in self.surfaces.iter().enumerate() {
            if surf.bc() == BoundaryType::Periodic {
                return Err(format!(
                    "surface {i} declares a Periodic boundary condition, which is not \
                     implemented (GitHub #259). Periodic needs a partner surface and the \
                     translation or rotation between the pair; `BoundaryType` carries no \
                     partner. Reflective is NOT a substitute -- the two differ on any \
                     lattice without mirror symmetry, which is the case periodic exists \
                     for. Use Reflective only if that is genuinely the problem you mean."
                ));
            }
        }
        Ok(())
    }

    fn cross_surface(
        &self,
        i_surf: usize,
        r: Position,
        u: Direction,
        seed: &mut u64,
    ) -> SurfaceCrossing {
        let surf = &self.surfaces[i_surf];
        match surf.bc() {
            BoundaryType::Vacuum => SurfaceCrossing {
                r,
                u,
                alive: false,
                on_surface: SurfaceToken::NONE,
            },
            // ~~"White/Periodic are approximated as reflective (documented
            // gap)."~~ **CORRECTED 2026-09-22 (GitHub #259).** White is now its
            // own condition and Periodic is refused rather than aliased. Both
            // used to fall through to the specular arm and return a plausible
            // `k` while answering a different problem.
            BoundaryType::White => {
                // Upstream applies `diffuse_reflect` to the struck surface and
                // nothing else (`WhiteBC::handle_particle`,
                // `src/boundary_condition.cpp:56`). It deliberately does NOT go
                // through `compose_corner_reflection`: that is this crate's own
                // fix for SPECULAR corners, where composing the mirror off each
                // coincident wall is what stops a corner leaking. A cosine
                // re-emission has no such composition -- the outgoing direction
                // is already distributed about one normal, and re-diffusing it
                // off a second wall would sample the wrong distribution rather
                // than fix anything.
                let u_new = surf.diffuse_reflect(r, u, seed);
                let p = nudge_across(surf, r, u, u_new, false);
                SurfaceCrossing {
                    r: p,
                    u: u_new,
                    alive: true,
                    on_surface: outgoing_side(surf, r, u_new, i_surf),
                }
            }
            BoundaryType::Periodic => {
                // A loud failure, on purpose. Periodic needs a PARTNER surface
                // and a translation or rotation between the two
                // (`TranslationalPeriodicBC` / `RotationalPeriodicBC`,
                // `include/openmc/boundary_condition.h:124`, `:141`), and
                // `BoundaryType` carries no partner today. Until it does there
                // is no honest behaviour available here: reflecting is a
                // different problem and so is vacuum.
                //
                // [`Geometry::validate_boundary_conditions`] rejects these at
                // construction so a run cannot reach this point. This arm is the
                // backstop for a `Geometry` assembled by hand from its public
                // fields, which the struct's layout allows.
                panic!(
                    "surface {i_surf} declares a Periodic boundary condition, which is not \
                     implemented (GitHub #259). Until 2026-09-22 it was silently aliased to \
                     Reflective, which answers a DIFFERENT problem on any lattice without \
                     mirror symmetry. Call Geometry::validate_boundary_conditions() at \
                     construction to catch this before transport starts."
                );
            }
            BoundaryType::Reflective => {
                // Compose the reflection off EVERY reflective surface coincident
                // with `r` (corner/edge handling — see
                // [`Geometry::compose_corner_reflection`]); for a lone wall this
                // reduces exactly to `surf.reflect(r, u)`.
                let u_new = compose_corner_reflection(self, i_surf, r, u);
                // The particle bounces back to the side it came from.
                let p = nudge_across(surf, r, u, u_new, false);
                SurfaceCrossing {
                    r: p,
                    u: u_new,
                    alive: true,
                    on_surface: outgoing_side(surf, r, u_new, i_surf),
                }
            }
            BoundaryType::Transmissive => {
                // The particle passes through to the far side.
                let p = nudge_across(surf, r, u, u, true);
                SurfaceCrossing {
                    r: p,
                    u,
                    alive: true,
                    on_surface: outgoing_side(surf, r, u, i_surf),
                }
            }
        }
    }

    fn cross_surface_in_frame(
        &self,
        i_surf: usize,
        path: &GeometryPath,
        coord_level: usize,
        r_global: Position,
        u: Direction,
        seed: &mut u64,
    ) -> SurfaceCrossing {
        // Root level, or a malformed level index: global IS the local frame.
        if coord_level == 0 || coord_level >= path.levels.len() {
            return self.cross_surface(i_surf, r_global, u, seed);
        }
        // The exact offset carried down by `locate`, NOT `levels[0].r -
        // levels[coord_level].r`. The subtraction form is catastrophic
        // cancellation and its ~1e-16 error is amplified to 1e-9 by the
        // `dot == 0.0` branch in `nudge_across` — see [`Coord::offset`].
        let offset = path.levels[coord_level].offset;
        let crossed = self.cross_surface(i_surf, r_global - offset, u, seed);
        SurfaceCrossing {
            r: crossed.r + offset,
            ..crossed
        }
    }
}

/// Compose the specular reflections of every reflective-type surface
/// coincident with the crossing point `r`, returning the outgoing direction.
///
/// # Physical principle
///
/// Specular reflections **compose at a corner/edge**. When a particle streams
/// into a point that lies on the primary reflective surface `i_surf` *and*
/// (within `CORNER_TOL`) on one or more other Reflective / White / Periodic
/// surfaces, reflecting off only `i_surf` leaves the particle still headed
/// into the neighbouring wall(s). Reflecting off **all** coincident reflective
/// surfaces in a single event sends it back out of the corner immediately; at
/// a right-angle corner this is the familiar retroreflection (every involved
/// direction component negated at once).
///
/// # Why this crate needs it (robustness fix, not a port)
///
/// This crate's surface tracker advances to a boundary and then nudges a fixed
/// [`NUDGE`](Self::cross_surface) (`1e-9 cm`) along the outgoing direction.
/// Near a corner a grazing particle otherwise "ping-pongs": it alternately
/// re-crosses the perpendicular walls at sub-nudge distances, advancing only
/// ~`NUDGE` *along* the wall per event, so clearing an O(1 cm) feature can take
/// ~10^5–10^9 events — past `MAX_EVENTS`, where the history is capped and
/// leaked (a small negative bias), or previously hung. Composing the corner
/// reflection makes the particle leave in one event and removes the bias. This
/// is a robustness fix for *this crate's fixed-nudge tracker*: OpenMC advances
/// exactly to each surface token and never accumulates a sub-nudge residual, so
/// it has no directly corresponding routine — only the physics (reflections
/// compose at a corner) is mirrored.
///
/// # Selection and composition
///
/// A surface `j != i_surf` is included only when the *incoming* direction
/// actually **crosses** it (`|u · n_j| > 0`, judged on the incoming `u`) — the
/// same condition that made `i_surf` a crossing in the first place. A wall the
/// particle grazes exactly parallel to (`u · n_j = 0`) is skipped (reflecting
/// off it would be a no-op anyway). The sign of `u · n_j` is deliberately *not*
/// used: an axis-aligned plane's [`SurfaceKind::normal`] always points along the
/// +axis regardless of which side bounds the cell, so a min-side wall (cell on
/// the +sense side, exited with `u · n_j < 0`) must be treated identically to a
/// max-side wall — testing only the sign would leave min-side corners leaking.
/// A particle exiting a convex corner from inside the cell crosses every
/// coincident bounding wall, so composing all their reflections is exactly the
/// retroreflection that sends it back inside. The common lone-wall case has no
/// other coincident surface, so this reduces to exactly `surf.reflect(r, u)`
/// (bit-identical prior behaviour).
///
/// Reflections are composed by applying each surface's [`SurfaceKind::reflect`]
/// in turn. For mutually orthogonal walls (axis-aligned cube / lattice-cell
/// corners — the realistic case) the order is immaterial and the result is the
/// exact retroreflection of the crossed components. For a corner of
/// non-orthogonal reflective surfaces the composition is order-dependent and
/// only approximate; such geometries do not arise in the current verification
/// set.
///
/// `r` is the crossing point; coincidence is judged by
/// `|SurfaceKind::evaluate(r)| <= CORNER_TOL`.
fn compose_corner_reflection(
    geom: &Geometry,
    i_surf: usize,
    r: Position,
    u: Direction,
) -> Direction {
    /// Coincidence tolerance \[cm\] for a shared corner/edge — the transport
    /// nudge scale, so a sub-nudge ping-pong pair is always caught while any
    /// physically distinct wall (>> 1e-9 cm away) is not.
    const CORNER_TOL: f64 = 1.0e-9;

    // Always reflect off the primary (crossed) surface.
    let mut u_new = geom.surfaces[i_surf].reflect(r, u);

    for (j, surf) in geom.surfaces.iter().enumerate() {
        if j == i_surf {
            continue;
        }
        // **Reflective only, corrected 2026-09-22 (GitHub #259).** This
        // composition is the specular corner fix; a White wall does not
        // compose (see the `White` arm of [`Self::cross_surface`]) and a
        // Periodic one is refused before transport starts. Including them
        // here made a corner between a reflective wall and a white one
        // reflect specularly off both.
        if !matches!(surf.bc(), BoundaryType::Reflective) {
            continue;
        }
        // Coincident with the crossing point?
        if surf.evaluate(r).abs() > CORNER_TOL {
            continue;
        }
        // Reflect off any coincident wall the particle actually crosses (has a
        // non-zero velocity component through). Sign-agnostic: min-side and
        // max-side walls share the same +axis geometric normal, so both must be
        // handled. A grazing wall (`u·n = 0`) is a no-op and is skipped.
        let n = surf.normal(r);
        let dot = u.u * n.u + u.v * n.v + u.w * n.w;
        if dot != 0.0 {
            u_new = surf.reflect(r, u_new);
        }
    }
    u_new
}

/// Move a particle sitting on surface `surf` (at `r`) decisively onto the
/// correct side of it, then along its outgoing direction.
///
/// - `u_in` is the **incoming** direction (used only for its sign relative to
///   the surface normal — which way the particle was crossing).
/// - `u_out` is the direction the particle leaves with (`= u_in` for a
///   transmissive crossing, the reflected direction for a reflective one).
/// - `through`: `true` for a transmissive crossing (end up on the *far* side),
///   `false` for a reflective one (bounce back to the *incoming* side).
///
/// The normal-direction step is what makes this robust at grazing incidence on
/// a curved surface — see [`Geometry::cross_surface`]. `Surface::evaluate`
/// increases along `+normal`, so stepping `±normal·NUDGE` flips its sign in the
/// intended direction no matter how tangent `u_out` is; the extra `u_out` step
/// keeps a grazing particle from re-hitting the same point.
fn nudge_across(
    surf: &SurfaceKind,
    r: Position,
    u_in: Direction,
    u_out: Direction,
    through: bool,
) -> Position {
    const NUDGE: f64 = 1.0e-9;
    let n = surf.normal(r);
    let dot = u_in.u * n.u + u_in.v * n.v + u_in.w * n.w;
    // Preserve prior behaviour for the (non-physical) exactly-tangent case.
    if dot == 0.0 {
        return stream(r, u_out, NUDGE);
    }
    // `+normal` raises `evaluate`. Transmissive: end up where evaluate has the
    // sign of the crossing direction (`dot`). Reflective: the opposite sign.
    let side = if through { dot.signum() } else { -dot.signum() };
    let across = Direction::new(n.u * side, n.v * side, n.w * side);
    let p = stream(r, u_out, NUDGE);
    stream(p, across, NUDGE)
}

#[cfg(test)]
mod tests;
