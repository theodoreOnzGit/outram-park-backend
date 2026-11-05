# OpenFOAM's upper-triangular face order: V&V record (gh:#1285)

Research, education and V&V only (see `RESPONSIBLE_USE.md`).

## What changed and where

OpenFOAM's `polyMesh` requires the internal faces in **upper-triangular
order**: `owner < neighbour` on every internal face, and the internal faces
sorted by owner, then by neighbour (`polyMeshGenChecks::checkUpperTriangular`,
ported as `poly_mesh_gen_checks::check_upper_triangular`, gh:#1261). The
tet-dual pipeline and `cartesian_box` did not produce it (`cartesian_box`
numbers its internal faces by direction; the dual, layer and mapping
stages rebuild meshes from cell face lists).

The reorder is upstream's own: steps 2–3 of
`polyMeshGenModifier::renumberMesh` (`renumber_mesh.rs`, gh:#1272), split
out as `renumber_mesh::reorder_faces` and called with the **identity cell
order** as `renumber_mesh::upper_triangular`. Step 1 of `renumberMesh`
(the cell walk) is not applied: cell labels stay as the pipeline made
them, so nothing indexed by cell changes meaning.

**Where.** One final step, not in each builder:

- `pipeline::run_pipeline` (both `surface_to_tet_dual_mesh` entry points,
  and the box / sphere / cylinder wrappers), after the patch naming
  (stage 8) and before the final gate;
- `cartesian::cartesian_box`, at its end (it is a public builder that does
  not go through the pipeline).

Why one final step: every stage of the tet-dual pipeline after the carve
rebuilds its mesh from cell face lists (`from_cell_faces`), so an order
imposed inside a builder is lost at the next stage; and the stage set
changes with the options (dual kinds, mapping, `optimiseFinalMesh`, three
layer strategies, patch naming). A final step guarantees the order on every
option combination with one call, costs one O(F) pass, and leaves every
stage's internals (and their own unit tests) as they were. The four
upstream workflows (`tet_mesh`, `p_mesh`, `cartesian_mesh`,
`cartesian_2d_mesh`) already end with the full `renumberMesh` and are not
touched.

## Predictions (written 2026-11-05, before the change was run)

1. **Geometry is unchanged.** Points are not touched. Boundary faces keep
   their order, ring and patch, so the **total volume** (the divergence sum
   over boundary faces) is **bit for bit** the same. Cell labels are kept,
   so the cell volumes can be compared **cell by cell**, not only as a
   multiset; face areas as a multiset.
2. Refinement of 1, stated before measuring: a reversed internal face
   (owner > neighbour before the reorder) has its ring reversed after its
   first point, so Newell's sum runs in another order, and each cell's
   volume sums its faces in another order. Expected: cell volumes and face
   areas equal **to round-off** (relative difference below 1e-12, most of
   them bit-identical), not necessarily bit for bit.
3. `check_upper_triangular` fails on the pre-change default outputs (pipe
   h 4/5/6 mm, Foam Studio's default mesh) and passes on every output after
   it. `checkMesh`'s other topology checks are unchanged.
4. The pinned fingerprints that hash face order move:
   `back_off_only_layers_are_pinned_bit_for_bit`,
   `dual_kinds_are_pinned_bit_for_bit`, `octree_path_is_pinned_bit_for_bit`,
   `layer_strategies_are_deterministic`,
   `foam_studio_default_mesh_keeps_its_fingerprint`, and the Foam Studio
   example pins; cell counts do not move.
   `correct_edges_is_pinned_bit_for_bit` hashes a stage output built on
   `cartesian_box`, so it moves too.
5. The Foam Studio solve on the default mesh: the FV operators sum face
   fluxes into cells in face order, so a reorder changes the summation order
   and the result moves **at round-off only**: relative change in the pinned
   pressure drop / velocity below 1e-10.

## Results

(Filled in after the runs.)
