//! **The k_eff run**: [`KeffSettings`], the [`MCSettings`] run-size presets,
//! the Auto entropy watcher, and the [`KeffRun`] result with its provenance
//! (gh #939, epic #925).
//!
//! # The presets (maintainer decisions on #925, 2026-10-17)
//!
//! | preset | inactive | active |
//! |---|---|---|
//! | [`MCSettings::Auto`] | watcher at 1 000 / cycle, then settle and re-confirm at 10 000 | 250 cycles × 10 000 |
//! | [`MCSettings::Smoke`] | the same watcher, all at 1 000 | 50 cycles × 1 000 (**does it run at all**) |
//! | [`MCSettings::Fast`] | the same watcher, re-confirm at 5 000 | 120 cycles × 5 000 (**a smoke test**) |
//! | [`MCSettings::Pcm`] | the same watcher, re-confirm at 10 000 | 10 000 / cycle until the correlation-aware σ reaches the tolerance |
//! | [`MCSettings::High`] | the same watcher, re-confirm at 50 000 | 1 500 cycles × 50 000 |
//! | [`MCSettings::Custom`] | fixed, given | fixed, given (every field, seed included) |
//!
//! Reference: F. B. Brown, *A Review of Monte Carlo Criticality
//! Calculations – Convergence, Bias, Statistics*, M&C 2009, LA-UR-08-06558
//! (standard open corpus `us-doe/la-ur-08-06558-brown-mc-criticality-review-paper.pdf`,
//! Conclusions, PDF pp. 11-12): view the geometry first; make a **trial run**
//! of ~100 cycles at ~1000 neutrons per cycle and examine k and H_src to
//! choose the discard; use **at least 5000** neutrons per cycle for production
//! (10 000, 20 000, 50 000 or more preferable) with **a few hundred active
//! cycles**; start from a uniform source over the fissionable regions.
//!
//! Every preset but `Custom` uses the fixed, recorded seed [`PRESET_SEED`].
//! The first generation starts uniform over the fissionable shapes
//! ([`super::AssembledModel::source_box`]).
//!
//! # The watcher's thresholds are NOT yet justified
//!
//! The Auto watcher (window, check interval, settling length, entropy mesh)
//! is implemented with **named constants whose values have no measured
//! justification yet**: the experiments that are to justify them, against
//! standard practice (10 000 neutrons per inactive cycle until k and H
//! converge), are gh #927, and the **normality** half of the criterion
//! (skewness and kurtosis on the window) needs a raffles estimator that does
//! not exist yet (gh #944). Until then the watcher applies the
//! **stationarity** half only (MSER-5 and Geweke on k and H over the window,
//! `stats::convergence`), and every result says so
//! ([`NormalityCheck::NotYetAvailable`]). **Auto is not validated.**
//!
//! # Run backend
//!
//! The single-thread CSG power iteration
//! ([`CsgPowerIteration`]), bit for bit `run_keff_csg` with
//! `ComputeType::CpuSingleThread` when the population is not changed. Lost
//! histories are not counted on this path (gh #699); the result says so.
//! A model with a delta-tracked `DhRegion` (gh #926) passes its majorants
//! ([`AssembledModel::majorants`]), which makes it `run_keff_csg_hybrid`.

use raffles::estimators::autocorrelation::CorrelatedMean;
use raffles::estimators::mean_and_stderr;

use super::collect::{
    Collector, Ealf, Kinetics, Leakage, MCDiagnosticResult, MeshMap, NeutronBalance, RunContext,
    RunHealth, SixFactors, Spectrum,
};
use super::data::DataError;
use super::diagnostics::{DiagnosticError, Diagnostics, MCDiagnostic, Region};
use super::model::{AssembledModel, Model, ModelError};
use crate::material::nuclide::Nuclide;
use crate::physics::keff::KeffSettings as LowLevelKeffSettings;
use crate::physics::transport_csg::CsgPowerIteration;
use crate::stats::convergence::{SourceConvergence, TraceDiagnosis};
use crate::tally::mesh::RegularMesh;

/// Seed of every preset run (OpenMC's default seed is also 1,
/// `src/random_lcg.cpp:8`). Recorded in each result.
pub const PRESET_SEED: u64 = 1;

/// Neutrons per inactive cycle while the watcher judges convergence.
/// Brown's trial run uses "a moderate number of neutrons per cycle (e.g.,
/// 1000)" for exactly this judgement (LA-UR-08-06558, Conclusions); whether
/// automating it at this size holds up is **not yet shown** (#927).
pub const WATCH_PARTICLES: usize = 1_000;

/// Cycles in the watcher's window (the maintainer's "minimum for a
/// Gaussian-shape judgement"). **Not yet justified** (#927).
pub const WATCH_WINDOW_CYCLES: usize = 30;

/// The watcher looks every this many cycles. **Not yet justified** (#927).
pub const WATCH_EVERY_CYCLES: usize = 5;

/// Cycles discarded right after raising the population, before the
/// watcher re-confirms convergence at the production size (maintainer
/// decision on #925: "discard a short transition"). The length is **not yet
/// justified** (#927).
pub const SETTLE_CYCLES: usize = 10;

/// Cap on all inactive cycles; past it the result says "not converged"
/// (maintainer decision on #925).
pub const MAX_INACTIVE_CYCLES: usize = 100;

/// Divisions per side of the entropy mesh over the fissionable shapes'
/// bounding box. **Not yet justified** (#927); the finite-sample bias of
/// the entropy level (#925, open point 1) does not affect a window that
/// stays at one population, which is how the watcher uses it.
pub const ENTROPY_MESH_DIVISIONS: usize = 5;

/// Auto's production population (maintainer decision; Brown: 10 000 is
/// among the preferable sizes).
pub const AUTO_PARTICLES: usize = 10_000;
/// Auto's active cycles (maintainer decision, 2026-10-17; Brown: "a few
/// hundred").
pub const AUTO_ACTIVE_CYCLES: usize = 250;
/// Smoke's population (maintainer direction, 2026-10-18: "1000 particles
/// per batch"); Brown's trial-run size, a fifth of his production minimum.
pub const SMOKE_PARTICLES: usize = 1_000;
/// Smoke's active cycles: with the watched inactive cycles, about Brown's
/// ~100-cycle trial run (LA-UR-08-06558, Conclusions). Chosen, not measured.
pub const SMOKE_ACTIVE_CYCLES: usize = 50;
/// Fast's population (meets Brown's 5000 minimum).
pub const FAST_PARTICLES: usize = 5_000;
/// Fast's active cycles (short of Brown's "a few hundred").
pub const FAST_ACTIVE_CYCLES: usize = 120;
/// High's population (among Brown's preferable sizes).
pub const HIGH_PARTICLES: usize = 50_000;
/// High's active cycles: the maintainer's choice. (Not the RMC reference's:
/// Li et al. 2014 ran 140 cycles of 10 000.)
pub const HIGH_ACTIVE_CYCLES: usize = 1_500;
/// Pcm's population.
pub const PCM_PARTICLES: usize = 10_000;
/// Pcm's minimum active cycles, so a lucky early σ cannot stop it. Reuses
/// Auto's 250, **provisionally** (open on #925).
pub const PCM_MIN_ACTIVE_CYCLES: usize = 250;
/// Pcm's maximum active cycles; reaching it is reported as "tolerance not
/// reached". **Not yet justified** (#925).
pub const PCM_MAX_ACTIVE_CYCLES: usize = 5_000;

/// A reactivity tolerance for [`MCSettings::Pcm`], in pcm (1 pcm = 1e-5 in
/// k).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReactivityTolerance {
    pcm: f64,
}

impl ReactivityTolerance {
    /// `value` pcm. Must be positive; a run refuses one that is not.
    pub fn pcm(value: f64) -> Self {
        Self { pcm: value }
    }

    /// The tolerance in pcm.
    pub fn in_pcm(&self) -> f64 {
        self.pcm
    }
}

/// **Every setting of a hand-sized run**, all required, the seed included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CustomRun {
    /// Neutrons per cycle (every cycle).
    pub particles: usize,
    /// Inactive (discarded) cycles.
    pub inactive: usize,
    /// Active (tallied) cycles.
    pub active: usize,
    /// RNG seed.
    pub seed: u64,
}

/// **How big a k_eff run is.** One enum, so the choice is one word. See the
/// module docs for each preset's numbers and their sources.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MCSettings {
    /// The watcher at 1 000 neutrons per inactive cycle (cap 100), then 250
    /// active cycles at 10 000. The watcher's thresholds are not yet
    /// justified (#927, #944); see the module docs.
    Auto,
    /// **Does it run at all**: [`SMOKE_PARTICLES`] per cycle throughout and
    /// [`SMOKE_ACTIVE_CYCLES`] active cycles, about Brown's trial run
    /// (LA-UR-08-06558, Conclusions). For checking a model builds and
    /// transports, and for tutorials' first look; k carries a large σ and
    /// is **not a result to quote**. The result says so.
    Smoke,
    /// **A smoke test**: a quick check that the model builds, runs and gives
    /// a plausible k_eff, **not a result to quote**. In the spirit of
    /// Brown's recommended trial run (LA-UR-08-06558, Conclusions), not his
    /// recipe: 5 000 per cycle meets his production minimum, but 120 active
    /// cycles fall short of his "few hundred". The result says so.
    Fast,
    /// 10 000 per cycle, active cycles until the correlation-aware σ of k
    /// (batch means, Brown: codes underestimate tally statistics) is at most
    /// the tolerance; at least [`PCM_MIN_ACTIVE_CYCLES`], at most
    /// [`PCM_MAX_ACTIVE_CYCLES`].
    Pcm(ReactivityTolerance),
    /// 50 000 × 1 500 active cycles (the maintainer's choice).
    High,
    /// Everything given by the user; no watcher (the convergence verdict is
    /// report-only).
    Custom(CustomRun),
}

/// **What to run**: a run size, the diagnostics to collect, and whether to
/// print progress while it runs.
#[derive(Debug, Clone, PartialEq)]
pub struct KeffSettings {
    mc: MCSettings,
    diagnostics: Option<Diagnostics>,
    progress: Progress,
}

/// **Progress printed while a run goes** (maintainer direction, 2026-10-18:
/// "by default as keff runs, should print statistics like OpenMC").
///
/// [`Progress::Print`] is the default: each nuclide as its data is
/// processed, then one line per generation in OpenMC's layout (generation,
/// population, k, Shannon entropy, and, in the active cycles, the running
/// average k ± σ), plus a line whenever the Auto watcher changes phase. It
/// goes to standard output, so a long run can be watched with `tail -f`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Progress {
    /// Print as the run goes (the default).
    #[default]
    Print,
    /// Print nothing.
    Silent,
}

impl KeffSettings {
    /// `KeffSettings::new(MCSettings::Auto, None)`: `None` means k_eff
    /// only; `Some(Diagnostics::default())` collects the "wanted by
    /// everyone" set ([`Diagnostics`]).
    pub fn new(mc: MCSettings, diagnostics: Option<Diagnostics>) -> Self {
        Self {
            mc,
            diagnostics,
            progress: Progress::Print,
        }
    }

    /// The same settings, printing nothing while the run goes
    /// ([`Progress::Silent`]).
    pub fn silent(mut self) -> Self {
        self.progress = Progress::Silent;
        self
    }

    /// Whether progress is printed.
    pub fn progress(&self) -> Progress {
        self.progress
    }

    /// The run size.
    pub fn mc(&self) -> MCSettings {
        self.mc
    }

    /// The diagnostics requested.
    pub fn diagnostics(&self) -> Option<&Diagnostics> {
        self.diagnostics.as_ref()
    }
}

/// Why a run could not be made.
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    /// The model did not assemble.
    #[error(transparent)]
    Model(#[from] ModelError),
    /// The nuclear data could not be loaded.
    #[error(transparent)]
    Data(#[from] DataError),
    /// A setting out of range (zero particles, non-positive tolerance, ...).
    #[error("invalid run settings: {0}")]
    InvalidSettings(String),
    /// The first generation could not be placed in fissionable material.
    #[error("only {got} of {wanted} source neutrons could be placed in fissionable material; is anything in the model fissionable?")]
    SourceUnderfilled {
        /// Placed.
        got: usize,
        /// Wanted.
        wanted: usize,
    },
    /// The nuclides passed do not match the model's nuclide list.
    #[error("{got} nuclides given, the model needs {wanted}")]
    NuclideCount {
        /// Given.
        got: usize,
        /// Needed.
        wanted: usize,
    },
    /// The neutron population died out.
    #[error("the neutron population died out at generation {0}")]
    PopulationDied(usize),
    /// A diagnostic could not be planned or assembled.
    #[error(transparent)]
    Diagnostic(#[from] DiagnosticError),
}

/// **The normality half of the watcher's criterion.** Needs a raffles
/// skewness/kurtosis estimator and normality test (gh #944), which does not
/// exist yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalityCheck {
    /// Not applied: #944 not yet built. Only stationarity was checked.
    NotYetAvailable,
}

/// **Was the fission source converged before tallying started?**
#[derive(Debug, Clone, PartialEq)]
pub enum Convergence {
    /// A preset's watcher ran.
    Watched {
        /// Inactive cycle (1-based) at which the window first passed at the
        /// low population; `None` if it never did before the switch.
        low_population_passed_at: Option<usize>,
        /// Inactive cycle at which the window passed at the production
        /// population; `None` = **not converged** (the cap was reached and
        /// active cycles started on an unconfirmed source).
        production_passed_at: Option<usize>,
        /// The normality check (not yet available, #944).
        normality: NormalityCheck,
    },
    /// `Custom`: no watcher; report-only analysis of the whole run
    /// (`stats::convergence::SourceConvergence`): `Some(true)` when the
    /// inactive count given was at least the recommended one, `Some(false)`
    /// when not, `None` when the traces gave no recommendation.
    NotWatched {
        /// The report-only verdict.
        inactive_sufficient: Option<bool>,
    },
}

impl Convergence {
    /// `true` when the watcher confirmed the source at the production
    /// population (or, for `Custom`, the report-only analysis agreed).
    pub fn converged(&self) -> bool {
        match self {
            Convergence::Watched {
                production_passed_at,
                ..
            } => production_passed_at.is_some(),
            Convergence::NotWatched {
                inactive_sufficient,
            } => *inactive_sufficient == Some(true),
        }
    }
}

/// The physics one transport nuclide carried (always on in this API; this
/// records it so a reader can check).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NuclidePhysics {
    /// Nuclide name.
    pub nuclide: String,
    /// Unresolved-resonance probability tables attached.
    pub urr_probability_tables: bool,
    /// DBRC attached; when not, why.
    pub dbrc: Result<(), String>,
    /// The thermal scattering law, if any.
    pub thermal_law: Option<String>,
}

/// **Everything about how a k_eff number was made.**
#[derive(Debug, Clone, PartialEq)]
pub struct RunProvenance {
    /// The preset (or custom sizes) asked for.
    pub settings: MCSettings,
    /// The seed used.
    pub seed: u64,
    /// Neutrons per inactive cycle at the start.
    pub inactive_particles: usize,
    /// Neutrons per active cycle.
    pub active_particles: usize,
    /// Inactive cycles actually run (all populations).
    pub inactive_cycles: usize,
    /// Active cycles actually run.
    pub active_cycles: usize,
    /// The convergence verdict.
    pub convergence: Convergence,
    /// Batch-means σ of k over the active cycles (correlation-aware), when
    /// at least two batches fit.
    pub sigma_batch_means: Option<f64>,
    /// `Pcm` only: whether the tolerance was reached before the cap.
    pub pcm_tolerance_reached: Option<bool>,
    /// Per transport nuclide: the physics it carried.
    pub physics: Vec<NuclidePhysics>,
    /// The nuclear-data library.
    pub data_library: String,
    /// The temperature every nuclide was processed at \[K\].
    pub temperature_k: f64,
    /// What ran it.
    pub backend: &'static str,
    /// Statements every reader should see (smoke test, unjustified
    /// thresholds, what is not counted).
    pub notes: Vec<String>,
    /// **Physics switched off on purpose** through [`super::advanced`]
    /// (`without_dbrc`, ...). Empty for every ordinary run: correct physics
    /// is on and hidden (#942).
    pub ablations: Vec<String>,
}

/// k_eff and its 1σ standard error over the active cycles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Keff {
    /// Mean over the active cycles.
    pub mean: f64,
    /// Standard error `s/√n` over the active cycles (the convention of every
    /// recorded number in this crate). It ignores cycle-to-cycle
    /// correlation; see [`RunProvenance::sigma_batch_means`].
    pub std: f64,
}

impl std::fmt::Display for Keff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.5} ± {:.5}", self.mean, self.std)
    }
}

/// **The result of a k_eff run.**
#[derive(Debug, Clone, PartialEq)]
pub struct KeffRun {
    keff: Keff,
    /// k per cycle, inactive first.
    pub k_by_cycle: Vec<f64>,
    /// Shannon entropy \[bits\] of the source each cycle left, on the
    /// watcher's mesh.
    pub entropy_by_cycle: Vec<Option<f64>>,
    /// Neutrons per cycle.
    pub particles_by_cycle: Vec<usize>,
    /// How it was made.
    pub provenance: RunProvenance,
    /// Each requested diagnostic with its result, in request order (empty
    /// for `None`).
    pub diagnostics: Vec<(MCDiagnostic, MCDiagnosticResult)>,
}

/// The first result of variant `$v`, or the "not requested" error.
macro_rules! first_result {
    ($self:ident, $v:ident, $how:expr) => {
        $self
            .diagnostics
            .iter()
            .find_map(|(_, r)| match r {
                MCDiagnosticResult::$v(x) => Some(x),
                _ => None,
            })
            .ok_or(DiagnosticError::NotRequested {
                variant: stringify!($v),
                how: $how,
            })
    };
}

impl KeffRun {
    /// k_eff ± σ.
    pub fn keff(&self) -> Keff {
        self.keff
    }

    /// **The six factors** (the first `SixFactors` requested).
    ///
    /// # Errors
    /// Not requested.
    pub fn six_factors(&self) -> Result<&SixFactors, DiagnosticError> {
        first_result!(
            self,
            SixFactors,
            "Diagnostics::default(), or MCDiagnostic::six_factors(..)"
        )
    }

    /// The six factors split at exactly these edges.
    ///
    /// # Errors
    /// No `SixFactors` at those edges was requested.
    pub fn six_factors_with_edges(
        &self,
        thermal_edge: uom::si::f64::Energy,
        fast_edge: uom::si::f64::Energy,
    ) -> Result<&SixFactors, DiagnosticError> {
        self.diagnostics
            .iter()
            .find_map(|(d, r)| match (d, r) {
                (
                    MCDiagnostic::SixFactors {
                        thermal_edge: t,
                        fast_edge: f,
                    },
                    MCDiagnosticResult::SixFactors(x),
                ) if *t == thermal_edge && *f == fast_edge => Some(x),
                _ => None,
            })
            .ok_or(DiagnosticError::NotRequested {
                variant: "SixFactors",
                how: "MCDiagnostic::six_factors(thermal_edge, fast_edge) with these edges",
            })
    }

    /// **The spectrum in `region`** (the first one requested there).
    ///
    /// # Errors
    /// No spectrum was requested in that region.
    pub fn spectrum(&self, region: Region) -> Result<&Spectrum, DiagnosticError> {
        self.diagnostics
            .iter()
            .find_map(|(_, r)| match r {
                MCDiagnosticResult::Spectrum(s) if s.region == region => Some(s),
                _ => None,
            })
            .ok_or(DiagnosticError::NotRequested {
                variant: "Spectrum",
                how: "Diagnostics::default() for the whole model, .with_spectrum_per_component(..) for components",
            })
    }

    /// Run health.
    ///
    /// # Errors
    /// Not requested.
    pub fn run_health(&self) -> Result<&RunHealth, DiagnosticError> {
        first_result!(
            self,
            RunHealth,
            "Diagnostics::default(), or MCDiagnostic::run_health()"
        )
    }

    /// The neutron balance (the first requested).
    ///
    /// # Errors
    /// Not requested.
    pub fn neutron_balance(&self) -> Result<&NeutronBalance, DiagnosticError> {
        first_result!(
            self,
            NeutronBalance,
            "Diagnostics::default(), or MCDiagnostic::neutron_balance(..)"
        )
    }

    /// EALF.
    ///
    /// # Errors
    /// Not requested.
    pub fn ealf(&self) -> Result<&Ealf, DiagnosticError> {
        first_result!(
            self,
            Ealf,
            "Diagnostics::default(), or MCDiagnostic::ealf()"
        )
    }

    /// Leakage (the first requested).
    ///
    /// # Errors
    /// Not requested.
    pub fn leakage(&self) -> Result<&Leakage, DiagnosticError> {
        first_result!(
            self,
            Leakage,
            "Diagnostics::default(), or MCDiagnostic::leakage(..)"
        )
    }

    /// Kinetics (β_eff, Λ by IFP).
    ///
    /// # Errors
    /// Not requested.
    pub fn kinetics(&self) -> Result<&Kinetics, DiagnosticError> {
        first_result!(
            self,
            Kinetics,
            "Diagnostics::reactor_physics(), or MCDiagnostic::kinetics(..)"
        )
    }

    /// The fission map (the first requested).
    ///
    /// # Errors
    /// Not requested.
    pub fn fission_map(&self) -> Result<&MeshMap, DiagnosticError> {
        first_result!(self, FissionMap, "Diagnostics::reactor_physics()")
    }

    /// The flux map (the first requested).
    ///
    /// # Errors
    /// Not requested.
    pub fn flux_map(&self) -> Result<&MeshMap, DiagnosticError> {
        first_result!(self, FluxMap, "Diagnostics::reactor_physics()")
    }
}

impl std::fmt::Display for KeffRun {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let p = &self.provenance;
        writeln!(f, "k_eff = {}", self.keff)?;
        if let Some(s) = p.sigma_batch_means {
            writeln!(f, "  batch-means sigma (correlation-aware): {s:.5}")?;
        }
        writeln!(
            f,
            "  run: {:?}, seed {}, {} inactive cycles (from {} per cycle), {} active cycles of {}",
            p.settings,
            p.seed,
            p.inactive_cycles,
            p.inactive_particles,
            p.active_cycles,
            p.active_particles
        )?;
        let verdict = if p.convergence.converged() {
            "converged"
        } else {
            "NOT CONVERGED"
        };
        writeln!(f, "  source: {verdict} ({:?})", p.convergence)?;
        if let Some(r) = p.pcm_tolerance_reached {
            writeln!(f, "  pcm tolerance reached: {r}")?;
        }
        writeln!(
            f,
            "  data: {} at {} K; {}",
            p.data_library, p.temperature_k, p.backend
        )?;
        for n in &p.physics {
            let dbrc = if n.dbrc.is_ok() { "on" } else { "OFF" };
            let urr = if n.urr_probability_tables {
                "on"
            } else {
                "none"
            };
            let law = n.thermal_law.as_deref().unwrap_or("free gas");
            writeln!(
                f,
                "  physics {}: URR tables {urr}, DBRC {dbrc}, thermal {law}",
                n.nuclide
            )?;
        }
        for a in &p.ablations {
            writeln!(f, "  ABLATED: {a}")?;
        }
        for n in &p.notes {
            writeln!(f, "  note: {n}")?;
        }
        Ok(())
    }
}

/// The run plan a preset expands to.
struct Plan {
    seed: u64,
    start_particles: usize,
    production_particles: usize,
    watch: bool,
    fixed_inactive: usize,
    active: Active,
}

enum Active {
    Fixed(usize),
    UntilSigma { tol: f64, min: usize, max: usize },
}

fn plan(mc: MCSettings) -> Result<Plan, RunError> {
    let preset = |production_particles, active| Plan {
        seed: PRESET_SEED,
        start_particles: WATCH_PARTICLES,
        production_particles,
        watch: true,
        fixed_inactive: 0,
        active,
    };
    Ok(match mc {
        MCSettings::Auto => preset(AUTO_PARTICLES, Active::Fixed(AUTO_ACTIVE_CYCLES)),
        MCSettings::Smoke => preset(SMOKE_PARTICLES, Active::Fixed(SMOKE_ACTIVE_CYCLES)),
        MCSettings::Fast => preset(FAST_PARTICLES, Active::Fixed(FAST_ACTIVE_CYCLES)),
        MCSettings::High => preset(HIGH_PARTICLES, Active::Fixed(HIGH_ACTIVE_CYCLES)),
        MCSettings::Pcm(t) => {
            if !(t.in_pcm() > 0.0 && t.in_pcm().is_finite()) {
                return Err(RunError::InvalidSettings(format!(
                    "pcm tolerance must be positive, not {}",
                    t.in_pcm()
                )));
            }
            preset(
                PCM_PARTICLES,
                Active::UntilSigma {
                    tol: t.in_pcm() * 1.0e-5,
                    min: PCM_MIN_ACTIVE_CYCLES,
                    max: PCM_MAX_ACTIVE_CYCLES,
                },
            )
        }
        MCSettings::Custom(c) => {
            if c.particles == 0 || c.active == 0 {
                return Err(RunError::InvalidSettings(
                    "Custom needs particles > 0 and active > 0".into(),
                ));
            }
            Plan {
                seed: c.seed,
                start_particles: c.particles,
                production_particles: c.particles,
                watch: false,
                fixed_inactive: c.inactive,
                active: Active::Fixed(c.active),
            }
        }
    })
}

/// Stationarity of k and H over a window (the watcher's criterion today).
fn window_passes(k: &[f64], h: &[f64]) -> bool {
    TraceDiagnosis::analyse("k", k).converged() && TraceDiagnosis::analyse("entropy", h).converged()
}

/// The traces of one run.
#[derive(Default)]
struct Traces {
    k: Vec<f64>,
    h: Vec<Option<f64>>,
    n: Vec<usize>,
    /// The first active cycle, once the active cycles have begun.
    first_active: Option<usize>,
}

impl Traces {
    fn window(&self, from: usize) -> Option<(Vec<f64>, Vec<f64>)> {
        let k = self.k.get(from..)?.to_vec();
        let h: Option<Vec<f64>> = self.h.get(from..)?.iter().copied().collect();
        Some((k, h?))
    }
}

impl AssembledModel {
    /// The entropy mesh the watcher uses: the source box, divided
    /// [`ENTROPY_MESH_DIVISIONS`] times per side.
    pub fn entropy_mesh(&self) -> RegularMesh {
        let (l, u) = (self.source_box.lower, self.source_box.upper);
        RegularMesh {
            lower_left: [l.x, l.y, l.z],
            upper_right: [u.x, u.y, u.z],
            dimension: [ENTROPY_MESH_DIVISIONS; 3],
        }
    }

    /// **Run k_eff on already-processed nuclides** (in [`Self::nuclides`]
    /// order; [`Self::load_nuclides`] makes them). [`Model::keff`] is this
    /// after assembling and loading.
    ///
    /// # Errors
    /// [`RunError`]: invalid settings, the wrong number of nuclides, a first
    /// generation that cannot be placed, or a population that dies out.
    pub fn keff_with(
        &self,
        nuclides: &[Nuclide],
        settings: &KeffSettings,
    ) -> Result<KeffRun, RunError> {
        self.keff_with_ablations(nuclides, settings, Vec::new())
    }

    /// [`Self::keff_with`], recording `ablations` (the physics an
    /// [`super::advanced`] run switched off) in the provenance.
    pub(crate) fn keff_with_ablations(
        &self,
        nuclides: &[Nuclide],
        settings: &KeffSettings,
        ablations: Vec<String>,
    ) -> Result<KeffRun, RunError> {
        if nuclides.len() != self.nuclides.len() {
            return Err(RunError::NuclideCount {
                got: nuclides.len(),
                wanted: self.nuclides.len(),
            });
        }
        let p = plan(settings.mc())?;
        let max_active = match p.active {
            Active::Fixed(n) => n,
            Active::UntilSigma { max, .. } => max,
        };
        let max_inactive = if p.watch {
            MAX_INACTIVE_CYCLES
        } else {
            p.fixed_inactive
        };
        let low = LowLevelKeffSettings {
            n_particles: p.start_particles,
            n_inactive: 0,
            n_active: max_inactive + max_active,
            temperature_k: self.temperature_k,
            seed: p.seed,
            ..LowLevelKeffSettings::default()
        };
        let (g, m) = (&self.geometry, &self.materials);
        let mut collector = match settings.diagnostics() {
            Some(d) => Some(Collector::new(d.items(), self)?),
            None => None,
        };
        let mut it = CsgPowerIteration::new(g, m, nuclides, self.source_box, &low)
            .with_majorants(self.majorants(nuclides));
        if let Some(s) = collector.as_ref().and_then(Collector::ifp) {
            it.enable_ifp(s).map_err(RunError::InvalidSettings)?;
        }
        #[cfg(not(target_arch = "wasm32"))]
        let clock = std::time::Instant::now();
        let placed = it.source_positions().len();
        if placed < p.start_particles {
            return Err(RunError::SourceUnderfilled {
                got: placed,
                wanted: p.start_particles,
            });
        }
        let mesh = self.entropy_mesh();
        let mut tr = Traces::default();
        let print = settings.progress() == Progress::Print;
        // Inactive cycles discard their scores; active ones keep them. With
        // no diagnostics this is exactly `step` (wave 1, bit for bit).
        let step = |it: &mut CsgPowerIteration,
                    tr: &mut Traces,
                    c: &mut Option<Collector>,
                    active: bool|
         -> Result<(), RunError> {
            let n = it.population();
            let r = match c.as_mut() {
                None => it.step(g, m, nuclides),
                Some(c) => it.step_scored(g, m, nuclides, &mut c.extra).map(|(r, t)| {
                    if active {
                        c.active(&t, n, r.k);
                    } else {
                        c.inactive(&t);
                    }
                    r
                }),
            }
            .ok_or(RunError::PopulationDied(tr.k.len()))?;
            if r.bank_size == 0 {
                return Err(RunError::PopulationDied(tr.k.len()));
            }
            if active && tr.first_active.is_none() {
                tr.first_active = Some(tr.k.len());
            }
            tr.k.push(r.k);
            tr.h.push(it.source_entropy(&mesh));
            tr.n.push(n);
            if print {
                let average = tr.first_active.map(|a| &tr.k[a..]);
                println!(
                    "{}",
                    generation_line(tr.k.len(), n, r.k, tr.h[tr.h.len() - 1], average)
                );
            }
            Ok(())
        };
        let say = |line: String| {
            if print {
                println!("{line}");
            }
        };
        say(GENERATION_HEADER.to_string());

        // ── Inactive cycles ──────────────────────────────────────────────
        let mut low_passed = None;
        let mut production_passed = None;
        if p.watch {
            // Phase 1: judge at the low population. Leave room for settling
            // and one production window under the cap.
            let low_cap = MAX_INACTIVE_CYCLES - SETTLE_CYCLES - WATCH_WINDOW_CYCLES;
            while tr.k.len() < low_cap {
                step(&mut it, &mut tr, &mut collector, false)?;
                let c = tr.k.len();
                if c >= WATCH_WINDOW_CYCLES && c % WATCH_EVERY_CYCLES == 0 {
                    if let Some((k, h)) = tr.window(c - WATCH_WINDOW_CYCLES) {
                        if window_passes(&k, &h) {
                            low_passed = Some(c);
                            say(format!(
                                " watcher: k and entropy stationary at {} per cycle, cycle {c}",
                                p.start_particles
                            ));
                            break;
                        }
                    }
                }
            }
            if low_passed.is_none() {
                say(format!(
                    " watcher: not stationary by cycle {low_cap}; going on"
                ));
            }
            // Phase 2: raise the population, discard the transition.
            say(format!(
                " watcher: population raised to {}; {SETTLE_CYCLES} cycles discarded to settle",
                p.production_particles
            ));
            it.set_population(p.production_particles);
            for _ in 0..SETTLE_CYCLES {
                step(&mut it, &mut tr, &mut collector, false)?;
            }
            // Phase 3: re-confirm at the production population.
            let start = tr.k.len();
            while tr.k.len() < MAX_INACTIVE_CYCLES {
                step(&mut it, &mut tr, &mut collector, false)?;
                let c = tr.k.len() - start;
                if c >= WATCH_WINDOW_CYCLES && c % WATCH_EVERY_CYCLES == 0 {
                    if let Some((k, h)) = tr.window(tr.k.len() - WATCH_WINDOW_CYCLES) {
                        if window_passes(&k, &h) {
                            production_passed = Some(tr.k.len());
                            say(format!(
                                " watcher: stationary again at {} per cycle, cycle {}",
                                p.production_particles,
                                tr.k.len()
                            ));
                            break;
                        }
                    }
                }
            }
        } else {
            for _ in 0..p.fixed_inactive {
                step(&mut it, &mut tr, &mut collector, false)?;
            }
        }
        let n_inactive = tr.k.len();
        say(format!(" active cycles begin after {n_inactive} inactive"));

        // ── Active cycles ────────────────────────────────────────────────
        let mut tol_reached = None;
        loop {
            step(&mut it, &mut tr, &mut collector, true)?;
            let active = &tr.k[n_inactive..];
            match p.active {
                Active::Fixed(n) if active.len() >= n => break,
                Active::Fixed(_) => {}
                Active::UntilSigma { tol, min, max } => {
                    if active.len() >= min && correlated_sigma(active) <= tol {
                        tol_reached = Some(true);
                        break;
                    }
                    if active.len() >= max {
                        tol_reached = Some(false);
                        break;
                    }
                }
            }
        }
        let active = &tr.k[n_inactive..];
        let (mean, std) = mean_and_stderr(active);
        say(format!(
            " k = {mean:.5} +/- {std:.5} over {} active cycles{}",
            active.len(),
            match tol_reached {
                Some(false) => " (tolerance NOT reached)",
                _ => "",
            }
        ));
        let convergence = if p.watch {
            Convergence::Watched {
                low_population_passed_at: low_passed,
                production_passed_at: production_passed,
                normality: NormalityCheck::NotYetAvailable,
            }
        } else {
            let h: Vec<f64> = tr.h.iter().map(|x| x.unwrap_or(f64::NAN)).collect();
            let h = if h.iter().all(|x| x.is_finite()) {
                h
            } else {
                Vec::new()
            };
            Convergence::NotWatched {
                inactive_sufficient: SourceConvergence::from_traces(&tr.k, &h, n_inactive)
                    .inactive_sufficient(),
            }
        };
        let provenance = RunProvenance {
            settings: settings.mc(),
            seed: p.seed,
            inactive_particles: p.start_particles,
            active_particles: p.production_particles,
            inactive_cycles: n_inactive,
            active_cycles: active.len(),
            sigma_batch_means: CorrelatedMean::estimate(active).batch.map(|b| b.sem),
            pcm_tolerance_reached: tol_reached,
            physics: self.physics(nuclides),
            data_library: self.data_label.clone(),
            temperature_k: self.temperature_k,
            backend: "CsgPowerIteration, CPU single thread (bit for bit run_keff_csg at a fixed population)",
            notes: notes(settings.mc(), &p),
            ablations,
            convergence,
        };
        #[cfg(not(target_arch = "wasm32"))]
        let transport_seconds = Some(clock.elapsed().as_secs_f64());
        #[cfg(target_arch = "wasm32")]
        let transport_seconds = None;
        let diagnostics = match collector {
            None => Vec::new(),
            Some(c) => c.finish(&RunContext {
                k_by_cycle: tr.k.clone(),
                entropy_by_cycle: tr.h.clone(),
                particles_by_cycle: tr.n.clone(),
                inactive_cycles: n_inactive,
                converged: provenance.convergence.converged(),
                transport_seconds,
            })?,
        };
        Ok(KeffRun {
            keff: Keff { mean, std },
            k_by_cycle: tr.k,
            entropy_by_cycle: tr.h,
            particles_by_cycle: tr.n,
            provenance,
            diagnostics,
        })
    }

    fn physics(&self, nuclides: &[Nuclide]) -> Vec<NuclidePhysics> {
        self.nuclides
            .iter()
            .zip(nuclides)
            .map(|(k, n)| NuclidePhysics {
                nuclide: k.isotope.name(),
                urr_probability_tables: n.has_urr_probability_tables(),
                dbrc: match n.dbrc_unavailable_reason() {
                    None => Ok(()),
                    Some(why) => Err(why),
                },
                thermal_law: k.law.map(|l| format!("{l:?} (LEAPR, ENDF/B-VIII.0 deck)")),
            })
            .collect()
    }
}

/// The column heads [`generation_line`] fills, as OpenMC prints them.
const GENERATION_HEADER: &str = " Bat./Gen.  particles        k   Entropy       Average k\n \
     =========  =========  =======  ========  ==================";

/// One generation's progress line: its number, population, k, Shannon
/// entropy and, once the active cycles have begun (`average` holds them so
/// far), the running mean ± standard error, as OpenMC prints it.
fn generation_line(
    generation: usize,
    particles: usize,
    k: f64,
    entropy: Option<f64>,
    average: Option<&[f64]>,
) -> String {
    let h = entropy.map_or_else(|| "       -".to_string(), |h| format!("{h:8.5}"));
    let avg = match average {
        Some(a) if a.len() >= 2 => {
            let (m, s) = mean_and_stderr(a);
            format!("  {m:.5} +/- {s:.5}")
        }
        _ => String::new(),
    };
    format!(" {generation:>9}  {particles:>9}  {k:.5}  {h}{avg}")
}

/// The batch-means σ when at least two batches fit, else the naive one,
/// never smaller than the naive one.
fn correlated_sigma(x: &[f64]) -> f64 {
    let c = CorrelatedMean::estimate(x);
    c.batch.map_or(c.naive_sem, |b| b.sem.max(c.naive_sem))
}

fn notes(mc: MCSettings, p: &Plan) -> Vec<String> {
    let mut v = Vec::new();
    if let MCSettings::Smoke = mc {
        v.push(format!(
            "SMOKE RUN, not a result to quote: {SMOKE_PARTICLES} per cycle is Brown's trial-run size, a fifth of his \
             production minimum of 5000, and {SMOKE_ACTIVE_CYCLES} active cycles fall far short of his \"a few hundred\" \
             (LA-UR-08-06558, Conclusions)."
        ));
    }
    if let MCSettings::Fast = mc {
        v.push(format!(
            "SMOKE TEST, not a result to quote: {FAST_ACTIVE_CYCLES} active cycles fall short of Brown's \"a few hundred\" \
             (LA-UR-08-06558, Conclusions); {FAST_PARTICLES} per cycle meets his minimum of 5000."
        ));
    }
    if p.watch {
        v.push(format!(
            "watcher thresholds (window {WATCH_WINDOW_CYCLES}, every {WATCH_EVERY_CYCLES}, settle {SETTLE_CYCLES}, \
             entropy mesh {ENTROPY_MESH_DIVISIONS}^3, {WATCH_PARTICLES} per inactive cycle) are not yet justified (#927); \
             only stationarity was checked, normality needs #944."
        ));
    }
    v.push("lost histories are not counted on this run path (#699).".into());
    v
}

impl Model {
    /// **Run k_eff**: assemble, process the nuclear data at the model's
    /// temperature (correct physics on), and run the power iteration the
    /// settings describe.
    ///
    /// # Errors
    /// [`RunError`]: the model does not assemble, data is missing or fails to
    /// process, or the run cannot proceed.
    pub fn keff(&self, settings: KeffSettings) -> Result<KeffRun, RunError> {
        let a = self.assemble()?;
        let nuclides = a.load_nuclides_with(self.data(), settings.progress())?;
        a.keff_with(&nuclides, &settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The progress line carries OpenMC's columns, and the running average
    /// appears only once two active cycles exist; progress prints by default.
    #[test]
    fn progress_lines_carry_openmc_columns_and_print_by_default() {
        let s = KeffSettings::new(MCSettings::Auto, None);
        assert_eq!(s.progress(), Progress::Print);
        assert_eq!(s.silent().progress(), Progress::Silent);
        let inactive = generation_line(3, 1_000, 1.01234, Some(4.5), None);
        assert_eq!(inactive, "         3       1000  1.01234   4.50000");
        let one = generation_line(4, 1_000, 1.0, None, Some(&[1.0]));
        assert!(one.ends_with("       -"), "{one}");
        let two = generation_line(5, 10_000, 1.0, Some(4.5), Some(&[1.0, 1.02]));
        assert!(two.ends_with("1.01000 +/- 0.01000"), "{two}");
    }

    /// Presets expand to the decided numbers; invalid sizes are refused.
    #[test]
    fn presets_expand_to_the_decided_numbers() {
        let p = plan(MCSettings::Auto).unwrap();
        assert_eq!(
            (p.seed, p.start_particles, p.production_particles, p.watch),
            (1, 1_000, 10_000, true)
        );
        assert!(matches!(p.active, Active::Fixed(250)));
        let smoke = plan(MCSettings::Smoke).unwrap();
        assert_eq!(
            (smoke.start_particles, smoke.production_particles),
            (1_000, 1_000)
        );
        assert!(matches!(smoke.active, Active::Fixed(50)));
        assert!(notes(MCSettings::Smoke, &smoke)[0].contains("SMOKE RUN"));
        assert!(matches!(
            plan(MCSettings::Fast).unwrap().active,
            Active::Fixed(120)
        ));
        assert_eq!(plan(MCSettings::High).unwrap().production_particles, 50_000);
        assert!(matches!(
            plan(MCSettings::High).unwrap().active,
            Active::Fixed(1_500)
        ));
        let pcm = plan(MCSettings::Pcm(ReactivityTolerance::pcm(50.0))).unwrap();
        assert!(matches!(
            pcm.active,
            Active::UntilSigma {
                min: 250,
                max: 5_000,
                ..
            }
        ));
        assert!(plan(MCSettings::Pcm(ReactivityTolerance::pcm(0.0))).is_err());
        let c = CustomRun {
            particles: 100,
            inactive: 3,
            active: 4,
            seed: 9,
        };
        let pc = plan(MCSettings::Custom(c)).unwrap();
        assert_eq!((pc.seed, pc.watch, pc.fixed_inactive), (9, false, 3));
        assert!(plan(MCSettings::Custom(CustomRun { particles: 0, ..c })).is_err());
        let s = KeffSettings::new(MCSettings::Fast, None);
        assert_eq!((s.mc(), s.diagnostics()), (MCSettings::Fast, None));
        let s = KeffSettings::new(MCSettings::Fast, Some(Diagnostics::default()));
        assert_eq!(s.diagnostics().map(|d| d.items().len()), Some(6));
        assert!(notes(MCSettings::Fast, &plan(MCSettings::Fast).unwrap())[0].contains("SMOKE TEST"));
        assert_eq!(ReactivityTolerance::pcm(30.0).in_pcm(), 30.0);
        assert_eq!(
            Keff {
                mean: 1.0,
                std: 0.0005
            }
            .to_string(),
            "1.00000 ± 0.00050"
        );
        assert!(Convergence::NotWatched {
            inactive_sufficient: Some(true)
        }
        .converged());
        assert!(!Convergence::Watched {
            low_population_passed_at: Some(30),
            production_passed_at: None,
            normality: NormalityCheck::NotYetAvailable
        }
        .converged());
    }

    /// The correlation-aware σ is never below the naive one, and a
    /// stationary window passes while a drifting one does not.
    #[test]
    fn sigma_and_window_criteria() {
        let mut s = 7u64;
        let mut u = || {
            s = s
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (s >> 11) as f64 / (1u64 << 53) as f64
        };
        let flat: Vec<f64> = (0..300).map(|_| 1.0 + 0.01 * (u() - 0.5)).collect();
        assert!(correlated_sigma(&flat) >= mean_and_stderr(&flat).1);
        let h: Vec<f64> = (0..30).map(|_| 6.0 + 0.01 * (u() - 0.5)).collect();
        assert!(window_passes(&flat[..30], &h));
        let drift: Vec<f64> = (0..30).map(|i| 0.5 + 0.02 * i as f64).collect();
        assert!(!window_passes(&drift, &h));
    }
}
