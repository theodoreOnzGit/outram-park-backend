//! Reduced equations of state for verification: stiffened gas, isentropic
//! stiffened gas, barotropic and linear (gh:#1057, RELAP-7 parity epic
//! gh:#1056). AI draft, clean room: written from the RELAP-7 Theory Manual's
//! description (INL/EXT-14-31366, §2.2.3.1–2.2.3.4, Eqs 54–93; §3.2.5,
//! Eqs 209–220) and the papers it cites, never from RELAP-7 source.
//!
//! **These are verification closures, not substitutes for IAPWS-IF97.**
//! Water and steam already have a full equation of state, [`super::Iapws97Hem`].
//! Each form here is a small analytic closure that runs through the same
//! [`super::HemPipe`] solver, for code-to-code checks against RELAP-7's
//! formulation and for the seven-equation model (gh:#1059), which needs a
//! per-phase convex EOS with a closed-form saturation curve.
//!
//! - [`StiffenedGasHem`]: `p = (γ − 1)ρ(e − q) − γp∞` (Eq 78), with its caloric
//!   law `ρ(p, T) = (p + p∞)/((γ − 1)c_v T)` (Eq 80) and Gibbs energy (Eq 82).
//!   Parameters for water and steam are Le Métayer, Massoni & Saurel (2004),
//!   *Int. J. Therm. Sci.* 43, 265–276, Table 1, fitted by the authors to the
//!   saturation curves over 298–473 K.
//! - [`StiffenedGasPair`]: a liquid and a vapour stiffened gas, whose
//!   saturation temperature follows from equal Gibbs energies (Eq 215, solved
//!   by Newton) and whose latent heat is `h_g − h_l` (Eq 220).
//! - [`IsentropicStiffenedGasHem`]: `p = (p₀ + p∞)(ρ/ρ₀)^γ − p∞` (Eq 59).
//! - [`BarotropicHem`]: `p = p₀ + a²(ρ − ρ₀)` (Eq 54).
//! - [`LinearEosHem`]: `p = p₀ + K_ρ(ρ − ρ₀) + K_T(T − T₀)`, `e = e₀ +
//!   c_v(T − T₀)` (Eqs 66–67), with coefficients generated from IAPWS-IF97 at a
//!   chosen reference state ([`LinearEosHem::from_iapws97`]), not copied from
//!   the manual's tables.
//!
//! The isentropic and barotropic forms close pressure from density alone:
//! they describe reversible processes only, so they carry no temperature. Their
//! [`HemThermoState::temperature`] is the reference temperature they were
//! built with, and their internal energy is whatever the solver transports.
//!
//! Every form is single phase: `quality` is `None`, and the void fraction is
//! `0` for a liquid-like closure (`p∞ > 0`, or the barotropic and linear
//! forms) and `1` for a gas-like one.

use petir::mathf::RealMath;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{
    AvailableEnergy, MassDensity, Pressure, Ratio, SpecificHeatCapacity, ThermodynamicTemperature,
    Velocity,
};
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::velocity::meter_per_second;

use super::eos::{require_positive, HemEquationOfState, HemThermoState};
use super::HemError;
use crate::region_1_subcooled_liquid::{cv_tp_1, u_tp_1, v_tp_1};
use crate::region_4_vap_liq_equilibrium::sat_temp_4;

fn pressure(p: f64) -> Pressure {
    Pressure::new::<pascal>(p)
}

fn energy(e: f64) -> AvailableEnergy {
    AvailableEnergy::new::<joule_per_kilogram>(e)
}

/// Assemble a single-phase state from plain SI numbers.
fn single_phase(p: f64, t: f64, rho: f64, e: f64, c: f64, void: f64) -> HemThermoState {
    HemThermoState {
        pressure: pressure(p),
        temperature: ThermodynamicTemperature::new::<kelvin>(t),
        density: MassDensity::new::<kilogram_per_cubic_meter>(rho),
        specific_internal_energy: energy(e),
        specific_enthalpy: energy(e + p / rho),
        equilibrium_sound_speed: Velocity::new::<meter_per_second>(c),
        quality: None,
        void_fraction: Ratio::new::<ratio>(void),
    }
}

/// `Err(EquationOfState)` naming the closure, for a state it cannot close.
fn eos_error(name: &str, detail: String) -> HemError {
    HemError::EquationOfState {
        cell: None,
        detail: format!("{name}: {detail}"),
    }
}

// ───────────────────────── stiffened gas ─────────────────────────

/// Stiffened gas, `p(ρ, e) = (γ − 1)ρ(e − q) − γp∞` (RELAP-7 Eq 78), with the
/// caloric law `ρ(p, T) = (p + p∞)/((γ − 1)c_v T)` (Eq 80), `h = γ c_v T + q`
/// (Eq 81), `T = (e − q − p∞/ρ)/c_v` (Eq 84) and `c² = γ(p + p∞)/ρ` (Eq 93).
///
/// `γ > 1`, `c_v > 0`, `p∞ ≥ 0`. `q` sets the zero of the internal energy and
/// `q'` enters only the Gibbs energy ([`Self::gibbs`]), which fixes the
/// saturation curve of a [`StiffenedGasPair`].
///
/// A state is accepted only with `p > 0` and `T > 0`: the solver works in
/// positive pressure, so a liquid in tension (`−p∞ < p ≤ 0`) is refused rather
/// than passed on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StiffenedGasHem {
    /// `γ`, dimensionless, `> 1` (not the ratio of specific heats for a liquid).
    pub gamma: f64,
    /// `p∞`, Pa, `≥ 0`.
    pub p_inf_pa: f64,
    /// Binding energy `q`, J/kg.
    pub q_j_per_kg: f64,
    /// Gibbs-energy parameter `q'`, J/(kg·K).
    pub q_prime_j_per_kg_k: f64,
    /// Constant-volume specific heat `c_v`, J/(kg·K), `> 0`.
    pub cv_j_per_kg_k: f64,
}

impl StiffenedGasHem {
    /// Build from the five parameters.
    ///
    /// # Errors
    ///
    /// [`HemError::InvalidInput`] if `γ ≤ 1`, `p∞ < 0`, `c_v ≤ 0`, or any is
    /// not finite.
    pub fn new(
        gamma: Ratio,
        p_inf: Pressure,
        q: AvailableEnergy,
        q_prime: SpecificHeatCapacity,
        cv: SpecificHeatCapacity,
    ) -> Result<Self, HemError> {
        let s = Self {
            gamma: gamma.get::<ratio>(),
            p_inf_pa: p_inf.get::<pascal>(),
            q_j_per_kg: q.get::<joule_per_kilogram>(),
            q_prime_j_per_kg_k: q_prime.get::<joule_per_kilogram_kelvin>(),
            cv_j_per_kg_k: cv.get::<joule_per_kilogram_kelvin>(),
        };
        let all_finite = [
            s.gamma,
            s.p_inf_pa,
            s.q_j_per_kg,
            s.q_prime_j_per_kg_k,
            s.cv_j_per_kg_k,
        ]
        .iter()
        .all(|v| v.is_finite());
        if !all_finite || s.gamma <= 1.0 || s.p_inf_pa < 0.0 || s.cv_j_per_kg_k <= 0.0 {
            return Err(HemError::InvalidInput(format!(
                "stiffened gas needs finite gamma > 1, p_inf >= 0, cv > 0; got {s:?}"
            )));
        }
        Ok(s)
    }

    /// Liquid water: Le Métayer, Massoni & Saurel (2004), Table 1 (γ 2.35,
    /// p∞ 1e9 Pa, q −1167 kJ/kg, q' 0, c_v 1816 J/(kg·K)). Fitted to the
    /// saturation curve over 298–473 K; outside that range it is an
    /// extrapolation.
    pub fn water_liquid_le_metayer_2004() -> Self {
        Self {
            gamma: 2.35,
            p_inf_pa: 1.0e9,
            q_j_per_kg: -1167.0e3,
            q_prime_j_per_kg_k: 0.0,
            cv_j_per_kg_k: 1816.0,
        }
    }

    /// Water vapour: Le Métayer, Massoni & Saurel (2004), Table 1 (γ 1.43,
    /// p∞ 0, q 2030 kJ/kg, q' −23 kJ/(kg·K), c_v 1040 J/(kg·K)).
    ///
    /// **`q'` is taken as RELAP-7 Table 5 prints it, −23×10³, because that is
    /// the document held.** The paper itself is not held. With this value the
    /// saturation curve of [`StiffenedGasPair`] runs 5–7.5 % (15–35 K) below
    /// IAPWS-IF97 (measured 2026-11-01, `tests::stiffened_gas_saturation_*`).
    /// Eq 215 is very sensitive to `q'_v`: a change of 400 J/(kg·K) shifts
    /// `ln p_sat` by 400/((γ_v − 1)c_v,v) = 0.89, which is the whole
    /// disagreement. The value usually reproduced from the paper is
    /// −23.4×10³; **it is not adopted**, because it cannot be checked here
    /// and choosing it because it matches IF97 would be calibration. Settle it
    /// from the paper (gh:#1057).
    pub fn water_vapour_le_metayer_2004() -> Self {
        Self {
            gamma: 1.43,
            p_inf_pa: 0.0,
            q_j_per_kg: 2030.0e3,
            q_prime_j_per_kg_k: -23.0e3,
            cv_j_per_kg_k: 1040.0,
        }
    }

    /// `c_p = γ c_v`, J/(kg·K).
    fn cp(&self) -> f64 {
        self.gamma * self.cv_j_per_kg_k
    }

    fn void(&self) -> f64 {
        if self.p_inf_pa > 0.0 {
            0.0
        } else {
            1.0
        }
    }

    /// The state at `(ρ, p)`, after checking `p > 0` and `T > 0`.
    fn from_rho_p(&self, rho: f64, p: f64) -> Result<HemThermoState, HemError> {
        let (g, pinf, cv) = (self.gamma, self.p_inf_pa, self.cv_j_per_kg_k);
        if !(p.is_finite() && p > 0.0) {
            return Err(eos_error(
                self.name(),
                format!("pressure {p} Pa is not positive"),
            ));
        }
        let e = (p + g * pinf) / ((g - 1.0) * rho) + self.q_j_per_kg;
        let t = (e - self.q_j_per_kg - pinf / rho) / cv;
        if !(t.is_finite() && t > 0.0) {
            return Err(eos_error(
                self.name(),
                format!("temperature {t} K is not positive"),
            ));
        }
        let c = (g * (p + pinf) / rho).sqrt();
        Ok(single_phase(p, t, rho, e, c, self.void()))
    }

    /// Gibbs energy `g(p, T) = (γc_v − q')T − c_v T ln(T^γ/(p + p∞)^(γ−1)) + q`
    /// (RELAP-7 Eq 82), J/kg.
    pub fn gibbs(
        &self,
        pressure: Pressure,
        temperature: ThermodynamicTemperature,
    ) -> AvailableEnergy {
        let (p, t) = (pressure.get::<pascal>(), temperature.get::<kelvin>());
        let g = self.gamma;
        let ln_arg = g * t.r_ln() - (g - 1.0) * (p + self.p_inf_pa).r_ln();
        energy(
            (self.cp() - self.q_prime_j_per_kg_k) * t - self.cv_j_per_kg_k * t * ln_arg
                + self.q_j_per_kg,
        )
    }
}

impl HemEquationOfState for StiffenedGasHem {
    fn name(&self) -> &'static str {
        "stiffened gas"
    }

    fn state_rho_e(
        &self,
        density: MassDensity,
        specific_internal_energy: AvailableEnergy,
        _pressure_hint: Option<Pressure>,
    ) -> Result<HemThermoState, HemError> {
        let rho = require_positive(
            "density",
            density.get::<kilogram_per_cubic_meter>(),
            "kg/m3",
        )?;
        let e = specific_internal_energy.get::<joule_per_kilogram>();
        if !e.is_finite() {
            return Err(eos_error(self.name(), format!("internal energy {e} J/kg")));
        }
        let p = (self.gamma - 1.0) * rho * (e - self.q_j_per_kg) - self.gamma * self.p_inf_pa;
        self.from_rho_p(rho, p)
    }

    fn state_p_t(
        &self,
        pressure: Pressure,
        temperature: ThermodynamicTemperature,
    ) -> Result<HemThermoState, HemError> {
        let p = require_positive("pressure", pressure.get::<pascal>(), "Pa")?;
        let t = require_positive("temperature", temperature.get::<kelvin>(), "K")?;
        let rho = (p + self.p_inf_pa) / ((self.gamma - 1.0) * self.cv_j_per_kg_k * t);
        self.from_rho_p(rho, p)
    }

    fn state_p_h(
        &self,
        pressure: Pressure,
        specific_enthalpy: AvailableEnergy,
    ) -> Result<HemThermoState, HemError> {
        require_positive("pressure", pressure.get::<pascal>(), "Pa")?;
        let h = specific_enthalpy.get::<joule_per_kilogram>();
        let t = (h - self.q_j_per_kg) / self.cp();
        if !(t.is_finite() && t > 0.0) {
            return Err(eos_error(
                self.name(),
                format!("h = {h} J/kg gives T = {t} K"),
            ));
        }
        self.state_p_t(pressure, ThermodynamicTemperature::new::<kelvin>(t))
    }

    fn state_p_rho(
        &self,
        pressure: Pressure,
        density: MassDensity,
    ) -> Result<HemThermoState, HemError> {
        let p = require_positive("pressure", pressure.get::<pascal>(), "Pa")?;
        let rho = require_positive(
            "density",
            density.get::<kilogram_per_cubic_meter>(),
            "kg/m3",
        )?;
        self.from_rho_p(rho, p)
    }
}

/// A liquid and a vapour stiffened gas: the saturation curve and latent heat
/// that follow from them (RELAP-7 §3.2.5, after Le Métayer et al. 2004).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StiffenedGasPair {
    /// The liquid phase.
    pub liquid: StiffenedGasHem,
    /// The vapour phase.
    pub vapour: StiffenedGasHem,
}

impl StiffenedGasPair {
    /// Water and steam with the Le Métayer et al. (2004) parameters.
    pub fn water_le_metayer_2004() -> Self {
        Self {
            liquid: StiffenedGasHem::water_liquid_le_metayer_2004(),
            vapour: StiffenedGasHem::water_vapour_le_metayer_2004(),
        }
    }

    /// The coefficients `(A, B, C, D)` of RELAP-7 Eqs 216–219.
    fn abcd(&self) -> (f64, f64, f64, f64) {
        let (l, v) = (&self.liquid, &self.vapour);
        let denom = v.cp() - v.cv_j_per_kg_k;
        (
            (l.cp() - v.cp() + v.q_prime_j_per_kg_k - l.q_prime_j_per_kg_k) / denom,
            (l.q_j_per_kg - v.q_j_per_kg) / denom,
            (v.cp() - l.cp()) / denom,
            (l.cp() - l.cv_j_per_kg_k) / denom,
        )
    }

    /// Saturation temperature at `pressure`: the root of equal Gibbs energies,
    /// `ln(p + p∞,v) = A + B/T + C ln T + D ln(p + p∞,l)` (RELAP-7 Eq 215), by
    /// Newton from 373.15 K (the manual's method).
    ///
    /// # Errors
    ///
    /// [`HemError::EquationOfState`] if `pressure` is not positive or Newton
    /// does not converge to `1e-12` relative in 100 iterations.
    pub fn saturation_temperature(
        &self,
        pressure: Pressure,
    ) -> Result<ThermodynamicTemperature, HemError> {
        let p = require_positive("pressure", pressure.get::<pascal>(), "Pa")?;
        let (a, b, c, d) = self.abcd();
        let lhs = (p + self.vapour.p_inf_pa).r_ln() - d * (p + self.liquid.p_inf_pa).r_ln() - a;
        // f(T) = B/T + C ln T − lhs, f'(T) = −B/T² + C/T.
        let mut t = 373.15_f64;
        for _ in 0..100 {
            let f = b / t + c * t.r_ln() - lhs;
            let df = -b / (t * t) + c / t;
            let step = f / df;
            let next = t - step;
            if !(next.is_finite() && next > 0.0) {
                break;
            }
            t = next;
            if step.abs() <= 1e-12 * t {
                return Ok(ThermodynamicTemperature::new::<kelvin>(t));
            }
        }
        Err(eos_error(
            "stiffened gas pair",
            format!("saturation temperature Newton did not converge at p = {p} Pa"),
        ))
    }

    /// Latent heat `L_v = (γ_v c_v,v T + q_v) − (γ_l c_v,l T + q_l)` at
    /// `temperature` (RELAP-7 Eq 220), J/kg.
    pub fn latent_heat(&self, temperature: ThermodynamicTemperature) -> AvailableEnergy {
        let t = temperature.get::<kelvin>();
        energy(
            (self.vapour.cp() * t + self.vapour.q_j_per_kg)
                - (self.liquid.cp() * t + self.liquid.q_j_per_kg),
        )
    }
}

// ───────────────────── isentropic stiffened gas ─────────────────────

/// Isentropic stiffened gas, `p = (p₀ + p∞)(ρ/ρ₀)^γ − p∞` (RELAP-7 Eq 59),
/// `c² = γ(p + p∞)/ρ` (Eq 60). The Tait form for a liquid; valid for weak
/// waves only, since it has no entropy rise across a shock.
///
/// Pressure depends on density alone; the temperature reported is
/// `reference_temperature`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IsentropicStiffenedGasHem {
    /// Reference pressure `p₀`, Pa.
    pub p0_pa: f64,
    /// Reference density `ρ₀`, kg/m³, `> 0`.
    pub rho0: f64,
    /// `p∞`, Pa, `≥ 0`.
    pub p_inf_pa: f64,
    /// `γ`, dimensionless, `> 1`.
    pub gamma: f64,
    /// Temperature reported with every state, K.
    pub reference_temperature_k: f64,
}

impl IsentropicStiffenedGasHem {
    /// Water as Courant and Friedrichs give it (via RELAP-7 Table 2, which
    /// cites them): `ρ₀ = 999.8`, `γ = 7`, `p = A(ρ/ρ₀)^γ − B` with `A =
    /// 3.04076e8`, `B = 3.03975e8` Pa, so `p∞ = B` and `p₀ = A − B`. Reference
    /// temperature 293.15 K.
    pub fn water_courant_friedrichs() -> Self {
        Self {
            p0_pa: 3.04076e8 - 3.03975e8,
            rho0: 999.8,
            p_inf_pa: 3.03975e8,
            gamma: 7.0,
            reference_temperature_k: 293.15,
        }
    }

    fn pressure_at(&self, rho: f64) -> f64 {
        (self.p0_pa + self.p_inf_pa) * (rho / self.rho0).r_powf(self.gamma) - self.p_inf_pa
    }

    fn density_at(&self, p: f64) -> f64 {
        self.rho0 * ((p + self.p_inf_pa) / (self.p0_pa + self.p_inf_pa)).r_powf(1.0 / self.gamma)
    }

    fn state(&self, rho: f64, e: f64) -> Result<HemThermoState, HemError> {
        let p = self.pressure_at(rho);
        if !(p.is_finite() && p > 0.0) {
            return Err(eos_error(
                self.name(),
                format!("pressure {p} Pa at rho = {rho} kg/m3"),
            ));
        }
        let c = (self.gamma * (p + self.p_inf_pa) / rho).sqrt();
        Ok(single_phase(
            p,
            self.reference_temperature_k,
            rho,
            e,
            c,
            0.0,
        ))
    }
}

impl HemEquationOfState for IsentropicStiffenedGasHem {
    fn name(&self) -> &'static str {
        "isentropic stiffened gas"
    }

    fn state_rho_e(
        &self,
        density: MassDensity,
        specific_internal_energy: AvailableEnergy,
        _pressure_hint: Option<Pressure>,
    ) -> Result<HemThermoState, HemError> {
        let rho = require_positive(
            "density",
            density.get::<kilogram_per_cubic_meter>(),
            "kg/m3",
        )?;
        self.state(rho, specific_internal_energy.get::<joule_per_kilogram>())
    }

    /// The temperature is not a state variable here; it is ignored.
    fn state_p_t(
        &self,
        pressure: Pressure,
        _temperature: ThermodynamicTemperature,
    ) -> Result<HemThermoState, HemError> {
        let p = require_positive("pressure", pressure.get::<pascal>(), "Pa")?;
        let rho = self.density_at(p);
        self.state(rho, p / rho)
    }

    fn state_p_h(
        &self,
        pressure: Pressure,
        specific_enthalpy: AvailableEnergy,
    ) -> Result<HemThermoState, HemError> {
        let p = require_positive("pressure", pressure.get::<pascal>(), "Pa")?;
        let rho = self.density_at(p);
        self.state(rho, specific_enthalpy.get::<joule_per_kilogram>() - p / rho)
    }

    /// `p` and `ρ` are not independent here: the state is the one at `ρ`, and
    /// a `p` that disagrees with it by more than `1e-9` relative is an error.
    fn state_p_rho(
        &self,
        pressure: Pressure,
        density: MassDensity,
    ) -> Result<HemThermoState, HemError> {
        let p = require_positive("pressure", pressure.get::<pascal>(), "Pa")?;
        let rho = require_positive(
            "density",
            density.get::<kilogram_per_cubic_meter>(),
            "kg/m3",
        )?;
        let s = self.state(rho, p / rho)?;
        let rel = (s.pressure.get::<pascal>() / p - 1.0).abs();
        if rel > 1e-9 {
            return Err(eos_error(
                self.name(),
                format!("p = {p} Pa is not p(rho = {rho}) (rel {rel:e})"),
            ));
        }
        Ok(s)
    }
}

// ───────────────────────── barotropic ─────────────────────────

/// Barotropic fluid, `p = p₀ + a²(ρ − ρ₀)` (RELAP-7 Eq 54), constant sound
/// speed `a`. For testing: discontinuities in the initial data propagate
/// without steepening.
///
/// Pressure depends on density alone; the temperature reported is
/// `reference_temperature`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarotropicHem {
    /// Reference pressure `p₀`, Pa.
    pub p0_pa: f64,
    /// Reference density `ρ₀`, kg/m³, `> 0`.
    pub rho0: f64,
    /// Sound speed `a`, m/s, `> 0`.
    pub sound_speed_m_s: f64,
    /// Temperature reported with every state, K.
    pub reference_temperature_k: f64,
}

impl BarotropicHem {
    fn state(&self, rho: f64, e: f64) -> Result<HemThermoState, HemError> {
        let a = self.sound_speed_m_s;
        let p = self.p0_pa + a * a * (rho - self.rho0);
        if !(p.is_finite() && p > 0.0) {
            return Err(eos_error(
                self.name(),
                format!("pressure {p} Pa at rho = {rho} kg/m3"),
            ));
        }
        Ok(single_phase(
            p,
            self.reference_temperature_k,
            rho,
            e,
            a,
            0.0,
        ))
    }

    fn density_at(&self, p: f64) -> f64 {
        self.rho0 + (p - self.p0_pa) / (self.sound_speed_m_s * self.sound_speed_m_s)
    }
}

impl HemEquationOfState for BarotropicHem {
    fn name(&self) -> &'static str {
        "barotropic"
    }

    fn state_rho_e(
        &self,
        density: MassDensity,
        specific_internal_energy: AvailableEnergy,
        _pressure_hint: Option<Pressure>,
    ) -> Result<HemThermoState, HemError> {
        let rho = require_positive(
            "density",
            density.get::<kilogram_per_cubic_meter>(),
            "kg/m3",
        )?;
        self.state(rho, specific_internal_energy.get::<joule_per_kilogram>())
    }

    /// The temperature is not a state variable here; it is ignored.
    fn state_p_t(
        &self,
        pressure: Pressure,
        _temperature: ThermodynamicTemperature,
    ) -> Result<HemThermoState, HemError> {
        let p = require_positive("pressure", pressure.get::<pascal>(), "Pa")?;
        let rho = require_positive("density", self.density_at(p), "kg/m3")?;
        self.state(rho, p / rho)
    }

    fn state_p_h(
        &self,
        pressure: Pressure,
        specific_enthalpy: AvailableEnergy,
    ) -> Result<HemThermoState, HemError> {
        let p = require_positive("pressure", pressure.get::<pascal>(), "Pa")?;
        let rho = require_positive("density", self.density_at(p), "kg/m3")?;
        self.state(rho, specific_enthalpy.get::<joule_per_kilogram>() - p / rho)
    }

    /// `p` and `ρ` are not independent here: a `p` that disagrees with
    /// `p(ρ)` by more than `1e-9` relative is an error.
    fn state_p_rho(
        &self,
        pressure: Pressure,
        density: MassDensity,
    ) -> Result<HemThermoState, HemError> {
        let p = require_positive("pressure", pressure.get::<pascal>(), "Pa")?;
        let rho = require_positive(
            "density",
            density.get::<kilogram_per_cubic_meter>(),
            "kg/m3",
        )?;
        let s = self.state(rho, p / rho)?;
        let rel = (s.pressure.get::<pascal>() / p - 1.0).abs();
        if rel > 1e-9 {
            return Err(eos_error(
                self.name(),
                format!("p = {p} Pa is not p(rho = {rho}) (rel {rel:e})"),
            ));
        }
        Ok(s)
    }
}

// ───────────────────────── linear ─────────────────────────

/// Linear liquid, `p = p₀ + K_ρ(ρ − ρ₀) + K_T(T − T₀)`, `e = e₀ + c_v(T −
/// T₀)` (RELAP-7 Eqs 66–67). Its sound speed is the isentropic one of that
/// `p(ρ, e)`: `c² = K_ρ + p K_T/(c_v ρ²)`.
///
/// Build it with [`Self::from_iapws97`], which takes the coefficients from
/// IAPWS-IF97 at a reference state; the manual's own tables are not used.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearEosHem {
    /// Reference pressure `p₀`, Pa.
    pub p0_pa: f64,
    /// Reference density `ρ₀`, kg/m³.
    pub rho0: f64,
    /// Reference temperature `T₀`, K.
    pub t0_k: f64,
    /// `K_ρ = (∂p/∂ρ)_T`, Pa·m³/kg.
    pub k_rho: f64,
    /// `K_T = (∂p/∂T)_ρ`, Pa/K.
    pub k_t: f64,
    /// `c_v`, J/(kg·K).
    pub cv_j_per_kg_k: f64,
    /// Reference internal energy `e₀`, J/kg.
    pub e0_j_per_kg: f64,
}

impl LinearEosHem {
    /// Coefficients from IAPWS-IF97 region 1 (subcooled liquid) at `(p₀, T₀)`:
    /// `ρ₀ = 1/v`, `e₀ = u`, `c_v` from IF97, and `K_ρ = 1/(∂ρ/∂p)_T`, `K_T =
    /// −(∂ρ/∂T)_p K_ρ` by central differences (steps `1e-6 p₀` and `1e-4` K).
    ///
    /// # Errors
    ///
    /// [`HemError::InvalidInput`] unless `(p₀, T₀)` is subcooled liquid (`T₀`
    /// below the IF97 saturation temperature at `p₀`).
    pub fn from_iapws97(p0: Pressure, t0: ThermodynamicTemperature) -> Result<Self, HemError> {
        let (p, t) = (p0.get::<pascal>(), t0.get::<kelvin>());
        if !(p.is_finite() && p > 0.0 && t.is_finite() && t > 273.15)
            || t >= sat_temp_4(p0).get::<kelvin>()
        {
            return Err(HemError::InvalidInput(format!(
                "linear EOS reference ({p} Pa, {t} K) must be IF97 subcooled liquid"
            )));
        }
        let rho = |p: f64, t: f64| {
            1.0 / v_tp_1(ThermodynamicTemperature::new::<kelvin>(t), pressure(p))
                .get::<cubic_meter_per_kilogram>()
        };
        let (dp, dt) = (1e-6 * p, 1e-4);
        let drho_dp = (rho(p + dp, t) - rho(p - dp, t)) / (2.0 * dp);
        let drho_dt = (rho(p, t + dt) - rho(p, t - dt)) / (2.0 * dt);
        let k_rho = 1.0 / drho_dp;
        Ok(Self {
            p0_pa: p,
            rho0: rho(p, t),
            t0_k: t,
            k_rho,
            k_t: -drho_dt * k_rho,
            cv_j_per_kg_k: cv_tp_1(t0, p0).get::<joule_per_kilogram_kelvin>(),
            e0_j_per_kg: u_tp_1(t0, p0).get::<joule_per_kilogram>(),
        })
    }

    fn from_rho_t(&self, rho: f64, t: f64) -> Result<HemThermoState, HemError> {
        let p = self.p0_pa + self.k_rho * (rho - self.rho0) + self.k_t * (t - self.t0_k);
        if !(p.is_finite() && p > 0.0 && t > 0.0) {
            return Err(eos_error(
                self.name(),
                format!("p = {p} Pa, T = {t} K at rho = {rho} kg/m3"),
            ));
        }
        let e = self.e0_j_per_kg + self.cv_j_per_kg_k * (t - self.t0_k);
        let c2 = self.k_rho + p * self.k_t / (self.cv_j_per_kg_k * rho * rho);
        if c2 <= 0.0 {
            return Err(eos_error(
                self.name(),
                format!("c^2 = {c2} at rho = {rho} kg/m3"),
            ));
        }
        Ok(single_phase(p, t, rho, e, c2.sqrt(), 0.0))
    }

    /// `ρ(p, T)` (RELAP-7 Eq 76).
    fn density_at(&self, p: f64, t: f64) -> f64 {
        self.rho0 + (p - self.p0_pa) / self.k_rho - self.k_t / self.k_rho * (t - self.t0_k)
    }
}

impl HemEquationOfState for LinearEosHem {
    fn name(&self) -> &'static str {
        "linear"
    }

    fn state_rho_e(
        &self,
        density: MassDensity,
        specific_internal_energy: AvailableEnergy,
        _pressure_hint: Option<Pressure>,
    ) -> Result<HemThermoState, HemError> {
        let rho = require_positive(
            "density",
            density.get::<kilogram_per_cubic_meter>(),
            "kg/m3",
        )?;
        let e = specific_internal_energy.get::<joule_per_kilogram>();
        let t = self.t0_k + (e - self.e0_j_per_kg) / self.cv_j_per_kg_k;
        self.from_rho_t(rho, t)
    }

    fn state_p_t(
        &self,
        pressure: Pressure,
        temperature: ThermodynamicTemperature,
    ) -> Result<HemThermoState, HemError> {
        let p = require_positive("pressure", pressure.get::<pascal>(), "Pa")?;
        let t = require_positive("temperature", temperature.get::<kelvin>(), "K")?;
        let rho = require_positive("density", self.density_at(p, t), "kg/m3")?;
        self.from_rho_t(rho, t)
    }

    /// `h = e + p/ρ(p, T)` is solved for `T` by Newton (it is smooth and
    /// nearly linear in `T` for a liquid).
    fn state_p_h(
        &self,
        pressure: Pressure,
        specific_enthalpy: AvailableEnergy,
    ) -> Result<HemThermoState, HemError> {
        let p = require_positive("pressure", pressure.get::<pascal>(), "Pa")?;
        let h = specific_enthalpy.get::<joule_per_kilogram>();
        let resid = |t: f64| {
            self.e0_j_per_kg + self.cv_j_per_kg_k * (t - self.t0_k) + p / self.density_at(p, t) - h
        };
        let mut t = self.t0_k + (h - self.e0_j_per_kg - p / self.rho0) / self.cv_j_per_kg_k;
        for _ in 0..50 {
            let f = resid(t);
            let df = (resid(t + 1e-3) - resid(t - 1e-3)) / 2e-3;
            let step = f / df;
            t -= step;
            if step.abs() <= 1e-12 * t.abs() {
                return self.state_p_t(pressure, ThermodynamicTemperature::new::<kelvin>(t));
            }
        }
        Err(eos_error(
            self.name(),
            format!("(p, h) Newton did not converge at h = {h} J/kg"),
        ))
    }

    fn state_p_rho(
        &self,
        pressure: Pressure,
        density: MassDensity,
    ) -> Result<HemThermoState, HemError> {
        let p = require_positive("pressure", pressure.get::<pascal>(), "Pa")?;
        let rho = require_positive(
            "density",
            density.get::<kilogram_per_cubic_meter>(),
            "kg/m3",
        )?;
        let t = self.t0_k + (p - self.p0_pa - self.k_rho * (rho - self.rho0)) / self.k_t;
        self.from_rho_t(rho, t)
    }
}

#[cfg(test)]
mod tests;
