//! Transmission and reflection at a TRISO layer interface.
//!
//! When a Walk-on-Spheres walker reaches an interface between two materials with
//! diffusion coefficients `D1` (the side it is on) and `D2` (the side it is
//! crossing into), it does not pass freely: the diffusion equation requires the
//! concentration **and** the flux to stay continuous across the interface (the
//! standard BISON / Jiang TRISO treatment — no chemical segregation unless a
//! partition coefficient is supplied). Discretising that continuity condition
//! gives the probability that a walker arriving at the interface **transmits**
//! rather than **reflects**:
//!
//! ```text
//! p_transmit = K * sqrt(D2) / ( sqrt(D1) + K * sqrt(D2) )
//! ```
//!
//! where `K` is an optional partition/solubility ratio (`K = 1` for pure
//! concentration continuity). This is the piece that turns SiC — whose `D` is
//! ~10^6 times smaller than the pyrolytic-carbon layers around it — into the
//! containment barrier: a walker arriving from PyC transmits into SiC with
//! probability `~sqrt(D_SiC / D_PyC)`, i.e. it is reflected back the
//! overwhelming majority of the time.
//!
//! This Phase-0 scaffold provides the pure probability function. Wiring it into
//! the per-hop walk (reflect vs. transmit, then continue Walk-on-Spheres in the
//! chosen region) is the multilayer interface phase.

use uom::si::diffusion_coefficient::square_meter_per_second;
use uom::si::f64::DiffusionCoefficient;

/// Probability that a walker arriving at a `D1 | D2` interface transmits into
/// the `D2` side (rather than reflecting back into the `D1` side).
///
/// Implements `p = K*sqrt(D2) / (sqrt(D1) + K*sqrt(D2))`, the discretised
/// continuity-of-concentration-and-flux rule.
///
/// # Arguments
///
/// - `d_current` — diffusion coefficient on the side the walker is currently on
///   (`D1`), a [`DiffusionCoefficient`] (m^2/s).
/// - `d_next` — diffusion coefficient on the side being entered (`D2`), a
///   [`DiffusionCoefficient`] (m^2/s).
/// - `partition_k` — dimensionless partition/solubility ratio `K`. Use `1.0`
///   for plain concentration continuity (the default TRISO assumption).
///
/// # Returns
///
/// A probability in `[0, 1]`. Returns `0.0` if both coefficients are zero (an
/// impenetrable interface), so the walker always reflects.
#[inline]
pub fn transmission_probability(
    d_current: DiffusionCoefficient,
    d_next: DiffusionCoefficient,
    partition_k: f64,
) -> f64 {
    let sqrt_d1 = d_current.get::<square_meter_per_second>().max(0.0).sqrt();
    let sqrt_d2 = d_next.get::<square_meter_per_second>().max(0.0).sqrt();
    let numerator = partition_k * sqrt_d2;
    let denominator = sqrt_d1 + numerator;
    if denominator <= 0.0 {
        return 0.0;
    }
    numerator / denominator
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn d(x: f64) -> DiffusionCoefficient {
        DiffusionCoefficient::new::<square_meter_per_second>(x)
    }

    #[test]
    fn equal_coefficients_split_evenly() {
        // Same D on both sides, unit partition => 50/50 transmit/reflect.
        assert_relative_eq!(transmission_probability(d(6.3e-8), d(6.3e-8), 1.0), 0.5);
    }

    #[test]
    fn sic_is_a_strong_barrier() {
        // PyC (6.3e-8) into SiC (5.5e-14): transmission is tiny, ~sqrt(D2/D1).
        let p = transmission_probability(d(6.3e-8), d(5.5e-14), 1.0);
        let expected = (5.5e-14_f64 / 6.3e-8).sqrt(); // p ~ sqrt(D2)/sqrt(D1) when D2 << D1
        assert!(p < 1e-2, "SiC transmission should be << 1, got {p}");
        assert_relative_eq!(p, expected, max_relative = 1e-3);
    }

    #[test]
    fn impenetrable_interface_reflects() {
        assert_eq!(transmission_probability(d(0.0), d(0.0), 1.0), 0.0);
    }
}
