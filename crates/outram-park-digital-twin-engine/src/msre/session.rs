//! # An operator session on [`MsreModel`], in plain numbers
//!
//! What a GUI or a web worker needs to drive the MSRE plant model without
//! touching `uom` types: operator commands ([`Command`]), a flat
//! [`Snapshot`] of the getters in the units MSRE's reports used (°F, MW,
//! pcm), a deterministic headless CSV ([`headless_csv`]) and the T-1
//! natural-period table ([`natural_periods`]). Built for the museum MSRE
//! simulator (`examples/msre_web`, gh:#1570).
//!
//! It adds **no physics**: every number comes from [`MsreModel`] or
//! `nee_soon`'s frequency-response model. The only logic here is
//! bookkeeping: applying commands, the optional ideal flux servo
//! ([`MsreModel::critical_rod_reactivity`] applied every internal step) and
//! formatting.
//!
//! Research, education and V&V only. NOT VALIDATED. AI draft (gh:#1570).

use uom::si::f64::{Power, Ratio, Time};
use uom::si::power::megawatt;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::degree_fahrenheit;
use uom::si::time::second;

use super::design::{per_f, MsreLoopModel, MEASURED_PERIODS_TM1647, ORNL_1965_FUEL_PER_F, ORNL_1965_GRAPHITE_PER_F};
use super::model::{MsreConfig, MsreModel, MsreModelError};
use super::pumps::FlowSchedule;

/// The model's internal step, [s]: the servo acts once per step.
const STEP_S: f64 = 0.01;

/// The longest stretch of plant time one [`Session::advance`] call runs, [s].
/// A worker handles one request at a time, so a request must stay short
/// (`bugis::web_demo::link`); at about 50 µs a step natively this is a few
/// tenths of a second of computing.
pub const MAX_ADVANCE_S: f64 = 10.0;

/// Which fuel and graphite temperature coefficients the plant uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coefficients {
    /// ORNL-TM-1070 Table 1's 1965 predictions: fuel −4.84, graphite
    /// −3.7 × 10⁻⁵ /°F (`MsreConfig::tm1070`'s default).
    Ornl1965,
    /// This workspace's Monte Carlo split on the N-1 model's current default
    /// (dimensions at temperature, whole shield, U-234/U-236 salt): fuel
    /// −3.79 ± 0.37, graphite −3.26 ± 0.33 × 10⁻⁵ /°F (MSRE N-3,
    /// gh:#1129, 2026-11-13; record in
    /// `crates/outram-mc-libs/verification_and_validation/msre_n1_first_criticality/README.md`).
    /// The high-fidelity level feeding the low-fidelity one.
    MonteCarloN3,
}

impl Coefficients {
    /// Fuel and graphite coefficients, [1/°F].
    pub fn per_f(self) -> (f64, f64) {
        match self {
            Coefficients::Ornl1965 => (ORNL_1965_FUEL_PER_F, ORNL_1965_GRAPHITE_PER_F),
            Coefficients::MonteCarloN3 => (MC_N3_FUEL_PER_F, MC_N3_GRAPHITE_PER_F),
        }
    }
}

/// The Monte Carlo fuel coefficient on the current N-1 default, [1/°F]
/// (see [`Coefficients::MonteCarloN3`]).
pub const MC_N3_FUEL_PER_F: f64 = -3.79e-5;
/// The Monte Carlo graphite coefficient on the current N-1 default, [1/°F].
pub const MC_N3_GRAPHITE_PER_F: f64 = -3.26e-5;

/// How a session starts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SessionConfig {
    /// Initial fission power, [MW]. The MSRE ran up to about 8 MW.
    pub power_mw: f64,
    pub coefficients: Coefficients,
    /// `true` (the default) for ORNL-TM-1626's line model; `false` for the
    /// well-stirred ablation (`MsreConfig::without_line_model`).
    pub line_model: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self { power_mw: 8.0, coefficients: Coefficients::Ornl1965, line_model: true }
    }
}

impl SessionConfig {
    /// The [`MsreConfig`] this session builds its model from.
    pub fn model_config(&self) -> MsreConfig {
        let mut c = MsreConfig::tm1070(Power::new::<megawatt>(self.power_mw));
        let (f, g) = self.coefficients.per_f();
        c.fuel_temperature_coefficient = per_f(f);
        c.graphite_temperature_coefficient = per_f(g);
        if self.line_model {
            c
        } else {
            c.without_line_model()
        }
    }
}

/// What an operator can do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Command {
    /// Rod reactivity relative to the initial critical state, [pcm];
    /// positive withdraws. Turns the servo off.
    RodsPcm(f64),
    /// The ideal flux servo on or off: on, the rods hold the present power
    /// (ORNL-4233 Fig. 24's servo, `MsreModel::critical_rod_reactivity`).
    Servo(bool),
    /// Radiator air flow as a fraction of the 10 MW design flow. ORNL
    /// controlled power above a few hundred kW this way.
    AirFraction(f64),
    /// Fuel pump trip: ORNL-4233 Fig. 23's measured speed coast-down.
    TripFuelPump,
    /// Coolant pump trip: ORNL-4233 Fig. 23's measured coolant-salt flow.
    TripCoolantPump,
    /// Fuel pump start: TM-0728's start-up schedule.
    StartFuelPump,
    /// Coolant pump start, on the same start-up schedule (no coolant one is
    /// published).
    StartCoolantPump,
}

/// The fuel salt's liquidus, [°F]: ORNL-TM-0728 Table 2.1, as cited in
/// `MsreModel`'s docs and TUAS's `msre_fuel_salt_tm0728`.
pub const FUEL_LIQUIDUS_F: f64 = 840.0;
/// The coolant salt's liquidus, [°F]: 458 °C (the L₂B peritectic,
/// ORNL-TM-2316 p. 2, as cited in TUAS's `msre_coolant_salt_tm2316`).
pub const COOLANT_LIQUIDUS_F: f64 = 458.0 * 9.0 / 5.0 + 32.0;

/// The plant now, in MSRE's report units.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Snapshot {
    pub time_s: f64,
    pub power_mw: f64,
    /// Heat deposited in the salt (prompt plus delayed gamma), [MW].
    pub heat_mw: f64,
    pub reactivity_pcm: f64,
    pub rod_pcm: f64,
    pub feedback_pcm: f64,
    /// The reactivity the circulating precursors cost, [pcm].
    pub circulation_loss_pcm: f64,
    pub core_inlet_f: f64,
    pub core_outlet_f: f64,
    pub fuel_f: f64,
    pub graphite_f: f64,
    pub hx_fuel_in_f: f64,
    pub hx_fuel_out_f: f64,
    pub hx_coolant_in_f: f64,
    pub hx_coolant_out_f: f64,
    pub radiator_coolant_out_f: f64,
    pub radiator_air_out_f: f64,
    pub fuel_flow: f64,
    pub coolant_flow: f64,
    pub air_flow: f64,
    /// 1 when the servo holds the rods, else 0.
    pub servo: f64,
}

impl Snapshot {
    /// Whether the coldest fuel-salt lump (core inlet or heat-exchanger
    /// fuel outlet) and the coldest coolant-salt lump (radiator or
    /// heat-exchanger coolant inlet) are below their liquidus. The model
    /// has no freezing (`MsreModel`'s docs): past this point its numbers
    /// describe a salt the real plant would have frozen, and ORNL's
    /// radiator doors existed to prevent it.
    pub fn below_liquidus(&self) -> (bool, bool) {
        (
            self.core_inlet_f.min(self.hx_fuel_out_f) < FUEL_LIQUIDUS_F,
            self.radiator_coolant_out_f.min(self.hx_coolant_in_f) < COOLANT_LIQUIDUS_F,
        )
    }

    /// Column names, in [`Snapshot::to_vec`]'s order (the CSV header).
    pub const FIELDS: [&'static str; 21] = [
        "time_s",
        "power_mw",
        "heat_mw",
        "reactivity_pcm",
        "rod_pcm",
        "feedback_pcm",
        "circulation_loss_pcm",
        "core_inlet_f",
        "core_outlet_f",
        "fuel_f",
        "graphite_f",
        "hx_fuel_in_f",
        "hx_fuel_out_f",
        "hx_coolant_in_f",
        "hx_coolant_out_f",
        "radiator_coolant_out_f",
        "radiator_air_out_f",
        "fuel_flow",
        "coolant_flow",
        "air_flow",
        "servo",
    ];

    /// The fields as numbers, in [`Snapshot::FIELDS`]' order: what crosses
    /// to the page.
    pub fn to_vec(&self) -> Vec<f64> {
        vec![
            self.time_s,
            self.power_mw,
            self.heat_mw,
            self.reactivity_pcm,
            self.rod_pcm,
            self.feedback_pcm,
            self.circulation_loss_pcm,
            self.core_inlet_f,
            self.core_outlet_f,
            self.fuel_f,
            self.graphite_f,
            self.hx_fuel_in_f,
            self.hx_fuel_out_f,
            self.hx_coolant_in_f,
            self.hx_coolant_out_f,
            self.radiator_coolant_out_f,
            self.radiator_air_out_f,
            self.fuel_flow,
            self.coolant_flow,
            self.air_flow,
            self.servo,
        ]
    }

    /// The inverse of [`Snapshot::to_vec`]; `None` if `v` is too short.
    pub fn from_slice(v: &[f64]) -> Option<Self> {
        if v.len() < Self::FIELDS.len() {
            return None;
        }
        Some(Self {
            time_s: v[0],
            power_mw: v[1],
            heat_mw: v[2],
            reactivity_pcm: v[3],
            rod_pcm: v[4],
            feedback_pcm: v[5],
            circulation_loss_pcm: v[6],
            core_inlet_f: v[7],
            core_outlet_f: v[8],
            fuel_f: v[9],
            graphite_f: v[10],
            hx_fuel_in_f: v[11],
            hx_fuel_out_f: v[12],
            hx_coolant_in_f: v[13],
            hx_coolant_out_f: v[14],
            radiator_coolant_out_f: v[15],
            radiator_air_out_f: v[16],
            fuel_flow: v[17],
            coolant_flow: v[18],
            air_flow: v[19],
            servo: v[20],
        })
    }
}

/// A running plant and the operator's settings.
pub struct Session {
    model: MsreModel,
    config: SessionConfig,
    servo: bool,
}

impl Session {
    /// A plant in steady state at `config.power_mw`, rods critical, servo off.
    pub fn new(config: SessionConfig) -> Result<Self, MsreModelError> {
        Ok(Self { model: MsreModel::new(config.model_config())?, config, servo: false })
    }

    /// The configuration this session started from.
    pub fn config(&self) -> SessionConfig {
        self.config
    }

    /// Apply one operator command. Errors for an out-of-range input (a
    /// non-finite rod reactivity, a non-positive air fraction).
    pub fn apply(&mut self, c: Command) -> Result<(), MsreModelError> {
        match c {
            Command::RodsPcm(pcm) => {
                if !pcm.is_finite() {
                    return Err(MsreModelError::InvalidInput(format!("rod reactivity must be finite, got {pcm} pcm")));
                }
                self.servo = false;
                self.model.set_rod_reactivity(Ratio::new::<ratio>(pcm * 1e-5));
            }
            Command::Servo(on) => self.servo = on,
            Command::AirFraction(f) => self.model.set_radiator_air_flow_fraction(Ratio::new::<ratio>(f))?,
            Command::TripFuelPump => self.model.trip_fuel_pump(FlowSchedule::ornl4233_fuel_pump_speed_coastdown()),
            Command::TripCoolantPump => {
                self.model.trip_coolant_pump(FlowSchedule::ornl4233_coolant_salt_flow_coastdown())
            }
            Command::StartFuelPump => self.model.start_fuel_pump(FlowSchedule::tm0728_fuel_flow_startup()),
            Command::StartCoolantPump => self.model.start_coolant_pump(FlowSchedule::tm0728_fuel_flow_startup()),
        }
        Ok(())
    }

    /// Run `seconds` of plant time (capped at [`MAX_ADVANCE_S`]; negative
    /// or non-finite runs nothing) and return the snapshot after it. With
    /// the servo on, the rods are reset to the holding value before every
    /// internal step.
    pub fn advance(&mut self, seconds: f64) -> Result<Snapshot, MsreModelError> {
        let s = if seconds.is_finite() { seconds.clamp(0.0, MAX_ADVANCE_S) } else { 0.0 };
        if self.servo {
            let n = (s / STEP_S + 1e-9).floor() as u64;
            for _ in 0..n {
                let hold = self.model.critical_rod_reactivity();
                self.model.set_rod_reactivity(hold);
                self.model.step(Time::new::<second>(STEP_S))?;
            }
        } else {
            self.model.step(Time::new::<second>(s))?;
        }
        Ok(self.snapshot())
    }

    /// The plant now.
    pub fn snapshot(&self) -> Snapshot {
        let m = &self.model;
        let f = |t: uom::si::f64::ThermodynamicTemperature| t.get::<degree_fahrenheit>();
        let pcm = |r: Ratio| r.get::<ratio>() * 1e5;
        Snapshot {
            time_s: m.time().get::<second>(),
            power_mw: m.power().get::<megawatt>(),
            heat_mw: m.heat_deposition().get::<megawatt>(),
            reactivity_pcm: pcm(m.reactivity()),
            rod_pcm: pcm(m.rod_reactivity()),
            feedback_pcm: pcm(m.feedback_reactivity()),
            circulation_loss_pcm: pcm(m.circulation_loss()),
            core_inlet_f: f(m.core_inlet_temperature()),
            core_outlet_f: f(m.core_outlet_temperature()),
            fuel_f: f(m.fuel_lump_temperature()),
            graphite_f: f(m.graphite_temperature()),
            hx_fuel_in_f: f(m.hx_fuel_inlet_temperature()),
            hx_fuel_out_f: f(m.hx_fuel_outlet_temperature()),
            hx_coolant_in_f: f(m.hx_coolant_inlet_temperature()),
            hx_coolant_out_f: f(m.hx_coolant_outlet_temperature()),
            radiator_coolant_out_f: f(m.radiator_coolant_outlet_temperature()),
            radiator_air_out_f: f(m.radiator_air_outlet_temperature()),
            fuel_flow: m.fuel_flow_fraction().get::<ratio>(),
            coolant_flow: m.coolant_flow_fraction().get::<ratio>(),
            air_flow: m.radiator_air_flow_fraction().get::<ratio>(),
            servo: if self.servo { 1.0 } else { 0.0 },
        }
    }
}

/// The demonstration transient the headless mode runs by default: 8 MW
/// steady, rods +10 pcm at 60 s, the fuel pump trips at 600 s, and 10 s
/// later the radiator doors close to 1 % of design air flow, as the MSRE's
/// operators closed them after a trip to keep the coolant from freezing
/// (the model has no freezing; see [`Snapshot::below_liquidus`]).
pub fn demo_script() -> Vec<(f64, Command)> {
    vec![(60.0, Command::RodsPcm(10.0)), (600.0, Command::TripFuelPump), (610.0, Command::AirFraction(0.01))]
}

/// The radiator air inlet, [°F] (TM-0728 Table 8.2, `MsreConfig::tm1070`):
/// the coldest heat sink, so no lump can fall below it (second law).
pub const AIR_INLET_F: f64 = 100.0;

/// Run `config` with `script`'s commands (each at its time, [s]) for
/// `end_s` seconds and return a CSV sampled every `every_s`: a header of
/// [`Snapshot::FIELDS`], then rows at fixed precision. Deterministic: no
/// clock, no randomness, so the same inputs give the same bytes (the
/// headless-mode rule in the crate's `AGENTS.md`).
pub fn headless_csv(
    config: SessionConfig,
    script: &[(f64, Command)],
    end_s: f64,
    every_s: f64,
) -> Result<String, MsreModelError> {
    if !(every_s >= STEP_S && end_s >= 0.0 && end_s.is_finite()) {
        return Err(MsreModelError::InvalidInput(format!("need every_s >= {STEP_S} and end_s >= 0, got {every_s}, {end_s}")));
    }
    let mut s = Session::new(config)?;
    let mut out = Snapshot::FIELDS.join(",");
    out.push('\n');
    let row = |snap: &Snapshot| snap.to_vec().iter().map(|x| format!("{x:.6}")).collect::<Vec<_>>().join(",") + "\n";
    out += &row(&s.snapshot());
    let mut events: Vec<(f64, Command)> = script.to_vec();
    events.sort_by(|a, b| a.0.total_cmp(&b.0));
    let samples = (end_s / every_s + 1e-9).floor() as u64;
    let mut next = 0;
    for k in 1..=samples {
        let target = k as f64 * every_s;
        // Run up to each event inside this sample interval, apply it, go on.
        while next < events.len() && events[next].0 <= target + 1e-9 {
            let gap = events[next].0 - s.snapshot().time_s;
            run(&mut s, gap)?;
            s.apply(events[next].1)?;
            next += 1;
        }
        let gap = target - s.snapshot().time_s;
        run(&mut s, gap)?;
        out += &row(&s.snapshot());
    }
    Ok(out)
}

/// Advance in [`MAX_ADVANCE_S`] chunks.
fn run(s: &mut Session, mut seconds: f64) -> Result<(), MsreModelError> {
    while seconds > 1e-9 {
        let chunk = seconds.min(MAX_ADVANCE_S);
        s.advance(chunk)?;
        seconds -= chunk;
    }
    Ok(())
}

/// One row of the T-1 table: power [MW], TM-1647's measured natural period
/// [min], and the linear model's [min] (`None` where it has no resonance).
pub type PeriodRow = (f64, f64, Option<f64>);

/// The MSRE T-1 natural periods at TM-1647's seven powers, from `nee_soon`'s
/// frequency-response model (TM-1070's well-stirred loop) with the given
/// coefficients, [1/°F], beside the measured periods. Not a fit: the model's
/// periods are computed, the measured ones read from TM-1647.
pub fn natural_periods(fuel_per_f: f64, graphite_per_f: f64) -> Vec<PeriodRow> {
    MEASURED_PERIODS_TM1647
        .iter()
        .map(|&(p, measured)| {
            let m = MsreLoopModel::tm1070(Power::new::<megawatt>(p), per_f(fuel_per_f), per_f(graphite_per_f));
            (p, measured, m.natural_period(600).map(|t| t.get::<second>() / 60.0))
        })
        .collect()
}

#[cfg(test)]
mod tests;
