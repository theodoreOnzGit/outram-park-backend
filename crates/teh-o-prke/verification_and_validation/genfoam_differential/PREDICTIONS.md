# Predictions, written BEFORE any comparison run (gh:#1439, 2026-11-13)

Upstream: GeN-Foam `652b3da` on ESI OpenFOAM v2506, the unmodified
`solvers::pointKineticNeutronics` class from `libNeutronicsSolvers.so`.
Environment: `/home/user/upstream-build/scripts/with-of.sh`.

**What was read before predicting (source, not a run).** Upstream
`include/correctReactivity.H`, `include/solvePointKinetics.H`,
`pointKineticNeutronics.C` (`calcDrivelineReactivity`, the constructor's
`precursorPowers_` start), `pointKineticNeutronics.H`; this crate's
`feedback_mechanisms/fast_reactor.rs` and `zero_power_prke/n_group_prke.rs`
(`backward_euler_step`); the appbuilder port's
`genfoam/neutronics/point_kinetics/mod.rs` header. Nothing was run. One
observation before writing: the issue names an "ESFR `3D_SmallESFR` point-kinetics
variant", but `Tutorials/reactorCases/3D_SmallESFR_old` carries no
`feedbackCoeff*` keys (it is a diffusion case); the tutorial that carries a
fast-reactor point-kinetics coefficient set (Doppler, fuel, clad, coolant density,
structure, driveline, rod-worth map) is `2D_FFTF`. The comparison uses it.

**Design.** A driver subclasses the real class (members are `protected`) and
compiles upstream's own `correctReactivity.H` and `solvePointKinetics.H` through
the class. Feedback case (F): fixed inputs, the include runs verbatim. Kinetics
case (K): the real `correctPhysics()` on a real mesh and `Time`, one step at a
time; this crate then takes upstream's printed `totalReactivity_`, `dt` and
**upstream's previous-step state** and does one `step_implicit`, so errors do not
accumulate and each step is its own comparison.

| # | claim | predicted outcome |
|---|---|---|
| P1 | F, linear terms (fuel, clad, coolant T, coolant density, structure) over 400 steps | **100 % bit-identical**: same `a * (T - Tref)`, same operand order, kelvin and 1/K are SI base units in `uom` |
| P2 | F, Doppler `-K ln(max(T,SMALL)/max(Tref,SMALL))` | bit-identical except where PETIR `r_ln` differs from glibc `log`; **at least 99 % bit-identical, worst 1 ulp**. Precedent: the #1361 `pow` check found 0 of 72 differing |
| P3 | F, driveline: `L alpha` multiplied first, rod-worth map | **100 % bit-identical** with `UpstreamZero` (inside, outside and at both map ends, ascending and descending map). With the default `Error` policy: identical wherever upstream interpolates, the tabulated end value at the one end upstream excludes (upstream gives 0), `Err` outside |
| P4 | F, total (upstream adds `precEq + ext + Doppler + ... + driveline + GEM + boron`, the zero terms exact) | bit-identical wherever every term is; otherwise within 2 ulp |
| P5 | K, one backward-Euler step from the same state, 8 delayed groups, `Lambda = 5e-7 s` (FFTF), dt from 1e-3 s to 0.1 s | **NOT bit-identical.** Upstream assembles `1/dt - (rho-zeta-beta)/Lambda`, `-lambda_i`, rhs `P_old/dt`; this crate assembles the same system with every row scaled by `dt` (`1 - dt/Lambda (rho - beta)`, `-dt lambda_i`, rhs `n_old`). I expect a minority (below 50 %) of steps bit-identical in `P`, worst relative difference in `P` **at most 1e-10** (the system is stiff, `dt/Lambda` up to 2e5) and in each `C_i` at most 1e-11 |
| P6 | K, initial precursor state: upstream `(beta_i P)/(lambda_i Lambda)`; this crate `(beta_i n)/(Lambda lambda_i)` | bit-identical (multiplication is commutative) |
| P7 | the two ports in this workspace agree with each other | the appbuilder `PointKineticsState::step` is upstream's unscaled form, so it should be closer to upstream than this crate; its feedback is the mesh port with negated coefficients. Judged by reading only (this task runs `teh-o-prke` tests only) |

Not compared, and why (to be repeated in the gap survey): the liquid-fuel path
(`solvePointKineticsLiquidFuel.H`: precursor advection), GEM reactivity, boron and
external-source modulation, `TStructMech`, and the flux-squared-weighted field
averages of `computeFeedbackFieldValues.H` (a mesh integral, not kinetics algebra).

Any number outside these is reported as observed with its localisation. None of the
bounds above will be edited after the run.
