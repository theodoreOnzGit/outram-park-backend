// SPDX-License-Identifier: GPL-3.0
//
// Verification of `boon_lay::triso_atops_fork`'s Booth kernel-release series
// against the Crank closed form, at the IAEA CRP-6 Case 1 inputs this crate
// already carries for its Lagrangian solver (gh:#382).
//
// Uses only boon-lay + uom (Android-friendly), so no target gate.

//! # V&V: TRISO-ATOPS Booth release vs the Crank series and CRP-6 Case 1 (gh:#382)
//!
//! ## What is being verified
//!
//! `triso_atops_fork::release_models::steady_state::booth_longlived` (and its
//! transient twin `transient::booth_transient`) is the fractional release of a
//! species with uniform initial concentration diffusing out of a sphere with a
//! perfect-sink surface:
//!
//! ```text
//! RF(tau) = 1 - 6 sum_{n>=1} exp(-(n pi)^2 tau) / (n pi)^2,   tau = D t / a^2
//! ```
//!
//! That is exactly Crank (1975) eq. 6.20, and exactly IAEA CRP-6 Case 1, a bare
//! UO2 kernel. `htgr_sim_v1` runs this branch for long-lived special metals
//! (Cs-137) under normal operation.
//!
//! Until now it was checked code-to-code against upstream (bit-exact) and at
//! two analytic limits with a 5e-3 relative tolerance
//! (`tests/triso_atops_fork_verification.rs`). It was never checked against
//! the Crank series or the CRP-6 inputs this crate already uses for the
//! Lagrangian solver
//! (`verification_and_validation/crp6_case1_kernel_release_vs_crank.md`).
//!
//! ## Methodology
//!
//! 1. **CRP-6 Case 1a/1b** (Cs-137, kernel radius 212.5 um, 200 h, 1200 °C and
//!    1600 °C, Jiang 2023 kernel `D` at zero fluence, the same inputs as the
//!    Lagrangian record). Compare `booth_longlived`, `booth_transient(D t/a^2)`
//!    and this crate's independent Crank implementation
//!    (`calculate_analytical_fraction_released`, 200 terms). Pass: agreement
//!    to 1e-12 absolute, since at these `tau` every omitted term underflows.
//! 2. **Small-`tau` truncation.** The early-time expansion
//!    `RF = 6 sqrt(tau/pi) - 3 tau` is exact up to terms of order
//!    `exp(-1/tau)`, which are below 1e-40 for `tau <= 1e-2`. So it is an
//!    independent reference there. `booth_longlived` keeps upstream's
//!    `BOOTH_SERIES_TERMS` = 5000, summing n = 1..4999. **Prediction, stated
//!    before measuring:** as `tau -> 0` the truncated series tends to
//!    `F = 1 - 6 sum_{n=1}^{4999} 1/(n pi)^2 = (6/pi^2) sum_{n>=5000} 1/n^2
//!    ~ 1.216e-4`, not to 0. So it over-states any release below ~1e-4.
//! 3. **Where that bites for HTR-10.** Each long-lived special metal on this
//!    branch (Cs, Sr, Ba, Eu) with TRISO-ATOPS's own kernel `D(T)`, the
//!    `htgr_sim_v1` Booth radius `a = sqrt(2 a_grain r)` = sqrt(2 x 1e-5 x
//!    2.5e-4) m and `t_irr` = 1 y: report `booth_longlived` against the
//!    early-time law over 700-1600 K.
//!
//! ## Results (2026-09-29, `develop` `7ee65476`)
//!
//! Measured by `cargo test --release -p boon-lay --test
//! triso_atops_booth_vs_crp6 -- --nocapture`. Full tables are in each test's
//! doc comment.
//!
//! 1. **CRP-6 Case 1:** Booth = Crank = transient Booth to 2.2e-16. 1a
//!    (1200 °C) gives 0.5337290191, matching the Lagrangian record's 0.5337.
//!    1b (1600 °C) gives 0.9999999983.
//! 2. **Truncation floor:** confirmed, with the predicted value 1.215976e-4.
//!    The series is exact to 4 significant figures down to tau = 1e-7, is
//!    +0.4 % at 1e-8 and 1.4x at 1e-9, and tends to the floor (359x at 1e-14).
//! 3. **At `htgr_sim_v1` inputs:** Cs-137 is unaffected (tau >= 2.14e-3,
//!    because upstream clamps its kernel temperature at 700 °C). **Sr/Ba/Eu
//!    are over-stated:** 120x at <= 973 K, 54x at 1000 K, 3.8x at 1100 K,
//!    +0.9 % at 1200 K, and exact from 1300 K. `htgr_sim_v1` does not track
//!    those metals today; Sr-90 in any TRISO-ATOPS or `sembawang` run at
//!    normal-operation temperatures is affected.

use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::release_fraction_analytical_solution::calculate_analytical_fraction_released;
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::release_fraction_crp_6_case_1a_1b::simulation_code::kernel_diffusion_coefficient;
use boon_lay::triso_atops_fork::diffusion::diffusion_coefficient;
use boon_lay::triso_atops_fork::release_models::steady_state::{booth_longlived, BOOTH_SERIES_TERMS};
use boon_lay::triso_atops_fork::release_models::transient::booth_transient;

use fission_yields_data::prelude::Nuclide;
use uom::si::diffusion_coefficient::square_meter_per_second;
use uom::si::f64::{DiffusionCoefficient, Length, Ratio, ThermodynamicTemperature, Time};
use uom::si::length::{meter, micrometer};
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::{degree_celsius, kelvin};
use uom::si::time::{hour, second};

/// CRP-6 Case 1 kernel radius: 425 um diameter.
fn crp6_kernel_radius() -> Length {
    Length::new::<micrometer>(425.0) / 2.0
}

/// The early-time expansion of the sphere release, exact to O(exp(-1/tau)).
fn early_time_law(tau: f64) -> f64 {
    6.0 * (tau / std::f64::consts::PI).sqrt() - 3.0 * tau
}

/// The predicted zero-time floor of the truncated series:
/// `1 - 6 sum_{n=1}^{N-1} 1/(n pi)^2`, with `N = BOOTH_SERIES_TERMS`.
fn predicted_truncation_floor() -> f64 {
    let pi2 = std::f64::consts::PI * std::f64::consts::PI;
    // Sum the omitted tail directly rather than subtracting from 1/6, to avoid
    // cancellation: sum_{n>=N} 1/n^2, to a million terms plus the
    // Euler-Maclaurin remainder 1/M.
    let big_m = 1_000_000usize;
    let mut tail = 0.0;
    for n in (BOOTH_SERIES_TERMS..big_m).rev() {
        let x = n as f64;
        tail += 1.0 / (x * x);
    }
    tail += 1.0 / big_m as f64;
    6.0 / pi2 * tail
}

/// **CRP-6 Case 1a/1b: Booth (TRISO-ATOPS) = Crank series = transient Booth.**
///
/// Results (2026-09-29):
///
/// | Case | T | D (Jiang) \[m^2/s\] | tau | booth_longlived | Crank (200 terms) | booth_transient |
/// |---|---|---|---|---|---|---|
/// | 1a | 1200 °C | 2.2519e-15 | 3.5906e-2 | 0.5337290191 | 0.5337290191 | 0.5337290191 |
/// | 1b | 1600 °C | 1.2503e-13 | 1.9936 | 0.9999999983 | 0.9999999983 | 0.9999999983 |
///
/// Measured differences: |booth - crank| = 2.2e-16 (1a), 0 (1b), and
/// |booth - transient| = 0 in both cases. 1a sits in the Hales et al. (2021) Table 4 range quoted
/// by the Lagrangian record (~0.53), and 1b is 1.0.
///
/// **Interpretation.** The TRISO-ATOPS Booth branch reproduces the Crank
/// closed form at the CRP-6 Case 1 inputs, so the Eulerian and Lagrangian
/// release models agree on the one shared benchmark. **This is verification
/// only.** Both sides use the same `D`, and the CRP-6 code-to-code spread is
/// not catalogued in `kovan-literature`, so it is not compared.
#[test]
fn crp6_case1_booth_equals_the_crank_series() {
    let radius = crp6_kernel_radius();
    let time = Time::new::<hour>(200.0);
    for (label, temp_c) in [("1a", 1200.0), ("1b", 1600.0)] {
        let temperature = ThermodynamicTemperature::new::<degree_celsius>(temp_c);
        let d = kernel_diffusion_coefficient(Nuclide::Cs137, temperature);
        let a = radius.get::<meter>();
        let tau = d.get::<square_meter_per_second>() * time.get::<second>() / (a * a);

        let booth = booth_longlived(d, time, radius).get::<ratio>();
        let transient = booth_transient(Ratio::new::<ratio>(tau)).get::<ratio>();
        let crank = calculate_analytical_fraction_released(d, radius, time, 200);

        println!(
            "CRP-6 {label}: T = {temp_c} C, D = {:.4e} m^2/s, tau = {tau:.4e}, \
             booth = {booth:.10}, crank = {crank:.10}, transient = {transient:.10}, \
             |booth-crank| = {:.2e}, |booth-transient| = {:.2e}",
            d.get::<square_meter_per_second>(),
            (booth - crank).abs(),
            (booth - transient).abs(),
        );
        assert!(
            (booth - crank).abs() < 1e-12,
            "{label}: Booth {booth} vs Crank {crank}"
        );
        assert!(
            (booth - transient).abs() < 1e-12,
            "{label}: Booth {booth} vs transient Booth {transient}"
        );
    }
}

/// **The truncated series has a floor at small tau, and it is the predicted
/// one.**
///
/// Methodology: see the file doc, item 2. Reference: the early-time law, exact
/// for `tau <= 1e-2` to far better than f64.
///
/// Results (2026-09-29), `booth_longlived` against the reference:
///
/// | tau | early-time law | booth_longlived | booth / law |
/// |---|---|---|---|
/// | 1e-2 | 3.085138e-1 | 3.085138e-1 | 1.0000 |
/// | 1e-4 | 3.355138e-2 | 3.355138e-2 | 1.0000 |
/// | 1e-6 | 3.382138e-3 | 3.382138e-3 | 1.0000 |
/// | 1e-7 | 1.070174e-3 | 1.070174e-3 | 1.0000 |
/// | 1e-8 | 3.384838e-4 | 3.398858e-4 | 1.0041 |
/// | 1e-9 | 1.070444e-4 | 1.504165e-4 | 1.4052 |
/// | 1e-10 | 3.385108e-5 | 1.245847e-4 | 3.6804 |
/// | 1e-12 | 3.385135e-6 | 1.216276e-4 | 35.93 |
/// | 1e-14 | 3.385137e-7 | 1.215979e-4 | 359.2 |
///
/// Predicted floor 1.215976e-4. At tau = 1e-16 the series returns
/// 1.215976e-4, which is the floor itself.
///
/// **Interpretation.** The prediction holds for the floor's value. The series
/// is exact (4 significant figures) down to tau = 1e-7, so the existing
/// early-time check at tau = 1e-4 was not hiding it. Below tau ~ 1e-8 the
/// floor dominates, and the reported release is **over-stated**, by 36x at
/// tau = 1e-12. This is inherited from
/// upstream's `num_terms=5000` (the port is bit-exact to it), so it is an
/// **upstream defect reproduced faithfully**, not a port error. It is filed
/// separately rather than fixed here, because changing it diverges from
/// upstream.
#[test]
fn booth_longlived_has_the_predicted_truncation_floor_at_small_tau() {
    let floor = predicted_truncation_floor();
    println!("BOOTH_SERIES_TERMS = {BOOTH_SERIES_TERMS}; predicted floor F = {floor:.6e}");
    println!(
        "{:>8} {:>14} {:>14} {:>10}",
        "tau", "early-time", "booth", "ratio"
    );

    // Drive booth_longlived through its public (D, t, a) arguments with a = 1 m,
    // t = 1 s, so D = tau numerically.
    let a = Length::new::<meter>(1.0);
    let t = Time::new::<second>(1.0);
    for exp10 in [-2, -3, -4, -5, -6, -7, -8, -9, -10, -11, -12, -13, -14] {
        let tau = 10f64.powi(exp10);
        let d = DiffusionCoefficient::new::<square_meter_per_second>(tau);
        let booth = booth_longlived(d, t, a).get::<ratio>();
        let law = early_time_law(tau);
        println!(
            "{tau:>8.0e} {law:>14.6e} {booth:>14.6e} {:>10.4}",
            booth / law
        );

        // Where the omitted tail is negligible, the series is the law.
        if tau >= 1e-2 {
            assert!(
                (booth / law - 1.0).abs() < 1e-9,
                "tau {tau}: {booth} vs {law}"
            );
        }
        // Everywhere, booth ~= law + (tail not yet decayed). The tail is
        // bounded by the floor, so booth is never below the law and never above
        // law + floor (to rounding).
        assert!(
            booth >= law * (1.0 - 1e-9),
            "tau {tau}: booth {booth} below law {law}"
        );
        assert!(
            booth <= law + floor * (1.0 + 1e-6),
            "tau {tau}: booth {booth} above law+F"
        );
    }
    // As tau -> 0 the series approaches the predicted floor, not zero.
    let tiny = booth_longlived(
        DiffusionCoefficient::new::<square_meter_per_second>(1e-16),
        t,
        a,
    )
    .get::<ratio>();
    let expected = early_time_law(1e-16) + floor;
    println!("tau = 1e-16: booth = {tiny:.6e}, law + F = {expected:.6e}");
    assert!(
        (tiny / expected - 1.0).abs() < 1e-2,
        "the zero-time limit {tiny:.6e} is not the predicted floor {expected:.6e}"
    );
}

/// **Where the floor bites at `htgr_sim_v1`'s inputs: not for Cs, badly for
/// Sr/Ba/Eu.**
///
/// For each long-lived special metal TRISO-ATOPS routes through
/// `booth_longlived` (Cs 55, Sr 38, Ba 56, Eu 63), kernel `D(T)` comes from
/// TRISO-ATOPS's own correlation. The Booth radius is `sqrt(2 a_grain r)`,
/// with `a_grain` = 1e-5 m (the TRISO-ATOPS reference) and `r` = 250 um (the
/// HTR-10 kernel), and `t_irr` = 1 y. These are the `htgr_sim_v1` inputs
/// (`Htr10TrisoAtopsInputs::htr10`), over 700-1600 K.
///
/// **Prediction (made before measuring):** at the cool end, Cs-137's true
/// release would fall below the floor. **That was wrong for Cs** and right for
/// the other three.
///
/// **Results (2026-09-29):**
///
/// | Z | T \[K\] | D \[m^2/s\] | tau | early-time law | booth | ratio |
/// |---|---|---|---|---|---|---|
/// | 55 (Cs) | 700-900 | 3.3899e-19 | 2.140e-3 | 1.50162e-1 | 1.50162e-1 | 1.0000 |
/// | 55 (Cs) | 1000 | 6.7829e-19 | 4.281e-3 | 2.08645e-1 | 2.08645e-1 | 1.0000 |
/// | 38 (Sr) | 700-900 | 1.4098e-29 | 8.898e-14 | 1.00976e-6 | 1.21600e-4 | **120.4** |
/// | 38 (Sr) | 1000 | 7.1193e-29 | 4.493e-13 | 2.26915e-6 | 1.21611e-4 | **53.6** |
/// | 38 (Sr) | 1100 | 1.4781e-26 | 9.329e-11 | 3.26960e-5 | 1.24385e-4 | **3.80** |
/// | 38 (Sr) | 1200 | 1.2612e-24 | 7.960e-9 | 3.01991e-4 | 3.04708e-4 | 1.009 |
/// | 38 (Sr) | 1300 | 5.4293e-23 | 3.427e-7 | 1.98057e-3 | 1.98057e-3 | 1.0000 |
///
/// Ba and Eu share Sr's correlation and give identical rows.
///
/// **Interpretation.** Cs-137, the only one of these `htgr_sim_v1` tracks, is
/// unaffected. Its `tau` never drops below 2.14e-3, because upstream evaluates
/// Rb/Cs and Sr/Ba/Eu at `max(T, 700 °C)`, so Cs `D` is flat below 973 K. The
/// same clamp makes the Sr/Ba/Eu over-statement **constant at 120x** below
/// 973 K. Normal-operation HTR-10 fuel (~900-1100 K) sits in the
/// 3.8x-120x band. Both behaviours are upstream's, reproduced bit-exactly. The
/// assertions pin the measurement so a change to either is noticed.
#[test]
fn where_the_floor_bites_at_htgr_sim_v1_inputs() {
    let a_booth = (2.0_f64 * 1.0e-5 * 2.5e-4).sqrt();
    let radius = Length::new::<meter>(a_booth);
    let t_irr = Time::new::<second>(3.155_76e7);
    println!("a_booth = {a_booth:.4e} m, t_irr = 1 y");
    println!(
        "{:>3} {:>7} {:>12} {:>11} {:>13} {:>13} {:>9}",
        "Z", "T [K]", "D [m^2/s]", "tau", "law", "booth", "ratio"
    );

    let mut min_tau = f64::INFINITY;
    let mut worst_ratio: f64 = 1.0;
    for z in [55u32, 38, 56, 63] {
        for t_k in (700..=1600).step_by(100) {
            let t = ThermodynamicTemperature::new::<kelvin>(t_k as f64);
            let d = diffusion_coefficient(z, t, t).kernel;
            let tau =
                d.get::<square_meter_per_second>() * t_irr.get::<second>() / (a_booth * a_booth);
            let booth = booth_longlived(d, t_irr, radius).get::<ratio>();
            min_tau = min_tau.min(tau);
            let law = if tau <= 1e-2 {
                early_time_law(tau)
            } else {
                f64::NAN
            };
            let ratio_v = booth / law;
            if ratio_v.is_finite() {
                worst_ratio = worst_ratio.max(ratio_v);
            }
            println!(
                "{z:>3} {t_k:>7} {:>12.4e} {tau:>11.3e} {law:>13.5e} {booth:>13.5e} {ratio_v:>9.4}",
                d.get::<square_meter_per_second>()
            );
        }
    }
    println!(
        "min tau = {min_tau:.3e}; worst booth / early-time law (tau <= 1e-2) = {worst_ratio:.6}"
    );

    // Cs-137 (tracked by htgr_sim_v1): never reaches the truncation region.
    let cs_min_tau = (700..=1600)
        .step_by(100)
        .map(|t_k| {
            let t = ThermodynamicTemperature::new::<kelvin>(t_k as f64);
            diffusion_coefficient(55, t, t)
                .kernel
                .get::<square_meter_per_second>()
                * t_irr.get::<second>()
                / (a_booth * a_booth)
        })
        .fold(f64::INFINITY, f64::min);
    assert!(cs_min_tau > 1e-3, "Cs-137 min tau {cs_min_tau:e}");
    // Sr/Ba/Eu: the measured ~120x over-statement at the 700 °C clamp.
    assert!(
        (worst_ratio - 120.42).abs() < 0.5,
        "Sr/Ba/Eu worst over-statement {worst_ratio} (measured 120.42 on 2026-09-29)"
    );
}
