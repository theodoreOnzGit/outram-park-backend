# Vendored upstream sources (development reference only)

This directory holds the upstream projects that `outram-park-fork-cfmesh` ports
from or references. **The clones themselves are gitignored** (see the crate
`.gitignore`) — they are present for development/porting reference only, are
**never committed** and **never packaged** (`Cargo.toml` `exclude`). This README
is the small, committed provenance record.

GPLv3-compatible upstreams only (workspace vendor rule).

## Clone commands

```bash
# cfMesh (GPL-3.0-only) — primary port target: Cartesian / tet / polyhedral
# meshing with boundary layers. Layout: meshLibrary/{cartesianMesh,
# cartesian2DMesh, tetMesh, utilities}, utilities/, tutorials/.
git clone --depth 1 https://github.com/wyldckat/cfMesh.git upstream_source/cfMesh

# cfMesh, DAFoam line (GPL-3.0-or-later) — the line the newer ports cite
# (docs/upstream_coverage.md) and the one BUILT AND RUN for gh:#1385.
git clone https://github.com/DAFoam/cfmesh.git upstream_source/cfmesh_dafoam
git -C upstream_source/cfmesh_dafoam checkout e944f6ba695f

# voro++ (modified BSD, LBNL) — 3D Voronoi tessellation, the polyhedral-dual
# (polyDualMesh-style) reference.
git clone --depth 1 https://github.com/chr1shr/voro.git upstream_source/voro
```

## Provenance snapshot

| Upstream | Commit | Date | Licence |
|---|---|---|---|
| cfMesh (`wyldckat/cfMesh`) | `6cf1d211f9cfc8f358e6c8dfccb3fb7503357572` | 2014-06-21 | GPL-3.0-only |
| cfMesh (`DAFoam/cfmesh`), cloned 2026-11-11 | `e944f6ba695fcefce73ad306074f6b3c0ea432ae` | 2026-02-17 | GPL-3.0-or-later (file headers) |
| voro++ (`chr1shr/voro`) | `b0dac575a47af0f90b5b100e6dc199a493c7cb83` | 2026-03-04 | modified-BSD (LBNL) |

## Building and running upstream cfMesh (gh:#1385, 2026-11-11)

`cfmesh_dafoam` at `e944f6ba695f` was built against a partial
OpenFOAM-v2506 (ESI, GitHub mirror `ZhangYanTJU/OpenFOAM-ESI`, tag
`OpenFOAM-v2506`, dummy Pstream, no MPI, no ThirdParty) outside the
repository, after a three-part API patch. The recipe, scripts, patch, logs
and every run's output are in
`../verification_and_validation/upstream_cfmesh_gh1385/` (`README.md` there,
section "Status and how to resume"). In short:

```bash
# OpenFOAM: OpenFOAM, fileFormats, surfMesh, meshTools, finiteVolume,
# blockMesh, extrudeModel, dynamicMesh, checkMesh (55 min on 3 cores)
scripts/build-of.sh
git -C upstream_source/cfmesh_dafoam apply \
  ../verification_and_validation/upstream_cfmesh_gh1385/patches/cfmesh-e944f6ba695f-openfoam-v2506.patch
scripts/build-cfmesh.sh   # meshLibrary + cartesianMesh, cartesian2DMesh, tetMesh, pMesh
```

See the crate `NOTICE` for the full provenance and licensing statement. Any file
ported into `src/` carries an upstream provenance header block per the workspace
provenance rule.
