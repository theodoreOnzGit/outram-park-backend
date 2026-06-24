/// External neutron source definition.
///
/// C++ source: `src/source.cpp` (778 LOC), `include/openmc/source.h`.
///
/// An `IndependentSource` is defined by three independent distributions:
///   - Spatial:  where the source particle starts
///   - Energy:   kinetic energy of the source neutron
///   - Angular:  initial direction
///
/// The source is sampled once per source particle at the start of each history.

use crate::geometry::position::{Direction, Position};

/// A sampled source particle state.
#[derive(Debug, Clone, Copy)]
pub struct SourceSite {
    pub r: Position,
    pub u: Direction,
    pub e: f64,
    pub wgt: f64,
}

/// Independent (uncorrelated) external source.  Maps to `openmc::IndependentSource`.
pub struct IndependentSource {
    pub spatial: Box<dyn super::spatial::SpatialDist>,
    pub energy:  Box<dyn super::energy::EnergyDist>,
    pub angle:   Box<dyn super::angle::AngleDist>,
    pub strength: f64,
}

impl IndependentSource {
    /// Sample one source particle.
    pub fn sample(&self, seed: &mut u64) -> SourceSite {
        let r = self.spatial.sample(seed);
        let e = self.energy.sample(seed);
        let u = self.angle.sample(seed, e);
        SourceSite { r, u, e, wgt: self.strength }
    }
}
