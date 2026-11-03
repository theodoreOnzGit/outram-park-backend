// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! The FPIC full-power steady state (gh:#1150): the gh:#1149 solid field
//! coupled to the [`super::flow`] gas network by outer iteration, the
//! pebble interior ([`super::pebble`]) and the 22 thermocouples.
//!
//! **Outer iteration (block Gauss-Seidel).** Each pass: (1) solve the gas
//! flow and energy for the current solid temperatures; (2) hand each cell's
//! `h a_v` and gas temperature to the solid field as an implicit exchange
//! (`SolidConductionField::set_cell_exchange`) and solve the solid steady
//! state (its own Picard loop for `lambda(T)` and radiation). Stop when no
//! solid or gas temperature moves more than [`FpicOptions::tolerance_k`].
//! The coupling is causal along the gas path (a Volterra-type map), so the
//! passes converge without relaxation; the history is reported.
//!
//! **Thermocouple instrument (fixed before the first run, see the V&V
//! README).** At a thermocouple the model value is the mean over the cells
//! touching the point (one cell inside a cell, two on a face, four on a
//! corner). For a cell of an open gas volume (plenum or cavity, void
//! >= 0.99) the **gas** temperature is used; for every other cell the
//! **solid** temperature. Both raw values are kept.

use uom::si::f64::{Power, ThermodynamicTemperature};
use uom::si::length::centimeter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::bar;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::volumetric_power_density::watt_per_cubic_meter;

use super::flow::{solve_gas, BypassSplit, FlowKind, GasNetwork, GasState, OperatingPoint, HELIUM_CP};
use super::geometry::idx;
use super::pebble::{pebble_profile, PebbleMatrix, PebbleProfile};
use super::{Htr10RzCase, Htr10RzError, Htr10RzModel, Htr10RzOptions, Htr10RzResult};
use crate::genfoam::thermal_hydraulics::solid_conduction::VolumetricHeatTransferCoefficient;

/// The embedded TECDOC-1694 Tab 2.7 / 2.14 / 7.1 thermocouple table, a copy
/// of `docs/reactor-scoping/htr10-th-data/fpic-thermocouples-tab7-1.csv`
/// (a unit test fails if they drift).
const TC_CSV: &str = include_str!("data/fpic-thermocouples-tab7-1.csv");

/// Participant column keys of Tab 7.1, in CSV order.
pub const PARTICIPANTS: [&str; 11] = [
    "CN", "FR", "JP", "KR", "NL", "ZA-TINTE", "ZA-VSOP", "TR", "UK-PHOENICS-1", "UK-PHOENICS-2", "UK-WIMSTER",
];

/// One measured FPIC thermocouple with the benchmark results.
#[derive(Debug, Clone, PartialEq)]
pub struct Thermocouple {
    /// Number 1-22.
    pub number: usize,
    /// Location label.
    pub location: String,
    /// Position, cm (TECDOC `z`, downward).
    pub r_cm: f64,
    /// Position, cm.
    pub z_cm: f64,
    /// Measured, deg C.
    pub measured_c: f64,
    /// INET (THERMIX-KONVEK) result, deg C (Tab 2.14).
    pub inet_c: f64,
    /// Each participant's result (Tab 7.1), deg C, `None` where not
    /// submitted, in [`PARTICIPANTS`] order.
    pub participants: Vec<Option<f64>>,
}

impl Thermocouple {
    /// Lowest and highest participant result, deg C.
    #[must_use]
    pub fn participant_range(&self) -> (f64, f64) {
        let v: Vec<f64> = self.participants.iter().flatten().copied().collect();
        (
            v.iter().copied().fold(f64::INFINITY, f64::min),
            v.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        )
    }
}

/// The 22 thermocouples.
///
/// # Panics
/// Never for the embedded file (its parse is pinned by a unit test).
#[must_use]
pub fn thermocouples() -> Vec<Thermocouple> {
    TC_CSV
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("no,") && !l.trim().is_empty())
        .map(|l| {
            let v: Vec<&str> = l.split(',').collect();
            let f = |s: &str| s.trim().parse::<f64>().expect("numeric thermocouple column");
            Thermocouple {
                number: v[0].trim().parse().expect("thermocouple number"),
                location: v[1].to_string(),
                r_cm: f(v[2]),
                z_cm: f(v[3]),
                measured_c: f(v[4]),
                inet_c: f(v[5]),
                participants: v[6..17]
                    .iter()
                    .map(|s| if s.trim().is_empty() { None } else { Some(f(s)) })
                    .collect(),
            }
        })
        .collect()
}

/// Which temperature a thermocouple reading was formed from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instrument {
    /// Solid cell temperature(s).
    Solid,
    /// Gas temperature(s) of an open volume.
    Gas,
    /// Both (the point lies on a face between a solid and an open volume).
    Mixed,
}

/// The model at one thermocouple.
#[derive(Debug, Clone, PartialEq)]
pub struct ThermocoupleReading {
    /// The thermocouple.
    pub tc: Thermocouple,
    /// Model value by the instrument rule, deg C.
    pub model_c: f64,
    /// Which temperature it is.
    pub instrument: Instrument,
    /// Solid temperature at the point (cell mean), deg C.
    pub solid_c: f64,
    /// Gas temperature at the point (cell mean over gas cells; `NaN` where
    /// no gas flows), deg C.
    pub gas_c: f64,
    /// Model regions of the cells touching the point.
    pub regions: Vec<&'static str>,
}

impl ThermocoupleReading {
    /// `model - measured`, K.
    #[must_use]
    pub fn residual(&self) -> f64 {
        self.model_c - self.tc.measured_c
    }

    /// Whether the model lies inside the participants' range.
    #[must_use]
    pub fn inside_participant_range(&self) -> bool {
        let (lo, hi) = self.tc.participant_range();
        self.model_c >= lo && self.model_c <= hi
    }
}

/// Modelling choices of the FPIC case, each defaulting to the base case.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FpicOptions {
    /// The solid model (mesh, bed closure, brick, ...). Its helium pressure
    /// is overwritten by the operating point's.
    pub solid: Htr10RzOptions,
    /// Plant state. Default: the benchmark specification (D4).
    pub operating_point: OperatingPoint,
    /// Bypass split. Default: [T1]'s assumed 87/1/2/10 (D5).
    pub split: BypassSplit,
    /// Fuel-zone conductivity rule of the pebble sub-model.
    pub pebble: PebbleMatrix,
    /// Outer-iteration tolerance on any temperature change, K.
    pub tolerance_k: f64,
    /// Outer-iteration cap.
    pub max_outer: usize,
    /// Under-relaxation of the gas temperature between outer passes
    /// (`T <- w T_new + (1 - w) T_old`), in `(0, 1]`.
    pub gas_relaxation: f64,
}

impl Default for FpicOptions {
    fn default() -> Self {
        Self {
            solid: Htr10RzOptions::default(),
            operating_point: OperatingPoint::fpic_specification(),
            split: BypassSplit::default(),
            pebble: PebbleMatrix::default(),
            tolerance_k: 1.0e-4,
            max_outer: 400,
            gas_relaxation: 0.5,
        }
    }
}

/// The outcome of a coupled FPIC solve.
#[derive(Debug, Clone)]
pub struct FpicResult {
    /// The solid report of the last pass.
    pub solid: Htr10RzResult,
    /// Outer passes taken.
    pub outer_iterations: usize,
    /// Largest temperature change of each pass, K.
    pub outer_history: Vec<f64>,
    /// Mixed core outlet (hot plenum), K.
    pub outlet_temperature: ThermodynamicTemperature,
    /// Bed outlet (cone floor), K.
    pub bed_outlet_temperature: ThermodynamicTemperature,
    /// RCCS side wall plus both heads.
    pub heat_to_rccs_and_heads: Power,
    /// Heat taken up by the gas (solid side).
    pub heat_to_gas: Power,
    /// `W c_p (T_out - T_in)`.
    pub enthalpy_rise: Power,
    /// `|P - Q_RCCS+heads - W c_p dT| / P`: the plant energy balance.
    pub plant_imbalance: f64,
    /// Bed-stream pressure drop to the outlet, Pa.
    pub core_pressure_drop: f64,
    /// Hottest pebble by the benchmark fuel-centre definition, and its cell
    /// centre `(r, z)` cm.
    pub max_fuel_centre: (PebbleProfile, (f64, f64)),
    /// Hottest pebble surface, K, and where.
    pub max_fuel_surface: (f64, (f64, f64)),
    /// Hottest side-reflector surface (inner face column of region 14), K.
    pub max_side_reflector_surface: f64,
    /// Hottest bottom-reflector cell (model regions 8, 9, 31), K.
    pub max_bottom_reflector: f64,
    /// The 22 thermocouples.
    pub thermocouples: Vec<ThermocoupleReading>,
    /// Gas temperatures of the tap nodes: bottom cavity, leak take-off, top
    /// plenum, K.
    pub tap_temperatures: [f64; 3],
}

/// The coupled model.
#[derive(Debug, Clone)]
pub struct Htr10FpicModel {
    solid: Htr10RzModel,
    network: GasNetwork,
    options: FpicOptions,
    gas: Option<GasState>,
}

impl Htr10FpicModel {
    /// Build the solid model (power case, FPIC shape) and the gas network.
    ///
    /// # Errors
    /// As [`Htr10RzModel::build`] and [`GasNetwork::build`].
    pub fn build(options: FpicOptions) -> Result<Self, Htr10RzError> {
        let mut solid_opts = options.solid;
        solid_opts.materials.helium_pressure_bar = options.operating_point.pressure.get::<bar>();
        let solid = Htr10RzModel::build(
            solid_opts,
            Htr10RzCase::DecayPower {
                power: options.operating_point.power,
            },
        )?;
        let network = GasNetwork::build(solid.field().mesh(), options.split)?;
        Ok(Self {
            solid,
            network,
            options,
            gas: None,
        })
    }

    /// The solid model.
    #[must_use]
    pub fn solid(&self) -> &Htr10RzModel {
        &self.solid
    }

    /// The gas network.
    #[must_use]
    pub fn network(&self) -> &GasNetwork {
        &self.network
    }

    /// The gas state of the last pass.
    #[must_use]
    pub fn gas(&self) -> Option<&GasState> {
        self.gas.as_ref()
    }

    fn solid_temperatures(&self) -> Vec<f64> {
        let f = self.solid.field();
        (0..f.mesh().n_cells()).map(|c| f.temperature(c).get::<kelvin>()).collect()
    }

    /// Run the outer iteration to convergence and report.
    ///
    /// # Errors
    /// [`Htr10RzError::Solve`] if a sub-solve fails or the outer iteration
    /// does not converge within [`FpicOptions::max_outer`].
    pub fn solve(&mut self) -> Result<FpicResult, Htr10RzError> {
        let op = self.options.operating_point;
        let n = self.solid.field().mesh().n_cells();
        let mut history = Vec::new();
        let mut t_s = self.solid_temperatures();
        for it in 1..=self.options.max_outer {
            let mut g = solve_gas(&self.network, &op, &t_s, self.gas.as_ref())?;
            if let Some(old) = &self.gas {
                let w = self.options.gas_relaxation;
                for (a, b) in g.temperature.iter_mut().zip(&old.temperature) {
                    if a.is_finite() && b.is_finite() {
                        *a = w * *a + (1.0 - w) * b;
                    }
                }
            }
            let ex: Vec<_> = (0..n)
                .map(|c| {
                    let h = g.exchange[c];
                    let t = if h > 0.0 { g.temperature[c] } else { 300.0 };
                    (
                        VolumetricHeatTransferCoefficient {
                            dimension: std::marker::PhantomData,
                            units: std::marker::PhantomData,
                            value: h,
                        },
                        ThermodynamicTemperature::new::<kelvin>(t),
                    )
                })
                .collect();
            self.solid
                .field_mut()
                .set_cell_exchange(&ex)
                .map_err(|e| Htr10RzError::Solve(e.to_string()))?;
            let rep = self.solid.solve_steady()?;
            let t_new = self.solid_temperatures();
            let mut change: f64 = t_new.iter().zip(&t_s).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
            if let Some(old) = &self.gas {
                for (a, b) in g.temperature.iter().zip(&old.temperature) {
                    if a.is_finite() && b.is_finite() {
                        change = change.max((a - b).abs());
                    }
                }
            } else {
                change = f64::INFINITY;
            }
            history.push(change);
            t_s = t_new;
            self.gas = Some(g);
            if change < self.options.tolerance_k {
                // One more gas pass on the final solid field, so gas and
                // solid are mutually consistent in the report.
                let g = solve_gas(&self.network, &op, &t_s, self.gas.as_ref())?;
                self.gas = Some(g);
                return self.report(rep, it, history);
            }
        }
        Err(Htr10RzError::Solve(format!(
            "FPIC outer iteration did not converge in {} passes (last change {:e} K)",
            self.options.max_outer,
            history.last().copied().unwrap_or(f64::NAN)
        )))
    }

    /// The model at a thermocouple, by the instrument rule of the module
    /// docs.
    #[must_use]
    pub fn reading(&self, tc: &Thermocouple) -> ThermocoupleReading {
        reading_at(&self.solid, &self.network, self.gas.as_ref(), tc)
    }
}

/// The model at a thermocouple by the instrument rule of the module docs,
/// for any solid model, network and gas state on one mesh (shared by the
/// FPIC case and the gh:#1151 part B transient core).
#[must_use]
pub fn reading_at(
    solid_model: &Htr10RzModel,
    network: &GasNetwork,
    gas: Option<&GasState>,
    tc: &Thermocouple,
) -> ThermocoupleReading {
    {
        let f = solid_model.field();
        let mesh = f.mesh();
        let d = 1e-3;
        let mut cells = Vec::new();
        for (dr, dz) in [(-d, -d), (d, -d), (-d, d), (d, d)] {
            if let Some(c) = mesh.cell_at(
                uom::si::f64::Length::new::<centimeter>(tc.r_cm + dr),
                uom::si::f64::Length::new::<centimeter>(tc.z_cm + dz),
            ) {
                if !cells.contains(&c) {
                    cells.push(c);
                }
            }
        }
        let (mut model, mut solid, mut gsum, mut ng) = (0.0, 0.0, 0.0, 0usize);
        let (mut n_gas_used, mut n_solid_used) = (0, 0);
        let mut regions = Vec::new();
        for &c in &cells {
            let ts = f.temperature(c).get::<kelvin>();
            solid += ts;
            regions.push(solid_model.regions()[mesh.cell_region(c)].name);
            let tg = gas.map_or(f64::NAN, |g| g.temperature[c]);
            if tg.is_finite() {
                gsum += tg;
                ng += 1;
            }
            let open = matches!(
                network.cell_flow(c),
                Some((fr, _)) if matches!(fr.kind, FlowKind::OpenVolume { .. }) && fr.void_fraction >= 0.99
            );
            if open && tg.is_finite() {
                model += tg;
                n_gas_used += 1;
            } else {
                model += ts;
                n_solid_used += 1;
            }
        }
        let k = cells.len() as f64;
        ThermocoupleReading {
            tc: tc.clone(),
            model_c: model / k - 273.15,
            instrument: match (n_gas_used, n_solid_used) {
                (0, _) => Instrument::Solid,
                (_, 0) => Instrument::Gas,
                _ => Instrument::Mixed,
            },
            solid_c: solid / k - 273.15,
            gas_c: if ng > 0 { gsum / ng as f64 - 273.15 } else { f64::NAN },
            regions,
        }
    }
}

impl Htr10FpicModel {
    fn report(&self, solid: Htr10RzResult, iterations: usize, history: Vec<f64>) -> Result<FpicResult, Htr10RzError> {
        let op = self.options.operating_point;
        let g = self.gas.as_ref().expect("a gas pass ran");
        let f = self.solid.field();
        let mesh = f.mesh();
        let n = mesh.n_cells();
        let p = op.power.get::<watt>();
        let q_out = (solid.rccs_side + solid.top_head + solid.bottom_head).get::<watt>();
        let mut best: Option<(PebbleProfile, (f64, f64))> = None;
        let mut surf = (f64::NEG_INFINITY, (0.0, 0.0));
        let mut side: f64 = f64::NEG_INFINITY;
        let mut bottom: f64 = f64::NEG_INFINITY;
        for c in 0..n {
            let reg = mesh.cell_region(c);
            let t = f.temperature(c).get::<kelvin>();
            let (r, z) = mesh.cell_centre(c);
            let at = (r.get::<centimeter>(), z.get::<centimeter>());
            if reg == idx::BED {
                let q = self.solid.cell_power_density(c).get::<watt_per_cubic_meter>();
                if q > 0.0 {
                    let prof = pebble_profile(t, q, self.options.pebble)?;
                    if best.as_ref().is_none_or(|b| prof.benchmark_centre > b.0.benchmark_centre) {
                        best = Some((prof, at));
                    }
                }
                if t > surf.0 {
                    surf = (t, at);
                }
            }
            if reg == idx::SIDE_INNER_GRAPHITE {
                let i = c % mesh.r_edges().n_cells();
                if (mesh.r_edges().edge(i).get::<centimeter>() - 90.0).abs() < 1e-9 {
                    side = side.max(t);
                }
            }
            if matches!(reg, idx::UPPER_BOTTOM_REFL | idx::LOWER_BOTTOM_REFL | idx::BOTTOM_REFL_SOLID) {
                bottom = bottom.max(t);
            }
        }
        let rise = g.enthalpy_rise;
        Ok(FpicResult {
            outer_iterations: iterations,
            outer_history: history,
            outlet_temperature: ThermodynamicTemperature::new::<kelvin>(g.outlet_temperature),
            bed_outlet_temperature: ThermodynamicTemperature::new::<kelvin>(g.bed_outlet_temperature),
            heat_to_rccs_and_heads: Power::new::<watt>(q_out),
            heat_to_gas: Power::new::<watt>(g.heat_to_gas),
            enthalpy_rise: Power::new::<watt>(rise),
            plant_imbalance: (p - q_out - rise).abs() / p,
            core_pressure_drop: g.core_pressure_drop,
            max_fuel_centre: best.ok_or_else(|| Htr10RzError::Solve("no heated bed cell".into()))?,
            max_fuel_surface: surf,
            max_side_reflector_surface: side,
            max_bottom_reflector: bottom,
            thermocouples: thermocouples().iter().map(|tc| self.reading(tc)).collect(),
            tap_temperatures: g.tap_temperatures,
            solid,
        })
    }
}

/// `W c_p`, W/K, of an operating point (reporting helper).
#[must_use]
pub fn capacity_rate(op: &OperatingPoint) -> f64 {
    op.mass_flow.get::<kilogram_per_second>() * HELIUM_CP
}
