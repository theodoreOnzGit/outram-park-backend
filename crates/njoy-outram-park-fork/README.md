# njoy-outram-park-fork

Pure-Rust port (**work in progress**) of [NJOY2016] — the modular nuclear-data
processing system that turns evaluated ENDF data into libraries for transport
codes. In OUTRAM PARK its job is to produce the **ACE** continuous-energy
libraries that [`openmc-libs`] consumes: NJOY is the data-prep step *upstream* of
an OpenMC calculation.

> **Status — RECONR Phase 2c + BROADR (SIGMA1):** RECONR reconstructs ENDF
> materials with no resonances (LRU=0, e.g. H-2), resolved SLBW/MLBW resonances
> (LRU=1, LRF=1/2, e.g. Ar-37), and **resolved Reich-Moore resonances (LRU=1,
> LRF=3, e.g. U-235)** — including the fissile two-channel / 3×3 complex
> R-matrix path. BROADR now performs **full SIGMA1 free-gas Doppler broadening**
> (both kernel terms). The `NuclearDataLibrary` OOP API supports the
> `from_file → reconstruct → broaden` pipeline with uom-typed cross-section
> queries and a `ContinuousEnergyData` export. Unresolved resonances (LRU=2,
> PURR) are the next phase. See
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

This crate is a **derivative work** (a translation) of NJOY2016 v2016.79.

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

The reference Fortran NJOY2016 lives at `../../../NJOY2016` and is used as a
golden oracle: run a module in upstream NJOY on a reference ENDF evaluation,
then assert the Rust port reproduces the same tape/ACE output within tolerance.
See the porting plan for the test strategy.

[NJOY2016]: https://github.com/njoy/NJOY2016
[`openmc-libs`]: https://github.com/theodoreOnzGit/outram-park-backend
