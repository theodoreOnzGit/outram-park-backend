# upstream_source (outram-foam-mesh)

The upstream clones live here but are **gitignored** and never committed.
This README is the record.

| item | value |
|---|---|
| project | OpenFOAM-dev (OpenFOAM Foundation) |
| URL | https://github.com/OpenFOAM/OpenFOAM-dev.git |
| commit | `852527377c7e87e33d51b19ca8974b5db60af28b` ("thermophysicalTransportModels::Fickian: Updated the mixture-averaged diffusion coefficient formulation", committed 2026-11-03T11:32:50Z) |
| date cloned | 2026-11-11 |
| licence | GPL-3.0-or-later (OpenFOAM `COPYING`); compatible with this workspace (AGPL-3.0-only, combined work) |
| used for | blockMesh parity, gh:#1458 (children #1461, #1463); snappyHexMesh parity, gh:#1459 (children #1465, #1470, #1480, #1467) |

Reproduce (blobless sparse clone, a few MB):

```bash
git clone --filter=blob:none --no-checkout --sparse \
  https://github.com/OpenFOAM/OpenFOAM-dev.git upstream_source/OpenFOAM-dev
cd upstream_source/OpenFOAM-dev
git sparse-checkout set src/mesh/blockMesh \
  applications/utilities/mesh/generation/blockMesh \
  tutorials/incompressibleFluid/T3A tutorials/incompressibleFluid/pitzDailySteady \
  tutorials/mesh/blockMesh src/OpenFOAM/geometry/splines \
  src/OpenFOAM/primitives/Tensor src/OpenFOAM/primitives/Vector \
  src/OpenFOAM/primitives/polynomialEqns src/OpenFOAM/dimensionSet \
  src/OpenFOAM/unitSet etc/configDict
git checkout 852527377c7e87e33d51b19ca8974b5db60af28b
```

Files read for the ports: `blockEdges/{arcEdge,polyLineEdge,splineEdge,BSplineEdge,lineDivide,lineEdge,blockEdge}`,
`geometry/splines/{Spline,CatmullRomSpline,BSpline}`, `blockDescriptor/blockDescriptorEdges.C`,
`blocks/block/blockCreate.C`, `blockMesh/blockMeshTopology.C`, `unitSet/units.C`, `etc/configDict` (units table),
`primitives/Tensor/TensorI.H` (`det`, `inv`, `&`).

## snappyHexMesh and checkMesh sparse set (gh:#1459, #1480)

Same clone and commit `852527377c7e87e33d51b19ca8974b5db60af28b`; add these
paths to the sparse set (verified 2026-11-11, 33 MB checked out):

```bash
git clone --filter=blob:none --no-checkout --sparse \
  https://github.com/OpenFOAM/OpenFOAM-dev.git upstream_source/OpenFOAM-dev
cd upstream_source/OpenFOAM-dev
git sparse-checkout set src/mesh/snappyHexMesh \
  applications/utilities/mesh/generation/snappyHexMesh \
  src/meshCheck src/OpenFOAM/meshes/primitiveMesh \
  etc/caseDicts/mesh/generation
git checkout 852527377c7e87e33d51b19ca8974b5db60af28b
```

Note: in this commit the mesh-quality checks live in `src/meshCheck/`
(`checkMesh.C`, `polyMeshCheck/polyMeshCheckQuality.C`,
`primitiveMeshCheck/primitiveMeshCheck.C`, namespace `meshCheck::`), not in
`primitiveMeshTools`. `meshCheck::boundaryFaceSkewness` projects `Cf - C` onto
the face normal (floor `0.4 |d|`); it does not mirror the owner cell.
