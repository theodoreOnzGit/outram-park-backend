# UNRESR — unresolved-range self-shielded cross sections

> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §UNRESR); upstream Fortran: `unresr.f90` (~1.8k lines).

## Theory

In the **unresolved resonance range (URR)** individual resonances are too dense
to resolve, so ENDF File 2 (LRU=2) gives only *average* resonance widths and
level spacings plus χ²/Wigner distribution functions. UNRESR converts this
statistical description into **effective self-shielded cross sections**.

It uses the **Bondarenko** narrow-resonance model: the flux depresses inside a
resonance as `φ(E) ∝ 1/(σ_t(E) + σ₀)`, where σ₀ is the *background* (dilution)
cross section representing everything else in the mixture. The effective cross
section is the flux-weighted average

```
σ_x_eff(E, T, σ₀) = ⟨σ_x/(σ_t + σ₀)⟩ / ⟨1/(σ_t + σ₀)⟩
```

evaluated by averaging over resonance-ladder statistics (the quantity-`f`
fluctuation integrals). The result is tabulated versus temperature *T* and
background σ₀ — the Bondarenko `f`-factors GROUPR later interpolates.

## How the port will implement it

**Not yet ported.** Planned: port the fluctuation-integral quadrature (NJOY's
`unresl`/`unresr` GNRL tables) over the χ² width distributions, producing a
σ(E, T, σ₀) table. Shares resonance-statistics machinery conceptually with `purr`
(the Monte-Carlo sibling) — differences are in *how* the ladder average is taken
(quadrature here, explicit ladders in PURR).

## Testing

**TODO.** Gate: reproduce upstream UNRESR effective cross sections for a nuclide
with a URR (e.g. U-238) versus dilution σ₀, within tolerance against the Fortran
oracle.

## Caveats

- Bondarenko/narrow-resonance is a **multigroup**-oriented model and is *not* well
  suited to continuous-energy Monte Carlo — for CE self-shielding use **PURR**
  probability tables (`../purr/README.md`).
- Requires an evaluation with an actual unresolved range; smooth-only nuclides
  are a no-op.

## References

- NJOY2016 manual §UNRESR (LA-UR-17-20093)
- `unresr.f90` (NJOY2016 2016.79)
- I. I. Bondarenko et al., group constants for reactor calculations (1964)
