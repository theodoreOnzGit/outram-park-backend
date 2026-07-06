# PLOTR — plot-data generator for ENDF/PENDF/GENDF

> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §PLOTR); upstream Fortran: `plotr.f90`.

## Theory

PLOTR is the data-selection half of NJOY's plotting pair (PLOTR → VIEWR). It reads
ENDF, PENDF, or GENDF files and produces **VIEWR input** describing the plots:

- conventional **2-D** plots (e.g. cross section *vs* energy) with lin/log axes,
  automatic or user ranges/labels, optional right-hand axis, and one/two titles;
- **3-D** plots (e.g. distributions vs energy and angle/secondary energy).

PLOTR itself does no rendering — it curates the curves and axes; VIEWR turns the
resulting commands into PostScript.

## How the port will implement it

**Not yet ported.** In a Rust workspace this is the module most worth
*re-thinking* rather than porting verbatim: instead of emitting NJOY's bespoke
VIEWR command stream, a port can expose the selected `(x, y)` series as plain data
for any modern plotting stack (or the workspace's own tooling). Planned: a thin
extractor from the `crate::endf` `Tape` to labelled series; VIEWR-command emission
only if byte-compatibility with NJOY plots is required.

## Testing

**TODO.** Gate: for a reference cross section, extract the same `(E, σ)` series
PLOTR would plot and compare against the values in the ENDF/PENDF tape.

## Caveats

- **Lowest priority (Phase 6)** — visualisation, not data processing.
- Faithful VIEWR-command reproduction is rarely worth it; prefer emitting data for
  a modern plotter. Decide before porting.

## References

- NJOY2016 manual §PLOTR (LA-UR-17-20093)
- `plotr.f90` (NJOY2016 2016.79); see also `../viewr/README.md`
