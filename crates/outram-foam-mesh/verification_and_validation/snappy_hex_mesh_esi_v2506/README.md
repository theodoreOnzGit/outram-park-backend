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
- `markFacesOnProblemCells` (2 cells), `zonify`, `splitAndMergeBaffles`,
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
