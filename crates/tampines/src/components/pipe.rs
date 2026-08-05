//! Pipe / pipeline component.
//!
//! Composes either of TAMPINES's two single-phase flow backends
//! ([`crate::single_phase::SinglePhaseFluidArray`] for lumped molten-salt/oil
//! loops, [`crate::compressible::CompressibleFluidArray`] for CoolProp-backed
//! compressible flow) behind one [`PipeBackend`] enum, plus the pipe
//! geometry a two-phase pressure-drop correlation
//! ([`outram_park_fork_dwsim_libs::pipe::PipeFlowCorrelation`]) would need if
//! the flow becomes two-phase.

use crate::compressible::CompressibleFluidArray;
use crate::single_phase::SinglePhaseFluidArray;
use tampines_steam_tables::openfoam_algorithms::rhoPimpleFoam::TampinesSteamArray;
use tuas_boussinesq_solver::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent;
use crate::TampinesError;
use outram_park_fork_dwsim_libs::pipe::PipeFlowCorrelation;
use uom::si::f64::{Angle, Length, Time};

/// Which single-phase flow model backs a [`Pipe`].
///
/// Enum dispatch, not a trait object, per the workspace's mandatory
/// "no trait objects" Rust design rule.
#[derive(Debug, Clone)]
pub enum PipeBackend {
    /// Lumped-parameter liquid flow (molten salt, thermal oil, ...) --
    /// see [`SinglePhaseFluidArray`] for backend fluid coverage.
    Lumped(SinglePhaseFluidArray),
    /// Compressible, CoolProp-backed flow (gas, near-critical, ...).
    Compressible(CompressibleFluidArray),
    /// Homogeneous-equilibrium (HEM) steam/water flow, backed by
    /// [`TampinesSteamArray`] and IAPWS-IF97 properties.
    ///
    /// This is the two-phase steam/water path, and the intended BASELINE that
    /// higher-fidelity two-phase models (drift-flux, two-fluid) are built on
    /// and measured against — see workspace beads `op-dt3.18` and `op-dt3.19`.
    /// Unlike [`Self::Lumped`] (single-phase liquid) and [`Self::Compressible`]
    /// (single-phase compressible), this variant carries phase information, so
    /// it is the one to reach for when the fluid may be wet.
    SteamHem(TampinesSteamArray),
    /// A TUAS **pre-built** insulated pipe: fluid array, metal pipe shell and
    /// insulation, already thermally coupled to each other and to an ambient
    /// boundary.
    ///
    /// Prefer this over hand-assembling a [`Self::Lumped`] array and wiring
    /// lateral links yourself — TUAS ships the coupling, and it is the only
    /// variant that can report a **wall metal temperature** as well as a fluid
    /// temperature, which is what a pipe's structural limit is judged against.
    InsulatedPipe(InsulatedFluidComponent),
}

/// A pipe or pipeline segment: a flow backend plus the geometry a two-phase
/// correlation would need if the flow becomes two-phase.
#[derive(Debug, Clone)]
pub struct Pipe {
    /// The flow model backing this pipe.
    pub backend: PipeBackend,
    /// Pipe internal diameter.
    pub diameter: Length,
    /// Pipe segment length.
    pub length: Length,
    /// Absolute pipe-wall roughness.
    pub roughness: Length,
    /// Pipe inclination from horizontal, positive = uphill.
    pub inclination: Angle,
    /// Two-phase pressure-drop correlation to use if/when this pipe carries
    /// two-phase flow (single-phase backends use their own native
    /// pressure-drop calculation instead).
    pub two_phase_correlation: PipeFlowCorrelation,
}

impl Pipe {
    /// Construct a new pipe segment around the given flow `backend` and
    /// geometry.
    pub fn new(
        backend: PipeBackend,
        diameter: Length,
        length: Length,
        roughness: Length,
        inclination: Angle,
    ) -> Self {
        Self {
            backend,
            diameter,
            length,
            roughness,
            inclination,
            two_phase_correlation: PipeFlowCorrelation::default(),
        }
    }

    /// Advance this pipe's flow state by one timestep `dt`.
    ///
    /// Not yet implemented -- the physics differs by [`PipeBackend`] variant
    /// (each already has a working native `advance_timestep`/`step` method;
    /// this wrapper's job is dispatching between them and handling the
    /// two-phase case, not yet wired up).
    pub fn step(&mut self, _dt: Time) -> Result<(), TampinesError> {
        Err(TampinesError::NotYetImplemented {
            component: "components::pipe::Pipe::step",
        })
    }
}
