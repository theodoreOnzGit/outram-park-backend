# ERRORR — multigroup covariance matrices

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §ERRORR); upstream Fortran: `errorr.f90` (~11.2k lines).

## Theory

Evaluators encode their uncertainty about nuclear data as **covariances** — the
joint (relative) covariance matrix of the evaluated quantities. ERRORR reads the
ENDF covariance files and collapses them to a user **multigroup** structure:

| ENDF MF | Covariance of |
|---|---|
| 31 | ν̄ (average fission neutrons) |
| 33 | cross sections |
| 34 | angular distributions |
| 35 | secondary energy spectra |
| 40 | production cross sections |

For cross sections it forms the group relative covariance
`rcov(g, g') = cov(σ_g, σ_{g'}) / (σ_g σ_{g'})` by projecting the ENDF
sub-material covariance components (NC/NI-type sub-subsections, each a pattern of
energy blocks) onto the group structure with the same flux weighting GROUPR uses.
The result feeds S/U analysis (e.g. sandwich-rule Δk/k propagation).

## How the port implements it

The MF=33 cross-section path is **ported end to end and validated against
the NJOY2016 binary** (`run_mf33`, `src/errorr/mf33.rs`). It is the
in-memory equivalent of the deck

```text
errorr / nendf npend 0 nout 0 0 / matd ign iwt iprint irelco / mprint tempin / 0 33 1 1 -1 2e6 0
```

| Fortran (`errorr.f90`) | Rust | What |
|---|---|---|
| `errorr` dictionary scan (l.713-792) | `gridd::scan_reactions` | which MF=33 `MT`s, lumps (851–870), MF=32 present? |
| `gridd` / `merge` / `lumpmt` (l.1091-1768) | `gridd::{gridd, merge, lumpmt}` | covariance energy grid `eni`, derivation coefficients `akxy(iy,ix,k)` over ranges `ek`, lumped components |
| `egngpn` + `uniong` (l.9534-9807) | `groups::neutron_group_structure` + `gridd::uniong` | user bounds `egn`, union grid `un` (`sigfig` to `ndig=6` throughout) |
| `egnwtf` / `egtwtf` / `egtflx` (l.9809-10286) | `weight::{ErrorrWeight, WeightSampler}` | the weight menu `iwt` 1–12 with its panel *stops* (`1.01e`, fusion-peak refinements, tabulated `terpa` steps) |
| `grpav` / `epanel` / `egtsig` + `endf.f90` `gety1` | `grpav::{grpav, PanelState, XsSampler}` | union-group `σ_g` and flux from the PENDF at infinite dilution (threshold shading `0.999999`, discontinuity step `0.999995`) |
| `covcal` / `lumpxs` (l.1770-2417) | `covcal::covcal` | absolute union-group covariances, `LB` 0–6 and 8 decoded with the Fortran's own index arithmetic |
| `sigc` / `covout` (l.7018-7898) | `covout::{sigc, covout, ErrorrResult::to_tape}` | coarse-group `csig`/`cflx`, the `akxy`-weighted collapse with the `isd`/`iabort` fast path, relative/absolute output, and the `nout` tape layout COVR reads |
| `resprx` (l.3011-3250) + `rescon` (l.8513-8819) | `resprx::{resprx, ResonanceCovariance::rescon}` | MF=32 driver (`irespr=1`, ERRORJ method): group window per range, `ISR`/`DAP` scattering-radius uncertainty, the `c**`/`u**` accumulators and their fold into each `(MT, MT1)` block |
| `rdumrd2` / `rskiprp` (l.5091-5225, 5369-5409) | `resprx::mf2::{Mf2Resonances, build_work_array}` | MF=2 scan (`nlspepi`, URR `amur` from an `LRF=2` URR), the resolved-range work array with `S_l`/`P_l` at each `ER` |
| `rpxlc2` / `rpxlc12` / `rpendf` (l.4108-4783, 5015-5089) | `resprx::resolved::{rpxlc2, rpxlc12, rpendf}` | `LCOMP=2` compact covariance (and `LCOMP=1`), central-difference MLBW sensitivities (`ER` ±0.01 %, widths ±1 %), the `eskip` pointwise grid |
| `ggmlbw` (l.6527-6650) | `resprx::mlbw::ggmlbw` | zero-temperature MLBW on the work array |
| `rpxgrp` (l.5227-5367) | `resprx::group::rpxgrp` | "simplistic" trapezoid group average with the `egtwtf` weight |
| `rpxunr` / `ggunr1` (l.4785-5013, 6800-6905) | `resprx::unresolved::{rpxunr, ggunr1}` | `LRU=2` one-sided 1 % sensitivities of the URR SLBW averages (`egnrl` fluctuation integrals) |
| `rpxsamm` (l.3252-3732) + `s2sammy`'s `mmtres` (l.796-808) | `resprx::sammy::{rpxsamm, mmtres_of}` | `LRF=7`: the `LCOMP=1`/`LCOMP=2` parameter covariance (INTG correlations read from the tape's raw text), `enode` from the resonance centres and half-heights, the adaptive panel integration of the analytic SAMM derivatives (`samm::derivs`) with the `egtwtf` weight, `cflx·csig/sigs` rescaling and the `crr(ig,ig2,mt,mt1)` fold that `rescon`'s SAMMY branch adds to every block |
| `covadd` (l.6907-6966) | `covadd::covadd` | the `nendf = 999` option: dummy `LB=5` MF=33 sections so a tape with MF=32 only has output blocks |

The MF=32 chain keeps several upstream idioms verbatim because the oracle
carries them: `gwidth = ER·1e-4` keeps the sign of a bound level; the
scattering-radius pass writes `AP+DAP` into each BW L-block's `QX` slot
and "restores" it as `(AP+DAP)/(1+DAP)`; `arat` is re-read from the MF=2
`AWRI` by every `ggmlbw` call; `rpxgrp`'s whole-group-skip branch divides
only its second term by `sumde`. One **deliberate divergence**: upstream's
resonance search (`rpxlc12`, l.4363-4380) never advances past an L-block
with zero resonances and NJOY2016 aborts on the unmodified TENDL-2023
Ar-37 tape; the port advances the pointer (see Testing).

Two upstream idioms were kept on purpose because the oracle depends on
them: every energy *and* every covariance datum is rounded to 6 significant
figures on input (`sigfig`), and ERRORR's `iwt=6` amplitude is
`wt6b = 1.57855e-3` — not GROUPR's `1.578551e-3` (a genuine
module-to-module difference in NJOY2016).

The earlier structural reader (`covariance.rs`, `read_covariance_section`)
is still available; `covcal` reads the records directly through
`SectionCursor` because it needs them staged in `covcal`'s own flat layout.

The SAMM branch keeps its own upstream idioms (see `resprx/sammy.rs`):
the panel search's `enext` is `egtwtf`'s next stop from the *previous*
panel, bounded by `(1+eps)·ee` and the next node; the midpoint test is
`eps = 0.01` relative plus `1e-7` absolute on every `sigp` slot; a range's
contribution above the shaded `eh` is zeroed; the total gets its
sensitivity only through `akxy`; and once MF=2 has an `LRF=7` range,
`rescon` uses `crr` for *every* block and ignores the `c**`/`u**`
accumulators (so an `LRU=2` range of the same material would be lost —
upstream, `errorr.f90:8528`). The port accumulates `crr` over ranges
where upstream would abort on a second `allocate`.

### Not ported (returns `NotPorted`, never approximates)

- **MF=32 branches outside the validated slice:** `LCOMP=0` (`rpxlc0`),
  `LRF=1` resolved sensitivities (upstream's `rpendf` has no SLBW branch and
  returns a stale `sig1`), `LRF=3` in the ERRORJ branch (`ggrmat`) and an
  `LRF=3` range in a material that also has an `LRF=7` one (upstream routes
  it through `rpxsamm`'s `rdsammy` mode 3), `NRO ≠ 0`, `NLRS > 0`, INTG
  correlation records in the ERRORJ branch (`rpxlc2`, `NM > 0`), `irespr =
  0` (`resprp`, the older method), an MF=2 URR that is not `LRF=2`
  (upstream reads uninitialised `amur`), `LRF=7` with background R-matrix
  terms (`KBK > 0`: `derext` and `samm::mf2` do not carry them), and
  `NIS > 1` with an `LRF=7` range (upstream: "multiple isotopes do not work
  with sammy method"). `LCOMP=1` is ported for both the MLBW and the
  `LRF=7` branch but has no oracle tape yet.
- **MF=31/34/35/40**, `iread = 1/2` (user reaction lists, extra `MAT1/MT1`
  pairs), `nstan` (ratio-to-standard, `LTY` 1–3, `grist`/`stand`), `nin`
  (input covariance tape merge), `ngout != 0` (`colaps` from a GENDF),
  `covadd`, ENDF/B-IV (`iverf = 4`), the `-33/333` speed-up modes.
- The listing printer (`nsyso` tables); `ErrorrResult` carries everything
  the listing prints.

## Testing

`tests/errorr_mf33_golden.rs` against `reference-data/errorr/` (nine
NJOY2016 runs, decks committed — see that README for provenance):

- **Tier 1 (engine isolation, NJOY's own PENDF in):** H-2 (`iwt=6/irelco=1`
  and `iwt=3/irelco=0`), Be-9, Li-6, C-12, F-19, Si-30, plus env-gated
  U-234 and U-238 (lumped `MT=851/852`, derived `MT=1`/`MT=4`, 135 union
  groups). Every group cross section and every covariance element of every
  block agrees to the tape's printing precision: worst 4.9e-7 relative for
  7-figure fields, 3.6e-6 for the one 6-figure (two-digit-exponent) value.
  Group counts, reaction lists and lumped placeholders match exactly.
- **Tier 2 (crate RECONR + BROADR in):** H-2 and Be-9. Covariances of
  directly-evaluated pairs within 3.3e-6 / 9.0e-4; the thermal-group
  `σ_g` is 32 % / 1.2 % high because the crate's BROADR does not insert grid
  points across the free-gas 1/v rise of a light nuclide (bead `op-tubm`,
  a BROADR defect the oracle exposed — reported, not asserted, here).
- The `to_tape` writer round-trips through the crate's ENDF reader with
  NJOY's layout (`output_tape_round_trips_like_njoy_layout`), and the tape
  it writes drives COVR to a BOXER library byte-identical to NJOY's for all
  nine materials (`tests/covr_boxer_golden.rs` tier 2). That test caught
  `covout` zeroing every output element below `eps = 1e-20`: upstream
  (`errorr.f90:7563-7568`) uses `eps` only to bracket a row's `ig2lo`/`ng2`
  and writes interior sub-`eps` elements verbatim, so the crate now does the
  same (2026-09-10).
- 46 unit tests (`merge`/`uniong`, `terpa`, `gety1` idioms, every `LB`
  kernel, the weight stops) in the modules' `#[cfg(test)]` blocks.

`tests/errorr_mf32_ar37_golden.rs` — the MF=32 chain on TENDL-2023 Ar-37
(MAT 1828; MLBW `LCOMP=2` with `DAP`, plus an `LRU=2` block; the only
committed evaluation with MF=32), 2026-09-10:

- NJOY2016 **aborts** on the unmodified tape (`rpxlc12 ... problem`,
  the empty-`L=1`-block pointer defect above), so the oracle deck ran on
  `n-018_Ar_37-tendl2023-mf2-L1-last.endf` (the empty block moved last in
  MF=2; NJOY's PENDF from the two tapes is byte-identical).
- **Tier 1 (NJOY PENDF in):** all 465 blocks / 14 048 non-zero elements
  agree with `tape23` to the printing precision — worst 4.97e-7 on the four
  resonance pairs `(1,1) (2,2) (2,102) (102,102)`, 4.97e-7 elsewhere;
  `σ_g` worst 3.4e-6. The unmodified tape through the port gives the same
  numbers (worst 4.97e-7 against the variant-tape oracle).
- **Listing diagonals:** the "resolved / unresolve" columns NJOY prints
  for the total, elastic and capture blocks (relative, 4 figures) match to
  4.3e-4; the unresolved contribution is non-zero only in group 12
  (3350–9120 eV, where the URR 4268–5548 eV sits): `4.479e-6` (tt),
  `7.788e-6` (ee), `1.563e-4` (gg).
- **Tier 2 (crate RECONR + BROADR in):** reported only. Its first run
  found the crate's Ar-37 **elastic** +6.3 % from 1e-5 eV to 100 eV and
  −8.2 % at 1 keV at 0 K while capture matched to 1e-7: the crate's RECONR
  evaluated `LRF=2` ranges with the SLBW elastic formula. Fixed the same
  day (`reconr::slbw::eval_mlbw_lstate`, bead `op-cral`,
  `tests/reconr_ar37_mlbw_njoy_golden.rs`); afterwards the crate PENDF is
  within 1.3e-4 of NJOY's at the sampled energies and the tier-2 `σ_g`
  worst is 3.7e-3 (`MT=3`, group 12), resonance-pair covariances worst
  2.3e-2 on a 6e-6 element of the `(1,1)` block.

`tests/errorr_mf32_cl35_rml_golden.rs` — the `LRF=7` SAMM path on
ENDF/B-VII.1 Cl-35 (MAT 1725; the NJOY2016 test-suite `cl35rml` resource,
`LCOMP=2` with 1088 parameters and 3093 INTG lines; dummy MF=33 via
`covadd`), 2026-09-11:

- **Tier 1 (NJOY PENDF in):** all 10 blocks / 3174 non-zero elements
  within **4.83e-7** of `tape23` — `(2,2)`, `(2,102)`, `(2,600)`,
  `(102,102)`, `(102,600)`, `(600,600)` all at the printing floor; the
  `MT=1` blocks identically zero on both sides. `σ_g` worst 3.3e-7. This
  is the first oracle for `samm::derivs` (`babb`, `abpart`'s derivative
  half, `setqri`, `settri`, `derres`, the u-parameter normalisation).
- The first run had the three cross blocks wrong by ~1.9 (signs flipped)
  with every diagonal block at 4e-7: the SAMMY branch of `rescon` stores
  `crr(ig,ig2)` into Fortran `cova(ig,ig2)`, which is the *transpose* of
  the crate's `cova` layout (`add_tri`/`add_sq` store Fortran
  `cova(ig2,ig)` at `[(ig-1)·ngn + ig2-1]`). Fixed and pinned by
  `rescon_sammy_branch_uses_crr_for_every_pair`.
- **Tier 2 (crate RECONR + BROADR in):** diagonal relative blocks
  unchanged (the normalisation cancels), cross blocks ≤ 3.7e-3, `σ_g` ≤
  3.6e-3 (`MT=600`, group 11) — reported. The first tier-2 run found the
  crate's RECONR dropping the R-matrix (n,p) channel from `MT=600` (80
  background points against NJOY's 10 730); `reconr::add_rml_range` now
  carries every extra `LRF=7` particle-pair channel as upstream `emerge`
  does (`tests/reconr_cl35_rml_njoy_golden.rs`).
- Cost: 13 714 panels / 131 147 derivative evaluations, 20 s in release
  against NJOY's ~3 s for the same deck — the per-energy derivative
  assembly allocates more than it should; recorded, not optimised further.

## Caveats

- **Not required by OpenMC CE** — Phase 5, sensitivity/uncertainty workflows only.
- Positive-semidefiniteness of ENDF covariances is not guaranteed by the data;
  the port reports what the file implies (as NJOY does) and never "fixes" it.
- Infinite dilution only in `grpav` (as upstream with `ngout = 0`).

## References

- NJOY2016 manual §ERRORR (LA-UR-17-20093)
- `errorr.f90` (NJOY2016 2016.79, `ac5adf5`)
- ENDF-102, Files 31–40 covariance formats
