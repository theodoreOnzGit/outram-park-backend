// SPDX-License-Identifier: GPL-3.0

//! **Uranium mixed evenly through graphite: the four-factor formula, measured.**
//! Tutorial rung 2 (GitHub #524, epic #520).
//!
//! > Research, education and V&V only. Nothing here is authoritative for the
//! > operation, licensing or safety of any reactor (`RESPONSIBLE_USE.md`).
//!
//! An infinite, homogeneous mixture of uranium and graphite: a reflective box
//! filled with one material, so `k` is `k_inf` and there is no leakage. The
//! eigenvalue comes from the ordinary CSG power iteration
//! ([`run_keff_reactor_physics`], i.e. `transport_csg::run_keff_csg_reactor_physics`),
//! and the same run's track-length tallies give the six factors of
//! `physics::reactor_physics` (three groups: thermal < 0.625 eV <= resonance
//! < 100 keV <= fast). With no leakage the two non-leakage factors are exactly
//! 1, and what is left is the four-factor formula, `k_inf = eta f p epsilon`.
//!
//! **Verification, not validation.** No experiment has been done on a
//! homogeneous uranium-graphite mixture; the checks below are against algebra,
//! against an energy-only run of the same collision physics, and against
//! another code (OpenMC).
//!
//! # Physics and data (the defaults, nothing ablated)
//!
//! - ENDF/B-VIII.0 tapes from `reference-data/endf/`, read directly through the
//!   workspace's NJOY port (RECONR + BROADR at 296 K, tolerance 0.001). No WMP.
//! - URR probability tables and DBRC: the constructor defaults (on).
//! - Graphite bound-atom scattering: `tsl-crystalline-graphite.endf` (MAT 30)
//!   at 296 K, a tabulated temperature, attached to C-12 and C-13. This is the
//!   law OpenMC calls `c_Graphite`.
//! - 296 K everywhere. It is the lowest tabulated temperature of the graphite
//!   law, and the lesson's room-temperature case.
//!
//! # The fuel definition, and why `f` needs a nuclide split here
//!
//! `run_keff_reactor_physics` calls a **material** "fuel" when it contains a
//! `U23*` nuclide. In a homogeneous mixture the only material is fuel, so its
//! thermal utilisation would be exactly 1 and `eta` would absorb the carbon
//! term. That is not the textbook `f`. This example therefore splits the
//! thermal absorption by **nuclide**: it takes the run's own fine-group
//! track-length flux in the thermal group (1000 bins per decade) and folds it
//! with each nuclide's `N sigma_a(E)`. The uranium share of that fold,
//! multiplied by the tallied thermal absorption, is the fuel absorption fed to
//! the library's own [`assemble_six_factors`], so the factor formulas are the
//! library's and not a second copy. The fold of the **total** `Sigma_a` is
//! printed against the tallied thermal absorption as a check on the binning.
//! The split leaves `eta * f`, and hence `k` from the factors, unchanged.
//!
//! # Checks
//!
//! 1. **Telescoping.** `eta f p epsilon` must equal `P_total / A_total`, and the
//!    power-iteration `k` within [`CONSISTENCY_BAND`] (the band exists because
//!    `(n,2n)` adds neutrons without a fission).
//! 2. **`p` against the energy-only walk at the same composition**
//!    (`common/energy_only_slowing_down.rs`, the kernel of
//!    `u238_resonance_escape.rs`): neutrons started at 100 keV and followed in
//!    energy only to 0.625 eV, same nuclides, same temperature, same physics.
//!    The two are **not** the same quantity, so agreement is expected only to
//!    within what separates them: the transport `p` is a ratio of absorption
//!    rates whose resonance group also holds neutrons *born* below 100 keV
//!    (about 1 % of a fission spectrum), and those start lower and escape more
//!    easily. The difference is reported, not gated.
//! 3. **OpenMC code-to-code** on the same mixture:
//!    `verification_and_validation/tutorial_rung2/openmc_inputs/ugraphite_openmc.py`
//!    and the record beside it.
//!
//! # The main case: an HTR-10 fuel pebble's moderation ratio
//!
//! The carbon-to-uranium ratio of the main case is **not** chosen to give any
//! particular `k`. It is the atom ratio of one HTR-10 fuel pebble, computed
//! from the published fuel-element specification already in this crate
//! (`pebble_beds::htr10`, Li, Yu & Wei 2014 Table 2, and IAEA-TECDOC-1382
//! Table 4-2 for the enrichment basis and graphite density): 8335 TRISO
//! particles of 250 um UO2 kernels at 10.4 g/cm3 and 17 wt% U-235, their
//! buffer / PyC / SiC coatings, a 2.5 cm fuel zone of 1.73 g/cm3 matrix
//! graphite and a 0.5 cm graphite shell. All the carbon of the pebble (matrix,
//! shell, buffer, both PyC layers and the carbon of the SiC) is counted, and
//! the uranium is the kernels'. The result is smeared over the 3 cm ball.
//!
//! **Deliberate liberties**, each unmeasured here: the oxygen of the UO2 and
//! the silicon of the SiC are left out (this rung is uranium and graphite, by
//! design), as are the boron impurities; the coolant gaps between pebbles are
//! not modelled (one pebble's own ratio, not the core's); and the TRISO
//! particles are smeared, which is exactly the heterogeneity rungs 3 and 5
//! put back.
//!
//! # Predictions, written 2026-10-04 BEFORE any run of this program
//!
//! (They were committed after one 2000-particle smoke test of the main case,
//! which printed `k_inf = 1.555 +/- 0.007`; the text below was not changed
//! after it. No point of the sweep had been run when they were committed.)
//!
//! **Main case.** A hand four-factor estimate with 2200 m/s cross sections
//! (sigma_a: U-235 681 b, U-238 2.68 b, C 3.4 mb; nu sigma_f U-235 ~1420 b),
//! the 17 wt% enrichment (17.2 at%) and the ratio above (~770): `eta ~ 2.0`,
//! `f ~ 0.98`, and `p ~ 0.70` from `exp(-N RI_eff / (xi Sigma_s))` with
//! U-238 at `RI_eff ~ 165 b` (the 2026-09-11 table of
//! `u238_resonance_escape.rs`, interpolated to sigma_0 ~ 4400 b) plus U-235's
//! epithermal absorption; `epsilon ~ 1.0-1.1`, mostly U-235 epithermal fission
//! in this convention. So **`k_inf ~ 1.4-1.6`**. The estimate ignores non-1/v
//! thermal behaviour and spectral hardening and is only good to ~10 %.
//!
//! **Step 7 sweep (natural uranium).** The textbook expectation stated in
//! #524: a homogeneous mixture of natural uranium and graphite **never reaches
//! `k_inf = 1` at any ratio**, because U-238 resonance capture is too strong.
//! The shape: at low `N_C/N_U` there is too little moderator per uranium atom
//! and `p` is small; at high `N_C/N_U` carbon's own capture takes the thermal
//! neutrons and `f` falls; so `k_inf` rises, passes a maximum and falls. The
//! same hand estimate as above (natural U: `eta ~ 1.33`; `p` from the
//! 2026-09-11 `RI_eff` table) gives a **maximum of ~0.75-0.8 near
//! `N_C/N_U` ~ 500-800**. A commonly quoted classical figure is a maximum of
//! about 0.85; that number is from memory and was **not** page-checked here,
//! so it is not a reference. What the run gives is reported below whatever it
//! is, including if it contradicts this.
//!
//! # Results
//!
//! Not yet run. (This section is filled in from the runs, with date, commit,
//! settings and hardware.)
//!
//! # Running
//!
//! ```text
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example ugraphite_four_factor               # main case + p check
//! MODE=sweep cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example ugraphite_four_factor               # natural-U k_inf vs N_C/N_U
//! ```
//!
//! Environment (all optional): `PARTICLES` (20000), `INACTIVE` (20), `ACTIVE`
//! (100), `THREADS` (3), `SEED`, `P_HIST` (energy-only histories, 400000),
//! `RATIOS` (comma list for the sweep), `CU` (override the main case's ratio).

#[path = "common/energy_only_slowing_down.rs"]
mod energy_only_slowing_down;

use energy_only_slowing_down::{slow_down, KernelOptions};
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::pebble_beds::fhr_pebble::{homogeneous_cube, TrisoSpec};
use outram_mc_libs::pebble_beds::htr10::{
    u235_atom_fraction, C12_ATOM_FRACTION_OF_NATURAL_C, C13_ATOM_FRACTION_OF_NATURAL_C,
    RHO_BUFFER, RHO_GRAPHITE, RHO_PYC, RHO_SIC, RHO_UO2,
};
use outram_mc_libs::physics::compute::{ComputeType, ThreadCount};
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::reactor_physics::{
    assemble_six_factors, run_keff_reactor_physics, Estimate, ReactorPhysicsConfig,
    ReactorPhysicsReport, SixFactors, CONSISTENCY_BAND,
};
use outram_mc_libs::physics::transport_csg::SourceBox;
use std::time::Instant;

/// Temperature of every material and every nuclide \[K\].
const TEMP_K: f64 = 296.0;
/// Thermal / resonance group boundary \[eV\] (the cadmium cutoff; the module's
/// default).
const THERMAL_CUT_EV: f64 = 0.625;
/// Resonance / fast group boundary \[eV\] (the module's default).
const RES_UPPER_EV: f64 = 1.0e5;
/// Bottom of the tally grid \[eV\]. The module's default of 1e-3 eV would drop
/// the slowest thermal neutrons from the tallies (off-grid tracks are not
/// scored), and at 296 K those carry a visible share of the 1/v absorption.
const E_MIN_EV: f64 = 1.0e-5;
/// Top of the tally grid \[eV\].
const E_MAX_EV: f64 = 2.0e7;
/// Fine bins: ~1000 per decade over 1e-5 .. 2e7 eV.
const N_FINE: usize = 12_300;
/// Half-width of the reflective cube \[cm\]. Irrelevant to `k_inf` (no
/// leakage); large enough that a flight rarely crosses a face.
const HALF_CM: f64 = 50.0;

/// Natural uranium, atom fractions: IUPAC representative isotopic composition
/// (U-234 0.0054 %, U-235 0.7204 %, U-238 99.2742 %). Recalled, not
/// page-checked in this change; the sweep's conclusion does not hinge on the
/// fourth digit.
const NAT_U: [f64; 3] = [0.000_054, 0.007_204, 0.992_742];

/// Avogadro's number times 1e-24 \[atoms cm2 / (mol barn)\].
const NA_B: f64 = 0.602_214_076;
const M_U235: f64 = 235.043_930;
const M_U238: f64 = 238.050_788;
const M_O16: f64 = 15.994_914_6;
const M_C: f64 = 12.011;
const M_SI: f64 = 28.0855;

/// Indices into the nuclide array.
mod nx {
    pub const U234: usize = 0;
    pub const U235: usize = 1;
    pub const U238: usize = 2;
    pub const URANIUM: [usize; 3] = [U234, U235, U238];
}

/// A homogeneous U + C mixture \[atoms/barn-cm\].
#[derive(Debug, Clone, Copy)]
struct Mix {
    u: [f64; 3],
    c: f64,
}

impl Mix {
    fn c_per_u(&self) -> f64 {
        self.c / self.u.iter().sum::<f64>()
    }

    fn densities(&self) -> [f64; 5] {
        [
            self.u[0],
            self.u[1],
            self.u[2],
            C12_ATOM_FRACTION_OF_NATURAL_C * self.c,
            C13_ATOM_FRACTION_OF_NATURAL_C * self.c,
        ]
    }

    fn material(&self, name: &str) -> Material {
        Material {
            id: 1,
            name: name.into(),
            temperature: TEMP_K,
            components: self
                .densities()
                .iter()
                .enumerate()
                .filter(|(_, d)| **d > 0.0)
                .map(|(i, d)| NuclideComponent {
                    nuclide_idx: i,
                    atom_density: *d,
                })
                .collect(),
        }
    }
}

/// One HTR-10 fuel pebble's uranium and carbon, smeared over the ball. See the
/// module docs for the inventory and the liberties.
fn htr10_pebble_mix() -> Mix {
    let spec = TrisoSpec::HTR10_LI2014;
    let (r_fz, r_peb) = (2.5_f64, 3.0_f64);
    let ball = |r: f64| 4.0 / 3.0 * std::f64::consts::PI * r.powi(3);
    let v = spec.layer_volumes(); // kernel, buffer, IPyC, SiC, OPyC
    let v_particle: f64 = v.iter().sum();
    let n_particles = (spec.packing_fraction * ball(r_fz) / v_particle).round();
    let v_matrix = ball(r_fz) - n_particles * v_particle;
    let v_shell = ball(r_peb) - ball(r_fz);

    let x5 = u235_atom_fraction();
    let m_u = x5 * M_U235 + (1.0 - x5) * M_U238;
    let mol_u = n_particles * v[0] * RHO_UO2 / (m_u + 2.0 * M_O16);
    let mol_c = (v_matrix + v_shell) * RHO_GRAPHITE / M_C
        + n_particles
            * (v[1] * RHO_BUFFER / M_C
                + (v[2] + v[4]) * RHO_PYC / M_C
                + v[3] * RHO_SIC / (M_C + M_SI));
    let v_peb = ball(r_peb);
    let n_u = mol_u * NA_B / v_peb;
    println!(
        "HTR-10 fuel pebble inventory: {n_particles:.0} particles, U {:.4} g, C {:.2} g \
         -> N_C/N_U = {:.1}",
        mol_u * m_u,
        mol_c * M_C,
        mol_c / mol_u
    );
    Mix {
        u: [0.0, x5 * n_u, (1.0 - x5) * n_u],
        c: mol_c * NA_B / v_peb,
    }
}

/// Natural uranium in graphite at 1.73 g/cm3 carbon, `c_per_u` carbon atoms
/// per uranium atom. (In an infinite homogeneous medium `k_inf` depends only on
/// the ratios, not on the absolute density.)
fn natural_mix(c_per_u: f64) -> Mix {
    let n_c = RHO_GRAPHITE * NA_B / M_C;
    let n_u = n_c / c_per_u;
    Mix {
        u: [NAT_U[0] * n_u, NAT_U[1] * n_u, NAT_U[2] * n_u],
        c: n_c,
    }
}

fn env_or<T: std::str::FromStr>(k: &str, d: T) -> T {
    std::env::var(k)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(d)
}

fn load_nuclides() -> Vec<Nuclide> {
    use njoy_outram_park_fork::reference_data::reference_endf;
    let load = |name: &str, file: &str| {
        let p = reference_endf(file).unwrap_or_else(|| panic!("reference tape {file} not found"));
        let t0 = Instant::now();
        let n = Nuclide::from_endf_file(&p, name, TEMP_K, 1.0e-3)
            .unwrap_or_else(|e| panic!("from_endf_file({}): {e}", p.display()));
        eprintln!("  {name:<5} RECONR+BROADR @ {TEMP_K} K: {:.1?}", t0.elapsed());
        n
    };
    let tsl = reference_endf("tsl-crystalline-graphite.endf").expect("graphite S(a,b) tape");
    let sab = ThermalScattering::from_endf_file(tsl.to_str().expect("path"), 30, TEMP_K, "c_Graphite")
        .expect("crystalline-graphite S(a,b) at 296 K");
    vec![
        load("U234", "n-092_U_234-ENDF8.0.endf"),
        load("U235", "n-092_U_235-ENDF8.0.endf"),
        load("U238", "n-092_U_238.endf"),
        load("C12", "n-006_C_012-ENDF8.0.endf").with_thermal_scattering(sab.clone()),
        load("C13", "n-006_C_013-ENDF8.0.endf").with_thermal_scattering(sab),
    ]
}

/// What one case produced.
struct CaseResult {
    report: ReactorPhysicsReport,
    /// Six factors with the thermal fuel absorption split by nuclide.
    factors: SixFactors,
    /// Fold of the total Sigma_a over the thermal flux bins, over the tallied
    /// thermal absorption (binning check; ideally 1).
    fold_check: f64,
    /// Uranium share of the thermal absorption, from the fold.
    u_share: f64,
    wall_s: f64,
}

fn run_case(label: &str, mix: Mix, nuclides: &[Nuclide], seed: u64) -> CaseResult {
    let threads: usize = env_or("THREADS", 3);
    let cfg = ReactorPhysicsConfig {
        keff: KeffSettings {
            n_particles: env_or("PARTICLES", 20_000),
            n_inactive: env_or("INACTIVE", 20),
            n_active: env_or("ACTIVE", 100),
            temperature_k: TEMP_K,
            seed,
            compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(threads)),
            ..KeffSettings::default()
        },
        source_box: SourceBox {
            lower: Position::new(-HALF_CM, -HALF_CM, -HALF_CM),
            upper: Position::new(HALF_CM, HALF_CM, HALF_CM),
        },
        thermal_cutoff_ev: THERMAL_CUT_EV,
        resonance_upper_ev: RES_UPPER_EV,
        n_fine_bins: N_FINE,
        energy_min_ev: E_MIN_EV,
        energy_max_ev: E_MAX_EV,
    };
    let geom = homogeneous_cube(HALF_CM, 0, TEMP_K);
    let mats = vec![mix.material(label)];
    let t0 = Instant::now();
    let report = run_keff_reactor_physics(&geom, &mats, nuclides, &cfg).expect("reactor physics");
    let wall_s = t0.elapsed().as_secs_f64();

    // Nuclide split of the thermal absorption (see the module docs).
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
    let sf = &report.six_factors;
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

fn print_case(label: &str, mix: Mix, r: &CaseResult) {
    let k = &r.report.keff;
    let f = &r.factors;
    let e = |x: Estimate| format!("{:.5} +/- {:.5}", x.mean, x.std);
    println!("\n=== {label} ===");
    println!(
        "N_C/N_U = {:.1}; N [atoms/b-cm]: U234 {:.4e}  U235 {:.4e}  U238 {:.4e}  C {:.5e}",
        mix.c_per_u(),
        mix.u[0],
        mix.u[1],
        mix.u[2],
        mix.c
    );
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
    let p: f64 = f.production_by_group.iter().map(|x| x.mean).sum();
    println!(
        "  P_total/A_total = {:.5} (telescoping: product - P/A = {:+.2e})",
        p / a,
        f.k_from_factors.mean - p / a
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
    println!(
        "nuclide split of thermal absorption: uranium share {:.5}; fold(Sigma_a,tot)/tally = {:.6}",
        r.u_share, r.fold_check
    );
}

fn main() {
    let mode = std::env::var("MODE").unwrap_or_else(|_| "main".into());
    let seed: u64 = env_or("SEED", 20_261_004);
    eprintln!("Reconstructing nuclides (ENDF/B-VIII.0, {TEMP_K} K):");
    let t_load = Instant::now();
    let nuclides = load_nuclides();
    eprintln!("data ready in {:.1} s", t_load.elapsed().as_secs_f64());
    println!(
        "settings: PARTICLES {} x [INACTIVE {} + ACTIVE {}], THREADS {}, SEED {seed}, {TEMP_K} K, \
         {N_FINE} fine bins over [{E_MIN_EV:e}, {E_MAX_EV:e}] eV",
        env_or::<usize>("PARTICLES", 20_000),
        env_or::<usize>("INACTIVE", 20),
        env_or::<usize>("ACTIVE", 100),
        env_or::<usize>("THREADS", 3),
    );

    match mode.as_str() {
        "sweep" => sweep(&nuclides, seed),
        _ => main_case(&nuclides, seed),
    }
}

fn main_case(nuclides: &[Nuclide], seed: u64) {
    let mut mix = htr10_pebble_mix();
    if let Ok(cu) = std::env::var("CU") {
        let cu: f64 = cu.parse().expect("CU");
        let n_u: f64 = mix.c / cu;
        let s: f64 = mix.u.iter().sum();
        mix.u = [mix.u[0] / s * n_u, mix.u[1] / s * n_u, mix.u[2] / s * n_u];
    }
    let label = "homogeneous U(17 wt%) + graphite at the HTR-10 pebble ratio";
    let r = run_case(label, mix, nuclides, seed);
    print_case(label, mix, &r);

    // ── Check 2: p against the energy-only walk ─────────────────────────────
    let p_hist: usize = env_or("P_HIST", 400_000);
    let mut s = seed ^ 0x0E0E_0E0E;
    let t0 = Instant::now();
    let c = slow_down(
        nuclides,
        &mix.densities(),
        TEMP_K,
        RES_UPPER_EV,
        THERMAL_CUT_EV,
        p_hist,
        &mut s,
        KernelOptions::default(),
    );
    let p_eo = c.escaped as f64 / c.histories as f64;
    let sig = (p_eo * (1.0 - p_eo) / c.histories as f64).sqrt();
    let p_tr = r.factors.p;
    println!(
        "\n=== p check: energy-only walk, 100 keV -> 0.625 eV, same mixture, {p_hist} histories \
         [{:.1} s] ===",
        t0.elapsed().as_secs_f64()
    );
    println!(
        "  p (energy-only escape)  = {p_eo:.5} +/- {sig:.5}\n  p (transport, 3-group)  = {:.5} +/- {:.5}\n  \
         difference transport - energy-only = {:+.5} ({:+.1} sigma combined)",
        p_tr.mean,
        p_tr.std,
        p_tr.mean - p_eo,
        (p_tr.mean - p_eo) / (p_tr.std.powi(2) + sig * sig).sqrt()
    );
    let names = ["U234", "U235", "U238", "C12", "C13"];
    let per: Vec<String> = c
        .absorbed_by
        .iter()
        .zip(names)
        .map(|(n, nm)| format!("{nm} {:.5}", *n as f64 / c.histories as f64))
        .collect();
    println!("  energy-only absorption per source neutron by nuclide: {}", per.join(", "));
}

fn sweep(nuclides: &[Nuclide], seed: u64) {
    let ratios: Vec<f64> = std::env::var("RATIOS")
        .ok()
        .map(|s| s.split(',').map(|x| x.trim().parse().expect("RATIOS")).collect())
        .unwrap_or_else(|| {
            vec![
                50.0, 100.0, 200.0, 300.0, 400.0, 500.0, 600.0, 800.0, 1000.0, 1500.0, 2500.0,
            ]
        });
    let mut rows = Vec::new();
    for (i, &cu) in ratios.iter().enumerate() {
        let mix = natural_mix(cu);
        let label = format!("natural U + graphite, N_C/N_U = {cu}");
        let r = run_case(&label, mix, nuclides, seed + i as u64);
        print_case(&label, mix, &r);
        rows.push((cu, r));
    }
    println!("\n=== Step 7 sweep: natural uranium, homogeneous, {TEMP_K} K ===");
    println!(
        "{:>8} {:>9} {:>8} {:>8} {:>8} {:>8} {:>8} {:>9} {:>8}",
        "N_C/N_U", "k_inf", "sigma", "eta", "f", "p", "eps", "k_factors", "wall s"
    );
    for (cu, r) in &rows {
        let f = &r.factors;
        println!(
            "{cu:>8.0} {:>9.5} {:>8.5} {:>8.5} {:>8.5} {:>8.5} {:>8.5} {:>9.5} {:>8.1}",
            r.report.keff.k_mean,
            r.report.keff.k_std,
            f.eta.mean,
            f.f.mean,
            f.p.mean,
            f.epsilon.mean,
            f.k_from_factors.mean,
            r.wall_s
        );
    }
    let best = rows
        .iter()
        .max_by(|a, b| a.1.report.keff.k_mean.total_cmp(&b.1.report.keff.k_mean))
        .expect("rows");
    println!(
        "maximum on this grid: k_inf = {:.5} +/- {:.5} at N_C/N_U = {}; reaches 1: {}",
        best.1.report.keff.k_mean,
        best.1.report.keff.k_std,
        best.0,
        rows.iter().any(|(_, r)| r.report.keff.k_mean >= 1.0)
    );
}
