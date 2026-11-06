# Step 6 — Simplify the particle to a pressure vessel: gas, pressure and stress

> **Research, education and V&V only.** Not for reactor operation, licensing, safety decisions or emergency response.
> **Review status: first draft, 2026-10-18, AI-assisted (Claude Sonnet), not yet reviewed by a human.**

**The question this page answers.** Steps 1 to 4 followed atoms from birth
through the layers. Some stay in. The noble gases (Kr, Xe) do not bond to
anything, and oxygen freed by fission leaves as CO. *How much pressure do
they build, and what does that pressure do to the layers?*

**The question from the previous step.** [Step 5](./05a-high-fidelity-fem.md)
described the high-fidelity way: a BISON-style finite-element fuel
performance model that resolves creep, swelling, cracking and asphericity,
and which this workspace does not have. It ended by asking for the simplest
model that still captures failure. The first simplification is to treat the
particle as a **pressure vessel**: gas inside, a shell that carries the load,
and a stress to compare against a strength. How much gas gets out of the
kernel to load that vessel is a diffusion question, which
[Steps 8 to 10](./05-random-walk.md) take up after the failure model.

This page starts from the model with the **fewest assumptions**, which is
also the most expensive: a finite-element model of the whole particle. It
then states what that model needs and what the workspace has today. The next
page, [Step 7](./09-panama.md), makes the cheap simplification.

## 1. What accumulates, and where

Three things are involved.

- **Gas atoms made.** Each fission yields some stable Kr and Xe. The code
  carries the report's yield of 0.31 atoms per fission
  ([`STABLE_FISSION_GAS_YIELD`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/pressure.rs)).
- **Gas atoms that leave the kernel.** Only the part that gets out of the
  kernel reaches the buffer. That is the release problem of
  [Steps 8 to 10](./05-random-walk.md), seen from the inside. In the failure
  code it is a number $F_d$ from a Booth-type closed form
  ([`booth.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/booth.rs)).
- **CO.** Fission frees oxygen from the oxide kernel. Some of it meets carbon
  and becomes CO. The code counts it as "oxygen per fission", $\mathrm{OPF}$,
  from the report's correlations
  ([`oxygen.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/oxygen.rs)).

The gas sits in the **buffer**, the porous carbon layer next to the kernel,
which is mostly empty space. The failure code takes the free volume $V_f$ as
50 % of the buffer volume, a value the report prescribes
([`history.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/history.rs),
field `free_volume`).

## 2. From moles to pressure

**Fewest assumptions: treat the gas as a gas.** The pressure is set by how
many moles $n$ are in the free volume at temperature $T$. The ideal-gas law
is the first model.

$$p = \frac{n R T}{V_f}$$

The code counts $n$ per mole of heavy metal. A mole of kernel compound has
volume $V_m$ (a molar volume, a fixed property of the compound). So the
kernel volume $V_k$ holds $V_k / V_m$ moles of heavy metal, and burnup $F_b$
(fissions per heavy-metal atom, FIMA) turns that into fissions. Putting it
together:

$$p = \frac{(F_d F_f + \mathrm{OPF})  F_b  R  T}{(V_f / V_k)  V_m}$$

This is the report's Eq (3) as the code uses it
([`internal_gas_pressure`](../../code-review/#crates/boon-lay/src/fuel_failure/pressure.rs::internal_gas_pressure)).
The printed fraction bar is ambiguous. Read one way, pressure would *fall* as
the particle heats. The code uses the only dimensionally consistent reading,
$p = nRT/V_f$, and a test checks it against $nRT/V$ computed separately.

The molar volume $V_m$ comes from Eqs (6a) to (6c): molar mass divided by
density, one value per kernel compound
([`molar_volume.rs`](../../code-review/#crates/boon-lay/src/fuel_failure/molar_volume.rs::molar_volume)):

| Kernel | $V_m$ (m$^3$/mol) |
|---|---|
| (Th,U)O$_2$ | $2.51905 \times 10^{-5}$ |
| UO$_2$ | $2.43796 \times 10^{-5}$ |
| UCO | $2.50654 \times 10^{-5}$ |

These are the report's printed quotients. A UCO particle given the UO$_2$
value would have a gas pressure wrong by 2.8 %, with no error raised, so the
code keeps three variants.

**What the ideal-gas law assumes, and where it fails.** It ignores the
volume of the molecules and the attraction between them. At the high gas
densities a hot, high-burnup buffer can reach, a non-ideal equation of state
would give a different pressure. The code has no such equation: it uses the
ideal-gas law throughout, with $R = 8.3143$ J/(mol K) as printed in the
report. How large the error is for a real buffer
[citation needed: maintainer to supply]. Treat the ideal-gas pressure as a
model choice, not a measured fact.

**Temperature acts twice.** $T$ appears directly in $p \propto T$. It also
changes $F_d$, because hotter kernels release more gas. This is why the
HTR-10 table in [Step 7](./09-panama.md) shows hoop stress rising by about
12 times from 776 C to 1600 C, far more than the ratio of absolute
temperatures (1873 K / 1049 K is about 1.8).

## 3. From pressure to stress: the shell as a pressure vessel

The layers form nested spherical shells: buffer, inner PyC, SiC, outer PyC.
The pressure acts on the inside of the stack, and the stiff SiC carries most
of the load. For a thin spherical shell of mean radius $r$ and thickness $d$,
the hoop stress is

$$\sigma_t = \frac{r  p}{2  d}$$

as a force balance on a hemisphere gives. For a SiC layer at the code's
illustration geometry (inner radius 250 micrometres, outer 285, so
$d = 35$ micrometres, and a cube-root mean radius of about 268.6 micrometres):

$$\frac{r}{2d} = \frac{268.6}{2 \times 35} \approx 3.84,
\qquad p = 10\ \text{MPa}  \Rightarrow  \sigma_t \approx 38\ \text{MPa}.$$

This is an order-of-magnitude illustration, with the arithmetic shown. It is
not a case from the report. The thin-shell form is what the failure code
implements, as Eq (2) of the report
([`induced_stress`](../../code-review/#crates/boon-lay/src/fuel_failure/stress.rs::induced_stress)).

## 4. What the layers do besides carry the load

A real particle is not a static vessel with a fixed pressure on the inside.

- **The PyC layers shrink under neutron irradiation, then swell, and creep.**
  Irradiation-induced dimensional change and creep of PyC change how much of
  the pressure load the PyC takes and how much reaches the SiC. The laws and
  their parameters [citation needed: maintainer to supply].
- **The kernel swells.** Solid fission products and the gas bubbles inside
  the kernel push on the buffer. The buffer is porous so that it can take
  this, and also gives the gas somewhere to go.
- **Layers can debond or crack.** A gap between layers changes the load path,
  and a cracked PyC layer sends its load to the SiC.
- **The SiC is anisotropic and brittle.** Its strength is a distribution, not
  a number, which is the subject of Step 7.

## 5. The first-principles model: a finite-element particle

**The model with the fewest assumptions.** Mesh the kernel and the four
layers. Give each layer its own elasticity, its own irradiation creep and
swelling law, and let the gas pressure act on the inside surface. Advance in
time with the changing temperature, burnup and fluence. Solve the weak form of
equilibrium for the displacements. That is what a fuel-performance code does.

The repo describes one such code, BISON, as a source of TRISO fission-product
diffusion models: the diffusion coefficients used in `boon-lay` are those
tabulated in Jiang et al. (2023), and the CRP-6 release benchmark in the
crate is taken from the BISON paper *Modeling fission product diffusion in
TRISO fuel particles with BISON*, Journal of Nuclear Materials 548, 152840
(cited as the repo's code comments give it:
[`release_fraction_crp_6_case_1a_1b.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/lagrangian_decay_simulator/lagrangian_diffusion/single_particle_simulator/release_fraction_crp_6_case_1a_1b.rs)).
The repo's bibliography also lists a BISON TRISO modelling and validation
report (Hales et al., 2020: *BISON TRISO modeling advancements and validation
to AGR-1 data*, Idaho National Laboratory;
[`references.bib`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/docs/site/references.bib),
key `hales2020bison`). What BISON's mechanical model contains is not
described in this repo, so this page does not say [citation needed:
maintainer to supply].

**The cost.** A mesh, a nonlinear solve at every time step, a history of
irradiation for every layer. It is the right way to ask "does this particle
fail?" and the wrong way to ask it for every particle in a core. The repo records
27 000 fuel elements for HTR-10 (`nee_soon`, from the paper's body text) and
about 8 300 particles per element (8340 counted, 8240 built, gh:#316), so
$27000 \times 8300 \approx 2.2 \times 10^{8}$ particles. That is an order of
magnitude from the recorded counts.

### What `farrer-park` has today

`farrer-park` is this workspace's finite-element structural mechanics crate.
The maintainer intends it as the place for a BISON-style translation. Reading
its [README](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/farrer-park/README.md):

| Have, implemented | Not started or absent |
|---|---|
| Lagrange elements: Tri3, Tri6, Quad4, Tet4, Hex8 | **Any TRISO particle model** |
| Linear elasticity, weak-form assembly | Irradiation creep and swelling laws |
| J2 plasticity, radial return | Contact between layers |
| Rate-dependent crystal plasticity | Thermo-mechanical coupling to a changing temperature field |
| Dirichlet and Neumann (traction) conditions | Axisymmetric or spherical-shell elements |
| Newton solution with load stepping | Burnup and gas-pressure coupling |
| Thick-walled cylinder against the Lamé solution | |

There is no TRISO model, no PyC or SiC material, and no creep or swelling law
in `farrer-park` (the README lists none, and a search of the crate for TRISO
or PyC finds only a mention in its `CLAUDE.md` about geometry visualisation,
not a mechanics model). It is verified against analytical and manufactured
solutions only, and the README states it is not a validated life-assessment
tool.

The thick-walled cylinder check is the nearest thing to a pressurised shell.
The README reports a 2.45 % maximum stress error (first order) and 0.021 % in
radial displacement against the closed-form Lamé solution (measured
2026-09-11). It is a cylinder, in plane strain, not a sphere.

### What a TRISO model in `farrer-park` would need

This is a list of requirements read from the table above. It is not a plan.

1. **A spherical-shell geometry.** The particle is spherically symmetric to
   first order. A 1D spherical element, or an axisymmetric 2D mesh, would cut
   the cost far below a 3D mesh. The crate has neither today; its elements are
   2D plane and 3D solid.
2. **Layered materials**, each with elastic constants as functions of
   temperature and irradiation [citation needed: maintainer to supply].
3. **Irradiation creep and dimensional change** for PyC, and a swelling law
   for the kernel and buffer, as time-integrated, history-dependent material
   updates. The crate's J2 and crystal-plasticity updates are the pattern, but
   the laws are new.
4. **Contact or a debonding model** between layers, which the crate lacks.
5. **Internal pressure from Eq (3)** applied as a load that changes with time,
   closing the loop with $F_d$ from the diffusion calculation of Steps 8 to 10.
6. **A failure criterion** on the SiC stress, which is where Step 7's Weibull
   law would plug in.
7. **Verification** against closed-form shell solutions first, then against
   published particle results.

Until then, the only particle-scale stress in this workspace is the thin-shell
formula of [Step 7](./09-panama.md).

## Why each simplification is allowed

**Kinds of justification used below.** (a) negligible effect, with an estimate; (b) computationally too expensive to be practical; (c) bounding or conservative, with the side it errs on; (d) data not available. **Open** means this page cannot justify it.

| Simplification | Kind | Justification |
|---|---|---|
| Ideal-gas law | (d), and **open** | The repo has no real-gas equation of state or data. The size of the error is not estimated here, so it is not called negligible |
| Free volume is 50 % of the buffer | (d) | The value is the report's, taken as printed |
| Single temperature over the free volume | **Open** | No estimate of temperature variation across the buffer in the repo |
| FEM replaced by a thin shell (Step 7) | (b), partly | Per-particle FEM times the number of particles in a core is too costly to run by default (about $2 \times 10^{8}$ particles for HTR-10, from the repo's counts above). Its accuracy against FEM is **not measured**, as `farrer-park` has no TRISO model |
| PyC creep and swelling omitted from the failure chain | (d) | The laws and parameters are not in the repo |

## What we assumed

- The gas is ideal, and a single temperature applies across the free volume.
- The free volume is a fixed fraction of the buffer.
- Only $F_d$ of the stable gas reaches the buffer, and $\mathrm{OPF}$ gives the CO.
- The shell is thin, and the SiC carries the hoop load.
- A finite-element model of the particle is the reference method, and
  `farrer-park` does not yet provide it.

## Next question

A finite-element solve for every particle in a core is too much. What can be
dropped, and what does dropping it cost? [Step 7](./09-panama.md) tells how
PANAMA made the choice, and how we made it in `boon-lay fuel failure`.
