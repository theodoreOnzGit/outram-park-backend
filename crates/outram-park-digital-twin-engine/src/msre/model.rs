//! # A web-ready MSRE stepping model
//!
//! [`MsreModel`] advances the whole MSRE heat-transport chain in time: the
//! circulating-fuel point kinetics, the core's fuel and graphite, the primary
//! heat exchanger, the coolant loop and the air radiator, joined by plug-flow
//! piping. It is GUI-free, deterministic (no clock, no RNG, no I/O), cheap
//! (about a microsecond per 0.01 s step), and compiles for `wasm32`. A web
//! demo drives it from a Web Worker and reads its getters to draw.
//!
//! ## What it is built from (reused, not re-derived)
//!
//! - **Kinetics:** ~~`teh_o_prke`'s `CirculatingPointKinetics` (MSRE T-C1,
//!   gh:#1131), unchanged~~ **CHANGED 2026-11-10 (gh:#1332):**
//!   [`super::circulation::CirculatingKinetics`], which carries a flow
//!   change, with TM-1070 App. B's six groups ([`super::design::LAMBDA`],
//!   [`super::design::BETA`]) and TM-1070 Table 1's Λ, core transit 8.46 s
//!   and loop transit 16.73 s by default ([`CirculationModel::WellStirredTm1070`]).
//!   At full flow it is `teh_o_prke`'s `CirculatingPointKinetics` step for
//!   step (pinned by a differential test to 1e-12). ORNL-TM-1626's
//!   importance-weighted line model is the named option
//!   [`CirculationModel::LineTm1626`] (gh:#1330).
//! - **Thermal network:** ORNL-TM-1070's (Ball and Kerlin, 1965) lumped
//!   equations, with the coefficients `nee_soon::msre_frequency_response`
//!   (MSRE T-1, gh:#1132) uses, read from there:
//!   - core (A.1)-(A.3): two fuel lumps and a graphite lump; reactivity
//!     follows the first fuel lump and the graphite (importances 1, 0, 1);
//!   - a delayed-gamma share 0.0564 with a 188 s lag (Table 1);
//!   - heat exchanger and radiator (A.32)-(A.37): two lumps per fluid, a
//!     tube, and the exchanger's shell;
//!   - piping as pure transport delays (App. B p. 70): core → HX 5.77 s,
//!     HX → core 8.67 s, HX → radiator 4.71 s, radiator → HX 8.24 s;
//!   - the radiator air side scaled with air flow as ORNL scaled it (TM-1070
//!     p. 60, [`HxSection::radiator_at`]).
//!
//! `nee_soon` solves these equations linearised, in the frequency domain.
//! Here the same equations are integrated in time with backward Euler, and
//! the kinetics keep their nonlinear `ρ·n` term. The frequency response is
//! the check: driven by a small sinusoidal reactivity, this model's
//! amplitude and phase reproduce `nee_soon`'s (see the tests).
//!
//! ## What is new here, and so new physics run uncalibrated
//!
//! 1. **Absolute temperatures.** TM-1070's model is a deviation model. The
//!    same linear equations carry absolute temperatures once one boundary
//!    value is fixed: the radiator air inlet, 100 °F (TM-0728 Table 8.2).
//!    Nothing else sets a temperature level; the steady state follows from
//!    the power, the air flow and that inlet. It is compared, uncalibrated,
//!    with TM-0728's design temperatures in the tests, and the disagreement
//!    is recorded (`docs/msre_library.md`).
//! 2. **Coupling by operator splitting.** Each step takes the reactivity from
//!    the previous step's temperatures, advances the kinetics, then the
//!    thermal network at the new power. First order in `dt`.
//! 3. **Feedback reference.** The temperature feedback is measured from the
//!    initial steady state, which is critical with no rod reactivity:
//!    `ρ = Δρ_circ + ρ_rod + α_F (F₁ − F₁⁰) + α_G (G − G⁰)`, where `Δρ_circ`
//!    is the circulation loss that holds the circulating core critical.
//!
//! ## Pump trips and flow changes (gh:#1332, 2026-11-10)
//!
//! ~~**Pump trips or any change of fuel or coolant flow** are not modelled;
//! the transits are fixed.~~ **CORRECTED 2026-11-10:** the fuel and coolant
//! flows follow a [`super::pumps::FlowSchedule`] after
//! [`MsreModel::trip_fuel_pump`], [`MsreModel::trip_coolant_pump`],
//! [`MsreModel::start_fuel_pump`] or [`MsreModel::start_coolant_pump`], or
//! are set directly. A flow fraction `f` enters as follows, every term
//! derived, none fitted:
//!
//! - **Circulating precursors:** the kinetics' loop is indexed by fluid
//!   displacement ([`super::circulation`]).
//! - **Transits** scale as `1/f`: the core lumps' `τ_F`, the heat-exchanger
//!   and radiator salt lumps' `t`, and the four piping lags (plug flow,
//!   advanced by displacement).
//! - **Salt-side heat transfer** in the heat exchanger and the radiator
//!   scales as `h ∝ f^0.8` (Dittus-Boelter, `Nu ∝ Re^0.8`; the salt flows
//!   are turbulent, Re ≈ 240 000 primary and 120 000 secondary, TM-1070
//!   p. 16; MSRE TH-1, gh:#1134, found Dittus-Boelter within +10 % of
//!   ORNL-TM-4079's turbulent salt data). That is the same scaling
//!   TM-1070 p. 60 applies to the air side with exponent 0.6. Hence
//!   `n = hA/(W c_p) ∝ f^-0.2` and the tube time constants `∝ f^-0.8`.
//! - **Core fuel-to-graphite `hA`** does not change: the core flow is
//!   laminar (Re ≈ 1000, TM-1070 p. 14), where the fully developed Nusselt
//!   number does not depend on the flow.
//!
//! **Limits, stated rather than hidden:**
//!
//! - **No natural circulation.** No MSRE natural-circulation flow was found
//!   in TM-0728, TM-1070, ORNL-4233 or TM-2987, so the flow goes where the
//!   schedule takes it, to zero, and the forced-convection terms with it.
//! - **The two-lump sections are Mann's lumping, valid for `n < 2` per
//!   lump** (TM-1070 ref. 18). With `n ∝ f^-0.2` the heat exchanger's fuel
//!   side passes `n = 2` below about 24 % fuel flow, its coolant side and
//!   the radiator's below about 5 % and 2 % coolant flow. Below that the
//!   heat-exchanger temperatures are outside the lumping's validity;
//!   [`MsreModel::thermal_lumping_within_validity`] reports it. A
//!   multi-section exchanger (TM-1070's complete model used ten) would
//!   remove the limit; it is not built.
//! - The core flux shape is held: no spatial kinetics after a trip.
//! - **Salt freezing and the radiator doors.** Nothing stops a salt
//!   cooling below its 840 °F liquidus (TM-0728 Table 2.1). After a
//!   fuel-pump trip at power with the air flow left on, the coolant reaches
//!   471 °F in 300 s (test `pump_trips_at_power_stay_finite`); the plant
//!   would have closed the radiator doors. A caller should cut the air flow
//!   ([`MsreModel::set_radiator_air_flow_fraction`]).
//!
//! Measured against ORNL-4233 Fig. 24 (zero-power coast-down and start-up
//! held critical by the rod servo): see `docs/msre_library.md` and the
//! tests in `model/pump_tests.rs`. Cost: about 0.7 µs per step at constant
//! flow, 5 µs while a flow changes (the network is refactored), 38 µs with
//! [`CirculationModel::LineTm1626`] (2026-11-10, one shared core).
//!
//! ## What it does not model (stated so it is not mistaken for a gap filled)
//!
//! - Xenon, the drain tank and freeze valve (`tampines`' generic components
//!   exist; the MSRE-specific model is the MSRE repository's, gh:#1136),
//!   spatial effects, decay heat after shutdown beyond the 188 s
//!   delayed-gamma lag.
//! - A lumped core gives volume averages; it cannot answer a peak-temperature
//!   question.
//!
//! ## Status: NOT VALIDATED
//!
//! AI-drafted (Claude Opus 5.5 in Claude Code), untrusted until reviewed.
//! Research, education and V&V only: an offline demonstration model, never
//! for operation, licensing or safety decisions (`RESPONSIBLE_USE.md`).
//! This module is a maintainer-directed exception to this crate's "no new
//! physics in the library" rule (2026-11-10, gh:#1317: "I would like to have
//! MSRE and EBR-II in library so I can build web simulations from the
//! outram-park metacrate"), on the [`crate::htr10`] pattern.

use std::collections::VecDeque;

use petir::linalg::{lu_decomp, lu_svx, LuPermutation, Matrix};
use petir::mathf::RealMath;
use uom::si::f64::{Power, Ratio, TemperatureCoefficient, ThermodynamicTemperature, Time};
use uom::si::power::megawatt;
use uom::si::ratio::ratio;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::thermodynamic_temperature::degree_fahrenheit;
use uom::si::time::second;

pub use super::circulation::CirculationModel;
use super::circulation::CirculatingKinetics;
use super::design::{
    per_f, HxSection, MsreLoopModel, GAMMA_LAG, GAMMA_SHARE, HA, K_F, K_G, MC_F, MC_G,
    ORNL_1965_FUEL_PER_F, ORNL_1965_GRAPHITE_PER_F, TAU_F,
};
use super::pumps::FlowSchedule;

/// The fixed internal step, 0.01 s. TM-1070's piping lags and transits are
/// given to 0.01 s, so every delay is a whole number of steps and nothing is
/// interpolated. [`MsreModel::step`] takes any `dt` and runs as many of these
/// as fit, carrying the remainder.
pub const INTERNAL_TIMESTEP_S: f64 = 0.01;

/// Design power the radiator air side is scaled from (TM-1070 p. 60:
/// ORNL's radiator coefficients are given at 10 MW).
const RADIATOR_REFERENCE_MW: f64 = 10.0;

// Lump indices.
const F1: usize = 0;
const F2: usize = 1;
const G: usize = 2;
const HX: usize = 3; // 3..=8: coolant T1, T1out, tube, fuel T2, T2out, shell
const RAD: usize = 9; // 9..=13: coolant T1, T1out, tube, air T2, T2out
const N: usize = 14;
// Exogenous inputs (piping outlets and the held air inlet).
const U_CORE_IN: usize = 0;
const U_HX_FUEL_IN: usize = 1;
const U_HX_COOLANT_IN: usize = 2;
const U_RAD_COOLANT_IN: usize = 3;
const U_AIR_IN: usize = 4;
const NU: usize = 5;

/// Errors from building or stepping an [`MsreModel`].
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum MsreModelError {
    /// An input was out of its physical range (stated in the message).
    #[error("invalid MSRE model input: {0}")]
    InvalidInput(String),
    /// The kinetics solver refused a step or a construction.
    #[error("MSRE kinetics: {0}")]
    Kinetics(String),
    /// A thermal-network matrix was singular.
    #[error("MSRE thermal network: singular matrix ({0})")]
    Singular(String),
}

/// What an [`MsreModel`] starts from. [`MsreConfig::tm1070`] gives the cited
/// defaults; every field may be changed before [`MsreModel::new`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MsreConfig {
    /// Initial fission power. The model starts in steady state here.
    pub initial_power: Power,
    /// Radiator air flow as a fraction of the 10 MW design flow (200 000 cfm,
    /// TM-0728 Table 8.2). ORNL ran the radiator with air flow proportional
    /// to power (TM-1070 p. 60), which is what [`MsreConfig::tm1070`] sets.
    pub radiator_air_flow_fraction: Ratio,
    /// Radiator air inlet temperature (TM-0728 Table 8.2: 100 °F).
    pub radiator_air_inlet: ThermodynamicTemperature,
    /// Fuel-salt temperature coefficient of reactivity.
    pub fuel_temperature_coefficient: TemperatureCoefficient,
    /// Graphite temperature coefficient of reactivity.
    pub graphite_temperature_coefficient: TemperatureCoefficient,
    /// Which ORNL circulating-fuel model the kinetics use (default
    /// [`CirculationModel::WellStirredTm1070`]; gh:#1330).
    pub circulation_model: CirculationModel,
}

impl MsreConfig {
    /// TM-1070's 1965 parameter set at `initial_power`: Table 1's predicted
    /// coefficients (fuel −4.84, graphite −3.7 × 10⁻⁵ /°F), the radiator air
    /// flow proportional to power, and the 100 °F air inlet of TM-0728
    /// Table 8.2. For the workspace's own Monte Carlo coefficients (MSRE N-3,
    /// gh:#1129: fuel −3.82 ± 0.44, graphite −3.77 ± 0.65 × 10⁻⁵ /°F), set the
    /// two coefficient fields with [`super::design::per_f`].
    pub fn tm1070(initial_power: Power) -> Self {
        Self {
            initial_power,
            radiator_air_flow_fraction: Ratio::new::<ratio>(
                initial_power.get::<megawatt>() / RADIATOR_REFERENCE_MW,
            ),
            radiator_air_inlet: ThermodynamicTemperature::new::<degree_fahrenheit>(100.0),
            fuel_temperature_coefficient: per_f(ORNL_1965_FUEL_PER_F),
            graphite_temperature_coefficient: per_f(ORNL_1965_GRAPHITE_PER_F),
            circulation_model: CirculationModel::WellStirredTm1070,
        }
    }
}

/// The linear thermal network `dx/dt = A x + B u + h q`, in TM-1070's units
/// (°F, s, MW): `x` the 14 lumps, `u` the 5 inlet temperatures, `q` the
/// heating power.
#[derive(Debug, Clone, PartialEq)]
struct Network {
    a: [[f64; N]; N],
    b: [[f64; NU]; N],
    h: [f64; N],
}

/// The salt-side heat-transfer coefficient at flow fraction `f`, relative
/// to full flow: `f^0.8` (Dittus-Boelter; see the module docs), zero at
/// zero flow.
fn salt_h_ratio(f: f64) -> f64 {
    if f > 0.0 {
        f.r_powf(0.8)
    } else {
        0.0
    }
}

/// The fuel and coolant flow fractions a [`Network`] is built at.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Flows {
    fuel: f64,
    coolant: f64,
}

impl Flows {
    const FULL: Flows = Flows { fuel: 1.0, coolant: 1.0 };
}

impl Network {
    /// The network with heat exchanger `hx` and radiator `rad` at full flow.
    fn new(hx: &HxSection, rad: &HxSection) -> Self {
        Self::at_flows(hx, rad, Flows::FULL)
    }

    /// The network at the flow fractions `flows`: transits `∝ 1/f`, salt
    /// heat transfer `∝ f^0.8`, core `hA` fixed (module docs). At full flow
    /// every coefficient is bit-identical to the fixed-flow network.
    fn at_flows(hx: &HxSection, rad: &HxSection, flows: Flows) -> Self {
        let ff = flows.fuel;
        let mut a = [[0.0; N]; N];
        let mut b = [[0.0; NU]; N];
        let mut h = [0.0; N];
        // Core, TM-1070 (A.1)-(A.3); graphite heat is shared equally by the
        // two fuel lumps and driven by F1, as in nee_soon's rows.
        let share = 0.5;
        let x = share * HA / MC_F;
        a[F1][F1] = -ff / TAU_F - x;
        a[F1][G] = x;
        b[F1][U_CORE_IN] = ff / TAU_F;
        h[F1] = K_F / MC_F;
        a[F2][F2] = -ff / TAU_F;
        a[F2][F1] = ff / TAU_F - x;
        a[F2][G] = x;
        h[F2] = K_F / MC_F;
        a[G][G] = -HA / MC_G;
        a[G][F1] = HA / MC_G;
        h[G] = 2.0 * K_G / MC_G;
        let coolant = SideFlow { f: flows.coolant, h: salt_h_ratio(flows.coolant) };
        let fuel = SideFlow { f: ff, h: salt_h_ratio(ff) };
        let air = SideFlow { f: 1.0, h: 1.0 };
        section(&mut a, &mut b, HX, hx, U_HX_COOLANT_IN, U_HX_FUEL_IN, true, coolant, fuel);
        section(&mut a, &mut b, RAD, rad, U_RAD_COOLANT_IN, U_AIR_IN, false, coolant, air);
        Self { a, b, h }
    }
}

/// One fluid's flow fraction `f` and heat-transfer ratio `h` in a section.
#[derive(Debug, Clone, Copy, PartialEq)]
struct SideFlow {
    f: f64,
    h: f64,
}

/// Rows `base..` of one heat-exchange section, TM-1070 (A.32)-(A.37): fluid 1
/// (in the tube) lumps T1, T1out; the tube; fluid 2 lumps T2, T2out; and,
/// with `shell`, the shell. Fluid 1 enters from input `in1`, fluid 2 from
/// `in2`. The same coefficients as `nee_soon`'s `hx_rows` at full flow
/// (`f = h = 1`). Below full flow the flow terms `2/t` scale with `f` and
/// the heat-transfer terms `n/t` and `1/τ` with `h` (module docs); the shell
/// is on fluid 2's side.
#[allow(clippy::too_many_arguments)]
fn section(
    a: &mut [[f64; N]; N],
    b: &mut [[f64; NU]; N],
    base: usize,
    s: &HxSection,
    in1: usize,
    in2: usize,
    shell: bool,
    side1: SideFlow,
    side2: SideFlow,
) {
    let (t1, t1o, tt, t2, t2o, ts) = (base, base + 1, base + 2, base + 3, base + 4, base + 5);
    let ns = if shell { s.ns } else { 0.0 };
    let (f1, h1, f2, h2) = (side1.f, side1.h, side2.f, side2.h);
    a[t1][t1] = -(2.0 * f1 + s.n1 * h1) / s.t1;
    b[t1][in1] = 2.0 * f1 / s.t1;
    a[t1][tt] = s.n1 * h1 / s.t1;
    a[t1o][t1o] = -2.0 * f1 / s.t1;
    a[t1o][t1] = 2.0 * f1 / s.t1 - s.n1 * h1 / s.t1;
    a[t1o][tt] = s.n1 * h1 / s.t1;
    a[tt][tt] = -(h1 / s.tau_t1 + h2 / s.tau_t2);
    a[tt][t1] = h1 / s.tau_t1;
    a[tt][t2] = h2 / s.tau_t2;
    a[t2][t2] = -(2.0 * f2 + (s.n2 + ns) * h2) / s.t2;
    b[t2][in2] = 2.0 * f2 / s.t2;
    a[t2][tt] = s.n2 * h2 / s.t2;
    a[t2o][t2o] = -2.0 * f2 / s.t2;
    a[t2o][t2] = 2.0 * f2 / s.t2 - (s.n2 + ns) * h2 / s.t2;
    a[t2o][tt] = s.n2 * h2 / s.t2;
    if shell {
        a[t2][ts] = ns * h2 / s.t2;
        a[t2o][ts] = ns * h2 / s.t2;
        a[ts][ts] = -h2 / s.tau_s;
        a[ts][t2] = h2 / s.tau_s;
    }
}

/// Mann's two-lump section is valid while `n < 2` per lump (TM-1070 ref.
/// 18); `n_eff = n h / f` at reduced flow. False at zero flow.
fn lump_valid(n: f64, side: SideFlow) -> bool {
    side.f > 0.0 && n * side.h / side.f < 2.0
}

/// One plug-flow pipe of `steps` cells, each the salt that passes in one
/// step at full flow. The salt moves one cell whenever the accumulated
/// displacement passes one cell: at full flow every step, the outlet then
/// being the inlet `steps` steps ago.
#[derive(Debug, Clone, PartialEq)]
struct Pipe {
    /// Inlet temperatures, oldest first; the front is the current outlet.
    history: VecDeque<f64>,
    /// Displacement not yet moved, in cells.
    displacement: f64,
}

impl Pipe {
    /// A pipe of `steps` steps, full of salt at `t`.
    fn full_at(steps: usize, t: f64) -> Self {
        Self { history: std::iter::repeat(t).take(steps).collect(), displacement: 0.0 }
    }

    /// The temperature leaving now.
    fn outlet(&self) -> f64 {
        self.history.front().copied().unwrap_or(f64::NAN)
    }

    /// Move the salt by this step's displacement `flow` (cells): push the
    /// inlet and drop the oldest once per whole cell.
    fn advance(&mut self, inlet: f64, flow: f64) {
        self.displacement += flow;
        while self.displacement >= 1.0 - 1e-9 {
            self.history.push_back(inlet);
            self.history.pop_front();
            self.displacement = (self.displacement - 1.0).max(0.0);
        }
    }
}

/// Whole number of internal steps in `seconds`, or an error.
fn whole_steps(name: &str, seconds: f64) -> Result<usize, MsreModelError> {
    let n = (seconds / INTERNAL_TIMESTEP_S).round();
    if n < 1.0 || ((n * INTERNAL_TIMESTEP_S - seconds) / seconds).abs() > 1e-9 {
        return Err(MsreModelError::InvalidInput(format!(
            "{name} {seconds} s is not a whole number of {INTERNAL_TIMESTEP_S} s steps"
        )));
    }
    Ok(n as usize)
}

/// Solve the dense system `m x = rhs` once (steady state).
fn solve_dense(m: Vec<f64>, n: usize, rhs: &[f64], what: &str) -> Result<Vec<f64>, MsreModelError> {
    let (lu, p) = factor(m, n, what)?;
    let mut x = rhs.to_vec();
    lu_svx(&lu, &p, &mut x).map_err(|e| MsreModelError::Singular(format!("{what}: {e:?}")))?;
    Ok(x)
}

/// LU-factor the row-major `n × n` matrix `m`.
fn factor(m: Vec<f64>, n: usize, what: &str) -> Result<(Matrix, LuPermutation), MsreModelError> {
    let mut lu = Matrix::from_row_major(n, n, m).map_err(|e| MsreModelError::Singular(format!("{what}: {e:?}")))?;
    let p = lu_decomp(&mut lu).map_err(|e| MsreModelError::Singular(format!("{what}: {e:?}")))?;
    if p.zero_pivot.is_some() {
        return Err(MsreModelError::Singular(format!("{what}: zero pivot")));
    }
    Ok((lu, p))
}

/// The MSRE stepping simulator. See the module docs for the model and its
/// limits.
///
/// ```
/// use outram_park_digital_twin_engine::msre::model::{MsreConfig, MsreModel};
/// use uom::si::f64::{Power, Ratio, Time};
/// use uom::si::{power::megawatt, ratio::ratio, time::second};
///
/// let mut m = MsreModel::new(MsreConfig::tm1070(Power::new::<megawatt>(8.0))).unwrap();
/// m.set_rod_reactivity(Ratio::new::<ratio>(1.0e-4));
/// m.step(Time::new::<second>(1.0)).unwrap();
/// assert!(m.power().get::<megawatt>() > 8.0);
/// ```
#[derive(Debug, Clone)]
pub struct MsreModel {
    params: MsreLoopModel,
    kinetics: CirculatingKinetics,
    circulation_loss: f64,
    /// Fuel and coolant flow fractions of the last step.
    flows: Flows,
    /// The flows the `network` was built at.
    network_flows: Flows,
    fuel_pump: PumpDrive,
    coolant_pump: PumpDrive,
    p0_mw: f64,
    /// Fission power \[MW\].
    power_mw: f64,
    /// Delayed-gamma power \[MW\].
    gamma_mw: f64,
    x: [f64; N],
    pipes: [Pipe; 4],
    air_in_f: f64,
    air_fraction: f64,
    network: Network,
    lu: Matrix,
    perm: LuPermutation,
    rho_rod: f64,
    f1_ref: f64,
    g_ref: f64,
    steps: u64,
    remainder_s: f64,
}

/// What sets one loop's flow: a fixed fraction, or a schedule and the time
/// since its event.
#[derive(Debug, Clone, PartialEq)]
enum PumpDrive {
    Fixed(f64),
    Scheduled { schedule: FlowSchedule, since_s: f64 },
}

impl PumpDrive {
    /// The flow fraction the next step of `dt` will use, without advancing.
    fn peek(&self, dt: f64) -> f64 {
        match self {
            PumpDrive::Fixed(f) => *f,
            PumpDrive::Scheduled { schedule, since_s } => {
                schedule.flow_fraction(Time::new::<second>(*since_s + dt)).get::<ratio>()
            }
        }
    }

    /// The flow fraction for a step ending `dt` from now, advancing the
    /// schedule clock.
    fn next(&mut self, dt: f64) -> f64 {
        match self {
            PumpDrive::Fixed(f) => *f,
            PumpDrive::Scheduled { schedule, since_s } => {
                *since_s += dt;
                schedule.flow_fraction(Time::new::<second>(*since_s)).get::<ratio>()
            }
        }
    }
}

// Pipe order in `MsreModel::pipes`: core→HX (fuel), HX→core (fuel),
// HX→radiator (coolant), radiator→HX (coolant).
const P_CORE_HX: usize = 0;
const P_HX_CORE: usize = 1;
const P_HX_RAD: usize = 2;
const P_RAD_HX: usize = 3;

impl MsreModel {
    /// A model in steady state at `config.initial_power`, critical with no
    /// rod reactivity. Errors if the power or air flow is not positive.
    pub fn new(config: MsreConfig) -> Result<Self, MsreModelError> {
        let p0 = config.initial_power.get::<megawatt>();
        let f = config.radiator_air_flow_fraction.get::<ratio>();
        if !(p0 > 0.0 && p0.is_finite()) {
            return Err(MsreModelError::InvalidInput(format!("initial power must be > 0, got {p0} MW")));
        }
        if !(f > 0.0 && f.is_finite()) {
            return Err(MsreModelError::InvalidInput(format!("radiator air flow fraction must be > 0, got {f}")));
        }
        let params = MsreLoopModel::tm1070(
            config.initial_power,
            config.fuel_temperature_coefficient,
            config.graphite_temperature_coefficient,
        );
        let kinetics = CirculatingKinetics::new(config.circulation_model, params.generation_time, INTERNAL_TIMESTEP_S)
            .map_err(|e| MsreModelError::Kinetics(format!("{e:?}")))?;
        let circulation_loss = kinetics.circulation_loss();
        let air_in_f = config.radiator_air_inlet.get::<degree_fahrenheit>();
        let network = Network::new(&params.hx, &HxSection::radiator_at(RADIATOR_REFERENCE_MW * f));
        let x = steady_state(&network, p0, air_in_f)?;
        let steps: Vec<usize> = ["core-to-HX lag", "HX-to-core lag", "HX-to-radiator lag", "radiator-to-HX lag"]
            .iter()
            .zip(params.delays.iter())
            .map(|(name, &d)| whole_steps(name, d))
            .collect::<Result<_, _>>()?;
        let pipes = [
            Pipe::full_at(steps[P_CORE_HX], x[F2]),
            Pipe::full_at(steps[P_HX_CORE], x[HX + 4]),
            Pipe::full_at(steps[P_HX_RAD], x[HX + 1]),
            Pipe::full_at(steps[P_RAD_HX], x[RAD + 1]),
        ];
        let (lu, perm) = factor(implicit_matrix(&network), N, "backward-Euler matrix")?;
        Ok(Self {
            params,
            kinetics,
            circulation_loss,
            flows: Flows::FULL,
            network_flows: Flows::FULL,
            fuel_pump: PumpDrive::Fixed(1.0),
            coolant_pump: PumpDrive::Fixed(1.0),
            p0_mw: p0,
            power_mw: p0,
            gamma_mw: p0,
            x,
            pipes,
            air_in_f,
            air_fraction: f,
            network,
            lu,
            perm,
            rho_rod: 0.0,
            f1_ref: x[F1],
            g_ref: x[G],
            steps: 0,
            remainder_s: 0.0,
        })
    }

    /// Advance by `dt`: as many [`INTERNAL_TIMESTEP_S`] steps as fit, the
    /// remainder carried to the next call. Errors for a negative or
    /// non-finite `dt`, or if the kinetics refuse a step.
    pub fn step(&mut self, dt: Time) -> Result<(), MsreModelError> {
        let dt = dt.get::<second>();
        if !(dt >= 0.0 && dt.is_finite()) {
            return Err(MsreModelError::InvalidInput(format!("dt must be >= 0, got {dt} s")));
        }
        let total = self.remainder_s + dt;
        let n = (total / INTERNAL_TIMESTEP_S + 1e-9).floor();
        self.remainder_s = (total - n * INTERNAL_TIMESTEP_S).max(0.0);
        for _ in 0..n as u64 {
            self.step_once()?;
        }
        Ok(())
    }

    /// One internal step: kinetics at the previous temperatures' reactivity,
    /// then the delayed-gamma lag, then the thermal network by backward Euler
    /// with the piping outlets as known inlets.
    fn step_once(&mut self) -> Result<(), MsreModelError> {
        let dt = INTERNAL_TIMESTEP_S;
        self.flows = Flows { fuel: self.fuel_pump.next(dt), coolant: self.coolant_pump.next(dt) };
        if self.flows != self.network_flows {
            self.rebuild_network()?;
        }
        let rho = self.circulation_loss + self.net_reactivity_value();
        self.kinetics
            .step(rho, self.flows.fuel, 0.0)
            .map_err(|e| MsreModelError::Kinetics(format!("{e:?}")))?;
        self.power_mw = self.p0_mw * self.kinetics.neutron_density();
        self.gamma_mw = (self.gamma_mw + dt / GAMMA_LAG * self.power_mw) / (1.0 + dt / GAMMA_LAG);
        let q = (1.0 - GAMMA_SHARE) * self.power_mw + GAMMA_SHARE * self.gamma_mw;
        let mut u = [0.0; NU];
        u[U_CORE_IN] = self.pipes[P_HX_CORE].outlet();
        u[U_HX_FUEL_IN] = self.pipes[P_CORE_HX].outlet();
        u[U_HX_COOLANT_IN] = self.pipes[P_RAD_HX].outlet();
        u[U_RAD_COOLANT_IN] = self.pipes[P_HX_RAD].outlet();
        u[U_AIR_IN] = self.air_in_f;
        let mut rhs = [0.0; N];
        for (i, r) in rhs.iter_mut().enumerate() {
            *r = self.x[i] / dt + self.network.h[i] * q;
            for (k, uk) in u.iter().enumerate() {
                *r += self.network.b[i][k] * uk;
            }
        }
        let mut xn = rhs.to_vec();
        lu_svx(&self.lu, &self.perm, &mut xn).map_err(|e| MsreModelError::Singular(format!("step: {e:?}")))?;
        self.x.copy_from_slice(&xn);
        let (ff, fc) = (self.flows.fuel, self.flows.coolant);
        self.pipes[P_CORE_HX].advance(self.x[F2], ff);
        self.pipes[P_HX_CORE].advance(self.x[HX + 4], ff);
        self.pipes[P_HX_RAD].advance(self.x[HX + 1], fc);
        self.pipes[P_RAD_HX].advance(self.x[RAD + 1], fc);
        self.steps += 1;
        Ok(())
    }

    /// `ρ_rod + α_F (F₁ − F₁⁰) + α_G (G − G⁰)`, as a plain number.
    fn net_reactivity_value(&self) -> f64 {
        self.rho_rod
            + self.params.alpha_fuel * (self.x[F1] - self.f1_ref)
            + self.params.alpha_graphite * (self.x[G] - self.g_ref)
    }

    // ── Inputs ──────────────────────────────────────────────────────────────

    /// Set the reactivity inserted by the control rods, relative to the
    /// initial critical state (positive withdraws). Takes effect on the next
    /// step.
    pub fn set_rod_reactivity(&mut self, rho: Ratio) {
        self.rho_rod = rho.get::<ratio>();
    }

    /// Set the radiator air flow as a fraction of the 10 MW design flow
    /// (how the MSRE's power was controlled above a few hundred kW:
    /// TM-0728 §2.4.11, "the power is controlled by regulating the air flow"). The air side is rescaled as
    /// TM-1070 p. 60 does (`h_air ∝ W_air^0.6`). ORNL used that scaling from
    /// 0.076 to 10 MW; outside about 0.0076 to 1 it is an extrapolation.
    /// Errors for a fraction that is not positive.
    pub fn set_radiator_air_flow_fraction(&mut self, fraction: Ratio) -> Result<(), MsreModelError> {
        let f = fraction.get::<ratio>();
        if !(f > 0.0 && f.is_finite()) {
            return Err(MsreModelError::InvalidInput(format!("radiator air flow fraction must be > 0, got {f}")));
        }
        let previous = self.air_fraction;
        self.air_fraction = f;
        if let Err(e) = self.rebuild_network() {
            self.air_fraction = previous;
            return Err(e);
        }
        Ok(())
    }

    /// Rebuild the network and its factorisation at the present air flow and
    /// salt flows.
    fn rebuild_network(&mut self) -> Result<(), MsreModelError> {
        let rad = HxSection::radiator_at(RADIATOR_REFERENCE_MW * self.air_fraction);
        let network = Network::at_flows(&self.params.hx, &rad, self.flows);
        let (lu, perm) = factor(implicit_matrix(&network), N, "backward-Euler matrix")?;
        self.network = network;
        self.lu = lu;
        self.perm = perm;
        self.network_flows = self.flows;
        Ok(())
    }

    /// Trip the fuel pump: from the next step the fuel flow follows
    /// `coastdown` from its time zero (e.g.
    /// [`FlowSchedule::ornl4233_fuel_pump_speed_coastdown`]).
    pub fn trip_fuel_pump(&mut self, coastdown: FlowSchedule) {
        self.fuel_pump = PumpDrive::Scheduled { schedule: coastdown, since_s: 0.0 };
    }

    /// Trip the coolant pump: the coolant flow follows `coastdown` (e.g.
    /// [`FlowSchedule::ornl4233_coolant_salt_flow_coastdown`], measured).
    pub fn trip_coolant_pump(&mut self, coastdown: FlowSchedule) {
        self.coolant_pump = PumpDrive::Scheduled { schedule: coastdown, since_s: 0.0 };
    }

    /// Start the fuel pump: the fuel flow follows `startup` (e.g.
    /// [`FlowSchedule::tm0728_fuel_flow_startup`]).
    pub fn start_fuel_pump(&mut self, startup: FlowSchedule) {
        self.fuel_pump = PumpDrive::Scheduled { schedule: startup, since_s: 0.0 };
    }

    /// Start the coolant pump: the coolant flow follows `startup`.
    pub fn start_coolant_pump(&mut self, startup: FlowSchedule) {
        self.coolant_pump = PumpDrive::Scheduled { schedule: startup, since_s: 0.0 };
    }

    /// Hold the fuel flow at `fraction` of full flow from the next step.
    /// Errors for a fraction that is negative or not finite.
    pub fn set_fuel_flow_fraction(&mut self, fraction: Ratio) -> Result<(), MsreModelError> {
        self.fuel_pump = PumpDrive::Fixed(flow_value("fuel", fraction)?);
        Ok(())
    }

    /// Hold the coolant flow at `fraction` of full flow from the next step.
    pub fn set_coolant_flow_fraction(&mut self, fraction: Ratio) -> Result<(), MsreModelError> {
        self.coolant_pump = PumpDrive::Fixed(flow_value("coolant", fraction)?);
        Ok(())
    }

    /// The fuel flow fraction of the last step.
    pub fn fuel_flow_fraction(&self) -> Ratio {
        Ratio::new::<ratio>(self.flows.fuel)
    }

    /// The coolant flow fraction of the last step.
    pub fn coolant_flow_fraction(&self) -> Ratio {
        Ratio::new::<ratio>(self.flows.coolant)
    }

    /// The circulating-fuel model in use.
    pub fn circulation_model(&self) -> CirculationModel {
        self.kinetics.model()
    }

    /// The rod reactivity that holds the reactor at its present power over
    /// the next step, at the flow that step will have and the present
    /// temperature feedback: what an ideal flux servo would set, as ORNL's
    /// did in ORNL-4233 Fig. 24 ("the reactivity added by the rod is then
    /// equal (and opposite) to the reactivity change due to the flow
    /// perturbations"). Exact for the kinetics; the feedback is the
    /// present step's (operator splitting), so at zero power it is exact.
    pub fn critical_rod_reactivity(&self) -> Ratio {
        let feedback = self.net_reactivity_value() - self.rho_rod;
        let flow = self.fuel_pump.peek(INTERNAL_TIMESTEP_S);
        let n = self.kinetics.neutron_density();
        Ratio::new::<ratio>(self.kinetics.holding_reactivity(flow, n) - self.circulation_loss - feedback)
    }

    /// Whether every heat-exchanger and radiator lump is inside Mann's
    /// lumping validity, `n_eff < 2`, at the present flows (module docs).
    pub fn thermal_lumping_within_validity(&self) -> bool {
        let hx = &self.params.hx;
        let rad = HxSection::radiator_at(RADIATOR_REFERENCE_MW * self.air_fraction);
        let coolant = SideFlow { f: self.flows.coolant, h: salt_h_ratio(self.flows.coolant) };
        let fuel = SideFlow { f: self.flows.fuel, h: salt_h_ratio(self.flows.fuel) };
        lump_valid(hx.n1, coolant) && lump_valid(hx.n2 + hx.ns, fuel) && lump_valid(rad.n1, coolant)
    }

    /// Set the radiator air inlet temperature.
    pub fn set_radiator_air_inlet(&mut self, t: ThermodynamicTemperature) {
        self.air_in_f = t.get::<degree_fahrenheit>();
    }

    // ── State ───────────────────────────────────────────────────────────────

    /// Simulated time since construction.
    pub fn time(&self) -> Time {
        Time::new::<second>(self.steps as f64 * INTERNAL_TIMESTEP_S)
    }

    /// The fission power (neutron population times the initial power).
    pub fn power(&self) -> Power {
        Power::new::<megawatt>(self.power_mw)
    }

    /// The heat deposited in the core: the prompt share of the fission power
    /// plus the delayed-gamma share, which lags it by 188 s.
    pub fn heat_deposition(&self) -> Power {
        Power::new::<megawatt>((1.0 - GAMMA_SHARE) * self.power_mw + GAMMA_SHARE * self.gamma_mw)
    }

    /// Net reactivity relative to critical: rods plus temperature feedback.
    /// Zero in the initial steady state.
    pub fn reactivity(&self) -> Ratio {
        Ratio::new::<ratio>(self.net_reactivity_value())
    }

    /// The temperature-feedback part of [`Self::reactivity`].
    pub fn feedback_reactivity(&self) -> Ratio {
        Ratio::new::<ratio>(self.net_reactivity_value() - self.rho_rod)
    }

    /// The rod reactivity last set.
    pub fn rod_reactivity(&self) -> Ratio {
        Ratio::new::<ratio>(self.rho_rod)
    }

    /// The reactivity the circulating core needs to stay critical, against
    /// the stationary core: the steady full-flow circulation loss of the
    /// kinetics' model, with TM-1070's transits by default (compare
    /// [`super::design::measured_circulation_loss_tm1626`]). It is the
    /// reference the feedback is measured from, fixed at construction; after
    /// a pump trip the kinetics regain it, which [`Self::critical_rod_reactivity`]
    /// shows.
    pub fn circulation_loss(&self) -> Ratio {
        Ratio::new::<ratio>(self.circulation_loss)
    }

    /// Each group's delayed-neutron source in the core, `Λ λ_i C_i / n₀`, in
    /// units of the initial prompt production `n₀/Λ` (for
    /// [`CirculationModel::LineTm1626`], importance-weighted over the flow
    /// path). In a stationary critical core these equal β_i; with
    /// circulation they are smaller, by the precursors decaying where their
    /// neutrons count for less or nothing.
    pub fn delayed_neutron_sources(&self) -> Vec<Ratio> {
        let lam = self.params.generation_time;
        self.kinetics.delayed_sources().iter().map(|d| Ratio::new::<ratio>(lam * d)).collect()
    }

    /// Fuel entering the core (leaving the HX-to-core pipe).
    pub fn core_inlet_temperature(&self) -> ThermodynamicTemperature {
        fahrenheit(self.pipes[P_HX_CORE].outlet())
    }

    /// Fuel leaving the core (the second fuel lump).
    pub fn core_outlet_temperature(&self) -> ThermodynamicTemperature {
        fahrenheit(self.x[F2])
    }

    /// The first fuel lump, which carries the fuel's reactivity feedback.
    pub fn fuel_lump_temperature(&self) -> ThermodynamicTemperature {
        fahrenheit(self.x[F1])
    }

    /// The graphite lump.
    pub fn graphite_temperature(&self) -> ThermodynamicTemperature {
        fahrenheit(self.x[G])
    }

    /// Fuel entering the heat exchanger (leaving the core-to-HX pipe).
    pub fn hx_fuel_inlet_temperature(&self) -> ThermodynamicTemperature {
        fahrenheit(self.pipes[P_CORE_HX].outlet())
    }

    /// Fuel leaving the heat exchanger.
    pub fn hx_fuel_outlet_temperature(&self) -> ThermodynamicTemperature {
        fahrenheit(self.x[HX + 4])
    }

    /// Coolant entering the heat exchanger (leaving the radiator-to-HX pipe).
    pub fn hx_coolant_inlet_temperature(&self) -> ThermodynamicTemperature {
        fahrenheit(self.pipes[P_RAD_HX].outlet())
    }

    /// Coolant leaving the heat exchanger.
    pub fn hx_coolant_outlet_temperature(&self) -> ThermodynamicTemperature {
        fahrenheit(self.x[HX + 1])
    }

    /// Coolant leaving the radiator.
    pub fn radiator_coolant_outlet_temperature(&self) -> ThermodynamicTemperature {
        fahrenheit(self.x[RAD + 1])
    }

    /// Air leaving the radiator.
    pub fn radiator_air_outlet_temperature(&self) -> ThermodynamicTemperature {
        fahrenheit(self.x[RAD + 4])
    }

    /// The radiator air flow fraction last set.
    pub fn radiator_air_flow_fraction(&self) -> Ratio {
        Ratio::new::<ratio>(self.air_fraction)
    }

    /// The fuel temperature coefficient in use.
    pub fn fuel_temperature_coefficient(&self) -> TemperatureCoefficient {
        TemperatureCoefficient::new::<per_kelvin>(self.params.alpha_fuel * 1.8)
    }

    /// The graphite temperature coefficient in use.
    pub fn graphite_temperature_coefficient(&self) -> TemperatureCoefficient {
        TemperatureCoefficient::new::<per_kelvin>(self.params.alpha_graphite * 1.8)
    }

    /// The TM-1070 parameter set the model runs on (transits, Λ, the HX, the
    /// piping lags). Its radiator is the one at the *initial* air flow.
    pub fn loop_parameters(&self) -> &MsreLoopModel {
        &self.params
    }
}

/// A flow fraction checked to be finite and `≥ 0`.
fn flow_value(name: &str, fraction: Ratio) -> Result<f64, MsreModelError> {
    let f = fraction.get::<ratio>();
    if !(f >= 0.0 && f.is_finite()) {
        return Err(MsreModelError::InvalidInput(format!("{name} flow fraction must be >= 0, got {f}")));
    }
    Ok(f)
}

/// A °F value as a temperature.
fn fahrenheit(t: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_fahrenheit>(t)
}

/// `I/dt − A`, row-major.
fn implicit_matrix(net: &Network) -> Vec<f64> {
    let mut m = vec![0.0; N * N];
    for i in 0..N {
        for j in 0..N {
            m[i * N + j] = -net.a[i][j];
        }
        m[i * N + i] += 1.0 / INTERNAL_TIMESTEP_S;
    }
    m
}

/// The steady state at heating power `q` \[MW\] (the delayed gamma at its
/// steady share) and air inlet `air_in` \[°F\]: `A x + B u(x) + h q = 0`,
/// with each pipe's outlet equal to its inlet.
fn steady_state(net: &Network, q: f64, air_in: f64) -> Result<[f64; N], MsreModelError> {
    // u as a function of x: which lump feeds each input through its pipe.
    let feeds = [(U_CORE_IN, HX + 4), (U_HX_FUEL_IN, F2), (U_HX_COOLANT_IN, RAD + 1), (U_RAD_COOLANT_IN, HX + 1)];
    let mut m = vec![0.0; N * N];
    let mut rhs = vec![0.0; N];
    for i in 0..N {
        for j in 0..N {
            m[i * N + j] = -net.a[i][j];
        }
        for &(k, from) in &feeds {
            m[i * N + from] -= net.b[i][k];
        }
        rhs[i] = net.h[i] * q + net.b[i][U_AIR_IN] * air_in;
    }
    let x = solve_dense(m, N, &rhs, "steady state")?;
    let mut out = [0.0; N];
    out.copy_from_slice(&x);
    Ok(out)
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod pump_tests;
