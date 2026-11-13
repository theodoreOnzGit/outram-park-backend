# Wall function (`nutkWallFunction`) against OpenFOAM-v2506 (ESI), gh:#1362 row 5, slice 1

**Scope: this is only the `nutkWallFunction` slice of row 5.** The turbulence
models themselves (k-epsilon, k-omega, SST, Spalart-Allmaras, Smagorinsky) and the
`epsilon`/`omega` wall functions were NOT run against upstream; see "Not done".

Upstream line: ESI v2506 (`libturbulenceModels`, unmodified). Foundation was not read.

Driver `driver/wallFnDump.C`: the real `wallFunctionCoefficients` class (computes
`yPlusLam`) plus a line-for-line transcription of the STEPWISE branch of
`nutkWallFunctionFvPatchScalarField::calcNut()` for one face (the patch field needs
a case, mesh and turbulence model). Grid: (kappa, E) in {(0.41, 9.8), (0.40, 9.0),
(0.38, 5.5)} x nu in {1e-5, 1.5e-5, 1e-6} x 12 y+ values from 1 to 5000.
Rebuild: `scripts/rebuild_reference.sh`. Test:
`tests/openfoam_wall_function_code_to_code.rs`.

## Prediction (written before the Rust side ran)

The port's `nu_t_wall` is `nu (kappa y+/ln(E y+) - 1)` clamped at 0, crossover 11;
upstream is `nu y+ kappa/ln(max(E y+, 1+1e-4)) - nu` above `yPlusLam`
(11.5301 for the defaults), else 0. Predicted: the two agree to rounding (not bit
for bit) at the default kappa, E above 11.53, and differ for any other kappa, E
(the port has them fixed); `yPlusLam` differs from the port's 11.

## Results (2026-11-13, release)

- `yPlusLam` (new `y_plus_lam`): **3/3 bit-identical**.
- `nu_t_wall_esi_v2506` (new, ESI arithmetic): **108/108 bit-identical**, 0 ulp.
- the older `nu_t_wall`: 42/108 bit-identical, 48 beyond 1e-12 (the non-default
  kappa/E rows, which it cannot represent), the rest at rounding level.
  Prediction right.

Option added, old form kept: `wall_functions::{y_plus_lam, nu_t_wall_esi_v2506}`.

## Not done in row 5 (ledger)

Read from the sources only, NOT run, so treat as hypotheses:

- k-epsilon `correctNut`: upstream bounds `epsilon` at `epsilonMin = small` and
  forms `Cmu k^2/epsilon`; the port bounds `epsilon >= Cmu k^2/(1e5 nu)` (a
  different, viscosity-ratio bound) and clips `k` at 0.
- The port's `correct()` solves a transient (`dt`) transport system with its own
  `SolverSettings`, so a bit-level comparison of the model needs the operator-level
  deferred item D2 (ulp orderings, gh:#1489) first; source-term-level dumps
  (`G`, `epsilon` source coefficients) on a frozen channel field remain to do.
- `epsilonWallFunction`, `omegaWallFunction`, `kqRWallFunction` do not exist in
  the port (only the log-law helpers).
