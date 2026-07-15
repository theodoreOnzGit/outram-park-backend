# DTFR — DTF-IV format for discrete-ordinates codes

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §DTFR); upstream Fortran: `dtfr.f90`.

## Theory

DTFR writes multigroup transport tables in the **DTF-IV** card-image format —
designed for the early DTF-IV Sₙ code and still accepted as an input option by
many discrete-ordinates and diffusion codes. It reorganises GROUPR **GENDF**
multigroup data into the DTF `sig` table layout: `itabl` positions per group,
`ng` groups, ordered high energy → low energy (DTF group `jg = ng - ig + 1`).

The scattering source is packed into a **reduced-length, up-scatter-capable**
band: a transfer from group `jg` to secondary group `jg2` lands at table position
`ipingp + (jg2 - jg)` — the in-group position `ipingp` when `jg2 == jg`, later
positions for down-scatter, earlier for up-scatter — with fold-back clamps that
lump anything past `itabl` into the last position and keep the scatter band above
the cross-section positions (`<= iptotl`). The P₀ absorption reaction is computed
as *total − scattering*, ν·σ_f and χ come from the fission matrices, and edits are
ENDF cross sections or linear combinations thereof.

## Ported vs. NotPorted

**Ported (self-contained, unit-tested):**

- **Input deck** (`input.rs`) — the free-format card sequence from `ruin`
  (`dtfr.f90:75-133, 590-767`): `DtfrInput`, the `iprint`/`ifilm`/`iedit`
  selector enums, `EditSpec`/`MaterialDesc`/`NeutronTables`, the **CLAW standard
  edit tables** (`CLAW_MTED`/`JPED`/`MULTD`/`HMTID`, `dtfr.f90:603-621`), and the
  three appended standard edits (`dtfr.f90:745-754`).
- **DTF table layout + scatter packing** (`table.rs`) — group ordering
  (`dtf_group`), the flat `sig(jpos + itabl*(jg-1))` index, and the **reduced Sₙ
  transfer-matrix (triangular) packing** with the two fold-back clamps
  (`scatter_position`, `dtfr.f90:410-416`) and the P₀ transport/absorption
  correction (`add_scatter_record`, `dtfr.f90:421-424`).
- **DTF card/line formatting** (`format.rs`) — the `dtfout` line layout
  (`dtfr.f90:769-946`): the `1p,eW.D` field (`fortran_e`), the format-0 header +
  six-per-line body, and the td6/CLAW block packer (`pack_dtf_block`) with
  final-line zero-fill and per-line sequence labels.

**NotPorted (documented gaps):**

- **GENDF tape reader** — the `contio`/`listio`/`moreio` walk of a GROUPR tape
  and the MF/MT accumulation dispatch (`dtfr.f90:181-553`). The ported kernels
  consume an in-memory `table::DtfTable` instead.
- **Plotting** — `ploted`/`plotnn`/`plotnp` viewr/PostScript streams
  (`dtfr.f90:948-1507`) are **permanently out of scope** for this port.
- `driver::run` documents the full pipeline and returns
  `NjoyError::NotPorted("dtfr::run")`; the registry entry `dtfr::run()` returns
  `NjoyError::NotPorted("dtfr")`. No table is fabricated.

## Provenance

- Upstream: NJOY2016 `src/dtfr.f90`, git commit
  `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`.
- NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
  this derivative is GPL-3.0-only, a modified non-LANL version not endorsed by
  LANL/DOE. See crate-root `LICENSE.njoy` + `NOTICE`.

## Testing (V&V status)

**Methodology.** Selector round-trips vs `dtfr.f90:84-87`; the CLAW standard
geometry and the four length-50 constant tables vs `dtfr.f90:603-684`; the three
standard edits vs `dtfr.f90:745-754`; group ordering and `sig` indexing vs
`dtfr.f90:342-346`; the scatter-band positions (in-group / down / up / fold-back)
and the absorption correction vs `dtfr.f90:410-424`; and the `dtfout` field/line
layout (field width, six-per-line, zero-fill, sequence numbers) vs
`dtfr.f90:790-856`.

**Results (2026-07-15, commit ac5adf5).** All 14 DTFR unit tests pass (run in an
isolated harness with identical sources because the in-crate `cargo test` build
was blocked by *other* modules under concurrent edit — see the handoff notes):

- `input`: 4/4 — selectors round-trip; `claw_defaults(5,30)` →
  `iptotl=46, ipingp=47, itabl=76, ned=48`, `edit_names.len()=43`,
  `edits.len()=48` with the `mt=443` double-entry (`mult` 0 then 1);
  standard edits absorp/nusigf/total at 44/45/46; constant tables length 50.
- `table`: 5/5 — `dtf_group(30,30)=1`; `linear_index(3,4)=32`; in-group at
  `ipingp=47`, down-scatter at `48`, up-scatter clamped to `iptotl+1=47`,
  far down-scatter folded to `≤ itabl`; absorption `sig(iptotl-2,jg) = -(Σ
  transfers)` for P₀, none for P₁.
- `format`: 4/4 — `fortran_e(12345.6,4,12)="  1.2346E+04"`,
  `-0.05→" -5.0000E-02"`; format-0 body of 15 values → 3 lines (72/72/36 chars);
  block packer zero-fills 4 tail fields and numbers sequences 1,2; `column`
  extraction `[12,22,32]`.
- `driver`: 1/1 — `run` → `NotPorted("dtfr::run")`.

## Caveats / what a human must verify

- **Untrusted AI draft.** The scatter-band clamps and absorption correction are
  the load-bearing physics-layout logic; confirm against a golden DTF table once
  the GENDF reader lands.
- **Fortran `write` byte fidelity.** `fortran_e` reproduces the signed
  2-digit-exponent `eW.D` field, but exact column-for-column agreement with a
  specific Fortran compiler's `1p,e12.4/e12.5` output (rounding mode, leading
  spaces) must be checked in a card-for-card golden comparison.
- Full format-1 (td6/CLAW) *edit-selection* branching (`dtfr.f90:820-938`, which
  positions to emit) is represented via the `pack_dtf_block` primitive; the
  per-position selection loop itself is not yet wired (needs the GENDF reader).

## References

- NJOY2016 manual §DTFR (LA-UR-17-20093)
- `dtfr.f90` (NJOY2016, commit ac5adf5); DTF-IV Sₙ code
