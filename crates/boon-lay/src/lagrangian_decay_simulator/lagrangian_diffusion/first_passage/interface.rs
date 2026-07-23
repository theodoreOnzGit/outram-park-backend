//! Transmission and reflection at a TRISO layer interface.
//!
//! When a Walk-on-Spheres walker reaches an interface between two materials with
//! diffusion coefficients `D1` (the side it is on) and `D2` (the side it is
//! crossing into), it does not pass freely: the diffusion equation requires the
//! concentration **and** the flux to stay continuous across the interface (the
//! standard BISON / Jiang TRISO treatment — no chemical segregation unless a
//! partition coefficient is supplied). The equilibrium such an interface must
//! reproduce is a **uniform concentration** for `K = 1` (zero net flux implies a
//! flat profile), or a concentration ratio `c2/c1 = K` for a partition ratio
//! `K`.
//!
//! The transmission probability that reproduces this depends on **how often the
//! particular random-walk scheme visits the interface**. In Walk-on-Spheres the
//! walker is reinserted a fixed distance from the interface and then takes a hop
//! whose duration is `tau ~ R^2 / D`, so the *rate* of interface encounters from
//! side `i` scales as `D_i` (the geometric return probability per hop is the
//! same on both sides). Detailed balance at equilibrium,
//! `c1 * D1 * p_(1->2) = c2 * D2 * p_(2->1)` with `c2/c1 = K`, then gives
//!
//! ```text
//! p_transmit = K * D2 / ( D1 + K * D2 ).
//! ```
//!
//! (This is the rule for the *Walk-on-Spheres* encounter statistics. A
//! fixed-time-step walk, whose step length scales as `sqrt(D)` and whose
//! encounter rate scales as `1/sqrt(D)`, needs the different
//! `sqrt(D)`-ratio rule — using that here would give the wrong equilibrium. See
//! the `interface_rule_gives_uniform_equilibrium_density` test in
//! `walk_on_spheres`, which checks the density is uniform to a few percent
//! across a 10x diffusivity contrast.)
//!
//! This is the piece that turns SiC — whose `D` is ~10^6 times smaller than the
//! pyrolytic-carbon layers around it — into the containment barrier: a walker
//! arriving from PyC transmits into SiC with probability `~ D_SiC / D_PyC`, i.e.
//! it is reflected back the overwhelming majority of the time.

use outram_mc_libs::rng::lcg::prn;
use uom::si::diffusion_coefficient::square_meter_per_second;
use uom::si::f64::DiffusionCoefficient;

/// Probability that a walker arriving at a `D1 | D2` interface transmits into
/// the `D2` side (rather than reflecting back into the `D1` side).
///
/// Implements `p = K*D2 / (D1 + K*D2)`, the rule that makes the Walk-on-Spheres
/// scheme reproduce Fickian diffusion with continuity of concentration (a
/// concentration ratio `K` across the interface). See the module docs for why
/// this is linear in `D`, not `sqrt(D)`, for this scheme.
///
/// # Arguments
///
/// - `d_current` — diffusion coefficient on the side the walker is currently on
///   (`D1`), a [`DiffusionCoefficient`] (m^2/s).
/// - `d_next` — diffusion coefficient on the side being entered (`D2`), a
///   [`DiffusionCoefficient`] (m^2/s).
/// - `partition_k` — dimensionless partition/solubility ratio `K` (equilibrium
///   `c2/c1`). Use `1.0` for plain concentration continuity (the default TRISO
///   assumption).
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
    let d1 = d_current.get::<square_meter_per_second>().max(0.0);
    let d2 = d_next.get::<square_meter_per_second>().max(0.0);
    let numerator = partition_k * d2;
    let denominator = d1 + numerator;
    if denominator <= 0.0 {
        return 0.0;
    }
    numerator / denominator
}

/// Decide whether a walker arriving at a `D1 | D2` interface transmits.
///
/// Draws one uniform deviate from `seed` (the caller's LCG state) and returns
/// `true` with probability [`transmission_probability`]`(d_current, d_next,
/// partition_k)`, `false` otherwise (reflection).
#[inline]
pub fn does_transmit(
    seed: &mut u64,
    d_current: DiffusionCoefficient,
    d_next: DiffusionCoefficient,
    partition_k: f64,
) -> bool {
    prn(seed) < transmission_probability(d_current, d_next, partition_k)
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
        // PyC (6.3e-8) into SiC (5.5e-14): transmission is tiny, ~D2/D1.
        let p = transmission_probability(d(6.3e-8), d(5.5e-14), 1.0);
        let expected = 5.5e-14_f64 / 6.3e-8; // p ~ D2/D1 when D2 << D1
        assert!(p < 1e-5, "SiC transmission should be << 1, got {p}");
        assert_relative_eq!(p, expected, max_relative = 1e-3);
    }

    #[test]
    fn partition_shifts_transmission() {
        // A partition ratio K>1 (higher solubility on the far side) raises the
        // transmission probability toward the far side.
        let p1 = transmission_probability(d(1e-8), d(1e-8), 1.0);
        let p_k = transmission_probability(d(1e-8), d(1e-8), 4.0);
        assert_relative_eq!(p1, 0.5);
        assert_relative_eq!(p_k, 4.0 / 5.0); // K/(1+K)
    }

    #[test]
    fn impenetrable_interface_reflects() {
        assert_eq!(transmission_probability(d(0.0), d(0.0), 1.0), 0.0);
    }

    #[test]
    fn does_transmit_frequency_matches_probability() {
        let mut seed = 0x0bad_c0de_u64;
        let p = transmission_probability(d(6.3e-8), d(6.3e-9), 1.0);
        let n = 200_000;
        let hits = (0..n)
            .filter(|_| does_transmit(&mut seed, d(6.3e-8), d(6.3e-9), 1.0))
            .count();
        let empirical = hits as f64 / n as f64;
        assert_relative_eq!(empirical, p, max_relative = 0.02);
    }
}
