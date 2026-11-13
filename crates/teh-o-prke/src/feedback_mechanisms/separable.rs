//! # RELAP5-style separable reactivity feedback (gh:#1083)
//!
//! RELAP-7 Theory Manual (INL/EXT-14-31366), §7.5, Eq (443), which RELAP-7
//! states is RELAP5's separable model. Each effect is independent of the
//! others:
//!
//! ```text
//! r(t) = sum_{i=1..n_s} r_si(t)
//!      + sum_{i=1..n_rho} [ W_rho,i R_rho(rho_i(t)) + a_M,i dT_M,i(t) ]
//!      + sum_{i=1..n_F}   [ W_F,i   R_F(T_F,i(t))  + a_F,i dT_F,i(t) ]
//! ```
//!
//! - `r_si(t)`: scram (or any externally scheduled) reactivity curves, tables
//!   of reactivity against time ([`ScramReactivityCurve`]);
//! - `R_rho`: one table of reactivity against moderator density, evaluated
//!   per thermal-fluid volume `i` at that volume's density and weighted by
//!   `W_rho,i` ([`DensityReactivityTable`], [`ModeratorVolume`]);
//! - `a_M,i dT_M,i`: linear moderator temperature term (temperature change
//!   from the start of the transient, density effects excluded);
//! - `R_F`: one table of Doppler reactivity against fuel temperature,
//!   evaluated per heat structure and weighted by `W_F,i`
//!   ([`FuelTemperatureReactivityTable`], [`FuelHeatStructure`]);
//! - `a_F,i dT_F,i`: linear fuel temperature term.
//!
//! **Units.** RELAP-7 writes Eq 443 for `r = rho / beta` (dollars, Eq 430).
//! The model is linear in its tables and coefficients, so this module works in
//! reactivity `Delta k / k` throughout; [`SeparableReactivity::total_in_dollars`]
//! converts. Tables entered in dollars must be multiplied by `beta` first.
//!
//! **Tables** interpolate linearly and clamp to the end values outside their
//! range, the same convention as GeN-Foam's `TimeProfile`
//! (`outram-foam-appbuilder-lib::genfoam::common::time_profile`, itself
//! OpenFOAM's `Function1` table with `outOfBounds clamp`). That type is not
//! reused because teh-o-prke does not depend on the appbuilder crate; the
//! small table here follows its rules (at least two points, strictly
//! increasing abscissae, start-time shift for scram curves).
//!
//! Boron feedback is not in Eq 443 (RELAP-7 notes it is absent) and is not
//! here. The thermal-hydraulic side (which densities and temperatures, and
//! the tampines interface) belongs to the coupling crate.
//!
//! > Research, education and V&V only. AI draft, unreviewed.

use uom::si::f64::*;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::ratio::ratio;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::ConstZero;

use crate::teh_o_prke_error::TehOPrkeError;

/// Piecewise-linear table with end clamping (private core of the typed
/// tables below).
#[derive(Clone, Debug, PartialEq)]
struct LinearTable {
    x: Vec<f64>,
    y: Vec<f64>,
}

impl LinearTable {
    fn new(x: Vec<f64>, y: Vec<f64>) -> Result<Self, TehOPrkeError> {
        if x.len() != y.len() || x.len() < 2 {
            return Err(TehOPrkeError::GenericStringError(format!(
                "reactivity table needs at least two (x, y) points of equal count; got {} and {}",
                x.len(),
                y.len()
            )));
        }
        if x.iter().chain(y.iter()).any(|v| !v.is_finite()) {
            return Err(TehOPrkeError::GenericStringError(
                "reactivity table values must be finite".to_string(),
            ));
        }
        if x.windows(2).any(|w| !(w[1] > w[0])) {
            return Err(TehOPrkeError::GenericStringError(
                "reactivity table abscissae must be strictly increasing".to_string(),
            ));
        }
        Ok(Self { x, y })
    }

    fn eval(&self, xq: f64) -> f64 {
        let n = self.x.len();
        if xq <= self.x[0] {
            return self.y[0];
        }
        if xq >= self.x[n - 1] {
            return self.y[n - 1];
        }
        // first index with x[k] > xq; then x[k-1] <= xq < x[k]
        let k = self.x.partition_point(|v| *v <= xq);
        let (x0, x1, y0, y1) = (self.x[k - 1], self.x[k], self.y[k - 1], self.y[k]);
        y0 + (y1 - y0) * (xq - x0) / (x1 - x0)
    }
}

/// A scram (or scheduled) reactivity curve `r_s(t)`.
#[derive(Clone, Debug, PartialEq)]
pub struct ScramReactivityCurve {
    table: LinearTable,
    start_time: Time,
}

impl ScramReactivityCurve {
    /// Reactivity against time since the curve starts. Linear between
    /// points, clamped outside. Errors on fewer than two points, unequal
    /// lengths, or non-increasing times.
    pub fn new(times: Vec<Time>, reactivities: Vec<Ratio>) -> Result<Self, TehOPrkeError> {
        Ok(Self {
            table: LinearTable::new(
                times.iter().map(|t| t.get::<second>()).collect(),
                reactivities.iter().map(|r| r.get::<ratio>()).collect(),
            )?,
            start_time: Time::ZERO,
        })
    }

    /// Shift the curve to start at simulation time `start` (e.g. the scram
    /// trip time). Before it, the first tabulated value holds.
    pub fn with_start_time(mut self, start: Time) -> Self {
        self.start_time = start;
        self
    }

    /// `r_s(t)` at simulation time `t`.
    pub fn reactivity(&self, time: Time) -> Ratio {
        Ratio::new::<ratio>(self.table.eval((time - self.start_time).get::<second>()))
    }
}

/// `R_rho(rho)`: reactivity against moderator density.
#[derive(Clone, Debug, PartialEq)]
pub struct DensityReactivityTable {
    table: LinearTable,
}

impl DensityReactivityTable {
    /// Linear between points, clamped outside; same validation as
    /// [`ScramReactivityCurve::new`].
    pub fn new(
        densities: Vec<MassDensity>,
        reactivities: Vec<Ratio>,
    ) -> Result<Self, TehOPrkeError> {
        Ok(Self {
            table: LinearTable::new(
                densities
                    .iter()
                    .map(|d| d.get::<kilogram_per_cubic_meter>())
                    .collect(),
                reactivities.iter().map(|r| r.get::<ratio>()).collect(),
            )?,
        })
    }

    /// `R_rho` at `density`.
    pub fn reactivity(&self, density: MassDensity) -> Ratio {
        Ratio::new::<ratio>(self.table.eval(density.get::<kilogram_per_cubic_meter>()))
    }
}

/// `R_F(T_F)`: Doppler reactivity against fuel temperature.
#[derive(Clone, Debug, PartialEq)]
pub struct FuelTemperatureReactivityTable {
    table: LinearTable,
}

impl FuelTemperatureReactivityTable {
    /// Linear between points, clamped outside; same validation as
    /// [`ScramReactivityCurve::new`].
    pub fn new(
        temperatures: Vec<ThermodynamicTemperature>,
        reactivities: Vec<Ratio>,
    ) -> Result<Self, TehOPrkeError> {
        Ok(Self {
            table: LinearTable::new(
                temperatures.iter().map(|t| t.get::<kelvin>()).collect(),
                reactivities.iter().map(|r| r.get::<ratio>()).collect(),
            )?,
        })
    }

    /// `R_F` at `temperature`.
    pub fn reactivity(&self, temperature: ThermodynamicTemperature) -> Ratio {
        Ratio::new::<ratio>(self.table.eval(temperature.get::<kelvin>()))
    }
}

/// One thermal-fluid volume of the core: `W_rho,i`, `a_M,i` and the
/// moderator temperature at the start of the transient.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModeratorVolume {
    /// `W_rho,i`, density weighting factor.
    pub density_weight: Ratio,
    /// `a_M,i`, fluid temperature coefficient (density effects excluded).
    pub temperature_coefficient: TemperatureCoefficient,
    /// Moderator temperature at the start of the transient.
    pub initial_temperature: ThermodynamicTemperature,
}

/// One fuel heat structure: `W_F,i`, `a_F,i` and the fuel temperature at
/// the start of the transient.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FuelHeatStructure {
    /// `W_F,i`, fuel temperature weighting factor.
    pub temperature_weight: Ratio,
    /// `a_F,i`, fuel temperature coefficient.
    pub temperature_coefficient: TemperatureCoefficient,
    /// Fuel temperature at the start of the transient.
    pub initial_temperature: ThermodynamicTemperature,
}

/// The current state of one thermal-fluid volume.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModeratorState {
    /// `rho_i(t)`
    pub density: MassDensity,
    /// spatially averaged moderator temperature
    pub temperature: ThermodynamicTemperature,
}

/// Eq 443 broken into its five sums, so each can be checked or plotted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeparableReactivity {
    /// `sum r_si`
    pub scram: Ratio,
    /// `sum W_rho,i R_rho(rho_i)`
    pub moderator_density: Ratio,
    /// `sum a_M,i dT_M,i`
    pub moderator_temperature: Ratio,
    /// `sum W_F,i R_F(T_F,i)`
    pub fuel_doppler_table: Ratio,
    /// `sum a_F,i dT_F,i`
    pub fuel_temperature: Ratio,
}

impl SeparableReactivity {
    /// The total reactivity, Eq 443.
    pub fn total(&self) -> Ratio {
        self.scram
            + self.moderator_density
            + self.moderator_temperature
            + self.fuel_doppler_table
            + self.fuel_temperature
    }

    /// The total in dollars, `r = rho / beta` (RELAP-7 Eq 430).
    pub fn total_in_dollars(&self, beta: Ratio) -> Ratio {
        self.total() / beta.get::<ratio>()
    }
}

/// RELAP5's separable feedback model (RELAP-7 Eq 443).
///
/// A missing table contributes zero, as does an empty list of curves,
/// volumes or structures.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SeparableFeedback {
    /// The `n_s` scram curves.
    pub scram_curves: Vec<ScramReactivityCurve>,
    /// `R_rho`, shared by every volume.
    pub density_table: Option<DensityReactivityTable>,
    /// `R_F`, shared by every heat structure.
    pub fuel_temperature_table: Option<FuelTemperatureReactivityTable>,
    /// The `n_rho` thermal-fluid volumes.
    pub moderator_volumes: Vec<ModeratorVolume>,
    /// The `n_F` fuel heat structures.
    pub fuel_structures: Vec<FuelHeatStructure>,
}

impl SeparableFeedback {
    /// Evaluate every term of Eq 443 at simulation time `time`, for the
    /// current state of each volume (`moderator`, same order and length as
    /// `moderator_volumes`) and each heat structure's fuel temperature
    /// (`fuel_temperatures`, same order and length as `fuel_structures`).
    pub fn evaluate(
        &self,
        time: Time,
        moderator: &[ModeratorState],
        fuel_temperatures: &[ThermodynamicTemperature],
    ) -> Result<SeparableReactivity, TehOPrkeError> {
        if moderator.len() != self.moderator_volumes.len()
            || fuel_temperatures.len() != self.fuel_structures.len()
        {
            return Err(TehOPrkeError::ShapeMismatch(format!(
                "separable feedback: {} volumes but {} moderator states, \
                 {} heat structures but {} fuel temperatures",
                self.moderator_volumes.len(),
                moderator.len(),
                self.fuel_structures.len(),
                fuel_temperatures.len()
            )));
        }
        let scram = self
            .scram_curves
            .iter()
            .fold(Ratio::ZERO, |acc, c| acc + c.reactivity(time));

        let mut moderator_density = Ratio::ZERO;
        let mut moderator_temperature = Ratio::ZERO;
        for (vol, st) in self.moderator_volumes.iter().zip(moderator) {
            if let Some(table) = &self.density_table {
                moderator_density += vol.density_weight * table.reactivity(st.density);
            }
            moderator_temperature += linear_term(
                vol.temperature_coefficient,
                st.temperature,
                vol.initial_temperature,
            );
        }

        let mut fuel_doppler_table = Ratio::ZERO;
        let mut fuel_temperature = Ratio::ZERO;
        for (hs, t) in self.fuel_structures.iter().zip(fuel_temperatures) {
            if let Some(table) = &self.fuel_temperature_table {
                fuel_doppler_table += hs.temperature_weight * table.reactivity(*t);
            }
            fuel_temperature += linear_term(hs.temperature_coefficient, *t, hs.initial_temperature);
        }

        Ok(SeparableReactivity {
            scram,
            moderator_density,
            moderator_temperature,
            fuel_doppler_table,
            fuel_temperature,
        })
    }
}

/// `a (T - T_0)` in the same arithmetic as
/// `NordheimFuchsExactTimestepper::reactivity_margin`'s feedback term.
fn linear_term(
    coefficient: TemperatureCoefficient,
    temperature: ThermodynamicTemperature,
    initial: ThermodynamicTemperature,
) -> Ratio {
    let delta_t_k = temperature.get::<kelvin>() - initial.get::<kelvin>();
    Ratio::new::<ratio>(coefficient.get::<per_kelvin>() * delta_t_k)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fuel_temperature_feedback::{
        obtain_fuel_temperature_feedback_coeff_thermal_spectrum,
        obtain_fuel_temperature_reactivity_feedback_thermal_spectrum,
    };

    fn r(x: f64) -> Ratio {
        Ratio::new::<ratio>(x)
    }
    fn k(x: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(x)
    }
    fn s(x: f64) -> Time {
        Time::new::<second>(x)
    }
    fn d(x: f64) -> MassDensity {
        MassDensity::new::<kilogram_per_cubic_meter>(x)
    }
    fn pk(x: f64) -> TemperatureCoefficient {
        TemperatureCoefficient::new::<per_kelvin>(x)
    }

    /// Table validation: fewer than two points, unequal lengths, non-
    /// increasing or non-finite abscissae are refused.
    #[test]
    fn tables_validate_inputs() {
        assert!(ScramReactivityCurve::new(vec![s(0.0)], vec![r(0.0)]).is_err());
        assert!(ScramReactivityCurve::new(vec![s(0.0), s(1.0)], vec![r(0.0)]).is_err());
        assert!(ScramReactivityCurve::new(vec![s(1.0), s(1.0)], vec![r(0.0), r(1.0)]).is_err());
        assert!(
            DensityReactivityTable::new(vec![d(f64::NAN), d(1.0)], vec![r(0.0), r(0.0)]).is_err()
        );
        assert!(FuelTemperatureReactivityTable::new(
            vec![k(300.0), k(600.0)],
            vec![r(0.0), r(-1e-3)]
        )
        .is_ok());
        let fb = SeparableFeedback::default();
        assert!(fb
            .evaluate(
                s(0.0),
                &[ModeratorState {
                    density: d(1.0),
                    temperature: k(300.0)
                }],
                &[]
            )
            .is_err());
    }

    /// **V&V: scram term by hand.** Curve (0 s, 0), (0.5 s, -0.01),
    /// (2 s, -0.05) starting at trip time 10 s; a second constant curve
    /// (0, -0.001), (1, -0.001). At t = 5 s (before trip) -> 0 - 0.001; at
    /// 10.25 s -> -0.005 - 0.001; at 11.25 s -> -0.01 + (-0.04)(0.75/1.5) =
    /// -0.03, total -0.031; at 20 s -> -0.051 (clamped). Prediction: exact
    /// to round-off (1e-17). *Result (2026-11-01):* passes.
    #[test]
    fn scram_term_hand_evaluation() {
        let fb = SeparableFeedback {
            scram_curves: vec![
                ScramReactivityCurve::new(
                    vec![s(0.0), s(0.5), s(2.0)],
                    vec![r(0.0), r(-0.01), r(-0.05)],
                )
                .unwrap()
                .with_start_time(s(10.0)),
                ScramReactivityCurve::new(vec![s(0.0), s(1.0)], vec![r(-0.001), r(-0.001)])
                    .unwrap(),
            ],
            ..Default::default()
        };
        for (t, want) in [
            (5.0, -0.001),
            (10.25, -0.006),
            (11.25, -0.031),
            (20.0, -0.051),
        ] {
            let got = fb.evaluate(s(t), &[], &[]).unwrap();
            assert!(
                (got.total().get::<ratio>() - want).abs() < 1e-17,
                "t={t}: {got:?}"
            );
            assert_eq!(got.moderator_density, Ratio::ZERO);
        }
    }

    /// **V&V: moderator density and temperature terms by hand.** R_rho:
    /// (600 kg/m³, -0.02), (700, -0.005), (750, 0). Two volumes, weights 0.6
    /// and 0.4, a_M = -2e-5 and -3e-5 /K, initial 560 K. State: 650 kg/m³ at
    /// 570 K, 760 kg/m³ at 555 K. Hand: R(650) = -0.0125, R(760) = 0
    /// (clamped); density sum 0.6(-0.0125) = -0.0075; temperature sum
    /// -2e-5 x 10 + -3e-5 x (-5) = -5e-5. Prediction: to 1e-17.
    /// *Result (2026-11-01):* passes.
    #[test]
    fn moderator_terms_hand_evaluation() {
        let fb = SeparableFeedback {
            density_table: Some(
                DensityReactivityTable::new(
                    vec![d(600.0), d(700.0), d(750.0)],
                    vec![r(-0.02), r(-0.005), r(0.0)],
                )
                .unwrap(),
            ),
            moderator_volumes: vec![
                ModeratorVolume {
                    density_weight: r(0.6),
                    temperature_coefficient: pk(-2e-5),
                    initial_temperature: k(560.0),
                },
                ModeratorVolume {
                    density_weight: r(0.4),
                    temperature_coefficient: pk(-3e-5),
                    initial_temperature: k(560.0),
                },
            ],
            ..Default::default()
        };
        let st = [
            ModeratorState {
                density: d(650.0),
                temperature: k(570.0),
            },
            ModeratorState {
                density: d(760.0),
                temperature: k(555.0),
            },
        ];
        let got = fb.evaluate(s(0.0), &st, &[]).unwrap();
        assert!(
            (got.moderator_density.get::<ratio>() + 0.0075).abs() < 1e-17,
            "{got:?}"
        );
        assert!(
            (got.moderator_temperature.get::<ratio>() + 5e-5).abs() < 1e-17,
            "{got:?}"
        );
        assert!((got.total().get::<ratio>() + 0.00755).abs() < 1e-17);
        assert!((got.total_in_dollars(r(0.0065)).get::<ratio>() + 0.00755 / 0.0065).abs() < 1e-14);
    }

    /// **V&V: tabulated Doppler R_F against the crate's existing sqrt law.**
    /// R_F tabulated at 300, 600, 900, 1200 K from
    /// `obtain_fuel_temperature_reactivity_feedback_thermal_spectrum`
    /// (alpha_coefficient 5e-4, T_ref 300 K). Prediction: at the nodes the
    /// separable term equals the law exactly (weight 1, one structure).
    /// Between nodes the table is a chord of `-2a sqrt(T)`, which is convex,
    /// so the chord lies above the law (smaller |rho|). Hand, at 750 K: law
    /// -2(5e-4)(sqrt 750 - sqrt 300) = -0.0100656, chord
    /// (-0.0071744 - 0.0126795)/2 = -0.0099269, difference 1.39e-4.
    /// Two structures with weights 0.5/0.5 at the same temperature give the
    /// single-structure value.
    /// *Result (2026-11-01):* nodes exact; midpoint chord -0.0099269
    /// (law -0.0100656), as predicted.
    #[test]
    fn fuel_table_matches_existing_sqrt_law_at_nodes() {
        let a = r(5.0e-4);
        let law = |t: f64| {
            obtain_fuel_temperature_reactivity_feedback_thermal_spectrum(a, k(t), k(300.0)).unwrap()
        };
        let nodes = [300.0, 600.0, 900.0, 1200.0];
        let table = FuelTemperatureReactivityTable::new(
            nodes.iter().map(|t| k(*t)).collect(),
            nodes.iter().map(|t| law(*t)).collect(),
        )
        .unwrap();
        let one = |w: f64| FuelHeatStructure {
            temperature_weight: r(w),
            temperature_coefficient: pk(0.0),
            initial_temperature: k(300.0),
        };
        let fb = SeparableFeedback {
            fuel_temperature_table: Some(table),
            fuel_structures: vec![one(1.0)],
            ..Default::default()
        };
        for t in nodes {
            let got = fb.evaluate(s(0.0), &[], &[k(t)]).unwrap();
            assert_eq!(got.fuel_doppler_table, law(t), "{t} K");
        }
        let mid = fb
            .evaluate(s(0.0), &[], &[k(750.0)])
            .unwrap()
            .fuel_doppler_table
            .get::<ratio>();
        let chord = 0.5 * (law(600.0).get::<ratio>() + law(900.0).get::<ratio>());
        assert!((mid - chord).abs() < 1e-17);
        assert!((mid + 0.0099269).abs() < 1e-7, "{mid}");
        assert!((law(750.0).get::<ratio>() + 0.0100656).abs() < 1e-7);
        let fb2 = SeparableFeedback {
            fuel_structures: vec![one(0.5), one(0.5)],
            ..fb.clone()
        };
        let two = fb2
            .evaluate(s(0.0), &[], &[k(750.0), k(750.0)])
            .unwrap()
            .fuel_doppler_table
            .get::<ratio>();
        assert!((two - mid).abs() < 1e-17);
    }

    /// **V&V: linear-only separable model reproduces the existing
    /// coefficient feedback exactly.**
    ///
    /// *Methodology.* Two existing linear coefficient forms in this crate:
    /// (a) `NordheimFuchsExactTimestepper`'s fuel feedback
    /// `alpha_f (T_f - T_ref)` (read through `reactivity_margin` with
    /// `rho_ext = beta = 0`), and (b) the coefficient
    /// `obtain_fuel_temperature_feedback_coeff_thermal_spectrum(a, T_ref)`
    /// times `dT`, by hand. One heat structure, no tables, `a_F` set to the
    /// same coefficient. (c) GeN-Foam's volume-weighted linear Doppler form
    /// `sum V_c (-alpha dT_c) / sum V_c` (ported by hand; no dependency)
    /// against three structures with `a_F,i = -alpha V_i / sum V`.
    ///
    /// *Prediction.* (a) and (b): bit-identical (same operations). (c): equal
    /// to round-off, not bitwise (different operation order), < 1e-17.
    /// *Result (2026-11-01):* (a) bitwise at all three temperatures. (b)
    /// bitwise at 451.7 K and 1234.5 K; **at dT = 0 the prediction missed by
    /// the sign of zero**: the hand product `coeff x 0.0` is `-0.0`
    /// (negative coefficient) while the model's sum starts from `+0.0` and
    /// returns `+0.0`. The values are equal; the assertion there compares
    /// values, not bits. (c) within 1e-17.
    #[test]
    fn linear_only_reproduces_existing_coefficient_feedback() {
        use crate::nordheim_fuchs::NordheimFuchsExactTimestepper;
        use uom::si::heat_capacity::joule_per_kelvin;
        use uom::si::power::watt;
        let alpha_f = pk(-2.5e-5);
        let mut nf = NordheimFuchsExactTimestepper::new(
            s(1e-4),
            r(0.0),
            HeatCapacity::new::<joule_per_kelvin>(1e5),
            alpha_f,
            k(300.0),
            k(300.0),
            Power::new::<watt>(1.0),
        )
        .unwrap();
        let fb = |a: TemperatureCoefficient| SeparableFeedback {
            fuel_structures: vec![FuelHeatStructure {
                temperature_weight: r(1.0),
                temperature_coefficient: a,
                initial_temperature: k(300.0),
            }],
            ..Default::default()
        };
        for t in [300.0, 451.7, 1234.5] {
            nf.fuel_temperature = k(t);
            let got = fb(alpha_f).evaluate(s(0.0), &[], &[k(t)]).unwrap().total();
            assert_eq!(
                got.get::<ratio>().to_bits(),
                nf.reactivity_margin().get::<ratio>().to_bits(),
                "(a) {t}"
            );

            let coeff =
                obtain_fuel_temperature_feedback_coeff_thermal_spectrum(r(5e-4), k(300.0)).unwrap();
            let a = pk(coeff.get::<ratio>());
            let got_b = fb(a)
                .evaluate(s(0.0), &[], &[k(t)])
                .unwrap()
                .total()
                .get::<ratio>();
            let hand_b = coeff.get::<ratio>() * (t - 300.0);
            // at dT = 0 the hand product is -0.0 and the model's sum (which
            // starts from +0.0) is +0.0: equal values, different sign bit.
            if hand_b != 0.0 {
                assert_eq!(got_b.to_bits(), hand_b.to_bits(), "(b) {t}");
            } else {
                assert_eq!(got_b, hand_b, "(b) {t}");
            }
        }
        // (c)
        let (alpha, vols, temps) = (3.0e-5, [0.2, 0.5, 0.3], [320.0, 410.0, 380.0]);
        let (mut num, mut den) = (0.0, 0.0);
        for c in 0..3 {
            num += vols[c] * (-alpha * (temps[c] - 300.0));
            den += vols[c];
        }
        let genfoam = num / den;
        let sum_v: f64 = vols.iter().sum();
        let fbc = SeparableFeedback {
            fuel_structures: vols
                .iter()
                .map(|v| FuelHeatStructure {
                    temperature_weight: r(0.0),
                    temperature_coefficient: pk(-alpha * v / sum_v),
                    initial_temperature: k(300.0),
                })
                .collect(),
            ..Default::default()
        };
        let got_c = fbc
            .evaluate(s(0.0), &[], &temps.map(k))
            .unwrap()
            .total()
            .get::<ratio>();
        assert!((got_c - genfoam).abs() < 1e-17, "{got_c} vs {genfoam}");
    }
}
