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

## The map of the ten steps

```text
 INSIDE THE PARTICLE
   1  Generation      fission yields, decay, burnup, (n,gamma), (n,2n)
   2  Recoil          fragments stop in kernel or buffer
   3  Layers          diffusion, and why chemistry makes layers differ
   4  Silicon carbide the barrier that matters most
 HOW WE COMPUTE DIFFUSION
   5  Random walk     one atom at a time (Lagrangian)
   6  Eulerian view   the aggregate; Booth; FRESCO
   7  Ray's equations how this crate does it
 WHEN THE PARTICLE IS PRESSURISED
   8  Pressure, stress   first principles (farrer-park)
   9  PANAMA          its simplification, and ours
  10  STACY in HCP    what comes after, and what boon-lay aims for
```

| Step | The question it answers | Fewest-assumptions model | Cheaper model |
|---|---|---|---|
| 1 | Which atoms exist, and how many? | full depletion of every nuclide | chosen nuclides, fixed flux |
| 2 | Where does a new atom start? | track each fragment's slowing down | recoil fraction from geometry |
| 3 | How does it move through each layer? | transport with the real chemistry | one Arrhenius law per layer |
| 4 | Why is SiC the barrier? | same, with SiC's physics | a diffusivity contrast |
| 5 | How do we compute the motion? | random walk of each atom | (next two steps) |
| 6 | Can we skip the atoms? | field equation on a mesh | Booth equivalent sphere |
| 7 | What does this crate do? | Ray's equations | |
| 8 | What does the gas do to the shell? | finite elements | thin-shell membrane stress |
| 9 | How did PANAMA simplify? | | PANAMA-I formulas |
| 10 | What comes next? | | |

## One thing to keep in mind throughout

**Transmutation is not modelled while fission products move through the
layers.** Step 1 describes transmutation as the physics demands. The crate's
walker can transmute one target by one explicit `(n,γ)` channel, and the
diffusion engine and the depletion engine are separate. Step 10 returns to
this as a stated simplification.

## What we assumed (for this page)

- The HTR-10 radii are transcribed from the cited table; no measurement checks
  them.
- Spherical, concentric layers (real particles are slightly aspherical).

## Next question

[Step 1](./01-generation.md): before anything moves, which atoms are there?
