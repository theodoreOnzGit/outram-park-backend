# Step 18 — Beyond the site: how a release travels, and what it costs to compute

> **Research, education and V&V only.** Not for reactor operation, licensing, safety decisions or emergency response.
> **Review status: first draft, 2026-10-18, AI-assisted (Claude Sonnet), not yet reviewed by a human.**

**The question this page answers.** Activity that has left the building
(Step 17) enters the environment: air first, then land, groundwater and sea.
*How does it travel, which equations describe that with the fewest
assumptions, and why can we not simply solve them?*

**The question from the previous step.** [Step 17](./16-building-and-bounds.md)
ended with a release rate at the stack or building exit. This page asks where
it goes next. Details and the interactive part are in `changi`'s own
dispersion lessons; this page is the theory ladder that links to them and does
not repeat them
([the dispersion deep dive](../../deep-dives/dispersion/)).

## 1. What happens in reality

Read this list as the physics a complete model would have to carry. The repo
does not source most of it, so no number is attached.

- **Advection.** The wind carries the released air. Its direction and speed
  change with time and height.
- **Turbulent mixing.** Eddies spread a cloud sideways and vertically. How
  strongly depends on how the air is stratified, which meteorologists sort
  into stability classes (Pasquill classes A to F in the repo's models).
  A clear night gives weak mixing, and a sunny afternoon strong mixing.
- **Plume rise.** A hot or fast release rises before it levels off, which
  moves the effective release height up.
- **Building wake.** A building makes a turbulent wake that can pull a
  low-level release down towards the ground.
- **Dry deposition.** Gases and particles settle or stick to the ground and
  vegetation, and leave the air.
- **Wet deposition.** Rain washes activity out of the air onto the ground.
- **Radioactive decay in flight.** Short-lived nuclides are gone before they
  arrive.
- **Ground and groundwater.** Deposited activity may stay on the surface, move
  with runoff, or infiltrate and then move with groundwater, with chemistry
  (sorption, precipitation) setting its speed.
- **Sea.** A release over or into the sea is carried by currents and mixed by
  the water's own turbulence.

General physics, no repo source for the details [citation needed: maintainer
to supply]. What the repo *implements* of each item is in section 3.

## 2. The equations, fewest assumptions first

### Level 0: the advection–diffusion equation

For the concentration $C(\mathbf{x}, t)$ of one species in a flowing medium:

$$\frac{\partial C}{\partial t} + \nabla \cdot (\mathbf{u} C) = \nabla \cdot (K \nabla C) + S - \lambda C - \Lambda C$$

with wind or current $\mathbf{u}$, eddy diffusivity $K$, source $S$, decay
$\lambda$ and a scavenging rate $\Lambda$ (deposition, washout). This is the
same equation as Step 12's coolant equation, with air in place of helium.
The assumption is that turbulence acts like a diffusivity $K$. `changi`'s
steady-plume page starts here and derives the Gaussian from it
([rung 1, steps 1 and 2](../../deep-dives/dispersion/rungs/01-plume.html)).

### Level 1: CFD or large-eddy simulation of the atmosphere

Do not model turbulence with one $K$. Resolve the large eddies on a 3-D mesh
with time-stepping, over terrain and buildings, driven by weather. It carries
the fewest assumptions about the *flow*. This workspace has finite-volume
CFD building blocks (the `outram-foam-*` crates) but no atmospheric dispersion
case built on them, and I found none in the dispersion crates.

### Level 2: Lagrangian particle dispersion (FLEXPART)

Release many computational particles. Move each one with the mean wind from
gridded weather plus a random turbulent velocity (a Langevin process), and
remove mass by decay, dry and wet deposition. The spread of the cloud
*emerges* from the turbulence statistics instead of being fitted
([side path, the random walk](../../deep-dives/dispersion/rungs/side-random-walk.html)).
This is the same random-walk idea as Step 8, with wind and turbulence in place
of diffusion in a particle.

### Level 3: subsurface flow and reactive transport (PFLOTRAN)

For groundwater: Richards flow in variably saturated soil, solute transport,
aqueous chemistry and mineral kinetics on a 3-D mesh.

## 3. What the repo actually has for each

| Piece | State, from its own docs and code |
|---|---|
| **`changi::flexpart`** | 34 modules, **kernels** of FLEXPART v10.4 (commit `3d7eebf`), verified one by one against compiled Fortran. The particle step, Hanna turbulence, dry and wet deposition and OH chemistry are ported. **The GRIB and NetCDF readers and file writers are not**, so it cannot be pointed at real weather. Its own doc says: "a verified set of its kernels", not "FLEXPART in Rust". **No driver runs it**: nothing in the dispersion chain, the examples or `htgr_sim_v1` runs FLEXPART |
| **`changi::puff`** | A complete port of the physics of the R package `puff` 0.1.1: Pasquill stability, Pasquill–Gifford sigmas, the Gaussian puff kernel, sensor and grid modes. Plots not ported. Verified code to code |
| **`changi::activity`** | Not a port: dilution factors, dry deposition, decay in transit, published HTR-10 tables. No upstream. Consistency checks only |
| **`buangkok::pydoseia`** | The steady Gaussian plume, plume rise, building wake, and dose pathways, ported from pyDOSEIA. The plume rise and wake formulas are unchecked against their cited sources (gh:#542) |
| **`outram-park-fork-pflotran`** | A pure-Rust PFLOTRAN translation. Richards flow, solute transport, heat transport, aqueous speciation, mineral kinetics, reactive transport and two-phase flow are implemented as **verification-only**: closed-form and manufactured-solution checks, no validation against PFLOTRAN reference cases, no human V&V |
| **`redhill`** | A **placeholder**. Nothing implemented: its only public item is a `SCOPE` string. It is meant to be mostly an integration of the PFLOTRAN fork |
| **Sea dispersion** | **Not built.** The only mention of the sea in the dispersion code is the land-sea mask used to pick a deposition class in `flexpart::landuse`. No ocean current or marine mixing model exists [citation needed: maintainer to supply] |
| **Atmospheric CFD or LES** | **Not built** (see Level 1) |

Sources: [`changi/README.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/README.md),
[`flexpart/mod.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/flexpart/mod.rs),
[`coverage.md`](../../deep-dives/dispersion/coverage.html),
[`pflotran README`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-park-fork-pflotran/README.md),
[`redhill README`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/redhill/README.md).

~~One inconsistency worth a reviewer's eye. The PFLOTRAN README's early
paragraph and its "v1 scope" section say solute transport, TH energy and
reactive geochemistry are out of v1 and the roadmap lists them as "not
started", while the table of what exists marks them as implemented.~~
**CORRECTED 2026-10-18:** the README's "v1 scope" and roadmap lines have been
struck through and corrected. A check of `src/lib.rs` and the module sources
confirmed the table of what exists (the modules and their `#[test]` functions
are present), so it is the one used here.

## 4. Why not just solve it

The brute-force route is Level 1 for air and Level 3 for ground, run for every
nuclide of interest. The cost comes from four things.

1. **Weather fields.** The wind, turbulence and rain are inputs, and they come
   from a forecast or reanalysis on a large grid. For the repo's FLEXPART, the
   input readers do not exist yet.
2. **3-D meshes.** Resolving eddies near a building and the ground needs fine
   cells, while the domain is tens of kilometres. The cell count multiplies.
3. **Long times.** A release may last days, and the deposited activity matters
   for years. Groundwater travel times are longer still, and a long run means
   many small time steps.
4. **Many nuclides.** Each nuclide has its own decay, deposition velocity and
   sorption. A decay chain couples them. Cost scales with the number of
   species.

A particle method lowers the first two costs, since particles need no
fine mesh for the transport. It still needs the weather and many particles per
species. So both families are used where the question needs a time-varying
wind, and neither is used for a quick answer. Hence the cheap models of
[Step 19](./18-puff-and-plume.md).

## What we assumed

- Turbulence acts like an eddy diffusivity at Level 0, and is resolved or
  parameterised at Levels 1 and 2.
- A species is tracked by its nuclide and a deposition class, not its chemical
  form.
- Only air and groundwater have code in the workspace. Sea and CFD do not.

## Next question

What is the cheapest model that still shows the cloud moving?
[Step 19](./18-puff-and-plume.md): the Gaussian puff, then the plume.
