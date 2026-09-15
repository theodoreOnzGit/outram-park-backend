# outram-blender — Mesh-Implementation Fleet Review Manifest

> ## ⚠️ HISTORICAL SNAPSHOT (superseded) — dated 2026-07-17
>
> This manifest records one AI-fleet pass as it stood on **2026-07-17**. Several
> "PARTIAL" / "not implemented" entries below have since been closed and are
> **no longer current** — the mesh **boolean** now does general union /
> difference / intersect on non-convex closed meshes, the **vertex bevel**
> rounds for `segments >= 2`, **CSG cylinder fitting** and the **faceted /
> DAGMC** export route both landed (the latter with a closed-2-manifold gate),
> `wgpu` is compiled unconditionally on desktop (there is no `gpu` cargo
> feature any more), and the crate has since gained end-to-end Monte Carlo
> (`sim` / MC Studio) and volume-meshing (`foam_mesh` / Mesh Studio) paths.
> The "Algorithms & provenance" claim that *no* upstream source is ported is
> likewise superseded: `src/boolean_predicates.rs` is now a literal, attributed
> port of Blender's `math_boolean.cc` (see `NOTICE`). For the crate's actual
> current status and module map, read the **README.md**; this file is kept as a
> record of that pass, not as a description of the present code.

> ## ⚠️ UNTRUSTED AI-GENERATED DRAFT — HUMAN REVIEW REQUIRED
>
> **Every line of the code described here was produced by an AI fleet (a lead
> agent plus six parallel subagents) and is untrusted draft material until a
> human reviews it**, per the workspace `RESPONSIBLE_USE.md` / `AI_USAGE.md`.
> It compiles, and its unit tests pass, but that is *verification of the
> implementation against hand-derived topology*, **not** validation that the
> algorithms are correct for all inputs or that the design is the right one.
> Do **not** describe this crate as validated or trusted until the maintainer
> personally clears both axes of the README `## Bookkeeping status` block.
> Not for nuclear facility operation, reactor control, safety-critical, or
> licensing decisions.

## What this pass did

Turned the `outram-blender` **operator/modifier/procedural/export stubs** into
real, tested code. Before this pass only `math`, `mesh`, `primitives`,
`transform`, and `export::triangulate` were real; everything else returned a
typed `NotImplemented`. This pass implements the mesh-editing verbs and the two
solver export bridges, and adds two new modules (`subdivision`, `boolean`).

Fleet layout (one distinct file per subagent, no write collisions; the lead
added shared `mesh` helpers up front and integrated + compiled):

| Subagent | File | Epic bead |
|---|---|---|
| A | `src/ops.rs` | `op-hzs.1` |
| B | `src/subdivision.rs` (new) | `op-hzs.2` |
| C | `src/boolean.rs` (new) | `op-hzs.3` |
| D | `src/modifiers.rs` | `op-hzs.4` |
| E | `src/procedural.rs` | `op-hzs.5` |
| F | `src/export.rs` | `op-hzs.6`, `op-hzs.7` |
| lead | `src/mesh.rs` helpers, `src/lib.rs`, `examples/mesh_operators.rs` | — |

## Per-module: real vs partial

| Module | State | What actually landed | Tests |
|---|---|---|---|
| `mesh` (helpers) | **REAL** | Added `positions`, `polygons`, `from_polygons`, `face_normal` (Newell), `face_centroid` — the polygon-soup view every operator builds on. | 2 (pre-existing) |
| `ops` | **REAL (bevel partial)** | `extrude_faces` (region cap + boundary side-walls), `extrude_edges`, `subdivide` (simple midpoint, no smoothing), `bevel_vertices` (vertex truncation). `MeshOp::{Extrude,Subdivide,Bevel,Boolean}` wired. **Partial:** multi-segment (rounded) bevel `segments >= 2` falls back to a single chamfer (documented; follow-up bead). | 7 |
| `subdivision` (new) | **REAL** | Catmull-Clark surface subdivision with local stencils (face/edge/vertex points; interior + boundary/crease rules). All-quad output. **Caveat:** non-manifold edges (>2 faces) and boundary vertices without exactly two boundary edges use documented fallbacks, not validated cases. | 5 |
| `boolean` (new) | **PARTIAL (honest)** | Convex-mesh **Intersect** via half-space (Sutherland-Hodgman) clipping + cap reconstruction → closed convex manifold. **Not implemented:** `Union`, `Difference` (return `Unsupported`), and general non-convex Intersect (heuristic convexity rejection). Follow-up bead. | 6 |
| `modifiers` | **REAL** | `Mirror` (reflect + winding-flip + seam weld), `Array` (N relative-offset copies, unwelded), `Subsurf` (wraps `subdivision::catmull_clark`). `ModifierStack` folds in order. | 6 |
| `procedural` | **REAL** | `GeometryGraph::evaluate` — recursive DFS from `OutputMesh` with out-of-range + cycle guards (no panics). Nodes: `Primitive` (cube/uv_sphere/cylinder), `Transform`, `Join`, `Subdivide`, `Boolean`, `OutputMesh`. | 6 |
| `export` | **REAL (fitting partial)** | `triangulate` (kept). `to_polymesh_text` / `write_polymesh` — OpenFOAM polyMesh ASCII (points/faces/owner/neighbour/boundary) as a **boundary-patch surface** (single dummy cell; NOT a solve-ready volume mesh — documented). `to_csg_primitive` — local mirror types (`CsgSurface`/`RegionToken`/`CsgDescription`) + **cube (AABB)** and **uv-sphere** primitive fitting. **Not implemented:** cylinder fitting, faceted/DAGMC route (follow-up beads). | 5 |

Plus `math` (3), `primitives` (5), `transform` (3) unchanged. **Total: 48 unit
tests + 1 doctest, all pass.** No fake-green: every unimplemented path returns a
typed error and every test asserts real topology/geometry.

## Algorithms + provenance

All algorithms are **reimplemented from first principles / textbook stencils**,
mirroring Blender/OpenSubdiv *concepts* — **no Blender or OpenSubdiv/Manifold
source is ported**, so no upstream attribution header is required (concepts
carry no copyright). Provenance by module:

- **Simple midpoint subdivide** (`ops`): per n-gon, insert deduplicated edge
  midpoints + face centroid, split into n quads; linear positions (topological
  refinement only). Analogue: Blender `bmo_subdivide` simple mode.
- **Vertex bevel / truncation** (`ops`): one edge-point per (vertex, incident
  edge) at `V + clamp(width,0,L/2)·û`; each face's corner replaced by its two
  edge-points; one new vertex-face per original vertex. Cube → truncated cube.
- **Catmull-Clark** (`subdivision`): E. Catmull & J. Clark (1978) local
  stencils — face point = centroid; interior edge point `(v0+v1+F_a+F_b)/4`,
  boundary `(v0+v1)/2`; interior vertex `(F_avg + 2·R_avg + (n-3)·P)/n`,
  boundary `(m0+6P+m1)/8`. No global solve. Analogue: OpenSubdiv / `MOD_subsurf`.
- **Convex boolean Intersect** (`boolean`): clip operand A's polytope against
  each of operand B's outward face half-spaces (3-D Sutherland-Hodgman), weld +
  angle-order cut points into cap faces. Analogue: Blender `bmo_boolean`
  (Manifold upstream) — restricted to the convex intersection case here.
- **Mirror / Array** (`modifiers`): reflection with orientation flip + epsilon
  weld; relative-offset tiling by the input AABB extent. Analogue:
  `MOD_mirror` / `MOD_array`.
- **polyMesh writer** (`export`): OpenFOAM `FoamFile` ASCII dictionary format
  (points `vectorField`, faces `faceList` as `n(v0 v1 …)`, owner/neighbour
  `labelList`, boundary patch) — format is a well-documented public spec.
- **CSG description** (`export`): local types mirroring `outram-mc-libs`
  `SurfaceKind` (`XPlane`/`YPlane`/`ZPlane`/`Sphere`/`ZCylinder`) + `RegionToken`
  RPN; cube/sphere fitting by AABB planes / centroid-radius.

## V&V — methodology + measured results (2026-07-17, this worktree)

Verification is against **hand-derived / closed-form topology**, asserted
exactly (Euler identities are exact). Key measured numbers (from the passing
suite and the `mesh_operators` example run):

- **Simple subdivide**, cube 1 iter: V=26, E=48, F=24, chi=2 (8 corners + 12
  edge midpoints + 6 centroids; 6·4 quads). 2 iters: V=98, F=96.
- **Bevel**, cube → truncated cube: V=24, E=36, F=14 (6 octagons + 8 triangles),
  chi=2 (asserted via a face-degree histogram).
- **Catmull-Clark**, cube 1 level: V=26, E=48, F=24, chi=2. Corner `(1,1,1)`
  (size-2 cube, r0=√3≈1.7320508) → exactly `(5/9,5/9,5/9)`, |new|≈0.9622504 <
  r0 (strictly inward). Bbox half-extent L0→L3: 1.0 → 0.8491753 (shrinks toward
  the limit surface). Levels=2: V=98, E=192, F=96, chi=2.
- **Boolean Intersect**, cube `[-1,1]³` ∩ translated `[0,2]³` → box `[0,1]³`,
  chi=2, bounds exact to 1e-9. Self-intersection idempotent (`[-1,1]³`, chi=2).
  45°-rotated ∩ axis cube → octagonal prism V=16,E=24,F=10,chi=2.
  Union/Difference → `Unsupported`; disjoint / non-convex → `Unsupported`.
- **Modifiers**: Mirror of an x∈[0,2] quad across X → welded V=6 (seam merged,
  not 8), F=2, span [-2,2]. Array cube×3 offset [1,0,0] → V=24, F=18 (unwelded).
  Subsurf L1 == Catmull-Clark (V=26,F=24,chi=2). Array×3→Subsurf → chi=6 (three
  disjoint closed surfaces, as expected for unwelded copies).
- **Procedural**: cube→Output = cube (8/6/2). Join two disjoint cubes = V=16,
  F=12, disjoint bounds. Subdivide node = CC (26/24/2). Malformed graph
  (out-of-range id, self-cycle) → `BadNode` / `Cycle`, no panic.
- **Export**: polyMesh of cube parses to 8 points / 6 faces / 6 owner labels /
  0 neighbour; CSG cube fit = 6 planes at ±1 + intersection region (11 tokens);
  CSG sphere `uv_sphere(16,8,3.0)` fit = one Sphere r=3.0±1e-6 at origin; grid →
  `NotImplemented`.

## Build / test / lint — measured (2026-07-17, Arch Linux, rustc stable, release)

- `cargo test -p outram-blender --release --lib` → **48 passed, 0 failed**.
- `cargo test -p outram-blender --release --doc` → **1 doctest passed**.
- `RUSTFLAGS="-D warnings" cargo build -p outram-blender --release` → **clean**.
- `cargo clippy -p outram-blender --release --lib --tests` → only the **2
  pre-existing** `math::Vec3` `add`/`sub` "confused for std trait" warnings; the
  new code adds **no** clippy warnings.
- `RUSTFLAGS="-D warnings" cargo check -p outram-blender --release --target
  aarch64-linux-android` → **clean** (pure-Rust: `thiserror` + `faer`; no
  BLAS/C/GUI/wgpu).
- `cargo tree -e no-dev` → **no wgpu** in the default tree (GPU stays behind the
  `gpu` feature).
- `cargo run -p outram-blender --example mesh_operators --release` → runs all
  operator/modifier/procedural/export steps; topology as reported above.
- **unsafe-free**: `grep -c unsafe` over all seven touched source files = 0.

## Human-verify checklist (for the maintainer)

1. **Boolean scope** — is convex-only `Intersect` an acceptable first landing,
   or is a robust general boolean (union/difference, non-convex) a blocker
   before this feeds `outram-mc-libs`? The convexity check is a heuristic
   (assumes correct outward normals), not a proof. (`op-hzs.3` + follow-up.)
2. **Catmull-Clark boundary/non-manifold rules** — spot-check the crease rule
   and the fallbacks for non-manifold edges / irregular boundary vertices.
3. **Bevel** — vertex truncation only; confirm the single-chamfer fallback for
   `segments >= 2` is acceptable until the rounded bevel lands (follow-up).
4. **polyMesh export** — confirm the single-dummy-cell boundary-patch encoding
   is the intended handoff to a volume mesher, and that the `FoamFile` headers
   match the `outram-foam-*` reader before wiring a real dependency (`op-hzs.6`).
5. **CSG export types** — confirm the local `CsgSurface`/`RegionToken` mirror
   matches `outram-mc-libs`' current `SurfaceKind`/`RegionToken` before wiring
   (`op-hzs.7`); cylinder + faceted fitting still TODO.
6. **Winding/normals** — Euler characteristic is asserted everywhere, but
   consistent outward winding after each operator is argued in comments and
   spot-checked, not exhaustively asserted.
7. **Bookkeeping axes** — both V&V and human-interface axes remain ❌ until
   personal review.

## Follow-up beads filed this pass

- Multi-segment (rounded) vertex bevel (`ops` — `segments >= 2`).
- Boolean `Union` / `Difference` + general non-convex `Intersect` (`boolean`).
- CSG cylinder fitting + faceted/DAGMC route (`export`).
- Wire `to_polymesh_text` / `to_csg_primitive` to the real `outram-foam-*` /
  `outram-mc-libs` types once those APIs settle (`op-hzs.6` / `op-hzs.7` keep
  the wiring half open).

> Beads live in the local Dolt store; the `.beads/issues.jsonl` export may lag
> due to a known cross-fleet sync divergence (other fleets' unimported records).
