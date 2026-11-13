// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! Freeze valve: a frozen salt plug that thaws and opens when it is heated
//! (gh:#1136, epic gh:#1125).
//!
//! > Research, education and V&V only. AI draft (Claude Opus 5.5 in Claude
//! > Code), unreviewed. Not for facility operation or safety decisions.
//!
//! **Generic:** no reactor's dimensions are built in. The MSRE-specific,
//! validated freeze-valve model lives in the MSRE repository (its MS-14).
//!
//! # Model
//!
//! A 1-D slab plug of thickness `L`, conduction with melting by the
//! **fixed-grid enthalpy method**. Each cell carries a volumetric enthalpy
//! `H`, with `H = 0` the solid at the melting point:
//!
//! ```text
//! H < 0              solid,   T = T_m + H / (ρ c_s)
//! 0 ≤ H ≤ ρ L_f      mushy,   T = T_m, liquid fraction f = H / (ρ L_f)
//! H > ρ L_f          liquid,  T = T_m + (H − ρ L_f) / (ρ c_l)
//! ```
//!
//! The conductivity is `f k_l + (1 − f) k_s`, harmonically averaged at faces.
//! Boundaries:
//! - **x = 0, the heated face:** a prescribed temperature (heater on,
//!   cooling lost) or a prescribed heat flux.
//! - **x = L:** insulated, i.e. the plug's symmetry plane.
//!
//! The plug **opens** when the melt front reaches `x = L`. The front is the
//! melted thickness `s = Σ f_i Δx`, which conserves the latent heat exactly.
//!
//! Time stepping is explicit, with as many equal sub-steps as stability
//! needs (`Δt_sub ≤ 0.4 Δx² / α_max`). The heat that crosses each face is
//! summed exactly, so energy is conserved to round-off per step.
//! [`FreezePlug::heat_in`] records the heat that entered.
//!
//! # V&V
//!
//! Criteria pre-registered on gh:#1136; see `tests` for the measured
//! results.

use uom::si::area::square_meter;
use uom::si::f64::*;
use uom::si::heat_flux_density::watt_per_square_meter;
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::energy::joule;
use petir::mathf::RealMath;

use crate::error::TampinesError;

/// Thermophysical properties of the plug material.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlugMaterial {
    /// Density (taken equal in both phases).
    pub density: MassDensity,
    /// Solid specific heat.
    pub cp_solid: SpecificHeatCapacity,
    /// Liquid specific heat.
    pub cp_liquid: SpecificHeatCapacity,
    /// Solid conductivity.
    pub k_solid: ThermalConductivity,
    /// Liquid conductivity.
    pub k_liquid: ThermalConductivity,
    /// Latent heat of fusion.
    pub latent_heat: AvailableEnergy,
    /// Melting temperature.
    pub melting_point: ThermodynamicTemperature,
}

/// The condition on the plug's heated face, `x = 0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HeatedFace {
    /// The face is held at this temperature.
    Temperature(ThermodynamicTemperature),
    /// This heat flux enters the plug (negative removes heat).
    HeatFlux(HeatFluxDensity),
}

/// A 1-D frozen-salt plug, thickness `L`, `n` cells.
#[derive(Debug, Clone, PartialEq)]
pub struct FreezePlug {
    material: PlugMaterial,
    thickness: f64,
    /// Cross-section the heat flows through \[m²\], for the energy books.
    area: f64,
    /// Volumetric enthalpy per cell, `H = 0` at the solid melting point \[J/m³\].
    h: Vec<f64>,
    /// Heat that has entered through `x = 0` since construction \[J\].
    heat_in: f64,
    /// Time since construction \[s\].
    time: f64,
}

impl FreezePlug {
    /// A plug of `thickness`, cross-section `area` and `cells` cells, all at
    /// `initial_temperature` (solid if at or below the melting point).
    pub fn new(
        material: PlugMaterial,
        thickness: Length,
        area: Area,
        cells: usize,
        initial_temperature: ThermodynamicTemperature,
    ) -> Result<Self, TampinesError> {
        let l = thickness.get::<meter>();
        if !(l > 0.0) || cells < 2 {
            return Err(TampinesError::InvalidInput(format!(
                "freeze plug needs a positive thickness and at least 2 cells (got {l} m, {cells})"
            )));
        }
        let plug = Self {
            material,
            thickness: l,
            area: area.get::<square_meter>(),
            h: vec![0.0; cells],
            heat_in: 0.0,
            time: 0.0,
        };
        let h0 = plug.enthalpy_of(initial_temperature.get::<kelvin>(), initial_temperature.get::<kelvin>() > plug.tm());
        Ok(Self { h: vec![h0; cells], ..plug })
    }

    fn rho(&self) -> f64 {
        self.material.density.get::<kilogram_per_cubic_meter>()
    }
    fn tm(&self) -> f64 {
        self.material.melting_point.get::<kelvin>()
    }
    fn rho_lf(&self) -> f64 {
        self.rho() * self.material.latent_heat.get::<joule_per_kilogram>()
    }

    /// Volumetric enthalpy of material at `t` \[K\]; at exactly `T_m`, solid
    /// unless `liquid`.
    fn enthalpy_of(&self, t: f64, liquid: bool) -> f64 {
        let tm = self.tm();
        if t < tm || (t == tm && !liquid) {
            self.rho() * self.material.cp_solid.get::<joule_per_kilogram_kelvin>() * (t - tm)
        } else {
            self.rho_lf() + self.rho() * self.material.cp_liquid.get::<joule_per_kilogram_kelvin>() * (t - tm)
        }
    }

    /// Temperature \[K\] and liquid fraction of a cell enthalpy.
    fn state_of(&self, h: f64) -> (f64, f64) {
        let tm = self.tm();
        let rlf = self.rho_lf();
        if h < 0.0 {
            (tm + h / (self.rho() * self.material.cp_solid.get::<joule_per_kilogram_kelvin>()), 0.0)
        } else if h <= rlf {
            (tm, h / rlf)
        } else {
            (tm + (h - rlf) / (self.rho() * self.material.cp_liquid.get::<joule_per_kilogram_kelvin>()), 1.0)
        }
    }

    fn conductivity(&self, f: f64) -> f64 {
        f * self.material.k_liquid.get::<watt_per_meter_kelvin>()
            + (1.0 - f) * self.material.k_solid.get::<watt_per_meter_kelvin>()
    }

    /// Advance by `dt` with `face` on the heated face.
    pub fn step(&mut self, dt: Time, face: HeatedFace) -> Result<(), TampinesError> {
        let dt = dt.get::<second>();
        if !(dt > 0.0) {
            return Err(TampinesError::InvalidInput(format!("time step must be > 0, got {dt}")));
        }
        let n = self.h.len();
        let dx = self.thickness / n as f64;
        let rho = self.rho();
        let cmin = self
            .material
            .cp_solid
            .get::<joule_per_kilogram_kelvin>()
            .min(self.material.cp_liquid.get::<joule_per_kilogram_kelvin>());
        let kmax = self
            .material
            .k_solid
            .get::<watt_per_meter_kelvin>()
            .max(self.material.k_liquid.get::<watt_per_meter_kelvin>());
        let alpha_max = kmax / (rho * cmin);
        let subs = ((dt * alpha_max / (0.4 * dx * dx)).ceil() as usize).max(1);
        let ds = dt / subs as f64;
        let mut flux = vec![0.0; n + 1]; // flux[i] into cell i through its left face [W/m²]
        for _ in 0..subs {
            let st: Vec<(f64, f64)> = self.h.iter().map(|&h| self.state_of(h)).collect();
            let k: Vec<f64> = st.iter().map(|&(_, f)| self.conductivity(f)).collect();
            flux[0] = match face {
                HeatedFace::Temperature(tw) => k[0] * (tw.get::<kelvin>() - st[0].0) / (0.5 * dx),
                HeatedFace::HeatFlux(q) => q.get::<watt_per_square_meter>(),
            };
            for i in 1..n {
                let kf = 2.0 * k[i - 1] * k[i] / (k[i - 1] + k[i]);
                flux[i] = kf * (st[i - 1].0 - st[i].0) / dx;
            }
            flux[n] = 0.0; // insulated far face
            for i in 0..n {
                self.h[i] += ds * (flux[i] - flux[i + 1]) / dx;
            }
            self.heat_in += flux[0] * self.area * ds;
        }
        self.time += dt;
        Ok(())
    }

    /// Melted thickness `s = Σ f_i Δx`.
    pub fn melt_front(&self) -> Length {
        let dx = self.thickness / self.h.len() as f64;
        Length::new::<meter>(self.h.iter().map(|&h| self.state_of(h).1).sum::<f64>() * dx)
    }

    /// Whether the plug has thawed through: the melt front has reached `L`
    /// (every cell fully liquid).
    pub fn is_open(&self) -> bool {
        self.h.iter().all(|&h| h >= self.rho_lf())
    }

    /// Heat that has entered through the heated face since construction.
    pub fn heat_in(&self) -> Energy {
        Energy::new::<joule>(self.heat_in)
    }

    /// Enthalpy stored in the plug relative to an all-solid plug at `T_m`.
    pub fn stored_enthalpy(&self) -> Energy {
        let dx = self.thickness / self.h.len() as f64;
        Energy::new::<joule>(self.h.iter().sum::<f64>() * dx * self.area)
    }

    /// Time since construction.
    pub fn time(&self) -> Time {
        Time::new::<second>(self.time)
    }
}

/// Neumann's similarity root for one-phase melting: `λ` with
/// `λ e^{λ²} erf(λ) = St / √π`, `St = c_l (T_w − T_m) / L_f`. Bisection to
/// round-off. The melt front is then `s(t) = 2 λ √(α_l t)`.
pub fn neumann_lambda(stefan_number: f64) -> f64 {
    let target = stefan_number / core::f64::consts::PI.sqrt();
    let g = |l: f64| l * (l * l).r_exp() * petir::erf(l) - target;
    let (mut lo, mut hi) = (0.0_f64, 5.0_f64);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if g(mid) > 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    0.5 * (lo + hi)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A generic fluoride-salt-like plug (round numbers, not a specific
    /// salt): ρ 2000 kg/m³, c 2000 J/kg/K, k 1 W/m/K, L_f 2e5 J/kg,
    /// T_m 733 K.
    fn salt() -> PlugMaterial {
        PlugMaterial {
            density: MassDensity::new::<kilogram_per_cubic_meter>(2000.0),
            cp_solid: SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(2000.0),
            cp_liquid: SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(2000.0),
            k_solid: ThermalConductivity::new::<watt_per_meter_kelvin>(1.0),
            k_liquid: ThermalConductivity::new::<watt_per_meter_kelvin>(1.0),
            latent_heat: AvailableEnergy::new::<joule_per_kilogram>(2.0e5),
            melting_point: ThermodynamicTemperature::new::<kelvin>(733.0),
        }
    }

    const L: f64 = 0.02;
    const DT_WALL: f64 = 50.0;

    fn plug(cells: usize) -> FreezePlug {
        FreezePlug::new(
            salt(),
            Length::new::<meter>(L),
            Area::new::<square_meter>(1.0),
            cells,
            ThermodynamicTemperature::new::<kelvin>(733.0),
        )
        .unwrap()
    }

    fn exact_front(t: f64) -> f64 {
        let m = salt();
        let st = 2000.0 * DT_WALL / 2.0e5;
        let alpha = m.k_liquid.get::<watt_per_meter_kelvin>()
            / (m.density.get::<kilogram_per_cubic_meter>() * m.cp_liquid.get::<joule_per_kilogram_kelvin>());
        2.0 * neumann_lambda(st) * (alpha * t).sqrt()
    }

    /// F1 and F2: the one-phase Stefan problem (slab at T_m, wall held at
    /// T_m + 50 K) against Neumann's front, and the plug's energy books.
    /// Generic salt (see [`salt`]), L = 2 cm, St = 0.5, t = 600 s, 1 s steps.
    ///
    /// **Results (2026-11-09, release):** front against Neumann's
    /// 1.138488e-2 m: −0.154 % (25 cells), −0.044 % (50), **+0.026 % (100)**,
    /// +0.006 % (200); criterion 2 % at 100 cells, error falling with
    /// refinement (predicted ~1 %, first order; the measured error is
    /// smaller and not monotone in sign). F2 energy books 2e-16 to 1e-14
    /// relative (criterion 1e-9).
    #[test]
    fn f1_f2_the_melt_front_follows_neumann_and_energy_is_conserved() {
        let face = HeatedFace::Temperature(ThermodynamicTemperature::new::<kelvin>(733.0 + DT_WALL));
        let t_end = 600.0;
        let exact = exact_front(t_end);
        let mut errs = Vec::new();
        for cells in [25, 50, 100, 200] {
            let mut p = plug(cells);
            for _ in 0..600 {
                p.step(Time::new::<second>(1.0), face).unwrap();
            }
            let s = p.melt_front().get::<meter>();
            let books = (p.stored_enthalpy().get::<joule>() / p.heat_in().get::<joule>() - 1.0).abs();
            println!("F1: {cells:4} cells: front {s:.6e} m vs Neumann {exact:.6e} m ({:+.3} %); F2 energy books {books:.2e}", 100.0 * (s / exact - 1.0));
            assert!(books <= 1.0e-9, "energy books {books}");
            errs.push((s / exact - 1.0).abs());
        }
        assert!(errs[2] <= 0.02, "100 cells: {:.3} %", 100.0 * errs[2]);
        assert!(errs[3] < errs[0], "the front error must fall with refinement");
    }

    /// F3: the plug opens when the front reaches L; against Neumann's
    /// t_open = (L / 2λ)² / α_l.
    ///
    /// **Result (2026-11-09, release, 100 cells, 1 s steps):** opened at
    /// 1852 s against Neumann's 1851.6 s, **+0.020 %** (criterion 2 %; the
    /// 1 s step quantises the opening time by up to 0.05 %).
    #[test]
    fn f3_the_plug_opens_when_the_front_reaches_its_thickness() {
        let st = 2000.0 * DT_WALL / 2.0e5;
        let alpha = 1.0 / (2000.0 * 2000.0);
        let t_open_exact = (L / (2.0 * neumann_lambda(st))).powi(2) / alpha;
        let face = HeatedFace::Temperature(ThermodynamicTemperature::new::<kelvin>(733.0 + DT_WALL));
        let mut p = plug(100);
        let mut t_open = None;
        for k in 1..=20_000 {
            p.step(Time::new::<second>(1.0), face).unwrap();
            if p.is_open() {
                t_open = Some(k as f64);
                break;
            }
        }
        let t = t_open.expect("the plug must open");
        println!("F3: opened at {t:.0} s vs Neumann {t_open_exact:.1} s ({:+.3} %)", 100.0 * (t / t_open_exact - 1.0));
        assert!((t / t_open_exact - 1.0).abs() <= 0.02);
    }

    /// A heat-flux face, and cooling: a negative flux freezes, the plug stays
    /// shut, and the energy books still close.
    #[test]
    fn a_cooled_plug_stays_shut_and_its_books_close() {
        let mut p = FreezePlug::new(
            salt(),
            Length::new::<meter>(L),
            Area::new::<square_meter>(0.5),
            50,
            ThermodynamicTemperature::new::<kelvin>(720.0),
        )
        .unwrap();
        let e0 = p.stored_enthalpy().get::<joule>();
        for _ in 0..300 {
            p.step(Time::new::<second>(1.0), HeatedFace::HeatFlux(HeatFluxDensity::new::<watt_per_square_meter>(-500.0))).unwrap();
        }
        assert!(!p.is_open());
        assert_eq!(p.melt_front().get::<meter>(), 0.0);
        let de = p.stored_enthalpy().get::<joule>() - e0;
        let qin = p.heat_in().get::<joule>();
        assert!(qin < 0.0, "heat must have left the plug");
        assert!((de / qin - 1.0).abs() <= 1e-9, "books {de} vs {qin}");
    }
}
