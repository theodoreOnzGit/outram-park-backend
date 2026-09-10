# SAMM (`LRF=7`) cross sections and resonance-parameter derivatives against NJOY2016 — ENDF/B-VII.1 Cl-35

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

**Generated:** 2026-09-11T00:30:00Z
**Crate version / commit:** `outram-park-backend` `develop` at `f7276573` plus the commit this file is part of (SAMM derivatives, `rpxsamm`, RECONR extra channels, `covadd`)

This record documents the first oracle comparison of the crate's
R-matrix-limited (`LRF=7`, `KRM=3`) resonance kernel — `samm.f90`'s port,
translated in July 2026 and never run against a real evaluation until
now — and of its resonance-parameter derivatives through ERRORR's MF=32
`rpxsamm` path. Both were blocked for the whole session on the absence of
an `LRF=7` evaluation: the committed ENDF/B-VIII.0 O-16 and F-19 tapes
carry no resonance parameters, and the public data hosts refuse this
environment. The NJOY2016 upstream repository's own test suite ships one
(`tests/resources/cl35rml`, reachable through `raw.githubusercontent.com`),
and that is the evaluation used here.

## 1. Methodology

**Oracle.** The upstream Fortran NJOY2016 at commit
`ac5adf5f33d893e42f2eed7fb286b0d51c7580da` (2016.79), gfortran 13.3.0,
built 2026-09-10, run on the committed decks. Two oracle products:

1. `reference-data/reconr/cl35-ENDF7.1-0K-err0.01.pendf` — `tape22` of
   NJOY's own `tests/20/input` deck (`errorr` 999 → `reconr 21 22 / 1725
   0 0 / 0.01 /`). RECONR writes the *exact* 0 K cross section at every
   node of its grid, so the tape's nodes are exact evaluations of the
   R-matrix regardless of the deck's `err`.
2. `reference-data/errorr/cl35-ENDF7.1-293.6K-ign4-iwt2-rel.errorr` and
   its PENDF — `reconr (0.001) → broadr (293.6 K) → errorr 20 22 0 23 0
   0 / 1725 4 2 1 1 / 1 293.6 / 0 33 1 1 -1 2e6 0 /` on the `999` output
   tape (the evaluation has no MF=33; the dummy `MT=1/2/102/600` sections
   are NJOY's own). `ign=4` (27 groups), `iwt=2`, `irelco=1`.

**Evaluation.** ENDF/B-VII.1 Cl-35, MAT 1725 (ORNL/LANL; Sayer, Guber,
Leal, Larson, Young), `LRU=1/LRF=7`, `KRM=3` (Reich-Moore-limited),
`IFG=0`, `NRO=NAPS=0`, 1e-5 eV – 1.2 MeV; three particle pairs (γ, n, p
with `Q = 615.22 keV`, `MT=102/2/600`); 8 spin groups with 1–3 explicit
channels (the eliminated capture channel listed first in every group);
MF=32 `LCOMP=2` with 1088 parameter uncertainties and 3093 `NDIGIT=2`
INTG correlation lines.

**Comparisons and predictions, stated before the first run.**

- *Kernel at NJOY's nodes* (`tests/reconr_cl35_rml_njoy_golden.rs`,
  tier 1): `samm::setup` + `samm::xsformula::cssammy` at each of the
  10 417 NJOY nodes below 1.2 MeV, plus the ENDF MF=3 background
  interpolated with the file's own law. Prediction: the tape's 7-figure
  printing floor (5e-7 relative) for elastic, whose MF=3 background is
  identically zero; for capture and (n,p) the same floor plus
  `1.5 × err × background` (the oracle linearised their `1/v` log-log
  backgrounds to `err = 0.01`).
- *Crate RECONR end to end* (tier 2): `reconr` at `err = 0.001`,
  interpolated at 13 sampled energies. Prediction: ≤ 3e-3 (two
  independently thinned grids), reported.
- *Derivatives through ERRORR MF=32* (`tests/errorr_mf32_cl35_rml_golden.rs`,
  tier 1, NJOY's PENDF in): every element of every output block within
  6e-6 (the tape's printing precision); the four `MT=1` blocks zero on
  both sides because a directly-evaluated dummy total gets no sensitivity
  through `akxy`. Tier 2 (crate RECONR + BROADR in): diagonal relative
  blocks unchanged (the `cflx·csig` normalisation of the sensitivities
  cancels exactly against `covout`'s division), cross blocks within ~1e-3.

**Pass criteria.** Tier-1 kernel: deviation ≤ 1.0 × allowance at every
node, each channel. Tier-1 ERRORR: ≤ 6e-6 relative on every element,
absolute floor 1e-12. Tier 2: reported, asserted at 3e-3 (RECONR
elastic/capture) and 1e-2 (ERRORR).

## 2. Reference

```bibtex
@techreport{MacFarlane2017NJOY2016,
  author      = {MacFarlane, R. E. and Muir, D. W. and Boicourt, R. M. and Kahler, A. C. and Conlin, J. L.},
  title       = {The {NJOY} Nuclear Data Processing System, Version 2016},
  institution = {Los Alamos National Laboratory},
  number      = {LA-UR-17-20093},
  year        = {2017},
  note        = {Source: \url{https://github.com/njoy/NJOY2016}, commit ac5adf5 (2016.79); \texttt{samm.f90} (SAMMY method, N. M. Larson, ORNL), \texttt{errorr.f90} \texttt{rpxsamm}, \texttt{reconr.f90}; test resource \texttt{tests/resources/cl35rml}}
}
@article{Chadwick2011ENDFB71,
  author  = {Chadwick, M. B. and others},
  title   = {{ENDF/B-VII.1} Nuclear Data for Science and Technology: Cross Sections, Covariances, Fission Product Yields and Decay Data},
  journal = {Nuclear Data Sheets},
  volume  = {112},
  number  = {12},
  pages   = {2887--2996},
  year    = {2011},
  doi     = {10.1016/j.nds.2011.11.002},
  note    = {Cl-35 (MAT 1725), R-matrix-limited resonance evaluation by R. O. Sayer, K. H. Guber, L. C. Leal, N. M. Larson (ORNL) with LANL fast-range data; MF=32 LCOMP=2}
}
@techreport{Larson2008SAMMY,
  author      = {Larson, N. M.},
  title       = {Updated Users' Guide for {SAMMY}: Multilevel {R}-Matrix Fits to Neutron Data Using {Bayes'} Equations},
  institution = {Oak Ridge National Laboratory},
  number      = {ORNL/TM-9179/R8},
  year        = {2008},
  note        = {The derivative formulation \texttt{samm.f90} implements (u-parameters, $\partial R/\partial u$, $\partial\sigma/\partial R$)}
}
```

## 3. Results — the R-matrix kernel at NJOY's nodes (2026-09-11, first run)

10 417 nodes below 1.2 MeV (NJOY's unionised grid has 10 730 per section;
those at or above the range's `eh` are excluded).

| Channel | Worst deviation / allowance | At E [eV] | Crate [b] | NJOY [b] | Relative |
|---|---|---|---|---|---|
| elastic (MT=2) | 0.82 | 6.71370e4 | 1.0114335 | 1.011433 | **4.90e-7** |
| capture (MT=102) | 0.74 | 1.12132e6 | 1.1004545e-3 | 1.100454e-3 | **4.48e-7** |
| (n,p) (MT=600) | 0.81 | 9.930686e4 | 1.0042245e-4 | 1.004224e-4 | **4.92e-7** |

Every channel sits at the printing floor at every node — `pgh`, `pghcou`
(the proton channel is charged, above its 615 keV threshold), the
closed-form 1/2/3-channel inverters, `setxqx` and `sectio` are all
exercised. The general 4+-channel solver (`yfour`) is **not** (no group
has more than three explicit channels), and no group lists the
eliminated channel other than first, so the `op-cjw.3` reorder is not
tested here either — both recorded as open.

The total needs a caveat that was found on the first run: the oracle's
`MT=1` is *not* the evaluation's MF=3 `MT=1` background plus the
resonance total. RECONR "reconstructs redundant reactions to be the sum
of their parts" (`reconr.f90:73`), and the check
`MT1 − (MT2 + MT102 + MT600) = MT107` holds to 4e-8 at 1.15 MeV, where the
evaluation's own `MT=1` TAB1 is `1e-20` but `MT=600`'s background is
7e-3 b. The kernel's own total is asserted equal to the sum of its three
channels (exact); the total against NJOY is reported (2.7e-3 at 1.15
MeV, entirely the `MT=600`/`MT=107` backgrounds).

Tier 2, the crate's RECONR at `err = 0.001` (27 069 points against NJOY's
10 730 at `err = 0.01`), at 13 sampled energies:

| MT | Worst | At E [eV] |
|---|---|---|
| 1 | 3.64e-4 | 1.10e6 |
| 2 | 1.27e-4 | 6.00e5 |
| 102 | 1.14e-3 | 1.00e5 |
| 600 | 4.04e-4 | 3.00e5 |

**Defect found by this tier** (the first run had `MT=600` with 80 points,
worst 1.0): the crate's `reconr::add_rml_range` handed only
total/elastic/fission/capture to `rebuild_range`, dropping every extra
`LRF=7` particle-pair channel. Upstream `emerge` adds the resonance term
to any MF=3 section whose `MT` is in `mmtres(3..)` (with `103..107`
remapped to `600..800`, `reconr.f90:284-290, 4762-4767`) and includes
those channels in `resxs`'s convergence test (`nsig`, l.331-340). Both
are now ported (`RangeDelta::other`, `test_panel` over `3 + MAX_OTHER`
partials).

## 4. Results — derivatives through ERRORR MF=32 (`rpxsamm`)

Tier 1, NJOY's 293.6 K PENDF in; 13 714 panels, 131 147 derivative
evaluations, 739 resonance nodes:

| Block (MT, MT1) | NJOY non-zero elements | Worst relative |
|---|---|---|
| (1,1), (1,2), (1,102), (1,600) | 0 | — (zero on both sides) |
| (2,2) | 529 | 4.16e-7 |
| (2,102) | 529 | 4.41e-7 |
| (2,600) | 529 | 4.12e-7 |
| (102,102) | 529 | 4.38e-7 |
| (102,600) | 529 | 4.20e-7 |
| (600,600) | 529 | **4.83e-7** |

Group cross sections `σ_g`: worst 3.26e-7 (`MT=102`, group 14). Group
bounds identical (1e-13). Every element of the covariance tape is
reproduced to the printing floor: this pins `babb`, `abpart`'s
`upr`/`upi`/`pr`/`pii`, `setqri`, `settri`, `derres`, the
`4π/E`/`uuuu`/`duuu` normalisation, `cssammy`'s `sigd` slot mapping,
`rpxsamm`'s node list, adaptive panel walk and `egtwtf` weighting, the
INTG correlation decoding (`(2k±1)/(2·10^NDIGIT)` × `sdev_i sdev_j`), the
`cflx·csig/sigs` rescaling and the `crr` fold.

**Defect found by this tier** (the first run): the diagonal blocks were
already at 4e-7 but the three cross blocks were off by ~1.9 with sign
flips. `rescon`'s SAMMY branch adds `crr(ig,ig2,ix,ixp)` to Fortran
`cova(ig,ig2)` (`errorr.f90:8802-8806`), whereas every ERRORJ branch
stores into `cova(ig2,ig)` — and the crate's `cova` layout had been
derived from the latter, so the `crr` store was transposed. Symmetric
diagonal blocks cannot see this; the cross blocks did. Fixed and pinned
by a unit test.

Tier 2, crate RECONR + BROADR in (reported): diagonal blocks unchanged
(4.16e-7 / 4.38e-7 / 4.83e-7, as predicted — the normalisation cancels),
cross blocks (2,102) 1.66e-3, (2,600) 3.71e-3, (102,600) 3.20e-3; `σ_g`
worst 3.55e-3 (`MT=600`, group 11).

Cross-check with NJOY's shipped `tests/20` deck (which takes `σ_g` from
a GROUPR GENDF rather than the PENDF): not asserted; the MF=32 arithmetic
is the same and only `csig`/`cflx` differ.

## 5. What this does and does not establish

Established: the `LRF=7` R-matrix kernel and its analytic parameter
derivatives reproduce NJOY2016 to printing precision on a real
three-pair, charged-exit-channel evaluation, and ERRORR's `LRF=7` MF=32
path is a faithful port including its upstream idioms (recorded in
`src/errorr/resprx/sammy.rs`).

Not established / open:

1. `yfour` (4+ channels), `KRM≠3`, `IFG=1`, `KBK>0` background terms,
   an eliminated channel not listed first (`op-cjw.3`) — need another
   `LRF=7` evaluation; none is available here.
2. `LCOMP=1` for `LRF=7` (ported alongside, no oracle).
3. The angular-distribution routines of `samm.f90` are dead code in
   NJOY2016 as shipped (`Want_Angular_Dist = .false.` in both callers)
   and are deliberately not ported.
4. Cost: the crate's derivative assembly is ~8× slower per energy than
   NJOY's (20 s against ~3 s for this deck); a performance item, not a
   correctness one.
