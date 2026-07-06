# SAMM — R-matrix-limited (RML) resonance kernel

> NJOY2016 port. `samm.f90` has **no standalone manual chapter** — theory is in
> the NJOY2016 manual §RECONR and the ENDF-102 LRF=7 specification. Upstream
> Fortran: `samm.f90` (~7.2k lines).

## Theory

`samm` is not a driver module — it is the **R-matrix engine** shared by RECONR and
UNRESR. Where SLBW/MLBW and Reich–Moore approximate the resonance cross section
with isolated poles, the **R-matrix-limited (RML, ENDF LRF=7)** formalism computes
it from the full multichannel R-matrix:

```
R_{cc'} = Σ_λ  γ_{λc} γ_{λc'} / (E_λ − E)
```

The scattering matrix U (and hence the cross sections) follows from the channel
matrix `(I − R L)⁻¹`, where L carries the penetrabilities/shift factors of each
channel. This handles **overlapping resonances**, multiple particle channels, and
light-nuclide evaluations (¹⁶O, ¹⁹F, …) correctly, where pole approximations fail.

## How the port will implement it

**Not yet ported.** Planned: an owned R-matrix evaluator (channel bookkeeping,
penetrability/shift/phase from Coulomb–hard-sphere functions, the complex
`(I − RL)⁻¹` solve per energy) callable from `crate::reconr` when it encounters an
LRF=7 section, and from a future `crate::modules::unresr`. Faithful port first —
the matrix inversion must match the Fortran before any optimisation.

## Testing

**TODO.** Gate: reconstruct an LRF=7 evaluation (e.g. ¹⁶O or ¹⁹F, whose ENDF/B-VIII
files use RML) and reproduce upstream RECONR's pointwise σ(E) within tolerance.

## Caveats

- **RECONR currently lacks RML** — evaluations using LRF=7 fail until `samm`
  lands. Per `porting-plan.md`, `samm` may need to move earlier than Phase 5 if a
  target evaluation uses RML: **check the evaluation's LRF before relying on
  RECONR.**
- Numerically delicate — channel-matrix conditioning near thresholds needs care.

## References

- NJOY2016 manual §RECONR (LA-UR-17-20093), resonance formalisms
- `samm.f90` (NJOY2016 2016.79)
- ENDF-102, File 2 LRF=7 (R-matrix limited); Lane & Thomas, R-matrix theory
