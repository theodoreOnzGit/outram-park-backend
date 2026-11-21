# Transport from an OpenMC HDF5 library (gh:#303, rows 2 and 3)

Started 2026-11-18. **This file is committed with its PREDICTIONS before any
comparison below is run.** The RESULTS sections are filled in afterwards, and a
prediction that misses is left in place with the outcome beside it.

Research, education and V&V only (`RESPONSIBLE_USE.md`). Open data only:
ENDF/B-VIII.0 through NJOY2016, OpenMC 0.15.3 (`27e38e894`).

## What is being tested

The library is the 11-nuclide LCT-008 OpenMC library built for gh:#550:
`reference-data/endf` --NJOY2016 (ACER, 293.6 K and 0 K, PURR)-->
`openmc.data.IncidentNeutron.from_njoy` --> `.h5`. It carries **two**
temperatures per nuclide (`0K` and `294K`), URR tables on U-234/235/238 and
Mn-55, `continuous`, `correlated` and `kalbach-mann` laws, and delayed-neutron
products on the three uranium nuclides. Law inventory read off the files
(h5py, 2026-11-18):

| nuclide | laws other than `level`/elastic |
|---|---|
| U-235, U-238 | `correlated` x4 (MT 5, 16, 17, 91); `continuous` fission (MT 18); 6 delayed `continuous` |
| U-234 | `kalbach-mann` x4 (MT 16, 17, 37, 91); `continuous` fission MT 19/20/21/38; 24 delayed `continuous` |
| Mn-55 | `correlated` x5 (MT 5, 16, 17, 37, 91) |
| Si-28/29/30 | `correlated` x4 (MT 16, 22, 28, 91), `kalbach-mann` (MT 5) |
| Al-27 | `kalbach-mann` x8 (MT 5, 16, 22, 28, 32, 33, ...) |
| O-16 | `kalbach-mann` x10 (MT 5, 16, 22, 23, 28, 32, ...) |
| H-1, B-10 | elastic and `level` only |

The reference for bit identity is `Nuclide::from_ace` on **the NJOY ACE the
library was built from** (`.../lct008_pin11/njoy/293.6K/*.ace`, 0 K companion in
`.../njoy/0K/`), because that is the same evaluation through the same ACER.

## Method

`Nuclide::from_hdf5` maps each decoded law onto the SAME internal
representation `from_ace` builds (`AceEnergyLaw` rows, then `ContinuumEmission`,
`UncorrelatedEmission`, `OtherChannel`, `FissionSpectrum`, `DelayedData`,
`UrrProbabilityTables`), so the samplers are the ones the ACE route uses and
nothing about sampling is re-derived. Layout citations: OpenMC
`openmc/data/{correlated,kalbach_mann,energy_distribution,urr,product,reaction}.py`.
Bit identity is checked field by field with a streaming hash of the `Debug`
text of each field (an `f64`'s `Debug` text is its shortest round-trip form, so
equal text is equal bits).

## PREDICTIONS (written before any run)

### Bit identity of the converted nuclide against `from_ace` (same ACE)

| item | predicted |
|---|---|
| every sampled cross section (MT != 1) | **0 differing words**: the h5 arrays are the ACE arrays |
| MT=1 total | **NOT identical**: OpenMC writes no MT=1, so the total is OpenMC's own sum of non-redundant reactions; relative difference <= 1e-8 on every nuclide (Al-27 gave 4.6e-9 in the existing test) |
| elastic angular tables, `level` angular tables | identical, or <= 1 ulp where `e_mev * 1e6 / 1e6` does not round-trip |
| `continuous` / `correlated` / `kalbach-mann` rows (e_out, pdf, cdf, r, a, cosines, incident grid) | **identical**: OpenMC scales e_out by `*1e6` and pdf by `/1e6` exactly as `read_tabulated_law` does |
| fission chi (U-234 partials, U-235/238 MT 18) | identical |
| URR tables | identical except the heating column, which OpenMC multiplies by 1e6 unconditionally; zero on these tables, so identical |
| delayed data | spectra identical; `lambda` identical or 1 ulp (`* 1e8` against `/ 1e-8`); `nu_delayed` and `group_fraction` NOT bit-identical (OpenMC stores `nu_d * p_k` and renormalises; the split is rebuilt as a ratio), relative difference <= 1e-12 at the nodes |
| total nu | identical |
| 0 K elastic (DBRC) from the h5's `0K` group | identical to the 0 K ACE route (`with_elastic_0k_from_ace`) |

Any nuclide with a residual outside these bounds is reported as a miss, not
adjusted.

### Row 2: LCT-008 k

Case-1 lattice, `lct008_ace_roundtrip` model and settings (10000 histories x
[250 inactive + 400 active]), >= 8 seeds, S(a,b) H in H2O built from the ENDF
tape and attached on every arm (the h5 thermal file is not read here, so this
tests the continuous-energy tables only).

- **h5 route against the NJOY-ACE route (same data)**: |z| <= 2; central
  expectation |difference| below 1 sigma of the ensemble (about 30 pcm), since
  the data are the same and only the 1e-9 total differs.
- **h5 route against the ENDF route** (this workspace's RECONR/BROADR/ACER):
  |z| <= 2. The existing ACE-against-ENDF parity measured -49.5 +- 31.9 pcm
  (1.55 sigma) and the NJOY data differ from this port's reconstruction by the
  same amount here, so a central expectation of about -50 pcm, with a real
  chance (about 1 in 8) of landing outside |z| <= 2.

### Row 3: data time, like for like

Machine: Intel Xeon 2.10 GHz, 4 cores, 15 GB, shared, CPU only.

- OpenMC reading the same library: about 1.5 s for 11 nuclides (2026-09-24
  figure, 1.54 +- 0.06 s; will be re-measured on this machine).
- outram-mc `read_nuclide` + `Nuclide::from_hdf5` on the same 11 files:
  **5 to 30 s, central 12 s** (pure-Rust `hdf5-pure`, 110 MB of files, the
  uranium files dominate), i.e. **3x to 20x slower than OpenMC** and **5x to 30x
  faster than building from ENDF** (158.57 +- 10.80 s on 2026-09-24).
- So the restated like-for-like data-time ratio, outram-mc / OpenMC, is
  predicted at about 8x (range 3x to 20x), not the 103x of 2026-09-24.
- Transport time is unaffected by the data route (same samplers): predicted
  within 5 % of the ENDF-route transport time.

---

# RESULTS (2026-11-18, run after the predictions above were committed in `8d0f47d56`)

Hardware for every number below: Intel Xeon 2.10 GHz, 4 logical cores, 15 GB
RAM, Linux 6.18, CPU only, **shared with two other agents** (load average 2.5
to 5). Thread counts are stated per run. Raw logs: `logs/`.

## 1. Bit identity against `Nuclide::from_ace` on the ACE the library was built from

`material::nuclide::hdf5_real_tests` (env-gated), all 11 nuclides, converted
at `294K` with the `0K` elastic, compared field by field with a streaming hash
of the `Debug` text. U-235 and U-238 reference ACE: the `reference-data/ace`
NJOY2016 293.6 K tables (their cross sections match the library's, 0 differing
words, so they are the same tables); the other nine are the ACE the library was
built from. U-238 has no 0 K companion in `reference-data/ace`, so its DBRC is
not compared (the `.h5` supplies one). Log: `logs/bit_identity_test.log`.

| item | predicted | outcome |
|---|---|---|
| sampled cross sections (MT != 1) | 0 differing words | **0 differing words on all 11 nuclides** (U-238: 0 of 1 564 692) |
| MT=1 total | NOT identical, <= 1e-8 | **NOT identical, and the bound MISSED**: worst relative 4.4e-9 (H-1), **9.98e-7 (B-10, 31.25 eV)**, 7.1e-8 (O-16), 3.1e-8 (Al-27), 5.8e-8 (Si-28), 5.3e-8 (Si-29), 5.7e-9 (Si-30), 5.0e-9 (Mn-55, U-234/235/238). The arrays are the ACE's, so this is ESZ's total against the sum of the table's own non-redundant partials: inherent to the table, not to the conversion. The test pins 1e-6 (set after this run). Effect on k: none resolvable (one nuclide, one energy). |
| elastic and level angular tables | identical or <= 1 ulp | **1 ulp** on B-10 (12 words, worst 2.0e-16), U-234 (1 word), U-235 (1 word); identical elsewhere |
| `continuous` / `correlated` / `kalbach-mann` rows | identical | **identical** (chi, MT 91/16/17, `other_channels`, all 11) |
| fission chi incl. U-234 partials | identical | **identical** |
| URR tables | identical | **identical** (U-234/235/238, Mn-55) |
| delayed data | spectra identical, lambda <= 1 ulp, nu_d and p_k <= 1e-12 | spectra **identical**; lambda 0 to 2.2e-16; nu_d <= 1.9e-16; p_k <= 3.2e-16 relative (better than predicted) |
| total nu | identical | **identical** |
| `0K` elastic (DBRC) | identical to the 0 K ACE route | **identical** (all but U-238, no ACE companion) |
| MT=5 yield regions | (not predicted) | first run: `[(n, 2)]` against the ACE route's empty `NR = 0`, same function in a different spelling; mapped back to the empty list, then **identical** |

Two defects of mine were found by this comparison and fixed before the table
above: the `0K` label has no `kTs` entry (the 0 K elastic was dropped and DBRC
was off), and OpenMC spells `|TY| = n` as `Polynomial((n,))` (the first
conversion of O-16 refused it).

## 2. Row 2: LCT-008 k, HDF5 route against ENDF route

`examples/lct008_hdf5_parity.rs`, case-1 lattice, 11 nuclides, S(a,b) H in H2O
from the ENDF tape on both arms, URR and DBRC on both, **2 transport threads**,
8 seeds (1 to 8).

**Settings differ from the ACE-roundtrip bar** (10000 x [250 + 400]): 5000
histories x [100 + 200], because 16 runs of the bar's size do not fit this
shared machine's budget. The gate is the same: |z| <= 2.

| arm | k (mean +- sem, 8 seeds) | seed sd |
|---|---|---|
| ENDF route | 1.00236 +- 0.00055 | 156 pcm |
| HDF5 route | 1.00275 +- 0.00034 | 95 pcm |
| **HDF5 - ENDF** | **+38.7 +- 64.5 pcm, z = +0.60** | |

**PASS** (|z| <= 2). The prediction was about -50 pcm with a 1-in-8 chance of
|z| > 2; the sign is the other way and the size is inside the noise. The
NJOY-ACE arm of the prediction was **not run** (it would repeat section 1, and
U-238 has no 0 K companion there). Per-seed values: `logs/parity.log`.

## 3. Row 3: the sweep with the fourth arm, data time restated

Pin cell of `lct008_pitch_sweep` (the model that has an OpenMC deck, gh:#550),
pitch 1.63576 cm, 11-nuclide tier, 5000 x [50 + 200], **2 threads**, seeds 1 to
4. **The 2026-09-24 sweep's model could not be reproduced** (its script is not
in the repository), so these are new numbers on the pin cell, compared with
each other and not with the 2026-09-24 table.

| arm | nuclear data | transport per run (4 runs) | k (mean, seed sd) |
|---|---|---|---|
| OpenMC 0.15.3, its HDF5 library | **1.07 +- 0.02 s** (whole initialisation: XML, 11 neutron + 2 thermal files) | 36.5 s | 1.06195, 101 pcm |
| outram-mc, same HDF5 library | **0.6 s** for the 11 nuclides (read 0.5 s + convert 0.1 s) | 202 s (198 to 207) | 1.06213, 97 pcm |
| outram-mc, ENDF tapes | **265 s** for the 11 nuclides | 212 s (204 to 217) | 1.06188, 170 pcm |

The two outram arms spend a further 33 s on the S(a,b), built from the ENDF
tape in both (`c_H_in_H2O.h5` is **not read**), so their whole data time is 33.6
s (HDF5) and 297.4 s (ENDF); the comparison with OpenMC's 1.07 s is like for
like on the nuclides only. k: HDF5 - ENDF +25.7 +- 97.9 pcm (z 0.26), HDF5 -
OpenMC +18.5 +- 70.0 pcm (z 0.26), ENDF - OpenMC -7.2 +- 98.6 pcm (z -0.07)
(4 seeds, a sanity check only).

Restated data-time ratios (nuclides only, same machine):

- ENDF route / OpenMC: **about 250x** (265 s / 1.07 s; 103x on 2026-09-24, load
  differs);
- outram-mc from the HDF5 library / OpenMC: **about 0.6x** (0.6 s / 1.07 s;
  OpenMC's 1.07 s also covers the two thermal files and the XML and was not
  split, so the true like-for-like ratio is somewhat higher);
- HDF5 route / ENDF route: about 440x faster.

Transport per run is within 5 % across the two outram arms (202 s against
212 s, 4.7 %) and about 5.5x OpenMC's 36.5 s.

### Prediction against outcome (row 3)

| prediction | outcome |
|---|---|
| outram HDF5 data time 5 to 30 s, central 12 s | **MISSED, 10x to 50x too slow a guess**: 0.6 s. `hdf5-pure` reads these files in well under a second; the guess assumed decode costs that do not occur |
| 3x to 20x slower than OpenMC, central 8x | **MISSED**: about 0.6x, not slower |
| 5x to 30x faster than ENDF | **MISSED the other way**: about 440x |
| transport within 5 % of the ENDF route | hit (4.7 %) |

## 4. What is not done

- Thermal scattering is not read from `c_H_in_H2O.h5` / `c_Graphite_30p.h5`:
  every outram arm above builds S(a,b) from the ENDF tape, so "starting from the
  library" is complete for neutron data only.
- The NJOY-ACE route was not run as a k arm (section 2).
- Only one library was converted (NJOY2016, 294K and 0K). Several transport
  temperatures need the caller to pick the label (`read_nuclide_selected`);
  there is no interpolation between temperatures.
- `nbody` is decoded and ported but no LCT-008 file carries it, so no real file
  covers it.
- `UrrTables::write` declares the shape `(n_energy, n_bands, 6)` where OpenMC's
  files are `(n_energy, 6, n_bands)` (doc corrected, writer untouched).
