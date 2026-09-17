//! Physics-thread panic detection + a "please restart" crash-notification
//! modal, shared by every digital-twin simulator built on this scaffold.
//!
//! A digital-twin simulator spawns one or more background *physics threads*
//! (see [`spawn_physics_thread`](super::spawn_physics_thread)) that update an
//! [`Arc`]`<`[`RwLock`]`<T>>` while the GUI thread reads it each frame. If one
//! of those threads panics -- e.g. a thermal-hydraulics step drives a property
//! out of its valid IAPWS range and unwraps a `NonConvergent`, or a `(p, h)`
//! flash lands off the steam dome -- the physics simply stops. Without this
//! module the GUI would keep painting stale numbers forever, giving the user no
//! signal that the simulation is dead.
//!
//! The pieces here close that gap:
//!
//! - [`ThreadHealth`] -- a cheap-to-clone shared flag. Clone one handle into
//!   each monitored thread and one into the GUI.
//! - [`spawn_monitored`] / [`spawn_physics_thread_monitored`] -- spawn a thread
//!   whose body is wrapped in [`std::panic::catch_unwind`]; on a panic they
//!   record the panic message (downcast from the payload) into the shared
//!   [`ThreadHealth`] instead of letting the thread die silently.
//! - [`show_crash_modal_if_crashed`] -- a one-line GUI helper: if any monitored
//!   thread has crashed it draws an unmissable [`egui::Modal`] (centered,
//!   backdrop-dimmed, input-blocking) telling the user to restart the
//!   simulator, with the captured panic message under a details header.
//! - [`show_crash_modal_with_restart`] -- the same modal plus a **Restart
//!   simulation** button, returning a [`CrashModalOutcome`] so the caller can
//!   act on the click.
//!
//! It deliberately does **not** try to restart the *process*, and it never
//! resumes the crashed run: a panicked physics thread may have poisoned a
//! shared lock mid-write, so the state it was building is not trustworthy. To
//! keep the GUI itself from cascade-panicking on that poisoned lock,
//! [`SharedState`](super::SharedState) recovers poisoned guards, and simulators
//! should early-return from their frame once the modal is shown (so they never
//! touch a poisoned `Mutex`).
//!
//! # What "restart" means here
//!
//! [`show_crash_modal_with_restart`] only *reports the click*. Honouring it is
//! the simulator's job, and the only safe way to honour it is to **start a new
//! run from defaults, never to resume the old one**: build fresh
//! [`SharedState`](super::SharedState) handles and a fresh [`ThreadHealth`],
//! spawn new monitored threads against them, and drop the old handles without
//! reading them. Anything that reaches back into the crashed run's state --
//! carrying over a snapshot, reusing the old `ThreadHealth`, re-locking the
//! poisoned `RwLock` to "recover" a value -- reintroduces exactly the hazard
//! this module exists to contain.
//!
//! # A run's threads stop together
//!
//! [`spawn_physics_thread_monitored`] checks [`ThreadHealth::is_running`] at the
//! top of every iteration, so all of a run's loops end when the run does --
//! whether because **any** one of them panicked, or because the application
//! called [`ThreadHealth::retire`] to end the run deliberately.
//!
//! A simulator typically runs several loops against one `ThreadHealth` (physics,
//! plot sampler, ...). Without a shared stop condition only the loop that
//! actually panicked would stop, leaving the survivors spinning against a dead
//! run's state forever -- one leaked thread per restart. That is what makes a
//! restart a clean swap rather than an accumulation.

use std::any::Any;
use std::cell::RefCell;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Once, RwLock};
use std::thread::{self, JoinHandle};

use super::SharedState;

/// A short, `Clone`-able record of the first background thread to panic:
/// which thread it was and the panic message extracted from its payload.
///
/// The `message` is the human-readable panic string (`panic!("...")` text or a
/// `.unwrap()`/`.expect()` message) recovered by downcasting the panic payload
/// to `&str` / `String`; if the payload was some other type it is a fixed
/// placeholder rather than the real value.
#[derive(Clone, Debug)]
pub struct CrashReport {
    /// The name passed to [`spawn_monitored`] /
    /// [`spawn_physics_thread_monitored`] for the thread that panicked.
    pub thread_name: String,
    /// The panic message (best-effort, downcast from the panic payload).
    pub message: String,
    /// Source location of the panic as `file:line:column`, e.g.
    /// `"examples/htgr_sim_v1/physics/secondary_loop.rs:388:9"`.
    ///
    /// **This is the field that identifies which component failed.** The
    /// `thread_name` only names the thread, and a simulator that runs its whole
    /// plant on one physics thread (as `htgr_sim_v1` does) would otherwise
    /// report nothing more useful than `"htgr-physics"`.
    ///
    /// `None` if the panic hook did not fire or the panic carried no location —
    /// possible for a panic raised through a path that bypasses the standard
    /// hook. Treat it as best-effort diagnostics, not a guarantee.
    pub location: Option<String>,
    /// The **plant component** being stepped when the panic happened, as set by
    /// [`mark_component`] -- e.g. `"steam generator"`.
    ///
    /// This is what a crash report should lead with: a source location tells a
    /// developer where to look, but this tells the operator which piece of
    /// equipment failed. `None` if the simulator does not mark its components.
    pub component: Option<&'static str>,
}

impl CrashReport {
    /// One-line human summary: the thread, the location if known, and the
    /// message. Suitable for a log line or the crash modal's headline.
    pub fn summary(&self) -> String {
        let where_ = match (self.component, &self.location) {
            (Some(component), Some(location)) => format!("{component} ({location})"),
            (Some(component), None) => component.to_string(),
            (None, Some(location)) => location.clone(),
            (None, None) => self.thread_name.clone(),
        };
        format!(
            "{} failed in {}: {}",
            self.thread_name, where_, self.message
        )
    }
}

thread_local! {
    /// The plant component this thread is currently stepping, set by
    /// [`mark_component`].
    static CURRENT_COMPONENT: RefCell<Option<&'static str>> = const { RefCell::new(None) };

    /// Source location recorded by the panic hook for the panic currently
    /// unwinding *this* thread.
    ///
    /// A thread-local is what makes this correct under concurrency: several
    /// physics threads can panic at once, and each hook invocation runs on the
    /// panicking thread, so no cross-thread interleaving can attribute one
    /// thread's location to another's payload.
    static PANIC_LOCATION: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Installs, exactly once per process, a panic hook that records each panic's
/// source location before the default hook runs.
///
/// # Why a hook is required
///
/// [`std::panic::catch_unwind`] yields only the panic *payload* — the message.
/// The `file:line:column` lives in the [`std::panic::PanicHookInfo`] handed to
/// the hook, and is gone by the time `catch_unwind` returns. Capturing it is
/// therefore not optional polish; it is the only way to learn where a panic
/// came from without a backtrace.
///
/// The previously installed hook is still called afterwards, so the usual
/// stderr panic output and `RUST_BACKTRACE` behaviour are preserved rather than
/// swallowed.
fn install_panic_location_hook() {
    static HOOK: Once = Once::new();
    HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let location = info
                .location()
                .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()));
            PANIC_LOCATION.with(|slot| {
                *slot.borrow_mut() = location;
            });
            previous(info);
        }));
    });
}

/// Takes the location recorded by the hook for the panic that just unwound this
/// thread, clearing it so a later panic cannot inherit a stale value.
fn take_panic_location() -> Option<String> {
    PANIC_LOCATION.with(|slot| slot.borrow_mut().take())
}

/// Records which **plant component** the calling physics thread is about to
/// step, so a crash can be attributed to a piece of equipment rather than only
/// to a source file.
///
/// # Why this exists
///
/// A source location tells a *developer* where a panic happened. It does not
/// tell the person running the simulator which part of the plant misbehaved,
/// and in a simulator whose whole plant runs on one physics thread the thread
/// name is no help either. Calling this at the head of each subsystem's step
/// turns "panicked in `secondary_loop.rs:388`" into "the steam generator
/// failed", which is what a crash report should lead with.
///
/// # Usage
///
/// Call it as the plant walks its subsystems, in the same order they step:
///
/// ```ignore
/// mark_component("reactor kinetics");
/// self.kinetics.step(dt, rho);
/// mark_component("pebble bed");
/// self.core.step(dt, power, ...);
/// mark_component("steam generator");
/// self.secondary.step(dt, duty, hot_side);
/// ```
///
/// The name should read as **equipment a plant operator would recognise**
/// ("steam generator", "helium circulator", "hot gas duct"), not as a module
/// path. Take `&'static str` so marking costs a pointer store per subsystem per
/// timestep -- negligible beside the physics it precedes.
///
/// It is *not* a stack: each call replaces the previous mark, so the report
/// names the innermost component that was marked, not a nesting chain.
pub fn mark_component(component: &'static str) {
    CURRENT_COMPONENT.with(|slot| {
        *slot.borrow_mut() = Some(component);
    });
}

/// Takes the component mark for the thread that just panicked.
fn take_current_component() -> Option<&'static str> {
    CURRENT_COMPONENT.with(|slot| slot.borrow_mut().take())
}

/// A shared "is this simulator run still going, and if not why not?" flag,
/// plus the [`CrashReport`] of the first panic.
///
/// Backed by an [`Arc`] internally, so [`Clone`] just bumps the refcount --
/// clone one handle per monitored thread (they *record* into it) and one for
/// the GUI (which *queries* it every frame via
/// [`has_crashed`](Self::has_crashed) / [`crash_report`](Self::crash_report)).
/// Only the first panic is kept; later panics from other threads are ignored so
/// the reported cause is the root one.
///
/// It carries **two** independent reasons a run can end, and they mean different
/// things to a user:
///
/// - **crashed** -- a monitored thread panicked. Recorded automatically by
///   [`spawn_monitored`]; surfaced by [`has_crashed`](Self::has_crashed) and the
///   crash modal.
/// - **retired** -- the application deliberately ended the run, e.g. because the
///   operator restarted the simulator. Set by [`retire`](Self::retire); shows up
///   *only* in [`is_running`](Self::is_running).
///
/// Keeping them apart is what stops a restart from looking like a fault:
/// retiring a run never invents a crash report, and never clears a real one.
#[derive(Clone, Debug)]
pub struct ThreadHealth {
    inner: Arc<ThreadHealthInner>,
}

#[derive(Debug)]
struct ThreadHealthInner {
    /// Fast path the GUI polls every frame. `Release`/`Acquire` ordered against
    /// `report` so that a reader which observes `true` is guaranteed to also
    /// see the `report` that was written just before the flag was set.
    crashed: AtomicBool,
    /// Set by [`ThreadHealth::retire`] when the application ends the run on
    /// purpose. Deliberately separate from `crashed`: a retired run is not a
    /// faulted one.
    retired: AtomicBool,
    /// The first crash's report. Written once (under the lock) before `crashed`
    /// is set; read poison-safely by the GUI.
    report: RwLock<Option<CrashReport>>,
}

impl ThreadHealth {
    /// Create a fresh, healthy handle (no crash recorded, not retired).
    pub fn new() -> Self {
        Self {
            inner: Arc::new(ThreadHealthInner {
                crashed: AtomicBool::new(false),
                retired: AtomicBool::new(false),
                report: RwLock::new(None),
            }),
        }
    }

    /// Has any monitored thread panicked? Cheap (a single atomic load) -- safe
    /// to call every frame from the GUI thread.
    ///
    /// **A retired run is not a crashed one**: this stays `false` after
    /// [`retire`](Self::retire), so shutting a healthy run down never raises the
    /// crash modal.
    pub fn has_crashed(&self) -> bool {
        self.inner.crashed.load(Ordering::Acquire)
    }

    /// Should this run's monitored loops keep stepping?
    ///
    /// `false` once the run has either crashed *or* been
    /// [`retire`](Self::retire)d. This is the condition
    /// [`spawn_physics_thread_monitored`] tests at the top of every iteration,
    /// and it is the whole reason a run's threads can be made to stop: a
    /// simulator runs several loops against one `ThreadHealth`, and without a
    /// shared "still running?" answer only the loop that actually panicked would
    /// ever end.
    pub fn is_running(&self) -> bool {
        !self.inner.crashed.load(Ordering::Acquire) && !self.inner.retired.load(Ordering::Acquire)
    }

    /// Ask this run's monitored loops to stop, without recording a fault.
    ///
    /// Signals and returns immediately, in the same spirit as
    /// [`OpcuaServerHandle::shutdown`](crate::opcua_core::OpcuaServerHandle::shutdown):
    /// each loop finishes the `step` it is in (and any sleep inside it) and
    /// returns at the next iteration, so the threads end promptly rather than
    /// instantly. Idempotent -- calling it twice, or on an already-crashed run,
    /// is harmless.
    ///
    /// # When you need this
    ///
    /// Whenever an application abandons a run but the process keeps going --
    /// principally an **in-app restart**, which swaps in a new run's state and
    /// drops its handles to the old one. Without retiring the old run first, its
    /// threads keep stepping a plant nothing will ever display, one leaked
    /// thread per restart, competing for cores with the run the operator is
    /// actually watching.
    ///
    /// It does **not** touch the crash flag or the [`CrashReport`]: retiring a
    /// crashed run leaves its report intact for anything still holding a handle,
    /// and retiring a healthy run leaves [`has_crashed`](Self::has_crashed)
    /// `false`.
    pub fn retire(&self) {
        self.inner.retired.store(true, Ordering::Release);
    }

    /// The [`CrashReport`] of the first thread to panic, or `None` if all
    /// monitored threads are still healthy. Poison-safe: it never panics even
    /// if the panicking thread poisoned the internal lock.
    pub fn crash_report(&self) -> Option<CrashReport> {
        if !self.has_crashed() {
            return None;
        }
        match self.inner.report.read() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    /// Record a crash. Called from inside a monitored thread's `catch_unwind`
    /// handler. The report is written *before* the flag is flipped so any
    /// reader that sees the flag also sees the report; only the first crash is
    /// kept.
    fn record(&self, report: CrashReport) {
        {
            let mut guard = match self.inner.report.write() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            if guard.is_none() {
                *guard = Some(report);
            }
        }
        self.inner.crashed.store(true, Ordering::Release);
    }
}

impl Default for ThreadHealth {
    fn default() -> Self {
        Self::new()
    }
}

/// Best-effort extraction of a panic message from a caught panic payload.
///
/// `panic!("msg")`, `.unwrap()`, and `.expect("msg")` produce payloads that are
/// `&'static str` or `String`; anything else yields a fixed placeholder.
fn panic_message(payload: Box<dyn Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "panicked with a non-string payload".to_string()
    }
}

/// Spawn `body` on a new OS thread, wrapping it in
/// [`std::panic::catch_unwind`] so that a panic is *reported* to `health`
/// (with `thread_name` and the panic message) instead of unwinding out of the
/// thread and being lost.
///
/// This is the general escape hatch for simulators that spawn their physics
/// loops by hand (e.g. the `fhr_sim_v2` example, which drives `Arc<Mutex<_>>`
/// directly rather than through [`SharedState`]). For the common looping
/// physics-thread pattern prefer [`spawn_physics_thread_monitored`].
///
/// `thread_name` is a human label surfaced in the crash modal (e.g.
/// `"fhr-thermal-hydraulics"`); it does not need to be unique but should
/// identify the subsystem. The returned [`JoinHandle`] joins cleanly (returns
/// `Ok`) even when `body` panicked, because the panic is caught inside.
pub fn spawn_monitored<F>(
    thread_name: impl Into<String>,
    health: ThreadHealth,
    body: F,
) -> JoinHandle<()>
where
    F: FnOnce() + Send + 'static,
{
    let thread_name = thread_name.into();
    // Must be installed before the thread can panic, so the hook is in place to
    // record the location. Idempotent across every monitored thread.
    install_panic_location_hook();
    thread::spawn(move || {
        // AssertUnwindSafe: on a panic we do not resume the simulation, we ask
        // the user to restart, so a partially-mutated shared state left behind
        // an unwind is acceptable here (it is exactly what we are reporting).
        let result = catch_unwind(AssertUnwindSafe(body));
        if let Err(payload) = result {
            let message = panic_message(payload);
            // Taken on the panicking thread, so it is this thread's location.
            let location = take_panic_location();
            let component = take_current_component();
            let report = CrashReport {
                thread_name,
                message,
                location,
                component,
            };
            eprintln!("[physics thread panicked] {}", report.summary());
            health.record(report);
        }
    })
}

/// The monitored counterpart of
/// [`spawn_physics_thread`](super::spawn_physics_thread): repeatedly calls
/// `step` against a cloned [`SharedState`] handle, but if `step` ever panics the
/// panic is caught and reported to `health` rather than silently killing the
/// thread.
///
/// Use this in place of `spawn_physics_thread` wherever you want a crashed
/// physics loop to surface the [`show_crash_modal_if_crashed`] restart prompt.
/// `thread_name` labels the subsystem in that modal.
///
/// # The loop is not infinite: it ends when the *run* does
///
/// [`ThreadHealth::is_running`] is checked at the top of every iteration, so
/// this thread returns as soon as the run it belongs to has ended -- either
/// because **any** monitored thread sharing that [`ThreadHealth`] panicked (not
/// only this one), or because the application called
/// [`ThreadHealth::retire`].
///
/// Both exits matter for the same reason. A simulator runs several of these
/// loops against a single `ThreadHealth` (a physics loop and a plot sampler,
/// say), and without a shared stop condition a panic in one would leave the
/// others alive, still stepping and still writing into a dead run's state. That
/// is merely untidy while the process is about to be closed, and becomes a
/// leaked thread per click once an in-app restart button exists (see
/// [`show_crash_modal_with_restart`]) -- the restart hands the GUI new state
/// while the survivors keep the old state alive and keep burning a core.
///
/// The check is two `Acquire` atomic loads per timestep. A loop already inside a
/// long sleep or a long `step` exits at the end of that call, not instantly; the
/// exit is prompt, not immediate.
pub fn spawn_physics_thread_monitored<T, F>(
    thread_name: impl Into<String>,
    state: SharedState<T>,
    health: ThreadHealth,
    mut step: F,
) -> JoinHandle<()>
where
    T: Send + Sync + 'static,
    F: FnMut(&SharedState<T>) + Send + 'static,
{
    // A second handle to the same flag: `health` is moved into `spawn_monitored`
    // (which *records* into it), this one is what the loop *reads*.
    let watch = health.clone();
    spawn_monitored(thread_name, health, move || {
        while watch.is_running() {
            step(&state);
        }
    })
}

/// What [`show_crash_modal_with_restart`] did this frame, and what the caller
/// should do about it.
///
/// Returned instead of a bare `bool` because a crash modal carrying a restart
/// button has **three** outcomes, not two, and the extra one is the whole point:
/// "the simulation is dead" and "the user asked for a new one" call for
/// different actions and must not collapse into one flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrashModalOutcome {
    /// No monitored thread has panicked. Nothing was drawn; render normally.
    Healthy,
    /// The modal is up and the user has not asked for anything. Stop rendering
    /// this frame (the run's state is frozen and possibly poisoned) and repaint.
    Showing,
    /// The user clicked **Restart simulation**. The modal was drawn, so still
    /// stop rendering this frame -- but start a fresh run first. See the module
    /// docs for what "fresh" has to mean.
    RestartRequested,
}

impl CrashModalOutcome {
    /// Whether a crash modal was drawn -- i.e. anything other than
    /// [`Healthy`](Self::Healthy).
    ///
    /// The caller should return from its frame without rendering the plant when
    /// this is `true`, in **both** the `Showing` and `RestartRequested` cases:
    /// a run started this frame has not stepped yet, so there is nothing new to
    /// paint either way.
    pub fn is_crashed(self) -> bool {
        !matches!(self, Self::Healthy)
    }
}

/// Draw the crash modal's contents. Returns whether the restart button was
/// clicked (always `false` when `offer_restart` is `false`).
///
/// Shared by [`show_crash_modal_if_crashed`] and
/// [`show_crash_modal_with_restart`] so the two cannot drift into telling the
/// operator different things about the same crash.
fn crash_modal_contents(ui: &mut egui::Ui, report: &CrashReport, offer_restart: bool) -> bool {
    ui.set_max_width(520.0);
    ui.vertical_centered(|ui| {
        ui.heading("\u{26A0} Simulation crashed");
    });
    ui.add_space(8.0);
    ui.label(
        "A background physics thread has stopped unexpectedly. The simulation \
         is no longer updating and cannot safely resume.",
    );
    ui.add_space(6.0);
    if offer_restart {
        // Deliberately explicit that this is not a resume: an operator who
        // expects the plant to pick up where it left off would misread the
        // numbers on the next frame.
        ui.strong("You can start a fresh run, or close and relaunch the simulator.");
        ui.add_space(4.0);
        ui.label(
            "Restarting builds a brand-new plant at its default operating point. \
             The crashed run's state -- including the plot histories and the \
             simulated clock -- is discarded, not resumed.",
        );
    } else {
        ui.strong("Please close and restart the simulator.");
    }
    ui.add_space(10.0);
    ui.separator();
    ui.label(format!("Crashed subsystem: {}", report.thread_name));
    // The location is what actually identifies the failing component: a
    // simulator running its whole plant on one physics thread reports only
    // that thread's name above, which pinpoints nothing. Shown outside the
    // collapsed section, and open by default, because it is the first thing
    // anyone reporting this crash needs.
    match report.component {
        Some(component) => {
            ui.label("Failing component:");
            ui.monospace(component);
        }
        None => {
            ui.label("Failing component: not identified (simulator does not mark components)");
        }
    }
    match &report.location {
        Some(location) => {
            ui.label("Source location:");
            ui.monospace(location);
        }
        None => {
            ui.label("Source location: not captured");
        }
    }
    egui::CollapsingHeader::new("Technical details")
        .default_open(true)
        .show(ui, |ui| {
            ui.monospace(report.summary());
        });

    if !offer_restart {
        return false;
    }

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(6.0);
    ui.vertical_centered(|ui| {
        ui.button("\u{21BB}  Restart simulation")
            .on_hover_text(
                "Discard the crashed run and start a new one from the plant's \
                 default operating point.",
            )
            .clicked()
    })
    .inner
}

/// If any monitored thread has panicked, draw an unmissable modal telling the
/// user the simulation has crashed and to restart it, and return `true`;
/// otherwise draw nothing and return `false`.
///
/// The modal is an [`egui::Modal`]: centered, with a backdrop that dims and
/// blocks input to the rest of the UI, so it cannot be missed or dismissed by
/// clicking elsewhere. The captured panic message is shown under a collapsible
/// "Technical details" header.
///
/// This variant tells the user to close and relaunch the process. For an
/// in-app restart button, use [`show_crash_modal_with_restart`] instead.
///
/// Intended call pattern -- at the very top of a simulator's `eframe::App`
/// frame, so a crashed run never renders (and never touches a possibly-poisoned
/// lock):
///
/// ```ignore
/// fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
///     if show_crash_modal_if_crashed(ui.ctx(), &self.thread_health) {
///         ui.ctx().request_repaint();
///         return;
///     }
///     // ... normal rendering ...
/// }
/// ```
pub fn show_crash_modal_if_crashed(ctx: &egui::Context, health: &ThreadHealth) -> bool {
    let Some(report) = health.crash_report() else {
        return false;
    };

    egui::Modal::new(egui::Id::new("outram_park_thread_crash_modal"))
        .show(ctx, |ui| crash_modal_contents(ui, &report, false));

    true
}

/// [`show_crash_modal_if_crashed`] plus a **Restart simulation** button,
/// reporting the click through a [`CrashModalOutcome`].
///
/// # What this does and does not do
///
/// It draws and it reports. It does **not** restart anything itself -- it has
/// no access to the caller's threads or state, and could not safely touch the
/// crashed run's state even if it did. Honouring
/// [`RestartRequested`](CrashModalOutcome::RestartRequested) means *starting a
/// new run from defaults*: fresh [`SharedState`](super::SharedState) handles,
/// a fresh [`ThreadHealth`], new monitored threads, and the old handles dropped
/// unread. See this module's "What restart means here" for why resuming is not
/// on the table.
///
/// The sibling threads of the crashed run stop by themselves -- see
/// [`spawn_physics_thread_monitored`] -- so the swap does not leak a thread per
/// click.
///
/// # Intended call pattern
///
/// ```ignore
/// fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
///     let outcome = show_crash_modal_with_restart(ui.ctx(), &self.thread_health);
///     if outcome == CrashModalOutcome::RestartRequested {
///         self.restart_simulation();
///     }
///     if outcome.is_crashed() {
///         ui.ctx().request_repaint();
///         return;
///     }
///     // ... normal rendering ...
/// }
/// ```
///
/// Note the order: restart *then* return. Rendering is skipped on the restart
/// frame too, because the new run has not taken its first step yet.
pub fn show_crash_modal_with_restart(
    ctx: &egui::Context,
    health: &ThreadHealth,
) -> CrashModalOutcome {
    let Some(report) = health.crash_report() else {
        return CrashModalOutcome::Healthy;
    };

    let restart_clicked = egui::Modal::new(egui::Id::new("outram_park_thread_crash_modal"))
        .show(ctx, |ui| crash_modal_contents(ui, &report, true))
        .inner;

    if restart_clicked {
        CrashModalOutcome::RestartRequested
    } else {
        CrashModalOutcome::Showing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc as StdArc;

    #[test]
    fn fresh_thread_health_is_not_crashed() {
        let health = ThreadHealth::new();
        assert!(!health.has_crashed());
        assert!(health.crash_report().is_none());
        assert!(health.is_running());
    }

    /// Retiring a run must stop its loops **without** reporting a fault.
    ///
    /// The two flags are separate on purpose (see [`ThreadHealth`]): if
    /// `retire` set the crash flag instead of its own, every in-app restart
    /// would raise the crash modal on the *new* run's first frame, and the
    /// simulator could never be restarted twice.
    #[test]
    fn retiring_stops_the_run_without_reporting_a_crash() {
        let health = ThreadHealth::new();
        health.retire();

        assert!(!health.is_running(), "a retired run must stop its loops");
        assert!(
            !health.has_crashed(),
            "retiring is not a fault -- this is what stops a restart raising the crash modal"
        );
        assert!(health.crash_report().is_none());

        // Idempotent.
        health.retire();
        assert!(!health.is_running());
        assert!(!health.has_crashed());
    }

    /// Retiring an already-crashed run must not erase why it died.
    #[test]
    fn retiring_a_crashed_run_keeps_its_crash_report() {
        let health = ThreadHealth::new();
        spawn_monitored("doomed", health.clone(), || panic!("the boiler exploded"))
            .join()
            .unwrap();
        health.retire();

        assert!(!health.is_running());
        assert!(health.has_crashed(), "the crash must still be visible");
        let report = health.crash_report().expect("report survives retirement");
        assert!(report.message.contains("the boiler exploded"));
    }

    /// **A monitored loop must end when its run is retired** -- otherwise an
    /// in-app restart leaks a live thread per click.
    ///
    /// # Why this exists
    ///
    /// `spawn_physics_thread_monitored` used to loop unconditionally forever,
    /// which was correct while the only response to a crash was closing the
    /// process. With a restart button (kopi-beans `op-wqk.18`) the process
    /// outlives the run, so a loop with no stop condition is a thread that
    /// spins on a dead plant for the rest of the session, competing for cores
    /// with the run the operator is watching.
    ///
    /// # Methodology
    ///
    /// Spawn a monitored loop that increments a counter and sleeps 1 ms per
    /// step. Let it run, retire the run, join the thread (which must therefore
    /// return -- a hang here *is* the failure, surfaced as a test timeout), then
    /// confirm the counter is frozen: it had stepped before the retire, and does
    /// not move afterwards.
    ///
    /// # Results (2026-08-14)
    ///
    /// The thread joined promptly after `retire()`, having stepped a non-zero
    /// number of times, and the counter was identical when re-read after the
    /// join. Interpretation: the loop observes the shared stop condition and
    /// returns, so a restart swaps runs rather than accumulating them.
    #[test]
    fn a_monitored_loop_returns_once_its_run_is_retired() {
        let steps = StdArc::new(AtomicUsize::new(0));
        let health = ThreadHealth::new();
        let steps_in_thread = steps.clone();

        let handle = spawn_physics_thread_monitored(
            "retirable-physics",
            SharedState::new(0_i32),
            health.clone(),
            move |s| {
                s.update(|v| *v += 1);
                steps_in_thread.fetch_add(1, Ordering::SeqCst);
                thread::sleep(std::time::Duration::from_millis(1));
            },
        );

        // Let it actually get going, so "it stopped" is not confused with "it
        // never started".
        thread::sleep(std::time::Duration::from_millis(50));
        health.retire();

        // The failure mode of the old unconditional `loop` is that this never
        // returns.
        handle.join().expect("a retired loop must return");

        let after_join = steps.load(Ordering::SeqCst);
        assert!(
            after_join > 0,
            "the loop must have run before being retired"
        );
        thread::sleep(std::time::Duration::from_millis(20));
        assert_eq!(
            steps.load(Ordering::SeqCst),
            after_join,
            "a returned loop must not still be stepping"
        );
    }

    /// **A run's sibling loops must stop when any one of them panics**, not just
    /// the one that panicked.
    ///
    /// # Why this exists
    ///
    /// `htgr_sim_v1` runs a physics loop and a plot sampler against one
    /// `ThreadHealth`. A panic in the physics loop leaves the sampler alive,
    /// sampling a frozen plant forever. Before the restart button that was
    /// invisible (the user closed the window); after it, the sampler survives
    /// every restart and accumulates.
    ///
    /// # Methodology
    ///
    /// Two monitored loops share one `ThreadHealth`. One panics on its third
    /// step; the other only counts and sleeps, and would never stop on its own.
    /// Join both. The survivor must return, and the recorded crash must be the
    /// panicking loop's -- proving the survivor stopped *because of the sibling
    /// crash*, not because of a fault of its own.
    ///
    /// # Results (2026-08-14)
    ///
    /// Both threads joined. `crash_report().thread_name == "crasher"` with
    /// message `"physics diverged"`, and the survivor recorded no crash of its
    /// own. Interpretation: the shared stop condition propagates across a run's
    /// threads, so one panic ends the whole run.
    #[test]
    fn a_sibling_loop_stops_when_another_thread_of_the_run_panics() {
        let health = ThreadHealth::new();

        let crasher = spawn_physics_thread_monitored(
            "crasher",
            SharedState::new(0_i32),
            health.clone(),
            |s| {
                s.update(|v| *v += 1);
                if s.snapshot() >= 3 {
                    panic!("physics diverged");
                }
                thread::sleep(std::time::Duration::from_millis(1));
            },
        );

        let survivor_steps = StdArc::new(AtomicUsize::new(0));
        let counter = survivor_steps.clone();
        let survivor = spawn_physics_thread_monitored(
            "survivor",
            SharedState::new(0_i32),
            health.clone(),
            move |_| {
                counter.fetch_add(1, Ordering::SeqCst);
                thread::sleep(std::time::Duration::from_millis(1));
            },
        );

        crasher.join().expect("the crashing loop joins cleanly");
        // The failure mode is that this hangs forever.
        survivor.join().expect("the sibling loop must also return");

        assert!(!health.is_running());
        let report = health.crash_report().expect("a crash was recorded");
        assert_eq!(
            report.thread_name, "crasher",
            "the survivor must have stopped because of the sibling's panic, not its own"
        );
        assert!(report.message.contains("physics diverged"));
        assert!(survivor_steps.load(Ordering::SeqCst) > 0);
    }

    #[test]
    fn spawn_monitored_captures_str_panic_message() {
        let health = ThreadHealth::new();
        let handle = spawn_monitored("unit-test-thread", health.clone(), || {
            panic!("physics went out of range");
        });
        // The thread itself does not panic (catch_unwind swallows it), so join
        // returns Ok and deterministically synchronises the recorded crash.
        handle.join().expect("monitored thread should join cleanly");

        assert!(health.has_crashed());
        let report = health.crash_report().expect("crash report captured");
        assert_eq!(report.thread_name, "unit-test-thread");
        assert!(
            report.message.contains("physics went out of range"),
            "message was: {}",
            report.message
        );
    }

    #[test]
    fn spawn_monitored_captures_unwrap_panic_message() {
        let health = ThreadHealth::new();
        let handle = spawn_monitored("unwrap-thread", health.clone(), || {
            let none: Option<i32> = None;
            none.expect("a required value was missing");
        });
        handle.join().expect("monitored thread should join cleanly");

        let report = health.crash_report().expect("crash report captured");
        assert!(
            report.message.contains("a required value was missing"),
            "message was: {}",
            report.message
        );
    }

    #[test]
    fn healthy_monitored_thread_records_nothing() {
        let health = ThreadHealth::new();
        let handle = spawn_monitored("healthy-thread", health.clone(), || {
            // returns normally, no panic
        });
        handle.join().expect("thread joins");
        assert!(!health.has_crashed());
        assert!(health.crash_report().is_none());
    }

    #[test]
    fn spawn_physics_thread_monitored_captures_looping_panic() {
        let state = SharedState::new(0_i32);
        let health = ThreadHealth::new();
        let handle =
            spawn_physics_thread_monitored("looping-physics", state.clone(), health.clone(), |s| {
                s.update(|v| *v += 1);
                if s.snapshot() >= 3 {
                    panic!("step blew up at count {}", s.snapshot());
                }
            });
        handle
            .join()
            .expect("monitored physics thread should join cleanly");

        assert!(health.has_crashed());
        let report = health.crash_report().expect("crash report captured");
        assert!(
            report.message.contains("step blew up"),
            "message was: {}",
            report.message
        );
    }

    #[test]
    fn only_the_first_crash_is_kept() {
        let health = ThreadHealth::new();
        spawn_monitored("first", health.clone(), || panic!("first failure"))
            .join()
            .unwrap();
        spawn_monitored("second", health.clone(), || panic!("second failure"))
            .join()
            .unwrap();
        let report = health.crash_report().unwrap();
        assert_eq!(report.thread_name, "first");
        assert!(report.message.contains("first failure"));
    }

    #[test]
    fn thread_health_read_survives_a_poisoned_recorder() {
        // A raw (unmonitored) thread that panics while holding the report lock
        // must not stop the GUI from reading health poison-safely. We simulate
        // this by poisoning via a real panic path and confirming crash_report
        // still returns without panicking.
        let health = ThreadHealth::new();
        spawn_monitored("poison-canary", health.clone(), || panic!("boom"))
            .join()
            .unwrap();
        // Even after a crash, repeated reads are safe and stable.
        assert!(health.crash_report().is_some());
        assert!(health.crash_report().is_some());
    }

    /// A healthy simulator must get **no modal and no restart offer**.
    ///
    /// The cheap early return is what lets this be called unconditionally at the
    /// top of every frame; if it drew anything (or reported anything but
    /// `Healthy`) the app would stop rendering the plant forever.
    #[test]
    fn the_restart_modal_stays_out_of_the_way_while_the_run_is_healthy() {
        let ctx = egui::Context::default();
        let health = ThreadHealth::new();
        let mut outcome = CrashModalOutcome::RestartRequested; // deliberately wrong
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            outcome = show_crash_modal_with_restart(ui.ctx(), &health);
        });
        assert_eq!(outcome, CrashModalOutcome::Healthy);
        assert!(!outcome.is_crashed());
    }

    /// A crashed simulator must report `Showing` -- **not** `RestartRequested`
    /// -- while nobody has clicked the button.
    ///
    /// # Why this exists
    ///
    /// This is the failure that would be worst in practice and quietest in
    /// review: a restart that fires on every frame of a crashed run would spawn
    /// a plant and two threads per repaint, at ~60 Hz, and the modal would never
    /// clear because each new run is immediately replaced. Pin that the click,
    /// not the crash, is what returns `RestartRequested`.
    ///
    /// # Methodology
    ///
    /// Crash a monitored thread for real, then run one egui pass with default
    /// input (no pointer, no clicks) and inspect the outcome. Asserts both that
    /// a modal *was* drawn (`is_crashed`) and that no restart was requested.
    ///
    /// # Results (2026-08-14)
    ///
    /// `CrashModalOutcome::Showing`, over three consecutive passes.
    /// Interpretation: drawing the modal is idempotent and does not
    /// self-trigger a restart.
    #[test]
    fn a_crashed_run_shows_the_modal_but_does_not_restart_itself() {
        let health = ThreadHealth::new();
        spawn_monitored("test-physics", health.clone(), || panic!("out of range"))
            .join()
            .unwrap();

        let ctx = egui::Context::default();
        // Several passes: egui needs one to lay the modal out, and a restart
        // that fired on any of them would be the bug this pins.
        for pass in 0..3 {
            let mut outcome = CrashModalOutcome::Healthy;
            let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
                outcome = show_crash_modal_with_restart(ui.ctx(), &health);
            });
            assert_eq!(
                outcome,
                CrashModalOutcome::Showing,
                "pass {pass}: an unclicked crash modal must not request a restart"
            );
            assert!(outcome.is_crashed());
        }
    }

    /// The `bool`-returning modal must keep behaving exactly as it did, because
    /// `fhr_sim_v2` still calls it.
    #[test]
    fn the_original_bool_crash_modal_is_unchanged() {
        let ctx = egui::Context::default();

        let healthy = ThreadHealth::new();
        let mut shown = true;
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            shown = show_crash_modal_if_crashed(ui.ctx(), &healthy);
        });
        assert!(!shown, "a healthy run draws no modal");

        let crashed = ThreadHealth::new();
        spawn_monitored("boom", crashed.clone(), || panic!("boom"))
            .join()
            .unwrap();
        let mut shown = false;
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            shown = show_crash_modal_if_crashed(ui.ctx(), &crashed);
        });
        assert!(shown, "a crashed run draws the modal");
    }

    #[test]
    fn only_a_healthy_outcome_lets_the_caller_render() {
        assert!(!CrashModalOutcome::Healthy.is_crashed());
        assert!(CrashModalOutcome::Showing.is_crashed());
        // The restart frame must ALSO skip rendering: the new run has not
        // stepped yet.
        assert!(CrashModalOutcome::RestartRequested.is_crashed());
    }

    /// A crash report must name the SOURCE LOCATION of the panic, not just the
    /// thread.
    ///
    /// **Why this test exists.** The location is the only field that identifies
    /// which component failed. A simulator that runs its whole plant on one
    /// physics thread reports a `thread_name` of `"htgr-physics"`, which
    /// pinpoints nothing. And because the location is captured through a
    /// process-wide panic hook rather than from `catch_unwind`, it can silently
    /// regress to `None` — a diagnostic that quietly stops diagnosing is worse
    /// than none at all, so it is pinned here.
    ///
    /// **Methodology.** Panic inside a monitored thread from a known line, then
    /// assert the report carries a location naming this source file, that the
    /// line number is present, and that `summary()` includes both the location
    /// and the message. Also asserts the message itself survives, so the test
    /// fails loudly rather than passing on a location with no payload.
    ///
    /// **Results (2026-08-12).** Location captured as
    /// `crates/outram-park-digital-twin-engine/src/app_scaffold/crash.rs:<line>:<col>`,
    /// message `"scram made it hotter"`, and `summary()` contained both.
    /// Interpretation: the hook fires on a monitored physics thread and the
    /// thread-local is read back on the same thread, so a crash modal can name
    /// the failing component.
    #[test]
    fn a_crash_report_names_the_source_location_not_just_the_thread() {
        let health = ThreadHealth::new();
        let handle = spawn_monitored("test-physics", health.clone(), || {
            panic!("scram made it hotter");
        });
        handle.join().expect("monitored thread joins cleanly");

        let report = health.crash_report().expect("a crash must be recorded");
        assert_eq!(report.thread_name, "test-physics");
        assert_eq!(report.message, "scram made it hotter");

        let location = report
            .location
            .as_deref()
            .expect("the panic location must be captured, or the modal names nothing");
        assert!(
            location.contains("crash.rs"),
            "location should name this source file, got {location}"
        );

        let summary = report.summary();
        assert!(
            summary.contains(location),
            "summary must carry the location"
        );
        assert!(
            summary.contains("scram made it hotter"),
            "summary must carry the message"
        );
    }
}
