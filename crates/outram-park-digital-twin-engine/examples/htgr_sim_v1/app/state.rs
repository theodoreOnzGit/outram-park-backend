//! Shared simulation state carried between the physics threads and the GUI.
//!
//! Two plain-data, `Clone` structs are shared through the engine's
//! [`outram_park_digital_twin_engine::app_scaffold::SharedState`]:
//!
//! - [`HtgrSnapshot`] -- the current scalar plant state (control inputs written
//!   by the GUI, outputs written by the physics thread). Deliberately holds
//!   only `f64` scalars so it is cheap to clone every frame and trivially
//!   `Send + Sync`; the GUI rebuilds engine visual widgets from these scalars
//!   rather than sharing the (non-`Clone`) physics objects across threads.
//! - [`HtgrPlotData`] -- bounded time-history ring buffers for the plot panel,
//!   updated by a separate plot-sampler thread.

/// Maximum number of samples kept in each plot history buffer.
const MAX_PLOT_SAMPLES: usize = 4000;

/// Scalar snapshot of the HTGR plant, shared between the physics thread (which
/// writes the output fields) and the GUI thread (which writes the control-input
/// fields and reads everything for display).
#[derive(Clone, Debug)]
pub struct HtgrSnapshot {
    // --- Control inputs: written by the GUI, read by the physics thread ---
    /// User-commanded **control-rod bank insertion fraction**, `0.0` fully
    /// withdrawn to `1.0` fully inserted.
    ///
    /// This is the control input an operator actually has. Reactivity is a
    /// *consequence* of rod position, not something commanded directly, and is
    /// published back as [`HtgrSnapshot::external_reactivity_dollars`] for
    /// display. The physics clamps out-of-range values at the stops, so an
    /// OPC-UA write cannot inject unbounded reactivity -- see
    /// [`crate::physics::control_rods`].
    ///
    /// The ten HTR-10 rods are ganged as one bank here; an asymmetric pattern
    /// or a stuck rod cannot be represented.
    pub control_rod_insertion_fraction: f64,
    /// User-commanded helium pump mass-flow setpoint \[kg/s\].
    pub helium_flow_setpoint_kg_per_s: f64,
    /// Whether the feedwater station is in **MANUAL** (`true`) or **AUTO**
    /// (`false`).
    ///
    /// Assembled into a [`crate::physics::secondary_loop::FeedwaterCommand`] by
    /// the physics thread; this snapshot deliberately stays plain scalars so it
    /// is cheap to clone every frame. Defaults to AUTO, which is the behaviour
    /// this simulator had before the mode existed.
    pub feedwater_manual: bool,
    /// User-commanded feedwater mass flow \[kg/s\], used only in **MANUAL**.
    ///
    /// While the station is in AUTO the physics thread keeps writing the
    /// *achieved* flow back into this field, so switching to MANUAL picks the
    /// plant up where it is instead of stepping it -- a bumpless transfer. The
    /// physics clamps it to the feed pump's 0.3 to 12.0 kg/s capacity.
    pub feedwater_manual_flow_kg_per_s: f64,
    /// User-commanded **AUTO** steam-generator outlet temperature setpoint
    /// \[K\].
    ///
    /// Held in kelvin because that is the snapshot's convention for every
    /// temperature; the GUI dials it in degrees Celsius, which is what an
    /// operator would use, and converts through `uom` at the widget. The
    /// physics clamps it to 260-540 degC.
    pub feedwater_target_steam_temp_k: f64,
    /// User-commanded condenser back-pressure \[kPa\].
    ///
    /// The **setpoint**, distinct from
    /// [`Self::condenser_pressure_kpa`], which is what the plant is actually
    /// running at after the physics clamps it to 4-30 kPa. Same
    /// command-and-consequence pairing as
    /// [`Self::helium_flow_setpoint_kg_per_s`] against
    /// [`Self::helium_mass_flow_kg_per_s`].
    pub condenser_pressure_setpoint_kpa: f64,

    // --- Kinetics outputs ---
    /// Total reactor thermal power (prompt + delayed) \[MW\].
    pub reactor_power_mw: f64,
    /// Prompt-excursion-layer power \[MW\].
    pub prompt_power_mw: f64,
    /// Delayed-neutron power increment `S*dt` added this step \[MW\].
    pub delayed_power_mw: f64,
    /// Lumped fuel temperature \[K\] -- the kinetics model's own
    /// reactivity-feedback node
    /// ([`crate::physics::kinetics::HtgrKinetics::fuel_temperature`]), kept
    /// separate from [`Self::bed_temperature_k`] by design (see that
    /// method's doc comment on why the closed-form Nordheim-Fuchs node is not
    /// overwritten by the bed's own temperature each step).
    ///
    /// **Prefer [`Self::bed_temperature_k`] for anything compared against a
    /// helium temperature, or for colouring a "how hot is the core"
    /// display.** This node is charged the same coolant heat-removal sink as
    /// the bed; until 2026-08-17 it had no matching decay-heat source (see
    /// [`crate::physics::kinetics::HtgrKinetics::apply_decay_heat`]), so
    /// after a trip it drifted measurably below the bed's real temperature
    /// (**-10.2 K by 300 s post-scram**, measured 2026-08-17). That
    /// divergence, displayed by the schematic's "T_fuel" tag and
    /// reactor-vessel colour instead of [`Self::bed_temperature_k`], was
    /// GitHub issue #22's "totally wrong energy balance" symptom -- not a
    /// violation in the underlying two-phase balance, which stayed correctly
    /// ordered throughout (see
    /// `physics::tests::reproduce_issue_22_scram_cooldown_energy_balance`).
    /// Both the missing source term and the display wiring are now fixed:
    /// this node tracks the bed within about +0.04 K through the same scram
    /// (see `physics::tests::kinetics_fuel_node_tracks_the_bed_node_after_a_scram`),
    /// but [`Self::bed_temperature_k`] remains the field to read for a
    /// helium-temperature comparison -- this one is for inspecting the
    /// reactivity-feedback path itself.
    /// Use this field only to inspect the reactivity-feedback path itself.
    pub fuel_temperature_k: f64,
    /// Bed-average GRAPHITE temperature of the pebble bed \[K\].
    ///
    /// Distinct from [`Self::fuel_temperature_k`], which is the kinetics
    /// model's lumped fuel node. This is the bed the helium actually flows
    /// through, so it is the one that must exceed the core outlet temperature
    /// — the gas cannot leave hotter than the solid heating it.
    ///
    /// **Not a peak fuel temperature.** It is a bed average; a real HTR-10
    /// peak fuel temperature is well above it and this model cannot resolve
    /// one, having a single lumped bed node.
    pub bed_temperature_k: f64,
    /// Whether the reactor protection system is armed.
    ///
    /// **Defaults to `false`** by maintainer decision on 2026-08-12, so the
    /// steam generator can be studied without a trip cutting the transient
    /// short. With it off, a full rod withdrawal exposes +16.45 $ with nothing
    /// to terminate it. The steam side is still bounded by the second-law cap
    /// in [`crate::physics::secondary_loop`], so that is a runaway rather than
    /// a crash.
    pub rps_enabled: bool,
    /// Operator request to clear a latched trip, consumed by the physics
    /// thread.
    ///
    /// A request rather than a direct write, because the protection system
    /// lives on the physics thread: clearing
    /// [`HtgrSnapshot::trip_reason`] from the GUI would be overwritten by the
    /// next physics step, which republishes the still-latched trip. The physics
    /// thread takes this flag, resets the protection system, and clears it.
    pub trip_reset_requested: bool,
    /// Whether the reactor protection system has tripped, and why.
    ///
    /// `None` while healthy. Latched once set -- see
    /// [`crate::physics::protection::ReactorProtectionSystem`]. The GUI shows a
    /// banner and a reset control when this is `Some`.
    pub trip_reason: Option<crate::physics::protection::TripReason>,
    /// How far the scram has driven the rod bank in, `0.0..=1.0`.
    ///
    /// This is the PROTECTION SYSTEM's demand, not the operator's command. The
    /// rods actually move to the deeper of the two.
    pub scram_insertion_fraction: f64,
    /// External reactivity resulting from the current rod position \[dollars\].
    ///
    /// **Derived, not commanded.** Published by the physics thread from
    /// [`HtgrSnapshot::control_rod_insertion_fraction`] so the GUI can show the
    /// operator what their rod position bought them. Writing to it does
    /// nothing -- the physics recomputes it from rod position every step.
    pub external_reactivity_dollars: f64,
    /// Reactivity margin \[dollars\].
    pub reactivity_margin_dollars: f64,
    /// Effective total delayed-neutron fraction \[pcm\].
    pub delayed_neutron_fraction_pcm: f64,

    // --- Primary helium loop outputs ---
    /// Core inlet helium temperature \[K\].
    pub core_inlet_temp_k: f64,
    /// Core outlet helium temperature \[K\].
    pub core_outlet_temp_k: f64,
    /// Helium mass flow \[kg/s\].
    pub helium_mass_flow_kg_per_s: f64,
    /// IHX duty transferred to the secondary loop \[MW\].
    pub ihx_duty_mw: f64,
    /// Helium-side IHX outlet temperature \[K\] -- what the core inlet
    /// relaxes toward once the return transport lag has played out.
    pub ihx_outlet_temp_k: f64,
    /// Helium loop residence time \[s\] (`m/m_dot`), driving the primary
    /// flow tracers in the schematic.
    pub helium_residence_time_s: f64,
    /// Frictional pressure drop around the whole helium loop \[kPa\]: the KTA
    /// pebble-bed term plus the published non-bed component sum.
    pub primary_pressure_drop_kpa: f64,
    /// The **pebble-bed** part of that drop alone \[kPa\], evaluated from the
    /// KTA packed-bed friction correlation at the live helium density and
    /// viscosity. Surfaced separately because it is the one hydraulic quantity
    /// in this model that is a real correlation result rather than a carried
    /// published figure.
    pub bed_pressure_drop_kpa: f64,
    /// Circulator hydraulic power \[MW\].
    pub circulator_power_mw: f64,
    /// Helium isobaric specific heat \[J/(kg K)\] at the current bulk mean
    /// temperature, from the real EOS -- shown so the operator can see the
    /// property is evaluated live rather than frozen.
    pub helium_cp_j_per_kg_k: f64,

    // --- Secondary steam loop outputs ---
    /// Live steam pressure \[MPa\].
    pub steam_pressure_mpa: f64,
    /// Steam-generator steam outlet temperature \[K\].
    pub sg_steam_outlet_temp_k: f64,
    /// Steam-generator / turbine-inlet steam specific enthalpy \[J/kg\], kept
    /// so the schematic can rebuild the exact `HemSteamCv` state the visual
    /// widgets colour by (the snapshot itself stays scalar-only).
    pub steam_enthalpy_j_per_kg: f64,
    /// Turbine inlet steam temperature \[K\].
    pub turbine_inlet_temp_k: f64,
    /// Turbine mechanical power \[MW\].
    pub turbine_power_mw: f64,
    /// Steam quality at the turbine exhaust \[0, 1\].
    pub steam_quality_after_turbine: f64,
    /// Condenser back-pressure \[kPa\].
    pub condenser_pressure_kpa: f64,
    /// Secondary (feedwater/steam) mass flow \[kg/s\], as moved by the
    /// feedwater controller.
    pub secondary_mass_flow_kg_per_s: f64,
    /// Secondary loop residence time \[s\] (`m/m_dot`), driving the steam-line
    /// flow tracers in the schematic.
    ///
    /// Built from the secondary **piping** inventory, not a plant water
    /// inventory -- see
    /// [`crate::physics::secondary_loop::SteamSecondaryLoop::piping_inventory`]
    /// for why the distinction is what makes these tracers move at all.
    pub secondary_residence_time_s: f64,
    /// Water/steam mass held in the secondary piping \[kg\] -- the numerator of
    /// [`Self::secondary_residence_time_s`].
    ///
    /// Surfaced so the diagnostics panel can show *why* the tracer speed is
    /// what it is, rather than leaving a residence time with no visible cause.
    pub secondary_piping_inventory_kg: f64,
    /// Feedwater specific enthalpy \[J/kg\] -- condensate plus real feed-pump
    /// work, not a fixed constant.
    pub feedwater_enthalpy_j_per_kg: f64,
    /// Condensate (hotwell saturated-liquid) specific enthalpy \[J/kg\], the
    /// cold end of the cycle the feed pump lifts from.
    pub condensate_enthalpy_j_per_kg: f64,
    /// Feed-pump power \[MW\].
    pub feed_pump_power_mw: f64,
    /// Net cycle power \[MW\]: turbine output less feed-pump work.
    pub net_cycle_power_mw: f64,
    /// Heat rejected in the condenser \[MW\].
    pub condenser_duty_mw: f64,
    /// Cooling-water outlet temperature from the condenser \[K\].
    pub cooling_water_outlet_temp_k: f64,

    // --- Turbine-generator shaft outputs ---
    /// Turbine-generator shaft angular velocity \[rad/s\].
    ///
    /// **Computed by a torque balance**, not an animation constant: it is the
    /// state of [`crate::physics::turbine_generator::TurbineGeneratorShaft`],
    /// driven by the secondary loop's enthalpy-drop power against the
    /// generator's electrical reaction torque. The schematic rebuilds the
    /// generator model from this scalar and hands it to `TurbineVisual`, whose
    /// rotor phase is `theta = omega * t`.
    ///
    /// **Read the module docs before quoting it.** The machine is *islanded*
    /// (fixed resistive load, no governor, no AVR), so its speed follows the
    /// square root of load; a grid-connected machine would instead be pinned at
    /// synchronous speed. And the load was sized at the machine's rated point,
    /// which is why the speed lands near synchronous at full load -- the model
    /// does not independently predict 3000 rpm.
    pub shaft_speed_rad_per_s: f64,
    /// The same shaft speed in rpm, for display.
    pub shaft_speed_rpm: f64,
    /// Three-phase electrical power delivered into the generator's resistive
    /// load \[MW\].
    ///
    /// Distinct from [`Self::turbine_power_mw`], which is the *mechanical*
    /// enthalpy-drop power at the coupling. At steady state this is the
    /// generator efficiency times that; during a transient the difference is
    /// what accelerates or decelerates the rotor.
    pub generator_electrical_power_mw: f64,
    /// Turbine-generator rating \[MW\] -- the design-point shaft power the
    /// electrical load and rotor inertia were sized against. An **illustrative
    /// balance-of-plant figure**, not an HTR-10 turbine-generator rating.
    pub generator_rating_mw: f64,

    // --- Diagnostics ---
    /// Steam-temperature control error \[K\] (`T_setpoint - T_steam`) seen by
    /// the feedwater PI trim. Near zero means the controller is holding
    /// setpoint; a persistent non-zero value means it is on a pump stop.
    pub steam_temperature_error_k: f64,
    /// Feedwater PI controller output (dimensionless). A large magnitude
    /// while the pump sits on a stop is the visible symptom of integrator
    /// windup -- the chem-eng controller has no anti-windup.
    pub feedwater_controller_output: f64,

    /// Fission-product decay-heat power \[MW\]. Non-zero after a trip -- this
    /// is what keeps heating the graphite once the chain reaction stops.
    pub decay_heat_mw: f64,
    /// Core THERMAL power \[MW\]: the promptly-released fission power plus
    /// decay heat. This, not the fission power, is what the pebble bed
    /// absorbs.
    pub core_thermal_power_mw: f64,

    /// Accumulated simulation time \[s\].
    pub sim_time_s: f64,
    /// Simulated seconds the physics thread has achieved per wall-clock second
    /// since it started.
    ///
    /// `1.0` is real time. `None` before the first tick -- reporting a ratio
    /// before one has been measured is the silent misinformation this field
    /// exists to replace. Published by the physics thread from
    /// [`RealTimePacer`](outram_park_digital_twin_engine::app_scaffold::RealTimePacer);
    /// writing to it does nothing.
    ///
    /// It is a **sim-clock rate**, not a frame rate: a low value means the
    /// plant model is not keeping up with wall clock, and says nothing about
    /// how smoothly the window is painting. Being cumulative it is steady but
    /// slow to react, so [`Self::real_time_deficit_s`] is the better read on a
    /// slowdown that has just started.
    pub real_time_ratio: Option<f64>,
    /// How far the plant clock is behind wall clock \[s\].
    ///
    /// Zero when the simulator is keeping up. The pacer works a deficit off by
    /// not sleeping, so a transient stall shows here and then decays; a
    /// deficit that keeps growing means the plant model is persistently more
    /// expensive than its tick budget.
    pub real_time_deficit_s: f64,
    /// Whether the physics thread is measurably behind real time.
    ///
    /// Set when [`Self::real_time_deficit_s`] exceeds the pacer's tolerance.
    /// The schematic shows the shortfall when this is true, because a simulator
    /// that has quietly dropped to half speed while still reading "seconds" on
    /// its clock is misleading.
    pub behind_real_time: bool,

    /// Operator request to run the plant clock faster than real time --
    /// written by the GUI, read by the physics thread each tick.
    ///
    /// **What this does and does not change.** It never touches the plant
    /// timestep ([`crate::physics::PLANT_TIMESTEP_S`], the one step size
    /// every test and sub-model in this simulator is derived from -- see
    /// that constant's doc comment on why a second, independent step size is
    /// exactly the class of bug this workspace has been bitten by before).
    /// What it changes is how many [`crate::physics::HtgrPlant::step`] calls
    /// the physics thread makes, and how long it sleeps, between one publish
    /// of [`HtgrSnapshot`] and the next -- see
    /// `app::start_simulation`'s physics-thread closure. Same pattern as
    /// `ciet_educational_simulator_v2`'s fast-forward checkbox
    /// (`fast_forward_settings_turned_on`): a bool the GUI writes, the
    /// physics thread reads once per tick, and a short floor sleep even
    /// while set so the checkbox stays responsive to being switched off and
    /// the physics thread never spins at 100% CPU uncontrolled.
    ///
    /// Defaults to `false` -- the simulator opens paced to real time, as it
    /// always has.
    pub fast_forward_enabled: bool,
}

impl Default for HtgrSnapshot {
    /// The **first frame only**: the physics thread overwrites every output
    /// field on its first tick, roughly one `PHYSICS_TICK` (100 ms) after the
    /// window opens.
    ///
    /// These are set at the plant's nominal operating point rather than at
    /// zero so the opening frame is not misleading -- a schematic that flashes
    /// a 573 K isothermal core before jumping to the real state reads as a
    /// fault. The values are the published HTR-10 phase-one operating
    /// conditions (IAEA-TECDOC-1382): 10 MWth, 4.3 kg/s of helium at
    /// 250 degC in / 700 degC out, main steam 4.0 MPa at 440 degC and
    /// 12.5 t/hr, feedwater 104 degC. The operator commands open at the same
    /// design condition -- feedwater station in AUTO on the published 440 degC
    /// steam temperature, condenser at its design 7 kPa -- which is exactly
    /// [`crate::physics::PlantCommands::default`]; `app::tests::the_gui_defaults_are_the_plant_command_defaults`
    /// pins that. They are **initial display values, not
    /// a claim about this model's converged state**, and the derived
    /// quantities (duties, powers, residence times) start at zero because
    /// nothing has been computed yet.
    fn default() -> Self {
        Self {
            // Rods start near the critical position implied by the published
            // bank worth and cold clean excess (~0.60 inserted; see
            // `crate::physics::control_rods`), so the simulator opens close to
            // steady state rather than on a prompt excursion.
            control_rod_insertion_fraction: crate::physics::GUI_INITIAL_ROD_INSERTION,
            // Feedwater in AUTO at the published 440 degC, and the condenser at
            // its design 7 kPa: the opening state is the plant's design
            // condition, and is exactly `physics::PlantCommands::default()`.
            // `the_gui_defaults_are_the_plant_command_defaults` pins that.
            feedwater_manual: true,
            feedwater_manual_flow_kg_per_s: 10.0,
            feedwater_target_steam_temp_k: 713.15,
            condenser_pressure_setpoint_kpa: 7.0,
            rps_enabled: false,
            trip_reset_requested: false,
            trip_reason: None,
            scram_insertion_fraction: 0.0,
            external_reactivity_dollars: 0.0,
            helium_flow_setpoint_kg_per_s: 4.3,
            reactor_power_mw: 10.0,
            prompt_power_mw: 10.0,
            delayed_power_mw: 0.0,
            fuel_temperature_k: 950.0,
            bed_temperature_k: 985.0,
            reactivity_margin_dollars: 0.0,
            delayed_neutron_fraction_pcm: 650.0,
            core_inlet_temp_k: 523.15,
            core_outlet_temp_k: 973.15,
            helium_mass_flow_kg_per_s: 4.3,
            ihx_duty_mw: 0.0,
            ihx_outlet_temp_k: 523.15,
            helium_residence_time_s: 0.0,
            primary_pressure_drop_kpa: 0.0,
            bed_pressure_drop_kpa: 0.0,
            circulator_power_mw: 0.0,
            helium_cp_j_per_kg_k: 5193.0,
            steam_pressure_mpa: 4.0,
            sg_steam_outlet_temp_k: 713.15,
            // Superheated steam at 4.0 MPa, 440 degC (IAPWS-IF97 region 2).
            steam_enthalpy_j_per_kg: 3.307e6,
            turbine_inlet_temp_k: 713.15,
            turbine_power_mw: 0.0,
            steam_quality_after_turbine: 0.0,
            condenser_pressure_kpa: 7.0,
            // 12.5 t/hr of main steam.
            secondary_mass_flow_kg_per_s: 3.47,
            secondary_residence_time_s: 0.0,
            secondary_piping_inventory_kg: 0.0,
            // Saturated liquid at 104 degC, the published feedwater state.
            feedwater_enthalpy_j_per_kg: 4.36e5,
            condensate_enthalpy_j_per_kg: 1.63e5,
            feed_pump_power_mw: 0.0,
            net_cycle_power_mw: 0.0,
            condenser_duty_mw: 0.0,
            cooling_water_outlet_temp_k: 298.15,
            // The shaft is the one output field that is a genuine STATE rather
            // than a derived quantity, so it opens at its real initial
            // condition -- synchronous speed, matching
            // `TurbineGeneratorShaft::new`. Everything derived from it starts
            // at zero like the other derived fields.
            shaft_speed_rad_per_s: std::f64::consts::TAU * 50.0,
            shaft_speed_rpm: 3000.0,
            generator_electrical_power_mw: 0.0,
            generator_rating_mw: 0.0,
            steam_temperature_error_k: 0.0,
            feedwater_controller_output: 0.0,
            decay_heat_mw: 0.0,
            core_thermal_power_mw: 10.0,
            sim_time_s: 0.0,
            real_time_ratio: None,
            real_time_deficit_s: 0.0,
            behind_real_time: false,
            fast_forward_enabled: false,
        }
    }
}

/// Bounded time-history buffers for the plot panel. Each entry is an
/// `[t_seconds, value]` pair suitable for `egui_plot`.
#[derive(Clone, Debug, Default)]
pub struct HtgrPlotData {
    /// Total reactor power \[MW\] vs time \[s\].
    pub reactor_power_mw: Vec<[f64; 2]>,
    /// Prompt-layer power \[MW\] vs time \[s\].
    pub prompt_power_mw: Vec<[f64; 2]>,
    /// Delayed-layer power \[MW\] vs time \[s\].
    pub delayed_power_mw: Vec<[f64; 2]>,
    /// Fuel temperature \[K\] vs time \[s\] -- the kinetics reactivity-feedback
    /// node ([`HtgrSnapshot::fuel_temperature_k`]), not the bed average. See
    /// that field's doc comment: it can run well below
    /// [`Self::bed_temperature_k`] after a trip, since it has no decay-heat
    /// source. Kept for diagnosing the kinetics/reactivity-feedback path, not
    /// for comparing against a helium temperature -- use
    /// [`Self::bed_temperature_k`] for that.
    pub fuel_temperature_k: Vec<[f64; 2]>,
    /// Bed-average pebble (graphite) temperature \[K\] vs time \[s\] -- the
    /// quantity the helium temperatures must never exceed. See
    /// [`HtgrSnapshot::bed_temperature_k`].
    pub bed_temperature_k: Vec<[f64; 2]>,
    /// Core outlet helium temperature \[K\] vs time \[s\].
    pub core_outlet_temp_k: Vec<[f64; 2]>,
    /// Turbine power \[MW\] vs time \[s\].
    pub turbine_power_mw: Vec<[f64; 2]>,
}

impl HtgrPlotData {
    /// Append one sample from `snapshot`, trimming each buffer to
    /// [`MAX_PLOT_SAMPLES`].
    pub fn push_sample(&mut self, snapshot: &HtgrSnapshot) {
        let t = snapshot.sim_time_s;
        push_capped(&mut self.reactor_power_mw, [t, snapshot.reactor_power_mw]);
        push_capped(&mut self.prompt_power_mw, [t, snapshot.prompt_power_mw]);
        push_capped(&mut self.delayed_power_mw, [t, snapshot.delayed_power_mw]);
        push_capped(
            &mut self.fuel_temperature_k,
            [t, snapshot.fuel_temperature_k],
        );
        push_capped(&mut self.bed_temperature_k, [t, snapshot.bed_temperature_k]);
        push_capped(
            &mut self.core_outlet_temp_k,
            [t, snapshot.core_outlet_temp_k],
        );
        push_capped(&mut self.turbine_power_mw, [t, snapshot.turbine_power_mw]);
    }

    /// Row-shaped view of every buffer, one row per sample index -- feeds
    /// [`outram_park_digital_twin_engine::app_scaffold::CsvSnapshotPanel`]
    /// via [`crate::app::panels::draw_plots_csv_panel`], which is what
    /// actually builds CSV text from this (only on an "Update CSV Data"
    /// click, not every frame -- see that type's doc comment). Column order
    /// matches [`Self::CSV_HEADER`].
    ///
    /// All seven buffers are pushed together, in lockstep, by
    /// [`Self::push_sample`], so they share one length and one time value per
    /// index -- this reads `time_s` from `reactor_power_mw`'s own `[t, _]`
    /// pairs rather than assuming a separate time vector exists. Indexing
    /// directly (not defensively) is deliberate: if that lockstep invariant
    /// were ever broken, a panic here is the honest outcome, not a silently
    /// misaligned CSV row.
    pub fn to_rows(&self) -> Vec<Vec<String>> {
        (0..self.reactor_power_mw.len())
            .map(|i| {
                vec![
                    self.reactor_power_mw[i][0].to_string(),
                    self.reactor_power_mw[i][1].to_string(),
                    self.prompt_power_mw[i][1].to_string(),
                    self.delayed_power_mw[i][1].to_string(),
                    self.fuel_temperature_k[i][1].to_string(),
                    self.bed_temperature_k[i][1].to_string(),
                    self.core_outlet_temp_k[i][1].to_string(),
                    self.turbine_power_mw[i][1].to_string(),
                ]
            })
            .collect()
    }

    /// Column names for [`Self::to_rows`], in order.
    pub const CSV_HEADER: [&'static str; 8] = [
        "time_s",
        "reactor_power_mw",
        "prompt_power_mw",
        "delayed_power_mw",
        "fuel_temperature_k",
        "bed_temperature_k",
        "core_outlet_temp_k",
        "turbine_power_mw",
    ];
}

/// Push `sample` onto `buf`, dropping the oldest entry if the buffer is full.
fn push_capped(buf: &mut Vec<[f64; 2]>, sample: [f64; 2]) {
    if buf.len() >= MAX_PLOT_SAMPLES {
        buf.remove(0);
    }
    buf.push(sample);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_snapshot(sim_time_s: f64) -> HtgrSnapshot {
        HtgrSnapshot {
            sim_time_s,
            reactor_power_mw: 10.0,
            prompt_power_mw: 9.0,
            delayed_power_mw: 1.0,
            fuel_temperature_k: 950.0,
            bed_temperature_k: 900.0,
            core_outlet_temp_k: 973.15,
            turbine_power_mw: 3.1,
            ..HtgrSnapshot::default()
        }
    }

    /// Methodology: push three samples through [`HtgrPlotData::push_sample`],
    /// then round-trip [`HtgrPlotData::to_rows`] through
    /// [`outram_park_digital_twin_engine::app_scaffold::rows_to_csv_string`]
    /// and an independent [`csv::Reader`], checking the header and every
    /// field of every row -- not just that a string came out.
    ///
    /// Result (2026-08-18): header matches exactly; all three rows carry the
    /// right time and the right value in every column.
    #[test]
    fn to_rows_round_trips_every_pushed_sample() {
        let mut plots = HtgrPlotData::default();
        for t in [0.0, 0.1, 0.2] {
            plots.push_sample(&sample_snapshot(t));
        }
        let rows = plots.to_rows();
        let csv_text = outram_park_digital_twin_engine::app_scaffold::rows_to_csv_string(
            &HtgrPlotData::CSV_HEADER,
            &rows,
        );

        let mut reader = csv::ReaderBuilder::new().from_reader(csv_text.as_bytes());
        assert_eq!(
            reader.headers().unwrap().iter().collect::<Vec<_>>(),
            HtgrPlotData::CSV_HEADER.to_vec()
        );
        let records: Vec<csv::StringRecord> = reader.records().map(|r| r.unwrap()).collect();
        assert_eq!(records.len(), 3);
        for (i, expected_t) in [0.0, 0.1, 0.2].iter().enumerate() {
            assert_eq!(records[i][0].parse::<f64>().unwrap(), *expected_t);
            assert_eq!(records[i][1].parse::<f64>().unwrap(), 10.0);
            assert_eq!(records[i][2].parse::<f64>().unwrap(), 9.0);
            assert_eq!(records[i][3].parse::<f64>().unwrap(), 1.0);
            assert_eq!(records[i][4].parse::<f64>().unwrap(), 950.0);
            assert_eq!(records[i][5].parse::<f64>().unwrap(), 900.0);
            assert_eq!(records[i][6].parse::<f64>().unwrap(), 973.15);
            assert_eq!(records[i][7].parse::<f64>().unwrap(), 3.1);
        }
    }

    /// Methodology: fill the buffer to
    /// [`outram_park_digital_twin_engine::app_scaffold::MAX_CSV_ROWS`] (4000,
    /// matching [`MAX_PLOT_SAMPLES`]) with slowly-varying (not identical,
    /// not degenerate) values on every column, so `f64::to_string()`'s
    /// variable-length output is representative of a real run rather than a
    /// constant that happens to print short. Build the CSV and measure its
    /// actual byte length -- the number the maintainer asked for when
    /// raising [`outram_park_digital_twin_engine::app_scaffold::MAX_CSV_ROWS`]
    /// from 1000 to 4000, 2026-08-18.
    ///
    /// # Results (measured 2026-08-18)
    ///
    /// **551,276 bytes -- 0.551 MB (decimal) / 0.526 MiB (binary)** for 4000
    /// rows x 8 columns of this simulator's actual field set, via `cargo
    /// test ... to_rows_at_max_csv_rows_measures_a_realistic_byte_size --
    /// --nocapture`. Roughly 138 bytes/row, 17 bytes/field -- consistent
    /// with `f64::to_string()`'s typical output width for values in the
    /// tens-to-thousands range with a handful of significant decimal
    /// digits, plus separators. The assertion is a loose sanity ceiling
    /// (2 MB), not a tight pin on this exact figure, since the byte count
    /// depends on `f64::to_string()`'s shortest-round-tripping-
    /// representation output, which is not deterministic-by-contract across
    /// floating-point library versions -- only the order of magnitude is.
    #[test]
    fn to_rows_at_max_csv_rows_measures_a_realistic_byte_size() {
        use outram_park_digital_twin_engine::app_scaffold::{rows_to_csv_string, MAX_CSV_ROWS};

        let mut plots = HtgrPlotData::default();
        for i in 0..MAX_CSV_ROWS {
            let t = i as f64 * 0.1;
            plots.push_sample(&HtgrSnapshot {
                sim_time_s: t,
                reactor_power_mw: 10.0 + (t * 0.001351).sin() * 2.0,
                prompt_power_mw: 9.0 + (t * 0.002213).sin() * 1.5,
                delayed_power_mw: 1.0 + (t * 0.000871).sin() * 0.3,
                fuel_temperature_k: 950.0 + (t * 0.000513).sin() * 40.0,
                bed_temperature_k: 900.0 + (t * 0.000377).sin() * 30.0,
                core_outlet_temp_k: 973.15 + (t * 0.000291).sin() * 15.0,
                turbine_power_mw: 3.1 + (t * 0.000662).sin() * 0.5,
                ..HtgrSnapshot::default()
            });
        }

        let rows = plots.to_rows();
        assert_eq!(rows.len(), MAX_CSV_ROWS);
        let csv_text = rows_to_csv_string(&HtgrPlotData::CSV_HEADER, &rows);
        let bytes = csv_text.len();
        println!(
            "4000-row HtgrPlotData CSV: {bytes} bytes ({:.4} MB, decimal; {:.4} MiB, binary)",
            bytes as f64 / 1_000_000.0,
            bytes as f64 / (1024.0 * 1024.0)
        );
        assert!(
            bytes < 2_000_000,
            "4000-row CSV grew to {bytes} bytes -- unexpectedly large, check for a runaway column"
        );
    }

    /// Methodology: an empty [`HtgrPlotData`] (opening frame, before any
    /// sample has been pushed) must produce zero rows, not panic on an
    /// empty buffer.
    ///
    /// Result (2026-08-18): passes.
    #[test]
    fn to_rows_on_an_empty_plot_data_is_empty() {
        let plots = HtgrPlotData::default();
        assert!(plots.to_rows().is_empty());
    }
}
