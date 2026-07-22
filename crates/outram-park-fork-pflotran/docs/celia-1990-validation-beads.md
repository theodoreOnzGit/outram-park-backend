# Celia (1990) validation — bead specifications

> Work items for validating the RICHARDS solver against the canonical
> **Celia, Bouloutas & Zarba (1990)** infiltration benchmark
> (*"A general mass-conservative numerical solution for the unsaturated flow
> equation"*, Water Resources Research 26(7):1483–1496). These are children of
> **op-v6s.9** ("V&V strategy + first RICHARDS benchmark").
>
> **FILED 2026-07-22** into the beads-rs store as **op-v6s.9.1 – op-v6s.9.4**
> (children of op-v6s.9), synced to `origin/beads/store` by the bd daemon. This
> file remains as the human-readable rationale + the Haverkamp formulas + the
> exact `bd create` commands used, in case the beads ever need to be recreated.

## Why Celia 1990

It is *the* reference verification/validation problem for variably-saturated
(Richards) flow: a sharp 1D infiltration front whose under-resolution exposes
mass-balance errors in the head-based form. Celia's mixed (mass-conservative)
form is what our finite-volume residual already uses (accumulation as
`Δ(φ S ρ)/Δt`), so the benchmark both validates the physics and demonstrates the
mass-conservation property.

## Beads

### op-v6s.9.1 — Haverkamp (1977) constitutive model + kr(pc) interface (feature, P2)
The Celia problem uses the **Haverkamp et al. (1977)** relations, in which
relative permeability is a function of **pressure head** `ψ`, not of saturation:

- `θ(ψ) = θr + α(θs − θr) / (α + |ψ|^β)`
- `K(ψ) = Ks · A / (A + |ψ|^γ)`

Our `CharacteristicCurves` currently exposes `relative_permeability(Se)`, which
cannot represent `K(ψ)` directly. This bead adds a Haverkamp variant and widens
the curve interface so `k_r` can be evaluated from capillary pressure directly
(with the analytic `dk_r/dpc` derivative, FD-checked like the existing models).
Requires a clear head↔pressure convention (`ψ = −pc/(ρ g)`, cm vs Pa).
**Depends on:** op-v6s.7.

### op-v6s.9.2 — Celia 1990 1D infiltration benchmark case (feature, P2)
Encode the standard Celia test: a 1D vertical column (Haverkamp sand), the
published soil parameters, uniform dry initial head, a wetter Dirichlet head at
the top, no-flow/fixed bottom, and the reported simulation time. Add it as a
worked example and an integration test that runs and produces the wetting-front
profile. **Depends on:** op-v6s.9.1, op-v6s.8.

### op-v6s.9.3 — Source the Celia reference solution + validation gate (chore, P2)
Digitize/transcribe the reference solution (fine-grid or published figure) from
the **open-access** WRR paper, recording full provenance (authors, title, DOI,
date accessed, digitization method) in a `References.md` beside the case — per
the workspace data-provenance rule; **open literature only**. Then add the
quantitative validation comparison (front position / profile RMSE within a
stated tolerance) and write the methodology-AND-results V&V doc. This is the
bead that turns "verification-only" into "validated against a public benchmark".
**Depends on:** op-v6s.9.2. **This is the actual validation gate for op-v6s.9.**

### op-v6s.9.4 — Global mass-conservation diagnostic (feature, P3)
Celia's central contribution is mass conservation. Add a total-fluid-mass
accessor and a cumulative mass-balance-error diagnostic to `RichardsSimulation`,
and a test proving a closed (no-flow, no-source) domain conserves total mass to
solver tolerance. (A first cut of this lands now, ahead of the bead, in
`flow::richards` + a conservation test.) **Depends on:** op-v6s.8.

## `bd create` commands (run in a beads-rs environment)

```bash
bd create --parent op-v6s.9 --type feature -p 2 \
  --title "Haverkamp (1977) constitutive model + kr(pc) interface" \
  --description "Add Haverkamp theta(psi)/K(psi) relations; widen CharacteristicCurves so k_r can be evaluated from capillary pressure directly (analytic dkr/dpc, FD-checked). Head<->pressure convention psi=-pc/(rho g). Needed for the Celia 1990 benchmark."
bd dep add <id.9.1> op-v6s.7

bd create --parent op-v6s.9 --type feature -p 2 \
  --title "Celia 1990 1D infiltration benchmark case" \
  --description "Encode the standard Celia et al. 1990 infiltration test (Haverkamp sand, published params, dry initial head, wet top Dirichlet, reported time) as a worked example + integration test producing the wetting-front profile."
bd dep add <id.9.2> <id.9.1>
bd dep add <id.9.2> op-v6s.8

bd create --parent op-v6s.9 --type chore -p 2 \
  --title "Source Celia 1990 reference solution + validation gate" \
  --description "Digitize/transcribe the reference solution from the open-access WRR 26(7):1483 paper with full provenance in References.md (open literature only). Add the quantitative validation comparison (front/profile RMSE within tolerance) and the methodology+results V&V doc. Turns verification-only into validated-vs-public-benchmark."
bd dep add <id.9.3> <id.9.2>

bd create --parent op-v6s.9 --type feature -p 3 \
  --title "Global mass-conservation diagnostic" \
  --description "Total-fluid-mass accessor + cumulative mass-balance-error on RichardsSimulation; test that a closed no-flow/no-source domain conserves mass to solver tolerance. First cut landed ahead of the bead in flow::richards."
bd dep add <id.9.4> op-v6s.8
```
