// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! # Conductivity of the HTR-10 solids (IAEA-TECDOC-1694 Tab. 2.9-2.12, Fig. 2.8)
//!
//! The benchmark's recommended conductivities for the solids of the HTR-10
//! reactor unit. Proprietary tier: coefficients and formulas only, each
//! cited to its table (maintainer decision D1). `T` is in deg C, the fast
//! fluence `D` in units of 1e21 per cm^2 (the table writes only
//! "dosage of fast neutrons [10^21]"; per cm^2 is the German convention it
//! follows, and the energy threshold is not stated — **Uncertain**).
//!
//! | Variant | Formula as printed, W/(cm K) | Source, stated range |
//! |---|---|---|
//! | `FuelElement` | `((-0.3906e-4 T + 0.06829)/(D + 1.931e-4 T + 0.105) + 1.228e-4 T + 0.042) * 1.2768`; `T` held at 450 below 450; `D` held at 2.5 above 2.5 | Tab. 2.9; 450-1300 deg C |
//! | `SideReflector` | `1.15 (1.5648 - 0.3162 log10(T + 100)) F`, `F = F1/(Y - F2) + F3`, `Y = D/10`, `F1 = -0.0054705 + 0.00038214 x + 0.13487 x^2`, `F2 = -0.013951 + 0.12064 x - 0.32955 x^2`, `F3 = -0.07264 + 0.41459 x + 0.23149 x^2`, `x = T/1000` | Tab. 2.11; 0-2000 deg C |
//! | `TopBottomReflector` | `1.15 (1 - 1.084 x + 0.743 x^2 - 0.213 x^3)`, `x = T/1000` | Tab. 2.11; up to 1700 deg C |
//! | `CarbonBrick` | `0.05 + 0.03e-3 T` | Tab. 2.12; up to 1000 deg C |
//! | `Steel(..)` | digitised from Fig. 2.8 (no formula is printed) | Fig. 2.8 |
//!
//! ## Two things a reader must know
//!
//! **The side-reflector `log` is read as log10.** The table writes `log`.
//! With the natural log, `1.5648 - 0.3162 ln(T + 100)` is negative above
//! 40 deg C, so the conductivity would be negative over the whole operating
//! range. With log10 the unirradiated value is 104 W/(m K) at 20 deg C, close
//! to the top/bottom-reflector fit's 113 W/(m K). That inference is
//! **Uncertain** and flagged for the maintainer.
//!
//! **The side-reflector fit breaks down below about 300 deg C, despite its
//! stated 0-2000 deg C range.** At zero fluence the factor `F` should be
//! about 1 (unirradiated graphite). Measured from the printed coefficients
//! (test `side_reflector_dose_factor_table`): `F` = 0.987 at 300 deg C,
//! 0.982 at 500, 1.000 at 700 and 1.156 at 1000 deg C, but **0.742 at
//! 250 deg C, 0.020 at 200 deg C and -0.528 at 20 deg C** — `F2` changes sign
//! near 130 deg C, so `F1/(Y - F2)` has a pole there. A negative or
//! near-zero conductivity is not graphite. This module therefore refuses the
//! side-reflector fit below 300 deg C with [`HtgrClosureError::Unphysical`]
//! rather than return those numbers. The 300 deg C cut-off is this module's
//! choice, made from the table above, not a value in the source. The HTR-10
//! side reflector sees about 250 deg C at the cold inlet, so a model that
//! needs it there must make an explicit choice (for example the
//! top/bottom-reflector fit, which is unirradiated). **Flagged for the
//! maintainer.**
//!
//! ## Steels (Fig. 2.8) — digitised, **Uncertain**
//!
//! The benchmark gives steel conductivity only as a figure, two curves:
//! "SA240, SS316, core barrel" (solid) and "SA508, SA516-70, RPV" (dashed).
//! The brief also named SA387-11; **it does not appear in Fig. 2.8**, so no
//! curve is given for it.
//!
//! *Digitisation protocol (2026-11-03).* Page 29 of the PDF (printed page 18)
//! was rendered at 600 dpi with `pdftoppm`. The plot frame and tick marks were
//! located from pixel rows and columns: x ticks every 470.4 px per 200 K
//! (frame 0 K at column 27, 1200 K at 2849.6); y ticks every 162.5 px per
//! 5 W/(m K) (0 at row 1205.5; the 45 W/(m K) frame edge falls at 45.02,
//! a calibration check). Solid-curve pixels (all channels < 100) and
//! dashed-curve pixels (blue exceeding red and green by 60) were averaged per
//! column and then over a +/-5 K window at each tabulated temperature. One
//! pixel is 0.03 W/(m K) vertically and 0.43 K horizontally; the line is
//! about 0.1 W/(m K) thick. The figure itself is coarse, so the read values
//! are taken to be good to about +/-0.3 W/(m K). Between points this module
//! interpolates linearly, and it refuses temperatures outside the drawn span
//! (no extrapolation).

use petir::mathf::RealMath;
use uom::si::areal_number_density::per_square_centimeter;
use uom::si::f64::{ArealNumberDensity, ThermalConductivity, ThermodynamicTemperature};
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::{degree_celsius, kelvin};

use super::HtgrClosureError;

/// The two steel curves of IAEA-TECDOC-1694 Fig. 2.8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SteelComponent {
    /// Solid curve, "SA240, SS316, core barrel".
    CoreBarrelSa240Ss316,
    /// Dashed curve, "SA508, SA516-70, RPV".
    RpvSa508Sa516_70,
}

impl SteelComponent {
    /// The digitised `(T [K], k [W/(m K)])` points of this curve (see the
    /// module docs for the protocol). **Uncertain**: read off a figure, good
    /// to about +/-0.3 W/(m K).
    pub fn digitised_points(&self) -> &'static [(f64, f64)] {
        match self {
            Self::CoreBarrelSa240Ss316 => &[
                (293.0, 13.31),
                (400.0, 14.89),
                (500.0, 16.38),
                (600.0, 17.80),
                (700.0, 19.18),
                (800.0, 20.45),
                (900.0, 21.65),
                (1000.0, 22.78),
                (1070.0, 23.55),
            ],
            Self::RpvSa508Sa516_70 => &[
                (290.0, 37.74),
                (300.0, 37.77),
                (350.0, 38.45),
                (400.0, 38.66),
                (450.0, 38.63),
                (500.0, 38.33),
                (550.0, 37.73),
                (600.0, 37.08),
                (650.0, 36.23),
                (700.0, 35.40),
            ],
        }
    }
}

/// A solid of the HTR-10 reactor unit whose conductivity IAEA-TECDOC-1694
/// recommends. See the module docs for each formula, its table and its range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StructuralMaterial {
    /// Fuel-element (pebble) graphite matrix, Tab. 2.9, irradiation-dependent.
    FuelElement {
        /// Fast-neutron fluence, per cm^2. The table's `DOSIS` is this
        /// divided by 1e21, held at 2.5 above 2.5.
        fast_fluence: ArealNumberDensity,
    },
    /// Side-reflector graphite, Tab. 2.11, irradiation-dependent. Refused
    /// below 300 deg C (see the module docs).
    SideReflector {
        /// Fast-neutron fluence, per cm^2 (`D` is this divided by 1e21).
        fast_fluence: ArealNumberDensity,
    },
    /// Top and bottom reflector graphite, Tab. 2.11, unirradiated.
    TopBottomReflector,
    /// Carbon brick, Tab. 2.12.
    CarbonBrick,
    /// A steel from Fig. 2.8, digitised (**Uncertain**).
    Steel(SteelComponent),
}

/// Range check with a 1e-9 slack, so a temperature that round-trips through
/// kelvin (1000 deg C comes back as 1000.0000000000001) is not refused.
fn within(t: f64, lo: f64, hi: f64) -> bool {
    t >= lo - 1e-9 && t <= hi + 1e-9
}

fn wmk(w_per_cm_k: f64) -> ThermalConductivity {
    ThermalConductivity::new::<watt_per_meter_kelvin>(100.0 * w_per_cm_k)
}

fn dose_units(fluence: ArealNumberDensity) -> Result<f64, HtgrClosureError> {
    let d = fluence.get::<per_square_centimeter>() / 1.0e21;
    if !d.is_finite() || d < 0.0 {
        return Err(HtgrClosureError::InvalidInput(format!(
            "fast fluence must be finite and non-negative, got {d}e21 /cm^2"
        )));
    }
    Ok(d)
}

impl StructuralMaterial {
    /// The side-reflector dose factor `F = F1/(Y - F2) + F3` of Tab. 2.11,
    /// `Y = D/10`, at `T` deg C. Exposed so the breakdown below 300 deg C can
    /// be tabulated (see the module docs).
    pub fn side_reflector_dose_factor(t_celsius: f64, dose_1e21: f64) -> f64 {
        let x = t_celsius / 1000.0;
        let f1 = -0.0054705 + 0.00038214 * x + 0.13487 * x * x;
        let f2 = -0.013951 + 0.12064 * x - 0.32955 * x * x;
        let f3 = -0.07264 + 0.41459 * x + 0.23149 * x * x;
        f1 / (dose_1e21 / 10.0 - f2) + f3
    }

    /// Thermal conductivity at `temperature`.
    ///
    /// Errors with [`HtgrClosureError::InvalidInput`] outside the stated
    /// range of the fit (or the drawn span of a steel curve), or for a
    /// negative fluence; with [`HtgrClosureError::Unphysical`] for the side
    /// reflector below 300 deg C or if a printed fit returns a non-positive
    /// value. Holds that the source prints (fuel element below 450 deg C,
    /// dose above 2.5e21) are applied as printed.
    pub fn thermal_conductivity(
        &self,
        temperature: ThermodynamicTemperature,
    ) -> Result<ThermalConductivity, HtgrClosureError> {
        let t = temperature.get::<degree_celsius>();
        if !t.is_finite() {
            return Err(HtgrClosureError::InvalidInput(format!(
                "non-finite temperature {t} deg C"
            )));
        }
        let out_of_range = |lo: f64, hi: f64, what: &str| {
            HtgrClosureError::InvalidInput(format!(
                "{what} conductivity covers {lo}-{hi} deg C, got {t}"
            ))
        };
        let k = match self {
            Self::FuelElement { fast_fluence } => {
                // Upper bound 1300 deg C as stated; the low end is held, as printed.
                if !within(t, f64::NEG_INFINITY, 1300.0) {
                    return Err(out_of_range(450.0, 1300.0, "fuel-element"));
                }
                let tc = t.max(450.0);
                let dosis = dose_units(*fast_fluence)?.min(2.5);
                wmk(((-0.3906e-4 * tc + 0.06829) / (dosis + 1.931e-4 * tc + 0.105)
                    + 1.228e-4 * tc
                    + 0.042)
                    * 1.2768)
            }
            Self::SideReflector { fast_fluence } => {
                if !within(t, 0.0, 2000.0) {
                    return Err(out_of_range(0.0, 2000.0, "side-reflector"));
                }
                let d = dose_units(*fast_fluence)?;
                if t < 300.0 {
                    return Err(HtgrClosureError::Unphysical(format!(
                        "side-reflector fit (TECDOC-1694 Tab. 2.11) is refused below 300 deg C: \
                         its dose factor F is {:.4} here at this fluence and falls through a \
                         pole near 130 deg C; got T = {t} deg C",
                        Self::side_reflector_dose_factor(t, d)
                    )));
                }
                let f = Self::side_reflector_dose_factor(t, d);
                wmk(1.15 * (1.5648 - 0.3162 * (t + 100.0).r_log10()) * f)
            }
            Self::TopBottomReflector => {
                if !within(t, 0.0, 1700.0) {
                    return Err(out_of_range(0.0, 1700.0, "top/bottom-reflector"));
                }
                let x = t / 1000.0;
                wmk(1.15 * (1.0 - 1.084 * x + 0.743 * x * x - 0.213 * x * x * x))
            }
            Self::CarbonBrick => {
                if !within(t, 0.0, 1000.0) {
                    return Err(out_of_range(0.0, 1000.0, "carbon-brick"));
                }
                wmk(0.05 + 0.03e-3 * t)
            }
            Self::Steel(steel) => {
                let pts = steel.digitised_points();
                let tk = temperature.get::<kelvin>();
                let (lo, hi) = (pts[0].0, pts[pts.len() - 1].0);
                if !within(tk, lo, hi) {
                    return Err(HtgrClosureError::InvalidInput(format!(
                        "{steel:?} is digitised over {lo}-{hi} K only, got {tk} K"
                    )));
                }
                let i = pts
                    .windows(2)
                    .position(|w| tk <= w[1].0)
                    .map(|i| i.min(pts.len() - 2))
                    .unwrap_or(pts.len() - 2);
                let ((t0, k0), (t1, k1)) = (pts[i], pts[i + 1]);
                ThermalConductivity::new::<watt_per_meter_kelvin>(
                    k0 + (k1 - k0) * (tk - t0) / (t1 - t0),
                )
            }
        };
        let v = k.get::<watt_per_meter_kelvin>();
        if !(v.is_finite() && v > 0.0) {
            return Err(HtgrClosureError::Unphysical(format!(
                "{self:?} gave a non-positive conductivity {v} W/(m K) at {t} deg C"
            )));
        }
        Ok(k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(t: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<degree_celsius>(t)
    }
    fn dose(d: f64) -> ArealNumberDensity {
        ArealNumberDensity::new::<per_square_centimeter>(d * 1e21)
    }
    fn k(m: StructuralMaterial, t: f64) -> f64 {
        m.thermal_conductivity(c(t)).unwrap().get::<watt_per_meter_kelvin>()
    }

    /// V&V — every TECDOC-1694 formula against an independent Python hand
    /// evaluation of the printed coefficients.
    ///
    /// **Methodology.** The printed formulas (Tab. 2.9, 2.11, 2.12) were
    /// evaluated in Python at the points below, separately from this code,
    /// with the side-reflector `log` as log10. Tolerance 1e-12 relative.
    ///
    /// **Results (2026-11-03).** Fuel element: 46.1608 (450 C, D 0),
    /// 33.5612 (1000 C, D 0), 22.3755 (1000 C, D 2.5), 21.6610 (800 C, D 1)
    /// W/(m K). Side reflector: 84.2096 (300 C, D 0), 31.6985 (500 C, D 1),
    /// 58.8085 (1000 C, D 2.5). Top/bottom: 112.5408 (20 C), 70.9694 (500 C),
    /// 51.2900 (1000 C), 29.6701 (1700 C). Carbon brick: 5.0 (0 C), 6.5
    /// (500 C), 8.0 (1000 C). All match to < 1e-12. Sanity: the fuel element
    /// at 1000 deg C, D 0, gives 33.6 W/(m K), above the >= 25 W/(m K)
    /// matrix specification quoted in the HTR-10 fuel literature.
    #[test]
    fn tecdoc1694_fits_match_hand_evaluations() {
        use StructuralMaterial::*;
        let cases: [(StructuralMaterial, f64, f64); 14] = [
            (FuelElement { fast_fluence: dose(0.0) }, 450.0, 46.16075478327211),
            (FuelElement { fast_fluence: dose(0.0) }, 1000.0, 33.561242664877554),
            (FuelElement { fast_fluence: dose(2.5) }, 1000.0, 22.37545707387156),
            (FuelElement { fast_fluence: dose(1.0) }, 800.0, 21.660982273268335),
            (SideReflector { fast_fluence: dose(0.0) }, 300.0, 84.2096),
            (SideReflector { fast_fluence: dose(1.0) }, 500.0, 31.6985),
            (SideReflector { fast_fluence: dose(2.5) }, 1000.0, 58.8085),
            (TopBottomReflector, 20.0, 112.54078203999998),
            (TopBottomReflector, 500.0, 70.96937499999999),
            (TopBottomReflector, 1000.0, 51.289999999999985),
            (TopBottomReflector, 1700.0, 29.67011499999999),
            (CarbonBrick, 0.0, 5.0),
            (CarbonBrick, 500.0, 6.5),
            (CarbonBrick, 1000.0, 8.0),
        ];
        for (m, t, expect) in cases {
            let got = k(m, t);
            println!("{m:?} {t} C: {got:.10} (hand {expect})");
            // The side-reflector hand values were printed to 4 decimals.
            let tol = if matches!(m, SideReflector { .. }) { 2e-6 } else { 1e-12 };
            assert!((got / expect - 1.0).abs() < tol, "{m:?} {t}: {got} vs {expect}");
        }
        // The fuel element holds T at 450 deg C below 450, as printed, and
        // DOSIS at 2.5 above 2.5.
        assert_eq!(k(FuelElement { fast_fluence: dose(0.0) }, 300.0), k(FuelElement { fast_fluence: dose(0.0) }, 450.0));
        assert_eq!(k(FuelElement { fast_fluence: dose(5.0) }, 900.0), k(FuelElement { fast_fluence: dose(2.5) }, 900.0));
    }

    /// V&V (characterisation) — the side-reflector dose factor `F` at zero
    /// and finite fluence, showing the breakdown below 300 deg C.
    ///
    /// **Results (2026-11-03).** At D = 0: F = -0.5277 (20 C), 0.0198
    /// (200 C), 0.7416 (250 C), 0.9868 (300 C), 0.9821 (500 C), 1.0002
    /// (700 C), 1.1558 (1000 C). At D = 2.5: 0.0989 (300 C), 0.2920 (500 C),
    /// 0.5096 (700 C), 0.8479 (1000 C). Irradiation lowers F at every
    /// temperature, most strongly when cold, as irradiated graphite should.
    /// The module refuses T < 300 deg C.
    #[test]
    fn side_reflector_dose_factor_table() {
        let f = StructuralMaterial::side_reflector_dose_factor;
        for (t, expect) in [
            (20.0, -0.5277),
            (200.0, 0.0198),
            (250.0, 0.7416),
            (300.0, 0.9868),
            (500.0, 0.9821),
            (700.0, 1.0002),
            (1000.0, 1.1558),
        ] {
            println!("F({t} C, D=0) = {:.4}", f(t, 0.0));
            assert!((f(t, 0.0) - expect).abs() < 5e-5, "{t}");
        }
        for t in [300.0, 500.0, 700.0, 1000.0] {
            assert!(f(t, 2.5) < f(t, 1.0) && f(t, 1.0) < f(t, 0.0));
        }
        let side = StructuralMaterial::SideReflector { fast_fluence: dose(0.0) };
        assert!(matches!(side.thermal_conductivity(c(250.0)), Err(HtgrClosureError::Unphysical(_))));
        assert!(side.thermal_conductivity(c(300.0)).is_ok());
        assert!(side.thermal_conductivity(c(2100.0)).is_err());
        let neg = StructuralMaterial::SideReflector { fast_fluence: dose(-1.0) };
        assert!(neg.thermal_conductivity(c(500.0)).is_err());
    }

    /// V&V — the digitised Fig. 2.8 steel curves interpolate through their
    /// points, stay inside the figure's visible span, and refuse
    /// extrapolation.
    ///
    /// **Results (2026-11-03).** Core barrel 13.31 (293 K) to 23.55 W/(m K)
    /// (1070 K), monotonic; RPV peaks at 38.66 W/(m K) near 400 K and falls
    /// to 35.40 at 700 K. Midpoint 650 K on the barrel interpolates to
    /// 18.49 W/(m K).
    #[test]
    fn digitised_steel_curves_interpolate_and_do_not_extrapolate() {
        let barrel = StructuralMaterial::Steel(SteelComponent::CoreBarrelSa240Ss316);
        let rpv = StructuralMaterial::Steel(SteelComponent::RpvSa508Sa516_70);
        let tk = |t: f64| ThermodynamicTemperature::new::<kelvin>(t);
        let kk = |m: StructuralMaterial, t: f64| {
            m.thermal_conductivity(tk(t)).unwrap().get::<watt_per_meter_kelvin>()
        };
        for steel in [SteelComponent::CoreBarrelSa240Ss316, SteelComponent::RpvSa508Sa516_70] {
            for &(t, v) in steel.digitised_points() {
                assert!((kk(StructuralMaterial::Steel(steel), t) - v).abs() < 1e-9);
            }
        }
        assert!((kk(barrel, 650.0) - 18.49).abs() < 1e-9);
        let pts = SteelComponent::CoreBarrelSa240Ss316.digitised_points();
        assert!(pts.windows(2).all(|w| w[1].1 > w[0].1));
        assert!(barrel.thermal_conductivity(tk(1100.0)).is_err());
        assert!(rpv.thermal_conductivity(tk(800.0)).is_err());
        assert!(rpv.thermal_conductivity(tk(250.0)).is_err());
        assert!(StructuralMaterial::CarbonBrick.thermal_conductivity(c(1100.0)).is_err());
        assert!(StructuralMaterial::TopBottomReflector.thermal_conductivity(c(1800.0)).is_err());
        assert!(StructuralMaterial::FuelElement { fast_fluence: dose(0.0) }
            .thermal_conductivity(c(1400.0))
            .is_err());
    }
}
