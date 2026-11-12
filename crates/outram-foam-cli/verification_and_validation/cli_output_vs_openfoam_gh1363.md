# `outram-foam-cli` written files vs ESI v2506 (gh:#1363, row 7)

**Research, education and V&V only.** Not for reactor operation, licensing or
safety decisions (`RESPONSIBLE_USE.md`). AI-run; not yet reviewed by a human.

Maintainer direction: "ESI preferred". Where the port's output follows Foundation
(or its own form) and ESI differs, an ESI writer option is added and the existing
form stays callable.

## Which binaries write OpenFOAM-format files (read from `src/bin/*.rs`, 2026-11-12)

| binary | writes | upstream run here |
|---|---|---|
| `block_mesh` | `constant/polyMesh/{points,faces,owner,neighbour,boundary}`, `cellZones` | ESI v2506 `blockMesh` (built) |
| `poly_dual_mesh` | the same five files, in a new time dir or `-overwrite` | ESI v2506 `polyDualMesh` (built) |
| `pimple_foam` | `<endTime>/{U,p}` (no `phi`, no `uniform/time`) | ESI v2506 `icoFoam` (built for this row) |
| `rho_central_foam` | `<endTime>/{p,rho,e,U}` | see below |
| `ideas_unv_to_foam` | polyMesh | ESI `ideasUnvToFoam` (not built) |
| `snappy_hex_mesh`, `cartesian_*`, `p_mesh`, `tet_mesh` | polyMesh (snappy) / stubs | snappyHexMesh built; not on the #1363 list |
| `rho_pimple_foam`, `sonic_foam`, `gen_foam` | nothing: their sources say "honest stub, not case-wired" | n/a |

## Predictions (written BEFORE any port output was diffed against ESI)

Made from reading `PolyMesh::write` / `header_str` / `fmt_scalar` in
`outram-foam-basic-lib/src/io` and the ESI files from `blockMesh` on
`tutorials/incompressible/icoFoam/cavity/cavity` (ESI output read, port output
not yet produced).

### block_mesh on the cavity tutorial `system/blockMeshDict`
- P1. `faces`, `owner`, `neighbour` DATA lines (integers) are identical; the
  entry counts (882 points, 1640 faces, 760 internal) agree.
- P2. `boundary` entries agree on name, `type`, `nFaces`, `startFace`; ESI adds
  `inGroups 1(wall);` / `1(empty)` lines that the port omits.
- P3. `points` values differ in format only: ESI writes 6 significant digits
  (`writePrecision 6`, `%g`); the port writes the shortest round-trip form. For
  the uniform cavity grid I predict most lines identical and a minority (a
  double that is not the shortest form of its 6-digit decimal, e.g. 0.035 as
  0.034999999999999996) different: 1 to 30 % of the 882 lines.
- P4. Headers differ in every file: banner text (`OUTRAM PARK` vs `OpenFOAM: The
  Open Source CFD Toolbox` / `Version: 2506`), no `arch "LSB;label=32;scalar=64"`
  line, no `note "nPoints:..."` line in `owner`/`neighbour`, and the blank-line
  count around the data (ESI two blank lines after the separator and two before
  the footer, port one and zero).
- P5. Whole-file byte equality with the existing writer: 0 of 5 files.
- P6. After adding an ESI writer option (ESI banner, `arch`, `note`, `inGroups`,
  two-blank-line layout, `%g` at `writePrecision`): 5 of 5 files byte-identical
  on this case. A residual, if any, would be in `points` digits; I predict none.

### block_mesh at `writePrecision 17`
- P7. Points are bit-identical in value (established by the mesh crate's earlier
  tests), so the ESI writer at precision 17 matches bytes too: `%.17g` of equal
  doubles is equal text.

### poly_dual_mesh (`-upstream -flavour esi`)
- P8. Values are bit-identical (recorded by the mesh crate for `-flavour esi`);
  the existing writer differs from ESI in the P4 ways. With the ESI writer
  option: all five files byte-identical.

### pimple_foam on the cavity tutorial dictionaries
- P9. As shipped (`tolerance 1e-06`, `relTol 0.05` for p): the port does not
  read per-field fvSolution tolerances (gh:#1291), so `U`, `p` at 0.5 s agree
  with ESI icoFoam only to about 1e-3 of the field maximum, and about 95 % of
  the 6-digit numbers differ. Not byte-comparable.
- P10. With the dictionaries tightened to 1e-14 on both sides (row 1's
  protocol): fields agree to about 1e-13 of max; at 6 significant digits at most
  0 to 5 of 1200 printed numbers differ, each by one unit in the last place.
- P11. The file SET differs: ESI writes `phi` and `uniform/time`; the port
  writes neither.
- P12. The `U` boundary block differs: ESI prints `noSlip` for `fixedWalls`
  (no `value` entry); the port has no `noSlip` type and prints
  `fixedValue` with `value uniform (0 0 0)`.

### rho_central_foam
- P13. Not predicted until an ESI `rhoCentralFoam` binary exists; see results.
