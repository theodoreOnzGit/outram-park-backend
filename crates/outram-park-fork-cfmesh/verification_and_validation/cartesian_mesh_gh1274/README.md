# cfMesh `cartesianMesh` as upstream: V&V record (gh:#1274)

**Research, education and V&V only.** Not for reactor operation, licensing or
safety decisions (`RESPONSIBLE_USE.md`). AI-written port, not yet reviewed by
a human.

**Upstream:** `github.com/DAFoam/cfmesh` at `e944f6ba695f` (cfMesh v1.1 line),
`cartesianMeshGenerator.C` (`generateMesh`) and `executables/cartesianMesh`.

**Port:** `pipeline::cartesian_mesh` (the workflow, step table in its module
docs) and `premap_vertices` (`meshSurfaceMapper::preMapVertices` and
`faceMetricInPatch`, ported here because `mapMeshToSurface` calls it).
`layers::refine_boundary_layers::points_in_bnd_layer` is new
(`pointsInBndLayer`). Everything else is the earlier tracks' code.

## What upstream does that I checked before predicting

- `cartesianMeshGenerator` does **not** call `surfaceMorpherCells` (grep of
  `meshLibrary/`: only `tetMeshGenerator` and `voronoiMeshGenerator` do). Its
  `surfacePreparation` is the four connectivity checks (gh:#1268). So the
  morpher's −22 % volume on the carved sphere (gh:#1267) is **not on this
  route**.
- `generateBoundaryLayers` runs `boundaryLayers::addLayerForAllPatches`
  **unconditionally**: every cartesianMesh gets one layer, with or without a
  `boundaryLayers` dict. Only `refBoundaryLayers` needs the dict.
- `addLayerForAllPatches` is patch-wise (`patchWiseLayers_ = true`): patches
  separated by feature edges are separate groups with their own vertices
  (`otherVrts_`). The port has the one-group route only (gh:#1270). On the
  sphere (one patch) the two are the same; on the pipe they are not.
- `optimizeBoundaryLayer` acts only with `optimiseLayer` in the dict
  (default off). `projectSurfaceAfterBackScaling` returns at once without
  `anisotropicSources`.
- The template keeps only `INSIDE` leaves that hold no triangle
  (`keepCellsIntersectingBoundary` off), so its boundary lies inside the
  surface by up to one leaf.

## Predictions (written 2026-11-05, before any run)

Setup S: sphere R 3 m, 24 x 48 UV (analytic 113.0973 m³), `maxCellSize`
0.5, no other settings. Setup P: pipe R 10 mm, L 60 mm, 48 sides,
walls/inlet/outlet, `maxCellSize` 2 mm. Setup F: the Foam Studio pipe,
L 200 mm, `maxCellSize` 5 mm. Baselines at the same cell size:
`carve_box` + `snap_to_surface`, and `surface_to_tet_dual_mesh` (defaults,
no layers).

- **C1.** S template: every cell a hex; volume below the analytic value by
  10–25 % (the triangle-holding leaves are dropped).
- **C2.** `surfaceTopology` on S removes at most 2 % of the cells; a
  uniform octree on a smooth convex surface has few irregular connections.
- **C3.** `preMapVertices` (3 iterations) moves every boundary point (a
  patch is found everywhere) and leaves the mean distance to the surface
  between 1/8 and 1/2 of its start (`premap_vertices` test).
- **C4.** The final S mesh is closed (`validate()`), and every cell that
  touches no boundary point is a hex (the plan's test).
- **C5.** Right after the layer (before the optimiser), some S cells are
  non-positive: the caveat that the cfMesh one-group layer inverts cells on
  curved walls (56 / 304 on the pipes, gh:#1270). I predict 1–200 here, and
  0 non-positive cells after `meshOptimisation`.
- **C6.** Volume error on S: |error| < 2 % (at most the tet-dual's
  uniform-0.5 m 1.72 % plus a little). The layer itself moves the volume by
  less than 1e-6 relative (ablation `without_boundary_layer`).
- **C7.** `carve_box` + `snap` gives a larger |volume error| than
  `cartesian_mesh` on S, and non-positive cells or worse skewness (the snap
  has no untangling).
- **C8.** P and F: closed, 0 non-positive cells at the end, |volume error|
  < 1 %. Patches inlet / outlet / walls after `edgeExtractor`.
- **C9.** With a `boundaryLayers` dict (`nLayers` 3, ratio 1.2):
  `refineBoundaryLayers` splits the S layer (every layer cell has one base
  face) and the mesh stays closed. On P, the rim cells are layer cells for
  two base faces, so the port refuses (`NotPorted`, the known caveat) and
  the layer stays one cell thick; the workflow still finishes.
- **C10.** Stop after `edgeExtraction`, restart: the uninterrupted mesh bit
  for bit; two runs bit for bit.
