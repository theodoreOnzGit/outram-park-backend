// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
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
//
// ---------------------------------------------------------------------------
// Ported from:
//   Project:  PRISMS-Plasticity (prisms-center/plasticity)
//   Source:   src/materialModels/crystalPlasticity/rotationOperations.cc
//               (`odfpoint`, Rodrigues vector -> orientation matrix)
//             src/materialModels/crystalPlasticity/MaterialModels/
//               RateDependentModel/calculatePlasticity.cc, lines 216-220
//               (`R S R^T` rotation of the Schmid tensor into sample axes)
//   Version:  commit ffdf4eb67b55b84f8b20cbb21407cf310ec3a7e4 (2026-08-27)
//   Copyright (c) 2016 The Regents of the University of Michigan, PRISMS Center
//   Licence:  LGPL-2.1-or-later upstream; relicensed to GPL-3.0-only here under
//             LGPL-2.1 section 3. See crates/farrer-park/NOTICE and
//             crates/farrer-park/docs/upstream-provenance.md.
// ---------------------------------------------------------------------------

//! Crystal orientation: the rotation that carries the lattice frame into the
//! sample frame, and the ways a user actually writes one down.
//!
//! # THE CONVENTION, stated once
//!
//! An [`Orientation`] stores the **crystal-to-sample** rotation matrix `R`:
//! a vector `v_c` written in lattice axes becomes `v_s = R v_c` in sample
//! axes, and a second-order tensor becomes `A_s = R A_c R^T`.
//!
//! This is PRISMS-Plasticity's convention — `calculatePlasticity.cc` builds
//! the Schmid tensor in lattice axes and then evaluates `rotmat * S *
//! rotmat^T`, so its `rotmat` is crystal-to-sample. Every constructor here
//! documents what it takes and converts to this one storage form, so there is
//! exactly one convention inside the crate.
//!
//! The **inverse** convention (sample-to-crystal) is what texture people
//! normally mean by "the orientation matrix `g`", and it is what Bunge Euler
//! angles define. [`Orientation::from_bunge_euler_radians`] takes Bunge angles
//! and transposes for you; it does not silently store `g`.
//!
//! # What is not here: lattice reorientation
//!
//! The orientation in this crate is **fixed for the life of the analysis**.
//! Crystallographic reorientation — the lattice spin that rotates a grain
//! towards a stable texture component under large plastic strain — is a
//! finite-deformation effect driven by the skew part of the plastic velocity
//! gradient, which a small-strain formulation does not have. See the
//! [`crate::crystal`] module documentation and bead `op-q75c`.
//!
//! # Units
//!
//! A rotation matrix is dimensionless. Rodrigues components are dimensionless
//! (`r = tan(theta/2) a` for a rotation of `theta` about the unit axis `a`).
//! Euler angles are in radians unless the constructor's name says degrees.

use crate::error::{FemError, Result};
use crate::tensor::Voigt6;

/// A crystal orientation, stored as the **crystal-to-sample** rotation matrix.
///
/// Constructed only through the checked constructors below, so an
/// `Orientation` is always a proper rotation (orthonormal, determinant `+1`)
/// to within the tolerance of [`Orientation::from_matrix`].
///
/// # Units
///
/// Dimensionless.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Orientation {
    /// Row-major `R[i][j]`, mapping lattice component `j` to sample
    /// component `i`.
    r: [[f64; 3]; 3],
}

impl Default for Orientation {
    /// The **cube orientation**: lattice axes aligned with sample axes.
    fn default() -> Self {
        Orientation::identity()
    }
}

impl Orientation {
    /// The cube orientation `R = I` — lattice `[100]`, `[010]`, `[001]` along
    /// the sample `x`, `y`, `z` axes.
    #[must_use]
    pub fn identity() -> Self {
        Self {
            r: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        }
    }

    /// Build from an explicit **crystal-to-sample** rotation matrix, checking
    /// that it really is one.
    ///
    /// # Arguments
    ///
    /// - `r` — row-major `R[i][j]`, dimensionless. `R^T R` must equal the
    ///   identity to `1e-10` in every entry and `det R` must be `+1` to
    ///   `1e-10`. A reflection (`det R = -1`) is rejected rather than
    ///   silently accepted: it would turn a right-handed lattice into a
    ///   left-handed one and flip the sign of every resolved shear stress.
    ///
    /// # Errors
    ///
    /// [`FemError::MaterialOutOfRange`] if the matrix is not orthonormal or
    /// its determinant is not `+1`.
    pub fn from_matrix(r: [[f64; 3]; 3]) -> Result<Self> {
        let mut worst = 0.0_f64;
        for i in 0..3 {
            for j in 0..3 {
                let mut s = 0.0;
                for k in 0..3 {
                    s += r[k][i] * r[k][j];
                }
                let target = if i == j { 1.0 } else { 0.0 };
                worst = worst.max((s - target).abs());
            }
        }
        if worst > 1.0e-10 {
            return Err(FemError::MaterialOutOfRange {
                parameter: "orientation matrix",
                value: worst,
                unit: "dimensionless",
                reason: "columns must be orthonormal: worst entry of R^T R - I exceeds 1e-10",
            });
        }
        let det = r[0][0] * (r[1][1] * r[2][2] - r[1][2] * r[2][1])
            - r[0][1] * (r[1][0] * r[2][2] - r[1][2] * r[2][0])
            + r[0][2] * (r[1][0] * r[2][1] - r[1][1] * r[2][0]);
        if (det - 1.0).abs() > 1.0e-10 {
            return Err(FemError::MaterialOutOfRange {
                parameter: "orientation determinant",
                value: det,
                unit: "dimensionless",
                reason: "must be +1; a determinant of -1 is a reflection, not a rotation",
            });
        }
        Ok(Self { r })
    }

    /// Build from a **Rodrigues (Gibbs) vector** `r = tan(theta/2) a`, the
    /// representation PRISMS-Plasticity stores orientations in.
    ///
    /// Ported verbatim in form from `rotationOperations.cc`'s `odfpoint`:
    ///
    /// `R_ij = ((1 - r.r) delta_ij + 2 r_i r_j - 2 eps_ijk r_k) / (1 + r.r)`
    ///
    /// which is the active rotation by `theta = 2 atan|r|` about `a = r/|r|`.
    /// `r = 0` gives the identity. A rotation of `180` degrees is at infinity
    /// in this parameterisation and cannot be represented — that is a property
    /// of Rodrigues vectors, not a defect here.
    ///
    /// # Units
    ///
    /// `r` dimensionless; result dimensionless.
    #[must_use]
    pub fn from_rodrigues(r: [f64; 3]) -> Self {
        let rr = r[0] * r[0] + r[1] * r[1] + r[2] * r[2];
        let (t1, t2) = (1.0 - rr, 1.0 + rr);
        let mut m = [[0.0_f64; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                m[i][j] = 2.0 * r[i] * r[j] + if i == j { t1 } else { 0.0 };
            }
        }
        m[0][1] -= 2.0 * r[2];
        m[0][2] += 2.0 * r[1];
        m[1][2] -= 2.0 * r[0];
        m[1][0] += 2.0 * r[2];
        m[2][0] -= 2.0 * r[1];
        m[2][1] += 2.0 * r[0];
        for row in m.iter_mut() {
            for v in row.iter_mut() {
                *v /= t2;
            }
        }
        Self { r: m }
    }

    /// Build from **Bunge (ZXZ) Euler angles** in radians.
    ///
    /// Bunge's `g = R_z(phi2) R_x(Phi) R_z(phi1)` is the **sample-to-crystal**
    /// matrix — the one an EBSD package reports. This constructor forms `g`
    /// and stores its transpose, so the resulting [`Orientation`] is
    /// crystal-to-sample like every other one in this crate.
    ///
    /// # Arguments
    ///
    /// - `phi1` — first rotation about the sample `z` axis, radians,
    ///   conventionally in `[0, 2 pi)`.
    /// - `cap_phi` — rotation about the new `x` axis, radians, conventionally
    ///   in `[0, pi]`.
    /// - `phi2` — final rotation about the new `z` axis, radians,
    ///   conventionally in `[0, 2 pi)`.
    ///
    /// Angles outside those ranges are accepted and simply wrap; no check is
    /// made, because every real value names a valid rotation.
    ///
    /// `(0, 0, 0)` is the cube orientation.
    #[must_use]
    pub fn from_bunge_euler_radians(phi1: f64, cap_phi: f64, phi2: f64) -> Self {
        let (s1, c1) = phi1.sin_cos();
        let (s, c) = cap_phi.sin_cos();
        let (s2, c2) = phi2.sin_cos();
        // g: sample -> crystal, Bunge ZXZ.
        let g = [
            [c1 * c2 - s1 * s2 * c, s1 * c2 + c1 * s2 * c, s2 * s],
            [-c1 * s2 - s1 * c2 * c, -s1 * s2 + c1 * c2 * c, c2 * s],
            [s1 * s, -c1 * s, c],
        ];
        // Store the transpose: crystal -> sample.
        let mut m = [[0.0_f64; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                m[i][j] = g[j][i];
            }
        }
        Self { r: m }
    }

    /// [`Orientation::from_bunge_euler_radians`] with the three angles given in
    /// **degrees**, which is how Euler angles are normally tabulated.
    #[must_use]
    pub fn from_bunge_euler_degrees(phi1: f64, cap_phi: f64, phi2: f64) -> Self {
        let d = std::f64::consts::PI / 180.0;
        Orientation::from_bunge_euler_radians(phi1 * d, cap_phi * d, phi2 * d)
    }

    /// The stored **crystal-to-sample** rotation matrix `R[i][j]`,
    /// dimensionless.
    #[must_use]
    pub fn matrix(&self) -> [[f64; 3]; 3] {
        self.r
    }

    /// Carry a vector from lattice axes into sample axes: `v_s = R v_c`.
    ///
    /// Units: whatever `v` carries, unchanged.
    #[must_use]
    pub fn rotate_vector(&self, v: [f64; 3]) -> [f64; 3] {
        let mut o = [0.0_f64; 3];
        for i in 0..3 {
            o[i] = self.r[i][0] * v[0] + self.r[i][1] * v[1] + self.r[i][2] * v[2];
        }
        o
    }

    /// Carry a symmetric second-order tensor from lattice axes into sample
    /// axes: `A_s = R A_c R^T`.
    ///
    /// The argument and result are in **stress-form** Voigt order (tensor
    /// components, shears *not* doubled) — see [`crate::tensor`]. Rotating an
    /// engineering-shear strain vector with this function would be wrong by a
    /// factor of two on the shears; convert first.
    ///
    /// Units: whatever `a` carries, unchanged.
    #[must_use]
    pub fn rotate_symmetric(&self, a: &Voigt6) -> Voigt6 {
        let f = symmetric_to_full(a);
        let mut t = [[0.0_f64; 3]; 3]; // t = R A
        for i in 0..3 {
            for j in 0..3 {
                let mut s = 0.0;
                for k in 0..3 {
                    s += self.r[i][k] * f[k][j];
                }
                t[i][j] = s;
            }
        }
        let mut o = [[0.0_f64; 3]; 3]; // o = t R^T
        for i in 0..3 {
            for j in 0..3 {
                let mut s = 0.0;
                for k in 0..3 {
                    s += t[i][k] * self.r[j][k];
                }
                o[i][j] = s;
            }
        }
        Voigt6::new(o[0][0], o[1][1], o[2][2], o[1][2], o[0][2], o[0][1])
    }

    /// The orientation obtained by rotating this crystal **rigidly in the
    /// sample frame** by `extra`: the composed matrix is `R' = Q R` with
    /// `Q = extra.matrix()`.
    ///
    /// This is the operation the frame-indifference verification case needs:
    /// rotate the crystal and the applied load by the same `Q` and the
    /// response must follow, `sigma' = Q sigma Q^T`.
    ///
    /// Units: dimensionless.
    #[must_use]
    pub fn pre_rotated_by(&self, extra: &Orientation) -> Orientation {
        let q = extra.r;
        let mut m = [[0.0_f64; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                let mut s = 0.0;
                for k in 0..3 {
                    s += q[i][k] * self.r[k][j];
                }
                m[i][j] = s;
            }
        }
        Orientation { r: m }
    }

    /// The inverse (sample-to-crystal) rotation, i.e. `R^T` stored as an
    /// [`Orientation`] in its own right. Dimensionless.
    #[must_use]
    pub fn inverse(&self) -> Orientation {
        let mut m = [[0.0_f64; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                m[i][j] = self.r[j][i];
            }
        }
        Orientation { r: m }
    }
}

/// Expand a stress-form Voigt symmetric tensor to a full `3x3` array.
///
/// Units: unchanged.
#[must_use]
pub(crate) fn symmetric_to_full(a: &Voigt6) -> [[f64; 3]; 3] {
    let v = a.as_array();
    [
        [v[0], v[5], v[4]],
        [v[5], v[1], v[3]],
        [v[4], v[3], v[2]],
    ]
}
