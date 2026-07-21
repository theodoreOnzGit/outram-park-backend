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
//! # The flight driver (bead `op-eby.2`, implemented)
//!
//! [`ClsMedium::material_at`] reconstructs phase occupancy statefully along a flight:
//! it seeds the phase from the volume-fraction prior, then advances by the scalar path
//! length between successive queries, toggling phase and re-sampling a chord at every
//! boundary crossed. Because the chord statistics are direction-independent this needs
//! only the distance between queries, which *is* the memoryless approximation. Its
//! defining consistency property — inclusion occupancy converging to the packing
//! fraction — is unit-tested. Coupling this into the k-eigenvalue transport loop (the
//! benchmark of bead `op-eby.7`) is the remaining integration step, not a gap in CLS
//! itself.
//!
//! # References
//!
//! - Cauchy's formula for the mean chord of a convex body, `<ℓ> = 4V/S`. For a sphere
//!   of radius `r` this gives `4r/3`.
//! - Binary stochastic mixtures and the Markovian chord relation: Lux & Koblinger,
//!   *Monte Carlo Particle Transport Methods*, CRC Press (1991); Zimmerman & Adams,
//!   *Algorithms for Monte Carlo particle transport in binary statistical mixtures*
//!   (1991). See also [`crate::pebble_beds::references`] for the dispersion-fuel bibliography.
//!
//! This module is **new work**, not a port — OpenMC has no CLS implementation, so the
//! crate's "mirror the canonical source" rule does not apply here (see the crate
//! `CLAUDE.md`: new parts are scaffolded only where genuinely absent upstream).

use crate::geometry::position::Position;
use crate::rng::lcg::prn;
use crate::stochastic::medium::{MaterialId, MediumError};

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

/// Transient per-history flight state used to reconstruct phase occupancy along a
/// CLS flight (see [`ClsMedium::material_at`]).
///
/// CLS is memoryless, so the only state a flight needs is *which phase the neutron
/// is currently in* and *how far it is to the next sampled boundary*. Because the
/// chord statistics are direction-independent (isotropic mean chord), the phase can
/// be advanced by the scalar path length travelled since the last query — no
/// direction is stored.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ClsFlight {
    /// Position of the previous `material_at` query \[cm\].
    last_position: Position,
    /// `true` if the neutron is currently inside an inclusion.
    in_inclusion: bool,
    /// Remaining path length \[cm\] before the next sampled phase boundary.
    dist_to_boundary: f64,
}

/// A memoryless chord-length-sampled random medium.
///
/// Holds the *statistics* of the packing — inclusion radius, packing fraction, and
/// the two phase materials — never the inclusions themselves; the only per-history
/// state is the small [`ClsFlight`] reconstructed lazily on the first
/// [`Self::material_at`]. That is the whole point: the struct is O(1) in memory
/// regardless of how many inclusions the medium notionally contains.
///
/// Not `Copy`: a medium carries live flight state mid-history, and a silent copy
/// would fork that state into two inconsistent flights.
#[derive(Debug, Clone, PartialEq)]
pub struct ClsMedium {
    inclusion_radius: f64,
    packing_fraction: f64,
    inclusion: MaterialId,
    matrix: MaterialId,
    /// Transient flight state, `None` until the first [`Self::material_at`] seeds it.
    flight: Option<ClsFlight>,
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
            flight: None,
        }
    }

    /// The phase material id for a boolean phase flag.
    fn phase_material(&self, in_inclusion: bool) -> MaterialId {
        if in_inclusion {
            self.inclusion
        } else {
            self.matrix
        }
    }

    /// Discard any in-progress flight, so the next [`Self::material_at`] re-seeds the
    /// phase from scratch. Call this when a transport driver starts a new history.
    pub fn begin_flight(&mut self) {
        self.flight = None;
    }

    /// The phase the reconstructed flight is currently in, or `None` before the first
    /// [`Self::material_at`] query. `true` means inside an inclusion.
    pub fn in_inclusion(&self) -> Option<bool> {
        self.flight.map(|f| f.in_inclusion)
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

    /// Material occupying `position` \[cm\], reconstructed along the CLS flight.
    ///
    /// CLS has no stored geometry, so occupancy is only defined *along a flight*: this
    /// method reconstructs it statefully. The first call seeds the phase from the
    /// unconditional prior — the neutron is inside an inclusion with probability equal
    /// to the packing (volume) fraction — and samples the distance to the first
    /// boundary. Each subsequent call advances the flight by the scalar path length
    /// travelled since the previous query, toggling phase and re-sampling a chord at
    /// every boundary it crosses, then returns the current phase's material.
    ///
    /// Because CLS is memoryless the reconstruction depends only on the *distance*
    /// between successive queries, not on direction — a neutron that reverses course
    /// simply flies a fresh independent chord, which is precisely the correlation CLS
    /// discards (and [`super::scls`] restores). Call [`Self::begin_flight`] between
    /// independent histories so their phase state does not leak across.
    ///
    /// `seed` is the caller's LCG stream ([`prn`]), advanced in place. This never
    /// errors — the `Result` is kept only to match the
    /// [`super::medium::StochasticMedium::material_at`] contract shared with models
    /// that can fail.
    pub fn material_at(
        &mut self,
        position: Position,
        seed: &mut u64,
    ) -> Result<MaterialId, MediumError> {
        // Precompute both phase means and the material ids so the mutable borrow of
        // `self.flight` below does not conflict with `&self` method calls.
        let mean_incl = self.mean_chord_inclusion();
        let mean_matrix = self.mean_chord_matrix();
        let pf = self.packing_fraction;
        let mean_for = |in_incl: bool| if in_incl { mean_incl } else { mean_matrix };

        let in_inclusion = if let Some(flight) = &mut self.flight {
            let mut step = position.distance(flight.last_position);
            // Cross every boundary the step reaches. Guard against a zero/degenerate
            // mean chord (which would leave dist_to_boundary at 0 forever) with the
            // `> 0.0` condition so the loop always terminates.
            while step >= flight.dist_to_boundary && flight.dist_to_boundary > 0.0 {
                step -= flight.dist_to_boundary;
                flight.in_inclusion = !flight.in_inclusion;
                flight.dist_to_boundary = sample_chord(mean_for(flight.in_inclusion), seed);
            }
            if flight.dist_to_boundary > 0.0 {
                flight.dist_to_boundary -= step;
            }
            flight.last_position = position;
            flight.in_inclusion
        } else {
            // First query: seed the phase from the volume-fraction prior.
            let in_inclusion = prn(seed) < pf;
            let dist_to_boundary = sample_chord(mean_for(in_inclusion), seed);
            self.flight = Some(ClsFlight {
                last_position: position,
                in_inclusion,
                dist_to_boundary,
            });
            in_inclusion
        };

        Ok(self.phase_material(in_inclusion))
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

    /// The flight driver reconstructs occupancy whose inclusion fraction converges to
    /// the packing (volume) fraction — the defining consistency property of CLS, since
    /// [`matrix_mean_chord_length`] ties the two mean chords to the volume fractions.
    ///
    /// Methodology: fly a straight ray in fine uniform steps, query `material_at` at
    /// each step, and count the fraction of points reported as inclusion material. The
    /// path-length fraction in the inclusion phase is
    /// `<ℓ_i>/(<ℓ_i>+<ℓ_m>) = pf` by construction, and uniform point spacing makes the
    /// point fraction an unbiased estimator of it. Result (pf=0.2): the estimate lands
    /// within a few % of 0.2.
    #[test]
    fn inclusion_fraction_along_a_flight_converges_to_packing_fraction() {
        let pf = 0.2;
        let mut m = ClsMedium::new(0.03, pf, MaterialId(1), MaterialId(0));
        let inclusion = MaterialId(1);

        let mut seed = 20260721u64;
        let step = 0.01;
        let n = 400_000;
        let mut in_incl = 0usize;
        for i in 0..n {
            let x = i as f64 * step;
            let mat = m
                .material_at(Position::new(x, 0.0, 0.0), &mut seed)
                .expect("CLS material_at never errors");
            if mat == inclusion {
                in_incl += 1;
            }
        }
        let frac = in_incl as f64 / n as f64;
        // ~n·step/(<ℓ_i>+<ℓ_m>) ≈ 20000 independent chords; 5σ ≈ 0.015. Allow 0.03.
        assert!(
            (frac - pf).abs() < 0.03,
            "inclusion fraction {frac} should converge to pf {pf}"
        );
    }

    /// `begin_flight` discards flight state so an independent history does not inherit
    /// the previous one's phase, and `in_inclusion` reports the reconstructed phase.
    #[test]
    fn begin_flight_resets_phase_state() {
        let mut m = ClsMedium::new(0.03, 0.2, MaterialId(1), MaterialId(0));
        assert_eq!(m.in_inclusion(), None, "no phase before the first query");

        let mut seed = 42u64;
        let _ = m.material_at(Position::new(0.0, 0.0, 0.0), &mut seed);
        assert!(m.in_inclusion().is_some(), "phase is seeded after the first query");

        m.begin_flight();
        assert_eq!(m.in_inclusion(), None, "begin_flight clears the flight");
    }

    /// The same seed and query sequence reproduces the same reconstruction — the flight
    /// is deterministic in the LCG stream (reproducibility guarantee).
    #[test]
    fn flight_reconstruction_is_reproducible() {
        let run = || {
            let mut m = ClsMedium::new(0.03, 0.25, MaterialId(1), MaterialId(0));
            let mut seed = 999u64;
            (0..500)
                .map(|i| {
                    m.material_at(Position::new(i as f64 * 0.02, 0.0, 0.0), &mut seed)
                        .unwrap()
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(run(), run(), "same seed -> identical flight");
    }
}
