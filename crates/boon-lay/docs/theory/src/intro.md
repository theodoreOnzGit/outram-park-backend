# How this book works

> **Research, education and V&V only.** Not for reactor operation, licensing, safety decisions or emergency response.

> **Review status: first draft, 2026-10-18, AI-assisted (Claude Sonnet), not yet reviewed by a human.**

*You may have arrived from the
[TRISO-ATOPS ladder](../../tutorials/triso-atops/intro.html), which teaches
the crate rung by rung through code. This book asks a different question:
what is the physics behind fission product transport, and which
simplifications are we making when we compute it?*

## The question

A fission inside a fuel particle makes new atoms. Some of them are
radioactive, and a few of those can leave the particle. To say how many leave,
and when, we need a model. **Which model, and how much can we throw away
before it stops being true?**

## The ladder: fewest assumptions first

Each page starts from the model with the **fewest assumptions**. That model is
the most intuitive ("follow every atom, solve the full problem") and the most
expensive. Each following section then makes one simplification, and for each
we write down:

1. what is thrown away;
2. **the judgement that makes it acceptable**, which is the part a computer
   cannot do for you;
3. when the simplification fails.

This is the same idea as the nuclear data lessons, where the book walks from a
resonance parameter down to the table a transport code reads. Here it walks
from "every atom and every collision" down to the formulas in this crate.

A page ends with **What we assumed** (a short list) and **Next question**.

## The particle in one picture

A TRISO particle is five concentric spheres, under a millimetre across
(HTR-10 values, from
[`fuel_failure::htr10`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/htr10/mod.rs),
transcribed from IAEA-TECDOC-1382 part 2, Table 4-17, as the
[rung 1](../../tutorials/triso-atops/triso.html) page records;
International Atomic Energy Agency, 2003, *Evaluation of High Temperature Gas Cooled Reactor Performance*, IAEA-TECDOC-1382, as cited in the repo):

```text
   centre                                          outside
     |                                                |
     v                                                v
   +--------+--------+-------+-----+--------+
   | kernel | buffer | IPyC  | SiC |  OPyC  |
   | UO2    | porous | dense | SiC | dense  |
   |        | carbon | PyC   |     | PyC    |
   +--------+--------+-------+-----+--------+
   0       250      340     380   415      455   radius, micrometres
```

| Layer | Outer radius (µm) | Thickness (µm) | Job |
|---|---|---|---|
| Kernel (UO₂) | 250 | 250 (radius) | the fuel; products are born here |
| Buffer | 340 | 90 | room for gas; stops recoiling fragments |
| IPyC | 380 | 40 | seals the kernel, base for the SiC |
| SiC | 415 | 35 | pressure vessel and main barrier |
| OPyC | 455 | 40 | protects the SiC |

The radii are the same ones the cascade probe uses (interfaces at 0, 40, 75,
115, 205 and 455 µm when counted inward from the outer surface; see the
[RustBCA cascade record](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/verification_and_validation/rustbca_cascades/README.md)).
The CRP-6 benchmark particle is a little smaller (212.5 to 427.5 µm); see
[rung 1](../../tutorials/triso-atops/triso.html) for both. The buffer is
"about 50 % voids" in that page's words; the exact porosity is not needed here.

## The map of the twenty-one steps

~~Twenty steps, with diffusion before pressure.~~ **REORDERED 2026-10-21**
(maintainer direction): the middle of the book is now one fidelity ladder,
from high fidelity down, then boon-lay's novelty. The page files keep their
old names so links still work; only the step numbers moved.

```text
 INSIDE THE PARTICLE
   1  Generation      fission yields, decay, burnup, (n,gamma), (n,2n)
   2  Recoil          fragments stop in kernel or buffer
   3  Layers          diffusion, and why chemistry makes layers differ
   4  Silicon carbide the barrier that matters most
 HOW WE SIMULATE THE PARTICLE: FROM HIGH FIDELITY DOWN
   5  High fidelity   fuel performance by FEM, as BISON does (not built here)
   6  Pressure vessel gas, pressure and stress
   7  PANAMA          fuel failure: its simplification, and ours
   8  Random walk     normal diffusion, one atom at a time (Lagrangian)
   9  Eulerian view   normal diffusion, FRESCO-style; Booth
  10  Ray's equations  boon-lay's novelty: between the walk and the mesh
  11  boon-lay's aim  after PANAMA and FRESCO: STACY in HCP, and one tool
 OUT OF THE PARTICLE
  12  Coolant         fission products, tramp uranium and dust
  13  Removal         purification, leak, plate-out and lift-off
  14  CFD + coeffs    the serious route (not built here)
  15  TRISO-ATOPS     lumped transport: the next best, and what we use
  16  DLOFC           a loss-of-cooling accident, and what is left out
  17  Building        outside the vessel, and bounding assumptions
  18  Dispersion      how a release travels, and what it costs to compute
  19  Puff and plume  the Gaussian puff, then the Gaussian plume
  20  Fidelity        levels of fidelity
  21  htgr_sim_v1     the whole chain in one worked example, and a review route
```

| Step | The question it answers | Fewest-assumptions model | Cheaper model |
|---|---|---|---|
| 1 | Which atoms exist, and how many? | full depletion of every nuclide | chosen nuclides, fixed flux |
| 2 | Where does a new atom start? | track each fragment's slowing down | recoil fraction from geometry |
| 3 | How does it move through each layer? | transport with the real chemistry | one Arrhenius law per layer |
| 4 | Why is SiC the barrier? | same, with SiC's physics | a diffusivity contrast |
| 5 | How is a particle simulated at high fidelity? | FEM fuel performance (BISON-style) | (the next steps) |
| 6 | What does the gas do to the shell? | finite elements | thin-shell membrane stress |
| 7 | How did PANAMA simplify failure? | | PANAMA-I formulas |
| 8 | How do we compute diffusion? | random walk of each atom | (next step) |
| 9 | Can we skip the atoms? | field equation on a mesh (FRESCO) | Booth equivalent sphere |
| 10 | What does this crate add? | Ray's equations | |
| 11 | What comes next? | | |
| 12 | What is in the coolant, and where from? | transport equation per species | groups, tramp-uranium fraction |
| 13 | How does it leave the coolant? | wall sorption and flow physics | first-order rates per group |
| 14 | What would a serious analysis solve? | CFD plus coefficients for every species (not built) | (the cost argument) |
| 15 | What is the next best thing, and what do we use? | | TRISO-ATOPS lumped pools |
| 16 | What happens in a loss-of-cooling accident? | compressible blowdown with chemistry | venting fraction and lift-off fraction |
| 17 | What is outside the vessel? | flow and aerosol per cavity | one well-mixed volume, or no credit |
| 18 | How does a release travel? | weather-driven CFD or particles | (the cost argument) |
| 19 | What is the cheapest cloud model? | | Gaussian puff, then plume |
| 20 | What is the pattern? | | levels of fidelity, and bounds |
| 21 | Which rung does one real chain stand on? | | the htgr_sim_v1 chain, with a review route |

## One thing to keep in mind throughout

**Transmutation is not modelled while fission products move through the
layers.** Step 1 describes transmutation as the physics demands. The crate's
walker can transmute one target by one explicit `(n,γ)` channel, and the
diffusion engine and the depletion engine are separate. Step 11 returns to
this as a stated simplification.

## What we assumed (for this page)

- The HTR-10 radii are transcribed from the cited table; no measurement checks
  them.
- Spherical, concentric layers (real particles are slightly aspherical).

## Next question

[Step 1](./01-generation.md): before anything moves, which atoms are there?
