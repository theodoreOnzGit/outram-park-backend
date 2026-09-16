# Hand-off: heavy neutronics runs for a bigger machine

**Written 2026-09-16** from branch `claude/nuclear-data-neutronics-rewgjw`
(`outram-park-backend`). Everything below is CPU-bound and was bounded rather
than resolved on a 4-core container. Nothing here needs OpenMC or NJOY — all
three jobs run against `reference-data/endf/` alone.

**Read this whole file before starting.** Two of the three jobs have a recorded
prediction that the run is meant to test, and one of them has already failed
once. Reporting "it ran, here are the numbers" without saying whether the
prediction held is not the deliverable.

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

## Environment notes

- ENDF tapes: `reference-data/endf/`, 44 files. All three jobs need only these.
  Override the directory with `OUTRAM_PARK_ENDF_DIR` if they live elsewhere.
- The `endf-pebble-cases` feature is required for all three.
- `WORKERS` is hard-coded at 4 in both ablation examples. Raise it.
- Nuclear-data reconstruction (RECONR + BROADR on three actinides) costs
  ~145 s once per run, before any transport. That is not a hang.
- Long runs: the workspace `CLAUDE.md` warns that a killed long run is **not a
  failing test**. Do not report a timeout as a failure, and never loosen a
  tolerance because a run was inconvenient.

## What to send back

For each job: the printed output verbatim, the machine's core count and the
wall-clock time, and — for jobs 1 and 2 — **an explicit statement of whether
the recorded prediction held, failed, or is still unresolved**. A failed
prediction is a result, not a problem; this study's record is built on several
of them.
