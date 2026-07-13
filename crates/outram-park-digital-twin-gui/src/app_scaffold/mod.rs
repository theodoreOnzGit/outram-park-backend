//! Reusable `eframe::App` threading/locking + panel-dispatch scaffold.
//!
//! Ported from the shared-state/physics-thread pattern used by
//! `tampines-steam-tables`'s `fhr_sim_v1`/`fhr_sim_v2` examples
//! (`examples/fhr_sim_v2/main.rs`'s `FHRSimulatorApp`/`FHRState`: one
//! `Arc<Mutex<FHRState>>` cloned into three spawned threads -- PRKE
//! kinetics, thermal-hydraulics, and plot-data updates -- each looping
//! indefinitely, locking to read inputs and write results back, while the
//! GUI thread locks briefly to clone a rendering snapshot). Real, working
//! code -- this pattern is simple and already proven twice; the content of
//! any given panel is still the calling application's job.
//!
//! **Deliberate deviation from the original:** this uses
//! [`Arc`]`<`[`RwLock`]`<T>>`, not `Arc<Mutex<T>>`, per this workspace's
//! mandatory Rust design rules -- `RwLock` allows concurrent reads from
//! multiple threads, where `Mutex` serialises even read-only access and so
//! defeats parallelism during a timestep's compute phase.

use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::thread::{self, JoinHandle};

/// A physics/simulation state shared between a rendering thread (the GUI)
/// and one or more computation threads, matching `fhr_sim_v2`'s
/// `Arc<Mutex<FHRState>>` pattern but backed by [`RwLock`] instead of
/// [`std::sync::Mutex`] (see this module's doc for why).
///
/// Cheap to [`Clone`] (it is just an `Arc` underneath) -- clone one handle
/// per thread that needs access, the same way `fhr_sim_v2` clones
/// `Arc<Mutex<FHRState>>` once per spawned thread.
pub struct SharedState<T>(Arc<RwLock<T>>);

impl<T> SharedState<T> {
    /// Wrap `initial` as a new shared state.
    pub fn new(initial: T) -> Self {
        Self(Arc::new(RwLock::new(initial)))
    }

    /// A cloned snapshot of the current state, for rendering -- the
    /// `RwLock` read guard is dropped before this returns, so the GUI thread
    /// never holds the lock while painting (matching `fhr_sim_v2`'s
    /// lock-clone-drop-then-render pattern in its `main_page`).
    pub fn snapshot(&self) -> T
    where
        T: Clone,
    {
        self.0.read().expect("SharedState lock poisoned").clone()
    }

    /// Mutate the state in place via `f`, holding the write lock only for
    /// the duration of the call -- used both by a physics thread updating
    /// its computed results and by GUI widgets (e.g. a slider) writing a
    /// user input back into the shared state.
    pub fn update(&self, f: impl FnOnce(&mut T)) {
        let mut guard: RwLockWriteGuard<T> = self.0.write().expect("SharedState lock poisoned");
        f(&mut guard);
    }

    /// Read the state via a closure without cloning it -- for callers who
    /// only need a derived value, not a full snapshot.
    pub fn read_with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        let guard: RwLockReadGuard<T> = self.0.read().expect("SharedState lock poisoned");
        f(&guard)
    }
}

impl<T> Clone for SharedState<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

/// Spawn a physics-computation thread that repeatedly calls `step` against
/// a cloned handle to `state`, looping indefinitely -- matching
/// `fhr_sim_v2`'s `thread::spawn(move || { FHRSimulatorApp::calculate_..._loop(state_ptr) })`
/// pattern, one call site per physics subsystem (that function's own
/// internal `loop { ... }` is `step`'s job here, called once per
/// iteration by this wrapper instead of written out at each call site).
///
/// `step` is handed a fresh clone of `state` each call, so it can read/write
/// the shared state itself; the *loop* (and thus the timestep cadence,
/// sleep/backoff, etc.) is entirely `step`'s responsibility.
pub fn spawn_physics_thread<T, F>(state: SharedState<T>, mut step: F) -> JoinHandle<()>
where
    T: Send + Sync + 'static,
    F: FnMut(&SharedState<T>) + Send + 'static,
{
    thread::spawn(move || loop {
        step(&state);
    })
}

/// A set of selectable top-level GUI panels/pages, implemented by the
/// calling application's own enum (matching `fhr_sim_v2`'s `Panel` enum:
/// `MainPage`/`ReactorPowerGraphs`/`PoisonGraphs`).
pub trait PanelSet: Copy + PartialEq + 'static {
    /// All selectable panels, in the order they should appear as tabs.
    const ALL: &'static [Self];

    /// Human-readable label for this panel's tab/button.
    fn label(&self) -> &'static str;
}

/// Render a horizontal row of selectable buttons, one per `P::ALL` panel,
/// updating `current` when the user picks a different one -- the reusable
/// half of `fhr_sim_v2`'s `ui.horizontal(|ui| { ui.selectable_value(&mut
/// self.open_panel, Panel::X, "X"); ... })` top-bar row.
pub fn panel_selector_ui<P: PanelSet>(ui: &mut egui::Ui, current: &mut P) {
    ui.horizontal(|ui| {
        for panel in P::ALL {
            ui.selectable_value(current, *panel, panel.label());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc as StdArc;

    #[test]
    fn snapshot_and_update_round_trip() {
        let state = SharedState::new(42_i32);
        assert_eq!(state.snapshot(), 42);
        state.update(|v| *v += 1);
        assert_eq!(state.snapshot(), 43);
    }

    #[test]
    fn clone_shares_the_same_underlying_state() {
        let state = SharedState::new(0_i32);
        let handle = state.clone();
        handle.update(|v| *v = 100);
        assert_eq!(state.snapshot(), 100);
    }

    #[test]
    fn read_with_does_not_clone() {
        let state = SharedState::new(vec![1, 2, 3]);
        let len = state.read_with(|v| v.len());
        assert_eq!(len, 3);
    }

    #[test]
    fn spawn_physics_thread_actually_runs_step_repeatedly() {
        let counter = StdArc::new(AtomicUsize::new(0));
        let state = SharedState::new(0_i32);
        let counter_clone = counter.clone();
        let _handle = spawn_physics_thread(state.clone(), move |s| {
            s.update(|v| *v += 1);
            counter_clone.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(1));
        });
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(counter.load(Ordering::SeqCst) > 0);
        assert!(state.snapshot() > 0);
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    enum MockPanel {
        Main,
        Diagnostics,
    }

    impl PanelSet for MockPanel {
        const ALL: &'static [Self] = &[Self::Main, Self::Diagnostics];

        fn label(&self) -> &'static str {
            match self {
                Self::Main => "Main",
                Self::Diagnostics => "Diagnostics",
            }
        }
    }

    #[test]
    fn panel_selector_ui_renders_without_panicking() {
        let ctx = egui::Context::default();
        let mut current = MockPanel::Main;
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show_inside(ui, |ui| {
                panel_selector_ui(ui, &mut current);
            });
        });
    }
}
