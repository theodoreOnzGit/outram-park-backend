# Hand-off: heavy neutronics runs for a bigger machine

**Written 2026-09-16, revised the same day at commit `5b6df9b`** on branch
`claude/nuclear-data-neutronics-rewgjw` (`outram-park-backend`). Everything
below is CPU-bound and was bounded rather than resolved on a 4-core container.

**Read this whole file before starting.** Most jobs carry a recorded prediction
that the run is meant to test, and two of those predictions have already failed
once. Reporting "it ran, here are the numbers" without saying whether the
prediction held is not the deliverable.

## What changed in the revision, if you read an earlier copy

Four new ablation hooks landed after this file was first written, closing gaps
3 and 4 of `docs/neutronics-physics-coverage.md`. Three of them have **never
been priced on a real case** — that is now the bulk of the work here, and it is
jobs 4-6 below.

| job | needs OpenMC? | needs a driver written first? |
|---|---|---|
| 1 — continuum angular worth to 3σ | no | no |
| 2 — reflective-boundary spectrum | no | one-line edit, stated inline |
| 3 — LCT-008 after the thermal fix | no | no |
| 4 — free-gas target motion, in-process | no | **yes** |
| 5 — the fission source (ν̄ and χ) | no | **yes** |
| 6 — `op-os8x` per-MT collision tally | **yes, both sides** | **yes** |
| 7 — (n,2n) yield multiplicity | no | **yes** |

Jobs 4-7 need a driver because the hooks are new and no example calls them yet.
The pattern to copy is `examples/godiva_continuum_anisotropy_ablation.rs`,
which is a paired-seed two-arm harness whose entire ablation is **one line**
(`.map(Nuclide::with_isotropic_continuum_scattering)` at line 280). Swapping
that call is the whole change; everything else — seeding, `WORKERS`, the
pooling and the σ arithmetic — is already right and should not be re-derived.

**Do not invent a different harness.** The statistics in that file were got
wrong twice before they were got right (gh:#196), and the corrections are
recorded in it.

Five hooks, not four, as of commit `fc2c234` — the (n,2n) yield landed after
the first revision and is job 7.

## Ground rules that are not negotiable

These come from the workspace `CLAUDE.md` and from this study's own history.

1. **Always `--release`.** Debug runs are meaningless here and the workspace
   forbids them.
2. **Never widen a gate to make something pass.** If an assertion fires,
   that is the result. Report it.
3. **Do not quote an unresolved central value as a measurement.** If the
   difference is inside ~2σ, it is a *bound*. Say "consistent with zero,
   bounded below X at 3σ" — not "worth −38 pcm". This study has been burned by
   single-draw numbers being quoted as answers (gh:#196).
4. **Seeds 1…N are nested.** A 128-seed run using seeds `1..=128` **contains**
   a 32-seed run using `1..=32`. They are not independent and must not be
   pooled or described as confirming each other. Use disjoint seed ranges if
   you want a genuinely independent check.
5. **If a prediction fails, correct the prediction** wherever it is written
   down. Do not reinterpret it after the fact. Job 2 below exists because that
   already happened once.

## Job 1 — resolve the continuum angular law's reactivity worth

**Status: bounded at `−38 ± 23 pcm` (1.6σ) over 128 seeds/arm. Not resolved.**

```bash
OUTRAM_GODIVA_SEEDS=400 cargo run --release -p outram-mc-libs \
    --features endf-pebble-cases --example godiva_continuum_anisotropy_ablation
```

Cost: ~4300 CPU-seconds per arm at 128 seeds on 4 cores, so ~3.1× that at 400.
Two arms. Budget roughly 7–8 CPU-hours; it parallelises over seeds with
`WORKERS = 4` hard-coded in the example — **raise that constant to the machine's
core count** before running, it is the only change needed.

**What it measures.** Ablating the ENDF MF=6 LAW=1 angular law (bead `op-og56`)
on Godiva, ANISO vs ISO over the same seeds.

**Prediction on record, made before any measurement:** `k` down, small — well
under 50 pcm. Held so far on both direction and magnitude.

**What resolution buys:** `σ_diff ≈ 13` at 400 seeds, which puts `−38` at ~3σ
and turns the bound into a measurement.

**Report:** the printed table verbatim, plus whether the ISO arm still
reproduces the pre-`op-og56` pooled `+16 ± 11 pcm` (it did at 0.3σ on 128
seeds — that is the harness check, and if it drifts, something else moved).

## Job 2 — the discriminating spectrum measurement (reflective boundary)

**Status: the per-collision sign argument is NOT supported and needs settling.**

This is the most interesting of the three, because a prediction already failed.

**Background.** For a centre-of-mass law,
`E' = E_cm + E_trans + 2·μ_cm·√(E_cm·E_trans)`, so a forward-peaked cosine
raises `⟨E'_lab⟩` per collision. That argument was written down as "this
hardens the spectrum". Measured on Godiva (vacuum boundary, 8 seeds/arm):

| measure | relative change | |
|---|---|---|
| mean `E` | −0.076 % ± 0.109 | 0.7σ |
| mean `ln E` | −0.008 % ± 0.008 | 1.0σ |
| flux fraction < 300 keV | +0.157 % ± 0.226 | 0.7σ |

Nothing resolved at 2σ, and **all three central values point the other way** —
softer, not harder. (Caution: those three are taken from the same tallied
spectrum and are strongly correlated, so they are close to one observation, not
three.)

**The after-the-fact hypothesis**, formed after seeing this and untested: in a
55.8 %-leakage bare sphere, raising the transport mean free path preferentially
removes the *fast* neutrons most likely to escape, softening the surviving
in-core flux. That would oppose the per-collision hardening.

**The discriminating run: do the same comparison with a REFLECTIVE boundary.**
With no leakage, the leakage-selection term vanishes and only the per-collision
term survives. This is the decomposition `op-tm9f` used to show its own fix was
leakage-only.

Start from `examples/godiva_continuum_spectrum_ablation.rs` and change one
thing — the sphere's boundary condition:

```rust
// in godiva_geometry(), change:
bc: BoundaryType::Vacuum,
// to:
bc: BoundaryType::Reflective,
```

Then raise the seed count:

```bash
OUTRAM_GODIVA_SEEDS=64 cargo run --release -p outram-mc-libs \
    --features endf-pebble-cases --example godiva_continuum_spectrum_ablation
```

Cost at 8 seeds/arm was ~410 s per arm on a contended 4-core box. 64 seeds is
~8×. Budget ~2 CPU-hours.

**How to read it:**

- **Spectrum hardens under reflection** ⇒ both effects are real and compete.
  The per-collision argument is right and leakage-selection masks it on the
  bare sphere. Record both.
- **Spectrum does not harden under reflection** ⇒ the per-collision argument is
  simply **wrong**, and every doc repeating it must be corrected rather than
  explained away. Those are, at time of writing:
  `examples/godiva_continuum_anisotropy_ablation.rs`,
  `tests/continuum_angular_ablation_control.rs`,
  `crates/outram-mc-libs/CLAUDE.md`, and
  `verification_and_validation/continuum_angular/mf6_law1_angular.md`.
- **Nothing resolved at 2σ** ⇒ raise the seed count, or report the bound and
  say the question is open.

**Either way, note what is already settled:** the spectral effect of this law
is below ~0.22 % in mean `E`, against the `+0.45 %` residual bead `op-os8x` is
about. **This law is not the explanation for `op-os8x` in either direction.**
That exclusion stands regardless of how the sign resolves.

## Job 3 — LCT-008 after the thermal-kernel fix (the biggest open gap)

**Status: never run. GitHub #188 named this as its own direct test and closed
without it.**

#188 ("H-in-H₂O incoherent-inelastic kernel transfers 2–5.5 % too little
energy") was closed 2026-09-14 by replacing `equiprobable_emission` with the
ported `aceth.f90::acesix`. Its closing text says the direct test is:

> re-run LEU-COMP-THERM-008 cases 1, 2 and 8, and the three Δk values must
> collapse together and toward zero

That was never done. The last measurement is **2026-09-13**, i.e. *before* the
fix landed:

| case | soluble B-10 | last measured Δk |
|---|---|---|
| 1 | 1511 ppm | +2665 ± 128 pcm |
| 2 | 1335.5 ppm | +2086 ± 118 pcm |
| 8 | 794 ppm (+144 pyrex rods) | +1605 ± 114 pcm |

```bash
for case in 1 2 8; do
  cargo run --release -p outram-mc-libs --features endf-pebble-cases \
      --example lct008_keff -- --case $case
done
```

Cost: the recorded runs were 10 000 × [250 inactive + 400 active] (~4 M active
histories each) and several CPU-hours per case on a 4-core box; a later re-run
used 4000 × [120 + 250]. **Use the full 10 000 × [250 + 400] if the machine can
afford it** — the pairwise differences are the load-bearing statistic and they
need the statistics.

**Why the pairwise differences matter more than the absolute k.** Every case of
LCT-008 is *independently critical*, so every pairwise difference is truly
zero. This code got them wrong by `+579 ± 174` (1→2) and `+481 ± 164` (2→8),
which says its boron is worth too little — and that statement owes nothing to
any estimate of absorption shares. **Report the pairwise differences
explicitly**, not just the three Δk.

**Read the example's own doc comment first** (`examples/lct008_keff.rs`). It
carries the full history including a *refuted* attribution (the residual is not
U-238 resonance escape — the case scan is what broke that), and a note that one
recorded inference (`op-rpb6`'s "1.7 pcm/ppm") does not survive its error bars
because it mixes case 8's lumped pyrex into a soluble-boron line.

Also pass `--clad-omission-bound` on at least one case: it re-adds the omitted
Al-6061 trace alloying elements as Mn-55 and bounds their worth by measurement
rather than assertion.

## Job 4 — price free-gas target motion in-process (NEW, needs a driver)

**Status: the hook exists and is controlled; it has never been run on a case.**

`Nuclide::with_target_at_rest` (landed `b774d32`) makes free-gas target motion
ablatable per-nuclide, in-process. Until then it was reachable only by zeroing
one example's transport temperature through `OUTRAM_RINGRPT_TARGET_AT_REST`, a
process-wide switch that cannot put two arms in one paired-seed study.

**Two runs, and the first is a harness check, not a physics result.**

1. **Godiva. Predicted worth: ~zero.** Recorded before any measurement: Godiva's
   flux is almost entirely above `FREE_GAS_THRESHOLD·kT` = 10.12 eV at 293.6 K,
   where the production path already holds a heavy target at rest. The control
   test proves the two arms are **bit-identical** above that threshold
   (2048/2048 draws, RNG in lockstep), so anything resolved here means the flag
   is reaching a path it should not. **A non-zero Godiva reading is a bug
   report, not a measurement.**
2. **The FHR pebble. Predicted worth: −2242 pcm**, because that is what the
   environment-variable route measured in gh:#193's pricing table. This is the
   real check: the new in-process hook must reproduce the old whole-material
   switch. A materially different number means the per-nuclide hook is reaching
   a different set of nuclides than zeroing the temperature did — which is
   plausible and worth knowing, since the env var zeroed the temperature for
   *everything* including the S(α,β) moderators.

**Report both, and say explicitly whether each matched its prediction.** Run 2
is the one that matters; run 1 exists so that a null result in run 2 could not
be blamed on a dead switch.

**Important caveat on the statistics.** Unlike the angular ablations, this hook
**does not preserve the RNG stream** — the free-gas kernel draws a target
velocity the target-at-rest kernel never draws, so the arms diverge at the first
thermal collision. The difference is attributable **statistically over an
ensemble of seeds only**, never history by history. A single paired run measures
nothing here; budget seeds accordingly and quote a σ.

## Job 5 — price the fission source: ν̄(E) and χ(E→E') (NEW, needs a driver)

**Status: both hooks exist and are controlled; neither has been run on a case.**

`Nuclide::with_frozen_nubar(e_ref)` and
`Nuclide::with_frozen_fission_spectrum(e_ref)` (landed `5b6df9b`) freeze each
factor of the fission source at a stated incident energy, removing its energy
dependence while keeping its magnitude. They *freeze* rather than remove because
a zero ν̄ is not an ablation, it is a subcritical block of metal.

Freeze both at **thermal (0.0253 eV)** for the headline numbers, which answers
"what if the curve had never risen". If you have budget for a second pair, also
freeze at Godiva's flux-average incident energy, which answers the narrower and
better-conditioned "what does the *shape* cost at fixed mean yield".

**Predictions on record, made before any measurement, from the control tests'
own numbers on ENDF/B-VIII.0 U-235:**

| hook | the data | predicted worth on Godiva |
|---|---|---|
| `with_frozen_nubar` | ν̄ 2.42985 @ 0.0253 eV vs 2.64574 @ 2 MeV; `nu_fission` falls **−8.16 %** at 2 MeV when frozen at thermal | **large and negative — thousands of pcm.** This is a direct multiplier on the fission source. |
| `with_frozen_fission_spectrum` | χ's own tape rows integrate to a mean birth energy of 1.99980e6 eV at 1e-5 eV incident and 2.01746e6 eV at 14 MeV — **+0.883 % across the entire range a fission spectrum occupies** | **very small — well under 100 pcm.** A larger reading means the wiring, not the physics. |

χ hardens sharply above ~15 MeV (+15.1 % by 30 MeV, third-chance fission and
pre-equilibrium emission), but no reactor spectrum reaches there. That number is
recorded because it is what makes the +0.883 % believable rather than
suspicious — not because it is expected to matter.

**Why this pair is worth the CPU.** It is the first time either factor of `ν̄ χ`
can be priced at all. ν̄'s slope is the single largest untested lever in the fast
kernel, and the two predictions above differ by roughly two orders of magnitude
— so this is a real test of whether the hooks are wired, not just a measurement.

**Caveat:** the χ hook does not generally preserve the RNG stream either (MF=5
laws are rejection-sampled, so a birth draw's variate count can depend on the
incident energy). Same rule as job 4 — ensemble statistics, quote a σ. The ν̄
hook **does** preserve it, since ν̄ is read rather than sampled.

**Known partial no-op, so you are not surprised by it:** on the LOW (`Core`)
tier *above* the WMP `e_max`, `nu_fission` comes from fast MGXS group data with
ν̄ already baked into the group constant, so `with_frozen_nubar` cannot reach it
there. Every Godiva case runs the HIGH (`Pointwise`) tier, where it is complete.

## Job 6 — the discriminating measurement for `op-os8x` (NEW, needs OpenMC)

**Status: the residual is localised in energy; the cause is not identified.**

This is the only job here that needs OpenMC on both sides, and it is the most
valuable if you have it.

Re-measured 2026-09-16 on HEAD against OpenMC 0.15.3 (`27e38e89`) on identical
data, 8 seeds a side: mean `E` **+0.42 %** (4.6σ), flux below 300 keV
**−1.22 %** (6.0σ), while `k` agrees to `−32 ± 34 pcm`. Localised per-bin to
**+0.88 % excess flux at 1.9–3.0 MeV** (4.7σ, 13.8 % of the flux) against
**1.4–1.9 % deficits at 67–174 keV**.

That is a deficit of down-scatter out of the MeV window. The only channel that
moves a 2 MeV neutron to ~100 keV in one collision is **inelastic** — elastic
off U-238 loses at most 1.7 % per collision. **Excluded by measurement:** the
angular laws (both `op-tm9f` and `op-og56` are in, and the continuum one's
ablation does not move the spectrum), the cross sections (≤0.06 % flux-weighted),
and `k` itself. **Leading suspect: the MT=91 continuum `f₀(E→E')` shape.**

**The measurement that discriminates: a per-MT collision tally in the
1.9–3.0 MeV band, on both codes.** If we have fewer MT=91 collisions there than
OpenMC, the cross section or the branching is wrong; if we have the same number
but they land at the wrong outgoing energy, the `f₀` shape is. Those two have
different fixes and the current evidence does not separate them.

Full record and the provenance for both sides:
`crates/outram-mc-libs/verification_and_validation/openmc_godiva_cross_code/README.md`.

## Job 7 — price the (n,2n) yield multiplicity (NEW, needs a driver)

**Status: the hook exists and is controlled at both data and kernel level; it
has never been priced on a real case.**

`Nuclide::with_unit_n2n_multiplicity` (landed `fc2c234`) cuts MT=16's yield from
2 to 1. (n,2n) is a genuine **neutron multiplier** — one in, two out above
threshold — so this removes a source rather than rearranging one.

**This is the best-conditioned ablation in the set, and it is worth knowing
why.** The secondary is drawn either way and only its *emission* is gated, so
the two arms consume identical RNG streams and stay in exact lockstep history by
history. The paired difference is therefore **deterministic on a shared seed**,
and the variance across seeds is far lower than for two independent runs. You
will resolve this to 3σ with **far fewer seeds than job 1 needed** — start at 32
per arm and check whether σ is already small enough rather than assuming 400.

**Prediction on record, made before any measurement:** down, and small — of order
**tens of pcm**, because MT=16 opens near 5.3 MeV (U-235) / 6 MeV (U-238) and a
fission spectrum puts roughly 1 % of its flux above that. The sign is not merely
expected but forced: deleting a neutron source cannot raise `k`.

**What has already been measured, and what it is not.** The kernel control gives
`k` **0.973390 → 0.972432, −95.8 pcm** on a *bare U-235 sphere* at one seed,
4000 × [20 inactive + 60 active]. That is a **harness check**, not a worth: one
seed, one material, and not Godiva. Do not quote it as the answer — reproduce it
on Godiva with an ensemble, and say whether the prediction held.

**A caution that applies to this job specifically.** The kernel-level control
here **failed on its first run** with the two arms bit-identical, because the
hook had reached four of five emission sites and missed the one `run_keff`
actually uses. If your Godiva ensemble comes back consistent with **exactly**
zero — not small, but identical arms — suspect the wiring before the physics and
say so. `cargo test --release -p outram-mc-libs --test ablation_hook_controls`
is the check; it must stay at 10 passed.

## Environment notes

- ENDF tapes: `reference-data/endf/`, 44 files. Jobs 1-5 and 7 need only these;
  job 6 additionally needs OpenMC and NJOY2016 built (see that job's V&V README
  for the exact commits and the deck).
  Override the directory with `OUTRAM_PARK_ENDF_DIR` if they live elsewhere.
- The `endf-pebble-cases` feature is required for every job here.
- `WORKERS` is hard-coded at 4 in every ablation example, including any you
  copy for jobs 4-7. **Raise it to the machine's core count** — it is the only
  change those files need to scale.
- Nuclear-data reconstruction (RECONR + BROADR on three actinides) costs
  ~145 s once per run, before any transport. That is not a hang.
- Long runs: the workspace `CLAUDE.md` warns that a killed long run is **not a
  failing test**. Do not report a timeout as a failure, and never loosen a
  tolerance because a run was inconvenient.

## What to send back

For each job: the printed output verbatim, the machine's core count and the
wall-clock time, and — for **jobs 1, 2, 4, 5 and 7**, every one of which carries
a recorded prediction — **an explicit statement of whether that prediction held,
failed, or is still unresolved**. A failed prediction is a result, not a
problem; this study's record is built on several of them, and two of the
predictions in this very file have already failed once.

If you wrote a driver for jobs 4-7, send the driver too, not just its output. A
number produced by a program nobody else has is not reproducible, and the point
of these hooks is that the next person can re-run them.
