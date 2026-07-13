//! Stochastic-media geometry for dispersion fuel — **scaffold (stub)**.
//!
//! This is where packed-particle geometry generation and sampling will live: the
//! Random Sequential Addition (RSA), Chord Length Sampling (CLS), and RSA–DEM /
//! ODR–DEM methods that produce and query the doubly-heterogeneous TRISO/pebble
//! layout the [`super::delta_tracking`] flight moves through. See the
//! [`super::references`] bibliography (Tan et al.) for the algorithms.
//!
//! **Status:** interface sketch only — the generators are not implemented yet.
//! The types below fix the vocabulary (a packed [`Sphere`], a [`PackingConfig`])
//! so callers and docs can refer to them, but [`PackingConfig::generate`] returns
//! [`StochasticMediaError::NotImplemented`] for now.

use crate::geometry::position::Position;

/// One packed spherical particle (e.g. a TRISO kernel) in a stochastic medium.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sphere {
    /// Centre \[cm\].
    pub center: Position,
    /// Radius \[cm\].
    pub radius: f64,
}

/// Which packing-generation algorithm to use. See [`super::references`].
///
/// The set is closed and dispatched by `match` (no trait objects), per the
/// workspace design rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackingMethod {
    /// Random Sequential Addition — reject-until-no-overlap insertion. Simple and
    /// parallelizable, but saturates around ~38 % packing fraction.
    /// Ref: [`super::references::TAN2024_RSA`].
    Rsa,
    /// Iterative RSA followed by Discrete-Element-Method relaxation, to reach the
    /// high packing fractions RSA alone cannot. Ref:
    /// [`super::references::TAN2026_RSA_DEM`].
    RsaDem,
    /// Coupled Ordered/Overlap-Driven-Relaxation with DEM. Ref:
    /// [`super::references::TAN2026_ODR_DEM`].
    OdrDem,
}

/// Parameters for generating a packed dispersion-fuel region.
#[derive(Debug, Clone, Copy)]
pub struct PackingConfig {
    /// Particle radius \[cm\] (all particles equal-radius for now).
    pub particle_radius: f64,
    /// Target volumetric packing fraction (0…1).
    pub packing_fraction: f64,
    /// Half-width \[cm\] of the cubic domain the particles pack into.
    pub domain_half_width: f64,
    /// Generation algorithm.
    pub method: PackingMethod,
    /// RNG seed for reproducibility.
    pub seed: u64,
}

/// Errors from stochastic-media generation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StochasticMediaError {
    /// The generator is not implemented yet (scaffold).
    #[error("stochastic-media generator not implemented yet (scaffold): {0:?}")]
    NotImplemented(PackingMethod),
}

impl PackingConfig {
    /// Generate the packed sphere list — **not implemented (scaffold)**.
    ///
    /// Will produce a non-overlapping set of [`Sphere`]s realizing
    /// `packing_fraction` in the domain via `method`. Returns
    /// [`StochasticMediaError::NotImplemented`] until the generators are ported.
    pub fn generate(&self) -> Result<Vec<Sphere>, StochasticMediaError> {
        Err(StochasticMediaError::NotImplemented(self.method))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_is_stubbed() {
        let cfg = PackingConfig {
            particle_radius: 0.025,
            packing_fraction: 0.3,
            domain_half_width: 1.0,
            method: PackingMethod::Rsa,
            seed: 1,
        };
        assert_eq!(
            cfg.generate(),
            Err(StochasticMediaError::NotImplemented(PackingMethod::Rsa))
        );
    }
}
