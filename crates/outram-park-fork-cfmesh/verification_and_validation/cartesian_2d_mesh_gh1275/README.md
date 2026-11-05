# cfMesh `cartesian2DMesh`: V&V record (gh:#1275)

**Research, education and V&V only.** Not for reactor operation, licensing or
safety decisions (`RESPONSIBLE_USE.md`). AI-written port, not yet reviewed by
a human.

**Upstream:** `github.com/DAFoam/cfmesh` at `e944f6ba695f`.

## What is ported

| upstream | crate |
|---|---|
| `cartesian2DMeshGenerator`, `executables/cartesian2DMesh` | `pipeline::cartesian_2d_mesh` |
| the quadtree in `meshOctree` (`isQuadtree_`, `refineCube2D`, negative `posZ`, the 2D neighbour searches, `findEdgeCentre`) | `mesh_octree`, `mesh_octree_queries`, `cartesian_mesh` |
| `cartesianMeshExtractor`'s empty patches | `cartesian_mesh::create_empty_patches` |
| `polyMeshGen2DEngine` | `poly_mesh_gen_2d_engine` |
| `meshSurfaceMapper2D` (`adjustZCoordinates`, `preMapVertices`, `mapVerticesOntoSurface`, `mapVerticesOntoSurfacePatches`, `mapCorners(edges)`, `findMappingDistance`) | `surface_mapper_2d` |
| `meshSurfaceEdgeExtractor2D` (`distributeBoundaryFaces`, `remapBoundaryPoints`) | `surface_mapper_2d` |
| `triSurfaceExtrude2DEdges`, `triSurface2DCheck::is2DSurface` | `tri_surface_2d` |

**Reused from the 3D workflow, as upstream reuses it:**
`cartesianMeshExtractor`, the four connectivity checks, `boundaryLayers`
(the one-group route, with the empty patches untreated as 2D mode leaves
them), `renumberMesh`, `renameBoundaryPatches` and `workflowControls`.

**Not ported (each is a note in the report):**

- `meshSurfaceOptimizer::optimizeSurface2D` and `untangleSurface2D` (the
  `optimiseMeshSurface` step, which runs twice);
- `refineBoundaryLayers::activate2DMode`;
- the patch-wise `boundaryLayers` groups;
- `triSurfacePatchManipulator` and `triSurfaceCleanupDuplicateTriangles`;
- `anisotropicSources`;
- the no-argument `mapCorners()` (nothing calls it);
- MPI;
- `utilities/extrudeEdgesInto2DSurface` (a `main` around the extruder);
- `triSurface2DCheck::createSubsets`.

## Predictions (written 2026-11-05, before any run)

Channel: outline `[0, 1] x [0, 0.25]`, extruded by `triSurfaceExtrude2DEdges`
(height 0.1 x the outline's diagonal, 0.10308 m). Regions: walls (y = 0,
y = 0.25), inlet (x = 0) and outlet (x = 1). `maxCellSize` 0.05. The 3D
comparison is the same channel as a closed box with z in `[0, 0.5]` and the
front and back in `frontAndBack`, meshed by `cartesian_mesh`.

- **D1.** The extruded rectangle passes `is2DSurface`. A sphere and a
  tilted ribbon fail it.
- **D2.** Every quadtree leaf has `posZ = -10` and spans the root box in z.
  The template is one cell thick: each cell has exactly one face in each
  empty patch. `polyMeshGen2DEngine` builds (half the points at z min).
- **D3.** The final mesh is closed. Every point lies **exactly** on z = 0
  or z = 0.10308 (`adjustZCoordinates`), half on each. 0 non-positive
  cells. Patches: walls, inlet, outlet, bottomEmptyFaces, topEmptyFaces,
  with the last two `empty`.
- **D4.** The template's x-y layout is **identical** (every node to 1e-9)
  to the 3D template's nodes on its mid-plane z = 0.25. The root boxes
  have the same x-y extent (both are set by L = 1), and the 3D leaves
  are the same in x-y at mid-height.
- **D5.** The final meshes' x-y layouts are **not** identical. The 3D
  layer is grown on all six sides as one group, the 2D layer only on the
  side patches, and the 3D mesh is also optimised. Predicted: at least
  half the 2D nodes still coincide with 3D mid-plane nodes (the interior
  ones).
- **D6.** The area (volume / thickness) is within 1e-9 of L x H = 0.25:
  the outline is axis-aligned, so mapping is exact, and the layer
  conserves the volume.
