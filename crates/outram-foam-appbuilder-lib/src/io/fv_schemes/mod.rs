// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
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

//! Parser for OpenFOAM's `system/fvSchemes` — the per-operator numerical scheme
//! selection dictionary. Each scheme family (ddt, grad, div, laplacian, snGrad,
//! interpolation) is a typed enum on [`FvSchemes`], so rust-analyzer surfaces
//! every valid option on hover and an unknown scheme is a `Result` error.

use crate::error::AppBuilderError;
use std::path::Path;

/// Parsed `system/fvSchemes` — numerical scheme selection for each operator.
#[derive(Debug, Clone)]
pub struct FvSchemes {
    pub ddt: DdtScheme,
    pub default_grad: GradScheme,
    /// The `grad(U)` entry of `gradSchemes`, when the case gives one; `None`
    /// means `default_grad` (upstream's lookup falls back to `default` the
    /// same way). Read through [`FvSchemes::grad_u_scheme`]. Consulted by
    /// `div(phi,U) Gauss linearUpwind grad(U)` and by the momentum
    /// Laplacian's non-orthogonal correction, which upstream's
    /// `correctedSnGrad::fullGradCorrection` takes from `grad(U)` (gh:#1202).
    pub grad_u: Option<GradScheme>,
    pub default_div: DivScheme,
    pub default_laplacian: LaplacianScheme,
    pub default_sn_grad: SnGradScheme,
    pub default_interpolation: InterpolationScheme,
}

/// Time-stepping scheme (ddtSchemes).
#[derive(Debug, Clone, PartialEq)]
pub enum DdtScheme {
    Euler,
    Backward,
    CrankNicolson(f64), // off-centring coefficient ψ ∈ [0,1]
    LocalEuler,
    SteadyState,
}

/// Gradient scheme (gradSchemes).
#[derive(Debug, Clone, PartialEq)]
pub enum GradScheme {
    /// `Gauss linear`: `(1/V) Σ_f S_f ⊗ φ_f`, φ_f linearly interpolated.
    GaussLinear,
    /// `cellLimited Gauss linear k` (gh:#1202): the Gauss linear gradient
    /// scaled, per component, so that extrapolating it from the cell centre
    /// to any face centre stays within the min/max of the cell and its face
    /// neighbours (minmod limiter, upstream `cellLimitedGrad<Type, minmod>`).
    /// `k ∈ [0, 1]`: 1 is fully limited, 0 is unlimited. Used by the
    /// OpenFOAM tutorials for `grad(U)` on snappy meshes (`propeller`,
    /// `motorBikeSteady`, `misalignedDuct`).
    CellLimitedGaussLinear(f64),
    LeastSquares,
    FourthOrder,
}

/// Divergence / convection scheme (divSchemes).
#[derive(Debug, Clone, PartialEq)]
pub enum DivScheme {
    /// `Gauss linear`: central differencing, fully implicit, as upstream's
    /// `gaussConvectionScheme` (since gh:#1363; see
    /// `fvm::div_vec_linear`). Unbounded: on a coarse convection-dominated
    /// mesh it can diverge, as upstream's does.
    GaussLinear,
    GaussUpwind,
    /// Central differencing by **deferred correction**: implicit upwind plus
    /// the lagged explicit `φ (U_linear − U_upwind)` in the source, which is
    /// what `GaussLinear` was before gh:#1363. **NOT an OpenFOAM scheme** and
    /// not upstream's `Gauss linear` (it changes `A` and `H` of PISO: 1.4e-3
    /// of the velocity range at step 2 of the cavity differential). Kept
    /// explicit so an ablation can still reproduce it.
    ///
    /// **Deprecated 2026-11-12 (gh:#1500, maintainer: "Deprecate the old
    /// scheme"):** use [`DivScheme::GaussLinear`]. The Foam Studio coil and
    /// its gh:#1157 V&V record now run `GaussLinear`; the one remaining use is
    /// the record's deferred-form ablation row (`--coil-run ... deferred`) and
    /// this scheme's own unit test, each under `#[allow(deprecated)]`.
    #[deprecated(note = "not an OpenFOAM scheme: use `DivScheme::GaussLinear` (upstream's fully implicit Gauss linear); see gh:#1500")]
    GaussLinearDeferred,
    GaussLinearUpwind(String), // e.g. "Gauss linearUpwind grad(U)"
    GaussVanLeer,
    GaussMUSCL,
    GaussLimitedLinear(f64),
}

/// Laplacian scheme (laplacianSchemes).
#[derive(Debug, Clone, PartialEq)]
pub enum LaplacianScheme {
    /// `Gauss linear corrected`: `1/(n·d)` implicit coefficients plus the
    /// explicit non-orthogonal correction. The upstream tutorial default and
    /// the default here; honoured by `PimpleFoam` since gh:#1161.
    GaussLinearCorrected,
    /// `Gauss linear orthogonal`: `1/|d|`, no correction. What `PimpleFoam`
    /// did before gh:#1161; the explicit ablation
    /// (`PimpleFoam::without_non_orthogonal_correction`).
    GaussLinearOrthogonal,
    GaussLinearUncorrected,
    GaussLinearLimited(f64), // limiter coefficient ∈ [0,1]
}

/// Surface-normal gradient scheme (snGradSchemes).
#[derive(Debug, Clone, PartialEq)]
pub enum SnGradScheme {
    Corrected,
    Uncorrected,
    Limited(f64),
}

/// Face interpolation scheme (interpolationSchemes).
#[derive(Debug, Clone, PartialEq)]
pub enum InterpolationScheme {
    Linear,
    Upwind(String), // e.g. "upwind phi"
    Harmonic,
}

impl FvSchemes {
    /// The gradient scheme for `grad(U)`: [`FvSchemes::grad_u`] if set,
    /// otherwise [`FvSchemes::default_grad`] (upstream's
    /// `mesh.schemes().grad("grad(U)")` falls back to `default` likewise).
    pub fn grad_u_scheme(&self) -> &GradScheme {
        self.grad_u.as_ref().unwrap_or(&self.default_grad)
    }

    /// Parse a `system/fvSchemes` file from disk.
    ///
    /// **Not yet implemented — calling this panics (`todo!`).** No OpenFOAM
    /// dictionary parsing exists in this crate; see the sibling
    /// [`crate::io::control_dict::ControlDict::read`] and
    /// [`crate::io::fv_solution::FvSolution::read`], which are in the same
    /// state.
    ///
    /// Build the struct in Rust instead — [`FvSchemes::default`] documents what
    /// the solvers in this crate actually do, and
    /// [`crate::solvers::schemes`] documents which selections are honoured and
    /// which return [`AppBuilderError::UnsupportedScheme`].
    pub fn read(path: &Path) -> Result<Self, AppBuilderError> {
        let _ = path;
        todo!("FvSchemes::read — parse system/fvSchemes")
    }
}

impl Default for FvSchemes {
    /// The defaults describe **what the solvers in this crate actually do** when
    /// no `system/fvSchemes` is supplied, not what a typical OpenFOAM case
    /// writes.
    ///
    /// Notably `default_div` is [`DivScheme::GaussUpwind`], not `Gauss linear`.
    /// Convection in [`crate::solvers::pimple_foam::PimpleFoam`] is assembled by
    /// [`crate::solvers::schemes::div_vec_scheme`], whose bounded first-order
    /// upwind arm is the safe default on the coarse meshes the tutorials use;
    /// second-order central differencing is opted into by setting
    /// `schemes.default_div = DivScheme::GaussLinear`. This field previously
    /// said `GaussLinear` while every solver hardwired upwind and ignored the
    /// struct entirely — the value was a claim nothing honoured.
    ///
    /// ~~`default_grad`, `default_laplacian`, `default_sn_grad` and
    /// `default_interpolation` are **still not consulted by any solver**~~
    /// **CORRECTED 2026-11-03 (gh:#1161):** `default_laplacian` is honoured by
    /// `PimpleFoam` (both its pressure and its momentum Laplacian; see
    /// `crate::solvers::non_orthogonal`), and its default
    /// [`LaplacianScheme::GaussLinearCorrected`] is now what that solver does.
    /// ~~`default_grad`, `default_sn_grad` and `default_interpolation` are still
    /// stored only; the correction's gradient is always `Gauss linear`.~~
    /// **CORRECTED 2026-11-04 (gh:#1202):** the velocity gradient
    /// ([`FvSchemes::grad_u_scheme`], i.e. `grad_u` or else `default_grad`) is
    /// honoured by `PimpleFoam`'s momentum Laplacian correction and by
    /// `Gauss linearUpwind`; the pressure correction's gradient is still
    /// always `Gauss linear`. `default_sn_grad` and `default_interpolation`
    /// are still stored only.
    fn default() -> Self {
        Self {
            ddt: DdtScheme::Euler,
            default_grad: GradScheme::GaussLinear,
            grad_u: None,
            default_div: DivScheme::GaussUpwind,
            default_laplacian: LaplacianScheme::GaussLinearCorrected,
            default_sn_grad: SnGradScheme::Corrected,
            default_interpolation: InterpolationScheme::Linear,
        }
    }
}
