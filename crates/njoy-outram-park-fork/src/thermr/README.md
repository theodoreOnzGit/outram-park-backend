# THERMR — thermal scattering cross sections from S(α,β)

> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §THERMR); upstream Fortran: `thermr.f90` (~3.4k lines).

## Theory

At thermal energies (below ~4 eV) the target atom is bound in a molecule or
crystal, so free-gas broadening (BROADR) is wrong. Scattering is governed by the
**thermal scattering law** S(α, β) — the dynamic structure factor in the
dimensionless momentum transfer α and energy transfer β. THERMR turns MF=7 S(α,β)
into pointwise cross sections and secondary distributions:

- **Coherent elastic** — Bragg diffraction from crystalline planes; a sawtooth
  σ(E) with edges at the Bragg energies, `σ(E) = (1/E)·Σ_{E_i<E} f_i`.
- **Incoherent elastic** — bound-atom elastic with a Debye–Waller form,
  `σ(E,T) = (σ_b/2)·(1 − e^{−4EW'})/(2EW')`, plus its angular law.
- **Incoherent inelastic** — the bound double-differential kernel
  `d²σ/dΩdE' ∝ (σ_b/4π)·√(E'/E)·S(α,β)`, integrated for σ(E→E') and σ_inel(E).

## How the port implements it

**Ported** in [`crate::thermr`]: `mf7` (MT=2 coherent/incoherent elastic, MT=4
incoherent inelastic S(α,β) parsing), `coherent` (σ_coh + Bragg reflection
cosines/weights), `incoherent_elastic` (closed-form σ + equiprobable cosines via
analytic CDF inversion), `inelastic` (double-differential kernel, σ(E→E'), and
the `nieb×nang` equiprobable emission table for the ACE ITXE block). The ACE
`…t` table writer is [`crate::acer::thermal`] (`AceTable::thermal_from_mf7`).

## Testing

**Ported and verified** — Al-27 (σ_b≈1.45 b; σ_inel rises to σ_free≈1.35 b near
1–2 eV) and H-in-ZrH; ACE round-trip in `tests/thermal_ace.rs` and
`tests/thermal_ace_zrh.rs`. See `docs/porting-plan.md` §4f.

## Caveats

- Only the **IFENG=0** (equiprobable) inelastic form is emitted — the
  skewed/continuous **IFENG=1/2** forms are not ported.
- Multi-scatterer mixing is taken as `nmix = 1`.
- Generating S(α,β) when an evaluation lacks it is **LEAPR**'s job (see
  `../leapr/README.md`), not THERMR's — unported, but ENDF/B ships MF=7 directly.
- The `run()` driver returns `NotPorted`; use `crate::thermr`.

## References

- NJOY2016 manual §THERMR (LA-UR-17-20093)
- `thermr.f90` (NJOY2016 2016.79)
- ENDF-102, File 7 thermal scattering format
