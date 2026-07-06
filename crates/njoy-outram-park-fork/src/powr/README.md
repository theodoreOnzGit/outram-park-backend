# POWR — EPRI-CELL / EPRI-CPM libraries

> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §POWR); upstream Fortran: `powr.f90`.

## Theory

POWR reformats GROUPR **GENDF** multigroup data into the libraries required by the
Electric Power Research Institute (EPRI) lattice-physics codes **EPRI-CELL** and
**EPRI-CPM**, which compute pin-cell / assembly performance for operating and
reloading power reactors. It writes the group cross sections, scattering matrices,
and resonance/self-shielding data in the specific record layout and group
conventions those codes expect.

## How the port will implement it

**Not yet ported.** Requires GROUPR (`../groupr/README.md`) first. Planned: a
serialisation-only reformatter from the GENDF `Tape` into the EPRI-CELL/CPM file
structure. No new physics — the data originates in GROUPR.

## Testing

**TODO.** Gate: reproduce an upstream POWR library for a reference nuclide/group
structure against the Fortran oracle.

## Caveats

- **Lowest priority (Phase 6)** — EPRI-CELL/EPRI-CPM are proprietary EPRI products
  OUTRAM PARK does not target; port strictly on demand.
- Output format is defined by the consuming codes' documentation, not by ENDF.

## References

- NJOY2016 manual §POWR (LA-UR-17-20093)
- `powr.f90` (NJOY2016 2016.79); EPRI-CELL / EPRI-CPM (proprietary, EPRI)
