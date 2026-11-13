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
serial run does not need. ~~Scripts: `/home/user/upstream-build/scripts/build-snappy.sh`
and `build-snappy-exe.sh`; logs `build-snappy-*.log` beside them.~~ **CORRECTED
2026-11-13** (container reset wiped `/home/user/upstream-build`; those two
scripts were outside the repo and are gone): rebuild with
`scripts/upstream/build-esi-v2506-and-genfoam.sh` (stages `closure` and
`meshing`), logs in `/home/user/upstream-build/logs/`. It builds the full
`liboverset` (in the closure) instead of the pruned `overset` + one-file edit
above, and builds `fvMotionSolvers` unmodified because `libfvOptions` now
exists; the `snappyHexMesh` application edit is unchanged. Whole build
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

### Round 4, tasks 1 and 2: results (2026-11-12, release, `tests/snappy_hex_mesh_esi_scale.rs`)

Code: `check_mesh::esi_*` (the nine checks, `esi_check_mesh`, `EsiQualityControls`,
`EsiGeometry`; beside the older `check_quality`, which is untouched; no kovan review
stamp exists in this crate) and `esi_snap::{scale_mesh, scale_mesh_loop, MotionSmoother,
affected_faces_and_points}`. `esi_snap::cell_centres` now delegates to the new
`cell_centres_vols` (same operations); the vector helpers became `pub(crate)`.

| stage | compared with | result |
|---|---|---|
| nine checks alone, ESI time-35 mesh | `checkMesh -meshQuality` with one check enabled | **all nine face sets equal**: pyramids {56918, 58032, 58040, 58067}, tets 6, twist 6, the other six empty; union 8; cumulative-new counts 0, 4, +2, 0, 0, 0, 0, +2, 0 as ESI's log |
| same checks, threshold `minVol` 1e-15 | ESI with 1e-15 | same 4 faces |
| ESI time-32 and time-36 meshes | ESI "Mesh OK" | 0 faces |
| `oldPoints + 1 * displacement` | ESI time 35 | **bit-identical, 79986 of 79986** |
| `scaleMesh` loop (2 iterations: 8 faces at scale 1, 0 faces at scale min 0.75) | ESI log iterations 0 and 1 | same counts and same 8 faces |
| snapped points | ESI time 36 | **bit-identical, 79986 of 79986** |

**Ledger.** P26 **held**, P27 **held**, P28 **held**; no round-off residual to count
(no transcendental other than `cos`/`sin` of the two thresholds, through
`RealMath`, and they agree). One pre-declared alternative was measured and refuted
rather than tuned: whether `curPoints` scales PATCH points by the scale field (the
`scale_*displacement_` product's boundary values might have stayed 1). With patch
points unscaled the run does not converge (10 iterations, 77664 of 79986 bit-identical,
worst 1.2e-4 m), so ESI does scale them; pinned by
`unscaled_patch_points_reading_is_refuted`. Round-3 statement "unscaled mesh differs
from time 36 by at most 6.0e-5 m" is now explained: that was the scaling of the 8 faces'
neighbourhood. Pins moved: none. Defects: the round-3 pyramid-threshold note (above).
Not covered by the port: baffles, coupled patches, `minArea`, `minTriangleTwist`,
`minFaceFlatness`, `minEdgeLength` (all -1 or absent in `meshQualityDict`), the
`polyMeshGeometry` overload used by layer addition. No picture drawn for this stage:
the change (8 faces' points scaled by at most 0.25 of a sub-0.1 mm displacement) is
below a pixel, as for round 3.

## Round 4, task 3a: `repatchToSurface` (predictions)

Committed before the port code. `snappySnapDriver::repatchToSurface` (v2506 `:2275`): per
boundary face `faceSnapDist` = max over its points of `calcSnapDistance` (snap tolerance 1.0
times the longest incident edge, evaluated on the SNAPPED mesh), `findNearestRegion` of the
face centre (`mesh.faceCentres()`) against the unzoned surfaces within `sqr(faceSnapDist)`, and
the face goes to the patch of the hit region (`globalToMasterPatch_`; here STL region k to
`flange_patch(k+1)`); `createBaffles(ownPatch, neiPatch)` with equal arrays only changes the
patch. Reference: the same ESI run with `mergePatchFaces false` (top level; `mergeType
NONE`, so neither merge happens), whose final mesh is the repatched mesh; ESI log "Repatched
42 faces". Faces are matched by (owner cell, ring up to rotation).

| # | prediction |
|---|---|
| P29 | Starting from ESI's castellated patch assignment and the snapped (time 36) points, the port repatches **exactly 42 faces**, the same 42 faces as ESI, each to the same patch (compared on all 12136 boundary faces: 0 differing). Risk: a face centre at equal distance from triangles of two regions (tie-break by ESI's octree order), which the brute-force nearest triangle (first minimum) may resolve differently. |

### Round 4, task 3a: result (2026-11-12, `tests/snappy_hex_mesh_esi_scale.rs`)

`esi_snap::repatch_to_surface`, from the castellated patch assignment and ESI's time-36
points: **42 faces repatched, the same 42 faces and the same new patch for each as ESI's
`mergePatchFaces false` run** (reference `esi_repatch_changes.txt.gz`: mesh face index in
the castellated mesh, old patch, new patch; faces matched by owner and ring up to rotation
and orientation across all 63194 faces, every face found). P29 **held**. Patch sizes after:
9336 / 1162 / 238 / 1400 (from 9310 / 1182 / 248 / 1396). Bit-level: no arithmetic beyond the
nearest-point routine already used in P21; the face centre and snap distance are the
earlier ported functions.

### Round 4: not done

`mergePatchFaces` / `mergeEdges` (`meshRefinement::mergePatchFacesUndo`,
`mergeEdgesUndo`: 2118 face sets, 2005 straight-edge points, 63194 to 61034 faces, with the
edge-intersection test and the undo/quality loop) are NOT ported. Read for the next round:
the merge runs only with `mergePatchFaces` true (default); `featureCos` comes from
`resolveFeatureAngle`; ESI's `Undo iteration 0` re-runs the nine checks (now available as
`check_mesh::esi_check_mesh`) on the merged mesh and found 0 faces, so no merge is undone on
the flange. Reference runs for it: this section's case with `mergePatchFaces` true gives the
final mesh (time 40 in the log of the castellated+snap run).

### Round 4 summary of the ledger

| # | outcome |
|---|---|
| P26 | held (nine face sets equal ESI's; 8 faces; 0 on times 32 and 36) |
| P27 | held (scaleMesh: 2 iterations, snapped points bit-identical to time 36, 79986 of 79986) |
| P28 | held (0 faces after scaling) |
| P29 | held (42 faces, same patches) |

## Round 5: `mergePatchFaces` / `mergeEdges` (predictions, committed before the port exists)

**Source read first** (v2506): `snappySnapDriver.C:3640-3660` (after `repatchToSurface`:
`mergePatchFacesUndo(featureCos, featureCos, meshedPatches, motionDict, duplicateFace,
GEOMETRIC)` then `mergeEdgesUndo(featureCos, motionDict)`, `featureCos = cos(degToRad(30))`
= 0.8660254037844387 from `resolveFeatureAngle 30`), `meshRefinementMerge.C`,
`combineFaces.C` (`getMergeSets`, `regioniseFaces`, `faceNeighboursValid`, `validFace`,
`getOutsideFace`, `setRefinement`), `removePoints.C` (`countPointUsage`, `setRefinement`),
`polyTopoChange.C` (`getFaceOrder`: internal faces by (cell, neighbour), boundary faces by
patch then OLD face label; `compact`: points keep their order when `orderPoints` is false;
`changeMesh(mesh, false, true)` so cells and points are not reordered).
Ordering that decides the numbering and that the port must replicate: the master face of
a set is `setFaces[0]`, i.e. the first face of the region in the iteration order of
`Map<label> faceRegion`, an OpenFOAM `HashTable` (power-of-two capacity >= 8, identity hash
`Hash<label>` = the label, insertion at the chain head, growth x2 at load 0.8, iteration by
bucket then chain), and the insertion order of that map follows `cellEdges(celli, set,
storage)` = the iteration order of a `labelHashSet` of capacity 128 (when `cellEdges()` is not
cached). ESI reference run: `snappyHexMeshDict.snap`, `debugFlags (mesh)`, ASCII precision 17;
time 36 snapped, 37 after faces, 38/40 after edges (log: 2118 sets, 2005 points, 63194 to 61034
faces, 26662 to 24615 points, both undo loops find 0 faces in error). Fixtures:
`extract_merge_fixtures.py`, `tests/fixtures/esi_v2506_flange/esi_merge_*.txt.gz`.

| # | prediction (before any merge code exists) |
|---|---|
| P30 | `getMergeSets` on the repatched time-36 mesh (patches from the P29 port, `GEOMETRIC`, `minCos = concaveCos = cos 30 deg`) finds **2118** sets whose faces union to ESI's `allFaceSets` (**4278** faces, same labels). Reason: the rules (same non-constraint patch, normal dot > minCos, fan validity, convexity) are deterministic and ported literally. Fails if face area normals differ at a threshold. |
| P31 | After `setRefinement` + the `polyTopoChange` renumbering, the mesh has **61034 faces**, **26620 points** (42 unused points removed; ESI time 37), patch sizes 7432/1162/238/1144, cells unchanged. |
| P32 | Face numbering equals ESI's: face `i` of the port equals face `i` of ESI's final mesh as a vertex ring up to rotation, **all 61034 faces including orientation, owner, neighbour and patch**, i.e. the master choice (hash iteration order) is reproduced. This is the risky one: if `cellEdges()` is cached in ESI at that point the insertion order differs and some masters (so face labels) differ while the face SET still equals ESI's. |
| P33 | Ring start vertex (rotation) of a merged face equals ESI's for **at least 90 %** of the 2118 merged faces (it follows `edgeLoops()[0]`, which starts at the first boundary edge of the patch built in set-face order); an inequality is a rotation only. |
| P34 | `countPointUsage` at `cos 30 deg` marks **2005** points; after removal the mesh has **24615** points, the surviving points are ESI's (labels `esi_merge_final_points`, coordinates unchanged bit for bit), and every face equals ESI's final face `i` up to rotation. |
| P35 | The undo loops re-run the nine face checks on the merged mesh (`check_mesh::esi_check_mesh`, all faces): **0 faces in error** after the face merge and after the point removal, so nothing is restored (`setUnrefinement` is not needed on the flange and is NOT ported; the port reports the count and stops if it is ever above 0). |

### Round 5: results (2026-11-12, release, `tests/snappy_hex_mesh_esi_merge.rs`)

Code: `src/snappy_hex_mesh/esi_merge.rs` (opt-in; nothing existing calls it). Start: ESI's
time-36 snapped mesh read in, `repatch_to_surface` (P29) and the `polyTopoChange`
renumbering of the repatch, then `merge_patch_faces_undo` and `merge_edges_undo`.

| stage | compared with | result |
|---|---|---|
| merge sets (`getMergeSets`, GEOMETRIC, `cos 30 deg`) | `36/polyMesh/sets/allFaceSets` | **2118 sets, union 4278 faces, equal** |
| `combineFaces::setRefinement` + renumbering | ESI time 37 | 61034 faces, 26620 points (42 unused removed, the same 42 labels); **all 61034 faces equal ESI per face index: same ring, same start vertex, same orientation, same owner and neighbour** |
| undo loop, faces step | `checkMesh` of the merged mesh | 0 faces in error, 1 iteration, nothing restored (ESI: same) |
| `countPointUsage` + `removePoints::setRefinement` | ESI times 38/40 | **2005 points removed, 24615 points left; the surviving points are ESI's by label and bit-identical in coordinates** (ESI's final points equal the time-36 points as exact text, checked in `extract_merge_fixtures.py`) |
| final mesh | ESI time 40 (final) | **61034 faces: all equal per face index** (ring, start vertex, orientation), owner 61034 of 61034, neighbour 51058 of 51058, patch 9976 of 9976; patches start at 51058, 58490, 59652, 59890 (sizes 7432, 1162, 238, 1144) |
| undo loop, edges step | `checkMesh` | 0 faces in error (ESI: same) |

**Numbering IS reachable and equal.** Face, point and owner/neighbour numbering equal
ESI's, not just the sets, because the three things that decide it are reproduced: (1) the
master face of a set is the first face of its region in the iteration order of the OpenFOAM
`Map<label>` (`FoamHashMap`: identity hash, head insertion, growth at 0.8); (2) its
insertion order follows the `labelHashSet` of `cellEdges` (capacity 128), which ESI uses
because `cellEdges()` is not cached at that point; (3) `polyTopoChange::getFaceOrder`
(internal faces by lower cell then higher cell, boundary faces by patch then OLD label),
with points keeping their order. **Negative control**
(`lowest_label_master_is_refuted_by_esi_numbering`): taking the lowest face label as master
instead changes the master of 831 of 2118 sets and **8477 faces** differ from the hash-order
result, so the equality is not an accident of the data. Cells are not renumbered (no cell is
removed; `orderCells` false).

**Prediction ledger.** P30 held (2118, 4278, equal). P31 held. P32 held (all 61034, the
risky prediction). P33 held beyond the prediction: **61034 of 61034** start vertices equal,
not 90 %: the ring start of a merged face follows `edgeLoops()[0]` of the patch built in
set-face order, as read. P34 held. P35 held. No prediction failed; I did not tune anything:
the first run of the port against ESI gave all of the above.

**Not ported (stated):** `combineFaces::setUnrefinement` and `removePoints::setUnrefinement`
(restoring merged faces/points when a face check fails after merging). The loops run the checks
and return `MergeError::UndoNeeded(n)` instead of restoring; ESI restores nothing on the flange
so this path has NO ESI reference and no test beyond the type. Also not done: merging across
patches (`IGNOREPATCH`) is implemented in `get_merge_sets` (largest-face master) but has no ESI
comparison; `preserveFaces`/baffles and face zones are not modelled. No picture this round:
the merged mesh differs from the unmerged one only in faces that are coplanar within `cos 30`
and the equality is by number; a slice would show nothing the counts do not.

**Moved pins:** none. **Defects found:** none in the port this round; one in the record: the
fixture `esi_merge_all_face_sets` holds labels of the REPATCHED mesh (the set is written after
`repatchToSurface`), recorded in `extract_merge_fixtures.py`. PETIR residuals: the only
transcendental is `cos(degToRad(30))` through `RealMath::r_cos`, equal to ESI's printed
0.8660254037844387; count 0 differing.

## Round 6: explicit feature snapping (gh:#1467, epic #1459 / #1336), predictions

**Written and committed BEFORE any feature-snap port code exists.** Upstream read first
(ESI v2506 `snappySnapDriverFeature.C`, 4467 lines; `snappySnapDriver.C::doSnap`
`:2574-3660`; `refinementFeatures.C`; `snapParameters.C`; `plane.C`, `pointConstraintI.H`,
`TensorI.H`, `PatchToolsNormals.C`, `PrimitivePatchMeshData.C`, `face.C`).

**Tool built.** `surfaceFeatureExtract` was not built (round 3 said so). Built with
`wmake -j1 libso extractionMethod && wmake -j1` in
`applications/utilities/surface/surfaceFeatureExtract` under `with-of.sh`: 15 s, no
other library needed, a few MB. Run on `flange.stl` with the tutorial's
`surfaceFeatureExtractDict` (`extractFromSurface`, `includedAngle 150`; copy
`surfaceFeatureExtractDict.flange`): 767 points (4 convex, 4 concave, 10 mixed
feature points, 749 non-feature), 776 edges (488 external, 198 internal, 90 flat),
376 region edges, written as `constant/extendedFeatureEdgeMesh/flange.extendedFeatureEdgeMesh`
(ASCII, `writePrecision 16`; committed gzipped as
`tests/fixtures/esi_v2506_flange/flange.extendedFeatureEdgeMesh.gz`, the **input**
of both ESI and the port, so its rounding is common to both).

**Case.** `snappyHexMeshDict.feature` = the round-3 reduced flange dictionary
(`snappyHexMeshDict.snap`) with exactly two changes: `features ( { file
"flange.extendedFeatureEdgeMesh"; level 0; } )` (level 0: castellation is unchanged,
ESI log "Selected for feature refinement : 0 cells") and `snapControls { nSmoothPatch 3;
tolerance 1.0; nSolveIter 300; nRelaxIter 5; nFeatureSnapIter 10; implicitFeatureSnap
false; explicitFeatureSnap true; multiRegionFeatureSnap false; }`. `multiRegionFeatureSnap`
is false here; the round-3 remark that the tutorial's `true` attracts "24 feature points
and 1446 feature-edge points" is the same as this explicit-only count (identical in
iteration 0). The tutorial's `refineHole` sphere is still not ported. The other
`snapParameters` are upstream's defaults (read from `snapParameters.C`, not assumed):
`releasePoints false`, `stringFeatures true`, `avoidDiagonal false`, `detectBaffles
true`, `baffleFeaturePoints false`, `detectNearSurfacesSnap true`, `strictRegionSnap
false`, `nFaceSplitInterval` unset (`labelMin`, so no face splitting and
`alignMeshEdges` true). Reference run: `blockMesh` with the ASCII `controlDict`
(`writePrecision 16`) first, then `snappyHexMesh` with `writeFormat binary` (exact
doubles; the round-3 note on blockMesh precision applies). The log is
`esi_feature_log.txt.gz`; it equals the ASCII run's log apart from timings.

**What ESI does (the reference numbers).** 18278 cells, 63194 faces, 26662 points
castellated, unchanged by the feature file (so rounds 1-5 carry over). `doSnap` runs
`nFeatIter = 10` morph iterations, each: `calcNearestSurface`, `detectNearSurfaces`,
`calcNearestSurfaceFeature` (blend factor `featureAttract = (iter+1)/10`),
`outwardsDisplacement` (a warning only), `setDisplacement`, `smoothDisplacement`,
`scaleMesh`, `meshMover.correct()`; then `repatchToSurface` and the two merges. Final
mesh (time 81): 18278 cells, **60813 faces, 24311 points** (the plain snap gives
61034 / 24615). Per iteration the log prints (the gates of this round):

| it | wanted displacement mean | raw point / edge | reverse-attract grown (initial) | after reverse point / edge | stringing | final point / edge | gap overrides | scale passes |
|---|---|---|---|---|---|---|---|---|
| 0 | 1.688540950712846e-4 | 24 / 1446 | 4312 (1470) | 18 / 1306 | 101 1 1 2 1 1 0 | 26 / 1405 | 0 | 1 |
| 1 | 1.557839932393523e-4 | 24 / 1446 | 4321 (1470) | 18 / 1300 | 109 4 1 2 1 0 | 26 / 1409 | 0 | 1 |
| 2 | 1.311383484862793e-4 | 28 / 1444 | 4323 (1472) | 18 / 1306 | 102 1 1 2 1 0 | 26 / 1405 | 0 | 1 |
| 3 | 9.992840140840707e-5 | 28 / 1444 | 4324 (1472) | 18 / 1310 | 98 2 0 | 26 / 1402 | 0 | 1 |
| 4 | 6.830408578944219e-5 | 28 / 1420 | 4271 (1448) | 18 / 1310 | 96 1 0 | 26 / 1399 | 0 | 1 |
| 5 | 4.103490302037604e-5 | 28 / 1414 | 4246 (1442) | 18 / 1318 | 88 0 | 24 / 1400 | 0 | 1 |
| 6 | 2.122369759119509e-5 | 26 / 1414 | 4228 (1442) | 18 / 1320 | 93 0 | 27 / 1404 | 0 | 7 |
| 7 | 1.238645002129053e-5 | 26 / 1414 | 4226 (1442) | 18 / 1328 | 85 0 | 27 / 1404 | 0 | 7 |
| 8 | 7.423801224059679e-6 | 24 / 1414 | 4223 (1439) | 18 / 1362 | 51 0 | 29 / 1402 | 0 | 7 |
| 9 | 3.265670138296469e-6 | 25 / 1413 | 4223 (1439) | 18 / 1383 | 28 0 | 28 / 1401 | **2** | 7 |

(Nearest-surface count 10660 and "rest" 0 at the raw stage; ~~0 baffle edges in all
iterations~~ **CORRECTED 2026-11-12 (round 7): 0 in iterations 0-5, then 2, 2, 1, 2**; "scale passes" is the number of `Moving mesh using displacement scaling`
passes, i.e. the `scaleMesh` loop of round 4.) Iteration `k` writes its smoothed mesh
and displacement at times 34, 36, 38, 40, 42, 44, 46, 54, 62, 70 and its last scaled
mesh at 35, 37, 39, 41, 43, 45, 53, 61, 69, 77.

**Inventory of this round (new code in `src/snappy_hex_mesh/esi_feature.rs`, opt-in,
nothing existing calls it):** the `extendedFeatureEdgeMesh` ASCII reader; `plane`
(`planeIntersect`, `planePlaneIntersect` with `Tensor::inv`); `pointConstraint`
(`applyConstraint`); `calcNearestFace`, `calcNearestFacePointProperties`; the per-point
`featureAttractionUsingReconstruction`; `determineFeatures` (single-region and
`multiRegionFeatureSnap` branches); `findNearFeatureEdge/Point`;
`determineBaffleFeatures` (~~detection only: it reports an error if a baffle edge is
found, none is on the flange~~ **CORRECTED 2026-11-12**: the log has 2, 2, 1, 2 baffle edges
in iterations 6-9; the attraction of their end points is ported in round 7); `reverseAttractMeshPoints`; `stringFeatureEdges`;
`preventFaceSqueeze`; `PatchTools::pointNormals`; `smoothAndConstrain`; the tail of
`calcNearestSurfaceFeature`; and a `doSnap` morph loop reusing rounds 3-5. NOT ported:
the mesh-wide `implicitFeatureSnap` driver (its per-point reconstruction is ported because
`determineFeatures` calls it), `releasePointsNextToMultiPatch`, `avoidDiagonalAttraction`,
`splitDiagonals` (face splitting), baffle point attraction, `detectNearSurfaces` (0
overrides in iterations 0-8, **2 in iteration 9**, see P41).

### Predictions (committed before the port code)

| # | prediction | how it can fail |
|---|---|---|
| P36 | The reader gives 767 points, 776 edges, `concaveStart 4 mixedStart 8 nonFeatureStart 18`, 376 region edges, and exact coordinates (no arithmetic). | file layout surprises |
| P37 | **Tie count.** Upstream searches feature edges, feature points and patch points through `indexedOctree::findNearest`; the port brute-forces (first minimum in index order). They differ only when two shapes are at exactly equal distance. The port counts exact-equal-distance ties per query class and reports them. Prediction: ties occur for **feature-edge queries** at shared edge vertices (a sample projecting beyond both edges clamps to the same vertex, so the two distances are bit-equal), a few tens of queries per iteration; **zero** for feature-point and patch-point queries. If a tie changes a later stage the octree traversal order has to be ported; if it does not (both edges give the same point, and `reverseAttract` reads only the points) the brute force stands and the tie count is pinned. | ties are more frequent than guessed, or reach `reverseAttract` through the processing order of `edgeAttractors[feati][edge]` |
| P38 | Iteration 0, started from ESI's time-33 points (`snap_points_33_n3`) and the castellated topology, reproduces **every count of table row 0**: raw 24 / 1446 / 10660 / 0; 0 baffle edges; reverse-attract selection 1470 then 4312; after reverse 18 / 1306 / 0 / 10806; stringing 101 1 1 2 1 1 0; final 26 / 1405 / 0 / 10699, and the `Attraction: linear max` line exactly. Rows 1-8 likewise, started from ESI's own mesh at the start of that iteration. | any divergence in `calcNearestFace` or the plane arithmetic changes which points are attracted; the count table localises it |
| P39 | The smoothed displacement of iteration 0 equals ESI's `34/displacement` **bit for bit on all 79986 components** (patch displacement from `calcNearestSurfaceFeature`, then round 3's `smoothDisplacement`), and the scaled points equal `35/polyMesh/points`. The operations are `+ - * /`, `sqrt`, `min`, in upstream's order; `inv(a)` is `adjunct()/det()`. | the nearest-surface triangle search (round 3: one component differed by 2.2e-19 m), the face-fan normals of `PatchTools::pointNormals`, a summation order in `smoothAndConstrain` |
| P40 | Iterations 1-5 (one scale pass each) are bit-identical to ESI's meshes at times 36/37 ... 44/45 when each is started from ESI's mesh, and chained they stay bit-identical. Iterations 6-8 (7 scale passes) are bit-identical if round 4's `scale_mesh_loop` is. | accumulated 1-ulp differences; iteration 6's later passes test the `errorReduction 0` branch (from pass `nRelaxIter` = 5 on) |
| P41 | Iteration 9 is **not** bit-identical: ESI's `detectNearSurfaces` overrides the nearest displacement at 2 points there and the port does not have it. First divergent operation: the nearest-surface displacement at those two points. | none: an inventory gap stated in advance; porting `detectNearSurfaces` is the fix |
| P42 | After the ten iterations (with ESI's iteration-9 displacement supplied if `detectNearSurfaces` is not ported), `repatchToSurface` + `mergePatchFacesUndo` + `mergeEdgesUndo` give ESI's final mesh: **60813 faces, 24311 points, 18278 cells, all faces equal per index** (rounds 4-5 machinery; only the point positions change, but the merge sets depend on them). | merge thresholds near `cos 30 deg` on features: a merge set could differ by a face |
| P43 | PETIR (`RealMath`) residuals: the transcendentals are `cos(degToRad(30))` (featureCos) and `cos(degToRad(110))` (baffleFeatureCos); both agree with ESI as in round 5; count of differing values 0. | |

Method: each stage is run from ESI's own previous-stage data first (isolating that
stage), then chained; every difference is explained by its first divergent operation;
no tolerance is relaxed and no input is tuned.

### Round 6: results (2026-11-12, release, `tests/snappy_hex_mesh_esi_feature.rs`)

Code: `src/snappy_hex_mesh/esi_feature.rs` (opt-in; nothing existing calls it). Iteration 0
started from ESI's time-33 points and the castellated topology.

| stage | compared with | result |
|---|---|---|
| eMesh reader (P36) | `flange.extendedFeatureEdgeMesh` | **held**: 767 points, 776 edges, starts 4 / 8 / 18, 376 region edges |
| wanted displacement | log line | min and max **exactly** (6.548561578869996e-8, 7.516794439290358e-4), mean to 1e-18 |
| raw analysis, baffles, reverse attraction, stringing, final (P38, single region) | log row 0 | **every count equal**: 24 / 1446 / 10660 / 0; 0 baffle edges; 1470 -> 4312; 18 / 1306; 101 1 1 2 1 1 0; 26 / 1405 / 0 / 10699 |
| `Attraction: linear` (max and average), `feature` max | log | **equal in all printed digits** |
| `Attraction: feature` average | log | equal to ~1e-16 absolute, not in the last digits (summation order of `gAverage` or an ulp in a few attractions; not resolved) |
| same, `multiRegionFeatureSnap true` | `esi_feature_multiregion_log.txt.gz` (new ESI run) | **every count equal**: raw 24 / 1651; 1675 -> 4745; 18 / 1495; 119 1 1 2 1 1 0; 26 / 1612 |
| smoothed displacement (P39) | `34/displacement` | **NOT met: 37365 of 79986 components bit-identical**, worst 1.4e-6 m |
| points after the move | `35/polyMesh/points` | 42862 of 79986 bit-identical, worst 1.4e-6 m, 1 scale pass as ESI |

**P39 failed; first divergence localised, cause not found.** Patch points by final
constraint (identical / all): feature points 26 / 26, **feature edges 1401 / 1405**, free
points 10231 / 10699. The 468 free points are the neighbourhood of the 4 edge points, which
are the four mirror-image valence-3 corner points mesh 6704, 6710, 6738, 6744: their
displacement agrees in z to the bit and differs in x, y by (1.4e-6, 1.2e-6) m, i.e. a shift
along the horizontal feature edge direction (0.605, 0.796, 0), after the 20-pass
`smoothAndConstrain` has spread it to the free points around. So the first divergent
quantity is the feature attraction of an edge-constrained neighbour (mesh 7 or 16888), not
the arithmetic of the smoothing. Variants tried and **refuted** (each measured against the
79986 components; none was tuned to ESI, each is a falsified alternative reading):
brute-force ties resolved by the LAST minimum for triangles (26 / 1405 becomes 30 / 1397,
26647 identical), for feature edges (35171 identical), and per single edge tie (14 toggles:
none improves, two worsen to 35449 / 35455); `preventFaceSqueeze` off (no change, 37365);
`stringFeatures` off (18793); `nbrFeatVec` read from the neighbour instead of the point as
upstream writes it (27763). Remaining candidates, not tested: the octree order for the 190
triangle ties (a different triangle gives the same nearest point but a different normal and
region), the `PrimitivePatch` edge order in `pointEdges` for the summation in
`smoothAndConstrain` (ported from `calcAddressing`, not compared with ESI directly), and
`face::unitNormal` of the patch versus the mesh-level normal. **Next step:** dump ESI's
`patchAttraction` (a `debugFlags (attraction)` run writes `patchAttraction_<iter>.obj`)
and compare it per point, which separates the attraction from the smoothing.

**Tie counts (P37, partly refuted).** Feature-edge ties 14 per iteration-0 run (as
predicted: edges sharing a vertex), feature-point ties 0, patch-point ties 0 (as
predicted); **triangle ties 190**, which P37 did not predict (face centres on a shared
triangle edge give the same nearest point for two triangles with different normals and,
at region boundaries, different regions). The first-minimum rule reproduces all log counts;
the octree order is not ported and is the main open suspect above.

**Not done this round:** iterations 1-9, the chained run, P40-P42 (final mesh 60813 faces),
`detectNearSurfaces` (P41), because P39 is open and everything chained depends on it.
**Moved numbers:** none. **Defects found in the existing port:** none confirmed (the
existing `calc_nearest_surface` compares `mag_sqr`, upstream compares `sqr(mag)`; the new
`calc_nearest_surface_sqr_mag` does the latter; whether it removes round 3's one 2.2e-19 m
component was not re-measured). PETIR: no transcendental in the new code (callers pass
`featureCos`); count of differing values 0.

**Ledger.** P36 held. P37 partly refuted (190 triangle ties). P38 held (counts, both
single-region and multi-region). P39 **failed** (37365 / 79986). P40-P43 untested.

---

# Round 7 (2026-11-12): the first divergence of round 6, found and fixed

## Evidence gathered BEFORE the port change (disclosed, not predictions)

1. **ESI reference rebuilt** (the round-6 run directory was gone): `blockMesh` (ASCII,
   `writePrecision 16`) then `snappyHexMesh` (binary) on `snappyHexMeshDict.feature`
   reproduces `esi_feature_log.txt.gz` line for line (apart from the case path), 9 s.
2. **A dump patch in `snappySnapDriverFeature.C`** (upstream edited in place, then restored
   and the library rebuilt from the restored source; the original `libsnappyHexMesh.so` and
   `.C` were backed up first, and the rebuilt library has the same size, 5027088 bytes).
   `calcNearestSurfaceFeature` wrote, per iteration, raw doubles of `ppLocalPoints`,
   `nearestDisp`, the final `patchAttraction`, `patchConstraints` (count and direction),
   and after smoothing `smoothedPatchDisp`, `tangPatchDisp`, `pointNormals`. The patched
   run gave `34/displacement` and `77/polyMesh/points` byte-identical to the unpatched run.
3. **Port vs the dump, iteration 0, per point (12130 patch points):** patch points 36390 of
   36390 components bit-identical; **constraints differ at exactly 4 points** and
   **attraction at exactly the same 4** (mesh points 6704, 6710, 6738, 6744). ESI's
   constraint direction there is (+-0.80286, +-0.59617, 0), the port's (+-0.60532, +-0.79598, 0):
   the points were attracted along a *different feature edge*, both edges at the same
   distance from the sample, ending at the same feature vertex (a tie; `TieCounts.edge` is
   14 per run). The z attraction agrees to the bit (the shared vertex), which is why round 6
   saw only an along-edge shift. So the divergence starts in the attraction, not in the
   smoothing.
4. Round 6's "single edge-tie toggles refuted" tested the wrong thing: a toggle flips the
   rule for one tie, not the per-query traversal order of upstream's octree.

**Cause (hypothesis, to be confirmed by the run below):** `refinementFeatures::findNearestEdge`
calls `indexedOctree<treeDataEdge>::findNearest`; among equidistant edges the first one the
traversal reaches wins (`distSqr < nearestDistSqr`). The port searched in index order.

## Port change (written after the evidence above, before the stage runs below)

`src/snappy_hex_mesh/esi_octree.rs` (new, opt-in): `indexedOctree` build and `findNearest`,
`treeBoundBox` (`subBbox`, `searchOrder`, sphere and box `subOverlaps`, segment `intersects`,
the separating-axis triangle test), `Random` (`Rand48` + libstdc++ `generate_canonical`)
for `inflate`, `treeDataEdge`, `treeDataTriSurface`. `FeatureSet::new` builds the edge and
region-edge trees as `refinementFeatures` does (`Random(65431)`, `inflate(rndGen, 1e-4,
ROOTVSMALL)`, `maxLevel 8`, leaf 10, duplicity 3); `FeatureSurface` builds
`triSurfaceSearch::tree()` (`maxTreeDepth 10`). The brute-force searches stay as `*_brute`
twins so tests can count where the two differ.

## PREDICTIONS (committed before the stage runs)

| # | prediction | how it can fail |
|---|---|---|
| P44 | With the octree, iteration 0 (from ESI's time-33 points) has **all 12130 constraints and attractions bit-identical** to the dump, and `34/displacement` is **79986 of 79986** bit-identical. | the 4 points were not the only divergence, or smoothing has its own one |
| P45 | The octree and the brute force differ in **at least 4** edge queries and in **0 or few** triangle queries of iteration 0 (190 triangle ties; a different triangle gives the same point). | the tie classes behave the other way |
| P46 | Iterations 1-8, each started from ESI's mesh at the start of that iteration (`feat_points_*`), are bit-identical in displacement and scaled points; chained they stay so. Iterations 6-8 pass through 7 scale passes (round 4 machinery). | accumulated ulp, or `scaleMesh` on the `errorReduction 0` branch |
| P47 | Iteration 9 is NOT bit-identical without `detectNearSurfaces` (2 overrides); with it ported, it is. | |
| P48 | After ten iterations (+ `detectNearSurfaces`), `repatchToSurface` + merges give ESI's final mesh, 60813 faces, 24311 points, all faces equal per index. | merge sets near `cos 30 deg` |

## Round 7: results (2026-11-12, release, `tests/snappy_hex_mesh_esi_feature.rs`)

Reference: the rebuilt ESI run (binary, `blockMesh` ASCII `writePrecision 16`), identical in
log to round 6's; exact doubles per iteration as in `extract_feature_fixtures.py`. The
per-iteration meshes are 640 KB each and are not committed: the test pins the **64-bit FNV-1a
digest of ESI's binary file** (`ESI_DIGESTS`), so a digest match is equality of all 79986
components. Final-mesh fixtures (`esi_feat_final_faces/points/boundary`, `esi_feat_all_face_sets`)
come from a second ESI run of the same case with ASCII `writePrecision 17`, through
`extract_merge_fixtures.py` run on a directory whose `36`, `37`, `40` are symlinks to ESI times
77, 78, 81.

| stage | compared with | result |
|---|---|---|
| per-point attraction, iteration 0 (P44) | ESI dump, 12130 patch points | **0 differing** constraints and attractions (were 4, mesh 6704, 6710, 6738, 6744) |
| smoothed displacement and scaled points, iteration 0 (P44) | `34/displacement`, `35/polyMesh/points` | **79986 of 79986 each** (was 37365 / 42862) |
| iterations 1-9, each from ESI's mesh and ESI's previous displacement (P46) | `N/displacement`, scaled points | **79986 of 79986, both, all nine** |
| iterations 0-9 **chained** from the port's own state (P46) | digests of ESI's files | **identical, ten of ten, both fields** |
| log counts per iteration | table above | all equal (raw, reverse, stringing, final), scale passes 1,1,1,1,1,1,7,7,7,7 |
| `repatchToSurface` | ESI log "Repatched 26 faces" | 26 faces |
| merge sets | `allFaceSets` | 4666 faces equal (2285 sets) |
| final mesh | ESI time 81 | **60813 faces, 24311 points, 18278 cells; all 60813 faces equal per index, ring and start vertex; owner 60813/60813; neighbour 51058/51058; patch 9755/9755**; patch starts 51058 / 58275 / 59449 / 59689 / 60813; surviving points are ESI's, coordinates bit-identical |

### What the dump found, in order (each fix measured before the next)

1. **Edge search order** (the round-6 miss): octree traversal. After porting it, iteration 0
   attraction and constraint differences: 4 -> 0; displacement 37365 -> 79986.
2. **Iteration 1 onward: the displacement field persists in `meshMover`.** With iteration 0
   exact, chained iterations still differed by up to 1.1e-11 m (37868 of 79986 components
   bit-identical in iteration 1) although `nearestDisp`, the constraints, the attraction and the
   patch displacement were all bit-identical to the dump. First divergent operation: the
   interior points. `motionSmootherAlgo::setDisplacement` writes only the patch points, so the
   interior starts the next 300 `smoothDisplacement` passes from the previous iteration's smoothed
   values, not zero (the port zeroed it; 300 passes leave ~1e-11 of the start). Fixed by
   `FeatureSnap::morph_iteration_from(.., prev)`; iterations 0-5: 79986 of 79986.
3. **Baffle edges.** Iteration 6 stopped with the round-6 `NotPorted` error because the port
   detected 2 baffle edges. **Round 6's record "0 baffle edges in all iterations" was wrong**:
   ESI's log has them in iterations 6-9. ~~0 baffle edges in all iterations~~ **CORRECTED
   2026-11-12**: ESI detects 2, 2, 1, 2 baffle edges (4, 4, 2, 3 baffle points, all attracted to a
   feature edge, 0 to a feature point) in iterations 6, 7, 8, 9, and "After baffle feature
   analysis" reads 26 / 1416, 26 / 1416, 24 / 1415, 25 / 1414 (feature point / feature edge); the
   port now reproduces all of these. `determineBaffleFeatures` with `baffleFeaturePoints false`
   is a loop over baffle-edge end points calling `findNearFeatureEdge` in point order (the
   feature-point loop and the `pointStatus == 1` branch are commented out or unreachable
   upstream), ported as `determine_baffle_features`. Iterations 6-9: 79986 of 79986.

### P41 refuted in effect; what `detectNearSurfaces` does (not ported)

ESI overrides the nearest displacement at 2 points in iteration 9 (log: "Overriding nearest
with intersection of close gaps at 2 out of 12130 points"; mesh points 19516 and 25248; the
port's nearest differs from ESI's there by up to 3.5e-8 m, measured against the dump). The
iteration is nevertheless bit-identical without it: at `featureAttract = 1` the displacement
of a feature-constrained point is its attraction, `(1-1)*nearest + 1*attraction`, and both
points are feature-constrained (constraints and attractions equal the dump at every point).
So the first divergent operation is the one predicted in P41, but it has no effect on the
result of this case. The port is therefore **not shown to be correct for a case whose overridden
points are free (nearest-surface) points**; `detectNearSurfaces` is still to be ported before the
feature snap is claimed for other geometry.

### Octree vs brute force (P45), measured

Triangle search at the 12130 patch points of iteration 0: the two searches return a different
triangle at **8** points and the same nearest point at **all** of them (equidistant
triangles). Round 6's "190 triangle ties" counted face-centre queries as well, which are not
in this comparison. Feature-edge search: the four corner points above. 410 s -> 0.2 s for the
per-point stage, since the nearest-triangle search is now O(log n).

### Moved numbers (old -> new)

| quantity | round 6 | round 7 |
|---|---|---|
| `34/displacement` bit-identical components | 37365 / 79986 (worst 1.4e-6 m) | **79986 / 79986** |
| `35/polyMesh/points` bit-identical | 42862 / 79986 | **79986 / 79986** |
| patch points, feature-edge class bit-identical | 1401 / 1405 | 1405 / 1405 |
| patch points, free class bit-identical | 10231 / 10699 | 10699 / 10699 |
| iterations 1-9 | not run | all bit-identical (single and chained) |
| baffle edges, iterations 6-9 | "0" (wrong) | 2, 2, 1, 2 (ESI's) |
| final mesh | not run | 60813 faces / 24311 points, all equal per index |

### PETIR (P43)

The only transcendentals are `cos(degToRad(30))` and `cos(degToRad(110))`, both through
`RealMath` (`r_cos`) and equal to ESI's as in round 5; **0 differing values**. The octree code
uses only `+ - * /`, `sqrt`, `min`, `max`; `Random` is integer arithmetic plus one
`u64 -> f64` conversion and a division, no libm.

### Ledger, round 7

P44 held. P45 held (differences exist, all nearest points equal; count 8, not the predicted
"0 or few" of triangle index differences only in the sense of "few"). P46 held (single and
chained, after the two further fixes above, neither of which was predicted: the persistent
displacement field and the baffle attraction). P47 refuted in effect (iteration 9 is
bit-identical without `detectNearSurfaces`; the override exists and is inert here). P48
held. P43 held.

### Honest limits

- The match is for this one case (reduced flange, one surface, serial, `multiRegionFeatureSnap
  false`). The multi-region counts are pinned for iteration 0 only (not chained).
- `detectNearSurfaces`, `baffleFeaturePoints true`, `implicitFeatureSnap`, face splitting and
  the `setUnrefinement` undo paths are not ported.
- The per-iteration references are pinned by a 64-bit digest, not stored; the iteration-0
  and scaled-iteration-0 meshes are stored (`feat_disp_34`, `feat_points_35`).
- ~~`addLayers` was not started.~~ **CORRECTED 2026-11-12:** round 8 starts it (below).
- The ESI dump patch (a copy of `snappySnapDriverFeature.C` edited in place, original and
  rebuilt library restored) is not committed; its layout is described above.

# Round 8 (2026-11-12): `addLayers`, ESI v2506 `snappyLayerDriver`

**Inventory first (search-before-building).** The workspace holds two layer codes and neither is
the ESI one: `src/snappy_hex_mesh/layers.rs` (an own interior shrink-and-insert with a global
layer-count fallback; its header says so) and `outram_park_fork_cfmesh::snappy_layers` (a
per-point port of the *Foundation*-style `extrudeMode` loop for cfMesh). Neither has
`layerParameters`, `medialAxisMeshMover`, `addPatchCellLayer` or ESI's `checkAndUnmark` loop;
`grep` for `addPatchCellLayer`, `medialAxis`, `layerParameters` over the workspace finds only
doc comments and the Foundation-style paths, which stay callable and are not touched.

## ESI reference run

Case `snappyHexMeshDict.layers`: the round-7 flange dictionary (explicit feature snap, 10
iterations) plus `addLayers true` with `layers { "flange_.*" { nSurfaceLayers 1; } }`,
`expansionRatio 1.0`, `finalLayerThickness 0.3`, `minThickness 0.25`, `nGrow 0`, `featureAngle
30`, and the tutorial's defaults for everything else (`nRelaxIter 5`, `nSmoothSurfaceNormals 1`,
`nSmoothNormals 3`, `nSmoothThickness 10`, `maxFaceThicknessRatio 0.5`,
`maxThicknessToMedialRatio 0.3`, `minMedialAxisAngle 90`, `nBufferCellsNoExtrude 0`,
`nLayerIter 50`, `nRelaxedIter 20`). The flange is one STL; ESI splits it into four region
patches `flange_patch1..4`, so "one surface" means four patches here, all matched by the regex.
Run: `blockMesh` (ASCII, `writePrecision 16`), `surfaceFeatureExtract`
(`surfaceFeatureExtractDict.flange`), `snappyHexMesh -noFunctionObjects` (binary), 19 s. The
snapped mesh (time 81) is the round-7 final mesh (60813 faces, checked against
`esi_feat_final_faces`, 0 differing). `debugFlags (mesh)` writes: 82/83 merged faces/edges (before
layers), 84 baffled, then per layer iteration the shrunk mesh (85, 87, 89) and the layer mesh (86,
88, 90); 91 final (26920 cells, 85399 faces, 31644 points). Log numbers: 1269 feature edges;
"Layers to add in current iteration : 8521"; three layer iterations (17, 2, 0 faces unextruded by
`checkAndUnmark`; 8724, 8658, 8642 of 9747 faces extruded); medial-axis scaling retries 6, 6, 0 in
the three iterations (displacement scaling down to 0 in iteration 0).

## PREDICTIONS (committed before any round-8 port code exists)

Written from reading `snappyLayerDriver.C` `addLayers` / `setPointNumLayers` /
`calculateLayerThickness`, `layerParameters.C` and `snappyLayerDriver::doLayers` (v2506); the
only ESI results used are the numbers quoted in the paragraph above.

| # | prediction | how it can fail |
|---|---|---|
| P49 | `mergePatchFacesUndo` + `mergeEdgesUndo` of `doLayers` on the round-7 final mesh (the existing `esi_merge`, same `cos 30 deg`) find **6 face sets and remove 4 points**, giving **60805 faces, 24305 points**, all faces equal per index to ESI time 83 (ring, start vertex, owner, neighbour, patch); time 84 equals 83. | the merge's master/hash order only held for the 2118-set case |
| P50 | `layerParameters` selects `FINAL_AND_EXPANSION`; per point `thickness = finalLayerThickness * edgeLen * nLayers` with `edgeLen = edge0 / 2^maxPointLevel` (level 2: `0.06/20/4 = 7.5e-4`), so **near-wall thickness 2.25e-4 m on all four patches** (the ESI table) and `overall` = near-wall times the fraction of points with `nLayers 1` (points with `nLayers 0` contribute 0). | the table averages by master point; a point on two patches takes the min |
| P51 | `setNumLayers` + `handleNonStringConnected` + `handleNonManifolds` + `handleFeatureAngle(30)` + `handleWarpedFaces` leave **exactly 8521 pp points with `EXTRUDE`** (ESI: "Layers to add 8521" = sum of `deltaNLayers`), **1269 feature edges**, 0 non-manifold points, 0 warped faces, and the four per-patch `overall` averages of the ESI table (1.89e-4, 1.72e-4, 1.84e-4, 1.98e-4 to 3 s.f.). | a one-point difference would show in the 3rd figure of the average |

## Round 8, stage 1: results (2026-11-12, release, `tests/snappy_hex_mesh_esi_layers.rs`)

Code: `src/snappy_hex_mesh/esi_layers.rs` (new, opt-in, serial, no faceZones): `layerParameters`
(all six thickness models, `calculateLayerParameters`, `layerThickness` slices), the layer patch
(`makePatch`), `setNumLayers`, `handleNonStringConnected`, `handleNonManifolds`,
`handleFeatureAngle`, `handleWarpedFaces`, `growNoExtrusion`, `calculateLayerThickness`.
Start: ESI's own time-81 snapped mesh (`layer_points_81.bin.gz` exact doubles, faces equal to
`esi_feat_final_faces`).

| stage | compared with | result |
|---|---|---|
| `mergePatchFacesUndo` + `mergeEdgesUndo` of `doLayers` | ESI time 83 | **6 sets, 4 points removed, 60805 faces, 24305 points; ring (start vertex included) 60805/60805, owner 60805/60805, neighbour 51058/51058, patch 9747/9747**; surviving points bit-identical to time 81's |
| `setPointNumLayers` | log | `pp` 9747 faces, 9773 points; **1269 feature edges, 0 non-manifold points, 0 warped faces, 8521 points `EXTRUDE`** (= "Layers to add 8521"), 9747 ideal added cells (= the "out of 9747" of the log) |
| `calculateLayerThickness` table | log, `setprecision(3)` | near-wall 2.25e-4 on all four patches; overall **1.89e-4, 1.72e-4, 1.84e-4, 1.98e-4**; faces 7209, 1174, 240, 1124: all equal |

**Ledger.** P49 held in outcome, but **the prediction text was partly wrong**: it said the merge
used "the same `cos 30 deg`". `snappyLayerDriver::mergePatchFacesUndo` passes
`concaveCos = cos(degToRad(concaveAngle))` with `concaveAngle` defaulting to **90**, while the snap
driver's call uses `featureCos` for both. With 30 degrees as concave limit the port found **0 sets**
(first run, kept as the negative control); with 90 degrees it found ESI's 6. P50 held. P51 held.
Nothing was tuned: the first run with the corrected call gave all of the above, and the 3-figure
averages first failed only because my test formatted four significant figures.

**Not verified at exact-double level.** The per-point thickness and `minThickness` arrays are
checked through the three-figure table and the integer counts, not bit for bit (ESI does not
write them). `PrimitivePatch` edge order is first-appearance (not ESI's), unused so far.
PETIR: `pow` goes through `r_powf` (only called for `expansionRatio != 1`, which this case never
does) and `cos(degToRad(30))`, `cos(degToRad(90))` through `r_cos`; 0 differing values (the
merge reproduces ESI's sets with both).

## Round 8, stage 2: `medialAxisMeshMover`, iteration 0 (predictions, committed before the port exists)

Written from reading `medialAxisMeshMover.C` (`update`, `isMaxEdge`, `calculateDisplacement`,
`findIsolatedRegions`, `handleFeatureAngleLayerTerminations`, `shrinkMesh`, `move`),
`fieldSmoother.C` (`smoothPatchNormals`, `smoothNormals`, `minSmoothField`),
`meshRefinement::calculateEdgeWeights/weightedSum`, `PatchTools::pointNormals`,
`PrimitivePatch::calcAddressing`, `PointData`/`pointEdgePoint`, `motionSmootherAlgo::scaleMesh`
and `snappyLayerDriver::getPatchDisplacement`/`syncPatchDisplacement`/`makeLayerDisplacementField`.
ESI numbers available from the log (printed to 16 digits): normal-smoothing residual of the patch
normals `0.07867287997577262`, of the interior normals `0.1764300260321269`, of the thickness
smoothing `9.162494450008907e-06` (all "Iteration 0"); 20 nodes with reduced thickness; 975
isolated points; scaling iterations 0-6 with faces in error 8, 8, 8, 6, 6, 4, 0 and scaling
min 1, 0.75, 0.5625, 0.421875, 0.31640625, 0.2373046875, 0. Exact doubles: the shrunk mesh of
iteration 0 is `85/polyMesh/points` (binary).

| # | prediction | how it can fail |
|---|---|---|
| P52 | The three printed residuals equal ESI's to all 16 digits (same edge order from `calcAddressing`, same accumulation, same `calculateEdgeWeights`). | a different summation order of `weightedSum` (patch edges are in `calcAddressing` order, mesh edges in `calcEdges` order); `gAverage` sums in point order |
| P53 | Counts equal the log: 20 reduced-thickness nodes, 975 isolated points, faces in error 8, 8, 8, 6, 6, 4, 0 over scaling iterations 0-6 and the scale minima above (the scale schedule is `0.75^k`, then 0 at iteration 5 as `nRelaxIter 5`). | the three PointEdgeWaves choose different origins at ties |
| P54 | The shrunk points of iteration 0 equal `85/polyMesh/points` in **all 24305 points, bit for bit**. | long chain (3 waves, 3 smoothers, 7 scalings); expect at best a few ulp; if not equal, the first divergent stage is found with a dump patch of `medialAxisMeshMover` (original source and library restored, size/hash verified) |

## Round 8, stage 2: results (2026-11-12, release, `tests/snappy_hex_mesh_esi_layers.rs`)

Code: `src/snappy_hex_mesh/esi_medial.rs` (new, opt-in): `getPatchDisplacement` +
`syncPatchDisplacement`, `PatchTools::pointNormals`, a generic `PointEdgeWave` with `PointData`,
`fieldSmoother` (`smoothPatchNormals`, `smoothNormals`, `minSmoothField`),
`medialAxisMeshMover::update`, `calculateDisplacement` (thickness truncation,
`findIsolatedRegions`, `handleFeatureAngleLayerTerminations`), and `move`/`shrinkMesh` on the
existing `esi_snap` `scaleMesh` machinery. `esi_layers::LayerPatch` now follows
`PrimitivePatch::calcAddressing` exactly (edges, edgeFaces, faceEdges, pointEdges).

| quantity (iteration 0) | ESI | port |
|---|---|---|
| patch-normal smoothing residual | 0.07867287997577262 | 0.07867287997577262 |
| interior-normal smoothing residual | 0.1764300260321269 | 0.1764300260321269 |
| thickness-smoothing residual | 9.162494450008907e-06 | 9.162494450008907e-06 (16 digits) |
| nodes with reduced thickness / isolated points | 20 / 975 | 20 / 975 |
| scaling iterations: scale min, faces in error | 1,.75,.5625,.421875,.31640625,.2373046875,0; 8,8,8,6,6,4,0 | equal |
| **shrunk points (`85/polyMesh/points`), all 24305 points** | | **72915 of 72915 components bit-identical** |

**Ledger.** P52, P53 and P54 held, but **not on the first run**, and the two causes are results:

1. **First run: 71591 of 72915 components identical, 1303 points differing, worst 3.5e-18 m (1-2
   ulp).** The ESI dump patch (below) localised the first divergent stage: `medialDist` /
   `medialVec` (1507 / 2617 values), upstream of everything else (`dispVec`, the three wave
   results at the patch, were identical). Cause 1: **`esi_snap::mesh_edges` oriented every edge
   in creation order; ESI's `getEdge` (`primitiveMeshEdges.C:40`) creates `(lower, higher)`
   point**, so `edge.vec()` and the `medialAxisPt = p0 + (s-dist0) eVec` rounded differently.
   The doc comment claiming creation order is struck through and corrected in `esi_snap.rs`; the
   snap tests (all 15 across `snappy_hex_mesh_esi_snap/feature/scale/merge`) still pass, as
   expected since the snap only uses edges symmetrically. After this fix medial fields were
   bit-identical but the patch displacement still differed by 1 ulp (16470 of 29319 values).
   Cause 2: **`level0Edge` printed by ESI (0.002999999999999978) has 16 digits and is not the
   double**; the double is `sqrt(min edge magSqr)` of the blockMesh points as `snappyHexMesh`
   reads them (16-digit ASCII), **0.0029999999999999784**, derived in
   `derive_level0_edge.py` from the case's `constant/polyMesh/points`, not tuned. With it all
   per-stage dumps are identical.
2. Dump patch: `medialAxisMeshMover.C` edited in place to append binary doubles of `medialRatio_`,
   `medialDist_`, `medialVec_`, `dispVec_`, the smoothed thickness, `patchDisp` after the
   isolated-region step and the displacement (the run with it gave the same `91/polyMesh/points`
   as the unpatched run, `cmp`); the original `.C` (sha256 `49384a3b...`) and
   `libsnappyHexMesh.so` (sha256 `092cd572...`, 5027088 bytes) were backed up first and
   **restored and verified by hash** (rebuilt library hash equals the original). The dumps are
   not committed; the env-gated test `medial_stages_against_esi_dump` (`LAYER_DUMP_DIR`) is the
   comparison; the end-to-end pin is `medial_axis_first_iteration_against_esi` against the
   committed `layer_points_85.bin.gz`.

PETIR: `cos(degToRad(90))`, `cos(degToRad(15))`, `cos(degToRad(30))` via `r_cos`; the rest is
`+ - * /` and `sqrt`; 0 differing values.

**Not done in stage 2.** Iterations 1 and 2 of the layer loop (they need `truncateDisplacement`,
`addPatchCellLayer` and `checkAndUnmark` first); `nSmoothDisplacement > 0`; non-adapt patches
with points (slip / fixedValue); `detectExtrusionIsland true` is implemented but has no ESI
reference.

## Round 8: what remains (not started, stated)

**CORRECTED 2026-11-12 (round 9):** the first four bullets below are ~~not started~~ **done**
(`truncateDisplacement`, `setupLayerInfoTruncation`, `addPatchCellLayer::setRefinement` with `polyTopoChange`
numbering, `checkAndUnmark`, the loop, medial iterations 1-2, final mesh 91 and the `printLayerData` table):
see "Round 9: results". Still open: multi-layer runs, `nOuterIter != 1`, faceZones, non-adapt patch seeds.

- `truncateDisplacement` (+ `getVertexString`, `sameEdgeNeighbour`): 0 faces unextruded in
  iteration 0 of the ESI log, 2 in iteration 1; not ported.
- `setupLayerInfoTruncation` and **`addPatchCellLayer::setRefinement`** (3517 lines upstream) with
  `polyTopoChange::makeMesh` numbering: the reference is `86/polyMesh` (31742 points, 85701 faces),
  `88`, `90`; the layer mesh is NOT built by the port. The existing `layers.rs` and
  `outram_park_fork_cfmesh::snappy_layers` are Foundation-style and are not ESI's topology.
- `checkAndUnmark` and the outer loop (ESI: 17, 2, 0 faces unextruded over three iterations,
  8642 of 9747 faces extruded at the end; `getLayerCellsFaces`, `printLayerData` table), `growNoExtrusion` between iterations (nGrow 0
  here), the final mesh `91` (26920 cells, 85399 faces, 31644 points).
- Layer iterations 1 and 2 of the medial mover (they start from `oldPoints`, so the mover itself needs
  no new code beyond the new `extrudeStatus`).
- Multi-layer (expansion ratio != 1) runs, `nOuterIter != 1`, faceZones, non-adapt patch seeds.
- `docs/outram-foam-mesh-api.md` is not regenerated (no `kovan-cli` in this environment).

# Round 9 (2026-11-12): `truncateDisplacement`, `addPatchCellLayer::setRefinement`, the layer loop

**Inventory first (search-before-building).** Re-grepped `polyTopoChange`, `topo_change`,
`renumber`, `addPatchCellLayer` over the workspace including cfmesh. Present: `esi_merge::change_mesh`
(removals and modifications only: assumes `owner < neighbour`, no added points, faces or cells) and
`poly_dual_mesh_upstream` (meshDualiser, its own builder, adds cells and faces but not through a
reusable change object). Neither contains `addPatchCellLayer` (cfmesh's and `layers.rs` are
Foundation-style, not ESI's, as in round 8). So the layer-topology builder is new, written beside
`esi_merge::change_mesh` and following `polyTopoChange::getFaceOrder` (cells in order, faces of a
cell sorted by neighbour, then patch faces by patch and `polyTopoChange` label).

**ESI side, re-run this round** (the round-8 run directory was not kept): `snappyHexMeshDict.layers`,
same blockMesh, 18 s. Counts reproduce round 8: three layer iterations, 17 / 2 / 0 faces removed,
final mesh 91: 31644 points, 85399 faces. **Pre-run observation, disclosed (not a prediction):**
the first 24305 points of `86/polyMesh/points` are bit-identical to `85/polyMesh/points` (the
shrunk mesh), i.e. `addPatchCellLayer` keeps the old point labels and appends the new points.
The extra detail in the log: iteration 0 `Added 8743 out of 9747 cells`, 8724 faces extruded.

## PREDICTIONS (committed before the round-9 port code exists)

Written from reading `snappyLayerDriver::truncateDisplacement`, `getVertexString`,
`setupLayerInfoTruncation`, `checkAndUnmark`, `getLayerCellsFaces`, `addLayers` (the loop),
`addPatchCellLayer::setRefinement` (+ `getEdgeString`, `addSideFace`, `findDuplicatedPoints`) and
`polyTopoChange::getFaceOrder`, v2506.

| # | prediction | how it can fail |
|---|---|---|
| P55 | `truncateDisplacement` after the iteration-0 medial move unmarks **0 and 0** faces (ESI log), leaving 7437 points (31742 - 24305) with `nPatchPointLayers = 1`, and `setupLayerInfoTruncation` (`nBufferCellsNoExtrude 0`) gives **8743 faces with `nPatchFaceLayers = 1`** (ESI: "Writing 8743 added cells"). | the face-layer rule for faces with a non-extruded point (`maxLevel`) |
| P56 | `addPatchCellLayer::setRefinement` + `polyTopoChange` numbering reproduces `86/polyMesh`: **31742 points** (first 24305 unchanged, new points bit-identical), **85701 faces**, old cells + 8743 added; **all faces equal per index** (ring including start vertex, owner, neighbour, patch) on the first run. | the ring start of the side faces (edge strings), the neighbour tie order of `getFaceOrder`, the order of `addSideFace` calls |
| P57 | `checkAndUnmark` on layer mesh 86 finds **2 non-orthogonal and 15 tet-quality faces, 17 illegal faces**, and unmarks the extrusion at **17 pp faces** (8724 extruded). | the checks are those of round 4, on a mesh with new boundary faces |
| P58 | Layer iterations 1 and 2: the shrunk meshes `87`, `89` equal ESI bit for bit (24305 points), `truncateDisplacement` iteration 1 prints 2, 0 then 0, 0; layer meshes `88`, `90` equal per index; extruded faces 8658 and 8642. | the mover's persistent state between iterations (displacement field), `unmarkExtrusion` of faces, `growNoExtrusion` |
| P59 | The final mesh (91, 31644 points, 85399 faces) equals `layer_mesh_91` per index and the `printLayerData` table equals ESI's. | `doLayers` does more after the loop than the outer loop shows |

## Round 9: results (2026-11-12, release, `tests/snappy_hex_mesh_esi_layers.rs::layer_loop_against_esi`)

Code (all new, opt-in, serial, no face zones, `nOuterIter 1`): `src/snappy_hex_mesh/esi_layer_topo.rs`
(`truncate_displacement` with `get_vertex_string`, `setup_layer_info_truncation`, `count_extrusion`,
`add_patch_cell_layer` = `addPatchCellLayer::setRefinement` + `findDuplicatedPoints` + `getEdgeString` +
`addSideFace` + `polyTopoChange` `makeCells`/`getFaceOrder` numbering, `determine_side_patches`,
`get_layer_cells_faces`, `check_and_unmark`) and `src/snappy_hex_mesh/esi_layer_loop.rs` (the `addLayers`
iteration loop and the `printLayerData` table). The references are a fresh ESI run of the same case (the
round-8 run directory was gone); fixtures `layer_mesh_{86,88,90,91}.txt.gz`, `layer_points_{86..90}.bin.gz`.

| quantity | ESI v2506 | port |
|---|---|---|
| `truncateDisplacement` passes, iterations 0 / 1 / 2 | (0,0) / (2,0),(0,0) / (0,0) | equal |
| iteration 0: faces with layers / points with layers | 8743 (log "8743 added cells") / 7437 (31742-24305) | 8743 / 7437 |
| iteration 0: layer mesh 86 | 31742 points, 85701 faces | points **31742 / 31742 bit-identical**; ring, owner **85701 / 85701**; neighbour **75954 / 75954**; patch **9747 / 9747** |
| `checkAndUnmark` iteration 0: non-orthogonal / tet quality / removed faces / extruded | 2 / 15 / 17 / 8724 | 2 / 15 / 17 / 8724 |
| iteration 1: shrunk mesh 87, layer mesh 88 (31658 points, 85449 faces); removed / extruded / added cells | 2 / 8658 / 8660 | 87: 24305 / 24305 points bit-identical; 88: points, ring, owner, neighbour, patch all equal; 2 / 8658 / 8660 |
| iteration 2: shrunk 89, layer mesh 90 (31644 points, 85399 faces); removed / extruded / added | 0 / 8642 / 8642 | equal in every item, loop ends by itself |
| final mesh 91 (26920 cells, 85399 faces, 31644 points) | ESI drops the empty `allBoundary` | every face ring, owner, neighbour, patch equal; points bit-identical |
| `printLayerData` table (faces, mean layers, thickness [m], [%]) | 7209 0.874 0.000155 78.8; 1174 0.983 0.000178 92.2; 240 0.933 0.000162 82.3; 1124 0.859 0.000173 81.7 | equal to 3 figures in all 16 entries |

**Ledger.** P55, P56, P57, P58 held on the first run of the finished code. P59 held for the mesh and the
table. Nothing was tuned. **What the prediction text got wrong (found by reading, before the first run):**
I had assumed `addPatchCellLayer(mesh)` has `extrude = false`; the constructor default is
**`extrude = true`** (`addPatchCellLayer.H:318`). With it the original patch face becomes the internal
face between its old owner and layer 0, the last layer face is the new boundary face (patch of the old
face, owner = the outermost layer cell), the old cells are NOT re-pointed (`findDuplicatedPoints` only
affects the neighbour cell of internal patch faces) and side faces are not reversed. The first unit test
(written with the wrong assumption) failed on exactly this, then the code was corrected from the source
before any ESI comparison. A second small defect, `n_cells_of` took only the owner maximum (the last cell can own
no face), was found by the first run (out-of-bounds panic), not by the comparison.
The persistent-state worry of P58 did not materialise: the mover is re-run from the original points each
iteration with the new extrude status and the shrunk meshes of iterations 1 and 2 are bit-identical.

**Not verified / limits.** One case; `nOuterIter 1`; one layer per point (`offset` branches of
`getEdgeString`/`addSideFace` for unequal layer counts are translated but only unit-covered by the
single-layer case), `nBufferCellsNoExtrude > 0` loop translated, not run on ESI data; relaxed quality dict
(iteration >= `nRelaxedIter`) not reached by this case (the port passes the default controls); no face
zones, baffles, flip map or parallel; `polyTopoChange` is modelled only for the changes `addPatchCellLayer`
makes. The `mapPolyMesh` maps are not produced (the face map `AddedLayers::face_map` is enough for the table).
`doLayers` steps after `addLayers` that are not ported: the final `checkMesh`, removing the empty
`allBoundary` patch (only compared), `writeLayerData`.
PETIR: the new code uses `+ - * /`, `sqrt` and comparisons only (no transcendental), 0 libm-dependent values.
