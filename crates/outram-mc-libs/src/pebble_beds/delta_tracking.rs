//! Woodcock (delta) tracking for doubly-heterogeneous media.
//!
//! In a pebble-bed core a neutron flies past an enormous number of material
//! interfaces (TRISO kernel / buffer / IPyC / SiC / OPyC shells, matrix, pebble
//! surfaces). Surface tracking must find the *nearest* of all those boundaries at
//! every flight — ruinously expensive when there are O(10⁵) of them along a path.
//!
//! Delta tracking removes boundary crossings from the inner loop. Pick a
//! **majorant** cross section `Σ_maj(E) ≥ Σ_t(E)` for *every* material the neutron
//! could be in. Sample the flight distance on the majorant,
//! `s = −ln ξ / Σ_maj(E)`. At the landing point read the *local* material's true
//! `Σ_t`; accept a **real** collision with probability `Σ_t/Σ_maj`, otherwise the
//! event is a **virtual** (delta) collision — nothing physical happens and the
//! flight simply continues from the new point. The neutron never counts the
//! surfaces it flew over; it only ever queries "what material am I in *here*".
//!
//! The rejected (virtual) fraction is the price: a loose majorant means many
//! virtual collisions. For pebble beds the majorant is dominated by the strongly
//! absorbing/scattering fuel, so a material-wise majorant is usually tight enough.
//!
//! This module provides the tracking *primitives* — majorant construction, the
//! flight-distance sample, and the real/virtual decision — plus a
//! [`track_to_collision`] driver that composes them against a caller-supplied
//! "material at this point" lookup. The lookup is where the doubly-heterogeneous
//! geometry (lattices of TRISO universes, stochastic packings) plugs in.
//!
//! Reference: E. R. Woodcock et al., "Techniques used in the GEM code…", ANL-7050
//! (1965); the method is standard in modern MC codes (OpenMC `delta_tracking`,
//! Serpent, RMC). See also [`super::references`] for the pebble-bed geometry work.

use crate::geometry::position::{Direction, Position};
use crate::material::material::Material;
use crate::material::nuclide::Nuclide;
use crate::rng::lcg::prn;

/// An energy-dependent majorant cross section `Σ_maj(E) ≥ Σ_t(E)` over a set of
/// materials — the sampling bound for delta tracking.
///
/// Stored as a tabulated `(energy [eV], Σ_maj [cm⁻¹])` curve on an ascending grid.
/// [`Self::at`] returns a value that is conservative (never below the true
/// tabulated majorant) by taking the larger of the two bracketing grid points, so
/// a coarse grid stays valid — it just loosens the bound (more virtual collisions).
#[derive(Debug, Clone, Default)]
pub struct Majorant {
    /// Ascending energy grid \[eV\].
    energy: Vec<f64>,
    /// Majorant Σ_maj \[cm⁻¹\] at each grid energy.
    sigma: Vec<f64>,
}

impl Majorant {
    /// A single flat majorant `sigma_max` \[cm⁻¹\] valid at all energies.
    ///
    /// The simplest bound: one conservative constant. Correct as long as
    /// `sigma_max ≥ Σ_t` everywhere the neutron can travel; looser than an
    /// energy-dependent majorant, so it produces more virtual collisions.
    pub fn uniform(sigma_max: f64) -> Self {
        Majorant {
            energy: vec![0.0, f64::INFINITY],
            sigma: vec![sigma_max, sigma_max],
        }
    }

    /// Build the majorant `Σ_maj(E) = max_m Σ_t,m(E)` over `materials` on the
    /// supplied energy grid, with a small `margin` fraction added for safety.
    ///
    /// For each grid energy this takes the maximum macroscopic total over every
    /// material (each evaluated at its own temperature), then multiplies by
    /// `1 + margin` so floating-point round-off at the exact grid points can never
    /// let a real `Σ_t` slip above the bound. Pass `margin = 0.0` for the tight
    /// bound, or e.g. `0.01` for a 1 % cushion.
    ///
    /// # Parameters
    /// - `materials` — every material the neutron might enter (fuel, matrix, …).
    /// - `nuclides` — the global nuclide array the materials index into.
    /// - `energies` — ascending energy grid \[eV\] to tabulate the majorant on.
    /// - `margin` — non-negative safety fraction added to each majorant value.
    pub fn from_materials(
        materials: &[Material],
        nuclides: &[Nuclide],
        energies: &[f64],
        margin: f64,
    ) -> Self {
        let scale = 1.0 + margin.max(0.0);
        let sigma = energies
            .iter()
            .map(|&e| {
                let m = materials
                    .iter()
                    .map(|mat| mat.macro_xs_total(e, nuclides))
                    .fold(0.0_f64, f64::max);
                m * scale
            })
            .collect();
        Majorant {
            energy: energies.to_vec(),
            sigma,
        }
    }

    /// Build a **provably bounding** majorant by taking, for each energy bin, the
    /// maximum of `Σ_t` over a dense sub-sample of that bin — not just its
    /// endpoints.
    ///
    /// [`Self::from_materials`] evaluates `Σ_t` only at the grid *points*, so a
    /// resonance peak that falls *between* two points slips under the bound — fatal
    /// for delta tracking, whose whole contract is `Σ_maj ≥ Σ_t` **everywhere**
    /// (an under-bound biases the real/virtual split toward the higher-`Σ_t`
    /// material). This constructor instead lays a log grid of `n_bins` bins over
    /// `[e_min, e_max]` and, for each bin, evaluates the per-material macroscopic
    /// total at `subsamples` energies spanning the bin and keeps the largest. That
    /// bin maximum is written to **both** bin edges, so [`Self::at`]'s
    /// bracket-maximum returns a value `≥` the bin's true peak for any energy inside
    /// it. A final `1 + margin` cushion covers sub-bin structure narrower than the
    /// sampling.
    ///
    /// The cost is `n_bins · subsamples` cross-section evaluations at construction
    /// (once), in exchange for an unbiased flight. For resonance data (WMP / HIGH
    /// tier) prefer this over [`Self::from_materials`]; for smooth or group data the
    /// cheaper point sampler is adequate.
    ///
    /// # Parameters
    /// - `materials` / `nuclides` — every material the neutron might enter.
    /// - `e_min` / `e_max` — energy span \[eV\] to bound (cover the whole range the
    ///   histories visit — birth energy down to the lowest energy reached).
    /// - `n_bins` — number of log-spaced bins; more bins ⇒ tighter (fewer virtual
    ///   collisions), same bounding guarantee.
    /// - `subsamples` — sub-energies evaluated per bin (`≥ 2`); more ⇒ safer against
    ///   narrow resonances.
    /// - `margin` — non-negative safety fraction multiplying the final envelope.
    pub fn bounding(
        materials: &[Material],
        nuclides: &[Nuclide],
        e_min: f64,
        e_max: f64,
        n_bins: usize,
        subsamples: usize,
        margin: f64,
    ) -> Self {
        let n_bins = n_bins.max(1);
        let subsamples = subsamples.max(2);
        let scale = 1.0 + margin.max(0.0);
        let ln_lo = e_min.max(f64::MIN_POSITIVE).ln();
        let ln_hi = e_max.max(e_min * 1.0001).ln();
        let edge = |i: usize| (ln_lo + (ln_hi - ln_lo) * i as f64 / n_bins as f64).exp();

        let energy: Vec<f64> = (0..=n_bins).map(edge).collect();
        let sigma_t_max = |e: f64| {
            materials
                .iter()
                .map(|m| m.macro_xs_total(e, nuclides))
                .fold(0.0_f64, f64::max)
        };

        let mut sigma = vec![0.0_f64; n_bins + 1];
        for b in 0..n_bins {
            let (lo, hi) = (energy[b].ln(), energy[b + 1].ln());
            let mut bin_max = 0.0_f64;
            for s in 0..subsamples {
                let e = (lo + (hi - lo) * s as f64 / (subsamples - 1) as f64).exp();
                bin_max = bin_max.max(sigma_t_max(e));
            }
            // Write the bin peak to both edges so `at`'s bracket-max bounds the bin.
            sigma[b] = sigma[b].max(bin_max);
            sigma[b + 1] = sigma[b + 1].max(bin_max);
        }
        for s in &mut sigma {
            *s *= scale;
        }
        Majorant { energy, sigma }
    }

    /// The majorant Σ_maj \[cm⁻¹\] at energy `e` \[eV\] — conservative (takes the
    /// larger bracketing grid value so it never under-bounds between points).
    pub fn at(&self, e: f64) -> f64 {
        match self.sigma.len() {
            0 => 0.0,
            1 => self.sigma[0],
            _ => {
                // First grid point strictly above e; the bracket is [i-1, i].
                let i = self.energy.partition_point(|&g| g <= e);
                if i == 0 {
                    self.sigma[0]
                } else if i >= self.sigma.len() {
                    *self.sigma.last().unwrap()
                } else {
                    self.sigma[i - 1].max(self.sigma[i])
                }
            }
        }
    }
}

/// The outcome of one delta-tracking flight segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeltaEvent {
    /// A physical interaction — sample the actual reaction here.
    Real,
    /// A virtual (delta) collision — no physics; continue the flight.
    Virtual,
}

/// Sample a delta-tracking flight distance \[cm\] from an exponential on the
/// majorant: `s = −ln ξ / Σ_maj`.
///
/// This is the ordinary free-flight sample, but on the majorant rather than the
/// local Σ_t, which is what lets the neutron cross material boundaries without
/// stopping at them. `majorant` is Σ_maj \[cm⁻¹\] at the current energy; a
/// non-positive majorant yields an infinite flight (no collisions possible).
pub fn sample_delta_distance(majorant: f64, seed: &mut u64) -> f64 {
    if majorant <= 0.0 {
        return f64::INFINITY;
    }
    -prn(seed).max(f64::MIN_POSITIVE).ln() / majorant
}

/// Decide whether a delta-tracking collision is real or virtual by rejection on
/// the ratio `Σ_t(local)/Σ_maj`.
///
/// Accepts a [`DeltaEvent::Real`] with probability `sigma_t_local / majorant`
/// (the physical collision), else [`DeltaEvent::Virtual`]. `sigma_t_local` is the
/// true macroscopic total \[cm⁻¹\] of the material at the landing point;
/// `majorant` is the Σ_maj the flight was sampled on. A `majorant ≤ 0` degenerates
/// to `Virtual`. The caller must guarantee `sigma_t_local ≤ majorant` (that is the
/// majorant's whole contract); if it is violated the ratio saturates at 1 (always
/// real), which is the safe direction.
pub fn classify_collision(sigma_t_local: f64, majorant: f64, seed: &mut u64) -> DeltaEvent {
    if majorant <= 0.0 {
        return DeltaEvent::Virtual;
    }
    let p_real = (sigma_t_local / majorant).clamp(0.0, 1.0);
    if prn(seed) < p_real {
        DeltaEvent::Real
    } else {
        DeltaEvent::Virtual
    }
}

/// Where a delta-tracking flight ended.
#[derive(Debug, Clone, Copy)]
pub struct DeltaFlight {
    /// Position \[cm\] of the real collision (or of leakage — see `escaped`).
    pub position: Position,
    /// Total path length \[cm\] flown, including all virtual-collision segments.
    pub distance: f64,
    /// Number of virtual (delta) collisions rejected before the real one.
    pub virtual_collisions: u32,
    /// `true` if the neutron left the tracking region before a real collision
    /// (the `sigma_t_local` lookup returned `None`).
    pub escaped: bool,
}

/// Drive a neutron from `start` along `direction` to its next **real** collision by
/// delta tracking, looping over virtual collisions internally.
///
/// At each step it samples a flight on `majorant.at(energy)`, advances, then asks
/// the caller "what is Σ_t at this point?" via `sigma_t_at`. That closure returns
/// `Some(sigma_t)` for a point inside the tracking region (looking up whichever
/// material — fuel kernel, matrix, pebble, coolant — actually occupies the point)
/// or `None` if the neutron has left the region (leakage). A real collision ends
/// the loop; a virtual one continues it.
///
/// This is deliberately generic over the geometry lookup (`impl Fn`, no trait
/// object) so the doubly-heterogeneous machinery — lattice/universe descent or a
/// stochastic-media membership test — supplies `sigma_t_at` without this core
/// depending on it.
///
/// # Parameters
/// - `start` / `direction` — the neutron's phase-space point.
/// - `energy` — incident energy \[eV\] (constant along the flight; scattering
///   changes it *after* a real collision, in the caller's transport loop).
/// - `majorant` — the delta-tracking bound (see [`Majorant`]).
/// - `max_virtual` — safety cap on virtual collisions before giving up (returns
///   `escaped = true`); guards against a pathologically loose majorant.
/// - `sigma_t_at` — local total Σ_t \[cm⁻¹\] lookup, `None` outside the region.
pub fn track_to_collision<F>(
    start: Position,
    direction: Direction,
    energy: f64,
    majorant: &Majorant,
    max_virtual: u32,
    seed: &mut u64,
    sigma_t_at: F,
) -> DeltaFlight
where
    F: Fn(Position) -> Option<f64>,
{
    let maj = majorant.at(energy);
    let mut pos = start;
    let mut total = 0.0;
    let mut virtuals = 0u32;

    loop {
        let s = sample_delta_distance(maj, seed);
        pos = pos + Position::new(direction.u * s, direction.v * s, direction.w * s);
        total += s;

        match sigma_t_at(pos) {
            None => {
                return DeltaFlight {
                    position: pos,
                    distance: total,
                    virtual_collisions: virtuals,
                    escaped: true,
                };
            }
            Some(sigma_t) => match classify_collision(sigma_t, maj, seed) {
                DeltaEvent::Real => {
                    return DeltaFlight {
                        position: pos,
                        distance: total,
                        virtual_collisions: virtuals,
                        escaped: false,
                    };
                }
                DeltaEvent::Virtual => {
                    virtuals += 1;
                    if virtuals >= max_virtual {
                        return DeltaFlight {
                            position: pos,
                            distance: total,
                            virtual_collisions: virtuals,
                            escaped: true,
                        };
                    }
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The majorant must bound every material's Σ_t at every tabulated energy —
    /// the correctness precondition of delta tracking. Uses the CORE-data Godiva
    /// uranium mix plus a light "matrix" so the max is non-trivially one material.
    #[test]
    fn majorant_bounds_every_material() {
        use crate::material::material::NuclideComponent;
        let nuclides = vec![
            Nuclide::from_core("U235").unwrap(),
            Nuclide::from_core("H1").unwrap(),
        ];
        let fuel = Material {
            id: 1,
            name: "fuel".into(),
            temperature: 293.6,
            components: vec![NuclideComponent {
                nuclide_idx: 0,
                atom_density: 4.8e-2,
            }],
        };
        let matrix = Material {
            id: 2,
            name: "matrix".into(),
            temperature: 293.6,
            components: vec![NuclideComponent {
                nuclide_idx: 1,
                atom_density: 6.0e-2,
            }],
        };
        let mats = [fuel, matrix];
        let grid: Vec<f64> = (0..40).map(|i| 1.0e3 * 1.4_f64.powi(i)).collect();
        let maj = Majorant::from_materials(&mats, &nuclides, &grid, 0.01);

        for &e in &grid {
            let m = maj.at(e);
            for mat in &mats {
                let st = mat.macro_xs_total(e, &nuclides);
                assert!(m >= st, "majorant {m} < Σ_t {st} at {e} eV");
            }
        }
    }

    /// Woodcock tracking in a homogeneous medium must reproduce the true collision
    /// density regardless of how loose the majorant is: the first real collision
    /// distance is exponentially distributed with mean 1/Σ_t. Here Σ_t = 0.5 cm⁻¹
    /// (mean free path 2 cm) tracked under a 4× majorant.
    #[test]
    fn delta_tracking_recovers_true_mean_free_path() {
        let sigma_t = 0.5_f64;
        let maj = Majorant::uniform(4.0 * sigma_t); // deliberately loose
        let mut seed = 0x1234_5678u64;
        let n = 200_000usize;
        let mut sum = 0.0;
        let mut virtual_total = 0u64;
        for _ in 0..n {
            let f = track_to_collision(
                Position::new(0.0, 0.0, 0.0),
                Direction::new(1.0, 0.0, 0.0),
                1.0e6,
                &maj,
                10_000,
                &mut seed,
                |_p| Some(sigma_t), // homogeneous, infinite medium
            );
            assert!(!f.escaped);
            sum += f.distance;
            virtual_total += f.virtual_collisions as u64;
        }
        let mean = sum / n as f64;
        let expected = 1.0 / sigma_t; // = 2 cm
        assert!(
            (mean - expected).abs() < 0.03,
            "delta-tracked mean free path {mean} ≠ 1/Σ_t {expected}"
        );
        // With a 4× majorant ~3 of every 4 collisions should be virtual.
        let virtual_frac = virtual_total as f64 / (virtual_total as f64 + n as f64);
        assert!(
            (virtual_frac - 0.75).abs() < 0.02,
            "virtual fraction {virtual_frac} ≠ 1 − Σ_t/Σ_maj = 0.75"
        );
    }

    /// The real/virtual split matches the ratio Σ_t/Σ_maj over many draws.
    #[test]
    fn classify_matches_ratio() {
        let mut seed = 99u64;
        let (sigma_t, maj) = (0.3_f64, 1.0_f64);
        let n = 100_000;
        let reals = (0..n)
            .filter(|_| classify_collision(sigma_t, maj, &mut seed) == DeltaEvent::Real)
            .count();
        let frac = reals as f64 / n as f64;
        assert!((frac - 0.3).abs() < 0.01, "real fraction {frac} ≠ 0.3");
    }
}
