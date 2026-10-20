//! **The delta-tracked power iteration, taken apart for a pool of workers**
//! (gh:#908): the worker half of [`super::run_keff_delta_par_in`], speaking
//! the chunk protocol of [`crate::physics::transport_csg::distributed`].
//!
//! Nothing here is new physics. [`super::run_keff_delta_par_in`] already
//! gives every history its own stream, `future_seed(hist ·
//! HIST_STRIDE, future_seed(gen · GEN_STRIDE, seed))`, sums the productions
//! and concatenates the banks in history order, and resamples on a separate
//! source stream by uniform combing. Those are exactly the rules of
//! `transport_csg::distributed::DistributedPowerIteration` (the same strides,
//! the same `comb_resample`, `k_running = max(k_gen, 1e-6)`). So:
//!
//! - the **coordinator** is that `DistributedPowerIteration`, unchanged,
//!   started with [`initial_source`]'s sites and stream;
//! - the **worker** is [`transport_chunk`]: the chunk's histories through
//!   this module's own `transport_history`, the same call the parallel
//!   driver makes.
//!
//! The result is `run_keff_delta_par_in`'s, bit for bit, for any chunking;
//! `tests::any_chunking_reproduces_run_keff_delta_par_in_bit_for_bit` pins
//! it. Delta-tracked histories are not traced here and their ends are not
//! counted (the chunk reports how many histories it ran).

use petir::mathf::RealMath;
use super::{sample_initial_source, transport_history, DeltaDomain, MaterialQuery, Site, GEN_STRIDE, HIST_STRIDE};
use crate::geometry::position::{Direction, Position};
use crate::material::material::Material;
use crate::material::nuclide::Nuclide;
use crate::physics::delta_tracking::Majorant;
use crate::physics::keff::KeffSettings;
use crate::physics::tracking_trace::no_trace;
use crate::physics::transport_csg::distributed::{ChunkCounts, ChunkResult, GenerationChunk, SourceSite};
use crate::rng::lcg::future_seed;

fn to_wire(s: &Site) -> SourceSite {
    SourceSite { r: [s.r.x, s.r.y, s.r.z], u: [s.u.u, s.u.v, s.u.w], e: s.e, delayed_group: None }
}

fn from_wire(s: &SourceSite) -> Site {
    Site { r: Position::new(s.r[0], s.r[1], s.r[2]), u: Direction { u: s.u[0], v: s.u[1], w: s.u[2] }, e: s.e }
}

/// The first generation's source, sampled as [`super::run_keff_delta_par_in`]
/// samples it (on the source stream, which starts at `settings.seed`), and
/// that stream's state after it: what
/// `DistributedPowerIteration::from_initial_source` starts from.
pub fn initial_source<Q: MaterialQuery>(
    domain: DeltaDomain,
    materials: &[Material],
    nuclides: &[Nuclide],
    material_at: &Q,
    settings: &KeffSettings,
) -> (Vec<SourceSite>, u64) {
    let mut seed = settings.seed;
    let sites = sample_initial_source(domain, materials, nuclides, material_at, settings, &mut seed);
    (sites.iter().map(to_wire).collect(), seed)
}

/// **Worker side**: delta-track the histories of `chunk`, each on the stream
/// `run_keff_delta_par_in` gives it, and return their productions and banks
/// in history order.
pub fn transport_chunk<Q: MaterialQuery>(
    domain: DeltaDomain,
    materials: &[Material],
    nuclides: &[Nuclide],
    majorant: &Majorant,
    material_at: &Q,
    chunk: &GenerationChunk,
) -> ChunkResult {
    let gen_base_seed = future_seed((chunk.generation as u64).wrapping_mul(GEN_STRIDE), chunk.run_seed);
    let mut production = Vec::with_capacity(chunk.sites.len());
    let mut bank: Vec<Site> = Vec::new();
    for (k, s) in chunk.sites.iter().enumerate() {
        let hist_idx = chunk.first_index + k;
        let mut seed = future_seed((hist_idx as u64).wrapping_mul(HIST_STRIDE), gen_base_seed);
        production.push(transport_history(
            from_wire(s),
            domain,
            materials,
            nuclides,
            majorant,
            chunk.k_running,
            material_at,
            &mut bank,
            &mut seed,
            &mut no_trace,
        ));
    }
    ChunkResult {
        generation: chunk.generation,
        first_index: chunk.first_index,
        counts: ChunkCounts { histories: chunk.sites.len() as u64, ..ChunkCounts::default() },
        production,
        bank: bank.iter().map(to_wire).collect(),
        tracks: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material::material::NuclideComponent;
    use crate::physics::compute::ThreadCount;
    use crate::physics::transport_csg::distributed::DistributedPowerIteration;

    /// A reflective cube, fuel inside a 3 cm ball and moderator outside it
    /// (two materials, so `material_at` matters), on the embedded LOW-tier
    /// data: no tapes needed.
    fn model() -> (DeltaDomain, Vec<Material>, Vec<Nuclide>, Majorant) {
        let nucs = vec![Nuclide::from_core("U235").expect("U235"), Nuclide::from_core("U238").expect("U238")];
        let mat = |id: i32, a: f64, b: f64| Material {
            id,
            name: format!("m{id}"),
            components: vec![NuclideComponent { nuclide_idx: 0, atom_density: a }, NuclideComponent { nuclide_idx: 1, atom_density: b }],
            temperature: 293.6,
        };
        let mats = vec![mat(1, 4.4994e-2, 2.4984e-3), mat(2, 1.0e-4, 4.7e-2)];
        let grid: Vec<f64> = (0..400).map(|i| 1.0e-5 * (2.0e7_f64 / 1.0e-5).r_powf(i as f64 / 399.0)).collect();
        let maj = Majorant::over_indices(&mats, &[0, 1], &nucs, &grid, 0.3);
        (DeltaDomain::Cube { half: 5.0 }, mats, nucs, maj)
    }

    fn query(p: Position) -> Option<usize> {
        Some(if p.x * p.x + p.y * p.y + p.z * p.z < 9.0 { 0 } else { 1 })
    }

    /// One chunk, three and seven, each crossing as `f64`s and reduced in
    /// reverse order by the CSG coordinator: every generation's `k` and the
    /// mean are `run_keff_delta_par_in`'s bit for bit.
    #[test]
    fn any_chunking_reproduces_run_keff_delta_par_in_bit_for_bit() {
        let (domain, mats, nucs, maj) = model();
        let settings = KeffSettings { n_particles: 300, n_inactive: 2, n_active: 4, seed: 0x0123_4567_89AB_CDEF, temperature_k: 293.6, ..KeffSettings::default() };
        let reference = super::super::run_keff_delta_par_in(domain, &mats, &nucs, &maj, query, &settings, ThreadCount::Fixed(1));
        let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
        for n_chunks in [1, 3, 7] {
            let (sites, seed) = initial_source(domain, &mats, &nucs, &query, &settings);
            let mut it = DistributedPowerIteration::from_initial_source(&settings, &sites, seed);
            while !it.finished() {
                let mut results: Vec<ChunkResult> = it
                    .chunks(n_chunks, 0)
                    .iter()
                    .map(|c| {
                        let c = GenerationChunk::from_f64s(&c.to_f64s()).expect("chunk");
                        let r = transport_chunk(domain, &mats, &nucs, &maj, &query, &c);
                        ChunkResult::from_f64s(&r.to_f64s()).expect("result")
                    })
                    .collect();
                results.reverse();
                it.finish_generation(results, None).expect("reduce");
            }
            assert_eq!(bits(it.k_by_generation()), bits(&reference.k_by_generation), "{n_chunks} chunks");
            let (m, s) = it.k_mean().expect("mean");
            assert_eq!((m.to_bits(), s.to_bits()), (reference.k_mean.to_bits(), reference.k_std.to_bits()), "{n_chunks} chunks");
        }
    }
}
