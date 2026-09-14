//! The execution boundary: controls in, bounded tick, snapshot out.
//!
//! Bead `op-37ke` (kernel extraction), under epic `op-276r` / gh #148. Design:
//! `docs/architecture/htgr-sim-execution.md`. Audit of what this replaces:
//! `docs/architecture/htgr-sim-dataflow-audit.md`.
//!
//! ## What this fixes
//!
//! The audit found that `HtgrSnapshot` is **not a snapshot**. It is one struct
//! under one `RwLock` carrying traffic in *both* directions: the GUI writes
//! control fields into it, the physics thread reads them back out via
//! `plant_commands_from`, writes 66 output fields in, and writes control state
//! back (`trip_reset_requested = false`). A shared mutable blackboard cannot
//! give atomic publication — a reader can see outputs from timestep N beside a
//! control it has just written.
//!
//! [`PlantRuntime`] separates the directions:
//!
//! ```text
//!   PlantControls  ──submit──►  PlantRuntime  ──publish──►  HtgrSnapshot
//!    (one way in)                 owns plant                 (one way out)
//!                                     │
//!                                  tick(dt)
//!                              bounded, no loop,
//!                               no thread, no clock
//! ```
//!
//! ## Why this is the whole extraction
//!
//! The physics needed **no change at all** — `HtgrPlant::step` was already
//! deterministic, bounded and thread-free, because the loop lived in
//! `app_scaffold::spawn_physics_thread` rather than in `physics/`. So this
//! module is not a rewrite of the physics; it is the removal of the *only*
//! thing that made the physics unschedulable: the surrounding
//! `thread::spawn(move || loop { .. })`.
//!
//! [`PlantRuntime::tick`] advances **exactly one** plant timestep and returns.
//! Who calls it, how often, and on which thread is the platform layer's
//! business — a native scheduler, a Web Worker, or a test harness.
//!
//! ## What this deliberately does not do
//!
//! It does not own a thread, a clock, a pacing policy or a substep count. Those
//! were in the old physics closure (`app/mod.rs`, which spun on
//! `tick_start.elapsed()`), and they are exactly what must move out to the
//! platform layer.

use crate::app::state::HtgrSnapshot;
use crate::physics::secondary_loop::{FeedwaterCommand, SecondaryCommands};
use crate::physics::{HtgrPlant, PlantCommands};
use uom::si::f64::{MassRate, Pressure, ThermodynamicTemperature, Time};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::pressure::kilopascal;
use uom::si::thermodynamic_temperature::kelvin;

/// Everything the operator commands. **One way: GUI into the runtime.**
///
/// Distinct from [`PlantCommands`], which is the physics layer's own input
/// type. This carries the *operator's* intent including the protection-system
/// controls, which are runtime state rather than per-step physics arguments.
#[derive(Clone, Debug, PartialEq)]
pub struct PlantControls {
    pub control_rod_insertion_fraction: f64,
    pub helium_flow_setpoint_kg_per_s: f64,
    pub feedwater_manual: bool,
    pub feedwater_manual_flow_kg_per_s: f64,
    pub feedwater_target_steam_temp_k: f64,
    pub condenser_pressure_setpoint_kpa: f64,
    /// Whether the reactor protection system is armed.
    pub rps_enabled: bool,
    /// A one-shot trip-reset request. The runtime consumes it — see
    /// [`PlantRuntime::submit`] on why this cannot latch.
    pub trip_reset_requested: bool,
}

impl PlantControls {
    /// Read the control fields out of a snapshot.
    ///
    /// A **migration shim**: the GUI still writes controls into `HtgrSnapshot`
    /// today. It exists so the runtime can be adopted without rewriting every
    /// panel in the same change, and should be deleted once the GUI submits
    /// [`PlantControls`] directly (bead: async GUI integration).
    pub fn from_snapshot(s: &HtgrSnapshot) -> Self {
        Self {
            control_rod_insertion_fraction: s.control_rod_insertion_fraction,
            helium_flow_setpoint_kg_per_s: s.helium_flow_setpoint_kg_per_s,
            feedwater_manual: s.feedwater_manual,
            feedwater_manual_flow_kg_per_s: s.feedwater_manual_flow_kg_per_s,
            feedwater_target_steam_temp_k: s.feedwater_target_steam_temp_k,
            condenser_pressure_setpoint_kpa: s.condenser_pressure_setpoint_kpa,
            rps_enabled: s.rps_enabled,
            trip_reset_requested: s.trip_reset_requested,
        }
    }

    /// The per-step physics arguments these controls imply.
    fn to_commands(&self) -> PlantCommands {
        PlantCommands {
            control_rod_insertion_fraction: self.control_rod_insertion_fraction,
            helium_flow_setpoint: MassRate::new::<kilogram_per_second>(
                self.helium_flow_setpoint_kg_per_s,
            ),
            secondary: SecondaryCommands {
                feedwater: if self.feedwater_manual {
                    FeedwaterCommand::Manual {
                        mass_flow_demand: MassRate::new::<kilogram_per_second>(
                            self.feedwater_manual_flow_kg_per_s,
                        ),
                    }
                } else {
                    FeedwaterCommand::Auto {
                        target_steam_temperature: ThermodynamicTemperature::new::<kelvin>(
                            self.feedwater_target_steam_temp_k,
                        ),
                    }
                },
                condenser_pressure: Pressure::new::<kilopascal>(
                    self.condenser_pressure_setpoint_kpa,
                ),
            },
        }
    }
}

impl Default for PlantControls {
    /// Matches `PlantCommands::default()` — the published operating point.
    ///
    /// Note that despite that type's docstring, this is **not** a steady-state
    /// hold: it is a startup transient reaching ~2.8x nominal power near 100 s
    /// before settling. See `headless`'s reference baseline.
    fn default() -> Self {
        let d = PlantCommands::default();
        let (manual, manual_flow, target_t) = match d.secondary.feedwater {
            FeedwaterCommand::Manual { mass_flow_demand } => {
                (true, mass_flow_demand.get::<kilogram_per_second>(), 0.0)
            }
            FeedwaterCommand::Auto {
                target_steam_temperature,
            } => (false, 0.0, target_steam_temperature.get::<kelvin>()),
        };
        Self {
            control_rod_insertion_fraction: d.control_rod_insertion_fraction,
            helium_flow_setpoint_kg_per_s: d.helium_flow_setpoint.get::<kilogram_per_second>(),
            feedwater_manual: manual,
            feedwater_manual_flow_kg_per_s: manual_flow,
            feedwater_target_steam_temp_k: target_t,
            condenser_pressure_setpoint_kpa: d.secondary.condenser_pressure.get::<kilopascal>(),
            // DISABLED, matching `ReactorProtectionSystem::new()` (which is
            // documented as "disabled by default") and `HtgrSnapshot::default()`
            // (`rps_enabled: false`).
            //
            // This defaulted to `true` when first written, and the reference
            // baseline caught it immediately: arming the RPS tripped the plant
            // and collapsed power at step 825 from 13.83 MW to 1.01 MW. A
            // runtime default that silently arms a protection system is a
            // behaviour change wearing the costume of a constructor.
            rps_enabled: false,
            trip_reset_requested: false,
        }
    }
}

/// Owns the plant and advances it one bounded timestep at a time.
///
/// **Owns no thread, no clock and no loop.** See the module docs.
pub struct PlantRuntime {
    plant: HtgrPlant,
    controls: PlantControls,
    commands: PlantCommands,
    completed_steps: u64,
}

impl PlantRuntime {
    /// A runtime holding a fresh plant at the default operating point.
    pub fn new() -> Self {
        let controls = PlantControls::default();
        let commands = controls.to_commands();
        Self {
            plant: HtgrPlant::new(),
            controls,
            commands,
            completed_steps: 0,
        }
    }

    /// Accept new operator intent. **One way in.**
    ///
    /// Applies the protection-system controls immediately, because arming and
    /// trip-reset are runtime state rather than per-step physics arguments.
    ///
    /// The trip reset is **consumed here**, so one request produces one reset —
    /// it cannot latch and repeatedly clear a genuine trip. That behaviour was
    /// previously achieved by the physics thread writing `false` back into the
    /// shared snapshot, which is exactly the reverse-direction write the audit
    /// flagged. Consuming it on entry preserves the behaviour without the
    /// back-write.
    pub fn submit(&mut self, controls: PlantControls) {
        if self.plant.protection.is_enabled() != controls.rps_enabled {
            self.plant.protection.set_enabled(controls.rps_enabled);
        }
        if controls.trip_reset_requested {
            self.plant.protection.reset();
        }
        self.commands = controls.to_commands();
        self.controls = PlantControls {
            trip_reset_requested: false,
            ..controls
        };
    }

    /// Advance **exactly one** plant timestep, then return.
    ///
    /// Bounded, deterministic, single-threaded, no clock. This is the kernel
    /// boundary: the platform layer decides when and where it is called.
    pub fn tick(&mut self, dt: Time) {
        self.plant.step(dt, self.commands.clone());
        self.completed_steps += 1;
    }

    /// Write the current plant state out. **One way out.**
    ///
    /// Control fields are written back too, only because the GUI still reads
    /// its own control state from `HtgrSnapshot`. That is the migration shim
    /// described on [`PlantControls::from_snapshot`], not part of the target
    /// design.
    pub fn publish(&self, out: &mut HtgrSnapshot) {
        self.plant.write_snapshot(out);
        out.control_rod_insertion_fraction = self.controls.control_rod_insertion_fraction;
        out.helium_flow_setpoint_kg_per_s = self.controls.helium_flow_setpoint_kg_per_s;
        out.feedwater_manual = self.controls.feedwater_manual;
        out.feedwater_manual_flow_kg_per_s = self.controls.feedwater_manual_flow_kg_per_s;
        out.feedwater_target_steam_temp_k = self.controls.feedwater_target_steam_temp_k;
        out.condenser_pressure_setpoint_kpa = self.controls.condenser_pressure_setpoint_kpa;
        out.rps_enabled = self.controls.rps_enabled;
        out.trip_reset_requested = false;
    }

    /// Timesteps completed since construction.
    pub fn completed_steps(&self) -> u64 {
        self.completed_steps
    }

    /// Read-only access to the plant, for tests and diagnostics.
    pub fn plant(&self) -> &HtgrPlant {
        &self.plant
    }
}

impl Default for PlantRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physics::PLANT_TIMESTEP_S;
    use uom::si::time::second;

    /// The kernel contract's central property: `tick` advances one step and
    /// returns. If this ever loops or blocks, the runtime is unschedulable.
    #[test]
    fn tick_advances_exactly_one_step() {
        let mut rt = PlantRuntime::new();
        assert_eq!(rt.completed_steps(), 0);
        rt.tick(Time::new::<second>(PLANT_TIMESTEP_S));
        assert_eq!(rt.completed_steps(), 1);
    }

    /// A trip reset must not latch. Submitting it once resets once; the
    /// runtime clears it on entry rather than writing back through the
    /// snapshot.
    #[test]
    fn trip_reset_is_consumed_not_latched() {
        let mut rt = PlantRuntime::new();
        rt.submit(PlantControls {
            trip_reset_requested: true,
            ..PlantControls::default()
        });
        assert!(
            !rt.controls.trip_reset_requested,
            "trip reset latched instead of being consumed"
        );
    }

    /// `PlantControls::default()` must agree with the plant's own initial
    /// state, or constructing a runtime silently changes the plant.
    ///
    /// Regression test for the bug the reference baseline caught: this
    /// defaulted `rps_enabled` to `true` while `ReactorProtectionSystem::new()`
    /// is disabled, which armed the RPS, tripped the plant, and collapsed
    /// power by an order of magnitude 82 s in.
    #[test]
    fn default_controls_do_not_change_the_plants_initial_state() {
        let fresh = HtgrPlant::new();
        let controls = PlantControls::default();
        assert_eq!(
            controls.rps_enabled,
            fresh.protection.is_enabled(),
            "PlantControls::default() disagrees with HtgrPlant::new() on RPS arming"
        );

        let mut rt = PlantRuntime::new();
        rt.submit(PlantControls::default());
        assert_eq!(
            rt.plant().protection.is_enabled(),
            fresh.protection.is_enabled(),
            "submitting default controls changed the protection state"
        );
    }

    /// Controls submitted are the controls published — the round trip does not
    /// silently drop or alter operator intent.
    #[test]
    fn submitted_controls_survive_the_round_trip() {
        let mut rt = PlantRuntime::new();
        let mut c = PlantControls::default();
        c.control_rod_insertion_fraction = 0.42;
        c.helium_flow_setpoint_kg_per_s = 3.5;
        rt.submit(c.clone());

        let mut snap = HtgrSnapshot::default();
        rt.publish(&mut snap);
        assert_eq!(snap.control_rod_insertion_fraction, 0.42);
        assert_eq!(snap.helium_flow_setpoint_kg_per_s, 3.5);
    }
}
