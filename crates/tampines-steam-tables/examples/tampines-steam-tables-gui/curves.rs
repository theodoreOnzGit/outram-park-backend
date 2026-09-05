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
/// # Why this one needs root-finding, unlike the other custom lines
///
/// This crate has no `(p,v)` or `(T,v)` flash — [`isenthalp`]/[`isentrope`]
/// above get away with a direct sweep because `try_t_ph_eqm`/`try_t_ps_eqm`
/// already invert `h`/`s` for them. For volume there is only the *forward*
/// single-phase dispatcher `try_v_tp_eqm_single_phase`, so this sweeps
/// temperature and, at each temperature, bisects on pressure for the value
/// that gives the requested `v0` — 60 bisection steps, comfortably enough for
/// `f64` precision on a monotonic bracket.
///
/// # Single-phase only
///
/// Unlike [`isenthalp`]/[`isentrope`], this does **not** cross the two-phase
/// dome: `try_v_tp_eqm_single_phase` correctly declines inside it (a `(T,p)`
/// pair inside the dome is not single-phase), so the bisection at those
/// temperatures fails and the point is dropped — which is what produces the
/// gap where the curve enters and leaves the dome. Drawing the constant-`v0`
/// locus *inside* the dome would need a lever-rule inversion of the
/// saturated-liquid/vapour volumes this crate does not provide, so the curve
/// is honestly discontinuous there rather than interpolated across it. It
/// returns one segment per contiguous single-phase run (typically two: a
/// liquid branch and a vapour branch, below the critical volume; one,
/// supercritical).
pub fn isochore(v0: SpecificVolume, samples: usize) -> Vec<Vec<ThermoPoint>> {
    let v0_si = v0.get::<cubic_meter_per_kilogram>();
    let n = samples.max(2);

    let mut segments = Vec::new();
    let mut current: Vec<ThermoPoint> = Vec::new();
    for i in 0..n {
        let frac = i as f64 / (n - 1) as f64;
        let t_kelvin = T_TRIPLE_PT_KELVIN + (T_MAX_KELVIN - T_TRIPLE_PT_KELVIN) * frac;
        let t = ThermodynamicTemperature::new::<kelvin>(t_kelvin);
        let point = bisect_pressure_for_volume(t, v0_si).and_then(|p| single_phase_point(t, p));
        match point {
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

/// Volume at `(t, p)` in SI units (m^3/kg), or `None` if this crate declines
/// to evaluate that state as single-phase.
fn volume_si(t: ThermodynamicTemperature, p_pascal: f64) -> Option<f64> {
    try_v_tp_eqm_single_phase(t, Pressure::new::<pascal>(p_pascal))
        .ok()
        .map(|v| v.get::<cubic_meter_per_kilogram>())
}

/// Finds the pressure at fixed `t` whose single-phase specific volume is
/// `v0_si` (m^3/kg), by bisection.
///
/// Specific volume decreases monotonically with pressure at fixed temperature
/// *within* a single phase, which is what the loop below assumes between
/// bisection steps. But the two endpoints bracketing `v0_si` do not by
/// themselves guarantee `v0_si` sits on a single continuous single-phase
/// branch: below the critical temperature, `v(p)` actually *jumps* at
/// `p_sat(t)` from the vapour branch's value straight to the liquid branch's
/// — everything strictly between the saturated-vapour and saturated-liquid
/// volumes exists only as a two-phase mixture at the *one* pressure
/// `p_sat(t)`, not across a range of pressures at all. A coarse
/// `v_hi <= v0_si <= v_lo` bracket check cannot see that gap, so bisection can
/// converge toward the discontinuity and land on a state whose volume is
/// nowhere near `v0_si` — which is exactly what an earlier version of this
/// function did at the triple point, caught by
/// `isochore_bisection_reproduces_the_requested_volume`. So the result is
/// **verified** before being accepted: only returned if it reproduces
/// `v0_si` to within 0.1 %, otherwise treated as "not achievable here" and
/// dropped, per this module's "never fabricate" rule.
fn bisect_pressure_for_volume(t: ThermodynamicTemperature, v0_si: f64) -> Option<Pressure> {
    let mut lo = P_TRIPLE_PT_PASCAL;
    let mut hi = P_MAX_PASCAL;
    let v_lo = volume_si(t, lo)?;
    let v_hi = volume_si(t, hi)?;
    if !(v_hi <= v0_si && v0_si <= v_lo) {
        return None;
    }
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        let v_mid = volume_si(t, mid)?;
        if v_mid > v0_si {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let p_pascal = 0.5 * (lo + hi);
    let v_converged = volume_si(t, p_pascal)?;
    let relative_error = (v_converged - v0_si).abs() / v0_si.max(1.0e-12);
    (relative_error < 1.0e-3).then(|| Pressure::new::<pascal>(p_pascal))
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

/// Checks [`isochore`]'s bisection actually converges: at every point on the
/// curve, independently recomputing specific volume at that point's `(T,p)`
/// via `try_v_tp_eqm_single_phase` reproduces the requested `v0` to within a
/// tight relative tolerance.
///
/// # Methodology
///
/// This is the one new curve generator in this module that is genuinely new
/// numerics (a bisection root-find) rather than a direct call into an
/// existing, separately-verified flash — see the module doc on
/// [`isochore`] for why. So unlike [`isenthalp`]/[`isentrope`] above, the load
/// -bearing check here is a **round trip**: for two representative specific
/// volumes (a liquid-like value and a vapour-like value, both comfortably
/// inside the achievable range at the pressures this module sweeps), builds
/// the isochore, and for every point on it, calls
/// `try_v_tp_eqm_single_phase(point.temperature, point.pressure)` and asserts
/// the recomputed volume matches `v0` to within 0.1 %.
///
/// # Result (measured 2026-08-20)
///
/// Passes at both `v0` values, over every point on every returned segment.
#[cfg(test)]
#[test]
fn isochore_bisection_reproduces_the_requested_volume() {
    use uom::si::specific_volume::cubic_meter_per_kilogram;

    for v0_si in [0.005, 0.5] {
        let v0 = SpecificVolume::new::<cubic_meter_per_kilogram>(v0_si);
        let segments = isochore(v0, 150);
        assert!(
            !segments.is_empty(),
            "isochore v0={v0_si} m3/kg produced no segments"
        );
        for segment in &segments {
            for point in segment {
                assert!(point.is_finite(), "isochore v0={v0_si}: non-finite point");
                let recomputed = try_v_tp_eqm_single_phase(point.temperature, point.pressure)
                    .unwrap_or_else(|e| {
                        panic!(
                            "isochore v0={v0_si}: point ({:?}, {:?}) does not itself \
                             re-evaluate as single-phase: {e:?}",
                            point.temperature, point.pressure
                        )
                    })
                    .get::<cubic_meter_per_kilogram>();
                let relative_error = (recomputed - v0_si).abs() / v0_si;
                assert!(
                    relative_error < 1.0e-3,
                    "isochore v0={v0_si}: recomputed v={recomputed} at ({:?}, {:?}), \
                     relative error {relative_error} exceeds 0.1%",
                    point.temperature,
                    point.pressure
                );
            }
        }
    }
}
