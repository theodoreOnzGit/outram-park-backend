//! The shared plant state of the CIET Educational Simulator v2.
//!
//! [`CietState`] is a flat, `Clone`-able snapshot of every quantity the
//! simulator either **accepts as a control input** or **publishes as an
//! output**. It is the single rendezvous point between three threads:
//!
//! | Thread | Access | Role |
//! |---|---|---|
//! | Physics | read controls, write outputs | integrates the CIET loop one timestep at a time |
//! | OPC-UA server | write controls, read outputs | serves remote clients (IEC 62541) |
//! | GUI (desktop only) | write controls, read outputs | the egui front end |
//!
//! Shared as [`SharedCietState`] = `Arc<RwLock<CietState>>`. `RwLock` rather
//! than `Mutex` per the workspace Rust design rules: the GUI and the OPC-UA
//! read callbacks are both readers and must not serialise against each other
//! while the physics thread is between writes.
//!
//! ## Provenance
//!
//! The field set is carried over from the CIET Educational Simulator **v1**
//! `CIETState`
//! (`crates/tuas_boussinesq_solver/examples/ciet_educational_simulator/ciet_simulator_v1/app/panels_and_pages/ciet_data.rs`),
//! deliberately keeping the same names so the v2 physics port stays a faithful
//! translation rather than a re-derivation. Field semantics, the 150 degC
//! heater killswitch thresholds and the 2-decimal-place rounding of published
//! temperatures are all v1 behaviour.
//!
//! ## What v2 adds over v1
//!
//! The **frequency-response / advanced heater control settings live here**, in
//! shared state. In v1 they lived inside the `eframe::App` struct and were
//! applied from the GUI repaint callback, which means they could only be driven
//! by a human at the keyboard. Moving them into shared state is what lets an
//! OPC-UA client — or the headless Termux build, which has no GUI at all —
//! drive a frequency-response experiment. The signal itself is evaluated once
//! per timestep by the physics thread, not per repaint.
//!
//! ## Status
//!
//! Per `RESPONSIBLE_USE.md` this is **untrusted draft material until
//! human-reviewed**, and the simulator is an **offline educational
//! demonstration** — not for facility operation, reactor control,
//! safety-critical decisions, or licensing.

use core::fmt;
use std::sync::{Arc, RwLock};

use uom::si::f64::{Power, Time};
use uom::si::power::kilowatt;
use uom::si::{angular_velocity::radian_per_second, f64::AngularVelocity, f64::Ratio};

/// The CIET plant state, shared between the physics, OPC-UA and GUI threads.
///
/// See the [module documentation](self) for the threading contract. Cloning is
/// cheap-ish (it is a flat `Copy`-of-scalars struct with one small enum) and is
/// the intended way to take a consistent snapshot without holding the lock:
/// `let snapshot = shared.read().unwrap().clone();`
pub type SharedCietState = Arc<RwLock<CietState>>;

/// Which discretisation of the CIET heater the physics thread should integrate.
///
/// CIET's heater is a porous-media annular test section. The two options trade
/// axial resolution against real-time performance; on a slower machine (or on
/// Termux) the coarse mesh keeps the simulation in real time.
///
/// Carried over unchanged from v1's `HeaterType`.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum HeaterType {
    /// 15 axial nodes (13 heated + 2 head nodes). Default; better resolved.
    InsulatedHeaterV1Fine15Mesh,
    /// 8 axial nodes (6 heated + 2 head nodes). Cheaper; for slower hardware.
    InsulatedHeaterV1Coarse8Mesh,
}

impl fmt::Display for HeaterType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl HeaterType {
    /// Number of axial fluid nodes in the heated section for this mesh choice.
    ///
    /// Used only for display; the physics thread builds both meshes up front
    /// and switches which one it advances.
    pub fn axial_node_count(&self) -> usize {
        match self {
            Self::InsulatedHeaterV1Fine15Mesh => 15,
            Self::InsulatedHeaterV1Coarse8Mesh => 8,
        }
    }
}

/// Advanced heater-control settings: steady power plus an optional sinusoidal
/// perturbation, for frequency-response (Bode) experiments.
///
/// The perturbation is
///
/// `P(t) = steady_state_power_kw + total_amplitude_kw * sin(omega * t)`
///
/// with `omega = angular_velocity_rad_per_s` and `t` the **simulation** time,
/// not wall-clock time. Ported from v1's `FreqResponseAndTransientSettings`
/// with the same formula; what changed is *where* it is evaluated (see the
/// module docs).
///
/// Frequency-response testing of CIET's heater is the experimental technique of
/// De Wet and Poresky (Bode plots of heater-outlet temperature against heater
/// power); this reproduces the input side of that experiment in simulation.
/// **No validation against their published data has been performed here** —
/// that remains open work.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeaterControlSettings {
    /// Master switch. When `false` the heater power is whatever was last
    /// written to [`CietState::heater_power_kilowatts`] directly, and the
    /// settings below are ignored.
    pub advanced_heater_control_switched_on: bool,
    /// When `true`, add the sinusoidal perturbation to the steady power.
    pub frequency_response_switched_on: bool,
    /// Steady-state (mean) heater power. Valid range 0..=15 kW; CIET's heater
    /// is rated to about 10 kW, and the simulator's killswitch trips on
    /// over-temperature well before this ceiling matters.
    pub steady_state_power_kw: f64,
    /// Peak amplitude of the sinusoidal perturbation, kW. Valid range 0..=4.
    pub total_amplitude_kw: f64,
    /// Angular frequency of the perturbation, rad/s. Valid range 0..=10.
    pub angular_velocity_rad_per_s: f64,
}

impl Default for HeaterControlSettings {
    fn default() -> Self {
        Self {
            advanced_heater_control_switched_on: false,
            frequency_response_switched_on: false,
            steady_state_power_kw: 0.0,
            total_amplitude_kw: 0.0,
            angular_velocity_rad_per_s: 0.0,
        }
    }
}

impl HeaterControlSettings {
    /// Heater power demanded at simulation time `current_sim_time`.
    ///
    /// Returns the steady power plus the sinusoid when frequency response is
    /// on, or just the steady power when it is off. Identical formula to v1's
    /// `get_frequency_response_signal` / `get_steady_state_power_signal`.
    pub fn demanded_power(&self, current_sim_time: Time) -> Power {
        if !self.frequency_response_switched_on {
            return Power::new::<kilowatt>(self.steady_state_power_kw);
        }

        let omega = AngularVelocity::new::<radian_per_second>(self.angular_velocity_rad_per_s);
        let angular_phase: Ratio = omega * current_sim_time;
        let angular_phase_f64: f64 = angular_phase.into();
        let sinusoid_signal = self.total_amplitude_kw * angular_phase_f64.sin();

        Power::new::<kilowatt>(self.steady_state_power_kw + sinusoid_signal)
    }

    /// Human-readable description of the perturbation signal, for the UI.
    pub fn sine_wave_label(&self) -> String {
        format!(
            "Perturbation signal: {} (kW) * sin({} (rad/s) * t)",
            self.total_amplitude_kw, self.angular_velocity_rad_per_s
        )
    }
}

/// Flat snapshot of the whole CIET loop: controls in, measurements out.
///
/// Naming follows CIET's instrument tags so that a reader who knows the
/// facility can find things: `BT-11` is a bulk-temperature thermocouple,
/// `FM-40` a flowmeter, and bare `pipe_N` fields are the simulated bulk
/// temperature of loop component `N` from the CIET nodalisation diagram.
/// Temperatures are degrees Celsius, mass flows kg/s, power kW, pressure Pa.
///
/// `f32` on the `pipe_*` fields (and `f64` on the instrumented `bt_*`/`fm_*`
/// fields) is v1's choice, kept for fidelity: the `pipe_*` values feed the
/// schematic colouring, the instrumented values feed plots and OPC-UA.
#[derive(Debug, Clone, PartialEq)]
pub struct CietState {
    // ---- time diagnostics ----
    /// Simulated time elapsed, seconds. Advances by the timestep each loop.
    pub simulation_time_seconds: f64,
    /// Wall-clock time elapsed since the physics thread started, seconds.
    /// Compare with `simulation_time_seconds` to see whether the simulation is
    /// keeping up with real time.
    pub elapsed_time_seconds: f64,
    /// Wall-clock cost of the last timestep, milliseconds.
    pub calc_time_ms: f64,

    // ---- user inputs / controls ----
    /// Electrical power into the heater, kW. Written by the GUI, by an OPC-UA
    /// client, or by the advanced-heater-control driver. Forced to zero by the
    /// over-temperature killswitch.
    pub heater_power_kilowatts: f64,
    /// CTAH pump fluid temperature, degC (diagnostic readout).
    pub ctah_pump_temp_degc: f64,
    /// Set point for the CTAH outlet temperature (BT-41), degC. Drives the
    /// CTAH's PID-controlled air-side heat transfer coefficient.
    pub bt_41_ctah_outlet_set_pt_deg_c: f64,
    /// Set point for the TCHX outlet temperature (BT-66), degC. Drives the
    /// TCHX's PID-controlled air-side heat transfer coefficient.
    pub bt_66_tchx_outlet_set_pt_deg_c: f64,

    // ---- heater branch ----
    /// Pipe 18 bulk temperature, degC.
    pub pipe_18_temp_degc: f32,
    /// Heater top head (1a) bulk temperature, degC.
    pub pipe_1a_temp_degc: f32,
    /// BT-11, heater inlet bulk temperature, degC.
    pub bt_11_heater_inlet_deg_c: f64,
    /// BT-12, heater outlet bulk temperature, degC.
    pub bt_12_heater_outlet_deg_c: f64,
    /// Heater bottom head (1b) bulk temperature, degC.
    pub pipe_1b_temp_degc: f32,
    /// Pipe 2a bulk temperature, degC.
    pub pipe_2a_temp_degc: f32,
    /// Static mixer 10 (label 2) bulk temperature, degC.
    pub pipe_2_temp_degc: f32,
    /// Pipe 3 bulk temperature, degC.
    pub pipe_3_temp_degc: f32,
    /// Pipe 4 bulk temperature, degC.
    pub pipe_4_temp_degc: f32,

    // ---- DHX branch ----
    /// Pipe 5a bulk temperature, degC.
    pub pipe_5a_temp_degc: f32,
    /// Pipe 26 bulk temperature, degC.
    pub pipe_26_temp_degc: f32,
    /// Static mixer 21 (label 25) bulk temperature, degC.
    pub pipe_25_temp_degc: f32,
    /// Pipe 25a bulk temperature, degC.
    pub pipe_25a_temp_degc: f32,
    /// BT-21 on the DHX shell inlet, degC.
    pub bt_21_dhx_shell_inlet_deg_c: f64,
    /// Static mixer 20 (label 23) bulk temperature, degC.
    pub pipe_23_temp_degc: f32,
    /// BT-27 on the DHX shell outlet, degC.
    pub bt_27_dhx_shell_outlet_deg_c: f64,
    /// Pipe 23a bulk temperature, degC.
    pub pipe_23a_temp_degc: f32,
    /// Pipe 22 bulk temperature, degC.
    pub pipe_22_temp_degc: f32,
    /// Flowmeter FM-20 (label 21a) fluid temperature, degC.
    pub fm20_label_21a_temp_degc: f32,
    /// FM-20, DHX-branch mass flowrate, kg/s.
    pub fm20_dhx_branch_kg_per_s: f32,
    /// Pipe 21 bulk temperature, degC.
    pub pipe_21_temp_degc: f32,
    /// Pipe 20 bulk temperature, degC.
    pub pipe_20_temp_degc: f32,
    /// Pipe 19 bulk temperature, degC.
    pub pipe_19_temp_degc: f32,
    /// Pipe 17b bulk temperature, degC.
    pub pipe_17b_temp_degc: f32,

    // ---- DRACS loop, hot branch ----
    /// BT-21 on the DHX tube outlet, degC.
    pub bt_21_dhx_tube_outlet_deg_c: f64,
    /// DHX tube side 30b bulk temperature, degC.
    pub pipe_30b_temp_degc: f32,
    /// Pipe 31a bulk temperature, degC.
    pub pipe_31a_temp_degc: f32,
    /// Static mixer 61 (label 31) bulk temperature, degC.
    pub pipe_31_temp_degc: f32,
    /// Pipe 32 bulk temperature, degC.
    pub pipe_32_temp_degc: f32,
    /// Pipe 33 bulk temperature, degC.
    pub pipe_33_temp_degc: f32,
    /// Pipe 34 bulk temperature, degC.
    pub pipe_34_temp_degc: f32,
    /// BT-65, TCHX inlet bulk temperature, degC.
    pub bt_65_tchx_inlet_deg_c: f64,

    // ---- DRACS loop, cold branch ----
    /// BT-66, TCHX outlet bulk temperature, degC. The PID-controlled variable.
    pub bt_66_tchx_outlet_deg_c: f64,
    /// TCHX air-side heat transfer coefficient, W/(m^2 K), as commanded by the
    /// TCHX PID controller. Output only.
    pub tchx_htc_watt_per_m2_kelvin: f64,
    /// Pipe 36a bulk temperature, degC.
    pub pipe_36a_temp_degc: f32,
    /// Static mixer 60 (label 36) bulk temperature, degC.
    pub pipe_36_temp_degc: f32,
    /// Pipe 37 bulk temperature, degC.
    pub pipe_37_temp_degc: f32,
    /// FM-60, DRACS-loop mass flowrate, kg/s (absolute value; v1 solves the
    /// DRACS loop for magnitude only, so reverse flow is not represented).
    pub fm_60_dracs_kg_per_s: f64,
    /// Flowmeter FM-60 (label 37a) fluid temperature, degC.
    pub fm60_label_37a_temp_degc: f32,
    /// Pipe 38 bulk temperature, degC.
    pub pipe_38_temp_degc: f32,
    /// Pipe 39 bulk temperature, degC.
    pub pipe_39_temp_degc: f32,
    /// DHX tube side 30a bulk temperature, degC.
    pub pipe_30a_temp_degc: f32,
    /// BT-60 on the DHX tube inlet, degC.
    pub bt_60_dhx_tube_inlet_deg_c: f64,

    // ---- CTAH branch ----
    /// Pipe 5b bulk temperature, degC.
    pub pipe_5b_temp_degc: f32,
    /// Pipe 6a bulk temperature, degC.
    pub pipe_6a_temp_degc: f32,
    /// Static mixer 41 (label 6) bulk temperature, degC.
    pub pipe_6_temp_degc: f32,
    /// BT-43, CTAH inlet bulk temperature, degC.
    pub bt_43_ctah_inlet_deg_c: f64,
    /// BT-41, CTAH outlet bulk temperature, degC. The PID-controlled variable.
    pub bt_41_ctah_outlet_deg_c: f64,
    /// CTAH air-side heat transfer coefficient, W/(m^2 K), as commanded by the
    /// CTAH PID controller. Output only.
    pub ctah_htc_watt_per_m2_kelvin: f64,
    /// Pipe 8a bulk temperature, degC.
    pub pipe_8a_temp_degc: f32,
    /// Static mixer 40 (label 8) bulk temperature, degC.
    pub pipe_8_temp_degc: f32,
    /// Pipe 9 bulk temperature, degC.
    pub pipe_9_temp_degc: f32,
    /// Pipe 10 bulk temperature, degC.
    pub pipe_10_temp_degc: f32,
    /// Pipe 11 bulk temperature, degC.
    pub pipe_11_temp_degc: f32,
    /// Pipe 12 bulk temperature, degC.
    pub pipe_12_temp_degc: f32,
    /// Pipe 13 bulk temperature, degC.
    pub pipe_13_temp_degc: f32,
    /// Pipe 14 bulk temperature, degC.
    pub pipe_14_temp_degc: f32,
    /// Flowmeter FM-40 (label 14a) fluid temperature, degC.
    pub fm40_label_14a_temp_degc: f32,
    /// FM-40, CTAH-branch mass flowrate, kg/s. Signed: negative means reverse
    /// (upward) flow through the CTAH branch.
    pub fm40_ctah_branch_kg_per_s: f64,
    /// Pipe 15 bulk temperature, degC.
    pub pipe_15_temp_degc: f32,
    /// Pipe 16 bulk temperature, degC.
    pub pipe_16_temp_degc: f32,
    /// Pipe 17a bulk temperature, degC.
    pub pipe_17a_temp_degc: f32,

    // ---- mixing nodes ----
    /// Top mixing node joining branches 5a, 5b and 4, degC.
    pub top_mixing_node_5a_5b_4_temp_degc: f32,
    /// Bottom mixing node joining branches 17a, 17b and 18, degC.
    pub bottom_mixing_node_17a_17b_18_temp_degc: f32,

    // ---- timestep / pacing controls ----
    /// User-requested timestep, seconds. Only honoured when
    /// `slow_motion_settings_turned_on` is `true`, and clamped to 0.1 s
    /// regardless — larger steps break the Courant-number stability limit.
    pub timestep_seconds: f32,
    /// Run faster than real time where the machine allows it.
    pub fast_forward_settings_turned_on: bool,
    /// Run slower than real time, honouring `timestep_seconds`.
    pub slow_motion_settings_turned_on: bool,

    // ---- pump and valve controls ----
    /// Pressure rise across the CTAH pump, Pa. This is the forced-circulation
    /// driver. Valid range -17000..=17000 Pa; negative values reverse the flow.
    pub ctah_pump_pressure_pascals: f32,
    /// Block flow through the CTAH branch (as if a valve were shut).
    pub is_ctah_branch_blocked: bool,
    /// Block flow through the DHX branch.
    pub is_dhx_branch_blocked: bool,

    /// Which heater discretisation the physics thread integrates.
    pub current_heater_type: HeaterType,

    /// Advanced heater control / frequency-response settings. v2 addition to
    /// shared state — see the module docs.
    pub heater_control: HeaterControlSettings,
}

impl Default for CietState {
    /// Isothermal 21 degC loop at rest: no heater power, no pump pressure, no
    /// branch blocked, fine heater mesh, 0.1 s timestep.
    fn default() -> Self {
        // Every temperature starts at laboratory ambient, matching v1.
        const T0_F32: f32 = 21.0;
        const T0_F64: f64 = 21.0;

        Self {
            simulation_time_seconds: 0.0,
            elapsed_time_seconds: 0.0,
            calc_time_ms: 0.0,

            heater_power_kilowatts: 0.0,
            ctah_pump_temp_degc: T0_F64,
            bt_41_ctah_outlet_set_pt_deg_c: T0_F64,
            bt_66_tchx_outlet_set_pt_deg_c: T0_F64,

            pipe_18_temp_degc: T0_F32,
            pipe_1a_temp_degc: T0_F32,
            bt_11_heater_inlet_deg_c: T0_F64,
            bt_12_heater_outlet_deg_c: T0_F64,
            pipe_1b_temp_degc: T0_F32,
            pipe_2a_temp_degc: T0_F32,
            pipe_2_temp_degc: T0_F32,
            pipe_3_temp_degc: T0_F32,
            pipe_4_temp_degc: T0_F32,

            pipe_5a_temp_degc: T0_F32,
            pipe_26_temp_degc: T0_F32,
            pipe_25_temp_degc: T0_F32,
            pipe_25a_temp_degc: T0_F32,
            bt_21_dhx_shell_inlet_deg_c: T0_F64,
            pipe_23_temp_degc: T0_F32,
            bt_27_dhx_shell_outlet_deg_c: T0_F64,
            pipe_23a_temp_degc: T0_F32,
            pipe_22_temp_degc: T0_F32,
            fm20_label_21a_temp_degc: T0_F32,
            fm20_dhx_branch_kg_per_s: 0.0,
            pipe_21_temp_degc: T0_F32,
            pipe_20_temp_degc: T0_F32,
            pipe_19_temp_degc: T0_F32,
            pipe_17b_temp_degc: T0_F32,

            bt_21_dhx_tube_outlet_deg_c: T0_F64,
            pipe_30b_temp_degc: T0_F32,
            pipe_31a_temp_degc: T0_F32,
            pipe_31_temp_degc: T0_F32,
            pipe_32_temp_degc: T0_F32,
            pipe_33_temp_degc: T0_F32,
            pipe_34_temp_degc: T0_F32,
            bt_65_tchx_inlet_deg_c: T0_F64,

            bt_66_tchx_outlet_deg_c: T0_F64,
            tchx_htc_watt_per_m2_kelvin: 0.0,
            pipe_36a_temp_degc: T0_F32,
            pipe_36_temp_degc: T0_F32,
            pipe_37_temp_degc: T0_F32,
            fm_60_dracs_kg_per_s: 0.0,
            fm60_label_37a_temp_degc: T0_F32,
            pipe_38_temp_degc: T0_F32,
            pipe_39_temp_degc: T0_F32,
            pipe_30a_temp_degc: T0_F32,
            bt_60_dhx_tube_inlet_deg_c: T0_F64,

            pipe_5b_temp_degc: T0_F32,
            pipe_6a_temp_degc: T0_F32,
            pipe_6_temp_degc: T0_F32,
            bt_43_ctah_inlet_deg_c: T0_F64,
            bt_41_ctah_outlet_deg_c: T0_F64,
            ctah_htc_watt_per_m2_kelvin: 0.0,
            pipe_8a_temp_degc: T0_F32,
            pipe_8_temp_degc: T0_F32,
            pipe_9_temp_degc: T0_F32,
            pipe_10_temp_degc: T0_F32,
            pipe_11_temp_degc: T0_F32,
            pipe_12_temp_degc: T0_F32,
            pipe_13_temp_degc: T0_F32,
            pipe_14_temp_degc: T0_F32,
            fm40_label_14a_temp_degc: T0_F32,
            fm40_ctah_branch_kg_per_s: 0.0,
            pipe_15_temp_degc: T0_F32,
            pipe_16_temp_degc: T0_F32,
            pipe_17a_temp_degc: T0_F32,

            top_mixing_node_5a_5b_4_temp_degc: T0_F32,
            bottom_mixing_node_17a_17b_18_temp_degc: T0_F32,

            timestep_seconds: 0.1,
            fast_forward_settings_turned_on: false,
            slow_motion_settings_turned_on: false,

            ctah_pump_pressure_pascals: 0.0,
            is_ctah_branch_blocked: false,
            is_dhx_branch_blocked: false,

            current_heater_type: HeaterType::InsulatedHeaterV1Fine15Mesh,
            heater_control: HeaterControlSettings::default(),
        }
    }
}

/// Hard bound on the CTAH pump pressure rise, Pa.
///
/// CIET's CTAH pump cannot deliver more than about 17 kPa; the simulator
/// enforces the same envelope in both directions so a client cannot drive the
/// loop into a non-physical regime. v1 applied this bound in its GUI slider.
pub const CTAH_PUMP_PRESSURE_LIMIT_PASCALS: f64 = 17000.0;

/// Hard bound on requested heater power, kW.
///
/// CIET's heater is rated near 10 kW; 15 kW leaves headroom for transient
/// experiments while keeping the killswitch (150 degC at heater inlet/outlet,
/// 160 degC in any fluid node, 350 degC in any shell node) as the real
/// protection.
pub const HEATER_POWER_LIMIT_KILOWATTS: f64 = 15.0;

/// Largest timestep the solver is allowed to take, seconds.
///
/// Above this the advection Courant number in the shortest loop component
/// exceeds unity and the explicit advection coupling goes unstable. Enforced by
/// the physics thread regardless of what a client requests.
pub const MAX_TIMESTEP_SECONDS: f64 = 0.1;

impl CietState {
    /// Overwrite this state wholesale with another snapshot.
    ///
    /// Used by the physics thread to publish its results in one write while
    /// holding the lock for as short a time as possible. Carried over from v1.
    pub fn overwrite_state(&mut self, ciet_state: Self) {
        *self = ciet_state;
    }

    /// Heater electrical power, kW.
    pub fn get_heater_power_kilowatts(&self) -> f64 {
        self.heater_power_kilowatts
    }

    /// Set the heater electrical power, kW, clamped to
    /// `0..=`[`HEATER_POWER_LIMIT_KILOWATTS`].
    ///
    /// Clamping (rather than v1's unchecked assignment) is deliberate: this
    /// setter is now reachable from a remote OPC-UA client, so an out-of-range
    /// write must not be able to push the solver somewhere non-physical.
    pub fn set_heater_power_kilowatts(&mut self, heater_power_kw: f64) {
        self.heater_power_kilowatts = heater_power_kw.clamp(0.0, HEATER_POWER_LIMIT_KILOWATTS);
    }

    /// BT-12, heater outlet temperature, degC.
    pub fn get_heater_outlet_temp_degc(&self) -> f64 {
        self.bt_12_heater_outlet_deg_c
    }

    /// BT-11, heater inlet temperature, degC.
    pub fn get_heater_inlet_temp_degc(&self) -> f64 {
        self.bt_11_heater_inlet_deg_c
    }

    /// BT-27, DHX shell outlet temperature, degC.
    pub fn get_dhx_shell_outlet_temp_degc(&self) -> f64 {
        self.bt_27_dhx_shell_outlet_deg_c
    }

    /// BT-21, DHX shell inlet temperature, degC.
    pub fn get_dhx_shell_inlet_temp_degc(&self) -> f64 {
        self.bt_21_dhx_shell_inlet_deg_c
    }

    /// BT-21, DHX tube outlet temperature, degC.
    pub fn get_dhx_tube_outlet_temp_degc(&self) -> f64 {
        self.bt_21_dhx_tube_outlet_deg_c
    }

    /// BT-60, DHX tube inlet temperature, degC.
    pub fn get_dhx_tube_inlet_temp_degc(&self) -> f64 {
        self.bt_60_dhx_tube_inlet_deg_c
    }

    /// BT-66, TCHX outlet temperature, degC.
    pub fn get_tchx_outlet_temp_degc(&self) -> f64 {
        self.bt_66_tchx_outlet_deg_c
    }

    /// BT-65, TCHX inlet temperature, degC.
    pub fn get_tchx_inlet_temp_degc(&self) -> f64 {
        self.bt_65_tchx_inlet_deg_c
    }

    /// BT-41, CTAH outlet temperature, degC.
    pub fn get_ctah_outlet_temp_degc(&self) -> f64 {
        self.bt_41_ctah_outlet_deg_c
    }

    /// BT-43, CTAH inlet temperature, degC.
    pub fn get_ctah_inlet_temp_degc(&self) -> f64 {
        self.bt_43_ctah_inlet_deg_c
    }

    /// Requested timestep, seconds.
    pub fn get_timestep_seconds(&self) -> f32 {
        self.timestep_seconds
    }

    /// Set the requested timestep, seconds, clamped to
    /// `0.001..=`[`MAX_TIMESTEP_SECONDS`].
    pub fn set_timestep_seconds(&mut self, timestep_seconds: f64) {
        self.timestep_seconds = timestep_seconds.clamp(0.001, MAX_TIMESTEP_SECONDS) as f32;
    }

    /// Whether fast-forward pacing is on.
    pub fn is_fast_fwd_on(&self) -> bool {
        self.fast_forward_settings_turned_on
    }

    /// CTAH pump pressure rise, Pa, as an `f64`.
    pub fn get_ctah_pump_pressure_f64(&self) -> f64 {
        self.ctah_pump_pressure_pascals as f64
    }

    /// Set the CTAH pump pressure rise, Pa, clamped to
    /// +/-[`CTAH_PUMP_PRESSURE_LIMIT_PASCALS`]. Negative reverses the flow.
    pub fn set_ctah_pump_pressure_pascals(&mut self, pressure_pascals: f64) {
        self.ctah_pump_pressure_pascals = pressure_pascals.clamp(
            -CTAH_PUMP_PRESSURE_LIMIT_PASCALS,
            CTAH_PUMP_PRESSURE_LIMIT_PASCALS,
        ) as f32;
    }
}

/// Build a fresh shared state handle at defaults (isothermal 21 degC, at rest).
pub fn new_shared_state() -> SharedCietState {
    Arc::new(RwLock::new(CietState::default()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::time::second;

    /// Verification of the advanced-heater-control signal against its closed
    /// form.
    ///
    /// **Methodology.** The perturbation is defined as
    /// `P(t) = P_ss + A sin(omega t)`. With `P_ss = 8 kW`, `A = 2 kW`,
    /// `omega = pi/2 rad/s`, the analytical values at `t = 0, 1, 2, 3 s` are
    /// `8, 10, 8, 6 kW` exactly (`sin` at `0, pi/2, pi, 3pi/2`). Pass criterion:
    /// agreement to 1e-9 kW, i.e. floating-point round-off only.
    ///
    /// **Results (2026-07-28).** All four sample points matched to within
    /// 1e-12 kW — far inside the 1e-9 kW criterion. This verifies the signal
    /// generator reproduces its own definition; it says nothing about whether
    /// the CIET *plant response* to such a perturbation is physically correct,
    /// which is a validation question still open against De Wet's and
    /// Poresky's published Bode data.
    #[test]
    fn frequency_response_signal_matches_closed_form() {
        use core::f64::consts::PI;

        let settings = HeaterControlSettings {
            advanced_heater_control_switched_on: true,
            frequency_response_switched_on: true,
            steady_state_power_kw: 8.0,
            total_amplitude_kw: 2.0,
            angular_velocity_rad_per_s: PI / 2.0,
        };

        let expected_kw = [8.0, 10.0, 8.0, 6.0];
        for (seconds, expected) in expected_kw.iter().enumerate() {
            let t = Time::new::<second>(seconds as f64);
            let got = settings.demanded_power(t).get::<kilowatt>();
            assert!(
                (got - expected).abs() < 1e-9,
                "at t = {seconds} s expected {expected} kW, got {got} kW"
            );
        }
    }

    /// Verifies the control setters clamp to their documented envelopes, which
    /// is what protects the solver from a hostile or careless OPC-UA write.
    ///
    /// **Methodology.** Write values far outside each documented range and
    /// assert the stored value is the range endpoint. Pass criterion: exact
    /// equality with the documented limit.
    ///
    /// **Results (2026-07-28).** Heater power clamps 1e6 kW -> 15 kW and
    /// -5 kW -> 0 kW, both exact (`f64` field). Pump pressure clamps
    /// +/-1e9 Pa -> +/-17000 Pa, exact because 17000 is exactly representable in
    /// `f32`. The timestep clamps 10 s -> 0.1 s but reads back as
    /// 0.10000000149011612, because `timestep_seconds` is an `f32` field
    /// inherited unchanged from v1's `CIETState` and 0.1 has no exact `f32`
    /// representation. The 1.5e-9 s discrepancy is physically irrelevant — nine
    /// orders of magnitude below the 0.1 s step — but it is real, and it is why
    /// this assertion uses an `f32`-aware tolerance rather than `assert_eq!`.
    /// Found by this test failing on an exact-equality criterion, not by
    /// inspection.
    ///
    /// The `f32` storage is retained deliberately: keeping v1's field types
    /// identical is what makes the v2 physics port a faithful translation, and
    /// widening the field would be a silent behavioural change to a solver the
    /// maintainer has already done validation work on.
    #[test]
    fn control_setters_clamp_to_documented_envelopes() {
        let mut state = CietState::default();

        state.set_heater_power_kilowatts(1.0e6);
        assert_eq!(state.heater_power_kilowatts, HEATER_POWER_LIMIT_KILOWATTS);
        state.set_heater_power_kilowatts(-5.0);
        assert_eq!(state.heater_power_kilowatts, 0.0);

        state.set_ctah_pump_pressure_pascals(1.0e9);
        assert_eq!(
            state.ctah_pump_pressure_pascals as f64,
            CTAH_PUMP_PRESSURE_LIMIT_PASCALS
        );
        state.set_ctah_pump_pressure_pascals(-1.0e9);
        assert_eq!(
            state.ctah_pump_pressure_pascals as f64,
            -CTAH_PUMP_PRESSURE_LIMIT_PASCALS
        );

        // `f32`-backed: compare with a relative f32 tolerance, not exactly.
        state.set_timestep_seconds(10.0);
        let stored_timestep = state.timestep_seconds as f64;
        let f32_tolerance = MAX_TIMESTEP_SECONDS * f32::EPSILON as f64;
        assert!(
            (stored_timestep - MAX_TIMESTEP_SECONDS).abs() <= f32_tolerance,
            "timestep clamped to {stored_timestep} s, expected \
             {MAX_TIMESTEP_SECONDS} s within {f32_tolerance}"
        );
    }
}
