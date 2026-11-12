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
use super::angle::AngleDist;
use super::energy::EnergyDist;
use super::spatial::SpatialDist;

/// A sampled source particle state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceSite {
    pub r: Position,
    pub u: Direction,
    pub e: f64,
    pub wgt: f64,
}

/// Independent (uncorrelated) external source.  Maps to `openmc::IndependentSource`.
#[derive(Debug, Clone)]
pub struct IndependentSource {
    /// Where the source particle starts.
    pub spatial: super::spatial::SpatialKind,
    /// Its kinetic energy \[eV\].
    pub energy: super::energy::EnergyKind,
    /// Its initial direction.
    pub angle: super::angle::AngleKind,
    /// Source strength, carried into the sampled site's weight.
    pub strength: f64,
}

impl IndependentSource {
    /// Sample one source particle.
    ///
    /// The site weight is `strength` times the importance weights of the
    /// spatial and energy draws. Those are exactly `1.0` for every
    /// non-`Distribution` variant, so existing sources sample bit-identically
    /// (`x * 1.0 == x`, and the draw order is unchanged).
    pub fn sample(&self, seed: &mut u64) -> SourceSite {
        let (r, wr) = self.spatial.sample_weighted(seed);
        let (e, we) = self.energy.sample_weighted(seed);
        let u = self.angle.sample(seed, e);
        SourceSite {
            r,
            u,
            e,
            wgt: self.strength * wr * we,
        }
    }

    /// `n` independent source sites for a **k-eigenvalue initial source**,
    /// drawn from `seed` (advanced in place), in the form
    /// `DistributedPowerIteration::from_initial_source`
    /// (`physics::transport_csg::distributed`) takes. The k-eigenvalue
    /// iteration carries no per-site weight, so importance weights are dropped:
    /// use only unbiased distributions here.
    pub fn sample_initial_source(
        &self,
        n: usize,
        seed: &mut u64,
    ) -> Vec<crate::physics::transport_csg::distributed::SourceSite> {
        (0..n)
            .map(|_| {
                let s = self.sample(seed);
                crate::physics::transport_csg::distributed::SourceSite {
                    r: [s.r.x, s.r.y, s.r.z],
                    u: [s.u.u, s.u.v, s.u.w],
                    e: s.e,
                    delayed_group: None,
                }
            })
            .collect()
    }
}
