# Step 14 — TRISO-ATOPS: the cheap model

> **Research, education and V&V only.** Not for reactor operation, licensing, safety decisions or emergency response.
> **Review status: first draft, 2026-10-18, AI-assisted (Claude Sonnet), not yet reviewed by a human.**

**The question this page answers.** [Step 13](./13-brute-force-cfd.md) showed
that computing the circuit directly is too heavy. *What does the cheap model
look like, what does it assume, and how far can it be trusted for a reactor it
was not built for?*

**A note on the point of this page.** TRISO-ATOPS matters here as an
*example* of how a calculation is made bearable by choosing what to keep. It
is not the point in itself. [Step 19](./19-levels-of-fidelity.md) puts it
beside boon-lay fuel failure and the dispersion models.

## 1. What TRISO-ATOPS is

TRISO-ATOPS is Idaho National Laboratory's source-term tool for gas-cooled
reactors, named in the fork's docs as "TRISO Analysis TOol for Predictive
Source terms". `boon_lay::triso_atops_fork` is a Rust fork of the MIT-licensed
Python at upstream commit `de374c8`
([fork guide](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/docs/triso-atops-fork.md)).
Its physics equations originate from the NP-MHTGR New Production Reactor
Program (Anderson et al., *Generic Reactor Plant Description and Source Terms
Volume 1*, EG&G Idaho, 1989), and the paper that describes it is Stoyer,
Raichart et al., *TRISO-ATOPS: A Mechanistic Source Term Model for Gas-Cooled
Reactors*, Nuclear Technology (2026), as the repo cites it.

The question it answers: for a given inventory, temperatures and failed-fuel
fractions, how much activity is in the graphite, circulating in the coolant,
plated out, and in the purification system, and how much leaves the core in a
depressurisation.

## 2. The ladder from CFD down to it

Fewest assumptions first, each rung dropping something and saying what keeps it
acceptable.

| Rung | Model | Dropped | Why acceptable |
|---|---|---|---|
| 0 | CFD, one equation per species (Step 13) | nothing | reference |
| 1 | One lumped volume per core node, first-order rates (this page) | position in the loop, wall chemistry, dust | the rates carry the net effect |
| 2 | Steady-state closed forms | time dependence | normal operation is slow compared with the transient |

TRISO-ATOPS lives at rungs 1 and 2. Rung 1 is the one the digital twin uses.

## 3. The model, in three parts

**Part 1: release from the fuel.** For each node and nuclide:

1. The diffusion coefficient $D(T)$ at the node's fuel temperature
   ([`diffusion_coefficient`](../../code-review/#crates/boon-lay/src/triso_atops_fork/diffusion/mod.rs::diffusion_coefficient)).
2. A release-to-birth ratio $\langle R/B\rangle$ for a failed particle,
   from a model chosen by group: a Booth equivalent sphere for the special
   metals, a breakthrough model for silver, an empirical correlation for noble
   gases and halogens, a fixed value for "other"
   ([`rb_fail`](../../code-review/#crates/boon-lay/src/triso_atops_fork/release_models/mod.rs::rb_fail)).
   This is the Booth closed form of [Step 6](./06-eulerian-fresco.md) put to
   work.
3. A release rate by failure class. Noble gases and halogens use
   $(f_{hm} + f_{inc}) \langle R/B\rangle B$, silver uses $\langle R/B\rangle B$,
   metals use all four failure fractions
   ([`release_rate`](../../code-review/#crates/boon-lay/src/triso_atops_fork/activities/source_terms.rs::release_rate)).

**Part 2: the graphite, then the pools.** Metals are held back by the fuel
element's graphite (an attenuation factor $A_f$). What reaches the coolant
feeds three pools: circulating $C$, plated out $P$, and cleaned up $H$:

$$\frac{dC}{dt} = S - (\lambda + k_{plate} + k_{clean})C$$

$$\frac{dP}{dt} = k_{plate}C - \lambda P, \qquad \frac{dH}{dt} = k_{clean}C - \lambda H$$

These are the same balances as [Step 12](./12-escape-and-removal.md), with
$S$ the source into the coolant. The closed forms are in
[`coolant_activity.rs`](../../code-review/#crates/boon-lay/src/triso_atops_fork/activities/coolant_activity.rs::circulating),
and the whole chain for one node and nuclide is
[`normal_operation_node`](../../code-review/#crates/boon-lay/src/triso_atops_fork/normal_operation/mod.rs::normal_operation_node).
`live_pools.rs`, which is **not a port**, steps the same balances from any
starting state with a leak sink added, so a transient can carry the pools
forward.

**Part 3: the accident.** Release is computed from the temperature history and
vented by gas expansion. [Step 15](./15-dlofc.md) goes through it.

## 4. How INL uses it for an MHTGR

The repo records the two MHTGR cases the paper runs, as inputs in
[`constants.csv`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/verification_and_validation/mhtgr_stoyer/constants.csv):

- **Case A**: 350 MWth. **Case B**: 600 MWth.
- Same plate-out and clean-up rates for both: $k_{plate} = 7.5 \times 10^{-5}$
  per second and $k_{clean} = 8.77 \times 10^{-5}$ per second (Table 3
  recorded as inputs, not derived).
- Lift-off fraction $x_{liftoff}$: 0.05 for Case A, 0.005 for Case B
  (Case B from the paper's section III.B.1).
- Failure fractions: for example $f_{hm} = 2 \times 10^{-5}$ in Case A and
  $1 \times 10^{-5}$ in Case B.

The repo checks its port against the paper's tables.
**Recorded results** (not re-run for this page), from
[`coolant.md`](../triso-atops/coolant.html) rung 7:

| Check | Recorded result |
|---|---|
| Port against upstream code, 14 130 cases | worst group $2.44 \times 10^{-9}$ relative |
| MHTGR normal operation, port vs upstream | worst $5.6 \times 10^{-12}$ relative |
| Paper's printed pools, graphite | 40 of 40 match |
| Paper's printed circulating pool, with $k_{plate}$ as printed | 11 of 64 match |
| The same, with $k_{plate} = 7.5 \times 10^{-4}$ | all within 2 % |
| Accident heat-up releases, final over paper | about 0.90, undiagnosed |

Two honest notes from that record. The printed $k_{plate}$ looks like a
misprint, and the repo keeps it as printed with the alternative as a labelled
diagnostic. And a common factor of about 0.90 on the accident releases is
**not diagnosed**. These are *verification* results (the port reproduces
another code's workflow), not validation (no measured reactor release enters).

## 5. Why use these numbers as stand-ins for HTR-10, and the limits

HTR-10 is a small research pebble-bed reactor. The repo has few HTR-10 values
for these inputs, so the digital twin borrows the MHTGR ones. Which inputs
are borrowed and which are HTR-10's, as the code records them
([`plant_parameters`](../../code-review/#crates/sembawang/src/htr10.rs::plant_parameters)):

| Input | Source in the HTR-10 stage |
|---|---|
| $k_{plate}$, $k_{clean}$, grain size | NP-MHTGR stand-ins, in the sembawang path |
| $x_{liftoff}$ | 0.05, the NP-MHTGR stand-in (Case A) |
| Fuel and graphite temperature | a single 776 C stand-in, uniform |
| Free uranium, in-service failure | HTR-10 values: measured free uranium (Tang et al. 2002), design in-service failure (Liu & Cao 2002) |
| Leak, purification efficiency, per-cycle plate-out | HTR-10's (Liu & Cao 2002), in the digital twin's own rates |

Reasons it can be defended: the model's structure is group chemistry and
first-order removal, which is not reactor-specific, and the numbers scale the
answer without changing its shape or temperature dependence (the code's own
description).

Reasons it must be limited:

- **A different reactor.** The failure fractions differ: $f_{hm}$ is
  $2 \times 10^{-5}$ in MHTGR Case A, against the measured
  $5.0 \times 10^{-5}$ (2.5 times) or the design specification
  $3 \times 10^{-4}$ (15 times) recorded for HTR-10 (Step 11). Noble-gas and
  halogen release is linear in it.
- **A different size.** Case A and B are 350 and 600 MWth. HTR-10's power is
  not stated on the pages I read here [citation needed: maintainer to supply].
- **Rates depend on the circuit.** Plate-out and clean-up depend on surface,
  temperature and purification flow, which are different in HTR-10. The
  HTR-10-specific rates in the twin are a separate set.
- **Nothing is validated** against a measured release for either reactor.

The twin states this itself: "They scale the answer without changing its
shape" and "NOT VALIDATED"
([`fission_product_release.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/fission_product_release.rs)).

## Why each simplification is allowed

**Kinds of justification used below.** (a) negligible effect, with an estimate; (b) computationally too expensive to be practical; (c) bounding or conservative, with the side it errs on; (d) data not available. **Open** means this page cannot justify it.

| Simplification | Kind | Justification |
|---|---|---|
| Lumped pools per node | (b) | Direct CFD per species is too costly (Step 13) |
| First-order rates | (d) | No sorption data; rates are constants from the paper or from Liu & Cao |
| MHTGR constants for HTR-10 | (d) | HTR-10 values are missing. Not shown small: the $f_{hm}$ ratio alone is 2.5 to 15 |
| Uniform 776 C fuel temperature in the HTR-10 stage | (d) | Stand-in; effect **open** |

## What we assumed

- One well-mixed volume per node, with constant first-order rates.
- Groups stand in for chemistry (Step 12).
- Release is the Booth, breakthrough or empirical closed form, not a walk.
- MHTGR constants can stand in for HTR-10's where HTR-10's are missing.

## Next question

The most important use of the model is an accident. What physically happens in
a loss-of-cooling accident, and which parts does TRISO-ATOPS keep?
[Step 15](./15-dlofc.md).
