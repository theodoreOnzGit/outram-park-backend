// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// PORTED FROM UPSTREAM — provenance (see the crate NOTICE):
//   Upstream project : LIGGGHTS-PUBLIC (DCS Computing GmbH / JKU Linz)
//   Upstream files   : src/superquadric.cpp / superquadric.h (`set`,
//                      `shape_function_*`, `plane_intersection`,
//                      `line_intersection`, `edge_intersection`,
//                      `surface_line_intersection1`,
//                      `calc_curvature_coefficient`,
//                      `surface_line_intersection`,
//                      `ellipsoid_line_intersection_local`, `set_shape`,
//                      `set_blockiness`, `calc_koef`, `tensor_quat_rotate`),
//                      src/math_extra_liggghts_superquadric.cpp
//                      (`check_inequalities`, `obb_intersect`,
//                      `bounding_sphere_radius_superquadric`,
//                      `inertia_superquadric`, `volume_superquadric`, `calc_F`,
//                      `invf`, `inverseMatrix4x4`, `determinant_4x4`,
//                      `calc_contact_point`,
//                      `calc_contact_point_if_no_previous_point_avaialable`,
//                      `calc_contact_point_using_prev_step`,
//                      `extended_overlap_algorithm`),
//                      src/math_extra_liggghts_nonspherical.h / .cpp
//                      (`pow_abs`, `pow_abs_int`, `sign`, `GMRES`, `matvecN`,
//                      `quat_to_mat`, `matvec`, `transpose_matvec`, `times3`,
//                      `transpose_times3`, `times3_transpose`, `isInteger`,
//                      `surfacesIntersectNonSpherical`, `point_wall_projection`,
//                      `omega_to_angmom`, `calc_conjqm`, `no_squish_rotate`,
//                      `clamp`), src/surface_model_superquadric.h
//                      (`checkSurfaceIntersect`, `surfacesIntersect`),
//                      src/fix_nve_asphere_base.cpp (`initial_integrate`,
//                      `final_integrate`, integration scheme 1),
//                      src/fix_wall_gran.cpp (`post_force_primitive`,
//                      `post_force_mesh`, the SUPERQUADRIC_ACTIVE_FLAG
//                      branches), src/tri_mesh_I_superquadric.h
//                      (`resolveTriSuperquadricContact`,
//                      `superquadricTriangleIntersection`), src/tri_mesh_I.h
//                      (`isInElement`), src/primitive_wall_definitions.h
//                      (`Plane`, `Cylinder` `resolveContact`),
//                      src/math_extra_liggghts_nonspherical.cpp
//                      (`get_effective_radius`, `get_effective_radius_wall`),
//                      src/neighbor.cpp (`decide`), src/neigh_gran.cpp
//                      (history carried over a rebuild), src/fix_wall_gran_base.h
//                      (`compute_force`), src/pair_gran_base.h (`compute_force`,
//                      the SUPERQUADRIC_ACTIVE_FLAG branches),
//                      src/normal_model_hertz.h and
//                      src/tangential_model_history.h (NONSPHERICAL_ACTIVE_FLAG
//                      torque branches), src/set.cpp (`shape`, `blockiness`,
//                      `density`, `quat_direct`, `omega` keywords for atom style
//                      superquadric), src/math_extra.h (`quatvec`, `invquatvec`),
//                      src/fix_template_superquadric.cpp (`randomize_ptilist`,
//                      expectancies, `max_rad`), src/particleToInsertSuperquadric.cpp
//                      (`insert`, `check_near_set_x_v_omega`, `set_x_v_omega`),
//                      src/region_neighbor_list_I.h (`hasOverlap_superquadric`,
//                      `insert_superquadric`), src/fix_insert.cpp (`load_xnear`,
//                      `check_obb`), src/fix_insert_pack.cpp (`x_v_omega`,
//                      `orientation random`), src/math_extra_liggghts.h
//                      (`random_unit_quat`), src/math_extra_liggghts_superquadric.cpp
//                      (`capsules_intersect`), src/math_extra_liggghts_nonspherical.cpp
//                      (`line_segments_distance`), src/rolling_model_cdt.h
//                      (`surfacesIntersect`, SUPERQUADRIC_ACTIVE_FLAG `reff`)
//   Upstream commit  : 3d5c00f20519e6bb6eb6756f51f1ad36564e649d (2024-06-07)
//   Upstream licence : "GNU Public License, version 2 or later" -> used here
//                      under the "or later" option as GPL-3.0. The superquadric
//                      files are Copyright 2015- JKU Linz / DCS Computing GmbH
//                      (Alexander Podlozhnyuk); `math_extra` is a LAMMPS
//                      original, Copyright (2003) Sandia Corporation (GPL).
// Do not strip this block during refactors (workspace CLAUDE.md).
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

//! # Superquadric particles: `atom_style superquadric`, `fix nve/superquadric`
//!
//! gh:#1415 (child of #1405). A superquadric is the body
//! `(|x/a|^n2 + |y/b|^n2)^(n1/n2) + |z/c|^n1 <= 1` in its own frame, with
//! half-axes `a, b, c` (`shape`) and blockiness `n1, n2 >= 2` (`n1 = n2 = 2` is
//! an ellipsoid; large `n` approaches a box). Upstream compiles it only with
//! `-DSUPERQUADRIC_ACTIVE_FLAG -DNONSPHERICAL_ACTIVE_FLAG`; the default
//! `make serial` build leaves it out (`upstream_source/README.md`).
//!
//! | upstream | here |
//! |---|---|
//! | `create_atoms single` + `set shape/blockiness/density/quat_direct/omega` | [`SuperquadricParticle::new`] |
//! | `bounding_sphere_radius_superquadric`, `volume_superquadric`, `inertia_superquadric` | private functions, run by [`SuperquadricParticle::new`] |
//! | `fix nve/superquadric` (integration scheme 1, the default) | [`SuperquadricSystem::run`] (~~`SuperquadricSystem::step`~~, no such method; corrected 2026-11-13) |
//! | `pair gran model hertz tangential history surface superquadric` | [`SuperquadricSystem`] pair pass |
//! | `fix wall/gran model hertz tangential history surface superquadric primitive xplane/yplane/zplane` | [`SuperquadricPlaneWall`] |
//! | `... primitive xcylinder/ycylinder/zcylinder r c1 c2` | [`SuperquadricCylinderWall`], [`SuperquadricSystem::with_cylinder_wall`] |
//! | `fix mesh/surface file F` + `fix wall/gran ... surface superquadric mesh` (static mesh) | [`SuperquadricSystem::with_mesh_wall`] |
//! | `surface superquadric curvatureLimitFactor f meanCurvature/gaussianCurvature yes` | [`SuperquadricSystem::with_curvature_limit`] |
//! | `neighbor <skin> bin` + `neigh_modify delay 0 every N check yes/no` | [`SuperquadricSystem::with_neighbor_list`] |
//! | `fix gravity ... vector` | [`SuperquadricSystem::with_gravity`] |
//! | `fix particletemplate/superquadric` (`constant` shape, blockiness, density) | [`ParticleTemplateSuperquadric`] |
//! | `rolling_friction cdt` (pair and walls, `torsionTorque off`) | [`SuperquadricSystem::with_rolling_cdt`] |
//! | `fix insert/pack` (`check_obb yes`, `orientation constant/random`) | [`crate::insert::InsertFix`] + [`SuperquadricSystem::with_insertion`] |
//!
//! [`SuperquadricSystem`] is a separate driver beside
//! [`crate::granular_system::GranularSystem`]: none of the sphere or
//! multisphere types gains a variant, so their paths are unchanged. It reuses
//! the Hertz law ([`crate::granular::GranularNormalModel::evaluate_with`]), the
//! type-pair coefficients ([`crate::granular::MaterialTable`]) and the
//! quaternion helpers of [`crate::multisphere`].
//!
//! ## Contact detection
//!
//! * **Wall** (closed form): the bounding sphere touches the plane, then
//!   `Superquadric::plane_intersection` gives the point of maximal penetration
//!   and the point of lowest shape-function value on the wall; contact if that
//!   value is negative. The overlap is the distance of the penetration point
//!   from the plane (`point_wall_projection`). A **cylinder** is handled the
//!   same way with the tangent plane at the bounding sphere's contact point
//!   (`Cylinder<dim>::resolveContact`): upstream has no cylinder-specific
//!   superquadric code.
//! * **Mesh facet** (`resolveTriSuperquadricContact`): bounding sphere against
//!   the facet, `plane_intersection` with the facet plane, then the face
//!   overlap if the point of lowest potential is on the facet (or the overlap
//!   is under `0.001 a`), otherwise the edge or corner with the lowest
//!   shape-function value (golden section, inactive edges and corners
//!   skipped), with the overlap along the gradient normal
//!   (`surface_line_intersection1`), whichever of the two is smaller.
//! * **Pair** (iterative): bounding spheres, then an oriented-bounding-box
//!   separating-axis test (15 inequalities, starting at the one that separated
//!   last time), then the contact point: the point where `f_i = f_j` and the
//!   gradients are anti-parallel, by Newton's method on a 4x4 system (explicit
//!   inverse when `|det J| > 1`, else `GMRES<4,4>`), with a golden-section line
//!   search when a step does not reduce the residual. With no previous point
//!   it walks the shapes from equal-volume spheres to the real ones in
//!   `max(10, n_max - 1)` steps. The overlap is the sum of the two
//!   line-surface distances along the normal (`extended_overlap_algorithm`).
//!   Same operation order as upstream, so the same iterates.
//!
//! ## Forces
//!
//! Hertz with the volume-equivalent radius `r = cbrt(3 V / 4 pi)` (pair:
//! `r_i r_j / (r_i + r_j)`; wall: the particle's own), or, with
//! [`SuperquadricSystem::with_curvature_limit`], `1 / (k_i + k_j)` (`1 / k_i`
//! at a wall) from the mean or Gaussian curvature at the surface points,
//! capped at `f` times that radius; tangential history, and
//! both normal and tangential forces applied at the contact point, so the
//! normal force also makes a torque `(x_c - x) x F` (upstream's
//! `NONSPHERICAL_ACTIVE_FLAG` branches).
//!
//! ## Maths library
//!
//! `pow`, `cbrt`, `sin`, `cos`, `exp` and `lgamma` go through PETIR's
//! `RealMath` (the `libm` crate), not glibc as upstream does (maintainer
//! decision on gh:#1455). Upstream's `std::beta` is libstdc++'s
//! `exp(lgamma(x) + lgamma(y) - lgamma(x + y))`, reproduced here in that order.
//!
//! ## What is translated, and what is not
//!
//! Translated: the above, single process, fixed boundaries. **Neighbour list:**
//! by default every pair `i < j` in index order, its contact history dropped
//! when the bounding spheres part; that is upstream's `neigh_modify every 1
//! check no` for two particles. With [`SuperquadricSystem::with_neighbor_list`]
//! it is upstream's list for any number of particles: rebuilt by
//! `Neighbor::decide`, pairs in `granular_bin_no_newton`'s binned order, and a
//! history that persists between rebuilds and is kept at a rebuild only
//! where the bounding spheres overlap (`neigh_gran.cpp`).
//!
//! **Not translated** (refused or absent, never silently approximated):
//! ~~`fix particletemplate/superquadric` and insertion of superquadrics~~
//! (ported 2026-11-13 for `constant` distributions and `insert/pack`;
//! `gaussian`/`uniform` shapes, `size`, and `insert/stream` of superquadrics
//! are not),
//! ~~the curvature-limited radius~~ (ported 2026-11-13), rolling-friction
//! models other than CDT (~~rolling-friction models~~: CDT ported
//! 2026-11-13, `torsionTorque off` only), ~~mesh walls (`resolveTriSuperquadricContact`)~~ (static meshes
//! ported 2026-11-13; moving meshes and `mesh/surface/stress` are not),
//! the superquadric atom sort (`atom_modify sort` must be `0 0`),
//! `area_superquadric` (not used by the dynamics), `compute
//! erotate/superquadric`, integration schemes 0, 2, 3 and 4, and the
//! CFD-coupling torque terms. Convex hull, ellipsoid, line and tri particles
//! are not ported.
//!
//! V&V: `docs/verification-and-validation.md` §§ 4.17, 4.17.1, 4.17.2 and
//! 4.17.3.

use crate::granular::{ContactKinematics, GranularMaterial, GranularNormalModel, MaterialTable};
use crate::mesh_wall::MeshWall;
use crate::neighbor_list::{self, LiggghtsNeighbor};
use crate::tri_mesh::TriMeshState;
use crate::particle::Vec3;
use crate::DemError;
use petir::mathf::RealMath;
use uom::si::f64::{Length, MassDensity, Time};
use uom::si::{length::meter, mass_density::kilogram_per_cubic_meter, time::second};

/// `scale_koef` of `math_extra_liggghts_nonspherical.h`.
const SCALE_KOEF: f64 = 1.0001;
/// `PHI_INV` of `math_extra_liggghts_superquadric.cpp`.
const PHI_INV: f64 = 0.61803398874989479;
/// `M_PI`.
const M_PI: f64 = std::f64::consts::PI;
/// `SMALL_TRIMESH` of `tri_mesh.h`.
const SMALL_TRIMESH: f64 = 1e-10;

type V3 = [f64; 3];

// ---------------------------------------------------------------------------
// Maths library seam (PETIR's `RealMath`, gh:#1455).
// ---------------------------------------------------------------------------

fn m_pow(x: f64, y: f64) -> f64 {
    x.r_powf(y)
}

fn m_cbrt(x: f64) -> f64 {
    x.r_cbrt()
}

fn m_sin(x: f64) -> f64 {
    x.r_sin()
}

fn m_cos(x: f64) -> f64 {
    x.r_cos()
}

fn m_exp(x: f64) -> f64 {
    x.r_exp()
}

fn m_lgamma(x: f64) -> f64 {
    petir::lgamma(x)
}

/// libstdc++'s `std::beta` (`std::__detail::__beta_lgamma`):
/// `exp(lgamma(x) + lgamma(y) - lgamma(x + y))`, NaN in, NaN out.
fn beta_fn(x: f64, y: f64) -> f64 {
    if x.is_nan() || y.is_nan() {
        return f64::NAN;
    }
    let bet = m_lgamma(x) + m_lgamma(y) - m_lgamma(x + y);
    m_exp(bet)
}

// ---------------------------------------------------------------------------
// Small helpers, in upstream's operation order.
// ---------------------------------------------------------------------------

/// `std::max(a, b)`: `(a < b) ? b : a`.
fn cmax(a: f64, b: f64) -> f64 {
    if a < b {
        b
    } else {
        a
    }
}

/// `std::min(a, b)`: `(b < a) ? b : a`.
fn cmin(a: f64, b: f64) -> f64 {
    if b < a {
        b
    } else {
        a
    }
}

/// `MathExtraLiggghts::max(a, b, c)`.
fn max3(a: f64, b: f64, c: f64) -> f64 {
    cmax(cmax(a, b), c)
}

/// `MathExtraLiggghts::min(a, b, c)`.
fn min3(a: f64, b: f64, c: f64) -> f64 {
    cmin(cmin(a, b), c)
}

/// `MathExtraLiggghts::compDouble(a, b, prec)`.
fn comp_double(a: f64, b: f64, prec: f64) -> bool {
    if a == b {
        return true;
    }
    if b == 0.0 {
        return a < prec && a > -prec;
    }
    let x = a - b;
    x < prec && x > -prec
}

/// `MathExtraLiggghtsNonspherical::isInteger`.
fn is_integer(x: f64) -> bool {
    let x_temp = (x + 1e-2).floor();
    comp_double(x, x_temp, 1e-2)
}

/// `MathExtraLiggghtsNonspherical::sign`.
fn sign(val: f64) -> f64 {
    if val > 1e-14 {
        1.0
    } else if val < -1e-14 {
        -1.0
    } else {
        0.0
    }
}

/// `MathExtraLiggghtsNonspherical::clamp`.
fn clamp(val: f64, min: f64, max: f64) -> f64 {
    if val < min {
        return min;
    }
    if val > max {
        return max;
    }
    val
}

/// `MathExtraLiggghtsNonspherical::pow_abs_int`.
fn pow_abs_int(a: f64, b: i32) -> f64 {
    let a = a.abs();
    match b {
        0 => 1.0,
        1 => a,
        2 => a * a,
        3 => a * a * a,
        4 => {
            let a2 = a * a;
            a2 * a2
        }
        5 => {
            let a2 = a * a;
            a2 * a2 * a
        }
        6 => {
            let a3 = a * a * a;
            a3 * a3
        }
        7 => {
            let a2 = a * a;
            let a3 = a2 * a;
            a2 * a2 * a3
        }
        8 => {
            let a2 = a * a;
            let a4 = a2 * a2;
            a4 * a4
        }
        9 => {
            let a3 = a * a * a;
            a3 * a3 * a3
        }
        _ => {
            let mut result = a;
            let mut i = 0;
            while i < b - 1 {
                result *= a;
                i += 1;
            }
            result
        }
    }
}

/// `MathExtraLiggghtsNonspherical::pow_abs`.
fn pow_abs(base: f64, exponent: f64) -> f64 {
    let base = base.abs();
    let tol = 1e-12;
    if exponent.abs() < tol || (base - 1.0).abs() < tol {
        1.0
    } else if base.abs() < tol {
        0.0
    } else {
        let n = exponent.abs().floor();
        let n_int = n as i32;
        let mut result = pow_abs_int(base, n_int);
        let n_delta = exponent.abs() - n;
        if n_delta > 0.0 {
            if (2.0 * n_delta - 1.0).abs() < 2.0 * tol {
                result *= base.sqrt();
            } else {
                result *= m_pow(base, n_delta);
            }
        }
        if exponent > 0.0 {
            result
        } else {
            1.0 / result
        }
    }
}

fn dot3(a: &V3, b: &V3) -> f64 {
    crate::multisphere::dot3(a, b)
}

fn cross3(a: &V3, b: &V3) -> V3 {
    crate::multisphere::cross3(a, b)
}

/// `vectorMag3D`.
fn mag3(v: &V3) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

/// `vectorSubtract3D(a, b, c)`: `c = a - b`.
fn sub3(a: &V3, b: &V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `vectorNormalize3D`.
fn normalize3(v: &mut V3) {
    let norm = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let invnorm = if norm == 0.0 { 0.0 } else { 1.0 / norm };
    v[0] *= invnorm;
    v[1] *= invnorm;
    v[2] *= invnorm;
}

/// `yabx3D(a, b, x, y)`: `y = a + b x`.
fn yabx3(a: &V3, b: f64, x: &[f64]) -> V3 {
    [a[0] + b * x[0], a[1] + b * x[1], a[2] + b * x[2]]
}

/// `MathExtraLiggghtsNonspherical::quat_to_mat` (row-major 3x3).
fn quat_to_mat(quat: &[f64; 4]) -> [f64; 9] {
    let w2 = quat[0] * quat[0];
    let i2 = quat[1] * quat[1];
    let j2 = quat[2] * quat[2];
    let k2 = quat[3] * quat[3];
    let twoij = 2.0 * quat[1] * quat[2];
    let twoik = 2.0 * quat[1] * quat[3];
    let twojk = 2.0 * quat[2] * quat[3];
    let twoiw = 2.0 * quat[1] * quat[0];
    let twojw = 2.0 * quat[2] * quat[0];
    let twokw = 2.0 * quat[3] * quat[0];
    [
        w2 + i2 - j2 - k2,
        twoij - twokw,
        twojw + twoik,
        twoij + twokw,
        w2 - i2 + j2 - k2,
        twojk - twoiw,
        twoik - twojw,
        twojk + twoiw,
        w2 - i2 - j2 + k2,
    ]
}

/// `MathExtraLiggghtsNonspherical::matvec`.
fn matvec(m: &[f64; 9], v: &V3) -> V3 {
    [
        m[0] * v[0] + m[1] * v[1] + m[2] * v[2],
        m[3] * v[0] + m[4] * v[1] + m[5] * v[2],
        m[6] * v[0] + m[7] * v[1] + m[8] * v[2],
    ]
}

/// `MathExtraLiggghtsNonspherical::transpose_matvec`.
fn transpose_matvec(m: &[f64; 9], v: &V3) -> V3 {
    [
        m[0] * v[0] + m[3] * v[1] + m[6] * v[2],
        m[1] * v[0] + m[4] * v[1] + m[7] * v[2],
        m[2] * v[0] + m[5] * v[1] + m[8] * v[2],
    ]
}

/// `MathExtraLiggghtsNonspherical::times3`: `m m2`.
fn times3(m: &[f64; 9], m2: &[f64; 9]) -> [f64; 9] {
    let mut ans = [0.0; 9];
    for r in 0..3 {
        for c in 0..3 {
            ans[r * 3 + c] = m[r * 3] * m2[c] + m[r * 3 + 1] * m2[3 + c] + m[r * 3 + 2] * m2[6 + c];
        }
    }
    ans
}

/// `MathExtraLiggghtsNonspherical::transpose_times3`: `m^T m2`.
fn transpose_times3(m: &[f64; 9], m2: &[f64; 9]) -> [f64; 9] {
    let mut ans = [0.0; 9];
    for r in 0..3 {
        for c in 0..3 {
            ans[r * 3 + c] = m[r] * m2[c] + m[3 + r] * m2[3 + c] + m[6 + r] * m2[6 + c];
        }
    }
    ans
}

/// `MathExtraLiggghtsNonspherical::times3_transpose`: `m m2^T`.
fn times3_transpose(m: &[f64; 9], m2: &[f64; 9]) -> [f64; 9] {
    let mut ans = [0.0; 9];
    for r in 0..3 {
        for c in 0..3 {
            ans[r * 3 + c] =
                m[r * 3] * m2[c * 3] + m[r * 3 + 1] * m2[c * 3 + 1] + m[r * 3 + 2] * m2[c * 3 + 2];
        }
    }
    ans
}

/// `MathExtraLiggghtsNonspherical::matvecN` (zero products skipped).
fn matvec_n<const N: usize>(a: &[f64], b: &[f64]) -> [f64; N] {
    let mut c = [0.0; N];
    for i in 0..N {
        c[i] = 0.0;
        for j in 0..N {
            if a[i * N + j] != 0.0 && b[j] != 0.0 {
                c[i] += a[i * N + j] * b[j];
            }
        }
    }
    c
}

/// `MathExtraLiggghtsNonspherical::normN`.
fn norm_n(a: &[f64]) -> f64 {
    let mut result = 0.0;
    for x in a {
        result += x * x;
    }
    result.sqrt()
}

/// `MathExtraLiggghtsNonspherical::GMRES<4,4>` (Saad-Schultz with Givens
/// rotations, one cycle of `m = 4`).
fn gmres4(a: &[f64; 16], b: &[f64; 4], x0: &[f64; 4], x: &mut [f64; 4]) {
    const N: usize = 4;
    const M_START: usize = 4;
    let mut m = M_START;
    let ax0 = matvec_n::<N>(a, x0);
    let mut res = [0.0; N];
    for i in 0..N {
        res[i] = b[i] - ax0[i];
    }
    *x = *x0;
    let mut beta_sq = 0.0;
    for r in &res {
        beta_sq += r * r;
    }
    if beta_sq < 1e-20 {
        return;
    }
    let beta = beta_sq.sqrt();
    let mut y = [0.0; M_START];
    let mut v = [0.0; (M_START + 1) * N];
    let mut h = [0.0; (M_START + 1) * M_START];
    let mut w = [0.0; M_START * N];

    let inv_beta = 1.0 / beta;
    for i in 0..N {
        v[i] = res[i] * inv_beta;
    }

    let mut j = 0;
    while j < m {
        let wj = matvec_n::<N>(&a[..], &v[j * N..j * N + N]);
        w[j * N..j * N + N].copy_from_slice(&wj);
        for i in 0..=j {
            for ix in 0..N {
                h[i * m + j] += w[j * N + ix] * v[i * N + ix];
            }
            for ix in 0..N {
                w[j * N + ix] -= h[i * m + j] * v[i * N + ix];
            }
        }
        h[(j + 1) * m + j] = norm_n(&w[j * N..j * N + N]);
        if h[(j + 1) * m + j] < 1e-12 {
            m = j + 1;
            break;
        } else {
            let k = 1.0 / h[(j + 1) * m + j];
            for ix in 0..N {
                v[(j + 1) * N + ix] = w[j * N + ix] * k;
            }
        }
        j += 1;
    }

    let mut hm = [0.0; (M_START + 1) * M_START];
    let mut g = [0.0; M_START + 1];
    g[0] = beta;
    for i in 0..m + 1 {
        for jj in 0..m {
            hm[i * M_START + jj] = h[i * M_START + jj];
        }
    }
    let mut row1 = [0.0; M_START];
    let mut row2 = [0.0; M_START];
    for jj in 0..m {
        let root_inv = 1.0
            / (hm[jj * M_START + jj] * hm[jj * M_START + jj]
                + hm[(jj + 1) * M_START + jj] * hm[(jj + 1) * M_START + jj])
                .sqrt();
        let s = hm[(jj + 1) * M_START + jj] * root_inv;
        let c = hm[jj * M_START + jj] * root_inv;
        for i in 0..m {
            row1[i] = c * hm[jj * M_START + i] + s * hm[(jj + 1) * M_START + i];
            row2[i] = -s * hm[jj * M_START + i] + c * hm[(jj + 1) * M_START + i];
        }
        for i in 0..m {
            hm[jj * M_START + i] = row1[i];
            hm[(jj + 1) * M_START + i] = row2[i];
        }
        let g1 = c * g[jj] + s * g[jj + 1];
        let g2 = -s * g[jj] + c * g[jj + 1];
        g[jj] = g1;
        g[jj + 1] = g2;
    }
    for jj in 0..m {
        let j1 = m - 1 - jj;
        let mut summ = 0.0;
        for i in 0..jj {
            let i1 = m - 1 - i;
            summ += hm[j1 * M_START + i1] * y[i1];
        }
        y[j1] = (g[j1] - summ) / hm[j1 * M_START + j1];
        for i in 0..N {
            x[i] += y[j1] * v[j1 * N + i];
        }
    }
}

/// `invf` of `math_extra_liggghts_superquadric.cpp` (one cofactor).
fn invf(i: i32, j: i32, m: &[f64; 16]) -> f64 {
    let o = 2 + (j - i);
    let i = i + 4 + o;
    let j = j + 4 - o;
    let e = |a: i32, b: i32| m[(((j + b) % 4) * 4 + ((i + a) % 4)) as usize];
    let inv = e(1, -1) * e(0, 0) * e(-1, 1)
        + e(1, 1) * e(0, -1) * e(-1, 0)
        + e(-1, -1) * e(1, 0) * e(0, 1)
        - e(-1, -1) * e(0, 0) * e(1, 1)
        - e(-1, 1) * e(0, -1) * e(1, 0)
        - e(1, -1) * e(-1, 0) * e(0, 1);
    if o % 2 != 0 {
        inv
    } else {
        -inv
    }
}

/// `inverseMatrix4x4`: returns the determinant (0 leaves `out` untouched).
fn inverse_matrix_4x4(m: &[f64; 16], out: &mut [f64; 16]) -> f64 {
    let mut inv = [0.0; 16];
    for i in 0..4 {
        for j in 0..4 {
            inv[(j * 4 + i) as usize] = invf(i, j, m);
        }
    }
    let mut d = 0.0;
    for k in 0..4 {
        d += m[k] * inv[k * 4];
    }
    if d == 0.0 {
        return d;
    }
    let d_inv = 1.0 / d;
    for i in 0..16 {
        out[i] = inv[i] * d_inv;
    }
    d
}

/// `determinant_4x4`.
fn determinant_4x4(mat: &[f64; 16]) -> f64 {
    let (m00, m01, m02, m03) = (mat[0], mat[1], mat[2], mat[3]);
    let (m10, m11, m12, m13) = (mat[4], mat[5], mat[6], mat[7]);
    let (m20, m21, m22, m23) = (mat[8], mat[9], mat[10], mat[11]);
    let (m30, m31, m32, m33) = (mat[12], mat[13], mat[14], mat[15]);
    m03 * m12 * m21 * m30 - m02 * m13 * m21 * m30 - m03 * m11 * m22 * m30
        + m01 * m13 * m22 * m30
        + m02 * m11 * m23 * m30
        - m01 * m12 * m23 * m30
        - m03 * m12 * m20 * m31
        + m02 * m13 * m20 * m31
        + m03 * m10 * m22 * m31
        - m00 * m13 * m22 * m31
        - m02 * m10 * m23 * m31
        + m00 * m12 * m23 * m31
        + m03 * m11 * m20 * m32
        - m01 * m13 * m20 * m32
        - m03 * m10 * m21 * m32
        + m00 * m13 * m21 * m32
        + m01 * m10 * m23 * m32
        - m00 * m11 * m23 * m32
        - m02 * m11 * m20 * m33
        + m01 * m12 * m20 * m33
        + m02 * m10 * m21 * m33
        - m00 * m12 * m21 * m33
        - m01 * m10 * m22 * m33
        + m00 * m11 * m22 * m33
}

/// `MathExtra::quatvec(a, b, c)`: `c = a * (0, b)`.
fn quatvec(a: &[f64; 4], b: &V3) -> [f64; 4] {
    [
        -a[1] * b[0] - a[2] * b[1] - a[3] * b[2],
        a[0] * b[0] + a[2] * b[2] - a[3] * b[1],
        a[0] * b[1] + a[3] * b[0] - a[1] * b[2],
        a[0] * b[2] + a[1] * b[1] - a[2] * b[0],
    ]
}

/// `MathExtra::invquatvec(a, b, c)`: `c = inv(a) * b`, vector part.
fn invquatvec(a: &[f64; 4], b: &[f64; 4]) -> V3 {
    [
        -a[1] * b[0] + a[0] * b[1] + a[3] * b[2] - a[2] * b[3],
        -a[2] * b[0] - a[3] * b[1] + a[0] * b[2] + a[1] * b[3],
        -a[3] * b[0] + a[2] * b[1] - a[1] * b[2] + a[0] * b[3],
    ]
}

/// `MathExtraLiggghtsNonspherical::calc_conjqm`.
fn calc_conjqm(quat: &[f64; 4], angmom_body: &V3) -> [f64; 4] {
    let mut c = quatvec(quat, angmom_body);
    c[0] *= 2.0;
    c[1] *= 2.0;
    c[2] *= 2.0;
    c[3] *= 2.0;
    c
}

/// `MathExtraLiggghtsNonspherical::no_squish_rotate`.
fn no_squish_rotate(k: usize, p: &mut [f64; 4], q: &mut [f64; 4], inertia: &V3, dt: f64) {
    let (kq, kp) = match k {
        1 => ([-q[1], q[0], q[3], -q[2]], [-p[1], p[0], p[3], -p[2]]),
        2 => ([-q[2], -q[3], q[0], q[1]], [-p[2], -p[3], p[0], p[1]]),
        _ => ([-q[3], q[2], -q[1], q[0]], [-p[3], p[2], -p[1], p[0]]),
    };
    let mut phi = p[0] * kq[0] + p[1] * kq[1] + p[2] * kq[2] + p[3] * kq[3];
    phi /= 4.0 * inertia[k - 1];
    let c_phi = m_cos(dt * phi);
    let s_phi = m_sin(dt * phi);
    for n in 0..4 {
        p[n] = c_phi * p[n] + s_phi * kp[n];
    }
    for n in 0..4 {
        q[n] = c_phi * q[n] + s_phi * kq[n];
    }
}

/// `MathExtraLiggghtsNonspherical::omega_to_angmom`.
fn omega_to_angmom(quat: &[f64; 4], omega: &V3, inertia: &V3) -> V3 {
    let a = quat_to_mat(quat);
    let omega_body = transpose_matvec(&a, omega);
    let angmom_body = [
        omega_body[0] * inertia[0],
        omega_body[1] * inertia[1],
        omega_body[2] * inertia[2],
    ];
    matvec(&a, &angmom_body)
}

/// `point_wall_projection`: distance of `input_point` from the plane through
/// `point_on_wall` with normal `normal`, and its projection onto the plane.
fn point_wall_projection(normal: &V3, point_on_wall: &V3, input: &V3) -> (f64, V3) {
    let (x0, y0, z0) = (point_on_wall[0], point_on_wall[1], point_on_wall[2]);
    let (x, y, z) = (input[0], input[1], input[2]);
    let (a, b, c) = (normal[0], normal[1], normal[2]);
    let alpha = -(a * (x - x0) + b * (y - y0) + c * (z - z0)) / (a * a + b * b + c * c);
    let out = [x + alpha * a, y + alpha * b, z + alpha * c];
    let d = ((a * (x - x0) + b * (y - y0) + c * (z - z0)) / (a * a + b * b + c * c).sqrt()).abs();
    (d, out)
}

// ---------------------------------------------------------------------------
// Shape properties (atom_vec_superquadric / set.cpp).
// ---------------------------------------------------------------------------

/// `bounding_sphere_radius_superquadric`.
fn bounding_sphere_radius(shape: &V3, blockiness: &[f64; 2]) -> f64 {
    let mut radius;
    if comp_double(blockiness[0], 2.0, 1e-13) && comp_double(blockiness[1], 2.0, 1e-13) {
        radius = cmax(cmax(shape[0], shape[1]), shape[2]);
    } else {
        let a = cmax(shape[0], shape[1]);
        let b = cmin(shape[0], shape[1]);
        let c = shape[2];
        if comp_double(blockiness[1], 2.0, 1e-13) {
            radius = (a * a + c * c).sqrt();
        } else {
            let n1 = cmax(2.1, blockiness[0]);
            let n2 = cmax(2.1, blockiness[1]);
            let alpha = if !comp_double(blockiness[1], 2.0, 1e-13) {
                pow_abs(b / a, 2.0 / (n2 - 2.0))
            } else {
                0.0
            };
            let gamma = pow_abs(1.0 + pow_abs(alpha, n2), n1 / n2 - 1.0);
            let beta = pow_abs(c * c / (a * a) * gamma, 1.0 / (n1 - 2.0));
            let x0 = 1.0
                / pow_abs(
                    pow_abs(1.0 + pow_abs(alpha, n2), n1 / n2) + pow_abs(beta, n1),
                    1.0 / n1,
                );
            let xtmp = x0 * a;
            let ytmp = alpha * x0 * b;
            let ztmp = beta * x0 * c;
            radius = (xtmp * xtmp + ytmp * ytmp + ztmp * ztmp).sqrt();
        }
    }
    radius *= SCALE_KOEF;
    radius
}

/// `volume_superquadric`.
fn volume_superquadric(shape: &V3, blockiness: &[f64; 2]) -> f64 {
    let eps1 = 2.0 / blockiness[0];
    let eps2 = 2.0 / blockiness[1];
    2.0 * shape[0]
        * shape[1]
        * shape[2]
        * eps1
        * eps2
        * beta_fn(0.5 * eps1, eps1 + 1.0)
        * beta_fn(0.5 * eps2, 0.5 * eps2 + 1.0)
}

/// `inertia_superquadric`: principal moments in the body frame.
fn inertia_superquadric(shape: &V3, blockiness: &[f64; 2], dens: f64) -> V3 {
    let eps1 = 2.0 / blockiness[0];
    let eps2 = 2.0 / blockiness[1];
    let (a1, a2, a3) = (shape[0], shape[1], shape[2]);
    let i_xx = 0.5
        * a1
        * a2
        * a3
        * eps1
        * eps2
        * (a2 * a2 * beta_fn(1.5 * eps2, 0.5 * eps2) * beta_fn(0.5 * eps1, 2.0 * eps1 + 1.0)
            + 4.0
                * a3
                * a3
                * beta_fn(0.5 * eps2, 0.5 * eps2 + 1.0)
                * beta_fn(1.5 * eps1, eps1 + 1.0))
        * dens;
    let i_yy = 0.5
        * a1
        * a2
        * a3
        * eps1
        * eps2
        * (a1 * a1 * beta_fn(1.5 * eps2, 0.5 * eps2) * beta_fn(0.5 * eps1, 2.0 * eps1 + 1.0)
            + 4.0
                * a3
                * a3
                * beta_fn(0.5 * eps2, 0.5 * eps2 + 1.0)
                * beta_fn(1.5 * eps1, eps1 + 1.0))
        * dens;
    let i_zz = 0.5
        * a1
        * a2
        * a3
        * eps1
        * eps2
        * (a1 * a1 + a2 * a2)
        * beta_fn(1.5 * eps2, 0.5 * eps2)
        * beta_fn(0.5 * eps1, 2.0 * eps1 + 1.0)
        * dens;
    [i_xx, i_yy, i_zz]
}

// ---------------------------------------------------------------------------
// struct Superquadric (superquadric.cpp).
// ---------------------------------------------------------------------------

/// Upstream's `struct Superquadric`, by value (upstream holds pointers into
/// the atom arrays; the routines that change `shape` or `blockiness` restore
/// them, so a copy is equivalent).
#[derive(Debug, Clone, Copy)]
struct Sq {
    center: V3,
    shape: V3,
    blockiness: [f64; 2],
    gradient: V3,
    hessian: [f64; 9],
    rotation_matrix: [f64; 9],
    koef: f64,
    shape_inv: V3,
    is_ellipsoid: bool,
    use_int_blockiness: bool,
}

impl Sq {
    /// `Superquadric::set`.
    fn set(center: V3, quat: &[f64; 4], shape: V3, blockiness: [f64; 2]) -> Self {
        let mut s = Self {
            center,
            shape,
            blockiness,
            gradient: [0.0; 3],
            hessian: [0.0; 9],
            rotation_matrix: quat_to_mat(quat),
            koef: 1.0,
            shape_inv: [1.0 / shape[0], 1.0 / shape[1], 1.0 / shape[2]],
            is_ellipsoid: false,
            use_int_blockiness: false,
        };
        s.calc_koef();
        s.set_flags();
        s
    }

    /// The `isEllipsoid` / `useIntBlockiness` tail of `set` and `set_blockiness`.
    fn set_flags(&mut self) {
        self.is_ellipsoid = comp_double(self.blockiness[0], 2.0, 1e-2)
            && comp_double(self.blockiness[1], 2.0, 1e-2);
        self.use_int_blockiness = if self.is_ellipsoid {
            true
        } else {
            is_integer(self.blockiness[0]) && is_integer(self.blockiness[1])
        };
    }

    /// `Superquadric::calc_koef`.
    fn calc_koef(&mut self) {
        self.koef = pow_abs(0.5, cmax(self.blockiness[0], self.blockiness[1]) - 2.0);
    }

    /// `Superquadric::set_shape`.
    fn set_shape(&mut self, a: f64, b: f64, c: f64) {
        self.shape = [a, b, c];
        self.shape_inv = [1.0 / a, 1.0 / b, 1.0 / c];
    }

    /// `Superquadric::set_blockiness`.
    fn set_blockiness(&mut self, n1: f64, n2: f64) {
        self.blockiness = [n1, n2];
        self.calc_koef();
        self.set_flags();
    }

    fn rotate_local2global(&self, v: &V3) -> V3 {
        matvec(&self.rotation_matrix, v)
    }

    fn rotate_global2local(&self, v: &V3) -> V3 {
        transpose_matvec(&self.rotation_matrix, v)
    }

    fn local2global(&self, v: &V3) -> V3 {
        let t = self.rotate_local2global(v);
        [
            t[0] + self.center[0],
            t[1] + self.center[1],
            t[2] + self.center[2],
        ]
    }

    fn global2local(&self, v: &V3) -> V3 {
        let t = sub3(v, &self.center);
        self.rotate_global2local(&t)
    }

    fn int_blockiness(&self) -> (i32, i32) {
        (
            (self.blockiness[0] + 1e-2).floor() as i32,
            (self.blockiness[1] + 1e-2).floor() as i32,
        )
    }

    /// `Superquadric::shape_function_local`.
    fn shape_function_local(&self, p: &V3) -> f64 {
        let n1 = self.blockiness[0];
        let n2 = self.blockiness[1];
        let f;
        if self.use_int_blockiness {
            let (n1_int, n2_int) = self.int_blockiness();
            if n1_int == n2_int {
                f = pow_abs_int(p[0] * self.shape_inv[0], n2_int)
                    + pow_abs_int(p[1] * self.shape_inv[1], n2_int)
                    + pow_abs_int(p[2] * self.shape_inv[2], n1_int)
                    - 1.0;
            } else {
                f = pow_abs(
                    pow_abs_int(p[0] * self.shape_inv[0], n2_int)
                        + pow_abs_int(p[1] * self.shape_inv[1], n2_int),
                    n1 / n2,
                ) + pow_abs_int(p[2] * self.shape_inv[2], n1_int)
                    - 1.0;
            }
        } else {
            f = pow_abs(
                pow_abs(p[0] * self.shape_inv[0], n2) + pow_abs(p[1] * self.shape_inv[1], n2),
                n1 / n2,
            ) + pow_abs(p[2] * self.shape_inv[2], n1)
                - 1.0;
        }
        self.koef * f
    }

    /// `Superquadric::shape_function_gradient_local`.
    fn shape_function_gradient_local(&self, p: &V3) -> V3 {
        let n1 = self.blockiness[0];
        let n2 = self.blockiness[1];
        let (a, b, c) = (self.shape_inv[0], self.shape_inv[1], self.shape_inv[2]);
        let xa = p[0] * a;
        let yb = p[1] * b;
        let zc = p[2] * c;
        let (xan21, ybn21, zcn11);
        if self.use_int_blockiness {
            let (n1_int, n2_int) = self.int_blockiness();
            xan21 = pow_abs_int(xa, n2_int - 1);
            ybn21 = pow_abs_int(yb, n2_int - 1);
            zcn11 = pow_abs_int(zc, n1_int - 1);
        } else {
            xan21 = pow_abs(xa, n2 - 1.0);
            ybn21 = pow_abs(yb, n2 - 1.0);
            zcn11 = pow_abs(zc, n1 - 1.0);
        }
        let k = self.koef;
        if comp_double(n1, n2, 1e-2) {
            [
                n1 * (k * a) * sign(xa) * xan21,
                n1 * (k * b) * sign(yb) * ybn21,
                n1 * (k * c) * sign(zc) * zcn11,
            ]
        } else {
            let xy_term = xan21 * xa.abs() + ybn21 * yb.abs();
            let xy_term_powed1 = pow_abs(xy_term, n1 / n2 - 1.0);
            [
                n1 * (k * a) * sign(xa) * xan21 * xy_term_powed1,
                n1 * (k * b) * sign(yb) * ybn21 * xy_term_powed1,
                n1 * (k * c) * sign(zc) * zcn11,
            ]
        }
    }

    /// The `xy_term_powed2` of the hessian routines.
    fn xy_term_powed2(n1: f64, n2: f64, xy_term: f64) -> f64 {
        if is_integer(n1 / n2) {
            let n1_over_n2_int = (n1 / n2 + 1e-2).floor() as i32;
            // upstream passes `n1_over_n2_int - 2.0` (a double) to an int
            // parameter: truncation
            pow_abs_int(xy_term, (f64::from(n1_over_n2_int) - 2.0) as i32)
        } else {
            pow_abs(xy_term, n1 / n2 - 2.0)
        }
    }

    /// `Superquadric::shape_function_hessian_local`.
    fn shape_function_hessian_local(&self, p: &V3) -> [f64; 9] {
        let n1 = self.blockiness[0];
        let n2 = self.blockiness[1];
        let (a, b, c) = (self.shape_inv[0], self.shape_inv[1], self.shape_inv[2]);
        let xa = p[0] * a;
        let yb = p[1] * b;
        let zc = p[2] * c;
        let (xan22, ybn22, zcn12);
        if self.use_int_blockiness {
            let (n1_int, n2_int) = self.int_blockiness();
            xan22 = pow_abs_int(xa, n2_int - 2);
            ybn22 = pow_abs_int(yb, n2_int - 2);
            zcn12 = pow_abs_int(zc, n1_int - 2);
        } else {
            xan22 = pow_abs(xa, n2 - 2.0);
            ybn22 = pow_abs(yb, n2 - 2.0);
            zcn12 = pow_abs(zc, n1 - 2.0);
        }
        let xan21 = xan22 * xa.abs();
        let ybn21 = ybn22 * yb.abs();
        let k = self.koef;
        let mut r = [0.0; 9];
        if comp_double(n1, n2, 1e-2) {
            r[0] = (k * a) * (n1 * (n2 - 1.0) * xan22) * a;
            r[4] = (k * b) * (n1 * (n2 - 1.0) * ybn22) * b;
            r[8] = (k * c) * (n1 * (n1 - 1.0) * zcn12) * c;
        } else {
            let xy_term = xan21 * xa.abs() + ybn21 * yb.abs();
            let xy_term_powed2 = Self::xy_term_powed2(n1, n2, xy_term);
            let xy_term_powed1 = xy_term_powed2 * xy_term;
            r[0] = (k * a)
                * (n1 * (n2 - 1.0) * xan22 * xy_term_powed1
                    + (n1 - n2) * n1 * xan21 * xan21 * xy_term_powed2)
                * a;
            r[4] = (k * b)
                * (n1 * (n2 - 1.0) * ybn22 * xy_term_powed1
                    + (n1 - n2) * n1 * ybn21 * ybn21 * xy_term_powed2)
                * b;
            r[1] = k
                * (n1 - n2)
                * n1
                * sign(xa)
                * sign(yb)
                * (xan21 * a)
                * (ybn21 * b)
                * xy_term_powed2;
            r[3] = r[1];
            r[8] = (k * c) * (n1 * (n1 - 1.0) * zcn12) * c;
        }
        r
    }

    /// `Superquadric::shape_function_props_local` (value, gradient and, if
    /// asked, hessian).
    fn shape_function_props_local(&self, p: &V3, want_hess: bool) -> (f64, V3, [f64; 9]) {
        let n1 = self.blockiness[0];
        let n2 = self.blockiness[1];
        let (a, b, c) = (self.shape_inv[0], self.shape_inv[1], self.shape_inv[2]);
        let xa = p[0] * a;
        let yb = p[1] * b;
        let zc = p[2] * c;
        let (xan22, ybn22, zcn12);
        if self.use_int_blockiness {
            let (n1_int, n2_int) = self.int_blockiness();
            xan22 = pow_abs_int(xa, n2_int - 2);
            ybn22 = pow_abs_int(yb, n2_int - 2);
            zcn12 = pow_abs_int(zc, n1_int - 2);
        } else {
            xan22 = pow_abs(xa, n2 - 2.0);
            ybn22 = pow_abs(yb, n2 - 2.0);
            zcn12 = pow_abs(zc, n1 - 2.0);
        }
        let xan21 = xan22 * xa.abs();
        let ybn21 = ybn22 * yb.abs();
        let zcn11 = zcn12 * zc.abs();
        let xan2 = xan21 * xa.abs();
        let ybn2 = ybn21 * yb.abs();
        let zcn1 = zcn11 * zc.abs();
        let k = self.koef;
        let mut hess = [0.0; 9];
        if comp_double(n1, n2, 1e-2) {
            let f = k * (xan2 + ybn2 + zcn1 - 1.0);
            let grad = [
                k * a * n1 * sign(xa) * xan21,
                k * b * n1 * sign(yb) * ybn21,
                k * c * n1 * sign(zc) * zcn11,
            ];
            if want_hess {
                hess[0] = (k * a) * (n1 * (n2 - 1.0) * xan22) * a;
                hess[4] = (k * b) * (n1 * (n2 - 1.0) * ybn22) * b;
                hess[8] = (k * c) * (n1 * (n1 - 1.0) * zcn12) * c;
            }
            (f, grad, hess)
        } else {
            let f = k * (pow_abs(xan2 + ybn2, n1 / n2) + zcn1 - 1.0);
            let xy_term = xan21 * xa.abs() + ybn21 * yb.abs();
            let xy_term_powed2 = Self::xy_term_powed2(n1, n2, xy_term);
            let xy_term_powed1 = xy_term_powed2 * xy_term;
            let grad = [
                k * a * n1 * sign(xa) * xan21 * xy_term_powed1,
                k * b * n1 * sign(yb) * ybn21 * xy_term_powed1,
                k * c * n1 * sign(zc) * zcn11,
            ];
            if want_hess {
                hess[0] = (k * a)
                    * (n1 * (n2 - 1.0) * xan22 * xy_term_powed1
                        + (n1 - n2) * n1 * xan21 * xan21 * xy_term_powed2)
                    * a;
                hess[4] = (k * b)
                    * (n1 * (n2 - 1.0) * ybn22 * xy_term_powed1
                        + (n1 - n2) * n1 * ybn21 * ybn21 * xy_term_powed2)
                    * b;
                hess[1] = k
                    * (n1 - n2)
                    * n1
                    * sign(xa)
                    * sign(yb)
                    * (xan21 * a)
                    * (ybn21 * b)
                    * xy_term_powed2;
                hess[3] = hess[1];
                hess[8] = (k * c) * (n1 * (n1 - 1.0) * zcn12) * c;
            }
            (f, grad, hess)
        }
    }

    /// `Superquadric::tensor_quat_rotate`: `A M A^T`.
    fn tensor_quat_rotate(&self, tensor: &[f64; 9]) -> [f64; 9] {
        let temp = times3(&self.rotation_matrix, tensor);
        times3_transpose(&temp, &self.rotation_matrix)
    }

    /// `Superquadric::shape_function_props_global` without the hessian.
    fn props_global(&self, p: &V3) -> (f64, V3) {
        let local = self.global2local(p);
        let (f, grad_local, _) = self.shape_function_props_local(&local, false);
        (f, self.rotate_local2global(&grad_local))
    }

    /// `Superquadric::shape_function_hessian_global`.
    fn shape_function_hessian_global(&self, p: &V3) -> [f64; 9] {
        let local = self.global2local(p);
        let h = self.shape_function_hessian_local(&local);
        self.tensor_quat_rotate(&h)
    }

    /// `Superquadric::plane_intersection`: the point of maximal penetration and
    /// whether the particle and the plane intersect.
    /// Returns `(intersect, point_of_maximal_penetration,
    /// point_of_lowest_potential)`, both points in the global frame.
    fn plane_intersection(&self, normal_global: &V3, x0_global: &V3) -> (bool, V3, V3) {
        let mut normal_local = self.rotate_global2local(normal_global);
        let x0_local = self.global2local(x0_global);
        if dot3(&normal_local, &x0_local) < 0.0 {
            normal_local = [-normal_local[0], -normal_local[1], -normal_local[2]];
        }
        let n1 = self.blockiness[0];
        let n2 = self.blockiness[1];
        let (nx, ny, nz) = (normal_local[0], normal_local[1], normal_local[2]);
        let (a, b, c) = (self.shape[0], self.shape[1], self.shape[2]);
        let mut pmp = [0.0; 3];
        let mut plp = [0.0; 3];
        let d = dot3(&normal_local, &x0_local);
        if comp_double(nx, 0.0, 1e-10) && comp_double(ny, 0.0, 1e-10) {
            pmp[0] = 0.0;
            pmp[1] = 0.0;
            pmp[2] = 1.0;
            plp[0] = 0.0;
            plp[1] = 0.0;
            plp[2] = d / nz;
        } else {
            if nx.abs() > ny.abs() {
                let alpha = pow_abs((ny * b) / (nx * a), 1.0 / (n2 - 1.0));
                let gamma1 = 1.0 + pow_abs(alpha, n2);
                let gamma = pow_abs(gamma1, n1 / n2 - 1.0);
                let beta = pow_abs((nz * c) / (nx * a) * gamma, 1.0 / (n1 - 1.0));
                pmp[0] = 1.0 / pow_abs(pow_abs(gamma1, n1 / n2) + pow_abs(beta, n1), 1.0 / n1);
                pmp[1] = alpha * pmp[0];
                pmp[2] = beta * pmp[0];
                plp[0] = d.abs() / (nx.abs() * a + alpha * ny.abs() * b + beta * nz.abs() * c);
                plp[1] = alpha * plp[0];
                plp[2] = beta * plp[0];
            } else {
                let alpha = pow_abs((nx * a) / (ny * b), 1.0 / (n2 - 1.0));
                let gamma1 = 1.0 + pow_abs(alpha, n2);
                let gamma = pow_abs(gamma1, n1 / n2 - 1.0);
                let beta = pow_abs((nz * c) / (ny * b) * gamma, 1.0 / (n1 - 1.0));
                pmp[1] = 1.0 / pow_abs(pow_abs(gamma1, n1 / n2) + pow_abs(beta, n1), 1.0 / n1);
                pmp[0] = alpha * pmp[1];
                pmp[2] = beta * pmp[1];
                plp[1] = d.abs() / (ny.abs() * b + alpha * nx.abs() * a + beta * nz.abs() * c);
                plp[0] = alpha * plp[1];
                plp[2] = beta * plp[1];
            }
            for i in 0..3 {
                plp[i] *= self.shape[i] * sign(normal_local[i]);
            }
        }
        for i in 0..3 {
            pmp[i] *= self.shape[i] * sign(normal_local[i]);
        }
        let ff = self.shape_function_local(&plp);
        let pmp_global = self.local2global(&pmp);
        let plp_global = self.local2global(&plp);
        (ff < 0.0, pmp_global, plp_global)
    }

    /// `Superquadric::shape_function_global`.
    fn shape_function_global(&self, p: &V3) -> f64 {
        let local = self.global2local(p);
        self.shape_function_local(&local)
    }

    /// `Superquadric::shape_function_gradient_global`.
    fn shape_function_gradient_global(&self, p: &V3) -> V3 {
        let local = self.global2local(p);
        let g = self.shape_function_gradient_local(&local);
        self.rotate_local2global(&g)
    }

    /// The two golden-section probes of `line_intersection` /
    /// `edge_intersection` at the bracket `[a, b]`.
    fn golden_probes(&self, pa: &V3, pb: &V3, a: f64, b: f64) -> (V3, V3, f64, f64) {
        let alpha1 = b - (b - a) * PHI_INV;
        let alpha2 = a + (b - a) * PHI_INV;
        let mut c1 = [0.0; 3];
        let mut c2 = [0.0; 3];
        for k in 0..3 {
            c1[k] = pa[k] + alpha1 * (pb[k] - pa[k]);
            c2[k] = pa[k] + alpha2 * (pb[k] - pa[k]);
        }
        let f1 = self.shape_function_local(&c1);
        let f2 = self.shape_function_local(&c2);
        (c1, c2, f1, f2)
    }

    /// `Superquadric::line_intersection`: the point of the segment `AB`
    /// (corners included) with the lowest shape-function value, by golden
    /// section to `1e-8`. Returns `(f, point)`.
    fn line_intersection(&self, point_a: &V3, point_b: &V3) -> (f64, V3) {
        let pa = self.global2local(point_a);
        let pb = self.global2local(point_b);
        let mut a: f64 = 0.0;
        let mut b = 1.0;
        let mut eps = (b - a).abs();
        let (mut c1, mut c2, _, _) = self.golden_probes(&pa, &pb, a, b);
        while eps > 1e-8 {
            let (n1, n2, f1, f2) = self.golden_probes(&pa, &pb, a, b);
            c1 = n1;
            c2 = n2;
            if f1 >= f2 {
                a = b - (b - a) * PHI_INV;
            } else {
                b = a + (b - a) * PHI_INV;
            }
            eps = (b - a).abs();
        }
        let mut cl = [0.0; 3];
        for k in 0..3 {
            cl[k] = 0.5 * (c1[k] + c2[k]);
        }
        let mut f = self.shape_function_local(&cl);
        let fa = self.shape_function_local(&pa);
        let fb = self.shape_function_local(&pb);
        let closest;
        if fa < f {
            closest = *point_a;
            f = fa;
        } else if fb < f {
            closest = *point_b;
            f = fb;
        } else {
            closest = self.local2global(&cl);
        }
        (f, closest)
    }

    /// `Superquadric::edge_intersection`: whether the segment `AB` enters the
    /// particle (golden section to `1e-4`, stopping at the first probe inside).
    fn edge_intersection(&self, point_a: &V3, point_b: &V3) -> bool {
        let pa = self.global2local(point_a);
        let pb = self.global2local(point_b);
        let mut a: f64 = 0.0;
        let mut b = 1.0;
        let mut eps = (b - a).abs();
        let (mut c1, mut c2, f1, f2) = self.golden_probes(&pa, &pb, a, b);
        if f1 < 0.0 || f2 < 0.0 {
            return true;
        }
        while eps > 1e-4 {
            let (n1, n2, f1, f2) = self.golden_probes(&pa, &pb, a, b);
            c1 = n1;
            c2 = n2;
            if f1 < 0.0 || f2 < 0.0 {
                return true;
            }
            if f1 >= f2 {
                a = b - (b - a) * PHI_INV;
            } else {
                b = a + (b - a) * PHI_INV;
            }
            eps = (b - a).abs();
        }
        let mut cl = [0.0; 3];
        for k in 0..3 {
            cl[k] = 0.5 * (c1[k] + c2[k]);
        }
        self.shape_function_local(&cl) < 0.0
    }

    /// `Superquadric::surface_line_intersection1`: up to 10 plain Newton
    /// steps from `start` along `dir`; returns the parameter `alpha`.
    fn surface_line_intersection1(&self, start: &V3, dir: &V3) -> f64 {
        let sp_local = self.global2local(start);
        let mut point = sp_local;
        let dv_local = self.rotate_global2local(dir);
        let mut alpha = 0.0;
        let eps = 1e-16;
        let mut f0 = self.shape_function_local(&sp_local);
        for _ in 0..10 {
            if f0.abs() < eps * self.koef {
                break;
            }
            let grad_local = self.shape_function_gradient_local(&point);
            let f_der = dot3(&grad_local, &dv_local);
            let delta = -f0 / f_der;
            alpha += delta;
            point = yabx3(&point, delta, &dv_local);
            if delta.abs() <= eps {
                break;
            }
            f0 = self.shape_function_local(&point);
        }
        alpha
    }

    /// `Superquadric::calc_curvature_coefficient`: the mean (`gaussian =
    /// false`) or Gaussian curvature of the surface at the point `p` mapped
    /// onto it along the ray from the centre.
    fn calc_curvature_coefficient(&self, gaussian: bool, p: &V3) -> f64 {
        let mut local = self.global2local(p);
        let f = self.shape_function_local(&local);
        let alpha = 1.0 / pow_abs(f / self.koef + 1.0, 1.0 / self.blockiness[0]);
        for v in &mut local {
            *v *= alpha;
        }
        let ff = self.shape_function_gradient_local(&local);
        let hessian = self.shape_function_hessian_local(&local);
        let f_mag = mag3(&ff);
        let n = [
            ff[0] * (1.0 / f_mag),
            ff[1] * (1.0 / f_mag),
            ff[2] * (1.0 / f_mag),
        ];
        let temp = matvec(&hessian, &n);
        if !gaussian {
            return (dot3(&n, &temp) - (hessian[0] + hessian[4] + hessian[8])).abs()
                / (2.0 * f_mag).abs();
        }
        let (fx, fy, fz) = (ff[0], ff[1], ff[2]);
        let (fxx, fxy, fxz) = (hessian[0], hessian[1], hessian[2]);
        let (fyy, fyz) = (hessian[4], hessian[5]);
        let fzz = hessian[8];
        let mat = [
            fxx, fxy, fxz, fx, fxy, fyy, fyz, fy, fxz, fyz, fzz, fz, fx, fy, fz, 0.0,
        ];
        let k = -determinant_4x4(&mat) / (f_mag * f_mag * f_mag * f_mag);
        k.abs().sqrt()
    }

    /// `Superquadric::ellipsoid_line_intersection_local`.
    fn ellipsoid_line_intersection_local(&self, sp: &V3, dv: &V3) -> (f64, f64) {
        let x0 = sp[0] * self.shape_inv[0];
        let nx = dv[0] * self.shape_inv[0];
        let y0 = sp[1] * self.shape_inv[1];
        let ny = dv[1] * self.shape_inv[1];
        let z0 = sp[2] * self.shape_inv[2];
        let nz = dv[2] * self.shape_inv[2];
        let a = nx * nx + ny * ny + nz * nz;
        let b = 2.0 * (x0 * nx + y0 * ny + z0 * nz);
        let c = x0 * x0 + y0 * y0 + z0 * z0 - 1.0;
        let d = b * b - 4.0 * a * c;
        let two_a_inverted = 0.5 / a;
        if d < 0.0 {
            let a1 = -b * two_a_inverted;
            (a1, a1)
        } else {
            let sqrt_d = d.sqrt();
            (
                (-b + sqrt_d) * two_a_inverted,
                (-b - sqrt_d) * two_a_inverted,
            )
        }
    }

    /// `Superquadric::surface_line_intersection` (20 iterations,
    /// `use_alpha_ellipsoid = false`, as `extended_overlap_algorithm` calls it):
    /// the parameter `alpha` of the point `start + alpha dir` on the surface.
    fn surface_line_intersection(&self, start: &V3, dir: &V3, alpha_prev: f64) -> (f64, V3) {
        let max_num_iters = 20;
        let sp_local = self.global2local(start);
        let dv_local = self.rotate_global2local(dir);
        let mut alpha = alpha_prev;
        let eps0 = 1e-14;
        if self.is_ellipsoid {
            let (alpha1, alpha2) = self.ellipsoid_line_intersection_local(&sp_local, &dv_local);
            alpha = if alpha1 > -eps0 && alpha2 < -eps0 {
                alpha1
            } else if alpha1 < -eps0 && alpha2 > -eps0 {
                alpha2
            } else if alpha1 < -eps0 && alpha2 < -eps0 {
                cmax(alpha1, alpha2)
            } else {
                cmin(alpha1, alpha2)
            };
        } else {
            let a = 0.5 * cmin(cmin(self.shape[0], self.shape[1]), self.shape[2]) / mag3(dir);
            let eps = 1e-16;
            alpha = clamp(alpha, -a, a);
            let mut point = yabx3(&sp_local, alpha, &dv_local);
            let mut f0 = self.shape_function_local(&point);
            if f0.abs() < eps {
                let r = self.local2global(&point);
                return (alpha, r);
            }
            for _ in 0..max_num_iters {
                if f0.abs() < eps * self.koef {
                    break;
                }
                let grad_local = self.shape_function_gradient_local(&point);
                let f_der = dot3(&grad_local, &dv_local);
                let mut delta;
                if f_der.abs() < 1e-10 {
                    delta = 1e-10;
                    let point_ = yabx3(&point, delta, &dv_local);
                    let grad_local_ = self.shape_function_gradient_local(&point_);
                    let f_der_ = dot3(&grad_local_, &dv_local);
                    if f_der_ < 0.0 {
                        delta = -delta;
                    }
                } else {
                    delta = clamp(-f0 / f_der, -a, a);
                }
                if delta.abs() <= eps {
                    alpha += delta;
                    // upstream also advances the local point here; it is not read again
                    break;
                } else {
                    while delta.abs() > eps {
                        let point_ = yabx3(&point, delta, &dv_local);
                        let f_ = self.shape_function_local(&point_);
                        if f_.abs() < f0.abs() || f_.abs() < eps * self.koef {
                            f0 = f_;
                            alpha += delta;
                            point = point_;
                            break;
                        } else {
                            delta *= 0.5;
                        }
                    }
                }
            }
        }
        let result = [
            start[0] + alpha * dir[0],
            start[1] + alpha * dir[1],
            start[2] + alpha * dir[2],
        ];
        (alpha, result)
    }
}

// ---------------------------------------------------------------------------
// Pair contact detection (math_extra_liggghts_superquadric.cpp).
// ---------------------------------------------------------------------------

/// `check_inequalities`: whether axis `i` separates the two boxes.
fn check_inequalities(i: u32, a: &V3, b: &V3, a_delta: &V3, b_delta: &V3, c: &[f64; 9]) -> bool {
    let (r0, r1, r) = match i {
        0 => (
            a[0],
            b[0] * c[0].abs() + b[1] * c[1].abs() + b[2] * c[2].abs(),
            a_delta[0].abs(),
        ),
        1 => (
            a[1],
            b[0] * c[3].abs() + b[1] * c[4].abs() + b[2] * c[5].abs(),
            a_delta[1].abs(),
        ),
        2 => (
            a[2],
            b[0] * c[6].abs() + b[1] * c[7].abs() + b[2] * c[8].abs(),
            a_delta[2].abs(),
        ),
        3 => (
            a[0] * c[0].abs() + a[1] * c[3].abs() + a[2] * c[6].abs(),
            b[0],
            b_delta[0].abs(),
        ),
        4 => (
            a[0] * c[1].abs() + a[1] * c[4].abs() + a[2] * c[7].abs(),
            b[1],
            b_delta[1].abs(),
        ),
        5 => (
            a[0] * c[2].abs() + a[1] * c[5].abs() + a[2] * c[8].abs(),
            b[2],
            b_delta[2].abs(),
        ),
        6 => (
            a[1] * c[6].abs() + a[2] * c[3].abs(),
            b[1] * c[2].abs() + b[2] * c[1].abs(),
            (c[3] * a_delta[2] - c[6] * a_delta[1]).abs(),
        ),
        7 => (
            a[1] * c[7].abs() + a[2] * c[4].abs(),
            b[0] * c[2].abs() + b[2] * c[0].abs(),
            (c[4] * a_delta[2] - c[7] * a_delta[1]).abs(),
        ),
        8 => (
            a[1] * c[8].abs() + a[2] * c[5].abs(),
            b[0] * c[1].abs() + b[1] * c[0].abs(),
            (c[5] * a_delta[2] - c[8] * a_delta[1]).abs(),
        ),
        9 => (
            a[0] * c[6].abs() + a[2] * c[0].abs(),
            b[1] * c[5].abs() + b[2] * c[4].abs(),
            (c[6] * a_delta[0] - c[0] * a_delta[2]).abs(),
        ),
        10 => (
            a[0] * c[7].abs() + a[2] * c[1].abs(),
            b[0] * c[5].abs() + b[2] * c[3].abs(),
            (c[7] * a_delta[0] - c[1] * a_delta[2]).abs(),
        ),
        11 => (
            a[0] * c[8].abs() + a[2] * c[2].abs(),
            b[0] * c[4].abs() + b[1] * c[3].abs(),
            (c[8] * a_delta[0] - c[2] * a_delta[2]).abs(),
        ),
        12 => (
            a[0] * c[3].abs() + a[1] * c[0].abs(),
            b[1] * c[8].abs() + b[2] * c[7].abs(),
            (c[0] * a_delta[1] - c[3] * a_delta[0]).abs(),
        ),
        13 => (
            a[0] * c[4].abs() + a[1] * c[1].abs(),
            b[0] * c[8].abs() + b[2] * c[6].abs(),
            (c[1] * a_delta[1] - c[4] * a_delta[0]).abs(),
        ),
        14 => (
            a[0] * c[5].abs() + a[1] * c[2].abs(),
            b[0] * c[7].abs() + b[1] * c[6].abs(),
            (c[2] * a_delta[1] - c[5] * a_delta[0]).abs(),
        ),
        _ => (0.0, 0.0, 0.0),
    };
    if (r0 + r1).abs() < 1e-14 && r.abs() < 1e-14 {
        false
    } else {
        r > r0 + r1
    }
}

/// `obb_intersect(particleA, particleB, inequality_to_start)`.
fn obb_intersect(pa: &Sq, pb: &Sq, inequality_to_start: &mut u32) -> bool {
    let delta = sub3(&pa.center, &pb.center);
    let sa = [
        pa.shape[0] * SCALE_KOEF,
        pa.shape[1] * SCALE_KOEF,
        pa.shape[2] * SCALE_KOEF,
    ];
    let sb = [
        pb.shape[0] * SCALE_KOEF,
        pb.shape[1] * SCALE_KOEF,
        pb.shape[2] * SCALE_KOEF,
    ];
    let c = transpose_times3(&pa.rotation_matrix, &pb.rotation_matrix);
    let a_delta = transpose_matvec(&pa.rotation_matrix, &delta);
    let b_delta = transpose_matvec(&pb.rotation_matrix, &delta);
    let mut result = true;
    for i in 0..15u32 {
        let check = if i == 0 {
            *inequality_to_start
        } else if i == *inequality_to_start {
            0
        } else {
            i
        };
        if check_inequalities(check, &sa, &sb, &a_delta, &b_delta, &c) {
            *inequality_to_start = check;
            result = false;
            break;
        }
    }
    result
}

/// `calc_F` with `mu` an output (`double *mu`): the residual of the
/// contact-point system at `point`.
struct CalcF {
    res: f64,
    fi: f64,
    fj: f64,
    grad_a: V3,
    grad_b: V3,
    f: [f64; 4],
    merit: f64,
}

/// `calc_F(..., double *mu, ...)`; returns `(CalcF, mu)`.
fn calc_f_mu_out(pa: &Sq, pb: &Sq, point: &V3) -> (CalcF, f64) {
    let (f1, grad_a) = pa.props_global(point);
    let (f2, grad_b) = pb.props_global(point);
    let mu_sq = -dot3(&grad_a, &grad_b) / dot3(&grad_b, &grad_b);
    let mu = mu_sq.abs().sqrt();
    (calc_f_tail(f1, f2, grad_a, grad_b, mu_sq), mu)
}

/// `calc_F(..., double mu, ...)`.
fn calc_f_mu_in(pa: &Sq, pb: &Sq, point: &V3, mu: f64) -> CalcF {
    let (f1, grad_a) = pa.props_global(point);
    let (f2, grad_b) = pb.props_global(point);
    let mu_sq = mu * mu;
    calc_f_tail(f1, f2, grad_a, grad_b, mu_sq)
}

fn calc_f_tail(f1: f64, f2: f64, grad_a: V3, grad_b: V3, mu_sq: f64) -> CalcF {
    let f = [
        grad_a[0] + mu_sq * grad_b[0],
        grad_a[1] + mu_sq * grad_b[1],
        grad_a[2] + mu_sq * grad_b[2],
        f1 - f2,
    ];
    let v = cross3(&grad_a, &grad_b);
    let sine = (dot3(&v, &v) / (dot3(&grad_a, &grad_a) * dot3(&grad_b, &grad_b))).abs();
    let aaa = (f1 - f2).abs() / (cmax(f1.abs(), f2.abs()) + 1e-16);
    let merit = cmax(sine, aaa * aaa);
    CalcF {
        res: f[0] * f[0] + f[1] * f[1] + f[2] * f[2] + f[3] * f[3],
        fi: f1,
        fj: f2,
        grad_a,
        grad_b,
        f,
        merit,
    }
}

/// `calc_contact_point`: Newton's method for the point where both shape
/// functions are equal and the gradients anti-parallel. Returns the fail flag;
/// writes `result_point`, `fi`, `fj` and the two particles' `gradient`.
fn calc_contact_point(
    pa: &mut Sq,
    pb: &mut Sq,
    ratio: f64,
    initial_point1: &V3,
    result_point: &mut V3,
    fi: &mut f64,
    fj: &mut f64,
) -> bool {
    let tol1 = 1e-10;
    let tol2 = 1e-12;
    let mut fail = false;

    let (c1, mu1_init) = calc_f_mu_out(pa, pb, initial_point1);
    if c1.merit < tol1 {
        *result_point = *initial_point1;
        pa.gradient = c1.grad_a;
        pb.gradient = c1.grad_b;
        *fi = c1.fi;
        *fj = c1.fj;
        return fail;
    }
    let mut initial_point2 = [0.0; 3];
    for k in 0..3 {
        initial_point2[k] = ratio * pb.center[k] + (1.0 - ratio) * pa.center[k];
    }
    let (c2, mu2_init) = calc_f_mu_out(pa, pb, &initial_point2);
    let res0;
    let mut point;
    let mut f_vec;
    let mut mu;
    let merit0;
    if c1.res < c2.res {
        res0 = c1.res;
        pa.gradient = c1.grad_a;
        pb.gradient = c1.grad_b;
        point = *initial_point1;
        f_vec = c1.f;
        *fi = c1.fi;
        *fj = c1.fj;
        mu = mu1_init;
        merit0 = c1.merit;
    } else {
        res0 = c2.res;
        pa.gradient = c2.grad_a;
        pb.gradient = c2.grad_b;
        point = initial_point2;
        f_vec = c2.f;
        *fi = c2.fi;
        *fj = c2.fj;
        mu = mu2_init;
        merit0 = c2.merit;
    }
    if merit0 < tol1 {
        *result_point = point;
        return fail;
    }
    let mut merit1 = merit0;
    let mut merit2;
    let mut res1 = res0;
    let mut res2;
    let size_i = min3(pa.shape[0], pa.shape[1], pa.shape[2]);
    let size_j = min3(pb.shape[0], pb.shape[1], pb.shape[2]);
    let size = cmin(size_i, size_j);

    let mut j4 = [0.0; 16];
    let mut delta = [0.0; 4];
    let mut delta_0;
    let niter = 100_000;
    let mut j4_inv = [0.0; 16];
    // the golden-section scratch of upstream's outer scope
    let mut mu1 = 0.0;
    let mut mu2 = 0.0;
    let mut grad_a1 = [0.0; 3];
    let mut grad_b1 = [0.0; 3];
    let mut grad_a2 = [0.0; 3];
    let mut grad_b2 = [0.0; 3];
    let mut fi1 = 0.0;
    let mut fj1 = 0.0;
    let mut fi2 = 0.0;
    let mut fj2 = 0.0;
    let mut f1v = [0.0; 4];
    let mut f2v = [0.0; 4];
    let mut pointb = [0.0; 3];

    for iter in 0..niter {
        merit2 = merit1;
        res2 = res1;
        pa.hessian = pa.shape_function_hessian_global(&point);
        pb.hessian = pb.shape_function_hessian_global(&point);
        let mu_sq = mu * mu;
        for i in 0..3 {
            for j in 0..3 {
                j4[4 * i + j] = pa.hessian[i * 3 + j] + mu_sq * pb.hessian[i * 3 + j];
            }
            j4[3 + 4 * i] = 2.0 * mu * pb.gradient[i];
            j4[i + 4 * 3] = pa.gradient[i] - pb.gradient[i];
        }
        j4[15] = 0.0;
        let det = determinant_4x4(&j4);
        if det.abs() > 1.0 {
            inverse_matrix_4x4(&j4, &mut j4_inv);
            delta = matvec_n::<4>(&j4_inv, &f_vec);
        } else {
            delta_0 = delta;
            let mut out = [0.0; 4];
            gmres4(&j4, &f_vec, &delta_0, &mut out);
            delta = out;
        }
        let mut nan_found = false;
        for d in &delta {
            if d.is_nan() {
                nan_found = true;
                fail = true;
                break;
            }
        }
        let mut converged = false;
        let deltax = max3(delta[0].abs(), delta[1].abs(), delta[2].abs());
        if deltax == 0.0 {
            *result_point = point;
            converged = true;
        } else {
            if nan_found {
                break;
            }
            let point_ = [
                point[0] - delta[0],
                point[1] - delta[1],
                point[2] - delta[2],
            ];
            let mu_ = mu - delta[3];
            let c_ = calc_f_mu_in(pa, pb, &point_, mu_);
            // upstream writes the trial gradients and residual in place
            pa.gradient = c_.grad_a;
            pb.gradient = c_.grad_b;
            f_vec = c_.f;
            if c_.res < res1 || c_.merit < tol1 || deltax < tol2 * size {
                merit2 = c_.merit;
                res2 = c_.res;
                mu = mu_;
                *fi = c_.fi;
                *fj = c_.fj;
                point = point_;
                if merit2 < tol1 || deltax < tol2 * size {
                    converged = true;
                }
            } else {
                let mut a = 0.0;
                let mut b: f64 = 1.0;
                let mut eps = (b - a).abs();
                let mut res2b = 0.0;
                let mut merit2b = 0.0;
                while eps > 1e-8 {
                    let alpha1 = b - (b - a) * PHI_INV;
                    let alpha2 = a + (b - a) * PHI_INV;
                    let pointa = yabx3(&point, -alpha1, &delta);
                    mu1 = mu - alpha1 * delta[3];
                    let ca = calc_f_mu_in(pa, pb, &pointa, mu1);
                    fi1 = ca.fi;
                    fj1 = ca.fj;
                    grad_a1 = ca.grad_a;
                    grad_b1 = ca.grad_b;
                    f1v = ca.f;
                    let res2a = ca.res;
                    let merit2a = ca.merit;
                    pointb = yabx3(&point, -alpha2, &delta);
                    mu2 = mu - alpha2 * delta[3];
                    let cb = calc_f_mu_in(pa, pb, &pointb, mu2);
                    fi2 = cb.fi;
                    fj2 = cb.fj;
                    grad_a2 = cb.grad_a;
                    grad_b2 = cb.grad_b;
                    f2v = cb.f;
                    res2b = cb.res;
                    merit2b = cb.merit;
                    if cmin(res2a, res2b) < res1 {
                        if res2a < res2b {
                            res2 = res2a;
                            f_vec = f1v;
                            mu = mu1;
                            pa.gradient = grad_a1;
                            pb.gradient = grad_b1;
                            point = pointa;
                            *fi = fi1;
                            *fj = fj1;
                            merit2 = merit2a;
                        } else {
                            res2 = res2b;
                            f_vec = f2v;
                            mu = mu2;
                            pa.gradient = grad_a2;
                            pb.gradient = grad_b2;
                            point = pointb;
                            *fi = fi2;
                            *fj = fj2;
                            merit2 = merit2b;
                        }
                        if merit2 < tol1 {
                            converged = true;
                        }
                        eps = 0.0;
                    } else {
                        if res2a > res2b {
                            a = alpha1;
                        } else {
                            b = alpha2;
                        }
                        eps = (b - a).abs();
                    }
                }
                if eps != 0.0 {
                    f_vec = f2v;
                    mu = mu2;
                    pa.gradient = grad_a2;
                    pb.gradient = grad_b2;
                    point = pointb;
                    *fi = fi2;
                    *fj = fj2;
                    res2 = res2b;
                    merit2 = merit2b;
                    if merit2 < tol1 {
                        converged = true;
                    }
                }
            }
            if (res1 - res2).abs() / cmax(res1, res2) < 1e-3 {
                converged = true;
            }
        }
        merit1 = merit2;
        res1 = res2;
        if iter >= niter - 1 {
            fail = true;
        }
        if converged || fail {
            *result_point = point;
            break;
        }
    }
    // silence the scratch the last iteration left unread
    let _ = (mu1, mu2, grad_a1, grad_b1, fi1, fj1, f1v, merit1);
    fail
}

/// `calc_contact_point_if_no_previous_point_avaialable`: walk both shapes
/// from equal-volume spheres to the real ones.
fn calc_contact_point_no_prev(
    pa: &mut Sq,
    pb: &mut Sq,
    contact_point: &mut V3,
    fi: &mut f64,
    fj: &mut f64,
) -> bool {
    let mut fail_flag = false;
    let (ai, bi, ci) = (pa.shape[0], pa.shape[1], pa.shape[2]);
    let (aj, bj, cj) = (pb.shape[0], pb.shape[1], pb.shape[2]);
    let ri = m_cbrt(ai * bi * ci);
    let rj = m_cbrt(aj * bj * cj);
    let ratio = ri / (ri + rj);
    let (ni0, ni1) = (pa.blockiness[0], pa.blockiness[1]);
    let (nj0, nj1) = (pb.blockiness[0], pb.blockiness[1]);
    let nmax = cmax(cmax(cmax(ni0, ni1), nj0), nj1);
    let step = 1.0;
    let n1 = ((nmax - 2.0) / step + 1.0) as i32;
    let n = if n1 < 10 { 10 } else { n1 };
    let n_inv = 1.0 / f64::from(n);
    for i in 0..=n {
        let fi_ = f64::from(i);
        pa.set_blockiness(
            2.0 + (ni0 - 2.0) * fi_ * n_inv,
            2.0 + (ni1 - 2.0) * fi_ * n_inv,
        );
        pb.set_blockiness(
            2.0 + (nj0 - 2.0) * fi_ * n_inv,
            2.0 + (nj1 - 2.0) * fi_ * n_inv,
        );
        pa.set_shape(
            ri + (ai - ri) * fi_ * n_inv,
            ri + (bi - ri) * fi_ * n_inv,
            ri + (ci - ri) * fi_ * n_inv,
        );
        pb.set_shape(
            rj + (aj - rj) * fi_ * n_inv,
            rj + (bj - rj) * fi_ * n_inv,
            rj + (cj - rj) * fi_ * n_inv,
        );
        if i == 0 {
            for k in 0..3 {
                contact_point[k] = ratio * pb.center[k] + (1.0 - ratio) * pa.center[k];
            }
        }
        let pre_estimation = *contact_point;
        // upstream overwrites the flag on every pass; the last one is returned
        fail_flag = calc_contact_point(pa, pb, ratio, &pre_estimation, contact_point, fi, fj);
        pa.set_shape(ai, bi, ci);
        pb.set_shape(aj, bj, cj);
        pa.set_blockiness(ni0, ni1);
        pb.set_blockiness(nj0, nj1);
    }
    fail_flag
}

// ---------------------------------------------------------------------------
// Public types.
// ---------------------------------------------------------------------------

/// One superquadric particle (`atom_style superquadric`).
///
/// Built as upstream's `create_atoms single` followed by
/// `set atom N shape a b c blockiness n1 n2 density rho quat_direct w x y z`
/// and `set atom N vx .. omegax ..`: the bounding radius, volume, mass and
/// principal moments follow from the shape, and the angular momentum from
/// `omega` (`omega_to_angmom`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SuperquadricParticle {
    /// Centre `[m]`.
    pub x: Vec3,
    /// Velocity `[m/s]`.
    pub v: Vec3,
    /// Angular velocity `[rad/s]`.
    pub omega: Vec3,
    /// Angular momentum `[kg m^2/s]`.
    pub angmom: Vec3,
    /// Orientation quaternion `(w, x, y, z)`, body to lab.
    pub quat: [f64; 4],
    /// Half-axes `a, b, c` `[m]`.
    pub shape: [f64; 3],
    /// Blockiness `n1, n2` `[-]`.
    pub blockiness: [f64; 2],
    /// Bounding-sphere radius `[m]` (times upstream's `scale_koef = 1.0001`).
    pub radius: f64,
    /// Volume `[m^3]`.
    pub volume: f64,
    /// Mass `[kg]`.
    pub mass: f64,
    /// Principal moments of inertia `[kg m^2]`.
    pub inertia: [f64; 3],
    /// Atom type, 0-based (upstream type 1 is 0).
    pub itype: usize,
    force: V3,
    torque: V3,
}

impl SuperquadricParticle {
    /// A particle at `x` with half-axes `shape`, blockiness `(n1, n2)`,
    /// density, orientation `quat` (normalised, as `set ... quat_direct`),
    /// velocity `v` `[m/s]` and angular velocity `omega` `[rad/s]`.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] for what `set` rejects: a non-positive
    /// half-axis or density, a blockiness below 2, or a zero quaternion.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        x: [Length; 3],
        shape: [Length; 3],
        blockiness: [f64; 2],
        density: MassDensity,
        quat: [f64; 4],
        v: Vec3,
        omega: Vec3,
        itype: usize,
    ) -> Result<Self, DemError> {
        let shape = [
            shape[0].get::<meter>(),
            shape[1].get::<meter>(),
            shape[2].get::<meter>(),
        ];
        let density = density.get::<kilogram_per_cubic_meter>();
        if shape.iter().any(|s| !(*s > 0.0)) {
            return Err(DemError::InvalidInput(
                "Invalid shape in set command".into(),
            ));
        }
        if blockiness[0] < 2.0 || blockiness[1] < 2.0 {
            return Err(DemError::InvalidInput(
                "Invalid blockiness (<2) in set command".into(),
            ));
        }
        if !(density > 0.0) {
            return Err(DemError::InvalidInput(
                "Invalid density in set command".into(),
            ));
        }
        let norm2 = quat[0] * quat[0] + quat[1] * quat[1] + quat[2] * quat[2] + quat[3] * quat[3];
        if !(norm2 > 0.0) {
            return Err(DemError::InvalidInput("quaternion must be non-zero".into()));
        }
        let mut q = quat;
        crate::multisphere::qnormalize(&mut q);
        let radius = bounding_sphere_radius(&shape, &blockiness);
        let volume = volume_superquadric(&shape, &blockiness);
        let mass = density * volume;
        let inertia = inertia_superquadric(&shape, &blockiness, density);
        let w = [omega.x, omega.y, omega.z];
        let l = omega_to_angmom(&q, &w, &inertia);
        Ok(Self {
            x: Vec3::new(
                x[0].get::<meter>(),
                x[1].get::<meter>(),
                x[2].get::<meter>(),
            ),
            v,
            omega,
            angmom: Vec3::new(l[0], l[1], l[2]),
            quat: q,
            shape,
            blockiness,
            radius,
            volume,
            mass,
            inertia,
            itype,
            force: [0.0; 3],
            torque: [0.0; 3],
        })
    }

    /// Total force of the last force evaluation `[N]`.
    #[must_use]
    pub fn force(&self) -> Vec3 {
        Vec3::new(self.force[0], self.force[1], self.force[2])
    }

    /// Total torque of the last force evaluation `[N m]`.
    #[must_use]
    pub fn torque(&self) -> Vec3 {
        Vec3::new(self.torque[0], self.torque[1], self.torque[2])
    }

    /// Volume-equivalent radius `cbrt(0.75 V / pi)` (upstream's `sidata.radi`).
    fn equivalent_radius(&self) -> f64 {
        m_cbrt(0.75 * self.volume / M_PI)
    }

    fn xa(&self) -> V3 {
        [self.x.x, self.x.y, self.x.z]
    }

    fn sq(&self) -> Sq {
        Sq::set(self.xa(), &self.quat, self.shape, self.blockiness)
    }
}

/// A primitive plane wall, `fix wall/gran ... primitive type T xplane|yplane|zplane c`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SuperquadricPlaneWall {
    /// Axis normal to the plane: 0 = x, 1 = y, 2 = z.
    pub dim: usize,
    /// Plane position on that axis `[m]`.
    pub coord: f64,
    /// Wall atom type, 0-based.
    pub wall_type: usize,
}

/// A primitive cylinder wall, `fix wall/gran ... primitive type T
/// xcylinder|ycylinder|zcylinder r c1 c2`: the axis is `axis` (0 = x, 1 = y,
/// 2 = z) and passes through `(c1, c2)` on the axes `(axis + 1) % 3` and
/// `(axis + 2) % 3` (upstream's `Dim<dim>`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SuperquadricCylinderWall {
    /// Cylinder axis: 0 = x, 1 = y, 2 = z.
    pub axis: usize,
    /// Radius `[m]`.
    pub radius: f64,
    /// Axis position `(c1, c2)` on the two other axes, in upstream's order `[m]`.
    pub center: [f64; 2],
    /// Wall atom type, 0-based.
    pub wall_type: usize,
}

/// Which curvature [`SuperquadricSystem::with_curvature_limit`] uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurvatureKind {
    /// `meanCurvature yes`.
    Mean,
    /// `gaussianCurvature yes`.
    Gaussian,
}

/// `curvatureLimitFactor` with the curvature it applies to.
#[derive(Debug, Clone, Copy, PartialEq)]
struct CurvatureLimit {
    factor: f64,
    gaussian: bool,
}

/// One wall fix of a [`SuperquadricSystem`], in the order added.
#[derive(Debug, Clone)]
enum SqWall {
    Plane(SuperquadricPlaneWall),
    Cylinder(SuperquadricCylinderWall),
    Mesh { mesh: MeshWall, wall_type: usize },
}

/// The primitive of a primitive wall, copied out of [`SqWall`].
#[derive(Debug, Clone, Copy)]
enum Primitive {
    Plane(SuperquadricPlaneWall),
    Cylinder(SuperquadricCylinderWall),
}

/// `Plane<dim>::resolveContact`: `(gap, delta)` with `delta` from the centre
/// to the plane.
fn plane_resolve_contact(wall: &SuperquadricPlaneWall, x: &V3, radi: f64) -> (f64, V3) {
    let mut delta = [0.0; 3];
    let gap = if x[wall.dim] > wall.coord {
        delta[wall.dim] = wall.coord - x[wall.dim];
        x[wall.dim] - wall.coord - radi
    } else {
        delta[wall.dim] = wall.coord - x[wall.dim];
        wall.coord - x[wall.dim] - radi
    };
    (gap, delta)
}

/// `Cylinder<dim>::resolveContact` (`primitive_wall_definitions.h`):
/// `(gap, delta)` with `delta` from the centre to the cylinder surface,
/// radially. On the axis (`compDouble(dist, 0)`), `delta = 0` and the gap is
/// 0, as upstream. [`crate::boundary::Boundary::cylinder_resolve`] is the
/// same routine for sphere [`crate::particle::Particle`]s (with `delta`
/// negated and its own axis guard), so it is not reused here.
fn cylinder_resolve_contact(wall: &SuperquadricCylinderWall, x: &V3, radi: f64) -> (f64, V3) {
    let ix = wall.axis;
    let iy = (wall.axis + 1) % 3;
    let iz = (wall.axis + 2) % 3;
    let dy = x[iy] - wall.center[0];
    let dz = x[iz] - wall.center[1];
    let dist = (dy * dy + dz * dz).sqrt();
    let mut delta = [0.0; 3];
    if comp_double(dist, 0.0, 1e-13) {
        return (0.0, delta);
    }
    let r = wall.radius;
    delta[ix] = 0.0;
    if dist > r {
        let fact = (dist - r) / dist;
        delta[iy] = -dy * fact;
        delta[iz] = -dz * fact;
        (dist - r - radi, delta)
    } else {
        let fact = (r - dist) / dist;
        delta[iy] = dy * fact;
        delta[iz] = dz * fact;
        (r - dist - radi, delta)
    }
}

/// `get_effective_radius` / `get_effective_radius_wall`: `1 / keff`, or
/// `rmax` when `keff < 1 / rmax`.
fn effective_radius_limited(keff: f64, rmax: f64) -> f64 {
    if keff < 1.0 / rmax {
        rmax
    } else {
        1.0 / keff
    }
}

/// `TriMesh::isInElement`.
fn is_in_element(nd: &[V3; 3], pos: &V3) -> bool {
    let v0 = sub3(&nd[2], &nd[0]);
    let v1 = sub3(&nd[1], &nd[0]);
    let v2 = sub3(pos, &nd[0]);
    let dot00 = dot3(&v0, &v0);
    let dot01 = dot3(&v0, &v1);
    let dot02 = dot3(&v0, &v2);
    let dot11 = dot3(&v1, &v1);
    let dot12 = dot3(&v1, &v2);
    let inv_denom = 1. / (dot00 * dot11 - dot01 * dot01);
    let u = (dot11 * dot02 - dot01 * dot12) * inv_denom;
    let v = (dot00 * dot12 - dot01 * dot02) * inv_denom;
    u > -SMALL_TRIMESH && v > -SMALL_TRIMESH && u + v < 1. + SMALL_TRIMESH
}

/// `TriMesh::superquadricTriangleIntersection`: 0 when the point of lowest
/// potential lies on the facet, 1 when a node is inside the particle, 2 when
/// an edge enters it, -1 otherwise.
fn superquadric_triangle_intersection(nd: &[V3; 3], plp: &V3, sq: &Sq) -> i32 {
    if is_in_element(nd, plp) {
        return 0;
    }
    if sq.shape_function_global(&nd[0]) < 0.0
        || sq.shape_function_global(&nd[1]) < 0.0
        || sq.shape_function_global(&nd[2]) < 0.0
    {
        return 1;
    }
    if sq.edge_intersection(&nd[0], &nd[1])
        || sq.edge_intersection(&nd[1], &nd[2])
        || sq.edge_intersection(&nd[2], &nd[0])
    {
        2
    } else {
        -1
    }
}

/// `TriMesh::resolveTriSuperquadricContact` (`tri_mesh_I_superquadric.h`):
/// `Some((deltan, delta, contact_point))` with `deltan` negative on overlap
/// and `delta` the unit direction from the particle to the facet, or `None`
/// for upstream's `LARGE_TRIMESH` (no intersection, or an inactive edge or
/// corner). The barycentric coordinates are not returned: they only weight
/// the node velocities of a moving mesh, which is not supported here.
fn resolve_tri_superquadric_contact(
    state: &TriMeshState,
    nd: &[V3; 3],
    i_tri: usize,
    sq: &Sq,
) -> Option<(f64, V3, V3)> {
    let mut surf_norm = state.surface_norm(i_tri);
    let (intersect, pmp, plp) = sq.plane_intersection(&surf_norm, &nd[0]);
    if !intersect {
        return None;
    }
    let flag = superquadric_triangle_intersection(nd, &plp, sq);
    if flag < 0 {
        return None;
    }
    let f_node = [
        sq.shape_function_global(&nd[0]),
        sq.shape_function_global(&nd[1]),
        sq.shape_function_global(&nd[2]),
    ];
    let (d_face, contact_point_face) = point_wall_projection(&surf_norm, &nd[0], &pmp);
    let dist = sub3(&sq.center, &nd[0]);
    if dot3(&dist, &surf_norm) > 0.0 {
        surf_norm = [-surf_norm[0], -surf_norm[1], -surf_norm[2]];
    }
    if d_face < 0.001 * sq.shape[0] || flag == 0 {
        return Some((-d_face, surf_norm, contact_point_face));
    }
    // particle-edge or particle-corner contact
    let (f0, c0) = sq.line_intersection(&nd[0], &nd[1]);
    let (f1, c1) = sq.line_intersection(&nd[1], &nd[2]);
    let (f2, c2) = sq.line_intersection(&nd[2], &nd[0]);
    let f_edge = [f0, f1, f2];
    let closest_points = [c0, c1, c2];
    if f_edge[0] > 0.0 && f_edge[1] > 0.0 && f_edge[2] > 0.0 {
        return Some((-d_face, surf_norm, contact_point_face));
    }
    let i_edge = if f_edge[0] <= cmin(f_edge[1], f_edge[2]) {
        0
    } else if f_edge[1] <= cmin(f_edge[0], f_edge[2]) {
        1
    } else {
        2
    };
    let ip = i_edge;
    let ipp = (i_edge + 1) % 3;
    let (edge_contact, contact_point_edge) = if f_node[ip] <= f_edge[i_edge] {
        if !state.corner_active(i_tri, ip) {
            return None;
        }
        (false, nd[ip])
    } else if f_node[ipp] <= f_edge[i_edge] {
        if !state.corner_active(i_tri, ipp) {
            return None;
        }
        (false, nd[ipp])
    } else {
        if !state.edge_active(i_tri, i_edge) {
            return None;
        }
        (true, closest_points[i_edge])
    };
    // (upstream's map_point here writes a point that
    // surface_line_intersection1 overwrites unread)
    let mut normal = sq.shape_function_gradient_global(&contact_point_edge);
    normalize3(&mut normal);
    if edge_contact {
        let mut direction = sub3(&nd[ip], &nd[ipp]);
        normalize3(&mut direction);
        let cosa = dot3(&normal, &direction);
        for i in 0..3 {
            normal[i] -= cosa * direction[i];
        }
        normalize3(&mut normal);
    }
    let d_edge = sq.surface_line_intersection1(&contact_point_edge, &normal);
    if d_edge < d_face {
        Some((-d_edge, normal, contact_point_edge))
    } else {
        Some((-d_face, surf_norm, contact_point_face))
    }
}

/// Upstream's neighbour list state for [`SuperquadricSystem::with_neighbor_list`].
#[derive(Debug, Clone)]
struct SqNeighbor {
    cfg: LiggghtsNeighbor,
    every: u64,
    check: bool,
    /// `Neighbor::ago`: steps since the last build.
    ago: u64,
    /// The half list, `(i, j)` with `i < j`, in upstream's order.
    pairs: Vec<(u32, u32)>,
    /// Positions at the last build.
    xhold: Vec<Vec3>,
}

/// Per-pair contact history of `surface superquadric` + `tangential history`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct PairHistory {
    inequality_start: f64,
    particles_were_in_contact: f64,
    contact_point: V3,
    alpha1: f64,
    alpha2: f64,
    shear: V3,
}

/// `SURFACES_FAR`, `SURFACES_CLOSE`, `SURFACES_INTERSECT`.
const SURFACES_FAR: f64 = 0.0;
const SURFACES_CLOSE: f64 = 1.0;
const SURFACES_INTERSECT: f64 = 2.0;

/// Upstream's `ForceData` (force and torque increments of one contact).
#[derive(Debug, Clone, Copy, Default)]
struct ForceData {
    f: V3,
    t: V3,
}

/// A system of superquadric particles: Hertz + tangential history contacts,
/// primitive plane walls, gravity, and `fix nve/superquadric` (scheme 1).
#[derive(Debug, Clone)]
pub struct SuperquadricSystem {
    particles: Vec<SuperquadricParticle>,
    materials: MaterialTable,
    normal: GranularNormalModel,
    walls: Vec<SqWall>,
    gravity: Option<V3>,
    curvature: Option<CurvatureLimit>,
    dt: f64,
    pair_history: Vec<PairHistory>,
    /// Tangential history per wall: one slot per particle (primitive walls)
    /// or per (facet, particle) (mesh walls, `i_tri * n + ip`).
    wall_shear: Vec<Vec<V3>>,
    is_setup: bool,
    neigh: Option<SqNeighbor>,
    step: u64,
    /// `fix insert/pack` over superquadric templates
    /// ([`SuperquadricSystem::with_insertion`]).
    inserters: Vec<crate::insert::InsertFix>,
    /// `rolling_model cdt`: `coeffRollFrict` per type pair, row-major
    /// ([`SuperquadricSystem::with_rolling_cdt`]).
    rolling_cdt: Option<Vec<f64>>,
}

impl SuperquadricSystem {
    /// A system with time step `dt`, per-type materials and Hertz with
    /// tangential damping (upstream's default).
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] for a non-positive `dt`, no particles, or a
    /// particle type outside the material table.
    pub fn new(
        particles: Vec<SuperquadricParticle>,
        materials: MaterialTable,
        dt: Time,
    ) -> Result<Self, DemError> {
        let dt = dt.get::<second>();
        if !(dt > 0.0) {
            return Err(DemError::InvalidInput("time step must be positive".into()));
        }
        if particles.is_empty() {
            return Err(DemError::InvalidInput("no particles".into()));
        }
        if particles.iter().any(|p| p.itype >= materials.n_types()) {
            return Err(DemError::InvalidInput(
                "particle type outside the material table".into(),
            ));
        }
        let n = particles.len();
        // Hertz's only switch here is tangential damping (on); the
        // coefficients come from the table per type pair.
        let normal = GranularNormalModel::hertz(GranularMaterial::new(1.0e7, 0.3, 0.5, 0.4)?);
        Ok(Self {
            particles,
            materials,
            normal,
            walls: Vec::new(),
            gravity: None,
            curvature: None,
            dt,
            pair_history: vec![PairHistory::default(); n * n],
            wall_shear: Vec::new(),
            is_setup: false,
            neigh: None,
            step: 0,
            inserters: Vec::new(),
            rolling_cdt: None,
        })
    }

    /// Add a primitive plane wall (applied in the order added, after the
    /// pair forces).
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] for `dim > 2` or a wall type outside the table.
    pub fn with_plane_wall(mut self, wall: SuperquadricPlaneWall) -> Result<Self, DemError> {
        if wall.dim > 2 || wall.wall_type >= self.materials.n_types() {
            return Err(DemError::InvalidInput("invalid plane wall".into()));
        }
        self.walls.push(SqWall::Plane(wall));
        self.wall_shear.push(vec![[0.0; 3]; self.particles.len()]);
        Ok(self)
    }

    /// Add a primitive cylinder wall, `fix wall/gran ... primitive type T
    /// xcylinder|ycylinder|zcylinder r c1 c2` (applied in the order added,
    /// after the pair forces).
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] for `axis > 2`, a non-positive radius or a
    /// wall type outside the table.
    pub fn with_cylinder_wall(mut self, wall: SuperquadricCylinderWall) -> Result<Self, DemError> {
        if wall.axis > 2 || !(wall.radius > 0.0) || wall.wall_type >= self.materials.n_types() {
            return Err(DemError::InvalidInput("invalid cylinder wall".into()));
        }
        self.walls.push(SqWall::Cylinder(wall));
        self.wall_shear.push(vec![[0.0; 3]; self.particles.len()]);
        Ok(self)
    }

    /// Add a static triangulated wall, `fix mesh/surface file F type T` with
    /// `fix wall/gran ... mesh n_meshes 1 meshes F` (applied in the order
    /// added, after the pair forces). The facets keep the order of `mesh`
    /// (the STL order, as upstream reads it).
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] for a wall type outside the table.
    pub fn with_mesh_wall(mut self, mesh: MeshWall, wall_type: usize) -> Result<Self, DemError> {
        if wall_type >= self.materials.n_types() {
            return Err(DemError::InvalidInput("invalid mesh wall type".into()));
        }
        let slots = mesh.triangles.len() * self.particles.len();
        self.walls.push(SqWall::Mesh { mesh, wall_type });
        self.wall_shear.push(vec![[0.0; 3]; slots]);
        Ok(self)
    }

    /// The curvature-limited contact radius, `surface superquadric
    /// curvatureLimitFactor f meanCurvature yes` (or `gaussianCurvature
    /// yes`): Hertz uses `1 / (k_i + k_j)` (`1 / k_i` at a wall), the
    /// curvatures at the two surface points, but at most `f` times the
    /// volume-equivalent radius. Upstream's default is off (`f = 0`).
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] for a negative or non-finite factor
    /// (upstream: "Curvature limiter cannot be negative!").
    pub fn with_curvature_limit(
        mut self,
        factor: f64,
        kind: CurvatureKind,
    ) -> Result<Self, DemError> {
        if !(factor >= 0.0) || !factor.is_finite() {
            return Err(DemError::InvalidInput(
                "Curvature limiter cannot be negative!".into(),
            ));
        }
        self.curvature = (factor > 0.0).then_some(CurvatureLimit {
            factor,
            gaussian: kind == CurvatureKind::Gaussian,
        });
        Ok(self)
    }

    /// `fix gravity magnitude vector x y z`, applied after the walls.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] for a zero direction.
    pub fn with_gravity(mut self, magnitude: f64, dir: Vec3) -> Result<Self, DemError> {
        let length = (dir.x * dir.x + dir.y * dir.y + dir.z * dir.z).sqrt();
        if length == 0.0 {
            return Err(DemError::InvalidInput(
                "Gravity direction vector = 0".into(),
            ));
        }
        self.gravity = Some([
            magnitude * (dir.x / length),
            magnitude * (dir.y / length),
            magnitude * (dir.z / length),
        ]);
        Ok(self)
    }

    /// The particles.
    #[must_use]
    pub fn particles(&self) -> &[SuperquadricParticle] {
        &self.particles
    }

    /// Steps taken.
    #[must_use]
    pub fn step_count(&self) -> u64 {
        self.step
    }

    /// Advance `n` steps (the first call runs upstream's `Verlet::setup`).
    pub fn run(&mut self, n: u64) {
        if !self.is_setup {
            if self.neigh.is_some() {
                self.rebuild_neighbor_list();
            }
            self.compute_forces(false);
            self.is_setup = true;
        }
        for _ in 0..n {
            self.initial_integrate();
            // `Neighbor::decide`: a fix's `next_reneighbor` forces the build
            // before `ago++`; `pre_exchange` (insertion) runs on every build
            let ntimestep = self.step + 1;
            if self.insertion_due(ntimestep) || self.neighbor_decide() {
                self.run_insertions(ntimestep);
                self.rebuild_neighbor_list();
            }
            self.compute_forces(true);
            self.final_integrate();
            self.step += 1;
        }
    }

    /// Use upstream's neighbour list: `neighbor <skin> bin` over the box of
    /// `cfg`, `neigh_modify delay 0 every <every> check <check>`. Pairs are
    /// then visited in `granular_bin_no_newton`'s order (the binned order
    /// that decides how several contacts of one particle are summed), and a
    /// pair's contact history lives as upstream's does: it persists between
    /// rebuilds whatever the separation, and a rebuild keeps it only for a
    /// pair whose bounding spheres overlap (`neigh_gran.cpp`; a superquadric
    /// pair's contact flag is always set once its bounding spheres have
    /// overlapped, Hertz's `surfacesClose` setting it). Without this option
    /// every pair `i < j` is visited in index order and the history is
    /// dropped as soon as the bounding spheres part, which is the same thing
    /// for `every 1 check no` and at most two particles.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] for `every = 0`, an atom sort
    /// (`cfg.sort_every != 0`: the superquadric atom sort is not ported, so
    /// the deck must say `atom_modify sort 0 0`), or a contact distance
    /// factor other than 1.
    pub fn with_neighbor_list(
        mut self,
        cfg: LiggghtsNeighbor,
        every: u64,
        check: bool,
    ) -> Result<Self, DemError> {
        if every == 0 {
            return Err(DemError::InvalidInput(
                "neigh_modify every must be positive".into(),
            ));
        }
        if cfg.sort_every != 0 {
            return Err(DemError::InvalidInput(
                "superquadric neighbour lists need the atom sort off (atom_modify sort 0 0)".into(),
            ));
        }
        if cfg.contact_distance_factor != 1.0 {
            return Err(DemError::InvalidInput(
                "superquadric neighbour lists support a contact distance factor of 1 only".into(),
            ));
        }
        self.neigh = Some(SqNeighbor {
            cfg,
            every,
            check,
            ago: 0,
            pairs: Vec::new(),
            xhold: Vec::new(),
        });
        Ok(self)
    }

    /// `Neighbor::decide` with `delay 0` and no fix-requested rebuilds.
    fn neighbor_decide(&mut self) -> bool {
        let Some(nb) = self.neigh.as_mut() else {
            return false;
        };
        nb.ago += 1;
        if nb.ago % nb.every != 0 {
            return false;
        }
        if !nb.check {
            return true;
        }
        let x: Vec<Vec3> = self.particles.iter().map(|p| p.x).collect();
        neighbor_list::needs_rebuild(&x, &nb.xhold, nb.cfg.skin)
    }

    /// A neighbour rebuild: the history carry-over of `granular_bin_no_newton`
    /// (kept only where the bounding spheres overlap), then the half list on
    /// the bounding radii, `xhold`, and `ago = 0`.
    fn rebuild_neighbor_list(&mut self) {
        let n = self.particles.len();
        for i in 0..n {
            for j in (i + 1)..n {
                let (a, b) = (&self.particles[i], &self.particles[j]);
                let delx = a.x.x - b.x.x;
                let dely = a.x.y - b.x.y;
                let delz = a.x.z - b.x.z;
                let rsq = delx * delx + dely * dely + delz * delz;
                let radsum = a.radius + b.radius;
                if !(rsq < radsum * radsum) {
                    self.pair_history[i * n + j] = PairHistory::default();
                }
            }
        }
        let x: Vec<Vec3> = self.particles.iter().map(|p| p.x).collect();
        let r: Vec<f64> = self.particles.iter().map(|p| p.radius).collect();
        let max_radius = r.iter().fold(0.0_f64, |m, &v| m.max(v));
        // the cut includes the largest body an insertion fix can still add
        // (`PairGran::init_style`: `max_rad(type)` of fixes)
        let max_radius = self
            .inserters
            .iter()
            .fold(max_radius, |m, f| m.max(f.max_rad()));
        let Some(nb) = self.neigh.as_mut() else {
            return;
        };
        let cut = nb.cfg.cutneighmax(max_radius);
        neighbor_list::half_bin_no_newton(&x, &r, &nb.cfg, cut, &mut nb.pairs);
        nb.xhold = x;
        nb.ago = 0;
    }

    /// `force_clear`, the pair pass, the walls, then gravity.
    fn compute_forces(&mut self, shearupdate: bool) {
        for p in &mut self.particles {
            p.force = [0.0; 3];
            p.torque = [0.0; 3];
        }
        if let Some(nb) = &self.neigh {
            let pairs = nb.pairs.clone();
            for (i, j) in pairs {
                self.pair(i as usize, j as usize, shearupdate);
            }
        } else {
            let n = self.particles.len();
            for i in 0..n {
                for j in (i + 1)..n {
                    self.pair(i, j, shearupdate);
                }
            }
        }
        for w in 0..self.walls.len() {
            match self.walls[w] {
                SqWall::Mesh { .. } => self.mesh_wall(w, shearupdate),
                SqWall::Plane(_) | SqWall::Cylinder(_) => self.primitive_wall(w, shearupdate),
            }
        }
        if let Some(acc) = self.gravity {
            for p in &mut self.particles {
                let massone = p.mass;
                p.force[0] += massone * acc[0];
                p.force[1] += massone * acc[1];
                p.force[2] += massone * acc[2];
            }
        }
    }

    /// One pair of `PairGran::compute` with `surface superquadric`.
    fn pair(&mut self, i: usize, j: usize, shearupdate: bool) {
        let n = self.particles.len();
        let pi = self.particles[i];
        let pj = self.particles[j];
        let xi = pi.xa();
        let xj = pj.xa();
        let delx = xi[0] - xj[0];
        let dely = xi[1] - xj[1];
        let delz = xi[2] - xj[2];
        let rsq = delx * delx + dely * dely + delz * delz;
        let radsum = pi.radius + pj.radius;
        let hist = &mut self.pair_history[i * n + j];
        if !(rsq < radsum * radsum) {
            // Without a neighbour list (rebuild every step): the history of a
            // pair whose bounding spheres do not overlap is dropped. With one,
            // upstream calls nothing here and the rebuild decides.
            if self.neigh.is_none() {
                *hist = PairHistory::default();
            }
            return;
        }
        let mut sq_i = pi.sq();
        let mut sq_j = pj.sq();
        let v_i = [pi.v.x, pi.v.y, pi.v.z];
        let v_j = [pj.v.x, pj.v.y, pj.v.z];
        let om_i = [pi.omega.x, pi.omega.y, pi.omega.z];
        let om_j = [pj.omega.x, pj.omega.y, pj.omega.z];
        let mi = pi.mass;
        let mj = pj.mass;

        // SurfaceModel<SURFACE_SUPERQUADRIC>::checkSurfaceIntersect
        let mut int_inequality_start = (hist.inequality_start + 0.5) as u32;
        let obb = if hist.particles_were_in_contact == SURFACES_INTERSECT {
            true
        } else {
            obb_intersect(&sq_i, &sq_j, &mut int_inequality_start)
        };
        let mut contact_point = [0.0; 3];
        let mut in_contact = false;
        let mut en = [0.0; 3];
        let mut deltan = 0.0;
        let mut reff = 0.0;
        if obb {
            let ri = m_cbrt(sq_i.shape[0] * sq_i.shape[1] * sq_i.shape[2]);
            let rj = m_cbrt(sq_j.shape[0] * sq_j.shape[1] * sq_j.shape[2]);
            let ratio = ri / (ri + rj);
            let mut fi = 0.0;
            let mut fj = 0.0;
            if hist.particles_were_in_contact == SURFACES_FAR {
                calc_contact_point_no_prev(
                    &mut sq_i,
                    &mut sq_j,
                    &mut contact_point,
                    &mut fi,
                    &mut fj,
                );
            } else {
                // calc_contact_point_using_prev_step
                let prev = hist.contact_point;
                let xci = sub3(&prev, &sq_i.center);
                let xcj = sub3(&prev, &sq_j.center);
                let v_rot_i = cross3(&om_i, &xci);
                let v_rot_j = cross3(&om_j, &xcj);
                let mut vi = [0.0; 3];
                let mut vj = [0.0; 3];
                for k in 0..3 {
                    vi[k] = v_i[k] + v_rot_i[k];
                    vj[k] = v_j[k] + v_rot_j[k];
                }
                let mut estimation = [0.0; 3];
                for k in 0..3 {
                    let v_eff = (vi[k] * mi + vj[k] * mj) / (mi + mj);
                    estimation[k] = prev[k] + v_eff * self.dt;
                }
                calc_contact_point(
                    &mut sq_i,
                    &mut sq_j,
                    ratio,
                    &estimation,
                    &mut contact_point,
                    &mut fi,
                    &mut fj,
                );
            }
            hist.contact_point = contact_point;
            in_contact = cmax(fi, fj) < 0.0;
            if in_contact {
                en = sub3(&sq_j.gradient, &sq_i.gradient);
                normalize3(&mut en);
                // extended_overlap_algorithm
                let neg = [-en[0], -en[1], -en[2]];
                let (a_i, cp_i) = sq_i.surface_line_intersection(&contact_point, &neg, hist.alpha1);
                hist.alpha1 = a_i;
                let (a_j, cp_j) = sq_j.surface_line_intersection(&contact_point, &en, hist.alpha2);
                hist.alpha2 = a_j;
                deltan = a_i.abs() + a_j.abs();
                let radi = pi.equivalent_radius();
                let radj = pj.equivalent_radius();
                reff = radi * radj / (radi + radj);
                if let Some(curv) = self.curvature {
                    // get_effective_radius
                    let koefi = sq_i.calc_curvature_coefficient(curv.gaussian, &cp_i);
                    let koefj = sq_j.calc_curvature_coefficient(curv.gaussian, &cp_j);
                    reff = effective_radius_limited(koefi + koefj, reff * curv.factor);
                }
                hist.particles_were_in_contact = SURFACES_INTERSECT;
            } else {
                hist.particles_were_in_contact = SURFACES_CLOSE;
            }
        } else {
            hist.particles_were_in_contact = SURFACES_FAR;
        }
        hist.inequality_start = f64::from(int_inequality_start);

        if !in_contact {
            // surfacesClose: the tangential model drops its spring
            hist.shear = [0.0; 3];
            return;
        }
        let meff = mi * mj / (mi + mj);
        // surfacesIntersectNonSpherical (pair)
        let xci = sub3(&contact_point, &xi);
        let xcj = sub3(&contact_point, &xj);
        let v_rot_i = cross3(&om_i, &xci);
        let v_rot_j = cross3(&om_j, &xcj);
        let v_rot_rel = sub3(&v_rot_i, &v_rot_j);
        let v_rel = [
            v_i[0] - v_j[0] + v_rot_rel[0],
            v_i[1] - v_j[1] + v_rot_rel[1],
            v_i[2] - v_j[2] + v_rot_rel[2],
        ];
        let props = self.materials.pair_props(pi.itype, pj.itype);
        let (fi_data, fj_data) = contact_forces(
            &self.normal,
            &props,
            &en,
            deltan,
            reff,
            meff,
            &v_rel,
            &xci,
            Some(&xcj),
            &mut hist.shear,
            shearupdate,
            self.dt,
            cdt_coeff(self.rolling_cdt.as_deref(), self.materials.n_types(), pi.itype, pj.itype).map(|rmu| (rmu, sub3(&om_i, &om_j))),
        );
        let fj_data = fj_data.unwrap_or_default();
        let p = &mut self.particles[i];
        for k in 0..3 {
            p.force[k] += 1.0 * fi_data.f[k];
            p.torque[k] += 1.0 * fi_data.t[k];
        }
        let p = &mut self.particles[j];
        for k in 0..3 {
            p.force[k] += 1.0 * fj_data.f[k];
            p.torque[k] += 1.0 * fj_data.t[k];
        }
    }

    /// `FixWallGran::post_force_primitive` for one plane or cylinder wall,
    /// superquadric branch: the bounding sphere is resolved against the
    /// primitive, then the particle is cut by the tangent plane through
    /// `x + delta` (for a cylinder too: upstream has no cylinder-specific
    /// superquadric code).
    fn primitive_wall(&mut self, w: usize, shearupdate: bool) {
        let (wall_type, primitive) = match &self.walls[w] {
            SqWall::Plane(p) => (p.wall_type, Primitive::Plane(*p)),
            SqWall::Cylinder(c) => (c.wall_type, Primitive::Cylinder(*c)),
            SqWall::Mesh { .. } => return,
        };
        let n = self.particles.len();
        for ip in 0..n {
            let p = self.particles[ip];
            let x = p.xa();
            let radi = p.radius;
            let (mut gap, delta) = match primitive {
                Primitive::Plane(wall) => plane_resolve_contact(&wall, &x, radi),
                Primitive::Cylinder(wall) => cylinder_resolve_contact(&wall, &x, radi),
            };
            if !(gap <= 0.0) {
                self.wall_shear[w][ip] = [0.0; 3];
                continue;
            }
            let sphere_contact_point = [x[0] + delta[0], x[1] + delta[1], x[2] + delta[2]];
            let sq = p.sq();
            let (intersect, closest, _) = sq.plane_intersection(&delta, &sphere_contact_point);
            let (proj, _) = point_wall_projection(&delta, &sphere_contact_point, &closest);
            gap = -proj;
            if gap > 0.0 {
                self.wall_shear[w][ip] = [0.0; 3];
                break;
            }
            if !intersect {
                // surfacesClose
                self.wall_shear[w][ip] = [0.0; 3];
                continue;
            }
            self.wall_contact(w, ip, ip, wall_type, -gap, &delta, &closest, shearupdate);
        }
    }

    /// `FixWallGran::post_force_mesh` for one mesh wall, superquadric branch:
    /// facets in order, the particles passing each facet's bounding-sphere
    /// test, `resolveTriSuperquadricContact`, and a per (particle, facet)
    /// history that lives while `deltan <= 0` (`handleContact(...,
    /// faceflag = false)`, so no coplanar copy).
    fn mesh_wall(&mut self, w: usize, shearupdate: bool) {
        let SqWall::Mesh { mesh, wall_type } = &self.walls[w] else {
            return;
        };
        let wall_type = *wall_type;
        let nodes = mesh.nodes();
        let n = self.particles.len();
        for (i_tri, nd) in nodes.iter().enumerate() {
            for ip in 0..n {
                let p = self.particles[ip];
                let x = p.xa();
                let SqWall::Mesh { mesh, .. } = &self.walls[w] else {
                    return;
                };
                // sphereTriangleIntersection: pointToTriangleDistance with
                // treatActiveFlag = false, against the bounding radius
                let (d, _, _, _) = mesh
                    .state
                    .resolve_tri_sphere_contact_bary(nd, i_tri, 0.0, &x, false);
                let mut resolved = None;
                if d < p.radius {
                    let sq = p.sq();
                    resolved = resolve_tri_superquadric_contact(&mesh.state, nd, i_tri, &sq);
                }
                let slot = i_tri * n + ip;
                match resolved {
                    Some((deltan, delta, contact_point)) if deltan <= 0.0 => {
                        self.wall_contact(
                            w,
                            slot,
                            ip,
                            wall_type,
                            -deltan,
                            &delta,
                            &contact_point,
                            shearupdate,
                        );
                    }
                    _ => self.wall_shear[w][slot] = [0.0; 3],
                }
            }
        }
    }

    /// `FixWallGranBase::compute_force` (superquadric branch) on particle
    /// `ip`, with history slot `slot` of wall `w`: `delta` is upstream's
    /// wall-side `delta`, `sidata_deltan` the overlap, `contact_point` where
    /// both forces act.
    #[allow(clippy::too_many_arguments)]
    fn wall_contact(
        &mut self,
        w: usize,
        slot: usize,
        ip: usize,
        wall_type: usize,
        sidata_deltan: f64,
        delta: &V3,
        contact_point: &V3,
        shearupdate: bool,
    ) {
        let p = self.particles[ip];
        let x = p.xa();
        let sidata_delta = [-delta[0], -delta[1], -delta[2]];
        let delta_inv = 1.0 / mag3(&sidata_delta);
        let en = [
            sidata_delta[0] * delta_inv,
            sidata_delta[1] * delta_inv,
            sidata_delta[2] * delta_inv,
        ];
        let radi = p.equivalent_radius();
        let mut reff = radi;
        if let Some(curv) = self.curvature {
            // SurfaceModel<SURFACE_SUPERQUADRIC>::surfacesIntersect, wall:
            // get_effective_radius_wall
            let koefi = p
                .sq()
                .calc_curvature_coefficient(curv.gaussian, contact_point);
            reff = effective_radius_limited(koefi, radi * curv.factor);
        }
        let xci = sub3(contact_point, &x);
        let om = [p.omega.x, p.omega.y, p.omega.z];
        let v_rot = cross3(&om, &xci);
        let v_wall = [0.0; 3];
        let v_rel = [
            p.v.x - v_wall[0] + v_rot[0],
            p.v.y - v_wall[1] + v_rot[1],
            p.v.z - v_wall[2] + v_rot[2],
        ];
        let props = self.materials.pair_props(p.itype, wall_type);
        let (fd, _) = contact_forces(
            &self.normal,
            &props,
            &en,
            sidata_deltan,
            reff,
            p.mass,
            &v_rel,
            &xci,
            None,
            &mut self.wall_shear[w][slot],
            shearupdate,
            self.dt,
            cdt_coeff(self.rolling_cdt.as_deref(), self.materials.n_types(), p.itype, wall_type).map(|rmu| (rmu, om)),
        );
        let p = &mut self.particles[ip];
        for k in 0..3 {
            p.force[k] += fd.f[k];
            p.torque[k] += fd.t[k];
        }
    }

    /// `FixNVEAsphereBase::initial_integrate`, scheme 1.
    fn initial_integrate(&mut self) {
        let dtv = self.dt;
        let dtf = 0.5 * self.dt * 1.0;
        let dtf2 = 2.0 * dtf;
        let dtq = 0.5 * dtv;
        for p in &mut self.particles {
            let mut rot = quat_to_mat(&p.quat);
            let dtfm = dtf / p.mass;
            p.v.x += dtfm * p.force[0];
            p.v.y += dtfm * p.force[1];
            p.v.z += dtfm * p.force[2];
            p.x.x += dtv * p.v.x;
            p.x.y += dtv * p.v.y;
            p.x.z += dtv * p.v.z;
            let angmom = [p.angmom.x, p.angmom.y, p.angmom.z];
            let mbody = transpose_matvec(&rot, &angmom);
            let mut conjqm = calc_conjqm(&p.quat, &mbody);
            let tbody = transpose_matvec(&rot, &p.torque);
            let fquat = quatvec(&p.quat, &tbody);
            for k in 0..4 {
                conjqm[k] += dtf2 * fquat[k];
            }
            let inertia = p.inertia;
            no_squish_rotate(3, &mut conjqm, &mut p.quat, &inertia, dtq);
            no_squish_rotate(2, &mut conjqm, &mut p.quat, &inertia, dtq);
            no_squish_rotate(1, &mut conjqm, &mut p.quat, &inertia, dtv);
            no_squish_rotate(2, &mut conjqm, &mut p.quat, &inertia, dtq);
            no_squish_rotate(3, &mut conjqm, &mut p.quat, &inertia, dtq);
            crate::multisphere::qnormalize(&mut p.quat);
            rot = quat_to_mat(&p.quat);
            let mut mbody = invquatvec(&p.quat, &conjqm);
            mbody[0] *= 0.5;
            mbody[1] *= 0.5;
            mbody[2] *= 0.5;
            let wbody = [
                mbody[0] / inertia[0],
                mbody[1] / inertia[1],
                mbody[2] / inertia[2],
            ];
            let l = matvec(&rot, &mbody);
            let w = matvec(&rot, &wbody);
            p.angmom = Vec3::new(l[0], l[1], l[2]);
            p.omega = Vec3::new(w[0], w[1], w[2]);
        }
    }

    /// `FixNVEAsphereBase::final_integrate`, scheme 1.
    fn final_integrate(&mut self) {
        let dtf = 0.5 * self.dt * 1.0;
        let dtf2 = dtf * 2.0;
        for p in &mut self.particles {
            let dtfm = dtf / p.mass;
            let rot = quat_to_mat(&p.quat);
            p.v.x += dtfm * p.force[0];
            p.v.y += dtfm * p.force[1];
            p.v.z += dtfm * p.force[2];
            let tbody = transpose_matvec(&rot, &p.torque);
            let angmom = [p.angmom.x, p.angmom.y, p.angmom.z];
            let mbody = transpose_matvec(&rot, &angmom);
            let mut conjqm = calc_conjqm(&p.quat, &mbody);
            let fquat = quatvec(&p.quat, &tbody);
            for k in 0..4 {
                conjqm[k] += dtf2 * fquat[k];
            }
            let mut mbody = invquatvec(&p.quat, &conjqm);
            mbody[0] *= 0.5;
            mbody[1] *= 0.5;
            mbody[2] *= 0.5;
            let inertia = p.inertia;
            let wbody = [
                mbody[0] / inertia[0],
                mbody[1] / inertia[1],
                mbody[2] / inertia[2],
            ];
            let l = matvec(&rot, &mbody);
            let w = matvec(&rot, &wbody);
            p.angmom = Vec3::new(l[0], l[1], l[2]);
            p.omega = Vec3::new(w[0], w[1], w[2]);
        }
    }
}

/// Hertz (`normal_model_hertz.h`) and tangential history
/// (`tangential_model_history.h`) at a non-spherical contact: both forces act
/// at the contact point. Returns the increments on `i` and, for a pair
/// (`xcj` given), on `j`.
#[allow(clippy::too_many_arguments)]
fn contact_forces(
    normal: &GranularNormalModel,
    props: &crate::granular::PairProps,
    en: &V3,
    deltan: f64,
    reff: f64,
    meff: f64,
    v_rel: &V3,
    xci: &V3,
    xcj: Option<&V3>,
    shear: &mut V3,
    shearupdate: bool,
    dt: f64,
    rolling: Option<(f64, V3)>,
) -> (ForceData, Option<ForceData>) {
    let vn = dot3(v_rel, en);
    let vtr = [
        v_rel[0] - vn * en[0],
        v_rel[1] - vn * en[1],
        v_rel[2] - vn * en[2],
    ];
    let k = ContactKinematics {
        en: Vec3::new(en[0], en[1], en[2]),
        delta_n: deltan,
        vn,
        vtr: Vec3::new(vtr[0], vtr[1], vtr[2]),
        cri: 0.0,
        crj: 0.0,
        r_eff: reff,
        m_eff: meff,
        omega_i: Vec3::zero(),
        omega_j: Vec3::zero(),
        is_wall: xcj.is_none(),
        itype: 0,
        jtype: 0,
        wr: Vec3::zero(),
        r: 0.0,
        rad_i: 0.0,
        rad_j: 0.0,
        mass_i: 0.0,
        mass_j: 0.0,
        liquid_i: 0.0,
        liquid_j: 0.0,
        wall_liquid_vol1000: 0.0,
    };
    let out = normal.evaluate_with(&k, props);
    let fn_ = out.fn_scalar;
    let kt = out.kt;
    let mut fi = ForceData::default();
    let mut fj = ForceData::default();
    // normal force and its torque
    let fn_i = [fn_ * en[0], fn_ * en[1], fn_ * en[2]];
    let torque_ni = cross3(xci, &fn_i);
    match xcj {
        None => {
            let fn_w = fn_ * 1.0;
            for c in 0..3 {
                fi.f[c] += fn_w * en[c];
            }
            for c in 0..3 {
                fi.t[c] += torque_ni[c];
            }
        }
        Some(xcj) => {
            for c in 0..3 {
                fi.f[c] += fn_ * en[c];
            }
            for c in 0..3 {
                fj.f[c] += -fi.f[c];
            }
            let fn_j = [-fn_i[0], -fn_i[1], -fn_i[2]];
            let torque_nj = cross3(xcj, &fn_j);
            for c in 0..3 {
                fi.t[c] += torque_ni[c];
            }
            for c in 0..3 {
                fj.t[c] += torque_nj[c];
            }
        }
    }
    // tangential history
    if shearupdate {
        shear[0] += vtr[0] * dt;
        shear[1] += vtr[1] * dt;
        shear[2] += vtr[2] * dt;
        let rsht = shear[0] * en[0] + shear[1] * en[1] + shear[2] * en[2];
        shear[0] -= rsht * en[0];
        shear[1] -= rsht * en[1];
        shear[2] -= rsht * en[2];
    }
    let shrmag = (shear[0] * shear[0] + shear[1] * shear[1] + shear[2] * shear[2]).sqrt();
    let xmu = props.friction;
    let mut ft = [-(kt * shear[0]), -(kt * shear[1]), -(kt * shear[2])];
    let ft_shear = kt * shrmag;
    let ft_friction = xmu * fn_.abs();
    if ft_shear > ft_friction {
        if shrmag != 0.0 {
            let ratio = ft_friction / ft_shear;
            ft[0] *= ratio;
            ft[1] *= ratio;
            ft[2] *= ratio;
            if shearupdate {
                shear[0] = -ft[0] / kt;
                shear[1] = -ft[1] / kt;
                shear[2] = -ft[2] / kt;
            }
        } else {
            ft = [0.0; 3];
        }
    } else {
        let gammat = out.gammat;
        ft[0] -= gammat * vtr[0];
        ft[1] -= gammat * vtr[1];
        ft[2] -= gammat * vtr[2];
    }
    let torque_ti = cross3(xci, &ft);
    // `rolling_model cdt` (after the tangential model): `i` loses the torque,
    // `j` gains it
    let r_torque = rolling.map(|(rmu, wr)| {
        cdt_rolling_torque(rmu, out.kn, deltan, reff, en, &wr, xcj.is_none())
    });
    match xcj {
        None => {
            for c in 0..3 {
                fi.f[c] += ft[c] * 1.0;
            }
            for c in 0..3 {
                fi.t[c] += torque_ti[c] * 1.0;
            }
            if let Some(r) = r_torque {
                for c in 0..3 {
                    fi.t[c] -= r[c];
                }
            }
            (fi, None)
        }
        Some(xcj) => {
            let ft_j = [-ft[0], -ft[1], -ft[2]];
            let torque_tj = cross3(xcj, &ft_j);
            for c in 0..3 {
                fi.f[c] += ft[c];
                fj.f[c] += -ft[c];
            }
            for c in 0..3 {
                fi.t[c] += torque_ti[c];
                fj.t[c] += torque_tj[c];
            }
            if let Some(r) = r_torque {
                for c in 0..3 {
                    fi.t[c] -= r[c];
                }
                for c in 0..3 {
                    fj.t[c] += r[c];
                }
            }
            (fi, Some(fj))
        }
    }
}

// ---------------------------------------------------------------------------
// Insertion: fix particletemplate/superquadric, ParticleToInsertSuperquadric,
// RegionNeighborList::hasOverlap_superquadric (gh:#1415, third pass).
// ---------------------------------------------------------------------------

/// `fix particletemplate/superquadric SEED atom_type T density constant RHO
/// shape constant a b c blockiness constant n1 n2`.
///
/// Only the `constant` distributions are ported. They draw nothing from the
/// template's `random_insertion` stream (`rand_value<RANDOM_CONSTANT>`), so
/// every entry of `randomize_ptilist` is the same body; the stream is kept for
/// upstream's distinct-seeds rule.
#[derive(Debug, Clone, PartialEq)]
pub struct ParticleTemplateSuperquadric {
    /// Seed of the template's `random_insertion` stream.
    pub seed: i32,
    /// `atom_type` (1-based, as in the deck).
    pub atom_type: usize,
    shape: V3,
    blockiness: [f64; 2],
    density: f64,
    random_insertion: crate::insert::RanPark,
}

impl ParticleTemplateSuperquadric {
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] for what upstream rejects: an invalid seed
    /// (see [`crate::insert::RanPark::checked`]), `atom_type < 1`, a
    /// non-positive half-axis or density, or a blockiness below 2.
    pub fn new(
        seed: i32,
        atom_type: usize,
        shape: [Length; 3],
        blockiness: [f64; 2],
        density: MassDensity,
    ) -> Result<Self, DemError> {
        let shape = [
            shape[0].get::<meter>(),
            shape[1].get::<meter>(),
            shape[2].get::<meter>(),
        ];
        let density = density.get::<kilogram_per_cubic_meter>();
        if atom_type < 1 {
            return Err(DemError::InvalidInput(
                "invalid atom type (must be >=1)".into(),
            ));
        }
        if shape.iter().any(|s| !(*s > 0.0)) {
            return Err(DemError::InvalidInput(
                "Illegal fix particletemplate/superquadric command, shape parameters must be >= 0".into(),
            ));
        }
        if blockiness[0] < 2.0 || blockiness[1] < 2.0 {
            return Err(DemError::InvalidInput(
                "Illegal fix particletemplate/superquadric command, blockiness parameters must >= 2"
                    .into(),
            ));
        }
        if !(density > 0.0) {
            return Err(DemError::InvalidInput("density must be >= 0".into()));
        }
        Ok(Self {
            seed,
            atom_type,
            shape,
            blockiness,
            density,
            random_insertion: crate::insert::RanPark::checked(seed)?,
        })
    }

    /// `volume_expect`: `volume_superquadric` of the expected shape `[m^3]`.
    #[must_use]
    pub fn volexpect(&self) -> f64 {
        volume_superquadric(&self.shape, &self.blockiness)
    }

    /// `mass_expect = E[density] * volume_expect` `[kg]`.
    #[must_use]
    pub fn massexpect(&self) -> f64 {
        self.density * self.volexpect()
    }

    /// `max_rad()` = `max_r_bound()`: the bounding-sphere radius `[m]`.
    #[must_use]
    pub fn r_bound(&self) -> f64 {
        bounding_sphere_radius(&self.shape, &self.blockiness)
    }

    pub(crate) fn seed_state(&self) -> i32 {
        self.random_insertion.state()
    }

    /// One entry of `randomize_ptilist`: bounding radius, density, volume,
    /// `mass = density * volume` and principal moments, in upstream's order.
    pub(crate) fn to_insert(&self) -> SuperquadricToInsert {
        let radius = bounding_sphere_radius(&self.shape, &self.blockiness);
        let density = self.density;
        let volume = volume_superquadric(&self.shape, &self.blockiness);
        let mass = density * volume;
        let inertia = inertia_superquadric(&self.shape, &self.blockiness, density);
        SuperquadricToInsert {
            shape: self.shape,
            blockiness: self.blockiness,
            radius,
            density,
            volume,
            mass,
            inertia,
            atom_type: self.atom_type,
            x: [0.0; 3],
            v: Vec3::zero(),
            omega: Vec3::zero(),
            quat: [1.0, 0.0, 0.0, 0.0],
        }
    }
}

/// `ParticleToInsertSuperquadric`: one superquadric to insert, placed by
/// `fix insert/pack`.
#[derive(Debug, Clone, PartialEq)]
pub struct SuperquadricToInsert {
    shape: V3,
    blockiness: [f64; 2],
    /// Bounding-sphere radius (`radius_ins[0] = r_bound_ins`) `[m]`.
    pub(crate) radius: f64,
    density: f64,
    volume: f64,
    /// `mass_ins` `[kg]`.
    pub(crate) mass: f64,
    inertia: V3,
    atom_type: usize,
    x: V3,
    v: Vec3,
    omega: Vec3,
    quat: [f64; 4],
}

impl SuperquadricToInsert {
    /// `set_x_v_omega` (no overlap check).
    pub(crate) fn set_x_v_omega(&mut self, x: Vec3, v: Vec3, omega: Vec3, quat: [f64; 4]) {
        self.x = [x.x, x.y, x.z];
        self.v = v;
        self.omega = omega;
        self.quat = quat;
    }

    /// `check_near_set_x_v_omega`: refuse `x` if it overlaps a body of
    /// `list` (`hasOverlap_superquadric`), else place it there and add it to
    /// `list`.
    pub(crate) fn check_near_set_x_v_omega(
        &mut self,
        x: Vec3,
        v: Vec3,
        omega: Vec3,
        quat: [f64; 4],
        list: &mut SqOverlapList,
    ) -> bool {
        let body = SqOverlapBody {
            x: [x.x, x.y, x.z],
            radius: self.radius,
            quat,
            shape: self.shape,
            blockiness: self.blockiness,
        };
        if list.has_overlap(&body) {
            return false;
        }
        self.set_x_v_omega(x, v, omega, quat);
        list.insert(body);
        true
    }

    /// `ParticleToInsertSuperquadric::insert`: the new atom, with the
    /// quaternion copied as it is (not normalised) and `angmom` from
    /// `omega_to_angmom`.
    #[must_use]
    pub fn into_particle(self) -> SuperquadricParticle {
        let w = [self.omega.x, self.omega.y, self.omega.z];
        let l = omega_to_angmom(&self.quat, &w, &self.inertia);
        SuperquadricParticle {
            x: Vec3::new(self.x[0], self.x[1], self.x[2]),
            v: self.v,
            omega: self.omega,
            angmom: Vec3::new(l[0], l[1], l[2]),
            quat: self.quat,
            shape: self.shape,
            blockiness: self.blockiness,
            radius: self.radius,
            volume: self.volume,
            mass: self.mass,
            inertia: self.inertia,
            itype: self.atom_type - 1,
            force: [0.0; 3],
            torque: [0.0; 3],
        }
    }

    /// The density `[kg/m^3]` (`density_ins`).
    #[must_use]
    pub fn density(&self) -> f64 {
        self.density
    }
}

/// `MathExtraLiggghts::random_unit_quat`: three draws from `random`, then
/// `sqrt`, `sin` and `cos` (PETIR's `RealMath`, gh:#1455). Not normalised.
pub(crate) fn random_unit_quat(random: &mut crate::insert::RanPark) -> [f64; 4] {
    let u1 = random.uniform();
    let u2 = random.uniform();
    let u3 = random.uniform();
    let h1 = (1.0 - u1).sqrt();
    let h2 = u1.sqrt();
    [
        h1 * m_sin(2.0 * M_PI * u2),
        h1 * m_cos(2.0 * M_PI * u2),
        h2 * m_sin(2.0 * M_PI * u3),
        h2 * m_cos(2.0 * M_PI * u3),
    ]
}

/// `MathExtraLiggghtsNonspherical::line_segments_distance`: the closest
/// points `C1` on `P1Q1` and `C2` on `P2Q2`.
fn line_segments_distance(p1: &V3, q1: &V3, p2: &V3, q2: &V3) -> (V3, V3) {
    let tol = 1e-14;
    let d1 = sub3(q1, p1);
    let d2 = sub3(q2, p2);
    let r = sub3(p1, p2);
    let a = dot3(&d1, &d1);
    let e = dot3(&d2, &d2);
    let f = dot3(&d2, &r);
    let (s, t);
    if a <= tol && e <= tol {
        return (*p1, *p2);
    }
    if a <= tol {
        s = 0.0;
        t = clamp(f / e, 0.0, 1.0);
    } else {
        let c = dot3(&d1, &r);
        if e <= tol {
            t = 0.0;
            s = clamp(-c / a, 0.0, 1.0);
        } else {
            let b = dot3(&d1, &d2);
            let determinant = a * e - b * b;
            if determinant >= tol * tol {
                let mut s0 = clamp((b * f - c * e) / determinant, 0.0, 1.0);
                let mut t0 = (b * s0 + f) / e;
                if t0 < 0.0 {
                    t0 = 0.0;
                    s0 = clamp(-c / a, 0.0, 1.0);
                } else if t0 > 1.0 {
                    t0 = 1.0;
                    s0 = clamp((b - c) / a, 0.0, 1.0);
                }
                s = s0;
                t = t0;
            } else {
                let mut s1 = 0.0;
                let mut s2 = 1.0;
                let mut t1 = (f + s1 * b) / e;
                let mut t2 = (f + s2 * b) / e;
                if t1 < 0.0 {
                    t1 = 0.0;
                    s1 = clamp(-c / a, 0.0, 1.0);
                } else if t1 > 1.0 {
                    t1 = 1.0;
                    s1 = clamp((b - c) / a, 0.0, 1.0);
                }
                if t2 < 0.0 {
                    t2 = 0.0;
                    s2 = clamp(-c / a, 0.0, 1.0);
                } else if t2 > 1.0 {
                    t2 = 1.0;
                    s2 = clamp((b - c) / a, 0.0, 1.0);
                }
                s = 0.5 * (s1 + s2);
                t = 0.5 * (t1 + t2);
            }
        }
    }
    (
        [p1[0] + d1[0] * s, p1[1] + d1[1] * s, p1[2] + d1[2] * s],
        [p2[0] + d2[0] * t, p2[1] + d2[1] * t, p2[2] + d2[2] * t],
    )
}

impl Sq {
    /// `Superquadric::isCylinder`, as `set` computes it.
    fn is_cylinder(&self) -> bool {
        !comp_double(self.blockiness[0], 2.0, 1e-2) && comp_double(self.blockiness[1], 2.0, 1e-2)
    }
}

/// `MathExtraLiggghtsNonspherical::capsules_intersect`: whether the capsules
/// around the two main axes intersect (the capsule contact point it also
/// computes is unused by the insertion test).
fn capsules_intersect(pa: &Sq, pb: &Sq) -> bool {
    let main_axis = |p: &Sq| -> usize {
        if p.is_cylinder() {
            2
        } else if p.shape[0] >= cmax(p.shape[1], p.shape[2]) {
            0
        } else if p.shape[1] >= cmax(p.shape[0], p.shape[2]) {
            1
        } else {
            2
        }
    };
    let main_axis_i = main_axis(pa);
    let main_axis_j = main_axis(pb);
    let l_i = pa.shape[main_axis_i];
    let l_j = pb.shape[main_axis_j];
    let r1_i = pa.shape[(main_axis_i + 1) % 3];
    let r2_i = pa.shape[(main_axis_i + 2) % 3];
    let r1_j = pb.shape[(main_axis_j + 1) % 3];
    let r2_j = pb.shape[(main_axis_j + 2) % 3];
    let mut r_i = cmax(r1_i, r2_i);
    let mut r_j = cmax(r1_j, r2_j);
    if !pa.is_cylinder() {
        r_i = (r1_i * r1_i + r2_i * r2_i).sqrt();
    }
    if !pb.is_cylinder() {
        r_j = (r1_j * r1_j + r2_j * r2_j).sqrt();
    }
    let radsum = r_i + r_j;
    let mut pi = [0.0; 3];
    let mut qi = [0.0; 3];
    let mut pj = [0.0; 3];
    let mut qj = [0.0; 3];
    for k in 0..3 {
        pi[k] = -pa.rotation_matrix[k * 3 + main_axis_i] * l_i + pa.center[k];
        qi[k] = pa.rotation_matrix[k * 3 + main_axis_i] * l_i + pa.center[k];
        pj[k] = -pb.rotation_matrix[k * 3 + main_axis_j] * l_j + pb.center[k];
        qj[k] = pb.rotation_matrix[k * 3 + main_axis_j] * l_j + pb.center[k];
    }
    let (c1, c2) = line_segments_distance(&pi, &qi, &pj, &qj);
    // `pointDistance`
    let d = ((c1[0] - c2[0]) * (c1[0] - c2[0])
        + (c1[1] - c2[1]) * (c1[1] - c2[1])
        + (c1[2] - c2[2]) * (c1[2] - c2[2]))
        .sqrt();
    d < radsum
}

/// A body in the insertion overlap list (`RegionNeighborList::Particle` with
/// the superquadric fields).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SqOverlapBody {
    x: V3,
    radius: f64,
    quat: [f64; 4],
    shape: V3,
    blockiness: [f64; 2],
}

impl SuperquadricParticle {
    /// This particle as `load_xnear` loads it (`insert_superquadric`).
    pub(crate) fn overlap_body(&self) -> SqOverlapBody {
        SqOverlapBody {
            x: self.xa(),
            radius: self.radius,
            quat: self.quat,
            shape: self.shape,
            blockiness: self.blockiness,
        }
    }
}

/// The insertion overlap list of `fix insert/pack` for superquadrics
/// (`RegionNeighborList` with `insert_superquadric` /
/// `hasOverlap_superquadric`). The answer is a boolean, so a uniform grid
/// with the same predicate gives the same answers as upstream's bins.
#[derive(Debug, Clone)]
pub(crate) struct SqOverlapList {
    check_obb: bool,
    cell: f64,
    cells: std::collections::BTreeMap<(i64, i64, i64), Vec<SqOverlapBody>>,
}

impl SqOverlapList {
    /// An empty list of cell size `cell` (at least the largest bounding
    /// diameter); `check_obb` is `fix insert ... check_obb yes|no`.
    pub(crate) fn new(cell: f64, check_obb: bool) -> Self {
        Self {
            check_obb,
            cell: if cell > 0.0 { cell } else { 1.0 },
            cells: std::collections::BTreeMap::new(),
        }
    }

    fn key(&self, p: &V3) -> (i64, i64, i64) {
        (
            (p[0] / self.cell).floor() as i64,
            (p[1] / self.cell).floor() as i64,
            (p[2] / self.cell).floor() as i64,
        )
    }

    /// `insert_superquadric`.
    pub(crate) fn insert(&mut self, b: SqOverlapBody) {
        let k = self.key(&b.x);
        self.cells.entry(k).or_default().push(b);
    }

    /// `hasOverlap_superquadric`: bounding spheres (`rsq <= radsum^2`), and
    /// with `check_obb` also `capsules_intersect` and `obb_intersect`
    /// (starting at inequality 0).
    pub(crate) fn has_overlap(&self, b: &SqOverlapBody) -> bool {
        let (cx, cy, cz) = self.key(&b.x);
        let particle1 = Sq::set(b.x, &b.quat, b.shape, b.blockiness);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let Some(v) = self.cells.get(&(cx + dx, cy + dy, cz + dz)) else {
                        continue;
                    };
                    for p in v {
                        let del = sub3(&b.x, &p.x);
                        let rsq = del[0] * del[0] + del[1] * del[1] + del[2] * del[2];
                        let radsum = b.radius + p.radius;
                        if self.check_obb {
                            let particle2 = Sq::set(p.x, &p.quat, p.shape, p.blockiness);
                            let mut start = 0_u32;
                            if rsq <= radsum * radsum
                                && (capsules_intersect(&particle1, &particle2)
                                    && obb_intersect(&particle1, &particle2, &mut start))
                            {
                                return true;
                            }
                        } else if rsq <= radsum * radsum {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}

impl SuperquadricSystem {
    /// A system with no particles yet, for bodies that
    /// [`SuperquadricSystem::with_insertion`] adds; otherwise as
    /// [`SuperquadricSystem::new`].
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] for a non-positive `dt`.
    pub fn without_particles(materials: MaterialTable, dt: Time) -> Result<Self, DemError> {
        let dt = dt.get::<second>();
        if !(dt > 0.0) {
            return Err(DemError::InvalidInput("time step must be positive".into()));
        }
        let normal = GranularNormalModel::hertz(GranularMaterial::new(1.0e7, 0.3, 0.5, 0.4)?);
        Ok(Self {
            particles: Vec::new(),
            materials,
            normal,
            walls: Vec::new(),
            gravity: None,
            curvature: None,
            dt,
            pair_history: Vec::new(),
            wall_shear: Vec::new(),
            is_setup: false,
            neigh: None,
            step: 0,
            inserters: Vec::new(),
            rolling_cdt: None,
        })
    }

    /// Attach a `fix insert/pack` (or `insert/rate/region`) over a
    /// distribution of [`ParticleTemplateSuperquadric`]s, and run its set-up.
    /// It acts at upstream's `pre_exchange` point: on its insertion step
    /// `Neighbor::decide` returns at once (no `ago++`), the bodies are
    /// appended with consecutive tags, and the list is rebuilt. The overlap
    /// test sees every body present (`load_xnear`, with `check_obb`).
    /// Call after [`SuperquadricSystem::with_neighbor_list`].
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] without upstream's neighbour list (the
    /// default all-pairs list is exact for two bodies only), for a
    /// distribution that is not all superquadric, for a template type
    /// outside the material table, or whatever
    /// [`crate::insert::InsertFix::setup`] reports.
    pub fn with_insertion(mut self, mut fix: crate::insert::InsertFix) -> Result<Self, DemError> {
        if self.neigh.is_none() {
            return Err(DemError::InvalidInput(
                "superquadric insertion needs upstream's neighbour list (with_neighbor_list)".into(),
            ));
        }
        if !fix.is_superquadric() {
            return Err(DemError::InvalidInput(
                "SuperquadricSystem inserts particletemplate/superquadric distributions only".into(),
            ));
        }
        if fix.max_atom_type() > self.materials.n_types() {
            return Err(DemError::InvalidInput(
                "template atom type outside the material table".into(),
            ));
        }
        fix.setup()?;
        self.inserters.push(fix);
        Ok(self)
    }

    /// The `i`-th insertion fix.
    #[must_use]
    pub fn insertion(&self, i: usize) -> &crate::insert::InsertFix {
        &self.inserters[i]
    }

    /// Whether an insertion fix asks for a rebuild on `step`
    /// (`Neighbor::decide`'s `next_reneighbor` test, before `ago++`).
    fn insertion_due(&self, step: u64) -> bool {
        self.inserters
            .iter()
            .any(|f| f.next_insertion_step() == Some(step))
    }

    /// `FixInsert::pre_exchange` of every insertion fix (it acts only on its
    /// own step), appending the new bodies and growing the histories.
    ///
    /// Errors upstream would abort the run on panic with its message, as
    /// [`SuperquadricSystem::run`] has no error channel.
    fn run_insertions(&mut self, step: u64) {
        for k in 0..self.inserters.len() {
            let atoms: Vec<crate::particle::Particle> = self
                .particles
                .iter()
                .map(|p| crate::particle::Particle {
                    position: p.x,
                    velocity: p.v,
                    angular_velocity: p.omega,
                    mass: p.mass,
                    radius: p.radius,
                    temperature: 300.0,
                })
                .collect();
            let bodies: Vec<SqOverlapBody> =
                self.particles.iter().map(SuperquadricParticle::overlap_body).collect();
            let new = match self.inserters[k].pre_exchange_superquadric(step, &atoms, &bodies) {
                Ok(v) => v,
                Err(e) => panic!("fix insert: {e}"),
            };
            if new.is_empty() {
                continue;
            }
            let old_n = self.particles.len();
            for s in new {
                self.particles.push(s.into_particle());
            }
            self.grow_histories(old_n);
        }
    }

    /// Re-index the pair history (`i * n + j`) and the wall histories (one
    /// slot per particle, or `i_tri * n + ip` on a mesh) from `old_n` to the
    /// current number of particles; new slots start empty, as upstream's
    /// `set_arrays` zeroes a new atom's history.
    fn grow_histories(&mut self, old_n: usize) {
        let n = self.particles.len();
        let mut pair = vec![PairHistory::default(); n * n];
        for i in 0..old_n {
            for j in 0..old_n {
                pair[i * n + j] = self.pair_history[i * old_n + j];
            }
        }
        self.pair_history = pair;
        for w in 0..self.walls.len() {
            let ntri = match &self.walls[w] {
                SqWall::Mesh { mesh, .. } => mesh.triangles.len(),
                SqWall::Plane(_) | SqWall::Cylinder(_) => 1,
            };
            let mut shear = vec![[0.0; 3]; ntri * n];
            for t in 0..ntri {
                for ip in 0..old_n {
                    shear[t * n + ip] = self.wall_shear[w][t * old_n + ip];
                }
            }
            self.wall_shear[w] = shear;
        }
    }
}

/// `RollingModel<ROLLING_CDT>::surfacesIntersect` with `torsionTorque off`
/// (the default): the constant directional torque `i` loses. At a wall
/// `Fn = deltan kn` and `wr` is the particle's angular velocity; for a pair
/// `wr = omega_i - omega_j`. `reff` is the superquadric `sidata.reff` (the
/// volume-equivalent or curvature-limited radius).
fn cdt_rolling_torque(
    rmu: f64,
    kn: f64,
    deltan: f64,
    reff: f64,
    en: &V3,
    wr: &V3,
    is_wall: bool,
) -> V3 {
    let mut r_torque = [0.0; 3];
    let wrmag = (wr[0] * wr[0] + wr[1] * wr[1] + wr[2] * wr[2]).sqrt();
    if !(wrmag > 0.0) {
        return r_torque;
    }
    if is_wall {
        let fn_ = deltan * kn;
        r_torque[0] = rmu * fn_ * wr[0] / wrmag * reff;
        r_torque[1] = rmu * fn_ * wr[1] / wrmag * reff;
        r_torque[2] = rmu * fn_ * wr[2] / wrmag * reff;
    } else {
        let scale = rmu * kn * deltan * reff / wrmag;
        r_torque = [scale * wr[0], scale * wr[1], scale * wr[2]];
    }
    let dot = r_torque[0] * en[0] + r_torque[1] * en[1] + r_torque[2] * en[2];
    let r_n = [en[0] * dot, en[1] * dot, en[2] * dot];
    [r_torque[0] - r_n[0], r_torque[1] - r_n[1], r_torque[2] - r_n[2]]
}

impl SuperquadricSystem {
    /// `pair_style gran ... rolling_friction cdt` and the same on every
    /// `fix wall/gran` (gh:#1415): the constant directional torque
    /// `mu_r k_n delta_n r_eff` against the relative angular velocity, with
    /// the torsional part removed (`torsionTorque off`, the default), using
    /// the superquadric effective radius. `coeff_roll_frict[i][j]` is
    /// `coeffRollFrict peratomtypepair`. Upstream's default is no rolling
    /// model.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] unless the table is square over the
    /// material types, symmetric, and non-negative.
    pub fn with_rolling_cdt(mut self, coeff_roll_frict: &[&[f64]]) -> Result<Self, DemError> {
        let n = self.materials.n_types();
        if coeff_roll_frict.len() != n || coeff_roll_frict.iter().any(|r| r.len() != n) {
            return Err(DemError::InvalidInput(
                "coeffRollFrict must have one value per type pair".into(),
            ));
        }
        let mut table = Vec::with_capacity(n * n);
        for i in 0..n {
            for j in 0..n {
                let v = coeff_roll_frict[i][j];
                if !(v >= 0.0) || v != coeff_roll_frict[j][i] {
                    return Err(DemError::InvalidInput(
                        "coeffRollFrict must be non-negative and symmetric".into(),
                    ));
                }
                table.push(v);
            }
        }
        self.rolling_cdt = Some(table);
        Ok(self)
    }
}

/// `coeffRollFrict[itype][jtype]` of an `n`-type table, when the CDT model
/// is on.
fn cdt_coeff(table: Option<&[f64]>, n: usize, itype: usize, jtype: usize) -> Option<f64> {
    table.map(|t| t[itype * n + jtype])
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::f64::{Length, MassDensity, Time};

    fn m(v: f64) -> Length {
        Length::new::<meter>(v)
    }

    #[test]
    fn sphere_limit_matches_closed_forms() {
        // n1 = n2 = 2 is an ellipsoid; equal half-axes a sphere
        let r = 0.002;
        let p = SuperquadricParticle::new(
            [m(0.0), m(0.0), m(0.0)],
            [m(r), m(r), m(r)],
            [2.0, 2.0],
            MassDensity::new::<kilogram_per_cubic_meter>(2500.0),
            [1.0, 0.0, 0.0, 0.0],
            Vec3::zero(),
            Vec3::zero(),
            0,
        )
        .unwrap();
        let v = 4.0 / 3.0 * M_PI * r * r * r;
        assert!((p.volume / v - 1.0).abs() < 1e-12, "{} vs {v}", p.volume);
        let i = 0.4 * p.mass * r * r;
        for k in 0..3 {
            assert!((p.inertia[k] / i - 1.0).abs() < 1e-12);
        }
        assert!((p.radius / (r * SCALE_KOEF) - 1.0).abs() < 1e-15);
        assert!((p.equivalent_radius() / r - 1.0).abs() < 1e-14);
    }

    #[test]
    fn linear_algebra_helpers_agree() {
        let mut j = [0.0; 16];
        for (k, x) in j.iter_mut().enumerate() {
            *x = ((k * 7 + 3) % 11) as f64 + if k % 5 == 0 { 4.0 } else { 0.0 };
        }
        let mut inv = [0.0; 16];
        let d = inverse_matrix_4x4(&j, &mut inv);
        assert!((d / determinant_4x4(&j) - 1.0).abs() < 1e-12);
        let b = [1.0, -2.0, 0.5, 3.0];
        let x = matvec_n::<4>(&inv, &b);
        let mut xg = [0.0; 4];
        gmres4(&j, &b, &[0.0; 4], &mut xg);
        for k in 0..4 {
            assert!(
                (x[k] - xg[k]).abs() < 1e-9 * (1.0 + x[k].abs()),
                "{x:?} {xg:?}"
            );
        }
    }

    #[test]
    fn free_spinning_body_conserves_angular_momentum() {
        let p = SuperquadricParticle::new(
            [m(0.0), m(0.0), m(0.0)],
            [m(0.004), m(0.003), m(0.0025)],
            [6.0, 6.0],
            MassDensity::new::<kilogram_per_cubic_meter>(2500.0),
            [0.9, 0.3, 0.2, 0.1],
            Vec3::new(0.1, 0.0, 0.0),
            Vec3::new(3.0, -2.0, 5.0),
            0,
        )
        .unwrap();
        let l0 = p.angmom;
        let mats = MaterialTable::new(&[(1.0e7, 0.3)], &[&[0.5]], &[&[0.4]]).unwrap();
        let mut s = SuperquadricSystem::new(vec![p], mats, Time::new::<second>(1e-5)).unwrap();
        s.run(1000);
        let l = s.particles()[0].angmom;
        assert!(
            (l.x - l0.x).abs() < 1e-18 && (l.y - l0.y).abs() < 1e-18 && (l.z - l0.z).abs() < 1e-18
        );
        assert!((s.particles()[0].x.x - 0.001).abs() < 1e-15);
        assert_eq!(s.particles()[0].force(), Vec3::zero());
        assert_eq!(s.particles()[0].torque(), Vec3::zero());
        assert_eq!(s.step_count(), 1000);
    }

    /// The new builders refuse what upstream refuses or what is not ported,
    /// and the small geometry helpers give their closed-form answers.
    #[test]
    fn wall_curvature_and_neighbour_builders_and_helpers() {
        let p = SuperquadricParticle::new(
            [m(0.0), m(0.0), m(0.01)],
            [m(0.004), m(0.003), m(0.0025)],
            [6.0, 6.0],
            MassDensity::new::<kilogram_per_cubic_meter>(2500.0),
            [1.0, 0.0, 0.0, 0.0],
            Vec3::zero(),
            Vec3::zero(),
            0,
        )
        .unwrap();
        let mats = MaterialTable::new(&[(1.0e7, 0.3)], &[&[0.5]], &[&[0.4]]).unwrap();
        let s = SuperquadricSystem::new(vec![p], mats, Time::new::<second>(1e-5)).unwrap();
        let cyl = |axis, radius, wall_type| SuperquadricCylinderWall {
            axis,
            radius,
            center: [0.0, 0.0],
            wall_type,
        };
        assert!(s.clone().with_cylinder_wall(cyl(3, 0.01, 0)).is_err());
        assert!(s.clone().with_cylinder_wall(cyl(2, 0.0, 0)).is_err());
        assert!(s.clone().with_cylinder_wall(cyl(2, 0.01, 1)).is_err());
        assert!(s
            .clone()
            .with_curvature_limit(-1.0, CurvatureKind::Mean)
            .is_err());
        let off = s
            .clone()
            .with_curvature_limit(0.0, CurvatureKind::Mean)
            .unwrap();
        assert!(off.curvature.is_none());
        let lo = Vec3::new(-0.05, -0.05, -0.05);
        let hi = Vec3::new(0.05, 0.05, 0.05);
        let cfg = LiggghtsNeighbor::new(lo, hi, 0.002).unwrap();
        // LIGGGHTS' default atom sort is on: refused for superquadrics
        assert!(s.clone().with_neighbor_list(cfg, 1, true).is_err());
        let mut cfg0 = cfg;
        cfg0.sort_every = 0;
        assert!(s.clone().with_neighbor_list(cfg0, 0, true).is_err());
        let mut cfg2 = cfg0;
        cfg2.contact_distance_factor = 1.1;
        assert!(s.clone().with_neighbor_list(cfg2, 1, true).is_err());
        assert!(s.clone().with_neighbor_list(cfg0, 1, true).is_ok());

        // Cylinder<2>: inside at r = 3 mm of a 10 mm cylinder, gap 7 mm - radius
        let w = cyl(2, 0.01, 0);
        let (gap, delta) = cylinder_resolve_contact(&w, &[0.003, 0.0, 0.5], 0.001);
        assert!((gap - 0.006).abs() < 1e-15);
        assert!((delta[0] - 0.007).abs() < 1e-15 && delta[1] == 0.0 && delta[2] == 0.0);
        // on the axis: upstream's zero-distance guard
        assert_eq!(
            cylinder_resolve_contact(&w, &[0.0, 0.0, 0.5], 0.001),
            (0.0, [0.0; 3])
        );
        // isInElement on the unit right triangle
        let nd = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        assert!(is_in_element(&nd, &[0.2, 0.2, 0.0]));
        assert!(!is_in_element(&nd, &[0.8, 0.8, 0.0]));
        // get_effective_radius: min(1 / keff, rmax), rmax for a flat surface
        assert_eq!(effective_radius_limited(1.0, 0.5), 0.5);
        assert_eq!(effective_radius_limited(4.0, 0.5), 0.25);
        // a sphere (n = 2, a = b = c = r) has mean curvature 1/r and Gaussian
        // curvature 1/r^2, whose square root the routine returns
        let r = 0.002;
        let sq = Sq::set([0.0; 3], &[1.0, 0.0, 0.0, 0.0], [r, r, r], [2.0, 2.0]);
        let km = sq.calc_curvature_coefficient(false, &[0.003, 0.001, -0.001]);
        let kg = sq.calc_curvature_coefficient(true, &[0.003, 0.001, -0.001]);
        assert!((km * r - 1.0).abs() < 1e-12, "{km}");
        assert!((kg * r - 1.0).abs() < 1e-12, "{kg}");
    }

    #[test]
    fn insertion_templates_overlap_list_and_refusals() {
        use crate::insert::{
            InsertFix, InsertMode, InsertRegion, PackTarget, ParticleDistributionDiscrete,
            ParticleTemplateSphere, RanPark,
        };
        use crate::neighbor_list::LiggghtsNeighbor;
        let rho = |v: f64| MassDensity::new::<kilogram_per_cubic_meter>(v);
        let tpl = |seed: i32| {
            ParticleTemplateSuperquadric::new(
                seed,
                1,
                [m(0.004), m(0.003), m(0.0025)],
                [6.0, 6.0],
                rho(2500.0),
            )
            .unwrap()
        };
        // template refusals and the expectancies of the shape functions
        assert!(ParticleTemplateSuperquadric::new(15485863, 0, [m(1.0); 3], [2.0, 2.0], rho(1.0)).is_err());
        assert!(ParticleTemplateSuperquadric::new(15485863, 1, [m(0.0); 3], [2.0, 2.0], rho(1.0)).is_err());
        assert!(ParticleTemplateSuperquadric::new(15485863, 1, [m(1.0); 3], [1.5, 2.0], rho(1.0)).is_err());
        assert!(ParticleTemplateSuperquadric::new(15485863, 1, [m(1.0); 3], [2.0, 2.0], rho(0.0)).is_err());
        let t = tpl(15485863);
        let shape = [0.004, 0.003, 0.0025];
        assert_eq!(t.volexpect(), volume_superquadric(&shape, &[6.0, 6.0]));
        assert_eq!(t.massexpect(), 2500.0 * t.volexpect());
        assert_eq!(t.r_bound(), bounding_sphere_radius(&shape, &[6.0, 6.0]));
        let mut q = t.to_insert();
        assert_eq!(q.density(), 2500.0);
        assert_eq!(q.mass, 2500.0 * t.volexpect());
        // a quaternion is copied, not normalised (`ParticleToInsertSuperquadric::insert`)
        q.set_x_v_omega(Vec3::new(0.0, 0.0, 0.01), Vec3::zero(), Vec3::new(1.0, 0.0, 0.0), [2.0, 0.0, 0.0, 0.0]);
        let p = q.clone().into_particle();
        assert_eq!(p.quat, [2.0, 0.0, 0.0, 0.0]);
        assert_eq!(p.itype, 0);
        // random_unit_quat is a unit quaternion and draws three numbers
        let mut r1 = RanPark::new(32452867).unwrap();
        let mut r2 = r1.clone();
        let rq = random_unit_quat(&mut r1);
        let n2 = rq.iter().map(|v| v * v).sum::<f64>();
        assert!((n2 - 1.0).abs() < 1e-14, "{n2}");
        for _ in 0..3 {
            r2.uniform();
        }
        assert_eq!(r1.state(), r2.state());
        // line_segments_distance: crossing segments, parallel ones, points
        let (c1, c2) = line_segments_distance(&[-1.0, 0.0, 0.0], &[1.0, 0.0, 0.0], &[0.0, -1.0, 1.0], &[0.0, 1.0, 1.0]);
        assert_eq!((c1, c2), ([0.0, 0.0, 0.0], [0.0, 0.0, 1.0]));
        let (c1, c2) = line_segments_distance(&[0.0; 3], &[1.0, 0.0, 0.0], &[0.0, 1.0, 0.0], &[1.0, 1.0, 0.0]);
        assert_eq!((c1[1], c2[1]), (0.0, 1.0));
        let (c1, c2) = line_segments_distance(&[0.0; 3], &[0.0; 3], &[0.0, 2.0, 0.0], &[0.0, 2.0, 0.0]);
        assert_eq!((c1, c2), ([0.0; 3], [0.0, 2.0, 0.0]));
        let (_, c2) = line_segments_distance(&[0.0; 3], &[0.0; 3], &[-1.0, 1.0, 0.0], &[1.0, 1.0, 0.0]);
        assert_eq!(c2, [0.0, 1.0, 0.0]);
        let (c1, _) = line_segments_distance(&[-1.0, 0.0, 0.0], &[1.0, 0.0, 0.0], &[3.0, 1.0, 0.0], &[3.0, 1.0, 0.0]);
        assert_eq!(c1, [1.0, 0.0, 0.0]);
        // overlap list: two boxes side by side along x whose bounding spheres
        // overlap but whose boxes do not; only check_obb separates them
        let id = [1.0, 0.0, 0.0, 0.0];
        let a = SqOverlapBody { x: [0.0; 3], radius: 0.0055, quat: id, shape, blockiness: [6.0, 6.0] };
        let b = SqOverlapBody { x: [0.0085, 0.0, 0.0], ..a };
        let sa = Sq::set(a.x, &a.quat, a.shape, a.blockiness);
        assert!(!sa.is_cylinder());
        let cyl = Sq::set(a.x, &a.quat, a.shape, [6.0, 2.0]);
        assert!(cyl.is_cylinder());
        assert!(capsules_intersect(&sa, &cyl));
        let mut obb = SqOverlapList::new(0.012, true);
        obb.insert(a);
        assert!(!obb.has_overlap(&b));
        let mut spheres = SqOverlapList::new(0.012, false);
        spheres.insert(a);
        assert!(spheres.has_overlap(&b));
        let c = SqOverlapBody { x: [0.006, 0.0, 0.0], ..a };
        assert!(obb.has_overlap(&c));
        // refusals around the insert fix and the system
        let lo = Vec3::new(-0.05, -0.05, -0.005);
        let hi = Vec3::new(0.05, 0.05, 0.15);
        let region = InsertRegion::block(Vec3::new(-0.01, -0.01, 0.02), Vec3::new(0.01, 0.01, 0.04)).unwrap();
        let sphere = ParticleTemplateSphere::new(15485867, 1, m(0.002), rho(2500.0)).unwrap();
        assert!(ParticleDistributionDiscrete::from_templates(
            32452843,
            true,
            vec![
                (crate::insert::ParticleTemplate::Sphere(sphere.clone()), 0.5),
                (crate::insert::ParticleTemplate::Superquadric(tpl(15485863)), 0.5),
            ],
        )
        .is_err());
        let sdist = ParticleDistributionDiscrete::new(32452843, true, vec![(sphere, 1.0)]).unwrap();
        let pack = InsertMode::Pack(PackTarget::Particles(3));
        let sfix = InsertFix::new(pack, 32452867, sdist, region, 0, lo, hi).unwrap();
        assert!(sfix.clone().with_random_orientation().is_err());
        let mats = MaterialTable::new(&[(1.0e7, 0.3)], &[&[0.5]], &[&[0.4]]).unwrap();
        let dt = Time::new::<second>(1e-5);
        let mut cfg = LiggghtsNeighbor::new(lo, hi, 0.002).unwrap();
        cfg.sort_every = 0;
        let empty = SuperquadricSystem::without_particles(mats.clone(), dt).unwrap();
        assert!(SuperquadricSystem::without_particles(mats.clone(), Time::new::<second>(0.0)).is_err());
        let with_list = empty.clone().with_neighbor_list(cfg, 1, false).unwrap();
        assert!(with_list.clone().with_insertion(sfix).is_err());
        let qdist = |seed| ParticleDistributionDiscrete::new_superquadric(seed, true, vec![(tpl(15485863), 1.0)]).unwrap();
        let qfix = |overlap: bool| {
            InsertFix::new(pack, 32452867, qdist(32452843), region, 0, lo, hi)
                .unwrap()
                .with_overlap_check(overlap)
                .with_check_obb(false)
                .with_random_orientation()
                .unwrap()
        };
        assert!(empty.with_insertion(qfix(true)).is_err());
        let mut f = qfix(true);
        f.setup().unwrap();
        assert!(f.pre_exchange_any(1, &[], &crate::insert::ClumpCensus::default()).is_err());
        assert!(qdist(32452843).randomize_list_any(1, true).len() == 1);
        // no overlap check: three bodies placed with random orientations,
        // then the histories grow and the run goes on
        let mut sys = with_list
            .with_plane_wall(SuperquadricPlaneWall { dim: 2, coord: 0.0, wall_type: 0 })
            .unwrap()
            .with_insertion(qfix(false))
            .unwrap();
        assert_eq!(sys.insertion(0).next_insertion_step(), Some(1));
        sys.run(2);
        assert_eq!(sys.particles().len(), 3);
        assert_eq!(sys.insertion(0).ninserted(), 3);
        assert_eq!(sys.pair_history.len(), 9);
        assert_eq!(sys.wall_shear[0].len(), 3);
        for p in sys.particles() {
            assert!(p.x.z > 0.02 && p.x.z < 0.04);
            assert_ne!(p.quat, [1.0, 0.0, 0.0, 0.0]);
        }
    }

    #[test]
    fn rolling_cdt_builder_and_torque() {
        let mats = MaterialTable::new(&[(1.0e7, 0.3)], &[&[0.5]], &[&[0.4]]).unwrap();
        let p = SuperquadricParticle::new(
            [m(0.0), m(0.0), m(0.01)],
            [m(0.002); 3],
            [2.0, 2.0],
            MassDensity::new::<kilogram_per_cubic_meter>(2500.0),
            [1.0, 0.0, 0.0, 0.0],
            Vec3::zero(),
            Vec3::zero(),
            0,
        )
        .unwrap();
        let sys = SuperquadricSystem::new(vec![p], mats, Time::new::<second>(1e-5)).unwrap();
        assert!(sys.clone().with_rolling_cdt(&[]).is_err());
        assert!(sys.clone().with_rolling_cdt(&[&[-0.1]]).is_err());
        let sys = sys.with_rolling_cdt(&[&[0.1]]).unwrap();
        assert_eq!(cdt_coeff(sys.rolling_cdt.as_deref(), 1, 0, 0), Some(0.1));
        assert_eq!(cdt_coeff(None, 1, 0, 0), None);
        let en = [0.0, 0.0, 1.0];
        // no relative spin: no torque
        assert_eq!(cdt_rolling_torque(0.1, 1e4, 1e-5, 1e-3, &en, &[0.0; 3], true), [0.0; 3]);
        // spin about the normal only: the torsional part is removed
        let t = cdt_rolling_torque(0.1, 1e4, 1e-5, 1e-3, &en, &[0.0, 0.0, 5.0], false);
        assert_eq!(t, [0.0; 3]);
        // rolling about x: |T| = mu_r kn deltan reff, along +x
        let t = cdt_rolling_torque(0.1, 1e4, 1e-5, 1e-3, &en, &[3.0, 0.0, 0.0], true);
        assert!((t[0] - 0.1 * 1e4 * 1e-5 * 1e-3).abs() < 1e-20 && t[1] == 0.0 && t[2] == 0.0);
    }
}
