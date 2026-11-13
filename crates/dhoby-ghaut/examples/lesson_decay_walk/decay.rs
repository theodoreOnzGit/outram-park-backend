//! The calls behind "An atom's clock: sampling a decay" (Fission products,
//! chapter 2): boon-lay's Lagrangian decay Monte Carlo
//! (`SingleNuclideSimulatorMC`, which walks a `StochasticDecayChain`) on its
//! `DecayLibrary` (the ENDF/B-VIII.0 depletion chain of
//! `openmc-endf-8-depletion-lib-b`, compiled into the crate), and the
//! deterministic check beside it, `expected_inventory::ExpectedInventory`
//! (PETIR's CRAM48 on the matrix the sampler implies, gh:#999). Public API
//! only. Shared by `main.rs` and `tests/lesson_decay_walk.rs` through
//! `#[path]`.
#![allow(dead_code)]

use boon_lay::lagrangian_decay_simulator::expected_inventory::ExpectedInventory;
use boon_lay::prelude::decay_library::DecayLibrary;
use boon_lay::prelude::{HalfLifeAndDecayEnergyInfo, SingleNuclideSimulatorMC};
use boon_lay::Nuclide;
use uom::si::f64::Time;
use uom::si::ratio::ratio;
use uom::si::time::second;

/// The lesson's parents: a short chain (Kr-88 -> Rb-88 -> Sr-88), the
/// iodine of the accident source term (I-131), a chain whose daughter
/// outlives its parent (I-135 -> Xe-135), a branch to a metastable state
/// (Cs-137 -> Ba-137m / Ba-137) and a two-way branch by different decay
/// modes (Bi-212 -> Po-212 by beta, Tl-208 by alpha).
pub const PARENTS: [Nuclide; 5] = [Nuclide::Kr88, Nuclide::I131, Nuclide::I135, Nuclide::Cs137, Nuclide::Bi212];

/// `Kr88` -> `Kr-88`, `Ba137m` -> `Ba-137m` (boon-lay's `Nuclide` Debug
/// name, written the usual way).
pub fn pretty(n: Nuclide) -> String {
    let raw = format!("{n:?}");
    let (base, iso) = match raw.split_once('_') {
        Some((b, i)) => (b.to_string(), if i.starts_with('m') { "m".to_string() } else { format!("_{i}") }),
        None => (raw.clone(), String::new()),
    };
    let k = base.find(|c: char| c.is_ascii_digit()).unwrap_or(base.len());
    format!("{}-{}{}", &base[..k], &base[k..], iso)
}

/// Half-life in seconds from the library, `None` if stable or absent.
pub fn half_life_s(lib: &DecayLibrary, n: Nuclide) -> Option<f64> {
    match lib.try_match_nuclides_to_decay_data(n)?.half_life_information {
        HalfLifeAndDecayEnergyInfo::Stable => None,
        HalfLifeAndDecayEnergyInfo::Unstable(t, _) => Some(t.get::<second>()),
    }
}

/// One decay branch as the library holds it: daughter, ratio, mode.
#[derive(Debug, Clone, PartialEq)]
pub struct Branch {
    pub to: Option<Nuclide>,
    pub ratio: f64,
    pub mode: String,
}

/// The branches of `n` (read from `decay_information` directly: the
/// accessor `get_decay_branch_info` ~~is a `todo!()`~~ has returned them
/// since 2026-11-10, gh:#538).
pub fn branches(lib: &DecayLibrary, n: Nuclide) -> Vec<Branch> {
    lib.try_match_nuclides_to_decay_data(n)
        .map(|d| {
            d.decay_information
                .iter()
                .map(|b| Branch { to: b.target, ratio: b.branching_ratio.get::<ratio>(), mode: format!("{:?}", b.decay_type) })
                .collect()
        })
        .unwrap_or_default()
}

/// One atom's whole sampled future: the nuclides it passes through
/// (`stages[0]` is the parent) and the time each stage ends, s, cumulative
/// (`ends.last()` is infinite: the stable end of the chain).
#[derive(Debug, Clone, PartialEq)]
pub struct Atom {
    pub stages: Vec<Nuclide>,
    pub ends: Vec<f64>,
}

/// The future `SingleNuclideSimulatorMC::new_decay_chain_simulation`
/// samples at birth: one branch per step (`StochasticDecayChain`) and one
/// lifetime per unstable nuclide (`get_time_to_decay_stochastic`), from the
/// library's own generator (seeded 77 by `DecayLibrary::new`).
pub fn sample_atom(lib: &mut DecayLibrary, n: Nuclide) -> (SingleNuclideSimulatorMC, Atom) {
    let sim = SingleNuclideSimulatorMC::new_decay_chain_simulation(n, lib);
    let mut stages = vec![n];
    stages.extend(sim.get_decay_chain_vec());
    let mut clock = sim.get_time_to_next_decay().get::<second>();
    let mut ends = vec![clock];
    for life in sim.get_time_to_live_vec() {
        clock += life.get::<second>();
        ends.push(clock);
    }
    // A stable parent has no chain and an infinite life already.
    ends.truncate(stages.len());
    (sim, Atom { stages, ends })
}

/// Which nuclide an atom is at time `t`, s (bookkeeping on its ends).
pub fn stage_at(a: &Atom, t: f64) -> Nuclide {
    let k = a.ends.iter().position(|&e| t < e).unwrap_or(a.stages.len() - 1);
    a.stages[k]
}

/// Step every simulator through the times `ts` (s, increasing from 0)
/// with `advance_timestep`, and count each species in `species` after each
/// step with `count_nuclides_in_sims_linear`. Row `i` of the result is the
/// fraction of the population that is `species[i]` at each time.
pub fn count_by_stepping(sims: &mut [SingleNuclideSimulatorMC], species: &[Nuclide], ts: &[f64]) -> Vec<Vec<f64>> {
    let n = sims.len() as f64;
    let mut out = vec![Vec::with_capacity(ts.len()); species.len()];
    let mut now = 0.0;
    for &t in ts {
        if t > now {
            for s in sims.iter_mut() {
                s.advance_timestep(Time::new::<second>(t - now));
            }
            now = t;
        }
        let counts = SingleNuclideSimulatorMC::count_nuclides_in_sims_linear(sims, species);
        for (row, (_, c)) in out.iter_mut().zip(counts) {
            row.push(c as f64 / n);
        }
    }
    out
}

/// The expected inventory of the chain from one atom of `n` at each time in
/// `ts` (s), by CRAM48 on the library's decay matrix; rows in the order of
/// the returned species (breadth-first from `n`).
pub fn cram(lib: &DecayLibrary, n: Nuclide, ts: &[f64]) -> (Vec<Nuclide>, Vec<Vec<f64>>) {
    let inv = ExpectedInventory::from_library(n, lib).expect("chain in the library");
    let species = inv.nuclides().to_vec();
    let mut rows = vec![Vec::with_capacity(ts.len()); species.len()];
    for &t in ts {
        let v = inv.solve_from_start(Time::new::<second>(t)).expect("CRAM48 solves");
        for (row, x) in rows.iter_mut().zip(v) {
            row.push(x);
        }
    }
    (species, rows)
}

/// Worst |MC - expected| / sigma over points with at least 10 expected
/// atoms on each side (N p >= 10 and N (1 - p) >= 10), where the binomial
/// sigma sqrt(p (1 - p) / N) is a fair yardstick; and how many points
/// qualified.
pub fn worst_sigma(mc: &[f64], expected: &[f64], n: usize) -> (f64, usize) {
    let nf = n as f64;
    let mut worst: f64 = 0.0;
    let mut used = 0;
    for (m, p) in mc.iter().zip(expected) {
        if nf * p >= 10.0 && nf * (1.0 - p) >= 10.0 {
            let s = (p * (1.0 - p) / nf).sqrt();
            worst = worst.max((m - p).abs() / s);
            used += 1;
        }
    }
    (worst, used)
}

/// Fraction of `atoms` whose first decay goes to each daughter in
/// `daughters`.
pub fn first_branch_fractions(atoms: &[Atom], daughters: &[Nuclide]) -> Vec<f64> {
    let n = atoms.len() as f64;
    daughters
        .iter()
        .map(|d| atoms.iter().filter(|a| a.stages.get(1) == Some(d)).count() as f64 / n)
        .collect()
}
