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

    // --- Kinetics outputs ---
    /// Total reactor thermal power (prompt + delayed) \[MW\].
    pub reactor_power_mw: f64,
    /// Prompt-excursion-layer power \[MW\].
    pub prompt_power_mw: f64,
    /// Delayed-neutron power increment `S*dt` added this step \[MW\].
    pub delayed_power_mw: f64,
    /// Lumped fuel temperature \[K\].
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
    pub secondary_residence_time_s: f64,
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

    // --- Diagnostics ---
    /// Accumulated simulation time \[s\].
    pub sim_time_s: f64,
}

impl Default for HtgrSnapshot {
    /// The **first frame only**: the physics thread overwrites every output
    /// field on its first tick, roughly 10 ms after the window opens.
    ///
    /// These are set at the plant's nominal operating point rather than at
    /// zero so the opening frame is not misleading -- a schematic that flashes
    /// a 573 K isothermal core before jumping to the real state reads as a
    /// fault. The values are the published HTR-10 phase-one operating
    /// conditions (IAEA-TECDOC-1382): 10 MWth, 4.3 kg/s of helium at
    /// 250 degC in / 700 degC out, main steam 4.0 MPa at 440 degC and
    /// 12.5 t/hr, feedwater 104 degC. They are **initial display values, not
    /// a claim about this model's converged state**, and the derived
    /// quantities (duties, powers, residence times) start at zero because
    /// nothing has been computed yet.
    fn default() -> Self {
        Self {
            // Rods start near the critical position implied by the published
            // bank worth and cold clean excess (~0.60 inserted; see
            // `crate::physics::control_rods`), so the simulator opens close to
            // steady state rather than on a prompt excursion.
            control_rod_insertion_fraction: 0.6035,
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
            // Saturated liquid at 104 degC, the published feedwater state.
            feedwater_enthalpy_j_per_kg: 4.36e5,
            condensate_enthalpy_j_per_kg: 1.63e5,
            feed_pump_power_mw: 0.0,
            net_cycle_power_mw: 0.0,
            condenser_duty_mw: 0.0,
            cooling_water_outlet_temp_k: 298.15,
            sim_time_s: 0.0,
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
    /// Fuel temperature \[K\] vs time \[s\].
    pub fuel_temperature_k: Vec<[f64; 2]>,
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
        push_capped(
            &mut self.core_outlet_temp_k,
            [t, snapshot.core_outlet_temp_k],
        );
        push_capped(&mut self.turbine_power_mw, [t, snapshot.turbine_power_mw]);
    }
}

/// Push `sample` onto `buf`, dropping the oldest entry if the buffer is full.
fn push_capped(buf: &mut Vec<[f64; 2]>, sample: [f64; 2]) {
    if buf.len() >= MAX_PLOT_SAMPLES {
        buf.remove(0);
    }
    buf.push(sample);
}
