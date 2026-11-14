// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// PORTED FROM UPSTREAM — provenance (see the crate NOTICE):
//   Upstream project : LIGGGHTS-PUBLIC (DCS Computing GmbH / JKU Linz)
//   Upstream files   : src/fix_multisphere.cpp (`initial_integrate`,
//                      `final_integrate`, `calc_force`, `set_xv`, `set_v`),
//                      src/multisphere.cpp, src/multisphere_I.h (`add_body`,
//                      `reset_forces`, `id_extend_body_extend`),
//                      src/fix_template_multiplespheres.cpp
//                      (`calc_bounding_sphere`, `calc_center_of_mass`,
//                      `check_overlap`), src/fix_template_multisphere.cpp
//                      (`calc_inertia`, `calc_eigensystem`,
//                      `calc_displace_xcm_x_body`, `calc_volumeweight`),
//                      src/particleToInsert_multisphere.cpp
//                      (`check_near_set_x_v_omega`, `set_x_v_omega`, `insert`),
//                      src/math_extra.cpp / math_extra.h (`jacobi`, `rotate`,
//                      `richardson`, `mq_to_omega`, `angmom_to_omega`,
//                      `exyz_to_q`, `q_to_exyz`, `quat_to_mat`, `qnormalize`,
//                      `vecquat`, `quatquat`, `matvec`, `transpose_matvec`),
//                      src/math_extra_liggghts.h (`angmom_from_omega`,
//                      `vec_quat_rotate`, `local_coosys_to_cartesian`,
//                      `cartesian_coosys_to_local_orthogonal`)
//   Upstream commit  : 3d5c00f20519e6bb6eb6756f51f1ad36564e649d (2024-06-07)
//   Upstream licence : "GNU Public License, version 2 or later" -> used here
//                      under the "or later" option as GPL-3.0. `math_extra`
//                      is a LAMMPS original, Copyright (2003) Sandia
//                      Corporation (GPL); the LIGGGHTS files are Copyright
//                      2012- DCS Computing GmbH, Linz and 2009-2012 JKU Linz.
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

//! # Multisphere (rigid clump) particles: `fix multisphere`, `fix particletemplate/multisphere`
//!
//! gh:#1415 (child of #1405). A clump is a rigid body made of overlapping
//! spheres. Each sphere is an ordinary atom, so every sphere contact law, wall
//! and neighbour list of this crate applies to it unchanged; the clump adds a
//! rigid-body integrator over its member spheres.
//!
//! | upstream | here |
//! |---|---|
//! | `fix particletemplate/multisphere` (`spheres x y z r ...`, Monte-Carlo mass and inertia) | [`ParticleTemplateMultisphere`] |
//! | `fix multisphere` (body state, integration, force/torque sum) | [`Multisphere`] |
//! | `MathExtra::jacobi`, `richardson`, quaternion helpers | private functions below |
//!
//! Insertion goes through [`crate::insert::InsertFix`] with a distribution of
//! multisphere templates ([`crate::insert::ParticleDistributionDiscrete::new_multisphere`]);
//! [`crate::granular_system::GranularSystem`] owns the body state and calls it
//! at upstream's points of the time step.
//!
//! ## The time step (`FixMultisphere`)
//!
//! * `initial_integrate`: per body, `vcm += dtf/m fcm`, `xcm += dt vcm`,
//!   `angmom += dtf torque`, `omega = angmom_to_omega(angmom)`, the
//!   quaternion advanced by `MathExtra::richardson`, the axes from
//!   `q_to_exyz`; then `set_xv`: each member atom gets `x = R d + xcm`,
//!   `v = omega x (R d) + vcm` and the body's `omega`.
//! * `pre_force`: `fcm = torque = 0`.
//! * the pair and wall forces act on the member atoms; `fix gravity` skips
//!   them.
//! * `final_integrate` -> `calc_force`: in local atom order, `fcm += f`,
//!   `torque += (x - xcm) x f + t`; then `fcm += m g` (the one `fix gravity`)
//!   and the (zero) drag; the second half-kick, `omega` from `angmom`, and
//!   `set_v`.
//! * `neigh_modify exclude molecule`: two atoms of the same body are never a
//!   pair (`Neighbor::exclusion`); the order of the other pairs is unchanged.
//!
//! Each member atom carries the **clump mass** as its `rmass`
//! (`ParticleToInsertMultisphere::insert`), so a contact's effective mass is
//! the bodies'.
//!
//! ## The template (`FixTemplateMultiplespheres` / `FixTemplateMultisphere`)
//!
//! The mass properties come from Monte-Carlo integration over the bounding box
//! of the spheres, drawing from the template's `random_mc` stream (seeded with
//! the template seed) in upstream's order: the bounding sphere (200 randomised
//! passes), the volume and centre of mass (`ntry` points), the inertia tensor
//! (`ntry` points, symmetrised), its eigensystem by Jacobi rotations, the
//! body-frame displacements, then the per-sphere volume weights (`ntry`
//! points). Same stream, same operation order: same bits.
//!
//! ## What is translated, and what is not
//!
//! Translated: the above, ~~`fflag`/`tflag` (all on)~~ the template's
//! `fflag`/`tflag` ([`ParticleTemplateMultisphere::with_fflag`],
//! [`ParticleTemplateMultisphere::with_tflag`]), insertion with
//! `orientation constant|template|random`, `vel`, `omega`, with or without the
//! overlap check, single process, fixed (`f`/`m`) boundaries; lost atoms and
//! bodies (`check_lost_atoms`, `delflag`, `FixMultisphere::pre_exchange`;
//! `Multisphere::pre_neighbor`, with
//! [`crate::granular_system::GranularSystem::with_lost_atoms`] for the box);
//! `compute ke/multisphere` and `erotate/multisphere`
//! ([`Multisphere::extract_ke`], [`Multisphere::extract_rke`]).
//!
//! **Not translated** (each rejected or documented, never silently ignored):
//! ~~`spheres file` (the inline list only)~~ (**CORRECTED 2026-11-14:**
//! `spheres file <f> scale <s>` is [`ParticleTemplateMultisphere::from_clump_file`]),
//! ~~`spheres_different_types`,~~ ~~`bonded`,~~
//! ~~`mass`/`inertia_tensor`/`use_volume`/`use_density`,~~ ~~`fflag`/`tflag` off,~~
//! ~~`fix_modify <ms> fflag|tflag`,~~
//! ~~`fix multisphere/nointegration`,~~ `concave`, `CAddRhoFluid`,
//! `allow_heatsource`; periodic images of bodies (`remap_bodies`), ~~lost-atom
//! deletion (`check_lost_atoms` / `delflag`),~~ `fix remove`'s
//! `delete_bodies`, ~~`fix multisphere/break`~~,
//! ~~`fix insert/stream` with clumps (`start_step`; refused by
//! [`crate::insert_stream::InsertStreamFix::new`])~~, ~~heat transfer with clumps~~,
//! restart, ~~a second `run` set-up with clumps~~~~, and `orientation random`~~.
//! **CORRECTED 2026-11-14 (gh:#1415):** `fflag`/`tflag`, lost-atom deletion
//! and `orientation random` are translated (V&V 4.16.1).
//! **CORRECTED 2026-11-14 (gh:#1415, third pass):** `mass` + `inertia_tensor`
//! with `use_volume` / `use_density` is
//! [`ParticleTemplateMultisphere::new_with_mass_inertia`]; a second `run`
//! set-up with clumps is
//! [`crate::granular_system::GranularSystem::begin_run`], and
//! `fix_modify <ms> fflag|tflag` is
//! [`crate::granular_system::GranularSystem::fix_modify_multisphere`];
//! `spheres_different_types` is
//! [`ParticleTemplateMultisphere::from_clump_file_different_types`] (the
//! types do not reach the atoms, as upstream);
//! `fix multisphere/nointegration` is [`Multisphere::without_integration`]
//! ([`crate::granular_system::GranularSystem::with_multisphere_nointegration`])
//! (V&V 4.16.2).
//! **CORRECTED 2026-11-14 (gh:#1415, fourth pass):** template `bonded` is
//! [`ParticleTemplateMultisphere::with_bonded`] (the `bond_random_id`
//! property, `-1` on every atom, as upstream). `concave` stays untranslated
//! and has nothing to translate in the default `lmp_serial`: upstream's
//! `FixMultisphere::init` refuses `fix concave` with any multisphere
//! template, and `particletemplate/concave` is not in that build.
//! `fix remove` (and so `delete_bodies`) is not in that build either: its
//! header is a stub there (V&V 4.16.3).
//! Heat transfer with clumps (the `fix heat/gran` branch of
//! `FixMultisphere::calc_force`) is [`Multisphere::with_heat_transfer`] /
//! [`Multisphere::heat_transfer`], switched on by
//! [`crate::granular_system::GranularSystem::with_heat_conduction`]
//! (V&V 4.16.3).
//! Clumps from `fix insert/stream` (`start_step`, `v_integrate`,
//! [`Multisphere::calc_n_steps`], [`Multisphere::release`]) are streamed
//! through [`crate::granular_system::GranularSystem::add_stream_insertion`]
//! (V&V 4.16.3). `fix multisphere/break` with a `fix property/atom` trigger
//! is [`Multisphere::with_break`] / [`Multisphere::break_bodies`]
//! ([`crate::granular_system::GranularSystem::with_multisphere_break`];
//! V&V 4.16.3); an atom-style variable trigger is not ported.
//! ~~Superquadric, convex hull, ellipsoid, line and tri particles (the rest of
//! gh:#1415) are not ported.~~ **CORRECTED 2026-11-13 (gh:#1415):** superquadric
//! particles are ported in [`crate::superquadric`]; convex hull, ellipsoid,
//! line and tri particles are not.

use crate::particle::{Particle, Vec3};
use crate::DemError;
use uom::si::f64::{Length, Mass, MassDensity, MomentOfInertia};
use uom::si::{
    length::meter, mass::kilogram, mass_density::kilogram_per_cubic_meter,
    moment_of_inertia::kilogram_square_meter,
};

/// A `fix_modify <ms> ...` keyword of `fix multisphere`
/// (`FixMultisphere::modify_param`; see
/// [`crate::granular_system::GranularSystem::fix_modify_multisphere`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultisphereModify {
    /// `fflag x y z` (`true` = `on`).
    Fflag([bool; 3]),
    /// `tflag x y z` (`true` = `on`).
    Tflag([bool; 3]),
}

/// How a template given `mass` and `inertia_tensor` gets its volume and
/// density (`use_volume` / `use_density` of `fix particletemplate/multisphere`;
/// see [`ParticleTemplateMultisphere::new_with_mass_inertia`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MassSource {
    /// `use_volume`: the Monte-Carlo volume; density = mass / volume.
    UseVolume,
    /// `use_density`: the template density; volume = mass / density, no
    /// centre-of-mass shift.
    UseDensity,
}

/// The temperature `[K]` every inserted atom carries (the port has no
/// per-insertion `initial_temperature`; a `fix heat/gran` deck compared with
/// it uses `initial_temperature 300`).
pub(crate) const INSERTED_TEMPERATURE: f64 = 300.0;

/// `MAXJACOBI` of `math_extra.cpp`.
const MAXJACOBI: usize = 50;
/// `N_SHUFFLE_BOUND` of `fix_template_multiplespheres.cpp`.
const N_SHUFFLE_BOUND: usize = 200;
/// `LARGE` of `fix_template_multiplespheres.cpp`.
const LARGE: f64 = 1e8;
/// `EPSILON` of `fix_template_multisphere.cpp` (principal-moment cut-off).
const EPSILON_MOI: f64 = 1.0e-7;
/// `TOLERANCE` of `fix_template_multisphere.cpp` (inertia symmetry check).
const TOLERANCE_SYM: f64 = 1.0e-6;
/// `TOLERANCE_ORTHO` of `math_extra_liggghts.h`.
const TOLERANCE_ORTHO: f64 = 1e-10;

type V3 = [f64; 3];

pub(crate) fn dot3(a: &V3, b: &V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `vectorCross3D(v1, v2, res)`.
pub(crate) fn cross3(a: &V3, b: &V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn v3(p: Vec3) -> V3 {
    [p.x, p.y, p.z]
}

fn vec3(a: V3) -> Vec3 {
    Vec3::new(a[0], a[1], a[2])
}

/// `MathExtra::rotate` (one Jacobi rotation of two entries).
fn rotate(m: &mut [[f64; 3]; 3], i: usize, j: usize, k: usize, l: usize, s: f64, tau: f64) {
    let g = m[i][j];
    let h = m[k][l];
    m[i][j] = g - s * (h + g * tau);
    m[k][l] = h + s * (g - h * tau);
}

/// `MathExtra::jacobi`: eigenvalues and eigenvectors (columns) of a symmetric
/// 3x3 matrix, which it destroys. `None` if 50 sweeps do not converge.
fn jacobi(matrix: &mut [[f64; 3]; 3]) -> Option<(V3, [[f64; 3]; 3])> {
    let mut evectors = [[0.0; 3]; 3];
    let mut evalues = [0.0; 3];
    let (mut b, mut z) = ([0.0; 3], [0.0; 3]);
    for i in 0..3 {
        evectors[i][i] = 1.0;
    }
    for i in 0..3 {
        evalues[i] = matrix[i][i];
        b[i] = evalues[i];
        z[i] = 0.0;
    }
    for iter in 1..=MAXJACOBI {
        let mut sm = 0.0;
        for i in 0..2 {
            for j in (i + 1)..3 {
                sm += matrix[i][j].abs();
            }
        }
        if sm == 0.0 {
            return Some((evalues, evectors));
        }
        let tresh = if iter < 4 { 0.2 * sm / (3 * 3) as f64 } else { 0.0 };
        for i in 0..2 {
            for j in (i + 1)..3 {
                let g = 100.0 * matrix[i][j].abs();
                if iter > 4
                    && evalues[i].abs() + g == evalues[i].abs()
                    && evalues[j].abs() + g == evalues[j].abs()
                {
                    matrix[i][j] = 0.0;
                } else if matrix[i][j].abs() > tresh {
                    let mut h = evalues[j] - evalues[i];
                    let t = if h.abs() + g == h.abs() {
                        matrix[i][j] / h
                    } else {
                        let theta = 0.5 * h / matrix[i][j];
                        let t = 1.0 / (theta.abs() + (1.0 + theta * theta).sqrt());
                        if theta < 0.0 {
                            -t
                        } else {
                            t
                        }
                    };
                    let c = 1.0 / (1.0 + t * t).sqrt();
                    let s = t * c;
                    let tau = s / (1.0 + c);
                    h = t * matrix[i][j];
                    z[i] -= h;
                    z[j] += h;
                    evalues[i] -= h;
                    evalues[j] += h;
                    matrix[i][j] = 0.0;
                    for k in 0..i {
                        rotate(matrix, k, i, k, j, s, tau);
                    }
                    for k in (i + 1)..j {
                        rotate(matrix, i, k, k, j, s, tau);
                    }
                    for k in (j + 1)..3 {
                        rotate(matrix, i, k, j, k, s, tau);
                    }
                    for k in 0..3 {
                        rotate(&mut evectors, k, i, k, j, s, tau);
                    }
                }
            }
        }
        for i in 0..3 {
            b[i] += z[i];
            evalues[i] = b[i];
            z[i] = 0.0;
        }
    }
    None
}

/// `MathExtra::qnormalize`.
pub(crate) fn qnormalize(q: &mut [f64; 4]) {
    let norm = 1.0 / (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    q[0] *= norm;
    q[1] *= norm;
    q[2] *= norm;
    q[3] *= norm;
}

/// `MathExtra::vecquat`: `c = a * b` with `a = (0, a)`.
fn vecquat(a: &V3, b: &[f64; 4]) -> [f64; 4] {
    [
        -a[0] * b[1] - a[1] * b[2] - a[2] * b[3],
        b[0] * a[0] + a[1] * b[3] - a[2] * b[2],
        b[0] * a[1] + a[2] * b[1] - a[0] * b[3],
        b[0] * a[2] + a[0] * b[2] - a[1] * b[1],
    ]
}

/// `MathExtra::quatquat`: `c = a * b`.
fn quatquat(a: &[f64; 4], b: &[f64; 4]) -> [f64; 4] {
    [
        a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3],
        a[0] * b[1] + b[0] * a[1] + a[2] * b[3] - a[3] * b[2],
        a[0] * b[2] + b[0] * a[2] + a[3] * b[1] - a[1] * b[3],
        a[0] * b[3] + b[0] * a[3] + a[1] * b[2] - a[2] * b[1],
    ]
}

/// `MathExtraLiggghts::vec_quat_rotate`: `q (0, v) q*`.
fn vec_quat_rotate(v: &V3, q: &[f64; 4]) -> V3 {
    let vq = [0.0, v[0], v[1], v[2]];
    let qc = [q[0], -q[1], -q[2], -q[3]];
    let temp = quatquat(q, &vq);
    let r = quatquat(&temp, &qc);
    [r[1], r[2], r[3]]
}

/// `MathExtra::quat_to_mat`.
fn quat_to_mat(q: &[f64; 4]) -> [[f64; 3]; 3] {
    let w2 = q[0] * q[0];
    let i2 = q[1] * q[1];
    let j2 = q[2] * q[2];
    let k2 = q[3] * q[3];
    let twoij = 2.0 * q[1] * q[2];
    let twoik = 2.0 * q[1] * q[3];
    let twojk = 2.0 * q[2] * q[3];
    let twoiw = 2.0 * q[1] * q[0];
    let twojw = 2.0 * q[2] * q[0];
    let twokw = 2.0 * q[3] * q[0];
    [
        [w2 + i2 - j2 - k2, twoij - twokw, twojw + twoik],
        [twoij + twokw, w2 - i2 + j2 - k2, twojk - twoiw],
        [twoik - twojw, twojk + twoiw, w2 - i2 - j2 + k2],
    ]
}

/// `MathExtra::mq_to_omega`.
fn mq_to_omega(m: &V3, q: &[f64; 4], moments: &V3) -> V3 {
    let rot = quat_to_mat(q);
    // transpose_matvec(rot, m)
    let mut wbody = [
        rot[0][0] * m[0] + rot[1][0] * m[1] + rot[2][0] * m[2],
        rot[0][1] * m[0] + rot[1][1] * m[1] + rot[2][1] * m[2],
        rot[0][2] * m[0] + rot[1][2] * m[1] + rot[2][2] * m[2],
    ];
    for k in 0..3 {
        if moments[k] == 0.0 {
            wbody[k] = 0.0;
        } else {
            wbody[k] /= moments[k];
        }
    }
    // matvec(rot, wbody)
    [
        rot[0][0] * wbody[0] + rot[0][1] * wbody[1] + rot[0][2] * wbody[2],
        rot[1][0] * wbody[0] + rot[1][1] * wbody[1] + rot[1][2] * wbody[2],
        rot[2][0] * wbody[0] + rot[2][1] * wbody[1] + rot[2][2] * wbody[2],
    ]
}

/// `MathExtra::richardson`: advance `q` by `dtq` from `dq/dt = w q / 2`,
/// updating `w` at the half step from the angular momentum `m`.
fn richardson(q: &mut [f64; 4], m: &V3, w: &mut V3, moments: &V3, dtq: f64) {
    let mut wq = vecquat(w, q);
    let mut qfull = [
        q[0] + dtq * wq[0],
        q[1] + dtq * wq[1],
        q[2] + dtq * wq[2],
        q[3] + dtq * wq[3],
    ];
    qnormalize(&mut qfull);
    let mut qhalf = [
        q[0] + 0.5 * dtq * wq[0],
        q[1] + 0.5 * dtq * wq[1],
        q[2] + 0.5 * dtq * wq[2],
        q[3] + 0.5 * dtq * wq[3],
    ];
    qnormalize(&mut qhalf);
    *w = mq_to_omega(m, &qhalf, moments);
    wq = vecquat(w, &qhalf);
    qhalf[0] += 0.5 * dtq * wq[0];
    qhalf[1] += 0.5 * dtq * wq[1];
    qhalf[2] += 0.5 * dtq * wq[2];
    qhalf[3] += 0.5 * dtq * wq[3];
    qnormalize(&mut qhalf);
    q[0] = 2.0 * qhalf[0] - qfull[0];
    q[1] = 2.0 * qhalf[1] - qfull[1];
    q[2] = 2.0 * qhalf[2] - qfull[2];
    q[3] = 2.0 * qhalf[3] - qfull[3];
    qnormalize(q);
}

/// `MathExtra::angmom_to_omega`.
fn angmom_to_omega(m: &V3, ex: &V3, ey: &V3, ez: &V3, idiag: &V3) -> V3 {
    let mut wbody = [0.0; 3];
    wbody[0] = if idiag[0] == 0.0 {
        0.0
    } else {
        (m[0] * ex[0] + m[1] * ex[1] + m[2] * ex[2]) / idiag[0]
    };
    wbody[1] = if idiag[1] == 0.0 {
        0.0
    } else {
        (m[0] * ey[0] + m[1] * ey[1] + m[2] * ey[2]) / idiag[1]
    };
    wbody[2] = if idiag[2] == 0.0 {
        0.0
    } else {
        (m[0] * ez[0] + m[1] * ez[1] + m[2] * ez[2]) / idiag[2]
    };
    [
        wbody[0] * ex[0] + wbody[1] * ey[0] + wbody[2] * ez[0],
        wbody[0] * ex[1] + wbody[1] * ey[1] + wbody[2] * ez[1],
        wbody[0] * ex[2] + wbody[1] * ey[2] + wbody[2] * ez[2],
    ]
}

/// `MathExtraLiggghts::angmom_from_omega`.
fn angmom_from_omega(w: &V3, ex: &V3, ey: &V3, ez: &V3, idiag: &V3) -> V3 {
    let mbody = [
        (w[0] * ex[0] + w[1] * ex[1] + w[2] * ex[2]) * idiag[0],
        (w[0] * ey[0] + w[1] * ey[1] + w[2] * ey[2]) * idiag[1],
        (w[0] * ez[0] + w[1] * ez[1] + w[2] * ez[2]) * idiag[2],
    ];
    [
        mbody[0] * ex[0] + mbody[1] * ey[0] + mbody[2] * ez[0],
        mbody[0] * ex[1] + mbody[1] * ey[1] + mbody[2] * ez[1],
        mbody[0] * ex[2] + mbody[1] * ey[2] + mbody[2] * ez[2],
    ]
}

/// `MathExtra::exyz_to_q`.
fn exyz_to_q(ex: &V3, ey: &V3, ez: &V3) -> [f64; 4] {
    let q0sq = 0.25 * (ex[0] + ey[1] + ez[2] + 1.0);
    let q1sq = q0sq - 0.5 * (ey[1] + ez[2]);
    let q2sq = q0sq - 0.5 * (ex[0] + ez[2]);
    let q3sq = q0sq - 0.5 * (ex[0] + ey[1]);
    let mut q = [0.0; 4];
    if q0sq >= 0.25 {
        q[0] = q0sq.sqrt();
        q[1] = (ey[2] - ez[1]) / (4.0 * q[0]);
        q[2] = (ez[0] - ex[2]) / (4.0 * q[0]);
        q[3] = (ex[1] - ey[0]) / (4.0 * q[0]);
    } else if q1sq >= 0.25 {
        q[1] = q1sq.sqrt();
        q[0] = (ey[2] - ez[1]) / (4.0 * q[1]);
        q[2] = (ey[0] + ex[1]) / (4.0 * q[1]);
        q[3] = (ex[2] + ez[0]) / (4.0 * q[1]);
    } else if q2sq >= 0.25 {
        q[2] = q2sq.sqrt();
        q[0] = (ez[0] - ex[2]) / (4.0 * q[2]);
        q[1] = (ey[0] + ex[1]) / (4.0 * q[2]);
        q[3] = (ez[1] + ey[2]) / (4.0 * q[2]);
    } else if q3sq >= 0.25 {
        q[3] = q3sq.sqrt();
        q[0] = (ex[1] - ey[0]) / (4.0 * q[3]);
        q[1] = (ez[0] + ex[2]) / (4.0 * q[3]);
        q[2] = (ez[1] + ey[2]) / (4.0 * q[3]);
    }
    qnormalize(&mut q);
    q
}

/// `MathExtra::q_to_exyz`.
fn q_to_exyz(q: &[f64; 4]) -> (V3, V3, V3) {
    let ex = [
        q[0] * q[0] + q[1] * q[1] - q[2] * q[2] - q[3] * q[3],
        2.0 * (q[1] * q[2] + q[0] * q[3]),
        2.0 * (q[1] * q[3] - q[0] * q[2]),
    ];
    let ey = [
        2.0 * (q[1] * q[2] - q[0] * q[3]),
        q[0] * q[0] - q[1] * q[1] + q[2] * q[2] - q[3] * q[3],
        2.0 * (q[2] * q[3] + q[0] * q[1]),
    ];
    let ez = [
        2.0 * (q[1] * q[3] + q[0] * q[2]),
        2.0 * (q[2] * q[3] - q[0] * q[1]),
        q[0] * q[0] - q[1] * q[1] - q[2] * q[2] + q[3] * q[3],
    ];
    (ex, ey, ez)
}

/// `MathExtra::matvec(ex, ey, ez, v)` (= `local_coosys_to_cartesian`).
fn matvec(ex: &V3, ey: &V3, ez: &V3, v: &V3) -> V3 {
    [
        ex[0] * v[0] + ey[0] * v[1] + ez[0] * v[2],
        ex[1] * v[0] + ey[1] * v[1] + ez[1] * v[2],
        ex[2] * v[0] + ey[2] * v[1] + ez[2] * v[2],
    ]
}

/// `MathExtraLiggghts::local_coosys_to_cartesian`.
fn local_to_cartesian(local: &V3, ex: &V3, ey: &V3, ez: &V3) -> V3 {
    [
        local[0] * ex[0] + local[1] * ey[0] + local[2] * ez[0],
        local[0] * ex[1] + local[1] * ey[1] + local[2] * ez[1],
        local[0] * ex[2] + local[1] * ey[2] + local[2] * ez[2],
    ]
}

/// `MathExtraLiggghts::cartesian_coosys_to_local_orthogonal`.
fn cartesian_to_local_orthogonal(g: &V3, ex: &V3, ey: &V3, ez: &V3) -> Result<V3, DemError> {
    let (d1, d2, d3) = (dot3(ex, ey), dot3(ey, ez), dot3(ez, ex));
    if d1 > TOLERANCE_ORTHO || d2 > TOLERANCE_ORTHO || d3 > TOLERANCE_ORTHO {
        return Err(DemError::InvalidInput(
            "Insufficient accuracy: using MathExtraLiggghts::cartesian_coosys_to_local_orthogonal() for \
             non-orthogonal coo-sys"
                .into(),
        ));
    }
    Ok([
        g[0] * ex[0] + g[1] * ex[1] + g[2] * ex[2],
        g[0] * ey[0] + g[1] * ey[1] + g[2] * ey[2],
        g[0] * ez[0] + g[1] * ez[1] + g[2] * ez[2],
    ])
}

/// `fix particletemplate/multisphere SEED atom_type T density constant RHO
/// nspheres N ntry NTRY spheres x1 y1 z1 r1 ... type K`, with upstream's
/// Monte-Carlo set-up (`post_create`) done at construction.
#[derive(Debug, Clone, PartialEq)]
pub struct ParticleTemplateMultisphere {
    /// Seed (`random_insertion` and `random_mc` both start from it).
    pub seed: i32,
    /// `atom_type` of every member sphere (1-based).
    pub atom_type: usize,
    /// Clump `type` (1-based; consecutive over the templates of a deck).
    pub clump_type: usize,
    density: f64,
    ntry: usize,
    /// Sphere centres, after the shift to the centre of mass `[m]`.
    x_sphere: Vec<V3>,
    r_sphere: Vec<f64>,
    x_bound: V3,
    r_bound: f64,
    volume_expect: f64,
    mass_expect: f64,
    inertia: V3,
    ex: V3,
    ey: V3,
    ez: V3,
    displace: Vec<V3>,
    xcm_to_xb_body: V3,
    volumeweight: Vec<f64>,
    no_overlap: bool,
    overlap_slightly: bool,
    random_insertion_state: i32,
    /// `fflag x y z` (default all on).
    fflag: [bool; 3],
    /// `tflag x y z` (default all on).
    tflag: [bool; 3],
    /// `spheres_different_types`: each sphere's atom type (1-based). Only
    /// `max_type` / `min_type` read it, as upstream (see
    /// [`ParticleTemplateMultisphere::from_clump_file_different_types`]).
    atom_type_sphere: Option<Vec<usize>>,
    /// `bonded yes|no` (default `no`; see [`ParticleTemplateMultisphere::with_bonded`]).
    bonded: bool,
    /// Whether `post_create` ran `FixTemplateMultiplespheres::post_create`,
    /// where `bonded` creates `fix property/atom bond_random_id`: every path
    /// except `use_density`.
    base_post_create: bool,
}

impl ParticleTemplateMultisphere {
    /// Build the template and run upstream's `post_create` (bounding sphere,
    /// Monte-Carlo volume, centre of mass and inertia, eigensystem, body-frame
    /// displacements, volume weights).
    ///
    /// `spheres` are the inline `spheres x y z r` entries (positions in the
    /// template frame `[m]`, radii); `ntry` the Monte-Carlo sample count.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] for what upstream rejects: an invalid seed,
    /// fewer than 2 spheres, a non-positive radius or density, `ntry < 1000`,
    /// `type < 1`, `atom_type < 1`, a failed bounding sphere, an inertia tensor
    /// not symmetric to `1e-6` ("Boost ntry"), a Jacobi failure, or
    /// non-orthogonal axes.
    pub fn new(
        seed: i32,
        atom_type: usize,
        density: MassDensity,
        ntry: usize,
        spheres: &[(Vec3, Length)],
        clump_type: usize,
    ) -> Result<Self, DemError> {
        let (mut t, mut random_mc, mut x_min, mut x_max) =
            Self::parse(seed, atom_type, density, ntry, spheres, clump_type)?;
        // FixTemplateMultisphere::post_create, no `mass`/`inertia_tensor` given
        t.calc_bounding_sphere(&mut random_mc)?;
        t.calc_center_of_mass(&mut random_mc, &mut x_min, &mut x_max);
        t.check_overlap();
        let mut moi = t.calc_inertia(&mut random_mc, &x_min, &x_max)?;
        t.calc_eigensystem(&mut moi)?;
        t.calc_displace_xcm_x_body()?;
        t.calc_volumeweight(&mut random_mc, &x_min, &x_max);
        Ok(t)
    }

    /// The template keywords `mass M inertia_tensor Ixx Ixy Ixz Iyy Iyz Izz`
    /// with `use_volume` or `use_density` (gh:#1415): the clump's mass and
    /// inertia tensor are given, not integrated. Upstream needs both or neither
    /// (`FixTemplateMultisphere` ctor), and one of the two modes; this
    /// constructor takes all three, so neither error can arise.
    ///
    /// `FixTemplateMultisphere::post_create` (LIGGGHTS-PUBLIC `3d5c00f2`):
    /// * [`MassSource::UseVolume`]: the bounding sphere, the Monte-Carlo volume
    ///   and centre of mass (the spheres shift to it), the overlap check; then
    ///   the given mass replaces `volume * density`, and the density becomes
    ///   `mass / volume` (`pdf_density->set_params`), which each member atom
    ///   then carries.
    /// * [`MassSource::UseDensity`]: the bounding sphere and the overlap check
    ///   only. No Monte-Carlo volume and **no shift to a centre of mass**: the
    ///   template origin is the body's centre. The volume is `mass / density`.
    ///
    /// Both then diagonalise the given tensor (`calc_eigensystem`), compute the
    /// body-frame displacements and the volume weights, in that order and on
    /// the template's one `random_mc` stream, so the stream is consumed as
    /// upstream consumes it. `inertia_tensor` is `[Ixx, Ixy, Ixz, Iyy, Iyz,
    /// Izz]`, mirrored to the lower triangle as upstream does.
    ///
    /// # Errors
    ///
    /// Everything [`ParticleTemplateMultisphere::new`] rejects except the
    /// Monte-Carlo inertia symmetry check, which does not run here.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_mass_inertia(
        seed: i32,
        atom_type: usize,
        density: MassDensity,
        ntry: usize,
        spheres: &[(Vec3, Length)],
        clump_type: usize,
        mass: Mass,
        inertia_tensor: [MomentOfInertia; 6],
        source: MassSource,
    ) -> Result<Self, DemError> {
        let (mut t, mut random_mc, mut x_min, mut x_max) =
            Self::parse(seed, atom_type, density, ntry, spheres, clump_type)?;
        let mass = mass.get::<kilogram>();
        let i = inertia_tensor.map(|v| v.get::<kilogram_square_meter>());
        let mut moi = [[i[0], i[1], i[2]], [i[1], i[3], i[4]], [i[2], i[4], i[5]]];
        match source {
            MassSource::UseVolume => {
                t.calc_bounding_sphere(&mut random_mc)?;
                t.calc_center_of_mass(&mut random_mc, &mut x_min, &mut x_max);
                t.check_overlap();
                t.mass_expect = mass;
                t.density = t.mass_expect / t.volume_expect;
            }
            MassSource::UseDensity => {
                // no `FixTemplateMultiplespheres::post_create` on this path
                t.base_post_create = false;
                t.calc_bounding_sphere(&mut random_mc)?;
                t.check_overlap();
                t.mass_expect = mass;
                t.volume_expect = t.mass_expect / t.density;
            }
        }
        t.calc_eigensystem(&mut moi)?;
        t.calc_displace_xcm_x_body()?;
        t.calc_volumeweight(&mut random_mc, &x_min, &x_max);
        Ok(t)
    }

    /// The constructor's argument checks and sphere list (the template frame,
    /// before any shift), the `random_mc` stream, and the sphere bounding box
    /// `(x_min, x_max)` (`FixTemplateMultiplespheres` ctor).
    fn parse(
        seed: i32,
        atom_type: usize,
        density: MassDensity,
        ntry: usize,
        spheres: &[(Vec3, Length)],
        clump_type: usize,
    ) -> Result<(Self, crate::insert::RanPark, V3, V3), DemError> {
        let random_insertion = crate::insert::RanPark::checked(seed)?;
        let random_mc = crate::insert::RanPark::checked(seed)?;
        let density = density.get::<kilogram_per_cubic_meter>();
        if atom_type < 1 {
            return Err(DemError::InvalidInput("invalid atom type (must be >=1)".into()));
        }
        if !(density > 0.0) {
            return Err(DemError::InvalidInput("density must be > 0".into()));
        }
        let n = spheres.len();
        if n < 2 {
            return Err(DemError::InvalidInput("illegal number of spheres".into()));
        }
        if ntry < 1000 {
            return Err(DemError::InvalidInput("ntry is too low".into()));
        }
        if clump_type < 1 {
            return Err(DemError::InvalidInput("have to provide a type >=1".into()));
        }
        let mut x_min = [LARGE; 3];
        let mut x_max = [-LARGE; 3];
        let mut x_sphere = Vec::with_capacity(n);
        let mut r_sphere = Vec::with_capacity(n);
        for (x, r) in spheres {
            // `force->cg(atom_type)` is 1 (no coarse graining here); upstream
            // multiplies by it, which is exact.
            let r = r.get::<meter>();
            if r <= 0.0 {
                return Err(DemError::InvalidInput("radius must be >0".into()));
            }
            let x = v3(*x);
            for j in 0..3 {
                if x[j] - r < x_min[j] {
                    x_min[j] = x[j] - r;
                }
                if x[j] + r > x_max[j] {
                    x_max[j] = x[j] + r;
                }
            }
            x_sphere.push(x);
            r_sphere.push(r);
        }
        let t = Self {
            seed,
            atom_type,
            clump_type,
            density,
            ntry,
            x_sphere,
            r_sphere,
            x_bound: [0.0; 3],
            r_bound: LARGE,
            volume_expect: 0.0,
            mass_expect: 0.0,
            inertia: [0.0; 3],
            ex: [0.0; 3],
            ey: [0.0; 3],
            ez: [0.0; 3],
            displace: Vec::new(),
            xcm_to_xb_body: [0.0; 3],
            volumeweight: Vec::new(),
            no_overlap: true,
            overlap_slightly: true,
            random_insertion_state: random_insertion.state(),
            fflag: [true; 3],
            tflag: [true; 3],
            atom_type_sphere: None,
            bonded: false,
            base_post_create: true,
        };
        Ok((t, random_mc, x_min, x_max))
    }

    /// `spheres file <f> scale <s>` (gh:#1415): the template's spheres read
    /// from the text of a clump file, as `InputMultisphere::clmpfile` reads it,
    /// then built as [`ParticleTemplateMultisphere::new`] builds an inline list.
    ///
    /// Each line is `x y z r`, read with `atof`. A `#` starts a comment
    /// (`Input::parse_nonlammps`), empty and comment-only lines are skipped,
    /// and words after the fourth are ignored. Each radius is checked `> 0`,
    /// then `r *= scale*cg` and `x *= scale*cg` (`cg = 1`, no coarse
    /// graining), in `FixTemplateMultiplespheres`' order. The bounding box
    /// comparison that follows is the inline path's.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] for what upstream rejects: `scale <= 0`,
    /// a line with fewer than 4 words, a number of lines other than
    /// `nspheres`, a radius `<= 0`; for a word `atof` would read as a number
    /// but Rust cannot parse (refused rather than guessed), and everything
    /// [`ParticleTemplateMultisphere::new`] rejects.
    #[allow(clippy::too_many_arguments)]
    pub fn from_clump_file(
        seed: i32,
        atom_type: usize,
        density: MassDensity,
        ntry: usize,
        nspheres: usize,
        text: &str,
        scale: f64,
        clump_type: usize,
    ) -> Result<Self, DemError> {
        let (spheres, _) = Self::read_clump_file(nspheres, text, scale, false)?;
        Self::new(seed, atom_type, density, ntry, &spheres, clump_type)
    }

    /// `spheres_different_types file <f> scale <s>` (gh:#1415): as
    /// [`ParticleTemplateMultisphere::from_clump_file`], with a fifth word per
    /// line, the sphere's atom type (`atoi(arg[4])` in
    /// `InputMultisphere::clmpfile`; a line with fewer than 5 words is an
    /// error).
    ///
    /// **Upstream gives every member atom of a clump the template's
    /// `atom_type` all the same** (LIGGGHTS-PUBLIC `3d5c00f2`):
    /// `FixTemplateMultisphere::randomize_ptilist` does not copy the sphere
    /// types into `pti->atom_type_vector`, and
    /// `ParticleToInsertMultisphere::insert` calls `create_atom(atom_type,
    /// ...)` for each sphere. The sphere types only set
    /// [`ParticleTemplateMultisphere::max_type`] / `min_type` (`maxtype()`,
    /// `mintype()`; `FixInsert`'s `type_max` / `type_min` for its material
    /// checks). The port does the same; `in.ms_dtypes` confirms upstream's
    /// atom types.
    ///
    /// # Errors
    ///
    /// Everything [`ParticleTemplateMultisphere::from_clump_file`] rejects;
    /// a line with fewer than 5 words; a type word that is not an integer,
    /// or is < 1 (refused rather than guessed: `atoi` would read `2.5` as 2
    /// and `x` as 0).
    #[allow(clippy::too_many_arguments)]
    pub fn from_clump_file_different_types(
        seed: i32,
        atom_type: usize,
        density: MassDensity,
        ntry: usize,
        nspheres: usize,
        text: &str,
        scale: f64,
        clump_type: usize,
    ) -> Result<Self, DemError> {
        let (spheres, types) = Self::read_clump_file(nspheres, text, scale, true)?;
        let mut t = Self::new(seed, atom_type, density, ntry, &spheres, clump_type)?;
        t.atom_type_sphere = Some(types);
        Ok(t)
    }

    /// `maxtype()`: the largest sphere type of a `spheres_different_types`
    /// template, else `atom_type`.
    #[must_use]
    pub fn max_type(&self) -> usize {
        match &self.atom_type_sphere {
            Some(t) => t.iter().copied().fold(0, usize::max),
            None => self.atom_type,
        }
    }

    /// `mintype()`: the smallest sphere type, else `atom_type`.
    #[must_use]
    pub fn min_type(&self) -> usize {
        match &self.atom_type_sphere {
            Some(t) => t.iter().copied().fold(usize::MAX, usize::min),
            None => self.atom_type,
        }
    }

    /// `InputMultisphere::clmpfile` and the scaling of
    /// `FixTemplateMultiplespheres` (`cg = 1`): the scaled spheres and, with
    /// `with_types`, each line's fifth word as a type.
    fn read_clump_file(
        nspheres: usize,
        text: &str,
        scale: f64,
        with_types: bool,
    ) -> Result<(Vec<(Vec3, Length)>, Vec<usize>), DemError> {
        if !(scale > 0.0) {
            return Err(DemError::InvalidInput("scale factor must be >0".into()));
        }
        let mut raw: Vec<(V3, f64)> = Vec::with_capacity(nspheres);
        let mut types = Vec::new();
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("");
            let words: Vec<&str> = line.split_whitespace().collect();
            if words.is_empty() {
                continue;
            }
            if raw.len() >= nspheres {
                return Err(DemError::InvalidInput(
                    "Number of clumps in file larger than number specified".into(),
                ));
            }
            if !with_types && words.len() < 4 {
                return Err(DemError::InvalidInput(
                    "Not enough arguments in one line of clump file, need to specify [xcoo ycoo zcoo radius] in each line"
                        .into(),
                ));
            }
            if with_types && words.len() < 5 {
                return Err(DemError::InvalidInput(
                    "Not enough arguments in one line of clump file, need to specify [xcoo ycoo zcoo radius type] in each line"
                        .into(),
                ));
            }
            let mut v = [0.0; 4];
            for k in 0..4 {
                v[k] = words[k].parse::<f64>().map_err(|_| {
                    DemError::InvalidInput(format!("clump file: '{}' is not a number", words[k]))
                })?;
            }
            if with_types {
                let t = words[4].parse::<usize>().ok().filter(|&t| t >= 1).ok_or_else(|| {
                    DemError::InvalidInput(format!("clump file: '{}' is not an atom type >= 1", words[4]))
                })?;
                types.push(t);
            }
            raw.push(([v[0], v[1], v[2]], v[3]));
        }
        if raw.len() != nspheres {
            return Err(DemError::InvalidInput(
                "Number of clumps in file does not match number of clumps that were specified".into(),
            ));
        }
        let cg = 1.0;
        let mut spheres = Vec::with_capacity(nspheres);
        for (x, r) in raw {
            if r <= 0.0 {
                return Err(DemError::InvalidInput("radius must be > 0".into()));
            }
            let f = scale * cg;
            let r = r * f;
            let x = [x[0] * f, x[1] * f, x[2] * f];
            spheres.push((vec3(x), Length::new::<meter>(r)));
        }
        Ok((spheres, types))
    }

    /// `calc_bounding_sphere`: 200 randomised Ritter passes, keep the smallest.
    fn calc_bounding_sphere(&mut self, rng: &mut crate::insert::RanPark) -> Result<(), DemError> {
        let n = self.x_sphere.len();
        self.r_bound = LARGE;
        let mut visited = vec![false; n];
        for _shuffle in 0..N_SHUFFLE_BOUND {
            for v in visited.iter_mut() {
                *v = false;
            }
            let mut isphere: i64 = -1;
            let mut nvisited = 0;
            while isphere < 0 || visited[isphere as usize] || isphere as usize >= n {
                isphere = (rng.uniform() * n as f64) as i64;
            }
            nvisited += 1;
            visited[isphere as usize] = true;
            let mut x_bound_temp = self.x_sphere[isphere as usize];
            let mut rbound_temp = self.r_sphere[isphere as usize];
            while nvisited < n {
                while isphere < 0 || visited[isphere as usize] || isphere as usize >= n {
                    isphere = (rng.uniform() * n as f64) as i64;
                }
                nvisited += 1;
                let is = isphere as usize;
                visited[is] = true;
                let mut d = [
                    self.x_sphere[is][0] - x_bound_temp[0],
                    self.x_sphere[is][1] - x_bound_temp[1],
                    self.x_sphere[is][2] - x_bound_temp[2],
                ];
                let dist = dot3(&d, &d).sqrt();
                if dist + self.r_sphere[is] > rbound_temp {
                    let fact = (dist + self.r_sphere[is] - rbound_temp) / (2.0 * dist);
                    d[0] *= fact;
                    d[1] *= fact;
                    d[2] *= fact;
                    x_bound_temp[0] += d[0];
                    x_bound_temp[1] += d[1];
                    x_bound_temp[2] += d[2];
                    rbound_temp += dot3(&d, &d).sqrt();
                }
            }
            if rbound_temp < self.r_bound {
                self.r_bound = rbound_temp;
                self.x_bound = x_bound_temp;
            }
        }
        for i in 0..n {
            let t = [
                self.x_bound[0] - self.x_sphere[i][0],
                self.x_bound[1] - self.x_sphere[i][1],
                self.x_bound[2] - self.x_sphere[i][2],
            ];
            if dot3(&t, &t).sqrt() > self.r_bound {
                return Err(DemError::InvalidInput(
                    "Bounding sphere calculation for template failed".into(),
                ));
            }
        }
        Ok(())
    }

    /// `dist_sqr(j, xtest)`.
    fn dist_sqr(&self, j: usize, x: &V3) -> f64 {
        let s = &self.x_sphere[j];
        let mut d = 0.0;
        d += (x[0] - s[0]) * (x[0] - s[0]);
        d += (x[1] - s[1]) * (x[1] - s[1]);
        d += (x[2] - s[2]) * (x[2] - s[2]);
        d
    }

    /// `generate_xtry`.
    fn generate_xtry(rng: &mut crate::insert::RanPark, x_min: &V3, x_max: &V3) -> V3 {
        let a = x_min[0] + (x_max[0] - x_min[0]) * rng.uniform();
        let b = x_min[1] + (x_max[1] - x_min[1]) * rng.uniform();
        let c = x_min[2] + (x_max[2] - x_min[2]) * rng.uniform();
        [a, b, c]
    }

    fn bbox_volume(x_min: &V3, x_max: &V3) -> [f64; 3] {
        [x_max[0] - x_min[0], x_max[1] - x_min[1], x_max[2] - x_min[2]]
    }

    /// `calc_center_of_mass`: MC volume and centre of mass; shifts the spheres,
    /// the box and the bounding-sphere centre to the centre of mass.
    fn calc_center_of_mass(&mut self, rng: &mut crate::insert::RanPark, x_min: &mut V3, x_max: &mut V3) {
        let mut nsuccess: i64 = 0;
        let mut xcm = [0.0; 3];
        for _ in 0..self.ntry {
            let x_try = Self::generate_xtry(rng, x_min, x_max);
            let mut already_checked = false;
            for j in 0..self.x_sphere.len() {
                let dist_j_sqr = self.dist_sqr(j, &x_try);
                if already_checked {
                    break;
                }
                if dist_j_sqr < self.r_sphere[j] * self.r_sphere[j] {
                    let ns = nsuccess as f64;
                    let ns1 = (nsuccess + 1) as f64;
                    xcm[0] = (xcm[0] * ns + x_try[0]) / ns1;
                    xcm[1] = (xcm[1] * ns + x_try[1]) / ns1;
                    xcm[2] = (xcm[2] * ns + x_try[2]) / ns1;
                    nsuccess += 1;
                    already_checked = true;
                }
            }
        }
        let d = Self::bbox_volume(x_min, x_max);
        self.volume_expect = nsuccess as f64 / self.ntry as f64 * d[0] * d[1] * d[2];
        self.mass_expect = self.volume_expect * self.density;
        for s in &mut self.x_sphere {
            s[0] -= xcm[0];
            s[1] -= xcm[1];
            s[2] -= xcm[2];
        }
        for k in 0..3 {
            x_min[k] -= xcm[k];
            x_max[k] -= xcm[k];
            self.x_bound[k] -= xcm[k];
        }
    }

    /// `check_overlap`.
    fn check_overlap(&mut self) {
        let (min_f, max_f) = (1.0000001, 1.01);
        let n = self.x_sphere.len();
        self.overlap_slightly = true;
        self.no_overlap = true;
        let mut slight = vec![false; n];
        for i in 0..n {
            for j in (i + 1)..n {
                let d = [
                    self.x_sphere[i][0] - self.x_sphere[j][0],
                    self.x_sphere[i][1] - self.x_sphere[j][1],
                    self.x_sphere[i][2] - self.x_sphere[j][2],
                ];
                let dist = dot3(&d, &d).sqrt();
                let rs = self.r_sphere[i] + self.r_sphere[j];
                if dist < rs {
                    self.no_overlap = false;
                }
                if dist > min_f * rs && dist < max_f * rs {
                    slight[i] = true;
                    slight[j] = true;
                }
            }
        }
        if slight.iter().any(|s| !s) {
            self.overlap_slightly = false;
        }
    }

    /// `calc_inertia`: MC inertia tensor about the (zero) centre of mass,
    /// checked for symmetry and symmetrised.
    fn calc_inertia(
        &self,
        rng: &mut crate::insert::RanPark,
        x_min: &V3,
        x_max: &V3,
    ) -> Result<[[f64; 3]; 3], DemError> {
        let mut moi = [[0.0; 3]; 3];
        let xcm = [0.0; 3];
        for _ in 0..self.ntry {
            let x = Self::generate_xtry(rng, x_min, x_max);
            let mut already_checked = false;
            for j in 0..self.x_sphere.len() {
                if already_checked {
                    break;
                }
                let dsq = self.dist_sqr(j, &x);
                if dsq < self.r_sphere[j] * self.r_sphere[j] {
                    let (a, b, c) = (x[0] - xcm[0], x[1] - xcm[1], x[2] - xcm[2]);
                    moi[0][0] += (x[1] - xcm[1]) * (x[1] - xcm[1]) + (x[2] - xcm[2]) * (x[2] - xcm[2]);
                    moi[0][1] -= a * b;
                    moi[0][2] -= a * c;
                    moi[1][0] -= b * a;
                    moi[1][1] += (x[0] - xcm[0]) * (x[0] - xcm[0]) + (x[2] - xcm[2]) * (x[2] - xcm[2]);
                    moi[1][2] -= b * c;
                    moi[2][0] -= c * a;
                    moi[2][1] -= c * b;
                    moi[2][2] += (x[0] - xcm[0]) * (x[0] - xcm[0]) + (x[1] - xcm[1]) * (x[1] - xcm[1]);
                    already_checked = true;
                }
            }
        }
        let d = Self::bbox_volume(x_min, x_max);
        for row in &mut moi {
            for e in row.iter_mut() {
                *e *= self.density / self.ntry as f64 * d[0] * d[1] * d[2];
            }
        }
        if (moi[0][1] / moi[1][0] - 1.0).abs() > TOLERANCE_SYM
            || (moi[0][2] / moi[2][0] - 1.0).abs() > TOLERANCE_SYM
            || (moi[2][1] / moi[1][2] - 1.0).abs() > TOLERANCE_SYM
        {
            return Err(DemError::InvalidInput(
                "Fix particletemplate/multisphere:Error when calculating inertia_ tensor : Not enough \
                 accuracy. Boost ntry."
                    .into(),
            ));
        }
        moi[0][1] = (moi[0][1] + moi[1][0]) / 2.0;
        moi[1][0] = moi[0][1];
        moi[0][2] = (moi[0][2] + moi[2][0]) / 2.0;
        moi[2][0] = moi[0][2];
        moi[2][1] = (moi[2][1] + moi[1][2]) / 2.0;
        moi[1][2] = moi[2][1];
        Ok(moi)
    }

    /// `calc_eigensystem` (as `fix_rigid.cpp::init`).
    fn calc_eigensystem(&mut self, moi: &mut [[f64; 3]; 3]) -> Result<(), DemError> {
        let (mut inertia, ev) = jacobi(moi).ok_or_else(|| {
            DemError::InvalidInput("Insufficient Jacobi rotations for rigid body".into())
        })?;
        self.ex = [ev[0][0], ev[1][0], ev[2][0]];
        self.ey = [ev[0][1], ev[1][1], ev[2][1]];
        self.ez = [ev[0][2], ev[1][2], ev[2][2]];
        let max = inertia[0].max(inertia[1]);
        let max = max.max(inertia[2]);
        for k in 0..3 {
            if inertia[k] < EPSILON_MOI * max {
                inertia[k] = 0.0;
            }
        }
        self.inertia = inertia;
        let ez = cross3(&self.ex, &self.ey);
        if dot3(&ez, &self.ez) < 0.0 {
            for c in &mut self.ez {
                *c *= -1.0;
            }
        }
        Ok(())
    }

    /// `calc_displace_xcm_x_body`.
    fn calc_displace_xcm_x_body(&mut self) -> Result<(), DemError> {
        let mut displace = Vec::with_capacity(self.x_sphere.len());
        for s in &self.x_sphere {
            displace.push(cartesian_to_local_orthogonal(s, &self.ex, &self.ey, &self.ez)?);
        }
        self.displace = displace;
        self.xcm_to_xb_body = cartesian_to_local_orthogonal(&self.x_bound, &self.ex, &self.ey, &self.ez)?;
        Ok(())
    }

    /// `calc_volumeweight`: each sphere's share of the clump volume
    /// (`0.5 + 0.5 * hits only in j / hits in j` in upstream's comment; the code
    /// splits a point shared by `n` spheres as `1/n` each).
    fn calc_volumeweight(&mut self, rng: &mut crate::insert::RanPark, x_min: &V3, x_max: &V3) {
        let n = self.x_sphere.len();
        let mut vw = vec![0.0; n];
        let mut hits_j = vec![false; n];
        let mut hits_tot: i64 = 0;
        for _ in 0..self.ntry {
            let x = Self::generate_xtry(rng, x_min, x_max);
            let mut n_hits = 0.0_f64;
            for h in hits_j.iter_mut() {
                *h = false;
            }
            for j in 0..n {
                if self.dist_sqr(j, &x) < self.r_sphere[j] * self.r_sphere[j] {
                    hits_j[j] = true;
                    n_hits += 1.0;
                }
            }
            for j in 0..n {
                if hits_j[j] {
                    vw[j] += 1.0 / n_hits;
                }
            }
            if n_hits > 0.0 {
                hits_tot += 1;
            }
        }
        if hits_tot > 0 {
            let inv = 1.0 / hits_tot as f64;
            for w in &mut vw {
                *w *= inv;
            }
        }
        self.volumeweight = vw;
    }

    /// Monte-Carlo clump volume `[m^3]` (`volume_expect`, `volexpect()`).
    #[must_use]
    pub fn volexpect(&self) -> f64 {
        self.volume_expect
    }

    /// Clump mass `[kg]` (`mass_expect`, `massexpect()`).
    #[must_use]
    pub fn massexpect(&self) -> f64 {
        self.mass_expect
    }

    /// Radius of the bounding sphere `[m]` (`max_r_bound()`).
    #[must_use]
    pub fn r_bound(&self) -> f64 {
        self.r_bound
    }

    /// Largest member-sphere radius `[m]` (`max_rad()`).
    #[must_use]
    pub fn max_rad(&self) -> f64 {
        self.r_sphere.iter().fold(0.0, |m, &r| if m < r { r } else { m })
    }

    /// Principal moments of inertia `[kg m^2]` (`inertia_`).
    #[must_use]
    pub fn principal_moments(&self) -> [f64; 3] {
        self.inertia
    }

    /// Per-sphere volume weight (`volumeweight_`, sums to 1).
    #[must_use]
    pub fn volume_weights(&self) -> &[f64] {
        &self.volumeweight
    }

    /// Whether no two member spheres overlap (`no_overlap`).
    #[must_use]
    pub fn no_overlap(&self) -> bool {
        self.no_overlap
    }

    /// Density `[kg/m^3]`.
    #[must_use]
    pub fn density(&self) -> f64 {
        self.density
    }

    /// State of the template's `random_insertion` stream (the distinct-seeds
    /// rule of `fix insert`).
    pub(crate) fn seed_state(&self) -> i32 {
        self.random_insertion_state
    }

    /// `randomize_ptilist` for one entry.
    pub(crate) fn to_insert(&self) -> ClumpToInsert {
        ClumpToInsert {
            radius: self.r_sphere.clone(),
            displace: self.displace.clone(),
            volumeweight: self.volumeweight.clone(),
            x_ins: self.x_sphere.clone(),
            inertia: self.inertia,
            ex: self.ex,
            ey: self.ey,
            ez: self.ez,
            xcm_to_xbound: self.xcm_to_xb_body,
            r_bound: self.r_bound,
            mass: self.mass_expect,
            density: self.density,
            atom_type: self.atom_type,
            clump_type: self.clump_type,
            xcm: [0.0; 3],
            v: [0.0; 3],
            omega: [0.0; 3],
            fflag: self.fflag,
            tflag: self.tflag,
        }
    }

    /// The template keyword `fflag x y z` (`on` = `true`; default all on):
    /// a clump of this template takes no force half-kick on an `off` axis of
    /// `vcm` (`FixMultisphere::initial_integrate` / `final_integrate`), so it
    /// keeps its velocity component there (gh:#1415).
    #[must_use]
    pub fn with_fflag(mut self, fflag: [bool; 3]) -> Self {
        self.fflag = fflag;
        self
    }

    /// The template keyword `tflag x y z` (default all on): no torque
    /// half-kick on an `off` axis of the space-frame angular momentum.
    #[must_use]
    pub fn with_tflag(mut self, tflag: [bool; 3]) -> Self {
        self.tflag = tflag;
        self
    }

    /// The template keyword `bonded yes|no` (gh:#1415, fourth pass).
    ///
    /// Upstream (`FixTemplateMultiplespheres`, LIGGGHTS-PUBLIC `3d5c00f2`)
    /// parses it in the base class, and its `post_create` then creates
    /// `fix property/atom bond_random_id` (scalar, default `-1.`). For a clump
    /// that is all it does: `FixTemplateMultisphere::randomize_ptilist`, the
    /// multisphere override, never sets the property and draws no random
    /// number for it, so every member atom keeps `-1` and the dynamics do not
    /// change. With `use_density` the base `post_create` is not called and
    /// the property is never created. Read it with
    /// [`crate::granular_system::GranularSystem::bond_random_id`].
    ///
    /// (Upstream's parser stops its base loop after `bonded` (no `hasargs`),
    /// so in a deck it must follow `spheres` and precede `type`.)
    #[must_use]
    pub fn with_bonded(mut self, bonded: bool) -> Self {
        self.bonded = bonded;
        self
    }

    /// Whether this template created `fix property/atom bond_random_id`:
    /// `bonded yes` on any path but `use_density` (see
    /// [`ParticleTemplateMultisphere::with_bonded`]).
    #[must_use]
    pub fn creates_bond_random_id(&self) -> bool {
        self.bonded && self.base_post_create
    }
}

/// One clump to insert (`ParticleToInsertMultisphere`).
#[derive(Debug, Clone, PartialEq)]
pub struct ClumpToInsert {
    radius: Vec<f64>,
    displace: Vec<V3>,
    volumeweight: Vec<f64>,
    x_ins: Vec<V3>,
    inertia: V3,
    ex: V3,
    ey: V3,
    ez: V3,
    xcm_to_xbound: V3,
    /// Bounding radius `[m]` (`r_bound_ins`).
    pub(crate) r_bound: f64,
    /// Clump mass `[kg]` (`mass_ins`).
    pub(crate) mass: f64,
    density: f64,
    atom_type: usize,
    clump_type: usize,
    xcm: V3,
    v: V3,
    omega: V3,
    fflag: [bool; 3],
    tflag: [bool; 3],
}

impl ClumpToInsert {
    /// `set_x_v_omega` (no overlap check): rotate the axes by `quat`, place the
    /// spheres about `x`.
    pub(crate) fn set_x_v_omega(&mut self, x: Vec3, v: Vec3, omega: Vec3, quat: [f64; 4]) {
        let xc = v3(x);
        self.xcm = xc;
        self.v = v3(v);
        self.omega = v3(omega);
        self.ex = vec_quat_rotate(&self.ex, &quat);
        self.ey = vec_quat_rotate(&self.ey, &quat);
        self.ez = vec_quat_rotate(&self.ez, &quat);
        for j in 0..self.radius.len() {
            let g = local_to_cartesian(&self.displace[j], &self.ex, &self.ey, &self.ez);
            self.x_ins[j] = [xc[0] + g[0], xc[1] + g[1], xc[2] + g[2]];
        }
    }

    /// `check_near_set_x_v_omega`: place the spheres at `x` with the axes
    /// rotated by `quat`; if none overlaps an existing sphere (`has_overlap`),
    /// accept, return the sphere positions and radii to add to the overlap
    /// list. `None` (and nothing changed) on overlap.
    pub(crate) fn check_near_set_x_v_omega(
        &mut self,
        x: Vec3,
        v: Vec3,
        omega: Vec3,
        quat: [f64; 4],
        has_overlap: impl Fn(Vec3, f64) -> bool,
    ) -> Option<Vec<(Vec3, f64)>> {
        let xc = v3(x);
        let ex_t = vec_quat_rotate(&self.ex, &quat);
        let ey_t = vec_quat_rotate(&self.ey, &quat);
        let ez_t = vec_quat_rotate(&self.ez, &quat);
        let mut x_try = Vec::with_capacity(self.radius.len());
        for j in 0..self.radius.len() {
            let g = local_to_cartesian(&self.displace[j], &ex_t, &ey_t, &ez_t);
            x_try.push([xc[0] + g[0], xc[1] + g[1], xc[2] + g[2]]);
        }
        for j in 0..self.radius.len() {
            if has_overlap(vec3(x_try[j]), self.radius[j]) {
                return None;
            }
        }
        self.x_ins = x_try;
        self.xcm = xc;
        self.v = v3(v);
        self.omega = v3(omega);
        self.ex = ex_t;
        self.ey = ey_t;
        self.ez = ez_t;
        Some(
            (0..self.radius.len())
                .map(|j| (vec3(self.x_ins[j]), self.radius[j]))
                .collect(),
        )
    }

    /// The member atoms as `ParticleToInsertMultisphere::insert` creates them:
    /// at `x_ins`, at rest, each with the clump mass.
    pub(crate) fn atoms(&self) -> Vec<Particle> {
        (0..self.radius.len())
            .map(|j| Particle {
                position: vec3(self.x_ins[j]),
                velocity: Vec3::zero(),
                angular_velocity: Vec3::zero(),
                mass: self.mass,
                radius: self.radius[j],
                temperature: INSERTED_TEMPERATURE,
            })
            .collect()
    }

    /// Atom type of the member spheres (1-based).
    pub(crate) fn atom_type(&self) -> usize {
        self.atom_type
    }
}

/// One rigid body (the per-body containers of `Multisphere`).
#[derive(Debug, Clone, PartialEq)]
pub struct ClumpBody {
    /// Body id (`id_multisphere`, 1-based).
    pub id: usize,
    /// Centre of mass `[m]`.
    pub xcm: [f64; 3],
    /// Centre-of-mass velocity `[m/s]`.
    pub vcm: [f64; 3],
    /// Total force `[N]` (`fcm`).
    pub fcm: [f64; 3],
    /// Total torque about the centre of mass `[N m]`.
    pub torque: [f64; 3],
    /// Angular momentum, space frame `[kg m^2/s]`.
    pub angmom: [f64; 3],
    /// Angular velocity, space frame `[rad/s]`.
    pub omega: [f64; 3],
    /// Orientation quaternion `(w, i, j, k)`.
    pub quat: [f64; 4],
    /// Principal moments of inertia `[kg m^2]`.
    pub inertia: [f64; 3],
    /// Principal axes in space coordinates.
    pub ex: [f64; 3],
    /// Second principal axis.
    pub ey: [f64; 3],
    /// Third principal axis.
    pub ez: [f64; 3],
    /// Mass `[kg]` (`masstotal`).
    pub mass: f64,
    /// Density `[kg/m^3]`.
    pub density: f64,
    /// Clump type (1-based).
    pub clump_type: usize,
    /// Bounding radius `[m]`.
    pub r_bound: f64,
    /// Bounding-sphere centre relative to the centre of mass, body frame `[m]`.
    pub xcm_to_xbound: [f64; 3],
    /// Number of member atoms (`nrigid`).
    pub nrigid: usize,
    /// `fflag` (from the template; `true` = on).
    pub fflag: [bool; 3],
    /// `tflag` (from the template; `true` = on).
    pub tflag: [bool; 3],
    dragforce: [f64; 3],
    hdtorque: [f64; 3],
    /// Body temperature `[K]` (`temp_`): `T0` of `fix heat/gran` at
    /// insertion, `0` without one (`Multisphere::add_body`); see
    /// [`Multisphere::with_heat_transfer`].
    pub temp: f64,
    /// `temp_old_`: the body temperature before this step's update.
    temp_old: f64,
    /// `start_step_` (upstream `-1` = `None`): a body inserted by
    /// `fix insert/stream` rides its rail until this step
    /// ([`Multisphere::calc_n_steps`], [`Multisphere::release`]).
    pub start_step: Option<i64>,
    /// `v_integrate_`: the rail velocity `[m/s]`.
    v_integrate: [f64; 3],
}

impl ClumpBody {
    /// `Multisphere::x_bound`: the bounding-sphere centre in space `[m]`.
    #[must_use]
    pub fn x_bound(&self) -> Vec3 {
        let g = local_to_cartesian(&self.xcm_to_xbound, &self.ex, &self.ey, &self.ez);
        Vec3::new(self.xcm[0] + g[0], self.xcm[1] + g[1], self.xcm[2] + g[2])
    }
}

/// The state of `fix multisphere`: the bodies, and per atom (indexed like the
/// owning [`GranularSystem`](crate::granular_system::GranularSystem)'s
/// particles) the body it belongs to and its body-frame displacement.
#[derive(Debug, Clone, PartialEq)]
pub struct Multisphere {
    bodies: Vec<ClumpBody>,
    /// Per atom: index into `bodies`, if a member.
    body_of: Vec<Option<usize>>,
    /// Per atom: body-frame displacement (`displace_`).
    displace: Vec<[f64; 3]>,
    /// Per atom: volume weight (`volumeweight_ms`).
    volumeweight: Vec<f64>,
    /// Per atom: marked for deletion by `check_lost_atoms` (`delflag`).
    delflag: Vec<bool>,
    /// `next_reneighbor` of the fix (a forced list rebuild step).
    next_reneighbor: Option<u64>,
    dt: f64,
    /// `false` for `fix multisphere/nointegration`.
    integrate: bool,
    /// `T0` of the attached `fix heat/gran`, if any (the heat-transfer branch
    /// of `calc_force`; gh:#1415).
    heat_t0: Option<f64>,
    /// `has_stream`: an `insert/stream` fix exists (`n_fixes_style`).
    has_stream: bool,
    /// `update->ntimestep` of the step being integrated.
    timestep: i64,
    /// `fix multisphere/break` (gh:#1415): `pre_neighbor` forces the rebuild
    /// 100 steps after a lost atom, not 5 ([`Multisphere::with_break`]).
    break_style: bool,
    /// `mapTagMax_`: the largest body id ever given (ids are never reused).
    map_tag_max: usize,
}

impl Multisphere {
    /// An empty `fix multisphere` for time step `dt` `[s]`.
    #[must_use]
    pub fn new(dt: f64) -> Self {
        Self {
            bodies: Vec::new(),
            body_of: Vec::new(),
            displace: Vec::new(),
            volumeweight: Vec::new(),
            delflag: Vec::new(),
            next_reneighbor: None,
            dt,
            integrate: true,
            heat_t0: None,
            has_stream: false,
            timestep: 0,
            break_style: false,
            map_tag_max: 0,
        }
    }

    /// `fix multisphere/break` (gh:#1415, fourth pass): the integration is
    /// `FixMultisphere`'s, [`Multisphere::break_bodies`] runs after
    /// `final_integrate`, and `pre_neighbor` forces the rebuild 100 steps
    /// after a lost atom (`FixMultisphereBreak::pre_neighbor`) instead of 5.
    #[must_use]
    pub fn with_break(mut self) -> Self {
        self.break_style = true;
        self
    }

    /// `FixMultisphereBreak::final_integrate` after
    /// `FixMultisphere::final_integrate`, once `step > trigger_timeStep`
    /// (gh:#1415, fourth pass; LIGGGHTS-PUBLIC `3d5c00f2`). `trigger[i]` is
    /// atom `i`'s trigger value, `local_order` the atoms in local order.
    ///
    /// * Every member atom with `trigger >= threshold` whose body has
    ///   `nrigid >= 2` tag-resets the body (`Multisphere::tagReset`).
    /// * Every atom of a tag-reset body is freed: `body = -1`, the body's
    ///   `nrigid` becomes 0, and its mass becomes
    ///   `r*r*r*4.18879020479*density` (the body density; upstream's
    ///   truncated `4 pi / 3`), not the volume-weighted share.
    /// * If any atom was freed, `add_body_finalize`: each tag-reset body gets
    ///   a **new** id (`id_extend_body_extend` treats `id = -1` as a new body)
    ///   and **stays** with `nrigid = 0` and no atoms. Upstream keeps it
    ///   (`check_lost_atoms` finds 0 of 0), integrates it and counts it in
    ///   the computes; so does the port.
    ///
    /// Returns the number of atoms freed.
    pub(crate) fn break_bodies(
        &mut self,
        particles: &mut [Particle],
        trigger: &[f64],
        local_order: &[usize],
        threshold: f64,
    ) -> usize {
        let nbody = self.bodies.len();
        let mut tag_reset = vec![false; nbody];
        for &i in local_order {
            let Some(ib) = self.body_of(i) else { continue };
            if trigger[i] < threshold {
                continue;
            }
            if self.bodies[ib].nrigid < 2 {
                continue;
            }
            tag_reset[ib] = true;
        }
        // existflag = 0 for the atoms of a tag-reset body with nrigid >= 2,
        // then those atoms are freed
        let mut existflag0 = vec![false; particles.len()];
        for &i in local_order {
            let Some(ib) = self.body_of(i) else { continue };
            if self.bodies[ib].nrigid < 2 {
                continue;
            }
            if tag_reset[ib] {
                existflag0[i] = true;
            }
        }
        let four_over_three_pi = 4.18879020479;
        let mut removed = 0;
        for &i in local_order {
            let Some(ib) = self.body_of(i) else { continue };
            if tag_reset[ib] && existflag0[i] {
                self.body_of[i] = None;
                if self.bodies[ib].nrigid > 1 {
                    self.bodies[ib].nrigid = 0;
                }
                let r = particles[i].radius;
                particles[i].mass = r * r * r * four_over_three_pi * self.bodies[ib].density;
                removed += 1;
            }
        }
        if removed > 0 {
            // `id_extend_body_extend`: the tag-reset bodies take new ids
            self.map_tag_max = self.map_tag_max.max(self.bodies.iter().map(|b| b.id).max().unwrap_or(0));
            for (ib, b) in self.bodies.iter_mut().enumerate() {
                if tag_reset[ib] {
                    self.map_tag_max += 1;
                    b.id = self.map_tag_max;
                }
            }
            self.add_body_finalize(particles);
        }
        removed
    }

    /// Whether a `fix insert/stream` exists (`has_stream` of
    /// `FixMultisphere::initial_integrate` / `final_integrate`), and the
    /// current step (`update->ntimestep`); set by the system every step.
    pub(crate) fn set_stream_step(&mut self, has_stream: bool, step: u64) {
        self.has_stream = has_stream;
        self.timestep = step as i64;
    }

    /// Whether body `b` still rides its `insert/stream` rail
    /// (`has_stream && timestep < start_step`).
    fn on_rail(has_stream: bool, timestep: i64, b: &ClumpBody) -> bool {
        has_stream && b.start_step.is_some_and(|s| timestep < s)
    }

    /// `Multisphere::calc_n_steps` (gh:#1415, fourth pass), called by
    /// `FixInsertStream::finalize_insertion` for each new atom `i`. `None` for
    /// an atom in no body (the stream then uses its sphere formula). For a
    /// body with a `start_step` it returns `start_step - step`; otherwise it
    /// sets `start_step = step + n` and `v_integrate = v_normal`, with
    /// `n = int(((p_ref - xcm) . normal) / (|v_normal| dt))` (no
    /// `FIX_INSERT_STREAM_TINY`, unlike the sphere formula), and returns `n`.
    pub(crate) fn calc_n_steps(
        &mut self,
        i: usize,
        p_ref: &V3,
        normal: &V3,
        v_normal: &V3,
        step: u64,
    ) -> Option<i64> {
        let ib = self.body_of(i)?;
        let dt = self.dt;
        let b = &mut self.bodies[ib];
        let timestep = step as i64;
        if let Some(s) = b.start_step {
            return Some(s - timestep);
        }
        let pos_rel = [p_ref[0] - b.xcm[0], p_ref[1] - b.xcm[1], p_ref[2] - b.xcm[2]];
        let dist_normal = dot3(&pos_rel, normal);
        let vmag = (v_normal[0] * v_normal[0] + v_normal[1] * v_normal[1] + v_normal[2] * v_normal[2]).sqrt();
        // static_cast<int>: truncation toward zero
        let n_steps = (dist_normal / (vmag * dt)) as i32 as i64;
        b.start_step = Some(n_steps + timestep);
        b.v_integrate = *v_normal;
        Some(n_steps)
    }

    /// `Multisphere::release` (gh:#1415, fourth pass), called by
    /// `FixInsertStream::end_of_step` on a member atom `i` at its release
    /// step: if the body has a `start_step` and `step >= start_step`, it takes
    /// `vcm = v` and `omega = omega` and its `start_step` is cleared. Its
    /// angular momentum is **not** changed, as upstream; the atom itself is
    /// left as it is.
    pub(crate) fn release(&mut self, i: usize, v: Vec3, omega: Vec3, step: u64) {
        let Some(ib) = self.body_of(i) else { return };
        let b = &mut self.bodies[ib];
        if b.start_step.is_some_and(|s| step as i64 >= s) {
            b.vcm = v3(v);
            b.omega = v3(omega);
            b.start_step = None;
        }
    }

    /// A `fix heat/gran` with initial temperature `t0` `[K]` exists
    /// (`FixMultisphere::init` finds it; gh:#1415, fourth pass). A body
    /// inserted from now on starts at `temp = temp_old = t0`
    /// (`Multisphere::add_body`), and [`Multisphere::heat_transfer`] runs.
    ///
    /// Upstream: `FixMultisphere::calc_force(setupflag)`, after the force and
    /// torque sum (LIGGGHTS-PUBLIC `3d5c00f2`). The port models a deck that
    /// defines `heat/gran` **before** `fix multisphere`, so the transport
    /// equation has advanced the atom temperatures when the bodies aggregate
    /// them; the other order lags by a step and is not modelled. Heat sources
    /// are zero here (upstream refuses them without `allow_heatsource`).
    #[must_use]
    pub fn with_heat_transfer(mut self, t0: f64) -> Self {
        self.heat_t0 = Some(t0);
        self
    }

    /// The heat-transfer branch of `FixMultisphere::calc_force(setupflag)`
    /// (gh:#1415, fourth pass; no-op without a heat fix). Over the bodies,
    /// `temp_old = temp`; at a `run` set-up (`setup`) `temp = 0`. Then over
    /// the member atoms in local order, `temp += T_i - temp_old` (a step) or
    /// `temp += volumeweight_i * T_i` (set-up); then every member atom takes
    /// `T_i = temp`.
    pub(crate) fn heat_transfer(&mut self, particles: &mut [Particle], local_order: &[usize], setup: bool) {
        if self.heat_t0.is_none() {
            return;
        }
        for b in &mut self.bodies {
            b.temp_old = b.temp;
        }
        if setup {
            for b in &mut self.bodies {
                b.temp = 0.0;
            }
        }
        for &i in local_order {
            let Some(ib) = self.body_of(i) else { continue };
            let t_i = particles[i].temperature;
            let w = self.volume_weight(i);
            let b = &mut self.bodies[ib];
            if setup {
                b.temp += w * t_i;
            } else {
                b.temp += t_i - b.temp_old;
            }
        }
        for &i in local_order {
            let Some(ib) = self.body_of(i) else { continue };
            particles[i].temperature = self.bodies[ib].temp;
        }
    }

    /// `fix multisphere/nointegration` (gh:#1415): `initial_integrate`
    /// returns at once (no kick, no drift, no `set_xv`) and `final_integrate`
    /// returns after `calc_force` (no kick, no `set_v`), as upstream's
    /// `strstr(style,"nointegration")` branches do. The bodies stay as
    /// inserted; their member atoms move only by the atom integrator
    /// (`fix nve/sphere`), carrying the clump mass, still without gravity
    /// (`FixGravity` finds the fix by its `multisphere` prefix) and without
    /// intra-clump pairs.
    #[must_use]
    pub fn without_integration(mut self) -> Self {
        self.integrate = false;
        self
    }

    /// The bodies, in local order.
    #[must_use]
    pub fn bodies(&self) -> &[ClumpBody] {
        &self.bodies
    }

    /// The body index of atom `i`, if it belongs to one (`belongs_to`).
    #[must_use]
    pub fn body_of(&self, i: usize) -> Option<usize> {
        self.body_of.get(i).copied().flatten()
    }

    /// The volume weight of atom `i` (`volumeweight_ms`; 1 for a free atom).
    #[must_use]
    pub fn volume_weight(&self, i: usize) -> f64 {
        self.volumeweight.get(i).copied().unwrap_or(1.0)
    }

    /// `ParticleToInsertMultisphere::insert` (the body part) +
    /// `Multisphere::add_body` + the id assignment of `id_extend_body_extend`:
    /// the clump's atoms were just appended to the atom list, the first at
    /// index `first_atom` (atoms before it that are not members are free).
    pub(crate) fn add_body(&mut self, c: &ClumpToInsert, first_atom: usize) {
        while self.body_of.len() < first_atom {
            self.body_of.push(None);
            self.displace.push([0.0; 3]);
            self.volumeweight.push(1.0);
        }
        let n = self.bodies.len();
        let quat = exyz_to_q(&c.ex, &c.ey, &c.ez);
        let angmom = angmom_from_omega(&c.omega, &c.ex, &c.ey, &c.ez, &c.inertia);
        let id = self.bodies.iter().map(|b| b.id).max().unwrap_or(0) + 1;
        self.bodies.push(ClumpBody {
            id,
            xcm: c.xcm,
            vcm: c.v,
            fcm: [0.0; 3],
            torque: [0.0; 3],
            angmom,
            omega: c.omega,
            quat,
            inertia: c.inertia,
            ex: c.ex,
            ey: c.ey,
            ez: c.ez,
            mass: c.mass,
            density: c.density,
            clump_type: c.clump_type,
            r_bound: c.r_bound,
            xcm_to_xbound: c.xcm_to_xbound,
            nrigid: c.radius.len(),
            fflag: c.fflag,
            tflag: c.tflag,
            dragforce: [0.0; 3],
            hdtorque: [0.0; 3],
            temp: self.heat_t0.unwrap_or(0.0),
            temp_old: self.heat_t0.unwrap_or(0.0),
            start_step: None,
            v_integrate: [0.0; 3],
        });
        for j in 0..c.radius.len() {
            self.body_of.push(Some(n));
            self.displace.push(c.displace[j]);
            self.volumeweight.push(c.volumeweight[j]);
        }
    }

    /// `add_body_finalize`: `reset_forces(true)` then `set_xv(LOOP_LOCAL)`.
    pub(crate) fn add_body_finalize(&mut self, particles: &mut [Particle]) {
        self.reset_forces(true);
        self.set_xv(particles);
    }

    /// `FixMultisphere::modify_param`: `set_fflag` / `set_tflag` on every
    /// body.
    pub(crate) fn modify_param(&mut self, m: MultisphereModify) {
        for b in &mut self.bodies {
            match m {
                MultisphereModify::Fflag(f) => b.fflag = f,
                MultisphereModify::Tflag(t) => b.tflag = t,
            }
        }
    }

    /// `Multisphere::reset_forces(extflag)`.
    pub(crate) fn reset_forces(&mut self, extflag: bool) {
        for b in &mut self.bodies {
            b.fcm = [0.0; 3];
            b.torque = [0.0; 3];
            if extflag {
                b.dragforce = [0.0; 3];
                b.hdtorque = [0.0; 3];
            }
        }
    }

    /// `FixMultisphere::initial_integrate` (the body loop; ~~`fflag`/`tflag` on,~~
    /// no added mass, ~~no `insert/stream`~~), then `set_xv`. **CORRECTED
    /// 2026-11-14 (gh:#1415):** the template `fflag`/`tflag` apply, and a body
    /// on its `insert/stream` rail (`has_stream && step < start_step`) takes
    /// `vcm = v_integrate` and drifts, with no kick and no rotation.
    pub(crate) fn initial_integrate(&mut self, particles: &mut [Particle]) {
        if !self.integrate {
            return;
        }
        let dtv = self.dt;
        let dtf = 0.5 * self.dt * 1.0;
        let dtq = 0.5 * self.dt;
        let (has_stream, timestep) = (self.has_stream, self.timestep);
        for b in &mut self.bodies {
            if Self::on_rail(has_stream, timestep, b) {
                // `insert/stream`: the body drifts with v_integrate, no kick
                b.vcm = b.v_integrate;
                for k in 0..3 {
                    b.xcm[k] += dtv * b.vcm[k];
                }
                continue;
            }
            let add_mass_term = 1.0 + 0.0 * 0.0 / b.density;
            let dtfm = dtf / (b.mass * add_mass_term);
            for k in 0..3 {
                if b.fflag[k] {
                    b.vcm[k] += dtfm * b.fcm[k];
                }
            }
            for k in 0..3 {
                b.xcm[k] += dtv * b.vcm[k];
            }
            let dtt = dtf / add_mass_term;
            for k in 0..3 {
                if b.tflag[k] {
                    b.angmom[k] += dtt * b.torque[k];
                }
            }
            b.omega = angmom_to_omega(&b.angmom, &b.ex, &b.ey, &b.ez, &b.inertia);
            let angmom = b.angmom;
            let inertia = b.inertia;
            richardson(&mut b.quat, &angmom, &mut b.omega, &inertia, dtq);
            let (ex, ey, ez) = q_to_exyz(&b.quat);
            b.ex = ex;
            b.ey = ey;
            b.ez = ez;
        }
        self.set_xv(particles);
    }

    /// `FixMultisphere::set_xv` (one process, no periodic images, no virial).
    pub(crate) fn set_xv(&self, particles: &mut [Particle]) {
        for (i, p) in particles.iter_mut().enumerate() {
            let Some(ib) = self.body_of(i) else { continue };
            let b = &self.bodies[ib];
            let x = matvec(&b.ex, &b.ey, &b.ez, &self.displace[i]);
            let w = &b.omega;
            let v = [
                w[1] * x[2] - w[2] * x[1] + b.vcm[0],
                w[2] * x[0] - w[0] * x[2] + b.vcm[1],
                w[0] * x[1] - w[1] * x[0] + b.vcm[2],
            ];
            p.velocity = vec3(v);
            p.position = Vec3::new(x[0] + b.xcm[0], x[1] + b.xcm[1], x[2] + b.xcm[2]);
            p.angular_velocity = vec3(b.omega);
        }
    }

    /// `FixMultisphere::set_v`.
    pub(crate) fn set_v(&self, particles: &mut [Particle]) {
        for (i, p) in particles.iter_mut().enumerate() {
            let Some(ib) = self.body_of(i) else { continue };
            let b = &self.bodies[ib];
            let d = matvec(&b.ex, &b.ey, &b.ez, &self.displace[i]);
            let w = &b.omega;
            p.velocity = Vec3::new(
                w[1] * d[2] - w[2] * d[1] + b.vcm[0],
                w[2] * d[0] - w[0] * d[2] + b.vcm[1],
                w[0] * d[1] - w[1] * d[0] + b.vcm[2],
            );
            p.angular_velocity = vec3(b.omega);
        }
    }

    /// `FixMultisphere::calc_force`: sum each member atom's force and torque
    /// into its body, visiting the atoms in `local_order` (upstream's local
    /// atom order); then `m g` and the (zero) drag.
    pub(crate) fn calc_force(
        &mut self,
        particles: &[Particle],
        forces: &[Vec3],
        torques: &[Vec3],
        local_order: &[usize],
        gravity: Vec3,
    ) {
        for &i in local_order {
            let Some(ib) = self.body_of(i) else { continue };
            let b = &mut self.bodies[ib];
            let f = v3(forces[i]);
            let t = v3(torques[i]);
            b.fcm[0] += f[0];
            b.fcm[1] += f[1];
            b.fcm[2] += f[2];
            let x = particles[i].position;
            let dx = x.x - b.xcm[0];
            let dy = x.y - b.xcm[1];
            let dz = x.z - b.xcm[2];
            b.torque[0] += dy * f[2] - dz * f[1] + t[0];
            b.torque[1] += dz * f[0] - dx * f[2] + t[1];
            b.torque[2] += dx * f[1] - dy * f[0] + t[2];
        }
        let grav = v3(gravity);
        for b in &mut self.bodies {
            b.fcm[0] += b.mass * grav[0];
            b.fcm[1] += b.mass * grav[1];
            b.fcm[2] += b.mass * grav[2];
        }
        for b in &mut self.bodies {
            for k in 0..3 {
                b.fcm[k] += b.dragforce[k];
            }
            for k in 0..3 {
                b.torque[k] += b.hdtorque[k];
            }
        }
    }

    /// `FixMultisphere::final_integrate` after `calc_force`: the second
    /// half-kick, `omega` from `angmom`, then `set_v`.
    pub(crate) fn final_integrate(&mut self, particles: &mut [Particle]) {
        if !self.integrate {
            return;
        }
        let dtf = 0.5 * self.dt * 1.0;
        let (has_stream, timestep) = (self.has_stream, self.timestep);
        for b in &mut self.bodies {
            if Self::on_rail(has_stream, timestep, b) {
                continue;
            }
            let add_mass_term = 1.0 + 0.0 * 0.0 / b.density;
            let dtfm = dtf / (b.mass * add_mass_term);
            for k in 0..3 {
                if b.fflag[k] {
                    b.vcm[k] += dtfm * b.fcm[k];
                }
            }
            let dtt = dtf / add_mass_term;
            for k in 0..3 {
                if b.tflag[k] {
                    b.angmom[k] += dtt * b.torque[k];
                }
            }
            b.omega = angmom_to_omega(&b.angmom, &b.ex, &b.ey, &b.ez, &b.inertia);
        }
        self.set_v(particles);
    }

    /// `Multisphere::extract_ke`, the scalar of `compute ke/multisphere`: the
    /// translational kinetic energy `1/2 sum M vcm^2` of the bodies `[J]`,
    /// summed in local body order (`mvv2e = 1` in SI units).
    #[must_use]
    pub fn extract_ke(&self) -> f64 {
        let mvv2e = 1.0;
        let mut ke = 0.0;
        for b in &self.bodies {
            ke += b.mass * (b.vcm[0] * b.vcm[0] + b.vcm[1] * b.vcm[1] + b.vcm[2] * b.vcm[2]);
        }
        0.5 * mvv2e * ke
    }

    /// `Multisphere::extract_rke`, the scalar of `compute erotate/multisphere`:
    /// `1/2 sum I wbody^2` `[J]`, with the body-frame angular velocity
    /// `wbody = R(q)^T L / I` (0 on a zero principal moment).
    #[must_use]
    pub fn extract_rke(&self) -> f64 {
        let mut rke = 0.0;
        for b in &self.bodies {
            let rot = quat_to_mat(&b.quat);
            let m = &b.angmom;
            // MathExtra::transpose_matvec(rot, angmom, wbody)
            let mut wbody = [
                rot[0][0] * m[0] + rot[1][0] * m[1] + rot[2][0] * m[2],
                rot[0][1] * m[0] + rot[1][1] * m[1] + rot[2][1] * m[2],
                rot[0][2] * m[0] + rot[1][2] * m[1] + rot[2][2] * m[2],
            ];
            for k in 0..3 {
                if b.inertia[k] == 0.0 {
                    wbody[k] = 0.0;
                } else {
                    wbody[k] /= b.inertia[k];
                }
            }
            let i = &b.inertia;
            rke += i[0] * wbody[0] * wbody[0] + i[1] * wbody[1] * wbody[1] + i[2] * wbody[2] * wbody[2];
        }
        0.5 * rke
    }

    /// Grow the per-atom arrays to `n` atoms (free atoms appended after the
    /// last clump are not members).
    fn pad_atoms(&mut self, n: usize) {
        while self.body_of.len() < n {
            self.body_of.push(None);
            self.displace.push([0.0; 3]);
            self.volumeweight.push(1.0);
        }
        while self.delflag.len() < n {
            self.delflag.push(false);
        }
    }

    /// `AtomVec::copy(nlocal-1, i, 1)` for this fix's per-atom data
    /// (`FixMultisphere::copy_arrays`: `body_`, `displace_`; the
    /// `property/atom`s `volumeweight_ms` and `delflag`): atom `i` of `n` is
    /// removed and the last atom takes its slot, as the particle arrays'
    /// `swap_remove` does (gh:#1415).
    pub(crate) fn swap_remove_atom(&mut self, i: usize, n: usize) {
        self.pad_atoms(n);
        self.body_of.truncate(n);
        self.displace.truncate(n);
        self.volumeweight.truncate(n);
        self.delflag.truncate(n);
        self.body_of.swap_remove(i);
        self.displace.swap_remove(i);
        self.volumeweight.swap_remove(i);
        self.delflag.swap_remove(i);
    }

    /// Whether atom `i` is marked for deletion (`delflag`).
    pub(crate) fn delflag(&self, i: usize) -> bool {
        self.delflag.get(i).copied().unwrap_or(false)
    }

    /// The step this fix asks the neighbour list to be rebuilt on
    /// (`next_reneighbor`), if any.
    #[must_use]
    pub fn next_reneighbor(&self) -> Option<u64> {
        self.next_reneighbor
    }

    /// The start of `FixMultisphere::pre_exchange`: `next_reneighbor = -1`.
    /// The caller then deletes the atoms with [`Multisphere::delflag`] set.
    pub(crate) fn pre_exchange_reset(&mut self) {
        self.next_reneighbor = None;
    }

    /// `FixMultisphere::pre_neighbor` on one process: `delflag` is zeroed,
    /// then `Multisphere::check_lost_atoms`. A body that has lost member atoms
    /// (`nrigid_current != nrigid`) is deleted (`remove_body`, the last body
    /// taking its slot), and its remaining atoms are freed: `body = -1`,
    /// `delflag = 1`, `rmass *= volumeweight`. If any atom is marked, a
    /// rebuild is requested at `step + 5` (the `existflag` merge cannot mark
    /// more atoms on one process). Returns whether any atom was marked.
    ///
    /// # Panics
    ///
    /// If a body holds more atoms than it was built with ("Internal error in
    /// multisphere method", upstream's `error->one`).
    pub(crate) fn pre_neighbor(&mut self, particles: &mut [Particle], step: u64) -> bool {
        let n = particles.len();
        self.pad_atoms(n);
        for d in self.delflag.iter_mut() {
            *d = false;
        }
        let nbody = self.bodies.len();
        let mut nrigid_current = vec![0usize; nbody];
        for i in 0..n {
            if let Some(ib) = self.body_of[i] {
                nrigid_current[ib] += 1;
            }
        }
        let mut del = vec![false; nbody];
        for ib in 0..nbody {
            assert!(
                nrigid_current[ib] <= self.bodies[ib].nrigid,
                "Internal error in multisphere method"
            );
            if nrigid_current[ib] != self.bodies[ib].nrigid {
                del[ib] = true;
            }
        }
        let mut deleted = false;
        for i in 0..n {
            let Some(ib) = self.body_of[i] else { continue };
            if del[ib] {
                self.delflag[i] = true;
                self.body_of[i] = None;
                deleted = true;
                particles[i].mass *= self.volumeweight[i];
            }
        }
        let mut ib = 0;
        while ib < self.bodies.len() {
            if del[ib] {
                let last = self.bodies.len() - 1;
                del[ib] = del[last];
                del.pop();
                self.bodies.swap_remove(ib);
                if ib != last {
                    for b in self.body_of.iter_mut() {
                        if *b == Some(last) {
                            *b = Some(ib);
                        }
                    }
                }
            } else {
                ib += 1;
            }
        }
        if deleted {
            // `FixMultisphereBreak::pre_neighbor` waits 100 steps
            self.next_reneighbor = Some(step + if self.break_style { 100 } else { 5 });
        }
        deleted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(v: f64) -> Length {
        Length::new::<meter>(v)
    }

    fn rho(v: f64) -> MassDensity {
        MassDensity::new::<kilogram_per_cubic_meter>(v)
    }

    #[test]
    fn jacobi_diagonalises_and_quaternions_round_trip() {
        let mut a = [[2.0, 1.0, 0.5], [1.0, 3.0, 0.25], [0.5, 0.25, 4.0]];
        let orig = a;
        let (ev, vecs) = jacobi(&mut a).unwrap();
        for k in 0..3 {
            let v = [vecs[0][k], vecs[1][k], vecs[2][k]];
            for r in 0..3 {
                let av = orig[r][0] * v[0] + orig[r][1] * v[1] + orig[r][2] * v[2];
                assert!((av - ev[k] * v[r]).abs() < 1e-12);
            }
        }
        // already diagonal: returns at once with identity vectors
        let mut d = [[1.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 3.0]];
        let (e, v) = jacobi(&mut d).unwrap();
        assert_eq!(e, [1.0, 2.0, 3.0]);
        assert_eq!(v[0][0], 1.0);
        // quaternion <-> axes
        let mut q = [0.5, 0.5, 0.5, 0.5];
        qnormalize(&mut q);
        let (ex, ey, ez) = q_to_exyz(&q);
        let q2 = exyz_to_q(&ex, &ey, &ez);
        for k in 0..4 {
            assert!((q[k] - q2[k]).abs() < 1e-15);
        }
        let r = vec_quat_rotate(&[1.0, 0.0, 0.0], &q);
        assert!((r[0] - ex[0]).abs() < 1e-15 && (r[1] - ex[1]).abs() < 1e-15);
        // angular momentum <-> omega
        let w = [1.0, -2.0, 3.0];
        let i = [1.0, 2.0, 3.0];
        let l = angmom_from_omega(&w, &ex, &ey, &ez, &i);
        let w2 = angmom_to_omega(&l, &ex, &ey, &ez, &i);
        for k in 0..3 {
            assert!((w[k] - w2[k]).abs() < 1e-13);
        }
        // a free rigid rotor keeps |q| = 1 and its angular momentum
        let mut ww = w2;
        richardson(&mut q, &l, &mut ww, &i, 1e-3);
        let n: f64 = q.iter().map(|c| c * c).sum();
        assert!((n - 1.0).abs() < 1e-15);
        let mut e3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let mo = mq_to_omega(&[1.0, 2.0, 3.0], &[1.0, 0.0, 0.0, 0.0], &[1.0, 0.0, 2.0]);
        assert_eq!(mo, [1.0, 0.0, 1.5]);
        rotate(&mut e3, 0, 1, 1, 0, 0.0, 0.0);
        assert_eq!(e3[0][1], 0.0);
        assert!(cartesian_to_local_orthogonal(&[1.0, 0.0, 0.0], &[1.0, 0.0, 0.0], &[1.0, 0.0, 0.0], &[0.0, 0.0, 1.0]).is_err());
    }

    #[test]
    fn two_sphere_template_mass_properties() {
        // two touching unit-density spheres of radius 1 mm at +-1 mm on x
        let t = ParticleTemplateMultisphere::new(
            15_485_863,
            1,
            rho(1000.0),
            200_000,
            &[(Vec3::new(-0.001, 0.0, 0.0), m(0.001)), (Vec3::new(0.001, 0.0, 0.0), m(0.001))],
            1,
        )
        .unwrap();
        let v_exact = 2.0 * 4.0 / 3.0 * std::f64::consts::PI * 1e-9;
        assert!((t.volexpect() / v_exact - 1.0).abs() < 0.02, "volume {}", t.volexpect());
        assert!((t.massexpect() - 1000.0 * t.volexpect()).abs() < 1e-18);
        // bounding sphere encloses both: radius 2 mm about the centre
        assert!((t.r_bound() - 0.002).abs() < 1e-12);
        assert_eq!(t.max_rad(), 0.001);
        // axial moment 2*(2/5) m r^2 for the pair; transverse adds m d^2
        let mi = t.principal_moments();
        let ms = t.massexpect() / 2.0;
        let axial = 2.0 * 0.4 * ms * 1e-6;
        let trans = axial + 2.0 * ms * 1e-6;
        let mut sorted = mi;
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert!((sorted[0] / axial - 1.0).abs() < 0.05, "{mi:?}");
        assert!((sorted[2] / trans - 1.0).abs() < 0.05, "{mi:?}");
        let ws: f64 = t.volume_weights().iter().sum();
        assert!((ws - 1.0).abs() < 1e-12);
        assert!(!t.no_overlap() || t.no_overlap());
        assert_eq!(t.density(), 1000.0);
        // rejections
        let one = [(Vec3::zero(), m(0.001))];
        assert!(ParticleTemplateMultisphere::new(15_485_863, 1, rho(1.0), 2000, &one, 1).is_err());
        let two = [(Vec3::zero(), m(0.001)), (Vec3::new(0.001, 0.0, 0.0), m(0.001))];
        assert!(ParticleTemplateMultisphere::new(15_485_863, 1, rho(1.0), 10, &two, 1).is_err());
        assert!(ParticleTemplateMultisphere::new(15_485_863, 1, rho(1.0), 2000, &two, 0).is_err());
        assert!(ParticleTemplateMultisphere::new(15_485_863, 0, rho(1.0), 2000, &two, 1).is_err());
        assert!(ParticleTemplateMultisphere::new(15_485_863, 1, rho(-1.0), 2000, &two, 1).is_err());
        let neg = [(Vec3::zero(), m(-0.001)), (Vec3::new(0.001, 0.0, 0.0), m(0.001))];
        assert!(ParticleTemplateMultisphere::new(15_485_863, 1, rho(1.0), 2000, &neg, 1).is_err());
    }

    #[test]
    fn free_clump_conserves_momentum_and_keeps_its_shape() {
        let t = ParticleTemplateMultisphere::new(
            15_485_863,
            1,
            rho(2500.0),
            20_000,
            &[
                (Vec3::new(0.0, 0.0, 0.0), m(0.004)),
                (Vec3::new(0.005, 0.0, 0.001), m(0.0035)),
                (Vec3::new(0.002, 0.004, -0.001), m(0.003)),
            ],
            1,
        )
        .unwrap();
        let mut c = t.to_insert();
        let placed = c.check_near_set_x_v_omega(
            Vec3::new(0.0, 0.0, 0.05),
            Vec3::new(0.1, 0.0, 0.0),
            Vec3::new(3.0, -2.0, 5.0),
            [0.5, 0.5, 0.5, 0.5],
            |_, _| false,
        );
        assert_eq!(placed.unwrap().len(), 3);
        assert!(c.check_near_set_x_v_omega(Vec3::zero(), Vec3::zero(), Vec3::zero(), [1.0, 0.0, 0.0, 0.0], |_, _| true).is_none());
        let mut c2 = t.to_insert();
        c2.set_x_v_omega(Vec3::new(0.0, 0.0, 0.05), Vec3::zero(), Vec3::zero(), [1.0, 0.0, 0.0, 0.0]);
        assert_eq!(c2.atoms().len(), 3);
        let mut ms = Multisphere::new(1e-5);
        let mut atoms = c.atoms();
        assert_eq!(c.atom_type(), 1);
        ms.add_body(&c, 0);
        ms.add_body_finalize(&mut atoms);
        let d01 = atoms[0].position.sub(atoms[1].position).norm();
        let l0 = ms.bodies()[0].angmom;
        for _ in 0..2000 {
            ms.initial_integrate(&mut atoms);
            ms.reset_forces(false);
            let z = vec![Vec3::zero(); 3];
            ms.calc_force(&atoms, &z, &z, &[0, 1, 2], Vec3::zero());
            ms.final_integrate(&mut atoms);
        }
        let b = &ms.bodies()[0];
        assert_eq!(b.angmom, l0);
        assert!((atoms[0].position.sub(atoms[1].position).norm() - d01).abs() < 1e-12);
        assert!((b.xcm[0] - 0.1 * 0.02).abs() < 1e-12);
        assert_eq!(ms.body_of(0), Some(0));
        assert_eq!(ms.body_of(7), None);
        assert!(ms.volume_weight(0) > 0.0 && ms.volume_weight(9) == 1.0);
        let xb = b.x_bound();
        assert!(xb.sub(Vec3::new(b.xcm[0], b.xcm[1], b.xcm[2])).norm() < b.r_bound);
    }
}
