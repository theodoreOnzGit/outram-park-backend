//! Mean log energy decrement **above** the S(α,β) cutoff — the part of the
//! slowing-down range where resonance escape is actually decided, and the part
//! `graphite_energy_decrement.rs` never reached.
//!
//! # Why this exists
//!
//! The FHR pebble sits ~+1.7 % above the OpenMC reference, with `p` too high and
//! `ε` too low — a spectrum that is too soft. Every named data mechanism is
//! excluded by measurement: point σ against NJOY's PENDF (±0.04 %), the capture
//! **resonance integral** against NJOY *and* the published RI_∞
//! (`u238_resonance_integral.rs`, +0.00 %), graphite σ against THERMR (±0.05 %).
//!
//! What was measured of the *kernel* was `ξ = ⟨ln(E/E′)⟩` from 0.0253 eV to
//! **3.9 eV** — entirely inside the bound-atom regime, where
//! `ThermalScattering::sample` runs. But U-238's resonances span **6 eV to
//! 20 keV**, and there the transport kernel takes a completely different branch:
//! `sample_thermal` returns `None` and the collision goes through
//! `sample_elastic_mu_cm` / `elastic_scatter`, i.e. target-at-rest two-body
//! kinematics. That branch has never been measured.
//!
//! It is exactly the branch that would produce the observed signature. Too much
//! energy per collision above the cutoff means fewer collisions inside the
//! resonance band, so fewer resonance captures (`p` up), a softer fast spectrum
//! (`ε` down) and a higher `k` — all three, from one defect.
//!
//! The per-collision kinematics turned out to be exact (`ξ/ξ₀ = 1.000` from 4 eV
//! to 10 keV, no kinematic-floor violations). The defect was one level up, and
//! only the second section below can see it.
//!
//! # The oracle is analytic, so no library is needed
//!
//! For isotropic-CM elastic scattering off a target at rest,
//!
//! ```text
//! α   = ((A−1)/(A+1))²
//! ξ   = 1 + α·ln α / (1 − α)          ⟨E′/E⟩ = (1 + α)/2
//! ```
//!
//! with `A` the nuclide's own AWR. Anisotropy in the CM shifts `ξ` by a small
//! amount proportional to `μ̄_cm`, which for carbon is well under 1 % below
//! ~100 keV — so a multi-percent departure is a defect, not anisotropy. The
//! floor `α·E` is a hard kinematic bound: **no** elastic collision may drop the
//! neutron below it, and a violation is unambiguous.
//!
//! # What each section measures
//!
//! **Section 1 — per-collision kinematics vs the analytic oracle.** The bound law
//! where a nuclide has one, otherwise the *target-at-rest* two-body form, which is
//! the branch the oracle above describes exactly. Every non-thermal row must land
//! on `ξ₀` and must respect the `α·E` floor.
//!
//! **Section 2 — the thermal fixed point.** The full transport branch, free-gas
//! target motion included (`free_gas_elastic_scatter`, gated at `400·kT` as
//! OpenMC gates it). Section 1 cannot see what this measures: each individual
//! target-at-rest collision is valid, and yet the sequence of them has **no
//! equilibrium** — held-at-rest nuclei can only take energy away, so a neutron
//! random walk cools without bound. That was the defect (bead `op-50vu`), and
//! this section is where it shows and where the fix is checked.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example epithermal_slowing_down
//! ```

use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::physics::scatter::{
    elastic_scatter, free_gas_elastic_scatter, two_body_scatter_with_mu, K_BOLTZMANN_EV_PER_K,
};
use outram_mc_libs::geometry::position::Direction;

const TEMP: f64 = 600.0;
const N: usize = 200_000;

/// Energies spanning the thermal range, the S(α,β) seam and the whole U-238
/// resonance band. `kT = 0.0517 eV` at 600 K, so the first three rows are inside
/// the Maxwellian where a free-gas target's own motion dominates.
const PROBE_EV: &[f64] = &[
    0.0253, 0.0517, 0.2, 1.0, 2.0, 3.9, 4.1, 5.0, 6.674, 10.0, 20.87, 100.0, 1.0e3, 1.0e4, 1.0e5,
    1.0e6, 5.0e6,
];

/// OpenMC's `FREE_GAS_THRESHOLD` — below `400·kT` a scatter off a nuclide with no
/// S(α,β) table must sample the target's thermal motion.
const FREE_GAS_THRESHOLD: f64 = 400.0;

fn main() {
    let dir = endf_dir();
    let sab = ThermalScattering::from_endf_file(
        dir.join("tsl-crystalline-graphite.endf")
            .to_str()
            .expect("path"),
        30,
        TEMP,
        "c_Graphite",
    )
    .expect("crystalline-graphite S(a,b)");

    // The moderating nuclides of the pebble, plus U-238 as the heavy control:
    // its alpha is 0.983, so a correct kernel can barely move it.
    let cases: Vec<(String, Nuclide)> = vec![
        (
            "C12 (graphite S(a,b))".into(),
            load("C12", "n-006_C_012-ENDF8.0.endf", &dir).with_thermal_scattering(sab.clone()),
        ),
        (
            "C12 (free gas, kernel C)".into(),
            load("C12", "n-006_C_012-ENDF8.0.endf", &dir),
        ),
        (
            "Be9  (FLiBe)".into(),
            load("Be9", "n-004_Be_009-ENDF8.0.endf", &dir),
        ),
        (
            "F19  (FLiBe)".into(),
            load("F19", "n-009_F_019-ENDF8.0.endf", &dir),
        ),
        (
            "Li7  (FLiBe)".into(),
            load("Li7", "n-003_Li_007-ENDF8.0.endf", &dir),
        ),
        (
            "O16  (UCO kernel)".into(),
            load("O16", "n-008_O_016-ENDF8.0.endf", &dir),
        ),
        (
            "Si28 (SiC coating)".into(),
            load("Si28", "n-014_Si_028-ENDF8.0.endf", &dir),
        ),
        (
            "U238 (heavy control)".into(),
            load("U238", "n-092_U_238.endf", &dir),
        ),
    ];

    let mut seed = 20_260_911_u64;
    let mut any_violation = false;

    for (label, nuc) in &cases {
        let a = nuc.awr;
        let alpha = ((a - 1.0) / (a + 1.0)).powi(2);
        let xi_iso = 1.0 + alpha * alpha.ln() / (1.0 - alpha);
        let ratio_iso = 0.5 * (1.0 + alpha);

        println!("\n=== {label}   AWR = {a:.5}   alpha = {alpha:.6} ===");
        println!(
            "isotropic-CM, target-at-rest oracle:  xi = {xi_iso:.6}   <E'/E> = {ratio_iso:.6}"
        );
        println!(
            "{:>10} {:>7} {:>11} {:>11} {:>9} {:>9} {:>8} {:>9}",
            "E [eV]", "thermal", "<E'/E>", "xi", "xi/xi_0", "ratio/r0", "up-sc %", "below aE"
        );

        for &e in PROBE_EV {
            let (mut sum_ratio, mut sum_xi) = (0.0_f64, 0.0_f64);
            let (mut n_up, mut n_thermal, mut n_below) = (0usize, 0usize, 0usize);
            let u = Direction::new(0.0, 0.0, 1.0);
            for _ in 0..N {
                // Exactly the branch order of `keff_delta::simulate` and
                // `transport_csg`: bound law if the nuclide has one and it
                // covers this energy, otherwise the elastic kernel.
                let ep = if let Some((e_out, _mu)) = nuc.sample_thermal(e, &mut seed) {
                    n_thermal += 1;
                    e_out
                } else {
                    match nuc.sample_elastic_mu_cm(e, &mut seed) {
                        Some(mu_cm) => two_body_scatter_with_mu(e, u, a, 0.0, mu_cm, &mut seed).0,
                        None => elastic_scatter(e, u, a, &mut seed).0,
                    }
                };
                if ep <= 0.0 {
                    continue;
                }
                sum_ratio += ep / e;
                sum_xi += (e / ep).ln();
                if ep > e {
                    n_up += 1;
                }
                // Kinematic floor. Only meaningful outside the bound regime --
                // the S(a,b) law legitimately breaks it via lattice recoil.
                if ep < alpha * e * (1.0 - 1.0e-9) {
                    n_below += 1;
                }
            }
            let k = N as f64;
            let (ratio, xi) = (sum_ratio / k, sum_xi / k);
            let th_frac = 100.0 * n_thermal as f64 / k;
            let below = 100.0 * n_below as f64 / k;
            // A free-kinematics violation is a hard defect; inside the bound
            // regime it is expected, so only flag it where thermal is off.
            if th_frac < 1.0 && below > 0.01 {
                any_violation = true;
            }
            println!(
                "{e:>10.4e} {th_frac:>6.1}% {ratio:>11.5} {xi:>11.5} {:>9.4} {:>9.4} {:>7.1} {below:>8.2}%",
                xi / xi_iso,
                ratio / ratio_iso,
                100.0 * n_up as f64 / k,
            );
        }
    }

    thermalization_demo(&cases);

    println!(
        "\nHow to read this.\n\
         - `thermal` is the share of collisions that took the S(a,b) branch. Where\n\
           it is 0 the row is pure two-body kinematics and the oracle is exact.\n\
         - `xi/xi_0` and `ratio/r0` must both be 1.000 on those rows, up to the CM\n\
           anisotropy of the evaluation (sub-percent for carbon below ~100 keV,\n\
           growing in the MeV range where forward scattering sets in -- there\n\
           `ratio/r0` rises above 1 and `xi/xi_0` falls below, which is correct).\n\
         - `below aE` must be 0.00 % on every non-thermal row: alpha*E is a hard\n\
           kinematic floor for elastic scattering off a target at rest.\n\
         - `up-sc %` must be 0.0 on every non-thermal row for the same reason."
    );
    if any_violation {
        println!("\nFAIL: kinematic floor violated outside the bound regime.");
        std::process::exit(1);
    }
    println!("\nNo kinematic-floor violations outside the bound regime.");
}

/// Where does a neutron end up after many scatters in a single-nuclide medium at
/// 600 K?
///
/// This is the question the per-collision table cannot answer, because the defect
/// it looks for is not in any one collision — each is a valid target-at-rest
/// scatter — but in the **absence of an equilibrium**. A real free gas at
/// temperature `T` up-scatters as well as down-scatters, and a neutron random-
/// walking in it relaxes to a Maxwellian with `⟨E⟩ = 1.5·kT` and stays there. A
/// target held at rest can only take energy away, so the walk has no fixed point
/// and `E → 0` without bound.
///
/// Graphite is the control: it carries the S(α,β) law, so it must equilibrate.
fn thermalization_demo(cases: &[(String, Nuclide)]) {
    const SCATTERS: usize = 400;
    const WALKERS: usize = 20_000;
    let kt = K_BOLTZMANN_EV_PER_K * TEMP;
    // The walk visits COLLISIONS, not the neutron density, so its fixed point is
    // the collision density n(E)*v*sigma ~ E*exp(-E/kT) (constant sigma, the CXS
    // approximation) -- a Gamma(2, kT) with mean 2kT, not the density's 1.5 kT.
    let e_fixed_point = 2.0 * kt;

    println!(
        "\n\n=== Thermal equilibrium: {WALKERS} walkers x {SCATTERS} scatters, start 1 eV ==="
    );
    println!(
        "kT = {kt:.6} eV at {TEMP} K;  a free gas must relax to <E> = 2 kT = {e_fixed_point:.6} eV"
    );
    println!(
        "free-gas target motion is required below 400 kT = {:.3} eV",
        FREE_GAS_THRESHOLD * kt
    );
    println!(
        "\n{:<28} {:>14} {:>14} {:>12}",
        "medium", "<E> final [eV]", "<E>/2kT", "median [eV]"
    );

    let mut seed = 77_030_412_u64;
    for (label, nuc) in cases {
        let a = nuc.awr;
        let mut finals: Vec<f64> = Vec::with_capacity(WALKERS);
        let u = Direction::new(0.0, 0.0, 1.0);
        for _ in 0..WALKERS {
            let mut e = 1.0_f64;
            for _ in 0..SCATTERS {
                e = if let Some((e_out, _mu)) = nuc.sample_thermal(e, &mut seed) {
                    e_out
                } else {
                    // The transport kernel's own branch, so this measures what
                    // the Monte Carlo does, fix included.
                    let mu_cm = nuc
                        .sample_elastic_mu_cm(e, &mut seed)
                        .unwrap_or_else(|| 2.0 * outram_mc_libs::rng::lcg::prn(&mut seed) - 1.0);
                    free_gas_elastic_scatter(e, u, a, kt, mu_cm, &mut seed).0
                };
                if !(e > 0.0) || !e.is_finite() {
                    e = f64::MIN_POSITIVE;
                    break;
                }
            }
            finals.push(e);
        }
        finals.sort_by(|x, y| x.partial_cmp(y).unwrap());
        let mean = finals.iter().sum::<f64>() / finals.len() as f64;
        let median = finals[finals.len() / 2];
        println!(
            "{label:<28} {mean:>14.4e} {:>14.4e} {median:>12.4e}",
            mean / e_fixed_point
        );
    }
    println!(
        "\nA medium whose `<E>/2kT` is not order 1 has no thermal equilibrium: its\n\
         nuclei are being held at rest, so every collision removes energy and the\n\
         random walk never turns around."
    );
}

fn endf_dir() -> std::path::PathBuf {
    njoy_outram_park_fork::reference_data::reference_endf("n-006_C_012-ENDF8.0.endf")
        .expect("reference-data/endf/")
        .parent()
        .expect("parent")
        .to_path_buf()
}

fn load(name: &str, file: &str, dir: &std::path::Path) -> Nuclide {
    let p = dir.join(file);
    eprint!("  reconstructing {name:<6} … ");
    let t0 = std::time::Instant::now();
    let n = Nuclide::from_endf_file(&p, name, TEMP, 1.0e-3)
        .unwrap_or_else(|e| panic!("from_endf_file({}): {e}", p.display()));
    eprintln!("{:.1?}", t0.elapsed());
    n
}
