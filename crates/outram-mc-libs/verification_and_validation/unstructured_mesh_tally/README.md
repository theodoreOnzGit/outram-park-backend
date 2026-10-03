# Unstructured-mesh tally scoring: verification plan and record (GitHub #492)

Research, education and V&V only. AI-generated draft until reviewed
(`RESPONSIBLE_USE.md`).

**Status: NOT YET MEASURED (testing deferred by maintainer, 2026-10-03).**
The tests below are written and compile (`cargo check --release`). None has
been run, so this page states methodology and pass criteria only, and no
results.

## What is under test

`MeshKind::Unstructured(Arc<UnstructuredMesh>)` scores tallies on the
neutral mesh of `outram_blender::unstructured`, which is the same object an
FV or FE solver is built from. The scoring code is
`src/tally/mesh_unstructured.rs` (`UnstructuredMeshExt`), plus the split
branch of `src/tally/scoring.rs::score_track_length`. It mirrors OpenMC
(commit `d7d3284a`):

| OpenMC | here |
|---|---|
| `MOABMesh::get_bin` / `LibMesh::get_bin` (`src/mesh.cpp:3326`, `:3973`) | `locate` |
| `MOABMesh::bins_crossed` + `intersect_track` (`:3168`, `:3144`) | `bins_crossed` |
| `UnstructuredMesh::sample_tet` (`:992`) | `sample_in_cell` |
| `MeshFilter::get_all_bins`, track-length branch (`src/tallies/filter_mesh.cpp:60-69`) | `score_track_length` split branch |

Upstream relies on MOAB (a k-d tree and ray-triangle queries) or libMesh (a
`PointLocator`). Here these are pure-Rust equivalents:

- a uniform bucket grid over cell bounding boxes (`CellLocator`);
- a generalized-winding-number inside test on each cell's bounding fan
  triangles;
- segment-triangle intersection over the faces of candidate cells.

Two departures from upstream are documented in the module docs:

- duplicate crossing distances are removed after sorting (upstream's
  `std::unique` runs before `std::sort` and never erases);
- there is no `TINY_BIT` nudge.

Upstream `LibMesh::bins_crossed` is not implemented (`fatal_error`), so the
MOAB algorithm is used for every mesh.

## Checks, with pass criteria fixed in advance

| # | test | methodology | pass criterion | result |
|---|---|---|---|---|
| 1 | `tally::mesh_unstructured::tests::locate_finds_each_hex_and_rejects_outside` | two unit hexes in cm; points inside each, beyond the end, below the base | exact bin, `None` outside | NOT YET MEASURED |
| 2 | `...::track_length_is_split_by_length_fraction` | segments x 0.25 -> 1.75 (half in each cell), 1.5 -> 3.5 (leaves the mesh), a segment wholly inside cell 0 | fractions 0.5 / 0.5 within 1e-12; 0.25 for the leaving segment; `[(0, 1.0)]` | NOT YET MEASURED |
| 3 | `...::samples_stay_in_their_cell` | 200 samples in cell 1 | every sample locates to cell 1 | NOT YET MEASURED |
| 4 | `tally::scoring::tests::unstructured_mesh_filter_splits_track_length` | energy x unstructured-mesh flux tally, one 1.5 cm segment across both hexes | 0.75 cm in each bin within 1e-12 | NOT YET MEASURED |
| 5 | outram-blender `unstructured` unit tests (face tables, geometry, locator, 1-D column vs `create_one_d_mesh`, blockMesh / cfMesh / farrer-park round trips) | see each test's doc comment | as stated there | NOT YET MEASURED |

## Planned, not written (follow-up)

- **Structured-equivalence check.** Build a `RegularMesh` and the same grid
  as unstructured Hex8 cells. Run one eigenvalue problem with both mesh
  filters, collision estimator. Pass if the per-bin flux agrees bin by bin
  within 3 combined sigma. The structured track-length path still scores a
  whole segment at its midpoint, so the track-length comparison is expected
  to differ by that approximation and is not a pass/fail gate.
- **MGXS round trip** (the #492 verification): MC tallies on a neutral mesh,
  then per-cell MGXS, then a deterministic solve on the same mesh. The pass
  is to reproduce the MC eigenvalue and reaction rates on a simple benchmark.
  The pass band must be fixed before the run; it belongs to `nee_soon`.
- **Performance.** Point location costs one `atan2` per bounding fan triangle
  of each candidate cell (24 for a hex). It has not been timed. When it is,
  record the hardware, as this crate's `CLAUDE.md` requires.

## Known limitations

- A 2-D neutral mesh bins nothing: `locate` returns `None`, as OpenMC fixes
  unstructured meshes at 3-D.
- Mesh-surface (current) tallies are refused on unstructured meshes, as
  upstream does.
- The structured kinds keep the midpoint approximation for track length.
  Changing that would move every recorded structured-mesh tally, so it is
  left for a separate, re-measured change.
