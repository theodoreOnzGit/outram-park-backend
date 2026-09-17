/// Angular source distributions.
///
/// C++ source: `src/distribution_angle.cpp`, `include/openmc/distribution_angle.h`.
use crate::geometry::position::Direction;

/// Trait for angular distributions.
pub trait AngleDist: Send + Sync {
    /// Sample a direction. `e` is the particle energy (eV) for energy-dependent
    /// distributions (e.g. Legendre expansion at each energy point).
    fn sample(&self, seed: &mut u64, e: f64) -> Direction;
}

/// Isotropic — uniform on the unit sphere.
/// TODO: port from `distribution_angle.cpp`.
pub struct IsotropicAngle;
impl AngleDist for IsotropicAngle {
    fn sample(&self, seed: &mut u64, _e: f64) -> Direction {
        let (u, v, w) = crate::rng::distributions::isotropic_direction(seed);
        Direction::new(u, v, w)
    }
}

/// Monodirectional — all particles in the same direction.
pub struct MonodirectionalAngle {
    pub d: Direction,
}
impl AngleDist for MonodirectionalAngle {
    fn sample(&self, _seed: &mut u64, _e: f64) -> Direction {
        self.d
    }
}

/// Every source angular distribution, as a closed enum. See
/// [`super::spatial::SpatialKind`] for why this is an enum and not a trait
/// object.
pub enum AngleKind {
    /// [`IsotropicAngle`].
    Isotropic(IsotropicAngle),
    /// [`MonodirectionalAngle`].
    Monodirectional(MonodirectionalAngle),
}

impl AngleDist for AngleKind {
    fn sample(&self, seed: &mut u64, e: f64) -> Direction {
        match self {
            AngleKind::Isotropic(d) => d.sample(seed, e),
            AngleKind::Monodirectional(d) => d.sample(seed, e),
        }
    }
}
