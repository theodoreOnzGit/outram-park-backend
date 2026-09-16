# The MF=6 LAW=1 continuum angular law: reading it, and what it is worth

**Date: 2026-09-16.** Tracks bead `op-og56`. **Class:** verification
(code-to-evaluation, plus an internal paired ablation). Not validation — no
comparison to an experiment is made here, and no human V&V.

## Summary

`njoy-outram-park-fork`'s ENDF MF=6 parser read the format's `NA` field only to
compute a row's stride, kept the outgoing-energy density `f₀`, and discarded the
angular coefficients `f₁ … f_NA`. `LANG` — the field that says what those
coefficients *mean* — was never read at all. The consequence was that every
continuum-inelastic (MT=91) and (n,2n) (MT=16) neutron in `outram-mc-libs` left
the collision **isotropically** in the frame the law names.

The coefficients are now retained, linearised into a samplable cosine law, and
sampled *correlated with the outgoing energy row actually drawn* — which is what
MF=6 LAW=1 is: a correlated energy-angle law, not two independent ones.

**This is the last of the three angular channels.** Elastic (MF=4/MT=2) was
always sampled; the discrete inelastic levels (MF=4/MT=51…90) were wired in by
`op-tm9f` and priced at **−198 pcm** on Godiva; the continuum is this one.

## Why the gap was invisible

Once the coefficients are dropped at parse time, **"this evaluation declares
isotropic emission" and "this port never read the angular data" produce
byte-identical results.** No test written against the parsed form can tell them
apart, because the parsed form no longer contains the difference. That is why
the defect survived a crate declared mature on a cross-code bar.

The fix is therefore as much a *type* change as a physics one.
`ContinuumAngular` now has four variants that were previously one number:

| variant | meaning | emission |
|---|---|---|
| `EvaluatedIsotropic` | the tape says `NA = 0` everywhere | isotropic — **correct** |
| `Legendre(…)` | `LANG = 1`, coefficients read and linearised | the evaluated law |
| `Unported(law)` | `LANG = 2` (Kalbach-Mann) — a real port gap | isotropic — **a known approximation** |
| `Ablated` | deliberately switched off for a measurement | isotropic — **an experiment** |

An ablated nuclide is self-describing, so an ablation that silently failed to
take effect is visible in the data rather than only in a suspiciously small Δk.

## Methodology

**Inputs.** ENDF/B-VIII.0 tapes in `reference-data/endf/`. Reconstruction
tolerance `1e-3`, 293.6 K.

**Reading the evaluations** —
`njoy-outram-park-fork/tests/mf6_continuum_angular_vs_endf.rs`. `LANG`, `NA` and
the per-row `a₁ = f₁/f₀` are read back and judged against the tape's own
declarations, in both directions: a nuclide the evaluation makes anisotropic
must report anisotropic, and one it makes isotropic must report isotropic.

**Sampling** — `outram-mc-libs/tests/continuum_angular_ablation_control.rs`.
The linearised cosine CDF is inverted 200 000 times with stratified variates and
the sample mean compared against the row's own `a₁`, which comes straight off the
tape and **never passes through the linearisation**. That makes it a real oracle
rather than a self-consistency check.

**Pricing** — `outram-mc-libs/examples/godiva_continuum_anisotropy_ablation.rs`,
two arms over the same seeds at 5000 histories × [40 inactive + 120 active], all
three ICSBEP HEU-MET-FAST-001 nuclides.

## Results — what the evaluations carry

| nuclide | MT | LANG | rows | rows with `NA>0` | peak `|⟨μ⟩|` |
|---|---|---|---|---|---|
| U-238 | 16 | Legendre | 2068 | 2066 | 0.4275 |
| U-238 | 91 | Legendre | 8654 | **8652** | **0.5573** |
| U-235 | 16 | Legendre | 2461 | 2459 | 0.4643 |
| U-235 | 91 | Legendre | 5296 | **5294** | **0.5751** |
| F-19 | 16 | Legendre | 1005 | 504 | 0.2574 |
| F-19 | 91 | Legendre | 175 | **0** | 0.0000 |
| Si-28 | 91 | Legendre | 205 | 205 | 0.2775 |
| O-16 | 91 | **Kalbach-Mann** | 1751 | 1751 | n/a |
| Al-27 | 91 | **Kalbach-Mann** | 642 | 642 | n/a |

`NA` reaches **26** on U-238's MT=91. That was measured *before* choosing an
implementation, and it ruled one out: a closed-form inversion of the `NA = 1`
linear density `(1 + 3a₁μ)/2` covers only 1231 of 8654 rows, so the Legendre
series has to be linearised in general.

F-19's MT=91 is the **negative control** — `NA = 0` on every incident energy, so
isotropic is the only correct answer there. Note its MT=16 is *not* isotropic,
which is why the control is scoped to one channel rather than to the nuclide.

## Results — the peak coefficient is the wrong number to reason from

This is the finding most likely to mislead the next reader, so it is stated
before the worth.

`peak |⟨μ⟩| = 0.557` is the largest `a₁` anywhere in U-238's MT=91 table. It is
**not** what a neutron experiences. Weighted by each row's own emission
probability `f₀`:

| `E_in` | pdf-weighted `⟨μ_cm⟩` | peak `|⟨μ⟩|` in that table |
|---|---|---|
| 0.4356 MeV (threshold) | **0.000000** | 0.000000 |
| 1.02 MeV | **0.000000** | 0.000000 |
| 1.34 MeV | −0.012053 | 0.022707 |
| 2.17 MeV | +0.000105 | 0.030456 |
| 3.00 MeV | +0.001684 | 0.035691 |
| 4.50 MeV | +0.009076 | 0.105401 |
| 8.50 MeV | +0.073219 | 0.190664 |
| 14.0 MeV | +0.272349 | 0.334664 |
| 20.0 MeV | +0.387079 | 0.451866 |

The 0.557 sits in the far tail of a high-energy table where `f₀` is negligible.
**This law has essentially no structure below ~4 MeV**, which is where a fission
spectrum keeps almost all of its flux.

An earlier draft of the ablation control reasoned from 0.557 and asserted the
two arms must differ at 2 MeV. They do not, and should not.

## Results — the sampler reproduces the tape

The most anisotropic row of U-238's 14 MeV table carries `a₁ = +0.334664`.
Drawing 200 000 stratified variates through its linearised cosine CDF gives
`⟨μ⟩ = +0.334445` — **agreement to 2.2e-4**. Since `a₁` never passes through the
linearisation, this checks the adaptive tabulation *and* the inversion.

## Results — the ablation controls, and controls only the angle

`⟨μ_lab⟩` over 4096 draws per arm, U-238 MT=91:

| `E_in` | evaluated | ablated | difference | RNG streams |
|---|---|---|---|---|
| 2 MeV | +0.00991 | +0.01010 | −0.00019 ± 0.01271 | **identical** |
| 14 MeV | **+0.25353** | −0.00426 | **+0.25779 ± 0.01238** (21 σ) | **identical** |

F-19 MT=91 at 8 MeV: **4096/4096** samples identical across the ablation in both
energy and angle.

The identical-RNG-stream property is what makes a paired Δk attributable: both
arms spend the same variates in the same order, one on a CDF inversion and one
on a linear map.

### A false invariant, recorded so it is not re-derived

The first draft of the control asserted the outgoing **lab** energy was
bit-identical across the ablation. It failed 4091/4096 — **correctly**. For a
centre-of-mass law, `E' = E_cm + E_trans + 2·μ_cm·√(E_cm·E_trans)`, so changing
the cosine changes the lab energy by construction. The invariant belongs on the
RNG stream, not on the energy.

## Results — the worth on Godiva

**Prediction, recorded before the measurement** (the workspace rule, and the
lesson of `op-mzvp.2.12`): **`k` down, by well under 50 pcm, plausibly under
20.** Direction: forward-peaked emission raises `⟨μ⟩`, lowers
`Σ_tr = Σ_t(1 − ⟨μ⟩)`, lengthens the transport mean free path and increases
leakage; Godiva is 55.8 % leakage. Magnitude: small, because the table above
shows this law is flat through the bulk of a fission spectrum. A result near
−200 pcm would mean the law is being applied at the wrong incident energy —
importing 14 MeV cosines into 2 MeV flux — and the wiring should be suspected
before the physics.

**Measured 2026-09-16**, 32 seeds per arm, 5000 histories × [40 inactive + 120
active], three ICSBEP nuclides, ENDF/B-VIII.0:

| arm | n | mean vs ICSBEP | sd | sem |
|---|---|---|---|---|
| **ANISO** (evaluated MF=6 LANG=1) | 32 | **+4 pcm** | 159 | ±28 |
| **ISO** (pre-`op-og56`) | 32 | **+45 pcm** | 182 | ±32 |
| **difference** | | **−41 pcm** | | **±43 (1.0 σ)** |

**The prediction held on direction and magnitude, and the result is a BOUND, not
a measurement.** `−41 ± 43 pcm` is 1.0 σ from zero, so this run cannot
distinguish the effect from nothing. What it does do is kill the alternative: an
effect the size of `op-tm9f`'s `−198 pcm` would sit **3.7 σ** from what was
observed. The continuum angular law is not a second `op-tm9f`.

**Do not quote −41 pcm as the worth of this law.** Quote it as *consistent with
zero, bounded well below 130 pcm at 3 σ, and negative in central value as
predicted*.

**A harness check that passes.** The ISO arm reproduces the behaviour this crate
had before `op-og56`, whose independently pooled value is `+16 ± 11 pcm` over
256 seeds. Measured here at `+45 ± 32` — a difference of `+29 ± 34 pcm`, 0.85 σ.
An ablation arm landing back on a number pooled by a different program is a
check on the instrument rather than a restatement of it.

**The seeds do not pair**, and the run reports it: paired `sd` 220 exceeds either
arm's 182, so the unpaired figure is the one quoted. Worth noting because unlike
the discrete-level ablation, these two arms *do* consume identical RNG variates
per collision, so pairing might have been expected to help. It does not — the
histories diverge in where they go, not in how many draws they take.

**What would resolve it.** `sigma_diff ≈ 14` is needed to put `−41` at 3 σ,
i.e. roughly **290 seeds per arm**, about nine times this run. Nothing in this
crate currently depends on the number being resolved rather than bounded.

## The tension this creates, stated rather than smoothed

For a CM law `⟨E'_lab⟩` **rises** with `⟨μ_cm⟩`, so this change **hardens** the
spectrum. This crate's flux is already 0.45 % too hard in mean `E` against
OpenMC (3.5 σ), with 1.03 % less flux below 300 keV — bead `op-os8x`, still
open. So a correct fix moves that residual the **wrong way**.

That is the same shape as gh:#192's MT=91 Q-value cap, which was correct and
moved Godiva 85 pcm *further* from a measured criticality experiment. Neither
was chosen for its direction.

## What is NOT covered

- **`LANG = 2` (Kalbach-Mann) is unported.** O-16 and Al-27 use it on both MT=16
  and MT=91 in ENDF/B-VIII.0, storing only `r` (`NA = 1`), so sampling it needs
  the Kalbach systematics for the slope `a`. Those laws report themselves as
  `ContinuumAngular::Unported` rather than passing as isotropic.
- **The `EnergyAngular` interpolation flag is still dropped.**
- **`LANG = 11…15`** (tabulated cosines) is retained but not sampled; no
  evaluation held here uses it on a neutron subsection.
- **No experiment.** Everything here compares this code against the evaluations
  it reads, or against itself with one mechanism removed.
- **One geometry, when the worth is measured.** A bare fast HEU metal sphere
  says nothing about thermal systems, where MT=91 is closed.

## Reproducing

```bash
cargo test --release -p njoy-outram-park-fork --test mf6_continuum_angular_vs_endf -- --nocapture
cargo test --release -p outram-mc-libs --test continuum_angular_ablation_control -- --nocapture

OUTRAM_GODIVA_SEEDS=32 cargo run --release -p outram-mc-libs \
    --features endf-pebble-cases --example godiva_continuum_anisotropy_ablation
```

A whole-run switch, `OUTRAM_MC_ISOTROPIC_CONTINUUM=1`, ablates the law in any
driver, matching the `OUTRAM_RINGRPT_*` family. Prefer
`Nuclide::with_isotropic_continuum_scattering` where both arms must exist in one
process over the same seeds, which an environment variable cannot give.
