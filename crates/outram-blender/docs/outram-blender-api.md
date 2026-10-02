# Crate Documentation

**Version:** 0.0.4

**Format Version:** 60

# Module `outram_blender`

# outram-blender

A pure-Rust, headless **mesh-authoring frontend** for the OUTRAM PARK
multiphysics suite, inspired by the **architecture** of
[Blender](https://github.com/blender/blender) (GPLv2-or-later, which is
GPLv3-compatible). It authors and procedurally generates geometry, then
bridges it into two OUTRAM PARK solver workflows:

- **Monte Carlo neutron transport** (feature `mc-export`). Author a surface,
  fit it to an `outram-mc-libs` CSG universe ([`export`]), attach materials,
  and run a k-eigenvalue (criticality) calculation returning `k_eff ± σ`
  (the `sim` module). This path is driven by the **MC Studio** egui app
  (`examples/mc_studio`).
- **CFD / thermal-hydraulics volume meshing** (feature `foam-mesh`). Hand a
  closed surface to `outram-park-fork-cfmesh`'s tet→dual→boundary-layers
  pipeline and write out an OpenFOAM `polyMesh` (the `foam_mesh` module). This
  path is driven by the **Mesh Studio** egui app (`examples/mesh_studio`).

The base authoring library (primitives, mesh operators, modifiers, procedural
evaluator, geometry processing) pulls in neither solver — both bridges are
opt-in cargo features, so the default build stays light and Android-buildable.

> **⚠️ Not a Blender port.** Blender is millions of lines of C/C++/Python;
> this crate borrows its *concepts and data-structure architecture* (the
> BMesh half-edge topology, the mesh-operator model, the modifier stack,
> geometry-nodes-style procedural generation) — it does **not** port
> Blender's code (the only literally-ported piece is the Shewchuk robust
> predicates in [`boolean_predicates`], with its GPL provenance header). The
> algorithms here — primitives, mesh operators, subdivision (Catmull-Clark &
> Loop), the general CSG boolean, the sparse-solve geometry processing
> (Laplacian/Taubin smoothing, harmonic parameterization, ARAP deformation),
> QEM decimation, the modifier stack, the procedural evaluator, and the
> export bridges — are written from first principles and unit-tested against
> analytic references. See the module map for per-module status.
>
> **Not affiliated with the Blender Foundation.** "Blender" names the
> upstream project whose architecture inspired this work; nothing here is
> endorsed by or sanctioned by the Blender Foundation. See the README's
> "Naming & trademark" section — the maintainer decided on 2026-07-17 to
> keep the name `outram-blender` and mark the fork status explicitly.
>
> **Untrusted AI-generated draft** until a human reviews it, per the
> workspace `RESPONSIBLE_USE.md`. Not for nuclear facility operation,
> reactor control, safety-critical, or licensing decisions.

## Module map — what belongs where

| Module | Blender analogue | Status |
|---|---|---|
| [`math`] | `blenlib` `BLI_math` vector types | **real** — a minimal pure-Rust [`math::Vec3`] |
| [`transform`] | `Object.matrix_world` affine placement | **real** — [`transform::Affine3`] per-vertex transform (CPU reference for the GPU kernel) |
| `gpu` *(feature `gpu`, default-on; never on Android)* | — (no Blender analogue) | **real** — headless `wgpu` compute (WGSL); one wired kernel (parallel affine vertex transform) with probe + graceful CPU fallback. ~~Compiled unconditionally on desktop~~ **CORRECTED 2026-10-02**: behind the default-on `gpu` feature; absent on Android, or with `--no-default-features` |
| [`mesh`] | `bmesh` (`BMVert`/`BMEdge`/`BMLoop`/`BMFace`) | **real** — index-based half-edge topology |
| [`selection`] | `editmesh_select.cc` / `BM_select_*` | **real** — select modes + flush; all/none/invert; box/sphere/lasso region; linked; mirror; edge/face loop, ring, boundary loop, shortest path; more/less; select similar; checker deselect; non-manifold / loose / interior-faces / faces-by-sides (GH issue #37 §A — `op-hzs.54.1`–`.4`) |
| [`topology`] | `bmesh_queries.cc` / `bmesh_walkers_impl.cc` | **real** — precomputed radial (edge→faces) + disk (vertex→edges) adjacency; edge-loop / edge-ring / face-loop walkers; Dijkstra + BFS path helpers |
| [`loop_cut`] | `editmesh_loopcut.cc` / `bmo_subdivide_edgering` | **real** — Loop Cut and Slide: N parallel loops across an edge ring, with a slide factor; quad-only, splices terminal n-gons (GH issue #37 §B) |
| [`knife`] | `editmesh_knife.cc` | **real** — split faces along a path of boundary-point chords (edge-split / vertex); polyline→chord resolver is follow-up (GH issue #37 §B) |
| [`slide`] | `transform_mode_edge_slide.cc` / `_vert_slide.cc` | **real** — position-only edge-loop / vertex slide along rail edges, consistent side propagation (GH issue #37 §B) |
| [`subdivide`] | `bmo_subdivide.cc` | **real** — N-cut subdivide (quad grid / tri lattice / n-gon fan) with smoothness + deterministic fractal; un-subdivide halves a quad grid (GH issue #37 §B) |
| [`bevel`] | `bmesh_bevel.cc` | **real** — multi-segment rounded edge bevel over [`edge_bevel`]: `segments`, `profile`, `WidthType` (offset/width/depth/percent), clamp-overlap; corner fan-filled (rounded corner patch + selected-subset are follow-up) (GH issue #37 §B) |
| [`extrude`] | `bmo_extrude.cc` / `editmesh_extrude.cc` | **real** — extrude individual faces (own normal), region along averaged normals, vertices, manifold; complements [`ops`]'s region/edge extrude (GH issue #37 §B) |
| [`merge`] | `editmesh_tools.cc` `MESH_OT_merge` | **real** — merge vertices at centre / point / first / last; collapse edges; merge-by-distance over a subset (Auto-Merge) (GH issue #37 §B) |
| [`rip_split`] | `editmesh_rip.cc` / `MESH_OT_separate` / `_split` | **real** — split a face group into an island; separate by selection / loose parts / group; rip a slit along edges (GH issue #37 §B) |
| [`bridge`] | `bmo_bridge.cc` | **real** — join two equal-length edge loops with a face strip; twist / cuts / flip / weld; ordered_ring walker (GH issue #37 §B) |
| [`fill`] | `bmo_grid_fill.cc` / `bmo_triangle_fill.cc` / `MESH_OT_edge_face_add` | **real** — make_face (F), grid_fill (Coons quad grid from a 4-sided loop), beauty_fill (Delaunay diagonal flips) (GH issue #37 §B) |
| [`dissolve`] | `bmo_dissolve.cc` / `MESH_OT_delete` | **real** — dissolve faces/edges/vertices (merge to one n-gon), limited dissolve (planar cleanup), the delete/erase matrix (GH issue #37 §B) |
| [`connect`] | `bmo_connect.cc` | **real** — connect vertex path / pairs (J) via knife face-chord splits (GH issue #37 §B) |
| [`poke_quads`] | `bmo_poke.cc` / `bmo_join_triangles.cc` | **real** — poke faces (centroid fan + offset), triangulate quads by method, tris↔quads join (GH issue #37 §B) |
| [`edge_tools`] | `bmo_rotate_edges.cc` / mesh_edge_flow / `MOD_edgesplit.cc` | **real** — rotate edge CW/CCW, set edge flow (loop relax), edge split operator (GH issue #37 §B) |
| [`transform_ops`] | `transform_mode_*.cc` | **real** — to-sphere / shear / bend / warp / push-pull / shrink-fatten / randomize / smooth-verts, position-only over a selection (GH issue #37 §C) |
| [`proportional`] | `transform_proportional_*` | **real** — proportional-edit falloff (smooth/sphere/root/inv-sq/sharp/linear/constant/random), Euclidean or connected-only distance (GH issue #37 §C) |
| [`symmetry`] | `bmo_symmetrize.cc` / `MESH_OT_symmetry_snap` | **real** — symmetrize (mirror + weld a half), snap-to-symmetry (average with partner), mirror_selection (GH issue #37 §C) |
| [`spin_screw`] | `bmo_spin_exec` / `MOD_screw.cc` | **real** — spin a profile selection around an axis (bridged or duplicates), screw = spin + axial translation (helix) (GH issue #37 §C) |
| [`transform_input`] | `transform_input.cc` / `transform_constraints.cc` | **real** — Constraint (free/axis/plane), TransformBasis (global/normal), NumericEntry with an expression evaluator (pi/tau/e, ^ right-assoc), grid-increment snap; the CAD precision-input model (GH issue #37 §D) |
| [`snap`] | `transform_snap.cc` / `transform_snap_object.cc` | **real** — snap to grid increment / vertex / edge-midpoint / nearest-on-edge / nearest-on-face; SnapBase closest/center/median/active; align-rotation-to-target; snap-onto-self exclusion (GH issue #37 §D) |
| [`cursor_pivot`] | view3d_cursor_snap / transform_orientations.cc | **real** — 3D cursor placement, PivotPoint (bbox/cursor/individual-origins/median/active), rotate/scale about pivot, custom orientation from a vertex/edge/face selection (GH issue #37 §D) |
| [`measure`] | mesh-statistics overlay / ruler gizmo / mesh-analysis | **real** — edge/area/angle/dihedral readouts, volume, dimensions, Ruler + Protractor, overhang / distortion / sharp-edges / self-intersection (tri-tri) / thickness (ray cast) (GH issue #37 §D) |
| [`normals`] | `MESH_OT_normals_*` / mesh_normals.cc | **real** — flip, recalc inside/outside, vertex_normals by weight (uniform/area/corner-angle), point-to-target, SplitNormals + split_normals_by_angle + harden (GH issue #37 §E) |
| [`attributes`] | `MESH_OT_mark_*` / crease / bevel-weight / auto-smooth | **real** — MeshAttributes: sharp/seam/freestyle marks, edge/vertex crease + bevel weight, per-face smooth + material; auto_smooth; linked_delimiters -> Selection::select_linked_delimited (GH issue #37 §E) |
| [`remesh`] | mesh_remesh_voxel.cc / MOD_mesh_to_volume | **real** — VoxelGrid occupancy, mesh_to_volume (ray-parity raster), volume_to_mesh (blocky isosurface), voxel_remesh (+ smoothing) (GH issue #37 §F) |
| [`deform`] | MOD_simpledeform / MOD_cast / MOD_displace / MOD_warp / MOD_wave | **real** — simple_deform (twist/bend/taper/stretch), cast (sphere/cyl/cuboid), displace (value noise), warp (segment map), wave (GH issue #37 §F) |
| [`deform2`] | MOD_curve / MOD_lattice / MOD_hook / MOD_shrinkwrap / MOD_surfacedeform | **real** — curve_deform (arc-length ride), Lattice + lattice_deform (trilinear FFD), hook (falloff drag), shrinkwrap (3 modes), bind_to_surface + surface_deform, laplacian_deform -> arap (GH issue #37 §F) |
| [`curve`] | BKE_curve / curve_to_mesh | **real** — Spline (poly / Bézier / NURBS de-Boor), ControlPoint (handle types, radius, tilt, weight), recalculate_handles, cyclic, sample + sample_with_frames (parallel-transport + tilt) (GH issue #37 §G) |
| [`curve_surface`] | curve_to_mesh.cc / displist.cc | **real** — Bevel (round / custom profile / none), taper spline, 2-D fill (ear clip), end caps; sweep a section along the spline frames (GH issue #37 §G) |
| [`curve_mesh`] | OBJECT_OT_convert / MOD_curve / MOD_skin | **real** — mesh_to_splines (edge chains), boundary_to_splines, spline_to_mesh, spline_deform_mesh (curve modifier), skin_spline (GH issue #37 §G) |
| [`nurbs_surface`] | BKE_nurb_makeFaces / editcurve_add.cc | **real** — NurbsSurface tensor-product rational B-spline (periodic knots for cyclic axes), evaluate + to_mesh, plane/sphere/cylinder/torus primitives, control-point patch editing (GH issue #37 §G) |
| [`text`] | `blenkernel/intern/vfont.cc` (text objects) | **real** — Font/Glyph outline table (+ built-in block stroke font), text_to_contours (baseline layout), text_to_mesh (ear-clip fill, extrude, chamfer bevel) for name plates / labels / gauge faces (GH issue #37 §G) |
| [`primitives`] | `editors/mesh/editmesh_add` primitive add-ops | **real** — cube / UV-sphere / cylinder / grid generators (unit-tested) |
| [`primitives_extra`] | `add_mesh_*` operators + redo panels | **real** — plane / circle (fill or wire) / cone + truncated cone / torus / geodesic icosphere; AddMeshOptions (location, XYZ-Euler rotation, scale) common settings, all Euler-checked (GH issue #37 §H) |
| [`extra_objects`] | `add_mesh_extra_objects` add-on | **real** — rounded_cube (rounded-box SDF projection), capsule, spur_gear (trapezoidal teeth), pipe + elbow (hollow, swept), wedge, star, honeycomb, z_function_surface (generic `Fn(x,y)->z`) (GH issue #37 §H) |
| [`draw_tool`] | interactive Add Object Tool (place gizmo) + snap/PDT entry | **real** — WorkPlane (xy/xz/yz/from-normal), staged DrawGesture (PickBase→DragFootprint→DragDepth→Done), box/circle/cone from-drag, snap_input (SnapTarget projection), eval_dimension (expression entry) (GH issue #37 §H) |
| [`loop_tools`] | `mesh_looptools` add-on | **real** — circle (best-fit circle), flatten (best-fit plane), relax (Laplacian), curve (Catmull–Rom toward anchors), space (equal arc length), gstretch (onto a stroke), bridge + loft, subdivide (loop-edge midpoint split) (GH issue #37 §I) |
| [`snap_line`] | `mesh_snap_utilities_line` add-on | **real** — LineTool connected polyline: add_raw / add_snapped (snap engine) / add_polar / add_constrained (numeric length + angle in a WorkPlane), undo, close, commit_wire, auto_cut_chords (edge-to-edge-on-one-face knife cuts) (GH issue #37 §I) |
| [`pdt`] | `precision_drawing_tools` add-on | **real** — Placement (Absolute/Delta/Polar/Percent), three_point_circle + three_point_arc, line_line_intersection (3-D closest approach), fillet (tangent corner arc), offset_polyline, taper, angle_between, mirror_point / mirror_vertices across a WorkPlane (GH issue #37 §I) |
| [`bool_tool`] | `object_boolean_tools` add-on | **real** — BoolStack of non-destructive BoolBrush cutters (Difference/Union/Intersect/Slice, enable toggle); bake (strict fold), carve (skip unresolvable brushes), slice_pieces (inside part per Slice brush); wraps `boolean` (GH issue #37 §I) |
| [`fill_helpers`] | `mesh_f2` / `object_auto_mirror` / `mesh_bsurfaces` | **real** — f2_fill (smart F: quad or triangle from one boundary edge), auto_mirror (bisect + mirror + weld in one call), bsurfaces (lofted quad surface through resampled strokes) (GH issue #37 §I) |
| [`object_ops`] | Object menu (Duplicate/Join/Separate/Apply/Set Origin/Align/Snap) | **real** — SceneObject (Arc<Mesh> + Affine3); duplicate vs linked_duplicate, join, separate_loose_parts (union-find), apply_transform, set_origin (geometry/surface-COM/volume-COM/cursor), align, snap_objects, cursor_to_objects (GH issue #37 §J) |
| [`array_patterns`] | Array modifier Object-Offset + Curve modifier | **real** — radial_array / circular_array (rotate about an axis), object_offset_array (compounding Affine3), array_along_curve (spline frames, align an Axis to the tangent), ArrayCaps start/end (GH issue #37 §J) |
| [`revolve`] | Spin (`bmo_spin`) | **real** — sweep a profile polyline around an axis into a surface of revolution (pipes / vessels / cones) |
| [`ops`] | `bmesh/operators/*` (`bmo_*`) mesh operators | **real** — extrude / midpoint-subdivide / vertex-bevel (flat or rounded multi-segment; boolean delegates to [`boolean`]) |
| [`subdivision`] | OpenSubdiv / `MOD_subsurf` | **real** — Catmull-Clark surface subdivision (local stencils) |
| [`loop_subdivision`] | `MOD_subsurf` (triangle path) | **real** — Loop subdivision surface for triangle meshes |
| [`laplacian`] | `MOD_laplaciansmooth` / `bmo_smooth_laplacian` | **real** — cotangent/uniform discrete Laplacian + implicit & Taubin smoothing (first `faer` sparse solve) |
| [`parameterize`] | UV unwrap (harmonic map) | **real** — Tutte/harmonic planar parameterization of a disk (reuses the Laplacian sparse solve) |
| [`arap`] | "As Rigid As Possible" deform | **real** — ARAP handle-based deformation (local rotation fit + cotangent-Laplacian global solve) |
| [`decimate`] | `MOD_decimate` (Collapse) | **real** — QEM (Garland–Heckbert) edge-collapse mesh simplification |
| [`convex_hull`] | `bmo_convex_hull` | **real** — 3D convex hull of a point set (incremental, robust `orient3d`) |
| [`weld`] | `bmo_remove_doubles` / Merge by Distance | **real** — merge coincident vertices within a tolerance (grid hash + union-find) |
| [`fill_holes`] | `bmo_holes_fill` / Fill Holes | **real** — cap open boundary loops with a centroid fan (watertight) |
| [`solidify`] | `MOD_solidify` (simple) | **real** — extrude a surface into a closed shell (inner offset + rim) |
| [`recalc_normals`] | `normals_make_consistent` (Recalculate Outside) | **real** — repair inconsistent winding (BFS) + flip each component outward |
| [`triangulate`] | `bmo_triangulate` (fan) | **real** — fan-triangulate every face into a triangle-only mesh |
| [`inset`] | `bmo_inset` (Individual) | **real** — per-face inset: shrunk inner copy + bridging ring quads |
| [`bisect`] | Bisect (plane cut) | **real** — half-space clip every face by a plane (Sutherland–Hodgman); leaves the cut open |
| [`edge_bevel`] | Bevel (edges) | **real** — chamfer every edge (cut faces back + fill edge/corner gaps); winding fixed by `recalc_normals` |
| [`boolean`] | `bmo_boolean` (Manifold upstream) | **real** — CSG entry point: exact convex-`Intersect` fast path, else delegates to [`boolean_general`] |
| [`boolean_general`] | `mesh_boolean.cc` / `mesh_intersect.cc` arrangement | **real** — general union/difference/intersect on non-convex closed meshes (arrangement + winding classification) |
| [`boolean_predicates`] | `blenlib` `math_boolean.cc` (Shewchuk) | **real** — robust `orient2d/3d`, `incircle`, `insphere` (adaptive f64 + double-double fallback) |
| [`boolean_classify`] | `mesh_boolean.cc` inside/outside classification | **real** — point-in-closed-mesh via generalized winding number (+ ray-parity cross-check) |
| [`modifiers`] | `modifiers/intern/MOD_*` modifier stack | **real** — subsurf / mirror / array |
| [`procedural`] | Geometry Nodes (`nodes/geometry/*`) | **real** — node-graph evaluator |
| [`export`] | I/O exporters (`io/*`) | **real** — OpenFOAM polyMesh text + CSG fitting (box/sphere/cylinder/convex-faceted) + DAGMC faceted-solid (with an opt-in closed-2-manifold gate, [`export::to_faceted_solid_checked`]) + feature-gated real-type bridges (`foam-export`, `mc-export`) |
| [`stl`] | STL I/O | **real** — ASCII + binary STL read/write (surface-mesh interchange / DAGMC / Monte-Carlo feed) |
| `sim` *(feature `mc-export`)* | — (no Blender analogue) | **real** — Monte Carlo setup + run: build materials, bundle geometry/source/settings, run a k-eigenvalue criticality calc (`k_eff ± σ`) via `outram-mc-libs`. Backend of **MC Studio** |
| `foam_mesh` *(feature `foam-mesh`)* | — (no Blender analogue) | **real** — volume-meshing bridge: blender surface → `outram-park-fork-cfmesh` tet→dual→boundary-layers pipeline → OpenFOAM `polyMesh`, gated by a closed-2-manifold check on the surface. Backend of **Mesh Studio** |

## Design rules honoured here (workspace `CLAUDE.md`)

- **Index-based topology, no lifetimes/pointers.** Every element is
  addressed by a newtype index ([`mesh::VertexId`], [`mesh::EdgeId`],
  [`mesh::LoopId`], [`mesh::FaceId`]) into a `Vec`, exactly as the workspace
  forbids `&'a`-linked graph nodes.
- **Enums for dispatch, never trait objects.** The operator, modifier, and
  procedural-node sets are closed and enumerated ([`ops::MeshOp`],
  [`modifiers::Modifier`], [`procedural::GeometryNode`]).
- **No `Box<T>`; `Arc<T>` for sharing.** Owned meshes are passed by value;
  shared read-only meshes use `std::sync::Arc`.

## Where to start reading

[`primitives`] is the primary entry point. Read [`primitives::cube`]
top-to-bottom, then the [`mesh::Mesh`] type it builds on; from there the
[`ops::MeshOp`] enum is the map of what you can *do* to a mesh (extrude,
bevel, boolean, smooth, decimate, subdivide, ARAP-deform).

```
use outram_blender::primitives;

// A unit cube centred at the origin: 8 vertices, 12 edges, 6 quad faces.
let cube = primitives::cube(1.0);
assert_eq!(cube.vertex_count(), 8);
assert_eq!(cube.edge_count(), 12);
assert_eq!(cube.face_count(), 6);
// Euler characteristic of a closed genus-0 surface: V - E + F = 2.
assert_eq!(cube.euler_characteristic(), 2);
```

## Modules

## Module `arap`

**As-Rigid-As-Possible (ARAP) surface deformation** (Sorkine & Alexa, 2007).

Blender analogue: the Laplacian-deform / "As Rigid As Possible" mesh tools.
This is the crate's most involved sparse-solve operator: it deforms a mesh to
match prescribed **handle** positions while keeping every local
neighbourhood **as rigid as possible** (bending and rotating, resisting
stretching/shearing). It reuses the cotangent-Laplacian machinery from
[`crate::laplacian`] — the global system matrix is exactly that Laplacian.

## Algorithm (local / global alternation)

Minimize `E = Σ_i Σ_{j∈N(i)} w_ij ‖(p'_i − p'_j) − R_i (p_i − p_j)‖²` over the
deformed positions `p'` and per-vertex rotations `R_i ∈ SO(3)`, where `p` is
the rest pose and `w_ij = (cot α + cot β)/2` are the cotangent weights.

- **Local step** (fix `p'`, solve each `R_i`): form the `3×3` covariance
  `S_i = Σ_j w_ij (p'_i − p'_j)(p_i − p_j)ᵀ` (deformed ⊗ rest) and take the
  closest rotation `R_i = U Vᵀ` (the orthogonal Procrustes solution, with a
  determinant sign-fix) from the SVD `S_i = U Σ Vᵀ` (`closest_rotation`) —
  the rotation that best maps the rest one-ring onto the deformed one.
- **Global step** (fix `{R_i}`, solve `p'`): solve `L p' = b` with
  `b_i = Σ_j (w_ij/2)(R_i + R_j)(p_i − p_j)`, where `L` is the cotangent
  Laplacian. Handle vertices are constrained (pinned to their targets), which
  grounds the otherwise-singular `L` — the exact same boundary-pinned SPD
  reduced system as [`crate::laplacian::laplacian_smooth`].

Because `L` and the free/fixed partition are fixed across iterations, the
sparse matrix is **factorized once** (`faer` sparse Cholesky) and only the
right-hand side `b` is rebuilt each iteration. The energy `E` is non-increasing
(each step is an exact partial minimizer); [`arap_energy`] exposes it for
convergence checks.

> **Untrusted AI-generated draft** until a human reviews it, per the
> workspace `RESPONSIBLE_USE.md`. Not for nuclear facility operation,
> reactor control, safety-critical, or licensing decisions.

```rust
pub mod arap { /* ... */ }
```

### Types

#### Enum `ArapError`

Errors from [`arap_deform`].

```rust
pub enum ArapError {
    NoConstraints,
    Assembly,
    NotPositiveDefinite,
}
```

##### Variants

###### `NoConstraints`

No handle/anchor constraints were given. Without at least one fixed
vertex per connected component the cotangent Laplacian is singular (the
constant/translation null space), so the solve is undefined.

###### `Assembly`

The sparse system could not be assembled (bad index).

###### `NotPositiveDefinite`

The reduced Laplacian is not positive definite — a component with no
constrained vertex, or a very obtuse (non-Delaunay) mesh whose cotangent
weights broke SPD.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Error**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(source: crate::arap::ArapError) -> Self { /* ... */ }
    ```

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `arap_deform`

Deform `mesh` (its positions are the rest pose) so the given `handles`
(vertex → target position) are met while every one-ring stays as rigid as
possible, via `iterations` local/global ARAP steps.

Handle vertices are pinned to their targets; all other vertices are solved
for. Topology is preserved (only positions change). More `iterations` = tighter
convergence; 3–10 is typically plenty. Returns the deformed mesh.

# Errors

[`ArapError::NoConstraints`] if `handles` is empty;
[`ArapError::NotPositiveDefinite`] / [`ArapError::Assembly`] on a solve/setup
failure (see those variants).

```rust
pub fn arap_deform(mesh: &crate::mesh::Mesh, handles: &[(crate::mesh::VertexId, crate::math::Vec3)], iterations: u32) -> Result<crate::mesh::Mesh, ArapError> { /* ... */ }
```

#### Function `arap_energy`

The ARAP energy `E = Σ_i Σ_{j∈N(i)} w_ij ‖(d_i − d_j) − R_i (p_i − p_j)‖²` of a
deformed configuration `deformed` relative to `rest` (`R_i` the per-vertex
optimal rotation). Non-negative; non-increasing across [`arap_deform`]
iterations — a convergence diagnostic (see the module tests).

`deformed` must have one position per vertex, in [`crate::mesh::VertexId`]
order (e.g. `arap_deform(...)?.positions()`).

```rust
pub fn arap_energy(rest: &crate::mesh::Mesh, deformed: &[crate::math::Vec3]) -> f64 { /* ... */ }
```

## Module `bisect`

Bisect — cut a mesh by a plane and keep one half.

This is the pure-Rust analogue of Blender's **Bisect**: a plane (a point and
a normal) slices the mesh, and the half on the normal-negative side —
`n · (x − point) <= 0` — is kept. Every face is clipped against that
half-space with the Sutherland–Hodgman algorithm, so faces straddling the
plane are cut cleanly and faces fully on the discarded side vanish.

The cut is left **open**: bisect does not cap the exposed section. That is
deliberate — it composes with [`crate::fill_holes`], which caps the planar
boundary loop into a watertight solid. The pair `bisect` then `fill_holes`
is the half-space-cut primitive the CSG / Monte-Carlo workflow builds on.

# Shared crossing vertices

Where the plane crosses an edge, one new vertex is created and **shared** by
both faces on that edge — the crossing point is keyed by the undirected
original edge, so the result stays manifold along the cut rather than
splitting into a crack. No `faer`, no external dependency; Android-safe.

```rust
pub mod bisect { /* ... */ }
```

### Functions

#### Function `bisect`

Cut `mesh` by the plane through `point` with the given `normal`, keeping the
half where `normal · (x − point) <= 0`.

`normal` need not be unit length (only its sign and direction matter).
Faces straddling the plane are clipped; the exposed cut is left open (cap it
with [`crate::fill_holes::fill_holes`] for a closed solid). If the whole
mesh is on the kept side the mesh is returned unchanged; if none of it is,
an empty mesh is returned. This is infallible.

# Examples

```
use outram_blender::{primitives, bisect::bisect, fill_holes::fill_holes, math::Vec3};

// Slice a cube [-1,1]³ at z = 0, keeping the lower half, then cap it:
// the result is a closed box of half the volume.
let cube = primitives::cube(2.0);
let lower = bisect(&cube, Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0));
let solid = fill_holes(&lower);
assert_eq!(solid.euler_characteristic(), 2);
```

```rust
pub fn bisect(mesh: &crate::mesh::Mesh, point: crate::math::Vec3, normal: crate::math::Vec3) -> crate::mesh::Mesh { /* ... */ }
```

## Module `attributes`

**Marks & surface attributes** (`op-hzs.54.28`, GH issue #37 §E) — the
per-element attribute layer that `select_linked` delimiters (`op-hzs.54.2`),
`tris_to_quads` comparisons (`op-hzs.54.17`), `edge_split` (`op-hzs.54.18`),
`bevel`'s mark-seam/sharp (`op-hzs.54.9`) and `separate_by_material`
(`op-hzs.54.12`) were all deferred to.

[`MeshAttributes`] holds:

- `sharp` / `seam` — `BTreeSet<EdgeId>` marks.
- `crease` / `edge_bevel_weight` / `vertex_bevel_weight` — `f64` in `[0, 1]`.
- `smooth` — `BTreeSet<FaceId>` (shade-smooth; default is flat).
- `material` — per-face `usize` index (default `0`).
- `freestyle_edge` / `freestyle_face` — marks.

The keys are mesh indices, so a `MeshAttributes` goes stale when topology
changes exactly like any [`crate::mesh::VertexId`] — rebuild it alongside
the mesh.

[`MeshAttributes::auto_smooth`] derives `smooth` + `sharp` from the dihedral
angle (Blender's auto-smooth). [`MeshAttributes::linked_delimiters`] gives
the edge set `select_linked` should not cross.

```rust
pub mod attributes { /* ... */ }
```

### Types

#### Struct `MeshAttributes`

The full per-element attribute layer for one mesh.

```rust
pub struct MeshAttributes {
    pub sharp: std::collections::BTreeSet<crate::mesh::EdgeId>,
    pub seam: std::collections::BTreeSet<crate::mesh::EdgeId>,
    pub crease: std::collections::HashMap<crate::mesh::EdgeId, f64>,
    pub edge_bevel_weight: std::collections::HashMap<crate::mesh::EdgeId, f64>,
    pub vertex_bevel_weight: std::collections::HashMap<crate::mesh::VertexId, f64>,
    pub smooth: std::collections::BTreeSet<crate::mesh::FaceId>,
    pub material: std::collections::HashMap<crate::mesh::FaceId, usize>,
    pub freestyle_edge: std::collections::BTreeSet<crate::mesh::EdgeId>,
    pub freestyle_face: std::collections::BTreeSet<crate::mesh::FaceId>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `sharp` | `std::collections::BTreeSet<crate::mesh::EdgeId>` | Edges marked sharp (a hard shading edge; also a bevel/split seed). |
| `seam` | `std::collections::BTreeSet<crate::mesh::EdgeId>` | Edges marked as a UV seam. |
| `crease` | `std::collections::HashMap<crate::mesh::EdgeId, f64>` | Subdivision-surface crease per edge, `[0, 1]`. |
| `edge_bevel_weight` | `std::collections::HashMap<crate::mesh::EdgeId, f64>` | Bevel-modifier weight per edge, `[0, 1]`. |
| `vertex_bevel_weight` | `std::collections::HashMap<crate::mesh::VertexId, f64>` | Bevel-modifier weight per vertex, `[0, 1]`. |
| `smooth` | `std::collections::BTreeSet<crate::mesh::FaceId>` | Faces shaded smooth (default: flat). |
| `material` | `std::collections::HashMap<crate::mesh::FaceId, usize>` | Per-face material index (default: `0`). |
| `freestyle_edge` | `std::collections::BTreeSet<crate::mesh::EdgeId>` | Edges marked as a Freestyle edge. |
| `freestyle_face` | `std::collections::BTreeSet<crate::mesh::FaceId>` | Faces marked as a Freestyle face. |

##### Implementations

###### Methods

- ```rust
  pub fn new(_mesh: &Mesh) -> Self { /* ... */ }
  ```
  A fresh, empty attribute layer for `mesh` (everything at its default).

- ```rust
  pub fn mark_sharp(self: &mut Self, edges: &[EdgeId]) { /* ... */ }
  ```
  Mark `edges` sharp.

- ```rust
  pub fn clear_sharp(self: &mut Self, edges: &[EdgeId]) { /* ... */ }
  ```
  Clear the sharp mark from `edges`.

- ```rust
  pub fn is_sharp(self: &Self, e: EdgeId) -> bool { /* ... */ }
  ```
  Whether `e` is sharp.

- ```rust
  pub fn mark_seam(self: &mut Self, edges: &[EdgeId]) { /* ... */ }
  ```
  Mark `edges` as a seam.

- ```rust
  pub fn clear_seam(self: &mut Self, edges: &[EdgeId]) { /* ... */ }
  ```
  Clear the seam mark from `edges`.

- ```rust
  pub fn is_seam(self: &Self, e: EdgeId) -> bool { /* ... */ }
  ```
  Whether `e` is a seam.

- ```rust
  pub fn set_crease(self: &mut Self, edges: &[EdgeId], value: f64) { /* ... */ }
  ```
  Set the crease of `edges` to `value` (clamped to `[0, 1]`; `0` removes

- ```rust
  pub fn crease(self: &Self, e: EdgeId) -> f64 { /* ... */ }
  ```
  The crease of `e` (`0.0` if unset).

- ```rust
  pub fn set_edge_bevel_weight(self: &mut Self, edges: &[EdgeId], value: f64) { /* ... */ }
  ```
  Set the bevel weight of `edges` to `value` (clamped, `0` removes).

- ```rust
  pub fn edge_bevel_weight(self: &Self, e: EdgeId) -> f64 { /* ... */ }
  ```
  The bevel weight of `e` (`0.0` if unset).

- ```rust
  pub fn shade_smooth(self: &mut Self, mesh: &Mesh, faces: &[FaceId]) { /* ... */ }
  ```
  Shade `faces` smooth (empty = whole mesh).

- ```rust
  pub fn shade_flat(self: &mut Self, mesh: &Mesh, faces: &[FaceId]) { /* ... */ }
  ```
  Shade `faces` flat (empty = whole mesh).

- ```rust
  pub fn is_smooth(self: &Self, f: FaceId) -> bool { /* ... */ }
  ```
  Whether `f` is shaded smooth.

- ```rust
  pub fn set_material(self: &mut Self, faces: &[FaceId], index: usize) { /* ... */ }
  ```
  Set the material index of `faces`.

- ```rust
  pub fn material(self: &Self, f: FaceId) -> usize { /* ... */ }
  ```
  The material index of `f` (`0` if unset).

- ```rust
  pub fn auto_smooth(self: &mut Self, mesh: &Mesh, angle: f64) { /* ... */ }
  ```
  Auto-smooth: every face becomes smooth, and every interior edge whose

- ```rust
  pub fn linked_delimiters(self: &Self, mesh: &Mesh, by_seam: bool, by_sharp: bool, by_material: bool) -> BTreeSet<EdgeId> { /* ... */ }
  ```
  The edges `select_linked` must not cross, per the given delimiters

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> MeshAttributes { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> MeshAttributes { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `bevel`

**Bevel — Blender parity** (`op-hzs.54.9`, GH issue #37 §B).

[`bevel`] extends [`crate::edge_bevel`]'s flat single-segment chamfer with:

- **`segments`** — the edge gap is filled with `segments` quads whose
  cross-section follows a circular arc (a rounded edge), not one flat quad.
- **`profile`** — `0.0` chord (flat chamfer) … `0.5` circular … `1.0` bulged
  toward the original edge (sharp). Blender's profile slider.
- **`width_type`** — how `amount` is interpreted ([`WidthType`]). `Offset`
  is exact (distance each face is cut back); `Width` / `Depth` / `Percent`
  are right-angle approximations, since a headless call has no live dihedral.
- **`clamp_overlap`** — clamp the offset to half the shortest edge so a face
  cannot invert.

The **corner** where three or more beveled edges meet is filled with a
single n-gon cap (as in `edge_bevel`); a rounded spherical-triangle corner
patch is tracked as follow-up under this bead. Every edge is beveled — a
*selected-subset* bevel needs partial-boundary handling and is also
follow-up.

```rust
pub mod bevel { /* ... */ }
```

### Types

#### Enum `WidthType`

How [`BevelOptions::amount`] is measured.

```rust
pub enum WidthType {
    Offset,
    Width,
    Depth,
    Percent,
}
```

##### Variants

###### `Offset`

Distance each adjacent face is moved back from the edge (exact).

###### `Width`

Width of the new bevel face (≈ `offset · √2` at a right angle).

###### `Depth`

Perpendicular distance from the original edge to the new face
(≈ `offset / √2` at a right angle).

###### `Percent`

Percentage (0–100) of the mean adjacent edge length.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> WidthType { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &WidthType) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `BevelOptions`

Tuning for [`bevel`].

```rust
pub struct BevelOptions {
    pub amount: f64,
    pub segments: usize,
    pub profile: f64,
    pub width_type: WidthType,
    pub clamp_overlap: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `amount` | `f64` | Bevel size, interpreted per [`BevelOptions::width_type`]. |
| `segments` | `usize` | Number of quad rings across the bevel (`>= 1`). `1` = flat chamfer. |
| `profile` | `f64` | Profile shape, `0.0` … `1.0` (`0.5` = circular). |
| `width_type` | `WidthType` | How `amount` is measured. |
| `clamp_overlap` | `bool` | Clamp the offset to half the shortest edge so no face inverts. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> BevelOptions { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `bevel`

Bevel every edge of `mesh` per `opts`.

```rust
pub fn bevel(mesh: &crate::mesh::Mesh, opts: BevelOptions) -> crate::mesh::Mesh { /* ... */ }
```

## Module `boolean`

Mesh boolean / CSG operator — **entry point + exact convex fast path**.

Blender analogue: `bmesh/tools/bmesh_boolean` / the `bmo_boolean` operator,
which upstream is backed by the **Manifold** library. [`boolean`] is the
single public entry point for all three CSG modes; it dispatches between two
implementations:

- **Exact convex fast path (this module).** When both operands are convex
  and the mode is [`crate::ops::BooleanMode::Intersect`], the intersection is
  computed here by **half-space clipping** — an exact, non-triangulated
  result (a clean n-gon mesh).
- **General arrangement path** ([`crate::boolean_general`]). Union,
  Difference, and any **non-convex** operand are handled there, by cutting
  the two surfaces against each other and classifying the resulting patches
  with the generalized winding number. See that module for its contract and
  limitations (coplanar-overlap rejection, generic-position assumption,
  triangulated output).

## The convex fast path, in detail

Every face of operand `B` defines an outward half-space

- plane through [`Mesh::face_centroid`] `c` with outward unit normal
  [`Mesh::face_normal`] `n`;
- the solid interior of `B` is the set of points `p` with
  `dot(n, p - c) <= 0` for **every** face of `B`.

Operand `A`'s convex polytope is clipped against each of `B`'s face
half-spaces in turn (successive 3-D Sutherland–Hodgman polygon clipping):
for each clip plane every face polygon of the running result is clipped to
the inside half-space, the segments where faces cross the plane are collected,
and a fresh **cap** face is built from that loop of cut points (ordered by
angle about the plane normal, wound so its outward normal matches the clip
plane). The result is the convex intersection `A ∩ B` — a valid closed convex
mesh — for which Euler's identity `V - E + F = 2` holds.

A **non-overlapping / empty** convex intersection returns
[`BooleanError::Unsupported`] rather than an empty mesh, so a caller cannot
mistake "no overlap" for a valid degenerate solid. (When the operands are
*not* both convex, `Intersect` never reaches this path — it goes to the
general pipeline, which handles the empty/disjoint case there.)

> **Untrusted AI-generated draft** until a human reviews it, per the workspace
> `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control,
> safety-critical, or licensing decisions.

```rust
pub mod boolean { /* ... */ }
```

### Types

#### Enum `BooleanError`

Errors from a mesh boolean operation.

The only variant is [`BooleanError::Unsupported`], carrying a short static
reason. It is returned for the inputs neither the convex fast path nor the
general arrangement pipeline can resolve (see the module docs and
[`crate::boolean_general`]) — an honest signal, never a silently wrong mesh.

```rust
pub enum BooleanError {
    Unsupported(&'static str),
}
```

##### Variants

###### `Unsupported`

The boolean could not be resolved: an empty/non-overlapping intersection,
a **coplanar overlapping face** between the operands, an
exactly-degenerate arrangement, or a result that welds to fewer than four
faces. The `&'static str` names which.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `&'static str` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Error**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(source: crate::boolean::BooleanError) -> Self { /* ... */ }
    ```

  - ```rust
    fn from(source: crate::boolean::BooleanError) -> Self { /* ... */ }
    ```

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `boolean`

Compute the boolean of two closed meshes under `mode`.

Single entry point for all three CSG modes, dispatching between the exact
convex fast path (this module) and the general arrangement pipeline
([`crate::boolean_general`]) — see the module-level docs. In summary:

- [`BooleanMode::Intersect`] on two **convex** closed meshes returns the
  convex intersection `A ∩ B` (a valid closed mesh, `V - E + F = 2`), computed
  exactly by half-space clipping of `a` against every face half-space of `b`.
- [`BooleanMode::Union`], [`BooleanMode::Difference`], and any **non-convex**
  operand are computed by [`crate::boolean_general::boolean_general`]
  (surface arrangement + winding classification; triangulated output).
- An **empty / non-overlapping** intersection, or a coplanar-overlap /
  degenerate arrangement input, returns [`BooleanError::Unsupported`] rather
  than a silently wrong mesh.

Both meshes are dimensionless model-space geometry (see [`crate::math`]);
`mode` selects the CSG operation. Operands are taken by shared reference and
are not modified.

```rust
pub fn boolean(a: &crate::mesh::Mesh, b: &crate::mesh::Mesh, mode: crate::ops::BooleanMode) -> Result<crate::mesh::Mesh, BooleanError> { /* ... */ }
```

## Module `boolean_classify`

Inside/outside point classification for a closed triangle mesh.

This is the primitive a **general** mesh boolean needs to decide which
surface patches of the arrangement to keep: once two operand surfaces are
cut against each other, each resulting patch is kept or discarded by
sampling a point near it and asking "is this point inside the *other*
solid?" [`crate::boolean`]'s current convex half-space clipper does not
need this (a convex clip never has to ask "which side wins"), but the
honest `Unsupported` Union/Difference path documented there is exactly
the gap this module is a building block for.

Algorithm reference: Blender `mesh_boolean.cc`
(github.com/blender/blender @ 96294be75080bbf687fa7f108e344a1063713586,
GPL-2.0-or-later) — inside/outside classification concept (it tracks a
per-shape *winding number* per arrangement cell and applies a boolean-op
predicate to it — see its `Cell::winding_`, `propagate_windings_and_in_output_volume`,
and `apply_bool_op`). This module does **not** transcribe that code: it
implements the standalone **generalized winding number** point-in-solid
test (Jacobson, Kavan, Sorkine-Hornung 2013) from first principles, plus a
textbook ray-casting parity check as an independent cross-check. The
`closest_point_on_triangle` helper is the well-known point/triangle
closest-point algorithm (Ericson, *Real-Time Collision Detection*,
§5.1.5) — public-domain-style textbook algorithm, not Blender code.

## The two techniques

- [`winding_number`] — sum, over every triangle of the (fan-triangulated)
  mesh, of the **signed solid angle** that triangle subtends at query
  point `p` (Van Oosterom–Strackee formula), divided by `4*pi`. For a
  closed, outward-oriented, manifold surface this is (very close to) an
  **integer**: `0` outside, `+-1` inside a simply-connected solid (a
  surface wound inward, or a self-overlapping input, can in principle
  produce other integers — see Limitations). It degrades gracefully
  (continuously, not catastrophically) even for meshes with small gaps or
  local non-manifoldness, which is *why* Blender's own approach and this
  one both lean on the same underlying idea.
- a private ray-parity helper (Möller–Trumbore ray/triangle intersection,
  odd number of crossings along a ray from `p` to infinity ⇒ inside) —
  included as an independent numerical cross-check exercised in the test
  suite, not as the primary classifier.

## [`classify_point`] tolerance choices

All tolerances below are **relative to the mesh's bounding-box diagonal**
(`mesh_scale`, the same pattern `boolean::intersect_convex` uses
for `clip_eps`/`weld`) so they hold at any model scale, not just
unit-sized test meshes:

- **`OnBoundary` detection** (`ON_BOUNDARY_REL_EPS = 1e-6`): `p` is
  `OnBoundary` if its distance to the *closest point on any triangle* is
  `<= 1e-6 * mesh_scale`. This is checked **before** computing the winding
  number, because a point essentially on the surface makes individual
  solid-angle terms numerically unstable (a near-zero denominator in the
  Van Oosterom–Strackee formula) even though the *sum* would still limit
  correctly for an exactly-on-surface point in exact arithmetic. `1e-6` is
  looser than `boolean.rs`'s `1e-7` convexity tolerance because a
  point-to-triangle closest-point query chains more floating-point
  operations (six dot products plus divisions) than a single plane
  distance, so it accumulates more rounding error.
- **`Inside`/`Outside` decision** (`|winding_number| > 0.5`): not a
  scale-dependent epsilon at all — for a point that is not on the surface,
  the winding number of a closed manifold is within numerical noise of an
  integer, so `0.5` is the natural half-way decision boundary with a huge
  safety margin (noise is typically `< 1e-6`, not anywhere near `0.5`).

## Limitations (read before trusting this on new geometry)

- **Requires a closed, manifold, consistently outward-oriented surface.**
  On an **open** mesh (a hole in the surface) the winding number varies
  continuously across the hole instead of jumping between integers, so
  `classify_point` can return a confident-looking `Inside`/`Outside` that
  is meaningless. This module does **not** check watertightness/manifoldness
  itself — callers must ensure the input is closed (e.g. everything
  [`crate::primitives`] generates, or `boolean::intersect_convex`'s
  output).
- **Fan triangulation of n-gons is inline and naive** (`(v0, v_i, v_{i+1})`
  for `i = 1..n-1`): correct for the convex faces every generator in this
  crate produces, but a **non-convex** n-gon can fan-triangulate into
  triangles that overlap or fold outside the polygon, silently corrupting
  the classification. There is no general n-gon triangulator here.
- **Points exactly on the surface are inherently a hard case in floating
  point.** `OnBoundary` detection is a distance threshold, not an exact
  predicate; a point that is mathematically on the surface but lands just
  outside `ON_BOUNDARY_REL_EPS` due to input coordinate rounding will be
  classified `Inside` or `Outside` instead, and (rarely) a point that is
  merely *very close* to the surface without being on it can be
  misclassified `OnBoundary`. No epsilon-based test can avoid this
  trade-off; exact/rational arithmetic would be needed to eliminate it.
- The private ray-parity cross-check is deliberately **not** the primary
  classifier: a ray that grazes a shared edge between two triangles can
  double-count or miss a crossing. It defends against this by trying a
  fixed list of non-axis-aligned, mutually non-parallel directions and
  rejecting any direction whose barycentric hit coordinates land within
  `1e-9` of a triangle's edge (signalling "try another direction"); if
  *every* candidate direction is degenerate against a given mesh (never
  observed in this module's tests, but not provably impossible for
  adversarial input) it falls back to a majority vote across all
  candidates rather than panicking. It exists to validate
  [`winding_number`] in tests, not as a second production API.

> **Untrusted AI-generated draft until human-reviewed, per
> `RESPONSIBLE_USE.md`** — not for safety-critical use. Verified so far
> only against the analytic primitives ([`crate::primitives::cube`],
> [`crate::primitives::uv_sphere`]) and a hand-built concave L-prism (see
> tests below); not yet validated against arbitrary imported/scanned
> geometry.

```rust
pub mod boolean_classify { /* ... */ }
```

### Types

#### Enum `PointClass`

Result of classifying a point against a closed mesh's solid interior.

```rust
pub enum PointClass {
    Inside,
    Outside,
    OnBoundary,
}
```

##### Variants

###### `Inside`

The point is in the solid's interior (winding number `~= +-1`, away
from the surface).

###### `Outside`

The point is outside the solid (winding number `~= 0`, away from the
surface).

###### `OnBoundary`

The point lies on (within `ON_BOUNDARY_REL_EPS` * the mesh's
bounding-box diagonal of) the mesh surface itself — neither cleanly
inside nor outside. See the module docs' "Limitations" section for why
this is an epsilon-based judgement call, not an exact predicate.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> PointClass { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PointClass) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `winding_number`

The generalized winding number of `p` with respect to `mesh`'s (fan-
triangulated) surface.

This is `(1 / 4*pi)` times the sum, over every triangle, of the **signed
solid angle** that triangle subtends at `p`, computed per-triangle with
the Van Oosterom–Strackee formula:

`solid_angle = 2 * atan2(ra . (rb x rc), |ra||rb||rc| + (ra.rb)|rc| + (rb.rc)|ra| + (rc.ra)|rb|)`

where `ra, rb, rc` are the triangle's vertices relative to `p`
(`vertex - p`). Summed over a **closed**, outward-wound surface this
telescopes to (very close to) an integer: `0` for `p` outside the solid,
`+1` for `p` inside a simply-connected solid whose faces wind
counter-clockwise as seen from outside (the convention every
[`crate::primitives`] generator and [`crate::mesh::Mesh::face_normal`]
use) — see [`PointClass`] and the module docs' "Limitations" for what can
make this not hold (open meshes, inconsistent winding, self-overlap).

A triangle whose relative vertex is (numerically) coincident with `p`
(any of `|ra|, |rb|, |rc|` under `1e-12`) is skipped rather than dividing
by a near-zero denominator; [`classify_point`] avoids this case in the
first place by checking [`PointClass::OnBoundary`] before calling this
function, but `winding_number` stays well-defined (if slightly
approximate) for a caller that invokes it directly on a near-surface `p`.

```rust
pub fn winding_number(mesh: &crate::mesh::Mesh, p: crate::math::Vec3) -> f64 { /* ... */ }
```

#### Function `classify_point`

Classify `p` against `mesh`'s solid interior.

First checks [`PointClass::OnBoundary`] (distance to the closest point on
any triangle `<= ON_BOUNDARY_REL_EPS * mesh_scale`); if not on the
boundary, classifies by [`winding_number`]: `|w| > 0.5` is `Inside`,
otherwise `Outside`. See the module docs for the reasoning behind both
tolerances and the closed-manifold assumption this relies on.

```rust
pub fn classify_point(mesh: &crate::mesh::Mesh, p: crate::math::Vec3) -> PointClass { /* ... */ }
```

## Module `boolean_general`

General mesh boolean — Union / Difference / Intersect on arbitrary closed
meshes, by **surface arrangement + generalized-winding-number
classification**.

This is the "robust route" the restricted convex clipper in
[`crate::boolean`] deferred to: it lifts the boolean out of the
convex-`Intersect`-only case and handles **non-convex** (and self-contained
genus-`g`) operands in **all three modes**. It is the pipeline the boolean
*foundation* modules were built for — [`crate::boolean_predicates`] (robust
Shewchuk orientation) supplies the exact-sign tests that make the
retriangulation stable, and [`crate::boolean_classify`] (generalized winding
number) supplies the inside/outside decision.

## Provenance / reference

Architecture reference: Blender `mesh_boolean.cc` / `mesh_intersect.cc`
(github.com/blender/blender @ 96294be75080bbf687fa7f108e344a1063713586,
GPL-2.0-or-later; GPLv3-compatible per the workspace provenance rule). This
module does **not** transcribe Blender's code — Blender builds an exact
(rational-arithmetic) arrangement of the two surfaces, tracks a per-shape
*winding number* on each arrangement cell, and keeps a face when its two
adjacent cells disagree on membership in the output volume (see Blender's
`apply_bool_op` / `propagate_windings_and_in_output_volume`). We implement
the same *idea* with a self-contained, C-dependency-free pipeline suitable
for this crate (Android-buildable, no GMP): cut each operand's triangles
along the other surface, then classify each resulting patch by the winding
number of the *other* operand and keep/flip/drop it per the boolean op.

The keep/flip/drop rules below are the two-operand specialization of
Blender's winding test (`winding[0]`/`winding[1]` = inside operand A / B):

| Op | A-patch kept when | B-patch kept when | B-patch winding |
|---|---|---|---|
| Union      | outside B | outside A | preserved |
| Intersect  | inside B  | inside A  | preserved |
| Difference | outside B | inside A  | **flipped** |

(Union = "in either" ⇒ each operand's surface survives only where it is not
already buried inside the other; Intersect = "in both"; Difference `A \ B` =
"in A and not in B" ⇒ A's surface survives outside B, and B's surface,
flipped to face outward from the removed cavity, survives inside A.)

## The pipeline (per call)

1. **Triangulate** both operands (fan triangulation of each face).
2. **Intersect** every triangle of A against every triangle of B; each
   intersecting pair contributes one **segment** lying on both triangles.
   The *same* segment (identical `f64` endpoints) is handed to A's triangle
   *and* B's triangle, so the two operands' cut curves are numerically
   identical — the output welds watertight along the seam.
3. **Retriangulate** each cut triangle, constrained to its segments, via a
   2D **constrained Delaunay triangulation** (`triangulate_constrained`,
   Bowyer–Watson + Anglada edge-insertion using the robust
   [`crate::boolean_predicates::orient2d`] / `incircle`). Uncut triangles
   pass through whole.
4. **Classify** each resulting sub-triangle by the winding number of the
   *other* operand at its centroid, and **select** (keep / keep-flipped /
   drop) per the table above.
5. **Weld** the kept triangles into one closed [`Mesh`].

## Why the constrained triangulation is well-conditioned here

The intersection of two **closed** surfaces is a set of closed 1-manifold
loops, so *within a single triangle* the constraint segments never cross
transversally — they meet only end-to-end where the curve passes between
adjacent triangles. That non-crossing property is what makes the
Anglada flip-insertion terminate cleanly (no need for Steiner points at
interior crossings).

## Limitations (read before trusting on new geometry — honest, not fake-green)

- **Generic position assumed.** Two operands that share a **coplanar
  overlapping face** are rejected with [`BooleanError::Unsupported`]
  ("coplanar faces") rather than guessed at — a coplanar overlap has no
  transverse intersection curve to cut along, so the winding classifier
  cannot resolve which coplanar patch wins. (Coplanarity *within* one
  operand is fine; only A-face-coplanar-with-B-face triggers this.)
- **Exactly-degenerate shared geometry** (a vertex of A exactly on a face of
  B, an edge of A exactly along an edge of B) can make a triangle pair's
  crossing ill-defined; such a pair is skipped (its cut is dropped) rather
  than forced. Nudging one operand by an irrational epsilon avoids it. This
  is the same class of hard case [`crate::boolean_classify`] documents for
  on-surface points.
- **Output is triangulated**, not merged back into co-planar n-gons. The
  Euler characteristic is still correct (triangulation-invariant), and
  downstream solvers triangulate anyway, but the face count is higher than a
  minimal polygonal result.
- **Requires closed, manifold, outward-oriented operands** — inherited from
  [`crate::boolean_classify::winding_number`]. Everything
  [`crate::primitives`] generates satisfies this; imported/scanned meshes
  must be repaired first.

> **Untrusted AI-generated draft** until a human reviews it, per the
> workspace `RESPONSIBLE_USE.md`. Not for nuclear facility operation,
> reactor control, safety-critical, or licensing decisions.

```rust
pub mod boolean_general { /* ... */ }
```

### Functions

#### Function `boolean_general`

Compute the boolean `A op B` of two **general closed** meshes by surface
arrangement + winding classification.

Handles [`BooleanMode::Union`], [`BooleanMode::Difference`], and
[`BooleanMode::Intersect`] on non-convex (but closed, manifold,
outward-oriented) operands — see the module docs for the full contract, the
keep/flip/drop rules, and the limitations (coplanar overlap, exact
degeneracy). Returns [`BooleanError::Unsupported`] for a coplanar-overlap
input or a result that welds to fewer than four faces (empty / degenerate).

Both meshes are dimensionless model-space geometry; `mode` selects the CSG
operation. Operands are taken by shared reference and are not modified.

```rust
pub fn boolean_general(a: &crate::mesh::Mesh, b: &crate::mesh::Mesh, mode: crate::ops::BooleanMode) -> Result<crate::mesh::Mesh, crate::boolean::BooleanError> { /* ... */ }
```

## Module `boolean_predicates`

Robust geometric predicates — orientation and in-circle/in-sphere tests.

Blender analogue / provenance: ported from `blender/blenlib`
`BLI_math_boolean.hh` / `intern/math_boolean.cc`, upstream repo
`github.com/blender/blender`, commit
`96294be75080bbf687fa7f108e344a1063713586`.

```text
SPDX-FileCopyrightText: 2023 Blender Authors
SPDX-License-Identifier: GPL-2.0-or-later
```

Adapted to pure Rust for `outram-blender` (GPL-3.0-only); GPL-2.0-or-later
is GPL-3.0-compatible per the workspace provenance rule. Only the `double`
(floating-point) predicate API is ported here — Blender's `mpq_class`
(GMP rational) overloads are intentionally **not** ported: this crate must
stay Android-buildable with **no C dependencies**, and GMP is a C library.

Blender's own `double` predicates are, in turn, a C++ adaptation of
Jonathan Shewchuk's `predicates.c` — "Routines for Arbitrary Precision
Floating-point Arithmetic and Fast Robust Geometric Predicates", placed in
the **public domain** by Jonathan Richard Shewchuk (Carnegie Mellon
University, May 1996). The error-bound coefficients used below
(`CCW_ERR_BOUND_A`, `O3D_ERR_BOUND_A`, `ICC_ERR_BOUND_A`) are taken
directly from Shewchuk's `exactinit()` derivation as reproduced in
Blender's `math_boolean.cc`.

## Robustness contract — what is actually implemented here

This is **not** a line-for-line port of Shewchuk's multi-stage adaptive
expansion arithmetic (`orient2dadapt`/`orient3dadapt`/`incircleadapt`,
which build growing exact "expansions" out of `double[]` arrays and only
spend as much precision as each case needs). That algorithm is correct
and fast, but reproducing its ~150-1000 lines of index-juggling C macros
by hand for `orient3d`/`incircle` carries real risk of a transcription bug
that silently produces a *wrong* sign — worse than an honest, simpler
scheme. Instead, each predicate here uses a **two-stage filter**:

1. **Fast path** — compute the determinant in plain `f64` (the `_fast`
   variant's formula) together with Shewchuk's rigorous `permanent`-based
   error bound (`errbound = C * permanent`, using the *exact* coefficients
   `C` from his error analysis). If `|det| > errbound`, the `f64` result's
   **sign is provably correct** — this stage is the real Shewchuk
   algorithm, not a simplification.
2. **Refine path** — if the fast filter is inconclusive (the near-
   degenerate case), recompute the *same* determinant formula using
   **double-double (`Dd`) arithmetic**: each `f64` is carried as an exact
   `(hi, lo)` pair via error-free transformations (Knuth's `two_sum`,
   FMA-based `two_prod`), giving roughly 106 bits of mantissa instead of
   53. The sign of the double-double result is returned.

Stage 2 is the **simplified/partial** part of this port, relative to
Shewchuk/Blender's full adaptive-precision expansion arithmetic:
double-double arithmetic is *not* arbitrary precision. It resolves the
sign correctly for the vast majority of practical near-degenerate
configurations (anything not degenerate below roughly the 106th
significant bit), and the `tests` module below demonstrates a concrete
case where the `_fast` plain-`f64` path returns the wrong sign and the
double-double-refined path returns the correct one. But it is not a
mathematical guarantee of exactness for *arbitrarily* degenerate
adversarial inputs (e.g. points constructed via exact expansion
arithmetic to be non-zero only at the 107th+ bit) — a full Shewchuk/CGAL-
style expansion port would be needed for that. This limitation is
intentional and documented rather than hidden.

[`insphere`] goes one step further in caution: deriving Shewchuk's precise
`isperrboundA` permanent formula correctly (it involves six 2x2
sub-determinants folded into four 3x3 cofactors) is easy to get subtly
wrong from a partial reading of the reference source, and a wrong
permanent bound could make the fast-path filter *unsoundly* trust an
incorrect `f64` sign. So [`insphere`] skips the fast-path filter entirely
and **always** evaluates via double-double arithmetic — slower, but never
unsoundly fast. This is called out again on the function itself.

## Sign convention

All predicates return a plain `i32` in `{-1, 0, +1}` (matching Blender's
`double` predicate signatures exactly), rather than an enum, so callers
can use ordinary integer comparisons/arithmetic on the result the same way
Blender's own callers do.

- [`orient2d`]/[`orient2d_fast`]: `+1` if `a, b, c` occur counter-clockwise,
  `-1` clockwise, `0` collinear.
- [`orient3d`]/[`orient3d_fast`]: `+1` if `d` lies *below* the plane through
  `a, b, c` (with `a, b, c` counter-clockwise when viewed from above that
  plane), `-1` above, `0` coplanar.
- [`incircle`]/[`incircle_fast`]: `+1` if `d` lies inside the circle
  through `a, b, c` (which must be given counter-clockwise, or the sign
  reverses), `-1` outside, `0` co-circular.
- [`insphere`]/[`insphere_fast`]: `+1` if `e` lies inside the sphere
  through `a, b, c, d` (which must be positively oriented per
  [`orient3d`], or the sign reverses), `-1` outside, `0` co-spherical.

> **Untrusted AI-generated draft** until a human reviews it, per the
> workspace `RESPONSIBLE_USE.md`. Not for nuclear facility operation,
> reactor control, safety-critical, or licensing decisions.

```rust
pub mod boolean_predicates { /* ... */ }
```

### Types

#### Struct `Vec2`

A 2-component vector, `f64` `x`/`y`.

Local to this module rather than added to [`crate::math`] — the boolean
predicates are the only 2D consumer in the crate today (see the task that
created this file). Dimensionless model-space coordinates, exactly like
[`crate::math::Vec3`].

```rust
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `f64` | X component. |
| `y` | `f64` | Y component. |

##### Implementations

###### Methods

- ```rust
  pub const fn new(x: f64, y: f64) -> Self { /* ... */ }
  ```
  Construct a vector from explicit components.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Vec2 { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Vec2) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `orient2d_fast`

Plain-`f64` 2D orientation test. **Not robust** — near-collinear inputs
can silently return the wrong sign (or `0`) due to floating-point
cancellation. Use [`orient2d`] unless the caller has already established
the points are well away from collinear.

Returns `+1` if `a, b, c` occur counter-clockwise, `-1` if clockwise, `0`
if (numerically) collinear. The magnitude of the underlying determinant is
twice the signed area of triangle `abc`.

```rust
pub fn orient2d_fast(a: Vec2, b: Vec2, c: Vec2) -> i32 { /* ... */ }
```

#### Function `orient2d`

Robust 2D orientation test — see the module-level robustness contract.

Returns `+1` if `a, b, c` occur counter-clockwise, `-1` if clockwise, `0`
if *exactly* (to double-double precision) collinear. Correct even when
[`orient2d_fast`] would flip sign or misreport `0` due to cancellation —
see `tests::orient2d_fast_gets_near_collinear_case_wrong` /
`tests::orient2d_robust_resolves_the_same_case_correctly` below for a
concrete demonstration.

```rust
pub fn orient2d(a: Vec2, b: Vec2, c: Vec2) -> i32 { /* ... */ }
```

#### Function `orient3d_fast`

Plain-`f64` 3D orientation test. **Not robust** — see [`orient2d_fast`]'s
caveat; the same cancellation risk applies in 3D. Use [`orient3d`] unless
the caller has already established the points are well away from
coplanar.

Returns `+1` if `d` lies below the plane through `a, b, c` (with `a, b, c`
counter-clockwise viewed from above that plane), `-1` if above, `0` if
(numerically) coplanar. The magnitude of the underlying determinant is six
times the signed volume of tetrahedron `abcd`.

```rust
pub fn orient3d_fast(a: crate::math::Vec3, b: crate::math::Vec3, c: crate::math::Vec3, d: crate::math::Vec3) -> i32 { /* ... */ }
```

#### Function `orient3d`

Robust 3D orientation test — see the module-level robustness contract.

Returns `+1` if `d` lies below the plane through `a, b, c` (with `a, b, c`
counter-clockwise viewed from above), `-1` if above, `0` if *exactly* (to
double-double precision) coplanar.

```rust
pub fn orient3d(a: crate::math::Vec3, b: crate::math::Vec3, c: crate::math::Vec3, d: crate::math::Vec3) -> i32 { /* ... */ }
```

#### Function `incircle_fast`

Plain-`f64` 2D in-circle test. **Not robust** — see [`orient2d_fast`]'s
caveat; the same cancellation risk applies here. Use [`incircle`] unless
the caller has already established `d` is well away from the circle
through `a, b, c`.

Returns `+1` if `d` lies inside the circle through `a, b, c` (which must
be given counter-clockwise, or the sign reverses), `-1` if outside, `0` if
(numerically) co-circular.

```rust
pub fn incircle_fast(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> i32 { /* ... */ }
```

#### Function `incircle`

Robust 2D in-circle test — see the module-level robustness contract.

Returns `+1` if `d` lies inside the circle through `a, b, c` (which must
be given counter-clockwise, or the sign reverses), `-1` if outside, `0` if
*exactly* (to double-double precision) co-circular.

```rust
pub fn incircle(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> i32 { /* ... */ }
```

#### Function `insphere_fast`

Plain-`f64` 3D in-sphere test. **Not robust** — see [`orient2d_fast`]'s
caveat. Use [`insphere`] unless the caller has already established `e` is
well away from the sphere through `a, b, c, d`.

Returns `+1` if `e` lies inside the sphere through `a, b, c, d` (which
must be positively oriented per [`orient3d`], or the sign reverses), `-1`
if outside, `0` if (numerically) co-spherical.

```rust
pub fn insphere_fast(a: crate::math::Vec3, b: crate::math::Vec3, c: crate::math::Vec3, d: crate::math::Vec3, e: crate::math::Vec3) -> i32 { /* ... */ }
```

#### Function `insphere`

Robust 3D in-sphere test.

Returns `+1` if `e` lies inside the sphere through `a, b, c, d` (which
must be positively oriented per [`orient3d`], or the sign reverses), `-1`
if outside, `0` if *exactly* (to double-double precision) co-spherical.

**Unlike [`orient2d`]/[`orient3d`]/[`incircle`] above, this always
evaluates in double-double precision — there is no `f64` fast-path
filter.** Shewchuk's real `isperrboundA`/permanent formula for insphere
folds together six 2x2 sub-determinants (`ab`, `bc`, `cd`, `da`, `ac`,
`bd`) into four 3x3 cofactors before the final 4-term combination; getting
that error-bound derivation subtly wrong from a partial read of the
reference source is a real risk, and a *too-tight* bound would make the
fast path unsoundly trust a wrong `f64` sign — silently worse than doing
no filtering at all. So this implementation is deliberately conservative:
always pay the double-double cost, never risk an unsound fast path. A
future contributor who re-derives and carefully verifies the exact
`isperrboundA` permanent formula against Shewchuk's source can add the
fast path the same way [`orient3d`]/[`incircle`] do.

```rust
pub fn insphere(a: crate::math::Vec3, b: crate::math::Vec3, c: crate::math::Vec3, d: crate::math::Vec3, e: crate::math::Vec3) -> i32 { /* ... */ }
```

## Module `bridge`

**Bridge Edge Loops** (`op-hzs.54.14`, GH issue #37 §B) — join two edge
loops with a face strip. Blender's `Edge ▸ Bridge Edge Loops`.

[`bridge_edge_loops`] takes the two loops as ordered vertex rings (open or
closed) plus [`BridgeOptions`]:

- `twist` — rotate the pairing between the two rings by this many steps
  (Blender's Twist).
- `cuts` — insert `cuts` intermediate rings, so the bridge is `cuts + 1`
  quads deep (linear interpolation).
- `flip` — reverse ring B's direction (fixes an inside-out strip).
- `merge_ends` — if a paired vertex on each ring is within a tolerance,
  weld them (bridging a loop back onto itself).

The two rings must have the **same** vertex count; unequal-count bridging
(Blender interpolates) is tracked as follow-up under this bead.

```rust
pub mod bridge { /* ... */ }
```

### Types

#### Struct `BridgeOptions`

Tuning for [`bridge_edge_loops`].

```rust
pub struct BridgeOptions {
    pub twist: i64,
    pub cuts: usize,
    pub flip: bool,
    pub merge_distance: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `twist` | `i64` | Rotate the A↔B vertex pairing by this many steps. |
| `cuts` | `usize` | Intermediate rings inserted along the bridge (`0` = one quad deep). |
| `flip` | `bool` | Reverse ring B before pairing. |
| `merge_distance` | `f64` | Weld paired vertices closer than this distance (`0` disables). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> BridgeOptions { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `bridge_edge_loops`

Bridge `ring_a` to `ring_b` (both ordered vertex rings of equal length),
appending the connecting faces to `mesh`. `closed` says whether the rings
are cyclic (a tube) or open (a ribbon).

```rust
pub fn bridge_edge_loops(mesh: &crate::mesh::Mesh, ring_a: &[crate::mesh::VertexId], ring_b: &[crate::mesh::VertexId], closed: bool, opts: BridgeOptions) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `ordered_ring`

Walk an edge set into its ordered vertex ring, or `None` if it is not a
single simple chain / loop. `closed` in the result says whether it cycles.

```rust
pub fn ordered_ring(mesh: &crate::mesh::Mesh, edges: &[crate::mesh::EdgeId]) -> Option<(Vec<crate::mesh::VertexId>, bool)> { /* ... */ }
```

#### Function `align_by_nearest`

Convenience: reorder `ring_b` so its first vertex is the one geometrically
nearest `ring_a[0]` — a reasonable default pairing before applying `twist`.

```rust
pub fn align_by_nearest(mesh: &crate::mesh::Mesh, ring_a: &[crate::mesh::VertexId], ring_b: &[crate::mesh::VertexId]) -> Vec<crate::mesh::VertexId> { /* ... */ }
```

## Module `connect`

**Connect Vertex Path / Pairs** (`op-hzs.54.16`, GH issue #37 §B) —
Blender's `J`.

- [`connect_vertex_path`] connects an ordered vertex list: each consecutive
  pair that shares a face splits that face along the chord.
- [`connect_vertex_pairs`] connects an explicit list of pairs.

Both compose [`crate::knife::knife`]'s face-chord split, so they inherit its
"one chord per face, applied in sequence" behaviour. A pair that shares no
face, or is already an edge, is skipped.

```rust
pub mod connect { /* ... */ }
```

### Functions

#### Function `connect_vertex_path`

Connect the ordered `path` of vertices: for each consecutive pair sharing a
face, split that face along the chord between them. Returns the rebuilt
mesh.

```rust
pub fn connect_vertex_path(mesh: &crate::mesh::Mesh, path: &[crate::mesh::VertexId]) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `connect_vertex_pairs`

Connect an explicit list of vertex `pairs`. Order matters when several pairs
touch one face (each acts on whichever sub-face contains it).

```rust
pub fn connect_vertex_pairs(mesh: &crate::mesh::Mesh, pairs: &[(crate::mesh::VertexId, crate::mesh::VertexId)]) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `common_face`

A face incident to **both** `a` and `b` (the first by id), or `None`.

```rust
pub fn common_face(mesh: &crate::mesh::Mesh, a: crate::mesh::VertexId, b: crate::mesh::VertexId) -> Option<crate::mesh::FaceId> { /* ... */ }
```

## Module `connect_concave`

Split concave faces into convex ones — a port of Blender's **Split
Concave Faces** (`bmo_connect_concave.cc`).

# Why a solver frontend wants this

A concave face's centroid lies outside the face. Anything downstream
that treats a face as "a centroid plus a normal plus an area" — which is
most of a finite-volume mesher's face handling, and the CSG bridge's
surface test — is then working from a point that is not on the surface
it is meant to describe. Splitting concave faces into convex pieces
removes that whole class of surprise, and it does so **without moving a
single vertex**: only edges are added.

This is the third member of the face-quality trio, alongside
[`crate::connect_nonplanar`] (warped faces, split) and
[`crate::planar_faces`] (warped faces, relaxed).

# The algorithm: shatter, then glue back while convex

Upstream does not search for good cuts directly. It triangulates the
concave face outright and then greedily merges the triangles back
together, accepting each merge only while the result stays convex. What
survives is a set of maximal convex pieces.

The merge *order* is where the quality comes from, and it is upstream's,
transcribed:

1. Interior edges whose **both** endpoints are concave corners of the
   original face are considered **last**. Those edges are the natural
   dividers between the face's convex lobes, so leaving them for last
   means they are still there to be kept.
2. Otherwise, **longer edges first** — upstream's comment is "shortest
   edges last". Merging across a long edge early tends to produce the
   larger, better-shaped pieces.

# Fidelity to upstream, and the deviations

The triangulate-then-remerge structure, the concave-corner tagging, the
sort comparator and the convexity gate are transcribed. Differences:

1. **`f64`, not `f32`.**
2. **The convexity gate checks the whole merged ring**, where upstream
   checks only the two corners the merge creates (`cross_tri_v3` against
   the face normal, once per side). The two are equivalent — the other
   corners were already convex — and upstream's is `O(1)` against this
   port's `O(n)`. Chosen for legibility; if this ever shows up in a
   profile, the two-corner test is the drop-in.
3. **Merging reuses [`crate::limited_dissolve`]'s boundary-cancellation
   join** rather than a second implementation of face merging. Upstream
   calls `BM_faces_join`, which is that same operation.
4. **No `f_double` handling.** Upstream watches for a merge that
   reproduces an existing face elsewhere in the mesh and kills it; on
   index rings rebuilt per face that case cannot arise.
5. **Rebuild, not in-place.**

```rust
pub mod connect_concave { /* ... */ }
```

### Functions

#### Function `connect_concave`

Split every concave face of `mesh` into convex pieces.

Convex faces and triangles are passed through untouched — a triangle is
always convex. Vertex positions are never modified; only edges are
added, so the surface is unchanged. Infallible.

# Examples

```
use outram_blender::{mesh::Mesh, math::Vec3};
use outram_blender::connect_concave::connect_concave;

// The L-shaped hexagon: one reflex corner.
let pts: Vec<Vec3> = [(0.0, 0.0), (1.0, 0.0), (1.0, 0.5),
                      (0.5, 0.5), (0.5, 1.0), (0.0, 1.0)]
    .iter().map(|&(x, y)| Vec3::new(x, y, 0.0)).collect();
let l = Mesh::from_polygons(&pts, &[(0..6).collect::<Vec<usize>>()]);
assert_eq!(l.face_count(), 1);

let split = connect_concave(&l);
assert!(split.face_count() > 1);
assert_eq!(split.vertex_count(), 6); // no vertices added
```

```rust
pub fn connect_concave(mesh: &crate::mesh::Mesh) -> crate::mesh::Mesh { /* ... */ }
```

## Module `connect_nonplanar`

Split non-planar faces along their flattest diagonal — a port of
Blender's **Split Non-Planar Faces** (`bmo_connect_nonplanar.cc`).

# What this computes and why a solver frontend needs it

A face with four or more corners is only planar by accident. A warped
quad has no single well-defined plane, so its normal, its centroid and
its area all depend on how you choose to interpret it — and a CFD mesher
or a Monte Carlo surface test will each interpret it differently. This
operator finds, for each face, the chord that splits it into the two
*flattest* halves, and cuts along it when the two halves' normals differ
by more than a given angle. It then recurses, so a badly warped n-gon is
reduced until every piece is planar to tolerance.

Inputs are positions in the caller's length unit; the tolerance is a true
angle in **radians** (upstream's `angle_limit` slot). Nothing here has a
physical dimension.

# The measure: total height variation, not a plane fit

Upstream scores a candidate half by `bm_face_subset_calc_planar`: project
every corner onto the half's own Newell normal and sum the **absolute
successive differences** in that height. This is total variation, not an
RMS residual. It punishes a face that zig-zags through the plane more
than one that bows smoothly away from it, which is the right bias — a
zig-zag is the shape a mesher chokes on. The two halves' scores are
summed and the lowest total wins.

The *cut decision* is separate from the *cut choice*: having found the
best chord, the face is split only if the cosine of the angle between
the two halves' normals is below `cos(angle_limit)`. So a gently warped
face is left whole.

# Fidelity to upstream, and the deviations

The `O(N^2)` chord search, the planarity measure, the normal-angle gate
and the recursion via a work stack are transcribed from upstream.
Differences:

1. **`f64`, not `f32`.**
2. **The legality test is our own.** Upstream calls
   `BM_face_splits_check_legal`, which works on live BMesh loops and
   knows about existing edges and face doubles. This crate rebuilds
   meshes from index rings, so the equivalent question is purely
   "is this chord a valid diagonal of this polygon?" — it must stay
   inside the ring and cross no edge. [`is_valid_diagonal`] answers that
   in the face's own projected plane. It is stricter than upstream's in
   one respect (it rejects a chord that merely touches an edge) and
   blind to one thing upstream catches (a pre-existing edge elsewhere in
   the mesh joining the same two vertices, which would create a double).
   Stated rather than papered over.
3. **Rebuild, not in-place split.** Upstream mutates a BMesh; this
   returns a new [`Mesh`], like every other operator in this crate.

```rust
pub mod connect_nonplanar { /* ... */ }
```

### Functions

#### Function `connect_nonplanar`

Split every face of `mesh` that is non-planar by more than
`angle_limit` radians, recursively, until no face can be improved.

Triangles are always planar and are passed through. Positions are never
moved — only faces are cut — so this is a topology change, not a
deformation. Use [`crate::planar_faces`] when you would rather move the
vertices than add edges.

`angle_limit` is in **radians**; [`DEFAULT_ANGLE_LIMIT`] is upstream's 5°.
A limit of 0 splits every face that is non-planar at all; a limit of
`PI` splits nothing. Infallible.

# Examples

```
use outram_blender::{mesh::Mesh, math::Vec3};
use outram_blender::connect_nonplanar::{connect_nonplanar, DEFAULT_ANGLE_LIMIT};

// A badly warped quad: one corner lifted well out of the others' plane.
let pts = vec![
    Vec3::new(0.0, 0.0, 0.0),
    Vec3::new(1.0, 0.0, 0.0),
    Vec3::new(1.0, 1.0, 1.0),
    Vec3::new(0.0, 1.0, 0.0),
];
let warped = Mesh::from_polygons(&pts, &[vec![0, 1, 2, 3]]);
let split = connect_nonplanar(&warped, DEFAULT_ANGLE_LIMIT);
assert_eq!(split.face_count(), 2);
```

```rust
pub fn connect_nonplanar(mesh: &crate::mesh::Mesh, angle_limit: f64) -> crate::mesh::Mesh { /* ... */ }
```

### Constants and Statics

#### Constant `DEFAULT_ANGLE_LIMIT`

Upstream's default for the **Split Non-Planar Faces** operator: 5°,
expressed in radians.

Faces whose two best halves differ by less than this are left alone.

```rust
pub const DEFAULT_ANGLE_LIMIT: f64 = _;
```

## Module `convex_hull`

**3D convex hull** of a point set — the incremental algorithm on the robust
[`crate::boolean_predicates::orient3d`] orientation test.

Given a set of points, [`convex_hull`] returns the closed, watertight,
outward-wound triangle [`Mesh`] of their convex hull (the smallest convex
solid containing them all). It complements the CSG / boolean suite — a hull
is the natural bounding volume, and the input to a convex decomposition.

## Method (incremental)

Start from a non-degenerate tetrahedron of four input points (chosen for good
conditioning: a farthest pair, the point of largest triangle area with that
edge, then the point farthest off their plane), with all four faces wound
**CCW as seen from outside**. Then add the remaining points one at a time:

- a face `(a, b, c)` is **visible** from a new point `p` iff `p` is on its
  outward side, i.e. `orient3d(a, b, c, p) == -1` (the crate's outward normal
  is the "above" side of `orient3d`, so "above" = `-1` = outward);
- if no face is visible, `p` is inside-or-on the hull and is skipped;
- otherwise the **horizon** (the loop of edges bordering a visible and a
  non-visible face) is found from the visible faces' directed edges — an edge
  `(a, b)` is on the horizon iff its reverse `(b, a)` is not also a visible
  edge — the visible faces are deleted, and each horizon edge `(a, b)` is
  coned to `p` as a new triangle `[a, b, p]` (already outward-wound).

All orientation decisions use the exact/robust `orient3d`, so the hull is
correct even for near-degenerate configurations that a naive `f64` sign would
misjudge. Coplanar points are handled by the strict `== -1` visibility rule
(a coplanar face is never deleted, so no sliver/duplicate faces arise; a
coplanar cube face still splits into two triangles via its non-coplanar
neighbours).

## Degeneracies

Duplicate points are removed first. Fewer than four distinct points, an
all-collinear set, or an all-coplanar set have no 3D hull and return a
[`HullError`] rather than a degenerate/open mesh.

> **Untrusted AI-generated draft** until a human reviews it, per the
> workspace `RESPONSIBLE_USE.md`. Not for nuclear facility operation,
> reactor control, safety-critical, or licensing decisions.

```rust
pub mod convex_hull { /* ... */ }
```

### Types

#### Enum `HullError`

Errors from [`convex_hull`].

```rust
pub enum HullError {
    NotEnoughPoints(usize),
    Collinear,
    Coplanar,
}
```

##### Variants

###### `NotEnoughPoints`

Fewer than four **distinct** points were supplied — a tetrahedron (the
minimal 3D hull) needs four.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

###### `Collinear`

Every point is collinear — the hull would be a line segment, not a solid.

###### `Coplanar`

Every point is coplanar — the hull would be a flat polygon, not a solid.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Error**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(source: crate::convex_hull::HullError) -> Self { /* ... */ }
    ```

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `convex_hull`

The convex hull of `points`, as a closed outward-wound triangle [`Mesh`].

Every input point lies on or inside the returned hull; the result is a
watertight genus-0 mesh (`V − E + F = 2`) with all faces wound
counter-clockwise as seen from outside. See the module docs for the method.

# Errors

[`HullError::NotEnoughPoints`] for fewer than four distinct points,
[`HullError::Collinear`] / [`HullError::Coplanar`] for a degenerate (lower
dimensional) input.

```rust
pub fn convex_hull(points: &[crate::math::Vec3]) -> Result<crate::mesh::Mesh, HullError> { /* ... */ }
```

## Module `curve`

**Curve authoring** (`op-hzs.54.34`, GH issue #37 §G — the CAD sketch
layer). The foundation for curve → surface geometry (`op-hzs.54.35`),
curve ↔ mesh conversion (`.36`), NURBS surfaces (`.37`) and text
(`.38`).

A [`Spline`] is an ordered list of [`ControlPoint`]s plus a
[`SplineType`] (poly / Bézier / NURBS) and a `cyclic` flag.
[`Spline::sample`] evaluates it to a polyline of `resolution` points per
segment; [`Spline::sample_with_frames`] also returns the per-point radius
and an oriented frame (tangent + tilted normal), which the sweep operators
ride.

```rust
pub mod curve { /* ... */ }
```

### Types

#### Enum `SplineType`

The interpolation family of a [`Spline`].

```rust
pub enum SplineType {
    Poly,
    Bezier,
    Nurbs,
}
```

##### Variants

###### `Poly`

Straight segments through the control points.

###### `Bezier`

Cubic Bézier between consecutive points, driven by their handles.

###### `Nurbs`

Non-uniform rational B-spline of the spline's `order`.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SplineType { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SplineType) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `HandleType`

How a Bézier handle is derived when [`Spline::recalculate_handles`] runs.

```rust
pub enum HandleType {
    Automatic,
    Vector,
    Aligned,
    Free,
}
```

##### Variants

###### `Automatic`

A smooth tangent from the neighbouring points' spacing.

###### `Vector`

Points straight at the neighbouring control point (a sharp-ish corner).

###### `Aligned`

Kept collinear with the opposite handle, length preserved.

###### `Free`

Left untouched.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> HandleType { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &HandleType) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `ControlPoint`

One control point of a [`Spline`].

```rust
pub struct ControlPoint {
    pub position: crate::math::Vec3,
    pub handle_left: crate::math::Vec3,
    pub handle_right: crate::math::Vec3,
    pub type_left: HandleType,
    pub type_right: HandleType,
    pub radius: f64,
    pub tilt: f64,
    pub weight: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `position` | `crate::math::Vec3` | The point itself (the "knot"). |
| `handle_left` | `crate::math::Vec3` | The incoming Bézier handle (absolute position). Unused for poly / NURBS. |
| `handle_right` | `crate::math::Vec3` | The outgoing Bézier handle (absolute position). |
| `type_left` | `HandleType` | Handle-recalculation rule for the left handle. |
| `type_right` | `HandleType` | Handle-recalculation rule for the right handle. |
| `radius` | `f64` | Cross-section radius at this point (rides through to a bevel). |
| `tilt` | `f64` | Roll of the local frame about the tangent, in radians. |
| `weight` | `f64` | NURBS weight (`1.0` = a plain B-spline point). |

##### Implementations

###### Methods

- ```rust
  pub fn new(position: Vec3) -> Self { /* ... */ }
  ```
  A point at `position` with mirrored auto handles and unit radius.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ControlPoint { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `Spline`

An ordered spline.

```rust
pub struct Spline {
    pub spline_type: SplineType,
    pub points: Vec<ControlPoint>,
    pub cyclic: bool,
    pub resolution: usize,
    pub order: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `spline_type` | `SplineType` | Interpolation family. |
| `points` | `Vec<ControlPoint>` | The control points, in order. |
| `cyclic` | `bool` | Whether the spline closes back on itself. |
| `resolution` | `usize` | Evaluated points per segment (`>= 1`). |
| `order` | `usize` | NURBS order (degree + 1); `>= 2`. Ignored for poly / Bézier. |

##### Implementations

###### Methods

- ```rust
  pub fn poly(positions: &[Vec3]) -> Self { /* ... */ }
  ```
  A new poly spline through `positions`.

- ```rust
  pub fn bezier(positions: &[Vec3]) -> Self { /* ... */ }
  ```
  A new Bézier spline through `positions` with auto handles.

- ```rust
  pub fn nurbs(positions: &[Vec3], order: usize) -> Self { /* ... */ }
  ```
  A new NURBS spline of `order` through `positions`.

- ```rust
  pub fn push(self: &mut Self, position: Vec3) { /* ... */ }
  ```
  Append a control point at `position`.

- ```rust
  pub fn toggle_cyclic(self: &mut Self) { /* ... */ }
  ```
  Toggle the cyclic flag.

- ```rust
  pub fn set_type(self: &mut Self, ty: SplineType) { /* ... */ }
  ```
  Change the interpolation family (recomputing handles for Bézier).

- ```rust
  pub fn subdivide(self: &mut Self) { /* ... */ }
  ```
  Insert a control point at the midpoint of every segment (a curve

- ```rust
  pub fn recalculate_handles(self: &mut Self) { /* ... */ }
  ```
  Recompute Bézier handles per each point's [`HandleType`]. `Free` handles

- ```rust
  pub fn sample(self: &Self) -> Vec<Vec3> { /* ... */ }
  ```
  Evaluate the spline to a polyline (`resolution` points per segment).

- ```rust
  pub fn sample_with_frames(self: &Self) -> Vec<SplineSample> { /* ... */ }
  ```
  Evaluate the spline to a list of [`SplineSample`]s (position, radius,

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Spline { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `SplineSample`

One evaluated point of a spline plus its local frame.

```rust
pub struct SplineSample {
    pub position: crate::math::Vec3,
    pub radius: f64,
    pub tangent: crate::math::Vec3,
    pub normal: crate::math::Vec3,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `position` | `crate::math::Vec3` |  |
| `radius` | `f64` |  |
| `tangent` | `crate::math::Vec3` | Unit tangent (direction of travel). |
| `normal` | `crate::math::Vec3` | Unit normal, rolled by the interpolated tilt. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SplineSample { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `curve_mesh`

**Curve ↔ Mesh conversion + curve deform + skin** (`op-hzs.54.36`, GH issue
#37 §G).

- [`mesh_to_splines`] — every maximal non-branching edge chain of a mesh
  becomes a poly [`Spline`] (Blender's *Convert to Curve*).
- [`boundary_to_splines`] — just the open-boundary loops.
- [`spline_to_mesh`] — [`crate::curve_surface::curve_to_mesh`] with sensible
  defaults (*Convert to Mesh*).
- [`spline_deform_mesh`] — deform a mesh so its axis rides a [`Spline`]
  (the Curve modifier).
- [`skin_spline`] — a round tube along a [`Spline`] (the Skin modifier on a
  curve).

```rust
pub mod curve_mesh { /* ... */ }
```

### Functions

#### Function `mesh_to_splines`

Extract every maximal edge chain of `mesh` as a poly [`Spline`]. A chain
ends at a branch vertex (valence != 2) or closes into a cyclic loop.

```rust
pub fn mesh_to_splines(mesh: &crate::mesh::Mesh) -> Vec<crate::curve::Spline> { /* ... */ }
```

#### Function `boundary_to_splines`

Extract the open-boundary loops of `mesh` as poly [`Spline`]s (each is
`cyclic`).

```rust
pub fn boundary_to_splines(mesh: &crate::mesh::Mesh) -> Vec<crate::curve::Spline> { /* ... */ }
```

#### Function `spline_to_mesh`

Convert a [`Spline`] to a mesh — a wire, a round tube, or a filled outline.

```rust
pub fn spline_to_mesh(spline: &crate::curve::Spline, tube_radius: Option<f64>) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `spline_deform_mesh`

Deform `mesh` so its `axis` coordinate rides `spline` (the Curve modifier).

```rust
pub fn spline_deform_mesh(mesh: &crate::mesh::Mesh, spline: &crate::curve::Spline, axis: crate::selection::Axis) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `skin_spline`

A round tube of `radius` along `spline` — the Skin modifier applied to a
curve. `segments` sides.

```rust
pub fn skin_spline(spline: &crate::curve::Spline, radius: f64, segments: usize) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `is_poly`

Whether `s` reads as a plausible poly conversion of a straight edge chain
(helper used by the tests / a round-trip check).

```rust
pub fn is_poly(s: &crate::curve::Spline) -> bool { /* ... */ }
```

## Module `curve_surface`

**Curve → surface geometry** (`op-hzs.54.35`, GH issue #37 §G). Depends on
[`crate::curve`].

[`curve_to_mesh`] turns a [`Spline`] into geometry per [`CurveGeometry`]:

- `bevel` — [`Bevel::Round`] sweeps a circle of `depth`, [`Bevel::Profile`]
  sweeps a custom 2-D cross-section, [`Bevel::None`] leaves a wire / fill.
- `taper` — an optional [`Spline`] whose height at parameter `t` scales the
  cross-section (a lathe taper object).
- `fill` — when there is no bevel and the spline is cyclic,
  [`FillMode::Full`] triangulates the outline (a flat cap).
- `caps` — close the ends of a swept open spline.

```rust
pub mod curve_surface { /* ... */ }
```

### Types

#### Enum `Bevel`

The cross-section swept along the spline.

```rust
pub enum Bevel {
    None,
    Round {
        depth: f64,
        segments: usize,
    },
    Profile {
        section: Vec<[f64; 2]>,
        closed: bool,
    },
}
```

##### Variants

###### `None`

No cross-section (wire, or a fill for a cyclic 2-D spline).

###### `Round`

A circle of the given radius, `segments` sides.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `depth` | `f64` |  |
| `segments` | `usize` |  |

###### `Profile`

A custom cross-section, as `[x, y]` points in the spline's frame plane.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `section` | `Vec<[f64; 2]>` |  |
| `closed` | `bool` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Bevel { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `FillMode`

How a bevel-free cyclic spline is filled.

```rust
pub enum FillMode {
    None,
    Full,
}
```

##### Variants

###### `None`

Not filled — leave a wire.

###### `Full`

Fill the outline once (a flat face).

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FillMode { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FillMode) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `CurveGeometry`

Options for [`curve_to_mesh`].

```rust
pub struct CurveGeometry {
    pub bevel: Bevel,
    pub taper: Option<crate::curve::Spline>,
    pub fill: FillMode,
    pub caps: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `bevel` | `Bevel` | The swept cross-section. |
| `taper` | `Option<crate::curve::Spline>` | Optional taper spline — its `y` at parameter `t` scales the section. |
| `fill` | `FillMode` | Fill for a bevel-free cyclic spline. |
| `caps` | `bool` | Cap the ends of a swept open spline. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CurveGeometry { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `curve_to_mesh`

Evaluate `spline` into a [`Mesh`] per `opts`.

```rust
pub fn curve_to_mesh(spline: &crate::curve::Spline, opts: &CurveGeometry) -> crate::mesh::Mesh { /* ... */ }
```

## Module `cursor_pivot`

**3D cursor + pivot points + custom orientations** (`op-hzs.54.25`, GH issue
#37 §D).

- [`Cursor3D`] and its placement helpers ([`Cursor3D::to_grid`],
  [`Cursor3D::to_selected`], [`Cursor3D::to_active`],
  [`Cursor3D::to_world_origin`], [`selection_to_cursor`]).
- [`PivotPoint`] and [`pivot_position`] — the point a rotation / scale
  turns about.
- [`rotate_about_pivot`] / [`scale_about_pivot`] — apply a transform to a
  vertex selection about a pivot, with [`PivotPoint::IndividualOrigins`]
  handled per connected component.
- [`orientation_from_selection`] — a [`TransformBasis`] from a
  vertex / edge / face selection (Blender's *Create Orientation*).

```rust
pub mod cursor_pivot { /* ... */ }
```

### Types

#### Struct `Cursor3D`

The 3D cursor: a position and a rotation frame.

```rust
pub struct Cursor3D {
    pub position: crate::math::Vec3,
    pub basis: crate::transform_input::TransformBasis,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `position` | `crate::math::Vec3` |  |
| `basis` | `crate::transform_input::TransformBasis` |  |

##### Implementations

###### Methods

- ```rust
  pub fn to_grid(self: &mut Self, step: f64) { /* ... */ }
  ```
  Snap the cursor position to a multiple of `step` on each axis.

- ```rust
  pub fn to_selected(self: &mut Self, mesh: &Mesh, verts: &[VertexId]) { /* ... */ }
  ```
  Move the cursor to the median of `verts` (empty = whole mesh).

- ```rust
  pub fn to_active(self: &mut Self, mesh: &Mesh, active: VertexId) { /* ... */ }
  ```
  Move the cursor to a single active vertex.

- ```rust
  pub fn to_world_origin(self: &mut Self) { /* ... */ }
  ```
  Move the cursor to the world origin.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Cursor3D { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `PivotPoint`

Which point a rotation / scale pivots about.

```rust
pub enum PivotPoint {
    BoundingBoxCenter,
    Cursor,
    IndividualOrigins,
    MedianPoint,
    ActiveElement(crate::mesh::VertexId),
}
```

##### Variants

###### `BoundingBoxCenter`

Centre of the selection's axis-aligned bounding box.

###### `Cursor`

The 3D cursor.

###### `IndividualOrigins`

Each connected component about its own median.

###### `MedianPoint`

The mean of the selected vertices.

###### `ActiveElement`

A caller-nominated active vertex.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::mesh::VertexId` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> PivotPoint { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PivotPoint) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `selection_to_cursor`

The delta that moves `verts` so their median lands on `cursor`.

```rust
pub fn selection_to_cursor(mesh: &crate::mesh::Mesh, verts: &[crate::mesh::VertexId], cursor: crate::math::Vec3) -> crate::math::Vec3 { /* ... */ }
```

#### Function `pivot_position`

The single pivot position for `pivot` (for [`PivotPoint::IndividualOrigins`]
this returns the overall median — use [`rotate_about_pivot`] for the real
per-component behaviour).

```rust
pub fn pivot_position(mesh: &crate::mesh::Mesh, verts: &[crate::mesh::VertexId], pivot: PivotPoint, cursor: crate::math::Vec3) -> crate::math::Vec3 { /* ... */ }
```

#### Function `rotate_about_pivot`

Rotate `verts` by `angle` about `axis` through the `pivot` point.
[`PivotPoint::IndividualOrigins`] rotates each connected component about its
own median.

```rust
pub fn rotate_about_pivot(mesh: &crate::mesh::Mesh, verts: &[crate::mesh::VertexId], pivot: PivotPoint, axis: crate::selection::Axis, angle: f64, cursor: crate::math::Vec3) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `scale_about_pivot`

Scale `verts` by `factor` about the `pivot` point.

```rust
pub fn scale_about_pivot(mesh: &crate::mesh::Mesh, verts: &[crate::mesh::VertexId], pivot: PivotPoint, factor: f64, cursor: crate::math::Vec3) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `orientation_from_selection`

A [`TransformBasis`] derived from a selection (Blender's *Create
Orientation*):

- a **face** — `z` = face normal, `x` along its first edge;
- an **edge** — `z` along the edge, `x` an arbitrary completion;
- two or more **vertices** — `z` along the line of best fit (here the vector
  between the two farthest-apart), `x` an arbitrary completion.

```rust
pub fn orientation_from_selection(mesh: &crate::mesh::Mesh, verts: &[crate::mesh::VertexId], edges: &[crate::mesh::EdgeId], faces: &[crate::mesh::FaceId]) -> Option<crate::transform_input::TransformBasis> { /* ... */ }
```

## Module `deform`

**Deform modifiers pt.1** (`op-hzs.54.31`, GH issue #37 §F) — position-only
space deformers.

- [`simple_deform`] — [`SimpleDeform::Twist`] / [`SimpleDeform::Bend`] /
  [`SimpleDeform::Taper`] / [`SimpleDeform::Stretch`] along an [`Axis`],
  parameterised over the mesh's extent on that axis.
- [`cast`] — pull toward a [`CastTarget`] (sphere / cylinder / cuboid).
- [`displace`] — offset along a direction by value noise (a stand-in for
  Blender's texture input).
- [`warp`] — bend space so a "from" segment maps onto a "to" segment.
- [`wave`] — a travelling sine ripple.

```rust
pub mod deform { /* ... */ }
```

### Types

#### Enum `SimpleDeform`

Simple Deform modes.

```rust
pub enum SimpleDeform {
    Twist(f64),
    Bend(f64),
    Taper(f64),
    Stretch(f64),
}
```

##### Variants

###### `Twist`

Rotate progressively about the axis (radians end-to-end).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `Bend`

Bend into an arc of the given total angle (radians).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `Taper`

Scale the perpendicular cross-section from `1` to `1 + factor` along
the axis.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `Stretch`

Stretch by `factor` along the axis, contracting perpendicular by
`1/√(1 + factor)` (volume-preserving-ish).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SimpleDeform { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SimpleDeform) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `CastTarget`

What [`cast`] pulls toward.

```rust
pub enum CastTarget {
    Sphere(f64),
    Cylinder {
        radius: f64,
        axis: crate::selection::Axis,
    },
    Cuboid(crate::math::Vec3),
}
```

##### Variants

###### `Sphere`

A sphere of the given radius about the mesh centre.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `Cylinder`

A cylinder of the given radius about the axis through the mesh centre.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `radius` | `f64` |  |
| `axis` | `crate::selection::Axis` |  |

###### `Cuboid`

A cuboid of the given half-extents about the mesh centre.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::math::Vec3` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CastTarget { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CastTarget) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `simple_deform`

Apply a [`SimpleDeform`] along `axis` over the selection's extent.

```rust
pub fn simple_deform(mesh: &crate::mesh::Mesh, verts: &[crate::mesh::VertexId], mode: SimpleDeform, axis: crate::selection::Axis) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `cast`

Pull the selection a fraction `factor` of the way toward `target`
(about the selection's centre).

```rust
pub fn cast(mesh: &crate::mesh::Mesh, verts: &[crate::mesh::VertexId], target: CastTarget, factor: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `displace`

Displace along `direction` by `strength` times value noise sampled at
`p * noise_scale`. A texture-free stand-in for Blender's Displace.

```rust
pub fn displace(mesh: &crate::mesh::Mesh, verts: &[crate::mesh::VertexId], direction: crate::math::Vec3, strength: f64, noise_scale: f64, seed: u64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `warp`

Warp space so the segment `from` … `from2` maps onto `to` … `to2` (a
rotate + scale + translate blended by proximity to `from`).

```rust
pub fn warp(mesh: &crate::mesh::Mesh, verts: &[crate::mesh::VertexId], from: crate::math::Vec3, from2: crate::math::Vec3, to: crate::math::Vec3, to2: crate::math::Vec3, falloff_radius: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `wave`

A travelling sine ripple: displace along `axis` by
`amplitude · sin(2π (r/wavelength − speed·time))` where `r` is the distance
from the origin in the plane orthogonal to `axis`.

```rust
pub fn wave(mesh: &crate::mesh::Mesh, verts: &[crate::mesh::VertexId], axis: crate::selection::Axis, amplitude: f64, wavelength: f64, speed: f64, time: f64) -> crate::mesh::Mesh { /* ... */ }
```

## Module `deform2`

**Deform modifiers pt.2** (`op-hzs.54.32`, GH issue #37 §F).

- [`curve_deform`] — bend the mesh so its `axis` coordinate follows the
  arc-length of a polyline curve, riding the curve's local frame.
- [`Lattice`] + [`lattice_deform`] — trilinear free-form deformation from a
  3-D control-point grid (Blender's Lattice modifier).
- [`hook`] — a hook point drags a vertex set, with a smooth falloff.
- [`shrinkwrap`] — project vertices onto a target mesh
  ([`ShrinkMode::NearestSurfacePoint`] / [`ShrinkMode::ProjectAlongNormal`] /
  [`ShrinkMode::NearestVertex`]).
- [`SurfaceBind`] + [`surface_deform`] — bind vertices to a target mesh's
  triangles (barycentric) once, then follow the deformed target.
- [`laplacian_deform`] — anchored deformation; forwards to
  [`crate::arap::arap_deform`].

```rust
pub mod deform2 { /* ... */ }
```

### Types

#### Struct `Lattice`

A 3-D grid of control points for [`lattice_deform`].

```rust
pub struct Lattice {
    pub dims: [usize; 3],
    pub rest_min: crate::math::Vec3,
    pub rest_max: crate::math::Vec3,
    pub points: Vec<crate::math::Vec3>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `dims` | `[usize; 3]` | Grid resolution `[nx, ny, nz]` (each `>= 2`). |
| `rest_min` | `crate::math::Vec3` | The undeformed grid's corner and its opposite corner. |
| `rest_max` | `crate::math::Vec3` |  |
| `points` | `Vec<crate::math::Vec3>` | Control-point positions, `points[x + nx*(y + ny*z)]`. |

##### Implementations

###### Methods

- ```rust
  pub fn from_bounds(dims: [usize; 3], min: Vec3, max: Vec3) -> Self { /* ... */ }
  ```
  A lattice matching a mesh's bounding box, control points at their rest

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Lattice { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `ShrinkMode`

How [`shrinkwrap`] snaps a vertex onto the target.

```rust
pub enum ShrinkMode {
    NearestSurfacePoint,
    ProjectAlongNormal,
    NearestVertex,
}
```

##### Variants

###### `NearestSurfacePoint`

The closest point anywhere on the target surface.

###### `ProjectAlongNormal`

The first target hit along `+normal` then `-normal` from the vertex.

###### `NearestVertex`

The closest target vertex.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ShrinkMode { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ShrinkMode) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `SurfaceBind`

A binding of a mesh's vertices to a target surface's triangles.

```rust
pub struct SurfaceBind {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SurfaceBind { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `curve_deform`

Deform `mesh` so its `axis` coordinate rides `curve` (a polyline, `>= 2`
points). A vertex at axis-coordinate `c` is placed at arc-length
`c - axis_min` along the curve, offset by its perpendicular components in
the curve's local frame (tangent + a stable up).

```rust
pub fn curve_deform(mesh: &crate::mesh::Mesh, curve: &[crate::math::Vec3], axis: crate::selection::Axis) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `lattice_deform`

Trilinear free-form deformation: each mesh vertex's normalised position in
the lattice's rest box picks a trilinear blend of the (possibly moved)
control points.

```rust
pub fn lattice_deform(mesh: &crate::mesh::Mesh, lat: &Lattice) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `hook`

A hook: drag `verts` by `to - from`, weighted by a smooth falloff from
`from` out to `falloff_radius` (`0` = rigid within the whole selection).

```rust
pub fn hook(mesh: &crate::mesh::Mesh, verts: &[crate::mesh::VertexId], from: crate::math::Vec3, to: crate::math::Vec3, falloff_radius: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `shrinkwrap`

Move each vertex of `mesh` onto `target` per `mode`, blended by `factor`.

```rust
pub fn shrinkwrap(mesh: &crate::mesh::Mesh, target: &crate::mesh::Mesh, mode: ShrinkMode, factor: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `bind_to_surface`

Bind `mesh`'s vertices to `target`'s triangles at the current pose.

```rust
pub fn bind_to_surface(mesh: &crate::mesh::Mesh, target: &crate::mesh::Mesh) -> SurfaceBind { /* ... */ }
```

#### Function `surface_deform`

Re-evaluate a [`SurfaceBind`] against a deformed `target` (same topology),
producing the corresponding deformed source mesh.

```rust
pub fn surface_deform(mesh: &crate::mesh::Mesh, bind: &SurfaceBind, deformed_target: &crate::mesh::Mesh) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `laplacian_deform`

Anchored (Laplacian) deformation — forwards to [`crate::arap::arap_deform`].

```rust
pub fn laplacian_deform(mesh: &crate::mesh::Mesh, handles: &[(crate::mesh::VertexId, crate::math::Vec3)], iterations: u32) -> Result<crate::mesh::Mesh, crate::arap::ArapError> { /* ... */ }
```

## Module `decimate`

**QEM mesh decimation** — Garland–Heckbert quadric-error-metric edge-collapse
simplification.

Blender analogue: the **Decimate** modifier (`MOD_decimate`, "Collapse"
mode). Reduces a mesh's triangle count while preserving its shape, by
repeatedly collapsing the edge whose removal introduces the least
*quadric error* (squared distance to the planes of the original surface).

## The method (Garland & Heckbert, 1997)

Each vertex carries a `4×4` symmetric **error quadric** `Q` — the sum of the
outer products `p pᵀ` of the plane equations `p = (a, b, c, d)` of its
incident triangles (area-weighted). The squared distance of a point `v` to
all those planes is `vᵀ Q v`. To collapse edge `(i, j)` we combine
`Q = Q_i + Q_j`, place the merged vertex at the `v` minimizing `vᵀ Q v` (a
`3×3` solve, with a fallback to the cheaper of the endpoints/midpoint when
that is singular), and use that minimum as the collapse **cost**. A min-heap
keyed on cost drives a greedy sequence of collapses until a target face count
is reached.

Guards keep the result sane: a **link-condition** check rejects collapses
that would make the mesh non-manifold, a **normal-flip** check rejects those
that would fold a triangle over, and **boundary** edges get a large penalty
quadric so an open border is preserved.

Everything here is closed-form (a hand-written symmetric-`3×3` solve and a
version-stamped lazy-deletion heap); no `faer` is needed. Works on the
polygon-soup view and rebuilds via [`Mesh::from_polygons`] — no half-edge
surgery.

> **Untrusted AI-generated draft** until a human reviews it, per the
> workspace `RESPONSIBLE_USE.md`. Not for nuclear facility operation,
> reactor control, safety-critical, or licensing decisions.

```rust
pub mod decimate { /* ... */ }
```

### Types

#### Enum `StopReason`

Why [`decimate`] stopped collapsing.

```rust
pub enum StopReason {
    ReachedTarget,
    NoLegalCollapse,
}
```

##### Variants

###### `ReachedTarget`

The requested target face count was reached.

###### `NoLegalCollapse`

No legal collapse remained (every candidate was rejected by the manifold
/ flip / boundary guards) before the target was reached.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> StopReason { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &StopReason) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `DecimateResult`

The result of a decimation: the simplified mesh and why it stopped.

```rust
pub struct DecimateResult {
    pub mesh: crate::mesh::Mesh,
    pub stop_reason: StopReason,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `crate::mesh::Mesh` | The simplified mesh. |
| `stop_reason` | `StopReason` | Why decimation stopped (target reached, or ran out of legal collapses). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> DecimateResult { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `decimate`

Simplify `mesh` down to roughly `target_faces` triangles by QEM edge
collapse, returning the simplified [`Mesh`].

Convenience wrapper over [`decimate_with_reason`] that discards the stop
reason. `target_faces` is a *lower bound goal*: the result has at most a
couple more faces than the target (collapses remove faces in pairs), or more
if no further legal collapse exists. Non-triangular faces are fan-triangulated
first, so the output is a triangle mesh.

```rust
pub fn decimate(mesh: &crate::mesh::Mesh, target_faces: usize) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `decimate_with_reason`

Like [`decimate`] but also reports the [`StopReason`].

```rust
pub fn decimate_with_reason(mesh: &crate::mesh::Mesh, target_faces: usize) -> DecimateResult { /* ... */ }
```

## Module `dissolve`

**Dissolve / Delete** (`op-hzs.54.13`, GH issue #37 §B).

- [`dissolve_faces`] merges a connected set of faces into one n-gon (the
  union boundary). Fails (returns the mesh unchanged) if the set has a hole
  or a non-simple boundary.
- [`dissolve_edges`] dissolves each interior edge by merging its two faces.
- [`dissolve_vertices`] removes each vertex, merging its incident faces and
  dropping the vertex from the merged ring.
- [`limited_dissolve`] dissolves every edge whose two faces are within
  `angle` of coplanar — the planar cleanup pass.
- [`delete`] is the delete/erase matrix ([`DeleteMode`]).

```rust
pub mod dissolve { /* ... */ }
```

### Types

#### Enum `DeleteMode`

The delete/erase matrix.

```rust
pub enum DeleteMode {
    Vertices,
    Edges,
    Faces,
    OnlyFaces,
    Collapse,
}
```

##### Variants

###### `Vertices`

Remove the vertices and every edge/face using them.

###### `Edges`

Remove the edges and every face using them; keep the vertices.

###### `Faces`

Remove the faces; keep their edges and vertices (leaves a hole).

###### `OnlyFaces`

Remove the faces only (same as [`DeleteMode::Faces`] in a soup model).

###### `Collapse`

Collapse the given edges (see [`crate::merge::merge_edges`]).

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> DeleteMode { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DeleteMode) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `dissolve_faces`

Merge a connected face set into a single n-gon. Returns the mesh unchanged
if `faces` is empty, disconnected, or its union boundary is not one simple
loop (e.g. it encloses a hole).

```rust
pub fn dissolve_faces(mesh: &crate::mesh::Mesh, faces: &[crate::mesh::FaceId]) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `dissolve_edges`

Dissolve each interior edge in `edges` (merge its two faces). Dissolves are
applied in id order; an edge whose faces were already merged is skipped.

```rust
pub fn dissolve_edges(mesh: &crate::mesh::Mesh, edges: &[crate::mesh::EdgeId]) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `dissolve_vertices`

Dissolve each vertex in `verts`: merge its incident faces and remove the
vertex from the merged boundary.

```rust
pub fn dissolve_vertices(mesh: &crate::mesh::Mesh, verts: &[crate::mesh::VertexId]) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `limited_dissolve`

Dissolve every interior edge whose two faces are within `angle` radians of
coplanar — Blender's Limited Dissolve (planar cleanup).

**Reimplemented 2026-09-19.** This now delegates to
[`crate::limited_dissolve::limited_dissolve`], the port of upstream's
`BM_mesh_decimate_dissolve`. The signature and behaviour-on-flat-geometry
are unchanged, so callers need no edit.

# Why it changed

The previous implementation scored every edge **once** against the input
normals and dissolved the whole qualifying set in a single
[`dissolve_edges`] call. Upstream re-costs the merged face's edges after
every join, which is what keeps a curved surface from collapsing: once
two facets merge, the merged normal is their average, so the next facet
is measured against a normal that has already moved.

Measured over two UV spheres, the one-shot approach failed two ways:

- **Area was not conserved.** On a 48x32 sphere at 5 degrees it produced
  a surface of area 15.501 against a true 12.533 — a +23.7 % error, from
  merged n-gons warped enough to no longer describe the same surface.
- **Past a threshold it silently did nothing.** At 15 and 30 degrees it
  returned the input unchanged, because a single all-at-once merge
  cannot form a simple boundary and bails. Asking for more
  simplification produced less, with no error.

The full table is on
`limited_dissolve::tests::the_iterative_dissolve_conserves_area_where_the_one_shot_did_not`.

```rust
pub fn limited_dissolve(mesh: &crate::mesh::Mesh, angle: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `delete`

Apply `mode` to the given elements, returning the rebuilt mesh.

```rust
pub fn delete(mesh: &crate::mesh::Mesh, mode: DeleteMode, verts: &[crate::mesh::VertexId], edges: &[crate::mesh::EdgeId], faces: &[crate::mesh::FaceId]) -> crate::mesh::Mesh { /* ... */ }
```

## Module `edge_bevel`

Edge bevel — chamfer every edge of a closed mesh.

This is the pure-Rust analogue of Blender's **Bevel** on edges (as opposed
to the vertex-truncation [`crate::ops::bevel_vertices`]). Each face is cut
back from its edges by `width`, the gap left along every original **edge**
is filled by a bevel face, and the gap at every original **vertex** is
capped — turning each sharp edge into a flat chamfer.

# Construction

For each face and each of its corners, a new **face-corner vertex** is placed
by moving the corner inward along its two incident in-face edge directions by
`width` (exact for right-angle corners, a good approximation otherwise). The
output is then three families of faces:

- **shrunk faces** — each original face, rebuilt on its face-corner vertices;
- **edge chamfers** — one quad per original edge, bridging the two shrunk
  faces that met there;
- **vertex caps** — one polygon per original vertex, filling the truncated
  corner (its incident face-corner vertices in umbrella order).

Only the **connectivity** is built here; the final consistent, outward
winding is delegated to [`crate::recalc_normals::recalculate_normals`], so
the construction never has to reason about per-face orientation.

# Scope (v1)

Flat chamfer (a single bevel face per edge; rounded multi-segment bevels are
future work). Intended for **closed manifold** meshes; boundary edges/vertices
are left uncapped. `width` must be smaller than half the shortest edge, or a
face can invert. No `faer`, no external dependency; Android-safe.

```rust
pub mod edge_bevel { /* ... */ }
```

### Functions

#### Function `bevel_edges`

Chamfer every edge of `mesh` by `width`, returning the beveled mesh.

`width` is the distance each face is cut back from its edges, in mesh units;
keep it below half the shortest edge length to avoid inverting a face. The
result is consistently wound and outward-facing. This is infallible.

# Examples

```
use outram_blender::{primitives, edge_bevel::bevel_edges};

// Beveling a cube's 12 edges: 6 shrunk squares + 12 edge quads + 8 corner
// triangles = 26 faces, still a closed genus-0 surface (χ = 2).
let beveled = bevel_edges(&primitives::cube(2.0), 0.3);
assert_eq!(beveled.face_count(), 26);
assert_eq!(beveled.euler_characteristic(), 2);
```

```rust
pub fn bevel_edges(mesh: &crate::mesh::Mesh, width: f64) -> crate::mesh::Mesh { /* ... */ }
```

## Module `edge_tools`

**Edge tools** (`op-hzs.54.18`, GH issue #37 §B).

- [`rotate_edge`] — spin an edge to the next pair of vertices of its two
  faces (a triangle flip generalises to any two faces). Blender's `Edge ▸
  Rotate Edge CW / CCW`.
- [`set_edge_flow`] — relax an edge loop's vertices toward a smooth path
  along their rail edges. Blender's `Edge ▸ Set Edge Flow` addon.
- [`edge_split`] — split the mesh along an edge set: each vertex shared by
  two face groups the split separates gets its own copy (the Edge Split
  modifier as an operator).

```rust
pub mod edge_tools { /* ... */ }
```

### Functions

#### Function `rotate_edge`

Rotate `edge` to connect the next vertices of its two incident faces —
`cw` picks the clockwise pair, `!cw` the counter-clockwise. A no-op unless
the edge has exactly two faces and the combined polygon stays simple.

```rust
pub fn rotate_edge(mesh: &crate::mesh::Mesh, edge: crate::mesh::EdgeId, cw: bool) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `set_edge_flow`

Relax the vertices of the edge loop `loop_edges` toward a smooth path:
`iterations` passes, each moving every 2-rail loop vertex a fraction
`strength` toward the midpoint of its two rail neighbours. Topology
unchanged.

```rust
pub fn set_edge_flow(mesh: &crate::mesh::Mesh, loop_edges: &[crate::mesh::EdgeId], iterations: u32, strength: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `edge_split`

Split `mesh` along `edges`: each vertex the split separates into two or more
face groups gets one copy per extra group. The Edge Split modifier as an
operator (pair with a crease attribute later).

```rust
pub fn edge_split(mesh: &crate::mesh::Mesh, edges: &[crate::mesh::EdgeId]) -> crate::mesh::Mesh { /* ... */ }
```

## Module `export`

Export bridges from an authored [`Mesh`] to the OUTRAM PARK solvers.

The **default** exporters are self-contained and dependency-free: the
OpenFOAM bridge emits **text** (the polyMesh ASCII files as `String`s), the
CSG bridge emits **local mirror types** ([`CsgSurface`], [`RegionToken`],
[`CsgDescription`]) that shadow the consumer crate's geometry vocabulary, and
[`FacetedSolid`] carries a triangulated boundary. So the frontend stays light
and Android-buildable with no solver-crate dependency.

**Real-type bridges** to the actual solver crates are available behind opt-in
cargo features, so neither solver crate is a hard dependency:

- `foam-export` → `to_poly_mesh` returns a real
  `outram_foam_basic_lib::io::poly_mesh::PolyMesh` (the type its OpenFOAM
  reader/writer round-trips), and `from_poly_mesh` reads one back into a
  [`Mesh`] — combined with `PolyMesh::read(dir)` this **imports** an OpenFOAM
  `constant/polyMesh` directory, the inverse of `write_polymesh`;
- `mc-export` → `to_mc_geometry` returns a real
  `outram_mc_libs::prelude::Geometry` (surfaces + a cell region).

These are the wired counterparts of the text / mirror exporters (epic
`op-hzs`, beads `op-hzs.6`/`op-hzs.7`).

An authored mesh here is a **boundary surface** — a shell of vertices,
edges, and polygon faces, with *no cells*. That single fact shapes both
bridges, so it is stated up front and repeated at each export point rather
than hidden.

## 1. OpenFOAM polyMesh (CFD) — [`to_polymesh_text`] / [`write_polymesh`]

OpenFOAM's `polyMesh` (mirrored by `outram_foam_basic_lib`'s `io::poly_mesh`)
is normally a finite-volume **VOLUME** mesh: `points`, `faces`,
`owner[f]`/`neighbour[f]` (the two cells straddling each *internal* face),
and named boundary **patches**.

Our surface mesh has faces but **no cells**, so what we emit is a polyMesh
**boundary description**, not a solve-ready volume mesh:

- `points` — the vertex coordinates;
- `faces` — each face as `n(v0 v1 …)`, wound as the mesh winds it;
- `owner` — every face owned by a single **dummy cell `0`**, because a
  surface has no real cells;
- `neighbour` — **empty** (there are no internal faces);
- `boundary` — one patch `authoredSurface` of `type patch`, covering all
  faces.

This is the input a volume mesher (blockMesh / snappyHexMesh) would *fill*
to produce cells; it is a boundary patch, **not** a ready-to-solve mesh.
Coordinates are dimensionless model space — the caller assigns a length
unit (conventionally metres) when handing the mesh to a solver.

## 2. `outram-mc-libs` CSG (Monte Carlo transport) — [`to_csg_primitive`]

`outram-mc-libs`'s geometry is **constructive solid geometry**: analytic
surfaces (`XPlane`/`YPlane`/`ZPlane`, `Sphere`, `ZCylinder`, …) combined by
an RPN region of signed half-spaces into cells. A triangulated boundary
mesh does not map onto analytic surfaces directly, so the bridge takes the
**primitive-fitting** route: recognise that a mesh *came from* a
[`crate::primitives`] generator and emit the exact analytic CSG for it.

Implemented analytic fits: an axis-aligned **cube/box** (six planes), a
**uv-sphere** (one `Sphere`), a **Z-axis cylinder** (`ZCylinder` ∩ two
`ZPlane` caps), and **any convex polyhedron** (the faceted convex route: one
general [`CsgSurface::Plane`] per face, intersected — exact because a convex
solid is the intersection of its face half-spaces). A **non-convex** mesh is
not a half-space intersection, so [`to_csg_primitive`] returns
[`ExportError::NotImplemented`] for it.

## 3. Faceted / DAGMC boundary (non-convex Monte Carlo) — [`to_faceted_solid`]

For an arbitrary (non-convex) solid — e.g. a general boolean result — the
[`FacetedSolid`] route keeps the triangulated boundary as-is (outward
oriented) and answers inside/outside by the **generalized winding number**,
the DAGMC point-in-volume idea. This is the honest representation when no
analytic primitive fits.

## Shared foundation — [`triangulate`]

[`triangulate`] provides a dependency-free indexed triangle soup
([`IndexedTriangles`]) — the common denominator an OBJ/STL writer, a
polyMesh patch, or a faceted-CSG surface each build on. It is fully
implemented and tested.

```rust
pub mod export { /* ... */ }
```

### Types

#### Enum `ExportError`

Errors from an export bridge.

```rust
pub enum ExportError {
    NotImplemented(&'static str),
    NotClosedSurface {
        tri: usize,
        a: u32,
        b: u32,
    },
    NonManifoldSurface {
        tri: usize,
        other_tri: usize,
        a: u32,
        b: u32,
    },
    DegenerateTriangle {
        tri: usize,
        a: u32,
        b: u32,
        c: u32,
    },
}
```

##### Variants

###### `NotImplemented`

A requested export path is documented but not implemented for this mesh.

Returned when [`to_csg_primitive`] (or `to_mc_geometry`, which builds
on it) is handed a mesh that is not a half-space intersection — i.e. a
**non-convex** solid, which no combination of analytic surfaces
describes. That is not a gap in this module: use [`to_faceted_solid`]
for the DAGMC-style faceted boundary representation of such a solid.
The payload is a human-readable explanation of what was expected.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `&'static str` |  |

###### `NotClosedSurface`

The surface is **not closed**: directed edge `a -> b` of triangle `tri`
has no opposing triangle, so the undirected edge `{a, b}` belongs to one
triangle instead of two.

Returned by [`FacetedSolid::check_closed_manifold`]. Inside/outside on a
faceted solid is decided by the **generalized winding number**, which is
only well-defined on a closed surface — an open one makes
[`FacetedSolid::contains`] silently arbitrary.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `tri` | `usize` | Index of the offending triangle in [`FacetedSolid::triangles`]. |
| `a` | `u32` | Start vertex of the unpaired directed edge. |
| `b` | `u32` | End vertex of the unpaired directed edge. |

###### `NonManifoldSurface`

Two triangles traverse the same directed edge — the surface is
non-manifold, inconsistently wound, or has a duplicated triangle.

Returned by [`FacetedSolid::check_closed_manifold`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `tri` | `usize` | Index of the later offending triangle. |
| `other_tri` | `usize` | Index of the triangle that already traversed this directed edge. |
| `a` | `u32` | Start vertex of the doubly-traversed directed edge. |
| `b` | `u32` | End vertex of the doubly-traversed directed edge. |

###### `DegenerateTriangle`

A triangle repeats a corner vertex, so it has no well-defined normal and
contributes a self-edge that can never be paired.

Returned by [`FacetedSolid::check_closed_manifold`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `tri` | `usize` | Index of the offending triangle. |
| `a` | `u32` | First corner vertex index. |
| `b` | `u32` | Second corner vertex index. |
| `c` | `u32` | Third corner vertex index. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Error**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `IndexedTriangles`

A dependency-free, flattened triangle mesh: the common export denominator.

Every polygon face of a [`Mesh`] is fan-triangulated into this indexed form
(`positions` + triangle `indices` triplets). This is what an OBJ/STL writer,
a polyMesh boundary patch, or a faceted-CSG surface each build from — it is
the shared, dependency-free foundation under every exporter in this module,
including the feature-gated real-type solver bridges.

```rust
pub struct IndexedTriangles {
    pub positions: Vec<crate::math::Vec3>,
    pub indices: Vec<u32>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `positions` | `Vec<crate::math::Vec3>` | Vertex positions, indexed by the entries of [`IndexedTriangles::indices`]. |
| `indices` | `Vec<u32>` | Flat list of triangle corner indices, three consecutive entries per<br>triangle, each indexing into [`IndexedTriangles::positions`]. |

##### Implementations

###### Methods

- ```rust
  pub fn triangle_count(self: &Self) -> usize { /* ... */ }
  ```
  Number of triangles (== `indices.len() / 3`).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> IndexedTriangles { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> IndexedTriangles { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `PolyMeshText`

The five OpenFOAM `polyMesh` ASCII files, each held as a `String`.

Produced by [`to_polymesh_text`] with no filesystem access, so it is fully
unit-testable; [`write_polymesh`] is the only function that touches disk.

**These describe a boundary SURFACE, not a solve-ready volume mesh.** Every
face is owned by a single dummy cell `0` and there are no internal faces
(`neighbour` is empty). A volume mesher must fill the interior before this
is a mesh a CFD solver can march on. See the module docs.

Coordinates are dimensionless model space; the caller assigns a length unit
(conventionally metres) at export.

```rust
pub struct PolyMeshText {
    pub points: String,
    pub faces: String,
    pub owner: String,
    pub neighbour: String,
    pub boundary: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `points` | `String` | `constant/polyMesh/points` — `class vectorField; object points;`. One<br>`(x y z)` per mesh vertex, in [`crate::mesh::VertexId`] order. |
| `faces` | `String` | `constant/polyMesh/faces` — `class faceList; object faces;`. One<br>`n(v0 v1 …)` per mesh face, wound as the mesh winds it. |
| `owner` | `String` | `constant/polyMesh/owner` — `class labelList; object owner;`. One label<br>per face, all `0` (the single dummy cell). A `note` records that this is<br>a boundary patch, not a volume mesh. |
| `neighbour` | `String` | `constant/polyMesh/neighbour` — `class labelList; object neighbour;`.<br>Empty (zero entries): a boundary surface has no internal faces. |
| `boundary` | `String` | `constant/polyMesh/boundary` — `class polyBoundaryMesh; object boundary;`.<br>A single patch `authoredSurface` of `type patch`, `nFaces` = face count,<br>`startFace 0`. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> PolyMeshText { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `CsgSurface`

An analytic CSG surface — a **local mirror** of `outram-mc-libs`'s
`SurfaceKind`.

Each variant is the implicit surface `f(x, y, z) = 0`; its
[`CsgSurface::signed_value`] gives `f`, whose sign selects a half-space
(see [`Sense`]). Offsets and radii are dimensionless model-space lengths
(the caller assigns a length unit, conventionally metres, at export). The
variant set here is exactly the subset the primitive fitter emits;
`outram-mc-libs` defines the same shapes.

```rust
pub enum CsgSurface {
    XPlane {
        x0: f64,
    },
    YPlane {
        y0: f64,
    },
    ZPlane {
        z0: f64,
    },
    Sphere {
        x0: f64,
        y0: f64,
        z0: f64,
        r: f64,
    },
    ZCylinder {
        x0: f64,
        y0: f64,
        r: f64,
    },
    Plane {
        a: f64,
        b: f64,
        c: f64,
        d: f64,
    },
}
```

##### Variants

###### `XPlane`

Plane `x = x0`, normal along +X. `f = x - x0`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `x0` | `f64` | The X coordinate of the plane. |

###### `YPlane`

Plane `y = y0`, normal along +Y. `f = y - y0`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `y0` | `f64` | The Y coordinate of the plane. |

###### `ZPlane`

Plane `z = z0`, normal along +Z. `f = z - z0`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `z0` | `f64` | The Z coordinate of the plane. |

###### `Sphere`

Sphere of radius `r` centred at `(x0, y0, z0)`.
`f = (x-x0)^2 + (y-y0)^2 + (z-z0)^2 - r^2` (negative inside).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `x0` | `f64` | Centre X. |
| `y0` | `f64` | Centre Y. |
| `z0` | `f64` | Centre Z. |
| `r` | `f64` | Radius (`> 0`). |

###### `ZCylinder`

Infinite cylinder of radius `r` about the line `x = x0, y = y0`, axis
parallel to Z. `f = (x-x0)^2 + (y-y0)^2 - r^2` (negative inside).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `x0` | `f64` | Axis X. |
| `y0` | `f64` | Axis Y. |
| `r` | `f64` | Radius (`> 0`). |

###### `Plane`

General (arbitrarily oriented) plane with unit normal `(a, b, c)` at
signed distance `d` from the origin along that normal:
`f = a*x + b*y + c*z - d`. The `+normal` side is `f > 0`. Used by the
faceted convex route ([`to_csg_primitive`]), where each face of a convex
polyhedron becomes one such plane. `(a, b, c)` is expected to be unit
length so `f` is a true signed distance.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `f64` | Normal X component (unit normal). |
| `b` | `f64` | Normal Y component (unit normal). |
| `c` | `f64` | Normal Z component (unit normal). |
| `d` | `f64` | Signed distance of the plane from the origin along `(a, b, c)`. |

##### Implementations

###### Methods

- ```rust
  pub fn signed_value(self: &Self, p: Vec3) -> f64 { /* ... */ }
  ```
  Evaluate the implicit function `f(p)` for this surface.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CsgSurface { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CsgSurface) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `Sense`

Which side of a [`CsgSurface`] a half-space selects — a local mirror of
`outram-mc-libs`'s surface sense.

```rust
pub enum Sense {
    Positive,
    Negative,
}
```

##### Variants

###### `Positive`

The `f > 0` side (outside a sphere/cylinder; +axis side of a plane).

###### `Negative`

The `f < 0` side (inside a sphere/cylinder; -axis side of a plane).

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Sense { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Sense) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `RegionToken`

One token of an RPN CSG region — a **local mirror** of `outram-mc-libs`'s
`RegionToken` (`Cell.region`).

The region is evaluated as a stack machine over booleans (see
[`CsgDescription::contains`]): a [`RegionToken::Halfspace`] pushes "point is
on the chosen side of surface `surface`"; [`RegionToken::Intersection`] /
[`RegionToken::Union`] pop two and push their AND / OR;
[`RegionToken::Complement`] pops one and pushes its negation.

```rust
pub enum RegionToken {
    Halfspace {
        surface: usize,
        sense: Sense,
    },
    Intersection,
    Union,
    Complement,
}
```

##### Variants

###### `Halfspace`

The signed half-space of surface index `surface` on side `sense`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `surface` | `usize` | Index into [`CsgDescription::surfaces`]. |
| `sense` | `Sense` | Which side of that surface this half-space is. |

###### `Intersection`

Boolean AND of the top two operands (set intersection).

###### `Union`

Boolean OR of the top two operands (set union).

###### `Complement`

Boolean NOT of the top operand (set complement).

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> RegionToken { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &RegionToken) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `CsgDescription`

A complete CSG solid: analytic `surfaces` plus an RPN `region` over them.

A **local mirror** of the geometry `outram-mc-libs` consumes — surfaces
(`SurfaceKind`) and a region (`Cell.region`) — so this crate need not depend
on it. Produced by [`to_csg_primitive`]. Lengths are dimensionless model
space; a length unit (conventionally metres) is assigned at export.

```rust
pub struct CsgDescription {
    pub surfaces: Vec<CsgSurface>,
    pub region: Vec<RegionToken>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `surfaces` | `Vec<CsgSurface>` | The analytic surfaces, referenced by index from `region`. |
| `region` | `Vec<RegionToken>` | The region as an RPN token stream over `surfaces` (see [`RegionToken`]). |

##### Implementations

###### Methods

- ```rust
  pub fn contains(self: &Self, p: Vec3) -> bool { /* ... */ }
  ```
  Test whether point `p` lies inside this CSG region.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CsgDescription { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CsgDescription) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `FacetedSolid`

A solid represented **directly by its triangulated boundary** — the
DAGMC-style representation for meshes that are *not* a recognised primitive
and are *not* convex (so they cannot be a CSG half-space intersection).

This is the honest export route for an arbitrary boolean result: rather than
forcing it into analytic surfaces, the solid *is* its outward-oriented
triangle surface, and inside/outside is decided by the **generalized winding
number** (the same test [`crate::boolean_classify`] uses and DAGMC's
point-in-volume query embodies). A local mirror — `outram-mc-libs` would
consume the triangle soup for ray-traced surface tracking; wiring that real
dependency is deferred with the rest of this module.

Coordinates are dimensionless model space; a length unit (conventionally
metres) is assigned when the solid reaches the transport solver.

```rust
pub struct FacetedSolid {
    pub positions: Vec<crate::math::Vec3>,
    pub triangles: Vec<[u32; 3]>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `positions` | `Vec<crate::math::Vec3>` | Vertex positions, indexed by [`FacetedSolid::triangles`]. |
| `triangles` | `Vec<[u32; 3]>` | Outward-wound triangles (each a corner-index triple into<br>[`FacetedSolid::positions`]). Outward orientation is enforced at<br>construction so face normals point out of the solid. |

##### Implementations

###### Methods

- ```rust
  pub fn triangle_count(self: &Self) -> usize { /* ... */ }
  ```
  Number of boundary triangles.

- ```rust
  pub fn check_closed_manifold(self: &Self) -> Result<(), ExportError> { /* ... */ }
  ```
  Verify that this boundary is a **closed, consistently-wound 2-manifold** —

- ```rust
  pub fn contains(self: &Self, p: Vec3) -> bool { /* ... */ }
  ```
  Test whether point `p` is inside the solid, by the **generalized winding

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FacetedSolid { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> FacetedSolid { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `triangulate`

Fan-triangulate every face of `mesh` into an [`IndexedTriangles`] soup.

Each `n`-gon face contributes `n - 2` triangles by a simple fan from its
first vertex — valid for the convex faces the [`crate::primitives`]
generators produce. Positions are copied 1:1 from the mesh (indices are
preserved), so `positions.len()` equals `mesh.vertex_count()`.

This is real, tested code — the dependency-free foundation the solver
bridges below build on.

```rust
pub fn triangulate(mesh: &crate::mesh::Mesh) -> IndexedTriangles { /* ... */ }
```

#### Function `to_polymesh_text`

Serialise `mesh` to the five OpenFOAM `polyMesh` files as strings.

Pure and filesystem-free (testable): builds `points`, `faces`, `owner`,
`neighbour`, and `boundary` from the mesh's public topology. Faces are
written in the mesh's own winding order. Every face is assigned owner cell
`0` (a surface has no real cells) and `neighbour` is empty, so the result is
a **boundary patch for a volume mesher to fill, not a solve-ready volume
mesh** (see [`PolyMeshText`]).

Coordinates are copied verbatim as dimensionless model-space `f64`s; the
caller assigns a length unit (conventionally metres) at export.

```rust
pub fn to_polymesh_text(mesh: &crate::mesh::Mesh) -> PolyMeshText { /* ... */ }
```

#### Function `write_polymesh`

Write the five [`PolyMeshText`] files into `dir/`, creating `dir` if needed.

The only filesystem-touching function in this module: it calls
[`to_polymesh_text`] and writes `points`, `faces`, `owner`, `neighbour`, and
`boundary` (no `.txt` extension — OpenFOAM's exact file names) into `dir`.
Typically `dir` is a case's `constant/polyMesh` directory. The output is a
boundary surface, not a solve-ready volume mesh (see [`to_polymesh_text`]).

```rust
pub fn write_polymesh(mesh: &crate::mesh::Mesh, dir: &std::path::Path) -> std::io::Result<()> { /* ... */ }
```

#### Function `to_csg_primitive`

Fit `mesh` to an analytic CSG solid consumable by `outram-mc-libs`.

Recognises the [`crate::primitives`] shapes it can express exactly and emits
the matching [`CsgDescription`] (local mirror types — no dependency on
`outram-mc-libs`):

- **axis-aligned cube/box** (8 vertices, 6 quad faces, all normals ±X/±Y/±Z,
  every vertex on the bounding box) → six planes at the box bounds, region =
  the intersection of the six inward half-spaces (the box interior);
- **uv-sphere** (vertices equidistant from their centroid, and vertex/face
  counts matching a `2 + (rings-1)*segments` / `rings*segments` uv-sphere) →
  one `Sphere` at the fitted centre/radius, region = its interior
  (`Negative` half-space);
- **Z-axis cylinder** (a [`crate::primitives::cylinder`]: `2*segments`
  vertices, `segments` side quads + two `segments`-gon caps, side vertices at
  a constant radius about a Z-parallel axis) → one `ZCylinder` intersected
  with two `ZPlane` caps;
- **any other convex closed polyhedron** → the **faceted convex** route: one
  general [`CsgSurface::Plane`] per face (outward normal), region = the
  intersection of every inward (`Negative`) half-space. This is exact for a
  convex solid — e.g. a rotated box or a convex boolean result — because a
  convex polyhedron *is* the intersection of its face half-spaces.

A **non-convex** mesh cannot be written as a single half-space intersection,
so it returns [`ExportError::NotImplemented`] here; use [`to_faceted_solid`]
for the DAGMC-style boundary representation of an arbitrary (non-convex)
solid.

Lengths are dimensionless model space; a length unit (conventionally metres)
is assigned when the description reaches the transport solver.

```rust
pub fn to_csg_primitive(mesh: &crate::mesh::Mesh) -> Result<CsgDescription, ExportError> { /* ... */ }
```

#### Function `to_faceted_solid`

Build the DAGMC-style [`FacetedSolid`] boundary of `mesh`: fan-triangulate
every face and orient the triangles **outward** (positive enclosed volume).

Works for any closed mesh, convex or not — this is the fallback the analytic
[`to_csg_primitive`] fitters do not cover. Outward orientation is enforced so
that a downstream consumer using face normals (not just the sign-agnostic
[`FacetedSolid::contains`]) sees them pointing out of the solid.

# This constructor does not validate the surface

It builds a [`FacetedSolid`] from whatever it is given, including an **open**
mesh — for which [`FacetedSolid::contains`] returns confident but arbitrary
answers (see [`FacetedSolid::check_closed_manifold`] for why). Prefer
[`to_faceted_solid_checked`] unless the mesh is already known closed; or call
[`FacetedSolid::check_closed_manifold`] on the result. Two further caveats
this constructor cannot detect:

- **Non-convex faces.** Fan triangulation from a face's first corner is exact
  only for **convex** faces (as [`crate::primitives`] produces). A concave
  `n`-gon — which [`crate::bisect`] and [`crate::boolean_general`] can emit —
  fans into overlapping and outside-the-face triangles.
- **Zero enclosed volume.** The outward-orientation flip below triggers on a
  strictly negative signed volume, so a surface enclosing exactly zero volume
  is left as-is rather than reported.

```rust
pub fn to_faceted_solid(mesh: &crate::mesh::Mesh) -> FacetedSolid { /* ... */ }
```

#### Function `to_faceted_solid_checked`

[`to_faceted_solid`], but **rejects** a surface on which the solid's
inside/outside test would be meaningless.

Builds the faceted boundary exactly as [`to_faceted_solid`] does, then runs
[`FacetedSolid::check_closed_manifold`] on it. Use this whenever the mesh's
closedness is not already guaranteed — an open or non-manifold boundary makes
[`FacetedSolid::contains`] silently arbitrary, which downstream shows up as a
transport particle in the wrong material rather than as an error.

# Errors

[`ExportError::DegenerateTriangle`], [`ExportError::NonManifoldSurface`], or
[`ExportError::NotClosedSurface`], naming the offending triangle and edge.

# Examples

```
use outram_blender::{primitives, export};

assert!(export::to_faceted_solid_checked(&primitives::cube(2.0)).is_ok());
// A flat patch encloses nothing; it is refused rather than silently accepted.
assert!(export::to_faceted_solid_checked(&primitives::grid(2, 2, 1.0)).is_err());
```

```rust
pub fn to_faceted_solid_checked(mesh: &crate::mesh::Mesh) -> Result<FacetedSolid, ExportError> { /* ... */ }
```

## Module `array_patterns`

**Array patterns** (`op-hzs.54.48`, GH issue #37 §J).

- [`radial_array`] / [`circular_array`] — copies rotated about an axis
  (pin lattices, MSR loops, sphere rings).
- [`object_offset_array`] — copies under a compounding [`Affine3`] offset
  (the Array modifier's *Object Offset*), spiralling / scaling stacks.
- [`array_along_curve`] — copies distributed along a [`crate::curve::Spline`],
  each oriented to the curve frame.
- [`ArrayCaps`] — optional start / end cap meshes on any of the above.

Every function returns one merged [`Mesh`]; copies are **not** welded (they
are separate shells), matching the Array modifier.

## Units

Positions/lengths are dimensionless model-space quantities; angles radians.

```rust
pub mod array_patterns { /* ... */ }
```

### Types

#### Struct `ArrayCaps`

Optional cap meshes placed at the ends of an array (Array modifier's
*Start Cap* / *End Cap*). Each cap is placed with the same offset the next
(or previous) copy would have had.

```rust
pub struct ArrayCaps {
    pub start: Option<std::sync::Arc<crate::mesh::Mesh>>,
    pub end: Option<std::sync::Arc<crate::mesh::Mesh>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `start` | `Option<std::sync::Arc<crate::mesh::Mesh>>` | Placed one offset step *before* the first copy. |
| `end` | `Option<std::sync::Arc<crate::mesh::Mesh>>` | Placed one offset step *after* the last copy. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ArrayCaps { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> ArrayCaps { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `radial_array`

`count` copies of `mesh`, copy `i` rotated by `i * step_angle` about `axis`
through `center`. `count` clamped `>= 1`.

```rust
pub fn radial_array(mesh: &crate::mesh::Mesh, count: usize, step_angle: f64, axis: crate::math::Vec3, center: crate::math::Vec3) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `radial_array_capped`

[`radial_array`] with [`ArrayCaps`].

```rust
pub fn radial_array_capped(mesh: &crate::mesh::Mesh, count: usize, step_angle: f64, axis: crate::math::Vec3, center: crate::math::Vec3, caps: &ArrayCaps) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `circular_array`

`count` copies of `mesh` spread evenly around a full turn about `axis`
through `center` (step `= 2π / count`).

```rust
pub fn circular_array(mesh: &crate::mesh::Mesh, count: usize, axis: crate::math::Vec3, center: crate::math::Vec3) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `object_offset_array`

`count` copies of `mesh`, copy `i` placed under `offset` applied `i` times
(the Array modifier's *Object Offset*). `count` clamped `>= 1`.

```rust
pub fn object_offset_array(mesh: &crate::mesh::Mesh, count: usize, offset: crate::transform::Affine3) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `object_offset_array_capped`

[`object_offset_array`] with [`ArrayCaps`].

```rust
pub fn object_offset_array_capped(mesh: &crate::mesh::Mesh, count: usize, offset: crate::transform::Affine3, caps: &ArrayCaps) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `array_along_curve`

`count` copies of `mesh` distributed at even parameter spacing along
`spline`, each translated to the sample point and rotated so its local
`align_axis` points along the curve tangent (and local +Y toward the curve
normal). `count` clamped `>= 1`.

```rust
pub fn array_along_curve(mesh: &crate::mesh::Mesh, spline: &crate::curve::Spline, count: usize, align_axis: crate::selection::Axis) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `array_along_curve_capped`

[`array_along_curve`] with [`ArrayCaps`] (caps sit at the curve ends,
oriented to the end frames).

```rust
pub fn array_along_curve_capped(mesh: &crate::mesh::Mesh, spline: &crate::curve::Spline, count: usize, align_axis: crate::selection::Axis, caps: &ArrayCaps) -> crate::mesh::Mesh { /* ... */ }
```

## Module `bool_tool`

**Bool Tool** (`op-hzs.54.45`, GH issue #37 §I) — a non-destructive stack of
brush cutters over a base mesh.

- [`BrushOp`] — Difference / Union / Intersect / Slice.
- [`BoolBrush`] — one cutter: an `Arc<Mesh>`, its op, and an `enabled`
  toggle. Nothing is applied until [`BoolStack::bake`].
- [`BoolStack`] — the ordered stack. [`BoolStack::bake`] folds every enabled
  brush into the base; [`BoolStack::slice_pieces`] returns the inside piece
  each `Slice` brush carves off as a separate mesh.
- **Carve mode** ([`BoolStack::carve`]) — a fast, best-effort bake that
  skips a brush the CSG cannot resolve instead of failing the whole stack.

The base and brushes are unchanged by any call here — that is what makes
the stack "non-destructive"; `bake` returns a fresh [`Mesh`].

```rust
pub mod bool_tool { /* ... */ }
```

### Types

#### Enum `BrushOp`

What a brush does to the base.

```rust
pub enum BrushOp {
    Difference,
    Union,
    Intersect,
    Slice,
}
```

##### Variants

###### `Difference`

Subtract the brush volume (`base \ brush`).

###### `Union`

Add the brush volume (`base ∪ brush`).

###### `Intersect`

Keep only the shared volume (`base ∩ brush`).

###### `Slice`

Keep the base outside the brush, and carve the inside off as a
separate piece (see [`BoolStack::slice_pieces`]).

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> BrushOp { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &BrushOp) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `BoolBrush`

One non-destructive cutter.

```rust
pub struct BoolBrush {
    pub mesh: std::sync::Arc<crate::mesh::Mesh>,
    pub op: BrushOp,
    pub enabled: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `std::sync::Arc<crate::mesh::Mesh>` | The cutter geometry (shared, never mutated). |
| `op` | `BrushOp` | What it does. |
| `enabled` | `bool` | Skipped by [`BoolStack::bake`] when `false`. |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: Arc<Mesh>, op: BrushOp) -> Self { /* ... */ }
  ```
  A new enabled brush.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> BoolBrush { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `BoolStack`

An ordered stack of brushes over a base mesh.

```rust
pub struct BoolStack {
    pub brushes: Vec<BoolBrush>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `brushes` | `Vec<BoolBrush>` | The brushes, applied in order by [`BoolStack::bake`]. |

##### Implementations

###### Methods

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```
  An empty stack.

- ```rust
  pub fn with(self: Self, brush: BoolBrush) -> Self { /* ... */ }
  ```
  Push a brush and return `self` (builder style).

- ```rust
  pub fn push(self: &mut Self, brush: BoolBrush) { /* ... */ }
  ```
  Add a brush in place.

- ```rust
  pub fn bake(self: &Self, base: &Mesh) -> Result<Mesh, BooleanError> { /* ... */ }
  ```
  Fold every **enabled** brush into `base`, in stack order, and return the

- ```rust
  pub fn carve(self: &Self, base: &Mesh) -> (Mesh, Vec<usize>) { /* ... */ }
  ```
  Like [`BoolStack::bake`], but a brush whose boolean fails is **skipped**

- ```rust
  pub fn slice_pieces(self: &Self, base: &Mesh) -> Vec<Result<Mesh, BooleanError>> { /* ... */ }
  ```
  The inside piece (`base ∩ brush`) that each enabled `Slice` brush carves

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> BoolStack { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> BoolStack { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `draw_tool`

**Interactive primitive-draw tool** (`op-hzs.54.41`, GH issue #37 §H) — the
CAD "draw a box / circle / cone" gesture, expressed as a headless staged
operator instead of a mouse-driven modal.

The gesture is: pick a [`WorkPlane`], drag out a footprint rectangle on it,
then drag a depth along the plane normal. Each 3-D input point can be run
through the [`crate::snap`] engine, and each scalar (a footprint side, the
depth) can be typed as an expression evaluated by
[`crate::transform_input::eval_expr`].

- [`WorkPlane`] — an oriented base plane (`origin`, orthonormal `u`, `v`,
  `normal`); [`WorkPlane::xy`] / [`WorkPlane::xz`] / [`WorkPlane::yz`] /
  [`WorkPlane::from_origin_normal`].
- [`DrawGesture`] — the staged state machine (`PickBase → DragFootprint →
  DragDepth → Done`); [`DrawGesture::resolve`] builds the [`Mesh`].
- [`box_from_drag`] / [`circle_from_drag`] / [`cone_from_drag`] — the
  one-shot forms when you already have the points.
- [`snap_input`] — project one world point onto a [`crate::snap::SnapTarget`].

## Units

Points and lengths are dimensionless model-space quantities (see
[`crate::math`]).

```rust
pub mod draw_tool { /* ... */ }
```

### Types

#### Struct `WorkPlane`

An oriented drawing plane: a point on it plus an orthonormal basis, where
`u` and `v` span the plane and `normal = u x v` is the extrude direction.

```rust
pub struct WorkPlane {
    pub origin: crate::math::Vec3,
    pub u: crate::math::Vec3,
    pub v: crate::math::Vec3,
    pub normal: crate::math::Vec3,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `origin` | `crate::math::Vec3` | A point on the plane (the gesture's local origin). |
| `u` | `crate::math::Vec3` | In-plane "x" axis (unit). |
| `v` | `crate::math::Vec3` | In-plane "y" axis (unit, perpendicular to `u`). |
| `normal` | `crate::math::Vec3` | Plane normal / extrude axis (unit, `= u x v`). |

##### Implementations

###### Methods

- ```rust
  pub fn xy() -> Self { /* ... */ }
  ```
  The world `x-y` plane through the origin, extruding along `+z`.

- ```rust
  pub fn xz() -> Self { /* ... */ }
  ```
  The world `x-z` plane, extruding along `+y`.

- ```rust
  pub fn yz() -> Self { /* ... */ }
  ```
  The world `y-z` plane, extruding along `+x`.

- ```rust
  pub fn from_origin_normal(origin: Vec3, normal: Vec3) -> Self { /* ... */ }
  ```
  A plane through `origin` with the given `normal` (need not be unit); the

- ```rust
  pub fn point(self: &Self, a: f64, b: f64, h: f64) -> Vec3 { /* ... */ }
  ```
  World-space point for plane coordinates `(a, b)` and height `h` along

- ```rust
  pub fn project(self: &Self, p: Vec3) -> (f64, f64) { /* ... */ }
  ```
  Project a world point onto plane coordinates `(u_coord, v_coord)`

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> WorkPlane { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &WorkPlane) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `DrawKind`

Which primitive a [`DrawGesture`] builds.

```rust
pub enum DrawKind {
    Box,
    Cylinder {
        segments: usize,
    },
    Cone {
        segments: usize,
    },
}
```

##### Variants

###### `Box`

A rectangular box.

###### `Cylinder`

A cylinder (footprint's shorter side is the diameter proxy — the
gesture uses the drag distance as the radius directly).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `segments` | `usize` | Sides around the axis. |

###### `Cone`

A cone with the apex at the depth end.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `segments` | `usize` | Sides around the base. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> DrawKind { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DrawKind) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `Stage`

The stage a [`DrawGesture`] is at.

```rust
pub enum Stage {
    PickBase,
    DragFootprint,
    DragDepth,
    Done,
}
```

##### Variants

###### `PickBase`

Waiting for the first base point.

###### `DragFootprint`

Have the first point; waiting for the opposite footprint corner / rim.

###### `DragDepth`

Have the footprint; waiting for the depth.

###### `Done`

Complete — [`DrawGesture::resolve`] will succeed.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Stage { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Stage) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `DrawGesture`

The staged "draw a primitive" operator. Feed it points (optionally
snapped by the caller via [`snap_input`]); call [`DrawGesture::resolve`]
once [`DrawGesture::stage`] is [`Stage::Done`].

```rust
pub struct DrawGesture {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(plane: WorkPlane, kind: DrawKind) -> Self { /* ... */ }
  ```
  Start a gesture on `plane` building `kind`.

- ```rust
  pub fn stage(self: &Self) -> Stage { /* ... */ }
  ```
  Current stage.

- ```rust
  pub fn push_point(self: &mut Self, world: Vec3) { /* ... */ }
  ```
  Supply the next point in the gesture (base corner, then footprint

- ```rust
  pub fn push_depth_point(self: &mut Self, world: Vec3) { /* ... */ }
  ```
  Supply the depth by a world point: the signed distance from the base

- ```rust
  pub fn set_depth(self: &mut Self, depth: f64) { /* ... */ }
  ```
  Supply the depth directly (or from [`eval_dimension`]).

- ```rust
  pub fn resolve(self: &Self) -> Option<Mesh> { /* ... */ }
  ```
  Build the mesh. `None` unless [`Self::stage`] is [`Stage::Done`].

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> DrawGesture { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `snap_input`

Project one world point onto a snap target of `mesh`, returning the snapped
position (or `p` unchanged if nothing is within `max_dist`).

```rust
pub fn snap_input(mesh: &crate::mesh::Mesh, p: crate::math::Vec3, target: crate::snap::SnapTarget, max_dist: f64) -> crate::math::Vec3 { /* ... */ }
```

#### Function `eval_dimension`

Evaluate a scalar that may be a literal or an expression (`"2*0.5"`,
`"pi/4"`); `None` on a parse error.

```rust
pub fn eval_dimension(s: &str) -> Option<f64> { /* ... */ }
```

#### Function `box_from_drag`

A box from two opposite base corners (world points, assumed on/near the
plane) and a `depth` along the plane normal. The footprint is the
axis-aligned (in plane coords) rectangle spanned by the two corners.

```rust
pub fn box_from_drag(plane: &WorkPlane, corner_a: crate::math::Vec3, corner_b: crate::math::Vec3, depth: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `circle_from_drag`

A cylinder from a base centre, a rim point (radius = their in-plane
distance) and a `depth` along the normal. `segments` clamped `>= 3`.

```rust
pub fn circle_from_drag(plane: &WorkPlane, center: crate::math::Vec3, rim: crate::math::Vec3, depth: f64, segments: usize) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `cone_from_drag`

A cone (apex up) from a base centre, a rim point and a `depth`.
`segments` clamped `>= 3`.

```rust
pub fn cone_from_drag(plane: &WorkPlane, center: crate::math::Vec3, rim: crate::math::Vec3, depth: f64, segments: usize) -> crate::mesh::Mesh { /* ... */ }
```

## Module `extra_objects`

**Extra-objects generators** (`op-hzs.54.40`, GH issue #37 §H) — the
parametric shapes Blender's *Add Mesh: Extra Objects* add-on provides.

- [`rounded_cube`] — a box with filleted edges/corners (rounded-box SDF
  projection of a subdivided cube).
- [`capsule`] — a cylinder capped by two hemispheres.
- [`spur_gear`] — an extruded trapezoidal-tooth spur gear.
- [`pipe`] / [`elbow`] — a straight pipe segment and a swept bend, both
  hollow (inner + outer wall), via [`crate::revolve`].
- [`wedge`] — a right-triangular prism.
- [`star`] — an extruded star polygon.
- [`honeycomb`] — a hex-cell grid (flat).
- [`z_function_surface`] — a grid patch with `z = f(x, y)`.

## Units

All radii / lengths are dimensionless model-space quantities; angles are
radians; tooth/segment counts are clamped to sane minimums.

```rust
pub mod extra_objects { /* ... */ }
```

### Functions

#### Function `rounded_cube`

A box of full extent `size` on each axis with edges and corners rounded to
`radius`, built by projecting a `segments`-subdivided cube onto the
rounded-box surface.

`radius` is clamped to `< size/2`; `segments` (per face edge, clamped
`>= 2`) controls how finely the fillets are tessellated. Closed genus-0,
`chi = 2` (after the shared-corner welding that [`Mesh::from_polygons`]
does).

```rust
pub fn rounded_cube(size: f64, radius: f64, segments: usize) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `capsule`

A capsule about the `z` axis: a cylinder of `radius` and cylindrical length
`length` (the straight part), capped top and bottom by hemispheres of the
same `radius`.

`segments` around the axis (clamped `>= 3`), `rings` per hemisphere
(clamped `>= 1`). Total height is `length + 2*radius`. Closed genus-0,
`chi = 2`.

```rust
pub fn capsule(radius: f64, length: f64, segments: usize, rings: usize) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `spur_gear`

An extruded spur gear about the `z` axis with `teeth` trapezoidal teeth.

`root_radius` is the radius at the tooth root, `tooth_height` the added
radial length of each tooth, `width` the extrusion depth along `z`
(centred on `z = 0`). `tooth_frac` in `(0, 1)` is the fraction of each
angular pitch the tooth tip occupies (`0.5` ≈ equal land/gap). `teeth` is
clamped `>= 3`. Closed genus-0 prism, `chi = 2`.

```rust
pub fn spur_gear(teeth: usize, root_radius: f64, tooth_height: f64, width: f64, tooth_frac: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `pipe`

A straight hollow pipe about the `z` axis: outer radius `outer`, wall
thickness `wall`, length `length` (centred on `z = 0`), `segments` around
(clamped `>= 3`). Both ends are open annular rims. Genus-1 (a tube),
`chi = 0`.

```rust
pub fn pipe(outer: f64, wall: f64, length: f64, segments: usize) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `elbow`

A swept pipe bend ("elbow"): a hollow annular cross-section (outer radius
`outer`, wall `wall`) swept along a circular arc of `bend_radius` through
`angle` radians.

The arc lies in the `x-y` plane starting along `+x`; `arc_segments` steps
along the bend, `tube_segments` around the section (both clamped `>= 3` /
`>= 2`). Open annular ends. Genus-1, `chi = 0`.

```rust
pub fn elbow(outer: f64, wall: f64, bend_radius: f64, angle: f64, arc_segments: usize, tube_segments: usize) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `wedge`

A right-triangular prism ("wedge"): the triangle has legs `size_x` (along
`+x`) and `size_z` (along `+z`) with the right angle at the origin;
extruded `size_y` along `+y`. Closed genus-0, `chi = 2`.

```rust
pub fn wedge(size_x: f64, size_y: f64, size_z: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `star`

An extruded star polygon in the `z = 0` plane: `points` spikes alternating
between `outer_radius` and `inner_radius`, extruded `depth` along `z`
(centred). `points` clamped `>= 2`. `depth = 0` gives the flat filled
outline. Closed genus-0 for `depth > 0`.

```rust
pub fn star(points: usize, outer_radius: f64, inner_radius: f64, depth: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `honeycomb`

A flat honeycomb: `rows` x `cols` pointy-top hexagonal cells of
circumradius `cell_radius` in the `z = 0` plane, each cell a single 6-gon
face, packed on the standard offset hex lattice.

Returns one face per cell (`rows * cols` faces); shared cell edges are
deduplicated by [`Mesh::from_polygons`].

```rust
pub fn honeycomb(rows: usize, cols: usize, cell_radius: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `z_function_surface`

A grid patch over `[-extent_x, extent_x] x [-extent_y, extent_y]` in the
`x-y` plane with height `z = f(x, y)`, tessellated `nx` by `ny` quads
(each clamped `>= 1`).

`f` is any `Fn(f64, f64) -> f64` (a plain generic — no trait object), so
callers pass a closure. A topological disc, `chi = 1`.

```rust
pub fn z_function_surface<F: Fn(f64, f64) -> f64>(nx: usize, ny: usize, extent_x: f64, extent_y: f64, f: F) -> crate::mesh::Mesh { /* ... */ }
```

## Module `extrude`

**Extrude family** (`op-hzs.54.10`, GH issue #37 §B) — the extrude modes
[`crate::ops::extrude_faces`] (region, fixed vector) and
[`crate::ops::extrude_edges`] do not cover:

- [`extrude_faces_individual`] — each face lifted along **its own** normal,
  with its own side walls (independent bumps).
- [`extrude_faces_along_normals`] — a region lifted with **each vertex**
  moving along its averaged normal, so a curved patch thickens evenly.
- [`extrude_vertices`] — selected vertices duplicated and joined to the
  originals by new edges (a wire extrude).
- [`extrude_manifold`] — region extrude that also removes the original
  faces (the source region becomes a clean opening bridged by the walls);
  for a standalone face group this equals the region extrude.

"Extrude to Cursor" is `extrude_* ` followed by a translate by the caller,
so it needs no dedicated entry point.

```rust
pub mod extrude { /* ... */ }
```

### Functions

#### Function `extrude_faces_individual`

Extrude each face in `faces` **individually** by `amount` along its own
outward normal. Each face gets its own duplicated top and side walls; the
original faces are removed.

```rust
pub fn extrude_faces_individual(mesh: &crate::mesh::Mesh, faces: &[crate::mesh::FaceId], amount: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `extrude_faces_along_normals`

Extrude the region `faces` by `amount`, each **vertex** moving along its
averaged (area-weighted) normal over the selected faces. The selected faces
become the raised top; boundary edges gain side walls.

```rust
pub fn extrude_faces_along_normals(mesh: &crate::mesh::Mesh, faces: &[crate::mesh::FaceId], amount: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `extrude_vertices`

Extrude selected `verts` by `offset` — duplicate each and add an edge (a
degenerate two-sided face is avoided by emitting nothing but the edge via a
wire; here we add a thin quad so the polygon-soup mesh keeps it). For a
surface mesh the more useful call is [`crate::ops::extrude_edges`]; this
covers the lone-vertex / wire case.

```rust
pub fn extrude_vertices(mesh: &crate::mesh::Mesh, verts: &[crate::mesh::VertexId], offset: crate::math::Vec3) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `extrude_manifold`

Region extrude that removes the original faces — the source region becomes a
clean opening bridged by the walls (Blender's Extrude Manifold). For a
standalone face group this is the same as [`crate::ops::extrude_faces`].

```rust
pub fn extrude_manifold(mesh: &crate::mesh::Mesh, faces: &[crate::mesh::FaceId], offset: crate::math::Vec3) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `selection_boundary_edges`

The set of boundary edges of a face selection — handy for a caller wiring
"extrude then move the new boundary".

```rust
pub fn selection_boundary_edges(mesh: &crate::mesh::Mesh, faces: &[crate::mesh::FaceId]) -> Vec<crate::mesh::EdgeId> { /* ... */ }
```

## Module `fill`

**Fill operators** (`op-hzs.54.15`, GH issue #37 §B).

- [`make_face`] — add one face through the given ordered vertices (Blender's
  `F` when it closes a loop). Two vertices with no face between them add a
  wire edge.
- [`grid_fill`] — fill a closed boundary loop of `2·(w + h)` vertices with a
  `w × h` quad grid, splitting the loop into four sides at `span`
  (Blender's `Face ▸ Grid Fill`).
- [`beauty_fill`] — flip the shared diagonal of adjacent triangle pairs
  toward the Delaunay (max-min-angle) criterion (Blender's `Face ▸ Beauty
  Fill`).
- Simple hole capping is [`crate::fill_holes`]; the F-fill of an *edge net*
  into multiple faces is tracked as follow-up.

```rust
pub mod fill { /* ... */ }
```

### Functions

#### Function `make_face`

Add one face through `verts` in the given order. If `verts.len() == 2` a
wire edge is recorded instead (a zero-area sliver in the soup model).
Returns the rebuilt mesh; `verts.len() < 2` is a no-op.

```rust
pub fn make_face(mesh: &crate::mesh::Mesh, verts: &[crate::mesh::VertexId]) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `grid_fill`

Fill the closed boundary loop `boundary` (ordered vertex ring) with a quad
grid. `span` is the number of edges on the first side; the loop must have
`2 · span + 2 · other` vertices for some `other >= 1`. Returns the mesh
unchanged if that does not hold.

```rust
pub fn grid_fill(mesh: &crate::mesh::Mesh, boundary: &[crate::mesh::VertexId], span: usize) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `beauty_fill`

Flip the shared diagonal of every adjacent triangle pair toward the
max-min-angle (Delaunay) criterion — one pass. Non-triangle faces are left
alone. Returns the rebuilt mesh.

```rust
pub fn beauty_fill(mesh: &crate::mesh::Mesh) -> crate::mesh::Mesh { /* ... */ }
```

## Module `fill_helpers`

**Fill / mirror helpers** (`op-hzs.54.46`, GH issue #37 §I).

- [`f2_fill`] — the F2 "smart F": from one boundary edge, close the corner
  with a quad when the two neighbouring boundary edges allow it, else a
  triangle.
- [`auto_mirror`] — bisect the mesh by a plane, keep one half, mirror it
  back and weld along the cut (Auto Mirror in one call).
- [`bsurfaces`] — a lofted quad surface through a set of ordered strokes
  (Bsurfaces from annotation strokes).

## Units

Positions are dimensionless model-space quantities (see [`crate::math`]).

```rust
pub mod fill_helpers { /* ... */ }
```

### Functions

#### Function `f2_fill`

Context-aware fill from a single boundary edge (F2's smart `F`).

`edge` must be a boundary edge (used by exactly one face). The two boundary
edges sharing its endpoints are followed to their far vertices `c` (past
the `verts[0]` end) and `d` (past the `verts[1]` end):

- `c == d` → a triangle `(a, b, c)` is added;
- otherwise → a quad `(c, a, b, d)` is added.

Returns the mesh unchanged if `edge` is not a boundary edge or the
neighbours cannot be found.

```rust
pub fn f2_fill(mesh: &crate::mesh::Mesh, edge: crate::mesh::EdgeId) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `auto_mirror`

Bisect `mesh` by `plane` (keeping the half on the `−normal` side, matching
[`crate::bisect::bisect`]), mirror that half across the plane, and weld the
two halves along the cut with tolerance `weld_dist`.

The result is symmetric about `plane`. If the mesh lies entirely on one
side, the kept half is just mirrored and welded (a doubled shell).

```rust
pub fn auto_mirror(mesh: &crate::mesh::Mesh, plane: &crate::draw_tool::WorkPlane, weld_dist: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `bsurfaces`

A lofted quad surface through `strokes` (each an ordered polyline). Every
stroke is resampled to `cols` points; consecutive strokes are bridged into
a `(strokes.len()-1) x (cols-1)` quad grid.

`cols` is clamped `>= 2`; strokes with fewer than 2 points are skipped.
Needs at least two usable strokes, else an empty mesh.

```rust
pub fn bsurfaces(strokes: &[Vec<crate::math::Vec3>], cols: usize) -> crate::mesh::Mesh { /* ... */ }
```

## Module `fill_holes`

Fill holes — cap the open boundary loops of a surface so it becomes
watertight.

This is the pure-Rust analogue of Blender's **Fill Holes**
(`bmesh` `bmo_holes_fill`): every boundary loop (a closed chain of edges
each incident to only one face) is capped with a triangle fan to the loop's
centroid, closing the surface. Together with [`crate::weld`] it is the
mesh-repair pair the export bridges rely on — weld stitches near-coincident
seams, fill-holes closes genuine gaps — so that an open authored surface can
become the closed geometry the [`crate::export`] Monte-Carlo CSG bridge
needs.

# How a hole is found and capped

A **boundary edge** is a directed half-edge `a → b` (from a face's winding)
whose reverse `b → a` appears in no face — i.e. the edge borders exactly one
face. Chaining boundary half-edges tail-to-head recovers each boundary
**loop**. Each loop is capped by adding one **centroid** vertex at the
average of the loop's positions and one triangle per boundary edge.

## Winding

For the cap to be consistent with the existing surface, two adjacent faces
must traverse their shared edge in **opposite** directions. A boundary edge
`a → b` belongs to an existing face that traverses it `a → b`, so its cap
triangle must traverse it `b → a`; the emitted triangle is therefore
`[b, a, centroid]`. This keeps every face's outward normal consistent
without any explicit normal computation.

# Scope

The centroid fan is always topologically valid and makes the surface
watertight, but for a strongly non-planar or non-convex hole the fan is a
*valid* cap, not a *minimal-area* or *beauty* triangulation — an honest
limitation, documented rather than hidden. A loop with fewer than three
edges (degenerate) is left uncapped. No `faer`, no external dependency;
Android-safe.

```rust
pub mod fill_holes { /* ... */ }
```

### Functions

#### Function `fill_holes`

Cap every open boundary loop of `mesh` with a centroid triangle fan,
returning the watertight mesh.

A mesh that is already closed (no boundary edges) is returned unchanged (a
no-op rebuild). Each hole adds one centroid vertex and one triangle per
boundary edge; the winding of every cap triangle is chosen to stay
consistent with the surrounding surface.

This is infallible: the result is always a valid mesh. Degenerate boundary
loops (fewer than three edges) are left uncapped.

# Examples

```
use outram_blender::{primitives, fill_holes::fill_holes};

// A closed cube has no holes, so filling is a no-op: still V=8, F=6, chi=2.
let cube = primitives::cube(2.0);
let filled = fill_holes(&cube);
assert_eq!(filled.vertex_count(), 8);
assert_eq!(filled.face_count(), 6);
assert_eq!(filled.euler_characteristic(), 2);
```

```rust
pub fn fill_holes(mesh: &crate::mesh::Mesh) -> crate::mesh::Mesh { /* ... */ }
```

## Module `inset`

Inset faces — replace each face with a smaller inner copy plus a bridging
ring of quads.

This is the pure-Rust analogue of Blender's **Inset Faces**
(`bmo_inset`, *Individual* mode): every face is shrunk toward its own
centroid by a fraction, the shrunk copy becomes an **inner face**, and a
ring of quads bridges the original boundary to the inner face. It is the
standard modelling operation for adding a border loop around each face
(panelling, framing, controlled bevelling of the interior).

# Individual mode

Each face gets its **own** inner vertices — they are not shared between
adjacent faces — which is Blender's *Individual* inset. This keeps the
construction purely local and always valid: the original corner vertices
stay shared between neighbours (so the surface stays manifold and closed),
while the inset ring is independent per face.

# Winding

The inner face keeps the original winding (outward normal preserved). Each
ring quad `[vi, v(i+1), v(i+1)', vi']` traverses the inner edge opposite to
the inner face, so the whole result stays consistently wound — the same
adjacent-faces-oppose rule used elsewhere in the crate. No `faer`, no
external dependency; Android-safe.

```rust
pub mod inset { /* ... */ }
```

### Functions

#### Function `inset_faces`

Inset every face of `mesh` by `amount`, returning the new mesh.

`amount` is the fraction each corner moves **toward its face centroid**:
`0.0` leaves the face unchanged (a no-op), `0.5` halves the face, and values
approaching `1.0` collapse the inner face onto the centroid. Values `<= 0`
are treated as a no-op; values `>= 1` are clamped just below `1` to avoid a
degenerate zero-area inner face.

Each face becomes `1 + k` faces (one inner `k`-gon plus `k` ring quads) and
gains `k` new inner vertices. This is infallible.

# Examples

```
use outram_blender::{primitives, inset::inset_faces};

// Inset every face of a cube by 30%: 6 faces → 6 inner quads + 24 ring
// quads = 30 faces, still a closed genus-0 surface (χ = 2).
let cube = primitives::cube(2.0);
let inset = inset_faces(&cube, 0.3);
assert_eq!(inset.face_count(), 30);
assert_eq!(inset.euler_characteristic(), 2);
```

```rust
pub fn inset_faces(mesh: &crate::mesh::Mesh, amount: f64) -> crate::mesh::Mesh { /* ... */ }
```

## Module `knife`

**Knife** (`op-hzs.54.6`, GH issue #37 §B) — cut new edges across faces
along a path of [boundary points](KnifePoint).

Blender's interactive knife resolves a screen-space polyline into a chain of
vertex / edge-crossing / face-interior points; this module takes that chain
already resolved — as a list of [`Chord`]s, each a straight cut across one
face between two points on its boundary — and rebuilds the mesh with every
crossed edge split and every crossed face divided in two.

Resolving a raw polyline (or another object's silhouette, for **Knife
Project**) into [`Chord`]s is the caller's job for now; a
`project_polyline` helper that walks the surface is tracked as follow-up
under this bead.

Each [`knife`] chord splits exactly one face. Multiple chords on the same
face are applied in sequence, each acting on whichever sub-face contains it.

```rust
pub mod knife { /* ... */ }
```

### Types

#### Enum `KnifePoint`

A point on the boundary of a face — where a [`Chord`] starts or ends.

```rust
pub enum KnifePoint {
    Vertex(crate::mesh::VertexId),
    EdgeSplit {
        edge: crate::mesh::EdgeId,
        t: f64,
    },
}
```

##### Variants

###### `Vertex`

An existing vertex of the face.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::mesh::VertexId` |  |

###### `EdgeSplit`

A new vertex `t` of the way along `edge` (from its `verts[0]` to
`verts[1]`), `0 < t < 1`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `edge` | `crate::mesh::EdgeId` |  |
| `t` | `f64` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> KnifePoint { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &KnifePoint) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `Chord`

One straight knife cut across a single face, between two points on its
boundary. The two points must lie on *different* sides / vertices of the
face (a chord, not a degenerate zero-length cut).

```rust
pub struct Chord {
    pub face: crate::mesh::FaceId,
    pub from: KnifePoint,
    pub to: KnifePoint,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `face` | `crate::mesh::FaceId` | The face to cut, by its id in the **input** mesh. |
| `from` | `KnifePoint` | Where the cut enters. |
| `to` | `KnifePoint` | Where the cut leaves. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Chord { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `KnifeResult`

The result of [`knife`].

```rust
pub struct KnifeResult {
    pub mesh: crate::mesh::Mesh,
    pub cut_vertices: Vec<crate::mesh::VertexId>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `crate::mesh::Mesh` | The rebuilt mesh. Ids may have moved; the new cut vertices are listed in<br>[`KnifeResult::cut_vertices`]. |
| `cut_vertices` | `Vec<crate::mesh::VertexId>` | Every vertex the knife introduced or cut along, in chord order (each<br>chord contributes its `from` then `to` vertex). Duplicates are kept so a<br>caller can see the per-chord pairing. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> KnifeResult { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `knife`

Apply `chords` to `mesh`. Chords are grouped by face; within a face they are
applied in the given order. A chord whose endpoints resolve to the same
point, or whose face is not found, is skipped.

```rust
pub fn knife(mesh: &crate::mesh::Mesh, chords: &[Chord]) -> KnifeResult { /* ... */ }
```

## Module `laplacian`

Discrete **Laplacian operators** over a mesh, and implicit **Laplacian
smoothing** (mesh fairing) built on them.

Blender analogue: the Smooth / "Smooth Vertices" and Laplacian-smooth mesh
operators (`MOD_laplaciansmooth`, `bmo_smooth_laplacian`). This is the first
module in the crate that assembles a **global sparse linear system over the
mesh** and solves it — the intended home for the `faer` sparse solver the
crate re-exports (see [`crate`] top-level docs). Future geometry-processing
operators that need the same machinery (ARAP deformation, mesh
parameterization) build on the operators here.

## What "the Laplacian" means here

For a mesh with `n` vertices, the discrete Laplacian is the symmetric `n x n`
matrix `L` with, for each edge `(i, j)` of weight `w_ij`,

- `L[i][j] = L[j][i] = -w_ij` (off-diagonal), and
- `L[i][i] = sum_j w_ij` (diagonal = the vertex's total edge weight).

So **every row sums to zero** (`L * 1 = 0`, the constant vector is in the
null space) and `L` is symmetric. Two weightings are offered
([`LaplacianWeighting`]):

- **Uniform / umbrella** (`w_ij = 1`): the graph Laplacian — connectivity
  only, ignores geometry. Always positive semidefinite.
- **Cotangent** (`w_ij = (cot α + cot β) / 2`, where `α`, `β` are the angles
  opposite edge `(i, j)` in the one or two triangles that share it): the
  discrete Laplace–Beltrami operator — geometry-aware (this is the one that
  approximates the smooth surface Laplacian). A boundary edge, in only one
  triangle, contributes a single `cot α / 2`.

Polygon faces are fan-triangulated before the cotangent weights are read off,
and the crate's [`crate::mesh`] has no edge→incident-face adjacency, so this
module builds the edge→opposite-vertex map itself from
[`crate::mesh::Mesh::polygons`].

> **Untrusted AI-generated draft** until a human reviews it, per the
> workspace `RESPONSIBLE_USE.md`. Not for nuclear facility operation,
> reactor control, safety-critical, or licensing decisions.

```rust
pub mod laplacian { /* ... */ }
```

### Types

#### Enum `LaplacianWeighting`

Which discrete Laplacian weighting to assemble (enum dispatch, per the
workspace no-trait-objects rule).

```rust
pub enum LaplacianWeighting {
    Uniform,
    Cotangent,
}
```

##### Variants

###### `Uniform`

Graph / umbrella Laplacian — every edge weight is `1`. Depends only on
connectivity, not vertex positions; always positive semidefinite. Cheap
and robust, but not a geometric (Laplace–Beltrami) approximation.

###### `Cotangent`

Cotangent-weighted discrete **Laplace–Beltrami** operator — edge weight
`(cot α + cot β) / 2` from the angles opposite the edge in its incident
triangle(s). Geometry-aware (the correct discretization of the surface
Laplacian). Weights can go negative on very obtuse triangles, so the
bare operator is only positive semidefinite for a Delaunay-ish mesh.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> LaplacianWeighting { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &LaplacianWeighting) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `LaplacianError`

Errors from [`laplacian_smooth`].

```rust
pub enum LaplacianError {
    Assembly,
    NotPositiveDefinite,
}
```

##### Variants

###### `Assembly`

The sparse smoothing matrix could not be assembled (an invalid
row/column index reached the sparse-matrix constructor). Indicates a
caller/topology bug, not a numerical one.

###### `NotPositiveDefinite`

The sparse Cholesky factorization failed because the system `I + λL`
(restricted to the free vertices) is not positive definite. Can happen
with the [`LaplacianWeighting::Cotangent`] weighting on a very obtuse
(non-Delaunay) mesh at a large `lambda`. Try a smaller `lambda`, the
[`LaplacianWeighting::Uniform`] weighting, or a better-conditioned mesh.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Error**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(source: crate::laplacian::LaplacianError) -> Self { /* ... */ }
    ```

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `laplacian_triplets`

Assemble the discrete Laplacian `L` of `mesh` under `weighting`, as
`(n, triplets)` — the vertex count and a list of `(row, col, value)` entries
(COO / triplet form) suitable for building a sparse matrix.

`L` is symmetric with zero row sums (see the module docs). Diagonal entries
are summed into one triplet per vertex; off-diagonals are one `(i, j)` and
one `(j, i)` per edge. Vertices with no incident edge (isolated) contribute
no entries (an all-zero row).

The triplet order is unspecified (it comes from a hash map); the assembled
matrix is independent of that order. This is the input a sparse-matrix
builder ([`crate::faer`]) consumes; it is also directly testable by
materializing a small dense matrix (see this module's tests).

```rust
pub fn laplacian_triplets(mesh: &crate::mesh::Mesh, weighting: LaplacianWeighting) -> (usize, Vec<(usize, usize, f64)>) { /* ... */ }
```

#### Function `boundary_vertices`

Flag each vertex that lies on the mesh **boundary** — incident to an edge
used by only one triangle (an open border, e.g. a [`crate::primitives::grid`]
patch). Returns a `bool` per vertex in [`crate::mesh::VertexId`] order.

Boundary vertices are the ones a smoothing operator pins in place so the
surface does not shrink at its border. For a closed mesh (every edge shared
by two triangles) this is all `false`.

```rust
pub fn boundary_vertices(mesh: &crate::mesh::Mesh) -> Vec<bool> { /* ... */ }
```

#### Function `laplacian_smooth`

**Implicit Laplacian smoothing** (mesh fairing): relax vertex positions by
solving `(I + λL) x' = x` once per iteration, with boundary vertices pinned.

# What it computes

Implicit (backward-Euler) smoothing of the surface by the discrete Laplacian
`L` ([`laplacian_triplets`], under `weighting`). Each iteration solves the
sparse SPD system `(I + λL) x' = x` — unconditionally stable for any `lambda`
(unlike explicit `x' = x - λL x`, which diverges for `λ` too large). With the
[`LaplacianWeighting::Cotangent`] weighting this approximates mean-curvature
flow (it denoises toward a smooth surface and gently shrinks); the
[`LaplacianWeighting::Uniform`] weighting smooths by connectivity only.

**Boundary vertices are pinned** (held fixed; see [`boundary_vertices`]) so an
open patch does not shrink at its border. The system is solved on the free
(interior) vertices only, as a symmetric-positive-definite reduced system
(pinned neighbours move to the right-hand side), factorized with `faer`'s
sparse Cholesky. For a closed mesh every vertex is free.

The cotangent weights are **recomputed from the current positions each
iteration**, so the flow tracks the evolving geometry.

# Inputs / units

- `mesh` — source mesh (borrowed, unmodified); topology is preserved, only
  positions change.
- `weighting` — [`LaplacianWeighting::Uniform`] or `Cotangent`.
- `lambda` — smoothing strength (dimensionless, `>= 0`); larger = smoother
  per step. `0` is a no-op; unconditionally stable at any value.
- `iterations` — number of implicit steps (`0` returns a clone).

# Errors

[`LaplacianError::NotPositiveDefinite`] if the reduced system is not SPD
(see that variant), or [`LaplacianError::Assembly`] on an internal indexing
failure.

```rust
pub fn laplacian_smooth(mesh: &crate::mesh::Mesh, weighting: LaplacianWeighting, lambda: f64, iterations: u32) -> Result<crate::mesh::Mesh, LaplacianError> { /* ... */ }
```

#### Function `taubin_smooth`

**Taubin `λ|μ` smoothing** — explicit, shrinkage-free mesh denoising.

# What it computes

Taubin's two-pass low-pass filter (Taubin, *A Signal Processing Approach to
Fair Surface Design*, 1995). Each iteration applies two **explicit**
normalized-Laplacian passes with opposite-sign factors:

- a **shrinking** pass `x ← x + λ Δx` (`λ > 0`), then
- an **un-shrinking** pass `x ← x + μ Δx` (`μ < 0`, with `|μ| > λ`),

where `Δx_i = (Σ_j w_ij (x_j − x_i)) / (Σ_j w_ij)` is the *normalized*
discrete Laplacian (a move toward the weighted one-ring average). Choosing
`μ` slightly more negative than `−λ` gives a filter that removes
high-frequency noise while (unlike plain [`laplacian_smooth`]) **not**
shrinking the surface — the low frequencies that carry the overall shape pass
through almost unchanged. Boundary vertices are pinned.

Unlike [`laplacian_smooth`] this needs **no linear solve** (it is an explicit
filter), so it is cheaper per step but only conditionally stable — keep `λ`
in `(0, 1)` and `μ ∈ (−1, −λ)`.

# Inputs / units

- `mesh` — source mesh (borrowed; topology preserved, positions change).
- `weighting` — [`LaplacianWeighting::Uniform`] (robust; `Σw = degree > 0`)
  or `Cotangent` (geometry-aware; a near-zero weight sum on a degenerate
  one-ring is skipped).
- `lambda` — the shrinking factor, `0 < λ < 1` (e.g. `0.5`).
- `mu` — the un-shrinking factor, `−1 < μ < −λ` (e.g. `−0.53`).
- `iterations` — number of `λ|μ` pairs (`0` is a no-op).

# Verification note

The defining property (shrinkage-free) is checked in the module tests: on a
noisy sphere, Taubin reduces the radial noise while preserving the mean
radius far better than plain Laplacian smoothing shrinks it.

```rust
pub fn taubin_smooth(mesh: &crate::mesh::Mesh, weighting: LaplacianWeighting, lambda: f64, mu: f64, iterations: u32) -> crate::mesh::Mesh { /* ... */ }
```

## Module `limited_dissolve`

Limited dissolve — merge faces across edges that are nearly flat, then
drop the vertices left stranded mid-edge. A port of Blender's
**Limited Dissolve** (`BM_mesh_decimate_dissolve`).

# What this is for

This is the cleanup pass to run before handing a surface to a mesher.
A boolean, a subdivision or an imported STL typically leaves a surface
carrying far more faces than its shape needs: a flat wall arrives as
dozens of coplanar triangles, each contributing a face to the volume
mesh and a surface to the CSG bridge for no geometric reason. Limited
dissolve removes exactly those edges — the ones whose two faces are
within `angle_limit` of coplanar — and **moves no vertices at all**, so
the surface it produces is the same surface, just described with fewer
faces.

That "moves nothing" property is what separates it from
[`crate::decimate`]: QEM collapse approximates the shape and trades
accuracy for face count, while this is lossless on any region that is
genuinely planar.

`angle_limit` is a true angle in **radians**. Positions are in the
caller's length unit and are never modified.

# The cost function

Upstream scores each manifold edge by `-cos(theta)`, where `theta` is
the angle between the two adjacent face normals, and dissolves while the
cheapest edge scores below `-cos(angle_limit)`. Two coplanar faces give
`-1` (cheapest); perpendicular faces give `0`. Working in the cosine
rather than the angle avoids an `acos` per edge per update, and the
comparison direction is preserved because `-cos` is monotonic over
`[0, PI]`.

Joining a face pair changes the score of every edge on the merged face,
so those are re-costed each time — which is what lets a long flat strip
collapse into a single n-gon rather than stopping after one merge.

# What is NOT ported, and why

Upstream's operator takes a `BMO_Delimit` mask — stop dissolving at
material boundaries, UV seams, sharp-marked edges, vertex-group borders.
Every one of those needs per-loop custom-data layers this crate does not
have, so none is ported and the mask is absent from the API rather than
present and ignored. If material or seam boundaries are ever added to
this crate's [`crate::attributes`], this is the operator that needs to
learn about them.

Upstream's `USE_DEGENERATE_CHECK` (a projected self-intersection test on
the prospective merged face) is also not ported. Instead, a join is
refused when it would repeat a vertex in the merged ring — a cheaper,
stricter test that catches the cases that matter here. Stated rather
than left for the reader to discover.

# Other deviations

1. **`f64`, not `f32`.**
2. **A linear-scan priority queue**, not an indexed binary heap. Same pop
   order; `O(E)` per pop. Consistent with
   [`crate::polyfill_beautify`], and for the same reason.
3. **Rebuild, not in-place.** Returns a new [`Mesh`].

```rust
pub mod limited_dissolve { /* ... */ }
```

### Functions

#### Function `limited_dissolve`

Dissolve edges whose two faces are within `angle_limit` radians of
coplanar, then remove vertices left stranded in the middle of a
straight run.

Vertex positions are never changed; only faces are merged and redundant
corners dropped. Boundary and non-manifold edges are left alone. With
`angle_limit` of 0 nothing dissolves; [`DEFAULT_ANGLE_LIMIT`] is
upstream's 5°. Infallible.

# Examples

```
use outram_blender::{primitives, subdivide::{subdivide, SubdivideOptions}};
use outram_blender::limited_dissolve::{limited_dissolve, DEFAULT_ANGLE_LIMIT};

// Subdividing a cube's flat faces adds faces but no shape.
let cube = primitives::cube(2.0);
let opts = SubdivideOptions { cuts: 3, ..Default::default() };
let dense = subdivide(&cube, opts);
assert!(dense.face_count() > cube.face_count());

// Limited dissolve takes the shape back to six faces.
let clean = limited_dissolve(&dense, DEFAULT_ANGLE_LIMIT);
assert_eq!(clean.face_count(), 6);
```

```rust
pub fn limited_dissolve(mesh: &crate::mesh::Mesh, angle_limit: f64) -> crate::mesh::Mesh { /* ... */ }
```

### Constants and Statics

#### Constant `DEFAULT_ANGLE_LIMIT`

Blender's default angle limit for Limited Dissolve: 5°, in radians.

```rust
pub const DEFAULT_ANGLE_LIMIT: f64 = _;
```

## Module `loop_cut`

**Loop Cut and Slide** (`op-hzs.54.5`, GH issue #37 §B) — insert `cuts`
parallel edge loops around the [ring](crate::topology::edge_ring) of a seed
edge.

Blender's `Ctrl+R` tool. Each quad the ring crosses is cut into `cuts + 1`
quads by new edges perpendicular to the ring direction; `factor` in
`[-1, 1]` slides the whole set of loops between the two rails
(`0` = evenly spaced, `±1` = the outermost loop pressed against a rail).

Only **quad** faces are cut. The walk stops at a triangle / n-gon / pole /
non-manifold edge or the mesh boundary; a terminal ring edge whose far face
is not part of the loop still gets the new vertices spliced into that face's
boundary (it gains sides) so the result stays watertight — the same
T-junction-free behaviour as Blender when a loop cut ends at an n-gon.

[`loop_cut`] returns the rebuilt [`Mesh`] plus, per cut, the ordered vertex
chain of the new loop, so a caller can select it (mirroring the "and Slide"
tool leaving the new loop selected).

```rust
pub mod loop_cut { /* ... */ }
```

### Types

#### Struct `LoopCutResult`

The result of [`loop_cut`]: the rebuilt mesh and the new loops it added.

```rust
pub struct LoopCutResult {
    pub mesh: crate::mesh::Mesh,
    pub new_loops: Vec<Vec<crate::mesh::VertexId>>,
    pub closed: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `crate::mesh::Mesh` | The mesh with the new loops inserted. Rebuilt from a polygon soup, so<br>every id from the source mesh may have moved — remap selections against<br>[`LoopCutResult::new_loops`]. |
| `new_loops` | `Vec<Vec<crate::mesh::VertexId>>` | One entry per cut (in slide order), each the ordered [`VertexId`] chain<br>of that new edge loop in the returned [`LoopCutResult::mesh`]. |
| `closed` | `bool` | `true` if the ring closed on itself (a band all the way around). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> LoopCutResult { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `loop_cut`

Insert `cuts` edge loops across the ring of `seed`. `cuts == 0` returns a
clone with no new loops. `factor` is clamped to `[-1, 1]`.

```rust
pub fn loop_cut(mesh: &crate::mesh::Mesh, seed: crate::mesh::EdgeId, cuts: usize, factor: f64) -> LoopCutResult { /* ... */ }
```

## Module `loop_tools`

**LoopTools** (`op-hzs.54.42`, GH issue #37 §I) — shape operators on an
ordered vertex loop.

Every operator takes the mesh and a `loop_verts` slice giving the loop in
order (`cyclic` says whether it closes), and returns a new [`Mesh`] with
those vertices repositioned (topology unchanged) — except [`bridge`] /
[`loft`], which add faces between loops, and [`subdivide`], which splits the
loop's edges.

- [`circle`] — snap the loop to a best-fit circle (even angular spacing).
- [`flatten`] — project the loop onto its best-fit plane.
- [`relax`] — Laplacian smoothing along the loop.
- [`curve`] — pull the loop toward a Catmull–Rom spline through a subset of
  its own vertices.
- [`space`] — redistribute the loop to equal arc-length spacing.
- [`gstretch`] — redistribute the loop along an external stroke polyline.
- [`bridge`] — connect two equal-length loops with a quad strip.
- [`loft`] — [`bridge`] a sequence of loops.
- [`subdivide`] — split each loop edge at its midpoint.

## Units

Positions are dimensionless model-space quantities (see [`crate::math`]).

```rust
pub mod loop_tools { /* ... */ }
```

### Functions

#### Function `circle`

Snap the loop to the best-fit circle in its best-fit plane: same centroid,
radius = mean vertex distance, vertices placed at even angular spacing
starting from the first vertex's current angle.

```rust
pub fn circle(mesh: &crate::mesh::Mesh, loop_verts: &[crate::mesh::VertexId], _cyclic: bool) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `flatten`

Project the loop's vertices onto their best-fit plane.

```rust
pub fn flatten(mesh: &crate::mesh::Mesh, loop_verts: &[crate::mesh::VertexId]) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `relax`

Laplacian smoothing along the loop: each vertex moves a `factor` fraction
toward the midpoint of its two loop neighbours, `iterations` times.

```rust
pub fn relax(mesh: &crate::mesh::Mesh, loop_verts: &[crate::mesh::VertexId], cyclic: bool, iterations: usize, factor: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `curve`

Pull the loop toward a smooth Catmull–Rom spline through every `keep`-th
vertex (the "anchors"), by `factor`. `keep >= 2`.

```rust
pub fn curve(mesh: &crate::mesh::Mesh, loop_verts: &[crate::mesh::VertexId], cyclic: bool, keep: usize, factor: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `space`

Redistribute the loop's vertices to equal arc-length spacing along the
polyline through their current positions (endpoints of an open loop stay
put).

```rust
pub fn space(mesh: &crate::mesh::Mesh, loop_verts: &[crate::mesh::VertexId], cyclic: bool) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `gstretch`

Redistribute the loop's vertices to equal arc-length spacing along an
external `stroke` polyline (the "GStretch" grease-pencil behaviour).

```rust
pub fn gstretch(mesh: &crate::mesh::Mesh, loop_verts: &[crate::mesh::VertexId], stroke: &[crate::math::Vec3], cyclic: bool) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `bridge`

Bridge two equal-length ordered loops with a quad strip.

```rust
pub fn bridge(mesh: &crate::mesh::Mesh, loop_a: &[crate::mesh::VertexId], loop_b: &[crate::mesh::VertexId], cyclic: bool) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `loft`

Bridge a sequence of loops in order (`loft`). All loops must be the same
length.

```rust
pub fn loft(mesh: &crate::mesh::Mesh, loops: &[&[crate::mesh::VertexId]], cyclic: bool) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `subdivide`

Split each edge of the loop at its midpoint, subdividing the faces those
edges bound. A face that gains exactly two midpoints is cut in two along
the chord between them; a face gaining one keeps it as an extra boundary
vertex.

```rust
pub fn subdivide(mesh: &crate::mesh::Mesh, loop_verts: &[crate::mesh::VertexId], cyclic: bool) -> crate::mesh::Mesh { /* ... */ }
```

## Module `loop_subdivision`

**Loop subdivision** — a smooth subdivision surface for **triangle** meshes.

Blender analogue: the Subdivision-Surface modifier in its triangle path
(Loop is the triangle analogue of Catmull–Clark, which this crate already
provides for quads in [`crate::subdivision`]). Each refinement step splits
every triangle into four and moves vertices toward a limit surface that is
`C²` almost everywhere (`C¹` at extraordinary vertices), producing a
progressively smoother mesh while preserving topology.

## The stencils (Loop, 1987; with Warren's β)

Given a triangle mesh, one step produces:

- **Edge points** — one new vertex per edge. An *interior* edge `(a, b)`
  shared by two triangles with opposite vertices `c, d` gets
  `3/8 (a + b) + 1/8 (c + d)`; a *boundary* edge gets the midpoint
  `1/2 (a + b)`.
- **Repositioned original vertices** — an *interior* vertex `v` of valence
  `n` with neighbours `v_k` moves to `(1 − nβ) v + β Σ_k v_k`, where
  `β = 3/16` for `n = 3` and `β = 3/(8n)` otherwise (Warren's simplified
  weights). A *boundary* vertex moves to `3/4 v + 1/8 (b₁ + b₂)`, using only
  its two boundary neighbours — so the boundary curve refines independently
  of the interior and is preserved.
- **New faces** — each old triangle `(a, b, c)` with edge points
  `e_ab, e_bc, e_ca` becomes four triangles: `(a, e_ab, e_ca)`,
  `(e_ab, b, e_bc)`, `(e_ca, e_bc, c)`, `(e_ab, e_bc, e_ca)`.

Non-triangular input faces are fan-triangulated first. Boundary edges (used
by a single triangle) are detected from the face soup, as elsewhere in the
crate.

> **Untrusted AI-generated draft** until a human reviews it, per the
> workspace `RESPONSIBLE_USE.md`. Not for nuclear facility operation,
> reactor control, safety-critical, or licensing decisions.

```rust
pub mod loop_subdivision { /* ... */ }
```

### Functions

#### Function `loop_subdivide`

Apply `iterations` steps of **Loop subdivision** to `mesh`, returning the
refined triangle mesh.

Each step quadruples the triangle count and smooths toward the Loop limit
surface; `iterations = 0` fan-triangulates and returns (a no-op-shaped
clone). Topology (Euler characteristic, boundary loops) is preserved. Input
faces are triangulated first, so the output is always a triangle mesh.

```rust
pub fn loop_subdivide(mesh: &crate::mesh::Mesh, iterations: u32) -> crate::mesh::Mesh { /* ... */ }
```

## Module `math`

Minimal pure-Rust vector math for mesh authoring.

This module deliberately reimplements only the tiny slice of vector algebra
the mesh layer needs, rather than pulling in a linear-algebra crate. Blender
uses its own `blenlib` `BLI_math_vector` routines for the same reason: mesh
topology work needs 3-component positions, dot/cross products, and lengths —
nothing that justifies a heavy dependency. Keeping it dependency-free also
keeps the crate trivially Android-buildable (no BLAS, no C toolchain).

If richer linear algebra is ever needed, this is the single place to swap in
a pure-Rust crate such as `glam` (add it to the workspace `[dependencies]`
first). Positions are plain `f64` in model space; **no physical units are
attached here** — a mesh is dimensionless geometry until an [`crate::export`]
bridge assigns it a length unit for a solver.

```rust
pub mod math { /* ... */ }
```

### Types

#### Struct `Vec3`

A 3-component vector in model space, stored as `f64` components.

Used both for vertex positions and for direction vectors (normals, offsets).
Components are dimensionless model-space coordinates; the consuming solver
assigns a length unit at export time. `Vec3` is `Copy`, so it is passed by
value throughout the mesh layer (no borrows, per the workspace no-lifetimes
rule).

```rust
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `f64` | X component (model-space coordinate or direction). |
| `y` | `f64` | Y component (model-space coordinate or direction). |
| `z` | `f64` | Z component (model-space coordinate or direction). |

##### Implementations

###### Methods

- ```rust
  pub const fn new(x: f64, y: f64, z: f64) -> Self { /* ... */ }
  ```
  Construct a vector from explicit components.

- ```rust
  pub fn add(self: Self, other: Vec3) -> Vec3 { /* ... */ }
  ```
  Component-wise sum, `self + other`.

- ```rust
  pub fn sub(self: Self, other: Vec3) -> Vec3 { /* ... */ }
  ```
  Component-wise difference, `self - other`.

- ```rust
  pub fn scale(self: Self, s: f64) -> Vec3 { /* ... */ }
  ```
  Scale every component by `s`.

- ```rust
  pub fn dot(self: Self, other: Vec3) -> f64 { /* ... */ }
  ```
  Euclidean dot product `self · other`.

- ```rust
  pub fn cross(self: Self, other: Vec3) -> Vec3 { /* ... */ }
  ```
  Right-handed cross product `self × other`.

- ```rust
  pub fn length(self: Self) -> f64 { /* ... */ }
  ```
  Euclidean length (L2 norm), `sqrt(self · self)`.

- ```rust
  pub fn normalize(self: Self) -> Vec3 { /* ... */ }
  ```
  Unit vector in the same direction.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Vec3 { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Vec3) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `measure`

**Measurement & inspection** (`op-hzs.54.26`, GH issue #37 §D). Depends on
[`crate::snap`] for the closest-point helpers.

- Per-element readouts: [`edge_length`], [`face_area`], [`face_perimeter`],
  [`dihedral_angle`], [`corner_angle`].
- Whole-mesh: [`total_edge_length`], [`total_surface_area`],
  [`signed_volume`], [`bounding_box`], [`dimensions`].
- Tools: [`Ruler`] (distance), [`Protractor`] (angle).
- Mesh analysis: [`overhang`], [`distortion`], [`sharp_edges`],
  [`self_intersections`], [`thickness`].

```rust
pub mod measure { /* ... */ }
```

### Types

#### Struct `Ruler`

A two-point distance measurement (Blender's Ruler).

```rust
pub struct Ruler {
    pub a: crate::math::Vec3,
    pub b: crate::math::Vec3,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `crate::math::Vec3` |  |
| `b` | `crate::math::Vec3` |  |

##### Implementations

###### Methods

- ```rust
  pub fn distance(self: &Self) -> f64 { /* ... */ }
  ```
  The measured distance.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Ruler { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `Protractor`

A three-point angle measurement — the angle at `vertex` (Blender's
Protractor).

```rust
pub struct Protractor {
    pub a: crate::math::Vec3,
    pub vertex: crate::math::Vec3,
    pub b: crate::math::Vec3,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `crate::math::Vec3` |  |
| `vertex` | `crate::math::Vec3` |  |
| `b` | `crate::math::Vec3` |  |

##### Implementations

###### Methods

- ```rust
  pub fn angle(self: &Self) -> f64 { /* ... */ }
  ```
  The measured angle at `vertex`, in radians.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Protractor { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `edge_length`

Length of an edge (`0.0` if out of range).

```rust
pub fn edge_length(mesh: &crate::mesh::Mesh, e: crate::mesh::EdgeId) -> f64 { /* ... */ }
```

#### Function `face_area`

Area of a face by Newell's method (robust for non-planar / concave faces).

```rust
pub fn face_area(mesh: &crate::mesh::Mesh, f: crate::mesh::FaceId) -> f64 { /* ... */ }
```

#### Function `face_perimeter`

Perimeter of a face (sum of its edge lengths).

```rust
pub fn face_perimeter(mesh: &crate::mesh::Mesh, f: crate::mesh::FaceId) -> f64 { /* ... */ }
```

#### Function `dihedral_angle`

Angle between the two faces on an edge, in radians (`0` = flat, `π` =
folded flat back). `None` if the edge does not have exactly two faces.

```rust
pub fn dihedral_angle(mesh: &crate::mesh::Mesh, e: crate::mesh::EdgeId) -> Option<f64> { /* ... */ }
```

#### Function `corner_angle`

Interior angle of face `f` at corner `v`, in radians.

```rust
pub fn corner_angle(mesh: &crate::mesh::Mesh, f: crate::mesh::FaceId, v: crate::mesh::VertexId) -> Option<f64> { /* ... */ }
```

#### Function `total_edge_length`

Sum of all edge lengths.

```rust
pub fn total_edge_length(mesh: &crate::mesh::Mesh) -> f64 { /* ... */ }
```

#### Function `total_surface_area`

Sum of all face areas.

```rust
pub fn total_surface_area(mesh: &crate::mesh::Mesh) -> f64 { /* ... */ }
```

#### Function `signed_volume`

Signed volume of the mesh via the divergence theorem (`Σ (a · (b × c)) / 6`
over a fan triangulation of each face). Meaningful for a **closed**,
consistently-wound surface; positive for outward-facing winding.

```rust
pub fn signed_volume(mesh: &crate::mesh::Mesh) -> f64 { /* ... */ }
```

#### Function `bounding_box`

Axis-aligned bounding box `(min, max)`.

```rust
pub fn bounding_box(mesh: &crate::mesh::Mesh) -> (crate::math::Vec3, crate::math::Vec3) { /* ... */ }
```

#### Function `dimensions`

The model's dimensions (bounding-box extent) — Blender's N-panel *Dimensions*.

```rust
pub fn dimensions(mesh: &crate::mesh::Mesh) -> crate::math::Vec3 { /* ... */ }
```

#### Function `overhang`

Overhang: the angle of face `f`'s normal from `up`, in radians. `0` = the
face points straight up; `π` = straight down. Used to flag unsupported
overhangs for additive manufacturing.

```rust
pub fn overhang(mesh: &crate::mesh::Mesh, f: crate::mesh::FaceId, up: crate::math::Vec3) -> f64 { /* ... */ }
```

#### Function `distortion`

Distortion: how far face `f` deviates from planar, as the maximum angle
(radians) between its per-triangle normals over a fan triangulation. `0` for
a triangle or a perfectly planar polygon.

```rust
pub fn distortion(mesh: &crate::mesh::Mesh, f: crate::mesh::FaceId) -> f64 { /* ... */ }
```

#### Function `sharp_edges`

Every edge whose dihedral angle exceeds `angle` radians — Blender's Mesh
Analysis "sharp" and the seed set for Mark Sharp.

```rust
pub fn sharp_edges(mesh: &crate::mesh::Mesh, angle: f64) -> Vec<crate::mesh::EdgeId> { /* ... */ }
```

#### Function `self_intersections`

Pairs of faces whose triangulations intersect. `O(F²)` broad phase on
bounding boxes then a triangle-triangle test — fine for interactive meshes,
not a spatial-hash implementation.

```rust
pub fn self_intersections(mesh: &crate::mesh::Mesh) -> Vec<(crate::mesh::FaceId, crate::mesh::FaceId)> { /* ... */ }
```

#### Function `thickness`

Local wall thickness at face `f`: cast a ray from its centroid along `-normal`
and return the distance to the first other face it hits, or `None`.

```rust
pub fn thickness(mesh: &crate::mesh::Mesh, f: crate::mesh::FaceId) -> Option<f64> { /* ... */ }
```

## Module `merge`

**Merge** (`op-hzs.54.11`, GH issue #37 §B) — collapse vertices together.

- [`merge_vertices`] collapses a vertex set to one point chosen by
  [`MergeTarget`] (centre / a supplied point / the first / the last of the
  set). This is Blender's `M` menu.
- [`merge_edges`] collapses each edge in a set to its midpoint (Blender's
  *Collapse*), independently.
- [`merge_by_distance`] merges only vertices *within* a given set that are
  closer than a threshold — the subset form of
  [`crate::weld::weld`] / Blender's *Merge by Distance* and the operation
  an **Auto-Merge** editor toggle runs after each edit.

```rust
pub mod merge { /* ... */ }
```

### Types

#### Enum `MergeTarget`

Where [`merge_vertices`] places the merged vertex.

```rust
pub enum MergeTarget {
    Center,
    Point(crate::math::Vec3),
    First,
    Last,
}
```

##### Variants

###### `Center`

The arithmetic mean of the merged vertices' positions.

###### `Point`

A caller-supplied point (Blender's *At Cursor*).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::math::Vec3` |  |

###### `First`

The position of the first vertex in the set (ascending id).

###### `Last`

The position of the last vertex in the set (ascending id).

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> MergeTarget { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &MergeTarget) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `merge_vertices`

Collapse `verts` into a single vertex placed per `target`. Faces that become
degenerate (fewer than three distinct corners) are dropped. Returns the
rebuilt mesh.

```rust
pub fn merge_vertices(mesh: &crate::mesh::Mesh, verts: &[crate::mesh::VertexId], target: MergeTarget) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `merge_edges`

Collapse each edge in `edges` to its midpoint, independently (Blender's
*Merge ▸ Collapse*). Chained edges collapse toward a shared vertex.

```rust
pub fn merge_edges(mesh: &crate::mesh::Mesh, edges: &[crate::mesh::EdgeId]) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `merge_by_distance`

Merge vertices *within* `verts` that lie within `threshold` of each other,
keeping the lowest id of each cluster. The subset form of
[`crate::weld::weld`].

```rust
pub fn merge_by_distance(mesh: &crate::mesh::Mesh, verts: &[crate::mesh::VertexId], threshold: f64) -> crate::mesh::Mesh { /* ... */ }
```

## Module `mesh`

BMesh-inspired **index-based half-edge** mesh topology.

This is the core data structure every other module operates on. It mirrors
the four-element model of Blender's **BMesh** (`source/blender/bmesh`):

| This crate | Blender BMesh | Role |
|---|---|---|
| [`Vertex`] | `BMVert` | a point in space |
| [`Edge`] | `BMEdge` | an undirected connection between two vertices |
| [`Loop`] | `BMLoop` | one corner of one face — a *directed* half-edge |
| [`Face`] | `BMFace` | a polygon, defined by its ring of loops |

The key idea borrowed from BMesh is the **loop**: a face is not stored as a
list of vertex indices but as a cycle of [`Loop`] records, each of which
knows its vertex, its edge, its face, and its `next`/`prev` neighbours
around the face. Two loops that share an edge but belong to different faces
are the two "half-edges" of that edge. This is what makes adjacency queries
(walk a face, walk around a vertex) O(1) instead of a search.

## No pointers, no lifetimes — indices only

Blender's C uses raw pointers between `BMVert`/`BMEdge`/`BMLoop`/`BMFace`.
The workspace design rules forbid `&'a`-linked graph nodes, so every link
here is a **newtype index** ([`VertexId`], [`EdgeId`], [`LoopId`],
[`FaceId`]) into one of the `Vec`s inside [`Mesh`]. This is the
`CellId(usize)`-into-a-`Vec` pattern the workspace `CLAUDE.md` prescribes.

## What this type does and does not cover

Implemented: incremental construction ([`Mesh::add_vertex`],
[`Mesh::add_face`] with automatic edge deduplication) and bulk construction
from a polygon soup ([`Mesh::from_polygons`]); element accessors
([`Mesh::vertex`], [`Mesh::edge`], [`Mesh::loop_at`], [`Mesh::face`]) and
counts; the soup view every operator module works over ([`Mesh::positions`],
[`Mesh::polygons`]); and the derived geometry queries
([`Mesh::face_vertices`], [`Mesh::face_normal`], [`Mesh::face_centroid`],
[`Mesh::euler_characteristic`]). That is the whole surface the
[`crate::primitives`] generators and the [`crate::ops`] / [`crate::modifiers`]
operators build on.

Deliberately **not** implemented: the full radial-cycle links around an edge
(BMesh's `radial_next`/`radial_prev`, needed to enumerate *all* faces on an
edge for non-manifold meshes), in-place Euler operators (split/join — the
operators here rebuild a fresh [`Mesh`] through [`Mesh::from_polygons`]
instead), and per-element custom data layers.

```rust
pub mod mesh { /* ... */ }
```

### Types

#### Struct `VertexId`

Index of a [`Vertex`] within a [`Mesh`]'s vertex array.

```rust
pub struct VertexId(pub usize);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> VertexId { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &K) -> Ordering { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &VertexId) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &VertexId) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &VertexId) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **RuleType**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `EdgeId`

Index of an [`Edge`] within a [`Mesh`]'s edge array.

```rust
pub struct EdgeId(pub usize);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> EdgeId { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &K) -> Ordering { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &EdgeId) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &EdgeId) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &EdgeId) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **RuleType**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `LoopId`

Index of a [`Loop`] within a [`Mesh`]'s loop array.

```rust
pub struct LoopId(pub usize);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> LoopId { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &K) -> Ordering { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &LoopId) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &LoopId) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &LoopId) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **RuleType**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `FaceId`

Index of a [`Face`] within a [`Mesh`]'s face array.

```rust
pub struct FaceId(pub usize);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FaceId { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &K) -> Ordering { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &FaceId) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FaceId) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &FaceId) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **RuleType**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `Vertex`

A mesh vertex: a point in model space plus one incident loop (BMesh `BMVert`).

```rust
pub struct Vertex {
    pub position: crate::math::Vec3,
    pub loop_id: Option<LoopId>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `position` | `crate::math::Vec3` | Position in dimensionless model space (see [`crate::math`]). |
| `loop_id` | `Option<LoopId>` | One [`Loop`] that starts at this vertex, or `None` for an isolated<br>vertex not yet used by any face. A full BMesh stores the disk cycle of<br>all incident edges; this crate deliberately keeps just a single<br>representative (see the module docs for what is and is not covered). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Vertex { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `Edge`

An undirected edge between two vertices (BMesh `BMEdge`).

```rust
pub struct Edge {
    pub verts: [VertexId; 2],
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `verts` | `[VertexId; 2]` | The two endpoint vertices. Order is the order the edge was first<br>created; [`Mesh::add_face`] treats `[a, b]` and `[b, a]` as the same<br>edge when deduplicating. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Edge { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `Loop`

One corner of one face — a directed half-edge (BMesh `BMLoop`).

A [`Face`] owns a cyclic list of loops. Following [`Loop::next`] repeatedly
walks the face's boundary counter-clockwise (as wound at construction) and
returns to the start after [`Face::len`] steps.

```rust
pub struct Loop {
    pub vert: VertexId,
    pub edge: EdgeId,
    pub face: FaceId,
    pub next: LoopId,
    pub prev: LoopId,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `vert` | `VertexId` | The vertex this loop-corner is anchored at (the *from* vertex of the<br>directed half-edge). |
| `edge` | `EdgeId` | The undirected [`Edge`] this loop runs along (from [`Loop::vert`] to the<br>next loop's vertex). |
| `face` | `FaceId` | The [`Face`] this loop belongs to. |
| `next` | `LoopId` | The next loop counter-clockwise around [`Loop::face`]. |
| `prev` | `LoopId` | The previous loop counter-clockwise around [`Loop::face`]. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Loop { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `Face`

A polygon face, stored as an entry point into its ring of loops (BMesh `BMFace`).

```rust
pub struct Face {
    pub loop_start: LoopId,
    pub len: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `loop_start` | `LoopId` | Any one loop on this face's boundary; walk [`Loop::next`] from here to<br>enumerate the whole face. |
| `len` | `usize` | Number of sides (vertices == edges == loops) on this face. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Face { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `Mesh`

An index-based half-edge mesh: the container all elements live in.

Build one with [`Mesh::new`], add geometry with [`Mesh::add_vertex`] and
[`Mesh::add_face`], then query it. All connectivity is by index into the
four `Vec`s below, so a `Mesh` is `Clone` and contains no borrows.

```rust
pub struct Mesh {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```
  Create an empty mesh with no vertices, edges, loops, or faces.

- ```rust
  pub fn add_vertex(self: &mut Self, position: Vec3) -> VertexId { /* ... */ }
  ```
  Add an isolated vertex at `position` and return its [`VertexId`].

- ```rust
  pub fn add_face(self: &mut Self, verts: &[VertexId]) -> FaceId { /* ... */ }
  ```
  Add a polygon face through the given vertices, in boundary order.

- ```rust
  pub fn vertex_count(self: &Self) -> usize { /* ... */ }
  ```
  Number of vertices (`V`).

- ```rust
  pub fn edge_count(self: &Self) -> usize { /* ... */ }
  ```
  Number of unique edges (`E`).

- ```rust
  pub fn loop_count(self: &Self) -> usize { /* ... */ }
  ```
  Number of loops (directed half-edge corners) — equals the sum of all

- ```rust
  pub fn face_count(self: &Self) -> usize { /* ... */ }
  ```
  Number of faces (`F`).

- ```rust
  pub fn vertex(self: &Self, id: VertexId) -> Option<&Vertex> { /* ... */ }
  ```
  Read-only access to a vertex by id, or `None` if out of range.

- ```rust
  pub fn edge(self: &Self, id: EdgeId) -> Option<&Edge> { /* ... */ }
  ```
  Read-only access to an edge by id, or `None` if out of range.

- ```rust
  pub fn loop_at(self: &Self, id: LoopId) -> Option<&Loop> { /* ... */ }
  ```
  Read-only access to a loop by id, or `None` if out of range.

- ```rust
  pub fn face(self: &Self, id: FaceId) -> Option<&Face> { /* ... */ }
  ```
  Read-only access to a face by id, or `None` if out of range.

- ```rust
  pub fn face_vertices(self: &Self, face: FaceId) -> Vec<VertexId> { /* ... */ }
  ```
  The vertices of a face, in boundary (loop) order.

- ```rust
  pub fn euler_characteristic(self: &Self) -> i64 { /* ... */ }
  ```
  The Euler characteristic `chi = V - E + F`.

- ```rust
  pub fn positions(self: &Self) -> Vec<Vec3> { /* ... */ }
  ```
  All vertex positions in [`VertexId`] order (`positions()[i]` is the

- ```rust
  pub fn polygons(self: &Self) -> Vec<Vec<VertexId>> { /* ... */ }
  ```
  Every face as its ring of [`VertexId`]s, in [`FaceId`] order

- ```rust
  pub fn from_polygons(positions: &[Vec3], faces: &[Vec<usize>]) -> Mesh { /* ... */ }
  ```
  Rebuild a [`Mesh`] from a positions array and a list of faces, each face

- ```rust
  pub fn face_normal(self: &Self, face: FaceId) -> Vec3 { /* ... */ }
  ```
  Unit outward normal of a face by **Newell's method**.

- ```rust
  pub fn face_centroid(self: &Self, face: FaceId) -> Vec3 { /* ... */ }
  ```
  Arithmetic mean (centroid) of a face's vertex positions.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Mesh { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Mesh { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `parameterize`

Planar **mesh parameterization** — flatten a disk-topology surface patch to
2D `(u, v)` coordinates by a harmonic / Tutte embedding.

Blender analogue: UV unwrapping (`uvedit` / `bmo_...unwrap`, the "Unwrap"
operator's underlying harmonic map). This is the crate's third sparse-solve
operator; it **reuses the same boundary-pinned SPD reduced system** as
[`crate::laplacian::laplacian_smooth`] — only the right-hand side and the
solution width (2, for `u`/`v`) differ.

## The map

For a mesh that is a **topological disk** (connected, orientable, genus 0,
with a single boundary loop), the parameterization:

1. **pins the boundary loop** to a convex target ([`BoundaryShape`] — a unit
   circle or square) by *arc length* along the loop, then
2. **solves the Laplace equation** `L x = 0` on the interior vertices with
   those boundary values fixed — a harmonic map. This is exactly the
   Dirichlet (grounded) Laplacian solve, `A_ff x_f = -L_fb x_b`, that the
   smoothing operator already assembles.

The [`crate::laplacian::LaplacianWeighting`] chooses the flavour:

- **Uniform** = *Tutte's barycentric embedding*. By Tutte's theorem, pinning
  the boundary of a 3-connected planar graph to a convex polygon yields a
  valid straight-line embedding — **no triangle flips or degeneracies**,
  guaranteed. Always solvable (SPD).
- **Cotangent** = *harmonic map*. Lower angle distortion (nearer conformal),
  but the guarantee is lost on obtuse (non-Delaunay) meshes, where a triangle
  can flip or the system can fail to be positive definite.

## Requirements

The mesh **must be an open disk**: a closed mesh (sphere) has no boundary to
pin and cannot be flattened without a cut; an annulus / disk-with-holes has
more than one boundary loop; a handle body is not genus 0. Each of these is
rejected with a specific [`ParamError`] rather than producing garbage.

> **Untrusted AI-generated draft** until a human reviews it, per the
> workspace `RESPONSIBLE_USE.md`. Not for nuclear facility operation,
> reactor control, safety-critical, or licensing decisions.

```rust
pub mod parameterize { /* ... */ }
```

### Types

#### Enum `BoundaryShape`

The convex boundary shape the mesh's border is pinned to.

```rust
pub enum BoundaryShape {
    Circle,
    Square,
}
```

##### Variants

###### `Circle`

The unit circle (radius 1, centred at the origin) — the natural default;
always convex, so Tutte's guarantee holds.

###### `Square`

The unit square `[0, 1]^2`, one quarter of the boundary length per side.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> BoundaryShape { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &BoundaryShape) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `ParamError`

Errors from [`parameterize`].

```rust
pub enum ParamError {
    NoBoundary,
    MultipleBoundaries,
    NonManifoldBoundary,
    NotADisk,
    Assembly,
    NotPositiveDefinite,
}
```

##### Variants

###### `NoBoundary`

The mesh has no boundary (it is closed). A closed surface cannot be
flattened to a disk without first introducing a cut/seam.

###### `MultipleBoundaries`

The mesh has more than one boundary loop (an annulus, or a disk with
holes). Only a single-boundary disk is supported.

###### `NonManifoldBoundary`

A boundary vertex has two outgoing boundary edges (a non-manifold
"bowtie" pinch); the boundary is not a simple loop.

###### `NotADisk`

The mesh is not a genus-0 disk (`Euler characteristic != 1`, e.g. it has
a handle).

###### `Assembly`

The sparse harmonic system could not be assembled.

###### `NotPositiveDefinite`

The sparse Cholesky factorization failed — the interior Laplacian system
is not positive definite (can happen with the cotangent weighting on an
obtuse mesh, or a disconnected interior). Try the uniform weighting.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Error**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `parameterize`

Parameterize `mesh` (a topological disk) to 2D, returning one `(u, v)` per
vertex in [`crate::mesh::VertexId`] order (`result[i]` is the UV of vertex
`i`).

Boundary vertices land exactly on the [`BoundaryShape`] (by arc length);
interior vertices are the harmonic solution under `weighting` (see the module
docs). Use [`LaplacianWeighting::Uniform`] for a guaranteed flip-free (Tutte)
embedding, or [`LaplacianWeighting::Cotangent`] for lower angle distortion on
a well-shaped mesh.

# Errors

See [`ParamError`] — the mesh must be a single-boundary genus-0 disk.

```rust
pub fn parameterize(mesh: &crate::mesh::Mesh, weighting: crate::laplacian::LaplacianWeighting, boundary: BoundaryShape) -> Result<Vec<(f64, f64)>, ParamError> { /* ... */ }
```

#### Function `flatten_to_plane`

Flatten `mesh` into the `z = 0` plane using its [`parameterize`] UVs: a new
mesh with the same faces but positions `(u, v, 0)`. Convenience wrapper when
a 2D *mesh* (rather than a UV list) is wanted.

```rust
pub fn flatten_to_plane(mesh: &crate::mesh::Mesh, weighting: crate::laplacian::LaplacianWeighting, boundary: BoundaryShape) -> Result<crate::mesh::Mesh, ParamError> { /* ... */ }
```

## Module `pdt`

**Precision Drawing Tools** (`op-hzs.54.44`, GH issue #37 §I) — the analytic
CAD constructions from Blender's PDT add-on, as pure functions on points
plus a few [`Mesh`] wrappers.

Placement (compute one point):
- [`Placement::Absolute`] / [`Placement::Delta`] / [`Placement::Polar`] /
  [`Placement::Percent`] → [`Placement::resolve`].

Constructions:
- [`three_point_circle`] — circumcircle (centre, radius, normal).
- [`three_point_arc`] — polyline along the arc `p0 → p1 → p2`.
- [`line_line_intersection`] — closest-approach point of two 3-D lines.
- [`fillet`] — tangent arc rounding a polyline corner.
- [`offset_polyline`] — parallel offset in a plane.
- [`taper`] — linear cross-section scaling along an axis.
- [`angle_between`] — the angle `∠(a, vertex, b)`.
- [`mirror_point`] / [`mirror_vertices`] — reflection across a
  [`crate::draw_tool::WorkPlane`].

## Units

Positions/lengths are dimensionless model-space quantities; angles radians;
`Percent` is a literal percentage (`50.0` = halfway).

```rust
pub mod pdt { /* ... */ }
```

### Types

#### Enum `Placement`

How to compute a single target point.

```rust
pub enum Placement {
    Absolute {
        coord: crate::math::Vec3,
    },
    Delta {
        from: crate::math::Vec3,
        delta: crate::math::Vec3,
    },
    Polar {
        from: crate::math::Vec3,
        plane: crate::draw_tool::WorkPlane,
        distance: f64,
        angle: f64,
    },
    Percent {
        a: crate::math::Vec3,
        b: crate::math::Vec3,
        percent: f64,
    },
}
```

##### Variants

###### `Absolute`

The point is exactly `coord`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `coord` | `crate::math::Vec3` |  |

###### `Delta`

`from + delta`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `from` | `crate::math::Vec3` |  |
| `delta` | `crate::math::Vec3` |  |

###### `Polar`

`from`, stepped `distance` along `angle` (radians) measured in `plane`
from `plane.u`, CCW about `plane.normal`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `from` | `crate::math::Vec3` |  |
| `plane` | `crate::draw_tool::WorkPlane` |  |
| `distance` | `f64` |  |
| `angle` | `f64` |  |

###### `Percent`

`a + (b - a) * percent/100`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `crate::math::Vec3` |  |
| `b` | `crate::math::Vec3` |  |
| `percent` | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn resolve(self: &Self) -> Vec3 { /* ... */ }
  ```
  Resolve to the world point.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Placement { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Placement) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `place_vertex`

Add a vertex at `placement`'s point to `mesh`, returning the new mesh and
the id.

```rust
pub fn place_vertex(mesh: &crate::mesh::Mesh, placement: &Placement) -> (crate::mesh::Mesh, crate::mesh::VertexId) { /* ... */ }
```

#### Function `three_point_circle`

The circumcircle of three points: `(centre, radius, unit normal)`, or
`None` if the points are collinear.

```rust
pub fn three_point_circle(p0: crate::math::Vec3, p1: crate::math::Vec3, p2: crate::math::Vec3) -> Option<(crate::math::Vec3, f64, crate::math::Vec3)> { /* ... */ }
```

#### Function `three_point_arc`

A polyline of `segments + 1` points along the circular arc that starts at
`p0`, passes through `p1`, and ends at `p2`. Falls back to the straight
chords `p0, p1, p2` if the points are collinear. `segments` clamped `>= 2`.

```rust
pub fn three_point_arc(p0: crate::math::Vec3, p1: crate::math::Vec3, p2: crate::math::Vec3, segments: usize) -> Vec<crate::math::Vec3> { /* ... */ }
```

#### Function `line_line_intersection`

The point of closest approach of line `A` (through `a0`, `a1`) and line
`B` (through `b0`, `b1`): the midpoint of the shortest connecting segment.
`None` if the lines are parallel.

```rust
pub fn line_line_intersection(a0: crate::math::Vec3, a1: crate::math::Vec3, b0: crate::math::Vec3, b1: crate::math::Vec3) -> Option<crate::math::Vec3> { /* ... */ }
```

#### Function `fillet`

A tangent fillet arc rounding the corner at `corner` between legs to
`prev` and `next`, with the given `radius`. Returns `segments + 1` points
from the tangent point on the `prev` leg to the one on the `next` leg
(empty if the legs are degenerate or the radius does not fit).

```rust
pub fn fillet(prev: crate::math::Vec3, corner: crate::math::Vec3, next: crate::math::Vec3, radius: f64, segments: usize) -> Vec<crate::math::Vec3> { /* ... */ }
```

#### Function `offset_polyline`

Parallel-offset a polyline by `distance` along the in-plane normal
(`plane_normal x segment_direction`), averaging the two adjacent segment
normals at each interior vertex. `cyclic` wraps the ends.

```rust
pub fn offset_polyline(pts: &[crate::math::Vec3], plane_normal: crate::math::Vec3, distance: f64, cyclic: bool) -> Vec<crate::math::Vec3> { /* ... */ }
```

#### Function `taper`

Taper `points` along `axis` (unit): each point's component perpendicular to
`axis`, measured from `pivot`, is scaled by `1 + rate * along`, where
`along` is its signed distance from `pivot` along `axis`.

```rust
pub fn taper(points: &[crate::math::Vec3], axis: crate::math::Vec3, rate: f64, pivot: crate::math::Vec3) -> Vec<crate::math::Vec3> { /* ... */ }
```

#### Function `angle_between`

The angle `∠(a, vertex, b)` in radians, in `[0, π]`.

```rust
pub fn angle_between(a: crate::math::Vec3, vertex: crate::math::Vec3, b: crate::math::Vec3) -> f64 { /* ... */ }
```

#### Function `mirror_point`

Reflect a point across a [`WorkPlane`].

```rust
pub fn mirror_point(p: crate::math::Vec3, plane: &crate::draw_tool::WorkPlane) -> crate::math::Vec3 { /* ... */ }
```

#### Function `mirror_vertices`

Reflect the given `verts` of `mesh` across `plane`, returning a new mesh
(topology unchanged; other vertices untouched).

```rust
pub fn mirror_vertices(mesh: &crate::mesh::Mesh, verts: &[crate::mesh::VertexId], plane: &crate::draw_tool::WorkPlane) -> crate::mesh::Mesh { /* ... */ }
```

## Module `modifiers`

Non-destructive **modifier stack** (Blender's `modifiers/intern/MOD_*`).

The [`Modifier`] enum and the [`ModifierStack`] that evaluates an ordered
list of them are real, compile, and produce geometry. Each modifier is a
pure function over the **polygon-soup** view of a mesh
([`Mesh::positions`] + [`Mesh::polygons`]): it reads the base mesh, computes
new positions and faces, and rebuilds a fresh [`Mesh`] through
[`Mesh::from_polygons`] (which recomputes edge dedup and loop wiring for
free). The base mesh is never mutated — that is the "non-destructive"
property that distinguishes a modifier from a [`crate::ops`] operator.

## Modifier stack vs. operators

A Blender **modifier** is *non-destructive*: it sits in an ordered stack on
an object and recomputes derived geometry from the original mesh every time,
leaving the base mesh untouched. That is the distinction from
[`crate::ops`], whose operators destructively edit a mesh in place. The
stack is evaluated top-to-bottom by [`ModifierStack::evaluate`].

## The modifiers (Blender analogue in parentheses)

- [`Modifier::Subsurf`] — Catmull-Clark subdivision surface at a view level
  (`MOD_subsurf`, backed by OpenSubdiv upstream). Forwards to the locked
  [`crate::subdivision::catmull_clark`] contract.
- [`Modifier::Mirror`] — mirror across one or more axis planes with a seam
  weld (`MOD_mirror`).
- [`Modifier::Array`] — repeat the mesh in a regular relative-offset pattern
  (`MOD_array`).

## Units

All coordinates are dimensionless model-space lengths (see [`crate::math`]);
the [`Modifier::Array`] `offset` is a *relative* offset expressed in
multiples of the input mesh's bounding-box extent along each axis, exactly
like Blender's Array modifier "Relative Offset" factor.

```rust
pub mod modifiers { /* ... */ }
```

### Types

#### Enum `ModifierError`

Errors returned while evaluating a [`Modifier`] or a [`ModifierStack`].

```rust
pub enum ModifierError {
    NotImplemented(&'static str),
    Failed(String),
}
```

##### Variants

###### `NotImplemented`

A modifier is scaffolded but its algorithm is not implemented yet.

Retained for forward compatibility (new modifier variants may land as
stubs).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `&'static str` |  |

###### `Failed`

A modifier's underlying operator failed (e.g. a boolean on
non-manifold input).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Error**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `MirrorAxes`

Which axis planes a [`Modifier::Mirror`] reflects across.

Each `bool` enables reflecting across the plane orthogonal to that axis; set
more than one to mirror sequentially (X, then Y, then Z).

```rust
pub struct MirrorAxes {
    pub x: bool,
    pub y: bool,
    pub z: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `bool` | Mirror across the YZ plane (reflect the X coordinate). |
| `y` | `bool` | Mirror across the XZ plane (reflect the Y coordinate). |
| `z` | `bool` | Mirror across the XY plane (reflect the Z coordinate). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> MirrorAxes { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> MirrorAxes { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &MirrorAxes) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `Modifier`

A closed set of non-destructive modifiers.

```rust
pub enum Modifier {
    Subsurf {
        levels: u32,
    },
    Mirror {
        axes: MirrorAxes,
    },
    Array {
        count: u32,
        offset: [f64; 3],
    },
    Bevel {
        options: crate::bevel::BevelOptions,
    },
    Boolean {
        operand: std::sync::Arc<crate::mesh::Mesh>,
        op: crate::ops::BooleanMode,
    },
    Solidify {
        thickness: f64,
    },
    Weld {
        distance: f64,
    },
    Wireframe {
        thickness: f64,
    },
    EdgeSplit {
        angle: f64,
    },
    Triangulate,
    Mask {
        keep: std::sync::Arc<Vec<crate::mesh::VertexId>>,
        invert: bool,
    },
    Remesh {
        voxel_size: f64,
        smooth_iterations: u32,
    },
    Screw {
        axis: crate::selection::Axis,
        turns: f64,
        steps: usize,
        screw_offset: f64,
    },
    Skin {
        radius: f64,
    },
    Build {
        factor: f64,
    },
    Multires {
        levels: u32,
    },
    Decimate {
        ratio: f64,
    },
    SimpleDeform {
        mode: crate::deform::SimpleDeform,
        axis: crate::selection::Axis,
    },
    Cast {
        target: crate::deform::CastTarget,
        factor: f64,
    },
    Displace {
        direction: crate::math::Vec3,
        strength: f64,
        noise_scale: f64,
        seed: u64,
    },
    Wave {
        axis: crate::selection::Axis,
        amplitude: f64,
        wavelength: f64,
        speed: f64,
        time: f64,
    },
}
```

##### Variants

###### `Subsurf`

Catmull-Clark subdivision to `levels` refinement passes.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `levels` | `u32` | Number of subdivision levels (viewport render level). `0` returns<br>the input unchanged; each level quadruples the face count of a<br>quad mesh. |

###### `Mirror`

Mirror the mesh across the selected axis planes and weld the seam.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `axes` | `MirrorAxes` | Which axis planes to reflect across. |

###### `Array`

Repeat the mesh `count` times, each copy offset by a multiple of the
mesh's bounding-box extent along each axis (relative-offset array).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `count` | `u32` | Number of copies including the original (`>= 1`; `0` is treated as<br>`1`). |
| `offset` | `[f64; 3]` | Per-axis relative offset. Copy `k` is translated by<br>`k * offset[axis] * bbox_size[axis]`, where `bbox_size` is the<br>input mesh's bounding-box extent along that axis. |

###### `Bevel`

Bevel every edge — forwards to [`crate::bevel::bevel`] (GH issue #37 §F,
`op-hzs.54.29`).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `options` | `crate::bevel::BevelOptions` | [`crate::bevel::BevelOptions`] for the bevel. |

###### `Boolean`

CSG boolean against `operand` — forwards to [`crate::boolean::boolean`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `operand` | `std::sync::Arc<crate::mesh::Mesh>` | The cutter / combiner mesh (shared, never mutated). |
| `op` | `crate::ops::BooleanMode` | Union / difference / intersect. |

###### `Solidify`

Thicken the surface into a shell — forwards to
[`crate::solidify::solidify`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `thickness` | `f64` | Shell thickness (model units). |

###### `Weld`

Merge vertices within `distance` — forwards to [`crate::weld::weld`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `distance` | `f64` | Merge distance. |

###### `Wireframe`

Replace the surface with a wireframe of beams along its edges.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `thickness` | `f64` | Beam thickness as a fraction of the local face size. |

###### `EdgeSplit`

Split the mesh along edges sharper than `angle` radians — forwards to
[`crate::edge_tools::edge_split`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `angle` | `f64` | Dihedral angle above which an edge is split. |

###### `Triangulate`

Fan-triangulate every face — forwards to
[`crate::triangulate::triangulate`].

###### `Mask`

Keep only faces all of whose vertices are in `keep`; `invert` keeps the
complement instead.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `keep` | `std::sync::Arc<Vec<crate::mesh::VertexId>>` | Vertices whose fully-covered faces survive. |
| `invert` | `bool` | Keep the complement. |

###### `Remesh`

Voxel remesh — rasterise and re-surface at `voxel_size`, then
`smooth_iterations` Laplacian passes (GH issue #37 §F, `op-hzs.54.30`).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `voxel_size` | `f64` | Voxel edge length. |
| `smooth_iterations` | `u32` | Smoothing passes (`0` = blocky). |

###### `Screw`

Screw: revolve the whole mesh's profile around `axis`, `turns` turns,
`steps` steps, advancing `screw_offset` along the axis (a helix).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `axis` | `crate::selection::Axis` | Rotation axis. |
| `turns` | `f64` | Number of full revolutions. |
| `steps` | `usize` | Steps per revolution. |
| `screw_offset` | `f64` | Total axial advance. |

###### `Skin`

Skin: a rectangular tube of half-width `radius` along every edge.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `radius` | `f64` | Tube half-width. |

###### `Build`

Build: reveal only the first `factor` fraction of the faces
(`0.0` … `1.0`).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `factor` | `f64` | Fraction of faces to keep. |

###### `Multires`

Multiresolution: `levels` of Catmull-Clark subdivision (the "simple"
flavour; forwards to [`crate::subdivision::catmull_clark`]).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `levels` | `u32` | Subdivision levels. |

###### `Decimate`

Decimate: reduce to `ratio` of the current face count (QEM collapse).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `ratio` | `f64` | Target face fraction (`0.0` … `1.0`). |

###### `SimpleDeform`

Simple Deform (twist / bend / taper / stretch) along an axis (GH issue
#37 §F, `op-hzs.54.31`).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `mode` | `crate::deform::SimpleDeform` | The deform mode + amount. |
| `axis` | `crate::selection::Axis` | Axis the deform is parameterised along. |

###### `Cast`

Cast toward a sphere / cylinder / cuboid.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `target` | `crate::deform::CastTarget` | The shape to cast toward. |
| `factor` | `f64` | `0` = unchanged, `1` = fully on the target. |

###### `Displace`

Displace along `direction` by value noise (a texture-free stand-in).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `direction` | `crate::math::Vec3` | Displacement direction. |
| `strength` | `f64` | Displacement amplitude. |
| `noise_scale` | `f64` | Noise frequency. |
| `seed` | `u64` | Noise seed. |

###### `Wave`

Wave: a travelling sine ripple along `axis`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `axis` | `crate::selection::Axis` | Ripple axis. |
| `amplitude` | `f64` | Peak displacement. |
| `wavelength` | `f64` | Wave period. |
| `speed` | `f64` | Travel speed. |
| `time` | `f64` | Evaluation time. |

##### Implementations

###### Methods

- ```rust
  pub fn evaluate(self: &Self, input: &Mesh) -> Result<Mesh, ModifierError> { /* ... */ }
  ```
  Evaluate this modifier against `input`, returning the derived mesh.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Modifier { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `ModifierStack`

An ordered, non-destructive stack of [`Modifier`]s applied to a base mesh.

Mirrors Blender's per-object modifier stack: the base mesh is kept, and
[`ModifierStack::evaluate`] folds each modifier in order to produce the
final derived mesh.

```rust
pub struct ModifierStack {
    pub modifiers: Vec<ModifierEntry>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `modifiers` | `Vec<ModifierEntry>` | The modifier entries, evaluated first-to-last (top-to-bottom in<br>Blender's UI). |

##### Implementations

###### Methods

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```
  Create an empty stack (evaluates to the input mesh unchanged).

- ```rust
  pub fn push(self: Self, m: Modifier) -> Self { /* ... */ }
  ```
  Append a modifier to the end of the stack (builder style), all toggles

- ```rust
  pub fn push_entry(self: Self, e: ModifierEntry) -> Self { /* ... */ }
  ```
  Append a fully-specified entry (builder style).

- ```rust
  pub fn move_up(self: &mut Self, i: usize) { /* ... */ }
  ```
  Move entry `i` one place earlier in the stack.

- ```rust
  pub fn move_down(self: &mut Self, i: usize) { /* ... */ }
  ```
  Move entry `i` one place later in the stack.

- ```rust
  pub fn remove(self: &mut Self, i: usize) { /* ... */ }
  ```
  Remove entry `i`.

- ```rust
  pub fn duplicate(self: &mut Self, i: usize) { /* ... */ }
  ```
  Copy entry `i` (append a clone to the end) — Blender's *Copy to

- ```rust
  pub fn evaluate(self: &Self, base: &Mesh) -> Result<Mesh, ModifierError> { /* ... */ }
  ```
  Evaluate the whole stack against `base`, skipping entries whose

- ```rust
  pub fn evaluate_render(self: &Self, base: &Mesh) -> Result<Mesh, ModifierError> { /* ... */ }
  ```
  Evaluate using the `show_render` toggle instead of `show_viewport`.

- ```rust
  pub fn apply_first(self: &mut Self, base: &Mesh) -> Result<Mesh, ModifierError> { /* ... */ }
  ```
  **Apply** the first entry: bake it into `base` and drop it from the

- ```rust
  pub fn apply_all(self: &mut Self, base: &Mesh) -> Result<Mesh, ModifierError> { /* ... */ }
  ```
  **Apply all**: bake the whole (viewport-enabled) stack into `base` and

- ```rust
  pub fn apply_as_shape_key(self: &Self, base: &Mesh) -> Result<Vec<Vec3>, ModifierError> { /* ... */ }
  ```
  **Apply as Shape Key**: the deformed vertex positions, valid only if the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ModifierStack { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> ModifierStack { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `ModifierEntry`

One modifier plus its per-entry visibility toggles (Blender's row of icons).

```rust
pub struct ModifierEntry {
    pub modifier: Modifier,
    pub show_viewport: bool,
    pub show_render: bool,
    pub show_in_editmode: bool,
    pub on_cage: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `modifier` | `Modifier` | The modifier. |
| `show_viewport` | `bool` | Evaluate this entry in the viewport result (Blender's monitor icon).<br>A disabled entry is skipped by [`ModifierStack::evaluate`]. |
| `show_render` | `bool` | Evaluate this entry in the render result. |
| `show_in_editmode` | `bool` | Show the modified geometry while in edit mode. |
| `on_cage` | `bool` | Edit the *modified* geometry directly (the "on cage" toggle). |

##### Implementations

###### Methods

- ```rust
  pub fn new(modifier: Modifier) -> Self { /* ... */ }
  ```
  An entry with every toggle on (the Blender default for a new modifier).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> ModifierEntry { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `normals`

**Normals toolset** (`op-hzs.54.27`, GH issue #37 §E).

- [`flip_faces`] — reverse the winding of selected faces.
- [`recalculate`] — [`crate::recalc_normals`] plus an *inside* option.
- [`vertex_normals`] — per-vertex normals by [`NormalWeight`]
  (uniform / face-area / corner-angle).
- [`point_normals_to_target`] — per-vertex normals aimed toward / away from
  a point.
- [`SplitNormals`] — a per-face-corner normal layer;
  [`split_normals_by_angle`] auto-smooths below an angle,
  [`harden_normals`] makes the selected faces contribute flat.

```rust
pub mod normals { /* ... */ }
```

### Types

#### Enum `NormalWeight`

How incident face normals are weighted into a vertex normal.

```rust
pub enum NormalWeight {
    Uniform,
    FaceArea,
    CornerAngle,
}
```

##### Variants

###### `Uniform`

Every incident face counts equally.

###### `FaceArea`

Weight by face area (Blender's "Face Area").

###### `CornerAngle`

Weight by the face's interior angle at that vertex (Blender's "Corner
Angle" — the default).

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> NormalWeight { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &NormalWeight) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `SplitNormals`

A per-face-corner normal layer (Blender's custom split normals). `normals[f]`
has one entry per corner of face `f`, in its vertex order.

```rust
pub struct SplitNormals {
    pub normals: Vec<Vec<crate::math::Vec3>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `normals` | `Vec<Vec<crate::math::Vec3>>` |  |

##### Implementations

###### Methods

- ```rust
  pub fn flat(mesh: &Mesh) -> Self { /* ... */ }
  ```
  Every corner normal equal to its face's flat normal.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SplitNormals { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> SplitNormals { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `flip_faces`

Reverse the winding (hence normal) of the faces in `faces` (empty = all).

```rust
pub fn flip_faces(mesh: &crate::mesh::Mesh, faces: &[crate::mesh::FaceId]) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `recalculate`

Make the whole mesh's winding consistent and point it `outside` (or inside).

```rust
pub fn recalculate(mesh: &crate::mesh::Mesh, outside: bool) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `vertex_normals`

Per-vertex normals for `mesh`, one entry per [`VertexId`], weighted per
`weight`. A vertex on no face gets [`Vec3::ZERO`].

```rust
pub fn vertex_normals(mesh: &crate::mesh::Mesh, weight: NormalWeight) -> Vec<crate::math::Vec3> { /* ... */ }
```

#### Function `point_normals_to_target`

Per-vertex normals aimed at `target` (`invert` flips them to aim away).
Blender's *Point to Target*.

```rust
pub fn point_normals_to_target(mesh: &crate::mesh::Mesh, target: crate::math::Vec3, invert: bool) -> Vec<crate::math::Vec3> { /* ... */ }
```

#### Function `split_normals_by_angle`

Auto-smooth split normals: a corner's normal is the average of the incident
face normals whose angle to this face's normal is `<= angle` radians; a
steeper neighbour is excluded (a hard edge). `angle = 0` gives flat shading,
`angle = π` gives fully smooth.

```rust
pub fn split_normals_by_angle(mesh: &crate::mesh::Mesh, angle: f64) -> SplitNormals { /* ... */ }
```

#### Function `harden_normals`

Harden normals: start from [`split_normals_by_angle`], then force every
corner of a face in `faces` to that face's flat normal (so the selection
reads as a crisp, faceted region regardless of its neighbours). Blender's
*Harden Normals*.

```rust
pub fn harden_normals(mesh: &crate::mesh::Mesh, faces: &[crate::mesh::FaceId], angle: f64) -> SplitNormals { /* ... */ }
```

## Module `nurbs_surface`

**NURBS surfaces** (`op-hzs.54.37`, GH issue #37 §G).

[`NurbsSurface`] is a tensor-product rational B-spline patch: an
`nu × nv` grid of control points with weights and a clamped uniform knot
vector on each axis. [`NurbsSurface::evaluate`] gives a point, and
[`NurbsSurface::to_mesh`] tessellates it into a quad grid.

Primitives ([`NurbsSurface::plane`] / [`sphere`](NurbsSurface::sphere) /
[`cylinder`](NurbsSurface::cylinder) / [`torus`](NurbsSurface::torus))
match Blender's Add-Surface menu. Patch editing is
[`NurbsSurface::move_control`] and [`NurbsSurface::control_mut`].

```rust
pub mod nurbs_surface { /* ... */ }
```

### Types

#### Struct `NurbsSurface`

A tensor-product NURBS surface patch.

```rust
pub struct NurbsSurface {
    pub nu: usize,
    pub nv: usize,
    pub order_u: usize,
    pub order_v: usize,
    pub control: Vec<crate::math::Vec3>,
    pub weights: Vec<f64>,
    pub cyclic_u: bool,
    pub cyclic_v: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nu` | `usize` | Control-point counts along `u` and `v`. |
| `nv` | `usize` |  |
| `order_u` | `usize` | Orders (degree + 1) along `u` and `v` (`>= 2`). |
| `order_v` | `usize` |  |
| `control` | `Vec<crate::math::Vec3>` | `control[u + nu * v]` — the control-point positions. |
| `weights` | `Vec<f64>` | Matching weights (`1.0` = a plain B-spline point). |
| `cyclic_u` | `bool` | Wrap the surface along `u` / `v` (a cylinder wraps in one, a torus in<br>both). |
| `cyclic_v` | `bool` |  |

##### Implementations

###### Methods

- ```rust
  pub fn control_mut(self: &mut Self, u: usize, v: usize) -> &mut Vec3 { /* ... */ }
  ```
  Mutable access to control point `(u, v)`.

- ```rust
  pub fn move_control(self: &mut Self, u: usize, v: usize, delta: Vec3) { /* ... */ }
  ```
  Translate control point `(u, v)` by `delta` (patch editing).

- ```rust
  pub fn evaluate(self: &Self, u: f64, v: f64) -> Vec3 { /* ... */ }
  ```
  Evaluate the surface at parameters `(u, v)`, each in `[0, 1]`.

- ```rust
  pub fn to_mesh(self: &Self, res_u: usize, res_v: usize) -> Mesh { /* ... */ }
  ```
  Tessellate the surface into a `res_u × res_v` quad grid.

- ```rust
  pub fn plane(nu: usize, nv: usize) -> Self { /* ... */ }
  ```
  A flat `nu × nv` control grid spanning `[-1, 1]²` in the `z = 0` plane.

- ```rust
  pub fn sphere(radius: f64) -> Self { /* ... */ }
  ```
  A NURBS sphere of `radius` — a dense `nu × nv` control grid sampled on

- ```rust
  pub fn cylinder(radius: f64, height: f64) -> Self { /* ... */ }
  ```
  A NURBS cylinder of `radius` and `height` — cyclic in `u`.

- ```rust
  pub fn torus(r: f64, t: f64) -> Self { /* ... */ }
  ```
  A NURBS torus of major radius `r` and minor radius `t` — cyclic in both.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> NurbsSurface { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `object_ops`

**Object-mode CAD operators** (`op-hzs.54.47`, GH issue #37 §J).

A [`SceneObject`] is a shared mesh in *local* space plus an [`Affine3`]
placing it in the world. The operators here work on objects and lists of
them:

- [`SceneObject::duplicate`] / [`SceneObject::linked_duplicate`] — a full
  copy vs. a copy that shares the same `Arc<Mesh>`.
- [`join`] — bake several objects' world geometry into one mesh.
- [`separate_loose_parts`] — split an object's mesh into its connected
  components, each a new object.
- [`SceneObject::apply_transform`] — bake the transform into the mesh and
  reset it to the identity.
- [`SceneObject::set_origin`] — move the object's local origin
  ([`OriginMode`]) without moving the geometry in the world.
- [`align`] — line objects' bounding boxes up along an axis.
- [`snap_objects`] / [`cursor_to_objects`] — the Snap menu.

## Units

Positions/lengths are dimensionless model-space quantities (see
[`crate::math`]).

```rust
pub mod object_ops { /* ... */ }
```

### Types

#### Struct `SceneObject`

A placed mesh: geometry in local space, [`Affine3`] to world space.

```rust
pub struct SceneObject {
    pub mesh: std::sync::Arc<crate::mesh::Mesh>,
    pub transform: crate::transform::Affine3,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `std::sync::Arc<crate::mesh::Mesh>` | Geometry in the object's local frame (shared, never mutated in place). |
| `transform` | `crate::transform::Affine3` | Local → world transform. |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: Mesh) -> Self { /* ... */ }
  ```
  A new object at the identity transform.

- ```rust
  pub fn world_mesh(self: &Self) -> Mesh { /* ... */ }
  ```
  This object's geometry in world space.

- ```rust
  pub fn world_origin(self: &Self) -> Vec3 { /* ... */ }
  ```
  The object's origin in world space (its transform's translation).

- ```rust
  pub fn duplicate(self: &Self) -> SceneObject { /* ... */ }
  ```
  A full, independent copy (new mesh storage).

- ```rust
  pub fn linked_duplicate(self: &Self) -> SceneObject { /* ... */ }
  ```
  A copy that **shares** the same mesh data (Blender's Linked Duplicate);

- ```rust
  pub fn shares_mesh_with(self: &Self, other: &SceneObject) -> bool { /* ... */ }
  ```
  Whether this object shares its mesh with `other`.

- ```rust
  pub fn apply_transform(self: &Self) -> SceneObject { /* ... */ }
  ```
  Bake the transform into the geometry and reset the transform to the

- ```rust
  pub fn set_origin(self: &Self, mode: OriginMode) -> SceneObject { /* ... */ }
  ```
  Move the object's local origin per `mode`, keeping every vertex in the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SceneObject { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `OriginMode`

Where [`SceneObject::set_origin`] puts the origin.

```rust
pub enum OriginMode {
    GeometryMedian,
    CenterOfMassSurface,
    CenterOfMassVolume,
    Cursor {
        world: crate::math::Vec3,
    },
}
```

##### Variants

###### `GeometryMedian`

The mean of the mesh vertices (Origin to Geometry).

###### `CenterOfMassSurface`

Area-weighted mean face centroid (Center of Mass — Surface).

###### `CenterOfMassVolume`

Volume centroid via signed tetrahedra (Center of Mass — Volume).

###### `Cursor`

A given world point (Origin to 3D Cursor).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `world` | `crate::math::Vec3` | The cursor position in world space. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> OriginMode { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &OriginMode) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `AlignMode`

How [`align`] lines objects up on the chosen axis.

```rust
pub enum AlignMode {
    Min,
    Center,
    Max,
}
```

##### Variants

###### `Min`

Match the minimum (negative) side of each bounding box.

###### `Center`

Match the bounding-box centres.

###### `Max`

Match the maximum (positive) side.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> AlignMode { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AlignMode) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `SnapObjectsTo`

Target for [`snap_objects`].

```rust
pub enum SnapObjectsTo {
    Cursor {
        world: crate::math::Vec3,
    },
    Grid {
        step: f64,
    },
    Active {
        index: usize,
    },
}
```

##### Variants

###### `Cursor`

Move each object's origin to `world` (Selection to Cursor).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `world` | `crate::math::Vec3` | The cursor position. |

###### `Grid`

Round each object's origin to a multiple of `step` (Selection to Grid).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `step` | `f64` | The grid step. |

###### `Active`

Move every object's origin onto object `index`'s origin (Selection to
Active).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `index` | `usize` | Index of the active object in the slice. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SnapObjectsTo { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SnapObjectsTo) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `join`

Bake `objects`' world geometry into a single identity-transform object
(Join). The first object's transform is discarded along with the rest —
use [`SceneObject::apply_transform`] on the result if you want it re-based.

```rust
pub fn join(objects: &[SceneObject]) -> SceneObject { /* ... */ }
```

#### Function `separate_loose_parts`

Split `object`'s mesh into connected components (by shared vertices), each
returned as a new object with the **same** transform as the original
(Separate → By Loose Parts).

```rust
pub fn separate_loose_parts(object: &SceneObject) -> Vec<SceneObject> { /* ... */ }
```

#### Function `align`

Translate each object along `axis` so its world bounding box lines up per
`mode` with the group's average reference coordinate. Returns new objects
(translation-only change).

```rust
pub fn align(objects: &[SceneObject], axis: crate::selection::Axis, mode: AlignMode) -> Vec<SceneObject> { /* ... */ }
```

#### Function `snap_objects`

Apply a Snap-menu target to every object (translation only).

```rust
pub fn snap_objects(objects: &[SceneObject], to: SnapObjectsTo) -> Vec<SceneObject> { /* ... */ }
```

#### Function `cursor_to_objects`

The median of the objects' world origins (Cursor to Selected).

```rust
pub fn cursor_to_objects(objects: &[SceneObject]) -> crate::math::Vec3 { /* ... */ }
```

## Module `ops`

Mesh **operators** — the editing verbs (Blender's `bmesh/operators`, `bmo_*`).

Each operator is a **pure function over the polygon-soup view** of a mesh:
it reads [`Mesh::positions`] + [`Mesh::polygons`], builds a fresh
`positions: Vec<Vec3>` and `faces: Vec<Vec<usize>>`, and rebuilds through
[`Mesh::from_polygons`]. Because the rebuild goes back through
[`Mesh::add_face`], edge deduplication and loop wiring are recomputed for
free, so an operator never touches the half-edge cycles by hand.

## Why an enum, not trait objects

The operator set is closed and known at compile time, so per the workspace
design rules it is a single [`MeshOp`] enum dispatched by `match` — not
`Box<dyn Trait>`. Adding a new operator forces every `match` site to handle
it (exhaustiveness), and rust-analyzer's go-to-definition works on each
variant.

## The operators (Blender analogue in parentheses)

- [`extrude_faces`] / [`MeshOp::Extrude`] — duplicate a face region, offset
  the cap, and wall the boundary edges (`bmo_extrude`, the Extrude Region
  tool).
- [`extrude_edges`] — build a quad off each selected edge and its offset
  copy (the Extrude Edges tool).
- [`subdivide`] / [`MeshOp::Subdivide`] — **simple midpoint** subdivision
  (topological quad split, no smoothing; `bmo_subdivide` with smoothness 0).
  This is distinct from Catmull-Clark, which lives in
  [`crate::subdivision::catmull_clark`] and the non-destructive
  [`crate::modifiers::Modifier::Subsurf`].
- [`bevel_vertices`] / [`bevel_vertices_rounded`] / [`MeshOp::Bevel`] —
  vertex bevel / truncation (`bmo_bevel` in vertex-only mode): a single flat
  chamfer, or a rounded spherical cap for `segments >= 2`.
- [`MeshOp::Boolean`] — CSG union/difference/intersection of two meshes,
  delegated to [`crate::boolean`] (`bmo_boolean`, backed upstream by the
  Manifold library). This is the operator most relevant to feeding
  `outram-mc-libs` CSG geometry.

```rust
pub mod ops { /* ... */ }
```

### Types

#### Enum `MeshOpError`

Errors returned by [`MeshOp::apply`].

```rust
pub enum MeshOpError {
    NotImplemented(&'static str),
    Boolean(crate::boolean::BooleanError),
    Laplacian(crate::laplacian::LaplacianError),
    Arap(crate::arap::ArapError),
    Hull(crate::convex_hull::HullError),
}
```

##### Variants

###### `NotImplemented`

An operator is declared but its algorithm is not implemented yet.

Retained for forward compatibility (a new [`MeshOp`] variant may land as
a stub); **every current variant is implemented**, so [`MeshOp::apply`]
never returns this today.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `&'static str` |  |

###### `Boolean`

Propagated from a mesh boolean (crate::boolean) — the operand meshes are
outside the supported restricted case.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::boolean::BooleanError` |  |

###### `Laplacian`

Propagated from Laplacian smoothing (crate::laplacian) — the sparse solve
failed (e.g. a non-positive-definite system).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::laplacian::LaplacianError` |  |

###### `Arap`

Propagated from ARAP deformation (crate::arap) — missing constraints or a
non-positive-definite system.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::arap::ArapError` |  |

###### `Hull`

Propagated from convex-hull construction (crate::convex_hull) — a
degenerate (fewer than four distinct / collinear / coplanar) point set.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::convex_hull::HullError` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Error**
  - ```rust
    fn source(self: &Self) -> ::core::option::Option<&dyn ::thiserror::__private18::Error + ''static> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(source: crate::boolean::BooleanError) -> Self { /* ... */ }
    ```

  - ```rust
    fn from(source: crate::laplacian::LaplacianError) -> Self { /* ... */ }
    ```

  - ```rust
    fn from(source: crate::arap::ArapError) -> Self { /* ... */ }
    ```

  - ```rust
    fn from(source: crate::convex_hull::HullError) -> Self { /* ... */ }
    ```

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `BooleanMode`

Boolean CSG mode for [`MeshOp::Boolean`] (mirrors Blender's Boolean modifier).

```rust
pub enum BooleanMode {
    Union,
    Difference,
    Intersect,
}
```

##### Variants

###### `Union`

Keep the volume in either mesh (A ∪ B).

###### `Difference`

Keep the volume of A outside B (A \ B).

###### `Intersect`

Keep the volume common to both (A ∩ B).

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> BooleanMode { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &BooleanMode) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `MeshOp`

A closed set of mesh-editing operators.

Construct a variant with its parameters, then call [`MeshOp::apply`] on a
mesh. Parameters are captured by value (no borrows) so the enum carries no
lifetimes.

```rust
pub enum MeshOp {
    Extrude {
        offset: crate::math::Vec3,
    },
    Subdivide {
        iterations: u32,
    },
    Bevel {
        width: f64,
        segments: u32,
    },
    Boolean {
        other: crate::mesh::Mesh,
        mode: BooleanMode,
    },
    Smooth {
        weighting: crate::laplacian::LaplacianWeighting,
        lambda: f64,
        iterations: u32,
    },
    Taubin {
        weighting: crate::laplacian::LaplacianWeighting,
        lambda: f64,
        mu: f64,
        iterations: u32,
    },
    Arap {
        handles: Vec<(crate::mesh::VertexId, crate::math::Vec3)>,
        iterations: u32,
    },
    Decimate {
        target_faces: usize,
    },
    LoopSubdivide {
        iterations: u32,
    },
    ConvexHull,
    Weld {
        distance: f64,
    },
    FillHoles,
    Solidify {
        thickness: f64,
    },
    RecalculateNormals,
    Triangulate,
    Inset {
        amount: f64,
    },
    Bisect {
        point: crate::math::Vec3,
        normal: crate::math::Vec3,
    },
    BevelEdges {
        width: f64,
    },
}
```

##### Variants

###### `Extrude`

Extrude **the whole mesh's faces** along `offset` (direction and distance
combined). [`MeshOp::apply`] runs [`extrude_faces`] over every face; use
[`extrude_faces`] directly to extrude a chosen face region.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `offset` | `crate::math::Vec3` | Model-space translation applied to the newly created cap geometry. |

###### `Subdivide`

Apply `iterations` rounds of **simple midpoint** subdivision (topological
quad split, no smoothing — existing vertices stay put).

This is *not* Catmull-Clark: for a smoothed subdivision surface use the
non-destructive [`crate::modifiers::Modifier::Subsurf`] or the direct
[`crate::subdivision::catmull_clark`]. See [`subdivide`] for the rules.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `iterations` | `u32` | Number of refinement passes (each roughly quadruples face count);<br>`0` is a no-op clone. |

###### `Bevel`

Bevel (truncate) every vertex by `width` model-space units.

Dispatches to [`bevel_vertices_rounded`]: `segments <= 1` is the single
flat chamfer ([`bevel_vertices`], the polyhedral truncation);
`segments >= 2` rounds each cut corner into a spherical cap with
`segments - 1` intermediate rings.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `width` | `f64` | Bevel width in model-space units (clamped per-edge to half the edge<br>length). |
| `segments` | `u32` | Number of segments across the bevel: `1` = a single flat chamfer,<br>`>= 2` = a rounded spherical cap with `segments - 1` intermediate<br>rings (see [`bevel_vertices_rounded`]). |

###### `Boolean`

Combine the target mesh with `other` under a [`BooleanMode`], delegated to
[`crate::boolean::boolean`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `other` | `crate::mesh::Mesh` | The second operand mesh (owned by value — no lifetimes). |
| `mode` | `BooleanMode` | Union / difference / intersection. |

###### `Smooth`

**Implicit Laplacian smoothing** (mesh fairing), delegated to
[`crate::laplacian::laplacian_smooth`]. Solves `(I + λL) x' = x` per
iteration with boundary vertices pinned; unconditionally stable.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `weighting` | `crate::laplacian::LaplacianWeighting` | Uniform (umbrella) or cotangent (Laplace–Beltrami) weighting. |
| `lambda` | `f64` | Smoothing strength per step (`>= 0`; `0` is a no-op). |
| `iterations` | `u32` | Number of implicit steps (`0` is a no-op). |

###### `Taubin`

**Taubin `λ|μ` smoothing** (explicit, shrinkage-free denoising), delegated
to [`crate::laplacian::taubin_smooth`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `weighting` | `crate::laplacian::LaplacianWeighting` | Uniform or cotangent weighting. |
| `lambda` | `f64` | Shrinking factor `0 < λ < 1`. |
| `mu` | `f64` | Un-shrinking factor `−1 < μ < −λ`. |
| `iterations` | `u32` | Number of `λ|μ` iteration pairs. |

###### `Arap`

**As-Rigid-As-Possible deformation**, delegated to
[`crate::arap::arap_deform`]. Deforms the mesh to meet the `handles`
(vertex → target) while keeping one-rings maximally rigid.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `handles` | `Vec<(crate::mesh::VertexId, crate::math::Vec3)>` | Handle constraints: each `(vertex, target position)`. |
| `iterations` | `u32` | Number of local/global ARAP iterations. |

###### `Decimate`

**QEM mesh decimation** (quadric-error-metric simplification), delegated
to [`crate::decimate::decimate`]. Reduces the mesh to roughly
`target_faces` triangles.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `target_faces` | `usize` | The goal triangle count (a lower-bound target). |

###### `LoopSubdivide`

**Loop subdivision** (smooth triangle subdivision surface), delegated to
[`crate::loop_subdivision::loop_subdivide`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `iterations` | `u32` | Number of refinement steps (each quadruples the triangle count). |

###### `ConvexHull`

Replace the mesh with the **convex hull of its vertices**, delegated to
[`crate::convex_hull::convex_hull`].

###### `Weld`

**Weld / remove-doubles**: merge vertices closer than `distance` into
one, delegated to [`crate::weld::weld`]. `distance = 0` welds only
bit-identical duplicates (a safe no-op otherwise).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `distance` | `f64` | Euclidean merge tolerance in mesh units (`0` = exact duplicates only). |

###### `FillHoles`

**Fill holes**: cap every open boundary loop with a centroid triangle
fan, delegated to [`crate::fill_holes::fill_holes`]. A no-op on an
already-closed mesh.

###### `Solidify`

**Solidify**: extrude the surface into a closed shell of the given
`thickness`, delegated to [`crate::solidify::solidify`]. An open surface
becomes a slab; a closed surface becomes a hollow double shell.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `thickness` | `f64` | Shell thickness in mesh units; the inner shell is offset inward. |

###### `RecalculateNormals`

**Recalculate normals outside**: make the winding globally consistent and
outward-facing, delegated to
[`crate::recalc_normals::recalculate_normals`]. Repairs an
inconsistently-wound polygon soup.

###### `Triangulate`

**Triangulate**: fan-triangulate every face into triangles, delegated to
[`crate::triangulate::triangulate`]. Produces a triangle-only mesh for
the operators/bridges that require one.

###### `Inset`

**Inset faces**: replace each face with a shrunk inner copy plus a ring
of bridging quads, delegated to [`crate::inset::inset_faces`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `amount` | `f64` | Fraction each corner moves toward its face centroid, in `(0, 1)`. |

###### `Bisect`

**Bisect**: cut the mesh by a plane and keep the `normal`-negative half,
delegated to [`crate::bisect::bisect`]. The cut is left open (cap it with
[`MeshOp::FillHoles`]).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `point` | `crate::math::Vec3` | A point the cutting plane passes through. |
| `normal` | `crate::math::Vec3` | The plane normal; the kept half is where `normal · (x − point) <= 0`. |

###### `BevelEdges`

**Edge bevel**: chamfer every edge by `width`, delegated to
[`crate::edge_bevel::bevel_edges`]. Distinct from [`MeshOp::Bevel`], which
truncates *vertices*.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `width` | `f64` | Distance each face is cut back from its edges (mesh units). |

##### Implementations

###### Methods

- ```rust
  pub fn apply(self: &Self, mesh: Mesh) -> Result<Mesh, MeshOpError> { /* ... */ }
  ```
  Apply this operator to `mesh`, returning the edited mesh.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> MeshOp { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `extrude_faces`

Extrude a set of faces: duplicate their vertices, offset the duplicates, cap
the region with the moved copies, and wall the boundary edges.

# What it computes

For the selected region (the faces named in `faces`, given as [`FaceId`]s):

1. **Cap.** Every vertex used by a selected face is duplicated and the copy
   is translated by `offset`. The selected faces are removed and replaced by
   "cap" faces on the moved copies, preserving winding.
2. **Side walls.** Each *boundary edge* of the region — an edge used by
   exactly one selected face — gets a side quad `[a, b, b', a']` connecting
   the old edge `a→b` (in that face's winding) to its moved copy `a'→b'`.
   The winding is chosen so the wall's outward normal points away from the
   region interior. Interior edges (shared by two selected faces) get no
   wall.
3. **Rest of the mesh.** Vertices and faces not in the selection are kept
   unchanged.

A single-quad selection therefore becomes a "cup": 4 side walls + 1 cap,
with the original quad's footprint left open.

# Inputs / units

- `mesh` — the source mesh (borrowed, unmodified).
- `faces` — the [`FaceId`]s to extrude; out-of-range ids are ignored.
- `offset` — model-space translation applied to the cap (dimensionless
  model-space units; direction and distance combined).

# Note

Extruding a *closed* region (every edge interior, e.g. all faces of a closed
solid) produces no walls and leaves the original vertices unreferenced by
the new topology — a whole-mesh extrude of a closed solid is a degenerate
case, not the intended use. Extrude an open region (a grid, a face patch).

```rust
pub fn extrude_faces(mesh: &crate::mesh::Mesh, faces: &[crate::mesh::FaceId], offset: crate::math::Vec3) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `extrude_edges`

Extrude a set of edges: build one quad off each edge and its offset copy.

For each [`EdgeId`] in `edges`, the two endpoints are duplicated (shared
across edges that touch the same vertex) and translated by `offset`, then a
quad `[a, b, b', a']` is added spanning the original edge `a–b` and its moved
copy `a'–b'`. The original faces are kept; the new quads are appended. This
is the secondary, deliberately simple edge-extrude — it does not attempt to
reason about which side the wall should face.

# Inputs / units

- `mesh` — source mesh (borrowed, unmodified).
- `edges` — [`EdgeId`]s to extrude; out-of-range ids are ignored.
- `offset` — model-space translation of the copies (dimensionless model
  units).

```rust
pub fn extrude_edges(mesh: &crate::mesh::Mesh, edges: &[crate::mesh::EdgeId], offset: crate::math::Vec3) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `subdivide`

Simple midpoint subdivision — split every face into quads, no smoothing.

# What it computes

This is **topological** subdivision (Blender's Subdivide with smoothness 0),
distinct from Catmull-Clark ([`crate::subdivision::catmull_clark`] /
[`crate::modifiers::Modifier::Subsurf`]): new points are placed by plain
linear interpolation and existing vertices are **not** moved, so the surface
keeps its exact shape while gaining resolution.

Each iteration, for every `n`-gon face:

- add the midpoint of each of its edges (deduplicated across faces by a
  canonical key on the sorted endpoint-index pair, so a shared edge yields a
  single shared midpoint), and
- add the face centroid (the vertex mean),

then replace the face with `n` quads, one per corner `v[i]`, wound
`[centroid, midpoint(edge into v[i]), v[i], midpoint(edge out of v[i])]` —
which preserves the original winding.

# Counts per iteration

For a closed genus-0 mesh with `V, E, F`, one pass yields
`V' = V + E + F`, `F' = sum of face side-counts` (`4F` when all faces are
quads), and `chi = 2` is preserved. E.g. a cube (`V=8, E=12, F=6`) becomes
`V=26, F=24, chi=2` after one pass.

# Inputs

- `mesh` — source mesh (borrowed, unmodified).
- `iterations` — number of passes; `iterations == 0` returns a clone.

```rust
pub fn subdivide(mesh: &crate::mesh::Mesh, iterations: u32) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `bevel_vertices`

Vertex bevel / truncation — cut off every vertex, replacing it with a face.

# What it computes

This is the vertex-only bevel (a single chamfer per vertex, the polyhedral
*truncation* operation). For each vertex `V` and each incident edge `e`, one
**edge-point** is placed at
`V + w_e * normalize(other_end(e) - V)`, where `w_e = min(width, L_e / 2)`
clamps the offset so the two edge-points on an edge of length `L_e` never
pass its midpoint. That edge-point is shared by the two faces on `e`.

The result mesh contains **only** the edge-points (every original vertex is
cut away):

- **Truncated faces.** Each original `n`-gon becomes a `2n`-gon: every
  corner `V` is replaced, in winding order, by its two edge-points — the one
  on the edge entering `V` then the one on the edge leaving `V`.
- **Vertex faces.** Each original vertex contributes one new face joining its
  edge-points in angular order around `V` (the order is read off the fan of
  faces around `V`), wound so its normal points outward.

# Counts (verification)

Truncating a cube (`V=8, E=12, F=6`) gives the **truncated cube**:
`V=24` (two edge-points per edge), `E=36`, `F=14` (6 octagons + 8 triangles),
`chi=2`. This is asserted in the module tests.

# Inputs / units

- `mesh` — source mesh (borrowed, unmodified). Assumed manifold and
  orientable (as produced by [`crate::primitives`]); the angular ordering of
  a vertex face is derived from the face fan around each vertex.
- `width` — chamfer width in dimensionless model-space units, clamped
  per-edge to at most half the edge length. `width <= 0` collapses the
  edge-points onto the original vertices (a no-op-shaped degenerate result).

```rust
pub fn bevel_vertices(mesh: &crate::mesh::Mesh, width: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `bevel_vertices_rounded`

Rounded (multi-segment) vertex bevel — truncate every vertex and replace the
flat chamfer with a **spherical cap** of `segments` bands.

# What it computes

The truncation (edge-points, widened faces) is identical to
[`bevel_vertices`]; only the per-vertex cap differs. For `segments <= 1` this
*is* [`bevel_vertices`] (one flat face per vertex). For `segments >= 2`, each
vertex `V`'s edge-point ring is domed into a spherical cap on the sphere
centred at `V` of radius `~width`:

- the ring of edge-points is **band 0** (shared with the truncated faces, so
  the result stays watertight);
- `segments - 1` **intermediate rings** are inserted by spherical-linear
  interpolation (`slerp`) of each edge-point's direction toward the cap
  **apex** `V + R * n`, where `n` is the outward vertex normal (sum of
  incident face normals) and `R` the mean edge-point distance; the radius is
  interpolated linearly so band 0 stays exactly on the (possibly
  per-edge-clamped) edge-points;
- the cap closes with a triangle fan to the apex.

The corner therefore becomes a smooth convex spherical cap of radius `~width`
about the original vertex — a *rounded* bevel. This is a mesh-authoring
rounding (a spherical cap through the edge-points), **not** a CAD fillet
tangent to the adjacent faces; the apex bulges toward the original corner
along the outward normal. Higher `segments` gives a smoother cap.

# Counts (verification)

For a cube (`V=8, E=12, F=6`, every vertex degree 3) at `segments = s >= 2`:
`V = 24s + 8`, `F = 24s + 6` (6 octagons + `3s` cap faces per vertex),
`E = 48s + 12`, `chi = 2`. Asserted in the module tests.

# Inputs / units

- `mesh` — source mesh (borrowed, unmodified); assumed manifold/orientable.
- `width` — chamfer width (model-space units, clamped per-edge to half the
  edge length), same as [`bevel_vertices`].
- `segments` — bevel resolution: `0`/`1` = single flat chamfer, `>= 2` =
  rounded cap with `segments - 1` intermediate rings.

```rust
pub fn bevel_vertices_rounded(mesh: &crate::mesh::Mesh, width: f64, segments: u32) -> crate::mesh::Mesh { /* ... */ }
```

## Module `planar_faces`

Flatten faces by relaxing their vertices onto each face's own plane — a
port of Blender's **Make Planar Faces** (`bmo_planar_faces.cc`).

# What this computes, and how it differs from splitting

[`crate::connect_nonplanar`] makes a warped face planar by **cutting it**
— topology changes, positions do not. This operator does the opposite:
it **moves the vertices** until the faces are planar, leaving topology
alone. For a solver mesh both are useful and they are not
interchangeable: cutting preserves the surface exactly but multiplies
face count, while relaxing keeps the face count but perturbs the
geometry. Pick by which of the two you can afford to lose.

# The algorithm

One iteration, per upstream:

1. Each face defines a plane through its (area-weighted) centroid with
   its own normal. That centroid and normal are taken **once, from the
   input**, and reused — upstream's comment is "keep original face data
   (else we 'move' the face)". Recomputing them each sweep lets the face
   drift bodily through space instead of just flattening, which is why
   the recompute is `#if 0`-ed out upstream. That is reproduced here.
2. Every vertex collects the closest point on each incident face's plane
   and averages them — upstream accumulates with a running mean
   (`interp_v3_v3v3(va.co, va.co, co, 1 / co_tot)`), which is the same
   value as a plain mean and is kept in that form.
3. Each vertex moves a `factor` of the way toward its average target.
   Vertices that moved less than `1e-5` are considered settled; when no
   vertex moves, the sweep stops early.

Triangles are skipped throughout — they are planar already, and pulling
their corners about would only distort the mesh.

Inputs are positions in the caller's length unit; `factor` is
dimensionless in `[0, 1]` and `iterations` is a count. Nothing here
carries a physical dimension.

# Fidelity to upstream, and the deviations

The plane-per-face construction, the frozen centroid/normal, the running
mean, the `1e-5` settle threshold and the early-out are transcribed.
Differences:

1. **`f64`, not `f32`.** The `1e-5` threshold is kept at that value; it
   is a geometric tolerance in model units, not a mantissa bound.
2. **No per-face dirty flag.** Upstream tracks `ELE_FACE_ADJUST` so a
   sweep only revisits faces touching a vertex that moved. This port
   recomputes every non-triangle face each sweep. Same fixed point, more
   work per sweep; the flag is an optimisation, not a semantic. Worth
   revisiting if this is ever run on a large mesh.
3. **Returns a new [`Mesh`]** rather than mutating in place, like every
   other operator here.

```rust
pub mod planar_faces { /* ... */ }
```

### Functions

#### Function `planar_faces`

Relax the vertices of `mesh` so its faces become planar.

`factor` is how far toward the target each vertex moves per sweep
(upstream's "factor" slot, [`DEFAULT_FACTOR`] = 1.0 for the full step);
`iterations` bounds the number of sweeps ([`DEFAULT_ITERATIONS`] = 1).
Topology is untouched — only positions change — so the returned mesh has
the same vertex, edge and face counts. Infallible.

Triangles are left alone. A mesh of only triangles is returned unchanged.

# Examples

```
use outram_blender::{mesh::Mesh, math::Vec3};
use outram_blender::planar_faces::{planar_faces, DEFAULT_FACTOR};

let pts = vec![
    Vec3::new(0.0, 0.0, 0.0),
    Vec3::new(1.0, 0.0, 0.0),
    Vec3::new(1.0, 1.0, 0.5),
    Vec3::new(0.0, 1.0, 0.0),
];
let warped = Mesh::from_polygons(&pts, &[vec![0, 1, 2, 3]]);
let flat = planar_faces(&warped, DEFAULT_FACTOR, 20);
assert_eq!(flat.face_count(), warped.face_count());
assert_eq!(flat.vertex_count(), warped.vertex_count());
```

```rust
pub fn planar_faces(mesh: &crate::mesh::Mesh, factor: f64, iterations: usize) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `planar_faces_converge`

Flatten by calling [`planar_faces`] repeatedly until the mesh stops
moving — **this crate's addition, not an upstream operator.**

# Why this exists

[`planar_faces`] freezes each face's target plane from its input, which
is deliberate upstream (it stops faces drifting bodily through space)
but means its own `iterations` argument does not converge on geometry
where faces share vertices: measured on a corrugated 3x3 grid, peak warp
goes 0.150 -> 0.0666 after one sweep and then *back up* to 0.0674 by 200
sweeps. Re-invoking the operator recomputes the planes and does
converge. See the `iterations_within_one_call_do_not_converge_but_repeated_calls_do`
test for the full table.

So this is a loop around the upstream operator, not a different
algorithm. Each pass runs exactly one upstream sweep.

Stops when the largest vertex movement in a pass falls below
`tolerance` (in the caller's length unit), or after `max_passes`.
Topology is untouched. Infallible.

# It cannot flatten below upstream's absolute `eps`

`planar_faces` skips any vertex whose step is shorter than 1e-5 **model
units** (upstream's `eps`), so this loop plateaus there however many
passes it is given — measured 7.71e-5 peak warp on the corrugated grid,
unchanged from 200 passes to 5000. That floor is absolute, not relative,
so it scales with your model's units. Where exact planarity is required,
use [`crate::connect_nonplanar`], which cuts rather than moves.

# Examples

```
use outram_blender::{mesh::Mesh, math::Vec3};
use outram_blender::planar_faces::{planar_faces_converge, DEFAULT_FACTOR};

let pts = vec![
    Vec3::new(0.0, 0.0, 0.0),
    Vec3::new(1.0, 0.0, 0.0),
    Vec3::new(1.0, 1.0, 0.5),
    Vec3::new(0.0, 1.0, 0.0),
];
let warped = Mesh::from_polygons(&pts, &[vec![0, 1, 2, 3]]);
let flat = planar_faces_converge(&warped, DEFAULT_FACTOR, 1e-9, 100);
assert_eq!(flat.face_count(), 1);
```

```rust
pub fn planar_faces_converge(mesh: &crate::mesh::Mesh, factor: f64, tolerance: f64, max_passes: usize) -> crate::mesh::Mesh { /* ... */ }
```

### Constants and Statics

#### Constant `DEFAULT_FACTOR`

Upstream's default relaxation factor for **Make Planar Faces**.

```rust
pub const DEFAULT_FACTOR: f64 = 1.0;
```

#### Constant `DEFAULT_ITERATIONS`

Upstream's default iteration count.

```rust
pub const DEFAULT_ITERATIONS: usize = 1;
```

## Module `poke_quads`

**Poke Faces / Tris ↔ Quads** (`op-hzs.54.17`, GH issue #37 §B).

- [`poke_faces`] replaces each face with a fan of triangles from a new
  centre vertex, offset by `offset` along the face normal (Blender's
  `Face ▸ Poke Faces`).
- [`triangulate_quads`] triangulates each quad by the diagonal chosen per
  [`QuadMethod`]; n-gons are centroid-fanned (Blender's `Face ▸
  Triangulate` with a quad method). The plain fan is
  [`crate::triangulate::triangulate`].
- [`tris_to_quads`] greedily merges adjacent coplanar-ish triangle pairs
  into quads whose corner angles stay within `max_angle` of 90° (Blender's
  `Face ▸ Tris to Quads`). Attribute comparisons (material / UV / sharp /
  seam) arrive with the attribute layers in `op-hzs.54.28`.

```rust
pub mod poke_quads { /* ... */ }
```

### Types

#### Enum `QuadMethod`

Which diagonal [`triangulate_quads`] cuts a quad along.

```rust
pub enum QuadMethod {
    ShortestDiagonal,
    Fixed,
    FixedAlternate,
    Beauty,
}
```

##### Variants

###### `ShortestDiagonal`

The shorter of the two diagonals. Same rule as
[`crate::triangulate::QuadMethod::ShortEdge`].

###### `Fixed`

Always `v0–v2`. Same as [`crate::triangulate::QuadMethod::Fixed`].

###### `FixedAlternate`

Always `v1–v3`. Same as [`crate::triangulate::QuadMethod::Alternate`].

###### `Beauty`

The diagonal that gives the better-shaped triangle pair, decided by
Blender's own rule — see [`crate::triangulate::QuadMethod::Beauty`].

**Changed 2026-09-19.** This variant previously used a hand-rolled
"maximise the smallest of the four resulting angles" measure, written
before Blender's own rule was ported. It now delegates to the ported
rule (`is_quad_flip_v3`, then area-over-perimeter), so the crate has
one definition of "beauty" rather than two that drift. The two agree
on planar convex quads and differ on warped or concave ones, where
the upstream rule is the one that avoids folding the pair.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> QuadMethod { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(m: QuadMethod) -> Self { /* ... */ }
    ```

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &QuadMethod) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `PokeCenter`

Where the poke centre goes — upstream's `BMOP_POKE_*`
(`bmesh_operators.hh:93`).

Blender's **Poke Faces** tool defaults to [`PokeCenter::MedianWeighted`]
(`editmesh_tools.cc:5470`), which is why that is the default here too.

```rust
pub enum PokeCenter {
    MedianWeighted,
    Median,
    Bounds,
}
```

##### Variants

###### `MedianWeighted`

Each corner weighted by the length of the two edges meeting there,
so a cluster of closely-spaced corners does not drag the centre
toward itself. Upstream `BMOP_POKE_MEDIAN_WEIGHTED`, and upstream's
default.

###### `Median`

The plain mean of the corner positions. Upstream `BMOP_POKE_MEDIAN`.

###### `Bounds`

The centre of the face's axis-aligned bounding box. Upstream
`BMOP_POKE_BOUNDS`.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> PokeCenter { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> PokeCenter { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PokeCenter) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `poke_faces`

Poke every face into a fan around a new centre vertex, using upstream's
defaults.

The centre is placed by [`PokeCenter::MedianWeighted`] and displaced
along the face normal by `offset` (an absolute length, in the caller's
units). See [`poke_faces_with`] for the other modes.

**Changed 2026-09-19.** This previously used the plain vertex mean,
which is upstream's `BMOP_POKE_MEDIAN` — *not* its default. Blender's
Poke Faces tool defaults to median-weighted, so this now does too. The
two agree on any face whose corners are evenly spaced and differ on one
where they are not; see
`poke_center_modes_differ_on_unevenly_spaced_corners`.

```rust
pub fn poke_faces(mesh: &crate::mesh::Mesh, offset: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `poke_faces_with`

Poke every face into a fan, choosing the centre mode and offset scaling.

When `use_relative_offset` is set, `offset` is multiplied by the mean
distance from the face centre to its corners, so the displacement scales
with the face rather than being absolute — upstream's
`use_relative_offset` slot, which defaults to off.

Positions of existing vertices are never changed; one vertex is added
per face. Infallible.

# Examples

```
use outram_blender::primitives;
use outram_blender::poke_quads::{poke_faces_with, PokeCenter};

let cube = primitives::cube(2.0);
// Six quads become six fans of four triangles.
let poked = poke_faces_with(&cube, 0.0, PokeCenter::MedianWeighted, false);
assert_eq!(poked.face_count(), 24);
assert_eq!(poked.vertex_count(), cube.vertex_count() + 6);
```

```rust
pub fn poke_faces_with(mesh: &crate::mesh::Mesh, offset: f64, center_mode: PokeCenter, use_relative_offset: bool) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `triangulate_quads`

Triangulate every quad by `method`; n-gons are centroid-fanned; triangles
are kept.

```rust
pub fn triangulate_quads(mesh: &crate::mesh::Mesh, method: QuadMethod) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `tris_to_quads`

Greedily merge adjacent triangle pairs into quads. A pair is merged when the
shared edge's two opposite vertices form a convex quad whose four corner
angles are all within `max_angle` (radians) of 90°, and the two triangle
normals agree.

```rust
pub fn tris_to_quads(mesh: &crate::mesh::Mesh, max_angle: f64) -> crate::mesh::Mesh { /* ... */ }
```

## Module `polyfill`

Ear-clipping triangulation of a single-boundary polygon — a port of
Blender's `BLI_polyfill_calc` (`polyfill_2d.cc`).

# What this computes

Given the `n` corners of one simple polygon, in order, this emits exactly
`n - 2` triangles as index triples into that corner list. The triangles
tile the polygon's interior and do not overlap. This is a *geometric*
routine — it produces no physical quantity and carries no units; the
inputs are plain positions in whatever length unit the caller's mesh uses,
and the outputs are corner indices.

# Why this exists — fan triangulation is wrong on concave faces

[`crate::triangulate`] historically fanned every face from its first
corner. For a **convex** polygon a fan is correct. For a **concave** one it
is not: the fan emits triangles that stick out past the boundary and
overlap each other, so the triangulated surface no longer bounds the same
solid. That matters here because this crate's meshes are handed to a CFD
volume mesher and a Monte Carlo CSG bridge, both of which take the triangle
soup as the truth about the geometry.

Upstream does not fan. `bmo_triangulate` routes n-gons through
`BLI_polyfill_calc`, which is the ear-clipping algorithm ported here.

# Fidelity to upstream, and the three deliberate deviations

The control flow, the two-pass ear search, the "desperate mode" fallback
and the sign conventions are transcribed from `polyfill_2d.cc` rather than
re-derived. Three things differ, each for a stated reason:

1. **No k-d tree.** Upstream compiles `USE_KDTREE` to accelerate the
   point-in-candidate-ear test. This port takes upstream's *own*
   `#else` branch — the linear scan over concave corners, which is in
   `pf_ear_tip_check` verbatim — so the result is identical and only the
   complexity differs (`O(n * concave)` rather than `O(n log n)`). A k-d
   tree is an acceleration structure, not a semantic: porting it would add
   ~330 lines that cannot change an answer.
2. **`f64`, not `f32`.** This crate is `f64` throughout. Upstream's own
   comments note the `TANGENTIAL` test compares exactly against `0.0`, so
   the *classification* of a near-degenerate corner can differ between the
   two precisions. Neither is "right"; ours is the stricter one.
3. **Indices, not pointers.** Upstream's circular doubly-linked list is
   raw `PolyIndex *`. The workspace forbids lifetime parameters and `Box`,
   so the list is an index-linked `Vec` — same structure, same operations.

# Winding — read this before using [`polyfill_2d`] directly

Upstream normalises the working polygon to **clockwise** in 2-D and assumes
that orientation everywhere downstream ("Because the polygon has clockwise
winding order, the area sign will be positive if the point is strictly
inside"). A consequence, which `BLI_polyfill_2d.hh` states as part of the
contract, is that **the emitted triples are clockwise whatever the input
ring's winding was**. That is ported as-is: [`polyfill_2d`] emits clockwise
triangles from a counter-clockwise ring.

Upstream's own mesh callers deal with this by projecting through the
*negated* face normal — `axis_dominant_v3_to_m3_negate` in
`bmesh_mesh_tessellate.cc:116`, immediately before
`BLI_polyfill_calc_arena(projverts, len, 1, ...)` — so the 2-D ring is
already clockwise and the clockwise output maps back to the face's own
winding in 3-D. [`polyfill_3d`] does exactly that, and is therefore the
entry point a mesh operator should call: it preserves the caller's winding
and hence its outward normal.

```rust
pub mod polyfill { /* ... */ }
```

### Functions

#### Function `project_to_plane`

Project 3-D polygon corners onto the plane of `normal`, as upstream's
`axis_dominant_v3_to_m3` + `mul_v2_m3v3` pair does.

`normal` need not be unit length; it is normalised here. A zero-length
normal (a fully degenerate face) falls back to the X/Y plane, which keeps
the routine total rather than panicking.

```rust
pub fn project_to_plane(points: &[crate::math::Vec3], normal: crate::math::Vec3) -> Vec<[f64; 2]> { /* ... */ }
```

#### Function `polyfill_2d`

Ear-clip a single-boundary 2-D polygon into `coords.len() - 2` triangles.

Returns index triples into `coords`, **wound clockwise regardless of the
input ring's winding** — upstream's documented contract
(`BLI_polyfill_2d.hh`: "This array is filled in with triangle indices in
clockwise order"). If you need the caller's winding preserved, use
[`polyfill_3d`], which applies upstream's negated-normal projection.

Fewer than three corners yields no triangles. Self-intersecting input is
degenerate — upstream's contract is that the triangle *count* and the
non-repetition of indices still hold, but the triangles may overlap.

Upstream takes a `coords_sign` hint to skip the winding test; this port
always measures it, which is semantically the same and one shoelace pass
slower.

This is the port of `BLI_polyfill_calc`; see the module docs for the three
deviations from upstream.

```rust
pub fn polyfill_2d(coords: &[[f64; 2]]) -> Vec<[usize; 3]> { /* ... */ }
```

#### Function `polyfill_3d`

Ear-clip a 3-D polygon given its corner positions and face normal.

Returns index triples into `points`, **wound the same way the input ring
is** — so a face's outward normal survives triangulation. This is the
entry point a mesh operator wants.

The winding is preserved by projecting through the *negated* normal, which
is what upstream's tessellation path does
(`axis_dominant_v3_to_m3_negate`, `bmesh_mesh_tessellate.cc:116`): it makes
the 2-D ring clockwise, so [`polyfill_2d`]'s clockwise output maps back to
the face's own orientation. Getting this backwards inverts every normal on
the triangulated mesh, which is why it is spelled out rather than left to
the reader.

```rust
pub fn polyfill_3d(points: &[crate::math::Vec3], normal: crate::math::Vec3) -> Vec<[usize; 3]> { /* ... */ }
```

#### Function `newell_normal`

Newell's method for a polygon normal — robust for non-planar rings, where
a single cross product of three corners is not.

Returns a non-normalised vector whose direction is the face's outward
normal under the ring's own winding, and whose length is twice the
projected area.

```rust
pub fn newell_normal(points: &[crate::math::Vec3]) -> crate::math::Vec3 { /* ... */ }
```

## Module `polyfill_beautify`

Improve an ear-clipped triangulation by rotating its interior edges — a
port of Blender's `BLI_polyfill_beautify` (`polyfill_2d_beautify.cc`).

# What this computes

[`crate::polyfill`] tiles a polygon correctly but not *well*: ear clipping
happily emits long slivers. This pass takes that tiling and repeatedly
flips the shared diagonal of adjacent triangle pairs, each time picking
the flip that most improves the pair, until no flip helps. The vertex set,
the boundary and the triangle count are all unchanged — only which
diagonals are used.

This is a *geometric* quality pass. It carries no units and computes no
physical quantity; it exists because downstream consumers (the CFD volume
mesher, the Monte Carlo CSG bridge) behave badly on sliver triangles.

# The quality measure is area-over-perimeter, not Delaunay

Worth stating plainly, because "beautify" reads like it should be a
Delaunay flip and it is not. Upstream's rule
(`BLI_polyfill_beautify_quad_rotate_calc_ex`) scores each of the two
diagonals of a quad by `sum over the two triangles of (2 * area) /
perimeter`, and rotates when the alternative scores higher. That is a
fatness measure — it maximises the inradius-like ratio — and it is what
[`quad_rotate_cost`] returns, negated so that "negative means rotate".

Two guards in that function are load-bearing and are ported as-is:
a rotation that would make the two triangles point in *opposite*
directions is refused (it would fold the surface), and one that would
produce a zero-area triangle is refused. Conversely, if the *current*
state is already folded or degenerate, the rotation is forced
(`-f64::MAX`, upstream's `-FLT_MAX`) — repairing a bad state beats
preserving it.

# Fidelity to upstream, and the deliberate deviations

The half-edge construction, the pairing of interior edges, the rotation
rewiring, the cost function and the two different insert thresholds are
transcribed rather than re-derived. Differences:

1. **`f64`, not `f32`.** As everywhere in this crate. Upstream's
   `eps_zero_area = 1e-12f` and `-1e-6f` thresholds are kept at those
   numeric values rather than rescaled, since they are tuned against
   coordinate magnitudes, not against the mantissa.
2. **A linear-scan priority queue, not a binary heap.** Upstream keeps an
   indexed `Heap` so it can update or remove one edge's entry in `O(log n)`.
   The number of interior edges is `n - 3` for an `n`-gon, so this port
   keeps a flat `Vec<Option<f64>>` of live costs and scans it for the
   minimum. Same pop order, `O(E)` per pop instead of `O(log E)`.
3. **Ties break on the lower edge index.** Upstream's heap does not
   specify a tie-break, so on exactly-equal costs the two can choose
   differently. This is deterministic, which upstream's is not; where they
   disagree both answers are equally good by the cost measure.
4. **Indices, not pointers.** Required by the workspace no-lifetimes rule.

```rust
pub mod polyfill_beautify { /* ... */ }
```

### Functions

#### Function `quad_rotate_cost`

Score rotating the shared diagonal of the quad `v1 v2 v3 v4` from the
`2-4` diagonal (the current state) to the `1-3` diagonal.

Port of `BLI_polyfill_beautify_quad_rotate_calc_ex`. Returns
`fac_24 - fac_13`, so a **negative** result means rotating improves the
pair. Returns [`f64::MAX`] when the rotation is refused outright, and
[`f64::MIN`] (upstream's `-FLT_MAX`) when the current state is broken
enough that rotating is mandatory.

When `r_area` is `Some`, it receives the mean of the four candidate
triangle areas — upstream includes both diagonals' pairs "for predictable
results", and the caller uses it to scale its re-insertion threshold.

`lock_degenerate` refuses the forced rotation as well; the 2-D polyfill
path passes `false`, the 3-D path in upstream passes `true`.

```rust
pub fn quad_rotate_cost(v1: [f64; 2], v2: [f64; 2], v3: [f64; 2], v4: [f64; 2], lock_degenerate: bool, r_area: Option<&mut f64>) -> f64 { /* ... */ }
```

#### Function `polyfill_beautify`

Improve a triangulation of a single-boundary polygon in place by rotating
interior edges — the port of `BLI_polyfill_beautify`.

`coords` is the polygon ring and `tris` the triangulation of it, as
produced by [`crate::polyfill::polyfill_2d`]; `tris` is rewritten with the
same triangle count over the same vertices. The polygon boundary is never
rotated.

A polygon with fewer than four triangles has no interior edge to rotate,
so this is a no-op below that size.

```rust
pub fn polyfill_beautify(coords: &[[f64; 2]], tris: &mut [[usize; 3]]) { /* ... */ }
```

#### Function `is_quad_flip_v3`

Upstream `is_quad_flip_v3` (`math_geom.cc:5627`) — does a 3-D quad fold
across either of its diagonals?

Returns a bit mask: bit 0 set means the `v1-v3` split folds, bit 1 set
means the `v2-v4` split does. A convex planar quad returns 0.

```rust
pub fn is_quad_flip_v3(v1: crate::math::Vec3, v2: crate::math::Vec3, v3: crate::math::Vec3, v4: crate::math::Vec3) -> u8 { /* ... */ }
```

#### Function `edge_rotate_cost_3d`

Score rotating a **3-D** quad's diagonal from `2-4` to `1-3`, by
projecting the quad onto the plane of its own averaged normal and
applying [`quad_rotate_cost`] there.

Port of `BLI_polyfill_edge_calc_rotate_beauty__area`
(`polyfill_2d_beautify.cc:180`). Negative means rotating improves the
pair; [`f64::MAX`] means refuse.

Two upstream subtleties are kept:

- The projection plane is the sum of the two *current* triangle normals,
  so the measure is taken in the quad's own frame rather than an
  arbitrary axis. A zero-length sum (the two triangles exactly oppose)
  means there is no sensible plane, and the rotation is refused.
- The two triangles are rejected outright when they already wind
  oppositely or are both degenerate, via the `signum_i_ex` sum. Upstream
  spells out the accept/ignore table: `(1,1)`/`(-1,-1)` accept,
  `(+/-1, 0)` accept (one degenerate — a rotation may fix it),
  `(-1, 1)` and `(0, 0)` ignore.

```rust
pub fn edge_rotate_cost_3d(v1: crate::math::Vec3, v2: crate::math::Vec3, v3: crate::math::Vec3, v4: crate::math::Vec3, lock_degenerate: bool) -> f64 { /* ... */ }
```

## Module `primitives`

**Real** mesh primitive generators (Blender's "Add Mesh" primitives).

This is the one module in the crate with fully implemented, unit-tested
algorithms — the analogue of Blender's `editors/mesh/editmesh_add.cc` add
operators (Add Cube / UV Sphere / Cylinder / Grid). Each function returns a
valid [`Mesh`] built entirely through the public [`Mesh::add_vertex`] /
[`Mesh::add_face`] API, so edge deduplication and loop wiring come for free.

## Correctness checks

The generators are validated against **Euler's polyhedron formula**
`V - E + F = chi`:

- closed genus-0 solids ([`cube`], [`uv_sphere`], [`cylinder`]) give
  `chi = 2`;
- a flat [`grid`] patch is a topological disc, `chi = 1`.

These are exact topological identities, so the tests assert them exactly
(no tolerance). Vertex/edge/face counts are also checked against the closed
forms derived in each function's doc comment.

## Units

All size/radius/height arguments are dimensionless model-space lengths (see
[`crate::math`]); a length unit is attached only when a mesh is handed to a
solver through an [`crate::export`] bridge.

```rust
pub mod primitives { /* ... */ }
```

### Functions

#### Function `cube`

An axis-aligned cube of side `size`, centred on the origin.

Topology: **8** vertices, **12** edges, **6** quad faces, `chi = 2`. Faces
are wound counter-clockwise as seen from outside, so face normals point
outward. `size` is the full edge length (a `size` of `1.0` spans `-0.5..0.5`
on each axis).

```rust
pub fn cube(size: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `uv_sphere`

A UV (latitude/longitude) sphere of the given `radius`.

`segments` is the number of subdivisions **around** the polar axis
(longitude), `rings` the number of subdivisions **from pole to pole**
(latitude). Both must be `>= 3` / `>= 2` respectively for a sensible sphere;
this matches Blender's Add-UV-Sphere defaults of 32 segments and 16 rings.

Topology (closed, genus-0, `chi = 2`): **2 poles + (rings - 1) * segments**
vertices; the two pole caps are triangle fans (`segments` triangles each)
and the `rings - 2` intermediate bands are quad strips
(`(rings - 2) * segments` quads). Faces are wound counter-clockwise as seen
from outside, so face normals point **outward** (positive enclosed volume) —
consistent with [`cube`] and [`cylinder`].

# Panics

Panics if `segments < 3` or `rings < 2` — too few to close the surface.

```rust
pub fn uv_sphere(segments: usize, rings: usize, radius: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `cylinder`

A closed cylinder of the given `radius` and `height`, axis along Z.

`segments` is the number of sides around the axis (Blender's "Vertices"
field, default 32). The caps are single n-gon faces (Blender's "Ngon"
cap-fill), not triangle fans.

Topology (closed, genus-0, `chi = 2`): **2 * segments** vertices,
**3 * segments** edges, **segments + 2** faces (`segments` side quads plus
two `segments`-gon caps). The cylinder spans `-height/2 .. height/2` on Z.

# Panics

Panics if `segments < 3` — too few sides to enclose a volume.

```rust
pub fn cylinder(segments: usize, radius: f64, height: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `grid`

A flat rectangular grid (subdivided plane) in the Z = 0 plane.

`nx` and `ny` are the number of quad subdivisions along X and Y (each `>= 1`);
`size` is the full side length (the patch spans `-size/2 .. size/2` on both
axes). This is Blender's Add-Grid primitive.

Topology (a topological **disc**, `chi = 1`): **(nx + 1) * (ny + 1)**
vertices and **nx * ny** quad faces. Being an open patch (it has a boundary),
it is *not* closed — that is the intended difference from [`cube`] /
[`cylinder`] / [`uv_sphere`], and its Euler characteristic is 1, not 2.

# Panics

Panics if `nx < 1` or `ny < 1`.

```rust
pub fn grid(nx: usize, ny: usize, size: f64) -> crate::mesh::Mesh { /* ... */ }
```

## Module `primitives_extra`

**Extra mesh primitives + Add-Mesh common settings** (`op-hzs.54.39`, GH
issue #37 §H).

Completes the [`crate::primitives`] set with the primitives Blender's *Add
Mesh* menu offers that were not yet covered:

- [`plane`] — a single quad (Add Plane).
- [`circle`] — an `n`-gon, optionally filled (Add Circle, fill = Nothing /
  N-Gon).
- [`cone`] — a cone or truncated cone (Add Cone, with `radius2`).
- [`torus`] — a ring torus (Add Torus, major/minor radius + segments).
- [`icosphere`] — a geodesic sphere by `subdivisions` of an icosahedron
  (Add Ico Sphere).

[`AddMeshOptions`] is the "redo panel" every add-operator shares: where to
put the new geometry (`location`) and how to orient it (`rotation_euler`,
XYZ radians). [`AddMeshOptions::place`] applies it to a freshly generated
mesh.

## Units

All radius / size arguments are dimensionless model-space lengths, as in
[`crate::primitives`]; angles are radians.

```rust
pub mod primitives_extra { /* ... */ }
```

### Types

#### Struct `AddMeshOptions`

The common "redo panel" settings shared by every *Add Mesh* operator:
where the new geometry is placed and how it is oriented.

```rust
pub struct AddMeshOptions {
    pub location: crate::math::Vec3,
    pub rotation_euler: crate::math::Vec3,
    pub scale: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `location` | `crate::math::Vec3` | World-space location of the primitive's origin. |
| `rotation_euler` | `crate::math::Vec3` | Orientation as intrinsic XYZ Euler angles, in radians, applied about<br>the primitive's own origin before translation. |
| `scale` | `f64` | Uniform scale applied about the origin before rotation (`1.0` = none). |

##### Implementations

###### Methods

- ```rust
  pub fn at(location: Vec3) -> Self { /* ... */ }
  ```
  Placement at `location` with no rotation and unit scale.

- ```rust
  pub fn place(self: &Self, mesh: &Mesh) -> Mesh { /* ... */ }
  ```
  Apply this placement (scale → rotate → translate) to `mesh`, returning

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> AddMeshOptions { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AddMeshOptions) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `plane`

A single `size x size` quad in the `z = 0` plane, centred on the origin,
wound CCW as seen from `+z`.

Topology: **4** vertices, **4** edges, **1** face, `chi = 1` (a disc).

```rust
pub fn plane(size: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `circle`

A regular `segments`-gon of `radius` in the `z = 0` plane, centred on the
origin.

With `fill = true` a single `segments`-sided N-gon face is added (`chi = 1`);
with `fill = false` only the boundary ring of edges exists (a wire loop,
`chi = 0`). `segments` is clamped to `>= 3`.

```rust
pub fn circle(segments: usize, radius: f64, fill: bool) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `cone`

A cone (or truncated cone) about the `z` axis, base at `z = -height/2`.

`radius1` is the base radius, `radius2` the top radius (`0.0` → a true
apex). `segments` (clamped `>= 3`) sides. Base and top are filled with
N-gon caps (the top cap is omitted when `radius2 == 0`). Closed genus-0,
`chi = 2`.

```rust
pub fn cone(segments: usize, radius1: f64, radius2: f64, height: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `torus`

A ring torus about the `z` axis: a tube of `minor_radius` whose centreline
is a circle of `major_radius` in the `z = 0` plane.

`major_segments` around the main ring, `minor_segments` around the tube
(each clamped `>= 3`). All quads, closed, genus-1 → `chi = 0`.

```rust
pub fn torus(major_segments: usize, minor_segments: usize, major_radius: f64, minor_radius: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `icosphere`

A geodesic sphere of `radius`: an icosahedron subdivided `subdivisions`
times, each new vertex pushed back onto the sphere.

`subdivisions` is clamped to `0..=5` (a level-5 icosphere is 20480 faces).
All triangles, closed genus-0 → `chi = 2`.

```rust
pub fn icosphere(subdivisions: usize, radius: f64) -> crate::mesh::Mesh { /* ... */ }
```

## Module `procedural`

Procedural geometry generation — a **Geometry-Nodes-style** node graph
(Blender's `nodes/geometry`, the Geometry Nodes system).

## The concept

Blender's Geometry Nodes builds geometry by evaluating a directed acyclic
graph of nodes: *input* nodes create primitives, *processing* nodes
transform or combine geometry (many wrap the same verbs as [`crate::ops`]),
and an *output* node yields the final mesh. The graph is data, not code, so
a design can be authored, stored, and replayed — which is exactly what an
OUTRAM PARK reactor-geometry generator wants (parametric fuel pins, lattices,
coolant channels driven by a few numeric inputs).

## What is implemented

[`GeometryGraph::evaluate`] is a **real** evaluator. It locates the single
[`GeometryNode::OutputMesh`] node and walks the graph upstream by
[`NodeId`], building a [`Mesh`] bottom-up:

- [`GeometryNode::Primitive`] emits a fresh primitive from [`crate::primitives`];
- [`GeometryNode::Transform`] applies a translation via [`crate::transform::Affine3`];
- [`GeometryNode::Join`] concatenates two meshes (offsetting the second
  operand's face indices by the first's vertex count);
- [`GeometryNode::Subdivide`] delegates to [`crate::subdivision::catmull_clark`];
- [`GeometryNode::Boolean`] delegates to [`crate::boolean::boolean`].

The walk is defensive: an out-of-range [`NodeId`] returns
[`ProceduralError::BadNode`] and a cyclic reference returns
[`ProceduralError::Cycle`] — a malformed graph never panics.

## Why an enum of nodes

The node kinds are a closed set, so [`GeometryNode`] is an enum (no trait
objects), and edges between nodes are **indices** ([`NodeId`]) into the
graph's node `Vec` — the same no-pointers/no-lifetimes discipline as
[`crate::mesh`].

```rust
pub mod procedural { /* ... */ }
```

### Types

#### Enum `ProceduralError`

Errors from evaluating a [`GeometryGraph`].

```rust
pub enum ProceduralError {
    NoOutput,
    BadNode(NodeId),
    Cycle(NodeId),
    Boolean(crate::boolean::BooleanError),
}
```

##### Variants

###### `NoOutput`

The graph has no [`GeometryNode::OutputMesh`] node to read a result from.

###### `BadNode`

A node referenced a [`NodeId`] that is out of range for the graph — a
malformed edge. Carries the offending id.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `NodeId` |  |

###### `Cycle`

A cycle was detected while walking upstream from the output — the node
carried is one that was re-entered while already on the current
depth-first path. A valid geometry graph must be acyclic.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `NodeId` |  |

###### `Boolean`

A [`GeometryNode::Boolean`] node's underlying CSG operation failed; the
[`crate::boolean::BooleanError`] is propagated unchanged.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::boolean::BooleanError` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Error**
  - ```rust
    fn source(self: &Self) -> ::core::option::Option<&dyn ::thiserror::__private18::Error + ''static> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(source: crate::boolean::BooleanError) -> Self { /* ... */ }
    ```

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `NodeId`

Index of a [`GeometryNode`] within a [`GeometryGraph`].

```rust
pub struct NodeId(pub usize);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> NodeId { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &NodeId) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `PrimitiveKind`

Which primitive an input node emits.

Each variant maps one-to-one onto a generator in [`crate::primitives`]; the
parameters are the same dimensionless model-space lengths those generators
take (see [`crate::math`]).

```rust
pub enum PrimitiveKind {
    Cube {
        size: f64,
    },
    UvSphere {
        segments: usize,
        rings: usize,
        radius: f64,
    },
    Cylinder {
        segments: usize,
        radius: f64,
        height: f64,
    },
}
```

##### Variants

###### `Cube`

A cube of the given side length — see [`crate::primitives::cube`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `size` | `f64` | Full edge length. |

###### `UvSphere`

A UV sphere — see [`crate::primitives::uv_sphere`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `segments` | `usize` | Subdivisions around the polar axis (must be `>= 3`). |
| `rings` | `usize` | Subdivisions pole to pole (must be `>= 2`). |
| `radius` | `f64` | Sphere radius. |

###### `Cylinder`

A closed cylinder with axis along Z — see [`crate::primitives::cylinder`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `segments` | `usize` | Number of sides around the axis (must be `>= 3`). |
| `radius` | `f64` | Cylinder radius. |
| `height` | `f64` | Full height along Z (the cylinder spans `-height/2 .. height/2`). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> PrimitiveKind { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PrimitiveKind) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `GeometryNode`

A node in a procedural geometry graph.

Each node references its inputs by [`NodeId`]. Evaluation performs a
depth-first walk from [`GeometryNode::OutputMesh`] back to the input nodes
(see [`GeometryGraph::evaluate`]).

```rust
pub enum GeometryNode {
    Primitive(PrimitiveKind),
    Transform {
        input: NodeId,
        translate: [f64; 3],
    },
    Join {
        a: NodeId,
        b: NodeId,
    },
    Subdivide {
        input: NodeId,
        levels: u32,
    },
    Boolean {
        a: NodeId,
        b: NodeId,
        mode: crate::ops::BooleanMode,
    },
    OutputMesh {
        input: NodeId,
    },
}
```

##### Variants

###### `Primitive`

Source node: emit a primitive mesh.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `PrimitiveKind` |  |

###### `Transform`

Transform the geometry coming from `input` by a uniform translation.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `input` | `NodeId` | Upstream node whose geometry is transformed. |
| `translate` | `[f64; 3]` | Model-space translation `[dx, dy, dz]` applied to every vertex. |

###### `Join`

Join two geometry streams into one mesh (concatenate elements). The two
operands keep their own vertices — no welding/deduplication is performed,
so joining two disjoint solids yields a mesh with both, each still a
separate closed surface.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `NodeId` | First upstream node. |
| `b` | `NodeId` | Second upstream node. |

###### `Subdivide`

Refine the geometry from `input` with `levels` rounds of Catmull-Clark
subdivision — see [`crate::subdivision::catmull_clark`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `input` | `NodeId` | Upstream node whose geometry is subdivided. |
| `levels` | `u32` | Number of Catmull-Clark refinement passes. |

###### `Boolean`

Combine the geometry from `a` and `b` under a CSG [`crate::ops::BooleanMode`]
— see [`crate::boolean::boolean`]. A failure of the underlying operation
surfaces as [`ProceduralError::Boolean`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `NodeId` | First operand node (the `A` mesh). |
| `b` | `NodeId` | Second operand node (the `B` mesh). |
| `mode` | `crate::ops::BooleanMode` | Union / difference / intersection. |

###### `OutputMesh`

Terminal node: the graph's result is the geometry from `input`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `input` | `NodeId` | Upstream node providing the final mesh. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> GeometryNode { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `GeometryGraph`

A directed graph of [`GeometryNode`]s that evaluates to a single [`Mesh`].

Nodes are stored in a `Vec` and referenced by [`NodeId`]; add nodes with
[`GeometryGraph::add`], which returns the new node's id for wiring later
nodes to it.

```rust
pub struct GeometryGraph {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```
  Create an empty graph.

- ```rust
  pub fn add(self: &mut Self, node: GeometryNode) -> NodeId { /* ... */ }
  ```
  Add a node and return its [`NodeId`] for wiring downstream nodes.

- ```rust
  pub fn node(self: &Self, id: NodeId) -> Option<&GeometryNode> { /* ... */ }
  ```
  Read-only access to a node by id.

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Number of nodes in the graph.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether the graph has no nodes.

- ```rust
  pub fn evaluate(self: &Self) -> Result<Mesh, ProceduralError> { /* ... */ }
  ```
  Evaluate the graph to a final mesh.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> GeometryGraph { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> GeometryGraph { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `reactor`

# Reactor geometry generators (frontend authoring)

Domain generators that compose the crate's primitive / [`revolve`](crate::revolve)
/ boolean operators into whole-reactor **surfaces**, ready to hand to the
solver bridges: the volume-meshing bridge ([`crate::foam_mesh`], feature
`foam-mesh`, for CFD / thermal-hydraulics) and the Monte-Carlo bridge
([`crate::sim`], feature `mc-export`, for neutronics). Nothing here meshes or
simulates — it only *authors* a closed, watertight, outward-wound
[`Mesh`](crate::mesh::Mesh) that those bridges then consume.

The first generator is the **HTR-10 pebble-bed core envelope**
([`htr10_core_envelope`]): the surface of revolution of the core region,
which becomes the single tet-dual polyMesh shared by neutronics and TH per
`docs/reactor-scoping/htr10-neutronics.md` §8.

## Units

All lengths are **metres** (`f64`). This crate's [`Vec3`](crate::math::Vec3)
is nominally dimensionless model space; the solver bridges treat one model
unit as one metre ([`crate::foam_mesh`] "Units and conventions"), so this
module works directly in metres to match.

## Provenance and the honest limit of the open geometry

The published, citable HTR-10 dimensions are transcribed from the **IAEA
HTGR benchmark document (IAEA-TECDOC-1382, Chapter 4)** — the same Open-tier
source behind
`crates/outram-park-digital-twin-engine/src/htr10/design.rs`
([`Htr10DesignPoint::iaea_benchmark`]): core diameter **180 cm**, average
core height **197 cm**, stated core volume **5.0 m³**, side-reflector
thickness **100 cm**.

Those numbers pin down the pebble-bed cavity as a *volume-equivalent
cylinder*: `π · (0.90 m)² · 1.97 m = 5.01 m³`, which closes against the cited
5.0 m³. That cylinder is the honest default this module emits
([`Htr10CoreDimensions::iaea_benchmark`]).

**What the open text does NOT give** (see the scoping doc §7.3): the conus
half-angle, the discharge-tube radius, the cavity-vs-conus axial split, and
the exact zone-boundary coordinates are *not* recoverable from the published
text and "must not be treated as authoritative". This module therefore keeps
the conus/discharge-tube geometry **opt-in and explicitly flagged as
illustrative** ([`Htr10CoreDimensions::illustrative_with_conus`]); it is a
shape for exercising the meshing pipeline, not a validated HTR-10 core. The
exact geometry awaits INL's evaluation of the initial critical configuration
(Terry, 2005, `INL/CON-05-00852`) or the upstream VTB mesh — see the scoping
doc's open question 3.

> **Education / research only**, open-source data only (`DATA_POLICY.md`).
> Not for reactor operation, licensing, or safety-critical use. Independent
> OUTRAM PARK work; not affiliated with the HTR-10 designers.

```rust
pub mod reactor { /* ... */ }
```

### Types

#### Struct `Htr10Conus`

The conical bottom + discharge tube of the HTR-10 core, as an **illustrative
(non-authoritative)** refinement of the pebble-bed cavity.

The pebble bed physically funnels through a conus to a central discharge
tube, but the conus half-angle and the discharge-tube radius are **not in
the open HTR-10 text** (`docs/reactor-scoping/htr10-neutronics.md` §7.3), so
every field here is a shape parameter for exercising the meshing pipeline,
**not** a validated dimension. Supply your own once the exact geometry is
obtained (Terry 2005 / the VTB mesh).

All lengths are metres.

```rust
pub struct Htr10Conus {
    pub discharge_tube_radius: f64,
    pub discharge_tube_height: f64,
    pub conus_height: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `discharge_tube_radius` | `f64` | Radius of the central fuel-discharge tube, metres. **Illustrative** —<br>not fixed by the open text. |
| `discharge_tube_height` | `f64` | Straight height of the discharge tube below the conus, metres.<br>**Illustrative.** |
| `conus_height` | `f64` | Axial rise of the conus frustum (from discharge-tube radius out to the<br>full cavity radius), metres. **Illustrative** — together with the two<br>radii this sets the conus half-angle. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Htr10Conus { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Htr10Conus) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `Htr10CoreDimensions`

HTR-10 pebble-bed core envelope dimensions (metres).

Construct with [`Htr10CoreDimensions::iaea_benchmark`] for the honest,
fully-cited volume-equivalent cylinder, or
[`Htr10CoreDimensions::illustrative_with_conus`] to add the opt-in,
explicitly-non-authoritative conus + discharge tube. Feed the result to
[`htr10_core_envelope`].

The `side_reflector_thickness` is carried for completeness (the cited 100 cm
side reflector) but the default envelope is the **cavity only** — the pebble
bed itself. Meshing the surrounding reflector and its boring pattern needs
the exact zone geometry the open text lacks (§7.3), so it is deferred to a
follow-up rather than fabricated here.

```rust
pub struct Htr10CoreDimensions {
    pub cavity_radius: f64,
    pub cavity_height: f64,
    pub side_reflector_thickness: f64,
    pub conus: Option<Htr10Conus>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `cavity_radius` | `f64` | Pebble-bed cavity radius, metres. Cited: core diameter 180 cm ⇒ 0.90 m<br>(IAEA-TECDOC-1382 Ch. 4). |
| `cavity_height` | `f64` | Straight cavity height, metres. For the cited cylinder this is the<br>average core height 197 cm ⇒ 1.97 m, chosen so<br>`π · cavity_radius² · cavity_height` reproduces the stated 5.0 m³ core<br>volume. With a conus, this is the height of the straight cylindrical<br>section *above* the conus. |
| `side_reflector_thickness` | `f64` | Side-reflector thickness, metres. Cited: 100 cm ⇒ 1.0 m. Not used by the<br>cavity-only envelope; carried so a later reflector-inclusive generator<br>need not re-transcribe it. |
| `conus` | `Option<Htr10Conus>` | Optional, **illustrative** conus + discharge tube (see [`Htr10Conus`]).<br>`None` yields the honest volume-equivalent cylinder. |

##### Implementations

###### Methods

- ```rust
  pub fn iaea_benchmark() -> Self { /* ... */ }
  ```
  The fully-cited HTR-10 core cavity as a **volume-equivalent cylinder**:

- ```rust
  pub fn illustrative_with_conus(conus: Htr10Conus) -> Self { /* ... */ }
  ```
  The cited cavity **plus an illustrative conus + discharge tube**.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Htr10CoreDimensions { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Htr10CoreDimensions) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `htr10_core_envelope`

Author the HTR-10 pebble-bed core envelope as a closed, watertight,
outward-wound surface [`Mesh`](crate::mesh::Mesh).

The envelope is a **surface of revolution** about the vertical (Z) axis with
its base at `z = 0`:

- **No conus** (`dims.conus == None`): a plain cylinder of radius
  `cavity_radius` and height `cavity_height` — the cited volume-equivalent
  cavity.
- **With conus** (`dims.conus == Some(..)`): from the base up, a straight
  discharge tube, then a conus frustum flaring out to `cavity_radius`, then
  the straight cavity wall — an **illustrative** funnel-bottomed cavity.

`segments` is the number of circumferential bands (`>= 3`); pass
[`DEFAULT_SEGMENTS`] for a sensible default. The revolved side wall is capped
top and bottom by [`fill_holes`](crate::fill_holes::fill_holes) and its
winding is made outward by
[`recalculate_normals`](crate::recalc_normals::recalculate_normals), so the
result satisfies the closed-2-manifold precondition of
[`crate::foam_mesh::mesh_to_tet_dual`].

# Examples

```
use outram_blender::reactor::{htr10_core_envelope, Htr10CoreDimensions, DEFAULT_SEGMENTS};

let dims = Htr10CoreDimensions::iaea_benchmark();
let core = htr10_core_envelope(&dims, DEFAULT_SEGMENTS);
// Closed solid of revolution: Euler characteristic 2, no boundary edges.
assert_eq!(core.euler_characteristic(), 2);
```

```rust
pub fn htr10_core_envelope(dims: &Htr10CoreDimensions, segments: usize) -> crate::mesh::Mesh { /* ... */ }
```

### Constants and Statics

#### Constant `DEFAULT_SEGMENTS`

Default number of circumferential segments in a revolved envelope — a
compromise between a smooth wall and a light triangle count. The faceted
(inscribed `N`-gon) volume approaches the true cylinder volume as this rises;
at `N = 48` the cavity volume is already within ~0.1 % of the analytic
cylinder (see [`tests`]).

```rust
pub const DEFAULT_SEGMENTS: usize = 48;
```

## Module `proportional`

**Proportional editing** (`op-hzs.54.20`, GH issue #37 §C) — spread a set of
explicit vertex displacements to nearby vertices by a falloff curve.

[`proportional_move`] takes the vertices the caller is "grabbing" with their
target displacements, a `radius`, a [`Falloff`], and `connected_only` (use
topological distance along edges instead of straight-line distance). Each
other vertex within `radius` of a grabbed vertex moves by
`falloff(dist / radius) · displacement_of_nearest_grabbed`.

```rust
pub mod proportional { /* ... */ }
```

### Types

#### Enum `Falloff`

Blender's proportional-edit falloff curves. `t` is `distance / radius` in
`[0, 1]`; each returns a weight in `[0, 1]` that is `1` at `t = 0`.

```rust
pub enum Falloff {
    Smooth,
    Sphere,
    Root,
    InverseSquare,
    Sharp,
    Linear,
    Constant,
    Random,
}
```

##### Variants

###### `Smooth`

`2t³ − 3t² + 1` (the default smoothstep).

###### `Sphere`

`√(1 − t²)` — a quarter circle, `1` at the centre.

###### `Root`

`√(1 − t)`.

###### `InverseSquare`

`(1 − t)²`.

###### `Sharp`

`t² − 2t + 1` … actually the sharp curve `(1 − t)² `? Blender's Sharp is
`(1 − t)²` with a steeper toe — implemented as `((1 − t))³`.

###### `Linear`

`1 − t`.

###### `Constant`

`1` for all `t < 1` (a hard cutoff).

###### `Random`

`1 − t` scaled by a per-vertex random value (see
[`proportional_move`]'s `seed`).

##### Implementations

###### Methods

- ```rust
  pub fn weight(self: Self, t: f64) -> f64 { /* ... */ }
  ```
  The weight for a normalised distance `t` (clamped to `[0, 1]`).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Falloff { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Falloff) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `proportional_move`

Apply the explicit displacements in `grabbed` and spread them to nearby
vertices per `falloff` within `radius`. `connected_only` measures distance
along mesh edges (Dijkstra); otherwise straight-line. `seed` is used only
by [`Falloff::Random`].

```rust
pub fn proportional_move(mesh: &crate::mesh::Mesh, grabbed: &[(crate::mesh::VertexId, crate::math::Vec3)], radius: f64, falloff: Falloff, connected_only: bool, seed: u64) -> crate::mesh::Mesh { /* ... */ }
```

## Module `recalc_normals`

Recalculate normals — make a mesh's face winding globally consistent and
outward-facing.

This is the pure-Rust analogue of Blender's **Recalculate Normals Outside**
(`mesh.normals_make_consistent`). It repairs a polygon soup whose faces are
wound inconsistently — the common state of an imported or hand-assembled
mesh — so that adjacent faces agree on their shared edges and every
connected component's normals point outward.

# Why this is not just [`crate::boolean_general`]'s orient-outward

`boolean_general` flips a whole mesh by signed-volume sign, but only when it
is **already** consistently wound. This operator does the harder job first:
a breadth-first propagation across the face-adjacency graph that flips any
neighbour disagreeing on a shared edge, turning an inconsistent soup into a
consistently-wound one. Only then is each connected component flipped
outward.

# Algorithm

1. **Adjacency.** Index faces by their undirected edges (sorted vertex
   pair). Two faces sharing an edge are adjacent.
2. **Orientation propagation (BFS).** Seed each connected component with one
   face (unflipped). For a neighbour across a shared edge, the two faces are
   consistent iff they traverse that edge in **opposite** directions; if not,
   the neighbour is marked to flip. Directions compose by XOR, so the
   neighbour's flip is a closed-form function of the seed-relative
   orientation.
3. **Outward pass.** For each connected component, compute the signed volume
   (divergence theorem) of its now-consistent faces; if negative, flip the
   whole component so its normals point outward.

A non-orientable surface (e.g. a Möbius band) cannot be made globally
consistent — the BFS will visit it, but the result is best-effort along the
spanning tree, matching Blender's own behaviour. No `faer`, no external
dependency; Android-safe.

```rust
pub mod recalc_normals { /* ... */ }
```

### Functions

#### Function `recalculate_normals`

Return `mesh` with every face rewound so the surface is consistently wound
and each connected component faces outward (positive enclosed volume).

This is infallible. An already-correct mesh is returned with the same
winding; an inconsistently-wound soup is repaired; a fully-inward mesh is
flipped outward. Winding is the only thing changed — vertex positions and
the face-vertex sets are untouched.

# Examples

```
use outram_blender::{primitives, recalc_normals::recalculate_normals};

// A pristine cube is already outward-wound, so this is a no-op on topology.
let cube = primitives::cube(2.0);
let fixed = recalculate_normals(&cube);
assert_eq!(fixed.vertex_count(), 8);
assert_eq!(fixed.face_count(), 6);
assert_eq!(fixed.euler_characteristic(), 2);
```

```rust
pub fn recalculate_normals(mesh: &crate::mesh::Mesh) -> crate::mesh::Mesh { /* ... */ }
```

## Module `rip_split`

**Rip / Split / Separate** (`op-hzs.54.12`, GH issue #37 §B).

- [`split_faces`] — disconnect the selected faces from the rest along their
  shared boundary (each shared vertex is duplicated), leaving one mesh with
  two islands that no longer share topology. Blender's `Y`.
- [`separate_selection`] — remove the selected faces from the mesh and
  return them as a **second** mesh. Blender's `P ▸ Selection`.
- [`separate_loose_parts`] — one mesh per connected component. Blender's
  `P ▸ By Loose Parts`.
- [`separate_by_group`] — one mesh per value of a caller-supplied
  per-face key (stands in for `P ▸ By Material` until material layers land
  in `op-hzs.54.28`).
- [`rip_edges`] — duplicate the vertices of the given interior edges and
  hand one side's faces the copies, tearing a slit. A minimal headless
  Rip; Rip Fill (capping the slit) composes it with
  [`crate::fill_holes`].

```rust
pub mod rip_split { /* ... */ }
```

### Functions

#### Function `split_faces`

Disconnect the selected faces from the unselected ones: every vertex shared
by both groups is duplicated so the two groups no longer share it. Returns
one mesh with two now-independent islands (positions unchanged).

```rust
pub fn split_faces(mesh: &crate::mesh::Mesh, faces: &[crate::mesh::FaceId]) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `separate_selection`

Remove the selected faces from `mesh` and return `(remaining, separated)`.
Each result is compacted to only its own vertices.

```rust
pub fn separate_selection(mesh: &crate::mesh::Mesh, faces: &[crate::mesh::FaceId]) -> (crate::mesh::Mesh, crate::mesh::Mesh) { /* ... */ }
```

#### Function `separate_loose_parts`

Split `mesh` into one mesh per connected component (by shared edge).

```rust
pub fn separate_loose_parts(mesh: &crate::mesh::Mesh) -> Vec<crate::mesh::Mesh> { /* ... */ }
```

#### Function `separate_by_group`

Split `mesh` into one mesh per distinct value of `group(face)` — a
stand-in for *Separate by Material*. Returns `(key, mesh)` sorted by key.

```rust
pub fn separate_by_group</* synthetic */ impl Fn(FaceId) -> usize: Fn(crate::mesh::FaceId) -> usize>(mesh: &crate::mesh::Mesh, group: impl Fn(crate::mesh::FaceId) -> usize) -> Vec<(usize, crate::mesh::Mesh)> { /* ... */ }
```

#### Function `rip_edges`

Tear a slit along `edges`: each edge's vertices are duplicated and the faces
on **one** side of the edge (the second incident face, by id) get the
copies. An interior edge becomes an open slit; pair with
[`crate::fill_holes`] for Rip Fill.

```rust
pub fn rip_edges(mesh: &crate::mesh::Mesh, edges: &[crate::mesh::EdgeId]) -> crate::mesh::Mesh { /* ... */ }
```

## Module `remesh`

**Voxel remesh + Mesh ↔ Volume** (`op-hzs.54.30`, GH issue #37 §F).

- [`VoxelGrid`] — a dense occupancy grid (one `bool` per cell).
- [`mesh_to_volume`] — rasterise a **closed** mesh: a cell is occupied if
  its centre is inside (ray-parity test). Blender's *Mesh to Volume*.
- [`volume_to_mesh`] — emit a quad for every occupied-cell face that borders
  an empty cell, giving a watertight blocky surface. Blender's *Volume to
  Mesh*.
- [`voxel_remesh`] — [`mesh_to_volume`] → [`volume_to_mesh`] →
  `smooth_iters` Laplacian passes. Blender's voxel Remesh (blocks mode at
  `smooth_iters = 0`, voxel/smooth otherwise).

```rust
pub mod remesh { /* ... */ }
```

### Types

#### Struct `VoxelGrid`

A dense voxel occupancy grid.

```rust
pub struct VoxelGrid {
    pub origin: crate::math::Vec3,
    pub cell: f64,
    pub dims: [usize; 3],
    pub occ: Vec<bool>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `origin` | `crate::math::Vec3` | World position of the centre of cell `(0, 0, 0)`. |
| `cell` | `f64` | Edge length of a cell. |
| `dims` | `[usize; 3]` | Grid resolution `[nx, ny, nz]`. |
| `occ` | `Vec<bool>` | `occ[x + nx*(y + ny*z)]` — whether that cell is inside the volume. |

##### Implementations

###### Methods

- ```rust
  pub fn get(self: &Self, x: i64, y: i64, z: i64) -> bool { /* ... */ }
  ```
  Whether cell `(x, y, z)` is occupied (`false` for out-of-range).

- ```rust
  pub fn cell_center(self: &Self, x: usize, y: usize, z: usize) -> Vec3 { /* ... */ }
  ```
  The world-space centre of cell `(x, y, z)`.

- ```rust
  pub fn occupied_count(self: &Self) -> usize { /* ... */ }
  ```
  Number of occupied cells.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> VoxelGrid { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `mesh_to_volume`

Rasterise `mesh` (assumed closed and consistently wound) into a
[`VoxelGrid`] of cell size `cell`. Adds one cell of padding on every side.

```rust
pub fn mesh_to_volume(mesh: &crate::mesh::Mesh, cell: f64) -> VoxelGrid { /* ... */ }
```

#### Function `volume_to_mesh`

Emit a watertight blocky surface: one outward-facing quad per occupied-cell
face that borders an empty cell.

```rust
pub fn volume_to_mesh(grid: &VoxelGrid) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `voxel_remesh`

Voxel remesh: rasterise, re-surface, then `smooth_iters` Laplacian passes
(`0` = the blocky result).

```rust
pub fn voxel_remesh(mesh: &crate::mesh::Mesh, cell: f64, smooth_iters: u32) -> crate::mesh::Mesh { /* ... */ }
```

## Module `revolve`

Revolve / spin — sweep a profile polyline around an axis into a surface of
revolution.

This is the pure-Rust analogue of Blender's **Spin**: a profile (an ordered
list of points) is rotated around an arbitrary axis in `segments` steps
through a total `angle`, and consecutive rings are stitched with quads. It
generates the surfaces of revolution a reactor geometry is full of — pipes,
tubes, pressure-vessel walls, cone frusta.

- A **full** sweep (`angle >= 2π`) closes the seam: the last ring wraps back
  onto the first, giving a closed loop of quads (an open-ended tube — cap it
  with [`crate::fill_holes`] for a solid).
- A **partial** sweep leaves the two end rings unstitched (an open strip).

# Profile orientation

The profile is a polyline in space; each point is rotated about the axis.
Neighbouring profile points become the two rails of a quad band, so the
profile's own ordering sets the surface's `v` direction and the sweep sets
its `u` direction. The winding is internally consistent across the whole
surface (fix its outward sense with
[`crate::recalc_normals::recalculate_normals`] if a particular profile winds
it inward).

# Limitation (v1): on-axis profile points

A profile point lying **on** the axis (a pole, radius ≈ 0) is rotated to the
same location on every ring, producing coincident duplicate vertices rather
than a shared pole. For a profile that touches the axis (e.g. a semicircle
swept into a sphere), run [`crate::weld::weld`] afterward to merge the pole
copies. Off-axis profiles need no post-processing. No `faer`, no external
dependency; Android-safe.

```rust
pub mod revolve { /* ... */ }
```

### Functions

#### Function `revolve`

Sweep `profile` around the axis through `axis_point` along `axis_dir` by
`angle` radians in `segments` steps, returning the surface of revolution.

`segments` is the number of quad bands around the sweep (`>= 3` for a full
revolution, `>= 1` for a partial one); `profile` needs at least two points.
An `angle` of `2π` (or more) makes a closed loop; a smaller angle leaves the
ends open. `axis_dir` need not be unit length. This is infallible for valid
inputs; a profile with fewer than two points or `segments == 0` yields an
empty mesh.

# Examples

```
use outram_blender::{revolve::revolve, fill_holes::fill_holes, math::Vec3};
use std::f64::consts::TAU;

// Revolve a vertical segment at radius 1 around Z into a 16-gon tube, then
// cap it into a closed prism.
let profile = [Vec3::new(1.0, 0.0, -1.0), Vec3::new(1.0, 0.0, 1.0)];
let tube = revolve(&profile, Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0), 16, TAU);
assert_eq!(tube.face_count(), 16);
assert_eq!(fill_holes(&tube).euler_characteristic(), 2);
```

```rust
pub fn revolve(profile: &[crate::math::Vec3], axis_point: crate::math::Vec3, axis_dir: crate::math::Vec3, segments: usize, angle: f64) -> crate::mesh::Mesh { /* ... */ }
```

## Module `selection`

Topological **element selection** — the state every Edit-Mode operator reads.

Blender analogue: `editors/mesh/editmesh_select.cc` plus the `BM_select_*` /
`BM_elem_flag` API. In Blender selection is a per-element flag on the mesh;
here it is a separate [`Selection`] value holding three sorted index sets
(selected vertices, edges, faces) and the current [`SelectMode`]. Keeping it
out of [`crate::mesh::Mesh`] means a mesh stays a pure geometry container and
an operator takes `(&Mesh, &Selection)` explicitly.

# What this module provides (GH issue #37 §A — `op-hzs.54.1`)

- **Select modes** — [`SelectMode::Vertex`] / [`SelectMode::Edge`] /
  [`SelectMode::Face`], with [`Selection::set_mode`] doing Blender's
  *selection flush*: switching to a coarser domain keeps only fully-selected
  elements, switching to a finer domain selects every sub-element.
- **Whole-mesh ops** — [`Selection::select_all`], [`Selection::deselect_all`],
  [`Selection::invert`].
- **Single element** — [`Selection::select`], [`Selection::deselect`],
  [`Selection::toggle`], [`Selection::is_selected`] over an [`Element`].
- **Region select** — [`Selection::select_in_box`],
  [`Selection::select_in_sphere`] (headless, model-space) and
  [`Selection::select_in_screen_polygon`] (the box / circle / lasso tools —
  the caller supplies the projection, so any orthographic or perspective
  camera works). Each takes a [`RegionMode`] deciding whether an edge/face
  needs *all* or just *any* of its vertices inside.
- **Select linked** — [`Selection::select_linked`] (grow to whole connected
  components) and [`Selection::select_linked_from`] (pick one component from
  a seed, Blender's `L`). Delimiters (seam / sharp / material) arrive with
  the per-edge attribute layers in `op-hzs.54.28`.
- **Select mirror** — [`Selection::select_mirror`]: for each selected element
  also select its mirror image across an [`Axis`] plane through the origin,
  matched by position within a tolerance.
- **Set algebra** — [`Selection::union`], [`Selection::subtract`],
  [`Selection::intersect`], [`Selection::retain`] compose the primitives
  above into Blender's extend / subtract / intersect box-select modes
  without widening the per-operator API.
- **Loop / ring** (`op-hzs.54.2`) — [`Selection::select_edge_loop`],
  [`Selection::select_edge_ring`], [`Selection::select_face_loop`],
  [`Selection::select_boundary_loop`], [`Selection::select_shortest_path`],
  over [`crate::topology`]'s adjacency walkers.
- **Grow / shrink / similar / nth** (`op-hzs.54.3`) —
  [`Selection::select_more`], [`Selection::select_less`],
  [`Selection::select_similar`] ([`SimilarTrait`]),
  [`Selection::checker_deselect`].
- **By trait** (`op-hzs.54.4`) — [`Selection::select_non_manifold`]
  ([`NonManifoldKinds`]), [`Selection::select_loose`],
  [`Selection::select_interior_faces`],
  [`Selection::select_faces_by_sides`] ([`NumberCompare`]). "Ungrouped
  vertices" waits on vertex groups in `op-hzs.54.28`.

```rust
pub mod selection { /* ... */ }
```

### Types

#### Enum `SelectMode`

Which element domain selection operations act on (Blender's Edit-Mode
vertex / edge / face select-mode buttons).

The active mode decides what [`Selection::select_all`],
[`Selection::invert`], and the region-select operators add, and what a bare
[`Element`] is expected to be. [`Selection::set_mode`] converts an existing
selection when the mode changes.

```rust
pub enum SelectMode {
    Vertex,
    Edge,
    Face,
}
```

##### Variants

###### `Vertex`

Individual vertices.

###### `Edge`

Whole edges (both endpoints).

###### `Face`

Whole faces (every corner).

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SelectMode { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SelectMode) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `Element`

One addressable mesh element, tagged by domain — the unit
[`Selection::select`] / [`Selection::deselect`] / [`Selection::toggle`] /
[`Selection::is_selected`] operate on.

```rust
pub enum Element {
    Vertex(crate::mesh::VertexId),
    Edge(crate::mesh::EdgeId),
    Face(crate::mesh::FaceId),
}
```

##### Variants

###### `Vertex`

A vertex by id.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::mesh::VertexId` |  |

###### `Edge`

An edge by id.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::mesh::EdgeId` |  |

###### `Face`

A face by id.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::mesh::FaceId` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Element { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Element) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `Axis`

A coordinate axis — names the reflection plane for [`Selection::select_mirror`]
(the plane through the origin **orthogonal** to this axis).

```rust
pub enum Axis {
    X,
    Y,
    Z,
}
```

##### Variants

###### `X`

The Y-Z plane (mirror negates X).

###### `Y`

The X-Z plane (mirror negates Y).

###### `Z`

The X-Y plane (mirror negates Z).

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Axis { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Axis) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `RegionMode`

Whether a multi-vertex element (edge or face) is caught by a region when
only *some* of its vertices lie inside.

Matches the two useful Blender box-select behaviours. Ignored when the
active [`SelectMode`] is [`SelectMode::Vertex`] (a vertex is simply in or
out).

```rust
pub enum RegionMode {
    Touching,
    Enclosed,
}
```

##### Variants

###### `Touching`

Select the element if **any** vertex is inside the region (Blender's
default — you can lasso part of a face).

###### `Enclosed`

Select the element only if **all** its vertices are inside the region
(a strict "fully enclosed" box select).

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> RegionMode { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &RegionMode) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `SimilarTrait`

A trait that [`Selection::select_similar`] matches on (Blender's
`Select ▸ Select Similar`, `Shift+G`). Each variant is meaningful in one
[`SelectMode`]; calling it in the wrong mode is a no-op.

Attribute-backed traits (material, crease, bevel weight, seam, sharp, vertex
groups) arrive with the per-element attribute layers in `op-hzs.54.28`.

```rust
pub enum SimilarTrait {
    VertexValence,
    EdgeLength,
    EdgeDirection,
    EdgeFaceCount,
    FaceArea,
    FaceSides,
    FacePerimeter,
    FaceNormal,
    FaceCoplanar,
}
```

##### Variants

###### `VertexValence`

**Vertex** — number of connecting edges (valence). Exact match;
threshold ignored.

###### `EdgeLength`

**Edge** — length. `threshold` is a relative tolerance
(`|a - ref| <= threshold * max(|ref|, eps)`).

###### `EdgeDirection`

**Edge** — direction (undirected). `threshold` is the angle tolerance
in radians.

###### `EdgeFaceCount`

**Edge** — number of incident faces (1 = boundary, 2 = manifold).
Exact match; threshold ignored.

###### `FaceArea`

**Face** — area. `threshold` is a relative tolerance.

###### `FaceSides`

**Face** — number of sides. Exact match; threshold ignored.

###### `FacePerimeter`

**Face** — perimeter. `threshold` is a relative tolerance.

###### `FaceNormal`

**Face** — normal direction. `threshold` is the angle tolerance in
radians.

###### `FaceCoplanar`

**Face** — coplanar with a selected face: normals parallel within
~0.5° **and** plane offset equal within `threshold` (absolute, model
units).

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SimilarTrait { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SimilarTrait) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `NonManifoldKinds`

Which non-manifold conditions [`Selection::select_non_manifold`] catches
(Blender's `Select ▸ All by Trait ▸ Non Manifold` checkboxes). All default
on via [`NonManifoldKinds::all`].

```rust
pub struct NonManifoldKinds {
    pub wire: bool,
    pub boundary: bool,
    pub multiple_faces: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `wire` | `bool` | Wire edges — edges with **no** incident face. |
| `boundary` | `bool` | Boundary edges — edges with **one** incident face (an open border). |
| `multiple_faces` | `bool` | Edges shared by **three or more** faces. |

##### Implementations

###### Methods

- ```rust
  pub fn all() -> Self { /* ... */ }
  ```
  Every condition enabled.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> NonManifoldKinds { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &NonManifoldKinds) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `NumberCompare`

How [`Selection::select_faces_by_sides`] compares a face's side count to the
target (Blender's `Select ▸ All by Trait ▸ Faces by Sides` "Type" dropdown).

```rust
pub enum NumberCompare {
    Equal,
    NotEqual,
    Less,
    Greater,
}
```

##### Variants

###### `Equal`

Side count `== n`.

###### `NotEqual`

Side count `!= n`.

###### `Less`

Side count `< n`.

###### `Greater`

Side count `> n`.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> NumberCompare { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &NumberCompare) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `Selection`

The set of currently-selected mesh elements plus the active [`SelectMode`].

Three sorted sets (vertices, edges, faces) are kept at all times; the active
mode is a *view* onto them, not a restriction — the region operators write
into the set matching the current mode, and [`Selection::set_mode`] rewrites
the sets so they stay mutually consistent (Blender's selection flush).

A `Selection` holds only indices, so it is `Clone` and carries no borrow of
the mesh it describes; pair it with the `&Mesh` explicitly at each call.
Indices are **not** validated against a mesh on construction — an operator
that mutates topology invalidates a `Selection` exactly as it would a raw
index, and should rebuild or remap it.

```rust
pub struct Selection {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(mode: SelectMode) -> Self { /* ... */ }
  ```
  An empty selection in `mode`.

- ```rust
  pub fn all(mesh: &Mesh, mode: SelectMode) -> Self { /* ... */ }
  ```
  Everything in `mesh` selected, in `mode` (and every domain flushed to

- ```rust
  pub fn mode(self: &Self) -> SelectMode { /* ... */ }
  ```
  The active select mode.

- ```rust
  pub fn selected_vertices(self: &Self) -> impl Iterator<Item = VertexId> + ''_ { /* ... */ }
  ```
  Currently-selected vertices, ascending by id.

- ```rust
  pub fn selected_edges(self: &Self) -> impl Iterator<Item = EdgeId> + ''_ { /* ... */ }
  ```
  Currently-selected edges, ascending by id.

- ```rust
  pub fn selected_faces(self: &Self) -> impl Iterator<Item = FaceId> + ''_ { /* ... */ }
  ```
  Currently-selected faces, ascending by id.

- ```rust
  pub fn vertex_count(self: &Self) -> usize { /* ... */ }
  ```
  Number of selected vertices.

- ```rust
  pub fn edge_count(self: &Self) -> usize { /* ... */ }
  ```
  Number of selected edges.

- ```rust
  pub fn face_count(self: &Self) -> usize { /* ... */ }
  ```
  Number of selected faces.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  `true` when no vertex, edge, or face is selected.

- ```rust
  pub fn is_selected(self: &Self, e: Element) -> bool { /* ... */ }
  ```
  Whether a specific element is selected (checks the set for its domain,

- ```rust
  pub fn select(self: &mut Self, mesh: &Mesh, e: Element) { /* ... */ }
  ```
  Select one element into the active mode's domain.

- ```rust
  pub fn deselect(self: &mut Self, mesh: &Mesh, e: Element) { /* ... */ }
  ```
  Deselect one element from the active mode's domain. Element-domain

- ```rust
  pub fn toggle(self: &mut Self, mesh: &Mesh, e: Element) { /* ... */ }
  ```
  Flip the selected state of one element.

- ```rust
  pub fn select_all(self: &mut Self, mesh: &Mesh) { /* ... */ }
  ```
  Select every element of `mesh` in the active mode (the other two domains

- ```rust
  pub fn deselect_all(self: &mut Self) { /* ... */ }
  ```
  Deselect everything (all three domains).

- ```rust
  pub fn invert(self: &mut Self, mesh: &Mesh) { /* ... */ }
  ```
  Invert the selection **in the active mode**: every element of that

- ```rust
  pub fn set_mode(self: &mut Self, mesh: &Mesh, mode: SelectMode) { /* ... */ }
  ```
  Change the active mode, rewriting the selection so the domains stay

- ```rust
  pub fn union(self: &mut Self, mesh: &Mesh, other: &Selection) { /* ... */ }
  ```
  Union another selection's active-domain set into this one, then re-sync.

- ```rust
  pub fn subtract(self: &mut Self, mesh: &Mesh, other: &Selection) { /* ... */ }
  ```
  Remove `other`'s active-domain elements from this selection, then

- ```rust
  pub fn intersect(self: &mut Self, mesh: &Mesh, other: &Selection) { /* ... */ }
  ```
  Keep only elements also present in `other`'s active-domain set, then

- ```rust
  pub fn retain</* synthetic */ impl FnMut(Element) -> bool: FnMut(Element) -> bool>(self: &mut Self, mesh: &Mesh, keep: impl FnMut(Element) -> bool) { /* ... */ }
  ```
  Keep only active-mode elements for which `keep` returns `true`, then

- ```rust
  pub fn select_in_box(self: &mut Self, mesh: &Mesh, min: Vec3, max: Vec3, region: RegionMode) { /* ... */ }
  ```
  Add to the selection every element of `mesh` inside the axis-aligned box

- ```rust
  pub fn select_in_sphere(self: &mut Self, mesh: &Mesh, center: Vec3, radius: f64, region: RegionMode) { /* ... */ }
  ```
  Add to the selection every element of `mesh` within `radius` of

- ```rust
  pub fn select_in_screen_polygon</* synthetic */ impl Fn(Vec3) -> [f64; 2]: Fn(Vec3) -> [f64; 2]>(self: &mut Self, mesh: &Mesh, project: impl Fn(Vec3) -> [f64; 2], polygon: &[[f64; 2]], region: RegionMode) { /* ... */ }
  ```
  Add to the selection every element of `mesh` whose projected position

- ```rust
  pub fn select_linked(self: &mut Self, mesh: &Mesh) { /* ... */ }
  ```
  Grow the selection so that every connected component containing at least

- ```rust
  pub fn select_linked_delimited(self: &mut Self, mesh: &Mesh, delimiters: &BTreeSet<EdgeId>) { /* ... */ }
  ```
  Like [`Selection::select_linked`] but the flood is over **face**

- ```rust
  pub fn select_linked_from(self: &mut Self, mesh: &Mesh, seed: Element) { /* ... */ }
  ```
  Select exactly the one connected component that contains `seed`

- ```rust
  pub fn select_edge_loop(self: &mut Self, mesh: &Mesh, seed: EdgeId) { /* ... */ }
  ```
  Select the **edge loop** through `seed` — Blender's `Alt`-click. In edge

- ```rust
  pub fn select_edge_ring(self: &mut Self, mesh: &Mesh, seed: EdgeId) { /* ... */ }
  ```
  Select the **edge ring** through `seed` — Blender's `Ctrl+Alt`-click.

- ```rust
  pub fn select_face_loop(self: &mut Self, mesh: &Mesh, seed: EdgeId) { /* ... */ }
  ```
  Select the **face loop** perpendicular to `seed` — the strip of quads

- ```rust
  pub fn select_boundary_loop(self: &mut Self, mesh: &Mesh, seed: EdgeId) { /* ... */ }
  ```
  Select the **boundary loop** that contains `seed` — the ring of open

- ```rust
  pub fn select_shortest_path(self: &mut Self, mesh: &Mesh, from: Element, to: Element) { /* ... */ }
  ```
  Select the **shortest path** between two elements (Blender's

- ```rust
  pub fn select_more(self: &mut Self, mesh: &Mesh) { /* ... */ }
  ```
  Grow the selection by one ring — add every element of the active domain

- ```rust
  pub fn select_less(self: &mut Self, mesh: &Mesh) { /* ... */ }
  ```
  Shrink the selection by one ring — remove every selected element of the

- ```rust
  pub fn select_similar(self: &mut Self, mesh: &Mesh, trait_: SimilarTrait, threshold: f64) { /* ... */ }
  ```
  Select every element of the active domain whose `trait_` value matches

- ```rust
  pub fn checker_deselect(self: &mut Self, mesh: &Mesh, selected: usize, deselected: usize, offset: usize) { /* ... */ }
  ```
  Thin out the selection to a regular pattern: over the selected elements

- ```rust
  pub fn select_non_manifold(self: &mut Self, mesh: &Mesh, kinds: NonManifoldKinds) { /* ... */ }
  ```
  Select the **non-manifold** geometry of `mesh` per `kinds` (Blender's

- ```rust
  pub fn select_loose(self: &mut Self, mesh: &Mesh) { /* ... */ }
  ```
  Select **loose geometry** — elements not connected to any face (Blender's

- ```rust
  pub fn select_interior_faces(self: &mut Self, mesh: &Mesh) { /* ... */ }
  ```
  Select **interior faces** — faces every edge of which is shared by three

- ```rust
  pub fn select_faces_by_sides(self: &mut Self, mesh: &Mesh, sides: usize, cmp: NumberCompare) { /* ... */ }
  ```
  Select faces whose side count compares to `sides` as `cmp` says

- ```rust
  pub fn select_mirror(self: &mut Self, mesh: &Mesh, axis: Axis, merge_dist: f64, extend: bool) { /* ... */ }
  ```
  For each currently-selected element, also select the element that is its

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Selection { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Selection) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `slide`

**Edge Slide** and **Vertex Slide** (`op-hzs.54.7`, GH issue #37 §B) — move
the vertices of an edge loop, or a single vertex, along their adjacent
**rail** edges. Topology is untouched; only positions change.

- [`edge_slide`] takes a connected chain / loop of edges and a signed
  `factor` in `[-1, 1]`. Each vertex on the chain has two rail edges (the
  loop-perpendicular edges); `factor > 0` slides toward the rail on one
  consistent side of the loop, `factor < 0` toward the other. `factor = ±1`
  collapses the loop onto the neighbouring loop.
- [`vertex_slide`] moves one vertex a fraction `factor` of the way along a
  chosen incident edge toward its far end.

The side used by [`edge_slide`] is fixed by propagating a "left face" along
the ordered chain, so the whole loop slides coherently. A vertex without
exactly two rails (a pole, a boundary end) is left where it is.

```rust
pub mod slide { /* ... */ }
```

### Functions

#### Function `edge_slide`

Slide the vertices of the edge chain `edges` along their rails by `factor`
(clamped to `[-1, 1]`). Returns a new mesh with the same topology and moved
positions. `edges` should be edge-connected (a loop or an open run); a
disconnected set slides each component with its own side propagation.

```rust
pub fn edge_slide(mesh: &crate::mesh::Mesh, edges: &[crate::mesh::EdgeId], factor: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `vertex_slide`

Move `vert` a fraction `factor` (clamped to `[0, 1]`) of the way along
`along_edge` toward its far end. Returns a new mesh; topology unchanged.

```rust
pub fn vertex_slide(mesh: &crate::mesh::Mesh, vert: crate::mesh::VertexId, along_edge: crate::mesh::EdgeId, factor: f64) -> crate::mesh::Mesh { /* ... */ }
```

## Module `snap`

**Snapping engine** (`op-hzs.54.24`, GH issue #37 §D — the precision/CAD
core). Depends on [`crate::transform_input`].

- [`SnapTarget`] — what to snap to: grid increment, vertex, edge midpoint,
  nearest point on an edge, nearest point on a face.
- [`SnapBase`] — which point of the moving selection is snapped: its
  closest vertex to the target, its bounding-box centre, its median, or a
  caller-nominated active vertex.
- [`snap_point`] — the nearest snap target to a query point.
- [`snap_translation`] — the delta that lands the base exactly on a target,
  or the raw delta if nothing is within `max_dist`.
- [`align_rotation_target`] — the surface normal at a face snap, so a caller
  can also orient the moved geometry (Blender's *Align Rotation to Target*).

```rust
pub mod snap { /* ... */ }
```

### Types

#### Enum `SnapTarget`

What a snap locks onto.

```rust
pub enum SnapTarget {
    Increment(f64),
    Vertex,
    EdgeMidpoint,
    EdgeNearest,
    FaceNearest,
}
```

##### Variants

###### `Increment`

Round each coordinate to a multiple of the given step (absolute grid).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `Vertex`

The nearest static vertex.

###### `EdgeMidpoint`

The nearest static edge's midpoint.

###### `EdgeNearest`

The nearest point lying on any static edge segment.

###### `FaceNearest`

The nearest point lying on any static face.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SnapTarget { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SnapTarget) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `SnapBase`

Which point of the moving selection is aligned to the snap target.

```rust
pub enum SnapBase {
    Closest,
    Center,
    Median,
    Active(crate::mesh::VertexId),
}
```

##### Variants

###### `Closest`

The moving vertex currently closest to the target.

###### `Center`

The centre of the moving selection's bounding box.

###### `Median`

The mean of the moving vertices.

###### `Active`

A caller-nominated vertex.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::mesh::VertexId` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SnapBase { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SnapBase) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `SnapHit`

A found snap location.

```rust
pub struct SnapHit {
    pub position: crate::math::Vec3,
    pub element: Option<SnapElement>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `position` | `crate::math::Vec3` | Where to snap to. |
| `element` | `Option<SnapElement>` | The element the snap landed on (`None` for a grid snap). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SnapHit { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `SnapElement`

Which mesh element a [`SnapHit`] is on.

```rust
pub enum SnapElement {
    Vertex(crate::mesh::VertexId),
    Edge(crate::mesh::EdgeId),
    Face(crate::mesh::FaceId),
}
```

##### Variants

###### `Vertex`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::mesh::VertexId` |  |

###### `Edge`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::mesh::EdgeId` |  |

###### `Face`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::mesh::FaceId` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SnapElement { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SnapElement) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `snap_point`

The nearest snap target to `query`. `exclude` vertices (and any element
using only excluded vertices) are ignored — pass the moving selection to
disable snap-onto-self. Returns `None` if nothing is within `max_dist`
(grid snaps always succeed).

```rust
pub fn snap_point(mesh: &crate::mesh::Mesh, query: crate::math::Vec3, target: SnapTarget, max_dist: f64, exclude: &[crate::mesh::VertexId]) -> Option<SnapHit> { /* ... */ }
```

#### Function `snap_translation`

The translation delta that snaps the [`SnapBase`] of `moving` (currently at
its position `+ raw_delta`) onto the nearest [`SnapTarget`] of the static
geometry. Falls back to `raw_delta` when nothing is within `max_dist`.

```rust
pub fn snap_translation(mesh: &crate::mesh::Mesh, moving: &[crate::mesh::VertexId], raw_delta: crate::math::Vec3, base: SnapBase, target: SnapTarget, max_dist: f64) -> crate::math::Vec3 { /* ... */ }
```

#### Function `align_rotation_target`

The unit normal at a face snap (Blender's *Align Rotation to Target*), or
`None` if the hit is not on a face.

```rust
pub fn align_rotation_target(mesh: &crate::mesh::Mesh, hit: &SnapHit) -> Option<crate::math::Vec3> { /* ... */ }
```

## Module `snap_line`

**Snap Utilities Line** (`op-hzs.54.43`, GH issue #37 §I) — place a
connected polyline with live snapping and numeric length/angle entry,
expressed as a headless staged operator.

- [`LineTool`] — the growing polyline. Add points [`LineTool::add_raw`],
  [`LineTool::add_snapped`] (through the [`crate::snap`] engine), or
  [`LineTool::add_polar`] / [`LineTool::add_constrained`] (numeric length
  and/or angle relative to a [`crate::draw_tool::WorkPlane`]).
- [`LineTool::undo`] / [`LineTool::close`].
- [`LineTool::commit_wire`] — append the polyline to a mesh as an edge
  wire.
- [`LineTool::auto_cut_chords`] + [`crate::knife::knife`] — cut faces the
  polyline crosses, for the tractable edge-to-edge-on-one-face case (the
  general surface-walking projection is deferred, as it is upstream in
  [`crate::knife`]).

## Units

Positions and lengths are dimensionless model-space quantities; angles are
radians.

```rust
pub mod snap_line { /* ... */ }
```

### Types

#### Struct `LinePoint`

One placed point of the polyline.

```rust
pub struct LinePoint {
    pub position: crate::math::Vec3,
    pub on: Option<crate::snap::SnapElement>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `position` | `crate::math::Vec3` | World position. |
| `on` | `Option<crate::snap::SnapElement>` | The mesh element it snapped to, if any. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> LinePoint { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &LinePoint) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `LineTool`

The connected-polyline drawing tool.

```rust
pub struct LineTool {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```
  A fresh, empty tool.

- ```rust
  pub fn points(self: &Self) -> &[LinePoint] { /* ... */ }
  ```
  The points placed so far, in order.

- ```rust
  pub fn is_closed(self: &Self) -> bool { /* ... */ }
  ```
  Whether [`LineTool::close`] has joined the ends.

- ```rust
  pub fn add_raw(self: &mut Self, position: Vec3) { /* ... */ }
  ```
  Append a raw world point (no snap).

- ```rust
  pub fn add_snapped(self: &mut Self, mesh: &Mesh, cursor: Vec3, target: SnapTarget, max_dist: f64) { /* ... */ }
  ```
  Snap `cursor` to the nearest `target` of `mesh` within `max_dist`, and

- ```rust
  pub fn add_polar(self: &mut Self, plane: &WorkPlane, length: f64, angle: f64) { /* ... */ }
  ```
  Append a point at `length` from the previous point, in the direction of

- ```rust
  pub fn add_constrained(self: &mut Self, mesh: &Mesh, cursor: Vec3, target: SnapTarget, max_dist: f64, plane: &WorkPlane, length: Option<f64>, angle: Option<f64>) { /* ... */ }
  ```
  Snap `cursor` as [`LineTool::add_snapped`] would, then optionally

- ```rust
  pub fn undo(self: &mut Self) { /* ... */ }
  ```
  Remove the last placed point.

- ```rust
  pub fn close(self: &mut Self) { /* ... */ }
  ```
  Join the last point back to the first (cyclic polyline).

- ```rust
  pub fn segment_count(self: &Self) -> usize { /* ... */ }
  ```
  Segment count (`points - 1`, or `points` when closed).

- ```rust
  pub fn commit_wire(self: &Self, base: &Mesh) -> Mesh { /* ... */ }
  ```
  Append the polyline to `base` as an edge wire (degenerate sliver

- ```rust
  pub fn auto_cut_chords(self: &Self, mesh: &Mesh) -> Vec<Chord> { /* ... */ }
  ```
  Knife chords for segments whose *both* endpoints snapped onto edges of

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> LineTool { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> LineTool { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `solidify`

Solidify — give a surface thickness by extruding it into a closed shell.

This is the pure-Rust analogue of Blender's **Solidify** modifier
(`MOD_solidify`, simple mode): every vertex is duplicated and offset inward
along its area-weighted vertex normal by `thickness`, the offset copy forms
an **inner shell** with reversed winding (so its normals face the cavity),
and any open boundary edge is bridged to the inner shell by a **rim quad**.

The result depends on whether the input is open or closed:

- An **open** surface (a grid, a patch, a surface with holes) becomes a
  **closed solid slab** of the given thickness — the use case for turning an
  authored boundary surface into something with volume (e.g. a CFD wall).
- A **closed** surface (a cube, a sphere) becomes a **hollow shell** — the
  original outer surface plus a nested, inward-offset inner surface, with a
  cavity between them.

# Winding

The outer shell keeps the input winding (outward normals). The inner shell
reverses it, so its normals point into the cavity (away from the solid
material). Each rim quad traverses the shared boundary edge opposite to the
outer face that owns it, keeping the whole result consistently wound — the
same adjacent-faces-oppose rule used in [`crate::fill_holes`].

# Degenerate normals

A vertex whose incident face normals cancel gets a zero normal (via
[`crate::math::Vec3::normalize`], which returns zero rather than `NaN`), so
its inner copy coincides with the outer vertex instead of flying off to a
non-finite coordinate. No `faer`, no external dependency; Android-safe.

```rust
pub mod solidify { /* ... */ }
```

### Functions

#### Function `solidify`

Extrude `mesh` into a closed shell of the given `thickness`, returning the
solidified mesh.

`thickness` is a length in mesh units; the inner shell is offset inward
(against the outward vertex normals) by this amount. A `thickness` of `0`
places the inner shell exactly on the outer one (a degenerate but valid
mesh). Open boundaries are closed with rim quads; a closed input yields a
hollow double shell.

This is infallible: the result is always a valid mesh.

# Examples

```
use outram_blender::{primitives, solidify::solidify};

// A flat grid (open disk, chi = 1) solidifies into a closed slab (chi = 2).
let grid = primitives::grid(2, 2, 2.0);
assert_eq!(grid.euler_characteristic(), 1);
let slab = solidify(&grid, 0.2);
assert_eq!(slab.euler_characteristic(), 2);
```

```rust
pub fn solidify(mesh: &crate::mesh::Mesh, thickness: f64) -> crate::mesh::Mesh { /* ... */ }
```

## Module `spin_screw`

**Spin / Screw** (`op-hzs.54.22`, GH issue #37 §C).

- [`spin`] — rotate-copy an ordered profile of vertices `steps` times over
  `angle` about an axis through `center`, bridging consecutive copies into a
  surface. `use_duplicates` places the copies without bridging.
- [`screw`] — [`spin`] plus a translation of `screw_offset` along the axis
  spread over the whole sweep, so `turns` revolutions trace a helix.

For an explicit polyline (rather than a mesh selection) use
[`crate::revolve`].

```rust
pub mod spin_screw { /* ... */ }
```

### Functions

#### Function `spin`

Rotate-copy `profile` (an ordered vertex chain) around the `axis` line
through `center`, `steps` times over `angle` radians. When `!use_duplicates`
the consecutive copies are bridged into quads. Returns the mesh with the new
geometry appended.

```rust
pub fn spin(mesh: &crate::mesh::Mesh, profile: &[crate::mesh::VertexId], center: crate::math::Vec3, axis: crate::selection::Axis, angle: f64, steps: usize, use_duplicates: bool) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `screw`

[`spin`] with an axial translation: the profile advances `screw_offset`
along the axis over `turns` full revolutions in `steps` steps, tracing a
helix. Always bridged.

```rust
pub fn screw(mesh: &crate::mesh::Mesh, profile: &[crate::mesh::VertexId], center: crate::math::Vec3, axis: crate::selection::Axis, screw_offset: f64, turns: f64, steps: usize) -> crate::mesh::Mesh { /* ... */ }
```

## Module `stl`

STL import / export — the lingua franca for surface-mesh interchange.

STL (STereoLithography) is an unstructured **triangle soup**: a flat list of
independent triangles, each with a facet normal and three vertices, with *no*
shared-vertex topology. It is the format most CAD/mesh tools and the DAGMC /
Monte-Carlo pipelines read and write, so it is the crate's interoperability
bridge alongside the OpenFOAM polyMesh and CSG exporters in
[`crate::export`].

# Writing

[`to_stl_ascii`] / [`to_stl_binary`] fan-triangulate every face and emit one
facet per triangle with an outward normal computed from the triangle
geometry (`(b − a) × (c − a)`, normalized). [`write_stl_ascii`] /
[`write_stl_binary`] are the disk counterparts. Binary is compact and exact
to `f32`; ASCII is human-readable and keeps full `f64` precision.

# Reading

[`from_stl_ascii`] / [`from_stl_binary`] parse a triangle soup into a
[`Mesh`]. Because STL has no shared vertices, the parsed mesh has one vertex
per triangle corner (a cube reads back as 36 vertices, 12 faces) — run
[`crate::weld::weld`] to merge coincident corners into a proper topological
mesh. [`from_stl_bytes`] auto-detects ASCII vs binary by the binary
size invariant, and [`read_stl`] does the same from a file.

No `faer`, no external dependency; Android-safe.

```rust
pub mod stl { /* ... */ }
```

### Types

#### Enum `StlError`

Errors from parsing an STL stream.

```rust
pub enum StlError {
    Truncated(usize),
    BadBinaryLength {
        got: usize,
        expected: usize,
    },
    Parse(String),
    Io(std::io::Error),
}
```

##### Variants

###### `Truncated`

The byte stream is too short to be a valid binary STL (needs at least
the 80-byte header plus the 4-byte triangle count).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

###### `BadBinaryLength`

A binary STL whose length does not match `84 + 50 * triangle_count`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `got` | `usize` | The actual byte length. |
| `expected` | `usize` | The length implied by the header's triangle count. |

###### `Parse`

An ASCII STL with a malformed `vertex` line or a truncated triangle.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Io`

A filesystem error while reading an STL file.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `std::io::Error` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Error**
  - ```rust
    fn source(self: &Self) -> ::core::option::Option<&dyn ::thiserror::__private18::Error + ''static> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(source: std::io::Error) -> Self { /* ... */ }
    ```

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `to_stl_ascii`

Fan-triangulate `mesh` and return it as an **ASCII** STL string.

Each triangle is written with an outward facet normal computed from its
geometry. Coordinates keep full `f64` precision (round-trippable). The solid
is named `outram_blender`.

# Examples

```
use outram_blender::{primitives, stl::to_stl_ascii};

let stl = to_stl_ascii(&primitives::cube(2.0));
// A cube's 6 quads fan-triangulate into 12 facets.
assert_eq!(stl.matches("facet normal").count(), 12);
```

```rust
pub fn to_stl_ascii(mesh: &crate::mesh::Mesh) -> String { /* ... */ }
```

#### Function `to_stl_binary`

Fan-triangulate `mesh` and return it as a **binary** STL byte buffer.

Layout: an 80-byte header, a little-endian `u32` triangle count, then 50
bytes per triangle (facet normal + three vertices as `f32`, plus a `u16`
attribute count of `0`). Coordinates are stored to `f32` precision.

```rust
pub fn to_stl_binary(mesh: &crate::mesh::Mesh) -> Vec<u8> { /* ... */ }
```

#### Function `write_stl_ascii`

Write `mesh` to `path` as ASCII STL.

```rust
pub fn write_stl_ascii(mesh: &crate::mesh::Mesh, path: &std::path::Path) -> std::io::Result<()> { /* ... */ }
```

#### Function `write_stl_binary`

Write `mesh` to `path` as binary STL.

```rust
pub fn write_stl_binary(mesh: &crate::mesh::Mesh, path: &std::path::Path) -> std::io::Result<()> { /* ... */ }
```

#### Function `from_stl_ascii`

Parse an **ASCII** STL string into a [`Mesh`] (an unwelded triangle soup).

Facet normals in the file are ignored (recomputed on any re-export). The
result has one vertex per triangle corner; [`crate::weld::weld`] it to
recover shared-vertex topology.

# Errors

[`StlError::Parse`] if a `vertex` line lacks three parseable coordinates or
the vertex count is not a multiple of three.

```rust
pub fn from_stl_ascii(text: &str) -> Result<crate::mesh::Mesh, StlError> { /* ... */ }
```

#### Function `from_stl_binary`

Parse a **binary** STL byte buffer into a [`Mesh`] (an unwelded triangle
soup).

# Errors

[`StlError::Truncated`] if shorter than the 84-byte minimum, or
[`StlError::BadBinaryLength`] if the length does not match the header's
triangle count.

```rust
pub fn from_stl_binary(bytes: &[u8]) -> Result<crate::mesh::Mesh, StlError> { /* ... */ }
```

#### Function `from_stl_bytes`

Parse an STL byte buffer, **auto-detecting** ASCII vs binary.

A stream is treated as binary when its length exactly matches the binary
size invariant `84 + 50 * count` (read from the header); otherwise it is
parsed as ASCII. This is the robust test — an ASCII file that merely starts
with the word `solid` will not accidentally satisfy the binary length.

```rust
pub fn from_stl_bytes(bytes: &[u8]) -> Result<crate::mesh::Mesh, StlError> { /* ... */ }
```

#### Function `read_stl`

Read an STL file from `path`, auto-detecting ASCII vs binary.

```rust
pub fn read_stl(path: &std::path::Path) -> Result<crate::mesh::Mesh, StlError> { /* ... */ }
```

## Module `subdivide`

**Subdivide** and **Un-Subdivide** (`op-hzs.54.8`, GH issue #37 §B).

[`subdivide`] cuts every face into `cuts + 1` pieces per side:

- a **quad** becomes a `(cuts+1) × (cuts+1)` grid of quads;
- a **triangle** becomes `(cuts+1)²` small triangles;
- an **n-gon** is fanned from its centroid, then each fan triangle is cut.

Edge points are shared between faces (deduplicated), so the result stays
watertight. [`SubdivideOptions`] adds:

- `smoothness` — blends new edge / interior points toward a Catmull-Clark-
  style smoothed position (0 = linear, 1 = full pull);
- `fractal` + `seed` — displaces each new vertex along the local face
  normal by `fractal · (rand − ½) · edge_len`, deterministically from
  `seed` (a small xorshift PRNG — no external crate, offline-reproducible).

[`un_subdivide`] is the partial inverse: it dissolves every other vertex of
a clean all-quad grid region, halving the resolution. It only acts where
the topology is a regular quad grid; elsewhere it is a no-op (Blender's is
similarly limited).

```rust
pub mod subdivide { /* ... */ }
```

### Types

#### Struct `SubdivideOptions`

Tuning for [`subdivide`].

```rust
pub struct SubdivideOptions {
    pub cuts: usize,
    pub smoothness: f64,
    pub fractal: f64,
    pub seed: u64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `cuts` | `usize` | Number of cuts per edge (`>= 1`). `1` = a single midpoint split. |
| `smoothness` | `f64` | Pull toward the smoothed surface, `0.0` (linear) … `1.0` (full). |
| `fractal` | `f64` | Fractal displacement amplitude along the face normal (`0.0` = none). |
| `seed` | `u64` | PRNG seed for the fractal displacement. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> SubdivideOptions { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `subdivide`

Subdivide every face of `mesh` per `opts`. `opts.cuts == 0` returns a clone.

```rust
pub fn subdivide(mesh: &crate::mesh::Mesh, opts: SubdivideOptions) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `un_subdivide`

Dissolve alternate rows/columns of a clean all-quad grid region, halving its
resolution. A no-op where the topology is not a regular quad grid.

`iterations` repeats the halving. Returns a new mesh.

```rust
pub fn un_subdivide(mesh: &crate::mesh::Mesh, iterations: u32) -> crate::mesh::Mesh { /* ... */ }
```

## Module `subdivision`

Catmull-Clark subdivision surface via **local stencils** (no global solve).

Catmull-Clark refinement takes any polygon mesh and produces a smoother,
denser, **all-quad** mesh that converges toward the Catmull-Clark limit
surface as the level increases. This module implements one round of
refinement with the classic three local point stencils and iterates it
`levels` times. Each new point is a fixed affine combination of a small
neighbourhood of the input mesh — there is no linear system to solve.

# The three stencils (one refinement level)

Working from the polygon soup ([`Mesh::positions`] + [`Mesh::polygons`]),
this builds its own undirected-edge adjacency (canonical key = the sorted
pair of endpoint vertex indices) recording, for each edge, its two endpoints
and the faces incident to it (1 incident face = a boundary/crease edge,
2 = interior).

- **Face point** `F_f` — the centroid (vertex average) of face `f`.
- **Edge point**:
  - *interior* edge (2 incident faces): `(v0 + v1 + F_a + F_b) / 4`, the
    average of the two endpoints and the two adjacent face points.
  - *boundary* edge (1 incident face): the endpoint midpoint `(v0 + v1) / 2`
    (crease rule — the boundary curve is refined but not pulled inward).
- **Vertex point** for an old vertex `P` of valence `n` (number of incident
  edges):
  - *interior* vertex (every incident edge interior):
    `(F_avg + 2*R_avg + (n - 3)*P) / n`, where `F_avg` is the average of the
    incident face points and `R_avg` is the average of the incident edge
    **midpoints** (the raw edge midpoints, not the edge points above).
  - *boundary* vertex (at least one incident boundary edge):
    `(m0 + 6*P + m1) / 8`, where `m0`, `m1` are the midpoints of the two
    incident boundary edges (crease rule). This keeps a straight boundary
    run exactly on its line and holds the boundary curve's shape fixed.

# New topology

Each old `n`-gon face emits exactly `n` quads. For every corner vertex `Pi`
of the face, with incoming edge `e_prev` and outgoing edge `e_next` (in the
old face's winding), one quad is emitted:
`[FacePoint_f, EdgePoint(e_prev), VertexPoint(Pi), EdgePoint(e_next)]`,
wound consistently with the old face. Every output face is therefore a quad.

All new points are deduplicated (one face point per face, one edge point per
undirected edge, one vertex point per old vertex), assigned contiguous
indices `[vertex points | edge points | face points]`, and rebuilt through
[`Mesh::from_polygons`] (which recomputes edge dedup and loop wiring).

# Valid inputs and boundary handling

Accepts **any** polygon mesh: triangles, quads, or higher `n`-gons; closed
(e.g. a cube) or open patches with a boundary (e.g. a grid). Boundary edges
and boundary vertices use the crease stencils above, so an open patch stays
a disc (`chi` unchanged) and its boundary loop keeps its shape.

# Limitations

- **Non-manifold edges (>2 incident faces) are not a validated case.** They
  do not panic: an edge with `k >= 2` incident faces uses the generalised
  interior edge point `(v0 + v1 + sum of the k face points) / (2 + k)`, which
  reduces to the standard `/4` rule at `k = 2`. A boundary vertex that does
  not have exactly two incident boundary edges (a dangling or non-manifold
  boundary) is held fixed as a safe fallback. Neither case is covered by the
  V&V tests below.

```rust
pub mod subdivision { /* ... */ }
```

### Functions

#### Function `catmull_clark`

Apply `levels` rounds of Catmull-Clark subdivision to `mesh`.

Returns a new, denser, all-quad [`Mesh`] converging toward the Catmull-Clark
limit surface. Each level applies the face-point / edge-point / vertex-point
stencils documented at the module level ([`crate::subdivision`]) once.

- `levels == 0` returns a plain clone of the input (identical topology and
  positions).
- `levels == 1` on an `n`-gon mesh yields one all-quad mesh where every old
  face has become `n` quads.

Accepts any polygon mesh (tris, quads, or higher `n`-gons; closed or open
with a boundary). Boundary edges and vertices are treated with the crease
stencils, so an open patch keeps its boundary shape and its Euler
characteristic. The output is always an all-quad mesh.

```rust
pub fn catmull_clark(mesh: &crate::mesh::Mesh, levels: u32) -> crate::mesh::Mesh { /* ... */ }
```

## Module `symmetry`

**Mesh symmetry** (`op-hzs.54.21`, GH issue #37 §C).

- [`symmetrize`] — keep one half of the mesh (the `keep_positive` side of
  the [`Axis`] plane through the origin), mirror it onto the other half, and
  weld the seam. Blender's `Mesh ▸ Symmetrize`.
- [`snap_to_symmetry`] — move every vertex to the average of its own
  position and its mirror partner's, so the mesh becomes exactly symmetric
  without changing topology. Blender's `Mesh ▸ Snap to Symmetry`.
- [`mirror_selection`] — the mirror-image vertices of a selection (the
  position-matched partners), for live / topology mirror editing.

```rust
pub mod symmetry { /* ... */ }
```

### Functions

#### Function `symmetrize`

Symmetrize `mesh` across the [`Axis`] plane through the origin. Keeps the
side where `axis · p >= 0` when `keep_positive`, else the `<= 0` side;
mirrors it onto the other side and welds vertices within `merge_threshold`
of the plane (and of each other on the seam).

```rust
pub fn symmetrize(mesh: &crate::mesh::Mesh, axis: Axis, keep_positive: bool, merge_threshold: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `snap_to_symmetry`

Make `mesh` exactly symmetric about the [`Axis`] plane without changing
topology: each vertex moves to the average of its position and the
position of the vertex nearest its mirror image (within `match_threshold`).
Unmatched vertices near the plane are snapped onto it.

```rust
pub fn snap_to_symmetry(mesh: &crate::mesh::Mesh, axis: Axis, match_threshold: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `mirror_selection`

The mirror-image vertices of `verts` across the [`Axis`] plane, matched by
position within `tolerance`. Unmatched inputs are dropped.

```rust
pub fn mirror_selection(mesh: &crate::mesh::Mesh, verts: &[crate::mesh::VertexId], axis: Axis, tolerance: f64) -> Vec<crate::mesh::VertexId> { /* ... */ }
```

### Re-exports

#### Re-export `Axis`

```rust
pub use crate::selection::Axis;
```

## Module `text`

**Text → geometry** (`op-hzs.54.38`, GH issue #37 §G) — lay glyph outlines
on a baseline, then fill / extrude / bevel them like a 2-D curve.

A [`Font`] maps a `char` to a [`Glyph`] (a list of closed `[x, y]` contours
on a `[0, 1]` em square, plus an advance width). [`Font::builtin_stroke`]
is a compact block font covering `A–Z`, `0–9`, space, `-` and `.` — enough
to letter parts and labels; supply your own [`Font`] for anything else.

- [`text_to_contours`] — the positioned, sized 2-D outlines.
- [`text_to_mesh`] — the outlines filled and, if `extrude > 0`, thickened
  into a solid with beveled front/back edges.

```rust
pub mod text { /* ... */ }
```

### Types

#### Struct `Glyph`

One glyph: closed contours on the `[0, 1]` em square, plus its advance.

```rust
pub struct Glyph {
    pub contours: Vec<Vec<[f64; 2]>>,
    pub advance: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `contours` | `Vec<Vec<[f64; 2]>>` | Closed polylines (the last point joins the first). |
| `advance` | `f64` | How far the pen advances after this glyph, in em units. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Glyph { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `Font`

A minimal font: `char` → [`Glyph`].

```rust
pub struct Font {
    pub glyphs: std::collections::HashMap<char, Glyph>,
    pub default_advance: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `glyphs` | `std::collections::HashMap<char, Glyph>` | Glyph table. |
| `default_advance` | `f64` | Advance for a `char` with no glyph (used for the space). |

##### Implementations

###### Methods

- ```rust
  pub fn glyph(self: &Self, c: char) -> Option<&Glyph> { /* ... */ }
  ```
  Look up a glyph, falling back to `None` (the caller draws nothing but

- ```rust
  pub fn builtin_stroke() -> Self { /* ... */ }
  ```
  A compact block stroke font — `A–Z`, `0–9`, space, `-`, `.` — each an

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Font { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Font { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `TextGeometry`

Options for [`text_to_mesh`].

```rust
pub struct TextGeometry {
    pub size: f64,
    pub tracking: f64,
    pub extrude: f64,
    pub bevel: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `size` | `f64` | Cap height (em → world scale). |
| `tracking` | `f64` | Extra advance between glyphs, in `size` units. |
| `extrude` | `f64` | Extrude depth along `+z` (`0` = a flat filled outline). |
| `bevel` | `f64` | Chamfer on the front/back edges (`0` = none). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> TextGeometry { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `text_to_contours`

The positioned, sized 2-D outlines for `text` — one contour list per glyph,
already offset along the baseline and scaled by `size`.

```rust
pub fn text_to_contours(text: &str, font: &Font, size: f64, tracking: f64) -> Vec<Vec<[f64; 2]>> { /* ... */ }
```

#### Function `text_to_mesh`

Build geometry for `text`: fill the outlines and, if `extrude > 0`, thicken
into a solid with (optional) beveled front/back edges.

```rust
pub fn text_to_mesh(text: &str, font: &Font, opts: &TextGeometry) -> crate::mesh::Mesh { /* ... */ }
```

## Module `topology`

Precomputed **mesh adjacency** — the queries [`crate::mesh::Mesh`] can only
answer by a scan, cached in flat `Vec`s so operators can walk topology in
`O(1)` per step.

[`crate::mesh::Mesh`] deliberately omits BMesh's radial cycle (all faces on
an edge) and disk cycle (all edges at a vertex) — see its module docs.
[`MeshTopology`] builds them once from the public [`crate::mesh::Mesh`] API:

- **edge → faces** ([`MeshTopology::edge_faces`]) — the radial cycle. 1
  face = boundary edge, 2 = manifold interior, >2 = non-manifold.
- **vertex → edges** ([`MeshTopology::vertex_edges`]) — the disk cycle
  (unordered here; ordering around the vertex is added when a consumer needs
  it).
- **vertex → faces** ([`MeshTopology::vertex_faces`]).
- the **quad opposite-edge** step ([`MeshTopology::opposite_edge_in_face`])
  and the loop/ring single steps ([`MeshTopology::edge_loop_step`],
  [`MeshTopology::edge_ring_step`]) that edge-loop / edge-ring / face-loop
  selection (`op-hzs.54.2`) and, later, loop cut (`op-hzs.54.5`) are built
  from.

Rebuild a [`MeshTopology`] after any operator that changes topology — like
any index into the mesh, it goes stale.

```rust
pub mod topology { /* ... */ }
```

### Types

#### Struct `MeshTopology`

Precomputed adjacency for one [`Mesh`] snapshot. Build with
[`MeshTopology::new`]; discard and rebuild after a topology edit.

```rust
pub struct MeshTopology {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: &Mesh) -> Self { /* ... */ }
  ```
  Build the adjacency tables for `mesh` (one pass over its edges and

- ```rust
  pub fn edge_faces(self: &Self, e: EdgeId) -> &[FaceId] { /* ... */ }
  ```
  Faces incident to edge `e` (its radial cycle). Empty for an out-of-range

- ```rust
  pub fn vertex_edges(self: &Self, v: VertexId) -> &[EdgeId] { /* ... */ }
  ```
  Edges incident to vertex `v` (its disk cycle, unordered).

- ```rust
  pub fn vertex_faces(self: &Self, v: VertexId) -> &[FaceId] { /* ... */ }
  ```
  Faces incident to vertex `v`.

- ```rust
  pub fn is_boundary_edge(self: &Self, e: EdgeId) -> bool { /* ... */ }
  ```
  `true` when `e` has exactly one incident face — a mesh-boundary (open)

- ```rust
  pub fn is_manifold_edge(self: &Self, e: EdgeId) -> bool { /* ... */ }
  ```
  `true` when `e` has exactly two incident faces — a manifold interior

- ```rust
  pub fn edge_between(self: &Self, a: VertexId, b: VertexId) -> Option<EdgeId> { /* ... */ }
  ```
  The id of the undirected edge between `a` and `b`, or `None`.

- ```rust
  pub fn other_end(self: &Self, mesh: &Mesh, e: EdgeId, v: VertexId) -> Option<VertexId> { /* ... */ }
  ```
  The other endpoint of `e` given one of them, or `None` if `v` is not on

- ```rust
  pub fn is_quad(self: &Self, mesh: &Mesh, f: FaceId) -> bool { /* ... */ }
  ```
  `true` when face `f` is a quadrilateral (four sides) — the case the

- ```rust
  pub fn face_edges(self: &Self, mesh: &Mesh, f: FaceId) -> Vec<EdgeId> { /* ... */ }
  ```
  The edges of face `f` in boundary order (one per consecutive vertex

- ```rust
  pub fn opposite_edge_in_face(self: &Self, mesh: &Mesh, f: FaceId, e: EdgeId) -> Option<EdgeId> { /* ... */ }
  ```
  The edge of quad `f` opposite `e` — the one two steps around the ring.

- ```rust
  pub fn edge_loop_step(self: &Self, edge: EdgeId, pivot: VertexId) -> Option<EdgeId> { /* ... */ }
  ```
  One step of an **edge loop** walk: from `edge`, pivoting about its

- ```rust
  pub fn edge_ring_step(self: &Self, mesh: &Mesh, edge: EdgeId, face: FaceId) -> Option<(EdgeId, Option<FaceId>)> { /* ... */ }
  ```
  One step of an **edge ring** walk: from `edge` across quad `face`, the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> MeshTopology { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `edge_loop`

The full **edge loop** through `seed` — the chain of edges that runs
"straight" across regular valence-4 vertices, or follows the mesh boundary
when `seed` is a boundary edge. Blender's `Alt`-click edge select.

The result always contains `seed`; it is unordered. A closed loop (around a
cylinder, say) terminates when the walk returns to `seed`; an open loop
terminates at the first pole / non-manifold vertex on each side.

```rust
pub fn edge_loop(topo: &MeshTopology, mesh: &crate::mesh::Mesh, seed: crate::mesh::EdgeId) -> Vec<crate::mesh::EdgeId> { /* ... */ }
```

#### Function `edge_ring`

The full **edge ring** through `seed` — the edges "parallel" to `seed`, one
per quad crossed as the walk steps to each quad's opposite edge. Blender's
`Ctrl+Alt`-click edge select. Always contains `seed`; unordered.

```rust
pub fn edge_ring(topo: &MeshTopology, mesh: &crate::mesh::Mesh, seed: crate::mesh::EdgeId) -> Vec<crate::mesh::EdgeId> { /* ... */ }
```

#### Function `face_loop`

The **face loop** perpendicular to `seed` — the strip of quads crossed by
the [`edge_ring`] walk (the faces, rather than their shared edges). Blender's
`Alt`-click face select. Unordered; contains every face incident to `seed`.

```rust
pub fn face_loop(topo: &MeshTopology, mesh: &crate::mesh::Mesh, seed: crate::mesh::EdgeId) -> Vec<crate::mesh::FaceId> { /* ... */ }
```

#### Function `boundary_loop`

The **boundary loop** containing `seed` — the ring of open (one-face) edges
around a hole or the outer border. Empty if `seed` is not a boundary edge.
(This is just [`edge_loop`] restricted to a boundary start, exposed
separately for intent.)

```rust
pub fn boundary_loop(topo: &MeshTopology, mesh: &crate::mesh::Mesh, seed: crate::mesh::EdgeId) -> Vec<crate::mesh::EdgeId> { /* ... */ }
```

#### Function `shortest_vertex_path`

A shortest **vertex path** from `from` to `to` along mesh edges, weighted by
edge length (Dijkstra). Returns the ordered vertex chain including both
ends, or an empty `Vec` if they are not connected. Blender's `Ctrl`-click
"Select Shortest Path" in vertex mode (geometry-distance flavour).

```rust
pub fn shortest_vertex_path(topo: &MeshTopology, mesh: &crate::mesh::Mesh, from: crate::mesh::VertexId, to: crate::mesh::VertexId) -> Vec<crate::mesh::VertexId> { /* ... */ }
```

#### Function `shortest_hop_path`

A shortest path (fewest hops) between two elements over a simple adjacency
graph — used for the edge-mode and face-mode "Select Shortest Path".
`adjacent(x)` yields the neighbours of `x`. Returns the ordered chain
including both ends, or empty if disconnected.

```rust
pub fn shortest_hop_path<T, F, I>(from: T, to: T, adjacent: F) -> Vec<T>
where
    T: Copy + Eq + std::hash::Hash,
    F: FnMut(T) -> I,
    I: IntoIterator<Item = T> { /* ... */ }
```

## Module `transform`

Affine transforms over mesh vertices — the **CPU reference path**.

An [`Affine3`] is a `3x3` linear map plus a translation, i.e. the standard
rigid/affine transform used to place, rotate, and scale mesh geometry
(Blender's `Object.matrix_world` is exactly this class of operation). It is
the "hello world" of an *embarrassingly parallel* mesh kernel: every vertex
is transformed independently, with no cross-vertex dependency, so the same
math maps cleanly onto a GPU compute shader.

## Why this lives in the always-compiled part of the crate

This module is compiled in **every** build, `gpu` feature or not. It is the
**trusted, deterministic reference**: the GPU path in [`crate::gpu`] exists
only to *accelerate* the exact same computation, and its result is checked
against [`Affine3::transform_points`] here. Per the workspace design rules,
the CPU path is what V&V and downstream solvers rely on; GPU output (f32,
non-deterministic reduction order across hardware) is never the source of
truth.

Precision: this CPU path is `f64`. The GPU compute shader is `f32` (the
portable storage type for WGSL). Agreement between the two is therefore an
*approximate* match within an `f32`-scale tolerance, not bit-exact — see the
GPU test in [`crate::gpu`].

```rust
pub mod transform { /* ... */ }
```

### Types

#### Struct `Affine3`

A 3D affine transform: a `3x3` linear map `M` followed by a translation `t`,
acting on a point `p` as `M p + t`.

The linear part [`Affine3::linear`] is stored **row-major** as three rows of
three `f64` components; `linear[i]` is row `i`, so the transformed
coordinate `i` is `dot(linear[i], p) + translation[i]`. All components are
dimensionless model-space quantities (see [`crate::math`]).

`Affine3` is `Copy` and holds no borrows, in line with the workspace
no-lifetimes rule.

```rust
pub struct Affine3 {
    pub linear: [[f64; 3]; 3],
    pub translation: crate::math::Vec3,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `linear` | `[[f64; 3]; 3]` | Row-major `3x3` linear part. `linear[i]` is the `i`-th output row; the<br>`i`-th transformed coordinate is `linear[i] . p + translation[i]`. |
| `translation` | `crate::math::Vec3` | Translation added after the linear map. |

##### Implementations

###### Methods

- ```rust
  pub const fn translation(t: Vec3) -> Affine3 { /* ... */ }
  ```
  A pure translation by `t` (identity linear part).

- ```rust
  pub const fn scale(sx: f64, sy: f64, sz: f64) -> Affine3 { /* ... */ }
  ```
  A non-uniform scale about the origin by `(sx, sy, sz)`, no translation.

- ```rust
  pub const fn from_rows(linear: [[f64; 3]; 3], translation: Vec3) -> Affine3 { /* ... */ }
  ```
  Build from an explicit row-major `3x3` linear part and a translation.

- ```rust
  pub fn transform_point(self: Self, p: Vec3) -> Vec3 { /* ... */ }
  ```
  Transform a single point `p` by this affine map, returning `M p + t`.

- ```rust
  pub fn transform_points(self: Self, positions: &[Vec3]) -> Vec<Vec3> { /* ... */ }
  ```
  Transform a whole slice of vertex positions on the CPU (the reference

- ```rust
  pub fn transform_points_best_effort(self: Self, positions: &[Vec3]) -> Vec<Vec3> { /* ... */ }
  ```
  Transform every position, **using the GPU as far as possible and falling

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Affine3 { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Affine3) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `transform_input`

**Numeric transform input + axis/plane constraints** (`op-hzs.54.23`, GH
issue #37 §D — the precision/CAD core).

The parameter model every transform operator ([`crate::transform_ops`], the
upcoming snapping engine, PDT) consumes:

- [`Constraint`] — free, locked to one axis, or locked to a plane.
- [`TransformBasis`] — three orthonormal vectors giving the coordinate space
  (global / local / normal / view). [`TransformBasis::global`] is the
  identity.
- [`NumericEntry`] — a per-component optional exact value, each parsed from
  a string with [`eval_expr`] (so `"1+1"`, `"pi/2"`, `"-tau"` all work).
- [`resolve_translation`] — combine a raw delta, a constraint, a basis,
  numeric overrides and a grid increment into the delta actually applied.
- [`apply_translation`] — move a vertex selection by a delta.

```rust
pub mod transform_input { /* ... */ }
```

### Types

#### Enum `Constraint`

Which components of a transform are free to move.

```rust
pub enum Constraint {
    Free,
    Axis(u8),
    Plane(u8),
}
```

##### Variants

###### `Free`

No constraint — all three components free.

###### `Axis`

Locked to one basis axis (`0 = X`, `1 = Y`, `2 = Z` of the basis).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `u8` |  |

###### `Plane`

Locked to the plane **orthogonal** to one basis axis (that component is
zeroed; the other two are free).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `u8` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Constraint { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Constraint) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `TransformBasis`

Three orthonormal basis vectors defining a transform space.

```rust
pub struct TransformBasis {
    pub x: crate::math::Vec3,
    pub y: crate::math::Vec3,
    pub z: crate::math::Vec3,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `crate::math::Vec3` |  |
| `y` | `crate::math::Vec3` |  |
| `z` | `crate::math::Vec3` |  |

##### Implementations

###### Methods

- ```rust
  pub fn global() -> Self { /* ... */ }
  ```
  The world axes — Blender's *Global* orientation.

- ```rust
  pub fn from_normal(normal: Vec3) -> Self { /* ... */ }
  ```
  A basis whose `z` is `normal` (Blender's *Normal* orientation), with

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> TransformBasis { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `NumericEntry`

Per-component optional exact value (already parsed). `None` = take the value
from the raw delta.

```rust
pub struct NumericEntry {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub z: Option<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `Option<f64>` |  |
| `y` | `Option<f64>` |  |
| `z` | `Option<f64>` |  |

##### Implementations

###### Methods

- ```rust
  pub fn parse(s: &str) -> Option<Self> { /* ... */ }
  ```
  Parse a `"x, y, z"` style string (each field optional, blank = `None`)

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> NumericEntry { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> NumericEntry { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `resolve_translation`

Resolve the delta actually applied.

1. Express `raw_delta` in `basis`.
2. Apply `constraint` (zero the locked components).
3. Override any component that has a [`NumericEntry`] value.
4. Snap each free component to a multiple of `increment` if `Some`.
5. Map back to world space.

```rust
pub fn resolve_translation(raw_delta: crate::math::Vec3, constraint: Constraint, basis: TransformBasis, numeric: NumericEntry, increment: Option<f64>) -> crate::math::Vec3 { /* ... */ }
```

#### Function `apply_translation`

Move `verts` (empty = whole mesh) by `delta`. Positions only.

```rust
pub fn apply_translation(mesh: &crate::mesh::Mesh, verts: &[crate::mesh::VertexId], delta: crate::math::Vec3) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `eval_expr`

Evaluate a small arithmetic expression to `f64`. Supports `+ - * / ^`,
parentheses, unary minus, and the constants `pi`, `tau`, `e`. Whitespace is
ignored. Returns `None` on any syntax error.

```rust
pub fn eval_expr(s: &str) -> Option<f64> { /* ... */ }
```

## Module `transform_ops`

**Edit-mode transform toolset** (`op-hzs.54.19`, GH issue #37 §C) —
parameterised versions of Blender's interactive transform tools. Every one
takes a `verts` subset (empty = whole mesh) and rewrites positions only.

- [`to_sphere`] — blend toward a sphere of `radius` about `center`.
- [`shear`] — offset along `shear_axis` proportional to the coordinate on
  `measure_axis`.
- [`bend`] — wrap the selection around an arc of `angle` about `center`.
- [`warp`] — Blender's Warp: bend around the 3D cursor in the view plane.
- [`push_pull`] — move each vertex toward / away from `center`.
- [`shrink_fatten`] — move each vertex along its averaged normal.
- [`randomize`] — deterministic per-vertex jitter.
- [`smooth_vertices`] — Laplacian smoothing with a per-axis mask.

```rust
pub mod transform_ops { /* ... */ }
```

### Types

#### Enum `Axis`

A coordinate axis for [`shear`] / [`bend`].

```rust
pub enum Axis {
    X,
    Y,
    Z,
}
```

##### Variants

###### `X`

###### `Y`

###### `Z`

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Axis { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Axis) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `to_sphere`

Blend the selection toward a sphere of `radius` about `center` by `factor`
(`0` = unchanged, `1` = fully on the sphere).

```rust
pub fn to_sphere(mesh: &crate::mesh::Mesh, verts: &[crate::mesh::VertexId], center: crate::math::Vec3, radius: f64, factor: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `shear`

Shear: shift each vertex along `shear_axis` by `factor · coord(measure_axis)`.

```rust
pub fn shear(mesh: &crate::mesh::Mesh, verts: &[crate::mesh::VertexId], measure_axis: Axis, shear_axis: Axis, factor: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `bend`

Bend the selection around an arc: a vertex at signed distance `t` along
`along` from `center` is rotated by `angle · t / span` about the axis
`axis`, where `span` is the selection's extent along `along`.

```rust
pub fn bend(mesh: &crate::mesh::Mesh, verts: &[crate::mesh::VertexId], center: crate::math::Vec3, along: Axis, axis: Axis, angle: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `warp`

Warp: bend the selection around `center` in the plane orthogonal to `axis`,
mapping its `along` extent onto an arc of `angle`.

```rust
pub fn warp(mesh: &crate::mesh::Mesh, verts: &[crate::mesh::VertexId], center: crate::math::Vec3, along: Axis, axis: Axis, angle: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `push_pull`

Push (`distance < 0`) or pull (`distance > 0`) each vertex along the ray
from `center`.

```rust
pub fn push_pull(mesh: &crate::mesh::Mesh, verts: &[crate::mesh::VertexId], center: crate::math::Vec3, distance: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `shrink_fatten`

Move each vertex `offset` along its averaged (incident-face) normal —
Blender's Shrink/Fatten (offset along normals).

```rust
pub fn shrink_fatten(mesh: &crate::mesh::Mesh, verts: &[crate::mesh::VertexId], offset: f64) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `randomize`

Deterministic per-vertex jitter of magnitude up to `amount`. `uniform`
gives the same displacement magnitude to every vertex (only the direction
varies); otherwise the magnitude is random too.

```rust
pub fn randomize(mesh: &crate::mesh::Mesh, verts: &[crate::mesh::VertexId], amount: f64, seed: u64, uniform: bool) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `smooth_vertices`

Laplacian-smooth the selection: `iterations` passes, each moving every
selected vertex a fraction `factor` toward the mean of its edge-neighbours.
`mask` disables movement on an axis when `false`.

```rust
pub fn smooth_vertices(mesh: &crate::mesh::Mesh, verts: &[crate::mesh::VertexId], iterations: u32, factor: f64, mask: [bool; 3]) -> crate::mesh::Mesh { /* ... */ }
```

## Module `triangulate`

Triangulate — convert every polygon face into triangles, returning a
triangle-only [`Mesh`].

This is the pure-Rust analogue of Blender's **Triangulate Faces**
(`bmo_triangulate` / `BM_face_triangulate`). A quad becomes two triangles
and an `n`-gon becomes `n − 2`, and the result is rebuilt as a [`Mesh`]
whose every face is a triangle.

# Concave faces: this used to be wrong

Until 2026-09-19 this module fanned every face from its first corner. That
is correct for a convex face and **incorrect for a concave one** — the fan
emits triangles that cross the reflex corner, leave the polygon, and
overlap each other, so the triangulated surface no longer bounds the same
solid. Measured on an L-shaped hexagon of true area 0.75, the fan totalled
1.000 (+33.3 %) from four of its six possible apex corners; see
[`crate::polyfill`]'s `concave_l_shape_is_tiled_exactly_where_a_fan_is_not`
for the full table. Since a face's corner order is just whatever the mesh
happens to store, that was firing on most concave faces.

N-gons now route through [`crate::polyfill`] (ear clipping, upstream's
`BLI_polyfill_calc`), which tiles exactly. The fan survives only as the
explicit [`QuadMethod::Fixed`] choice on quads, where it is exact.

# Methods

Upstream exposes two orthogonal choices, and so does this module:
[`QuadMethod`] for four-cornered faces and [`NgonMethod`] for the rest.
[`triangulate`] applies upstream's own defaults; [`triangulate_with`]
takes them explicitly.

# Why not [`crate::export::triangulate`]?

[`crate::export::triangulate`] produces an
[`crate::export::IndexedTriangles`] — a flat positions + `u32` index buffer
for a GPU or a solver import, **not** a [`Mesh`]. This operator instead
returns a first-class [`Mesh`] with half-edge topology, so downstream
mesh operators that assume or prefer triangles — [`crate::loop_subdivision`]
(Loop subdivision is only defined on triangle meshes),
[`crate::decimate`] (QEM edge collapse), and the CSG/polyMesh bridges — can
consume the result directly.

# Winding

Every method preserves each face's winding, so a consistently-wound,
outward-facing input stays that way. No `faer`, no external dependency;
Android-safe.

```rust
pub mod triangulate { /* ... */ }
```

### Types

#### Enum `QuadMethod`

How to split a four-cornered face into two triangles.

Mirrors upstream's `TriangulateModifierQuadMethod`
(`DNA_modifier_types.h:1927`). A quad has exactly two candidate diagonals;
every variant is a different rule for picking one. All are exact — a quad
is always tiled correctly by either diagonal *if* it is planar and convex;
for a non-planar or concave quad the choice changes the surface, which is
why upstream makes it a user decision rather than a constant.

```rust
pub enum QuadMethod {
    Beauty,
    Fixed,
    Alternate,
    ShortEdge,
    LongEdge,
}
```

##### Variants

###### `Beauty`

Pick the diagonal that gives the better-shaped triangle pair.
Upstream `MOD_TRIANGULATE_QUAD_BEAUTY`.

Decided exactly as upstream's `BM_face_triangulate` does: first
[`crate::polyfill_beautify::is_quad_flip_v3`] rejects a diagonal that
would fold the quad, and only if neither folds is the
area-over-perimeter measure
([`crate::polyfill_beautify::edge_rotate_cost_3d`]) consulted.
Unlike the length-based variants this one is aware of non-planarity.

###### `Fixed`

Always split corner 0 to corner 2 — the historical fan. Upstream
`MOD_TRIANGULATE_QUAD_FIXED`.

###### `Alternate`

Always split corner 1 to corner 3. Upstream
`MOD_TRIANGULATE_QUAD_ALTERNATE`.

###### `ShortEdge`

Split along the **shorter** diagonal. Upstream
`MOD_TRIANGULATE_QUAD_SHORTEDGE`, and upstream's default.

###### `LongEdge`

Split along the **longer** diagonal. Upstream
`MOD_TRIANGULATE_QUAD_LONGEDGE`.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> QuadMethod { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> QuadMethod { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(m: QuadMethod) -> Self { /* ... */ }
    ```

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &QuadMethod) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `NgonMethod`

How to split a face with five or more corners.

Mirrors upstream's `TriangulateModifierNgonMethod`
(`DNA_modifier_types.h:1921`).

```rust
pub enum NgonMethod {
    Beauty,
    EarClip,
}
```

##### Variants

###### `Beauty`

Ear-clip, then improve the result by rotating interior edges.
Upstream `MOD_TRIANGULATE_NGON_BEAUTY`, and upstream's default.

The rotation pass is [`crate::polyfill_beautify::polyfill_beautify`],
the port of `BLI_polyfill_beautify`. Both variants tile correctly;
this one additionally removes slivers (measured 5.8x improvement in
the worst triangle's fatness on a sliver-prone fixture — see that
module).

###### `EarClip`

Ear-clip only. Upstream `MOD_TRIANGULATE_NGON_EARCLIP`, via
[`crate::polyfill::polyfill_3d`].

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Boilerplate**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> NgonMethod { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> NgonMethod { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &NgonMethod) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `triangulate`

Triangulate every face of `mesh` using upstream's default methods.

Those defaults are [`QuadMethod::ShortEdge`] and [`NgonMethod::Beauty`]
(`DNA_modifier_types.h:1939-1940`). Positions are unchanged; only faces
are re-cut. A face with fewer than three corners is dropped (it is already
degenerate); a triangle is passed through unchanged. This is infallible.

# Examples

```
use outram_blender::{primitives, triangulate::triangulate};

// A cube's 6 quads split into 12 triangles; still χ = 2.
let cube = primitives::cube(2.0);
let tris = triangulate(&cube);
assert_eq!(tris.face_count(), 12);
assert_eq!(tris.euler_characteristic(), 2);
```

```rust
pub fn triangulate(mesh: &crate::mesh::Mesh) -> crate::mesh::Mesh { /* ... */ }
```

#### Function `triangulate_with`

Triangulate every face of `mesh`, choosing the quad and n-gon methods.

See [`QuadMethod`] and [`NgonMethod`]. Infallible.

# Examples

```
use outram_blender::{primitives, triangulate::{triangulate_with, QuadMethod, NgonMethod}};

let cube = primitives::cube(2.0);
// The historical fan, still available where it is exact.
let tris = triangulate_with(&cube, QuadMethod::Fixed, NgonMethod::EarClip);
assert_eq!(tris.face_count(), 12);
```

```rust
pub fn triangulate_with(mesh: &crate::mesh::Mesh, quad: QuadMethod, ngon: NgonMethod) -> crate::mesh::Mesh { /* ... */ }
```

## Module `weld`

Weld / remove-doubles — merge coincident vertices within a distance
tolerance into a single vertex.

This is the pure-Rust analogue of Blender's **Merge by Distance**
(`bmesh` `bmo_remove_doubles` / the Weld modifier `MOD_weld`): vertices
closer than `distance` collapse to one, faces are rebuilt on the merged
vertex set, and any face that loses too many distinct corners (fewer than
3) is dropped. It is the cleanup primitive that hardens a polygon soup — a
boolean result, an imported mesh, or an "exploded" face-varying surface —
into a watertight, shared-vertex mesh before it feeds the CSG / polyMesh
[`crate::export`] bridges.

# What "coincident" means

Two vertices are welded when the Euclidean distance between them is
`<= distance`. The merge relation is **not transitive** — `A`–`B` within
tolerance and `B`–`C` within tolerance does not imply `A`–`C` within
tolerance — so clusters are formed by **connectivity** (a union-find over
all within-tolerance pairs), not by pairwise distance from a single seed.
Every vertex in one connected cluster collapses to one output vertex placed
at the **average** of the cluster's positions (deterministic and
order-independent, matching the averaging convention used elsewhere in the
crate — Catmull-Clark, centroids).

# Algorithm (O(n) expected)

1. **`distance <= 0`** — weld only *exactly* coincident vertices, keyed on
   the `f64` bit pattern. A no-op when the mesh has no exact duplicates.
2. **`distance > 0`** — hash every vertex into a uniform grid of cell size
   `distance`. Because a within-tolerance partner can straddle a cell
   boundary, each vertex is tested against candidates in the **3×3×3 block
   of neighbour cells** (a cell size equal to the tolerance makes one cell
   of reach provably sufficient). Each accepted pair is `union`-ed.
3. Average each cluster's positions, rebuild faces on the merged set
   (dropping consecutive-duplicate corners and any face left with `< 3`
   distinct vertices), and compact so no isolated vertices survive.

No `faer`, no external dependency — just the fixed-size [`crate::math`]
types and standard collections. Android-safe.

```rust
pub mod weld { /* ... */ }
```

### Functions

#### Function `weld`

Merge vertices closer than `distance` into one, returning the welded mesh.

`distance` is a Euclidean length in the mesh's coordinate units. A value of
`0.0` (or negative) welds only bit-identical duplicate vertices, so it is a
safe no-op on a mesh that has none. Faces whose corners collapse to fewer
than three distinct vertices after welding are discarded; the winding order
of every surviving face is preserved (only vertex identities are
substituted).

This is infallible: the result is always a valid mesh (possibly with fewer
vertices and faces than the input).

# Examples

```
use outram_blender::{primitives, weld::weld};

// A pristine cube has no coincident vertices, so a zero-tolerance weld is a
// no-op and even a generous tolerance below the edge length changes nothing.
let cube = primitives::cube(2.0);
let welded = weld(&cube, 0.1);
assert_eq!(welded.vertex_count(), 8);
assert_eq!(welded.face_count(), 6);
assert_eq!(welded.euler_characteristic(), 2);
```

```rust
pub fn weld(mesh: &crate::mesh::Mesh, distance: f64) -> crate::mesh::Mesh { /* ... */ }
```

## Module `gpu`

**Attributes:**

- `Other("#[attr = CfgTrace([All([NameValue { name: \"feature\", value: Some(\"gpu\"), span: crates/outram-blender/src/lib.rs:316:11: 316:26 (#0) }, Not(NameValue { name: \"target_os\", value: Some(\"android\"), span: crates/outram-blender/src/lib.rs:316:32: 316:53 (#0) }, crates/outram-blender/src/lib.rs:316:31: 316:54 (#0))], crates/outram-blender/src/lib.rs:316:10: 316:55 (#0))])]")`

Headless GPU compute via `wgpu`. Behind the **default-on `gpu` feature**
(~~compiled unconditionally on every desktop target, no cargo feature~~
**CORRECTED 2026-10-02**, GitHub issue #486) and **absent on Android**
(`target_os = "android"`) whatever the feature says, since Android has no
system Vulkan/Metal loader and the workspace Android rule forbids GPU deps
in the library build. Whether or not this module is present, callers get
a graceful CPU fallback: with the feature off or on Android the GPU attempt
is compiled out entirely, and otherwise `gpu::probe` returning `None` or a
recoverable `gpu::GpuError` routes to the CPU reference path. See
[`transform::Affine3::transform_points_best_effort`] for the unified
try-GPU-then-CPU entry point, and the `gpu` module for the fallback
contract.
GPU compute (headless, target-gated OFF Android; behind the default-on `gpu`
feature since 2026-10-02 — ~~no cargo feature~~).

Headless GPU acceleration via [`wgpu`] for the *embarrassingly parallel*
parts of mesh authoring — per-vertex / per-face kernels, subdivision
evaluation, deformation. **No window or surface** is created; this is
compute-only (WGSL compute shaders).

## The wired demonstrator kernel

This module now carries **one real, end-to-end kernel**: applying an
[`crate::transform::Affine3`] to every vertex of a mesh in parallel via a
WGSL compute shader ([`crate::gpu::transform_vertices_gpu`]). The identical computation
on the CPU is [`crate::transform::Affine3::transform_points`], which is the
reference the GPU result is validated against (see the tests below). This
kernel is deliberately the simplest embarrassingly-parallel mesh operation —
it exists to prove the GPU compute path is live and CPU-checked, not because
an affine transform needs a GPU. Heavier per-vertex kernels (deformation,
subdivision) follow the same buffer/pipeline pattern.

## Non-negotiable contract for using this module

1. **Target-gated AND feature-gated.** ~~This module is compiled
   **unconditionally on every desktop target** — there is no `gpu` cargo
   feature to enable~~ **CORRECTED 2026-10-02** (GitHub issue #486): it is
   behind the **default-on `gpu` feature**, so a default build still has the
   GPU path and uses it as far as possible, and a dependent that does not
   want wgpu takes the crate with `default-features = false`. It is never
   present on **Android** (`target_os = "android"`), where the workspace
   Android rule forbids GPU deps in the library build. With the feature off
   or on Android the GPU attempt is compiled out and the CPU path runs.
2. **Runtime CPU fallback is mandatory.** Even where wgpu is compiled, at
   runtime there may be **no usable GPU adapter** (headless servers, VMs) or
   a submission may fail mid-flight. Callers MUST treat [`crate::gpu::probe`] returning
   `None`, and [`crate::gpu::try_transform_vertices_gpu`] returning `Err`, as "run the
   CPU path", never as a hard error. [`crate::transform::Affine3::transform_points_best_effort`]
   wraps exactly this: try GPU, fall back to CPU, always return a result.
3. **CPU is the trusted / reference path.** GPU float reduction order will
   not bit-match the CPU (`f64`, [`crate::transform`]) result, so anything
   that feeds V&V or a solver stays CPU-deterministic. GPU is *acceleration
   only*, and [`crate::gpu::transform_vertices_gpu`] returns `f32`-precision results.

```rust
pub mod gpu { /* ... */ }
```

### Types

#### Enum `GpuError`

A **recoverable** GPU execution failure from [`try_transform_vertices_gpu`].

Every variant means the same thing to a caller: the GPU attempt did not
complete, so fall back to the CPU reference path
([`Affine3::transform_points`]). The GPU is acceleration only and never the
source of truth, so a `GpuError` is a routine "use the CPU" signal, not a
fatal condition — [`Affine3::transform_points_best_effort`] does this
automatically. This deliberately does **not** cover the "no adapter at all"
case, which surfaces earlier as [`crate::gpu::probe`] returning `None`.

```rust
pub enum GpuError {
    Poll(String),
    Map(String),
    MapCallbackMissing,
}
```

##### Variants

###### `Poll`

Polling the device to drive the readback failed (e.g. device lost).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Map`

The staging buffer could not be mapped back to the CPU.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `MapCallbackMissing`

The buffer-map callback never fired despite a wait-indefinitely poll.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Error**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `GpuContext`

A live GPU compute context: a headless [`wgpu::Device`] and its
[`wgpu::Queue`].

Obtain one from [`probe`]. A `GpuContext` owns its device and queue by value
(no borrows — workspace no-lifetimes rule) and is `!Clone`; share it across
threads behind an `Arc` if needed. Dropping it releases the GPU resources.

```rust
pub struct GpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `device` | `wgpu::Device` | The logical GPU device — used to create buffers, shaders, and pipelines. |
| `queue` | `wgpu::Queue` | The command queue — used to upload buffers and submit compute work. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ByRef**
  - ```rust
    fn by_ref(self: &Self) -> &T { /* ... */ }
    ```

- **CastableFrom**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DistributionExt**
- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Imply**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
- **RefUnwindSafe**
- **Send**
- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **VZip**
  - ```rust
    fn vzip(self: Self) -> V { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `probe`

Probe for a usable headless GPU compute adapter.

Creates a [`wgpu::Instance`] over all backends enabled for this platform,
requests an adapter with **no surface** (headless compute — `power_preference
= None`, no `compatible_surface`), then requests a device + queue with the
downlevel default limits (the broadest-compatibility profile, so software
adapters like Lavapipe/WARP also qualify). The blocking wait on wgpu's async
requests is done with a tiny in-crate executor (`block_on`) so this crate
pulls in no async-runtime dependency.

Returns `Some(GpuContext)` when a headless compute device is available, or
`None` when the caller must fall back to the CPU path
([`crate::transform::Affine3::transform_points`]). `None` is a normal,
expected outcome on headless CI and the Android emulator — it is **not** an
error.

```rust
pub fn probe() -> Option<GpuContext> { /* ... */ }
```

#### Function `try_transform_vertices_gpu`

Apply `affine` to every position in `positions` on the GPU, returning the
transformed positions in the same order — the **fallible** entry point.

This is the demonstrator GPU kernel. It uploads the positions as an `f32`
storage buffer, dispatches `AFFINE_TRANSFORM_WGSL` one invocation per
vertex, and reads the result back. **Results are `f32` precision** — the
caller must treat them as an acceleration of, and approximation to,
[`Affine3::transform_points`] (the trusted `f64` CPU reference), not as a
bit-exact match.

An empty `positions` slice returns an empty `Vec` without touching the GPU.

# Errors

Returns [`GpuError`] if the submitted work cannot be completed (device lost
during the readback poll, or buffer-map failure). This is **recoverable**:
the caller should fall back to [`Affine3::transform_points`] — which
[`Affine3::transform_points_best_effort`] does automatically. The "no adapter
at all" case is handled earlier by [`crate::gpu::probe`] returning `None`, not here.

```rust
pub fn try_transform_vertices_gpu(ctx: &GpuContext, affine: crate::transform::Affine3, positions: &[crate::math::Vec3]) -> Result<Vec<crate::math::Vec3>, GpuError> { /* ... */ }
```

#### Function `transform_vertices_gpu`

Apply `affine` to every position on the GPU, panicking on failure — the
strict convenience wrapper over [`try_transform_vertices_gpu`].

Use this only when a GPU failure should abort (e.g. a benchmark that must run
on the GPU, or a test that has already confirmed an adapter via [`probe`]).
For normal use prefer [`Affine3::transform_points_best_effort`], which never
panics and falls back to the CPU. **Results are `f32` precision.**

# Panics

Panics if [`try_transform_vertices_gpu`] returns a [`GpuError`].

```rust
pub fn transform_vertices_gpu(ctx: &GpuContext, affine: crate::transform::Affine3, positions: &[crate::math::Vec3]) -> Vec<crate::math::Vec3> { /* ... */ }
```

### Re-exports

#### Re-export `wgpu`

Re-export of the GPU backend so callers can build pipelines without adding
their own `wgpu` dependency. Present wherever this module is: a non-Android
target with the default-on `gpu` feature.

```rust
pub use wgpu;
```

## Re-exports

### Re-export `faer`

Heavy linear-algebra backend for the *large* mesh solves the advanced
operators will need — Laplacian mesh editing, ARAP deformation, and mesh
parameterization all build a sparse Laplacian over the mesh and solve
`A x = b`. Re-exports [`faer`], a pure-Rust, Android-safe dense **and**
sparse linear-algebra library (SIMD via `pulp`, no system BLAS).

**Division of labour:** per-element geometry math (positions, normals,
transforms) stays in the fixed-size [`math`] types — small, fast, no
allocation. `faer` is only for the big systems. For interactive editing
(same matrix, many right-hand sides) prefer `faer`'s sparse **Cholesky**
factorization over an iterative solve. An *optional* bridge to
`outram-foam-basic-lib`'s CG/GAMG iterative solvers is tracked separately for
large one-off sparse solves (see beads `op-hzs`).

**First consumer:** [`laplacian`] — the cotangent/uniform discrete Laplacian
and implicit Laplacian smoothing assemble a sparse system and solve it with
`faer`'s sparse Cholesky. Future ARAP / parameterization operators reuse the
same path.

```rust
pub use faer;
```

