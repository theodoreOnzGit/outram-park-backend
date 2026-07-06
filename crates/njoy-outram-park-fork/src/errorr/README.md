# ERRORR — multigroup covariance matrices

> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §ERRORR); upstream Fortran: `errorr.f90` (~11.2k lines).

## Theory

Evaluators encode their uncertainty about nuclear data as **covariances** — the
joint (relative) covariance matrix of the evaluated quantities. ERRORR reads the
ENDF covariance files and collapses them to a user **multigroup** structure:

| ENDF MF | Covariance of |
|---|---|
| 31 | ν̄ (average fission neutrons) |
| 33 | cross sections |
| 34 | angular distributions |
| 35 | secondary energy spectra |
| 40 | production cross sections |

For cross sections it forms the group relative covariance
`rcov(g, g') = cov(σ_g, σ_{g'}) / (σ_g σ_{g'})` by projecting the ENDF
sub-material covariance components (NC/NI-type sub-subsections, each a pattern of
energy blocks) onto the group structure with the same flux weighting GROUPR uses.
The result feeds S/U analysis (e.g. sandwich-rule Δk/k propagation).

## How the port will implement it

**Not yet ported.** Planned: an MF=31/33 reader in `crate::endf` (the NI/NC
sub-subsection covariance patterns), then group projection reusing the GROUPR
weighting spectrum. Vector-cross-section covariance (MF=33) first; MF=34/35/40
later. Output pairs with `covr` for visualisation/output.

## Testing

**TODO.** Gate: reproduce an upstream group covariance matrix (e.g. U-235 (n,f)
MF=33) against the Fortran oracle — symmetric, positive-semidefinite, correct
group diagonal relative variances.

## Caveats

- **Not required by OpenMC CE** — Phase 5, sensitivity/uncertainty workflows only.
- Positive-semidefiniteness of ENDF covariances is not guaranteed by the data;
  the port must report (not silently "fix") non-PSD matrices.
- Requires GROUPR-style weighting; couple the two.

## References

- NJOY2016 manual §ERRORR (LA-UR-17-20093)
- `errorr.f90` (NJOY2016 2016.79)
- ENDF-102, Files 31–40 covariance formats
