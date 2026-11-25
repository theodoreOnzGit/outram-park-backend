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

## The suffix property on other evaluations (2026-11-24, later)

Same throwaway check, ENDF/B-VIII.0 at 293.6 K, tolerances 0.001: **every
section of every nuclide is a suffix of its union grid.**

| nuclide | union grid points | sections, all suffix |
|---|---|---|
| H-1 | 631 | 3 |
| H-2 | 542 | 4 |
| Be-9 | 644 | 13 |
| B-10 | 941 | 53 |
| C-12 | 1,615 | 22 |
| O-16 | 3,081 | 66 |
| F-19 | 2,322 | 33 |
| Na-23 | 3,556 | 26 |
| Al-27 | 5,563 | 134 |
| Si-28 | 7,447 | 61 |
| Cr-52 | 31,549 | 21 |
| Fe-56 | 42,087 | 79 |
| U-236 | 24,056 | 53 |

Cu-63 could not be checked: its tape fails to process (`MF=32 MT=151`, a
non-integer in an integer field, which NJOY2016 also stops on), unrelated to
this change.

## Profile after the change

The gh:#1292 method, repeated on both builds: `gdb` stack samples every ~0.4 s
(`bt 40`), keeping only samples inside `run_keff`, tallied by the **innermost
frame** (inlined callees count to their caller). Godiva, single thread, 5000 x
[40 + 1500] so that transport lasts long enough to sample. Same host as above;
the two builds were profiled at different times, each alone on the machine
(sampling slows the run, so these transport times are not timings).

| innermost frame | before (146 samples) | after (77 samples) |
|---|---|---|
| `ReconrResult::eval_section` (its `partition_point` and `eval_lin_lin` inlined) | **67.1 %** | 3.9 % |
| `nuclide::eval_section_x` | 3.4 % | - |
| `log_grid::eval_section_located` (interpolation) | - | **29.9 %** |
| `UnionGrid::locate` | - | **18.2 %** |
| `::at` (a tabulated law) | 6.8 % | 10.4 % |
| `sample_ct_table_indexed`, `sample_mf4_mu_cm` (angular laws) | 6.8 % | 13.0 % |

So the cross-section lookup went from ~70 % of transport to ~52 % of a
transport that is 1.9x shorter: the lookup itself takes about **2.6x less
time**. Both long runs gave `k = 0.999492`, bits `3feffbd674d57da0`, before and
after: bit identity again, over 1540 generations.

**What is left, and the next step.** `locate` is now 18 %. It takes one `ln`
(PETIR's) per nuclide per lookup. OpenMC computes the log-bin index **once per
particle energy** for every nuclide (`i_log_union`, from the library-wide
`E_min` and `log_spacing`) and passes it down. Doing the same needs a
library-wide grid and a change to the call chain that picks a collision nuclide;
filed as a follow-up, gh:#1732. **Done 2026-11-25, differently; see below.**

## One `ln` per energy (gh:#1732, 2026-11-25)

**What changed.** `log_grid::shared_ln(e)` keeps the last energy and its
`ln` per thread (a one-entry memo keyed on the bits of `e`; without `std` it
computes `ln` each time). `UnionGrid::locate` takes the bin as
`(ln e - ln E_min) * (1 / spacing)` from it, so the `ln` is taken once per
particle energy, not once per nuclide. Nothing is threaded through the
`Material` port functions, so the port stays as it was (this crate's
"the port is the oracle" rule). The bracket is still verified on every
lookup: the bin, then its two neighbours, then a full search. A memo returns
the same double `r_ln` would, so no value can change; the unit test checks
`shared_ln == r_ln` bit for bit.

**Why not OpenMC's library-wide `i_log_union`.** It needs one energy range for
every nuclide and a new argument down the call chain that picks a collision
nuclide, which is port code. The memo gives the same saving (one `ln` per
energy) without touching it. The per-nuclide bin width stays as in gh:#1295.

**Bit identity.** `tests/log_grid_lookup_1295.rs`,
`tests/log_grid_routes_1295.rs` (all four CPU routes),
`tests/speed_tier_fast_is_exact.rs`, `tests/total_fast_path_matches_full.rs`
and the lib tests pass unchanged. Every timing run below gave the same k bits
on both builds (`3feffed8b36828a3` single-thread, `3ff007f9f44a37af` on 3
threads), the same bits as gh:#1295's before and after.

**Predicted before measuring** (on gh:#1732): transport 10-15 % shorter, a
1.10-1.17x speed-up, from removing most of `locate`'s 18 %.

**Measured.** Same driver and case as above (Godiva HIGH tier, 5000 x
[40 + 120], `run_keff`, transport time only). after = gh:#1295 (`df8eb4334`
code), after1732 = this change (`6b18ac8c8`). Five repetitions per arm,
interleaved, on a machine doing nothing else.

**Hardware:** Intel Xeon Processor @ 2.10 GHz, 4 logical cores, 15 GB RAM,
Linux 6.18.44 (`-fc-v880`), CPU only; a cloud container, load average 0.7
before the runs. This is a different container from the gh:#1295 timing
above (the same build runs about 5 % faster here), so compare only the ratios
within each table.

| threads | gh:#1295, s | gh:#1732, s | mean | mean | speed-up | paired difference |
|---|---|---|---|---|---|---|
| 1 | 3.42, 3.79, 3.63, 3.40, 3.73 | 3.70, 3.34, 3.83, 3.12, 3.33 | 3.59 | 3.46 | 1.04x | -0.13 +- 0.15 s (0.8 sigma) |
| 3 | 1.61, 1.36, 1.36, 1.34, 1.35 | 1.37, 1.27, 1.29, 1.18, 1.31 | 1.40 | 1.28 | **1.09x** | -0.12 +- 0.04 s (3.3 sigma) |

**The prediction was not met.** On 3 threads the gain is -8.5 %, resolved,
just under the predicted 10-15 %. Single-thread it is -3.6 %, not resolved
from zero at five repetitions (run-to-run scatter here is 5-8 %). So `ln`
was a smaller part of `locate`'s 18 % than assumed: the rest is the bracket
check and the `partition_point` inside the bin, which this change does not
touch. Ratios hold for this host only. An earlier partial run, cut short by a
container restart and possibly on another host, is not quoted.
