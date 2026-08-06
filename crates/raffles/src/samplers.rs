//! Samplers — strategies that turn distributions into a set of design points.
//!
//! **UNIMPLEMENTED.** This module is an empty placeholder. Nothing is exported
//! from it yet, so there is no public path a caller can reach. Do not describe
//! any sampler as available until it appears here with its verification test.
//!
//! ## Scope — what belongs here
//!
//! - **Monte Carlo** — independent random draws from the joint input
//!   distribution (RAVEN's `Samplers/MonteCarlo.py`).
//! - **Latin hypercube / stratified** — one draw per equiprobable stratum per
//!   variable, randomly paired across variables. This is RAVEN's `Stratified`
//!   sampler; the name difference is worth keeping in mind when reading
//!   upstream.
//! - **Grid** — full-factorial sampling on a tensor grid of per-variable
//!   levels, specified either as values or as CDF quantiles (RAVEN's
//!   `Samplers/Grid.py`, with `GridEntities.py` behind it).
//! - **Factorial and response-surface designs** — fractional factorial,
//!   central composite, Box-Behnken (`FactorialDesign.py`,
//!   `ResponseSurfaceDesign.py`), if the crate owner wants them.
//! - **The random number generator and its seeding**, since reproducibility of
//!   a design is a property of the sampler, not of a distribution.
//!
//! A sampler's output is a set of points in input space plus, where the method
//! defines one, a weight per point. Nothing more.
//!
//! ## What does NOT belong here
//!
//! - **The distributions.** They live in [`crate::distributions`]; a sampler
//!   consumes their inverse CDFs.
//! - **Running the model.** RAFFLES does not launch simulations, schedule
//!   jobs, or manage run directories — that is RAVEN's application layer and
//!   it is out of scope. The caller evaluates their own model at the points a
//!   sampler returns.
//! - **Post-processing the results.** Statistics and importance measures
//!   belong in [`crate::sensitivity`].
//! - **Adaptive / model-in-the-loop sampling** (RAVEN's adaptive samplers,
//!   dynamic event trees, limit-surface search). Out of scope for now: those
//!   need the model evaluation loop this crate deliberately does not own.
//!
//! ## A note on Sobol sampling
//!
//! RAVEN has a `Samplers/Sobol.py`, but it builds a Sobol *sparse-grid
//! decomposition* for surrogate construction — it is not the same thing as the
//! Sobol index estimator in [`crate::sensitivity`], and it is not the Sobol
//! low-discrepancy sequence either. Keep the three apart when porting, and say
//! in each file's doc comment which one it is.
//!
//! ## Design
//!
//! One `Sampler` enum with a variant per concrete strategy, dispatched by
//! `match` — never `Box<dyn Sampler>`. No lifetime parameters: a sampler owns
//! its distributions (or shares them with `Arc`), it does not borrow them.
//!
//! ## Verification requirement
//!
//! A sampler is not done until its design is checked against a property that
//! holds independently of the implementation: for Latin hypercube, that every
//! variable has exactly one point per stratum; for grid, that the point count
//! and coordinates match the tensor product exactly; for Monte Carlo, that
//! sample moments converge to the analytic moments at the expected `1/sqrt(N)`
//! rate, and that a fixed seed reproduces a design bit-for-bit. Record the
//! methodology *and* the measured numbers.
//!
//! ## Dependency note
//!
//! Sampling needs a random number generator, and RAFFLES has no RNG dependency
//! yet. When one is added it goes into the root `[workspace.dependencies]`
//! (single source of truth for versions), must be pure Rust so the crate stays
//! Termux-buildable, and must support explicit seeding for reproducibility.
//!
//! ## Provenance
//!
//! No RAVEN code has been ported into this module. When it is, each derived
//! file carries the attribution header shown in the crate `CLAUDE.md`, naming
//! the upstream file under `ravenframework/Samplers/`, the commit, the
//! copyright holder and the licence.
