# GROUPR, GAMINR, COVR and LEAPR against the NJOY2016 binary

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

**Generated:** 2026-09-10T21:45:00Z
**Crate version / commit:** `outram-park-backend` `develop` at `ced7a0c7` (ERRORR MF=32 and RECONR MLBW commits included; the LEAPR run-driver commit that follows this file is described in §4.4)

This record collects, in one place, what the committed NJOY2016 oracle runs
prove about the four Phase-5/6 modules bead `op-cjw.9` names — GROUPR,
GAMINR, COVR, LEAPR — and the modules they feed (ERRORR, DTFR, RESXSR,
MIXR). Every number below is quoted from a committed test or a
`reference-data/*/README.md`; nothing here was measured for this document
alone. Where a comparison has not been done, §6 says so.

## 1. Methodology

**Oracle.** The upstream Fortran NJOY2016 at commit
`ac5adf5f33d893e42f2eed7fb286b0d51c7580da` (version 2016.79), built with
gfortran 13.3.0 on 2026-09-10, run on the *same* input decks and tapes the
crate is given. Every oracle tape is committed under `reference-data/`
together with the exact deck (`*.njoy-input`), so any result can be
regenerated with `njoy < deck`.

**Tiers.** The pattern of `tests/groupr_u238_gendf_golden.rs`:

- *Tier 1 — engine isolation.* The crate's module is fed NJOY's own
  upstream product (NJOY's PENDF for GROUPR/ERRORR, NJOY's GENDF for DTFR,
  NJOY's ERRORR tape for COVR). Whatever then differs is the module under
  test alone. The expected agreement is the tape's own printing precision:
  ENDF `a11` fields carry 7 significant figures with a one-digit exponent
  (half-ulp 5e-7) and 6 with a two-digit exponent (5e-6).
- *Tier 2 — end to end.* The crate's own upstream stage (RECONR + BROADR,
  or ERRORR) feeds the module, so grid and reconstruction differences are
  included. Agreement is looser and is *reported*, with the cause of each
  larger deviation localised and stated.

**Predictions before measurement.** Every golden test states the expected
sign and size of the agreement in its doc comment before the numbers were
first produced; where a measured value contradicted the prediction the
disagreement was localised to a specific Fortran line before anything was
changed, and the test asserts the corrected understanding. A code path
with an implementation but no oracle test is treated as unverified.

**Pass criterion.** Tier-1 comparisons are asserted at the printing floor
(1e-5 relative, or 6e-6 where the tape mixes 6- and 7-figure fields);
tier-2 at the documented, cause-attributed tolerance.

## 2. Reference

```bibtex
@techreport{MacFarlane2017NJOY2016,
  author      = {MacFarlane, R. E. and Muir, D. W. and Boicourt, R. M. and Kahler, A. C. and Conlin, J. L.},
  title       = {The {NJOY} Nuclear Data Processing System, Version 2016},
  institution = {Los Alamos National Laboratory},
  number      = {LA-UR-17-20093},
  year        = {2017},
  note        = {Source: \url{https://github.com/njoy/NJOY2016}, commit ac5adf5 (2016.79). Sections GROUPR, GAMINR, COVR, LEAPR, ERRORR, DTFR, RESXSR, MIXR.}
}
@article{Brown2018ENDFB8,
  author  = {Brown, D. A. and others},
  title   = {{ENDF/B-VIII.0}: The 8th Major Release of the Nuclear Reaction Data Library},
  journal = {Nuclear Data Sheets},
  volume  = {148},
  pages   = {1--142},
  year    = {2018},
  doi     = {10.1016/j.nds.2018.02.001},
  note    = {Neutron and thermal-scattering sublibraries, tapes in reference-data/endf/README.md}
}
@misc{Koning2023TENDL,
  author = {Koning, A. J. and Rochman, D. and Sublet, J.-Ch. and others},
  title  = {{TENDL-2023}: {TALYS}-based evaluated nuclear data library},
  year   = {2023},
  note   = {Ar-37 (MAT 1828), the only committed evaluation with MF=32; reference-data/endf/README.md}
}
```

## 3. Results — GROUPR

Golden tapes: `reference-data/gendf/` (decks alongside). All 29-group
(`ign = 1` user grid), `iwt = 3`, 293.6 K, U-238 ENDF/B-VIII.0 MAT 9237
unless stated.

| Test | What is compared | Tier 1 (NJOY PENDF in) | Tier 2 (crate RECONR+BROADR) |
|---|---|---|---|
| `groupr_u238_gendf_golden.rs` | `sigma_g(sigma_0)` for MT=1/2/18/102 at 6 dilutions, group flux; `genflx` + `panel`; tier 3 URR via UNRESR MF=2/152; tiers 4–5 the `iwt = -3` flux calculator, homogeneous and heterogeneous | MT=1 6.85e-7, MT=2 4.55e-7, MT=18 2.07e-6, MT=102 **2.65e-6**; flux 4.93e-7 (7-figure floor) | MT=1/2/102 ≤ 9.2e-4; MT=18 1.1e-2 — NJOY's own `errint = err/20000` relaxation on a 2.5e-7 b sub-threshold fission (`reconr.f90:112-117`), the crate reconstructs tighter (961k vs 448k points); asserted 3e-2 |
| `groupr_u238_elastic_matrix_golden.rs` | MF=6/MT=2 transfer matrix, `lord = 0` and `lord = 3` (P0–P3), 6 dilutions | `lord=0`: 516 words identical structure, every element at the 7-figure floor; `lord=3`: 2064 words, P0/P1 and large P2/P3 at the floor, the rest ≤ 9.99e-6 (small P3 elements at 1 unit of `1e-7 sigma_g`), fluxes 6.89e-7 | reported in the test |
| `groupr_u238_inelastic_matrix_golden.rs` | discrete-level MF=3+MF=6 for MT=51/52/60/89, `q < 0` thresholds, 1 and 6 dilutions | every vector ≤ 3.2e-6, flux ≤ 1.5e-7 (1e-13 on the six-sigma-zero deck), every transfer element ≤ 1e-5 or < 1 unit of `1e-7 sigma_g`; P0 row sums to 3e-7 | — |
| `groupr_u238_derived_quantities_golden.rs` | MT=257/258/259 analytic `getsig` edits | asserted ≤ 1e-5 (env-gated: needs `OUTRAM_PARK_NJOY_U238_PENDF`) | — |
| `groupr_u235_mf10_golden.rs` | MF=10 residual production `4zzzaaam` (U-235 MT=4 ground/isomer), `nsigz = 1` | both subsections, 9 records each, every word ≤ **2.2e-6** | reported |

Three findings came out of the inelastic oracle (recorded in that test's
doc comment): `GroupFlux::analytic` stepped at 1.05 where `getwtf` uses
`s101 = 1.01`; `getfle`'s label-210 slide went stale on higher-order
coefficients; and a P3 element that disagreed by 8 % was shown, with an
independent converged quadrature, to be an upstream quirk (a stale `a3`
copied across a bracket), then ported deliberately.

## 4. Results — GAMINR, COVR, LEAPR

### 4.1 GAMINR (`tests/gaminr_synthetic_photoat_golden.rs`)

Oracle: `reference-data/gendf/photoat-synthetic-Z6-lanl12-iwt3-lord3.gendf`
— NJOY GAMINR on the **synthetic** photoatomic tape
`reference-data/endf/photoat-synthetic-Z6.endf` (generator script committed
beside it), LANL 12-group, `iwt = 3`, `lord = 3`, reactions
`23/501 502 504 516 522`, `26/502 504 516`, `23/525` (total heating).

| Section | records / words | worst relative deviation |
|---|---|---|
| MF=23/MT=501 | 12 / 24 | 5.58e-9 |
| MF=23/MT=502 | 12 / 24 | 2.52e-7 |
| MF=23/MT=504 | 12 / 24 | 1.65e-8 |
| MF=23/MT=516 | 9 / 18 | 2.21e-8 |
| MF=23/MT=522 | 12 / 24 | 4.35e-7 (`1.0837955e-8` vs `1.0837960e-8`) |
| MF=26/MT=502 | 12 / 96 | 3.51e-7 |
| MF=26/MT=504 | 12 / 320 | 4.10e-7 |
| MF=26/MT=516 | 9 / 18 | 2.21e-8 |
| MF=23/MT=525 (heating) | 12 / 24 | 1.59e-9 |

Worst overall **4.35e-7** — the 7-figure floor. **Negative result on
scope:** this is a *synthetic* evaluation. A real photoatomic tape
(`photoat-006_C_000` or similar) could not be obtained in this environment
— `www-nds.iaea.org` and `www.nndc.bnl.gov` both answer 403 — so the
photoatomic path is verified against NJOY's arithmetic on a tape with the
right *structure*, not against an evaluated library. That gap stands.

### 4.2 COVR (`tests/covr_boxer_golden.rs`)

Oracle: the 18 BOXER libraries of `reference-data/covr/` (nine ERRORR
tapes × `matype` 3 and 4). Tier 1 (NJOY's ERRORR tape in): the crate's
library is **byte-identical** to every one of the 18 files. Tier 2 (the
crate's ERRORR tape in, written by `ErrorrResult::to_tape`): byte-identical
for all nine materials — which is also what caught `covout` zeroing
sub-`eps` elements that upstream writes verbatim (`errorr.f90:7563-7568`).

### 4.3 LEAPR — kernels (`reference-data/leapr/`, three NJOY2016 runs on the crate's own embedded decks)

| Run | Exercises | Measured |
|---|---|---|
| `tsl-HinH2O-293.6K` (`leapr_h2o_njoy_oracle.rs`) | `contin` + `trans` + `discre`, free-gas O secondary (`b7 = 1`) | 44,961 `S(α,β)` points to **1.0e-13**; `T_eff` 1194.341 K exact; `B(1..12)` identical |
| `tsl-DinD2O-293.6K` (`leapr_d2o_skold_njoy_oracle.rs`) | the Sköld correction (`nsk = 2`) | 60,322 points to **1.0e-13**; `T_eff` 865.561 K exact |
| `tsl-SiO2-alpha`, 5 T (`leapr_sio2_mixed_moderator_oracle.rs`) | mixed moderator: second pass with `α/arat`, `S = S_Si + (sbs/sb) S_O`, two `T_eff` TAB1s | 43,449 points to **1.0e-13** at all five temperatures; both `T_eff` tables and the `contin` lambdas to every printed figure |

Against the *published* ENDF/B-VIII.0 graphite tape (a different LEAPR
build, `tests/leapr_graphite_deck_parity.rs`): 48,941 points at 296 K, max
4.917e-6 / RMS 6.4e-7 with the pre-2017 `k_B`, every point inside the
tape's own 6-/7-figure printing band; end to end through `endout` the
stored `S` values are identical at 60,000 / 60,000 points. The coherent
elastic channel is treated in `docs/leapr-sic-coherent-elastic-vv.md`.

### 4.4 LEAPR — the whole-deck driver (`tests/leapr_run_driver_njoy_oracle.rs`, 2026-09-10)

`leapr::run::run_deck` runs each of the three `.njoy-input` decks above
end to end (all temperatures, elastic channel as `endout` decides it, MF=1
header). Measured: MF=1/MT=451 **byte-identical** in columns 1–75 (60,
78 and 72 lines — HEAD/CONTs, the Hollerith comment cards, the dictionary
with NJOY's card-count estimates 24097 / 9677+5 / 53069); every MF=7 data
row identical (worst 1.3e-16 relative over 24,101 / 9,686+5 / 53,072
rows), which pins the five-temperature `LT = 4` layout and SiO2's
incoherent-elastic TAB1 (`LTHR = 2`, `SB = 2.16877`, written because
`iel = 0` with `twt = 0` flips to incoherent, `leapr.f90:3043`). The D2O
run exposed a card-reader defect (`/ end leapr` taken as a comment card),
fixed the same day.

## 5. Results — the consumers of these tapes

- **ERRORR MF=33** (`errorr_mf33_golden.rs`, nine ENDF/B-VIII.0 tapes):
  every group cross section and covariance element within 4.9e-7
  (7-figure fields) / 3.6e-6 (the one 6-figure value). **ERRORR MF=32**
  (`errorr_mf32_ar37_golden.rs`, TENDL-2023 Ar-37): all 465 blocks /
  14,048 elements within 4.97e-7; the listing's resolved/unresolved
  diagonals to 4.3e-4 (4 printed figures). NJOY2016 itself aborts on the
  unmodified Ar-37 tape (`errorr.f90:4363-4380` never advances past an
  empty L-block); the oracle ran on a physics-identical reorder whose
  RECONR+BROADR PENDF is byte-identical to the original's.
- **DTFR** (`dtfr_u238_claw_*_golden.rs`): the 32 × 29 CLAW table from
  the same GENDF, 115 non-zero entries, at the printing floor; full-channel
  assembly (ν σ_f, χ incl. constant-spectrum and delayed, P1 table, photon
  table) matches. Not yet done: a thermal `ntherm > 0` DTFR oracle.
- **RESXSR** (`resxsr_h2_njoy_golden.rs`): byte-identical, two temperatures.
- **MIXR** (`mixr_h2_be9_njoy_golden.rs`): every value identical after the
  tape's rounding at 787 × 3 points.

## 6. Defects the oracles found, and what remains open

Defects found by running the Fortran (none by reading the port), this
session: `GroupFlux::analytic` step (1.05 vs 1.01), `getfle` stale slide,
the P3 quirk (ported deliberately), `covout` sub-`eps` zeroing, BROADR's
missing free-gas 1/v grid points for light nuclides (`op-tubm`), RECONR
evaluating `LRF=2` with the SLBW elastic formula (Ar-37 elastic +6.3 % at
thermal, −8.2 % at 1 keV — `op-cral`, fixed and pinned by
`reconr_ar37_mlbw_njoy_golden.rs`), the LEAPR comment-card terminator, and
the upstream `rpxlc12` pointer defect.

Open, stated as such:

1. GAMINR on a **real** photoatomic evaluation — blocked by the data hosts'
   403; the synthetic tape verifies arithmetic, not an evaluation.
2. DTFR with `ntherm > 0` (thermal upscatter groups) — no oracle yet.
3. The U-238 tier-1 goldens that need NJOY's 40 MB PENDF are env-gated
   (`OUTRAM_PARK_NJOY_U238_PENDF`); the deck to regenerate it is committed
   with each GENDF golden.
4. LEAPR `coldh` (Young–Koppel rotational convolution) is self-consistency
   tested only; no NJOY run of a cold-hydrogen deck has been compared.
5. The Phase-6 formatters (WIMSR, MATXSR, CCCCR, POWR) have no port yet.
   Scoping found that a WIMSR oracle needs a GENDF with a thermal
   inelastic matrix (`mti`, e.g. free-gas MT=221 through THERMR) — NJOY's
   `xsecs` aborts with "mti missing from higher temps" otherwise; the
   committed U-238 GENDF tapes carry none, so a dedicated
   RECONR→BROADR→THERMR→GROUPR→WIMSR deck is required (see the `op-cjw.14`
   bead for the deck that runs).
