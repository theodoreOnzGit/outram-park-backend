# Step 10 — After FRESCO and PANAMA: STACY in HCP, and what boon-lay aims for

> **Research, education and V&V only.** Not for reactor operation, licensing, safety decisions or emergency response.
> **Review status: first draft, 2026-10-18, AI-assisted (Claude Sonnet), not yet reviewed by a human.**

**The question this page answers.** Release (the FRESCO line) and failure
(the PANAMA line) were built as separate tools. *Who put them together, and
what is this crate trying to do about it?*

**The question from the previous step.** [Step 9](./09-panama.md) gave a
failure fraction from a temperature history. [Steps 5 to 7](./05-random-walk.md)
gave release from a particle. A reactor needs both at once, for every
nuclide, inside one calculation.

## Credit: STACY in HCP

**STACY in HCP is credited here as the follow-up to FRESCO and PANAMA.** It is
the successor that combines fission-product release, in the line of FRESCO,
with particle failure, in the line of PANAMA.

That sentence is the maintainer's framing. This repo holds almost nothing
about it, so the rest is left open on purpose:

| Item | Status |
|---|---|
| Authors and institution of STACY | [citation needed: maintainer to supply] |
| Dates and versions of STACY | [citation needed: maintainer to supply] |
| What "HCP" stands for | [citation needed: maintainer to supply] |
| What STACY computes and how (models, features) | [citation needed: maintainer to supply] |
| How STACY relates to FRESCO and PANAMA in detail | [citation needed: maintainer to supply] |

The only trace in the repo is one line in the literature catalogue. The entry
for Xhonneux, Druska, Struth and Allelein (2014), *Calculation of the Fission
Product Release for the HTR-10 based on its Operation History*, HTR 2014
proceedings, Weihai, China, paper HTR2014-5-181, is described there as good
for "fission-product release (Ag-110m, Cs-137, Sr-90, I-131) over the real
HTR-10 operation history with VSOP/STACY"
([`CATALOGUE.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/kovan-literature/CATALOGUE.md)).
That paper is marked proprietary in the catalogue (no licence statement and
no redistribution rights), so nothing is quoted from it here. A mention in a
catalogue is not a description of STACY, and this page does not treat it as
one.

The FRESCO line, as the repo has it: Fang, Li, Cao, Liu and Fang (2021),
*Analysis of Fission Products' Release in Pebble-Bed High-Temperature
Gas-Cooled Reactor Fuel Elements Using a Modified FRESCO II Numerical
Model*, Science and Technology of Nuclear Installations 2021, 6627789,
[doi:10.1155/2021/6627789](https://doi.org/10.1155/2021/6627789) (open
access, CC BY; in the repo's open corpus). Step 6 treats it. The PANAMA line
is the 1990 report of Step 9.

## What boon-lay tries to achieve

One crate that carries, for a TRISO particle in an HTGR or FHR:

1. **Generation.** Fission products and activation products made by fission
   and neutron capture (Step 1).
2. **Transport**, both ways of computing it:
   - **Lagrangian**: atoms that walk through the layers and decay on the way
     ([`lagrangian_decay_simulator`](https://github.com/theodoreOnzGit/outram-park-backend/tree/@@COMMIT@@/crates/boon-lay/src/lagrangian_decay_simulator),
     Step 5);
   - **Eulerian**: the aggregate closed forms of the TRISO-ATOPS fork
     ([`triso_atops_fork`](https://github.com/theodoreOnzGit/outram-park-backend/tree/@@COMMIT@@/crates/boon-lay/src/triso_atops_fork),
     Steps 6 and 7).
3. **Failure**: boon-lay fuel failure, from the PANAMA-I formulas (Step 9).
4. **The hand-off to the source term**: activity in the coolant, on the circuit
   walls, and leaving the core in an accident, then on to the offsite chain.
   See the tutorial rungs
   [To the coolant](../triso-atops/coolant.html) and
   [Into the source term](../triso-atops/handoff.html), and the deep dive
   [Into the source term](../../deep-dives/triso-atops/source-term.html).

The two transport routes are complements, not rivals: the Lagrangian one shows
how atoms get out, and the Eulerian one gives the release fractions the
offsite chain consumes (the crate's
[README](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/README.md)
says the same). The chain downstream is `boon-lay`, then `sembawang` (source
term), then `changi` (dispersion).

The aim above is stated as an aim. The next sections say how far it has got.

> **Simplification, stated explicitly.**
> **A framework exists, but transmutation is not yet modelled while fission
> products move through the layers (no per-nuclide cross sections).**
> The intended behaviour is that an atom diffusing out of a particle can
> still be changed by neutrons on the way: Xe-135 capturing a neutron to
> become Xe-136 is the standard example. The crate has the competing-clock
> machinery for it, but not the nuclear data to drive it. The full design
> (decay, capture, `(n,2n)`, fission) is a plan, not a result.
>
> **What the code actually does (verified in `depletion.rs`).**
> The Lagrangian walker in
> [`depletion.rs`](../../code-review/#crates/boon-lay/src/lagrangian_decay_simulator/lagrangian_diffusion/first_passage/depletion.rs::advance_until)
> does change nuclide *while it walks*. Its competing clock is
> decay plus a transmutation rate, and a unit test runs one `(n,gamma)`
> channel. The field is **one explicit
> capture channel** with a single user-supplied rate $\phi\sigma$, one target
> and one product. There are no per-nuclide cross sections, no flux map,
> no `(n,2n)`, and no fission with yield sampling. No neutron-capture
> channel was found in the Eulerian fork (Not re-checked in full). The deep dive
> [Transmutation while walking](../../deep-dives/triso-atops/transmutation.html)
> records the same limits, and notes there is no Bateman or analytic check of
> a depleting ensemble yet (gh:#539). So: *realistic* transmutation in transit
> is not modelled; a *framework* with one hand-fed channel exists, and has
> not been validated.

## How far it has got

What exists, and its verification status as the crate records it
([README](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/README.md)):

| Piece | State |
|---|---|
| TRISO-ATOPS fork (Eulerian) | verified code-to-code against upstream (14 130 cases in the third pass; worst group $2.44 \times 10^{-9}$ relative) |
| TRISO-ATOPS extensions | reduce to the port bit for bit with upstream's rates; the new rates are not validated |
| Lagrangian simulator | unit-tested; Monte Carlo half-lives checked against ENDF/B-VIII.0; release fraction checked against the IAEA CRP-6 Case 1a/1b analytical solution; no cross-code comparison |
| boon-lay fuel failure | printed tables of the report reproduced where they exist; Eqs (4) and (11) to (14b) have nothing to check against |

None of this is validated against measured release data.

## What is not built yet

An honest list, from the crate's own limitations page and CLAUDE.md:

- **Realistic transmutation in transit** (the box above): per-nuclide cross
  sections and flux maps (natural sources: `njoy-outram-park-fork` and
  `outram-mc-libs`), `(n,2n)`, and fission with yield sampling.
- **A Bateman or analytic check** of a depleting Lagrangian ensemble (gh:#539).
- **Generation inside the particle model.** The Lagrangian engine takes an
  atom's identity as given; fission with yield sampling is not implemented, so
  generation at the source is not yet part of the walk.
- **A full multilayer CRP-6 release record.** The benchmark is single-layer.
- **Graphite diffusion coefficients.** The graphite lookups are `todo!()`,
  and elements other than Ag, Cs, Sr and Kr fall back to silver (gh:#541).
- **A first-principles stress model.** There is no finite-element particle
  model in `farrer-park` today ([Step 8](./08-pressure-stress.md)). The PyC
  creep and swelling are not in the failure chain.
- **An oxidation-failure model.** Failures in air are added by hand
  downstream; the mechanistic chain is gh:#441.
- **Normal-operation failure.** In-service failure is an input, not a result.
- **Validation** against measured release or failure data, for any piece.
- **A description of STACY and HCP in this book** (the placeholders above).
- **Accident release beyond venting.** An isothermal hold releases exactly
  0 Bq in the TRISO-ATOPS accident path (gh:#446).

## What we assumed

- STACY in HCP is the successor combining release and failure; every detail of
  that is the maintainer's to supply.
- A particle-level model that carries generation, transport, failure and the
  hand-off in one crate is worth building, even though each part is unfinished.
- The two transport routes agree where they overlap. That has been checked for
  the single-layer benchmark only.

## Next question

This is the last step of the book. The open question is the first one in the
list above: what does the particle's inventory become on its way out, once
neutrons are allowed to act on it? Start again from
[Step 1](./01-generation.md), with the transmutation channel in mind.
