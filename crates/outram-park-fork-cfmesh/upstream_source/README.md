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

# voro++ (modified BSD, LBNL) — 3D Voronoi tessellation, the polyhedral-dual
# (polyDualMesh-style) reference.
git clone --depth 1 https://github.com/chr1shr/voro.git upstream_source/voro
```

## Provenance snapshot

| Upstream | Commit | Date | Licence |
|---|---|---|---|
| cfMesh (`wyldckat/cfMesh`) | `6cf1d211f9cfc8f358e6c8dfccb3fb7503357572` | 2014-06-21 | GPL-3.0-only |
| voro++ (`chr1shr/voro`) | `b0dac575a47af0f90b5b100e6dc199a493c7cb83` | 2026-03-04 | modified-BSD (LBNL) |

See the crate `NOTICE` for the full provenance and licensing statement. Any file
ported into `src/` carries an upstream provenance header block per the workspace
provenance rule.
