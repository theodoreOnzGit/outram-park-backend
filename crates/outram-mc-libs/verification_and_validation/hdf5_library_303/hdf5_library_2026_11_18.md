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
