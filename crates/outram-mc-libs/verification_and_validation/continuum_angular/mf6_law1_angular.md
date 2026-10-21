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
`ContinuumAngular` now has five variants that were previously one number:

| variant | meaning | emission |
|---|---|---|
| `EvaluatedIsotropic` | the tape says `NA = 0` everywhere | isotropic — **correct** |
| `Legendre(…)` | `LANG = 1`, coefficients read and linearised | the evaluated law |
| `KalbachMann(…)` | `LANG = 2`, `(r, a)` per row | the evaluated law |
| `Unported(law)` | a representation this port does not sample | isotropic — **a known approximation** |
| `Ablated` | deliberately switched off for a measurement | isotropic — **an experiment** |

An ablated nuclide is self-describing, so an ablation that silently failed to
take effect is visible in the data rather than only in a suspiciously small Δk.
No evaluation in `reference-data/endf/` currently reaches `Unported`.

## Results — Kalbach-Mann (`LANG = 2`)

O-16 and Al-27 use `LANG = 2` on both MT=16 and MT=91, with `NA = 1` throughout
— so they tabulate the pre-compound fraction `r` and leave the slope `a` to the
**Kalbach-86 systematics**. That systematics was already in this workspace:
`groupr::kinematics::bach`, a port of NJOY2016 `groupr.f90:8812-8932` written
for the GROUPR path. It is reused rather than reimplemented; two copies of one
systematics drift.

**The sampler is a closed-form, single-variate inverse.** The density

```text
f(mu) = a [cosh(a mu) + r sinh(a mu)] / (2 sinh a)
```

has `sinh(a mu) + r cosh(a mu) = sqrt(1 - r^2) sinh(a mu + phi)` with
`phi = atanh(r)`, so its cumulative inverts directly:

```text
mu = [ asinh( ((2 xi - 1) sinh a + r cosh a) / sqrt(1 - r^2) ) - phi ] / a
```

**One variate is a requirement, not an optimisation.** The usual implementation
splits on `r` and spends two draws — one to choose the `cosh` or `sinh` branch,
one to invert it. That would consume a different number of variates from the
isotropic fallback, so an ablation of this law would shift the random stream and
a measured Δk would mix physics with re-randomisation. The single-variate form
preserves the invariant the ablation control asserts.

**Verified against the density's own closed-form mean**,
`⟨μ⟩ = r·(coth a − 1/a)` — the Langevin function scaled by `r`. Over 25 `(r, a)`
combinations spanning `r ∈ [0, 0.95]` and `a ∈ [0.05, 8]`, 200 000 stratified
draws each, the sampled mean matches to better than `2e-3`. The two share no
code: one integrates the density analytically, the other inverts its cumulative.

**O-16 MT=91, pdf-weighted `⟨μ_cm⟩`** (measured 2026-09-16):

| `E_in` | `⟨μ_cm⟩` |
|---|---|
| 10.19 MeV (threshold) | 0.000000 |
| 12.5 MeV | +0.125488 |
| 19.0 MeV | +0.212117 |
| 25.0 MeV | +0.310159 |

**This changes nothing for the reactor cases in this workspace.** O-16's MT=91
threshold is ~10 MeV, far above where a fission spectrum has flux. The value of
closing it is that the *representation* gap is closed — an evaluation reaching
`Unported` now means something genuinely unhandled, rather than one of the two
laws everything actually uses.

One property worth pinning, and pinned: `a → 0` is the isotropic limit of the
Kalbach form and **`r = 0` is not**. With `r = 0` the density is
`a cosh(a mu)/(2 sinh a)`, which has zero mean but is peaked at *both* ends. A
predicate treating `r = 0` as isotropic would silently discard that structure.

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

**Measured 2026-09-16**, 128 seeds per arm, 5000 histories × [40 inactive + 120
active], three ICSBEP nuclides, ENDF/B-VIII.0:

| arm | n | mean vs ICSBEP | sd | sem |
|---|---|---|---|---|
| **ANISO** (evaluated MF=6 LANG=1) | 128 | **−26 pcm** | 197 | ±17 |
| **ISO** (pre-`op-og56`) | 128 | **+11 pcm** | 166 | ±15 |
| **difference** | | **−38 pcm** | | **±23 (1.6 σ)** |

**The prediction held on direction and magnitude, and the result is a BOUND,
not a measurement.** At 1.6 σ it is consistent with zero. What it kills is the
alternative: an effect the size of `op-tm9f`'s `−198 pcm` would sit **7 σ** from
what was observed. The continuum angular law is not a second `op-tm9f`.

**Do not quote −38 pcm as the worth of this law.** Quote it as *consistent with
zero, bounded below 70 pcm at 3 σ, central value negative as predicted*.

**This supersedes an earlier 32-seed run** (`−41 ± 43 pcm`) rather than
confirming it: that run used seeds 1…32 and this one 1…128, so the earlier
sample is a **subset**. The stability across them is reassuring; it is not a
second measurement, and pooling would double-count.

**A harness check that passes.** The ISO arm reproduces the pre-`op-og56`
behaviour, independently pooled at `+16 ± 11 pcm` over 256 seeds. Measured here
at `+11 ± 15` — a difference of `−5 ± 19 pcm`, **0.3 σ**. An ablation arm
landing on a number pooled by a different program checks the instrument rather
than restating it.

**Consequence for `RECORDED_PCM = 16.0`**, which was measured pre-`op-og56`:
the post-`op-og56` mean is lower by `38 ± 23 pcm`, i.e. near `−22`. It has
**not** been re-measured at 256 seeds and is **not** changed on a 1.6 σ shift.
The drift gate built on it is `4·√(σ_run² + 11²) ≈ 693 pcm` for a single run,
so nothing is currently mis-gated.

**The seeds do not pair** (paired `sd` 246 exceeds either arm's 197), so the
unpaired figure is quoted. Worth recording because these two arms consume
identical RNG variates per collision, unlike the discrete-level ablation, so
pairing might have been expected to help. It does not — the histories diverge
in where they go, not in how many draws they take.

**What would resolve it:** `σ_diff ≈ 13`, i.e. roughly **400 seeds per arm**.
Nothing here depends on the number being resolved rather than bounded.

### The 400-seed run: the statistics hit their target, the effect did not

**Measured 2026-09-20**, 400 seeds per arm, same case and same settings, on
4 cores (ANISO 3757.5 s, ISO 3699.5 s, nuclear data 111.4 s; 2.10 h total).
**Run verbatim:**

```
Δk from ICSBEP HEU-MET-FAST-001 = 1.0000, pcm
  arm       n     mean      sd     sem
  ANISO   400       -10     187       9
  ISO     400        +6     194      10

  difference (ANISO − ISO), unpaired = -16 ± 13 pcm  (1.2 sigma)
  difference (ANISO − ISO), paired   = -16 ± 13 pcm  (1.2 sigma), paired sd 259
  -> paired sd (259) EXCEEDS either arm's (194); the seeds do not pair, so quote the unpaired figure.
```

**`σ_diff` came in at exactly the predicted 13 pcm.** The sizing arithmetic in
the paragraph above was right. What it also assumed — that the central value
would stay near `−38` — was not: the estimate moved to **`−16`**, so the result
is **still a bound, at 1.2 σ**, not the ~3 σ measurement 400 seeds were chosen
to buy.

That distinction matters and is the reason this subsection exists. The plan did
not fail because the statistics missed; it failed because a bound that shrinks
when you add samples was never going to resolve on the schedule a fixed central
value implies. **Sizing a run from an unresolved central value assumes the thing
being measured.**

**The prediction HELD, now on its tighter clause too.** Direction: negative, as
predicted. Magnitude: `|−16|` is under 50 pcm and under the *"plausibly under
20"* half of the recorded prediction, which the 128-seed `−38` did not satisfy.
The alternative the 128-seed run killed stays killed: an `op-tm9f`-sized
`−198 pcm` now sits **14 σ** away.

**Quote it as:** *consistent with zero, bounded below 40 pcm at 3 σ, central
value negative as predicted* — replacing the `70 pcm` bound above.

**This supersedes the 128-seed row, and again is not a second measurement of
it.** Seeds `1…128` are a subset of `1…400`. The `−38 → −16` movement is one
estimate being refined, not two runs disagreeing, and pooling would
double-count.

**The seeds still do not pair, and now by how much:** from `sd_a = 187`,
`sd_b = 194`, `sd_d = 259`, the implied correlation is **ρ = 0.076**. The arms
are, for practical purposes, independent samples. This confirms at 400 seeds
what 128 suggested: identical RNG consumption per collision does not make two
arms pair when the physics sends the histories to different places.

**What resolving `−16` at 3 σ would actually cost:** `σ_diff ≤ 5.3` pcm, i.e.
**~2380 seeds per arm** — about **12.4 hours on 4 cores** at the measured
37.6 CPU-s per seed. Recorded so the decision is made on a number rather than
an impression. Nothing in this document depends on it.

### The ANISO arm is also the current Godiva residual

The ANISO arm is the unablated crate, so **`−10 ± 9 pcm` over 400 seeds is
Godiva's residual under the 2026-09-20 defaults** — the first measurement of it
with **URR probability tables and DBRC on** (they became default-on that day;
see the workspace `CLAUDE.md`). It sits well inside ICSBEP's own ±100 pcm band.

**Do not read the two arms against the pre-`op-og56` pooled `+16 ± 11 pcm`
the way the 128-seed run did.** That check was valid when the only difference
between the arms and that pooled figure was the continuum angular law. It is
now **confounded**: this run also carries URR and DBRC, which that pooled value
does not. The ISO arm's `+6 ± 10` happens to sit 0.7 σ from `+16 ± 11`, and
that agreement is *not* evidence the instrument is unchanged — it is a
comparison across two model differences at once, and nothing here separates
them. A clean re-check needs `+16 ± 11` re-pooled under the new defaults.

**Consequence for `RECORDED_PCM = 16.0`:** it was measured pre-`op-og56` *and*
pre-URR/DBRC, so the `−22` projection above is superseded by neither arm
cleanly. `−10 ± 9` is the honest current figure for the full model, but
changing the constant on it would fold two unseparated effects into one number;
that is a maintainer decision, not something to do from this run.

## Results — the spectral side effect: predicted harder, measured softer

For a CM law `⟨E'_lab⟩` rises with `⟨μ_cm⟩` per collision, so this section
originally asserted that the change **hardens** the spectrum and moves
`op-os8x` — this crate's flux being 0.45 % too hard in mean `E` against OpenMC
(3.5 σ), with 1.03 % less flux below 300 keV — the **wrong way**.

That was a prediction, and it is now measured
(`examples/godiva_continuum_spectrum_ablation.rs`, 8 seeds per arm,
2026-09-16), ANISO against ISO on the same 50-bin log grid the cross-code study
uses:

| measure | ANISO | ISO | relative change | |
|---|---|---|---|---|
| mean `E` \[eV\] | 1.47409e6 | 1.47521e6 | **−0.076 % ± 0.109** | 0.7 σ |
| mean `ln E` | 13.6835 | 13.6846 | **−0.008 % ± 0.008** | 1.0 σ |
| flux fraction < 300 keV | 0.155354 | 0.155109 | **+0.157 % ± 0.226** | 0.7 σ |

**Nothing is resolved at 2 σ, and all three central values point the opposite
way to the prediction** — softer, not harder. So the sign argument is **not
supported**. It is not refuted either, and the honest statement is a bound:
the spectral effect of this law is below about `0.22 %` in mean `E` at 2 σ,
against the `+0.45 %` residual `op-os8x` is about.

**Either way, this law is not the explanation for `op-os8x`.** If it hardens,
it does so too little; if it softens, it points away from the residual
entirely. That is a useful exclusion and it is what this measurement bought.

A caution about reading the table: the three measures are **not three
independent votes**. They are taken from the same tallied spectrum in the same
runs and are strongly correlated, so their agreeing in sign is close to one
observation, not three.

**An after-the-fact hypothesis, labelled as such** because it was formed after
seeing the numbers and has not been tested: in a 55.8 %-leakage bare sphere,
raising the transport mean free path preferentially removes the *fast* neutrons
most likely to escape, softening the surviving in-core flux. That would oppose
the per-collision hardening and is tied to the same leakage that produced the
reactivity effect. **The discriminating measurement is the same spectrum
comparison under a reflective boundary** (`k_inf`, no leakage), where only the
per-collision term survives — the decomposition `op-tm9f` used to show its own
fix was leakage-only. Not done.

The original framing also invoked gh:#192's MT=91 Q-value cap as "the same
shape" — a correct fix that moved Godiva 85 pcm *further* from a measured
experiment. That parallel no longer holds here, since the effect this section
predicted is not the one measured. It is left recorded because the general
point stands: correctness is not chosen for its direction.

## What is NOT covered

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
