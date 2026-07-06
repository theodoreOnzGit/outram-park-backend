# VIEWR — PostScript plotting back-end

> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §VIEWR); upstream Fortran: `viewr.f90` (+ shared `graph.f90`).

## Theory

VIEWR is the rendering half of NJOY's plotting pair. It reads user (or
PLOTR/COVR/DTFR-generated) commands defining 2-D and 3-D graphs and writes a
high-quality **PostScript** file. Capabilities include conventional 2-D plots
(lin/log axes, auto or user ranges/labels, optional right-hand axis, one/two
titles), curves with various line patterns labelled by tags/arrows or a legend
block, and 3-D surface/isometric plots. The low-level drawing primitives are
shared with COVR/PLOTR via `graph.f90`.

## How the port will implement it

**Not yet ported.** As with PLOTR, verbatim porting of a bespoke PostScript
generator is rarely the right call in a modern Rust workspace. Planned stance:
port only if byte-compatible NJOY plots are explicitly required; otherwise treat
VIEWR/PLOTR as a *data* interface and render with a modern plotting library. This
module remains a stub documenting the upstream behaviour and that decision.

## Testing

**TODO.** If ported: render a reference plot command set and diff the PostScript
(or a rasterised comparison) against the Fortran oracle.

## Caveats

- **Lowest priority (Phase 6)** — pure visualisation; no nuclear-data content.
- Consider *not* porting the PostScript engine at all — see the plan note above.

## References

- NJOY2016 manual §VIEWR (LA-UR-17-20093)
- `viewr.f90`, `graph.f90` (NJOY2016 2016.79); see `../plotr/README.md`
