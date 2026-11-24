# OpenMC's logarithmic union-grid index in `outram-mc-libs` (gh:#1295, 2026-11-24)

**What changed.** `SpeedTier::Fast` (the default) now finds a HIGH-tier
nuclide's energy **once per lookup** on its union energy grid, with OpenMC's
logarithmic index, and reads every reaction at that position. It used to run a
binary search on each reaction's own table, some 70 per U-238 collision; the
Godiva profile on gh:#1292 put about 70 % of transport in that lookup.
`SpeedTier::Standard` keeps the per-reaction search, unchanged, as the oracle.

Code: `crates/outram-mc-libs/src/material/log_grid.rs` (`UnionGrid`,
`eval_section_located`), wired into the `Fast` paths of `material/nuclide.rs`
(`total_at_energy`, `xs_at_energy`, the absorption sum, `nu_fission`, the
"other" channels, `sample_inelastic`, `sample_other_emission`).

## Upstream

OpenMC `8f79d9d0`, read: `Nuclide::init_grid` (`src/nuclide.cpp:499-530`),
`simulation::log_spacing` (`src/simulation.cpp:905-915`), `n_log_bins = 8000`
(`src/settings.cpp:119`), and the grid search of `Nuclide::calculate_xs`
(`src/nuclide.cpp:721-746`). Two deliberate differences, both for bit identity
with the oracle:

- **The interpolation is `eval_lin_lin`'s**, `y0 + (y1 - y0)(x - x0)/(x1 - x0)`,
  not OpenMC's `(1 - f) y0 + f y1`, which rounds differently.
- **Per-nuclide bounds.** OpenMC bins over the library's `E_min`/`E_max`; here
  each nuclide uses its own grid's ends. The bracket is verified on every
  lookup, so the bounds only set the bin width.

## Why it can be exact: the data's own structure, measured

RECONR and BROADR write every section on the material's union grid, a threshold
section starting later, so each section is a **suffix** of the union grid
(OpenMC's `threshold_idx` layout). Measured before writing any code,
ENDF/B-VIII.0 at 293.6 K, tolerances 0.001:

| nuclide | union grid points | sections | suffix of the union grid | not a slice | repeated energies |
|---|---|---|---|---|---|
| U-235 | 76,027 | 87 | 87 | 0 | 0 |
| U-238 | 155,207 | 52 | 52 | 0 | 0 |
| U-234 | 25,393 | 53 | 53 | 0 | 0 |

`UnionGrid::new` checks the suffix property **per section, bit for bit**; a
section that fails it keeps the per-reaction search. `locate` returns the count
of union energies `<= e`, the same count `partition_point` gives, verifies its
bracket, and falls back to a full search if the log-bin guess were ever wrong.
Below, above or at the ends of a section, and for thresholds, the lookup hands
over to the existing `eval_section_x`, so every case returns the same double.

## Verification (bit identity, `Fast` against `Standard`)

| test | what | result |
|---|---|---|
| `material::log_grid::tests` | synthetic grid: grid points, midpoints, both ends, a non-suffix section | identical |
| `tests/log_grid_lookup_1295.rs` | U-238 and U-235: every 7th union energy exactly, the doubles either side, midpoints, both ends; every `MicroXS` field, `total_at_energy`, 16 `sample_inelastic` draws | **identical**: U-238 2,394,792 values, U-235 1,173,096 |
| `tests/log_grid_routes_1295.rs` | HIGH-tier Godiva, one seed, k on `run_keff` single, `run_keff` multi(3), `run_keff_csg_par`(3), `run_keff_delta_par_in`(3) | **identical k_mean and k_std on all four routes** |
| `tests/speed_tier_fast_is_exact.rs` (existing) | U-238, U-234, F-19, H-1 free gas and with `c_H_in_H2O`, nuclide selection | identical |
| `tests/total_fast_path_matches_full.rs` (existing) | total fast path against the full set | identical |

## Timing

**Predicted before measuring** (on gh:#1295, 2026-11-24): transport time down
35-50 %, a 1.5-2x speed-up, on Godiva single-thread; possibly more on several
threads.

**Measured.** Godiva, HIGH tier, ENDF/B-VIII.0 U-234/235/238 (`vv::godiva`),
5000 histories x [40 inactive + 120 active], `run_keff`. The same driver was
built twice: before = `2637457c0` (the commit before this change), after =
this change. Runs were interleaved before/after, three repetitions per arm, on
a machine doing nothing else (load average about 1.3, from the runs
themselves). Transport time only; the nuclear-data processing is not
included.

**Hardware:** Intel Xeon Processor @ 2.10 GHz, 4 logical cores, 15 GB RAM,
Linux 6.18.44, CPU only; a cloud container, not known to be shared during the
runs.

| threads | before, s | after, s | mean before | mean after | speed-up |
|---|---|---|---|---|---|
| 1 (`CpuSingleThread`) | 6.80, 7.34, 6.72 | 3.71, 4.01, 3.56 | 6.95 | 3.76 | **1.85x** (-46 %) |
| 3 (`CpuMultiThread(Fixed(3))`) | 3.18, 2.86, 3.18 | 1.73, 1.59, 1.60 | 3.07 | 1.64 | **1.87x** (-47 %) |

k on every run: `k = 0.999859` single-thread and `1.001947` on 3 threads,
**the same bits before and after** (`3feffed8b36828a3`, `3ff007f9f44a37af`; the
two thread counts draw different streams, as `run_keff_cpu_multi` documents).

The prediction held: -46 % sits inside the predicted 35-50 %. The multi-thread
gain is the same as the single-thread one, not larger as I had thought it might
be: on this 4-core host the scattered per-reaction searches did not cost more
per thread when three threads shared the cache. Ratios hold for this host only
(`crates/outram-mc-libs/AGENTS.md`, "Every timing carries its hardware").

## Not done

- **Other evaluations** (light nuclides, moderators, structural materials) were
  not surveyed for the suffix property. Nothing depends on it holding: a
  section that is not a suffix is searched as before. Only the speed-up would
  be smaller.
- No new profile was taken after the change; the remaining share of the lookup
  is not measured.
