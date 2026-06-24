/// Particle phase-space state.
///
/// C++ source: `src/particle.cpp` (1044 LOC), `include/openmc/particle.h` (136 LOC).
/// Base data layout: `src/particle_data.cpp`, `include/openmc/particle_data.h`.
///
/// A `Particle` carries its full phase-space state (position, direction, energy,
/// weight) and bookkeeping needed by the transport loop (cell/material indices,
/// surface crossing history, RNG seeds, event log).
///
/// Units: position in cm, energy in eV.

use crate::geometry::position::{Direction, Position};

/// Particle type.  Maps to `openmc::Particle::Type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticleType {
    Neutron,
    Photon,
    Electron,
    Positron,
}

/// Event type — recorded each step for event-based tallying.
/// Maps to `openmc::TallyEvent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TallyEvent {
    None,
    Scatter,
    Fission,
    Absorption,
    Surface,
    Leak,
}

/// Full Monte Carlo particle state.  Maps to `openmc::Particle`.
///
/// Fields mirror `particle_data.h` then `particle.h` in order.
pub struct Particle {
    // ── Phase-space coordinates ──────────────────────────────────────────────
    /// Current position (cm).
    pub r: Position,
    /// Current direction cosines (unit vector).
    pub u: Direction,
    /// Current kinetic energy (eV).
    pub e: f64,
    /// Statistical weight.
    pub wgt: f64,

    // ── Geometry state ───────────────────────────────────────────────────────
    /// Cell index in global cell array.
    pub cell: usize,
    /// Material index (usize::MAX for void).
    pub material: usize,
    /// Surface index of the last surface crossed (usize::MAX if none).
    pub surface: usize,

    // ── RNG ──────────────────────────────────────────────────────────────────
    /// Primary RNG seed for this particle.
    pub seed: u64,

    // ── Bookkeeping ──────────────────────────────────────────────────────────
    pub particle_type: ParticleType,
    pub alive: bool,
    pub event: TallyEvent,

    // ── History ──────────────────────────────────────────────────────────────
    /// Number of collisions this particle has had.
    pub n_collision: u32,
}

impl Particle {
    /// Create a new particle at the given phase-space coordinates.
    pub fn new(r: Position, u: Direction, e: f64, wgt: f64, seed: u64) -> Self {
        Self {
            r, u, e, wgt,
            cell: 0,
            material: usize::MAX,
            surface: usize::MAX,
            seed,
            particle_type: ParticleType::Neutron,
            alive: true,
            event: TallyEvent::None,
            n_collision: 0,
        }
    }
}
