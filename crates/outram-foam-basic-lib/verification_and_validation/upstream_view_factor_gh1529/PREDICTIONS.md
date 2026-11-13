# Predictions, committed BEFORE the upstream viewFactor driver or the Rust port was run (gh:#1529, 2026-11-13)

Upstream: ESI v2506 `radiationModels/viewFactor` (direct dense solve, `useDirectSolver true`), view factors
from upstream `faceAgglomerate` + `viewFactorsGen` (built with `-DNO_CGAL`, no CGAL on this box), run
once on a closed 1 m cube (6 patches in group `viewFactorWall`, 4x4x4 blockMesh).
Cases: `cubeA` (no agglomeration, 96 coarse faces, variable emissivity), `cubeB` (agglomerated to
~4 coarse faces per patch, 24 coarse faces, `smoothing true`, `constantEmissivity true`),
`cubeC` (= cubeA mesh and inputs, `constantEmissivity true`, `smoothing false`). Six different wall T and
emissivities (one wall black, E = 1). Hoi (`qro`) = 0 on five walls and 1000 W/m2 on one.
`writePrecision 17`, so F, areas and agglomeration are read back exactly.

1. Parsing F / globalFaceFaces / finalAgglom in ASCII: the Rust reader reproduces the driver's
   (upstream-read) F matrix **bit for bit on 100 %** of entries (strtod and Rust parse are both
   correctly rounded).
2. Coarse-face averages (T4ave, Eave, Hoave): bit-identical 100 % (same operation order, pow4 =
   sqr(sqr), accumulation in ascending fine-face order). No FMA in either build (x86-64 baseline).
3. Rust `SquareMatrix::lu_decompose`/`lu_back_substitute` vs upstream `LUDecompose`/`LUBacksubstitute`:
   I expect the same Crout loop and pivot tie rule, so `qr` bit-identical on every face of all three
   cases (100 %). If a residual shows, it will be a loop-order difference and I will prove it from
   the two sources, not accept it unexplained.
4. `constantEmissivity true` vs `false` give bit-identical `qr` on the same inputs (same C, same
   LU, same RHS order): cubeC == cubeA to the bit.
5. Independent identity (no ESI needed, Hoi = 0 cases): with J_i = sigma T_i^4 + (1-E_i)/E_i q_i the
   radiosity equation J_i - (1-E_i) sum_j F_ij J_j = E_i sigma T_i^4 holds to a relative residual
   below 1e-12; and sum_i A_i q_i = 0 (closed-enclosure energy balance) to 1e-10 of sum A_i |q_i|
   once `smoothing true` forces row sums to 1, and to about the F row-sum error without it.
6. viewFactorsGen accuracy (Gauss tolerance 0.1, 2AI far / 2LI near): I expect the unit-cube row
   sums to be within 5 % of 1 on the unagglomerated mesh and the opposite-face F within 5 % of the
   exact 0.19982 (adjacent 0.20004; Modest, Radiative Heat Transfer, 3rd ed., catalogue of view
   factors, unit cube). Not gated; reported. If it is worse than 5 %, that is the finding.
7. Parallel plates (square plates of side L at distance d inside a closed box whose four side walls
   are black at 0 K, plates 8x8 faces): the centre-face net flux approaches
   sigma (T1^4 - T2^4)/(1/E1 + 1/E2 - 1) from below as d/L falls, error below 15 % at L/d = 8 and
   monotonically decreasing for L/d = 4, 8, 16. Reported, not tuned.
