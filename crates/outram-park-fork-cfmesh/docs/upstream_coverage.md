# cfMesh upstream coverage

Audit for gh:#1251, step 1. Written 2026-11-05 against branch `claude/outram-foam-8ookor`.

**Upstream:** `github.com/DAFoam/cfmesh` at `e944f6ba695f` (cfMesh v1.1 line). It has 129,738 lines of C++ (`.C` and `.H`) in `meshLibrary/`, `executables/` and `utilities/`. `tutorials/`, `testingInterfaces/` and `python/` are not counted.

**Crate:** `crates/outram-park-fork-cfmesh/src`, 24 files, 14,994 lines of Rust.

## How this was checked

- **Upstream lines:** `wc -l` of every `.C`/`.H`, summed per leaf directory. Where a directory mixes serial and MPI files, the MPI files are a separate row. The row totals add up to 129,738.
- **Rust side:** I read the provenance header and module docs of all 24 files. I listed every `fn` in `mesh_optimizer`, `boundary_layer_optimisation`, `layers`, `correct_edges`, `surface_mapper`, `checks`, `smooth`, `octree` and `dual`. I grepped every Rust file for each upstream class name and for "not ported".
- **Upstream side:** I read the four generators' `generateMesh` and `tetMeshGenerator`'s steps, `optimizeMeshFV.C`, the boundary half of `tetMeshOptimisation.C`, and `volumeOptimizer::optimizeNodePosition`. I listed the member functions of `meshOptimizer`, `tetMeshOptimisation`, the advanced smoothers, `meshSurfaceOptimizer`, `partTriMesh` and `polyMeshGenChecks`. "No caller" claims come from a grep of `meshLibrary/`.
- **Not re-checked:**
  - The Rust ports were not diffed line by line against upstream. Where this file says `ported`, it relies on the port's own doc comments and on the function lists matching.
  - `helperFunctions` was not checked function by function (rows L).
  - `smooth`, `snap`, `carve`, `cartesian`, `octree`, `tet` and `dual` are re-implementations "from the algorithm, not transcribed" (their own headers say so). They are classed against the nearest upstream class.

**Status key.**
- `ported`: every member function that upstream's workflows call is in Rust.
- `partial`: some are. The row says exactly what is missing, and its lines are the whole upstream unit, not the share that is ported.
- `not ported`: nothing is in Rust.
- `not applicable`: the unit has no job in this crate, for the reason given.

Paths are under `meshLibrary/utilities/` unless they start with `meshLibrary/` or `executables/`, or sit in group M.

## 1. Totals

| Status | Upstream lines | Share | Rows |
|---|---:|---:|---:|
| ported | ~~3,349~~ ~~11,115~~ ~~17,537~~ ~~18,526~~ ~~36,553~~ ~~41,405~~ ~~42,349~~ ~~53,277~~ ~~56,269~~ ~~63,379~~ ~~69,591~~ 73,820 | ~~2.6 %~~ ~~8.6 %~~ ~~13.5 %~~ ~~14.3 %~~ ~~28.2 %~~ ~~31.9 %~~ ~~32.6 %~~ ~~41.1 %~~ ~~43.4 %~~ ~~48.9 %~~ ~~53.6 %~~ 56.9 % | ~~6~~ ~~12~~ ~~21~~ ~~22~~ ~~40~~ ~~41~~ ~~44~~ ~~65~~ ~~71~~ ~~85~~ ~~88~~ 89 |
| partial | ~~54,119~~ ~~48,890~~ ~~47,494~~ ~~38,266~~ ~~38,673~~ ~~35,766~~ ~~36,030~~ ~~35,781~~ ~~30,545~~ 26,316 | ~~41.7 %~~ ~~37.7 %~~ ~~36.6 %~~ ~~29.5 %~~ ~~29.8 %~~ ~~27.6 %~~ ~~27.8 %~~ ~~27.6 %~~ ~~23.5 %~~ 20.3 % | ~~27~~ ~~25~~ ~~23~~ ~~16~~ ~~17~~ ~~16~~ ~~17~~ ~~15~~ 14 |
| not ported | ~~49,628~~ ~~47,091~~ ~~42,065~~ ~~41,076~~ ~~32,277~~ ~~27,018~~ ~~26,074~~ ~~18,053~~ ~~14,797~~ ~~7,936~~ 6,960 | ~~38.3 %~~ ~~36.3 %~~ ~~32.4 %~~ ~~31.7 %~~ ~~24.9 %~~ ~~20.8 %~~ ~~20.1 %~~ ~~13.9 %~~ ~~11.4 %~~ ~~6.1 %~~ 5.4 % | ~~80~~ ~~76~~ ~~69~~ ~~68~~ ~~57~~ ~~55~~ ~~52~~ ~~32~~ ~~25~~ ~~11~~ 10 |
| not applicable | 22,642 | 17.5 % | 25 |
| **total** | **129,738** | 100 % | 138 |

**Updated 2026-11-05** after gh:#1255, #1256, #1260 and #1261, and again after gh:#1267/#1268 and gh:#1270/#1271 (~~these two tables count those items only; the octree track's gh:#1264–#1266 rows are updated in §2~~ **CORRECTED 2026-11-05, see the next paragraph:** groups A–C now count gh:#1262–#1266 too) (the struck-through numbers are the audit's; §5 has what moved). The `partial` lines overstate what is ported. They count each partly-ported unit whole: for example, the 4,441 lines of `polyMeshGenChecks` for 9 of its 34 function definitions.

**Updated again 2026-11-05** after gh:#1272 and #1273, merged with gh:#1267/#1268's counts. **CORRECTED at the same time:** these totals had not followed the rows already marked ported by gh:#1258 (`partTriMesh`, group C, 1,331) and gh:#1262–#1266 (group B, and `cartesianMeshExtractor` in group A). Groups A, B and C were recounted from their rows; group G moves by gh:#1267's `surfaceMorpherCells` and gh:#1272's `renameBoundaryPatches`; group I is gh:#1267/#1268's; group H is gh:#1270/#1271's. **Not re-checked:** groups D–M against their rows (other rows changed by gh:#1257–#1266, for example `meshSurfaceEngine`, `meshSurfacePartitioner`, `meshSurfaceCheckInvertedVertices` in G and `FrontalMarking` in L, may not be in their group's totals).

**Updated 2026-11-05 after gh:#1275** (merged after gh:#1276–#1278; the totals and groups C and J combine both). To ported: `cartesian2DMesh` (66) and `cartesian2DMeshGenerator` (536) in A, `polyMeshGen2DEngine` (689) in C, `meshSurfaceMapper2D` (1,189) and `meshSurfaceEdgeExtractor2D` (311) in G, `triSurfaceExtrude2DEdges` (201) in J; `triSurface2DCheck` (264, J) to partial. The quadtree is inside rows already counted as ported (B). Only those rows moved.

**Updated 2026-11-05 after gh:#1274:** `executables/cartesianMesh` (62) and `cartesianMeshGenerator` (580) in group A, and `meshSurfaceMapperPremapVertices.C` (302) in group G, from not ported to ported (944 lines, 3 rows). Only those rows moved.

**Updated 2026-11-05 after gh:#1269:** `edgeExtractor` (4,852) from not ported to ported, `meshSurfaceEdgeExtractorFUN` (407) from not ported to partial; both in group G. Only those two rows moved.

**Updated 2026-11-05 after gh:#1280, #1281 and #1279 (items 25–27):** to ported: `symmetryPlaneOptimisation` (405, E); `createFundamentalSheets` (908) and `meshSurfaceEdgeExtractorFUN` (407, partial before) (G); `extrudeLayer` (1,915) and `triangulateNonPlanarBaseFaces` (357) (H); the four `anisotropicMeshing` rows (2,559, K); `improveMeshQuality`, `improveSymmetryPlanes`, `scaleMesh`, `scaleSurfaceMesh`, `surfaceGenerateBoundingBox` (559, M). To partial: `generateBoundaryLayers` (158, M). Only those 15 rows moved.

**Updated 2026-11-05 after gh:#1270's remainder** (merged after the octree remainders; the totals combine both): `boundaryLayers/boundaryLayers` (4,229, H) from partial to ported (serial; its MPI branches are not applicable). Only that row moved.

**Updated 2026-11-05 after gh:#1276, #1277 and #1278 (wave 3, items 22–24 in part):** 12 rows of group J (6,749 lines: every `triSurfaceTools` row except the two 2D ones and `triSurfaceMetaData`, and `quadricFitting`), `meshes/triSurf` in group C (2,907, partial → ported) and 8 utilities in group M (1,272) are ported. Only those 21 rows moved.

**Updated 2026-11-05 after the octree remainders (gh:#1278 second half, gh:#1264–#1266 remainders):** group B's `meshOctreeCreator` (2,118) and `meshOctreeAddressing` (3,118) partial → ported, `meshOctreeAutomaticRefinement` (976) not ported → ported. Group B is now wholly ported apart from its MPI rows. Only those 3 rows moved.

**Checked 2026-11-05 after gh:#1279's remainder (`anisotropicSources` in the four generators): no row moved, so the totals stand.** The generator rows and group K were already counted as ported. Only the notes of `tetMeshGenerator`, `cartesianMeshGenerator` and `coordinateModification` changed; §5 has the details.

| Group | ported | partial | not ported | not applicable |
|---|---:|---:|---:|---:|
| A. Workflows | ~~0~~ ~~5,333~~ ~~5,975~~ 6,577 | ~~1,423~~ 0 | ~~5,154~~ ~~1,244~~ ~~602~~ 0 | 1,438 |
| B. Octree | ~~0~~ ~~10,689~~ 16,901 | ~~13,041~~ ~~5,236~~ 0 | ~~3,860~~ ~~976~~ 0 | 2,111 |
| C. Mesh data structures | ~~0~~ ~~442~~ ~~2,055~~ ~~4,962~~ 5,651 | ~~12,767~~ 9,860 | ~~3,538~~ ~~3,096~~ ~~1,483~~ 794 | 3,354 |
| D. Quality checks | ~~0~~ 4,441 | ~~4,441~~ 0 | 0 | 0 |
| E. Volume optimiser | ~~1,141~~ ~~3,599~~ 4,004 | ~~5,244~~ 4,456 | ~~2,404~~ ~~734~~ 329 | 798 |
| F. Surface optimiser | 0 | 0 | 2,811 | 522 |
| G. Surface tools | ~~2,208~~ ~~3,347~~ ~~3,739~~ ~~8,591~~ ~~8,893~~ ~~10,393~~ 11,708 | ~~4,082~~ ~~4,489~~ 4,082 | ~~10,962~~ ~~9,823~~ ~~9,431~~ ~~4,172~~ ~~3,870~~ ~~2,370~~ 1,462 | 969 |
| H. Boundary layers | ~~0~~ ~~989~~ ~~3,261~~ 7,490 | ~~9,395~~ 5,166 | ~~3,261~~ ~~2,272~~ 0 | 0 |
| I. Topology repair | ~~0~~ 5,283 | ~~1,396~~ 0 | ~~3,887~~ 0 | 0 |
| J. triSurface tools, helper classes | ~~0~~ ~~425~~ ~~7,174~~ 7,375 | ~~0~~ 264 | ~~7,639~~ ~~7,214~~ ~~465~~ 0 | 233 |
| K. Anisotropic meshing | ~~0~~ 2,559 | 0 | ~~2,559~~ 0 | 0 |
| L. Helpers, containers, MPI, I/O | 0 | 2,330 | 1,482 | 11,782 |
| M. Top-level utilities | ~~0~~ ~~1,272~~ 1,831 | ~~0~~ 158 | ~~2,071~~ ~~799~~ 82 | 1,435 |

## 2. Per-unit table

### A. Workflows, generators and executables

| Upstream | Lines | Status | Rust | Notes |
|---|---:|---|---|---|
| `executables/tetMesh` | 65 | ~~partial~~ **ported (2026-11-05, gh:#1272)** | `pipeline::tet_mesh::{tet_mesh, tet_mesh_restart}`; `pipeline::surface_to_tet_dual_mesh(_multipatch)` kept | Thin `main` over `tetMeshGenerator`. Settings are typed (`TetMeshOptions`); ~~no `meshDict` reader~~. **Binary since gh:#1476 (2026-11-11):** `outram-foam-cli`'s `tetMesh` reads `system/meshDict` (`mesh_dict::read_mesh_dict`) and the `surfaceFile` (`tri_surf_io`) and writes `constant/polyMesh` (`poly_mesh_gen_io`), through `outram_foam_cli::cfmesh`. |
| `meshLibrary/tetMesh/tetMeshGenerator` | 558 | ~~partial~~ **ported (2026-11-05, gh:#1272)** | `pipeline::tet_mesh` (the generator and `generateMesh`); `boundary_layer_optimisation`, `surface_mapper`, `pipeline` kept | Every member is in Rust. ~~Steps whose callees are other rows' gaps run through what exists and say so in `TetMeshReport::notes`: `surfacePreparation` (topology track) and `optimiseMeshSurface` (surface track) are skipped, `untangleSurface` is replaced by reverting tangled moves, `extractPatches` uses the nearest triangle.~~ **CORRECTED 2026-11-05:** `surfacePreparation`, `untangleSurface`, `optimiseMeshSurface` and `optimizeSurface` call the gh:#1267 / gh:#1259 ports; ~~only `extractPatches` (`edgeExtractor`, nearest triangle instead) is a gap, noted in `TetMeshReport::notes`~~ **UPDATED 2026-11-05 (gh:#1269):** `extractPatches` calls `edge_extractor::extract_patches` (the nearest triangle is the ablation `PatchAssignment::NearestTriangle`). ~~No `boundaryLayers` dict~~ **A `boundaryLayers` dict since gh:#1270's remainder (`TetMeshOptions::boundary_layers`)**, ~~and no `anisotropicSources`~~ **and `anisotropicSources` with `projectSurfaceAfterBackScaling` since gh:#1279's remainder**. Results: `verification_and_validation/tet_mesh_gh1272/`. |
| `meshLibrary/tetMesh/tetMeshExtractorOctree` | 386 | ~~not ported~~ **ported (2026-11-05, gh:#1272)** | `tet_mesh_extractor_octree` (with the shared `polyMeshGen` assembly `poly_mesh_from_cells`) | Beside `tet::tetrahedralize` + `delaunay::flip_to_delaunay` (§3); compared on the sphere and the pipe. |
| `meshLibrary/utilities/tetrahedra/tetCreatorOctree` | 1678 | ~~not ported~~ **ported (2026-11-05, gh:#1272)** | `tet_creator_octree` | All five tet routines. Upstream's unguarded reads are counted (`TetCreatorDefects`), zero on every run. |
| `executables/cartesianMesh` | 62 | ~~not ported~~ **ported (2026-11-05, gh:#1274)** | `pipeline::cartesian_mesh::{cartesian_mesh, cartesian_mesh_restart}` | Thin `main` over `cartesianMeshGenerator`. Settings typed (`CartesianMeshGeneratorOptions`). **Binary since gh:#1476 (2026-11-11):** `outram-foam-cli`'s `cartesianMesh` reads `system/meshDict` (`mesh_dict::read_mesh_dict`) and the `surfaceFile` (`tri_surf_io`) and writes `constant/polyMesh` (`poly_mesh_gen_io`), through `outram_foam_cli::cfmesh`. |
| `meshLibrary/cartesianMesh/cartesianMeshGenerator` | 580 | ~~not ported~~ **ported (2026-11-05, gh:#1274)** | `pipeline::cartesian_mesh`; `carve`, `snap`, `octree` kept | ~~No `cartesianMesh` workflow.~~ Every member in Rust. Gaps it calls through: ~~`boundaryLayers` is the one-group route (patch-wise groups not ported)~~ (**patch-wise since gh:#1270's remainder**); `refineBoundaryLayers` refuses two-direction cells; no ~~`anisotropicSources` /~~ `triSurfacePatchManipulator` (**`anisotropicSources` and `projectSurfaceAfterBackScaling` since gh:#1279's remainder**). Results: `verification_and_validation/cartesian_mesh_gh1274/`. |
| `meshLibrary/cartesianMesh/cartesianMeshExtractor` | 800 | ported (2026-11-05, gh:#1266) | `cartesian_mesh::cartesian_mesh_extractor`; `octree::build_mesh` kept | ~~partial: re-implemented only; missing `decomposeSplitHexesIntoTetsAndPyramids`, `createPointsAndAddressing`, cell-type bookkeeping~~ **CORRECTED 2026-11-05:** transcribed beside `octree::build_mesh`. On the same leaves the two build the same faces ring for ring (sphere and pipe, `verification_and_validation/cartesian_octree_gh1262/README.md`). ~~Quadtree empty patches not ported.~~ **2026-11-05 (gh:#1275):** the quadtree's empty patches are `cartesian_mesh::create_empty_patches`, called by the 2D generator right after the extractor. |
| `executables/pMesh` | 65 | ~~not ported~~ **ported (2026-11-05, gh:#1273)** | `pipeline::p_mesh::{p_mesh, p_mesh_restart}` | Thin `main` over `voronoiMeshGenerator`. **Binary since gh:#1476 (2026-11-11):** `outram-foam-cli`'s `pMesh` reads `system/meshDict` (`mesh_dict::read_mesh_dict`) and the `surfaceFile` (`tri_surf_io`) and writes `constant/polyMesh` (`poly_mesh_gen_io`), through `outram_foam_cli::cfmesh`. |
| `meshLibrary/voronoiMesh/voronoiMeshGenerator` | 630 | ~~not ported~~ **ported (2026-11-05, gh:#1273)** | `pipeline::p_mesh` (shares `pipeline::tet_mesh`'s generator body) | Workflow as `tetMesh`, plus `preMapVertices` (~~not ported, surface track; noted~~ **called since gh:#1274's remainder**) and a second `untangleSurface` (~~not ported~~ **CORRECTED 2026-11-05, gh:#1269:** called, `tet_mesh::untangle_again`, since gh:#1259 merged). `edgeExtraction` is `meshSurfaceEdgeExtractorFUN` ~~without its fundamental sheets (gh:#1269)~~, with them since gh:#1281. Results: `verification_and_validation/p_mesh_gh1273/`. |
| `meshLibrary/voronoiMesh/voronoiMeshExtractor` | 674 | ~~not ported~~ **ported (2026-11-05, gh:#1273)** | `voronoi_mesh_extractor`; `cell_centre_dual`, `dual::polyhedral_dual*` kept | Tet-centroid dual of `tetCreatorOctree` tets, cells only at interior vertices. Equal to `CellCentre`'s interior cells to 9.8e-15 where `polyDualMesh` accepts the tets (§3). |
| `executables/cartesian2DMesh` | 66 | ~~not ported~~ **ported (2026-11-05, gh:#1275)** | `pipeline::cartesian_2d_mesh::{cartesian_2d_mesh, cartesian_2d_mesh_restart}` | Thin `main`. **Binary since gh:#1476 (2026-11-11):** `outram-foam-cli`'s `cartesian2DMesh` reads `system/meshDict` (`mesh_dict::read_mesh_dict`) and the `surfaceFile` (`tri_surf_io`) and writes `constant/polyMesh` (`poly_mesh_gen_io`), through `outram_foam_cli::cfmesh`. |
| `meshLibrary/cartesian2DMesh/cartesian2DMeshGenerator` | 536 | ~~not ported~~ **ported (2026-11-05, gh:#1275)** | `pipeline::cartesian_2d_mesh` | Every member in Rust. ~~Steps whose callees are gaps say so in the report: `optimiseMeshSurface` (`optimizeSurface2D`, `untangleSurface2D`), the 2D `refineBoundaryLayers`;~~ **UPDATED 2026-11-05 (gh:#1275 remainder):** `optimiseMeshSurface` and the 2D `refineBoundaryLayers` are ported and called; the layer is ~~the one-group route~~ **patch-wise with `activate2DMode` (gh:#1270 remainder)**. Results: `verification_and_validation/cartesian_2d_mesh_gh1275/`. |
| `meshLibrary/utilities/workflowControls` | 477 | ~~not ported~~ **ported (2026-11-05, gh:#1272)** | `workflow_controls` | `stopAfter` / restart of a workflow step; a member of all four generators. The meta data and the saved mesh are values, not files; **since gh:#1476** the binaries write them (`meshMetaDict`) and read them back for `restartFromLatestStep` (`poly_mesh_gen_io`). |
| `meshLibrary/utilities/checkMeshDict` | 1438 | ~~not applicable~~ **ported (2026-11-11, gh:#1476)** | `mesh_dict::{check_mesh_dict, check_local_refinement_level}`; the reader `mesh_dict::read_mesh_dict` | ~~Validation of a `meshDict`; settings here are typed Rust structs. Revisit if a `meshDict` reader is ever added (§4, "Not planned").~~ `checkEntries` in upstream's order, defects kept (`checkLocalRefinementLevel` not called; the subset list branch reads `patchCellSize`). Not ported: `updateDictionaries` (used only after `triSurfacePatchManipulator` re-patches a surface with feature edges; the binaries refuse such a surface). |

_Subtotal: 8015 lines._

### B. Octree (`octrees/meshOctree`)

| Upstream | Lines | Status | Rust | Notes |
|---|---:|---|---|---|
| `octrees/meshOctree (serial: meshOctree.C/H/I.H, InsideCalculations, NeighbourSearches, CubePatches, FindNearestSurfacePoint, Slot)` | 2303 | ported (2026-11-05, gh:#1262, #1263) | `mesh_octree` (`MeshOctree`, `OctreeSlot`, `TriSurf`), `mesh_octree_queries` | ~~partial: `octree` re-implementation only; missing nearest-point queries, neighbour searches, `findLeafContainingVertex`~~ **CORRECTED 2026-11-05:** transcribed beside `octree` (which is unchanged). Every serial member function is in Rust, ~~quadtree excluded~~ the quadtree included since gh:#1275 (`MeshOctree::new_quadtree`, the 2D neighbour searches). `findNearestSurfacePoint` keeps upstream's stopping rule (misses the nearest triangle for 4–7 % of points off the surface, measured); `NearestSearch::Exact` is the port's exact variant, bit-identical to the brute force, and `patches` / `snap` can route through it (`SurfaceSearch::Octree`, opt-in). |
| `octrees/meshOctree: meshOctreeParallelCommunication.C` | 448 | not applicable | — | MPI; the crate is serial. |
| `octrees/meshOctree/meshOctreeCube` | 3078 | ported (2026-11-05, gh:#1262) | `mesh_octree` (`CubeCoordinates`, `OctreeCube`, `CubeType`) | ~~partial: coordinates and refinement only~~ **CORRECTED 2026-11-05:** intersections, cube types, `refineCube`, `findContainedEdges` and the recursive leaf functions transcribed. Not ported: ~~`refineCube2D` (quadtree, item 21)~~ (ported 2026-11-05, gh:#1275: `refine_cube` on a quadtree), the MPI functions, the unused Hilbert tables. `octree::Cell` is unchanged. |
| `octrees/meshOctree/meshOctreeCreator (serial)` | 2118 | ~~partial (2026-11-05, gh:#1264, #1265)~~ **ported (2026-11-05, gh:#1264/#1265 remainders)** | `mesh_octree_creator::MeshOctreeCreator`, `MeshDict`, `EdgeMesh`; `octree::refine_near_boundary(_banded)` kept | ~~missing `maxCellSize` / `boundaryCellSize` sizing, `localRefinement`, `objectRefinements`, `refineBoxesContainedInObjects`, `createOctreeWithRefinedBoundary`, `refineBoxesNearDataBoxes`~~ **CORRECTED 2026-11-05:** all of those transcribed, plus `patchCellSize`. ~~Still missing: `refineBoxesIntersectingSurfaces` / `EdgeMeshes` (refinement sources), `subsetCellSize`, `anisotropicSources`, the automatic refinement call.~~ **CORRECTED 2026-11-05:** the sources, `subsetCellSize` (and subset `localRefinement` entries) and the automatic refinement call are transcribed, all off unless the `MeshDict` asks. `anisotropicSources` moves the sources through group K's `anisotropic` (gh:#1279; `source_surface_geometry`, `source_edge_mesh_geometry`, `MeshDict::anisotropic_sources`). Source files are values, not read. Results: `verification_and_validation/octree_remainders_gh1278/`. |
| `octrees/meshOctree/meshOctreeCreator: LoadDistribution.C` | 56 | not applicable | — | MPI load balance. |
| `octrees/meshOctree/meshOctreeModifier (serial)` | 1469 | ported (2026-11-05, gh:#1263) | `mesh_octree_modifier`; `octree::balance_2to1` kept | ~~partial: missing `reduceMemoryConsumption`, `markAdditionalLayers`, `refineSelectedBoxesAndAdditionalLayers`~~ **CORRECTED 2026-11-05:** all serial functions transcribed. Upstream's 1-irregularity is over nodes, edges and faces; `octree::balance_2to1` balances over faces only. |
| `octrees/meshOctree/meshOctreeModifier: DistributeLeaves, LoadDistribution, ParallelRefinement, UpdateCommunicationPattern` | 920 | not applicable | — | MPI. |
| `octrees/meshOctree/meshOctreeAddressing (serial)` | 3118 | partial (2026-11-05, gh:#1266) | `cartesian_mesh::MeshOctreeAddressing`; `octree::build_mesh` kept | ~~missing `octreeFaces`, `nodeLabels`/`nodeLeaves`~~ **CORRECTED 2026-11-05:** used boxes, node labels, points, node leaves, node types, octree faces, leaf faces, `findEdgeCentre`, `cubesAroundEdge` transcribed. ~~Still missing: octree edges and their five maps, `leafLeaves`, `nodeFaces`, `isIntersectedFace/Edge`, `checkGluedRegions`, `IrregularConnections`.~~ **CORRECTED 2026-11-05 (gh:#1266 remainder): status partial -> ported.** Octree edges and their five maps (`edge_addressing`), `leafLeaves`, `nodeFaces`, `isIntersectedFace/Edge`, `edgeIntersections`, `checkGluedRegions` (`from_dict`, `checkForGluedMesh`), `checkAndFixIrregularConnections` (no upstream caller; its node test is dead code upstream) and `findUsedBoxes`' `nonManifoldMeshing` / `keep`/`removeCellsIntersectingPatches` (`CellSelection`) are transcribed. Results: `verification_and_validation/octree_remainders_gh1278/`. |
| `octrees/meshOctree/meshOctreeAddressing: ParallelAddressing.C` | 687 | not applicable | — | MPI. |
| `octrees/meshOctree/meshOctreeInsideOutside` | 955 | ported (2026-11-05, gh:#1264) | `mesh_octree_creator::mesh_octree_inside_outside`; `octree::inside` kept | ~~partial, ray parity only~~ **CORRECTED 2026-11-05:** frontal marking transcribed (serial). Against ray parity: 0 disagreements on every non-DATA leaf of the sphere and the pipe. |
| `octrees/meshOctree/meshOctreeAutomaticRefinement` | 976 | ~~not ported~~ **ported (2026-11-05, gh:#1278)** | `mesh_octree_automatic_refinement` (with `help::numberOfFaceGroups` / `numberOfEdgeGroups`) | Curvature- and proximity-based refinement; corners and partitions. Runs from `createOctreeBoxes` when `MeshDict::min_cell_size` is set (off by default, as upstream without `minCellSize`). The unused `octreeAddressing()` member is not built. Results: `verification_and_validation/octree_remainders_gh1278/`. |
| `octrees/meshOctree/refinementControls/objectRefinement` | 2572 | ported (2026-11-05, gh:#1265) | `object_refinement` | ~~not ported~~ **CORRECTED 2026-11-05:** box, sphere, cone, hollow cone, line (an enum). Dictionary I/O not ported (typed fields instead). |
| `octrees/meshOctree/refinementControls/patchRefinement` | 312 | ported (2026-11-05, gh:#1265) | `mesh_octree_creator::PatchRefinement` | ~~not ported~~ **CORRECTED 2026-11-05:** by region index, not by name. |

_Subtotal: 19012 lines._

### C. Mesh data structures and addressing (`meshes/`)

| Upstream | Lines | Status | Rust | Notes |
|---|---:|---|---|---|
| `meshes/polyMeshGen` | 2664 | partial | `volume_mesh::VolumeMesh`, `from_cell_faces(_wound)`, `cells_faces` | Points/faces/owner/neighbour/patches. Missing: ~~point/face/cell subsets (`addPointSubset` etc.)~~ (`mesh_subsets`, gh:#1261), `polyMeshGenCells` cell list (cells are implicit), ~~`write`/`read`~~ (**ported 2026-11-11, gh:#1476:** `poly_mesh_gen_io::{write_poly_mesh_gen, read_poly_mesh_gen}`, with subsets as `pointSet`/`faceSet`/`cellSet` files and `meshMetaDict`; feature `foam-export`). |
| `meshes/polyMeshGen/boundaryPatch (boundaryPatch, boundaryPatchBase)` | 639 | partial | `volume_mesh::BoundaryPatch` | Name, start, size. Patch `type` (wall/patch/symmetry) not carried; `foam` decides it. |
| `meshes/polyMeshGen/boundaryPatch: processorBoundaryPatch` | 363 | not applicable | — | MPI processor patches. |
| `meshes/primitives/meshSubsets` | 442 | ported (gh:#1261) | `mesh_subsets::{MeshSubset, SubsetGroup, MeshSubsets}`; `mesh_optimizer::MeshOptimizer::lock_*_in_subset` | ~~not ported~~. A value beside `VolumeMesh` (31 struct literals would need a new field), not inside it. ~~Subset file I/O (`polyMeshGen::read`/`write`) not ported.~~ **Ported 2026-11-11 (gh:#1476):** `poly_mesh_gen_io`. |
| `meshes/polyMeshGenAddressing (serial)` | 2576 | partial | `mesh_optimizer::point_cells`, `cell_faces`, `checks::cell_geometry`, ad hoc maps per module | No shared addressing object: each module rebuilds what it needs. Missing as a reusable set: `edges`, `edgeFaces`, `faceEdges`, `pointPoints`, `cellCells`, `cellEdges`. |
| `meshes/polyMeshGenAddressing: ParallelAddressing.C` | 974 | not applicable | — | MPI. |
| `meshes/polyMeshGenModifier (AddCells, RemoveCells, RemoveFaces, RemoveUnusedVertices, ReorderBoundaryFaces, ReplaceBoundary, AddCellByCell, .H)` | 1862 | partial | `volume_mesh::from_cell_faces`, `patches::assign_patches_by_region`; `tet_mesh_extractor_octree::poly_mesh_from_cells` (gh:#1272) | Replaced: every stage rebuilds the mesh from cell face lists. Not a line-by-line port. **Added 2026-11-05 (gh:#1272):** `ReorderBoundaryFaces`, `RemoveUnusedVertices` (serial) and `ReplaceBoundary` (serial, in `rename_boundary_patches`) are transcribed. |
| `meshes/polyMeshGenModifier: RenumberMesh.C` | 282 | ~~not ported~~ **ported (2026-11-05, gh:#1272)** | `renumber_mesh::renumber_mesh`, with `calcCellCells` | ~~Cuthill–McKee renumbering. No Rust renumbering found in the workspace.~~ **CORRECTED 2026-11-05:** upstream's comment says Cuthill–McKee, but `LongList::remove` pops the last element, so the walk is depth-first; ported as written. Cells' face lists come from the caller (`cells_from_mesh`: ascending face order). |
| `meshes/polyMeshGenModifier: ZipUpCells.C` | 794 | not ported | — (`octree::conforming_ring` splices hanging nodes during assembly) | No caller in `meshLibrary/` at this commit (grep). Not planned. |
| `meshes/polyMeshGenModifier: AddBufferCells, AddProcessorFaces` | 437 | not applicable | — | MPI. |
| `meshes/polyMeshGen2DEngine` | 689 | ~~not ported~~ **ported (2026-11-05, gh:#1275)** | `poly_mesh_gen_2d_engine::PolyMeshGen2DEngine` | Eager, not demand-driven. |
| `meshes/partTetMesh (serial)` | 2119 | partial | `mesh_optimizer::PartTetMesh` | Ported: `partTetMesh(mesh, locked, badFaces, layers)`, ~~missing~~ the `(mesh, locked, nLayers)` constructor (`near_boundary`, gh:#1255), `BOUNDARY` nodes moved by the boundary smoothers (gh:#1255), `createPointsAndTets`, `partTetMeshSimplex`, `updateVerticesSMP`, `updateOrigMesh`. Missing: `createPolyMesh` (debug output), `tetMatcher` handling. |
| `meshes/partTetMesh: ParallelAddressing.C` | 535 | not applicable | — | MPI. |
| `meshes/partTriMesh (serial)` | 1331 | ~~not ported~~ **ported (2026-11-05, gh:#1258)** | `surface_optimizer::PartTriMesh`, `PartTriMeshSimplex` | Surface tri mesh for `meshSurfaceOptimizer`: both constructors, `updateVertex`, `updateVertices`, the simplex and its normal. |
| `meshes/partTriMesh: ParallelAddressing.C` | 538 | not applicable | — | MPI. |
| `meshes/primitives/partTet` | 507 | not applicable | `[usize; 4]` + `mesh_optimizer::tet_volume` | Primitive, replaced by an index array. |
| `meshes/triSurf` | 2907 | ~~partial~~ **ported (2026-11-05, gh:#1276)** | `mesh_octree::TriSurf` (`SurfacePatch`; subsets as `mesh_subsets::SubsetGroup`) | ~~Points, facets and region (patch) per facet. Missing: feature edges, facet/point/edge subsets, `triSurfAddressing` (edges, edgeFacets, pointFacets, facetEdges).~~ Points, facets, regions, patches, stored feature edges, point/facet/feature-edge subsets, the whole `triSurfAddressing` (`facetNormals`, `facetCentres`, `pointNormals`, `facetFacetsEdges` computed on demand). ~~Not ported: the FMS/FTR/STL readers and writers (file formats).~~ **UPDATED 2026-11-11 (gh:#1476):** the readers are ported (`tri_surf_io::{read_fms, read_ftr, read_surface_bytes}`; STL through `outram-foam-mesh`'s port of OpenFOAM's `STLReader`, feature `surface-file`); the writers are not. `topologyCheck` is the panics of `TriSurf::from_parts`. |

_Subtotal: 19659 lines._

### D. Quality checks

| Upstream | Lines | Status | Rust | Notes |
|---|---:|---|---|---|
| `meshes/polyMeshGenChecks` | 4441 | ported (gh:#1261) | `poly_mesh_gen_checks` (every check, thresholds as arguments, `MeshQualitySettings`, `find_bad_faces_additional_checks`, `checkGeometry`/`Topology`/`Mesh`); `mesh_optimizer` (`find_bad_faces`, `find_low_quality_faces`, `find_worst_quality_faces`, fixed thresholds) | ~~partial: 9 of 34 definitions~~. `checks.rs` holds OpenFOAM's definitions, kept beside. Upstream quirks kept and documented (`checkCellDeterminant` checks nothing without active flags; `checkMinVolRatio`/`checkFaceUniformity` always return false; `checkVolumeUniformity` is a stub; `> n` label tests). |

_Subtotal: 4441 lines._

### E. Volume optimiser (`smoothers/geometry/meshOptimizer`)

| Upstream | Lines | Status | Rust | Notes |
|---|---:|---|---|---|
| `smoothers/geometry/meshOptimizer (meshOptimizer.C/H/I.H, optimizeMeshFV.C, OptimizeSurface.C)` | 1590 | partial | `mesh_optimizer::{untangle_mesh_fv_with, UntangleControls, optimize_mesh_near_boundaries, optimize_mesh_fv_best_quality, MeshOptimizer}`, `optimize_low_quality_faces`; `boundary_layer_optimisation` | Ported since the audit: the boundary loop of `untangleMeshFV` (opt-in, gh:#1255), `optimizeMeshNearBoundaries`, `optimizeMeshFVBestQuality`, `optimizeMeshFV` (gh:#1256), `enforceConstraints`, the lock API and the subset locks (gh:#1256, #1261). Missing: `optimizeSurface` (the surface track, gh:#1258/#1259). `lockPointsInSubset` deliberately differs: upstream calls `lockCells` with point labels (defect). |
| `smoothers/geometry/meshOptimizer: meshOptimizerOptimizePoint.C (laplaceSmoother)` | 437 | ported (gh:#1256) | `mesh_optimizer::{laplace_smoother, laplace_smoother_surface, LaplaceKind}` | ~~not ported~~. `laplacian`, `laplacianPC`, `laplacianWPC`, `laplacianSurface`; the `badFaces` overloads are `NotImplemented` upstream. `smooth::laplacian_smooth` (Freitag's) is kept, separate. |
| `smoothers/geometry/meshOptimizer: meshOptimizerOptimizePointParallel.C` | 461 | not applicable | — | MPI. |
| `smoothers/geometry/meshOptimizer/tetMeshOptimisation (serial)` | 788 | ported (gh:#1255, #1260) | `mesh_optimizer::PartTetMesh::{optimise_using_knupp_metric, optimise_using_mesh_untangler, optimise_using_volume_optimizer, optimise_boundary_volume_optimizer, optimise_boundary_surface_laplace}` | ~~partial~~. The eigen split of `optimiseBoundaryVolumeOptimizer(true)` calls `outram_foam_basic_lib::primitives::eigen` (OpenFOAM's `eigenValues`). Port defect found, not fixed (default output): the Knupp loop's pass counter (see its doc comment). |
| `smoothers/geometry/meshOptimizer/tetMeshOptimisation: Parallel.C` | 337 | not applicable | — | MPI. |
| `.../advancedSmoothers/knuppMetric` | 359 | ported | `mesh_optimizer::knupp_metric` | Not re-checked line by line; module header and doc comment cite `optimizeNodePosition`, `evaluateMetric(NoBeta)` and `evaluateGradients`. |
| `.../advancedSmoothers/simplexSmoother` | 179 | ported | `mesh_optimizer` simplex signature `(pts, tets, free)` | Abstract base (bounding box, point access); replaced by a free-function signature. gh:#1251 lists it as missing: it is not. |
| `.../advancedSmoothers/volumeOptimizer` | 603 | ported | `mesh_optimizer::volume_optimizer`, `vo_func`, `vo_gradients`, `vo_stabilisation` | `optimizeNodePosition` checked: divide and conquer, then Newton/steepest descent, kept only if no worse. |
| `.../advancedSmoothers/meshUntangler` | 1233 | ported (gh:#1260) | `mesh_untangler::{CutRegion, Plane, mesh_untangler}` | ~~not ported~~. `createPolyMeshFromRegion` (debug) not ported; `tieBreak`'s region walk bounded to one lap. |
| `.../advancedSmoothers/quadricMetric` | 329 | not ported | — | No upstream workflow calls it (`grep` of `meshLibrary/`: only its own files). |
| `smoothers/geometry/meshOptimizer/boundaryLayerOptimisation` | 2866 | partial | `boundary_layer_optimisation` | Ported: `calculateHairEdges`, `optimiseLayersAtExittingFaces`, `optimiseLayer`, the normals and thickness routines. ~~Missing: the `meshSurfaceOptimizer::optimizeSurface(2)` call inside `optimiseLayer` (the wall points never move);~~ **UPDATED 2026-11-05 (gh:#1259):** the `optimizeSurface(2)` call is ported, opt-in (`BoundaryLayerOptimisationControls::optimise_surface`, off by default). Hair classes come from `layers::LayerVertex`, not `detectBoundaryLayers`. |
| `smoothers/geometry/meshOptimizer/symmetryPlaneOptimisation` | 405 | ~~not ported~~ **ported (2026-11-05, gh:#1280)** | `symmetry_plane_optimisation` | Called by `rename_boundary_patches` (`checkSymmetryPlanes`), so by `tet_mesh`, `p_mesh` and `cartesian_mesh`. |

_Subtotal: 9587 lines._

### F. Surface optimiser (`smoothers/geometry/meshSurfaceOptimizer`)

| Upstream | Lines | Status | Rust | Notes |
|---|---:|---|---|---|
| `smoothers/geometry/meshSurfaceOptimizer (serial)` | 2266 | ~~not ported~~ **partial (2026-11-05, gh:#1259)** | `surface_optimizer::MeshSurfaceOptimizer`, `map_mesh_to_surface`, `optimise_mesh_surface`; `mesh_optimizer::optimize_surface` (`meshOptimizerOptimizeSurface.C`) | Ported (3-D, serial): `untangleSurface` (both forms), `optimizeSurface`, Laplacian FC, `surfaceOptimizer` smoothing, edge-node smoothing, `lockBoundaryFaces`/`Points`/`FeatureEdges`, `removeUserConstraints`, the mapper calls. Not ported: ~~`untangleSurface2D`, `optimizeSurface2D`~~ (**ported 2026-11-05, gh:#1275 remainder**: `optimize_surface_2d`, `untangle_surface_2d`, wired into `cartesian_2d_mesh`), `enforceConstraints` (no point subsets), `newPositionLaplacian`/`LaplacianWFC` (no 3-D caller). All opt-in (`TetDualOptions::untangle_surface`); differential against `revert_tangled` in `laminar_pipe_gh1155.md` (gh:#1259). |
| `smoothers/geometry/meshSurfaceOptimizer: OptimizePointParallel.C` | 522 | not applicable | — | MPI. |
| `.../advancedSurfaceSmoothers/surfaceOptimizer` | 545 | ~~not ported~~ **ported (2026-11-05, gh:#1258)** | `surface_optimizer::optimize_point` | The 2D (in-plane) volume-optimiser analogue `untangleSurface` uses; `matrix2D`'s solve inline. |

_Subtotal: 3333 lines._

### G. Surface tools (`surfaceTools/`, `intersectionTools/`)

| Upstream | Lines | Status | Rust | Notes |
|---|---:|---|---|---|
| `surfaceTools/meshSurfaceEngine (serial)` | 3086 | ~~partial~~ **ported (2026-11-05, gh:#1257)** | ~~ad hoc boundary maps in `layers::layer_setup`, `correct_edges`, `surface_mapper`~~ `surface_engine::SurfaceEngine` | ~~No shared engine. Missing as a reusable object: …~~ Every serial accessor and `meshSurfaceEngineModifier` (`moveBoundaryVertex(NoUpdate)`, `updateGeometry`). Not ported: the `activePatch` constructor (no caller here). Built eagerly (no lifetimes); the caller owns the points. `layer_setup`, `correct_edges` and `surface_mapper` now read their boundary maps from it. |
| `surfaceTools/meshSurfaceEngine: ParallelAddressing.C` | 775 | not applicable | — | MPI. |
| `surfaceTools/meshSurfaceMapper (mapVerticesOntoSurface(Patches), mapCorners, mapEdgeNodes, findMappingDistance)` | 1175 | ported | `surface_mapper::map_vertices_onto_surface_patches(_untangled(_given))`, `map_vertices_onto_surface_untangled`; `surface_mapper::SurfaceQueries`, `map_edge_node` | `mapVerticesOntoSurface` has no `untangleSurface` after it (moves undone instead). `mapVerticesOntoSurfacePatches(nodesToMap)` (a subset) is `surface_mapper::map_points_onto_patches` (gh:#1269; gh:#1283's `map_points_onto_surface_patches_given` goes through it too; the whole-surface mapping now calls it with no subset, operation for operation as before, pinned by the dual-kind fingerprints). `findMappingDistance` uses vertex-average face centres here, OpenFOAM centres upstream (kept, so the pins hold). Surface queries are brute force (upstream: `meshOctree`). |
| `surfaceTools/meshSurfaceMapper: PremapVertices.C` | 302 | ~~not ported~~ **ported (2026-11-05, gh:#1274)** | `premap_vertices::{pre_map_vertices, face_metric_in_patch}` | `preMapVertices` (cartesianMesh, cartesian2DMesh, pMesh). Serial; the MPI exchange is not applicable. Called by `pipeline::cartesian_mesh`; ~~`pipeline::p_mesh` still notes it as missing (not wired, to keep the edgeExtractor track's file untouched; re-measure when wired).~~ **Wired into `pipeline::p_mesh` 2026-11-05 (gh:#1274 remainder)**; gh:#1273 re-measured. |
| `surfaceTools/meshSurfaceMapper: parMapperHelper.H` | 194 | not applicable | — | MPI. |
| `surfaceTools/meshSurfaceMapper2D` | 1189 | ~~not ported~~ **ported (2026-11-05, gh:#1275)** | `surface_mapper_2d` | Serial. The no-argument `mapCorners()` has no caller and is not ported (it also mixes mesh and boundary-point labels). |
| `surfaceTools/meshSurfaceEdgeExtractorNonTopo` | 198 | ported | `pipeline` (`correct_edges` then `surface_mapper`) | `decomposeBoundaryFaces` + `remapBoundaryPoints`. |
| `surfaceTools/correctEdgesBetweenPatches` | 835 | ported | `correct_edges::correct_edges_between_patches` | `decomposeProblematicFaces` is commented out upstream. Opt-in (`TetDualOptions::correct_edges`). |
| `surfaceTools/meshSurfaceCheckEdgeTypes` | 483 | partial | `correct_edges` (tet test) | `classifyEdges` tet test only. Missing: the `meshSurfaceCheckInvertedVertices` (UNDETERMINED) branch, the `convexEdges`/`concaveEdges` accessors as a reusable class. |
| `surfaceTools/meshSurfaceCheckInvertedVertices` | 555 | ~~not ported~~ **ported (2026-11-05, gh:#1258)** | `surface_optimizer::check_inverted_vertices`, `is_face_convex_and_ok` (`surface_mapper::cells_with_nonpositive_pyramids` is a cell test, not this) | On the crate's per-point / cell-centre duals of the 48-sided pipe (h 6 mm) it flags 554 of 1 390 boundary points before any mapping, 550 of them as reflex corners of 156 non-convex boundary polygons (the differential test in `surface_optimizer`). Not yet wired into `correct_edges`' UNDETERMINED edge class. |
| `surfaceTools/meshSurfacePartitioner` | 513 | ~~partial~~ **ported (2026-11-05, gh:#1257)** | `surface_engine::SurfacePartitioner` | ~~Corners (>2 feature edges) and edge points (2). Missing: `patchPatches`, `edgeGroups`, as a reusable class.~~ All of it, serial: corners, edge points, `pointPatches`, `patchPatches`, `featureEdges`, `numberOfFeatureEdgesAtPoint`. **CORRECTED 2026-11-05:** `meshSurfacePartitioner.H` at `e944f6ba695f` has no `edgeGroups`; the earlier row named a member that does not exist there. |
| `surfaceTools/edgeExtraction/edgeExtractor` | 4852 | ~~not ported~~ **ported (2026-11-05, gh:#1269)** | ~~— (`patches::assign_patches_by_region`: nearest triangle only)~~ `edge_extractor::{EdgeExtractor, extract_patches}`; wired into `pipeline::tet_mesh` / `p_mesh` (`PatchAssignment`) | Refines the face→patch assignment. Every member the workflows call (constructor, `extractEdges`, `updateMeshPatches`, `faceEvaluator`) plus `moveVerticesTowardsDiscontinuities`, `projectDeterminedFeatureVertices`, `untangleSurface`, `surfaceWithPatches`, `markPatchPoints`. Not ported, no caller upstream: `findEdgeCandidates` (only prints; indexes mesh edges with surface edge labels), `checkConcaveEdgeCells` (needs `triSurfaceClassifyEdges`), `findCornerCandidates` (discards its result), `checkCorners` (calls `::exit(0)` part-way), `cornerEvaluator` (never constructed). Results: `verification_and_validation/edge_extractor_gh1269/`. |
| `surfaceTools/meshSurfaceEdgeExtractor` | 406 | not ported | — | Older topological extractor; no generator in `meshLibrary/` calls it. |
| `surfaceTools/meshSurfaceEdgeExtractorFUN` | 407 | ~~not ported~~ ~~**partial (2026-11-05, gh:#1269)**~~ **ported (2026-11-05, gh:#1281)** | `mesh_surface_edge_extractor_fun` (called by `pipeline::p_mesh`) | Fundamental-sheets extractor (used with `createFundamentalSheets`). Ported: the constructor's `smoothMeshSurface` and `remapBoundaryPoints`. ~~Missing: `createBasicFundamentalSheets` (`createFundamentalSheetsJFS`, which needs `extrudeLayer`, item 26);~~ **gh:#1281:** `createBasicFundamentalSheets` runs (`FunSheets::Without` is the ablation). `distributeBoundaryFaces` and `improveQualityOfFundamentalSheets` have no caller. |
| `surfaceTools/meshSurfaceEdgeExtractor2D` | 311 | ~~not ported~~ **ported (2026-11-05, gh:#1275)** | `surface_mapper_2d::{distribute_boundary_faces, remap_boundary_points}` | |
| `surfaceTools/createFundamentalSheets (base, FJ, JFS)` | 908 | ~~not ported~~ **ported (2026-11-05, gh:#1281)** | `fundamental_sheets` (`SheetsKind` is the run-time selection) | Layer of cells at feature edges for `meshSurfaceEdgeExtractorFUN`. |
| `surfaceTools/renameBoundaryPatches` | 392 | ~~not ported~~ **ported (2026-11-05, gh:#1272)** | `rename_boundary_patches` (`RenameBoundary`, `regex_full_match`) | `renameBoundary` / patch `type`. Types travel beside the mesh (`VolumeMesh` has none). ~~`checkSymmetryPlanes` calls `symmetryPlaneOptimisation` (not ported): reported, not run.~~ **gh:#1280:** `checkSymmetryPlanes` runs `symmetry_plane_optimisation`. The patch-name regex supports the common subset (module docs). |
| `surfaceTools/surfaceMorpherCells` | 1139 | ported (2026-11-05, gh:#1267) | `topology::surface_morpher_cells` | ~~not ported~~ `surfacePreparation`: one boundary face per cell. Serial; adds an iteration guard upstream lacks. |
| `intersectionTools/findCellsIntersectingSurface` | 501 | not ported | — | No caller in `meshLibrary/` at this commit (grep). |

_Subtotal: 18221 lines._

### H. Boundary layers (`boundaryLayers/`)

| Upstream | Lines | Status | Rust | Notes |
|---|---:|---|---|---|
| `boundaryLayers/boundaryLayers` | 4229 | ~~partial~~ **ported (2026-11-05, gh:#1270 remainder)** | `layers::layer_setup`, `layers::add_boundary_layers_adaptive_on_patches`; **`layers::boundary_layers::boundary_layers_add_layer` (2026-11-05, gh:#1270)** | Ported: the per-patch border rules; ~~missing `boundaryLayersCreateVertices`, `boundaryLayerCells`~~ **CORRECTED 2026-11-05 (gh:#1270):** the one-group creation route (`findPatchVertices`, `createNewVertex`, `createNewVertices(boolList)`, `createNewFacesAndCells`) is ported beside the crate's shrink. ~~Missing: `findPatchesToBeTreatedTogether`, the patch-wise route (`createNewVertices(labelList)`, `createLayerCells`, `boundaryLayerCells.C`), `checkTopologyOfBndFaces`, `addWrapperLayer`, 2D, MPI.~~ **UPDATED 2026-11-05 (gh:#1270 remainder):** the class is `layers::boundary_layers::BoundaryLayers` with all of it: `findPatchesToBeTreatedTogether`, the patch-wise route, `activate2DMode`, `createOTopologyLayers`, `addWrapperLayer` (an upstream defect followed to its intent), `checkTopologyOfBoundaryFaces`; `isSharedEdgeConvex`, `angleBetweenFaces`. Wired into all four workflows and `generateBoundaryLayers`. Not applicable: the MPI branches. Record: `verification_and_validation/patch_wise_layers_gh1270/`. |
| `boundaryLayers/refineBoundaryLayers` | 5166 | partial | `layers::refine_boundary_layers::refine_boundary_layers` (2026-11-05, gh:#1271); `layers::build_layered_variable` | ~~Only "split each hair by the grading"~~ **CORRECTED 2026-11-05 (gh:#1271):** the settings (global and per-patch `nLayers`, `thicknessRatio`, `maxFirstLayerThickness`, `allowDiscontinuity`), `analyseLayers` and `generateNewVertices` are ported, with the split of prism layer cells. Missing: ~~`refineEdgeHexCell`, `refineCornerHexCell` and the general `refineBoundaryLayersFaces`/`Cells` (refused as `NotPorted`)~~ (**ported 2026-11-05, gh:#1271 remainder**: `refineFace`, `generateNewFaces`, `generateNewCells` by `refType`, the edge and corner hexes; only the appended-points case of a face whose count differs from its hairs' is refused), `activateSpecialMode`, ~~`pointsInBndLayer`~~ (ported, gh:#1274), 2D, MPI. |
| `boundaryLayers/detectBoundaryLayers` | 989 | ported (2026-11-05, gh:#1271) | `layers::refine_boundary_layers::detect_boundary_layers` | ~~not ported~~ Serial. |
| `boundaryLayers/extrudeLayer` | 1915 | ~~not ported~~ **ported (2026-11-05, gh:#1280)** | `layers::extrude_layer` | Serial (the MPI half is not applicable). Edge and corner cells (self-intersecting layers) transcribed, not reached by any test. |
| `boundaryLayers/triangulateNonPlanarBaseFaces` | 357 | ~~not ported~~ **ported (2026-11-05, gh:#1280)** | `layers::triangulate_non_planar_base_faces` | No upstream caller at this commit; none here. |

_Subtotal: 12656 lines._

### I. Topology repair and decomposition

| Upstream | Lines | Status | Rust | Notes |
|---|---:|---|---|---|
| `smoothers/topology/topologicalCleaner` | 857 | ported (2026-11-05, gh:#1267) | `topology::topological_cleaner` | ~~not ported~~ `surfacePreparation`. Serial. |
| `smoothers/topology/checkIrregularSurfaceConnections` | 1153 | ported (2026-11-05, gh:#1268) | `topology::{irregular_vertices, check_and_fix_irregular_connections}` | ~~not ported~~ cartesianMesh. Serial (the MPI dual-edge exchange is not applicable). |
| `smoothers/topology/checkBoundaryFacesSharingTwoEdges` | 649 | ported (2026-11-05, gh:#1268) | `topology::BoundaryFacesSharingTwoEdges` | ~~not ported~~ cartesianMesh. Upstream defect kept: `improveTopology` never marks a cell (`decomposeCell[owner[faceI]];` assigns nothing), so only faces are fanned. |
| `smoothers/topology/checkCellConnectionsOverFaces` | 369 | ported (2026-11-05, gh:#1268) | `topology::check_cell_connections_over_faces` (through `mesh_octree_creator::group_marking`) | ~~not ported~~ cartesianMesh. |
| `smoothers/topology/checkNonMappableCellConnections` | 437 | ported (2026-11-05, gh:#1268) | `topology::{non_mappable_cells, remove_non_mappable_cells}` | ~~not ported~~ cartesianMesh. |
| `decomposeCells` | 755 | ported (2026-11-05, gh:#1267) | `topology::{decompose_cells, decompose_cells_into_pyramids}` | ~~partial: missing the `decomposeMesh` driver~~ The driver is `decompose_cells`, the one shared copy (used by `correct_edges`, `cartesian_mesh` and the topology checks). Fan points are numbered as found, not in face order (point numbering only). |
| `faceDecomposition: decomposeFaces` | 641 | ported (2026-11-05, gh:#1267) | `topology::TopoMesh::{decompose_mesh_faces, decompose_concave_internal_faces}` | ~~partial~~ `newFacesForFace` (the face-subset update) is not returned. |
| `faceDecomposition: faceDecomposition` | 422 | ported (2026-11-05, gh:#1267) | `topology::{concave_vertex, is_face_convex, is_face_planar, decompose_face, decompose_face_into_triangles}` | ~~not ported~~ Concave face → convex parts. |

_Subtotal: 5283 lines._

### J. Surface (`triSurfaceTools/`) and helper classes

| Upstream | Lines | Status | Rust | Notes |
|---|---:|---|---|---|
| `triSurfaceTools/triSurfaceChecks` | 1253 | **ported (2026-11-05, gh:#1277)** | `surface::checks` | ~~not ported~~ With `help::doTrianglesIntersect` (distance form) and `doTrianglesOverlap`. Upstream defect fixed: the subset form of `checkForNonManifoldEdges` runs the hole check. |
| `triSurfaceTools/triSurfaceDetectFeatureEdges` | 277 | **ported (2026-11-05, gh:#1276)** | `surface::features::TriSurfaceDetectFeatureEdges` | ~~not ported~~ Differential against `outram-foam-mesh`'s `snapping::detect_feature_edges` (~~private~~ made `pub` for it, 2026-11-05): equal on closed, consistently wound surfaces. |
| `triSurfaceTools/triSurfacePatchManipulator` | 509 | **ported (2026-11-05, gh:#1276)** | `surface::features::TriSurfacePatchManipulator` | ~~not ported~~ Split a surface into patches at feature edges. The `meshDict` update is `patches_for_patch` (no dictionary). |
| `triSurfaceTools/triSurfaceClassifyEdges` | 575 | **ported (2026-11-05, gh:#1276)** | `surface::features::TriSurfaceClassifyEdges` | ~~not ported~~ |
| `triSurfaceTools/triSurfacePartitioner` | 598 | **ported (2026-11-05, gh:#1276)** | `surface::features::TriSurfacePartitioner` | ~~not ported~~ |
| `triSurfaceTools/triSurfaceCurvatureEstimator` | 1229 | **ported (2026-11-05, gh:#1278)** | `surface::curvature::TriSurfaceCurvatureEstimator` | ~~not ported~~ Feeds `meshOctreeAutomaticRefinement` (~~not ported~~ **ported 2026-11-05, gh:#1278**). The VTK debug output is not ported. |
| `triSurfaceTools/triSurfaceCleanupDuplicates` | 454 | **ported (2026-11-05, gh:#1277)** | `surface::cleanup::cleanup_duplicates` | ~~not ported~~ |
| `triSurfaceTools/triSurfaceCleanupDuplicateTriangles` | 235 | **ported (2026-11-05, gh:#1277)** | `surface::cleanup::cleanup_duplicate_triangles` | ~~not ported~~ |
| `triSurfaceTools/triSurfaceCopyParts` | 412 | **ported (2026-11-05, gh:#1277)** | `surface::cleanup::copy_surface_parts` | ~~not ported~~ |
| `triSurfaceTools/triSurfaceRemoveFacets` | 377 | **ported (2026-11-05, gh:#1277)** | `surface::cleanup::TriSurfaceRemoveFacets` | ~~not ported~~ |
| `triSurfaceTools/triSurfaceImportSurfaceAsSubset` | 343 | **ported (2026-11-05, gh:#1277)** | `surface::cleanup::import_surface_as_subset` | ~~not ported~~ |
| `triSurfaceTools/triSurfaceExtrude2DEdges` | 201 | ~~not ported~~ **ported (2026-11-05, gh:#1275)** | `tri_surface_2d::extrude_2d_edges` | Optional region per edge (upstream: all `patch0`). |
| `triSurfaceTools/triSurface2DCheck` | 264 | ~~not ported~~ **partial (2026-11-05, gh:#1275)** | `tri_surface_2d::is_2d_surface` | `is2DSurface` ported; `createSubsets` not. On a rectangle the check is decided by round-off in another crate's eigen routine (gh:#1275 README, D1). |
| `triSurfaceTools/triSurfaceMetaData` | 233 | not applicable | — | FMS metadata. |
| `helperClasses/sortEdgesIntoChains` | 425 | ported (gh:#1260) | `mesh_untangler::sort_edges_into_chains` | ~~not ported~~. Used by `tieBreak` (and upstream by `checkIrregularSurfaceConnections`, item 14). |
| `helperClasses/geometry/quadricFitting` | 487 | **ported (2026-11-05, gh:#1278)** | `surface::curvature::QuadricFitting`, `lu_solve` | ~~not ported~~ Feeds `triSurfaceCurvatureEstimator`, its only caller, which uses the constructor with a given normal; the eigenvector constructor (`calculateNormalVector`) has no caller and is not ported. |

_Subtotal: 7872 lines._

### K. Anisotropic meshing (`anisotropicMeshing/`)

| Upstream | Lines | Status | Rust | Notes |
|---|---:|---|---|---|
| `anisotropicMeshing/coordinateModification` | 1686 | ~~not ported~~ **ported (2026-11-05, gh:#1279)** | `anisotropic` (`Modification`, `BoxScaling`, `PlaneScaling`, `CoordinateModifier`) | `anisotropicSources` (box/plane scaling). The dictionary reader is not ported (structs). ~~No generator calls it.~~ **The four generators call it since gh:#1279's remainder** (`MeshDict::anisotropic_sources`). |
| `anisotropicMeshing/edgeMeshGeometryModification` | 255 | ~~not ported~~ **ported (2026-11-05, gh:#1279)** | `anisotropic::{modify_points, revert_points}` | An edge mesh's points through the modifier. |
| `anisotropicMeshing/polyMeshGenGeometryModification` | 241 | ~~not ported~~ **ported (2026-11-05, gh:#1279)** | `anisotropic::{modify_points, revert_points}` |  |
| `anisotropicMeshing/surfaceMeshGeometryModification` | 377 | ~~not ported~~ **ported (2026-11-05, gh:#1279)** | `anisotropic::modify_surface` | A cloned `TriSurf` with moved points (its addressing is topological). |

_Subtotal: 2559 lines._

### L. Helper functions, containers, parallel helpers, I/O

| Upstream | Lines | Status | Rust | Notes |
|---|---:|---|---|---|
| `helperFunctions: GeometryQueries.H + GeometryQueriesI.H` | 2330 | partial | `boundary_layer_optimisation` (`nearest_point_on_line`, `tri_segment_intersection`, `segment_face_intersection`), `snap::closest_point_on_triangle`, `math::Vec3` | Ported where used: `nearestPointOnTheEdge`, `lineFaceIntersection`, `triLineIntersection`; **added 2026-11-05:** `numberOfFaceGroups`, `numberOfEdgeGroups` (`mesh_octree_automatic_refinement`), `doFaceAndTriangleIntersect` (`cartesian_mesh`); the octree's own transcriptions are in `mesh_octree::help`. Not re-checked one by one: the rest (`isFaceConvex`, `pointInTetrahedron`, `boundBoxLineIntersection`, …). |
| `helperFunctions: FrontalMarking` | 653 | ported (2026-11-05, gh:#1264) | `mesh_octree_creator::{frontal_marking, group_marking}` | ~~not ported~~ **CORRECTED 2026-11-05:** serial `frontalMarking` and `groupMarking` as generic functions (closures, no `dyn`). |
| `helperFunctions: TopologyManipulation.H + I.H` | 829 | not ported | — | Not re-checked: some modules write their own face merging / reversal; none cites these functions. |
| `helperFunctions: helperFunctions.H` | 46 | not applicable | — | Umbrella header. |
| `helperFunctions: helperFunctionsPar` | 711 | not applicable | — | MPI. |
| `helperClasses/matrices (matrix2D, matrix3D)` | 534 | not applicable | `mesh_optimizer` `Mat3`, `det3`, `solve3` | Replaced by fixed-size arrays. |
| `helperClasses/parallelHelpers (8 classes)` | 1876 | not applicable | — | MPI exchange types. |
| `containers (DynList, FRWGraph, Graphs, IODynList, IOLongList, Lists, LongList, VRWGraph, VRWGraphList, graphRow, subGraph)` | 8093 | not applicable | `Vec`, `Vec<Vec<_>>`, `BTreeMap`, `BTreeSet` | OpenFOAM-style containers. |
| `dataConversion/foamToFPMA` | 522 | not applicable | — | FPMA file format. |

_Subtotal: 15594 lines._

### M. Top-level `utilities/` executables (paths below are under `utilities/`)

| Upstream | Lines | Status | Rust | Notes |
|---|---:|---|---|---|
| `generateBoundaryLayers` | 158 | ~~not ported~~ **partial (2026-11-05, gh:#1280)** | `mesh_utilities::generate_boundary_layers` | `boundaryLayers` + `refineBoundaryLayers` on an existing mesh. Missing: ~~`optimizeBoundaryLayer`/`untangleBoundaryLayer` (only inside the `CfMeshOptimise` strategy)~~ (**called since 2026-11-05, gh:#1280**: `boundary_layer_optimisation::{optimize_boundary_layer, untangle_boundary_layer}`), `optimizeBoundaryLayer`'s buffer layer (`activateSpecialMode`), 2D. |
| `improveMeshQuality` | 145 | ~~not ported~~ **ported (2026-11-05, gh:#1280)** | `mesh_utilities::improve_mesh_quality` |  |
| `improveSymmetryPlanes` | 65 | ~~not ported~~ **ported (2026-11-05, gh:#1280)** | `mesh_utilities::improve_symmetry_planes` |  |
| `scaleMesh` | 83 | ~~not ported~~ **ported (2026-11-05, gh:#1279)** | `anisotropic::scale_points` |  |
| `scaleSurfaceMesh` | 81 | ~~not ported~~ **ported (2026-11-05, gh:#1279)** | `anisotropic::scale_points` |  |
| `surfaceFeatureEdges` | 103 | **ported (2026-11-05, gh:#1276)** | `surface::features::surface_feature_edges` | `triSurfaceDetectFeatureEdges` + `triSurfacePatchManipulator`. |
| `checkSurfaceMesh` | 242 | **ported (2026-11-05, gh:#1277)** | `surface::checks::check_surface_mesh` |  |
| `copySurfaceParts` | 87 | **ported (2026-11-05, gh:#1277)** | `surface::cleanup::copy_surface_parts` |  |
| `extrudeEdgesInto2DSurface` | 82 | not ported | — |  |
| `importSurfaceAsSubset` | 89 | **ported (2026-11-05, gh:#1277)** | `surface::cleanup::import_surface_as_subset` |  |
| `mergeSurfacePatches` | 402 | **ported (2026-11-05, gh:#1277)** | `surface::cleanup::merge_surface_patches`, `patch_ids` |  |
| `patchesToSubsets` | 91 | **ported (2026-11-05, gh:#1276)** | `surface::features::patches_to_subsets` |  |
| `removeSurfaceFacets` | 80 | **ported (2026-11-05, gh:#1277)** | `surface::cleanup::remove_surface_facets` |  |
| `subsetToPatch` | 178 | **ported (2026-11-05, gh:#1276)** | `surface::features::subset_to_patch` (upstream's intent: its text discards the result) |  |
| `surfaceGenerateBoundingBox` | 185 | ~~not ported~~ **ported (2026-11-05, gh:#1279)** | `anisotropic::surface_generate_bounding_box` |  |
| `FMSToSurface, FMSToVTK, FLMAToSurface, surfaceToFMS, meshToFPMA` | 1318 | not applicable | — | FMS/FLMA/FPMA/VTK file formats. |
| `preparePar` | 117 | not applicable | — | MPI decomposition. |

_Subtotal: 3506 lines._

## 3. Where the port deliberately differs from upstream

These are not gaps. They are different routes to the same stage, so the plan in §4 adds the upstream route **beside** them and compares the two (`AGENTS.md`, differential testing). It does not replace them.

| Stage | Upstream | This crate | What differs |
|---|---|---|---|
| Volume tets | `tetCreatorOctree` + `tetMeshExtractorOctree`: tets from the octree's vertices and cube centres (`tetMesh`, `pMesh`) | `tet::tetrahedralize` (centroid subdivision of carved or graded cells), then `delaunay::flip_to_delaunay` | Different tets and point set. The crate's tets are not Delaunay before the flips (`delaunay` docs). Upstream's `tetMesh` stops at tets; the crate goes on to a dual. **Updated 2026-11-05 (gh:#1272):** the upstream route is now beside it (`tet_creator_octree`, `pipeline::tet_mesh`); measured differences in `verification_and_validation/tet_mesh_gh1272/`. |
| Polyhedral cells | `pMesh`: `voronoiMeshExtractor`, one cell per interior tet vertex, corners at tet **centroids**, no cells at boundary vertices; boundary then mapped and untangled | `dual::polyhedral_dual*` (median dual: edge midpoints, face and cell centroids), `dual::polyhedral_dual_per_point`, `cell_centre_dual` (OpenFOAM `polyDualMesh` through `outram-foam-mesh`); selected by `TetDualOptions::dual_kind` | The crate's duals keep boundary-vertex cells and build boundary faces themselves. `CellCentre` is the nearest to `voronoiMeshExtractor` (same dual corners inside). **Updated 2026-11-05 (gh:#1273):** `voronoi_mesh_extractor` and `pipeline::p_mesh` are now beside them; on the same tets the interior cells equal `CellCentre`'s to 9.8e-15 (box), where `polyDualMesh` accepts the tets. |
| Inside/outside | `meshOctreeInsideOutside`: front marking from the surface-intersecting (DATA) boxes | `carve`, `octree::inside`: ray parity; the upstream route is now beside it (`mesh_octree_creator`, gh:#1264) | Different test. ~~unmeasured near it~~ **Measured 2026-11-05:** the same answer on every leaf that does not cross the surface (sphere, pipe). |
| Background mesh | `meshOctreeCreator` sizing (`maxCellSize`, `boundaryCellSize`, local and object refinement), `cartesianMeshExtractor` + `decomposeSplitHexes` | `carve::carve_box` (uniform), `octree::refine_near_boundary(_banded)` (N levels near the wall), hanging nodes spliced into the face rings; the upstream route is now beside it (`mesh_octree_creator`, `cartesian_mesh`, gh:#1264–#1266), not yet wired into `pipeline` | The crate's route has no size-driven refinement, and its split hexes stay polyhedra. |
| First surface fit | `meshSurfaceMapper::preMapVertices` + `mapVerticesOntoSurface`, then `meshSurfaceOptimizer::untangleSurface` | `snap::snap_to_surface` (one closest-point projection); in the multipatch pipeline `surface_mapper::map_vertices_onto_surface_untangled` | Tangled cells are repaired by **undoing** the moves (`revert_tangled`), not by untangling. |
| Patch assignment | `edgeExtractor` refines nearest-triangle patches | `patches::assign_patches_by_region` (nearest triangle only) | No refinement of the assignment. **Updated 2026-11-05 (gh:#1269):** the upstream route is beside it (`edge_extractor`) and is the default of `pipeline::tet_mesh` / `p_mesh`; the tet-dual pipeline still uses the nearest triangle. Differential in `verification_and_validation/edge_extractor_gh1269/`. |
| Volume smoothing | `meshOptimizer::laplaceSmoother` (`laplacianPC`: point-cell centres) | `smooth::laplacian_smooth` (Freitag's smart Laplacian: edge neighbours, move only if no cell inverts) | Different smoother, not a port. |
| Layer creation | `boundaryLayers`: new vertices, one layer at about half the local edge length, then split by `refineBoundaryLayers` | `layers` (`BackOffOnly`): wall points shrink inward by the requested profile, with per-point caps and a global back-off. `CfMeshOptimise` puts the ported `boundaryLayerOptimisation` on the crate's columns. `SnappyCollapse` is OpenFOAM's | Only the border rules (`layer_setup`) and the optimisation are cfMesh's. All three strategies share `layers::layer_setup` / `build_layered_variable` (crate `CLAUDE.md`). |
| Quality metrics | `polyMeshGenChecks` (cfMesh definitions) | `mesh_optimizer` (cfMesh's, for the optimiser) and `checks` (OpenFOAM's `checkMesh`, for reports and the snappy strategy) | Centres: vertex-average face centre here, area-weighted upstream (`mesh_optimizer` docs). |
| Parallelism | MPI buffer layers and processor patches; OpenMP loops | Serial, deterministic, index order, Jacobi updates | All MPI rows are `not applicable`. |
| Upstream line | — | `boundary_layer_optimisation`, `mesh_optimizer` and `correct_edges` cite DAFoam `e944f6ba695f`. `surface_mapper` cites `6cf1d211f9cf`. The older files cite `wyldckat/cfMesh` v1.0 by directory only | New ports cite `e944f6ba695f`. |

## 4. Proposed porting plan

Every item follows the same rules:
- the upstream provenance header (`e944f6ba695f`);
- predictions written before the runs;
- a new route is opt-in, and the defaults stay unchanged (the maintainer decides);
- meshes are drawn as PNGs (crate `CLAUDE.md`);
- `cargo test --release`.

"Lines" is the size of the upstream files the item touches, including parts already ported. Items are in dependency order within each wave.

### Wave 1: the gh:#1241 gaps

**1. `untangleMeshFV` boundary moves and the remaining `meshOptimizer` drivers**
- **Upstream:** 3,090 lines.
  - `meshOptimizer/optimizeMeshFV.C` (663): the boundary loop of `untangleMeshFV`, `optimizeMeshNearBoundaries`, `optimizeMeshFVBestQuality`.
  - `tetMeshOptimisation/tetMeshOptimisation.C` (657): `optimiseBoundaryVolumeOptimizer(nonShrinking)`, `optimiseBoundarySurfaceLaplace`.
  - `partTetMesh/partTetMesh.{C,H}` + `partTetMeshAddressing.C` (1,700): `BOUNDARY` nodes, the `(mesh, locked, nLayers)` constructor.
  - `polyMeshGenChecksGeometry.C`: `findWorstQualityFaces` (~70).
- **Target:** `mesh_optimizer`.
- **Reuse:** `PartTetMesh`, `volume_optimizer`, `find_bad_faces`, `untangle_mesh_fv_bounded`, `bad_faces_at_the_boundary`. For the symmetric-tensor eigen split use `outram_foam_basic_lib::primitives::eigen::{eigen_values_symm, eigen_vectors_symm}`: it is OpenFOAM's `eigenValues`, as upstream calls it. It is behind the optional `foam-export` dependency today, so the issue decides whether to un-gate it. `petir::linalg::eigen_symmv` (GSL) orders its results differently.
- **Differential:** `CfMeshOptimise` on the gh:#1155 pipe, with boundary moves off (today) and on. Record bad faces left, `bad_faces_at_the_boundary`, and time against the 140–370 s of gh:#1241 item 4.
- **Test:** a carved box with one boundary point pushed inward until a face is bad.
  - Predict interior-only `untangle_mesh_fv` leaves ≥ 1 bad face, and with the boundary loop 0.
  - Points on a flat side stay on its plane to 1e-12 while `nGlobalIter < 2`, and the 8 corners never move.

**2. `laplaceSmoother` (the Laplace pass of `optimizeMeshFV`) and the lock API**
- **Upstream:** `meshOptimizerOptimizePoint.C` (437), `meshOptimizer.C` (275: `lockCellsInSubset` / `Faces` / `Points`, `enforceConstraints`, `removeUserConstraints`), `meshOptimizerI.H` (273). 985 lines.
- **Target:** `mesh_optimizer`.
- **Reuse:** `point_cells`, `cell_faces`, `checks::cell_geometry`. Keep `smooth::laplacian_smooth`; do not merge the two.
- **Differential:** `laplacianPC` against `smooth::laplacian_smooth` on the same graded sphere: quality (`check_quality`) and point displacement.
- **Test:**
  - A uniform `cartesian_box` is a fixed point (no point moves more than 1e-12).
  - One interior point displaced by 0.3 h returns to the mean of its point-cells' centres after one pass (exact by construction).
  - `boundary_layer_optimisation`'s "Laplace pass not ported" note is struck through in the same change.

**3. `meshSurfaceEngine` (serial) and `meshSurfacePartitioner`**
- **Upstream:** `surfaceTools/meshSurfaceEngine` without `ParallelAddressing.C` (3,086), `meshSurfacePartitioner` (513). 3,599 lines.
- **Target:** new `surface_engine`. It is the shared boundary addressing that items 4, 5, 14, 15 and 16 need.
- **Reuse:** the boundary maps already written in `layers::layer_setup` (`point_faces`), `correct_edges` (feature edges, corners, edge points) and `surface_mapper`. Port the engine once and move those callers onto it, one per change, each with its existing tests green. Do not leave a fourth copy.
- **Differential:** the engine's `pointFaces`, corners and edge points against `layer_setup` and `correct_edges` on the pipe.
- **Test:** `carve_box(box_surface(0, 2), 0.5)` (4×4×4 hexes) has:
  - 98 boundary points, 96 boundary faces, 192 boundary edges (Euler);
  - 8 corners and 36 edge points;
  - point normals on the open sides equal to the axis normals to 1e-12.

**4. Surface simplex tools: `partTriMesh`, `surfaceOptimizer`, `meshSurfaceCheckInvertedVertices`**
- **Upstream:** `meshes/partTriMesh` without `ParallelAddressing.C` (1,331), `advancedSurfaceSmoothers/surfaceOptimizer` (545), `surfaceTools/meshSurfaceCheckInvertedVertices` (555). 2,431 lines.
- **Target:** new `surface_optimizer`.
- **Reuse:** `surface_engine` (item 3), and the `PartTetMesh` pattern (index arrays, Jacobi updates).
- **Differential:** `meshSurfaceCheckInvertedVertices` against `surface_mapper::cells_with_nonpositive_pyramids` on a mapped pipe. They test different things (points against cells), so record the overlap rather than requiring equality.
- **Test:**
  - A planar 6-triangle fan with its centre dragged outside the hexagon's kernel. `findInvertedVertices` flags exactly that point.
  - `surfaceOptimizer::optimizePoint` returns a point where all 6 triangle normals agree with the plane normal.
  - On the regular hexagon the centre is a fixed point to 1e-12.

**5. `meshSurfaceOptimizer` (serial, 3D): `untangleSurface`, `optimizeSurface`, wired in**
- **Upstream:** `smoothers/geometry/meshSurfaceOptimizer` without `OptimizePointParallel.C` and without `optimizeSurface2D` and `untangleSurface2D` (≈2,000 of 2,266), `meshOptimizerOptimizeSurface.C` (55). About 2,050 lines.
- **Target:** `surface_optimizer`. The wiring is opt-in, so the defaults stay unchanged:
  - `surface_mapper` (`untangleSurface` after `mapVerticesOntoSurface`);
  - `boundary_layer_optimisation::optimise_layer` (`optimizeSurface(2)`);
  - `mesh_optimizer` (`meshOptimizer::optimizeSurface`).
- **Reuse:** items 3–4, `surface_mapper::map_impl`, `correct_edges`.
- **Differential:** `untangleSurface` against `surface_mapper::revert_tangled` on the 24- and 48-sided pipes at h 4 and 5 mm, with `correct_edges` on and off. Record boundary points left unmapped (105 on the 24-sided pipe on 2026-11-04 before `correct_edges`; not re-measured), inlet area / πR², and cells with non-positive pyramids.
- **Test:** on the 24-sided pipe, h 4 mm:
  - predict fewer unmapped points with `untangleSurface` than with `revert_tangled`;
  - both meshes `validate()`, with no non-positive pyramid.

**6. LP `meshUntangler`**
- **Upstream:** `advancedSmoothers/meshUntangler` (1,233), `helperClasses/sortEdgesIntoChains` (425, used by `cutRegion`), `tetMeshOptimisation::optimiseUsingMeshUntangler` (~140). About 1,800 lines.
- **Target:** `mesh_optimizer`. It is the middle smoother of `untangleMeshFV`.
- **Reuse:** `PartTetMesh::smooth_nodes` and the simplex signature `(pts, tets, free)`.
- **Differential:** `knupp_metric`, `mesh_untangler` and `volume_optimizer` on the same simplices (the existing inverted-star fixture), and `untangleMeshFV` with the untangler on and off on the pipe. Record bad faces and time (gh:#1241 item 4: "compare against the unported parts before writing anything new").
- **Test:**
  - The feasible region of a simplex whose opposite-face planes bound the corner cube `x, y, z > 0, x + y + z < 1` is found, and the returned point is inside it.
  - On a perturbed star with a non-empty feasible region, every tet volume is > 0 after `mesh_untangler`. That is the LP guarantee, which `knupp_metric` does not give.
  - On an infeasible star the point is left where it was.

**7. `polyMeshGenChecks` completion and `meshSubsets`**
- **Upstream:** `polyMeshGenChecks{Geometry,Topology}.C` (the functions the §2 row D lists as missing; the files total 3,952), `meshes/primitives/meshSubsets` (442). About 2,500 new lines.
- **Target:** `mesh_optimizer` for the cfMesh-definition checks, a `subsets` field on `VolumeMesh` (point/face/cell sets by name), and `checks`.
- **Reuse:** `checks::faces_failing_mesh_quality` already has OpenFOAM's twist, determinant and volume ratio. Share their geometry helpers; keep both definitions, cfMesh's and OpenFOAM's.
- **Differential:** cfMesh against OpenFOAM `checkMinTwist`, `checkCellDeterminant` and `checkMinVolRatio` on the pipe mesh: count faces flagged by each and the faces flagged by only one.
- **Test:**
  - A unit cube: twist 1, determinant 1, volume ratio 1.
  - One face folded by a known angle θ gives twist cos θ to 1e-12.
  - `checkUpperTriangular` fails on a mesh with two internal faces swapped.

### Wave 2: the upstream workflows

**8. Octree cubes and surface intersections**
- **Upstream:** `meshOctreeCube` (3,078), `meshOctree.{C,H}` + `meshOctreeI.H` (860), `meshOctreeSlot.H` + `meshOctreeCubePatches.C` (134). 4,072 lines.
- **Target:** `octree`. Extend `Cell`, `children` and `balance_2to1`; do not add a second octree.
- **Reuse:** `octree::Cell`, the `BTreeSet` leaves (determinism pin `pipeline::tests::octree_path_is_pinned_bit_for_bit`).
- **Differential:** DATA leaves (cube ∩ surface) against the leaves `refine_near_boundary` selects on the sphere.
- **Test:** on a 4-level octree over `box_surface` and `sphere_surface`, the DATA set equals a brute-force triangle–box overlap (separating axis) over every leaf, exactly.

**9. Octree queries and modifier**
- **Upstream:** `meshOctreeFindNearestSurfacePoint.C` (578), `meshOctreeNeighbourSearches.C` (546), `meshOctreeInsideCalculations.C` (185), `meshOctreeModifier` serial (1,469). 2,778 lines.
- **Target:** `octree`. Then route `patches::nearest_triangle`, `snap::closest_point_on_surface` and `surface_mapper` through it as an option, keeping the brute force callable.
- **Differential:** octree query against brute force on the same points.
- **Test:** on the 48-sided pipe and the sphere, for 10,000 sample points:
  - the nearest point and the nearest triangle are bit-identical (ties broken by the lower triangle index on both routes);
  - record the time of both.

**10. `meshOctreeCreator`, `meshOctreeInsideOutside`, frontal marking**
- **Upstream:** `meshOctreeCreator` serial (2,118), `meshOctreeInsideOutside` (955), `helperFunctionsFrontalMarking` (653). 3,726 lines.
- **Target:** `octree`.
- **Reuse:** `refine_with`, `balance_2to1`, item 8–9.
- **Differential:** inside/outside against `octree::inside` (ray parity) on every leaf of the sphere and the pipe; cell count against `refine_near_boundary_banded` for the README's sphere table.
- **Test:** sphere R 3, `maxCellSize` 1.0, `boundaryCellSize` 0.25:
  - every DATA leaf has size 0.25;
  - every non-DATA leaf is classed as its centre's `|x| < R`.

**11. Object and patch refinement**
- **Upstream:** `refinementControls/objectRefinement` (2,572), `patchRefinement` (312). 2,884 lines.
- **Target:** `octree`, and options on a new `MeshDict`-like struct (not a dictionary reader).
- **Reuse:** item 10.
- **Differential:** none in the crate; the check is analytic.
- **Test:** sphere/box/cone/line sources on a cube domain. Leaves whose centre lies in the object reach the requested level, no other leaf does, and the 2:1 balance holds.

**12. `meshOctreeAddressing` and `cartesianMeshExtractor`**
- **Upstream:** `meshOctreeAddressing` serial (3,118), `cartesianMesh/cartesianMeshExtractor` (800). 3,918 lines.
- **Target:** new `cartesian_mesh`.
- **Reuse:** `VolumeMesh`, `from_cell_faces`.
- **Differential:** against `octree::build_mesh` on the same leaves.
- **Test:**
  - Same total volume to 1e-12 relative.
  - Same cell count with `decomposeSplitHexes` off.
  - `validate()` holds.
  - With it on, every split hex becomes tets and pyramids of the same volume.

**13. Surface preparation**
- **Upstream:** `surfaceTools/surfaceMorpherCells` (1,139), `smoothers/topology/topologicalCleaner` (857), the `decomposeCells` driver (`decomposeMesh`, `addNewCells`, `removeDecomposedCells`: ~470 not yet ported), `faceDecomposition/faceDecomposition` (422). About 2,890 lines.
- **Target:** new `topology`.
- **Reuse:** move `correct_edges`' pyramid decomposition (`pyramids`, `checkFaceConnections`, `findTopVertex`) into it as one shared function; `correct_edges` then calls it.
- **Differential:** `correct_edges` before and after the move must be bit-identical (pin with its existing tests).
- **Test:** after `surfaceMorpherCells` on the carved sphere:
  - every boundary cell has exactly one boundary face;
  - the mesh is closed;
  - the volume equals the sum of the kept cells.

**14. cartesianMesh connectivity checks**
- **Upstream:** `checkIrregularSurfaceConnections` (1,153), `checkBoundaryFacesSharingTwoEdges` (649), `checkCellConnectionsOverFaces` (369), `checkNonMappableCellConnections` (437). 2,608 lines.
- **Target:** `topology`.
- **Reuse:** `surface_engine`.
- **Differential:** none.
- **Test:** hand-built meshes:
  - two cubes touching at one vertex are split or removed as upstream does;
  - a boundary face sharing two edges with one neighbour is repaired;
  - a clean box is left bit-identical.

**15. `edgeExtractor`**
- **Upstream:** `edgeExtraction/edgeExtractor/edgeExtractor.{C,H}` (3,050). `edgeExtractorCorners.C` (1,802) is item 15b.
- **Target:** `surface_mapper` / `patches`.
- **Reuse:** `patches::assign_patches_by_region` as the initial assignment (upstream also starts from the nearest patch), `surface_engine`, item 9.
- **Differential:** the patch assignment from `edgeExtractor` against nearest-triangle on the 24- and 48-sided pipes. Record the faces that change patch and the inlet area / πR².
- **Test:** on a box with three named patches meeting at a corner, every boundary face takes the patch of the side it lies on, including the faces the nearest-triangle rule misassigns at the rim. Count both.

**16. cfMesh `boundaryLayers` (layer creation)**
- **Upstream:** `boundaryLayers/boundaryLayers` (4,229).
- **Target:** `layers`, as a new creation route beside the three strategies.
- **Reuse:** `layers::layer_setup` for patch and border classification (the crate rule: one copy of the column geometry). State in the doc comment where cfMesh's vertex creation genuinely differs.
- **Differential:** against `add_boundary_layers_adaptive_on_patches` on the box and the pipe. Record the volume (both must conserve it), layer cell count (one per layer face), first-layer thickness distribution, and `check_quality`.
- **Test:** a carved box with `walls` on four sides. Exactly one layer cell per wall face, of thickness half the local edge length (upstream's rule as `layers` docs state it; re-read `boundaryLayersCreateVertices.C` before fixing the number), with the ends re-partitioned and not moved.

**17. `refineBoundaryLayers` and `detectBoundaryLayers`**
- **17a:** `refineBoundaryLayers.{C,H}`, `Functions.C`, `Faces.C` (3,125).
- **17b:** `refineBoundaryLayersCells.C` (2,041) and `detectBoundaryLayers` (989), 3,030.
- **Target:** `layers`.
- **Reuse:** `layers::build_layered_variable`.
- **Differential:** on straight one-cell columns, the split points from `refineBoundaryLayers` against `build_layered_variable` for the same `nLayers` and `thicknessRatio`, to 1e-12.
- **Test:**
  - Per-patch `nLayers` 3 on `walls` and 1 elsewhere gives the predicted cell count.
  - `maxFirstLayerThickness` caps the first layer.
  - `detectBoundaryLayers` on a mesh from item 16 finds exactly the layer cells.

**18. `tetMesh` as upstream**
- **Upstream:** `tetMeshGenerator` (558, the rest), `tetCreatorOctree` (1,678), `tetMeshExtractorOctree` (386), `executables/tetMesh` (65), `workflowControls` (477), `renameBoundaryPatches` (392), `polyMeshGenModifierRenumberMesh.C` (282). 3,838 lines.
- **Target:** `pipeline::tet_mesh`, a new route beside `surface_to_tet_dual_mesh`.
- **Reuse:** items 1–17.
- **Differential:** `tet_mesh` against `tetrahedralize` + `flip_to_delaunay` on the sphere and the pipe at equal cell size. Record tet count, volume error against the analytic value, `check_quality`, and run time.
- **Test:** sphere R 3:
  - all tets positive;
  - closed;
  - volume within the README's uniform 0.5 m error (1.72 %), or the miss recorded.

**19. `pMesh` (Voronoi)**
- **Upstream:** `voronoiMeshExtractor` (674), `voronoiMeshGenerator` (630), `executables/pMesh` (65), `meshSurfaceMapperPremapVertices.C` (302). 1,671 lines.
- **Target:** `pipeline::p_mesh`.
- **Reuse:** item 18's tets, `cell_centre_dual`.
- **Differential:** `voronoiMeshExtractor` against `DualKind::CellCentre` and `MedianPerCorner` on the **same** `tetCreatorOctree` tets. Predict the same interior cells as `CellCentre` (same corners); record any difference.
- **Test:** interior cell count equals the number of tet vertices with no boundary edge; volume conserved over those cells.

**20. `cartesianMesh` as upstream**
- **Upstream:** `cartesianMeshGenerator` (580), `executables/cartesianMesh` (62).
- **Target:** `pipeline::cartesian_mesh`.
- **Reuse:** items 8–17.
- **Differential:** against `carve` + `snap` and against the tet-dual pipeline on the sphere and the pipe. Record volume error, `check_quality`, cells, time, and the laminar pipe of gh:#1155 if it runs.
- **Test:**
  - The sphere at `maxCellSize` 0.5 is all hexes away from the wall.
  - `validate()` holds, with no non-positive pyramid.
  - Volume error is recorded against 113.0973 m³.
- **Note:** this item is small (642 lines) because its parts are items 8–17.

**21. 2D workflow**
- **Upstream:** about 3,610 lines.
  - `cartesian2DMeshGenerator` (536), `executables/cartesian2DMesh` (66);
  - `polyMeshGen2DEngine` (689), `meshSurfaceMapper2D` (1,189), `meshSurfaceEdgeExtractor2D` (311);
  - `triSurface2DCheck` (264), `triSurfaceExtrude2DEdges` (201), `utilities/extrudeEdgesInto2DSurface` (82);
  - `meshSurfaceOptimizer::optimizeSurface2D` and `untangleSurface2D` (~270, `meshSurfaceOptimizerOptimizeSurface.C` L663–934).
- **Target:** `pipeline::cartesian_2d_mesh`.
- **Differential:** against item 20 on the extruded slab.
- **Test:** a 2D channel. One cell thick, front/back faces exactly on z = 0 and z = Δz, and the x–y layout identical to item 20's mid-plane section.

### Wave 3: surface tools and utilities

**22. Surface features and patching**
- **Upstream:** 3,590 lines.
  - `triSurfaceDetectFeatureEdges` (277), `triSurfacePatchManipulator` (509), `triSurfaceClassifyEdges` (575), `triSurfacePartitioner` (598);
  - `triSurf` addressing and feature edges (~1,530 of 2,907);
  - `utilities/surfaceFeatureEdges` (103).
- **Target:** new `surface`, with a `TriSurf` that extends `SurfaceRegions`.
- **Reuse:** `outram-foam-mesh`'s `snappy_hex_mesh::snapping::detect_feature_edges` (private today; make it `pub` there). This crate may depend on `outram-foam-mesh` (it does, optionally) but not on `outram-blender` (`measure::sharp_edges`).
- **Differential:** feature edges against `detect_feature_edges` at the same angle on the pipe and the box.
- **Test:** an unlabelled pipe surface split at 45° gives exactly 3 patches (wall, two ends), and the rims are 2 × 48 feature edges.

**23. Surface checks and clean-up**
- **Upstream:** 3,974 lines.
  - `triSurfaceChecks` (1,253), `triSurfaceCleanupDuplicates` (454), `triSurfaceCleanupDuplicateTriangles` (235);
  - `triSurfaceCopyParts` (412), `triSurfaceRemoveFacets` (377), `triSurfaceImportSurfaceAsSubset` (343);
  - utilities `checkSurfaceMesh` (242), `copySurfaceParts` (87), `removeSurfaceFacets` (80), `importSurfaceAsSubset` (89), `mergeSurfacePatches` (402).
- **Target:** `surface`.
- **Differential:** `triSurfaceChecks` open-edge and orientation results against `shapes::surface_volume`'s sign and `patches::SurfaceRegions::validate`.
- **Test:** a box soup with one duplicated point, one duplicated triangle and one flipped triangle. Each is reported and cleaned, and the volume is then 8 exactly.

**24. Automatic refinement**
- **Upstream:** `meshOctreeAutomaticRefinement` (976), `triSurfaceCurvatureEstimator` (1,229), `quadricFitting` (487). 2,692 lines.
- **Target:** `octree`, `surface`.
- **Differential:** none in the crate.
- **Test:** curvature of `sphere_surface` R against 1/R (record the error at 24×48 and 48×96). Proximity refinement between two parallel plates at gap g refines to the level whose cell is below g/`nCells`.

**25. Anisotropic meshing and scaling**
- **Upstream:** `anisotropicMeshing` (2,559), utilities `scaleMesh` (83), `scaleSurfaceMesh` (81), `surfaceGenerateBoundingBox` (185). 2,908 lines.
- **Target:** new `anisotropic`.
- **Test:** `boxScaling` and `planeScaling` modify-then-revert return every point to 1e-12, and a box scaled ×2 in x has twice the volume.

**26. Layer and quality utilities**
- **Upstream:** 3,314 lines.
  - `extrudeLayer` (1,915), `triangulateNonPlanarBaseFaces` (357), `symmetryPlaneOptimisation` (405);
  - utilities `generateBoundaryLayers` (158), `improveMeshQuality` (145), `improveSymmetryPlanes` (65), `patchesToSubsets` (91), `subsetToPatch` (178).
- **Target:** `layers`, `mesh_optimizer`, `pipeline`.
- **Test:**
  - `improveMeshQuality` on the dented box equals item 1's result.
  - `symmetryPlaneOptimisation` puts every point of a perturbed symmetry patch back on the plane to 1e-12.
  - `patchesToSubsets` followed by `subsetToPatch` is the identity.

**27. Fundamental sheets (optional)**
- **Upstream:** `createFundamentalSheets` (908), `meshSurfaceEdgeExtractorFUN` (407), `helperFunctionsTopologyManipulation` (829). 2,144 lines.
- **Value:** low; the FUN extractor is an alternative to items 15–15b.
- **Differential:** FUN against `edgeExtractor` patches on the pipe.

**Not planned:**
- the `not applicable` rows;
- `quadricMetric`, `meshSurfaceEdgeExtractor`, `findCellsIntersectingSurface` and `polyMeshGenModifierZipUpCells.C`: no caller in `meshLibrary/` at this commit (grep);
- ~~`checkMeshDict`: there is no `meshDict` reader.~~ **Ported 2026-11-11 (gh:#1476)** with the reader.

~~If a `meshDict` reader is wanted, it is a new item after 20.~~ It was gh:#1476.

## 5. Progress against the plan

### Items 1, 2, 6, 7 (gh:#1255, #1256, #1260, #1261), 2026-11-05

Each item lands opt-in: the default pipeline's output is unchanged. The pins
`back_off_only_layers_are_pinned_bit_for_bit`, `octree_path_is_pinned_bit_for_bit`
and `layer_strategies_are_deterministic` pass after every item. The pipe
differentials are in
`crates/outram-foam-appbuilder-lib/verification_and_validation/laminar_pipe_gh1155.md`
(sections gh:#1255/#1256 and gh:#1260). Every prediction was written and
committed before its run, and the misses are recorded where they occurred.

- **Item 1 (gh:#1255), `mesh_optimizer`:**
  - `UntangleControls`, `untangle_mesh_fv_with` (the boundary loop),
    `PartTetMesh::{near_boundary, optimise_boundary_volume_optimizer,
    optimise_boundary_surface_laplace}`, `optimize_mesh_near_boundaries`,
    `face_dot_products`, `face_skewness`, `find_worst_quality_faces`,
    `optimize_mesh_fv_best_quality`;
  - control `BoundaryLayerOptimisationControls::untangle_boundary_moves`;
  - the eigen split reuses `outram_foam_basic_lib::primitives::eigen`. That
    dependency is now non-optional; the crate still checks for
    `wasm32-unknown-unknown` and `aarch64-linux-android` (`--lib`).
  - Tests: `boundary_moves_repair_a_dent_interior_moves_cannot`,
    `optimize_mesh_near_boundaries_moves_only_the_near_wall_region`,
    `worst_quality_faces_and_best_quality_optimiser`.
- **Item 2 (gh:#1256), `mesh_optimizer`:**
  - `laplace_smoother` (`laplacian`, `laplacianPC`, `laplacianWPC`),
    `laplace_smoother_surface`, `vertex_type`, `MeshOptimizer` (the lock
    API, `removeUserConstraints`, `enforceConstraints` →
    `ConstraintViolation`, `optimizeMeshFV`);
  - control `BoundaryLayerOptimisationControls::optimize_mesh_fv_laplace`.
  - Tests: `laplace_smoother_fixed_point_and_one_pass`,
    `lock_api_and_enforce_constraints`; differential
    `diagnose_laplace_pc_against_smart_laplacian`.
- **Item 6 (gh:#1260), new module `mesh_untangler`:**
  - `CutRegion`, `Plane`, `mesh_untangler`, `sort_edges_into_chains`;
  - `PartTetMesh::optimise_using_mesh_untangler`, `UntangleControls::mesh_untangler`;
  - control `BoundaryLayerOptimisationControls::untangle_mesh_untangler`.
  - Tests: `sort_edges_into_chains_closes_faces`,
    `cut_region_of_the_corner_simplex`, `mesh_untangler_lp_guarantee`,
    `three_smoothers_on_the_same_simplices`,
    `untangle_with_and_without_the_lp_untangler`.
- **Item 7 (gh:#1261), new modules `poly_mesh_gen_checks` and
  `mesh_subsets`:**
  - every `polyMeshGenChecks` function, `MeshQualitySettings`,
    `find_bad_faces_with`;
  - `MeshSubsets`, and the subset locks and recorded `badFaces`,
    `badCells`, `badPoints` on `MeshOptimizer`.
  - Tests: `analytic_checks`, `upper_triangular_detects_a_swap`,
    `find_bad_faces_with_default_settings_is_find_bad_faces`,
    `subsets_follow_upstream`, `subset_locks_and_recorded_subsets`;
    differential `pipeline::tests::diagnose_cfmesh_against_openfoam_checks`.

**Findings on the way, not fixed (each would change a default):**
- `optimise_using_knupp_metric` counts its passes differently from
  upstream (doc comment);
- this crate's meshes are not in OpenFOAM's upper-triangular face order
  (`cartesian_box` numbers its internal faces by direction).

**Upstream defects found:**
- `lockPointsInSubset` calls `lockCells` with point labels. The port locks
  the points instead and says so.
- `optimiseUsingMeshUntangler` never re-marks inverted tetrahedra, so it is
  one pass. Kept, documented.

**Differential results (2026-11-05).**

- **gh:#1256, `laplacianPC` against `smooth::laplacian_smooth`.** Setup: the
  graded sphere, R 3, cell 1.0, one level within 1.0 of the wall; 1 381
  cells, 3 791 inside points; five passes each.

  | quantity | unsmoothed | `laplacianPC` | smart Laplacian |
  |---|---|---|---|
  | max non-orthogonality | 83.8° | 59.4° | 73.7° |
  | max skewness | 0.571 | 0.366 | 0.464 |
  | `findBadFaces` | 336 | 0 | 0 |
  | inverted cells | 0 | 0 | 0 |
  | mean point displacement | — | 0.111 | 0.076 |
  | max point displacement | — | 0.240 | 0.352 |

  Every prediction held: 0 inverted cells for both, both lower than the
  unsmoothed mesh, mean displacements within a factor of 3. cfMesh's
  unconditional point-cell Laplacian does better here than the smart one.
  The smart Laplacian refuses any move that inverts a cell, and so stops
  short.
- **gh:#1261, cfMesh against OpenFOAM checks.** Setup: the 48-sided pipe at
  h 5 mm, default dual, bare (3 113 cells) and with 3 back-off layers
  (26 009 cells); OpenFOAM's thresholds for both.

  | check | bare: cfMesh / OpenFOAM, only cfMesh / only OpenFOAM | 3 layers: same columns |
  |---|---|---|
  | volume ratio 0.01 | 0 / 0, 0 / 0 | 2 536 / 2 536, 0 / 0 |
  | twist 0.02 | 152 / 152, 0 / 0 | 160 / 160, 0 / 0 |
  | determinant 0.001 | 0 / 0, 0 / 0 | **0 / 352**, 0 / 352 |

  Volume ratio and twist agree face for face (held). The dual has no
  boundary triangles, so the predicted cfMesh-only boundary-triangle faces
  could not occur. The determinant agrees on the bare mesh. With layers,
  OpenFOAM flags 352 faces and cfMesh none (held: OpenFOAM ≥ cfMesh).
  OpenFOAM drops a wall cell's boundary faces from its tensor; cfMesh keeps
  them, and a thin prism with its wall face counted is not degenerate.
- **gh:#1260, the per-point smoothers on the same simplices
  (`three_smoothers_on_the_same_simplices`).** Smallest tetrahedron volume
  on the jittered star:

  | smoother | min volume |
  |---|---|
  | Knupp | 0.1522 |
  | volume optimiser | 0.1418 |
  | LP untangler | 0.1386 |

  All three are positive. The predicted ordering "LP ≥ Knupp" **missed**.
  On the pipe, see the gh:#1260 section of the V&V record.

### Items 18 and 19 (gh:#1272, #1273), 2026-11-05

New entry points; no default changed. The three pins
(`back_off_only_layers_are_pinned_bit_for_bit`,
`octree_path_is_pinned_bit_for_bit`, `layer_strategies_are_deterministic`)
pass. Predictions were committed before the runs; the records with every
miss are `verification_and_validation/tet_mesh_gh1272/README.md` and
`verification_and_validation/p_mesh_gh1273/README.md`, with PNG slices.

- **Item 18 (gh:#1272), `tetMesh`:**
  - new modules `tet_creator_octree`, `tet_mesh_extractor_octree` (with the
    shared `polyMeshGen` assembly: owners from cells,
    `reorderBoundaryFaces`, `removeUnusedVertices`), `workflow_controls`,
    `rename_boundary_patches` (with `replaceBoundary`), `renumber_mesh`;
  - `pipeline::tet_mesh::{tet_mesh, tet_mesh_restart, TetMeshOptions,
    TetMeshReport, SavedMesh, create_octree}`.
  - `edgeExtraction` calls the ported `meshSurfaceEdgeExtractorNonTopo`
    route (`correct_edges` + `surface_mapper`).
  - **Gaps, called through what exists and noted per run:**
    ~~`surfaceMorpherCells` + `topologicalCleaner` (topology track),
    `meshSurfaceOptimizer` and `meshOptimizer::optimizeSurface` (surface
    track),~~ ~~`edgeExtractor`,~~ `boundaryLayers` / `refineBoundaryLayers`
    (layers track), `symmetryPlaneOptimisation`. **CORRECTED 2026-11-05:**
    the struck steps are called since gh:#1267 and gh:#1259 merged
    (`edgeExtractor` since gh:#1269).
  - **Differential (sphere 0.5 m / pipe 2 mm), against `tetrahedralize` +
    `flip_to_delaunay`:** volume error ~~+26.7 % / +22.3 %~~ **−1.36 % /
    −0.29 %** against −1.72 % / −0.54 %; cells ~~13 968 / 35 416~~
    **12 840 / 31 136** against 21 888 / 57 600; the octree template's
    non-orthogonality is 0. The struck numbers were measured before
    gh:#1267 / #1259 landed, with the boundary left on the DATA leaves.
- **Item 19 (gh:#1273), `pMesh`:** new module `voronoi_mesh_extractor`;
  `pipeline::p_mesh::{p_mesh, p_mesh_restart}`.
  - On the same tets the Voronoi cells are exactly the interior tet
    vertices, and equal `CellCentre`'s cells to 9.8e-15 where
    `polyDualMesh` accepts the tets (box). It refuses the sphere and pipe
    templates (96 / 2 240 non-manifold boundary points).
  - The workflow: ~~−0.97 % (sphere) and −0.05 % (pipe)~~ **−1.54 % and
    −0.35 %** (after gh:#1267 / #1259 were wired in) volume error, no
    negative cell. The pipe's maximum non-orthogonality rose to 75.9° on
    that re-run (not yet located).

**Findings on the way, not fixed:**
- the octree tet template has non-manifold boundary points, made by
  upstream's `createTetsFromFacesWithCentreNode` in coarse `BOUNDARY`
  leaves (as written upstream);
- `meshSurfaceMapperPremapVertices.C` (item 19's list) belongs to the
  surface track and is not ported.

**Upstream defects found:**
- `renumberMesh` is a depth-first walk, not Cuthill–McKee (its comment);
- `tetCreatorOctree` reads possibly empty `VRWGraph` rows and `-1` labels
  unguarded (counted here, zero on every run);
- `workflowControls` records each step twice and never stops after the
  last step.
### Items 13 and 14 (gh:#1267, #1268), 2026-11-05

New module `topology`, opt-in (no pipeline stage calls it; the defaults
are unchanged and the four pins pass).

- **Item 13 (gh:#1267):** `TopoMesh` (the `polyMeshGen` working form, with
  `removeCells`, `removeFaces`, `replaceBoundary`, `removeUnusedVertices`),
  `decompose_cells` (the `decomposeMesh` driver, now the one shared copy:
  moved from `correct_edges`, where the octree track had split it out for
  `cartesian_mesh`), `decompose_mesh_faces`,
  `decompose_concave_internal_faces`, the `faceDecomposition` functions,
  `surface_morpher_cells`, `topological_cleaner`.
  - **Differential (the move):** `correct_edges` before and after,
    bit for bit. `correct_edges_is_pinned_bit_for_bit` fingerprints the
    corrected pipe mesh (L 60 mm, h 6 mm, `MedianPerPoint`/`Corners`:
    1 727 cells, 8 concave faces, 16 split, 112 fanned, 14 cells into
    pyramids). Recorded before the move, asserted after: held; the
    `cartesian_mesh` tests also pass unchanged.
  - **Tests:** `face_decomposition_by_hand`,
    `face_fans_and_cell_removal_on_a_box`,
    `surface_morpher_cells_on_the_carved_sphere`,
    `topological_cleaner_on_the_morphed_sphere_and_a_box`,
    `diagnose_morpher_volume_by_step`.
  - **Results against the plan's test** (carved sphere R 3 m, h 0.6 m,
    552 cells). After the morpher, every boundary cell has exactly one
    boundary face (held), the mesh is closed (held), and the volume equals
    the sum of the kept cells (held; none was removed).
  - **Misses, recorded in the tests:**
    - The volume fell from 119.232 to 93.024 m³ (predicted under 1 %).
      Its cause is located: `morphInternalFaces` conserves the volume
      exactly, while `morphBoundaryFaces` merges a staircase corner's faces
      into one non-planar face. A centroid fan measures that face as a
      chamfer.
    - `cartesian_box` has six patches, not one, so `removeCells`
      collapses them into `defaultFaces`. This is upstream behaviour; the
      prediction misread the input.
  - **Images:** `verification_and_validation/topology_gh1267/`.
- **Item 14 (gh:#1268):** `irregular_vertices`,
  `check_and_fix_irregular_connections`, `BoundaryFacesSharingTwoEdges`,
  `non_mappable_cells`, `remove_non_mappable_cells`,
  `check_cell_connections_over_faces` (through the octree track's
  `group_marking`).
  - **Tests:** `irregular_vertex_between_two_cubes`,
    `boundary_faces_sharing_two_edges_by_hand`,
    `non_mappable_cell_connections`, `clean_box_is_left_bit_identical`.
    Every prediction held.
  - **Not reached by a test:** the face-fanning branch of
    `improve_topology`. It needs a point in two boundary faces that is
    also in an internal face without boundary edges on both sides; no
    hand-built manifold mesh gave one.

**Upstream defect found (kept, documented):**
`checkBoundaryFacesSharingTwoEdges::improveTopology` writes
`decomposeCell[owner[faceI]];` without assigning, so no cell is
decomposed.

**Upstream quirk (harmless):**
`checkInvalidConnectionsForVerticesCells` appends one copy of the point
per group and relabels from the second copy on, so the first copy is
never used.

### Items 16 and 17 (gh:#1270, #1271), 2026-11-05

New opt-in routes `layers::boundary_layers` and `layers::refine_boundary_layers`. No default
changes; the four pins pass. They reuse `layer_setup` and `build_layered_variable` (through
`build_layered_variable_with_columns`, the same code path), with no second copy of the column
geometry. Results and methodology are in the gh:#1270 and gh:#1271 sections of
`crates/outram-foam-appbuilder-lib/verification_and_validation/laminar_pipe_gh1155.md`.

- **Item 16 (gh:#1270):** `boundary_layers_add_layer`, `BoundaryLayersControls`,
  `LayerPoint`, `BoundaryLayersOutcome`.
  - **Box:** every prediction held. One cell per wall face; columns 0.5 h, or 0.9 h/√2 at the
    8 corners; the ends re-partitioned, not moved.
  - **Pipe differential against `BackOffOnly`:** at every `PATCHNODE` the ratio is exactly
    the crate's back-off factor (0.36), to 1e-15. cfMesh's unchecked layer inverts 56 cells
    on the short pipe and 304 on the Foam Studio pipe.
  - **Miss:** the volume moves by 2.5e-7 and −4.7e-7 on the Foam Studio pipe. It is located
    in the non-planar end patches (0.47 mm spread on the default dual). The `layers` docs'
    "preserves the mesh volume exactly" is corrected.
- **Item 17 (gh:#1271):** `detect_boundary_layers` (ported whole) and
  `refine_boundary_layers` (settings, `analyseLayers`, `generateNewVertices`, and the prism
  split).
  - **Split points against `build_layered_variable`:** within 4.6e-15 of the column length,
    and 97 % bit for bit.
  - **Detect:** finds exactly the layer cells.
  - **Misses, recorded:**
    - the inverted cells come from the input;
    - the tolerance of the first-layer cap is 3e-12, from cancellation;
    - the box forms 3 groups, not 1, and refines rather than being refused.
- **Not ported:** the patch-wise `boundaryLayers` route; `refineEdgeHexCell` and
  `refineCornerHexCell` (refused as `NotPorted`); the special mode.

**Upstream quirks ported as written:**
- `createNewVertex` sets `dist = 0.9 dst` when `dst < dist`;
- a two-patch corner moves half the edge whatever `blHeightFactor` is;
- `nDistinctLayers` counts groups emptied by the patch clearing.

### Items 15 and 15b (gh:#1269), 2026-11-05

New modules `edge_extractor` (`edgeExtractor`, its `faceEvaluator`) and
`mesh_surface_edge_extractor_fun` (`meshSurfaceEdgeExtractorFUN` without
`createFundamentalSheetsJFS`); `surface_mapper::map_points_onto_patches`
(`mapVerticesOntoSurfacePatches(nodesToMap)`, the whole-surface mapping now
goes through it unchanged). Wired into `pipeline::tet_mesh` / `p_mesh` in
upstream's order: this **changes their default output** (as upstream's
workflow does); `TetMeshOptions::patch_assignment =
PatchAssignment::NearestTriangle` is the ablation. The tet-dual pipeline is
unchanged. Pins passing: `back_off_only_layers_are_pinned_bit_for_bit`,
`dual_kinds_are_pinned_bit_for_bit`, `octree_path_is_pinned_bit_for_bit`,
`layer_strategies_are_deterministic`, `correct_edges_is_pinned_bit_for_bit`,
`tet_mesh_stop_and_restart_reproduce_the_full_run`. Predictions were
committed before the runs; record with the misses:
`verification_and_validation/edge_extractor_gh1269/README.md`, with PNGs.

- **Item 15b** (`edgeExtractorCorners.C`): the `faceEvaluator` half is
  ported (the workflows call it). `cornerEvaluator`, `findCornerCandidates`
  and `checkCorners` have no caller; `checkCorners` calls `::exit(0)`
  part-way. Not ported.
- **Differential, Foam Studio pipe (`CellCentre` + `Corners`, h 4/5/6
  mm):** 16 / 0 / 5 faces change patch, all at the rims; inlet
  `|Σ S_z|/πR²` 0.98830 → 0.98765, 0.98760 → 0.98762, 0.97508 → 0.97486
  (predicted closer to the 48-gon's 0.99715: **missed**). At 5 mm the
  normal-alignment and topology steps move the same 16 faces and back.
- **`tet_mesh` pipe:** −0.289 % → −0.291 %, 11 faces moved; **`p_mesh`
  pipe:** −0.35 % → −0.325 %, max non-orthogonality 75.9° → 55.3° (64.5°
  from FUN's order alone).

### Items 22, 23 and 24 in part (gh:#1276, #1277, #1278), 2026-11-05

New module `surface` (`features`, `checks`, `cleanup`, `curvature`) on the
crate's one surface type: `mesh_octree::TriSurf` is extended with upstream's
patch list, stored feature edges, point/facet/feature-edge subsets and the
rest of `triSurfAddressing` (no second surface type). Nothing in the
pipelines calls the new module: no default changes. `outram-foam-mesh`'s
`snapping::detect_feature_edges` is now `pub` (the differential) and a
dev-dependency here. Predictions were committed before the runs; record with
the misses and PNGs: `verification_and_validation/surface_tools_gh1276/README.md`.

- **Item 22 (gh:#1276):** the unlabelled 48-sided pipe at 45° has 96
  feature edges and splits into exactly 3 patches (48/48/96 facets; the
  prediction said 192 for the wall, my arithmetic: missed and corrected).
  The feature edges equal `outram-foam-mesh`'s on the pipe (45°: 96; 5°:
  144) and the box (12); they differ where predicted: a flipped facet (the
  port 12, foam-mesh 13: upstream negates the cosine of an inconsistently
  wound pair) and an open pipe (the port 96, foam-mesh 48: upstream stores
  open edges as feature edges).
- **Item 23 (gh:#1277):** the box with a duplicated point, a duplicated
  facet, a flipped facet and a stray triangle is reported (7 open-edge
  entries, 9 non-manifold, 2 parts, points [6, 8] collocated, 5 orientation
  groups, all as predicted) and cleaned to 12 facets, 8 points and a volume
  of **exactly 8**. The flip is the port's own step: cfMesh has no tool that
  reorients facets. `checkSurfaceMesh` passes the clean box and pipe; the
  prediction that upstream's overlap test would flag every coplanar facet
  pair **missed** (0 flagged).
- **Item 24, part (gh:#1278):** `triSurfaceCurvatureEstimator` and
  `quadricFitting`. Sphere R 3, mean curvature against −1/R: median error
  0.37 % (24 × 48) and 0.087 % (48 × 96), max 0.66 % and 0.16 %, falling
  4.3× per doubling. ~~**Not ported:** `meshOctreeAutomaticRefinement` (976),
  so the proximity test of item 24 is not run.~~ **UPDATED 2026-11-05:**
  ported and run; see "Item 24, rest" below.

**Upstream defects found (not fixed upstream; the port's handling is in each
doc comment):** `subsetToPatch` discards the patches it builds;
`checkForNonManifoldEdges(triSurf&)` runs the hole check;
`triSurfaceCleanupDuplicates` reads stale `pointFacets` and leaves feature
edges and point subsets on old labels; `triSurfaceRemoveFacets` reads
whichever addressing is cached; `triSurfaceCopyParts` and
`triSurfaceClassifyEdges` read index `-1`; `mergeSurfacePatches` copies
subsets by loop index; `calculateEdgeCurvature` measures the turn with the
edges' stored directions, which gives 23.0/R instead of 1.0007/R at the 4
rim points of the 48-sided pipe where the point numbering wraps (measured).
Behaviours as written but not as named: `checkAngles` flags near-180°
angles, `addSurfaceAsSubset` compares a cosine with 5° in radians, and
`doTrianglesOverlap` keeps the outside of each clipping edge for two
same-wound facets (no overlap reported even for a genuine one, measured).

### Item 20 (gh:#1274), 2026-11-05

This item adds three things, with no default changed:

- a new entry point, `pipeline::cartesian_mesh` (`cartesianMeshGenerator`);
- `premap_vertices` (`preMapVertices`, `faceMetricInPatch`);
- `layers::refine_boundary_layers::points_in_bnd_layer` (`pointsInBndLayer`).

It reuses `tet_mesh`'s `create_octree`, `SavedMesh`, `PatchAssignment` and
`face_regions`, and calls `edge_extractor` (gh:#1269) in
`patchAssignment`.

- **Differential (sphere R 3 at 0.5 m):**

  | | `cartesian_mesh` | `carve_box` + `snap` | tet-dual |
  |---|---|---|---|
  | volume error | −1.938 % | −1.719 % | −1.719 % |
  | cells | 1 128 | 912 | 5 269 |
  | max non-orthogonality | 15.6° | 24.1° | 72.5° |

  All three are closed, with 0 non-positive cells.
- **Pipes:**
  - 60 mm pipe at 2 mm: −0.659 %.
  - Foam Studio pipe at 5 mm: **−9.96 %**. The core is 2 x 2 cells, so the
    mesh is under-resolved.
  - Foam Studio pipe with 1.25 mm boundary cells: −0.156 %.
- **The plan's test:**
  - All cells away from the wall are hexes on the sphere: held.
  - `validate()` holds and no pyramid is non-positive: held.
  - The volume error is recorded.
- **Caveats met:**
  - The one-group layer inverts 32 cells on one pipe and 64 on the other.
    `meshOptimisation` repairs all of them.
  - The morpher is not on this route: upstream does not call it.
  - `NotPorted` is not hit, because the one-group layer has no cells with
    two base faces.
- **Misses:** C1 (template volume), C5 (sphere), C7 (`carve_box` + `snap`
  is closer in volume), C8 (Foam Studio at 5 mm) and C9 (no `NotPorted`).
  Each is explained in
  `verification_and_validation/cartesian_mesh_gh1274/README.md`.
- **Still not ported on this route:**
  - the patch-wise `boundaryLayers` groups;
  - `refineEdgeHexCell` and `refineCornerHexCell`;
  - `optimiseLayer`;
  - the surface optimiser's `enforceConstraints`;
  - `anisotropicSources`;
  - `triSurfacePatchManipulator`.

### Item 21 (gh:#1275), 2026-11-05

New entry point `pipeline::cartesian_2d_mesh` (`cartesian2DMeshGenerator`).
The quadtree is in `mesh_octree` / `mesh_octree_queries`, and the empty
patches in `cartesian_mesh::create_empty_patches`. The new modules are
`poly_mesh_gen_2d_engine`, `surface_mapper_2d` (`meshSurfaceMapper2D` and
`meshSurfaceEdgeExtractor2D`) and `tri_surface_2d`. No default changed: a
non-quadtree octree takes every old branch, and all the pins pass.

- **Test, the channel `[0, 1] x [0, 0.25]` at 0.05:**
  - It is one cell thick: every point lies exactly on z = 0 or z = Δz, and
    every cell has one face in each empty patch.
  - The area is L x H to 4e-16.
  - It is closed, with 0 non-positive cells.
  - The template's x-y layout equals the 3D `cartesian_mesh` template's
    mid-plane, all 95 nodes to 1e-9.
  - The finished layouts differ by up to 0.6 cell, because the 3D route's
    Laplacian moves the nodes and the 2D route has none.
- **Misses:**
  - D1: `is2DSurface` on a rectangle, through another crate's eigen
    routine.
  - D5: the finished layouts.
- **Not ported:**
  - `optimizeSurface2D` and `untangleSurface2D`;
  - the 2D `refineBoundaryLayers`;
  - the patch-wise layer;
  - `triSurface2DCheck::createSubsets`;
  - `utilities/extrudeEdgesInto2DSurface`.

### Items 25, 26 and 27 (gh:#1279, #1280, #1281), and gh:#1286, 2026-11-05

- **gh:#1286.** The tet-dual pipeline's stage gate (`pipeline::acceptable`)
  now also refuses a face below `VSMALL` (`poly_mesh_gen_checks::check_face_areas`,
  the check upstream's `findBadFaces` runs) and a non-finite point. It had
  kept gh:#1283's `bmE` mesh at h 4 mm with two zero-area (collinear)
  internal triangles. Pins unchanged. Record: the gh:#1286 section of
  `laminar_pipe_gh1155.md`.
- **Item 26 (gh:#1280).** `layers::extrude_layer` (`extrudeLayer`),
  `layers::triangulate_non_planar_base_faces`, `symmetry_plane_optimisation`
  (wired into `rename_boundary_patches`, as upstream), and
  `mesh_utilities` (`improveMeshQuality`, `improveSymmetryPlanes`,
  `generateBoundaryLayers` partial). `patchesToSubsets` / `subsetToPatch`
  were already ported by the surface track.
- **Item 27 (gh:#1281).** `fundamental_sheets` (JFS, FJ);
  `meshSurfaceEdgeExtractorFUN` now runs `createFundamentalSheetsJFS` in
  `p_mesh` by default (`TetMeshOptions::fun_sheets`; `FunSheets::Without`
  keeps the gh:#1269 route). `p_mesh` re-measured: pipe 4 303 → 9 781
  cells, −0.325 % → −0.320 %, max non-orthogonality 55.29° → 58.24°.
  `helperFunctionsTopologyManipulation` (829, listed with item 27) is not
  used by these classes (grep) and is not ported.
- **Item 25 (gh:#1279).** `anisotropic`: box and plane scaling with their
  inverse, the coordinate modifier and its validity check, the geometry
  modifications, `scaleMesh`, `scaleSurfaceMesh`,
  `surfaceGenerateBoundingBox`. Modify then revert: 2.5e-16 at worst. Not
  wired into any generator.
- Record, predictions against results (misses included) and images:
  `verification_and_validation/fundamental_sheets_gh1281/README.md`.

### Item 24, rest, and the octree remainders (gh:#1278, gh:#1264–#1266), 2026-11-05

No default changes: every new step is off unless the `MeshDict` asks, and
all existing pins pass unchanged. Record, misses and PNGs:
`verification_and_validation/octree_remainders_gh1278/README.md`.

- **gh:#1278, second half:** `meshOctreeAutomaticRefinement`
  (`mesh_octree_automatic_refinement`), on with `MeshDict::min_cell_size`,
  called from `createOctreeBoxes` in upstream's place.
  - Proximity, two plates 0.05 apart, `maxCellSize` 0.125: interior plate
    leaves at level 7, 3.2 leaves across the gap, **as predicted**. The
    first case (gap 0.1, `maxCellSize` 0.25 on the same 1 m plate)
    **missed**: nothing is refined, because at that size every leaf's
    `1.732 h` range reaches a side wall that joins the plates into one
    group. Upstream's criterion as written; kept as a pinned case.
  - Curvature, sphere R 3: triangle leaves from level 4 to level 5
    (`h <= 0.2835 / κ`, `κ` 0.334–0.335), **as predicted**.
- **gh:#1264/#1265 remainders:** `surfaceMeshRefinement` /
  `edgeMeshRefinement` sources, `subsetCellSize` and subset
  `localRefinement`. Differential: the sphere as its own surface source
  gives the `boundaryCellSize` octree leaf for leaf; `subsetCellSize` gives
  the `patchCellSize` octree leaf for leaf. `anisotropicSources` moves the
  sources through gh:#1279's `anisotropic` (merged the same day).
- **gh:#1266 remainders:** the octree edges and their five maps,
  `leafLeaves`, `nodeFaces`, `isIntersectedFace/Edge`, `edgeIntersections`,
  `checkGluedRegions`, `checkAndFixIrregularConnections`, and
  `findUsedBoxes`' `nonManifoldMeshing` / `keep`/`removeCellsIntersectingPatches`
  (`cartesian_mesh_extractor_from_dict`). Octree edges equal the extracted
  mesh's edges (sphere 13 356, pipe 27 762); the glued check splits two
  cubes 0.05 apart into two components; every prediction held.
- **Upstream defects found:** the corner criterion skips the
  `maxRefLevel_` test; the extra layer and the regularity closure exceed
  `maxRefLevel_` (64 pipe leaves at `maxRefLevel_ + 1`); the irregular
  connections' node test is dead code; `checkGluedRegions` leaves the
  addressing it built on the old box types; the sources' `cellSize` loop
  gives one more level at exact powers of two.
- **Still not ported in group B:** nothing serial. Not wired into any pipeline: the
  `cartesianMesh` generator calls `cartesian_mesh_extractor`, not the
  dict-driven `cartesian_mesh_extractor_from_dict`, so the new
  `meshDict` entries reach the generators only through
  `pipeline::tet_mesh::create_octree` (the sources and `minCellSize`), not
  the extractor's cell selection; `subsetCellSize` cannot reach them yet,
  because `create_octree` rebuilds the surface without facet subsets.

### gh:#1270 remainder: the patch-wise `boundaryLayers`, 2026-11-05

`layers::boundary_layers::BoundaryLayers` is upstream's class (serial):
`findPatchesToBeTreatedTogether`, `createNewVertices(labelList)`,
`createLayerCells` (edge and corner cells), `activate2DMode`,
`createOTopologyLayers`, `addWrapperLayer`, `checkTopologyOfBoundaryFaces`;
`create_new_vertex` is now the one `createNewVertex` both routes call, and
`generate_layer` the one `generateBoundaryLayers` step all workflows call.
Wired as upstream: `cartesian_mesh` and `cartesian_2d_mesh` always (2D
mode for the latter; `LayerTopology::OTopology` is the ablation),
`tet_mesh` / `p_mesh` with a `boundaryLayers` dict, the
`generateBoundaryLayers` utility. Defaults of the tet-dual pipeline and Foam
Studio unchanged.

- **Box, one patch per side:** 216 cells (96 face, 48 edge, 8 corner), the
  predicted 6 x 6 x 6 tensor grid, volume 1.
- **Tet-dual pipe:** upstream's concavity rule merges the three patches into
  one group (24 of 72 rim edges fold past 135°); the displacements equal the
  one-group route's bit for bit (1 658 of 1 658).
- **Cartesian pipe 60 mm:** three groups, 2 688 cells (64 edge cells), the
  rim no longer pinched; −0.659 %, closed, 0 non-positive at the end.
  `cartesian_mesh` records re-measured (struck through in
  `cartesian_mesh_gh1274/README.md`).
- **Misses:** the tet-dual pipe's grouping (P3); "bit for bit" against the
  O-topology route on the sphere (labels and round-off differ; volumes to
  2.9e-15).

Record: `verification_and_validation/patch_wise_layers_gh1270/README.md`.

### gh:#1271 remainder: `refineEdgeHexCell`, `refineCornerHexCell`, the two-direction split, 2026-11-05

`refine_boundary_layers` now ports `refineFace` / `generateNewFaces` and
`generateNewCells` by upstream's `refType`; the edge and corner hexahedra
are split as one lattice filled from their faces' grids (upstream's
transfinite face points, trilinear interior points). The prism path is
pinned bit for bit. Six-patch box with the patch-wise layer, `nLayers` 3:
1 000 cells, the exact tensor grid; the cartesian 60 mm pipe now refines
(5 248 cells, closed, 0 non-positive). The row stays partial (2D, the
appended-points case, `activateSpecialMode`). Record:
`verification_and_validation/patch_wise_layers_gh1270/README.md`.

### gh:#1275 remainder: `optimizeSurface2D`, `untangleSurface2D`, the 2D `refineBoundaryLayers`, 2026-11-05

`surface_optimizer::{optimize_surface_2d, untangle_surface_2d}` (about
270 upstream lines), `surface_mapper_2d::map_edges_onto_surface_patches_2d`
(the selected-edge mapping; the all-edge form calls it) and
`refine_boundary_layers_2d` (`activate2DMode`: the empty faces out of every
layer, the rest the general split). `cartesian_2d_mesh` calls
`optimiseMeshSurface` in `edgeExtraction` and `meshOptimisation` and the 2D
refinement with `untangleSurface2D`; `Cartesian2DMeshOptions::boundary_layers`
replaces `refine_boundary_layers_requested`. The channel stays one cell
thick with its area exact; `nLayers` 3 gives 240 cells; stop and restart
reproduce the run. The `meshSurfaceOptimizer` row stays partial
(`enforceConstraints`, `newPositionLaplacian(WFC)`). Record:
`verification_and_validation/cartesian_2d_mesh_gh1275/README.md`.

### gh:#1274 remainder: `preMapVertices` in `pMesh`, 2026-11-05

`pipeline::p_mesh`'s `surfaceProjection` calls `premap_vertices` (3
iterations) before mapping and untangling, as `voronoiMeshGenerator`
does; `tetMesh` is unchanged (upstream does not call it there). The
gh:#1273 record is re-measured: sphere −1.537 % → −1.484 %, the pipe
unchanged to 0.001 points; all five predictions held. Record:
`verification_and_validation/p_mesh_gh1273/README.md` (last section).

### gh:#1279 remainder: `anisotropicSources` in the four generators, 2026-11-05

The four generators apply `anisotropicSources` where upstream's do. Each
constructor builds its octree on the modified surface
(`anisotropic::modified_surface_points`, `surfaceMeshGeometryModification`),
and every step up to the revert works against that surface.

- `cartesianMesh`, `tetMesh` and `pMesh` revert at the end of the first
  `optimiseFinalMesh`. They then run `projectSurfaceAfterBackScaling`:
  `anisotropic::revert_and_project` (`mapVerticesOntoSurface` on the
  original surface), then `optimiseFinalMesh` again.
  `projectSurfaceAfterBackScaling` is a method of the generators, not of
  `meshOptimizer` / `meshSurfaceOptimizer`.
- `cartesian2DMesh` reverts at the end of `generateBoundaryLayers`. It
  then builds a new quadtree on the original surface
  (`cartesian_2d_mesh::refined_quadtree`) and runs `remapBoundaryPoints`
  and `optimiseMeshSurface` on it.

Without sources, the output of all four generators keeps the fingerprints
recorded before the wiring
(`anisotropic::tests::generators_without_anisotropic_sources_are_pinned_bit_for_bit`).

**No row moved:** the generators and the `anisotropicMeshing` rows were
already counted as ported. The row notes that said "no
`anisotropicSources`" or "no generator calls it" are corrected. Record
and images: `verification_and_validation/anisotropic_gh1279/README.md`.
