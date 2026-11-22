# cfMesh's anisotropic workflow in the four generators: V&V record (gh:#1279 remainder)

**Research, education and V&V only.** Not for reactor operation, licensing or
safety decisions (`RESPONSIBLE_USE.md`). AI-written port, not yet reviewed by
a human.

**Upstream:** `github.com/DAFoam/cfmesh` at `e944f6ba695f` (cfMesh v1.1
line): `cartesianMeshGenerator.C`, `tetMeshGenerator.C`,
`cartesian2DMeshGenerator.C`, `voronoiMeshGenerator.C` (the `pMesh`
executable), and `utilities/anisotropicMeshing/`.

**Port:** `pipeline::{cartesian_mesh, tet_mesh, p_mesh, cartesian_2d_mesh}`
(the wiring), `crate::anisotropic` (the modifier, ported earlier in
gh:#1279).

## What upstream does (read before predicting)

All four generators' **constructors** do the same thing: with
`anisotropicSources` in `meshDict`,
`surfaceMeshGeometryModification(surf, meshDict).modifyGeometry()` makes
the modified surface (`modSurfacePtr_`), and the octree is built on **it**
(`meshOctree(*modSurfacePtr_)`, `createOctreeBoxes`). Every step up to the
revert works in the modified space, against that octree.

Where the mesh goes back to real space differs:

| generator | revert (`polyMeshGenGeometryModification::revertGeometryModification`) | after the revert |
|---|---|---|
| `cartesianMesh` | end of `optimiseFinalMesh` (step `meshOptimisation`), after `optimizeMeshFV`, `optimizeLowQualityFaces`, `optimizeBoundaryLayer(modSurfacePtr_ == nullptr)` (so `false` on this pass), `untangleMeshFV` | `projectSurfaceAfterBackScaling`: new octree on the **original** surface, `createOctreeWithRefinedBoundary(20, 30)`, `meshSurfaceMapper::mapVerticesOntoSurface` (nearest point, every boundary point), then `optimiseFinalMesh` **again** (now with `modSurfacePtr_` null: no second revert, `optimizeBoundaryLayer(true)`) |
| `tetMesh` | end of `optimiseFinalMesh` (same place) | `projectSurfaceAfterBackScaling`, as `cartesianMesh` |
| `pMesh` (`voronoiMeshGenerator`) | end of `optimiseFinalMesh` (same place) | `projectSurfaceAfterBackScaling`, as `cartesianMesh` |
| `cartesian2DMesh` | inside `generateBoundaryLayers`, **after** `addLayerForAllPatches` | new quadtree on the original surface (`meshOctree(surf, true)`, `createOctreeWithRefinedBoundary(20)`, i.e. 15 triangles per leaf), `mapEdgesAndCorners` (`remapBoundaryPoints`), `optimiseMeshSurface`; the later steps use the new quadtree. **No `projectSurfaceAfterBackScaling`** in 2D. |

Conditions: the revert is guarded by `modSurfacePtr_` (set only by the
constructor), `projectSurfaceAfterBackScaling` by
`meshDict_.found("anisotropicSources")`. Both are outside the
`runCurrentStep` checks only in the sense that they sit inside the step that
calls them: a run stopped before `meshOptimisation` (3D) or
`boundaryLayerGeneration` (2D) returns a mesh **still in the modified
space**, and a 2D restart that resumes after `boundaryLayerGeneration`
keeps the constructor's modified quadtree for its later steps. The port
keeps both, as upstream.

`projectSurfaceAfterBackScaling` is a **generator** method, not a
`meshOptimizer` / `meshSurfaceOptimizer` one (grep of the whole tree: it
exists only in the three 3D generators). `meshSurfaceMapper::
mapVerticesOntoSurface` (`meshSurfaceMapperMapVertices.C`) is
`findNearestSurfacePoint` for every boundary point, no patch, no limit: the
crate's `snap::snap_to_surface_with(.., SurfaceSearch::Octree)` (gh:#1263,
bit-identical to the brute-force nearest point) is that.

The search octrees: the port's surface searches (`SurfaceQueries`, the
surface optimiser, `snap`) take the surface's points and triangles, not
the octree, so the octree that upstream rebuilds in
`projectSurfaceAfterBackScaling` has no counterpart in 3D; the points are
what changes. In 2D the quadtree **is** what the 2D steps read, so it is
rebuilt as upstream.

## Predictions (written 2026-11-05, before the wiring was run)

- **W0.** Without `anisotropicSources` every generator's output is
  bit-identical to the code before the wiring
  (`anisotropic::tests::generators_without_anisotropic_sources_are_pinned_bit_for_bit`,
  fingerprints committed before the wiring), and the named pins and the
  three restart tests hold.

Setup S: sphere R 3 m, 24 x 48 UV (analytic 113.0973 m³, faceted surface
−0.712 %), `maxCellSize` 0.5, and one `boxScaling` "slab": centre (0, 0,
0), lengths (8, 8, 2), scales (1, 1, 0.5). The forward map stretches
`|z| <= 1` by 2 (and shifts everything above it by +2), so the octree's
cubes become 0.5 x 0.5 x 0.25 boxes in the slab once reverted.

- **W1 (`cartesian_mesh`).** Every hexahedron touching no boundary point
  and lying wholly in the slab has `dz / dx = 0.5` to 1e-9; every one
  wholly outside it (`|z| >= 1`) has `dz / dx = 1` to 1e-9. Straddling
  cells lie between. The slab holds 54.45 m³ of the sphere's 113.1 m³, so
  the modified sphere is 1.48 times as large: about 1.48 x 1 128 ≈ 1 670
  cells (±20 %).
- **W2.** Closed (`validate`), 0 non-positive cells.
- **W3.** Volume error against 113.0973 m³ within 1 percentage point of the
  isotropic control run (−1.938 % recorded in gh:#1274).
- **W4.** After the run every boundary point is within 1 % of the cell size
  (5 mm) of the original surface: the projection puts them on it and the
  second `optimiseFinalMesh` keeps them there unless it finds bad faces.
- **W5 (`tet_mesh`).** Same setup. Cells touching no boundary point: the
  ratio of the mean z-extent to the mean x-extent of the cells wholly in
  the slab is 0.5 ± 5 %; of those wholly outside it, within 5 % of the same
  ratio on the isotropic control run. Closed, 0 non-positive. Volume error
  within 1 percentage point of the isotropic control.
- **W6 (`p_mesh`).** Same setup: closed, 0 non-positive; the z/x extent
  ratio in the slab 0.5 ± 10 % (the Voronoi cells are less regular).
- **W7 (`cartesian_2d_mesh`).** The channel ribbon `[0, 1] x [0, 0.25]`
  at 0.05 with a `boxScaling` centre (0.25, 0.125, 0), lengths (0.5, 1,
  1), scales (0.5, 1, 1): `x <= 0.5` is stretched by 2. Interior cells
  wholly in `x <= 0.5` have `dx / dy = 0.5` to 1e-9, the others `1`.
  Closed, 0 non-positive, one cell thick, area 0.25 to 1e-9 (the walls are
  straight, so mapping onto them is exact).
- **W8.** An anisotropic `cartesian_mesh` run stopped after
  `edgeExtraction` (mesh still in the modified space) and restarted gives
  the uninterrupted run's mesh bit for bit.

## Results (2026-11-05)

Tests: `anisotropic::tests::{generators_without_anisotropic_sources_are_pinned_bit_for_bit,
anisotropic_sources_through_the_generators, interior_hexes_stay_cubes_until_mesh_optimisation,
draw_anisotropic_images}` (the last `--ignored`; it writes the PNGs here).
All runs `--release`.

**Summary.** The wiring works: all four generators produce closed meshes
with no non-positive cells. Inside the scaled region the cells are
compressed by close to the predicted 0.5. The volume errors are within
0.2 percentage points of the isotropic controls, and an anisotropic
restart reproduces the full run bit for bit. Three predictions missed:

- **W1, exact ratio.** The predicted *exact* `dz / dx` was wrong because
  the optimiser moves interior points, in the control too.
- **W4, distance to the surface.** Boundary points end up 8 mm from the
  surface: the final `optimizeSurface` smooths without mapping, as
  upstream does.
- **W5, cells outside the slab.** The tet cells outside the slab are 8 %
  flatter than in the control, not within 5 %.

### Sphere, setup S

Measures:

- "rings 1": cells touching no boundary point. This is the measure the
  predictions named.
- "rings 2": also sharing no point with such a cell. I added it **after**
  seeing the rings-1 numbers, because the rings-1 cells next to the
  `cartesianMesh` layer carry points the projection moved. It is a
  second instrument, not a replacement. Both are reported.
- `cartesianMesh` rows count hexahedra only.
- The ratio is (mean z-extent) / (mean x-extent) of the cells' bounding
  boxes, with the per-cell min..max.
- "In slab": bounding box within `|z| <= 1`. "Outside": wholly at
  `|z| >= 1`.
- Volume error is against 113.0973 m³. The boundary distance is the
  largest distance from a boundary point to the faceted surface.

| generator | run | rings | cells | volume error | non-positive | closed | in slab: n, ratio, min..max | outside: n, ratio, min..max | max boundary distance |
|---|---|---|---|---|---|---|---|---|---|
| cartesian_mesh | isotropic | 1 | 1128 | -1.938 % | 0 | yes | 176, 1.025476, 0.930..1.384 | 256, 0.958000, 0.706..1.104 | 2.891e-3 |
| cartesian_mesh | slab | 1 | 1624 | -1.733 % | 0 | yes | 512, 0.578277, 0.492..0.776 | 272, 0.847058, 0.706..1.094 | 8.021e-3 |
| cartesian_mesh | isotropic | 2 | 1128 | -1.938 % | 0 | yes | 80, 1.006043, 0.991..1.074 | 40, 0.985237, 0.930..1.006 | 2.891e-3 |
| cartesian_mesh | slab | 2 | 1624 | -1.733 % | 0 | yes | 272, 0.560450, 0.510..0.660 | 40, 0.818340, 0.795..0.843 | 8.021e-3 |
| tet_mesh | isotropic | 1 | 12840 | -1.362 % | 0 | yes | 3647, 0.979785, 0.469..2.071 | 3079, 0.957088, 0.470..2.201 | 5.4e-16 |
| tet_mesh | slab | 1 | 18264 | -1.321 % | 0 | yes | 7008, 0.522009, 0.232..1.223 | 3804, 0.881759, 0.347..2.241 | 6.3e-16 |
| tet_mesh | isotropic | 2 | 12840 | -1.362 % | 0 | yes | 2367, 0.983622, 0.491..2.000 | 1269, 0.941283, 0.500..2.027 | 5.4e-16 |
| tet_mesh | slab | 2 | 18264 | -1.321 % | 0 | yes | 4648, 0.519430, 0.242..1.207 | 1728, 0.845880, 0.377..2.015 | 6.3e-16 |
| p_mesh | isotropic | 1 | 1749 | -1.484 % | 0 | yes | 379, 0.972164, 0.820..1.138 | 414, 1.044432, 0.878..1.228 | 4.575e-3 |
| p_mesh | slab | 1 | 2537 | -1.367 % | 0 | yes | 927, 0.513625, 0.411..0.635 | 414, 0.958654, 0.769..1.206 | 8.327e-3 |
| p_mesh | isotropic | 2 | 1749 | -1.484 % | 0 | yes | 231, 0.984681, 0.914..1.003 | 140, 1.033193, 1.001..1.096 | 4.575e-3 |
| p_mesh | slab | 2 | 2537 | -1.367 % | 0 | yes | 571, 0.513988, 0.444..0.608 | 140, 0.919197, 0.838..1.058 | 8.327e-3 |

What the reports show:

- **Back-scaling.** `revertGeometryModification` moved every point (2 005
  cartesian, 3 711 tet, 14 288 pMesh). The largest projection move onto
  the original surface was 1.5e-2 m (cartesian), 1.8e-2 m (tet) and
  1.8e-2 m (pMesh).
- **The second `optimiseFinalMesh`.** It found 0 bad faces in all three
  generators, so the volume optimiser moved nothing.

### Channel, W7

The 2D measure is (mean x-extent) / (mean y-extent) of the cells two
rings from the non-empty patches. The face area is the volume divided by
the thickness 0.10308.

| run | cells | area error | non-positive | closed | `x <= 0.5`: n, dx/dy, min..max | `x >= 0.5`: n, dx/dy, min..max |
|---|---|---|---|---|---|---|
| isotropic | 120 | +2.2e-16 | 0 | yes | 16, 1.172734, 1.133..1.226 | 14, 1.178494, 1.138..1.226 |
| half box | 180 | +2.2e-16 | 0 | yes | 34, 0.583282, 0.550..0.752 | 16, 1.138096, 0.944..1.224 |

The revert note: 434 points reverted, the new quadtree built, 4 corner
edges remapped, `untangleSurface2D` found 0 bad faces.

### Against the predictions

- **W0 held.** All four fingerprints match the pre-wiring values. The
  three restart tests pass, and so do the named pins (see "Pins" below).
- **W1 missed as stated.**
  - The cell count held: 1 624 cells against 1 670 ± 20 %.
  - The *exact* ratios did not hold. Hexahedra in the slab span 0.49–0.78
    (rings 1) and 0.51–0.66 (rings 2), with means 0.578 and 0.560, where I
    predicted 0.5 to 1e-9.
  - The cause is not the wiring. In the isotropic control the rings-2
    hexahedra are **exact cubes** after every step up to
    `boundaryLayerGeneration` (ratio 1.000000, min = max). They stop being
    cubes only in `meshOptimisation`: 0.930..1.074 afterwards. The test is
    `interior_hexes_stay_cubes_until_mesh_optimisation`. The optimiser's
    smoothing moves interior points, and that is upstream's design.
  - Divided by the control, the in-slab ratio is 0.560 / 1.006 = 0.557.
  - The cells wholly outside the slab measure 0.82 (rings 2), not 1.
    These 40 cells all sit near the poles, against the wall. My reading
    is that the second `optimiseFinalMesh`, in real space, smooths them
    toward their flattened neighbours. **Not checked**: it would need
    the mesh just before the second pass.
- **W2 held** for all three 3D generators: closed, 0 non-positive cells.
- **W3 held.** The cartesian slab run is −1.733 %, against −1.938 % for
  the control (+0.21 percentage points). Tet: −1.321 % against −1.362 %.
  pMesh: −1.367 % against −1.484 %.
- **W4 missed.**
  - The largest boundary-point distance is 8.0e-3 m (cartesian) and
    8.3e-3 m (pMesh), against the predicted 5e-3. The tet route gives
    6e-16.
  - The isotropic controls are off the surface too (2.9e-3 and 4.6e-3),
    so this is not the back-scaling.
  - Upstream's `meshSurfaceOptimizer::optimizeSurface`
    (`meshSurfaceOptimizerOptimizeSurface.C`, read for this) maps only
    the **edge** points (`mapEdgeNodes`). The surface points get
    `smoothLaplacianFC` and `smoothSurfaceOptimizer` with no mapping
    afterwards, so a smoothed point lies on the chord, off a curved
    surface.
  - The tet route's final `optimizeSurface` is
    `meshOptimizer::optimizeSurface`, which leaves the points on the
    surface.
  - The prediction was wrong about upstream's behaviour, not the port's.
    **Not checked:** whether the port's surface smoothing matches
    upstream's displacement for displacement.
- **W5 held in part.**
  - In the slab: 0.522 (rings 1) and 0.519 (rings 2), inside 0.5 ± 5 %.
  - Closed, 0 non-positive.
  - Volume: −1.321 %, against −1.362 % for the control.
  - **Outside the slab it missed.** The ratio is 0.882 against the
    control's 0.957 (−7.9 %, rings 1) and 0.846 against 0.941 (−10 %,
    rings 2), where I predicted within 5 %. This is the same flattening
    near the poles as in W1.
- **W6 held.** Closed, 0 non-positive. The slab ratio is 0.514, inside
  0.5 ± 10 %.
- **W7 held in part.**
  - Held: closed, 0 non-positive cells, area exact to 2.2e-16. Both
    meshes span z in [0, 0.10308].
  - **Not checked:** that every point lies exactly on z min or z max (the
    one-cell-thick check of gh:#1275).
  - **The exact ratios missed.** The isotropic control's own interior
    cells measure dx/dy = 1.17, not 1, so the instrument measures
    something besides the octree's cubes: the channel is 5–6 cells high,
    and its rings-2 cells carry points that the 2D surface optimiser
    moved. **Not investigated.**
  - Divided by the control: 0.583 / 1.173 = 0.497 in the stretched half
    and 1.138 / 1.178 = 0.966 in the other.
- **W8 held.** The run stopped after `edgeExtraction` reaches z = 4.9993:
  it is in the modified space, where the sphere's top is at 5. The
  restarted run equals the full run bit for bit.

### Pins

The full suite passed on 2026-11-05:
`cargo test --release -j 2 -p outram-park-fork-cfmesh --lib --tests` gave
234 passed, 0 failed, 19 ignored. That run includes these pins:

- `back_off_only_layers_are_pinned_bit_for_bit`
- `octree_path_is_pinned_bit_for_bit`
- `layer_strategies_are_deterministic`
- `correct_edges_is_pinned_bit_for_bit`
- `prism_refinement_is_pinned_bit_for_bit`
- `foam_studio_default_mesh_keeps_its_fingerprint`
- the tet, cartesian and 2D `*_stop_and_restart_reproduce_the_full_run`
- `generators_without_anisotropic_sources_are_pinned_bit_for_bit`

`dual_kinds_are_pinned_bit_for_bit` (`--features cell-centre-dual`) was
run separately and passed.

### Images (`draw_anisotropic_images`, `--ignored`)

Each image is an x-z slice at y = 0.05, drawn from the assembled mesh,
with the analytic sphere as a dashed circle. The channel images are the
x-y mid-plane.

- `sphere_xz_cartesian_mesh_isotropic.png` / `sphere_xz_cartesian_mesh_slab.png`
- `sphere_xz_tet_mesh_isotropic.png` / `sphere_xz_tet_mesh_slab.png`
- `sphere_xz_p_mesh_isotropic.png` / `sphere_xz_p_mesh_slab.png`
- `channel_xy_isotropic.png` / `channel_xy_half_box.png`

What I checked:

- In `sphere_xz_cartesian_mesh_slab.png` the band `|z| <= 1` holds 8 rows
  of half-height cells, where the control has 4 rows of square ones. The
  rows above and below are square near the axis.
- The boundary lies on the dashed circle all round, with no visible gap
  at the slab's edges (z = ±1).
- The vertical grid lines bow slightly: the optimiser's smoothing (W1).
- In `channel_xy_half_box.png` the left half (x < 0.5) has columns half
  as wide as the right half's, and the walls are straight.

- The tet and pMesh slab images show the same flattened band in
  `|z| <= 1`, and the boundary on the circle. These two were looked at,
  not measured from.

What I could not check from the images: anything out of the y = 0.05
plane.

## Defects and remainders

- No defect found in the port's anisotropic classes or the wiring.
- **Upstream quirks, kept:**
  - A run stopped before the revert returns a mesh in the modified space.
  - A 2D restart that resumes after `boundaryLayerGeneration` keeps the
    constructor's modified quadtree for `meshOptimisation` and
    `boundaryLayerRefinement`.
  - `optimizeBoundaryLayer(modSurfacePtr_ == nullptr)` is `false` on
    `cartesianMesh`'s first pass. It acts only with `optimiseLayer`
    (off), so it changes nothing here.
- **Not ported:**
  - The `meshDict` reader for `anisotropicSources`: the sources are
    structs.
  - `triSurfacePatchManipulator`: the constructors' feature-edge patches
    are still skipped.
- **Open:**
  - Why the cells outside the slab end up flattened (W1, W5); see the
    note in W1.
  - Why the 2D control's interior cells are not square (W7).
