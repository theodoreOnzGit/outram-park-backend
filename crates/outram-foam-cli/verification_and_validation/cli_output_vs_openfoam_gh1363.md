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

## Method (as run, 2026-11-12, release)

- Upstream: ESI OpenFOAM-v2506 `blockMesh`, `polyDualMesh`, `ideasUnvToFoam`
  (built for this row, 11 s) and `icoFoam` (built for this row, 19 s) from
  `/home/user/upstream-build/OpenFOAM-v2506`, `writeFormat ascii`.
- Port: the `outram-foam-cli` binaries, default layout and `-flavour esi`.
- Harness (scratch, not committed): shell loops that copy a tutorial's `system/`
  into two directories, run ESI in one and the port in the other, and `cmp` every
  file of `constant/polyMesh` (or of the time directory).
- Committed references (gzipped ESI output, about 280 KB) and the test that pins
  them: `tests/cases/esi_v2506/`, `tests/esi_output_parity_gh1363.rs` (14 tests).
- `icoFoam` protocol (row 1's): the shipped `fvSolution` with `tolerance 1e-14`
  and `relTol 0` on both sides, because the port's linear solvers have no
  relative tolerance (gh:#1291). The as-shipped dictionaries were run too and
  are reported separately.

## Results

### Prediction scorecard

| # | predicted | measured | verdict |
|---|---|---|---|
| P1 | faces/owner/neighbour data lines identical | identical (data lines compared after dropping headers) | held |
| P2 | boundary agrees except `inGroups` | held | held |
| P3 | 1 to 30 % of `points` lines differ in the default writer | **738 of 882 (84 %)**: the port's coordinates carry round-off (`0.005000000000000001`) that shortest-round-trip printing shows and ESI's 10-digit print hides | MISSED. The premise was also wrong: ESI `blockMesh` prints at least 10 digits, not 6 (`IOstream::minPrecision(10)`, `blockMesh.C:293`), found only when a coordinate such as 1.333333333 appeared |
| P4 | headers differ in banner, `arch`, `note`, blank lines | held | held |
| P5 | 0 of 5 default-layout files identical | 0 of 5 | held |
| P6 | 5 of 5 identical with an ESI option | first run **4 of 5** (my `boundary` had two blank lines where ESI has one); 5 of 5 after the fix | missed once, fixed |
| P7 | precision-17 points match | not run for `blockMesh`; polyDualMesh was run at `writePrecision 17` on three cases and matched | not tested as stated |
| P8 | polyDualMesh ESI option: 5 of 5 | **5 of 5 on four cases (cavity at precision 6; lshape, annulus, warped at precision 17): 20 of 20** | held |
| P9 | tutorial tolerances: about 95 % of printed numbers differ | U 368 of 400 lines, p 393 of 400 (92 to 98 %) | held |
| P10 | tight: 0 to 5 of 1200 numbers differ | after the defects below were fixed: **U 0, phi 0, uniform/time 0, p 1** (the pinned reference cell, `6.02365e-16` vs `-5.87933e-15`, both zero to round-off) | held, but only after fixing the defects; the first run differed in U and p entirely |
| P11 | phi, uniform/time not written | held; both are now written under `-flavour esi` | held |
| P12 | `noSlip` printed as `fixedValue` | **not observed**: the field reader keeps `noSlip` and the writer prints it | MISSED |

### Per-binary table

| binary | ESI program | cases | byte-identical | what differs |
|---|---|---|---|---|
| `block_mesh -flavour esi` | blockMesh | cavity; two-zone block at 3 sizes | all files (5, then 6 with `cellZones`) | nothing |
| `block_mesh -flavour esi` | blockMesh | 449 tutorial `blockMeshDict`s | **213 identical, 84 differ, 113 the port cannot parse, 39 ESI cannot run as copied** | see the sweep below |
| `poly_dual_mesh -upstream -flavour esi` | polyDualMesh | cavity, lshape, annulus, warped | 20 of 20 files | nothing |
| `pimple_foam -flavour esi` | icoFoam | cavity, tight tolerances | U, phi, uniform/time; **p except one line** | the pinned reference cell (round-off noise) |
| `pimple_foam -flavour esi` | icoFoam | cavity, as shipped | none of U, p | the solution (gh:#1291) |
| `ideas_unv_to_foam -flavour esi` | ideasUnvToFoam | 8 ESI sample `.unv` | 0 of 8 whole cases | conversion content (point order, face groups), not layout; layout pinned on the port's own cube only |
| `rho_central_foam` | rhoCentralFoam | shockTube | not run | the tutorial ships `0.orig` (needs setFields) and the CLI wants `0/e` and `0/rho`; no ESI binary built; row 2 covers the solver with a driver |
| `rho_pimple_foam`, `sonic_foam`, `gen_foam` | n/a | n/a | n/a | their sources say they write nothing (case-wiring stubs) |

### Sweep of the 449 tutorial `blockMeshDict`s (`-flavour esi`, ascii, 2026-11-12)

Counts of `find -name blockMeshDict -path '*system*'` under the v2506 tutorials.
Before the `defaultFaces` fix: 144 identical, 153 differ, 113 port parse
failure, 39 ESI failure (with `writeFormat` left as shipped); after the fix and
with `writeFormat ascii` forced: **213 / 84 / 113 / 39**. Classes of the 84 that
differ (first differing line; counts are approximate and overlap):

| class | cases | cause | status |
|---|---|---|---|
| `symmetryPlane` patches written as `symmetry` | 19 | `PatchKind` has no `symmetryPlane`; the type word is lost | OPEN: needs a `PatchKind` variant or the original type kept on `BoundaryPatch` (many `match` sites) |
| extra patch entries dropped (`overset`, `mappedPatch`, `cyclicAMI` with `neighbourPatch`, `matchTolerance`, `transform`) | about 35 | the mesh crate reads only name, type and faces | OPEN |
| point coordinates differ in the last digit, or `-5.9e-19` vs `0` | about 15 | arc/projected edges: values not bit-identical (straight and graded blocks are) | OPEN (mesh crate) |
| `format binary` in the upstream file | 14 | the harness `sed` did not reach those `controlDict`s | not a port difference |
| one extra patch `defaultFaces` | 5 (before) | a declared `defaultFaces { faces (); }` was written empty plus a second default patch | FIXED here |
| other | rest | not examined individually | OPEN |

### Defects found by running the shipped dictionaries (fixed here, each reached by a test)

1. `fvSolution` `pFinal { $p; relTol 0; }` made the case unreadable (`empty
   entry value`): the dictionary parser had no `$name;` macro. Added.
2. `constant/transportProperties` was ignored: `pimple_foam` ran the shipped
   cavity at the solver's default viscosity. Now read (Newtonian `nu`).
3. `system/fvSchemes` `div(phi,U)` was ignored (default upwind where the
   tutorial says `Gauss linear`); `fvSolution` tolerances were ignored (partly
   closes gh:#1291: only entries with `relTol 0`).
4. The written `U` carried `object HbyA;` (the solver reassigns `U` from the
   `HbyA` expression and the field kept that name); the default layout had the
   same defect. Files are now named by their file name.
5. `blockMesh`: a declared patch named like the default patch took no faces and
   a second default patch was added.

### ESI options added (existing forms stay callable)

- `outram_foam_basic_lib::io::esi`: `EsiWriteOptions`, `format_g` (C++ `%g`),
  `esi_header`, `PolyMesh::write_esi`, `write_cell_zones_esi`,
  `write_time_file_esi`; `io::field::{write_vol_scalar_field_esi,
  write_vol_vector_field_esi, write_surface_scalar_field_esi}`. The list rule
  (`N{v}`, inline up to 10 elements, else long form) and blockMesh's
  `minPrecision(10)` are implemented; `write_vol_*_field` and `PolyMesh::write`
  are untouched in behaviour.
- CLI: `blockMesh`, `polyDualMesh` (`-writeFlavour`, which follows `-flavour`),
  `ideasUnvToFoam`, `pimpleFoam`, `rhoCentralFoam`: `-flavour default|esi`.
- `CaseRun::write_fields_esi`.

### Differences accepted, with reasons

- The `p` reference cell: both codes set `p[pRefCell] = 0` to round-off; the
  printed noise is not a model quantity. Pinned: at most that one line, both
  values below 1e-12.
- The ESI banner is reproduced only under `-flavour esi`; the default banner
  keeps naming this project. A file carrying the ESI banner was not produced by
  OpenFOAM: that is the price of byte identity and the reason it is an option.

### Not done

`rhoCentralFoam` (above); `snappyHexMesh` and the cfMesh binaries (not on the
list; their writers share `PolyMesh::write`); the ESI layout of a non-uniform
boundary `value` in the field writers (unverified; the cavity has none);
`faceZones` and `pointZones` in ESI layout; the OPEN classes of the sweep; the
UNV converter's content differences; the as-shipped `relTol` (gh:#1291).
