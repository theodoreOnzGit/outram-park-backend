# Side page — The four benchmark cases, run in your browser

> **Research, education and V&V only.** Not for reactor operation, licensing,
> safety decisions or emergency response
> ([`RESPONSIBLE_USE.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/RESPONSIBLE_USE.md)).
> **Review status: first draft, 2026-10-13, AI-assisted, not yet reviewed by a
> human.** Built from
> [`@@COMMIT_SHORT@@`](https://github.com/theodoreOnzGit/outram-park-backend/commit/@@COMMIT@@)
> on @@BUILD_DATE@@; every code link points at that commit.

## The question

The rungs of this ladder teach one idea each. This page asks a different
question: **does the code work where its users run it?** That is what
*acceptance testing* means in NUREG/BR-0167's four test levels (unit,
integration, qualification, acceptance). For the Rust NJOY port and Outram MC
the users' environments are a command line on Windows, macOS and Linux,
Android, and a web browser.

The same four ICSBEP critical experiments are used in all of them:

| case | what it is | demo |
|---|---|---|
| HEU-MET-FAST-001, "Godiva" | a bare sphere of highly enriched uranium metal | [watch](../../demos/monte-carlo/?rung=godiva&mode=watch) · [run k_eff](../../demos/monte-carlo/?rung=godiva&mode=run) |
| IEU-MET-FAST-002, "Jemima" | a cylinder of 16 % enriched uranium inside natural-uranium reflectors | [watch](../../demos/monte-carlo/?rung=jemima&mode=watch) · [run k_eff](../../demos/monte-carlo/?rung=jemima&mode=run) |
| HEU-SOL-THERM-009 case 1 | a sphere of uranium oxyfluoride solution in an aluminium tank, in water | [watch](../../demos/monte-carlo/?rung=hst009&mode=watch) · [run k_eff](../../demos/monte-carlo/?rung=hst009&mode=run) |
| LEU-COMP-THERM-008 case 1 | 4961 rods of low-enriched UO₂ in borated water | [watch](../../demos/monte-carlo/?rung=lct008&mode=watch) · [run k_eff](../../demos/monte-carlo/?rung=lct008&mode=run) |

Two are fast metal systems and two are water-moderated, so between them they
use every part of the data path: resolved and unresolved resonances,
resonance elastic scattering, and the thermal scattering law of hydrogen bound
in water.

## What each demo does

Each of the four rungs does the same five things.

1. **It draws the geometry**, from the same model the recorded results were
   measured on ([`vv::godiva`, `vv::jemima`, `vv::hst009`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/vv.rs),
   and for the lattice the benchmark's own input cards, read at run time).
2. **It shows recorded neutron tracks at once.** These are real histories,
   recorded natively and shipped with the page. The picture says they are
   recorded.
3. **It processes the nuclear data in your browser**: the ENDF/B-VIII.0 tapes
   are downloaded and reduced by the Rust NJOY port in a background worker.
   Nothing is pre-processed on a server. When that is done the tracks become
   live ones, transported in your tab.
4. **It keeps the processed data in this browser**, so a second visit starts
   in seconds. The side panel says what was processed now and what came from
   the cache.
5. **Run k_eff takes your settings**: neutrons per generation, inactive
   generations, active generations and the seed, as an OpenMC settings file
   would.
6. **Run k_eff shows the run as it goes** (since 2026-10-14): the case's
   geometry with neutrons walking through it, beside the plot of k per
   generation, the Shannon entropy of the fission source, and the console.
   Each drawn neutron is a real history born at a fission site of the
   generation the run has just finished. It is run in addition to the run's
   own neutrons and is not counted in k. The entropy is on a 5 × 5 × 5 mesh,
   so its ceiling is log₂ 125 = 6.97 bits. Watch it during the inactive
   generations: on the lattice it is still falling at generation 40, which is
   the source not yet settled, and is why the lattice's default run reads
   high.

## What to expect from Run k_eff

Every rung's default is 5000 neutrons × [40 inactive + 120 active]. For the
first three cases that is the run size of the recorded campaign, so one run of
yours is one seed of the many the record pools. For the lattice it is smaller
than the campaign's 10 000 × [250 + 400], chosen so that it runs in a browser.

| case | default run | benchmark k | recorded, outram-mc (route 4) | recorded, OpenMC + NJOY2016 (route 1) |
|---|---|---|---|---|
| Godiva | 5000 × [40 + 120] | 1.0000 ± 0.0010 | −10 ± 11 pcm (288 seeds) | +16 ± 21 pcm |
| Jemima | 5000 × [40 + 120] | 1.0000 ± 0.0030 | −391 ± 27 pcm | −406 ± 17 pcm |
| HEU-SOL-THERM-009 | 5000 × [40 + 120] | 0.9990 ± 0.0043 | +298 ± 32 pcm | +334 ± 22 pcm |
| LEU-COMP-THERM-008 | 5000 × [40 + 120] (record: 10 000 × [250 + 400]) | 1.0007 ± 0.0016 | +48 ± 19 pcm (35 nuclides, 8 seeds); ~~+166 ± 8~~ was the retired 11-nuclide model | +98 ± 23 pcm (35 nuclides); ~~+174 ± 5~~ was the 11-nuclide model |

All figures are differences from the benchmark k of the case
(**corrected 2026-10-14**: this table first measured every case from k = 1,
which gave +198 and +234 pcm for HEU-SOL-THERM-009 and +236 and +244 pcm for
the lattice). They come from the
[five-route record](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/verification_and_validation/icsbep/five_route_keff_2026_09_29.md).

**Corrected 2026-10-14:** the lattice row first gave the 11-nuclide model's
numbers. Since that day the demo and the record run all 35 nuclides of the
case-1 cards, with hydrogen in water from the Rust LEAPR port, and the 11-nuclide
model is a smoke test only.

Read the table across, not down. ~~**Three of the four cases do not sit on
their benchmark k**~~ **Two of the four cases do not sit on their benchmark
k**, on either code. HEU-SOL-THERM-009 is inside its
± 430 pcm benchmark uncertainty, Jemima is outside its ± 300 pcm, and the
lattice ~~is just outside~~ is inside its ± 160 pcm on the full nuclide list
(the 11-nuclide model was just outside). The two codes agree with each other to within their
statistics, so those offsets come from the nuclear data or from how the
benchmark model simplifies the experiment, not from the transport. This is
why the acceptance check compares with the other code and not with the
benchmark.

One run of yours will scatter around the recorded value by about 150 to
200 pcm on the first three cases. That is the spread of one run, not an error.

The lattice at the default size is not a converged calculation. Its 1.5 m core
needs about 250 inactive generations for the fission source to settle; with
40, k comes out about 80 pcm above the campaign's (16 seeds gave +70 ± 28 pcm
against OpenMC + NJOY2016, where the campaign size gives −8 ± 9; measured on
the 11-nuclide model and not re-measured on the full one). One run
scatters by about 190 pcm, so you will not see that offset in a single run.
For a k comparable with the record, set the campaign's numbers in the panel
and wait.

## How the acceptance check decides

On every operating system the check runs the four cases at reduced statistics
and compares the mean k with the recorded OpenMC + NJOY2016 value. Since
2026-10-13 every case, the lattice included, runs at 5000 × [40 + 120], with
16 seeds on the lattice: the check asks whether the code runs and gives a sane
k on that platform, not whether it is validated. The lattice's 40 inactive
generations are fewer than the 250 this core's source wants, so its k carries
a source-convergence bias, measured at about +80 pcm (above); the check would
catch a bias over 200 pcm.

- **pass** if they differ by at most 200 pcm;
- **unresolved**, which also fails, if three standard errors do not fit
  inside 200 pcm. A check that cannot fail checks nothing.

The tighter 50 pcm criterion belongs to qualification testing, which is the
proper V&V: the full-statistics runs of the record, which take hours.

The results for each environment, with the run they came from, are in the
[acceptance record](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/verification_and_validation/acceptance_testing/README.md).

## What this page does not claim

- **Not validation of a reactor.** Four critical experiments at room
  temperature say nothing about a power reactor.
- ~~**No handbook uncertainty.** The demo's "experiment" band for three of
  the cases is this workspace's stand-in.~~ **Corrected 2026-10-14: the
  benchmark values are cited second-hand.** The ICSBEP handbook is
  licence-restricted and is not in this repository. Each benchmark k and
  uncertainty is the handbook value as tabulated in an open source:
  Mosteller, *An Expanded Criticality Validation Suite for MCNP*,
  LA-UR-10-06230 (2010), and van der Marck, *Benchmarking ENDF/B-VII.0*,
  Nuclear Data Sheets 107 (2006) 3061. They report the handbook of their own
  year; a later revision could differ.
- **A browser run is one seed.** It can show a gross error. It cannot
  reproduce the record's ± 8 pcm.
