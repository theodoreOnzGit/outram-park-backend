# Step 17 — Outside the vessel: the building, and bounding assumptions

> **Research, education and V&V only.** Not for reactor operation, licensing, safety decisions or emergency response.
> **Review status: first draft, 2026-10-18, AI-assisted (Claude Sonnet), not yet reviewed by a human.**

**The question this page answers.** Activity that has left the primary circuit
([Step 16](./15-dlofc.md)) is not yet in the environment. *What is between the
vessel and the open air, what would it do in reality, and why may a safety
argument choose to give it no credit at all?*

## 1. What happens in reality, for the one building the repo describes

The repo describes one reactor's building in detail: HTR-10's. Its
`bishan` crate records the published design, citing Jiang, Ye & Yang (2002),
*Nucl. Eng. Des.* 218, 209-214, which is proprietary and not redistributed
([`building.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/bishan/src/building.rs)):

- HTR-10 has a **vented confinement**, not a pressurised containment.
- Five cavities (reactor, steam generator, fuel handling, operation gas
  valves, helium purification), the first four interconnected.
- A negative-pressure exhaust holds the cavities at -150 Pa in normal
  operation. The allowable leakage rate is 100 % of the volume per day at that
  pressure. The exhaust passes prefilters and HEPA filters (and adsorbers on
  the accident train) to a 40 m chimney.
- Above 10 kPa gauge, rupture disks burst and the helium leaves by a 22.6 m
  chimney **without filtering**. Maximum design pressure is 30 kPa gauge.

Whether other HTGR designs use a vented confinement or a pressure-retaining
containment is not stated in the repo's pages I read
[citation needed: maintainer to supply]. Do not carry HTR-10's arrangement
over to another design.

In such a building, five things can happen to activity that arrives:

1. **Hold-up.** It mixes into the building air and stays for a while.
2. **Decay.** Short-lived nuclides decay before they leave.
3. **Deposition.** Some settles onto surfaces.
4. **Filtration.** Exhaust passes through filters that capture some of it.
5. **Leakage and venting.** The rest leaves through the exhaust, leaks, or the
   disks.

## 2. The fewest-assumptions model, and the cheap one

**Fewest assumptions:** a flow and aerosol calculation over each cavity,
with the connecting flows, sprayed or settled aerosol, wall deposition,
filter efficiency as a function of particle size, and the pressure history
that decides whether the disks open. That is the CFD-sized problem of
[Step 14](./13-brute-force-cfd.md) again.

**The cheap model the repo has:** one well-mixed volume. For the airborne
inventory $B$ of a nuclide:

$$\frac{dB}{dt} = I - (\lambda + k_{dep} + k_{ex})B$$

$$\text{stack release rate} = (1 - c)k_{ex}B$$

with source $I$ from the primary circuit, decay $\lambda$, deposition
$k_{dep}$, exhaust turnover $k_{ex}$ and filter capture fraction $c$
([`bishan::building`](../../code-review/#crates/bishan/src/building.rs::step)).
It is stepped exactly, so atoms are conserved. The illustration below uses
this closed form for a single puff.

<link rel="stylesheet" href="anim/theory-widgets.css">
<div class="bl-anim" data-anim="building"></div>
<script src="anim/theory-widgets.js"></script>

The dashed curve is **no credit**: no deposition, no filter. The solid
curve takes what the sliders give. The one slider value the repo sources is the
turnover of 1 per day (from the allowable leakage above). The deposition and
filter sliders are **illustrative, not HTR-10 values**: the repo does not have
them. The readout shows how much the no-credit curve over-predicts.

The rest of what the module does not carry, by its own statement: the
pressure of the building, the rupture-disk path and the 22.6 m chimney,
separate cavities, aerosol physics and resuspension.

## 3. What the repo credits, and why not

In the digital twin's default release path the building is **not credited**:
the primary leak goes straight to the stack, with no hold-up, deposition or
filtration. The maintainer's decision on 2026-09-29 (gh:#409) is recorded in
the twin's source and in the `bishan` README: "the reactor-building credit is
deferred", the building control volume stays in the tree, off the default
path
([`fission_product_release.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/fission_product_release.rs)).

The published source does the same. `bishan`'s default `BuildingParameters::htr10()`
sets filter capture to 0 and deposition to 0, citing Liu & Cao (2002): the
filtering is "not taken into account in the conservative calculation", and
"the filtering and plate-out effects in the reactor building are not taken
into account". The published HTR-10 filter decontamination factors are not
available to the repo
([`building.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/bishan/src/building.rs)).

The reactor-comparison pages choose the same, for a different reason: each
side is credited only with its reactor-level barrier (the containment vessel
for the light-water reactor, the TRISO particles for the HTR), and "nothing
surrounding the reactor" is credited on either side
([`bounding_air_ingress.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/bounding_air_ingress.rs)).

## 4. Engineering conservatism as a bounding assumption

> **A bounding assumption is a deliberate choice that errs in one direction.**
> It is not "a cheaper model". It is a model chosen so that the true answer is
> at most the modelled one (for a release), whatever the uncertainty.

**What gets dropped.** Everything that would *lower* the predicted release but
that is hard to prove: building deposition, filter capture, plant-specific
leakage tightness, and more.

**Why a bound is acceptable in a safety argument when a best estimate is not.**
A best estimate needs its uncertainty known, and a claim that the true release
is below some limit needs the *whole* error distribution under it. A bound
sidesteps that: if even the pessimistic number is acceptable, the exact number
need not be known. That is the argument. The repo does not state it as policy;
it is the reasoning behind the conservative choices it records, and any
regulator's actual practice is outside this book
[citation needed: maintainer to supply].

**The price.** The bound over-predicts. The widget above shows the ratio. For
a set of weakly-coupled bounds, the over-prediction multiplies.

**How this differs from the fidelity ladder.** A move *down* the ladder
(CFD to TRISO-ATOPS pools) is chosen because it is cheaper, and it can err
either way. A bounding choice is chosen to err on one side, and it can be
applied at *any* rung. A high-fidelity model with an optimistic input is not
bounding. A crude model with every input pushed the safe way can be.

**A bound must be checked for direction.** An omission is not a bound by
being an omission:

- gh:#446 above: the accident model released **zero** in an isothermal hold. That
  is a model gap in the *non*-conservative direction, not a bound.
- `total_failure_fraction` carries the note that dropping $\varphi_2$ "models
  pressure-vessel failure alone, which is non-conservative above about
  2000 C" ([`mod.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/mod.rs)).
- Swapping the Weibull median convention for the characteristic one shifts the
  strength scale by about 4 % in the flattering direction (Step 7).

## 5. Where the book already makes bounding choices

All recorded in the repo, each with the direction it errs:

| Where | Choice | Direction |
|---|---|---|
| Step 13 | Leak 1 % of the volume per day, "specified to be below" it (Liu & Cao 2002 s2.4.2) | an allowance, high |
| Step 13 | Purification efficiencies "conservatively set" (99 %, 90 %) | per the source; efficiency high, so *low* removal needs a check [citation needed: maintainer to supply] |
| Step 13 | Plate-out per cycle "conservatively taken" against AVR's 50 to 90 % (Liu & Cao) | per the source |
| Step 13 | Desorption at 2.4 times coolant activity "might be overestimated by a factor of 3", not applied | high release |
| Step 7 | PANAMA-I is "a conservative design model, and over-predicting failure is the direction a safety code is built to err in" (repo's `qualification.rs`) | high failure |
| Step 7 | The with-corrosion variant "covers the measured values in a conservative approximation" (the report, as quoted in `history.rs`) | high failure |
| Step 16 | Circulating activity leaves all at once | high release |
| Step 17 | Building and filter not credited (gh:#409; Liu & Cao) | high release |
| Air-ingress case | Every particle exposed to air at 1400 C for 140 h | assumed worst |
| Dispersion | Worst stability class, 1 m/s, ground release, 96 h (Step 19) | high concentration |

Two notes. The purification row: higher purification efficiency *lowers*
circuit inventory, so "conservative" in the source's sense is a statement about
what they did, and the direction for each downstream quantity needs checking.
And the 1 % leak is an *allowance* of the design, so the model uses a design
limit, which is a bound only if the plant's real leakage is below it.

## Why each simplification is allowed

**Kinds of justification used below.** (a) negligible effect, with an estimate; (b) computationally too expensive to be practical; (c) bounding or conservative, with the side it errs on; (d) data not available. **Open** means this page cannot justify it.

| Simplification | Kind | Justification |
|---|---|---|
| One well-mixed volume | (b) | A multi-cavity flow and aerosol model is costly |
| No deposition or filter credit | (c) | Errs high on release; and (d): HTR-10 filter factors are unavailable to the repo |
| Rupture-disk path, chimney and pressure not carried | (d) | Not in the module; effect **open** |

## What we assumed

- The one building described is HTR-10's vented confinement; nothing here
  applies to another design without a source.
- The cheap building model is one well-mixed volume with constant rates.
- A bound's value is that its direction can be argued; a gap is not a bound.

## Next question

Activity that leaves the building enters the air. How does it travel, and what
is the cheapest model that still says something useful?
[Step 18](./17-dispersion-reality.md).
