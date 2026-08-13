//! Graceful salt-freeze handling: detect, pause, tell the operator, offer a
//! melt.
//!
//! # The failure this replaces
//!
//! Both salt loops in this simulator carry a fluid whose property
//! correlations are only valid above a lower temperature bound, and
//! `tuas_boussinesq_solver` **range-checks every property call** against it.
//! Drive a loop below that bound — by over-cooling it at the steam generator,
//! or by stopping circulation while heat is still being removed — and the next
//! density / viscosity / `cp` lookup returns an out-of-range error that the
//! thermal-hydraulics stack unwraps. The physics thread dies mid-timestep.
//!
//! Before 2026-08-12 that was the entire story: the panic was caught by
//! [`ThreadHealth`](outram_park_digital_twin_engine::app_scaffold::ThreadHealth)
//! and the user was shown "Simulation crashed — please restart", with the raw
//! unwrap message under a details header. That is technically safe and
//! educationally useless. A frozen salt loop is a *real and well known*
//! operational hazard for a molten-salt reactor; it deserves to be taught, not
//! reported as a software fault.
//!
//! # What this module does instead
//!
//! 1. **Detects the freeze from state, before the property call that would
//!    panic.** [`detect_salt_freeze`] is a pure function of the temperature
//!    profiles the *previous* timestep produced. The driver checks it at the
//!    top of the loop and, if a node has gone below its salt's lower bound,
//!    never enters the step that would look up a property there. Stopping at a
//!    known-good state matters: recovering from a `catch_unwind` would leave
//!    the loop half-advanced and its stored energy indeterminate.
//! 2. **Pauses the physics thread** rather than killing it. The thread parks in
//!    a short sleep loop, holding the plant exactly as it was when it froze.
//! 3. **Tells the operator what happened** — which salt, which component, the
//!    measured temperature and the bound it crossed
//!    ([`SaltFreezeEvent::headline`]).
//! 4. **Offers a melt** ([`show_salt_freeze_modal`]) which restores the frozen
//!    loop and resumes.
//!
//! # The melt is a cheat, and says so
//!
//! Melting a frozen salt loop from a GUI button is **not** physical. A real
//! salt loop is thawed with trace heating over hours to days, with a genuine
//! risk of never recovering the loop at all: local re-freeze plugs, thermal
//! stress on the piping, and blocked flow paths that trace heating cannot
//! reach. Nothing of that is modelled here. What the button does is
//! **reconstruct the frozen loop's components at
//! [`MELT_RESTORE_TEMPERATURE_DEGC`]**, i.e. teleport the loop back to the
//! simulator's own cold-start condition.
//!
//! That is an acceptable trade for an educational demonstrator — the
//! alternative is ending the session — but it is stated plainly in the modal
//! text rather than presented as the plant recovering by itself. Do not
//! reword the modal to imply otherwise.
//!
//! # Threshold provenance
//!
//! The thresholds are **not invented here**. Each is
//! `LiquidMaterial::min_temperature()` from `tuas_boussinesq_solver`'s liquid
//! database — the same number the range check that would panic uses, which is
//! what makes detection exact rather than approximate:
//!
//! | Salt | `min_temperature()` | degC | What that number is |
//! |---|---|---|---|
//! | FLiBe (primary loop) | 732.2 K | 459.05 | The **melting point**. `flibe.rs` states the density correlation "applies from melting point to critical point, 732.2 - 4498.8 K", citing Romatoski & Hu (2017), *Ann. Nucl. Energy* **109**, 635-647, and Sohal et al. (2010), INL/EXT-10-18297. |
//! | HITEC (intermediate loop) | 440.0 K | 166.85 | The **lower bound of the property correlations**, from Du et al. (2018), *Int. Comm. Heat Mass Transfer* **96**, 61-68 (valid 440-800 K). HITEC's literature melting point is lower (~142 degC), so this is a *conservative* freeze alarm: the simulator stops before the salt would actually solidify, because that is where its own physics stops being valid. |
//!
//! These match the warnings the simulator's own side panel already shows
//! ("FLiBe min temp ... KEEP ABOVE 470 degrees C or else freeze", "HITEC min
//! temp (steam generator) KEEP ABOVE 170 degrees C or else freeze"), which are
//! rounded-off versions of the same two numbers.
//!
//! # Scope
//!
//! This handles **freezing** only. The other out-of-range failure the
//! simulator suffers — HITEC thermally decomposing above 800 K / 526.85 degC —
//! is the same shape of problem and would fit the same machinery, but is not
//! implemented here; it still reaches the crash modal.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use tuas_boussinesq_solver::boussinesq_thermophysical_properties::LiquidMaterial;
use uom::si::f64::*;
use uom::si::thermodynamic_temperature::degree_celsius;

use crate::app::thermal_hydraulics_backend::fhr_thermal_hydraulics_state::FHRThermalHydraulicsState;

/// Temperature every component of a melted loop is rebuilt at \[degC\].
///
/// This is the simulator's own cold-start temperature (`FHRState::default()`
/// initialises both loops at 500 degC), chosen so a melt lands the plant in a
/// state the model is already known to run from rather than at some new
/// hand-picked condition. It sits comfortably above both thresholds: 41 K
/// above FLiBe's 459.05 degC melting point and 333 K above HITEC's 166.85 degC
/// correlation floor, while still 27 K below HITEC's 526.85 degC
/// decomposition limit.
pub const MELT_RESTORE_TEMPERATURE_DEGC: f64 = 500.0;

/// How long the paused physics thread sleeps between checks for a melt
/// request \[ms\]. Short enough that the button feels immediate, long enough
/// that a paused simulator does not spin a core.
pub const FREEZE_PAUSE_POLL_INTERVAL_MS: u64 = 50;

/// Which salt loop froze. The two loops carry different salts with different
/// thresholds, and only the frozen one is restored on a melt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrozenLoop {
    /// The FHR primary loop: FLiBe, from the reactor vessel round through the
    /// downcomers, pipes 4/5/7/8/10/11, pump 9 and the IHX **shell** side.
    PrimaryFlibe,
    /// The intermediate loop: HITEC, the IHX **tube** side plus pipes
    /// 12/13/15/17, pump 16 and the steam-generator shell.
    IntermediateHitec,
}

impl FrozenLoop {
    /// The salt this loop carries, as declared by the component constructors
    /// in `components.rs`.
    pub fn salt(&self) -> LiquidMaterial {
        match self {
            Self::PrimaryFlibe => LiquidMaterial::FLiBe,
            Self::IntermediateHitec => LiquidMaterial::HITEC,
        }
    }

    /// Human name of the salt, for the modal text.
    pub fn salt_name(&self) -> &'static str {
        match self {
            Self::PrimaryFlibe => "FLiBe",
            Self::IntermediateHitec => "HITEC",
        }
    }

    /// Human name of the loop, for the modal text.
    pub fn loop_name(&self) -> &'static str {
        match self {
            Self::PrimaryFlibe => "primary (FLiBe) loop",
            Self::IntermediateHitec => "intermediate (HITEC) loop",
        }
    }

    /// The freeze threshold for this loop \[K\] —
    /// `LiquidMaterial::min_temperature()` straight from
    /// `tuas_boussinesq_solver`, i.e. exactly the bound whose range check
    /// would otherwise panic the physics thread. See the module docs for what
    /// each number physically is.
    pub fn freeze_threshold(&self) -> ThermodynamicTemperature {
        self.salt().min_temperature()
    }

    /// The freeze threshold in degC, for display and for comparison against
    /// the simulator's degC temperature profiles.
    pub fn freeze_threshold_degc(&self) -> f64 {
        self.freeze_threshold().get::<degree_celsius>()
    }
}

/// A recorded salt freeze: which salt, where, how cold, and against what
/// bound.
///
/// Carries enough to teach the operator something rather than just signal an
/// error — see [`Self::headline`].
#[derive(Debug, Clone)]
pub struct SaltFreezeEvent {
    /// Which loop (and therefore which salt) froze.
    pub frozen_loop: FrozenLoop,
    /// Human-readable component name of the coldest offending node, e.g.
    /// `"steam generator shell (component 14)"`.
    pub location: String,
    /// The coldest temperature measured anywhere in that loop \[degC\] — the
    /// node that tripped the guard.
    pub coldest_temperature_degc: f64,
    /// The threshold it fell below \[degC\].
    pub threshold_degc: f64,
}

impl SaltFreezeEvent {
    /// One sentence naming the salt, the place, the measured temperature and
    /// the bound, e.g.
    ///
    /// ```text
    /// FLiBe froze in the reactor vessel (component 1) at 452.3 degC (freezes at 459.1 degC)
    /// ```
    ///
    /// This is the line the modal leads with. "Simulation error" teaches
    /// nothing; this teaches what a salt-reactor operator actually has to
    /// avoid.
    pub fn headline(&self) -> String {
        format!(
            "{} froze in the {} at {:.1} degC (freezes at {:.1} degC)",
            self.frozen_loop.salt_name(),
            self.location,
            self.coldest_temperature_degc,
            self.threshold_degc,
        )
    }

    /// How far below the threshold the loop went \[K\]. Always positive for a
    /// recorded event.
    pub fn undershoot_kelvin(&self) -> f64 {
        self.threshold_degc - self.coldest_temperature_degc
    }
}

/// Every temperature profile in the state, paired with the loop it belongs to
/// and a human name.
///
/// The IHX contributes **both** its sides: its shell carries primary FLiBe and
/// its tube carries intermediate HITEC (see
/// `new_ihx_sthe_6_version_1` in `components.rs`), so the two halves are
/// checked against different thresholds.
fn labelled_profiles(
    state: &FHRThermalHydraulicsState,
) -> Vec<(FrozenLoop, &'static str, &Vec<f64>)> {
    use FrozenLoop::{IntermediateHitec, PrimaryFlibe};
    vec![
        // ── primary loop, FLiBe ──────────────────────────────────────────
        (
            PrimaryFlibe,
            "reactor vessel (component 1)",
            &state.reactor_temp_profile_degc,
        ),
        (
            PrimaryFlibe,
            "downcomer 2",
            &state.downcomer_2_temp_profile_degc,
        ),
        (
            PrimaryFlibe,
            "downcomer 3",
            &state.downcomer_3_temp_profile_degc,
        ),
        (PrimaryFlibe, "pipe 4", &state.pipe_4_temp_profile_degc),
        (PrimaryFlibe, "pipe 5", &state.pipe_5_temp_profile_degc),
        (
            PrimaryFlibe,
            "IHX shell side (component 6)",
            &state.ihx_shell_side_temp_profile_degc,
        ),
        (PrimaryFlibe, "pipe 7", &state.pipe_7_temp_profile_degc),
        (PrimaryFlibe, "pipe 8", &state.pipe_8_temp_profile_degc),
        (
            PrimaryFlibe,
            "primary pump 9",
            &state.pump_9_temp_profile_degc,
        ),
        (PrimaryFlibe, "pipe 10", &state.pipe_10_temp_profile_degc),
        (PrimaryFlibe, "pipe 11", &state.pipe_11_temp_profile_degc),
        // ── intermediate loop, HITEC ─────────────────────────────────────
        (
            IntermediateHitec,
            "IHX tube side (component 6)",
            &state.ihx_tube_side_temp_profile_degc,
        ),
        (
            IntermediateHitec,
            "pipe 12",
            &state.pipe_12_temp_profile_degc,
        ),
        (
            IntermediateHitec,
            "pipe 13",
            &state.pipe_13_temp_profile_degc,
        ),
        (
            IntermediateHitec,
            "steam generator shell (component 14)",
            &state.sg_shell_side_temp_profile_degc,
        ),
        (
            IntermediateHitec,
            "pipe 15",
            &state.pipe_15_temp_profile_degc,
        ),
        (
            IntermediateHitec,
            "intermediate pump 16",
            &state.pump_16_temp_profile_degc,
        ),
        (
            IntermediateHitec,
            "pipe 17",
            &state.pipe_17_temp_profile_degc,
        ),
    ]
}

/// Scan a thermal-hydraulics state for a salt node that has gone at or below
/// its salt's freeze threshold, and return the **coldest** such node.
///
/// Pure function of the previous timestep's temperature profiles — no locks,
/// no property calls, no `egui`. That is deliberate: the driver calls it
/// *before* entering the step whose property lookups would panic, and it is
/// directly testable headlessly.
///
/// Returns `None` when every salt node is above its threshold, which is the
/// normal case. Empty profiles (the state before the first timestep) are
/// skipped rather than treated as frozen.
///
/// The comparison is `<=`, not `<`: `tuas_boussinesq_solver`'s `range_check`
/// rejects temperatures below the bound, so sitting exactly on it is the last
/// safe state and one more step of cooling is a panic. Stopping there is the
/// conservative choice.
pub fn detect_salt_freeze(state: &FHRThermalHydraulicsState) -> Option<SaltFreezeEvent> {
    let mut coldest: Option<SaltFreezeEvent> = None;

    for (frozen_loop, location, profile) in labelled_profiles(state) {
        let threshold_degc = frozen_loop.freeze_threshold_degc();

        for &node_temperature_degc in profile {
            if node_temperature_degc > threshold_degc {
                continue;
            }
            let is_colder = coldest
                .as_ref()
                .is_none_or(|worst| node_temperature_degc < worst.coldest_temperature_degc);
            if is_colder {
                coldest = Some(SaltFreezeEvent {
                    frozen_loop,
                    location: location.to_string(),
                    coldest_temperature_degc: node_temperature_degc,
                    threshold_degc,
                });
            }
        }
    }

    coldest
}

/// Shared "has a salt loop frozen, and has the operator asked to melt it?"
/// handle, cloned between the physics thread (which records freezes and
/// consumes melt requests) and the GUI thread (which displays and requests).
///
/// Backed by an [`Arc`], so [`Clone`] just bumps a refcount. Deliberately
/// modelled on the engine scaffold's
/// [`ThreadHealth`](outram_park_digital_twin_engine::app_scaffold::ThreadHealth)
/// — same atomic-flag-plus-`RwLock`-payload shape, same poison-safe reads —
/// because this is the *graceful* sibling of that crash path and should be
/// recognisable as such. It is a separate type rather than an addition to
/// `ThreadHealth` because a freeze is recoverable and a panic is not: mixing
/// them would let a "resume" button appear after a genuine crash.
#[derive(Clone, Debug)]
pub struct SaltFreezeMonitor {
    inner: Arc<SaltFreezeMonitorInner>,
}

#[derive(Debug)]
struct SaltFreezeMonitorInner {
    /// Fast path the GUI polls every frame. `Release`/`Acquire` ordered
    /// against `event`, so a reader that observes `true` also sees the event
    /// written just before the flag was set.
    frozen: AtomicBool,
    /// The freeze currently being reported. Poison-safe on read.
    event: RwLock<Option<SaltFreezeEvent>>,
    /// Set by the GUI when the operator presses "melt the salt"; consumed by
    /// the physics thread.
    melt_requested: AtomicBool,
}

impl SaltFreezeMonitor {
    /// A fresh handle with nothing frozen and no melt pending.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(SaltFreezeMonitorInner {
                frozen: AtomicBool::new(false),
                event: RwLock::new(None),
                melt_requested: AtomicBool::new(false),
            }),
        }
    }

    /// Is a salt loop currently frozen and the simulation paused? A single
    /// atomic load — safe to call every GUI frame.
    pub fn is_frozen(&self) -> bool {
        self.inner.frozen.load(Ordering::Acquire)
    }

    /// The freeze being reported, or `None` if the plant is healthy.
    /// Poison-safe: never panics.
    pub fn event(&self) -> Option<SaltFreezeEvent> {
        if !self.is_frozen() {
            return None;
        }
        match self.inner.event.read() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    /// Record a freeze and pause. Called from the physics thread the moment
    /// [`detect_salt_freeze`] returns `Some`, *before* the step that would
    /// panic. Re-recording while already frozen keeps the first event, so the
    /// operator sees the root cause rather than a later, colder consequence.
    pub fn record(&self, event: SaltFreezeEvent) {
        {
            let mut guard = match self.inner.event.write() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            if guard.is_none() {
                *guard = Some(event);
            }
        }
        self.inner.frozen.store(true, Ordering::Release);
    }

    /// GUI side: the operator pressed "melt the salt and resume".
    ///
    /// This only *requests*; the physics thread performs the restore, so the
    /// components are only ever mutated by the thread that owns them.
    pub fn request_melt(&self) {
        self.inner.melt_requested.store(true, Ordering::Release);
    }

    /// Physics side: has a melt been requested? Consumes the request and
    /// clears the frozen state, returning `true` exactly once per request.
    ///
    /// The caller **must** actually restore the loop when this returns `true`,
    /// otherwise the guard will simply re-trip on the next iteration (which is
    /// safe, just confusing).
    pub fn take_melt_request(&self) -> bool {
        if !self.inner.melt_requested.swap(false, Ordering::AcqRel) {
            return false;
        }
        {
            let mut guard = match self.inner.event.write() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            *guard = None;
        }
        self.inner.frozen.store(false, Ordering::Release);
        true
    }
}

impl Default for SaltFreezeMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Overwrite the frozen loop's temperature profiles in `state` with the
/// melt-restore temperature, so [`detect_salt_freeze`] does not immediately
/// re-trip on the stale, frozen numbers before the next real timestep
/// produces fresh ones.
///
/// Call this from the driver **together with** rebuilding that loop's
/// components at [`MELT_RESTORE_TEMPERATURE_DEGC`]; on its own it would
/// falsify the reported state.
pub fn reset_profiles_after_melt(state: &mut FHRThermalHydraulicsState, frozen_loop: FrozenLoop) {
    let restored = |profile: &mut Vec<f64>| {
        if profile.is_empty() {
            profile.push(MELT_RESTORE_TEMPERATURE_DEGC);
        } else {
            profile.fill(MELT_RESTORE_TEMPERATURE_DEGC);
        }
    };

    match frozen_loop {
        FrozenLoop::PrimaryFlibe => {
            restored(&mut state.reactor_temp_profile_degc);
            restored(&mut state.downcomer_2_temp_profile_degc);
            restored(&mut state.downcomer_3_temp_profile_degc);
            restored(&mut state.pipe_4_temp_profile_degc);
            restored(&mut state.pipe_5_temp_profile_degc);
            restored(&mut state.ihx_shell_side_temp_profile_degc);
            restored(&mut state.pipe_7_temp_profile_degc);
            restored(&mut state.pipe_8_temp_profile_degc);
            restored(&mut state.pump_9_temp_profile_degc);
            restored(&mut state.pipe_10_temp_profile_degc);
            restored(&mut state.pipe_11_temp_profile_degc);
            // The IHX is one object holding both sides, so rebuilding it for
            // a primary-loop melt necessarily resets its HITEC tube side too.
            restored(&mut state.ihx_tube_side_temp_profile_degc);
        }
        FrozenLoop::IntermediateHitec => {
            restored(&mut state.ihx_tube_side_temp_profile_degc);
            restored(&mut state.pipe_12_temp_profile_degc);
            restored(&mut state.pipe_13_temp_profile_degc);
            restored(&mut state.sg_shell_side_temp_profile_degc);
            restored(&mut state.pipe_15_temp_profile_degc);
            restored(&mut state.pump_16_temp_profile_degc);
            restored(&mut state.pipe_17_temp_profile_degc);
            // Same coupling in the other direction.
            restored(&mut state.ihx_shell_side_temp_profile_degc);
        }
    }
}

/// If a salt loop is frozen, draw an unmissable modal naming the salt, the
/// place and the temperature, offering a melt, and return `true`; otherwise
/// draw nothing and return `false`.
///
/// An [`egui::Modal`]: centered, backdrop-dimmed, input-blocking, so a frozen
/// plant cannot be mistaken for a running one. Follows the same call pattern
/// and visual shape as the engine scaffold's
/// `show_crash_modal_if_crashed`, and is drawn immediately after it — a
/// genuine panic still wins, because that one is unrecoverable.
///
/// Pressing the melt button only calls [`SaltFreezeMonitor::request_melt`];
/// the physics thread does the restoring. The modal stays up until that
/// thread has actually resumed.
///
/// The text says outright that the melt is not physical. Keep it that way.
pub fn show_salt_freeze_modal(ctx: &egui::Context, monitor: &SaltFreezeMonitor) -> bool {
    let Some(event) = monitor.event() else {
        return false;
    };

    egui::Modal::new(egui::Id::new("fhr_sim_v2_salt_freeze_modal")).show(ctx, |ui| {
        ui.set_max_width(560.0);
        ui.vertical_centered(|ui| {
            ui.heading("\u{2744} Salt freeze -- simulation paused");
        });
        ui.add_space(8.0);

        ui.strong(event.headline());
        ui.add_space(6.0);
        ui.label(format!(
            "The {} fell {:.1} K below the temperature its salt stays liquid at, so \
             the simulation has been paused at that state rather than allowed to \
             carry on with a solid coolant.",
            event.frozen_loop.loop_name(),
            event.undershoot_kelvin(),
        ));

        ui.add_space(10.0);
        ui.separator();
        ui.label(
            "A frozen coolant loop is one of the defining operational hazards of a \
             molten-salt reactor. Circulation stops, decay heat has nowhere to go, \
             and the plug does not clear itself. Keeping every part of the loop \
             above its salt's melting point -- including the coldest corner, at the \
             heat-exchanger outlets -- is a continuous operating requirement, not a \
             trip setpoint.",
        );

        ui.add_space(10.0);
        ui.separator();
        ui.label(
            egui::RichText::new(
                "The melt below is a teaching shortcut, not physics. A real salt loop \
                 is thawed with trace heating over hours to days, and may not be \
                 recoverable at all -- re-freeze plugs can block the very flow paths \
                 the heating has to reach. None of that is modelled. Pressing the \
                 button teleports the loop back to the simulator's 500 degC \
                 cold-start condition.",
            )
            .italics(),
        );

        ui.add_space(12.0);
        ui.vertical_centered(|ui| {
            if ui
                .button(format!(
                    "Melt the {} and resume (restores it to {:.0} degC)",
                    event.frozen_loop.loop_name(),
                    MELT_RESTORE_TEMPERATURE_DEGC,
                ))
                .clicked()
            {
                monitor.request_melt();
            }
        });

        ui.add_space(6.0);
        egui::CollapsingHeader::new("Technical details")
            .default_open(false)
            .show(ui, |ui| {
                ui.monospace(format!(
                    "salt            : {}\n\
                     loop            : {}\n\
                     location        : {}\n\
                     measured        : {:.3} degC\n\
                     freeze threshold: {:.3} degC (LiquidMaterial::min_temperature)\n\
                     undershoot      : {:.3} K",
                    event.frozen_loop.salt_name(),
                    event.frozen_loop.loop_name(),
                    event.location,
                    event.coldest_temperature_degc,
                    event.threshold_degc,
                    event.undershoot_kelvin(),
                ));
            });
    });

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::thermal_hydraulics_backend::secondary_loop::steam_generator_duty::SteamGeneratorDutyLimit;
    use uom::si::power::watt;
    use uom::si::thermodynamic_temperature::kelvin;
    use uom::ConstZero;

    /// A healthy state: every salt node at the simulator's 500 degC
    /// cold-start temperature.
    fn healthy_state() -> FHRThermalHydraulicsState {
        let hot = vec![500.0, 500.0];
        FHRThermalHydraulicsState {
            reactor_branch_flow: MassRate::ZERO,
            downcomer_branch_1_flow: MassRate::ZERO,
            downcomer_branch_2_flow: MassRate::ZERO,
            intermediate_heat_exchanger_branch_flow: MassRate::ZERO,
            intrmd_loop_ihx_br_flow: MassRate::ZERO,
            intrmd_loop_steam_gen_br_flow: MassRate::ZERO,
            simulation_time: Time::ZERO,
            reactor_temp_profile_degc: hot.clone(),
            ihx_shell_side_temp_profile_degc: hot.clone(),
            ihx_tube_side_temp_profile_degc: hot.clone(),
            sg_shell_side_temp_profile_degc: hot.clone(),
            pipe_4_temp_profile_degc: hot.clone(),
            pipe_5_temp_profile_degc: hot.clone(),
            pipe_7_temp_profile_degc: hot.clone(),
            pipe_8_temp_profile_degc: hot.clone(),
            pump_9_temp_profile_degc: hot.clone(),
            pipe_10_temp_profile_degc: hot.clone(),
            pipe_11_temp_profile_degc: hot.clone(),
            pipe_12_temp_profile_degc: hot.clone(),
            pipe_13_temp_profile_degc: hot.clone(),
            pipe_15_temp_profile_degc: hot.clone(),
            pump_16_temp_profile_degc: hot.clone(),
            pipe_17_temp_profile_degc: hot.clone(),
            downcomer_2_temp_profile_degc: hot.clone(),
            downcomer_3_temp_profile_degc: hot,
            heat_added_to_steam_generator_shell_side: Energy::ZERO,
            steam_generator_effectiveness: 0.0,
            steam_generator_maximum_duty: Power::new::<watt>(0.0),
            steam_generator_duty_limit: SteamGeneratorDutyLimit::NoDrivingTemperatureDifference,
        }
    }

    /// **V&V — the thresholds are the library's, not this module's.**
    ///
    /// ## Methodology
    ///
    /// Asserts that each loop's freeze threshold is exactly
    /// `LiquidMaterial::min_temperature()` from `tuas_boussinesq_solver` — the
    /// same bound the `range_check` that would panic the physics thread uses —
    /// and pins the two numbers in kelvin so a silent change to the upstream
    /// database is caught here rather than discovered as a mysterious change
    /// in simulator behaviour.
    ///
    /// ## Results — measured 2026-08-12
    ///
    /// - FLiBe: 732.200 K = **459.050 degC**, matching `min_temp_flibe()` in
    ///   `tuas_boussinesq_solver`'s `flibe.rs`, which documents 732.2 K as the
    ///   FLiBe melting point (Romatoski & Hu 2017; Sohal et al. 2010).
    /// - HITEC: 440.000 K = **166.850 degC**, matching `min_temp_hitec()` in
    ///   `hitec_nitrate_salt.rs`, the lower bound of the Du et al. (2018)
    ///   correlations.
    ///
    /// Both agree with the rounded warnings already shown in the simulator's
    /// side panel (470 degC and 170 degC).
    ///
    /// ## Interpretation
    ///
    /// Detection fires exactly where the physics stops being valid, so there
    /// is no window in which the guard thinks the plant is fine but a property
    /// call would panic.
    #[test]
    fn freeze_thresholds_come_from_the_tuas_liquid_database() {
        let flibe_k = FrozenLoop::PrimaryFlibe.freeze_threshold().get::<kelvin>();
        let hitec_k = FrozenLoop::IntermediateHitec
            .freeze_threshold()
            .get::<kelvin>();

        assert!(
            (flibe_k - 732.2).abs() < 1.0e-9,
            "FLiBe threshold {flibe_k} K should be min_temp_flibe() = 732.2 K"
        );
        assert!(
            (hitec_k - 440.0).abs() < 1.0e-9,
            "HITEC threshold {hitec_k} K should be min_temp_hitec() = 440.0 K"
        );
        assert!(
            (FrozenLoop::PrimaryFlibe.freeze_threshold_degc() - 459.05).abs() < 1.0e-3,
            "FLiBe melts at 459.05 degC"
        );
        assert!(
            (FrozenLoop::IntermediateHitec.freeze_threshold_degc() - 166.85).abs() < 1.0e-3,
            "HITEC correlation floor is 166.85 degC"
        );
        assert!(
            MELT_RESTORE_TEMPERATURE_DEGC > FrozenLoop::PrimaryFlibe.freeze_threshold_degc(),
            "the melt-restore temperature must clear the FLiBe melting point"
        );
    }

    /// **V&V — a healthy plant is never reported as frozen.**
    ///
    /// ## Methodology
    ///
    /// Every salt node at the 500 degC cold-start temperature, plus the
    /// empty-profile state that exists before the first timestep. Neither may
    /// produce an event; a guard that fires spuriously would pause a working
    /// simulation.
    ///
    /// ## Results — measured 2026-08-12
    ///
    /// `detect_salt_freeze` returns `None` for both.
    #[test]
    fn a_healthy_plant_is_not_reported_as_frozen() {
        assert!(detect_salt_freeze(&healthy_state()).is_none());

        let mut before_first_timestep = healthy_state();
        before_first_timestep.reactor_temp_profile_degc = vec![];
        before_first_timestep.sg_shell_side_temp_profile_degc = vec![];
        assert!(
            detect_salt_freeze(&before_first_timestep).is_none(),
            "empty profiles are 'no data yet', not 'frozen'"
        );
    }

    /// **V&V — a frozen loop is detected, named and located, without a panic.**
    ///
    /// ## Methodology
    ///
    /// This is the headless version of the failure the guard exists for. Two
    /// scenarios are driven, each by pushing one component below its salt's
    /// threshold while the rest of the plant stays hot:
    ///
    /// 1. **HITEC in the steam-generator shell.** The steam generator is the
    ///    coldest point of the intermediate loop and the realistic place for
    ///    an over-cooled loop to freeze first. Set to 150.0 degC against a
    ///    166.85 degC threshold.
    /// 2. **FLiBe in the reactor vessel.** Set to 452.3 degC against a
    ///    459.05 degC threshold — the case quoted in the maintainer's brief.
    ///
    /// In each case the test asserts the returned event names the right salt,
    /// the right component and the measured temperature, that
    /// [`SaltFreezeEvent::headline`] reads as a lesson rather than an error
    /// code, and that the whole path runs on a headless thread with no
    /// display and no panic.
    ///
    /// ## Results — measured 2026-08-12
    ///
    /// - Scenario 1 headline: `HITEC froze in the steam generator shell
    ///   (component 14) at 150.0 degC (freezes at 166.9 degC)`; undershoot
    ///   16.850 K.
    /// - Scenario 2 headline: `FLiBe froze in the reactor vessel (component 1)
    ///   at 452.3 degC (freezes at 459.1 degC)`; undershoot 6.750 K.
    ///
    /// No panic, no display, no property call.
    ///
    /// ## Interpretation
    ///
    /// The guard reports the physically meaningful fact — this salt, in this
    /// component, at this temperature, against this melting point — from state
    /// alone, so it can run *before* the timestep whose property lookups would
    /// have crashed the thread.
    #[test]
    fn a_frozen_loop_is_detected_named_and_located() {
        // ── HITEC in the steam-generator shell ──────────────────────────
        let mut state = healthy_state();
        state.sg_shell_side_temp_profile_degc = vec![500.0, 150.0];

        let event = detect_salt_freeze(&state).expect("a HITEC freeze must be detected");
        assert_eq!(event.frozen_loop, FrozenLoop::IntermediateHitec);
        assert_eq!(event.frozen_loop.salt_name(), "HITEC");
        assert_eq!(event.location, "steam generator shell (component 14)");
        assert!((event.coldest_temperature_degc - 150.0).abs() < 1.0e-9);
        assert!((event.undershoot_kelvin() - 16.85).abs() < 1.0e-3);
        assert_eq!(
            event.headline(),
            "HITEC froze in the steam generator shell (component 14) at 150.0 degC \
             (freezes at 166.9 degC)"
        );

        // ── FLiBe in the reactor vessel ─────────────────────────────────
        let mut state = healthy_state();
        state.reactor_temp_profile_degc = vec![500.0, 452.3, 500.0];

        let event = detect_salt_freeze(&state).expect("a FLiBe freeze must be detected");
        assert_eq!(event.frozen_loop, FrozenLoop::PrimaryFlibe);
        assert_eq!(event.location, "reactor vessel (component 1)");
        assert!((event.coldest_temperature_degc - 452.3).abs() < 1.0e-9);
        assert!((event.undershoot_kelvin() - 6.75).abs() < 1.0e-3);
        assert_eq!(
            event.headline(),
            "FLiBe froze in the reactor vessel (component 1) at 452.3 degC \
             (freezes at 459.1 degC)"
        );
    }

    /// **V&V — the coldest node wins, across both salts at once.**
    ///
    /// ## Methodology
    ///
    /// A state in which *both* loops are frozen: FLiBe in pipe 10 at
    /// 400.0 degC (59.05 K under) and HITEC in pipe 13 at 100.0 degC (66.85 K
    /// under). The guard must report the numerically coldest node, so the
    /// operator is shown the worst of the plant rather than whichever loop
    /// happened to be scanned first.
    ///
    /// ## Results — measured 2026-08-12
    ///
    /// Reported: HITEC, pipe 13, 100.0 degC — the coldest node, correctly
    /// preferred over the FLiBe node despite FLiBe being scanned first.
    #[test]
    fn the_coldest_node_is_reported_when_both_loops_freeze() {
        let mut state = healthy_state();
        state.pipe_10_temp_profile_degc = vec![400.0];
        state.pipe_13_temp_profile_degc = vec![100.0];

        let event = detect_salt_freeze(&state).expect("a freeze must be detected");
        assert_eq!(event.frozen_loop, FrozenLoop::IntermediateHitec);
        assert_eq!(event.location, "pipe 13");
        assert!((event.coldest_temperature_degc - 100.0).abs() < 1.0e-9);
    }

    /// **V&V — the pause/melt handshake, driven headlessly on real threads.**
    ///
    /// ## Methodology
    ///
    /// Reproduces the full runtime sequence with no display: a worker thread
    /// stands in for the physics loop, checking [`detect_salt_freeze`] at the
    /// top of each iteration exactly as the driver does. A frozen state is
    /// installed, and the test asserts that
    ///
    /// 1. the worker **records the freeze and stops advancing** (its step
    ///    counter is frozen too) instead of panicking;
    /// 2. the GUI-side [`SaltFreezeMonitor::event`] reports the right salt,
    ///    location and temperature while paused;
    /// 3. after [`SaltFreezeMonitor::request_melt`], the worker consumes the
    ///    request exactly once, restores the loop's profiles via
    ///    [`reset_profiles_after_melt`], and **resumes stepping**;
    /// 4. the monitor is clear afterwards, so the modal disappears.
    ///
    /// ## Results — measured 2026-08-12
    ///
    /// The worker paused with its step counter held constant across a 300 ms
    /// observation window; the event read from the GUI side was `HITEC froze
    /// in the steam generator shell (component 14) at 150.0 degC (freezes at
    /// 166.9 degC)`; after the melt request the worker resumed and the
    /// restored profiles all read 500.0 degC; `is_frozen()` returned `false`
    /// and `event()` returned `None`. The worker thread joined cleanly — it
    /// never panicked.
    ///
    /// ## Interpretation
    ///
    /// The freeze path is a genuine pause-and-resume, not a crash dressed up
    /// as one: no unwinding happens, the plant state is held intact while
    /// paused, and the restore is performed by the thread that owns the
    /// components.
    #[test]
    fn the_simulation_pauses_on_freeze_and_resumes_after_a_melt() {
        use std::sync::atomic::AtomicUsize;
        use std::sync::Mutex;
        use std::thread;
        use std::time::Duration;

        let monitor = SaltFreezeMonitor::new();
        let steps = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));

        // The plant state the stand-in physics thread reads and writes.
        let mut frozen_state = healthy_state();
        frozen_state.sg_shell_side_temp_profile_degc = vec![500.0, 150.0];
        let shared_state = Arc::new(Mutex::new(frozen_state));

        let worker = {
            let monitor = monitor.clone();
            let steps = steps.clone();
            let stop = stop.clone();
            let shared_state = shared_state.clone();
            thread::spawn(move || {
                while !stop.load(Ordering::Acquire) {
                    // ── exactly the driver's ordering ───────────────────
                    if let Some(event) = detect_salt_freeze(&shared_state.lock().unwrap()) {
                        monitor.record(event);
                    }
                    if monitor.is_frozen() {
                        if monitor.take_melt_request() {
                            let mut state = shared_state.lock().unwrap();
                            reset_profiles_after_melt(&mut state, FrozenLoop::IntermediateHitec);
                        } else {
                            thread::sleep(Duration::from_millis(FREEZE_PAUSE_POLL_INTERVAL_MS));
                            continue;
                        }
                    }
                    // "advancing the physics"
                    steps.fetch_add(1, Ordering::AcqRel);
                    thread::sleep(Duration::from_millis(5));
                }
            })
        };

        // (1) it pauses rather than advancing
        thread::sleep(Duration::from_millis(200));
        assert!(monitor.is_frozen(), "the guard should have tripped");
        let steps_while_paused = steps.load(Ordering::Acquire);
        thread::sleep(Duration::from_millis(300));
        assert_eq!(
            steps.load(Ordering::Acquire),
            steps_while_paused,
            "a frozen simulation must not keep advancing"
        );

        // (2) the GUI side sees a useful report
        let event = monitor.event().expect("the GUI must see the freeze");
        assert_eq!(event.frozen_loop, FrozenLoop::IntermediateHitec);
        assert_eq!(event.location, "steam generator shell (component 14)");
        assert!((event.coldest_temperature_degc - 150.0).abs() < 1.0e-9);

        // (3) the melt resumes it
        monitor.request_melt();
        let mut resumed = false;
        for _ in 0..100 {
            thread::sleep(Duration::from_millis(20));
            if steps.load(Ordering::Acquire) > steps_while_paused + 2 {
                resumed = true;
                break;
            }
        }
        assert!(resumed, "the simulation should resume after a melt");

        // (4) the modal goes away and the loop really was restored
        assert!(!monitor.is_frozen());
        assert!(monitor.event().is_none());
        for &node in &shared_state.lock().unwrap().sg_shell_side_temp_profile_degc {
            assert!(
                (node - MELT_RESTORE_TEMPERATURE_DEGC).abs() < 1.0e-9,
                "the melted loop should read {MELT_RESTORE_TEMPERATURE_DEGC} degC, got {node}"
            );
        }

        stop.store(true, Ordering::Release);
        worker
            .join()
            .expect("the physics stand-in must never panic");
    }

    /// **V&V — a melt request is consumed exactly once.**
    ///
    /// ## Methodology
    ///
    /// A double press of the melt button, and a `take_melt_request` with no
    /// press behind it. The physics thread performs a real state restore on
    /// each `true`, so a request that could be taken twice would restore twice
    /// and a spurious `true` would restore a healthy loop.
    ///
    /// ## Results — measured 2026-08-12
    ///
    /// With no request pending, `take_melt_request()` returns `false`. After
    /// one or more `request_melt()` calls it returns `true` once and `false`
    /// thereafter, and the frozen flag and event are cleared on that one
    /// `true`.
    #[test]
    fn a_melt_request_is_consumed_exactly_once() {
        let monitor = SaltFreezeMonitor::new();
        assert!(!monitor.take_melt_request(), "nothing requested yet");

        monitor.record(SaltFreezeEvent {
            frozen_loop: FrozenLoop::PrimaryFlibe,
            location: "pipe 5".to_string(),
            coldest_temperature_degc: 450.0,
            threshold_degc: FrozenLoop::PrimaryFlibe.freeze_threshold_degc(),
        });
        assert!(monitor.is_frozen());

        monitor.request_melt();
        monitor.request_melt();
        assert!(monitor.take_melt_request(), "the first take consumes it");
        assert!(!monitor.take_melt_request(), "and only the first");
        assert!(!monitor.is_frozen());
        assert!(monitor.event().is_none());
    }

    /// **V&V — the first freeze is the one reported.**
    ///
    /// ## Methodology
    ///
    /// Two freezes recorded back to back. The plant keeps cooling while
    /// paused only if the driver keeps stepping (it does not), but the driver
    /// re-checks every poll iteration, so `record` is called repeatedly with
    /// the same or a later event. The operator must keep seeing the root
    /// cause.
    ///
    /// ## Results — measured 2026-08-12
    ///
    /// The second `record` is ignored; `event()` still reports the first
    /// (`pipe 5`, 450.0 degC).
    #[test]
    fn re_recording_while_frozen_keeps_the_first_event() {
        let monitor = SaltFreezeMonitor::new();
        let threshold = FrozenLoop::PrimaryFlibe.freeze_threshold_degc();

        monitor.record(SaltFreezeEvent {
            frozen_loop: FrozenLoop::PrimaryFlibe,
            location: "pipe 5".to_string(),
            coldest_temperature_degc: 450.0,
            threshold_degc: threshold,
        });
        monitor.record(SaltFreezeEvent {
            frozen_loop: FrozenLoop::PrimaryFlibe,
            location: "pipe 8".to_string(),
            coldest_temperature_degc: 300.0,
            threshold_degc: threshold,
        });

        let event = monitor.event().expect("frozen");
        assert_eq!(event.location, "pipe 5");
        assert!((event.coldest_temperature_degc - 450.0).abs() < 1.0e-9);
    }
}
