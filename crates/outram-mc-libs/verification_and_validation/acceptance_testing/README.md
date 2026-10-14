# Acceptance testing: the NJOY port and Outram MC in their users' environments

**Class:** acceptance testing (NUREG/BR-0167's fourth test level, after unit,
integration and qualification): the software working in the environment its
users run it in. **No human V&V sign-off.** This is an AI-assisted draft under
`RESPONSIBLE_USE.md`, for research, education and V&V only; not for any
operational, licensing or safety use.

Tracked on GitHub #859. The code under test is the Rust NJOY port
(`njoy-outram-park-fork`) feeding Outram MC (`outram-mc-libs`).

## The environments

| # | environment | how it is tested | result |
|---|---|---|---|
| 1 | command line, Linux | CI job `k-eff / ubuntu-latest` | **PASS**, all four cases (run 38419666325) |
| 2 | command line, Windows | CI job `k-eff / windows-latest` | **PASS**, all four cases (run 38419666325) |
| 3 | command line, macOS | CI job `k-eff / macos-latest` | **PASS**, all four cases (run 38419666325) |
| 4 | Android | CI job `k-eff / android emulator (x86_64)`: the same driver, cross-compiled and run under the Android emulator | **PASS**, all four cases (run 38419666325); an emulator, not a phone |
| 5 | web browser | the Monte Carlo demo, built and published by the `pages` workflow; checked in headless Chromium | **PASS** in headless Chromium, with the limits listed under "Web browser" |

**Re-run 2026-10-14.** Run 38476700995, on `5286ff93d5` (the commit that
added the source entropy and the cited benchmark values), passed all four
cases in all four CI environments, with every `k` and standard error equal to
run 38419666325's to the six printed decimals.

Workflow: `.github/workflows/acceptance-tests.yml`. Scripts:
`scripts/acceptance-keff.sh`, `scripts/acceptance-android.sh`.

**Two tiers (maintainer, 2026-10-14).** On a push to `develop` the workflow
is a smoke test: the code builds and runs on every platform and gives a sane
`k`, with the 200 pcm code-to-code allowance as the bar. A pull request into
`main` runs the same workflow with no path filter, beside `full-tests.yml`,
which runs the whole test suite there. Neither is validation: the 50 pcm
criterion at full statistics is qualification testing.

## Methodology

### What is run

The four ICSBEP cases of the five-route study
([`../icsbep/five_route_keff_2026_09_29.md`](../icsbep/five_route_keff_2026_09_29.md)),
on its route 4: ENDF/B-VIII.0 tapes from `reference-data/endf/`, processed by
the Rust NJOY port (RECONR, BROADR, unresolved-resonance tables, resonance
elastic scattering, the H in H₂O thermal law), transported by Outram MC.

| case | benchmark | model |
|---|---|---|
| `godiva` | HEU-MET-FAST-001 | `outram_mc_libs::vv::godiva`, bare sphere |
| `jemima` | IEU-MET-FAST-002 | `outram_mc_libs::vv::jemima`, four-cell cylinder |
| `hst009` | HEU-SOL-THERM-009 case 1 | `outram_mc_libs::vv::hst009`, three spheres |
| `lct008` | LEU-COMP-THERM-008 case 1 | the committed benchmark cards, parsed at run time; 11-nuclide tier |

The driver is `examples/icsbep_five_route_keff.rs` with `--accept`, the
program the five-route campaign itself ran. The web demo builds the same
models from the same modules.

### Reduced statistics

Every case runs 5000 neutrons × [40 inactive + 120 active] per seed, with 16,
24, 16 and 16 seeds. For Godiva, Jemima and HST-009 that is the campaign's own
run size, with fewer seeds than its 32 to 288. Seed counts were sized from the
campaign's seed-to-seed standard deviations so that three standard errors sit
well inside the allowance. A first local trial with 12 Jemima seeds gave a
standard error of 58 pcm, too close to the limit, so Jemima went to 24.

**The lattice does not run at its campaign size** (10 000 × [250 + 400]).
Maintainer direction, 2026-10-13: *"particles per batch is too high for
acceptance. Make it like the others, this is not validation but acceptance. I
just want to see it run on all platforms."* The first CI runs at the campaign
size spent about 20 of each desktop job's 40 minutes on the lattice. At the
smaller size its fission source is not fully converged, and that shows: see
the results.

### Pass criterion

For each case, with `k` the mean over the seeds and `k₁` the recorded
**route-1** value (OpenMC on NJOY2016 data), read from the committed
`five_route_keff/data/summary_keff.csv`:

- **PASS** if `|k − k₁| ≤ 200 pcm`;
- **UNRESOLVED**, which fails, if three combined standard errors exceed
  200 pcm (the run could not have told a pass from a fail);
- **FAIL** otherwise.

**Why the comparison is with OpenMC + NJOY2016 and not with k = 1.** The
issue left this open. The 200 pcm allowance is the reduced-statistics form of
the study's 50 pcm *code-to-code* criterion, so it is judged against the same
reference. And the benchmark value cannot be the reference for a reason that
has nothing to do with this code: on this data OpenMC itself gives Jemima
−406 ± 17 pcm ~~and the lattice +244 ± 5 pcm from k = 1. A check against 1
would fail both~~ and HEU-SOL-THERM-009 +334 ± 22 pcm from the benchmark k. A
200 pcm check against the benchmark would fail both on any correct transport
code. **This is a maintainer decision made by default here; it is flagged on
#859.**

**CORRECTED 2026-10-14: the benchmark values.** The sentence above first
measured from k = 1, with no cited uncertainty for three cases. The benchmark
k and its uncertainty are now cited for all four, as the handbook values
tabulated in two open secondary sources (the handbook itself is licence
restricted): Mosteller, *An Expanded Criticality Validation Suite for MCNP*, LA-UR-10-06230 (2010), and van der Marck, *Benchmarking ENDF/B-VII.0*, Nucl. Data Sheets 107 (2006) 3061. The maintainer checked the rows against the source
pages on 2026-10-13.

| case | benchmark k | source |
|---|---|---|
| Godiva, HEU-MET-FAST-001 | 1.0000 ± 0.0010 | Mosteller 2010; van der Marck 2006 |
| Jemima, IEU-MET-FAST-002 | 1.0000 ± 0.0030 | Mosteller 2010; van der Marck 2006 |
| HEU-SOL-THERM-009 case 1 | 0.9990 ± 0.0043 | van der Marck 2006 |
| LEU-COMP-THERM-008 case 1 | 1.0007 ± 0.0016 | Mosteller 2010 |

Against these, route 1 (OpenMC + NJOY2016) sits at +16 ± 21, −406 ± 17,
+334 ± 22 and +174 ± 5 pcm: Godiva and HEU-SOL-THERM-009 inside their bands,
Jemima and the lattice outside (the lattice by 14 pcm). That comparison is
validation and belongs to qualification testing. The acceptance criterion is
unchanged by it.

**The 50 pcm criterion is not tested here.** It belongs to the
full-statistics qualification runs (the five-route campaign), which take
hours.

## Results

### Command line and Android

**Run:** GitHub Actions run 38419666325, 2026-10-13, commit `0a39f220b7`;
local run the same day on the same tree. Each row is `k − k₁` in pcm, ± the
combined standard error; allowance 200 pcm.

| case (seeds) | Linux, CI | Windows, CI | macOS arm64, CI | Android emulator, CI | Linux, local |
|---|---|---|---|---|---|
| Godiva (16) | −9.4 ± 42.6 PASS | −9.4 ± 42.6 PASS | −9.4 ± 42.6 PASS | −9.4 ± 42.6 PASS | −9.4 ± 42.6 PASS |
| Jemima (24) | +6.4 ± 36.9 PASS | +6.4 ± 36.9 PASS | +6.4 ± 36.9 PASS | +6.4 ± 36.9 PASS | +6.4 ± 36.9 PASS |
| HST-009 (16) | −25.2 ± 41.8 PASS | −26.1 ± 41.6 PASS | −26.1 ± 41.6 PASS | −26.1 ± 41.6 PASS | −25.2 ± 41.8 PASS |
| LCT-008 (16), reduced size | +70.2 ± 28.4 PASS | +70.2 ± 28.4 PASS | +70.2 ± 28.4 PASS | +70.2 ± 28.4 PASS | +70.2 ± 28.4 PASS |

| environment | hardware | threads | data + transport, four cases | job wall time |
|---|---|---|---|---|
| Linux, CI | AMD EPYC 9V45, 4 logical cores, 16 GB, ubuntu-latest, CPU only | 4 | 258 + 910 s | 22 min |
| Windows, CI | Intel Xeon Platinum 8573C, 4 logical cores, 16 GB, windows-latest, CPU only | 4 | 430 + 1438 s | 37 min |
| macOS, CI | Apple M1 (virtual), 3 cores, macos-latest, CPU only; RAM not printed | 3 | 363 + 973 s | 27 min |
| Android, CI | x86_64 emulator, API 30, "Android virtual processor", 4 cores, 6 GB, on an ubuntu-latest runner | 4 | 462 + 1902 s | 42 min |
| Linux, local | Intel Core i9-13900K, 32 logical cores, 62 GB, Linux 7.2, CPU only, shared | 14 | 173 + 200 s | 6 min |

**Reading it.**

- **Every environment passes every case**, and every check resolved its
  allowance (three standard errors at most 128 pcm).
- **The platforms agree with each other far more closely than the allowance
  asks.** Godiva, Jemima and the lattice give the same mean `k` to the
  printed digit on all five. HST-009 differs by 0.9 pcm between Linux and the
  other three, the mark of a different maths library in a thermal case
  (GitHub #578), not of a different answer.
- **Expected beforehand**, from the campaign's route-4 offsets: Godiva
  −26 ± 23, Jemima +15 ± 32, HST-009 −36 ± 39. Measured: −9, +6, −25. (The
  expectation was not written anywhere a reader can check before the runs;
  it is the campaign's own table.)
- **The lattice at the reduced size is biased, as expected.** +70 ± 28 pcm,
  where the campaign size gives −8 ± 9 on the same route: about +80 pcm from
  running 40 inactive generations where this 1.5 m core's source wants about
  250. The other session's prediction before its run was "PASS, bias under
  100 pcm". It is inside the allowance, it is the same on every platform, and
  **it is not a number to quote for the lattice's `k`**; the campaign's is.
- **At the campaign size** the lattice was run to completion locally only
  (6 seeds, 14 threads, tree of `c2613d5f28`): −48.5 ± 24.0 pcm, PASS. The CI
  run at that size (38409956147) was cancelled by a later push with the
  lattice at seed 5 of 6 on Linux and Windows; its other three cases had
  passed on all four environments with the numbers above.

**Earlier runs, for the history.** Run 38409181646 (`c2613d5f28`) failed on
Linux and macOS in 40 s with exit 126: the scripts had been committed without
the executable bit. Nothing was computed. Fixed in `c6dc726070`.

### Web browser

**Protocol (2026-10-13).** The demo was built with
`crates/dhoby-ghaut/web/monte_carlo/build.sh` at the tree of `c2613d5f28`, served from a
local static server, and opened in headless Chromium 154.0.8037.97
(1280 × 800, software rendering) with one browser profile kept between
visits, so the second visit to a rung finds what the first stored. The page
title carries the state (loading step, neutrons shown, generation, running
`k`), and the console log carries where the data came from; both were read
through the browser's debugging port. Hardware: Intel Core i9-13900K, 32
logical cores of which the tab's worker uses one, 62 GB RAM, Linux 7.2, no
GPU used; the machine was shared with other builds, so the seconds are
indicative only. ~~Nothing here was checked by eye on a screen or a phone.~~
**Corrected 2026-10-14:** the rows below were read through the debugging
port, not by eye; the maintainer has since checked the demo by eye on an
Android phone, see "Checked on a phone" below.

| rung | what was checked | result |
|---|---|---|
| Jemima, Watch, first visit | recorded tracks play while the data load; then live tracks | recorded tracks from 4 s; 3 tapes processed in the tab in 55 s and saved to the cache; live tracks from 58 s |
| Jemima, Watch, second visit | processed data come from the browser's cache | "all 3 from this browser's cache"; live tracks at 3 s |
| Jemima, Run k_eff, default 5000 × [40 + 120], seed 1 | data at NJOY's tolerance in the tab, then the run | data 170 s, transport 48 s; **k = 0.99786 ± 0.00174**. The native run of the same rung (`--headless-keff-rung jemima`) prints 0.99786 ± 0.00174: the same to every printed digit |
| HEU-SOL-THERM-009, Watch | twelve tapes including the thermal law | 3 from cache (the uranium tapes Jemima stored), 9 processed now; live tracks at 52 s |
| HEU-SOL-THERM-009, Run k_eff, default, seed 1 | as Jemima | transport 59 s; **k = 1.00111 ± 0.00162** (record, route 4: 1.00198 ± 0.00032; one run scatters by 181 pcm) |
| LEU-COMP-THERM-008, Watch | the lattice, twelve tapes | 11 from cache, 1 processed now; live tracks at 3 s |
| LEU-COMP-THERM-008, Run k_eff, default 5000 × [40 + 120], seed 1 (rebuilt after the default was cut to this size) | the whole-core run in a tab | data all from cache; transport 51 s; **k = 1.00047 ± 0.00155** (record, route 4, at the campaign size: 1.00236 ± 0.00008; at this size one run scatters by about 190 pcm and the source is not fully converged) |
| Godiva, Watch | recorded and live tracks | all 3 from cache; live tracks at 6 s |
| Godiva, Run k_eff, default, seed 1 | as Jemima | transport 5 s; **k = 0.99689 ± 0.00178**, the value the rung's own record of 2026-10-05 gives for the native and the browser run of that seed |

**What the five requirements of #859 come to, per rung.** Geometry drawn:
all four (`McRung::draw`; review images in
`crates/dhoby-ghaut/examples/monte_carlo_web/geometry/`). Random-walk tracks:
all four. Processed data cached in the browser and reused: all four (table
above). Recorded tracks shipped with the page: all four
(`<rung>/data/tracks.bin.z`, 60 / 60 / 30 / 30 histories, pinned by
`the_shipped_tracks_are_whole_histories_inside_their_geometry`). Run settings
chosen by the reader (neutrons per generation, inactive and active
generations, seed): all four, in the side panel or by link
(`?particles=&inactive=&active=&seed=`).

**Not done.** No check on a real phone or at phone width for the new rungs
(they use the demo's existing layout unchanged). The lattice was not run
in a browser at its campaign size. A CSG rung reports ~~no source entropy and~~
no leak / capture / fission counts, which the Godiva rung does (**corrected
2026-10-14:** the source entropy is now reported, see below; the phone-width
check is now done for Jemima's run view, in headless Chromium at 390 × 800,
~~still not on a real phone~~ and, since then, on a real phone, see the next
paragraph).

**Checked on a phone, 2026-10-14.** The maintainer reports having checked the
browser demo on their own Android phone. This is the maintainer's check by
eye, reported in conversation: no title, log or k value from the phone was
captured here, and the phone model, the browser and which of the four rungs
and modes were opened are not recorded. Earlier the same day the maintainer
reported that the Jemima rung works. It covers the browser only: the
command-line driver was not run in Termux on the phone (see "Android is an
emulator" below).

**Added 2026-10-14: the run view, source entropy and benchmark values.**
At the maintainer's request Run k_eff now shows, on all four rungs, the case
geometry with neutrons walking through it beside the k plot, the entropy plot
and the console (above them on a narrow screen), and the three CSG rungs
report the Shannon entropy of the fission source
(`CsgPowerIteration::source_entropy`, 5 × 5 × 5 mesh; pinned by
`the_csg_source_entropy_is_bounded_and_reading_it_changes_nothing` in
`tests/tracking_trace.rs`). Each drawn neutron is one extra traced history
born at a site of the run's present fission source; it is not one of the
run's own histories and is not counted in k. The "experiment" band of each
rung is now the cited benchmark value (table under Methodology).

Protocol as above, same machine and browser, tree after `f5c30d2a1b`, all data
from the browser's cache. Screenshots were taken through the debugging port
mid-run and at the end and were looked at by the AI assistant, not by a
person.

| rung, Run k_eff, default 5000 × [40 + 120], seed 1 | result | entropy at the end | neutrons drawn |
|---|---|---|---|
| Godiva | k = 0.99689 ± 0.00178 in 5 s | 6.131 bits | 9 |
| Jemima | k = 0.99786 ± 0.00174 in 30 s | 5.696 bits | 3 |
| HEU-SOL-THERM-009 | k = 1.00111 ± 0.00162 in 80 s | 6.488 bits | 3 |
| LEU-COMP-THERM-008 | k = 1.00047 ± 0.00155 in 81 s | 6.008 bits | 22 |

All four `k` are the same, to every printed digit, as in the table of
2026-10-13 above, so drawing the extra neutrons and reading the entropy does
not disturb the run. The seconds are longer than then for the two thermal
cases; the machine was running test suites at the same time. On the lattice
the entropy falls from 6.3 to 6.0 bits over the 40 inactive generations and
is still drifting when the active ones start, which is the unsettled source
the record already names as the reason this run size reads high. Few
neutrons are drawn on Jemima and the solution because each is animated at
the Watch view's speed. At 390 × 800 (Jemima) the geometry sits above the two
plots and the console, and all four are on screen.

## What this does and does not establish

- **Establishes:** the same source builds and runs on the three desktop
  operating systems and on Android, and gives a `k` for each of the four
  cases that agrees with the recorded OpenMC + NJOY2016 value within 200 pcm,
  with the run able to resolve that. The browser processes the same tapes
  and transports the same models.
- **Does not establish:** agreement at the 50 pcm level (the campaign's
  job), validation against the experiments (~~three of the four cases have no
  citable handbook uncertainty here~~ **corrected 2026-10-14:** all four
  benchmark uncertainties are now cited, see above; Jemima and
  HEU-SOL-THERM-009 sit hundreds of pcm from their benchmark k on both codes,
  the second inside its ± 430 pcm band), or anything about a power reactor.
- **Android is an emulator.** x86_64 Android under the emulator has a real
  Android kernel and the Bionic C library, which a Termux binary also runs
  on. It is not a run inside the Termux app on a physical aarch64 phone.
  Whether that is required is a maintainer decision, flagged on #859.
