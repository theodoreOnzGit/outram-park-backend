# Into the source term

**The problem.** A release fraction per particle is not yet what anyone
downstream needs. The offsite chain wants activity, in becquerels or curies,
in the coolant, on the circuit walls, in the purification system, and what
leaves the plant in an accident. This chapter follows a nuclide from the fuel
to those pools, all in the
[`triso_atops_fork`](../../api/boon_lay/triso_atops_fork/index.html) port.

## Step 1: failure fractions meet release-to-birth

TRISO-ATOPS describes the failed fuel with four fractions
([`FailureFractions`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/triso_atops_fork/activities/source_terms.rs#L49-L65)):
heavy-metal contamination `f_hm`, as-manufactured defective SiC `f_sic`,
in-service failure `f_inc`, and in-service SiC-only failure `f_inc_sic`.

How they combine with `<R/B>` depends on the transport group
([`release_rate`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/triso_atops_fork/activities/source_terms.rs#L166-L192)):

```rust,ignore
{{#include ../../../src/triso_atops_fork/activities/source_terms.rs:166:192}}
```

- **Volatiles** (noble gases, halogens) leave only from fully exposed fuel:
  `(f_hm + f_inc)·<R/B>`.
- **Silver** uses `<R/B>` directly, because its breakthrough model already
  includes the SiC.
- **Metals** use all four fractions.

The birth rate is the inventory activity for a short-lived nuclide (secular
equilibrium) and `A / (1 − e^{−λt})` for a long-lived one.

## The seam between the two models

This is where boon-lay fuel failure plugs into TRISO-ATOPS.
[`with_fuel_failure_incremental`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/triso_atops_fork/activities/source_terms.rs#L106-L116)
replaces `f_inc` with `1 − (1 − φ₁)(1 − φ₂)` from an accident history, and
leaves the other three untouched:

```rust,ignore
{{#include ../../../src/triso_atops_fork/activities/source_terms.rs:106:116}}
```

It is an **added** route, not a replacement: a run that sets all four by hand
is unaffected, and a test pins that. Its doc says when **not** to use it:
for normal operation, `φ₁` answers a different question (see
[the fuel-failure chapter](./fuel-failure.md)). `φ₂` is deliberately not
routed to `f_inc_sic`, because upstream's `f_inc_sic` is a different
population, and mapping one onto the other would invent a correspondence
neither code states
([`in_service_failure_fraction`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/history.rs#L429-L441)).

## Step 2: graphite hold-up, then the loop pools

Metals released from the particles still have to cross the fuel-element
graphite, which holds some back (the attenuation factor `Af`). Non-metal
"other" nuclides get `Af = 10⁸`, which means they are effectively all held
([`OTHER_ATTENUATION_FACTOR`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/triso_atops_fork/activities/source_terms.rs#L42-L47)).
What reaches the coolant feeds three pools: circulating `C`, plate-out on
surfaces `P`, and clean-up by the helium purification system `H`.

The ported closed forms give the pools after time `t` from empty with a
constant source. A simulator needs more: it must carry the pools while the
source changes. That is
[`live_pools`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/triso_atops_fork/activities/live_pools.rs#L12-L32),
which is **not a port**. It steps the same balances exactly over a step with
a constant source, adds a leak sink, and reproduces the ported closed forms
when started from empty:

```text
dC/dt = S − β·C,   β = λ + k_plate + k_clean + k_leak
dP/dt = k_plate·C − λ·P
dH/dt = k_clean·C − λ·H
```

The whole normal-operation chain for one core node is
[`normal_operation_node`](../../api/boon_lay/triso_atops_fork/normal_operation/fn.normal_operation_node.html).

### Units: one decision, made explicitly

Upstream mixed atoms, atoms/s, curies and becquerels through hard-coded
factors (`× 3.7e10`, `÷ (1 − e^{−λt})`, `× λ / 3.7e10`). The port makes each
one explicit. Activity is a `uom` frequency (one decay per second). Ci↔Bq is
one named constant. Atom counts stay plain `f64`, because a count is
dimensionless and "atoms/s" has the same dimension as a decay constant, so
`uom` could not tell them apart
([`activities/mod.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/triso_atops_fork/activities/mod.rs#L31-L56)).

## Step 3: the accident

The accident path is ported as **pieces**, because upstream has no single
`accident_case` function to port; callers compose it (gh:#447). Per nuclide
and node:

```text
∫D dt over the transient → release fraction (kernel, graphite)
  → release_activity (what is left) → curies
    → × coolant_release fraction + plate-out lift-off → released curies
```

It is a **depressurisation** model. Activity leaves the core only by venting
while the core heats, so there is no air- or water-ingress transport
(gh:#446). `sembawang` adds flow-through options on top, and labels them as
not upstream
([`accident/mod.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/triso_atops_fork/accident/mod.rs#L31-L49)).

> **Check yourself.** In a depressurisation where the core temperature first
> rises and then falls, during which part does `coolant_release` move
> activity out of the core in this model? What physical route does that leave
> out?
