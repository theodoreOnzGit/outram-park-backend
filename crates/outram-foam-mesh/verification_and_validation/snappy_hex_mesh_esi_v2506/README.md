# snappyHexMesh vs OpenFOAM.com ESI v2506: castellation differential (gh:#1459, #1465, #1466)

**Upstream specification: OpenFOAM.com ESI v2506** (maintainer decision on
gh:#1459, 2026-11-12, "ESI preferred"; one ESI tree, so GeN-Foam #1361 can be
compared against the same sources later). OpenFOAM-dev `852527377c7e` is
background only. The issue text of #1465 and #1466 cites dev files; the ESI
counterparts are named below.

## Build recipe (ESI v2506 snappyHexMesh, dummy Pstream, in the existing tree)

Tree `/home/user/upstream-build/OpenFOAM-v2506`, environment
`/home/user/upstream-build/scripts/with-of.sh` (`WM_MPLIB=dummy`,
`WM_COMPILE_OPTION=Opt`, `WM_LABEL_SIZE=32`, DP). `WM_NCOMPPROCS=2`. Already
present: OpenFOAM, fileFormats, surfMesh, meshTools, finiteVolume, dynamicMesh,
blockMesh, extrudeModel. Added, in this order (`wmake -j2 libso <dir>`):

1. `parallel/decompose/decompositionMethods` (no scotch/metis, none needed)
2. `parallel/distributed`
3. `lagrangian/basic` (liblagrangian, for `trackedParticle`)
4. `fvMotionSolver/fvMotionSolvers`, **with `-lfvOptions` removed from its
   `Make/options`** (fvOptions drags in the whole thermophysical and
   turbulence chain; nothing in `snappyHexMesh` calls it)
5. `sampling` (`meshRefinement.C` includes `shortestPathSet.H`)
6. `mesh/snappyHexMesh`, with `-loverset` and `-I overset/lnInclude` removed
   and one file added to `Make/files`,
   `../../overset/cellCellStencil/trackingInverseDistance/voxelMeshSearch.C`
   (the only overset unit `snappyVoxelMeshDriver.C`, a dry-run path, needs;
   its include directory is added to `EXE_INC`). Originals kept beside the
   edits as `Make/options.orig`, `Make/files.orig`.
7. the application `applications/utilities/mesh/generation/snappyHexMesh`
   (see "Executable" below).

Disk cost: see "Executable ... and disk" below.

## Case (the maintainer's flange)

`tutorials/mesh/snappyHexMesh/flange` of v2506: `flange.stl` (from
`tutorials/resources/geometry/flange.stl.gz`, one solid `patch1`, 45 284 STL
lines), `blockMesh` box (-0.03..0.03, -0.03..0.03, -0.03..0.01), 20 x 16 x 12
cells, `locationInMesh (-9.23149e-05 -0.0025 -0.0025)`, `level (2 2)`,
`nCellsBetweenLevels 1`. **Reduced to what the port can express:** castellation
only (`snap false`, `addLayers false`), no `features` (needs
`surfaceFeatureExtract`, not built), no `refinementRegions` (the `refineHole`
sphere shell is not ported). The tutorial's full dictionary is NOT what is
compared; the reduced dictionary is committed beside the results.

## PREDICTIONS (written and committed BEFORE the port was run on this case)

Written from reading `snappyRefineDriver.C::doRefine` (v2506, lines 3470-3790),
`hexRef8::faceConsistentRefinement` (`hexRef8.C:1567`) and the port's
`refinement.rs`; the ESI binary had not been run when these were written.

Stages that run in ESI for this dictionary, in order: `surfaceOnlyRefine`
(maxIter 100), `gapOnlyRefine` (no gap levels: no-op), `surfaceProximityBlock`,
`removeInsideCells` (nBufferLayers 1), `shellRefine` (no shells: no-op),
`removeInsideCells` (0), `danglingCellRefine` (21, then 24 refined faces),
`refinementInterfaceRefine` (maxIter 10), `directionalShellRefine` (no-op),
`baffleAndSplitMesh`, `zonify`, `splitAndMergeBaffles`; `mergePatchFaces` runs
only `if (prepareForSnapping)`, which is `snap`, so it does NOT run here.
The port has only the first.

| # | prediction | why it can fail |
|---|---|---|
| P1 | **Iteration by iteration, the number of cells ESI's first `surfaceOnlyRefine` selects equals the port's `surface_only_refine`** (integers, same doubles, same segments, same face-only 2:1 closure), and so does the leaf-level histogram after the loop. | `refinementSurfaces::findHigherIntersection` uses an octree `findLine` on the triSurfaceMesh, the port a brute-force segment test; a segment grazing a triangle edge could split them. ~~The STL is read as ASCII text in both.~~ **WRONG, see Run 1.** |
| P2 | After all stages, ESI has MORE refined cells than the port (the port ends at the P1 octree): `danglingCellRefine` and `refinementInterfaceRefine` add level-1/2 cells; no stage of the cases here removes any. Prediction: ESI total refined (before removal) exceeds the port by a small positive amount, 0 < Delta < 5 % of the cell count. | If the flange octree has no hex with 5-6 refined faces and no level interface needing extra refinement, Delta = 0. |
| P3 | Kept cells (after removal) differ between ESI (face-connected region holding `locationInMesh`, cut cells included when reachable) and the port (centre parity). Every cell in the symmetric difference touches the surface or is one of the P2 extras; **no cell farther than its own edge length from the surface differs** apart from the P2 extras. | The port's parity test mis-classifies through-thin-wall cells. |
| P4 | Patch names differ by construction: ESI `flange_patch1` for the surface, port `surface`; ESI keeps one background patch `allBoundary` (as blockMesh), port splits the box into six (`xMin` ...). Counts of boundary faces on the surface patch differ whenever P3 differs; they are equal if P2 = 0 and P3 = 0. | |
| P5 | The final total face count differs by more than the cell count would explain alone only if ESI merged patch faces; it does not here (`mergePatchFaces` off), so face counts follow from the cell sets. | |

## Results

### Run 1, ESI side (2026-11-12) and the first miss

ESI (reduced flange, `snap false`) selects, in the first `surfaceOnlyRefine`,
914 / 4294 / 344 / 12 / 0 cells per iteration (log
`esi_log.txt` beside this file). The port with
`read_stl` gives 758 / 3500 / 348 / 14 / 0. **P1 failed at iteration 0**: 156
of the 914 level-0 cells ESI refines are not refined by the port, a miss of
17 %. Tracing the 156 cells with an independent brute-force segment test
(Python, `probe.py` logic) shows each has a face-neighbour segment that hits a
triangle of `solid patch2` or `patch3`. **The STL has four `solid ... endsolid`
blocks (6468 triangles, regions `patch1..patch4`)** and the port's ASCII reader
(`read_stl_ascii_str`) stops at the first `endsolid`: it meshed against one
solid of four. The surface search differs, not the refinement loop.

### PREDICTIONS for the follow-up (written before re-running the port)

| # | prediction |
|---|---|
| P6 | Reading all four solids (double precision, `read_stl` fixed) makes the per-iteration counts equal ESI's 914 / 4294 / 344 / 12 / 0 exactly, or within 1 cell per iteration. |
| P7 | Upstream's single-precision STL coordinates (`STLpoint` is `float`; `read_stl_zoned` already ports this) change the counts by at most a few cells (the only decisions that can flip are segments grazing a plane at a cell-centre coordinate); the result with `read_stl_zoned` equals ESI's exactly. |

### Run 2 results: stage 1 (`surfaceOnlyRefine`), ESI v2506 vs port

Selected cells per iteration (after the 2:1 closure), reduced flange:

| port reader | iter 0 | iter 1 | iter 2 | iter 3 | iter 4 | leaves | cells equal to ESI |
|---|---|---|---|---|---|---|---|
| ESI v2506 `snappyHexMesh` | 914 | 4294 | 344 | 12 | 0 | 42788 | (reference) |
| `read_stl` as found (first solid only) | 758 | 3500 | 348 | 14 | 0 | 36180 | 35068 of 42788 |
| `read_stl` fixed (all solids, double) | 914 | 4294 | 370 | 14 | 0 | 42984 | 42760 (28 only in ESI, 224 only in port) |
| `read_stl_zoned` (all solids, single precision) | 914 | 4294 | 344 | 12 | 0 | 42788 | **42788 of 42788, levels equal** |

- **P1 failed** as written (it assumed the STL was read identically). Cause 1,
  first divergent operation: `read_stl_ascii_str` stopped at the first
  `endsolid` and meshed against 1 of the flange's 4 solids (patch1; 6468
  triangles in the file). Fixed in `stl.rs` (all solids are read now).
- **P6 half held**: with all solids the first two iterations equal ESI
  (914, 4294); iterations 2-4 do not (370 vs 344, 14 vs 12), so "within 1 cell"
  was wrong.
- **P7 held in its equality, missed in its size**: single-precision STL
  corners (upstream's `STLpoint` is `float`; `read_stl_zoned` ports it) remove
  the last difference, and the effect was 196 leaves (0.46 %), not "a few
  cells". Cause 2, first divergent operation: a double-precision corner such as
  0.0075 sits on a cell face or a cell-centre plane of the background grid and
  the float rounding flips grazing segment tests. Upstream is the specification,
  so the single-precision reader is the right one for parity.
- The ESI tutorial's `controlDict` has `writePrecision 6`, so its `blockMesh`
  points are rounded (e.g. z = -0.02833335 for -0.0283333...). The
  differential run uses `writePrecision 16`; the counts above are the same with
  6 digits (914, 4294, 344, 12), so the rounding does not matter here, but a
  comparison against a 6-digit tutorial mesh should not assume that.

### Stage 2: cell removal and the stages the port does not have

ESI after `surfaceOnlyRefine` runs `removeInsideCells` (42788 -> 30264 cells),
`danglingCellRefine` (22+4+4+4+2 then 2 cells: 38 parents) and
`refinementInterfaceRefine` (0 selected), then `baffleAndSplitMesh` (30530 ->
18278 cells; 12145 faces intersected by the surface become baffles) and
`splitAndMergeBaffles`. Final ESI mesh (time 32): 18278 cells, 63194 faces,
patches `flange_patch1..4` = 9310 / 1182 / 248 / 1396 faces (the domain boundary
patch ends up empty: the kept region is the inside of the flange).

Port (octree of stage 1, `read_stl_zoned`, centre-parity removal): 18014
cells, 62644 faces, one `surface` patch with 12140 faces (ESI: 12136). Cell
comparison against the ESI final mesh: 17974 matched, **304 only in ESI (256
level-2, 48 level-1) and 40 only in port (32 level-1, 6 level-0, 2
level-2)**. 304 = 8 x 38: ESI's dangling-cell refinement split 38 cells that the
port leaves whole (32 level-1 and 6 level-0 parents, each replaced by its 8
children). That leaves **2 level-2 cells that only the port keeps**: they are
mirror images (x = +-0.009375, y = 0.0173, z = -0.00958): cells whose centre
is on the keep side by ray parity but which the surface cuts off from
`locationInMesh` (face-connected region test).

### PREDICTIONS for the baffle-region removal (written before it was run)

| # | prediction |
|---|---|
| P8 | `CastellationControls::with_baffle_region_removal()` (new: keeps the face-connected region holding `locationInMesh` across faces whose cell-centre segment misses the surface) removes exactly those 2 cells: the port keeps **18012** cells and the comparison with the ESI final mesh shows **304 only in ESI, 38 only in port, 0 level differences**. |
| P9 | The remaining 304 / 38 go away only by porting `danglingCellRefine` (nFaces 21 and 24) on the post-`removeInsideCells` mesh; that needs the nBufferLayers erosion of `splitMesh`, which is NOT ported in this change (see "What remains"). |

### Result of the baffle-region removal (P8, P9)

`CastellationControls::with_baffle_region_removal()` (code:
`refinement::baffle_region`, ESI `splitMesh` / `baffleAndSplitMesh`) was run on
the same octree. **P8 failed:** the port keeps the same **18014** cells as with
centre parity (not 18012), and the residual is unchanged (304 only in ESI, 40
only in port). So the 2 level-2 cells are not an inside/outside parity artefact:
they are connected to `locationInMesh` through faces the surface does not cut.
What the ESI log shows instead: the two cells (x = +-0.009375, y = 0.0173438,
z = -0.00958333) exist in ESI's meshes after `removeInsideCells` (time 7) and
after the dangling-cell stage (time 13) and are gone after `baffleAndSplitMesh`
(time 21), whose log reads "Introducing baffles to block off problem cells:
`markFacesOnProblemCells` marked 25295 additional internal faces". The picture
`images/slice_y.png` shows both on the outer edge of the pipe wall at the
tangent of the faceted circle, i.e. cells with a sliver of kept volume.
**Hypothesis, not isolated:** `markFacesOnProblemCells` (quality-driven removal,
`meshRefinementProblemCells.C:383`, not ported) removes them. The port's
baffle-region removal is kept (opt-in, as upstream's semantic; it changes
nothing on this case) and is tested by `baffle_region_separates_inside_from_outside`.
**P9 stands untested:** `danglingCellRefine` needs the `removeInsideCells(1)`
buffer-layer erosion first (it counts a cell's internal faces in the
post-removal mesh); neither is ported.

### Pictures (AGENTS.md: meshes are drawn)

`images/slice_z.png` (x-y plane, z = -0.009583: the flange profile, axis along
y), `images/slice_y.png` (x-z plane, y = 0.01734: the pipe ring) and
`images/slice_z2.png` (z = -0.015), from
`examples/draw_flange_differential.rs`, which draws the ESI reference cells and
the port's `castellate` output (not constants). Level 0/1/2 cells present in
both meshes are shaded pale/mid/dark green, cells only in ESI blue, only in the
port red, the STL cut by the plane is black, ticks every 10 mm. **Checked** (I
opened them): the kept region follows the STL on both sides of the wall; the
red squares are the six level-0 and 32 level-1 cells that ESI's dangling-cell
stage splits (their level-2 children are the blue ones, hidden where the
plane misses them) and the two small red level-2 cells at the outer edge of the
ring. **Not checked:** blue cells are not visible in these three planes
(they lie at other z and y); no 3-D view; no picture of the stage-1 octree
itself (its equality is an integer comparison of 42788 cells).

### Prediction ledger

| # | prediction | outcome |
|---|---|---|
| P1 | per-iteration counts and level histogram equal ESI | **failed** with `read_stl` (758 vs 914 at iteration 0); holds (42788 of 42788 cells) with the upstream reader. ~~"The STL is read as ASCII text in both."~~ was the wrong assumption: ESI reads four solids with single-precision corners. |
| P2 | ESI has more refined cells than the port, 0 < delta < 5 % | **held**: +266 cells from `danglingCellRefine` (38 parents x 7), 1.5 % of 18278 final cells; `refinementInterfaceRefine` selected 0. |
| P3 | removal differs only at cut cells and the P2 extras | **held**: only difference beyond the extras is 2 level-2 cells at the surface. |
| P4 | patch names differ; surface-face counts differ if P3 differs | **held in part**: ESI `flange_patch1..4` = 9310 / 1182 / 248 / 1396 = 12136 faces, port `surface` 12140 (+4, not attributed: the 38 refined parents and the 2 extra cells both change it); ESI keeps an empty `allBoundary` patch, the port has no box patches (none of its faces are on the box). |
| P5 | `mergePatchFaces` does not run (`snap false`) | **held**: log "Merging 0 sets of faces"; ESI face count 63194 vs port 62644. |
| P6 | all solids, double precision: counts equal ESI within 1 cell | **half**: iterations 0-1 equal, 2-4 not (370 vs 344, 14 vs 12). |
| P7 | single precision changes "a few" cells; equals ESI | **equality held, size missed**: 196 leaves (0.46 %). |
| P8 | baffle-region removal removes the 2 cells (18012) | **failed**: 18014; cause is ESI's problem-cell handling (hypothesis). |
| P9 | dangling-cell refinement needs the buffer-layer erosion | untested, not ported. |

### What remains (not done in this change)

- `removeInsideCells` with `nBufferLayers` (the point-based erosion in
  `splitMesh`, `meshRefinementBaffles.C:5157`), then `danglingCellRefine`
  (nFaces 21, 24) and `refinementInterfaceRefine` on the post-removal mesh: these
  explain 304 + 38 of the 344 residual cells.
- ~~`markFacesOnProblemCells` (2 cells)~~ **CORRECTED 2026-11-12 (round 2/3): ported; ESI's 25295 faces reproduced in ESI's cell order**, `zonify`, `splitAndMergeBaffles`,
  `mergePatchFaces` (needs `snap true`), region patches from `patchInfo`,
  multiple surfaces, faceZones/cellZones, `locationsInMesh`.
- #1465 items 1 (`(min max)` levels, curvature), 2 (`nCellsBetweenLevels` > 1),
  3 (`refinementRegions`, the tutorial's `refineHole`), 4 (features, needs
  `surfaceFeatureExtract`, not built), 5 (gap refinement, cell limits).
- Snapping and layers stages of the flange (the tutorial's `snap true`), and the
  full tutorial dictionary.
- A sphere case through ESI (the #1465 step-1 sphere pin 3360 is not compared).
- The cfMesh `snappy_layers` comparison against ESI (#1469).

### Executable (step 7 of the recipe) and disk

`parallel/decompose/decompose` and `parallel/reconstruct/reconstruct` are built
first (`processorMeshes.H`, `decompositionModel.H`); then the application with
its `Make/options` edited (original kept as `Make/options.orig`) to drop the
dummy `-L$(FOAM_LIBBIN)/dummy -lkahipDecomp -lmetisDecomp -lptscotchDecomp
-lscotchDecomp` line, which the dummy-Pstream tree does not provide and the
serial run does not need. Scripts: `/home/user/upstream-build/scripts/build-snappy.sh`
and `build-snappy-exe.sh`; logs `build-snappy-*.log` beside them. Whole build
about 25 minutes at `WM_NCOMPPROCS=2`, all at the tree's default `-O3` (no
`-O0` was needed).

**Disk.** New objects under `build/` about 85 MB (snappyHexMesh 14, sampling
34, fvMotionSolver 21, parallel 13, lagrangian 1, application 3), new libraries
and the binary about 16 MB under `platforms/` (`libsnappyHexMesh.so` 5.0 MB,
`snappyHexMesh` 0.5 MB). Free space on `/` fell from 5.2 GB to 3.9 GB over the
session; the flange run directory with its debug meshes is 158 MB, and the
rest of the fall is not attributable to this build (the merge-gate run shares
the disk). Intermediate time directories were deleted after the reference
cells were extracted.

### Run it again

```bash
cd /home/user/upstream-build/runs/snappy_flange      # blockMesh + system/ from the tutorial
/home/user/upstream-build/scripts/with-of.sh blockMesh
/home/user/upstream-build/scripts/with-of.sh snappyHexMesh -noFunctionObjects
# port side, stage by stage:
dump_castellation constant/triSurface/flange.stl -0.03 -0.03 -0.03 0.03 0.03 0.01 \
    20 16 12 2 -9.23149e-05 -0.0025 -0.0025 OUT.txt octree 100 zoned
python3 -I compare_cells.py 4/polyMesh 4/polyMesh/cellLevel OUT.txt     # stage 1
```

`system/controlDict` must have `writePrecision 16` (the tutorial's 6 rounds the
`blockMesh` points). The dictionary is `snappyHexMeshDict.reduced`, the full ESI
log `esi_log.txt`. Reference cells are committed gzipped under
`tests/fixtures/esi_v2506_flange/` and pinned by
`tests/snappy_hex_mesh_esi_flange.rs`.

### Moved pins

None moved. `read_stl_ascii_str` changed behaviour only for multi-solid files
(it returned the first solid; now all): no existing test or example reads one
(the cfMesh crate uses `read_stl_zoned_bytes`). `read_stl` itself stays
double precision (the single-precision upstream reader is the new
`TriangleSoup::from_zoned`, opt-in), and the default cell removal stays centre
parity, so the CLI and driver pins are untouched.

---

# Round 2 (2026-11-12): removeInsideCells(nBufferLayers), danglingCellRefine, problem cells

Written from reading the ESI v2506 sources listed below; the port code for
these stages did not exist when this section was committed, and the
reference cells after `removeInsideCells(1)` (30264) and after the
dangling-cell stage (30530) are the ESI intermediate meshes of the round-1 run
(`esi_removeinside_cells.txt.gz`, `esi_dangling_cells.txt.gz`, same `level i j
k` format as the stage-1 fixture; the conversion reproduces the committed final
fixture exactly, checked with `diff`).

Sources read (v2506, `src/mesh/snappyHexMesh`): `meshRefinementBaffles.C:4927`
and `:5014` (`splitMesh`: face blocking, `regionSplit`/`findRegions`, then the
`nBufferLayers` loop: cells -> points -> faces -> cells), `snappyRefineDriver.C:1002`
(`danglingCellRefine`: candidates are cells whose number of *internal* faces is
`nFaces`, then `hexRef8::consistentRefinement`), `:1760` (`removeInsideCells`),
`:3560-3640` (the stage order), `meshRefinementBaffles.C:4635`
(`baffleAndSplitMesh`) and `meshRefinementProblemCells.C:383`
(`markFacesOnProblemCells`, `removeEdgeConnectedCells` false, `minVolCollapseRatio`
unset so `checkCollapse` false: the log has no "Deleting all-anchor" line).
`handleSnapProblems` runs because `refineParams.handleSnapProblems()` defaults
true, independent of `snap`.

What the sources say, which the port is to reproduce:

1. `splitMesh(nBufferLayers = 1)` keeps the face-connected region holding
   `locationInMesh` (faces cut by the surface are blocked), then ONE buffer layer:
   every point of a face between kept and removed cells (and of a domain boundary
   face of a kept cell) is marked; every face using a marked point is marked;
   every removed cell owning or neighbouring a marked face is kept. In octree
   terms: removed cells that share a mesh point (corner or hanging edge/face
   point) with the region boundary come back.
2. `danglingCellRefine(21)` then `(24)`: a cell is refined when exactly 21 (or 24)
   of its faces are internal (1 + 5 x 4, or 6 x 4), then face 2:1 closure among
   the surviving cells; repeat until none.
3. `markFacesOnProblemCells`: marks the points/edges/faces of every face in a
   surface patch (the baffle pairs and the faces exposed by the first removal);
   a cell with all 8 anchor (own-level corner) points marked has all its internal
   faces baffled and these faces are marked as boundary at once (so a LATER cell
   in cell order can reach 8); a point with more than 3 seven-anchor cells baffles
   those cells; every internal face whose edges are all marked is baffled. The
   cell-order dependence is real and is a risk for the port.

## PREDICTIONS (committed before the port code for these stages was run)

| # | prediction | why it can fail |
|---|---|---|
| P10 | After the face-connected region plus the one point-connected buffer layer, the port's alive set equals ESI's mesh after `removeInsideCells(1)`: **30264 cells, 30264 of 30264 equal** (levels equal). | Hanging points: a removed cell is point-adjacent through a hanging edge point only if hexRef8 inserted that point into the cell's faces; the port derives point existence from the leaf corners of the whole octree. If that is wrong the count will be off by a few cells. |
| P11 | Dangling-cell refinement per iteration selects **22, 4, 4, 4, 2, 0** (nFaces 21) and **2, 0** (nFaces 24), and the alive set then equals ESI's mesh at 30530 cells, **30530 of 30530 equal**. | The internal-face count of a cell next to a removed region depends on how hexRef8 splits faces (4 sub-faces per coarse face next to finer cells, also next to removed finer cells). The port counts on the full octree with alive flags. |
| P12 | At the baffle step the number of intersected faces is **12145** (all internal, none exposed: the log says 12145 duplicate pairs). The port's count of face segments hitting the surface among alive-alive pairs is 12145 and among exposed pairs (extrapolated neighbour centre, `calcNeighbourData`) 0. | The exposed-face segment (mirror image of the owner centre) is a port reconstruction. |
| P13 | With `markFacesOnProblemCells` (simple branch) the port marks **25295** additional internal faces, the kept region after `splitMeshRegions` is **18278 cells, 18278 of 18278 equal to ESI**, and **the two level-2 cells at x = +-0.009375, y = 0.0173438, z = -0.00958333 are gone**. | The 8-anchor loop mutates the boundary marks in cell order; the port's cell order is not OpenFOAM's. If the count or the cell set differs, the next step is to reproduce hexRef8's cell order (parent keeps its label, children 1..7 appended in hex-vertex order) and rerun. |
| P14 | Final face count **63194** and the sum of boundary faces on the surface patches **12136** (ESI: 9310 + 1182 + 248 + 1396) once P13 holds. | Face count follows from the cell set; the surface-face count needs the exposed faces to equal ESI's. |

Disclosure: P10-P14 are integers read from the round-1 ESI log and meshes; no
port run of these stages had happened when they were written.

## Round 2 results (2026-11-12, release, `tests/snappy_hex_mesh_esi_flange.rs`)

Code: `src/snappy_hex_mesh/esi_split.rs` (`remove_inside_cells`,
`dangling_cell_refine`, `baffle_and_split_mesh`, `esi_stages`), opt-in through
`CastellationControls::with_esi_split_mesh_removal()` (`CellRemoval::EsiSplitMesh`,
needs `with_surface_intersection_refinement()`). Defaults unchanged.

| stage | ESI v2506 | port | cells equal |
|---|---|---|---|
| `surfaceOnlyRefine` (round 1) | 42788 | 42788 | 42788 of 42788 |
| `removeInsideCells(1)` | 30264 | 30264 | **30264 of 30264** |
| `danglingCellRefine(21)` selected per iteration | 22, 4, 4, 4, 2, 0 | 22, 4, 4, 4, 2, 0 | |
| `danglingCellRefine(24)` selected per iteration | 2, 0 | 2, 0 | |
| after both dangling stages | 30530 | 30530 | **30530 of 30530** |
| faces cut by the surface (baffles) | 12145 | 12145 (boundary: 0) | |
| `markFacesOnProblemCells`, additional internal faces | 25295 | **24646** | |
| final castellated cells | 18278 | 18278 | **18278 of 18278** |
| final faces | 63194 | 63194 (internal 51058) | |
| surface-patch boundary faces (all regions) | 12136 | 12136 | |

Before this round: 18014 cells, 62644 faces, 12140 surface faces, residual
304 / 40 (round 1 table above).

### Prediction ledger, round 2

| # | outcome |
|---|---|
| P10 | **held**: 30264 of 30264. The geometric point model (corner of any leaf, dead or alive; closed-box adjacency; hanging edge points inserted into face rings) reproduced the buffer layer exactly. |
| P11 | **held**: selected 22, 4, 4, 4, 2, 0 and 2, 0; 30530 of 30530. |
| P12 | **held**: 12145 internal cut faces, 0 cut boundary faces (exposed-face segments use the `calcNeighbourData` extrapolation). |
| P13 | **cell set held, count missed**: final 18278 of 18278 equal to ESI and the two level-2 cells are gone, but the port marks **24646** problem faces against ESI's **25295** (-649, -2.6 %). Not isolated. Candidate causes, none tested: (a) faces ESI counts that the port excludes (`!hit`, internal-only), (b) cell-order dependence of the 8-anchor loop (the port visits cells in nested leaf order, OpenFOAM in blockMesh order with children appended), (c) ESI counting a face once per side of a coupled/baffled pair. The extra 649 faces are not needed for the cell set: it is equal. |
| P14 | **held**: 63194 faces and 12136 surface faces. |

**Cell-order sensitivity** (the stated risk of P13) was not measured: with the
port's order the cell set equals ESI's, and the 649-face miss may or may not be
the order. A reversed-order run would answer it; not done.

### Hypothesis P8 of round 1, closed

The two port-only level-2 cells are removed by `markFacesOnProblemCells`, as
hypothesised: the port, with that stage, no longer keeps them (18278 of 18278).
The baffle-region removal of round 1 alone (`with_baffle_region_removal`) was
not the cause, as recorded.

## Round 2, task 2: region patches (predictions, committed before the code)

ESI source read: `meshRefinementBaffles.C:200-284` (`getIntersections`) and
`:285-420` (`getBafflePatches`): for a cut face, `findNearestIntersection` along the
cell-centre to cell-centre segment returns the first hit from the segment start
(`region1`, the owner side) and from its end (`region2`, the neighbour side); the
owner-side boundary face gets `globalToMasterPatch[region1]`, the neighbour-side face
`globalToMasterPatch[region2]`. So **a kept cell's boundary face takes the region of
the first triangle hit walking from that cell's own centre towards the cut
neighbour.** Faces baffled by `markFacesOnProblemCells` carry
`nearestAdaptPatch` instead (`meshRefinement.C:545`, a topological walk from the
existing patch faces). The ESI log gives 12132 cut faces on the final mesh and 12136
patch faces: **4 faces come from the problem-cell rule.**

`outram-park-fork-cfmesh/src/patches.rs` (`assign_patches_by_region`) assigns the
region of the triangle NEAREST to the face centroid. Its module comment says that is
"exactly how snappyHexMesh assigns a cut face to a surface region"; the ESI source
above says otherwise (first hit along the cell-cell segment), so that sentence is
corrected in place in this change. Dependency direction: `outram-park-fork-cfmesh`
depends on `outram-foam-mesh` only optionally, and `assign_patches_by_region` takes
cfMesh's `VolumeMesh`, so it cannot be called from here, and `SurfaceRegions` cannot
move here without making cfMesh depend on this crate unconditionally. What is shared
in spirit is the label-to-name data (per-triangle region index plus names); the ESI
rule needs the segment, so it is written here.

| # | prediction (integers, before the code) |
|---|---|
| P15 | With the ESI rule (first hit from the kept cell's centre; the 4 problem faces take the region of the nearest cut face of a kept cell) the port's faces per patch are **flange_patch1..4 = 9310 / 1182 / 248 / 1396**, equal to ESI. |
| P16 | The cfMesh rule (nearest triangle to the face centroid) applied to the same 12136 faces gives counts that are NOT all equal to ESI's: at least one patch differs, and at most 5 % of the faces (606) change patch. It can fail by agreeing exactly, if regions meet only along edges the cell faces avoid. |

### Round 2, task 2 results (2026-11-12, `region_patches_against_esi`)

Code: `CastellationControls::with_surface_region_patches(patch_names, region_of_tri)`
(needs `with_esi_split_mesh_removal()`), `TriangleSoup::segment_first_hit`,
`esi_split::boundary_regions`; the assembler of `castellation.rs` now has one boundary
bucket per surface region. Default (no regions) is the single `surface` patch, unchanged.

| patch | ESI v2506 | port, ESI rule (first hit from the kept centre) | port, cfMesh rule (nearest triangle to the face centre) |
|---|---|---|---|
| flange_patch1 | 9310 | **9310** | 9320 |
| flange_patch2 | 1182 | **1182** | 1182 |
| flange_patch3 | 248 | **248** | 242 |
| flange_patch4 | 1396 | **1396** | 1392 |
| total | 12136 | 12136 | 12136 |

- **P15 held**: the four patch counts equal ESI's exactly. The 4 faces blocked by
  `markFacesOnProblemCells` take the region of the nearest cut face by face-centre
  distance (a stand-in for ESI's topological `nearestPatch` walk); the counts match,
  but this does not prove they are the same 4 faces assigned the same way. Per-face
  equality against ESI's `polyMesh/boundary` and face list was not checked: only the
  counts were.
- **P16 held**: the nearest-triangle rule of `outram-park-fork-cfmesh`'s
  `assign_patches_by_region` differs from ESI's, on **10 of 12136 faces (0.08 %)**:
  counts 9320 / 1182 / 242 / 1392. Far below the 5 % bound I predicted, but not zero.
- **Reuse**: `assign_patches_by_region` cannot be called here (it takes cfMesh's
  `VolumeMesh`; that crate depends on this one only optionally, so moving
  `SurfaceRegions` here would make cfMesh depend on this crate unconditionally). The
  ESI rule needs the cell-cell segment anyway. `bash scripts/release-lockstep.sh
  --dry-run` was therefore not run: no dependency edge was added. The cfMesh module
  comment's claim that the nearest-triangle rule is "exactly how snappyHexMesh assigns
  a cut face" is struck through and corrected in `patches.rs` (doc only).

### Round 2 pictures

`images/round2_slice_z.png` (z = -0.009583), `images/round2_slice_y.png`
(y = 0.01734375) and `images/round2_slice_z2.png` (z = -0.015), from
`examples/draw_flange_differential.rs`, drawn from the ESI reference cells and the
port's `castellate` output with the ESI removal chain and region patches (the
slices of round 1 are kept beside them for the before/after). **Looked at (I opened
all three):** every cell is green, none red (port only) or blue (ESI only), as the
integer comparison says (18278 of 18278), and in `round2_slice_y.png` the ring has
no small cells left at its outer tangent where round 1 had two red ones; the patch
dots follow the STL cut (black line): patch1 (orange) on the outer faces, patch4
(yellow) on the bore and the slot, patch2 (magenta) along the flat bottom, patch3
(cyan) on the top flat. **Not checked:** the dot band is 0.8 mm, so some dots in
`round2_slice_z2.png` (below the lower stub, left of the upper arm) belong to faces
of cells at other z, not to the plane; no 3-D view; which faces are the 4
problem-cell patch faces was not marked.

---

# Round 3 (2026-11-12): problem-face order, per-face equality, snapping

ESI v2506 re-run (same dictionary, `writePrecision 16`, debug meshes) into
`/home/user/upstream-build/runs/r3_cast`; `extract_esi_fixtures.py` (beside this
file) writes the committed references `esi_baffle_cell_order`, `esi_problem_faces`,
`esi_final_cells_ordered`, `esi_final_points`, `esi_final_faces`,
`esi_final_boundary` (`tests/fixtures/esi_v2506_flange/`).

## Evidence gathered BEFORE the port change (disclosed, not predictions)

A pure-Python replica of `markFacesOnProblemCells` (v2506
`meshRefinementProblemCells.C:383-1000`, simple branch, no collapse check) run on
ESI's own time-16 mesh (the mesh `baffleAndSplitMesh` runs it on: baffled, 30530
cells) reproduces **25295 faces and the identical face set as ESI's
`16/polyMesh/sets/problemFaces`** when cells are visited in ESI's cell order, and
gives **24331** when the same cells are visited in reverse. So the algorithm as
read is complete, and the count depends on the visit order of the 8-anchor loop
(it marks faces, hence points, as it goes). The port's leaf order (nested i,j,k,
dangling children appended) gave 24646.
ESI's cell order is not a simple function of the lattice: after
`surfaceOnlyRefine` iteration 1 it is exactly "parent keeps its label as child 0,
children 1..7 appended per parent in refined-cell order, octant bit0 = x, bit1 = y,
bit2 = z" (0 mismatches of 10238 cells); from iteration 2 it is not: the child
numbering follows each parent's own hex frame (hexRef8 orders children by the
cell's anchor-point numbering, which comes from polyMesh face order): 13 distinct
child permutations among the iteration-2 parents, 8 of them determined by the
parent's octant alone and the rest differing in 2 to 3 slots for about 1 % of parents.
Deriving it needs hexRef8's cell orientation and polyTopoChange's face reordering:
NOT ported.

## PREDICTIONS (committed before the port was changed or run)

| # | prediction |
|---|---|
| P17 | `CastellationControls::with_esi_problem_cell_order(keys)` (visit the 8-anchor loop in the ESI cell order of `esi_baffle_cell_order`) makes the port mark **25295** problem faces, and the set of marked faces (as pairs of cell keys) equals `esi_problem_faces` exactly. Reason: the replica above; the port's topology (leaf corners, hanging edge/face points in face rings) already matched on every count so far. Can fail if the port's anchor test differs from `pointLevel <= cellLevel` for a cell with a hanging corner. |
| P18 | Per-face equality of the final castellated mesh with ESI's `32/polyMesh` (face keyed by its vertex ring on the level-2 lattice, up to cyclic rotation, with owner key and neighbour key or patch name; orientation compared): **all 63194 faces equal, including orientation**, except at most 8 boundary faces whose patch differs (the 4 problem-cell faces use the nearest-cut-face stand-in for ESI's topological `nearestPatch`). Point set equal (26662). |
| P19 | Without the order (default leaf order) P18 still holds for internal faces and counts, but the problem-face count stays 24646 (pinned as the measured effect of the visit order). |

## Round 3, task 1: first results and the next prediction (committed before the fix)

- **P17 held** (`problem_faces_in_esi_cell_order_equal_esi`): with ESI's cell order
  the port marks **25295** problem faces and the set equals ESI's
  `problemFaces` (as pairs of cell keys), cells 18278 of 18278. The 649-face miss of
  round 2 was the visit order, not a missing rule.
- **P18 failed**: per-face comparison of the final mesh. Cell set, point set
  (26662) and face count equal, but **1853 faces differ** (1853 in ESI only, the same
  number in the port only), none with the same vertex set. The first ones: ESI's
  face between level-1 cells `(1,24,15,9)|(1,24,15,10)` has **5 vertices** (a hanging
  point on the midpoint of one edge, the corner of a level-2 neighbour), the port's has
  4. The port's final assembler (`MeshAssembler`) writes quads only, so its polyMesh has
  T-junction faces where ESI's hexRef8 has inserted the edge points into the face rings
  (ESI 5- and 6-gons). `esi_split::build_topo` already builds such rings, for the
  stage logic, but the final assembly did not use them. Topologically the port's mesh
  is not conformal there (the edge of the neighbouring faces is split, this face's is
  not), which matters for snapping.

| # | prediction (before the fix) |
|---|---|
| P20 | With the hanging edge points inserted into the face rings of the final mesh (`esi_split::insert_hanging_points`, the points being the corners of every leaf of the octree incl. removed ones, as hexRef8's rings at refinement time), **all 63194 faces equal ESI's** (ring up to rotation, orientation, cells, patch), the 1853 differing faces being exactly the faces with a hanging point; patches may differ on at most 8 faces (problem-cell patch stand-in). |

### Round 3, task 1: results (2026-11-12, release, `tests/snappy_hex_mesh_esi_flange.rs`)

| # | outcome |
|---|---|
| P17 | **held**: 25295 problem faces, the same faces, cells 18278 of 18278 (`problem_faces_in_esi_cell_order_equal_esi`). Python replica on ESI's own mesh: 25295 in ESI order, 24331 reversed; port: 25295 in ESI order, 24646 in leaf order. The count depends on the 8-anchor loop visit order and nothing else. |
| P18 | **failed** (1853 faces, hanging edge points missing from the port's rings), see above. |
| P19 | **held**: default leaf order, 24646, pinned in `round2_stages_equal_esi`. |
| P20 | **held**: with `esi_split::insert_hanging_points` (always applied under `CellRemoval::EsiSplitMesh`) **all 63194 faces equal ESI's** (ring up to rotation, orientation relative to the owner, both cells, patch), 51058 internal + 12136 boundary, **including the 4 problem-cell faces** whose patch the nearest-cut-face stand-in assigns (no face differs; 0 of the allowed 8). 1853 faces have 5+ vertices, as in ESI. Point sets equal (26662). |

**Not compared**: face, cell and point NUMBERING. ESI's cell labels follow hexRef8's
refinement history (children numbered by the parent's own hex frame, see above) and
its face order is polyTopoChange's upper-triangular renumbering; the port uses its own
(owner, then neighbour) order, so owner/neighbour arrays are not equal as arrays, only
as the set of (face, cell, cell) records. The 8-anchor visit order is therefore an
INPUT of the port (`with_esi_problem_cell_order`, fixture `esi_baffle_cell_order`),
not derived. With the default (port leaf order) the cell set and everything else
agree and only the problem-face selection differs (24646 faces, same final cells here).

**Defect fixed**: the final assembler wrote T-junction faces (quads next to split
edges); hexRef8's rings carry the edge points. Opt-in only (`EsiSplitMesh`); the default
path (`CentreParity`, `BaffleRegion`) still writes quads and its pins are unchanged.

## Round 3, task 2: snapping against ESI v2506, stage by stage (predictions)

**Case and reference.** `snappyHexMeshDict.snap` (beside this file) is the reduced
flange dictionary with `snap true`, the tutorial's `snapControls` (`nSmoothPatch 3;
tolerance 1.0; nSolveIter 300; nRelaxIter 5`) and the tutorial's
`meshQualityControls` (`meshQualityDict`, `relaxed { maxNonOrtho 75; }`,
`nSmoothScale 4; errorReduction 0.75`). **Deviation from the tutorial, stated:**
`nFeatureSnapIter`, `implicitFeatureSnap`, `explicitFeatureSnap` and
`multiRegionFeatureSnap` are LEFT OUT (the tutorial's `nFeatureSnapIter 10` with
`multiRegionFeatureSnap true` makes ESI attract 24 feature points and 1446 feature-edge
points between the four STL regions, from the multi-patch feature detection of
`snappySnapDriverFeature.C`, which the port does not have; no `.eMesh` is needed for
that, but it is a different algorithm). Without them ESI runs the plain loop of
`doSnap` (`nFeatIter = 1`). ESI log: `esi_snap_log.txt.gz`. With `snap true` the
castellated mesh is unchanged (18278 cells, 63194 faces, 26662 points; "Merging 0 sets of
faces"), so task 1's equality carries over.

ESI sequence read (v2506 `snappySnapDriver.C`, `motionSmootherAlgo.C`):
`calcSnapDistance` (872: `snapTol * max incident edge length` per patch point),
`motionSmoother` + `checkMesh` of the initial mesh (0 faces in error),
`preSmoothPatch` (872-985): `nSmoothPatch` times { `smoothPatchDisplacement` (290-
560: blends the average of connected boundary-face centres and of connected internal-
face centres, internalBlend 0.1, blend 0.1 with the current position),
`setDisplacement`, `scaleMesh` (quality check, scale 1 when no face is in error) };
then one morph iteration: `calcNearestSurface` (nearest surface point within
`snapDist`), `detectNearSurfaces` (default on; "Overriding ... at 0 out of 12130
points" here), `outwardsDisplacement`, `freezeExposedPoints`, `setDisplacement`,
`smoothDisplacement` (2134: `nSmoothDispl` = `nSolveIter` 300 iterations of
`motionSmootherAlgo::smooth` with edge weights `1/(wall distance + SMALL)` from
`edgePatchDist`, a `PointEdgeWave`), `scaleMesh` (2208: up to `2 nSnap` scaling
iterations; faces failing the full `meshQualityControls` get `errorReduction`,
`nSmoothScale` `minSmooth` passes, move again), `repatchToSurface` (42 faces),
then `mergePatchFaces`/`mergeEdges` (2118 sets, 2005 points: the final ESI mesh has 61034
faces and 24615 points; NOT ported, so the comparison stops at the snapped mesh of
time 36 with 63194 faces).

**Method.** The port starts from ESI's castellated polyMesh READ IN (faces and
owner/neighbour from `esi_final_faces.txt.gz`, points as exact doubles from the binary
`snap_points_32`), so numbering, ring start vertices and summation order are ESI's and
a difference names an operation, not an ordering. References (binary, exact doubles,
`extract_snap_fixtures.py`): points after each `preSmoothPatch` iteration (runs with
`nSmoothPatch` 1, 2, 3), `34/displacement` (patch points: the displacement handed to
`setDisplacement`; internal points: after `smoothDisplacement`), points at time 35
(scaling iteration 0) and 36 (the snapped mesh). Counts below are over the 26662
points; 12130 are patch points.

| # | prediction (committed before any port code for snapping exists) |
|---|---|
| P21 | Surface attraction: the port's displacement (`calcNearestSurface` with `snapDist`, then `outwardsDisplacement`; `detectNearSurfaces` and `freezeExposedPoints` change nothing here, ESI: 0 overrides, no frozen points) equals ESI's patch-point values of `34/displacement` on **all 12130 points to within 1e-12 m**; the mean wanted distance is ESI's **1.688540950712846e-4 m** (min 6.548561578869996e-8, max 7.516794439290358e-4) to 1e-12 relative. Bit identity is NOT predicted (the nearest-point-on-triangle routines differ in operation order: `triangle::nearestPointClassify` against `TriangleSoup::closest_point`), only <= 1e-12 m. |
| P22 | `smoothPatchDisplacement` + move, three iterations, all 26662 points: **bit-identical to ESI after iterations 1, 2 and 3** (the faces, face-centre formula and loop order are ESI's; no transcendental is involved; neither build uses FMA). Expected failure mode: face-centre estimate accumulation order. |
| P23 | `smoothDisplacement` (300 iterations): all 26662 points of the displacement equal ESI's `34/displacement` **to 1e-12 relative to the largest displacement, bit-identical on at least 90 %**; the `edgePatchDist` wave (ported as `PointEdgeWave`'s propagation with its tolerance) gives ESI's edge distances exactly. Risk: the wave's order of updates on ties. |
| P24 | `scaleMesh` iteration 0 (full displacement) finds faces in error of the three kinds ESI reports, **4 pyramid-volume, 2 tet-quality, 2 twist**, all other checks 0, and the second iteration (scale min 0.75) is clean; the points equal ESI's time 36 to 1e-12 m. This needs the ESI face checks (`polyMeshGeometry`); if they are not all ported in this round the stage is reported as NOT DONE rather than approximated. |
| P25 | `checkMesh` counts on the snapped mesh (9 `meshQualityControls` checks): 0 for all (ESI log, "Checking final mesh"). |

### Round 3, task 2: results so far (2026-11-12, release, `tests/snappy_hex_mesh_esi_snap.rs`)

Code: `src/snappy_hex_mesh/esi_snap.rs` (opt-in, the older `snapping::snap` is
untouched). Stage table, from ESI's castellated mesh read in:

| stage | compared with | port | result |
|---|---|---|---|
| `preSmoothPatch` iteration 1 (`smoothPatchDisplacement`, move) | ESI time-33 points of the `nSmoothPatch 1` run | 26662 points | **bit-identical, 79986 of 79986 components** |
| iteration 2 | `nSmoothPatch 2` run | | **bit-identical, 79986 of 79986** |
| iteration 3 | `nSmoothPatch 3` run | | **bit-identical, 79986 of 79986** |
| `calcSnapDistance` + `calcNearestSurface` (12130 patch points, from the time-33 mesh) | patch points of `34/displacement` | | **36389 of 36390 components bit-identical, worst 2.2e-19 m**; wanted displacement min 6.548561578869996e-8 and max 7.516794439290358e-4 equal ESI's log exactly, mean 1.688540950712846e-4 equal to 1e-18 |
| `outwardsDisplacement` | ESI warning "mesh point 16952" | | same first offender, 16952 |
| `smoothDisplacement` (`edgePatchDist` wave + 300 `smooth` passes), from ESI's patch displacement | all of `34/displacement` | 26662 points | **bit-identical, 79986 of 79986** |

**Prediction ledger.** P21 **held** (the <= 1e-12 m bound, better: 2.2e-19 m; bit identity
was not predicted and holds for all but one component). P22 **held** (bit-identical, as
predicted). P23 **held after a defect was found and fixed**: the first run was **47.98 %
bit-identical with a worst difference of 4.76e-7 m (6e-4 of the largest displacement)**:
first divergent operation, the order of the mesh edges. `primitiveMesh::calcEdges`
creates edges face by face and then **renumbers them by lower end point and, within it,
higher end point** (the `oldToNew` pass, `primitiveMeshEdges.C:300-440`); `avg` sums
per point in edge-index order and 300 passes amplified the summation-order difference to
6e-4. I had expected "round-off" and was wrong about the size: an order difference over 300
smoothing passes is not 1e-16. With the renumbering ported (`mesh_edges`) the result is
bit-identical, which also shows the `PointEdgeWave` port (propagation order, tolerance 0.01)
is exact on this mesh. P24 and P25 below.

**Pictures** (`images/snap_slice_z.png`, `images/snap_slice_y.png`, from
`examples/draw_snap_differential.rs`; z = -0.013333 and y = 0 are cell-face planes of
the castellated mesh): grey = castellated edges lying in the plane, blue = ESI's snapped
mesh (time 36), red = the port with the full displacement (scale 1), black = the STL cut.
**Looked at (I opened both):** the snapped edges follow the STL outline, including the
circular bores and the slot, with the cut cells' edges pulled onto the surface and
the grey grid showing the undisplaced position. **Not seen:** any red, because red
is drawn under blue and the largest difference between the port at scale 1 and ESI's time-36
mesh is 6.0e-5 m (0.9 px at 15000 px/m), which is the effect of the `errorReduction`
scaling (below); the images cannot show it, the number does. No 3-D view.

### Round 3, task 2: P24 and P25 NOT DONE (stated, not approximated)

`scaleMesh` iteration 0 needs ESI's `meshQualityControls` face checks
(`motionSmootherAlgoCheck.C` + `polyMeshGeometry.C`: nine checks, about 1500 lines of
C++). They are NOT ported in this round, so the stage that turns the unscaled mesh
(ESI time 35; the port reproduces its displacement bit for bit above) into the snapped
mesh of time 36 is open, and so is the `checkMesh` count table of P25. Measured instead:
the largest distance between the port's unscaled mesh and ESI's time-36 mesh is
**6.0e-5 m**, i.e. the effect of `errorReduction 0.75` at the points of the 8 faces
that failed (4 pyramid-volume, 2 tet-quality, 2 twist; ESI log, iteration 0), which the
port does not scale. What was read for the next round:

- `checkMesh` runs: `checkFaceDotProduct`, `checkFacePyramids`, `checkFaceTets`,
  `checkFaceAngles`, `checkFaceSkewness`, `checkFaceWeights`, `checkVolRatio`,
  `checkFaceTwist`, `checkCellDeterminant` (the disabled ones: `minArea`,
  `minTriangleTwist`, `minEdgeLength` are -1 in `meshQualityDict`).
- **Source observation, recorded for the log line it explains:** the pyramid check is
  called as `checkFacePyramids(report, minTetQuality, ...)`, with `minTetQuality`
  (1e-15) as its threshold, while the log line that follows prints `minVol`
  (1e-13). The count in ESI's "faces with face pyramid volume < 1e-13" is therefore
  the number of faces with pyramid volume above `-1e-15` (owner side) or below
  `1e-15` (neighbour side).
- `tetrahedron::quality()` is `mag()/(8/(9 sqrt 3) * pow3(min(circumRadius, GREAT)) + ROOTVSMALL)`:
  a `sqrt` and a division by `circumRadius`, no `pow`; face-pyramid and tet checks use
  `polyMeshGeometry`'s own cell centres (`updateCellCentresAndVols`: face-centre
  estimate, bounding-box guard), not `primitiveMesh`'s.
- After a failed check: `errorReduction` at the points of the wrong faces, `nSmoothScale` (4)
  `minSmooth` passes over the affected points with `calcEdgeWeights(oldPoints)`, move
  again. `repatchToSurface` (42 faces) and `mergePatchFaces`/`mergeEdges` (2118 face
  sets, 2005 points: 63194 to 61034 faces) follow and are also not ported.
- Reuse note for #1467: the dev-lineage `outram_park_fork_cfmesh::checks` cannot be called
  from this crate (cfMesh depends on it, optionally); a quality check shared by both
  has to live here, ESI-translated, and cfMesh call it.

### Round 3 summary of the prediction ledger

| # | outcome |
|---|---|
| P17 | held (25295, same faces, ESI cell order as input) |
| P18 | failed (1853 faces without hexRef8 hanging points); cause fixed |
| P19 | held (24646 in the port's leaf order) |
| P20 | held (all 63194 faces equal ESI per face, patches included) |
| P21 | held (36389 of 36390 bit-identical, worst 2.2e-19 m) |
| P22 | held (bit-identical, 3 iterations) |
| P23 | held after fixing the edge order (first run 47.98 %, 4.8e-7 m) |
| P24 | not done (face checks not ported); unscaled mesh differs from ESI time 36 by at most 6.0e-5 m |
| P25 | not done |

### Moved pins and defects, round 3

Pins moved: none. `CellRemoval::EsiSplitMesh` now writes hexRef8 hanging-point
rings (5-8 vertex faces) in `CastellatedMesh::topology`; face and cell counts are
unchanged (`castellate_esi_split_mesh_cells` still 18278 / 63194), the default
removal modes are untouched. Defects found: (1) final assembler wrote T-junction quads
(fixed, opt-in path); (2) the "649-face problem-face miss" was the visit order, not a missing
rule (README round 2 corrected); (3) in this round's own port, the edge renumbering of
`calcEdges` (fixed before the first commit that claims P23).

## Round 4, tasks 1 and 2: the nine face checks and `scaleMesh` (predictions)

Written and committed before any round-4 port code exists.

**Correction of a round-3 statement, from reading `motionSmootherAlgoCheck.C` again
before porting.** Round 3 recorded that ESI calls `checkFacePyramids(report,
minTetQuality, ...)`. That is true of the OVERLOAD that takes a `polyMeshGeometry`
(`checkMesh(report, dict, meshGeom, points, checkFaces, ...)`, line 503 onwards, used by
`snappyLayerDriver`). `snappySnapDriver::scaleMesh` calls `meshMover.scaleMesh(checkFaces,
baffles, true, nInitErrors)`, which calls the OTHER overload,
`checkMesh(false, mesh_, meshQualityDict, checkFaces, baffles, wrongFaces)` (line 59), and
that one passes `minVol` (1e-13) to `polyMeshGeometry::checkFacePyramids`, using the
mesh's own `cellCentres()` / `faceCentres()` / `faceAreas()` (`primitiveMeshTools`, as
`polyMesh::movePoints` computes them), not `polyMeshGeometry`'s incremental ones. The
snapping port therefore uses the mesh overload. On the flange the distinction cannot be
seen in the result: ESI `checkMesh -meshQuality` at time 35 finds the same 4 pyramid faces
with `minVol` 1e-13 and with 1e-15.

**Reference (new, `tests/fixtures/esi_v2506_flange/esi_t35_wrong_faces_by_check.txt.gz`).**
A fresh ESI run of `snappyHexMeshDict.snap` (`debugFlags (mesh)`, binary write). It
reproduces `snap_points_32/35/36` bit for bit **only if `blockMesh` is run first with the
ASCII `controlDict`, `writePrecision 16`: with a binary controlDict the background points
are exact doubles and every point differs by up to 7e-16.** Then `checkMesh -meshQuality
-time 35` (the mesh overload of `motionSmootherAlgo::checkMesh`), once per check with the
other eight disabled, gives the exact face sets of the time-35 mesh (scaling iteration 0,
full displacement). Faces in error: pyramids {56918, 58032, 58040, 58067}; tet quality
{15275, 28221, 56918, 58032, 58040, 58067}; twist {51965, 56915, 56918, 58032, 58040,
58067}; non-orthogonality, concavity, skewness, weights, volume ratio, determinant: none.
Union 8 faces. ESI's log counts are cumulative-new per check (4, +2, +2); the port will
print the same.

| # | prediction (committed before the port code) |
|---|---|
| P26 | The ported nine checks (`check_mesh::esi_*`: `checkFaceDotProduct`, `checkFacePyramids`, `checkFaceTets` incl. `polyMeshTetDecomposition`, `checkFaceAngles`, `checkFaceSkewness`, `checkFaceWeights`, `checkVolRatio`, `checkFaceTwist`, `checkCellDeterminant`) on ESI's time-35 mesh give **exactly** the per-check face sets above (pyramids 4, tets 6, twist 6, the other six checks empty; union 8) and **zero** faces on ESI's time-32 (castellated) and time-36 (snapped) meshes. Pass/fail is discrete, so bit identity of the metric is not needed, only that no face lies within round-off of a threshold; none is expected to. |
| P27 | `scaleMesh` ported (`errorReduction 0.75`, `nSmoothScale 4`, `calcEdgeWeights(oldPoints)`, `minSmooth` over patch points then internal points, `getAffectedFacesAndPoints`) from the time-34 displacement and the castellated points: iteration 0 finds the 8 faces above, iteration 1 (scale min 0.75, max 1) finds none, and the resulting points equal ESI's time 36 **bit for bit on all 79986 components**. The operations are `+ - * /`, `min` and `sqrt` in upstream's order; the earlier stages were bit-identical for the same reason. Expected failure mode if wrong: the affected-point set (`pointCells` growth) or the order of `scale_ *= errorReduction` before `minSmooth`. |
| P28 | `checkMesh` of the port's scaled mesh: **0 faces for all nine checks**, as ESI's "Moved mesh" iteration 1 and `checkMesh -meshQuality -time 36`. |
