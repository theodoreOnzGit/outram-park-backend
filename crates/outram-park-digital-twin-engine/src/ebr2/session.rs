//! # An operator session on [`Ebr2Model`], in plain numbers
//!
//! What a GUI or a web worker needs to drive the EBR-II SHRT model without
//! touching `uom` types (gh:#1741, the museum EBR-II simulator
//! `examples/ebr2_web`, built like the MSRE one, `msre::session`):
//!
//! - operator [`Command`]s: a pump-speed override, an intermediate-loop flow
//!   override, external reactivity;
//! - a flat [`Snapshot`] of the model's getters in °C, MW, kg/s, rpm and pcm,
//!   with the feedback broken into its terms ([`Ebr2Model::reactivity`]);
//! - the model's own steady state against the cited initial conditions
//!   ([`Initial`], from [`Ebr2Model::initial_comparison`]);
//! - a deterministic headless CSV ([`headless_csv`]);
//! - a guard that holds the plant at its last good state when a step fails
//!   ([`Session::advance_guarded`]).
//!
//! It adds **no physics**: every number comes from [`Ebr2Model`]. The only
//! logic here is bookkeeping: applying commands, the guard, and formatting.
//! The wall-clock pacer is MSRE's ([`Pacer`], re-exported, not rewritten).
//!
//! ## What the operator can and cannot do (gaps recorded on gh:#1741)
//!
//! - **No pump-trip command.** The SHRT tables trip the pumps at t = 0; the
//!   only way to "not trip" is the speed override, which holds both pumps
//!   at a chosen speed. Returning to the table resumes the test's coast-down
//!   at the current time.
//! - **No coefficient setter.** The feedback coefficients are fixed when the
//!   model is built (`KineticsConfig`), so there is no "send to plant" from
//!   a Monte Carlo calculation as the MSRE demo has.
//! - **External reactivity is SHRT-45R only.** SHRT-17's power is its
//!   tabulated boundary condition, so reactivity has nothing to act on and
//!   the model refuses it ([`Ebr2Model::set_external_reactivity`]).
//!
//! Research, education and V&V only. NOT VALIDATED. AI draft (gh:#1741).

use uom::si::angular_velocity::revolution_per_minute;
use uom::si::electric_current::ampere;
use uom::si::f64::{AngularVelocity, Length, MassRate, Ratio, ThermodynamicTemperature, Time};
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::megawatt;
use uom::si::pressure::kilopascal;
use uom::si::ratio::ratio;
use uom::si::temperature_interval::kelvin as kelvin_interval;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::second;

use super::model::{Ebr2Config, Ebr2Error, Ebr2Model};
use super::shrt::ShrtTest;

/// The wall-clock pacer of a self-paced plant worker: MSRE's, reused as is
/// (it holds no MSRE physics, only speed, pause and the wall gap).
pub use crate::msre::session::{Pacer, MAX_WALL_GAP_S};

/// The longest stretch of plant time one [`Session::advance`] call runs, [s].
/// A worker answers one request at a time, so a request must stay short; at
/// about 10 µs per 0.05 s sub-step natively (`docs/ebr2_library.md`) this
/// is a couple of milliseconds of computing.
pub const MAX_ADVANCE_S: f64 = 10.0;

/// The stretch the guard runs on a copy of the plant, [s]: a failed step is
/// found to within this much plant time ([`Session::advance_guarded`]).
pub const GUARD_STRETCH_S: f64 = 0.5;

/// The IHX-above-core thermal-centre height the library's V&V record ran
/// at, [m] (`docs/ebr2_library.md`, sections 2-4). **Not cited:** no
/// transcribed source gives it, and the model requires it with no default.
/// The demo starts here and lets the reader change it; the record's sweep
/// over 1, 2 and 4 m shows SHRT-17's peak power-to-flow ratio moves from
/// 2.47 to 1.16 across that range.
pub const VV_THERMAL_CENTRE_HEIGHT_M: f64 = 2.0;

/// How a session starts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SessionConfig {
    /// Which test's boundary conditions drive the plant.
    pub test: ShrtTest,
    /// IHX thermal-centre height above the core's, [m]. Required by the
    /// model, not in the sources (see [`VV_THERMAL_CENTRE_HEIGHT_M`]).
    pub thermal_centre_height_m: f64,
}

impl Default for SessionConfig {
    /// SHRT-45R (the unprotected test, where the feedback acts) at the V&V
    /// record's 2 m.
    fn default() -> Self {
        Self { test: ShrtTest::Shrt45r, thermal_centre_height_m: VV_THERMAL_CENTRE_HEIGHT_M }
    }
}

impl SessionConfig {
    /// The [`Ebr2Config`] this session builds its model from: the library's
    /// own `shrt17` / `shrt45r` presets at this height.
    pub fn model_config(&self) -> Ebr2Config {
        let h = Length::new::<meter>(self.thermal_centre_height_m);
        match self.test {
            ShrtTest::Shrt17 => Ebr2Config::shrt17(h),
            ShrtTest::Shrt45r => Ebr2Config::shrt45r(h),
        }
    }
}

/// A test as a wire code: 0 SHRT-17, 1 SHRT-45R.
pub fn test_code(t: ShrtTest) -> f64 {
    match t {
        ShrtTest::Shrt17 => 0.0,
        ShrtTest::Shrt45r => 1.0,
    }
}

/// The inverse of [`test_code`]; anything but 0 is SHRT-45R.
pub fn test_from_code(x: f64) -> ShrtTest {
    if x == 0.0 {
        ShrtTest::Shrt17
    } else {
        ShrtTest::Shrt45r
    }
}

/// What an operator can do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Command {
    /// Hold both primary pumps at this speed, [rpm]; `None` returns them to
    /// the test's tabulated coast-down at the current time
    /// ([`Ebr2Model::set_pump_speeds`]).
    PumpSpeedRpm(Option<f64>),
    /// Hold the intermediate IHX inlet flow at this value, [kg/s]; `None`
    /// returns it to the test's table ([`Ebr2Model::set_intermediate_flow`]).
    IntermediateFlowKgS(Option<f64>),
    /// External reactivity, [pcm] (a rod, a scram); SHRT-45R only
    /// ([`Ebr2Model::set_external_reactivity`]).
    ExternalPcm(f64),
}

/// The plant now, in plain numbers.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Snapshot {
    pub time_s: f64,
    pub fission_mw: f64,
    pub decay_mw: f64,
    pub total_mw: f64,
    /// Fuel volume-mean temperature, [°C].
    pub fuel_c: f64,
    /// Core inlet = pool temperature, [°C].
    pub core_inlet_c: f64,
    pub core_mean_c: f64,
    /// Mixed-mean core outlet, [°C].
    pub core_outlet_c: f64,
    /// IHX primary outlet, [°C].
    pub ihx_outlet_c: f64,
    pub primary_flow_kg_s: f64,
    /// Primary flow over the model's own initial flow.
    pub flow_fraction: f64,
    /// `(P/P0)/(W/W0)`, the ratio the benchmark reports.
    pub power_to_flow: f64,
    pub rho_total_pcm: f64,
    pub rho_external_pcm: f64,
    pub rho_doppler_pcm: f64,
    pub rho_axial_pcm: f64,
    pub rho_cladding_pcm: f64,
    pub rho_coolant_temperature_pcm: f64,
    pub rho_coolant_density_pcm: f64,
    pub rho_radial_pcm: f64,
    pub rho_driveline_pcm: f64,
    /// beta-eff, [pcm]; NaN in tabulated mode (SHRT-17).
    pub beta_eff_pcm: f64,
    pub pump1_rpm: f64,
    pub pump2_rpm: f64,
    pub intermediate_flow_kg_s: f64,
    pub intermediate_inlet_c: f64,
    pub aux_em_pump_a: f64,
    /// IHX effectiveness; NaN if it could not be evaluated.
    pub ihx_effectiveness: f64,
    /// Sodium in the core node, the IHX node and the pool, [kg]: with the
    /// flow, what a drawing's tracer residence times come from.
    pub core_na_kg: f64,
    pub ihx_na_kg: f64,
    pub pool_na_kg: f64,
    /// 1 when the pump speeds are overridden, else 0.
    pub pump_override: f64,
    /// 1 when the intermediate flow is overridden, else 0.
    pub intermediate_override: f64,
    /// 1 in point-kinetics mode (SHRT-45R), 0 with tabulated power.
    pub kinetics: f64,
}

impl Snapshot {
    /// Column names, in [`Snapshot::to_vec`]'s order (the CSV header).
    pub const FIELDS: [&'static str; 34] = [
        "time_s",
        "fission_mw",
        "decay_mw",
        "total_mw",
        "fuel_c",
        "core_inlet_c",
        "core_mean_c",
        "core_outlet_c",
        "ihx_outlet_c",
        "primary_flow_kg_s",
        "flow_fraction",
        "power_to_flow",
        "rho_total_pcm",
        "rho_external_pcm",
        "rho_doppler_pcm",
        "rho_axial_pcm",
        "rho_cladding_pcm",
        "rho_coolant_temperature_pcm",
        "rho_coolant_density_pcm",
        "rho_radial_pcm",
        "rho_driveline_pcm",
        "beta_eff_pcm",
        "pump1_rpm",
        "pump2_rpm",
        "intermediate_flow_kg_s",
        "intermediate_inlet_c",
        "aux_em_pump_a",
        "ihx_effectiveness",
        "core_na_kg",
        "ihx_na_kg",
        "pool_na_kg",
        "pump_override",
        "intermediate_override",
        "kinetics",
    ];

    /// The fields as numbers, in [`Snapshot::FIELDS`]' order: what crosses
    /// to the page. A held plant is reported by [`Advance::Held`], not by
    /// a snapshot.
    pub fn to_vec(&self) -> Vec<f64> {
        vec![
            self.time_s,
            self.fission_mw,
            self.decay_mw,
            self.total_mw,
            self.fuel_c,
            self.core_inlet_c,
            self.core_mean_c,
            self.core_outlet_c,
            self.ihx_outlet_c,
            self.primary_flow_kg_s,
            self.flow_fraction,
            self.power_to_flow,
            self.rho_total_pcm,
            self.rho_external_pcm,
            self.rho_doppler_pcm,
            self.rho_axial_pcm,
            self.rho_cladding_pcm,
            self.rho_coolant_temperature_pcm,
            self.rho_coolant_density_pcm,
            self.rho_radial_pcm,
            self.rho_driveline_pcm,
            self.beta_eff_pcm,
            self.pump1_rpm,
            self.pump2_rpm,
            self.intermediate_flow_kg_s,
            self.intermediate_inlet_c,
            self.aux_em_pump_a,
            self.ihx_effectiveness,
            self.core_na_kg,
            self.ihx_na_kg,
            self.pool_na_kg,
            self.pump_override,
            self.intermediate_override,
            self.kinetics,
        ]
    }

    /// The inverse of [`Snapshot::to_vec`]; `None` if `v` is too short.
    pub fn from_slice(v: &[f64]) -> Option<Self> {
        if v.len() < Self::FIELDS.len() {
            return None;
        }
        Some(Self {
            time_s: v[0],
            fission_mw: v[1],
            decay_mw: v[2],
            total_mw: v[3],
            fuel_c: v[4],
            core_inlet_c: v[5],
            core_mean_c: v[6],
            core_outlet_c: v[7],
            ihx_outlet_c: v[8],
            primary_flow_kg_s: v[9],
            flow_fraction: v[10],
            power_to_flow: v[11],
            rho_total_pcm: v[12],
            rho_external_pcm: v[13],
            rho_doppler_pcm: v[14],
            rho_axial_pcm: v[15],
            rho_cladding_pcm: v[16],
            rho_coolant_temperature_pcm: v[17],
            rho_coolant_density_pcm: v[18],
            rho_radial_pcm: v[19],
            rho_driveline_pcm: v[20],
            beta_eff_pcm: v[21],
            pump1_rpm: v[22],
            pump2_rpm: v[23],
            intermediate_flow_kg_s: v[24],
            intermediate_inlet_c: v[25],
            aux_em_pump_a: v[26],
            ihx_effectiveness: v[27],
            core_na_kg: v[28],
            ihx_na_kg: v[29],
            pool_na_kg: v[30],
            pump_override: v[31],
            intermediate_override: v[32],
            kinetics: v[33],
        })
    }

    /// The feedback terms with their names, in `teh-o-prke`'s order, then
    /// the external reactivity, [pcm]: what the Reactivity tab draws.
    pub fn reactivity_terms(&self) -> [(&'static str, f64); 8] {
        [
            ("Doppler", self.rho_doppler_pcm),
            ("axial fuel expansion", self.rho_axial_pcm),
            ("cladding", self.rho_cladding_pcm),
            ("coolant temperature", self.rho_coolant_temperature_pcm),
            ("coolant density", self.rho_coolant_density_pcm),
            ("radial expansion", self.rho_radial_pcm),
            ("driveline", self.rho_driveline_pcm),
            ("external", self.rho_external_pcm),
        ]
    }
}

/// The model's own steady state at t = 0 against the cited initial
/// conditions ([`Ebr2Model::initial_comparison`]), in plain numbers. A
/// check, never an input: the model is not forced to match.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Initial {
    pub model_flow_kg_s: f64,
    pub cited_flow_kg_s: f64,
    pub model_core_inlet_c: f64,
    pub cited_core_inlet_c: f64,
    pub model_core_outlet_c: f64,
    /// Fuel minus core mean coolant, [K].
    pub model_fuel_minus_coolant_k: f64,
    /// Mean pump discharge head, [kPa].
    pub model_pump_head_kpa: f64,
}

impl Initial {
    /// As wire numbers.
    pub fn to_vec(&self) -> Vec<f64> {
        vec![
            self.model_flow_kg_s,
            self.cited_flow_kg_s,
            self.model_core_inlet_c,
            self.cited_core_inlet_c,
            self.model_core_outlet_c,
            self.model_fuel_minus_coolant_k,
            self.model_pump_head_kpa,
        ]
    }

    /// The inverse of [`Initial::to_vec`]; `None` if `v` is too short.
    pub fn from_slice(v: &[f64]) -> Option<Self> {
        if v.len() < 7 {
            return None;
        }
        Some(Self {
            model_flow_kg_s: v[0],
            cited_flow_kg_s: v[1],
            model_core_inlet_c: v[2],
            cited_core_inlet_c: v[3],
            model_core_outlet_c: v[4],
            model_fuel_minus_coolant_k: v[5],
            model_pump_head_kpa: v[6],
        })
    }
}

/// A step that failed: the plant is held at the last state that stepped.
#[derive(Debug, Clone, PartialEq)]
pub struct Hold {
    /// The plant time held at, [s].
    pub held_at_s: f64,
    /// The model's error for the step that was refused.
    pub message: String,
}

impl Hold {
    /// One line for the operator.
    pub fn headline(&self) -> String {
        format!(
            "Held at t = {:.1} s: the next step failed ({}). The usual cause is sodium leaving the model's 400-1000 K property range; sodium boils near 1156 K, so past this point the model has nothing honest to say.",
            self.held_at_s, self.message
        )
    }
}

/// What a guarded advance did.
#[derive(Debug, Clone, PartialEq)]
pub enum Advance {
    /// The plant ran; the snapshot after it.
    Ran(Snapshot),
    /// A step failed: the plant is held at the last state that stepped.
    Held(Hold),
}

/// A running plant and the operator's settings.
pub struct Session {
    model: Ebr2Model,
    config: SessionConfig,
    held: Option<Hold>,
}

/// Degrees Celsius of a temperature.
fn celsius(t: ThermodynamicTemperature) -> f64 {
    t.get::<degree_celsius>()
}

/// pcm of a reactivity.
fn pcm(r: Ratio) -> f64 {
    r.get::<ratio>() * 1e5
}

impl Session {
    /// A plant in its own steady state at the test's initial conditions.
    pub fn new(config: SessionConfig) -> Result<Self, Ebr2Error> {
        if !(config.thermal_centre_height_m.is_finite() && config.thermal_centre_height_m > 0.0) {
            return Err(Ebr2Error::Invalid(format!(
                "thermal-centre height must be finite and > 0, got {} m",
                config.thermal_centre_height_m
            )));
        }
        Ok(Self { model: Ebr2Model::new(config.model_config())?, config, held: None })
    }

    /// The configuration this session started from.
    pub fn config(&self) -> SessionConfig {
        self.config
    }

    /// Apply one operator command. Errors, changing nothing, for a
    /// non-finite or negative speed or flow, a non-finite reactivity, or
    /// external reactivity on SHRT-17. A command that applies releases a
    /// hold ([`Session::advance_guarded`]): the plant goes on from the last
    /// good state under the new setting.
    pub fn apply(&mut self, c: Command) -> Result<(), Ebr2Error> {
        let non_negative = |x: f64, what: &str| {
            if x.is_finite() && x >= 0.0 {
                Ok(x)
            } else {
                Err(Ebr2Error::Invalid(format!("{what} must be finite and >= 0, got {x}")))
            }
        };
        match c {
            Command::PumpSpeedRpm(s) => {
                let speeds = match s {
                    Some(rpm) => {
                        let w = AngularVelocity::new::<revolution_per_minute>(non_negative(rpm, "pump speed")?);
                        Some((w, w))
                    }
                    None => None,
                };
                self.model.set_pump_speeds(speeds);
            }
            Command::IntermediateFlowKgS(f) => {
                let flow = match f {
                    Some(x) => Some(MassRate::new::<kilogram_per_second>(non_negative(x, "intermediate flow")?)),
                    None => None,
                };
                self.model.set_intermediate_flow(flow);
            }
            Command::ExternalPcm(x) => {
                if !x.is_finite() {
                    return Err(Ebr2Error::Invalid(format!("external reactivity must be finite, got {x} pcm")));
                }
                self.model.set_external_reactivity(Ratio::new::<ratio>(x * 1e-5))?;
            }
        }
        self.held = None;
        Ok(())
    }

    /// Run `seconds` of plant time (capped at [`MAX_ADVANCE_S`]; negative or
    /// non-finite runs nothing) and return the snapshot after it.
    /// Unguarded: a failed step returns its error with the model part-way
    /// through the stretch.
    pub fn advance(&mut self, seconds: f64) -> Result<Snapshot, Ebr2Error> {
        let s = if seconds.is_finite() { seconds.clamp(0.0, MAX_ADVANCE_S) } else { 0.0 };
        self.model.step(Time::new::<second>(s))?;
        Ok(self.snapshot())
    }

    /// [`Session::advance`] with a guard, after MSRE's salt-freeze guard
    /// (`msre::session::Session::advance_guarded`).
    ///
    /// **Why a guard.** A failed [`Ebr2Model::step`] leaves the model
    /// part-way through its sub-steps, and the usual failure is a sodium
    /// node leaving TUAS's 400-1000 K range, past which the model has no
    /// properties. So the stretch is run in [`GUARD_STRETCH_S`] pieces, each
    /// on a copy kept beside the plant; a piece that fails is undone, and the
    /// plant is held at the start of it until a command or a reset. The
    /// failure is found to within one piece, and nothing past it is shown.
    /// Reported again on every advance while held.
    pub fn advance_guarded(&mut self, seconds: f64) -> Advance {
        if let Some(h) = &self.held {
            return Advance::Held(h.clone());
        }
        let mut left = if seconds.is_finite() { seconds.clamp(0.0, MAX_ADVANCE_S) } else { 0.0 };
        while left > 1e-12 {
            let piece = left.min(GUARD_STRETCH_S);
            left -= piece;
            let saved = self.model.clone();
            if let Err(e) = self.model.step(Time::new::<second>(piece)) {
                self.model = saved;
                let h = Hold { held_at_s: self.model.time().get::<second>(), message: e.to_string() };
                self.held = Some(h.clone());
                return Advance::Held(h);
            }
        }
        Advance::Ran(self.snapshot())
    }

    /// The hold the plant is at, if any.
    pub fn held(&self) -> Option<&Hold> {
        self.held.as_ref()
    }

    /// The plant now.
    pub fn snapshot(&self) -> Snapshot {
        let m = &self.model;
        let rho = m.reactivity();
        let fb = rho.feedback;
        let inputs = m.inputs();
        let overrides = m.overrides();
        let initial_flow = m.initial_comparison().primary_flow.get::<kilogram_per_second>();
        let flow = m.primary_flow().get::<kilogram_per_second>();
        let (core, ihx, pool) = m.sodium_masses();
        Snapshot {
            time_s: m.time().get::<second>(),
            fission_mw: m.fission_power().get::<megawatt>(),
            decay_mw: m.decay_power().get::<megawatt>(),
            total_mw: m.total_power().get::<megawatt>(),
            fuel_c: celsius(m.fuel_temperature()),
            core_inlet_c: celsius(m.core_inlet_temperature()),
            core_mean_c: celsius(m.core_mean_coolant_temperature()),
            core_outlet_c: celsius(m.core_outlet_temperature()),
            ihx_outlet_c: celsius(m.ihx_primary_outlet_temperature()),
            primary_flow_kg_s: flow,
            flow_fraction: flow / initial_flow,
            power_to_flow: m.power_to_flow_ratio().get::<ratio>(),
            rho_total_pcm: pcm(rho.total()),
            rho_external_pcm: pcm(rho.external),
            rho_doppler_pcm: pcm(fb.doppler),
            rho_axial_pcm: pcm(fb.axial_fuel_expansion),
            rho_cladding_pcm: pcm(fb.cladding),
            rho_coolant_temperature_pcm: pcm(fb.coolant_temperature),
            rho_coolant_density_pcm: pcm(fb.coolant_density),
            rho_radial_pcm: pcm(fb.radial_expansion),
            rho_driveline_pcm: pcm(fb.driveline),
            beta_eff_pcm: m.beta_eff().map_or(f64::NAN, pcm),
            pump1_rpm: inputs.pump_speeds.0.get::<revolution_per_minute>(),
            pump2_rpm: inputs.pump_speeds.1.get::<revolution_per_minute>(),
            intermediate_flow_kg_s: inputs.intermediate_flow.get::<kilogram_per_second>(),
            intermediate_inlet_c: celsius(inputs.intermediate_inlet_temperature),
            aux_em_pump_a: inputs.aux_em_pump_current.get::<ampere>(),
            ihx_effectiveness: m.ihx_state().map_or(f64::NAN, |(_, eps)| eps),
            core_na_kg: core.get::<kilogram>(),
            ihx_na_kg: ihx.get::<kilogram>(),
            pool_na_kg: pool.get::<kilogram>(),
            pump_override: if overrides.pump_speeds.is_some() { 1.0 } else { 0.0 },
            intermediate_override: if overrides.intermediate_flow.is_some() { 1.0 } else { 0.0 },
            kinetics: if m.beta_eff().is_some() { 1.0 } else { 0.0 },
        }
    }

    /// The model's t = 0 steady state against the cited initial conditions.
    pub fn initial(&self) -> Initial {
        let c = self.model.initial_comparison();
        Initial {
            model_flow_kg_s: c.primary_flow.get::<kilogram_per_second>(),
            cited_flow_kg_s: c.cited_primary_flow.get::<kilogram_per_second>(),
            model_core_inlet_c: celsius(c.core_inlet),
            cited_core_inlet_c: celsius(c.cited_core_inlet),
            model_core_outlet_c: celsius(c.core_outlet),
            model_fuel_minus_coolant_k: c.fuel_minus_coolant.get::<kelvin_interval>(),
            model_pump_head_kpa: c.pump_head.get::<kilopascal>(),
        }
    }
}

/// Run `config` with `script`'s commands (each at its time, [s]) for
/// `end_s` seconds and return a CSV sampled every `every_s`: a header of
/// [`Snapshot::FIELDS`], then rows at fixed precision. Deterministic: no
/// clock, no randomness, so the same inputs give the same bytes (the
/// headless-mode rule in the crate's `AGENTS.md`). Unguarded: a failed step
/// is an error, never a silent hold.
pub fn headless_csv(
    config: SessionConfig,
    script: &[(f64, Command)],
    end_s: f64,
    every_s: f64,
) -> Result<String, Ebr2Error> {
    if !(every_s > 0.0 && every_s.is_finite() && end_s >= 0.0 && end_s.is_finite()) {
        return Err(Ebr2Error::Invalid(format!("need every_s > 0 and end_s >= 0, got {every_s}, {end_s}")));
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
fn run(s: &mut Session, mut seconds: f64) -> Result<(), Ebr2Error> {
    while seconds > 1e-9 {
        let chunk = seconds.min(MAX_ADVANCE_S);
        s.advance(chunk)?;
        seconds -= chunk;
    }
    Ok(())
}

/// The script the headless mode runs by default: **none**. Each SHRT test
/// is its own demonstration transient, driven by its tabulated boundary
/// conditions (pump coast-down at t = 0, the intermediate side, SHRT-45R's
/// auxiliary pump current); a command would turn it into something else.
pub fn demo_script() -> Vec<(f64, Command)> {
    Vec::new()
}

#[cfg(test)]
mod tests;
