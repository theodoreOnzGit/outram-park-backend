# blockMesh topological merge and upstream numbering (gh:#1464)

Upstream: OpenFOAM-dev `852527377c7e` (`blockMeshMerge.C`, `blockMeshCreate.C`,
`blockCreate.C`, `polyMeshFromShapeMesh.C`, `cellMatcher/*`, `cellShape`).
Code: `src/block_topology.rs`, `BlockMeshDict::build` in `src/block_mesh.rs`.
No OpenFOAM binary is available in this environment, so no byte comparison
against upstream's own `polyMesh` was run; the checks below are by derivation
from the upstream source and by invariants.

## Predictions (committed before any run of the new code)

Baseline (old code, recorded 2026-11-11 with `examples/dump_block_mesh.rs`):

| dict | points | faces | internal | cells | volume |
|---|---|---|---|---|---|
| cavity | 882 | 1640 | 760 | 400 | 1.0e-4 |
| pitzDailySteady | 25012 | 49180 | 24170 | 12225 | 1.451604e-5 |
| T3A | 54568 | 107745 | 53175 | 26820 | 0.303775012 |

1. **Geometry unchanged.** Counts, patch sizes, volume and the numbering-
   independent face-set hash (`dump_block_mesh`, coordinates rounded to 1e-9)
   equal the baseline for all three. Points move by at most a few ulp (the
   12-edge blend replaces the trilinear map; `lineDivide` replaces
   `graded_positions`).
2. **Numbering changes, and is derivable.** For the single-block cavity:
   point `n = i + 21 j + 441 k`; `faces[0] = [1 22 463 442]` with owner 0 and
   neighbour 1; ~~`faces[1] = [2 23 464 443]`~~ (MISS, see Results); 760 internal faces. (A regular hex
   is re-labelled by the collapse step to `[0 3 7 4 1 2 6 5]`, so cell 0's
   face shared with cell 1 is the model's z-max face, which yields exactly that
   winding.)
3. **Merge classes equal the old geometric merge** on these dicts (all block
   interfaces conforming), so point counts are unchanged.
4. **Solver outputs** that depend on the mesh (pipe Hagen-Poiseuille, cavity
   steady state) move by round-off only, because only the summation order of
   face sums changes.

## Results (2026-11-11)

Code: commits on `claude/outram-foam-8ookor`, `src/block_topology.rs`,
`BlockMeshDict::build`. Tests: `tests/block_mesh_upstream_numbering.rs` (6
tests), `block_topology::tests` (6 tests), the existing 48 + 10 + 14 tests,
all green (`cargo test --release -p outram-foam-mesh --lib --test block_mesh
--test block_mesh_curved_edges --test block_mesh_upstream_numbering`).

### Against a real upstream run (this is the byte comparison)

An upstream `blockMesh` was available late in the work: OpenFOAM-**v2506**
(ESI, partial build under `/home/user/upstream-build`, `libblockMesh` plus the
application built with `wmake`, `WM_MPLIB=dummy`, `writePrecision 17`). v2506
is not the commit the port follows (OpenFOAM-dev `852527377c7e`); that the two
agree on the structure compared here is itself a result.
`examples/compare_polymesh.rs` reads the upstream `constant/polyMesh` and
compares it with our in-memory mesh:

| case | points | faces | owner | neighbour | boundary |
|---|---|---|---|---|---|
| cavity (dev tutorial) | 882 / 882 **bit-identical** | 1640 / 1640 identical loops | 1640 equal | 760 equal | identical |
| T3A (8 arcs) | 49965 / 54496 bit-identical; worst abs diff 1.1e-16 | 107707 / 107707 identical loops | 107707 equal | 53213 equal | identical |
| pitzDailySteady (`units [mm]` replaced by `convertToMeters 1`, see below) | 24797 / 25012 bit-identical; worst abs diff 5.7e-14 on values up to ~290 (2e-16 relative) | 49180 / 49180 identical loops | 49180 equal | 24170 equal | identical |

So **point, face, owner, neighbour and patch numbering match upstream exactly
on all three**, and every point agrees to about 1 ulp or better. The
non-bit-identical points (T3A 8.3 %, pitz 0.9 %) are all within that; the
cause is not isolated here. Candidates: the platform `pow` upstream calls in
`lineDivide` against `petir`'s (this crate must not call the platform libm),
and operation order inside the blend. Both are round-off, neither is a
numbering matter. Under the 1e-8 allowance with this cause stated, flagged for
the maintainer.

Caveat on pitz: ESI v2506 silently ignores the `units [mm];` entry that dev
reads (it falls into an unknown-entry skip, the very bug fixed in gh:#1463), so
the upstream run meshes in mm. The comparison therefore used the same dict with
`units [mm];` replaced by `convertToMeters 1;`. T3A and cavity carry no units
entry. This is a difference between the two upstream versions, not between our
port and dev.

### Predictions against results

1. **Geometry unchanged: held for cavity and pitzDailySteady, MISSED for T3A.**
   cavity and pitz: counts, volume, face-set hash identical to the baseline.
   T3A: points 54568 -> **54496** (72 fewer), internal faces 53175 -> **53213**
   (38 more), `defaultFaces` 53716 -> 53640, hash changed; volume identical
   (0.3037750120827). Cause: the old coordinate merge (tolerance 1e-9 of the
   diagonal) did not merge block-interface points that the curved-edge blend
   places up to **1.7e-8** apart (measured, raw dict units); 38 interior faces
   were left as pairs of boundary faces in the default patch. Upstream's
   topological merge pairs them (`sqrMergeTol` = a tenth of the squared face
   spacing). The old T3A mesh had these 38 cracks; the new one matches the
   upstream run exactly (above). A defect found and fixed, not a regression.
2. **Numbering: held, with one wrong sub-prediction.** Points `n = i + 21 j +
   441 k`, `faces[0] = [1 22 463 442]` owner 0 nbr 1, 760 internal faces: held
   (`cavity_numbering_follows_upstream`, and the upstream run). `faces[1]`: I
   predicted `[2 23 464 443]`; wrong, because cell 0's second neighbour
   (cell 20) comes before cell 1's faces. Actual and upstream: `[21 462 463
   22]`, owner 0, neighbour 20; `[2 23 464 443]` is `faces[2]` (owner 1,
   neighbour 2).
3. **Merge classes equal the old geometric merge: held** for cavity and pitz,
   **missed** for T3A (point 1).
4. **Solver outputs move by round-off only**: see "Moved numbers".

### Things this change broke or exposed, and what was done

- **`block_primitives::PipeOGrid` emitted a topologically non-conforming
  dict**: one n x n central block against n outer blocks per side. Upstream
  merges only across shared block faces, so the interface stayed unmerged (212
  section cells instead of 132; a `defaultFaces` patch). The generator now
  emits the central square as n x n single-cell-wide blocks sharing vertices
  with the ring (`images/pipe_ogrid_split_central.png`, n = 4, 3 radial cells:
  conforming, circle patch in green). The mesh is geometrically the same
  (interior vertices are the old bilinear positions); cell order differs.
- `section::plane_section` now treats a point within 1e-12 of the extent of
  the plane as on it. The 12-edge blend puts nodes at `0.1 +- ulp`, and a test
  that passes a plane exactly through a layer of nodes split the layer.
- The `(4 7 6 5)` frontAndBack winding of `tests/blockMeshDict.cavity` was
  suspected (a mis-wound face) and is accepted by upstream: its "points
  inwards" check iterates the mesh's own patch faces, not the dictionary quads,
  so it cannot fire on a mis-wound dictionary face. Ported as upstream does it;
  fixture left as it was.

### Moved numbers

- `outram-foam-appbuilder-lib` `laminar_pipe_hagen_poiseuille` (5280-cell
  O-grid, both rows): the baseline (commit a0c58ef7b) and the new code print
  identical `dpdz/HP` 1.03160 / 1.01768, `uc15` 0.9902 / 0.9902, `flux`,
  `cont` 1.4e-11 / 5.7e-12, same step counts (19, 22). Agreement to the printed
  digits (<= 5e-6 relative); cell order changed, so face-sum order changed. Not
  re-pinned: no pinned value moved.
- `outram-foam-cli` `mesh_tools`, `mechanics_code_to_code`; appbuilder
  `pimple_foam_open_channel`, `pimple_foam_energy_defaults`,
  `cht_double_pipe_exchanger`, `energy_graetz_nusselt`: pass unchanged.

### Not done (gh:#1464)

`mergePatchPairs` (still `NotImplemented` for a non-empty list, never skipped),
`fastMerge yes` (`NotImplemented`).

### Images (AGENTS.md drawing rule)

`images/pipe_ogrid_split_central.png`, `images/t3a_after_topological_merge.png`,
`images/pitz_after_topological_merge.png` (`draw_block_mesh`, lowest-z plane,
patches coloured). Checked: the O-grid is conforming with a closed circle
patch; T3A's grading and patches as before. Not checkable from a picture: the
72 merged points (they sit within 2e-8 of one another).
