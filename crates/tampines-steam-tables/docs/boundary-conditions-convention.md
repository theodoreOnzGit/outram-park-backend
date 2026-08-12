# Boundary conditions on the fluid array: use the TUAS convention

> Maintainer decision, 2026-08-12. Recorded here because the AI-assisted work
> in this crate got it wrong, and the wrong version was plausible enough to pass
> its own tests. Full account, including the process failure:
> `docs/human-corrections-to-ai-work.md` at the workspace root.

## The rule

**`TampinesSteamArray` boundaries follow the TUAS upwind-advection convention, NOT
generic OpenFOAM patch semantics.**

The reference implementation is
`crates/tuas_boussinesq_solver/src/lib/single_control_vol/boundary_condition_interactions/advection_to_bcs.rs`.
Reuse it, or port its logic faithfully and cite it. Do not invent a third set of
semantics.

## Why — these are pipes, not CFD domains

This array is a **1-D flow component that connects to other components in a
network**. Its boundaries are therefore **junctions**, not patches on a
standalone domain. An upwind advection terminal is what allows a loop or a heat
exchanger to be assembled out of these components; `zeroGradient` /
`fixedValue` / `inletOutlet` patch semantics answer a different question — a
standalone case with externally prescribed boundaries.

The `rhoPimpleFoam` lineage this array is ported from brought OpenFOAM's patch
defaults with it. That default is not bad; it is simply the wrong model for a
pipe in a network.

## The convention

From the **previous timestep's** thermodynamic state, always apply an
upwind-equivalent advection boundary condition:

1. Hold **both** candidate upstream states — the extrapolated boundary state and
   the control-volume state.
2. Select by the **sign of the mass flow** at that boundary: boundary-side
   density and enthalpy on inflow, control-volume-side on outflow.
3. Pass enthalpies **upstream-first**, with the order flipped between the
   into-domain and out-of-domain paths.

This makes the failure below **structurally impossible**, because the upstream
value is chosen by direction rather than defaulted.

## What went wrong, so it is recognisable

`FvMatrix::solve` returns a field with **zero-gradient boundaries**, and the
energy equation ended up with zero-gradient at **both** ends.

- A zero-gradient inlet gives `h_face = h_cell0`, so the advective influx is
  `mdot * h_cell0` — **the domain advects in its own enthalpy**. The interior
  field becomes structurally indifferent to whatever is actually flowing in.
- With zero-gradient at both ends there is no boundary through which energy can
  enter or leave, so `mdot*(h_out - h_in) = Q` cannot even be posed.

The reported cause at the time was narrower — "the inlet enthalpy setter is a
one-shot, the BC template is not re-stamped after the energy solve". That was
true, and fixing it makes the setter work, but it restores the *value* while
leaving the *formulation* wrong.

## Tests that are now required

A per-setter test ("does the field move toward the boundary value") only catches
the defect already known about. These catch the class:

1. **Global energy balance** — assert
   `mdot*(h_out - h_in) == lateral/source power + d(stored energy)/dt`, at
   steady state **and** through a transient.
2. **Reversal** — drive a flow reversal and assert enthalpy stays bounded
   between physically available limits. A forward-flow-only test passes
   *identically* with a wrong outlet boundary condition and proves nothing.

## Where this lives in the code (implemented 2026-08-12)

| Piece | Symbol |
|---|---|
| The two candidate upstream states at one end | `AdvectionTerminalState` — `ZeroGradientExtrapolated` (TUAS's `*_non_set_temperature`) or `Junction(h)` (TUAS's `*_set_temperature`) |
| Per-terminal state | `TampinesSteamArray::inlet_terminal` / `outlet_terminal`, set via `set_inlet_enthalpy` / `set_outlet_enthalpy` |
| **Enthalpy** selection on the flux sign | `TampinesSteamArray::correct_advection_terminals` |
| **Density** selection on the flux sign | `TampinesSteamArray::apply_junction_densities` |

The insertion point is the `correct_bcs` re-stamp after the energy solve. That
re-stamp exists because of the lineage difference: TUAS selects the upstream
state per boundary *at step time*, whereas `FvMatrix::solve` discards patch BCs
wholesale and requires them re-stamped afterwards. Those are not competing
mechanisms — the re-stamp is exactly the hook the per-terminal upwind selection
needs, because it is the one place that runs after `phi` is final. Velocity and
pressure are still replayed from a captured template (they are genuine patch
quantities); enthalpy is *derived* from the flux instead.

**One deliberate deviation from TUAS.** TUAS breaks the tie at exactly zero mass
flow toward the control volume (`if mass_flow_from_bc_to_cv > MassRate::zero()`);
this implementation breaks it toward the junction (`phi_b <= 0.0`). At zero flux
the advective term is identically zero either way, so this cannot change the
advected energy. What it does change is that a prescribed junction state stays
effective at start-up and **under stagnation**, where it becomes the Dirichlet
end condition of the axial conduction term — which at stagnation is the entire
heat path.

## Axial conduction is present, and small (2026-08-12)

Related, because the terminal treatment supplies its end conditions. The energy
equation carries `-div(alpha_h grad h)` with `alpha_h = lambda/c_p` read from the
real IAPWS-IF97 conductivity and specific heat every `correct_thermo` — a genuine
term, not a switch. TUAS's assumption that conduction is negligible is justified
*for TUAS's regime* but does not degrade gracefully: it inverts at stagnation,
where convection vanishes and conduction is all that is left.

Measured at the crate's single-phase design point (4 MPa water, 400 K, 3.2 kg/s
through 0.02 m², `dx` = 0.5 m):

- **Péclet number 4.96e5**, so axial conduction is **60.0 mW against a 200 kW
  duty — 3.00e-7 of it**.
- **Crossover at Pe = 1**: velocity 3.447e-7 m/s, mass flow 6.425e-6 kg/s, two
  millionths of the design flow.
- The operator itself is verified against the analytical decay of a Fourier mode
  to **+0.237 %**.

## For a heat exchanger

Four terminals — hot in, hot out, cold in, cold out. Each needs the upwind
treatment independently, keyed on **its own stream's** flow direction. In a
counter-flow arrangement the two streams run in opposite directions, so their
inlets are at opposite ends; the per-stream sign convention is the thing to get
right, and a reversal test on each stream separately is what proves it.
