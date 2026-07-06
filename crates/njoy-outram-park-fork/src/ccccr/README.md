# CCCCR — CCCC standard interface files

> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §CCCCR); upstream Fortran: `ccccr.f90`.

## Theory

The **CCCC** interface files ("four cees") were standardised by the Committee for
Computer Code Coordination for the US Fast Breeder Reactor Program to let
different reactor codes exchange multigroup data. CCCCR produces them from GROUPR
**GENDF** output:

| File | Content |
|---|---|
| **ISOTXS** | isotope-ordered multigroup cross sections + scattering matrices |
| **BRKOXS** | Bondarenko self-shielding factors vs. σ₀ and temperature |
| **DLAYXS** | delayed-neutron precursor yields, decay constants, spectra |

These are fixed-layout **binary** records (the CCCC-III/IV standard): file
identification, control/counts, then per-isotope data blocks in the mandated
order.

## How the port will implement it

**Not yet ported.** Requires GROUPR (`../groupr/README.md`) first. Planned: an
owned writer for each of the three record structures, reading from the GENDF
`Tape`. This is a **binary-format serialisation** task — the record layouts are
precisely specified; correctness is about byte/word layout, not physics.

## Testing

**TODO.** Gate: produce ISOTXS/BRKOXS/DLAYXS for a reference nuclide and compare
record-by-record against the Fortran oracle (and, ideally, load into a CCCC-aware
reader).

## Caveats

- **Lowest priority (Phase 6)** — OUTRAM PARK does not target CCCC-based fast-
  reactor codes; port on demand.
- Binary word-size/endianness conventions of the CCCC standard must be honoured
  exactly for interchange.

## References

- NJOY2016 manual §CCCCR (LA-UR-17-20093)
- `ccccr.f90` (NJOY2016 2016.79)
- CCCC-III / CCCC-IV standard interface file specifications
