# RESXSR — pointwise resonance cross-section files

> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §RESXSR); upstream Fortran: `resxsr.f90`.

## Theory

In thermal-reactor lattices, resonance self-shielding in the **near-epithermal**
range (≈4–200 eV) is poorly captured by the simple Bondarenko model, which assumes
every absorber resonance is *narrow* with respect to the energy a neutron loses
per scatter. Many resonances in this range violate that assumption. RESXSR
prepares **pointwise** resonance cross-section files so that codes can apply
better treatments (e.g. intermediate-resonance theory, as used in the WIMSR path)
instead of relying on narrow-resonance f-factors alone.

## How the port will implement it

**Not yet ported.** Depends on the reconstructed pointwise σ(E) from RECONR
(`crate::reconr`, already ported) plus a group/energy-mesh selection. Planned: a
reformatter that extracts the near-epithermal pointwise cross sections into the
RESXSR file layout expected by the consuming lattice codes.

## Testing

**TODO.** Gate: reproduce an upstream RESXSR file for a resonance absorber (e.g.
U-238) against the Fortran oracle.

## Caveats

- **Lowest priority (Phase 6)** — supports lattice-physics codes OUTRAM PARK does
  not currently target.
- Closely tied to the WIMSR intermediate-resonance workflow; port them together
  if this becomes needed.

## References

- NJOY2016 manual §RESXSR (LA-UR-17-20093)
- `resxsr.f90` (NJOY2016 2016.79); TRANSX; intermediate-resonance theory
