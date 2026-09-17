# OpenMC-Libs Validation & Verification (V&V)

<!-- op-jis-supersede-banner -->
> ⚠️ **Superseded by `op-jis` — PENDING A RE-RUN (2026-08-06).**
> On 2026-08-06 `rng::lcg::prn` gained OpenMC's PCG-RXS-M-XS output permutation
> (bead `op-jis`). The LCG **state recurrence is unchanged**, so all the geometry,
> material, and methodology below still stand, but **every k_eff in this document
> is a statistic of the sampled uniforms and has moved.** The high-fidelity run
> recorded here comes from the `godiva_keff_endf` path, which needs
> `--features net-fetch` to download ENDF/B-VII.1 tapes and has no local cache on
> this machine — so it **could not be re-run** and **no replacement value has
> been measured or invented**. The numbers below are retained as the 2026-07-07
> historical record measured with the **pre-`op-jis` output function (raw top-52
> state bits)**; **do not cite them as current**. Retire this note by re-running
> the ENDF Godiva case on a networked machine.

## Godiva Bare-Sphere Criticality (HEU-MET-FAST-001)

### Benchmark Reference
- **Benchmark:** ICSBEP HEU-MET-FAST-001 (bare Godiva sphere)
- **Reference k_eff:** 1.0000 ± 0.0010
- **Geometry:** Bare HEU metal sphere, r = 8.7407 cm
- **Material:** HEU (~93.8 atom% U-235) at 293.6 K
  - U-234: 4.9184e-4 atoms/barn·cm
  - U-235: 4.4994e-2 atoms/barn·cm
  - U-238: 2.4984e-3 atoms/barn·cm

### High-Fidelity Run (ENDF/B-VII.1, 2026-07-07) — SUPERSEDED, PENDING RE-RUN

**Result (pre-`op-jis`; superseded 2026-08-06, no replacement measured):**
- **k_eff = 1.00094 ± 0.00198**
- **Δk from benchmark = +94 pcm** (~0.4σ combined; well within 1σ)

> **Supersedes note.** These two figures were measured on 2026-07-07 with the
> pre-`op-jis` output function (raw top-52 state bits). They are **not** the
> current generator's result. This case requires `--features net-fetch`
> (ENDF/B-VII.1 download) and was not re-runnable on this machine, so nothing
> here has been replaced or estimated.

**Methodology:**
- Continuous-energy pointwise cross sections from ENDF/B-VII.1
- RECONR resonance reconstruction (0.1% tolerance)
- BROADR Doppler broadening to 293.6 K
- Energy-dependent ν̄(E) from MF=1/452
- Full anisotropic elastic scattering (MF=4)
- Inelastic energy-loss laws (MT=51…91)
- Energy-dependent fission spectrum (MF=5)
- (n,2n) yield-2 multiplicity (MT=16)

**Monte Carlo Settings:**
- 5000 histories/generation
- 40 inactive + 120 active generations
- Total histories: 5000 × 160 = 800,000

**Timing Breakdown:**
| Stage | Duration |
|---|---|
| U-234 reconstruction (RECONR + BROADR) | 0.4 s |
| U-235 reconstruction (RECONR + BROADR) | 12.3 s |
| U-238 reconstruction (RECONR + BROADR) | 29.2 s |
| **Total nuclear data preparation** | **41.9 s** |
| Monte Carlo transport (160 generations) | 49.4 s |
| **Total runtime** | **~91 s** |

**Key Dependencies:**
- Recent RECONR/BROADR fixes critical for achieving this fidelity
- U-238 capture wing pedestal (RECONR grid density)
- Adler-Adler (LRF=4) resonance reconstruction support
- SAMM Phase 6 orchestration improvements

**Interpretation:**
The excellent agreement with benchmark (+94 pcm) validates that:
1. RECONR continuous-energy reconstruction is functioning correctly
2. BROADR Doppler broadening produces accurate temperature-dependent cross sections
3. The Monte Carlo transport physics (anisotropic scattering, energy-dependent spectra) is correctly implemented
4. The port from canonical OpenMC C++ maintains numerical fidelity in this high-consequence benchmark

This result confirms the NJOY porting effort is mature and ready for production transport calculations.

> **Standing of this interpretation after `op-jis` (2026-08-06):** the four
> conclusions above are drawn from the superseded +94 pcm figure. The
> nuclear-data reconstruction (RECONR/BROADR) is deterministic and RNG-free, so
> points 1 and 2 are unaffected by the output-permutation change; points 3 and 4
> depend on the transport k_eff and therefore **await the pending re-run** before
> they can be restated as current.
