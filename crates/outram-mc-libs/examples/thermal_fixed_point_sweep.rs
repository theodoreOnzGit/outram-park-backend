//! **Does every bound thermal law this repo carries relax onto its own
//! temperature? — the fixed-point oracle across all eight S(α,β) evaluations.**
//!
//! # Why this exists
//!
//! `tests/thermal_kernel_stationary_distribution.rs` runs the fixed-point oracle
//! on the two laws the benchmarks use, `c_H_in_H2O` and `c_Graphite`, and finds
//! both displaced. This runs the identical measurement on **every** `tsl-*`
//! evaluation in `reference-data/endf/`, to answer a question the two-material
//! version cannot: is the displacement a property of those two evaluations, or of
//! the processing every evaluation goes through?
//!
//! It needs **no external data**. A scattering kernel that satisfies detailed
//! balance relaxes a neutron population onto the Maxwellian of its own
//! temperature, whatever the material — so the reference value is
//! `⟨E⟩ = 1.5 kT` and `⟨E²⟩/⟨E⟩² = 5/3` exactly, for all eight, and any
//! departure is the law's. That independence is the point: the NJOY goldens this
//! repo owns cover only graphite and light water, so moment comparisons simply
//! cannot be made for the other six.
//!
//! # Method
//!
//! Identical to the committed test. Each chain is a random walk on the law
//! itself; the chain's stationary density is the collision density, so each
//! collision is weighted `1/(σ_thermal(E)·√E)` to recover the neutron density.
//! Chains start hot (40 kT) and cold (0.2 kT); the two must agree or neither is a
//! fixed point.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example thermal_fixed_point_sweep
//! ```
//!
//! # Measured 2026-09-13, ENDF/B-VIII.0 where available
//!
//! ```text
//!   law                              T [K]      T_eff [K]        vs nom      shape      vs 5/3
//!   H in H2O (liquid)                293.6   294.29 +/- 0.33     +0.24 %    1.6410     -1.54 %
//!   H in ZrH (solid hydride)         296.0   303.92 +/- 1.34     +2.68 %    1.6479     -1.13 %
//!   C crystalline graphite           296.0   299.11 +/- 0.94     +1.05 %    1.6403     -1.58 %
//!   C reactor graphite 10 % porous   296.0   300.06 +/- 0.80     +1.37 %    1.6277     -2.34 %
//!   C reactor graphite 30 % porous   296.0   299.01 +/- 0.72     +1.02 %    1.6330     -2.02 %
//!   C in SiC                         296.0   302.16 +/- 0.84     +2.08 %    1.6562     -0.63 %
//!   Si in SiC                        296.0   302.22 +/- 1.27     +2.10 %    1.6589     -0.47 %
//!   Al-27 metal                      296.0   295.53 +/- 0.70     -0.16 %    1.6527     -0.84 %
//! ```
//!
//! **Every evaluation is narrow, one-signed, between 0.5 % and 2.3 %.** A
//! liquid, a solid hydride, three graphites, two sublattices of a compound and a
//! metal have nothing physical in common and cannot share a defect by
//! coincidence. So GitHub #188 is not "the H-in-H₂O kernel is wrong" — it is
//! THERMR's kernel construction narrowing every evaluation it processes, and any
//! fix should be expected to move all eight rows.
//!
//! Al-27 is the row that earns its place. Its fixed-point temperature is right to
//! **−0.16 %** while it is still **0.84 % narrow**, which separates the two
//! symptoms: the hot displacement is material-dependent (+0.24 % to +2.68 %,
//! and negative here), the narrowness is not. A single explanation is therefore
//! unlikely to cover both.
//!
//! For scale, the per-collision width deficit against NJOY2016's MF=6 — a
//! different quantity, measured only where this repo owns goldens — is −1.96 %
//! for graphite and −4.02 % for H-in-H₂O. Same sign, same order.
//!
//! # What this file is not
//!
//! It is a **diagnostic sweep, not a gate.** It asserts nothing and is not in the
//! test suite: six of the eight laws have no committed NJOY golden, so there is
//! nothing to pin them against except the Maxwellian identities, and pinning
//! eight materials to those would be pinning the defect rather than the physics.
//! `tests/thermal_kernel_stationary_distribution.rs` is the gated form, on the two
//! laws the benchmarks actually use.
//!
//! Data-gated: skips any evaluation whose tape is absent.

use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::physics::scatter::K_BOLTZMANN_EV_PER_K;

/// Independent chains per start.
const N_CHAINS: usize = 2_000;
/// Collisions discarded before tallying, per chain.
const N_BURN: usize = 400;
/// Collisions tallied per chain.
const N_TALLY: usize = 1_600;

/// A Maxwellian's shape factor `⟨E²⟩/⟨E⟩²` — `3.75 / 1.5² = 5/3`, exactly, at
/// every temperature.
const MAXWELLIAN_SHAPE: f64 = 5.0 / 3.0;

/// `(file, MAT, temperature [K], label)`. The temperature is one the evaluation
/// actually tabulates, so nothing is interpolated in `T`.
const CASES: [(&str, i32, f64, &str); 8] = [
    ("tsl-HinH2O.endf", 1, 293.6, "H in H2O (liquid)"),
    (
        "tsl-HinZrH-ENDF8.0.endf",
        7,
        296.0,
        "H in ZrH (solid hydride)",
    ),
    (
        "tsl-crystalline-graphite.endf",
        30,
        296.0,
        "C crystalline graphite",
    ),
    (
        "tsl-reactor-graphite-10P.endf",
        31,
        296.0,
        "C reactor graphite 10 % porous",
    ),
    (
        "tsl-reactor-graphite-30P.endf",
        32,
        296.0,
        "C reactor graphite 30 % porous",
    ),
    ("tsl-CinSiC.endf", 44, 296.0, "C in SiC"),
    ("tsl-SiinSiC.endf", 43, 296.0, "Si in SiC"),
    ("tsl-013_Al_027-ENDF8.0.endf", 53, 296.0, "Al-27 metal"),
];

fn mean_and_sem(xs: &[f64]) -> (f64, f64) {
    let n = xs.len() as f64;
    let mean = xs.iter().sum::<f64>() / n;
    let var = xs.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / (n - 1.0);
    (mean, (var / n).sqrt())
}

/// `(T_eff [K], sem, shape, shape_sem, escapes)` from `N_CHAINS` chains started
/// at `e_start`.
fn equilibrium(law: &ThermalScattering, e_start: f64, seed0: u64) -> (f64, f64, f64, f64, usize) {
    let mut t_eff = Vec::with_capacity(N_CHAINS);
    let mut shape = Vec::with_capacity(N_CHAINS);
    let (mut pw, mut pwe, mut pwe2) = (0.0, 0.0, 0.0);
    let mut escapes = 0usize;
    for c in 0..N_CHAINS {
        let mut seed = seed0.wrapping_add((c as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let mut e = e_start;
        let mut collide = |e: f64, seed: &mut u64, escapes: &mut usize| -> f64 {
            match law.sample(e, seed) {
                Some((ep, _)) if ep > 0.0 && ep.is_finite() => ep,
                _ => {
                    // Above the thermal cutoff the bound law declines; at these
                    // temperatures that is a vanishing tail, so hold the energy
                    // and count it rather than pretending it did not happen.
                    *escapes += 1;
                    e
                }
            }
        };
        for _ in 0..N_BURN {
            e = collide(e, &mut seed, &mut escapes);
        }
        let (mut w_sum, mut w_e, mut w_e2) = (0.0, 0.0, 0.0);
        for _ in 0..N_TALLY {
            e = collide(e, &mut seed, &mut escapes);
            let s = law.total_xs(e);
            if !(s > 0.0) {
                continue;
            }
            let w = 1.0 / (s * e.sqrt());
            w_sum += w;
            w_e += w * e;
            w_e2 += w * e * e;
        }
        if !(w_sum > 0.0) {
            continue;
        }
        pw += w_sum;
        pwe += w_e;
        pwe2 += w_e2;
        let m1 = w_e / w_sum;
        t_eff.push(m1 / (1.5 * K_BOLTZMANN_EV_PER_K));
        shape.push(w_e2 / w_sum / (m1 * m1));
    }
    let m1 = pwe / pw;
    let (_, t_sem) = mean_and_sem(&t_eff);
    let (_, s_sem) = mean_and_sem(&shape);
    (
        m1 / (1.5 * K_BOLTZMANN_EV_PER_K),
        t_sem,
        pwe2 / pw / (m1 * m1),
        s_sem,
        escapes,
    )
}

fn main() {
    println!(
        "Fixed-point oracle across every S(a,b) evaluation in reference-data/endf/.\n\
         Reference values are exact and need no library: T_eff = the law's own \
         temperature,\nshape <E^2>/<E>^2 = 5/3. {N_CHAINS} chains x ({N_BURN} burn + \
         {N_TALLY} tallied) per start.\n"
    );
    println!(
        "{:<32} {:>7}  {:>17}  {:>9}  {:>17}  {:>9}",
        "law", "T [K]", "T_eff [K]", "vs nom", "shape", "vs 5/3"
    );
    let mut rows = Vec::new();
    for (file, mat, temp_k, label) in CASES {
        let Some(path) = njoy_outram_park_fork::reference_data::reference_endf(file) else {
            println!("{label:<32}  SKIP: {file} absent");
            continue;
        };
        let law = match ThermalScattering::from_endf_file(
            path.to_str().expect("path"),
            mat,
            temp_k,
            label,
        ) {
            Ok(l) => l,
            Err(e) => {
                println!("{label:<32}  SKIP: {e:?}");
                continue;
            }
        };
        let kt = K_BOLTZMANN_EV_PER_K * temp_k;
        let hot = equilibrium(&law, 40.0 * kt, 0x5EED_1001);
        let cold = equilibrium(&law, 0.2 * kt, 0x5EED_1002);
        let t = 0.5 * (hot.0 + cold.0);
        let t_sem = 0.5 * (hot.1 + cold.1);
        let sh = 0.5 * (hot.2 + cold.2);
        let sh_sem = 0.5 * (hot.3 + cold.3);
        let n_sigma = (hot.0 - cold.0).abs() / (hot.1 * hot.1 + cold.1 * cold.1).sqrt().max(1e-12);
        println!(
            "{label:<32} {temp_k:>7.1}  {t:>9.2} +/- {t_sem:<4.2}  {:>+8.2} %  \
             {sh:>9.4} +/- {sh_sem:<5.4}  {:>+7.2} %{}",
            100.0 * (t - temp_k) / temp_k,
            100.0 * (sh - MAXWELLIAN_SHAPE) / MAXWELLIAN_SHAPE,
            if n_sigma > 6.0 {
                format!("   [hot/cold disagree, {n_sigma:.1} sigma]")
            } else {
                String::new()
            },
        );
        rows.push((
            label,
            100.0 * (t - temp_k) / temp_k,
            100.0 * (sh - MAXWELLIAN_SHAPE) / MAXWELLIAN_SHAPE,
        ));
    }

    println!("\n=== reading ===");
    if rows.is_empty() {
        println!("  no evaluations available");
        return;
    }
    let worst_t = rows
        .iter()
        .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).expect("finite"))
        .expect("non-empty");
    let worst_s = rows
        .iter()
        .max_by(|a, b| a.2.abs().partial_cmp(&b.2.abs()).expect("finite"))
        .expect("non-empty");
    let all_narrow = rows.iter().all(|r| r.2 < 0.0);
    println!(
        "  {} evaluations measured; worst temperature {:+.2} % ({}), worst shape \
         {:+.2} % ({})",
        rows.len(),
        worst_t.1,
        worst_t.0,
        worst_s.2,
        worst_s.0
    );
    println!(
        "  shape deficit one-signed narrow across every evaluation: {}",
        if all_narrow { "YES" } else { "no" }
    );
    println!(
        "\n  A displacement common to evaluations with nothing physical in common —\n  \
         a liquid, a hydride, three graphites, two SiC sublattices and a metal —\n  \
         is a property of the processing, not of any one evaluation. A displacement\n  \
         that varies with the material is the opposite. This table is what tells\n  \
         them apart, and it needs no reference library to do it."
    );
}
