# gh:#1470 (partial): snappyHexMeshDict reader and `snappyHexMesh` binary

**Upstream:** OpenFOAM-dev `852527377c7e` `snappyHexMesh.C`,
`refinementParameters.C`, `snapParameters.C`, tutorial `flange`.
Code: `src/snappy_hex_mesh/dict.rs`,
`crates/outram-foam-cli/src/bin/snappy_hex_mesh.rs`.

Finding from reading upstream first: dev's keyword is `insidePoint` (ESI:
`locationInMesh`); both are read. Dev reads surfaces from `constant/geometry`.

## Method
Test `crates/outram-foam-cli/tests/snappy_hex_mesh_cli.rs`: blockMesh 8^3 unit
box, 24x24 sphere r = 0.3 STL, dev-shaped dict. Predictions (committed first):
cell count in (512, 32768); report names the unported `mergeTolerance` and
`features` and says layers were not added; `1/polyMesh` written, background
untouched.

## Results (2026-11-11, release)
All three predictions held. Run through the binary: 3152 cells, 5470 points,
11748 faces; castellated count equals final count (snap moved points, not the
count). Dict reader pin: every keyword of a flange-style dict is honoured or
listed (`flange_style_dict_is_fully_accounted`).

## Not done (still open on #1470)
`#includeEtc` and `.cfg` defaults (parser limit), searchable shapes in
`geometry`, `cellLevel`/`pointLevel` output, `mergeTolerance`, `-checkGeometry`,
`-surfaceSimplify`, accelerated surface search, and the real `flange` /
`motorBike` dictionaries as pins (the dev tutorial ships no `flange.stl` in
`constant/geometry`, only a README, and its dict needs `refinementRegions`,
`features` and curvature behaviours that are not ported).
