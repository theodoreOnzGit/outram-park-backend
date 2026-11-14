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
//! formatting. Since gh:#1573 also the live plant's
//! bookkeeping: a salt-freeze guard ([`Session::advance_guarded`], which
//! holds the plant at its last state above the liquidus and offers a thaw
//! stated as a cheat) and a wall-clock [`Pacer`] for a self-paced worker.
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
#[derive(Debug, Clone, Copy, PartialEq)]
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
    /// Measured in this browser session on the simulator's k_eff tab
    /// (gh:#1574): the same split as [`Coefficients::MonteCarloN3`] (salt
    /// and graphite each 100 °F up from 1131 °F), run by the reader, with
    /// each coefficient's σ, [1/°F]. Sent to the plant only by the reader's
    /// own button, never automatically.
    Measured { fuel_per_f: f64, fuel_sigma: f64, graphite_per_f: f64, graphite_sigma: f64 },
}

impl Coefficients {
    /// Fuel and graphite coefficients, [1/°F].
    pub fn per_f(self) -> (f64, f64) {
        match self {
            Coefficients::Ornl1965 => (ORNL_1965_FUEL_PER_F, ORNL_1965_GRAPHITE_PER_F),
            Coefficients::MonteCarloN3 => (MC_N3_FUEL_PER_F, MC_N3_GRAPHITE_PER_F),
            Coefficients::Measured { fuel_per_f, graphite_per_f, .. } => (fuel_per_f, graphite_per_f),
        }
    }

    /// Where they come from, for the plant's status line.
    pub fn describe(self) -> String {
        match self {
            Coefficients::Ornl1965 => "ORNL 1965 predictions (TM-1070)".into(),
            Coefficients::MonteCarloN3 => "precomputed Monte Carlo N-3 (this workspace)".into(),
            Coefficients::Measured { fuel_per_f, fuel_sigma, graphite_per_f, graphite_sigma } => format!(
                "measured in this browser: fuel {:.2} ± {:.2}, graphite {:.2} ± {:.2} (×1e-5 /°F)",
                fuel_per_f * 1e5,
                fuel_sigma * 1e5,
                graphite_per_f * 1e5,
                graphite_sigma * 1e5
            ),
        }
    }
}

/// The N-3 split's temperatures, °F: the base (salt and graphite), and the
/// step each component is raised by (ORNL-4233's 1131 / 1231 °F as the
/// N-3 record runs them).
pub const SPLIT_BASE_F: f64 = 1131.0;
pub const SPLIT_STEP_F: f64 = 100.0;

/// The fuel or graphite coefficient from two eigenvalues and their σ: the
/// reactivity ρ = (k − 1)/k at the base and with one component raised by
/// `dt_f`, α = (ρ_raised − ρ_base)/dt_f [1/°F], with σ by first-order
/// propagation of the two independent runs' σ (σ_ρ = σ_k/k²). The N-3
/// record's arithmetic (its fuel −3.79 is 1.02232 → 1.01837 over 100 °F).
pub fn coefficient_from_pair(k_base: f64, s_base: f64, k_raised: f64, s_raised: f64, dt_f: f64) -> (f64, f64) {
    let a = (1.0 / k_base - 1.0 / k_raised) / dt_f;
    let (b, r) = (s_base / (k_base * k_base), s_raised / (k_raised * k_raised));
    let s = (b * b + r * r).sqrt() / dt_f;
    (a, s)
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
    /// The salt freeze the guarded advance stopped at, until a thaw
    /// ([`Session::advance_guarded`], [`Session::thaw`]).
    frozen: Option<Freeze>,
    /// Plant time before the last thaw, [s]: a thaw builds a new model
    /// (its clock starts at zero) and the session's clock carries on.
    time_offset_s: f64,
}

impl Session {
    /// A plant in steady state at `config.power_mw`, rods critical, servo off.
    pub fn new(config: SessionConfig) -> Result<Self, MsreModelError> {
        Ok(Self { model: MsreModel::new(config.model_config())?, config, servo: false, frozen: None, time_offset_s: 0.0 })
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
            time_s: self.time_offset_s + m.time().get::<second>(),
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

// ─── The live plant: salt-freeze guard and wall-clock pacing (gh:#1573) ─────

/// The coldest lump of a salt, where a freeze is reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreezeLocation {
    /// Fuel salt entering the core.
    CoreInlet,
    /// Fuel salt leaving the primary heat exchanger.
    HxFuelOutlet,
    /// Coolant salt leaving the radiator.
    RadiatorCoolantOutlet,
    /// Coolant salt entering the primary heat exchanger.
    HxCoolantInlet,
}

impl FreezeLocation {
    /// Every location, in [`FreezeLocation::code`]'s order.
    pub const ALL: [FreezeLocation; 4] = [
        FreezeLocation::CoreInlet,
        FreezeLocation::HxFuelOutlet,
        FreezeLocation::RadiatorCoolantOutlet,
        FreezeLocation::HxCoolantInlet,
    ];

    /// Where, in words.
    pub fn name(self) -> &'static str {
        match self {
            FreezeLocation::CoreInlet => "the core inlet",
            FreezeLocation::HxFuelOutlet => "the heat exchanger's fuel outlet",
            FreezeLocation::RadiatorCoolantOutlet => "the radiator's coolant outlet",
            FreezeLocation::HxCoolantInlet => "the heat exchanger's coolant inlet",
        }
    }

    /// `true` for the fuel salt, `false` for the coolant salt.
    pub fn is_fuel(self) -> bool {
        matches!(self, FreezeLocation::CoreInlet | FreezeLocation::HxFuelOutlet)
    }

    /// Which salt, in words.
    pub fn salt_name(self) -> &'static str {
        if self.is_fuel() {
            "fuel salt (LiF-BeF2-ZrF4-UF4)"
        } else {
            "coolant salt (LiF-BeF2)"
        }
    }

    /// That salt's liquidus, [°F] ([`FUEL_LIQUIDUS_F`], [`COOLANT_LIQUIDUS_F`]).
    pub fn liquidus_f(self) -> f64 {
        if self.is_fuel() {
            FUEL_LIQUIDUS_F
        } else {
            COOLANT_LIQUIDUS_F
        }
    }

    /// A wire code (the index in [`FreezeLocation::ALL`]).
    pub fn code(self) -> f64 {
        Self::ALL.iter().position(|&l| l == self).unwrap_or(0) as f64
    }

    /// The inverse of [`FreezeLocation::code`].
    pub fn from_code(x: f64) -> Option<Self> {
        Self::ALL.get(x as usize).copied().filter(|_| x >= 0.0 && x.fract() == 0.0)
    }
}

/// A salt that went below its liquidus: what the guard stopped at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Freeze {
    pub location: FreezeLocation,
    /// The temperature the step that was refused would have reached there, [°F].
    pub temperature_f: f64,
    /// The plant time the session is held at: the last state with every salt
    /// above its liquidus, [s].
    pub held_at_s: f64,
}

impl Freeze {
    /// One line for the operator: which salt, where, how cold.
    pub fn headline(&self) -> String {
        format!(
            "The {} would freeze at {}: {:.0} °F, below its {:.0} °F liquidus.",
            self.location.salt_name(),
            self.location.name(),
            self.temperature_f,
            self.location.liquidus_f()
        )
    }
}

impl Snapshot {
    /// The salt below its liquidus, if one is, at its coldest lump; the fuel
    /// salt is reported first. The same test as [`Snapshot::below_liquidus`],
    /// with the place named.
    pub fn freeze(&self) -> Option<Freeze> {
        let fuel = if self.core_inlet_f <= self.hx_fuel_out_f {
            (FreezeLocation::CoreInlet, self.core_inlet_f)
        } else {
            (FreezeLocation::HxFuelOutlet, self.hx_fuel_out_f)
        };
        let coolant = if self.radiator_coolant_out_f <= self.hx_coolant_in_f {
            (FreezeLocation::RadiatorCoolantOutlet, self.radiator_coolant_out_f)
        } else {
            (FreezeLocation::HxCoolantInlet, self.hx_coolant_in_f)
        };
        [fuel, coolant]
            .into_iter()
            .find(|(l, t)| *t < l.liquidus_f())
            .map(|(location, temperature_f)| Freeze { location, temperature_f, held_at_s: self.time_s })
    }
}

/// What a guarded advance did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Advance {
    /// The plant ran; the snapshot after it.
    Ran(Snapshot),
    /// A salt would have gone below its liquidus: the plant is held at the
    /// last state above it, and stays held until [`Session::thaw`] (or a new
    /// session). Reported again on every advance while held.
    Frozen(Freeze),
}

impl Session {
    /// [`Session::advance`] with the salt-freeze guard, after `fhr_sim_v2`'s
    /// `salt_freeze_guard` (detect from state, stop at a known-good state,
    /// say which salt and where, offer a thaw).
    ///
    /// **Why a guard.** [`MsreModel`] has no freezing: below a liquidus its
    /// numbers describe a salt the real plant would have frozen. So the
    /// stretch is run on a copy of the plant's state kept beside it, and if
    /// any salt ends below its liquidus the copy is put back: the plant is
    /// held at the start of the refused stretch, every salt still liquid.
    /// The freeze is therefore found to within one stretch (a page's tick,
    /// at most [`MAX_ADVANCE_S`] of plant time), and nothing past it is ever
    /// shown. A plant that starts below a liquidus is held at once.
    pub fn advance_guarded(&mut self, seconds: f64) -> Result<Advance, MsreModelError> {
        if let Some(f) = self.frozen {
            return Ok(Advance::Frozen(f));
        }
        let now = self.snapshot();
        if let Some(f) = now.freeze() {
            self.frozen = Some(f);
            return Ok(Advance::Frozen(f));
        }
        let saved = self.model.clone();
        let snap = self.advance(seconds)?;
        match snap.freeze() {
            Some(f) => {
                self.model = saved;
                let f = Freeze { held_at_s: now.time_s, ..f };
                self.frozen = Some(f);
                Ok(Advance::Frozen(f))
            }
            None => Ok(Advance::Ran(snap)),
        }
    }

    /// The freeze the plant is held at, if any.
    pub fn frozen(&self) -> Option<Freeze> {
        self.frozen
    }

    /// **The thaw: a cheat, and it says so.** A real frozen salt loop is
    /// thawed by trace heating over hours to days, with a real risk of
    /// plugs and of never recovering the loop; none of that is modelled.
    /// This puts the plant back in the hot steady state it started in (the
    /// session's [`SessionConfig`]: its power, both pumps running, rods
    /// critical, servo off), keeping the plant clock running on. Returns
    /// `true` if a freeze was thawed; `false`, changing nothing, if none
    /// was held: one freeze, one thaw.
    pub fn thaw(&mut self) -> Result<bool, MsreModelError> {
        if self.frozen.is_none() {
            return Ok(false);
        }
        let elapsed = self.snapshot().time_s;
        self.model = MsreModel::new(self.config.model_config())?;
        self.time_offset_s = elapsed;
        self.servo = false;
        self.frozen = None;
        Ok(true)
    }
}

/// The longest wall-clock gap one tick of [`Pacer`] turns into plant time,
/// [s]. A page in a background tab, or a tick that came late because the
/// last one computed slowly, runs on at this rate rather than leaping: the
/// plant slows down instead of jumping a stretch nobody saw.
pub const MAX_WALL_GAP_S: f64 = 1.0;

/// The live plant's clock: turns wall-clock readings into plant seconds at
/// the chosen speed (plant seconds per wall second), with pause. No physics
/// and no clock of its own: the caller passes the wall time it read, so a
/// test drives it with any numbers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pacer {
    pub speed: f64,
    pub paused: bool,
    last_wall_s: Option<f64>,
}

impl Pacer {
    pub fn new(speed: f64, paused: bool) -> Self {
        Self { speed, paused, last_wall_s: None }
    }

    /// The plant seconds to run for a tick at wall time `wall_s` [s]: the
    /// wall time since the last tick (at most [`MAX_WALL_GAP_S`]) times the
    /// speed; zero on the first tick, while paused, for a clock that went
    /// backwards, or for a non-finite or negative speed.
    pub fn plant_seconds(&mut self, wall_s: f64) -> f64 {
        let gap = match self.last_wall_s {
            Some(last) if wall_s.is_finite() => (wall_s - last).clamp(0.0, MAX_WALL_GAP_S),
            _ => 0.0,
        };
        if wall_s.is_finite() {
            self.last_wall_s = Some(wall_s);
        }
        if self.paused || !(self.speed.is_finite() && self.speed > 0.0) {
            0.0
        } else {
            gap * self.speed
        }
    }
}

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
