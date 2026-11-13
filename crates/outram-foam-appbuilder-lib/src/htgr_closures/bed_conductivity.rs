// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! # Pebble-bed effective thermal conductivity — model selection
//!
//! [`BedConductivityModel`] chooses how the stagnant effective conductivity of
//! a pebble bed is computed. **The default is [`BedConductivityModel::Breitbach`]**
//! (maintainer decision D3, 2026-11-03): it is the correlation the
//! IAEA-TECDOC-1694 HTR-10 benchmark specifies, and what the INET THERMIX
//! reference calculation used. The default is pinned by the test
//! `breitbach_is_the_physics_default`.
//!
//! ## The variants
//!
//! **`Breitbach`** — IAEA-TECDOC-1694 Table 2.9, "conductivity of uniformed
//! pebble bed", labelled there the German Breitbach correlation
//! (proprietary tier; coefficients only, per D1):
//!
//! ```text
//! lambda = 1.1538e-6 * T1^1.6622   W/(cm K),   T1 = T + 100,   T in deg C
//! if T < 250 deg C then T = 250 deg C
//! ```
//!
//! It is a fit in temperature alone. Pebble diameter (6 cm), porosity
//! (about 0.39), graphite emissivity and the solid conductivity are fixed
//! inside it, because it was fitted to beds of 6 cm graphite spheres. So it
//! is only valid for such a bed. The fuller Breitbach-Barthels model, with
//! `d`, `eps` and emissivity as inputs, is not what the benchmark prints and
//! is **not implemented here** (no source for its coefficients was available
//! to check; Pronghorn's `BreitbachBarthels` radiation object is closed
//! source).
//!
//! **`Zbs(ZbsBed)`** — the analytic Zehner-Bauer-Schlunder model with
//! radiation and contact, [`super::zbs`]. It takes the solid and pore-gas
//! conductivities, the porosity, the diameter and the emissivity. Kept as a
//! labelled comparison variant (D3). It is what KAERI's GAMMA used in the same
//! benchmark.
//!
//! **`Kugeler2017`** — Kugeler, Nabielek & Buckthorpe (2017), EUR 28712 EN,
//! eqs. (108)-(109) (**open**): a fit to the Barthels, Breitbach and Robold
//! laboratory measurements on 6 cm graphite beds, irradiation included,
//! stated uncertainty +/-10 % or more:
//!
//! ```text
//! lambda = 2.549e-4 T^1.545 + 1.5          W/(m K), T <= 1300 deg C
//! lambda = 2e-3 (T - 135)^1.29 + 0.003     W/(m K), 1300 <= T <= 2500 deg C
//! ```
//!
//! It is the open-tier cross-check on the Breitbach fit: both describe the
//! same experiments.
//!
//! ## Verification (methodology and results)
//!
//! See the tests in this module and
//! `verification_and_validation/htr10_th_closures_vs_tecdoc1694.md`. In
//! short (measured 2026-11-03): the Breitbach implementation reproduces
//! hand evaluations of the printed formula to 1e-12. Over 300-1300 K it agrees
//! with the open Kugeler fit within +/-10 % from 800 K up, but Kugeler is
//! 27-34 % higher at 500-600 K, where Breitbach is held at its 250 deg C
//! value. The analytic ZBS, with the benchmark's own helium and unirradiated
//! fuel-matrix conductivity, is **32-107 % above Breitbach** over the HTR-10
//! operating range (523-1273 K); the full table is in
//! `breitbach_vs_zbs_over_the_htr10_range`.

use petir::mathf::RealMath;
use uom::si::f64::{ThermalConductivity, ThermodynamicTemperature};
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::degree_celsius;

use super::zbs::ZbsBed;
use super::HtgrClosureError;

/// The local state a bed-conductivity model is evaluated at.
///
/// The temperature is always needed. The solid (pebble) and pore-gas
/// conductivities are needed only by [`BedConductivityModel::Zbs`]; the
/// fitted models ignore them. Build with [`BedState::new`] when you have all
/// three, or [`BedState::at_temperature`] for a fitted model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BedState {
    /// Local bed temperature.
    pub temperature: ThermodynamicTemperature,
    /// Conductivity of the pebble solid (graphite matrix), if known.
    pub solid_conductivity: Option<ThermalConductivity>,
    /// Conductivity of the gas in the pores, if known.
    pub fluid_conductivity: Option<ThermalConductivity>,
}

impl BedState {
    /// A full state, usable by every model.
    pub fn new(
        temperature: ThermodynamicTemperature,
        solid_conductivity: ThermalConductivity,
        fluid_conductivity: ThermalConductivity,
    ) -> Self {
        Self {
            temperature,
            solid_conductivity: Some(solid_conductivity),
            fluid_conductivity: Some(fluid_conductivity),
        }
    }

    /// A temperature-only state. [`BedConductivityModel::Zbs`] rejects it.
    pub fn at_temperature(temperature: ThermodynamicTemperature) -> Self {
        Self {
            temperature,
            solid_conductivity: None,
            fluid_conductivity: None,
        }
    }
}

/// How the stagnant effective conductivity of a pebble bed is computed.
///
/// `BedConductivityModel::default()` is [`BedConductivityModel::Breitbach`],
/// the benchmark's correlation. The other variants are comparisons; choosing
/// one is an explicit, visible act. See the module docs for each formula and
/// its source.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum BedConductivityModel {
    /// IAEA-TECDOC-1694 Table 2.9 Breitbach fit. **The physics default.**
    #[default]
    Breitbach,
    /// Analytic Zehner-Bauer-Schlunder with radiation and contact. Comparison
    /// variant; needs the solid and gas conductivities.
    Zbs(ZbsBed),
    /// Kugeler et al. (2017) eqs. (108)-(109), open-tier fit to the same
    /// laboratory data as Breitbach. Comparison variant.
    Kugeler2017,
}

impl BedConductivityModel {
    /// Effective bed conductivity at `state`.
    ///
    /// Errors with [`HtgrClosureError::InvalidInput`] when `Zbs` is given a
    /// state without both conductivities, when `Kugeler2017` is asked outside
    /// 0-2500 deg C, or when any model gets a non-finite temperature. ZBS's
    /// own domain errors pass through.
    pub fn effective_conductivity(
        &self,
        state: &BedState,
    ) -> Result<ThermalConductivity, HtgrClosureError> {
        let t_c = state.temperature.get::<degree_celsius>();
        if !t_c.is_finite() {
            return Err(HtgrClosureError::InvalidInput(format!(
                "bed conductivity: non-finite temperature {t_c} deg C"
            )));
        }
        match self {
            Self::Breitbach => Ok(breitbach_tecdoc1694(state.temperature)),
            Self::Kugeler2017 => kugeler2017(state.temperature),
            Self::Zbs(bed) => match (state.solid_conductivity, state.fluid_conductivity) {
                (Some(ks), Some(kf)) => bed.effective_conductivity(ks, kf, state.temperature),
                _ => Err(HtgrClosureError::InvalidInput(
                    "ZBS bed conductivity needs the solid and fluid conductivities; \
                     build the state with BedState::new"
                        .to_string(),
                )),
            },
        }
    }

    /// A short label for tables and logs.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Breitbach => "Breitbach (TECDOC-1694 Tab. 2.9)",
            Self::Zbs(_) => "ZBS (Bauer-Schlunder, comparison)",
            Self::Kugeler2017 => "Kugeler 2017 eq. (108)-(109)",
        }
    }
}

/// The TECDOC-1694 Table 2.9 Breitbach fit,
/// `1.1538e-6 (T + 100)^1.6622` W/(cm K) with `T` in deg C held at 250 below
/// 250 deg C, as printed.
///
/// The low-temperature hold is the source's own rule. It means the fit
/// returns 1.954 W/(m K) at any temperature up to 250 deg C.
pub fn breitbach_tecdoc1694(temperature: ThermodynamicTemperature) -> ThermalConductivity {
    let t_c = temperature.get::<degree_celsius>().max(250.0);
    let w_per_cm_k = 1.1538e-6 * (t_c + 100.0).r_powf(1.6622);
    ThermalConductivity::new::<watt_per_meter_kelvin>(100.0 * w_per_cm_k)
}

/// Kugeler et al. (2017) eqs. (108)-(109), W/(m K), `T` in deg C, valid
/// 0-2500 deg C.
///
/// Errors with [`HtgrClosureError::InvalidInput`] outside 0-2500 deg C
/// (eq. (108) is undefined for negative Celsius temperatures, and eq. (109)
/// stops at 2500 deg C).
pub fn kugeler2017(
    temperature: ThermodynamicTemperature,
) -> Result<ThermalConductivity, HtgrClosureError> {
    let t = temperature.get::<degree_celsius>();
    if !(-1e-9..=2500.0 + 1e-9).contains(&t) {
        return Err(HtgrClosureError::InvalidInput(format!(
            "Kugeler 2017 bed conductivity covers 0-2500 deg C, got {t}"
        )));
    }
    let k = if t <= 1300.0 {
        2.549e-4 * t.max(0.0).r_powf(1.545) + 1.5
    } else {
        2.0e-3 * (t - 135.0).r_powf(1.29) + 0.003
    };
    Ok(ThermalConductivity::new::<watt_per_meter_kelvin>(k))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::htgr_closures::helium::KtaHelium;
    use crate::htgr_closures::structural_materials::StructuralMaterial;
    use uom::si::areal_number_density::per_square_centimeter;
    use uom::si::f64::{ArealNumberDensity, Pressure};
    use uom::si::pressure::bar;
    use uom::si::thermodynamic_temperature::kelvin;

    /// Pinned `[Breitbach, ZBS, ZBS(k_s=26), Kugeler]` at 300..=1300 K.
    const PINNED_COMPARISON: [[f64; 4]; 11] = [
        [1.953801496631243, 2.4716016001858567, 2.0774151758542474, 1.541123376280918],
        [1.953801496631243, 3.165167887126182, 2.6968708599597004, 1.9528492095595962],
        [1.953801496631243, 4.048791100379751, 3.4903688468318403, 2.611696545509089],
        [2.717528544901252, 5.172177351878012, 4.4918655723110374, 3.454511596097899],
        [3.8558367658780957, 6.567276494210073, 5.71319271652674, 4.452195758686665],
        [5.147257804026324, 8.118235776722548, 7.145122568272454, 5.586732638905651],
        [6.583003295386917, 9.855092012895696, 8.76051814992691, 6.845491510175237],
        [8.156000551118659, 11.7629894075315, 10.519860162045363, 8.218959811009864],
        [9.860379940422103, 13.786539932426601, 12.378113472492647, 9.699627710380495],
        [11.69115672333455, 15.874941728659383, 14.291381614654508, 11.281364482844694],
        [13.644021058957904, 17.990042427834773, 16.222033008875332, 12.95903806202565],
    ];

    fn wmk(k: ThermalConductivity) -> f64 {
        k.get::<watt_per_meter_kelvin>()
    }

    fn celsius(t: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<degree_celsius>(t)
    }

    /// Pinned default (root CLAUDE.md, "correct physics is the default").
    ///
    /// Asserts that the ordinary construction path,
    /// `BedConductivityModel::default()`, is `Breitbach`, and that it gives
    /// the Breitbach number at 800 deg C. If someone changes the default to
    /// ZBS, this fails.
    #[test]
    fn breitbach_is_the_physics_default() {
        let model = BedConductivityModel::default();
        assert_eq!(model, BedConductivityModel::Breitbach);
        let k = model
            .effective_conductivity(&BedState::at_temperature(celsius(800.0)))
            .unwrap();
        assert!((wmk(k) - 9.390147924107204).abs() < 1e-9, "{}", wmk(k));
        assert!(model.label().starts_with("Breitbach"));
    }

    /// V&V — Breitbach against hand evaluations of TECDOC-1694 Table 2.9.
    ///
    /// **Methodology.** Evaluate at 100, 250, 526.85, 800, 1000 and
    /// 1300 deg C, and compare with the printed formula evaluated
    /// independently in Python (`100 * 1.1538e-6 * (max(T,250)+100)**1.6622`).
    /// Tolerance 1e-12 relative. Also check the source's hold below 250 deg C.
    ///
    /// **Results (2026-11-03).** 1.9538015 (100 and 250 deg C, the hold),
    /// 5.1472578, 9.3901479, 13.1079102, 19.5715200 W/(m K); all agree to
    /// better than 1e-12.
    #[test]
    fn breitbach_reproduces_hand_evaluations_of_the_table() {
        let cases = [
            (100.0, 1.9538014966312431),
            (250.0, 1.9538014966312431),
            (526.85, 5.147257804026324),
            (800.0, 9.390147924107204),
            (1000.0, 13.107910153131026),
            (1300.0, 19.57151998053828),
        ];
        for (t, expect) in cases {
            let k = wmk(breitbach_tecdoc1694(celsius(t)));
            println!("Breitbach {t:8.2} C: {k:.10} (hand {expect:.10})");
            assert!((k / expect - 1.0).abs() < 1e-12, "{t}: {k} vs {expect}");
        }
    }

    /// V&V — Kugeler (2017) eqs. (108)-(109) against hand evaluations, and
    /// continuity at the 1300 deg C joint.
    ///
    /// **Results (2026-11-03).** 1.8135955 (100 C), 9.2919089 (800 C),
    /// 12.4994213 (1000 C), 17.9972788 (1300 C, eq. 108) W/(m K); eq. (109)
    /// at 1300 C gives 18.0577, a 0.34 % step at the joint, which is the
    /// source's own (the two printed fits do not meet exactly).
    #[test]
    fn kugeler_reproduces_hand_evaluations() {
        for (t, expect) in [
            (100.0, 1.813595509680076),
            (800.0, 9.291908883413319),
            (1000.0, 12.49942126833973),
            (1300.0, 17.997278832236596),
        ] {
            let k = wmk(kugeler2017(celsius(t)).unwrap());
            assert!((k / expect - 1.0).abs() < 1e-12, "{t}: {k} vs {expect}");
        }
        let just_above = wmk(kugeler2017(celsius(1300.0 + 1e-9)).unwrap());
        let step = just_above / 17.997278832236596 - 1.0;
        println!("Kugeler step at 1300 C: {:+.4} %", 100.0 * step);
        assert!(step.abs() < 0.005);
        assert!(kugeler2017(celsius(-1.0)).is_err());
        assert!(kugeler2017(celsius(2600.0)).is_err());
    }

    /// ZBS through the enum needs both conductivities.
    #[test]
    fn zbs_variant_rejects_a_temperature_only_state() {
        let m = BedConductivityModel::Zbs(ZbsBed::htr10());
        assert!(m
            .effective_conductivity(&BedState::at_temperature(celsius(500.0)))
            .is_err());
        assert!(BedConductivityModel::Breitbach
            .effective_conductivity(&BedState::at_temperature(ThermodynamicTemperature::new::<
                kelvin,
            >(f64::NAN)))
            .is_err());
    }

    /// V&V (code-to-correlation comparison) — Breitbach vs ZBS vs Kugeler over
    /// the HTR-10 range, 300-1300 K.
    ///
    /// **Methodology.** At every 100 K from 300 to 1300 K, evaluate:
    /// Breitbach (the default); ZBS via [`ZbsBed::htr10`] (eps 0.39, d 6 cm,
    /// emissivity 0.8, phi 0.0077, C 1.25) with the benchmark's own inputs —
    /// pore gas from [`KtaHelium`] at 30 bar (the HTR-10 3.0 MPa, bar reading
    /// per D5), solid from the TECDOC-1694 Table 2.9 fuel-element
    /// conductivity at zero fast fluence ([`StructuralMaterial::FuelElement`];
    /// the table holds it at 450 deg C below that); and the open Kugeler 2017
    /// fit. Nothing is tuned. The test prints the table, records the spread
    /// `ZBS/Breitbach - 1` and `Kugeler/Breitbach - 1`, and pins the
    /// Breitbach and ZBS values as a regression (1e-9 relative).
    ///
    /// A fourth column repeats ZBS with the VTB deck's solid, `k_s = 26
    /// W/(m K)`, to show how much of the ZBS result rides on `k_s`.
    ///
    /// **Results (2026-11-03, release).** W/(m K); spread relative to
    /// Breitbach.
    ///
    /// | T [K] | k_f | k_s | Breitbach | ZBS | ZBS(k_s 26) | Kugeler | ZBS/B-1 | ZBS26/B-1 | Kug/B-1 |
    /// |---|---|---|---|---|---|---|---|---|---|
    /// | 300 | 0.1553 | 46.16 | 1.954 | 2.472 | 2.077 | 1.541 | +26.5 % | +6.3 % | -21.1 % |
    /// | 400 | 0.1902 | 46.16 | 1.954 | 3.165 | 2.697 | 1.953 | +62.0 % | +38.0 % | -0.0 % |
    /// | 500 | 0.2227 | 46.16 | 1.954 | 4.049 | 3.490 | 2.612 | +107.2 % | +78.6 % | +33.7 % |
    /// | 600 | 0.2532 | 46.16 | 2.718 | 5.172 | 4.492 | 3.455 | +90.3 % | +65.3 % | +27.1 % |
    /// | 700 | 0.2823 | 46.16 | 3.856 | 6.567 | 5.713 | 4.452 | +70.3 % | +48.2 % | +15.5 % |
    /// | 800 | 0.3102 | 43.09 | 5.147 | 8.118 | 7.145 | 5.587 | +57.7 % | +38.8 % | +8.5 % |
    /// | 900 | 0.3371 | 39.93 | 6.583 | 9.855 | 8.761 | 6.845 | +49.7 % | +33.1 % | +4.0 % |
    /// | 1000 | 0.3631 | 37.52 | 8.156 | 11.763 | 10.520 | 8.219 | +44.2 % | +29.0 % | +0.8 % |
    /// | 1100 | 0.3884 | 35.69 | 9.860 | 13.787 | 12.378 | 9.700 | +39.8 % | +25.5 % | -1.6 % |
    /// | 1200 | 0.4130 | 34.32 | 11.691 | 15.875 | 14.291 | 11.281 | +35.8 % | +22.2 % | -3.5 % |
    /// | 1300 | 0.4370 | 33.33 | 13.644 | 17.990 | 16.222 | 12.959 | +31.9 % | +18.9 % | -5.0 % |
    ///
    /// **Interpretation.** Over the HTR-10 operating range (523-1273 K) ZBS
    /// is 32-107 % above the benchmark's Breitbach fit, largest at 500-600 K
    /// where Breitbach is held at its 250 deg C value. Breitbach and the open
    /// Kugeler fit, which describe the same Juelich experiments, agree within
    /// +/-10 % from 800 K up. ZBS is above both laboratory fits everywhere
    /// above 300 K. Part of that is the solid: both fits include irradiation
    /// (Kugeler says so explicitly), while the ZBS column uses the
    /// unirradiated matrix; with `k_s = 26` the excess falls by 13-29
    /// percentage points but does not close. Nothing was tuned. This is a
    /// comparison of models with each other, not a validation; it says that a
    /// bed temperature computed with ZBS instead of the benchmark's
    /// correlation will run cooler, by an amount a later sensitivity study
    /// should measure.
    #[test]
    fn breitbach_vs_zbs_over_the_htr10_range() {
        let zbs = BedConductivityModel::Zbs(ZbsBed::htr10());
        let zero_dose = ArealNumberDensity::new::<per_square_centimeter>(0.0);
        let fuel = StructuralMaterial::FuelElement {
            fast_fluence: zero_dose,
        };
        let vtb_solid = ThermalConductivity::new::<watt_per_meter_kelvin>(26.0);
        let p = Pressure::new::<bar>(30.0);
        println!(
            "{:>6} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8}",
            "T [K]", "k_f", "k_s", "Breitb", "ZBS", "ZBS(26)", "Kugeler", "ZBS/B-1", "Z26/B-1", "Kug/B-1"
        );
        let mut rows = Vec::new();
        for i in 0..=10 {
            let t = ThermodynamicTemperature::new::<kelvin>(300.0 + 100.0 * i as f64);
            let kf = KtaHelium::new(p, t).unwrap().thermal_conductivity();
            let ks = fuel.thermal_conductivity(t).unwrap();
            let state = BedState::new(t, ks, kf);
            let b = wmk(BedConductivityModel::default().effective_conductivity(&state).unwrap());
            let z = wmk(zbs.effective_conductivity(&state).unwrap());
            let z26 = wmk(zbs
                .effective_conductivity(&BedState::new(t, vtb_solid, kf))
                .unwrap());
            let k = wmk(BedConductivityModel::Kugeler2017
                .effective_conductivity(&state)
                .unwrap());
            println!(
                "{:6.0} {:8.5} {:8.4} {:8.4} {:8.4} {:8.4} {:8.4} {:+7.1}% {:+7.1}% {:+7.1}%",
                t.get::<kelvin>(),
                wmk(kf),
                wmk(ks),
                b,
                z,
                z26,
                k,
                100.0 * (z / b - 1.0),
                100.0 * (z26 / b - 1.0),
                100.0 * (k / b - 1.0)
            );
            rows.push([b, z, z26, k]);
        }
        // Regression pin of the measured table (2026-11-03). Not a gate on
        // the spread: the spread is the finding, recorded in the doc.
        const PINNED: [[f64; 4]; 11] = PINNED_COMPARISON;
        for (row, pin) in rows.iter().zip(PINNED.iter()) {
            for (got, want) in row.iter().zip(pin.iter()) {
                assert!((got / want - 1.0).abs() < 1e-6, "{got} vs pinned {want}");
            }
        }
    }
}
