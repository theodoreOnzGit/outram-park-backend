// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! A **lumped stand-in** for the HTR-10 core, to exercise the primary loop
//! and the kinetics until the R-Z core of gh:#1150 can be coupled (part B).
//!
//! **This is not the core model.** It has no radial resolution in the bed,
//! no in-vessel natural circulation, no top or bottom reflector, and one
//! node for the whole side reflector. Chen et al. (2009) Sec 5 name the
//! in-vessel natural circulation as an effective heat-transport path after
//! the baffle closes, and the top reflector heats by ~170 K in the LOFC test
//! ([C09] Sec 5): both are absent here, so this stand-in **under-states the
//! heat removed from the bed** after the trip. Every parameter below is
//! derived from cited geometry and material data; nothing is fitted to the
//! LOFC trace.
//!
//! # Nodes (19)
//!
//! ```text
//!  per heated axial zone k = 0..4 (z bands 0-36-72-108-144-187 cm, TECDOC z down):
//!     fuel zone  f_k  --G_fs-->  fuel-pebble shell  s_k  --G_sd--  dummy pebbles  d_k
//!                                     |  film hA (KTA 3102.2)           | film hA
//!                                     +----------- bed helium (zone k) -+
//!     s_k, d_k --(bed 8 pi k_eff h in series with inner half of reflector)--> R
//!     s_k <-> s_k+1, d_k <-> d_k+1 axial bed conduction (k_eff pi R^2 / dz)
//!  cone zone 5 (z 187-217.5, dummies only, unheated): d_5
//!  R (side reflector graphite + carbon brick) -- outer half graphite + brick + He gap --> B (barrel)
//!  B -- He annulus (gas + radiation, eps 0.85) --> V (RPV) -- radiation --> RCCS 50 degC
//!  forced flow: downcomer helium washes B and V; riser helium washes R
//! ```
//!
//! # Gas path (forced direction)
//!
//! cold nozzle -> downcomer (B, V walls) -> bottom: discharge-tube bypass
//! leaves -> 20 risers (R wall) -> top plenum: control-rod and leakage
//! bypasses leave -> bed zones 0..4 -> cone -> hot plenum (all streams mix)
//! -> hot nozzle. Reversed flow runs the same path backwards. Each wetted
//! segment is the exact isothermal-wall effectiveness form
//! `T_out = T_w + (T_in - T_w) exp(-hA / (m c_p))` with several walls
//! combined by their `hA`; the gas is quasi-steady and eliminated
//! **linearly**, so the gas coupling is inside the one backward-Euler
//! solve of all 19 nodes (no lagged convective source).
//!
//! # Data (all cited; Tab = IAEA-TECDOC-1694 table, sheet = `htr10-th-benchmark.md`)
//!
//! | Item | Value | Source |
//! |---|---|---|
//! | bed radius; pebble / fuel-zone diameter | 0.90 m; 6.0 / 5.0 cm | Tab 2.3, 2.2 |
//! | bed porosity | 0.39 | Tab 2.5 |
//! | pebbles at the test | 13,651 fuel + 16,408 dummy, 10,298 of them mixed 57:43 | [J09] Tab 2; sheet Sec 6 (Derived) |
//! | cone dummies | 2,486, from the stepped cone volume x 0.61 / V_pebble | Tab 2.3, Sec 3.2 cone (Derived) |
//! | zone power fractions | Tab 2.56 integrated over each z band | Tab 2.56 |
//! | graphite, carbon brick `rho c_p`; bed, fuel, side-reflector, brick `k` | Tab 2.9, 2.11, 2.12 via `outram-foam-appbuilder-lib::htr10_rz::RegionMaterial` | Tab 2.9-2.12 |
//! | side reflector r 90-167.5 cm, solid fractions by sub-annulus | 1.0 / 0.793 / 1.0 / 0.862 / 1.0 | Tab 2.4, Fig 2.5 |
//! | side reflector node height | z -65 to 325 cm = 3.90 m (top of CR channels/risers to hot-plenum bottom) | Fig 2.6 (Uncertain) |
//! | carbon brick r 167.5-190 cm | — | Tab 2.3 |
//! | barrel r 191-194 cm, 7.37 m; RPV r 210-218 cm, 11.155 m | — | Tab 2.3 (N5: Fig 2.5 places the barrel at 194-197) |
//! | gap / annulus emissivities 0.82 / 0.85; RPV-cavity 0.85 | — | Tab 2.13 |
//! | RCCS panels at r 2.854 m, 50 degC | — | Fig 2.5 (Uncertain), Sec 2.1.1.4 |
//! | risers 20 x 0.08 m, z -65 to 446 (5.11 m) | — | Tab 2.3, Fig 2.6 |
//! | downcomer r 194-210 cm over z 295-507 (2.12 m) | — | Fig 2.6 regions 16-17 |
//! | in-RPV form losses: side-reflector pass 0.7 kPa at 3.846 kg/s; plena 6.1 kPa at 4.32 kg/s | — | [G02] Tab 1 via `htr10-plant-data.md` Sec 7.5 |
//! | bed friction | KTA 3102.3 per zone (`gas_phase::KtaBed`) | Tab 2.10 |
//! | helium | Tab 2.8 fits, bar-fixed (`htgr_closures::KtaHelium`) | Tab 2.8, sheet Sec 4.1 |
//!
//! # Named assumptions (each stated where it is used)
//!
//! A1 fuel and dummy pebbles are mixed uniformly over 0-187 cm; A2 all
//! power is deposited in the fuel zones (no gamma heating of graphite);
//! A3 the fuel-to-shell resistance is the uniformly heated sphere plus the
//! unheated shell, with the Tab 2.9 fuel-element `k` at a fast dose of zero
//! (the as-run dose is Unknown); A4 shell-to-dummy exchange in a zone is a
//! sphere in the effective bed medium, `2 pi k_eff d` per fuel pebble times
//! the dummy fraction; A5 the control-rod, leakage and discharge-tube
//! bypass streams pick up no heat; A6 the reflector node lumps the side
//! reflector graphite and the carbon brick; top and bottom reflectors are
//! absent; A7 no air convection in the cavity, radiation only; A8 riser and
//! downcomer film coefficients are Dittus-Boelter (heating) above Re = 1e4,
//! the laminar 4.36 below 2300, linear in Re between.
//!
//! > Research, education and V&V only. AI draft, unreviewed.

use outram_foam_appbuilder_lib::htgr_closures::{
    kta::kta_3102_2_heat_transfer_coefficient, KtaHelium, StructuralMaterial,
};
use outram_foam_appbuilder_lib::htr10_rz::{MaterialOptions, RegionMaterial};
use petir::mathf::RealMath;
// The same Crout LU (Numerical Recipes `ludcmp`) as `teh-o-prke`'s inlined
// copy, which is a copy of this one: identical arithmetic.
use outram_foam_basic_lib::matrix::SquareMatrix;
use uom::si::areal_number_density::per_square_centimeter;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{
    Area, ArealNumberDensity, DynamicViscosity, Length, MassDensity, MassRate, Power, Pressure,
    Ratio, ThermodynamicTemperature, Time,
};
use uom::si::area::square_meter;
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::{bar, pascal};
use uom::si::ratio::ratio;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use super::core_model::{CoreBoundary, CoreResponse};
use super::data::{lofc_power_map, map_power_in_band_w};
use super::core_model::FeedbackTemperatures;
use crate::gas_phase::KtaBed;
use crate::TampinesError;

const PI: f64 = core::f64::consts::PI;
const SIGMA_SB: f64 = 5.670_374_419e-8;
const G: f64 = 9.806_65;
/// Tab 2.8 helium `c_p`, J/(kg K).
const CP_HE: f64 = 5195.0;

const R_BED: f64 = 0.90;
const D_PEB: f64 = 0.06;
const R_FUEL_ZONE: f64 = 0.025;
const POROSITY: f64 = 0.39;
/// Heated-zone z edges, cm (TECDOC z, downward from the bed top).
const ZONE_EDGES_CM: [f64; 6] = [0.0, 36.0, 72.0, 108.0, 144.0, 187.0];
/// Cone: z 187-217.5 cm.
const CONE_HEIGHT: f64 = 0.305;
/// Side-reflector sub-annuli (r_in, r_out, solid fraction), Tab 2.4 / Fig 2.5.
const SIDE_REFLECTOR: [(f64, f64, f64); 5] = [
    (0.90, 0.956, 1.0),
    (0.956, 1.086, 0.793),
    (1.086, 1.406, 1.0),
    (1.406, 1.486, 0.862),
    (1.486, 1.675, 1.0),
];
const R_REFL_OUT: f64 = 1.675;
const R_REFL_MID: f64 = 0.5 * (R_BED + R_REFL_OUT);
const H_REFL: f64 = 3.90;
const R_BRICK_OUT: f64 = 1.90;
const R_BARREL_IN: f64 = 1.91;
const R_BARREL_OUT: f64 = 1.94;
const H_BARREL: f64 = 7.37;
const R_RPV_IN: f64 = 2.10;
const R_RPV_OUT: f64 = 2.18;
const H_RPV: f64 = 11.155;
const R_RCCS: f64 = 2.854;
const EPS_GAP: f64 = 0.82;
const EPS_ANNULUS: f64 = 0.85;
const EPS_CAVITY: f64 = 0.85;
const T_RCCS: f64 = 323.15;
const N_RISERS: f64 = 20.0;
const D_RISER: f64 = 0.08;
const L_RISER: f64 = 5.11;
const H_DOWNCOMER: f64 = 2.12;
/// Elevation changes (m, up positive) of the in-RPV path, forced direction:
/// downcomer z 310 -> 507, bottom turn 507 -> 446, risers 446 -> -65, top
/// plenum -65 -> 0, bed 0 -> 187, cone 187 -> 217.5, bottom reflector and
/// hot plenum 217.5 -> 310. Closes to zero.
const DZ_DOWNCOMER: f64 = -1.97;
const DZ_BOTTOM_TURN: f64 = 0.61;
const DZ_RISERS: f64 = 5.11;
const DZ_TOP_PLENUM: f64 = -0.65;
const DZ_BELOW_CONE: f64 = -0.925;
/// [G02] Tab 1 in-RPV form losses: (kPa, kg/s, reference T K at 3.0 MPa).
const DP_SIDE_REFLECTOR_PASS: (f64, f64, f64) = (0.7, 3.846, 523.15);
const DP_MIXTURE_PLENUMS: (f64, f64, f64) = (6.1, 4.32, 973.15);

const N_ZONES: usize = 5;
const N_NODES: usize = 3 * N_ZONES + 1 + 3;
const IDX_CONE: usize = 3 * N_ZONES;
const IDX_R: usize = IDX_CONE + 1;
const IDX_B: usize = IDX_R + 1;
const IDX_V: usize = IDX_B + 1;

fn f_idx(k: usize) -> usize {
    3 * k
}
fn s_idx(k: usize) -> usize {
    3 * k + 1
}
fn d_idx(k: usize) -> usize {
    if k == N_ZONES {
        IDX_CONE
    } else {
        3 * k + 2
    }
}

fn prop_err(e: impl std::fmt::Display) -> TampinesError {
    TampinesError::Numerical(format!("HTR-10 lumped core property: {e}"))
}

/// How the loop flow divides inside the RPV (fractions of the loop flow).
///
/// Unmeasured ([T1] Sec 7.1.2): a **sensitivity input, never tuned** (D5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BypassSplit {
    /// Through the pebble bed.
    pub bed: f64,
    /// Through the fuel discharge tube (leaves at the bottom, unheated).
    pub discharge_tube: f64,
    /// Down the control-rod channels (leaves at the top plenum, unheated).
    pub control_rods: f64,
    /// Leakage through graphite gaps, cold to hot (leaves at the top plenum).
    pub leakage: f64,
}

impl BypassSplit {
    /// [T1] Sec 2.1.1.3: 87 / 1 / 2 / 10 %. The base assumption.
    pub fn tecdoc1694() -> Self {
        Self { bed: 0.87, discharge_tube: 0.01, control_rods: 0.02, leakage: 0.10 }
    }
    /// `BYP-G02`: 86 % to the bed, 14 % over the three paths in [T1]'s
    /// proportions (1 : 2 : 10).
    pub fn g02() -> Self {
        let r = 0.14 / 13.0;
        Self { bed: 0.86, discharge_tube: r, control_rods: 2.0 * r, leakage: 10.0 * r }
    }
    /// `BYP-S5-CR`: control rods 2.5 % ([S5]), leakage 9.5 %.
    pub fn s5_control_rods() -> Self {
        Self { bed: 0.87, discharge_tube: 0.01, control_rods: 0.025, leakage: 0.095 }
    }
    /// `BYP-NOLEAK`: the 10 % leakage to the bed (the bound [T1] implies).
    pub fn no_leakage() -> Self {
        Self { bed: 0.97, discharge_tube: 0.01, control_rods: 0.02, leakage: 0.0 }
    }
    fn check(&self) -> Result<(), TampinesError> {
        let s = self.bed + self.discharge_tube + self.control_rods + self.leakage;
        let all_ok = [self.bed, self.discharge_tube, self.control_rods, self.leakage]
            .iter()
            .all(|v| (0.0..=1.0).contains(v));
        if !all_ok || (s - 1.0).abs() > 1e-9 || self.bed <= 0.0 {
            return Err(TampinesError::InvalidInput(format!(
                "bypass split must be fractions summing to 1 with a positive bed share: {self:?}"
            )));
        }
        Ok(())
    }
}

/// A linear form `sum c_i T_i + k` over the node temperatures.
#[derive(Debug, Clone, PartialEq)]
struct Lf {
    c: Vec<f64>,
    k: f64,
}

impl Lf {
    fn constant(v: f64) -> Self {
        Self { c: vec![0.0; N_NODES], k: v }
    }
    fn node(i: usize) -> Self {
        let mut c = vec![0.0; N_NODES];
        c[i] = 1.0;
        Self { c, k: 0.0 }
    }
    fn axpy(&self, a: f64, o: &Lf, b: f64) -> Lf {
        Lf {
            c: self.c.iter().zip(o.c.iter()).map(|(x, y)| a * x + b * y).collect(),
            k: a * self.k + b * o.k,
        }
    }
    fn eval(&self, t: &[f64]) -> f64 {
        self.c.iter().zip(t.iter()).map(|(c, t)| c * t).sum::<f64>() + self.k
    }
}

/// One wetted gas segment: walls `(node, hA)`, capacity rate `cdot`.
/// Returns the outlet form, the per-wall heat-to-gas forms and the gas
/// mean-temperature form.
fn segment(inlet: &Lf, walls: &[(usize, f64)], cdot: f64) -> (Lf, Vec<(usize, Lf)>, Lf) {
    let h: f64 = walls.iter().map(|w| w.1).sum();
    if h <= 0.0 || cdot <= 0.0 {
        return (inlet.clone(), Vec::new(), inlet.clone());
    }
    let mut t_se = Lf::constant(0.0);
    for (i, ha) in walls {
        t_se = t_se.axpy(1.0, &Lf::node(*i), ha / h);
    }
    let e = (-h / cdot).r_exp();
    let t_out = t_se.axpy(1.0 - e, inlet, e);
    // Q = cdot (T_out - T_in); T_gm = T_se - Q / H.
    let q = t_out.axpy(cdot, inlet, -cdot);
    let t_gm = t_se.axpy(1.0, &q, -1.0 / h);
    let per_wall = walls
        .iter()
        .map(|(i, ha)| (*i, Lf::node(*i).axpy(*ha, &t_gm, -*ha)))
        .collect();
    (t_out, per_wall, t_gm)
}

/// Mix streams `(form, flow weight)`.
fn mix(streams: &[(&Lf, f64)]) -> Lf {
    let total: f64 = streams.iter().map(|s| s.1).sum();
    let mut out = Lf::constant(0.0);
    for (f, w) in streams {
        out = out.axpy(1.0, f, w / total);
    }
    out
}

/// Pipe film coefficient, W/(m^2 K), assumption A8.
fn pipe_h(mass_flux: f64, d_h: f64, he: &KtaHelium) -> f64 {
    let mu = he.viscosity().get::<pascal_second>();
    let k = he.thermal_conductivity().get::<watt_per_meter_kelvin>();
    let pr = he.prandtl().get::<ratio>();
    let re = mass_flux.abs() * d_h / mu;
    let nu_turb = |re: f64| 0.023 * re.r_powf(0.8) * pr.r_powf(0.4);
    let nu = if re >= 1.0e4 {
        nu_turb(re)
    } else if re <= 2300.0 {
        4.36
    } else {
        let w = (re - 2300.0) / (1.0e4 - 2300.0);
        4.36 * (1.0 - w) + nu_turb(1.0e4) * w
    };
    nu * k / d_h
}

/// Static per-zone data.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Zone {
    height: f64,
    area: f64,
    n_fuel: f64,
    n_dummy: f64,
    power_fraction: f64,
}

/// The lumped stand-in core. See the module docs.
#[derive(Debug, Clone, PartialEq)]
pub struct LumpedPebbleBedCore {
    bypass: BypassSplit,
    fuel_fast_fluence_per_cm2: f64,
    zones: Vec<Zone>,
    /// Node temperatures, K.
    temps: Vec<f64>,
    /// Gas mean temperature per bed zone (0..=5), downcomer, risers, top
    /// plenum, hot plenum (property evaluation and gravity head), K.
    gas: GasTemperatures,
    last_outlet: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct GasTemperatures {
    bed: [f64; N_ZONES + 1],
    downcomer: f64,
    downcomer_out: f64,
    risers: f64,
    top_plenum: f64,
    hot_plenum: f64,
}

impl LumpedPebbleBedCore {
    /// The HTR-10 stand-in at the as-run LOFC loading, with `bypass`, every
    /// node seeded at `initial_temperature`.
    ///
    /// # Errors
    /// [`TampinesError::InvalidInput`] for a bad bypass split; the power map
    /// parse errors.
    pub fn htr10(bypass: BypassSplit, initial_temperature: ThermodynamicTemperature) -> Result<Self, TampinesError> {
        bypass.check()?;
        let map = lofc_power_map()?;
        let band_powers: Vec<f64> = (0..N_ZONES)
            .map(|k| map_power_in_band_w(&map, ZONE_EDGES_CM[k], ZONE_EDGES_CM[k + 1]))
            .collect();
        let total: f64 = band_powers.iter().sum();
        let mixed_height = ZONE_EDGES_CM[N_ZONES] - ZONE_EDGES_CM[0];
        // A1: [J09] Tab 2 counts, mixed 57:43 (sheet Sec 6, Derived 10,298).
        let fuel = 13_651.0;
        let mixed_dummy = fuel * 43.0 / 57.0;
        let mut zones: Vec<Zone> = (0..N_ZONES)
            .map(|k| {
                let h_cm = ZONE_EDGES_CM[k + 1] - ZONE_EDGES_CM[k];
                Zone {
                    height: h_cm / 100.0,
                    area: PI * R_BED * R_BED,
                    n_fuel: fuel * h_cm / mixed_height,
                    n_dummy: mixed_dummy * h_cm / mixed_height,
                    power_fraction: band_powers[k] / total,
                }
            })
            .collect();
        // Cone: stepped radii 83.5 / 70.5 / 60 / 50 / 39 cm, 7.5 cm steps
        // (sheet Sec 3.2), the first step's 0.5 cm above 187 cm neglected.
        let cone_volume: f64 = [0.835_f64, 0.705, 0.60, 0.50, 0.39]
            .iter()
            .map(|r| PI * r * r * 0.075)
            .sum();
        let v_peb = PI / 6.0 * D_PEB * D_PEB * D_PEB;
        zones.push(Zone {
            height: CONE_HEIGHT,
            area: cone_volume / 0.375,
            n_fuel: 0.0,
            n_dummy: cone_volume * (1.0 - POROSITY) / v_peb,
            power_fraction: 0.0,
        });
        let t0 = initial_temperature.get::<kelvin>();
        Ok(Self {
            bypass,
            fuel_fast_fluence_per_cm2: 0.0,
            zones,
            temps: vec![t0; N_NODES],
            gas: GasTemperatures {
                bed: [t0; N_ZONES + 1],
                downcomer: t0,
                downcomer_out: t0,
                risers: t0,
                top_plenum: t0,
                hot_plenum: t0,
            },
            last_outlet: t0,
        })
    }

    /// The bypass split in use.
    pub fn bypass(&self) -> BypassSplit {
        self.bypass
    }

    /// Zone power fractions (Tab 2.56), heated zones then the cone (0).
    pub fn zone_power_fractions(&self) -> Vec<f64> {
        self.zones.iter().map(|z| z.power_fraction).collect()
    }

    /// Zone heights, m, cone last (what the solver uses for capacities,
    /// conductances and the gravity head).
    pub fn zone_heights(&self) -> Vec<f64> {
        self.zones.iter().map(|z| z.height).collect()
    }

    /// The radial bodies the solver uses, `(name, r_in, r_out, height)` in m:
    /// side-reflector sub-annuli with their solid fractions in the name,
    /// brick, barrel, RPV, and the RCCS radius (zero thickness).
    pub fn radial_layout() -> Vec<(String, f64, f64, f64)> {
        let mut v: Vec<(String, f64, f64, f64)> = SIDE_REFLECTOR
            .iter()
            .map(|(a, b, f)| (format!("side reflector (solid {f})"), *a, *b, H_REFL))
            .collect();
        v.push(("carbon brick".into(), R_REFL_OUT, R_BRICK_OUT, H_REFL));
        v.push(("barrel".into(), R_BARREL_IN, R_BARREL_OUT, H_BARREL));
        v.push(("RPV".into(), R_RPV_IN, R_RPV_OUT, H_RPV));
        v.push(("RCCS (50 C)".into(), R_RCCS, R_RCCS, H_RPV));
        v
    }

    /// Pebble counts `(fuel, dummy)` per zone, cone last.
    pub fn zone_pebbles(&self) -> Vec<(f64, f64)> {
        self.zones.iter().map(|z| (z.n_fuel, z.n_dummy)).collect()
    }

    /// Node temperatures, K: `[f0, s0, d0, .., f4, s4, d4, cone, R, B, V]`.
    pub fn node_temperatures(&self) -> &[f64] {
        &self.temps
    }

    /// Bed gas mean temperature per zone (cone last), K.
    pub fn bed_gas_temperatures(&self) -> [f64; N_ZONES + 1] {
        self.gas.bed
    }

    fn opts(p_bar: f64) -> MaterialOptions {
        MaterialOptions { helium_pressure_bar: p_bar.clamp(10.0, 100.0), ..MaterialOptions::default() }
    }

    fn helium(t: f64, p_bar: f64) -> Result<KtaHelium, TampinesError> {
        KtaHelium::new(
            Pressure::new::<bar>(p_bar.clamp(KtaHelium::MIN_PRESSURE_BAR, KtaHelium::MAX_PRESSURE_BAR)),
            ThermodynamicTemperature::new::<kelvin>(
                t.clamp(KtaHelium::MIN_TEMPERATURE_K, KtaHelium::MAX_TEMPERATURE_K),
            ),
        )
        .map_err(prop_err)
    }

    fn k_of(m: RegionMaterial, t: f64, o: &MaterialOptions) -> Result<f64, TampinesError> {
        m.conductivity_si(t, o).map_err(prop_err)
    }

    fn c_of(m: RegionMaterial, t: f64, o: &MaterialOptions) -> Result<f64, TampinesError> {
        m.heat_capacity_si(t, o).map_err(prop_err)
    }

    /// Fuel-element conductivity (Tab 2.9), held at 450 degC below and
    /// 1300 degC above (the fit's own hold and range).
    fn k_fuel_element(&self, t: f64) -> Result<f64, TampinesError> {
        StructuralMaterial::FuelElement {
            fast_fluence: ArealNumberDensity::new::<per_square_centimeter>(self.fuel_fast_fluence_per_cm2),
        }
        .thermal_conductivity(ThermodynamicTemperature::new::<kelvin>(t.clamp(723.15, 1573.15)))
        .map(|k| k.get::<watt_per_meter_kelvin>())
        .map_err(prop_err)
    }

    /// Heat capacities of every node at the current temperatures, J/K.
    fn capacities(&self, o: &MaterialOptions) -> Result<Vec<f64>, TampinesError> {
        let graphite = RegionMaterial::TopBottomReflector { solid_fraction: 1.0 };
        let v_peb = PI / 6.0 * D_PEB * D_PEB * D_PEB;
        let v_fz = 4.0 / 3.0 * PI * R_FUEL_ZONE.r_powi(3);
        let mut c = vec![0.0; N_NODES];
        for k in 0..N_ZONES {
            let z = self.zones[k];
            c[f_idx(k)] = z.n_fuel * v_fz * Self::c_of(graphite, self.temps[f_idx(k)], o)?;
            c[s_idx(k)] = z.n_fuel * (v_peb - v_fz) * Self::c_of(graphite, self.temps[s_idx(k)], o)?;
            c[d_idx(k)] = z.n_dummy * v_peb * Self::c_of(graphite, self.temps[d_idx(k)], o)?;
        }
        let cone = self.zones[N_ZONES];
        c[IDX_CONE] = cone.n_dummy * v_peb * Self::c_of(graphite, self.temps[IDX_CONE], o)?;
        let tr = self.temps[IDX_R];
        let mut cr = 0.0;
        for (ri, ro, sf) in SIDE_REFLECTOR {
            cr += PI * (ro * ro - ri * ri) * H_REFL
                * Self::c_of(RegionMaterial::SideReflector { solid_fraction: sf }, tr, o)?;
        }
        cr += PI * (R_BRICK_OUT * R_BRICK_OUT - R_REFL_OUT * R_REFL_OUT) * H_REFL
            * Self::c_of(RegionMaterial::CarbonBrick, tr, o)?;
        c[IDX_R] = cr;
        c[IDX_B] = PI * (R_BARREL_OUT.r_powi(2) - R_BARREL_IN.r_powi(2)) * H_BARREL
            * Self::c_of(RegionMaterial::BarrelSteel { solid_fraction: 1.0 }, self.temps[IDX_B], o)?;
        c[IDX_V] = PI * (R_RPV_OUT.r_powi(2) - R_RPV_IN.r_powi(2)) * H_RPV
            * Self::c_of(RegionMaterial::RpvSteel { solid_fraction: 1.0 }, self.temps[IDX_V], o)?;
        Ok(c)
    }

    /// Side-reflector conductivity, area-weighted over the sub-annuli.
    fn k_side_reflector(t: f64, o: &MaterialOptions) -> Result<f64, TampinesError> {
        let mut num = 0.0;
        let mut den = 0.0;
        for (ri, ro, sf) in SIDE_REFLECTOR {
            let a = ro * ro - ri * ri;
            num += a * Self::k_of(RegionMaterial::SideReflector { solid_fraction: sf }, t, o)?;
            den += a;
        }
        Ok(num / den)
    }

    /// Solid conductances `(i, j, G)` and boundary conductances to the RCCS
    /// `(i, G)`, evaluated at the current temperatures.
    fn conductances(&self, o: &MaterialOptions) -> Result<(Vec<(usize, usize, f64)>, Vec<(usize, f64)>), TampinesError> {
        let t = &self.temps;
        let mut g = Vec::new();
        let bed = RegionMaterial::PebbleBed;
        let a = R_FUEL_ZONE;
        let r_peb = 0.5 * D_PEB;
        let k_sr_inner = Self::k_side_reflector(0.5 * (t[IDX_R] + t[s_idx(2)]), o)?;
        for k in 0..N_ZONES {
            let z = self.zones[k];
            let n = z.n_fuel + z.n_dummy;
            let (xf, xd) = (z.n_fuel / n, z.n_dummy / n);
            // A3: uniformly heated sphere (mean minus surface = Q/(20 pi k a))
            // plus the unheated shell a..R (Q (1/a - 1/R) / (4 pi k)).
            let kf = self.k_fuel_element(t[f_idx(k)])?;
            let r_one = 1.0 / (20.0 * PI * kf * a) + (1.0 / a - 1.0 / r_peb) / (4.0 * PI * kf);
            g.push((f_idx(k), s_idx(k), z.n_fuel / r_one));
            let t_bed = xf * t[s_idx(k)] + xd * t[d_idx(k)];
            let kb = Self::k_of(bed, t_bed, o)?;
            // A4.
            g.push((s_idx(k), d_idx(k), z.n_fuel * 2.0 * PI * kb * D_PEB * xd));
            // Bed mean -> wall (8 pi k H) in series with the inner half of
            // the side reflector, split by pebble share.
            let g_bed = 8.0 * PI * kb * z.height;
            let g_wall = 2.0 * PI * k_sr_inner * z.height / (R_REFL_MID / R_BED).r_ln();
            let g_rad = 1.0 / (1.0 / g_bed + 1.0 / g_wall);
            g.push((s_idx(k), IDX_R, xf * g_rad));
            g.push((d_idx(k), IDX_R, xd * g_rad));
            // Axial bed conduction to the next zone (or the cone).
            let next = self.zones[k + 1];
            let (dz, area) = if k + 1 < N_ZONES {
                (0.5 * (z.height + next.height), PI * R_BED * R_BED)
            } else {
                (0.5 * (z.height + next.height), PI * 0.835 * 0.835)
            };
            let t_next = if k + 1 < N_ZONES {
                let nn = next.n_fuel + next.n_dummy;
                (next.n_fuel * t[s_idx(k + 1)] + next.n_dummy * t[d_idx(k + 1)]) / nn
            } else {
                t[IDX_CONE]
            };
            let kax = Self::k_of(bed, 0.5 * (t_bed + t_next), o)?;
            let g_ax = kax * area / dz;
            if k + 1 < N_ZONES {
                let nn = next.n_fuel + next.n_dummy;
                let xf_n = next.n_fuel / nn;
                // Shell-to-shell for the fuel share, dummy-to-dummy for the rest.
                g.push((s_idx(k), s_idx(k + 1), g_ax * 0.5 * (xf + xf_n)));
                g.push((d_idx(k), d_idx(k + 1), g_ax * (1.0 - 0.5 * (xf + xf_n))));
            } else {
                g.push((s_idx(k), IDX_CONE, g_ax * xf));
                g.push((d_idx(k), IDX_CONE, g_ax * xd));
            }
        }
        // R -> B: outer half of the graphite, the brick, the 1 cm He gap.
        let k_sr_outer = Self::k_side_reflector(t[IDX_R], o)?;
        let k_cb = Self::k_of(RegionMaterial::CarbonBrick, t[IDX_R], o)?;
        let k_gap = Self::k_of(
            RegionMaterial::HeliumCavity { emissivity: EPS_GAP, width_m: R_BARREL_IN - R_BRICK_OUT },
            0.5 * (t[IDX_R] + t[IDX_B]),
            o,
        )?;
        let r_rb = (R_REFL_OUT / R_REFL_MID).r_ln() / (2.0 * PI * k_sr_outer * H_REFL)
            + (R_BRICK_OUT / R_REFL_OUT).r_ln() / (2.0 * PI * k_cb * H_REFL)
            + (R_BARREL_IN / R_BRICK_OUT).r_ln() / (2.0 * PI * k_gap * H_REFL);
        g.push((IDX_R, IDX_B, 1.0 / r_rb));
        // B -> V: helium annulus, gas conduction + radiation, over the barrel.
        let k_ann = Self::k_of(
            RegionMaterial::HeliumCavity { emissivity: EPS_ANNULUS, width_m: R_RPV_IN - R_BARREL_OUT },
            0.5 * (t[IDX_B] + t[IDX_V]),
            o,
        )?;
        g.push((IDX_B, IDX_V, 2.0 * PI * k_ann * H_BARREL / (R_RPV_IN / R_BARREL_OUT).r_ln()));
        // V -> RCCS: concentric grey cylinders, secant radiative conductance.
        let eps = 1.0 / (1.0 / EPS_CAVITY + R_RPV_OUT / R_RCCS * (1.0 / EPS_CAVITY - 1.0));
        let area = 2.0 * PI * R_RPV_OUT * H_RPV;
        let tv = t[IDX_V];
        let g_rccs = SIGMA_SB * eps * area * (tv * tv + T_RCCS * T_RCCS) * (tv + T_RCCS);
        Ok((g, vec![(IDX_V, g_rccs)]))
    }

    /// Bed film `hA` per zone `(shell, dummy)` at superficial bed flow `w_bed`.
    fn bed_film(&self, k: usize, w_bed: f64, p_bar: f64) -> Result<(f64, f64), TampinesError> {
        let z = self.zones[k];
        if w_bed.abs() <= 0.0 {
            return Ok((0.0, 0.0));
        }
        let he = Self::helium(self.gas.bed[k], p_bar)?;
        let flux = uom::si::f64::MassFlux::new::<uom::si::mass_flux::kilogram_per_square_meter_second>(
            w_bed.abs() / z.area,
        );
        let h = kta_3102_2_heat_transfer_coefficient(
            flux,
            Length::new::<meter>(D_PEB),
            Ratio::new::<ratio>(POROSITY),
            he.viscosity(),
            he.thermal_conductivity(),
            he.prandtl(),
        )?
        .get::<uom::si::heat_transfer::watt_per_square_meter_kelvin>();
        let a1 = PI * D_PEB * D_PEB;
        Ok((h * a1 * z.n_fuel, h * a1 * z.n_dummy))
    }

    /// The gas path as linear forms: per-node heat-to-gas forms, the
    /// outlet form, and gas-temperature forms for bookkeeping.
    #[allow(clippy::type_complexity)]
    fn gas_path(
        &self,
        boundary: &CoreBoundary,
    ) -> Result<(Vec<(usize, Lf)>, Lf, Vec<(usize, Lf)>), TampinesError> {
        let w = boundary.mass_flow.get::<kilogram_per_second>();
        let p_bar = boundary.pressure.get::<bar>();
        let mut q: Vec<(usize, Lf)> = Vec::new();
        // bookkeeping slots: 0..=5 bed zones, 6 downcomer, 7 downcomer out,
        // 8 risers, 9 top plenum, 10 hot plenum.
        let mut book: Vec<(usize, Lf)> = Vec::new();
        if w == 0.0 {
            let out = Lf::constant(self.last_outlet);
            return Ok((q, out, book));
        }
        let wa = w.abs();
        let b = self.bypass;
        let a_dc = PI * (R_RPV_IN * R_RPV_IN - R_BARREL_OUT * R_BARREL_OUT);
        let h_dc = pipe_h(wa / a_dc, 2.0 * (R_RPV_IN - R_BARREL_OUT), &Self::helium(self.gas.downcomer, p_bar)?);
        let dc_walls = [
            (IDX_B, h_dc * 2.0 * PI * R_BARREL_OUT * H_DOWNCOMER),
            (IDX_V, h_dc * 2.0 * PI * R_RPV_IN * H_DOWNCOMER),
        ];
        let w_riser = (1.0 - b.discharge_tube) * wa;
        let a_r = N_RISERS * PI * 0.25 * D_RISER * D_RISER;
        let h_r = pipe_h(w_riser / a_r, D_RISER, &Self::helium(self.gas.risers, p_bar)?);
        let riser_walls = [(IDX_R, h_r * N_RISERS * PI * D_RISER * L_RISER)];
        let w_bed = b.bed * wa;
        let mut bed_walls = Vec::with_capacity(N_ZONES + 1);
        for k in 0..=N_ZONES {
            let (hs, hd) = self.bed_film(k, w_bed, p_bar)?;
            let mut v = Vec::new();
            if k < N_ZONES {
                v.push((s_idx(k), hs));
            }
            v.push((d_idx(k), hd));
            bed_walls.push(v);
        }
        let push = |q: &mut Vec<(usize, Lf)>, items: Vec<(usize, Lf)>| q.extend(items);
        if w > 0.0 {
            let t_in = Lf::constant(boundary.cold_nozzle_temperature.get::<kelvin>());
            let (t_dc, qa, gm) = segment(&t_in, &dc_walls, wa * CP_HE);
            push(&mut q, qa);
            book.push((6, gm));
            book.push((7, t_dc.clone()));
            let (t_top, qa, gm) = segment(&t_dc, &riser_walls, w_riser * CP_HE);
            push(&mut q, qa);
            book.push((8, gm));
            book.push((9, t_top.clone()));
            let mut t_bed = t_top.clone();
            for (k, walls) in bed_walls.iter().enumerate() {
                let (o, qa, gm) = segment(&t_bed, walls, w_bed * CP_HE);
                push(&mut q, qa);
                book.push((k, gm));
                t_bed = o;
            }
            let hot = mix(&[
                (&t_bed, b.bed),
                (&t_top, b.control_rods + b.leakage),
                (&t_dc, b.discharge_tube),
            ]);
            book.push((10, hot.clone()));
            Ok((q, hot, book))
        } else {
            let t_in = Lf::constant(boundary.hot_nozzle_temperature.get::<kelvin>());
            book.push((10, t_in.clone()));
            let mut t_bed = t_in.clone();
            for k in (0..=N_ZONES).rev() {
                let (o, qa, gm) = segment(&t_bed, &bed_walls[k], w_bed * CP_HE);
                push(&mut q, qa);
                book.push((k, gm));
                t_bed = o;
            }
            let t_top = mix(&[(&t_bed, b.bed), (&t_in, b.control_rods + b.leakage)]);
            book.push((9, t_top.clone()));
            let (t_r, qa, gm) = segment(&t_top, &riser_walls, w_riser * CP_HE);
            push(&mut q, qa);
            book.push((8, gm));
            let t_bottom = mix(&[(&t_r, 1.0 - b.discharge_tube), (&t_in, b.discharge_tube)]);
            book.push((7, t_bottom.clone()));
            let (t_out, qa, gm) = segment(&t_bottom, &dc_walls, wa * CP_HE);
            push(&mut q, qa);
            book.push((6, gm));
            Ok((q, t_out, book))
        }
    }

    /// One backward-Euler step (`dt = Some`) or one steady Picard pass
    /// (`dt = None`), conductances at the current temperatures.
    pub(crate) fn advance(
        &mut self,
        dt: Option<Time>,
        power: Power,
        boundary: &CoreBoundary,
    ) -> Result<CoreResponse, TampinesError> {
        let p_bar = boundary.pressure.get::<bar>();
        let o = Self::opts(p_bar);
        let (g, gb) = self.conductances(&o)?;
        let caps = self.capacities(&o)?;
        let (q, out, book) = self.gas_path(boundary)?;
        let p = power.get::<watt>();
        let mut m = SquareMatrix::new(N_NODES);
        let mut rhs = vec![0.0; N_NODES];
        if let Some(dt) = dt {
            let h = dt.get::<second>();
            for i in 0..N_NODES {
                m.add(i, i, caps[i] / h);
                rhs[i] += caps[i] / h * self.temps[i];
            }
        }
        for k in 0..N_ZONES {
            rhs[f_idx(k)] += p * self.zones[k].power_fraction;
        }
        for (i, j, gij) in &g {
            m.add(*i, *i, *gij);
            m.add(*j, *j, *gij);
            m.add(*i, *j, -*gij);
            m.add(*j, *i, -*gij);
        }
        for (i, gi) in &gb {
            m.add(*i, *i, *gi);
            rhs[*i] += gi * T_RCCS;
        }
        for (i, lf) in &q {
            for (j, c) in lf.c.iter().enumerate() {
                if *c != 0.0 {
                    m.add(*i, j, *c);
                }
            }
            rhs[*i] -= lf.k;
        }
        let new = m
            .solve(&rhs)
            .map_err(|e| TampinesError::Numerical(format!("lumped core solve: {e}")))?;
        if new.iter().any(|t| !t.is_finite() || *t <= 0.0) {
            return Err(TampinesError::Unphysical(format!("lumped core temperatures {new:?}")));
        }
        self.temps = new;
        let t = &self.temps;
        for (slot, lf) in &book {
            let v = lf.eval(t);
            match slot {
                0..=5 => self.gas.bed[*slot] = v,
                6 => self.gas.downcomer = v,
                7 => self.gas.downcomer_out = v,
                8 => self.gas.risers = v,
                9 => self.gas.top_plenum = v,
                _ => self.gas.hot_plenum = v,
            }
        }
        let t_out = out.eval(t);
        self.last_outlet = t_out;
        let q_coolant: f64 = q.iter().map(|(_, lf)| lf.eval(t)).sum();
        let q_rccs: f64 = gb.iter().map(|(i, gi)| gi * (t[*i] - T_RCCS)).sum();
        Ok(CoreResponse {
            outlet_temperature: ThermodynamicTemperature::new::<kelvin>(t_out),
            power_to_coolant: Power::new::<watt>(q_coolant),
            heat_to_rccs: Power::new::<watt>(q_rccs),
            feedback: self.feedback_temperatures(),
        })
    }

    /// Under-relaxed Picard iteration of [`Self::advance`] without
    /// capacities until the largest node change of an unrelaxed pass is
    /// below 1e-7 K.
    ///
    /// # Errors
    /// [`TampinesError::Numerical`] if 2000 passes do not converge.
    pub fn steady_state(&mut self, power: Power, boundary: &CoreBoundary) -> Result<CoreResponse, TampinesError> {
        let mut last = self.temps.clone();
        for _ in 0..2000 {
            let r = self.advance(None, power, boundary)?;
            let change = self
                .temps
                .iter()
                .zip(last.iter())
                .map(|(a, b)| (a - b).abs())
                .fold(0.0, f64::max);
            if change < 1e-7 {
                return Ok(r);
            }
            // Under-relaxed Picard (0.5): the temperature-dependent
            // conductances and film coefficients can make the plain
            // fixed point oscillate (it does for reversed flow).
            for (t, l) in self.temps.iter_mut().zip(last.iter()) {
                *t = 0.5 * (*t + *l);
            }
            last = self.temps.clone();
        }
        Err(TampinesError::Numerical("lumped core steady state did not converge in 2000 passes".into()))
    }

    /// Power-weighted fuel and moderator temperatures, and the reflector's.
    pub fn feedback_temperatures(&self) -> FeedbackTemperatures {
        let v_peb = PI / 6.0 * D_PEB * D_PEB * D_PEB;
        let v_fz = 4.0 / 3.0 * PI * R_FUEL_ZONE.r_powi(3);
        let mut tf = 0.0;
        let mut tm = 0.0;
        for k in 0..N_ZONES {
            let z = self.zones[k];
            let w = z.power_fraction;
            tf += w * self.temps[f_idx(k)];
            let vs = z.n_fuel * (v_peb - v_fz);
            let vd = z.n_dummy * v_peb;
            tm += w * (vs * self.temps[s_idx(k)] + vd * self.temps[d_idx(k)]) / (vs + vd);
        }
        FeedbackTemperatures {
            fuel: ThermodynamicTemperature::new::<kelvin>(tf),
            moderator: ThermodynamicTemperature::new::<kelvin>(tm),
            reflector: ThermodynamicTemperature::new::<kelvin>(self.temps[IDX_R]),
        }
    }

    fn form_loss(spec: (f64, f64, f64), w: f64, rho: f64) -> Result<f64, TampinesError> {
        let rho_ref = Self::helium(spec.2, 30.0)?.density().get::<kilogram_per_cubic_meter>();
        Ok(spec.0 * 1.0e3 * (w / spec.1) * (w / spec.1).abs() * rho_ref / rho)
    }

    /// Signed in-RPV pressure loss at the trial loop flow (see module docs).
    pub fn pressure_drop(&self, mass_flow: MassRate, pressure: Pressure) -> Result<Pressure, TampinesError> {
        let w = mass_flow.get::<kilogram_per_second>();
        if w == 0.0 {
            return Ok(Pressure::new::<pascal>(0.0));
        }
        let p_bar = pressure.get::<bar>();
        let wb = self.bypass.bed * w.abs();
        let mut dp = 0.0;
        for (k, z) in self.zones.iter().enumerate() {
            let he = Self::helium(self.gas.bed[k], p_bar)?;
            let bed = KtaBed::new(
                Ratio::new::<ratio>(POROSITY),
                Length::new::<meter>(D_PEB),
                Area::new::<square_meter>(z.area),
            );
            dp += bed
                .pressure_drop(
                    MassRate::new::<kilogram_per_second>(wb),
                    MassDensity::new::<kilogram_per_cubic_meter>(he.density().get::<kilogram_per_cubic_meter>()),
                    DynamicViscosity::new::<pascal_second>(he.viscosity().get::<pascal_second>()),
                    Length::new::<meter>(z.height),
                )?
                .get::<pascal>();
        }
        let rho_r = Self::helium(self.gas.risers, p_bar)?.density().get::<kilogram_per_cubic_meter>();
        let rho_h = Self::helium(self.gas.hot_plenum, p_bar)?.density().get::<kilogram_per_cubic_meter>();
        dp += Self::form_loss(DP_SIDE_REFLECTOR_PASS, (1.0 - self.bypass.discharge_tube) * w.abs(), rho_r)?;
        dp += Self::form_loss(DP_MIXTURE_PLENUMS, w.abs(), rho_h)?;
        Ok(Pressure::new::<pascal>(dp * w.signum()))
    }

    /// `-g sum rho dz` along the forced direction over the closed in-RPV
    /// path between the nozzles (which sit at the same elevation).
    pub fn gravity_head(&self, pressure: Pressure) -> Result<Pressure, TampinesError> {
        let p_bar = pressure.get::<bar>();
        let rho = |t: f64| -> Result<f64, TampinesError> {
            Ok(Self::helium(t, p_bar)?.density().get::<kilogram_per_cubic_meter>())
        };
        let mut s = rho(self.gas.downcomer)? * DZ_DOWNCOMER
            + rho(self.gas.downcomer_out)? * DZ_BOTTOM_TURN
            + rho(self.gas.risers)? * DZ_RISERS
            + rho(self.gas.top_plenum)? * DZ_TOP_PLENUM
            + rho(self.gas.hot_plenum)? * DZ_BELOW_CONE;
        for (k, z) in self.zones.iter().enumerate() {
            s -= rho(self.gas.bed[k])? * z.height;
        }
        Ok(Pressure::new::<pascal>(-G * s))
    }

    /// `sum L / A` of the in-RPV path, 1/m (downcomer, risers, bed on its
    /// interstitial area).
    pub fn inertance_per_m(&self) -> f64 {
        let a_dc = PI * (R_RPV_IN * R_RPV_IN - R_BARREL_OUT * R_BARREL_OUT);
        let a_r = N_RISERS * PI * 0.25 * D_RISER * D_RISER;
        let bed: f64 = self.zones.iter().map(|z| z.height / (z.area * POROSITY)).sum();
        DZ_DOWNCOMER.abs() / a_dc + L_RISER / a_r + bed
    }

    /// Total stored-energy capacity `sum C_i`, J/K, at the current state;
    /// `(bed pebbles, reflector, barrel + RPV)`.
    ///
    /// # Errors
    /// Property errors.
    pub fn capacity_summary(&self, pressure: Pressure) -> Result<(f64, f64, f64), TampinesError> {
        let c = self.capacities(&Self::opts(pressure.get::<bar>()))?;
        let bed: f64 = c[..=IDX_CONE].iter().sum();
        Ok((bed, c[IDX_R], c[IDX_B] + c[IDX_V]))
    }

    /// Conductance summary at the current state, W/K: `(sum fuel->shell,
    /// sum bed->reflector, reflector->barrel, barrel->RPV, RPV->RCCS)`.
    ///
    /// # Errors
    /// Property errors.
    pub fn conductance_summary(&self, pressure: Pressure) -> Result<[f64; 5], TampinesError> {
        let (g, gb) = self.conductances(&Self::opts(pressure.get::<bar>()))?;
        let mut out = [0.0; 5];
        for (i, j, v) in g {
            if j == i + 1 && i % 3 == 0 && i < IDX_CONE {
                out[0] += v;
            } else if j == IDX_R {
                out[1] += v;
            } else if i == IDX_R && j == IDX_B {
                out[2] += v;
            } else if i == IDX_B && j == IDX_V {
                out[3] += v;
            }
        }
        out[4] = gb.iter().map(|x| x.1).sum();
        Ok(out)
    }
}
