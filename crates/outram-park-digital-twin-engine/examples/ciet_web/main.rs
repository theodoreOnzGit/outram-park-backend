//! CIET Educational Simulator v2 in the browser (gh:#924).
//!
//! The physics is the desktop simulator's own: `CietLoop`
//! (`src/bin/ciet_educational_simulator_v2/.../full_simulation/stepper.rs`)
//! and the solver modules it calls are included here by path, so there is one
//! copy of the CIET model and this file adds none. What it adds is a [`Plant`]
//! (the stepper plus the plant state, and the controls a reader may set) and,
//! on `wasm32`, a `wasm-bindgen` face for it that a Web Worker drives
//! (`web/ciet/worker.js`).
//!
//! ## Threads, natively and in the browser
//!
//! | desktop (v2) | browser (this demo) |
//! |---|---|
//! | GUI thread draws | the page's main thread draws (plain JS, canvas) |
//! | physics thread steps and paces to real time | a Web Worker steps and paces to real time |
//! | per step: DRACS flow, primary flows (two of them) on threads beside the heat transfer | the same solves run inline (`fork`): `wasm32` has no threads |
//!
//! The inline order gives the same numbers as the threaded one, bit for bit
//! (`stepper`'s `the_stepper_is_deterministic_and_inline_equals_threaded`).
//!
//! ## Scope
//!
//! Education and research only (`RESPONSIBLE_USE.md`): an offline
//! demonstration, never connected to a plant. The physics is a faithful port
//! of v1, which the maintainer validated; **v1-to-v2 equivalence has not been
//! verified** (see the stepper's module docs), and this page says so.

#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

use outram_park_digital_twin_engine::ciet_opcua::state::{CietState, HeaterType};
use uom::si::f64::Time;
use uom::si::time::second;

#[path = "../../src/bin/ciet_educational_simulator_v2/ciet_simulator_v2/app/panels_and_pages/full_simulation/fork.rs"]
mod fork;

#[path = "../../src/bin/ciet_educational_simulator_v2/ciet_simulator_v2/app/panels_and_pages/full_simulation/fluid_solvers.rs"]
mod fluid_solvers;

#[path = "../../src/bin/ciet_educational_simulator_v2/ciet_simulator_v2/app/panels_and_pages/full_simulation/heat_transfer_solvers.rs"]
mod heat_transfer_solvers;

#[path = "../../src/bin/ciet_educational_simulator_v2/ciet_simulator_v2/app/panels_and_pages/full_simulation/diagnostics.rs"]
mod diagnostics;

#[path = "../../src/bin/ciet_educational_simulator_v2/ciet_simulator_v2/app/panels_and_pages/full_simulation/stepper.rs"]
mod stepper;

use stepper::{max_timestep, CietLoop, FlowSource, FlowSnapshot, GivenFlows};

/// What [`Plant::readout`] returns, in order. Temperatures in degC, flows in
/// kg/s, heat transfer coefficients in W/(m^2 K), time in s, power in kW.
/// The `pipe_*` entries are the first control volume of each CIET component,
/// named by its number on the facility drawing, as the desktop main page
/// shows them.
pub const FIELDS: [&str; 79] = [
    "time_s",
    "heater_kw",
    "flow_dhx_branch",
    "flow_heater_branch",
    "flow_ctah_branch",
    "flow_dracs",
    "bt_11_heater_inlet",
    "bt_12_heater_outlet",
    "bt_21_dhx_shell_inlet",
    "bt_27_dhx_shell_outlet",
    "bt_60_dhx_tube_inlet",
    "bt_21_dhx_tube_outlet",
    "bt_65_tchx_inlet",
    "bt_66_tchx_outlet",
    "bt_43_ctah_inlet",
    "bt_41_ctah_outlet",
    "tchx_htc",
    "ctah_htc",
    "pipe_1a",
    "pipe_1b",
    "pipe_2",
    "pipe_2a",
    "pipe_3",
    "pipe_4",
    "pipe_18",
    "pipe_5a",
    "pipe_26",
    "pipe_25a",
    "pipe_25",
    "pipe_23",
    "pipe_23a",
    "pipe_22",
    "pipe_21a",
    "pipe_21",
    "pipe_20",
    "pipe_19",
    "pipe_17b",
    "pipe_5b",
    "pipe_6",
    "pipe_6a",
    "pipe_8a",
    "pipe_8",
    "pipe_9",
    "pipe_10",
    "pipe_11",
    "pipe_12",
    "pipe_ctah_pump",
    "pipe_13",
    "pipe_14",
    "pipe_14a",
    "pipe_15",
    "pipe_16",
    "pipe_17a",
    "pipe_30a",
    "pipe_30b",
    "pipe_31",
    "pipe_31a",
    "pipe_32",
    "pipe_33",
    "pipe_34",
    "pipe_36",
    "pipe_36a",
    "pipe_37",
    "pipe_37a",
    "pipe_38",
    "pipe_39",
    "node_top",
    "node_bottom",
    "pump_pa",
    "tchx_setpoint",
    "ctah_setpoint",
    "ctah_blocked",
    "dhx_blocked",
    "coarse_heater",
    "timestep_s",
    "slow_motion",
    "dhx_shell_mean",
    "dhx_tube_mean",
    "heater_mean",
];

/// The CIET loop as the browser drives it: the desktop stepper, the plant
/// state it reads its controls from and writes its measurements to, and the
/// timestep it takes.
pub struct Plant {
    ciet: CietLoop,
    state: CietState,
    /// The timestep taken while not in slow motion: the starting state's,
    /// an `f32` (0.10000000149 s), as the desktop loop takes while it keeps
    /// up with real time (see `stepper`'s module docs).
    realtime_timestep: Time,
}

impl Default for Plant {
    fn default() -> Self {
        Self::new()
    }
}

impl Plant {
    /// The plant as the desktop simulator starts: isothermal at 21 degC, at
    /// rest, heater off ([`CietState::default`]).
    pub fn new() -> Self {
        let state = CietState::default();
        // Not clamped: 0.1 as an f32 is 1.5e-9 s over `max_timestep`, and the
        // desktop loop takes it unclamped outside slow motion.
        let realtime_timestep = Time::new::<second>(state.get_timestep_seconds() as f64);
        Self { ciet: CietLoop::new(&state), state, realtime_timestep }
    }

    /// Set one control, clamped as the desktop and OPC-UA paths clamp it.
    ///
    /// | name | meaning |
    /// |---|---|
    /// | `heater_kw` | heater power, 0 to the state's limit |
    /// | `pump_pa` | CTAH pump pressure rise, +/- the state's limit (negative reverses) |
    /// | `tchx_setpoint`, `ctah_setpoint` | cooler outlet set points, 25-110 degC (the desktop sliders' range) |
    /// | `ctah_blocked`, `dhx_blocked` | a branch valve shut (non-zero) or open |
    /// | `coarse_heater` | the 8-node heater mesh (non-zero) or the 15-node one |
    /// | `slow_motion`, `timestep_s` | take `timestep_s` per step instead of 0.1 s |
    ///
    /// # Errors
    ///
    /// An unknown name, or a value that is not finite.
    pub fn set(&mut self, name: &str, value: f64) -> Result<(), String> {
        if !value.is_finite() {
            return Err(format!("{name}: {value} is not a number"));
        }
        let s = &mut self.state;
        match name {
            "heater_kw" => s.set_heater_power_kilowatts(value),
            "pump_pa" => s.set_ctah_pump_pressure_pascals(value),
            "tchx_setpoint" => s.bt_66_tchx_outlet_set_pt_deg_c = value.clamp(25.0, 110.0),
            "ctah_setpoint" => s.bt_41_ctah_outlet_set_pt_deg_c = value.clamp(25.0, 110.0),
            "ctah_blocked" => s.is_ctah_branch_blocked = value != 0.0,
            "dhx_blocked" => s.is_dhx_branch_blocked = value != 0.0,
            "coarse_heater" => {
                s.current_heater_type = if value != 0.0 {
                    HeaterType::InsulatedHeaterV1Coarse8Mesh
                } else {
                    HeaterType::InsulatedHeaterV1Fine15Mesh
                }
            }
            "slow_motion" => s.slow_motion_settings_turned_on = value != 0.0,
            "timestep_s" => s.set_timestep_seconds(value),
            _ => return Err(format!("no control named {name}")),
        }
        Ok(())
    }

    /// The timestep the next step takes: the desktop's while it keeps real
    /// time (0.1 s as an `f32`), or the reader's own with slow motion on,
    /// never above 0.1 s (the desktop loop's rule).
    pub fn timestep(&self) -> Time {
        if self.state.slow_motion_settings_turned_on {
            Time::new::<second>(self.state.get_timestep_seconds() as f64).min(max_timestep())
        } else {
            self.realtime_timestep
        }
    }

    /// Advance `steps` timesteps, the flows solved in each step (inline on
    /// `wasm32`): the one-worker mode.
    pub fn advance(&mut self, steps: u32) {
        for _ in 0..steps {
            let dt = self.timestep();
            self.ciet.step(&mut self.state, dt, false);
        }
    }

    /// The multi-worker mode, first half of a step: what the flow workers
    /// solve, taken before the step.
    pub fn snapshot(&self) -> FlowSnapshot {
        self.ciet.flow_snapshot(&self.state)
    }

    /// The multi-worker mode, the step's heat transfer, while the flow
    /// workers solve [`Plant::snapshot`]. [`Plant::finish`] must follow.
    pub fn begin_step(&mut self) {
        let dt = self.timestep();
        self.ciet.step_with(&mut self.state, dt, false, FlowSource::Later);
    }

    /// The multi-worker mode, the step's end: the flows the workers solved,
    /// kg/s (DRACS magnitude; DHX, heater, CTAH branches).
    pub fn finish(&mut self, dracs: f64, primary: [f64; 3]) {
        self.ciet.finish_step(&mut self.state, GivenFlows::from_kg_per_s(dracs, primary));
    }

    /// Simulation time reached, s.
    pub fn simulation_time_s(&self) -> f64 {
        self.ciet.simulation_time().get::<second>()
    }

    /// The plant now, in the order of [`FIELDS`].
    pub fn readout(&self) -> Vec<f64> {
        let s = &self.state;
        let [dhx, heater, ctah, dracs] = self.ciet.branch_flows_kg_per_s();
        let f = |x: f32| x as f64;
        let b = |x: bool| if x { 1.0 } else { 0.0 };
        vec![
            self.simulation_time_s(),
            s.heater_power_kilowatts,
            dhx,
            heater,
            ctah,
            dracs,
            s.bt_11_heater_inlet_deg_c,
            s.bt_12_heater_outlet_deg_c,
            s.bt_21_dhx_shell_inlet_deg_c,
            s.bt_27_dhx_shell_outlet_deg_c,
            s.bt_60_dhx_tube_inlet_deg_c,
            s.bt_21_dhx_tube_outlet_deg_c,
            s.bt_65_tchx_inlet_deg_c,
            s.bt_66_tchx_outlet_deg_c,
            s.bt_43_ctah_inlet_deg_c,
            s.bt_41_ctah_outlet_deg_c,
            s.tchx_htc_watt_per_m2_kelvin,
            s.ctah_htc_watt_per_m2_kelvin,
            f(s.pipe_1a_temp_degc),
            f(s.pipe_1b_temp_degc),
            f(s.pipe_2_temp_degc),
            f(s.pipe_2a_temp_degc),
            f(s.pipe_3_temp_degc),
            f(s.pipe_4_temp_degc),
            f(s.pipe_18_temp_degc),
            f(s.pipe_5a_temp_degc),
            f(s.pipe_26_temp_degc),
            f(s.pipe_25a_temp_degc),
            f(s.pipe_25_temp_degc),
            f(s.pipe_23_temp_degc),
            f(s.pipe_23a_temp_degc),
            f(s.pipe_22_temp_degc),
            f(s.fm20_label_21a_temp_degc),
            f(s.pipe_21_temp_degc),
            f(s.pipe_20_temp_degc),
            f(s.pipe_19_temp_degc),
            f(s.pipe_17b_temp_degc),
            f(s.pipe_5b_temp_degc),
            f(s.pipe_6_temp_degc),
            f(s.pipe_6a_temp_degc),
            f(s.pipe_8a_temp_degc),
            f(s.pipe_8_temp_degc),
            f(s.pipe_9_temp_degc),
            f(s.pipe_10_temp_degc),
            f(s.pipe_11_temp_degc),
            f(s.pipe_12_temp_degc),
            s.ctah_pump_temp_degc,
            f(s.pipe_13_temp_degc),
            f(s.pipe_14_temp_degc),
            f(s.fm40_label_14a_temp_degc),
            f(s.pipe_15_temp_degc),
            f(s.pipe_16_temp_degc),
            f(s.pipe_17a_temp_degc),
            f(s.pipe_30a_temp_degc),
            f(s.pipe_30b_temp_degc),
            f(s.pipe_31_temp_degc),
            f(s.pipe_31a_temp_degc),
            f(s.pipe_32_temp_degc),
            f(s.pipe_33_temp_degc),
            f(s.pipe_34_temp_degc),
            f(s.pipe_36_temp_degc),
            f(s.pipe_36a_temp_degc),
            f(s.pipe_37_temp_degc),
            f(s.fm60_label_37a_temp_degc),
            f(s.pipe_38_temp_degc),
            f(s.pipe_39_temp_degc),
            f(s.top_mixing_node_5a_5b_4_temp_degc),
            f(s.bottom_mixing_node_17a_17b_18_temp_degc),
            s.get_ctah_pump_pressure_f64(),
            s.bt_66_tchx_outlet_set_pt_deg_c,
            s.bt_41_ctah_outlet_set_pt_deg_c,
            b(s.is_ctah_branch_blocked),
            b(s.is_dhx_branch_blocked),
            b(matches!(s.current_heater_type, HeaterType::InsulatedHeaterV1Coarse8Mesh)),
            s.get_timestep_seconds() as f64,
            b(s.slow_motion_settings_turned_on),
            0.5 * (s.bt_21_dhx_shell_inlet_deg_c + s.bt_27_dhx_shell_outlet_deg_c),
            0.5 * (s.bt_60_dhx_tube_inlet_deg_c + s.bt_21_dhx_tube_outlet_deg_c),
            0.5 * (s.bt_11_heater_inlet_deg_c + s.bt_12_heater_outlet_deg_c),
        ]
    }
}

/// A flow worker's loop (gh:#924): a CIET loop built as the plant's is,
/// which solves the flows of a [`FlowSnapshot`] the plant sends it. v2 runs
/// these solves on threads; the browser runs each on a Web Worker of its own.
pub struct FlowSolver(CietLoop);

impl Default for FlowSolver {
    fn default() -> Self {
        Self(CietLoop::new(&CietState::default()))
    }
}

impl FlowSolver {
    /// The DRACS flow magnitude, kg/s, of a [`FlowSnapshot::dracs`].
    ///
    /// # Errors
    ///
    /// The snapshot does not fit the loop.
    pub fn dracs(&mut self, snapshot: &[f64]) -> Result<f64, String> {
        self.0.solve_dracs_snapshot(snapshot)
    }

    /// The (DHX, heater, CTAH) branch flows, kg/s, of a
    /// [`FlowSnapshot::primary`].
    ///
    /// # Errors
    ///
    /// The snapshot does not fit the loop.
    pub fn primary(&mut self, snapshot: &[f64]) -> Result<[f64; 3], String> {
        self.0.solve_primary_snapshot(snapshot)
    }
}

/// The headless trace (the crate's headless rule): `steps` timesteps of a
/// plant at `heater_kw`, the readout every `every` steps as CSV with a
/// [`FIELDS`] header, fixed precision. Deterministic.
pub fn trace_csv(steps: u32, every: u32, heater_kw: f64) -> String {
    let mut plant = Plant::new();
    plant.set("heater_kw", heater_kw).expect("heater_kw is a control");
    let mut out = FIELDS.join(",");
    out.push('\n');
    let every = every.max(1);
    for i in 1..=steps {
        plant.advance(1);
        if i % every == 0 {
            let row: Vec<String> = plant.readout().iter().map(|x| format!("{x:.6}")).collect();
            out.push_str(&row.join(","));
            out.push('\n');
        }
    }
    out
}

/// Natively: `--headless [steps] [sample_every] [heater_kw]` prints
/// [`trace_csv`]. CLI glue, reached through `trace_csv`'s test.
#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some("--headless") {
        eprintln!("ciet_web is the CIET browser demo's wasm module; build it with web/ciet/build.sh.");
        eprintln!("Natively: --headless [steps=600] [sample_every=10] [heater_kw=8]");
        return;
    }
    let num = |i: usize, d: f64| args.get(i).and_then(|a| a.parse::<f64>().ok()).unwrap_or(d);
    print!("{}", trace_csv(num(1, 600.0) as u32, num(2, 10.0) as u32, num(3, 8.0)));
}

/// On `wasm32` the page loads this module and the worker calls [`web::CietWeb`].
#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(target_arch = "wasm32")]
mod web {
    use wasm_bindgen::prelude::*;

    /// The plant, for the worker (`web/ciet/worker.js`).
    #[wasm_bindgen]
    pub struct CietWeb(super::Plant);

    #[wasm_bindgen]
    impl CietWeb {
        /// A fresh plant ([`super::Plant::new`]).
        #[wasm_bindgen(constructor)]
        pub fn new() -> CietWeb {
            CietWeb(super::Plant::new())
        }

        /// The readout's field names, comma-separated ([`super::FIELDS`]).
        pub fn fields() -> String {
            super::FIELDS.join(",")
        }

        /// Set a control ([`super::Plant::set`]).
        pub fn set(&mut self, name: &str, value: f64) -> Result<(), JsValue> {
            self.0.set(name, value).map_err(|e| JsValue::from_str(&e))
        }

        /// Advance `steps` timesteps.
        pub fn advance(&mut self, steps: u32) {
            self.0.advance(steps);
        }

        /// The timestep the next step takes, s.
        pub fn timestep_s(&self) -> f64 {
            self.0.timestep().get::<uom::si::time::second>()
        }

        /// Simulation time, s.
        pub fn simulation_time_s(&self) -> f64 {
            self.0.simulation_time_s()
        }

        /// The plant now, as a `Float64Array` in [`super::FIELDS`] order.
        pub fn readout(&self) -> Vec<f64> {
            self.0.readout()
        }
        /// Multi-worker mode: the DRACS half of the step's snapshot.
        pub fn snapshot_dracs(&self) -> Vec<f64> {
            self.0.snapshot().dracs
        }

        /// Multi-worker mode: the primary half of the step's snapshot.
        pub fn snapshot_primary(&self) -> Vec<f64> {
            self.0.snapshot().primary
        }

        /// Multi-worker mode: the step's heat transfer.
        pub fn begin_step(&mut self) {
            self.0.begin_step();
        }

        /// Multi-worker mode: the step's end, with the workers' flows.
        pub fn finish(&mut self, dracs: f64, dhx: f64, heater: f64, ctah: f64) {
            self.0.finish(dracs, [dhx, heater, ctah]);
        }
    }

    /// A flow worker's solver ([`super::FlowSolver`]).
    #[wasm_bindgen]
    pub struct CietFlow(super::FlowSolver);

    #[wasm_bindgen]
    impl CietFlow {
        /// A loop built as the plant's.
        #[wasm_bindgen(constructor)]
        pub fn new() -> CietFlow {
            CietFlow(super::FlowSolver::default())
        }

        /// The DRACS flow, kg/s, of a DRACS snapshot.
        pub fn dracs(&mut self, snapshot: &[f64]) -> Result<f64, JsValue> {
            self.0.dracs(snapshot).map_err(|e| JsValue::from_str(&e))
        }

        /// The (DHX, heater, CTAH) flows, kg/s, of a primary snapshot.
        pub fn primary(&mut self, snapshot: &[f64]) -> Result<Vec<f64>, JsValue> {
            self.0.primary(snapshot).map(|f| f.to_vec()).map_err(|e| JsValue::from_str(&e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The browser's plant is the desktop stepper** (gh:#924).
    ///
    /// Methodology: a [`Plant`] set to 8 kW and advanced 200 steps, against
    /// a `CietLoop` stepped by hand on a `CietState` set the same way, at the
    /// desktop loop's real-time step (0.1 s as an `f32`). Pass:
    /// the readout has one value per [`FIELDS`] name, and its plant values
    /// are bit for bit the hand-stepped state's. The controls clamp as the
    /// desktop does and refuse unknown names and non-numbers.
    ///
    /// Result, 2026-10-17 (release): passes.
    #[test]
    fn the_browser_plant_is_the_desktop_stepper() {
        let mut plant = Plant::new();
        plant.set("heater_kw", 8.0).unwrap();
        plant.advance(200);
        let r = plant.readout();
        assert_eq!(r.len(), FIELDS.len());

        let mut state = CietState::default();
        state.set_heater_power_kilowatts(8.0);
        let mut ciet = CietLoop::new(&state);
        let desktop_dt = Time::new::<second>(CietState::default().get_timestep_seconds() as f64);
        assert_ne!(desktop_dt, max_timestep(), "the desktop's step is 0.1 as an f32");
        for _ in 0..200 {
            ciet.step(&mut state, desktop_dt, false);
        }
        assert_eq!(plant.state, state, "the plant state");
        let idx = |n: &str| FIELDS.iter().position(|f| *f == n).expect(n);
        assert_eq!(r[idx("time_s")].to_bits(), ciet.simulation_time().get::<second>().to_bits());
        assert_eq!(r[idx("bt_12_heater_outlet")].to_bits(), state.bt_12_heater_outlet_deg_c.to_bits());
        assert_eq!(r[idx("flow_dhx_branch")].to_bits(), ciet.branch_flows_kg_per_s()[0].to_bits());

        // Controls: clamps and refusals.
        plant.set("heater_kw", 1e9).unwrap();
        plant.set("tchx_setpoint", -5.0).unwrap();
        plant.set("coarse_heater", 1.0).unwrap();
        plant.set("slow_motion", 1.0).unwrap();
        plant.set("timestep_s", 0.01).unwrap();
        let r = plant.readout();
        assert!(r[idx("heater_kw")] < 1e9 && r[idx("heater_kw")] > 8.0, "heater clamped: {}", r[idx("heater_kw")]);
        assert_eq!(r[idx("tchx_setpoint")], 25.0);
        assert_eq!(r[idx("coarse_heater")], 1.0);
        assert!((plant.timestep().get::<second>() - 0.01).abs() < 1e-9);
        assert!(plant.set("no_such_control", 1.0).is_err());
        assert!(plant.set("heater_kw", f64::NAN).is_err());
        plant.advance(1); // the coarse mesh steps too
    }

    /// **The three-worker step is the one-worker step** (gh:#924).
    ///
    /// Methodology: two [`Plant`]s at 8 kW, the pump at 2 kPa from 15 s,
    /// 300 steps: one with [`Plant::advance`], the other as the browser's
    /// plant worker drives it, [`Plant::snapshot`] to two [`FlowSolver`]s
    /// (the DRACS and primary workers), [`Plant::begin_step`], then
    /// [`Plant::finish`] with their flows. Pass: the readouts are equal
    /// after every step. Result, 2026-10-17 (release): passes.
    #[test]
    fn the_three_worker_step_is_the_one_worker_step() {
        let mut one = Plant::new();
        let mut three = Plant::new();
        let (mut dracs_worker, mut primary_worker) = (FlowSolver::default(), FlowSolver::default());
        for p in [&mut one, &mut three] {
            p.set("heater_kw", 8.0).unwrap();
        }
        for i in 0..300 {
            if i == 150 {
                one.set("pump_pa", 2000.0).unwrap();
                three.set("pump_pa", 2000.0).unwrap();
            }
            one.advance(1);
            let snap = three.snapshot();
            three.begin_step();
            let dracs = dracs_worker.dracs(&snap.dracs).unwrap();
            let primary = primary_worker.primary(&snap.primary).unwrap();
            three.finish(dracs, primary);
            assert_eq!(one.readout(), three.readout(), "step {}", i + 1);
        }
        assert!(FlowSolver::default().dracs(&[1.0]).is_err(), "a short snapshot is refused");
    }

    /// **The headless trace is deterministic** (the crate's headless rule).
    ///
    /// Methodology: [`trace_csv`] for 100 steps, sampled every 10, twice.
    /// Pass: byte-identical, a header plus 10 rows of [`FIELDS`]` columns,
    /// time ending at 10.0 s. Result, 2026-10-17 (release): passes.
    #[test]
    fn the_headless_trace_is_deterministic() {
        let a = trace_csv(100, 10, 8.0);
        assert_eq!(a, trace_csv(100, 10, 8.0));
        let lines: Vec<&str> = a.lines().collect();
        assert_eq!(lines.len(), 11);
        assert!(lines.iter().all(|l| l.split(',').count() == FIELDS.len()));
        assert!(lines[10].starts_with("10.000000,"), "{}", lines[10]);
    }
}
