# Pricing the fission source and free-gas target motion on Godiva and the FHR pebble

**Generated:** 2026-09-17, UTC.
**Crate version / commit:** `outram-mc-libs` 0.0.4, at `d716ab5` on
`claude/neutronics-runs-handoff-8xk972`.
**Class:** verification — internal paired-seed ablations of this crate against
itself, plus one code-to-code consistency check against a previously recorded
run. **Not validation**: no experiment is compared against here, and no human
V&V has been done. AI-assisted draft.

> **⚠ SUPERSEDED MODEL — added 2026-09-20.** Every number below was taken at
> `d716ab5`. The model has since moved twice, in ways that change the arms these
> ablations are differenced against:
>
> 1. **URR probability tables and DBRC became default-on** in
>    `Nuclide::from_endf_file` ("correct physics is the default", 2026-09-20).
>    At `d716ab5` *neither* was applied, so every BASE / MOVING arm here is a
>    model missing both terms.
> 2. **MT=5 was wired** into the reaction inventory.
>
> The **prediction verdicts** stated below stand as the record of what was
> predicted and what was measured at `d716ab5` — that is what a V&V record is
> for, and it is not rewritten after the fact. The **prices** (the pcm figures)
> are what may have moved, and none has been re-measured.
>
> The harness check at the end is the part most likely to read as a drift
> alarm and must not: `+1 ± 18 pcm` was checked against a pre-URR/DBRC
> expectation of `−22 pcm`. Under the new defaults that expectation is unknown,
> so a BASE arm landing somewhere else is not evidence that anything broke.
>
> Re-running both drivers on current `HEAD` is open work.

## Why this record exists

Three `Nuclide` ablation hooks landed on 2026-09-16 —
[`with_target_at_rest`] (`b774d32`), [`with_frozen_nubar`] and
[`with_frozen_fission_spectrum`] (`5b6df9b`) — closing gaps 3 and 4 of
`docs/neutronics-physics-coverage.md`. All three were verified *against the
tape* by `tests/ablation_hook_controls.rs`, which asserts that a switch
switches. **None had ever been run on a case**, so no mechanism had a price.
`docs/handoff-heavy-neutronics-runs.md` jobs 4 and 5 are those runs; this is
their record.

Each carries a **prediction made before the measurement**, and the deliverable
is an explicit statement of whether it held. Two of the four failed, which is a
result rather than a problem.

## Methodology

Common to every arm below:

- **Godiva**: ICSBEP HEU-MET-FAST-001, bare HEU sphere, `r = 8.7407` cm, vacuum
  boundary, 293.6 K, the three ICSBEP nuclide densities
  (U-234 `4.9184e-4`, U-235 `4.4994e-2`, U-238 `2.4984e-3` atoms/barn·cm),
  ENDF/B-VIII.0 reconstructed on device (RECONR + BROADR, tolerance `1.0e-3`).
  5000 histories × [40 inactive + 120 active], `CpuSingleThread`.
- **FHR pebble**: the ring-RPT CSG case of `examples/fhr_ring_rpt_endf.rs`,
  reproduced verbatim — 600 K, 14 nuclides, crystalline-graphite S(α,β) on the
  matrix/shell carbon, reflective root at `r = 3` cm. 4000 histories ×
  [30 inactive + 80 active], `CpuMultiThread`.
- **Paired seeds**: each pair of arms runs the same seed list `1..=N` and
  differs by exactly one call. Both the unpaired and the paired difference are
  computed and the program reports **which is tighter, measured rather than
  assumed** — the statistics in this study were got wrong twice before they
  were got right (gh:#196).
- **Machine**: 4 cores. `WORKERS` is read from `available_parallelism()`.

**Controls run before any transport**, on the actual nuclides and temperature
of each case, because a hook that silently removes nothing reports "no
difference" and that reads as "this physics does not matter" (bead `op-50vu`):

1. there was something to remove; 2. it is gone; 3. nothing else moved — and
for the fission source, that the two factors `ν̄` and `χ` stay separable, so
neither number absorbs the other's worth.

## Results

### Godiva, free-gas target motion — `with_target_at_rest`

128 seeds/arm. 141.2 s data + 1372.7 + 1355.6 s transport (46.2 min).
Control: kinematics `kT 2.530049e-2 -> 0.0` eV on all three nuclides.

| arm | n | mean Δk vs 1.0000 | sd | sem |
|---|---|---|---|---|
| MOVING (the physics) | 128 | **+1 pcm** | 200 | ±18 |
| AT-REST (ablated) | 128 | **+1 pcm** | 200 | ±18 |
| difference, unpaired | | +0 pcm | | ±25 (0.0σ) |
| **difference, paired** | | **+0 pcm** | **paired sd 5** | **±0 (1.0σ)** |

**Prediction: ~zero. HELD**, and bounded below **1 pcm at 3σ**.

The load-bearing number is the **paired sd of 5 pcm against either arm's 200**.
This is the only ablation in this study where pairing works — the continuum
angular law, ν̄ and χ all have paired sd *exceeding* their arms' (261, 247,
246 respectively). Five pcm is the transport-level confirmation of what the
control test shows at the kernel level: above `FREE_GAS_THRESHOLD·kT` =
10.12 eV the two kernels are bit-identical (2048/2048 draws, RNG in lockstep),
so only a negligible sub-threshold tail can differ.

**This is a harness check, not a physics result.** Its purpose is that the
switch demonstrably *fires* (the kT control) while the eigenvalue demonstrably
does not move, so a null on the FHR pebble could not be blamed on a dead
switch.

### FHR ring-RPT pebble, free-gas target motion — `with_target_at_rest`

*Run in progress at the time of writing; this section is filled in on
completion. Recorded prediction: **−2242 pcm**, the value the process-wide
environment-variable route measured on this same case in gh:#193's pricing
table.*

### Godiva, the fission source — `with_frozen_nubar` / `with_frozen_fission_spectrum`

128 seeds/arm, both frozen at **thermal, 0.0253 eV**. 150.6 s data +
1406.6 + 1372.9 + 1376.8 s transport (70.1 min).

| arm | n | mean Δk vs 1.0000 | sd | sem |
|---|---|---|---|---|
| BASE (the evaluation) | 128 | **+1 pcm** | 200 | ±18 |
| NU-FROZEN | 128 | **−6411 pcm** | 159 | ±14 |
| CHI-FROZEN | 128 | **−140 pcm** | 176 | ±16 |

| difference vs BASE | unpaired | paired | quote |
|---|---|---|---|
| NU-FROZEN | **−6412 ± 23 pcm** (284.1σ) | −6412 ± 23 (277.5σ), paired sd 261 | unpaired |
| CHI-FROZEN | **−141 ± 24 pcm** (6.0σ) | −141 ± 22 (6.5σ), paired sd 247 | unpaired |

Both paired sds exceed either arm's sd, so the unpaired figure is the one to
quote in both cases. Note this holds **even for ν̄**, which is documented as
preserving the RNG stream — stream preservation is not sufficient for pairing
here, because the histories diverge in where they go rather than in how many
variates they draw.

#### ν̄'s slope: prediction HELD

Predicted "large and negative — thousands of pcm", from the control test's
`nu_fission` falling **−8.16 %** at 2 MeV when ν̄ is frozen at thermal.
Measured **−6412 ± 23 pcm**, resolved at 284σ.

This is a *measurement*, not a bound, and it is **the largest single reactivity
lever this crate has measured anywhere** — larger than the FHR pebble's whole
inelastic channel (−4190 pcm). ν̄'s slope was the single largest untested lever
in the fast kernel; it is now priced.

#### χ's incident-energy dependence: prediction FAILED as stated

Predicted "very small — well under 100 pcm". Measured **−141 ± 24 pcm**, 6.0σ
from zero; even the 3σ lower bound on the magnitude (69 pcm) leaves the central
value outside the predicted range.

**The cause is the prediction's arithmetic basis, not the wiring.** The
prediction was calibrated on `+0.883 %`, which is the mean-birth-energy spread
between χ's **1e-5 eV and 14 MeV tape rows** — an *endpoint* pair, and it
brackets a bulge. Scanned through the sampler this run actually uses
(400 000 draws per point, printed by the driver's own controls):

| `E_in` | U-234 | U-235 | U-238 |
|---|---|---|---|
| 1.0e-5 eV | 1.93852e6 (+0.00 %) | 2.00032e6 (+0.00 %) | 1.93029e6 (+0.00 %) |
| 1.0e3 eV | 1.93855e6 (+0.00 %) | 2.00034e6 (+0.00 %) | 1.93032e6 (+0.00 %) |
| 1.0e5 eV | 1.94198e6 (+0.18 %) | 2.00233e6 (+0.10 %) | 1.93305e6 (+0.14 %) |
| 5.0e5 eV | 1.95581e6 (+0.89 %) | 2.01040e6 (+0.50 %) | 1.94409e6 (+0.72 %) |
| 1.0e6 eV | 1.97309e6 (+1.78 %) | 2.02530e6 (**+1.25 %**) | 1.95774e6 (+1.42 %) |
| 2.0e6 eV | 2.00176e6 (+3.26 %) | 2.05459e6 (**+2.71 %**) | 1.98458e6 (+2.81 %) |
| 4.0e6 eV | 2.04867e6 (+5.68 %) | 2.11121e6 (**+5.54 %**) | 2.03659e6 (+5.51 %) |
| 8.0e6 eV | 1.92145e6 (−0.88 %) | 2.03662e6 (+1.81 %) | 1.89978e6 (−1.58 %) |
| 1.4e7 eV | 2.02447e6 (+4.43 %) | 2.01807e6 (+0.89 %) | 2.08649e6 (+8.09 %) |
| 3.0e7 eV | 2.20918e6 (+13.96 %) | 2.30211e6 (+15.09 %) | 2.27238e6 (+17.72 %) |

χ is flat below ~1 keV, rises to **+1.25 % at 1 MeV and +5.54 % at 4 MeV**,
then falls back to +0.89 % by 14 MeV. Fissions in Godiva are induced at 1–4 MeV
— precisely the band the endpoint pair steps over. The relevant shift is
therefore several times the `+0.883 %` the prediction assumed, and −141 pcm is
the right order for it: the continuum angular law's ≤0.22 % mean-`E` shift is
worth ~38 pcm on the same case.

**The hand-off says to suspect the wiring before the physics when χ reads
large. That suspicion is not supported**, on four counts: (a) the control test
asserts the frozen arm is bit-identical to the unablated arm *at* the frozen
energy and that 14 MeV draws become bit-identical to thermal ones; (b) the
driver's own separability control asserts the χ freeze leaves ν̄ bit-identical
and vice versa; (c) the sampler reproduces each endpoint tape row's own
integrated mean within 1 %; (d) the measured magnitude is consistent with the
scanned shift at the energies that matter.

**The decisive run has not been done.** Freezing χ at Godiva's flux-average
incident energy instead of thermal removes the *shape* at fixed mean and should
return ≈0 if the hook is wired correctly, or another ~−141 pcm if it is not.
Until that is run, the honest statement is: *the prediction failed as stated,
the leading explanation is that it was calibrated on endpoint rows that bracket
a bulge, and the wiring hypothesis is disfavoured but not excluded.*

**The non-monotonicity at 8 MeV is not explained here** (U-234 and U-238 both
dip *below* their thermal value before rising again at 14 MeV). It is recorded
because it is visible, not because it is understood; no case in this workspace
has meaningful flux there.

## Harness check

The BASE / MOVING arms — which are the unablated crate — come in at
**+1 ± 18 pcm** on both Godiva programs independently. The hand-off's expected
post-`op-og56` value is near **−22 pcm** (from `+16 ± 11` pooled pre-ablation,
shifted by the continuum law's `−38 ± 23`). +1 ± 18 sits within 1σ of that, so
the instrument is where it should be and nothing else has drifted.

## What this does NOT say

- Nothing here is validated. No experiment is compared against.
- The ν̄ and χ numbers are **worths under a deliberately wrong model**, not
  error bars on this crate's physics. Freezing ν̄ at thermal is not a defect
  this crate has; it is a lever being measured.
- `with_frozen_nubar` is a documented **partial no-op** on the LOW (`Core`)
  tier above the WMP `e_max`, where ν̄ is baked into the group constant. Every
  Godiva case above runs the HIGH (`Pointwise`) tier, where it is complete.
