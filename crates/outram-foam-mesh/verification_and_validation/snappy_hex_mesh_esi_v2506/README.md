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

(filled in below after the runs)
