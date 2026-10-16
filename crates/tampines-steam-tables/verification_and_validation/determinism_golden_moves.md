# Determinism goldens: record of every move

`tests/cross_platform_determinism.rs` pins FNV-1a-64 hashes of solver outputs
on x86_64-unknown-linux-gnu (release). Its rule is that a golden may move
**only with a named cause**. This file is where each move is recorded.

## 2026-10-16: Part B (`edw_*`, 200-step Edwards slice), all three modes

- **Cause.** gh:#900 (epic #896 Phase A) changed `TampinesSteamArray`'s
  defaults to match upstream OpenFOAM-10 `rhoPimpleFoam`. The maintainer
  approved this change of defaults (D5, 2026-10-16). The three changes are:
  - implicit upwind energy convection with deferred correction
    (`EnergyBalanceMode::Implicit`);
  - `PsiRefresh::OncePerOuterCorrector`;
  - the kinetic-energy terms (`KineticEnergyTerms::Included`).
- **What moved.**
  - 45 of 48 Part B sub-hashes moved: Pimple, hybrid KNP and KNP-forced, each
    at steps 1, 10, 50 and 200.
  - The three `he` hashes at step 1 did not move.
  - Part A (the IF97 property sweep) did not move, so the property routines
    are untouched.
- **Old and new values.** The old hashes are in git history: the
  `GOLDEN-TABLE` in `5d16d67e2`. The new hashes are in the current file.
- **Interpretation.** This is a physics/numerics change by design, not a
  determinism regression. The Edwards 600 ms results it corresponds to are
  recorded in the `tests/edwards_blowdown.rs` header. On the Linear scheme,
  the plateau and GS-1 RMSE went from 359.0 / 42.8 psia to 360.5 / 17.2 psia,
  and pressure-bound events went from 150 to 0.
