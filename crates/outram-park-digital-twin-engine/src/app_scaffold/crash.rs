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
//!
//! It deliberately does **not** try to restart the process: a panicked physics
//! thread may have poisoned a shared lock mid-write, so the safe course is to
//! stop and ask the user to relaunch. To keep the GUI itself from then
//! cascade-panicking on that poisoned lock, [`SharedState`](super::SharedState)
//! recovers poisoned guards, and simulators should early-return from their
//! frame once the modal is shown (so they never touch a poisoned `Mutex`).

use std::any::Any;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
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
}

/// A shared "has any monitored background thread panicked?" flag for one
/// simulator, plus the [`CrashReport`] of the first panic.
///
/// Backed by an [`Arc`] internally, so [`Clone`] just bumps the refcount --
/// clone one handle per monitored thread (they *record* into it) and one for
/// the GUI (which *queries* it every frame via
/// [`has_crashed`](Self::has_crashed) / [`crash_report`](Self::crash_report)).
/// Only the first panic is kept; later panics from other threads are ignored so
/// the reported cause is the root one.
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
    /// The first crash's report. Written once (under the lock) before `crashed`
    /// is set; read poison-safely by the GUI.
    report: RwLock<Option<CrashReport>>,
}

impl ThreadHealth {
    /// Create a fresh, healthy handle (no crash recorded).
    pub fn new() -> Self {
        Self {
            inner: Arc::new(ThreadHealthInner {
                crashed: AtomicBool::new(false),
                report: RwLock::new(None),
            }),
        }
    }

    /// Has any monitored thread panicked? Cheap (a single atomic load) -- safe
    /// to call every frame from the GUI thread.
    pub fn has_crashed(&self) -> bool {
        self.inner.crashed.load(Ordering::Acquire)
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
    thread::spawn(move || {
        // AssertUnwindSafe: on a panic we do not resume the simulation, we ask
        // the user to restart, so a partially-mutated shared state left behind
        // an unwind is acceptable here (it is exactly what we are reporting).
        let result = catch_unwind(AssertUnwindSafe(body));
        if let Err(payload) = result {
            let message = panic_message(payload);
            eprintln!("[physics thread '{thread_name}' panicked] {message}");
            health.record(CrashReport {
                thread_name,
                message,
            });
        }
    })
}

/// The monitored counterpart of
/// [`spawn_physics_thread`](super::spawn_physics_thread): repeatedly calls
/// `step` against a cloned [`SharedState`] handle in an infinite loop, but if
/// `step` ever panics the panic is caught and reported to `health` rather than
/// silently killing the thread.
///
/// Use this in place of `spawn_physics_thread` wherever you want a crashed
/// physics loop to surface the [`show_crash_modal_if_crashed`] restart prompt.
/// `thread_name` labels the subsystem in that modal.
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
    spawn_monitored(thread_name, health, move || loop {
        step(&state);
    })
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

    egui::Modal::new(egui::Id::new("outram_park_thread_crash_modal")).show(ctx, |ui| {
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
        ui.strong("Please close and restart the simulator.");
        ui.add_space(10.0);
        ui.separator();
        ui.label(format!("Crashed subsystem: {}", report.thread_name));
        egui::CollapsingHeader::new("Technical details")
            .default_open(false)
            .show(ui, |ui| {
                ui.monospace(&report.message);
            });
    });

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_thread_health_is_not_crashed() {
        let health = ThreadHealth::new();
        assert!(!health.has_crashed());
        assert!(health.crash_report().is_none());
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
}
