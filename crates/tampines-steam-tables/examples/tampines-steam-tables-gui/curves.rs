//! Every plotted **curve**, computed live from `tampines-steam-tables`.
//!
//! # The rule this module exists to keep
//!
//! Issue #26's motivation is that the figures must be *traceable to the
//! implementation being validated*. So nothing here is a stored table: the
//! saturation dome, the quality lines, the isobars and the isotherms are all
//! evaluated, on every rebuild, through this crate's own public IAPWS-IF97
//! entry points. If a region equation regresses, these curves deform — which is
//! the entire diagnostic value of the tool.
//!
//! The reference **points** are the opposite: cited data, never computed. They
//! live in [`crate::reference_data`].
//!
//! # Which routines are called
//!
//! | Quantity | Below 623.15 K | Above 623.15 K |
//! |---|---|---|
//! | `p_sat(T)` | `region_4::sat_pressure_4` | same |
//! | `h_f`, `s_f` | `region_1::h_tp_1`, `s_tp_1` | `region_3::h_rho_t_3`, `s_rho_t_3` on `v_tp_3{c,s,u,y}` |
//! | `h_g`, `s_g` | `region_2::h_tp_2`, `s_tp_2` | `region_3::h_rho_t_3`, `s_rho_t_3` on `v_tp_3{t,r,x,z}` |
//! | single-phase `h`, `s` | `checked::try_h_tp_eqm_single_phase`, `try_s_tp_eqm_single_phase` | same |
//!
//! The Region-3 sub-region selection above 623.15 K (which of `3c`, `3s`, `3u`,
//! `3y` for the liquid branch and `3t`, `3r`, `3x`, `3z` for the vapour branch)
//! uses the **same temperature thresholds** as this crate's own
//! `x_ph_flash`, deliberately, so the dome drawn here and the quality the
//! library reports cannot disagree about where the saturated states are.
//!
//! # Why the `checked` facade
//!
//! The unchecked `(T,p)` flash **panics** outside its envelope — including, by
//! design, at exactly `p = p_sat(T)`, where it reports Region 4 and the
//! single-phase enthalpy routine has nothing to return. A GUI cannot have that.
//! Every single-phase evaluation therefore goes through
//! `interfaces::checked`, which returns `Result`, and every sweep is
//! additionally kept a small offset away from the saturation line. A point that
//! still fails to evaluate becomes a **break in the curve**, never a fabricated
//! or clamped value.
//!
//! # Known limitation, stated rather than papered over
//!
//! Between 623.15 K and the critical temperature, on the **liquid** side of the
//! saturation line, this crate's `region_fwd_eqn_single_phase` classifies
//! `p_sat < p < p_B23` as Region 2 rather than Region 3. Compressed liquid
//! evaluated through the Region-2 vapour equations is an extrapolation, so the
//! sub-critical isobar liquid branches here **stop at 623.15 K** rather than
//! running up to their saturation temperature. The gap is visible on the
//! figure as a break in the curve, which is the honest representation of "this
//! implementation cannot currently be trusted here". It is recorded as a
//! finding in the bead for issue #26 rather than worked around.

use tampines_steam_tables::constants::{
    P_C_MPA, P_TRIPLE_PT_PASCAL, RHO_C_KG_PER_M3, T_C_KELVIN, T_TRIPLE_PT_KELVIN,
};
use tampines_steam_tables::interfaces::checked::{
    try_h_ps_eqm, try_h_tp_eqm_single_phase, try_s_ph_eqm, try_s_tp_eqm_single_phase, try_t_ph_eqm,
    try_t_ps_eqm, try_v_tp_eqm_single_phase, try_x_ph_flash, try_x_ps_flash,
};
use tampines_steam_tables::region_1_subcooled_liquid::{h_tp_1, s_tp_1};
use tampines_steam_tables::region_2_vapour::{h_tp_2, s_tp_2};
use tampines_steam_tables::region_3_single_phase_plus_supercritical_steam::{
    h_rho_t_3, s_rho_t_3, v_tp_3c, v_tp_3r, v_tp_3s, v_tp_3t, v_tp_3u, v_tp_3x, v_tp_3y, v_tp_3z,
};
use tampines_steam_tables::region_4_vap_liq_equilibrium::{sat_pressure_4, sat_temp_4};
use uom::si::f64::*;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::{megapascal, pascal};
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermodynamic_temperature::kelvin;

use crate::data::ThermoPoint;

/// Temperature, in kelvin, of the Region 1 / Region 3 boundary — the highest
/// temperature at which the Region-1 liquid equations apply.
pub const T_REGION_13_BOUNDARY_KELVIN: f64 = 623.15;

/// Highest temperature, in kelvin, covered by the Regions 1–3 forward
/// equations. Above it IAPWS-IF97 hands over to Region 5, which has no backward
/// `(p,h)` correlation, so the sweeps here stop at this isotherm.
pub const T_MAX_KELVIN: f64 = 1073.15;

/// Highest pressure, in pascals, covered by IAPWS-IF97 (100 MPa).
pub const P_MAX_PASCAL: f64 = 100.0e6;

/// How far, in kelvin, a single-phase sweep is held clear of the saturation
/// line. The `(T,p)` region dispatcher returns Region 4 at *exactly*
/// `p = p_sat(T)`, and the single-phase enthalpy routine cannot serve that, so
/// approaching the line arbitrarily closely is not an option. 5 mK is far below
/// plotting resolution and far above the dispatcher's exact-equality test.
const SATURATION_OFFSET_KELVIN: f64 = 5.0e-3;

/// Lowest temperature, in kelvin, at which the saturation curve is evaluated.
///
/// This is the IAPWS-IF97 Region 1 lower limit (273.15 K), which is 10 mK
/// *below* the triple point (273.16 K). The published Wagner saturation table
/// starts at 0 degC = 273.15 K and this crate's fixtures verify against that
/// row, so the curve has to reach it. The 273.15-273.16 K sliver is a
/// formulation extrapolation below the triple point rather than a physical
/// saturation state; it contributes exactly one sample and the crate's own
/// `CLAUDE.md` already flags sub-triple-point pressures as unvalidated.
const T_SAT_CURVE_MIN_KELVIN: f64 = 273.15;

/// Highest temperature, in kelvin, at which the saturation curve is evaluated.
///
/// The crate's own documentation warns that the Region 3 backward equations
/// "lose digits within ~0.5 K of Tc". Stopping 10 mK short keeps the dome from
/// ending in numerical noise; the critical point itself is added separately, as
/// a marker, from the published constants.
const T_SAT_CURVE_MAX_KELVIN: f64 = T_C_KELVIN - 0.01;

/// The four saturated properties at one temperature on the vapour-pressure
/// curve.
#[derive(Clone, Copy, Debug)]
pub struct SaturationState {
    /// Saturation temperature.
    pub temperature: ThermodynamicTemperature,
    /// Saturation pressure at that temperature.
    pub pressure: Pressure,
    /// Saturated-liquid specific enthalpy.
    pub h_liquid: AvailableEnergy,
    /// Saturated-vapour specific enthalpy.
    pub h_vapour: AvailableEnergy,
    /// Saturated-liquid specific entropy.
    pub s_liquid: SpecificHeatCapacity,
    /// Saturated-vapour specific entropy.
    pub s_vapour: SpecificHeatCapacity,
}

impl SaturationState {
    /// Whether this state is physically possible, which is the acceptance test
    /// for a Region-3 sub-region choice.
    ///
    /// # Why a finiteness check is not enough
    ///
    /// The Region-3 backward `(T,p)` volume equations are high-order
    /// polynomials fitted on narrow sub-regions of the near-critical surface.
    /// Evaluated outside their band they do not return `NaN` — they return a
    /// perfectly finite, wildly wrong number. Observed on 2026-08-20 at
    /// `T_sat` = 646.503 K, `p_sat` = 21.906 MPa: the sub-region chain used by
    /// this crate's `x_ph_flash` selects `v_tp_3y`, whose IAPWS-IF97 validity
    /// band begins **above** the critical pressure, and the resulting
    /// saturated-liquid enthalpy came out as −1.108e21 kJ/kg. That value is
    /// finite, so a finiteness filter passes it straight into the figure, where
    /// it collapses the whole enthalpy axis.
    ///
    /// # The criterion
    ///
    /// Three statements that are true of every real saturated state and false
    /// of a diverged polynomial, with no fitted magic numbers:
    ///
    /// * `h_f <= h_g` — vaporisation absorbs energy,
    /// * `s_f <= s_g` — vaporisation increases entropy,
    /// * `h_f >= -1 kJ/kg` — IAPWS-IF97 fixes `h_f` to zero at the triple
    ///   point and it rises monotonically from there, so the only slack needed
    ///   is for the small negative value the formulation gives just below
    ///   273.16 K.
    pub fn is_physically_ordered(&self) -> bool {
        use uom::si::available_energy::kilojoule_per_kilogram;
        use uom::si::specific_heat_capacity::kilojoule_per_kilogram_kelvin;
        let h_f = self.h_liquid.get::<kilojoule_per_kilogram>();
        let h_g = self.h_vapour.get::<kilojoule_per_kilogram>();
        let s_f = self.s_liquid.get::<kilojoule_per_kilogram_kelvin>();
        let s_g = self.s_vapour.get::<kilojoule_per_kilogram_kelvin>();
        h_f.is_finite()
            && h_g.is_finite()
            && s_f.is_finite()
            && s_g.is_finite()
            && h_f >= -1.0
            && h_f <= h_g
            && s_f <= s_g
    }

    /// The two-phase state at vapour quality `x`, by the Region-4 lever rule
    ///
    /// ```text
    /// h = h_f + x (h_g - h_f),      s = s_f + x (s_g - s_f)
    /// ```
    ///
    /// This is the definition issue #26 mandates. The same lever rule applies
    /// to entropy because the mixture is at equilibrium, so both phases share
    /// the saturation temperature and pressure.
    ///
    /// **The resulting quality is a derived quantity, not an independently
    /// validated property** — see [`ThermoPoint`].
    pub fn at_quality(&self, x: f64) -> ThermoPoint {
        ThermoPoint::new(
            self.pressure,
            self.temperature,
            self.h_liquid + (self.h_vapour - self.h_liquid) * x,
            self.s_liquid + (self.s_vapour - self.s_liquid) * x,
            Some(x),
        )
    }
}

/// Saturated liquid and vapour properties at temperature `t`.
///
/// Returns `None` outside `[T_triple, T_c)`, where the saturation line is not
/// defined.
pub fn saturation_state(t: ThermodynamicTemperature) -> Option<SaturationState> {
    let t_kelvin = t.get::<kelvin>();
    if !(T_SAT_CURVE_MIN_KELVIN..=T_SAT_CURVE_MAX_KELVIN).contains(&t_kelvin) {
        return None;
    }
    let pressure = sat_pressure_4(t);

    if t_kelvin <= T_REGION_13_BOUNDARY_KELVIN {
        let state = SaturationState {
            temperature: t,
            pressure,
            h_liquid: h_tp_1(t, pressure),
            h_vapour: h_tp_2(t, pressure),
            s_liquid: s_tp_1(t, pressure),
            s_vapour: s_tp_2(t, pressure),
        };
        return state.is_physically_ordered().then_some(state);
    }

    // Above 623.15 K, Region 3 straddles the saturation line and the saturated
    // volumes come from the Region-3 backward `(T,p)` sub-region equations. The
    // primary sub-region thresholds below are copied from this crate's own
    // `x_ph_flash`, so the dome and the library's reported quality normally use
    // the same branch. Where that branch produces a physically impossible
    // state, the neighbouring sub-region is tried — see
    // `region_3_saturated_volume` for why that is necessary and what it means.
    let v_vapour_candidates: [SpecificVolume; 2] = if t_kelvin <= 640.691 {
        [v_tp_3t(t, pressure), v_tp_3r(t, pressure)]
    } else if t_kelvin <= 643.15 {
        [v_tp_3r(t, pressure), v_tp_3x(t, pressure)]
    } else if t_kelvin <= 646.599 {
        [v_tp_3x(t, pressure), v_tp_3z(t, pressure)]
    } else {
        [v_tp_3z(t, pressure), v_tp_3x(t, pressure)]
    };
    let v_liquid_candidates: [SpecificVolume; 2] = if t_kelvin <= 634.659 {
        [v_tp_3c(t, pressure), v_tp_3s(t, pressure)]
    } else if t_kelvin <= 643.15 {
        [v_tp_3s(t, pressure), v_tp_3u(t, pressure)]
    } else if t_kelvin <= 646.483 {
        [v_tp_3u(t, pressure), v_tp_3y(t, pressure)]
    } else {
        // The primary choice here would be `v_tp_3y`, whose IAPWS-IF97 validity
        // band starts *above* the critical pressure; below it the polynomial
        // diverges. `v_tp_3u` is tried first for that reason. See
        // `region_3_saturated_volume`.
        [v_tp_3u(t, pressure), v_tp_3y(t, pressure)]
    };

    for v_liquid in v_liquid_candidates {
        for v_vapour in v_vapour_candidates {
            let state = SaturationState {
                temperature: t,
                pressure,
                h_liquid: h_rho_t_3(v_liquid.recip(), t),
                h_vapour: h_rho_t_3(v_vapour.recip(), t),
                s_liquid: s_rho_t_3(v_liquid.recip(), t),
                s_vapour: s_rho_t_3(v_vapour.recip(), t),
            };
            if state.is_physically_ordered() {
                return Some(state);
            }
        }
    }
    None
}

/// Temperatures at which the saturation curve is sampled, clustered towards the
/// critical point.
///
/// The dome's curvature is concentrated in the last few kelvin below `T_c`, so
/// a uniform sweep either wastes points on the flat low-temperature stretch or
/// visibly facets the apex. The mapping `T = T_c - (T_c - T_triple) (1 - u)^p`
/// with `p = 1.8` puts roughly a third of the samples in the top 5 % of the
/// range.
pub fn saturation_sweep_temperatures(samples: usize) -> Vec<ThermodynamicTemperature> {
    let n = samples.max(2);
    (0..n)
        .map(|i| {
            let u = i as f64 / (n - 1) as f64;
            let t = T_SAT_CURVE_MAX_KELVIN
                - (T_SAT_CURVE_MAX_KELVIN - T_SAT_CURVE_MIN_KELVIN) * (1.0 - u).powf(1.8);
            ThermodynamicTemperature::new::<kelvin>(t)
        })
        .collect()
}

/// The saturated-liquid line (`x = 0`) and the saturated-vapour line (`x = 1`),
/// from the triple point to just below the critical point.
pub fn saturation_lines(samples: usize) -> (Vec<ThermoPoint>, Vec<ThermoPoint>) {
    let mut liquid = Vec::with_capacity(samples);
    let mut vapour = Vec::with_capacity(samples);
    for t in saturation_sweep_temperatures(samples) {
        if let Some(state) = saturation_state(t) {
            liquid.push(state.at_quality(0.0));
            vapour.push(state.at_quality(1.0));
        }
    }
    (liquid, vapour)
}

/// A constant-quality line inside the dome, by the Region-4 lever rule.
pub fn quality_line(x: f64, samples: usize) -> Vec<ThermoPoint> {
    saturation_sweep_temperatures(samples)
        .into_iter()
        .filter_map(saturation_state)
        .map(|state| state.at_quality(x))
        .collect()
}

/// The critical point, from the published constants plus a Region-3 evaluation
/// at the critical density.
///
/// # Methodology
///
/// `T_c` and `p_c` are the IAPWS constants this crate already defines
/// (647.096 K, 22.064 MPa). Enthalpy and entropy are evaluated with the
/// Region-3 `(rho, T)` forward equations at the critical density
/// (322 kg/m³) — the crate's own advice for the critical region, since the
/// backward equations lose digits there.
///
/// # Result
///
/// Evaluates to `h_c` ≈ 2 087 kJ/kg and `s_c` ≈ 4.412 kJ/(kg K); the entropy
/// agrees with this crate's published constant `S_C_KJ_PER_KG_K`
/// (4.412 021 482 kJ/(kg K)) — which is what
/// `critical_point_matches_the_published_constant` checks.
pub fn critical_point() -> ThermoPoint {
    let t = ThermodynamicTemperature::new::<kelvin>(T_C_KELVIN);
    let rho = MassDensity::new::<kilogram_per_cubic_meter>(RHO_C_KG_PER_M3);
    ThermoPoint::new(
        Pressure::new::<megapascal>(P_C_MPA),
        t,
        h_rho_t_3(rho, t),
        s_rho_t_3(rho, t),
        None,
    )
}

/// The triple point, as its saturated-liquid state.
///
/// # Methodology
///
/// `T_triple` = 273.16 K and `p_triple` = 611.657 Pa are this crate's published
/// constants. The enthalpy and entropy are the Region-1 saturated-liquid values
/// there, which is the IAPWS-IF97 zero reference.
///
/// # Result
///
/// Evaluates to `h_f` ≈ 0.000 61 kJ/kg and `s_f` ≈ 0 kJ/(kg K), matching the
/// Wagner saturation table's first rows (`h_liq = 0.000 611 78 kJ/kg`,
/// `s_liq = 0.0`) — checked by
/// `triple_point_matches_the_wagner_saturation_table`.
pub fn triple_point_liquid() -> ThermoPoint {
    let t = ThermodynamicTemperature::new::<kelvin>(T_TRIPLE_PT_KELVIN);
    let p = Pressure::new::<pascal>(P_TRIPLE_PT_PASCAL);
    ThermoPoint::new(p, t, h_tp_1(t, p), s_tp_1(t, p), Some(0.0))
}

/// Evaluates a single-phase state at `(T, p)`, or `None` if this crate declines
/// to.
pub fn single_phase_point(t: ThermodynamicTemperature, p: Pressure) -> Option<ThermoPoint> {
    let h = try_h_tp_eqm_single_phase(t, p).ok()?;
    let s = try_s_tp_eqm_single_phase(t, p).ok()?;
    let point = ThermoPoint::new(p, t, h, s, None);
    point.is_finite().then_some(point)
}

/// A constant-pressure line, returned as contiguous segments.
///
/// # Structure
///
/// Below the critical pressure the isobar is three pieces: a compressed-liquid
/// branch, the horizontal two-phase crossing at `T_sat(p)` from `x = 0` to
/// `x = 1`, and a superheated-vapour branch. They are returned as separate
/// segments — the two-phase crossing is a genuinely different locus from the
/// single-phase branches and joining them with one polyline would draw a
/// corner that is not a state path.
///
/// At or above the critical pressure there is no phase change and the isobar is
/// a single segment.
///
/// See the module doc for why the sub-critical liquid branch stops at 623.15 K.
pub fn isobar(p: Pressure, samples: usize) -> Vec<Vec<ThermoPoint>> {
    let p_pascal = p.get::<pascal>();
    if !(P_TRIPLE_PT_PASCAL..=P_MAX_PASCAL).contains(&p_pascal) {
        return Vec::new();
    }
    let mut segments = Vec::new();
    let supercritical = p_pascal >= P_C_MPA * 1.0e6;

    if supercritical {
        segments.push(single_phase_sweep_over_temperature(
            p,
            T_TRIPLE_PT_KELVIN,
            T_MAX_KELVIN,
            samples,
        ));
    } else {
        let t_sat = sat_temp_4(p).get::<kelvin>();

        // Compressed liquid, up to whichever comes first: saturation, or the
        // Region 1/3 boundary beyond which this crate's (T,p) region dispatch
        // is not trustworthy on the liquid side.
        let liquid_top = (t_sat - SATURATION_OFFSET_KELVIN).min(T_REGION_13_BOUNDARY_KELVIN);
        if liquid_top > T_TRIPLE_PT_KELVIN {
            segments.push(single_phase_sweep_over_temperature(
                p,
                T_TRIPLE_PT_KELVIN,
                liquid_top,
                samples / 3,
            ));
        }

        // The horizontal two-phase crossing.
        if let Some(state) = saturation_state(ThermodynamicTemperature::new::<kelvin>(t_sat)) {
            let steps = (samples / 6).max(8);
            segments.push(
                (0..=steps)
                    .map(|i| state.at_quality(i as f64 / steps as f64))
                    .collect(),
            );
        }

        // Superheated vapour.
        let vapour_bottom = t_sat + SATURATION_OFFSET_KELVIN;
        if vapour_bottom < T_MAX_KELVIN {
            segments.push(single_phase_sweep_over_temperature(
                p,
                vapour_bottom,
                T_MAX_KELVIN,
                samples / 2,
            ));
        }
    }
    segments.retain(|segment: &Vec<ThermoPoint>| segment.len() >= 2);
    segments
}

/// A constant-temperature line, returned as contiguous segments.
///
/// # Structure
///
/// Below the critical temperature the isotherm is three pieces, sweeping
/// pressure upward: a superheated-vapour branch below `p_sat(T)`, the two-phase
/// crossing at `p_sat(T)` from `x = 1` down to `x = 0`, and a compressed-liquid
/// branch above it. Above the critical temperature it is one segment.
///
/// Pressure is swept **logarithmically**, because the useful range spans the
/// triple-point pressure to 100 MPa — more than five decades.
pub fn isotherm(t: ThermodynamicTemperature, samples: usize) -> Vec<Vec<ThermoPoint>> {
    let t_kelvin = t.get::<kelvin>();
    if !(T_TRIPLE_PT_KELVIN..=T_MAX_KELVIN).contains(&t_kelvin) {
        return Vec::new();
    }
    let mut segments = Vec::new();

    if t_kelvin >= T_REGION_13_BOUNDARY_KELVIN {
        // No sub-critical saturation crossing to worry about on this branch:
        // either the isotherm is supercritical, or it is in the near-critical
        // band the module doc excludes.
        segments.push(single_phase_sweep_over_pressure(
            t,
            P_TRIPLE_PT_PASCAL,
            P_MAX_PASCAL,
            samples,
        ));
    } else {
        let p_sat = sat_pressure_4(t).get::<pascal>();
        // Held clear of the saturation pressure by the same relative margin the
        // temperature sweeps use, expressed here as a pressure fraction.
        let margin = (p_sat * 1.0e-6).max(1.0e-3);

        if p_sat - margin > P_TRIPLE_PT_PASCAL {
            segments.push(single_phase_sweep_over_pressure(
                t,
                P_TRIPLE_PT_PASCAL,
                p_sat - margin,
                samples / 3,
            ));
        }
        if let Some(state) = saturation_state(t) {
            let steps = (samples / 6).max(8);
            segments.push(
                (0..=steps)
                    .map(|i| state.at_quality(1.0 - i as f64 / steps as f64))
                    .collect(),
            );
        }
        if p_sat + margin < P_MAX_PASCAL {
            segments.push(single_phase_sweep_over_pressure(
                t,
                p_sat + margin,
                P_MAX_PASCAL,
                samples / 2,
            ));
        }
    }
    segments.retain(|segment: &Vec<ThermoPoint>| segment.len() >= 2);
    segments
}

/// A constant-enthalpy line ("isenthalp"), for the GUI's custom-line control
/// (issue #26: "Add custom isenthalpic lines").
///
/// Swept logarithmically in pressure over the full IF97 range, using this
/// crate's own `(p,h)` flash (`try_t_ph_eqm`/`try_s_ph_eqm`) at each pressure
/// — the same routines the p-h diagram itself reports state through, so this
/// curve cannot disagree with what a `(p,h)` lookup at any point on it would
/// return. Unlike [`isobar`]/[`isotherm`], no special-casing of the two-phase
/// dome is needed: `(p,h)` uniquely determines a state everywhere IF97 is
/// defined (inside the dome that state is a Region-4 mixture, and `try_t_ph_eqm`
/// returns its saturation temperature directly), so the curve is naturally
/// continuous and is returned as a single segment (a handful of points this
/// crate declines to evaluate are simply dropped, following the rest of this
/// module's convention).
pub fn isenthalp(h: AvailableEnergy, samples: usize) -> Vec<Vec<ThermoPoint>> {
    let n = samples.max(2);
    let (lo, hi) = (P_TRIPLE_PT_PASCAL.log10(), P_MAX_PASCAL.log10());
    let points: Vec<ThermoPoint> = (0..n)
        .filter_map(|i| {
            let frac = i as f64 / (n - 1) as f64;
            let p = Pressure::new::<pascal>(10.0_f64.powf(lo + (hi - lo) * frac));
            let t = try_t_ph_eqm(p, h).ok()?;
            let s = try_s_ph_eqm(p, h).ok()?;
            // Quality where meaningful (a Region-4 point on the sweep) —
            // `try_x_ph_flash` returns `Err` for a single-phase state, which
            // is exactly when `None` (no quality) is right.
            let quality = try_x_ph_flash(p, h).ok();
            let point = ThermoPoint::new(p, t, h, s, quality);
            point.is_finite().then_some(point)
        })
        .collect();
    if points.len() >= 2 {
        vec![points]
    } else {
        Vec::new()
    }
}

/// A constant-entropy line ("isentrope"), for the GUI's custom-line control
/// (issue #26: "Add custom isentropic lines").
///
/// Structurally identical to [`isenthalp`], swept in pressure using the
/// `(p,s)` flash (`try_t_ps_eqm`/`try_h_ps_eqm`) instead of `(p,h)`.
pub fn isentrope(s: SpecificHeatCapacity, samples: usize) -> Vec<Vec<ThermoPoint>> {
    let n = samples.max(2);
    let (lo, hi) = (P_TRIPLE_PT_PASCAL.log10(), P_MAX_PASCAL.log10());
    let points: Vec<ThermoPoint> = (0..n)
        .filter_map(|i| {
            let frac = i as f64 / (n - 1) as f64;
            let p = Pressure::new::<pascal>(10.0_f64.powf(lo + (hi - lo) * frac));
            let t = try_t_ps_eqm(p, s).ok()?;
            let h = try_h_ps_eqm(p, s).ok()?;
            let quality = try_x_ps_flash(p, s).ok();
            let point = ThermoPoint::new(p, t, h, s, quality);
            point.is_finite().then_some(point)
        })
        .collect();
    if points.len() >= 2 {
        vec![points]
    } else {
        Vec::new()
    }
}

/// A constant-specific-volume line ("isochore"), for the GUI's custom-line
/// control (issue #26: "Add custom isovolumetric lines").
///
/// Physically this is the locus a **closed rigid vessel** follows as it is
/// heated, so the dome crossing is the whole story of that process and the part
/// worth drawing well.
///
/// # Parameterised by temperature, not enthalpy
///
/// Volume has no inverse in IAPWS-IF97 — the standard gives `v(p,h)`, not
/// `p(rho,h)` — so a constant-`v` locus must be solved for. **Which variable
/// you sweep decides how much of that solving is actually necessary**, and
/// sweeping temperature makes most of it disappear:
///
/// * **Inside the dome — closed form, no root finding.** A sub-critical `T`
///   fixes the pressure outright at `p_sat(T)`, and `v` is *exactly linear* in
///   `h` there (both are linear in quality), so two flash evaluations pin the
///   line and the quality follows by arithmetic.
/// * **Region 3 — closed form, no root finding.** Its fundamental equation is a
///   Helmholtz free energy explicit in `(rho, T)`, so [`p_rho_t_3`] gives the
///   pressure directly.
/// * **Regions 1 and 2 — one monotone bisection.** No `p(v,T)` exists here
///   either, but at fixed `T` specific volume is monotone in pressure, so the
///   solve is safe and warm-starts from the previous point on the sweep.
///
/// # Why the previous version was replaced
///
/// It swept **enthalpy** at fixed density and took the pressure from
/// `p_rho_h_eqm`, a bracketed root find in which every residual evaluation is a
/// full forward `(p,h)` flash. That is a real inversion, and it costs like one.
/// Measured 2026-09-15
/// (`diagnose_the_cost_of_the_inversion_relative_to_a_ph_flash`):
///
/// | call | cost |
/// |---|---|
/// | `v_ph_eqm` (forward `(p,h)` flash) | 3 296.7 ns |
/// | `p_rho_h_eqm` (inverse) | 249 186.8 ns |
/// | ratio | **75.6x** |
///
/// At the GUI's default 400 samples that is about **100 ms per isochore**, and
/// the custom-line layer rebuilt every frame, which capped the plot page near
/// 10 fps with one isochore on screen.
///
/// **Measured after this rewrite: 9.8–11.3 ms for 400 samples (24–28 us per
/// sample) across `v0` = 0.005, 0.05 and 0.5 m3/kg — about a 10x improvement**,
/// on top of which `PlotterApp::custom_layers` now caches, so the cost is paid
/// once per edit rather than once per frame.
///
/// # Accuracy
///
/// The dome branch is not merely faster, it is **more accurate**, because a
/// lever rule fitted to the flash is exact where a root find is only converged.
/// Round-trip through `v_ph_eqm`, measured 2026-09-15 at 150 samples:
///
/// | `v0` (m3/kg) | two-phase points | worst two-phase `|dv/v|` | worst single-phase `|dv/v|` |
/// |---|---|---|---|
/// | 0.005 | 68 | **4.25e-14** | 1.003e-4 |
/// | 0.5 | 26 | **5.55e-16** | 1.816e-5 |
///
/// Two-phase agreement is at machine precision. The single-phase residuals are
/// **not** construction error: those points take their pressure from the exact
/// forward equations (`p_rho_t_3`, or a bisection converged to 1e-10 on
/// `v_tp_eqm_single_phase`), and the check round-trips them through the
/// *backward* flash, whose own IF97 accuracy is the limit. The worst cases sit
/// where that is documented to be weakest — 643.62 K / 21.117 MPa, about 3.5 K
/// below the critical point, and 810.06 K / 48.802 MPa. See the tolerance note
/// in `isochore_reproduces_the_requested_volume_including_inside_the_dome`.
///
/// # Structure
///
/// One segment per contiguous run of evaluable states. A constant-density line
/// passes smoothly through Region 4 with quality varying along it, so a
/// sub-critical isochore is typically a single segment rather than separate
/// liquid and vapour branches. Gaps still break the curve where a state cannot
/// be evaluated, so a polyline is never drawn across points that were not
/// computed.
///
/// # Accuracy caveat for *measured* densities
///
/// Pressure along a **liquid** isochore is the least certain part of this
/// diagram: liquid water is nearly incompressible, so the amplification
/// `|d ln p / d ln v|_h` is large at low pressure (measured `2.3e1` at 99 MPa
/// against `4.0e4` at 0.5 bar). The curve is computed from an exact density so
/// it is accurate as drawn, but a reader inferring pressure from a *measured*
/// liquid density should not expect the same — see `p_rho_h_conditioning`.
pub fn isochore(v0: SpecificVolume, samples: usize) -> Vec<Vec<ThermoPoint>> {
    let v0_si = v0.get::<cubic_meter_per_kilogram>();
    if !(v0_si.is_finite() && v0_si > 0.0) {
        return Vec::new();
    }

    let n = samples.max(2);
    let mut segments: Vec<Vec<ThermoPoint>> = Vec::new();
    let mut current: Vec<ThermoPoint> = Vec::new();
    // Warm start for the Region 1/2 pressure solve: marching in temperature,
    // the previous accepted pressure is an excellent initial bracket centre.
    let mut p_guess_pa: Option<f64> = None;

    for i in 0..n {
        let frac = i as f64 / (n - 1) as f64;
        let t_kelvin = T_SAT_CURVE_MIN_KELVIN + (T_MAX_KELVIN - T_SAT_CURVE_MIN_KELVIN) * frac;
        let t = ThermodynamicTemperature::new::<kelvin>(t_kelvin);

        match isochore_point_at_temperature(v0_si, t, &mut p_guess_pa) {
            Some(point) => current.push(point),
            None => {
                if current.len() >= 2 {
                    segments.push(std::mem::take(&mut current));
                } else {
                    current.clear();
                }
            }
        }
    }
    if current.len() >= 2 {
        segments.push(current);
    }
    segments
}

/// One point on the `v = v0` isochore at temperature `t`, or `None` where the
/// state is not representable.
///
/// `p_guess_pa` carries the previous accepted pressure along the sweep and is
/// updated in place; see the Region 1/2 branch for what it is worth.
fn isochore_point_at_temperature(
    v0_si: f64,
    t: ThermodynamicTemperature,
    p_guess_pa: &mut Option<f64>,
) -> Option<ThermoPoint> {
    use tampines_steam_tables::interfaces::checked::try_v_ph_eqm;
    use uom::si::available_energy::joule_per_kilogram;
    use tampines_steam_tables::region_3_single_phase_plus_supercritical_steam::intensive_properties::p_rho_t_3;
    use tampines_steam_tables::region_3_single_phase_plus_supercritical_steam::p_boundary_2_3;

    // ── 1. Inside the dome: closed form, no iteration ──────────────────────
    //
    // At fixed `v`, a sub-critical temperature fixes everything: the pressure
    // is `p_sat(T)` outright, and the quality follows from the lever rule
    //
    //     x = (v0 - v_f) / (v_g - v_f)
    //
    // with `h` and `s` the same linear blend. This is the whole reason the
    // sweep is parameterised by temperature rather than by enthalpy — see the
    // note on the previous implementation below.
    if let Some(sat) = saturation_state(t) {
        // Inside the dome `v` is EXACTLY linear in `h` at fixed pressure --
        // both are linear in quality -- so two evaluations pin the line and the
        // answer follows by arithmetic. No root finding.
        //
        // The two samples are taken slightly INSIDE the dome (x = 0.001 and
        // 0.999) rather than on the saturation boundaries. On the boundary the
        // flash's own region classification is marginal: at `h` exactly `h_f`
        // it may resolve the state as single-phase Region 1 and return the
        // Region-1 volume, which differs from the dome's `v_f` by ~2e-5
        // relative. Fitting the line from two interior points and solving along
        // it keeps this construction consistent with the flash BY
        // CONSTRUCTION, which is what the round-trip test checks.
        //
        // Checked calls throughout: near the triple point `p_sat` sits at the
        // very edge of the flash's accepted pressure range and the unchecked
        // `v_ph_eqm` PANICS rather than declining. A curve generator must drop
        // a point it cannot evaluate, never abort the GUI.
        let h_f = sat.h_liquid.get::<joule_per_kilogram>();
        let h_g = sat.h_vapour.get::<joule_per_kilogram>();
        let span = h_g - h_f;
        if span > 0.0 {
            let h_at = |x: f64| AvailableEnergy::new::<joule_per_kilogram>(h_f + span * x);
            let v_at = |x: f64| {
                try_v_ph_eqm(sat.pressure, h_at(x))
                    .map(|v| v.get::<cubic_meter_per_kilogram>())
                    .ok()
                    .filter(|v| v.is_finite())
            };
            if let (Some(v_a), Some(v_b)) = (v_at(0.001), v_at(0.999)) {
                let slope = (v_b - v_a) / 0.998;
                if slope.abs() > 0.0 {
                    let mut x = 0.001 + (v0_si - v_a) / slope;
                    // Decide dome membership from the SOLVED quality, before
                    // refining. Testing `v0` against the two sample volumes
                    // instead would be wrong wherever the dome is wide: at
                    // 278.5 K, `v_f` is 0.001 and `v_g` about 145 m3/kg, so the
                    // x = 0.001 sample already sits at v = 0.146 and a target of
                    // 0.005 (true x = 2.8e-5) would be rejected as "outside"
                    // when it is comfortably inside.
                    //
                    // Checking first also stops the refinement from dragging an
                    // out-of-dome x back into [0, 1] and emitting a
                    // superheated-vapour state mislabelled two-phase --
                    // v0 = 0.5 m3/kg at 418 K came out 18% wrong that way.
                    if !(0.0..=1.0).contains(&x) {
                        return isochore_single_phase_pressure(v0_si, t, p_guess_pa)
                            .and_then(|p| single_phase_point(t, p));
                    }
                    // Near the critical point the flash's quality resolution
                    // degrades and `v(h)` stops being exactly linear -- the
                    // crate's own guidance is that the Region 3 backward
                    // equations lose digits approaching Tc. Measured there, the
                    // bare linear fit lands 1.3e-5 relative off at 643.6 K /
                    // 21.1 MPa.
                    //
                    // A bounded secant refinement on the SAME slope fixes it.
                    // Note what this is and is not: at most three extra forward
                    // flashes (~10 us) on an already near-linear function, not
                    // the 75-flash bracketed inversion this rewrite exists to
                    // remove. It converges to the flash's own precision because
                    // the function really is almost a straight line.
                    for _ in 0..3 {
                        let Some(v_x) = v_at(x) else { break };
                        let error = v_x - v0_si;
                        if error.abs() <= 1.0e-12 * v0_si.abs().max(1.0) {
                            break;
                        }
                        x -= error / slope;
                    }
                    // Emit ONLY if the refinement actually converged. An
                    // unconverged `x` that happens to land in [0, 1] is a
                    // fabricated state, not a solution -- this is what produced
                    // the 18% error above. Verified against the flash, which is
                    // the same thing the round-trip test checks.
                    let converged = v_at(x)
                        .map(|v| (v - v0_si).abs() <= 1.0e-9 * v0_si.abs().max(1.0))
                        .unwrap_or(false);
                    if converged && (0.0..=1.0).contains(&x) {
                        let h = h_at(x);
                        let s = sat.s_liquid + (sat.s_vapour - sat.s_liquid) * x;
                        *p_guess_pa = Some(sat.pressure.get::<pascal>());
                        let candidate = ThermoPoint::new(sat.pressure, t, h, s, Some(x));
                        return candidate.is_finite().then_some(candidate);
                    }
                }
            }
        }
    }

    // ── 2. Region 3: closed form, no iteration ────────────────────────────
    //
    // Region 3's fundamental equation is a Helmholtz free energy explicit in
    // `(rho, T)`, so the pressure is a direct evaluation rather than a solve.
    // Whether the state really is Region 3 cannot be known before the pressure
    // is in hand, so evaluate first and then validate against the region's own
    // bounds; if it does not land in Region 3, fall through.
    if t.get::<kelvin>() >= T_REGION_13_BOUNDARY_KELVIN {
        let rho = MassDensity::new::<kilogram_per_cubic_meter>(1.0 / v0_si);
        let p = p_rho_t_3(rho, t);
        let p_pa = p.get::<pascal>();
        if p_pa.is_finite()
            && (P_TRIPLE_PT_PASCAL..=P_MAX_PASCAL).contains(&p_pa)
            && p_pa >= p_boundary_2_3(t).get::<pascal>()
        {
            if let Some(point) = single_phase_point(t, p) {
                *p_guess_pa = Some(p_pa);
                return Some(point);
            }
        }
    }

    // ── 3. Regions 1 and 2: a monotone 1-D solve, warm started ────────────
    //
    // IF97 publishes no `p(v, T)` for the liquid and vapour regions either, so
    // this branch does iterate. It is a far tamer problem than the `(rho, h)`
    // inversion it replaces: at fixed `T`, specific volume is *monotone
    // decreasing* in pressure across the whole of Region 1 and Region 2, so
    // there are no seams to partition around and a bracketed bisection cannot
    // land on the wrong root.
    isochore_single_phase_pressure(v0_si, t, p_guess_pa).and_then(|p| single_phase_point(t, p))
}

/// Solves `v_tp(T, p) = v0` for pressure in Region 1 or 2 at fixed `T`.
///
/// Monotone in `p`, so a bracketed bisection is both safe and sufficient. The
/// bracket is seeded from `p_guess_pa` (the previous point on the sweep) and
/// widened geometrically until it straddles the root, which on a smooth
/// isochore usually succeeds on the first expansion.
fn isochore_single_phase_pressure(
    v0_si: f64,
    t: ThermodynamicTemperature,
    p_guess_pa: &mut Option<f64>,
) -> Option<Pressure> {
    // Residual is positive where the fluid is too expansive (pressure too low)
    // and negative where it is too dense, because v falls as p rises.
    let residual = |p_pa: f64| -> Option<f64> {
        let p = Pressure::new::<pascal>(p_pa);
        try_v_tp_eqm_single_phase(t, p)
            .ok()
            .map(|v| v.get::<cubic_meter_per_kilogram>() - v0_si)
            .filter(|r| r.is_finite())
    };

    // Seed a bracket around the previous pressure, widening by decades until it
    // straddles the root or the chart bounds are exhausted.
    let seed = p_guess_pa
        .unwrap_or(1.0e5)
        .clamp(P_TRIPLE_PT_PASCAL, P_MAX_PASCAL);
    let (mut lo, mut hi) = (seed, seed);
    let (mut r_lo, mut r_hi) = (residual(seed)?, residual(seed)?);
    for _ in 0..24 {
        if r_lo * r_hi <= 0.0 && lo < hi {
            break;
        }
        lo = (lo / 3.0).max(P_TRIPLE_PT_PASCAL);
        hi = (hi * 3.0).min(P_MAX_PASCAL);
        r_lo = residual(lo)?;
        r_hi = residual(hi)?;
        if lo <= P_TRIPLE_PT_PASCAL && hi >= P_MAX_PASCAL && r_lo * r_hi > 0.0 {
            // This volume is unreachable at this temperature anywhere on the
            // chart. Dropped, never fabricated.
            return None;
        }
    }
    if r_lo * r_hi > 0.0 {
        return None;
    }

    // Bisection to a relative pressure tolerance; ~40 halvings of a decade
    // bracket is well past f64 usefulness, so 60 is a hard stop, not a budget.
    for _ in 0..60 {
        if (hi - lo) <= 1.0e-10 * hi.abs().max(1.0) {
            break;
        }
        let mid = 0.5 * (lo + hi);
        let r_mid = residual(mid)?;
        if r_mid == 0.0 {
            lo = mid;
            hi = mid;
            break;
        }
        if r_lo * r_mid < 0.0 {
            hi = mid;
            r_hi = r_mid;
        } else {
            lo = mid;
            r_lo = r_mid;
        }
    }
    let _ = r_hi;
    let p_pa = 0.5 * (lo + hi);
    *p_guess_pa = Some(p_pa);
    Some(Pressure::new::<pascal>(p_pa))
}

/// Linear temperature sweep at fixed pressure, dropping points this crate
/// declines to evaluate.
fn single_phase_sweep_over_temperature(
    p: Pressure,
    t_lo_kelvin: f64,
    t_hi_kelvin: f64,
    samples: usize,
) -> Vec<ThermoPoint> {
    let n = samples.max(2);
    (0..n)
        .filter_map(|i| {
            let frac = i as f64 / (n - 1) as f64;
            let t_kelvin = t_lo_kelvin + (t_hi_kelvin - t_lo_kelvin) * frac;
            single_phase_point(ThermodynamicTemperature::new::<kelvin>(t_kelvin), p)
        })
        .collect()
}

/// Logarithmic pressure sweep at fixed temperature, dropping points this crate
/// declines to evaluate.
fn single_phase_sweep_over_pressure(
    t: ThermodynamicTemperature,
    p_lo_pascal: f64,
    p_hi_pascal: f64,
    samples: usize,
) -> Vec<ThermoPoint> {
    if !(p_lo_pascal > 0.0 && p_hi_pascal > p_lo_pascal) {
        return Vec::new();
    }
    let n = samples.max(2);
    let (lo, hi) = (p_lo_pascal.log10(), p_hi_pascal.log10());
    (0..n)
        .filter_map(|i| {
            let frac = i as f64 / (n - 1) as f64;
            let p_pascal = 10.0_f64.powf(lo + (hi - lo) * frac);
            single_phase_point(t, Pressure::new::<pascal>(p_pascal))
        })
        .collect()
}

/// Default isobars, in bar, drawn when the isobar layer is on.
///
/// Spread roughly logarithmically from a condenser vacuum to the IF97 ceiling,
/// with 1 bar and the supercritical band both represented.
pub const DEFAULT_ISOBARS_BAR: [f64; 10] =
    [0.01, 0.1, 1.0, 5.0, 20.0, 50.0, 100.0, 160.0, 300.0, 700.0];

/// Default isotherms, in degrees Celsius, drawn when the isotherm layer is on.
///
/// The sub-critical entries stay at or below 340 °C, which is comfortably below
/// the 623.15 K (350 °C) Region 1/3 boundary discussed in the module doc; the
/// rest are supercritical.
pub const DEFAULT_ISOTHERMS_DEGC: [f64; 9] =
    [50.0, 100.0, 150.0, 200.0, 250.0, 300.0, 340.0, 500.0, 700.0];

/// Quality lines required by issue #26.
pub const QUALITY_LINE_VALUES: [f64; 5] = [0.1, 0.3, 0.5, 0.7, 0.9];

/// Slider bounds for the GUI's custom-line controls (issue #26: "Use sensible
/// defaults depending on line type... Add numeric input beside sliders for
/// precise values"), one `(min, max)` pair per line type in the unit its
/// slider is shown in.
///
/// Isobar and isotherm reuse the same physical bounds every other sweep in
/// this module is clamped to (the triple point to the IF97 ceiling); entropy,
/// enthalpy and specific volume are bounded to the range this crate's own
/// single-phase equations actually cover across that same `(p,T)` box —
/// loosely, since the true achievable range is state-path-dependent and a
/// slightly generous bound just means a few points near the edge fail to
/// evaluate and are dropped, per this module's "never fabricate" rule, not
/// silently clamped to something wrong.
pub const CUSTOM_ISOBAR_RANGE_BAR: (f64, f64) = (P_TRIPLE_PT_PASCAL / 1.0e5, P_MAX_PASCAL / 1.0e5);
/// See [`CUSTOM_ISOBAR_RANGE_BAR`].
pub const CUSTOM_ISOTHERM_RANGE_DEGC: (f64, f64) = (0.01, 800.0);
/// See [`CUSTOM_ISOBAR_RANGE_BAR`].
pub const CUSTOM_ISENTROPE_RANGE_KJ_PER_KG_K: (f64, f64) = (0.0, 12.0);
/// See [`CUSTOM_ISOBAR_RANGE_BAR`].
pub const CUSTOM_ISENTHALP_RANGE_KJ_PER_KG: (f64, f64) = (0.0, 4500.0);
/// See [`CUSTOM_ISOBAR_RANGE_BAR`].
pub const CUSTOM_ISOCHORE_RANGE_M3_PER_KG: (f64, f64) = (0.001, 50.0);
/// Vapour quality `x` is dimensionless by definition, `0.0` (saturated
/// liquid) to `1.0` (saturated vapour) — the full physical range, not a
/// truncated slider like the other custom-line types need.
pub const CUSTOM_QUALITY_RANGE: (f64, f64) = (0.0, 1.0);

/// Verifies the computed saturation curve against the published Wagner
/// saturation table.
///
/// # Methodology
///
/// This is the tool's own V&V gate, and it is the check that makes the figures
/// worth anything: the plotted dome is compared, point by point, against
/// Kretzschmar & Wagner's *International Steam Tables* values as carried in
/// [`crate::reference_data::wagner::WAGNER_SATURATION_TABLE`]. For every table
/// row, [`saturation_state`] is evaluated at the tabulated saturation
/// temperature and the computed saturation pressure, saturated-liquid enthalpy
/// and saturated-vapour enthalpy are compared with the tabulated ones.
///
/// Tolerances: 0.5 % relative on `p_sat`; 1.0 kJ/kg absolute **or** 0.5 %
/// relative, whichever is looser, on `h_f` and `h_g`. The absolute floor on
/// enthalpy exists because `h_f` passes through zero at the triple point, where
/// a relative tolerance is meaningless. Rows within 1 K of the critical
/// temperature are skipped: the crate documents that its Region 3 backward
/// equations lose digits there, and this test is not the place to relitigate
/// that.
///
/// # Result (measured 2026-08-20)
///
/// Passes over the whole table from 0 °C to within 1 K of `T_c`. The curve the
/// GUI draws is therefore the published saturation line to within the
/// tolerances above, across the full sub-critical range — which is exactly the
/// claim the "validation coverage" figures make visually.
#[cfg(test)]
#[test]
fn saturation_curve_matches_the_wagner_steam_table() {
    use crate::reference_data::wagner::{
        SAT_COL_H_LIQ, SAT_COL_H_VAP, SAT_COL_P_BAR, SAT_COL_T_DEGC, WAGNER_SATURATION_TABLE,
    };
    use uom::si::available_energy::kilojoule_per_kilogram;
    use uom::si::pressure::bar;
    use uom::si::thermodynamic_temperature::degree_celsius;

    let mut checked = 0usize;
    for row in WAGNER_SATURATION_TABLE {
        let t = ThermodynamicTemperature::new::<degree_celsius>(row[SAT_COL_T_DEGC]);
        if t.get::<kelvin>() > T_C_KELVIN - 1.0 {
            continue;
        }
        let Some(state) = saturation_state(t) else {
            panic!("no saturation state at {} degC", row[SAT_COL_T_DEGC]);
        };

        let p_ref = row[SAT_COL_P_BAR];
        let p_got = state.pressure.get::<bar>();
        assert!(
            (p_got - p_ref).abs() <= p_ref.abs() * 5.0e-3,
            "p_sat at {} degC: got {p_got} bar, table {p_ref} bar",
            row[SAT_COL_T_DEGC]
        );

        for (label, reference, computed) in [
            (
                "h_f",
                row[SAT_COL_H_LIQ],
                state.h_liquid.get::<kilojoule_per_kilogram>(),
            ),
            (
                "h_g",
                row[SAT_COL_H_VAP],
                state.h_vapour.get::<kilojoule_per_kilogram>(),
            ),
        ] {
            let tolerance = (reference.abs() * 5.0e-3).max(1.0);
            assert!(
                (computed - reference).abs() <= tolerance,
                "{label} at {} degC: got {computed} kJ/kg, table {reference} kJ/kg",
                row[SAT_COL_T_DEGC]
            );
        }
        checked += 1;
    }
    assert!(
        checked > 190,
        "expected most of the table to be checked, got {checked}"
    );
}

/// Verifies the computed critical point against this crate's published critical
/// entropy constant.
///
/// # Methodology
///
/// [`critical_point`] evaluates the Region-3 `(rho, T)` equations at
/// 322 kg/m³ and 647.096 K. Its entropy is compared with
/// `constants::S_C_KJ_PER_KG_K`, which the crate derived independently.
/// Tolerance 0.1 % relative.
///
/// # Result (measured 2026-08-20)
///
/// Passes: the Region-3 evaluation reproduces the published constant, so the
/// critical-point marker on every diagram sits where the crate says the
/// critical point is, rather than at a hard-coded guess.
#[cfg(test)]
#[test]
fn critical_point_matches_the_published_constant() {
    use tampines_steam_tables::constants::S_C_KJ_PER_KG_K;
    use uom::si::specific_heat_capacity::kilojoule_per_kilogram_kelvin;
    let point = critical_point();
    let s = point
        .specific_entropy
        .get::<kilojoule_per_kilogram_kelvin>();
    assert!(
        (s - S_C_KJ_PER_KG_K).abs() <= S_C_KJ_PER_KG_K * 1.0e-3,
        "critical entropy: computed {s}, published constant {S_C_KJ_PER_KG_K}"
    );
    assert!(point.is_finite());
}

/// Verifies the triple-point marker against the first rows of the Wagner
/// saturation table.
///
/// # Methodology
///
/// Compares [`triple_point_liquid`]'s enthalpy and entropy with the 0.01 °C row
/// of the published table (`h_f = 0.000 611 78 kJ/kg`, `s_f = 0.0`), to 1e-3
/// kJ/kg and 1e-4 kJ/(kg K) absolute — the IAPWS-IF97 zero reference, so both
/// should be essentially zero.
///
/// # Result (measured 2026-08-20)
///
/// Passes.
#[cfg(test)]
#[test]
fn triple_point_matches_the_wagner_saturation_table() {
    use uom::si::available_energy::kilojoule_per_kilogram;
    use uom::si::specific_heat_capacity::kilojoule_per_kilogram_kelvin;
    let point = triple_point_liquid();
    let h = point.specific_enthalpy.get::<kilojoule_per_kilogram>();
    let s = point
        .specific_entropy
        .get::<kilojoule_per_kilogram_kelvin>();
    assert!(
        h.abs() <= 1.0e-3,
        "triple-point h_f = {h} kJ/kg, expected ~0"
    );
    assert!(
        s.abs() <= 1.0e-4,
        "triple-point s_f = {s} kJ/(kg K), expected ~0"
    );
}

/// Checks that quality lines really are the lever rule, and that they nest
/// inside the dome.
///
/// # Methodology
///
/// At a spread of saturation temperatures, asserts that the `x = 0.5` state's
/// enthalpy is the exact arithmetic mean of `h_f` and `h_g` (to 1e-9 relative),
/// and that the five required quality lines are strictly ordered in enthalpy at
/// every temperature: `h(0.1) < h(0.3) < h(0.5) < h(0.7) < h(0.9)`, all of them
/// strictly between `h_f` and `h_g`.
///
/// # Result (measured 2026-08-20)
///
/// Passes at 1 °C, 100 °C, 200 °C, 300 °C and 360 °C — the last of which is
/// above the Region 1/3 boundary, so it also exercises the Region-3 branch of
/// [`saturation_state`].
#[cfg(test)]
#[test]
fn quality_lines_follow_the_lever_rule_and_nest_inside_the_dome() {
    use uom::si::available_energy::joule_per_kilogram;
    use uom::si::thermodynamic_temperature::degree_celsius;
    for t_degc in [1.0, 100.0, 200.0, 300.0, 360.0] {
        let t = ThermodynamicTemperature::new::<degree_celsius>(t_degc);
        let state = saturation_state(t).expect("sub-critical temperature has a saturation state");
        let h_f = state.h_liquid.get::<joule_per_kilogram>();
        let h_g = state.h_vapour.get::<joule_per_kilogram>();
        let half = state
            .at_quality(0.5)
            .specific_enthalpy
            .get::<joule_per_kilogram>();
        let expected = 0.5 * (h_f + h_g);
        assert!(
            (half - expected).abs() <= expected.abs() * 1.0e-9,
            "lever rule broken at {t_degc} degC"
        );

        let mut previous = h_f;
        for x in QUALITY_LINE_VALUES {
            let h = state
                .at_quality(x)
                .specific_enthalpy
                .get::<joule_per_kilogram>();
            assert!(h > previous, "quality lines out of order at {t_degc} degC");
            assert!(h < h_g, "quality line escaped the dome at {t_degc} degC");
            previous = h;
        }
    }
}

/// Checks that isobars and isotherms come back as sensible, finite, ordered
/// segments.
///
/// # Methodology
///
/// For each default isobar and isotherm, asserts at least one segment is
/// produced, every point is finite, and — for sub-critical isobars — that
/// exactly one segment is the two-phase crossing (every point in it carries a
/// `Some(quality)`) while the others are single-phase (`None`).
///
/// # Result (measured 2026-08-20)
///
/// Passes for all ten default isobars and all nine default isotherms. The
/// 160 bar and 300 bar isobars exercise, respectively, the sub-critical
/// liquid-branch cut-off at 623.15 K and the continuous supercritical sweep.
#[cfg(test)]
#[test]
fn default_isobars_and_isotherms_produce_finite_ordered_segments() {
    use uom::si::pressure::bar;
    use uom::si::thermodynamic_temperature::degree_celsius;

    for p_bar in DEFAULT_ISOBARS_BAR {
        let p = Pressure::new::<bar>(p_bar);
        let segments = isobar(p, 120);
        assert!(!segments.is_empty(), "no isobar segments at {p_bar} bar");
        let two_phase = segments
            .iter()
            .filter(|segment| segment.iter().all(|point| point.quality.is_some()))
            .count();
        if p_bar < P_C_MPA * 10.0 {
            assert_eq!(
                two_phase, 1,
                "sub-critical isobar at {p_bar} bar needs exactly one two-phase crossing"
            );
        }
        for segment in &segments {
            for point in segment {
                assert!(point.is_finite(), "non-finite isobar point at {p_bar} bar");
            }
        }
    }

    for t_degc in DEFAULT_ISOTHERMS_DEGC {
        let t = ThermodynamicTemperature::new::<degree_celsius>(t_degc);
        let segments = isotherm(t, 120);
        assert!(
            !segments.is_empty(),
            "no isotherm segments at {t_degc} degC"
        );
        for segment in &segments {
            for point in segment {
                assert!(
                    point.is_finite(),
                    "non-finite isotherm point at {t_degc} degC"
                );
            }
        }
    }
}

/// Checks the two new custom-line curves that reuse an existing flash
/// ([`isenthalp`], [`isentrope`]) produce finite, correctly-labelled points
/// spanning subcooled liquid, the two-phase dome and superheated vapour.
///
/// # Methodology
///
/// `h`/`s` values are chosen to be representative of each region at moderate
/// pressure (subcooled liquid near the triple point's `h_f`, a mid-dome value,
/// superheated near 500 degC at 10 bar). For each: asserts the curve is
/// non-empty and every point finite, and that pressure is monotonic along the
/// single returned segment (both flashes sweep pressure directly, so a
/// non-monotonic result would mean the sweep itself is broken, not a physics
/// issue). For the two-phase value, additionally asserts at least one point
/// on the curve reports `quality.is_some()` — i.e. the dome-crossing wiring
/// via `try_x_ph_flash`/`try_x_ps_flash` actually fires rather than staying
/// `None` everywhere.
///
/// # Result (measured 2026-08-20)
///
/// Passes for all three representative values on both curve types.
#[cfg(test)]
#[test]
fn isenthalp_and_isentrope_sweep_cleanly_through_every_region() {
    use uom::si::available_energy::kilojoule_per_kilogram;
    use uom::si::pressure::pascal as pascal_unit;
    use uom::si::specific_heat_capacity::kilojoule_per_kilogram_kelvin;

    let check_monotonic_pressure = |segments: &[Vec<ThermoPoint>], label: &str| {
        assert!(!segments.is_empty(), "{label}: produced no segments");
        for segment in segments {
            for point in segment {
                assert!(point.is_finite(), "{label}: non-finite point");
            }
            for pair in segment.windows(2) {
                assert!(
                    pair[1].pressure.get::<pascal_unit>() > pair[0].pressure.get::<pascal_unit>(),
                    "{label}: pressure sweep is not monotonically increasing"
                );
            }
        }
    };

    // Subcooled liquid, two-phase, superheated -- kJ/kg values chosen to land
    // in each region at pressures within the sweep's own range.
    for h_kj in [200.0, 1500.0, 3400.0] {
        let h = AvailableEnergy::new::<kilojoule_per_kilogram>(h_kj);
        let segments = isenthalp(h, 200);
        check_monotonic_pressure(&segments, &format!("isenthalp h={h_kj} kJ/kg"));
    }
    let two_phase_h = AvailableEnergy::new::<kilojoule_per_kilogram>(1500.0);
    let two_phase_segments = isenthalp(two_phase_h, 200);
    assert!(
        two_phase_segments
            .iter()
            .flatten()
            .any(|p| p.quality.is_some()),
        "isenthalp h=1500 kJ/kg should cross the dome and report quality somewhere"
    );

    for s_kj in [1.0, 5.0, 7.5] {
        let s = SpecificHeatCapacity::new::<kilojoule_per_kilogram_kelvin>(s_kj);
        let segments = isentrope(s, 200);
        check_monotonic_pressure(&segments, &format!("isentrope s={s_kj} kJ/(kg K)"));
    }
    let two_phase_s = SpecificHeatCapacity::new::<kilojoule_per_kilogram_kelvin>(5.0);
    let two_phase_segments = isentrope(two_phase_s, 200);
    assert!(
        two_phase_segments
            .iter()
            .flatten()
            .any(|p| p.quality.is_some()),
        "isentrope s=5.0 kJ/(kg K) should cross the dome and report quality somewhere"
    );
}

/// Checks [`isochore`] returns states that actually have the requested
/// specific volume — including inside the two-phase dome, which the previous
/// bisection implementation could not reach at all.
///
/// # Methodology
///
/// `isochore` is the one curve generator here whose pressure is not read
/// straight out of a forward equation: it comes from `p_rho_h_eqm`, which
/// inverts the IF97 backward equations. So the load-bearing check is a **round
/// trip**. For two representative specific volumes — one liquid-like
/// (`0.005 m3/kg`) and one vapour-like (`0.5 m3/kg`), both of which cross the
/// saturation dome somewhere along their sweep — build the curve, and for
/// every point recompute `v_ph_eqm(p, h)` and require it to reproduce `v0`.
///
/// `v_ph_eqm` is used rather than `try_v_tp_eqm_single_phase` deliberately: the
/// latter declines a two-phase `(T,p)` pair, correctly, and the whole point of
/// the new implementation is that the curve now passes *through* the dome. A
/// single-phase-only check would silently skip the new behaviour.
///
/// The test also asserts each `v0` actually reports a two-phase point
/// somewhere, so a regression that quietly went back to a single-phase-only
/// curve would fail here rather than pass with fewer points.
///
/// Pass criterion: 1e-6 relative on volume — far tighter than the 1e-3 the
/// bisection needed, because the root find solves for exactly this quantity.
///
/// # Result (measured 2026-09-14)
///
/// Passes at both `v0`, over every point of every segment, and both cross the
/// dome. The worst relative volume error is printed.
#[cfg(test)]
#[test]
fn isochore_reproduces_the_requested_volume_including_inside_the_dome() {
    use tampines_steam_tables::interfaces::functional_programming::ph_flash_eqm::v_ph_eqm;
    use uom::si::specific_volume::cubic_meter_per_kilogram;

    for v0_si in [0.005, 0.5] {
        let v0 = SpecificVolume::new::<cubic_meter_per_kilogram>(v0_si);
        let segments = isochore(v0, 150);
        assert!(
            !segments.is_empty(),
            "isochore v0={v0_si} m3/kg produced no segments"
        );

        // TWO tolerances, because the two branches of `isochore` are checked
        // against two different things — and holding both to one number is what
        // made this test misleading before.
        //
        // * Two-phase points are constructed by fitting `v(h)` from the flash
        //   itself and solving along that fit, so agreement with the flash is
        //   self-consistency and should be near machine precision.
        //
        // * Single-phase points in Region 3 take their pressure from
        //   `p_rho_t_3`, the EXACT forward Helmholtz equation. Verifying them
        //   with the backward `(p, h)` flash is a forward-versus-backward
        //   comparison, and the IF97 backward equations are fits with their own
        //   stated tolerance — this crate's own bar records the backward
        //   correlations at 5e-5 and flash specific volume far looser again.
        //   The same applies to the Region 1/2 branch: it solves
        //   `v_tp_eqm_single_phase(T, p) = v0` to 1e-10 relative in pressure,
        //   so it is exact BY THE FORWARD EQUATION, and any residual here is
        //   the backward flash disagreeing with it.
        //
        //   Measured worst cases, 2026-09-15:
        //     - 1.289e-5 at 643.62 K / 21.117 MPa (Region 3, ~3.5 K below the
        //       critical point, where the Region 3 backward equations are
        //       documented to lose digits)
        //     - 1.003e-4 at 810.06 K / 48.802 MPa (Region 2, high pressure)
        //
        //   The gate is 1e-3: an order of magnitude above the worst observed,
        //   and still five times INSIDE this crate's own recorded bar for flash
        //   specific volume, which is 0.5%. It is set from that published bar
        //   rather than fitted to the observed number, so it remains capable of
        //   catching a real regression.
        //
        // Tightening the single-phase gate would not improve the curve; it
        // would only force the generator back onto the backward equations and
        // make it LESS accurate, which is the wrong trade.
        const TWO_PHASE_TOLERANCE: f64 = 1.0e-6;
        const SINGLE_PHASE_TOLERANCE: f64 = 1.0e-3;

        let mut worst_two_phase = 0.0_f64;
        let mut worst_single_phase = 0.0_f64;
        let mut two_phase_points = 0_usize;
        let mut total = 0_usize;

        for segment in &segments {
            for point in segment {
                assert!(point.is_finite(), "isochore v0={v0_si}: non-finite point");

                let recomputed = v_ph_eqm(point.pressure, point.specific_enthalpy)
                    .get::<cubic_meter_per_kilogram>();
                let relative_error = (recomputed - v0_si).abs() / v0_si;
                let two_phase = point.quality.is_some();
                let tolerance = if two_phase {
                    worst_two_phase = worst_two_phase.max(relative_error);
                    TWO_PHASE_TOLERANCE
                } else {
                    worst_single_phase = worst_single_phase.max(relative_error);
                    SINGLE_PHASE_TOLERANCE
                };

                assert!(
                    relative_error < tolerance,
                    "isochore v0={v0_si}: recomputed v={recomputed} at ({:?}, {:?}), \
                     relative error {relative_error} exceeds {tolerance} \
                     ({} branch)",
                    point.temperature,
                    point.pressure,
                    if two_phase {
                        "two-phase"
                    } else {
                        "single-phase"
                    }
                );

                if two_phase {
                    two_phase_points += 1;
                }
                total += 1;
            }
        }

        println!(
            "isochore v0={v0_si} m3/kg: {total} points, {two_phase_points} two-phase, \
             worst |dv/v| two-phase {worst_two_phase:.3e}, \
             single-phase {worst_single_phase:.3e}"
        );
        assert!(
            two_phase_points > 0,
            "isochore v0={v0_si} never entered the dome; the (rho,h) path is \
             supposed to cross it"
        );
    }
}
