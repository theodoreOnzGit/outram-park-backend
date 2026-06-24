/// Spatial source distributions.
///
/// C++ source: `src/distribution_spatial.cpp`, `include/openmc/distribution_spatial.h`.

use crate::geometry::position::Position;
use crate::rng::lcg::prn;

/// Trait for spatial distributions.
pub trait SpatialDist: Send + Sync {
    fn sample(&self, seed: &mut u64) -> Position;
}

/// Point source — all particles start at the same location.
pub struct PointSource { pub r: Position }
impl SpatialDist for PointSource {
    fn sample(&self, _seed: &mut u64) -> Position { self.r }
}

/// Uniform box source.
pub struct BoxSource { pub lower_left: Position, pub upper_right: Position }
impl SpatialDist for BoxSource {
    fn sample(&self, seed: &mut u64) -> Position {
        Position::new(
            self.lower_left.x + (self.upper_right.x - self.lower_left.x) * prn(seed),
            self.lower_left.y + (self.upper_right.y - self.lower_left.y) * prn(seed),
            self.lower_left.z + (self.upper_right.z - self.lower_left.z) * prn(seed),
        )
    }
}

/// Spherical shell source (uniform surface or volume).
/// TODO: port from `distribution_spatial.cpp`.
pub struct SphericalSource {
    pub center: Position,
    pub r_inner: f64,
    pub r_outer: f64,
}
impl SpatialDist for SphericalSource {
    fn sample(&self, _seed: &mut u64) -> Position {
        todo!("SphericalSource::sample: port from distribution_spatial.cpp")
    }
}
