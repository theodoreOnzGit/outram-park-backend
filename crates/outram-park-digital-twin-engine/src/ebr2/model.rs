//! # `Ebr2Model`: a web-ready stepping model of EBR-II's primary system
//!
//! A lumped model of the SHRT-17 and SHRT-45R loss-of-flow tests:
//!
//! - **Power** is either taken from SHRT-17's power table (fission plus
//!   decay heat, a boundary condition), or computed by six-group point
//!   kinetics (`teh-o-prke`) with fast-reactor feedback
//!   (`teh_o_prke::feedback_mechanisms::fast_reactor`) and fission-product
//!   decay heat (`teh_o_prke::decay_heat`).
//! - **Four thermal nodes:** fuel; core coolant (with cladding and the
//!   in-core steel); the IHX primary side; the pool, which feeds the core
//!   inlet.
//! - **Flow** comes from a quasi-static loop momentum balance. Its terms are
//!   the NATDEMO pump curves ([`super::pump`]) at the tabulated speeds,
//!   SHRT-45R's auxiliary EM pump, natural-circulation buoyancy, and a loop
//!   resistance.
//! - **The intermediate side** (flow, inlet temperature) is a boundary
//!   condition.
//!
//! Sodium properties are CoolProp `LiqNa` through TUAS
//! (`tuas_boussinesq_solver`, 400-1000 K). Heat transfer uses TUAS's named
//! liquid-metal Nusselt correlations. Steel properties are TUAS's
//! `SteelSS304LHighTemp` (Kim 1975, ANL-75-55).
//!
//! ## This is new physics, derived and run uncalibrated
//!
//! Every coefficient below is derived from the cited geometry
//! ([`super::design`]), the cited boundary conditions ([`super::shrt`]) and
//! property data. **Nothing is fitted to a SHRT result.** The model's own
//! steady state is computed from those inputs. It is then compared with the
//! cited initial conditions; it is not forced to match them. The
//! disagreements are recorded in the tests and in
//! `crates/outram-park-digital-twin-engine/docs/ebr2_library.md`.
//!
//! ## Derivations and assumptions (each one is labelled, none is cited)
//!
//! - **Fuel to coolant.** Per pin, the following resistances act in series:
//!   - the fuel's mean-to-surface resistance `1/(8 pi k_f)` (uniform heat
//!     generation in a solid cylinder);
//!   - the sodium bond `ln(D_ci/D_f)/(2 pi k_Na)`;
//!   - the cladding `ln(D_co/D_ci)/(2 pi k_ss)`;
//!   - the film `1/(h pi D_co)`.
//!
//!   The fuel is fully dense U-5Fs (no porosity correction, see
//!   [`super::design::U5FS_CONDUCTIVITY`]). The cladding is 316 SS in the
//!   source, but TUAS has no 316, so 304L properties are used.
//! - **Core coolant node.** The node temperature is the mean of inlet and
//!   outlet, and outlet = `2 T_mean - T_in` (a linear axial profile, the
//!   usual lumped-channel closure). The flow through it is the **whole**
//!   primary flow, so its outlet is the mixed-mean core outlet. **A lumped
//!   core cannot give a peak subassembly temperature**, so do not compare
//!   it to one.
//! - **Core heat transfer flow.** The Péclet number is taken for one
//!   driver. Rows 1-7 (127 positions, `3n(n-1)+1` at n = 7) share the
//!   high-pressure plenum's 85 % of the flow (A226 p.3) equally. Reflectors
//!   in rows 6-7 really take less, and drivers more.
//! - **Wire-wrapped pin pitch** = cladding OD + wire diameter
//!   (P/D = 1.2817), the usual wire-wrap relation.
//! - **IHX.** A counter-flow epsilon-NTU model. The primary side is a node
//!   with the sodium volume of the tube bundle's shell side. The
//!   **assignment of primary to the shell side** is not in the transcribed
//!   data; [`IhxPrimarySide`] swaps it, and the tests report the effect.
//!   Each side uses its own correlation: the shell side Mikityuk at the
//!   tube lattice's P/D, the tube side Lyon-Martinelli.
//! - **Pool.** The pool sodium is the cited 340 m3 at 371 C, less the core
//!   and IHX nodes. It is one well-mixed node, with no stratification, no
//!   upper plenum and no Z-pipe, and no heat loss to the tank or the
//!   shutdown coolers.
//! - **Loop momentum.** Quasi-static (no fluid inertia):
//!   `H_r [ (h(s1,w) + h(s2,w))/2 - w|w| ] + dp_aux + dp_buoy = 0`, with
//!   `w = W/(2 W_r)`. The loop resistance `H_r w|w|` puts the operating
//!   point at the pumps' rated point (rated head at rated flow at rated
//!   speed), which is how a pump is matched to its loop. Both pumps carry
//!   half the flow and the mean head is used. The resistance is purely
//!   quadratic, which underestimates the low-flow (laminar) resistance in
//!   natural circulation. Flow reversal is not modelled: if the net head at
//!   zero flow is negative, the flow is zero.
//! - **Buoyancy** `g dz (rho_pool - rho_core outlet)`. `dz`, the elevation
//!   of the IHX thermal centre above the core's, is **a required input with
//!   no default**. No transcribed source gives it, and natural circulation
//!   after the pumps stop depends on it.
//! - **Auxiliary EM pump (SHRT-45R).** `dp = 0.25 psi x I / 9000 A`, the
//!   A226 p.30 rating scaled **linearly** in current. That is a conduction
//!   pump with a field independent of its current; it is an assumption.
//!   With a series field the pressure would scale as `I^2`. The pressure's
//!   fall with flow (back EMF) is ignored. T1819 reports that flow rose
//!   about 20 % when the current rose 50 % at 600 s (gh:#1316). That was
//!   known when the linear form was chosen, so the comparison is not an
//!   independent check of it.
//! - **Heat deposition.** All power, fission and decay alike, goes into the
//!   fuel node. Gamma heating of the coolant and structure is not split
//!   out.
//! - **Decay heat (point-kinetics mode).** `teh_o_prke::decay_heat`'s U-235
//!   thermal set, the 1978 ANS draft. The benchmark itself computed SHRT-17's
//!   decay heat with the ANS LWR standard (A226 p.17). The set is driven
//!   through A226's assumed SHRT-45R pre-test history, from a cold start: the
//!   decay heat left from the 1.5-month shutdown before it is neglected.
//!   The history's powers are total powers, and are converted to fission
//!   powers by [`history_decay_heat`].
//!
//! ## Numerics
//!
//! [`Ebr2Model::step`] splits `dt` into sub-steps of at most
//! [`MAX_SUBSTEP_S`]. Each sub-step does the following, in order:
//!
//! 1. Solve the flow by bisection, with temperatures from the start of the
//!    sub-step.
//! 2. Advance the power: backward-Euler point kinetics with the feedback at
//!    the start-of-step temperatures, or the power table at the sub-step
//!    midpoint.
//! 3. Solve fuel and core coolant together, implicitly (a 2 x 2 system).
//! 4. Solve the IHX node, then the pool node, each implicitly in its own
//!    temperature.
//!
//! Every node update is a weighted mean of bounded values, so the scheme is
//! stable at any sub-step. Properties are evaluated at the start of the
//! sub-step. There is no allocation per step beyond one small `Vec` for the
//! feedback state, no I/O, and no randomness, so the model is deterministic,
//! cheap, and builds for `wasm32-unknown-unknown`.
//!
//! **A sodium temperature outside TUAS's 400-1000 K range makes `step`
//! return an error.** 1000 K is below sodium's normal boiling point (about
//! 1156 K, TUAS `liquid_sodium_coolprop` module doc), so a model that steps
//! successfully has kept every node's sodium below boiling.
//!
//! > Research, education and V&V only. **NOT VALIDATED.** AI draft,
//! > unreviewed.

use petir::mathf::RealMath;
use teh_o_prke::decay_heat::{DecayHeat, FissioningNuclide};
use teh_o_prke::feedback_mechanisms::fast_reactor::{
    CoolantDensityTerm, DrivelineExpansionTerm, FastDopplerTerm, FastReactorFeedback,
    FastReactorReactivity, FastReactorState, LinearTemperatureTerm,
};
use teh_o_prke::prelude::{PointKinetics, TehOPrkeError};
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::density::try_get_rho;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::dynamic_viscosity::try_get_mu_viscosity;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::liquid_database::liquid_sodium_coolprop::liquid_sodium;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::specific_heat_capacity::try_get_cp;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::thermal_conductivity::try_get_kappa_thermal_conductivity;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::{Material, SolidMaterial};
use tuas_boussinesq_solver::heat_transfer_correlations::nusselt_number_correlations::liquid_metal::LiquidMetalCorrelation;
use tuas_boussinesq_solver::tuas_lib_error::TuasLibError;
use uom::si::angular_velocity::revolution_per_minute;
use uom::si::electric_current::ampere;
use uom::si::f64::*;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::{atmosphere, pascal, pound_force_per_square_inch};
use uom::si::ratio::ratio;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::{day, second};
use uom::si::volumetric_number_density::per_cubic_meter;
use uom::ConstZero;

use super::design;
use super::kinetics::{self, ArgonneSolver};
use super::pump;
use super::shrt::{self, ShrtTest};

/// The largest internal sub-step, 0.05 s.
pub const MAX_SUBSTEP_S: f64 = 0.05;

/// Standard gravity, 9.80665 m/s2 (CGPM 1901).
const STANDARD_GRAVITY: f64 = 9.80665;

/// Rows 1-7 of the hexagonal grid: `3 n (n - 1) + 1` at `n = 7` = 127
/// positions, the high-pressure plenum's (A226 p.106, gh:#1315).
pub const HIGH_PRESSURE_POSITIONS: u32 = 127;

/// Errors from building or stepping the model.
#[derive(Debug, thiserror::Error)]
pub enum Ebr2Error {
    /// A sodium or steel property could not be evaluated (usually a
    /// temperature outside TUAS's 400-1000 K sodium range).
    #[error("property evaluation failed: {0}")]
    Property(#[from] TuasLibError),
    /// The kinetics or feedback evaluation failed.
    #[error("kinetics failed: {0}")]
    Kinetics(#[from] TehOPrkeError),
    /// An input is outside what the model accepts.
    #[error("invalid input: {0}")]
    Invalid(String),
}

// ---------------------------------------------------------------------------
// Property helpers (TUAS)
// ---------------------------------------------------------------------------

fn sodium() -> Material {
    Material::Liquid(liquid_sodium())
}

fn steel() -> Material {
    Material::Solid(SolidMaterial::SteelSS304LHighTemp)
}

fn one_atmosphere() -> Pressure {
    Pressure::new::<atmosphere>(1.0)
}

/// Liquid sodium density (CoolProp `LiqNa` via TUAS, 400-1000 K).
pub fn sodium_density(t: ThermodynamicTemperature) -> Result<MassDensity, TuasLibError> {
    try_get_rho(sodium(), t, one_atmosphere())
}

/// Liquid sodium heat capacity (CoolProp `LiqNa` via TUAS).
pub fn sodium_heat_capacity(
    t: ThermodynamicTemperature,
) -> Result<SpecificHeatCapacity, TuasLibError> {
    try_get_cp(sodium(), t, one_atmosphere())
}

fn sodium_conductivity(t: ThermodynamicTemperature) -> Result<ThermalConductivity, TuasLibError> {
    try_get_kappa_thermal_conductivity(sodium(), t, one_atmosphere())
}

fn sodium_viscosity(t: ThermodynamicTemperature) -> Result<DynamicViscosity, TuasLibError> {
    try_get_mu_viscosity(sodium(), t, one_atmosphere())
}

fn steel_conductivity(t: ThermodynamicTemperature) -> Result<ThermalConductivity, TuasLibError> {
    try_get_kappa_thermal_conductivity(steel(), t, one_atmosphere())
}

fn steel_heat_capacity(t: ThermodynamicTemperature) -> Result<SpecificHeatCapacity, TuasLibError> {
    try_get_cp(steel(), t, one_atmosphere())
}

fn steel_density(t: ThermodynamicTemperature) -> Result<MassDensity, TuasLibError> {
    try_get_rho(steel(), t, one_atmosphere())
}

/// `d rho / dT` of sodium at `t`, by a central difference over +-0.5 K.
pub fn sodium_density_slope(t: ThermodynamicTemperature) -> Result<f64, TuasLibError> {
    let k = t.get::<kelvin>();
    let hi = sodium_density(ThermodynamicTemperature::new::<kelvin>(k + 0.5))?;
    let lo = sodium_density(ThermodynamicTemperature::new::<kelvin>(k - 0.5))?;
    Ok((hi - lo).get::<kilogram_per_cubic_meter>())
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Which side of the IHX tube bundle carries the primary sodium. The
/// transcribed geometry (A226 Table 5.2.7.1) does not say; see the module
/// doc.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IhxPrimarySide {
    /// Primary on the shell side, intermediate in the tubes (default).
    Shell,
    /// Primary in the tubes.
    Tube,
}

/// The temperature the radial-expansion coefficient multiplies.
///
/// **Default `CoreInlet`, chosen before any run.** The core is
/// bottom-supported (A226 p.105, gh:#1313 A8), so its subassemblies stand
/// in the grid plate, which sits in core-inlet sodium. The above-core load
/// pads, at roughly outlet temperature, also set the pitch in a real core.
/// The two alternatives are kept for ablation, and the tests report all
/// three.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadialExpansionTemperature {
    /// The grid plate at core-inlet temperature (default).
    CoreInlet,
    /// The core mean coolant temperature.
    CoreMeanCoolant,
    /// The load pads at core-outlet temperature.
    CoreOutlet,
}

/// Reactivity coefficients as the CRP prints them (per kelvin), plus the
/// Doppler constant's nominal temperature.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReactivityCoefficients {
    /// Axial fuel expansion, on the fuel temperature.
    pub axial_fuel_expansion: TemperatureCoefficient,
    /// Radial core expansion, on [`RadialExpansionTemperature`].
    pub radial_expansion: TemperatureCoefficient,
    /// Sodium density, per kelvin of core coolant temperature. Converted to
    /// a per-density coefficient with CoolProp's `d rho / dT` at the initial
    /// core coolant temperature (derived), because GeN-Foam's term is in
    /// density.
    pub sodium_density_per_kelvin: TemperatureCoefficient,
    /// Doppler, per kelvin at [`Self::doppler_nominal_temperature`]. The
    /// Doppler constant is `K_D = -alpha_D T_nom` (A228 Eq 4.2's `1/T`
    /// law), applied as `-K_D ln(T/T_ref)`.
    pub doppler_per_kelvin: TemperatureCoefficient,
    /// The fuel temperature at which `doppler_per_kelvin` is defined.
    pub doppler_nominal_temperature: ThermodynamicTemperature,
}

impl ReactivityCoefficients {
    /// Argonne's coefficients for `solver` (T1819 TABLE 37, gh:#1314), with
    /// the Doppler nominal temperature of A228 p.25 (487 C).
    pub fn argonne(solver: ArgonneSolver) -> Self {
        let v = solver.reference_values();
        Self {
            axial_fuel_expansion: kinetics::pcm_per_kelvin(v.axial_expansion_pcm_per_k),
            radial_expansion: kinetics::pcm_per_kelvin(v.radial_expansion_pcm_per_k),
            sodium_density_per_kelvin: kinetics::pcm_per_kelvin(v.sodium_density_pcm_per_k),
            doppler_per_kelvin: kinetics::pcm_per_kelvin(v.doppler_pcm_per_k),
            doppler_nominal_temperature: kinetics::doppler_nominal_fuel_temperature(),
        }
    }
}

/// Point-kinetics configuration.
#[derive(Clone, Debug, PartialEq)]
pub struct KineticsConfig {
    /// beta-eff (default Argonne's 705 pcm).
    pub beta_eff: Ratio,
    /// Prompt generation time (default Argonne's 3.58238e-7 s).
    pub generation_time: Time,
    /// Feedback coefficients (default Argonne VARIANT).
    pub coefficients: ReactivityCoefficients,
    /// Which temperature drives radial expansion (default core inlet).
    pub radial_expansion_temperature: RadialExpansionTemperature,
    /// Optional driveline term, on the core-outlet temperature (default
    /// none: Argonne's set has no driveline coefficient).
    pub driveline: Option<DrivelineExpansionTerm>,
    /// Pre-test irradiation, `(total thermal power, duration)` in order,
    /// from a cold start (default A226 p.24's SHRT-45R history). See
    /// [`history_decay_heat`] for how total power becomes fission power.
    pub decay_history: Vec<(Power, Time)>,
}

impl KineticsConfig {
    /// Argonne's kinetics and VARIANT coefficients with the SHRT-45R
    /// pre-test history: every number cited, see [`super::kinetics`].
    pub fn argonne_shrt45r() -> Self {
        Self {
            beta_eff: kinetics::beta_eff(),
            generation_time: kinetics::prompt_generation_time(),
            coefficients: ReactivityCoefficients::argonne(ArgonneSolver::Variant),
            radial_expansion_temperature: RadialExpansionTemperature::CoreInlet,
            driveline: None,
            decay_history: shrt::SHRT45R_PRE_TEST_HISTORY
                .iter()
                .map(|[mw, days]| {
                    (
                        Power::new::<uom::si::power::megawatt>(*mw),
                        Time::new::<day>(*days),
                    )
                })
                .collect(),
        }
    }
}

/// Where the power comes from.
#[derive(Clone, Debug, PartialEq)]
pub enum PowerConfig {
    /// Follow SHRT-17's fission and decay-heat table (only valid for
    /// [`ShrtTest::Shrt17`]).
    Tabulated,
    /// Point kinetics with feedback; the initial power is the test's.
    PointKinetics(KineticsConfig),
}

/// Model geometry choices (cited counts, derived quantities).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ebr2Geometry {
    /// Fuelled subassemblies counted as 91-pin drivers (default 87, see
    /// [`design::RUN_138B_CORE_DRIVERS`]).
    pub fuelled_subassemblies: u32,
    /// Liquid-metal correlation in the core bundle (default Mikityuk at
    /// the derived wire-wrap P/D).
    pub core_nusselt: LiquidMetalCorrelation,
    /// Which IHX side is primary (default shell).
    pub ihx_primary_side: IhxPrimarySide,
}

impl Ebr2Geometry {
    /// The cited counts and the derived default choices.
    pub fn cited() -> Self {
        Self {
            fuelled_subassemblies: design::RUN_138B_CORE_DRIVERS,
            core_nusselt: LiquidMetalCorrelation::Mikityuk {
                pitch_to_diameter: wire_wrap_pitch_to_diameter(),
            },
            ihx_primary_side: IhxPrimarySide::Shell,
        }
    }
}

/// Wire-wrapped pin pitch over diameter, `(D_co + d_w) / D_co` (derived).
pub fn wire_wrap_pitch_to_diameter() -> Ratio {
    let d = design::cladding_outer_diameter();
    (d + design::wire_wrap_diameter()) / d
}

/// Everything the model needs that is not a cited constant.
#[derive(Clone, Debug, PartialEq)]
pub struct Ebr2Config {
    /// Which test's boundary conditions drive the model.
    pub test: ShrtTest,
    /// Elevation of the IHX thermal centre above the core's. **Required,
    /// no default** (not in the transcribed sources).
    pub thermal_centre_height: Length,
    /// Geometry choices.
    pub geometry: Ebr2Geometry,
    /// Power source.
    pub power: PowerConfig,
    /// Whether SHRT-45R's auxiliary EM pump head is applied (default true
    /// for SHRT-45R; SHRT-17's current is zero anyway). Turning it off is
    /// an ablation.
    pub aux_em_pump: bool,
}

impl Ebr2Config {
    /// SHRT-17: tabulated power, cited geometry.
    pub fn shrt17(thermal_centre_height: Length) -> Self {
        Self {
            test: ShrtTest::Shrt17,
            thermal_centre_height,
            geometry: Ebr2Geometry::cited(),
            power: PowerConfig::Tabulated,
            aux_em_pump: true,
        }
    }

    /// SHRT-45R: Argonne point kinetics and feedback, cited geometry.
    pub fn shrt45r(thermal_centre_height: Length) -> Self {
        Self {
            test: ShrtTest::Shrt45r,
            thermal_centre_height,
            geometry: Ebr2Geometry::cited(),
            power: PowerConfig::PointKinetics(KineticsConfig::argonne_shrt45r()),
            aux_em_pump: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Derived geometry
// ---------------------------------------------------------------------------

/// Quantities derived once from the geometry (SI).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DerivedGeometry {
    /// Fuelled pins.
    pub pins: f64,
    /// Fuel mass, kg (cell fuel fraction x cell volume x density).
    pub fuel_mass_kg: f64,
    /// In-core sodium volume, m3 (cell sodium fraction).
    pub core_sodium_volume_m3: f64,
    /// In-core steel volume, m3 (cell steel fraction).
    pub core_steel_volume_m3: f64,
    /// Driver bundle flow area inside the duct, m2.
    pub bundle_flow_area_m2: f64,
    /// Driver bundle hydraulic diameter, m.
    pub bundle_hydraulic_diameter_m: f64,
    /// IHX shell-side flow area (tube lattice), m2.
    pub ihx_shell_flow_area_m2: f64,
    /// IHX shell-side hydraulic diameter, m.
    pub ihx_shell_hydraulic_diameter_m: f64,
    /// IHX tube-side flow area, m2.
    pub ihx_tube_flow_area_m2: f64,
    /// IHX shell-side P/D.
    pub ihx_pitch_to_diameter: f64,
}

impl DerivedGeometry {
    /// Derive from [`super::design`] for `fuelled_subassemblies` drivers.
    pub fn new(fuelled_subassemblies: u32) -> Self {
        let pi = std::f64::consts::PI;
        let n_sa = f64::from(fuelled_subassemblies);
        let pins = n_sa * f64::from(design::DRIVER_PINS_PER_SUBASSEMBLY);
        let cell_volume = design::lattice_cell_area().value * design::FUEL_SLUG_LENGTH_M;
        let [f_fuel, f_ss, f_na] = design::driver_volume_fractions().map(|f| f.get::<ratio>());
        let d_co = design::cladding_outer_diameter().value;
        let d_w = design::wire_wrap_diameter().value;
        let flat = design::hex_duct_flat_inside().value;
        let n_pin = f64::from(design::DRIVER_PINS_PER_SUBASSEMBLY);
        let hex_area = 0.5 * 3.0_f64.sqrt() * flat * flat;
        let bundle_flow_area = hex_area - n_pin * 0.25 * pi * (d_co * d_co + d_w * d_w);
        let wetted = n_pin * pi * (d_co + d_w) + 2.0 * 3.0_f64.sqrt() * flat;
        let d_o = design::ihx_tube_outer_diameter().value;
        let d_i = design::ihx_tube_inner_diameter().value;
        let p = design::ihx_tube_pitch().value;
        let n_t = f64::from(design::IHX_TUBE_COUNT);
        let lattice = 0.5 * 3.0_f64.sqrt() * p * p - 0.25 * pi * d_o * d_o;
        Self {
            pins,
            fuel_mass_kg: n_sa * cell_volume * f_fuel * design::u5fs_density().value,
            core_sodium_volume_m3: n_sa * cell_volume * f_na,
            core_steel_volume_m3: n_sa * cell_volume * f_ss,
            bundle_flow_area_m2: bundle_flow_area,
            bundle_hydraulic_diameter_m: 4.0 * bundle_flow_area / wetted,
            ihx_shell_flow_area_m2: n_t * lattice,
            ihx_shell_hydraulic_diameter_m: 4.0 * lattice / (pi * d_o),
            ihx_tube_flow_area_m2: n_t * 0.25 * pi * d_i * d_i,
            ihx_pitch_to_diameter: p / d_o,
        }
    }
}

// ---------------------------------------------------------------------------
// The model
// ---------------------------------------------------------------------------

/// Inputs overridden by the caller (a web slider); `None` follows the
/// test's table.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct InputOverrides {
    /// Pump speeds `(pump 1, pump 2)`.
    pub pump_speeds: Option<(AngularVelocity, AngularVelocity)>,
    /// Intermediate IHX inlet flow.
    pub intermediate_flow: Option<MassRate>,
    /// Intermediate IHX inlet temperature.
    pub intermediate_inlet_temperature: Option<ThermodynamicTemperature>,
    /// Auxiliary EM pump current.
    pub aux_em_pump_current: Option<ElectricCurrent>,
}

/// The boundary values in force at the current time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ebr2Inputs {
    /// Pump speeds `(pump 1, pump 2)`.
    pub pump_speeds: (AngularVelocity, AngularVelocity),
    /// Intermediate IHX inlet flow.
    pub intermediate_flow: MassRate,
    /// Intermediate IHX inlet temperature.
    pub intermediate_inlet_temperature: ThermodynamicTemperature,
    /// Auxiliary EM pump current.
    pub aux_em_pump_current: ElectricCurrent,
}

/// The reactivity broken into its terms (all zero in tabulated mode).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ebr2Reactivity {
    /// The fast-reactor feedback terms.
    pub feedback: FastReactorReactivity,
    /// Externally inserted reactivity (rods, a web slider).
    pub external: Ratio,
}

impl Ebr2Reactivity {
    fn zero() -> Self {
        Self {
            feedback: FastReactorReactivity {
                doppler: Ratio::ZERO,
                axial_fuel_expansion: Ratio::ZERO,
                cladding: Ratio::ZERO,
                coolant_temperature: Ratio::ZERO,
                coolant_density: Ratio::ZERO,
                radial_expansion: Ratio::ZERO,
                driveline: Ratio::ZERO,
            },
            external: Ratio::ZERO,
        }
    }

    /// Feedback plus external.
    pub fn total(&self) -> Ratio {
        self.feedback.total() + self.external
    }
}

/// Point-kinetics state.
#[derive(Clone, Debug)]
struct KineticsState {
    kinetics: PointKinetics,
    generation_time: Time,
    beta_eff: Ratio,
    feedback: FastReactorFeedback,
    radial_expansion_temperature: RadialExpansionTemperature,
    decay: DecayHeat,
    initial_neutron_density: f64,
    initial_fission_power: Power,
}

/// Where the power comes from, with its state.
#[derive(Clone, Debug)]
enum PowerState {
    Tabulated,
    Kinetics(KineticsState),
}

/// The four node temperatures.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Temperatures {
    fuel: ThermodynamicTemperature,
    core_mean: ThermodynamicTemperature,
    ihx_outlet: ThermodynamicTemperature,
    pool: ThermodynamicTemperature,
}

impl Temperatures {
    fn core_outlet(&self) -> ThermodynamicTemperature {
        let k = 2.0 * self.core_mean.get::<kelvin>() - self.pool.get::<kelvin>();
        ThermodynamicTemperature::new::<kelvin>(k)
    }
}

/// The steady state the model computes for the test's initial conditions,
/// with the cited values it can be compared against. See
/// [`Ebr2Model::initial_comparison`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InitialComparison {
    /// Model initial primary flow.
    pub primary_flow: MassRate,
    /// Cited initial primary flow (A226 test description, gpm at 800 F).
    pub cited_primary_flow: MassRate,
    /// Model core inlet (pool) temperature.
    pub core_inlet: ThermodynamicTemperature,
    /// Cited initial core inlet temperature.
    pub cited_core_inlet: ThermodynamicTemperature,
    /// Model core outlet temperature.
    pub core_outlet: ThermodynamicTemperature,
    /// Model fuel minus core mean coolant temperature.
    pub fuel_minus_coolant: TemperatureInterval,
    /// Model pump discharge head (mean of the two pumps).
    pub pump_head: Pressure,
    /// Fuel-to-coolant conductance.
    pub fuel_to_coolant_ua: ThermalConductance,
    /// IHX conductance.
    pub ihx_ua: ThermalConductance,
}

/// The EBR-II stepping model. See the module doc.
#[derive(Clone, Debug)]
pub struct Ebr2Model {
    config: Ebr2Config,
    derived: DerivedGeometry,
    time: Time,
    temperatures: Temperatures,
    primary_flow: MassRate,
    initial_primary_flow: MassRate,
    initial_total_power: Power,
    fission_power: Power,
    decay_power: Power,
    power: PowerState,
    external_reactivity: Ratio,
    reactivity: Ebr2Reactivity,
    overrides: InputOverrides,
    inputs: Ebr2Inputs,
    core_heat_capacity: HeatCapacity,
    core_sodium_mass: Mass,
    fuel_heat_capacity_mass: Mass,
    ihx_sodium_mass: Mass,
    pool_sodium_mass: Mass,
    initial_comparison: InitialComparison,
}

/// The thermal resistances of one fuel pin per unit length, and the
/// conductances that follow (for the tests and the V&V doc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConductanceBreakdown {
    /// Fuel mean-to-surface, m K/W.
    pub fuel: f64,
    /// Sodium bond, m K/W.
    pub bond: f64,
    /// Cladding, m K/W.
    pub cladding: f64,
    /// Film, m K/W.
    pub film: f64,
    /// Core-bundle Péclet number.
    pub peclet: f64,
    /// Core-bundle Nusselt number.
    pub nusselt: f64,
    /// Total UA, W/K.
    pub ua: f64,
}

impl Ebr2Model {
    /// SHRT-17 with cited defaults; `thermal_centre_height` is required.
    pub fn shrt17(thermal_centre_height: Length) -> Result<Self, Ebr2Error> {
        Self::new(Ebr2Config::shrt17(thermal_centre_height))
    }

    /// SHRT-45R with cited defaults; `thermal_centre_height` is required.
    pub fn shrt45r(thermal_centre_height: Length) -> Result<Self, Ebr2Error> {
        Self::new(Ebr2Config::shrt45r(thermal_centre_height))
    }

    /// Build the model at its own steady state for `config.test`'s initial
    /// conditions (see the module doc).
    pub fn new(config: Ebr2Config) -> Result<Self, Ebr2Error> {
        if let (PowerConfig::Tabulated, ShrtTest::Shrt45r) = (&config.power, config.test) {
            return Err(Ebr2Error::Invalid(
                "SHRT-45R has no power table (its power is a result); use point kinetics"
                    .to_string(),
            ));
        }
        let dz = config.thermal_centre_height.value;
        if !dz.is_finite() || dz < 0.0 {
            return Err(Ebr2Error::Invalid(format!(
                "thermal centre height must be finite and >= 0, got {dz} m"
            )));
        }
        let derived = DerivedGeometry::new(config.geometry.fuelled_subassemblies);
        let zero = Time::ZERO;
        let overrides = InputOverrides::default();
        let inputs = evaluate_inputs(config.test, &overrides, zero)?;

        let total_power = match config.test.tabulated_power(zero) {
            Some((f, d)) => f + d,
            None => config.test.initial_power(),
        };

        // Steady state by fixed-point iteration.
        let t_in_guess = config.test.initial_core_inlet_temperature();
        let mut temps = Temperatures {
            fuel: t_in_guess,
            core_mean: t_in_guess,
            ihx_outlet: t_in_guess,
            pool: t_in_guess,
        };
        let mut ua_fc = ThermalConductance::ZERO;
        let mut ua_ihx = ThermalConductance::ZERO;
        for _ in 0..50 {
            let flow = solve_primary_flow(&config, &inputs, &temps)?;
            let c_p = loop_heat_capacity(&temps)?;
            let (ua, eps, c_min) =
                ihx_effectiveness(&config, &derived, &temps, flow, &inputs)?;
            ua_ihx = ua;
            let t_s = inputs.intermediate_inlet_temperature.get::<kelvin>();
            let t_out = t_s + total_power.get::<watt>() / (eps * c_min);
            let t_in = t_out - total_power.get::<watt>() / (flow.value * c_p.value);
            let t_mean = 0.5 * (t_in + t_out);
            ua_fc = fuel_to_coolant(&config, &derived, &temps, flow)?.ua_si();
            let t_fuel = t_mean + total_power.get::<watt>() / ua_fc.value;
            let new = Temperatures {
                fuel: ThermodynamicTemperature::new::<kelvin>(t_fuel),
                core_mean: ThermodynamicTemperature::new::<kelvin>(t_mean),
                ihx_outlet: ThermodynamicTemperature::new::<kelvin>(t_in),
                pool: ThermodynamicTemperature::new::<kelvin>(t_in),
            };
            let change = (new.fuel.get::<kelvin>() - temps.fuel.get::<kelvin>()).abs()
                + (new.core_mean.get::<kelvin>() - temps.core_mean.get::<kelvin>()).abs();
            temps = new;
            if change < 1e-9 {
                break;
            }
        }
        let flow = solve_primary_flow(&config, &inputs, &temps)?;

        // Node masses at the initial temperatures (held fixed afterwards).
        let rho_core = sodium_density(temps.core_mean)?;
        let core_sodium_mass = rho_core * Volume::new::<uom::si::volume::cubic_meter>(
            derived.core_sodium_volume_m3,
        );
        let steel_mass = steel_density(temps.core_mean)?
            * Volume::new::<uom::si::volume::cubic_meter>(derived.core_steel_volume_m3);
        let core_heat_capacity = core_sodium_mass * sodium_heat_capacity(temps.core_mean)?
            + steel_mass * steel_heat_capacity(temps.core_mean)?;
        let ihx_volume = derived.ihx_shell_flow_area_m2 * design::IHX_TUBE_LENGTH_M;
        let ihx_volume = match config.geometry.ihx_primary_side {
            IhxPrimarySide::Shell => ihx_volume,
            IhxPrimarySide::Tube => derived.ihx_tube_flow_area_m2 * design::IHX_TUBE_LENGTH_M,
        };
        let ihx_sodium_mass =
            sodium_density(temps.ihx_outlet)? * Volume::new::<uom::si::volume::cubic_meter>(ihx_volume);
        let total_sodium_mass = sodium_density(design::primary_sodium_reference_temperature())?
            * design::primary_sodium_capacity();
        let pool_sodium_mass = total_sodium_mass - core_sodium_mass - ihx_sodium_mass;

        // Power state.
        let (power, fission_power, decay_power) = match &config.power {
            PowerConfig::Tabulated => {
                let (f, d) = config
                    .test
                    .tabulated_power(zero)
                    .ok_or_else(|| Ebr2Error::Invalid("no power table".to_string()))?;
                (PowerState::Tabulated, f, d)
            }
            PowerConfig::PointKinetics(k) => {
                let decay = history_decay_heat(&k.decay_history);
                let decay_power = decay.total_decay_heat_power();
                let fission_power = total_power - decay_power;
                if fission_power.value <= 0.0 {
                    return Err(Ebr2Error::Invalid(
                        "decay heat from the history exceeds the initial power".to_string(),
                    ));
                }
                let mut pk = kinetics::argonne_point_kinetics(k.beta_eff);
                let n0 = 1.0e10;
                pk.set_critical_equilibrium(
                    VolumetricNumberDensity::new::<per_cubic_meter>(n0),
                    k.generation_time,
                );
                let feedback = build_feedback(k, &temps)?;
                (
                    PowerState::Kinetics(KineticsState {
                        kinetics: pk,
                        generation_time: k.generation_time,
                        beta_eff: k.beta_eff,
                        feedback,
                        radial_expansion_temperature: k.radial_expansion_temperature,
                        decay,
                        initial_neutron_density: n0,
                        initial_fission_power: fission_power,
                    }),
                    fission_power,
                    decay_power,
                )
            }
        };

        let (s1, s2) = inputs.pump_speeds;
        let w = flow.value / (2.0 * design::PUMP_RATED_FLOW_KG_PER_S);
        let h_mean = 0.5
            * (pump::head_ratio(s1.get::<revolution_per_minute>() / design::PUMP_RATED_SPEED_RPM, w)
                + pump::head_ratio(s2.get::<revolution_per_minute>() / design::PUMP_RATED_SPEED_RPM, w));
        let initial_comparison = InitialComparison {
            primary_flow: flow,
            cited_primary_flow: config.test.initial_primary_flow()?,
            core_inlet: temps.pool,
            cited_core_inlet: config.test.initial_core_inlet_temperature(),
            core_outlet: temps.core_outlet(),
            fuel_minus_coolant: TemperatureInterval::new::<uom::si::temperature_interval::kelvin>(
                temps.fuel.get::<kelvin>() - temps.core_mean.get::<kelvin>(),
            ),
            pump_head: Pressure::new::<pascal>(h_mean * design::PUMP_RATED_HEAD_PA),
            fuel_to_coolant_ua: ua_fc,
            ihx_ua: ua_ihx,
        };

        let mut model = Self {
            fuel_heat_capacity_mass: Mass::new::<uom::si::mass::kilogram>(derived.fuel_mass_kg),
            config,
            derived,
            time: zero,
            temperatures: temps,
            primary_flow: flow,
            initial_primary_flow: flow,
            initial_total_power: total_power,
            fission_power,
            decay_power,
            power,
            external_reactivity: Ratio::ZERO,
            reactivity: Ebr2Reactivity::zero(),
            overrides,
            inputs,
            core_heat_capacity,
            core_sodium_mass,
            ihx_sodium_mass,
            pool_sodium_mass,
            initial_comparison,
        };
        model.reactivity = model.evaluate_reactivity()?;
        Ok(model)
    }

    /// Advance by `dt` (split into sub-steps of at most [`MAX_SUBSTEP_S`]).
    pub fn step(&mut self, dt: Time) -> Result<(), Ebr2Error> {
        let dt_s = dt.get::<second>();
        if !dt_s.is_finite() || dt_s < 0.0 {
            return Err(Ebr2Error::Invalid(format!("dt must be finite and >= 0, got {dt_s} s")));
        }
        if dt_s == 0.0 {
            return Ok(());
        }
        let n = (dt_s / MAX_SUBSTEP_S).ceil().max(1.0);
        let h = Time::new::<second>(dt_s / n);
        for _ in 0..(n as u64) {
            self.substep(h)?;
        }
        Ok(())
    }

    fn substep(&mut self, h: Time) -> Result<(), Ebr2Error> {
        let t_end = self.time + h;
        let t_mid = self.time + 0.5 * h;
        let inputs = evaluate_inputs(self.config.test, &self.overrides, t_end)?;
        let temps = self.temperatures;

        // 1. flow
        let flow = solve_primary_flow(&self.config, &inputs, &temps)?;

        // 2. power
        self.reactivity = self.evaluate_reactivity()?;
        let (fission, decay) = match &mut self.power {
            PowerState::Tabulated => self
                .config
                .test
                .tabulated_power(t_mid)
                .ok_or_else(|| Ebr2Error::Invalid("no power table".to_string()))?,
            PowerState::Kinetics(k) => {
                let rho = self.reactivity.total();
                k.kinetics
                    .step_implicit(h, rho, k.generation_time, VolumetricNumberRate::ZERO)?;
                let n = k.kinetics.neutron_density().get::<per_cubic_meter>();
                let fission = k.initial_fission_power * (n / k.initial_neutron_density);
                k.decay.advance_timestep(fission, h);
                (fission, k.decay.total_decay_heat_power())
            }
        };
        let power = fission + decay;

        // 3. fuel + core coolant, implicit 2x2
        let hs = h.get::<second>();
        let c_fuel = self.fuel_heat_capacity_mass.value
            * design::u5fs_heat_capacity(temps.fuel).value;
        let c_core = self.core_heat_capacity.value;
        let ua = fuel_to_coolant(&self.config, &self.derived, &temps, flow)?.ua;
        // One c_p for every primary advective term, at the core mean
        // temperature, so that the initial steady state is a fixed point of
        // the step (see `loop_heat_capacity`).
        let cp_loop = loop_heat_capacity(&temps)?.value;
        let wc = flow.value * cp_loop;
        let t_f = temps.fuel.get::<kelvin>();
        let t_c = temps.core_mean.get::<kelvin>();
        let t_in = temps.pool.get::<kelvin>();
        let a11 = c_fuel / hs + ua;
        let a12 = -ua;
        let b1 = c_fuel * t_f / hs + power.get::<watt>();
        let a21 = -ua;
        let a22 = c_core / hs + ua + 2.0 * wc;
        let b2 = c_core * t_c / hs + 2.0 * wc * t_in;
        let det = a11 * a22 - a12 * a21;
        let t_f_new = (b1 * a22 - a12 * b2) / det;
        let t_c_new = (a11 * b2 - a21 * b1) / det;
        let t_out_new = 2.0 * t_c_new - t_in;

        // 4. IHX primary node, then pool
        let mut hot = temps;
        hot.core_mean = ThermodynamicTemperature::new::<kelvin>(t_c_new);
        let (_, eps, c_min) = ihx_effectiveness(&self.config, &self.derived, &hot, flow, &inputs)?;
        let t_s = inputs.intermediate_inlet_temperature.get::<kelvin>();
        let q_ihx = eps * c_min * (t_out_new - t_s);
        let c_ihx = self.ihx_sodium_mass.value * sodium_heat_capacity(temps.ihx_outlet)?.value;
        let t_x = temps.ihx_outlet.get::<kelvin>();
        let t_x_new = (c_ihx * t_x / hs + wc * t_out_new - q_ihx) / (c_ihx / hs + wc);
        let c_pool = self.pool_sodium_mass.value * sodium_heat_capacity(temps.pool)?.value;
        let t_p_new = (c_pool * t_in / hs + wc * t_x_new) / (c_pool / hs + wc);

        let new = Temperatures {
            fuel: ThermodynamicTemperature::new::<kelvin>(t_f_new),
            core_mean: ThermodynamicTemperature::new::<kelvin>(t_c_new),
            ihx_outlet: ThermodynamicTemperature::new::<kelvin>(t_x_new),
            pool: ThermodynamicTemperature::new::<kelvin>(t_p_new),
        };
        // Range check: every sodium node, and the core outlet, must stay in
        // TUAS's sodium range (below boiling).
        for t in [new.core_mean, new.ihx_outlet, new.pool, new.core_outlet()] {
            sodium_density(t)?;
        }
        if !t_f_new.is_finite() {
            return Err(Ebr2Error::Invalid("fuel temperature is not finite".to_string()));
        }
        self.temperatures = new;
        self.primary_flow = flow;
        self.fission_power = fission;
        self.decay_power = decay;
        self.inputs = inputs;
        self.time = t_end;
        Ok(())
    }

    fn evaluate_reactivity(&self) -> Result<Ebr2Reactivity, Ebr2Error> {
        match &self.power {
            PowerState::Tabulated => Ok(Ebr2Reactivity::zero()),
            PowerState::Kinetics(k) => {
                let temps = &self.temperatures;
                let state = FastReactorState {
                    fuel_temperature: temps.fuel,
                    cladding_temperature: temps.core_mean,
                    coolant_temperature: temps.core_mean,
                    coolant_density: sodium_density(temps.core_mean)?,
                    radial_structure_temperatures: vec![radial_temperature(
                        k.radial_expansion_temperature,
                        temps,
                    )],
                    driveline_temperature: temps.core_outlet(),
                };
                Ok(Ebr2Reactivity {
                    feedback: k.feedback.evaluate(&state)?,
                    external: self.external_reactivity,
                })
            }
        }
    }

    // ----- setters -------------------------------------------------------

    /// Override the pump speeds (`None` returns to the test's table).
    pub fn set_pump_speeds(&mut self, speeds: Option<(AngularVelocity, AngularVelocity)>) {
        self.overrides.pump_speeds = speeds;
    }

    /// Override the intermediate inlet flow (`None` returns to the table).
    pub fn set_intermediate_flow(&mut self, flow: Option<MassRate>) {
        self.overrides.intermediate_flow = flow;
    }

    /// Override the intermediate inlet temperature (`None` returns to the
    /// table).
    pub fn set_intermediate_inlet_temperature(&mut self, t: Option<ThermodynamicTemperature>) {
        self.overrides.intermediate_inlet_temperature = t;
    }

    /// Override the auxiliary EM pump current (`None` returns to the table).
    pub fn set_aux_em_pump_current(&mut self, current: Option<ElectricCurrent>) {
        self.overrides.aux_em_pump_current = current;
    }

    /// Set the externally inserted reactivity (e.g. a scram or a rod
    /// withdrawal). Point-kinetics mode only: in tabulated mode the power
    /// is a boundary condition, so this returns an error.
    pub fn set_external_reactivity(&mut self, rho: Ratio) -> Result<(), Ebr2Error> {
        match self.power {
            PowerState::Tabulated => Err(Ebr2Error::Invalid(
                "external reactivity has no effect on tabulated power".to_string(),
            )),
            PowerState::Kinetics(_) => {
                self.external_reactivity = rho;
                Ok(())
            }
        }
    }

    // ----- getters -------------------------------------------------------

    /// Time since the start of the test.
    pub fn time(&self) -> Time {
        self.time
    }
    /// The configuration.
    pub fn config(&self) -> &Ebr2Config {
        &self.config
    }
    /// The derived geometry.
    pub fn derived_geometry(&self) -> DerivedGeometry {
        self.derived
    }
    /// Fission power.
    pub fn fission_power(&self) -> Power {
        self.fission_power
    }
    /// Decay-heat power.
    pub fn decay_power(&self) -> Power {
        self.decay_power
    }
    /// Total power (fission + decay): the power deposited over the last
    /// sub-step. In tabulated mode that is the table at the sub-step's
    /// midpoint; in kinetics mode, the end-of-sub-step value.
    pub fn total_power(&self) -> Power {
        self.fission_power + self.decay_power
    }
    /// Fuel (volume-mean) temperature.
    pub fn fuel_temperature(&self) -> ThermodynamicTemperature {
        self.temperatures.fuel
    }
    /// Core inlet temperature (= pool).
    pub fn core_inlet_temperature(&self) -> ThermodynamicTemperature {
        self.temperatures.pool
    }
    /// Core mean coolant temperature.
    pub fn core_mean_coolant_temperature(&self) -> ThermodynamicTemperature {
        self.temperatures.core_mean
    }
    /// Mixed-mean core outlet temperature.
    pub fn core_outlet_temperature(&self) -> ThermodynamicTemperature {
        self.temperatures.core_outlet()
    }
    /// IHX primary outlet temperature.
    pub fn ihx_primary_outlet_temperature(&self) -> ThermodynamicTemperature {
        self.temperatures.ihx_outlet
    }
    /// Pool temperature.
    pub fn pool_temperature(&self) -> ThermodynamicTemperature {
        self.temperatures.pool
    }
    /// Primary mass flow.
    pub fn primary_flow(&self) -> MassRate {
        self.primary_flow
    }
    /// `(P / P0) / (W / W0)`, the normalised power-to-flow ratio the
    /// benchmark reports.
    pub fn power_to_flow_ratio(&self) -> Ratio {
        let p = self.total_power() / self.initial_total_power;
        let w = self.primary_flow / self.initial_primary_flow;
        p / w
    }
    /// The reactivity terms (zero in tabulated mode).
    pub fn reactivity(&self) -> Ebr2Reactivity {
        self.reactivity
    }
    /// beta-eff in kinetics mode.
    pub fn beta_eff(&self) -> Option<Ratio> {
        match &self.power {
            PowerState::Tabulated => None,
            PowerState::Kinetics(k) => Some(k.beta_eff),
        }
    }
    /// The boundary values used by the last step.
    pub fn inputs(&self) -> Ebr2Inputs {
        self.inputs
    }
    /// The current overrides.
    pub fn overrides(&self) -> InputOverrides {
        self.overrides
    }
    /// The model's steady state against the cited initial conditions.
    pub fn initial_comparison(&self) -> InitialComparison {
        self.initial_comparison
    }
    /// The fuel-to-coolant resistance breakdown at the current state.
    pub fn conductance_breakdown(&self) -> Result<ConductanceBreakdown, Ebr2Error> {
        fuel_to_coolant(&self.config, &self.derived, &self.temperatures, self.primary_flow)
    }
    /// The IHX `(UA, effectiveness)` at the current state.
    pub fn ihx_state(&self) -> Result<(ThermalConductance, f64), Ebr2Error> {
        let (ua, eps, _) = ihx_effectiveness(
            &self.config,
            &self.derived,
            &self.temperatures,
            self.primary_flow,
            &self.inputs,
        )?;
        Ok((ua, eps))
    }
    /// Sodium masses `(core, IHX node, pool)`.
    pub fn sodium_masses(&self) -> (Mass, Mass, Mass) {
        (self.core_sodium_mass, self.ihx_sodium_mass, self.pool_sodium_mass)
    }
}

impl ConductanceBreakdown {
    fn ua_si(&self) -> ThermalConductance {
        ThermalConductance::new::<uom::si::thermal_conductance::watt_per_kelvin>(self.ua)
    }
}

// ---------------------------------------------------------------------------
// Physics helpers
// ---------------------------------------------------------------------------

fn evaluate_inputs(
    test: ShrtTest,
    overrides: &InputOverrides,
    t: Time,
) -> Result<Ebr2Inputs, Ebr2Error> {
    Ok(Ebr2Inputs {
        pump_speeds: overrides.pump_speeds.unwrap_or_else(|| test.pump_speeds(t)),
        intermediate_flow: match overrides.intermediate_flow {
            Some(f) => f,
            None => test.intermediate_flow(t)?,
        },
        intermediate_inlet_temperature: overrides
            .intermediate_inlet_temperature
            .unwrap_or_else(|| test.intermediate_inlet_temperature(t)),
        aux_em_pump_current: overrides
            .aux_em_pump_current
            .unwrap_or_else(|| test.aux_em_pump_current(t)),
    })
}

/// Decay heat after a pre-test history of **total** thermal powers (as A226
/// states them), from a cold start.
///
/// Decay heat is linear in the fission power, so one factor
/// `x = P_last / (P_last + D_1)` converts every segment's total power to
/// fission power. Here `D_1` is the decay heat at the end of the history
/// when the totals are used unscaled. With the factor, fission plus decay
/// equals the last segment's power at its end, and the short-lived groups
/// end in equilibrium with that fission power. Feeding the totals in
/// directly would leave them about 5 % high, and they would relax in the
/// first seconds of the transient (found by
/// `initial_state_is_stationary_with_inputs_held`, 2026-11-10).
pub fn history_decay_heat(history: &[(Power, Time)]) -> DecayHeat {
    let run = |scale: f64| {
        let mut decay = DecayHeat::new(FissioningNuclide::U235Thermal);
        for (p, dt) in history {
            decay.advance_timestep(*p * scale, *dt);
        }
        decay
    };
    let unscaled = run(1.0);
    match history.last() {
        Some((p_last, _)) if p_last.value > 0.0 => {
            let d1 = unscaled.total_decay_heat_power();
            run((*p_last / (*p_last + d1)).get::<ratio>())
        }
        _ => unscaled,
    }
}

fn radial_temperature(
    which: RadialExpansionTemperature,
    temps: &Temperatures,
) -> ThermodynamicTemperature {
    match which {
        RadialExpansionTemperature::CoreInlet => temps.pool,
        RadialExpansionTemperature::CoreMeanCoolant => temps.core_mean,
        RadialExpansionTemperature::CoreOutlet => temps.core_outlet(),
    }
}

/// The feedback set with every reference at the initial steady state.
fn build_feedback(k: &KineticsConfig, temps: &Temperatures) -> Result<FastReactorFeedback, Ebr2Error> {
    let c = &k.coefficients;
    let doppler_constant = Ratio::new::<ratio>(
        -c.doppler_per_kelvin.get::<per_kelvin>() * c.doppler_nominal_temperature.get::<kelvin>(),
    );
    let slope = sodium_density_slope(temps.core_mean)?;
    let a_rho = c.sodium_density_per_kelvin.get::<per_kelvin>() / slope;
    Ok(FastReactorFeedback {
        doppler: Some(FastDopplerTerm {
            doppler_constant,
            reference_temperature: temps.fuel,
        }),
        axial_fuel_expansion: Some(LinearTemperatureTerm {
            coefficient: c.axial_fuel_expansion,
            reference_temperature: temps.fuel,
        }),
        cladding: None,
        coolant_temperature: None,
        coolant_density: Some(CoolantDensityTerm {
            coefficient: SpecificVolume::new::<uom::si::specific_volume::cubic_meter_per_kilogram>(
                a_rho,
            ),
            reference_density: sodium_density(temps.core_mean)?,
        }),
        radial_expansion: vec![LinearTemperatureTerm {
            coefficient: c.radial_expansion,
            reference_temperature: radial_temperature(k.radial_expansion_temperature, temps),
        }],
        driveline: k.driveline.clone(),
    })
}

/// The heat capacity used for every primary advective term and for the IHX
/// primary capacity rate: sodium's at the core mean temperature. One value
/// for the whole loop keeps the energy carried round it consistent between
/// nodes (an enthalpy formulation would remove the need; CoolProp's c_p
/// varies by about 1 % over the loop's 100 K).
fn loop_heat_capacity(temps: &Temperatures) -> Result<SpecificHeatCapacity, TuasLibError> {
    sodium_heat_capacity(temps.core_mean)
}

/// Net loop head at total flow `w_total` (kg/s): pumps + aux + buoyancy -
/// loop resistance, Pa.
fn net_head(config: &Ebr2Config, inputs: &Ebr2Inputs, buoyancy_pa: f64, w_total: f64) -> f64 {
    let w = w_total / (2.0 * design::PUMP_RATED_FLOW_KG_PER_S);
    let s1 = inputs.pump_speeds.0.get::<revolution_per_minute>() / design::PUMP_RATED_SPEED_RPM;
    let s2 = inputs.pump_speeds.1.get::<revolution_per_minute>() / design::PUMP_RATED_SPEED_RPM;
    let pumps = 0.5 * (pump::head_ratio(s1, w) + pump::head_ratio(s2, w));
    let aux = if config.aux_em_pump {
        let [psi, _, _, _, amps] = design::AUX_EM_PUMP_RATING;
        Pressure::new::<pound_force_per_square_inch>(psi).get::<pascal>()
            * inputs.aux_em_pump_current.get::<ampere>()
            / amps
    } else {
        0.0
    };
    design::PUMP_RATED_HEAD_PA * (pumps - w * w.abs()) + aux + buoyancy_pa
}

/// Quasi-static primary flow by bisection (see the module doc).
fn solve_primary_flow(
    config: &Ebr2Config,
    inputs: &Ebr2Inputs,
    temps: &Temperatures,
) -> Result<MassRate, Ebr2Error> {
    let rho_cold = sodium_density(temps.pool)?.value;
    let rho_hot = sodium_density(temps.core_outlet())?.value;
    let buoyancy =
        STANDARD_GRAVITY * config.thermal_centre_height.value * (rho_cold - rho_hot);
    let f = |w: f64| net_head(config, inputs, buoyancy, w);
    if !(f(0.0) > 0.0) {
        return Ok(MassRate::ZERO);
    }
    let mut hi = 2.0 * design::PUMP_RATED_FLOW_KG_PER_S;
    let mut guard = 0;
    while f(hi) > 0.0 {
        hi *= 2.0;
        guard += 1;
        if guard > 40 {
            return Err(Ebr2Error::Invalid("primary flow did not bracket".to_string()));
        }
    }
    let mut lo = 0.0;
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        if f(mid) > 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Ok(MassRate::new::<kilogram_per_second>(0.5 * (lo + hi)))
}

/// Per-pin resistances and the core UA at `temps` and `flow`.
fn fuel_to_coolant(
    config: &Ebr2Config,
    derived: &DerivedGeometry,
    temps: &Temperatures,
    flow: MassRate,
) -> Result<ConductanceBreakdown, Ebr2Error> {
    let pi = std::f64::consts::PI;
    let d_f = design::fuel_slug_diameter().value;
    let d_ci = design::cladding_inner_diameter().value;
    let d_co = design::cladding_outer_diameter().value;
    let k_f = design::u5fs_conductivity(temps.fuel).value;
    let k_na = sodium_conductivity(temps.core_mean)?.value;
    let k_ss = steel_conductivity(temps.core_mean)?.value;
    let rho = sodium_density(temps.core_mean)?.value;
    let mu = sodium_viscosity(temps.core_mean)?.value;
    let cp = sodium_heat_capacity(temps.core_mean)?.value;
    let per_sa = flow.value * design::high_pressure_plenum_flow_share().get::<ratio>()
        / f64::from(HIGH_PRESSURE_POSITIONS);
    let velocity = per_sa / (rho * derived.bundle_flow_area_m2);
    let d_h = derived.bundle_hydraulic_diameter_m;
    let reynolds = rho * velocity * d_h / mu;
    let prandtl = mu * cp / k_na;
    let peclet = reynolds * prandtl;
    let nusselt = config
        .geometry
        .core_nusselt
        .nusselt(Ratio::new::<ratio>(peclet))
        .get::<ratio>();
    let h = nusselt * k_na / d_h;
    let fuel = 1.0 / (8.0 * pi * k_f);
    let bond = (d_ci / d_f).r_ln() / (2.0 * pi * k_na);
    let cladding = (d_co / d_ci).r_ln() / (2.0 * pi * k_ss);
    let film = 1.0 / (h * pi * d_co);
    let ua = derived.pins * design::FUEL_SLUG_LENGTH_M / (fuel + bond + cladding + film);
    Ok(ConductanceBreakdown {
        fuel,
        bond,
        cladding,
        film,
        peclet,
        nusselt,
        ua,
    })
}

/// IHX `(UA, effectiveness, C_min)` at the primary flow `flow` and the
/// intermediate boundary values, counter-flow.
fn ihx_effectiveness(
    config: &Ebr2Config,
    derived: &DerivedGeometry,
    temps: &Temperatures,
    flow: MassRate,
    inputs: &Ebr2Inputs,
) -> Result<(ThermalConductance, f64, f64), Ebr2Error> {
    let pi = std::f64::consts::PI;
    let t_p = temps.core_outlet();
    let t_s = inputs.intermediate_inlet_temperature;
    let w_p = flow.value.max(0.0);
    let w_s = inputs.intermediate_flow.value.max(0.0);
    let d_o = design::ihx_tube_outer_diameter().value;
    let d_i = design::ihx_tube_inner_diameter().value;
    let n_t = f64::from(design::IHX_TUBE_COUNT);
    let l = design::IHX_TUBE_LENGTH_M;

    let shell = |w: f64, t: ThermodynamicTemperature| -> Result<f64, Ebr2Error> {
        let (rho, mu, k, cp) = (
            sodium_density(t)?.value,
            sodium_viscosity(t)?.value,
            sodium_conductivity(t)?.value,
            sodium_heat_capacity(t)?.value,
        );
        let v = w / (rho * derived.ihx_shell_flow_area_m2);
        let pe = rho * v * derived.ihx_shell_hydraulic_diameter_m / mu * (mu * cp / k);
        let nu = LiquidMetalCorrelation::Mikityuk {
            pitch_to_diameter: Ratio::new::<ratio>(derived.ihx_pitch_to_diameter),
        }
        .nusselt(Ratio::new::<ratio>(pe))
        .get::<ratio>();
        Ok(nu * k / derived.ihx_shell_hydraulic_diameter_m)
    };
    let tube = |w: f64, t: ThermodynamicTemperature| -> Result<f64, Ebr2Error> {
        let (rho, mu, k, cp) = (
            sodium_density(t)?.value,
            sodium_viscosity(t)?.value,
            sodium_conductivity(t)?.value,
            sodium_heat_capacity(t)?.value,
        );
        let v = w / (rho * derived.ihx_tube_flow_area_m2);
        let pe = rho * v * d_i / mu * (mu * cp / k);
        let nu = LiquidMetalCorrelation::LyonMartinelli
            .nusselt(Ratio::new::<ratio>(pe))
            .get::<ratio>();
        Ok(nu * k / d_i)
    };
    let (h_o, h_i) = match config.geometry.ihx_primary_side {
        IhxPrimarySide::Shell => (shell(w_p, t_p)?, tube(w_s, t_s)?),
        IhxPrimarySide::Tube => (shell(w_s, t_s)?, tube(w_p, t_p)?),
    };
    let t_wall = ThermodynamicTemperature::new::<kelvin>(0.5 * (t_p.get::<kelvin>() + t_s.get::<kelvin>()));
    let k_ss = steel_conductivity(t_wall)?.value;
    let r_per_length =
        1.0 / (h_i * pi * d_i) + (d_o / d_i).r_ln() / (2.0 * pi * k_ss) + 1.0 / (h_o * pi * d_o);
    let ua = n_t * l / r_per_length;
    let c_p = w_p * loop_heat_capacity(temps)?.value;
    let c_s = w_s * sodium_heat_capacity(t_s)?.value;
    let (c_min, c_max) = if c_p < c_s { (c_p, c_s) } else { (c_s, c_p) };
    let eps = counterflow_effectiveness(ua, c_min, c_max);
    Ok((
        ThermalConductance::new::<uom::si::thermal_conductance::watt_per_kelvin>(ua),
        eps,
        c_min,
    ))
}

/// Counter-flow effectiveness (e.g. Incropera & DeWitt Table 11.3):
/// `eps = (1 - exp(-N(1-Cr))) / (1 - Cr exp(-N(1-Cr)))`, and
/// `N/(1+N)` at `Cr = 1`; zero when either capacity rate is zero.
pub fn counterflow_effectiveness(ua: f64, c_min: f64, c_max: f64) -> f64 {
    if !(c_min > 0.0) || !(c_max > 0.0) {
        return 0.0;
    }
    let ntu = ua / c_min;
    let cr = c_min / c_max;
    if (1.0 - cr).abs() < 1e-9 {
        return ntu / (1.0 + ntu);
    }
    let e = (-ntu * (1.0 - cr)).r_exp();
    (1.0 - e) / (1.0 - cr * e)
}
