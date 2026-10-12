# AGENTS.md — raffles

Guidance for any AI agent working in `crates/raffles`, whatever tool it runs
under. `crates/raffles/CLAUDE.md` points here; the rules below bind exactly as
if they were written there.

Where this file and the workspace-root `CLAUDE.md` differ, **this file wins for
this crate**, because it records maintainer direction specific to RAFFLES. The
root file still binds for everything this file does not address.

---

## Calibration is acceptable when it is the intention

**Maintainer direction, 2026-10-12:** *"Calibration is acceptable if that's the
intention, surrogate models by definition calibrate."*

### What this means in RAFFLES

- **A surrogate model is a calibrated model, and that is not a defect.** A
  polynomial regression, a Gaussian process (kriging), a polynomial chaos
  expansion or a neural network is *defined* by fitting its coefficients to
  data. Fitting is the method, and the fitted model is the product. Do not
  treat building one as a breach of any rule against calibration, and do not
  refuse or hedge a surrogate task on those grounds.
- **Bayesian model updating is calibration by design.** TMCMC, TEMCMC, plain
  MCMC and ABC exist to infer parameters from data, so the posterior *is* the
  calibration. Calibrating model inputs against measured data with them is
  their intended use.
- **This holds when RAFFLES is used on physics outputs too.** Example: fitting
  a surrogate to Monte Carlo `k_eff` results, or inferring uncertain HTGR
  inputs (packing fraction, graphite impurity) from measured criticality data.
  When calibrating is the stated purpose of the work, it is acceptable.

### How this relates to the root `CLAUDE.md`

The root file's "Model hierarchy" rule says to run a surrogate uncalibrated
first and calibrate only afterwards, with ablation testing. Its "Get the
process right" rule forbids tuning an input until a comparison passes. Both
rules target one failure: a fit dressed up as an independent check, where the
benchmark has silently become an input.

The maintainer's direction separates that failure from intended calibration:

- **Calibration is the intention:** acceptable in RAFFLES. Say that the model
  is calibrated and what it was calibrated against. Saying so is what makes
  the intention visible.
- **The comparison is meant to be a check** (a verification or validation gate,
  a benchmark the result is judged against): tuning to make that check pass is
  still forbidden. Calibrating to the very data that is supposed to test the
  model destroys the test.
