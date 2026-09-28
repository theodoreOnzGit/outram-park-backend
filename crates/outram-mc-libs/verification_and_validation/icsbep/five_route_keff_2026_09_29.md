# Four ICSBEP cases, five code × data routes — k_eff

**Class:** verification, i.e. code-to-code and data-route-to-data-route, plus a
comparison with the three critical experiments. **No human V&V sign-off.** This
is an AI-assisted draft under `RESPONSIBLE_USE.md`. It is not for any
operational, licensing or safety use.

**Status (2026-09-28, 22:15 SGT): the preparation is done, the campaign has not
been run.** The maintainer scheduled the seed campaign for 2026-09-29 06:00 SGT.
Everything in *Results* is pending. The *Preparation findings* are measured
and final. The *Predictions* were written before any campaign number existed.

Files: [`five_route_keff/`](five_route_keff/). Launcher:
`five_route_keff/run_all.sh`. Per-seed data: `five_route_keff/data/per_seed_keff.csv`.
Figure and summary: `five_route_keff/scripts/make_figure.py`.

## Methodology

### Cases

| key | benchmark | model | source of the numbers |
|---|---|---|---|
| `godiva` | HEU-MET-FAST-001 | bare sphere, r = 8.7407 cm, U-234/235/238 | `examples/godiva_keff_endf_local.rs` |
| `jemima` | IEU-MET-FAST-002 | 4-cell cylinder: core plus natural-U reflector | `examples/jemima_keff.rs` |
| `hst009` | HEU-SOL-THERM-009 case 1 | 3 spheres: solution, Al tank, water. U-236 and Cu/Zn dropped, O-17 folded into O-16 | `examples/hst009_keff.rs` |
| `lct008s` | **LEU-COMP-THERM-008 case 1, SIMPLIFIED** | homogenised sphere, r = 40 cm, 30 % fuel / 70 % borated water by volume | `examples/lct008_ace_roundtrip.rs` |

**How "simplified" differs from the benchmark.** The real LCT-008 case 1 is a
lattice of 22 distinct 15 × 15 assemblies of 1.03 cm UO₂ pins. The pins are
clad in Al-6061 and sit in 1511 ppm borated water. Its whole value is the
lumping: a pellet is ~176 mean free paths across at 6.674 eV. `lct008s`
removes all of that:

- fuel and moderator are homogenised into one sphere;
- the cladding, U-234 and B-11 are dropped;
- the fuel volume fraction (0.30) is a chosen value, not a benchmark value.

**Its k (≈ 0.85) is therefore not comparable to 1.0000 and is not an LCT-008
result.** It is compared between routes only. One change from the source
example: H-1 carries S(α,β) H in H₂O on every route here. The round-trip
example used free-gas H.

OpenMC runs **exactly these four models**, not the full benchmark decks
(`openmc_inputs/icsbep_openmc.py`). Densities and dimensions are copied from
the Rust driver.

### Routes

| route | transport | nuclear data |
|---|---|---|
| 1 | OpenMC 0.16.1-dev25 (`d7d3284a1`) | NJOY2016 2016.79 (`ac5adf5`) ACE → HDF5 |
| 2 | OpenMC, same build | this workspace's Rust NJOY port → ACE → HDF5 |
| 3 | outram-mc (`examples/icsbep_five_route_keff.rs`) | NJOY2016 ACE |
| 4 | outram-mc | Rust NJOY, ENDF read directly (`Nuclide::from_endf_file_with_speed`, default tier) |
| 5 | outram-mc | Rust-NJOY ACE |

All data comes from ENDF/B-VIII.0 tapes in `reference-data/endf/`, processed at
293.6 K with a RECONR tolerance of 0.001.

- **NJOY2016 decks.** The committed `openmc_godiva_cross_code/make_ace.sh` for
  RECONR → BROADR → PURR(20 bins, 64 ladders) → ACER, and `make_ace_0k.sh` for
  the 0 K companion.
- **Uranium from the submodule.** U-234/235/238 at 293.6 K and U-235 at 0 K are
  taken from the `reference-data/ace` submodule, which was built with the same
  deck and the same NJOY build.
- **H in H₂O.** Built with OpenMC's `make_ace_thermal` driver with `iwt = 1`
  (IFENG = 0), because outram-mc refuses IFENG = 2.
- **Rust NJOY.** `njoy-outram-park-fork/examples/write_ace_library.rs` writes
  `build_full_with_purr(20, 64, 10 000)`, a 0 K companion (`build_full`,
  kT = 0) and `AceTable::thermal_from_mf7`. The thermal table uses NJOY2016's
  incident grid and bin counts (118 energies, 64 bins × 16 cosines) and the
  tape's own B(6) = 2 as natom.
- **HDF5 conversion.** `openmc_inputs/ace_to_hdf5_route.py` converts both
  libraries. It attaches 0 K elastic from each library's own 0 K table. It does
  not use `add_elastic_0K_from_endf`, which would run NJOY inside route 2.

### Physics carried, per route, and every asymmetry

| term | OpenMC (1, 2) | outram-mc ENDF (4) | outram-mc ACE (3, 5) |
|---|---|---|---|
| URR probability tables | on (`ptables`), from the table's UNR block | on, built at load: PURR 20 bins / **16 ladders / 2000 samples** | on, from the UNR block: 20 / 64 (NJOY) or 20 / 64 / 10 000 (Rust) |
| resonance elastic scattering | `dbrc`, 1e-5 eV ≤ E ≤ 1 keV, all nuclides | DBRC, E ≤ 1 keV, all nuclides | DBRC, E ≤ 1 keV, all nuclides (0 K companion paired by `from_ace_file`) |
| S(α,β) H in H₂O (hst009, lct008s) | ACE table, IFENG = 0 | from `tsl-HinH2O.endf` with this crate's own emission grid | ACE table, IFENG = 0 |
| k estimator in `k` column | combined (`StatePoint.keff`); `k_alt` = generation mean | generation mean | generation mean |

**Known asymmetries, stated rather than removed:**

- **URR ladders.** The ENDF route uses fewer PURR ladders and samples than
  either ACE library.
- **S(α,β) discretisation.** The ENDF route's S(α,β) uses a different
  discretisation from the ACE routes.
- **DBRC lower bound.** OpenMC's DBRC starts at 1e-5 eV because its API refuses
  0. outram-mc has no lower bound. This matters only below the S(α,β) cutoff
  for H and below 1e-5 eV otherwise.
- **k estimator.** OpenMC's headline estimator differs from outram-mc's. Both
  are recorded, and the comparison uses the seed-scatter uncertainty.

### Settings and statistics

- **Histories.** 5000 histories × [40 inactive + 120 active] on every route,
  for both codes.
- **Threads.** 8 threads per job, run one job at a time (`RAYON_NUM_THREADS=8`,
  `openmc -s 8`).
- **Seeds.** 32 independent seeds per case × route (seeds 1..32). The same seed
  numbers are used on every route, but runs are not paired across codes.
- **Uncertainty.** Quoted per case × route as the **standard error of the mean
  over seeds** (sd / √n). Each run's internal σ is not averaged.
- **Δ vs experiment.** `(mean − 1) × 1e5` pcm ± sem.
- **Δ vs route 1.** Its σ is √(sem_r² + sem_1²), because the runs are
  independent.

## Preparation findings (measured 2026-09-28)

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

### 3. Two defects in this study's own thermal-table build, caught before any run

The first Rust H(H₂O) table was built wrong twice. The defects were in the new
library writer, not in the port.

- **Wrong temperature.** `parse_mf7` returns the tape's **base-temperature**
  (283.6 K) S(α,β), which was then evaluated at 293.6 K. σ_inel came out 1.68×
  NJOY's at 1e-5 eV.
- **Wrong natom.** natom was passed as 1 where the tape's B(6) is 2, giving
  σ_inel **2.0×** NJOY's across the grid.

Both were found by differencing against the NJOY2016 table, after a smoke run
had HST-009 route 2 about 2800 pcm below route 1 and LCT-008s about 2700 pcm
above it.

**After the fix**, compared with the NJOY2016 table:

| quantity | worst | typical |
|---|---|---|
| σ_inel | **6.6e-2** relative, at the lowest grid point (1e-5 eV) | median 3.9e-3 relative; ≈3e-2 at 5.85–10 eV |
| emission E′ | 8.2e-2 relative | median 1.5e-3 relative |
| emission cosines | 0.144 absolute | mean 1.7e-3 absolute |

These worst values are larger than the recorded thermal-ACE parity
(`njoy-outram-park-fork/verification_and_validation/acer_thermal_vs_njoy2016.md`:
7.6e-3 on E′, 4.3e-2 on cosines, taken at 283.6 K). **This is unexplained**,
and it is carried into the interpretation of routes 2 and 5 on the two thermal
cases.

### 4. Routes 3 and 5 cannot run HEU-SOL-THERM-009

`Nuclide::from_ace_file` refuses F-19 on **both** libraries. F-19's MT=16
(n,2n) is an ACE LNW chain of two law-61 distributions, applicable from
10.99 MeV to 20 MeV, and the transport side has no mixture-of-correlated-laws
representation. Filed as **GitHub #365**.

**Not worked around.** Dropping F-19 or its MT=16 would change the model. The
figure marks those two points "not run".

### 5. Cost

Full-length single runs, 8 threads:

| run | load | transport per seed |
|---|---|---|
| outram-mc LCT-008s, route 3 | 2 s | 1.6 s |
| outram-mc HST-009, route 4 | 43 s | 4.3 s |
| OpenMC HST-009, route 1 | — | 4.4 s end to end |

ENDF-route loads are 33–45 s per case, and ACE-route loads about 2 s. The whole
campaign (4 × 5 × 32 seeds) is estimated at **30–45 min** of wall time.

## Predictions (written before the campaign)

1. **Routes 1 ≡ 2 and 3 ≡ 5** exactly, seed by seed, on Godiva and Jemima (§1).
2. **Route 2 − route 1 and route 5 − route 3 on HST-009 and LCT-008s** are
   carried by the S(α,β) table alone, because every other neutron table has
   identical ESZ. No sign is predicted: the σ_inel residual changes sign across
   the grid.
3. **Route 4 − route 3.** This isolates ENDF-direct against ACE inside
   outram-mc, with the PURR-ladder and S(α,β)-grid asymmetries above. The
   earlier 8-seed LCT-008s parity record bounds it below ~250 pcm at 2σ on the
   free-gas model.
4. **Route 3 − route 1** is the transport comparison on identical NJOY2016
   data. It is the cleanest number here.

## Results

**Pending the campaign scheduled for 2026-09-29 06:00 SGT.** This section will
record:

- the table from `data/summary_keff.md`;
- the figure `figures/five_route_keff.png`;
- the date, commit and seed count;
- each prediction above, marked met or refuted.

## What this cannot do

- **Benchmark uncertainties are not the handbook's.** Only Godiva's ±0.0010 is
  a quoted ICSBEP value. Jemima's ±0.003 and HST-009's ±0.006 are the
  examples' stand-ins, because the handbook is licence-restricted (see
  `README.md` in this directory).
- **LCT-008s says nothing about LEU-COMP-THERM-008.**
- **HST-009 on routes 3 and 5**: blocked by #365.
