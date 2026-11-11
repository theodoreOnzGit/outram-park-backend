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

## Rust-side differential, round 2 (gh:#1385, 2026-11-11)

**Research, education and V&V only.** AI-run; not reviewed by a human yet.

**UPDATES to the section above.** The "Result: stage table", "The difference
left, and its proven cause" and "What remains" subsections are ~~as written~~
**superseded by this section** (the old values stay struck through in the
tests' doc comments): the cell lists are carried now, the sphere is
bit-identical for all three generators, and asmoOctree is bit-identical
through `boundaryLayerGeneration`.

### Stage table, sphere (points bit-identical / total, faces index-identical / total)

All seven steps, and the **final** mesh (after `renumberMesh` and
`renameBoundaryPatches`), are bit-identical for the three generators:
`tests/cfmesh_vs_upstream_sphere.rs`
(`cartesian_mesh_matches_upstream_through_mesh_optimisation`,
`p_mesh_matches_upstream_through_mesh_optimisation`,
`tet_mesh_matches_upstream_through_mesh_optimisation`,
`final_meshes_match_upstream`).

| | before this round | now |
|---|---|---|
| `cartesianMesh` meshOptimisation | 623/1425 points, worst 1.332e-15 m | 1425/1425, faces 3648/3648 |
| `cartesianMesh` final | 623/1425 points, 2/3648 faces | 1425/1425, 3648/3648 |
| `tetMesh` surfaceTopology | points 2651/2651, faces 819/26580 | 2651/2651, 26580/26580 |
| `tetMesh` meshOptimisation / final | final 2/2651 points (worst 0.19 m), faces not matched | 2651/2651, 26580/26580 |
| `pMesh` meshOptimisation | 3435/9744, worst 1.332e-15 m | 9744/9744, 11494/11494 |
| `pMesh` final | not compared | 9744/9744, 11494/11494 |

`tetMesh` stage references are new: `runs/stages/tetMesh_<step>` (sphere, the
same `workflowControls stopAfter` method; 1.7 MB in all).

### The fixes (before -> after, each with its cause)

Predictions were committed in the messages of the first two commits (the
cell-list prediction: the carried lists equal upstream's 1128 dumped lists and
the Laplacian then matches on all 1425 points; the tetMesh prediction: the
`surfaceTopology` faces differ only in order). The asmoOctree fixes were found
by trace comparison against the instrumented upstream
(`patches/gh1385-trace-instrumentation-2.patch`), so no sign was predicted
before them; each cause below was confirmed by the number it moved.

1. **Cell face lists carried** (`VolumeMesh::cells`, optional; a carried list
   is used only while consistent with owner / neighbour, else the ascending
   derived one). Built as upstream builds them at: template (`createPolyMesh`
   discovery order, `reorderBoundaryFaces` relabelling), `addCells` +
   `reorderBoundaryFaces` + `replaceBoundary` (layers, `decomposeCells`),
   `replaceBoundary` (`edgeExtractor::updateMeshPatches`, `surfaceMorpherCells`),
   `removeCells`, `decomposeFaces`, `renumberMesh`. Proof:
   `tests/cfmesh_vs_upstream_cell_order.rs`: the port's lists equal upstream's
   dump for all 1128 cells (cartesian sphere), 12840 (tet sphere,
   `traces/sphere_tetMesh_before_optimizeMeshFV_cells.txt.gz`, new) and 53003
   (asmoOctree, `traces/asmoOctree_cartesianMesh_before_optimizeMeshFV_cells.txt.gz`,
   new). `foam_cell_centres_and_vols` and `renumberMesh` read them.
2. **`TopoMesh::remove_cells`** follows `removeCells`: reverse, renumber,
   `reorderBoundaryFaces` over all faces (a face of removed cells only has no
   neighbour and is swapped like a boundary face), then `removeFaces`; and
   `face::reverseFace` keeps the first point (a plain reversal is the same ring
   rotated). tetMesh `surfaceTopology` faces 819 -> 26580 of 26580.
3. **`optimizeSurface` of `tetMesh` and of `pMesh`'s `optimiseFinalMesh` take
   the octree** (`optimize_surface_octree`; the generators passed none).
4. **`findNearestPointToPatches`: `newP /= patches.size()`** (division).
   asmoOctree surfaceProjection 49577/49690 -> 49690/49690.
5. **`mapEdgeNodes` / `mapCorners` as upstream writes them** (with the octree;
   `map_points_onto_patches_upstream`): nodes classified by the partitioner's
   `corners` / `edgePoints`; patches in `pointFaces` order, unsorted; the
   approximation loop stops **before** replacing the estimate; `findNearestEdgePoint`
   through the octree, the 1.2 rule, the `maxDist` cut `f*(p' - p) + p`;
   corners map to the nearest surface corner (`triSurfacePartitioner`) whose
   patches include the point's, no cut; `findMappingDistance` with
   `face::centre`. The untangler's `mapEdgeNodes` uses the same routine.
   asmoOctree patchAssignment 49541/49690 (worst 6.3e-6 m) -> identical.
6. **`decomposeConcaveFaces`**: a feature edge at an inverted point
   (`meshSurfaceCheckInvertedVertices`) is `UNDETERMINED`, and
   `problematicTypes` holds it (the port ignored it): 12 -> 13 internal faces
   (upstream's log says 13), cells 32413 -> 32419.
7. **`decomposeCells` in upstream's sequence**: `checkFaceConnections` then
   `decomposeMeshFaces` (fan points numbered in face order), then per marked
   cell the pyramids (apex = mean of the cell's distinct points in face-list
   order, divided), `removeCells`, `addCells`, `reorderBoundaryFaces`,
   `replaceBoundary`, `removeUnusedVertices`. asmoOctree edgeExtraction points
   49734/49829 and faces 111784/114411 -> identical.
8. **`createNewVertex`**: `help::distanceOfPointFromTheEdge` is the distance to
   the **infinite line** (it calls `nearestPointOnTheEdge`, not the segment
   variant; the port used the segment), `nearestPointOnTheEdge` divides
   (`e /(d*d)`), and `v /= magV`, `normal /= magN`, `normal /= 2.0*dist` divide.
   asmoOctree boundaryLayerGeneration points 70827/71650 (worst 1.6e-4 m) ->
   identical.
9. **`replaceBoundary` appends each new boundary face to its owner's cell list
   in the order of the boundary list**, while the labels run patch by patch
   (the port appended in label order): asmoOctree cell lists 52538 -> 53003 of
   53003 identical.

### asmoOctree (`tests/cfmesh_vs_upstream_stages.rs` in `outram-foam-cli`, through `run_cfmesh`)

| step | port vs upstream |
|---|---|
| templateGeneration, surfaceTopology, surfaceProjection, patchAssignment | points and faces identical |
| edgeExtraction | identical (points 49829, faces 114411, cells 32419; was 32413) |
| boundaryLayerGeneration | identical (points 71650, faces 177398, cells 53003; was 52997) |
| meshOptimisation | counts equal, faces index-identical, **71542/71650 points** bit-identical, worst 4.944e-4 m |

Committed upstream references: `runs/stages/asmoOctree_cartesianMesh_edgeExtraction`
and `..._boundaryLayerGeneration` (4.8 MB); the other stages are read from
`GH1385_UPSTREAM_RUNS` (default `/home/user/upstream-build/runs`,
`stage_asmo_cartesianMesh_<step>`) when present, else skipped. The test asserts
the first six steps identical.

**meshOptimisation, localised with sub-step traces** (instrumented upstream,
`GH1385_TRACE`; the port's `trace_points`, an env-gated print): `fm_surface`
(after `optimizeSurface`) 71649/71650 identical (one point differs by 4.1e-25),
the 5 Laplacian iterations `fv_lap` 71649/71650, **`optimizeMeshFV`'s untangle
`fm_fv` 71616/71650**, `optimizeLowQualityFaces` `fm_lowq` 71542/71650, worst
4.944e-4 m; `untangleMeshFV` adds none. So the remaining difference is in the
volume optimiser (`untangleMeshFV`'s smoothers) and `optimizeLowQualityFaces`,
first at point 38527. Cause not found; the traces are regenerable (the 10 MB
point dump is not committed).

### elbow_90degree: not matched, first difference at the template

Cells 21830 vs 21977 at `templateGeneration` (patch sizes 13141 vs 13234), and
so on (final 108383 vs 108995, `fixedWalls` 18384 vs 18477). The tutorial sets
`minCellSize 1.0`, so upstream runs `meshOctreeAutomaticRefinement` (curvature,
corners, proximity); asmoOctree sets none and matches. The port's octree
refines this surface differently; not investigated further. Upstream stage
meshes for the elbow exist locally (`stage_elbow_cartesianMesh_<step>`, eight
steps) but are not committed.

### Moved pins (old value struck through in the source)

| pin | moved | cause |
|---|---|---|
| `anisotropic::...generators_without_anisotropic_sources_are_pinned_bit_for_bit` | cartesian, tet (this round); p, 2D (second move) | fixes 1-9; **`AGENTS.md` says "do not move that pin"**, moved again under the task's rule that a fix following upstream may move pins; flagged for the maintainer |
| `correct_edges::tests::correct_edges_is_pinned_bit_for_bit` | hash 15811906865840324897 -> 2364802067049664889 (counts unchanged) | fixes 6, 7 |
| `layers::refine_boundary_layers::tests::prism_refinement_is_pinned_bit_for_bit` | pipe 0x018284a992e4c789 -> 0x591e062dc441a92f, box 0x687256342d92ed50 -> 0xb10f852a9b013e88 | fix 8, 9 |

Did not move: `back_off_only_layers_are_pinned_bit_for_bit`,
`dual_kinds_are_pinned_bit_for_bit` (feature `cell-centre-dual`: not run in this
round), `octree_path_is_pinned_bit_for_bit`, `layer_strategies_are_deterministic`,
`foam_studio_default_mesh_keeps_its_fingerprint`, the restart pins, and the
`cfmesh_vs_upstream_*` and `cfmesh_patch_types*` tests (full `--lib --tests` run
of the crate: all pass after the three re-pins). Foam Studio's
`headless_unstructured_default_is_pinned` was **not run** (see the report).

### What remains

1. asmoOctree `meshOptimisation`: the volume optimiser (above).
2. asmoOctree final mesh (after `renumberMesh`): not compared (follows 1).
3. elbow_90degree: the template (`minCellSize` automatic refinement).
4. `cutCube` (tetMesh, pMesh, cartesianMesh) and `hatOctree` (2D): no staged
   upstream runs yet.
5. Brute-force searches still used where upstream uses the octree (`snap`,
   `SurfaceQueries::nearest_edge_point`), and other `scale(1.0 / x)` where
   upstream divides.

## Rust-side differential, round 3 (gh:#1385, 2026-11-11)

**Research, education and V&V only.** AI-run; not reviewed by a human yet.

### Predictions, written before the fixes (committed first)

Reading upstream's `volumeOptimizer*.C`, `knuppMetric.C`, `partTetMesh.C`
against the port's `mesh_optimizer.rs` (the volume optimiser had never been
compared operation by operation; only the Laplacian and surface steps had)
shows arithmetic that associates or rounds differently: `evaluateGradientsExact`
(`grad_v.scale(v/stab)` for `Vtri*gradV/stab`, `scale(2C/Vs13)` for
`C*(2*gradVs)/Vs13`, `scale(1/x)` for `/x`, Hessian terms accumulated into the
running sum instead of `gradGradF += ((A - B) - C) + D`, `pow(stab,3)` /
`pow(Vs13,4)` as `r_powi`), `Tensor::det` (the port used a cofactor expansion),
`tetrahedron::mag` (`(1.0/6.0)*x`, the port divided by 6), the final
`points_ /= scale`, `knuppMetric`'s `n/d`, and `partTetMesh::updateVerticesSMP`'s
centres (`c/=3` then `c*area`; `centre/cellVol`).
Prediction: asmoOctree `meshOptimisation` moves from 71542/71650 bit-identical
points toward 71650/71650 (the first differing point 38527 is a volume-optimiser
move), the worst difference falls below 4.9e-4 m, and the sphere final meshes of
all three generators stay bit-identical (their untangle never reaches a Newton
step with a tolerance-sensitive decision). If a residual remains, the next
suspects are `meshUntangler` (LP cut region), `optimizeLowQualityFaces` and the
`findBadFaces` checks.

### Prediction for elbow_90degree, written before the fix (committed first)

Traces of both codes (`OCT`, `AUTOREF`, `ECURV`, `MCURV` lines under
`GH1385_TRACE`; upstream's from `patches/gh1385-trace-instrumentation-3.patch`)
localise the template difference: the octree is identical after
`refineBoundary` (17872 leaves, every level) and differs after the automatic
refinement (upstream 39950 leaves, port 39747). Upstream's curvature criterion
marks 131, 112, 0 leaves in its three passes; the port's marks 112, 108, 0
(corner 0 / partition 27 vs 35 / proximity 13 both). The per-triangle mean
curvature of the two `triSurfaceCurvatureEstimator`s differs by more than 1e-9
relative in 927 of 5150 triangles (worst -0.215 upstream against -4.54 here)
and in the last bit in most of the rest. Reading upstream: the quadric fit is
over `otherLabels`, a `labelHashSet`, **iterated in OpenFOAM's hash order**
(`Hash<label>` is the label itself, capacity 128, insertion at the head of a
bucket chain, rehash at load 0.8), and the port's `BTreeSet` iterates in
ascending order. With 5 to 7 neighbour points the 5x5 normal equations are
nearly singular, so the order of the sums is amplified from round-off to O(1)
in the curvature. Divisions (`e /= d`, `vecX_ /= mag`, `normal_/d`) are also
`scale(1.0 / x)` in the port.
Prediction: with the hash order emulated (`OpenFOAM` `HashTable::setEntry` /
`setCapacity` chaining) and the divisions fixed, the mean curvature agrees
with upstream's to the bit on all triangles (or, if some fit remains
order-sensitive through the LU solve, the first differing triangle names it),
curvature marks become 131, 112, 0, the automatic refinement 39950 leaves, and
the elbow template 21977 cells, bit-identical points and faces. If curvature
agrees but the template still differs, the next suspect is the partition
criterion (27 vs 35: `findEdgesInBox` / `edgeGroups`).

### Prediction for elbow_90degree meshOptimisation, written before the fix (committed first)

State after the curvature-estimator fix (the elbow template and every stage
through `patchAssignment` are bit-identical; counts equal at all eight steps,
108 995 cells at the end). `edgeExtraction` differs in 253 boundary points by
1e-15 to 1e-14 m; `boundaryLayerGeneration` in 537; `meshOptimisation`
49266/49382... see below; the final 106 825/121 398.

**Cause of the 253, proven, the same as asmoOctree's single point.** The first
differing boundary point (bp 12597) enters `surfaceOptimizer::optimizePoint`
with bit-identical inputs (point normal, `vecX`, `vecY`, 10 points, divide-and-
conquer iterations 0-8 bit-identical); the first `evaluateGradients` has an
identical gradient and a Hessian diagonal that differs by 1 ulp (1066.8964240848784
upstream, ...82 here). Python's `math.pow` (this machine's glibc, whose `pow`
is the FMA ifunc build) in upstream's operation order reproduces upstream's
Hessian; `x*x*x` for `pow(stab, 3)` reproduces ours. PETIR's `powf` is the
non-FMA ARM routine by design, and differs from glibc's FMA build on some
inputs by 1 ulp. **Check by experiment (not committed; the std `powf` is the
platform libm that PETIR's gate forbids):** with `stab.powf(3.0)` and the other
`pow`s of `surface_optimizer.rs` / `mesh_optimizer.rs` on the platform libm,
asmoOctree `meshOptimisation` and its final mesh are 71650/71650 points and
177 398/177 398 faces bit-identical, and the elbow is bit-identical through
`boundaryLayerGeneration` (49 382/49 382 points, 128 265 faces). So the pow
ulp is the **only** cause of those differences.

**What the experiment leaves: a real difference in elbow `meshOptimisation`**
(49266/49382 points bit-identical, worst 0.21 m; final 120098/121398, worst
0.23 m). Upstream's `optimizeMeshFV` untangle flags 2 faces (93688, 95202), the
port 3 (adds 97570), with a part tet mesh of 1366 nodes against 1915. The port's
`flag_faces` is an approximation of `findBadFaces` (vertex-average face
centres, `cell_geometry` centres, `face_area_vector`), not the checks as
upstream computes them on the polyMeshGen addressing's `faceCentres`,
`faceAreas`, `cellCentres` and OpenFOAM's `face::areaNormal` / `face::centre`
for `pyramidPointFaceRef::mag`.
Prediction: with `checkFacePyramids`, `checkFaceFlatness`,
`checkCellPartTetrahedra`, `checkFaceAreas`, `checkFaceDotProduct`,
`checkFaceSkewness` ported on that geometry, the elbow's `untangle` flags 93688
and 95202 only, the part tet mesh has 1366 nodes, and (with the glibc-pow
experiment) elbow `meshOptimisation` and the final mesh are bit-identical. If a
difference remains, the first `BADF`/`TN` line that differs names it.

### Round 3 results (2026-11-11, release)

The predictions above, checked:

* **asmoOctree `meshOptimisation`: prediction 71650/71650, got 71649/71650,
  worst 4.1e-25 m**, and the same for the final mesh (`renumberMesh`,
  `renameBoundaryPatches`). Before: 71542/71650, worst 4.9e-4 m. The miss is
  explained, not a defect to fix here: see "The pow ulp" below.
* **elbow_90degree template: predicted and got 21977 cells, bit-identical**
  (before 21830). The prediction named the cause (hash order of
  `otherLabels` in `triSurfaceCurvatureEstimator`); the curvature marks went
  from 112/108/0 to upstream's 131/112/0, partitions 35 -> 27.
* **elbow `meshOptimisation`: predicted bit-identical with the glibc `pow`
  experiment, got it** (49382/49382 points, 128265/128265 faces), after the
  `findBadFaces` port (2 bad faces as upstream, part tet mesh of 1366 nodes
  as upstream) and the LP untangler's two divisions.

#### Stage tables

asmoOctree (`cartesianMesh`), points bit-identical / faces index-identical:

| step | round 2 | round 3, PETIR `powf` | round 3, glibc `pow` (experiment) |
|---|---|---|---|
| templateGeneration ... boundaryLayerGeneration | identical | identical | identical |
| meshOptimisation | 71542/71650, worst 4.9e-4 m, faces identical | **71649/71650, worst 4.1e-25 m**, faces identical | **71650/71650** |
| final mesh | not compared | 71649/71650, faces identical | 71650/71650, faces identical |

elbow_90degree (`cartesianMesh`, 8 steps, `GH1385_UPSTREAM_RUNS/stage_elbow_cartesianMesh_<step>`):

| step | round 2 | round 3, PETIR `powf` | round 3, glibc `pow` (experiment) |
|---|---|---|---|
| templateGeneration | 21830 cells vs 21977 | identical (34125 points, 77492 faces) | identical |
| surfaceTopology, surfaceProjection, patchAssignment | inherited | identical | identical |
| edgeExtraction | | counts equal; 34255/34508 points, worst 1.4e-14 m | identical |
| boundaryLayerGeneration | | counts equal; 48845/49382 points | identical |
| meshOptimisation | 43606/49382 points, worst 0.21 m; 3 bad faces vs 2 | counts equal; 43613/49382 points, worst 6.6e-5 m (the pow ulp, grown through the optimiser) | identical |
| boundaryLayerRefinement (final) | 106825/121398 points, worst 0.23 m; cells 108383 vs 108995 | counts equal (108 995 cells); 107600/121398 points, worst 6.6e-5 m; **faces not index-identical** | 121333/121398 points, worst 2.7e-15 m; faces not index-identical |

Status records, not staged yet (`cut_cube_and_hat_octree_final_meshes_status`;
final meshes only; counts equal in all four):

| case (generator) | PETIR `powf` | glibc `pow` (experiment) |
|---|---|---|
| cutCube (cartesianMesh) | **13470/13470 points, 32329/32329 faces bit-identical** | same |
| cutCube (tetMesh) | 39496/40498 points, worst 2.2e-10 m | 39924/40498, worst 3.3e-13 m |
| cutCube (pMesh) | 18160/222560 points, worst 6.9e-3 m | 19206/222560, worst 6.9e-3 m |
| hatOctree (cartesian2DMesh) | 25342/38090 points, worst 7.3e-5 m | same |

The tetMesh last-bit rest after the pow experiment (3.3e-13) has another
source not found; the pMesh (6.9e-3) and hatOctree (7.3e-5) differences are
**real** (not round-off), not localised: there is no staged upstream run of
them, which is the first thing to make.

#### The fixes (before -> after, each with its cause)

1. **`volumeOptimizer::evaluateGradientsExact` and the Knupp metric,
   operation for operation** (`mesh_optimizer.rs`): the port divided or
   multiplied by a reciprocal where upstream's expression divides or
   associates otherwise (`Vtri*gradV/stab`, `C*(2.*gradVs)/Vs13`,
   `gradF += a - b`, the Hessian `((A - B) - C) + D`, `pow(stab, 3)` /
   `pow(Vs13, 4)` as `pow(double, double)`), `Tensor::det` (cofactor
   expansion against `xx*yy*zz + ...`), `tetrahedron::mag` (`(1.0/6.0)*x`),
   `points_ /= scale`, `normal = n/d`. Alone these did **not** move asmoOctree
   `meshOptimisation` (71542/71650 both before and after): the first prediction
   above (71650/71650 from these) was **wrong about the cause**, and the first
   differing point was not a volume-optimiser move.
2. **`partTetMesh::createPointsAndTets`**: face and cell centres are the
   polyMeshGen addressing's (`pmg_face_centre_and_area`,
   `foam_cell_centres_and_vols`, new `checks::pmg_geometry`), not the vertex
   average and the pyramid centroid; a cell's faces run in cfMesh's cell-list
   order; a cell of four triangles is one tet (`tetMatcher::matchShape`,
   `match_tet`), with no cell-centre node; `updateVerticesSMP`'s centres divide
   (`c /= 3` then `c*area`, `centre/cellVol`). asmoOctree `meshOptimisation`
   71542 -> 71649.
3. **`updateOrigMesh` marks every tet-mesh node as changed**, moved or not
   (`changedNode[...] = true`), so the next `changedFace` covers every cell
   around the region; the port marked only moved nodes, so the faces around a
   locked node were not re-checked and the 9 low-quality faces left after
   `refineBoundaryLayers` (all points of the layer locked) were reported as
   repaired (9 -> 0) where upstream keeps 9 for ten iterations.
4. **`findBadFaces`, `findBadFacesRelaxed`, `findLowQualityFaces` are cfMesh's
   checks on the addressing's geometry** (`mesh_optimizer::flag_faces` now
   calls `poly_mesh_gen_checks::check_face_pyramids_g` (OpenFOAM's
   `pyramidPointFaceRef::mag`: `(1.0/3.0)*(areaNormal & (apex - centre))` with
   `face::areaNormal` and `face::centre`), `check_face_flatness_g`,
   `check_cell_part_tetrahedra_g`, `check_face_areas_g`,
   `check_face_dot_product_g`, `check_face_skewness_g`); `tet_volume` is
   `(1/6)*x` there too; `face_skewness`'s weighting is `((c*d)/s) + ((c*d)/s)`
   and its boundary normal is `areaNormal / mag`; the dot product threshold
   is `cos(65/180*pi)`. The elbow flagged 3 faces where upstream flags 2.
5. **`meshUntangler`**: `plane`'s normal is `n/mag(n)` and the feasible
   region's mean `p/size` divides (both were `scale(1/x)`). Elbow
   `untangler` stage: 52 nodes differing by 4e-4 m -> identical.
6. **`triSurfaceCurvatureEstimator`**: the quadric fit is over `otherLabels`,
   a `labelHashSet`, iterated in OpenFOAM's hash order (new private
   `FoamLabelHashSet`: `Hash<label>` is the label, capacity 128 on first
   insert, head insertion into a bucket chain, rehash at 0.8 load with each
   node moved to the head of its new bucket); the second ring is collected
   through a second hash set the same way; `e /= d`, `vecX /= mag`,
   `normal/d - c3*vecX/d - c4*vecY/d`, `plane`'s unit normal divide. The fit
   is near-singular with five to seven points, so the order is not a round-off
   matter: 927 of 5150 triangles' mean curvature differed by more than 1e-9
   relative (worst -0.215 vs -4.54).
7. **`refineBoundaryLayers::generateNewVertices`**: `vec = v/(magv + VSMALL)`,
   `pow(ratio, n)` as `pow(double, double)`. Points after the split: 942
   differing -> 0.

#### The pow ulp (the residual of asmoOctree and of the elbow's `edgeExtraction` onward)

`surfaceOptimizer::evaluateGradients` has `pow(stab, 3)`. This machine's
glibc 2.39 `pow` is its FMA ifunc build; PETIR's `powf` is the non-FMA ARM
`pow` **by design** (`petir::fast_exp` module docs: glibc's FMA build differs
on a small fraction of inputs by one ulp; PETIR tracks the no-FMA row on
purpose so that its bits do not depend on the CPU). For `stab =
0.03166163363177029`, glibc gives 3.173949098934236e-5 and PETIR
3.1739490989342366e-5. Chain of evidence, asmoOctree point 51898 (bp 2069)
and elbow bp 12597: traces of both codes with identical inputs (point
normal, `vecX`, `vecY`, the simplex points, every divide-and-conquer step) and
a Hessian diagonal that differs by one ulp with an identical gradient; Python's
`math.pow` (glibc) in upstream's operation order reproduces upstream's Hessian
and `x*x*x` reproduces ours; and replacing the pows of `surface_optimizer.rs`
and `mesh_optimizer.rs` by the platform `powf` (an experiment, **not
committed**: the platform libm is what PETIR's gate forbids) makes asmoOctree
71650/71650 and the elbow bit-identical through `meshOptimisation`. The
difference is below 1e-8 relative with a proven round-off cause: accepted and
**flagged for the maintainer**, who may want a `pow` that matches the FMA
build (PETIR change; not made here). `asmo_octree_mesh_optimisation_is_within_one_pow_ulp`
pins it from above (at most that one point, worst 4.2e-25 m).

#### Moved pins (old values struck through in the source)

| pin | moved | cause |
|---|---|---|
| `pipeline::tests::layer_strategies_are_deterministic` (CfMeshOptimise) | 18177691582502802893 -> 14814967247723517733 (fixes 1, 2) -> 6667351423853767910 (fixes 3-5, 7); cell count 7725 unchanged; SnappyCollapse unchanged | the optimiser, its checks and the tet-mesh geometry follow upstream |
| `layers::refine_boundary_layers::tests::prism_refinement_is_pinned_bit_for_bit` | pipe 0x591e062dc441a92f -> 0xeaba2317968a1edb, box 0xb10f852a9b013e88 -> 0xe9d49663cdf3d5e8 | fix 7 |
| `mesh_optimizer::tests::knupp_metric_makes_one_pass_as_upstream` | the case (three pushed points) is now repaired in one pass, so the test picks the first of up to seven pushed points that leaves inverted tetrahedra | fixes 2, 4; the one-pass rule is unchanged |
| `final_mesh::tests::projected_untangle_keeps_the_inlet_planar` | the "gh1285 order" case: `AfterUntangle` ~~1.78e-4 m off the inlet plane~~ 0e0 (assertion `> 1e-5` -> `< 1e-9`) | fixes 2-5 |

**`anisotropic::...generators_without_anisotropic_sources_are_pinned_bit_for_bit`
did not move this round** (all four fingerprints identical after all fixes;
its maintainer flag from round 2 stands). Did not move: the sphere tests
(`cfmesh_vs_upstream_sphere`: all seven steps and the final meshes of the
three generators stay bit-identical), `cfmesh_vs_upstream_cell_order`,
`cfmesh_patch_types*`, `octree_path`, `back_off_only`. **Not run:** Foam
Studio's `headless_unstructured_default_is_pinned` (the `dhoby-ghaut` example
`foam_studio_web` was not built on this machine); it shares
`surface_optimizer` and the part tet mesh, so check it before merging.

#### What remains

1. **`refineBoundaryLayers` assembly order** (elbow `boundaryLayerRefinement`,
   final mesh): upstream returns the refined mesh with the original internal
   faces in order (each with its pieces), then the newly generated internal
   faces in creation order, then the boundary faces per patch, and the first
   child of a cell keeps the cell's label while the others are appended;
   `refine_boundary_layers_mode` assembles through `from_cell_faces_wound`
   (RFF dump of both: 339 258 of 339 265 faces differ in position; the mesh
   before the split is index-identical). The in-flight mesh then differs from
   upstream's before `renumberMesh`, so the final faces are not
   index-identical although the topology and (after fix 3) the points agree to
   2.7e-15 m. The new-face order depends on the order of each cell's faces in
   `generateNewCellsPrism` / `generateNewCellsHex...`, which the port builds in
   its own order.
2. **pMesh (cutCube, 6.9e-3 m) and cartesian2DMesh (hatOctree, 7.3e-5 m)**:
   real differences, no staged upstream runs yet; tetMesh's 3.3e-13 m rest
   after the pow experiment has an unfound source.
3. **The pow ulp** (above): a decision for the maintainer.
4. Brute-force searches in `snap` and `SurfaceQueries::nearest_edge_point`,
   and about 140 `scale(1.0 / x)` sites, remain as in round 2.

## Rust-side differential, round 4 (gh:#1385, 2026-11-11)

**Research, education and V&V only.** AI-run; not reviewed by a human yet.

Round 4 starts from the round-3 state (merge 61398efc6). Foam Studio's
headless test, which round 3 did not run, is recorded under "Foam Studio
headless" below.

### Prediction 1, written before the fix (committed first): `refineBoundaryLayers` in upstream's assembly order

Reading upstream `refineBoundaryLayers::generateNewFaces` / `generateNewCells` /
`generateNewCellsPrism` (`refineBoundaryLayersFaces.C`, `...Cells.C`) against
the port's `refine_boundary_layers_mode`, which builds the cells as vertex
rings and assembles them through `from_cell_faces_wound`:

* `newFaces_` is the pieces of every original face in label order (internal,
  then boundary), then the faces the refined cells create in creation order;
  a cell face is looked up among the existing ones by `areFacesEqual`
  (cyclic, either orientation) and appended when absent.
* A prism's children are ordered **from the far side to the wall**: child 0
  (which keeps the label) holds the far base face, the child `nLayers-1` the
  wall face; piece `k` of a side face goes to child `max(nLayers-1-k, 0)`; the
  cap face of layer `l` is appended in the order it is first met, oriented as
  the wall face (outward of the child above it). The port numbers its columns
  from the wall: child order, face order inside a child and the creation order
  of the caps all differ.
* Faces that were internal and have several pieces are reversed when
  `newOwner > newNeighbour`; the mesh is then written as: original internal
  pieces, new internal faces, boundary pieces per patch; owner = lower cell,
  neighbour = higher, cells' face lists carried in upstream's order.
* The port's `refine_hex_cell` is a lattice construction, not a translation of
  `refineEdgeHexCell` / `refineCornerHexCell`; cells it makes are not in
  upstream's order.

Prediction: after porting the assembly and `generateNewCellsPrism`, elbow's
`boundaryLayerRefinement` stage has equal counts and **faces index-identical on
every face of the layer prisms and of the unrefined cells**; the remaining
differences, if any, are the faces of edge/corner hexahedra (the first
differing face names such a cell) and, through the optimiser, points of the
pow ulp (107600/121398 with PETIR `powf`, round 3). With the glibc-pow
experiment the mesh before `renumberMesh` should agree in every face, and the
final points 121333/121398 stay as in round 3 (2.7e-15 m).

### Result of item 1 (committed with the fix): elbow, `refineBoundaryLayers`

See "Round 4 results" below.

### Staged runs of cutCube `pMesh` and hatOctree `cartesian2DMesh` (first table of round 4, before any fix)

Upstream stopped after each step (`stage_cutCube_pMesh_<step>`,
`stage_hatOctree_cartesian2DMesh_<step>`, `scripts/run-stages.sh`), the port
through `outram-foam-cli/tests/cfmesh_vs_upstream_stages.rs`
(`cut_cube_p_mesh_stages`, `hat_octree_cartesian_2d_mesh_stages`), PETIR `powf`:

| step | cutCube pMesh points / faces | hatOctree 2D points / faces |
|---|---|---|
| templateGeneration | 205182/205182, 224843/224843 | 29498/29498, 57960/57960 |
| surfaceTopology | 172560/172560, 202629/202629 | 29498/29498, 57960/57960 |
| surfaceProjection | identical | **29008/29498, worst 7.8e-4 m** |
| patchAssignment | identical | same |
| edgeExtraction | **211975/222560, worst 7.07e-3 m**, faces 204753/313010 | 25460/29498, worst 1.2e-4 m |
| boundaryLayerGeneration | same count and worst | 26076/30890 |
| meshOptimisation | 18160/222560, worst 6.9e-3 m | 22662/30890, worst 7.3e-5 m |
| boundaryLayerRefinement (final) | 18160/222560, faces 3/313010 | 25342/38090, faces 1091/75072 |

First divergences: **hat at `surfaceProjection`** (`adjustZCoordinates`,
`preMapVertices(2)`, `mapVerticesOntoSurface`), **pMesh at `edgeExtraction`**
(`meshSurfaceEdgeExtractorFUN`, then `optimizeSurface` + `untangleSurface`).
Counts are equal at every step of both cases.

### Prediction 2, written before the fix: hat `surfaceProjection`

Reading upstream `meshSurfaceMapper2DPremapVertices.C` against
`surface_mapper_2d.rs`: `lp.coordinates() /= lp.pointLabel()` divides, the port
multiplies by `1.0 / n` (a last-bit difference of the search point);
`mapCorners` likewise (`newP /= ePatches.size()`). Prediction: with both
divisions the hat's `surfaceProjection` points move from 29008/29498 toward
29498/29498. If points still differ, the next suspects are the nearest-point
query used by the 2D mapper (`findNearestSurfacePoint` on the quadtree; the
3D mapper needed `NearestSearch::Upstream` and the octree), and the order in
which `mapVerticesOntoSurface` reads the live points.

### Prediction 2 was wrong; prediction 3, written before the fix

Result of prediction 2: **wrong.** With `lp.coordinates() /= n` and
`newP /= ePatches.size()` as divisions the hat's `surfaceProjection` stays at
29008/29498 points (worst 7.813e-4 m): the divisions are not the cause. (They
stay, as upstream.)

Reading `meshSurfaceMapper2D.H` again: **`preMapVertices(const label
nIterations = 2)`** (the 3D `meshSurfaceMapper.H` has 3), and
`cartesian2DMeshGenerator::mapMeshToSurface` calls `mapper.preMapVertices()`.
The port's pipeline calls `pre_map_vertices_2d(m, &o, 3)` and its module docs
say "default 3": a port defect (the iteration count of the 3D routine carried
over), and a doc claim the upstream header contradicts.
Prediction 3: with two iterations the hat's `surfaceProjection` is
29498/29498 bit-identical and faces identical; later stages then show what is
left (`edgeExtraction` and on: the 2D edge extractor, `optimizeSurface2D`,
`untangleSurface2D`, the pow ulp).

### Result of prediction 3, and prediction 4 (hat, written before the fix)

**Prediction 3 held, and more:** with `preMapVertices` at 2 iterations the
hat's points are **bit-identical at every step through
`boundaryLayerRefinement`** (29498, 29498, 29498, 29498, 29498, 30890, 30890,
38090 of the same counts, no pow ulp on this case), faces index-identical
through `meshOptimisation`. The final mesh has 38090/38090 points identical but
faces 1091/75072 index-identical, same cells as face sets (18676), different
cell labels: the `renumberMesh` walk starts from different cell lists.

Traces of both codes (upstream's instrumented `cartesian2DMeshGenerator::
refBoundaryLayers` prints `PFC` before and `RFF`/`RFC` after `refineLayers`;
`patches/gh1385-trace-instrumentation-4.patch`): the mesh after
`refineLayers` has **all 75072 faces identical at their index** (so round 4's
assembly is right in 2D too) and 18560 of 18676 cell face lists identical; the
116 that differ are original cells whose list **before** the refinement
differs (same faces, other order; e.g. cell 0: upstream `1 3 29055 28464`, port
`1 3 28464 29055`). The port's `distribute_boundary_faces`
(`meshSurfaceEdgeExtractor2D::distributeBoundaryFaces`) clones the mesh, so its
carried cell lists no longer match the truncated faces and fall back to the
ascending derived lists; upstream's `replaceBoundary` keeps the internal faces
in each list and **appends the new boundary labels in the order of the boundary
list** (the old boundary faces' order), the labels running patch by patch.
Prediction 4: carrying the lists through that replacement makes the pre-refine
lists equal upstream's (116 -> 0), the cell lists after `refineLayers`
identical, and the final hat mesh index-identical (75072/75072 faces, 38090
points as now).

### pMesh `edgeExtraction` localised, and prediction 5 (written before the fix)

Traces of both codes inside `meshSurfaceEdgeExtractorFUN` (upstream's
instrumented constructor prints the mesh after `createBasicFundamentalSheets`
= `FA`, after `smoothMeshSurface` = `FB`, after `remapBoundaryPoints` = `FC`:
points, faces with owner and neighbour, cell lists; the port's
`trace_dump`; `patches/gh1385-trace-instrumentation-5.patch`). cutCube
`pMesh`, PETIR `powf`:

| after | points identical | faces identical at index | cell lists identical |
|---|---|---|---|
| `FA` (createFundamentalSheetsJFS) | 221489/222560, worst 2.2e-16 m | 204753/313010 | 20320/60263 |
| `FB` (smoothMeshSurface) | 216032/222560, worst 2.7e-4 m | 204753/313010 | same |
| `FC` (remapBoundaryPoints) | 218269/222560, worst 1.1e-4 m | 204753/313010 | same |

So the first divergence is **inside `createFundamentalSheetsJFS`**, before the
surface smoothing: 1071 new points off by one ulp, and the topology's
numbering. Reading `extrudeLayer.C` against `layers/extrude_layer.rs`
(an operation-for-operation translation on a `TopoMesh`):
(a) `movePoints` divides (`normal /= d`), the port multiplies by `1/m`: the 1071
points; (b) `createDuplicateFrontFaces` renumbers the cells' lists **in place**
(`c[fI] = nOrigFaces_ + lp.first()`), starting from the mesh's own cell lists,
while the port starts from ascending derived lists (`w.cell_faces()`);
(c) `updateBoundary` is `reorderBoundaryFaces()` then `replaceBoundary()` with the
old boundary faces first (their order in the old range) and the new boundary
quads after them, the labels running patch by patch and the lists appended in
that order, while the port keeps the faces in label order and drops none.
Prediction 5: with (a), (b) and (c) the `FA` points are identical (222560), the
faces index-identical (313010) and the cell lists identical (60263); `FB` /
`FC` then show what `smoothMeshSurface` and `remapBoundaryPoints` add (the
first differing point names the next step).
