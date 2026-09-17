# GENERAL multiphase flow mode — design of record (op-v6s.13)

> **Status: designed, NOT implemented.** This records the intended translation
> and, importantly, the **infrastructure prerequisite** that must land first.
> No multiphase solver code exists yet — writing one before the prerequisite
> would be dishonest scaffolding. Untrusted AI draft, for human review.

## What GENERAL is

PFLOTRAN's GENERAL mode is multiphase, multicomponent flow — for the v1 target,
**air–water–energy**: two fluid phases (liquid water + gas/air) plus energy,
with phase appearance/disappearance. Per grid cell the primary unknowns are a
**set** (e.g. `{liquid pressure, gas saturation OR air mole fraction, temperature}`
with a variable-switching scheme as phases appear/disappear), not the single
scalar of RICHARDS.

## Why it cannot reuse the current solver as-is

The v1 solver stack (`solver::NewtonSolver` over `outram-foam-basic-lib`'s
`LduMatrix`) is **scalar: one degree of freedom per cell**. RICHARDS (1 unknown:
`p`), solute transport (1: `c`), and TH energy (1: `T`, one-way coupled) all fit
this. GENERAL has **N>1 tightly-coupled unknowns per cell**; its Jacobian is
block-structured (an `N×N` dense block on the diagonal and per-connection), which
the scalar `LduMatrix` cannot represent.

**Prerequisite (bead op-v6s.4.1):** a multi-DOF coupled block Newton–Krylov
layer — block-sparse Jacobian assembly (`n_dof_per_cell`) over the grid
connectivity, block ILU/Jacobi preconditioning, solving the flattened `N·n_cells`
system with the existing foam-basic-lib Krylov solvers. GENERAL (op-v6s.13)
**depends on** this (dependency edge recorded in beads).

## Intended formulation (once the block solver exists)

Per phase `α ∈ {liquid, gas}`, component `κ ∈ {water, air}` and energy:

- **Component mass conservation** (summed over phases):
  `∂/∂t( φ Σ_α S_α ρ_α x_α^κ ) + ∇·( Σ_α ρ_α x_α^κ q_α ) = Q^κ`,
  with per-phase Darcy flux `q_α = -(k k_{rα}/μ_α)(∇p_α - ρ_α g)`.
- **Energy conservation:** as in the TH `energy` module but summed over phases
  (advected enthalpy per phase + effective conduction).
- **Constraints:** `Σ_α S_α = 1`, capillary `p_gas - p_liq = p_c(S_liq)`
  (reuse `properties::CharacteristicCurves`), equilibrium partitioning
  (Henry's law for air in water, vapour pressure for water in gas).
- **Primary-variable switching** for phase appearance/disappearance (the
  standard GENERAL/TOUGH approach): choose the active unknown set per cell by
  phase state, and re-map on state change.

## Reuse from the existing crate

- `grid` (connectivity, transmissibility), `properties::CharacteristicCurves`
  (retention + rel-perm, already has van Genuchten / Brooks–Corey / Haverkamp),
  `properties::thermal` (thermal props), the `energy` conduction/advection
  pattern, and the foam-basic-lib Krylov solvers (via the new block layer).

## Phasing

1. **op-v6s.4.1** — block multi-DOF Newton–Krylov solver (+ its own verification:
   a coupled 2-variable analytical system).
2. **op-v6s.13** — two-phase (air–water) isothermal first, then add energy;
   verification against an analytical two-phase case (e.g. Buckley–Leverett for
   the saturation front) before any GENERAL benchmark.
3. Validation (op-v6s.13.1) — deferred per the current maintainer directive.
