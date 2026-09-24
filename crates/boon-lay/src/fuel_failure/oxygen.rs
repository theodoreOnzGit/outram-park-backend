// SPDX-License-Identifier: GPL-3.0
//
// PANAMA-I reimplementation — provenance
// --------------------------------------
// Reference : Verfondern, K. & Nabielek, H., "The Mathematical Basis of the
//             PANAMA-I Code for Modeling Pressure Vessel Failure of TRISO
//             Coated Particles under Accident Conditions",
//             Forschungszentrum Jülich, HTA-IB-03/90, 1 August 1990.
//             Reprinted as Appendix C, printed pages -479- to -511-.
// Status    : the report is restricted literature with no reuse licence. Only
//             the governing EQUATIONS and their constants are reproduced here,
//             with citation, as scientific facts. No prose, figure or page of
//             that document is copied into this repository, and the PDF is not
//             tracked here. See DATA_POLICY.md.
// Nature    : an independent Rust implementation of the published model, not a
//             port of the PANAMA Fortran (which is closed-source and was never
//             consulted).

//! # TRISO coated-particle failure — the PANAMA-I pressure-vessel model
//!
//! A TRISO particle is a pressure vessel. Fission gas and CO accumulate inside
//! it, the SiC layer carries the hoop stress, and the particle fails when that
//! stress exceeds the SiC strength. PANAMA-I couples three failure populations:
//!

//! **Eqs (5a)–(5f)** — the number of oxygen atoms released per fission,
//! `OPF` (pages -488- to -491-).
//!
//! Oxygen freed when a `UO2` or `(Th,U)O2` kernel fissions forms CO, which
//! adds to the internal gas pressure alongside the fission gases themselves.
//! `OPF` enters Eq (3) directly, so it sets the **absolute** pressure.
//!
//! | Eq | Kernel | Regime | Source |
//! |---|---|---|---|
//! | (5a) | `(Th,U)O2` | any | Strigl 1984 |
//! | (5b) | `UO2` | before heating | Proksch 1982 |
//! | (5c) | `UO2` | during heating | Proksch 1982 |
//! | (5d) | `UCO` | any | zero by assumption |
//! | (5e) | all | cap | `OPF_max = 0.625` |
//!
//! # `t_B` is in SECONDS — settled by Fig. 3
//!
//! This was the last open units question, and it mattered more than any
//! other: `(5b)`/`(5c)` carry `2·log t_B`, so a seconds-vs-days confusion
//! moves `log OPF` by about **9.9 decades**, and `OPF` sets the absolute
//! pressure for everything downstream.
//!
//! The report's symbol list (-511-) says seconds. Fig. 3's curve labels say
//! `1000 °C, 1000 d`; Figs. 7 and 8's captions say `260 FPD` and `500 FPD`;
//! and page -488- gives the correlation's validity range as "66 and 550 full
//! power days". Three places in days against one in seconds — so the
//! extraction record left it open rather than guessing.
//!
//! Checked 2026-09-24 against a digitisation of Fig. 3's four labelled `UO2`
//! curves, spanning `T_B` 900–1100 °C and `t_B` 500–1000 d:
//!
//! | reading of `t_B` | mean abs error in `OPF` |
//! |---|---|
//! | **seconds** | **0.0087** |
//! | days | 0.277 — `OPF` collapses to ~0 everywhere |
//!
//! So the symbol list was right and the labels are simply human-readable:
//! the **curve** is titled in days, the **formula** takes seconds. That also
//! explains why the printed validity range is in full-power days — it
//! describes the experiments, not the argument.
//!
//! [`oxygen_per_fission_uo2`] therefore takes a `uom` [`Time`] and converts
//! internally, so a caller cannot get this wrong at all.

use uom::si::f64::{Ratio, ThermodynamicTemperature, Time};
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

/// **Eq (5e)** — the upper limit on `OPF`, 0.625 (page -489-).
///
/// The report notes that for `(Th,U)O2` this would only be exceeded above
/// 5700 K, i.e. never in practice; for `UO2` it binds at accident
/// temperatures and is visible as the horizontal dashed line in Fig. 3.
pub const OPF_MAX: f64 = 0.625;

/// The `+75 °C` correction in Eq (5c) (page -489-).
///
/// `T_B` is the particle **surface** temperature, while the temperature that
/// determines release inside a `UO2` TRISO kernel is about 75 degrees higher.
pub const KERNEL_SURFACE_OFFSET_K: f64 = 75.0;

/// **Eq (5a)** — `OPF` for a `(Th,U)O2` kernel (page -488-, Strigl 1984).
///
/// ```text
/// log10(OPF) = 0.96 - 0.442e4/T + 0.4*log10(N) + 0.3*log10(F_b)
/// ```
///
/// - `temperature` — `T`, the accident temperature.
/// - `thorium_to_u235` — `N`, the thorium / uranium-235 ratio. The report
///   gives `N = 5` for AVR and `N = 10` for THTR.
/// - `burnup` — `F_b`, in FIMA as a fraction.
///
/// Unlike the `UO2` correlations this has **no irradiation history** in it at
/// all — no `T_B`, no `t_B`. That asymmetry is the report's own: it states
/// that oxygen formation in `UO2` "is greatly dependent on the irradiation
/// history" and for `(Th,U)O2` it is not.
///
/// Capped at [`OPF_MAX`].
pub fn oxygen_per_fission_thoria(
    temperature: ThermodynamicTemperature,
    thorium_to_u235: f64,
    burnup: Ratio,
) -> Ratio {
    let t = temperature.get::<kelvin>();
    let f_b = burnup.get::<ratio>();
    if t <= 0.0 || thorium_to_u235 <= 0.0 || f_b <= 0.0 {
        return Ratio::new::<ratio>(0.0);
    }
    let log_opf = 0.96 - 0.442e4 / t + 0.4 * thorium_to_u235.log10() + 0.3 * f_b.log10();
    Ratio::new::<ratio>(10f64.powf(log_opf).min(OPF_MAX))
}

/// Which side of the accident a `UO2` `OPF` is wanted for.
///
/// `PartialEq` only: the `DuringHeating` arm carries a temperature, and a
/// float has no total equality.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HeatingRegime {
    /// **Eq (5b)** — the inventory built up during irradiation, before the
    /// accident begins. Depends only on the irradiation history.
    BeforeHeating,
    /// **Eq (5c)** — during the accident, at accident temperature `T`. Adds
    /// the `-0.404*(1e4/T - 1e4/(T_B + 75))` term.
    DuringHeating {
        /// `T`, the accident temperature.
        temperature: ThermodynamicTemperature,
    },
}

/// **Eqs (5b)/(5c)** — `OPF` for a `UO2` kernel (page -489-, Proksch 1982).
///
/// ```text
/// (5b)  log10(OPF) = -10.08 - 0.85e4/T_B + 2*log10(t_B)
/// (5c)  log10(OPF) = -10.08 - 0.85e4/T_B + 2*log10(t_B)
///                    - 0.404*(1e4/T - 1e4/(T_B + 75))
/// ```
///
/// - `irradiation_temperature` — `T_B`, the particle **surface** temperature
///   during irradiation.
/// - `irradiation_time` — `t_B`. **Enters the formula in seconds**; taking a
///   `uom` [`Time`] here is deliberate, because this was the one unit in the
///   whole report that three separate places disagreed about. See the module
///   docs.
///
/// The report states these are valid for a **constant** irradiation
/// temperature, over 66–550 full-power days and `T_B` of 950–1525 °C. Those
/// bounds are not enforced: the report itself plots Fig. 3 outside them
/// (curves at 1000 d), and silently clamping an input is worse than
/// returning what the correlation says.
///
/// Capped at [`OPF_MAX`].
pub fn oxygen_per_fission_uo2(
    irradiation_temperature: ThermodynamicTemperature,
    irradiation_time: Time,
    regime: HeatingRegime,
) -> Ratio {
    let t_b_k = irradiation_temperature.get::<kelvin>();
    let t_b_s = irradiation_time.get::<second>();
    if t_b_k <= 0.0 || t_b_s <= 0.0 {
        return Ratio::new::<ratio>(0.0);
    }
    let mut log_opf = -10.08 - 0.85e4 / t_b_k + 2.0 * t_b_s.log10();
    if let HeatingRegime::DuringHeating { temperature } = regime {
        let t = temperature.get::<kelvin>();
        if t <= 0.0 {
            return Ratio::new::<ratio>(0.0);
        }
        log_opf -= 0.404 * (1.0e4 / t - 1.0e4 / (t_b_k + KERNEL_SURFACE_OFFSET_K));
    }
    Ratio::new::<ratio>(10f64.powf(log_opf).min(OPF_MAX))
}

/// **Eq (5d)** — `OPF` for a `UCO` kernel: zero (page -489-).
///
/// "No oxygen production is assumed to happen in particles with UCO
/// kernels." A function rather than a bare constant so a caller dispatching
/// on kernel type reads the same at every arm.
pub fn oxygen_per_fission_uco() -> Ratio {
    Ratio::new::<ratio>(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::thermodynamic_temperature::degree_celsius;
    use uom::si::time::day;

    fn uo2(t_c: f64, t_b_c: f64, t_b_days: f64) -> f64 {
        oxygen_per_fission_uo2(
            ThermodynamicTemperature::new::<degree_celsius>(t_b_c),
            Time::new::<day>(t_b_days),
            HeatingRegime::DuringHeating {
                temperature: ThermodynamicTemperature::new::<degree_celsius>(t_c),
            },
        )
        .get::<ratio>()
    }

    /// **Eq (5c) reproduces Fig. 3, with `t_B` in seconds.**
    ///
    /// Sample points from a digitisation of the figure's four labelled `UO2`
    /// curves (maintainer, 2026-09-24), spanning `T_B` 900-1100 degC and
    /// `t_B` 500-1000 d. Mean absolute error over the full set is 0.0087 in
    /// `OPF`, on an axis running 0 to 0.7.
    #[test]
    fn figure_3_is_reproduced() {
        // (accident T degC, T_B degC, t_B days, digitised OPF)
        let samples = [
            (1549.0, 900.0, 500.0, 0.089),
            (2106.0, 900.0, 500.0, 0.324),
            (1459.0, 1000.0, 500.0, 0.153),
            (1891.0, 1000.0, 500.0, 0.457),
            (1129.0, 1000.0, 1000.0, 0.169),
            (1347.0, 1000.0, 1000.0, 0.436),
            (1032.0, 1100.0, 1000.0, 0.194),
            (1168.0, 1100.0, 1000.0, 0.391),
        ];
        for (t, t_b, days, want) in samples {
            let got = uo2(t, t_b, days);
            assert!(
                (got - want).abs() < 0.03,
                "T={t}, T_B={t_b}, t_B={days} d: Eq (5c) gives {got:.3}, figure {want:.3}"
            );
        }
    }

    /// **`t_B` read as DAYS is excluded by the figure.**
    ///
    /// `2*log10(t_B)` means the two readings differ by ~9.9 decades, so this
    /// is the most strongly discriminated unit question in the whole report.
    /// Pins it so the seconds reading cannot drift back.
    #[test]
    fn reading_t_b_as_days_collapses_the_answer() {
        // Feed 1000 *seconds* where 1000 days is meant -- i.e. the mistake.
        let wrong = oxygen_per_fission_uo2(
            ThermodynamicTemperature::new::<degree_celsius>(1000.0),
            Time::new::<second>(1000.0),
            HeatingRegime::DuringHeating {
                temperature: ThermodynamicTemperature::new::<degree_celsius>(1347.0),
            },
        )
        .get::<ratio>();
        let right = uo2(1347.0, 1000.0, 1000.0);
        assert!(
            right > 0.4,
            "the correct reading reproduces the figure: {right:.3}"
        );
        assert!(
            wrong < 1e-6,
            "the days-as-seconds mistake must collapse OPF, got {wrong:e}"
        );
    }

    /// Eq (5c) minus Eq (5b) is exactly the heating term, and at
    /// `T = T_B + 75` the two coincide -- the accident has not yet moved the
    /// kernel off its irradiation temperature.
    #[test]
    fn the_heating_term_vanishes_at_the_irradiation_temperature() {
        let t_b = ThermodynamicTemperature::new::<degree_celsius>(1000.0);
        let t_b_time = Time::new::<day>(500.0);
        let before =
            oxygen_per_fission_uo2(t_b, t_b_time, HeatingRegime::BeforeHeating).get::<ratio>();
        // T such that 1e4/T == 1e4/(T_B + 75): T = T_B + 75 in KELVIN.
        let matched =
            ThermodynamicTemperature::new::<kelvin>(t_b.get::<kelvin>() + KERNEL_SURFACE_OFFSET_K);
        let during = oxygen_per_fission_uo2(
            t_b,
            t_b_time,
            HeatingRegime::DuringHeating {
                temperature: matched,
            },
        )
        .get::<ratio>();
        assert!(
            (before - during).abs() / before < 1e-12,
            "before {before:e} vs during {during:e}"
        );
    }

    /// **Eq (5a)'s temperature term is right.** Inverting it on the dashed
    /// `(Th,U)O2` curve leaves a constant, which is what says the
    /// `0.442e4/T` dependence matches the figure rather than merely passing
    /// through one point. The constant recovers a plausible burnup: 11.7 %
    /// FIMA at `N = 5`, 4.7 % at `N = 10`.
    #[test]
    fn the_thoria_curve_has_the_right_temperature_dependence() {
        // (accident T degC, digitised OPF) along the dashed curve.
        let pts: [(f64, f64); 4] = [
            (1316.4, 0.0115),
            (1709.7, 0.0538),
            (2099.9, 0.1384),
            (2529.5, 0.2645),
        ];
        let c: Vec<f64> = pts
            .iter()
            .map(|(t, opf)| opf.log10() - 0.96 + 0.442e4 / (t + 273.15))
            .collect();
        let mean = c.iter().sum::<f64>() / c.len() as f64;
        for (i, v) in c.iter().enumerate() {
            assert!(
                (v - mean).abs() < 0.12,
                "point {i}: implied constant {v:.3} strays from {mean:.3} -- the \
                 0.442e4/T term would be wrong"
            );
        }
        // And that constant is a physical burnup at the report's own N values.
        let f_b_at_n5 = 10f64.powf((mean - 0.4 * 5f64.log10()) / 0.3);
        assert!(
            (0.01..0.25).contains(&f_b_at_n5),
            "implied F_b at N=5 is {f_b_at_n5:.4}, outside anything plausible"
        );
    }

    /// The cap binds, UCO is zero, and degenerate inputs give zero rather
    /// than a NaN.
    #[test]
    fn the_cap_and_the_degenerate_cases_hold() {
        // A long, hot irradiation drives Eq (5b) past the cap.
        let capped = oxygen_per_fission_uo2(
            ThermodynamicTemperature::new::<degree_celsius>(1500.0),
            Time::new::<day>(5000.0),
            HeatingRegime::BeforeHeating,
        )
        .get::<ratio>();
        assert_eq!(capped, OPF_MAX);

        assert_eq!(oxygen_per_fission_uco().get::<ratio>(), 0.0);
        assert_eq!(
            oxygen_per_fission_uo2(
                ThermodynamicTemperature::new::<degree_celsius>(1000.0),
                Time::new::<second>(0.0),
                HeatingRegime::BeforeHeating
            )
            .get::<ratio>(),
            0.0,
            "zero irradiation time is zero OPF, not a log of zero"
        );
        assert_eq!(
            oxygen_per_fission_thoria(
                ThermodynamicTemperature::new::<degree_celsius>(1600.0),
                5.0,
                Ratio::new::<ratio>(0.0)
            )
            .get::<ratio>(),
            0.0,
            "zero burnup is zero OPF"
        );
    }
}
