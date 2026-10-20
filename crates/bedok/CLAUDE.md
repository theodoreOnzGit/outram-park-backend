# CLAUDE.md — bedok

**BEDOK — Broad-scope Environment for Dual-phase advanced reactor Operation
simulation Kit.** 3-D nodal-diffusion neutronics (semi-analytic nodal method)
coupled to channel thermal hydraulics, fuel-rod conduction and cross-section
feedback; the IAEA-3D, NEACRP A1/A2 and D1 benchmark cases, steady and
transient. The fidelity band above 1-D neutronics and below CFD.

The workspace root `CLAUDE.md` binds here in full. This file adds only what is
specific to this crate. Added 2026-10-02 (GitHub #503); the crate had none.

## Provenance (read before changing anything)

- A Rust translation of **Than Yan Ren's (SNRSI) MATLAB implementation**
  (`main_exec_diff3d_standalone` snapshot), made **with the author's
  permission** for open-source release under OUTRAM PARK, with institutional
  approval (`NOTICE`, `README.md`, `docs/bedok-port-scoping.md` §6). Cite the
  published paper for the method: Than, Y. R., & Xiao, S. (2026), *Energy
  Engineering* 123(9) (see `README.md`).
- **Publishing on crates.io is approved** by Than Yan Ren and Xiao SiCong
  (relayed by the maintainer, 2026-10-20). The crate is published with the
  rest of the workspace; it ships its own `LICENSE` and `NOTICE`.
- **One module per `.m` file, named after it.** Keep the original filename in
  each module's doc comment.
- **No silent repairs.** Reference defects are translated as they are,
  documented on the item that carries them and pinned by a test asserting the
  wrong behaviour (`README.md`, "Translation policy").
- `src/iapws_if97/` is a translation of a BSD-2-Clause IAPWS-IF97 MATLAB
  package; its licence text is in `NOTICE` and must stay there.

## Build and test

Always `--release` (workspace rule).

```bash
cargo check --release -p bedok --lib --tests
cargo test  --release -p bedok --lib --tests -- --test-threads=4
cargo quick-test -p bedok          # no `long-tests`; iteration only
```

### faer -> petir::sparse (2026-10-20, gh #994): what moved, and what it costs

The flux solves' sparse LU (`matlab::Decomposition`) moved from `faer` to
`petir::sparse::SparseLu`, a `no_std` port of faer 0.24.4's **simplicial**
LU with COLAMD ordering. `outram-foam-basic-lib`'s Krylov solvers were
rejected: MATLAB's `decomposition` solves directly, and an iterative solve
would move every eigenvalue at the solver-tolerance level. faer is no longer
a bedok dependency, and bedok now passes the wasm `--lib` gate.

**Numbers (measured 2026-10-20, whole lib suite before and after, one core,
`RAYON_NUM_THREADS=1`, every printed line diffed):** 265 passed / 27 ignored
both times. faer had chosen its *supernodal* path for the 3-D flux operator,
which the port does not reproduce bit for bit (rounding-level, 2.2e-15
relative on the n = 10 404 `LHS`), so exactly three printed numbers moved:

| test | quantity | faer | petir::sparse |
|---|---|---|---|
| `matlab_parity_iaea3ds` | flux max rel. diff vs MATLAB | 1.024e-12 | 1.018e-12 |
| `matlab_parity_iaea3ds` | pwrdens sum rel. diff vs MATLAB | 3.998e-14 | 4.155e-14 |
| `x1_sweep_the_nodal_update_interval` | `nodalupd = 6` row (the diverging regime) | k 297.98, 647 iterations | k 284 335.69, 663 iterations |

Unchanged at every printed digit: IAEA-3D `k_eff` 1.0290842762 (-0.0000 pcm),
the X1 frozen-nodal `k_eff` 1.0230689628 (+0.00 pcm, 211 iterations, flux-sum
rel. diff 2.928e-11), every other `nodalupd` row, the NEACRP and transient
outputs. The `nodalupd = 6` row is the chaotic regime (as in
`petir/docs/why-petir-last-ulp-evidence.md` section 4): garbage in, a
different garbage out; nothing asserts it.

**Cost: the suite took 8434 s against 1279 s (6.6x)** on the same single
core. The simplicial factorisation of the n = 10 404 operator takes 1.21 s
against faer's supernodal 0.17 s (faer's own simplicial path: 1.17-1.22 s, so
this is the algorithm, not the port); solves are as fast. The transients
(`neacrpd1t`), which refactorise on every nodal update, carry most of it. The
per-test table below predates this change and was **not** re-measured; no
test was gated. Whether to port faer's supernodal LU as well, or gate the
transients behind `long-tests`, is a maintainer decision (gh #994).

### The tests are internally parallel — do not oversubscribe (2026-10-02)

~~The flux solves run on `faer` with `rayon`, so **one test already uses every
core it can get**,~~ **CHANGED 2026-10-20:** the solves are now serial
(`petir::sparse`, above), so `--test-threads` alone sets the core count. The
2026-10-02 record follows: the flux solves ran on `faer` with `rayon`, so
**one test already used every core it could get**, and `--test-threads` does not limit that. With the harness
default (one test thread per core) a 16-core machine ran 14 of these at once
and the lib suite took **49+ minutes** (GitHub #503); the same suite takes
**1509 s at `--test-threads=4`** (measured 2026-10-02, i9-13900K, pinned to 14
then 10 cores during the run). On a shared machine pin it:

```bash
RAYON_NUM_THREADS=8 taskset -c 0-9 cargo test --release -p bedok --lib --tests -- --test-threads=4
```

### Per-test runtimes and the `long-tests` gate

The feature exists and is default-on (workspace rule,
`docs/claude-md/long-tests-and-ci.md`), but **no test is gated**, because none
reaches 5 minutes when run alone. Measured 2026-10-02 on an i9-13900K, each
test alone with `--exact --test-threads=1`, `RAYON_NUM_THREADS=12` on 14 cores
(the two `neacrpd1t` tests re-run with 8 threads on 10 cores):

| test | runtime |
|---|---|
| `neacrpd1t::tests::the_two_kinetics_schemes_agree_on_a_gentle_window` | 202 s (14 cores), 206 s (10 cores) |
| `neacrpd1t::tests::the_transient_marches_without_diverging` | 108 s (14 cores), 103 s (10 cores) |
| `criticalboron_xyz::tests::the_search_finds_a_critical_boron_on_the_pwr_case` | 99 s |
| `neacrpd1::tests::the_hem_path_heats_the_coolant_along_each_channel` | 95 s |
| `driftflux6_solverstatic1d::tests::t10_the_two_th_paths_disagree_about_quality` | 56 s |
| `neacrpd1::tests::the_coupled_loop_runs_on_the_benchmark_case` | 24 s |
| `driftflux6_solverstatic1d::tests::t10_the_corrected_quality_matches_the_hem_definition` | 23 s |
| `criticalboron_xyz::tests::x1_sweep_the_nodal_update_interval` | 19 s |
| the other five that exceeded 60 s only under oversubscription | 5–7 s each |

Every other test stayed under 60 s even in the oversubscribed run. Re-measure
before gating anything; a test that crosses 5 minutes on the machine you
measure on goes behind `#[cfg_attr(not(feature = "long-tests"), ignore =
"<runtime> on <date>/<host>; long-tests")]`, and nothing else about it changes.
