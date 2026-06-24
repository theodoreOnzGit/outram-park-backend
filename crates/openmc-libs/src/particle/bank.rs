/// Fission site bank — stores secondary neutron phase-space for the next generation.
///
/// C++ source: `src/bank.cpp`, `include/openmc/bank.h`.
///
/// The bank accumulates fission neutron sites during a generation.  At the
/// start of the next generation, particles are sampled (with replacement)
/// from the bank to form the next source.

use crate::geometry::position::{Direction, Position};

/// A single entry in the fission site bank.  Maps to `openmc::SourceSite`.
#[derive(Debug, Clone, Copy)]
pub struct BankSite {
    pub r: Position,
    pub u: Direction,
    pub e: f64,
    pub wgt: f64,
    /// RNG seed to use when this site becomes a source particle.
    pub seed: u64,
}

/// Particle bank.
pub struct Bank {
    pub sites: Vec<BankSite>,
}

impl Bank {
    pub fn new() -> Self { Self { sites: Vec::new() } }

    pub fn push(&mut self, site: BankSite) { self.sites.push(site); }

    pub fn len(&self) -> usize { self.sites.len() }

    pub fn clear(&mut self) { self.sites.clear(); }
}

impl Default for Bank { fn default() -> Self { Self::new() } }
