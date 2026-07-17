# outram-blender — Scaffold Review Manifest

> **⚠️ AI-GENERATED DRAFT — HUMAN REVIEW REQUIRED per `RESPONSIBLE_USE.md`.**
> Everything in the `outram-blender` crate was produced by an AI fleet (lead +
> subagents) and is **untrusted draft material until a human reviews it**. It
> compiles and its tests pass, but that is verification of the *scaffold*, not
> validation that the design is the right one. Do not describe this crate as
> validated or trusted until the maintainer clears both axes of the README's
> `## Bookkeeping status` block.

## What this crate is

A new workspace member: a pure-Rust, headless **mesh-authoring frontend** that
borrows Blender's mesh/geometry **architecture** (not its code) to eventually
author and procedurally generate geometry for the OUTRAM PARK solvers. This is a
**scaffold**: a compiling crate skeleton, real primitive generators, honest
stubs elsewhere, and a Blender dependency map.

## Real vs stub — the honest breakdown

| Module | State | What is actually there |
|---|---|---|
| `math` | **REAL** | `Vec3` with add/sub/scale/dot/cross/length/normalize; 3 unit tests |
| `mesh` | **REAL** | Index-based half-edge (`VertexId`/`EdgeId`/`LoopId`/`FaceId`); `add_vertex`, `add_face` with **automatic edge dedup**, `face_vertices`, `euler_characteristic`; 2 unit tests |
| `primitives` | **REAL** | `cube`, `uv_sphere`, `cylinder`, `grid` — build valid meshes; 5 unit tests assert exact counts + Euler characteristic (chi=2 closed, chi=1 disc) |
| `export::triangulate` | **REAL** | Fan-triangulation of any mesh into an indexed triangle soup; 1 unit test |
| `ops` | **STUB** | `MeshOp` enum (Extrude/Subdivide/Bevel/Boolean) + dispatch; every `apply` returns `NotImplemented` |
| `modifiers` | **STUB** | `Modifier` enum (Subsurf/Mirror/Array) + `ModifierStack`; empty stack is identity, non-empty returns `NotImplemented` |
| `procedural` | **STUB** | `GeometryGraph` / `GeometryNode` sketch; `evaluate` returns `NoOutput` or `NotImplemented` |
| `export::to_polymesh_surface` / `to_csg_primitive` | **STUB** | Document the `outram-foam-mesh` polyMesh and `outram-mc-libs` CSG target interfaces; return `NotImplemented` |

**No fake-green.** Every stub returns a typed `NotImplemented` error and has a
test asserting exactly that — no stub pretends to work.

## Build / test / Android — measured output

Run in the isolated worktree on 2026-07-17 (host: Arch, rustc stable):

- `RUSTFLAGS="-D warnings" cargo build -p outram-blender --release` → **clean, no warnings**.
- `cargo test -p outram-blender --release` → **17 unit tests + 1 doctest, all pass** (0 failed).
- `cargo run -p outram-blender --example authoring_primitives --release` → prints V/E/F/chi for cube, uv_sphere, cylinder, grid; all Euler-characteristic assertions pass.
- `RUSTFLAGS="-D warnings" cargo check -p outram-blender --target aarch64-linux-android` → **clean** (pure-Rust, only `thiserror`; no BLAS/C/GUI).
- `cargo metadata --no-deps` → OK (workspace not broken).

## Dependency map summary

`docs/blender-dependencies.md` audits Blender's `versions.cmake`
(commit `9f5c3edcf34bea02589fa09fc2ce6830ffe4acdf`, 2026-07-02), ~120 library
entries in 9 purpose-groups. Verdict: only a small handful are relevant to a
mesh-authoring frontend, and all as **reimplement-the-concept** or **small
pure-Rust crate**, never as native ports:

- geometry math → `glam`/`nalgebra` (vs Eigen/Imath); exact predicates → `robust`/`num-bigint` (vs GMP);
- half-edge topology → **no dependency** (reimplement BMesh concept, done here);
- subdivision → reimplement Catmull-Clark (concept of OpenSubdiv);
- boolean/CSG → reimplement robust booleans (concept of Manifold);
- decimation → `meshopt` or reimplement (meshoptimizer);
- mesh I/O → `obj-rs`/`tobj`, `ply-rs`, `stl_io`, `gltf` (not even Blender deps).

Everything else (rendering, color, media, audio, GPU/windowing, XR, text
shaping, scripting, USD/Alembic/MaterialX, C/C++ infra) is **out of scope** and
mostly Android-hostile — which is precisely why avoiding it keeps this crate
pure-Rust and Android-buildable.

## Provenance & licensing

- Crate license: **GPL-3.0-only** (workspace default).
- Blender is **GPLv2-or-later** → **GPLv3-compatible**. At scaffold stage
  **nothing is ported** — only architecture/concepts, which carry no copyright
  obligation. Any future literal port of a Blender (or dependency) algorithm
  must add the upstream attribution header block and pass a GPLv3 license
  re-check, per the workspace provenance rule.

## ⚠️ Naming / trademark flag (needs a maintainer decision)

The crate is named **`outram-blender`** at the maintainer's explicit request.
The workspace convention (bead `op-ahi`) names independent forks
`outram-park-fork-<project>` (e.g. `outram-park-fork-coolprop`), so this name
departs from convention. It is also a **Blender-Foundation trademark
adjacency** concern. The crate is **not affiliated with the Blender
Foundation**; "Blender" only identifies the upstream architecture. This is
flagged prominently in `README.md` ("Naming & trademark") and `Cargo.toml`.
**Decision required:** keep `outram-blender`, or rename to
`outram-park-fork-blender` / another name. Tracked in bead `op-hzs.8`.

## Human-verify checklist (for the maintainer)

1. **Topology design** — is the simplified half-edge (single incident loop per
   vertex, no radial cycle around edges yet) the right foundation, or should the
   full BMesh radial links be added before building operators on top? (`op-hzs.1`
   depends on this.)
2. **Primitive correctness** — spot-check winding/normals (Euler characteristic
   is checked, but consistent outward winding is only argued in comments, not
   asserted).
3. **Math crate choice** — keep the in-crate `Vec3`, or adopt `glam`/`nalgebra`
   (adds a workspace dep)? (`op-hzs.8`.)
4. **Export target interfaces** — confirm the described `outram-foam-mesh`
   polyMesh boundary-patch route and `outram-mc-libs` CSG primitive-fitting route
   match those crates' current APIs before wiring (`op-hzs.6`, `op-hzs.7`).
5. **Naming/trademark** — resolve `outram-blender` vs the `op-ahi` convention
   (`op-hzs.8`).
6. **Bookkeeping axes** — both V&V and human-interface axes are ❌; clear them
   only after personal review.

## Beads filed (epic `op-hzs`)

`op-hzs.1` half-edge mesh ops · `op-hzs.2` Catmull-Clark subdivision ·
`op-hzs.3` boolean/CSG operator · `op-hzs.4` modifier stack ·
`op-hzs.5` procedural evaluator · `op-hzs.6` polyMesh export bridge ·
`op-hzs.7` CSG export bridge · `op-hzs.8` dependency + math-crate + naming decisions.

> Note: the beads live in the local Dolt store. The passive
> `.beads/issues.jsonl` export was **skipped** by `bd` because that file already
> contains 119 records from other fleets not yet imported to this worktree's
> Dolt store — a pre-existing cross-fleet sync divergence. Left untouched to
> avoid clobbering other fleets' work.
