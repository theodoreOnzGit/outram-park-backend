# Barycentric particle tracking vs OpenFOAM.com v2506 (gh:#1382, epic #1336)

> Research, education and V&V only (`RESPONSIBLE_USE.md`). Not for operating, controlling
> or licensing a nuclear facility.

Subject: `outram_foam_mesh::particle_tracking` (`src/particle_tracking.rs`), a port of the
core of `Foam::particle` (`src/lagrangian/basic/particle`, `liblagrangian`): the
`tetBasePtIs` decomposition, `locate`, `track`, `trackToFace`, `trackToStationaryTri`,
`changeTet`/`changeFace`/`changeCell`. Upstream is the specification; the rest of the
Lagrangian tree is **not** ported (see "Not ported").

## What was searched first

`lagrangian`, `particle`, `Cloud`, `barycentric`, `trackToFace`, `parcel`, `injection`, `drag`
across all crates. Found and rejected: `outram-park-fork-liggghts` (LIGGGHTS DEM: contact
mechanics, no carrier-fluid coupling, does not track through an `fvMesh`; read only,
untouched), `outram-mc-libs` (CSG neutron tracking), `outram-foam-mesh::poly_dual_mesh` and
`dynamic_mesh` (no particles), `sembawang`/`buangkok` (box models and Gaussian puffs named
"Lagrangian" in prose, no mesh tracking). Reused: `check_mesh::esi_geometry` (cell centres and
`cells()`, bit-identical to `polyMesh`), `check_mesh::esi_tet_quality` and
`esi_tet_decomposition_min_quality` (made `pub(crate)`: visibility only, body unchanged; the
folder's `review.md` holds no review stamp), `esi_snap` vector helpers, the
`outram-foam-basic-lib` polyMesh reader.

## Method

Meshes: `warped3d` (hex, 120 cells, 434 faces) and `dual_warped3d` (polyhedral: 665 four-,
144 five- and 24 six-vertex faces) of `tests/fixtures/esi_checkmesh/`. Per mesh a committed
script (`tests/fixtures/esi_particle_tracking/<mesh>.script`, 876 commands, 84 particles,
seeded LCG, written by the ignored test `regenerate_scripts`) is run by both codes:

* ESI: probe `trackParticles/` (`wmake`d by stage `lagrangian` of
  `scripts/upstream/build-esi-v2506-and-genfoam.sh`, at most 2 jobs; `run_esi.sh` runs it),
  which calls `particle(mesh, position, celli)`, `reset()`, `track()`, `trackToFace()` and
  prints the full tracking state at `%.17g`;
* Rust: `tests/particle_tracking_esi_v2506.rs` replays the script and compares every printed
  token (17 columns per command) by IEEE-754 bit pattern against `<mesh>.esi.out`.

Script groups per mesh: 0) 40 interior particles, 4 full legs of 0.05-2 cell lengths;
1) 12 particles whose given `celli` is a neighbour of the true cell (`locate`'s `minF`
fallback); 2) 12 with 5-30 cell-length tracks (boundary hits, then legs from the boundary);
3) 12 with three 0.25-fraction sub-steps and a `trackToFace`; 4) 8 starting at a cell centre
(the `VSMALL` perturbation) or an internal face centre. ESI is built `-O3`, no
`-ffast-math`, no `-march`; rustc does not contract to FMA; tracking uses only `+ - * /`
and `sqrt`.

Pass criterion: all 17 columns bit-identical (no tolerance).

## Results (2026-11-14, ESI v2506 patch 0, liblagrangian from `lib-lagrangian_basic`)

| Quantity | warped3d | dual_warped3d | worst difference |
|---|---|---|---|
| returned fraction (track/trackToFace) | 876/876 identical | 876/876 | 0 |
| barycentric coordinates a, b, c, d | 876/876 each | 876/876 each | 0 |
| `celli`, `tetFacei`, `tetPti`, `facei` | 876/876 each | 876/876 each | 0 |
| `stepFraction` | 876/876 | 876/876 | 0 |
| `behind`, `nBehind` | 876/876 each | 876/876 each | 0 |
| position x, y, z | 876/876 each | 876/876 each | 0 |

17 columns x 876 lines x 2 meshes = 29,784 values, **all bit-identical**, on the first
comparison run. The ESI output is frozen in `<mesh>.esi.out`; the test does not need ESI.
Mutation check: swapping the `reflect()` pair `(c, d)` for `(b, d)` makes the test fail
(708/876 returned fractions, 162/876 `y_b` identical), so the comparison can fail.

Coverage of the run (from the ESI output): 396 `track`/`trackToFace` lines per mesh; 186 / 182
end on a boundary face; 13 of 84 `new` particles are moved to another cell by `locate`
(all 12 group-1 particles); the polyhedral mesh exercises 5- and 6-vertex faces and
`changeFace` with nonzero base points.

## Predictions (`PREDICTIONS.md`, committed before any run) vs outcome

P1-P4 (tet indices, cell/face, coordinates, fractions, both meshes): held, 100 %.
P5 (group 1 relocated): held, 12 of 12 per mesh change cell and agree with ESI.
P6 (`behind`/`nBehind` zero in >= 95 % of lines): held trivially, 100 % are zero; **this
means the `detA <= 0` branch and the `MAX_N_BEHIND` stuck path were never exercised**, so
they are ported but not verified by this run.
P7: first half held (group 2 first legs on a boundary face: 12/12 on both meshes). The second
half **missed**: group 0 first legs ended in the cell (`facei == -1`) in 33/40 (hex) and 25/40
(polyhedral), not >= 90 %. Cause: the mesh is small (cells ~0.2 across) and legs of up to two
cell lengths in a random direction reach the boundary from cells next to it. The prediction was
wrong about the script, not about the port.

## Not ported / not verified (child issues of #1382)

`celli < 0` location (`mesh.cellTree().findInside`, indexedOctree), moving-mesh tracking
(`trackToMovingTri`, `cubicEqn`), coupled patches in `findFaceBasePts`, parallel transfer,
`Cloud` and `Cloud::move`, injection models, kinematic parcel forces (SphereDrag, gravity,
dispersion), `lagrangianIntermediate` and the `kinematicParcelFoam`-equivalent, boundary
interaction models (`patchInteractionModel`), `detA <= 0` and stuck-particle paths (need a
mesh with a bad tet and a script that provokes it).
