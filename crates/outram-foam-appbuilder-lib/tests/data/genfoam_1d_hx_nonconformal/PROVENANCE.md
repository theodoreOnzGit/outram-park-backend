# Provenance: `featureCases/1D_HX/onePhase` with the secondary exchanger zone refined (README round 44, gh:#1847)

- **Derived from** `tests/data/genfoam_1d_hx_onephase` (the shipped case, commit 652b3da of GeN-Foam, GPL-3.0, (C) 2015-2022 EPFL).
- **Only change:** `system/fluidRegion/blockMeshDict`, block `sHX`: `(1 1 20)` -> `(1 1 30)`, and the mesh regenerated with
  `blockMesh -region fluidRegion` (OpenFOAM-ESI v2506) by `scripts/make_hx_variants.sh`. The primary and secondary exchanger zones keep
  equal volumes (the one condition `heatExchanger.C` checks) and their meshes become non-conformal, so the `meshToMesh` weights are
  fractional. Justified from `heatExchanger.C` ("the two meshes of the heat exchanger could be non-conformal") and the shipped
  tutorial's comment (the zones "can have any relative displacement").
