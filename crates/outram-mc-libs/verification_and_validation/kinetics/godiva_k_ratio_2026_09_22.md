# Godiva β_eff and Λ by the k-ratio route

GitHub **#262** scope item 3. Measured 2026-09-22 by
`examples/godiva_kinetics_k_ratio.rs` at 24 paired seeds.

## Methodology

Bare HEU sphere at Godiva's critical radius (8.7407 cm), U-235 + U-238 from
`reference-data/endf/` (ENDF/B-VIII.0) at 293.6 K, 4000 histories ×
[20 inactive + 60 active] per run, single-thread CPU backend.

Two eigenvalue solves per seed, differing **only** in the nuclides' yield:

- `k` — the ordinary run.
- `k_p` — every nuclide through `Nuclide::with_prompt_only_nubar`, so
  production is `ν̄ − ν̄_d(E)` with ν̄_d read from ENDF **MF=1/455**.

`β_eff ≈ 1 − k_p/k`. `Λ = ∫(φ/v) / ∫(νΣ_f φ)` from a whole-geometry tally of
`ScoreType::InverseVelocity` and `ScoreType::NuFission` on the `k` arm.

## The delayed data, before any eigenvalue

Both nuclides carry MF=1/455 **and** MF=5/455, so the per-group split is the
evaluation's own rather than the equal-split fallback:

| | groups | λ_k \[s⁻¹\] |
|---|---|---|
| U-235 | 6 | 0.01334, 0.03274, 0.12078, 0.30278, 0.84949, 2.853 |
| U-238 | 6 | 0.01363, 0.03133, 0.12334, 0.32373, 0.90597, 3.0487 |

Bare delayed fractions at 1 MeV, straight from the ν̄ tables:

| | ν̄ total | ν̄ prompt | β = ν̄_d/ν̄ |
|---|---|---|---|
| U-235 | 2.52969 | 2.51299 | **0.00660** |
| U-238 | 2.53025 | 2.48625 | **0.01739** |

These are the per-nuclide fractions, not β_eff — no spatial or energy
weighting — and they are reported because they are the one part of this
calculation that can be checked against the tape directly.

## Results

| | value |
|---|---|
| `k` | **0.99370 ± 0.00059** |
| `k_p` | **0.98686 ± 0.00067** |
| **β_eff (k-ratio)** | **688 ± 77 pcm** |
| **Λ** | **6.21 ns** (6.212e−9 ± 2.5e−12 s) |

## Interpretation

**Λ is the solid number here.** Its per-seed scatter is 2.5e−12 s on 6.2e−9,
i.e. 0.04 % — because it is a ratio of two tallies from the *same* run, so
almost everything cancels. It also has the right order of magnitude for a bare
fast metal assembly by inspection: a 2 MeV neutron travels ~2e9 cm/s and
Godiva is ~9 cm across, so a generation of a few collisions is a few
nanoseconds.

**β_eff at 688 ± 77 pcm sits between the two nuclides' bare fractions**
(660 pcm for U-235, 1739 for U-238) and much nearer the U-235 one, which is
what a 93.7 % U-235 system should give. That is a consistency check, not a
validation.

### The "pairing" is nominal, and that is worth knowing

The two arms share a seed, but changing ν̄ changes how many fission neutrons
are sampled and therefore the entire downstream RNG stream. So the arms are
**effectively independent**, and the numbers say so:

| | β_eff |
|---|---|
| paired (mean of per-seed differences) | 688.2 ± 76.9 pcm |
| unpaired (ratio of the ensemble means) | 688.8 ± 89.5 pcm |

Identical central values and only a 16 % difference in the error. Real pairing
would have cut the variance substantially. **A single run is useless for
β_eff on this case**: the per-seed values above range from 116 to 1295 pcm,
because `k` and `k_p` each scatter by ~290 pcm and their difference by ~410.
Anyone quoting a β_eff from one run of this program is quoting noise.

### What is NOT established

1. **This is the biased definition.** The k-ratio route is the "prompt-`k`"
   β_eff, not the adjoint-weighted one. The IFP route (#262 scope item 4,
   `src/ifp.cpp`) is not ported. `KineticsMethod::KRatio` is carried on the
   result so the number cannot be quoted without its provenance, and #262's
   acceptance asks for **both** routes with the ratio-route bias stated as a
   measured number. That comparison cannot be made yet.

2. **No literature comparison is made here, deliberately.** #262's acceptance
   asks for a benchmark with a published value. Godiva has one, but this
   checkout has **no citable source for it**.

   ~~The `reactor-literature` submodule is not initialised (the directory is
   empty).~~ **CORRECTED the same day, before this file was committed** — that
   was the first reason written down and it was wrong by the time it was
   checked. The submodule initialises fine
   (`git submodule update --init crates/kovan-literature/reactor-literature`,
   done 2026-09-22, checked out `da94a18`). The real reason is narrower and
   more useful: the corpus holds **17 PDFs** and **none of them concerns
   Godiva, β_eff or delayed-neutron fractions** — it is NRC guidance,
   PHYSOR 2026 papers, thermophysical-property references and the maintainer's
   own thesis.

   The workspace rules require a documented source — author, title, licence,
   URL/DOI, date accessed — for any reference value. Writing down a remembered
   number and calling it the literature value would be exactly the fabrication
   those rules exist to prevent, and this crate has already had one fabricated
   V&V number corrected in place this month.

   **What completing this needs** is a citable Godiva kinetics reference added
   to the corpus — not a calculation, and not a checkout step either.

   **The measured numbers above stand on their own and are not tuned to
   anything.**

3. **Two-nuclide Godiva.** U-234 (4.9184e-4 /b·cm in the ICSBEP specification)
   is absent, as in every two-nuclide case in this crate.

4. **Delayed neutrons are still born at the same instant as prompt ones** —
   the standard `k`-eigenvalue approximation. That is what makes the *ratio*
   route available and is not by itself a time-dependent solution.

## Provenance

OpenMC upstream read at commit `afa7a14` in `/opt/src/openmc`; #262 cites
`608a1c33`, which is not an object in that clone. ENDF/B-VIII.0 tapes from
`reference-data/endf/`, reconstructed at tolerance 1e-3 and broadened to
293.6 K in-process.
