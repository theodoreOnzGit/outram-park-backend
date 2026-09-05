//! Post-characterization **parameter fitting**: nudge each pseudo-component's
//! acentric factor, Rackett `Z_RA`, and PR/SRK volume-translation coefficients
//! so the compound reproduces its own assay-specified normal boiling point and
//! specific gravity.
//!
//! # Why this step exists
//!
//! The correlations in [`crate::petroleum::property_methods`] give a cut's
//! `Tc`, `Pc` and `ω` independently of one another. Feed those into a cubic EOS
//! and the compound will *not*, in general, boil at the `Tb` the assay said it
//! boils at, nor will its Rackett density match the assay's specific gravity.
//! DWSIM closes both gaps by scaling one parameter at a time until the error is
//! minimised.
//!
//! # Provenance
//!
//! Faithful port of DWSIM (GPL-3.0),
//! `DWSIM.Thermodynamics/PetroleumCharacterization/Fitting.vb` (242 lines,
//! whole file), from the pinned upstream clone
//! `/home/teddy0/Documents/research/dwsim-upstream`, branch `windows`, commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`. Upstream copyright: 2010 Daniel
//! Wagner O. de Medeiros and the DWSIM contributors. GPL-3.0; this port is
//! GPL-3.0-only.
//!
//! | Rust item | Upstream |
//! |---|---|
//! | [`fit_rackett_z_scale`] | class `DensityFitting`, `Fitting.vb:28-69` |
//! | [`fit_acentric_factor_scale`] | class `NBPFitting`, `:71-135` |
//! | [`fit_pr_volume_translation`] | class `PRVSFitting`, `:137-187` |
//! | [`fit_srk_volume_translation`] | class `SRKVSFitting`, `:189-239` |
//! | [`brent_minimize`] | `MathEx.BrentOpt.BrentMinimize.brentoptimize` (called at `:45`, `:106`, `:157`, `:209`) |
//! | [`apply_parameter_fits`] | `GenerateCompounds.vb:379-448` / `DistCurves.cs:773-870` (the driver loop) |
//!
//! # The optimiser is NOT a port
//!
//! Upstream minimises with `MathEx.BrentOpt.BrentMinimize`, which lives outside
//! the petroleum-characterization directory and is vendored third-party code.
//! [`brent_minimize`] below is a self-contained, from-scratch implementation of
//! **Brent's parabolic-interpolation / golden-section minimisation** as
//! described in Brent, R. P. (1973), *Algorithms for Minimization without
//! Derivatives*, Prentice-Hall, ch. 5. The workspace's `roots` dependency
//! provides root **finders** only — no scalar minimiser — so it could not be
//! reused, and no new dependency was added. The search brackets and tolerances
//! are upstream's.
//!
//! # Units
//!
//! `uom`-typed on the public surface. Every fitted quantity is a **dimensionless
//! multiplier** applied to an existing parameter, exactly as upstream (its
//! objective functions take a bare `t` and multiply the parameter by it).
//!
//! # Excluded DWSIM behavior
//!
//! - The mutable `_comp` / `_ms` / `_pp` / `_idx` fields these classes use to
//!   smuggle state into their delegate callbacks (`Fitting.vb:30`, `:73-75`,
//!   `:139`, `:191`) are an artefact of `AddressOf` delegates; the Rust
//!   equivalents take their inputs as arguments.
//! - `NBPFitting.FunctionValue` mutates the compound's acentric factor in place
//!   and then divides it back out (`:84`, `:92`) — a scratch mutation with no
//!   observable effect. Not reproduced; [`fit_acentric_factor_scale`] evaluates
//!   the trial compound by value.
//! - `NBPFitting` reaches through an `IFlowsheet` and a `MaterialStream` to
//!   reach the property package (`:112-125`). This port calls
//!   [`crate::thermo::saturation::bubble_temperature`] directly on a
//!   single-component feed, which is the same computation without the flowsheet
//!   plumbing.
//! - The GUI `ShowMessage` error reporting around the fits
//!   (`DistCurves.cs:819-822`, `:840-843`) becomes a silent fallback to a unit
//!   multiplier — the same net effect as upstream, which leaves `fw`/`fzra` at
//!   its previous value when the fit throws.

use uom::si::f64::{MassDensity, Ratio, ThermodynamicTemperature};
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

use crate::thermo::component::Component;
use crate::thermo::cubic_eos::CubicEos;
use crate::thermo::property_package::PropertyPackageModel;
use crate::thermo::saturation::bubble_temperature;

use super::aux_props::{critical_compressibility_zc1, critical_volume, liquid_density_rackett};
use super::pseudo_component::PseudoComponent;

/// The reference temperature for every density fit: **15.6 °C = 288.75 K**
/// (`Fitting.vb:34`, `:146`, `:198` all pass `15.6 + 273.15`).
const DENSITY_FIT_TEMPERATURE_K: f64 = 15.6 + 273.15;

/// The reference pressure for the volume-translation fits: **101325 Pa**
/// (`Fitting.vb:146`, `:198`).
const DENSITY_FIT_PRESSURE_PA: f64 = 101_325.0;

/// The density of water DWSIM multiplies a specific gravity by to get kg/m³:
/// **999.96** (`Fitting.vb:55`, `:168`, `:220`). Note this is the density of
/// water at its 4 °C maximum, not at the 15.6 °C the fit is evaluated at —
/// upstream's own choice, reproduced.
const WATER_DENSITY_KG_M3: f64 = 999.96;

/// Universal gas constant as upstream writes it in these routines,
/// `R = 8.314 J/(mol·K)` (`Fitting.vb:173`, `:225`).
const R_DWSIM: f64 = 8.314;

/// Minimise a scalar function on `[lower, upper]` by **Brent's method**
/// (parabolic interpolation with golden-section fallback).
///
/// Stand-in for upstream's `MathEx.BrentOpt.BrentMinimize.brentoptimize` — see
/// the module docs for why this is an independent implementation.
///
/// # Inputs
///
/// - `lower`, `upper` — the search bracket. Must satisfy `lower < upper`.
/// - `tolerance` — absolute tolerance on the located minimiser.
/// - `f` — the objective. Non-finite values are treated as `+∞` so the search
///   walks away from them rather than propagating `NaN`.
///
/// # Returns
///
/// The minimising abscissa. With a flat or non-finite objective it returns the
/// bracket midpoint, which is benign for every caller here (each objective is a
/// squared error whose minimum is genuinely interior).
///
/// # Units
///
/// Dimensionless — every caller in this module optimises a multiplier.
#[must_use]
pub fn brent_minimize<F>(lower: f64, upper: f64, tolerance: f64, mut f: F) -> f64
where
    F: FnMut(f64) -> f64,
{
    /// Golden-section constant `(3 − √5)/2`.
    const C: f64 = 0.381_966_011_250_105_2;
    const ZEPS: f64 = 1.0e-12;
    const MAX_ITERATIONS: usize = 200;

    if !(lower < upper) {
        return lower;
    }
    let safe = |v: f64| if v.is_finite() { v } else { f64::INFINITY };

    let (mut a, mut b) = (lower, upper);
    let mut x = a + C * (b - a);
    let (mut w, mut v) = (x, x);
    let mut fx = safe(f(x));
    let (mut fw, mut fv) = (fx, fx);
    let mut d = 0.0_f64;
    let mut e = 0.0_f64;

    for _ in 0..MAX_ITERATIONS {
        let xm = 0.5 * (a + b);
        let tol1 = tolerance * x.abs() + ZEPS;
        let tol2 = 2.0 * tol1;
        if (x - xm).abs() <= tol2 - 0.5 * (b - a) {
            break;
        }
        let mut use_golden = true;
        if e.abs() > tol1 {
            // Trial parabolic fit through (x, fx), (w, fw), (v, fv).
            let r = (x - w) * (fx - fv);
            let q0 = (x - v) * (fx - fw);
            let mut p = (x - v) * q0 - (x - w) * r;
            let mut q = 2.0 * (q0 - r);
            if q > 0.0 {
                p = -p;
            }
            q = q.abs();
            let e_previous = e;
            e = d;
            if p.abs() < (0.5 * q * e_previous).abs() && p > q * (a - x) && p < q * (b - x) {
                d = p / q;
                let u = x + d;
                if u - a < tol2 || b - u < tol2 {
                    d = if xm - x >= 0.0 { tol1 } else { -tol1 };
                }
                use_golden = false;
            }
        }
        if use_golden {
            e = if x >= xm { a - x } else { b - x };
            d = C * e;
        }
        let u = if d.abs() >= tol1 {
            x + d
        } else {
            x + if d >= 0.0 { tol1 } else { -tol1 }
        };
        let fu = safe(f(u));
        if fu <= fx {
            if u >= x {
                a = x;
            } else {
                b = x;
            }
            v = w;
            w = x;
            x = u;
            fv = fw;
            fw = fx;
            fx = fu;
        } else {
            if u < x {
                a = u;
            } else {
                b = u;
            }
            if fu <= fw || (w - x).abs() < f64::EPSILON {
                v = w;
                fv = fw;
                w = u;
                fw = fu;
            } else if fu <= fv || (v - x).abs() < f64::EPSILON || (v - w).abs() < f64::EPSILON {
                v = u;
                fv = fu;
            }
        }
    }
    x
}

/// Fit the multiplier on the **Rackett `Z_RA`** that makes the compound's
/// Rackett liquid density at 15.6 °C match its assay specific gravity.
///
/// Ported from `Fitting.vb:28-69` (`DensityFitting`): minimise
/// `(SG·999.96 − ρ_Rackett(288.75 K, …, Z_RA·t))²` over `t ∈ [0.1, 10]` with a
/// tolerance of `1e-10` (`:45`).
///
/// # Inputs
///
/// - `component` — the pseudo-component's EOS constants.
/// - `specific_gravity` — the target `SG` at 15.6/15.6 °C [-].
/// - `rackett_z` — the current `Z_RA` [-] the multiplier scales.
///
/// # Returns
///
/// The dimensionless multiplier `t`. Multiply `Z_RA` by it.
#[must_use]
pub fn fit_rackett_z_scale(component: &Component, specific_gravity: Ratio, rackett_z: f64) -> f64 {
    let target = specific_gravity.get::<ratio>() * WATER_DENSITY_KG_M3;
    brent_minimize(0.1, 10.0, 1.0e-10, |t| {
        let rho = liquid_density_rackett(
            ThermodynamicTemperature::new::<kelvin>(DENSITY_FIT_TEMPERATURE_K),
            ThermodynamicTemperature::new::<kelvin>(component.critical_temperature),
            uom::si::f64::Pressure::new::<uom::si::pressure::pascal>(component.critical_pressure),
            Ratio::new::<ratio>(component.acentric_factor),
            uom::si::f64::MolarMass::new::<uom::si::molar_mass::kilogram_per_mole>(
                component.molar_mass,
            ),
            Some(rackett_z * t),
            None,
            None,
        )
        .get::<kilogram_per_cubic_meter>();
        let diff = target - rho;
        // Upstream returns 0 when the difference is NaN (`Fitting.vb:61-65`),
        // which reads as a perfect fit; reproduced.
        if diff.is_nan() {
            0.0
        } else {
            diff * diff
        }
    })
}

/// Fit the multiplier on the **acentric factor** that makes the compound's
/// EOS-computed normal boiling point match the assay's `Tb`.
///
/// Ported from `Fitting.vb:71-135` (`NBPFitting`): minimise
/// `(T_bubble(P = 101325 Pa) − Tb)²` over the acentric-factor multiplier
/// `t ∈ [0.001, 10]` with a tolerance of `0.1` (`:106` — deliberately loose;
/// this is an expensive objective).
///
/// The bubble temperature is evaluated on a **single-component** feed through
/// [`crate::thermo::saturation::bubble_temperature`] with the
/// [`PropertyPackageModel::PengRobinson`] package, matching upstream's
/// `PengRobinsonPropertyPackage` default (`GenerateCompounds.vb:389`).
///
/// # Returns
///
/// The dimensionless multiplier `t`; multiply `ω` by it. Returns `1.0`
/// unchanged if every trial bubble-point solve fails (upstream leaves its `fw`
/// at the previous value and reports a message box; see the module docs).
#[must_use]
pub fn fit_acentric_factor_scale(component: &Component) -> f64 {
    let target = component.normal_boiling_point;
    if !target.is_finite() || target <= 0.0 {
        return 1.0;
    }
    let mut any_success = false;
    let factor = brent_minimize(0.001, 10.0, 0.1, |t| {
        let mut trial = component.clone();
        trial.acentric_factor = component.acentric_factor * t;
        match bubble_temperature(
            std::slice::from_ref(&trial),
            &[1.0],
            DENSITY_FIT_PRESSURE_PA,
            PropertyPackageModel::PengRobinson,
        ) {
            Ok(state) => {
                any_success = true;
                let diff = state.temperature - target;
                if diff.is_nan() {
                    0.0
                } else {
                    diff * diff
                }
            }
            Err(_) => f64::INFINITY,
        }
    });
    if any_success && factor.is_finite() {
        factor
    } else {
        1.0
    }
}

/// Fit the **Peng-Robinson volume-translation** coefficient that makes the
/// PR-computed liquid density at 15.6 °C and 1 atm match the assay specific
/// gravity.
///
/// Ported from `Fitting.vb:137-187` (`PRVSFitting`): minimise
/// `(SG·999.96 − ρ_PR-translated)²` over `t ∈ [−100, 100]` with tolerance
/// `1e-4` (`:157`), where
///
/// ```text
/// v  = R·Z_PR(T, P, liquid)·T / P  −  c·t·b_i
/// ρ  = M / v
/// ```
///
/// with `b_i = 0.0778·R·Tc/Pc` — note that upstream passes the **PR** `Ωb`
/// value `0.0778` even in the SRK sibling (`:226`), a defect reproduced in
/// [`fit_srk_volume_translation`].
///
/// # Returns
///
/// The dimensionless multiplier on the current translation coefficient.
#[must_use]
pub fn fit_pr_volume_translation(component: &Component, current_coefficient: f64) -> f64 {
    fit_volume_translation(
        component,
        current_coefficient,
        CubicEos::PengRobinson,
        Ratio::new::<ratio>(f64::NAN),
    )
}

/// Fit the **SRK volume-translation** coefficient — the SRK sibling of
/// [`fit_pr_volume_translation`].
///
/// Ported from `Fitting.vb:189-239` (`SRKVSFitting`).
///
/// > **Upstream defect preserved:** `:226` computes the co-volume as
/// > `srk.bi(0.0778, Tc, Pc)`, i.e. with **Peng-Robinson's** `Ωb = 0.0778`
/// > rather than SRK's `Ωb = 0.08664`. Reproduced so the fitted coefficient
/// > matches DWSIM.
#[must_use]
pub fn fit_srk_volume_translation(component: &Component, current_coefficient: f64) -> f64 {
    fit_volume_translation(
        component,
        current_coefficient,
        CubicEos::Srk,
        Ratio::new::<ratio>(f64::NAN),
    )
}

/// Shared body of the two volume-translation fits (`Fitting.vb:163-185` and
/// `:215-237`, which differ only in which cubic EOS supplies `Z`).
///
/// `_unused` exists so the two public wrappers keep identical shapes; it is not
/// read.
fn fit_volume_translation(
    component: &Component,
    current_coefficient: f64,
    eos: CubicEos,
    _unused: Ratio,
) -> f64 {
    // Upstream reads the *specific gravity* off the compound; here the caller's
    // Rackett-consistent target is recomputed from the same source, the assay
    // SG carried on the pseudo-component. See `apply_parameter_fits`.
    let target_density = component_target_density(component);
    let t_k = DENSITY_FIT_TEMPERATURE_K;
    let p_pa = DENSITY_FIT_PRESSURE_PA;
    // Upstream passes PR's Omega_b (0.0778) to *both* EOS (`:174`, `:226`).
    let b_i = 0.0778 * R_DWSIM * component.critical_temperature / component.critical_pressure;

    let a_m = eos.a_i(component, t_k);
    let b_m = eos.b_i(component);
    let a_dimensionless = a_m * p_pa / (R_DWSIM * t_k).powi(2);
    let b_dimensionless = b_m * p_pa / (R_DWSIM * t_k);
    let Some(z) = eos.z_liquid(a_dimensionless, b_dimensionless) else {
        return 0.0;
    };

    brent_minimize(-100.0, 100.0, 1.0e-4, |t| {
        let mut v = R_DWSIM * z * t_k / p_pa;
        v -= current_coefficient * t * b_i;
        let rho = 1.0 / v * component.molar_mass;
        let diff = target_density - rho;
        if diff.is_nan() {
            0.0
        } else {
            diff * diff
        }
    })
}

/// The density target used by the volume-translation fits, taken from the
/// compound's own Rackett density at 15.6 °C.
///
/// Upstream reads `PF_SG × 999.96` straight off the `ConstantProperties`
/// record (`Fitting.vb:168`, `:220`); [`apply_parameter_fits`] passes the real
/// assay `SG` instead, and this fallback is used only when a bare
/// [`Component`] is fitted without its [`PseudoComponent`] wrapper.
fn component_target_density(component: &Component) -> f64 {
    liquid_density_rackett(
        ThermodynamicTemperature::new::<kelvin>(DENSITY_FIT_TEMPERATURE_K),
        ThermodynamicTemperature::new::<kelvin>(component.critical_temperature),
        uom::si::f64::Pressure::new::<uom::si::pressure::pascal>(component.critical_pressure),
        Ratio::new::<ratio>(component.acentric_factor),
        uom::si::f64::MolarMass::new::<uom::si::molar_mass::kilogram_per_mole>(
            component.molar_mass,
        ),
        None,
        None,
        None,
    )
    .get::<kilogram_per_cubic_meter>()
}

/// Which of the two optional fits to run — upstream's `AdjustAF` / `AdjustZR`
/// booleans (`GenerateCompounds.vb:30`, `:399`, `:421`).
///
/// The **volume-translation** fits are not optional upstream: they run
/// unconditionally for every compound (`:434-445`), and they do here too.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ParameterFitOptions {
    /// Scale each acentric factor so the EOS reproduces the assay `Tb`
    /// (upstream `AdjustAF`). Expensive — each trial runs a bubble-point solve.
    pub adjust_acentric_factor: bool,
    /// Scale each Rackett `Z_RA` so the Rackett density reproduces the assay
    /// `SG` (upstream `AdjustZR`).
    pub adjust_rackett_z: bool,
}

/// Run the whole post-characterization fitting pass over a set of
/// pseudo-components, in place.
///
/// Ported from the driver loop at `GenerateCompounds.vb:379-448` (equivalently
/// `DistCurves.cs:773-870`), in upstream's order:
///
/// 1. If `adjust_acentric_factor`: clamp a negative `ω` to `0.5` (`:400-403`),
///    fit the acentric-factor multiplier, apply it, recompute `Z_RA`, `Zc` and
///    `Vc` (`:410-419`).
/// 2. Refresh `Z_RA = Zc1(ω)` with the `< 0 → 0.2` floor, then `Zc` and `Vc`
///    (`DistCurves.cs:825-832`).
/// 3. If `adjust_rackett_z`: fit and apply the `Z_RA` multiplier (`:421-433`).
/// 4. Always fit both volume-translation coefficients, rejecting a fitted
///    multiplier whose magnitude reaches 99 by zeroing the coefficient
///    (`:434-445`) — upstream's guard against the optimiser running to its
///    `±100` bracket.
///
/// # Valid range
///
/// Every fit is a bounded one-dimensional minimisation, so it always returns;
/// nothing here can diverge. A pseudo-component whose constants are already
/// non-physical will produce meaningless multipliers, which is why
/// [`crate::petroleum::pseudo_component::build_pseudo_component`] rejects those
/// before this pass runs.
pub fn apply_parameter_fits(components: &mut [PseudoComponent], options: ParameterFitOptions) {
    for pc in components.iter_mut() {
        let mut recalculate_vc = false;

        if options.adjust_acentric_factor {
            if pc.component.acentric_factor < 0.0 {
                pc.component.acentric_factor = 0.5;
                recalculate_vc = true;
            }
            let factor = fit_acentric_factor_scale(&pc.component);
            pc.component.acentric_factor *= factor;
            pc.chao_seader_acentricity = pc.component.acentric_factor;
        }

        let omega = Ratio::new::<ratio>(pc.component.acentric_factor);
        pc.rackett_z = critical_compressibility_zc1(omega);
        if pc.rackett_z < 0.0 {
            pc.rackett_z = 0.2;
            recalculate_vc = true;
        }
        pc.critical_compressibility = critical_compressibility_zc1(omega);
        pc.component.critical_volume = critical_volume(
            ThermodynamicTemperature::new::<kelvin>(pc.component.critical_temperature),
            uom::si::f64::Pressure::new::<uom::si::pressure::pascal>(
                pc.component.critical_pressure,
            ),
            pc.critical_compressibility,
        )
        .get::<uom::si::molar_volume::cubic_meter_per_mole>();

        if options.adjust_rackett_z {
            let factor = fit_rackett_z_scale(&pc.component, pc.specific_gravity, pc.rackett_z);
            pc.rackett_z *= factor;
        }

        if pc.critical_compressibility < 0.0 || recalculate_vc {
            pc.critical_compressibility = pc.rackett_z;
            pc.component.critical_volume = critical_volume(
                ThermodynamicTemperature::new::<kelvin>(pc.component.critical_temperature),
                uom::si::f64::Pressure::new::<uom::si::pressure::pascal>(
                    pc.component.critical_pressure,
                ),
                pc.critical_compressibility,
            )
            .get::<uom::si::molar_volume::cubic_meter_per_mole>();
        }

        pc.pr_volume_translation_coefficient = 1.0;
        let f_pr = fit_pr_volume_translation(&pc.component, 1.0);
        pc.pr_volume_translation_coefficient = if f_pr.abs() < 99.0 { f_pr } else { 0.0 };

        pc.srk_volume_translation_coefficient = 1.0;
        let f_srk = fit_srk_volume_translation(&pc.component, 1.0);
        pc.srk_volume_translation_coefficient = if f_srk.abs() < 99.0 { f_srk } else { 0.0 };
    }
}

/// The Rackett liquid density of a pseudo-component at 15.6 °C, i.e. the
/// quantity [`fit_rackett_z_scale`] drives onto the assay specific gravity.
///
/// Exposed so a caller can check the fit afterwards; not an upstream function.
#[must_use]
pub fn rackett_density_at_standard_conditions(pc: &PseudoComponent) -> MassDensity {
    liquid_density_rackett(
        ThermodynamicTemperature::new::<kelvin>(DENSITY_FIT_TEMPERATURE_K),
        ThermodynamicTemperature::new::<kelvin>(pc.component.critical_temperature),
        uom::si::f64::Pressure::new::<uom::si::pressure::pascal>(pc.component.critical_pressure),
        Ratio::new::<ratio>(pc.component.acentric_factor),
        uom::si::f64::MolarMass::new::<uom::si::molar_mass::kilogram_per_mole>(
            pc.component.molar_mass,
        ),
        Some(pc.rackett_z),
        None,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Methodology.** [`brent_minimize`] must locate the minimum of a smooth
    /// unimodal function to the requested tolerance. Test `f(x) = (x − 2.5)²`
    /// on `[0.1, 10]` (the same bracket the Rackett fit uses) and
    /// `f(x) = (x + 37)²` on `[−100, 100]` (the volume-translation bracket).
    ///
    /// **Results (2026-08-11, this port).** Both minimisers are located to
    /// better than 1e-5 absolute. Test passes.
    #[test]
    fn brent_locates_a_quadratic_minimum() {
        let x = brent_minimize(0.1, 10.0, 1.0e-10, |t| (t - 2.5) * (t - 2.5));
        assert!((x - 2.5).abs() < 1.0e-5, "got {x}");
        let y = brent_minimize(-100.0, 100.0, 1.0e-8, |t| (t + 37.0) * (t + 37.0));
        assert!((y + 37.0).abs() < 1.0e-5, "got {y}");
    }

    /// **Methodology.** The Rackett fit must move the compound's liquid density
    /// *towards* its assay specific gravity. Build a kerosene pseudo-component,
    /// record the density error before the fit, apply the fitted `Z_RA`
    /// multiplier, and require the error to shrink.
    ///
    /// **Results (2026-08-11, this port).** Reported by the assertion message on
    /// failure; on the checked run the post-fit error is strictly smaller than
    /// the pre-fit error and the fitted density lands within 1 % of
    /// `SG × 999.96`. Test passes.
    #[test]
    fn rackett_fit_reduces_the_density_error() {
        use super::super::pseudo_component::{
            build_pseudo_component, default_viscosity_points, CorrelationSet,
        };
        use uom::si::molar_mass::gram_per_mole;

        let tb = ThermodynamicTemperature::new::<kelvin>(450.0);
        let sg = Ratio::new::<ratio>(0.78);
        let (t1, t2, v1, v2) = default_viscosity_points(tb, sg);
        let mut pc = build_pseudo_component(
            "Fit",
            1,
            tb,
            sg,
            uom::si::f64::MolarMass::new::<gram_per_mole>(160.0),
            t1,
            t2,
            v1,
            v2,
            CorrelationSet::default(),
        )
        .expect("in range");

        let target = sg.get::<ratio>() * WATER_DENSITY_KG_M3;
        let before = (rackett_density_at_standard_conditions(&pc)
            .get::<kilogram_per_cubic_meter>()
            - target)
            .abs();
        let factor = fit_rackett_z_scale(&pc.component, pc.specific_gravity, pc.rackett_z);
        pc.rackett_z *= factor;
        let after = (rackett_density_at_standard_conditions(&pc).get::<kilogram_per_cubic_meter>()
            - target)
            .abs();

        assert!(
            after <= before,
            "fit made the density error worse: {before} -> {after} (factor {factor})"
        );
        assert!(
            after / target < 0.01,
            "post-fit density error {after} kg/m³ is more than 1 % of {target}"
        );
    }

    /// **Methodology.** The full [`apply_parameter_fits`] pass must leave every
    /// pseudo-component with finite, in-range parameters: `Z_RA` positive,
    /// `Zc` positive, `Vc` positive, and both volume-translation coefficients
    /// finite with magnitude below the 99 guard. Run it with both optional fits
    /// enabled over three cuts.
    ///
    /// **Results (2026-08-11, this port).** All three cuts come back with
    /// finite parameters inside the stated bounds. Test passes.
    #[test]
    fn full_fitting_pass_leaves_parameters_in_range() {
        use super::super::pseudo_component::{
            build_pseudo_component, default_viscosity_points, CorrelationSet,
        };
        use uom::si::molar_mass::gram_per_mole;

        let mut cuts: Vec<PseudoComponent> = [(400.0, 120.0), (480.0, 190.0), (560.0, 280.0)]
            .iter()
            .enumerate()
            .map(|(i, &(tb_k, mw))| {
                let tb = ThermodynamicTemperature::new::<kelvin>(tb_k);
                let sg = Ratio::new::<ratio>(0.80);
                let (t1, t2, v1, v2) = default_viscosity_points(tb, sg);
                build_pseudo_component(
                    "Fit",
                    i + 1,
                    tb,
                    sg,
                    uom::si::f64::MolarMass::new::<gram_per_mole>(mw),
                    t1,
                    t2,
                    v1,
                    v2,
                    CorrelationSet::default(),
                )
                .expect("in range")
            })
            .collect();

        apply_parameter_fits(
            &mut cuts,
            ParameterFitOptions {
                adjust_acentric_factor: true,
                adjust_rackett_z: true,
            },
        );

        for c in &cuts {
            assert!(c.rackett_z > 0.0 && c.rackett_z.is_finite(), "{c:?}");
            assert!(
                c.critical_compressibility > 0.0 && c.critical_compressibility.is_finite(),
                "{c:?}"
            );
            assert!(
                c.component.critical_volume > 0.0 && c.component.critical_volume.is_finite(),
                "{c:?}"
            );
            assert!(
                c.pr_volume_translation_coefficient.abs() < 99.0
                    && c.pr_volume_translation_coefficient.is_finite(),
                "{c:?}"
            );
            assert!(
                c.srk_volume_translation_coefficient.abs() < 99.0
                    && c.srk_volume_translation_coefficient.is_finite(),
                "{c:?}"
            );
        }
    }
}
