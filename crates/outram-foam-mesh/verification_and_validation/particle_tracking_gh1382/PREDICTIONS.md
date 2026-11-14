# gh:#1382 predictions, committed BEFORE any ESI or Rust comparison run

Date: 2026-11-14. Subject: `outram_foam_mesh::particle_tracking` (barycentric tracking on a
static `polyMesh`: `tetBasePtIs`, `particle::locate`, `track`, `trackToFace`,
`trackToStationaryTri`, `changeTet/Face/Cell`) against OpenFOAM.com v2506 `Foam::particle`
(`liblagrangian`), probed by `trackParticles` (this directory).

Cases: meshes `warped3d` (hex, 120 cells) and `dual_warped3d` (polyhedral) of
`tests/fixtures/esi_checkmesh/`; one committed script per mesh
(`tests/fixtures/esi_particle_tracking/<mesh>.script`, seeded LCG, 5 groups: 40 interior
particles with 4 full legs; 12 whose given cell is a neighbour of the true one; 12 with
long 5-30 cell-length tracks that hit the boundary; 12 with 0.25-fraction sub-steps and a
`trackToFace`; 8 starting exactly at a cell centre or a face centre). 882 script lines per
mesh. Every printed quantity is compared by IEEE-754 bit pattern: barycentric coordinates,
`celli`, `tetFacei`, `tetPti`, `facei`, `stepFraction`, `behind`, `nBehind`, position, the
returned fraction.

Reasoning shared by all predictions: the port repeats the upstream expression trees
operation for operation (dot `x*x+y*y+z*z`, cross, `cmptSum` left to right,
`BarycentricTensor::inner`, `Foam::max(0, y)` as `0 > y ? 0 : y`). ESI is built `-O3`
without `-ffast-math` or `-march`, so no FMA contraction or reassociation; rustc never
contracts. No libm call is made by tracking (only `sqrt`, IEEE-exact; `cbrt` is used only
by the script generator, before the ESI run).

| # | Quantity | Prediction |
|---|---|---|
| P1 | `tetFacei`, `tetPti` (hence `tetBasePtIs`, which only these expose) after every command, both meshes | 100 % identical. Risk: faces whose first base point is rejected (quality <= 1e-30) on the polyhedral mesh; `esi_tet_quality` is already bit-identical (gh:#1364), so none expected. |
| P2 | `celli`, `facei` after every command, hex mesh | 100 % identical. |
| P3 | Same, polyhedral mesh (faces with more than four vertices, `changeFace` base-point shifts) | 100 % identical. The one place a divergence would show first is `changeFace` on a face whose base point is not 0. |
| P4 | barycentric coordinates, `stepFraction`, position, returned fraction, both meshes | 100 % bit-identical. If a residual appears, expected cause: 1-ulp differences from a different association of `fraction*muH*detA`; localised and replayed before accepting. |
| P5 | Group 1 (given cell is wrong): `locate` ends in the true cell for >= 90 % of the 12 particles per mesh via the `minF` fallback `track`, and ESI and the port agree on every one, including those ending on a boundary face. | |
| P6 | `nBehind` and `behind`: both are 0 in >= 95 % of all output lines; nonzero values (a tet with `detA <= 0` or a retreat) are reproduced exactly where they occur. | |
| P7 | Group 2 (long tracks): >= 80 % of first legs end on a boundary face (`facei >= nInternalFaces`) with `stepFraction < 1`; group 0 first legs end in a cell (`facei == -1`, `stepFraction == 1`) for >= 90 %. Independent of ESI: coordinates are non-negative and sum to 1 within 1e-9 on every line. | |

Not attempted this session (child issues): `celli < 0` location through
`mesh.cellTree().findInside` (indexedOctree), moving-mesh tracking (`trackToMovingTri`,
`cubicEqn`), coupled patches, `Cloud`/injection/parcel sub-models, kinematic parcel
Stokes drag.
