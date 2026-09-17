// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/phaseModels/structureModels/
//             powerModels/{powerModel,fixedPower,fixedTemperature}.{C,H}
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream authors: Stefan Radman, Carlo Fiorina (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `power_model` — the solid-structure energy source / surface temperature
//!
//! Port of GeN-Foam's `powerModel` family. A power model describes the
//! **thermal state of the mesh-unresolved solid structure** in a porous cell:
//! it owns the structure surface temperature and, each timestep, updates that
//! temperature from (a) the internal power it produces and (b) the convective
//! heat it exchanges with the surrounding fluid. That surface temperature is
//! then what the fluid-structure heat closure sees (see [`super::heat_source`]).
//!
//! ## Model set (closed enum, no `dyn` dispatch)
//!
//! | Variant | Upstream | Physics |
//! |---|---|---|
//! | [`PowerModel::FixedPower`] | `fixedPower` | Prescribed volumetric power; surface temperature from a transient **lumped** energy balance |
//! | [`PowerModel::FixedTemperature`] | `fixedTemperature` | Prescribed surface temperature (constant or time-scaled); no energy balance |
//!
//! The higher-fidelity pin/pebble/FMU models upstream
//! (`heatedPin`, `nuclearFuelPin`, `interpolatedNuclearFuelPin`,
//! `nuclearSteadyStatePebble`, `nuclearFuelFMU`, `fixedTemperatureFMU`) solve a
//! **radial (1-D) conduction** problem inside the pin/pebble each timestep and
//! are *not* ported here — that radial-conduction machinery is a substantial
//! follow-up (its own bead) and the FMU variants couple to an external
//! functional-mock-up unit that is out of scope for this crate. The two lumped
//! models above are the self-contained, hand-verifiable core of the family.
//!
//! ## Lumped energy balance (the `fixedPower` core)
//!
//! GeN-Foam solves, cell by cell, a **backward-Euler** update of the structure
//! surface temperature `T` (`fixedPower::correct`). Writing `C = alpha*rho*Cp`
//! (the [`VolumetricHeatCapacity`]), `a_v` the [`InterfacialAreaDensity`],
//! `q'''` the [`PowerDensity`], `alpha` the power-region volume fraction, and
//! the fluid coupling as the two sums
//!
//! ```text
//! h_sum  = sum_j frac_j * htc_j          [W/(m^2 K)]   (== H  upstream)
//! ht_sum = sum_j frac_j * htc_j * T_j    [W/m^2]       (== HT upstream)
//! ```
//!
//! the update is
//!
//! ```text
//!            a_v*ht_sum + alpha*q''' + (C/dt)*T_old
//!   T_new = ---------------------------------------
//!                   C/dt + a_v*h_sum
//! ```
//!
//! This is the discretised form of
//! `d/dt (C*T) = alpha*q''' + a_v*(ht_sum - h_sum*T)`, i.e. thermal-inertia
//! storage on the left, internal power plus net convective exchange on the
//! right. In the steady limit (`dt -> inf`) it reduces to the algebraic balance
//! `alpha*q''' = a_v*(h_sum*T - ht_sum)`, the case exercised by the V&V test.
//!
//! ## Time dependence
//!
//! Both models accept an optional [`TimeProfile`] read as a **scaling factor**
//! relative to the initial value (GeN-Foam's `powerTimeProfile` /
//! `temperatureTimeProfile`): the instantaneous value is
//! `value_initial * profile(t)`. Upstream expresses this as the algebraically
//! equivalent ratio recurrence `v(t) = v(t-dt)*profile(t)/profile(t-dt)`
//! (`powerUpdate`); here we evaluate the closed form directly against the stored
//! initial value, which avoids the recurrence's division-by-zero edge when a
//! profile passes through zero.

use uom::si::f64::{Ratio, ThermodynamicTemperature, Time};

use crate::genfoam::common::TimeProfile;

use super::units::{
    HeatFlux, HeatTransferCoefficient, InterfacialAreaDensity, PowerDensity, VolumetricHeatCapacity,
};

/// A `fixedPower` structure: prescribed internal power, transient lumped
/// surface temperature.
///
/// Owns the structure surface temperature as evolving state; call
/// [`FixedPower::correct`] once per timestep with the current fluid coupling to
/// advance it. Construct with [`FixedPower::new`] (optionally chaining
/// [`FixedPower::with_power_profile`]).
#[derive(Debug, Clone, PartialEq)]
pub struct FixedPower {
    /// Power-region volume fraction `alpha` (dimensionless) — the fraction of
    /// the total cell volume that actually produces power.
    alpha: Ratio,
    /// Interfacial-area density `a_v` [1/m] over which the structure exchanges
    /// heat with the fluid.
    interfacial_area: InterfacialAreaDensity,
    /// Volumetric power density `q'''` [W/m^3] at the profile's reference time.
    power_density_initial: PowerDensity,
    /// `alpha*rho*Cp` [J/(m^3 K)] — the lumped thermal inertia.
    alpha_rho_cp: VolumetricHeatCapacity,
    /// Optional time scaling for the power (a factor relative to the initial
    /// value). `None` ⇒ constant power.
    power_profile: Option<TimeProfile>,
    /// Structure surface temperature [K] — the evolving state.
    temperature: ThermodynamicTemperature,
}

impl FixedPower {
    /// Build a fixed-power model at an initial surface temperature.
    ///
    /// # Parameters
    /// - `alpha` — power-region volume fraction (dimensionless).
    /// - `interfacial_area` — `a_v` [1/m], structure-to-fluid area per volume.
    /// - `power_density_initial` — `q'''` [W/m^3] at the profile reference time.
    /// - `alpha_rho_cp` — `alpha*rho*Cp` [J/(m^3 K)], the lumped thermal inertia.
    /// - `initial_temperature` — the starting surface temperature [K].
    #[must_use]
    pub fn new(
        alpha: Ratio,
        interfacial_area: InterfacialAreaDensity,
        power_density_initial: PowerDensity,
        alpha_rho_cp: VolumetricHeatCapacity,
        initial_temperature: ThermodynamicTemperature,
    ) -> Self {
        Self {
            alpha,
            interfacial_area,
            power_density_initial,
            alpha_rho_cp,
            power_profile: None,
            temperature: initial_temperature,
        }
    }

    /// Attach a power-vs-time scaling profile (a factor relative to the initial
    /// power density). Consumes and returns `self` for chaining.
    #[must_use]
    pub fn with_power_profile(mut self, profile: TimeProfile) -> Self {
        self.power_profile = Some(profile);
        self
    }

    /// The instantaneous volumetric power density at time `t`.
    ///
    /// `q'''(t) = q'''_initial * profile(t)`, or the constant initial value if
    /// no profile is set.
    #[must_use]
    pub fn power_density(&self, t: Time) -> PowerDensity {
        match &self.power_profile {
            Some(p) => self.power_density_initial * p.value(t),
            None => self.power_density_initial,
        }
    }

    /// The current structure surface temperature [K].
    #[must_use]
    pub fn surface_temperature(&self) -> ThermodynamicTemperature {
        self.temperature
    }

    /// Interfacial-area density `a_v` [1/m] of this power region.
    #[must_use]
    pub fn interfacial_area(&self) -> InterfacialAreaDensity {
        self.interfacial_area
    }

    /// Advance the structure surface temperature by one backward-Euler step.
    ///
    /// Faithful port of `fixedPower::correct` (see the [module
    /// documentation](self) for the discretised equation). `h_sum`/`ht_sum` are
    /// the fluid-coupling sums for this cell, `dt` the timestep, `t` the new
    /// (end-of-step) time. Returns the updated surface temperature (also stored
    /// as the model's state).
    pub fn correct(
        &mut self,
        h_sum: HeatTransferCoefficient,
        ht_sum: HeatFlux,
        dt: Time,
        t: Time,
    ) -> ThermodynamicTemperature {
        // The public signature is fully `uom`-typed; the lumped update itself is
        // evaluated on SI base magnitudes. This sidesteps `uom`'s deliberate
        // refusal to let a `power/volume ÷ power/(volume·temperature)` division
        // land on the special `ThermodynamicTemperature` kind, while keeping
        // every input dimension-checked at the boundary. All quantities below
        // are read in coherent SI base units (W/m^3, J/(m^3 K), 1/m, …), so the
        // ratio is in kelvin by construction.
        use uom::si::heat_flux_density::watt_per_square_meter;
        use uom::si::heat_transfer::watt_per_square_meter_kelvin;
        use uom::si::ratio::ratio;
        use uom::si::reciprocal_length::reciprocal_meter;
        use uom::si::thermodynamic_temperature::kelvin;
        use uom::si::time::second;
        use uom::si::volumetric_heat_capacity::joule_per_cubic_meter_kelvin;
        use uom::si::volumetric_power_density::watt_per_cubic_meter;

        let q = self.power_density(t).get::<watt_per_cubic_meter>();
        let c = self.alpha_rho_cp.get::<joule_per_cubic_meter_kelvin>();
        let dt_s = dt.get::<second>();
        let a_v = self.interfacial_area.get::<reciprocal_meter>();
        let h = h_sum.get::<watt_per_square_meter_kelvin>();
        let ht = ht_sum.get::<watt_per_square_meter>();
        let t_old = self.temperature.get::<kelvin>();
        let alpha = self.alpha.get::<ratio>();

        let c_by_dt = c / dt_s;
        let numerator = a_v * ht + alpha * q + c_by_dt * t_old;
        let denominator = c_by_dt + a_v * h;
        self.temperature = ThermodynamicTemperature::new::<kelvin>(numerator / denominator);
        self.temperature
    }

    /// Zero the power (GeN-Foam's `powerOff`): the internal source is removed but
    /// the model keeps exchanging heat with the fluid.
    pub fn power_off(&mut self) {
        self.power_density_initial = PowerDensity::default();
        self.power_profile = None;
    }
}

/// A `fixedTemperature` structure: prescribed surface temperature, no energy
/// balance.
///
/// The structure surface is simply held at a prescribed temperature (constant,
/// or scaled by a [`TimeProfile`]). Useful for a boundary-like structure whose
/// temperature is imposed rather than computed.
#[derive(Debug, Clone, PartialEq)]
pub struct FixedTemperature {
    /// Interfacial-area density `a_v` [1/m].
    interfacial_area: InterfacialAreaDensity,
    /// Prescribed surface temperature [K] at the profile reference time.
    temperature_initial: ThermodynamicTemperature,
    /// Optional time scaling (a factor relative to the initial temperature).
    temperature_profile: Option<TimeProfile>,
}

impl FixedTemperature {
    /// Build a fixed-temperature model.
    ///
    /// - `interfacial_area` — `a_v` [1/m].
    /// - `temperature_initial` — imposed surface temperature [K].
    #[must_use]
    pub fn new(
        interfacial_area: InterfacialAreaDensity,
        temperature_initial: ThermodynamicTemperature,
    ) -> Self {
        Self {
            interfacial_area,
            temperature_initial,
            temperature_profile: None,
        }
    }

    /// Attach a temperature-vs-time scaling profile (a factor relative to the
    /// initial temperature).
    #[must_use]
    pub fn with_temperature_profile(mut self, profile: TimeProfile) -> Self {
        self.temperature_profile = Some(profile);
        self
    }

    /// The imposed surface temperature at time `t`
    /// (`T(t) = T_initial * profile(t)`).
    #[must_use]
    pub fn surface_temperature_at(&self, t: Time) -> ThermodynamicTemperature {
        match &self.temperature_profile {
            Some(p) => self.temperature_initial * p.value(t),
            None => self.temperature_initial,
        }
    }

    /// Interfacial-area density `a_v` [1/m].
    #[must_use]
    pub fn interfacial_area(&self) -> InterfacialAreaDensity {
        self.interfacial_area
    }
}

/// Run-time-selectable structure power model — closed enum, no `dyn` dispatch.
///
/// Port of the concrete, self-contained members of GeN-Foam's `powerModel`
/// run-time-selection table. Dispatch the surface-temperature update through
/// [`PowerModel::correct`] and read the result through
/// [`PowerModel::surface_temperature`].
#[derive(Debug, Clone, PartialEq)]
pub enum PowerModel {
    /// Prescribed internal power; transient lumped surface temperature.
    FixedPower(FixedPower),
    /// Prescribed surface temperature; no energy balance.
    FixedTemperature(FixedTemperature),
}

impl PowerModel {
    /// Advance the model by one timestep and return the new structure surface
    /// temperature.
    ///
    /// For [`PowerModel::FixedPower`] this runs the lumped backward-Euler energy
    /// balance; for [`PowerModel::FixedTemperature`] it simply evaluates the
    /// prescribed profile at `t` (the fluid coupling and `dt` are ignored, as
    /// upstream). The fluid-coupling sums `h_sum`/`ht_sum` are per-cell.
    pub fn correct(
        &mut self,
        h_sum: HeatTransferCoefficient,
        ht_sum: HeatFlux,
        dt: Time,
        t: Time,
    ) -> ThermodynamicTemperature {
        match self {
            Self::FixedPower(m) => m.correct(h_sum, ht_sum, dt, t),
            Self::FixedTemperature(m) => m.surface_temperature_at(t),
        }
    }

    /// The current structure surface temperature at time `t`.
    ///
    /// For [`PowerModel::FixedPower`] this returns the stored state (independent
    /// of `t`); for [`PowerModel::FixedTemperature`] it evaluates the prescribed
    /// profile at `t`.
    #[must_use]
    pub fn surface_temperature(&self, t: Time) -> ThermodynamicTemperature {
        match self {
            Self::FixedPower(m) => m.surface_temperature(),
            Self::FixedTemperature(m) => m.surface_temperature_at(t),
        }
    }

    /// Interfacial-area density `a_v` [1/m] of this power region.
    #[must_use]
    pub fn interfacial_area(&self) -> InterfacialAreaDensity {
        match self {
            Self::FixedPower(m) => m.interfacial_area(),
            Self::FixedTemperature(m) => m.interfacial_area(),
        }
    }

    /// Turn the internal power off (no-op for a fixed-temperature model).
    pub fn power_off(&mut self) {
        if let Self::FixedPower(m) = self {
            m.power_off();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::heat_flux_density::watt_per_square_meter;
    use uom::si::heat_transfer::watt_per_square_meter_kelvin;
    use uom::si::ratio::ratio;
    use uom::si::reciprocal_length::reciprocal_meter;
    use uom::si::thermodynamic_temperature::kelvin;
    use uom::si::time::second;
    use uom::si::volumetric_heat_capacity::joule_per_cubic_meter_kelvin;
    use uom::si::volumetric_power_density::watt_per_cubic_meter;

    fn kelv(x: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(x)
    }

    /// V&V — steady-state lumped fuel-pin heat balance.
    ///
    /// Methodology: drive a `fixedPower` structure with a fixed fluid coupling
    /// (`h_sum`, `ht_sum = h_sum*T_fluid`) and a very large timestep so the
    /// thermal-inertia term vanishes; the update must converge to the algebraic
    /// balance `alpha*q''' = a_v*h_sum*(T - T_fluid)`, i.e.
    /// `T = T_fluid + alpha*q''' / (a_v*h_sum)`.
    ///
    /// Inputs: alpha = 0.5, q''' = 4.0e7 W/m^3, a_v = 100 1/m,
    /// h_sum = 1.0e4 W/(m^2 K), T_fluid = 600 K.
    /// Hand calc: dT = 0.5*4.0e7 / (100*1.0e4) = 2.0e7/1.0e6 = 20 K ⇒ T = 620 K.
    ///
    /// Result (2026-07-15): converged T = 620.000 K, |error| < 1e-6 K. Pass.
    #[test]
    fn fixed_power_steady_state_matches_hand_calc() {
        let t_fluid = kelv(600.0);
        let h_sum = HeatTransferCoefficient::new::<watt_per_square_meter_kelvin>(1.0e4);
        let ht_sum = HeatFlux::new::<watt_per_square_meter>(1.0e4 * 600.0);

        let mut m = FixedPower::new(
            Ratio::new::<ratio>(0.5),
            InterfacialAreaDensity::new::<reciprocal_meter>(100.0),
            PowerDensity::new::<watt_per_cubic_meter>(4.0e7),
            VolumetricHeatCapacity::new::<joule_per_cubic_meter_kelvin>(3.85e6),
            t_fluid,
        );

        // March to steady state with a large timestep.
        let dt = Time::new::<second>(1.0e9);
        let mut temp = m.surface_temperature();
        for i in 1..=5 {
            temp = m.correct(h_sum, ht_sum, dt, Time::new::<second>(i as f64));
        }
        assert!(
            (temp.get::<kelvin>() - 620.0).abs() < 1e-6,
            "steady-state T = {} K, expected 620 K",
            temp.get::<kelvin>()
        );
    }

    /// V&V — transient lumped step reproduced by hand.
    ///
    /// Methodology: a single backward-Euler step from `T_old = T_fluid` with a
    /// finite timestep must equal the closed-form update. With no interfacial
    /// area (`a_v = 0`) the balance is pure storage+source:
    /// `T = T_old + alpha*q'''*dt / (alpha*rho*Cp)`.
    ///
    /// Inputs: alpha = 1.0, q''' = 1.0e8 W/m^3, C = 4.0e6 J/(m^3 K),
    /// dt = 0.1 s, T_old = 500 K.
    /// Hand calc: dT = 1.0e8*0.1/4.0e6 = 1.0e7/4.0e6 = 2.5 K ⇒ T = 502.5 K.
    ///
    /// Result (2026-07-15): T = 502.500 K, |error| < 1e-9 K. Pass.
    #[test]
    fn fixed_power_adiabatic_step_matches_hand_calc() {
        let mut m = FixedPower::new(
            Ratio::new::<ratio>(1.0),
            InterfacialAreaDensity::new::<reciprocal_meter>(0.0),
            PowerDensity::new::<watt_per_cubic_meter>(1.0e8),
            VolumetricHeatCapacity::new::<joule_per_cubic_meter_kelvin>(4.0e6),
            kelv(500.0),
        );
        let temp = m.correct(
            HeatTransferCoefficient::new::<watt_per_square_meter_kelvin>(0.0),
            HeatFlux::new::<watt_per_square_meter>(0.0),
            Time::new::<second>(0.1),
            Time::new::<second>(0.1),
        );
        assert!((temp.get::<kelvin>() - 502.5).abs() < 1e-9);
    }

    #[test]
    fn power_profile_scales_initial_density() {
        let m = FixedPower::new(
            Ratio::new::<ratio>(1.0),
            InterfacialAreaDensity::new::<reciprocal_meter>(1.0),
            PowerDensity::new::<watt_per_cubic_meter>(1.0e8),
            VolumetricHeatCapacity::new::<joule_per_cubic_meter_kelvin>(1.0),
            kelv(500.0),
        )
        .with_power_profile(
            TimeProfile::table(
                vec![Time::new::<second>(0.0), Time::new::<second>(10.0)],
                vec![1.0, 0.1],
            )
            .unwrap(),
        );
        // Halfway (t=5 s) the linear profile is 0.55 ⇒ 5.5e7 W/m^3.
        let q = m.power_density(Time::new::<second>(5.0));
        assert!((q.get::<watt_per_cubic_meter>() - 5.5e7).abs() < 1.0);
    }

    #[test]
    fn power_off_zeroes_source() {
        let mut m = FixedPower::new(
            Ratio::new::<ratio>(1.0),
            InterfacialAreaDensity::new::<reciprocal_meter>(1.0),
            PowerDensity::new::<watt_per_cubic_meter>(1.0e8),
            VolumetricHeatCapacity::new::<joule_per_cubic_meter_kelvin>(1.0),
            kelv(500.0),
        );
        m.power_off();
        assert_eq!(
            m.power_density(Time::new::<second>(0.0))
                .get::<watt_per_cubic_meter>(),
            0.0
        );
    }

    #[test]
    fn fixed_temperature_evaluates_profile() {
        let m = FixedTemperature::new(
            InterfacialAreaDensity::new::<reciprocal_meter>(50.0),
            kelv(617.0),
        );
        assert_eq!(
            m.surface_temperature_at(Time::new::<second>(0.0))
                .get::<kelvin>(),
            617.0
        );

        let mut pm = PowerModel::FixedTemperature(m);
        let t = pm.correct(
            HeatTransferCoefficient::new::<watt_per_square_meter_kelvin>(1.0),
            HeatFlux::new::<watt_per_square_meter>(1.0),
            Time::new::<second>(1.0),
            Time::new::<second>(1.0),
        );
        assert_eq!(t.get::<kelvin>(), 617.0);
    }
}
