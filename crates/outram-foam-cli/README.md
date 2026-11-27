<!--
SPDX-License-Identifier: AGPL-3.0-only
Part of Outram Park (outram-park-backend). OpenFOAM-style CLI tools in Rust.
Independent fork, not the official OpenFOAM software; see TRADEMARKS.md.
-->

# outram-foam-cli

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. For research, education and V&V only. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

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

**Name.** `outram` is the workspace's OUTRAM: **O**pen-source **U**nified
**TR**Ansient **M**ulti-Physics (from OUTRAM PARK, `docs/ecosystem-naming.md`);
`foam` marks the OpenFOAM lineage; `cli` is the command-line front end.
**The question it answers:** can the OpenFOAM workflow (mesh, set fields, run a
solver, from a case directory) be driven by the Rust ports, and do the files it
writes match what OpenFOAM itself writes?

**What this crate lets you study and challenge.** It is the thin layer where an
OpenFOAM case directory meets the ports: each binary is a short file in
`src/bin/` that reads the case's dictionaries and calls one library function,
so a reader can see exactly which part of a case the port honours and which it
ignores. The files it writes have been diffed byte for byte against ESI
OpenFOAM-v2506's own utilities, and the record keeps the misses (84 of 449
tutorial `blockMeshDict`s differ, 113 the port cannot parse) with their
causes. The solvers that cannot yet be run from a case say so and refuse.

## Knowledge in this crate

See the workspace [vision](../../VISION.md).

| Knowledge artifact | In this crate |
|---|---|
| Scientific literature | Upstream programs it mirrors: OpenFOAM's `blockMesh`, `ideasUnvToFoam`, `polyDualMesh`, `snappyHexMesh` (OpenFOAM-dev `852527377c7e`, per `src/bin/snappy_hex_mesh.rs`), `setFields`, `topoSet`, `pimpleFoam`, `rhoCentralFoam`; cfMesh's `cartesianMesh`, `tetMesh`, `pMesh`, `cartesian2DMesh`; GeN-Foam (`gen-foam`). The comparison build is ESI OpenFOAM-v2506 (GPL-3.0). No `crates/kovan-literature/CATALOGUE.md` entry is tied to this crate. |
| Mathematical equations | None here: the equations live in the library crates the binaries call (`outram-foam-mesh`, `outram-park-fork-cfmesh`, `outram-foam-appbuilder-lib`). |
| Source code | `src/bin/` (one file per binary), `src/cfmesh.rs` (the cfMesh `meshDict` front end), `src/set_fields.rs`, `src/topo_set.rs`. |
| Interactive simulations | None yet. The binaries are terminal tools (table below). |
| Visualisations | None yet. |
| Verification and validation evidence | [`verification_and_validation/cli_output_vs_openfoam_gh1363.md`](verification_and_validation/cli_output_vs_openfoam_gh1363.md) (2026-11-12): `blockMesh -flavour esi` byte-identical on the cavity and a two-zone block; 449 tutorial `blockMeshDict`s give 213 identical, 84 differ, 113 the port cannot parse, 39 ESI cannot run as copied; `polyDualMesh` 20 of 20 files; `pimpleFoam` on the cavity with tight tolerances byte-identical in U, phi and `uniform/time`, p except one line; `ideasUnvToFoam` 0 of 8 whole cases. [`verification_and_validation/set_fields_topo_set_extrude_gh1384.md`](verification_and_validation/set_fields_topo_set_extrude_gh1384.md): `setFields` 4 of 4 files and `topoSet` 6 of 6 set files byte-identical to ESI v2506. Tests: `tests/esi_output_parity_gh1363.rs`, `tests/set_fields_vs_esi_gh1384.rs`, `tests/cfmesh_vs_upstream_stages.rs`, `tests/mesh_tools.rs`, `tests/solvers.rs`. |
| Human reviews and corrections | 0 functions carry a kovan human-review stamp (no `kind = "review"` block in any `review.md` here). No entry in `docs/human-corrections-to-ai-work.md` names this crate. |

**Status:** the written files of `blockMesh`, `polyDualMesh`, `pimpleFoam`,
`setFields` and `topoSet` have AI-run byte comparisons against ESI v2506; the
cfMesh binaries are compared stage by stage with upstream cfMesh `e944f6ba695f`
through this crate's entry point (`tests/cfmesh_vs_upstream_stages.rs`, record
in `crates/outram-park-fork-cfmesh/verification_and_validation/upstream_cfmesh_gh1385/`),
and `snappyHexMesh` end to end (record in
`crates/outram-foam-mesh/verification_and_validation/snappy_hex_mesh_1470/`);
`rhoCentralFoam` has no CLI-level comparison; `rhoPimpleFoam`, `sonicFoam` and
`gen-foam` are stubs. Nothing here has been reviewed by a human.

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
| `snappyHexMesh` | `outram-foam-mesh` (snappy port) | ✅ live (surface + `blockMesh` background → polyMesh; gh:#1470) — row added 2026-11-27 |
| `cartesianMesh`, `tetMesh`, `pMesh`, `cartesian2DMesh` | `outram-park-fork-cfmesh` via `src/cfmesh.rs` | ✅ live (`system/meshDict` + `surfaceFile` → polyMesh; gh:#1476) — rows added 2026-11-27 |
| `setFields` | `src/set_fields.rs` | ✅ live (`system/setFieldsDict`; `-flavour esi`; gh:#1384) — row added 2026-11-27 |
| `topoSet` | `src/topo_set.rs` | ✅ live (`system/topoSetDict` cellSets; gh:#1384) — row added 2026-11-27 |

**CORRECTED 2026-11-27** — the table listed 7 binaries; `Cargo.toml` declares
16 (the rows above were added after checking each `src/bin/*.rs`).

The CLI accepts OpenFOAM's single-dash options (`-case cavity`) like upstream.
Wiring tracked under the `op-x8x` beads epic; the stub prerequisites are beaded
separately.
