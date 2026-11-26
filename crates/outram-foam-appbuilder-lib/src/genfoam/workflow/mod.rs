// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/neutronics.{C,H} (reactorState_, keff_, pTarget_),
//                    Tutorials/reactorCases/2D_MSFR/Allrun (steadyStateEN -> transient)
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal authors: Carlo Fiorina, Nordine Kerkar, Konstantin Mikityuk (EPFL)
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
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! Eigenvalue -> transient in one call (gh:#1814).
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! GeN-Foam normalises a transient with an eigenvalue calculation run first: with `eigenvalueNeutronics
//! true` every time step rescales `keff_ *= pTot/pTarget_` and the flux to `pTarget_`, and what the stage
//! hands on is the **state**: `keff_` and `pTarget_` (`<time>/uniform/reactorState`), the flux, the
//! precursors, the power density and the thermal-hydraulic fields. The transient then starts from that state
//! with `eigenvalueNeutronics false`: `keff_` is frozen, so the reactor is critical at the target power and
//! only what the case perturbs moves it. Upstream does this with three cloned cases and `foamDictionary`
//! (`Tutorials/reactorCases/2D_MSFR/Allrun`); here it is a value:
//!
//! ```ignore
//! // the cases are `2D_MSFR`'s: see tests/genfoam_msfr_chain_vs_upstream.rs for the whole thing
//! let stage = EigenvalueStage::EnergyNeutroDiffusion { stage: coupled, controls }; // steadyStateEN
//! let normalised = stage.run()?;                                  // typed NormalisedState
//! println!("keff = {}", normalised.reactor_state().k_eff);
//! let mut transient = normalised.start_transient(TransientStart::energy_neutro(settings))?;
//! while transient.time() < 175.0 { transient.step_adjustable()?; } // keff frozen
//! ```
//!
//! # Enum dispatch
//!
//! [`EigenvalueStage`] is the first stage, [`NormalisedState`] its result, [`TransientRun`] the second
//! stage. The neutronics models the port has an eigenvalue AND a transient path for:
//!
//! | model | eigenvalue stage | transient stage | verified against the real binary |
//! |---|---|---|---|
//! | diffusion, stationary fuel | `pseudo_time` (`calcKeff.H`) | `DiffusionNeutronics::step` | eigenvalue yes (round 2 guided slabs); transient no (port-native scheme) |
//! | SP3, stationary fuel | `pseudo_time` | `Sp3Neutronics::step` | eigenvalue yes; transient no |
//! | diffusion, circulating fuel + energy-only `onePhase` | `steadyStateEN` ([`EnergyNeutroDiffusion`]) | `energyNeutroLoop` ([`EnergyNeutroTransient`]) | both (round 11) |
//!
//! Refused by name: `steadyStateTH` (fluid mechanics, gh:#1812), SN (no transient path), point kinetics (no
//! eigenvalue: its `keff_` is not iterated upstream).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use uom::si::f64::Time;

pub mod energy_neutro;
pub mod reactor_state;

pub use energy_neutro::{
    precursor_transport_from_velocity, EigenvalueRunControls, EnergyEigenvalueRecord, EnergyNeutroDiffusion,
    EnergyNeutroTransient, TransientRecord, TransientSettings,
};
pub use reactor_state::ReactorState;

use crate::genfoam::neutronics::diffusion::DiffusionNeutronics;
use crate::genfoam::neutronics::pseudo_time::{
    run_pseudo_time_eigenvalue, PseudoTimeControls, PseudoTimeError, PseudoTimeRecord,
};
use crate::genfoam::neutronics::sp3::Sp3Neutronics;
use crate::genfoam::neutronics::NeutronicsError;

/// Why a workflow stage could not run.
#[derive(Debug, thiserror::Error)]
pub enum WorkflowError {
    /// A neutronics model refused or failed.
    #[error("neutronics: {0}")]
    Neutronics(#[from] NeutronicsError),
    /// The pseudo-time eigenvalue driver refused or failed.
    #[error("pseudo-time eigenvalue: {0}")]
    PseudoTime(#[from] PseudoTimeError),
    /// A case input is inconsistent.
    #[error("case: {0}")]
    Case(String),
    /// A stage the port does not implement, named (extend the port, never this refusal).
    #[error("`{stage}` is not ported: {reason}")]
    NotPorted {
        /// The upstream stage or solver.
        stage: &'static str,
        /// Why, and the issue that tracks it.
        reason: &'static str,
    },
    /// `reactorState` could not be read.
    #[error("reactorState: {0}")]
    ReactorState(String),
}

/// The first stage of the chain. The stationary-fuel models arrive at their start state (flux and `keff_`
/// set with `set_initial_guess`); `controls.p_target` is `pTarget`.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum EigenvalueStage {
    /// Neutronics alone, diffusion, stationary fuel (`regionSolvers { neutroRegion diffusionNeutronics; }`).
    Diffusion {
        /// The model at its start state.
        model: DiffusionNeutronics,
        /// `controlDict` of the stage (`pTarget` included).
        controls: PseudoTimeControls,
    },
    /// Neutronics alone, SP3, stationary fuel.
    Sp3 {
        /// The model at its start state.
        model: Sp3Neutronics,
        /// `controlDict` of the stage.
        controls: PseudoTimeControls,
    },
    /// `steadyStateEN` of `2D_MSFR`: circulating-fuel diffusion coupled with the energy-only `onePhase`.
    EnergyNeutroDiffusion {
        /// The two regions at their start state.
        stage: EnergyNeutroDiffusion,
        /// `controlDict` of the stage.
        controls: EigenvalueRunControls,
    },
    /// `steadyStateTH`: fluid mechanics, not ported (gh:#1812).
    SteadyStateTh,
    /// SN has an eigenvalue driver but no transient to hand the state to.
    Sn,
    /// Point kinetics has no eigenvalue stage.
    PointKinetics,
}

/// The normalised initial state: what the eigenvalue stage hands the transient.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum NormalisedState {
    /// Stationary-fuel diffusion.
    Diffusion {
        /// The model at the converged flux, precursors and `keff_`.
        model: DiffusionNeutronics,
        /// `keff_` and `pTarget_`.
        reactor: ReactorState,
        /// `keff_` of every pseudo-time step.
        history: Vec<PseudoTimeRecord>,
    },
    /// Stationary-fuel SP3.
    Sp3 {
        /// The model.
        model: Sp3Neutronics,
        /// `keff_` and `pTarget_`.
        reactor: ReactorState,
        /// `keff_` of every pseudo-time step.
        history: Vec<PseudoTimeRecord>,
    },
    /// The coupled circulating-fuel state (flux, `precStar`, `powerDensity`, `T` at the end of the stage).
    EnergyNeutroDiffusion {
        /// The two regions.
        stage: EnergyNeutroDiffusion,
        /// Every step of the stage.
        history: Vec<EnergyEigenvalueRecord>,
    },
}

impl EigenvalueStage {
    /// Run the eigenvalue stage to its `endTime`.
    ///
    /// # Errors
    ///
    /// [`WorkflowError::NotPorted`] naming `steadyStateTH`, SN or point kinetics; otherwise the stage's error.
    pub fn run(self) -> Result<NormalisedState, WorkflowError> {
        match self {
            Self::Diffusion { mut model, controls } => {
                let history = run_pseudo_time_eigenvalue(&mut model, &controls)?;
                let reactor = ReactorState { k_eff: model.state().k_eff_raw(), p_target: controls.p_target };
                Ok(NormalisedState::Diffusion { model, reactor, history })
            }
            Self::Sp3 { mut model, controls } => {
                let history = run_pseudo_time_eigenvalue(&mut model, &controls)?;
                let reactor = ReactorState { k_eff: model.state().k_eff_raw(), p_target: controls.p_target };
                Ok(NormalisedState::Sp3 { model, reactor, history })
            }
            Self::EnergyNeutroDiffusion { mut stage, controls } => {
                let history = stage.run_eigenvalue(&controls, |_, _| {})?;
                Ok(NormalisedState::EnergyNeutroDiffusion { stage, history })
            }
            Self::SteadyStateTh => Err(WorkflowError::NotPorted {
                stage: "steadyStateTH",
                reason: "the fluid-mechanics (momentum + pressure) stage is gh:#1812",
            }),
            Self::Sn => Err(WorkflowError::NotPorted {
                stage: "SN eigenvalue -> transient",
                reason: "SN has a pseudo-time eigenvalue driver but no transient step to start",
            }),
            Self::PointKinetics => Err(WorkflowError::NotPorted {
                stage: "pointKinetics eigenvalue",
                reason: "point kinetics solves no eigenvalue (keff_ is an input)",
            }),
        }
    }
}

/// What the transient stage needs besides the state.
#[derive(Debug, Clone)]
pub enum TransientStart {
    /// Stationary-fuel models take nothing further.
    Plain,
    /// The coupled circulating-fuel transient.
    EnergyNeutro(TransientSettings),
}

impl NormalisedState {
    /// `keff_` and `pTarget_` of the state (what `uniform/reactorState` holds).
    #[must_use]
    pub fn reactor_state(&self) -> ReactorState {
        match self {
            Self::Diffusion { reactor, .. } | Self::Sp3 { reactor, .. } => *reactor,
            Self::EnergyNeutroDiffusion { stage, .. } => stage.reactor,
        }
    }

    /// Build the transient from the state, `eigenvalueNeutronics false`: `keff_` is not updated again.
    ///
    /// # Errors
    ///
    /// [`WorkflowError::Case`] if `start` does not match the model; the transient construction error.
    pub fn start_transient(self, start: TransientStart) -> Result<TransientRun, WorkflowError> {
        match (self, start) {
            (Self::Diffusion { model, .. }, TransientStart::Plain) => Ok(TransientRun::Diffusion { model, time: 0.0 }),
            (Self::Sp3 { model, .. }, TransientStart::Plain) => Ok(TransientRun::Sp3 { model, time: 0.0 }),
            (Self::EnergyNeutroDiffusion { stage, .. }, TransientStart::EnergyNeutro(settings)) => {
                Ok(TransientRun::EnergyNeutroDiffusion(EnergyNeutroTransient::new(stage, settings)?))
            }
            _ => Err(WorkflowError::Case("the transient start does not match the model of the eigenvalue stage".into())),
        }
    }
}

/// The second stage of the chain, with `keff_` frozen.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum TransientRun {
    /// Stationary-fuel diffusion.
    Diffusion {
        /// The model.
        model: DiffusionNeutronics,
        /// Elapsed transient time [s].
        time: f64,
    },
    /// Stationary-fuel SP3.
    Sp3 {
        /// The model.
        model: Sp3Neutronics,
        /// Elapsed transient time [s].
        time: f64,
    },
    /// Circulating-fuel diffusion + energy-only `onePhase`, adjustable step.
    EnergyNeutroDiffusion(EnergyNeutroTransient),
}

impl TransientRun {
    /// Total power of the neutronics [W] (`power()`).
    #[must_use]
    pub fn power(&self) -> f64 {
        match self {
            Self::Diffusion { model, .. } => model.state().total_power_raw(),
            Self::Sp3 { model, .. } => model.state().total_power_raw(),
            Self::EnergyNeutroDiffusion(t) => t.inner.neutro.state().total_power_raw(),
        }
    }

    /// `keff_` (frozen during the transient).
    #[must_use]
    pub fn k_eff(&self) -> f64 {
        match self {
            Self::Diffusion { model, .. } => model.state().k_eff_raw(),
            Self::Sp3 { model, .. } => model.state().k_eff_raw(),
            Self::EnergyNeutroDiffusion(t) => t.inner.neutro.state().k_eff_raw(),
        }
    }

    /// One fixed step of `dt` for the stationary-fuel models.
    ///
    /// # Errors
    ///
    /// [`WorkflowError::NotPorted`] for the coupled model (its step is adjustable: use
    /// [`Self::step_adjustable`]); the neutronics step's error.
    pub fn step_fixed(&mut self, dt: Time) -> Result<(), WorkflowError> {
        match self {
            Self::Diffusion { model, time } => {
                model.step(dt)?;
                *time += dt.value;
                Ok(())
            }
            Self::Sp3 { model, time } => {
                model.step(dt)?;
                *time += dt.value;
                Ok(())
            }
            Self::EnergyNeutroDiffusion(_) => Err(WorkflowError::NotPorted {
                stage: "fixed step of the coupled transient",
                reason: "the coupled transient's step follows adjustDeltaT; call step_adjustable",
            }),
        }
    }

    /// One adjustable step of the coupled transient.
    ///
    /// # Errors
    ///
    /// [`WorkflowError::NotPorted`] for the stationary-fuel models; the step's error.
    pub fn step_adjustable(&mut self) -> Result<TransientRecord, WorkflowError> {
        match self {
            Self::EnergyNeutroDiffusion(t) => t.step(),
            _ => Err(WorkflowError::NotPorted {
                stage: "adjustable step of a stationary-fuel model",
                reason: "stationary-fuel models step with step_fixed",
            }),
        }
    }
}
