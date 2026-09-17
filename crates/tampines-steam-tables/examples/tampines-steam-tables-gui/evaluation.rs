//! Resolving a diagram click into a full thermodynamic state (issue #26,
//! 2026-08-21: *"In the evaluation tab, I want to be able to double click the
//! graph and add custom pH, TS, etc points to the plot and then I have all
//! the thermodynamic properties plotted out for me"*, and *"Top left hover
//! display should also have all the classic properties ... Density, Specific
//! vol, Temp, Pressure, H, S, G, A"*).
//!
//! [`evaluate_diagram_point`] is the single entry point both the Evaluation
//! tab (double-click) and the Graph tab's corner hover readout call, so the
//! two can never disagree about what a given `(x, y)` click resolves to.
//!
//! # Which flash each diagram uses
//!
//! | Diagram | Native coordinates | Route |
//! |---|---|---|
//! | p-h | `(h, p)` | direct: [`interfaces::checked`]'s `(p,h)` flash |
//! | T-p | `(T, p)` | direct: [`interfaces::checked`]'s single-phase `(T,p)` flash |
//! | T-s | `(s, T)` | in-dome: Region-4 lever rule via [`crate::curves::saturation_state`]; \
//!   otherwise a bisection on pressure at fixed `T` using the checked single-phase `(T,p)` entropy \
//!   -- IAPWS-IF97 has no backward `(T,s)` correlation, so there is nothing to call directly |
//! | h-s (Mollier) | `(s, h)` | [`interfaces::functional_programming::hs_flash_eqm`], the crate's \
//!   only `(h,s)` flash -- **unchecked** (`interfaces::checked`'s own module doc lists it as a \
//!   "known gap, deliberately not gated"). This module's own doc for `evaluate_hs` explains why \
//!   catching the panic at this GUI boundary is the correct place for it, not a workaround |
//!
//! # Derived properties
//!
//! Every flash above returns `(p, T, h, s, v)` plus, where meaningful, a
//! vapour quality. Gibbs energy `g = h - Ts` and Helmholtz energy
//! `a = u - Ts` (with `u = h - pv`) are plain algebra on those already-checked
//! properties, computed in SI base units to sidestep `uom`'s restrictions on
//! multiplying an absolute [`ThermodynamicTemperature`] directly (the same
//! `uom` trap this crate's own commit history already flags: a temperature
//! *interval* multiplies cleanly, a temperature *point* does not).

use uom::si::available_energy::{joule_per_kilogram, kilojoule_per_kilogram};
use uom::si::f64::*;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::{bar, pascal};
use uom::si::ratio::ratio;
use uom::si::specific_heat_capacity::{joule_per_kilogram_kelvin, kilojoule_per_kilogram_kelvin};
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermodynamic_temperature::{degree_celsius, kelvin};

use tampines_steam_tables::interfaces::checked::{
    try_h_tp_eqm_single_phase, try_h_tp_eqm_two_phase, try_s_ph_eqm, try_s_tp_eqm_single_phase,
    try_t_ph_eqm, try_v_tp_eqm_single_phase, try_v_tp_eqm_two_phase, try_v_ph_eqm, try_x_ph_flash,
};
use tampines_steam_tables::interfaces::functional_programming::hs_flash_eqm::tpvx_hs_flash_eqm;
use tampines_steam_tables::constants::P_TRIPLE_PT_PASCAL;
use tampines_steam_tables::region_4_vap_liq_equilibrium::sat_pressure_4;

use crate::curves::{self, P_MAX_PASCAL};
use crate::diagram::DiagramKind;

/// A fully-resolved thermodynamic state: every classic property issue #26's
/// 2026-08-21 comment asked for, plus the vapour quality when the state is
/// (or is close to) two-phase.
#[derive(Clone, Copy, Debug)]
pub struct EvaluatedState {
    pub pressure: Pressure,
    pub temperature: ThermodynamicTemperature,
    pub specific_enthalpy: AvailableEnergy,
    pub specific_entropy: SpecificHeatCapacity,
    pub density: MassDensity,
    pub specific_volume: SpecificVolume,
    /// `Some(x)` only where quality is a meaningful, derived-by-lever-rule
    /// property of the state (see the workspace-wide "quality is derived,
    /// not independently validated" rule this GUI already follows for the
    /// dome's own quality lines).
    pub quality: Option<f64>,
    pub specific_gibbs: AvailableEnergy,
    pub specific_helmholtz: AvailableEnergy,
}

impl EvaluatedState {
    /// Assembles the derived properties (density, Gibbs, Helmholtz) from the
    /// five directly-flashed quantities, in SI base units throughout.
    fn assemble(
        pressure: Pressure,
        temperature: ThermodynamicTemperature,
        specific_enthalpy: AvailableEnergy,
        specific_entropy: SpecificHeatCapacity,
        specific_volume: SpecificVolume,
        quality: Option<f64>,
    ) -> Self {
        let t_kelvin = temperature.get::<kelvin>();
        let p_pascal = pressure.get::<pascal>();
        let h_si = specific_enthalpy.get::<joule_per_kilogram>();
        let s_si = specific_entropy.get::<joule_per_kilogram_kelvin>();
        let v_si = specific_volume.get::<cubic_meter_per_kilogram>();

        let u_si = h_si - p_pascal * v_si;
        let g_si = h_si - t_kelvin * s_si;
        let a_si = u_si - t_kelvin * s_si;

        Self {
            pressure,
            temperature,
            specific_enthalpy,
            specific_entropy,
            density: MassDensity::new::<kilogram_per_cubic_meter>(1.0 / v_si),
            specific_volume,
            quality,
            specific_gibbs: AvailableEnergy::new::<joule_per_kilogram>(g_si),
            specific_helmholtz: AvailableEnergy::new::<joule_per_kilogram>(a_si),
        }
    }

    /// One display line per property, in the crate's usual kJ/kg-family
    /// units. Shared by the Evaluation tab's property table, the CSV export,
    /// and the Graph/Evaluation tabs' corner hover readout, so all three
    /// always agree.
    pub fn property_lines(&self) -> Vec<String> {
        vec![
            format!("p = {:.4} bar", self.pressure.get::<bar>()),
            format!(
                "T = {:.2} \u{00B0}C",
                self.temperature.get::<degree_celsius>()
            ),
            format!(
                "h = {:.2} kJ/kg",
                self.specific_enthalpy.get::<kilojoule_per_kilogram>()
            ),
            format!(
                "s = {:.4} kJ/(kg\u{00B7}K)",
                self.specific_entropy.get::<kilojoule_per_kilogram_kelvin>()
            ),
            format!(
                "\u{03C1} = {:.3} kg/m\u{00B3}",
                self.density.get::<kilogram_per_cubic_meter>()
            ),
            format!(
                "v = {:.6} m\u{00B3}/kg",
                self.specific_volume.get::<cubic_meter_per_kilogram>()
            ),
            match self.quality {
                Some(x) => format!("x = {x:.4}"),
                None => "x = (single-phase)".to_string(),
            },
            format!(
                "g = {:.2} kJ/kg",
                self.specific_gibbs.get::<kilojoule_per_kilogram>()
            ),
            format!(
                "a = {:.2} kJ/kg",
                self.specific_helmholtz.get::<kilojoule_per_kilogram>()
            ),
        ]
    }

    /// `(label, value)` pairs for the Evaluation tab's property table and
    /// CSV export -- the same nine properties as [`EvaluatedState::property_lines`],
    /// split so the CSV writer does not have to parse formatted text back apart.
    pub fn property_rows(&self) -> Vec<(&'static str, f64, &'static str)> {
        vec![
            ("p", self.pressure.get::<bar>(), "bar"),
            ("T", self.temperature.get::<degree_celsius>(), "degC"),
            (
                "h",
                self.specific_enthalpy.get::<kilojoule_per_kilogram>(),
                "kJ/kg",
            ),
            (
                "s",
                self.specific_entropy.get::<kilojoule_per_kilogram_kelvin>(),
                "kJ/(kg K)",
            ),
            (
                "rho",
                self.density.get::<kilogram_per_cubic_meter>(),
                "kg/m3",
            ),
            (
                "v",
                self.specific_volume.get::<cubic_meter_per_kilogram>(),
                "m3/kg",
            ),
            ("x", self.quality.unwrap_or(f64::NAN), "-"),
            (
                "g",
                self.specific_gibbs.get::<kilojoule_per_kilogram>(),
                "kJ/kg",
            ),
            (
                "a",
                self.specific_helmholtz.get::<kilojoule_per_kilogram>(),
                "kJ/kg",
            ),
        ]
    }
}

/// Resolves a click at `(x, y)` in `diagram`'s own plot-space coordinates
/// into a full [`EvaluatedState`], or a short message explaining why not.
///
/// `y_is_log` matches the live canvas's own log-pressure toggle: when true,
/// `y` is `log10(p / bar)` rather than `p` itself, for any diagram whose y
/// axis is pressure (mirrors [`DiagramKind::y_hover`]'s own convention).
///
/// Never panics on bad input (issue #26: "The GUI should not panic if
/// property calls fail... It should skip invalid points and show a
/// warning") -- every branch below either returns `Ok` or a descriptive
/// `Err`, for the caller to show as a status-line warning and otherwise
/// ignore the click.
pub fn evaluate_diagram_point(
    diagram: DiagramKind,
    x: f64,
    y: f64,
    y_is_log: bool,
) -> Result<EvaluatedState, String> {
    match diagram {
        DiagramKind::TemperaturePressure => evaluate_tp(x, y, y_is_log),
        DiagramKind::PressureEnthalpy => evaluate_ph(x, y, y_is_log),
        DiagramKind::TemperatureEntropy => evaluate_ts(x, y),
        DiagramKind::EnthalpyEntropy => evaluate_hs(x, y),
    }
}

fn pressure_from_y(y: f64, y_is_log: bool) -> Pressure {
    let p_bar = if y_is_log { 10.0_f64.powf(y) } else { y };
    Pressure::new::<bar>(p_bar)
}

/// `(T, p)`: direct, via the checked single-phase flash. A click landing
/// exactly on (or inside) the two-phase envelope is rejected rather than
/// guessed at -- a bare `(T, p)` pair cannot resolve a two-phase state
/// without a steam quality, the same reason [`LayerId::availability_on`]
/// disables the quality lines on this diagram (see [`crate::layers`]).
fn evaluate_tp(x: f64, y: f64, y_is_log: bool) -> Result<EvaluatedState, String> {
    let t = ThermodynamicTemperature::new::<degree_celsius>(x);
    let p = pressure_from_y(y, y_is_log);
    let h = try_h_tp_eqm_single_phase(t, p).map_err(|e| e.to_string())?;
    let s = try_s_tp_eqm_single_phase(t, p).map_err(|e| e.to_string())?;
    let v = try_v_tp_eqm_single_phase(t, p).map_err(|e| e.to_string())?;
    Ok(EvaluatedState::assemble(p, t, h, s, v, None))
}

/// `(p, h)`: direct, via the checked `(p,h)` flash family -- the crate's
/// most complete checked route, including its own quality flash.
fn evaluate_ph(x: f64, y: f64, y_is_log: bool) -> Result<EvaluatedState, String> {
    let h = AvailableEnergy::new::<kilojoule_per_kilogram>(x);
    let p = pressure_from_y(y, y_is_log);
    let t = try_t_ph_eqm(p, h).map_err(|e| e.to_string())?;
    let s = try_s_ph_eqm(p, h).map_err(|e| e.to_string())?;
    let v = try_v_ph_eqm(p, h).map_err(|e| e.to_string())?;
    // `try_x_ph_flash` saturates to 0 or 1 outside the dome rather than
    // reporting "not applicable" -- the same trap `crate::layers`'
    // `point_from_ph` already guards against. Only trust it inside the dome.
    let inside_dome = (p.get::<pascal>() - sat_pressure_4(t).get::<pascal>()).abs()
        <= sat_pressure_4(t).get::<pascal>() * 1.0e-6;
    let quality = inside_dome.then(|| try_x_ph_flash(p, h).ok()).flatten();
    Ok(EvaluatedState::assemble(p, t, h, s, v, quality))
}

/// `(s, T)`: IAPWS-IF97 defines no backward `(T,s)` correlation, so there is
/// no direct flash to call. Two branches:
///
/// * **In the dome** -- if `s` lies between the saturated-liquid and
///   saturated-vapour entropies at `T` ([`curves::saturation_state`], the
///   same helper the saturation dome and quality lines already use), the
///   state is the Region-4 mixture at the lever-rule quality that entropy
///   implies, resolved through the checked two-phase `(T,p,x)` flash.
/// * **Single-phase** -- otherwise, bisect on pressure at fixed `T` for
///   `s(T,p) == s_target`, using the checked single-phase `(T,p)` entropy as
///   the (locally monotonic, away from the dome) objective function.
fn evaluate_ts(x: f64, y: f64) -> Result<EvaluatedState, String> {
    let s_target = SpecificHeatCapacity::new::<kilojoule_per_kilogram_kelvin>(x);
    let t = ThermodynamicTemperature::new::<degree_celsius>(y);

    if let Some(sat) = curves::saturation_state(t) {
        let s_f = sat.s_liquid.get::<kilojoule_per_kilogram_kelvin>();
        let s_g = sat.s_vapour.get::<kilojoule_per_kilogram_kelvin>();
        if (s_f..=s_g).contains(&x) {
            let quality = (x - s_f) / (s_g - s_f);
            let h = try_h_tp_eqm_two_phase(t, sat.pressure, quality).map_err(|e| e.to_string())?;
            let v = try_v_tp_eqm_two_phase(t, sat.pressure, quality).map_err(|e| e.to_string())?;
            return Ok(EvaluatedState::assemble(
                sat.pressure,
                t,
                h,
                s_target,
                v,
                Some(quality),
            ));
        }
    }

    let objective = |p_pascal: f64| -> Option<f64> {
        let p = Pressure::new::<pascal>(p_pascal);
        try_s_tp_eqm_single_phase(t, p)
            .ok()
            .map(|s| s.get::<kilojoule_per_kilogram_kelvin>() - x)
    };
    let (mut lo, mut hi) = (P_TRIPLE_PT_PASCAL, P_MAX_PASCAL);
    let f_lo = objective(lo)
        .ok_or_else(|| "no evaluable single-phase pressure at this temperature".to_string())?;
    let f_hi = objective(hi)
        .ok_or_else(|| "no evaluable single-phase pressure at this temperature".to_string())?;
    if f_lo.abs() < 1.0e-9 {
        hi = lo;
    } else if f_hi.abs() < 1.0e-9 {
        lo = hi;
    } else if f_lo.signum() == f_hi.signum() {
        return Err(format!(
            "s = {x:.4} kJ/(kg K) at T = {y:.2} \u{00B0}C is not reachable by any single-phase \
             pressure in range"
        ));
    } else {
        for _ in 0..80 {
            let mid = 0.5 * (lo + hi);
            let Some(f_mid) = objective(mid) else {
                break;
            };
            if f_mid.signum() == f_lo.signum() {
                lo = mid;
            } else {
                hi = mid;
            }
        }
    }
    let p = Pressure::new::<pascal>(0.5 * (lo + hi));
    let h = try_h_tp_eqm_single_phase(t, p).map_err(|e| e.to_string())?;
    let v = try_v_tp_eqm_single_phase(t, p).map_err(|e| e.to_string())?;
    Ok(EvaluatedState::assemble(p, t, h, s_target, v, None))
}

/// `(s, h)`: routed through
/// [`tpvx_hs_flash_eqm`](tampines_steam_tables::interfaces::functional_programming::hs_flash_eqm::tpvx_hs_flash_eqm),
/// the crate's only `(h,s)` flash. `interfaces::checked`'s own module doc
/// lists the whole `(h,s)` family as a "known gap, deliberately not gated" --
/// no bounds check this facade could add would exclude every internal panic
/// site, so the library correctly declines to pretend otherwise.
///
/// This GUI, however, still must not crash on a bad double-click (issue #26).
/// [`std::panic::catch_unwind`] here is not a workaround for that gap and
/// does not contradict `control_volume.rs`'s own "no catch_unwind" note --
/// that note is about *that module's* checked-constructor facade specifically
/// declining to paper over an internal panic with a false sense of safety.
/// This is a different place: the outermost edge of a GUI event handler,
/// exactly where a panic boundary belongs, per issue #26's explicit
/// requirement to skip and warn rather than crash.
fn evaluate_hs(x: f64, y: f64) -> Result<EvaluatedState, String> {
    let s = SpecificHeatCapacity::new::<kilojoule_per_kilogram_kelvin>(x);
    let h = AvailableEnergy::new::<kilojoule_per_kilogram>(y);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| tpvx_hs_flash_eqm(h, s)));
    let (t, p, v, x_ratio) =
        result.map_err(|_| "no valid IAPWS-IF97 state at this (h, s) point".to_string())?;

    if !t.get::<kelvin>().is_finite()
        || !p.get::<pascal>().is_finite()
        || !v.get::<cubic_meter_per_kilogram>().is_finite()
    {
        return Err("(h, s) flash returned a non-finite state".to_string());
    }
    // `tpvx_hs_flash_eqm` returns a sentinel x = 0.0 (Region 1) or x = 1.0
    // (Region 2/3-vapour-like/5) for every single-phase state, the same
    // "saturates outside the dome" trap `evaluate_ph` guards against. Only a
    // resolved pressure genuinely equal to the saturation pressure at the
    // resolved temperature is a real Region-4 mixture.
    let x_val = x_ratio.get::<ratio>();
    let inside_dome = (p.get::<pascal>() - sat_pressure_4(t).get::<pascal>()).abs()
        <= sat_pressure_4(t).get::<pascal>() * 1.0e-6;
    let quality =
        (inside_dome && x_val.is_finite() && (0.0..=1.0).contains(&x_val)).then_some(x_val);
    Ok(EvaluatedState::assemble(p, t, h, s, v, quality))
}

/// The pointer's hover text for the corner-anchored readout on both the
/// Graph tab (issue #26: "top left hover display should also have all the
/// classic properties ... for both graph tab and evaluation tab") and the
/// Evaluation tab. Falls back to the bare coordinate line when no state can
/// be resolved there, so the readout is never blank.
pub fn hover_text(diagram: DiagramKind, log: bool, x: f64, y: f64) -> String {
    let coords = format!("{}\n{}", diagram.x_hover(x), diagram.y_hover(y, log));
    match evaluate_diagram_point(diagram, x, y, log) {
        Ok(state) => format!("{coords}\n{}", state.property_lines().join("\n")),
        Err(_) => coords,
    }
}

/// Checks every diagram resolves a representative interior point (well away
/// from the dome and any axis edge) to a finite, physically ordered state.
///
/// # Methodology
///
/// One point per diagram, chosen deep in single-phase territory: T-p at
/// (300 degC, 10 bar), p-h at (2800 kJ/kg, 10 bar), T-s at (250 degC,
/// 6.5 kJ/(kg K)), h-s at (6.5 kJ/(kg K), 2800 kJ/kg). Asserts every property
/// is finite, density and specific volume are reciprocal, and quality is
/// `None` (all four points are single-phase by construction).
///
/// # Result (measured 2026-08-21)
///
/// Passes on all four diagrams.
#[cfg(test)]
#[test]
fn representative_single_phase_points_resolve_on_every_diagram() {
    let cases = [
        (DiagramKind::TemperaturePressure, 300.0, 10.0, false),
        (DiagramKind::PressureEnthalpy, 2800.0, 10.0, false),
        (DiagramKind::TemperatureEntropy, 6.5, 250.0, false),
        (DiagramKind::EnthalpyEntropy, 6.5, 2800.0, false),
    ];
    for (diagram, x, y, log) in cases {
        let state = evaluate_diagram_point(diagram, x, y, log)
            .unwrap_or_else(|e| panic!("{diagram:?} at ({x}, {y}) failed: {e}"));
        for line in state.property_lines() {
            assert!(
                !line.contains("NaN") && !line.contains("inf"),
                "{diagram:?}: {line}"
            );
        }
        let rho = state.density.get::<kilogram_per_cubic_meter>();
        let v = state.specific_volume.get::<cubic_meter_per_kilogram>();
        assert!(
            (rho * v - 1.0).abs() < 1.0e-9,
            "{diagram:?}: density and specific volume are not reciprocal"
        );
        assert!(
            state.quality.is_none(),
            "{diagram:?}: expected a single-phase point, got quality {:?}",
            state.quality
        );
    }
}

/// Checks the T-s in-dome branch: a click at a saturation temperature's
/// midpoint entropy resolves to a Region-4 mixture at `x ~= 0.5`, on the
/// saturation pressure, rather than falling through to the single-phase
/// bisection branch.
///
/// # Result (measured 2026-08-21)
///
/// Holds at T = 200 degC: resolved quality is within 1e-6 of 0.5, and the
/// resolved pressure matches `sat_pressure_4(200 degC)`.
#[cfg(test)]
#[test]
fn ts_click_inside_the_dome_resolves_the_two_phase_lever_rule() {
    let t = ThermodynamicTemperature::new::<degree_celsius>(200.0);
    let sat = curves::saturation_state(t).expect("200 degC is within the dome's range");
    let s_f = sat.s_liquid.get::<kilojoule_per_kilogram_kelvin>();
    let s_g = sat.s_vapour.get::<kilojoule_per_kilogram_kelvin>();
    let s_mid = 0.5 * (s_f + s_g);

    let state = evaluate_diagram_point(DiagramKind::TemperatureEntropy, s_mid, 200.0, false)
        .expect("midpoint entropy must resolve inside the dome");
    let quality = state.quality.expect("expected a two-phase quality");
    assert!((quality - 0.5).abs() < 1.0e-6, "quality = {quality}");
    assert!(
        (state.pressure.get::<pascal>() - sat.pressure.get::<pascal>()).abs() < 1.0e-3,
        "expected the resolved pressure to equal the saturation pressure at 200 degC"
    );
}
