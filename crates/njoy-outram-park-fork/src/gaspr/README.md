# GASPR — gas-production cross sections

> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §GASPR); upstream Fortran: `gaspr.f90` (~1.15k lines).

## Theory

The light products of nuclear reactions — protons, deuterons, tritons, ³He, and
alphas — accumulate as gases in structural and cladding materials and drive
swelling/embrittlement. GASPR forms the **total gas-production** cross sections:

| MT | Species |
|---|---|
| 203 | total proton (H-1) production |
| 204 | total deuteron production |
| 205 | total triton production |
| 206 | total ³He production |
| 207 | total alpha production |

Each is a **yield-weighted sum** over every reaction that emits that particle:
`σ_MT2xx(E) = Σ_r y_{r,species} · σ_r(E)`, where `y` is the multiplicity of the
species in reaction *r*. Existing MT=203–207 sections are removed first, then
recomputed and added to the directory. GASPR is usually run after BROADR.

## How the port implements it

**Ported** in [`crate::gaspr`]: MT=203–207 are computed as a yield-weighted sum
over the reconstructed MF=3 sections using the crate's own `MtReaction`
particle-content naming (rather than NJOY's residual-mass bookkeeping), covering
the modern lumped-channel ENDF representation (MT=11/16/17/22–45/102–117).

## Testing

**Ported and verified** — 6 unit tests (`--lib gaspr`): additivity, multi-particle
yields, two-species channels, and non-gas-reaction exclusion. See
`docs/porting-plan.md` §3.

## Caveats

- The legacy **MT=600–849** detailed-breakup fallback (pre-ENDF/B-VI style) is
  **not ported** — rare in ENDF/B-VII/VIII, which use the lumped channels.
- The `run()` driver returns `NotPorted`; use `crate::gaspr`.

## References

- NJOY2016 manual §GASPR (LA-UR-17-20093)
- `gaspr.f90` (NJOY2016 2016.79)
