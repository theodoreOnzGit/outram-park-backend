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

~~**Not yet ported.** Requires GROUPR (and, for photon data, GAMINR). Planned: an
owned MATXS writer over the GENDF `Tape` — a structured serialisation of the
particle/material/reaction hierarchy with the MATXS control words. No new physics;
the group data comes from GROUPR.~~ **CORRECTED 2026-11-13 (gh:#1369):** translated routine by routine
from `matxsr.f90` (NJOY2016.80 `ac5adf5`), not redesigned:

| Fortran | Rust |
|---|---|
| `matxsr`, `ruinm` (cards) | `cards.rs` |
| `mtxdat`, `findg`, `vector` | `data.rs` |
| `matrix`, `band`, `shufl` | `matrix.rs` |
| `hname` | `hname.rs` |
| `cmatxs` (coded branch) | `output.rs` |
| `lst1io` | replaced by the GENDF reader (`errorr::gendf`), same columns |

Kept from upstream: the `real(k4)` container (every word on the file is the
`f32` upstream stores; `a+b` with `b` double is a double sum rounded to `f32`,
`a+a` a single sum), `hname`'s hard-wired `iverf = 6` (ENDF-4 MT=602/621 are
named `p02`/`p21`), `maxord` as set by `band`, `shufl`'s passes without clearing
the container, and gfortran's execution of a `/` reached exactly as the list
ends (an empty line).

Refused by name: a binary MATXS (`nmatx < 0`); `shufl` when one band does not
fit the container (`imax` is then undefined upstream); and a vector MT=1/501 (or
charged-particle MT=2) after another vector reaction, where upstream overwrites
`hvps(1:2)` and prints a name it never set (zero bytes; deck X2 in the test).

## Testing

~~**TODO.**~~ **CORRECTED 2026-11-13 (gh:#1369):** `tests/deck_driver_photon_vs_njoy2016.rs` runs upstream decks
03 (gamma only, H and U, `gscat`) and 30 (H-1: `nscat`, `ng`, `gscat`, 175 and 42
groups) whole in NJOY2016.80 and here: the MATXS units (35: 211 lines, 44: 18 924
lines) are identical line for line, and unit 44 is upstream's `referenceTape44`
byte for byte (2026-11-13). Deck X2 (our own: a 4-group read-in gamout tape)
is identical too. Not exercised by any deck: the reference-subtraction path
(`iref`, several temperatures or sigma-zeros), constant sub-blocks (`jconst`),
charged particles, thermal `ntherm` types. Not loaded into TRANSX.

## Caveats

- **Lowest priority (Phase 6)** — needed only for TRANSX-based deterministic
  workflows OUTRAM PARK does not currently target.
- Breadth (multi-particle, self-shielded, time-dependent) makes it a large
  formatter; port the neutron subset first if a need arises.

## References

- NJOY2016 manual §MATXSR (LA-UR-17-20093)
- `matxsr.f90` (NJOY2016 2016.79)
- TRANSX code; CCCC-III/IV standards
