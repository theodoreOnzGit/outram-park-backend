//! MSRE TH-C1 (gh:#1135, epic gh:#1125): TUAS's laminar and turbulent pipe
//! Nusselt correlations against the analytical Graetz solution and the
//! textbook turbulent correlations. Verification only (code against
//! analytical/correlation references); research, education and V&V only.
//!
//! # Methodology (fixed before the runs, gh:#1135 comment of 2026-11-09)
//!
//! **Reference: the Graetz problem**, solved here. Thermal entry with a developed
//! Hagen-Poiseuille profile, constant wall temperature, no axial conduction.
//! The eigenproblem `(r R')' + β² r (1 − r²) R = 0`, `R'(0) = 0`, `R(1) = 0`
//! (r = radius / pipe radius) is integrated by RK4 from a series start at
//! r = 1e-6 and its roots in β are found by scan and bisection. This is the
//! same shooting as `outram-foam-appbuilder-lib/tests/energy_graetz_nusselt.rs`
//! (gh:#1156), extended from the first eigenvalue to the whole series. With
//! `x⁺ = (x/D)/(Re Pr)` and inlet θ = 1:
//!
//! - `θ = Σ C_n R_n(r) exp(−2 β_n² x⁺)`, with `C_n = I1_n / I2_n`,
//!   `I1 = ∫ r(1−r²) R dr` and `I2 = ∫ r(1−r²) R² dr`;
//! - bulk (mixing-cup) temperature `θ_b = 4 Σ C_n I1_n exp(−2 β_n² x⁺)`;
//! - mean Nusselt number over the length, from the energy balance:
//!   `Nu_m = −ln θ_b / (4 x⁺)`. The developed local value is `β_0² / 2`.
//!
//! The series is summed until a term falls below 1e-13 of θ_b. The
//! constant-flux developed value is the closed form 48/11. The developing
//! constant-flux series (Siegel, Sparrow and Hallman) is **not** built here.
//!
//! **Checks.** A1, A2, B1, B2 and B4 are gated with the criteria fixed in
//! gh:#1135. B3 and C1 are reported, not gated, because the two sides are not
//! the same quantity (B3) or there is no analytical turbulent reference (C1).
//!
//! # Results (2026-11-09, develop a647a6bde + this change, Rust 1.99.1)
//!
//! **Reference solver.**
//! - A1: β_0 = 2.70436442, developed Nu = 3.65679346 (textbook 3.6568).
//! - A3: the first five modes match the classic tabulated values to every
//!   printed digit (β = 2.70436, 6.67903, 10.67338, 14.67108, 18.66987;
//!   G = 0.74877, 0.54383, 0.46286, 0.41542, 0.38292). Modes 10 to 40 sit
//!   within 0.0011 of `4n + 8/3`, and within 0.10 % of the G asymptote.
//! - The 120 bulk weights sum to 0.99960; the 4e-4 shortfall is the
//!   unsummed tail, which matters only as x⁺ → 0.
//!
//! **TUAS, gated (all pass).**
//! - A2: 3.66 is +0.088 % of exact; 4.36 is −0.083 % of 48/11.
//! - B1: the developing wall-T correlation's long-tube limit is 3.660577,
//!   +0.103 %.
//! - B2: the developing constant-flux limit is 4.354359, −0.213 % of 48/11
//!   (its blend constant is 4.354).
//!
//! **Reported findings.**
//! - **B3:** the developing wall-T correlation against the exact Graetz mean.
//!   It is within −0.8 to +0.2 % for Gz ≤ 3. Above that it climbs ever
//!   further above Graetz, because the correlation's velocity-development
//!   term adds to it and that term shrinks with Pr. At Gz = 1e4 it is
//!   +116 % at Pr = 0.7, +58 % at Pr = 7, +47 % at Pr = 13 and +22 % at
//!   Pr = 100.
//!   - Prediction miss: TUAS ≥ Graetz everywhere was predicted, but at
//!     Gz ≤ 3 it is up to 0.8 % below.
//!   - Interpretation: the correlation describes simultaneously developing
//!     flow. Where the velocity profile is already developed at the heated
//!     section, as in a long unheated run before it, it overpredicts by the
//!     amounts above.
//! - **B4:** the pre-registered gate FAILED.
//!   - At Gz = 1e4, `Nu_m/Gz^(1/3)` is 1.56934, −2.83 % of 1.615, below the
//!     constant rather than above.
//!   - The deficit from Lévêque is −0.34, −0.77 and −0.98 at Gz = 1e2, 1e3
//!     and 1e4: still approaching its constant.
//!   - The reference is right (A3). The prediction underestimated the next
//!     asymptotic term.
//! - **C1:** Dittus-Boelter (heating) against Gnielinski (smooth tube, no
//!   wall-property correction) for Re 1e4 to 1e5 and Pr 0.7 to 13.
//!   - The spread is −17.0 to +12.4 %: DB above at Pr = 0.7, below at
//!     Pr ≥ 7 and Re ≥ 3e4.
//!   - That is inside the ~±25 % usually quoted for DB.
//!   - The turbulent correlations have no analytical reference, so this is a
//!     spread between correlations, not a verification.
//!
//! **Not covered:** the developing constant-flux Graetz series; turbulent
//! correlations against measured data (that is TH-1, gh:#1134).

#[cfg(test)]
use petir::mathf::RealMath;

/// RK4 steps from the series start to the wall. β h at the largest
/// eigenvalue used (β ≈ 380) is about 0.01.
#[cfg(test)]
const STEPS: usize = 40_000;

/// Series start radius: `R ≈ 1 − β² r² / 4` there.
#[cfg(test)]
const R_START: f64 = 1.0e-6;

/// One Graetz eigenfunction, integrated to the wall.
#[cfg(test)]
#[derive(Clone, Copy, Debug)]
struct Shot {
    /// R(1): zero at an eigenvalue.
    r_wall: f64,
    /// `∫ r (1 − r²) R dr`.
    i1: f64,
    /// `∫ r (1 − r²) R² dr`.
    i2: f64,
}

/// Right-hand side of the shooting system `[R, P = r R', I1, I2]`.
#[cfg(test)]
fn graetz_rhs(r: f64, y: [f64; 4], beta_sq: f64) -> [f64; 4] {
    let weight = r * (1.0 - r * r);
    [y[1] / r, -beta_sq * weight * y[0], weight * y[0], weight * y[0] * y[0]]
}

/// Integrate the Graetz eigenproblem at `beta` from the axis to the wall.
#[cfg(test)]
fn shoot(beta: f64) -> Shot {
    let beta_sq = beta * beta;
    let mut r = R_START;
    let mut y = [
        1.0 - beta_sq * r * r / 4.0,
        -beta_sq * r * r / 2.0,
        0.0,
        0.0,
    ];
    let h = (1.0 - R_START) / STEPS as f64;
    for _ in 0..STEPS {
        let k1 = graetz_rhs(r, y, beta_sq);
        let k2 = graetz_rhs(r + h / 2.0, add(y, k1, h / 2.0), beta_sq);
        let k3 = graetz_rhs(r + h / 2.0, add(y, k2, h / 2.0), beta_sq);
        let k4 = graetz_rhs(r + h, add(y, k3, h), beta_sq);
        for i in 0..4 {
            y[i] += h / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
        }
        r += h;
    }
    Shot {
        r_wall: y[0],
        i1: y[2],
        i2: y[3],
    }
}

/// `y + scale k`, componentwise.
#[cfg(test)]
fn add(y: [f64; 4], k: [f64; 4], scale: f64) -> [f64; 4] {
    [
        y[0] + scale * k[0],
        y[1] + scale * k[1],
        y[2] + scale * k[2],
        y[3] + scale * k[3],
    ]
}

/// One term of the Graetz series: eigenvalue and bulk-temperature weight
/// `G_n = 4 C_n I1_n`.
#[cfg(test)]
#[derive(Clone, Copy, Debug)]
struct GraetzTerm {
    beta: f64,
    weight: f64,
}

/// The first `count` Graetz eigenvalues and their bulk weights.
#[cfg(test)]
fn graetz_terms(count: usize) -> Vec<GraetzTerm> {
    let mut terms = Vec::with_capacity(count);
    let step = 0.25;
    let mut lo = 0.5;
    let mut f_lo = shoot(lo).r_wall;
    while terms.len() < count {
        let hi = lo + step;
        let f_hi = shoot(hi).r_wall;
        if f_lo.signum() != f_hi.signum() {
            let (mut a, mut b, mut fa) = (lo, hi, f_lo);
            for _ in 0..60 {
                let m = 0.5 * (a + b);
                let fm = shoot(m).r_wall;
                if fm.signum() == fa.signum() {
                    a = m;
                    fa = fm;
                } else {
                    b = m;
                }
            }
            let beta = 0.5 * (a + b);
            let s = shoot(beta);
            terms.push(GraetzTerm {
                beta,
                weight: 4.0 * s.i1 * s.i1 / s.i2,
            });
        }
        lo = hi;
        f_lo = f_hi;
    }
    terms
}

/// The series, computed once for every test in this module.
#[cfg(test)]
fn series() -> &'static [GraetzTerm] {
    static SERIES: std::sync::OnceLock<Vec<GraetzTerm>> = std::sync::OnceLock::new();
    SERIES.get_or_init(|| graetz_terms(120))
}

/// Exact Graetz mean Nusselt number over a tube of `x⁺ = (L/D)/(Re Pr)`,
/// and the number of series terms it needed.
#[cfg(test)]
fn graetz_mean_nusselt(x_plus: f64) -> (f64, usize) {
    let terms = series();
    let mut theta_b = 0.0;
    let mut used = 0;
    for term in terms {
        let contribution = term.weight * (-2.0 * term.beta * term.beta * x_plus).r_exp();
        theta_b += contribution;
        used += 1;
        if contribution.abs() < 1.0e-13 * theta_b.abs() {
            break;
        }
    }
    assert!(
        used < terms.len(),
        "Graetz series not converged at x+ = {x_plus} with {} terms",
        terms.len()
    );
    (-theta_b.r_ln() / (4.0 * x_plus), used)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fluid_mechanics_correlations::darcy;
    use crate::heat_transfer_correlations::nusselt_number_correlations::pipe_correlations;

    /// A1: the developed constant-wall-temperature Nusselt number from the
    /// first eigenvalue, against the textbook 3.6568 (1e-4 relative).
    #[test]
    fn a1_the_first_graetz_eigenvalue_gives_3_6568() {
        let beta0 = series()[0].beta;
        let nu = beta0 * beta0 / 2.0;
        println!("A1: beta0 = {beta0:.8}, Nu_developed = {nu:.8}");
        approx::assert_relative_eq!(nu, 3.6568, max_relative = 1.0e-4);
        // The bulk weights of a complete series sum to theta_b(0) = 1.
        let sum: f64 = series().iter().map(|t| t.weight).sum();
        println!("A1: sum of 120 bulk weights = {sum:.10}");
    }

    /// A3: the high modes of the reference solver against the large-n
    /// asymptotics `β_n ≈ 4n + 8/3` and `G_n = β_n² weight_n / 8 ≈ 1.01276
    /// β_n^(-1/3)`, n = 10..40 (criteria pre-registered on gh:#1135 after B4
    /// failed: 0.01 absolute on β_n, 1 % on G_n). The short-tube means rest on
    /// these modes.
    #[test]
    fn a3_the_high_graetz_modes_follow_the_asymptotics() {
        for (n, term) in series().iter().enumerate().take(41) {
            let g = term.beta * term.beta * term.weight / 8.0;
            let beta_asym = 4.0 * n as f64 + 8.0 / 3.0;
            let g_asym = 1.01276 * term.beta.r_powf(-1.0 / 3.0);
            if n < 5 || n % 10 == 0 {
                println!(
                    "A3: n = {n:2}: beta = {:.5} (asym {beta_asym:.5}), G = {g:.5} (asym {g_asym:.5}, {:+.3} %)",
                    term.beta,
                    100.0 * (g / g_asym - 1.0)
                );
            }
            if n >= 10 {
                assert!((term.beta - beta_asym).abs() <= 0.01, "n = {n}: beta {}", term.beta);
                approx::assert_relative_eq!(g, g_asym, max_relative = 0.01);
            }
        }
    }

    /// A2: the developed laminar constants against the exact values, within
    /// half the last digit of a three-significant-figure constant (0.005).
    #[test]
    fn a2_the_developed_laminar_constants_round_the_exact_values() {
        let beta0 = series()[0].beta;
        let exact_wall_t = beta0 * beta0 / 2.0;
        let exact_flux = 48.0 / 11.0;
        let tuas_wall_t =
            pipe_correlations::laminar_nusselt_uniform_wall_temperature_fully_developed(1000.0);
        let tuas_flux = pipe_correlations::laminar_nusselt_uniform_heat_flux_fully_developed(1000.0);
        println!(
            "A2: wall T {tuas_wall_t} vs {exact_wall_t:.6} ({:+.3} %); flux {tuas_flux} vs {exact_flux:.6} ({:+.3} %)",
            100.0 * (tuas_wall_t / exact_wall_t - 1.0),
            100.0 * (tuas_flux / exact_flux - 1.0)
        );
        assert!((tuas_wall_t - exact_wall_t).abs() <= 0.005);
        assert!((tuas_flux - exact_flux).abs() <= 0.005);
    }

    /// B1 and B2: the developing correlations in the long-tube limit
    /// (Re Pr D/L = 1e-6) against the exact developed values. B1 within
    /// 0.005, B2 within 0.01 (its blend constant is 4.354).
    #[test]
    fn b1_b2_the_developing_correlations_reach_the_developed_limits() {
        let beta0 = series()[0].beta;
        let exact_wall_t = beta0 * beta0 / 2.0;
        let exact_flux = 48.0 / 11.0;
        // Re Pr D/L = 1e-6 with Re = 1, Pr = 1.
        let l_over_d = 1.0e6;
        let wall_t =
            pipe_correlations::laminar_nusselt_uniform_wall_temperature_developing(1.0, 1.0, l_over_d);
        let flux = pipe_correlations::laminar_nusselt_uniform_heat_flux_developing(1.0, 1.0, l_over_d);
        println!(
            "B1: {wall_t:.6} vs {exact_wall_t:.6} ({:+.3} %); B2: {flux:.6} vs {exact_flux:.6} ({:+.3} %)",
            100.0 * (wall_t / exact_wall_t - 1.0),
            100.0 * (flux / exact_flux - 1.0)
        );
        assert!((wall_t - exact_wall_t).abs() <= 0.005);
        assert!((flux - exact_flux).abs() <= 0.01);
    }

    /// B4 (reported; its pre-registered gate FAILED, 2026-11-09): the
    /// short-tube (Lévêque) asymptote of the exact Graetz mean. Pre-registered
    /// on gh:#1135: `Nu_m / Gz^(1/3)` at Gz = 1e4 within 2 % of 1.615,
    /// approached from above. Measured: 1.56934, **−2.83 %, from below**. A3
    /// rules out a solver defect, so the miss is the next asymptotic term,
    /// still about 3 % of the result at Gz = 1e4. The gate was not moved;
    /// the check is a recorded finding. The printed deficit
    /// `Nu_m − 1.615 Gz^(1/3)` shows the approach to a constant.
    #[test]
    fn b4_report_the_graetz_mean_against_the_leveque_asymptote() {
        for gz in [1.0e2_f64, 1.0e3, 1.0e4] {
            let (nu_m, used) = graetz_mean_nusselt(1.0 / gz);
            let leveque = 1.615 * gz.r_cbrt();
            println!(
                "B4: Gz = {gz:6.0}: Nu_m = {nu_m:.5}, Nu_m/Gz^(1/3) = {:.5} ({:+.3} % of 1.615), deficit {:+.4}, {used} terms",
                nu_m / gz.r_cbrt(),
                100.0 * (nu_m / leveque - 1.0),
                nu_m - leveque
            );
            assert!(nu_m.is_finite() && nu_m < leveque);
        }
    }

    /// B3 (reported, not gated): TUAS's developing constant-wall-temperature
    /// correlation (simultaneously developing) against the exact Graetz mean
    /// (thermally developing only), Gz = Re Pr D/L from 1 to 1e4.
    #[test]
    fn b3_report_developing_wall_temperature_against_graetz() {
        println!("B3: Pr, Gz, terms, Graetz Nu_m, TUAS Nu_m, TUAS/Graetz - 1 [%]");
        for pr in [0.7, 7.0, 13.0, 100.0] {
            for gz in [1.0, 3.0, 10.0, 30.0, 100.0, 300.0, 1000.0, 3000.0, 1.0e4] {
                let (exact, used) = graetz_mean_nusselt(1.0 / gz);
                // Re = 1000, laminar; L/D chosen to give this Gz at this Pr.
                let re = 1000.0;
                let l_over_d = re * pr / gz;
                let tuas = pipe_correlations::laminar_nusselt_uniform_wall_temperature_developing(
                    re, pr, l_over_d,
                );
                println!(
                    "B3: {pr:6.1} {gz:8.0} {used:4} {exact:9.4} {tuas:9.4} {:+8.2}",
                    100.0 * (tuas / exact - 1.0)
                );
                assert!(exact.is_finite() && tuas.is_finite());
            }
        }
    }

    /// C1 (reported, not gated): Gnielinski (fully developed, smooth-tube
    /// Darcy factor from `darcy`, no wall-property correction: Pr_wall =
    /// Pr_bulk) against Dittus-Boelter (heating, n = 0.4), Re 1e4 to 1e5.
    #[test]
    fn c1_report_gnielinski_against_dittus_boelter() {
        println!("C1: Re, Pr, f_Darcy, Gnielinski, Dittus-Boelter, DB/Gn - 1 [%]");
        for re in [1.0e4, 3.0e4, 1.0e5] {
            for pr in [0.7, 7.0, 13.0] {
                let f = darcy(re, 0.0).unwrap();
                let gn = pipe_correlations::gnielinski_correlation_liquids_fully_developed(re, pr, pr, f);
                let db = pipe_correlations::dittus_boelter_correlation(re, pr, true);
                println!(
                    "C1: {re:8.0} {pr:5.1} {f:.5} {gn:9.3} {db:9.3} {:+7.2}",
                    100.0 * (db / gn - 1.0)
                );
                assert!(gn.is_finite() && db.is_finite());
            }
        }
    }
}
