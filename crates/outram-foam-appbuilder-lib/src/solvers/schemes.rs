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
//! | div | `GaussUpwind`, `GaussLinear` | `GaussLinearUpwind`, `GaussVanLeer`, `GaussMUSCL`, `GaussLimitedLinear` |
//!
//! The limited/TVD `div` schemes need a face limiter driven by a reconstructed
//! upwind gradient; `outram-foam-basic-lib` provides the limiter functions
//! (`limiters::FluxLimiter`) but not the face-`r` reconstruction for a vector
//! field, so they are declared unsupported rather than approximated.

use crate::error::AppBuilderError;
use crate::io::fv_schemes::{DdtScheme, DivScheme};
use outram_foam_basic_lib::prelude::*;
use std::sync::Arc;

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
///   **Known limitation — `Backward` is wired, not verified as second order.**
///   [`crate::solvers::pimple_foam::PimpleFoam`]'s PISO loop adds a Rhie–Chow
///   flux correction `rAU_f · fvc::ddt_corr(U_old, φ_old, Δt)`, and
///   `outram-foam-basic-lib`'s `ddt_corr` implements only the **Euler** form.
///   Selecting `Backward` changes `rAU = V/A` (the diagonal becomes `1.5 V/Δt`
///   instead of `V/Δt`) without changing `ddt_corr`, so the two are
///   inconsistent by a ratio tending to 1.5 as Δt → 0. Measured consequence
///   (`tests/fv_scheme_selection.rs`, 2026-08-07): the Euler and Backward
///   lid-driven-cavity runs converge to steady states differing by 1.0e-2 to
///   2.9e-2 m/s (1–3 % of U_lid), and that difference *grows* as Δt is
///   refined — the signature of an inconsistency, not of truncation error.
///   Fixing it needs OpenFOAM's `backwardDdtScheme::fvcDdtPhiCorr`, which
///   belongs in `outram-foam-basic-lib`. Until then, treat `Backward` as
///   available and benchmark-neutral (it scores marginally better than Euler
///   against Ghia 1982 at every Δt tested), but not as a verified second-order
///   time integration.
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
/// * `phi`    — face flux φ = U·S_f [m³/s] (volumetric) or ρU·S_f [kg/s]
///   (mass); the scheme is agnostic, the units follow `phi`.
/// * `u`      — the transported velocity field [m/s].
/// * `mesh`   — the mesh.
///
/// # Schemes
///
/// * [`DivScheme::GaussUpwind`] — first-order upwind, `fvm::div_vec` unmodified.
///   Unconditionally bounded, strongly diffusive on a coarse mesh.
/// * [`DivScheme::GaussLinear`] — second-order central differencing, assembled
///   by **deferred correction**: the implicit matrix stays the (diagonally
///   dominant) upwind operator, and the difference between the linear and upwind
///   face values is added explicitly to the source,
///
///   ```text
///     D_f = φ_f (U_f^linear − U_f^upwind)
///     source[owner]     −= D_f
///     source[neighbour] += D_f
///   ```
///
///   This is the standard Khosla–Rubin treatment: it recovers second-order
///   accuracy at convergence while keeping the matrix M-like. It is **not**
///   bounded — central differencing on a convection-dominated coarse mesh can
///   produce over/undershoots.
///
///   Boundary faces are left to `fvm::div_vec`: on a zero-gradient patch the
///   linear and upwind face values coincide (both equal the owner value), and on
///   a fixed-value patch `fvm::div_vec` already uses the prescribed value, so
///   there is no correction to apply either way.
///
/// # Errors
///
/// [`AppBuilderError::UnsupportedScheme`] for the limited/TVD schemes
/// (`linearUpwind`, `vanLeer`, `MUSCL`, `limitedLinear`) — see the module
/// documentation.
pub fn div_vec_scheme(
    scheme: &DivScheme,
    phi: &SurfaceScalarField,
    u: &VolVectorField,
    mesh: Arc<FvMesh>,
) -> Result<FvVectorMatrix, AppBuilderError> {
    let unsupported = |name: &str| AppBuilderError::UnsupportedScheme {
        family: "divSchemes",
        scheme: name.to_string(),
        reason: "limited/TVD convection needs a face limiter driven by a reconstructed \
                 upwind gradient, which is not yet implemented for vector fields",
    };

    match scheme {
        DivScheme::GaussUpwind => Ok(fvm::div_vec(phi, u, mesh)),
        DivScheme::GaussLinear => {
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
        DivScheme::GaussLinearUpwind(g) => Err(unsupported(&format!("Gauss linearUpwind {g}"))),
        DivScheme::GaussVanLeer => Err(unsupported("Gauss vanLeer")),
        DivScheme::GaussMUSCL => Err(unsupported("Gauss MUSCL")),
        DivScheme::GaussLimitedLinear(k) => Err(unsupported(&format!("Gauss limitedLinear {k}"))),
    }
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

    /// V&V (verification) — `DivScheme::GaussLinear` reproduces the exact
    /// central-difference convective flux on a uniform 1-D mesh.
    ///
    /// **Methodology.** Reference (analytical): for a uniform mesh the linear
    /// face value is the arithmetic mean `(U_O + U_N)/2`, so the deferred
    /// correction on face f is `D_f = φ_f (U_O + U_N)/2 − φ_f U_upwind`. With
    /// φ_f = +1 m³/s (flow owner→neighbour, upwind = owner) that is
    /// `φ_f (U_N − U_O)/2`. Inputs: the 3-cell unit line mesh, φ = 1 m³/s on
    /// both internal faces, U_x = {1, 2, 3} m/s. Expected corrections: face 0
    /// (cells 0→1) D = 1·(2−1)/2 = 0.5; face 1 (cells 1→2) D = 1·(3−2)/2 = 0.5.
    /// Hence `source_linear − source_upwind` = [−0.5, 0.0, +0.5] m⁴/s².
    /// Pass criterion: |Δ| < 1e-12; and the implicit matrix (diag, off-diagonals)
    /// must be unchanged, since deferred correction touches only the source.
    ///
    /// **Results (run 2026-08-07, release mode).** measured
    /// `source_linear − source_upwind` = [−0.5, 0.0, +0.5] m⁴/s² exactly
    /// (max |Δ| = 0.0); diag and upper/lower coefficients identical between the
    /// two schemes to 0.0. Confirms the scheme selection changes the
    /// discretisation in precisely the intended way and nowhere else.
    #[test]
    fn linear_div_adds_the_exact_central_difference_correction() {
        let mesh = line_mesh();
        let mut u = VolVectorField::zero("U", mesh.clone());
        u.internal = Field::new(vec![vx(1.0), vx(2.0), vx(3.0)]);
        let mut phi = SurfaceScalarField::zeros("phi", mesh.clone());
        phi.internal[0] = 1.0;
        phi.internal[1] = 1.0;

        let up = div_vec_scheme(&DivScheme::GaussUpwind, &phi, &u, mesh.clone()).unwrap();
        let lin = div_vec_scheme(&DivScheme::GaussLinear, &phi, &u, mesh.clone()).unwrap();

        let expected = [-0.5, 0.0, 0.5];
        for (c, want) in expected.iter().enumerate() {
            let d = lin.source[c].x - up.source[c].x;
            assert!(
                (d - want).abs() < 1e-12,
                "cell {c}: correction {d} expected {want}"
            );
            assert!(
                (lin.ldu.diag[c] - up.ldu.diag[c]).abs() < 1e-15,
                "diag[{c}]"
            );
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
            DivScheme::GaussLinearUpwind("grad(U)".to_string()),
        ] {
            assert!(
                div_vec_scheme(&s, &phi, &u, mesh.clone()).is_err(),
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
}
