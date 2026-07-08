//! [`OPCPFluidSingleCV`] — a single, uniform, `uom`-typed control volume backed
//! by the CoolProp Helmholtz EOS.
//!
//! This is the CoolProp-fork analogue of `tampines-steam-tables`'
//! `TampinesSteamTableCV`: a 0-D lump of one [`Fluid`] at thermodynamic
//! equilibrium, carrying the intensive state (p, T, ρ, v, u, h, s, c_v, c_p,
//! speed of sound) plus one extensive property — the fixed control-volume
//! [`Volume`]. Unlike the steam-table CV it is **multi-fluid** (it stores which
//! [`Fluid`] it holds) and, for now, **single-phase** (no two-phase quality —
//! CoolProp has no saturation/VLE solver yet, bead op-kbc).
//!
//! Construction is either the EOS-native `(T, ρ)` or one of the single-phase
//! flashes `(p, T)`, `(p, h)`, `(p, s)` (see [`crate::flash`]); the flash
//! constructors are fallible and return [`FlashError`].
//!
//! # Units
//!
//! The public API is `uom`-typed (matching `TampinesSteamTableCV`): specific
//! enthalpy and internal energy are [`AvailableEnergy`] (J/kg); specific entropy
//! and the heat capacities are [`SpecificHeatCapacity`] (J/(kg·K)). Internally
//! the state is raw `f64` SI ([`FluidState`]); conversion happens only at this
//! boundary.
//!
//! # Example
//!
//! ```
//! use outram_park_fork_coolprop::{Fluid, OPCPFluidSingleCV};
//! use uom::si::f64::*;
//! use uom::si::{pressure::pascal, thermodynamic_temperature::kelvin, volume::cubic_meter};
//!
//! // 1 m³ of steam at 5 bar, 600 K (single-phase, superheated).
//! let p = Pressure::new::<pascal>(5.0e5);
//! let t = ThermodynamicTemperature::new::<kelvin>(600.0);
//! let v = Volume::new::<cubic_meter>(1.0);
//! let cv = OPCPFluidSingleCV::try_new_from_pt(Fluid::Water, p, t, v).unwrap();
//! assert!(cv.get_specific_enthalpy().value > 0.0);
//! assert!(cv.get_speed_of_sound().value > 0.0);
//! ```

use uom::si::f64::*;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::velocity::meter_per_second;

use crate::flash::{state_ph, state_ps, state_pt, FlashError};
use crate::fluid::Fluid;
use crate::props::{state_trho, FluidState};
use crate::vle::{phase_at_ph, saturation_at_temperature, saturation_temperature, PhaseAtPh};
use crate::{conductivity, viscosity};

/// Build the mixture [`FluidState`] + quality for a `(p, h)` input, or the
/// single-phase state via the flash. `p` \[Pa\], `h` \[J/kg\].
fn state_quality_ph(fluid: Fluid, p: f64, h: f64) -> Result<(FluidState, Option<f64>), FlashError> {
    if let PhaseAtPh::TwoPhase { temperature, quality, density } = phase_at_ph(fluid, p, h) {
        let sat = saturation_at_temperature(fluid, temperature).ok_or(FlashError::NonConvergent)?;
        let sl = state_trho(fluid, temperature, sat.rho_liquid);
        let sv = state_trho(fluid, temperature, sat.rho_vapour);
        let x = quality;
        let mix = FluidState {
            temperature,
            density,
            pressure: p,
            internal_energy: (1.0 - x) * sl.internal_energy + x * sv.internal_energy,
            enthalpy: h,
            entropy: (1.0 - x) * sl.entropy + x * sv.entropy,
            cv: f64::NAN,
            cp: f64::NAN,
            speed_of_sound: f64::NAN,
        };
        Ok((mix, Some(x)))
    } else {
        Ok((state_ph(fluid, p, h)?, None))
    }
}

/// Build the mixture [`FluidState`] + quality for a `(p, s)` input, or the
/// single-phase state via the flash. `p` \[Pa\], `s` \[J/(kg·K)\].
fn state_quality_ps(fluid: Fluid, p: f64, s: f64) -> Result<(FluidState, Option<f64>), FlashError> {
    if let Some(t_sat) = saturation_temperature(fluid, p) {
        if let Some(sat) = saturation_at_temperature(fluid, t_sat) {
            let sl = state_trho(fluid, t_sat, sat.rho_liquid);
            let sv = state_trho(fluid, t_sat, sat.rho_vapour);
            if s >= sl.entropy && s <= sv.entropy && sv.entropy > sl.entropy {
                let x = (s - sl.entropy) / (sv.entropy - sl.entropy);
                let v = (1.0 - x) / sat.rho_liquid + x / sat.rho_vapour;
                let mix = FluidState {
                    temperature: t_sat,
                    density: 1.0 / v,
                    pressure: p,
                    internal_energy: (1.0 - x) * sl.internal_energy + x * sv.internal_energy,
                    enthalpy: (1.0 - x) * sl.enthalpy + x * sv.enthalpy,
                    entropy: s,
                    cv: f64::NAN,
                    cp: f64::NAN,
                    speed_of_sound: f64::NAN,
                };
                return Ok((mix, Some(x)));
            }
        }
    }
    Ok((state_ps(fluid, p, s)?, None))
}

/// A single, uniform control volume of one [`Fluid`] at equilibrium.
///
/// Holds the fluid identity, its [`FluidState`] (raw SI), the fixed
/// control-volume [`Volume`], and the vapour `quality` (`None` when
/// single-phase). Build it with a constructor, then read properties with the
/// `get_*` accessors (all `uom`-typed).
///
/// **Two-phase:** the `(p, h)` / `(p, s)` constructors detect a two-phase state
/// (inside the saturation dome) via [`crate::vle`] and store the mixture: `T` is
/// the saturation temperature, `ρ`/`h`/`s`/`u` are the quality-weighted mixture
/// values, and [`get_quality`](Self::get_quality) is `Some(x)`. In two-phase the
/// single-phase-only quantities `c_v`, `c_p`, `γ` and the speed of sound are
/// `NaN` (they are not defined for a two-phase mixture).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OPCPFluidSingleCV {
    fluid: Fluid,
    state: FluidState,
    volume: Volume,
    quality: Option<f64>,
}

impl OPCPFluidSingleCV {
    /// Construct from the EOS-native inputs: temperature and mass density.
    ///
    /// This is a direct, non-iterative EOS evaluation (no flash), so it always
    /// succeeds. `temperature` in K, `density` in kg/m³, `volume` in m³.
    pub fn new_from_trho(
        fluid: Fluid,
        temperature: ThermodynamicTemperature,
        density: MassDensity,
        volume: Volume,
    ) -> Self {
        let state = state_trho(
            fluid,
            temperature.get::<kelvin>(),
            density.get::<kilogram_per_cubic_meter>(),
        );
        Self { fluid, state, volume, quality: None }
    }

    /// Construct from pressure and temperature (single-phase `(p, T)` flash —
    /// solves for density). Returns [`FlashError`] if the density solve does not
    /// converge (e.g. a two-phase target, not modelled).
    pub fn try_new_from_pt(
        fluid: Fluid,
        pressure: Pressure,
        temperature: ThermodynamicTemperature,
        volume: Volume,
    ) -> Result<Self, FlashError> {
        let state = state_pt(fluid, temperature.get::<kelvin>(), pressure.get::<pascal>())?;
        Ok(Self { fluid, state, volume, quality: None })
    }

    /// Construct from pressure and specific enthalpy. Detects a **two-phase**
    /// state (inside the dome) via [`crate::vle`] and stores the mixture with
    /// its vapour quality; otherwise a single-phase `(p, h)` flash. Returns
    /// [`FlashError`] on non-convergence.
    pub fn try_new_from_ph(
        fluid: Fluid,
        pressure: Pressure,
        specific_enthalpy: AvailableEnergy,
        volume: Volume,
    ) -> Result<Self, FlashError> {
        let (state, quality) = state_quality_ph(
            fluid,
            pressure.get::<pascal>(),
            specific_enthalpy.get::<joule_per_kilogram>(),
        )?;
        Ok(Self { fluid, state, volume, quality })
    }

    /// Construct from pressure and specific entropy. Two-phase-aware (as
    /// [`try_new_from_ph`](Self::try_new_from_ph)); otherwise a single-phase
    /// `(p, s)` flash. Returns [`FlashError`] on non-convergence.
    pub fn try_new_from_ps(
        fluid: Fluid,
        pressure: Pressure,
        specific_entropy: SpecificHeatCapacity,
        volume: Volume,
    ) -> Result<Self, FlashError> {
        let (state, quality) = state_quality_ps(
            fluid,
            pressure.get::<pascal>(),
            specific_entropy.get::<joule_per_kilogram_kelvin>(),
        )?;
        Ok(Self { fluid, state, volume, quality })
    }

    // ── Re-equilibration setters (keep the same volume) ─────────────────────

    /// Re-equilibrate to a new `(T, ρ)` state (EOS-native, infallible,
    /// single-phase).
    pub fn set_trho(&mut self, temperature: ThermodynamicTemperature, density: MassDensity) {
        self.state = state_trho(
            self.fluid,
            temperature.get::<kelvin>(),
            density.get::<kilogram_per_cubic_meter>(),
        );
        self.quality = None;
    }

    /// Re-equilibrate to a new `(p, T)` state (single-phase flash).
    pub fn set_pt(
        &mut self,
        pressure: Pressure,
        temperature: ThermodynamicTemperature,
    ) -> Result<(), FlashError> {
        self.state = state_pt(self.fluid, temperature.get::<kelvin>(), pressure.get::<pascal>())?;
        self.quality = None;
        Ok(())
    }

    /// Re-equilibrate to a new `(p, h)` state (two-phase-aware).
    pub fn set_ph(&mut self, pressure: Pressure, specific_enthalpy: AvailableEnergy) -> Result<(), FlashError> {
        let (state, quality) = state_quality_ph(
            self.fluid,
            pressure.get::<pascal>(),
            specific_enthalpy.get::<joule_per_kilogram>(),
        )?;
        self.state = state;
        self.quality = quality;
        Ok(())
    }

    /// Re-equilibrate to a new `(p, s)` state (two-phase-aware).
    pub fn set_ps(&mut self, pressure: Pressure, specific_entropy: SpecificHeatCapacity) -> Result<(), FlashError> {
        let (state, quality) = state_quality_ps(
            self.fluid,
            pressure.get::<pascal>(),
            specific_entropy.get::<joule_per_kilogram_kelvin>(),
        )?;
        self.state = state;
        self.quality = quality;
        Ok(())
    }

    // ── Getters ─────────────────────────────────────────────────────────────

    /// The fluid this control volume holds.
    pub fn get_fluid(&self) -> Fluid {
        self.fluid
    }

    /// Pressure \[Pa\].
    pub fn get_pressure(&self) -> Pressure {
        Pressure::new::<pascal>(self.state.pressure)
    }

    /// Temperature \[K\].
    pub fn get_temperature(&self) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(self.state.temperature)
    }

    /// Mass density \[kg/m³\].
    pub fn get_density(&self) -> MassDensity {
        MassDensity::new::<kilogram_per_cubic_meter>(self.state.density)
    }

    /// Specific volume `v = 1/ρ` \[m³/kg\].
    pub fn get_specific_volume(&self) -> SpecificVolume {
        SpecificVolume::new::<cubic_meter_per_kilogram>(1.0 / self.state.density)
    }

    /// Specific internal energy \[J/kg\].
    pub fn get_specific_internal_energy(&self) -> AvailableEnergy {
        AvailableEnergy::new::<joule_per_kilogram>(self.state.internal_energy)
    }

    /// Specific enthalpy \[J/kg\].
    pub fn get_specific_enthalpy(&self) -> AvailableEnergy {
        AvailableEnergy::new::<joule_per_kilogram>(self.state.enthalpy)
    }

    /// Specific entropy \[J/(kg·K)\].
    pub fn get_specific_entropy(&self) -> SpecificHeatCapacity {
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(self.state.entropy)
    }

    /// Isochoric specific heat `c_v` \[J/(kg·K)\].
    pub fn get_cv(&self) -> SpecificHeatCapacity {
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(self.state.cv)
    }

    /// Isobaric specific heat `c_p` \[J/(kg·K)\].
    pub fn get_cp(&self) -> SpecificHeatCapacity {
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(self.state.cp)
    }

    /// Ratio of specific heats `γ = c_p/c_v` \[-\].
    pub fn get_specific_heat_ratio(&self) -> Ratio {
        Ratio::new::<ratio>(self.state.cp / self.state.cv)
    }

    /// Speed of sound \[m/s\].
    pub fn get_speed_of_sound(&self) -> Velocity {
        Velocity::new::<meter_per_second>(self.state.speed_of_sound)
    }

    /// The fixed control-volume [`Volume`] \[m³\].
    pub fn get_volume(&self) -> Volume {
        self.volume
    }

    /// The mass contained, `m = ρ·V` \[kg\].
    pub fn get_mass(&self) -> Mass {
        self.volume * self.get_density()
    }

    /// Vapour quality `x` \[-\] if this CV is a two-phase mixture, else `None`
    /// (single phase). `x = 0` is saturated liquid, `x = 1` saturated vapour.
    pub fn get_quality(&self) -> Option<f64> {
        self.quality
    }

    /// `true` if this control volume is a two-phase (liquid+vapour) mixture.
    pub fn is_two_phase(&self) -> bool {
        self.quality.is_some()
    }

    /// Dynamic viscosity `μ` \[Pa·s\], or `None` if the fluid has no supported
    /// viscosity model or the CV is two-phase (mixture transport not modelled).
    /// Critical enhancement is omitted (see [`crate::transport`]).
    pub fn get_viscosity(&self) -> Option<DynamicViscosity> {
        if self.quality.is_some() {
            return None;
        }
        viscosity(self.fluid, self.state.temperature, self.state.density)
            .map(DynamicViscosity::new::<pascal_second>)
    }

    /// Thermal conductivity `λ` \[W/(m·K)\], or `None` if the fluid has no
    /// supported conductivity model or the CV is two-phase. Critical
    /// enhancement is omitted (see [`crate::transport`]).
    pub fn get_thermal_conductivity(&self) -> Option<ThermalConductivity> {
        if self.quality.is_some() {
            return None;
        }
        conductivity(self.fluid, self.state.temperature, self.state.density)
            .map(ThermalConductivity::new::<watt_per_meter_kelvin>)
    }
}
