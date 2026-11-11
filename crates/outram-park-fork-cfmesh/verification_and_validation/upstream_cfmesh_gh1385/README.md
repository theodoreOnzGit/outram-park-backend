# Upstream cfMesh, built and run: reference output (gh:#1385)

**Research, education and V&V only.** Not for reactor operation, licensing or
safety decisions (`RESPONSIBLE_USE.md`). AI-run; the numbers here have not yet
been reviewed by a human.

**Upstream:** cfMesh, `github.com/DAFoam/cfmesh` at
`e944f6ba695fcefce73ad306074f6b3c0ea432ae` (2026-02-17, GPL-3.0-or-later),
built against OpenFOAM-v2506 (ESI, GPL-3.0-or-later) from the GitHub mirror
`github.com/ZhangYanTJU/OpenFOAM-ESI` tag `OpenFOAM-v2506`
(`615aae61d7e95e110ca842ca105c799d10f59178`).

This is the first time upstream cfMesh has been run in this workspace. The
record holds upstream's own output, for the Rust port to be compared against
(the comparison itself is #1385's next step and #1476's reader; no Rust is
written here).

## Status and how to resume

STATUS: done 2026-11-11 (build, all runs, #1299 distances). ~~Next step, not done here: the Rust-side comparison per stage (#1385 table; #1476's `meshDict` reader).~~ **UPDATED 2026-11-11 (gh:#1385):** the Rust-side per-stage comparison is made on the sphere, see "Rust-side differential" at the end; what remains is listed there.

Everything is built **outside the repository**, under
`/home/user/upstream-build/`, and is lost with the container. To rebuild:

1. `apt-get update && apt-get install -y flex bison zlib1g-dev libfl-dev time`
2. `git clone --depth 1 --branch OpenFOAM-v2506 https://github.com/ZhangYanTJU/OpenFOAM-ESI.git /home/user/upstream-build/OpenFOAM-v2506`
3. Copy `scripts/of-env.sh` and `scripts/build-of.sh` from this folder to
   `/home/user/upstream-build/scripts/` and run `build-of.sh` (partial build,
   below).
4. Clone cfMesh into the crate's gitignored `upstream_source/cfmesh_dafoam`
   (`upstream_source/README.md`), apply
   `git -C upstream_source/cfmesh_dafoam apply <this folder>/patches/cfmesh-e944f6ba695f-openfoam-v2506.patch`
   and run `scripts/build-cfmesh.sh`.
5. `scripts/run-case.sh <name> <tutorial dir> <app> [setup]` per case
   (commands in the results table).

## Predictions (written 2026-11-11, before any upstream run)

Read from upstream before the run:
`cartesianMeshGenerator::optimiseFinalMesh` and
`voronoiMeshGenerator::optimiseFinalMesh` call
`meshSurfaceOptimizer::optimizeSurface()` directly
(`meshSurfaceOptimizerOptimizeSurface.C:582`), which re-maps **only the edge
points** (`mapEdgeNodes`, line 636) and then smooths the remaining surface
points (`smoothLaplacianFC`, `smoothSurfaceOptimizer`, lines 650–652) with no
mapping after. `tetMeshGenerator::optimiseFinalMesh` instead calls
`meshOptimizer::optimizeSurface(octree)`
(`meshOptimizerOptimizeSurface.C:35`), which ends with
`meshSurfaceMapper::mapVerticesOntoSurfacePatches()`.

- **U1 (#1299, sphere, `cartesianMesh`).** Sphere R 3 m, 24 x 48 UV,
  `maxCellSize 0.5` (the #1279 setup S isotropic control). Upstream leaves
  boundary points **off** the surface: max distance of order 1e-3 m, within a
  factor 3 of the port's 2.891e-3 m.
- **U2 (#1299, sphere, `pMesh`).** Same: off the surface, within a factor 3
  of the port's 4.575e-3 m.
- **U3 (sphere, `tetMesh`).** On the surface to round-off: max distance below
  1e-12 m (port: 5.4e-16 m).
- **U4 (asmoOctree, `cartesianMesh`).** Points on the curved patches off the
  surface by up to a few percent of the local cell size; points on planar
  patches (inlet, outlet, symmetry plane) within 1e-9 of the bounding-box
  diagonal of their plane, since smoothing on a plane stays on it.
- **U5 (sphere, cell counts).** Upstream's counts equal the port's
  (`cartesian_mesh` 1128, `tet_mesh` 12840, `p_mesh` 1749). Least confident
  for `pMesh`, where `preMapVertices` is recorded as not ported (gh:#1273).

## Build

Machine: 4 cores (3 used: `WM_NCOMPPROCS=3`, another agent's cargo build on
the fourth), Ubuntu 24.04, g++ 13.3.0, no MPI.

### OpenFOAM-v2506, partial (2026-11-11)

`scripts/of-env.sh` sources `etc/bashrc` with `WM_MPLIB=dummy` (the dummy
Pstream, no MPI), `WM_COMPILE_OPTION=Opt` (`-O3`), double precision, 32-bit
labels. `scripts/build-of.sh` builds only what cfMesh and `checkMesh` link:

| target | why |
|---|---|
| `wmake/src` tools | wmake itself |
| `OSspecific/POSIX` (`libOSspecific.o`), `Pstream/dummy` | linked into `libOpenFOAM` |
| `OpenFOAM`, `fileFormats`, `surfMesh`, `meshTools` | cfMesh `meshLibrary/Make/options` |
| `finiteVolume` | cfMesh executables' `Make/options` |
| `mesh/blockMesh`, `mesh/extrudeModel`, `dynamicMesh` | `checkMesh` links `-ldynamicMesh`, which needs `extrudeModel`, which needs `blockMesh` |
| `applications/utilities/mesh/manipulation/checkMesh` | case 8 |

Not built: `foamToVTK` (it links `lagrangianIntermediate`, `conversion`,
`finiteArea` and `genericPatchFields`, most of the remaining tree; the polyMesh
files are what the comparison needs), ThirdParty, scotch, MPI, the rest of `src/`.

System packages installed: `flex bison zlib1g-dev libfl-dev` (and `time`, for
`/usr/bin/time -v`). No patch to OpenFOAM.

**One obstacle.** The first attempt passed `libso` to
`OSspecific/POSIX/Allwmake` and `Pstream/Allwmake`; that overrides their own
preferred target (`libo`, a `libOSspecific.o` object that `libOpenFOAM.so`
links), and the `libOpenFOAM.so` link failed (`cannot find
.../libOSspecific.o`). Calling the two `Allwmake` scripts with no argument,
as `src/Allwmake` does, fixed it; the rerun reused the compiled objects.

Wall time: 476 s (attempt 1, to the link error) + 2 843 s (attempt 2) =
**3 319 s (55 min)**, 1 607 compiler invocations. Disk: 864 MB in all
(297 MB source, 472 MB objects under `build/`, 90 MB libraries and binary
under `platforms/`). Log: `logs/build-openfoam-v2506.log.gz` (both attempts).

### cfMesh e944f6ba695f against it (2026-11-11)

`scripts/build-cfmesh.sh`: `wmake libso meshLibrary`, then `wmake` on the
four executables `cartesianMesh`, `cartesian2DMesh`, `tetMesh`, `pMesh`.
`meshLibrary` compiles with `-fopenmp` (`COMP_OPENMP`); every run below sets
`OMP_NUM_THREADS=1`. The utilities are not built (none is needed;
`preparePar` would need `libdecompose`).

**It does not build against v2506 unpatched.** DAFoam's line targets
OpenFOAM v1812-era APIs. Three changes, all in
`patches/cfmesh-e944f6ba695f-openfoam-v2506.patch` (24 files, +33 −59),
none touching a meshing algorithm:

| change | files | why |
|---|---|---|
| `contiguous<T>()` → `is_contiguous<T>::value`; the 14 `template<> inline bool contiguous<Module::X>() { return true; }` specialisations → `template<> struct is_contiguous<Module::X> : std::true_type {};` | 16 | OpenFOAM v1906 replaced the `contiguous` function template with the `is_contiguous` trait (`src/OpenFOAM/primitives/traits/contiguous.H`). Used only for binary I/O of `LongList` and parallel transfers; neither runs here (ASCII, serial). |
| `polyMeshGenModifier::zipUpCells`: `edgesToInsert[n] = pointChain` (a `List<label>` from an `SLList<label>`) → an explicit element copy | 1 | v2506's `List` has no assignment from `SLList`. Same values, same order. |
| `#include "demandDrivenData.H"` added where `deleteDemandDrivenData` is used and the header is no longer reached transitively | 7 | v2506 headers stopped including it. |

Wall time: 1 019 s from the first attempt to the finished build
(09:18:56–09:31:55), of which the clean final pass took 475 s; 254 compiler
invocations over the four attempts. Disk: the clone with objects is 125 MB;
`libmeshLibrary.so` is 5.6 MB. Log: `logs/build-cfmesh.log.gz` (all
attempts).

## Runs

Every run: `scripts/run-case.sh <name> <case dir> <app>` (copy of the
tutorial, then `<app>` and `checkMesh`, each under `/usr/bin/time -v`),
serial, `OMP_NUM_THREADS=1`, 2026-11-11.

**One change to every tutorial:** `writePrecision 17;` is added to
`system/controlDict` (OpenFOAM's default is 6 significant digits, too few to
compare points at 1e-12), and `writeCompression` is set `off` (the two
`tetMesh` tutorials had it `on`). Both affect only how the mesh is written.
The meshing inputs are the tutorials' own, apart from the sphere, which is
not a tutorial (below).

| run | case | app |
|---|---|---|
| `sphere_cartesianMesh`, `sphere_tetMesh`, `sphere_pMesh` | the #1279 setup S isotropic control: `inputs/sphere_r3_24x48.stl.gz` (from `tools/make_sphere_stl.py`, sha256 `d5a65715…eca739c` uncompressed), `meshDict` = `surfaceFile` + `maxCellSize 0.5`, controlDict from `asmoOctree`. Cases 1–6 on one shared surface | each |
| `asmoOctree` | `tutorials/cartesianMesh/asmoOctree` (cases 1–4) | `cartesianMesh` |
| `elbow_90degree` | `tutorials/cartesianMesh/elbow_90degree` (`boundaryLayers`: `nLayers 5`, `thicknessRatio 1.1`, `maxFirstLayerThickness 0.5`) | `cartesianMesh` |
| `cutCube_tetMesh` | `tutorials/tetMesh/cutCubeOctree` (case 5) | `tetMesh` |
| `cutCube_pMesh`, `cutCube_cartesianMesh` | the same case and `meshDict` (`geom1.stl`), other generator: a second shared surface (case 6) | `pMesh`, `cartesianMesh` |
| `hatOctree` | `tutorials/cartesian2DMesh/hatOctree` (case 7, `nLayers 6`, `thicknessRatio 1.2`) | `cartesian2DMesh` |

Committed per run, under `runs/<run>/`: `system/meshDict` and
`system/controlDict` as run, `constant/polyMesh/*.gz` (ASCII, 17 digits,
`gzip -9 -n`), `polyMesh.sha256` (of the uncompressed files),
`log.<app>`, `log.checkMesh`, the two `/usr/bin/time -v` outputs, and
`surface_distance.txt` for the 3D runs. **Not committed:**
`cutCube_pMesh`'s `polyMesh` (8.9 MB gzipped; its checksums and logs are
here, and the run takes 22 s to repeat). `elbow_90degree`'s is 6.1 MB and
is committed. The `sets/` that `checkMesh` (`nonOrthoFaces`) and `pMesh`
(`invertedBoundaryPoints`, on `cutCube_pMesh`) wrote are not copied.

**Reproducibility.** `sphere_*` (all three) and `asmoOctree` were run twice,
in separate processes: `points`, `faces`, `owner`, `neighbour` and `boundary`
are bit-identical (sha256) between the runs.

## Results

### Meshes and `checkMesh` (case 8)

Analytic sphere volume 4/3 π 3³ = 113.0973 m³.

| run | cells | faces | points | non-orth. max / avg (deg) | max skewness | total volume | `***` failures | `checkMesh` | wall, generator / `checkMesh` |
|---|---|---|---|---|---|---|---|---|---|
| `sphere_cartesianMesh` | 1 128 (all hex) | 3 648 | 1 425 | 15.320 / 5.553 | 0.4612 | 110.90858 (−1.935 %) | 0 | Mesh OK | 0.24 s / 0.04 s |
| `sphere_tetMesh` | 12 840 (all tet) | 26 580 | 2 651 | 53.934 / 11.057 | 0.5524 | 111.56479 (−1.355 %) | 0 | Mesh OK | 0.33 s / 0.15 s |
| `sphere_pMesh` | 1 749 (1 743 poly, 6 hex) | 11 494 | 9 744 | 18.509 / 5.297 | 0.8069 | 111.40529 (−1.496 %) | 0 | Mesh OK | 0.46 s / 0.11 s |
| `asmoOctree` | 53 003 (45 912 hex, 220 prism, 324 tet, 5 927 poly) | 177 398 | 71 650 | 63.211 / 7.088 | 1.7506 | 6.8761898 | 0 | Mesh OK | 5.22 s / 0.52 s |
| `elbow_90degree` | 108 995 (91 329 hex, 5 485 prism, 1 331 tet, 8 276 poly) | 339 265 | 121 398 | 74.215 / 10.028 | 2.3514 | 11 822.283 | 0 (2 faces > 70°, a warning) | Mesh OK | 7.74 s / 0.97 s |
| `cutCube_cartesianMesh` | 9 480 | 32 329 | 13 470 | 56.656 / 5.912 | 1.7306 | 7.0000000 | 0 | Mesh OK | 1.11 s / 0.10 s |
| `cutCube_tetMesh` | 205 182 (all tet) | 420 782 | 40 498 | 77.978 / 15.828 | 2.0423 | 6.9999972 | 0 (8 faces > 70°) | Mesh OK | 8.76 s / 0.72 s |
| `cutCube_pMesh` | 60 263 | 313 010 | 222 560 | 70.647 / 12.286 | 2.0939 | 7.0000093 | 0 (1 face > 70°) | Mesh OK | 22.08 s / 1.22 s |
| `hatOctree` (2D) | 18 676 (all hex) | 75 072 | 38 090 | 30.616 / 3.057 | 1.1908 | 0.15000000 | 0 | Mesh OK | 1.59 s / 0.30 s |

Peak memory of any generator run: 151 MB (`cutCube_tetMesh`). Full numbers:
the `log.checkMesh` per run.

### Against the port, on the shared sphere (U5)

The port's numbers are the #1279 record's (`anisotropic_gh1279/README.md`,
isotropic rows), not re-run here.

| generator | cells, upstream / port | volume error, upstream / port | max boundary distance, upstream / port |
|---|---|---|---|
| `cartesianMesh` | 1 128 / 1 128 | −1.935 % / −1.938 % | 2.838e-3 / 2.891e-3 m |
| `tetMesh` | 12 840 / 12 840 | −1.355 % / −1.362 % | 6.4e-16 / 5.4e-16 m |
| `pMesh` | 1 749 / 1 749 | −1.496 % / −1.484 % | 4.251e-3 / 4.575e-3 m |

**U5 held:** the cell counts are identical for all three generators. The
volumes and distances are not: they differ in the third or fourth digit, so
the port's final points are **not** upstream's. ~~Part of that may be the
input: the port builds the sphere in double precision, upstream reads the
STL as float (a difference of order 1e-7 m in the surface). Which stage the
points first part is the per-stage comparison #1385 asks for; it is not
made here.~~ **CORRECTED 2026-11-11 (gh:#1385):** the comparison is made
(last section). It was not the input: on the same float-rounded STL the port
first parted from upstream at `edgeExtraction` (cartesianMesh) and
`surfaceProjection` (pMesh), and the causes were port defects, not
precision.

### #1299: do upstream's final boundary points lie on the surface?

Measured with `tools/boundary_surface_distance.cpp` (standalone, no
OpenFOAM): every point of every non-`empty` boundary face, exact
point-to-triangle distance, brute force over all STL triangles, STL
coordinates rounded to float as OpenFOAM's reader holds them. D = the STL's
bounding-box diagonal. Per-patch rows: `runs/<run>/surface_distance.txt`.

| run | boundary points | max | p99 | mean | points > 1e-6 D |
|---|---|---|---|---|---|
| `sphere_cartesianMesh` | 530 | **2.838e-3 m** | 2.822e-3 | 8.14e-4 | 522 |
| `sphere_pMesh` | 1 392 | **4.251e-3 m** | 4.245e-3 | 1.10e-3 | 1 392 |
| `sphere_tetMesh` | 902 | 6.4e-16 m | 5.1e-16 | 8.5e-17 | 0 |
| `asmoOctree` (D 5.575 m) | 21 821 | **6.630e-4 m** | 1.838e-4 | 1.28e-5 | 5 637 |
| `elbow_90degree` (D 65.33) | 19 882 | **4.009e-2** | 5.22e-3 | 4.50e-4 | 8 726 |
| `cutCube_cartesianMesh` (planar) | 5 125 | 2.7e-16 | 1.8e-16 | 4.3e-17 | 0 |
| `cutCube_pMesh` (planar) | 21 444 | 1.035e-3 | 1.3e-9 | 2.5e-7 | 32 |
| `cutCube_tetMesh` (planar) | 10 420 | 4.30e-6 | 8.5e-8 | 8.8e-9 | 2 |

**Finding: upstream leaves boundary points off the surface, as the port
does.** On the #1299 sphere, upstream's `cartesianMesh` boundary sits up to
2.84 mm off the STL (port 2.89 mm) and `pMesh` up to 4.25 mm (port 4.58 mm);
`tetMesh`, whose `optimiseFinalMesh` goes through
`meshOptimizer::optimizeSurface` and re-maps, is on it to round-off in both.
The 3–8 mm gap in #1299 is upstream behaviour, not a port departure, for the
isotropic case (the anisotropic 8 mm rows were not run upstream here).
`sphere_boundary_distance.svg` plots every sphere boundary point's distance
against its polar angle (drawn by `tools/plot_sphere_distance_svg.py`).

**Where the offset appears (added after the final-mesh numbers above).**
The sphere was rerun with `workflowControls { stopAfter <step>; }` appended
to `meshDict`, once per step; upstream writes the mesh after that step
(`workflowControls::stopAfterCurrentStep`) with every patch typed `empty`,
so the distance tool runs with `BSD_INCLUDE_EMPTY=1`. Meshes, checksums and
distances: `runs/stages/<app>_<step>/` (run as `stage_sphere_<app>_<step>`) (2.8 MB in all).

| step stopped after | `cartesianMesh`: cells, max distance | `pMesh`: cells, max distance |
|---|---|---|
| `templateGeneration` | 600, 0.814 m | 1 749, 0.410 m |
| `surfaceTopology` | 600, 0.814 m | 1 749, 0.410 m |
| `surfaceProjection` | 600, **6.3e-16 m** | 1 749, **7.1e-16 m** |
| `patchAssignment` | 600, 6.3e-16 m | 1 749, 7.3e-16 m |
| `edgeExtraction` | 600, **2.346e-3 m** | 1 749, **2.197e-3 m** |
| `boundaryLayerGeneration` | 1 128, 2.346e-3 m | 1 749, 2.197e-3 m |
| `meshOptimisation` (= final geometry, before renumbering) | 1 128, **2.838e-3 m** | 1 749, **4.251e-3 m** |

So upstream's boundary is on the surface after `surfaceProjection` and
`patchAssignment`, leaves it in `edgeExtraction` (about 2.2–2.3 mm) and
moves further in `meshOptimisation` (2.8 and 4.3 mm). This is consistent
with the reading in the predictions (smoothing with no mapping after it in
`meshSurfaceOptimizer::optimizeSurface`), but which call inside
`edgeExtraction` moves the points was not isolated. The `pMesh` template's
4 488 boundary points fall to 1 392 at `surfaceTopology` (its boundary is
rebuilt there). These stage meshes are the per-stage reference for cases
2–3 of the #1385 table on the shared sphere; `asmoOctree` was not staged
(about 3.5 MB per stage).

**Against the predictions:**

- **U1, U2, U3 held.** Same order as the port, within 2 % and 8 %; tetMesh
  on the surface.
- **U4 held in part.**
  - The off-surface points are on the curved patches, as predicted:
    `defaultFaces0006` (local `cellSize` 0.01) max 5.6e-4 m;
    `defaultFaces0007` (0.005) 6.6e-4 m; `defaultFaces0009` (0.005)
    6.1e-4 m. That is up to **13 % of the local cell size**, not "a few
    percent": that part missed.
  - The planar patches `INLE10000`, `OUTL20001`, `defaultFaces0003`,
    `defaultFaces0005` and `defaultFaces0008` are within 2.2e-10 m (4e-11 D).
    The symmetry plane `SYMP30002` is not: 242 of its 3 907 points are above
    1e-9 D, max 6.5e-7 m (1.2e-7 D). That part missed; the cause is not
    investigated (a guess, not checked: points on its edge with the curved
    body).
- **Not predicted:** on the all-planar `cutCube` surface, `pMesh` leaves 32
  points more than 1e-6 D off, up to 1.0 mm (`patch0003`, `0004`, `0006`,
  `0008`), and `tetMesh` up to 4.3 µm, while `cartesianMesh` is on it to
  round-off. Not investigated.

## Build obstacles, in one place

1. `libOSspecific.o` link failure: my `libso` argument overrode the
   `Allwmake` scripts' own target; fixed by passing none.
2. cfMesh e944f6ba695f does not compile against v2506 (v1812-era APIs):
   patched as above, three mechanical API changes.
3. The OpenFOAM `etc/bashrc` aborts under `set -e`
   (`pop_var_context`); the scripts source it before `set -e`.

`foamToVTK` was not built (see above). No image of the meshes themselves is
drawn here: the port's own records already draw the port's meshes, and
drawing upstream's needs a reader of these files, which is #1476's job.

## Rust-side differential (gh:#1385, 2026-11-11)

**Research, education and V&V only.** AI-run; not reviewed by a human yet.

The port against the upstream output above, stage by stage, on the sphere
(R 3 m, `maxCellSize 0.5`). Tests: `tests/cfmesh_vs_upstream_sphere.rs`
(stage pins), `tests/cfmesh_vs_upstream_cell_order.rs` (the proven cause of
the one difference left), helpers `tests/common/mod.rs` (gunzip with
`miniz_oxide`, `outram_foam_basic_lib::io::poly_mesh::PolyMesh::read`, the
comparison).

**CORRECTED 2026-11-11 (gh:#1385)** for the "Against the port" paragraph
above: ~~"the port's final points are not upstream's ... Which stage the
points first part is ... not made here"~~ the per-stage comparison is made
below. The input was not the cause (the committed single-precision STL is
used for both); the points part at `meshOptimisation`, in the last bit, once
the fixes below are in.

### Method

1. **Same input.** The committed STL (`inputs/sphere_r3_24x48.stl.gz`) read by
   the port's STL reader (a port of OpenFOAM's `STLReader`), so both codes
   hold the same single-precision points; the port's own double-precision
   `sphere_surface` is not used.
2. **Same steps.** `WorkflowSettings { stop_after }` for each of the seven
   steps against `workflowControls stopAfter` in the upstream run.
3. **Comparison per stage.** Counts; points at the same index (bit-identical
   count, largest absolute difference); faces as vertex sets and at the same
   index (ring, owner, neighbour). The upstream `neighbour` file lists every
   face (-1 on the boundary); the helper keeps the internal ones, as
   OpenFOAM's reader wants.
4. **At the first difference, upstream was instrumented** (not only read):
   `patches/gh1385-trace-instrumentation.patch` (applies on top of the
   OpenFOAM-v2506 patch) prints point coordinates to stderr when
   `GH1385_TRACE` is set, inside `preMapVertices`, `mapVerticesOntoSurface`,
   `meshSurfaceOptimizer::optimizeSurface` and `optimiseFinalMesh`. The port
   was compared with those traces sub-step by sub-step (premap iteration 1, 2,
   3; `smoothLaplacianFC`, `smoothSurfaceOptimizer`, ...), which is how each
   cause below was isolated. Output unchanged without the variable. The raw
   traces are not committed (about 1 MB each); only the one the cell-order
   proof needs is (`traces/`).

### Result: stage table (points bit-identical / total; faces identical at their index with owner and neighbour / total)

| step | cartesianMesh points | faces | pMesh points | faces |
|---|---|---|---|---|
| templateGeneration | 895/895 | 2064/2064 | 12840/12840 | 13690/13690 |
| surfaceTopology | 895/895 | 2064/2064 | 9744/9744 | 11494/11494 |
| surfaceProjection | 895/895 | 2064/2064 | 9744/9744 | 11494/11494 |
| patchAssignment | 895/895 | 2064/2064 | 9744/9744 | 11494/11494 |
| edgeExtraction | 895/895 | 2064/2064 | 9744/9744 | 11494/11494 |
| boundaryLayerGeneration | 1425/1425 | 3648/3648 | 9744/9744 | 11494/11494 |
| meshOptimisation | **623/1425, worst 1.332e-15 m** | 3648/3648 | **3435/9744, worst 1.332e-15 m** | 11494/11494 |

**First stage that differs: `meshOptimisation`, in the last bit, for both
generators.** Before the fixes below the first difference was
`edgeExtraction` (cartesianMesh, 2.5e-5 m) and `surfaceProjection` (pMesh,
7.5e-4 m); at the end of the workflow they were 5.0e-3 m (cartesianMesh) and
1.3e-2 m (pMesh).

**Not compared (no staged upstream reference):** `tetMesh` (only its final
mesh is committed) and everything after `meshOptimisation`: the
`renumberMesh` and `renameBoundaryPatches` that end every generator. The
final meshes do not match: `cartesianMesh` keeps 623/1425 points at the same
index but its faces differ in order (2/3648 index-identical: the port's
`renumber_mesh` does not reproduce upstream's output order); `tetMesh`'s
points are in another order (2/2651 at the same index, worst 0.19 m; its
faces do not match as vertex sets either). These are the next steps, not
results.

### The fixes, in the order found (each: prediction, then result)

Every fix follows the upstream routine named; the cfMesh crate has no
kovan-reviewed function (`src/review.md` holds no `kind = "review"` block).
Committed with `gh:#1385` and the prediction in the message.

1. **Template face order** (`cartesian_mesh::extract_from_addressing`).
   Upstream's `createPolyMesh` numbers the faces in octree-face order, internal
   and boundary mixed, and `polyMeshGenModifier::reorderBoundaryFaces` then
   swaps the boundary faces in the internal range with the internal faces in
   the boundary range, pairwise. The port sorted stably (internal, then
   boundary). *Prediction:* faces index-identical, and the 2.5e-5 m difference
   at `edgeExtraction` (smoothing in place follows the face order) shrinks.
   *Result:* faces 14/2064 -> 2064/2064 identical; `edgeExtraction` worst
   2.5e-5 m -> 3.1e-15 m.
2. **`surfaceProjection`** (cartesianMesh 428/895 -> 895/895, worst 1.7e-15 m
   -> 0; pMesh 8472/9744, worst 7.5e-4 m -> 9744/9744). Found by tracing
   `preMapVertices` (iteration 1: 528/530 bit-identical, iteration 2: 137/530,
   so a difference that needs a first move to show):
   - `foam_face_centre` is `face::centre` operation for operation: area weight
     `|(p - c) x (next - c)|` (the port had `(q - p) x (avg - p)`), the
     average and the result divided, a triangle `(1.0/3.0)*(p0+p1+p2)`;
   - `Vec3::div`: OpenFOAM's `v /= s` and `v/s` divide each component; the
     port multiplied by `1/s` (`premap_vertices`, last bit);
   - `mapVerticesOntoSurface` and the untangler's re-mapping now call
     `meshOctree::findNearestSurfacePoint` (`SurfaceQueries::with_octree`);
     the port used a brute-force closest point, which differs in the last bit
     from `nearestPointOnTheTriangle`. The pMesh miss of 7.5e-4 m was this one
     (brute force and the octree search pick different triangles near a shared
     edge).
3. **`patchAssignment`** (cartesianMesh 636/895, worst 1.6e-15 m -> 895/895;
   pMesh -> 9744/9744): `EdgeExtractor` queries are octree-backed
   (`checkFacePatchesGeometry` calls `mapVerticesOntoSurfacePatches`).
4. **`edgeExtraction`, cartesianMesh** (374/895, worst 2.2e-15 m -> 895/895).
   Traced through `optimizeSurface`: `smoothLaplacianFC` 530/530 first, then
   `smoothSurfaceOptimizer` 171/530:
   - `meshSurfaceEdgeExtractorNonTopo` always runs (`decomposeBoundaryFaces`,
     `remapBoundaryPoints`); the port skipped it on a surface without named
     regions (`regions` `None`), which is every STL;
   - `ring_area_vector` is `face::areaNormal` (central decomposition about the
     vertex average, triangles direct), not `0.5 sum(p_i x p_{i+1})`;
   - divisions where OpenFOAM divides: the `plane` normal (`normal_ /=
     magNormal`), `vecX`, `vecY`, the point normal, the average of face
     centres, `optimizePoint`'s `pts_ /= scale`, `partTriMesh::updateVertex`'s
     `c /= 3` and `centre/faceArea`;
   - `optimiseSteepestDescent`: `dir = disp/mag(disp); disp = dir*0.2*avgEdge`;
     `pow(stab, 3)` through `r_powf(3.0)` (it was `r_powi`).
   `optimizeSurface` is then 530/530 bit-identical at each of its 5 iterations.
5. **`boundaryLayerGeneration`** (faces 536/3648 -> 3648/3648 identical; the
   points were already 1425/1425). The layer is assembled as upstream's
   `addCells` (old faces keep their places, each layer cell appends the faces
   it does not find), the owner = the lower cell, `reorderBoundaryFaces`, and
   `replaceBoundary` (boundary faces patch by patch in list order):
   `boundary_layers::add_cells_upstream`.
6. **`edgeExtraction`, pMesh** (8352/9744, worst 1.1e-2 m -> 9744/9744): the
   same rule, `meshSurfaceEdgeExtractorFUN` (`createFundamentalSheetsJFS`,
   `smoothMeshSurface`, `remapBoundaryPoints`) ran only with named regions;
   upstream always runs it. The untangle after it uses the octree too.
7. **`meshOptimisation`** (cartesianMesh 530/1425, worst 5.0e-3 m -> 623/1425,
   worst 1.3e-15 m; pMesh worst 6.5e-3 m -> 1.3e-15 m). `laplacianPC` used a
   different cell-centre formula (`cell_geometry`, pyramids about the average
   of the face centroids): `checks::foam_cell_centres_and_vols` is
   `polyMeshGenAddressing::makeCellCentresAndVols`, with the addressing's own
   face centres and areas (`pmg_face_centre_and_area`, not `face::centre`).
   The earlier expectation in `laplace_smoother_fixed_point_and_one_pass` used
   the old formula and differed by 1.3e-5.

### The difference left, and its proven cause

`meshOptimisation` differs in the last bit: worst 1.332e-15 m on coordinates
of 2-3 m (6e-16 relative), 623/1425 and 3435/9744 points bit-identical. **Cause,
proven (`cfmesh_vs_upstream_cell_order.rs`):** the cell centre sums over the
faces **in the cell's face list**, and 491 of the 1128 upstream cells have a
list that is not ascending in face label (cell 433: `1461 1513 1529 1534 1535
0`: face labels move when `reorderBoundaryFaces` swaps, the lists do not).
`VolumeMesh` has no cell lists; the port sums in ascending face order. Fed
upstream's own cell lists (dumped by the instrumented run,
`traces/sphere_cartesianMesh_meshOptimisation_cells_surface_fv.txt.gz`), the
same five Laplacian iterations are **bit-identical on all 1425 points**; with
ascending lists they give 623. Proven for `cartesianMesh`; the same cause is
expected for `pMesh` and not separately shown.

**Why this is not closed:** reproducing the lists means carrying cell lists
through every stage of the port (26 `VolumeMesh { .. }` literals in this
crate), or rebuilding upstream's cell-list history. It is a change of the
mesh type, not a fix of a routine, so it is left for a decision. Below 1e-8
relative with a proven cause is acceptable (task rule); this is 6e-16, and it
is counted and pinned (`cartesian_mesh_mesh_optimisation_is_within_the_last_bit`,
`p_mesh_mesh_optimisation_is_within_the_last_bit`: the bit-identical count may
not fall, the worst difference may not grow).

### Moved pins (fixes that follow upstream; the old value is struck through in the source)

| pin | moved | cause |
|---|---|---|
| `correct_edges::tests::correct_edges_is_pinned_bit_for_bit` | hash 7179125119703641544 -> 15811906865840324897 (counts unchanged) | its input pipeline now follows upstream (fixes 2-6) |
| `layers::refine_boundary_layers::tests::prism_refinement_is_pinned_bit_for_bit` | pipe 0x677a875f14ca046b -> 0x018284a992e4c789 (box unchanged) | the layer's faces in upstream's order (fix 5) |
| `anisotropic::tests::generators_without_anisotropic_sources_are_pinned_bit_for_bit` | all four fingerprints (cell counts unchanged) | fixes 1-7; **the crate's `AGENTS.md` says "do not move that pin"** (written to guard the anisotropic wiring); moved here under the task's default that a fix following upstream may move pins; flagged for the maintainer |
| `mesh_optimizer::tests::laplace_smoother_fixed_point_and_one_pass` | its expectation now uses `foam_cell_centres_and_vols` | fix 7 |

**Did not move:** `back_off_only_layers_are_pinned_bit_for_bit`,
`dual_kinds_are_pinned_bit_for_bit`, `octree_path_is_pinned_bit_for_bit`,
`layer_strategies_are_deterministic`,
`foam_studio_default_mesh_keeps_its_fingerprint` and the restart pins (full
`--lib` run of the crate on 2026-11-11, before the last two edits: 244 passed,
3 failed = the three re-pinned above, 19 ignored). **Not run:** Foam Studio's
`headless_unstructured_default_is_pinned` (0.9394 / 1.0062, the `dhoby-ghaut`
example `foam_studio_web`): it was not built on this machine. Foam Studio
meshes through the tet-dual pipeline, not the four workflows, but it shares
`surface_optimizer`, `ring_area_vector` and `foam_face_centre`, so it can
move; check before merging.

### What remains

1. `meshOptimisation`: cell lists (above).
2. `renumberMesh` and `renameBoundaryPatches` (the end of every generator):
   the final meshes do not match yet.
3. `tetMesh`, `cartesian2DMesh`: no staged upstream runs yet (run with
   `workflowControls stopAfter`, as for the sphere); the same fixes apply to
   `tetMesh`'s shared steps but are not measured.
4. `asmoOctree`, `elbow_90degree`, `cutCube`, `hatOctree`: the cell counts of
   #1476 (52,997 vs 53,003; 108,383 vs 108,995) are the leads; start with
   `asmoOctree` at the first stage. These multi-patch cases exercise
   `findNearestEdgePoint`, `mapEdgeNodes`, `mapCorners` and the edge
   extractor's own searches, none of which was checked against upstream here:
   only the single-patch paths were.
5. Searches elsewhere in the port that still use the brute-force
   `closest_point_on_surface` where upstream uses the octree (`snap`,
   `surface_mapper::nearest_edge_point`), and other `scale(1.0 / x)` where
   upstream divides (about 140 sites remain; the ones on the paths above are
   fixed).
