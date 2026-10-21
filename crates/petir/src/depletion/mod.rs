// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// ---------------------------------------------------------------------------
// PORTED from OpenMC's `openmc/deplete/` (NOT from GSL). MIT, commit
// d7d3284a1b13d7cae020e0d8fea9f1e60d91b18b; per-file provenance in each
// submodule; the notice is in NOTICE ("DERIVED WORK -- OpenMC"). PETIR's
// seventh non-GSL source, approved by the maintainer on gh:#950
// (2026-10-17): "Integrators can go in PETIR", then "All math moved to petir",
// confirmed as depletion's math only. One submodule is NOT OpenMC:
// `onix_cram.rs` ports ONIX's `onix/salameche/cram.py` (MIT; notice in NOTICE,
// "DERIVED WORK -- ONIX"), moved here from `outram-park-fork-onix` on gh:#999
// (2026-10-20): "The cram from Onix can go to petir and attribute onix".
// ---------------------------------------------------------------------------

//! **Depletion numerics** — OpenMC's burnup time integrators, its IPF CRAM
//! matrix-exponential solver and the dense burnup matrix they act on, plus
//! ONIX's partial-fraction CRAM16 ([`onix_cram`]).
//!
//! <!-- vv-unverified-banner -->
//! > ⚠️ **Unverified until validated.** Research, education and V&V only. Not
//! > for nuclear facility operation, reactor control, safety-critical, or
//! > licensing decisions.
//!
//! # What is here, and what is not
//!
//! The *math* of depletion: given a burnup matrix `A(n)` that an operator
//! supplies, advance `dn/dt = A(n) n`. The *physics* — the chain, decay
//! constants, branching, fission yields, reaction-rate tallies and the
//! assembly of `A`'s entries — belongs to the caller (in this workspace,
//! `outram-mc-libs`), behind [`TransportOperator`]. Raw `f64` throughout.
//!
//! | module | upstream | lineage |
//! |---|---|---|
//! | [`matrix`] | `Chain.form_matrix`'s output (`scipy.sparse.csc_array`) | moved from `outram-mc-libs`; dense (flagged) |
//! | [`cram`] | `openmc/deplete/cram.py` | ported in `outram-mc-libs` 2026-07-15, moved 2026-10-17; `substeps` added |
//! | [`matrix_funcs`] | `openmc/deplete/_matrix_funcs.py` | ported |
//! | [`integrators`] | `openmc/deplete/integrators.py`, `abc.py`, `pool.py` | ported |
//! | [`onix_cram`] | ONIX `onix/salameche/cram.py` (`CRAM16`, MIT) | ported in `outram-park-fork-onix` 2026-08-04, moved 2026-10-20 (gh:#999); the partial-fraction form, kept apart from [`cram`]'s IPF form |
//!
//! # The eight integrators
//!
//! | [`Integrator`] | upstream class | operator calls / step | predicted order |
//! |---|---|---|---|
//! | `Predictor` | `PredictorIntegrator` | 1 | 1 (docstring) |
//! | `CeCm` | `CECMIntegrator` | 2 | 2 (docstring) |
//! | `Cf4` | `CF4Integrator` | 4 | 4 (docstring) |
//! | `CeLi` | `CELIIntegrator` | 2 | 2 |
//! | `EpcRk4` | `EPCRK4Integrator` | 4 | 2 |
//! | `LeQi` | `LEQIIntegrator` | 2 at step 0, then 3 | 3 |
//! | `SiCeLi` | `SICELIIntegrator` | `n_steps + 1` (+1 at step 0) | 2 |
//! | `SiLeQi` | `SILEQIIntegrator` | `n_steps + 1` (+1 at step 0) | 3 |
//!
//! "Operator calls" counts the beginning-of-step evaluation the driver makes
//! ([`Integrator::operator_calls`], pinned by counting). The predicted orders
//! are for a composition-dependent `A` that does not commute with itself;
//! their derivation and the measured orders are in
//! `verification_and_validation/depletion/petir_integrators_2026_10_17.md`.
//!
//! # Every simplification and deviation, in one place
//!
//! No simplification was made **for compute speed**: there is no
//! `SIMPLIFICATION (speed)` anywhere in this module. Upstream's own redundant
//! work is kept — LE/QI re-evaluates the operator at the beginning of each
//! step after the first, although the driver has just done so
//! (`integrators.py:386`), and every special matrix re-forms its operands with
//! `form_matrix` each time, as `_matrix_funcs.py` does.
//!
//! The `DEVIATION`s, each flagged where it is made:
//!
//! 1. **Dense matrix** instead of `scipy.sparse` ([`matrix`]). Values are the
//!    same; only structural zeros become stored zeros.
//! 2. **Dense Gaussian elimination / LU** instead of SuperLU (`spsolve`,
//!    `splu`) in [`cram`]. Same answer to round-off, not to the bit; measured
//!    against OpenMC in `tests/depletion_vs_openmc.rs`.
//! 3. **CRAM degenerate inputs** (`dt == 0`, an all-zero or empty matrix)
//!    return `n0` exactly; upstream evaluates the approximation.
//! 4. **Singular pivot** is an error, not a NaN.
//! 5. **`dt ** 2`** in the LE/QI coefficients is `dt * dt`
//!    ([`matrix_funcs`]).
//! 6. **Clip keeps `-0.0`**, as `np.maximum` does ([`integrators`]).
//! 7. **`k`'s standard deviation** in the stochastic-implicit average is the
//!    independent-sum formula rather than `uncertainties`' correlation
//!    tracking; the nominal value is upstream's exactly.
//! 8. **Scope:** no restart from previous results (LE/QI and SI-LE/QI step 0
//!    is always the CE/LI start-up), no transfer rates, external sources,
//!    redox or keff search control, no MPI or multiprocessing, no results
//!    file, no unit conversion of timesteps, no `proc_time`.
//!
//! # Upstream's CF4 docstring and code disagree; this follows the code
//!
//! `CF4Integrator`'s docstring writes the last line as
//! `n_{i+1} = exp(A1/4 + A2/6 + A3/6 - A4/12) exp(-A1/12 + A2/6 + A3/6 +
//! A4/4) n_i`, which, read as an operator product, applies the second factor
//! first. Its code applies `cf4_f3` (the first factor) first
//! (`integrators.py:180-183`). The other two-exponential methods' docstrings
//! (CE/LI `:200-205`, LE/QI `:343`) agree with their code under the
//! right-to-left reading, so the CF4 docstring is the odd one out. The port
//! follows the code, which is upstream's running behaviour; the V&V record
//! measures both orders.

pub mod cram;
pub mod integrators;
pub mod matrix;
pub mod matrix_funcs;
pub mod onix_cram;

pub use cram::{cram16, cram48, ipf_cram, Complex64, CramSolver, DepSystemSolver, LuFactor};
pub use integrators::{
    AverageRates, DepletionError, DepletionIntegrator, Integrator, Keff, OperatorResult,
    StepOutcome, StepRecord, TransportOperator,
};
pub use matrix::{lincomb, DepletionMatrix, Sign};
pub use onix_cram::cram16_pfd;
