// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! The HTR-10 R-Z core **in time** (gh:#1151 part B): the gh:#1149 solid
//! field marched by backward Euler, the gh:#1150 porous gas network solved
//! quasi-steadily at every step with buoyancy in the bed and channels, and
//! a lagged fuel-zone node per heated bed cell.
//!
//! **Original work, not a port.** The coupling is THERMIX's (Chen et al.
//! 2009 [C09] Sec 3.3, input sheet `docs/reactor-scoping/htr10-th-benchmark.md`):
//! "a quasi-stationary gas flow model ... coupling with a given
//! time-dependent temperature profile of the solid structures, this module
//! solves steady-state continuity, momentum and energy equations of gas".
//! The gas holds a few tens of kJ/K against the bed's ~8 MJ/K, so its own
//! storage is dropped.
//!
//! ## One step `dt`
//!
//! 1. **Gas** ([`solve_gas`]): the flow and the gas temperatures for the
//!    solid temperatures at the start of the step (explicit in the gas, as
//!    THERMIX). With the loop stopped (`mass_flow = 0`) every inflow is zero
//!    and only gravity drives the gas: the **in-vessel natural circulation**
//!    of [C09] Sec 5 in the bed, the discharge tube, the bottom-reflector
//!    borings and the control-rod channels. In the plena and cavities (open
//!    volumes) the gravity term stays dropped (gh:#1150: it gave a
//!    Rayleigh-Darcy limit cycle in the hot plenum), so the plena carry the
//!    circulation only as low-resistance connections. The 1-D inlet chain
//!    (annulus, risers, top plenum) carries only the loop flow, so **no
//!    riser loop** exists here; see the V&V README for its size.
//!    **The steady state (gh:#1176, 2026-11-03)** solves the gas
//!    consistently in its own density
//!    ([`super::flow::solve_gas_consistent`], Aitken-relaxed to 1e-4 K):
//!    with the densities lagged one outer pass the steady iteration
//!    wandered at 3-9 K from the axis cells of the discharge tube on the
//!    4x-refined 20 cm mesh and never converged. **A step keeps the lagged
//!    densities** (explicit in the gas, as THERMIX): a consistent solve per
//!    step was tried and needed 15-87 evaluations per step, and failed to
//!    converge at the last forced-flow step of the coastdown on the 2x mesh
//!    (2026-11-03). Known limitation: on the 4x mesh at a constant state
//!    the tube's axis gas alternates by 50-120 K from step to step (V&V
//!    README Sec 10).
//! 2. **Solid** (`SolidConductionField::step`): backward Euler with
//!    Picard on `lambda(T)`, `rho c(T)` and radiation; the gas exchange
//!    `h a_v (T - T_gas)` implicit in `T`.
//! 3. **Fuel zone** (per heated bed cell, the [`FuelZoneModel::Lagged`]
//!    default): a node of the fuel zones' heat capacity receiving the
//!    power and conducting to the cell (the pebble surface temperature of
//!    gh:#1150) through `G = 1/(R_zone + R_shell)` per fuel pebble,
//!    `R_zone = 1/(20 pi k r_1)` (volume mean of a uniformly heated sphere
//!    to its surface) and `R_shell = (1/r_1 - 1/r_2)/(4 pi k)`, `r_1 = 2.5`,
//!    `r_2 = 3.0 cm` (Tab 2.2), `k` the Tab 2.9 fuel-element fit at the
//!    fuel temperature (zero dose, the gh:#1150 reading). The node is
//!    eliminated **exactly** into the solid row: with `a = C_f/dt`,
//!    `h' = G a/(a + G)` to the reference `T* = T_f^n + q/a`. Its capacity
//!    is the fraction `phi = x_fuel (r_1/r_2)^3` of the bed's `rho c`, and
//!    the bed cell keeps `1 - phi` (`set_cell_capacity_factor`; `1 - phi -
//!    phi_d` with the dummy node of item 4), so nothing is counted twice. `x_fuel` is the as-run fuel-pebble fraction of the
//!    mixed zone, 13,651 / (13,651 + 10,298) ([J09] Tab 2; sheet Sec 6,
//!    Derived 57:43).
//!
//! 4. **Dummy pebbles** (gh:#1174, [`DummyPebbleModel::Lagged`], default):
//!    the mixed zone holds 43 % dummies, all-graphite balls that generate
//!    nothing and heat only through the bed around them. Each heated cell
//!    gets a dummy node of capacity `phi_d = 1 - x_fuel` of the bed's
//!    `rho c` (the cell keeps the fuel pebbles' shells, `x (1 - (r_1/r_2)^3)`),
//!    coupled to the cell by [`dummy_pebble_conductance`] per dummy:
//!    the sphere's mean-to-surface resistance `1/(20 pi k R)` in series with
//!    `2 pi D k_bed + h A` (a sphere in a medium of the bed's effective
//!    conductivity, plus the gas film of one pebble, both referenced to the
//!    cell). Eliminated into the cell row exactly as the fuel node
//!    (`h' = G a/(a+G)` to `T_d^n`), enthalpy-form update, counted in
//!    [`Htr10RzTransientCore::stored_energy`]. Stated approximations: the
//!    bed's conduction network runs through the cell, not the dummies; the
//!    gas film is referenced to the cell, not the gas (a lower bound on the
//!    lag in forced flow). Hand time constant ~45 s in forced flow, ~100 s
//!    stagnant. Ablation `DUMMY-IN-CELL`.
//!
//! ## Feedback temperatures (kinetics input)
//!
//! [T1] Sec 2.1.1.6 defines each coefficient as the change for a 1 deg C
//! change of that temperature "within the whole core", split over zones by
//! power fraction. Here: **fuel** = power-weighted fuel-zone node
//! temperature; **moderator** = power-weighted ~~bed-cell temperature
//! (pebble graphite outside the fuel zones)~~ **(gh:#1174, 2026-11-03)**
//! volume mean of all pebble graphite in the cell,
//! `w_f T_f + w_s T_cell + w_d T_d` with the fuel zones' matrix
//! `w_f = x (r_1/r_2)^3 = 0.330`, the fuel pebbles' shells
//! `w_s = x (1 - (r_1/r_2)^3) = 0.240` and the dummies `w_d = 1 - x = 0.430`
//! ([`ModeratorTemperature::AllPebbleGraphite`]): the moderator is the
//! graphite, and a third of it is the fuel zones' matrix, which the part B
//! reading left out. [T1] defines the moderator coefficient only as
//! "similar" to the fuel's, so which graphite it reads is an **assumption**
//! (the all-graphite reading is VSOP's fuel/moderator split, not checked
//! against a source); `MOD-OUTSIDE-FZ` is the part B reading, kept as a
//! sensitivity. **reflector** = volume mean of
//! all graphite reflector cells ([`ReflectorAverage::AllGraphite`], a named
//! assumption: any weighting reproduces the uniform-change definition;
//! `SideOnly` is part A's side-reflector node, kept as a sensitivity).
//!
//! > Research, education and V&V only. Not for licensing, operation or
//! > safety decisions. AI draft, unreviewed.

use petir::mathf::RealMath;
use uom::si::energy::joule;
use uom::si::f64::{
    Energy, Length, MassRate, Power, Pressure, ThermodynamicTemperature, Time, VolumetricPowerDensity,
};
use uom::si::length::centimeter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::{bar, pascal};
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::volumetric_power_density::watt_per_cubic_meter;

use super::flow::{gas, solve_gas, solve_gas_consistent, BypassSplit, GAS_DENSITY_TOLERANCE_K, GasNetwork, GasState, OperatingPoint, GRAVITY};
use super::fpic::{reading_at, Thermocouple, ThermocoupleReading};
use super::geometry::idx;
use super::pebble::tab29_k;
use super::power_map::{cell_average_w_per_m3, crw_power_map, fpic_power_map, lofc_power_map, map_power_w, PowerMapCell};
use super::{Htr10RzCase, Htr10RzError, Htr10RzModel, Htr10RzOptions, RegionMaterial};
use crate::genfoam::thermal_hydraulics::solid_conduction::{
    ConductivityModel, HeatCapacityModel, RzPatch, VolumetricHeatTransferCoefficient,
};

const PI: f64 = std::f64::consts::PI;
/// Fuel-zone and pebble radii, m (TECDOC-1694 Tab 2.2).
const R_FUEL_ZONE: f64 = 0.025;
const R_PEBBLE: f64 = 0.030;
/// Bed packing fraction (Tab 2.2: porosity 0.39).
const PACKING: f64 = 0.61;
/// As-run fuel pebbles and the dummies mixed with them ([J09] Tab 2;
/// input sheet Sec 6, Derived 13,651 x 43/57).
pub const AS_RUN_FUEL_PEBBLES: f64 = 13_651.0;
/// See [`AS_RUN_FUEL_PEBBLES`].
pub const AS_RUN_MIXED_DUMMIES: f64 = 10_298.0;

/// Which TECDOC-1694 power-density map shapes the power.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerShape {
    /// Tab 2.6, the 10 MW FPIC map.
    FpicTab26,
    /// Tab 2.56, the 3 MW LOFC map.
    LofcTab256,
    /// Tab 2.61, the 3 MW CRW map (one cell differs from Tab 2.56, N1).
    CrwTab261,
}

impl PowerShape {
    /// The map.
    #[must_use]
    pub fn map(self) -> Vec<PowerMapCell> {
        match self {
            Self::FpicTab26 => fpic_power_map(),
            Self::LofcTab256 => lofc_power_map(),
            Self::CrwTab261 => crw_power_map(),
        }
    }
}

/// How the fuel zones of the pebbles respond in time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FuelZoneModel {
    /// A fuel-zone node with its own heat capacity (module docs). Default.
    #[default]
    Lagged,
    /// No fuel-zone storage: `T_f = T_s + q/G` at every instant, all the
    /// capacity in the bed cell (the explicit ablation `FUEL-QS`).
    QuasiSteady,
}

/// Which graphite the reflector feedback temperature averages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReflectorAverage {
    /// Volume mean of every graphite reflector region (side, top, bottom,
    /// the channel rings and the throttles). Default.
    #[default]
    AllGraphite,
    /// The side reflector `r 90-167.5 cm`, `z -65..325 cm`: part A's lumped
    /// reflector node (sensitivity `REFL-SIDE`).
    SideOnly,
}

/// How the dummy (all-graphite) pebbles mixed with the fuel in the heated
/// zone respond in time (gh:#1174; module docs, "Two-temperature bed").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DummyPebbleModel {
    /// A dummy-pebble node per heated bed cell with its own heat capacity,
    /// coupled to the bed cell through the dummy's interior and the bed
    /// around it ([`dummy_pebble_conductance`]). Default (gh:#1174).
    #[default]
    Lagged,
    /// The dummies are part of the bed cell's capacity, at the cell's
    /// temperature (the gh:#1151 part B model; ablation `DUMMY-IN-CELL`).
    InBedCell,
}

/// Which pebble graphite the **moderator** temperature coefficient reads
/// (gh:#1174; module docs, "Feedback temperatures").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModeratorTemperature {
    /// All pebble graphite of a heated cell, weighted by volume: the fuel
    /// zones (matrix, at the fuel-zone node), the fuel pebbles' shells (the
    /// bed cell) and the dummies. Default (gh:#1174).
    #[default]
    AllPebbleGraphite,
    /// The pebble graphite outside the fuel zones only (shells and
    /// dummies): the gh:#1151 part B reading (sensitivity `MOD-OUTSIDE-FZ`).
    OutsideFuelZones,
    /// All pebble graphite, each part weighted by its volume **and its
    /// measured reactivity importance per unit graphite** (gh:#1174): the
    /// fuel pebbles' graphite (fuel-zone matrix and shell, one importance)
    /// against the dummies', in the ratio
    /// [`TransientCoreOptions::fuel_graphite_importance`] from the whole-core
    /// Monte Carlo worths (V&V README Sec 11). With a ratio of 1 it is
    /// [`Self::AllPebbleGraphite`].
    ImportanceWeighted,
}

/// Everything that defines the transient core, each defaulting to the
/// gh:#1151 part B base case.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransientCoreOptions {
    /// The solid model (mesh resolution, bed conductivity, heads, RCCS).
    /// Its helium pressure is overwritten by the operating point's.
    pub solid: Htr10RzOptions,
    /// The bypass split (D5: an assumption, never tuned).
    pub split: BypassSplit,
    /// The power shape.
    pub shape: PowerShape,
    /// Fuel pebbles over all pebbles in the heated (mixed) zone.
    pub fuel_pebble_fraction: f64,
    /// Fuel-zone node.
    pub fuel_zone: FuelZoneModel,
    /// Gravity in the porous bed and channels (in-vessel natural
    /// circulation). Default ON; `false` is the ablation `NC-OFF`.
    pub buoyancy: bool,
    /// Reflector feedback temperature.
    pub reflector: ReflectorAverage,
    /// Dummy pebbles in the heated zone (gh:#1174).
    pub dummy_pebbles: DummyPebbleModel,
    /// What the moderator coefficient reads (gh:#1174).
    pub moderator: ModeratorTemperature,
    /// Reactivity worth per kelvin per unit graphite of the fuel pebbles'
    /// graphite over the dummies' ([`ModeratorTemperature::ImportanceWeighted`];
    /// gh:#1174, measured by Monte Carlo, V&V README Sec 11).
    pub fuel_graphite_importance: f64,
    /// Outer-iteration tolerance of the initial steady state, K.
    pub steady_tolerance_k: f64,
}

impl TransientCoreOptions {
    /// The LOFC base: Tab 2.56 shape, as-run fuel fraction, [T1] split,
    /// 20 cm cells (see the V&V README's mesh study), Breitbach bed (D3).
    #[must_use]
    pub fn htr10_lofc() -> Self {
        Self {
            solid: Htr10RzOptions {
                max_cell: Some(Length::new::<centimeter>(20.0)),
                ..Htr10RzOptions::default()
            },
            split: BypassSplit::tecdoc_1694(),
            shape: PowerShape::LofcTab256,
            fuel_pebble_fraction: AS_RUN_FUEL_PEBBLES / (AS_RUN_FUEL_PEBBLES + AS_RUN_MIXED_DUMMIES),
            fuel_zone: FuelZoneModel::Lagged,
            buoyancy: true,
            reflector: ReflectorAverage::AllGraphite,
            dummy_pebbles: DummyPebbleModel::default(),
            moderator: ModeratorTemperature::default(),
            fuel_graphite_importance: 1.0,
            steady_tolerance_k: 1.0e-4,
        }
    }

    /// The CRW base: as [`Self::htr10_lofc`] with the Tab 2.61 shape.
    #[must_use]
    pub fn htr10_crw() -> Self {
        Self { shape: PowerShape::CrwTab261, ..Self::htr10_lofc() }
    }

    /// The ablation `NC-OFF`: no gravity term anywhere in the gas.
    #[must_use]
    pub fn without_buoyancy(mut self) -> Self {
        self.buoyancy = false;
        self
    }

    /// The ablation `FUEL-QS`: no fuel-zone storage.
    #[must_use]
    pub fn with_quasi_steady_fuel(mut self) -> Self {
        self.fuel_zone = FuelZoneModel::QuasiSteady;
        self
    }

    /// The ablation `DUMMY-IN-CELL`: the dummies at the bed cell's
    /// temperature, no dummy-pebble node (gh:#1174).
    #[must_use]
    pub fn without_dummy_pebble_lag(mut self) -> Self {
        self.dummy_pebbles = DummyPebbleModel::InBedCell;
        self
    }

    /// The sensitivity `MOD-OUTSIDE-FZ`: the moderator coefficient reads the
    /// pebble graphite outside the fuel zones only (gh:#1174).
    #[must_use]
    pub fn with_moderator_outside_fuel_zones(mut self) -> Self {
        self.moderator = ModeratorTemperature::OutsideFuelZones;
        self
    }

    /// The moderator reading weighted by the measured importance of the
    /// fuel pebbles' graphite against the dummies' (`ratio`; gh:#1174).
    #[must_use]
    pub fn with_importance_weighted_moderator(mut self, ratio: f64) -> Self {
        self.moderator = ModeratorTemperature::ImportanceWeighted;
        self.fuel_graphite_importance = ratio;
        self
    }

    /// The ablation of the importance weighting: all pebble graphite by
    /// volume (the gh:#1174 reading of 2026-11-03).
    #[must_use]
    pub fn with_all_pebble_graphite_moderator(mut self) -> Self {
        self.moderator = ModeratorTemperature::AllPebbleGraphite;
        self
    }

    /// Change the largest cell size.
    #[must_use]
    pub fn with_max_cell(mut self, max_cell: Length) -> Self {
        self.solid.max_cell = Some(max_cell);
        self
    }

    /// Split every cell of the mesh uniformly, `radial x axial`
    /// ([`Htr10RzOptions::refinement`]; the gh:#1176 geometric mesh
    /// sequence).
    #[must_use]
    pub fn with_refinement(mut self, radial: usize, axial: usize) -> Self {
        self.solid = self.solid.with_refinement(radial, axial);
        self
    }
}

/// Power-weighted fuel and moderator temperatures and the reflector's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoreFeedbackTemperatures {
    /// Power-weighted fuel-zone temperature.
    pub fuel: ThermodynamicTemperature,
    /// Power-weighted bed (pebble graphite) temperature.
    pub moderator: ThermodynamicTemperature,
    /// Reflector temperature ([`ReflectorAverage`]).
    pub reflector: ThermodynamicTemperature,
}

/// In-vessel natural-circulation measures of the last gas solve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NaturalCirculation {
    /// Upward mass flow across the bed row nearest `z = 90 cm` (sum over
    /// cells with upward `G_z`).
    pub bed_upflow: MassRate,
    /// Downward mass flow across the same row.
    pub bed_downflow: MassRate,
    /// Largest superficial mass flux magnitude in the bed, kg/(m^2 s).
    pub max_bed_mass_flux: f64,
    /// Net heat the gas carries into the core-entry plenum and the top
    /// reflector flow zone through the bed top (`z = 0`), W: positive
    /// upward, `c_p sum F (T - T_ref)` over the faces, `T_ref` the area-mean
    /// of the two sides (so a zero net mass flow carries no reference
    /// enthalpy).
    pub enthalpy_up_through_bed_top: Power,
}

/// What a step (or the steady state) returns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoreStepReport {
    /// Mixed core outlet (hot plenum) gas; the area mean of the outlet
    /// cells when the loop is stopped.
    pub outlet_temperature: ThermodynamicTemperature,
    /// Heat taken up by the gas from the solid, `sum h a_v V (T_s - T_g)`
    /// with the end-of-step solid temperatures.
    pub heat_to_gas: Power,
    /// Heat leaving through the RPV side wall and both heads.
    pub heat_to_rccs_and_heads: Power,
    /// Power deposited this step.
    pub power: Power,
    /// Feedback temperatures at the end of the step.
    pub feedback: CoreFeedbackTemperatures,
    /// Natural-circulation measures.
    pub circulation: NaturalCirculation,
    /// Picard passes of the solid solve.
    pub solid_picard_iterations: usize,
    /// Picard passes of the gas flow solve.
    pub gas_picard_iterations: usize,
}

/// One pass of the steady solid-gas outer iteration
/// ([`Htr10RzTransientCore::solve_steady`]), kept so a stall can be told
/// apart from an oscillation or a divergence (gh:#1176).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SteadyOuterPass {
    /// Largest change of a solid cell temperature over the pass, K.
    pub solid_change_k: f64,
    /// Largest change of a gas cell temperature over the pass, K
    /// (infinite on the first pass).
    pub gas_change_k: f64,
    /// The cell that set the larger of the two.
    pub worst_cell: usize,
    /// Picard passes of the solid solve.
    pub solid_picard: usize,
    /// Picard passes of the gas flow solve.
    pub gas_picard: usize,
}

/// The transient HTR-10 R-Z core.
#[derive(Debug, Clone)]
pub struct Htr10RzTransientCore {
    solid: Htr10RzModel,
    network: GasNetwork,
    gas: Option<GasState>,
    options: TransientCoreOptions,
    /// Bed-volumetric power per W of total power, W/m^3/W.
    unit_source: Vec<f64>,
    heated: Vec<usize>,
    reflector_cells: Vec<usize>,
    /// Fuel-zone node temperature per cell, K (heated cells only).
    fuel: Vec<f64>,
    bed_capacity: HeatCapacityModel,
    /// Fuel pebbles per m^3 of bed.
    fuel_pebbles_per_m3: f64,
    /// Fraction of a heated cell's capacity carried by its fuel node.
    phi: f64,
    /// Dummy-pebble node temperature per cell, K (heated cells only, and
    /// only with [`DummyPebbleModel::Lagged`]).
    dummy: Vec<f64>,
    /// Fraction of a heated cell's capacity carried by its dummy node.
    phi_d: f64,
    /// Dummy pebbles per m^3 of bed.
    dummies_per_m3: f64,
    /// Effective conductivity of the bed (the solid model's bed closure).
    bed_conductivity: ConductivityModel,
    power_w: f64,
    time_s: f64,
    /// Passes of the last steady outer iteration.
    steady_history: Vec<SteadyOuterPass>,
}

fn hv(v: f64) -> VolumetricHeatTransferCoefficient {
    VolumetricHeatTransferCoefficient {
        dimension: std::marker::PhantomData,
        units: std::marker::PhantomData,
        value: v,
    }
}

fn is_graphite_reflector(region: usize) -> bool {
    use idx::*;
    matches!(
        region,
        SIDE_GRAPHITE
            | TOP_BOTTOM_GRAPHITE
            | CR_RING
            | RISERS
            | SIDE_INNER_GRAPHITE
            | UPPER_BOTTOM_REFL
            | LOWER_BOTTOM_REFL
            | BOTTOM_REFL_SOLID
            | TOP_FLOW_ZONE
            | DISCHARGE_THROTTLE
            | CR_EXIT_THROTTLE
    )
}

/// Fuel-zone-to-pebble-surface conductance of one fuel pebble, W/K, with
/// the zone and the shell at conductivity `k` (module docs).
#[must_use]
pub fn fuel_pebble_conductance(k: f64) -> f64 {
    let r_zone = 1.0 / (20.0 * PI * k * R_FUEL_ZONE);
    let r_shell = (1.0 / R_FUEL_ZONE - 1.0 / R_PEBBLE) / (4.0 * PI * k);
    1.0 / (r_zone + r_shell)
}

/// Conductance of one dummy pebble between its mean temperature and the bed
/// around it, W/K (gh:#1174): `1 / (R_int + 1/G_ext)`, with
///
/// - `R_int = 1/(20 pi k_g R)`, the mean-to-surface resistance of a sphere
///   whose temperature profile is the quasi-steady parabola (the same form
///   as the fuel zone's, here for a heat sink spread over the sphere);
/// - `G_ext = 2 pi D k_bed + h A`: a sphere of diameter `D` in an infinite
///   medium of the bed's effective conductivity (conduction shape factor
///   `S = 2 pi D`, Incropera & DeWitt 6th ed. Tab 4.1 case 1) in parallel
///   with the gas film `h A` of one pebble, referenced to the bed cell.
///
/// `k_g` is the pebble graphite (`k_graphite`, W/(m K)), `k_bed` the bed
/// (W/(m K)), `h_a` the per-pebble gas film conductance (W/K).
#[must_use]
pub fn dummy_pebble_conductance(k_graphite: f64, k_bed: f64, h_a: f64) -> f64 {
    let r_int = 1.0 / (20.0 * PI * k_graphite * R_PEBBLE);
    let g_ext = 2.0 * PI * 2.0 * R_PEBBLE * k_bed + h_a;
    1.0 / (r_int + 1.0 / g_ext)
}

/// Volume fractions of the pebble graphite of a heated cell: fuel zones,
/// fuel-pebble shells, dummies, for fuel-pebble fraction `x`.
fn graphite_fractions(x: f64) -> (f64, f64, f64) {
    let zone = (R_FUEL_ZONE / R_PEBBLE).r_powi(3);
    (x * zone, x * (1.0 - zone), 1.0 - x)
}

/// Bed-volumetric power per W of total power, W/m^3/W, of every cell of
/// `mesh` for the power-density map `map` (exact axisymmetric overlap,
/// [`cell_average_w_per_m3`]); zero outside the bed region. The one
/// computation [`Htr10RzTransientCore::build`] and
/// [`Htr10RzTransientCore::set_power_map`] share, so a custom map equal to a
/// tabulated one gives the same source bit for bit.
///
/// # Errors
/// [`Htr10RzError::Input`] when the map carries no power (`map_power_w <=
/// 0`) or a negative density.
fn unit_source_of(
    mesh: &crate::genfoam::thermal_hydraulics::solid_conduction::RzMesh,
    map: &[PowerMapCell],
) -> Result<Vec<f64>, Htr10RzError> {
    if map.iter().any(|c| !(c.q_w_per_cm3 >= 0.0)) {
        return Err(Htr10RzError::Input("a power map density is negative or not a number".into()));
    }
    let total = map_power_w(map);
    if !(total > 0.0) {
        return Err(Htr10RzError::Input(format!("the power map integrates to {total} W")));
    }
    let per_watt = 1.0 / total;
    let (re, ze) = (mesh.r_edges().clone(), mesh.z_edges().clone());
    let nr = re.n_cells();
    Ok((0..mesh.n_cells())
        .map(|c| {
            if mesh.cell_region(c) != idx::BED {
                return 0.0;
            }
            let (i, j) = (c % nr, c / nr);
            per_watt
                * cell_average_w_per_m3(
                    map,
                    [re.edge(i).get::<centimeter>(), re.edge(i + 1).get::<centimeter>()],
                    [ze.edge(j).get::<centimeter>(), ze.edge(j + 1).get::<centimeter>()],
                )
        })
        .collect())
}

impl Htr10RzTransientCore {
    /// Build the solid model, the gas network and the fuel nodes.
    ///
    /// # Errors
    /// As [`Htr10RzModel::build`] and [`GasNetwork::build`];
    /// [`Htr10RzError::Input`] for a fuel fraction outside `(0, 1]`.
    pub fn build(options: TransientCoreOptions) -> Result<Self, Htr10RzError> {
        let x = options.fuel_pebble_fraction;
        if !(x > 0.0 && x <= 1.0) {
            return Err(Htr10RzError::Input(format!("fuel pebble fraction {x} must lie in (0, 1]")));
        }
        let solid = Htr10RzModel::build(
            options.solid,
            Htr10RzCase::DecayPower { power: Power::new::<watt>(1.0) },
        )?;
        let mut network = GasNetwork::build(solid.field().mesh(), options.split)?;
        if !options.buoyancy {
            network = network.without_buoyancy();
        }
        let mesh = solid.field().mesh();
        let n = mesh.n_cells();
        let unit_source = unit_source_of(mesh, &options.shape.map())?;
        let heated: Vec<usize> = (0..n).filter(|c| unit_source[*c] > 0.0).collect();
        let reflector_cells: Vec<usize> = (0..n)
            .filter(|c| {
                let reg = mesh.cell_region(*c);
                match options.reflector {
                    ReflectorAverage::AllGraphite => is_graphite_reflector(reg),
                    ReflectorAverage::SideOnly => {
                        let (r, z) = mesh.cell_centre(*c);
                        let (r, z) = (r.get::<centimeter>(), z.get::<centimeter>());
                        matches!(reg, idx::SIDE_GRAPHITE | idx::SIDE_INNER_GRAPHITE | idx::CR_RING | idx::RISERS)
                            && r < 167.5
                            && (-65.0..=325.0).contains(&z)
                    }
                }
            })
            .collect();
        let mut mopts = options.solid.materials;
        mopts.helium_pressure_bar = 30.0;
        let bed_capacity = RegionMaterial::PebbleBed.heat_capacity_model(&mopts)?;
        let v_peb = 4.0 / 3.0 * PI * R_PEBBLE.r_powi(3);
        let phi = match options.fuel_zone {
            FuelZoneModel::Lagged => x * (R_FUEL_ZONE / R_PEBBLE).r_powi(3),
            FuelZoneModel::QuasiSteady => 0.0,
        };
        let phi_d = match options.dummy_pebbles {
            DummyPebbleModel::Lagged => 1.0 - x,
            DummyPebbleModel::InBedCell => 0.0,
        };
        let bed_conductivity = RegionMaterial::PebbleBed.conductivity_model(&options.solid.materials)?;
        let mut me = Self {
            solid,
            network,
            gas: None,
            options,
            unit_source,
            heated,
            reflector_cells,
            fuel: vec![f64::NAN; n],
            bed_capacity,
            fuel_pebbles_per_m3: x * PACKING / v_peb,
            phi,
            dummy: vec![f64::NAN; n],
            phi_d,
            dummies_per_m3: (1.0 - x) * PACKING / v_peb,
            bed_conductivity,
            power_w: 0.0,
            time_s: 0.0,
            steady_history: Vec::new(),
        };
        if phi + phi_d > 0.0 {
            let mut factor = vec![1.0; n];
            for &c in &me.heated {
                factor[c] = 1.0 - phi - phi_d;
            }
            me.solid
                .field_mut()
                .set_cell_capacity_factor(&factor)
                .map_err(|e| Htr10RzError::Solve(e.to_string()))?;
        }
        Ok(me)
    }

    /// The solid model.
    #[must_use]
    pub fn solid(&self) -> &Htr10RzModel {
        &self.solid
    }

    /// **Build with a custom power-density map** (gh:#1185): as
    /// [`Self::build`], then [`Self::set_power_map`]. The options' `shape`
    /// still decides which bed cells are heated (the fuel and dummy nodes
    /// live on those), so pass a shape whose heated zone covers the custom
    /// map; every TECDOC map heats the whole bed (core and cone).
    ///
    /// # Errors
    /// As [`Self::build`] and [`Self::set_power_map`].
    pub fn build_with_power_map(options: TransientCoreOptions, map: &[PowerMapCell]) -> Result<Self, Htr10RzError> {
        let mut core = Self::build(options)?;
        core.set_power_map(map)?;
        Ok(core)
    }

    /// **Replace the power shape with a custom map** (gh:#1185), e.g. a Monte
    /// Carlo power tally mapped into TECDOC `z` (`nee_soon::power_shape`).
    /// `map` is cells of `[z_lo, z_hi] x [r_lo, r_hi]` cm (TECDOC `z`,
    /// downward) with densities in W/cm^3, the layout of
    /// [`super::power_map::fpic_power_map`]; only its **shape** is used, the
    /// total power stays the core's own (`step`/`solve_steady` set it). The
    /// mapping is the build's own (exact axisymmetric overlap), so a map
    /// equal to `FpicTab26`'s gives the tabulated source bit for bit.
    ///
    /// Callable between solves (a Picard iteration); the temperatures are
    /// kept.
    ///
    /// # Errors
    /// [`Htr10RzError::Input`] when the map carries no power, a negative
    /// density, or power on a cell outside the heated set the core was built
    /// with ([`Self::heated_cells`]); nothing is changed then.
    pub fn set_power_map(&mut self, map: &[PowerMapCell]) -> Result<(), Htr10RzError> {
        let unit = unit_source_of(self.solid.field().mesh(), map)?;
        let mut heated = vec![false; unit.len()];
        for &c in &self.heated {
            heated[c] = true;
        }
        if let Some(c) = (0..unit.len()).find(|&c| unit[c] > 0.0 && !heated[c]) {
            return Err(Htr10RzError::Input(format!(
                "the power map heats cell {c}, outside the heated bed cells the core was built with"
            )));
        }
        let vol = &self.solid.field().mesh().mesh().cell_volumes;
        if !(self.heated.iter().map(|&c| unit[c] * vol[c]).sum::<f64>() > 0.0) {
            return Err(Htr10RzError::Input("the power map puts no power on the heated bed cells".into()));
        }
        self.unit_source = unit;
        Ok(())
    }

    /// Share of the power map's integral that lands on the heated bed cells,
    /// `sum_c q_c V_c / P_map`: `1` for a map drawn on the bed cells, and
    /// `1.000000000` for TECDOC Tab 2.6 on the coarse mesh (measured
    /// 2026-11-03, `custom_power_map_equal_to_fpic_reproduces_the_tabulated_case`).
    /// Power a map puts outside the heated cells is not deposited.
    #[must_use]
    pub fn captured_power_fraction(&self) -> f64 {
        let vol = &self.solid.field().mesh().mesh().cell_volumes;
        self.heated.iter().map(|&c| self.unit_source[c] * vol[c]).sum()
    }

    /// The cell power density per watt of total power, W/m^3/W, of every
    /// cell (zero outside the heated cells): the current shape.
    #[must_use]
    pub fn unit_source(&self) -> &[f64] {
        &self.unit_source
    }

    /// The passes of the last steady outer iteration (gh:#1176).
    #[must_use]
    pub fn steady_history(&self) -> &[SteadyOuterPass] {
        &self.steady_history
    }

    /// The gas network.
    #[must_use]
    pub fn network(&self) -> &GasNetwork {
        &self.network
    }

    /// The gas state of the last solve.
    #[must_use]
    pub fn gas(&self) -> Option<&GasState> {
        self.gas.as_ref()
    }

    /// The options.
    #[must_use]
    pub fn options(&self) -> &TransientCoreOptions {
        &self.options
    }

    /// Time marched since the steady state, s.
    #[must_use]
    pub fn time(&self) -> Time {
        Time::new::<second>(self.time_s)
    }

    /// Cells that generate power.
    #[must_use]
    pub fn heated_cells(&self) -> &[usize] {
        &self.heated
    }

    /// The bed-volumetric power density of cell `c` at the current power.
    #[must_use]
    pub fn cell_power_density(&self, c: usize) -> VolumetricPowerDensity {
        VolumetricPowerDensity::new::<watt_per_cubic_meter>(self.unit_source[c] * self.power_w)
    }

    /// Fuel-zone temperature of cell `c` (`None` for an unheated cell).
    #[must_use]
    pub fn fuel_temperature(&self, c: usize) -> Option<ThermodynamicTemperature> {
        let t = self.fuel[c];
        t.is_finite().then(|| ThermodynamicTemperature::new::<kelvin>(t))
    }

    /// Dummy-pebble node temperature of cell `c` (`None` for an unheated
    /// cell or with [`DummyPebbleModel::InBedCell`]).
    #[must_use]
    pub fn dummy_temperature(&self, c: usize) -> Option<ThermodynamicTemperature> {
        let t = self.dummy[c];
        t.is_finite().then(|| ThermodynamicTemperature::new::<kelvin>(t))
    }

    /// Dummy conductance per m^3 of bed, W/(m^3 K), for dummy temperature
    /// `t_d`, bed cell temperature `t_cell` and gas exchange `h a_v`
    /// (W/(m^3 K)) of the cell.
    fn g_dummy(&self, t_d: f64, t_cell: f64, hv_gas: f64) -> Result<f64, Htr10RzError> {
        let k_bed = self.bed_conductivity.0.value(t_cell);
        let v_peb = 4.0 / 3.0 * PI * R_PEBBLE.r_powi(3);
        let h_a = hv_gas.max(0.0) * v_peb / PACKING;
        Ok(self.dummies_per_m3 * dummy_pebble_conductance(tab29_k(t_d)?, k_bed, h_a))
    }

    fn t_solid(&self) -> Vec<f64> {
        let f = self.solid.field();
        (0..f.mesh().n_cells()).map(|c| f.temperature(c).get::<kelvin>()).collect()
    }

    /// Fuel conductance per m^3 of bed at fuel temperature `t`, W/(m^3 K).
    fn g_fuel(&self, t: f64) -> Result<f64, Htr10RzError> {
        Ok(self.fuel_pebbles_per_m3 * fuel_pebble_conductance(tab29_k(t)?))
    }

    fn set_source(&mut self, steady_or_quasi: bool) -> Result<(), Htr10RzError> {
        let p = self.power_w;
        let src: Vec<VolumetricPowerDensity> = self
            .unit_source
            .iter()
            .map(|q| VolumetricPowerDensity::new::<watt_per_cubic_meter>(if steady_or_quasi { q * p } else { 0.0 }))
            .collect();
        self.solid
            .field_mut()
            .set_cell_heat_source(&src)
            .map_err(|e| Htr10RzError::Solve(e.to_string()))
    }

    /// `T_f = T_s + q / G(T_f)`, iterated on `k(T_f)`.
    fn quasi_steady_fuel(&mut self) -> Result<(), Htr10RzError> {
        let ts = self.t_solid();
        for i in 0..self.heated.len() {
            let c = self.heated[i];
            let q = self.unit_source[c] * self.power_w;
            let mut tf = ts[c];
            for _ in 0..50 {
                let new = ts[c] + q / self.g_fuel(tf)?;
                let done = (new - tf).abs() < 1e-9;
                tf = new;
                if done {
                    break;
                }
            }
            self.fuel[c] = tf;
        }
        Ok(())
    }

    fn exchange(&self, g: &GasState, fuel: &[(usize, f64, f64)]) -> Vec<(VolumetricHeatTransferCoefficient, ThermodynamicTemperature)> {
        let n = self.fuel.len();
        let mut ex: Vec<(f64, f64)> = (0..n)
            .map(|c| {
                let h = g.exchange[c];
                (h, if h > 0.0 { g.temperature[c] } else { 300.0 })
            })
            .collect();
        for &(c, h2, t2) in fuel {
            let (h1, t1) = ex[c];
            let h = h1 + h2;
            ex[c] = (h, (h1 * t1 + h2 * t2) / h);
        }
        ex.into_iter()
            .map(|(h, t)| (hv(h), ThermodynamicTemperature::new::<kelvin>(t)))
            .collect()
    }

    /// Solve the coupled steady state at `op` (block Gauss-Seidel as the
    /// gh:#1150 FPIC case: gas, then solid with the gas exchange implicit,
    /// 0.5 under-relaxation of the gas temperature), then put the fuel
    /// nodes at their steady values.
    ///
    /// # Errors
    /// [`Htr10RzError::Solve`] if a sub-solve fails or 400 passes do not
    /// converge; [`Htr10RzError::Input`] for a non-positive power.
    pub fn solve_steady(&mut self, op: &OperatingPoint) -> Result<CoreStepReport, Htr10RzError> {
        self.solve_steady_with_progress(op, |_| {})
    }

    /// [`Self::solve_steady`], calling `progress` after every outer pass
    /// (gh:#1176: to watch a slow or stalling outer iteration live).
    ///
    /// # Errors
    /// As [`Self::solve_steady`].
    pub fn solve_steady_with_progress(
        &mut self,
        op: &OperatingPoint,
        mut progress: impl FnMut(&SteadyOuterPass),
    ) -> Result<CoreStepReport, Htr10RzError> {
        let p = op.power.get::<watt>();
        if !(p > 0.0) || !p.is_finite() {
            return Err(Htr10RzError::Input("steady power must be positive".into()));
        }
        self.power_w = p;
        self.set_source(true)?;
        let mut t_s = self.t_solid();
        let mut last = None;
        self.steady_history.clear();
        let mut prev_change = f64::INFINITY;
        for _ in 0..400 {
            // Inexact inner solves (gh:#1176): the gas is made consistent in
            // its density to a tenth of the last outer change, never looser
            // than needed and tightening to GAS_DENSITY_TOLERANCE_K; the
            // converged state is checked at that tolerance below.
            let inner_tol = GAS_DENSITY_TOLERANCE_K.max(0.1 * prev_change);
            let mut g = solve_gas_consistent(&self.network, op, &t_s, self.gas.as_ref(), inner_tol)?;
            if let Some(old) = &self.gas {
                for (a, b) in g.temperature.iter_mut().zip(&old.temperature) {
                    if a.is_finite() && b.is_finite() {
                        *a = 0.5 * *a + 0.5 * b;
                    }
                }
            }
            let ex = self.exchange(&g, &[]);
            self.solid.field_mut().set_cell_exchange(&ex).map_err(|e| Htr10RzError::Solve(e.to_string()))?;
            let rep = self.solid.field_mut().solve_steady().map_err(|e| Htr10RzError::Solve(e.to_string()))?;
            let t_new = self.t_solid();
            let (mut solid_change, mut worst) = (0.0_f64, 0usize);
            for (c, (a, b)) in t_new.iter().zip(&t_s).enumerate() {
                if (a - b).abs() > solid_change {
                    (solid_change, worst) = ((a - b).abs(), c);
                }
            }
            let mut gas_change = 0.0_f64;
            let mut gas_worst = 0usize;
            match &self.gas {
                Some(old) => {
                    for (c, (a, b)) in g.temperature.iter().zip(&old.temperature).enumerate() {
                        if a.is_finite() && b.is_finite() && (a - b).abs() > gas_change {
                            (gas_change, gas_worst) = ((a - b).abs(), c);
                        }
                    }
                }
                None => gas_change = f64::INFINITY,
            }
            let change = solid_change.max(gas_change);
            prev_change = change;
            if gas_change > solid_change {
                worst = gas_worst;
            }
            t_s = t_new;
            let gp = g.flow_picard.0;
            self.steady_history.push(SteadyOuterPass {
                solid_change_k: solid_change,
                gas_change_k: gas_change,
                worst_cell: worst,
                solid_picard: rep.iterations,
                gas_picard: gp,
            });
            progress(self.steady_history.last().expect("just pushed"));
            self.gas = Some(g);
            last = Some((rep.iterations, gp));
            if change < self.options.steady_tolerance_k {
                let g = solve_gas_consistent(&self.network, op, &t_s, self.gas.as_ref(), GAS_DENSITY_TOLERANCE_K)?;
                self.gas = Some(g);
                self.quasi_steady_fuel()?;
                if self.phi_d > 0.0 {
                    // No source in a dummy: at steady state it sits at the
                    // temperature of the bed around it.
                    for &c in &self.heated {
                        self.dummy[c] = t_s[c];
                    }
                }
                self.time_s = 0.0;
                let (si, gi) = last.unwrap_or((0, 0));
                return Ok(self.report(si, gi));
            }
        }
        Err(Htr10RzError::Solve(format!(
            "steady outer iteration did not converge in 400 passes (last {last:?})"
        )))
    }

    /// Advance by `dt` with `op.power` deposited and the loop state of
    /// `op` (flow, inlet temperature, pressure); see the module docs.
    ///
    /// # Errors
    /// [`Htr10RzError::Input`] for a non-positive `dt` or a negative
    /// power or flow; sub-solve errors.
    pub fn step(&mut self, dt: Time, op: &OperatingPoint) -> Result<CoreStepReport, Htr10RzError> {
        let h = dt.get::<second>();
        let p = op.power.get::<watt>();
        if !(h > 0.0) || !h.is_finite() || !(p >= 0.0) || !p.is_finite() {
            return Err(Htr10RzError::Input(format!("step dt {h} s, power {p} W")));
        }
        self.power_w = p;
        let t_s = self.t_solid();
        // Explicit in the gas, densities lagged one step (THERMIX's
        // coupling; gh:#1176 kept it: see the module docs, step 1).
        let g = solve_gas(&self.network, op, &t_s, self.gas.as_ref())?;
        let gp = g.flow_picard.0;
        let lagged = self.phi > 0.0;
        let mut fuel_ex = Vec::new();
        let mut fuel_coef = Vec::new();
        if lagged {
            for &c in &self.heated {
                let tf = self.fuel[c];
                let a = self.phi * self.bed_capacity.0.value(tf) / h;
                let gf = self.g_fuel(tf)?;
                let q = self.unit_source[c] * p;
                fuel_ex.push((c, gf * a / (a + gf), tf + q / a));
                fuel_coef.push((a, gf, q));
            }
        }
        let mut dummy_coef = Vec::new();
        if self.phi_d > 0.0 {
            for &c in &self.heated {
                let td = self.dummy[c];
                let a = self.phi_d * self.bed_capacity.0.value(td) / h;
                let gd = self.g_dummy(td, t_s[c], g.exchange[c])?;
                fuel_ex.push((c, gd * a / (a + gd), td));
                dummy_coef.push((a, gd));
            }
        }
        self.set_source(!lagged)?;
        let ex = self.exchange(&g, &fuel_ex);
        self.solid.field_mut().set_cell_exchange(&ex).map_err(|e| Htr10RzError::Solve(e.to_string()))?;
        let rep = self.solid.field_mut().step(dt).map_err(|e| Htr10RzError::Solve(e.to_string()))?;
        self.gas = Some(g);
        if self.phi_d > 0.0 {
            let ts = self.t_solid();
            for (k, &c) in self.heated.iter().enumerate() {
                let (a, gd) = dummy_coef[k];
                let t0 = self.dummy[c];
                let t_lin = (a * t0 + gd * ts[c]) / (a + gd);
                self.dummy[c] = self.enthalpy_update(t0, t_lin);
            }
        }
        if lagged {
            let ts = self.t_solid();
            for (k, &c) in self.heated.iter().enumerate() {
                let (a, gf, q) = fuel_coef[k];
                let t0 = self.fuel[c];
                let t_lin = (a * t0 + q + gf * ts[c]) / (a + gf);
                // Enthalpy form (as the solid field's storage): the node
                // received c(T0)(T_lin - T0) per unit of its capacity, so
                // its temperature is the one whose exact enthalpy rise is
                // that, and the stored energy is conserved exactly.
                let m = &self.bed_capacity.0;
                let dh = m.value(t0) * (t_lin - t0);
                let mut t = t_lin;
                for _ in 0..20 {
                    let r = m.integral(t0, t) - dh;
                    let step = r / m.value(t);
                    t -= step;
                    if step.abs() < 1e-10 {
                        break;
                    }
                }
                self.fuel[c] = t;
            }
        } else {
            self.quasi_steady_fuel()?;
        }
        self.time_s += h;
        Ok(self.report(rep.iterations, gp))
    }

    /// The node temperature whose exact enthalpy rise from `t0` equals
    /// `c(t0) (t_lin - t0)` (the solid field's enthalpy form, so a node
    /// conserves energy exactly).
    fn enthalpy_update(&self, t0: f64, t_lin: f64) -> f64 {
        let m = &self.bed_capacity.0;
        let dh = m.value(t0) * (t_lin - t0);
        let mut t = t_lin;
        for _ in 0..20 {
            let r = m.integral(t0, t) - dh;
            let step = r / m.value(t);
            t -= step;
            if step.abs() < 1e-10 {
                break;
            }
        }
        t
    }

    /// Thermal energy above `reference` in the solid, the fuel nodes and the
    /// dummy nodes.
    #[must_use]
    pub fn stored_energy(&self, reference: ThermodynamicTemperature) -> Energy {
        let mut e = self.solid.field().thermal_energy(reference).get::<joule>();
        if self.phi > 0.0 {
            let fv = self.solid.field().mesh().mesh();
            let tr = reference.get::<kelvin>();
            for &c in &self.heated {
                e += self.phi * fv.cell_volumes[c] * self.bed_capacity.0.integral(tr, self.fuel[c]);
            }
        }
        if self.phi_d > 0.0 {
            let fv = self.solid.field().mesh().mesh();
            let tr = reference.get::<kelvin>();
            for &c in &self.heated {
                e += self.phi_d * fv.cell_volumes[c] * self.bed_capacity.0.integral(tr, self.dummy[c]);
            }
        }
        Energy::new::<joule>(e)
    }

    /// The feedback temperatures now.
    #[must_use]
    pub fn feedback(&self) -> CoreFeedbackTemperatures {
        let f = self.solid.field();
        let vol = &f.mesh().mesh().cell_volumes;
        let (gz, gs, gd) = graphite_fractions(self.options.fuel_pebble_fraction);
        let (mut wf, mut wm, mut w) = (0.0, 0.0, 0.0);
        for &c in &self.heated {
            let q = self.unit_source[c] * vol[c];
            let tc = f.temperature(c).get::<kelvin>();
            let td = if self.phi_d > 0.0 { self.dummy[c] } else { tc };
            let tm = match self.options.moderator {
                ModeratorTemperature::OutsideFuelZones => (gs * tc + gd * td) / (gs + gd),
                ModeratorTemperature::AllPebbleGraphite => gz * self.fuel[c] + gs * tc + gd * td,
                ModeratorTemperature::ImportanceWeighted => {
                    let rho = self.options.fuel_graphite_importance;
                    (rho * (gz * self.fuel[c] + gs * tc) + gd * td) / (rho * (gz + gs) + gd)
                }
            };
            wf += q * self.fuel[c];
            wm += q * tm;
            w += q;
        }
        let (mut vr, mut v) = (0.0, 0.0);
        for &c in &self.reflector_cells {
            vr += vol[c] * f.temperature(c).get::<kelvin>();
            v += vol[c];
        }
        CoreFeedbackTemperatures {
            fuel: ThermodynamicTemperature::new::<kelvin>(wf / w),
            moderator: ThermodynamicTemperature::new::<kelvin>(wm / w),
            reflector: ThermodynamicTemperature::new::<kelvin>(vr / v),
        }
    }

    fn circulation(&self) -> NaturalCirculation {
        let zero = NaturalCirculation {
            bed_upflow: MassRate::new::<kilogram_per_second>(0.0),
            bed_downflow: MassRate::new::<kilogram_per_second>(0.0),
            max_bed_mass_flux: 0.0,
            enthalpy_up_through_bed_top: Power::new::<watt>(0.0),
        };
        let Some(g) = &self.gas else { return zero };
        let mesh = self.solid.field().mesh();
        let (re, ze) = (mesh.r_edges(), mesh.z_edges());
        let nr = re.n_cells();
        let row = (0..ze.n_cells())
            .min_by(|a, b| {
                let za = 0.5 * (ze.edge(*a) + ze.edge(*a + 1)).get::<centimeter>();
                let zb = 0.5 * (ze.edge(*b) + ze.edge(*b + 1)).get::<centimeter>();
                (za - 90.0).abs().total_cmp(&(zb - 90.0).abs())
            })
            .unwrap_or(0);
        let ring = |i: usize| {
            let (a, b) = (re.edge(i).get::<uom::si::length::meter>(), re.edge(i + 1).get::<uom::si::length::meter>());
            PI * (b * b - a * a)
        };
        let (mut up, mut down, mut gmax) = (0.0, 0.0, 0.0_f64);
        for c in 0..mesh.n_cells() {
            if mesh.cell_region(c) != idx::BED {
                continue;
            }
            let (gr, gz) = g.mass_flux[c];
            gmax = gmax.max(gr.r_hypot(gz));
            if c / nr == row {
                // G_z positive downward (TECDOC z).
                if gz < 0.0 {
                    up -= gz * ring(c % nr);
                } else {
                    down += gz * ring(c % nr);
                }
            }
        }
        // Enthalpy through the bed top: the bed cells of the first bed row
        // against the cells above them.
        let mut q_up = 0.0;
        for i in 0..nr {
            for j in 1..ze.n_cells() {
                let c = mesh.cell_index(i, j);
                let above = mesh.cell_index(i, j - 1);
                if mesh.cell_region(c) == idx::BED && mesh.cell_region(above) != idx::BED {
                    let gz = 0.5 * (g.mass_flux[c].1 + g.mass_flux[above].1);
                    let (tb, ta) = (g.temperature[c], g.temperature[above]);
                    if tb.is_finite() && ta.is_finite() {
                        let t_up = if gz < 0.0 { tb } else { ta };
                        q_up += -gz * ring(i) * super::flow::HELIUM_CP * (t_up - 0.5 * (ta + tb));
                    }
                    break;
                }
            }
        }
        NaturalCirculation {
            bed_upflow: MassRate::new::<kilogram_per_second>(up),
            bed_downflow: MassRate::new::<kilogram_per_second>(down),
            max_bed_mass_flux: gmax,
            enthalpy_up_through_bed_top: Power::new::<watt>(q_up),
        }
    }

    fn report(&self, solid_iterations: usize, gas_iterations: usize) -> CoreStepReport {
        let f = self.solid.field();
        let bal = f.energy_balance();
        let w = |p: RzPatch| bal.removed_per_patch[p.index()].get::<watt>();
        let rccs = w(RzPatch::Outer) + w(RzPatch::Bottom) + w(RzPatch::Top);
        let fv = f.mesh().mesh();
        let mut to_gas = 0.0;
        let mut t_out = f64::NAN;
        if let Some(g) = &self.gas {
            for c in 0..f.mesh().n_cells() {
                let h = g.exchange[c];
                if h > 0.0 {
                    to_gas += h * fv.cell_volumes[c] * (f.temperature(c).get::<kelvin>() - g.temperature[c]);
                }
            }
            t_out = g.outlet_temperature;
        }
        CoreStepReport {
            outlet_temperature: ThermodynamicTemperature::new::<kelvin>(t_out),
            heat_to_gas: Power::new::<watt>(to_gas),
            heat_to_rccs_and_heads: Power::new::<watt>(rccs),
            power: Power::new::<watt>(self.power_w),
            feedback: self.feedback(),
            circulation: self.circulation(),
            solid_picard_iterations: solid_iterations,
            gas_picard_iterations: gas_iterations,
        }
    }

    /// The model at a thermocouple (the gh:#1150 instrument rule).
    #[must_use]
    pub fn reading(&self, tc: &Thermocouple) -> ThermocoupleReading {
        reading_at(&self.solid, &self.network, self.gas.as_ref(), tc)
    }

    /// Gravity head along the forced path between the RPV nozzles,
    /// `g (sum_down rho dz - sum_up rho dz)`, positive when it drives the
    /// forced direction: down the annulus (z 325-507) and the bed-stream
    /// column (`r < 90 cm`, z -65 to the hot-plenum mid-height 310), up the
    /// bottom annular path (507-461) and the risers (446 to -65). The two
    /// legs close (557 cm each way). The mixed and horizontal nodes (entry,
    /// bottom cavity, rounding path, top plenum) add no height.
    #[must_use]
    pub fn forced_path_gravity_head(&self, pressure: Pressure) -> Pressure {
        let Some(g) = &self.gas else { return Pressure::new::<pascal>(0.0) };
        let p_bar = pressure.get::<bar>();
        let mesh = self.solid.field().mesh();
        let (re, ze) = (mesh.r_edges(), mesh.z_edges());
        let nr = re.n_cells();
        let rho = |t: f64| gas(t, p_bar).map_or(f64::NAN, |x| x.rho);
        // (regions, z range cm, sign: +1 down leg, -1 up leg, r limit)
        use idx::*;
        let legs: [(&[usize], (f64, f64), f64, f64); 4] = [
            (&[ANNULAR_PATH], (325.0, 507.0), 1.0, 1e9),
            (
                &[TOP_FLOW_ZONE, CORE_ENTRY_PLENUM, BED, DISCHARGE_TUBE, UPPER_BOTTOM_REFL, BOTTOM_REFL_SOLID, SMALL_PLENUM, LOWER_BOTTOM_REFL, HOT_PLENUM],
                (-65.0, 310.0),
                1.0,
                90.0,
            ),
            (&[BOTTOM_ANNULAR_HE], (461.0, 507.0), -1.0, 1e9),
            (&[RISERS], (-65.0, 446.0), -1.0, 1e9),
        ];
        let mut s = 0.0;
        for (regions, (z0, z1), sign, rmax) in legs {
            for j in 0..ze.n_cells() {
                let (a, b) = (ze.edge(j).get::<centimeter>(), ze.edge(j + 1).get::<centimeter>());
                let (lo, hi) = (a.max(z0), b.min(z1));
                if hi <= lo {
                    continue;
                }
                let (mut sa, mut sr) = (0.0, 0.0);
                for i in 0..nr {
                    let c = mesh.cell_index(i, j);
                    let t = g.temperature[c];
                    if regions.contains(&mesh.cell_region(c)) && t.is_finite() && re.edge(i).get::<centimeter>() < rmax {
                        let (r0, r1) = (re.edge(i).get::<uom::si::length::meter>(), re.edge(i + 1).get::<uom::si::length::meter>());
                        let area = PI * (r1 * r1 - r0 * r0);
                        sa += area;
                        sr += area * rho(t);
                    }
                }
                if sa > 0.0 {
                    s += sign * sr / sa * (hi - lo) / 100.0;
                }
            }
        }
        Pressure::new::<pascal>(GRAVITY * s)
    }
}
