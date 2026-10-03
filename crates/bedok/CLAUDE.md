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

### The tests are internally parallel — do not oversubscribe (2026-10-02)

The flux solves run on `faer` with `rayon`, so **one test already uses every
core it can get**, and `--test-threads` does not limit that. With the harness
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
