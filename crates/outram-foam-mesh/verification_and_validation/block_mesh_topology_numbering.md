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
   neighbour 1; `faces[1] = [2 23 464 443]`; 760 internal faces. (A regular hex
   is re-labelled by the collapse step to `[0 3 7 4 1 2 6 5]`, so cell 0's
   face shared with cell 1 is the model's z-max face, which yields exactly that
   winding.)
3. **Merge classes equal the old geometric merge** on these dicts (all block
   interfaces conforming), so point counts are unchanged.
4. **Solver outputs** that depend on the mesh (pipe Hagen-Poiseuille, cavity
   steady state) move by round-off only, because only the summation order of
   face sums changes.
