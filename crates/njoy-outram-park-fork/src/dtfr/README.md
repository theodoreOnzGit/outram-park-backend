# DTFR — DTF-IV format for discrete-ordinates codes

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §DTFR); upstream Fortran: `dtfr.f90`.

## Theory

DTFR writes multigroup transport tables in the **DTF-IV** card-image format —
designed for the early DTF-IV Sₙ code and still accepted as an input option by
many discrete-ordinates and diffusion codes. It reorganises GROUPR **GENDF**
multigroup data into the DTF table layout: per group a block of
`[σ_transport-corrected, σ_absorption, ν σ_f, (χ), scattering P_ℓ down/within/up]`
rows, with the transport correction and group ordering the Sₙ code expects. DTFR
also has a simple built-in plotter for a quick look at its output.

## How the port will implement it

**Not yet ported.** Requires GROUPR (`../groupr/README.md`) first. Planned: a
straightforward reformatter from the GENDF `Tape` to DTF card images (a
serialisation concern, not new physics). The built-in plotter is out of scope —
route to `viewr`/external tooling if plots are needed.

## Testing

**TODO.** Gate: reproduce an upstream DTFR table for a reference nuclide/group
structure card-for-card (numerically) against the Fortran oracle.

## Caveats

- **Lowest priority (Phase 6)** — OUTRAM PARK does not target DTF Sₙ codes;
  `porting-plan.md` marks it "port only on demand."
- Largely **superseded** by MATXS/TRANSX (`../matxsr/README.md`) — prefer that
  path for new deterministic work.

## References

- NJOY2016 manual §DTFR (LA-UR-17-20093)
- `dtfr.f90` (NJOY2016 2016.79); DTF-IV Sₙ code
