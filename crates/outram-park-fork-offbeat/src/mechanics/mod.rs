// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream `offbeatLib/physicsSubSolvers/mechanicsSubSolver/`
// (`smallStrain.C`, `mechanicsSubSolver.C`).
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Solid mechanics — the displacement solve at the heart of fuel performance.
//!
//! # What this module computes
//!
//! Given a temperature field and the material's accumulated irradiation state,
//! it solves for the **displacement field** `D` of the fuel and cladding, and
//! from it the **stress** and **strain**. That is what tells you whether the
//! fuel/cladding gap has closed, how hard the pellet is pushing on the cladding,
//! and whether the cladding is close to failing.
//!
//! # Why a fuel rod is not just a thermo-elastic body
//!
//! Ordinary thermo-elasticity has one source of stress-free deformation: thermal
//! expansion. Irradiated fuel has four, and they are the whole subject:
//!
//! | Source | Sign | Rough end-of-life magnitude |
//! |---|---|---|
//! | Thermal expansion | + | ~1% linear |
//! | Fission-product swelling | + | ~0.1% linear per 10 MWd/kgHM |
//! | Densification | − | ~0.5% linear, saturating early |
//! | Crack relocation | + | comparable to the as-built gap width |
//!
//! All four are **eigenstrains** — deformation the material would undergo freely
//! if nothing constrained it, generating no stress on its own. Stress appears
//! only where geometry, or a neighbouring body, prevents that free deformation.
//! [`Eigenstrain`] carries all four together, because they enter the momentum
//! balance identically and separating them buys nothing.
//!
//! That unification is why this module is small. Swelling is not a special case
//! bolted onto a thermo-elastic solver; it is one more term in `ε*`.
//!
//! # The equation actually solved
//!
//! Quasi-static equilibrium — no inertia, because a fuel rod evolves over months
//! while elastic waves cross it in microseconds:
//!
//! `∇·σ = 0`
//!
//! with the small-strain isotropic constitutive law, `ε*` the isotropic
//! (linear) eigenstrain and `I` the identity tensor:
//!
//! `σ = 2μ(ε − ε* I) + λ(tr(ε) − 3ε*) I`,   `ε = ½(∇D + ∇Dᵀ)`
//!
//! Substituting and splitting off an implicit Laplacian gives the segregated
//! form upstream's `smallStrain.C` assembles, and which
//! [`MechanicsSolver::solve_quasi_static`] assembles here:
//!
//! `∇·[(2μ+λ)∇D] + ∇·[σ_e − (2μ+λ)∇D] − ∇(3K ε*) = 0`
//!
//! The first term is implicit (a vector Laplacian on the LDU matrix), the second
//! is the explicit stress correction that makes the split exact at convergence,
//! and the third is the eigenstrain load. Because the second term depends on the
//! solution, the system is iterated — the outer corrector loop.
//!
//! Splitting this way, rather than assembling the full anisotropic operator, is
//! what lets a segregated finite-volume code reuse the ordinary Laplacian
//! machinery per component. The price is that outer iteration.
//!
//! # Scope of this port
//!
//! **Implemented:** small-strain isotropic linear elasticity with arbitrary
//! isotropic eigenstrain, quasi-static and transient (inertial) forms, on a
//! single mesh region.
//!
//! **Not implemented here:** plasticity and creep (they belong in
//! [`crate::rheology`], which returns a corrected stress this solver consumes),
//! contact and gap closure ([`crate::gap`]), large-strain updated/total
//! Lagrangian kinematics, and multi-material interface correction. Each is
//! tracked separately under bead `op-6sl`.

mod solver;

pub use solver::{Eigenstrain, LinearElastic, MechanicsReport, MechanicsSolver};
