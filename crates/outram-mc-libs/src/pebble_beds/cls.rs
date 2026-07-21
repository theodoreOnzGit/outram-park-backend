//! Classical Chord Length Sampling (CLS) — memoryless random-media transport.
//!
//! CLS replaces stored geometry with a *distribution*. Instead of asking an explicit
//! packing "where is the next kernel surface along this ray?", CLS samples the distance
//! to the next inclusion crossing from a chord-length distribution whose mean is fixed
//! by the packing statistics. Nothing is remembered between samples.
//!
//! ```text
//! explicit:  ray ──►│kernel│────►│kernel│───►     (surfaces looked up)
//! CLS:       ray ──► sample ℓ₁ ──► sample ℓ₂ ──►  (surfaces re-invented each time)
//! ```
//!
//! The win is memory and speed: a pebble holds O(10⁴) kernels and a core O(10⁵)
//! pebbles, none of which CLS stores. The cost is the **Markov (memoryless)
//! assumption** — each sampled chord is independent of every previous one, so the
//! model forgets geometry. A neutron that scatters backwards does not re-encounter the
//! kernel it just traversed, and clustered or correlated packings are not reproduced.
//! Recovering that memory is what [`super::scls`] exists to do.
//!
//! # What is implemented here
//!
//! The chord-length statistics, which are exact, closed-form and independently
//! testable:
//!
//! - [`mean_chord_length_sphere`] — Cauchy's mean-chord result for a convex body.
//! - [`matrix_mean_chord_length`] — the binary-Markovian matrix counterpart.
//! - [`sample_chord`] — exponential sampling from a mean chord length.
//!
//! # What is NOT implemented here
//!
//! The CLS **transport driver** — threading these samples through a flight, handling
//! inclusion entry/exit bookkeeping, and coupling to the k-eigenvalue loop — is not
//! built out. [`ClsMedium::material_at`] returns
//! [`MediumError::NotImplemented`] rather
//! than a fabricated answer. Tracked as bead `op-eby.2`.
//!
//! # References
//!
//! - Cauchy's formula for the mean chord of a convex body, `<ℓ> = 4V/S`. For a sphere
//!   of radius `r` this gives `4r/3`.
//! - Binary stochastic mixtures and the Markovian chord relation: Lux & Koblinger,
//!   *Monte Carlo Particle Transport Methods*, CRC Press (1991); Zimmerman & Adams,
//!   *Algorithms for Monte Carlo particle transport in binary statistical mixtures*
//!   (1991). See also [`super::references`] for the dispersion-fuel bibliography.
//!
//! This module is **new work**, not a port — OpenMC has no CLS implementation, so the
//! crate's "mirror the canonical source" rule does not apply here (see the crate
//! `CLAUDE.md`: new parts are scaffolded only where genuinely absent upstream).

use crate::geometry::position::Position;
use crate::pebble_beds::medium::{MaterialId, MediumError};
use crate::rng::lcg::prn;

/// Mean chord length \[cm\] through a sphere of radius `radius` \[cm\].
///
/// Cauchy's mean-chord result for a convex body is `<ℓ> = 4V/S`. For a sphere,
/// `V = (4/3)πr³` and `S = 4πr²`, so
///
/// ```text
/// <ℓ> = 4 · (4/3)πr³ / (4πr²) = 4r/3
/// ```
///
/// This is the mean distance a uniformly-and-isotropically incident ray spends inside
/// one inclusion, and it sets the inclusion-phase chord statistics for CLS.
///
/// `radius` must be > 0; a non-positive radius yields 0.
pub fn mean_chord_length_sphere(radius: f64) -> f64 {
    if radius <= 0.0 {
        return 0.0;
    }
    4.0 * radius / 3.0
}

/// Mean chord length \[cm\] through the *matrix* phase between spherical inclusions.
///
/// For a binary stochastic mixture the two phases' mean chords are tied to their volume
/// fractions by the Markovian relation `p_i = <ℓ_i> / (<ℓ_1> + <ℓ_2>)`, i.e.
/// `<ℓ_matrix> / <ℓ_incl> = p_matrix / p_incl`. With the inclusion phase occupying the
/// packing fraction `pf` and the matrix the remaining `1 - pf`:
///
/// ```text
/// <ℓ_matrix> = (4r/3) · (1 - pf) / pf
/// ```
///
/// So a sparse packing gives long matrix flights and a dense one gives short flights,
/// as expected.
///
/// # Parameters
/// - `radius` — inclusion radius \[cm\].
/// - `packing_fraction` — inclusion volume fraction, in (0, 1).
///
/// Returns [`f64::INFINITY`] when `packing_fraction` is 0 (no inclusions, so the
/// neutron never hits one) and 0 when it is >= 1 (no matrix to fly through).
pub fn matrix_mean_chord_length(radius: f64, packing_fraction: f64) -> f64 {
    if packing_fraction <= 0.0 {
        return f64::INFINITY;
    }
    if packing_fraction >= 1.0 {
        return 0.0;
    }
    mean_chord_length_sphere(radius) * (1.0 - packing_fraction) / packing_fraction
}

/// Sample a chord length \[cm\] from an exponential distribution with the given mean.
///
/// The Markovian assumption makes chord lengths exponentially distributed, so inverse
/// -transform sampling gives `ℓ = -<ℓ>·ln(ξ)` for `ξ` uniform on (0, 1]. `seed` is the
/// crate LCG stream ([`prn`]), advanced in place.
///
/// Uses the crate's OpenMC-derived LCG rather than `rand`/`rand_chacha`: the v0.1
/// design scaffold names those crates, but this crate's reproducibility guarantee
/// depends on per-particle LCG streams with O(log n) jump-ahead
/// ([`crate::rng::lcg::future_seed`]), which `rand_chacha` would break. Workspace and
/// crate rules take precedence over the design doc here.
///
/// A non-positive `mean_chord` yields 0.
pub fn sample_chord(mean_chord: f64, seed: &mut u64) -> f64 {
    if mean_chord <= 0.0 {
        return 0.0;
    }
    if !mean_chord.is_finite() {
        return f64::INFINITY;
    }
    // prn() returns [0, 1); shift off zero so ln() stays finite.
    let xi = 1.0 - prn(seed);
    -mean_chord * xi.ln()
}

/// A memoryless chord-length-sampled random medium.
///
/// Holds only the *statistics* of the packing — inclusion radius, packing fraction, and
/// the two phase materials — never the inclusions themselves. That is the whole point:
/// the struct is O(1) in memory regardless of how many inclusions the medium notionally
/// contains.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClsMedium {
    inclusion_radius: f64,
    packing_fraction: f64,
    inclusion: MaterialId,
    matrix: MaterialId,
}

impl ClsMedium {
    /// Build a CLS medium from packing statistics.
    ///
    /// - `inclusion_radius` — inclusion radius \[cm\], > 0.
    /// - `packing_fraction` — inclusion volume fraction, in (0, 1).
    /// - `inclusion` / `matrix` — the two phase materials.
    pub fn new(
        inclusion_radius: f64,
        packing_fraction: f64,
        inclusion: MaterialId,
        matrix: MaterialId,
    ) -> Self {
        Self {
            inclusion_radius,
            packing_fraction,
            inclusion,
            matrix,
        }
    }

    /// Inclusion radius \[cm\].
    pub fn inclusion_radius(&self) -> f64 {
        self.inclusion_radius
    }

    /// Inclusion volume (packing) fraction.
    pub fn packing_fraction(&self) -> f64 {
        self.packing_fraction
    }

    /// Material id of the inclusion phase.
    pub fn inclusion_material(&self) -> MaterialId {
        self.inclusion
    }

    /// Material id of the matrix phase.
    pub fn matrix_material(&self) -> MaterialId {
        self.matrix
    }

    /// Mean chord length \[cm\] through one inclusion — [`mean_chord_length_sphere`].
    pub fn mean_chord_inclusion(&self) -> f64 {
        mean_chord_length_sphere(self.inclusion_radius)
    }

    /// Mean chord length \[cm\] through the matrix — [`matrix_mean_chord_length`].
    pub fn mean_chord_matrix(&self) -> f64 {
        matrix_mean_chord_length(self.inclusion_radius, self.packing_fraction)
    }

    /// Sample the distance \[cm\] to the next inclusion boundary, given which phase the
    /// neutron is currently in.
    ///
    /// This is CLS's real query — the flight-level one. `in_inclusion` selects which
    /// phase's chord statistics to sample from.
    pub fn sample_distance_to_boundary(&self, in_inclusion: bool, seed: &mut u64) -> f64 {
        let mean = if in_inclusion {
            self.mean_chord_inclusion()
        } else {
            self.mean_chord_matrix()
        };
        sample_chord(mean, seed)
    }

    /// Point membership — **not implemented**, and not CLS's natural query.
    ///
    /// CLS reconstructs occupancy along a *flight*, not at an isolated point: with no
    /// stored geometry there is nothing to test a point against. Answering this
    /// properly means carrying the flight's phase state, which belongs to the CLS
    /// transport driver (bead `op-eby.2`). Returns
    /// [`MediumError::NotImplemented`] rather than guessing.
    pub fn material_at(
        &mut self,
        _position: Position,
        _seed: &mut u64,
    ) -> Result<MaterialId, MediumError> {
        Err(MediumError::NotImplemented("CLS"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cauchy's mean chord for a sphere is 4r/3.
    #[test]
    fn sphere_mean_chord_is_four_thirds_radius() {
        assert!((mean_chord_length_sphere(1.0) - 4.0 / 3.0).abs() < 1e-15);
        assert!((mean_chord_length_sphere(0.03) - 0.04).abs() < 1e-15);
        assert_eq!(mean_chord_length_sphere(0.0), 0.0);
    }

    /// The matrix chord follows the binary-Markovian ratio p_matrix/p_incl.
    #[test]
    fn matrix_mean_chord_follows_volume_fraction_ratio() {
        let r = 0.03;
        // At pf = 0.5 the phases are symmetric, so the chords are equal.
        let half = matrix_mean_chord_length(r, 0.5);
        assert!((half - mean_chord_length_sphere(r)).abs() < 1e-15);

        // At pf = 0.2 the matrix chord is 4x the inclusion chord ((1-0.2)/0.2 = 4).
        let sparse = matrix_mean_chord_length(r, 0.2);
        assert!((sparse - 4.0 * mean_chord_length_sphere(r)).abs() < 1e-15);

        // Degenerate limits.
        assert_eq!(matrix_mean_chord_length(r, 0.0), f64::INFINITY);
        assert_eq!(matrix_mean_chord_length(r, 1.0), 0.0);
    }

    /// Sampled chords are exponentially distributed: the sample mean converges to the
    /// requested mean. Uses the crate LCG so the result is reproducible.
    #[test]
    fn sampled_chords_have_the_requested_mean() {
        let mean = 0.04;
        let n = 200_000;
        let mut seed = 12_345u64;
        let mut sum = 0.0;
        for _ in 0..n {
            let c = sample_chord(mean, &mut seed);
            assert!(c >= 0.0, "chord must be non-negative");
            sum += c;
        }
        let sample_mean = sum / n as f64;
        // Standard error of an exponential mean is mean/sqrt(n); allow 5 sigma.
        let tol = 5.0 * mean / (n as f64).sqrt();
        assert!(
            (sample_mean - mean).abs() < tol,
            "sample mean {sample_mean} deviates from {mean} by more than {tol}"
        );
    }

    /// The flight-level query picks the right phase statistics.
    #[test]
    fn distance_sampling_selects_phase_statistics() {
        let m = ClsMedium::new(0.03, 0.2, MaterialId(1), MaterialId(0));
        assert!((m.mean_chord_inclusion() - 0.04).abs() < 1e-15);
        assert!((m.mean_chord_matrix() - 0.16).abs() < 1e-15);

        // Both phases must produce finite, non-negative distances.
        let mut seed = 7u64;
        for _ in 0..1000 {
            let d_in = m.sample_distance_to_boundary(true, &mut seed);
            let d_out = m.sample_distance_to_boundary(false, &mut seed);
            assert!(d_in.is_finite() && d_in >= 0.0);
            assert!(d_out.is_finite() && d_out >= 0.0);
        }
    }

    /// Point membership is honestly reported as unimplemented, not fabricated.
    #[test]
    fn material_at_reports_not_implemented() {
        let mut m = ClsMedium::new(0.03, 0.2, MaterialId(1), MaterialId(0));
        let mut seed = 1u64;
        assert_eq!(
            m.material_at(Position::new(0.0, 0.0, 0.0), &mut seed),
            Err(MediumError::NotImplemented("CLS"))
        );
    }
}
