//! **Fixed-source** Monte Carlo transport — an external neutron source driving
//! a (sub-critical or non-multiplying) system, scoring track-length tallies.
//!
//! # New orchestration over the ported collision loop
//!
//! Per the crate porting rule, the *physics* here is **not** reinvented: every
//! flight, collision, reaction and boundary crossing is the already-ported
//! [`transport_history_vr`](crate::physics::transport_csg::transport_history_vr) (the
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
//! ~~Analog transport (no variance reduction).~~ **CORRECTED 2026-09-22
//! (gh:#258)** — analog by default; `FixedSourceSettings::variance_reduction`
//! enables survival biasing, roulette and weight windows, which is what a
//! shielding fixed source needs. Fission neutrons are tracked as
//! secondaries with a per-source-particle safety cap, so a **sub-critical**
//! (`k < 1`) or non-multiplying system converges; a super-critical system would
//! multiply without bound and is capped (and physically meaningless for a fixed
//! source). Neutron-only, offline data. See the workspace `RESPONSIBLE_USE.md`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use crate::geometry::geometry::Geometry;
use crate::geometry::position::{Direction, Position};
use crate::material::material::Material;
use crate::material::nuclide::Nuclide;
use crate::physics::transport_csg::{transport_history_vr, Site};
use crate::rng::distributions::isotropic_direction;
use crate::rng::lcg::prn;
use crate::tally::scoring::flush_batch;
use crate::tally::tally::Tally;

/// An external neutron source for a fixed-source run.
///
/// [`Self::Point`] and [`Self::Box`] are isotropic and mono-energetic (the
/// common shielding/detector case). [`Self::Surface`] replays a recorded
/// surface crossing bank, which carries its own direction, energy **and
/// weight**.
///
/// **Not `Copy` since GitHub #264**: [`Self::Surface`] holds an `Arc` to a
/// recorded bank. Per the workspace design rules that is `Arc<T>` for
/// read-only shared data rather than a lifetime parameter, and it follows
/// [`FixedSourceSettings`], which gave up `Copy` for the same reason when
/// weight windows landed.
#[derive(Debug, Clone)]
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
    /// **Replay a recorded surface-crossing bank** — stage two of the two-stage
    /// shielding workflow (GitHub #264).
    ///
    /// Each sampled site is a crossing recorded by a previous run at a watched
    /// surface, with the direction, energy **and weight** it carried when it
    /// crossed. Nothing is re-sampled: the whole point is that stage two starts
    /// exactly where stage one left off.
    ///
    /// # Normalising stage two
    ///
    /// Sampling is **uniform over the `K` recorded crossings**, so `M` replayed
    /// histories cover `M / K` of the bank and the stage-one-equivalent estimate
    /// is `tally * K / M`. Read `K` from
    /// [`SurfaceSource::len`](crate::source::extra::SurfaceSource::len). With
    /// `M = K` the factor is 1, which is what
    /// `tests/surface_source_two_stage.rs` runs.
    ///
    /// # It panics on an unusable bank, deliberately
    ///
    /// An empty bank, or one that hit its cap and **dropped** crossings, makes
    /// [`SurfaceSource::sample`](crate::source::extra::SurfaceSource::sample)
    /// return an error, and this variant turns that into a panic carrying the
    /// full message. A truncated bank is a prefix biased towards whatever the
    /// first histories did: replaying it yields a systematically wrong second
    /// stage **that looks converged**, and there is no in-band way to signal
    /// that from inside the history loop. Failing loudly beats returning a
    /// plausible wrong number. Check
    /// [`SurfaceSource::dropped`](crate::source::extra::SurfaceSource::dropped)
    /// before building this if a soft failure is wanted.
    Surface(alloc::sync::Arc<crate::source::extra::SurfaceSource>),
    /// **A general [`IndependentSource`](crate::source::source::IndependentSource)**
    /// (gh:#1388): any spatial, energy and angular distribution, including
    /// OpenMC's `Mixture`, `CylindricalIndependent`, ... (via the
    /// `Distribution` variants of `SpatialKind` / `EnergyKind`). The birth
    /// weight is the source's `strength` times the importance weights.
    Independent(alloc::sync::Arc<crate::source::source::IndependentSource>),
}

impl FixedSource {
    /// Sample one source neutron: its birth site and its **birth weight**.
    ///
    /// The weight is `1.0` for [`Self::Point`] and [`Self::Box`]; a
    /// [`Self::Surface`] replay returns the weight the recorded particle
    /// carried across the surface. Returning it rather than assuming 1.0 is
    /// what makes a two-stage run answer the same problem as a single-stage
    /// one — under variance reduction the recorded weights are nowhere near 1,
    /// and discarding them would rescale stage two's answer silently.
    pub(crate) fn sample(&self, seed: &mut u64) -> (Site, f64) {
        match self {
            FixedSource::Point { r, energy_ev } => {
                let (dx, dy, dz) = isotropic_direction(seed);
                (Site::new(*r, Direction::new(dx, dy, dz), *energy_ev), 1.0)
            }
            FixedSource::Box {
                lower,
                upper,
                energy_ev,
            } => {
                let (dx, dy, dz) = isotropic_direction(seed);
                let u = Direction::new(dx, dy, dz);
                let r = Position::new(
                    lower.x + prn(seed) * (upper.x - lower.x),
                    lower.y + prn(seed) * (upper.y - lower.y),
                    lower.z + prn(seed) * (upper.z - lower.z),
                );
                (Site::new(r, u, *energy_ev), 1.0)
            }
            FixedSource::Independent(src) => {
                let site = src.sample(seed);
                (Site::new(site.r, site.u, site.e), site.wgt)
            }
            FixedSource::Surface(ss) => {
                // A replay draws no direction or energy of its own: both were
                // recorded. `sample` consumes exactly one variate to choose
                // which crossing, so the stream stays predictable.
                let site = ss
                    .sample(seed)
                    .expect("a SurfaceSource used as a source must be replayable");
                (
                    Site::new(site.r, site.u, site.e),
                    site.wgt,
                )
            }
        }
    }
}

/// Settings for a fixed-source run.
/// Not `Copy`: since GitHub #258 this carries an optional `Arc` to a weight
/// window set. Clone it explicitly where a copy was previously implicit.
#[derive(Debug, Clone)]
pub struct FixedSourceSettings {
    /// Number of source particles to sample and transport.
    pub n_particles: usize,
    /// Number of statistical batches (realizations) the particles are split into
    /// for the tally's mean/uncertainty. Each batch is flushed as one
    /// realization; read a tally with `n_batches` as the realization count.
    pub n_batches: usize,
    /// ~~Material temperature \[K\] for the cross-section lookup.~~
    /// **CORRECTED 2026-09-30 (GitHub #313).** Not read by the fixed-source
    /// transport: cross sections use each `Material::temperature`, and the
    /// free-gas kinematics use the nuclide's data temperature (pointwise) or
    /// the material's (multipole), see [`crate::material::nuclide::Nuclide::free_gas_kt`].
    /// Until 2026-09-30 it set the free-gas kT only.
    pub temperature_k: f64,
    /// Master RNG seed (fixed → reproducible on the single-thread path).
    pub seed: u64,
    /// Safety cap on fission secondaries transported per source particle — the
    /// backstop against runaway multiplication if a (mis-specified)
    /// super-critical system is run as a fixed source.
    pub max_secondaries: usize,
    /// Normalise each batch per source particle before it is flushed (gh:#675,
    /// OpenMC `Tally::accumulate`); `false` (default) keeps the raw per-batch sums.
    /// The divisor is that batch's own particle count.
    pub normalise_tallies: bool,
    /// Variance reduction (GitHub #258). The [`Default`] is analog.
    ///
    /// A fixed-source shielding run is the case variance reduction exists for:
    /// analog histories die long before reaching a detector behind a shield,
    /// so the attenuated tally never converges. That is why this field is here
    /// and not only on [`crate::physics::keff::KeffSettings`].
    pub variance_reduction: crate::physics::variance_reduction::VarianceReduction,
    /// Where each history's random streams start. The default is the run's
    /// single sequential stream. [`SeedingMode::OpenMc`] puts particle `p` of
    /// batch `b` (OpenMC's `current_batch = b + 1`, `n_particles` per batch,
    /// one generation per batch) on OpenMC's `init_particle_seeds` layout: the
    /// source is sampled from `STREAM_SOURCE`, the transport (and the fission
    /// progeny, which continue the parent's stream as in OpenMC) from
    /// `STREAM_TRACKING`, the URR bands from `STREAM_URR_PTABLE`
    /// (gh:#1386, `initialize_history`, `src/simulation.cpp`).
    pub seeding: crate::rng::openmc_seeding::SeedingMode,
    /// Report-only log of each batch's tally bin sums (GitHub #506); `None`
    /// (the default) records nothing. Never read by transport.
    pub tally_batch_log: Option<crate::stats::correlated_sigma::TallyBatchLog>,
}

impl FixedSourceSettings {
    /// This run with `seeding` ([`FixedSourceSettings::seeding`]).
    pub fn with_seeding(mut self, seeding: crate::rng::openmc_seeding::SeedingMode) -> Self {
        self.seeding = seeding;
        self
    }
}

impl Default for FixedSourceSettings {
    fn default() -> Self {
        Self {
            variance_reduction: Default::default(),
            n_particles: 10_000,
            n_batches: 20,
            temperature_k: 293.6,
            seed: 1,
            max_secondaries: 10_000,
            normalise_tallies: false,
            seeding: Default::default(),
            tally_batch_log: None,
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
    /// Fission secondaries **discarded** because a source particle's chain hit
    /// [`FixedSourceSettings::max_secondaries`] (gh:#685). Non-zero means
    /// `multiplication` and any tally are **biased low**: upstream aborts here
    /// (`fatal_error`, `src/physics.cpp:122-130`) rather than truncating, and
    /// this port reports instead of aborting, so check this field. Near
    /// critical (`k -> 1`) the expected chain length is `1/(1-k)`, so a fixed
    /// cap truncates a growing share of the answer.
    pub dropped_secondaries: usize,
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
    tally: Option<&mut Tally>,
) -> FixedSourceResult {
    run_fixed_source_traced(
        geom, materials, nuclides, source, settings, tally, None, None, None,
    )
}

/// [`run_fixed_source`], failing as OpenMC does when the secondary bank is
/// truncated (gh:#685): `fatal_error("The secondary particle bank appears to be
/// growing without bound ...")`, `src/physics.cpp:122-130`. `Ok` is bit-identical
/// to [`run_fixed_source`]; an `Err` carries the message and the truncated
/// result is dropped, since its `multiplication` and tallies are biased low.
///
/// # Errors
///
/// A message naming how many secondaries were dropped and the cap, when
/// [`FixedSourceResult::dropped_secondaries`] is non-zero.
pub fn try_run_fixed_source(
    geom: &Geometry,
    materials: &[Material],
    nuclides: &[Nuclide],
    source: &FixedSource,
    settings: &FixedSourceSettings,
    tally: Option<&mut Tally>,
) -> Result<FixedSourceResult, String> {
    let res = run_fixed_source(geom, materials, nuclides, source, settings, tally);
    if res.dropped_secondaries > 0 {
        return Err(format!(
            "{} fission secondaries were dropped at max_secondaries = {} (OpenMC aborts, \
             src/physics.cpp:122-130): the multiplication and any tally are biased low. \
             Raise FixedSourceSettings::max_secondaries, or use run_fixed_source and \
             check dropped_secondaries.",
            res.dropped_secondaries, settings.max_secondaries
        ));
    }
    Ok(res)
}

/// [`run_fixed_source`] with **particle track capture** — GitHub #271.
///
/// Every phase-space state of the first `recorder.max_tracks` histories is
/// recorded. Recording draws no randomness, so this returns exactly the same
/// result as [`run_fixed_source`] with the same inputs — pinned by
/// `track_capture_does_not_perturb_the_run`, because a debugging instrument
/// that changes the thing being debugged is worse than none.
pub fn run_fixed_source_traced(
    geom: &Geometry,
    materials: &[Material],
    nuclides: &[Nuclide],
    source: &FixedSource,
    settings: &FixedSourceSettings,
    mut tally: Option<&mut Tally>,
    mut tracks: Option<&mut crate::physics::track_output::TrackRecorder>,
    mut surface_source: Option<&mut crate::source::extra::SurfaceSource>,
    // Distribcell offset tables (GitHub #261), for a tally carrying a
    // `DistribcellFilter` or a `CellInstanceFilter`.
    //
    // This is a parameter rather than something the transport derives because
    // the tables are built PER TARGET CELL, and only the caller knows which
    // cell its tally is about.
    distribcell: Option<&crate::geometry::distribcell::DistribcellOffsets>,
) -> FixedSourceResult {
    // Cell temperatures are the default (GitHub #313): see `run_keff_csg_par_progress`.
    let bound = crate::material::cell_temperature::with_cell_temperatures(geom, materials);
    let (geom, materials): (&Geometry, &[Material]) = match &bound {
        Some((g, m)) => (g, m.as_slice()),
        None => (geom, materials),
    };
    let mut seed = settings.seed;
    let n_bins = tally.as_ref().map(|t| t.n_bins()).unwrap_or(0);
    let mut batch = vec![0.0; n_bins];

    let n = settings.n_particles.max(1);
    let n_batches = settings.n_batches.max(1);
    // gh:#675: EXACTLY `n_batches` batches, sizes differing by at most one
    // (`n / nb`, the first `n % nb` batches one larger). ~~`ceil(n / nb)` per
    // batch with an early `break`, which flushed fewer than `n_batches`
    // realizations (n = 100, nb = 40 gave 34) though the docs say to read a
    // tally with `n_batches`.~~ **CORRECTED 2026-11-19.** An empty batch
    // (n < nb) still closes as a zero realization. `per_batch` stays the
    // largest batch for the OpenMC seeding layout (unchanged when `nb | n`).
    let (base, rem) = (n / n_batches, n % n_batches);
    let per_batch = n.div_ceil(n_batches);

    let mut total_histories = 0usize;
    let mut dropped_secondaries = 0usize;
    let mut production_sum = 0.0;
    let mut done = 0usize;

    for b in 0..n_batches {
        let this_batch = base + usize::from(b < rem);
        for i in 0..this_batch {
            // SeedingMode::OpenMc: this particle's three OpenMC streams.
            let openmc = match settings.seeding {
                crate::rng::openmc_seeding::SeedingMode::Outram => None,
                crate::rng::openmc_seeding::SeedingMode::OpenMc { total_gen } => {
                    let mut layout = crate::rng::openmc_seeding::OpenMcRunLayout::single_rank(
                        settings.seed as i64,
                        per_batch as i64,
                        1,
                    );
                    layout.total_gen = total_gen;
                    Some(layout.particle_seeds(b as i64 + 1, 1, i as i64 + 1))
                }
            };
            let mut src_stream = openmc.map(|s| s[crate::rng::openmc_seeding::STREAM_SOURCE]);
            // The default keeps the one sequential stream, copied in and out.
            let urr_start = openmc.map(|s| s[crate::rng::openmc_seeding::STREAM_URR_PTABLE]);
            // One source particle and its fission progeny (a private bank).
            let (birth_site, birth_weight) = match src_stream.as_mut() {
                Some(ss) => source.sample(ss),
                None => source.sample(&mut seed),
            };
            // gh:#1493: the copy is taken AFTER the source draws, so the default
            // mode keeps develop's one sequential stream. Copying before them
            // (3134e3134) made `seed = hseed` below discard the source's advance:
            // every history reused the same source variates.
            let mut hseed = openmc.map_or(seed, |s| s[crate::rng::openmc_seeding::STREAM_TRACKING]);
            let mut bank = vec![birth_site];
            // Only the SOURCE particle carries the replayed weight; its
            // fission progeny are born at 1 like any other secondary.
            let mut is_source_particle = true;
            let mut secondaries = 0usize;
            while let Some(site) = bank.pop() {
                total_histories += 1;
                let mut next: Vec<Site> = Vec::new();
                // k_running = 1.0: analog multiplicity (no eigenvalue normalization).
                let prod = crate::physics::transport_csg::transport_history_vr_urr(
                    site,
                    geom,
                    materials,
                    nuclides,
                    // No delta-tracked regions in the fixed-source path (yet).
                    &[],
                    1.0,
                    &mut next,
                    &mut hseed,
                    tally.as_deref(),
                    &mut batch,
                    // Leakage accounting is not tracked for fixed-source runs —
                    // the escaping-flux quantity there is a boundary-current
                    // tally, a separate feature. Pass the disabled sink.
                    &[],
                    &mut [],
                    None,
                    &settings.variance_reduction,
                    // No delta regions here (`&[]` above); the default.
                    crate::physics::transport_csg::DeltaTallyEstimator::default(),
                    tracks.as_deref_mut(),
                    surface_source.as_deref_mut(),
                    distribcell,
                    // Only the source particle carries a replayed weight; a
                    // fission secondary is born at 1 like any other.
                    if is_source_particle { birth_weight } else { 1.0 },
                    urr_start,
                    &mut crate::physics::tracking_trace::no_trace,
                );
                is_source_particle = false;
                production_sum += prod.production;
                for s in next {
                    if secondaries < settings.max_secondaries {
                        bank.push(s);
                        secondaries += 1;
                    } else {
                        dropped_secondaries += 1;
                    }
                }
            }
            if openmc.is_none() {
                seed = hseed;
            }
        }
        done += this_batch;
        // Close this batch as one tally realization.
        if let Some(t) = tally.as_deref_mut() {
            // Report-only per-batch log (GitHub #506): reads, draws nothing.
            if let Some(log) = &settings.tally_batch_log {
                log.push(&batch);
            }
            if settings.normalise_tallies && this_batch > 0 {
                let w = this_batch as f64;
                batch.iter_mut().for_each(|v| *v /= w);
            }
            flush_batch(t, &mut batch);
        }
    }

    FixedSourceResult {
        source_particles: done,
        total_histories,
        multiplication: production_sum / done.max(1) as f64,
        dropped_secondaries,
    }
}

/// The result of [`run_fixed_source_flux_derivative`] (the general type of
/// [`crate::physics::derivative_run`]).
pub use crate::physics::derivative_run::DerivativeResult as FluxDerivativeResult;

/// **A tally derivative, wired into the transport** (gh:#683, #263): the FLUX in
/// `score_material` and its derivative with respect to the variable of
/// `derivative`, from one fixed-source run. A thin wrapper over
/// [`crate::physics::derivative_run::run_fixed_source_derivative`] with the flux
/// score; see that module for the method, scope and the other scores.
///
/// # Errors
///
/// A temperature derivative, or any error of the derivative accumulators.
pub fn run_fixed_source_flux_derivative(
    geom: &Geometry,
    materials: &[Material],
    nuclides: &[Nuclide],
    source: &FixedSource,
    settings: &FixedSourceSettings,
    derivative: &crate::tally::derivative::TallyDerivative,
    score_material: usize,
) -> Result<FluxDerivativeResult, String> {
    crate::physics::derivative_run::run_fixed_source_derivative(
        geom,
        materials,
        nuclides,
        source,
        settings,
        &crate::physics::derivative_run::DerivativeSpec {
            derivative: derivative.clone(),
            score: crate::tally::tally::ScoreType::Flux,
            score_material,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
    use crate::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
    use crate::geometry::universe::Universe;
    use crate::tally::filter::{CellFilter, FilterKind};
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
        let filter: FilterKind = FilterKind::Cell(CellFilter {
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
        assert_eq!(res.dropped_secondaries, 0, "default cap not reached here");
    }

    /// gh:#675: `normalise_tallies` divides each batch by its own particle count:
    /// 100 particles in 4 batches of 25, so every bin mean is the raw one over 25;
    /// the run itself (histories, multiplication) is untouched.
    #[test]
    fn normalised_fixed_source_tallies_are_the_raw_ones_per_source_particle() {
        let geom = sphere_geom(5.0, BoundaryType::Vacuum, CellFill::Void);
        let src = FixedSource::Point { r: Position::ZERO, energy_ev: 1.0e6 };
        let run = |norm: bool| {
            let mut tally = flux_tally();
            let res = run_fixed_source(
                &geom, &[], &[], &src,
                &FixedSourceSettings { n_particles: 100, n_batches: 4, normalise_tallies: norm, ..Default::default() },
                Some(&mut tally),
            );
            (res.total_histories, tally.bins[0].mean_realized())
        };
        let (h_raw, raw) = run(false);
        let (h_norm, norm) = run(true);
        assert_eq!(h_raw, h_norm);
        assert!((raw / 25.0 - norm).abs() <= 1e-12 * raw, "{raw} / 25 vs {norm}");
    }

    /// gh:#675: exactly `n_batches` realizations are flushed, whatever `n`
    /// (100 particles in 40 batches flushed 34 before; 10 in 6 flushed 5), and the
    /// void-sphere tally, where every particle scores exactly R, has zero variance.
    #[test]
    fn exactly_n_batches_realizations_are_flushed() {
        let r_cm = 5.0;
        let geom = sphere_geom(r_cm, BoundaryType::Vacuum, CellFill::Void);
        let src = FixedSource::Point { r: Position::ZERO, energy_ev: 1.0e6 };
        for (n, nb) in [(100usize, 40usize), (10, 6), (1000, 20), (7, 7)] {
            let mut tally = flux_tally();
            let res = run_fixed_source(
                &geom,
                &[],
                &[],
                &src,
                &FixedSourceSettings { n_particles: n, n_batches: nb, ..Default::default() },
                Some(&mut tally),
            );
            assert_eq!(res.source_particles, n);
            assert_eq!(tally.bins[0].count, nb as u64, "n = {n}, n_batches = {nb}");
        }
    }

    /// gh:#685: a cap that truncates a chain is REPORTED, not silent. The same
    /// strongly sub-critical HEU sphere with `max_secondaries = 1` must drop
    /// secondaries and say so, and the default cap drops none.
    #[test]
    fn a_truncating_secondary_cap_is_reported() {
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
                NuclideComponent { nuclide_idx: 0, atom_density: 4.9184e-4 },
                NuclideComponent { nuclide_idx: 1, atom_density: 4.4994e-2 },
                NuclideComponent { nuclide_idx: 2, atom_density: 2.4984e-3 },
            ],
        };
        let geom = sphere_geom(4.0, BoundaryType::Vacuum, CellFill::Material(0));
        let src = FixedSource::Point { r: Position::ZERO, energy_ev: 2.0e6 };
        let run = |cap: usize| {
            run_fixed_source(
                &geom,
                &[heu.clone()],
                &nuclides,
                &src,
                &FixedSourceSettings { n_particles: 200, n_batches: 2, max_secondaries: cap, ..Default::default() },
                None,
            )
        };
        let capped = run(1);
        assert!(capped.dropped_secondaries > 0, "cap of 1 must truncate");
        let refused = try_run_fixed_source(
            &geom, &[heu.clone()], &nuclides, &src,
            &FixedSourceSettings { n_particles: 200, n_batches: 2, max_secondaries: 1, ..Default::default() },
            None,
        );
        assert!(refused.unwrap_err().contains("dropped"), "the try_ form refuses");
        assert_eq!(run(10_000).dropped_secondaries, 0);
    }

    /// gh:#683 / #263, analytic: in an infinite (reflective) non-fissile medium
    /// every neutron dies by absorption, and the spectrum shape does not depend
    /// on the density, so the track-length flux is `Phi = 1 / Sigma_a_eff` and
    /// scales as `1 / rho`: `d Phi / d rho = -Phi / rho`. Hydrogen, reflective
    /// sphere, a 2 MeV point source, 20 batches of 1000 histories (measured 2026-11-20:
    /// flux 75.54 +- 0.38, d flux/d rho -1049 +- 94 against the analytic -1144, +1.01
    /// sigma; the same run with 50 histories per batch gave -1488 +- 281, -1.32 sigma). The derivative
    /// accumulated along each history through the trace observer must reproduce
    /// `-Phi / rho` within four combined standard errors; a missing streaming or
    /// collision term (the gh:#683 defect) gives a different sign or size. The
    /// density derivative of a run on a DIFFERENT material (the filter) is zero.
    #[test]
    fn the_flux_derivative_in_an_infinite_medium_is_minus_flux_over_density() {
        use crate::material::material::{Material, NuclideComponent};
        use crate::material::nuclide::Nuclide;
        use crate::tally::derivative::{DerivativeVariable, TallyDerivative};
        let nuclides = vec![Nuclide::from_core("H1").unwrap()];
        let rho = 6.6e-2;
        let mats = vec![Material {
            id: 1,
            name: "H".into(),
            temperature: 293.6,
            components: vec![NuclideComponent { nuclide_idx: 0, atom_density: rho }],
        }];
        let geom = sphere_geom(30.0, BoundaryType::Reflective, CellFill::Material(0));
        let src = FixedSource::Point { r: Position::ZERO, energy_ev: 2.0e6 };
        let settings = FixedSourceSettings { n_particles: 20000, n_batches: 20, seed: 7, ..Default::default() };
        let d = TallyDerivative { material_idx: 0, variable: DerivativeVariable::Density };
        let r = run_fixed_source_flux_derivative(&geom, &mats, &nuclides, &src, &settings, &d, 0).unwrap();
        let expected = -r.flux / rho;
        let sigma = (r.d_flux_std.powi(2) + (r.flux_std / rho).powi(2)).sqrt();
        println!(
            "flux = {:.4} +- {:.4}; d flux/d rho = {:.3} +- {:.3}; analytic -flux/rho = {:.3} ({:+.2} sigma)",
            r.flux, r.flux_std, r.d_flux, r.d_flux_std, expected, (r.d_flux - expected) / sigma
        );
        assert!(r.flux > 0.0 && r.d_flux < 0.0, "flux {} and derivative {} must have opposite signs", r.flux, r.d_flux);
        assert!((r.d_flux - expected).abs() < 4.0 * sigma, "d flux/d rho {} vs -flux/rho {expected} (sigma {sigma})", r.d_flux);
        // One nuclide in the material: a nuclide-density derivative is the density
        // derivative (`d Sigma/d N_i = sigma_i = Sigma/rho`), on the same histories.
        let dn = TallyDerivative { material_idx: 0, variable: DerivativeVariable::NuclideDensity { nuclide_idx: 0 } };
        let rn = run_fixed_source_flux_derivative(&geom, &mats, &nuclides, &src, &settings, &dn, 0).unwrap();
        assert!((rn.d_flux - r.d_flux).abs() <= 1e-9 * r.d_flux.abs(), "{} vs {}", rn.d_flux, r.d_flux);
        // Temperature is refused.
        let dt = TallyDerivative { material_idx: 0, variable: DerivativeVariable::Temperature };
        assert!(run_fixed_source_flux_derivative(&geom, &mats, &nuclides, &src, &settings, &dt, 0).is_err());
    }
}
