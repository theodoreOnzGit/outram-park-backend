# MATXSR — MATXS generalized material format

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §MATXSR); upstream Fortran: `matxsr.f90`.

## Theory

**MATXS** is a generalized CCCC-type interface format that carries neutron,
photon, and charged-particle data together — cross sections, group-to-group
matrices, temperature variations, self-shielding, and time dependence — in one
uniform structure organised by particle type, material, and reaction type. MATXSR
writes MATXS libraries from GROUPR (and GAMINR) **GENDF** output.

MATXS is the standard input to the **TRANSX** code, which reads it to build
effective, self-shielded macroscopic cross sections for a specified mixture and
group structure, ready for deterministic transport. It is the modern replacement
for the DTFR path.

## How the port will implement it

**Not yet ported.** Requires GROUPR (and, for photon data, GAMINR). Planned: an
owned MATXS writer over the GENDF `Tape` — a structured serialisation of the
particle/material/reaction hierarchy with the MATXS control words. No new physics;
the group data comes from GROUPR.

## Testing

**TODO.** Gate: produce a MATXS library for a reference material and verify it
against the Fortran oracle (structure + values), ideally by loading it into
TRANSX.

## Caveats

- **Lowest priority (Phase 6)** — needed only for TRANSX-based deterministic
  workflows OUTRAM PARK does not currently target.
- Breadth (multi-particle, self-shielded, time-dependent) makes it a large
  formatter; port the neutron subset first if a need arises.

## References

- NJOY2016 manual §MATXSR (LA-UR-17-20093)
- `matxsr.f90` (NJOY2016 2016.79)
- TRANSX code; CCCC-III/IV standards
