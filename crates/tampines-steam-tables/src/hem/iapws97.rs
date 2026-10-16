//! IAPWS-IF97 water/steam as a HEM equation of state, [`Iapws97Hem`].
//!
//! # The `(ρ, e)` closure
//!
//! The solver carries `ρ` and `e`; IF97 has no `(ρ, e)` backward equation. The
//! state is recovered as a **one-dimensional bracketed root in pressure**
//! (`petir::roots`, Brent): find `p` such that
//!
//! ```text
//! g(p) = ρ · v(p, h = e + p/ρ) − 1 = 0
//! ```
//!
//! with `v(p, h)` this crate's explicit `(p,h)` flash, [`v_ph_eqm`]. Any root
//! is an equilibrium state with the requested `(v, e)` (because
//! `e = h − p·v = e + p/ρ − p/ρ`), and the equilibrium state for a given
//! `(v, e)` is unique, so the sign change is unique too. The same strategy as
//! [`crate::interfaces::functional_programming::rho_h_flash_eqm`], which inverts
//! `(ρ, h)`; that module cannot be reused directly because `h` is not known
//! here until `p` is.
//!
//! The inversion adds numerics, not thermodynamics: every property comes from
//! the IF97 `(p,h)` flash. Its tolerance is [`P_RHO_E_REL_TOL`] relative in
//! pressure, against this crate's own `v(p,h)`.
//!
//! # Consistency of the initial state
//!
//! A state built from `(p, T)` is routed through `h(T, p)` and then the same
//! `(p, h)` flash, so `ρ = 1/v(p, h)` and `e = h − p/ρ` and the first `(ρ, e)`
//! inversion returns the initial pressure. Building `ρ` from the forward
//! `v(T, p)` instead would differ from the backward `v(p, h)` by the IF97
//! backward-equation consistency error (`~1e-6` relative in liquid), which a
//! stiff liquid turns into kilopascals.
//!
//! # Range
//!
//! The `(p, h)` envelope of [`check_ph_envelope`]: `p` from `p_sat(273.15 K)`
//! to 100 MPa, `h` between the 273.15 K and 1073.15 K isotherms. Region 5 is
//! out of range. Outside it every call returns
//! [`HemError::EquationOfState`]; nothing here panics on a state outside the
//! envelope.

use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{
    AvailableEnergy, MassDensity, MassFlux, Pressure, Ratio, SpecificVolume,
    ThermodynamicTemperature, Velocity,
};
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::mass_flux::kilogram_per_square_meter_second;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::velocity::meter_per_second;

use super::eos::{pressure_from_rho_e, require_positive, BreakFlow, HemEquationOfState, HemThermoState};
use super::HemError;
use crate::constants::RHO_C_KG_PER_M3;
use crate::interfaces::checked::{
    check_ph_envelope, check_tp_single_phase_envelope, try_h_ps_eqm, try_rho_ps_eqm, try_s_ph_eqm,
    SteamTablesError,
};
use crate::interfaces::functional_programming::ph_flash_eqm::{
    ph_flash_region, t_ph_eqm, v_ph_eqm, w_ph_eqm_hem, x_ph_flash,
};
use crate::interfaces::functional_programming::pt_flash_eqm::{h_tp_eqm_single_phase, FwdEqnRegion};
use crate::region_2_vapour::v_tp_2;
use crate::region_4_vap_liq_equilibrium::{sat_pressure_4, sat_temp_4};
use crate::steam_turbine_equations::converging_diverging_nozzles::choked_flow::get_critical_pressure_and_mass_flux_multiphase_ph;

/// Relative tolerance on pressure of the `(ρ, e)` inversion: the returned `p`
/// is within `P_RHO_E_REL_TOL · p` of the root of `ρ·v(p, e + p/ρ) = 1`
/// (the bracket width at convergence). A numerical tolerance only; it says
/// nothing about IF97's own uncertainty.
pub const P_RHO_E_REL_TOL: f64 = 1.0e-12;

/// Upper pressure bound of the IF97 `(p,h)` domain, Pa.
const P_MAX_PA: f64 = 100.0e6;

/// IAPWS-IF97 water and steam (this crate) as a HEM closure.
///
/// - `(ρ, e) → p`: bracketed Brent root over the `(p,h)` flash (module doc).
/// - Sound speed: [`w_ph_eqm_hem`], the homogeneous-**equilibrium** speed
///   (Kieffer 1977 eq. 28 inside the dome), never the frozen
///   [`crate::interfaces::functional_programming::ph_flash_eqm::w_ph_wood_wallis`].
/// - Choked break: [`get_critical_pressure_and_mass_flux_multiphase_ph`], the
///   Moody/Zaloudek-verified HEM critical-flow dispatcher.
///
/// Valid 273.15–1073.15 K, `p_sat(273.15 K)`–100 MPa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Iapws97Hem;

fn steam_err(context: &str, e: SteamTablesError) -> HemError {
    HemError::EquationOfState {
        cell: None,
        detail: format!("{context}: {e}"),
    }
}

impl Iapws97Hem {
    /// Lower pressure bound of the `(p,h)` domain, `p_sat(273.15 K)`, Pa.
    fn p_min_pa() -> f64 {
        sat_pressure_4(ThermodynamicTemperature::new::<kelvin>(273.15)).get::<pascal>()
    }

    /// `v(p, h)` [m³/kg], or `None` outside the `(p,h)` envelope.
    fn specific_volume(p: Pressure, h: AvailableEnergy) -> Option<SpecificVolume> {
        check_ph_envelope(p, h).ok()?;
        Some(v_ph_eqm(p, h))
    }

    /// Homogeneous no-slip void fraction of a `(p, h)` state already known to
    /// be inside the envelope, given its mixture density `rho`.
    fn void_fraction(region: FwdEqnRegion, p: Pressure, x: f64, rho: f64) -> f64 {
        match region {
            FwdEqnRegion::Region1 => 0.0,
            FwdEqnRegion::Region2 | FwdEqnRegion::Region5 => 1.0,
            // Single-phase near-critical/supercritical fluid: classed by which
            // side of the critical density it sits on.
            FwdEqnRegion::Region3 => {
                if rho >= RHO_C_KG_PER_M3 {
                    0.0
                } else {
                    1.0
                }
            }
            FwdEqnRegion::Region4 => {
                let rho_g = v_tp_2(sat_temp_4(p), p)
                    .get::<cubic_meter_per_kilogram>()
                    .recip();
                (x * rho / rho_g).clamp(0.0, 1.0)
            }
        }
    }
}

impl HemEquationOfState for Iapws97Hem {
    fn name(&self) -> &'static str {
        "IAPWS-IF97"
    }

    fn state_rho_e(
        &self,
        density: MassDensity,
        specific_internal_energy: AvailableEnergy,
        pressure_hint: Option<Pressure>,
    ) -> Result<HemThermoState, HemError> {
        let rho = require_positive(
            "density",
            density.get::<kilogram_per_cubic_meter>(),
            "kg/m3",
        )?;
        let e = specific_internal_energy.get::<joule_per_kilogram>();
        if !e.is_finite() {
            return Err(HemError::EquationOfState {
                cell: None,
                detail: format!("specific internal energy = {e} J/kg is not finite"),
            });
        }
        let p = pressure_from_rho_e(
            density,
            specific_internal_energy,
            pressure_hint,
            (
                Pressure::new::<pascal>(Self::p_min_pa()),
                Pressure::new::<pascal>(P_MAX_PA),
            ),
            P_RHO_E_REL_TOL,
            Self::specific_volume,
        )?
        .get::<pascal>();
        let mut st = self.state_p_h(
            Pressure::new::<pascal>(p),
            AvailableEnergy::new::<joule_per_kilogram>(e + p / rho),
        )?;
        // The conserved variables are the definition of the state; report them
        // exactly rather than the flash's round trip of them.
        st.density = density;
        st.specific_internal_energy = specific_internal_energy;
        Ok(st)
    }

    fn state_p_t(
        &self,
        pressure: Pressure,
        temperature: ThermodynamicTemperature,
    ) -> Result<HemThermoState, HemError> {
        check_tp_single_phase_envelope(temperature, pressure)
            .map_err(|e| steam_err("IF97 (p, T) state", e))?;
        // Route through (p, h) so ρ and e are consistent with the (ρ, e)
        // inversion (module doc, "Consistency of the initial state").
        self.state_p_h(pressure, h_tp_eqm_single_phase(temperature, pressure))
    }

    fn state_p_h(
        &self,
        pressure: Pressure,
        specific_enthalpy: AvailableEnergy,
    ) -> Result<HemThermoState, HemError> {
        check_ph_envelope(pressure, specific_enthalpy)
            .map_err(|e| steam_err("IF97 (p, h) state", e))?;
        let p = pressure.get::<pascal>();
        let h = specific_enthalpy.get::<joule_per_kilogram>();
        let rho = v_ph_eqm(pressure, specific_enthalpy)
            .get::<cubic_meter_per_kilogram>()
            .recip();
        let region = ph_flash_region(pressure, specific_enthalpy);
        let x = x_ph_flash(pressure, specific_enthalpy);
        let c = w_ph_eqm_hem(pressure, specific_enthalpy).get::<meter_per_second>();
        if !(c.is_finite() && c > 0.0) {
            return Err(HemError::EquationOfState {
                cell: None,
                detail: format!(
                    "IF97 equilibrium sound speed is {c} m/s at p = {p} Pa, h = {h} J/kg \
                     (both Region 4 closures failed; near the critical point?)"
                ),
            });
        }
        let quality = match region {
            FwdEqnRegion::Region4 => Some(Ratio::new::<ratio>(x)),
            _ => None,
        };
        Ok(HemThermoState {
            pressure,
            temperature: t_ph_eqm(pressure, specific_enthalpy),
            density: MassDensity::new::<kilogram_per_cubic_meter>(rho),
            specific_internal_energy: AvailableEnergy::new::<joule_per_kilogram>(h - p / rho),
            specific_enthalpy,
            equilibrium_sound_speed: Velocity::new::<meter_per_second>(c),
            quality,
            void_fraction: Ratio::new::<ratio>(Self::void_fraction(region, pressure, x, rho)),
        })
    }

    /// Not provided for IF97: no Sod-style `(p, ρ)` input is needed for water,
    /// and the inversion would be a second root find. Use `(p, T)` or `(p, h)`.
    fn state_p_rho(
        &self,
        _pressure: Pressure,
        _density: MassDensity,
    ) -> Result<HemThermoState, HemError> {
        Err(HemError::Unsupported {
            eos: self.name(),
            what: "a (p, rho) state; give (p, T) or (p, h)",
        })
    }

    /// HEM break flow from stagnation `(p0, h0)` to `back_pressure`.
    ///
    /// - `p0 ≤ back_pressure`: zero flow (backflow is not modelled).
    /// - Otherwise the critical `(p*, G*)` of
    ///   [`get_critical_pressure_and_mass_flux_multiphase_ph`]. If
    ///   `p* > back_pressure` the break is choked at `G*`.
    /// - If not choked, the subcritical isentropic nozzle flux to the back
    ///   pressure, `G = ρ(p_b, s0)·√(2(h0 − h(p_b, s0)))`.
    ///
    /// **Panics** if the critical-flow dispatcher fails to bracket its own
    /// root ("unable to find bracket for critical mass flux root finding");
    /// that dispatcher has no `Result` path yet (see the "Known gaps" of
    /// [`crate::interfaces::checked`]), and this function does not wrap a
    /// panic with `catch_unwind`. The stagnation state itself is checked first.
    fn break_flow(
        &self,
        stagnation_pressure: Pressure,
        stagnation_enthalpy: AvailableEnergy,
        back_pressure: Pressure,
    ) -> Result<BreakFlow, HemError> {
        let p0 = stagnation_pressure.get::<pascal>();
        let pb = back_pressure.get::<pascal>();
        if p0 <= pb {
            return Ok(BreakFlow {
                mass_flux: MassFlux::new::<kilogram_per_square_meter_second>(0.0),
                choked: false,
            });
        }
        check_ph_envelope(stagnation_pressure, stagnation_enthalpy)
            .map_err(|e| steam_err("IF97 break stagnation state", e))?;
        let (p_crit, g_crit) = get_critical_pressure_and_mass_flux_multiphase_ph(
            stagnation_pressure,
            stagnation_enthalpy,
        );
        if p_crit.get::<pascal>() > pb {
            return Ok(BreakFlow {
                mass_flux: g_crit,
                choked: true,
            });
        }
        let s0 = try_s_ph_eqm(stagnation_pressure, stagnation_enthalpy)
            .map_err(|e| steam_err("IF97 break stagnation entropy", e))?;
        let h_b = try_h_ps_eqm(back_pressure, s0)
            .map_err(|e| steam_err("IF97 break exit enthalpy", e))?
            .get::<joule_per_kilogram>();
        let rho_b = try_rho_ps_eqm(back_pressure, s0)
            .map_err(|e| steam_err("IF97 break exit density", e))?
            .get::<kilogram_per_cubic_meter>();
        let dh = stagnation_enthalpy.get::<joule_per_kilogram>() - h_b;
        if !(dh >= 0.0) {
            return Err(HemError::EquationOfState {
                cell: None,
                detail: format!(
                    "IF97 isentropic expansion from p0 = {p0} Pa to p_b = {pb} Pa gave \
                     h0 - h_b = {dh} J/kg < 0"
                ),
            });
        }
        Ok(BreakFlow {
            mass_flux: MassFlux::new::<kilogram_per_square_meter_second>(rho_b * (2.0 * dh).sqrt()),
            choked: false,
        })
    }
}
