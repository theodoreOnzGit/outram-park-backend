# GASPR gas production vs NJOY2016 — five light nuclides

**Date:** 2026-09-17
**Module:** `src/gaspr/` (ENDF MT=203–207, gas production)
**Test:** `tests/gaspr_vs_njoy2016.rs` (5 cases, 17 compared sections)
**Status:** verification (cross-code). **No human V&V.**

## Why this exists

Before this work `src/gaspr/` had **no test of any kind against an external
reference** — only unit tests of its own lookup table, which is the shape of
check that cannot find a missing term. Its module documentation asserted that
NJOY's residual-nucleus bookkeeping (`izr`/`izg` in `gaspr.f90`) was *"not
needed"* because a flat per-MT lookup encoded the same information. That claim
was wrong, and this comparison is what established it.

## Methodology

### Reference

NJOY2016 at commit `ac5adf5f` (`/home/user/njoy2016-src`), **built from source
and executed** for this comparison. Deck, per nuclide:

```
moder
20 -21 /
reconr
-21 -22 /
'<case> pendf for gaspr cross-check' /
<mat> 0 /
0.001 /
0 /
gaspr
-21 -22 -23 /
moder
-23 24 /
stop
```

Input evaluations are the committed ENDF/B-VIII.0 tapes in
`reference-data/endf/`. The resulting PENDF tapes, trimmed to MF=1 plus MF=3
MT=203–207, are committed as
`reference-data/gaspr/<case>-ENDF8.0-0K-err0.001.gaspr.pendf`, with the deck
that produced each one beside it, so the reference regenerates rather than
being trusted.

### Our side

`reconr` at the same `err = 0.001`, then
`GasProduction::from_reconr_and_tape(&recon, &endf, mat)`. Both sides therefore
start from the same evaluation and the same reconstruction tolerance; what is
compared is the gas *bookkeeping*, not the reconstruction.

### Comparison

The two codes tabulate on different grids by construction — NJOY writes gas
production on the MT=1 union grid above its gas threshold `thrg`, this port on
the union of the contributing sections' own grids — so both are lin-lin
interpolated at 600 log-spaced energies across the overlap and the worst
relative difference per MT is taken.

**Pass criterion: 1 % per MT.** Both sides interpolate linearly between
*different* node sets, so a curved σ(E) shows a chord difference bounded by
RECONR's own `errmax = 10·err = 1 %` convergence — the same reasoning
`tests/broadr_light_nuclide_pendf_golden.rs` records.

Two exclusions, both stated rather than tuned:

- Points below `1e-6` of the reference section's own peak, and points where
  both sides are below `1e-12` b. Immediately above a threshold each code
  interpolates along its own first chord, so the two disagree by 100 %
  relative while differing by ~1e-9 b absolute.
- The sweep starts 1 % above the overlap, because the threshold node itself
  carries σ = 0 on the reference side by ENDF convention.

The thresholds are **not** waived: each MT's threshold — defined as the last
zero node below the first non-zero one, which is how ENDF marks one — is
asserted separately to 1 %.

### The five cases

| case | MAT | what it isolates |
|---|---|---|
| Li-6 | 325 | MT=105 residual = ⁴He; `LR=32` breakup on 30 inelastic levels |
| Be-9 | 425 | the `izr = 4008` rule — ⁸Be counts as **two** alphas |
| B-10 | 525 | `LR=22/28/35` breakup; lumped MT=105 absent from the tape; ⁷Li residual as a negative control |
| H-2  | 128 | MT=102 capture producing ³H, from the residual alone |
| C-12 | 625 | `LR=23` (3α) breakup; MT=5 energy-dependent MF=6 yields |

Between them these exercise every residual rule that fires on a real
evaluation, the `LR` half of the ejectile table, and the MF=6/MT=5 path.

## Results

Worst relative difference per section, over the asserted range:

| case | MT=203 (p) | MT=204 (d) | MT=205 (t) | MT=207 (α) |
|---|---|---|---|---|
| Li-6 | 4.5e-5 | 3.7e-3 | **7.5e-4** | **7.5e-4** |
| Be-9 | 4.4e-5 | 2.1e-5 | 6.1e-5 | **5.5e-5** |
| B-10 | 4.3e-4 | 4.9e-3 | **6.9e-4** | **8.7e-4** |
| H-2  | 3.8e-3 | — | **8.3e-4** | — |
| C-12 | **3.1e-5** | **1.9e-7** | — | 1.4e-4 |

Bold marks a value that depends on the residual rule, the `LR` table, or the
MF=6/MT=5 yields — something a lookup keyed on MT alone cannot produce.

**Worst over all 17 sections: 4.9e-3** (B-10 MT=204, 48 keV above its 4.85 MeV
threshold, where the chord difference is largest). Typical: 1e-4 to 8e-4.
Neither code produces a section the other does not. Every threshold pair agrees
to better than 1 %.

Cross sections held in common by both codes agree to **every printed digit**
where the grids coincide — B-10 MT=103, 104, 107, 113, 600, 700, 800 and 801
were compared node-for-node during the diagnosis and were identical — so the
differences above are interpolation, not disagreement.

## Three defects found, and fixed

### 1. The residual nucleus was not credited at all

`gaspr.f90:821-826` adds the residual nuclide to the gas tally when it is
itself one of the five gases:

```
izr = 1001 → +1 p    izr = 2003 → +1 He-3
izr = 1002 → +1 d    izr = 2004 → +1 α
izr = 1003 → +1 t    izr = 4008 → +2 α   (⁸Be is particle-unbound)
```

with `izr = ZA_target + 1 − Σ ZA_ejectile`. This port had none of it, because
its yield table was keyed on MT alone and the residual depends on the
**target**. Consequences, each independently checkable against textbook
nuclear physics:

| reaction | was | is |
|---|---|---|
| ⁶Li(n,t)⁴He — tritium breeding | 1 t | 1 t **+ 1 α** |
| ³He(n,p)³H — the ³He detector reaction | 1 p | 1 p **+ 1 t** |
| ⁹Be(n,2n)⁸Be → 2α | nothing | **2 α** |
| ²H(n,γ)³H — tritium from heavy water | nothing | **1 t** |
| ¹⁰B(n,t)⁸Be | 1 t | 1 t **+ 2 α** |
| ¹⁰B(n,α)⁷Li | 1 α | 1 α (correctly unchanged) |

### 2. RECONR did not synthesise the lumped MT=103–107

`anlyzd` (`reconr.f90:566-590`) adds MT=103/104/105/106/107 to RECONR's
redundant-reaction list as soon as *any* section in the corresponding discrete
range (600–649, 650–699, 700–749, 750–799, 800–849) is present, and `emerge`
reconstructs it as the sum of its parts. An evaluation may carry the discrete
levels alone, and **B-10 does**: MT=700 with no MT=105.

Measured: NJOY's PENDF has MT=105 = 3.0068978570e-2 b at 7.6986e5 eV and
2.1783660360e-1 b at 5.5909e6 eV, both equal to its MT=700 to every printed
digit. This crate produced no MT=105 at all. The visible damage was in GASPR —
B-10 tritium 3.19e-5 b against 3.01e-2 b (**a factor of 940**) and alpha
0.568 b against 1.004 b — but the missing section is a RECONR output that ACER,
HEATR and transport all read, so the defect was never confined to gas
production.

Fixed in `reconr::synthesise_lumped_particle_channels`. Where the lumped
section **is** present it is left alone rather than recomputed, unlike upstream;
on B-10, which carries both MT=103 and MT=600–605, the evaluation's own MT=103
already equals NJOY's recomputed sum to every printed digit
(2.9376236660e-2 b at 5.5909e6 eV on both sides), so recomputing would be churn
with real regression risk and no measured gain.

### 3. MT=5's energy-dependent MF=6 yields were not read

MT=5 `(n,anything)` lumps final states the MT number cannot distinguish, so the
evaluation states the gas per event as an MF=6 multiplicity `y(E)` per product
`ZAP` (`gaspr.f90:100-240`, the `y = 111` sentinel at `:501-507`). C-12 carries
proton and deuteron multiplicities from 14.5 MeV to 150 MeV.

Measured before the fix: our proton and deuteron production above 20 MeV was
**identically zero** against NJOY's 3.48e-2 b and 1.31e-2 b at 20 MeV. After:
3.1e-5 and 1.9e-7 relative.

Fixed by `mf6::parse_mf6_product_yields` plus
`GasProduction::from_reconr_and_tape`. `GasProduction::from_reconr` still
exists and still omits this term — it is the right entry point only when the
evaluation has no MT=5 or the tape is not to hand, and its doc says so.

## What this does NOT establish

- **No validation.** This is agreement with another code on the same
  evaluation, not agreement with measurement. Nothing here says the ENDF/B-VIII.0
  gas-production data is right.
- **No human review.** Per `RESPONSIBLE_USE.md` this remains untrusted
  AI-assisted draft material.
- **Five light nuclides only.** Chosen because they are where the residual and
  `LR` rules actually fire; a heavy actinide exercises neither.
- **The legacy MT=600–849 GASPR fallback is still not ported** — NJOY's path
  for a pre-ENDF/B-VI tape that omits the lumped channels entirely. Such a tape
  would be under-counted here rather than mis-counted. Note this is distinct
  from defect 2 above, which was about RECONR's lumped *output*.
- **`LANG`/`JP` disambiguation in `parse_mf6_product_yields`** is not done: two
  subsections sharing a `ZAP` would have only the first read. No evaluation in
  `reference-data/endf/` does that on MT=5.
