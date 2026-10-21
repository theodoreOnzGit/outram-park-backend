# Step 19 — The Gaussian puff, then the Gaussian plume

> **Research, education and V&V only.** Not for reactor operation, licensing, safety decisions or emergency response.
> **Review status: first draft, 2026-10-18, AI-assisted (Claude Sonnet), not yet reviewed by a human.**

**The question this page answers.** [Step 18](./17-dispersion-reality.md)
showed that the faithful models need weather fields, fine meshes and long
runs. *What is the cheapest model that still shows a cloud moving, and what
is the cheapest that gives a number at a distance?*

The details, the derivations and the interactive parts are in `changi`'s
dispersion lessons and the dispersion demo. This page is the view from the
fidelity ladder. It links to them and does not repeat them.

| Model | Where it is taught | Demo |
|---|---|---|
| Gaussian **puff** train | [rung 4](../../deep-dives/dispersion/rungs/04-puffs.html), [puff](../../deep-dives/dispersion/puff.html), [puff train](../../deep-dives/dispersion/puff-train.html) | [puffs](../../demos/dispersion/?rung=puffs) |
| Gaussian **plume** | [rung 1](../../deep-dives/dispersion/rungs/01-plume.html), [plume](../../deep-dives/dispersion/plume.html), rungs [2](../../deep-dives/dispersion/rungs/02-sigmas.html) and [3](../../deep-dives/dispersion/rungs/03-rise-wake.html) | [plume](../../demos/dispersion/?rung=plume) |
| Deposition and dose on top | rungs [5](../../deep-dives/dispersion/rungs/05-deposition.html) and [6](../../deep-dives/dispersion/rungs/06-dose.html) | [deposition](../../demos/dispersion/?rung=deposition), [dose](../../demos/dispersion/?rung=dose) |
| The whole chain, bounding | [rung 7](../../deep-dives/dispersion/rungs/07-capstone.html) | [capstone](../../demos/dispersion/?rung=capstone) (recorded results, not live) |
| The model above both | [random walk side path](../../deep-dives/dispersion/rungs/side-random-walk.html) | none |

## 1. The first simplification: from FLEXPART to a puff

**What is dropped relative to Step 18.** Gridded weather, the turbulent
velocity process, and mass-by-particle bookkeeping. In their place:
a release is cut into a train of *puffs*. Each puff is one Gaussian blob of
mass $Q$ at height $H$. It travels with the wind sampled when it was emitted.
Its width grows with the distance it has travelled by Pasquill–Gifford
sigma curves for the stability class at emission. At a receptor the
contributions of all puffs add. A mirror image below the ground keeps mass
from crossing it
([puff internals](../../deep-dives/dispersion/puff.html)).

The kernel is one closed-form expression, so it is cheap enough to run live on
a phone.

**The judgement that makes it acceptable.** The sigma curves are empirical
fits to field experiments. Over a site-sized domain the cloud's spread is
mostly set by the stability class, and not by the details of the wind field.
`changi`'s own survey shows the point: at the same 1 m/s wind, a clear night
reads about 77 times a sunny afternoon at a receptor, because of the stability
class and not the wind speed (from the crate's README, as recorded there).

**What the puff model keeps that the plume model does not.** The wind can
start, stop and turn. A release of twenty minutes with the wind swinging
halfway can be represented, and the steady plume cannot.

**Where it breaks.**

- The sigma fits are meaningful over roughly 0.1 to 10 km. Upstream and the
  port apply them at any distance, without a clamp
  ([`puff/dispersion.rs`](../../code-review/#crates/changi/src/puff/dispersion.rs::pasquill_gifford_sigmas)).
- A single stability class per puff, and no terrain.
- Wind veer or shear with height, and rain, are not represented.
- It is a model of dry air. Wet deposition is not in the puff model.

**The check.** The puff train is compared with the plume in the limit where
they must agree (constant wind, one class). All 21 gated points pass, and the
measured difference matches a second-order prediction
(`buangkok/tests/changi_puff_train_vs_plume.rs`, recorded 2026-09-29 and
re-run 2026-10-04 in [rung 4](../../deep-dives/dispersion/rungs/04-puffs.html)
for example class D at 5.0 km, measured $-6.878 \times 10^{-4}$ against
predicted $-6.876 \times 10^{-4}$). That is verification that two ports of the
same Gaussian physics agree. It says nothing about real air.

## 2. The second simplification: from the puff to the plume

**What is dropped relative to the puff.** Time. The wind is steady and
constant in direction, the release is continuous, and the answer is a
concentration field that does not change in time. The plume is the solution
of the steady advection–diffusion equation with one diffusivity per direction
(rung 1, steps 1 and 2): carrying by the wind beats spreading along the wind,
so the plume is *slender*, and the result is

$$\frac{\chi}{Q} = \frac{1}{2 \pi \sigma_y \sigma_z u}\ \exp\left(-\frac{y^2}{2\sigma_y^2}\right) \left[\exp\left(-\frac{(z-H)^2}{2\sigma_z^2}\right) + \exp\left(-\frac{(z+H)^2}{2\sigma_z^2}\right)\right]$$

At ground level on the centreline this reduces to
$\chi / Q = \exp(-H^2 / 2\sigma_z^2) / (\pi \sigma_y \sigma_z u)$.
The code that evaluates it is
[`master_equation_single_plume`](../../code-review/#crates/buangkok/src/pydoseia/dispersion.rs::master_equation_single_plume).
The page's own comment bears repeating: nothing in the workspace *solves* the
partial differential equation. The code evaluates its solution.

**What the plume buys.** One number per distance, independent of the source
size. $\chi / Q$ depends on the geometry, the wind and the stability class
alone. So a bound on the dilution can be set once and reused for any release.
The `htgr_sim_v1` dispersion module makes the same point: every uncertainty
about inventories and leaks cancels out of $\chi / Q$
([`atmospheric_dispersion.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/atmospheric_dispersion.rs)).

**Where it breaks.** A turning wind, a short release, calm or very light wind,
terrain, and any distance outside the range of the sigma fits. Plume rise and
building wake are in `buangkok` but unverified against their cited sources and
unused in any dose path (gh:#542).

## 3. Using it for a bound: the conservative choices

The plume is the model chosen for the repo's bounding air-ingress case. Its
choices are written down in
[rung 7](../../deep-dives/dispersion/rungs/07-capstone.html). From that page:

| Choice | Direction of the error | Size |
|---|---|---|
| Largest $\chi / Q$ over the six stability classes, at each distance ("worst class") | over-states | Rung 2's spread between classes shows the size |
| Wind of 1 m/s at 10 m | over-states | not measured on this page |
| One direction for 96 h, receptor on the centreline | over-states | not measured on this page |
| Ground-level release, no plume rise, no building wake | over-states for an elevated hot release, either way for a ground release | not measured on this page |
| No depletion and no decay in transit | over-states far downwind | not measured on this page |
| Whole core at 1400 C for 140 h upstream of it | over-states the release | "by a factor not measured here" |

**Not conservative, stated by the repo.** The deposition on top of this is dry
only. In the deposition rung's own words: "No wet scavenging, so the answer is
**not an upper bound**: rain would raise it". And the same rung notes that
because deposited activity is not removed from the plume, far-downwind air
concentration and deposition are over-predicted. So the chain is conservative
in some links and not in others, and a reader must not call the whole thing
"bounding" without saying which links.

Also **under-counted, stated** in the rung 7 record: ingestion, plume shine,
short-lived nuclides screened out, and three nuclides TRISO-ATOPS lacks.

This is the same logic as the building page: the choices are made to err high,
not just to be cheap, and the price is over-prediction. See
[Step 17](./16-building-and-bounds.md).

## 4. Dispersion is the third worked example of the ladder

| Level | Model | What is dropped on the way down |
|---|---|---|
| Fewest assumptions | CFD or LES of the atmosphere (not built) | nothing |
| Lagrangian | FLEXPART kernels (`changi::flexpart`, verified kernels, no weather reader, no driver) | the resolved flow |
| Cheap, time-varying | Gaussian puff train (`changi::puff`) | weather fields, turbulence process, particles |
| Cheapest | Gaussian plume (`buangkok::pydoseia`) | time dependence |

[Step 20](./19-levels-of-fidelity.md) puts this next to the particle and the
coolant ladders. The part to take from it: what the cheaper models drop is
exactly what the fewest-assumption models spend their cost on.

## What we assumed

- Pasquill–Gifford sigma fits describe real spreading over 0.1 to 10 km.
- A stability class summarises the turbulence at a site and time.
- For the bounding case: the worst class, a low steady wind and the
  centreline receptor over-state the concentration. The conservative
  direction is argued by the repo, not measured here.
- Sea and groundwater are not part of these two models.

## Next question

One ladder per link of the chain has now been shown. What do the ladders have
in common, and how is a bounding choice different from a cheaper one?
[Step 20](./19-levels-of-fidelity.md).
