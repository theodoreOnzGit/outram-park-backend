# RECONR LRF=7 with a background R-matrix (`KBK`) vs NJOY2016 — Sr-88

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

**Date:** 2026-09-14 · **Tracked:** gh:#202, `bn:op-hb9l` · **Class:** verification
(code-to-code against the code this crate is a translation of).

## Summary

The crate's LRF=7 (R-matrix limited) reconstruction was returning the
hard-sphere/potential term with no resonance contribution reaching the elastic
channel. The cause was a **discarded background R-matrix** (`KBK > 0`). With it
applied, Sr-88 reproduces NJOY2016 to within the reconstruction tolerance across
the whole resolved range, and reproduces the three reference values recorded in
the defect report at the 7-figure floor of NJOY's printed output.

## Methodology

**Quantity.** MF=3 MT=1 (total), MT=2 (elastic) and MT=102 (capture), pointwise
at 0 K, over Sr-88's resolved-resonance range `1e-5 .. 9.5e5 eV`.

**Inputs.** `reference-data/endf/n-038_Sr_088-ENDF8.1.endf`, ENDF/B-VIII.1,
MAT 3837 — the only LRF=7 evaluation held in this repository. Reconstruction
tolerance `err = 0.001` (and `0.0001` for the scaling check below), temperature
0 K.

**Reference.** NJOY2016 upstream `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`
(2016.79), built in-session with gfortran 13.3.0 (Ubuntu 13.3.0-6ubuntu2~24.04.1),
CMake 3.28.3, `CMAKE_BUILD_TYPE=Release`. Deck and output tape committed as
`reference-data/reconr/sr88-ENDF8.1-0K-err0.001.{njoy-input,pendf}`:

```
reconr
20 22
'sr88 lrf7 pendf'/
3837 0/
.001/
0/
stop
```

**Comparison.** On **NJOY's own grid** — all 44,326 points its PENDF carries
inside the resolved range — so the crate is judged where the reference chose to
place points, not where it placed its own. Duplicate energies (NJOY writes them
at MF=3 discontinuities) keep the first.

**Pass criterion.** Worst relative deviation ≤ 2e-2 per MT at `err = 0.001`,
plus the three gh:#202 values to < 1e-5 relative. Gate in
`tests/reconr_sr88_lrf7_kbk_njoy_golden.rs`.

## The defect

`reconr`'s LRF=7 path ran end to end — this was verified, not assumed:
`range.rml` was `Some`, `spin_groups = 7`, `samm::setup::setup` returned `Ok`,
and none of `add_rml_range`'s three silent no-op exits fired. **The issue's own
"where to look" note was wrong**: it inferred from `samm::run` returning
`NotPorted` that the reconstruction "is not wired". `samm::run` is vestigial and
nothing calls it; RECONR goes `reconr/mod.rs:424 → rml::add_rml_range →
samm::setup + cssammy`.

What was missing is the **background R-matrix**, `samm.f90:1199-1256` (read) and
`:3265-3295` (applied to the diagonal of the R-matrix before any resonance
contribution). The parser advanced its cursor past those records and threw the
data away, with a comment calling it "a secondary, rarer LRF=7 feature".

It is not secondary here. Every one of Sr-88's 7 spin groups carries `KBK = 1`
with `LCH = 2`, `LBK = 2` — a SAMMY-parametrised background on the **elastic**
channel — and all 443 of its resonances sit at or above 12.41 keV. At thermal
the resonance sum has nothing to say, so that term carries essentially the whole
elastic cross section beyond hard-sphere scattering.

That also explains the defect's signature, which the original report recorded
without drawing the inference: the error was **elastic-specific** (worst MT=2
1.04e1) while capture — the eliminated channel, whose thermal value is a
distant-resonance `1/v` tail that *was* being computed — was only mildly wrong
(1.38e0).

## Results

Worst relative deviation over all 44,326 NJOY grid points in the resolved range:

| MT | before | after, `err = 0.001` | after, `err = 0.0001` |
|---|---|---|---|
| 1 (total) | 1.03e1 | 9.80e-3 | 2.39e-3 |
| 2 (elastic) | 1.04e1 | **1.00e-2** | 9.97e-4 |
| 102 (capture) | 1.38e0 | 9.98e-3 | 1.00e-3 |

At the three energies gh:#202 recorded NJOY values for:

| E (eV) | NJOY | ours, before | ours, after | relative |
|---|---|---|---|---|
| 1.00000e-5 | 8.843210 | 4.969327 | 8.843210 | 5.63e-8 |
| 1.03125e-5 | 8.838604 | 4.969327 | 8.838613 | 9.79e-7 |
| 1.06250e-5 | 8.834137 | 4.969327 | 8.834151 | 1.62e-6 |

## Interpretation

**The residual is linearisation, not physics — measured, not assumed.**
Tightening `err` tenfold shrank the worst deviation by very close to tenfold on
MT=2 (1.00e-2 → 9.97e-4) and MT=102 (9.98e-3 → 1.00e-3). A physics error does
not scale with the reconstruction tolerance. Grid parity against NJOY is a
known separate matter (`bn:op-428f`).

**One divergence is left standing deliberately.** NJOY's MT=1 is the sum of all
partials (`reconr.f90:73`); this crate builds MT=1 as its own MF=3 background
plus the resonance total. At 1e-5 eV NJOY's total (9.047157 b) equals *this
crate's own* MT=2 + MT=102 (8.843210 + 0.203947) exactly, while the crate
reports 9.025528 b — short by 2.1629e-2 b, precisely the MF=3 MT=102 background
it omits from the total. This is why MT=1 is the one column above that does not
scale cleanly with `err`. Filed separately; not fixed here.

## What this does NOT establish

- **`LBK = 1` (tabulated) and `LBK = 3` (Fröhner) backgrounds are ported but
  unverified.** No held evaluation exercises either. They are written from
  `samm.f90:3269-3293` and covered by unit tests only.
- **Single-channel groups only.** All seven Sr-88 groups have one explicit
  channel; multi-channel LRF=7 with a background is untested.
- `KRM != 3`, `IFG = 1`, and an eliminated channel not appearing first remain
  untested — all open on `bn:op-cjw.2`, and a second LRF=7 tape is what would
  close them.
- This is **verification against NJOY2016, not validation against measurement.**
  Agreement here means the port reproduces the code it is a translation of; it
  says nothing about whether the Sr-88 evaluation matches experiment.

## References

- NJOY2016, `src/samm.f90` (`rdsammy` `:1199-1256`, `setr` `:3265-3295`,
  `crosss` `:3011-3229`, `sectio` `:6286-6440`) and `src/reconr.f90`, upstream
  `ac5adf5f`. Modified BSD 3-Clause (LANL/DOE).
- ENDF-6 Formats Manual, MF=2/MT=151 LRF=7 `KBK`/`LBK` record definitions.
- ENDF/B-VIII.1 Sr-88 evaluation, MAT 3837 (open, publicly released library).
