# Four ICSBEP cases, five code × data routes — k_eff

**Class:** verification (code-to-code and data-route-to-data-route) plus
comparison with three critical experiments. **No human V&V sign-off.** This is
an AI-assisted draft under `RESPONSIBLE_USE.md`, not for any operational,
licensing or safety use.

**Status (2026-09-30).** All five routes are complete for all four cases.
Routes 3–5 are at `0414bc8277`, after the OpenMC-parity audit (#407); see
"Results — after the OpenMC-parity audit" below. The lattice is at 96 seeds on
routes 1 and 3–5. The table that follows is the superseded interim status,
kept for the history.

~~**Status (2026-09-29, INTERIM).**~~

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
| P6 | at the final code, every case × route within 2σ of route 1 | **held on 11 of 12 outram-mc cells**; the lattice on route 3 is −36 ± 15 (2.5σ), and an extension is declared |
| P5 | route 3 with the #366 fix lands within ~60 pcm of route 1 on Godiva and Jemima | met on the patch test (Godiva −5 ± 37, Jemima −51 ± 33); the post-fix campaign gives −54 ± 40 and +124 ± 38, the latter since attributed to the delayed spectra (see below) |
| P7 | #459 (route-4 S(α,β) as OpenMC's library): lattice +40 to +100, HST-009 +50 to +150 pcm, paired | **refuted**: −40.0 ± 17.5 (lattice) and −3.7 ± 37.8 (HST-009); route 4 now agrees with route 5 (#459) |
| P8 | #460 (combed fission source): +0 to +5 pcm on the lattice, below what 96 seeds resolve | not measured paired; the unpaired route-3 move from f78b5180d5 (96 seeds each) is +5 ± 10, consistent with it and with zero |
| P9 | at the audited code, every case × route within 2σ of route 1 | **held on 11 of 12 outram-mc cells**; the lattice on route 3 is −18 ± 8 (2.3σ) at 96 seeds a side |

## Results — after the OpenMC-parity audit (2026-09-30, routes 3/4/5 at `0414bc8277`)

**These supersede the 2026-09-29 table below.** The audit (#407) compared
outram-mc with OpenMC `d7d3284a1` routine by routine and fixed:
- #459: the route-4 S(α,β) cutoff is the tape's `E_max`, and σ_inel is THERMR's `calcem` xsi at tol 0.001;
- #460: the fission source is combed, as OpenMC's `synchronize_bank`;
- #313: the free-gas/DBRC kT comes from the data temperature;
- #461: variance reduction applies on the parallel path;
- #463 items 1–2;
- a7e6506945: the DBRC rejection loop is unbounded, as upstream.

**Protocol.**
- Routes 3, 4 and 5 were re-run at `0414bc8277`, with binaries snapshotted in `target/five_route_keff/bin/0414bc8277/`. Campaign settings are unchanged.
- Godiva, Jemima and HST-009 have 32 seeds each.
- The lattice has 96 seeds on routes 3–5. Route 1 is taken to 96 as well: seeds 33–96 from the extension run at the same settings (#407).
- The route-5 thermal ACE, regenerated at `0414bc8277`, is data-identical to the library used (only the header comment line differs), so the libraries were not rebuilt.
- The replaced rows are archived in `data/per_seed_keff_pre_audit_routes345.csv`.

| case | route | n | k_eff ± sem | seed sd [pcm] | Δ vs k=1 [pcm] | Δ vs route 1 [pcm] |
|---|---|---|---|---|---|---|
| godiva | route1 | 32 | 1.00016 ± 0.00021 | 118 | +16 ± 21 | reference |
| godiva | route2 | 32 | 1.00016 ± 0.00021 | 118 | +16 ± 21 | +0 ± 29 (+0.0σ) |
| godiva | route3 | 32 | 0.99976 ± 0.00036 | 204 | -24 ± 36 | -40 ± 42 (-1.0σ) |
| godiva | route4 | 32 | 0.99948 ± 0.00027 | 151 | -52 ± 27 | -68 ± 34 (-2.0σ) |
| godiva | route5 | 32 | 0.99976 ± 0.00036 | 204 | -24 ± 36 | -40 ± 42 (-1.0σ) |
| jemima | route1 | 32 | 0.99594 ± 0.00017 | 99 | -406 ± 17 | reference |
| jemima | route2 | 32 | 0.99594 ± 0.00017 | 99 | -406 ± 17 | +0 ± 25 (+0.0σ) |
| jemima | route3 | 32 | 0.99594 ± 0.00026 | 146 | -406 ± 26 | +0 ± 31 (+0.0σ) |
| jemima | route4 | 32 | 0.99609 ± 0.00027 | 152 | -391 ± 27 | +15 ± 32 (+0.5σ) |
| jemima | route5 | 32 | 0.99594 ± 0.00026 | 146 | -406 ± 26 | +0 ± 31 (+0.0σ) |
| hst009 | route1 | 32 | 1.00234 ± 0.00022 | 127 | +234 ± 22 | reference |
| hst009 | route2 | 32 | 1.00275 ± 0.00031 | 174 | +275 ± 31 | +41 ± 38 (+1.1σ) |
| hst009 | route3 | 32 | 1.00205 ± 0.00022 | 127 | +205 ± 22 | -29 ± 32 (-0.9σ) |
| hst009 | route4 | 32 | 1.00198 ± 0.00032 | 181 | +198 ± 32 | -36 ± 39 (-0.9σ) |
| hst009 | route5 | 32 | 1.00255 ± 0.00023 | 130 | +255 ± 23 | +21 ± 32 (+0.7σ) |
| lct008 | route1 | 96 | 1.00244 ± 0.00005 | 50 | +244 ± 5 | reference |
| lct008 | route2 | 32 | 1.00248 ± 0.00008 | 45 | +248 ± 8 | +4 ± 9 (+0.4σ) |
| lct008 | route3 | 96 | 1.00225 ± 0.00006 | 59 | +225 ± 6 | -18 ± 8 (-2.3σ) |
| lct008 | route4 | 96 | 1.00236 ± 0.00008 | 74 | +236 ± 8 | -8 ± 9 (-0.8σ) |
| lct008 | route5 | 96 | 1.00241 ± 0.00006 | 64 | +241 ± 6 | -3 ± 8 (-0.3σ) |

**What the numbers say.**
- **Every cell is within 2σ of route 1 except the lattice on route 3: −18 ± 8 pcm (2.3σ)** at 96 seeds a side. It was −24 ± 9 at `f78b5180d5`. The 96-seed extension declared on 2026-09-29 therefore leaves a small but still resolved residual.
- The lattice on routes 4 and 5 is −8 ± 9 and −3 ± 8.
- ~~Godiva route 4 is at −1.99σ (−68 ± 34). Nothing in the audit fixes acts on a bare fast sphere by more than a few pcm, and at 2σ this is not investigated further.~~ **Re-measured 2026-10-05 (#546): the 32-seed −52 was a low draw.** Route 4 at HEAD reproduces these 32 seeds digit for digit, and 288 seeds give −10 ± 11 pcm, which is −26 ± 24 (1.1σ) from route 1. See "Later measurements" below.
- **The #459 prediction was wrong in sign** (P7). The fix moved route 4 on the lattice by −40.0 ± 17.5 pcm, paired. That brought route 4 into agreement with route 5, which is NJOY's processing of the same S(α,β) evaluation.
- **DBRC worth on the lattice (route 3 against OpenMC, #407).** Outram-mc is −13.9 ± 8.3 pcm (96 paired seeds); OpenMC is +27.6 ± 9.9 (32). The difference is **−41.5 ± 12.9 pcm (3.2σ)**. Before the sampler fix outram-mc's worth was −42.6 ± 20.
  - This disagreement has the same sign as the route-3 residual and exceeds it in size. It is the leading lead.
  - The single-scatter sampler test at 6.4, 20.5 and 36.4 eV agrees with OpenMC (worst z = 3.6). The difference must therefore sit elsewhere: higher resonances, other nuclides, or an interaction in transport.
  - Declared next (on #407): a per-nuclide split of the DBRC worth, and per-resonance distribution tests.

## Later measurements (2026-10-05)

### Godiva route 4 extended to 288 seeds (GitHub #546)

**Why.** The audited table puts Godiva route 4 at −52 ± 27 pcm. That is
2.3σ from the 256-seed `+16 ± 11` that `godiva_keff_endf_local.rs` gated
against, and −68 ± 34 (2.0σ) from route 1.

**Method.** The route-4 driver `icsbep_five_route_keff --case godiva --route
endf`, binary built at `8b17079bc`, ran 2 threads with campaign settings
(5000 × [40 + 120]) on seeds 1–288. The prediction was written before the
extension: if the single-threaded ensemble harness and this multi-threaded
driver sample the same distribution, the 288-seed mean lies within 2σ of the
ensemble's −6 ± 5, roughly [−27, +15] pcm. Per-seed rows are in
[`data/per_seed_keff_godiva_route4_288_2026_10_05.csv`](five_route_keff/data/per_seed_keff_godiva_route4_288_2026_10_05.csv).

**Hardware.** Intel Xeon Processor @ 2.10 GHz, 2 of 4 shared logical cores
(`taskset -c 2,3`), 15.7 GB RAM, Linux 6.18, CPU only. Data took 101 s and
transport 3.3 s per seed.

**Results.**

| seeds | k_eff ± sem | seed sd [pcm] | Δ vs k=1 [pcm] | Δ vs route 1 [pcm] |
|---|---|---|---|---|
| 1–32 | 0.99948 ± 0.00027 | 151 | −52 ± 27 | identical, per seed, to the audited row above (32/32 to every printed digit) |
| 33–288 | 0.99995 ± 0.00012 | 187 | −5 ± 12 | |
| **1–288** | **0.99990 ± 0.00011** | 183 | **−10 ± 11** | **−26 ± 24 (−1.1σ)** |

- **The code did not move.** Route 4 reproduces itself bit for bit from
  `0414bc8277` to `8b17079bc`.
- **The −52 was a 32-seed low draw.** Seeds 1–32 sit −1.6σ from seeds
  33–288.
- **The prediction held.** The pooled ensemble of the same model,
  `godiva_keff_ensemble.rs` (1024 seeds, single-threaded per seed,
  `6faff1ed8`), gives −6 ± 5. That agrees with route 4's 288 seeds to
  −4 ± 12 pcm, and is now the recorded value `godiva_keff_endf_local.rs`
  gates against.
- **Route 4 now agrees with route 1** within 1.1σ. Godiva no longer
  qualifies the "every cell within 2σ" statement.

### LCT-008 lattice: worth of the 24 dropped nuclides, first measurement (GitHub #533)

**Question.** The lattice runs the 11-nuclide tier. What is the worth of the 24
nuclides that tier drops? They are B-11 in the water and Mg, Ti, Cr, Fe, Cu
and Zn in the Al-6061 clad. The case-1 cards name 35 nuclides; `lct008_keff.rs`'s
default `TAPES` lists 36, and Na-23 is not in this case.

**Prediction (written before any run).** The dropped nuclides are almost all
parasitic absorbers in the clad. From nominal Al-6061 shares and 2200 m/s
cross sections, they add about 9e-4 /cm of absorption, about 7 % on top of the
clad's own. That puts the worth, full minus 11, at about **−100 pcm**, with a
plausible range of −40 to −250. A positive worth resolved at more than 2σ would
refute the prediction.

**Method.**
- **Full arm.** `icsbep_five_route_keff --case lct008 --route endf
  --full-nuclides`, a new arm built at `8b17079bc`. Everything else is route 4
  unchanged: loader, URR and DBRC defaults, S(α,β), 10 000 × [250 + 400] and
  the seeds. 35 nuclides; URR on 10, DBRC on 35.
- **11-nuclide arm.** The 96 route-4 seeds already in this record
  (`0414bc8277`). They were not re-run, because route 4 reproduces at
  `8b17079bc`: seeds 1 and 16, re-run, match the recorded `k` and internal σ to
  every printed digit.

**Size.** #533 asks for at least 32 paired seeds. The timed pilot gave 684 s
per full-arm seed and 368 s per 11-nuclide seed. Both used 2 threads on the
hardware below. 32 full seeds is therefore about 6 h of transport, more than
was available here. **16 full seeds** were run, about 3 h.

**Estimator, fixed after 3 of the 16 seeds and before anything was
resolved.** The two arms' random streams diverge at the first collision with a
dropped nuclide, so same-seed pairing carries almost no correlation. The
**primary** estimator is therefore unpaired: full (16) minus the 11-nuclide
arm (96). The paired 16-seed difference, which #533 asked for, is reported
beside it.

**Hardware.** Intel Xeon Processor @ 2.10 GHz, 2 of 4 shared logical cores
(`taskset -c 2,3`), 2 threads, 15.7 GB RAM, Linux 6.18, CPU only. Data took
155 s for the full arm and 134 s for the 11-nuclide arm. Transport took
597–730 s per full seed (seed 2 took 1162 s, run beside a build of mine) and
368–384 s per 11-nuclide seed. The full tier costs 1.8× per seed.

Per-seed rows:
[`data/per_seed_keff_lct008_full35_route4_2026_10_05.csv`](five_route_keff/data/per_seed_keff_lct008_full35_route4_2026_10_05.csv).

**Results.**

| arm | seeds | k_eff ± sem | seed sd [pcm] | Δ vs k=1 [pcm] |
|---|---|---|---|---|
| 11 nuclides (route 4, recorded) | 96 | 1.00236 ± 0.00008 | 74 | +236 ± 8 |
| 11 nuclides, seeds 1–16 only | 16 | 1.00205 ± 0.00016 | 64 | +205 ± 16 |
| **full, 35 nuclides** | **16** | **1.00171 ± 0.00019** | 76 | **+171 ± 19** |

| worth of the 24 dropped nuclides (full − 11) | value [pcm] | significance |
|---|---|---|
| ~~**unpaired, full 16 vs 11-nuclide 96 (primary)**~~ | ~~**−65 ± 21**~~ | ~~**3.2σ**~~ |
| ~~paired, seeds 1–16 (as #533 asked)~~ | ~~−35 ± 26~~ | ~~1.3σ; seed-to-seed correlation between the arms −0.14~~ |

**Superseded 2026-10-05 by the 32-seed extension below:** −75 ± 14 pcm
unpaired (primary), −50 ± 16 paired. The 16-seed rows above are kept as
measured; they are seeds 1–16 of the 32.

- **The prediction held** in sign and in range: −65 ± 21 against −100
  predicted (−40 to −250). The dropped nuclides are worth a few tens of pcm,
  and in the direction parasitic capture gives.
- **The two estimators agree to 1.3σ.** Their difference is the 11-nuclide
  arm's seeds 1–16 (+205 ± 16) against its 96-seed mean (+236 ± 8). The
  pairing gained nothing (correlation −0.14), as expected.
- **What it means for the quoted lattice result.** The full model would sit at
  about ~~**+171 ± 19 pcm**~~ **+161 ± 12 pcm (32 seeds, below)**, not
  +236 ± 8. The 11-nuclide figure stays the one to quote for the code-to-code
  comparison, because OpenMC ran the same 11 nuclides. Against k = 1, the
  simplification is worth about ~~−65~~ **−75** pcm.
- ~~**Not done: the request in full.** #533 asks for at least 32 pairs. This is
  16 full seeds, so the worth is resolved at 3.2σ, not at the precision 32
  seeds would give (about ±16 pcm). #533 stays open for the rest.~~ **Done
  2026-10-05:** seeds 17–32 were run; see the next subsection. The worth is
  not split by nuclide.

### LCT-008 lattice: dropped-nuclide worth at 32 full-model seeds (GitHub #533)

**Prediction ([posted on #533](https://github.com/theodoreOnzGit/outram-park-backend/issues/533#issuecomment-5997622924)
before any of seeds 17–32 ran).**
1. Unpaired worth at 32 full seeds against the 96 11-nuclide seeds: about
   **−70 pcm, ±16 expected, plausible range −105 to −35**. This combines the
   capture estimate (−100) with the 16-seed −65 ± 21, weighted toward the
   measurement.
2. The full arm's seeds 17–32 alone fall within 2σ of seeds 1–16, i.e. in
   [+117, +225] pcm.
3. Paired over seeds 1–32, the correlation between the arms stays near zero,
   |r| < 0.35.

Refutation: a positive worth resolved beyond 2σ, or a worth more negative than
−130 resolved beyond 2σ. The stopping point, 32 seeds, was declared with the
prediction, and the batch was not extended after its results were seen.

**Method.** The first 16 seeds' driver, rebuilt from `8b17079bc`
(`icsbep_five_route_keff --case lct008 --route endf --full-nuclides
--particles 10000 --inactive 250 --active 400`). Same loader, URR on 10 of 35
nuclides, DBRC on 35 of 35, S(α,β) on. Seeds 17–32 ran as four processes, one
per logical core (`taskset -c <core>`), at `--threads 1` instead of 2. The
multi-thread transport documents its eigenvalue as independent of thread
count. That was tested before the batch was used: **full seed 1, re-run at 1
thread, reproduces the 2-thread `k = 1.002295`, σ = 0.000619 to every printed
digit.** It is the `route4-full35-recheck1t` row of the CSV. The 11-nuclide
arm is again the 96 recorded route-4 seeds. The estimators are the ones fixed
for the first measurement: unpaired is primary, and paired is reported beside
it.

**Hardware.** Intel Xeon Processor @ 2.10 GHz, all 4 logical cores (one
1-thread process each), 15.7 GB RAM (about 560 MB per process), Linux 6.18,
CPU only. Data took 314–317 s per process. Transport took 2005–2130 s per seed
at 1 thread, with four processes sharing the machine. Per-seed rows are in the
same
[`data/per_seed_keff_lct008_full35_route4_2026_10_05.csv`](five_route_keff/data/per_seed_keff_lct008_full35_route4_2026_10_05.csv).

**Results.**

| arm | seeds | k_eff ± sem | seed sd [pcm] | Δ vs k=1 [pcm] |
|---|---|---|---|---|
| 11 nuclides (route 4, recorded) | 96 | 1.00236 ± 0.00008 | 74 | +236 ± 8 |
| 11 nuclides, seeds 1–32 only | 32 | 1.00211 ± 0.00010 | 56 | +211 ± 10 |
| full, 35 nuclides, seeds 1–16 (first measurement) | 16 | 1.00171 ± 0.00019 | 76 | +171 ± 19 |
| full, 35 nuclides, seeds 17–32 (this batch) | 16 | 1.00152 ± 0.00014 | 56 | +152 ± 14 |
| **full, 35 nuclides, seeds 1–32** | **32** | **1.00161 ± 0.00012** | 66 | **+161 ± 12** |

| worth of the 24 dropped nuclides (full − 11) | value [pcm] | significance |
|---|---|---|
| **unpaired, full 32 vs 11-nuclide 96 (primary)** | **−75 ± 14** | **5.4σ** |
| paired, seeds 1–32 | −50 ± 16 | 3.1σ; correlation between the arms −0.09 |

- **All three predictions held.**
  1. −75 ± 14 against a predicted −70, inside the −105 to −35 range. The
     paired −50 is inside it too.
  2. Seeds 17–32 give +152 ± 14, inside [+117, +225]. That is −19 ± 24
     (0.8σ) from seeds 1–16.
  3. The correlation is −0.09.
  Neither refutation condition was met.
- **The worth is resolved: the 24 dropped nuclides are worth −75 ± 14 pcm**
  on the lattice. It is negative, as parasitic capture in the clad trace
  elements and B-11 predicts, and smaller than the −100 capture estimate.
- **The two estimators differ by 2.3σ, and that is reported, not chosen
  between after the fact.** Their difference is exactly the 11-nuclide arm's
  seeds 1–32 (+211 ± 10) against its own 96-seed mean (+236 ± 8). A 32-seed
  subset of a 96-seed sample has σ = 11 pcm about the full mean, so this subset
  sits 2.3σ low. Within route 4, seeds 1–32 sit −37 ± 14 below seeds 33–96.
  Routes 3 and 5 split less (+4 ± 13 and −16 ± 13). Route 4's settings,
  commit (`0414bc8277`) and thread count are identical across the split. This reads
  as a low draw, not as a seed-dependent bias: if seed numbers carried a bias
  into both arms, the arms would correlate, and they do not (r = −0.09). The
  unpaired estimator stays primary, as declared before either measurement.
- **What it means for the quoted lattice result.** The full model sits at
  **+161 ± 12 pcm** against k = 1. The 11-nuclide model sits at +236 ± 8. The
  11-nuclide figure stays the one quoted for the code-to-code comparison,
  because OpenMC ran the same 11 nuclides.
- **Still not done:** a per-nuclide split. #533 did not require one.

## Results — FINAL (2026-09-29, routes 3/4/5 at `f78b5180d5`) — *superseded 2026-09-30, above*

- **Routes 1 and 2** are unchanged from the interim campaign.
- **Routes 3, 4 and 5** were re-run at commit `f78b5180d5`, 32 seeds each, at
  campaign settings. The lattice ran at 10000 × [250 + 400]. The binary
  snapshot is `target/five_route_keff/bin/f78b5180d5/`.
- The rows they replace (code `f1ac422b11`, binary `2d2a1787fa`) are archived
  in `data/per_seed_keff_pre_final_routes345.csv`.
  - That archive holds **Jemima's 96-seed routes 3–5**. At the final code
    Jemima is at 32 seeds, like every other case, so its interval is wider
    than the interim one.

| case | route | n | k_eff ± sem | seed sd [pcm] | Δ vs k=1 [pcm] | Δ vs route 1 [pcm] |
|---|---|---|---|---|---|---|
| godiva | route1 | 32 | 1.00016 ± 0.00021 | 118 | +16 ± 21 | reference |
| godiva | route2 | 32 | 1.00016 ± 0.00021 | 118 | +16 ± 21 | +0 ± 29 (+0.0σ) |
| godiva | route3 | 32 | 0.99992 ± 0.00036 | 206 | -8 ± 36 | -24 ± 42 (-0.6σ) |
| godiva | route4 | 32 | 1.00020 ± 0.00029 | 165 | +20 ± 29 | +4 ± 36 (+0.1σ) |
| godiva | route5 | 32 | 0.99992 ± 0.00036 | 206 | -8 ± 36 | -24 ± 42 (-0.6σ) |
| jemima | route1 | 32 | 0.99594 ± 0.00017 | 99 | -406 ± 17 | reference |
| jemima | route2 | 32 | 0.99594 ± 0.00017 | 99 | -406 ± 17 | +0 ± 25 (+0.0σ) |
| jemima | route3 | 32 | 0.99630 ± 0.00037 | 209 | -370 ± 37 | +36 ± 41 (+0.9σ) |
| jemima | route4 | 32 | 0.99609 ± 0.00033 | 187 | -391 ± 33 | +15 ± 37 (+0.4σ) |
| jemima | route5 | 32 | 0.99630 ± 0.00037 | 209 | -370 ± 37 | +36 ± 41 (+0.9σ) |
| hst009 | route1 | 32 | 1.00234 ± 0.00022 | 127 | +234 ± 22 | reference |
| hst009 | route2 | 32 | 1.00275 ± 0.00031 | 174 | +275 ± 31 | +41 ± 38 (+1.1σ) |
| hst009 | route3 | 32 | 1.00229 ± 0.00037 | 209 | +229 ± 37 | -5 ± 43 (-0.1σ) |
| hst009 | route4 | 32 | 1.00216 ± 0.00026 | 145 | +216 ± 26 | -17 ± 34 (-0.5σ) |
| hst009 | route5 | 32 | 1.00218 ± 0.00025 | 141 | +218 ± 25 | -16 ± 34 (-0.5σ) |
| lct008 | route1 | 32 | 1.00251 ± 0.00007 | 42 | +251 ± 7 | reference |
| lct008 | route2 | 32 | 1.00248 ± 0.00008 | 45 | +248 ± 8 | -3 ± 11 (-0.3σ) |
| lct008 | route3 | 32 | 1.00214 ± 0.00012 | 71 | +214 ± 12 | -36 ± 15 (-2.5σ) |
| lct008 | route4 | 32 | 1.00272 ± 0.00011 | 63 | +272 ± 11 | +21 ± 13 (+1.6σ) |
| lct008 | route5 | 32 | 1.00230 ± 0.00012 | 69 | +230 ± 12 | -21 ± 14 (-1.5σ) |

### What the numbers say

**Every case × route is within 2σ of route 1, except the lattice on route 3:
−36 ± 15 pcm (2.5σ).**
- Before the final code the lattice routes 3/4/5 stood at −73 ± 15,
  −50 ± 13 and −69 ± 15.
- **Declared extension:** lattice routes 3 and 1 are being extended to 96
  seeds (#407) before anything is concluded from the 2.5σ.

**HST-009 (#367) is closed.**
- The shared −232 to −302 pcm of the interim campaign is now −5 ± 43
  (route 3), −17 ± 34 (route 4) and −16 ± 34 (route 5).
- The delayed-neutron spectra carry most of it: their paired A/B was
  +174 ± 48 on route 3 and +154 ± 42 on route 4, against a +50 to +200
  prediction.
- Transporting the other neutron-emitting reactions added +33 ± 42.
- The exclusions recorded earlier stand, each as an A/B: the S(α,β) data,
  the free-gas S(α,β) test (−247 ± 37), and F-19 (−272 ± 38).

**Jemima.**
- The interim +106 to +124 pcm is gone: now +36 ± 41, +15 ± 37 and
  +36 ± 41.
- The 96-seed interim rows (archived) measured the delayed-spectra code
  alone. The final code adds the URR band-per-energy port, which is
  +38 ± 20 on Jemima at 128 seeds.

**The code changes between the interim and final campaigns**, each with a
paired A/B and a prediction posted first (#365, #407). Records:
`../ace_route_physics/correlated_angle_2026-09-29.md`,
`../ace_route_physics/urr_dbrc_sab_as_openmc_2026-09-29.md`.
- **Law-61 and law-44 angular sampling, as OpenMC.** Godiva −10.5 ± 9.4
  (512 seeds); lattice −13.1 ± 9.4 (96 seeds).
- **URR, one band per nuclide and energy.** Lattice +32 ± 18; Jemima
  +38 ± 20.
- **DBRC gate and window.** Lattice +7 ± 18.
- **S(α,β) equiprobable sampling, as OpenMC.** This one replaced #188's
  scheme, which was never a maintainer decision. Lattice +36 ± 16 on route 3,
  +42 ± 14 on route 4.

**#407 (lattice residual).** URR-worth cross-check on the lattice, 32 seeds
per arm: outram-mc −4 ± 21 pcm, OpenMC +41 ± 11 pcm, difference −45 ± 24
(1.9σ). It is not resolved. A 96-seed follow-up is declared. A DBRC-worth
cross-check is running.

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
  model `lct008_keff.rs` runs by default (35 nuclides in case 1). The dropped
  24 are worth ~~**−65 ± 21 pcm**, measured 2026-10-05 with 16 full-tier
  seeds~~ **−75 ± 14 pcm**, measured 2026-10-05 with 32 full-tier seeds
  (corrected the same day from the 16-seed figure; see "Later measurements";
  #533).

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
