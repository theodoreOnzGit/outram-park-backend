# MIXR — linear combinations of cross sections

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §MIXR); upstream Fortran: `mixr.f90`.

## Theory

MIXR builds a new PENDF tape whose reactions are user-specified **linear
combinations** of cross sections drawn from one or more input tapes:

```
σ_out,MT(E) = Σ_i  w_i · σ_{in_i, MT}(E)
```

The classic uses are (a) constructing an **element** cross section from its
isotopes weighted by natural abundance, and (b) mixing materials to plot combined
cross sections (with PLOTR/VIEWR). The output tape contains only ENDF **File 1 and
File 3** sections and assumes **linear-linear** interpolation (input interpolation
laws are ignored for ENDF-format inputs).

## How the port will implement it

**Not yet ported.** Straightforward once tapes share (or are unionised onto) a
common energy grid: union the input grids, interpolate each contributing σ(E)
linearly, and accumulate the weighted sum into a new `crate::endf` `Tape` with
File 1 + File 3 only. No resonance or distribution handling — purely MF=3 vector
arithmetic.

## Testing

**TODO.** Gate: mix two isotopes by abundance and check the result equals the
hand-computed weighted sum on the union grid (and matches upstream MIXR).

## Caveats

- **Lowest priority (Phase 6)** — a convenience/plotting utility.
- Output is MF=3 only and lin-lin — not a full evaluation; do not feed it back
  into resonance-dependent modules.

## References

- NJOY2016 manual §MIXR (LA-UR-17-20093)
- `mixr.f90` (NJOY2016 2016.79)
