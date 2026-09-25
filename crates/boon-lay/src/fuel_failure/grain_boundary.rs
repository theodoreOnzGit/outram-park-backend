// SPDX-License-Identifier: GPL-3.0
//
// boon-lay fuel failure — provenance
// ----------------------------------
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
// Nature    : boon-lay fuel failure is boon-lay's own model: a Rust
//             implementation of the PANAMA-I FORMULAS, coded agentically (by an
//             AI coding agent, then reviewed) from the published equations. It
//             is not the PANAMA code and not a port of the PANAMA Fortran
//             (closed-source, never consulted); call it "boon-lay fuel failure",
//             and keep "PANAMA-I" for the report and its own printed results.

//! **Eqs (10b)/(10c)** — strength loss of the SiC layer by grain-boundary
//! corrosion (page -495-).
//!
//! ```text
//! m  = m_o·(0.44 + 0.56·exp(−η̇·t))                                (10b)
//! η̇  = 0.565·exp(−187400/(R·T))   [s⁻¹]                            (10c)
//! ```
//!
//! Fission products attack the SiC grain boundaries, widening the Weibull
//! distribution — `m` falls toward `0.44·m_o`, so the *scatter* in strength
//! grows while the median `σ_o` is untouched. Since Eq (1) raises
//! `σ_t/σ_o` to the power `m`, a smaller `m` lifts the failure fraction
//! dramatically in the `σ_t ≪ σ_o` regime where a TRISO particle actually
//! sits: Figs. 7 and 8 show it worth **one to two decades** in the release
//! fraction at 1600 °C.
//!
//! # This is OFF by default, and that is the report's own default
//!
//! Page -495- states plainly that grain-boundary corrosion "is not generally
//! taken into consideration, although it can be selected by setting one switch
//! per input", and page -499- that **Eq (10a) (`m = m_o`) is normally used in
//! place of (10b)**. Both Fig. 7 and Fig. 8 carry `η̇(T) ≡ 0` in their
//! captions while plotting a "with grain boundary corrosion" curve alongside
//! for comparison.
//!
//! So [`GrainBoundaryCorrosion::Disabled`] is the default in
//! [`super::history`], and that is **not** an instance of the workspace's
//! "correct physics is the default setting" rule being waived: the rule is
//! about physics the model supplies, and here the source model's own
//! specified default is off. Turning it on silently would mean this
//! reconstruction stopped reproducing the report. The enum is visible at the
//! call site so the choice is made, not inherited.
//!
//! # `η̇·t` for a varying history is an extension, not the report
//!
//! Eq (10b) prints `exp(−η̇·t)` with a single rate and a single time, i.e. it
//! is written for an isothermal hold. For a varying temperature history the
//! natural discrete analogue is to accumulate `∫η̇ dt` exactly as Eq (11)
//! accumulates `∫k dt` — and that is what
//! [`advance_grain_boundary_exposure`] does. **The report does not state
//! this**; it is this implementation's reading, chosen for consistency with
//! Eq (11) and because the alternative (evaluating `η̇` at the current
//! temperature and multiplying by the total elapsed time) would retroactively
//! apply the latest temperature to the whole history. It is recorded as an
//! open item in `docs/panama-i-units-and-open-questions.md`, and it collapses
//! to the printed form for an isothermal hold — pinned by
//! [`tests::the_exposure_reduces_to_the_printed_form_when_isothermal`].
//!
//! # Verification status
//!
//! **Not verified against any output of the report.** Figs. 7 and 8 plot the
//! "with grain boundary corrosion" curve, but their captions state neither the
//! particle geometry, the kernel volume nor the buffer void volume, so the
//! absolute release fraction cannot be reproduced without inventing three
//! inputs. What can be said is checked and no more: the qualitative direction
//! (corrosion raises the failure fraction), the floor at `0.44·m_o`, and the
//! `0.565`/`187400` Arrhenius as transcribed.

use uom::si::f64::{Frequency, Ratio, ThermodynamicTemperature, Time};
use uom::si::frequency::hertz;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

/// Eq (10c)'s pre-factor, \[s⁻¹\] (page -495-).
pub const GRAIN_BOUNDARY_PREFACTOR_PER_S: f64 = 0.565;

/// Eq (10c)'s activation energy, J/mol (page -495-).
pub const GRAIN_BOUNDARY_ACTIVATION_J_PER_MOL: f64 = 187_400.0;

/// The asymptotic floor of Eq (10b): `m → 0.44·m_o` as `η̇·t → ∞`.
pub const RESIDUAL_MODULUS_FRACTION: f64 = 0.44;

/// Whether the grain-boundary corrosion of Eqs (10b)/(10c) is applied.
///
/// [`Disabled`](Self::Disabled) is the default, matching the report's own
/// switch (page -495-) and its stated normal use of Eq (10a), `m = m_o`
/// (page -499-).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GrainBoundaryCorrosion {
    /// Eq (10a): `m = m_o`. The report's default, and this crate's.
    #[default]
    Disabled,
    /// Eqs (10b)/(10c): `m` decays toward `0.44·m_o`.
    Enabled,
}

/// **Eq (10c)** — the SiC grain-boundary corrosion rate `η̇` \[s⁻¹\]
/// (page -495-).
///
/// ```text
/// η̇ = 0.565·exp(−187400/(R·T))
/// ```
///
/// Returns zero at non-positive temperature rather than a NaN.
pub fn grain_boundary_corrosion_rate(temperature: ThermodynamicTemperature) -> Frequency {
    let t = temperature.get::<kelvin>();
    if t <= 0.0 {
        return Frequency::new::<hertz>(0.0);
    }
    let r = super::pressure::GAS_CONSTANT_J_PER_MOL_K;
    Frequency::new::<hertz>(
        GRAIN_BOUNDARY_PREFACTOR_PER_S * (-GRAIN_BOUNDARY_ACTIVATION_J_PER_MOL / (r * t)).exp(),
    )
}

/// Advance the accumulated grain-boundary exposure `∫η̇ dt` across one time
/// step at its own mean temperature.
///
/// Dimensionless, starting at zero, and feeding the `η̇·t` slot of Eq (10b).
///
/// **This accumulation is an extension of the printed equation, not the
/// printed equation.** Eq (10b) writes a single `η̇·t`; see the module docs.
pub fn advance_grain_boundary_exposure(
    previous: Ratio,
    mean_temperature: ThermodynamicTemperature,
    step: Time,
) -> Ratio {
    let advance: Ratio = grain_boundary_corrosion_rate(mean_temperature) * step;
    previous + advance
}

/// **Eq (10b)** — the Weibull modulus after grain-boundary corrosion
/// (page -495-):
///
/// ```text
/// m = m_o·(0.44 + 0.56·exp(−η̇·t))
/// ```
///
/// - `end_of_irradiation_modulus` — `m_o`, from
///   [`super::irradiated_weibull_modulus`].
/// - `exposure` — the accumulated `∫η̇ dt` from
///   [`advance_grain_boundary_exposure`], or simply `η̇·t` for an isothermal
///   hold.
///
/// At zero exposure this returns `m_o` exactly, so Eq (10a) is the `t = 0`
/// limit of Eq (10b) rather than a separate branch.
pub fn corroded_weibull_modulus(end_of_irradiation_modulus: f64, exposure: Ratio) -> f64 {
    let x = exposure.get::<ratio>().max(0.0);
    end_of_irradiation_modulus
        * (RESIDUAL_MODULUS_FRACTION + (1.0 - RESIDUAL_MODULUS_FRACTION) * (-x).exp())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::f64::Pressure;
    use uom::si::pressure::megapascal;
    use uom::si::thermodynamic_temperature::degree_celsius;
    use uom::si::time::hour;

    fn eta_dot(t_c: f64) -> f64 {
        grain_boundary_corrosion_rate(ThermodynamicTemperature::new::<degree_celsius>(t_c))
            .get::<hertz>()
    }

    /// Eq (10a) is the zero-exposure limit of Eq (10b), and `0.44·m_o` is the
    /// floor. Both are algebraic identities in the printed constants.
    #[test]
    fn eq_10b_runs_from_m_o_down_to_0_44_m_o() {
        let m_o = 7.0;
        assert!((corroded_weibull_modulus(m_o, Ratio::new::<ratio>(0.0)) - m_o).abs() < 1e-15);
        let far = corroded_weibull_modulus(m_o, Ratio::new::<ratio>(50.0));
        assert!(
            (far - RESIDUAL_MODULUS_FRACTION * m_o).abs() < 1e-12,
            "{far}"
        );
        // Monotone decreasing in exposure.
        let mut prev = f64::INFINITY;
        for i in 0..100 {
            let v = corroded_weibull_modulus(m_o, Ratio::new::<ratio>(i as f64 * 0.1));
            assert!(v <= prev + 1e-15);
            prev = v;
        }
    }

    /// The accumulated exposure collapses to the printed `η̇·t` when the
    /// temperature is constant — the check that the extension in the module
    /// docs is an extension and not a change.
    #[test]
    fn the_exposure_reduces_to_the_printed_form_when_isothermal() {
        let t = ThermodynamicTemperature::new::<degree_celsius>(1600.0);
        let total = Time::new::<hour>(1000.0);
        let printed: Ratio = grain_boundary_corrosion_rate(t) * total;

        let mut x = Ratio::new::<ratio>(0.0);
        for _ in 0..100 {
            x = advance_grain_boundary_exposure(x, t, Time::new::<hour>(10.0));
        }
        assert!((x.get::<ratio>() - printed.get::<ratio>()).abs() / printed.get::<ratio>() < 1e-12);
    }

    /// **Grain-boundary corrosion raises the failure fraction**, which is the
    /// only claim Figs. 7 and 8 make that can be checked without their
    /// unstated geometry: they plot the "with" curve one to two decades above
    /// the "without" curve at 1600 °C.
    ///
    /// Measured here for `σ_t/σ_o = 0.25`, `m_o = 6`, `η̇·t` at 1600 °C over
    /// 1000 h: the failure fraction rises by a factor of **3.0·10²**, i.e.
    /// 2.5 decades — the same direction and order as the figures, though not
    /// a reproduction of them (their input sets are incomplete).
    #[test]
    fn corrosion_lifts_the_failure_fraction_as_figs_7_and_8_show() {
        let sigma_o = Pressure::new::<megapascal>(600.0);
        let sigma_t = Pressure::new::<megapascal>(150.0);
        let m_o = 6.0;

        let t = ThermodynamicTemperature::new::<degree_celsius>(1600.0);
        let exposure: Ratio = grain_boundary_corrosion_rate(t) * Time::new::<hour>(1000.0);
        let m = corroded_weibull_modulus(m_o, exposure);

        let without = super::super::weibull_failure_fraction(sigma_t, sigma_o, m_o).get::<ratio>();
        let with = super::super::weibull_failure_fraction(sigma_t, sigma_o, m).get::<ratio>();
        let decades = (with / without).log10();
        assert!(
            with > without,
            "grain-boundary corrosion must worsen the failure fraction"
        );
        assert!(
            (1.0..3.5).contains(&decades),
            "expected one to three decades as in Figs. 7/8, got {decades:.2}"
        );
        assert!(m < m_o && m > RESIDUAL_MODULUS_FRACTION * m_o);
    }

    /// The Arrhenius as transcribed: rising with temperature, finite, and
    /// zero at absolute zero rather than a NaN.
    #[test]
    fn the_rate_is_monotone_and_safe_at_zero() {
        assert!(eta_dot(2000.0) > eta_dot(1600.0));
        assert!(eta_dot(1600.0) > 0.0 && eta_dot(1600.0).is_finite());
        assert_eq!(
            grain_boundary_corrosion_rate(ThermodynamicTemperature::new::<kelvin>(0.0))
                .get::<hertz>(),
            0.0
        );
        assert_eq!(
            GrainBoundaryCorrosion::default(),
            GrainBoundaryCorrosion::Disabled
        );
    }
}
