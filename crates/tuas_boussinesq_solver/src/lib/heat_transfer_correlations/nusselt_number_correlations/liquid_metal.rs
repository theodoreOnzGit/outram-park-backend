//! Named liquid-metal Nusselt-number correlations (gh:#1321; EBR-II
//! groundwork, epic gh:#1312).
//!
//! Liquid metals have Prandtl numbers of order 1e-3 to 1e-2 (sodium about
//! 5e-3, see `liquid_database::liquid_sodium_coolprop`), so heat is carried
//! largely by molecular conduction even in turbulent flow, and the
//! Dittus-Boelter/Gnielinski forms (fitted for `Pr ≳ 0.5`) do not apply.
//! These correlations are written in the Péclet number `Pe = Re·Pr`.
//!
//! | Function | Geometry | Formula | Validity |
//! |---|---|---|---|
//! | [`lyon_martinelli_nusselt`] | circular tube, uniform heat flux | `7 + 0.025 Pe^0.8` | upstream states none |
//! | [`seban_shimazaki_nusselt`] | circular tube, uniform wall temperature | `5 + 0.025 Pe^0.8` | not checked |
//! | [`kazimi_carelli_nusselt`] | triangular rod bundle | `4 + 0.33 x^3.8 (Pe/100)^0.86 + 0.16 x^5` | `1.1 ≤ x ≤ 1.4`, `10 ≤ Pe ≤ 5000` |
//! | [`schad_modified_nusselt`] | triangular rod bundle | `f(x) Pe^0.3` (`Pe ≥ 150`), `4.496 f(x)` (`Pe < 150`) | `1.1 ≤ x ≤ 1.5`, `Pe ≤ 1000` |
//! | [`mikityuk_nusselt`] | triangular rod bundle | `0.047 (1 − e^(−3.8(x−1))) (Pe^0.77 + 250)` | `1.1 ≤ x ≤ 1.5`, `30 ≤ Pe ≤ 5000` |
//!
//! with `x = P/D`, the pitch-to-diameter ratio, and
//! `f(x) = −16.15 + 24.96 x − 8.55 x²`. Nusselt and Péclet numbers are on the
//! hydraulic diameter.
//!
//! # Upstream
//!
//! Four of the five (Lyon, Kazimi-Carelli, Schad-modified, Mikityuk) follow
//! the MOOSE framework's thermal-hydraulics module (Idaho National
//! Laboratory, LGPL-2.1), commit `e316801b544a`,
//! `modules/thermal_hydraulics/src/materials/ADWallHeatTransferCoefficient{Lyon,Kazimi,Schad,Mikityuk}Material.C`,
//! whose documentation cites Todreas & Kazimi, *Nuclear Systems I*, 3rd ed.
//! (2021), Eqs. 10.126a, 10.129, 10.130 and 10.133. These are one-line
//! published formulas; the MOOSE implementation was read as the reference for
//! the formula **and its guards**, and no MOOSE code was copied. What was
//! taken from it:
//!
//! - **`Pe` is clamped to at least 1** (`max(1.0, Pe)` in every one of the
//!   four), so `Pe = 0` (stagnant fluid) gives a finite, positive Nusselt
//!   number. Ported here unchanged.
//! - **Validity ranges are warned about, not enforced.** MOOSE calls
//!   `mooseWarning` once and still evaluates. Here the formula is always
//!   evaluated and [`LiquidMetalNusseltData::is_within_validity_range`]
//!   reports whether the inputs lie inside the upstream range.
//! - **Schad's branch point is `Pe < 150`** (strict), the low-Pe constant
//!   being `4.496 f(x)`; `4.496 ≈ 150^0.3`, so the two
//!   branches meet to 4 significant figures at `Pe = 150`
//!   (`150^0.3 = 4.49601`).
//!
//! **An upstream documentation defect, found while porting.** MOOSE's page
//! for the Mikityuk material writes the bundle factor as
//! `1 − exp(P/D − 1)`, without the `−3.8` multiplier; its code (and Mikityuk
//! 2009) use `1 − exp(−3.8 (P/D − 1))`. The code's form is used here. With
//! the documented form the factor is negative for every `P/D > 1`.
//!
//! # Sources
//!
//! - R.N. Lyon, "Liquid metal heat-transfer coefficients", *Chem. Eng.
//!   Prog.* 47 (1951) 75-79 (the Lyon-Martinelli correlation).
//! - R.A. Seban, T.T. Shimazaki, "Heat transfer to a fluid flowing
//!   turbulently in a smooth pipe with walls at constant temperature",
//!   *Trans. ASME* 73 (1951) 803-809. **No upstream code was found for this
//!   one**; the form `5 + 0.025 Pe^0.8` is the textbook form and is **Not
//!   re-checked** against the paper (not held in the corpus).
//! - M.S. Kazimi, M.D. Carelli, *Heat Transfer Correlation for Analysis of
//!   CRBRP Assemblies*, Westinghouse report CRBRP-ARD-0034 (1976), via
//!   Todreas & Kazimi Eq. 10.129.
//! - Schad-modified: Todreas & Kazimi Eq. 10.130 (MOOSE's citation; the
//!   original Schad report is not held and is **Not re-checked**).
//! - K. Mikityuk, "Heat transfer to liquid metal: Review of data and
//!   correlations for tube bundles", *Nucl. Eng. Des.* 239 (2009) 680-687.
//!   MOOSE warns outside `1.1 ≤ P/D ≤ 1.5`; the paper's own stated range is
//!   **Not re-checked** here.
//! - N.E. Todreas, M.S. Kazimi, *Nuclear Systems Volume I: Thermal Hydraulic
//!   Fundamentals*, 3rd ed., CRC Press (2021).
//!
//! None of the papers is in the literature corpus; every range and formula
//! above is as MOOSE states it unless marked otherwise.
//!
//! # Uncalibrated
//!
//! No coefficient here is fitted to anything in this workspace. They are the
//! published values, run as they are (root `CLAUDE.md`, model hierarchy).
//!
//! # Low-Péclet axial conduction in TUAS (read, not changed)
//!
//! At low `Pe` the fluid's own axial conduction matters. TUAS's
//! `non_insulated_parallel_fluid_components` and `shell_and_tube_heat_exchanger`
//! `calculation.rs` add an axial conductance `k A / Δx` between fluid nodes
//! only when `Pe < 100`, and carry the author's note "this part is quite
//! buggy as in the peclet number correction bit". For sodium (`Pr ≈ 5e-3`)
//! `Pe < 100` means `Re ≲ 2e4`, which a coast-down reaches; that switch is
//! not exercised by these correlations and has not been verified for liquid
//! metals.
//!
//! V&V: the test module below; summary in
//! `crates/tuas_boussinesq_solver/docs/liquid_sodium_coolprop_vv.md`.
//!
//! AI-drafted (Claude Opus 5.5 in Claude Code), untrusted until reviewed.
//! Research, education and V&V only.

use petir::mathf::RealMath;
use uom::si::f64::Ratio;
use uom::si::ratio::ratio;

use crate::tuas_lib_error::TuasLibError;

/// MOOSE's guard: `Pe` is taken as at least 1 before any correlation.
fn clamped_peclet(peclet: Ratio) -> f64 {
    peclet.get::<ratio>().max(1.0)
}

/// Lyon-Martinelli, circular tube, uniform heat flux:
/// `Nu = 7 + 0.025 Pe^0.8` (Lyon 1951; MOOSE `ADWallHeatTransferCoefficientLyonMaterial`).
pub fn lyon_martinelli_nusselt(peclet: Ratio) -> Ratio {
    let pe = clamped_peclet(peclet);
    Ratio::new::<ratio>(7.0 + 0.025 * pe.r_powf(0.8))
}

/// Seban-Shimazaki, circular tube, uniform wall temperature:
/// `Nu = 5 + 0.025 Pe^0.8` (Seban & Shimazaki 1951). MOOSE's `Pe ≥ 1` clamp
/// is applied for consistency with the other four.
pub fn seban_shimazaki_nusselt(peclet: Ratio) -> Ratio {
    let pe = clamped_peclet(peclet);
    Ratio::new::<ratio>(5.0 + 0.025 * pe.r_powf(0.8))
}

/// Kazimi-Carelli, triangular rod bundle:
/// `Nu = 4 + 0.33 x^3.8 (Pe/100)^0.86 + 0.16 x^5`, `x = P/D`.
pub fn kazimi_carelli_nusselt(peclet: Ratio, pitch_to_diameter: Ratio) -> Ratio {
    let pe = clamped_peclet(peclet);
    let x = pitch_to_diameter.get::<ratio>();
    Ratio::new::<ratio>(4.0 + 0.33 * x.r_powf(3.8) * (pe / 100.0).r_powf(0.86) + 0.16 * x.r_powf(5.0))
}

/// Schad-modified, triangular rod bundle:
/// `Nu = f(x) Pe^0.3` for `Pe ≥ 150`, `Nu = 4.496 f(x)` for `Pe < 150`,
/// `f(x) = −16.15 + 24.96 x − 8.55 x²`, `x = P/D`.
pub fn schad_modified_nusselt(peclet: Ratio, pitch_to_diameter: Ratio) -> Ratio {
    let pe = clamped_peclet(peclet);
    let x = pitch_to_diameter.get::<ratio>();
    let f = -16.15 + 24.96 * x - 8.55 * x * x;
    if pe < 150.0 {
        Ratio::new::<ratio>(4.496 * f)
    } else {
        Ratio::new::<ratio>(f * pe.r_powf(0.3))
    }
}

/// Mikityuk, triangular rod bundle:
/// `Nu = 0.047 (1 − exp(−3.8 (x − 1))) (Pe^0.77 + 250)`, `x = P/D`.
pub fn mikityuk_nusselt(peclet: Ratio, pitch_to_diameter: Ratio) -> Ratio {
    let pe = clamped_peclet(peclet);
    let x = pitch_to_diameter.get::<ratio>();
    Ratio::new::<ratio>(0.047 * (1.0 - (-3.8 * (x - 1.0)).r_exp()) * (pe.r_powf(0.77) + 250.0))
}

/// Which liquid-metal correlation, with the bundle geometry where it needs
/// one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LiquidMetalCorrelation {
    /// [`lyon_martinelli_nusselt`]: circular tube, uniform heat flux.
    LyonMartinelli,
    /// [`seban_shimazaki_nusselt`]: circular tube, uniform wall temperature.
    SebanShimazaki,
    /// [`kazimi_carelli_nusselt`]: rod bundle.
    KazimiCarelli {
        /// Pitch-to-diameter ratio `P/D`.
        pitch_to_diameter: Ratio,
    },
    /// [`schad_modified_nusselt`]: rod bundle.
    SchadModified {
        /// Pitch-to-diameter ratio `P/D`.
        pitch_to_diameter: Ratio,
    },
    /// [`mikityuk_nusselt`]: rod bundle.
    Mikityuk {
        /// Pitch-to-diameter ratio `P/D`.
        pitch_to_diameter: Ratio,
    },
}

/// The validity range the upstream (MOOSE) states for a correlation.
/// `None` on a bound means none is stated.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LiquidMetalValidityRange {
    /// Lowest Péclet number.
    pub peclet_min: Option<f64>,
    /// Highest Péclet number.
    pub peclet_max: Option<f64>,
    /// Lowest and highest `P/D` (rod-bundle correlations only).
    pub pitch_to_diameter: Option<(f64, f64)>,
}

impl LiquidMetalCorrelation {
    /// The upstream validity range (see the module table). Lyon-Martinelli
    /// and Seban-Shimazaki carry none upstream.
    pub fn validity_range(&self) -> LiquidMetalValidityRange {
        match self {
            Self::LyonMartinelli | Self::SebanShimazaki => LiquidMetalValidityRange {
                peclet_min: None,
                peclet_max: None,
                pitch_to_diameter: None,
            },
            Self::KazimiCarelli { .. } => LiquidMetalValidityRange {
                peclet_min: Some(10.0),
                peclet_max: Some(5000.0),
                pitch_to_diameter: Some((1.1, 1.4)),
            },
            Self::SchadModified { .. } => LiquidMetalValidityRange {
                peclet_min: None,
                peclet_max: Some(1000.0),
                pitch_to_diameter: Some((1.1, 1.5)),
            },
            Self::Mikityuk { .. } => LiquidMetalValidityRange {
                peclet_min: Some(30.0),
                peclet_max: Some(5000.0),
                pitch_to_diameter: Some((1.1, 1.5)),
            },
        }
    }

    /// The Nusselt number at Péclet number `peclet`.
    pub fn nusselt(&self, peclet: Ratio) -> Ratio {
        match *self {
            Self::LyonMartinelli => lyon_martinelli_nusselt(peclet),
            Self::SebanShimazaki => seban_shimazaki_nusselt(peclet),
            Self::KazimiCarelli { pitch_to_diameter } => {
                kazimi_carelli_nusselt(peclet, pitch_to_diameter)
            }
            Self::SchadModified { pitch_to_diameter } => {
                schad_modified_nusselt(peclet, pitch_to_diameter)
            }
            Self::Mikityuk { pitch_to_diameter } => mikityuk_nusselt(peclet, pitch_to_diameter),
        }
    }

    fn pitch_to_diameter(&self) -> Option<f64> {
        match self {
            Self::LyonMartinelli | Self::SebanShimazaki => None,
            Self::KazimiCarelli { pitch_to_diameter }
            | Self::SchadModified { pitch_to_diameter }
            | Self::Mikityuk { pitch_to_diameter } => Some(pitch_to_diameter.get::<ratio>()),
        }
    }
}

/// A liquid-metal correlation with its Reynolds and bulk Prandtl numbers,
/// for [`super::enums::NusseltCorrelation::LiquidMetal`]. `Pe = Re · Pr_bulk`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LiquidMetalNusseltData {
    /// Which correlation.
    pub correlation: LiquidMetalCorrelation,
    /// Reynolds number on the hydraulic diameter; finite, non-negative.
    pub reynolds: Ratio,
    /// Bulk-fluid Prandtl number; finite, positive.
    pub prandtl_bulk: Ratio,
}

impl LiquidMetalNusseltData {
    /// `Pe = Re · Pr_bulk`.
    pub fn peclet(&self) -> Ratio {
        self.reynolds * self.prandtl_bulk
    }

    /// The Nusselt number, after checking `Re ≥ 0` and `Pr > 0` are finite.
    /// The validity range is not enforced (as upstream); see
    /// [`Self::is_within_validity_range`].
    pub fn try_get_nusselt(&self) -> Result<Ratio, TuasLibError> {
        let re = self.reynolds.get::<ratio>();
        let pr = self.prandtl_bulk.get::<ratio>();
        if !re.is_finite() || re < 0.0 {
            return Err(TuasLibError::CorrelationRangeError {
                parameter: "Reynolds number".to_string(),
                value: re,
                lower_bound: 0.0,
                upper_bound: f64::INFINITY,
                units: "dimensionless".to_string(),
            });
        }
        if !pr.is_finite() || pr <= 0.0 {
            return Err(TuasLibError::CorrelationRangeError {
                parameter: "Prandtl number".to_string(),
                value: pr,
                lower_bound: 0.0,
                upper_bound: f64::INFINITY,
                units: "dimensionless".to_string(),
            });
        }
        Ok(self.correlation.nusselt(self.peclet()))
    }

    /// Whether `Pe` (unclamped) and `P/D` lie inside the upstream validity
    /// range; bounds the upstream does not state are not checked.
    pub fn is_within_validity_range(&self) -> bool {
        let range = self.correlation.validity_range();
        let pe = self.peclet().get::<ratio>();
        let pe_ok = range.peclet_min.is_none_or(|lo| pe >= lo)
            && range.peclet_max.is_none_or(|hi| pe <= hi);
        let pd_ok = match (range.pitch_to_diameter, self.correlation.pitch_to_diameter()) {
            (Some((lo, hi)), Some(x)) => x >= lo && x <= hi,
            _ => true,
        };
        pe_ok && pd_ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heat_transfer_correlations::nusselt_number_correlations::enums::NusseltCorrelation;

    fn r(x: f64) -> Ratio {
        Ratio::new::<ratio>(x)
    }

    /// Verification against hand evaluation of each published formula.
    ///
    /// Methodology: each value below was computed from the formula in the
    /// module table with a separate Python 3 one-liner (2026-11-10), not from
    /// this code, at `Pe = 200` and `P/D = 1.25` (inside every rod-bundle
    /// range), plus Schad's low-Pe branch at `Pe = 100`. Digits as printed
    /// there, rounded to six significant figures:
    ///
    /// - `200^0.8 = 69.3145`: Lyon `7 + 0.025·69.3145 = 8.73286`;
    ///   Seban-Shimazaki `5 + 1.73286 = 6.73286`.
    /// - Kazimi-Carelli: `1.25^3.8 = 2.33484`, `2^0.86 = 1.81504`,
    ///   `1.25^5 = 3.05176`: `4 + 0.33·2.33484·1.81504 + 0.16·3.05176 =
    ///   5.88677`.
    /// - Schad: `f(1.25) = −16.15 + 31.2 − 13.359375 = 1.690625`;
    ///   `200^0.3 = 4.90127`: `8.28622`; at `Pe = 100`: `4.496·1.690625 = 7.60105`.
    /// - Mikityuk: `1 − e^(−0.95) = 1 − 0.386741 = 0.613259`;
    ///   `200^0.77 = 59.1280`: `0.047·0.613259·309.1280 = 8.91005`.
    ///
    /// Pass criterion: relative difference ≤ 1e-5 (the reference values carry
    /// six significant figures). Result 2026-11-10: all six pass (asserted).
    /// (A first set of values worked out mentally had three of these wrong in
    /// the third or fourth figure; they were replaced by the Python values
    /// before the test was first run.)
    #[test]
    fn each_correlation_matches_its_hand_computed_value() {
        let pe = r(200.0);
        let x = r(1.25);
        let cases = [
            ("Lyon", lyon_martinelli_nusselt(pe), 8.73286),
            ("Seban-Shimazaki", seban_shimazaki_nusselt(pe), 6.73286),
            ("Kazimi-Carelli", kazimi_carelli_nusselt(pe, x), 5.88677),
            ("Schad Pe=200", schad_modified_nusselt(pe, x), 8.28622),
            ("Schad Pe=100", schad_modified_nusselt(r(100.0), x), 7.60105),
            ("Mikityuk", mikityuk_nusselt(pe, x), 8.91005),
        ];
        for (name, got, hand) in cases {
            let got = got.get::<ratio>();
            println!("{name}: code {got:.6}, python {hand}");
            approx::assert_relative_eq!(got, hand, max_relative = 1e-5);
        }
    }

    /// MOOSE's `max(1, Pe)` clamp: `Pe = 0` and `Pe = 1` give the same,
    /// finite Nusselt number (Lyon: 7.025).
    #[test]
    fn peclet_below_one_is_clamped_to_one_as_upstream() {
        assert_eq!(lyon_martinelli_nusselt(r(0.0)), lyon_martinelli_nusselt(r(1.0)));
        approx::assert_relative_eq!(lyon_martinelli_nusselt(r(0.0)).get::<ratio>(), 7.025);
        assert_eq!(mikityuk_nusselt(r(0.5), r(1.2)), mikityuk_nusselt(r(1.0), r(1.2)));
    }

    /// Schad's two branches meet at `Pe = 150` to the 4 significant figures
    /// of the 4.496 constant (`150^0.3 = 4.49601`).
    #[test]
    fn schad_branches_meet_at_peclet_150() {
        let x = r(1.2);
        let below = schad_modified_nusselt(r(149.999_999), x).get::<ratio>();
        let at = schad_modified_nusselt(r(150.0), x).get::<ratio>();
        approx::assert_relative_eq!(below, at, max_relative = 1e-4);
    }

    /// The enum, the data struct and `NusseltCorrelation::LiquidMetal` all
    /// reach the same function, `Pe` is `Re·Pr`, the validity check follows
    /// the table, and bad inputs error.
    #[test]
    fn data_struct_and_nusselt_correlation_dispatch() {
        let x = r(1.25);
        let data = LiquidMetalNusseltData {
            correlation: LiquidMetalCorrelation::Mikityuk { pitch_to_diameter: x },
            reynolds: r(40_000.0),
            prandtl_bulk: r(0.005),
        };
        approx::assert_relative_eq!(data.peclet().get::<ratio>(), 200.0, max_relative = 1e-12);
        let direct = mikityuk_nusselt(r(200.0), x);
        approx::assert_relative_eq!(
            data.try_get_nusselt().unwrap().get::<ratio>(),
            direct.get::<ratio>(),
            max_relative = 1e-12
        );
        assert!(data.is_within_validity_range());

        let correlation = NusseltCorrelation::LiquidMetal(data);
        approx::assert_relative_eq!(
            correlation.try_get_nusselt().unwrap().get::<ratio>(),
            direct.get::<ratio>(),
            max_relative = 1e-12
        );
        // The estimate_* routes replace Re and Pr: Re 20000 x Pr 0.01 = Pe 200.
        let estimated = correlation
            .estimate_based_on_prandtl_and_reynolds_no_wall_correction(r(0.01), r(20_000.0))
            .unwrap();
        approx::assert_relative_eq!(estimated.get::<ratio>(), direct.get::<ratio>(), max_relative = 1e-12);
        let estimated_wall = correlation
            .estimate_based_on_prandtl_reynolds_and_wall_correction(r(0.01), r(0.02), r(20_000.0))
            .unwrap();
        approx::assert_relative_eq!(estimated_wall.get::<ratio>(), direct.get::<ratio>(), max_relative = 1e-12);
        let estimated_darcy = correlation
            .estimate_based_on_prandtl_darcy_and_reynolds_no_wall_correction(r(0.01), r(0.02), r(20_000.0))
            .unwrap();
        approx::assert_relative_eq!(estimated_darcy.get::<ratio>(), direct.get::<ratio>(), max_relative = 1e-12);
        let estimated_darcy_wall = correlation
            .estimate_based_on_prandtl_darcy_and_reynolds_wall_correction(
                r(0.01),
                r(0.02),
                r(0.02),
                r(20_000.0),
            )
            .unwrap();
        approx::assert_relative_eq!(
            estimated_darcy_wall.get::<ratio>(),
            direct.get::<ratio>(),
            max_relative = 1e-12
        );

        // Outside the range: P/D 1.6 for Mikityuk, Pe 2000 for Schad.
        let out_pd = LiquidMetalNusseltData {
            correlation: LiquidMetalCorrelation::Mikityuk { pitch_to_diameter: r(1.6) },
            ..data
        };
        assert!(!out_pd.is_within_validity_range());
        let out_pe = LiquidMetalNusseltData {
            correlation: LiquidMetalCorrelation::SchadModified { pitch_to_diameter: x },
            reynolds: r(400_000.0),
            prandtl_bulk: r(0.005),
        };
        assert!(!out_pe.is_within_validity_range());
        let tube = LiquidMetalNusseltData {
            correlation: LiquidMetalCorrelation::LyonMartinelli,
            ..out_pe
        };
        assert!(tube.is_within_validity_range());
        assert_eq!(LiquidMetalCorrelation::SebanShimazaki.validity_range().peclet_min, None);
        approx::assert_relative_eq!(
            LiquidMetalCorrelation::KazimiCarelli { pitch_to_diameter: x }
                .nusselt(r(200.0))
                .get::<ratio>(),
            5.88677,
            max_relative = 1e-5
        );
        approx::assert_relative_eq!(
            LiquidMetalCorrelation::SebanShimazaki.nusselt(r(200.0)).get::<ratio>(),
            6.73286,
            max_relative = 1e-5
        );

        let bad = LiquidMetalNusseltData { reynolds: r(-1.0), ..data };
        assert!(bad.try_get_nusselt().is_err());
        let bad = LiquidMetalNusseltData { prandtl_bulk: r(0.0), ..data };
        assert!(bad.try_get_nusselt().is_err());
    }
}
