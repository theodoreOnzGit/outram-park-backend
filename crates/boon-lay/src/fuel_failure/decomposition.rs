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

//! **SiC thermal decomposition** — `φ₂`. The Arrhenius "decay constant"
//! (unnumbered, page -495-, Benz 1982), the action integral `ζ` and its
//! discrete form Eq (11), the rate constant Eq (12), the failure law Eq (13)
//! and its two calibrations Eqs (14a)/(14b) (pages -496-, -497-).
//!
//! ```text
//! k        = k_o·exp(−Q/(R·T)),   Q = 556 kJ/mol                 (p-495)
//! ζ        = ∫ k(T) dt                                            (p-496)
//! ζ(t₂)    = ζ(t₁) + k(T_m)·(t₂ − t₁)                             (11)
//! k(T_m)   = (375/d_o)·exp(−556000/(R·T_m))   [s⁻¹]               (12)
//! φ₂(t,T)  = 1 − exp(−α·ζ^β)                                      (13)
//! α = 0.693 (= ln 2), β = 0.88   loose particles                  (14a)
//! α = 0.0001,         β = 4      particles in a sphere            (14b)
//! ```
//!
//! Above roughly 2000 °C this is the dominant failure mechanism, and it is
//! the one `φ₁` cannot see: SiC decomposes to gaseous Si and solid graphite,
//! so the layer stops being a pressure vessel rather than bursting as one.
//!
//! # `ζ` carries the history; `φ₂` does not accumulate
//!
//! This is the structural difference from `φ₁`, and the report is explicit
//! about it on page -483-: `ζ` increases monotonically step by step and
//! `φ₂(t₂)` is then read **directly** off Eq (13) at that `ζ`. Only the
//! *rate* `φ̇₂ = Δφ₂/Δt` is formed by differencing. `φ₁`, by contrast, is
//! accumulated from positive increments (page -482-).
//!
//! Accumulating `φ₂` by increments instead would give the same answer for a
//! monotone temperature history and a different one for any history that
//! cools, which is exactly the case a reactor transient is. See
//! [`super::history`], where the two are stepped side by side.
//!
//! # The units of the 375
//!
//! Eq (12) prints `375/d_o` with `d_o` in metres and declares the result
//! `[s⁻¹]`. For that to hold, **375 must carry units of m/s**: it is the
//! `k_o` of the page -495- Arrhenius made concrete, a decomposition front
//! velocity divided by the layer it has to eat through. The report never says
//! so; it is the only reading that balances, and it is why
//! [`decomposition_rate_constant`] takes a `uom` [`Length`] rather than a
//! bare number. Recorded in `docs/panama-i-units-and-open-questions.md`.
//!
//! A consequence worth stating: `k ∝ 1/d_o`, so a 50 µm layer decomposes
//! 30 % more slowly than a 35 µm one at the same temperature, and `ζ` scales
//! with it directly.
//!
//! # Verification status — no figure in the report checks this group
//!
//! **Unlike Eqs (7), (8a), (9a), (5a)–(5f), `D_S` and `f(τ)`, nothing here is
//! verified against a plotted or tabulated output of the report.** Figs. 10–13's
//! reactor cases do not state `d_o`, and the Benz 1982 weight-loss data that
//! fixed `Q` is in the missing reference list (page -510-).
//!
//! Fig. 6 does, however, **exclude one of the two calibrations**. At 1600 °C
//! with `d_o = 35 µm`, `ζ(300 h) = 3.6·10⁻³`, which under Eq (14a) gives
//! `φ₂ = 4.9·10⁻³` — a floor that every one of Fig. 6's eight curves would sit
//! on, where the figure in fact runs from 2·10⁻⁶ to about 5·10⁻³ and spreads
//! across three decades. Under Eq (14b) the same `ζ` gives `φ₂ = 1.7·10⁻¹⁴`,
//! invisible. So Fig. 6 was computed with the **sphere** calibration (14b), or
//! with `φ₂` switched off; it cannot have used (14a). Pinned by
//! [`tests::figure_6_excludes_the_loose_particle_calibration`].
//!
//! What *is* checked here is internal and algebraic, and it is stated as such:
//!
//! | check | result |
//! |---|---|
//! | Eq (14a)'s `α = ln 2` makes `ζ = 1` the median | exact, [`tests::alpha_ln2_makes_unit_action_the_median`] |
//! | Eq (11) telescopes at constant `T` | 1·10⁻¹² over 30 steps |
//! | `Q = 556 kJ/mol` reproduces Benz's stated 1600–2200 °C range as ~4 decades in `k` | 3.76 decades |
//! | `k ∝ 1/d_o` | exact |
//!
//! An order-of-magnitude comparison against Fig. 9 is recorded in the units
//! doc; it is **not** a verification, because Fig. 9's caption states neither
//! the burnup nor the irradiation history.

use uom::si::f64::{Frequency, Length, Ratio, ThermodynamicTemperature, Time};
use uom::si::frequency::hertz;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

/// The activation energy `Q` of SiC decomposition, J/mol (page -495-,
/// Benz 1982, 63 specimens decomposed between 1600 °C and 2200 °C).
pub const DECOMPOSITION_ACTIVATION_J_PER_MOL: f64 = 556_000.0;

/// Eq (12)'s numerator, \[m/s\] — the `k_o` of the page -495- Arrhenius
/// expressed as a front velocity, so that `k_o = 375/d_o` comes out in s⁻¹.
///
/// See the module docs: the unit is not printed in the report and this is the
/// only reading that balances.
pub const DECOMPOSITION_FRONT_VELOCITY_M_PER_S: f64 = 375.0;

/// **Eq (12)** — the decomposition rate constant `k(T_m)` \[s⁻¹\]
/// (page -496-).
///
/// ```text
/// k(T_m) = (375/d_o)·exp(−556000/(R·T_m))
/// ```
///
/// - `initial_thickness` — `d_o`. The **initial** thickness, not `d_act`:
///   Eq (12) is written against `d_o` and the report does not couple
///   decomposition to the volume-corrosion thinning of Eq (7).
/// - `mean_temperature` — `T_m`, the step's mean temperature, in kelvin via
///   `uom`.
///
/// Returns zero at non-positive temperature or thickness rather than a NaN or
/// an infinity — nothing decomposes at absolute zero, and a vanished layer
/// has no rate left to define.
pub fn decomposition_rate_constant(
    initial_thickness: Length,
    mean_temperature: ThermodynamicTemperature,
) -> Frequency {
    let t = mean_temperature.get::<kelvin>();
    let d_o = initial_thickness.get::<uom::si::length::meter>();
    if t <= 0.0 || d_o <= 0.0 {
        return Frequency::new::<hertz>(0.0);
    }
    let r = super::pressure::GAS_CONSTANT_J_PER_MOL_K;
    let k = DECOMPOSITION_FRONT_VELOCITY_M_PER_S / d_o
        * (-DECOMPOSITION_ACTIVATION_J_PER_MOL / (r * t)).exp();
    Frequency::new::<hertz>(k)
}

/// **Eq (11)** — advance the action integral `ζ` across one time step
/// (page -496-):
///
/// ```text
/// ζ(t₂) = ζ(t₁) + k(T_m)·(t₂ − t₁)
/// ```
///
/// `ζ` is dimensionless (`s⁻¹ × s`) and starts at zero. It is carried
/// forward rather than recomputed from the total elapsed time for the same
/// reason `FKOR` is in [`super::corrosion`]: each step must contribute at its
/// own mean temperature, and `k` spans decades over a transient.
pub fn advance_action_integral(
    previous: Ratio,
    initial_thickness: Length,
    mean_temperature: ThermodynamicTemperature,
    step: Time,
) -> Ratio {
    let advance: Ratio = decomposition_rate_constant(initial_thickness, mean_temperature) * step;
    previous + advance
}

/// Which of the report's two empirical fits of Eq (13) to use.
///
/// The two are not small perturbations of one another — `β` is 0.88 against
/// 4 — so they disagree by orders of magnitude away from `ζ ≈ 1`. Which one
/// applies is a property of the *experiment* being modelled (a loose particle
/// in a furnace, or one embedded in a fuel sphere), not a fitting knob.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecompositionCalibration {
    /// **Eq (14a)** — loose particles. Ramp tests to 2500 °C on loose
    /// `UO₂`-TRISO irradiated in DR-S6 (Goodin et al. 1985).
    /// `α = 0.693 (= ln 2)`, `β = 0.88`.
    LooseParticles,
    /// **Eq (14b)** — particles embedded in a fuel sphere (Schenk 1984, AVR
    /// GO2). `α = 0.0001`, `β = 4`. The report states the result is the same
    /// for irradiated and unirradiated elements.
    ParticlesInSphere,
}

impl DecompositionCalibration {
    /// `α` of Eq (13).
    pub const fn alpha(self) -> f64 {
        match self {
            DecompositionCalibration::LooseParticles => std::f64::consts::LN_2,
            DecompositionCalibration::ParticlesInSphere => 0.0001,
        }
    }

    /// `β` of Eq (13).
    pub const fn beta(self) -> f64 {
        match self {
            DecompositionCalibration::LooseParticles => 0.88,
            DecompositionCalibration::ParticlesInSphere => 4.0,
        }
    }
}

/// **Eq (13)** — the failed fraction from thermal decomposition (page -496-).
///
/// ```text
/// φ₂(t,T) = 1 − exp(−α·ζ^β)
/// ```
///
/// Evaluated **directly** from the running `ζ`, never accumulated from
/// increments: `ζ` already carries the whole temperature–time history
/// (page -483-).
///
/// The form is chosen so that `φ₂ ≤ 1` for any `ζ`. Returns zero for
/// `ζ ≤ 0`, where `ζ^β` is not defined for the fractional `β` of Eq (14a).
pub fn thermal_decomposition_failure_fraction(
    action_integral: Ratio,
    calibration: DecompositionCalibration,
) -> Ratio {
    thermal_decomposition_failure_fraction_with(
        action_integral,
        calibration.alpha(),
        calibration.beta(),
    )
}

/// [`thermal_decomposition_failure_fraction`] with an explicit `α` and `β`.
///
/// The report states these "must be empirically determined" and gives two
/// fits; a third measurement would come in here rather than by editing the
/// enum.
pub fn thermal_decomposition_failure_fraction_with(
    action_integral: Ratio,
    alpha: f64,
    beta: f64,
) -> Ratio {
    let zeta = action_integral.get::<ratio>();
    if zeta <= 0.0 {
        return Ratio::new::<ratio>(0.0);
    }
    Ratio::new::<ratio>(1.0 - (-alpha * zeta.powf(beta)).exp())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::length::micrometer;
    use uom::si::thermodynamic_temperature::degree_celsius;
    use uom::si::time::hour;

    fn d_o() -> Length {
        Length::new::<micrometer>(35.0)
    }

    fn k_at(t_c: f64) -> f64 {
        decomposition_rate_constant(d_o(), ThermodynamicTemperature::new::<degree_celsius>(t_c))
            .get::<hertz>()
    }

    fn zeta_isothermal(t_c: f64, hours: f64) -> Ratio {
        advance_action_integral(
            Ratio::new::<ratio>(0.0),
            d_o(),
            ThermodynamicTemperature::new::<degree_celsius>(t_c),
            Time::new::<hour>(hours),
        )
    }

    /// **Eq (14a)'s `α = ln 2` makes `ζ = 1` the median action integral** —
    /// exactly the normalisation Eq (1) uses for `σ_o`, and the reason the
    /// report bothers to print "(= ln 2)" beside the 0.693.
    ///
    /// This is an algebraic identity in the report's own constants, so it is
    /// exact rather than a tolerance: at `ζ = 1`, `ζ^β = 1` for any `β`, so
    /// `φ₂ = 1 − e^(−ln2) = 0.5`. A transcription of `α` as 0.69 instead of
    /// `LN_2` would miss 0.5 by 1.5·10⁻³ and nothing else would notice.
    #[test]
    fn alpha_ln2_makes_unit_action_the_median() {
        let phi = thermal_decomposition_failure_fraction(
            Ratio::new::<ratio>(1.0),
            DecompositionCalibration::LooseParticles,
        );
        assert!((phi.get::<ratio>() - 0.5).abs() < 1e-15, "{phi:?}");

        // The sphere calibration reaches its median much later:
        // zeta = (ln2/1e-4)^(1/4) = 9.1247...
        let zeta_half = (std::f64::consts::LN_2 / 0.0001).powf(0.25);
        let phi = thermal_decomposition_failure_fraction(
            Ratio::new::<ratio>(zeta_half),
            DecompositionCalibration::ParticlesInSphere,
        );
        assert!(
            (phi.get::<ratio>() - 0.5).abs() < 1e-12,
            "{phi:?} at {zeta_half}"
        );
        assert!((zeta_half - 9.1247).abs() < 1e-3);
    }

    /// **Eq (11) telescopes at constant temperature**, which is what the
    /// report's page -482- claim that "at a constant temperature, the length
    /// of the time interval does not influence the computed result" requires
    /// of `ζ`.
    ///
    /// Measured 2026-09-24: 30 ten-hour steps against one 300-hour step agree
    /// to better than 1·10⁻¹² relative.
    #[test]
    fn the_action_integral_telescopes_at_constant_temperature() {
        let t = ThermodynamicTemperature::new::<degree_celsius>(2200.0);
        let one = zeta_isothermal(2200.0, 300.0).get::<ratio>();
        let mut z = Ratio::new::<ratio>(0.0);
        for _ in 0..30 {
            z = advance_action_integral(z, d_o(), t, Time::new::<hour>(10.0));
        }
        assert!(
            (z.get::<ratio>() - one).abs() / one < 1e-12,
            "{} vs {one}",
            z.get::<ratio>()
        );
    }

    /// A varying history must accumulate at each step's own temperature: the
    /// hot leg dominates `ζ` by decades, which is the whole point of Eq (11).
    #[test]
    fn a_varying_history_is_dominated_by_its_hot_leg() {
        let hot = ThermodynamicTemperature::new::<degree_celsius>(2200.0);
        let cold = ThermodynamicTemperature::new::<degree_celsius>(1600.0);
        let step = Time::new::<hour>(50.0);
        let mut z = Ratio::new::<ratio>(0.0);
        z = advance_action_integral(z, d_o(), hot, step);
        let after_hot = z.get::<ratio>();
        z = advance_action_integral(z, d_o(), cold, step);
        // k(1600 degC)/k(2200 degC) = 10^-3.76 = 1.7e-4, so an equal-length
        // 1600 degC leg adds that fraction and no more.
        let added = (z.get::<ratio>() - after_hot) / after_hot;
        assert!(
            (1.0e-4..3.0e-4).contains(&added),
            "the 1600 degC leg should add ~1.7e-4 of the 2200 degC leg, got {added:e}"
        );
    }

    /// `Q = 556 kJ/mol` over Benz's own 1600–2200 °C measurement range spans
    /// close to four decades in `k`. A transcription of `Q` as 55.6 or
    /// 5560 kJ/mol would give 0.4 or 39 decades instead, so this is a real
    /// (if coarse) check on the constant rather than a tautology.
    ///
    /// Measured 2026-09-24: **3.76 decades** between the endpoints of the
    /// stated range.
    #[test]
    fn the_activation_energy_spans_benzs_measurement_range() {
        let decades = (k_at(2200.0) / k_at(1600.0)).log10();
        assert!(
            (decades - 3.76).abs() < 0.02,
            "1600->2200 degC should span ~3.9 decades in k, got {decades:.3}"
        );
    }

    /// `k ∝ 1/d_o` — Eq (12)'s thickness dependence, and the reason a 50 µm
    /// layer decomposes 30 % more slowly than a 35 µm one.
    #[test]
    fn the_rate_constant_scales_inversely_with_thickness() {
        let t = ThermodynamicTemperature::new::<degree_celsius>(2000.0);
        let thin = decomposition_rate_constant(Length::new::<micrometer>(35.0), t).get::<hertz>();
        let thick = decomposition_rate_constant(Length::new::<micrometer>(50.0), t).get::<hertz>();
        assert!((thin / thick - 50.0 / 35.0).abs() < 1e-12);
        assert_eq!(
            decomposition_rate_constant(Length::new::<micrometer>(0.0), t).get::<hertz>(),
            0.0
        );
        assert_eq!(
            decomposition_rate_constant(d_o(), ThermodynamicTemperature::new::<kelvin>(0.0))
                .get::<hertz>(),
            0.0
        );
    }

    /// **`φ₂` under the sphere calibration is invisible at 1600 °C** — which
    /// is why Fig. 6 cannot verify it, and why that gap is stated rather than
    /// papered over.
    ///
    /// Measured 2026-09-24, `d_o = 35 µm`, 300 h isothermal:
    ///
    /// | T | `ζ` | `φ₂` (14b) | `φ₂` (14a) |
    /// |---|---|---|---|
    /// | 1600 °C | 3.6·10⁻³ | 1.7·10⁻¹⁴ | 4.9·10⁻³ |
    /// | 2000 °C | 1.94 | 1.4·10⁻³ | 0.711 |
    #[test]
    fn phi_2_is_negligible_at_1600_degc() {
        let z16 = zeta_isothermal(1600.0, 300.0);
        let phi16 = thermal_decomposition_failure_fraction(
            z16,
            DecompositionCalibration::ParticlesInSphere,
        );
        assert!(
            phi16.get::<ratio>() < 1e-13,
            "phi_2 at 1600 degC/300 h should be invisible on Fig. 6's axis, got {phi16:?}"
        );

        let z20 = zeta_isothermal(2000.0, 300.0);
        let phi20 = thermal_decomposition_failure_fraction(
            z20,
            DecompositionCalibration::ParticlesInSphere,
        );
        assert!(
            (1.0e-3..1.0e-2).contains(&phi20.get::<ratio>()),
            "phi_2 at 2000 degC/300 h should be of order 1e-3, got {phi20:?}"
        );
        // The loose-particle calibration is far more severe at the same zeta.
        let loose =
            thermal_decomposition_failure_fraction(z20, DecompositionCalibration::LooseParticles);
        assert!(loose.get::<ratio>() > 100.0 * phi20.get::<ratio>());
    }

    /// **Fig. 6 excludes Eq (14a).**
    ///
    /// Methodology: Fig. 6 (page -500-) plots eight SiC varieties at
    /// 1600 degC out to 300 h, on a log axis. `φ₂` is variety-independent, so
    /// whatever calibration the report used puts the *same* floor under all
    /// eight curves. The digitised figure runs from 2·10⁻⁶ to about 5·10⁻³
    /// and spreads across three decades, so any floor above ~10⁻⁶ is
    /// excluded.
    ///
    /// Result, 2026-09-24: at `ζ(300 h) = 3.6·10⁻³`, Eq (14a) gives
    /// `φ₂ = 4.9·10⁻³` — above the *top* of the figure — while Eq (14b) gives
    /// `1.7·10⁻¹⁴`. Fig. 6 therefore used the sphere calibration, or `φ₂`
    /// switched off. It cannot have used the loose-particle one.
    ///
    /// This is a negative result and it is worth as much as a positive one:
    /// it is the only constraint any figure in the report puts on `φ₂`.
    #[test]
    fn figure_6_excludes_the_loose_particle_calibration() {
        let z = zeta_isothermal(1600.0, 300.0);
        assert!(
            (z.get::<ratio>() - 3.62e-3).abs() < 1e-5,
            "zeta at 1600 degC/300 h: {z:?}"
        );
        let loose =
            thermal_decomposition_failure_fraction(z, DecompositionCalibration::LooseParticles)
                .get::<ratio>();
        assert!(
            loose > 5.0e-3 * 0.9,
            "Eq (14a) should sit at ~4.9e-3, above the whole of Fig. 6, got {loose:e}"
        );
        let sphere =
            thermal_decomposition_failure_fraction(z, DecompositionCalibration::ParticlesInSphere)
                .get::<ratio>();
        assert!(
            sphere < 1.0e-6 * 1.0e-6,
            "Eq (14b) should be invisible on Fig. 6's axis, got {sphere:e}"
        );
    }

    /// Eq (13) stays a fraction, is monotone in `ζ`, and is zero at `ζ = 0`
    /// for both calibrations — including the fractional `β` of Eq (14a),
    /// where `0^0.88` would otherwise be the interesting case.
    #[test]
    fn eq_13_is_a_monotone_fraction() {
        for cal in [
            DecompositionCalibration::LooseParticles,
            DecompositionCalibration::ParticlesInSphere,
        ] {
            assert_eq!(
                thermal_decomposition_failure_fraction(Ratio::new::<ratio>(0.0), cal)
                    .get::<ratio>(),
                0.0
            );
            let mut prev = -1.0;
            for i in 0..200 {
                let z = i as f64 * 0.25;
                let v = thermal_decomposition_failure_fraction(Ratio::new::<ratio>(z), cal)
                    .get::<ratio>();
                assert!((0.0..=1.0).contains(&v), "{cal:?} at zeta={z}: {v}");
                assert!(v >= prev, "{cal:?} must be monotone in zeta");
                prev = v;
            }
            assert!(prev > 0.999, "{cal:?} must saturate");
        }
    }
}
