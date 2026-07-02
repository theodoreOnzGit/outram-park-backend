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
    fn sample(&self, seed: &mut u64) -> Position {
        // Radius sampled uniform-in-volume: the volume element is ∝ r² dr, so the
        // CDF over [r_inner, r_outer] inverts to r = (r_i³ + ξ(r_o³ − r_i³))^{1/3}.
        // A degenerate shell (r_inner == r_outer) yields that exact radius.
        let ri3 = self.r_inner.powi(3);
        let ro3 = self.r_outer.powi(3);
        let r = (ri3 + prn(seed) * (ro3 - ri3)).cbrt();
        // Isotropic point on the sphere of that radius.
        let (dx, dy, dz) = crate::rng::distributions::isotropic_direction(seed);
        Position::new(
            self.center.x + r * dx,
            self.center.y + r * dy,
            self.center.z + r * dz,
        )
    }
}
