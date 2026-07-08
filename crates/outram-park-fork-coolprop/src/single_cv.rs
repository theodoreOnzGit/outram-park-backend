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
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::velocity::meter_per_second;

use crate::flash::{state_ph, state_ps, state_pt, FlashError};
use crate::fluid::Fluid;
use crate::props::{state_trho, FluidState};

/// A single, uniform control volume of one [`Fluid`] at equilibrium.
///
/// Holds the fluid identity, its full single-phase [`FluidState`] (raw SI), and
/// the fixed control-volume [`Volume`]. Build it with a constructor, then read
/// properties with the `get_*` accessors (all `uom`-typed).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OPCPFluidSingleCV {
    fluid: Fluid,
    state: FluidState,
    volume: Volume,
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
        Self { fluid, state, volume }
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
        Ok(Self { fluid, state, volume })
    }

    /// Construct from pressure and specific enthalpy (single-phase `(p, h)`
    /// flash). Returns [`FlashError`] on non-convergence.
    pub fn try_new_from_ph(
        fluid: Fluid,
        pressure: Pressure,
        specific_enthalpy: AvailableEnergy,
        volume: Volume,
    ) -> Result<Self, FlashError> {
        let state = state_ph(
            fluid,
            pressure.get::<pascal>(),
            specific_enthalpy.get::<joule_per_kilogram>(),
        )?;
        Ok(Self { fluid, state, volume })
    }

    /// Construct from pressure and specific entropy (single-phase `(p, s)`
    /// flash). Returns [`FlashError`] on non-convergence.
    pub fn try_new_from_ps(
        fluid: Fluid,
        pressure: Pressure,
        specific_entropy: SpecificHeatCapacity,
        volume: Volume,
    ) -> Result<Self, FlashError> {
        let state = state_ps(
            fluid,
            pressure.get::<pascal>(),
            specific_entropy.get::<joule_per_kilogram_kelvin>(),
        )?;
        Ok(Self { fluid, state, volume })
    }

    // ── Re-equilibration setters (keep the same volume) ─────────────────────

    /// Re-equilibrate to a new `(T, ρ)` state (EOS-native, infallible).
    pub fn set_trho(&mut self, temperature: ThermodynamicTemperature, density: MassDensity) {
        self.state = state_trho(
            self.fluid,
            temperature.get::<kelvin>(),
            density.get::<kilogram_per_cubic_meter>(),
        );
    }

    /// Re-equilibrate to a new `(p, T)` state (single-phase flash).
    pub fn set_pt(
        &mut self,
        pressure: Pressure,
        temperature: ThermodynamicTemperature,
    ) -> Result<(), FlashError> {
        self.state = state_pt(self.fluid, temperature.get::<kelvin>(), pressure.get::<pascal>())?;
        Ok(())
    }

    /// Re-equilibrate to a new `(p, h)` state (single-phase flash).
    pub fn set_ph(&mut self, pressure: Pressure, specific_enthalpy: AvailableEnergy) -> Result<(), FlashError> {
        self.state = state_ph(
            self.fluid,
            pressure.get::<pascal>(),
            specific_enthalpy.get::<joule_per_kilogram>(),
        )?;
        Ok(())
    }

    /// Re-equilibrate to a new `(p, s)` state (single-phase flash).
    pub fn set_ps(&mut self, pressure: Pressure, specific_entropy: SpecificHeatCapacity) -> Result<(), FlashError> {
        self.state = state_ps(
            self.fluid,
            pressure.get::<pascal>(),
            specific_entropy.get::<joule_per_kilogram_kelvin>(),
        )?;
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
}
