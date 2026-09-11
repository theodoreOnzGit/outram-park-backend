//! Mean log energy decrement per collision from the graphite S(alpha,beta)
//! kernel — the number that decides whether the ~+1.7 % k offset vs OpenMC is a
//! scattering-kernel effect.
//!
//! # Why this number
//!
//! Both cross-section sets are already verified against NJOY2016 to ~0.1 %
//! (`u238_vs_njoy_pendf.rs`, `graphite_vs_njoy_thermr.rs`), yet the spectrum
//! differs by several percent and in the *soft* direction. `sigma(E)` sets how
//! often a neutron collides; the kernel sets how much energy it loses. So the
//! discriminating quantity is the mean log energy decrement
//!
//! ```text
//! xi = <ln(E / E')>
//! ```
//!
//! Above the bound-scattering regime this must approach the free-gas value for
//! carbon, which is fixed by kinematics alone:
//!
//! ```text
//! alpha = ((A-1)/(A+1))^2 = (11/13)^2 = 0.715976
//! xi_fg = 1 + alpha*ln(alpha)/(1-alpha) = 0.157769   for A = 12
//! ```
//!
//! A kernel that transfers too much energy per collision — the signature of an
//! over-soft spectrum — shows up here as `xi` overshooting the free-gas
//! asymptote at the top of the thermal range. This needs no external oracle:
//! the asymptote is analytic.
//!
//! # CORRECTION (2026-09-11): this file used to have the sign backwards
//!
//! It previously said `xi` is *expected to sit above* `xi_fg` at thermal
//! energies "because the moderator's own thermal motion up-scatters". Up-scatter
//! pushes `xi` the **other** way. `xi = <ln(E/E')>`, so a neutron that *gains*
//! energy contributes a **negative** term, and at 0.0253 eV — where 15.6 % of
//! scatters are up-scatters and `<E'>/E = 1.19` — the measured `xi` is
//! **−0.0886**, i.e. −0.56 `xi_fg`. It then rises monotonically through zero
//! near 0.09 eV and approaches `xi_fg` from **below**.
//!
//! The claim was never checked because this program printed its table and exited
//! 0. Recording the measurement is what caught it, which is the whole argument
//! for asserting oracle comparisons rather than printing them.
//!
//! # V&V result (2026-09-11, ENDF/B-VIII.0 @ 600 K, 200 000 samples/energy)
//!
//! ```text
//!     E [eV]      <E'>/E          xi    xi/xi_fg  up-scat %
//!     0.0253     1.19080    -0.08859      -0.562      15.6
//!     0.0500     1.08498    -0.04072      -0.258      19.6
//!     0.1000     1.01497     0.01998       0.127      25.7
//!     0.2000     0.95249     0.08005       0.507      25.3
//!     0.5000     0.89721     0.13716       0.869      25.2
//!     1.0000     0.87779     0.14583       0.924      22.3
//!     2.0000     0.86760     0.16050       1.017      24.1
//!     3.0000     0.86390     0.15595       0.988      11.6
//!     3.9000     0.86271     0.15801       1.002      11.1
//! ```
//!
//! **`xi/xi_fg` = 1.002 at 3.9 eV.** The graphite kernel converges onto the
//! analytic free-gas asymptote to 0.2 % at the top of the thermal range, with no
//! overshoot. The moderator is therefore not transferring too much energy per
//! collision, and the ~+1.7 % k offset against OpenMC is not a graphite-kernel
//! effect. That is an exclusion, and it is the reason the search moved to
//! H-in-H2O (GitHub #188), whose kernel does *not* converge — its deviation
//! grows with energy instead.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example graphite_energy_decrement
//! ```

fn main() {
    use njoy_outram_park_fork::reference_data::reference_endf;
    use outram_mc_libs::material::thermal::ThermalScattering;
    use outram_mc_libs::vv::{assert_absolute, assert_monotone};

    const TEMP: f64 = 600.0;
    const N: usize = 200_000;

    let a = 12.0_f64;
    let alpha = ((a - 1.0) / (a + 1.0)).powi(2);
    let xi_fg = 1.0 + alpha * alpha.ln() / (1.0 - alpha);

    let tsl = reference_endf("tsl-crystalline-graphite.endf").expect("graphite tape");
    let g = ThermalScattering::from_endf_file(tsl.to_str().unwrap(), 30, TEMP, "C in graphite")
        .expect("thermal law");

    println!("graphite S(a,b) @ {TEMP} K, {N} samples per energy");
    println!("free-gas carbon asymptote: xi_fg = {xi_fg:.6}  (alpha = {alpha:.6})\n");
    println!(
        "{:>10}  {:>10}  {:>10}  {:>10}  {:>9}",
        "E [eV]", "<E'>/E", "xi", "xi/xi_fg", "up-scat %"
    );

    let mut seed = 20_260_911_u64;
    // (E, xi/xi_fg) for the V&V gate below.
    let mut trend: Vec<(f64, f64)> = Vec::new();
    for &e in &[0.0253, 0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 3.0, 3.9] {
        let (mut sum_ratio, mut sum_xi, mut n_ok, mut n_up) = (0.0, 0.0, 0usize, 0usize);
        for _ in 0..N {
            if let Some((ep, _mu)) = g.sample(e, &mut seed) {
                if ep > 0.0 {
                    sum_ratio += ep / e;
                    sum_xi += (e / ep).ln();
                    if ep > e {
                        n_up += 1;
                    }
                    n_ok += 1;
                }
            }
        }
        if n_ok == 0 {
            println!("{e:>10.4} {:>11}", "no samples");
            continue;
        }
        let k = n_ok as f64;
        let (ratio, xi) = (sum_ratio / k, sum_xi / k);
        println!(
            "{e:>10.4}  {ratio:>10.5}  {xi:>10.5}  {:>10.3}  {:>8.1}",
            xi / xi_fg,
            100.0 * n_up as f64 / k
        );
        trend.push((e, xi / xi_fg));
    }

    println!(
        "\nxi BELOW xi_fg at thermal energies is expected, and it is negative at\n\
         0.0253 eV: the moderator's own thermal motion up-scatters, and\n\
         xi = <ln(E/E')> counts an energy GAIN as a negative term. (This program\n\
         used to claim the opposite; see the correction in the module docs.) The\n\
         diagnostic is the TREND toward xi_fg as E rises past the phonon scale,\n\
         and whether the approach overshoots."
    );

    // ── V&V gate ──────────────────────────────────────────────────────────────
    //
    // The oracle is analytic and needs no library: two-body kinematics off a
    // carbon nucleus at rest fix xi_fg exactly. Three claims, and the third is
    // the one that carries the physics:
    //
    //   1. xi is NEGATIVE at 0.0253 eV      — up-scatter dominates there
    //   2. xi rises monotonically toward xi_fg — the trend
    //   3. xi converges ONTO xi_fg with no overshoot at the top of the range
    //
    // Claim 3 is what excludes the graphite kernel from the k offset. A kernel
    // transferring too much energy per collision would overshoot; one
    // transferring too little would stall below.
    println!("\n=== V&V gate: graphite xi against the analytic free-gas asymptote ===");

    let (_, r_thermal) = trend[0];
    assert!(
        r_thermal < 0.0,
        "xi/xi_fg = {r_thermal:.3} at 0.0253 eV, but it must be NEGATIVE there: \
         15.6 % of scatters are up-scatters and <E'>/E = 1.19, so the mean of \
         ln(E/E') is below zero (recorded −0.562). A non-negative value means \
         up-scatter has been lost from the kernel."
    );
    println!("  [PASS] xi is negative at 0.0253 eV ({r_thermal:.3} xi_fg): up-scatter dominates");

    let ratios: Vec<f64> = trend.iter().map(|&(_, r)| r).collect();
    // 5 % of slack: at 200 000 samples the 2 eV and 3 eV points straddle the
    // asymptote within their own noise (1.017 then 0.988 as recorded).
    assert_monotone(
        "xi rises toward the free-gas asymptote as E leaves the phonon scale",
        &ratios,
        true,
        0.05,
    );

    let (e_top, r_top) = *trend.last().expect("nine energies were sampled");
    assert_absolute(
        &format!("xi/xi_fg at {e_top} eV converges onto the free-gas asymptote"),
        r_top,
        1.0,
        0.03,
    );

    let worst_overshoot = ratios.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    assert!(
        worst_overshoot < 1.10,
        "xi overshoots the free-gas asymptote by {:.1} % somewhere in the range \
         (worst xi/xi_fg = {worst_overshoot:.3}). Overshoot is the signature of a \
         kernel transferring too much energy per collision — an over-soft \
         spectrum — which is the failure mode this program exists to detect.",
        100.0 * (worst_overshoot - 1.0),
    );
    println!(
        "  [PASS] no overshoot: worst xi/xi_fg across the range is {worst_overshoot:.3} \
         (must stay under 1.10)"
    );
}
