# snappyHexMesh implementation — AI fleet review manifest

> # ⚠️ UNTRUSTED AI DRAFT — HUMAN REVIEW REQUIRED
>
> Every source and test file referenced here was written by AI agents (Claude
> Opus 4.8) working as a fleet. Per `RESPONSIBLE_USE.md` and `AI_USAGE.md`, this
> is **untrusted draft material** until a human has inspected the code, checked
> the OpenFOAM provenance and licence headers, and re-run and interpreted the
> V&V results. Do **not** describe any of this as validated or trusted until
> that review is done. Nothing here is for nuclear-facility operation, reactor
> control, licensing, or any safety-critical use.

> **Historical snapshot — partly superseded (noted 2026-08-07).** This file
> records the state of the crate *as reviewed on 2026-07-17* and is deliberately
> left un-rewritten as a record of that pass. Two of its "PARTIAL" findings have
> since been addressed in the code, so do **not** read them as the current
> status:
>
> - **§3 Layer addition / follow-up bead `op-ax7.2.2`** — layers are no longer
>   only an *extrusion off the patch*. The medial-axis **interior
>   shrink-and-insert** is now implemented (restricted to regions where it stays
>   watertight), with the outward extrusion retained as the fallback for octree
>   hanging-node regions. See `snappy_hex_mesh/layers.rs`'s "Honest scope" and
>   `driver::LayerOutcome`, which reports which placement a given case got.
> - **§4 blockMesh grading / follow-up bead `op-ax7.5`** — per-edge
>   `edgeGrading` is no longer restricted to the equal-edges-per-direction case;
>   the full 12-edge blend is implemented (straight-edge branch of
>   `block::createPoints`). See `block_mesh.rs`'s module docs.
>
> The V&V numbers below are those measured on 2026-07-17 and were not re-run for
> this note. Current, re-measured V&V results live in the module doc comments and
> the crate `README.md`. The human-verify checklist in §"Human-verify checklist"
> remains **open** — nothing here has been signed off by the maintainer.

- **Date:** 2026-07-17
- **Crate:** `crates/outram-foam-mesh` (bead epic **op-ax7**)
- **Scope of this pass:** snappyHexMesh Phase 2 (snapping, op-ax7.2.1), Phase 3
  (layer addition, op-ax7.2.2), the shared point-topology substrate, the
  full castellation→snapping→layers wiring, and blockMesh multi-grading
  (op-ax7.5).
- **Canonical OpenFOAM mirrored from:** `/home/teddy0/Documents/research/openfoam/`
- **Host / mode:** x86_64 Linux, `cargo … --release`.

## Fleet structure (one distinct file per subagent; lead integrated)

| Agent | File (only file it edited) | Deliverable |
|---|---|---|
| Lead | `snappy_hex_mesh/poly_topology.rs` (new), `snappy_hex_mesh/castellation.rs` (enriched), `snappy_hex_mesh.rs`, `tests/snappy_hex_mesh.rs` | Shared point/face topology + FvMesh-rebuild + quality metrics; integration wiring + tests |
| Subagent A | `snappy_hex_mesh/snapping.rs` | Phase-2 snapping |
| Subagent B | `snappy_hex_mesh/layers.rs` | Phase-3 layer addition |
| Subagent C | `block_mesh.rs` | blockMesh multi-grading + edgeGrading |

## Per-piece status — REAL vs PARTIAL

### 1. Shared point-topology substrate — `poly_topology.rs` (lead) — REAL
`PolyPatchMesh` (points + per-face point connectivity) with:
- `build_fvmesh()` — recomputes all FV geometry from points.
- `quality()` / `QualityLimits` — non-orthogonality, skewness, min-volume gate.

**OpenFOAM provenance:** face centre/area — `primitiveMeshFaceCentresAndAreas.C`
/ `primitiveMeshTools::makeFaceCentresAndAreas`; cell centre/volume —
`primitiveMeshCellCentresAndVols.C` / `makeCellCentresAndVols`; quality —
`primitiveMeshCheck/primitiveMeshTools.C` `faceOrthogonality` / `faceSkewness`.

**V&V (measured 2026-07-17):** the enriched castellation carries a topology that
rebuilds the *original* castellation `FvMesh` geometry exactly —
`max|ΔSf| = 0`, `max|ΔCf| = 0`, `max|ΔV| = 4.16e-17`, `max|ΔC| = 6.84e-16` over
the 848-cell sphere-box case. Unit cube: volume 1.0, centre error < 1e-12, all
six outward face normals correct. Collapsed cell → flagged non-positive volume
and rejected. **3 lib tests + 1 integration test.**

### 2. Snapping — `snapping.rs` (subagent A) — REAL (feature snapping restricted)
`snap()` implements `snappySnapDriver::doSnap`: find the wall patch, project
patch points to the nearest STL point, Laplacian-smooth the displacement over
patch-point adjacency, apply under a quality-gated λ-halving relaxation, rebuild
the `FvMesh`. Hanging-node (2:1 octree T-junction) constraints keep the mesh
watertight while points move.

**OpenFOAM provenance:** `snappySnapDriver.C` (`doSnap` C:2574,
`calcNearestSurface` C:1797, `smoothPatchDisplacement` C:290,
`smoothDisplacement`/`scaleMesh` C:2134/2208); feature snapping from
`snappySnapDriverFeature.C`.

**V&V (measured 2026-07-17):**
- Sphere: max wall→surface distance **0.10500 m → 1.4686e-4 m** (~715× tighter),
  mesh `validate()` Ok, default quality gate accepts, **0** negative-volume
  cells (min cell vol 5.20e-4 m³), watertightness max |Σ Sf| = 3.2e-17 m².
- Feature snap (box): 12 feature edges detected, 188 patch points snapped onto
  an edge with 0.0 m residual; mesh valid + watertight.
- **5 lib tests + 1 integration test.**

**PARTIAL / restricted:** feature snapping is a genuine, tested restricted port —
it snaps points near a crease onto the edge but does not reproduce OpenFOAM's
full feature-*point* / `pointConstraint` alignment (documented in the module
docs). Hanging-node constraints cover single-level 2:1 edge midpoints (nested
handled via extra relaxation passes).

### 3. Layer addition — `layers.rs` (subagent B) — PARTIAL (restricted, honest)
`add_layers()` extrudes graded prism layers off the wall patch: area-weighted
point normals, geometric grading (`expansion_ratio`, `first_layer_thickness`, or
back-solved `final_layer_thickness`), per-point thickness cap, quality-limited
collapse (reduce layers rather than ever return a negative-volume cell), then
`FvMesh` rebuild.

**OpenFOAM provenance:** `snappyLayerDriver.C`; `layerParameters/layerParameters.C`
(FIRST/FINAL_AND_EXPANSION relation, C:927).

**V&V (measured 2026-07-17):** flat-wall unit case — new cell count 8 (was 2),
3 new layers/face, measured successive-layer height ratios **1.5000, 1.5000**
(err < 1e-12), `total_layer_thickness = 0.475 m`; rebuilt mesh validates,
`n_negative_volume_cells = 0`, watertight (< 1e-12 m²), `final_layer_thickness`
back-solve recovers the expected first-layer thickness. Sphere-box integration:
**848 → 1472 cells**, valid, watertight, inversion-free. **5 lib tests + 1
integration test.**

**PARTIAL — remaining work (honest TODO, follow-up bead filed):** this is an
*extrusion off the patch* (grows a prism block attached to the wall), **not**
the full OpenFOAM medial-axis approach that shrinks the existing near-wall
interior mesh to insert layers on the fluid side. The interior-coupled
medial-axis displacement + true `addPatchCellLayer` splitting is not implemented.
See the module docs' "Honest scope" section.

### 4. blockMesh multi-grading — `block_mesh.rs` (subagent C) — REAL (edgeGrading restricted)
Multi-grading lists (`simpleGrading ( (frac_len frac_cells expansion)… gy gz )`)
fully implemented via a per-direction `Grading` enum (Uniform / Segments);
`edgeGrading` handled for the equal-edges-per-direction case (exact), otherwise
an improved `NotImplemented` message.

**OpenFOAM provenance:** `blockMesh` `gradingDescriptor`/`gradingDescriptors`,
block edge-point distribution.

**V&V (measured 2026-07-17):** multi-grading cube preserves total cell count,
monotone node positions, segment boundaries at requested fractions, per-segment
geometric ratio asserted numerically; uniform `simpleGrading` regression intact;
built `FvMesh` validates and total volume equals block volume. **8 lib tests.**

## Aggregate verification (this pass, 2026-07-17)

- `cargo build -p outram-foam-mesh --release` — clean, **no warnings**.
- `cargo test -p outram-foam-mesh --release --lib --tests` — **49 tests pass**
  (21 lib + 8 + 3 + 6 + 11 integration), 0 failed.
- `cargo check -p outram-foam-mesh --target aarch64-linux-android` — clean
  (Android-portable; mesh gen is pure compute, no BLAS/GUI).
- Hard rules: no `Box<T>`, no `dyn` dispatch, no new lifetime parameters, no
  channels; index-based topology; GPLv3 + OpenFOAM provenance headers on all
  ported files; `///`/`//!` docs on public items.

## Human-verify checklist (maintainer)

1. **Snapping correctness beyond the sphere:** confirm the hanging-node
   constraint logic holds on a real multi-level STL (sharp features, thin gaps),
   not just the sphere/box test cases. Check the 50/50 blend of smoothed vs
   direct surface-pull displacement is physically acceptable.
2. **Layer model suitability:** decide whether the restricted *extrude-off-patch*
   layer model is acceptable for the intended use, or whether the medial-axis
   interior-shrink is required before this is used for boundary-layer meshing.
   The current layers grow a block attached to the wall — verify this matches
   the reviewer's expectation before trusting layered meshes.
3. **Feature snapping fidelity:** verify the restricted feature-edge snapping vs
   OpenFOAM's feature-point/`pointConstraint` behaviour on cases with corners.
4. **edgeGrading:** confirm the equal-edges-per-direction restriction and the
   `NotImplemented` fallback are acceptable; full per-edge grading is deferred.
5. **Re-run all V&V** and confirm the measured numbers above reproduce on the
   maintainer's machine (they are host/rounding-sensitive at the 1e-16 level).
6. **Provenance/licence:** confirm the OpenFOAM file:line citations in the doc
   comments are accurate and the GPLv3 headers are intact.

## Follow-up beads

- **op-ax7.2.2** left OPEN-with-progress (layers): medial-axis interior-coupled
  layer insertion is the remaining work.
- **op-ax7.5** (blockMesh grading): full per-edge `edgeGrading` deferred.
