# cfMesh octree and cartesian extraction: V&V record (gh:#1262–#1266)

**Research, education and V&V only.** Not for reactor operation, licensing or
safety decisions (`RESPONSIBLE_USE.md`). AI-written port, not yet reviewed by
a human.

**Upstream:** `github.com/DAFoam/cfmesh` at `e944f6ba695f` (cfMesh v1.1 line).
**Ported, beside `octree.rs` (unchanged; `octree_path_is_pinned_bit_for_bit`
still passes):**

| issue | upstream | crate module |
|---|---|---|
| #1262 | `meshOctreeCube*`, `meshOctree.{C,H}`, `meshOctreeI.H`, `meshOctreeSlot.H`, `meshOctreeCubePatches.C` | `mesh_octree` |
| #1263 | `meshOctreeNeighbourSearches.C`, `meshOctreeInsideCalculations.C`, `meshOctreeFindNearestSurfacePoint.C`, `meshOctreeModifier` (serial) | `mesh_octree_queries`, `mesh_octree_modifier`; opt-in `SurfaceSearch::Octree` in `patches` and `snap` |
| #1264 | `meshOctreeCreator` (serial), `meshOctreeInsideOutside`, `helperFunctionsFrontalMarking` | `mesh_octree_creator` |
| #1265 | `objectRefinement` (box, sphere, cone, hollow cone, line), `patchRefinement` | `object_refinement`, `mesh_octree_creator::{PatchRefinement, LocalRefinement}` |
| #1266 | `meshOctreeAddressing` (serial; nodes, used boxes, faces), `cartesianMeshExtractor` | `cartesian_mesh` (the `decomposeCells` step shared with `correct_edges`) |

Each test's `///` doc carries its methodology, the prediction written before
the run, and the result. This page collects the numbers. All runs were made on
2026-11-05 with `cargo test --release`.

## Results

### Surface octree (#1262)

| check | prediction | result |
|---|---|---|
| Triangles stored per leaf against a separating-axis oracle, every leaf, 4 levels, box `[0,2]^3` | identical | identical (1 520 leaves, 728 holding triangles) |
| Same, sphere R 3 (24 × 48) | identical, no touches within 1e-15 | **missed**: leaf `(4; 6, 5, 3)` holds triangle 1834, which touches it to 1e-15 along `x = y`. The oracle was then set to upstream's definition (box inflated by `SMALL ×` root span), and the sets are identical on every leaf; 380 touching pairs are pinned. |
| DATA cells against `octree.rs`'s `TouchesWall` / `DistanceBand(1)` on the same lattice (sphere, 0.5 m) | `B_in ⊂ A1`; every `A` cell is in or next to `B`; `|B| > |A|` | all hold: `|A|` 336, `|A1|` 360, `|B|` 680, `|B_in|` 312, `|A ∩ B|` 288 |

### Queries and modifier (#1263)

Nearest point on 10 000 points per surface, in the bounding box grown by 20 %:

| surface | `Exact` + Ericson vs brute force | upstream rule misses (worst) | brute force | `Exact` | upstream rule |
|---|---|---|---|---|---|
| pipe 48 | 0 point / 0 triangle differences | 427 (+35.7 %) | 31 ms | 77 ms | 65 ms |
| sphere | 0 / 0 | 734 (+41.4 %) | 285 ms | 382 ms | 116 ms |

- Bit-identity held, as predicted.
- Upstream's `findNearestSurfacePoint` stops at the first box holding any
  triangle, so it is not exact off the surface. It is kept as written.
- **The time prediction missed:** I predicted the octree would beat brute
  force on the sphere, but `Exact` is slower there. The run was single and
  not profiled.
- `patches::assign_patches_by_region_with` and `snap::snap_to_surface_with`
  with `SurfaceSearch::Octree` reproduce the brute-force meshes bit for bit
  (pipe, sphere).
- The neighbour searches equal brute-force box adjacency on every leaf.
- `refineSelectedBoxes` keeps the octree 1-irregular. Refinement without the
  regularity pass reaches jumps of 2–3.

### Creator and inside/outside (#1264)

| surface | dict | leaves | INSIDE | DATA | OUTSIDE | UNKNOWN | non-DATA vs ray parity |
|---|---|---|---|---|---|---|---|
| sphere R 3 | max 1.0, boundary 0.25 | 10 816 | 4 312 | 1 584 | 4 920 | 0 | 0 disagreements |
| pipe 48 | max 4 mm, boundary 1 mm | 20 952 | 8 512 | 4 352 | 8 088 | 0 | 0 disagreements |

- **The root-size prediction missed.** Upstream sizes the root from the
  initial root box (1.5 × the surface extent). Sphere: root side 16, triangle
  leaves at level 6. Pipe: root side 0.128 m, level 7.
- `reviseDataBoxes` retypes 1 064 (sphere) and 1 928 (pipe) DATA leaves as
  INSIDE. On the sphere, **48 of them have their centre outside the surface**.
  That is upstream's rule, pinned rather than fixed. Leaves holding triangles
  are never cells (`findUsedBoxes`), so these do not reach the mesh unless
  `keepCellsIntersectingBoundary` is set.
- Against `refine_near_boundary_banded` (sphere, 1.0 m, one level), the counts
  were within the predicted factor 2: 864 cells for `octree.rs`, against 792
  INSIDE leaves (1 224 with DATA) for the creator.

### Object and patch refinement (#1265)

- All five shapes on `[0, 8]^3` (`maxCellSize` 1): every leaf the object
  meets is at the target level, every target-level leaf has a parent the
  object meets, and the octree is 1-irregular.
- The target level is 7, not 6: upstream's `calculateAdditionalRefLevels`
  counts 3 halvings for a 0.25 m request. I found this from the source before
  the run, and the corrected prediction held.
- **Finding:** a hollow cone with shell radii 0.8–1.5 lies between the 1 m
  leaf centres and refines nothing. Sphere, cone and hollow cone test only the
  leaf **centre**. Pinned by
  `hollow_cone_thinner_than_the_background_spacing_is_missed`.
- `patchCellSize` and `localRefinement` reach level 6 on their regions.
  **The "others stay at 5" part missed:** `refineBoxesNearDataBoxes` lifts
  1 092 of 2 776 other triangle leaves to 6 (chained through the children of
  split corner leaves). The count is pinned.

### Cartesian extraction (#1266), against `octree::build_mesh` on the same leaves

| surface | cells | faces | points | volume rel. diff | faces in only one | split hexes |
|---|---|---|---|---|---|---|
| sphere | 3 248 = 3 248 | 11 532 = 11 532 | 5 073 = 5 073 | 2.5e-15 | 0 / 0 | 168 |
| pipe 48 | 6 584 = 6 584 | 23 740 = 23 740 | 10 607 = 10 607 | 1.5e-14 | 0 / 0 | 656 |

- Every prediction held: the two routes build the same faces ring for ring.
  The volume difference comes from upstream's 1e-15 inflation of the cube
  corners.
- `decomposeSplitHexes` on the pipe decomposes 864 cells (656 with more than
  6 faces, plus 208 hexes with hanging edge points), giving 27 064 cells. The
  decomposed mesh is closed, its volume is equal to 1e-12, and every cell is
  positive.

## Images (`mesh_octree_images` example; regenerate with the command in its docs)

| file | what |
|---|---|
| `sphere_xz_type.png`, `sphere_xz_level.png` | sphere R 3, creator octree, x-z slice at y = 0.01, by leaf type and by level |
| `pipe_xy_type.png`, `pipe_xy_level.png`, `pipe_xz_type.png` | pipe 48, mid-length x-y slice and axial x-z slice |
| `objects_xy_level.png` | cube `[0,8]^3` with a sphere (R 1.3) and a line object, z = 4.01, by level |
| `sphere_xz_cartesian_mesh.png`, `pipe_xy_cartesian_mesh.png`, `pipe_xz_cartesian_mesh.png` | extracted cartesian mesh edges (blue) over the leaf types |
| `pipe_xy_cartesian_mesh_decomposed.png` | the same with split hexes decomposed |

**What I checked in them:**

- The DATA band follows the surface on every slice, with no UNKNOWN (red)
  leaf.
- OUTSIDE fills the corners.
- The refinement grades by one level at a time.
- The mesh edges stop at the INSIDE leaves without triangles. The INSIDE
  leaves that straddle the surface are the revised DATA leaves: they are not
  cells and carry no blue edges.
- The line and sphere objects are refined to level 7 along their extent.

**What I could not check:**

- 3D views. All images are planar slices.
- The decomposed image shows the pyramid edges only where they cut the slice.

## Not ported (see each module's docs)

- The quadtree and 2D workflow.
- MPI.
- `meshOctreeAutomaticRefinement`.
- The surface and edge-mesh refinement sources, `subsetCellSize` and
  `anisotropicSources`.
- `checkGluedRegions` and `checkAndFixIrregularConnections`.
- The octree edges and their addressing.
- `nonManifoldMeshing` and `remove`/`keepCellsIntersectingPatches`.
- The `cartesianMeshGenerator` workflow itself (`docs/upstream_coverage.md`
  §4 item 20).
