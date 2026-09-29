# Four ICSBEP cases, five code × data routes — k_eff

**Class:** verification (code-to-code and data-route-to-data-route) plus
comparison with three critical experiments. **No human V&V sign-off.** This is
an AI-assisted draft under `RESPONSIBLE_USE.md`, not for any operational,
licensing or safety use.

**Status (2026-09-29, INTERIM).**

| route(s) | state |
|---|---|
| 1 and 2 | final for all four cases |
| 3, 4 and 5 | await the ACE agent's delayed-neutron-spectra commit (default on, both outram routes, moves route 4 by +87 / −96 pcm on Godiva / Jemima); this record's route 3/4/5 numbers predate it |
| LCT-008 route 5 | not yet run |
| LCT-008 route 3 | 15 of 32 seeds (the run was stopped when that commit was announced) |

Launcher: `five_route_keff/run_all.sh`. Data: `five_route_keff/data/`. Figure:
[`five_route_keff/figures/five_route_keff.png`](five_route_keff/figures/five_route_keff.png).

> **2026-09-29: the homogenised-sphere LCT-008 case (`lct008s`) and all its
> results were deleted at the maintainer's direction because it was the wrong
> model; the case-1 lattice below replaces it.**

## Methodology

### Cases

| key | benchmark | model | source |
|---|---|---|---|
| `godiva` | HEU-MET-FAST-001 | bare sphere, r = 8.7407 cm, U-234/235/238 | `examples/godiva_keff_endf_local.rs` |
| `jemima` | IEU-MET-FAST-002 | 4-cell cylinder, core plus natural-U reflector | `examples/jemima_keff.rs` |
| `hst009` | HEU-SOL-THERM-009 case 1 | 3 spheres: solution, Al tank, water. U-236 and Cu/Zn dropped, O-17 folded into O-16 | `examples/hst009_keff.rs` |
| `lct008` | LEU-COMP-THERM-008 case 1 | **the real lattice**: 22 assembly lattices in a 7 × 7 core, parsed at run time from the committed `mit-crpg/benchmarks` OpenMC cards (`leu-comp-therm-008/`) | `examples/common/lct008_model.rs`, shared with `lct008_keff.rs` and `lct008_ace_roundtrip.rs` |

**LCT-008 is "simplified" only in its nuclide list.** It uses `lct008_keff.rs`'s
`--cheap-nuclides` tier of 11 nuclides: H1, B10, O16, U234, U235, U238, Al27,
Si28, Si29, Si30, Mn55. The 24 other nuclides the model names are **dropped,
not renormalised** (identically on both codes):

| material | dropped | amount |
|---|---|---|
| water | B-11 | 6.75e-5 /b·cm, 0.07 % of its atoms |
| Al-6061 clad | Mg-24/25/26, Ti-46–50, Cr-50/52/53/54, Fe-54/56/57/58, Cu-63/65, Zn-64/66/67/68/70 | 1.03e-3 /b·cm, 1.86 % of its atoms |

The fuel, including U-234 and its B-10 impurity, is complete.

- **Geometry self-check.** 200 000 points pass. Volume shares are water 68.9 %,
  fuel 22.6 %, clad 8.5 %.
- **OpenMC deck.** OpenMC reads the same XML with its own readers and drops the
  same 24 nuclides (`openmc_inputs/icsbep_openmc.py`, `lct008()`).
- **Benchmark k.** The reference is 1.0000 by construction, because an ICSBEP
  critical is reduced to k = 1. **No handbook uncertainty is cited.** The ICSBEP
  handbook is licence-restricted (`DATA_POLICY.md`), and none of the repo, the
  literature corpus or the local notes holds it. The figure's ±0.006 band is the
  examples' stand-in and is labelled as such; Godiva's ±0.0010 is the one
  quoted ICSBEP value.

### Routes

| route | transport | nuclear data |
|---|---|---|
| 1 | OpenMC 0.16.1-dev25 (`d7d3284a1`) | NJOY2016 2016.79 (`ac5adf5`) ACE → HDF5 |
| 2 | OpenMC | this workspace's Rust NJOY port → ACE → HDF5 |
| 3 | outram-mc | NJOY2016 ACE |
| 4 | outram-mc | Rust NJOY, ENDF read directly (default tier) |
| 5 | outram-mc | Rust-NJOY ACE |

Data and decks:

- **Evaluations.** ENDF/B-VIII.0 tapes in `reference-data/endf/`, processed at
  293.6 K with a RECONR tolerance of 0.001.
- **NJOY2016 decks.** `openmc_godiva_cross_code/make_ace.sh` (RECONR → BROADR →
  PURR 20/64 → ACER) and `make_ace_0k.sh` for the 0 K DBRC companions. The
  uranium tables come from the `reference-data/ace` submodule, which was built
  with the same deck and the same NJOY build.
- **NJOY2016 H in H₂O.** OpenMC's `make_ace_thermal` with `iwt = 1` (IFENG = 0;
  outram-mc refuses IFENG = 2) and THERMR `tol = 0.001`.
- **Rust NJOY.** `njoy-outram-park-fork/examples/write_ace_library.rs`:
  `build_full_with_purr(20, 64, 10 000)`, the 0 K companions, and
  `thermal_from_mf7_with_tolerance` at `tol = 0.001` on NJOY2016's incident grid
  (118 energies, 64 bins × 16 cosines), natom = the tape's B(6) = 2.
- **HDF5 conversion.** `openmc_inputs/ace_to_hdf5_route.py` converts both
  libraries, attaching 0 K elastic from each library's own 0 K table.

### Physics per route, and the asymmetries

| term | OpenMC (1, 2) | outram-mc ENDF (4) | outram-mc ACE (3, 5) |
|---|---|---|---|
| URR probability tables | on, from the UNR block | on, built at load (PURR 20 bins / 16 ladders / 2000 samples) | on, from the UNR block |
| resonance elastic scattering | `dbrc`, 1e-5 eV to 1 keV, all nuclides | DBRC, E ≤ 1 keV, all nuclides | DBRC, E ≤ 1 keV, 0 K companion paired |
| S(α,β) H in H₂O (hst009, lct008) | ACE table, IFENG = 0 | this crate's ENDF kernel | ACE table, IFENG = 0 |
| k in the `k` column | combined estimator (`k_alt` holds the generation mean) | generation mean | generation mean |

**Known asymmetries:**

- **URR ladders.** The ENDF route builds its tables with 16 ladders and 2000
  samples; the ACE libraries used 64.
- **S(α,β) discretisation.** The ENDF route uses its own discretisation.
- **DBRC lower bound.** OpenMC's DBRC starts at 1e-5 eV because its API refuses
  0; outram-mc has no lower bound.
- **Estimators.** OpenMC's generation-mean k agrees with its combined k to
  within 1σ on every case.
- **Thermal sampling (#188).** ~~outram-mc's thermal scattering uses the
  deliberate #188 scheme, not OpenMC's.~~ **CORRECTED 2026-09-29: never a maintainer decision; replaced by OpenMC's scheme per the maintainer** (GitHub #407): the
  equiprobable S(a,b) form is now sampled with OpenMC's
  `IncoherentInelasticAEDiscrete` on both routes. The #188 scheme survives only
  as `--ablate legacy-thermal-sampling`. It was excluded as the cause of #367
  (see below).

### Settings and statistics

| setting | Godiva, Jemima, HST-009 | lattice LCT-008 |
|---|---|---|
| histories | 5000 × [40 + 120] | 10 000 × [250 + 400] |
| threads | 8 threads (first campaign) and 4 threads (later runs), one job at a time | 4 threads, one job at a time |

- **Thread count does not change the answer.** Both codes' multi-thread results
  are independent of the thread count.
- **Seeds.** 32 independent seeds per case × route.
- **Uncertainty.** The sem of the per-seed k: sd / √n, with no averaging of the
  internal σ.
- **Δ vs route 1.** σ = √(sem² + sem₁²).

## Preparation findings (2026-09-28)

### 1. The two ACE libraries, compared before any transport

`ESZ` below is the energy grid plus the total, absorption and elastic columns.

| nuclide(s) | ESZ | other blocks |
|---|---|---|
| U-234, U-235, U-238 (293.6 K and 0 K) | identical | tables identical in length (449 695 / 6 712 632 / 6 247 445 words at 293.6 K), consistent with the recorded word-for-word parity |
| F-19, O-16, H-1, Al-27, Si-28/29/30, Mn-55 | identical (max rel. diff 0) | Rust tables omit photon production for O-16 and H-1 and charged-particle production for all light nuclides (`ntype` 0 against NJOY's 2–5) |
| B-10 | max rel. diff **7.7e-7** | as above |

Photon and charged-particle production do not enter k_eff.

**Prediction that follows:** for Godiva and Jemima, which are uranium only,
route 2 must equal route 1 and route 5 must equal route 3 **seed for seed, to
every printed digit**. The smoke runs already show this: Godiva 0.995862 on
both OpenMC routes and 1.019885 on both outram-mc ACE routes (15 generations,
one seed each).

### 2. NJOY2016's Type-1 writer aborts on B-10

NJOY2016 2016.79 stops in `change` (`acefc.f90:13942`, "Undefined law for dlwh
block: 0") while formatting B-10's charged-particle block. It leaves a
truncated Type-1 file.

- **Workaround.** The same deck with `itype = 2` completes, because the Type-2
  branch never calls `change`. B-10's NJOY tables are therefore binary.
- **Reading them.** outram-mc's reader sniffs Type 2. For OpenMC,
  `ace_to_hdf5_route.py` reads NJOY's Fortran record layout itself, because
  OpenMC's binary reader expects MCNP's fixed-length records.
- **Detection.** `make_njoy_library.sh` detects the abort from NJOY's stdout
  rather than special-casing B-10.

### 3. The Rust H(H₂O) table: four defects, all fixed (GitHub #368)

| # | defect | effect |
|---|---|---|
| 1 | tape base-temperature S(α,β) evaluated at 293.6 K | σ_inel 1.68× NJOY's |
| 2 | natom 1 instead of the tape's B(6) = 2 | σ_inel 2.0× NJOY's |
| 3 | THERMR `tol` hard-coded at 0.05, while NJOY2016's reference used 0.001 | emission E′ up to 8.2e-2, cosines 0.144 |
| 4 | ITIX written as an analytic integral instead of THERMR's `calcem` `xsi` via `terp` (`thermr.f90:2166-2173`, `:2459`; `aceth.f90:131`) | σ_inel up to 6.6e-2 at 1e-5 eV |

Defects 1 and 2 were in this study's library writer and were caught before
any run. Defects 3 and 4 were in the port: 3 is fixed by the new
`thermal_from_mf7_with_tolerance`, and 4 by a port of THERMR's `terp`.

**After all four fixes**, against NJOY2016's table:

| quantity | worst |
|---|---|
| σ_inel | 1.55e-4 (median 3.0e-5) |
| emission E′ | 1.9e-4 |
| emission cosines | 0.010 |

Regression: `njoy-outram-park-fork/tests/thermal_ace_tolerance.rs`. All nine
njoy thermal test binaries pass, as does outram-mc's `thermal_from_ace`.

### 4. HST-009 on the ACE route was blocked by F-19

HST-009 could not run on routes 3 and 5 until the F-19 refusal (#365) was
fixed; it runs since then.

## Predictions (written before each measurement)

| # | prediction | verdict |
|---|---|---|
| P1 | route 2 ≡ route 1 and route 5 ≡ route 3, seed for seed, on Godiva and Jemima | **held**: 32/32 identical in every printed digit, both pairings, both campaigns |
| P2 | after the thermal fixes, route 2 − route 1 within 2σ of 0 on HST-009 and LCT-008 | **held**: +41 ± 38 and −3 ± 11 (was −96 ± 33 and −94 ± 11) |
| P3 | #367: if outram-mc's S(α,β) sampling (#188) causes HST-009's gap, a free-gas A/B closes it | **refuted**: −247 ± 37 |
| P4 | #367: if F-19 causes it, dropping F-19 closes it | **refuted**: −272 ± 38 |
| P5 | route 3 with the #366 fix lands within ~60 pcm of route 1 on Godiva and Jemima | met on the patch test (Godiva −5 ± 37, Jemima −51 ± 33); the post-fix campaign gives −54 ± 40 and +124 ± 38, the latter since attributed to the delayed spectra (see below) |

## Results — interim (2026-09-29)

**Route 1 and 2 rows are final. The route 3/4/5 rows predate the delayed-neutron-spectra commit.**

| case | route | n | k_eff ± sem | seed sd [pcm] | Δ vs k=1 [pcm] | Δ vs route 1 [pcm] |
|---|---|---|---|---|---|---|
| godiva | route1 | 32 | 1.00016 ± 0.00021 | 118 | +16 ± 21 | reference |
| godiva | route2 | 32 | 1.00016 ± 0.00021 | 118 | +16 ± 21 | +0 ± 29 (+0.0σ) |
| godiva | route3 | 32 | 0.99962 ± 0.00035 | 195 | -38 ± 35 | -54 ± 40 (-1.3σ) |
| godiva | route4 | 32 | 0.99933 ± 0.00035 | 197 | -67 ± 35 | -83 ± 41 (-2.0σ) |
| godiva | route5 | 32 | 0.99962 ± 0.00035 | 195 | -38 ± 35 | -54 ± 40 (-1.3σ) |
| jemima | route1 | 32 | 0.99594 ± 0.00017 | 99 | -406 ± 17 | reference |
| jemima | route2 | 32 | 0.99594 ± 0.00017 | 99 | -406 ± 17 | +0 ± 25 (+0.0σ) |
| jemima | route3 | 32 | 0.99718 ± 0.00034 | 191 | -282 ± 34 | +124 ± 38 (+3.3σ) |
| jemima | route4 | 32 | 0.99699 ± 0.00029 | 163 | -301 ± 29 | +106 ± 34 (+3.1σ) |
| jemima | route5 | 32 | 0.99718 ± 0.00034 | 191 | -282 ± 34 | +124 ± 38 (+3.3σ) |
| hst009 | route1 | 32 | 1.00234 ± 0.00022 | 127 | +234 ± 22 | reference |
| hst009 | route2 | 32 | 1.00275 ± 0.00031 | 174 | +275 ± 31 | +41 ± 38 (+1.1σ) |
| hst009 | route3 | 32 | 1.00002 ± 0.00033 | 189 | +2 ± 33 | -232 ± 40 (-5.8σ) |
| hst009 | route4 | 32 | 0.99962 ± 0.00027 | 152 | -38 ± 27 | -271 ± 35 (-7.8σ) |
| hst009 | route5 | 32 | 0.99931 ± 0.00044 | 248 | -69 ± 44 | -302 ± 49 (-6.1σ) |
| lct008 | route1 | 32 | 1.00251 ± 0.00007 | 42 | +251 ± 7 | reference |
| lct008 | route2 | 32 | 1.00248 ± 0.00008 | 45 | +248 ± 8 | -3 ± 11 (-0.3σ) |
| lct008 | route3 | 15 | 1.00212 ± 0.00015 | 60 | +212 ± 15 | -39 ± 17 (-2.3σ) |
| lct008 | route4 | 32 | 1.00192 ± 0.00012 | 67 | +192 ± 12 | -59 ± 14 (-4.2σ) |

Binaries:

- routes 3 and 5: `25270acb67` (post-#365 and #366, pre-URR interpolation, pre-delayed spectra);
- lattice route 4: `3d9526e5a1`;
- Godiva, Jemima and HST-009 route 4: `be4206c2d3` (the first campaign).

Earlier states of routes 2, 3 and 5 are archived rather than overwritten:

- `data/per_seed_keff_pre366_routes35.csv`: routes 3 and 5 before #366's fix,
  i.e. Godiva +1853 and Jemima −923 pcm against route 1;
- `data/per_seed_keff_pre_thermalfix_route2.csv`: route 2 with the pre-fix
  thermal table.

### What the numbers say

- **Data routes (route 2 against route 1).** The Rust NJOY port's library is at
  k-parity with NJOY2016's on all four cases:
  - identical seed for seed on the two uranium-only cases;
  - +41 ± 38 on HST-009;
  - −3 ± 11 on the lattice.
- **The outram-mc ACE route (#366).** Since the fix, outram-mc on ACE and
  outram-mc on ENDF agree to within 2σ on every case. The remaining gaps to
  OpenMC are shared by both outram routes, so they are transport-side, not
  data-side.
- **Jemima.** The shared +106 to +124 pcm is, per the ACE agent's measurement,
  carried by the delayed-neutron spectra. Its pending commit gives route 4
  −1 ± 39 against route 1. To be re-measured here once it lands.
- **HST-009 (#367).** The shared −232 to −302 pcm is open. Excluded so far,
  each by an A/B with the prediction recorded first (data in
  `data/diagnostics.csv`):
  - the S(α,β) data: route 3 reads OpenMC's own table;
  - the #188 thermal-sampling scheme: free-gas A/B, −247 ± 37;
  - F-19: F-19-free A/B, −272 ± 38.
- **Lattice LCT-008.** Routes 1, 2 and 4 all sit at +190 to +250 pcm above
  k = 1, inside the stand-in band. Route 4 − route 1 is −59 ± 14. That is small,
  but resolved at 4σ, and it is to be re-measured with the delayed spectra.

### The ACE-route defect, as found by the first campaign (GitHub #366, fixed at `a15958912c`)

- **Symptom.** outram-mc on either ACE library was +1853 ± 37 pcm (Godiva) and
  −923 ± 40 pcm (Jemima) against OpenMC on the identical tables.
- **Cause, part 1.** `Nuclide::from_ace` took its transport channels from
  `ce_decode::channel_mts`, a total-reconstruction rule. That kept the MT=4
  lump with Q = 0 in place of the 40 discrete levels.
- **Cause, part 2.** U-234's partial-fission-only table gave zero fission.
- **Confirmation.** A temporary patch plus U-234 from ENDF brought Godiva to
  −5 ± 37 pcm. Localised with `examples/ace_vs_endf_nuclide_probe.rs`,
  per-nuclide swaps, and ablations (`data/diagnostics.csv`).

## What this cannot do

- **Benchmark bands.** The handbook uncertainties are not available here;
  Jemima, HST-009 and LCT-008 bands are stand-ins.
- **Data library.** Only ENDF/B-VIII.0 at 293.6 K is covered.
- **LCT-008 nuclide list.** The lattice runs an 11-nuclide tier, not the full
  36-nuclide model `lct008_keff.rs` runs by default.

## Appendix — OpenMC provenance and driver script (verbatim)

| item | value |
|---|---|
| OpenMC | 0.16.1-dev25, commit `d7d3284a1b13d7cae020e0d8fea9f1e60d91b18b` |
| cross sections | built in this study (see *Routes*) |
| run settings | as in *Settings and statistics* |

No Shannon-entropy check was run. The lattice's 250 inactive generations are
`lct008_keff.rs`'s own setting.

```python
"""OpenMC decks for the four ICSBEP cases of the five-route study (routes 1, 2).

The SAME models `examples/icsbep_five_route_keff.rs` runs on outram-mc: Godiva,
Jemima and HST-009 with the atom densities and dimensions copied from that file
(which cites godiva_keff_endf_local.rs, jemima_keff.rs, hst009_keff.rs), and
LCT-008 case 1 read from the committed benchmark XML (see `lct008()` below). Only the cross-section library differs between
route 1 (NJOY2016 ACE -> HDF5) and route 2 (Rust-NJOY ACE -> HDF5), selected
with --xs <cross_sections.xml>.

  python icsbep_openmc.py --case godiva --xs <h5>/cross_sections.xml --seed 1 \
      --label route1 --csv out.csv --workdir runs/godiva_route1_s1 [--threads 8] \
      [--particles 5000 --inactive 40 --active 120]

Physics settings, each chosen to match what outram-mc carries by default:

* temperature 293.6 K on every material (the libraries hold one temperature,
  labelled 294K by OpenMC from kT = 2.53e-8 MeV);
* URR probability tables ON (``ptables``, OpenMC's default);
* resonance elastic scattering ON, method ``dbrc``, 1e-5 eV <= E <= 1 keV,
  every nuclide carrying 0 K elastic (all of them) -- outram-mc applies DBRC
  to every nuclide below 1 keV with no lower limit (``DbrcTable::applies``).
  OpenMC's Python API refuses energy_min <= 0, so it is set to 1e-5 eV, the
  bottom of every ENDF evaluation's grid, rather than OpenMC's 0.01 eV default;
* S(a,b) ``c_H_in_H2O`` on the water-bearing materials (HST-009 solution and
  reflector, LCT-008s mixture), none elsewhere;
* source: Watt fission spectrum (OpenMC's default, the same a, b as outram-mc's
  ``KeffSettings`` default) uniform over the fuel region, fissionable-only.

One CSV row is appended per run. ``k`` is OpenMC's combined estimator
(``StatePoint.keff``); ``k_alt`` is the mean of the per-batch generation
k over active batches, the estimator outram-mc reports. Both are recorded so the
choice of estimator can be checked rather than argued.
"""
import argparse
import csv
import os
import pathlib
import time

import numpy as np
import openmc

T = 293.6


def mat(name, comps, sab=False):
    m = openmc.Material(name=name, temperature=T)
    for n, d in comps:
        m.add_nuclide(n, d, "ao")
    m.set_density("sum")
    if sab:
        m.add_s_alpha_beta("c_H_in_H2O")
    return m


def godiva():
    m = mat("Godiva HEU", [("U234", 4.9184e-4), ("U235", 4.4994e-2), ("U238", 2.4984e-3)])
    s = openmc.Sphere(r=8.7407, boundary_type="vacuum")
    geo = openmc.Geometry([openmc.Cell(fill=m, region=-s)])
    src = openmc.stats.spherical_uniform(r_outer=8.7407)
    return [m], geo, src


def jemima():
    core = mat("oralloy core", [("U234", 8.4430e-05), ("U235", 7.7777e-03), ("U238", 3.9671e-02)])
    refl = mat("natural uranium reflector",
               [("U234", 2.6433e-06), ("U235", 3.4603e-04), ("U238", 4.7711e-02)])
    z0 = openmc.ZPlane(0.0, boundary_type="vacuum")
    z1 = openmc.ZPlane(7.62)
    z2 = openmc.ZPlane(39.571)
    z3 = openmc.ZPlane(47.0894, boundary_type="vacuum")
    c1 = openmc.ZCylinder(r=19.05)
    c2 = openmc.ZCylinder(r=26.6446, boundary_type="vacuum")
    cells = [
        openmc.Cell(fill=refl, region=+z0 & -z1 & -c2),
        openmc.Cell(fill=core, region=+z1 & -z2 & -c1),
        openmc.Cell(fill=refl, region=+z1 & -z2 & +c1 & -c2),
        openmc.Cell(fill=refl, region=+z2 & -z3 & -c2),
    ]
    geo = openmc.Geometry(cells)
    src = openmc.stats.Box((-19.05, -19.05, 7.62), (19.05, 19.05, 39.571))
    return [core, refl], geo, src


def hst009():
    sol = mat("uranium oxyfluoride solution", [
        ("U234", 1.7561e-05), ("U235", 1.6626e-03), ("U238", 9.4079e-05),
        ("F19", 3.5663e-03), ("O16", 3.334735656e-02 + 1.264344e-05), ("H1", 5.9587e-02)], sab=True)
    tank = mat("1100 aluminium tank", [
        ("Al27", 5.9699e-02), ("Si28", 5.09126279536e-04), ("Si29", 2.5851979832e-05),
        ("Si30", 1.7041740632e-05), ("Mn55", 1.4853e-05)])
    water = mat("water reflector", [("H1", 6.6659e-02), ("O16", 3.3316368309e-02 + 1.2631691e-05)],
                sab=True)
    s1 = openmc.Sphere(r=11.5177)
    s2 = openmc.Sphere(r=11.6764)
    s3 = openmc.Sphere(r=35.0, boundary_type="vacuum")
    geo = openmc.Geometry([openmc.Cell(fill=sol, region=-s1),
                           openmc.Cell(fill=tank, region=+s1 & -s2),
                           openmc.Cell(fill=water, region=+s2 & -s3)])
    r = 11.5177
    src = openmc.stats.Box((-r, -r, -r), (r, r, r))
    return [sol, tank, water], geo, src


# LEU-COMP-THERM-008 case 1, the REAL lattice: the same committed
# mit-crpg/benchmarks cards outram-mc parses (examples/common/lct008_model.rs),
# read here with OpenMC's own XML readers, so geometry and densities are the
# file's, not a transcription. Restricted to the SAME 11-nuclide tier outram-mc
# runs (lct008_keff.rs `TAPES_CHEAP`): every other nuclide is dropped and NOT
# renormalised, matching `build_materials` (density units="sum" then sums what
# is left, exactly as outram-mc's per-nuclide atom densities do).
LCT008_DIR = pathlib.Path(__file__).resolve().parents[2] / "leu-comp-therm-008"
LCT008_TIER = {"H1", "B10", "O16", "U234", "U235", "U238", "Al27",
               "Si28", "Si29", "Si30", "Mn55"}


def lct008():
    mats = openmc.Materials.from_xml(str(LCT008_DIR / "materials.xml"))
    dropped = set()
    for m in mats:
        m.temperature = T
        for nuc in [n.name for n in m.nuclides]:
            if nuc not in LCT008_TIER:
                m.remove_nuclide(nuc)
                dropped.add(nuc)
    print("lct008: dropped (not in the outram-mc tier):", ", ".join(sorted(dropped)))
    geo = openmc.Geometry.from_xml(str(LCT008_DIR / "geometry.xml"), materials=mats)
    used = list(geo.get_all_materials().values())
    # Same source region as outram-mc: the core cylinder's bounding box.
    src = openmc.stats.Box((-76.2, -76.2, -81.662), (76.2, 76.2, 81.662))
    return used, geo, src


CASES = {"godiva": godiva, "jemima": jemima, "hst009": hst009, "lct008": lct008}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--case", required=True, choices=CASES)
    ap.add_argument("--xs", required=True)
    ap.add_argument("--seed", type=int, required=True)
    ap.add_argument("--label", required=True)
    ap.add_argument("--csv", required=True)
    ap.add_argument("--workdir", required=True)
    ap.add_argument("--threads", type=int, default=8)
    ap.add_argument("--particles", type=int, default=5000)
    ap.add_argument("--inactive", type=int, default=40)
    ap.add_argument("--active", type=int, default=120)
    ap.add_argument("--commit", default="unknown")
    ap.add_argument("--drop-nuclide", default=None,
                    help="diagnostic (GitHub #367): remove one nuclide everywhere")
    ap.add_argument("--no-sab", action="store_true",
                    help="diagnostic (GitHub #367): drop S(a,b), H-1 free gas")
    ap.add_argument("--openmc", default=os.path.expanduser("~/Documents/research/openmcbin/bin/openmc"))
    a = ap.parse_args()

    mats, geo, space = CASES[a.case]()
    if a.no_sab:
        for m in mats:
            m._sab = []
    if a.drop_nuclide:
        for m in mats:
            if a.drop_nuclide in [n.name for n in m.nuclides]:
                m.remove_nuclide(a.drop_nuclide)
    materials = openmc.Materials(mats)
    materials.cross_sections = str(pathlib.Path(a.xs).resolve())
    s = openmc.Settings()
    s.run_mode = "eigenvalue"
    s.particles = a.particles
    s.inactive = a.inactive
    s.batches = a.inactive + a.active
    s.seed = a.seed
    s.source = openmc.IndependentSource(
        space=space, energy=openmc.stats.Watt(a=0.988e6, b=2.249e-6),
        constraints={"fissionable": True})
    s.temperature = {"default": T, "method": "nearest", "tolerance": 10.0}
    s.ptables = True
    s.resonance_scattering = {"enable": True, "method": "dbrc",
                              "energy_min": 1.0e-5, "energy_max": 1000.0}
    s.output = {"tallies": False, "summary": False}
    model = openmc.Model(geometry=geo, materials=materials, settings=s)

    wd = pathlib.Path(a.workdir)
    wd.mkdir(parents=True, exist_ok=True)
    t0 = time.time()
    sp_path = model.run(cwd=wd, threads=a.threads, openmc_exec=a.openmc, output=False)
    wall = time.time() - t0
    with openmc.StatePoint(sp_path) as sp:
        k = sp.keff
        kgen = np.asarray(sp.k_generation)[a.inactive:]
    # Keep the statepoint small on disk: the CSV row is the record.
    csv_path = pathlib.Path(a.csv)
    new = not csv_path.is_file()
    with csv_path.open("a", newline="") as f:
        w = csv.writer(f)
        if new:
            w.writerow(["case", "route", "code", "seed", "k", "k_std_internal", "particles",
                        "inactive", "active", "threads", "wall_s", "load_s", "n_nuclides",
                        "n_urr", "n_dbrc", "sab", "commit", "k_alt"])
        n_nuc = len({n.name for m in mats for n in m.nuclides})
        w.writerow([a.case, a.label, "openmc", a.seed, f"{k.nominal_value:.6f}",
                    f"{k.std_dev:.6f}", a.particles, a.inactive, a.active, a.threads,
                    f"{wall:.1f}", "", n_nuc, "", "", str(CASES[a.case] in (hst009, lct008)).lower(),
                    a.commit, f"{kgen.mean():.6f}"])
    print(f"{a.case} {a.label} seed {a.seed}: k = {k.nominal_value:.5f} +/- {k.std_dev:.5f}"
          f"  (generation mean {kgen.mean():.5f})  {wall:.1f} s")


if __name__ == "__main__":
    main()
```
