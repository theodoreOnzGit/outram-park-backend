# HTTR start-up core: explicit-TRISO Monte Carlo, uncalibrated

> ⚠️ **Research, education and V&V only.** Not for reactor operation,
> licensing or safety decisions (`RESPONSIBLE_USE.md`).
>
> **Review status:** AI-assisted first build (Claude Opus 5.5), 2026-10-16,
> **not yet reviewed by a human.**

GitHub epic #888 (data: #883-#887). Model: `crates/nee_soon/src/httr/`.

## What is computed, and against what

`k_eff` of the HTTR fully-loaded 30-column start-up core at 300 K, built from
the IRPhEP benchmark specification and run **uncalibrated**: no input was
chosen or adjusted by comparison with any reference value.

| reference | kind | value | source |
|---|---|---|---|
| benchmark `k_eff`, fully-loaded critical | **validation** (experiment, evaluated) | 1.0025 (−0.0060 / +0.0071) | R1 §3.1.5.1 |
| benchmark `k`, fully-loaded subcritical (rods at −55 mm) | **validation** (inverse kinetics) | 0.6876 ± 0.0104 | R1 §3.1.5.2 |
| MCNP5 on the same benchmark model, six libraries | **verification** (code to code) | critical 1.0178-1.0236, subcritical 0.6979-0.7036, ±0.0001 | R1 Tables 4.1-4.2 |

**R1** is Bess, Fujimoto, Dolphin, Snoj & Zukeran, *Evaluation of the
Start-Up Core Physics Tests at Japan's HTTR (Fully-Loaded Core)*,
HTTR-GCR-RESR-001, INL/EXT-08-14767 Rev. 2 (2010), doi:10.2172/974753,
<https://inldigitallibrary.inl.gov/sites/sti/sti/4460716.pdf> (US DOE report,
open download; accessed 2026-10-16). Its MCNP5 results are not ground truth:
they carry their own library and model terms, and R1 reports that every
Monte Carlo code overpredicts this core by 1.4-2.7 % (R1 §4.1), attributed
to graphite impurities, graphite and U-235 data, and gas or moisture in the
graphite pores. **None of the MCNP5 libraries is ENDF/B-VIII.0**, which this
model uses throughout, so the code-to-code comparison carries a library term
that is not separated here.

## Model

Every dimension and composition is R1's (§3.1, Tables 3.1-3.9) or its sample
MCNP5 deck's (App. A.1.8), each cited in `httr::spec` and
`httr::materials`. Explicit: 867 million TRISO particles (ordered lattice,
333 per 0.1 cm layer, 181 818 per rod; uranium 188.58 g per rod to 0.05 %),
4770 fuel rods, BP pins, 33- and 31-hole blocks, reflector channels, 16
control columns with 10-section rods at the measured critical heights, 2 mm
helium gaps, each column's rotation, and the PGX cylinder (r = 212.5 cm).
The compact annulus is **delta-tracked** against a majorant over the
compact's 18 materials; everything else is surface-tracked (as HTR-10).

Nuclear data: ENDF/B-VIII.0, 48 nuclides, processed at 300 K by this
workspace's NJOY port (RECONR, BROADR; tolerance 1e-3). Thermal laws:
graphite 30 % porous (tape), C-in-SiC and Si-in-SiC (tape), U-in-UO2,
O-in-UO2, Fe-56 and Al-27 metal (LEAPR from the ENDF/B-VIII.0 decks).

**Departures from R1** (each stated in the module docs): natural boron
(B-11 added) in every impurity; natural carbon; each carbon's own thermal
law (SiC, B4C, graphite); the 30 % porous graphite law (R1's libraries are
crystalline-type); and three places where R1's text and deck disagree,
where the deck is followed.

**Geometry checked by drawing it** (`examples/httr_geometry_images.rs`,
images written to `images/`, which is not committed: they are posted on
GitHub issue #888): core at each axial zone, the centre column's rods, a
rotated and an unrotated fuel column, a 31-hole block, a fuel rod and its
TRISO layer. What was checked in them: 61 columns in R1 Fig. 3.17's
arrangement; rods present in C, R1 and R2 columns at layer 1 and absent in
the R3 columns (withdrawn above it); B01 (rotated 180°) has its empty BP
hole on −x where B04 has it on +x; D07 has 31 holes; the rod bottom is at
+32.5 cm; the TRISO grid is 0.106 cm with the overcoat clipped by the tile
as in R1's deck. Not checked by eye: every column's rotation one by one (the
core map's rotations are pinned against the deck by the code that decodes
it).

**Construction tests** (`httr::tests`, no nuclear data, 9 pass): particle
count and uranium mass per rod; material totals against R1's tables; the
core map's rings and roles and 4770 rods; points located through the whole
geometry (kernel enrichment per zone and layer, rotated BP pins, the rod at
its critical height, the empty RSS hole); kernel volume fraction of a
compact sampled through the global geometry; no lost point in 50 000 random
probes.

## Prediction, recorded before any result

k_eff (critical, ENDF/B-VIII.0, 30 % porous graphite) in [1.005, 1.035],
central about 1.02. Basis: R1's MCNP5 ENDF/B-VII.0 1.0229; a VIII.0-vs-VII.0
library term of order ±1000 pcm (HTR-10 measured −1644 pcm on its model); the
30P-vs-crystalline law of order +500 to +700 pcm (HTR-10: +705 ± 86).
Crystalline ablation predicted to lower k by 400-900 pcm.

## Results

Taken 2026-10-16 at the uncommitted tree on top of `e9b5484608`, ENDF/B-VIII.0,
`examples/httr_keff.rs`, 20 000 histories x (100 inactive + 300 active), seed
20261016, one seed per case. Hardware: 13th Gen Intel Core i9-13900K, 16
cores (14 threads used), 62.5 GiB RAM, Linux; CPU only (an RTX A5000 present,
unused). Nuclear data about 104 s per run; compact majorant 185-206 s, single
threaded.

| case | k | vs benchmark | vs MCNP5 ENDF/B-VII.0 | transport |
|---|---|---|---|---|
| critical, 30P graphite (**default**) | **1.02400 ± 0.00049** | +2150 pcm (+3.03 σ_b) | +110 pcm | 3575 s |
| subcritical, 30P graphite (**default**) | **0.71135 ± 0.00046** | +2375 pcm (+2.28 σ_b) | +1145 pcm | 2779 s |
| critical, crystalline graphite (ablation) | 1.01716 ± 0.00052 | +1466 pcm (+2.06 σ_b) | −574 pcm | 3373 s |
| subcritical, crystalline graphite (ablation) | 0.70464 ± 0.00044 | +1704 pcm (+1.64 σ_b) | +474 pcm | 2761 s |

Against the other MCNP5 libraries (critical, 30P): +420 (V.2), +180 (VI.8),
+40 (JEFF-3.1), +620 (JENDL-3.3), +90 (MCNPX VII.0) pcm. The sigmas are each
run's own; with one seed they may understate the run-to-run scatter.

**Source convergence, measured.** Shannon entropy on the 9 x 9 x 5 fuel mesh
is flat from the first tenth of every run (critical about 7.68 bits,
subcritical about 7.61). The active halves differ by −37, +51, +145 and +50
pcm (critical 30P, subcritical 30P, critical crystalline, subcritical
crystalline), the largest about 1.4 of its sigma.

**Quoted numbers.** The 30P rows are the model's answer, because the 30P law
is the default chosen by density before any run. The crystalline rows are an
ablation. They are also the like-for-like comparison with R1's MCNP5, whose
libraries all carry crystalline-type graphite.

**Interpretation.**

- **Validation.** Every case overpredicts the benchmark, by 1.5-2.4 %. R1
  reports the same for every Monte Carlo code on this core (critical 1.4-2.7
  %, subcritical 1.5-2.3 %), attributed to impurities, graphite and U-235
  data, and gas or moisture in the pores. This model reproduces that known
  bias; it does not explain it.
- **Prediction.** k_crit = 1.02400 is inside the recorded [1.005, 1.035]
  with centre 1.02. The crystalline law lowered k by 684 ± 71 pcm, inside
  the predicted 400-900 pcm.
- **Verification, critical.** With like-for-like graphite (crystalline), k
  is 574 pcm below MCNP5 ENDF/B-VII.0. That is a VIII.0-vs-VII.0 library term
  plus any model difference, not separated here. Its sign matches HTR-10's
  measured VIII.0 shift (−1644 pcm on that model).
- **Rod worth is low: an open finding.** The all-rods-in worth
  ρ = 1/k_sub − 1/k_crit is:

  | | ρ (Δk/k) |
  |---|---|
  | 30P | 0.4292 ± 0.0010 |
  | crystalline | 0.4360 ± 0.0010 |
  | MCNP5 VII.0 | 0.4512 |
  | benchmark k's | 0.4568 |
  | measured scram (R1 Table 1.32) | 0.460-0.463 |

  Before the crystalline subcritical run, the graphite law was named as the
  hypothesis, predicting a worth of 0.430-0.445. The result was 0.4360, so
  the law accounts for about a third of the gap to MCNP5. The remaining
  3.4 % is unexplained. Already checked equal to R1's deck: the rod frame
  (universe 152: −55 mm is relative to the fuel bottom), the radii, the
  section lattice and the absorber composition (m23). R1 notes that rod
  worths are not fully benchmark-evaluated, so the measured scram worth is
  a comparison, not a benchmark. Follow-up: GitHub issue #919.

Raw logs (`target/httr_runs/*.log`) and run diagnostics (the gitignored
workspace-root `verification_and_validation/local_perf/`) are local, not
committed.
