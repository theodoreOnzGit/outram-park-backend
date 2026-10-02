//! **Statistics for Monte Carlo results** — epic GitHub #493, built on
//! RAFFLES (`raffles`, Adolphus Lye's crate).
//!
//! # The split, and why
//!
//! **The maths on numbers lives in RAFFLES; deciding what is counted, and
//! running the transport, stays here** (the rule from GitHub #500, and the
//! ownership comment on #493: simulation drivers and adaptive /
//! model-in-the-loop samplers are outside RAFFLES' declared scope). So each
//! submodule here is a thin driver or adapter: it knows what a
//! [`KeffResult`](crate::physics::keff::KeffResult), a tally or a weight-window
//! mesh is, and it hands arrays of numbers to `raffles` for the statistics.
//!
//! | module | issue | what it does | changes transport? |
//! |---|---|---|---|
//! | [`ensemble`] | #494 | runs `N` independent seeds, pools them, checks `χ²/dof` of the seeds against their own internal `σ`, flags outlier seeds | no — runs the caller's closure unchanged |
//! | [`correlated_sigma`] | #495 | autocorrelation-corrected `σ` for `k` and tally series, **alongside** the naive `σ` | no — post-processing |
//! | [`convergence`] | #496 | stationarity diagnostics on the entropy and `k` traces, recommended inactive count | no — post-processing |
//! | [`learned_importance`] | #497 | weight windows from early-cycle tallies with a RAFFLES surrogate filling unresolved cells; FOM and unbiasedness comparison | **only if a caller attaches the windows** — off by default |
//! | [`uq`] | #498 | input-uncertainty propagation and sensitivity by sampling | no — runs the caller's closure |
//! | [`sweep`] | #499 | surrogate interpolation of a parameter sweep with jackknife+ prediction intervals, held-out validation, next-run proposals | no |
//!
//! # Default-path guarantee
//!
//! **Nothing in this module is called by any driver.** A default
//! `run_keff` / `run_keff_csg*` / `run_fixed_source` run is bit-for-bit what
//! it was before the module existed: every function here either reads a
//! finished result or runs a closure the caller wrote. The one module that can
//! change transport, [`learned_importance`], only *builds* a
//! [`WeightWindows`](crate::physics::weight_windows::WeightWindows); the
//! transport changes only if a caller attaches it to a
//! [`VarianceReduction`](crate::physics::variance_reduction::VarianceReduction),
//! whose default stays analog.
//!
//! # Status
//!
//! Draft, AI-assisted, with **no V&V measured yet**: the maintainer deferred
//! all test and benchmark runs on 2026-10-03, so every V&V gate named in these
//! modules' docs reads "NOT YET MEASURED". The unit tests are written and
//! compile; they have not been run. V&V stubs live under
//! `verification_and_validation/stats_epic_493/`.

pub mod convergence;
pub mod correlated_sigma;
pub mod ensemble;
pub mod learned_importance;
