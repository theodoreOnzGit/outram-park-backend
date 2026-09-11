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
// Method reference (not a line-by-line port):
//   PRISMS-Plasticity, commit ffdf4eb67b55b84f8b20cbb21407cf310ec3a7e4,
//   src/materialModels/crystalPlasticity/calculatePlasticity.cc line 52
//   (`elasticmoduli(Dmat2, rotmat, elasticStiffnessMatrix)`) rotates the
//   crystal-frame 6x6 stiffness into sample axes with a Voigt "Bond" matrix.
//   The same operation is done here by expanding to the full fourth-order
//   tensor, applying `R (x) R (x) R (x) R`, and contracting back — the arithmetic
//   is heavier but there is no sign or factor-of-two convention to get wrong,
//   and a verification case asserts that rotating an isotropic tensor changes
//   nothing.
//   Upstream's cubic constants for FCC copper, used as this module's example
//   values, are read from
//   applications/crystalPlasticity/fcc/FCC_Random_RateDependent/prm.prm:
//   C11 = 170 GPa, C12 = 124 GPa, C44 = 75 GPa.
// ---------------------------------------------------------------------------

//! Single-crystal elasticity: isotropic or cubic, expressed in the crystal
//! lattice frame and rotated into sample axes by the grain's orientation.
//!
//! # What belongs in this module
//!
//! The elastic stiffness of one grain, and the fourth-order rotation that
//! carries it from lattice axes into sample axes.
//!
//! # What does NOT belong here
//!
//! Plastic flow ([`crate::crystal::flow`]), slip geometry
//! ([`crate::crystal::slip`]) and the orientation type itself
//! ([`crate::crystal::orient`]).
//!
//! # Units
//!
//! Every stiffness constant is in pascals (newton per square metre); the
//! Zener anisotropy ratio is dimensionless.

use crate::crystal::orient::Orientation;
use crate::error::{FemError, Result};
use crate::material::LinearElastic;
use crate::tensor::{Tensor4, VOIGT};

/// Voigt index of the tensor index pair `(i, j)`, for the component order
/// `xx, yy, zz, yz, xz, xy` this crate uses throughout.
#[inline]
const fn voigt_index(i: usize, j: usize) -> usize {
    match (i, j) {
        (0, 0) => 0,
        (1, 1) => 1,
        (2, 2) => 2,
        (1, 2) | (2, 1) => 3,
        (0, 2) | (2, 0) => 4,
        _ => 5,
    }
}

/// The closed set of single-crystal elastic symmetries this crate implements.
///
/// # Units
///
/// All constants in pascals.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CrystalElasticity {
    /// **Isotropic** elasticity. Not physically right for a single crystal of
    /// any real cubic metal, but it is the setting in which a polycrystal must
    /// reduce to von Mises plasticity, so it is what the `J2`-reduction
    /// verification case uses. Tungsten is very nearly isotropic
    /// (Zener ratio 1.01), so it is not purely a numerical device.
    Isotropic(LinearElastic),
    /// **Cubic** elasticity, three independent constants, in pascals:
    ///
    /// - `c11` — `sigma_xx / eps_xx` under uniaxial lattice strain along a
    ///   `<100>` axis. Copper 170 GPa, aluminium 108 GPa, iron 232 GPa.
    /// - `c12` — the transverse coupling `sigma_yy / eps_xx` under the same.
    ///   Copper 124 GPa, aluminium 61 GPa, iron 136 GPa.
    /// - `c44` — the `{100}<010>` shear modulus, `sigma_yz / gamma_yz`.
    ///   Copper 75 GPa, aluminium 29 GPa, iron 117 GPa.
    ///
    /// The **Zener anisotropy ratio** `A = 2 c44 / (c11 - c12)` is `1` exactly
    /// when the crystal is isotropic (copper 3.26, aluminium 1.22, iron 2.42).
    Cubic {
        /// `C11` in pascals.
        c11: f64,
        /// `C12` in pascals.
        c12: f64,
        /// `C44` in pascals.
        c44: f64,
    },
}

impl CrystalElasticity {
    /// Cubic elasticity from its three constants, with the positive-definiteness
    /// conditions checked.
    ///
    /// # Arguments
    ///
    /// All three in pascals. The Born stability conditions for a cubic crystal
    /// are `c11 > |c12|`, `c11 + 2 c12 > 0` and `c44 > 0`; they are exactly
    /// what makes the stiffness positive definite, so they are enforced rather
    /// than assumed.
    ///
    /// # Errors
    ///
    /// [`FemError::MaterialOutOfRange`] if any is violated or any argument is
    /// not finite.
    pub fn cubic(c11: f64, c12: f64, c44: f64) -> Result<Self> {
        let bad = |parameter: &'static str, value: f64, reason: &'static str| {
            Err(FemError::MaterialOutOfRange {
                parameter,
                value,
                unit: "Pa",
                reason,
            })
        };
        if !c11.is_finite() || !c12.is_finite() || !c44.is_finite() {
            return bad("cubic elastic constants", c11, "must all be finite");
        }
        if !(c44 > 0.0) {
            return bad("c44", c44, "Born stability requires c44 > 0");
        }
        if !(c11 > c12.abs()) {
            return bad("c11", c11, "Born stability requires c11 > |c12|");
        }
        if !(c11 + 2.0 * c12 > 0.0) {
            return bad("c11 + 2 c12", c11 + 2.0 * c12, "Born stability requires c11 + 2 c12 > 0");
        }
        Ok(CrystalElasticity::Cubic { c11, c12, c44 })
    }

    /// **Zener anisotropy ratio** `A = 2 c44 / (c11 - c12)`, dimensionless.
    ///
    /// Exactly `1.0` for the isotropic variant, and `1.0` for a cubic variant
    /// whose constants happen to satisfy `2 c44 = c11 - c12`. The further from
    /// `1`, the more the single-crystal response depends on orientation.
    #[must_use]
    pub fn zener_ratio(&self) -> f64 {
        match self {
            CrystalElasticity::Isotropic(_) => 1.0,
            CrystalElasticity::Cubic { c11, c12, c44 } => 2.0 * c44 / (c11 - c12),
        }
    }

    /// The stiffness `C` in the **crystal lattice frame** \[Pa\], in this
    /// crate's Voigt convention (`sigma_V = C eps_V` with engineering shear).
    #[must_use]
    pub fn crystal_stiffness(&self) -> Tensor4 {
        match self {
            CrystalElasticity::Isotropic(e) => e.stiffness(),
            CrystalElasticity::Cubic { c11, c12, c44 } => {
                let mut d = [[0.0_f64; VOIGT]; VOIGT];
                for i in 0..3 {
                    for j in 0..3 {
                        d[i][j] = if i == j { *c11 } else { *c12 };
                    }
                    d[3 + i][3 + i] = *c44;
                }
                Tensor4(d)
            }
        }
    }

    /// The stiffness `C` in **sample axes** \[Pa\], obtained by rotating the
    /// lattice-frame tensor with the grain's orientation:
    /// `C'_ijkl = R_ip R_jq R_kr R_ls C_pqrs`.
    ///
    /// For [`CrystalElasticity::Isotropic`] the result is mathematically
    /// independent of `orientation`; it is still computed through the same
    /// path rather than short-circuited, so that the invariance is a property
    /// the verification case can *measure* rather than one the code asserts by
    /// construction.
    #[must_use]
    pub fn stiffness_in_sample_frame(&self, orientation: &Orientation) -> Tensor4 {
        rotate_stiffness(&self.crystal_stiffness(), orientation)
    }
}

/// Rotate a minor-symmetric fourth-order tensor from lattice axes into sample
/// axes: `C'_ijkl = R_ip R_jq R_kr R_ls C_pqrs`, with `R` the
/// crystal-to-sample matrix.
///
/// Implemented by expanding the 6x6 Voigt matrix to the full `3^4` array,
/// contracting four times, and reading the representative components back.
/// This crate's Voigt convention has `D[I][J] = c_ijkl` with **no** factors of
/// two (see [`crate::tensor::Tensor4`]), so both conversions are plain index
/// lookups.
///
/// Cost is `4 x 3^5 = 972` multiply-adds per call plus the expansion; it is
/// called once per quadrature point per stress update, outside the local
/// Newton loop.
///
/// # Units
///
/// Pascals in, pascals out.
#[must_use]
pub fn rotate_stiffness(c: &Tensor4, orientation: &Orientation) -> Tensor4 {
    let r = orientation.matrix();
    let mut full = [[[[0.0_f64; 3]; 3]; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            for k in 0..3 {
                for l in 0..3 {
                    full[i][j][k][l] = c.0[voigt_index(i, j)][voigt_index(k, l)];
                }
            }
        }
    }
    // Contract one index at a time: 4 passes of 3^5 instead of one of 3^8.
    let mut a = [[[[0.0_f64; 3]; 3]; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            for k in 0..3 {
                for l in 0..3 {
                    let mut s = 0.0;
                    for p in 0..3 {
                        s += r[i][p] * full[p][j][k][l];
                    }
                    a[i][j][k][l] = s;
                }
            }
        }
    }
    let mut b = [[[[0.0_f64; 3]; 3]; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            for k in 0..3 {
                for l in 0..3 {
                    let mut s = 0.0;
                    for q in 0..3 {
                        s += r[j][q] * a[i][q][k][l];
                    }
                    b[i][j][k][l] = s;
                }
            }
        }
    }
    for i in 0..3 {
        for j in 0..3 {
            for k in 0..3 {
                for l in 0..3 {
                    let mut s = 0.0;
                    for m in 0..3 {
                        s += r[k][m] * b[i][j][m][l];
                    }
                    a[i][j][k][l] = s;
                }
            }
        }
    }
    let mut out = [[0.0_f64; VOIGT]; VOIGT];
    const PAIR: [(usize, usize); VOIGT] = [(0, 0), (1, 1), (2, 2), (1, 2), (0, 2), (0, 1)];
    for (big_i, (i, j)) in PAIR.iter().enumerate() {
        for (big_j, (k, l)) in PAIR.iter().enumerate() {
            let mut s = 0.0;
            for n in 0..3 {
                s += r[*l][n] * a[*i][*j][*k][n];
            }
            out[big_i][big_j] = s;
        }
    }
    Tensor4(out)
}
