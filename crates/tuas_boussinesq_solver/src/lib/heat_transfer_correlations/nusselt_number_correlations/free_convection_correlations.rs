//! Free (natural) convection Nusselt-number correlations, with the Grashof
//! and Rayleigh numbers they are built on.
//!
//! Added for GitHub issue #1063 (the RELAP5 geometry-101/110 single-phase
//! heat-transfer selection, which takes the maximum of forced-laminar,
//! forced-turbulent and free-convection coefficients; RELAP-7 Theory Manual
//! §2.2.2.1, Eqs 47–48). Searched before writing (2026-11-01): no
//! Churchill–Chu correlation existed in the workspace; the nearest relative,
//! `sembawang::bwr_mark1::pool_surface::structure_surface` (moved on
//! 2026-11-01 to `tampines::components::wall_condensation::structure_surface`,
//! gh:#1096, and re-exported from `sembawang`), is MELCOR's
//! piecewise `0.59 Ra^1/4` / `0.10 Ra^1/3` external-surface form, a
//! different correlation.
//!
//! > Research, education and V&V only. AI draft, unreviewed.

use petir::mathf::RealMath;
use uom::si::acceleration::meter_per_second_squared;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::*;
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::ratio::ratio;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::temperature_interval::kelvin as kelvin_interval;

use crate::tuas_lib_error::TuasLibError;

/// Returns an error unless `value` is finite and at least `lower_bound`
/// (or strictly above it when `strict`).
fn check_finite_lower_bound(
    parameter: &str,
    value: f64,
    lower_bound: f64,
    strict: bool,
) -> Result<(), TuasLibError> {
    let below = if strict {
        value <= lower_bound
    } else {
        value < lower_bound
    };
    if !value.is_finite() || below {
        return Err(TuasLibError::CorrelationRangeError {
            parameter: parameter.to_string(),
            value,
            lower_bound,
            upper_bound: f64::INFINITY,
            units: "dimensionless".to_string(),
        });
    }
    Ok(())
}

/// Grashof number `Gr = ρ² g β |T_w − T| L³ / μ²`.
///
/// The form is RELAP-7 Theory Manual §2.2.2.1, Eq. 48 (RELAP5's), which is
/// the textbook definition (e.g. Incropera & DeWitt, *Fundamentals of Heat
/// and Mass Transfer*, Eq. 9.12, written with `ν = μ/ρ`). The absolute value
/// of the wall-to-bulk difference is taken so that the same buoyancy
/// magnitude results for a heated and a cooled wall; the free-convection
/// correlations here are written for `Gr ≥ 0`.
///
/// Evaluated in SI floats from the `uom` inputs (so no unit can be mixed
/// up), returned as a [`Ratio`].
///
/// Hand check (pinned in `tests::grashof_number_matches_hand_calculation`):
/// water near 20 °C, `ρ = 998.2 kg/m³`, `β = 2.07e-4 1/K`,
/// `μ = 1.002e-3 Pa s`, `ΔT = 10 K`, `L = 0.1 m`, `g = 9.80665 m/s²`:
/// `Gr = 998.2² × 9.80665 × 2.07e-4 × 10 × 0.1³ / (1.002e-3)² = 2.0146e7`.
pub fn grashof_number(
    density: MassDensity,
    gravity: Acceleration,
    thermal_expansion_coefficient: TemperatureCoefficient,
    wall_minus_bulk_temperature: TemperatureInterval,
    length_scale: Length,
    dynamic_viscosity: DynamicViscosity,
) -> Ratio {
    let rho = density.get::<kilogram_per_cubic_meter>();
    let g = gravity.get::<meter_per_second_squared>();
    let beta = thermal_expansion_coefficient.get::<per_kelvin>();
    let delta_t = wall_minus_bulk_temperature.get::<kelvin_interval>().abs();
    let l = length_scale.get::<meter>();
    let mu = dynamic_viscosity.get::<pascal_second>();
    Ratio::new::<ratio>(rho * rho * g * beta * delta_t * l * l * l / (mu * mu))
}

/// Rayleigh number `Ra = Gr Pr`.
pub fn rayleigh_number(grashof: Ratio, prandtl: Ratio) -> Ratio {
    grashof * prandtl
}

/// Churchill–Chu correlation for free convection along a vertical flat
/// plate, valid over the whole Rayleigh range (laminar and turbulent):
///
/// ```text
/// Nu_L = { 0.825 + 0.387 Ra^(1/6) / [1 + (0.492/Pr)^(9/16)]^(8/27) }²
/// ```
///
/// Source: S. W. Churchill and H. H. S. Chu (1975), "Correlating equations
/// for laminar and turbulent free convection from a vertical plate",
/// *International Journal of Heat and Mass Transfer* **18**(11), 1323–1329,
/// doi:10.1016/0017-9310(75)90243-4 (their Eq. for the whole range,
/// recommended for `10⁻¹ < Ra < 10¹²`). Used by RELAP5 and RELAP-7 for the
/// free-convection leg of the single-phase wall heat-transfer selection
/// (RELAP-7 Theory Manual §2.2.2.1, Eq. 47).
///
/// Inputs are plain floats, like the rest of
/// [`super::pipe_correlations`]; the `uom` entry point is
/// [`ChurchillChuData::try_get_nusselt`]. The Nusselt and Rayleigh numbers
/// are on the same length scale `L` (the plate height; RELAP5 defaults it to
/// the heat-transfer hydraulic diameter).
///
/// Hand checks (pinned in this module's tests):
///
/// - `Ra = 0`: `Nu = 0.825² = 0.680625` exactly (the conduction limit of the
///   correlation, independent of `Pr`).
/// - `Ra = 1e9, Pr = 0.71` (air): `(0.492/0.71)^(9/16) = 0.81376`,
///   `ψ = 1.81376^(8/27) = 1.19290`, `Ra^(1/6) = 31.6228`,
///   `Nu = (0.825 + 0.387 × 31.6228 / 1.19290)² = (11.0841)² = 122.857`.
/// - `Ra = 1e6, Pr = 7` (water): `Nu = 19.9767`.
///
/// ```rust
/// use tuas_boussinesq_solver::heat_transfer_correlations::
/// nusselt_number_correlations::free_convection_correlations::
/// churchill_chu_vertical_plate_nusselt;
///
/// let nu = churchill_chu_vertical_plate_nusselt(0.0, 1.0);
/// approx::assert_relative_eq!(nu, 0.680625, max_relative = 1e-12);
/// ```
pub fn churchill_chu_vertical_plate_nusselt(rayleigh_number: f64, prandtl_number: f64) -> f64 {
    let psi = (1.0 + (0.492 / prandtl_number).r_powf(9.0 / 16.0)).r_powf(8.0 / 27.0);
    let root = 0.825 + 0.387 * rayleigh_number.r_powf(1.0 / 6.0) / psi;
    root * root
}

/// Inputs to [`churchill_chu_vertical_plate_nusselt`], as dimensionless
/// `uom` ratios. Holds the Grashof number rather than the Rayleigh number,
/// so that replacing the Prandtl number (as
/// [`super::enums::NusseltCorrelation`]'s `estimate_*` methods do) keeps
/// `Ra = Gr Pr` consistent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChurchillChuData {
    /// Grashof number on the free-convection length scale (see
    /// [`grashof_number`]); must be finite and non-negative.
    pub grashof: Ratio,
    /// Fluid Prandtl number; must be finite and positive.
    pub prandtl: Ratio,
}

impl ChurchillChuData {
    /// Churchill–Chu vertical-plate Nusselt number, after checking that the
    /// Grashof number is finite and non-negative and the Prandtl number
    /// finite and positive. No upper Rayleigh bound is enforced (the
    /// published recommendation is `Ra < 10¹²`): RELAP5 evaluates the
    /// correlation everywhere it takes the maximum.
    pub fn try_get_nusselt(&self) -> Result<Ratio, TuasLibError> {
        let gr = self.grashof.get::<ratio>();
        let pr = self.prandtl.get::<ratio>();
        check_finite_lower_bound("Grashof number", gr, 0.0, false)?;
        check_finite_lower_bound("Prandtl number", pr, 0.0, true)?;
        Ok(Ratio::new::<ratio>(churchill_chu_vertical_plate_nusselt(
            gr * pr,
            pr,
        )))
    }
}

/// V&V pins for the free-convection correlations (GitHub #1063).
///
/// **Methodology.** Each correlation is evaluated at points computed by
/// hand from the source paper's equation (calculator arithmetic shown in
/// each test's doc comment; independently re-evaluated in IEEE double with
/// the platform `pow`), and compared at `max_relative = 1e-10` (PETIR's
/// `r_powf` against the platform `pow` differs at most by round-off).
/// Analytic limits are pinned exactly.
///
/// **Predictions (written 2026-11-01, before the first run).** All pins
/// agree to better than 1e-12 relative; the `Ra = 0` limit is 0.680625 to
/// round-off.
///
/// **Results (2026-11-01, `cargo test --release -p tuas_boussinesq_solver
/// --lib free_convection`, toolchain 1.99.1).** 6 of 6 pass on the first
/// run. With every `1e-10` tolerance temporarily tightened to `1e-15` they
/// still pass (agreement to a few ulp). Table:
/// `crates/tampines/docs/relap5-htc-dispatcher-vv.md`.
#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::acceleration::meter_per_second_squared;

    /// `Ra = 0` gives `0.825²` whatever the Prandtl number: the
    /// correlation's conduction limit.
    #[test]
    fn churchill_chu_zero_rayleigh_is_0_825_squared() {
        for pr in [0.01, 0.71, 7.0, 1000.0] {
            approx::assert_relative_eq!(
                churchill_chu_vertical_plate_nusselt(0.0, pr),
                0.680625,
                max_relative = 1e-14
            );
        }
    }

    /// Air, `Ra = 1e9, Pr = 0.71`: hand value `122.857` (calculator,
    /// module doc); IEEE re-evaluation `122.85653487620696`.
    /// Water, `Ra = 1e6, Pr = 7`: IEEE re-evaluation `19.976669857906355`.
    #[test]
    fn churchill_chu_matches_hand_calculation() {
        approx::assert_relative_eq!(
            churchill_chu_vertical_plate_nusselt(1e9, 0.71),
            122.85653487620696,
            max_relative = 1e-10
        );
        approx::assert_relative_eq!(
            churchill_chu_vertical_plate_nusselt(1e9, 0.71),
            122.857,
            max_relative = 1e-5
        );
        approx::assert_relative_eq!(
            churchill_chu_vertical_plate_nusselt(1e6, 7.0),
            19.976669857906355,
            max_relative = 1e-10
        );
    }

    /// `Pr → ∞`: the Prandtl function `ψ → 1`, so
    /// `Nu → (0.825 + 0.387 Ra^(1/6))²`; at `Ra = 1e6` that is
    /// `(0.825 + 3.87)² = 22.043025`. At `Pr = 1e12`,
    /// `(0.492e-12)^(9/16) ≈ 2.3e-7`, so the residual is ~1e-7 relative.
    #[test]
    fn churchill_chu_large_prandtl_limit() {
        approx::assert_relative_eq!(
            churchill_chu_vertical_plate_nusselt(1e6, 1e12),
            22.043025,
            max_relative = 1e-6
        );
    }

    /// The `uom` entry point forms `Ra = Gr Pr` and rejects unphysical
    /// inputs.
    #[test]
    fn churchill_chu_data_forms_rayleigh_and_rejects_bad_inputs() {
        let data = ChurchillChuData {
            grashof: Ratio::new::<ratio>(1e9 / 0.71),
            prandtl: Ratio::new::<ratio>(0.71),
        };
        approx::assert_relative_eq!(
            data.try_get_nusselt().unwrap().get::<ratio>(),
            122.85653487620696,
            max_relative = 1e-10
        );
        let negative_gr = ChurchillChuData {
            grashof: Ratio::new::<ratio>(-1.0),
            ..data
        };
        assert!(negative_gr.try_get_nusselt().is_err());
        let zero_pr = ChurchillChuData {
            prandtl: Ratio::new::<ratio>(0.0),
            ..data
        };
        assert!(zero_pr.try_get_nusselt().is_err());
        let nan_gr = ChurchillChuData {
            grashof: Ratio::new::<ratio>(f64::NAN),
            ..data
        };
        assert!(nan_gr.try_get_nusselt().is_err());
        approx::assert_relative_eq!(
            rayleigh_number(data.grashof, data.prandtl).get::<ratio>(),
            1e9,
            max_relative = 1e-14
        );
    }

    /// The two new `NusseltCorrelation` variants (GitHub #1063) through
    /// every dispatch method.
    ///
    /// Dittus–Boelter, `Re = 1e4, Pr = 7`, by hand: `Re^0.8 = 1584.893`,
    /// `Pr^0.4 = 2.177906`, `Pr^0.3 = 1.793932`, so heating
    /// `Nu = 0.023 × 1584.893 × 2.177906 = 79.390` and cooling
    /// `Nu = 0.023 × 1584.893 × 1.793932 = 65.352` (IEEE re-evaluation
    /// `79.39022851754193`, `65.35175396357931`).
    ///
    /// Prediction: the variant reproduces the existing free function
    /// `dittus_boelter_correlation` bit for bit (it delegates to it), and
    /// the `estimate_*` methods replace Re and Pr.
    #[test]
    fn new_nusselt_correlation_variants_dispatch() {
        use super::super::enums::NusseltCorrelation;
        use super::super::input_structs::{DittusBoelterData, FluidHeatingDirection};
        use super::super::pipe_correlations::dittus_boelter_correlation;

        let heated = DittusBoelterData {
            reynolds: Ratio::new::<ratio>(1e4),
            prandtl_bulk: Ratio::new::<ratio>(7.0),
            heating_direction: FluidHeatingDirection::FluidHeated,
        };
        let cooled = DittusBoelterData {
            heating_direction: FluidHeatingDirection::FluidCooled,
            ..heated
        };
        let nu = |c: NusseltCorrelation| c.try_get_nusselt().unwrap().get::<ratio>();
        approx::assert_relative_eq!(
            nu(NusseltCorrelation::DittusBoelter(heated)),
            79.39022851754193,
            max_relative = 1e-10
        );
        approx::assert_relative_eq!(
            nu(NusseltCorrelation::DittusBoelter(cooled)),
            65.35175396357931,
            max_relative = 1e-10
        );
        assert_eq!(
            nu(NusseltCorrelation::DittusBoelter(heated)),
            dittus_boelter_correlation(1e4, 7.0, true)
        );

        // estimate_*: Re and Pr replaced; same result as building fresh.
        let db = NusseltCorrelation::DittusBoelter(DittusBoelterData {
            reynolds: Ratio::new::<ratio>(1.0),
            prandtl_bulk: Ratio::new::<ratio>(1.0),
            ..heated
        });
        let pr7 = Ratio::new::<ratio>(7.0);
        let re = Ratio::new::<ratio>(1e4);
        let f = Ratio::new::<ratio>(0.03);
        let expected = 79.39022851754193;
        for got in [
            db.estimate_based_on_prandtl_and_reynolds_no_wall_correction(pr7, re),
            db.estimate_based_on_prandtl_reynolds_and_wall_correction(pr7, pr7, re),
            db.estimate_based_on_prandtl_darcy_and_reynolds_no_wall_correction(pr7, f, re),
            db.estimate_based_on_prandtl_darcy_and_reynolds_wall_correction(pr7, pr7, f, re),
        ] {
            approx::assert_relative_eq!(got.unwrap().get::<ratio>(), expected, max_relative = 1e-10);
        }
        let bad = DittusBoelterData {
            reynolds: Ratio::new::<ratio>(-1.0),
            ..heated
        };
        assert!(NusseltCorrelation::DittusBoelter(bad).try_get_nusselt().is_err());
        let bad_pr = DittusBoelterData {
            prandtl_bulk: Ratio::new::<ratio>(f64::INFINITY),
            ..heated
        };
        assert!(bad_pr.try_get_nusselt().is_err());

        // Churchill–Chu: Gr held, Pr replaced, Re ignored.
        let cc = NusseltCorrelation::ChurchillChuVerticalPlate(ChurchillChuData {
            grashof: Ratio::new::<ratio>(1e9 / 0.71),
            prandtl: Ratio::new::<ratio>(5.0),
        });
        let pr = Ratio::new::<ratio>(0.71);
        for got in [
            cc.estimate_based_on_prandtl_and_reynolds_no_wall_correction(pr, re),
            cc.estimate_based_on_prandtl_reynolds_and_wall_correction(pr, pr7, re),
            cc.estimate_based_on_prandtl_darcy_and_reynolds_wall_correction(pr, pr7, f, re),
        ] {
            approx::assert_relative_eq!(
                got.unwrap().get::<ratio>(),
                122.85653487620696,
                max_relative = 1e-10
            );
        }
        assert!(nu(cc) > 122.86);
    }

    /// Water near 20 °C over a 0.1 m length at 10 K: hand value
    /// `Gr = 998.2² × 9.80665 × 2.07e-4 × 10 × 0.1³ / (1.002e-3)²
    /// = 2.0146087e7`; IEEE re-evaluation `20146087.18236204`. The sign of
    /// the temperature difference does not matter.
    #[test]
    fn grashof_number_matches_hand_calculation() {
        let gr = |dt: f64| {
            grashof_number(
                MassDensity::new::<kilogram_per_cubic_meter>(998.2),
                Acceleration::new::<meter_per_second_squared>(9.80665),
                TemperatureCoefficient::new::<per_kelvin>(2.07e-4),
                TemperatureInterval::new::<kelvin_interval>(dt),
                Length::new::<meter>(0.1),
                DynamicViscosity::new::<pascal_second>(1.002e-3),
            )
            .get::<ratio>()
        };
        approx::assert_relative_eq!(gr(10.0), 20146087.18236204, max_relative = 1e-12);
        approx::assert_relative_eq!(gr(-10.0), gr(10.0), max_relative = 1e-15);
    }
}
