// SPDX-License-Identifier: GPL-3.0

//! **Uranium and graphite at 296 K: the pieces the rung-2 and rung-3 examples
//! share** (GitHub #524, #525).
//!
//! `ugraphite_four_factor.rs` (homogeneous mixture) and
//! `lumped_ugraphite_kinf.rs` (uranium lumps in graphite) must run the same
//! nuclides, the same compositions and the same six-factor bookkeeping, or the
//! lumped cell's homogeneous limit could not be compared with the homogeneous
//! result. Each pulls this in with
//! `#[path = "common/ugraphite_common.rs"] mod ugraphite_common;`. A
//! subdirectory without `main.rs` is not an example target, so Cargo does not
//! build this file on its own. The physics, the compositions' provenance and
//! the recorded results are documented in `ugraphite_four_factor.rs`.

#![allow(dead_code)]

use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::material::material::Material;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::physics::compute::{ComputeType, ThreadCount};
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::reactor_physics::{
    assemble_six_factors, run_keff_reactor_physics, Estimate, ReactorPhysicsConfig,
    ReactorPhysicsReport, SixFactors, CONSISTENCY_BAND,
};
use outram_mc_libs::physics::transport_csg::SourceBox;
use std::time::Instant;

// The compositions live in the library since 2026-10-04 so the web demo's
// rungs run the same numbers (`outram_mc_libs::vv::ugraphite`); re-exported
// here under the names the two examples use.
#[allow(unused_imports)]
pub use outram_mc_libs::vv::ugraphite::{
    graphite, graphite_density, natural_mix, uranium_metal, uranium_volume_fraction, Mix,
    GRAPHITE_TSL, NAT_U, TAPES, TEMPERATURE_K as TEMP_K,
};

/// Thermal / resonance group boundary \[eV\] (the cadmium cutoff; the module's
/// default).
pub const THERMAL_CUT_EV: f64 = 0.625;
/// Resonance / fast group boundary \[eV\] (the module's default).
pub const RES_UPPER_EV: f64 = 1.0e5;
/// Bottom of the tally grid \[eV\]. The module's default of 1e-3 eV would drop
/// the slowest thermal neutrons from the tallies (off-grid tracks are not
/// scored), and at 296 K those carry a visible share of the 1/v absorption.
pub const E_MIN_EV: f64 = 1.0e-5;
/// Top of the tally grid \[eV\].
pub const E_MAX_EV: f64 = 2.0e7;
/// Fine bins: ~1000 per decade over 1e-5 .. 2e7 eV.
pub const N_FINE: usize = 12_300;

/// Indices into the nuclide array of [`load_nuclides`] (the order of
/// [`TAPES`]).
pub mod nx {
    pub const U234: usize = 0;
    pub const U235: usize = 1;
    pub const U238: usize = 2;
    pub const URANIUM: [usize; 3] = [U234, U235, U238];
}

/// One HTR-10 fuel pebble's uranium and carbon, smeared over the ball
/// (`vv::ugraphite::htr10_pebble_mix`), with its inventory printed.
pub fn htr10_pebble_mix() -> Mix {
    let (mix, inv) = outram_mc_libs::vv::ugraphite::htr10_pebble_mix();
    println!(
        "HTR-10 fuel pebble inventory: {:.0} particles, U {:.4} g, C {:.2} g -> N_C/N_U = {:.1}",
        inv.particles, inv.uranium_g, inv.carbon_g, inv.c_per_u
    );
    mix
}

pub fn env_or<T: std::str::FromStr>(k: &str, d: T) -> T {
    std::env::var(k)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(d)
}

/// U-234, U-235, U-238, C-12, C-13 from ENDF/B-VIII.0 at [`TEMP_K`] (RECONR +
/// BROADR, tolerance 0.001; URR and DBRC by the constructor's defaults), with
/// crystalline-graphite S(alpha,beta) (MAT 30, 296 K) on both carbons.
pub fn load_nuclides() -> Vec<Nuclide> {
    use njoy_outram_park_fork::reference_data::reference_endf;
    let load = |name: &str, file: &str| {
        let p = reference_endf(file).unwrap_or_else(|| panic!("reference tape {file} not found"));
        let t0 = Instant::now();
        let n = Nuclide::from_endf_file(&p, name, TEMP_K, 1.0e-3)
            .unwrap_or_else(|e| panic!("from_endf_file({}): {e}", p.display()));
        eprintln!("  {name:<5} RECONR+BROADR @ {TEMP_K} K: {:.1?}", t0.elapsed());
        n
    };
    let tsl = reference_endf(GRAPHITE_TSL.0).expect("graphite S(a,b) tape");
    let sab = ThermalScattering::from_endf_file(tsl.to_str().expect("path"), GRAPHITE_TSL.1, TEMP_K, "c_Graphite")
        .expect("crystalline-graphite S(a,b) at 296 K");
    TAPES
        .iter()
        .map(|&(name, file)| {
            let n = load(name, file);
            if name.starts_with('C') {
                n.with_thermal_scattering(sab.clone())
            } else {
                n
            }
        })
        .collect()
}

/// How "fuel" is identified for the thermal utilisation `f`.
#[derive(Debug, Clone, Copy)]
pub enum FuelSplit {
    /// The whole problem is this one mixture: split the thermal absorption by
    /// **nuclide** (uranium vs carbon) from the run's fine-group flux. Needed
    /// for a homogeneous medium, which the library's material-based
    /// definition would call all fuel.
    ByNuclide(Mix),
    /// The library's own definition: materials holding a `U23*` nuclide. Exact
    /// for a cell whose uranium is all in its own material (a lump).
    ByMaterial,
}

/// What one case produced.
pub struct CaseResult {
    pub report: ReactorPhysicsReport,
    /// Six factors with the textbook fuel definition.
    pub factors: SixFactors,
    /// Fold of the total Sigma_a over the thermal flux bins, over the tallied
    /// thermal absorption (binning check; ideally 1). NaN for `ByMaterial`.
    pub fold_check: f64,
    /// Uranium share of the thermal absorption (= `f`).
    pub u_share: f64,
    pub wall_s: f64,
}

/// The run settings every case uses, from the environment: `PARTICLES`
/// (20000), `INACTIVE` (20), `ACTIVE` (100), `THREADS` (3).
pub fn physics_config(seed: u64, source: SourceBox) -> ReactorPhysicsConfig {
    let threads: usize = env_or("THREADS", 3);
    ReactorPhysicsConfig {
        keff: KeffSettings {
            n_particles: env_or("PARTICLES", 20_000),
            n_inactive: env_or("INACTIVE", 20),
            n_active: env_or("ACTIVE", 100),
            temperature_k: TEMP_K,
            seed,
            compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(threads)),
            ..KeffSettings::default()
        },
        source_box: source,
        thermal_cutoff_ev: THERMAL_CUT_EV,
        resonance_upper_ev: RES_UPPER_EV,
        n_fine_bins: N_FINE,
        energy_min_ev: E_MIN_EV,
        energy_max_ev: E_MAX_EV,
    }
}

/// Run the power iteration with six-factor capture on `geom` and assemble the
/// factors with the requested fuel definition.
pub fn run_case(
    geom: &Geometry,
    mats: &[Material],
    nuclides: &[Nuclide],
    seed: u64,
    source_half_cm: f64,
    split: FuelSplit,
) -> CaseResult {
    let h = source_half_cm;
    let cfg = physics_config(
        seed,
        SourceBox {
            lower: Position::new(-h, -h, -h),
            upper: Position::new(h, h, h),
        },
    );
    let t0 = Instant::now();
    let report = run_keff_reactor_physics(geom, mats, nuclides, &cfg).expect("reactor physics");
    let wall_s = t0.elapsed().as_secs_f64();
    let sf = &report.six_factors;
    match split {
        FuelSplit::ByMaterial => CaseResult {
            factors: sf.clone(),
            fold_check: f64::NAN,
            u_share: sf.f.mean,
            report,
            wall_s,
        },
        FuelSplit::ByNuclide(mix) => {
            let dens = mix.densities();
            let sp = &report.spectrum;
            let (mut fold_tot, mut fold_u) = (0.0_f64, 0.0_f64);
            for (b, w) in sp.energy_edges_ev.windows(2).enumerate() {
                if w[1] > THERMAL_CUT_EV * (1.0 + 1e-9) {
                    break;
                }
                let e_mid = (w[0] * w[1]).sqrt();
                for (i, nuc) in nuclides.iter().enumerate() {
                    if dens[i] <= 0.0 {
                        continue;
                    }
                    let r = sp.flux_raw[b] * dens[i] * nuc.xs_at_energy(e_mid, TEMP_K).absorption;
                    fold_tot += r;
                    if nx::URANIUM.contains(&i) {
                        fold_u += r;
                    }
                }
            }
            let a_th = sf.absorption_by_group[0];
            let u_share = fold_u / fold_tot;
            let a_fuel = Estimate {
                mean: a_th.mean * u_share,
                std: a_th.std * u_share,
            };
            let factors = assemble_six_factors(
                sf.absorption_by_group,
                a_fuel,
                sf.production_by_group,
                sf.leakage_by_group,
                sf.group_bounds_ev,
            );
            CaseResult {
                fold_check: fold_tot / a_th.mean,
                u_share,
                report,
                factors,
                wall_s,
            }
        }
    }
}

/// Print one case: k, the three-group factors, telescoping, the two-group
/// view, the group rates and the fuel split.
pub fn print_case(label: &str, c_per_u: f64, r: &CaseResult) {
    let k = &r.report.keff;
    let f = &r.factors;
    let e = |x: Estimate| format!("{:.5} +/- {:.5}", x.mean, x.std);
    println!("\n=== {label} ===");
    println!("cell-average N_C/N_U = {c_per_u:.1}");
    println!(
        "k_inf (power iteration, generation mean) = {:.5} +/- {:.5}   [{:.1} s]",
        k.k_mean, k.k_std, r.wall_s
    );
    println!("three-group factors (library convention, L = 0):");
    println!("  eta     = {}", e(f.eta));
    println!("  f       = {}", e(f.f));
    println!("  p       = {}", e(f.p));
    println!("  epsilon = {}", e(f.epsilon));
    println!("  P_FNL   = {}   P_TNL = {}", e(f.p_fnl), e(f.p_tnl));
    println!("  eta*f*p*eps = {}", e(f.k_from_factors));
    let a: f64 = f.absorption_by_group.iter().map(|x| x.mean).sum();
    let l: f64 = f.leakage_by_group.iter().map(|x| x.mean).sum();
    let p: f64 = f.production_by_group.iter().map(|x| x.mean).sum();
    println!(
        "  P_total/(A_total + L_total) = {:.5} (telescoping: product - P/(A+L) = {:+.2e}; \
         leakage {l:.2e})",
        p / (a + l),
        f.k_from_factors.mean - p / (a + l)
    );
    let gap = (k.k_mean - f.k_from_factors.mean) / k.k_mean;
    println!(
        "  (k - k_factors)/k = {:+.5} (band {:?}: {})",
        gap,
        CONSISTENCY_BAND,
        if gap > CONSISTENCY_BAND.0 && gap < CONSISTENCY_BAND.1 {
            "inside"
        } else {
            "OUTSIDE"
        }
    );
    let (eta2, f2, p2, eps2) = f.two_group_openmc_convention();
    println!(
        "two-group view (split at 0.625 eV): eta {eta2:.5}  f {f2:.5}  p {p2:.5}  eps {eps2:.5}  \
         product {:.5}",
        eta2 * f2 * p2 * eps2
    );
    println!(
        "group rates per source neutron [thermal, resonance, fast]: absorption \
         [{:.5}, {:.5}, {:.5}]  nu-fission [{:.5}, {:.5}, {:.5}]",
        f.absorption_by_group[0].mean,
        f.absorption_by_group[1].mean,
        f.absorption_by_group[2].mean,
        f.production_by_group[0].mean,
        f.production_by_group[1].mean,
        f.production_by_group[2].mean,
    );
    if r.fold_check.is_finite() {
        println!(
            "nuclide split of thermal absorption: uranium share {:.5}; \
             fold(Sigma_a,tot)/tally = {:.6}",
            r.u_share, r.fold_check
        );
    } else {
        println!("fuel = the uranium material (library definition); f = {:.5}", r.u_share);
    }
}
