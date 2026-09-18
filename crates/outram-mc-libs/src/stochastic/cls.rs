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
//! fraction — is unit-tested.
//!
//! ~~Coupling this into the k-eigenvalue transport loop is the remaining
//! integration step~~ **CORRECTED 2026-09-18** — it is coupled.
//! [`crate::dh_universe::DhTreatment::ChordLength`] drives this medium through the
//! full FHR unit-cell k-eigenvalue calculation, and `examples/dh_keff_vv.rs`
//! publishes the resulting eigenvalue.
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
//! **Those two references describe different media, and conflating them caused a
//! real defect.** The Markovian binary mixture of Zimmerman & Adams has
//! *exponentially* distributed chords in **both** phases. Dispersion fuel does not:
//! its inclusions are spheres of **fixed radius**, whose chord law is
//! `f(l) = l / (2R^2)` on `[0, 2R]` — same mean, bounded support, about a third
//! the relative spread. The matrix phase is still treated as Markovian here, which
//! is standard; the inclusion phase is not, and is sampled by
//! [`sample_chord_sphere`].
//!
//! The SCLS method the sibling module implements is Tan, Feng, Chan & Wang (2025),
//! `10.1016/j.anucene.2025.111436`
//! ([`crate::pebble_beds::references::TAN2025_CLS`]). **That paper is not yet
//! catalogued in `crates/kovan-literature`**, which the workspace requires of any
//! literature that informs the code.
//!
//! This module is **new work**, not a port — OpenMC has no CLS implementation, so the
//! crate's "mirror the canonical source" rule does not apply here (see the crate
//! `CLAUDE.md`: new parts are scaffolded only where genuinely absent upstream).

use crate::geometry::position::Position;
use crate::rng::lcg::prn;
use crate::stochastic::medium::{MaterialId, MediumError};
use crate::mathf::RealMath;

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
    -mean_chord * xi.r_ln()
}

/// Sample a chord \[cm\] through a **sphere** of radius `radius`, from the true
/// geometric chord-length distribution.
///
/// # Why this is not exponential
///
/// For a convex body under uniform isotropic incidence the chord length is *not*
/// exponentially distributed. For a sphere the impact parameter `b` is uniform in
/// area, so `b = R·√ξ`, and the chord is the corresponding secant:
///
/// ```text
/// ℓ = 2·√(R² − b²) = 2R·√(1 − ξ)
/// ```
///
/// which for `ξ` uniform on [0,1) is the same law as `ℓ = 2R·√ξ`. The density is
/// `f(ℓ) = ℓ / (2R²)` on `0 ≤ ℓ ≤ 2R`, whose mean is `4R/3` — Cauchy's result, so
/// this agrees with [`mean_chord_length_sphere`] — with standard deviation
/// `√(2R²/9)` ≈ `0.471R`, i.e. `σ/⟨ℓ⟩ = √2/4 ≈ 0.354`.
///
/// # The defect this replaces — measured 2026-09-18
///
/// ~~Inclusion chords were drawn from [`sample_chord`], i.e. exponentially~~
/// **CORRECTED**. The exponential is the right law for a *Markovian binary
/// mixture* (Zimmerman & Adams), which is what this module's references
/// describe, but the inclusions here are **spheres of fixed radius**, which is
/// the non-Markovian case. The exponential preserves the mean — which is exactly
/// why the unit tests passed — and gets everything else wrong:
///
/// | | mean | σ/⟨ℓ⟩ | `P(ℓ > 2R)` | max/2R |
/// |---|---|---|---|---|
/// | ray-traced sphere (truth) | `4R/3` | **0.353** | **0** | **1.00** |
/// | exponential (previous code) | `4R/3` | 1.000 | **0.223** | 8.01 |
///
/// **22 % of sampled inclusion chords exceeded `2R`, the longest chord a sphere
/// has**, and the longest sampled was eight diameters. Measured over 400 000
/// samples at `R = 0.02135 cm`.
///
/// The consequence is under-absorption, because `1 − e^{−Σℓ}` is concave in `ℓ`,
/// so by Jensen a higher-variance chord distribution at the same mean absorbs
/// less. At `Σ⟨ℓ⟩ ≈ 2.8` — the resonance regime for a TRISO kernel — the mean
/// absorption probability per traversal was **0.740 against a true 0.899**.
///
/// `radius` must be > 0; a non-positive radius yields 0.
pub fn sample_chord_sphere(radius: f64, seed: &mut u64) -> f64 {
    if radius <= 0.0 {
        return 0.0;
    }
    // prn() is [0, 1); sqrt of it is the uniform-in-area impact parameter.
    // `sqrt` is IEEE-exact and stays the inherent method, per `mathf`: only
    // transcendentals are routed through PETIR for cross-platform bit-stability.
    2.0 * radius * prn(seed).sqrt()
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
        if in_inclusion {
            // A sphere's chord, not an exponential — see `sample_chord_sphere`.
            sample_chord_sphere(self.inclusion_radius, seed)
        } else {
            // The matrix phase IS modelled as Markovian, so exponential is right.
            sample_chord(self.mean_chord_matrix(), seed)
        }
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
        let radius = self.inclusion_radius;
        let mean_matrix = self.mean_chord_matrix();
        let pf = self.packing_fraction;
        // The two phases obey different laws: a sphere's chord inside an
        // inclusion, an exponential in the Markovian matrix.
        let sample_for = |in_incl: bool, seed: &mut u64| {
            if in_incl {
                sample_chord_sphere(radius, seed)
            } else {
                sample_chord(mean_matrix, seed)
            }
        };

        let in_inclusion = if let Some(flight) = &mut self.flight {
            let mut step = position.distance(flight.last_position);
            // Cross every boundary the step reaches. Guard against a zero/degenerate
            // mean chord (which would leave dist_to_boundary at 0 forever) with the
            // `> 0.0` condition so the loop always terminates.
            while step >= flight.dist_to_boundary && flight.dist_to_boundary > 0.0 {
                step -= flight.dist_to_boundary;
                flight.in_inclusion = !flight.in_inclusion;
                flight.dist_to_boundary = sample_for(flight.in_inclusion, seed);
            }
            if flight.dist_to_boundary > 0.0 {
                flight.dist_to_boundary -= step;
            }
            flight.last_position = position;
            flight.in_inclusion
        } else {
            // First query: seed the phase from the volume-fraction prior.
            let in_inclusion = prn(seed) < pf;
            let dist_to_boundary = sample_for(in_inclusion, seed);
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

    /// A sphere's chord can never exceed its diameter.
    ///
    /// **This is the test the previous implementation failed.** Inclusion chords
    /// were drawn exponentially, which preserves the mean `4R/3` — so the
    /// mean-only test below passed — while putting **22 % of samples beyond
    /// `2R`**, the longest chord a sphere has, out to eight diameters.
    #[test]
    fn sphere_chords_never_exceed_the_diameter() {
        let r = 0.02135;
        let mut seed = 20260918_u64;
        let mut worst = 0.0_f64;
        for _ in 0..200_000 {
            let l = sample_chord_sphere(r, &mut seed);
            assert!(
                l >= 0.0 && l <= 2.0 * r + 1e-15,
                "sampled chord {l} outside [0, 2R] for R = {r}"
            );
            worst = worst.max(l);
        }
        // and it must actually reach near the diameter, or the sampler is not
        // covering the distribution
        assert!(
            worst > 1.99 * r,
            "longest of 200 000 chords was {worst}, barely under 2R = {} — the \
             sampler is not reaching the full range",
            2.0 * r
        );
        assert_eq!(sample_chord_sphere(0.0, &mut seed), 0.0);
    }

    /// The sphere chord law reproduces BOTH moments, not just the mean.
    ///
    /// `f(l) = l / (2R^2)` on `[0, 2R]` has mean `4R/3` (Cauchy) and standard
    /// deviation `sqrt(2)R/3`, i.e. `sigma/mean = sqrt(2)/4 ~= 0.3536`. The
    /// exponential this replaced has `sigma/mean = 1` exactly — the single
    /// number that distinguishes them at equal mean.
    #[test]
    fn sphere_chord_matches_the_analytic_first_and_second_moments() {
        let r = 0.02135;
        let n = 400_000;
        let mut seed = 424242_u64;
        let (mut sum, mut sum_sq) = (0.0_f64, 0.0_f64);
        for _ in 0..n {
            let l = sample_chord_sphere(r, &mut seed);
            sum += l;
            sum_sq += l * l;
        }
        let mean = sum / n as f64;
        let var = sum_sq / n as f64 - mean * mean;
        let sd = var.sqrt();

        let mean_exact = 4.0 * r / 3.0;
        // E[l^2] = int_0^2R l^2 * l/(2R^2) dl = 2R^2  =>  var = 2R^2 - (4R/3)^2
        let sd_exact = (2.0 * r * r - mean_exact * mean_exact).sqrt();

        let tol = 5.0 * sd / (n as f64).sqrt();
        assert!(
            (mean - mean_exact).abs() < tol,
            "mean {mean} vs Cauchy {mean_exact}"
        );
        assert!(
            (sd - sd_exact).abs() < 0.02 * sd_exact,
            "sd {sd} vs analytic {sd_exact} (sigma/mean {:.4}, expected {:.4}); \
             an exponential would give sigma/mean = 1",
            sd / mean,
            sd_exact / mean_exact
        );
    }

    /// The sampler reproduces an actual **ray trace through a real sphere** —
    /// so the geometry is checked, not only the algebra.
    ///
    /// # Methodology
    ///
    /// A previous version of this test claimed to ray-trace but did not: it
    /// drew `b = R*sqrt(u)` and evaluated the secant `2*sqrt(R^2 - b^2)`, which
    /// is the *same* closed form the sampler uses, rearranged. Since `1 - u` is
    /// uniform when `u` is, `2*sqrt(R^2 - R^2 u) = 2R*sqrt(1-u)` is literally
    /// `sample_chord_sphere`'s own law — so it could only ever have confirmed
    /// that two spellings of one formula agree. It would have passed even if
    /// the formula itself were the wrong one for a sphere.
    ///
    /// This version constructs the geometry instead:
    ///
    /// 1. **Entry point** `P` uniform on the surface of a sphere of radius `R`
    ///    (`z` uniform on `[-1,1]`, azimuth uniform — Archimedes' hat-box).
    /// 2. **Direction** `d` drawn **cosine-weighted about the inward normal**,
    ///    in a basis built from that normal. Cosine weighting is what "uniform
    ///    isotropic incidence" means for a convex body, and it is the condition
    ///    under which Cauchy's `<l> = 4V/S = 4R/3` holds.
    /// 3. **Exit** found by solving the ray-sphere quadratic
    ///    `|P + t d|^2 = R^2` numerically, with `c = |P|^2 - R^2` computed
    ///    rather than assumed zero, and the chord taken as the Euclidean
    ///    distance between the two intersection points.
    ///
    /// Nowhere does this path evaluate `2R*sqrt(xi)`, so agreement is a real
    /// check. Compared by **quantile**, which is sensitive to the whole shape
    /// rather than to the first two moments — and the first moment is exactly
    /// what the exponential got right while being wrong everywhere else.
    ///
    /// Pass criterion: every compared quantile within 2 % of `2R`.
    ///
    /// # Result (2026-09-18)
    ///
    /// Agrees at all six quantiles. The worst deviation is well inside the 2 %
    /// bar, and the ray trace independently reproduces `sigma/<l> ~ 0.354` and
    /// `P(l > 2R) = 0` — the two statistics the exponential got wrong by a
    /// factor of three and by 22 percentage points respectively.
    #[test]
    fn sphere_chord_matches_a_ray_trace_of_a_real_sphere() {
        let r = 0.02135;
        let n = 200_000;
        let mut seed = 987_654_u64;

        let mut sampled: Vec<f64> = (0..n).map(|_| sample_chord_sphere(r, &mut seed)).collect();

        // Genuine ray trace: build the geometry, then intersect it.
        let mut traced: Vec<f64> = Vec::with_capacity(n);
        for _ in 0..n {
            // 1. Entry point uniform on the sphere's surface.
            let cz = 2.0 * prn(&mut seed) - 1.0;
            let sz = (1.0 - cz * cz).max(0.0).sqrt();
            let phi = 2.0 * std::f64::consts::PI * prn(&mut seed);
            let p = [r * sz * phi.cos(), r * sz * phi.sin(), r * cz];

            // Inward normal, and an orthonormal basis around it.
            let nrm = [-p[0] / r, -p[1] / r, -p[2] / r];
            // Pick a seed axis that is not parallel to the normal.
            let helper = if nrm[0].abs() < 0.9 {
                [1.0, 0.0, 0.0]
            } else {
                [0.0, 1.0, 0.0]
            };
            let mut t1 = [
                helper[1] * nrm[2] - helper[2] * nrm[1],
                helper[2] * nrm[0] - helper[0] * nrm[2],
                helper[0] * nrm[1] - helper[1] * nrm[0],
            ];
            let t1n = (t1[0] * t1[0] + t1[1] * t1[1] + t1[2] * t1[2]).sqrt();
            t1 = [t1[0] / t1n, t1[1] / t1n, t1[2] / t1n];
            let t2 = [
                nrm[1] * t1[2] - nrm[2] * t1[1],
                nrm[2] * t1[0] - nrm[0] * t1[2],
                nrm[0] * t1[1] - nrm[1] * t1[0],
            ];

            // 2. Cosine-weighted inward direction: cos(theta) = sqrt(xi).
            let cos_t = prn(&mut seed).sqrt();
            let sin_t = (1.0 - cos_t * cos_t).max(0.0).sqrt();
            let psi = 2.0 * std::f64::consts::PI * prn(&mut seed);
            let d = [
                sin_t * psi.cos() * t1[0] + sin_t * psi.sin() * t2[0] + cos_t * nrm[0],
                sin_t * psi.cos() * t1[1] + sin_t * psi.sin() * t2[1] + cos_t * nrm[1],
                sin_t * psi.cos() * t1[2] + sin_t * psi.sin() * t2[2] + cos_t * nrm[2],
            ];

            // 3. Solve |P + t d|^2 = R^2 for the far root.
            let a = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
            let b = 2.0 * (p[0] * d[0] + p[1] * d[1] + p[2] * d[2]);
            let c = p[0] * p[0] + p[1] * p[1] + p[2] * p[2] - r * r;
            let disc = (b * b - 4.0 * a * c).max(0.0);
            let t_exit = (-b + disc.sqrt()) / (2.0 * a);

            // Chord = distance between the two intersection points.
            let exit = [
                p[0] + t_exit * d[0],
                p[1] + t_exit * d[1],
                p[2] + t_exit * d[2],
            ];
            let dx = exit[0] - p[0];
            let dy = exit[1] - p[1];
            let dz = exit[2] - p[2];
            traced.push((dx * dx + dy * dy + dz * dz).sqrt());
        }

        // No traced chord may exceed the diameter -- a check on the trace itself,
        // so a broken reference cannot silently validate a broken sampler.
        let longest = traced.iter().copied().fold(0.0_f64, f64::max);
        assert!(
            longest <= 2.0 * r * (1.0 + 1.0e-9),
            "the ray trace itself is wrong: longest chord {longest} exceeds 2R = {}",
            2.0 * r
        );

        sampled.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
        traced.sort_by(|a, b| a.partial_cmp(b).expect("finite"));

        for q in [0.05, 0.25, 0.50, 0.75, 0.95, 0.99] {
            let i = ((n - 1) as f64 * q) as usize;
            let (a, b) = (sampled[i], traced[i]);
            assert!(
                (a - b).abs() < 0.02 * (2.0 * r),
                "quantile {q}: sampled {a} vs ray-traced {b} (tol 2 % of 2R)"
            );
        }
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
        assert!(
            m.in_inclusion().is_some(),
            "phase is seeded after the first query"
        );

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
