# ERRORR MF=32 Reich-Moore (`LRF=3`, `ggrmat`) sensitivities and the `LCOMP=1` covariance format against NJOY2016 — JENDL-3.3 U-238

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

**Generated:** 2026-09-11T03:30:00Z
**Crate version / commit:** `outram-park-backend` `develop` at `b9d44326` plus the commit this file is part of (`errorr::resprx::rmatrix::ggrmat`, the `LRF=3` branch of `rpendf`)

## 1. Methodology

**What is computed.** ERRORR's MF=32 ERRORJ branch (`resprx` →
`rpxlc12` → `rpendf` → `ggrmat`) for a material whose resolved ranges are
Reich-Moore (`LRF=3`) with `LCOMP=1` general covariance LISTs, followed
by `rpxunr` for its unresolved range and the MF=33 chain, on the LANL
30-group structure (`ign=3`), `iwt=6`, 300 K, relative covariances
(`irelco=1`). Before this work the port refused `LRF=3` (`rpendf` had no
`ggrmat` branch) and its `LCOMP=1` reader had never been run against an
oracle.

**Oracle.** Upstream Fortran NJOY2016 at commit
`ac5adf5f33d893e42f2eed7fb286b0d51c7580da` (2016.79), gfortran 13.3.0,
built 2026-09-10, on NJOY2016's own `tests/16` deck
(`reference-data/errorr/u238-JENDL3.3-300K-ign3-iwt6-rel.njoy-input`):

```text
reconr 20 21 / 9237 0 / 0.001 /
broadr 20 21 22 / 9237 1 / 0.001 / 300. /
errorr 20 22 0 23 0 0 / 9237 3 6 1 1 / 1 300. / 0 33 1 1 -1 2e6 0 /
```

**Evaluation.** JENDL-3.3 U-238, MAT 9237 (JAEA; the NJOY2016 test-suite
`J33U238` resource, `reference-data/endf/n-092_U_238-JENDL3.3.endf`,
SHA-256 `e1a6fad0…`): MF=2 with ten `LRU=1/LRF=3` ranges of 1 keV from
1e-5 eV to 10 keV and an `LRU=2/LRF=2` (`LSSF=0`) range to 150 keV; MF=32
with one `LCOMP=1` short-range block per resolved range (`NSRS=1`,
`MPAR=3`: `ER`, `Γn`, `Γγ`; 26–37 resonances; the only fission widths on
file are `1e-10`-scale) and an `LRU=2` block; MF=33 for 37 reactions.

**Comparisons and predictions, stated before the first run.**

- *Tier 1* (NJOY's 300 K PENDF in, so only ERRORR's arithmetic differs;
  env-gated on `OUTRAM_PARK_NJOY_J33U238_PENDF`, 25 MB not committed):
  every group cross section and every element of every MF=33 block within
  the tape's printing precision, 6e-6 (5e-7 for one-digit exponents); the
  listing's `resolved`/`unresolve` diagonals for the seven resonance pairs
  within 6e-4 (4 printed figures).
- *Tier 2* (crate RECONR at 0.001 + BROADR 300 K in): the crate's RECONR
  does not reconstruct the infinitely-dilute averages of an `LSSF=0` URR
  (JENDL-3.3's MF=3 elastic is zero to 150 keV, NJOY's `sigunr` fills
  it), so the groups touching 9120 eV – 183 keV (13–18) were expected to
  differ grossly and are reported only; groups 1–12 asserted at 2e-2 on
  `σ_g` and 5e-2 on the covariances, listing rows at 2e-2.

**Pass criterion.** Tier 1: worst relative deviation < 6e-6 on the tape,
< 6e-4 on the listing. Tier 2 (groups 1–12): < 2e-2 / 5e-2 / 2e-2.

## 2. Reference

```bibtex
@techreport{njoy2016,
  author      = {MacFarlane, R. E. and Muir, D. W. and Boicourt, R. M. and Kahler, A. C. and Conlin, J. L.},
  title       = {The {NJOY} Nuclear Data Processing System, Version 2016},
  institution = {Los Alamos National Laboratory},
  number      = {LA-UR-17-20093},
  year        = {2016},
  note        = {\S ERRORR (ERRORJ resonance-parameter covariances); source \texttt{errorr.f90} \texttt{ggrmat} l.6227-6525, git ac5adf5}
}
@article{jendl33,
  author  = {Shibata, K. and Kawano, T. and Nakagawa, T. and others},
  title   = {Japanese Evaluated Nuclear Data Library Version 3 Revision-3: {JENDL-3.3}},
  journal = {Journal of Nuclear Science and Technology},
  volume  = {39},
  number  = {11},
  pages   = {1125--1136},
  year    = {2002},
  note    = {U-238, MAT 9237, as shipped in NJOY2016 \texttt{tests/resources/J33U238}}
}
```

The reference numbers are the committed oracle tape
`reference-data/errorr/u238-JENDL3.3-300K-ign3-iwt6-rel.errorr` (5958
lines) and the listing rows pinned in the test.

## 3. Results (2026-09-11, `tests/errorr_mf32_j33u238_lrf3_golden.rs`)

Command:

```text
OUTRAM_PARK_NJOY_J33U238_PENDF=<scratch>/j33/tape22 \
  cargo test --release -p njoy-outram-park-fork --test errorr_mf32_j33u238_lrf3_golden -- --nocapture
```

Output (abridged):

```text
[tier1 j33 u238 lrf3 lcomp1] message: resprx: mf2 nls=2, but mf32 nls=0 — continue with partial urr covariance data   (×10)
[tier1 j33 u238 lrf3 lcomp1] 666 blocks, 14069 non-zero elements | sigma_g 4.426e-7 at MT=74 ig=29 (got 1.033795458e-3, njoy 1.033795000e-3) | cov(resonance pairs) 4.565e-7 at MT=2 MT1=2 ig=18 igp=24 (got 1.019883466e-5, njoy 1.019883000e-5) | cov(other) 4.953e-7 at MT=2 MT1=67 ig=28 igp=26 (got -1.001782496e-5, njoy -1.001782000e-5)
[tier1 j33 u238 lrf3 lcomp1] listing diagonals worst 4.669e-4 at tt resolved ig=8 (got 1.054492348e-4, njoy 1.054000000e-4)
test tier1_env_gated_matches_njoy_tape23 ... ok
[tier2 j33 u238 (crate PENDF)] crate RECONR + BROADR took 79.4 s
[tier2 j33 u238 (crate PENDF) all groups, reported] ... sigma_g 1.000e0 at MT=2 ig=14 (got 0, njoy 1.243953180e1) ...
[tier2 j33 u238 (crate PENDF)] 666 blocks, 742 non-zero elements | sigma_g 4.093e-3 at MT=18 ig=4 (got 1.721511737e-6, njoy 1.714465000e-6) | cov(resonance pairs) 8.170e-3 at MT=18 MT1=18 ig=4 igp=4 (got 1.369559350e-4, njoy 1.380841000e-4) | cov(other) 0.000e0
[tier2 j33 u238 (crate PENDF)] listing diagonals worst 8.284e-3 at ff resolved ig=4 (got 1.369559350e-4, njoy 1.381000000e-4)
test tier2_crate_pendf_and_listing ... ok
test result: ok. 2 passed; 0 failed
```

| Tier | Quantity | Count | Worst relative deviation | Criterion | Met |
|---|---|---|---|---|---|
| 1 | group cross sections (37 reactions × 30 groups) | 1110 | 4.43e-7 (`MT=74`, g29) | 6e-6 | yes |
| 1 | covariances, resonance pairs | part of 14 069 | 4.57e-7 (`(2,2)` g18/g24) | 6e-6 | yes |
| 1 | covariances, all other blocks | part of 14 069 | 4.95e-7 (`(2,67)` g28/g26) | 6e-6 | yes |
| 1 | listing diagonals, 7 pairs × 15 rows | 105 | 4.67e-4 (`tt` resolved g8) | 6e-4 | yes |
| 2 | `σ_g`, groups 1–12, `σ_g ≥ 1e-6 b` | — | 4.09e-3 (`MT=18` g4) | 2e-2 | yes |
| 2 | covariances, resonance pairs, groups 1–12 | 742 non-zero | 8.17e-3 (`(18,18)` g4) | 5e-2 | yes |
| 2 | listing diagonals, rows 1–12 | — | 8.28e-3 (`ff` resolved g4) | 2e-2 | yes |

Every tier-1 element of every block is at the tape's printing floor: the
port's `ggrmat` (one `(L, J)` at a time, the `gf`-never-reset 3×3 path),
the `LCOMP=1` reader, ten consecutive `rpxlc12` ranges and `rpxunr` agree
with upstream to the last printed digit.

**Findings recorded on the way.**

1. *Listing quirk mirrored.* The cross-pair rows (`ef`, `eg`, `fg`) first
   disagreed while the diagonal ones matched: `errorr.f90:7673` indexes
   the *square* cross-term arrays with the packed-triangle index it uses
   for the diagonal ones, so the printed "resolved" value for a cross pair
   is not that pair's diagonal element. `ResonanceCovariance::diagonal`
   mirrors it (documented at the function). The output tape is unaffected.
2. *Tier 2, `LSSF=0` URR.* As predicted, the crate's PENDF has zero
   elastic in groups 13–18 (`σ_g(MT=2)` g14: crate 0, NJOY 12.44 b). A
   pointwise probe put the crate's RECONR within 1e-4 of NJOY's `tape21`
   at 3.5, 5, 7 and 9 keV (inside the resolved ranges) and at zero from
   10 to 150 keV. Bead `op-t0wt` (port `sigunr`). Group 13 (9120–24 800
   eV) is mostly URR, so the assertion stops at group 12.
3. *Tier 2, sub-threshold fission.* Not predicted: with the first cut at
   group 12 the assertion failed on `MT=18` group 11 — `σ_g` 9.225e-8 b
   (crate) against 8.623e-8 b (NJOY), 6.5 % apart, and the relative
   `(18,18)` element 3.823e-6 against 4.376e-6. Attribution before the
   fix: both RECONR/BROADR pipelines accept a panel once its
   resonance-integral error is under `errint = err/20000 = 5e-8 b`
   (`reconr.f90:2401`, `broadr.f90:209`; the crate mirrors both), so a
   cross section of ~1e-7 b is not resolved by either; if the absolute
   covariance agrees, the relative one should scale as `σ_g⁻²`:
   `(8.623/9.225)² = 0.8736` and `3.823/4.376 = 0.8736`. Confirmed. Tier 2
   therefore skips group cross sections under 1e-6 b and covariance
   elements (and listing rows) whose partial is under 1e-6 b in that
   group; tier 1 has no such floor. After the change the tier-2 worst
   moved to `MT=18` group 4 (1.72e-6 b, just above the floor) at 4.09e-3.

## 4. What this does and does not establish

- Established at the printing floor: `ggrmat` for a 1-channel R-function
  material with negligible fission widths (the 3×3 `efrobns` path *is*
  taken — `gf` is set by the `1e-10` widths — but its fission output is
  ~1e-7 b, so a wrong fission row would show up only in the `MT=18`
  blocks, which do match to 4.6e-7); the `LCOMP=1` reader with `NSRS=1`,
  `MPAR=3`; ten ranges in one material; `rpxunr` after them.
- Not established: `LCOMP=1` with `MPAR ≥ 4` (fission widths in the
  covariance), `NSRS > 1`, `NLRS > 0`, a material with real fission
  (Pu-239/Pu-240 ENDF/B-VIII.0 are `LRF=3 LCOMP=1` candidates — Pu-239
  with `ISR=1` — noted, not fetched); `LRF=3` with `NRO ≠ 0`; the crate's
  own PENDF above 9120 eV for this material (`op-t0wt`).
- The oracle is NJOY2016, so this verifies the translation; it says
  nothing about the physical adequacy of ERRORJ's central-difference
  sensitivities.
