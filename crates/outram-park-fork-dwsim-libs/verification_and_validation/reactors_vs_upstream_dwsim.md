# Reactors vs upstream DWSIM (code-to-code)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** Code-to-code verification against
> upstream DWSIM, not validation against experiment. Not for nuclear facility
> operation, reactor control, safety-critical, or licensing decisions.

**Generated:** 2026-10-02 (runs between 01:00 and 06:00 UTC)
**Crate commit:** `74f5585b40` (conversion), `96643feee1` (PFR, CSTR LH),
`b2f235b991` (equilibrium), `81f2c517db` (Gibbs), on branch
`worktree-agent-a0373aff4202acade` off `develop` `bcac8b58a0`.
**Upstream:** DWSIM 9.0.5.0 built from `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`
with `dotnet` SDK 10.0.112 + conda-forge Mono 6.12.0.199
(`docs/upstream-harness/README.md`, "Building without root").

## Methodology

For each reactor, a C# driver (`docs/upstream-harness/<reactor>_driver.cs`)
builds a real upstream unit operation on a headless flowsheet. The setup is an
inlet, outlet(s) and an energy stream; Peng-Robinson; isothermal operation;
reactions created through upstream's own `FlowsheetBase.Create*Reaction`. It
then calls `Calculate()` and prints `KEY=value` lines. The same case is then
solved by this crate in `tests/upstream_<reactor>_parity.rs`, with upstream's
numbers frozen as constants or, for the PFR, as fixtures in
`tests/fixtures/upstream_pfr/`.

Two comparisons are kept apart, because they answer different questions:

1. **Solver parity.** This crate is handed the quantities its documented
   simplifications would otherwise compute differently: upstream's outlet or
   per-segment volumetric flow `Q` (CSTR, PFR), and upstream's `g°/RT`,
   `P0` and converged PR `ln φ` (Gibbs). Gates: closed-form cases 1e-12,
   iterative cases 1e-8 to 1e-10, Gibbs 5e-5 (GibbsMin) and 1e-6 (Lagrange).
2. **Normal use.** This crate as a caller would run it: inlet `Q`, φ = 1. The
   gap is the measured cost of the simplification (coverage row R7). It is
   pinned within a band, not gated to zero.

Where upstream's iteration is not converged at its defaults (CSTR
`Tolerance`, equilibrium `InternalLoopTolerance`), it is tightened, and the
value used is recorded in the test.

## Reference

```bibtex
@software{dwsim_1abf72d1,
  author  = {Medeiros, Daniel Wagner Oliveira de},
  title   = {{DWSIM} -- Open Source Chemical Process Simulator},
  version = {9.0.5.0, commit 1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766},
  url     = {https://github.com/DanWBR/dwsim},
  license = {GPL-3.0},
  note    = {DWSIM.UnitOperations/Reactors/{Conversion,Equilibrium,Gibbs,PFR,CSTR}.vb}
}
```

## Results

Worst per-species relative gap of outlet molar flow (or extent, where stated):

```csv
reactor,case,solver_parity,normal_use,note
conversion,single (nu_BC=-2),3.7e-16,3.7e-16,closed form
conversion,sequential ranks,0,0,bit-identical
conversion,parallel rank-0,0,0,"before fix: CH4 +75%, CO -50% (#477)"
conversion,overspecified group,exact optimum,1.8e-3,upstream simplex stops short
conversion,penalty scope,n/a,n/a,"upstream loses 25% of carbon (#478)"
conversion,liquid-phase reaction 2-phase feed,n/a,+67% iC4,ReactionPhase not ported (#479)
equilibrium,WGS molfrac 1 bar,3.9e-12,3.9e-12,upstream at tol 1e-20
equilibrium,SMR molfrac 10 bar,1.7e-15,1.7e-15,
equilibrium,SMR fugacity 10 bar,n/a,+0.317% extent,"before fix +129% (#482)"
equilibrium,SMR partial pressure 30 bar,n/a,+1.114% extent,ideal phi (#483)
equilibrium,SMR activity 10 bar,n/a,+129.5% extent,activity basis vs ReactionPhase (#479)
gibbs,SMR 1100 K 1/10/20 bar vs GibbsMin,8.5e-6,"-6.1e-4..-6.4e-3 CH4 (ideal gas)",upstream penalty imbalance 2e-6
gibbs,SMR 10/20 bar vs Lagrange,3.5e-7 / 1.6e-9,,"after adding upstream's ln(P/P0)/(RT) slip (#485)"
pfr,iso_gas V=20,1.4e-10,5.1e-3,
pfr,smr V=2,2.6e-9,6.0e-2,constant-Q cost (#483)
pfr,iso_liq V=0.02,1.2e-10 on 0.99 dV,5.9e-2,"upstream integrates 99% of each segment (#480); full dV gap 1.9e-2"
pfr,hetcat bed,2.0e-10,n/a,"before fix +96.3% n-butane (#481)"
cstr,iso_liq / iso_gas / smr,4.9e-12 / 1.7e-13 / 6.1e-10,+9.6% X(CH4) on smr,recorded 2026-09-25; reproduced bit-for-bit 2026-10-02
cstr,hetcat liquid,2.4e-13,n/a,"before fix -99.1% (#481)"
```

**Interpretation.** Every solver agrees with upstream to round-off or to
upstream's own convergence limit, once both are handed the same thermodynamic
state. The differences that remain are of two kinds. The first is **port
defects**, now fixed: conversion rank groups, the equilibrium fugacity basis,
and heterogeneous catalytic rates. The second is **documented
simplifications** with measured costs: constant `Q`, ideal φ and no
`ReactionPhase`. The comparison also found **upstream defects** (#478, #480,
#484, #485, plus the earlier #326), which the port does not reproduce. Two of
them change answers by more than 1 % without warning: the PFR segment
truncation (+1.9 %) and the conversion penalty scope (25 % of the carbon
lost).
