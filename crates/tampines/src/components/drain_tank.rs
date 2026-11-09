// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! Drain tank: a receiving vessel for molten salt, filled by gravity
//! drainage from a source vessel, with decay heat and passive heat removal
//! (gh:#1136, epic gh:#1125).
//!
//! > Research, education and V&V only. AI draft (Claude Opus 5.5 in Claude
//! > Code), unreviewed. Not for facility operation or safety decisions.
//!
//! **Generic:** no reactor's dimensions are built in. The MSRE-specific,
//! validated drain system lives in the MSRE repository (its MS-14). Following
//! `tampines`' rule, heat sources are **inputs**: the decay power is whatever
//! the caller passes each step.
//!
//! # Model
//!
//! - [`GravityDrain`]: a source vessel of constant cross-section `A_s`,
//!   draining through an orifice of area `A_o` and discharge coefficient
//!   `C_d` by Torricelli's law,
//!   `ṁ = ρ C_d A_o √(2 g h_s)`, `dh_s/dt = −ṁ / (ρ A_s)`.
//!   Heun's method (second order) in time; the step never drains more
//!   than the vessel holds.
//! - [`DrainTank`]: a vertical prismatic vessel of cross-section `A_t`.
//!   Inventory `m`, level `h = m / (ρ A_t)`, one lumped salt temperature `T`
//!   with constant `c_p`:
//!   `d(m c_p T)/dt = ṁ_in c_p T_in + P(t) − UA (T − T_sink)`.
//!   Explicit Euler in time. The energy that crosses each boundary per step
//!   is summed exactly, so the tank's energy books close to round-off
//!   ([`DrainTank::energy_in`], [`DrainTank::energy_removed`]).
//!
//! The freeze valve that usually sits between the two is
//! [`super::freeze_valve::FreezePlug`]; a caller starts the drain when the
//! plug reports [`super::freeze_valve::FreezePlug::is_open`].
//!
//! # V&V
//!
//! Criteria pre-registered on gh:#1136 (D1–D3); see `tests` for the
//! measured results.

use uom::si::acceleration::meter_per_second_squared;
use uom::si::area::square_meter;
use uom::si::energy::joule;
use uom::si::f64::*;
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermal_conductance::watt_per_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use crate::error::TampinesError;

/// A source vessel draining by gravity through an orifice (Torricelli).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GravityDrain {
    source_area: f64,
    orifice_area: f64,
    discharge_coefficient: f64,
    density: f64,
    gravity: f64,
    level: f64,
}

impl GravityDrain {
    /// A source vessel of cross-section `source_area` holding salt of
    /// `density` to `level` above an orifice of `orifice_area` with
    /// discharge coefficient `discharge_coefficient`, under `gravity`.
    pub fn new(
        source_area: Area,
        orifice_area: Area,
        discharge_coefficient: f64,
        density: MassDensity,
        gravity: Acceleration,
        level: Length,
    ) -> Result<Self, TampinesError> {
        let d = Self {
            source_area: source_area.get::<square_meter>(),
            orifice_area: orifice_area.get::<square_meter>(),
            discharge_coefficient,
            density: density.get::<kilogram_per_cubic_meter>(),
            gravity: gravity.get::<meter_per_second_squared>(),
            level: level.get::<meter>(),
        };
        let positive = d.source_area > 0.0
            && d.orifice_area > 0.0
            && d.density > 0.0
            && d.gravity > 0.0
            && d.discharge_coefficient > 0.0
            && d.discharge_coefficient <= 1.0;
        if !positive || !(d.level >= 0.0) || d.orifice_area > d.source_area {
            return Err(TampinesError::InvalidInput(format!(
                "gravity drain needs positive areas (orifice ≤ source), density and gravity, \
                 0 < C_d ≤ 1 and a level ≥ 0; got {d:?}"
            )));
        }
        Ok(d)
    }

    /// Torricelli's mass flow at the current level.
    pub fn mass_flow(&self) -> MassRate {
        MassRate::new::<kilogram_per_second>(self.flow())
    }

    fn flow(&self) -> f64 {
        self.flow_at(self.level)
    }

    fn flow_at(&self, level: f64) -> f64 {
        self.density * self.discharge_coefficient * self.orifice_area * (2.0 * self.gravity * level.max(0.0)).sqrt()
    }

    /// Advance by `dt` (Heun's method, second order) and return the mass
    /// that left the vessel. Never more than the vessel holds.
    ///
    /// Explicit Euler was tried first and **failed** D1's pre-registered
    /// 1e-3 (2.6e-3 at `3 t_e/4`, `dt` 0.1 s, 2026-11-09): its error relative
    /// to the level grows as the level falls. Heun replaced it; the
    /// criterion was not moved.
    pub fn step(&mut self, dt: Time) -> Result<Mass, TampinesError> {
        let dt = dt.get::<second>();
        if !(dt > 0.0) {
            return Err(TampinesError::InvalidInput(format!("time step must be > 0, got {dt}")));
        }
        let held = self.density * self.source_area * self.level;
        let per_level = self.density * self.source_area;
        let q1 = self.flow();
        let predicted = (self.level - q1 * dt / per_level).max(0.0);
        let q2 = self.flow_at(predicted);
        let out = (0.5 * (q1 + q2) * dt).min(held);
        self.level = if out == held { 0.0 } else { self.level - out / (self.density * self.source_area) };
        Ok(Mass::new::<kilogram>(out))
    }

    /// The source vessel's level above the orifice.
    pub fn level(&self) -> Length {
        Length::new::<meter>(self.level)
    }

    /// The mass the source vessel still holds above the orifice.
    pub fn inventory(&self) -> Mass {
        Mass::new::<kilogram>(self.density * self.source_area * self.level)
    }
}

/// A vertical prismatic drain tank: inventory, level and one lumped salt
/// temperature.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrainTank {
    area: f64,
    density: f64,
    cp: f64,
    mass: f64,
    temperature: f64,
    energy_in: f64,
    energy_removed: f64,
}

impl DrainTank {
    /// A tank of cross-section `area` holding `mass` of salt (`density`,
    /// constant `cp`) at `temperature`.
    pub fn new(
        area: Area,
        density: MassDensity,
        cp: SpecificHeatCapacity,
        mass: Mass,
        temperature: ThermodynamicTemperature,
    ) -> Result<Self, TampinesError> {
        let t = Self {
            area: area.get::<square_meter>(),
            density: density.get::<kilogram_per_cubic_meter>(),
            cp: cp.get::<joule_per_kilogram_kelvin>(),
            mass: mass.get::<kilogram>(),
            temperature: temperature.get::<kelvin>(),
            energy_in: 0.0,
            energy_removed: 0.0,
        };
        if !(t.area > 0.0 && t.density > 0.0 && t.cp > 0.0 && t.mass >= 0.0 && t.temperature > 0.0) {
            return Err(TampinesError::InvalidInput(format!(
                "drain tank needs positive area, density, c_p and temperature and a mass ≥ 0; got {t:?}"
            )));
        }
        Ok(t)
    }

    /// Advance by `dt` (explicit Euler): `inflow` of mass arrives at
    /// `inflow_temperature`, `decay_power` is deposited in the salt, and
    /// `ua (T − sink_temperature)` leaves through the passive path.
    ///
    /// With an empty tank and no inflow the temperature is held: there is
    /// no salt to heat.
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        dt: Time,
        inflow: Mass,
        inflow_temperature: ThermodynamicTemperature,
        decay_power: Power,
        ua: ThermalConductance,
        sink_temperature: ThermodynamicTemperature,
    ) -> Result<(), TampinesError> {
        let dt = dt.get::<second>();
        let dm = inflow.get::<kilogram>();
        if !(dt > 0.0) || !(dm >= 0.0) {
            return Err(TampinesError::InvalidInput(format!(
                "drain tank step needs dt > 0 and inflow ≥ 0; got dt {dt}, inflow {dm}"
            )));
        }
        let e_inflow = dm * self.cp * inflow_temperature.get::<kelvin>();
        let e_decay = decay_power.get::<watt>() * dt;
        let e_out = ua.get::<watt_per_kelvin>() * (self.temperature - sink_temperature.get::<kelvin>()) * dt;
        let new_mass = self.mass + dm;
        if new_mass > 0.0 {
            let e = self.mass * self.cp * self.temperature + e_inflow + e_decay - e_out;
            self.temperature = e / (new_mass * self.cp);
            self.energy_in += e_inflow + e_decay;
            self.energy_removed += e_out;
        }
        self.mass = new_mass;
        Ok(())
    }

    /// Salt inventory.
    pub fn mass(&self) -> Mass {
        Mass::new::<kilogram>(self.mass)
    }

    /// Salt level above the tank floor, `m / (ρ A_t)`.
    pub fn level(&self) -> Length {
        Length::new::<meter>(self.mass / (self.density * self.area))
    }

    /// The lumped salt temperature.
    pub fn temperature(&self) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(self.temperature)
    }

    /// Sensible enthalpy `m c_p T` (reference 0 K).
    pub fn enthalpy(&self) -> Energy {
        Energy::new::<joule>(self.mass * self.cp * self.temperature)
    }

    /// Energy brought in since construction: inflow enthalpy plus decay heat.
    pub fn energy_in(&self) -> Energy {
        Energy::new::<joule>(self.energy_in)
    }

    /// Energy removed through the passive path since construction.
    pub fn energy_removed(&self) -> Energy {
        Energy::new::<joule>(self.energy_removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use petir::mathf::RealMath;

    const RHO: f64 = 2000.0;
    const G: f64 = 9.806_65;

    fn drain(h0: f64) -> GravityDrain {
        GravityDrain::new(
            Area::new::<square_meter>(1.0),
            Area::new::<square_meter>(0.005),
            0.6,
            MassDensity::new::<kilogram_per_cubic_meter>(RHO),
            Acceleration::new::<meter_per_second_squared>(G),
            Length::new::<meter>(h0),
        )
        .unwrap()
    }

    fn tank(mass: f64, t: f64) -> DrainTank {
        DrainTank::new(
            Area::new::<square_meter>(2.0),
            MassDensity::new::<kilogram_per_cubic_meter>(RHO),
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(2000.0),
            Mass::new::<kilogram>(mass),
            ThermodynamicTemperature::new::<kelvin>(t),
        )
        .unwrap()
    }

    /// D1 (level against Torricelli) and D2 (the tank holds exactly what
    /// left the source). Source: `A_s` 1 m², orifice 0.005 m², `C_d` 0.6,
    /// `h_0` 2 m, so it empties at `t_e = √h_0 / ((C_d A_o/A_s) √(g/2))`
    /// ≈ 213 s. Level compared at `t_e/4`, `t_e/2` and `3 t_e/4`,
    /// `dt` = 0.1 s.
    ///
    /// **Results (2026-11-09, release, Heun):** D1 relative level error
    /// 4.9e-8, 2.2e-7 and 1.3e-6 at the three times (criterion 1e-3; explicit
    /// Euler gave 1.8e-4, 6.5e-4 and **2.6e-3, a fail**, before Heun replaced
    /// it). D2: the tank holds 4000.000000000 kg against 4000 kg drained,
    /// relative 2.0e-15 (criterion 1e-12); level 1.000000 m in a 2 m² tank.
    #[test]
    fn d1_d2_the_source_drains_by_torricelli_and_the_tank_holds_it() {
        let h0: f64 = 2.0;
        let k: f64 = 0.6 * 0.005 / 1.0 * (G / 2.0).sqrt();
        let t_empty = h0.sqrt() / k;
        let exact = |t: f64| (h0.sqrt() - k * t).max(0.0).powi(2);
        let mut d = drain(h0);
        let m0 = d.inventory().get::<kilogram>();
        let mut tk = tank(0.0, 900.0);
        let dt = 0.1;
        let mut t = 0.0;
        let mut worst: f64 = 0.0;
        for frac in [0.25, 0.5, 0.75] {
            while t + 0.5 * dt < frac * t_empty {
                let out = d.step(Time::new::<second>(dt)).unwrap();
                tk.step(
                    Time::new::<second>(dt),
                    out,
                    ThermodynamicTemperature::new::<kelvin>(900.0),
                    Power::new::<watt>(0.0),
                    ThermalConductance::new::<watt_per_kelvin>(0.0),
                    ThermodynamicTemperature::new::<kelvin>(300.0),
                )
                .unwrap();
                t += dt;
            }
            let h = d.level().get::<meter>();
            let rel = (h / exact(t) - 1.0).abs();
            println!("D1: t {t:7.2} s ({frac} t_e): level {h:.6} m vs Torricelli {:.6} m, rel {rel:.2e}", exact(t));
            worst = worst.max(rel);
        }
        assert!(worst <= 1e-3, "D1 worst relative level error {worst:.3e}");
        // Run it dry: the step must not overdraw.
        for _ in 0..10_000 {
            let out = d.step(Time::new::<second>(dt)).unwrap();
            tk.step(
                Time::new::<second>(dt),
                out,
                ThermodynamicTemperature::new::<kelvin>(900.0),
                Power::new::<watt>(0.0),
                ThermalConductance::new::<watt_per_kelvin>(0.0),
                ThermodynamicTemperature::new::<kelvin>(300.0),
            )
            .unwrap();
        }
        assert_eq!(d.level().get::<meter>(), 0.0);
        let m = tk.mass().get::<kilogram>();
        let d2 = (m / m0 - 1.0).abs();
        println!("D2: tank {m:.9} kg vs drained {m0:.9} kg, rel {d2:.2e}; tank level {:.6} m", tk.level().get::<meter>());
        assert!(d2 <= 1e-12, "D2 mass books {d2:.3e}");
        assert!((tk.level().get::<meter>() - m0 / (RHO * 2.0)).abs() <= 1e-12);
        assert!((tk.temperature().get::<kelvin>() - 900.0).abs() <= 1e-9);
    }

    /// D3: a full tank with constant decay power `P` and constant `UA`
    /// relaxes as `T∞ + (T_0 − T∞) e^{−t/τ}`, `T∞ = T_sink + P/UA`,
    /// `τ = m c_p / UA`. `dt = τ/1000`, compared over five time constants.
    ///
    /// **Results (2026-11-09, release):** τ 20 000 s, T∞ 350 K; worst
    /// relative temperature error over 5 τ **1.96e-4** (criterion 1e-3,
    /// explicit Euler, first order as predicted); T(5 τ) 353.70 K. Energy
    /// books close within 1e-9 of the initial enthalpy.
    #[test]
    fn d3_decay_heat_and_passive_removal_relax_exponentially() {
        let (m, cp, ua, p, t0, ts) = (4000.0, 2000.0, 400.0, 20_000.0, 900.0, 300.0);
        let tau = m * cp / ua;
        let t_inf = ts + p / ua;
        let mut tk = tank(m, t0);
        let dt = tau / 1000.0;
        let mut worst: f64 = 0.0;
        for k in 1..=5000 {
            tk.step(
                Time::new::<second>(dt),
                Mass::new::<kilogram>(0.0),
                ThermodynamicTemperature::new::<kelvin>(ts),
                Power::new::<watt>(p),
                ThermalConductance::new::<watt_per_kelvin>(ua),
                ThermodynamicTemperature::new::<kelvin>(ts),
            )
            .unwrap();
            let t = k as f64 * dt;
            let exact = t_inf + (t0 - t_inf) * (-t / tau).r_exp();
            worst = worst.max((tk.temperature().get::<kelvin>() / exact - 1.0).abs());
        }
        println!("D3: τ {tau:.0} s, T∞ {t_inf:.1} K; worst relative T error over 5 τ {worst:.2e}; T(5τ) {:.4} K", tk.temperature().get::<kelvin>());
        assert!(worst <= 1e-3, "D3 worst {worst:.3e}");
        let books = tk.enthalpy().get::<joule>() - m * cp * t0 - (tk.energy_in() - tk.energy_removed()).get::<joule>();
        assert!(books.abs() <= 1e-9 * m * cp * t0, "energy books {books}");
    }

    /// Mixing: cold salt into a hot tank, the books close and the
    /// temperature is the mass-weighted mean.
    #[test]
    fn inflow_mixes_and_the_energy_books_close() {
        let mut tk = tank(1000.0, 900.0);
        tk.step(
            Time::new::<second>(1.0),
            Mass::new::<kilogram>(3000.0),
            ThermodynamicTemperature::new::<kelvin>(800.0),
            Power::new::<watt>(0.0),
            ThermalConductance::new::<watt_per_kelvin>(0.0),
            ThermodynamicTemperature::new::<kelvin>(300.0),
        )
        .unwrap();
        assert!((tk.temperature().get::<kelvin>() - 825.0).abs() <= 1e-9);
        let books = tk.enthalpy().get::<joule>() - 1000.0 * 2000.0 * 900.0 - tk.energy_in().get::<joule>();
        assert!(books.abs() <= 1e-6);
        assert!(drain(1.0).step(Time::new::<second>(0.0)).is_err());
        assert!(GravityDrain::new(
            Area::new::<square_meter>(1.0),
            Area::new::<square_meter>(2.0),
            0.6,
            MassDensity::new::<kilogram_per_cubic_meter>(RHO),
            Acceleration::new::<meter_per_second_squared>(G),
            Length::new::<meter>(1.0),
        )
        .is_err());
    }
}
