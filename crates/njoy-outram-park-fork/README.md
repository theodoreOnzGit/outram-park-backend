# njoy-outram-park-fork

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping pass" command). A crate is **complete** only once the maintainer has personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

Pure-Rust port (**work in progress**) of [NJOY2016] — the modular nuclear-data
processing system that turns evaluated ENDF data into libraries for transport
codes. In OUTRAM PARK its job is to produce the **ACE** continuous-energy
libraries that [`outram-mc-libs`] consumes: NJOY is the data-prep step *upstream* of
an OpenMC calculation.

## GPU compute — precision / performance tradeoff (opt-in)

njoy ships an **optional GPU compute path** (Cargo feature `gpu`, **desktop only**
— `wgpu` is a target-gated dependency, so Android stays lean and CPU-only). It
accelerates the *compute-bound* windowed-multipole (WMP) cross-section
evaluation — the complex Faddeeva pole-sum — on the GPU.

**The tradeoff, stated plainly (accepted design choice):**

- **GPU = `f32` (single precision), fast.** On large energy grids the
  compute-bound Faddeeva kernel is dramatically faster than the CPU (measured on
  an NVIDIA RTX 3050: ~3× at 1e4 energies, ~23× at 1e5, ~60× at 1e6; below ~1e4
  the CPU wins because kernel-launch overhead dominates).
- **CPU = `f64` (double precision), trusted.** The CPU path is the
  **deterministic reference**. The GPU `f32` result introduces accuracy loss that
  grows on dense grids near sharp resonances — max relative error ≈ 3e-3 at 2000
  energies, rising to ≈ 2e-2 at 1e6. So **GPU is acceleration only**; the trusted
  / V&V / publication path stays on the CPU.
- **Graceful fallback.** The GPU path falls back to CPU with a debug message when
  no GPU adapter is present; on Android there is no GPU code compiled at all.

Choose GPU when you want throughput on large energy grids and can accept `f32`
precision; keep CPU for the reference/validated result. (Beads `op-0m5`,
`op-0nh`.)

> **Status — most of the pipeline is ported (translation-level; V&V is the trust
> gate, see banner above).** RECONR reconstructs all five ENDF-6 resolved-resonance
> formalisms: no resonances (LRU=0, e.g. H-2), SLBW/MLBW (LRF=1/2, e.g. Ar-37),
> Reich-Moore (LRF=3, e.g. U-235, incl. the fissile two-channel / 3×3 complex
> R-matrix path), Adler-Adler (LRF=4), and R-Matrix-Limited (LRF=7) — the last
> dispatched to the full `samm` port (`reconr/mf2.rs` routes LRF=7 → `samm`).
> BROADR performs **full SIGMA1 free-gas Doppler broadening**. The unresolved
> region is covered by **UNRESR** (infinite-dilution average σ) and **PURR** (URR
> probability tables, incl. the `unrest` Monte Carlo core). Ported downstream of
> RECONR/BROADR: **HEATR** (KERMA H1–H5 + damage-energy H7 two-body channels; the
> full photon energy-balance H6 is deferred), **GASPR** (gas production MT=203–207),
> **THERMR** (MF=7 S(α,β): coherent/incoherent elastic + inelastic), **SAMM** (all
> six phases of the R-matrix-limited formalism, for the RECONR-reachable scope),
> and **ACER** in full (4a cross-section core, 4c elastic angular, 4d energy
> distributions, 4e heating column, 4f thermal S(α,β) tables, 4g Windowed
> Multipole import). The **WMP** evaluator (`src/wmp.rs`, independent MIT CRPG
> work — not NJOY) is a ~1276-line port with a 125-nuclide CORE library baked in.
> The Phase-5 multigroup/covariance set is also ported: **GROUPR** (~9.4k lines),
> **GAMINR**, **ERRORR**, **COVR**, and **LEAPR**; plus the Phase-6 formatters
> **DTFR**, **RESXSR**, and **MIXR**. The `NuclearDataLibrary` OOP API supports the
> `from_file → reconstruct → broaden` pipeline with uom-typed cross-section
> queries and a `ContinuousEnergyData` export. The remaining `NjoyError::NotPorted`
> stubs are only **CCCCR, MATXSR, POWR, PLOTR, VIEWR, and WIMSR** (output formats
> OUTRAM PARK does not target). All ported modules are translation-level until a
> V&V case demonstrates otherwise — see the banner and
> [`docs/porting-plan.md`](docs/porting-plan.md).

## Patch notes

### 2026-06-29 — Reich-Moore (LRF=3) + SIGMA1 Doppler broadening

- **RECONR Phase 2c — Reich-Moore (LRF=3).** Ported `csrmat`/`frobns`/`thrinv`
  from `reconr.f90`: per-l 3×3 complex R-matrix inversion for fissile nuclides
  (two fission channels GFA/GFB) plus a scalar fast path for non-fissile cases.
  U-235 reconstructs to its accepted 2200 m/s thermal cross sections from raw
  resonance parameters: σ_fission ≈ 586 b, σ_capture ≈ 99 b, σ_elastic ≈ 14 b,
  σ_total ≈ 700 b. Validated by `tests/reconr_u235.rs` (11 tests).
- **BROADR — SIGMA1 free-gas Doppler broadening.** Pure-Rust SIGMA1 with both
  kernel terms (the dominant `exp(-(x-y)²)` pass and the `exp(-(x+y)²)`
  near-thermal correction), analytic panel integrals via the f/h functions, and
  a pure-Rust `erfc` (no C ABI). Verified against a brute-force numerical
  integration of the kernel and confirmed to preserve a 1/v cross section
  exactly — the true physical invariant.
- **Bug fix — resonance/background merge.** The previous point-merge
  interpolated a half-built, unsorted grid while appending to it, causing
  runaway accumulation (U-235 fission read ~1.79 M b instead of ~586 b). The
  merge now snapshots the background once and rebuilds each reaction as
  `background(E) + resonance(E)`. Also fixed a `doppler_broaden` panic when
  reactions carried different-length grids.

## License and provenance — please read

This crate is a **derivative work** (a translation) of NJOY2016 v2016.79
([`njoy/NJOY2016`](https://github.com/njoy/NJOY2016), commit
`ac5adf5f33d893e42f2eed7fb286b0d51c7580da` — see `upstream_source/README.md`
for the full provenance record).

- **Upstream license:** NJOY2016 is under a *modified BSD 3-Clause* license (the
  LANL/DOE variant), preserved verbatim in [`LICENSE.njoy`](LICENSE.njoy). Its
  terms continue to apply to everything derived from NJOY2016.
- **This crate's license:** `GPL-3.0-only`, matching the rest of the OUTRAM PARK
  workspace. The modified BSD 3-Clause license is GPL-compatible, so the combined
  work may be distributed under the GPL.
- **Not the LANL version.** This is **not** endorsed by or affiliated with Los
  Alamos National Laboratory, LANL, Los Alamos National Security LLC, or the U.S.
  Government. Do **not** report issues with this port to the NJOY developers.
- **Documentation source (theory).** The per-module `README.md` files under
  `src/modules/*/` summarise their theory from the **NJOY2016 users manual**,
  taken from the published repository
  [`njoy/NJOY2016-manual`](https://github.com/njoy/NJOY2016-manual) (commit
  `9a2951f`, 2022-03-02), which corresponds to LANL document **LA-UR-17-20093**.
  The manual is © 2016 Los Alamos National Security, LLC, under the same modified
  BSD 3-Clause (LANL/DOE) terms as the code. Full credit to the NJOY authors
  (R. E. MacFarlane, A. C. Kahler, D. W. Muir, et al.); this port paraphrases the
  manual for documentation and claims no authorship of the underlying methods.

The full provenance, modification statement, and no-endorsement notice are in
[`NOTICE`](NOTICE). Redistributions must keep both `LICENSE.njoy` and `NOTICE`.

## The pipeline

```
MODER → RECONR → BROADR → [HEATR] → [GASPR] → [PURR] → [THERMR] → ACER → ACE file → OpenMC
```

Modules in `[brackets]` are optional depending on what physics the ACE library
needs (heating/damage, gas production, unresolved-resonance probability tables,
thermal scattering).

## Verifying against upstream

The reference Fortran NJOY2016 lives at `upstream_source/NJOY2016` and is used as a
golden oracle: run a module in upstream NJOY on a reference ENDF evaluation,
then assert the Rust port reproduces the same tape/ACE output within tolerance.
See the porting plan for the test strategy.

[NJOY2016]: https://github.com/njoy/NJOY2016
[`outram-mc-libs`]: https://github.com/theodoreOnzGit/outram-park-backend
