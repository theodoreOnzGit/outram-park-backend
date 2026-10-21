# Step 20 — Levels of fidelity

> **Research, education and V&V only.** Not for reactor operation, licensing, safety decisions or emergency response.
> **Review status: first draft, 2026-10-18, AI-assisted (Claude Sonnet), not yet reviewed by a human.**

**The question this page answers.** Every step of this book did the same
thing: start from the model with the fewest assumptions, then give up detail
to make the calculation bearable. *What is that pattern, how do we choose a
level, and how does it differ from a bound?*

## 1. Fidelity is a ladder, and a rung is a choice of what to keep

A **level of fidelity** is a choice of which physics to resolve and which to
replace with a number. The top rung resolves almost everything and costs the
most. Each rung down replaces something resolved with something assumed.

Three things come with every rung, and a page that gives a cheap model should
give all three:

1. **What was dropped.**
2. **The judgement** that makes dropping it acceptable, stated so a reader can
   disagree with it.
3. **Where it fails**, so a user knows when to climb back up.

The judgement in (2) is one of four kinds, and each page of this book names
which: **(a) negligible effect**, with an estimate; **(b) computationally too
expensive to be practical**, with the cost argument; **(c) bounding or
conservative**, with the side it errs on; **(d) data not available**. If none
of the four can be argued, the page says **open**. Calling something negligible
without an estimate is not allowed here.

**The point of the book is this pattern. It is not any one model.**
TRISO-ATOPS ([Step 15](./14-triso-atops.md)) is one example. boon-lay fuel
failure ([Step 7](./09-panama.md)) is another. Dispersion
([Step 19](./18-puff-and-plume.md)) is a third.

## 2. The whole book on one ladder

Each row: the fewest-assumptions model, the cheap one, and what the move
costs. Steps are as written in this book.

| Link | Fewest assumptions | Cheap model | Step |
|---|---|---|---|
| Generation | Track the full nuclide network | One atom at a time; decay only, fixed flux | 1 |
| Recoil | Slow down every fragment | A recoil fraction from geometry | 2 |
| Diffusion in the layers | Diffusion with the real chemistry | One Arrhenius $D(T)$ per layer and element | 3 |
| Transport through the particle | Follow every atom (fixed step) | Walk-on-Spheres, then aggregate closed forms | 8 to 10 |
| Pressure and stress | Finite-element particle (not built; Step 5) | Ideal gas, thin SiC shell | 5 to 7 |
| Strength and failure | Per-particle flaws | One Weibull law over the population | 7 |
| Coolant transport and removal | CFD for every species (not built) | Lumped pools, first-order rates | 12 to 15 |
| Accident release | Compressible blowdown with chemistry | Venting fraction and lift-off fraction | 16 |
| Building | Flow and aerosol per cavity | One well-mixed volume, or no credit | 17 |
| Atmosphere | Weather-driven CFD or particles | Gaussian puff, then Gaussian plume | 18, 19 |

On each row the cost falls by orders of magnitude, and the capability falls
too. The cheap models give a number in seconds. They cannot say anything the
dropped physics would have said.

## 3. Three worked examples

### boon-lay fuel failure (Step 7)

- **Top:** a finite-element model of the particle with creep and swelling.
  `farrer-park` does not have it yet.
- **Kept:** the SiC as a thin pressure vessel, one hoop stress, a Weibull law
  over the population, and an Arrhenius action integral for decomposition.
- **Dropped:** the PyC load sharing, contact, per-particle geometry.
- **Judgement:** the SiC carries the pressure load; strength scatter, not
  detail, sets a failure *fraction*.
- **Fails:** thick layers, cracked PyC, and anywhere outside the fitted range
  of the report's correlations.

### TRISO-ATOPS (Step 15)

- **Top:** CFD with one equation per species and wall sorption.
- **Kept:** first-order pools per core node, closed-form release, venting.
- **Dropped:** position in the circuit, wall chemistry, dust, ingress.
- **Judgement:** the rates carry the net effect of the dropped physics.
- **Fails:** an isothermal hold releases zero, because the only exit in the
  model is expansion on heating (gh:#446).

### Dispersion (Steps 18 and 19)

- **Top:** weather-driven particle or CFD models. In the repo, a verified set
  of FLEXPART kernels, with no driver that runs them.
- **Middle:** the Gaussian puff, which follows a changing wind.
- **Kept at the bottom:** the Gaussian plume: a steady wind, a fixed
  stability class.
- **Judgement:** for a screening answer, a steady plume gives the right order
  of magnitude at the chosen receptor.
- **Fails:** when the wind turns, the terrain is complex, or a puff has not
  yet reached steady state. The recorded check of puff against plume is in
  Step 19.

## 4. Fidelity is not the same as bounding

This is the point to take away from [Step 17](./16-building-and-bounds.md).

| | Fidelity ladder | Bounding assumption |
|---|---|---|
| Chosen for | Cost | Direction of error |
| Errs | Either way | One way, on purpose |
| Applies at | A move between rungs | Any rung |
| Checked by | Comparison against a higher rung | An argument that the direction holds |
| Price | Lost detail | Over-prediction |

They combine. A crude model with every input pushed the safe way is a bound.
A high-fidelity model with one optimistic input is neither. A gap is not a
bound: the zero release of an isothermal hold is a hole in the non-conservative
direction (gh:#446).

Dispersion shows both at once. The plume is a cheap model (a ladder step)
*and* the repo uses it with bounding choices (the worst stability class,
1 m/s, ground release, a receptor on the centreline, 96 h), which are
bounding steps on top of it (Step 19).

## 5. Choosing a level

Ask in order:

1. **What question is the number for?** A screening comparison needs
   different fidelity from a design choice.
2. **What does the next rung up say that this one cannot?** If the answer is
   "nothing relevant", stay.
3. **Can the cheap model be checked against the dearer one on a case where
   both run?** The repo records some such checks: the Walk-on-Spheres engine
   against the analytic CRP-6 release solution, the puff train against the
   plume (21 of 21 gated points, Step 19), the port of TRISO-ATOPS against
   upstream. It records no check of the lumped pools against a CFD model, because
   no CFD model of the circuit exists. A cheap model with no dearer partner
   has only its inputs to stand on.
4. **Which direction should the remaining error go?** If a bound is wanted,
   choose it explicitly and record it.

## 6. What the ladder does not give you

- **Validation.** Moving up or down is verification of structure. Whether any
  rung describes a real reactor is a separate test against measurement, and
  the repo records none for the chain in this part of the book.
- **A single "right" level.** The right one depends on the question.
- **Free uncertainty.** A cheap model has hidden error that the dearer model
  would have shown.

## What we assumed

- Each rung's dropped physics can be named.
- "Cheaper" and "less capable" move together.
- A bound is argued, and an argued direction can still be wrong.

## Next question

All of this has been shown piece by piece. What does the whole chain look like
in one worked program, and in which order should a reviewer read it?
[Step 21](./20-htgr-sim-v1-chain.md).
