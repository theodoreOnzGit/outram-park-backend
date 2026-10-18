# Step 13 — The brute-force model: CFD for every species

> **Research, education and V&V only.** Not for reactor operation, licensing, safety decisions or emergency response.
> **Review status: first draft, 2026-10-18, AI-assisted (Claude Sonnet), not yet reviewed by a human.**

**The question this page answers.** [Step 12](./12-escape-and-removal.md)
described removal with a few first-order rates. *If we refused to assume
those rates and computed the transport directly, what would we have to solve,
and why is that too expensive to do for every case we care about?*

This is the model with the **fewest assumptions** for the circuit. It is also
the reference against which the cheap model of [Step 14](./14-triso-atops.md)
is a simplification, so we need its shape clearly.

## 1. What it would have to solve

The circuit is a network of flow paths: core, ducts, circulator, steam
generator, purification branch. A direct model puts a mesh on all of it and
solves the following, together.

**1. The helium flow.** Mass, momentum and energy for a compressible gas
through the pebble bed and the open ducts, with turbulence:

$$\frac{\partial \rho}{\partial t} + \nabla \cdot (\rho \mathbf{u}) = 0$$

$$\frac{\partial (\rho \mathbf{u})}{\partial t} + \nabla \cdot (\rho \mathbf{u}\mathbf{u}) = -\nabla p + \nabla \cdot \boldsymbol{\tau} + \mathbf{f}_{bed}$$

plus an energy equation and a turbulence closure. The term $\mathbf{f}_{bed}$
is the drag of the packed bed on the gas.

**2. One transport equation per species group.** For each nuclide, or each
chemical group of nuclides, the equation of Step 11:

$$\frac{\partial (\rho Y_s)}{\partial t} + \nabla \cdot (\rho \mathbf{u} Y_s) = \nabla \cdot (\rho D_s \nabla Y_s) + S_s - \lambda_s \rho Y_s + \sum_p \lambda_p \rho Y_p - R_s$$

with $Y_s$ the mass fraction. TRISO-ATOPS carries a table of 84 nuclides
([`supported_nuclides`](../../api/boon_lay/triso_atops_fork/nuclide_model/nuclide_database/fn.supported_nuclides.html)),
so this is of the order of 84 coupled equations, linked by decay chains.

**3. Aerosol and dust transport and deposition.** Dust is a population of
particles of different sizes. It needs its own size-resolved transport, with
deposition by several mechanisms and resuspension by shear. That physics is
general knowledge, and **the repo contains none of it**
[citation needed: maintainer to supply].

**4. Wall sorption.** The removal term $R_s$ for a metal or a halogen is a
surface process: adsorption and desorption on the wall, with a rate that
depends on the species, the surface material, its temperature and its oxide
or deposit layer, and with diffusion into the wall. This needs a sorption
model and data per element and surface
[citation needed: maintainer to supply]. The repo has one number per group in
its place.

**5. Decay.** Included in the species equation above, and the heat it
deposits feeds the energy equation.

**6. In an accident, all of this changes in time.** The blowdown of
[Step 15](./15-dlofc.md) is a fast compressible transient: pressure falls,
gas accelerates, surfaces see high shear, and the flow may reverse.

## 2. Why it is cumbersome

1. **Many scalars.** One flow solve is expensive; 84 more transport equations
   on the same mesh multiply the memory and the work by a large factor, and
   they are coupled through decay.
2. **Very different time scales.** Gas crosses the circuit in seconds. Plate-out
   fills over years. Half-lives run from fractions of a second to decades. A
   single time step cannot serve all of them, which is the *stiffness* that
   makes a burnup matrix hard (see the crate's
   [`README`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/lagrangian_transmutation_and_fission_simulator/README.md)).
3. **Missing data.** The sorption chemistry, the dust, and the gas-surface
   temperatures along the circuit are the inputs the model most needs and the
   ones the repo lacks.
4. **Geometry.** A pebble bed is millions of spheres. Meshing it resolved is
   out of reach, so even the "first-principles" model uses a porous-media
   closure for the bed, which is itself a model.
5. **Cost, with the arithmetic shown.** This is an illustration, not a
   benchmark. Take $10^{7}$ cells (a modest 3D mesh), 84 species, and a
   30 h transient (the length of the DLOFC transient that the repo's
   tutorial cites) at a 1 s step:

$$10^{7} \times 84 \approx 8.4 \times 10^{8}\ \text{unknowns per step}$$

$$30 \times 3600\ \text{s} / 1\ \text{s} = 1.08 \times 10^{5}\ \text{steps}$$

$$8.4 \times 10^{8} \times 1.08 \times 10^{5} \approx 9 \times 10^{13}\ \text{cell-species updates}$$

Even at $10^{8}$ updates per second on one core that is about
$9 \times 10^{5}$ s, roughly ten days, for *one* case with *no* chemistry
beyond first order. Parameter studies, uncertainty, or a pebble-by-pebble
bed multiply this. The step size, cell count and update rate are
assumptions chosen for the arithmetic, not measurements.

## 3. What the workspace has

The building blocks for a direct model exist in pieces: OpenFOAM-style
finite-volume operators and a k-omega SST turbulence closure in
`outram-foam-*`, and the thermal-hydraulic framework `tampines`
([members list](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/docs/claude-md/members-and-dependencies.md)).
I did not find a nuclide-group transport solver for a primary circuit, with
sorption and dust, assembled from them. It is **not built**.

## 4. Hence cheaper models

Each simplification below throws away detail and keeps something. What
it keeps must be said, because that is the model's whole claim:

| Dropped | Kept |
|---|---|
| The flow field, position in the circuit | One well-mixed volume per nuclide |
| Wall chemistry in detail | One first-order rate per group |
| Dust size and transport | Nothing, or a fixed share |
| The 3D bed | A node per core region |
| Compressible blowdown | A fraction vented by gas expansion |

The result is a short list of ordinary differential equations. That is
[Step 14](./14-triso-atops.md).

## Why each simplification is allowed

**Kinds of justification used below.** (a) negligible effect, with an estimate; (b) computationally too expensive to be practical; (c) bounding or conservative, with the side it errs on; (d) data not available. **Open** means this page cannot justify it.

| Dropped | Kind | Justification |
|---|---|---|
| Position in the circuit | (b) | Cost argument above: about $9 \times 10^{13}$ cell-species updates per case |
| Dust size resolution | (d) | Not in the repo |
| Wall sorption in detail | (d) | Not in the repo |
| Compressible blowdown | (b) | Fast transient with many species |

## What we assumed

- A porous-media model of the bed stands in for the real pebbles.
- Sorption can be written as a wall source term once its rate is known.
- The cost estimate uses illustrative sizes, labelled as such.

## Next question

What does the cheap model look like, who built it, and how far can its
numbers be trusted for a reactor it was not built for?
[Step 14](./14-triso-atops.md).
