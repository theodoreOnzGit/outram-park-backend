# OpenFOAM's upper-triangular face order: V&V record (gh:#1285)

Research, education and V&V only (see `RESPONSIBLE_USE.md`).

## What changed and where

OpenFOAM's `polyMesh` requires the internal faces in **upper-triangular
order**: `owner < neighbour` on every internal face, and the internal faces
sorted by owner, then by neighbour (`polyMeshGenChecks::checkUpperTriangular`,
ported as `poly_mesh_gen_checks::check_upper_triangular`, gh:#1261). The
tet-dual pipeline and `cartesian_box` did not produce it (`cartesian_box`
numbers its internal faces by direction; the dual, layer and mapping
stages rebuild meshes from cell face lists).

The reorder is upstream's own: steps 2–3 of
`polyMeshGenModifier::renumberMesh` (`renumber_mesh.rs`, gh:#1272), split
out as `renumber_mesh::reorder_faces` and called with the **identity cell
order** as `renumber_mesh::upper_triangular`. Step 1 of `renumberMesh`
(the cell walk) is not applied: cell labels stay as the pipeline made
them, so nothing indexed by cell changes meaning.

**Where.** One final step, not in each builder:

- `pipeline::run_pipeline` (both `surface_to_tet_dual_mesh` entry points,
  and the box / sphere / cylinder wrappers), after the patch naming
  (stage 8) and before the final gate;
- `cartesian::cartesian_box`, at its end (it is a public builder that does
  not go through the pipeline).

Why one final step: every stage of the tet-dual pipeline after the carve
rebuilds its mesh from cell face lists (`from_cell_faces`), so an order
imposed inside a builder is lost at the next stage; and the stage set
changes with the options (dual kinds, mapping, `optimiseFinalMesh`, three
layer strategies, patch naming). A final step guarantees the order on every
option combination with one call, costs one O(F) pass, and leaves every
stage's internals (and their own unit tests) as they were. The four
upstream workflows (`tet_mesh`, `p_mesh`, `cartesian_mesh`,
`cartesian_2d_mesh`) already end with the full `renumberMesh` and are not
touched.

## Predictions (written 2026-11-05, before the change was run)

1. **Geometry is unchanged.** Points are not touched. Boundary faces keep
   their order, ring and patch, so the **total volume** (the divergence sum
   over boundary faces) is **bit for bit** the same. Cell labels are kept,
   so the cell volumes can be compared **cell by cell**, not only as a
   multiset; face areas as a multiset.
2. Refinement of 1, stated before measuring: a reversed internal face
   (owner > neighbour before the reorder) has its ring reversed after its
   first point, so Newell's sum runs in another order, and each cell's
   volume sums its faces in another order. Expected: cell volumes and face
   areas equal **to round-off** (relative difference below 1e-12, most of
   them bit-identical), not necessarily bit for bit.
3. `check_upper_triangular` fails on the pre-change default outputs (pipe
   h 4/5/6 mm, Foam Studio's default mesh) and passes on every output after
   it. `checkMesh`'s other topology checks are unchanged.
4. The pinned fingerprints that hash face order move:
   `back_off_only_layers_are_pinned_bit_for_bit`,
   `dual_kinds_are_pinned_bit_for_bit`, `octree_path_is_pinned_bit_for_bit`,
   `layer_strategies_are_deterministic`,
   `foam_studio_default_mesh_keeps_its_fingerprint`, and the Foam Studio
   example pins; cell counts do not move.
   `correct_edges_is_pinned_bit_for_bit` hashes a stage output built on
   `cartesian_box`, so it moves too.
5. The Foam Studio solve on the default mesh: the FV operators sum face
   fluxes into cells in face order, so a reorder changes the summation order
   and the result moves **at round-off only**: relative change in the pinned
   pressure drop / velocity below 1e-10.

## Methodology (as run)

- `pipeline::tests::upper_triangular_order_moves_no_geometry` runs the
  pipeline twice on the same input, without and with the final step (the
  private `run_pipeline` flag), on the demo pipe (R 10 mm, L 200 mm, 48
  facets): h 4 / 5 / 6 mm bare, h 5 mm with 3 back-off layers, and h 5 mm
  `CellCentre` + `Corners` bare. It compares the points, the total volume
  (bits), each cell's volume (`checks::cell_geometry`; labels are kept, so
  cell by cell), the face-area multiset, the patch ranges and every
  boundary face, `checkUpperTriangular` before and after, and whether the
  new internal faces are `outram_blender`'s `to_poly_mesh` stable sort of
  the old ones (the sort Foam Studio's solver mesh goes through).
- The pins were re-run after the change and after merging
  `claude/outram-foam-8ookor` at `a593a5b84` (PETIR routing of the
  transcendental calls); the post-merge values equal the pre-merge ones.
- `cargo test --release -p outram-park-fork-cfmesh --features cell-centre-dual --lib`.

## Results (2026-11-05)

| case | cells | internal faces | upper-triangular before / after | faces reversed | cell volumes bit-identical | worst rel. cell-volume difference | face areas bit-identical | `to_poly_mesh` sort of the old = new |
|---|---|---|---|---|---|---|---|---|
| h 4 bare (Foam Studio's pre-gh:#1298 mesh) | 6 353 | 33 636 | no / yes | 0 | 6 353 / 6 353 | 0 | 46 140 / 46 140 | yes |
| h 5 bare | 3 113 | 15 960 | no / yes | 0 | 3 113 / 3 113 | 0 | 23 928 / 23 928 | yes |
| h 6 bare | 1 939 | 9 894 | no / yes | 0 | 1 939 / 1 939 | 0 | 14 862 / 14 862 | yes |
| h 5, 3 back-off layers | 26 009 | 84 480 | no / yes | 0 | 22 613 / 26 009 | 4.1e-16 | 92 784 / 92 784 | yes |
| h 5 `CellCentre` + `Corners` bare | 3 113 | 15 960 | no / yes | 0 | 1 787 / 3 113 | 5.2e-16 | 17 386 / 17 386 | yes |

Total volume bit for bit in every case; points, patches and boundary faces
identical.

**Against the predictions.**

1. Geometry unchanged, total volume bit for bit: **held**.
2. Round-off only: **held**, and tighter than stated. No face was
   reversed (every builder already set `owner < neighbour`; only the
   order failed), so every face area is bit-identical. Cell volumes differ
   in the last bit (≤ 5.2e-16 relative) on the layered and cell-centre
   meshes, where a cell's faces are now summed in another order; on the
   bare median duals they are bit-identical.
3. `checkUpperTriangular` fails before and passes after on every case:
   **held**. `cartesian_box` passes as built
   (`poly_mesh_gen_checks::tests::upper_triangular_detects_a_swap` now
   asserts it).
4. Pins that hash face order moved, cell counts did not: **held**. Old →
   new (struck through in the test sources):

   | pin | old | new |
   |---|---|---|
   | `back_off_only_layers_are_pinned_bit_for_bit`, pipe h 6 n 1/2/3 | 7691211397797103741, 6777344945964783458, 8963858472090204797 | 6256401714239669657, 7331035643881703750, 18337414743125392665 |
   | same, pipe h 4 n 1/2/3 | 11354576510400452468, 6008187938627271744, 13850840632559401666 | 12495095278579800432, 10932642480450119360, 13769222349618449910 |
   | same, box duct h 5 n 2 | 7672853737214871962 | 15315517425586397770 |
   | `dual_kinds_are_pinned_bit_for_bit` (corner; point/edge; centre/edge; point/corners; centre/corners) | 16736591880388189566, 5331245050290215594, 1937584253539663578, 3093209209138859925, 10005039613933009028 | 12398942909958749874, 11035141598418972758, 5798262695090360502, 6679129286926630001, 2459340814925991440 |
   | `octree_path_is_pinned_bit_for_bit` (octree mesh; pipe) | 16070505555120789497; 24335298658873933 | **16070505555120789497 (not moved: not a pipeline output)**; 2018287953755369937 |
   | `layer_strategies_are_deterministic` (CfMeshOptimise; SnappyCollapse) | 11521056484853145326; 10164118906460112537 | 5422758738504758818; 18444736245533413537 |
   | `correct_edges_is_pinned_bit_for_bit` (counts 1727, 8, 16, 112, 14 unchanged) | 714198749002929189 | 7179125119703641544 |
   | `foam_studio_default_mesh_keeps_its_fingerprint` (6 353 cells) | 18084991279620725399 | 13281953444677737411 |

   `anisotropic::tests::generators_without_anisotropic_sources_are_pinned_bit_for_bit`
   (the four upstream workflows, which already ran `renumberMesh`) and
   `prism_refinement_is_pinned_bit_for_bit` did not move, as they should not.
5. Foam Studio's solve moves at round-off only: **held, and exactly**. The
   solver's polyMesh is unchanged face for face: `outram_blender`'s
   `to_poly_mesh` already stable-sorted the internal faces by (owner,
   neighbour), and on every case above that sort of the old order is the
   new order (last column), so the mesh the solver gets, and with it the
   solve, is the same to the bit. The Foam Studio pins
   (`headless_unstructured_pipe_runs_and_is_pinned`: u_c/2U 0.8925,
   dp/dz 1.0326) pass unchanged (run below).

**A test whose case moved: `final_mesh::tests::projected_untangle_keeps_the_inlet_planar`.**
Its input shakes every third interior point **in face order**, so the new
order shook other points. On the new shake `EachPass` still keeps the
inlet planar (asserted), but `AfterUntangle` leaves a rim point (r 9.967
mm) 1.78e-4 m off the inlet plane: the boundary passes slid it 1.9 mm up
the wall, and the one projection at the end does not bring it back onto
the rim. The recorded case is kept (its `small_pipe` mesh is built in the
pre-gh:#1285 order) and asserted as before; the new shake is asserted
both ways (`EachPass` planar, `AfterUntangle` off by more than 1e-5 m, so
a change is seen). Note: on the recorded shake the plain untangle never
took the inlet off the plane (0 m), so `AfterUntangle`'s pass there tested
little; the new shake is the harder case and the one it fails. This is a
limit of the gh:#1283 `AfterUntangle` mode (not the default, not used by
gh:#1298's route, which uses `EachPass`).

## Images

`face_order_h4_before.png`, `face_order_h4_after.png` (written by
`pipeline::tests::render_gh1285_face_order`, `--ignored`): every internal
face of the h 4 mm pipe as a dot at (face index, owner cell), red where
`owner > neighbour`. No slice of the mesh is drawn: the geometry is
identical (checked numerically above), so a slice would be the same
picture twice.

**Checked by eye (2026-11-05):** `before` shows several separate sweeps
through the owner labels (the internal faces visit the cells more than
once, the dual's own face order); `after` is one monotone rising curve
(owner never decreases with the face index). No red dot in either: no
internal face had `owner > neighbour`, matching the 0 reversed faces in
the table. Recompressed losslessly (zlib 9, pixels verified identical).
**Not checked by image:** the other cases of the table (numerically only).

## Tests run

- `cargo test --release -p outram-park-fork-cfmesh --features cell-centre-dual --lib`:
  239 passed, 0 failed, 25 ignored (2026-11-05, after the re-pins).
- `cargo test --release -p dhoby-ghaut --example foam_studio_web`: 18
  passed, 7 ignored (diagnostics); `headless_unstructured_pipe_runs_and_is_pinned`
  passes with its 0.8925 / 1.0326 pin.
- `cargo test --release -p outram-blender --features foam-mesh --lib foam_mesh`:
  14 passed.
