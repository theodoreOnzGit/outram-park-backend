# LEAPR — thermal scattering law S(α,β) generation

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §LEAPR); upstream Fortran: `leapr.f90` (~3.6k lines).

## Theory

LEAPR **generates** the thermal scattering law S(α, β) for bound moderators (in
ENDF-6 MF=7 form) — it is the *upstream* of THERMR, which only *reads* MF=7. It is
based on the British LEAP + ADDELT codes and handles the large α, β encountered at
high incident energy / low temperature that GASKET could not.

S(α, β) is built in the **incoherent / Gaussian** approximation from a phonon
frequency spectrum ρ(ω):

- **Solid-type (phonon expansion)** — S is a sum over phonon orders,
  `S = e^{−α λ} Σ_n (α λ)ⁿ/n! · T_n(β)`, with the Debye–Waller λ and the
  self-convolution functions T_n derived from ρ(ω).
- **Translational / diffusive** — a free-gas or diffusion term added for liquids.
- **Discrete oscillators** — molecular vibrational modes (e.g. H₂O bending/
  stretching) convolved in.
- **Coherent elastic** parameters (Bragg edges) for crystalline solids.

## How the port will implement it

**Not yet ported.** Planned: the phonon expansion (Debye–Waller integral +
recursive T_n convolutions), the translational term, and discrete-oscillator
convolution, emitting an MF=7 `Tape` that `crate::thermr` can consume — closing
the loop THERMR ← LEAPR entirely in Rust.

## Testing

**TODO.** Gate: regenerate S(α,β) for a standard moderator (e.g. H in H₂O or
graphite) from its phonon spectrum and reproduce upstream LEAPR's MF=7 within
tolerance; downstream, THERMR cross sections from the regenerated law should match
those from the ENDF/B MF=7.

## Caveats

- **Not currently needed** — ENDF/B ships MF=7 thermal evaluations directly, so
  THERMR is fed without LEAPR. Port on demand (new/custom moderators).
- The incoherent-Gaussian approximation has known limits for strongly coherent
  inelastic scatterers.

## References

- NJOY2016 manual §LEAPR (LA-UR-17-20093)
- `leapr.f90` (NJOY2016 2016.79)
- LEAP + ADDELT (UK); GASKET (General Atomics); ENDF-102 File 7
