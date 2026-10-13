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

## What to expect from Run k_eff

The defaults are the run sizes of the recorded campaign, so one run of yours is
one seed of the many the record pools.

| case | default run | recorded, outram-mc (route 4) | recorded, OpenMC + NJOY2016 (route 1) |
|---|---|---|---|
| Godiva | 5000 × [40 + 120] | −10 ± 11 pcm (288 seeds) | +16 ± 21 pcm |
| Jemima | 5000 × [40 + 120] | −391 ± 27 pcm | −406 ± 17 pcm |
| HEU-SOL-THERM-009 | 5000 × [40 + 120] | +198 ± 32 pcm | +234 ± 22 pcm |
| LEU-COMP-THERM-008 | 10 000 × [250 + 400] | +236 ± 8 pcm | +244 ± 5 pcm |

All figures are differences from k = 1, from the
[five-route record](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/verification_and_validation/icsbep/five_route_keff_2026_09_29.md).

Read the table across, not down. **Three of the four cases do not sit at
k = 1**, on either code. The two codes agree with each other to within their
statistics, so those offsets come from the nuclear data or from how the
benchmark model simplifies the experiment, not from the transport. This is
why the acceptance check compares with the other code and not with 1.

One run of yours will scatter around the recorded value by about 150 to
200 pcm on the first three cases. That is the spread of one run, not an error.

The lattice is slow: its default is 6.5 million histories on one thread. A
smaller run is quicker, but this 1.5 m core needs its 250 inactive generations
for the fission source to settle. With fewer, k is biased, not just noisier.

## How the acceptance check decides

On every operating system the check runs the four cases at reduced statistics
and compares the mean k with the recorded OpenMC + NJOY2016 value:

- **pass** if they differ by at most 200 pcm;
- **unresolved**, which also fails, if three standard errors do not fit
  inside 200 pcm. A check that cannot fail checks nothing.

The tighter 50 pcm criterion belongs to the full-statistics runs of the
record, which take hours.

The results for each environment, with the run they came from, are in the
[acceptance record](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/verification_and_validation/acceptance_testing/README.md).

## What this page does not claim

- **Not validation of a reactor.** Four critical experiments at room
  temperature say nothing about a power reactor.
- **No handbook uncertainty.** The ICSBEP handbook is licence-restricted and
  is not in this repository, so the demo's "experiment" band for three of the
  cases is this workspace's stand-in, and is labelled so. Godiva's ± 0.0010 is
  the one quoted value.
- **A browser run is one seed.** It can show a gross error. It cannot
  reproduce the record's ± 8 pcm.
