# gh:#1465 step 1: surface-intersection refinement (`surfaceOnlyRefine`)

**Upstream:** ~~OpenFOAM-dev `852527377c7e87e33d51b19ca8974b5db60af28b`
(`snappyRefineDriver.C:179`, `meshRefinementRefine.C:908`,
`refinementSurfaces.C:410`, `hexRef8.C:1426/2127`)~~ **CORRECTED 2026-11-12**
(maintainer on gh:#1459, "ESI preferred"): OpenFOAM.com ESI v2506
(`snappyRefineDriver.C:342`, `meshRefinementRefine.C:945`,
`refinementSurfaces.C:1265`, `hexRef8.C:1567/2233`; same algorithm, re-read). Code:
`src/snappy_hex_mesh/refinement.rs`, selected by
`CastellationControls::with_surface_intersection_refinement()`; the default
stays the historical distance band so callers' pins do not move.

~~**Not run against upstream.** snappyHexMesh is not built here (see gh:#1364);
this is a port check against the algorithm, not a differential.~~ **CORRECTED
2026-11-12:** ESI v2506 `snappyHexMesh` is built (recipe in
`../snappy_hex_mesh_esi_v2506/README.md`) and the refinement stage is compared
with it, cell by cell, on the flange: identical (42788 of 42788 cells). The
sphere case below was NOT run through ESI (no STL of it was written); its
counts are a port check only.

## Methodology
Sphere r = 0.3 at (0.5, 0.5, 0.5), 32x32 UV tessellation, unit cube, 8^3
background, level 2, keep point (0.02, 0.02, 0.02) (outside the sphere).
Predictions were committed before the first run (test doc comment in
`refinement.rs`): (1) postconditions hold (no surface-crossing face segment
joins a cell below level 2; no face differs by more than one level); (2) leaf
count strictly between 512 and 512*64.

## Results (2026-11-11, release)
| rule | cells | by level 0/1/2 |
|---|---|---|
| distance band (old) | 2856 | 376 / 368 / 2112 |
| surface intersection (OpenFOAM rule) | 3360 | 328 / 728 / 2304 |

Predictions 1 and 2 held. No prediction was made for the band-vs-intersection
count; the intersection rule gives +504 cells (+17.6 %), mostly level 1, from
the 2:1 closure (compare the pictures). Pinned by
`castellate_modes_recorded_counts`.

Pictures: `castellation_distance_band.png`, `castellation_surface_intersection.png`
(z = 0.5 slice, cell level colour, sphere in red). Checked: refined cells hug
the sphere in both; the intersection rule has a level-1 buffer where level 2
meets level 0 (2:1); cell removal inside the sphere is clean. Not checked: 3D
faces off the mid-plane.

## Open
Curvature/`(min max)` levels, regions, features, gaps, nCellsBetweenLevels > 1,
cell-count caps, upstream differential (flange), baffle-based cell removal.
