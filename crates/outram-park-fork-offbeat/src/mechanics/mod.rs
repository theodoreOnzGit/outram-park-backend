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
//! # Inelastic deformation: creep and plasticity
//!
//! [`crate::rheology`] owns the constitutive laws; this module drives them.
//! Attach one with [`MechanicsSolver::set_rheology`] and step the solve with
//! [`MechanicsSolver::solve_creep_step`] instead of
//! [`MechanicsSolver::solve_quasi_static`]. The coupling adds two things to the
//! equation above:
//!
//! - the strain handed to the constitutive law is the **mechanical** strain
//!   `ε − ε* I`, with the eigenstrain already removed, so an unconstrained
//!   freely expanding body is correctly stress-free and does not creep;
//! - the accumulated inelastic strain `ε_in = ε_p + ε_c` re-enters the momentum
//!   balance as an additional (tensor) eigenstrain through the extra explicit
//!   term `−∇·[2μ ε_in + λ tr(ε_in) I]`, which restores equilibrium after the
//!   corrected stress comes back softer than the elastic one. This is the
//!   finite-volume analogue of upstream's `correctAdditionalStrain`, and it
//!   rides on exactly the same explicit-remainder hook as the segregated split.
//!
//! The per-cell [`RheologyState`](crate::rheology::RheologyState) is advanced
//! **once** per completed step, after the corrector loop, never inside it.
//!
//! # Scope of this port
//!
//! **Implemented:** small-strain isotropic linear elasticity with arbitrary
//! isotropic eigenstrain, quasi-static and transient (inertial) forms, and the
//! inelastic coupling described above (creep and plasticity through
//! [`crate::rheology`], with [`CreepTimeStepControl`](crate::rheology::CreepTimeStepControl)
//! bounding the step), on a single mesh region.
//!
//! **Not implemented here:** contact and gap closure ([`crate::gap`]),
//! large-strain updated/total Lagrangian kinematics, traction boundary
//! conditions, and multi-material interface correction — the last of which
//! matters for the stress *recovery* across a sharp material interface; see the
//! measured limitation recorded on
//! `solver::rheology_tests::spatially_varying_creep_keeps_the_axial_stress_uniform`.
//! Each is tracked separately under bead `op-6sl`.

mod solver;

pub use solver::{CreepStepReport, Eigenstrain, LinearElastic, MechanicsReport, MechanicsSolver};
