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

use outram_blender::csg::position::{Direction, Position};
use outram_blender::csg::surface::SurfaceKind;

/// Surface methods that stay in `outram-mc-libs` (GitHub #486): the white
/// boundary's cosine-law re-emission, which draws from this crate's RNG and
/// scattering kinematics, and the two virtual-lattice helpers.
pub trait SurfaceKindExt {
    /// See the implementation on [`SurfaceKind`] below.
    fn diffuse_reflect(&self, r: Position, u: Direction, seed: &mut u64) -> Direction;
    /// See the implementation on [`SurfaceKind`] below.
    fn sphere_centre_radius(&self) -> Option<([f64; 3], f64)>;
    /// See the implementation on [`SurfaceKind`] below.
    fn overlaps_voxel(&self, centre: [f64; 3], pitch: [f64; 3]) -> bool;
}

impl SurfaceKindExt for SurfaceKind {
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
    #[inline]
    fn sphere_centre_radius(&self) -> Option<([f64; 3], f64)> {
        match self {
            Self::Sphere(s) => Some(([s.x0, s.y0, s.z0], s.r)),
            _ => None,
        }
    }

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
