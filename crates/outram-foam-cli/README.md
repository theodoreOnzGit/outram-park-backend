<!--
SPDX-License-Identifier: GPL-3.0-only
Part of Outram Park (outram-park-backend). OpenFOAM-style CLI tools in Rust.
Independent fork, not the official OpenFOAM software; see TRADEMARKS.md.
-->

# outram-foam-cli

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping pass" command). A crate is **complete** only once the maintainer has personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

OpenFOAM-style command-line utilities as **terminal binaries**, named exactly
like upstream so you run them from a case directory:

```bash
blockMesh -case cavity        # generate the mesh
pimpleFoam -case cavity       # run the solver
gen-foam -case reactor        # GeN-Foam neutronics + TH
```

Each binary reads/writes an OpenFOAM case via `outram-foam-basic-lib::io`, and
does the actual work in the library crates (`outram-foam-mesh` for meshing,
`outram-foam-appbuilder-lib` for the solvers). Independent OUTRAM PARK fork, not
the official OpenFOAM (see `TRADEMARKS.md`).

## Tools

| Binary | Backend | Status |
|---|---|---|
| `blockMesh` | `outram-foam-mesh::block_mesh` | ✅ live (dict → `constant/polyMesh`) |
| `ideasUnvToFoam` | `outram-foam-mesh::ideas_unv_to_foam` | ✅ live (`.unv` → polyMesh) |
| `polyDualMesh` | `outram-foam-mesh::poly_dual_mesh` | ✅ live (→ `constant/dualMesh`) |
| `rhoCentralFoam` | `outram-foam-appbuilder-lib::solvers::rho_central_foam` | ✅ live (built-in perfect-gas EOS; end-to-end Sod proof) |
| `pimpleFoam` | `outram-foam-appbuilder-lib::solvers::pimple_foam` | ✅ live (incompressible) |
| `rhoPimpleFoam` / `sonicFoam` | `…::solvers` | 🚧 stub — needs a `constant/thermophysicalProperties` reader (ψ/Cp/µ). Refuses to run rather than fake. |
| `gen-foam` | `outram-foam-appbuilder-lib::genfoam` | 🚧 stub — needs a case-driven GeN-Foam driver + `constant/nuclearData` reader |

The CLI accepts OpenFOAM's single-dash options (`-case cavity`) like upstream.
Wiring tracked under the `op-x8x` beads epic; the stub prerequisites are beaded
separately.
