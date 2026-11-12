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

Disk cost is recorded in the Results section.

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
| P1 | **Iteration by iteration, the number of cells ESI's first `surfaceOnlyRefine` selects equals the port's `surface_only_refine`** (integers, same doubles, same segments, same face-only 2:1 closure), and so does the leaf-level histogram after the loop. | `refinementSurfaces::findHigherIntersection` uses an octree `findLine` on the triSurfaceMesh, the port a brute-force segment test; a segment grazing a triangle edge could split them. The STL is read as ASCII text in both. |
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

(results below)
