# PURR — unresolved-resonance probability tables

> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §PURR); upstream Fortran: `purr.f90` (~2.9k lines).

## Theory

The Bondarenko self-shielding data from UNRESR is fine for multigroup methods but,
as Levitt observed, not directly usable by continuous-energy Monte Carlo. PURR
implements the **probability-table** method for the unresolved resonance range.

At each URR energy grid point PURR samples many explicit **resonance ladders**
from the ENDF average widths and spacings (with the correct χ²/Wigner statistics),
computes the total and partial cross sections for each ladder, and **bins** them
into a small number (typically 20) of equiprobable probability bins. The table
stores, per energy and temperature, the bin cross sections and their probabilities
— a discrete sampling of the cross-section probability distribution P(σ | E). A
Monte-Carlo code then samples a σ from this table on entering the URR, capturing
resonance self-shielding statistically without resolved resonances.

## How the port will implement it

**Not yet ported.** Planned:

- ladder sampler over the ENDF File-2 LRU=2 average parameters (width sampling
  from χ² with the correct degrees of freedom; spacing from Wigner);
- per-ladder cross-section evaluation (SLBW-like in the URR) with temperature via
  the same broadening kernel as `crate::broadr`;
- equiprobable binning → the ACE **UNR** block (`acefc`), which the ACE writer
  (`crate::acer`) will consume alongside the ESZ grid.

## Testing

**TODO.** Gate: reproduce upstream PURR probability tables for U-238 (bin cross
sections + probabilities) within statistical tolerance, and confirm the
table-averaged σ recovers the infinite-dilution and self-shielded limits.

## Caveats

- Inherently **stochastic** — results depend on ladder count and RNG seed;
  reproducibility against the Fortran oracle is statistical, not exact.
- Requires an evaluation with a URR; feeds the ACE UNR block, so it is coupled to
  ACER 4-series progress.

## References

- NJOY2016 manual §PURR (LA-UR-17-20093)
- `purr.f90` (NJOY2016 2016.79)
- L. B. Levitt, "The probability table method…", Nucl. Sci. Eng. (1972)
