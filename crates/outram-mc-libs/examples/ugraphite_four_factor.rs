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
//! ## Main case (2026-10-04, `develop` at `c199b7dc1f`)
//!
//! 20 000 neutrons x [20 inactive + 100 active], seed 20 261 004, 2 threads,
//! 12 300 fine bins over 1e-5 eV .. 20 MeV. Hardware: i9-13900K (16 logical
//! cores, 62 GB, Linux, CPU only), cores pinned with `taskset`, machine shared
//! with other jobs. Data 90 s (RECONR + BROADR of five tapes), transport
//! 819 s, energy-only `p` check 82 s.
//!
//! ```text
//!   N_C/N_U = 767.2   N: U-235 1.9256e-5, U-238 9.2829e-5, C 8.59914e-2 /b-cm
//!   k_inf (generation mean)        1.56777 +/- 0.00081
//!   eta                            2.02809 +/- 0.00132
//!   f   (nuclide split)            0.97470 +/- 0.00063
//!   p   (3-group)                  0.71398 +/- 0.00044
//!   epsilon (3-group)              1.11144 +/- 0.00093
//!   eta f p epsilon                1.56867 +/- 0.00256   = P_total/A_total to 2e-16
//!   (k - k_factors)/k              -0.00057              inside the consistency band
//!   two-group view: eta 2.02809  f 0.97470  p 0.71052  epsilon 1.11686
//!   absorption per source neutron [thermal, resonance, fast] 0.71010 0.28447 0.00485
//!   binning check: fold(Sigma_a,tot) / tallied thermal absorption = 0.999989
//! ```
//!
//! - **Telescoping holds exactly** (product minus `P/A` = 2e-16), and `k` from
//!   the power iteration agrees with the factors' product to 0.06 %, inside
//!   the band (the factors' quoted sigma is conservative, see the module docs).
//! - **The prediction held**: `k_inf ~ 1.4-1.6` predicted, 1.568 measured;
//!   each factor was inside its predicted range (`eta ~ 2.0`, `f ~ 0.98`,
//!   `p ~ 0.70`, `epsilon ~ 1.0-1.1`).
//! - **`p` against the energy-only walk:** 0.71274 +/- 0.00072 (400 000
//!   histories from 100 keV) against the transport's 0.71398 +/- 0.00044,
//!   **+0.00124, 1.5 sigma**. Agreement at the level the two definitions allow
//!   (the transport's resonance group also holds the ~1 % of fission neutrons
//!   born below 100 keV, which escape more easily); not a resolved difference.
//! - **The two conventions differ as they should**: `p` 0.714 (3-group) vs
//!   0.711 (2-group), `epsilon` 1.111 vs 1.117, same product. Quote the
//!   3-group values with the library's definitions; the 2-group view is for
//!   comparing with decks that use it.
//!
//! **OpenMC code-to-code** (verification, not validation): the same mixture
//! in OpenMC 0.16.1-dev25 (`d7d3284a1`) on the openmc.org ENDF/B-VIII.0 HDF5
//! library (neutron data at 294 K by nearest-temperature lookup, `c_Graphite`
//! at 296 K; outram-mc broadens to 296 K exactly), ptables and DBRC on, same
//! histories, seed 1, 1 thread, 316 s:
//!
//! ```text
//!                       outram-mc            OpenMC                 difference
//!   k (gen. mean)       1.56777 +/- 0.00081  1.56711 +/- 0.00134    +66 +/- 157 pcm (0.4 sigma)
//!   k (OpenMC combined)                      1.56807 +/- 0.00066    -30 +/- 104 pcm (0.3 sigma)
//!   eta                 2.02809              2.02786                +0.01 %
//!   f                   0.97470              0.97471                -0.00 %
//!   p                   0.71398 +/- 0.00044  0.71301 +/- 0.00084    +0.14 % (1.0 sigma)
//!   epsilon             1.11144              1.11272                -0.12 %
//! ```
//!
//! **Agreement within statistics on `k` and on every factor.** The 2 K
//! temperature difference (296 K here, 294 K neutron data there) is not
//! corrected; with a Doppler coefficient of a few pcm/K it is below the
//! statistics. Record and deck:
//! `verification_and_validation/tutorial_rung2/README.md`.
//!
//! ## Step 7 sweep (natural uranium), 2026-10-05
//!
//! `MODE=sweep`, binary built from `develop` at `f39501b8bd` (the commits
//! after it in this series change comments and an off-by-default ablation
//! knob only). **5000 neutrons x [20 inactive + 50 active]** per point, seed
//! 20 261 004 + point index, 2 threads; sized for the 2 cores available
//! (a ~7 min point gives sigma ~ 200 pcm, small against the thousands of pcm
//! the curve spans). Hardware: Intel Xeon @ 2.10 GHz, 2 threads pinned to 2 of
//! 4 logical cores, 15 GB, Linux, CPU only; the machine was shared with
//! another agent's jobs and this session's own builds and browser checks
//! (load average up to ~8), so the wall times are upper bounds. Data
//! 112 s; the run 2026-10-05 00:09-02:07 UTC.
//!
//! ```text
//!  N_C/N_U   k_inf +/- sigma      eta      f        p        epsilon  wall s
//!      50   0.49117 0.00185    1.31230  0.97457  0.30725  1.23959    153
//!     100   0.61994 0.00197    1.32213  0.95077  0.45311  1.09216    190
//!     200   0.72553 0.00229    1.32821  0.90663  0.58182  1.03976    220
//!     300   0.76517 0.00169    1.33054  0.86645  0.64808  1.02534    366
//!     400   0.77850 0.00217    1.33178  0.82970  0.69192  1.01853    476
//!     500   0.77774 0.00254    1.33253  0.79594  0.72371  1.01491    633
//!     600   0.77153 0.00243    1.33304  0.76483  0.74684  1.01248    707
//!     800   0.74798 0.00213    1.33368  0.70937  0.78376  1.00942    832
//!    1000   0.71898 0.00219    1.33410  0.66142  0.80868  1.00772    806
//!    1500   0.64640 0.00196    1.33464  0.56580  0.85134  1.00559   1129
//!    2500   0.52621 0.00193    1.33507  0.43890  0.89470  1.00400   1422
//! ```
//!
//! Factor sigmas are 0.0008-0.0033 (eta), 0.0003-0.0024 (f), 0.0006-0.0009
//! (p), 0.0010-0.0037 (epsilon), largest at the densest uranium. At every point the product telescopes to
//! `P/A` exactly, `(k - k_factors)/k` is inside the band (-0.4 % .. +0.8 %),
//! and the binning check is 0.999992-0.999993.
//!
//! **Against the prediction written before the run:**
//! - **Never reaches `k_inf = 1`: held.** The maximum on the grid is
//!   0.77850 +/- 0.00217 at `N_C/N_U = 400`, ~100 sigma below 1.
//! - **Rises, peaks, falls, with `p` rising and `f` falling: held.** `p` goes
//!   0.31 -> 0.89 and `f` 0.97 -> 0.44 across the grid; `eta` is 1.31-1.34
//!   throughout (predicted ~1.33).
//! - **Maximum ~0.75-0.8: held** (0.7785).
//! - **Location ~500-800: partly refuted.** The top is flat between 400
//!   (0.77850) and 500 (0.77774, 0.2 sigma apart); 600 is already 2.2 sigma
//!   lower and 800 is 0.748. The optimum is at the low edge of the predicted
//!   range or below it: my hand `p` was too low at low ratios, so it put the
//!   balance point too far towards dilution. Not investigated further.
//! - The fast fission factor grows strongly when the uranium is dense
//!   (`epsilon` 1.24 at `N_C/N_U = 50`), which the hand estimate did not
//!   consider; it is a real feature (most absorption there is in the
//!   resonance group, so the thermal fissions epsilon divides by are few).
//!
//! **OpenMC code-to-code for the sweep: not run** (no OpenMC on the machine
//! that ran it). The deck supports it unchanged:
//! `verification_and_validation/tutorial_rung2/openmc_inputs/ugraphite_openmc.py --case natural --cu R`.
//! Pending the maintainer.
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
#[path = "common/ugraphite_common.rs"]
mod ugraphite_common;

use energy_only_slowing_down::{slow_down, KernelOptions};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::pebble_beds::fhr_pebble::homogeneous_cube;
use std::time::Instant;
use ugraphite_common::{
    env_or, htr10_pebble_mix, load_nuclides, natural_mix, print_case, run_case, FuelSplit, Mix,
    CaseResult, E_MAX_EV, E_MIN_EV, N_FINE, RES_UPPER_EV, TEMP_K, THERMAL_CUT_EV,
};

// ANCHOR: model (mdBook include, Monte Carlo tutorial rung 2, gh:#524)
/// Half-width of the reflective cube \[cm\]. Irrelevant to `k_inf` (no
/// leakage); large enough that a flight rarely crosses a face.
const HALF_CM: f64 = 50.0;

/// The homogeneous medium: one material filling the reflective cube.
fn run_homogeneous(label: &str, mix: Mix, nuclides: &[Nuclide], seed: u64) -> CaseResult {
    let geom = homogeneous_cube(HALF_CM, 0, TEMP_K);
    let mats = vec![mix.material(1, label)];
    run_case(&geom, &mats, nuclides, seed, HALF_CM, FuelSplit::ByNuclide(mix))
}
// ANCHOR_END: model

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
    let r = run_homogeneous(label, mix, nuclides, seed);
    print_case(label, mix.c_per_u(), &r);
    println!(
        "N [atoms/b-cm]: U234 {:.4e}  U235 {:.4e}  U238 {:.4e}  C {:.5e}",
        mix.u[0], mix.u[1], mix.u[2], mix.c
    );

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
        let r = run_homogeneous(&label, mix, nuclides, seed + i as u64);
        print_case(&label, mix.c_per_u(), &r);
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
