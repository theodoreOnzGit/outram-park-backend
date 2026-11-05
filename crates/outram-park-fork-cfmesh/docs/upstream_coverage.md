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
| ported | 3,349 | 2.6 % | 6 |
| partial | 54,119 | 41.7 % | 27 |
| not ported | 49,628 | 38.3 % | 80 |
| not applicable | 22,642 | 17.5 % | 25 |
| **total** | **129,738** | 100 % | 138 |

The `partial` lines overstate what is ported. They count each partly-ported unit whole: for example, the 4,441 lines of `polyMeshGenChecks` for 9 of its 34 function definitions.

| Group | ported | partial | not ported | not applicable |
|---|---:|---:|---:|---:|
| A. Workflows | 0 | 1,423 | 5,154 | 1,438 |
| B. Octree | 0 | 13,041 | 3,860 | 2,111 |
| C. Mesh data structures | 0 | 12,767 | 3,538 | 3,354 |
| D. Quality checks | 0 | 4,441 | 0 | 0 |
| E. Volume optimiser | 1,141 | 5,244 | 2,404 | 798 |
| F. Surface optimiser | 0 | 0 | 2,811 | 522 |
| G. Surface tools | 2,208 | 4,082 | 10,962 | 969 |
| H. Boundary layers | 0 | 9,395 | 3,261 | 0 |
| I. Topology repair | 0 | 1,396 | 3,887 | 0 |
| J. triSurface tools, helper classes | 0 | 0 | 7,639 | 233 |
| K. Anisotropic meshing | 0 | 0 | 2,559 | 0 |
| L. Helpers, containers, MPI, I/O | 0 | 2,330 | 1,482 | 11,782 |
| M. Top-level utilities | 0 | 0 | 2,071 | 1,435 |

## 2. Per-unit table

### A. Workflows, generators and executables

| Upstream | Lines | Status | Rust | Notes |
|---|---:|---|---|---|
| `executables/tetMesh` | 65 | partial | `pipeline::surface_to_tet_dual_mesh(_multipatch)` | Thin `main` over `tetMeshGenerator`; the crate's entry point is a different workflow (§3). No `meshDict` reader. |
| `meshLibrary/tetMesh/tetMeshGenerator` | 558 | partial | `boundary_layer_optimisation::add_boundary_layers_cfmesh`, `surface_mapper`, `pipeline` | Ported: the call order of `mapMeshToSurface` → `mapEdgesAndCorners` (surface_mapper) and `optimiseFinalMesh` → `refBoundaryLayers` (boundary_layer_optimisation). Missing: `createTetMesh` (octree tets), `surfacePreparation`, `extractPatches`, `optimiseMeshSurface`, `generateBoundaryLayers` via `boundaryLayers`, `projectSurfaceAfterBackScaling`, `renumberMesh`, `replaceBoundaries`. |
| `meshLibrary/tetMesh/tetMeshExtractorOctree` | 386 | not ported | — (`tet::tetrahedralize` + `delaunay::flip_to_delaunay` instead) | Deliberate difference, §3. |
| `meshLibrary/utilities/tetrahedra/tetCreatorOctree` | 1678 | not ported | — (`tet::tetrahedralize`) | Octree-vertex/centre tets that feed `tetMesh` and `pMesh`. Deliberate difference, §3. |
| `executables/cartesianMesh` | 62 | not ported | — | Thin `main` over `cartesianMeshGenerator`. |
| `meshLibrary/cartesianMesh/cartesianMeshGenerator` | 580 | not ported | — (`carve`, `snap`, `octree` are the crate's own stages) | No `cartesianMesh` workflow. `pipeline` mirrors its staging loosely. |
| `meshLibrary/cartesianMesh/cartesianMeshExtractor` | 800 | partial | `octree::build_mesh`, `carve::carve_box` | Re-implemented, not transcribed: leaves → polyhedra with hanging-node face rings. Missing: `decomposeSplitHexesIntoTetsAndPyramids`, `createPointsAndAddressing` from `meshOctreeAddressing`, cell-type bookkeeping. |
| `executables/pMesh` | 65 | not ported | — | Thin `main` over `voronoiMeshGenerator`. |
| `meshLibrary/voronoiMesh/voronoiMeshGenerator` | 630 | not ported | — | Workflow as `tetMesh`, plus `preMapVertices` and a second `untangleSurface`. |
| `meshLibrary/voronoiMesh/voronoiMeshExtractor` | 674 | not ported | closest analogue: `cell_centre_dual`, `dual::polyhedral_dual*` | Tet-centroid dual of `tetCreatorOctree` tets, cells only at interior vertices. §3. |
| `executables/cartesian2DMesh` | 66 | not ported | — |  |
| `meshLibrary/cartesian2DMesh/cartesian2DMeshGenerator` | 536 | not ported | — |  |
| `meshLibrary/utilities/workflowControls` | 477 | not ported | — | `stopAfter` / restart of a workflow step from `meshDict`; a member of all four generators. |
| `meshLibrary/utilities/checkMeshDict` | 1438 | not applicable | `pipeline::TetDualOptions`, `BoundaryLayerOptimisationControls` | Validation of a `meshDict`; settings here are typed Rust structs. Revisit if a `meshDict` reader is ever added (§4, "Not planned"). |

_Subtotal: 8015 lines._

### B. Octree (`octrees/meshOctree`)

| Upstream | Lines | Status | Rust | Notes |
|---|---:|---|---|---|
| `octrees/meshOctree (serial: meshOctree.C/H/I.H, InsideCalculations, NeighbourSearches, CubePatches, FindNearestSurfacePoint, Slot)` | 2303 | partial | `octree` (`Cell`, `children`, `inside`), `patches::nearest_triangle`, `snap::closest_point_on_surface` | Re-implemented: leaves are a `BTreeSet<Cell>`. Missing: `findNearestSurfacePoint(InRegion)`, `findNearestEdgePoint` and `findNearestCorner` via the octree (done here by brute force over all triangles), neighbour searches across levels, `findLeafContainingVertex`. |
| `octrees/meshOctree: meshOctreeParallelCommunication.C` | 448 | not applicable | — | MPI; the crate is serial. |
| `octrees/meshOctree/meshOctreeCube` | 3078 | partial | `octree::Cell`, `octree::children` | Coordinates and refinement only. Missing: cube–triangle and cube–edge intersections (`meshOctreeCubeIntersections`, `CoordinatesIntersections`), `cubeType` (INSIDE/DATA/OUTSIDE/UNKNOWN), recursive leaf functions. |
| `octrees/meshOctree/meshOctreeCreator (serial)` | 2118 | partial | `octree::refine_near_boundary(_banded)`, `pipeline::TetDualOptions::refinement_levels` | Ported in kind: N levels near the wall (touching shell or band). Missing: `maxCellSize` / `boundaryCellSize` sizing from the surface, `localRefinement` per patch, `objectRefinements`, `refineBoxesContainedInObjects`, `createOctreeWithRefinedBoundary`, `refineBoxesNearDataBoxes`. |
| `octrees/meshOctree/meshOctreeCreator: LoadDistribution.C` | 56 | not applicable | — | MPI load balance. |
| `octrees/meshOctree/meshOctreeModifier (serial)` | 1469 | partial | `octree::refine_with`, `octree::balance_2to1` | `refineSelectedBoxes` and `ensureCorrectRegularity` (2:1) re-implemented. Missing: `reduceMemoryConsumption`, `markAdditionalLayers`, `refineSelectedBoxesAndAdditionalLayers`. |
| `octrees/meshOctree/meshOctreeModifier: DistributeLeaves, LoadDistribution, ParallelRefinement, UpdateCommunicationPattern` | 920 | not applicable | — | MPI. |
| `octrees/meshOctree/meshOctreeAddressing (serial)` | 3118 | partial | `octree::build_mesh`, `octree::conforming_ring` | Points/faces of the leaves re-implemented with hanging nodes spliced in. Missing: `octreeFaces`, `nodeLabels`/`nodeLeaves`, `checkGluedRegions` (`GluedMesh`), `IrregularConnections`. |
| `octrees/meshOctree/meshOctreeAddressing: ParallelAddressing.C` | 687 | not applicable | — | MPI. |
| `octrees/meshOctree/meshOctreeInsideOutside` | 955 | partial | `octree::inside`, `carve` (ray parity) | Deliberate difference: ray-parity inside test instead of front marking from DATA boxes (§3). |
| `octrees/meshOctree/meshOctreeAutomaticRefinement` | 976 | not ported | — | Curvature- and proximity-based refinement. |
| `octrees/meshOctree/refinementControls/objectRefinement` | 2572 | not ported | — | box, cone, hollowCone, line, sphere refinement sources. |
| `octrees/meshOctree/refinementControls/patchRefinement` | 312 | not ported | — |  |

_Subtotal: 19012 lines._

### C. Mesh data structures and addressing (`meshes/`)

| Upstream | Lines | Status | Rust | Notes |
|---|---:|---|---|---|
| `meshes/polyMeshGen` | 2664 | partial | `volume_mesh::VolumeMesh`, `from_cell_faces(_wound)`, `cells_faces` | Points/faces/owner/neighbour/patches. Missing: point/face/cell subsets (`addPointSubset` etc.), `polyMeshGenCells` cell list (cells are implicit), `write`/`read`. |
| `meshes/polyMeshGen/boundaryPatch (boundaryPatch, boundaryPatchBase)` | 639 | partial | `volume_mesh::BoundaryPatch` | Name, start, size. Patch `type` (wall/patch/symmetry) not carried; `foam` decides it. |
| `meshes/polyMeshGen/boundaryPatch: processorBoundaryPatch` | 363 | not applicable | — | MPI processor patches. |
| `meshes/primitives/meshSubsets` | 442 | not ported | — | Needed by `patchesToSubsets`, `subsetToPatch`, `lockCellsInSubset`. |
| `meshes/polyMeshGenAddressing (serial)` | 2576 | partial | `mesh_optimizer::point_cells`, `cell_faces`, `checks::cell_geometry`, ad hoc maps per module | No shared addressing object: each module rebuilds what it needs. Missing as a reusable set: `edges`, `edgeFaces`, `faceEdges`, `pointPoints`, `cellCells`, `cellEdges`. |
| `meshes/polyMeshGenAddressing: ParallelAddressing.C` | 974 | not applicable | — | MPI. |
| `meshes/polyMeshGenModifier (AddCells, RemoveCells, RemoveFaces, RemoveUnusedVertices, ReorderBoundaryFaces, ReplaceBoundary, AddCellByCell, .H)` | 1862 | partial | `volume_mesh::from_cell_faces`, `patches::assign_patches_by_region` | Replaced: every stage rebuilds the mesh from cell face lists. Not a line-by-line port. |
| `meshes/polyMeshGenModifier: RenumberMesh.C` | 282 | not ported | — | Cuthill–McKee renumbering. No Rust renumbering found in the workspace. |
| `meshes/polyMeshGenModifier: ZipUpCells.C` | 794 | not ported | — (`octree::conforming_ring` splices hanging nodes during assembly) | No caller in `meshLibrary/` at this commit (grep). Not planned. |
| `meshes/polyMeshGenModifier: AddBufferCells, AddProcessorFaces` | 437 | not applicable | — | MPI. |
| `meshes/polyMeshGen2DEngine` | 689 | not ported | — | 2D workflow. |
| `meshes/partTetMesh (serial)` | 2119 | partial | `mesh_optimizer::PartTetMesh` | Ported: `partTetMesh(mesh, locked, badFaces, layers)`, `createPointsAndTets`, `partTetMeshSimplex`, `updateVerticesSMP`, `updateOrigMesh`. Missing: the `(mesh, locked, nLayers)` constructor (`optimizeMeshNearBoundaries`), `BOUNDARY` nodes as movable nodes (the boundary optimisers), `createPolyMesh`, `tetMatcher` handling. |
| `meshes/partTetMesh: ParallelAddressing.C` | 535 | not applicable | — | MPI. |
| `meshes/partTriMesh (serial)` | 1331 | ~~not ported~~ **ported (2026-11-05, gh:#1258)** | `surface_optimizer::PartTriMesh`, `PartTriMeshSimplex` | Surface tri mesh for `meshSurfaceOptimizer`: both constructors, `updateVertex`, `updateVertices`, the simplex and its normal. |
| `meshes/partTriMesh: ParallelAddressing.C` | 538 | not applicable | — | MPI. |
| `meshes/primitives/partTet` | 507 | not applicable | `[usize; 4]` + `mesh_optimizer::tet_volume` | Primitive, replaced by an index array. |
| `meshes/triSurf` | 2907 | partial | `(Vec<Vec3>, Vec<[usize; 3]>)` + `patches::SurfaceRegions` | Points, facets and region (patch) per facet. Missing: feature edges, facet/point/edge subsets, `triSurfAddressing` (edges, edgeFacets, pointFacets, facetEdges). |

_Subtotal: 19659 lines._

### D. Quality checks

| Upstream | Lines | Status | Rust | Notes |
|---|---:|---|---|---|
| `meshes/polyMeshGenChecks` | 4441 | partial | `checks::check_quality`, `checks::faces_failing_mesh_quality`, `mesh_optimizer::find_bad_faces`, `find_low_quality_faces` | Ported (cfMesh definitions, in `mesh_optimizer`): `checkFaceDotProduct`, `checkFaceSkewness`, `checkFacePyramids`, `checkFaceFlatness`, `checkCellPartTetrahedra`, `checkFaceAreas`, `findBadFaces`, `findBadFacesRelaxed`, `findLowQualityFaces`. `checks.rs` holds OpenFOAM's definitions, not cfMesh's. Missing: `checkTetQuality`, `checkMinTwist`, `checkTriangleTwist`, `checkCellDeterminant`, `checkMinVolRatio`, `checkFaceUniformity`, `checkVolumeUniformity`, `checkFaceAngles`, `findBadFacesAdditionalChecks`, `findWorstQualityFaces`, the topology checks (`checkPoints`, `checkUpperTriangular`, `checkCellsZipUp`, `checkFaceVertices`), `meshQualitySettings`. |

_Subtotal: 4441 lines._

### E. Volume optimiser (`smoothers/geometry/meshOptimizer`)

| Upstream | Lines | Status | Rust | Notes |
|---|---:|---|---|---|
| `smoothers/geometry/meshOptimizer (meshOptimizer.C/H/I.H, optimizeMeshFV.C, OptimizeSurface.C)` | 1590 | partial | `mesh_optimizer::untangle_mesh_fv(_bounded)`, `optimize_low_quality_faces`; `boundary_layer_optimisation` (`optimizeBoundaryLayer`, `untangleBoundaryLayer`) | Missing: the boundary-point loop of `untangleMeshFV` (`optimiseBoundaryVolumeOptimizer(true/false)`, `optimiseBoundarySurfaceLaplace`), `optimizeSurface`, `optimizeMeshNearBoundaries`, `optimizeMeshFVBestQuality`, `enforceConstraints` and the bad-points subset, the subset locks (`lockCellsInSubset`/`Faces`/`Points`). Points can be locked through the `locked` argument. |
| `smoothers/geometry/meshOptimizer: meshOptimizerOptimizePoint.C (laplaceSmoother)` | 437 | not ported | — (`smooth::laplacian_smooth` is Freitag's smart Laplacian, not this) | `laplacian`, `laplacianPC`, `laplacianWPC`, `laplacianSurface`. The Laplace pass of `optimizeMeshFV`. |
| `smoothers/geometry/meshOptimizer: meshOptimizerOptimizePointParallel.C` | 461 | not applicable | — | MPI. |
| `smoothers/geometry/meshOptimizer/tetMeshOptimisation (serial)` | 788 | partial | `mesh_optimizer::PartTetMesh::smooth_nodes` with `knupp_metric` / `volume_optimizer` | Ported: `optimiseUsingKnuppMetric`, `optimiseUsingVolumeOptimizer`. Missing: `optimiseUsingMeshUntangler`, `optimiseBoundaryVolumeOptimizer`, `optimiseBoundarySurfaceLaplace`. |
| `smoothers/geometry/meshOptimizer/tetMeshOptimisation: Parallel.C` | 337 | not applicable | — | MPI. |
| `.../advancedSmoothers/knuppMetric` | 359 | ported | `mesh_optimizer::knupp_metric` | Not re-checked line by line; module header and doc comment cite `optimizeNodePosition`, `evaluateMetric(NoBeta)` and `evaluateGradients`. |
| `.../advancedSmoothers/simplexSmoother` | 179 | ported | `mesh_optimizer` simplex signature `(pts, tets, free)` | Abstract base (bounding box, point access); replaced by a free-function signature. gh:#1251 lists it as missing: it is not. |
| `.../advancedSmoothers/volumeOptimizer` | 603 | ported | `mesh_optimizer::volume_optimizer`, `vo_func`, `vo_gradients`, `vo_stabilisation` | `optimizeNodePosition` checked: divide and conquer, then Newton/steepest descent, kept only if no worse. |
| `.../advancedSmoothers/meshUntangler` | 1233 | not ported | — | LP untangler: the feasible region (cut by the opposite-face planes) and `cutRegion` with `tieBreak`. |
| `.../advancedSmoothers/quadricMetric` | 329 | not ported | — | No upstream workflow calls it (`grep` of `meshLibrary/`: only its own files). |
| `smoothers/geometry/meshOptimizer/boundaryLayerOptimisation` | 2866 | partial | `boundary_layer_optimisation` | Ported: `calculateHairEdges`, `optimiseLayersAtExittingFaces`, `optimiseLayer`, the normals and thickness routines. ~~Missing: the `meshSurfaceOptimizer::optimizeSurface(2)` call inside `optimiseLayer` (the wall points never move);~~ **UPDATED 2026-11-05 (gh:#1259):** the `optimizeSurface(2)` call is ported, opt-in (`BoundaryLayerOptimisationControls::optimise_surface`, off by default). Hair classes come from `layers::LayerVertex`, not `detectBoundaryLayers`. |
| `smoothers/geometry/meshOptimizer/symmetryPlaneOptimisation` | 405 | not ported | — |  |

_Subtotal: 9587 lines._

### F. Surface optimiser (`smoothers/geometry/meshSurfaceOptimizer`)

| Upstream | Lines | Status | Rust | Notes |
|---|---:|---|---|---|
| `smoothers/geometry/meshSurfaceOptimizer (serial)` | 2266 | ~~not ported~~ **partial (2026-11-05, gh:#1259)** | `surface_optimizer::MeshSurfaceOptimizer`, `map_mesh_to_surface`, `optimise_mesh_surface`; `mesh_optimizer::optimize_surface` (`meshOptimizerOptimizeSurface.C`) | Ported (3-D, serial): `untangleSurface` (both forms), `optimizeSurface`, Laplacian FC, `surfaceOptimizer` smoothing, edge-node smoothing, `lockBoundaryFaces`/`Points`/`FeatureEdges`, `removeUserConstraints`, the mapper calls. Not ported: `untangleSurface2D`, `optimizeSurface2D`, `enforceConstraints` (no point subsets), `newPositionLaplacian`/`LaplacianWFC` (no 3-D caller). All opt-in (`TetDualOptions::untangle_surface`); differential against `revert_tangled` in `laminar_pipe_gh1155.md` (gh:#1259). |
| `smoothers/geometry/meshSurfaceOptimizer: OptimizePointParallel.C` | 522 | not applicable | — | MPI. |
| `.../advancedSurfaceSmoothers/surfaceOptimizer` | 545 | ~~not ported~~ **ported (2026-11-05, gh:#1258)** | `surface_optimizer::optimize_point` | The 2D (in-plane) volume-optimiser analogue `untangleSurface` uses; `matrix2D`'s solve inline. |

_Subtotal: 3333 lines._

### G. Surface tools (`surfaceTools/`, `intersectionTools/`)

| Upstream | Lines | Status | Rust | Notes |
|---|---:|---|---|---|
| `surfaceTools/meshSurfaceEngine (serial)` | 3086 | ~~partial~~ **ported (2026-11-05, gh:#1257)** | ~~ad hoc boundary maps in `layers::layer_setup`, `correct_edges`, `surface_mapper`~~ `surface_engine::SurfaceEngine` | ~~No shared engine. Missing as a reusable object: …~~ Every serial accessor and `meshSurfaceEngineModifier` (`moveBoundaryVertex(NoUpdate)`, `updateGeometry`). Not ported: the `activePatch` constructor (no caller here). Built eagerly (no lifetimes); the caller owns the points. `layer_setup`, `correct_edges` and `surface_mapper` now read their boundary maps from it. |
| `surfaceTools/meshSurfaceEngine: ParallelAddressing.C` | 775 | not applicable | — | MPI. |
| `surfaceTools/meshSurfaceMapper (mapVerticesOntoSurface(Patches), mapCorners, mapEdgeNodes, findMappingDistance)` | 1175 | ported | `surface_mapper::map_vertices_onto_surface_patches(_untangled(_given))`, `map_vertices_onto_surface_untangled`; `surface_mapper::SurfaceQueries`, `map_edge_node` | `mapVerticesOntoSurface` has no `untangleSurface` after it (moves undone instead). `findMappingDistance` uses vertex-average face centres here, OpenFOAM centres upstream (kept, so the pins hold). Surface queries are brute force (upstream: `meshOctree`). |
| `surfaceTools/meshSurfaceMapper: PremapVertices.C` | 302 | not ported | — | `preMapVertices` (cartesianMesh, cartesian2DMesh, pMesh). |
| `surfaceTools/meshSurfaceMapper: parMapperHelper.H` | 194 | not applicable | — | MPI. |
| `surfaceTools/meshSurfaceMapper2D` | 1189 | not ported | — | 2D workflow. |
| `surfaceTools/meshSurfaceEdgeExtractorNonTopo` | 198 | ported | `pipeline` (`correct_edges` then `surface_mapper`) | `decomposeBoundaryFaces` + `remapBoundaryPoints`. |
| `surfaceTools/correctEdgesBetweenPatches` | 835 | ported | `correct_edges::correct_edges_between_patches` | `decomposeProblematicFaces` is commented out upstream. Opt-in (`TetDualOptions::correct_edges`). |
| `surfaceTools/meshSurfaceCheckEdgeTypes` | 483 | partial | `correct_edges` (tet test) | `classifyEdges` tet test only. Missing: the `meshSurfaceCheckInvertedVertices` (UNDETERMINED) branch, the `convexEdges`/`concaveEdges` accessors as a reusable class. |
| `surfaceTools/meshSurfaceCheckInvertedVertices` | 555 | ~~not ported~~ **ported (2026-11-05, gh:#1258)** | `surface_optimizer::check_inverted_vertices`, `is_face_convex_and_ok` (`surface_mapper::cells_with_nonpositive_pyramids` is a cell test, not this) | On the crate's per-point / cell-centre duals of the 48-sided pipe (h 6 mm) it flags 554 of 1 390 boundary points before any mapping, 550 of them as reflex corners of 156 non-convex boundary polygons (the differential test in `surface_optimizer`). Not yet wired into `correct_edges`' UNDETERMINED edge class. |
| `surfaceTools/meshSurfacePartitioner` | 513 | ~~partial~~ **ported (2026-11-05, gh:#1257)** | `surface_engine::SurfacePartitioner` | ~~Corners (>2 feature edges) and edge points (2). Missing: `patchPatches`, `edgeGroups`, as a reusable class.~~ All of it, serial: corners, edge points, `pointPatches`, `patchPatches`, `featureEdges`, `numberOfFeatureEdgesAtPoint`. **CORRECTED 2026-11-05:** `meshSurfacePartitioner.H` at `e944f6ba695f` has no `edgeGroups`; the earlier row named a member that does not exist there. |
| `surfaceTools/edgeExtraction/edgeExtractor` | 4852 | not ported | — (`patches::assign_patches_by_region`: nearest triangle only) | Refines the face→patch assignment (`extractEdges`, `updateMeshPatches`, corners). |
| `surfaceTools/meshSurfaceEdgeExtractor` | 406 | not ported | — | Older topological extractor; no generator in `meshLibrary/` calls it. |
| `surfaceTools/meshSurfaceEdgeExtractorFUN` | 407 | not ported | — | Fundamental-sheets extractor (used with `createFundamentalSheets`). |
| `surfaceTools/meshSurfaceEdgeExtractor2D` | 311 | not ported | — | 2D workflow. |
| `surfaceTools/createFundamentalSheets (base, FJ, JFS)` | 908 | not ported | — | Layer of cells at feature edges for `meshSurfaceEdgeExtractorFUN`. |
| `surfaceTools/renameBoundaryPatches` | 392 | not ported | — (`patches` names come from `SurfaceRegions`) | `renameBoundary` / patch `type` from `meshDict`. |
| `surfaceTools/surfaceMorpherCells` | 1139 | not ported | — | `surfacePreparation`: one boundary face per cell. |
| `intersectionTools/findCellsIntersectingSurface` | 501 | not ported | — | No caller in `meshLibrary/` at this commit (grep). |

_Subtotal: 18221 lines._

### H. Boundary layers (`boundaryLayers/`)

| Upstream | Lines | Status | Rust | Notes |
|---|---:|---|---|---|
| `boundaryLayers/boundaryLayers` | 4229 | partial | `layers::layer_setup`, `layers::add_boundary_layers_adaptive_on_patches` | Ported: the per-patch border rules (slide in a non-layer patch's plane, along the edge between two, refuse >3 patches). Differs: the crate shrinks the wall points by a requested profile; upstream creates new vertices at half the local edge length and splits later (§2). Missing: `boundaryLayersCreateVertices`, `boundaryLayerCells`, `checkTopologyOfBndFaces`, `addWrapperLayer`. |
| `boundaryLayers/refineBoundaryLayers` | 5166 | partial | `layers::build_layered_variable`, step 3 of `boundary_layer_optimisation` | Only "split each hair by the grading" on a one-cell layer. Missing: per-patch `nLayers` / `thicknessRatio` / `maxFirstLayerThickness`, `allowDiscontinuity`, the topological split of arbitrary layer cells (`refineBoundaryLayersCells`, `Faces`), `activateSpecialMode`, `pointsInBndLayer`. |
| `boundaryLayers/detectBoundaryLayers` | 989 | not ported | — (layer columns known from construction) | Needed only for a mesh whose layers this crate did not build. |
| `boundaryLayers/extrudeLayer` | 1915 | not ported | — |  |
| `boundaryLayers/triangulateNonPlanarBaseFaces` | 357 | not ported | — |  |

_Subtotal: 12656 lines._

### I. Topology repair and decomposition

| Upstream | Lines | Status | Rust | Notes |
|---|---:|---|---|---|
| `smoothers/topology/topologicalCleaner` | 857 | not ported | — | `surfacePreparation`. |
| `smoothers/topology/checkIrregularSurfaceConnections` | 1153 | not ported | — | cartesianMesh. |
| `smoothers/topology/checkBoundaryFacesSharingTwoEdges` | 649 | not ported | — | cartesianMesh. |
| `smoothers/topology/checkCellConnectionsOverFaces` | 369 | not ported | — | cartesianMesh. |
| `smoothers/topology/checkNonMappableCellConnections` | 437 | not ported | — | cartesianMesh. |
| `decomposeCells` | 755 | partial | `correct_edges` (`checkFaceConnections`, `findTopVertex`, `decomposeCellIntoPyramids`) | Missing: the general `decomposeMesh` driver (`addNewCells`, `removeDecomposedCells`, `storeBoundaryFaces`). |
| `faceDecomposition: decomposeFaces` | 641 | partial | `correct_edges` (triangle fan) | Missing: `decomposeConcaveInternalFaces`, `decomposeMeshFaces` as a driver. |
| `faceDecomposition: faceDecomposition` | 422 | not ported | — | Concave face → convex parts. |

_Subtotal: 5283 lines._

### J. Surface (`triSurfaceTools/`) and helper classes

| Upstream | Lines | Status | Rust | Notes |
|---|---:|---|---|---|
| `triSurfaceTools/triSurfaceChecks` | 1253 | not ported | — | `checkSurfaceMesh`. |
| `triSurfaceTools/triSurfaceDetectFeatureEdges` | 277 | not ported | — | Workspace has `outram-foam-mesh` `snapping::detect_feature_edges` (private) and `outram-blender` `measure::sharp_edges` (this crate cannot depend on blender). |
| `triSurfaceTools/triSurfacePatchManipulator` | 509 | not ported | — | Split a surface into patches at feature edges. |
| `triSurfaceTools/triSurfaceClassifyEdges` | 575 | not ported | — |  |
| `triSurfaceTools/triSurfacePartitioner` | 598 | not ported | — |  |
| `triSurfaceTools/triSurfaceCurvatureEstimator` | 1229 | not ported | — | Feeds `meshOctreeAutomaticRefinement`. |
| `triSurfaceTools/triSurfaceCleanupDuplicates` | 454 | not ported | — |  |
| `triSurfaceTools/triSurfaceCleanupDuplicateTriangles` | 235 | not ported | — |  |
| `triSurfaceTools/triSurfaceCopyParts` | 412 | not ported | — |  |
| `triSurfaceTools/triSurfaceRemoveFacets` | 377 | not ported | — |  |
| `triSurfaceTools/triSurfaceImportSurfaceAsSubset` | 343 | not ported | — |  |
| `triSurfaceTools/triSurfaceExtrude2DEdges` | 201 | not ported | — | 2D workflow. |
| `triSurfaceTools/triSurface2DCheck` | 264 | not ported | — | 2D workflow. |
| `triSurfaceTools/triSurfaceMetaData` | 233 | not applicable | — | FMS metadata. |
| `helperClasses/sortEdgesIntoChains` | 425 | not ported | — | Used by `meshUntangler::cutRegion` (plan item 6). |
| `helperClasses/geometry/quadricFitting` | 487 | not ported | — | Feeds `triSurfaceCurvatureEstimator`. |

_Subtotal: 7872 lines._

### K. Anisotropic meshing (`anisotropicMeshing/`)

| Upstream | Lines | Status | Rust | Notes |
|---|---:|---|---|---|
| `anisotropicMeshing/coordinateModification` | 1686 | not ported | — | `anisotropicSources` (box/plane scaling). |
| `anisotropicMeshing/edgeMeshGeometryModification` | 255 | not ported | — |  |
| `anisotropicMeshing/polyMeshGenGeometryModification` | 241 | not ported | — |  |
| `anisotropicMeshing/surfaceMeshGeometryModification` | 377 | not ported | — |  |

_Subtotal: 2559 lines._

### L. Helper functions, containers, parallel helpers, I/O

| Upstream | Lines | Status | Rust | Notes |
|---|---:|---|---|---|
| `helperFunctions: GeometryQueries.H + GeometryQueriesI.H` | 2330 | partial | `boundary_layer_optimisation` (`nearest_point_on_line`, `tri_segment_intersection`, `segment_face_intersection`), `snap::closest_point_on_triangle`, `math::Vec3` | Ported where used: `nearestPointOnTheEdge`, `lineFaceIntersection`, `triLineIntersection`. Not re-checked one by one: the rest (`isFaceConvex`, `pointInTetrahedron`, `boundBoxLineIntersection`, …). |
| `helperFunctions: FrontalMarking` | 653 | not ported | — | Front-marking region growth (used by edgeExtractor, inside/outside). |
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
| `generateBoundaryLayers` | 158 | not ported | — | `boundaryLayers` + `refineBoundaryLayers` on an existing mesh. |
| `improveMeshQuality` | 145 | not ported | — (library calls exist: `untangle_mesh_fv`, `optimize_low_quality_faces`) |  |
| `improveSymmetryPlanes` | 65 | not ported | — |  |
| `scaleMesh` | 83 | not ported | — |  |
| `scaleSurfaceMesh` | 81 | not ported | — |  |
| `surfaceFeatureEdges` | 103 | not ported | — | `triSurfaceDetectFeatureEdges` + `triSurfacePatchManipulator`. |
| `checkSurfaceMesh` | 242 | not ported | — |  |
| `copySurfaceParts` | 87 | not ported | — |  |
| `extrudeEdgesInto2DSurface` | 82 | not ported | — |  |
| `importSurfaceAsSubset` | 89 | not ported | — |  |
| `mergeSurfacePatches` | 402 | not ported | — |  |
| `patchesToSubsets` | 91 | not ported | — |  |
| `removeSurfaceFacets` | 80 | not ported | — |  |
| `subsetToPatch` | 178 | not ported | — |  |
| `surfaceGenerateBoundingBox` | 185 | not ported | — |  |
| `FMSToSurface, FMSToVTK, FLMAToSurface, surfaceToFMS, meshToFPMA` | 1318 | not applicable | — | FMS/FLMA/FPMA/VTK file formats. |
| `preparePar` | 117 | not applicable | — | MPI decomposition. |

_Subtotal: 3506 lines._

## 3. Where the port deliberately differs from upstream

These are not gaps. They are different routes to the same stage, so the plan in §4 adds the upstream route **beside** them and compares the two (`AGENTS.md`, differential testing). It does not replace them.

| Stage | Upstream | This crate | What differs |
|---|---|---|---|
| Volume tets | `tetCreatorOctree` + `tetMeshExtractorOctree`: tets from the octree's vertices and cube centres (`tetMesh`, `pMesh`) | `tet::tetrahedralize` (centroid subdivision of carved or graded cells), then `delaunay::flip_to_delaunay` | Different tets and point set. The crate's tets are not Delaunay before the flips (`delaunay` docs). Upstream's `tetMesh` stops at tets; the crate goes on to a dual. |
| Polyhedral cells | `pMesh`: `voronoiMeshExtractor`, one cell per interior tet vertex, corners at tet **centroids**, no cells at boundary vertices; boundary then mapped and untangled | `dual::polyhedral_dual*` (median dual: edge midpoints, face and cell centroids), `dual::polyhedral_dual_per_point`, `cell_centre_dual` (OpenFOAM `polyDualMesh` through `outram-foam-mesh`); selected by `TetDualOptions::dual_kind` | The crate's duals keep boundary-vertex cells and build boundary faces themselves. `CellCentre` is the nearest to `voronoiMeshExtractor` (same dual corners inside). |
| Inside/outside | `meshOctreeInsideOutside`: front marking from the surface-intersecting (DATA) boxes | `carve`, `octree::inside`: ray parity | Different test. Same answer away from the surface, unmeasured near it. |
| Background mesh | `meshOctreeCreator` sizing (`maxCellSize`, `boundaryCellSize`, local and object refinement), `cartesianMeshExtractor` + `decomposeSplitHexes` | `carve::carve_box` (uniform), `octree::refine_near_boundary(_banded)` (N levels near the wall), hanging nodes spliced into the face rings | No size-driven refinement. Split hexes stay polyhedra instead of being decomposed. |
| First surface fit | `meshSurfaceMapper::preMapVertices` + `mapVerticesOntoSurface`, then `meshSurfaceOptimizer::untangleSurface` | `snap::snap_to_surface` (one closest-point projection); in the multipatch pipeline `surface_mapper::map_vertices_onto_surface_untangled` | Tangled cells are repaired by **undoing** the moves (`revert_tangled`), not by untangling. |
| Patch assignment | `edgeExtractor` refines nearest-triangle patches | `patches::assign_patches_by_region` (nearest triangle only) | No refinement of the assignment. |
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
- `checkMeshDict`: there is no `meshDict` reader.

If a `meshDict` reader is wanted, it is a new item after 20.
