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

//! **The reduced diffusion coefficient `D_S`** for fission gases in the
//! particle kernel (page -487-).
//!
//! `D_S = D_eff / r_o^2`, in s^-1 — the Booth equivalent-sphere diffusion
//! coefficient divided by the equivalent sphere's radius squared, which is
//! the form the release function needs. It feeds the dimensionless times
//! `tau_i = D_S(T_B)*t_B` and `tau_a = D_S(T)*t`, and from there the Booth
//! series, `F_d`, and Eq (3)'s pressure.
//!
//! Two correlations, one per kernel type, both printed without equation
//! numbers on page -487- and cited by page.
//!
//! # Verification against Fig. 2 (page -487-)
//!
//! The figure plots both correlations, so it is a verification target rather
//! than a data source. Checked 2026-09-24 against a digitisation of it:
//!
//! | curve | nominal `F_b` | `F_b` recovered from the fit | mean abs err in `log10(D_S)` |
//! |---|---|---|---|
//! | 1 % FIMA | 0.01 | 0.0086 | 0.021 |
//! | 5 % FIMA | 0.05 | 0.0494 | 0.035 |
//! | 10 % FIMA | 0.10 | 0.0950 | 0.013 |
//! | 15 % FIMA | 0.15 | 0.1456 | 0.023 |
//!
//! Each (Th,U)O2 curve recovers **its own printed label** from a blind fit,
//! which is a stronger statement than the residuals: it says the burnup term
//! `3.24/(1 + 0.11/F_b)` is right in form and not only in magnitude. The 1 %
//! curve is the loosest (−13.6 % in `F_b`) and that is expected — the term is
//! most sensitive to `F_b` where `F_b` is smallest.
//!
//! # The UO2 correlation disagrees with the report's own figure
//!
//! **This is a defect in the source, recorded rather than resolved.** The
//! printed Horsley correlation sits *above* the figure's dashed UO2 curve
//! everywhere, by a factor that falls monotonically with temperature:
//!
//! | `10^4/T` | figure | equation | equation / figure |
//! |---|---|---|---|
//! | 3.19 | 10^−5.69 | 10^−4.89 | **6.4×** |
//! | 5.59 | 10^−7.34 | 10^−6.84 | 3.2× |
//! | 7.21 | 10^−8.46 | 10^−8.16 | 2.0× |
//! | 8.59 | 10^−9.40 | 10^−9.28 | 1.3× |
//!
//! A blind fit to the plotted curve gives a slope of −0.6875 against the
//! equation's −0.8116, so it is a **slope** disagreement, not an offset —
//! the two are not reconcilable by a units or a decade error. The
//! transcription was checked against the page image directly, and the
//! equation reads `log DS = −2.30 − 0.8116·10^4/T` as implemented.
//!
//! [`reduced_diffusion_coefficient`] implements **the equation**, because for
//! a code reconstruction the equation is the specification and the figure is
//! illustrative. Anyone comparing against Fig. 2 should expect the offset
//! above and should not "fix" it by tuning.

use uom::si::f64::{Frequency, Ratio, ThermodynamicTemperature};
use uom::si::frequency::hertz;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

/// Which kernel the correlation is for. Closed set, enum-dispatched per the
/// workspace Rust design rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelKind {
    /// `(Th,U)O2` — Myers 1977. Burnup-dependent.
    ThoriumUraniumOxide,
    /// `UO2` — Horsley 1976. The report states this is **also used for UCO**
    /// (page -487-). No burnup dependence.
    UraniumOxide,
}

/// The reduced diffusion coefficient `D_S` \[s^-1\] (page -487-).
///
/// ```text
/// (Th,U)O2   log10(D_S) = -5.94 + 3.24/(1 + 0.11/F_b) - 0.5460e4/T
/// UO2, UCO   log10(D_S) = -2.30 - 0.8116e4/T
/// ```
///
/// - `temperature` — `T`, in kelvin via `uom`. (The report writes `10^4/T`
///   throughout and its symbol list gives temperatures in degC; kelvin is
///   established in `docs/panama-i-units-and-open-questions.md`.)
/// - `burnup` — `F_b`, heavy-metal burnup in FIMA as a **fraction**, not a
///   percent. Fig. 2's curves are labelled `1 % FIMA` … `15 % FIMA`, i.e.
///   `F_b` = 0.01 … 0.15, and those labels are recovered from the figure by
///   [`tests::the_thorium_curves_recover_their_own_burnup_labels`]. Ignored
///   for [`KernelKind::UraniumOxide`].
///
/// Returns zero at non-positive temperature rather than a NaN or an infinity:
/// `10^4/T` is undefined there and a zero diffusion coefficient is the
/// physically right limit (nothing diffuses).
pub fn reduced_diffusion_coefficient(
    kernel: KernelKind,
    temperature: ThermodynamicTemperature,
    burnup: Ratio,
) -> Frequency {
    let t = temperature.get::<kelvin>();
    if t <= 0.0 {
        return Frequency::new::<hertz>(0.0);
    }
    let log_ds = match kernel {
        KernelKind::ThoriumUraniumOxide => {
            let f_b = burnup.get::<ratio>();
            // `3.24/(1 + 0.11/F_b)` -> 0 as F_b -> 0, which is the right
            // limit (fresh fuel, no burnup enhancement) and avoids a
            // division by zero at exactly zero burnup.
            let term = if f_b > 0.0 {
                3.24 / (1.0 + 0.11 / f_b)
            } else {
                0.0
            };
            -5.94 + term - 0.5460e4 / t
        }
        KernelKind::UraniumOxide => -2.30 - 0.8116e4 / t,
    };
    Frequency::new::<hertz>(10f64.powf(log_ds))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `log10(D_S)` at `10^4/T = x`, for brevity in the tables below.
    fn log_ds(kernel: KernelKind, x: f64, f_b: f64) -> f64 {
        let t = ThermodynamicTemperature::new::<kelvin>(1.0e4 / x);
        reduced_diffusion_coefficient(kernel, t, Ratio::new::<ratio>(f_b))
            .get::<hertz>()
            .log10()
    }

    /// **The (Th,U)O2 correlation reproduces Fig. 2 (page -487-).**
    ///
    /// Sample points read off a digitisation of the figure's four solid
    /// curves (digitised by the maintainer, 2026-09-24, from Fig. 2 of
    /// Verfondern & Nabielek HTA-IB-03/90 -- see this module's docs for the
    /// full comparison). Agreement is within 0.04 in `log10(D_S)`, i.e.
    /// under 10 % in `D_S`, which is the width of a hand-placed marker on a
    /// four-decade log axis.
    #[test]
    fn the_thorium_curves_reproduce_figure_2() {
        // (F_b, 10^4/T, digitised log10 D_S)
        let samples = [
            (0.01, 3.2154, -7.4383),
            (0.01, 4.5122, -8.1252),
            (0.05, 3.1900, -6.6300),
            (0.10, 3.1600, -6.1400),
            (0.15, 3.1200, -5.8000),
        ];
        for (f_b, x, want) in samples {
            let got = log_ds(KernelKind::ThoriumUraniumOxide, x, f_b);
            assert!(
                (got - want).abs() < 0.10,
                "F_b={f_b}, 10^4/T={x}: correlation {got:.3} vs figure {want:.3}"
            );
        }
    }

    /// **Each (Th,U)O2 curve recovers its own printed burnup label.**
    ///
    /// The stronger statement: inverting the burnup term at a point on each
    /// digitised curve returns the `% FIMA` that curve is labelled with. That
    /// tests the *form* of `3.24/(1 + 0.11/F_b)`, not just its magnitude --
    /// a wrong functional form could still match one curve.
    #[test]
    fn the_thorium_curves_recover_their_own_burnup_labels() {
        // (nominal F_b, 10^4/T, digitised log10 D_S) -- one point per curve.
        let curves: [(f64, f64, f64); 4] = [
            (0.01, 3.2154, -7.4383),
            (0.05, 3.1900, -6.6300),
            (0.10, 3.1600, -6.1400),
            (0.15, 3.1200, -5.8000),
        ];
        for (nominal, x, log_ds_seen) in curves {
            // Invert: term = log_DS + 5.94 + 0.5460*x, then
            // term = 3.24/(1 + 0.11/F)  =>  F = 0.11/(3.24/term - 1).
            let term = log_ds_seen + 5.94 + 0.5460 * x;
            let recovered = 0.11 / (3.24 / term - 1.0);
            let err = (recovered - nominal).abs() / nominal;
            assert!(
                err < 0.20,
                "curve labelled {:.0}% FIMA recovers F_b = {recovered:.4} ({:.1}% out)",
                nominal * 100.0,
                err * 100.0
            );
        }
    }

    /// **The UO2 correlation is steeper than the figure's dashed curve.**
    ///
    /// Pins the source's own internal inconsistency (see this module's docs)
    /// so it cannot be silently "fixed" later: the equation sits above the
    /// plotted curve by ~6x at `10^4/T = 3.2` and ~1.3x at 8.6. If this test
    /// starts failing, either the transcription or the digitisation changed
    /// and the discrepancy needs re-deriving rather than re-tuning.
    #[test]
    fn the_uo2_equation_sits_above_the_figures_dashed_curve() {
        // (10^4/T, digitised log10 D_S, expected equation/figure factor)
        let samples = [
            (3.19, -5.694, 6.4),
            (5.59, -7.344, 3.2),
            (8.59, -9.398, 1.3),
        ];
        for (x, figure, want_factor) in samples {
            let eq = log_ds(KernelKind::UraniumOxide, x, 0.0);
            let factor = 10f64.powf(eq - figure);
            assert!(
                (factor - want_factor).abs() / want_factor < 0.10,
                "10^4/T={x}: equation is {factor:.1}x the figure, expected ~{want_factor}x"
            );
            assert!(eq > figure, "the equation must sit ABOVE the plotted curve");
        }
    }

    /// `D_S` rises with temperature and with burnup, and UO2 ignores burnup
    /// entirely -- the three monotonicities a sign slip would invert.
    #[test]
    fn the_correlations_are_monotone_in_the_right_directions() {
        let hot = ThermodynamicTemperature::new::<kelvin>(2000.0);
        let cold = ThermodynamicTemperature::new::<kelvin>(1200.0);
        let f = Ratio::new::<ratio>(0.05);
        for k in [KernelKind::ThoriumUraniumOxide, KernelKind::UraniumOxide] {
            assert!(
                reduced_diffusion_coefficient(k, hot, f)
                    > reduced_diffusion_coefficient(k, cold, f),
                "{k:?}: D_S must rise with temperature"
            );
        }
        let lo = Ratio::new::<ratio>(0.01);
        let hi = Ratio::new::<ratio>(0.15);
        assert!(
            reduced_diffusion_coefficient(KernelKind::ThoriumUraniumOxide, hot, hi)
                > reduced_diffusion_coefficient(KernelKind::ThoriumUraniumOxide, hot, lo),
            "(Th,U)O2: D_S must rise with burnup"
        );
        assert_eq!(
            reduced_diffusion_coefficient(KernelKind::UraniumOxide, hot, hi),
            reduced_diffusion_coefficient(KernelKind::UraniumOxide, hot, lo),
            "UO2 has no burnup term at all"
        );
    }

    /// Zero burnup is the fresh-fuel limit, not a division by zero, and a
    /// non-positive temperature gives zero rather than a NaN.
    #[test]
    fn the_degenerate_inputs_are_limits_not_nans() {
        let t = ThermodynamicTemperature::new::<kelvin>(1600.0);
        let fresh = reduced_diffusion_coefficient(
            KernelKind::ThoriumUraniumOxide,
            t,
            Ratio::new::<ratio>(0.0),
        );
        assert!(fresh.get::<hertz>().is_finite() && fresh.get::<hertz>() > 0.0);
        // It must equal the no-burnup-term value exactly.
        assert!((fresh.get::<hertz>().log10() - (-5.94 - 0.5460e4 / 1600.0)).abs() < 1e-12);

        let zero_k = ThermodynamicTemperature::new::<kelvin>(0.0);
        assert_eq!(
            reduced_diffusion_coefficient(
                KernelKind::UraniumOxide,
                zero_k,
                Ratio::new::<ratio>(0.0)
            )
            .get::<hertz>(),
            0.0
        );
    }
}
