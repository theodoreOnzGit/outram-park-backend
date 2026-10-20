//! The boon-lay calls behind the "A fragment's story" chapter: decay chains
//! from boon-lay's `DecayLibrary` (half-lives and decay modes of the
//! ENDF/B-VIII.0 depletion chain), sampled histories from its decay Monte
//! Carlo (`SingleNuclideSimulatorMC`, which walks a `StochasticDecayChain`),
//! and the cited Arrhenius correlations of its TRISO-ATOPS fork
//! (`triso_atops_fork::diffusion`). Public API only; nothing in boon-lay is
//! changed or copied. Shared by `main.rs` and
//! `tests/lesson_fission_cascades.rs` through `#[path]`.
#![allow(dead_code)]

use boon_lay::prelude::decay_library::DecayLibrary;
use boon_lay::prelude::{diffusion_coefficient, diffusion_coefficient_sic_ag, HalfLifeAndDecayEnergyInfo, SingleNuclideSimulatorMC};
use boon_lay::Nuclide;
use petir::rng::lcg::Lcg64;
use uom::si::diffusion_coefficient::square_meter_per_second;
use uom::si::energy::electronvolt;
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::second;

/// One decay on a chain's main path: the parent, how it decays, and into
/// what.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    /// The Nuclide enum's spelling, e.g. "Sr95".
    pub parent: String,
    pub z: u32,
    pub a: u32,
    /// Half-life [s].
    pub half_life_s: f64,
    /// The library's mean energy released per decay [eV] (OpenMC chain
    /// `decay_energy`: the heat the decay leaves, neutrinos excluded).
    pub decay_energy_ev: f64,
    /// The decay type's Debug name ("BetaMinus", ...).
    pub mode: String,
    /// Branching ratio of this branch.
    pub branching: f64,
    pub daughter: String,
    pub daughter_z: u32,
    pub daughter_a: u32,
}

/// Half-life [s] of `n`, or `None` when the library has it as stable or does
/// not hold it.
pub fn half_life_s(lib: &DecayLibrary, n: Nuclide) -> Option<f64> {
    lib.try_match_nuclides_to_decay_data(n)?.try_get_half_life().map(|t| t.get::<second>())
}

/// The main path of `start`'s decay chain: at every nuclide, the branch with
/// the largest branching ratio, until a stable nuclide (or `max_steps`).
pub fn main_path(lib: &DecayLibrary, start: Nuclide, max_steps: usize) -> Vec<Step> {
    let mut out = Vec::new();
    let mut n = start;
    for _ in 0..max_steps {
        let Some(d) = lib.try_match_nuclides_to_decay_data(n) else { break };
        let HalfLifeAndDecayEnergyInfo::Unstable(t, e) = d.half_life_information else { break };
        let Some(best) = d
            .decay_information
            .iter()
            .filter(|b| b.target.is_some())
            .max_by(|a, b| a.branching_ratio.value.total_cmp(&b.branching_ratio.value))
        else {
            break;
        };
        let target = best.target.expect("filtered");
        let (z, a) = n.get_z_a();
        let (dz, da) = target.get_z_a();
        out.push(Step {
            parent: format!("{n:?}"),
            z,
            a,
            half_life_s: t.get::<second>(),
            decay_energy_ev: e.get::<electronvolt>(),
            mode: format!("{:?}", best.decay_type),
            branching: best.branching_ratio.value,
            daughter: format!("{target:?}"),
            daughter_z: dz,
            daughter_a: da,
        });
        n = target;
    }
    out
}

/// Whether `n`'s decay data list a branch of the given type (Debug name),
/// and its branching ratio.
pub fn branch_of(lib: &DecayLibrary, n: Nuclide, mode: &str) -> Option<(f64, Nuclide)> {
    let d = lib.try_match_nuclides_to_decay_data(n)?;
    d.decay_information
        .iter()
        .find(|b| format!("{:?}", b.decay_type) == mode && b.target.is_some())
        .map(|b| (b.branching_ratio.value, b.target.expect("checked")))
}

/// `n` sampled histories of `start` through boon-lay's decay Monte Carlo:
/// each is the list of (nuclide, time of its decay since the fragment
/// stopped [s]) along one randomly branched chain, ending with the stable
/// nuclide at `f64::INFINITY`. The library's generator is reseeded with
/// `seed` first, so the output is reproducible.
pub fn sample_histories(lib: &mut DecayLibrary, start: Nuclide, n: usize, seed: u128) -> Vec<Vec<(String, f64)>> {
    lib.random_number_generator = Lcg64::new(seed);
    (0..n)
        .map(|_| {
            let sim = SingleNuclideSimulatorMC::new_decay_chain_simulation(start, lib);
            // The start nuclide's own lifetime, then one per daughter.
            let mut names = vec![format!("{start:?}")];
            names.extend(sim.get_decay_chain_vec().iter().map(|x| format!("{x:?}")));
            let mut lives = vec![sim.get_time_to_next_decay().get::<second>()];
            lives.extend(sim.get_time_to_live_vec().iter().map(|t| t.get::<second>()));
            let mut t = 0.0;
            names
                .into_iter()
                .zip(lives)
                .map(|(name, life)| {
                    t += life;
                    (name, t)
                })
                .collect()
        })
        .collect()
}

/// One cited Arrhenius correlation sampled on a temperature grid.
#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    pub label: String,
    pub z: u32,
    pub material: &'static str,
    /// D [m^2/s] at each grid temperature.
    pub d: Vec<f64>,
    /// The highest grid temperature [°C] at or below which D does not change
    /// from one grid point to the next (the correlation is clamped there);
    /// `None` when it changes everywhere.
    pub flat_up_to_c: Option<f64>,
}

/// The highest `t[i]` with `d[i] == d[i - 1]` scanning upward while D is
/// flat from the start, i.e. the top of a lower clamp; `None` if `d[1] != d[0]`.
pub fn flat_up_to(t: &[f64], d: &[f64]) -> Option<f64> {
    let mut top = None;
    for i in 1..d.len() {
        if d[i] == d[i - 1] {
            top = Some(t[i]);
        } else {
            break;
        }
    }
    top
}

/// TRISO-ATOPS's NP-MHTGR correlations through boon-lay's public
/// `diffusion_coefficient` (kernel and graphite) and
/// `diffusion_coefficient_sic_ag`, on the grid `t_c` [°C].
pub fn diffusion_series(t_c: &[f64]) -> Vec<Series> {
    let temp = |c: f64| ThermodynamicTemperature::new::<degree_celsius>(c);
    let mut out = Vec::new();
    for (z, name) in [(36u32, "Kr"), (54, "Xe"), (55, "Cs"), (38, "Sr"), (47, "Ag")] {
        for material in ["kernel", "graphite"] {
            let d: Vec<f64> = t_c
                .iter()
                .map(|&c| {
                    let k = diffusion_coefficient(z, temp(c), temp(c));
                    let v = if material == "kernel" { k.kernel } else { k.graphite };
                    v.get::<square_meter_per_second>()
                })
                .collect();
            out.push(Series { label: format!("{name} in {material}"), z, material, flat_up_to_c: flat_up_to(t_c, &d), d });
        }
    }
    let d: Vec<f64> = t_c.iter().map(|&c| diffusion_coefficient_sic_ag(temp(c)).get::<square_meter_per_second>()).collect();
    out.push(Series { label: "Ag in SiC".to_string(), z: 47, material: "SiC", flat_up_to_c: flat_up_to(t_c, &d), d });
    out
}
