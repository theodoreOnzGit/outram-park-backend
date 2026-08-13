//! The OPC-UA node map: the single source of truth for what CIET v2 exposes.
//!
//! Three enums describe every OPC-UA variable the simulator publishes:
//!
//! | Enum | Direction | OPC-UA type | Meaning |
//! |---|---|---|---|
//! | [`CietSignal`] | read-only (`CurrentRead`) | `Double` | a measurement or diagnostic the simulator produces |
//! | [`CietControl`] | read/write (`CurrentRead \| CurrentWrite`) | `Double` | a continuous set point a client may drive |
//! | [`CietSwitch`] | read/write (`CurrentRead \| CurrentWrite`) | `Boolean` | an on/off control a client may drive |
//!
//! Everything downstream is derived from these enums — the server's address
//! space, its read and write callbacks, the GUI's "how to connect" node table,
//! and the demo client's browse list. Adding a variable means adding one enum
//! variant and filling in its `match` arms; the compiler then points at every
//! place that must be updated. That exhaustiveness is exactly why these are
//! enums rather than a table of trait objects (see the workspace Rust design
//! rules: no `Box<dyn Trait>` for dispatch).
//!
//! ## Node identifiers
//!
//! Nodes live in the namespace [`CIET_NAMESPACE_URI`] and use **string**
//! identifiers, so a client can address them by name without browsing:
//!
//! ```text
//! ns=<index>;s=CIET.Heater.PowerKw
//! ns=<index>;s=CIET.Temperature.BT12HeaterOutletDegC
//! ```
//!
//! The namespace index is assigned by the server at start-up (typically `2`,
//! after the OPC-UA core namespace `0` and the server's own namespace `1`), and
//! the running index is what the GUI displays. Do not hard-code `2` in a client
//! — read it from the server, or use [`browse_name`](CietSignal::browse_name)
//! and let the client resolve it.
//!
//! ## Safety envelope
//!
//! Every writable variable carries a documented [`valid_range`](CietControl::valid_range)
//! and writes are clamped to it by the state setters in
//! [`super::state`]. A client cannot push the solver outside its stable
//! envelope, no matter what it sends.
//!
//! ## Scope
//!
//! `RESPONSIBLE_USE.md` applies: this interface exists so an **offline**
//! educational simulator can be driven by standard OPC-UA tooling. It must
//! never be connected to live operational systems, plant systems,
//! safety-critical infrastructure, or institutional production systems.

use super::state::{
    CietState, CTAH_PUMP_PRESSURE_LIMIT_PASCALS, HEATER_POWER_LIMIT_KILOWATTS, MAX_TIMESTEP_SECONDS,
};

/// Namespace URI for every CIET v2 variable.
pub const CIET_NAMESPACE_URI: &str = "urn:outram-park:ciet-educational-simulator-v2";

/// Default OPC-UA TCP port. 4840 is the IANA-registered port for `opcua-tcp`.
pub const DEFAULT_OPCUA_PORT: u16 = 4840;

/// Endpoint path appended to the server URL, giving
/// `opc.tcp://<host>:<port>/ciet`.
pub const ENDPOINT_PATH: &str = "/ciet";

/// Browse name of the folder holding the writable controls.
pub const CONTROLS_FOLDER_NAME: &str = "Controls";

/// Browse name of the folder holding the read-only outputs.
pub const OUTPUTS_FOLDER_NAME: &str = "Outputs";

/// A read-only quantity the simulator publishes.
///
/// These are the values a client would trend, log, or display: the
/// instrumented temperatures and flowrates a real CIET operator would watch,
/// plus the controller outputs and timing diagnostics. All are `f64` in the
/// units named by [`unit`](Self::unit).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CietSignal {
    // ---- headline plant values ----
    /// Heater electrical power actually applied this timestep, kW. Differs from
    /// the requested value when the over-temperature killswitch has tripped.
    HeaterPowerKw,
    /// BT-11, heater inlet bulk temperature, degC.
    Bt11HeaterInletDegC,
    /// BT-12, heater outlet bulk temperature, degC.
    Bt12HeaterOutletDegC,
    /// BT-43, CTAH inlet bulk temperature, degC.
    Bt43CtahInletDegC,
    /// BT-41, CTAH outlet bulk temperature, degC.
    Bt41CtahOutletDegC,
    /// BT-60, DHX tube inlet bulk temperature, degC.
    Bt60DhxTubeInletDegC,
    /// BT-21, DHX tube outlet bulk temperature, degC.
    Bt21DhxTubeOutletDegC,
    /// BT-21, DHX shell inlet bulk temperature, degC.
    Bt21DhxShellInletDegC,
    /// BT-27, DHX shell outlet bulk temperature, degC.
    Bt27DhxShellOutletDegC,
    /// BT-65, TCHX inlet bulk temperature, degC.
    Bt65TchxInletDegC,
    /// BT-66, TCHX outlet bulk temperature, degC.
    Bt66TchxOutletDegC,

    // ---- flowrates ----
    /// FM-40, CTAH-branch mass flowrate, kg/s. Signed; negative is reverse flow.
    Fm40CtahBranchKgPerS,
    /// FM-20, DHX-branch mass flowrate, kg/s.
    Fm20DhxBranchKgPerS,
    /// FM-60, DRACS-loop mass flowrate magnitude, kg/s.
    Fm60DracsKgPerS,

    // ---- controller outputs ----
    /// CTAH air-side heat transfer coefficient commanded by its PID controller,
    /// W/(m^2 K).
    CtahHtcWattPerM2K,
    /// TCHX air-side heat transfer coefficient commanded by its PID controller,
    /// W/(m^2 K).
    TchxHtcWattPerM2K,

    // ---- mixing nodes ----
    /// Top mixing node (branches 5a/5b/4) temperature, degC.
    TopMixingNodeDegC,
    /// Bottom mixing node (branches 17a/17b/18) temperature, degC.
    BottomMixingNodeDegC,

    // ---- timing diagnostics ----
    /// Simulated time elapsed, s.
    SimulationTimeSeconds,
    /// Wall-clock time elapsed, s. Compare with the simulated time to see
    /// whether the simulation is keeping up with real time.
    ElapsedTimeSeconds,
    /// Wall-clock cost of the last timestep, ms.
    CalcTimeMs,
}

impl CietSignal {
    /// Every signal, in the order the address space and UI table present them.
    pub const ALL: &'static [CietSignal] = &[
        Self::HeaterPowerKw,
        Self::Bt11HeaterInletDegC,
        Self::Bt12HeaterOutletDegC,
        Self::Bt43CtahInletDegC,
        Self::Bt41CtahOutletDegC,
        Self::Bt60DhxTubeInletDegC,
        Self::Bt21DhxTubeOutletDegC,
        Self::Bt21DhxShellInletDegC,
        Self::Bt27DhxShellOutletDegC,
        Self::Bt65TchxInletDegC,
        Self::Bt66TchxOutletDegC,
        Self::Fm40CtahBranchKgPerS,
        Self::Fm20DhxBranchKgPerS,
        Self::Fm60DracsKgPerS,
        Self::CtahHtcWattPerM2K,
        Self::TchxHtcWattPerM2K,
        Self::TopMixingNodeDegC,
        Self::BottomMixingNodeDegC,
        Self::SimulationTimeSeconds,
        Self::ElapsedTimeSeconds,
        Self::CalcTimeMs,
    ];

    /// The string part of this signal's `NodeId`, e.g.
    /// `"CIET.Temperature.BT12HeaterOutletDegC"`.
    ///
    /// Stable: treat these as public API, because client configurations and
    /// saved trend definitions reference them by name.
    pub fn node_identifier(&self) -> &'static str {
        match self {
            Self::HeaterPowerKw => "CIET.Heater.PowerKw",
            Self::Bt11HeaterInletDegC => "CIET.Temperature.BT11HeaterInletDegC",
            Self::Bt12HeaterOutletDegC => "CIET.Temperature.BT12HeaterOutletDegC",
            Self::Bt43CtahInletDegC => "CIET.Temperature.BT43CtahInletDegC",
            Self::Bt41CtahOutletDegC => "CIET.Temperature.BT41CtahOutletDegC",
            Self::Bt60DhxTubeInletDegC => "CIET.Temperature.BT60DhxTubeInletDegC",
            Self::Bt21DhxTubeOutletDegC => "CIET.Temperature.BT21DhxTubeOutletDegC",
            Self::Bt21DhxShellInletDegC => "CIET.Temperature.BT21DhxShellInletDegC",
            Self::Bt27DhxShellOutletDegC => "CIET.Temperature.BT27DhxShellOutletDegC",
            Self::Bt65TchxInletDegC => "CIET.Temperature.BT65TchxInletDegC",
            Self::Bt66TchxOutletDegC => "CIET.Temperature.BT66TchxOutletDegC",
            Self::Fm40CtahBranchKgPerS => "CIET.Flow.FM40CtahBranchKgPerS",
            Self::Fm20DhxBranchKgPerS => "CIET.Flow.FM20DhxBranchKgPerS",
            Self::Fm60DracsKgPerS => "CIET.Flow.FM60DracsKgPerS",
            Self::CtahHtcWattPerM2K => "CIET.Controller.CtahHtcWattPerM2K",
            Self::TchxHtcWattPerM2K => "CIET.Controller.TchxHtcWattPerM2K",
            Self::TopMixingNodeDegC => "CIET.Temperature.TopMixingNodeDegC",
            Self::BottomMixingNodeDegC => "CIET.Temperature.BottomMixingNodeDegC",
            Self::SimulationTimeSeconds => "CIET.Time.SimulationSeconds",
            Self::ElapsedTimeSeconds => "CIET.Time.ElapsedSeconds",
            Self::CalcTimeMs => "CIET.Time.CalcMs",
        }
    }

    /// Short OPC-UA browse name, e.g. `"BT12HeaterOutletDegC"`.
    pub fn browse_name(&self) -> &'static str {
        // The identifier's last dot-separated segment is the browse name.
        let id = self.node_identifier();
        match id.rfind('.') {
            Some(dot) => &id[dot + 1..],
            None => id,
        }
    }

    /// Human-facing label for the GUI and client tables.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::HeaterPowerKw => "Heater power",
            Self::Bt11HeaterInletDegC => "BT-11 heater inlet",
            Self::Bt12HeaterOutletDegC => "BT-12 heater outlet",
            Self::Bt43CtahInletDegC => "BT-43 CTAH inlet",
            Self::Bt41CtahOutletDegC => "BT-41 CTAH outlet",
            Self::Bt60DhxTubeInletDegC => "BT-60 DHX tube inlet",
            Self::Bt21DhxTubeOutletDegC => "BT-21 DHX tube outlet",
            Self::Bt21DhxShellInletDegC => "BT-21 DHX shell inlet",
            Self::Bt27DhxShellOutletDegC => "BT-27 DHX shell outlet",
            Self::Bt65TchxInletDegC => "BT-65 TCHX inlet",
            Self::Bt66TchxOutletDegC => "BT-66 TCHX outlet",
            Self::Fm40CtahBranchKgPerS => "FM-40 CTAH branch flow",
            Self::Fm20DhxBranchKgPerS => "FM-20 DHX branch flow",
            Self::Fm60DracsKgPerS => "FM-60 DRACS loop flow",
            Self::CtahHtcWattPerM2K => "CTAH air-side HTC",
            Self::TchxHtcWattPerM2K => "TCHX air-side HTC",
            Self::TopMixingNodeDegC => "Top mixing node",
            Self::BottomMixingNodeDegC => "Bottom mixing node",
            Self::SimulationTimeSeconds => "Simulation time",
            Self::ElapsedTimeSeconds => "Wall-clock time",
            Self::CalcTimeMs => "Timestep cost",
        }
    }

    /// Engineering unit, as a short display string.
    pub fn unit(&self) -> &'static str {
        match self {
            Self::HeaterPowerKw => "kW",
            Self::Bt11HeaterInletDegC
            | Self::Bt12HeaterOutletDegC
            | Self::Bt43CtahInletDegC
            | Self::Bt41CtahOutletDegC
            | Self::Bt60DhxTubeInletDegC
            | Self::Bt21DhxTubeOutletDegC
            | Self::Bt21DhxShellInletDegC
            | Self::Bt27DhxShellOutletDegC
            | Self::Bt65TchxInletDegC
            | Self::Bt66TchxOutletDegC
            | Self::TopMixingNodeDegC
            | Self::BottomMixingNodeDegC => "degC",
            Self::Fm40CtahBranchKgPerS | Self::Fm20DhxBranchKgPerS | Self::Fm60DracsKgPerS => {
                "kg/s"
            }
            Self::CtahHtcWattPerM2K | Self::TchxHtcWattPerM2K => "W/(m^2 K)",
            Self::SimulationTimeSeconds | Self::ElapsedTimeSeconds => "s",
            Self::CalcTimeMs => "ms",
        }
    }

    /// Read this signal's current value out of a state snapshot.
    ///
    /// This is the only place the OPC-UA read callbacks touch plant state, so
    /// the mapping from node to field exists exactly once.
    pub fn read(&self, state: &CietState) -> f64 {
        match self {
            Self::HeaterPowerKw => state.heater_power_kilowatts,
            Self::Bt11HeaterInletDegC => state.bt_11_heater_inlet_deg_c,
            Self::Bt12HeaterOutletDegC => state.bt_12_heater_outlet_deg_c,
            Self::Bt43CtahInletDegC => state.bt_43_ctah_inlet_deg_c,
            Self::Bt41CtahOutletDegC => state.bt_41_ctah_outlet_deg_c,
            Self::Bt60DhxTubeInletDegC => state.bt_60_dhx_tube_inlet_deg_c,
            Self::Bt21DhxTubeOutletDegC => state.bt_21_dhx_tube_outlet_deg_c,
            Self::Bt21DhxShellInletDegC => state.bt_21_dhx_shell_inlet_deg_c,
            Self::Bt27DhxShellOutletDegC => state.bt_27_dhx_shell_outlet_deg_c,
            Self::Bt65TchxInletDegC => state.bt_65_tchx_inlet_deg_c,
            Self::Bt66TchxOutletDegC => state.bt_66_tchx_outlet_deg_c,
            Self::Fm40CtahBranchKgPerS => state.fm40_ctah_branch_kg_per_s,
            Self::Fm20DhxBranchKgPerS => state.fm20_dhx_branch_kg_per_s as f64,
            Self::Fm60DracsKgPerS => state.fm_60_dracs_kg_per_s,
            Self::CtahHtcWattPerM2K => state.ctah_htc_watt_per_m2_kelvin,
            Self::TchxHtcWattPerM2K => state.tchx_htc_watt_per_m2_kelvin,
            Self::TopMixingNodeDegC => state.top_mixing_node_5a_5b_4_temp_degc as f64,
            Self::BottomMixingNodeDegC => state.bottom_mixing_node_17a_17b_18_temp_degc as f64,
            Self::SimulationTimeSeconds => state.simulation_time_seconds,
            Self::ElapsedTimeSeconds => state.elapsed_time_seconds,
            Self::CalcTimeMs => state.calc_time_ms,
        }
    }
}

/// A continuous control a client may write.
///
/// Writes are **clamped** to [`valid_range`](Self::valid_range) — an
/// out-of-range write is honoured at the nearest limit rather than rejected, so
/// a client that sends 1000 kW gets the 15 kW ceiling and a clear read-back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CietControl {
    /// Heater electrical power set point, kW. Ignored while advanced heater
    /// control is switched on (that driver overwrites it every timestep).
    HeaterPowerKw,
    /// CTAH pump pressure rise, Pa. The forced-circulation driver; negative
    /// reverses the flow direction.
    CtahPumpPressurePascals,
    /// Set point for BT-41, the CTAH outlet temperature, degC.
    Bt41CtahOutletSetPointDegC,
    /// Set point for BT-66, the TCHX outlet temperature, degC.
    Bt66TchxOutletSetPointDegC,
    /// Steady (mean) heater power used by advanced heater control, kW.
    HeaterSteadyStatePowerKw,
    /// Peak amplitude of the frequency-response perturbation, kW.
    FrequencyResponseAmplitudeKw,
    /// Angular frequency of the frequency-response perturbation, rad/s.
    FrequencyResponseAngularVelocityRadPerS,
    /// Requested solver timestep, s. Only honoured in slow-motion mode, and
    /// clamped to the Courant stability limit either way.
    TimestepSeconds,
}

impl CietControl {
    /// Every control, in address-space and UI order.
    pub const ALL: &'static [CietControl] = &[
        Self::HeaterPowerKw,
        Self::CtahPumpPressurePascals,
        Self::Bt41CtahOutletSetPointDegC,
        Self::Bt66TchxOutletSetPointDegC,
        Self::HeaterSteadyStatePowerKw,
        Self::FrequencyResponseAmplitudeKw,
        Self::FrequencyResponseAngularVelocityRadPerS,
        Self::TimestepSeconds,
    ];

    /// Position of this control in [`Self::ALL`].
    ///
    /// Used to index the fixed-size pending-request arrays in
    /// [`super::user_controls::CietUserControls`] without allocating or
    /// hashing. Guaranteed to be `< CietControl::ALL.len()`.
    pub fn index(&self) -> usize {
        Self::ALL
            .iter()
            .position(|candidate| candidate == self)
            .expect("every CietControl variant must appear in CietControl::ALL")
    }

    /// The string part of this control's `NodeId`.
    pub fn node_identifier(&self) -> &'static str {
        match self {
            Self::HeaterPowerKw => "CIET.Control.HeaterPowerKw",
            Self::CtahPumpPressurePascals => "CIET.Control.CtahPumpPressurePascals",
            Self::Bt41CtahOutletSetPointDegC => "CIET.Control.Bt41CtahOutletSetPointDegC",
            Self::Bt66TchxOutletSetPointDegC => "CIET.Control.Bt66TchxOutletSetPointDegC",
            Self::HeaterSteadyStatePowerKw => "CIET.Control.HeaterSteadyStatePowerKw",
            Self::FrequencyResponseAmplitudeKw => "CIET.Control.FrequencyResponseAmplitudeKw",
            Self::FrequencyResponseAngularVelocityRadPerS => {
                "CIET.Control.FrequencyResponseAngularVelocityRadPerS"
            }
            Self::TimestepSeconds => "CIET.Control.TimestepSeconds",
        }
    }

    /// Short OPC-UA browse name.
    pub fn browse_name(&self) -> &'static str {
        let id = self.node_identifier();
        match id.rfind('.') {
            Some(dot) => &id[dot + 1..],
            None => id,
        }
    }

    /// Human-facing label for the GUI and client tables.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::HeaterPowerKw => "Heater power set point",
            Self::CtahPumpPressurePascals => "CTAH pump pressure",
            Self::Bt41CtahOutletSetPointDegC => "CTAH outlet set point (BT-41)",
            Self::Bt66TchxOutletSetPointDegC => "TCHX outlet set point (BT-66)",
            Self::HeaterSteadyStatePowerKw => "Heater steady-state power",
            Self::FrequencyResponseAmplitudeKw => "Frequency-response amplitude",
            Self::FrequencyResponseAngularVelocityRadPerS => "Frequency-response angular velocity",
            Self::TimestepSeconds => "Solver timestep",
        }
    }

    /// Engineering unit, as a short display string.
    pub fn unit(&self) -> &'static str {
        match self {
            Self::HeaterPowerKw | Self::HeaterSteadyStatePowerKw => "kW",
            Self::FrequencyResponseAmplitudeKw => "kW",
            Self::CtahPumpPressurePascals => "Pa",
            Self::Bt41CtahOutletSetPointDegC | Self::Bt66TchxOutletSetPointDegC => "degC",
            Self::FrequencyResponseAngularVelocityRadPerS => "rad/s",
            Self::TimestepSeconds => "s",
        }
    }

    /// Inclusive `(minimum, maximum)` this control accepts.
    ///
    /// Writes outside the range are clamped, not rejected. The temperature set
    /// point ranges span the loop's practical operating window: Therminol VP-1
    /// / Dowtherm A is liquid well below 21 degC, but CIET is an atmospheric
    /// loop and the simulator's killswitch trips at 150 degC, so set points
    /// above 120 degC are not useful.
    pub fn valid_range(&self) -> (f64, f64) {
        match self {
            Self::HeaterPowerKw | Self::HeaterSteadyStatePowerKw => {
                (0.0, HEATER_POWER_LIMIT_KILOWATTS)
            }
            Self::CtahPumpPressurePascals => (
                -CTAH_PUMP_PRESSURE_LIMIT_PASCALS,
                CTAH_PUMP_PRESSURE_LIMIT_PASCALS,
            ),
            Self::Bt41CtahOutletSetPointDegC | Self::Bt66TchxOutletSetPointDegC => (15.0, 120.0),
            Self::FrequencyResponseAmplitudeKw => (0.0, 4.0),
            Self::FrequencyResponseAngularVelocityRadPerS => (0.0, 10.0),
            Self::TimestepSeconds => (0.001, MAX_TIMESTEP_SECONDS),
        }
    }

    /// Read this control's current value out of a state snapshot, so a client
    /// can see what the set point actually is after clamping.
    pub fn read(&self, state: &CietState) -> f64 {
        match self {
            Self::HeaterPowerKw => state.heater_power_kilowatts,
            Self::CtahPumpPressurePascals => state.ctah_pump_pressure_pascals as f64,
            Self::Bt41CtahOutletSetPointDegC => state.bt_41_ctah_outlet_set_pt_deg_c,
            Self::Bt66TchxOutletSetPointDegC => state.bt_66_tchx_outlet_set_pt_deg_c,
            Self::HeaterSteadyStatePowerKw => state.heater_control.steady_state_power_kw,
            Self::FrequencyResponseAmplitudeKw => state.heater_control.total_amplitude_kw,
            Self::FrequencyResponseAngularVelocityRadPerS => {
                state.heater_control.angular_velocity_rad_per_s
            }
            Self::TimestepSeconds => state.timestep_seconds as f64,
        }
    }

    /// Apply a client's write to plant state, clamped to
    /// [`valid_range`](Self::valid_range).
    ///
    /// Returns the value actually stored, which a caller may log or feed back.
    pub fn write(&self, state: &mut CietState, value: f64) -> f64 {
        let (min, max) = self.valid_range();
        let clamped = if value.is_nan() {
            // A NaN write would poison the solver; treat it as "no change".
            return self.read(state);
        } else {
            value.clamp(min, max)
        };

        match self {
            Self::HeaterPowerKw => state.set_heater_power_kilowatts(clamped),
            Self::CtahPumpPressurePascals => state.set_ctah_pump_pressure_pascals(clamped),
            Self::Bt41CtahOutletSetPointDegC => state.bt_41_ctah_outlet_set_pt_deg_c = clamped,
            Self::Bt66TchxOutletSetPointDegC => state.bt_66_tchx_outlet_set_pt_deg_c = clamped,
            Self::HeaterSteadyStatePowerKw => state.heater_control.steady_state_power_kw = clamped,
            Self::FrequencyResponseAmplitudeKw => state.heater_control.total_amplitude_kw = clamped,
            Self::FrequencyResponseAngularVelocityRadPerS => {
                state.heater_control.angular_velocity_rad_per_s = clamped
            }
            Self::TimestepSeconds => state.set_timestep_seconds(clamped),
        }

        self.read(state)
    }
}

/// A boolean control a client may write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CietSwitch {
    /// Master switch for advanced heater control. While on, the heater power is
    /// driven by the steady-state + frequency-response settings each timestep
    /// and direct writes to the heater power set point are overwritten.
    AdvancedHeaterControlOn,
    /// Add the sinusoidal perturbation to the steady heater power.
    FrequencyResponseOn,
    /// Block flow through the CTAH branch, as if a valve were shut.
    CtahBranchBlocked,
    /// Block flow through the DHX branch.
    DhxBranchBlocked,
    /// Run faster than real time where the machine allows it.
    FastForwardOn,
    /// Run slower than real time, honouring the requested timestep.
    SlowMotionOn,
    /// Use the coarse 8-node heater mesh instead of the fine 15-node mesh.
    /// Cheaper per timestep; useful on slow hardware and on Termux.
    CoarseHeaterMesh,
}

impl CietSwitch {
    /// Every switch, in address-space and UI order.
    pub const ALL: &'static [CietSwitch] = &[
        Self::AdvancedHeaterControlOn,
        Self::FrequencyResponseOn,
        Self::CtahBranchBlocked,
        Self::DhxBranchBlocked,
        Self::FastForwardOn,
        Self::SlowMotionOn,
        Self::CoarseHeaterMesh,
    ];

    /// Position of this switch in [`Self::ALL`].
    ///
    /// Used to index the fixed-size pending-request arrays in
    /// [`super::user_controls::CietUserControls`]. Guaranteed to be
    /// `< CietSwitch::ALL.len()`.
    pub fn index(&self) -> usize {
        Self::ALL
            .iter()
            .position(|candidate| candidate == self)
            .expect("every CietSwitch variant must appear in CietSwitch::ALL")
    }

    /// The string part of this switch's `NodeId`.
    pub fn node_identifier(&self) -> &'static str {
        match self {
            Self::AdvancedHeaterControlOn => "CIET.Control.AdvancedHeaterControlOn",
            Self::FrequencyResponseOn => "CIET.Control.FrequencyResponseOn",
            Self::CtahBranchBlocked => "CIET.Control.CtahBranchBlocked",
            Self::DhxBranchBlocked => "CIET.Control.DhxBranchBlocked",
            Self::FastForwardOn => "CIET.Control.FastForwardOn",
            Self::SlowMotionOn => "CIET.Control.SlowMotionOn",
            Self::CoarseHeaterMesh => "CIET.Control.CoarseHeaterMesh",
        }
    }

    /// Short OPC-UA browse name.
    pub fn browse_name(&self) -> &'static str {
        let id = self.node_identifier();
        match id.rfind('.') {
            Some(dot) => &id[dot + 1..],
            None => id,
        }
    }

    /// Human-facing label for the GUI and client tables.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::AdvancedHeaterControlOn => "Advanced heater control",
            Self::FrequencyResponseOn => "Frequency response",
            Self::CtahBranchBlocked => "CTAH branch blocked",
            Self::DhxBranchBlocked => "DHX branch blocked",
            Self::FastForwardOn => "Fast forward",
            Self::SlowMotionOn => "Slow motion",
            Self::CoarseHeaterMesh => "Coarse heater mesh (8 nodes)",
        }
    }

    /// Read this switch's current value out of a state snapshot.
    pub fn read(&self, state: &CietState) -> bool {
        use super::state::HeaterType;
        match self {
            Self::AdvancedHeaterControlOn => {
                state.heater_control.advanced_heater_control_switched_on
            }
            Self::FrequencyResponseOn => state.heater_control.frequency_response_switched_on,
            Self::CtahBranchBlocked => state.is_ctah_branch_blocked,
            Self::DhxBranchBlocked => state.is_dhx_branch_blocked,
            Self::FastForwardOn => state.fast_forward_settings_turned_on,
            Self::SlowMotionOn => state.slow_motion_settings_turned_on,
            Self::CoarseHeaterMesh => {
                state.current_heater_type == HeaterType::InsulatedHeaterV1Coarse8Mesh
            }
        }
    }

    /// Apply a client's write to plant state.
    pub fn write(&self, state: &mut CietState, value: bool) {
        use super::state::HeaterType;
        match self {
            Self::AdvancedHeaterControlOn => {
                state.heater_control.advanced_heater_control_switched_on = value
            }
            Self::FrequencyResponseOn => {
                state.heater_control.frequency_response_switched_on = value
            }
            Self::CtahBranchBlocked => state.is_ctah_branch_blocked = value,
            Self::DhxBranchBlocked => state.is_dhx_branch_blocked = value,
            Self::FastForwardOn => state.fast_forward_settings_turned_on = value,
            Self::SlowMotionOn => state.slow_motion_settings_turned_on = value,
            Self::CoarseHeaterMesh => {
                state.current_heater_type = if value {
                    HeaterType::InsulatedHeaterV1Coarse8Mesh
                } else {
                    HeaterType::InsulatedHeaterV1Fine15Mesh
                }
            }
        }
    }
}

/// Total number of OPC-UA variables the simulator publishes.
///
/// Useful for the GUI's "N nodes served" line and for a sanity check in tests.
pub fn total_node_count() -> usize {
    CietSignal::ALL.len() + CietControl::ALL.len() + CietSwitch::ALL.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Every node identifier must be unique, or the address space would silently
    /// collapse two variables into one and a client would read the wrong value.
    ///
    /// **Methodology.** Collect the `node_identifier()` of every signal, control
    /// and switch into a set and compare the set size with the enumerated count.
    /// Pass criterion: exact equality.
    ///
    /// **Results (2026-07-28).** 36 variables enumerated (21 signals,
    /// 8 controls, 7 switches), 36 distinct identifiers — no collisions.
    #[test]
    fn node_identifiers_are_unique() {
        let mut seen: HashSet<&str> = HashSet::new();
        for s in CietSignal::ALL {
            assert!(seen.insert(s.node_identifier()), "duplicate: {s:?}");
        }
        for c in CietControl::ALL {
            assert!(seen.insert(c.node_identifier()), "duplicate: {c:?}");
        }
        for s in CietSwitch::ALL {
            assert!(seen.insert(s.node_identifier()), "duplicate: {s:?}");
        }
        assert_eq!(seen.len(), total_node_count());
    }

    /// Tolerance for a control read-back, accounting for `f32`-backed storage.
    ///
    /// Two controls are stored in `f32` fields inherited unchanged from v1's
    /// `CIETState` (`timestep_seconds` and `ctah_pump_pressure_pascals`), so a
    /// value written as `f64` and read back through `f32` carries a relative
    /// error of up to `f32::EPSILON` (about 1.2e-7). A timestep of 0.1 s, for
    /// instance, reads back as 0.10000000149011612 — physically irrelevant at
    /// 1.5e-9 s, but far larger than an `f64` round-off tolerance would allow.
    ///
    /// Returns an absolute tolerance scaled to the magnitude being compared.
    fn read_back_tolerance(control: &CietControl, magnitude: f64) -> f64 {
        let stored_as_f32 = matches!(
            control,
            CietControl::TimestepSeconds | CietControl::CtahPumpPressurePascals
        );
        if stored_as_f32 {
            // Relative f32 error, with a floor so a comparison against zero
            // does not demand exactness it cannot have.
            (magnitude.abs() * f32::EPSILON as f64).max(1e-9)
        } else {
            1e-9
        }
    }

    /// Verifies that a write outside a control's documented envelope is clamped
    /// to the envelope rather than stored raw, for every control.
    ///
    /// **Methodology.** For each [`CietControl`], write `min - 1e6` and
    /// `max + 1e6`, then read back. Pass criterion: the read-back equals the
    /// documented `min`/`max` to within [`read_back_tolerance`] — 1e-9 for
    /// `f64`-backed controls, and a relative `f32::EPSILON` for the two
    /// `f32`-backed ones.
    ///
    /// **Results (2026-07-28).** All 8 controls clamp at both ends. Six are
    /// exact. The two `f32`-backed controls behave as predicted: the CTAH pump
    /// pressure limit (17000 Pa) happens to be exactly representable in `f32`
    /// and so is also exact, while the timestep ceiling of 0.1 s reads back as
    /// 0.10000000149011612 — a 1.5e-9 s discrepancy, which is why this test
    /// needs an `f32`-aware tolerance rather than an `f64` one. This was found
    /// by the test failing on its original 1e-9 criterion, not by inspection.
    #[test]
    fn every_control_clamps_out_of_range_writes() {
        for control in CietControl::ALL {
            let (min, max) = control.valid_range();
            let mut state = CietState::default();

            let low = control.write(&mut state, min - 1.0e6);
            assert!(
                (low - min).abs() <= read_back_tolerance(control, min),
                "{control:?}: low write gave {low}, expected {min}"
            );

            let high = control.write(&mut state, max + 1.0e6);
            assert!(
                (high - max).abs() <= read_back_tolerance(control, max),
                "{control:?}: high write gave {high}, expected {max}"
            );
        }
    }

    /// A NaN write must leave the plant state untouched — a NaN set point would
    /// propagate into the solver and destroy the whole simulation.
    ///
    /// **Methodology.** Write a mid-range value, then write `f64::NAN`, then
    /// read back. Pass criterion: the read-back is the mid-range value, and is
    /// finite.
    ///
    /// **Results (2026-07-28).** All 8 controls retained their prior value and
    /// stayed finite after a NaN write.
    #[test]
    fn nan_writes_are_ignored() {
        for control in CietControl::ALL {
            let (min, max) = control.valid_range();
            let midpoint = 0.5 * (min + max);
            let mut state = CietState::default();

            control.write(&mut state, midpoint);
            let after_nan = control.write(&mut state, f64::NAN);

            assert!(after_nan.is_finite(), "{control:?} became non-finite");
            assert!(
                (after_nan - midpoint).abs() < 1e-6,
                "{control:?}: NaN write changed value to {after_nan}"
            );
        }
    }

    /// Every switch must round-trip `true` and `false`.
    ///
    /// **Methodology.** Write `true`, read back, write `false`, read back, for
    /// each switch. Pass criterion: read-back matches what was written.
    ///
    /// **Results (2026-07-28).** All 7 switches round-tripped, including
    /// `CoarseHeaterMesh`, which maps a boolean onto the two-variant
    /// `HeaterType` enum.
    #[test]
    fn every_switch_round_trips() {
        for switch in CietSwitch::ALL {
            let mut state = CietState::default();
            switch.write(&mut state, true);
            assert!(switch.read(&state), "{switch:?} did not latch true");
            switch.write(&mut state, false);
            assert!(!switch.read(&state), "{switch:?} did not latch false");
        }
    }

    /// Browse names must be non-empty and free of dots, since OPC-UA browse
    /// names are single path segments.
    ///
    /// **Methodology.** Check each derived browse name. Pass criterion:
    /// non-empty, contains no `.`.
    ///
    /// **Results (2026-07-28).** All 36 browse names valid.
    #[test]
    fn browse_names_are_single_segments() {
        for s in CietSignal::ALL {
            let name = s.browse_name();
            assert!(!name.is_empty() && !name.contains('.'), "bad: {name}");
        }
        for c in CietControl::ALL {
            let name = c.browse_name();
            assert!(!name.is_empty() && !name.contains('.'), "bad: {name}");
        }
        for s in CietSwitch::ALL {
            let name = s.browse_name();
            assert!(!name.is_empty() && !name.contains('.'), "bad: {name}");
        }
    }
}
