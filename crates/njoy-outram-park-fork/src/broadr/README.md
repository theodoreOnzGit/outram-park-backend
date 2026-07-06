# BROADR — Doppler broadening (SIGMA1 kernel)

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

## Testing

**Ported and verified** — `crate::broadr` unit tests plus the U-238 Doppler
study (`docs/keff-doppler-roadmap.md`): BROADR-broadened capture is the
independent kernel oracle cross-checked against the WMP analytic-broadening path
(`crate::wmp`) at the 6.67 eV resonance.

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
