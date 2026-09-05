//! **Fixed-source** Monte Carlo transport — an external neutron source driving
//! a (sub-critical or non-multiplying) system, scoring track-length tallies.
//!
//! # New orchestration over the ported collision loop
//!
//! Per the crate porting rule, the *physics* here is **not** reinvented: every
//! flight, collision, reaction and boundary crossing is the already-ported
//! [`transport_history`](crate::physics::transport_csg::transport_history) (the
//! translation of OpenMC `src/physics.cpp`). What is new — and marked as new,
//! not a port — is the fixed-source **orchestration** around it: sample source
//! particles from an external [`FixedSource`], transport each to death, and
//! transport any fission secondaries they produce (sub-critical multiplication)
//! until the bank drains. There is no fission-source power iteration and no
//! `k_eff`; the result is the flux/reaction tally the source induces.
//!
//! This complements [`crate::physics::keff`] (k-eigenvalue / criticality) with
//! the second canonical Monte Carlo mode — **shielding / detector-response**
//! style problems (attenuation, leakage, flux far from a source).
//!
//! # Scope
//!
//! Analog transport (no variance reduction). Fission neutrons are tracked as
//! secondaries with a per-source-particle safety cap, so a **sub-critical**
//! (`k < 1`) or non-multiplying system converges; a super-critical system would
//! multiply without bound and is capped (and physically meaningless for a fixed
//! source). Neutron-only, offline data. See the workspace `RESPONSIBLE_USE.md`.

use crate::geometry::geometry::Geometry;
use crate::geometry::position::{Direction, Position};
use crate::material::material::Material;
use crate::material::nuclide::Nuclide;
use crate::physics::transport_csg::{transport_history, Site};
use crate::rng::distributions::isotropic_direction;
use crate::rng::lcg::prn;
use crate::tally::scoring::flush_batch;
use crate::tally::tally::Tally;

/// An external neutron source for a fixed-source run. Isotropic in direction;
/// mono-energetic in energy (the common shielding/detector case). Position is
/// either a point or uniform in an axis-aligned box.
#[derive(Debug, Clone, Copy)]
pub enum FixedSource {
    /// Isotropic point source at `r` \[cm\] emitting neutrons of energy
    /// `energy_ev` \[eV\].
    Point {
        /// Emission point \[cm\].
        r: Position,
        /// Emission energy \[eV\].
        energy_ev: f64,
    },
    /// Isotropic source sampled uniformly in the box `[lower, upper]` \[cm\],
    /// energy `energy_ev` \[eV\].
    Box {
        /// Lower corner \[cm\].
        lower: Position,
        /// Upper corner \[cm\].
        upper: Position,
        /// Emission energy \[eV\].
        energy_ev: f64,
    },
}

impl FixedSource {
    /// Sample one source neutron (position + isotropic direction + energy).
    fn sample(&self, seed: &mut u64) -> Site {
        let (dx, dy, dz) = isotropic_direction(seed);
        let u = Direction::new(dx, dy, dz);
        match *self {
            FixedSource::Point { r, energy_ev } => Site::new(r, u, energy_ev),
            FixedSource::Box {
                lower,
                upper,
                energy_ev,
            } => {
                let r = Position::new(
                    lower.x + prn(seed) * (upper.x - lower.x),
                    lower.y + prn(seed) * (upper.y - lower.y),
                    lower.z + prn(seed) * (upper.z - lower.z),
                );
                Site::new(r, u, energy_ev)
            }
        }
    }
}

/// Settings for a fixed-source run.
#[derive(Debug, Clone, Copy)]
pub struct FixedSourceSettings {
    /// Number of source particles to sample and transport.
    pub n_particles: usize,
    /// Number of statistical batches (realizations) the particles are split into
    /// for the tally's mean/uncertainty. Each batch is flushed as one
    /// realization; read a tally with `n_batches` as the realization count.
    pub n_batches: usize,
    /// Material temperature \[K\] for the cross-section lookup.
    pub temperature_k: f64,
    /// Master RNG seed (fixed → reproducible on the single-thread path).
    pub seed: u64,
    /// Safety cap on fission secondaries transported per source particle — the
    /// backstop against runaway multiplication if a (mis-specified)
    /// super-critical system is run as a fixed source.
    pub max_secondaries: usize,
}

impl Default for FixedSourceSettings {
    fn default() -> Self {
        Self {
            n_particles: 10_000,
            n_batches: 20,
            temperature_k: 293.6,
            seed: 1,
            max_secondaries: 10_000,
        }
    }
}

/// The outcome of a fixed-source run (tally results are accumulated in place into
/// the caller's `Tally`).
#[derive(Debug, Clone, Copy)]
pub struct FixedSourceResult {
    /// Source particles transported.
    pub source_particles: usize,
    /// Total histories transported (source particles **plus** every fission
    /// secondary) — `> source_particles` reveals sub-critical multiplication.
    pub total_histories: usize,
    /// Mean number of fission neutrons produced per **source** particle
    /// (`Σ ν·σ_f / σ_t` over collisions) — the sub-critical multiplication `M`.
    /// `0` for a non-fissile (shielding) system.
    pub multiplication: f64,
}

/// Run a fixed-source transport calculation.
///
/// Samples `settings.n_particles` neutrons from `source`, transports each (and
/// its fission secondaries) through `geom` to death, and — if a `tally` is
/// supplied — accumulates its track-length scores in place (flushed once per
/// batch, so read it back with `settings.n_batches` realizations). Returns a
/// [`FixedSourceResult`] balance.
///
/// Single-threaded reference path (deterministic for a fixed `seed`).
///
/// # Example — void streaming (the analytic check)
/// ```
/// use outram_mc_libs::physics::fixed_source::{run_fixed_source, FixedSource, FixedSourceSettings};
/// use outram_mc_libs::geometry::position::Position;
/// use outram_mc_libs::geometry::surface::{Sphere, SurfaceKind, BoundaryType};
/// use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
/// use outram_mc_libs::geometry::universe::Universe;
/// use outram_mc_libs::geometry::geometry::Geometry;
///
/// // A vacuum sphere of radius R: a point source at the centre streams straight
/// // out, so every neutron travels exactly R — the mean path length is R.
/// let r_cm = 5.0;
/// let geom = Geometry {
///     surfaces: vec![SurfaceKind::Sphere(Sphere { x0: 0.0, y0: 0.0, z0: 0.0, r: r_cm, bc: BoundaryType::Vacuum })],
///     cells: vec![Cell::fill(1, vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Inside }], CellFill::Void, Position::ZERO)],
///     universes: vec![Universe { id: 0, cell_indices: vec![0] }],
///     lattices: vec![],
///     root_universe: 0,
/// };
/// let src = FixedSource::Point { r: Position::ZERO, energy_ev: 2.0e6 };
/// let res = run_fixed_source(&geom, &[], &[], &src, &FixedSourceSettings { n_particles: 1000, ..Default::default() }, None);
/// assert_eq!(res.total_histories, 1000); // void: no collisions, no secondaries
/// ```
pub fn run_fixed_source(
    geom: &Geometry,
    materials: &[Material],
    nuclides: &[Nuclide],
    source: &FixedSource,
    settings: &FixedSourceSettings,
    mut tally: Option<&mut Tally>,
) -> FixedSourceResult {
    let mut seed = settings.seed;
    let n_bins = tally.as_ref().map(|t| t.n_bins()).unwrap_or(0);
    let mut batch = vec![0.0; n_bins];

    let n = settings.n_particles.max(1);
    let n_batches = settings.n_batches.max(1);
    // Even split; the last batch absorbs the remainder.
    let per_batch = n.div_ceil(n_batches);

    let mut total_histories = 0usize;
    let mut production_sum = 0.0;
    let mut done = 0usize;

    for _b in 0..n_batches {
        let this_batch = per_batch.min(n - done);
        if this_batch == 0 {
            break;
        }
        for _ in 0..this_batch {
            // One source particle and its fission progeny (a private bank).
            let mut bank = vec![source.sample(&mut seed)];
            let mut secondaries = 0usize;
            while let Some(site) = bank.pop() {
                total_histories += 1;
                let mut next: Vec<Site> = Vec::new();
                // k_running = 1.0: analog multiplicity (no eigenvalue normalization).
                let prod = transport_history(
                    site,
                    geom,
                    materials,
                    nuclides,
                    settings.temperature_k,
                    1.0,
                    &mut next,
                    &mut seed,
                    tally.as_deref(),
                    &mut batch,
                );
                production_sum += prod;
                for s in next {
                    if secondaries < settings.max_secondaries {
                        bank.push(s);
                        secondaries += 1;
                    }
                }
            }
        }
        done += this_batch;
        // Close this batch as one tally realization.
        if let Some(t) = tally.as_deref_mut() {
            flush_batch(t, &mut batch);
        }
    }

    FixedSourceResult {
        source_particles: done,
        total_histories,
        multiplication: production_sum / done.max(1) as f64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
    use crate::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
    use crate::geometry::universe::Universe;
    use crate::tally::filter::{CellFilter, Filter};
    use crate::tally::tally::{ScoreType, Tally, TallyBin};

    /// A single-cell sphere of radius `r_cm`; `fill` chooses void or a material.
    fn sphere_geom(r_cm: f64, bc: BoundaryType, fill: CellFill) -> Geometry {
        Geometry {
            surfaces: vec![SurfaceKind::Sphere(Sphere {
                x0: 0.0,
                y0: 0.0,
                z0: 0.0,
                r: r_cm,
                bc,
            })],
            cells: vec![Cell::fill(
                1,
                vec![RegionToken::HalfSpace {
                    surface_idx: 0,
                    sense: HalfSpaceSense::Inside,
                }],
                fill,
                Position::ZERO,
            )],
            universes: vec![Universe {
                id: 0,
                cell_indices: vec![0],
            }],
            lattices: vec![],
            root_universe: 0,
        }
    }

    fn flux_tally() -> Tally {
        let filter: Box<dyn Filter> = Box::new(CellFilter {
            cell_indices: vec![0],
        });
        Tally {
            id: 1,
            name: "flux".into(),
            filters: vec![filter],
            scores: vec![ScoreType::Flux],
            bins: vec![TallyBin::default(); 1],
        }
    }

    /// V&V — **analytic**: void streaming. Methodology: an isotropic point source
    /// at the centre of a vacuum sphere of radius R. In a void there are no
    /// collisions, so every neutron streams straight to the boundary a distance
    /// R and leaks — the track-length flux tally is therefore **exactly** N·R,
    /// i.e. the mean path length per source particle equals R. Pass criterion:
    /// tally flux sum / N == R to within tight numerical tolerance (this is
    /// deterministic — zero variance — so the tolerance is machine-small).
    /// Result: mean path length = R exactly; no secondaries, no multiplication.
    #[test]
    fn void_streaming_mean_path_equals_radius() {
        let r_cm = 5.0;
        let geom = sphere_geom(r_cm, BoundaryType::Vacuum, CellFill::Void);
        let src = FixedSource::Point {
            r: Position::ZERO,
            energy_ev: 2.0e6,
        };
        let n = 2000usize;
        let mut tally = flux_tally();
        let res = run_fixed_source(
            &geom,
            &[],
            &[],
            &src,
            &FixedSourceSettings {
                n_particles: n,
                n_batches: 10,
                ..Default::default()
            },
            Some(&mut tally),
        );

        assert_eq!(
            res.total_histories, n,
            "void: no collisions, no secondaries"
        );
        assert_eq!(res.multiplication, 0.0, "no fissile material");
        // Flux tally sum is the total track length = N·R (exact for a void).
        let total_path = tally.bins[0].sum;
        let mean_path = total_path / n as f64;
        assert!(
            (mean_path - r_cm).abs() < 1e-9,
            "mean path length = R; got {mean_path} vs {r_cm}"
        );
    }

    /// V&V — behavioural: sub-critical multiplication is tracked and terminates.
    /// Methodology: a point source of fast neutrons at the centre of a small HEU
    /// sphere (well below the Godiva critical radius, so k < 1). Pass criteria:
    /// the run terminates (no runaway); fission secondaries are produced
    /// (multiplication > 0) so more histories run than source particles; and the
    /// flux tally is strictly positive. (A quantitative M = 1/(1−k_eff) check is
    /// deferred — it needs the eigenvalue of this exact body.)
    #[test]
    fn subcritical_multiplication_is_tracked_and_bounded() {
        use crate::material::material::{Material, NuclideComponent};
        use crate::material::nuclide::Nuclide;

        let nuclides = vec![
            Nuclide::from_core("U234").unwrap(),
            Nuclide::from_core("U235").unwrap(),
            Nuclide::from_core("U238").unwrap(),
        ];
        let heu = Material {
            id: 1,
            name: "HEU".into(),
            temperature: 293.6,
            components: vec![
                NuclideComponent {
                    nuclide_idx: 0,
                    atom_density: 4.9184e-4,
                },
                NuclideComponent {
                    nuclide_idx: 1,
                    atom_density: 4.4994e-2,
                },
                NuclideComponent {
                    nuclide_idx: 2,
                    atom_density: 2.4984e-3,
                },
            ],
        };
        // r = 4 cm ≪ 8.74 cm Godiva critical radius ⇒ strongly sub-critical.
        let geom = sphere_geom(4.0, BoundaryType::Vacuum, CellFill::Material(0));
        let src = FixedSource::Point {
            r: Position::ZERO,
            energy_ev: 2.0e6,
        };
        let mut tally = flux_tally();
        let res = run_fixed_source(
            &geom,
            &[heu],
            &nuclides,
            &src,
            &FixedSourceSettings {
                n_particles: 500,
                n_batches: 5,
                ..Default::default()
            },
            Some(&mut tally),
        );

        assert!(
            res.multiplication > 0.0,
            "fissile system multiplies, M = {}",
            res.multiplication
        );
        assert!(
            res.total_histories > res.source_particles,
            "secondaries were transported"
        );
        assert!(tally.bins[0].sum > 0.0, "flux tally is positive");
    }
}
