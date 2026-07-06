# WIMSR — WIMS reactor-physics libraries

> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §WIMSR); upstream Fortran: `wimsr.f90`.

## Theory

WIMSR builds libraries for **WIMS** ("Winfrith Improved Multigroup Scheme"), a
widely used lattice-physics code (WIMS-D is freely distributed; WIMS-E was the
commercial 1990-era version) developed at AEE/Winfrith. WIMS uses
**collision-probability** methods to compute fluxes in reactor pin cells and more
complex geometries.

WIMSR reformats GROUPR **GENDF** data into the WIMS library layout: multigroup
cross sections, scattering matrices, fission data, and — importantly for thermal
lattices — **resonance integrals** tabulated for self-shielding, including the
intermediate-resonance treatment WIMS relies on in the near-epithermal range
(coupled to the RESXSR concern).

## How the port will implement it

**Not yet ported.** Requires GROUPR (`../groupr/README.md`); the
resonance-integral tabulation also relates to `resxsr`. Planned: a reformatter
from the GENDF `Tape` to the WIMS library records, plus the resonance-integral
vs. temperature/dilution tables WIMS expects.

## Testing

**TODO.** Gate: reproduce an upstream WIMSR library for a reference nuclide /
group structure against the Fortran oracle, including the resonance-integral
tables.

## Caveats

- **Lowest priority (Phase 6)** — WIMS is not an OUTRAM PARK target; port on
  demand.
- The resonance-integral/intermediate-resonance data ties WIMSR to RESXSR; treat
  them together if this is ever needed.

## References

- NJOY2016 manual §WIMSR (LA-UR-17-20093)
- `wimsr.f90` (NJOY2016 2016.79); WIMS-D/WIMS-E (AEE/Winfrith)
