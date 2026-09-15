// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
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

//! # `genfoam::neutronics` — deterministic reactor neutronics
//!
//! Rust port of GeN-Foam's `src/classes/neutronics/`. GeN-Foam offers several
//! run-time-selectable neutronics models; upstream they share the abstract
//! `Foam::neutronics` base class. In this port the model set is a **closed enum**
//! (per the workspace no-`dyn`-dispatch rule) rather than a virtual base — adding
//! a model forces every `match` site to handle it.
//!
//! ## What belongs here
//!
//! - [`point_kinetics`] — 0-D point-kinetics (implemented).
//! - [`diffusion`] — multigroup neutron diffusion, k-eigenvalue + transient
//!   (implemented).
//! - [`sp3`] — simplified-P3 transport, eigenvalue + transient (implemented).
//! - [`sn`] — discrete-ordinates (S_N) transport, **eigenvalue only**
//!   (implemented; the transient `step` is deferred, unlike `diffusion` and
//!   `sp3`). Each model also exposes a lightweight state-only constructor
//!   (`::new`) that allocates flux state without cross sections and therefore
//!   cannot solve — build with `with_cross_sections` to obtain a working model.
//! - [`state`] — the shared spatial flux / power / precursor / power-density
//!   state that the spatial models read and write.
//! - [`xs`] — the multigroup cross-section (`XS`) data structures.
//!
//! Cross sections are read from GeN-Foam `nuclearData` dictionaries; this module
//! does **not** pull data from the NJOY / Monte Carlo crates.
//!
//! ## Model dispatch — [`NeutronicsModel`]
//!
//! GeN-Foam selects a neutronics model at run time through the abstract
//! `Foam::neutronics` base class. Here the closed set of models is a
//! [`NeutronicsModel`] enum (per the workspace no-`dyn` rule): adding a variant
//! forces every `match` to handle it. The shared, model-agnostic surface —
//! [`NeutronicsModel::power`], [`NeutronicsModel::k_eff`],
//! [`NeutronicsModel::kind`] — is what a coupling/driver layer calls without
//! knowing which model it holds.

pub mod diffusion;
pub mod point_kinetics;
pub mod sn;
pub mod sp3;
pub mod state;
pub mod xs;

use uom::si::f64::Power;

pub use diffusion::{DiffusionNeutronics, DiffusionSettings, EigenvalueReport};
pub use sn::SnNeutronics;
pub use sp3::Sp3Neutronics;
pub use state::{NeutronMultiplicationFactor, NeutronicsState};

use point_kinetics::{PointKineticsError, PointKineticsParameters, PointKineticsState, Reactivity};
use uom::si::f64::Time;

/// Which neutronics model a [`NeutronicsModel`] holds. Returned by
/// [`NeutronicsModel::kind`] and used to name an unimplemented model in
/// [`NeutronicsError::ModelNotImplemented`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NeutronicsModelKind {
    /// 0-D point kinetics.
    PointKinetics,
    /// Multigroup neutron diffusion.
    Diffusion,
    /// Simplified P3 (SP3) transport (eigenvalue + transient).
    Sp3,
    /// Discrete-ordinates (SN) transport (eigenvalue only).
    Sn,
}

/// Errors from constructing or advancing a neutronics model.
///
/// Not [`Clone`] — it wraps [`xs::XsError`], which owns un-`Clone` RBF-solve
/// diagnostics.
#[derive(Debug, thiserror::Error)]
pub enum NeutronicsError {
    /// A cross-section materialisation / evaluation failed.
    #[error("cross-section error: {0}")]
    Xs(#[from] xs::XsError),

    /// A point-kinetics sub-solve failed.
    #[error("point-kinetics error: {0}")]
    PointKinetics(#[from] PointKineticsError),

    /// The per-patch flux boundary list did not have one entry per mesh patch.
    #[error("flux boundary list has {given} entries but the mesh has {patches} patches")]
    PatchCountMismatch {
        /// Number of boundary conditions supplied.
        given: usize,
        /// Number of mesh patches.
        patches: usize,
    },

    /// The initial fission-neutron production was zero, so `k_eff` is undefined
    /// (a non-multiplying configuration).
    #[error("no fission source: initial fission production is zero, k_eff undefined")]
    NoFissionSource,

    /// The eigenvalue power iteration did not converge within the outer-iteration
    /// budget.
    #[error(
        "eigenvalue power iteration did not converge after {outer_iterations} \
         iterations (k residual {k_residual:e}, flux residual {flux_residual:e})"
    )]
    NotConverged {
        /// Outer iterations performed.
        outer_iterations: usize,
        /// Final relative change in `k_eff`.
        k_residual: f64,
        /// Final relative L2 change in the flux.
        flux_residual: f64,
    },

    /// The transient time step was not strictly positive.
    #[error("time step must be positive, got {0} s")]
    NonPositiveTimeStep(f64),

    /// The model was built with the state-only `::new` constructor, so it holds
    /// flux state but no cross sections and cannot solve. Rebuild it with
    /// `with_cross_sections` to obtain a working model.
    #[error("neutronics model {0:?} was built without cross sections (state-only); use with_cross_sections to solve")]
    ModelNotImplemented(NeutronicsModelKind),
}

/// A 0-D point-kinetics model wrapped as a [`NeutronicsModel`] variant.
///
/// Composes the already-ported [`PointKineticsParameters`] and
/// [`PointKineticsState`] into one owned unit so it can sit in the
/// [`NeutronicsModel`] enum alongside the spatial models. Unlike the spatial
/// models it has no [`NeutronicsState`] / mesh — point kinetics tracks only the
/// scalar amplitude.
#[derive(Debug, Clone, PartialEq)]
pub struct PointKineticsModel {
    params: PointKineticsParameters,
    state: PointKineticsState,
}

impl PointKineticsModel {
    /// Build a point-kinetics model from its kinetics parameters and an initial
    /// state.
    #[must_use]
    pub fn new(params: PointKineticsParameters, state: PointKineticsState) -> Self {
        Self { params, state }
    }

    /// The kinetics parameters.
    #[must_use]
    pub fn params(&self) -> &PointKineticsParameters {
        &self.params
    }

    /// The current 0-D state (fission power + precursor powers).
    #[must_use]
    pub fn state(&self) -> &PointKineticsState {
        &self.state
    }

    /// Advance one implicit time step under total reactivity `reactivity` and
    /// external source power `external_source_power`. Delegates to
    /// [`PointKineticsState::step`].
    ///
    /// # Errors
    ///
    /// [`NeutronicsError::PointKinetics`] on a singular system, mismatched group
    /// count, or non-positive step.
    pub fn step(
        &mut self,
        dt: Time,
        reactivity: Reactivity,
        external_source_power: Power,
    ) -> Result<(), NeutronicsError> {
        self.state
            .step(&self.params, dt, reactivity, external_source_power)?;
        Ok(())
    }
}

/// The closed set of deterministic neutronics models (enum dispatch, no
/// `dyn`).
///
/// A driver holds a `NeutronicsModel` and calls the shared surface
/// ([`Self::power`], [`Self::k_eff`], [`Self::kind`]) without a compile-time
/// dependency on which model it is. To *run* a model, match it out and call the
/// model-specific method (`DiffusionNeutronics::solve_eigenvalue` / `::step`,
/// `PointKineticsModel::step`) — those interfaces differ (0-D vs. mesh-based)
/// too much to share one signature honestly.
///
/// The variants differ in size (the mesh-based `Diffusion` model is much larger
/// than the 0-D `PointKinetics` one), which clippy's `large_enum_variant` would
/// flag — the standard fix is to `Box` the big variant, but `Box<T>` is banned
/// by the workspace design rules, so the lint is allowed here instead. A
/// `NeutronicsModel` is created once per reactor region and lives for the whole
/// run, so the size asymmetry costs nothing in practice.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum NeutronicsModel {
    /// 0-D point kinetics.
    PointKinetics(PointKineticsModel),
    /// Multigroup neutron diffusion.
    Diffusion(DiffusionNeutronics),
    /// Simplified P3 transport (eigenvalue + transient).
    Sp3(Sp3Neutronics),
    /// Discrete-ordinates transport (eigenvalue only).
    Sn(SnNeutronics),
}

impl NeutronicsModel {
    /// Which model this is.
    #[must_use]
    pub fn kind(&self) -> NeutronicsModelKind {
        match self {
            Self::PointKinetics(_) => NeutronicsModelKind::PointKinetics,
            Self::Diffusion(_) => NeutronicsModelKind::Diffusion,
            Self::Sp3(_) => NeutronicsModelKind::Sp3,
            Self::Sn(_) => NeutronicsModelKind::Sn,
        }
    }

    /// The current total fission power `P` (watts).
    ///
    /// Point kinetics returns its scalar fission power; the spatial models
    /// return the domain-integrated power of their neutronics state.
    #[must_use]
    pub fn power(&self) -> Power {
        match self {
            Self::PointKinetics(m) => m.state().fission_power(),
            Self::Diffusion(m) => m.state().total_power(),
            Self::Sp3(m) => m.state().total_power(),
            Self::Sn(m) => m.state().total_power(),
        }
    }

    /// The effective multiplication factor `k_eff`, if the model tracks one.
    ///
    /// Point kinetics is amplitude-only and does not compute a spatial `k_eff`,
    /// so it returns [`None`]; the spatial models return [`Some`] of their
    /// state's current `k_eff`.
    #[must_use]
    pub fn k_eff(&self) -> Option<NeutronMultiplicationFactor> {
        match self {
            Self::PointKinetics(_) => None,
            Self::Diffusion(m) => Some(m.state().k_eff()),
            Self::Sp3(m) => Some(m.state().k_eff()),
            Self::Sn(m) => Some(m.state().k_eff()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use outram_foam_basic_lib::interface::one_dimensional_meshing::create_one_d_mesh;
    use uom::si::area::square_meter;
    use uom::si::f64::{Area, Frequency, Length, Power, Ratio, Time};
    use uom::si::frequency::hertz;
    use uom::si::length::meter;
    use uom::si::power::watt;
    use uom::si::ratio::ratio;
    use uom::si::time::second;

    fn slab() -> Arc<outram_foam_basic_lib::mesh::fv_mesh::FvMesh> {
        Arc::new(
            create_one_d_mesh(Length::new::<meter>(1.0), Area::new::<square_meter>(1.0), 4)
                .unwrap(),
        )
    }

    fn point_kinetics_model() -> PointKineticsModel {
        let params = PointKineticsParameters::new(
            vec![Ratio::new::<ratio>(0.0065)],
            vec![Frequency::new::<hertz>(0.08)],
            Time::new::<second>(1.0e-4),
        )
        .unwrap();
        let state = PointKineticsState::new_equilibrium(&params, Power::new::<watt>(1.0e6));
        PointKineticsModel::new(params, state)
    }

    #[test]
    fn dispatch_kind_and_keff_across_variants() {
        // Point kinetics: amplitude-only, no k_eff, power from fission power.
        let pk = NeutronicsModel::PointKinetics(point_kinetics_model());
        assert_eq!(pk.kind(), NeutronicsModelKind::PointKinetics);
        assert!(pk.k_eff().is_none());
        assert!((pk.power().get::<watt>() - 1.0e6).abs() < 1.0);

        // SP3 / SN scaffolds: report a k_eff from their (seeded) state.
        let sp3 = NeutronicsModel::Sp3(Sp3Neutronics::new(slab(), 1, 0));
        assert_eq!(sp3.kind(), NeutronicsModelKind::Sp3);
        assert!((sp3.k_eff().unwrap().get::<ratio>() - 1.0).abs() < 1e-12);

        let sn = NeutronicsModel::Sn(SnNeutronics::new(slab(), 1, 0));
        assert_eq!(sn.kind(), NeutronicsModelKind::Sn);
        assert!(sn.k_eff().is_some());
    }

    #[test]
    fn scaffolds_report_not_implemented() {
        let mut sp3 = Sp3Neutronics::new(slab(), 1, 0);
        assert!(matches!(
            sp3.solve_eigenvalue(),
            Err(NeutronicsError::ModelNotImplemented(
                NeutronicsModelKind::Sp3
            ))
        ));
        let mut sn = SnNeutronics::new(slab(), 1, 0);
        assert!(matches!(
            sn.solve_eigenvalue(),
            Err(NeutronicsError::ModelNotImplemented(
                NeutronicsModelKind::Sn
            ))
        ));
    }

    #[test]
    fn point_kinetics_model_steps() {
        let mut pk = point_kinetics_model();
        pk.step(
            Time::new::<second>(1.0e-3),
            Ratio::new::<ratio>(0.0),
            Power::new::<watt>(0.0),
        )
        .unwrap();
        // Null step at equilibrium keeps the power unchanged.
        assert!((pk.state().fission_power().get::<watt>() - 1.0e6).abs() < 1.0);
    }
}
