//! # `LocalHost`: a twin on native threads (gh:#1745)
//!
//! The native [`TwinHost`]: the twin lives in an `Arc<RwLock<..>>` (the
//! workspace's shared-state rule), a plant thread advances it in real time
//! (times a speed-up), and each Monte Carlo or multiphysics request runs on
//! its own thread on a clone of the case, so the plant keeps stepping and
//! the UI thread only reads. A finished Monte Carlo set is recorded on the
//! twin ([`MultiFidelityTwin::record_monte_carlo`]) and reaches the plant
//! only on "send to plant".
//!
//! In the browser a host runs the twin in a Web Worker instead (the
//! adopters' `web` modules); this one is native only.
//!
//! Research, education and V&V only. NOT VALIDATED. AI draft (gh:#1745).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread::JoinHandle;

use egui::Ui;
use uom::si::f64::Time;
use uom::si::time::second;

use super::app::{McPageView, McRunView, TwinAction, TwinHost};
use super::{
    ControlVolumeMap, ControlVolumes, FeedbackCoupling, KeffResult, MeshResolution, MonteCarloCase, MultiFidelityTwin,
    MultiphysicsCase, PlantModel, TwinError, TwinPage, TwinSnapshot,
};

/// Most plant rows kept for the trend.
pub const HISTORY_ROWS: usize = 20_000;

/// The adopter's plant controls: drawn in the panel, returning the
/// commands the reader gave this frame.
pub trait PlantControls<P: PlantModel> {
    /// GUI code (exempt from a reaching test).
    fn controls(&mut self, ui: &mut Ui, latest: Option<&P::Snapshot>) -> Vec<P::Command>;
}

/// See the module docs.
pub struct LocalHost<P, M, F, C>
where
    P: PlantModel + ControlVolumeMap + FeedbackCoupling<M::Coefficients>,
    M: MonteCarloCase,
    F: MultiphysicsCase,
{
    twin: Arc<RwLock<MultiFidelityTwin<P, M, F>>>,
    latest: Arc<RwLock<Option<P::Snapshot>>>,
    history: Arc<RwLock<Vec<Vec<f64>>>>,
    mc: Arc<RwLock<McPageView>>,
    multiphysics: Arc<RwLock<[Vec<String>; 2]>>,
    case: Option<M>,
    physics: Option<F>,
    controls: C,
    title: String,
    stop: Arc<AtomicBool>,
    plant_thread: Option<JoinHandle<()>>,
}

fn write<T>(l: &RwLock<T>) -> std::sync::RwLockWriteGuard<'_, T> {
    l.write().unwrap_or_else(|e| e.into_inner())
}

fn read<T>(l: &RwLock<T>) -> std::sync::RwLockReadGuard<'_, T> {
    l.read().unwrap_or_else(|e| e.into_inner())
}

impl<P, M, F, C> LocalHost<P, M, F, C>
where
    P: PlantModel + ControlVolumeMap + FeedbackCoupling<M::Coefficients> + Send + Sync + 'static,
    P::Snapshot: Send + Sync + 'static,
    P::Command: Send + 'static,
    M: MonteCarloCase + Clone + Send + Sync + 'static,
    M::Coefficients: Send + Sync + 'static,
    F: MultiphysicsCase + Clone + Send + Sync + 'static,
    F::Result: Send + 'static,
    C: PlantControls<P>,
{
    /// Start the plant thread: every `wall` of real time it advances the
    /// plant by `wall × speed`. A plant error stops the thread and shows on
    /// the Monte Carlo page's error line and in [`Self::plant_error`].
    pub fn start(twin: MultiFidelityTwin<P, M, F>, controls: C, title: String, wall: Time, speed: f64) -> Self {
        let case = twin.monte_carlo().cloned();
        let physics = twin.multiphysics().cloned();
        let first = twin.plant().snapshot();
        let history = Arc::new(RwLock::new(vec![first.values()]));
        let latest = Arc::new(RwLock::new(Some(first)));
        let twin = Arc::new(RwLock::new(twin));
        let mc = Arc::new(RwLock::new(McPageView {
            runs: case.as_ref().map(|c| c.runs().into_iter().map(|r| McRunView { label: r.label, result: None }).collect()).unwrap_or_default(),
            ..McPageView::default()
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let plant_thread = {
            let (twin, latest, history, mc, stop) = (twin.clone(), latest.clone(), history.clone(), mc.clone(), stop.clone());
            let wall_s = wall.get::<second>().max(1e-3);
            let dt = Time::new::<second>(wall_s * speed);
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    std::thread::sleep(std::time::Duration::from_secs_f64(wall_s));
                    let step = write(&twin).advance(dt);
                    match step {
                        Ok(s) => {
                            let mut h = write(&history);
                            h.push(s.values());
                            if h.len() > HISTORY_ROWS {
                                let extra = h.len() - HISTORY_ROWS;
                                h.drain(..extra);
                            }
                            *write(&latest) = Some(s);
                        }
                        Err(e) => {
                            write(&mc).error = Some(format!("the plant stopped: {e}"));
                            break;
                        }
                    }
                }
            })
        };
        LocalHost { twin, latest, history, mc, multiphysics: Arc::new(RwLock::new([Vec::new(), Vec::new()])), case, physics, controls, title, stop, plant_thread: Some(plant_thread) }
    }

    /// The twin, shared (read it briefly: the plant thread writes it).
    pub fn twin(&self) -> Arc<RwLock<MultiFidelityTwin<P, M, F>>> {
        self.twin.clone()
    }

    /// Apply a plant command now.
    pub fn send(&self, command: P::Command) -> Result<(), TwinError> {
        write(&self.twin).apply(command)
    }

    /// The plant thread's failure, if it stopped on one.
    pub fn plant_error(&self) -> Option<String> {
        read(&self.mc).error.clone().filter(|e| e.starts_with("the plant stopped"))
    }

    /// Whether a Monte Carlo request is still running.
    pub fn monte_carlo_running(&self) -> bool {
        read(&self.mc).running
    }

    fn run_monte_carlo(&self) {
        let Some(case) = self.case.clone() else {
            return;
        };
        {
            let mut v = write(&self.mc);
            if v.running {
                return;
            }
            v.running = true;
            v.error = None;
            for r in &mut v.runs {
                r.result = None;
            }
        }
        let (twin, mc) = (self.twin.clone(), self.mc.clone());
        std::thread::spawn(move || {
            let mut case = case;
            let n = case.runs().len();
            let mut results: Vec<KeffResult> = Vec::with_capacity(n);
            for i in 0..n {
                match case.run(i) {
                    Ok(k) => {
                        results.push(k);
                        if let Some(r) = write(&mc).runs.get_mut(i) {
                            r.result = Some(k);
                        }
                    }
                    Err(e) => {
                        let mut v = write(&mc);
                        v.error = Some(e.to_string());
                        v.running = false;
                        return;
                    }
                }
            }
            let recorded = write(&twin).record_monte_carlo(results).map(|_| ());
            let mut v = write(&mc);
            v.running = false;
            if let Err(e) = recorded {
                v.error = Some(e.to_string());
            }
        });
    }

    fn run_multiphysics(&self, r: MeshResolution) {
        let Some(physics) = self.physics.clone() else {
            return;
        };
        if let Err(e) = read(&self.twin).resolution_status(r) {
            write(&self.multiphysics)[slot(r)] = vec![e.to_string()];
            return;
        }
        write(&self.multiphysics)[slot(r)] = vec!["running…".into()];
        let out = self.multiphysics.clone();
        std::thread::spawn(move || {
            let mut physics = physics;
            let lines = match physics.run(r) {
                Ok(result) => physics.describe(&result),
                Err(e) => vec![format!("Failed: {e}")],
            };
            write(&out)[slot(r)] = lines;
        });
    }
}

fn slot(r: MeshResolution) -> usize {
    match r {
        MeshResolution::Coarse => 0,
        MeshResolution::Fine => 1,
    }
}

impl<P, M, F, C> TwinHost for LocalHost<P, M, F, C>
where
    P: PlantModel + ControlVolumeMap + FeedbackCoupling<M::Coefficients> + Send + Sync + 'static,
    P::Snapshot: Send + Sync + 'static,
    P::Command: Send + 'static,
    M: MonteCarloCase + Clone + Send + Sync + 'static,
    M::Coefficients: Send + Sync + 'static,
    F: MultiphysicsCase + Clone + Send + Sync + 'static,
    F::Result: Send + 'static,
    C: PlantControls<P>,
{
    fn title(&self) -> String {
        self.title.clone()
    }
    fn page_status(&self, page: TwinPage) -> Result<(), TwinError> {
        read(&self.twin).page_status(page)
    }
    fn columns(&self) -> Vec<&'static str> {
        P::Snapshot::columns()
    }
    fn history(&self) -> Vec<Vec<f64>> {
        read(&self.history).clone()
    }
    fn control_volumes(&self) -> ControlVolumes {
        read(&self.twin).control_volumes()
    }
    fn monte_carlo(&self) -> McPageView {
        let mut v = read(&self.mc).clone();
        let t = read(&self.twin);
        v.in_use = t.coefficients().provenance();
        if let (Some(live), Some(case)) = (t.live_coefficients(), t.monte_carlo()) {
            v.live = case.describe(live.value());
        }
        v
    }
    fn multiphysics(&self, resolution: MeshResolution) -> Vec<String> {
        read(&self.multiphysics)[slot(resolution)].clone()
    }
    fn controls(&mut self, ui: &mut Ui) {
        let latest = read(&self.latest).clone();
        for c in self.controls.controls(ui, latest.as_ref()) {
            if let Err(e) = self.send(c) {
                write(&self.mc).error = Some(e.to_string());
            }
        }
    }
    fn act(&mut self, action: TwinAction) {
        match action {
            TwinAction::RunMonteCarlo => self.run_monte_carlo(),
            TwinAction::SendToPlant => {
                if let Err(e) = write(&self.twin).send_to_plant() {
                    write(&self.mc).error = Some(e.to_string());
                }
            }
            TwinAction::RunMultiphysics(r) => self.run_multiphysics(r),
        }
    }
}

impl<P, M, F, C> Drop for LocalHost<P, M, F, C>
where
    P: PlantModel + ControlVolumeMap + FeedbackCoupling<M::Coefficients>,
    M: MonteCarloCase,
    F: MultiphysicsCase,
{
    /// Stop the plant thread and wait for it (a Monte Carlo thread finishes
    /// its run on its own and writes to state nobody reads).
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.plant_thread.take() {
            let _ = h.join();
        }
    }
}
