# BROADR — Doppler broadening (SIGMA1 kernel)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §BROADR); upstream Fortran: `broadr.f90` (~2.0k lines).

## Theory

BROADR generates Doppler-broadened cross sections σ(E, T) from the 0 K
piecewise-linear σ(E) written by RECONR. It uses the **SIGMA1** kernel-broadening
method (D. E. Cullen): the effective cross section seen by a neutron of speed *v*
against a Maxwellian target gas at temperature *T* is

```
σ̄(v) = (1 / v²) · ∫₀^∞ σ(u) · u² · [exp(−(u−v)²·β²) − exp(−(u+v)²·β²)] · (β/√π) du
```

with β² = A·m_n / (2·k·T). "Kernel broadening" is fully accurate — it broadens
resonance and smooth cross sections together with no narrow-resonance
approximation, and reproduces the free-gas scattering kernel exactly.

## How the port implements it

The **SIGMA1 kernel is ported** in [`crate::broadr`]: it consumes a reconstructed
0 K grid and returns σ(E, T) on a (possibly re-thinned) grid, reached through
[`crate::interface`]. Broadening is done in the analytic exponential-integral
form so that adjacent linear panels of σ(E) integrate in closed form against the
Gaussian kernel.

This `modules::broadr` entry is the card-input **driver** (temperature list,
thinning tolerance, tape I/O) and is deferred with the NJOY `main` driver.

The per-energy-point broadening is **data-parallel** (`rayon`): each
`bsigma_scalar` call is an independent pure function of the shared, immutable
0 K grid, so the output points are broadened across all cores (results are
bit-for-bit identical to serial order). On a dense reconstructed grid this is a
large win over serial Fortran — each call walks many σ panels within the
Gaussian cutoff, so the cost is O(points × panels-in-cutoff).

## The wing-pedestal investigation (2026-07-07) — it wasn't SIGMA1

A U-238 code-to-code verification against OpenMC
(`tests/u238_doppler_verification/`) surfaced a striking artefact: after
broadening, U-238 capture carried a spurious **~200 b flat pedestal** several eV
past every strong resonance, where OpenMC (reference NJOY) had decayed to ~1 b
(e.g. 105 eV, 900 K: OpenMC 1.8 b vs the port 211 b). It looked like a SIGMA1
kernel bug — a wing that decays far too slowly.

It was **not** the kernel. Direct kernel checks and grid dumps traced it
entirely to **RECONR's input grid being too coarse in the resonance wings**: the
0 K reconstruction left multi-eV gaps between resonances (for the 102.56 eV line,
nothing between 103.03 eV and 111.10 eV), so the true Lorentzian wing was
represented by a single straight line ~35× too high, and SIGMA1 was both *fed*
that over-stated wing and *sampled* on the same coarse output grid. Fixing the
reconstruction grid density (`reconr::refine_resonance_grid`, adaptive bisection
to tolerance — see `../reconr/README.md`) dropped the U-238 capture RRR L1 vs
OpenMC from **≈0.30 → ≈0.0007** with **no change to the SIGMA1 code at all**.

The lesson worth recording: a "broadening" artefact was really an upstream
reconstruction-grid deficiency. When a broadened wing looks wrong, check the
input grid the kernel is integrating (and the output grid it is sampled on)
before suspecting the kernel.

## Testing

**Ported and verified** — `crate::broadr` unit tests plus the U-238 Doppler
study (`docs/keff-doppler-roadmap.md`): BROADR-broadened capture is the
independent kernel oracle cross-checked against the WMP analytic-broadening path
(`crate::wmp`) at the 6.67 eV resonance. With the reconstruction-grid fix above,
the full RECONR→BROADR pipeline now reproduces OpenMC's Doppler-broadened U-238
capture across the whole resolved region to **<0.1% L1** at 900 K and 1200 K.

## Caveats

- Only free-gas broadening — bound/crystalline effects at very low energy are
  THERMR's job (S(α,β)), not BROADR's.
- Thinning tolerance trades grid size against accuracy; keep it tighter than the
  downstream ACER tolerance.
- The `run()` driver returns `NotPorted`; use `crate::interface`.

## References

- NJOY2016 manual §BROADR (LA-UR-17-20093)
- `broadr.f90` (NJOY2016 2016.79)
- D. E. Cullen, "SIGMA1" / Cullen–Weisbin kernel broadening
