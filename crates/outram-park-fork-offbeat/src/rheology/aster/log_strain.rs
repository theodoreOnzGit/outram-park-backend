// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from code_aster (https://gitlab.com/codeaster/src)
//   Copyright (C) 1991 - 2026 - EDF R&D
//   Licence: GPL-3.0-or-later
//   Upstream commit: b504ea08c2f49575e04644cee2e39a63ea45c16e
//   Source: bibfor/algorith/gdlog_module.F90 -- `gdlog_defo` (F -> logarithmic
//           strain) and `gdlog_nice_cauchy` (work-conjugate stress T -> PK2 via
//           the projection tensor -> Cauchy). The element-level drivers
//           `ngvlog`/`nglgic` are the finite-element framework and are out of
//           scope for this port.
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! The `GDEF_LOG` finite-strain wrapper.
//!
//! # The idea
//!
//! Writing a constitutive law at finite strain is hard; writing one at small
//! strain is comparatively routine, and the literature is full of them. The
//! logarithmic-strain framework buys the former with the latter: it wraps an
//! **unmodified small-strain law** in a pre- and post-processing pair, and the
//! result is a genuine finite-strain model.
//!
//! Three steps, per integration point, per timestep:
//!
//! 1. **Pre-process.** Turn the deformation gradient `F` into the logarithmic
//!    (Hencky) strain `E = ½ ln(C)`, `C = FᵀF`.
//! 2. **Call the small-strain law**, handing it `E` as though it were an
//!    engineering strain. It returns a stress `T` — the quantity work-conjugate
//!    to `E`, which is *not* any of the usual stress measures.
//! 3. **Post-process.** Map `T` to the second Piola-Kirchhoff stress through the
//!    projection `S = P : T`, then push forward to the true (Cauchy) stress
//!    `σ = F S Fᵀ / J`.
//!
//! # Why it works
//!
//! Because Hencky strain is *additive* in successive coaxial stretches, a law
//! calibrated on small-strain data stays meaningful when the strain is large:
//! stretching by 2 then 3 gives the same logarithmic strain as stretching by 6.
//! The framework is exact for the isotropic case, not an approximation — the
//! projection `P` is precisely the derivative that makes `T : dE` equal the
//! stress power.
//!
//! # What this module does and does not cover
//!
//! **Covers:** the kinematic wrapper — strain pre-processing, the projection
//! tensor, and the stress post-processing, for an isotropic material in 3D.
//!
//! **Does not cover:** the consistent tangent transformation (the `T:d²E`
//! geometric-stiffness term, upstream's `gdlog_rigeo`), and the element-level
//! `B`-matrix machinery, which belongs to a finite-element framework this crate
//! does not have. A caller wanting a Newton tangent at the structural level
//! needs those; a caller integrating a constitutive law at a point does not.
//!
//! # Boundary with OFFBEAT's small-strain mechanics
//!
//! [`crate::mechanics::MechanicsSolver`] is a **small-strain** solver: it
//! assembles equilibrium for `ε = ½(∇D + ∇Dᵀ)` and knows nothing about `F`.
//! These laws are finite-strain. That difference is deliberate and must stay
//! visible rather than implied, so the two meet only through
//! [`LogarithmicStrain::from_displacement_gradient`], which is the one place the
//! conversion happens. Do not feed a small-strain tensor to this wrapper
//! directly; it expects a deformation gradient.

use outram_foam_basic_lib::primitives::{
    eigen_values_symm, eigen_vectors_symm_with, SymmTensor, Tensor, Vector3,
};

use crate::error::Result;
use crate::rheology::aster::kinematics::DeformationGradient;

/// Relative eigenvalue separation below which two principal stretches are
/// treated as coincident.
///
/// The off-diagonal projection coefficient `(ln λi - ln λj) / (λi - λj)` is a
/// removable singularity: it tends to `1/λ` as `λj -> λi`. Evaluating it
/// naively near coincidence subtracts two nearly-equal logarithms and divides
/// by a nearly-zero difference, losing most of the significant digits. Below
/// this separation the limit is used instead.
const EIGEN_COINCIDENCE: f64 = 1.0e-8;

/// A deformation prepared for a small-strain constitutive law.
///
/// Holds the logarithmic strain to hand the law, plus the spectral data needed
/// to map the law's stress back afterwards. Build one per integration point per
/// timestep.
///
/// # Example
///
/// ```no_run
/// use outram_park_fork_offbeat::rheology::aster::{DeformationGradient, LogarithmicStrain};
/// # use outram_foam_basic_lib::primitives::{SymmTensor, Tensor};
/// # fn small_strain_law(_e: SymmTensor) -> SymmTensor { SymmTensor::new(0.,0.,0.,0.,0.,0.) }
/// # fn demo(f: Tensor) -> Result<(), Box<dyn std::error::Error>> {
/// let gradient = DeformationGradient::new(f)?;
/// let wrapper = LogarithmicStrain::new(gradient)?;
///
/// // The law sees a strain and returns its work-conjugate stress.
/// let t = small_strain_law(wrapper.log_strain());
///
/// // Which the wrapper turns into the true stress.
/// let cauchy = wrapper.cauchy_from_conjugate(t);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogarithmicStrain {
    gradient: DeformationGradient,
    /// Logarithmic strain `½ ln(C)`.
    log_strain: SymmTensor,
    /// Eigenvalues of `C` — the squared principal stretches, ascending.
    stretches_squared: Vector3,
    /// Eigenvectors of `C` as rows.
    basis: Tensor,
}

impl LogarithmicStrain {
    /// Prepare a deformation gradient for a small-strain law.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`](crate::error::OffbeatError::Unphysical) if
    /// any squared principal stretch is non-positive — infinite compression, or
    /// a corrupt gradient. Upstream's `gdlog_defo` returns its `iret = 1` for
    /// the same condition.
    pub fn new(gradient: DeformationGradient) -> Result<Self> {
        let c = gradient.right_cauchy_green();
        let log_strain = crate::rheology::aster::kinematics::hencky_strain(c)?;
        let stretches_squared = eigen_values_symm(c);
        let basis = eigen_vectors_symm_with(c, stretches_squared);

        Ok(Self {
            gradient,
            log_strain,
            stretches_squared,
            basis,
        })
    }

    /// Prepare from a small displacement gradient, `F = I + ∇u`.
    ///
    /// The single crossing point between OFFBEAT's small-strain mechanics solve
    /// and this finite-strain layer — see the module documentation.
    pub fn from_displacement_gradient(grad_u: Tensor) -> Result<Self> {
        Self::new(DeformationGradient::from_displacement_gradient(grad_u)?)
    }

    /// The logarithmic strain to hand the small-strain law.
    #[must_use]
    pub fn log_strain(&self) -> SymmTensor {
        self.log_strain
    }

    /// The deformation gradient this was built from.
    #[must_use]
    pub fn gradient(&self) -> DeformationGradient {
        self.gradient
    }

    /// The principal stretches `λ` (not their squares), ascending.
    #[must_use]
    pub fn principal_stretches(&self) -> Vector3 {
        Vector3::new(
            self.stretches_squared.x.sqrt(),
            self.stretches_squared.y.sqrt(),
            self.stretches_squared.z.sqrt(),
        )
    }

    /// Map the law's work-conjugate stress `T` to the second Piola-Kirchhoff
    /// stress `S = P : T`.
    ///
    /// `P = ∂E/∂E_GL` is the projection relating the logarithmic strain to the
    /// Green-Lagrange strain. In the eigenbasis of `C` it acts component-wise:
    ///
    /// - diagonal `i = j`:  `1 / λᵢ`
    /// - off-diagonal `i ≠ j`:  `(ln λᵢ - ln λⱼ) / (λᵢ - λⱼ)`
    ///
    /// where `λ` here are the eigenvalues of `C`, i.e. the *squared* principal
    /// stretches. The off-diagonal expression tends to the diagonal one as the
    /// eigenvalues coincide, which is how the degenerate case is handled.
    #[must_use]
    pub fn second_piola_from_conjugate(&self, t: SymmTensor) -> SymmTensor {
        // Rotate T into the eigenbasis of C.
        let t_local = rotate_to_basis(t, self.basis);

        let l = [
            self.stretches_squared.x,
            self.stretches_squared.y,
            self.stretches_squared.z,
        ];
        let coefficient = |i: usize, j: usize| -> f64 {
            if i == j {
                1.0 / l[i]
            } else {
                let (li, lj) = (l[i], l[j]);
                let separation = (li - lj).abs() / li.abs().max(lj.abs()).max(f64::MIN_POSITIVE);
                if separation < EIGEN_COINCIDENCE {
                    // Removable singularity: the limit as the two stretches
                    // coincide. Evaluating the quotient here would subtract two
                    // nearly-equal logarithms and divide by nearly zero.
                    2.0 / (li + lj)
                } else {
                    (li.ln() - lj.ln()) / (li - lj)
                }
            }
        };

        // Apply component-wise in the eigenbasis.
        let s_local = SymmTensor::new(
            coefficient(0, 0) * t_local.xx,
            coefficient(0, 1) * t_local.xy,
            coefficient(0, 2) * t_local.xz,
            coefficient(1, 1) * t_local.yy,
            coefficient(1, 2) * t_local.yz,
            coefficient(2, 2) * t_local.zz,
        );

        rotate_from_basis(s_local, self.basis)
    }

    /// Push the second Piola-Kirchhoff stress forward to Cauchy stress.
    ///
    /// `σ = F S Fᵀ / J`. This is upstream's `pk2sig`.
    #[must_use]
    pub fn cauchy_from_second_piola(&self, s: SymmTensor) -> SymmTensor {
        let f = self.gradient.tensor();
        let j = self.gradient.jacobian();

        // F S Fᵀ, exploiting the symmetry of S.
        let fs = Tensor::new(
            f.xx * s.xx + f.xy * s.xy + f.xz * s.xz,
            f.xx * s.xy + f.xy * s.yy + f.xz * s.yz,
            f.xx * s.xz + f.xy * s.yz + f.xz * s.zz,
            f.yx * s.xx + f.yy * s.xy + f.yz * s.xz,
            f.yx * s.xy + f.yy * s.yy + f.yz * s.yz,
            f.yx * s.xz + f.yy * s.yz + f.yz * s.zz,
            f.zx * s.xx + f.zy * s.xy + f.zz * s.xz,
            f.zx * s.xy + f.zy * s.yy + f.zz * s.yz,
            f.zx * s.xz + f.zy * s.yz + f.zz * s.zz,
        );

        SymmTensor::new(
            (fs.xx * f.xx + fs.xy * f.xy + fs.xz * f.xz) / j,
            (fs.xx * f.yx + fs.xy * f.yy + fs.xz * f.yz) / j,
            (fs.xx * f.zx + fs.xy * f.zy + fs.xz * f.zz) / j,
            (fs.yx * f.yx + fs.yy * f.yy + fs.yz * f.yz) / j,
            (fs.yx * f.zx + fs.yy * f.zy + fs.yz * f.zz) / j,
            (fs.zx * f.zx + fs.zy * f.zy + fs.zz * f.zz) / j,
        )
    }

    /// The full post-processing step: work-conjugate stress to Cauchy stress.
    ///
    /// Equivalent to [`second_piola_from_conjugate`](Self::second_piola_from_conjugate)
    /// followed by
    /// [`cauchy_from_second_piola`](Self::cauchy_from_second_piola), and the
    /// counterpart of upstream's `gdlog_nice_cauchy`.
    #[must_use]
    pub fn cauchy_from_conjugate(&self, t: SymmTensor) -> SymmTensor {
        self.cauchy_from_second_piola(self.second_piola_from_conjugate(t))
    }
}

/// Rotate a symmetric tensor into a basis whose vectors are the rows of `q`:
/// `q T qᵀ`.
fn rotate_to_basis(t: SymmTensor, q: Tensor) -> SymmTensor {
    let rows = [q.row_x(), q.row_y(), q.row_z()];
    let mut out = [[0.0_f64; 3]; 3];
    for (i, ri) in rows.iter().enumerate() {
        let tri = t.mat_vec(*ri);
        for (j, rj) in rows.iter().enumerate() {
            out[i][j] = tri.dot(*rj);
        }
    }
    SymmTensor::new(
        out[0][0], out[0][1], out[0][2], out[1][1], out[1][2], out[2][2],
    )
}

/// The inverse of [`rotate_to_basis`]: `qᵀ T q`.
fn rotate_from_basis(t: SymmTensor, q: Tensor) -> SymmTensor {
    let rows = [q.row_x(), q.row_y(), q.row_z()];
    let comp = [[t.xx, t.xy, t.xz], [t.xy, t.yy, t.yz], [t.xz, t.yz, t.zz]];

    let mut out = SymmTensor::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for (i, ri) in rows.iter().enumerate() {
        for (j, rj) in rows.iter().enumerate() {
            let c = comp[i][j];
            out = SymmTensor::new(
                out.xx + c * ri.x * rj.x,
                out.xy + c * ri.x * rj.y,
                out.xz + c * ri.x * rj.z,
                out.yy + c * ri.y * rj.y,
                out.yz + c * ri.y * rj.z,
                out.zz + c * ri.z * rj.z,
            );
        }
    }
    out
}

#[cfg(test)]
mod tests;
