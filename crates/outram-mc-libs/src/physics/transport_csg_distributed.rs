//! **The multi-thread CSG power iteration, with each generation's histories
//! handed out in chunks to workers that share nothing but messages**
//! (gh:#786).
//!
//! [`run_keff_csg_par`](super::run_keff_csg_par) already makes the answer
//! independent of how histories are scheduled: history `i` of generation `g`
//! runs on its own random stream, derived from `(settings.seed, g, i)` alone,
//! and the generation is reduced **in history order** (productions summed,
//! fission banks concatenated) before the bank is resampled on a separate
//! sequential stream. This module is that driver taken apart at those seams,
//! so the histories of a generation can run in different processes or
//! browser Web Workers, which share no memory:
//!
//! - **the coordinator** ([`DistributedPowerIteration`]) holds the source,
//!   the sequential resampling stream and the running `k`; it cuts a
//!   generation into [`GenerationChunk`]s, and reduces the
//!   [`ChunkResult`]s in history order ([`DistributedPowerIteration::finish_generation`]).
//!   It needs no geometry and no nuclear data;
//! - **a worker** runs [`transport_chunk`] on a chunk with its own copy of
//!   the geometry, materials, nuclides and majorants.
//!
//! **The answer does not depend on how many chunks or workers there are, bit
//! for bit**: every history draws from the same stream as in
//! `run_keff_csg_par`, and the reduction adds the same numbers in the same
//! order. `tests` below pin it against `run_keff_csg_par` itself, for one
//! chunk and for several uneven ones.
//!
//! Everything a chunk or result holds encodes to `f64`s exactly
//! ([`GenerationChunk::to_f64s`], [`ChunkResult::to_f64s`]); 64-bit seeds go
//! as two 32-bit halves.
//!
//! Not supported (as in `CsgPowerIteration`): tallies, the leakage spectrum
//! and the `k` trigger. A run goes to its last generation unless the
//! population dies out.

use super::{
    bank_entropy, resample, sample_box_source, transport_history_vr, Site, SourceBox, GEN_STRIDE,
    HIST_STRIDE,
};
use crate::geometry::geometry::Geometry;
use crate::geometry::position::{Direction, Position};
use crate::material::material::Material;
use crate::material::nuclide::Nuclide;
use crate::physics::delta_tracking::Majorant;
use crate::physics::keff::KeffSettings;
use crate::physics::track_output::{Track, TrackEvent, TrackRecorder, TrackState};
use crate::rng::lcg::future_seed;
use raffles::estimators::mean_and_stderr;

/// Cap on states kept per traced history (a history through a reflective,
/// weakly absorbing region can run long; the track is for drawing).
pub const MAX_TRACE_STATES: usize = 200_000;

/// A fission-source neutron as plain numbers: position \[cm\], direction,
/// energy \[eV\], and the delayed-neutron precursor group it was born from
/// (`None` for prompt).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceSite {
    pub r: [f64; 3],
    pub u: [f64; 3],
    pub e: f64,
    pub delayed_group: Option<usize>,
}

impl SourceSite {
    /// Words per site in [`Self::encode`].
    pub const WORDS: usize = 8;

    fn from_site(s: &Site) -> Self {
        Self {
            r: [s.r.x, s.r.y, s.r.z],
            u: [s.u.u, s.u.v, s.u.w],
            e: s.e,
            delayed_group: s.delayed_group,
        }
    }

    fn to_site(self) -> Site {
        Site {
            r: Position::new(self.r[0], self.r[1], self.r[2]),
            u: Direction {
                u: self.u[0],
                v: self.u[1],
                w: self.u[2],
            },
            e: self.e,
            delayed_group: self.delayed_group,
        }
    }

    /// Sites as `[x, y, z, u, v, w, E, group or -1]` each.
    pub fn encode(sites: &[SourceSite]) -> Vec<f64> {
        let mut v = Vec::with_capacity(Self::WORDS * sites.len());
        for s in sites {
            v.extend_from_slice(&s.r);
            v.extend_from_slice(&s.u);
            v.push(s.e);
            v.push(s.delayed_group.map_or(-1.0, |g| g as f64));
        }
        v
    }

    /// The inverse of [`Self::encode`].
    ///
    /// # Errors
    ///
    /// A length that is not a whole number of sites.
    pub fn decode(v: &[f64]) -> Result<Vec<SourceSite>, String> {
        if v.len() % Self::WORDS != 0 {
            return Err(format!("{} words is not a whole number of sites", v.len()));
        }
        Ok(v.chunks_exact(Self::WORDS)
            .map(|c| SourceSite {
                r: [c[0], c[1], c[2]],
                u: [c[3], c[4], c[5]],
                e: c[6],
                delayed_group: (c[7] >= 0.0).then_some(c[7] as usize),
            })
            .collect())
    }
}

/// A 64-bit seed as two `f64`s (high and low 32 bits), exactly.
pub fn seed_words(seed: u64) -> [f64; 2] {
    [(seed >> 32) as f64, (seed & 0xFFFF_FFFF) as f64]
}

/// The inverse of [`seed_words`].
pub fn seed_from_words(hi: f64, lo: f64) -> u64 {
    ((hi as u64) << 32) | (lo as u64)
}

/// One contiguous run of a generation's histories: the unit of work handed
/// to a worker.
#[derive(Debug, Clone, PartialEq)]
pub struct GenerationChunk {
    /// Generation index, from 0 (inactive generations included).
    pub generation: usize,
    /// Index, within the generation, of the first history in `sites`.
    pub first_index: usize,
    /// The run's seed ([`KeffSettings::seed`]); with `generation` and the
    /// history index it fixes each history's random stream.
    pub run_seed: u64,
    /// The `k` fission sites are banked against (the previous generation's).
    pub k_running: f64,
    /// Histories whose index in the generation is below this are traced
    /// (every state recorded, for drawing). Recording draws no random
    /// numbers, so it changes nothing else.
    pub trace_first: usize,
    /// The source neutrons, in history order.
    pub sites: Vec<SourceSite>,
}

impl GenerationChunk {
    /// `[generation, first_index, seed_hi, seed_lo, k_running, trace_first,
    /// sites...]`.
    pub fn to_f64s(&self) -> Vec<f64> {
        let s = seed_words(self.run_seed);
        let mut v = vec![
            self.generation as f64,
            self.first_index as f64,
            s[0],
            s[1],
            self.k_running,
            self.trace_first as f64,
        ];
        v.extend(SourceSite::encode(&self.sites));
        v
    }

    /// The inverse of [`Self::to_f64s`].
    ///
    /// # Errors
    ///
    /// Too short, or a site list that is not whole.
    pub fn from_f64s(v: &[f64]) -> Result<Self, String> {
        if v.len() < 6 {
            return Err("generation chunk: too short".into());
        }
        Ok(Self {
            generation: v[0] as usize,
            first_index: v[1] as usize,
            run_seed: seed_from_words(v[2], v[3]),
            k_running: v[4],
            trace_first: v[5] as usize,
            sites: SourceSite::decode(&v[6..])?,
        })
    }
}

/// Run-level counts of a chunk, a generation or a whole run (the
/// diagnostics `KeffResult` reports).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ChunkCounts {
    pub histories: u64,
    pub collisions: u64,
    pub virtual_collisions: u64,
    pub majorant_violations: u64,
    pub delta_lost: u64,
    pub lost_locate: u64,
    pub stuck_events: u64,
    pub leak_vacuum: u64,
    pub leak_infinity: u64,
}

impl ChunkCounts {
    const WORDS: usize = 9;
    fn add(&mut self, o: &ChunkCounts) {
        self.histories += o.histories;
        self.collisions += o.collisions;
        self.virtual_collisions += o.virtual_collisions;
        self.majorant_violations += o.majorant_violations;
        self.delta_lost += o.delta_lost;
        self.lost_locate += o.lost_locate;
        self.stuck_events += o.stuck_events;
        self.leak_vacuum += o.leak_vacuum;
        self.leak_infinity += o.leak_infinity;
    }
    fn words(&self) -> [f64; Self::WORDS] {
        [
            self.histories as f64,
            self.collisions as f64,
            self.virtual_collisions as f64,
            self.majorant_violations as f64,
            self.delta_lost as f64,
            self.lost_locate as f64,
            self.stuck_events as f64,
            self.leak_vacuum as f64,
            self.leak_infinity as f64,
        ]
    }
    fn from_words(w: &[f64]) -> Self {
        let u = |i: usize| w[i] as u64;
        Self {
            histories: u(0),
            collisions: u(1),
            virtual_collisions: u(2),
            majorant_violations: u(3),
            delta_lost: u(4),
            lost_locate: u(5),
            stuck_events: u(6),
            leak_vacuum: u(7),
            leak_infinity: u(8),
        }
    }
}

/// What a worker returns for a [`GenerationChunk`].
#[derive(Debug, Clone, PartialEq)]
pub struct ChunkResult {
    pub generation: usize,
    pub first_index: usize,
    /// Fission neutron production of each history, in history order.
    pub production: Vec<f64>,
    /// The fission sites banked, in history order.
    pub bank: Vec<SourceSite>,
    pub counts: ChunkCounts,
    /// The traced histories: `(index in the generation, track)`.
    pub tracks: Vec<(usize, Track)>,
}

const STATE_WORDS: usize = 12;

fn event_code(e: TrackEvent) -> f64 {
    match e {
        TrackEvent::Born => 0.0,
        TrackEvent::SurfaceCrossing => 1.0,
        TrackEvent::Scatter => 2.0,
        TrackEvent::Fission => 3.0,
        TrackEvent::Absorption => 4.0,
        TrackEvent::Rouletted => 5.0,
        TrackEvent::Leak => 6.0,
        TrackEvent::Lost => 7.0,
    }
}

fn event_from(c: f64) -> Result<TrackEvent, String> {
    Ok(match c as i64 {
        0 => TrackEvent::Born,
        1 => TrackEvent::SurfaceCrossing,
        2 => TrackEvent::Scatter,
        3 => TrackEvent::Fission,
        4 => TrackEvent::Absorption,
        5 => TrackEvent::Rouletted,
        6 => TrackEvent::Leak,
        7 => TrackEvent::Lost,
        other => return Err(format!("unknown track event {other}")),
    })
}

impl ChunkResult {
    /// `[generation, first_index, n, production×n, counts×9, n_bank,
    /// bank..., n_tracks, (index, dropped, n_states, states×12...)...]`.
    pub fn to_f64s(&self) -> Vec<f64> {
        let mut v = vec![
            self.generation as f64,
            self.first_index as f64,
            self.production.len() as f64,
        ];
        v.extend_from_slice(&self.production);
        v.extend(self.counts.words());
        v.push(self.bank.len() as f64);
        v.extend(SourceSite::encode(&self.bank));
        v.push(self.tracks.len() as f64);
        for (i, t) in &self.tracks {
            v.extend([*i as f64, t.dropped_states as f64, t.states.len() as f64]);
            for s in &t.states {
                v.extend([
                    s.r.x,
                    s.r.y,
                    s.r.z,
                    s.u.u,
                    s.u.v,
                    s.u.w,
                    s.energy,
                    s.time,
                    s.weight,
                    if s.cell == usize::MAX {
                        -1.0
                    } else {
                        s.cell as f64
                    },
                    s.material.map_or(-1.0, |m| m as f64),
                    event_code(s.event),
                ]);
            }
        }
        v
    }

    /// The inverse of [`Self::to_f64s`].
    ///
    /// # Errors
    ///
    /// A vector too short for what it declares, with words left over, or an
    /// unknown track event.
    pub fn from_f64s(v: &[f64]) -> Result<Self, String> {
        let mut at = 0usize;
        let mut take = |n: usize| -> Result<Vec<f64>, String> {
            let s = v.get(at..at + n).ok_or("chunk result: too short")?.to_vec();
            at += n;
            Ok(s)
        };
        let h = take(3)?;
        let (generation, first_index, n) = (h[0] as usize, h[1] as usize, h[2] as usize);
        let production = take(n)?;
        let counts = ChunkCounts::from_words(&take(ChunkCounts::WORDS)?);
        let n_bank = take(1)?[0] as usize;
        let bank = SourceSite::decode(&take(SourceSite::WORDS * n_bank)?)?;
        let n_tracks = take(1)?[0] as usize;
        let mut tracks = Vec::with_capacity(n_tracks);
        for _ in 0..n_tracks {
            let t = take(3)?;
            let (index, dropped, n_states) = (t[0] as usize, t[1] as usize, t[2] as usize);
            let states = take(STATE_WORDS * n_states)?
                .chunks_exact(STATE_WORDS)
                .map(|c| {
                    Ok(TrackState {
                        r: Position::new(c[0], c[1], c[2]),
                        u: Direction {
                            u: c[3],
                            v: c[4],
                            w: c[5],
                        },
                        energy: c[6],
                        time: c[7],
                        weight: c[8],
                        cell: if c[9] < 0.0 {
                            usize::MAX
                        } else {
                            c[9] as usize
                        },
                        material: (c[10] >= 0.0).then_some(c[10] as usize),
                        event: event_from(c[11])?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            tracks.push((
                index,
                Track {
                    states,
                    dropped_states: dropped,
                },
            ));
        }
        if at != v.len() {
            return Err("chunk result: words left over".into());
        }
        Ok(Self {
            generation,
            first_index,
            production,
            bank,
            counts,
            tracks,
        })
    }
}

/// **Worker side**: transport the histories of `chunk`, each on the stream
/// `run_keff_csg_par` gives it, and return their productions, banks, counts
/// and the traced tracks.
///
/// `settings` supplies the variance reduction and the delta-region tally
/// estimator (it must be the coordinator's; the seed comes from the chunk).
pub fn transport_chunk(
    geom: &Geometry,
    materials: &[Material],
    nuclides: &[Nuclide],
    majorants: &[Majorant],
    settings: &KeffSettings,
    chunk: &GenerationChunk,
) -> ChunkResult {
    let gen_base_seed = future_seed(
        (chunk.generation as u64).wrapping_mul(GEN_STRIDE),
        chunk.run_seed,
    );
    let mut production = Vec::with_capacity(chunk.sites.len());
    let mut bank: Vec<Site> = Vec::new();
    let mut counts = ChunkCounts::default();
    let mut tracks = Vec::new();
    let (mut batch, mut leak_batch) = (Vec::new(), Vec::new());
    for (k, s) in chunk.sites.iter().enumerate() {
        let hist_idx = chunk.first_index + k;
        let mut seed = future_seed((hist_idx as u64).wrapping_mul(HIST_STRIDE), gen_base_seed);
        let mut rec =
            (hist_idx < chunk.trace_first).then(|| TrackRecorder::new(1, MAX_TRACE_STATES));
        let o = transport_history_vr(
            s.to_site(),
            geom,
            materials,
            nuclides,
            majorants,
            chunk.k_running,
            &mut bank,
            &mut seed,
            None,
            &mut batch,
            &[],
            &mut leak_batch,
            &settings.variance_reduction,
            settings.delta_tally_estimator,
            rec.as_mut(),
            None,
            None,
            1.0,
            &mut crate::physics::tracking_trace::no_trace,
        );
        production.push(o.production);
        counts.add(&ChunkCounts {
            histories: 1,
            collisions: o.collisions,
            virtual_collisions: o.virtual_collisions,
            majorant_violations: o.majorant_violations,
            delta_lost: o.delta_lost,
            lost_locate: o.lost_locate,
            stuck_events: o.stuck_events,
            leak_vacuum: o.leak_vacuum,
            leak_infinity: o.leak_infinity,
        });
        if let Some(t) = rec.and_then(|r| r.tracks.into_iter().next()) {
            tracks.push((hist_idx, t));
        }
    }
    ChunkResult {
        generation: chunk.generation,
        first_index: chunk.first_index,
        production,
        bank: bank.iter().map(SourceSite::from_site).collect(),
        counts,
        tracks,
    }
}

/// One reduced generation.
#[derive(Debug, Clone, PartialEq)]
pub struct DistributedGeneration {
    pub index: usize,
    pub active: bool,
    /// This generation's `k`: fission production per source neutron.
    pub k: f64,
    /// Mean and standard error over the active generations so far.
    pub k_mean: Option<(f64, f64)>,
    /// Shannon entropy of the bank \[bits\], when a mesh was given.
    pub entropy: Option<f64>,
    /// Fission sites banked, before resampling.
    pub bank_size: usize,
    pub counts: ChunkCounts,
    /// Traced histories, in history order.
    pub tracks: Vec<(usize, Track)>,
}

/// **Coordinator side** of the distributed power iteration (module docs).
/// Holds no geometry and no nuclear data.
#[derive(Clone)]
pub struct DistributedPowerIteration {
    settings: KeffSettings,
    src_seed: u64,
    source: Vec<Site>,
    k_running: f64,
    active_k: Vec<f64>,
    k_by_generation: Vec<f64>,
    entropy: Vec<f64>,
    generation: usize,
    finished: bool,
    totals: ChunkCounts,
}

impl DistributedPowerIteration {
    /// The initial source as `run_keff_csg_par` samples it (points uniform in
    /// `source_box`, kept in fissile cells, on the sequential source stream
    /// started at `settings.seed`), with that stream's state after it. Needs
    /// the model: run it where the data are (a worker).
    pub fn initial_source(
        geom: &Geometry,
        materials: &[Material],
        nuclides: &[Nuclide],
        source_box: SourceBox,
        settings: &KeffSettings,
    ) -> (Vec<SourceSite>, u64) {
        let mut seed = settings.seed;
        let sites = sample_box_source(
            geom,
            materials,
            nuclides,
            source_box,
            settings.n_particles,
            &mut seed,
        );
        (sites.iter().map(SourceSite::from_site).collect(), seed)
    }

    /// Start from an initial source sampled by [`Self::initial_source`] and
    /// the source stream's state after it.
    pub fn from_initial_source(
        settings: &KeffSettings,
        sites: &[SourceSite],
        src_seed: u64,
    ) -> Self {
        let n_gen = settings.n_inactive + settings.n_active;
        Self {
            settings: settings.clone(),
            src_seed,
            finished: n_gen == 0 || sites.is_empty(),
            source: sites.iter().map(|s| s.to_site()).collect(),
            k_running: 1.0,
            active_k: Vec::with_capacity(settings.n_active),
            k_by_generation: Vec::with_capacity(n_gen),
            entropy: Vec::new(),
            generation: 0,
            totals: ChunkCounts::default(),
        }
    }

    /// [`Self::initial_source`] then [`Self::from_initial_source`], for a
    /// coordinator that has the model.
    pub fn new(
        geom: &Geometry,
        materials: &[Material],
        nuclides: &[Nuclide],
        source_box: SourceBox,
        settings: &KeffSettings,
    ) -> Self {
        let (sites, seed) = Self::initial_source(geom, materials, nuclides, source_box, settings);
        Self::from_initial_source(settings, &sites, seed)
    }

    /// The current generation cut into at most `n_chunks` contiguous chunks
    /// of near-equal size (empty once the run is over). Histories with index
    /// below `trace_first` are traced.
    pub fn chunks(&self, n_chunks: usize, trace_first: usize) -> Vec<GenerationChunk> {
        if self.finished {
            return Vec::new();
        }
        let n = self.source.len();
        let parts = n_chunks.clamp(1, n.max(1));
        (0..parts)
            .map(|p| (p * n / parts, (p + 1) * n / parts))
            .filter(|(a, b)| b > a)
            .map(|(a, b)| GenerationChunk {
                generation: self.generation,
                first_index: a,
                run_seed: self.settings.seed,
                k_running: self.k_running,
                trace_first,
                sites: self.source[a..b]
                    .iter()
                    .map(SourceSite::from_site)
                    .collect(),
            })
            .collect()
    }

    /// Reduce the current generation from its chunks' results (any order):
    /// productions summed and banks concatenated in history order, as
    /// `run_keff_csg_par` does, then the bank resampled into the next source.
    ///
    /// # Errors
    ///
    /// Results for another generation, or that do not cover the generation's
    /// histories exactly once.
    pub fn finish_generation(
        &mut self,
        mut results: Vec<ChunkResult>,
        entropy_mesh: Option<&crate::tally::mesh::RegularMesh>,
    ) -> Result<DistributedGeneration, String> {
        if self.finished {
            return Err("the run is over".into());
        }
        let gen = self.generation;
        if let Some(r) = results.iter().find(|r| r.generation != gen) {
            return Err(format!(
                "a result for generation {} while reducing {gen}",
                r.generation
            ));
        }
        results.sort_by_key(|r| r.first_index);
        let mut next = 0usize;
        for r in &results {
            if r.first_index != next {
                return Err(format!(
                    "histories {next}..{} missing or repeated",
                    r.first_index
                ));
            }
            next += r.production.len();
        }
        if next != self.source.len() {
            return Err(format!(
                "{next} of {} histories reported",
                self.source.len()
            ));
        }
        let mut production = 0.0_f64;
        let mut next_bank: Vec<Site> = Vec::with_capacity(self.settings.n_particles);
        let mut counts = ChunkCounts::default();
        let mut tracks = Vec::new();
        for r in results {
            for p in &r.production {
                production += p;
            }
            next_bank.extend(r.bank.iter().map(|s| s.to_site()));
            counts.add(&r.counts);
            tracks.extend(r.tracks);
        }
        self.totals.add(&counts);
        let active = gen >= self.settings.n_inactive;
        let k_gen = production / self.settings.n_particles as f64;
        let h = entropy_mesh.and_then(|mesh| bank_entropy(mesh, &next_bank));
        if let Some(h) = h {
            self.entropy.push(h);
        }
        self.k_by_generation.push(k_gen);
        self.k_running = k_gen.max(1.0e-6);
        if active {
            self.active_k.push(k_gen);
        }
        self.generation += 1;
        let bank_size = next_bank.len();
        if self.generation == self.settings.n_inactive + self.settings.n_active
            || next_bank.is_empty()
        {
            self.finished = true;
        } else {
            self.source = resample(&next_bank, self.settings.n_particles, &mut self.src_seed);
        }
        Ok(DistributedGeneration {
            index: gen,
            active,
            k: k_gen,
            k_mean: active.then(|| mean_and_stderr(&self.active_k)),
            entropy: h,
            bank_size,
            counts,
            tracks,
        })
    }

    /// Whether every generation has run (or the population died out).
    pub fn finished(&self) -> bool {
        self.finished
    }

    /// Generations reduced so far.
    pub fn generations_done(&self) -> usize {
        self.generation
    }

    /// The settings the run was started with.
    pub fn settings(&self) -> &KeffSettings {
        &self.settings
    }

    /// `k` of every generation so far, inactive first.
    pub fn k_by_generation(&self) -> &[f64] {
        &self.k_by_generation
    }

    /// Shannon entropy of every generation so far (when a mesh was given).
    pub fn entropy(&self) -> &[f64] {
        &self.entropy
    }

    /// Mean and standard error over the active generations so far.
    pub fn k_mean(&self) -> Option<(f64, f64)> {
        (!self.active_k.is_empty()).then(|| mean_and_stderr(&self.active_k))
    }

    /// Counts over every generation so far.
    pub fn totals(&self) -> ChunkCounts {
        self.totals
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
    use crate::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
    use crate::geometry::universe::Universe;
    use crate::material::material::NuclideComponent;
    use crate::physics::compute::{ComputeType, ThreadCount};
    use crate::tally::mesh::RegularMesh;

    const TEMP: f64 = 293.6;

    /// A delta-tracked HEU core in a surface-tracked U-238 shell (both
    /// tracking methods, a fission bank, leakage), on the embedded LOW-tier
    /// data so the test needs no tapes.
    fn model() -> (Geometry, Vec<Material>, Vec<Nuclide>, Vec<Majorant>) {
        let nucs = vec![
            Nuclide::from_core("U235").expect("U235"),
            Nuclide::from_core("U238").expect("U238"),
        ];
        let mat = |id: i32, a: f64, b: f64| Material {
            id,
            name: format!("m{id}"),
            components: vec![
                NuclideComponent {
                    nuclide_idx: 0,
                    atom_density: a,
                },
                NuclideComponent {
                    nuclide_idx: 1,
                    atom_density: b,
                },
            ],
            temperature: TEMP,
        };
        let mats = vec![mat(1, 4.4994e-2, 2.4984e-3), mat(2, 1.0e-4, 4.7e-2)];
        let sph = |r: f64, bc| {
            SurfaceKind::Sphere(Sphere {
                x0: 0.0,
                y0: 0.0,
                z0: 0.0,
                r,
                bc,
            })
        };
        let hs = |i: usize, sense| RegionToken::HalfSpace {
            surface_idx: i,
            sense,
        };
        let geom = Geometry {
            surfaces: vec![
                sph(6.0, BoundaryType::Transmissive),
                sph(9.0, BoundaryType::Vacuum),
            ],
            cells: vec![
                Cell::material(1, vec![hs(0, HalfSpaceSense::Inside)], 0, TEMP).delta_tracked(0),
                Cell::material(
                    2,
                    vec![
                        hs(0, HalfSpaceSense::Outside),
                        hs(1, HalfSpaceSense::Inside),
                    ],
                    1,
                    TEMP,
                ),
            ],
            universes: vec![Universe {
                id: 0,
                cell_indices: vec![0, 1],
            }],
            lattices: vec![],
            root_universe: 0,
        };
        let grid: Vec<f64> = (0..400)
            .map(|i| 1.0e-5 * (2.0e7_f64 / 1.0e-5).powf(i as f64 / 399.0))
            .collect();
        let maj = vec![Majorant::over_indices(&mats, &[0], &nucs, &grid, 0.3)];
        (geom, mats, nucs, maj)
    }

    fn settings() -> KeffSettings {
        KeffSettings {
            n_particles: 300,
            n_inactive: 2,
            n_active: 4,
            seed: 0x0123_4567_89AB_CDEF,
            temperature_k: TEMP,
            compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(1)),
            ..KeffSettings::default()
        }
    }

    /// Run the distributed iteration with `n_chunks` chunks per generation,
    /// every chunk and result crossing as `f64`s, results reduced in reverse
    /// order (the reduction must sort them).
    fn distributed(n_chunks: usize) -> (Vec<f64>, Vec<f64>, (f64, f64), usize) {
        let (geom, mats, nucs, maj) = model();
        let s = settings();
        let mesh = RegularMesh {
            lower_left: [-9.0; 3],
            upper_right: [9.0; 3],
            dimension: [3, 3, 3],
        };
        let box_ = SourceBox {
            lower: Position::new(-6.0, -6.0, -6.0),
            upper: Position::new(6.0, 6.0, 6.0),
        };
        let (sites, seed) =
            DistributedPowerIteration::initial_source(&geom, &mats, &nucs, box_, &s);
        let sites = SourceSite::decode(&SourceSite::encode(&sites)).expect("sites");
        let mut it = DistributedPowerIteration::from_initial_source(&s, &sites, seed);
        let mut traced = 0;
        while !it.finished() {
            let mut results = Vec::new();
            for c in it.chunks(n_chunks, 2) {
                let c = GenerationChunk::from_f64s(&c.to_f64s()).expect("chunk");
                let r = transport_chunk(&geom, &mats, &nucs, &maj, &s, &c);
                results.push(ChunkResult::from_f64s(&r.to_f64s()).expect("result"));
            }
            results.reverse();
            let g = it.finish_generation(results, Some(&mesh)).expect("reduce");
            traced += g.tracks.len();
            assert!(g.tracks.iter().all(|(i, t)| *i < 2 && t.states.len() >= 2));
        }
        (
            it.k_by_generation().to_vec(),
            it.entropy().to_vec(),
            it.k_mean().expect("mean"),
            traced,
        )
    }

    fn bits(v: &[f64]) -> Vec<u64> {
        v.iter().map(|x| x.to_bits()).collect()
    }

    /// One chunk, three uneven chunks and seven: every generation's `k`, the
    /// entropy and the mean are `run_keff_csg_par`'s bit for bit.
    #[test]
    fn any_chunking_reproduces_run_keff_csg_par_bit_for_bit() {
        let (geom, mats, nucs, maj) = model();
        let mesh = RegularMesh {
            lower_left: [-9.0; 3],
            upper_right: [9.0; 3],
            dimension: [3, 3, 3],
        };
        let box_ = SourceBox {
            lower: Position::new(-6.0, -6.0, -6.0),
            upper: Position::new(6.0, 6.0, 6.0),
        };
        let reference = super::super::run_keff_csg_par(
            &geom,
            &mats,
            &nucs,
            &maj,
            Some(&mesh),
            box_,
            &settings(),
            None,
            &[],
            None,
            ThreadCount::Fixed(1),
        );
        assert!(
            reference.virtual_collisions > 0,
            "the delta region must be exercised"
        );
        for n_chunks in [1, 3, 7] {
            let (k, h, mean, traced) = distributed(n_chunks);
            assert_eq!(
                bits(&k),
                bits(&reference.k_by_generation),
                "{n_chunks} chunks: k by generation"
            );
            assert_eq!(
                bits(&h),
                bits(&reference.entropy),
                "{n_chunks} chunks: entropy"
            );
            assert_eq!(
                (mean.0.to_bits(), mean.1.to_bits()),
                (reference.k_mean.to_bits(), reference.k_std.to_bits()),
                "{n_chunks} chunks: mean"
            );
            assert_eq!(traced, 2 * 6, "two traced histories per generation");
        }
    }

    /// A reduction refuses results that do not cover the generation exactly
    /// once, or belong to another generation.
    #[test]
    fn the_reduction_refuses_incomplete_or_foreign_results() {
        let (geom, mats, nucs, maj) = model();
        let s = settings();
        let box_ = SourceBox {
            lower: Position::new(-6.0, -6.0, -6.0),
            upper: Position::new(6.0, 6.0, 6.0),
        };
        let it = DistributedPowerIteration::new(&geom, &mats, &nucs, box_, &s);
        let chunks = it.chunks(2, 0);
        let r: Vec<ChunkResult> = chunks
            .iter()
            .map(|c| transport_chunk(&geom, &mats, &nucs, &maj, &s, c))
            .collect();
        assert!(it
            .clone()
            .finish_generation(vec![r[0].clone()], None)
            .is_err());
        assert!(it
            .clone()
            .finish_generation(vec![r[0].clone(), r[0].clone(), r[1].clone()], None)
            .is_err());
        let mut other = r[1].clone();
        other.generation = 5;
        assert!(it
            .clone()
            .finish_generation(vec![r[0].clone(), other], None)
            .is_err());
        assert!(it.clone().finish_generation(r, None).is_ok());
        assert_eq!(
            seed_from_words(seed_words(u64::MAX)[0], seed_words(u64::MAX)[1]),
            u64::MAX
        );
        assert!(SourceSite::decode(&[1.0; 9]).is_err());
        assert!(ChunkResult::from_f64s(&[0.0, 0.0, 5.0]).is_err());
    }
}
