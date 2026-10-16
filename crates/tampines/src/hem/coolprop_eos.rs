//! The CoolProp HEM closure, [`CoolPropHem`], and the run-time choice of
//! closure, [`HemEos`].
//!
//! These live in `tampines` because only `tampines` depends on both property
//! crates (maintainer decision D2, gh #896): the solver and the IF97 and
//! ideal-gas closures are in `tampines-steam-tables`.

use outram_park_fork_coolprop::flash::{hem_sound_speed_ph, state_ph, state_pt};
use outram_park_fork_coolprop::vle::{phase_at_ph, saturation_at_temperature, PhaseAtPh};
use outram_park_fork_coolprop::Fluid as CoolPropFluid;
use outram_park_fork_coolprop::FluidState;
use tampines_steam_tables::hem::{
    pressure_from_rho_e, BreakFlow, HemEquationOfState, HemError, HemThermoState, Iapws97Hem,
    IdealGasHem,
};
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{
    AvailableEnergy, MassDensity, Pressure, Ratio, SpecificVolume, ThermodynamicTemperature,
    Velocity,
};
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::velocity::meter_per_second;

/// Relative pressure tolerance of the CoolProp `(ρ, e)` inversion.
pub const COOLPROP_P_RHO_E_REL_TOL: f64 = 1.0e-12;

/// Lowest pressure the CoolProp `(ρ, e)` inversion searches, Pa. Below the
/// triple-point pressure of every fluid this workspace runs (water: 611.657 Pa),
/// so it never cuts off a real root; it only bounds the bracket search.
const P_MIN_PA: f64 = 1.0;

/// A CoolProp fluid (Helmholtz EOS plus Maxwell VLE) as a HEM closure.
///
/// - **Single phase:** `outram_park_fork_coolprop::flash::state_ph`, whose
///   EOS sound speed is the equilibrium speed there (no phase change to lag).
///   A result inside the saturation dome at its own temperature (a metastable
///   or unstable root) is an error; see `close_ph`. **Known limitation,
///   measured 2026-10-16:** that guard fires for compressed-liquid water,
///   because the coolprop crate's `density_pt` returns a vapour-side or
///   unstable root there, so this closure cannot yet run liquid water.
/// - **Two-phase dome:** `vle::phase_at_ph` (Maxwell VLE at `T_sat(p)`, lever
///   rule on enthalpy) for the mixture density, and
///   `flash::hem_sound_speed_ph` for the homogeneous-**equilibrium** speed.
///   `state_ph` itself is single-phase only; the dome is reached through
///   `phase_at_ph`.
/// - **`(ρ, e)`:** [`pressure_from_rho_e`], the same bracketed Brent root over
///   this `(p, h)` flash that the IF97 closure uses (decision D4).
///
/// Subcritical two-phase only where the fluid ships saturation ancillaries;
/// above the critical pressure every state is single phase. No break model:
/// a [`tampines_steam_tables::hem::HemBoundary::ChokedBreak`] with this
/// closure is [`HemError::Unsupported`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoolPropHem {
    /// The CoolProp fluid.
    pub fluid: CoolPropFluid,
}

/// Where a `(p, h)` state sits, closed by CoolProp.
#[derive(Debug, Clone, Copy)]
enum PhClosure {
    /// Inside the dome: saturation temperature, quality, mixture density.
    TwoPhase { t: f64, x: f64, rho: f64 },
    /// Single phase, the stable root.
    SinglePhase(FluidState),
}

impl CoolPropHem {
    /// Close `(p, h)`: Maxwell VLE inside the dome, else the single-phase
    /// flash, **rejecting** a single-phase result whose density lies inside
    /// the saturation dome at its own temperature.
    ///
    /// That guard exists because `outram_park_fork_coolprop::flash::density_pt`
    /// starts its Newton from the ideal-gas density and so lands on the vapour
    /// side: measured 2026-10-16 for water, `state_pt(350 K, 2 MPa)` returns
    /// `ρ = 322.0 kg/m³` (the critical density) with `h ≈ −6e18 J/kg`, and
    /// `state_pt(455 K, 1.3 MPa)` a metastable vapour (`ρ = 6.84 kg/m³`) where
    /// the stable state is compressed liquid; `state_ph` inherits both. Such a
    /// root is not the equilibrium state, so it is an error here, never a
    /// value. Consequence: compressed-liquid water cannot be closed by this
    /// closure until `density_pt` takes a liquid-side initial guess (upstream
    /// CoolProp's `solver_rho_Tp` uses the saturated-liquid ancillary density
    /// for a liquid).
    fn close_ph(&self, p: f64, h: f64) -> Result<PhClosure, HemError> {
        if !(p.is_finite() && p > 0.0 && h.is_finite()) {
            return Err(self.eos_err(format!("(p, h) = ({p} Pa, {h} J/kg) is not a valid input")));
        }
        match phase_at_ph(self.fluid, p, h) {
            PhaseAtPh::TwoPhase {
                temperature,
                quality,
                density,
            } => Ok(PhClosure::TwoPhase {
                t: temperature,
                x: quality,
                rho: density,
            }),
            PhaseAtPh::SinglePhase => {
                let s = state_ph(self.fluid, p, h)
                    .map_err(|e| self.eos_err(format!("(p, h) = ({p} Pa, {h} J/kg): {e:?}")))?;
                if s.temperature < self.fluid.eos().t_critical {
                    if let Some(sat) = saturation_at_temperature(self.fluid, s.temperature) {
                        if s.density > sat.rho_vapour && s.density < sat.rho_liquid {
                            return Err(self.eos_err(format!(
                                "single-phase (p, h) flash at ({p} Pa, {h} J/kg) returned rho = {} kg/m3 \
                                 at T = {} K, inside the saturation dome [{}, {}] kg/m3: a metastable or \
                                 unstable root, not the equilibrium state",
                                s.density, s.temperature, sat.rho_vapour, sat.rho_liquid
                            )));
                        }
                    }
                }
                if !(s.density.is_finite() && s.density > 0.0) {
                    return Err(self.eos_err(format!("(p, h) flash gave rho = {}", s.density)));
                }
                Ok(PhClosure::SinglePhase(s))
            }
        }
    }

    /// Mixture specific volume at `(p, h)`, or `None` where the closure fails.
    fn specific_volume(&self, p: Pressure, h: AvailableEnergy) -> Option<SpecificVolume> {
        let rho = match self
            .close_ph(p.get::<pascal>(), h.get::<joule_per_kilogram>())
            .ok()?
        {
            PhClosure::TwoPhase { rho, .. } => rho,
            PhClosure::SinglePhase(s) => s.density,
        };
        Some(SpecificVolume::new::<cubic_meter_per_kilogram>(1.0 / rho))
    }

    fn eos_err(&self, detail: String) -> HemError {
        HemError::EquationOfState {
            cell: None,
            detail: format!("CoolProp {}: {detail}", self.fluid.eos().name),
        }
    }
}

impl HemEquationOfState for CoolPropHem {
    fn name(&self) -> &'static str {
        "CoolProp"
    }

    fn state_rho_e(
        &self,
        density: MassDensity,
        specific_internal_energy: AvailableEnergy,
        pressure_hint: Option<Pressure>,
    ) -> Result<HemThermoState, HemError> {
        let eos = self.fluid.eos();
        let p = pressure_from_rho_e(
            density,
            specific_internal_energy,
            pressure_hint,
            (
                Pressure::new::<pascal>(P_MIN_PA),
                Pressure::new::<pascal>(eos.p_max),
            ),
            COOLPROP_P_RHO_E_REL_TOL,
            |p, h| self.specific_volume(p, h),
        )?;
        let rho = density.get::<kilogram_per_cubic_meter>();
        let h = specific_internal_energy.get::<joule_per_kilogram>() + p.get::<pascal>() / rho;
        let mut st = self.state_p_h(p, AvailableEnergy::new::<joule_per_kilogram>(h))?;
        st.density = density;
        st.specific_internal_energy = specific_internal_energy;
        Ok(st)
    }

    fn state_p_t(
        &self,
        pressure: Pressure,
        temperature: ThermodynamicTemperature,
    ) -> Result<HemThermoState, HemError> {
        let (p, t) = (pressure.get::<pascal>(), temperature.get::<kelvin>());
        let s = state_pt(self.fluid, t, p)
            .map_err(|e| self.eos_err(format!("(p, T) = ({p} Pa, {t} K): {e:?}")))?;
        // Through (p, h), so ρ and e agree with the (ρ, e) inversion.
        self.state_p_h(
            pressure,
            AvailableEnergy::new::<joule_per_kilogram>(s.enthalpy),
        )
    }

    fn state_p_h(
        &self,
        pressure: Pressure,
        specific_enthalpy: AvailableEnergy,
    ) -> Result<HemThermoState, HemError> {
        let (p, h) = (
            pressure.get::<pascal>(),
            specific_enthalpy.get::<joule_per_kilogram>(),
        );
        let (t, rho, quality, c, void) = match self.close_ph(p, h)? {
            PhClosure::TwoPhase { t, x, rho } => {
                // c_min = 0: a failed equilibrium finite difference returns 0,
                // which is reported below instead of being floored.
                let c = hem_sound_speed_ph(self.fluid, p, h, 0.0);
                let rho_g = saturation_at_temperature(self.fluid, t)
                    .map(|s| s.rho_vapour)
                    .ok_or_else(|| self.eos_err(format!("no VLE at T = {t} K")))?;
                (t, rho, Some(x), c, (x * rho / rho_g).clamp(0.0, 1.0))
            }
            PhClosure::SinglePhase(s) => {
                let rho_crit = self.fluid.eos().rho_critical * self.fluid.eos().molar_mass;
                let void = if s.density >= rho_crit { 0.0 } else { 1.0 };
                (s.temperature, s.density, None, s.speed_of_sound, void)
            }
        };
        if !(c.is_finite() && c > 0.0) {
            return Err(self.eos_err(format!(
                "equilibrium sound speed {c} m/s at (p, h) = ({p} Pa, {h} J/kg)"
            )));
        }
        Ok(HemThermoState {
            pressure,
            temperature: ThermodynamicTemperature::new::<kelvin>(t),
            density: MassDensity::new::<kilogram_per_cubic_meter>(rho),
            specific_internal_energy: AvailableEnergy::new::<joule_per_kilogram>(h - p / rho),
            specific_enthalpy,
            equilibrium_sound_speed: Velocity::new::<meter_per_second>(c),
            quality: quality.map(Ratio::new::<ratio>),
            void_fraction: Ratio::new::<ratio>(void),
        })
    }

    /// Not provided: give `(p, T)` or `(p, h)`.
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
}

/// Which closure a [`super::HemPipe`] runs with, chosen at run time (enum
/// dispatch, per the workspace rule; no `dyn`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HemEos {
    /// IAPWS-IF97 water/steam (`tampines-steam-tables`). The only closure with
    /// a choked-break model. Valid 273.15–1073.15 K, up to 100 MPa.
    Iapws97,
    /// A CoolProp fluid: Helmholtz EOS plus Maxwell VLE.
    CoolProp(CoolPropFluid),
    /// Calorically perfect gas, for verification (the Sod limit).
    IdealGas(IdealGasHem),
}

impl HemEquationOfState for HemEos {
    fn name(&self) -> &'static str {
        match self {
            HemEos::Iapws97 => Iapws97Hem.name(),
            HemEos::CoolProp(f) => CoolPropHem { fluid: *f }.name(),
            HemEos::IdealGas(g) => g.name(),
        }
    }

    fn state_rho_e(
        &self,
        density: MassDensity,
        specific_internal_energy: AvailableEnergy,
        pressure_hint: Option<Pressure>,
    ) -> Result<HemThermoState, HemError> {
        match self {
            HemEos::Iapws97 => {
                Iapws97Hem.state_rho_e(density, specific_internal_energy, pressure_hint)
            }
            HemEos::CoolProp(f) => CoolPropHem { fluid: *f }.state_rho_e(
                density,
                specific_internal_energy,
                pressure_hint,
            ),
            HemEos::IdealGas(g) => g.state_rho_e(density, specific_internal_energy, pressure_hint),
        }
    }

    fn state_p_t(
        &self,
        pressure: Pressure,
        temperature: ThermodynamicTemperature,
    ) -> Result<HemThermoState, HemError> {
        match self {
            HemEos::Iapws97 => Iapws97Hem.state_p_t(pressure, temperature),
            HemEos::CoolProp(f) => CoolPropHem { fluid: *f }.state_p_t(pressure, temperature),
            HemEos::IdealGas(g) => g.state_p_t(pressure, temperature),
        }
    }

    fn state_p_h(
        &self,
        pressure: Pressure,
        specific_enthalpy: AvailableEnergy,
    ) -> Result<HemThermoState, HemError> {
        match self {
            HemEos::Iapws97 => Iapws97Hem.state_p_h(pressure, specific_enthalpy),
            HemEos::CoolProp(f) => CoolPropHem { fluid: *f }.state_p_h(pressure, specific_enthalpy),
            HemEos::IdealGas(g) => g.state_p_h(pressure, specific_enthalpy),
        }
    }

    fn state_p_rho(
        &self,
        pressure: Pressure,
        density: MassDensity,
    ) -> Result<HemThermoState, HemError> {
        match self {
            HemEos::Iapws97 => Iapws97Hem.state_p_rho(pressure, density),
            HemEos::CoolProp(f) => CoolPropHem { fluid: *f }.state_p_rho(pressure, density),
            HemEos::IdealGas(g) => g.state_p_rho(pressure, density),
        }
    }

    fn break_flow(
        &self,
        stagnation_pressure: Pressure,
        stagnation_enthalpy: AvailableEnergy,
        back_pressure: Pressure,
    ) -> Result<BreakFlow, HemError> {
        match self {
            HemEos::Iapws97 => {
                Iapws97Hem.break_flow(stagnation_pressure, stagnation_enthalpy, back_pressure)
            }
            HemEos::CoolProp(f) => CoolPropHem { fluid: *f }.break_flow(
                stagnation_pressure,
                stagnation_enthalpy,
                back_pressure,
            ),
            HemEos::IdealGas(g) => {
                g.break_flow(stagnation_pressure, stagnation_enthalpy, back_pressure)
            }
        }
    }
}
