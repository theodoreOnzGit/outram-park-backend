# POWR — EPRI-CELL / EPRI-CPM libraries

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §POWR); upstream Fortran: `powr.f90`.

## Theory

POWR reformats GROUPR **GENDF** multigroup data into the libraries required by the
Electric Power Research Institute (EPRI) lattice-physics codes **EPRI-CELL** and
**EPRI-CPM**, which compute pin-cell / assembly performance for operating and
reloading power reactors. It writes the group cross sections, scattering matrices,
and resonance/self-shielding data in the specific record layout and group
conventions those codes expect. No new physics: the data originate in GROUPR.

- `lib = 1` (fast, input for GAMTAP): 68 groups. Absorption, fission, nu, fission
  and delayed chi, P0/P1 elastic, inelastic and (n,2n) matrices packed in GAM
  form, and absorption/fission f-factors (`ln(sigma0) - 2 ln f`) over the sigma
  zeroes and temperatures for groups 19-62.
- `lib = 2` (thermal, input for LIBRAR): 35 groups. Absorption, fission,
  transport, and the thermal scattering matrix (per unit energy, packed by row).
- `lib = 3` (EPRI-CPM, input for CLIB): 69 groups.

A 185-group GENDF is collapsed to these structures by `icgrp`'s tables.

## How the port implements it

~~**Not yet ported.** Requires GROUPR (`../groupr/README.md`) first. Planned: a
serialisation-only reformatter from the GENDF `Tape` into the EPRI-CELL/CPM file
structure.~~ **Translated 2026-11-14 (gh:#1373)** for `lib = 1` and `lib = 2`,
line for line, keeping upstream's statement labels as `match` arms:

| file | upstream |
|---|---|
| `mod.rs` | `powr` (branch), `icgrp` |
| `reader.rs` | `powm`'s module variables, `scr(5000)`, the record routines (through `plotr::stream::EndfStream`, read card by card from the GENDF text) |
| `fast.rs` | `fast`, `gamll`, `gamxs`, `mergt` |
| `gamff.rs` | `gamff` |
| `therm.rs` | `therm`, `packa` |
| `fmt.rs` | the edit descriptors (`1pe12.5`, `f10.4`, `iw`) |
| `../deck/powr.rs` | the cards of `powr`, `fast`, `therm` |

All arithmetic is `real(kr)`; POWR's only `real(4)` items (`fsn`) hold
characters. The listing (`nsyso`, with `pgam`, `pgamff`, `tprnt`) is not
written.

## Testing

~~**TODO.**~~ `tests/deck_driver_powr_vs_njoy2016.rs`: seven decks of our own (no
upstream deck runs POWR) on NJOY2016's `t404` (Pu-238, Th-232) and `t511` +
`t322` (graphite), run whole by NJOY2016.80 and by `run_deck`. The GENDF and the
library are identical line for line (trailing blanks included) on every deck:
fast with f-factors, (n,2n), delayed neutrons, 185-group collapse; thermal free
gas at three temperatures, 185-group collapse, graphite with a thermal elastic
MT. Ablations show the gate fails on a trimmed line, a changed `izref` and a
changed `itrc`. Methodology and the table:
`verification_and_validation/deck_driver/README.md` §39. Unit tests: `icgrp`'s
tables, `packa`, the edit descriptors, the `e11.0` label check.

## Caveats

- **`lib = 3` (EPRI-CPM) is not translated** and is refused by name
  (`powr.f90:1992-4902`: `cpm`, `pinit`, `sfile2`-`sfile6`, `cpmout`).
  **2026-11-14 (gh:#1373): upstream writes no `lib = 3` library.** `cpm` reads
  card 5 into locals and fills the per-material arrays from `scr`, which nothing
  wrote (an instrumented NJOY2016 printed zeros for every card-5 field). On eight
  card-5 variants NJOY2016.80 stopped in `sfile2` ("reference sigma zero not
  found"), on `DEALLOCATE` of `glamt`, or with SIGSEGV, and the library unit was
  empty each time. There is no output to reproduce, so the refusal stays; the
  upstream behaviour is pinned by `powr_cpm_lib3_upstream_writes_no_library`.
  Record: `verification_and_validation/deck_driver/README.md` §39a.
- A **blocked-binary GENDF** (`ngendf < 0`) is refused: convert it with MODER.
- `lib = 2` stops, as upstream does, when the GENDF label (GROUPR's title) does
  not read as six `e11.0` numbers: `therm` reads it with `contio`.
- Delayed nu with no MF=5 MT=455 is refused (`nldla` undefined upstream).
- EPRI-CELL/EPRI-CPM are proprietary EPRI products OUTRAM PARK does not target;
  the output format is defined by the consuming codes' documentation, not by ENDF.

## References

- NJOY2016 manual §POWR (LA-UR-17-20093)
- `powr.f90` (NJOY2016.80, `ac5adf5`); EPRI-CELL / EPRI-CPM (proprietary, EPRI)
