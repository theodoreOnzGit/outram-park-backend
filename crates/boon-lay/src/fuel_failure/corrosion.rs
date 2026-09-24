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

//! **SiC layer thinning by volume corrosion** — Eq (7) and the corrosion-rate
//! Arrhenius (page -492-, attributed to Montgomery 1981).
//!
//! ```text
//! d_act = d_o / (1 + v̇·t/d_o) = d_o / FKOR        (7)
//! FKOR(t2) = FKOR(t1) + v̇(T_m)·(t2 − t1)/d_o
//! v̇ = A · exp(−179500/(R·T))   [m/s]
//! ```
//!
//! `FKOR` is carried forward across time steps rather than recomputed from a
//! total elapsed time, which is what lets a **varying** temperature history
//! accumulate correctly: each step adds `v̇(T_m)·Δt/d_o` at that step's own
//! mean temperature.
//!
//! # The printed pre-factor is a decade out, and Fig. 4 proves it
//!
//! The report prints `v̇ = 5.87·10⁻⁷ · exp(−179500/(R·T))`. That value does
//! **not** reproduce Fig. 4 on the facing page, which plots this very
//! equation for `d_o = 35 µm` at five isothermal temperatures. Checked
//! 2026-09-24 against a digitisation of all five curves (483 points):
//!
//! | pre-factor | mean abs error in `d_act/d_o` | worst |
//! |---|---|---|
//! | `5.87e-7` as printed | 0.12 – 0.51 per curve | 0.51 |
//! | **`5.87e-8`** | **0.0055** | 0.0142 |
//!
//! On an axis running 0 to 1, 0.0055 is digitisation noise. Independently,
//! fitting `v̇` freely from the figure gives an activation energy of
//! **160.8 kJ/mol** against the printed 179.5 — but with the pre-factor
//! corrected the printed activation energy fits every curve, so the free fit
//! was absorbing the decade rather than finding a different energy.
//!
//! [`PRINTED_PREFACTOR`] and [`FIGURE_PREFACTOR`] are both exposed and the
//! **figure's** value is the default, because it is the one the report's own
//! plotted output is consistent with. This is a departure from the rule used
//! for `D_S` in [`super::diffusion`], where the equation was preferred over
//! the figure — the difference is that there the two disagreed in *slope*,
//! with no single parameter reconciling them, whereas here one factor of ten
//! reconciles five curves over a 1000 °C span. That is a typo, not a
//! modelling choice.

use uom::si::f64::{Length, Ratio, ThermodynamicTemperature, Time, Velocity};
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::velocity::meter_per_second;

/// Activation energy in the corrosion-rate Arrhenius, J/mol (page -492-).
pub const CORROSION_ACTIVATION_J_PER_MOL: f64 = 179_500.0;

/// The pre-factor **as printed** on page -492-, `5.87e-7` m/s.
///
/// Does not reproduce Fig. 4; see the module docs. Exposed so the
/// discrepancy can be reproduced rather than only described.
pub const PRINTED_PREFACTOR: f64 = 5.87e-7;

/// The pre-factor **Fig. 4 is consistent with**, `5.87e-8` m/s — the printed
/// value divided by ten. This is the default.
pub const FIGURE_PREFACTOR: f64 = 5.87e-8;

/// The SiC volume-corrosion rate `v̇` \[m/s\] (page -492-, Montgomery 1981).
///
/// Uses [`FIGURE_PREFACTOR`]; see the module docs for why, and
/// [`corrosion_rate_with`] to evaluate the printed value instead.
pub fn corrosion_rate(temperature: ThermodynamicTemperature) -> Velocity {
    corrosion_rate_with(FIGURE_PREFACTOR, temperature)
}

/// [`corrosion_rate`] with an explicit pre-factor, so the printed-vs-figure
/// discrepancy is reproducible rather than merely documented.
pub fn corrosion_rate_with(prefactor: f64, temperature: ThermodynamicTemperature) -> Velocity {
    let t = temperature.get::<kelvin>();
    if t <= 0.0 {
        return Velocity::new::<meter_per_second>(0.0);
    }
    let r = super::pressure::GAS_CONSTANT_J_PER_MOL_K;
    Velocity::new::<meter_per_second>(prefactor * (-CORROSION_ACTIVATION_J_PER_MOL / (r * t)).exp())
}

/// `FKOR` after holding at a constant `temperature` for `elapsed`, starting
/// from an uncorroded layer (page -492-).
///
/// `FKOR = 1 + v̇·t/d_o`, so `d_act = d_o/FKOR`. Starts at 1, never below it.
pub fn thinning_factor(
    initial_thickness: Length,
    temperature: ThermodynamicTemperature,
    elapsed: Time,
) -> Ratio {
    let advance: Ratio = corrosion_rate(temperature) * elapsed / initial_thickness;
    Ratio::new::<ratio>(1.0) + advance
}

/// Advance `FKOR` across one time step at mean temperature `t_m`
/// (page -492-):
///
/// ```text
/// FKOR(t2) = FKOR(t1) + v̇(T_m)·(t2 − t1)/d_o
/// ```
///
/// Carried forward rather than recomputed from total elapsed time: that is
/// what makes a **varying** temperature history accumulate correctly, since
/// each step contributes at its own temperature. Recomputing from `t_total`
/// at the current temperature would silently apply the latest temperature to
/// the whole history.
pub fn advance_thinning_factor(
    previous: Ratio,
    initial_thickness: Length,
    mean_temperature: ThermodynamicTemperature,
    step: Time,
) -> Ratio {
    let advance: Ratio = corrosion_rate(mean_temperature) * step / initial_thickness;
    previous + advance
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::length::micrometer;
    use uom::si::thermodynamic_temperature::degree_celsius;
    use uom::si::time::hour;

    fn d_o() -> Length {
        Length::new::<micrometer>(35.0) // Fig. 4's stated initial thickness
    }

    fn ratio_at(t_c: f64, hours: f64) -> f64 {
        let f = thinning_factor(
            d_o(),
            ThermodynamicTemperature::new::<degree_celsius>(t_c),
            Time::new::<hour>(hours),
        );
        1.0 / f.get::<ratio>()
    }

    /// **Eq (7) reproduces Fig. 4 — with the pre-factor read as `5.87e-8`.**
    ///
    /// Fig. 4 (page -493-) plots this equation for `d_o = 35 µm` at five
    /// isothermal temperatures, so it is a direct verification target.
    /// Sample points from a digitisation of all five curves (maintainer,
    /// 2026-09-24; 483 points total).
    ///
    /// Against the **printed** `5.87e-7` the error runs 0.12 to 0.51 in
    /// `d_act/d_o`; against `5.87e-8` it is 0.0055 mean and 0.0142 worst
    /// across every point, on an axis running 0 to 1. One factor of ten
    /// reconciles five curves over a 1000 degC span, which is a typo rather
    /// than a modelling difference.
    #[test]
    fn figure_4_is_reproduced_with_the_corrected_prefactor() {
        // (T degC, t hours, digitised d_act/d_o)
        let samples = [
            (1600.0, 493.0, 0.964),
            (2000.0, 264.0, 0.891),
            (2000.0, 490.0, 0.810),
            (2200.0, 485.0, 0.674),
            (2400.0, 253.0, 0.671),
            (2400.0, 492.0, 0.510),
            (2600.0, 243.0, 0.561),
            (2600.0, 485.0, 0.373),
        ];
        for (t_c, hours, want) in samples {
            let got = ratio_at(t_c, hours);
            assert!(
                (got - want).abs() < 0.02,
                "{t_c} degC at {hours} h: Eq (7) gives {got:.3}, figure shows {want:.3}"
            );
        }
    }

    /// **The printed pre-factor does NOT reproduce Fig. 4.** Pins the
    /// discrepancy so the correction cannot be quietly undone, and so that
    /// anyone who re-reads the page and "fixes" the constant back sees this
    /// fail with the reason.
    #[test]
    fn the_printed_prefactor_is_excluded_by_the_figure() {
        let t = ThermodynamicTemperature::new::<degree_celsius>(2600.0);
        let elapsed = Time::new::<hour>(485.0);
        let printed = corrosion_rate_with(PRINTED_PREFACTOR, t);
        let advance: Ratio = printed * elapsed / d_o();
        let with_printed = 1.0 / (1.0 + advance.get::<ratio>());
        assert!(
            (with_printed - 0.373).abs() > 0.2,
            "the printed pre-factor gives {with_printed:.3} where the figure \
             shows 0.373 -- if these have become close, re-derive rather than \
             re-tune"
        );
        assert!((PRINTED_PREFACTOR / FIGURE_PREFACTOR - 10.0).abs() < 1e-9);
    }

    /// `FKOR` accumulated step by step over a varying history must match a
    /// single call only when the temperature is CONSTANT -- that is the whole
    /// reason the report carries it forward instead of recomputing from total
    /// elapsed time.
    #[test]
    fn stepping_matches_a_single_call_at_constant_temperature() {
        let t = ThermodynamicTemperature::new::<degree_celsius>(2200.0);
        let total = Time::new::<hour>(300.0);
        let one = thinning_factor(d_o(), t, total).get::<ratio>();

        let mut f = Ratio::new::<ratio>(1.0);
        for _ in 0..30 {
            f = advance_thinning_factor(f, d_o(), t, Time::new::<hour>(10.0));
        }
        assert!(
            (f.get::<ratio>() - one).abs() < 1e-9,
            "{} vs {one}",
            f.get::<ratio>()
        );
    }

    /// A hot-then-cold history must thin MORE than cold-then-hot would if the
    /// rate were applied at the final temperature throughout -- i.e. the
    /// accumulation really is per-step. Recomputing from total elapsed time
    /// at the current temperature would make the two equal.
    #[test]
    fn a_varying_history_accumulates_at_each_steps_own_temperature() {
        let hot = ThermodynamicTemperature::new::<degree_celsius>(2600.0);
        let cold = ThermodynamicTemperature::new::<degree_celsius>(1600.0);
        let step = Time::new::<hour>(100.0);

        let mut f = Ratio::new::<ratio>(1.0);
        f = advance_thinning_factor(f, d_o(), hot, step);
        f = advance_thinning_factor(f, d_o(), cold, step);

        // The naive "recompute at the final temperature" answer:
        let naive = thinning_factor(d_o(), cold, Time::new::<hour>(200.0)).get::<ratio>();
        // Compare the ADVANCES, not the factors: FKOR starts at 1, so the
        // interesting quantity is how far past 1 each got.
        let stepped_advance = f.get::<ratio>() - 1.0;
        let naive_advance = naive - 1.0;
        assert!(
            stepped_advance > naive_advance * 10.0,
            "the hot leg must dominate: stepped advance {stepped_advance:.4} vs \
             naive {naive_advance:.4}"
        );
    }

    /// The rate rises with temperature and is zero at absolute zero rather
    /// than a NaN.
    #[test]
    fn the_rate_is_monotone_and_safe_at_zero() {
        let a = corrosion_rate(ThermodynamicTemperature::new::<degree_celsius>(1600.0));
        let b = corrosion_rate(ThermodynamicTemperature::new::<degree_celsius>(2600.0));
        assert!(b > a, "corrosion must accelerate with temperature");
        assert_eq!(
            corrosion_rate(ThermodynamicTemperature::new::<kelvin>(0.0)).get::<meter_per_second>(),
            0.0
        );
    }
}
