# Step 8 — One atom at a time: the random walk

> **Research, education and V&V only.** Not for reactor operation, licensing, safety decisions or emergency response.
>
> **Review status: first draft, 2026-10-18, AI-assisted (Claude Sonnet), not yet reviewed by a human.**

**Back:** [Step 7, PANAMA fuel failure](./09-panama.md), and before it
[Step 4, silicon carbide](./04-sic.md).
**The question that led here:** steps 6 and 7 reduced the particle to a
pressure vessel that either holds or fails. An intact particle still lets
some fission products out by normal diffusion, and steps 2 to 4 said that
atoms diffuse, and that each layer has its own diffusion coefficient $D$.
*How do we actually compute where the atoms end up?*

**This page answers:** what is the model with the fewest assumptions,
and how do we make it cheaper without losing the physics we need?

---

## 1. The most intuitive model: follow every atom

Take the picture from step 3 literally. Each fission product is one atom.
It sits on a lattice site, then hops to a neighbour, billions of times a
second, in a random direction. Between hops it may decay. Nothing else is
assumed.

That is a **Lagrangian** model: we follow the things that move.

There is no diffusion equation in it. No concentration, no $D$ as an
input to a PDE. $D$ only appears as a summary of how fast an atom
wanders.

### The diffusion equation falls out

Add up $n$ small random jumps. The sum is close to a Gaussian, whatever
the shape of one jump. This is the **central limit theorem**. Each
Cartesian component of the displacement over a time $\Delta t$ then has
variance

$$\sigma^2 = 2 D \Delta t .$$

A crowd of such atoms spreads exactly as the diffusion equation
$\partial C/\partial t = D\nabla^2 C$ says. So the equation is not an
extra assumption. It is what the random walk becomes for a large crowd.

This is why the random walk is the "fewest assumptions" model, and why
the Eulerian models of [step 9](./06-eulerian-fresco.md) can be read as
its large-crowd limit.

### The cost, honestly

Following every real jump is hopeless: of order $10^{9}$ jumps per
second per atom ([walk rung](../../tutorials/triso-atops/walk.html), step 1).
So we **jump straight to the end of an interval**.

The crate's first engine did exactly that: one Gaussian step per time
step, in
[`scatter_within_triso_particle_gaussian_simple`](../../code-review/#crates/boon-lay/src/lagrangian_decay_simulator/lagrangian_diffusion/single_particle_simulator/movement_within_triso_particle/mod.rs::scatter_within_triso_particle_gaussian_simple)
(the older code is mapped in
[the legacy Gaussian engine](../../deep-dives/triso-atops/legacy-gaussian.html)).

---

## 2. Why the fixed-step walk breaks in a layered particle

The central limit theorem gives the answer for **free space**. A TRISO
particle is not free space. It is a stack of thin shells with very
different $D$.

Arithmetic you can redo, from
[`docs/buffer_clt_failure_analysis.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/docs/buffer_clt_failure_analysis.md)
(buffer $D \approx 10^{-8}$ m²/s):

| $\Delta t$ | $\sigma=\sqrt{2D\Delta t}$ | compare with |
|---|---|---|
| 1 s | 141 µm | buffer is 100 µm thick |
| 10 s | 447 µm | particle radius 427.5 µm |

One step can carry an atom over a whole layer, and past an interface
that should have reflected it. Shrinking $\Delta t$ cures it, but then
buffer atoms need hundreds of steps per second while kernel atoms
(much smaller $D$) barely move per step.

**The judgement this teaches:** a cheap step is only acceptable if it
cannot step across the geometry. When it can, the step must shrink, and
the saving is gone.

---

## 3. Cheaper walks: first passage and Walk-on-Spheres

Instead of fixing the time and finding the position, fix the
**geometry** and find the time.

1. Draw the largest sphere around the atom that touches no interface.
2. Inside it the medium is uniform, so the atom leaves at a point that is
   uniform over the sphere.
3. How long that takes has a known distribution. For a sphere of radius
   $R$, the mean first-passage time is

$$\langle t\rangle = \frac{R^{2}}{6D}.$$

4. Jump to the exit point, add the time, repeat.

Far from an interface the hops are large. Near one they shrink. A hop
touches the nearest interface and never crosses it, so the buffer
overshoot cannot happen. There is **no time step** at all.

Where to look: the walker, with its walk to a surface, is
[`WoSWalker`](../../code-review/#crates/boon-lay/src/lagrangian_decay_simulator/lagrangian_diffusion/first_passage/walk_on_spheres.rs::walk_to_absorbing_sphere),
the exit-time sampler is
[`sample_first_passage_time`](../../code-review/#crates/boon-lay/src/lagrangian_decay_simulator/lagrangian_diffusion/first_passage/sphere_fpt.rs::sample_first_passage_time).
The full walkthrough, with the sampling code, is in the
[walk rung](../../tutorials/triso-atops/walk.html), step 3.

**What judgement makes it acceptable.** Inside each hop sphere the
medium must really be uniform, with one $D$. That holds for concentric
shells. It also assumes the atom's path is Brownian, which is the same
assumption as step 1.

**When it fails.** Where $D$ varies smoothly inside a layer (for
example with a steep temperature gradient), the sphere is no longer
uniform. The crate looks $D$ up at the atom's position; whether that is
accurate enough for a given gradient has not been tested here.

---

## 4. Crossing an interface

An atom that arrives at an interface between side 1 (diffusion
coefficient $D_1$) and side 2 ($D_2$) must either cross or bounce.
The diffusion equation says concentration and flux stay continuous.
Detailed balance then fixes the crossing probability.

For Walk-on-Spheres:

$$p_{\text{transmit}} = \frac{K D_2}{D_1 + K D_2}$$

where $K$ is the partition ratio (1 for plain continuity). The
derivation is in the module docs of
[`interface.rs`](../../code-review/#crates/boon-lay/src/lagrangian_decay_simulator/lagrangian_diffusion/first_passage/interface.rs::transmission_probability).

**Why linear in $D$?** The rule depends on *how often this kind of walk
visits the interface*. In Walk-on-Spheres the rate of visits from side
$i$ scales as $D_i$. A **fixed-step** walk has a step length that scales
as $\sqrt{D}$ and a visit rate that scales as $1/\sqrt{D}$, so it needs
a different rule, in terms of the ratio $\sqrt{D_2}/\sqrt{D_1}$. Using
the wrong rule gives the wrong equilibrium. (The module docs state this
for the fixed-step case; the crate implements only the
Walk-on-Spheres rule.)

This is the SiC barrier of step 4 as arithmetic: an atom arriving from
pyrolytic carbon transmits into SiC with probability of order
$D_{\text{SiC}}/D_{\text{PyC}}$. With the Arrhenius factors, that ratio
is 1/13 to 1/4100 over Cs, Sr, Ag and Kr at 1000 to 1600 °C (Cs: 1/120
at 1000 °C, 1/440 at 1600 °C), measured with
`examples/layer_diffusion_table.rs`
([walk rung](../../tutorials/triso-atops/walk.html) and
[interface record](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/verification_and_validation/interface_uniform_equilibrium_density.md)).

---

## 5. Does the crowd give the right answer?

Two checks, both **verification** against exact results, not validation
against experiment.

**(a) One uniform sphere: CRP-6 Case 1.** Cs-137 from a bare 212.5 µm
UO$_2$ kernel at 200 h, $N = 40 000$ walkers, pass if
$|MC-\text{Crank}|<0.02$. Crank's series is the reference. Re-measured
2026-10-04 (**the numbers to quote**):

| Case | MC | Crank | abs. error |
|---|---|---|---|
| 1a, 1200 °C | 0.5331 | 0.5337 | 0.0006 |
| 1b, 1600 °C | 1.0000 | 1.0000 | 0.0000 |

The 1-sigma binomial error at $p=0.5$ is
$\sqrt{p(1-p)/N}=0.0025$. Record:
[`crp6_case1_kernel_release_vs_crank.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/verification_and_validation/crp6_case1_kernel_release_vs_crank.md).
That record also notes an open disagreement: the code's own citation
quotes 0.453 to 0.498 for Case 1a, and 0.5337 lies outside it
(gh:#537). It does not affect this check, since both sides use the same
$D$, but it questions whether the Case 1a inputs are the benchmark's.

**(b) The interface rule.** A single walker in a two-region sphere with a
10 times $D$ contrast must spend a fraction of time in the inner region equal
to its volume fraction, 0.125. Re-measured 2026-10-04: **0.1368**, error
0.0118 against a 0.02 threshold. The earlier record read 0.1216; same
seed, so the random stream changed. The record itself says the spread
over seeds has not been measured, so the 0.02 threshold cannot yet be
called tight.

**What is not checked:** there is no quantitative full five-layer
release record. The multilayer walk is exercised only qualitatively.

---

## 6. What the random walk costs, and what it buys

**Cost: statistical noise.** Each history is independent. A release
fraction near $p$ from $N$ histories has error

$$\epsilon \approx \sqrt{\frac{p(1-p)}{N}} \propto \frac{1}{\sqrt{N}}.$$

Ten times smaller error costs a hundred times more histories. For
illustration, at $p=0.5$: $N=400$ gives $0.025$; $N=40 000$ gives
$0.0025$. Small releases are worse in relative terms, since the error
is then comparable to $p$ itself.

**Gain.**

- **Watch the atoms.** You can see which atom is where, and when it
  leaves. The live ensemble does this
  ([`LiveEnsemble`](../../code-review/#crates/boon-lay/src/lagrangian_decay_simulator/lagrangian_diffusion/first_passage/live.rs)).
- **Any geometry** you can draw. Only the distance to the nearest
  interface is needed.
- **Decay clocks are natural.** Each atom carries its own decay clock;
  when it fires inside a hop, the atom changes nuclide and, with it, its
  $D$ ([`depletion.rs`](../../code-review/#crates/boon-lay/src/lagrangian_decay_simulator/lagrangian_diffusion/first_passage/depletion.rs)).
  No Bateman matrix. The neutron-field side of that module is a
  framework only: one explicit capture channel, with no per-nuclide
  cross sections wired in. **Transmutation is not yet modelled while
  products move through the layers**; see [step 1](./01-generation.md)
  and the [transmutation deep dive](../../deep-dives/triso-atops/transmutation.html).
- **Embarrassingly parallel.** Histories are independent
  ([`ensemble.rs`](../../code-review/#crates/boon-lay/src/lagrangian_decay_simulator/lagrangian_diffusion/first_passage/ensemble.rs)).

**The trade.** The Eulerian view gives a smooth answer with no noise but
needs a mesh and, to be cheap, strong assumptions. The random walk
gives a noisy answer with almost no assumptions. Step 9 takes the other
road.

---

## What we assumed

- Each atom's motion is a random walk whose long-time behaviour is
  Fickian, with $D$ from correlations (steps 2 to 4).
- Inside each hop sphere the medium is uniform.
- Interfaces: continuity of concentration and flux, partition ratio $K$
  (default 1).
- No interaction between atoms (no clustering, no bubbles) in the walk.
- Verified only on a single uniform sphere and a two-region interface;
  not yet on the full multilayer release.

**Next question:** the random walk is noisy and costs $1/\sqrt N$. If we
only want the *average* behaviour, can we solve for the concentration
directly? [Step 9, the aggregate view](./06-eulerian-fresco.md).
Then [step 10](./07-ray-equations.md) says how this crate sits between the
two.
