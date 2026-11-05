# cfMesh `tetMesh` as upstream: V&V record (gh:#1272)

**Research, education and V&V only.** Not for reactor operation, licensing or
safety decisions (`RESPONSIBLE_USE.md`). AI-written port, not yet reviewed by
a human.

**Upstream:** `github.com/DAFoam/cfmesh` at `e944f6ba695f` (cfMesh v1.1 line).

## Predictions (written 2026-11-05, before any run)

Setup A, sphere: R 3 m, 24 x 48 UV surface (analytic 113.0973 m^3),
`maxCellSize` 0.5 (the octree root is 16 m, so leaves are exactly 0.5 m),
no `boundaryCellSize`. Setup B, pipe: R 10 mm, L 60 mm, 48 sides, patches
walls/inlet/outlet, `maxCellSize` 2 mm. Baseline at equal cell size: the
crate's `carve_box` + `snap` + `tetrahedralize` + `flip_to_delaunay`
(`surface_to_tet_dual_mesh` with `dual: false`, no layers).

1. `tetCreatorOctree` on a uniform octree (setup A): no sub-nodes, no face
   centres, no split edges; every tet is "around an edge" (two leaf centres
   and an octree edge: the BCC lattice). Every tet has positive volume. No
   read upstream leaves undefined (all `TetCreatorDefects` zero).
2. The extracted tet mesh is closed (`validate()`), every face in at most
   two tets. Its volume before mapping equals the volume of the `MESHCELL`
   leaves minus one pyramid `h^3/6` per leaf face on the region boundary,
   to 1e-12 relative.
3. On the graded pipe (B with `boundaryCellSize` 1 mm) the split-edge and
   face-centre tets appear, still all positive, defects zero.
4. Full workflow (A): all tets positive and the mesh closed. Volume error
   against 113.0973: I predict a **miss** of the README's uniform 0.5 m
   figure (1.72 %), because `surfaceMorpherCells` / `topologicalCleaner`
   (topology track) and `meshSurfaceOptimizer::untangleSurface` (surface
   track) are not ported: the stand-in reverts tangled boundary moves, so
   part of the boundary stays on the octree, outside the surface. Expected
   between 1.72 % and 8 %.
5. Tet count (A) between 1.0 and 1.6 times the baseline's.
6. Before mapping, the octree tets' max non-orthogonality is below the
   baseline's (BCC tets are near-regular; centroid-subdivision tets are not).
7. Run time: `tet_mesh` slower than the baseline (the optimiser with
   boundary moves and the LP untangler runs; the baseline has no optimiser).
8. `renumberMesh` leaves the internal faces upper-triangular; the patches
   keep their faces.

## Results

(Filled in after the runs; see below.)
