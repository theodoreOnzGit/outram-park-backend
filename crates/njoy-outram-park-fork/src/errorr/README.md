# ERRORR — multigroup covariance matrices

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


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

**Partially ported.** `src/errorr/covariance.rs` is a faithful structural
reader for MF=31/33 sections (`read_covariance_section`, plus the `iverf`
format-era detector `detect_endf_version`): it walks a section's `NL`
subsections and their `NC`/`NI` sub-subsections and returns every raw ENDF
field (energy windows, `LTY`, `LS`/`LB`, the flat data array), ported
line-for-line from `covcal`'s ENDF I/O staging phase (`errorr.f90:1868-2060`).
It does **not** decode `LB`-tagged matrix data, apply energy windows, or
compute a single covariance number — that group-average interpretation
(`errorr.f90:2086-2417`) needs the union energy grid (`gridd`,
`errorr.f90:1091-1483`, not yet ported) and is fused into `covcal`'s
per-group-pair loop rather than being a separable decode step. MF=34/35/40 use
different subsection layouts and are not covered by this reader. Group
projection (reusing the GROUPR weighting spectrum) is still planned. Output
pairs with `covr` for visualisation/output.

## Testing

**Structural tests only so far** (`src/errorr/covariance.rs`'s
`#[cfg(test)]` block): a synthetic hand-built round-trip, and a smoke test
against the real `n-092_U_235-ENDF8.0.endf` MF=33/MT=1 and MT=2 sections
checking record counts, `LTY`/`LS`/`LB` values, and the `NT = NP*(NP+1)/2`
symmetric-pack invariant for `LB=5`. No covariance *values* are computed or
verified yet.

**Still TODO.** Gate: reproduce an upstream group covariance matrix (e.g.
U-235 (n,f) MF=33) against the Fortran oracle — symmetric,
positive-semidefinite, correct group diagonal relative variances. Blocked on
the union-grid and group-average kernels above.

## Caveats

- **Not required by OpenMC CE** — Phase 5, sensitivity/uncertainty workflows only.
- Positive-semidefiniteness of ENDF covariances is not guaranteed by the data;
  the port must report (not silently "fix") non-PSD matrices.
- Requires GROUPR-style weighting; couple the two.

## References

- NJOY2016 manual §ERRORR (LA-UR-17-20093)
- `errorr.f90` (NJOY2016 2016.79)
- ENDF-102, Files 31–40 covariance formats
