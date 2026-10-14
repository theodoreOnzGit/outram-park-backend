# Where does σ(E) come from?

> **Research, education and V&V only.** Not for reactor operation, licensing,
> safety decisions or emergency response.

> **Review status: first draft, 2026-10-04, AI-assisted, not yet reviewed by a
> human.**

*Rung 0 of the nuclear data track. You probably arrived from a Monte Carlo
lesson (for example [rung 1, Godiva](../../tutorials/monte-carlo/godiva.html)),
at the line where a neutron's flight length is sampled as
`d = −ln ξ / Σ_t(E)`. This page answers the question that line raises, and the
rest of the book answers it in depth.*

## The question

A Monte Carlo code needs, at every collision, the **macroscopic total cross
section** of the material the neutron is in:

```text
Σ_t(E) = Σ_i  N_i · σ_t,i(E, T)        [1/cm]

  N_i       atoms of nuclide i per cm³ (from the material's density and composition)
  σ_t,i     microscopic total cross section of nuclide i [barn = 1e-24 cm²]
  E         the neutron's energy;  T  the material temperature
```

The number density `N_i` is easy: it is the material. The hard part is
`σ_t,i(E, T)`. For uranium-238 it swings by more than four orders of magnitude
across a few electron-volts, through hundreds of resonances, and it changes
shape with temperature. **Where does that curve come from?**

## The shortest answer

Nobody measures σ(E) at every energy a code will ask for. **Evaluators**
combine measurements with nuclear models and publish an **evaluation**: a file
in the **ENDF-6** format holding a compact description of the nucleus
(resonance parameters, tabulated smooth cross sections, angular and energy
distributions, thermal scattering laws, covariances). A **processing code**
then turns that description into the table a transport code reads. NJOY is
that code [(MacFarlane et al., 2017)](#ref-njoy2016), and this crate is its Rust port.

NJOY's manual puts it in one sentence:

> "These libraries represent the underlying nuclear data from a physics
> viewpoint, but practical calculations usually require special libraries for
> particle transport codes or reactor core physics codes. This is the mission
> of NJOY — to take the basic data from the nuclear data library and convert it
> into the forms needed for applications."
>
> — MacFarlane et al., *The NJOY Nuclear Data Processing System, Version 2016*,
> LA-UR-17-20093, §1, p. 1 (PDF p. 15).

## The pipeline, one rung per step

```text
  ENDF-6 evaluation (reference-data/endf/*.endf)
        │   rung 1  ENDF: what the evaluator hands us         endf, moder
        ▼
  RECONR   resonance parameters ──► pointwise σ(E) at 0 K     rung 2  reconr, samm
        ▼
  BROADR   Doppler-broaden to the material temperature T      rung 3  broadr
        ▼
  UNRESR / PURR  the unresolved range: probability tables     rung 4  unresr, purr
        ▼
  THERMR (+ LEAPR)  thermal scattering, S(α,β)                rung 5  thermr, leapr
        ▼
  HEATR / GASPR  heating (KERMA), damage, gas production      rung 6  heatr, gaspr
        │
        ├──► GROUPR / GAMINR  multigroup constants            rung 7  groupr, gaminr
        │         └──► ERRORR / COVR  covariances             rung 8  errorr, covr
        │         └──► WIMSR, DTFR, … other codes' formats    Part II
        ▼
  ACER     the ACE table a continuous-energy code reads       rung 9  acer
```

## Two ways the Monte Carlo track gets its σ(E)

This workspace feeds `outram-mc-libs` by **two routes**, and the Monte Carlo
lessons use both:

1. **ENDF read directly** (the default, "route 4" in the five-route
   comparison). The transport code calls RECONR and BROADR **in process** when
   it loads a nuclide, keeps the result in memory, and never writes a file.
   The [TRISO pebble demo](../../demos/monte-carlo/?rung=triso&mode=watch) does exactly this in your
   browser.
2. **Through an ACE file** ("route 5"). ACER writes the same data out as an ACE
   table, and the transport code reads it back the way OpenMC would read
   NJOY2016's own.

Both routes start from the same evaluation and the same two modules.

## In the code: from a Monte Carlo nuclide down to σ(E)

The first route begins in `outram-mc-libs`, in the constructor that builds a
nuclide from an ENDF tape, and immediately crosses into this crate.

> **On the [Code Review map](../../code-review/):** start at [`Nuclide::from_endf_file_with_speed`](../../code-review/#crates/outram-mc-libs/src/material/nuclide.rs::Nuclide::from_endf_file_with_speed) and follow its callees (▸) to [`broadr_joint`](../../code-review/#crates/njoy-outram-park-fork/src/broadr/joint.rs::broadr_joint). Each card opens the function's source and shows its review stamp.

> **The physics functions:** this page has none of its own; its physics is in rungs 2 and 3. See [Where the physics is](./physics-functions.md#where-sigma-comes-from) (a proposal for maintainer review, 2026-10-14; the entry-point link above stays until it is accepted).

**Reading the path.** The table below is the path the Code Review map link above follows, with what each hop does:

| hop | function | file | resolved (kopitiam pass, 2026-10-04) |
|---|---|---|---|
| 0 | `Nuclide::from_endf_file_with_speed` | [`outram-mc-libs/src/material/nuclide.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#@@L:crates/outram-mc-libs/src/material/nuclide.rs:fn=from_endf_file_with_speed@@) | entry point |
| 1 | `Tape::read_file` (rung 1) | [`endf/tape.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/endf/tape.rs#@@L:crates/njoy-outram-park-fork/src/endf/tape.rs:fn=read_file@@) | filled by hand |
| 1 | `Nuclide::from_tape_with_speed` → `from_tape_with_broadr_tolerance` | [`nuclide.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#@@L:crates/outram-mc-libs/src/material/nuclide.rs:fn=from_tape_with_speed@@), [`from_tape_with_broadr_tolerance`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#@@L:crates/outram-mc-libs/src/material/nuclide.rs:fn=from_tape_with_broadr_tolerance@@) | filled by hand |
| 2 | `reconr::reconr` at 0 K (rung 2) | [`reconr/mod.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/reconr/mod.rs#@@L:crates/njoy-outram-park-fork/src/reconr/mod.rs:fn=reconr@@) | filled by hand |
| 2 | `broadr::broaden_result_with_tolerance` to `T` (rung 3) | [`broadr/mod.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/broadr/mod.rs#@@L:crates/njoy-outram-park-fork/src/broadr/mod.rs:fn=broaden_result_with_tolerance@@) | filled by hand |
| 2 | `NuBar::from_endf`, `FissionSpectrum::from_endf_mf5` (ν̄ and χ) | [`nuclear_data/secondary.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/nuclear_data/secondary.rs) | filled by hand |
| 2 | `acer::angular::parse_mf4_angular` (elastic angles) | [`acer/angular.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/acer/angular.rs) | filled by hand |


The heart of it is three calls in
[`from_tape_with_broadr_tolerance`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/material/nuclide.rs#@@L:crates/outram-mc-libs/src/material/nuclide.rs:fn=from_tape_with_broadr_tolerance@@):
RECONR at zero kelvin (`reconr(tape, &ReconrConfig { mat, tolerance,
temperature: 0.0 })`), keep the 0 K elastic curve up to `DBRC_GRID_MAX_EV` (25 keV)
for resonance elastic scattering (DBRC, which models the target motion itself and so needs
the unbroadened σ), then BROADR to the material temperature
(`broaden_result_with_tolerance(&recon0, temp_k, broadr_errthn)`). That code
belongs to the Monte Carlo crate, so this page links to it rather than
including it (an include by line number into another crate would drift
silently when that crate changes).

Note the comment on step 3: BROADR is **bounded at `thnmax`**, the top of the
resolved resonance region. Rung 3 tells the story of the defect that sentence
records.

## The check

The cleanest single check that this crate supplies the right σ(E) is a
**data-route comparison**: run the *same* transport code (OpenMC) on NJOY2016's
ACE files and on this crate's ACE files, everything else identical. Recorded in
[`five_route_keff_2026_09_29.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/verification_and_validation/icsbep/five_route_keff_2026_09_29.md),
"Results — after the OpenMC-parity audit" (2026-09-30, routes 3/4/5 at
`0414bc8277`; routes 1/2 unchanged from the earlier campaign), 32 seeds each:

| case | OpenMC on NJOY2016 ACE (route 1) | OpenMC on this crate's ACE (route 2) | route 2 − route 1 |
|---|---|---|---|
| Godiva | 1.00016 ± 0.00021 | 1.00016 ± 0.00021 | +0 ± 29 pcm |
| Jemima | 0.99594 ± 0.00017 | 0.99594 ± 0.00017 | +0 ± 25 pcm |
| HST-009 | 1.00234 ± 0.00022 | 1.00275 ± 0.00031 | +41 ± 38 pcm |
| LCT-008 | 1.00244 ± 0.00005 (96 seeds) | 1.00248 ± 0.00008 | +4 ± 9 pcm |

On the two fast spheres the two libraries give **the same `k` seed for seed, in
every printed digit** (the record's prediction P1). That is possible only
because rung 9's ACE tables reproduce NJOY2016's in every word. The thermal
cases differ through the S(α,β) table, within 1.1σ.

This is **verification** (one data-processing route against another), not
validation. Whether `k` matches the experiment is the Monte Carlo track's
question.

## Predict

The tape for U-238 does not contain a table of σ(E) in the resonance region at
all. It contains a few thousand numbers per resonance: an energy and some
widths. **Before you open rung 1, guess:** how many energy points does it take
to draw U-238's cross section to 0.1 % accuracy? A hundred? Ten thousand? A
million? (Rung 2 has the recorded answer.)

**Next:** [Rung 1 — ENDF: what the evaluator hands us](./endf.md).

<!-- references:begin -->
## References

<p class="csl-entry" id="ref-njoy2016" style="padding-left: 2em; text-indent: -2em;">MacFarlane, R. E., Muir, D. W., Boicourt, R. M., Kahler, A. C., &#38; Conlin, J. L. (2017). <i>The NJOY Nuclear Data Processing System, Version 2016</i> (Technical Report LA-UR-17-20093). Los Alamos National Laboratory. https://www.osti.gov/biblio/1338791</p>

<!-- references:end -->
