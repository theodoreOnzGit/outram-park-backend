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

//! # Applying `fvSchemes` selections to the assembled equations
//!
//! [`crate::io::fv_schemes::FvSchemes`] parses OpenFOAM's scheme dictionary into
//! typed enums. This module is what makes those enums *do* something: it turns a
//! [`DdtScheme`] or [`DivScheme`] into the corresponding discretisation on a
//! momentum equation.
//!
//! Before this module existed, every solver in this crate stored an `FvSchemes`
//! and then ignored it — the time derivative was hardwired to Euler and
//! convection to first-order upwind, whatever the dictionary said. A scheme
//! selection that is silently discarded is worse than none, because it reads as
//! a promise.
//!
//! ## What is implemented, and what returns an error
//!
//! Unimplemented schemes return
//! [`AppBuilderError::UnsupportedScheme`](crate::error::AppBuilderError::UnsupportedScheme)
//! rather than falling back to a default. A silent fallback would reintroduce
//! exactly the failure mode this module exists to remove.
//!
//! | Family | Implemented | Returns `UnsupportedScheme` |
//! |---|---|---|
//! | ddt | `Euler`, `Backward`, `SteadyState` | `CrankNicolson`, `LocalEuler` |
//! | ddtCorr ([`ddt_corr_scheme`]) | `Euler`, `Backward`, `SteadyState` | `CrankNicolson`, `LocalEuler` |
//! | div | `GaussUpwind`, `GaussLinear`, `GaussLinearUpwind` (gh:#1202) | `GaussVanLeer`, `GaussMUSCL`, `GaussLimitedLinear` |
//! | grad of U ([`grad_vec_scheme`]) | `GaussLinear`, `CellLimitedGaussLinear` (gh:#1202) | `LeastSquares`, `FourthOrder` |
//!
//! ~~The limited/TVD `div` schemes need a face limiter driven by a reconstructed
//! upwind gradient; `outram-foam-basic-lib` provides the limiter functions
//! (`limiters::FluxLimiter`) but not the face-`r` reconstruction for a vector
//! field, so they are declared unsupported rather than approximated.~~
//! **UPDATED 2026-11-04 (gh:#1202):** `Gauss linearUpwind grad(U)` is ported
//! (OpenFOAM-dev `852527377c7e`, `linearUpwind.C`, the vector
//! specialisation), with the `cellLimited Gauss linear k` gradient the
//! tutorials pair it with (`cellLimitedGrad.C`, minmod). The TVD schemes
//! (`vanLeer`, `MUSCL`, `limitedLinear`) are still unsupported.

use crate::error::AppBuilderError;
use crate::io::fv_schemes::{DdtScheme, DivScheme, GradScheme};
use outram_foam_basic_lib::prelude::*;
use std::sync::Arc;

/// Which Rhie–Chow `ddtCorr` form a PISO/PIMPLE solver pairs with its ddt
/// scheme.
///
/// The default, [`DdtCorrForm::MatchDdtScheme`], is the correct physics and
/// what OpenFOAM does: the correction comes from the *same* ddt scheme as the
/// momentum time derivative ([`ddt_corr_scheme`]). [`DdtCorrForm::EulerOnly`]
/// is an explicit **ablation** that reproduces this crate's behaviour before
/// gh:#1067 (Euler `ddtCorr` whatever the ddt scheme); it exists so the
/// temporal-order study in `tests/bdf2_temporal_order.rs` can measure the
/// mismatched pairing beside the consistent one. Do not select it for a
/// production run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DdtCorrForm {
    /// Correction from the selected ddt scheme (Euler → Euler form, Backward →
    /// BDF2 form, steadyState → zero). **Default.**
    #[default]
    MatchDdtScheme,
    /// Ablation: always the Euler form `fvc::ddt_corr`, as before gh:#1067.
    EulerOnly,
}

/// Rhie–Chow time-derivative flux correction `fvc::ddtCorr(U, phi)` for the
/// selected ddt scheme — the face-flux partner of [`ddt_vec_scheme`].
///
/// Returns a face field [m³/s²] (volumetric flux per unit time); a PISO loop
/// adds `interpolate(rAU) · ddtCorr` to `phiHbyA`.
///
/// # Arguments
///
/// * `scheme`      — the `ddtSchemes` selection; must be the one used for the
///   momentum ddt, or the correction and the `H/A` history it replaces do not
///   match.
/// * `u_old`, `u_old_old` — U^{n−1}, U^{n−2} [m/s]; `u_old_old = None` on the
///   first step.
/// * `phi_old`, `phi_old_old` — φ^{n−1}, φ^{n−2} [m³/s]; `phi_old_old = None`
///   on the first step.
/// * `dt` — time step Δt [s] > 0, assumed constant (as in [`ddt_vec_scheme`]).
///
/// # Schemes (each mirrors the upstream `fvcDdtPhiCorr(U, phi)`)
///
/// * [`DdtScheme::Euler`] — `fvc::ddt_corr` (`EulerDdtScheme`).
/// * [`DdtScheme::Backward`] — `fvc::ddt_corr_backward` with `Δt₀ = Δt`
///   (`backwardDdtScheme`); reduces to the Euler form exactly while
///   `u_old_old`/`phi_old_old` are `None`, matching [`ddt_vec_scheme`]'s
///   start-up.
/// * [`DdtScheme::SteadyState`] — a zero field (`steadyStateDdtScheme`
///   returns zero: with no time derivative there is no history to couple to).
///
/// # Errors
///
/// [`AppBuilderError::UnsupportedScheme`] for `CrankNicolson` and `LocalEuler`,
/// for the same reasons as [`ddt_vec_scheme`].
pub fn ddt_corr_scheme(
    scheme: &DdtScheme,
    u_old: &VolVectorField,
    u_old_old: Option<&VolVectorField>,
    phi_old: &SurfaceScalarField,
    phi_old_old: Option<&SurfaceScalarField>,
    dt: f64,
) -> Result<SurfaceScalarField, AppBuilderError> {
    match scheme {
        DdtScheme::Euler => Ok(fvc::ddt_corr(u_old, phi_old, dt)),
        DdtScheme::Backward => Ok(fvc::ddt_corr_backward(
            u_old,
            u_old_old,
            phi_old,
            phi_old_old,
            dt,
            dt,
        )),
        DdtScheme::SteadyState => Ok(SurfaceScalarField::zeros("ddtCorr", phi_old.mesh.clone())),
        DdtScheme::CrankNicolson(psi) => Err(AppBuilderError::UnsupportedScheme {
            family: "ddtSchemes",
            scheme: format!("CrankNicolson({psi})"),
            reason: "needs the stored off-centred ddt0 field carried between time steps",
        }),
        DdtScheme::LocalEuler => Err(AppBuilderError::UnsupportedScheme {
            family: "ddtSchemes",
            scheme: "localEuler".to_string(),
            reason: "needs a per-cell local time-step (LTS) field",
        }),
    }
}

/// Assemble the transient term `∂U/∂t` of a vector transport equation according
/// to `scheme`.
///
/// The returned [`FvVectorMatrix`] is *volume-integrated*: its diagonal carries
/// `V/Δt`-like coefficients [m³/s] and its source `V·U/Δt` [m⁴/s²], matching
/// `fvm::ddt_vec` and the rest of this crate's assembly convention.
///
/// # Arguments
///
/// * `scheme`     — the `ddtSchemes` selection.
/// * `u`          — the velocity field being solved for, U^n [m/s].
/// * `u_old`      — U at the previous time level, U^{n−1} [m/s].
/// * `u_old_old`  — U at the level before that, U^{n−2} [m/s]. Required by
///   [`DdtScheme::Backward`]; pass `None` on the first step of a run, where this
///   function falls back to Euler for that step (the standard second-order
///   backward start-up, since U^{n−2} does not exist yet).
/// * `dt`         — time step Δt [s], must be > 0.
/// * `mesh`       — the mesh.
///
/// # Schemes
///
/// * [`DdtScheme::Euler`] — first-order implicit Euler, `(U^n − U^{n−1})/Δt`.
///   Bounded and unconditionally stable; the default.
/// * [`DdtScheme::Backward`] — second-order backward differencing,
///   `(3U^n − 4U^{n−1} + U^{n−2})/(2Δt)`. Diagonal coefficient `1.5 V/Δt`,
///   source `V(2U^{n−1} − 0.5U^{n−2})/Δt`. More accurate but less bounded; can
///   ring on a coarse mesh at high Courant number.
///
///   ~~**Known limitation — `Backward` is wired, not verified as second order.**
///   [`crate::solvers::pimple_foam::PimpleFoam`]'s PISO loop adds a Rhie–Chow
///   flux correction `rAU_f · fvc::ddt_corr(U_old, φ_old, Δt)`, and
///   `outram-foam-basic-lib`'s `ddt_corr` implements only the **Euler** form.
///   Selecting `Backward` changes `rAU = V/A` (the diagonal becomes `1.5 V/Δt`
///   instead of `V/Δt`) without changing `ddt_corr`, so the two are
///   inconsistent by a ratio tending to 1.5 as Δt → 0.~~ Measured consequence
///   (`tests/fv_scheme_selection.rs`, 2026-08-07): the Euler and Backward
///   lid-driven-cavity runs converge to steady states differing by 1.0e-2 to
///   2.9e-2 m/s (1–3 % of U_lid), and that difference *grows* as Δt is
///   refined — the signature of an inconsistency, not of truncation error.
///   ~~Fixing it needs OpenFOAM's `backwardDdtScheme::fvcDdtPhiCorr`, which
///   belongs in `outram-foam-basic-lib`. Until then, treat `Backward` as
///   available and benchmark-neutral (it scores marginally better than Euler
///   against Ghia 1982 at every Δt tested), but not as a verified second-order
///   time integration.~~
///
///   **CORRECTED 2026-11-01** (gh:#1067) — the partner is ported:
///   `outram_foam_basic_lib::fv_operators::fvc::ddt_corr_backward`
///   (`backwardDdtScheme::fvcDdtPhiCorr`), and `PimpleFoam` now selects it
///   through [`ddt_corr_scheme`] whenever `Backward` is chosen (the old
///   pairing survives as the [`DdtCorrForm::EulerOnly`] ablation). Measured
///   2026-11-01 (`tests/bdf2_temporal_order.rs`, record
///   `verification_and_validation/bdf2_temporal_order.md`): the steady
///   Euler/Backward gap falls 2.7–3.6× (8.04e-3 vs 2.93e-2 m/s at Δt = 1e-3)
///   but **still grows** as Δt is refined, and a Richardson temporal-order
///   study on the cavity gave order ≈ 0 for Euler and Backward alike, because
///   the discrete fixed point depends on Δt. So `Backward` is now paired
///   consistently with upstream, but it is **still not verified as second
///   order**.
///   This function assumes a **constant** Δt (weights `1.5`, `2`, `0.5`);
///   [`ddt_corr_scheme`] makes the same assumption.
/// * [`DdtScheme::SteadyState`] — the term is dropped entirely (a zero matrix),
///   for steady solvers.
///
/// # Errors
///
/// [`AppBuilderError::UnsupportedScheme`] for `CrankNicolson` (needs the stored
/// off-centred `ddt0` field OpenFOAM keeps between steps) and `LocalEuler`
/// (needs a per-cell local time-step field).
pub fn ddt_vec_scheme(
    scheme: &DdtScheme,
    u: &VolVectorField,
    u_old: &VolVectorField,
    u_old_old: Option<&VolVectorField>,
    dt: f64,
    mesh: Arc<FvMesh>,
) -> Result<FvVectorMatrix, AppBuilderError> {
    match scheme {
        DdtScheme::Euler => Ok(fvm::ddt_vec(u, u_old, dt, mesh)),
        DdtScheme::SteadyState => Ok(FvVectorMatrix::new(mesh)),
        DdtScheme::Backward => {
            let Some(u_oo) = u_old_old else {
                // First step of a run: U^{n−2} does not exist. OpenFOAM's
                // `backwardDdtScheme` degenerates to Euler here too.
                return Ok(fvm::ddt_vec(u, u_old, dt, mesh));
            };
            let mut mat = FvVectorMatrix::new(mesh.clone());
            for c in 0..mesh.n_cells {
                let v = mesh.cell_volumes[c];
                mat.ldu.diag[c] += 1.5 * v / dt;
                mat.source[c] += (u_old.internal[c] * 2.0 - u_oo.internal[c] * 0.5) * (v / dt);
            }
            Ok(mat)
        }
        DdtScheme::CrankNicolson(psi) => Err(AppBuilderError::UnsupportedScheme {
            family: "ddtSchemes",
            scheme: format!("CrankNicolson({psi})"),
            reason: "needs the stored off-centred ddt0 field carried between time steps",
        }),
        DdtScheme::LocalEuler => Err(AppBuilderError::UnsupportedScheme {
            family: "ddtSchemes",
            scheme: "localEuler".to_string(),
            reason: "needs a per-cell local time-step (LTS) field",
        }),
    }
}

/// Assemble the convection term `∇·(φ U)` of a vector transport equation
/// according to `scheme`.
///
/// # Arguments
///
/// * `scheme` — the `divSchemes` selection for `div(phi,U)`.
/// * `grad`   — the `grad(U)` scheme ([`crate::io::fv_schemes::FvSchemes::grad_u_scheme`]);
///   consulted only by [`DivScheme::GaussLinearUpwind`].
/// * `phi`    — face flux φ = U·S_f [m³/s] (volumetric) or ρU·S_f [kg/s]
///   (mass); the scheme is agnostic, the units follow `phi`.
/// * `u`      — the transported velocity field [m/s].
/// * `mesh`   — the mesh.
///
/// # Schemes
///
/// * [`DivScheme::GaussUpwind`] — first-order upwind, `fvm::div_vec` unmodified.
///   Unconditionally bounded, strongly diffusive on a coarse mesh.
/// * [`DivScheme::GaussLinear`] — second-order central differencing, fully
///   IMPLICIT as upstream's `gaussConvectionScheme` with the `linear` weights
///   (`fvm::div_vec_linear`: `lower = -w φ`, `upper = lower + φ`, `negSumDiag`).
///   ~~Until gh:#1363 this was the upwind matrix plus a lagged explicit
///   (Khosla–Rubin deferred) correction, which is a different discrete operator
///   inside a time step~~ **CORRECTED 2026-11-11** (cavity differential against
///   OpenFOAM-dev icoFoam: 1.4e-3 of the velocity range at step 2 before,
///   ~1e-13 after). The matrix is no longer diagonally dominant and the scheme
///   is **not** bounded — central differencing on a convection-dominated coarse
///   mesh can produce over/undershoots.
///
///   Boundary faces are treated as in `fvm::div_vec` (the boundary
///   coefficients do not depend on the interior weights).
/// * [`DivScheme::GaussLinearUpwind`] — OpenFOAM's `linearUpwind`
///   (gh:#1202): upwind weights in the implicit matrix plus the explicit
///   correction `φ_f (C_f − C_u)·(∇U)_u` of [`linear_upwind_correction`],
///   `u` the upwind cell and `∇U` the [`grad_vec_scheme`] gradient. Second
///   order, upwind-biased; exact for a linear field when the gradient is.
///   The `String` (upstream's gradient scheme name, e.g. `grad(U)`) is not
///   looked up: the port has one velocity gradient entry, `grad`.
///
/// # Errors
///
/// [`AppBuilderError::UnsupportedScheme`] for the TVD schemes (`vanLeer`,
/// `MUSCL`, `limitedLinear`), and for `linearUpwind` with an unsupported
/// gradient — see the module documentation.
pub fn div_vec_scheme(
    scheme: &DivScheme,
    grad: &GradScheme,
    phi: &SurfaceScalarField,
    u: &VolVectorField,
    mesh: Arc<FvMesh>,
) -> Result<FvVectorMatrix, AppBuilderError> {
    let unsupported = |name: &str| AppBuilderError::UnsupportedScheme {
        family: "divSchemes",
        scheme: name.to_string(),
        reason: "TVD convection (vanLeer, MUSCL, limitedLinear) needs a face limiter \
                 driven by a reconstructed upwind ratio, which is not yet ported",
    };

    match scheme {
        DivScheme::GaussUpwind => Ok(fvm::div_vec(phi, u, mesh)),
        // gh:#1363: upstream `gaussConvectionScheme` with the `linear`
        // weights is fully implicit (`lower = -w*phi`, `upper = lower + phi`);
        // this arm used to be implicit upwind plus a lagged explicit
        // correction, a different operator inside a time step.
        DivScheme::GaussLinear => Ok(fvm::div_vec_linear(phi, u, mesh)),
        // The pre-gh:#1363 `Gauss linear`: implicit upwind plus a lagged
        // explicit central correction. Not an OpenFOAM scheme.
        DivScheme::GaussLinearDeferred => {
            let mut mat = fvm::div_vec(phi, u, mesh.clone());
            let u_f = fvc::interpolate(u); // geometric linear weighting
            for f in 0..mesh.n_internal_faces {
                let o = mesh.owner[f];
                let nb = mesh.neighbour[f];
                let phi_f = phi.internal[f];
                let u_up = if phi_f >= 0.0 {
                    u.internal[o]
                } else {
                    u.internal[nb]
                };
                let d = (u_f.internal[f] - u_up) * phi_f;
                mat.source[o] -= d;
                mat.source[nb] += d;
            }
            Ok(mat)
        }
        DivScheme::GaussLinearUpwind(_) => {
            let mut mat = fvm::div_vec(phi, u, mesh.clone());
            let g = grad_vec_scheme(grad, u)?;
            let corr = linear_upwind_correction(phi, &g);
            for (f, c) in corr.iter().enumerate() {
                // upstream: fvm += fvc::surfaceIntegrate(faceFlux*correction),
                // i.e. source −= Σ_f φ_f c_f with the owner/neighbour signs
                // (the same convention as the `GaussLinear` arm above).
                let d = *c * phi.internal[f];
                mat.source[mesh.owner[f]] -= d;
                mat.source[mesh.neighbour[f]] += d;
            }
            Ok(mat)
        }
        DivScheme::GaussVanLeer => Err(unsupported("Gauss vanLeer")),
        DivScheme::GaussMUSCL => Err(unsupported("Gauss MUSCL")),
        DivScheme::GaussLimitedLinear(k) => Err(unsupported(&format!("Gauss limitedLinear {k}"))),
    }
}

/// The `grad(U)` gradient of `u` by `scheme` (gh:#1202).
///
/// * [`GradScheme::GaussLinear`] — `fvc::grad_vec`, unchanged (OpenFOAM
///   `gaussGrad`): `(1/V) Σ_f S_f ⊗ u_f`.
/// * [`GradScheme::CellLimitedGaussLinear`]`(k)` — that gradient limited by
///   [`cell_limited_grad_vec`].
///
/// Returns `∂u_j/∂x_i` in `T_ij` (the OpenFOAM index order of `grad_vec`).
///
/// # Errors
/// [`AppBuilderError::UnsupportedScheme`] for `leastSquares` and `fourth`
/// (not ported for vector fields).
pub fn grad_vec_scheme(scheme: &GradScheme, u: &VolVectorField) -> Result<VolTensorField, AppBuilderError> {
    match scheme {
        GradScheme::GaussLinear => Ok(fvc::grad_vec(u)),
        GradScheme::CellLimitedGaussLinear(k) => Ok(cell_limited_grad_vec(u, fvc::grad_vec(u), *k)),
        GradScheme::LeastSquares => Err(AppBuilderError::UnsupportedScheme {
            family: "gradSchemes",
            scheme: "leastSquares".to_string(),
            reason: "the least-squares gradient is ported for scalar fields only",
        }),
        GradScheme::FourthOrder => Err(AppBuilderError::UnsupportedScheme {
            family: "gradSchemes",
            scheme: "fourth".to_string(),
            reason: "not ported",
        }),
    }
}

/// Component `i` (0 = x, 1 = y, 2 = z) of a vector.
fn cmpt(v: Vector3, i: usize) -> f64 {
    match i {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    }
}

/// Port of OpenFOAM's `cellLimitedGrad<vector, minmod>::calcGrad`
/// (OpenFOAM-dev `852527377c7e`,
/// `src/finiteVolume/finiteVolume/gradSchemes/limitedGradSchemes/cellLimitedGrad/cellLimitedGrad.{C,H}`,
/// limiter `gradientLimiters/minmodGradientLimiter.H`).
///
/// For each cell and each velocity component `j`:
///
/// 1. `max_j`, `min_j`: the largest and smallest `u_j` over the cell and
///    every face neighbour; across a boundary face the neighbour is the
///    boundary face value (the paired cell's value across a cyclic seam, as
///    upstream's `patchNeighbourField`). Then `max_j −= u_j`, `min_j −= u_j`.
/// 2. If `k < 1`: widen both by `(1/k − 1)(max_j − min_j)`.
/// 3. For every face of the cell, the extrapolated change
///    `e_j = ((C_f − C)·∇u)_j`; if `e_j > small` the ratio `r = max_j/e_j`,
///    if `e_j < −small` `r = min_j/e_j`, otherwise the face is skipped; the
///    cell's limiter `l_j = min(l_j, min(r, 1))` (minmod), starting at 1.
/// 4. `(∇u)_ij ← l_j (∇u)_ij`: each component's gradient is scaled by its
///    own limiter (upstream's `cmptMultiply` on each row).
///
/// With `k < small` (1e-15, OpenFOAM's `small`) the gradient is returned
/// unlimited, as upstream.
///
/// # Arguments
/// * `u`    — the field whose gradient this is (cell and boundary values).
/// * `grad` — its unlimited gradient (usually `fvc::grad_vec(u)`).
/// * `k`    — the limiter coefficient in `[0, 1]`.
///
/// # Panics
/// If `k` is outside `[0, 1]` (upstream stops with a `FatalIOError`).
pub fn cell_limited_grad_vec(u: &VolVectorField, mut grad: VolTensorField, k: f64) -> VolTensorField {
    assert!((0.0..=1.0).contains(&k), "cellLimited coefficient {k} must be in [0, 1]");
    const SMALL: f64 = 1.0e-15;
    if k < SMALL {
        return grad;
    }
    let mesh = u.mesh.clone();
    let n = mesh.n_cells;
    let as3 = |v: Vector3| [v.x, v.y, v.z];
    let mut max_v: Vec<[f64; 3]> = (0..n).map(|c| as3(u.internal[c])).collect();
    let mut min_v = max_v.clone();
    let widen = |c: usize, v: Vector3, max_v: &mut [[f64; 3]], min_v: &mut [[f64; 3]]| {
        for i in 0..3 {
            max_v[c][i] = max_v[c][i].max(cmpt(v, i));
            min_v[c][i] = min_v[c][i].min(cmpt(v, i));
        }
    };
    for f in 0..mesh.n_internal_faces {
        let (o, nb) = (mesh.owner[f], mesh.neighbour[f]);
        widen(o, u.internal[nb], &mut max_v, &mut min_v);
        widen(nb, u.internal[o], &mut max_v, &mut min_v);
    }
    let u_b = fvc::interpolate(u);
    for (pi, p) in mesh.patches.iter().enumerate() {
        for fi in 0..p.size {
            let gf = p.start + fi;
            let partner = if p.kind == PatchKind::Cyclic { mesh.cyclic_partner_face(gf) } else { None };
            let v = match partner {
                Some(pf) => u.internal[mesh.owner[pf]],
                None => u_b.boundary[pi].values[fi],
            };
            widen(mesh.owner[gf], v, &mut max_v, &mut min_v);
        }
    }
    for c in 0..n {
        for i in 0..3 {
            let ui = cmpt(u.internal[c], i);
            max_v[c][i] -= ui;
            min_v[c][i] -= ui;
            if k < 1.0 {
                let w = (1.0 / k - 1.0) * (max_v[c][i] - min_v[c][i]);
                max_v[c][i] += w;
                min_v[c][i] -= w;
            }
        }
    }
    let mut lim = vec![[1.0_f64; 3]; n];
    let limit_face = |c: usize, cf: Vector3, lim: &mut [[f64; 3]], g: &VolTensorField| {
        let e = Tensor::vec_mat(cf - mesh.cell_centres[c], g.internal[c]);
        for i in 0..3 {
            let ei = cmpt(e, i);
            let r = if ei > SMALL {
                max_v[c][i] / ei
            } else if ei < -SMALL {
                min_v[c][i] / ei
            } else {
                continue;
            };
            lim[c][i] = lim[c][i].min(r.min(1.0));
        }
    };
    for f in 0..mesh.n_internal_faces {
        limit_face(mesh.owner[f], mesh.face_centres[f], &mut lim, &grad);
        limit_face(mesh.neighbour[f], mesh.face_centres[f], &mut lim, &grad);
    }
    for f in mesh.n_internal_faces..mesh.owner.len() {
        limit_face(mesh.owner[f], mesh.face_centres[f], &mut lim, &grad);
    }
    for (c, [lx, ly, lz]) in lim.into_iter().enumerate() {
        let t = &mut grad.internal[c];
        t.xx *= lx;
        t.yx *= lx;
        t.zx *= lx;
        t.xy *= ly;
        t.yy *= ly;
        t.zy *= ly;
        t.xz *= lz;
        t.yz *= lz;
        t.zz *= lz;
    }
    grad
}

/// Port of OpenFOAM's `linearUpwind<vector>::correction` (OpenFOAM-dev
/// `852527377c7e`,
/// `src/finiteVolume/interpolation/surfaceInterpolation/schemes/linearUpwind/linearUpwind.C`):
/// on each internal face, the explicit correction to the upwind face value,
///
/// ```text
///   c_f = (C_f − C_u) · (∇U)_u,   u = owner if φ_f > 0, else neighbour
/// ```
///
/// (`(d·∇U)_j = Σ_i d_i ∂U_j/∂x_i`); the face value is `U_u + c_f`.
/// Non-coupled boundary faces carry no correction (upstream leaves them at
/// zero); cyclic seams are not corrected here either (upstream corrects
/// coupled patches; no case in this crate runs `linearUpwind` on one).
/// At `φ_f = 0` upstream takes the **neighbour** while the upwind weights
/// take the owner; the convective flux is zero there either way.
pub fn linear_upwind_correction(phi: &SurfaceScalarField, grad: &VolTensorField) -> Vec<Vector3> {
    let mesh = &grad.mesh;
    (0..mesh.n_internal_faces)
        .map(|f| {
            let c = if phi.internal[f] > 0.0 { mesh.owner[f] } else { mesh.neighbour[f] };
            Tensor::vec_mat(mesh.face_centres[f] - mesh.cell_centres[c], grad.internal[c])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::prelude::{BoundaryPatch, FvMeshBuilder, PatchKind};

    fn vx(x: f64) -> Vector3 {
        Vector3::new(x, 0.0, 0.0)
    }

    /// 3 equal cells along x (centres 0.5, 1.5, 2.5 m, unit volume, unit face
    /// area), 2 internal faces, ordinary patches at each end.
    fn line_mesh() -> Arc<FvMesh> {
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(3)
                .n_internal_faces(2)
                .owner(vec![0, 1, 0, 2])
                .neighbour(vec![1, 2])
                .patches(vec![
                    BoundaryPatch::new("left", 2, 1, PatchKind::Patch),
                    BoundaryPatch::new("right", 3, 1, PatchKind::Patch),
                ])
                .cell_volumes(vec![1.0, 1.0, 1.0])
                .cell_centres(vec![vx(0.5), vx(1.5), vx(2.5)])
                .face_area_vectors(vec![vx(1.0), vx(1.0), vx(-1.0), vx(1.0)])
                .face_centres(vec![vx(1.0), vx(2.0), vx(0.0), vx(3.0)])
                .build()
                .unwrap(),
        )
    }

    /// V&V (verification) — `DdtScheme::Backward` reproduces the second-order
    /// backward-differencing formula exactly.
    ///
    /// **Methodology.** Reference (analytical): BDF2 is
    /// `∂U/∂t ≈ (3U^n − 4U^{n−1} + U^{n−2})/(2Δt)`, so the volume-integrated
    /// matrix must have `diag = 1.5 V/Δt` and `source = V(2U^{n−1} −
    /// 0.5U^{n−2})/Δt`. Inputs: the 3-cell unit-volume line mesh, Δt = 0.1 s,
    /// U^{n−1} = (2,0,0) m/s, U^{n−2} = (1,0,0) m/s. Expected diag = 1.5/0.1 =
    /// 15 m³/s; expected source_x = (2·2 − 0.5·1)/0.1 = 35 m⁴/s².
    /// Pass criterion: |Δ| < 1e-12 for every cell.
    ///
    /// **Results (run 2026-08-07, release mode).** diag = [15, 15, 15] m³/s and
    /// source_x = [35, 35, 35] m⁴/s² in all three cells; max |Δ| = 0.0. The
    /// Euler assembly on the same inputs gives diag = 10 m³/s and source_x = 20
    /// m⁴/s², confirming the two schemes are genuinely different operators and
    /// not one aliased to the other.
    #[test]
    fn backward_ddt_matches_the_bdf2_formula() {
        let mesh = line_mesh();
        let dt = 0.1;
        let u = VolVectorField::zero("U", mesh.clone());
        let mut u_old = VolVectorField::zero("U0", mesh.clone());
        u_old.internal = Field::new(vec![vx(2.0); 3]);
        let mut u_oo = VolVectorField::zero("U00", mesh.clone());
        u_oo.internal = Field::new(vec![vx(1.0); 3]);

        let bwd = ddt_vec_scheme(
            &DdtScheme::Backward,
            &u,
            &u_old,
            Some(&u_oo),
            dt,
            mesh.clone(),
        )
        .expect("backward is supported");
        for c in 0..mesh.n_cells {
            assert!((bwd.ldu.diag[c] - 15.0).abs() < 1e-12, "diag[{c}]");
            assert!((bwd.source[c].x - 35.0).abs() < 1e-12, "source[{c}]");
        }

        let eul = ddt_vec_scheme(&DdtScheme::Euler, &u, &u_old, None, dt, mesh.clone())
            .expect("euler is supported");
        assert!((eul.ldu.diag[0] - 10.0).abs() < 1e-12);
        assert!((eul.source[0].x - 20.0).abs() < 1e-12);

        // Without U^{n−2}, backward must degenerate to Euler (run start-up).
        let start = ddt_vec_scheme(&DdtScheme::Backward, &u, &u_old, None, dt, mesh.clone())
            .expect("backward start-up falls back to Euler");
        assert!((start.ldu.diag[0] - eul.ldu.diag[0]).abs() < 1e-15);
        assert!((start.source[0] - eul.source[0]).mag() < 1e-15);

        // SteadyState drops the term entirely.
        let steady = ddt_vec_scheme(&DdtScheme::SteadyState, &u, &u_old, None, dt, mesh.clone())
            .expect("steadyState is supported");
        assert!(steady.ldu.diag.iter().all(|d| d.abs() < 1e-15));
    }


    /// V&V (verification) — `DivScheme::GaussLinear` is the fully IMPLICIT
    /// central operator of upstream `gaussConvectionScheme` (gh:#1363).
    ///
    /// **Methodology.** Reference (analytical): on the 3-cell unit line with
    /// φ = 1 m³/s on both internal faces and linear weight w = 1/2,
    /// `lower = -w φ = -0.5`, `upper = lower + φ = 0.5`,
    /// `diag = negSumDiag = [0.5, 0, -0.5]` (+ boundary terms, common to
    /// upwind). So `lin.diag - up.diag = [-0.5, -1, -0.5]`, every `upper` is
    /// 0.5 and every `lower` -0.5, and the interior source is NOT touched
    /// (the previous implementation put the central part in a lagged source).
    /// Pass criterion: |Δ| < 1e-15. This replaces the 2026-08-07 test of the
    /// deferred-correction form, which asserted the opposite (matrix equal to
    /// upwind, source carrying the correction): changed because that form is
    /// not upstream's operator (gh:#1363, cavity differential).
    #[test]
    fn linear_div_is_the_fully_implicit_central_operator() {
        let mesh = line_mesh();
        let mut u = VolVectorField::zero("U", mesh.clone());
        u.internal = Field::new(vec![vx(1.0), vx(2.0), vx(3.0)]);
        let mut phi = SurfaceScalarField::zeros("phi", mesh.clone());
        phi.internal[0] = 1.0;
        phi.internal[1] = 1.0;

        let g = GradScheme::GaussLinear;
        let up = div_vec_scheme(&DivScheme::GaussUpwind, &g, &phi, &u, mesh.clone()).unwrap();
        let lin = div_vec_scheme(&DivScheme::GaussLinear, &g, &phi, &u, mesh.clone()).unwrap();

        let want_diag_shift = [-0.5, -1.0, -0.5];
        for c in 0..3 {
            let d = lin.ldu.diag[c] - up.ldu.diag[c];
            assert!((d - want_diag_shift[c]).abs() < 1e-15, "diag shift[{c}] = {d}");
            assert!((lin.source[c].x - up.source[c].x).abs() < 1e-15, "source[{c}]");
        }
        for f in 0..mesh.n_internal_faces {
            assert!((lin.ldu.upper[f] - 0.5).abs() < 1e-15);
            assert!((lin.ldu.lower[f] + 0.5).abs() < 1e-15);
        }
    }

    /// V&V (verification) — `DivScheme::GaussLinearDeferred` is the
    /// pre-gh:#1363 `Gauss linear`: the upwind matrix unchanged and the central
    /// correction in the source. This is the 2026-08-07 test of that form,
    /// kept for the explicit variant (it asserted `source_linear -
    /// source_upwind = [-0.5, 0, 0.5]` on the 3-cell line with U = {1, 2, 3},
    /// phi = 1, and the diagonal and off-diagonals equal to upwind's; measured
    /// then and re-measured 2026-11-12 with max |delta| = 0).
    #[test]
    fn deferred_linear_div_adds_the_exact_central_difference_correction() {
        let mesh = line_mesh();
        let mut u = VolVectorField::zero("U", mesh.clone());
        u.internal = Field::new(vec![vx(1.0), vx(2.0), vx(3.0)]);
        let mut phi = SurfaceScalarField::zeros("phi", mesh.clone());
        phi.internal[0] = 1.0;
        phi.internal[1] = 1.0;

        let g = GradScheme::GaussLinear;
        let up = div_vec_scheme(&DivScheme::GaussUpwind, &g, &phi, &u, mesh.clone()).unwrap();
        let lin = div_vec_scheme(&DivScheme::GaussLinearDeferred, &g, &phi, &u, mesh.clone()).unwrap();

        let expected = [-0.5, 0.0, 0.5];
        for (c, want) in expected.iter().enumerate() {
            let d = lin.source[c].x - up.source[c].x;
            assert!((d - want).abs() < 1e-12, "cell {c}: correction {d} expected {want}");
            assert!((lin.ldu.diag[c] - up.ldu.diag[c]).abs() < 1e-15, "diag[{c}]");
        }
        for f in 0..mesh.n_internal_faces {
            assert!((lin.ldu.upper[f] - up.ldu.upper[f]).abs() < 1e-15);
            assert!((lin.ldu.lower[f] - up.ldu.lower[f]).abs() < 1e-15);
        }
    }

    /// Unimplemented scheme selections must return an error, never silently fall
    /// back to a default — the failure mode this module was written to remove.
    #[test]
    fn unimplemented_schemes_error_rather_than_falling_back() {
        let mesh = line_mesh();
        let u = VolVectorField::zero("U", mesh.clone());
        let phi = SurfaceScalarField::zeros("phi", mesh.clone());

        for s in [
            DivScheme::GaussVanLeer,
            DivScheme::GaussMUSCL,
            DivScheme::GaussLimitedLinear(1.0),
        ] {
            assert!(
                div_vec_scheme(&s, &GradScheme::GaussLinear, &phi, &u, mesh.clone()).is_err(),
                "{s:?} must be reported as unsupported"
            );
        }
        for s in [DdtScheme::CrankNicolson(0.9), DdtScheme::LocalEuler] {
            assert!(
                ddt_vec_scheme(&s, &u, &u, None, 0.1, mesh.clone()).is_err(),
                "{s:?} must be reported as unsupported"
            );
        }
    }

    /// `ddt_corr_scheme` dispatches to the upstream partner of each ddt scheme.
    ///
    /// Inputs: the 3-cell line mesh, U^{n−1} = (1,0,0), U^{n−2} = (0.5,0,0) m/s,
    /// φ^{n−1} = 2, φ^{n−2} = 1.5 m³/s on every face, Δt = 1 s. Expected
    /// (hand-computed): Euler `0.5·(2 − 1)/1 = 0.5`; Backward
    /// `0.5·[(2·2 − 0.5·1.5) − (2·1 − 0.5·0.5)]/1 = 0.75`; steadyState 0;
    /// Backward with no old-old level = the Euler value bit for bit;
    /// CrankNicolson an error. Results (2026-11-01): as expected, |Δ| < 1e-12.
    #[test]
    fn ddt_corr_scheme_dispatches_to_the_matching_partner() {
        let m = line_mesh();
        let u0 = VolVectorField::uniform("U", m.clone(), vx(1.0));
        let u00 = VolVectorField::uniform("U", m.clone(), vx(0.5));
        let mut p0 = SurfaceScalarField::zeros("phi", m.clone());
        let mut p00 = SurfaceScalarField::zeros("phi", m.clone());
        for f in 0..m.n_internal_faces {
            p0.internal[f] = 2.0;
            p00.internal[f] = 1.5;
        }
        let e = ddt_corr_scheme(&DdtScheme::Euler, &u0, Some(&u00), &p0, Some(&p00), 1.0).unwrap();
        let b =
            ddt_corr_scheme(&DdtScheme::Backward, &u0, Some(&u00), &p0, Some(&p00), 1.0).unwrap();
        let b1 = ddt_corr_scheme(&DdtScheme::Backward, &u0, None, &p0, None, 1.0).unwrap();
        let z = ddt_corr_scheme(
            &DdtScheme::SteadyState,
            &u0,
            Some(&u00),
            &p0,
            Some(&p00),
            1.0,
        )
        .unwrap();
        for f in 0..m.n_internal_faces {
            assert!(
                (e.internal[f] - 0.5).abs() < 1e-12,
                "euler {}",
                e.internal[f]
            );
            assert!(
                (b.internal[f] - 0.75).abs() < 1e-12,
                "backward {}",
                b.internal[f]
            );
            assert_eq!(b1.internal[f].to_bits(), e.internal[f].to_bits());
            assert_eq!(z.internal[f], 0.0);
        }
        assert!(
            ddt_corr_scheme(&DdtScheme::CrankNicolson(0.9), &u0, None, &p0, None, 1.0).is_err()
        );
        assert!(ddt_corr_scheme(&DdtScheme::LocalEuler, &u0, None, &p0, None, 1.0).is_err());
        assert_eq!(DdtCorrForm::default(), DdtCorrForm::MatchDdtScheme);
    }

    /// 3 cells along x of widths 1, 2, 1 m (faces at x = 0, 1, 3, 4; centres
    /// 0.5, 2, 3.5), unit face area; patches `left` (x = 0) and `right`
    /// (x = 4). `dy` shifts the internal face centres off the x axis by
    /// (0, dy, 0) to make the faces skewed.
    fn uneven_line_mesh(dy: f64) -> Arc<FvMesh> {
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(3)
                .n_internal_faces(2)
                .owner(vec![0, 1, 0, 2])
                .neighbour(vec![1, 2])
                .patches(vec![
                    BoundaryPatch::new("left", 2, 1, PatchKind::Patch),
                    BoundaryPatch::new("right", 3, 1, PatchKind::Patch),
                ])
                .cell_volumes(vec![1.0, 2.0, 1.0])
                .cell_centres(vec![vx(0.5), vx(2.0), vx(3.5)])
                .face_area_vectors(vec![vx(1.0), vx(1.0), vx(-1.0), vx(1.0)])
                .face_centres(vec![
                    Vector3::new(1.0, dy, 0.0),
                    Vector3::new(3.0, dy, 0.0),
                    vx(0.0),
                    vx(4.0),
                ])
                .build()
                .unwrap(),
        )
    }

    /// A vector field with the given cell values and fixed values on the two
    /// patches of a line mesh.
    fn field_with_ends(mesh: &Arc<FvMesh>, cells: Vec<Vector3>, left: Vector3, right: Vector3) -> VolVectorField {
        VolVectorField::new(
            "U",
            mesh.clone(),
            Field::new(cells),
            vec![PatchField::fixed_value_vec(1, left), PatchField::fixed_value_vec(1, right)],
        )
    }

    /// V&V (verification) — `Gauss linearUpwind grad(U)` (gh:#1202) gives the
    /// hand-computed corrections and is exact for a linear field.
    ///
    /// **Methodology.** Reference (analytical, by hand): on the uneven line
    /// mesh (widths 1, 2, 1 m), `U_x = 1 + 2x` gives cells 2, 5, 8 m/s and
    /// end values 1, 9. The Gauss linear gradient is exactly 2 in every cell
    /// (cell 1: (−3 + 7)/2), so the linearUpwind face values must be the exact
    /// 3 and 7 m/s at x = 1, 3. With φ = +1 m³/s (upwind = owner) the
    /// corrections are c = 3 − 2 = 1 and 7 − 5 = 2, so `source_LU −
    /// source_upwind` = [−1, 1 − 2, +2] = [−1, −1, +2] m⁴/s²; with φ = −1
    /// (upwind = neighbour) c = (1 − 2)·2 = −2 and (3 − 3.5)·2 = −1, again
    /// face values 3 and 7. A second check, for skewed faces: face centres
    /// moved off the axis by 0.3 m and `U = A·x` for a full tensor A, with the
    /// exact gradient Aᵀ supplied, must give `U_u + c_f = A·C_f` on every face
    /// (the correction's geometry, independent of the gradient). Pass: |Δ| <
    /// 1e-12, implicit coefficients unchanged.
    ///
    /// **Results (2026-11-04, release).** As expected to |Δ| ≤ 1e-15: the
    /// corrections are [1, 2] and [−2, −1], the source difference is
    /// [−1, −1, 2], the matrix coefficients are unchanged, and the skewed
    /// check reproduces A·C_f.
    #[test]
    fn linear_upwind_matches_hand_values_and_is_exact_for_linear_fields() {
        let mesh = uneven_line_mesh(0.0);
        let u = field_with_ends(&mesh, vec![vx(2.0), vx(5.0), vx(8.0)], vx(1.0), vx(9.0));
        let g = fvc::grad_vec(&u);
        for c in 0..3 {
            assert!((g.internal[c].xx - 2.0).abs() < 1e-14, "grad[{c}] = {:?}", g.internal[c]);
        }
        let mut phi = SurfaceScalarField::zeros("phi", mesh.clone());
        phi.internal[0] = 1.0;
        phi.internal[1] = 1.0;
        let c = linear_upwind_correction(&phi, &g);
        assert!((c[0].x - 1.0).abs() < 1e-14 && (c[1].x - 2.0).abs() < 1e-14, "{c:?}");
        let lu = DivScheme::GaussLinearUpwind("grad(U)".into());
        let gl = GradScheme::GaussLinear;
        let up = div_vec_scheme(&DivScheme::GaussUpwind, &gl, &phi, &u, mesh.clone()).unwrap();
        let m = div_vec_scheme(&lu, &gl, &phi, &u, mesh.clone()).unwrap();
        for (cell, want) in [-1.0, -1.0, 2.0].iter().enumerate() {
            assert!((m.source[cell].x - up.source[cell].x - want).abs() < 1e-14, "cell {cell}");
            assert_eq!(m.ldu.diag[cell], up.ldu.diag[cell]);
        }
        for f in 0..2 {
            assert_eq!(m.ldu.upper[f], up.ldu.upper[f]);
            assert_eq!(m.ldu.lower[f], up.ldu.lower[f]);
        }
        phi.internal[0] = -1.0;
        phi.internal[1] = -1.0;
        let c = linear_upwind_correction(&phi, &g);
        assert!((c[0].x + 2.0).abs() < 1e-14 && (c[1].x + 1.0).abs() < 1e-14, "{c:?}");

        // Skewed faces, full tensor A, exact gradient Aᵀ supplied.
        let mesh = uneven_line_mesh(0.3);
        let a = Tensor::new(1.0, -2.0, 0.5, 0.3, 4.0, -1.0, 2.0, 0.0, 3.0);
        let cells: Vec<Vector3> = mesh.cell_centres.iter().map(|x| a.mat_vec(*x)).collect();
        let mut g = fvc::grad_vec(&field_with_ends(&mesh, cells.clone(), Vector3::ZERO, Vector3::ZERO));
        for c in 0..3 {
            g.internal[c] = a.transpose();
        }
        for sign in [1.0, -1.0] {
            let mut phi = SurfaceScalarField::zeros("phi", mesh.clone());
            phi.internal[0] = sign;
            phi.internal[1] = sign;
            let c = linear_upwind_correction(&phi, &g);
            for f in 0..2 {
                let up = if sign > 0.0 { mesh.owner[f] } else { mesh.neighbour[f] };
                let exact = a.mat_vec(mesh.face_centres[f]);
                assert!((cells[up] + c[f] - exact).mag() < 1e-13, "face {f} sign {sign}");
            }
        }
    }

    /// V&V (verification) — `cellLimited Gauss linear k` (gh:#1202) limits
    /// each component by the hand-computed minmod factor and leaves a linear
    /// component alone.
    ///
    /// **Methodology.** Reference (by hand, upstream `cellLimitedGrad.C`): on
    /// the uniform 3-cell line (centres 0.5, 1.5, 2.5 m), `U_x` = 0, 1, 1.2
    /// with end values −1 and 1.4, `U_y` = x (cells 0.5, 1.5, 2.5, ends 0
    /// and 3). Gauss gradients of `U_x`: 1.5, 0.6, 0.3. Cell 1: neighbours
    /// span [0, 1.2], so max − U = 0.2, min − U = −1; the face at x = 2
    /// extrapolates +0.3, r = 0.2/0.3, limiter 2/3, gradient 0.4; the face at
    /// x = 1 extrapolates −0.3, r = 3.3 (no limit). Cells 0 and 2: r = 4/3 on
    /// both faces, not limited. `U_y` is linear: every r ≥ 1, unlimited (1).
    /// With k = 0.5 cell 1's bounds widen by (1/k − 1)(0.2 + 1) = 1.2, r =
    /// 1.4/0.3, not limited. With k = 0 nothing is limited. Pass |Δ| < 1e-14.
    ///
    /// **Results (2026-11-04, release).** As predicted: ∂U_x/∂x = [1.5, 0.4,
    /// 0.3] at k = 1, [1.5, 0.6, 0.3] at k = 0.5 and 0, ∂U_y/∂x = 1 in every
    /// cell throughout.
    #[test]
    fn cell_limited_gradient_matches_hand_values() {
        let mesh = line_mesh();
        let v = |x: f64, y: f64| Vector3::new(x, y, 0.0);
        let u = field_with_ends(&mesh, vec![v(0.0, 0.5), v(1.0, 1.5), v(1.2, 2.5)], v(-1.0, 0.0), v(1.4, 3.0));
        let raw = fvc::grad_vec(&u);
        let gx = |g: &VolTensorField| [g.internal[0].xx, g.internal[1].xx, g.internal[2].xx];
        let close = |a: [f64; 3], b: [f64; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-14);
        assert!(close(gx(&raw), [1.5, 0.6, 0.3]), "{:?}", gx(&raw));
        let k1 = grad_vec_scheme(&GradScheme::CellLimitedGaussLinear(1.0), &u).unwrap();
        assert!(close(gx(&k1), [1.5, 0.4, 0.3]), "{:?}", gx(&k1));
        let k05 = cell_limited_grad_vec(&u, raw.clone(), 0.5);
        assert!(close(gx(&k05), [1.5, 0.6, 0.3]), "{:?}", gx(&k05));
        let k0 = cell_limited_grad_vec(&u, raw.clone(), 0.0);
        assert!(close(gx(&k0), [1.5, 0.6, 0.3]));
        for g in [&k1, &k05, &k0] {
            for c in 0..3 {
                assert!((g.internal[c].xy - 1.0).abs() < 1e-14, "dUy/dx cell {c}");
            }
        }
        assert_eq!(grad_vec_scheme(&GradScheme::GaussLinear, &u).unwrap().internal.as_slice(), raw.internal.as_slice());
        assert!(grad_vec_scheme(&GradScheme::LeastSquares, &u).is_err());
    }
}
