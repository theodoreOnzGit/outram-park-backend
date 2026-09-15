// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! One-dimensional two-phase system-code solvers, reduced from the OUTRAM-FOAM
//! 3-D multiphase reference.
//!
//! # What is in here
//!
//! Two 1-D transient solvers for compressible steam/water pipe flow, one step
//! above the [`crate::hem`] homogeneous-equilibrium baseline:
//!
//! | Solver | Equations | Slip between phases | Thermal non-equilibrium |
//! |---|---|---|---|
//! | [`crate::hem`] (HEM) | 3 | none (`u_g = u_l`) | none (both saturated) |
//! | [`drift_flux::DriftFlux1d`] | 4 | **algebraic** (`u_g − j = U_dm`) | none |
//! | [`two_fluid::TwoFluid1d`] | 6 | **dynamic** (own momentum equation) | **yes** (own energy equation) |
//!
//! That is the standard system-code ladder — each rung relaxes one assumption
//! of the rung below and costs one more transported field per phase.
//!
//! # The trace-back constraint, and exactly how it is honoured
//!
//! Beads `op-dt3.12` and `op-dt3.13` impose a hard constraint: *1-D models must
//! trace back to the OUTRAM-FOAM 3-D reference in
//! [`outram_foam_multiphase`], never be invented independently.* Being precise
//! about what that does and does not mean here matters, because the two codes
//! do not solve the same equations.
//!
//! **What is reused verbatim** — the *algebraic closures*, which are
//! dimensionality-agnostic:
//!
//! - [`outram_foam_multiphase::drift_flux::SlipModel`] supplies the drift
//!   velocity `U_dm` for [`drift_flux::DriftFlux1d`]. Its `ZuberFindlay`
//!   variant is the classic `v_d = C₀ j + V_gj`.
//! - [`outram_foam_multiphase::two_fluid::DragModel`] supplies the volumetric
//!   drag coefficient `K_d` for [`two_fluid::TwoFluid1d`], including the
//!   Schiller-Naumann and Wen-Yu correlations ported from OpenFOAM.
//!
//! These are consumed through the reference crate's own public API, so an
//! upstream correction to a correlation propagates here automatically and a
//! divergence cannot silently open up.
//!
//! **What is NOT reused, and why.** The 3-D reference is
//! **incompressible with constant per-phase density** — its
//! `DriftFluxMixture` stores `ρ_d` and `ρ_c` as scalars, and its mixture
//! density is the linear rule `ρ_m = α ρ_d + (1−α) ρ_c`. A pipe blowdown is
//! the opposite regime: the pressure falls by two orders of magnitude, water
//! flashes to steam, and both phase densities move by orders of magnitude
//! along the transient. So the *field equations* here are compressible and the
//! densities come from an IAPWS-IF97 flash, not from the reference's linear
//! rule. Reducing an incompressible formulation to 1-D and then bolting
//! compressibility onto it would be a worse kind of "independent invention"
//! than being explicit about the difference.
//!
//! The honest summary: **closures traced, field equations re-derived for
//! compressible flow, and the difference stated rather than papered over.**
//!
//! # Numerical method, and why it is not explicit
//!
//! Both solvers use a **semi-implicit pressure-based march**, the method
//! RELAP5 and TRACE use and for the same reason: an explicit compressible
//! march is limited by the acoustic CFL `Δt < Δx/(|u| + c)`, and in subcooled
//! water `c ≈ 1500 m/s`, so a 17 cm cell would need `Δt ≈ 10⁻⁴ s` purely to
//! chase sound waves that carry almost no energy. Each step:
//!
//! 1. Transport the conserved quantities explicitly with **donor-cell**
//!    (first-order upwind) fluxes at the old-time velocities.
//! 2. Linearise the mass residual in pressure through the compressibility
//!    `ψ = ∂ρ/∂p|_h` and solve the resulting **tridiagonal** pressure equation
//!    with [`thomas_solve`].
//! 3. Correct the velocities with the new pressure gradient.
//! 4. Recover the thermodynamic state from the new pressure and the
//!    transported energy.
//!
//! First-order upwind is a deliberate choice, not an oversight: it is
//! monotone, which matters far more than formal order at a flashing front,
//! and it is what the reference system codes use. It is also **diffusive**,
//! and that shows up as a smeared rarefaction wave — see the measured results
//! in the Edwards cases.
//!
//! # Honest scope — what these do NOT do
//!
//! Stated here so it is not discovered late:
//!
//! - **No wall heat transfer.** Both solvers are adiabatic. A blowdown is
//!   dominated by depressurisation, not by wall heat, but a rewetting or
//!   reflood problem is not solvable with these as they stand.
//! - **No interfacial area transport.** [`two_fluid::TwoFluid1d`] takes a
//!   prescribed bubble diameter, so its drag and its interfacial heat transfer
//!   do not respond to a changing flow regime. That absence is not cosmetic: it
//!   is what makes the six-equation solver **refuse** a transient started at a
//!   low void fraction, because a phase with almost no interfacial area cannot
//!   shed its reversible expansion work fast enough to stay inside the bounded
//!   metastable branches. Measured boundary and diagnosis in
//!   `two_fluid_tests::six_equation_march_refuses_where_a_phase_cannot_shed_its_expansion_work`.
//! - **No flow-regime map.** A real system code selects closures from a
//!   regime map (bubbly / slug / annular / mist). Here the closure is whatever
//!   the caller passed, everywhere, for the whole transient.
//! - **No counter-current flow limitation, no critical-flow model of their
//!   own.** The break boundary reuses the crate's existing HEM critical-flow
//!   dispatcher, so the *break* is HEM even when the *pipe* is drift-flux or
//!   two-fluid. This is a real modelling inconsistency and it is called out
//!   again at the boundary condition itself.
//! - **One validation case, and only one.** As of 2026-08-11
//!   [`drift_flux::DriftFlux1d`] is compared against the digitised
//!   Edwards–O'Brien experimental GS-1 pressure curve by
//!   `edwards_drift_flux_gs1_pressure_history` (see `edwards_tests.rs` for the
//!   full V&V record: RMSE 29.0 psia over 0–0.30 s, plateau 354.3 psia inside
//!   the experimental band, and an explicit list of what is *not* gated).
//!   Every other test in this module is **verification** — closed-form
//!   identities and invariants, compared against no experiment — and
//!   [`two_fluid::TwoFluid1d`], implemented 2026-08-12, is compared against no
//!   experiment at all. Its `two_fluid_tests.rs` battery is invariants,
//!   degenerate limits and one **documented disagreement** with
//!   [`drift_flux::DriftFlux1d`] in the single-phase limit; read that file's
//!   header for what it does and does not establish.
//!
//! # Units
//!
//! Public constructors and accessors are `uom`-typed. The inner marching loops
//! carry raw `f64` in strict SI — pascal, kelvin, `J/kg`, `kg/m³`, `m/s` —
//! because they are per-cell per-timestep and `uom` round-trips inside them
//! cost more than they buy. Every raw-`f64` boundary says so.

pub mod drift_flux;
pub mod geometry;
pub mod interfacial;
pub mod properties;
pub mod two_fluid;

pub use drift_flux::{DriftFlux1d, DriftFluxReport};
pub use geometry::Pipe1d;
pub use interfacial::{DispersedPhase, InterfacialCellState, InterfacialExchange, InterfacialSources};
pub use properties::{SaturatedProperties, TwoPhaseState};
pub use two_fluid::{
    PhaseCouplingBlock, PhaseCouplingInputs, TwoFluid1d, TwoFluidBoundary, TwoFluidReport,
};

/// Solve a tridiagonal linear system `A x = b` by the Thomas algorithm.
///
/// # What this is for
///
/// The pressure equation of a 1-D finite-volume solve couples each cell only
/// to its two axial neighbours, so its matrix is tridiagonal and the Thomas
/// algorithm — Gaussian elimination specialised to that structure — solves it
/// **exactly** in `O(n)` with no iteration and no convergence tolerance. On a
/// 1-D mesh this is strictly better than the general Krylov solvers in
/// [`outram_foam_basic_lib::ldu_matrix`]: same answer, one pass, no residual
/// to check.
///
/// # Arguments
///
/// All slices have length `n` and carry raw `f64` in whatever consistent units
/// the caller's equation uses (for the pressure equation: `a`, `b`, `c` in
/// `m·s`, `d` in `kg/s`, giving `x` in `Pa`).
///
/// - `a` — sub-diagonal, `a[0]` is unused and may be any value.
/// - `b` — main diagonal.
/// - `c` — super-diagonal, `c[n-1]` is unused.
/// - `d` — right-hand side.
///
/// # Returns
///
/// The solution `x`, length `n`.
///
/// # Errors
///
/// [`crate::TampinesError::Numerical`] if the system is not diagonally
/// solvable — a zero (to within `1e-300`) pivot arises. For a pressure
/// equation assembled from positive compressibilities and positive face
/// coefficients this cannot happen, so a failure here means the assembly is
/// wrong, not that the solver is fussy.
///
/// # Example
///
/// ```
/// use tampines::multiphase_1d::thomas_solve;
///
/// // The 1-D Laplacian with Dirichlet ends: -x[i-1] + 2 x[i] - x[i+1] = 1.
/// let n = 5;
/// let a = vec![-1.0; n];
/// let b = vec![2.0; n];
/// let c = vec![-1.0; n];
/// let d = vec![1.0; n];
/// let x = thomas_solve(&a, &b, &c, &d).unwrap();
///
/// // The exact solution is the discrete parabola x[i] = (i+1)(n-i)/2.
/// for (i, xi) in x.iter().enumerate() {
///     let exact = (i as f64 + 1.0) * (n as f64 - i as f64) / 2.0;
///     assert!((xi - exact).abs() < 1e-12, "cell {i}: {xi} vs {exact}");
/// }
/// ```
pub fn thomas_solve(
    a: &[f64],
    b: &[f64],
    c: &[f64],
    d: &[f64],
) -> Result<Vec<f64>, crate::TampinesError> {
    let n = b.len();
    if a.len() != n || c.len() != n || d.len() != n {
        return Err(crate::TampinesError::Numerical(format!(
            "thomas_solve: diagonal lengths disagree (a={}, b={n}, c={}, d={})",
            a.len(),
            c.len(),
            d.len()
        )));
    }
    if n == 0 {
        return Ok(Vec::new());
    }

    let mut c_star = vec![0.0_f64; n];
    let mut d_star = vec![0.0_f64; n];

    if b[0].abs() < 1.0e-300 {
        return Err(crate::TampinesError::Numerical(
            "thomas_solve: zero pivot at cell 0 -- the assembled matrix is singular".to_string(),
        ));
    }
    c_star[0] = c[0] / b[0];
    d_star[0] = d[0] / b[0];

    for i in 1..n {
        let denominator = b[i] - a[i] * c_star[i - 1];
        if denominator.abs() < 1.0e-300 {
            return Err(crate::TampinesError::Numerical(format!(
                "thomas_solve: zero pivot at cell {i} -- the assembled matrix is singular"
            )));
        }
        c_star[i] = c[i] / denominator;
        d_star[i] = (d[i] - a[i] * d_star[i - 1]) / denominator;
    }

    let mut x = vec![0.0_f64; n];
    x[n - 1] = d_star[n - 1];
    for i in (0..n - 1).rev() {
        x[i] = d_star[i] - c_star[i] * x[i + 1];
    }
    Ok(x)
}

#[cfg(test)]
mod edwards_tests;

#[cfg(test)]
mod marviken_tests;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod two_fluid_tests;
