# GROUPR — multigroup cross sections and matrices

> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §GROUPR); upstream Fortran: `groupr.f90` (~12.7k lines).

## Theory

GROUPR collapses continuous-energy data onto a **multigroup** structure for
deterministic transport. A group cross section is the flux-weighted average

```
σ_x,g = ∫_g σ_x(E)·φ(E) dE  /  ∫_g φ(E) dE
```

with φ(E) a weighting spectrum (built-in analytic weights — 1/E, Maxwellian +
1/E + fission, thermal — or a user tabulation, optionally self-shielded via the
Bondarenko σ₀ from UNRESR). Beyond vector cross sections GROUPR builds:

- **group-to-group scattering matrices** σ_{g→g'} with Legendre order for
  anisotropy,
- **photon-production matrices** (neutron in → photon out),
- **ratio quantities** — μ̄, ν̄, inverse velocity, photon yield,
- **fission** as a full group-to-group matrix (χ ⊗ ν σ_f) for generality,
- delayed-neutron spectra by time group, anisotropic thermal scattering.

Output is a **GENDF** tape, the input to CCCCR/MATXSR/DTFR/POWR/WIMSR.

## How the port will implement it

**Not yet ported.** This is the largest Phase-5 module and the hub for all
deterministic/formatter output. Planned decomposition:

- group-structure + weighting-spectrum definitions (enum of built-in weights +
  tabulated), reusing `uom` energy grids;
- the flux-weighted vector integrator over `crate::reconr`/`crate::broadr` σ(E);
- the group-to-group feed-function machinery (elastic/inelastic/(n,xn)/fission)
  — the bulk of the Fortran, driven by MF=4/5/6 secondary distributions;
- a GENDF `Tape` writer in `crate::endf`.

## Testing

**TODO.** Gate: reproduce upstream group cross sections + a scattering matrix for
a reference nuclide/group structure (e.g. a light nuclide on a standard fast or
thermal group set) against the Fortran oracle within tolerance.

## Caveats

- **Not required by OpenMC CE** — Phase 5, deterministic/sensitivity workflows
  only (`porting-plan.md` §2).
- Self-shielded matrices depend on UNRESR/PURR outputs; port those first for URR
  nuclides.
- Huge surface area — port feed-function by feed-function, verifying each.

## References

- NJOY2016 manual §GROUPR (LA-UR-17-20093)
- `groupr.f90` (NJOY2016 2016.79)
