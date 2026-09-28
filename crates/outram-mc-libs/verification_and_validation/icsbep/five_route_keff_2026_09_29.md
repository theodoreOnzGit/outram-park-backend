# Four ICSBEP cases, five code × data routes — k_eff

**Class:** verification, i.e. code-to-code and data-route-to-data-route, plus a
comparison with the three critical experiments. **No human V&V sign-off.** This
is an AI-assisted draft under `RESPONSIBLE_USE.md`. It is not for any
operational, licensing or safety use.

**Status (2026-09-29): measured.** The campaign ran 2026-09-29 06:00–06:23 SGT
at commit `be4206c2d3` and wrote 576 rows. The only failures were HST-009 on
routes 3 and 5, both from the F-19 refusal predicted in §4 of the preparation
findings.

The campaign found **two outram-mc ACE-reader defects (GitHub #366)** and **one
unexplained ENDF-route gap on HST-009 (GitHub #367)**. The *Predictions*
section was written the evening before, when no campaign number existed.

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

## Results (2026-09-29, commit `be4206c2d3`, 32 seeds per case × route)

Figure: [`five_route_keff/figures/five_route_keff.png`](five_route_keff/figures/five_route_keff.png).
Tables: `five_route_keff/data/summary_keff.{csv,md}`, generated from
`data/per_seed_keff.csv` by `scripts/make_figure.py`.

### Results table

Uncertainty is the sem over seeds. Δ vs route 1 carries σ = √(sem² + sem₁²).

| case | route | k_eff ± sem | seed sd [pcm] | Δ vs k = 1 [pcm] | Δ vs route 1 [pcm] |
|---|---|---|---|---|---|
| Godiva | 1 OpenMC + NJOY2016 | 1.00016 ± 0.00021 | 118 | +16 ± 21 | reference |
| Godiva | 2 OpenMC + Rust-NJOY | 1.00016 ± 0.00021 | 118 | +16 ± 21 | +0 ± 29 |
| Godiva | 3 outram-mc + NJOY2016 ACE | 1.01869 ± 0.00031 | 175 | +1869 ± 31 | **+1853 ± 37** (defect #366) |
| Godiva | 4 outram-mc + Rust-NJOY ENDF | 0.99933 ± 0.00035 | 197 | −67 ± 35 | −83 ± 41 (2.0σ) |
| Godiva | 5 outram-mc + Rust-NJOY ACE | 1.01869 ± 0.00031 | 175 | +1869 ± 31 | **+1853 ± 37** (defect #366) |
| Jemima | 1 | 0.99594 ± 0.00017 | 99 | −406 ± 17 | reference |
| Jemima | 2 | 0.99594 ± 0.00017 | 99 | −406 ± 17 | +0 ± 25 |
| Jemima | 3 | 0.98671 ± 0.00037 | 207 | −1329 ± 37 | **−923 ± 40** (defect #366) |
| Jemima | 4 | 0.99699 ± 0.00029 | 163 | −301 ± 29 | +106 ± 34 (3.1σ) |
| Jemima | 5 | 0.98671 ± 0.00037 | 207 | −1329 ± 37 | **−923 ± 40** (defect #366) |
| HST-009 | 1 | 1.00234 ± 0.00022 | 127 | +234 ± 22 | reference |
| HST-009 | 2 | 1.00138 ± 0.00024 | 134 | +138 ± 24 | −96 ± 33 (2.9σ) |
| HST-009 | 3 | not run (#365) | | | |
| HST-009 | 4 | 0.99962 ± 0.00027 | 152 | −38 ± 27 | **−271 ± 35** (7.8σ, #367) |
| HST-009 | 5 | not run (#365) | | | |
| LCT-008s | 1 | 0.85107 ± 0.00019 | 106 | not meaningful | reference |
| LCT-008s | 2 | 0.85053 ± 0.00021 | 121 | not meaningful | −54 ± 28 (1.9σ) |
| LCT-008s | 3 | 0.85058 ± 0.00026 | 150 | not meaningful | −49 ± 32 (1.5σ) |
| LCT-008s | 4 | 0.85078 ± 0.00030 | 167 | not meaningful | −29 ± 35 (0.8σ) |
| LCT-008s | 5 | 0.85041 ± 0.00030 | 170 | not meaningful | −66 ± 35 (1.9σ) |

**Which numbers to quote.**

- **Routes 1, 2 and 4** are the valid measurements.
- **Routes 3 and 5 on Godiva and Jemima** measure defect #366, not the code, and
  must not be quoted as outram-mc results.
- **Routes 3 and 5 on LCT-008s** are unaffected: that case has no U-234, and its
  inelastic scattering matters little in a thermal system.
- **Estimator check.** OpenMC's generation-mean k agrees with its combined k to
  within 1σ on all four cases (e.g. Godiva 1.00010 ± 0.00025 against 1.00016).
  The estimator choice is not a source of difference.

### The predictions, checked

1. **Routes 1 ≡ 2 and 3 ≡ 5, seed for seed, on Godiva and Jemima — HELD.**
   All 32 seeds on all four pairings are identical in every printed digit, `k`
   and `k_alt` both. The two ACE libraries are interchangeable for uranium, as
   §1 of the preparation findings predicted from the table comparison.
2. **Thermal route differences carried by the S(α,β) table alone — consistent,
   weakly tested.** Route 2 − 1 is −96 ± 33 pcm on HST-009 and −54 ± 28 pcm on
   LCT-008s. Route 5 − 3 on LCT-008s is −17 ± 40 pcm. No sign was predicted. Both
   OpenMC numbers are negative, and HST-009's is 2.9σ. That is compatible with
   the unexplained emission-table differences in §3 of the preparation findings,
   but this campaign cannot attribute it.
3. **Route 4 − route 3 below ~250 pcm — HELD on LCT-008s (+20 ± 40), FAILED
   on Godiva (−1936) and Jemima (+1028).** The failure is defect #366.
4. **Route 3 − route 1 as the cleanest transport comparison — FAILED as
   framed.** On the fast cases it measures an ACE-reader defect, not transport.
   It stands only on LCT-008s (−49 ± 32).

### The ACE-route defect: localised, confirmed, not fixed (GitHub #366)

Diagnostic runs are in `data/diagnostics.csv`, all at campaign settings. The
driver's `--ablate` and `--endf-nuclides` flags were added for these runs; the
campaign never uses them.

**Per-nuclide swap on Godiva, route 3, 16 seeds.** One nuclide at a time is
loaded from ENDF instead of ACE:

| nuclide from ENDF | route 3 result |
|---|---|
| U-235 | +1869 → −612 pcm |
| U-234 | → +2361 |
| U-238 | → +1885 (no change) |

**The quantities transport consumes, both routes**
(`examples/ace_vs_endf_nuclide_probe.rs`):

- For U-235, the cross sections, ν̄(E), the sampled fission spectrum, and the
  elastic and per-level CM mean cosines all agree to printed precision.
- **But the ACE route's inelastic channel list is `[(MT=4, Q=0)]` against 40
  discrete levels on the ENDF route.** `ce_decode::channel_mts` exists to
  reconstruct the total without double counting, and prefers the MT=4 lump
  whenever it is in MTR. `Nuclide::from_ace` reuses that rule to choose its
  transport channels. Inelastic scattering therefore loses no energy, and MT=91
  is never sampled.
- OpenMC marks MT=4 redundant when its components exist
  (`openmc/data/neutron.py:634-640`).
- U-234's table has no MT=18, only partials MT=19/20/21/38, and the pointwise
  tier reads fission from MT=18 only. **U-234 fission is zero on the ACE route**
  (ENDF route: 1.09 b at 1 MeV).

**Confirmation.** A temporary patch was applied, measured and reverted; none of
it is committed. It made `channel_mts` prefer the levels over the MT=4 lump.
The prediction was recorded before running.

| run | prediction | result | verdict |
|---|---|---|---|
| Jemima, patch | within ~60 pcm of route 1 | −457 → **−51 ± 33** pcm vs route 1 | met |
| Godiva, patch | within ~60 pcm of route 1 | **−583 ± 34** | failed: over-corrected, exposing the U-234 fission defect |
| Godiva, patch + U-234 from ENDF | — | **1.00011 ± 0.00030**, i.e. **−5 ± 37 pcm** vs route 1 | both defects together account for the whole gap |

**LCT-008s under the patch.** Route 3 gives 0.85157 ± 0.00032 and route 5 gives
0.84929 ± 0.00027, so route 5 − 3 = **−228 ± 42 pcm**. Unpatched it was
−17 ± 40. Recorded, not explained. It is measured on an uncommitted patch, and
the patch itself deserves re-measurement once #366 is fixed properly.

**History.** `examples/godiva_ace_roundtrip.rs` recorded +206 pcm (one seed) on
2026-09-23. That was before this workspace's ACE writer emitted MT=4
(2026-09-26, "ACE blocks reproduce NJOY2016's word for word"). NJOY2016's own
tables always carried MT=4, so **every outram-mc k on an NJOY ACE table, and
every one on a Rust-NJOY table since 2026-09-26, carries this defect** for
nuclides with MT=4 or with partial fission only.

### Against the experiments (routes 1 and 4, valid)

| case | OpenMC + NJOY2016 | outram-mc ENDF route | benchmark band |
|---|---|---|---|
| Godiva | +16 ± 21 | −67 ± 35 | ±100 (ICSBEP) |
| Jemima | −406 ± 17 | −301 ± 29 | ±300 (stand-in) |
| HST-009 | +234 ± 22 | −38 ± 27 | ±600 (stand-in) |

**Jemima's low residual is shared by OpenMC**, and OpenMC's is 105 pcm deeper.
That moves the long-running Jemima investigation in this directory's
`README.md`, which eliminated three material-property candidates. The −300
to −400 pcm now looks like a property of the model or the ENDF/B-VIII.0 data
that an independent code reproduces, not an outram-mc transport defect. This
is an inference from one code-to-code comparison and is not established.

**HST-009 flips.** outram-mc's ENDF route sits close to k = 1. OpenMC sits
+234 pcm above it, on data whose uranium and light-nuclide tables are identical
to within 7.7e-7. The −271 ± 35 pcm between them is unexplained (#367). The
S(α,β) representation is the first candidate, because route 2 − 1 shows the
thermal table alone moving this case by about 100 pcm.

### Cost

The campaign took 23 min of wall time on 8 threads, one job at a time:

- outram-mc ENDF-route loads: 33–45 s per case;
- transport per seed: 0.2–4.8 s, depending on case and code.

It came in under the 30–45 min estimate.

## What this cannot do

- **Benchmark uncertainties are not the handbook's.** Only Godiva's ±0.0010 is
  a quoted ICSBEP value. Jemima's ±0.003 and HST-009's ±0.006 are the
  examples' stand-ins, because the handbook is licence-restricted (see
  `README.md` in this directory).
- **LCT-008s says nothing about LEU-COMP-THERM-008.**
- **HST-009 on routes 3 and 5**: blocked by #365, and #366 would also need fixing
  before those two rows could be trusted.
- **Only one data library.** Everything is ENDF/B-VIII.0 at 293.6 K.

## Appendix — OpenMC provenance and driver script (verbatim)

**Provenance of the OpenMC side:**

| item | value |
|---|---|
| OpenMC | 0.16.1-dev25, commit `d7d3284a1b13d7cae020e0d8fea9f1e60d91b18b`, `~/Documents/research/openmcbin/bin/openmc` |
| cross sections | built in this study (see *Routes*) |
| run settings | 5000 particles, 40 inactive + 120 active batches, `-s 8`, seeds 1..32 |

No Shannon-entropy check was run on either code. The 40 inactive generations
match the outram-mc examples' own settings.

The ACE→HDF5 converter is committed beside it, as
[`five_route_keff/openmc_inputs/ace_to_hdf5_route.py`](five_route_keff/openmc_inputs/ace_to_hdf5_route.py).

```python
"""OpenMC decks for the four ICSBEP cases of the five-route study (routes 1, 2).

The SAME models `examples/icsbep_five_route_keff.rs` runs on outram-mc, with the
atom densities and dimensions copied from that file (which cites its own
sources: godiva_keff_endf_local.rs, jemima_keff.rs, hst009_keff.rs,
lct008_ace_roundtrip.rs). Only the cross-section library differs between
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


def lct008s():
    fv, wv = 0.30, 0.70
    mix = lambda f, w: f * fv + w * wv  # noqa: E731
    m = mat("LCT-008 case 1, homogenised 30/70", [
        ("U235", mix(0.00056868, 0.0)), ("U238", mix(0.022268, 0.0)),
        ("O16", mix(0.045683, 0.033369)), ("H1", mix(0.0, 0.066737)),
        ("B10", mix(2.6055e-07, 1.6769e-05))], sab=True)
    s = openmc.Sphere(r=40.0, boundary_type="vacuum")
    geo = openmc.Geometry([openmc.Cell(fill=m, region=-s)])
    src = openmc.stats.spherical_uniform(r_outer=40.0)
    return [m], geo, src


CASES = {"godiva": godiva, "jemima": jemima, "hst009": hst009, "lct008s": lct008s}


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
    ap.add_argument("--openmc", default=os.path.expanduser("~/Documents/research/openmcbin/bin/openmc"))
    a = ap.parse_args()

    mats, geo, space = CASES[a.case]()
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
                    f"{wall:.1f}", "", n_nuc, "", "", str(CASES[a.case] in (hst009, lct008s)).lower(),
                    a.commit, f"{kgen.mean():.6f}"])
    print(f"{a.case} {a.label} seed {a.seed}: k = {k.nominal_value:.5f} +/- {k.std_dev:.5f}"
          f"  (generation mean {kgen.mean():.5f})  {wall:.1f} s")


if __name__ == "__main__":
    main()
```
