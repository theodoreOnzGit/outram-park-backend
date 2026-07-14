# COVR — covariance post-processing and plotting

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §COVR); upstream Fortran: `covr.f90` (~3k lines).

## Theory

COVR is an **editing** module that stands to ERRORR as MATXSR/DTFR stand to
GROUPR: it consumes the multigroup covariance output of ERRORR and performs two
largely independent functions:

1. **Reformatting** — writes the group covariances into interchange/report forms
   (e.g. relative standard deviations per group, correlation matrices, or a
   BOXER-style compact representation) for use by other codes and for tabulation.
2. **Plotting** — generates VIEWR input to draw high-quality PostScript figures:
   the per-group **relative standard deviation** curve and the **correlation
   matrix** heat-map that reviewers use to sanity-check an evaluation's
   uncertainties.

The correlation matrix is derived from the covariance as
`corr(g, g') = cov(g, g') / √(cov(g,g)·cov(g',g'))`, clipped to [−1, 1].

## How the port will implement it

**Not yet ported.** Depends on ERRORR (`../errorr/README.md`) being ported first.
Planned: a covariance reader for ERRORR output, a correlation-matrix reducer, and
emit plot commands to the `viewr` module rather than reproducing NJOY's low-level
`graph.f90` plotting primitives verbatim.

## Testing

**TODO.** Gate: for a reference ERRORR covariance, reproduce the per-group
relative standard deviations and correlation matrix against the Fortran oracle.

## Caveats

- **Not required by OpenMC CE** — Phase 5/6; couples to ERRORR and VIEWR.
- The plotting half is lowest priority; the reformatting half is the more useful
  part for programmatic S/U workflows.

## References

- NJOY2016 manual §COVR (LA-UR-17-20093)
- `covr.f90` (NJOY2016 2016.79)
