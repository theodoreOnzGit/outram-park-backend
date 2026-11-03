// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! **Helical-coil once-through steam generator, steady 1-D march**
//! (gh:#1157, part of epic gh:#1153).
//!
//! > Research, education and V&V only. AI draft, unreviewed. Not for
//! > facility operation, licensing or safety decisions.
//!
//! # What it computes
//!
//! One helically coiled tube (of `parallel_tubes` identical ones) carries
//! water upward from a subcooled inlet to a superheated outlet. A
//! single-phase hot fluid with constant, user-set properties flows down the
//! shell in **counterflow**. The model marches the water side along the tube
//! from inlet to outlet, `s = 0 … L`, integrating
//!
//! ```text
//! m_w dh/ds     =  q'(s)                      water enthalpy (IF97 water/steam)
//! m_h c_p dT_h/ds =  q'(s)                    hot fluid, flowing towards s = 0
//! dp/ds         = −(friction + gravity) − G² dv/ds   (acceleration, HEM volume)
//! q'(s)         =  (T_h − T_wi) / (R'_o + R'_wall) = π d_i q''_i(T_wi)
//! ```
//!
//! with a second-order (Heun) step per cell, and finds the hot-fluid outlet
//! temperature that makes the march return the hot **inlet** temperature at
//! `s = L` by shooting (Illinois regula falsi, [`Shooting`]). Each shooting
//! iteration is one march, so a caller can step it (the browser demo asks for
//! one per worker message).
//!
//! # The water side, region by region
//!
//! | Regime ([`WaterRegime`]) | Inner coefficient | Source |
//! |---|---|---|
//! | subcooled liquid | coil single-phase Nusselt, liquid properties at `(p, h)` | [`correlations::nusselt_coil`] (Gnielinski 1986) |
//! | subcooled boiling | the same plus `S h_FZ (T_w − T_sat)` once `T_w > T_sat`, `F = 1` | Chen (1966) superposition extended to subcooled flow **[U]** (see below) |
//! | saturated boiling | Chen: `F h_l + S h_FZ` on `T_w − T_sat`, `h_l` the coil liquid-alone value | [`correlations::chen_f`], [`correlations::chen_s`], [`correlations::forster_zuber_htc`] |
//! | post-dryout | Dougall–Rohsenow vapour coefficient on `T_w − T_sat` | reused: `outram_foam_multiphase::dryout::PostDryoutModel` |
//! | superheated vapour | coil single-phase Nusselt, vapour properties at `(p, h)` | [`correlations::nusselt_coil`] |
//!
//! **Dryout** is placed where the pre-dryout inner-wall heat flux reaches
//! the Biasi et al. (1967) critical heat flux at the local `(p, G, x, d_i)`
//! (reused: `outram_foam_multiphase::chf::Biasi`), located inside its cell
//! by linear interpolation of `q''/q''_CHF`, and holds for the rest of the
//! tube (in a steady once-through tube the quality only rises). The CHF is
//! the straight round-tube value: coil
//! curvature is known to change it and is **not** modelled.
//!
//! **Pressure drop**: coil friction ([`correlations::darcy_friction_coil`])
//! single-phase; Lockhart–Martinelli/Chisholm with coil friction factors
//! two-phase ([`correlations::two_phase_friction_gradient`]); gravity
//! `g sin θ / v` along the helix; acceleration `G² Δv` per cell, with `v`
//! the IF97 equilibrium (HEM) specific volume, so no slip.
//!
//! **Hot side**: constant `c_p` and a constant, user-set shell-side
//! coefficient `h_o`. No shell-side correlation is evaluated: the shell
//! geometry of a coil bundle is not part of this model's input.
//!
//! **Wall**: cylindrical conduction `ln(d_o/d_i) / (2π k_w)` per unit
//! length, constant `k_w`.
//!
//! # Physics on by default; ablations are explicit
//!
//! [`OtsgOptions::default`] has every term on. Each `without_*` builder turns
//! one off: [`OtsgOptions::without_curvature`] (the coil correlations at
//! `d/D_c = 0`), `without_nucleate_boiling`, `without_subcooled_boiling`,
//! `without_dryout`, `without_gravity`, `without_acceleration`,
//! `without_wall_resistance`. `tests::correct_physics_is_the_default` pins
//! the default.
//!
//! # Stated simplifications (NOT in model)
//!
//! - **Thermodynamic equilibrium**: the bulk is at `T_sat` from `x = 0` to
//!   `x = 1`; no vapour superheat beside droplets after dryout, no subcooled
//!   void.
//! - **No wall-property correction** (`(Pr/Pr_w)^0.14` or a gas
//!   temperature-ratio factor): a boiling tube's wall state crosses
//!   saturation, where a single-phase `Pr_w` is undefined.
//! - **No DNB in subcooled flow**; CHF is checked only for `x > 0`.
//! - **No transition boiling**: the coefficient drops from Chen to
//!   Dougall–Rohsenow at the dryout node.
//! - **No axial wall conduction, no entrance effects**, smooth tube.
//! - **The subcooled-boiling extension** (Chen with `F = 1`, `S` at the
//!   liquid Reynolds number) is the form the `ht` library search summary
//!   attributes to the literature; its primary source was not read **[U]**.
//!
//! Verification and validation, with predictions written before the runs:
//! `crates/tampines/verification_and_validation/helical_coil_otsg_gh1157.md`.

pub mod correlations;
#[cfg(test)]
mod tests;

use outram_foam_multiphase::chf::{Biasi, ChfModel};
use petir::mathf::RealMath;
use outram_foam_multiphase::dryout::{
    PostDryoutConditions, PostDryoutHeatTransfer, PostDryoutModel, VaporProperties,
};
use tampines_steam_tables::prelude::{
    cp_ph_eqm, h_tp_eqm_single_phase, lambda_ph_eqm, mu_ph_eqm, sat_pressure_4, sat_temp_4,
    t_ph_eqm, v_ph_eqm,
};
use tampines_steam_tables::surface_tension::water_surf_tension;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{
    AvailableEnergy, DynamicViscosity, HeatFluxDensity, HeatTransfer, Length, MassDensity,
    MassFlux, MassRate, Power, Pressure, Ratio, SpecificHeatCapacity, ThermalConductivity,
    ThermodynamicTemperature,
};
use uom::si::heat_flux_density::watt_per_square_meter;
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::mass_flux::kilogram_per_square_meter_second;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::radiant_exposure::joule_per_square_meter;
use uom::si::ratio::ratio;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

use crate::error::TampinesError;
use crate::multiphase_1d::properties::{SaturatedProperties, SaturatedTransport};
use correlations as corr;

/// Standard gravity \[m/s²\].
const G_STD: f64 = 9.80665;
/// Upper temperature of IAPWS-IF97 Region 2 \[K\]; the march stops a guess
/// whose water or hot side passes it (see [`Shooting`]).
const T_MAX_REGION_2: f64 = 1073.15;
/// Highest temperature at which the saturation pressure is evaluated for the
/// Forster–Zuber `Δp_sat` \[K\], just below the critical point (647.096 K).
/// Only bisection trial points far from any converged boiling wall reach it.
const T_PSAT_CEILING: f64 = 647.0;
/// Shooting stops when the hot side's returned inlet temperature is within
/// this of the set value \[K\].
pub const SHOOTING_TOLERANCE_K: f64 = 1.0e-6;
/// Shooting gives up after this many marches.
pub const SHOOTING_MAX_ITERATIONS: usize = 100;
/// Most closure switches located inside one cell; past it the step is
/// taken whole (a guard against a switch that flips back and forth).
const MAX_EVENTS_PER_CELL: usize = 16;

/// **Geometry of one helical coil** (all `parallel_tubes` are identical).
///
/// | Field | Meaning | Unit |
/// |---|---|---|
/// | `coil_diameter` | helix diameter `D_w`, tube centreline to centreline | m |
/// | `pitch` | axial rise per turn `h` | m |
/// | `turns` | number of turns `N` (need not be an integer) | – |
/// | `tube_inner_diameter` | bore `d_i` (water side) | m |
/// | `tube_outer_diameter` | `d_o` (hot side) | m |
/// | `wall_conductivity` | tube wall `k_w` | W/(m·K) |
/// | `parallel_tubes` | number of identical tubes sharing the flows | – |
///
/// The developed tube length follows from the geometry:
/// `L = N √((π D_w)² + h²)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HelicalCoilGeometry {
    pub coil_diameter: Length,
    pub pitch: Length,
    pub turns: f64,
    pub tube_inner_diameter: Length,
    pub tube_outer_diameter: Length,
    pub wall_conductivity: ThermalConductivity,
    pub parallel_tubes: u32,
}

impl HelicalCoilGeometry {
    /// **An illustrative HTR-10-scale coil**, not a design.
    ///
    /// Published (`docs/reactor-scoping/htr10-plant-data.md` §5): helical
    /// once-through tubes of about **34 m** developed length each, in modules
    /// of **112 mm** bundle diameter. **Invented** (no source states them): a
    /// 0.10 m helix diameter, a 0.03 m pitch, 108 turns (34.08 m, the
    /// published length), 19 mm × 2.5 mm tubes and 90 tubes (as
    /// [`super::helical_coil_steam_generator::SteamGeneratorGeometry::htr10_illustrative`]
    /// invents them), and `k_w = 30 W/(m·K)`, a round value for a low-alloy
    /// steel, not a 2.25Cr1Mo datum.
    pub fn illustrative() -> Self {
        Self {
            coil_diameter: Length::new::<meter>(0.10),
            pitch: Length::new::<meter>(0.03),
            turns: 108.0,
            tube_inner_diameter: Length::new::<meter>(0.014),
            tube_outer_diameter: Length::new::<meter>(0.019),
            wall_conductivity: ThermalConductivity::new::<watt_per_meter_kelvin>(30.0),
            parallel_tubes: 90,
        }
    }

    /// Tube length of one turn, `√((π D_w)² + h²)`.
    pub fn turn_length(&self) -> Length {
        let c = std::f64::consts::PI * self.coil_diameter.get::<meter>();
        let h = self.pitch.get::<meter>();
        Length::new::<meter>((c * c + h * h).sqrt())
    }

    /// Developed length of one tube, `N` turn lengths.
    pub fn tube_length(&self) -> Length {
        self.turn_length() * self.turns
    }

    /// Height of the coil, `N h`.
    pub fn coil_height(&self) -> Length {
        self.pitch * self.turns
    }

    /// Rise per unit tube length, `sin θ = h / turn length`.
    pub fn elevation_sine(&self) -> f64 {
        (self.pitch / self.turn_length()).get::<ratio>()
    }

    /// Mean curvature diameter `D_c` ([`correlations::curvature_diameter`]).
    pub fn curvature_diameter(&self) -> Length {
        Length::new::<meter>(corr::curvature_diameter(
            self.coil_diameter.get::<meter>(),
            self.pitch.get::<meter>(),
        ))
    }

    /// Curvature ratio `d_i / D_c`.
    pub fn curvature_ratio(&self) -> Ratio {
        self.tube_inner_diameter / self.curvature_diameter()
    }

    /// Centreline point at helix angle `phi` \[rad\], `0 ≤ phi ≤ 2πN`: the
    /// helix starts at `(D_w/2, 0, 0)` and rises `h` per turn. Strict SI \[m\].
    pub fn centreline_point(&self, phi: f64) -> [f64; 3] {
        let r = 0.5 * self.coil_diameter.get::<meter>();
        let c = self.pitch.get::<meter>() / (2.0 * std::f64::consts::PI);
        let (s, co) = phi.r_sin_cos();
        [r * co, r * s, c * phi]
    }

    /// Shortest distance from the point `[x, y, z]` \[m\] to the tube
    /// centreline (the helix from `phi = 0` to `2πN`, ends included).
    pub fn distance_to_centreline(&self, point: [f64; 3]) -> f64 {
        let [x, y, z] = point;
        let r = 0.5 * self.coil_diameter.get::<meter>();
        let c = self.pitch.get::<meter>() / (2.0 * std::f64::consts::PI);
        let phi_max = 2.0 * std::f64::consts::PI * self.turns;
        let dist2 = |phi: f64| {
            let p = self.centreline_point(phi);
            let (a, b, e) = (x - p[0], y - p[1], z - p[2]);
            a * a + b * b + e * e
        };
        let phi0 = y.r_atan2(x).rem_euclid(2.0 * std::f64::consts::PI);
        let k0 = ((z - c * phi0) / (2.0 * std::f64::consts::PI * c.max(1e-300))).round();
        let mut best = dist2(0.0).min(dist2(phi_max));
        for dk in -1..=1 {
            let mut phi = phi0 + 2.0 * std::f64::consts::PI * (k0 + f64::from(dk));
            // Newton on d(dist²)/dphi from the angular match.
            for _ in 0..8 {
                let (s, co) = phi.r_sin_cos();
                let g = r * (x * s - y * co) - c * (z - c * phi);
                let h = r * (x * co + y * s) + c * c;
                if h <= 0.0 {
                    break;
                }
                phi -= g / h;
            }
            best = best.min(dist2(phi.clamp(0.0, phi_max)));
        }
        best.sqrt()
    }

    /// **What the solver's geometry holds at a point** \[m\]: the bore
    /// (water, within `d_i/2` of the centreline), the tube wall (up to
    /// `d_o/2`) or the shell (hot fluid). The drawing rule's images are
    /// coloured by this, so they show the helix the march integrates along.
    pub fn region_at(&self, point: [f64; 3]) -> CoilRegion {
        let d = self.distance_to_centreline(point);
        if d <= 0.5 * self.tube_inner_diameter.get::<meter>() {
            CoilRegion::Bore
        } else if d <= 0.5 * self.tube_outer_diameter.get::<meter>() {
            CoilRegion::Wall
        } else {
            CoilRegion::Shell
        }
    }

    /// Refuses a geometry that cannot be built: non-positive sizes,
    /// `d_o ≤ d_i`, a helix no wider than the tube, or a pitch below the
    /// tube outer diameter (adjacent turns would overlap).
    ///
    /// # Errors
    /// [`TampinesError::InvalidInput`] naming the failed condition.
    pub fn validate(&self) -> Result<(), TampinesError> {
        let d_i = self.tube_inner_diameter.get::<meter>();
        let d_o = self.tube_outer_diameter.get::<meter>();
        let d_w = self.coil_diameter.get::<meter>();
        let h = self.pitch.get::<meter>();
        let k = self.wall_conductivity.get::<watt_per_meter_kelvin>();
        let bad = |m: &str| Err(TampinesError::InvalidInput(format!("helical coil: {m}")));
        if !(d_i > 0.0 && d_o > d_i) {
            return bad("need 0 < d_i < d_o");
        }
        if !(d_w > d_o) {
            return bad("the helix diameter must exceed the tube outer diameter");
        }
        if !(h >= d_o) {
            return bad("the pitch must be at least the tube outer diameter (turns would overlap)");
        }
        if !(self.turns > 0.0 && self.turns.is_finite()) {
            return bad("need a positive number of turns");
        }
        if !(k > 0.0) || self.parallel_tubes == 0 {
            return bad("need k_w > 0 and at least one tube");
        }
        Ok(())
    }
}

/// Where a point lies in the coil ([`HelicalCoilGeometry::region_at`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoilRegion {
    /// Inside the tube bore: water/steam.
    Bore,
    /// In the tube wall.
    Wall,
    /// Outside the tube: the hot shell-side fluid.
    Shell,
}

/// **Operating point**: the two inlets and the hot side's constant
/// properties. Flows are totals over all tubes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OperatingPoint {
    pub water_mass_flow: MassRate,
    pub water_inlet_temperature: ThermodynamicTemperature,
    pub water_inlet_pressure: Pressure,
    pub hot_mass_flow: MassRate,
    pub hot_inlet_temperature: ThermodynamicTemperature,
    /// Constant hot-fluid isobaric specific heat.
    pub hot_specific_heat: SpecificHeatCapacity,
    /// Constant shell-side (outer-wall) heat-transfer coefficient, user-set.
    pub shell_side_htc: HeatTransfer,
}

impl OperatingPoint {
    /// **An illustrative HTR-10-like duty**, not a design point.
    ///
    /// Published (`docs/reactor-scoping/htr10-plant-data.md` §5–6): helium
    /// 4.3 kg/s entering at 700 °C; feedwater 3.49 kg/s at 104 °C. Helium
    /// `c_p = (5/2) R / M = 5193.1 J/(kg·K)` (monatomic ideal gas, from the
    /// CODATA gas constant and the helium molar mass 4.002602 g/mol).
    /// **Invented**: the inlet pressure 4.5 MPa (the published values are
    /// 6.1 MPa feedwater before throttles and 4.0 MPa steam; throttles are not
    /// modelled) and `h_o = 500 W/(m²·K)`, a round value chosen before any run
    /// and not adjusted after one.
    pub fn illustrative() -> Self {
        Self {
            water_mass_flow: MassRate::new::<kilogram_per_second>(3.49),
            water_inlet_temperature: ThermodynamicTemperature::new::<kelvin>(377.15),
            water_inlet_pressure: Pressure::new::<pascal>(4.5e6),
            hot_mass_flow: MassRate::new::<kilogram_per_second>(4.3),
            hot_inlet_temperature: ThermodynamicTemperature::new::<kelvin>(973.15),
            hot_specific_heat: SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(
                2.5 * 8.314_462_618 / 4.002_602e-3,
            ),
            shell_side_htc: HeatTransfer::new::<watt_per_square_meter_kelvin>(500.0),
        }
    }

    /// Refuses an operating point outside what the model can evaluate.
    ///
    /// # Errors
    /// [`TampinesError::InvalidInput`] for non-positive flows, `c_p` or
    /// `h_o`; an inlet pressure outside 0.2–21 MPa (below the critical
    /// 22.064 MPa, so a dome exists); a water inlet that is not subcooled; a
    /// hot inlet at or below the water inlet; or a hot inlet above
    /// 1073.15 K (IF97 Region 2's upper limit, which the water could reach).
    pub fn validate(&self) -> Result<(), TampinesError> {
        let bad = |m: &str| Err(TampinesError::InvalidInput(format!("operating point: {m}")));
        let p = self.water_inlet_pressure.get::<pascal>();
        let t_w = self.water_inlet_temperature.get::<kelvin>();
        let t_h = self.hot_inlet_temperature.get::<kelvin>();
        if !(self.water_mass_flow.get::<kilogram_per_second>() > 0.0
            && self.hot_mass_flow.get::<kilogram_per_second>() > 0.0)
        {
            return bad("both flows must be positive");
        }
        if !(self.hot_specific_heat.get::<joule_per_kilogram_kelvin>() > 0.0
            && self.shell_side_htc.get::<watt_per_square_meter_kelvin>() > 0.0)
        {
            return bad("hot-side c_p and h_o must be positive");
        }
        if !(0.2e6..=21.0e6).contains(&p) {
            return bad("water inlet pressure must lie in 0.2-21 MPa");
        }
        if !(t_w > 273.16 && t_w < sat_temp_4(self.water_inlet_pressure).get::<kelvin>()) {
            return bad("water must enter subcooled (above 273.16 K, below T_sat)");
        }
        if !(t_h > t_w && t_h <= T_MAX_REGION_2) {
            return bad("need T_water,in < T_hot,in <= 1073.15 K");
        }
        Ok(())
    }
}

/// **Which physics terms are on.** All on by default; each `without_*`
/// builder is an explicit, visible ablation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OtsgOptions {
    /// Axial cells along the tube (≥ 2).
    pub nodes: usize,
    pub curvature: bool,
    pub nucleate_boiling: bool,
    pub subcooled_boiling: bool,
    pub dryout: bool,
    pub gravity: bool,
    pub acceleration: bool,
    pub wall_resistance: bool,
}

impl Default for OtsgOptions {
    /// 200 cells and every physics term on.
    fn default() -> Self {
        Self {
            nodes: 200,
            curvature: true,
            nucleate_boiling: true,
            subcooled_boiling: true,
            dryout: true,
            gravity: true,
            acceleration: true,
            wall_resistance: true,
        }
    }
}

impl OtsgOptions {
    /// Same physics, `nodes` axial cells.
    pub fn with_nodes(mut self, nodes: usize) -> Self {
        self.nodes = nodes;
        self
    }
    /// Ablation: evaluate every coil correlation at `d/D_c = 0` (their own
    /// straight limit, which is not exactly the straight-tube correlations;
    /// see the V&V record).
    pub fn without_curvature(mut self) -> Self {
        self.curvature = false;
        self
    }
    /// Ablation: no Forster–Zuber term (`S = 0`) anywhere.
    pub fn without_nucleate_boiling(mut self) -> Self {
        self.nucleate_boiling = false;
        self
    }
    /// Ablation: no nucleate term while the bulk is subcooled.
    pub fn without_subcooled_boiling(mut self) -> Self {
        self.subcooled_boiling = false;
        self
    }
    /// Ablation: no CHF check, Chen up to `x = 1` (its quality is capped at
    /// `1 − 1e-4` inside `X_tt`, where Chen's laminar liquid-alone term would
    /// otherwise diverge).
    pub fn without_dryout(mut self) -> Self {
        self.dryout = false;
        self
    }
    /// Ablation: no hydrostatic term.
    pub fn without_gravity(mut self) -> Self {
        self.gravity = false;
        self
    }
    /// Ablation: no acceleration term.
    pub fn without_acceleration(mut self) -> Self {
        self.acceleration = false;
        self
    }
    /// Ablation: an infinitely conducting tube wall.
    pub fn without_wall_resistance(mut self) -> Self {
        self.wall_resistance = false;
        self
    }
}

/// **The water side's heat-transfer regime at a node.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaterRegime {
    SubcooledLiquid,
    SubcooledBoiling,
    SaturatedBoiling,
    PostDryout,
    SuperheatedVapour,
}

impl WaterRegime {
    /// Short label for tables and the demo.
    pub fn label(self) -> &'static str {
        match self {
            Self::SubcooledLiquid => "subcooled liquid",
            Self::SubcooledBoiling => "subcooled boiling",
            Self::SaturatedBoiling => "saturated boiling",
            Self::PostDryout => "post-dryout",
            Self::SuperheatedVapour => "superheated vapour",
        }
    }
    /// A stable integer code (0–4) for wire formats and CSV.
    pub fn code(self) -> u8 {
        match self {
            Self::SubcooledLiquid => 0,
            Self::SubcooledBoiling => 1,
            Self::SaturatedBoiling => 2,
            Self::PostDryout => 3,
            Self::SuperheatedVapour => 4,
        }
    }
    /// Inverse of [`Self::code`].
    pub fn from_code(c: u8) -> Option<Self> {
        [
            Self::SubcooledLiquid,
            Self::SubcooledBoiling,
            Self::SaturatedBoiling,
            Self::PostDryout,
            Self::SuperheatedVapour,
        ]
        .into_iter()
        .find(|r| r.code() == c)
    }
}

/// One node of the solved profile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProfilePoint {
    /// Distance along the tube from the water inlet.
    pub length: Length,
    /// Height above the water inlet.
    pub height: Length,
    pub water_temperature: ThermodynamicTemperature,
    pub hot_temperature: ThermodynamicTemperature,
    pub inner_wall_temperature: ThermodynamicTemperature,
    pub outer_wall_temperature: ThermodynamicTemperature,
    pub pressure: Pressure,
    /// Equilibrium quality `(h − h_f)/h_fg`; below 0 subcooled, above 1
    /// superheated.
    pub equilibrium_quality: Ratio,
    pub water_enthalpy: AvailableEnergy,
    /// Heat flux at the inner wall.
    pub inner_heat_flux: HeatFluxDensity,
    /// Inner (water-side) heat-transfer coefficient on the regime's driving
    /// difference (`T_w − T_b`, or `T_w − T_sat` when boiling).
    pub inner_htc: HeatTransfer,
    /// `q''/q''_CHF` where the CHF was evaluated (saturated, pre-dryout).
    pub chf_ratio: Option<f64>,
    pub regime: WaterRegime,
}

/// **The steady solution** (or the latest shooting iterate).
#[derive(Clone, Debug, PartialEq)]
pub struct OtsgSolution {
    pub points: Vec<ProfilePoint>,
    /// Heat into the water, all tubes.
    pub duty_water: Power,
    /// Heat out of the hot fluid, all tubes.
    pub duty_hot: Power,
    /// `(duty_water − duty_hot) / duty_hot`.
    pub energy_closure: Ratio,
    pub hot_outlet_temperature: ThermodynamicTemperature,
    /// Hot temperature the march returned at `s = L`; equals the set inlet
    /// when converged.
    pub hot_inlet_returned: ThermodynamicTemperature,
    pub pressure_drop_friction: Pressure,
    pub pressure_drop_gravity: Pressure,
    pub pressure_drop_acceleration: Pressure,
    /// Tube length at the dryout node, if the tube dries out.
    pub dryout_length: Option<Length>,
    pub shooting_iterations: usize,
    pub converged: bool,
}

impl OtsgSolution {
    /// Total water-side pressure drop.
    pub fn pressure_drop(&self) -> Pressure {
        self.pressure_drop_friction + self.pressure_drop_gravity + self.pressure_drop_acceleration
    }
    /// The outlet node.
    pub fn outlet(&self) -> &ProfilePoint {
        // A march always has at least two nodes (OtsgOptions::nodes >= 2).
        &self.points[self.points.len() - 1]
    }
}

/// **The model**: a validated geometry, operating point and option set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HelicalCoilOtsg {
    geometry: HelicalCoilGeometry,
    operating: OperatingPoint,
    options: OtsgOptions,
}

/// Local evaluation at one `(h, p, T_h)` point (strict SI).
#[derive(Clone, Copy, Debug)]
struct Local {
    t_b: f64,
    t_wi: f64,
    t_wo: f64,
    /// Heat per unit tube length into the water \[W/m\].
    q_lin: f64,
    q_flux: f64,
    h_i: f64,
    chf_ratio: Option<f64>,
    x_eq: f64,
    regime: WaterRegime,
    dpds_friction: f64,
    dpds_gravity: f64,
    /// Whether the liquid (or the single phase) and the gas are above the
    /// coil `Re_crit`: the friction law switches there.
    turbulent: (bool, bool),
    /// HEM specific volume \[m³/kg\].
    v: f64,
}

/// One complete march at a guessed hot outlet temperature.
#[derive(Clone, Debug)]
struct March {
    /// `(s, h, p, local)` per node.
    nodes: Vec<(f64, f64, f64, Local)>,
    /// Hot temperature per node.
    t_h: Vec<f64>,
    dp_fric: f64,
    dp_grav: f64,
    dp_acc: f64,
    /// The march stopped early: the hot side passed the set inlet (or IF97's
    /// range), so the guess is too hot. Carries the overshoot \[K\].
    overshoot: Option<f64>,
    /// Tube length of the located dryout point \[m\].
    dryout_s: Option<f64>,
    /// Switches located inside cells.
    events: usize,
}

impl March {
    fn add(&mut self, (fric, grav, acc): (f64, f64, f64)) {
        self.dp_fric += fric;
        self.dp_grav += grav;
        self.dp_acc += acc;
    }
}

/// Result of one Heun step.
#[derive(Clone, Copy, Debug)]
enum Stepped {
    Done { h: f64, p: f64, t_h: f64, fric: f64, grav: f64, acc: f64 },
    Overshoot(f64),
}

impl HelicalCoilOtsg {
    /// Build and validate.
    ///
    /// # Errors
    /// [`TampinesError::InvalidInput`] from either `validate`, or for fewer
    /// than 2 cells.
    pub fn new(
        geometry: HelicalCoilGeometry,
        operating: OperatingPoint,
        options: OtsgOptions,
    ) -> Result<Self, TampinesError> {
        geometry.validate()?;
        operating.validate()?;
        if options.nodes < 2 {
            return Err(TampinesError::InvalidInput("need at least 2 cells".into()));
        }
        Ok(Self {
            geometry,
            operating,
            options,
        })
    }

    pub fn geometry(&self) -> HelicalCoilGeometry {
        self.geometry
    }
    pub fn operating(&self) -> OperatingPoint {
        self.operating
    }
    pub fn options(&self) -> OtsgOptions {
        self.options
    }

    /// Solve to convergence ([`Shooting`] stepped to the end).
    ///
    /// # Errors
    /// Whatever a march raises (an IF97 state out of range, a pressure
    /// exhausted by the pressure drop), or [`TampinesError::Numerical`] if
    /// shooting does not converge in [`SHOOTING_MAX_ITERATIONS`].
    pub fn solve(&self) -> Result<OtsgSolution, TampinesError> {
        let mut s = self.shooting();
        while !s.step()? {}
        s.solution()
            .ok_or_else(|| TampinesError::Numerical("shooting produced no march".into()))
    }

    /// A stepper over the shooting iterations; [`Shooting::step`] runs one
    /// march.
    pub fn shooting(&self) -> Shooting {
        Shooting {
            model: *self,
            lo: None,
            hi: None,
            last: None,
            last_side: 0,
            iterations: 0,
            converged: false,
        }
    }

    // ── strict-SI internals ────────────────────────────────────────────

    fn r(&self) -> f64 {
        if self.options.curvature {
            self.geometry.curvature_ratio().get::<ratio>()
        } else {
            0.0
        }
    }
    fn d_i(&self) -> f64 {
        self.geometry.tube_inner_diameter.get::<meter>()
    }
    fn n_tubes(&self) -> f64 {
        f64::from(self.geometry.parallel_tubes)
    }
    fn m_w(&self) -> f64 {
        self.operating.water_mass_flow.get::<kilogram_per_second>() / self.n_tubes()
    }
    fn m_h_cp(&self) -> f64 {
        self.operating.hot_mass_flow.get::<kilogram_per_second>() / self.n_tubes()
            * self.operating.hot_specific_heat.get::<joule_per_kilogram_kelvin>()
    }
    fn mass_flux(&self) -> f64 {
        let d = self.d_i();
        self.m_w() / (0.25 * std::f64::consts::PI * d * d)
    }
    /// Outer film plus wall resistance per unit length \[m·K/W\].
    fn r_ext(&self) -> f64 {
        let d_o = self.geometry.tube_outer_diameter.get::<meter>();
        let h_o = self.operating.shell_side_htc.get::<watt_per_square_meter_kelvin>();
        let r_o = 1.0 / (h_o * std::f64::consts::PI * d_o);
        let r_w = if self.options.wall_resistance {
            (d_o / self.d_i()).r_ln() / (2.0 * std::f64::consts::PI
                * self.geometry.wall_conductivity.get::<watt_per_meter_kelvin>())
        } else {
            0.0
        };
        r_o + r_w
    }
    fn h_in(&self) -> f64 {
        h_tp_eqm_single_phase(
            self.operating.water_inlet_temperature,
            self.operating.water_inlet_pressure,
        )
        .get::<joule_per_kilogram>()
    }

    /// Single-phase coil friction gradient \[Pa/m\] for flux `g` of a phase of
    /// density `rho`, viscosity `mu`.
    fn friction_single(&self, g: f64, rho: f64, mu: f64) -> f64 {
        let d = self.d_i();
        let re = g * d / mu;
        if re <= 0.0 {
            return 0.0;
        }
        corr::darcy_friction_coil(re, self.r()) * g * g / (2.0 * rho * d)
    }

    /// Inner-wall temperature for a linear inner law `q'' = h (T_w − T_ref)`.
    fn wall_linear(&self, t_h: f64, h: f64, t_ref: f64) -> f64 {
        let a = std::f64::consts::PI * self.d_i() * h;
        let r = self.r_ext();
        (t_h / r + a * t_ref) / (1.0 / r + a)
    }

    /// Inner-wall temperature for `q''(T_w) = h_c (T_w − T_ref) + nuc(T_w)`,
    /// `nuc` the nucleate term, zero up to `T_sat` and increasing above it.
    /// Bisection between `T_sat` and the convective-only root, which bound
    /// the root because adding a positive increasing term can only lower it.
    fn wall_boiling(
        &self,
        t_h: f64,
        h_c: f64,
        t_ref: f64,
        t_sat: f64,
        nuc: &impl Fn(f64) -> f64,
    ) -> f64 {
        let t_lin = self.wall_linear(t_h, h_c, t_ref);
        if t_lin <= t_sat {
            return t_lin;
        }
        let r = self.r_ext();
        let pd = std::f64::consts::PI * self.d_i();
        let resid = |tw: f64| (t_h - tw) / r - pd * (h_c * (tw - t_ref) + nuc(tw));
        let (mut lo, mut hi) = (t_sat, t_lin);
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            if resid(mid) > 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
            if hi - lo < 1e-10 {
                break;
            }
        }
        0.5 * (lo + hi)
    }

    /// Evaluate the water side at `(h, p)` against hot temperature `t_h`.
    /// `post` selects the post-dryout coefficient for a saturated state (the
    /// march decides where dryout is; this never switches on its own).
    fn evaluate(&self, h: f64, p: f64, t_h: f64, post: bool) -> Result<Local, TampinesError> {
        let d = self.d_i();
        let g = self.mass_flux();
        let r = self.r();
        let sat = SaturatedProperties::at(p)?;
        let tr = SaturatedTransport::at(p)?;
        let p_q = Pressure::new::<pascal>(p);
        let h_q = AvailableEnergy::new::<joule_per_kilogram>(h);
        let h_fg = sat.h_g - sat.h_f;
        let x_eq = (h - sat.h_f) / h_fg;
        let v = v_ph_eqm(p_q, h_q).get::<cubic_meter_per_kilogram>();
        let gravity = if self.options.gravity {
            G_STD * self.geometry.elevation_sine() / v
        } else {
            0.0
        };
        let sigma = || {
            water_surf_tension(ThermodynamicTemperature::new::<kelvin>(sat.t_sat))
                .get::<joule_per_square_meter>()
        };
        // Forster–Zuber at wall temperature tw, saturated-liquid properties.
        let fz = |tw: f64, sigma: f64| {
            let tp = tw.min(T_PSAT_CEILING);
            let dp = sat_pressure_4(ThermodynamicTemperature::new::<kelvin>(tp)).get::<pascal>() - p;
            corr::forster_zuber_htc(
                tr.lambda_f, tr.cp_f, sat.rho_f, sigma, sat.mu_f, h_fg, sat.rho_g, tw - sat.t_sat, dp,
            )
        };
        let single_phase = |regime: WaterRegime| -> Result<(f64, f64, f64, f64, f64), TampinesError> {
            let t_b = t_ph_eqm(p_q, h_q).get::<kelvin>();
            let mu = mu_ph_eqm(p_q, h_q).get::<pascal_second>();
            let k = lambda_ph_eqm(p_q, h_q).get::<watt_per_meter_kelvin>();
            let cp = cp_ph_eqm(p_q, h_q).get::<joule_per_kilogram_kelvin>();
            if !(mu > 0.0 && k > 0.0 && cp > 0.0) {
                return Err(TampinesError::Unphysical(format!(
                    "IF97 transport at p = {p} Pa, h = {h} J/kg ({}): mu {mu}, k {k}, cp {cp}",
                    regime.label()
                )));
            }
            let re = g * d / mu;
            let h_c = corr::nusselt_coil(re, cp * mu / k, r) * k / d;
            Ok((t_b, mu, h_c, re, 1.0 / v))
        };

        let re_crit = corr::critical_reynolds_schmidt(r);
        let (t_b, t_wi, h_i, chf_ratio, regime, friction, turbulent);
        if x_eq <= 0.0 {
            let (tb, mu, h_c, re, rho) = single_phase(WaterRegime::SubcooledLiquid)?;
            t_b = tb;
            turbulent = (re > re_crit, false);
            friction = self.friction_single(g, rho, mu);
            let nucleate = self.options.nucleate_boiling && self.options.subcooled_boiling;
            if nucleate && t_h > sat.t_sat {
                let s = corr::chen_s(re);
                let sg = sigma();
                let nuc = |tw: f64| s * fz(tw, sg) * (tw - sat.t_sat);
                t_wi = self.wall_boiling(t_h, h_c, t_b, sat.t_sat, &nuc);
                let q = h_c * (t_wi - t_b) + nuc(t_wi);
                h_i = q / (t_wi - t_b);
                regime = if t_wi > sat.t_sat {
                    WaterRegime::SubcooledBoiling
                } else {
                    WaterRegime::SubcooledLiquid
                };
            } else {
                t_wi = self.wall_linear(t_h, h_c, t_b);
                h_i = h_c;
                regime = WaterRegime::SubcooledLiquid;
            }
            chf_ratio = None;
        } else if x_eq < 1.0 {
            t_b = sat.t_sat;
            let x_c = if self.options.dryout { x_eq } else { x_eq.min(1.0 - 1e-4) };
            // Liquid-alone convection on the coil, and the gas-alone flow.
            let re_l = g * (1.0 - x_c) * d / sat.mu_f;
            let pr_l = tr.cp_f * sat.mu_f / tr.lambda_f;
            let h_l = corr::nusselt_coil(re_l, pr_l, r) * tr.lambda_f / d;
            let f = corr::chen_f(corr::martinelli_xtt(x_c, sat.rho_f, sat.rho_g, sat.mu_f, sat.mu_g));
            let s = if self.options.nucleate_boiling {
                corr::chen_s(re_l * f.r_powf(1.25))
            } else {
                0.0
            };
            let re_g = g * x_eq * d / sat.mu_g;
            turbulent = (re_l > re_crit, re_g > re_crit);
            let dp_l = self.friction_single(g * (1.0 - x_eq), sat.rho_f, sat.mu_f);
            let dp_g = self.friction_single(g * x_eq, sat.rho_g, sat.mu_g);
            friction = corr::two_phase_friction_gradient(
                dp_l,
                dp_g,
                corr::chisholm_c(re_l > re_crit, re_g > re_crit),
            );
            let post_dryout = |t_hot: f64| -> Result<(f64, f64), TampinesError> {
                let pr_g = tr.cp_g * sat.mu_g / tr.lambda_g;
                let cond = PostDryoutConditions {
                    mass_flux: MassFlux::new::<kilogram_per_square_meter_second>(g),
                    quality: Ratio::new::<ratio>(x_eq),
                    hydraulic_diameter: Length::new::<meter>(d),
                    wall_temperature: ThermodynamicTemperature::new::<kelvin>(t_hot),
                    vapor: VaporProperties {
                        density: MassDensity::new::<kilogram_per_cubic_meter>(sat.rho_g),
                        liquid_density: MassDensity::new::<kilogram_per_cubic_meter>(sat.rho_f),
                        dynamic_viscosity: DynamicViscosity::new::<pascal_second>(sat.mu_g),
                        thermal_conductivity: ThermalConductivity::new::<watt_per_meter_kelvin>(
                            tr.lambda_g,
                        ),
                        prandtl: Ratio::new::<ratio>(pr_g),
                    },
                };
                let h = PostDryoutModel::FilmBoilingDougallRohsenow
                    .heat_transfer_coefficient(&cond)?
                    .get::<watt_per_square_meter_kelvin>();
                Ok((self.wall_linear(t_hot, h, sat.t_sat), h))
            };
            if self.options.dryout && post {
                let (tw, h) = post_dryout(t_h)?;
                t_wi = tw;
                h_i = h;
                chf_ratio = None;
                regime = WaterRegime::PostDryout;
            } else {
                let sg = sigma();
                let nuc = |tw: f64| s * fz(tw, sg) * (tw - sat.t_sat);
                let tw = self.wall_boiling(t_h, f * h_l, sat.t_sat, sat.t_sat, &nuc);
                let q = f * h_l * (tw - sat.t_sat) + nuc(tw);
                chf_ratio = if self.options.dryout {
                    let q_chf = Biasi::new()
                        .critical_heat_flux(
                            p_q,
                            MassFlux::new::<kilogram_per_square_meter_second>(g),
                            x_eq,
                            Length::new::<meter>(d),
                        )?
                        .get::<watt_per_square_meter>();
                    Some(q / q_chf.max(f64::MIN_POSITIVE))
                } else {
                    None
                };
                t_wi = tw;
                h_i = if (tw - sat.t_sat).abs() > 0.0 { q / (tw - sat.t_sat) } else { f * h_l };
                regime = WaterRegime::SaturatedBoiling;
            }
        } else {
            let (tb, mu, h_c, re, rho) = single_phase(WaterRegime::SuperheatedVapour)?;
            t_b = tb;
            turbulent = (re > re_crit, false);
            friction = self.friction_single(g, rho, mu);
            t_wi = self.wall_linear(t_h, h_c, t_b);
            h_i = h_c;
            chf_ratio = None;
            regime = WaterRegime::SuperheatedVapour;
        }
        let q_lin = (t_h - t_wi) / self.r_ext();
        let d_o = self.geometry.tube_outer_diameter.get::<meter>();
        let h_o = self.operating.shell_side_htc.get::<watt_per_square_meter_kelvin>();
        let t_wo = t_h - q_lin / (h_o * std::f64::consts::PI * d_o);
        Ok(Local {
            t_b,
            t_wi,
            t_wo,
            q_lin,
            q_flux: q_lin / (std::f64::consts::PI * d),
            h_i,
            chf_ratio,
            x_eq,
            regime,
            dpds_friction: friction,
            dpds_gravity: gravity,
            turbulent,
            v,
        })
    }

    /// One Heun step of length `ds` from `(h, p, t_h)` with start-point
    /// evaluation `k1`, in the pre- or post-dryout coefficient (`post`).
    #[allow(clippy::too_many_arguments)]
    fn heun(
        &self,
        h: f64,
        p: f64,
        t_h: f64,
        k1: &Local,
        ds: f64,
        post: bool,
        s_end: f64,
    ) -> Result<Stepped, TampinesError> {
        let (m_w, m_h_cp) = (self.m_w(), self.m_h_cp());
        let g = self.mass_flux();
        let t_h_in = self.operating.hot_inlet_temperature.get::<kelvin>();
        let p_floor = 0.05 * self.operating.water_inlet_pressure.get::<pascal>();
        let too_hot = |t: f64| t > T_MAX_REGION_2 || t > t_h_in + 50.0;
        let exhausted = |pp: f64| {
            TampinesError::Unphysical(format!(
                "the water-side pressure drop exhausts the inlet pressure ({pp:.0} Pa at s = {s_end:.2} m)"
            ))
        };
        if ds <= 0.0 {
            return Ok(Stepped::Done { h, p, t_h, fric: 0.0, grav: 0.0, acc: 0.0 });
        }
        let h_pred = h + ds * k1.q_lin / m_w;
        let th_pred = t_h + ds * k1.q_lin / m_h_cp;
        let p_pred = p - ds * (k1.dpds_friction + k1.dpds_gravity);
        if too_hot(th_pred) {
            return Ok(Stepped::Overshoot(th_pred - t_h_in));
        }
        if p_pred < p_floor {
            return Err(exhausted(p_pred));
        }
        let k2 = self.evaluate(h_pred, p_pred, th_pred, post)?;
        let q = 0.5 * (k1.q_lin + k2.q_lin);
        let fric = 0.5 * (k1.dpds_friction + k2.dpds_friction) * ds;
        let grav = 0.5 * (k1.dpds_gravity + k2.dpds_gravity) * ds;
        let h_new = h + ds * q / m_w;
        let th_new = t_h + ds * q / m_h_cp;
        let mut acc = 0.0;
        if self.options.acceleration {
            // Two passes: v at the end depends on the end pressure.
            for _ in 0..2 {
                let v_end = v_ph_eqm(
                    Pressure::new::<pascal>(p - fric - grav - acc),
                    AvailableEnergy::new::<joule_per_kilogram>(h_new),
                )
                .get::<cubic_meter_per_kilogram>();
                acc = g * g * (v_end - k1.v);
            }
        }
        let p_new = p - fric - grav - acc;
        if too_hot(th_new) {
            return Ok(Stepped::Overshoot(th_new - t_h_in));
        }
        if p_new < p_floor {
            return Err(exhausted(p_new));
        }
        Ok(Stepped::Done { h: h_new, p: p_new, t_h: th_new, fric, grav, acc })
    }

    /// March the water side from the inlet with hot outlet `t_out` \[K\].
    ///
    /// **Every switch in the closures is located inside its cell**, not
    /// snapped to a node: a change of regime (including dryout, where the
    /// pre-dryout `q''/q''_CHF` reaches 1), or of a friction law across the
    /// coil `Re_crit`. When a trial step ends on the other side of one, the
    /// switch point is found by bisection on the sub-step length (to
    /// `1e-10` of a cell) and the cell is integrated in Heun sub-steps
    /// between switches. A switch snapped to a node makes the march's
    /// returned hot temperature jump as the guess moves it across a node,
    /// so shooting can only close to the size of the jump (found on the
    /// first runs: energy closure 1.4e-5 against a 1e-8 prediction, and a
    /// grid study that did not converge; V&V record §2).
    fn march(&self, t_out: f64) -> Result<March, TampinesError> {
        let n = self.options.nodes;
        let l = self.geometry.tube_length().get::<meter>();
        let ds = l / n as f64;
        let p_in = self.operating.water_inlet_pressure.get::<pascal>();
        let (mut h, mut p, mut t_h) = (self.h_in(), p_in, t_out);
        let mut latched = false;
        let mut out = March {
            nodes: Vec::with_capacity(n + 1),
            t_h: Vec::with_capacity(n + 1),
            dp_fric: 0.0,
            dp_grav: 0.0,
            dp_acc: 0.0,
            overshoot: None,
            dryout_s: None,
            events: 0,
        };
        let mut here = self.evaluate(h, p, t_h, false)?;
        let dryout_on = self.options.dryout;
        for i in 0..=n {
            out.nodes.push((i as f64 * ds, h, p, here));
            out.t_h.push(t_h);
            if i == n {
                break;
            }
            let mut s_cur = i as f64 * ds;
            let mut rem = ds;
            for event in 0..=MAX_EVENTS_PER_CELL {
                let start = here;
                let switched = |e: &Local| {
                    e.regime != start.regime
                        || e.turbulent != start.turbulent
                        || (!latched && dryout_on && e.chf_ratio.is_some_and(|r| r >= 1.0))
                };
                let (h1, p1, t1, inc) = match self.heun(h, p, t_h, &start, rem, latched, s_cur + rem)? {
                    Stepped::Overshoot(o) => {
                        out.overshoot = Some(o);
                        return Ok(out);
                    }
                    Stepped::Done { h, p, t_h, fric, grav, acc } => (h, p, t_h, (fric, grav, acc)),
                };
                let end = self.evaluate(h1, p1, t1, latched)?;
                if !switched(&end) || event == MAX_EVENTS_PER_CELL {
                    (h, p, t_h) = (h1, p1, t1);
                    out.add(inc);
                    here = end;
                    break;
                }
                // Bisect for the switch: lo is before it, hi after.
                let (mut lo, mut hi) = (0.0, 1.0);
                let mut after = (h1, p1, t1, inc, end);
                while (hi - lo) * rem > 1e-10 * ds {
                    let mid = 0.5 * (lo + hi);
                    match self.heun(h, p, t_h, &start, mid * rem, latched, s_cur + mid * rem)? {
                        Stepped::Overshoot(_) => hi = mid,
                        Stepped::Done { h: hm, p: pm, t_h: tm, fric, grav, acc } => {
                            let e = self.evaluate(hm, pm, tm, latched)?;
                            if switched(&e) {
                                hi = mid;
                                after = (hm, pm, tm, (fric, grav, acc), e);
                            } else {
                                lo = mid;
                            }
                        }
                    }
                }
                let (ha, pa, ta, inc_a, e_a) = after;
                (h, p, t_h) = (ha, pa, ta);
                out.add(inc_a);
                out.events += 1;
                s_cur += hi * rem;
                rem -= hi * rem;
                here = if !latched && dryout_on && e_a.chf_ratio.is_some_and(|r| r >= 1.0) {
                    latched = true;
                    out.dryout_s = Some(s_cur);
                    self.evaluate(h, p, t_h, true)?
                } else {
                    e_a
                };
                if rem <= 1e-12 * ds {
                    break;
                }
            }
        }
        Ok(out)
    }

    /// Package a march (complete or not) as a dimensioned solution.
    fn package(&self, m: &March, iterations: usize, converged: bool) -> OtsgSolution {
        let sin = self.geometry.elevation_sine();
        let points: Vec<ProfilePoint> = m
            .nodes
            .iter()
            .zip(&m.t_h)
            .map(|(&(s, h, p, loc), &t_h)| ProfilePoint {
                length: Length::new::<meter>(s),
                height: Length::new::<meter>(s * sin),
                water_temperature: ThermodynamicTemperature::new::<kelvin>(loc.t_b),
                hot_temperature: ThermodynamicTemperature::new::<kelvin>(t_h),
                inner_wall_temperature: ThermodynamicTemperature::new::<kelvin>(loc.t_wi),
                outer_wall_temperature: ThermodynamicTemperature::new::<kelvin>(loc.t_wo),
                pressure: Pressure::new::<pascal>(p),
                equilibrium_quality: Ratio::new::<ratio>(loc.x_eq),
                water_enthalpy: AvailableEnergy::new::<joule_per_kilogram>(h),
                inner_heat_flux: HeatFluxDensity::new::<watt_per_square_meter>(loc.q_flux),
                inner_htc: HeatTransfer::new::<watt_per_square_meter_kelvin>(loc.h_i),
                chf_ratio: loc.chf_ratio,
                regime: loc.regime,
            })
            .collect();
        let n = self.n_tubes();
        let first = &m.nodes[0];
        let last = &m.nodes[m.nodes.len() - 1];
        let t_out = m.t_h[0];
        let t_ret = m.t_h[m.t_h.len() - 1];
        let duty_w = n * self.m_w() * (last.1 - first.1);
        // The hot side's duty against the SET inlet, so the closure includes
        // the shooting residual.
        let duty_h =
            n * self.m_h_cp() * (self.operating.hot_inlet_temperature.get::<kelvin>() - t_out);
        OtsgSolution {
            dryout_length: m.dryout_s.map(Length::new::<meter>),
            points,
            duty_water: Power::new::<watt>(duty_w),
            duty_hot: Power::new::<watt>(duty_h),
            energy_closure: Ratio::new::<ratio>(if duty_h != 0.0 {
                (duty_w - duty_h) / duty_h
            } else {
                0.0
            }),
            hot_outlet_temperature: ThermodynamicTemperature::new::<kelvin>(t_out),
            hot_inlet_returned: ThermodynamicTemperature::new::<kelvin>(t_ret),
            pressure_drop_friction: Pressure::new::<pascal>(m.dp_fric),
            pressure_drop_gravity: Pressure::new::<pascal>(m.dp_grav),
            pressure_drop_acceleration: Pressure::new::<pascal>(m.dp_acc),
            shooting_iterations: iterations,
            converged,
        }
    }
}

/// **Shooting on the hot outlet temperature**, one march per
/// [`Shooting::step`].
///
/// The unknown is `T_h(0)`, the hot outlet. Its residual
/// `F = T_h(L) − T_h,in` rises with the guess. The bracket starts at
/// `[T_w,in, T_h,in]` (one march each) and closes by Illinois regula falsi.
/// A march whose hot side passes `T_h,in + 50 K` (or 1073.15 K) stops early
/// and counts as `F > 0` with the overshoot as its value, so a too-hot guess
/// never asks IF97 for a state beyond Region 2.
#[derive(Clone, Debug)]
pub struct Shooting {
    model: HelicalCoilOtsg,
    lo: Option<(f64, f64)>,
    hi: Option<(f64, f64)>,
    last: Option<March>,
    /// Which end Illinois moved last (−1 lo, +1 hi), for its halving.
    last_side: i8,
    iterations: usize,
    converged: bool,
}

impl Shooting {
    fn residual(&self, m: &March) -> f64 {
        let t_h_in = self.model.operating.hot_inlet_temperature.get::<kelvin>();
        match m.overshoot {
            Some(o) => o.max(1e-3),
            None => m.t_h[m.t_h.len() - 1] - t_h_in,
        }
    }

    /// Run one march. Returns `Ok(true)` once converged (further calls do
    /// nothing).
    ///
    /// # Errors
    /// A march's error, or [`TampinesError::Numerical`] when the bracket does
    /// not hold a sign change or [`SHOOTING_MAX_ITERATIONS`] is reached.
    pub fn step(&mut self) -> Result<bool, TampinesError> {
        if self.converged {
            return Ok(true);
        }
        if self.iterations >= SHOOTING_MAX_ITERATIONS {
            return Err(TampinesError::Numerical(format!(
                "shooting did not converge in {SHOOTING_MAX_ITERATIONS} marches"
            )));
        }
        let t_w_in = self.model.operating.water_inlet_temperature.get::<kelvin>();
        let t_h_in = self.model.operating.hot_inlet_temperature.get::<kelvin>();
        let guess = match (self.lo, self.hi) {
            (None, _) => t_w_in,
            (Some(_), None) => t_h_in,
            (Some((a, fa)), Some((b, fb))) => {
                let t = b - fb * (b - a) / (fb - fa);
                if t > a && t < b { t } else { 0.5 * (a + b) }
            }
        };
        let m = self.model.march(guess)?;
        self.iterations += 1;
        let f = self.residual(&m);
        let complete = m.overshoot.is_none();
        self.last = Some(m);
        match (self.lo, self.hi) {
            (None, _) => {
                if f >= 0.0 {
                    return Err(TampinesError::Numerical(format!(
                        "shooting: a hot outlet at the water inlet temperature already returns {f:+.3e} K at the hot inlet"
                    )));
                }
                self.lo = Some((guess, f));
            }
            (Some(_), None) => {
                if f <= 0.0 {
                    return Err(TampinesError::Numerical(format!(
                        "shooting: a hot outlet at the hot inlet temperature returns {f:+.3e} K"
                    )));
                }
                self.hi = Some((guess, f));
            }
            (Some(lo), Some(hi)) => {
                // Illinois: when the same end moves twice running, halve the
                // stored residual of the end that stayed.
                if f < 0.0 {
                    self.lo = Some((guess, f));
                    if self.last_side == -1 {
                        self.hi = Some((hi.0, 0.5 * hi.1));
                    }
                    self.last_side = -1;
                } else {
                    self.hi = Some((guess, f));
                    if self.last_side == 1 {
                        self.lo = Some((lo.0, 0.5 * lo.1));
                    }
                    self.last_side = 1;
                }
                let width = self.hi.map_or(0.0, |h| h.0) - self.lo.map_or(0.0, |l| l.0);
                if complete && (f.abs() < SHOOTING_TOLERANCE_K || width < 1e-11) {
                    self.converged = true;
                }
            }
        }
        Ok(self.converged)
    }

    /// Marches run so far.
    pub fn iterations(&self) -> usize {
        self.iterations
    }

    /// Converged yet.
    pub fn converged(&self) -> bool {
        self.converged
    }

    /// The latest march as a solution (`converged` says whether it is the
    /// answer). `None` before the first step.
    pub fn solution(&self) -> Option<OtsgSolution> {
        self.last
            .as_ref()
            .map(|m| self.model.package(m, self.iterations, self.converged))
    }
}
