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
//! - **Kinetics:** `teh_o_prke`'s [`CirculatingPointKinetics`] (MSRE T-C1,
//!   gh:#1131), unchanged, with TM-1070 App. B's six groups
//!   ([`super::design::LAMBDA`], [`super::design::BETA`]) and TM-1070
//!   Table 1's Λ, core transit 8.46 s and loop transit 16.73 s.
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
//! ## What it does not model (stated so it is not mistaken for a gap filled)
//!
//! - **Pump trips or any change of fuel or coolant flow.** The transits are
//!   fixed. Variable flow needs variable-transit circulating kinetics in
//!   `teh-o-prke` and flow-dependent lumps; that is gh:#1332, not a
//!   setter that silently does nothing.
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
use teh_o_prke::zero_power_prke::circulating::CirculatingPointKinetics;
use uom::si::f64::{
    Frequency, Power, Ratio, TemperatureCoefficient, ThermodynamicTemperature, Time,
    VolumetricNumberDensity, VolumetricNumberRate,
};
use uom::si::frequency::hertz;
use uom::si::power::megawatt;
use uom::si::ratio::ratio;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::thermodynamic_temperature::degree_fahrenheit;
use uom::si::time::second;
use uom::si::volumetric_number_density::per_cubic_meter;
use uom::si::volumetric_number_rate::per_cubic_meter_second;

use super::design::{
    per_f, HxSection, MsreLoopModel, BETA, GAMMA_LAG, GAMMA_SHARE, HA, K_F, K_G, LAMBDA, MC_F,
    MC_G, ORNL_1965_FUEL_PER_F, ORNL_1965_GRAPHITE_PER_F, TAU_F,
};

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

impl Network {
    /// The network with heat exchanger `hx` and radiator `rad`.
    fn new(hx: &HxSection, rad: &HxSection) -> Self {
        let mut a = [[0.0; N]; N];
        let mut b = [[0.0; NU]; N];
        let mut h = [0.0; N];
        // Core, TM-1070 (A.1)-(A.3); graphite heat is shared equally by the
        // two fuel lumps and driven by F1, as in nee_soon's rows.
        let share = 0.5;
        let x = share * HA / MC_F;
        a[F1][F1] = -1.0 / TAU_F - x;
        a[F1][G] = x;
        b[F1][U_CORE_IN] = 1.0 / TAU_F;
        h[F1] = K_F / MC_F;
        a[F2][F2] = -1.0 / TAU_F;
        a[F2][F1] = 1.0 / TAU_F - x;
        a[F2][G] = x;
        h[F2] = K_F / MC_F;
        a[G][G] = -HA / MC_G;
        a[G][F1] = HA / MC_G;
        h[G] = 2.0 * K_G / MC_G;
        section(&mut a, &mut b, HX, hx, U_HX_COOLANT_IN, U_HX_FUEL_IN, true);
        section(&mut a, &mut b, RAD, rad, U_RAD_COOLANT_IN, U_AIR_IN, false);
        Self { a, b, h }
    }
}

/// Rows `base..` of one heat-exchange section, TM-1070 (A.32)-(A.37): fluid 1
/// (in the tube) lumps T1, T1out; the tube; fluid 2 lumps T2, T2out; and,
/// with `shell`, the shell. Fluid 1 enters from input `in1`, fluid 2 from
/// `in2`. The same coefficients as `nee_soon`'s `hx_rows`.
fn section(a: &mut [[f64; N]; N], b: &mut [[f64; NU]; N], base: usize, s: &HxSection, in1: usize, in2: usize, shell: bool) {
    let (t1, t1o, tt, t2, t2o, ts) = (base, base + 1, base + 2, base + 3, base + 4, base + 5);
    let ns = if shell { s.ns } else { 0.0 };
    a[t1][t1] = -(2.0 + s.n1) / s.t1;
    b[t1][in1] = 2.0 / s.t1;
    a[t1][tt] = s.n1 / s.t1;
    a[t1o][t1o] = -2.0 / s.t1;
    a[t1o][t1] = 2.0 / s.t1 - s.n1 / s.t1;
    a[t1o][tt] = s.n1 / s.t1;
    a[tt][tt] = -(1.0 / s.tau_t1 + 1.0 / s.tau_t2);
    a[tt][t1] = 1.0 / s.tau_t1;
    a[tt][t2] = 1.0 / s.tau_t2;
    a[t2][t2] = -(2.0 + s.n2 + ns) / s.t2;
    b[t2][in2] = 2.0 / s.t2;
    a[t2][tt] = s.n2 / s.t2;
    a[t2o][t2o] = -2.0 / s.t2;
    a[t2o][t2] = 2.0 / s.t2 - (s.n2 + ns) / s.t2;
    a[t2o][tt] = s.n2 / s.t2;
    if shell {
        a[t2][ts] = ns / s.t2;
        a[t2o][ts] = ns / s.t2;
        a[ts][ts] = -1.0 / s.tau_s;
        a[ts][t2] = 1.0 / s.tau_s;
    }
}

/// One plug-flow pipe: the outlet is the inlet `steps` steps ago.
#[derive(Debug, Clone, PartialEq)]
struct Pipe {
    /// Inlet temperatures, oldest first; the front is the current outlet.
    history: VecDeque<f64>,
}

impl Pipe {
    /// A pipe of `steps` steps, full of salt at `t`.
    fn full_at(steps: usize, t: f64) -> Self {
        Self { history: std::iter::repeat(t).take(steps).collect() }
    }

    /// The temperature leaving now.
    fn outlet(&self) -> f64 {
        self.history.front().copied().unwrap_or(f64::NAN)
    }

    /// Push this step's inlet and drop the oldest.
    fn advance(&mut self, inlet: f64) {
        self.history.push_back(inlet);
        self.history.pop_front();
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
    kinetics: CirculatingPointKinetics,
    circulation_loss: f64,
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
        let time = Time::new::<second>;
        let kinetics = CirculatingPointKinetics::with_groups(
            BETA.iter().map(|&b| Ratio::new::<ratio>(b)).collect(),
            LAMBDA.iter().map(|&l| Frequency::new::<hertz>(l)).collect(),
            time(params.core_transit),
            time(params.loop_transit),
            time(params.generation_time),
            time(INTERNAL_TIMESTEP_S),
        )
        .map_err(|e| MsreModelError::Kinetics(format!("{e:?}")))?;
        let circulation_loss = kinetics.circulation_reactivity_loss().get::<ratio>();
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
        let rho = self.circulation_loss + self.net_reactivity_value();
        self.kinetics
            .step_implicit(Ratio::new::<ratio>(rho), VolumetricNumberRate::new::<per_cubic_meter_second>(0.0))
            .map_err(|e| MsreModelError::Kinetics(format!("{e:?}")))?;
        self.power_mw = self.p0_mw * self.kinetics.neutron_density().get::<per_cubic_meter>();
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
        self.pipes[P_CORE_HX].advance(self.x[F2]);
        self.pipes[P_HX_CORE].advance(self.x[HX + 4]);
        self.pipes[P_HX_RAD].advance(self.x[HX + 1]);
        self.pipes[P_RAD_HX].advance(self.x[RAD + 1]);
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
        let network = Network::new(&self.params.hx, &HxSection::radiator_at(RADIATOR_REFERENCE_MW * f));
        let (lu, perm) = factor(implicit_matrix(&network), N, "backward-Euler matrix")?;
        self.network = network;
        self.lu = lu;
        self.perm = perm;
        self.air_fraction = f;
        Ok(())
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
    /// the stationary core: the steady circulation loss of the kinetics with
    /// TM-1070's transits (compare [`super::design::measured_circulation_loss_tm1626`]).
    pub fn circulation_loss(&self) -> Ratio {
        Ratio::new::<ratio>(self.circulation_loss)
    }

    /// Each group's delayed-neutron source in the core, `Λ λ_i C_i / n₀`, in
    /// units of the initial prompt production `n₀/Λ`. In a stationary
    /// critical core these equal β_i; with circulation they are smaller, by
    /// the precursors decaying outside the core.
    pub fn delayed_neutron_sources(&self) -> Vec<Ratio> {
        let lam = self.params.generation_time;
        self.kinetics
            .precursor_densities()
            .iter()
            .zip(LAMBDA.iter())
            .map(|(c, l): (&VolumetricNumberDensity, &f64)| Ratio::new::<ratio>(lam * l * c.get::<per_cubic_meter>()))
            .collect()
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
