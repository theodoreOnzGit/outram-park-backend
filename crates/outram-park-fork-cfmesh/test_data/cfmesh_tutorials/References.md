# cfMesh tutorial inputs (gh:#1476)

Upstream cfMesh tutorial inputs, copied verbatim so the Rust port and
upstream cfMesh can be run on the **same case** (gh:#1385). Used by
`src/mesh_dict.rs`'s tests (every tutorial's `meshDict` reads with no
unhandled keyword) and by the `outram-foam-cli` binaries' end-to-end runs.

## Source

- **Project:** cfMesh v1.1 line as carried by DAFoam,
  <https://github.com/DAFoam/cfmesh>
- **Commit:** `e944f6ba695fcefce73ad306074f6b3c0ea432ae` (2026-02-17)
- **Authors:** Franjo Juretic, Creative Fields, Ltd. (tutorial `meshDict`
  banners); DAFoam maintainers for the repository.
- **Licence:** GPL-3.0-or-later (upstream `README.md`: "either version 3 of
  the License, or (at your option) any later version"). Public, open
  source; no restricted or confidential data.
- **Accessed:** 2026-11-11, from a read-only clone of the repository.
- **Processing:** none. Files are byte-for-byte copies (MD5 below).

## Files

| path here | upstream path | MD5 |
|---|---|---|
| `cartesianMesh/asmoOctree/system/meshDict` | `tutorials/cartesianMesh/asmoOctree/system/meshDict` | |
| `cartesianMesh/asmoOctree/geom.stl` (3.0 MB) | `tutorials/cartesianMesh/asmoOctree/geom.stl` | `36bc0582a9d2dae6c735aed94ab2cd39` |
| `cartesianMesh/elbow_90degree/system/meshDict` | same path upstream | |
| `cartesianMesh/elbow_90degree/elbow_90degree.stl` (1.4 MB) | same path upstream | `f8dcf156eebbe7d7e3da9d3dc94524a4` |
| `cartesianMesh/multipleOrifices/system/meshDict` | same path upstream | |
| `cartesianMesh/singleOrifice/system/meshDict` | same path upstream | |
| `cartesianMesh/singleOrifice/singleOrifice.stl` (0.8 MB) | same path upstream | `7f356dd708075cf08eb9396e4eb1a0a8` |
| `cartesianMesh/ship5415Octree/system/meshDict` | same path upstream | |
| `pMesh/multipleOrifices/system/meshDict` | same path upstream | |
| `pMesh/multipleOrifices/multipleOrifices.stl` (3.2 MB) | same path upstream | `7aa435eecd6a09b38a3cb138ea8def2f` |
| `cartesian2DMesh/hatOctree/system/meshDict` | same path upstream | |
| `cartesian2DMesh/hatOctree/geom.fms` (334 B) | same path upstream | `a637a1545f7b27a19ff4f5206ea4f6bd` |
| `tetMesh/cutCubeOctree/system/meshDict` | same path upstream | |
| `tetMesh/cutCubeOctree/geom1.stl` (6 KB) | same path upstream | `2e0903c9131a8e9b6db8a6baf8032a01` |

## Not copied, and why

- **`cartesianMesh/multipleOrifices/multipleOrifices.stl`** (3.3 MB, MD5
  `e06f8146c889229d6421360384706307`): the same file as
  `pMesh/multipleOrifices/multipleOrifices.stl` with CRLF line ends
  (identical after `tr -d '\r'`, checked 2026-11-11). The cartesianMesh case
  is run on the pMesh copy; the ASCII STL reader splits on any white space,
  so the line ends do not change what is read.
- **`cartesianMesh/ship5415Octree/ship5415-hull-and-box.stl`** and
  **`ship5415-hull.stl`** (7.3 MB each): over the ~5 MB limit for crate test
  data. Their `meshDict` is here; the case's `surfaceFile` is
  `ship5415-hull-and-box.ftr`, which upstream's `Allrun` *generates* with
  `surfaceFeatureEdges ship5415-hull-and-box.stl ship5415-hull-and-box.ftr`,
  so it is not in the repository either. Running the ship case needs those
  STLs placed beside the `meshDict` (or under `reference-data/`) and the FTR
  generated.
- Every other tutorial (`bunnyOctree`, `intakePortOctree`, `sBendOctree`,
  `sawOctree`, `pMesh/bunnyPoly`, `tetMesh/socketOctree`): not on gh:#1476's
  list.
