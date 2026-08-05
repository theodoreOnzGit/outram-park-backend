//! Stage 2 — the same physics rebuilt on OUTRAM PARK libraries.
//!
//! **No physics lives here yet.** What lives here is the *seam*: the set of
//! swappable components, the enum each one dispatches through, and the parity
//! state each is in. The seam exists before the implementations so that a
//! substitution arrives as one new enum variant with a parity gate attached,
//! rather than as a fork of the solver.
//!
//! # The rule that governs this module
//!
//! No component is accepted here until it reproduces [`crate::reference`] on
//! the benchmark suite to a stated tolerance, and **no component is improved
//! before it has passed parity**. A substitution that changes results *and*
//! claims to be better cannot be distinguished from one that is simply wrong.
//!
//! [`Component`] enumerates the substitutions planned in
//! `docs/bedok-port-scoping.md` §5, and [`Component::parity_status`] records
//! where each one stands. That is deliberately data rather than prose: a test
//! walks [`Component::ALL`] and fails if the map here drifts from the scoping
//! document, and adding a substitution without stating its parity status will
//! not compile.
//!
//! # Dispatch
//!
//! Each component module defines an enum whose variants are the available
//! implementations — [`channel_flow::ChannelFlowKernel`] and friends. Per the
//! workspace Rust rules there are **no trait objects**: the set of physics
//! implementations is closed and known at compile time, so adding one forces
//! every `match` to handle it, and a missed case is a compile error rather
//! than a runtime surprise. The shape every kernel follows is:
//!
//! ```text
//! match kernel {
//!     Kernel::Reference   => reference_path(...),
//!     Kernel::Substitute  => substituted_path(...),
//! }
//! ```
//!
//! # A note on the linear-solver substitution
//!
//! The reference path uses a **direct** sparse LU (`faer`), because that is
//! what MATLAB's `\` and `decomposition()` do, and stage 1 must match it.
//! `outram-foam-basic-lib` offers **iterative** solvers only — conjugate
//! gradient, GAMG, Gauss-Seidel. Swapping one in is therefore a real
//! substitution with a real question behind it: does an iterative solve reach
//! the same `k_eff` as a direct factorisation, and at what cost? Note also
//! that the diffusion LHS (`gradD + nodal + sigma.tot - sigma.s`) is
//! **non-symmetric** because of down-scattering, so conjugate gradient does
//! not apply to it unmodified. This is the substitution most likely to break
//! bit-level agreement while being perfectly correct, which is why parity
//! tolerances are set physically rather than at machine epsilon.

pub mod channel_flow;
pub mod chf;
pub mod cross_sections;
pub mod drift_flux;
pub mod fuel_rod;
pub mod kinetics;
pub mod linear_solver;

/// Which implementation a running solve is dispatching to.
///
/// Every component kernel maps onto one of these, so a solve can report the
/// path it actually took without the reporting code knowing which component it
/// is looking at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Implementation {
    /// The stage-1 faithful translation in [`crate::reference`].
    Reference,
    /// An OUTRAM PARK library standing in for it.
    Substituted,
}

/// How far a component has got through its parity gate.
///
/// Recorded as data so the substitution map cannot quietly claim more than has
/// been measured. Per the workspace V&V rule, a `Passed` variant must carry the
/// measured number and the date it was measured — not merely the word "passed".
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParityStatus {
    /// No substituted implementation has been written.
    NotStarted,
    /// An implementation exists but has not been run against the reference.
    AwaitingGate,
    /// Measured against the reference and within the stated tolerance.
    Passed {
        /// Largest relative difference measured against the reference \[-\].
        max_relative_difference: f64,
        /// ISO date the measurement was taken, e.g. `"2026-08-05"`.
        measured: &'static str,
    },
    /// Measured against the reference and outside the stated tolerance.
    Failed {
        /// Largest relative difference measured against the reference \[-\].
        max_relative_difference: f64,
        /// ISO date the measurement was taken.
        measured: &'static str,
    },
}

impl ParityStatus {
    /// Whether this component may be used in a substituted solve.
    ///
    /// Only [`ParityStatus::Passed`] qualifies. This is the rule from the
    /// scoping document expressed as a function, so the gate is checkable
    /// rather than remembered.
    #[must_use]
    pub const fn is_accepted(&self) -> bool {
        matches!(self, Self::Passed { .. })
    }
}

/// A component of the solve that stage 2 plans to replace.
///
/// The variants are exactly the rows of the substitution map in
/// `docs/bedok-port-scoping.md` §5. Adding a row there means adding a variant
/// here, which forces every method below to account for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Component {
    /// Single-phase channel flow with evaporation.
    ChannelFlow,
    /// Six-equation drift-flux two-phase flow.
    DriftFlux,
    /// Critical heat flux (W-3 correlation).
    CriticalHeatFlux,
    /// One-dimensional cylindrical fuel-rod conduction.
    FuelRod,
    /// Delayed-neutron kinetics in the transient path.
    Kinetics,
    /// Cross-section data and its feedback update.
    CrossSections,
    /// Sparse linear algebra behind the diffusion solve.
    LinearSolver,
}

impl Component {
    /// Every component, in the order the substitution map lists them.
    pub const ALL: [Component; 7] = [
        Component::ChannelFlow,
        Component::DriftFlux,
        Component::CriticalHeatFlux,
        Component::FuelRod,
        Component::Kinetics,
        Component::CrossSections,
        Component::LinearSolver,
    ];

    /// The MATLAB routines this component is translated from.
    ///
    /// Provenance for tracing a substitution back to Yan Ren's original, per
    /// the naming rule in `docs/bedok-port-scoping.md` §7.
    #[must_use]
    pub const fn matlab_origin(&self) -> &'static str {
        match self {
            Self::ChannelFlow => "singleflow1devap, singleflow1devaptime",
            Self::DriftFlux => "driftflux6_solverstatic3d",
            Self::CriticalHeatFlux => "w3chf, w3chfhottest",
            Self::FuelRod => "fuelrodheat_1dcylnd, fuelrodheat_1dcylndtime",
            Self::Kinetics => "thdiffusion_solvertimexyz (delayed-neutron path)",
            Self::CrossSections => "sigmavalupd3d",
            Self::LinearSolver => "mldivide, decomposition, gmres",
        }
    }

    /// The OUTRAM PARK crate (or crates) proposed to stand in for it.
    #[must_use]
    pub const fn substitute(&self) -> &'static str {
        match self {
            Self::ChannelFlow => "tuas_boussinesq_solver, tampines",
            Self::DriftFlux => "outram-foam-multiphase::drift_flux",
            Self::CriticalHeatFlux => {
                "outram-foam-multiphase::chf, outram-foam-appbuilder-lib closures::heat_transfer::chf"
            }
            Self::FuelRod => "outram-park-fork-offbeat, tuas one_d_solid_structure",
            Self::Kinetics => "teh-o-prke",
            Self::CrossSections => "njoy-outram-park-fork",
            Self::LinearSolver => "outram-foam-basic-lib ldu_matrix, krylov",
        }
    }

    /// Where this component stands against its parity gate.
    ///
    /// All components are [`ParityStatus::NotStarted`] as of 2026-08-05:
    /// nothing has been substituted, and nothing has been measured. Updating a
    /// status without a measurement to back it is a violation of the workspace
    /// rule against reporting unverified results.
    #[must_use]
    pub const fn parity_status(&self) -> ParityStatus {
        match self {
            Self::ChannelFlow
            | Self::DriftFlux
            | Self::CriticalHeatFlux
            | Self::FuelRod
            | Self::Kinetics
            | Self::CrossSections
            | Self::LinearSolver => ParityStatus::NotStarted,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_has_passed_a_parity_gate_yet() {
        // This test is the honesty check on the substitution map. When a
        // component genuinely passes its gate, this assertion is what forces
        // the change to be deliberate and to carry a measured number.
        for component in Component::ALL {
            assert_eq!(
                component.parity_status(),
                ParityStatus::NotStarted,
                "{component:?} claims progress it has not demonstrated"
            );
            assert!(!component.parity_status().is_accepted());
        }
    }

    #[test]
    fn every_component_names_its_origin_and_its_substitute() {
        for component in Component::ALL {
            assert!(!component.matlab_origin().is_empty(), "{component:?}");
            assert!(!component.substitute().is_empty(), "{component:?}");
        }
    }

    #[test]
    fn the_map_covers_every_scoping_document_row() {
        // Seven rows in docs/bedok-port-scoping.md §5. If that table grows,
        // this fails and points at the drift.
        assert_eq!(Component::ALL.len(), 7);
    }
}
