// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! The 1-D **six-equation two-fluid** solver — NOT YET IMPLEMENTED.
//!
//! # Status: documented scaffold, no physics
//!
//! This module exists so the API shape, the module map and the doc links are
//! in place, and so the six-equation model's requirements are written down
//! while they are fresh. **It computes nothing.** [`TwoFluid1d::step`] returns
//! [`TampinesError::NotYetImplemented`] rather than a plausible number.
//!
//! It is deliberately *not* a partially-working solver: a six-equation model
//! that marches but gets its interfacial closures wrong is far more dangerous
//! than one that refuses, because its output looks like an answer. Tracked as
//! bead `op-dt3.13`.
//!
//! # What the model is, and how it differs from drift flux
//!
//! Two-fluid drops *both* remaining equilibrium assumptions of
//! [`super::drift_flux`]. Each phase gets its own mass, momentum and energy
//! equation — six in total — so the phases may differ in both velocity *and*
//! temperature. That is what a blowdown actually needs: the liquid can stay
//! subcooled while vapour at the wall is superheated, and neither drift flux
//! nor HEM can represent it.
//!
//! For `k ∈ {g, l}`:
//!
//! `∂(α_k ρ_k)/∂t + ∂(α_k ρ_k u_k)/∂x = Γ_k`  (phase mass, `Γ_g = −Γ_l`)
//!
//! `∂(α_k ρ_k u_k)/∂t + ∂(α_k ρ_k u_k²)/∂x = −α_k ∂p/∂x + α_k ρ_k g_x + F_k^i + Γ_k u^i − F_k^wall`  (phase momentum)
//!
//! `∂(α_k ρ_k h_k)/∂t + ∂(α_k ρ_k u_k h_k)/∂x = α_k ∂p/∂t + Q_k^i + Γ_k h_k^i + q_k^wall`  (phase energy)
//!
//! with `α_g + α_l = 1` closing the set.
//!
//! # What still has to be decided before this can be written
//!
//! Recorded now so the work is scoped rather than discovered:
//!
//! 1. **The interfacial drag `F_k^i`** traces back to
//!    [`outram_foam_multiphase::two_fluid::DragModel`] — Schiller-Naumann and
//!    Wen-Yu are already ported there, and this module must consume them
//!    rather than reimplement, exactly as [`super::drift_flux`] consumes
//!    `SlipModel`. That part is settled.
//! 2. **The interfacial heat transfer `Q_k^i` is NOT settled.** The 3-D
//!    reference has no interfacial heat-transfer closure at all — it is
//!    isothermal — so there is nothing to trace back to, and inventing one
//!    here would violate the constraint in bead `op-dt3.13`. Either the
//!    reference gains one first (the honest order), or this solver ships with
//!    a documented placeholder and says so.
//! 3. **The drag coupling must be solved implicitly and pairwise.** At the
//!    interfacial drag magnitudes a flashing flow produces, an explicit drag
//!    term is stiff: the two phase momentum equations exchange momentum on a
//!    timescale far shorter than the transport step, so an explicit coupling
//!    oscillates and diverges. The standard treatment solves the two momentum
//!    equations as a 2×2 block per face.
//! 4. **The pressure equation gains a void-propagation term.** Unlike drift
//!    flux, the two-fluid system's characteristics can be complex — the
//!    six-equation model without interfacial pressure terms is famously
//!    ill-posed as an initial-value problem. Whatever regularisation is chosen
//!    (interfacial pressure, virtual mass, or artificial viscosity) is a
//!    modelling decision that must be stated, not buried.
//!
//! Point 4 in particular is why this is not a matter of typing out four more
//! equations: the naive six-equation system is ill-posed, and the choice that
//! makes it well-posed changes the answer.

use uom::si::f64::Time;

use super::geometry::Pipe1d;
use crate::TampinesError;

/// Per-step diagnostics from [`TwoFluid1d::step`].
///
/// Defined now so the API shape is fixed; no field is ever populated with a
/// computed value until the solver exists.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwoFluidReport {
    /// Simulated time at the end of the step \[s\].
    pub time: f64,
    /// Mass flow through the right-hand boundary \[kg/s\], positive outward.
    pub outlet_mass_flow: f64,
    /// Largest void fraction anywhere \[-\].
    pub max_void_fraction: f64,
    /// Largest phase-temperature difference `|T_g − T_l|` anywhere \[K\] — the
    /// quantity that justifies a six-equation model over drift flux.
    pub max_thermal_nonequilibrium: f64,
}

/// A 1-D transient six-equation two-fluid solver.
///
/// **Not implemented.** See the module documentation for what the model is and
/// what has to be decided before it can be written. Every method that would
/// compute something returns [`TampinesError::NotYetImplemented`].
pub struct TwoFluid1d {
    pipe: Pipe1d,
}

impl TwoFluid1d {
    /// Build a solver on `pipe`.
    ///
    /// Constructing one is allowed — it is inert — so that a caller can write
    /// the case now and have it fail loudly at [`step`](Self::step) rather than
    /// failing to compile later.
    #[must_use]
    pub fn new(pipe: Pipe1d) -> Self {
        Self { pipe }
    }

    /// The pipe geometry.
    #[must_use]
    pub fn pipe(&self) -> &Pipe1d {
        &self.pipe
    }

    /// Advance one timestep — **not implemented**.
    ///
    /// # Errors
    ///
    /// Always [`TampinesError::NotYetImplemented`]. This is deliberate: see
    /// the module docs. A six-equation model that marches with wrong
    /// interfacial closures produces output that looks like an answer, which
    /// is worse than no output.
    pub fn step(&mut self) -> Result<TwoFluidReport, TampinesError> {
        Err(TampinesError::NotYetImplemented {
            component: "multiphase_1d::two_fluid::TwoFluid1d::step",
        })
    }

    /// Elapsed simulated time — always zero, since nothing has advanced.
    #[must_use]
    pub fn time(&self) -> Time {
        Time::new::<uom::si::time::second>(0.0)
    }
}
