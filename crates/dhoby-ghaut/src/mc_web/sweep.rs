//! **A sweep: k_eff at a list of settings, run one after another on a
//! worker pool** (gh:#1677). A worth curve is a sweep over rod insertions;
//! any k_eff simulator on [`super::pool::KeffPool`] can drive one.
//!
//! [`Sweep::step`] is called every frame. For each setting in turn it asks
//! the pool to rebuild the workers' model with that setting, waits until
//! the pool holds it, starts a run, and collects `k ± sem` when the run
//! finishes. A run that fails, is stopped or is replaced by another one
//! ends the sweep with a reason ([`SweepStep::Failed`]).
//!
//! What the sweep needs from a model's pool is the [`SweepPool`] contract:
//! readiness, the k_eff half, whether a setting is already built, rebuild,
//! and start. The contract is a compile-time trait (no trait objects,
//! workspace rule); each simulator implements it on its own pool state.
//!
//! Moved out of the HTR-10 rung (`examples/monte_carlo_web/htr10/rods.rs`,
//! `LiveSweep`), whose stepping logic this is, made generic over the
//! setting. The rung keeps its rod families and recorded curves and
//! delegates the stepping here, so its own test still pins the behaviour.
//! Research, education and V&V only.

use super::keff::KeffConfig;
use super::pool::KeffPool;

/// What a [`Sweep`] needs from a model's pool.
pub trait SweepPool {
    /// One setting of the model's parameters (HTR-10: ten rod insertions).
    type Setting: Clone;
    /// The model a run starts on ([`KeffPool`]'s parameter).
    type Model;
    /// Why the pool cannot take the next step yet (data still processing,
    /// the workers still rebuilding), or `None` when it is ready.
    fn not_ready(&self) -> Option<String>;
    /// The pool's k_eff half: its run, if any.
    fn keff(&self) -> &KeffPool<Self::Model>;
    /// Whether the workers already hold `want`.
    fn holds(&self, want: &Self::Setting) -> bool;
    /// Rebuild the workers with `want`.
    ///
    /// # Errors
    /// The pool refuses (not ready, a run going, an invalid setting).
    fn rebuild(&mut self, want: Self::Setting) -> Result<(), String>;
    /// Start a run on the setting the workers hold.
    ///
    /// # Errors
    /// The pool refuses to start.
    fn start(&mut self, cfg: KeffConfig, now_s: f64) -> Result<(), String>;
    /// What the sweep says while the workers rebuild with the next setting.
    fn rebuilding_note(&self) -> String {
        "the workers are rebuilding the model with this setting".into()
    }
}

/// One finished position of a sweep.
#[derive(Clone, Debug, PartialEq)]
pub struct SweepResult<S> {
    /// The setting the run was built with.
    pub setting: S,
    /// The run's settings, as it ran them.
    pub cfg: KeffConfig,
    /// The run's `k` and its standard error over the active generations
    /// (`NaN` when the run left no mean).
    pub k: f64,
    pub sem: f64,
    /// Wall time of the run's generations, s.
    pub wall_s: f64,
}

/// What a sweep is doing.
#[derive(Clone, Debug, PartialEq)]
pub enum SweepStep {
    /// Waiting for the pool (data, a rebuild, or another run).
    Waiting(String),
    /// Running position `i` of `n`.
    Running { i: usize, n: usize },
    Done,
    Failed(String),
}

/// **k_eff at a list of settings, run one after another on the pool.**
pub struct Sweep<S> {
    /// The settings, in the order they run.
    pub targets: Vec<S>,
    /// The run settings every position uses.
    pub cfg: KeffConfig,
    next: usize,
    /// The run for `targets[next]` has been started (and its epoch).
    started: Option<u32>,
    /// The finished positions, in order.
    pub results: Vec<SweepResult<S>>,
    /// Why the sweep ended early, if it did.
    pub failed: Option<String>,
}

impl<S: Clone> Sweep<S> {
    /// A sweep over `targets`, each run with `cfg`.
    pub fn new(targets: Vec<S>, cfg: KeffConfig) -> Self {
        Self { targets, cfg, next: 0, started: None, results: Vec::new(), failed: None }
    }

    /// Every position finished, or the sweep failed.
    pub fn done(&self) -> bool {
        self.next >= self.targets.len() || self.failed.is_some()
    }

    /// Positions finished, and in all.
    pub fn progress(&self) -> (usize, usize) {
        (self.results.len(), self.targets.len())
    }

    /// Drive the sweep one step (call every frame): rebuild the pool with
    /// the next setting, start its run, collect it when done.
    pub fn step<P: SweepPool<Setting = S>>(&mut self, pool: &mut P, now_s: f64) -> SweepStep {
        if let Some(e) = &self.failed {
            return SweepStep::Failed(e.clone());
        }
        let n = self.targets.len();
        if self.next >= n {
            return SweepStep::Done;
        }
        if let Some(epoch) = self.started {
            let Some(run) = pool.keff().run.as_ref().filter(|r| r.epoch == epoch) else {
                return self.fail("the run was replaced by another one".into());
            };
            if run.going() {
                return SweepStep::Running { i: self.next, n };
            }
            if let Some(e) = &run.failed {
                let msg = format!("position {} failed: {e}", self.next + 1);
                return self.fail(msg);
            }
            if run.stopped {
                return self.fail("stopped".into());
            }
            let (k, sem) = run.gens.last().and_then(|g| g.gen.k_mean).unwrap_or((f64::NAN, f64::NAN));
            let wall_s = run.gens.iter().map(|g| g.wall_s).sum();
            self.results.push(SweepResult { setting: self.targets[self.next].clone(), cfg: run.cfg, k, sem, wall_s });
            self.next += 1;
            self.started = None;
            return if self.next >= n { SweepStep::Done } else { SweepStep::Waiting("next position".into()) };
        }
        if let Some(why) = pool.not_ready() {
            return SweepStep::Waiting(why);
        }
        if pool.keff().run.as_ref().is_some_and(|r| r.going()) {
            return SweepStep::Waiting("another run is going".into());
        }
        let want = self.targets[self.next].clone();
        if !pool.holds(&want) {
            return match pool.rebuild(want) {
                Ok(()) => SweepStep::Waiting(pool.rebuilding_note()),
                Err(e) => self.fail(e),
            };
        }
        match pool.start(self.cfg, now_s) {
            Ok(()) => {
                self.started = pool.keff().run.as_ref().map(|r| r.epoch);
                SweepStep::Running { i: self.next, n }
            }
            Err(e) => self.fail(e),
        }
    }

    fn fail(&mut self, why: String) -> SweepStep {
        self.failed = Some(why.clone());
        SweepStep::Failed(why)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_mc_libs::physics::keff::KeffSettings;

    fn settings(cfg: &KeffConfig) -> KeffSettings {
        KeffSettings { n_particles: cfg.n_particles, n_inactive: cfg.n_inactive, n_active: cfg.n_active, seed: cfg.seed, ..KeffSettings::default() }
    }

    /// A pool whose setting is one number: it holds `held`, rebuilds take a
    /// call to [`Fake::finish_rebuild`], and settings above 9 are refused.
    struct Fake {
        keff: KeffPool<u8>,
        held: u8,
        rebuilding: bool,
        rebuilds: Vec<u8>,
    }
    impl Fake {
        fn new() -> Self {
            Self { keff: KeffPool::new(2, "test".into(), settings, 0), held: 0, rebuilding: false, rebuilds: Vec::new() }
        }
        fn finish_rebuild(&mut self) {
            self.rebuilding = false;
        }
    }
    impl SweepPool for Fake {
        type Setting = u8;
        type Model = u8;
        fn not_ready(&self) -> Option<String> {
            self.rebuilding.then(|| "rebuilding".into())
        }
        fn keff(&self) -> &KeffPool<u8> {
            &self.keff
        }
        fn holds(&self, want: &u8) -> bool {
            self.held == *want
        }
        fn rebuild(&mut self, want: u8) -> Result<(), String> {
            if want > 9 {
                return Err(format!("setting {want} is out of range"));
            }
            self.held = want;
            self.rebuilding = true;
            self.rebuilds.push(want);
            Ok(())
        }
        fn start(&mut self, cfg: KeffConfig, now_s: f64) -> Result<(), String> {
            self.keff.start_run(cfg, self.held, now_s)
        }
    }

    const CFG: KeffConfig = KeffConfig { n_particles: 10, n_inactive: 1, n_active: 2, seed: 7, point_source: false, want_sites: false };

    /// A sweep rebuilds before each setting it does not hold, starts the
    /// run on it, and ends with the run's reason when the run is stopped.
    #[test]
    fn a_sweep_rebuilds_then_runs_and_stops_with_its_run() {
        let mut p = Fake::new();
        let mut sw = Sweep::new(vec![0u8, 3], CFG);
        assert_eq!(sw.progress(), (0, 2));
        // The pool holds 0: the first run starts at once, on 0.
        assert_eq!(sw.step(&mut p, 0.0), SweepStep::Running { i: 0, n: 2 });
        assert_eq!(p.keff.run.as_ref().map(|r| r.model), Some(0));
        assert!(p.rebuilds.is_empty());
        assert_eq!(sw.step(&mut p, 1.0), SweepStep::Running { i: 0, n: 2 }, "still going");
        p.keff.stop_run();
        assert_eq!(sw.step(&mut p, 2.0), SweepStep::Failed("stopped".into()));
        assert!(sw.done());
        assert_eq!(sw.step(&mut p, 3.0), SweepStep::Failed("stopped".into()), "stays failed");

        // A setting the pool does not hold: rebuild, wait, then run on it.
        let mut one = Sweep::new(vec![5u8], CFG);
        assert!(matches!(one.step(&mut p, 4.0), SweepStep::Waiting(_)));
        assert_eq!(p.rebuilds, vec![5]);
        assert_eq!(one.step(&mut p, 5.0), SweepStep::Waiting("rebuilding".into()));
        p.finish_rebuild();
        assert_eq!(one.step(&mut p, 6.0), SweepStep::Running { i: 0, n: 1 });
        assert_eq!(p.keff.run.as_ref().map(|r| r.model), Some(5));
        assert!(!one.done());
    }

    /// Another run started over the sweep's ends it, and a setting the pool
    /// refuses ends it with the pool's reason.
    #[test]
    fn a_replaced_run_or_a_refused_setting_ends_the_sweep() {
        let mut p = Fake::new();
        let mut sw = Sweep::new(vec![0u8], CFG);
        assert_eq!(sw.step(&mut p, 0.0), SweepStep::Running { i: 0, n: 1 });
        p.keff.stop_run();
        p.keff.start_run(CFG, 0, 1.0).expect("a new run");
        assert_eq!(sw.step(&mut p, 2.0), SweepStep::Failed("the run was replaced by another one".into()));

        let mut q = Fake::new();
        let mut bad = Sweep::new(vec![12u8], CFG);
        assert_eq!(bad.step(&mut q, 0.0), SweepStep::Failed("setting 12 is out of range".into()));
        assert_eq!(bad.failed.as_deref(), Some("setting 12 is out of range"));
        assert_eq!(bad.progress(), (0, 1));
    }
}
