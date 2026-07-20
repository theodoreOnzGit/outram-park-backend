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
