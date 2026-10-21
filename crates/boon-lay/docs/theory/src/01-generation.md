# Step 1 — Where fission products come from: burnup and transmutation

> **Research, education and V&V only.** Not for reactor operation, licensing, safety decisions or emergency response.

> **Review status: first draft, 2026-10-18, AI-assisted (Claude Sonnet), not yet reviewed by a human.**

*Previous: [how this book works](./intro.md). We have a particle with five
layers. Before anything moves, which atoms are in it?*

## The question

Inside the kernel, uranium fissions. Each fission makes two fragments and a
few neutrons. The fragments are mostly radioactive, and they decay into other
nuclides, which may decay again. Neutrons can also change a nuclide without
fission. **How many atoms of each nuclide are in the kernel at time $t$?**

## The model with the fewest assumptions: track everything

Write $N_i(t)$ for the number of atoms of nuclide $i$. Every nuclide

- is **created** by fission of a heavier nuclide (a *fission yield* says how
  many atoms of $i$ per fission), by the decay of a parent, or by a neutron
  reaction on a neighbour;
- is **destroyed** by its own decay, by fission, or by a neutron reaction.

Neutron reactions that matter are capture, $(n,\gamma)$, which adds one
neutron (Xe-135 becomes Xe-136, the example the crate uses), and $(n,2n)$,
which removes one.

Collect all nuclides in a vector $\mathbf{N}$. Then

$$\frac{d\mathbf{N}}{dt} = A \mathbf{N},$$

where the **burnup matrix** $A$ holds decay constants, $\phi \sigma$ for each
reaction (with $\phi$ the neutron flux and $\sigma$ the cross section), and the
fission yields. If $A$ were constant the answer is the matrix exponential,
$\mathbf{N}(t)=e^{At}\mathbf{N}(0)$. It is not constant: flux and cross
sections change as the fuel burns, so a code steps in time, rebuilding $A$ each
step.

The assumptions are few: rate laws are linear in the populations, and the
nuclide list is complete enough. The cost is that $A$ is large and **stiff**:
half-lives span many orders of magnitude, from fractions of a second to
millions of years, so a naive time step is either unstable or tiny.

### What this workspace has

- The **numerics** of that solve live in `petir::depletion`: OpenMC's burnup
  time integrators, its IPF CRAM matrix-exponential solver and the dense burnup
  matrix, ported with provenance
  ([source header](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/petir/src/depletion/mod.rs)).
  CRAM (the Chebyshev rational approximation method) is the stiff-safe way to
  evaluate $e^{At}\mathbf{N}$. The crate states the *physics* (the chain, decay
  constants, branching, fission yields, reaction rates) belongs to the caller,
  here `outram-mc-libs`.
- The **data**: ENDF/B-VIII.0 decay data is read by boon-lay's
  `DecayLibrary`
  ([decay data](../../deep-dives/triso-atops/decay-data.html)), and fission
  yields are in the separate `fission-yields-data` crate, which boon-lay
  re-exports as the `Nuclide` enum.

**What boon-lay's Lagrangian simulator does not have yet.** Its own
[transmutation page](../../deep-dives/triso-atops/transmutation.html) says that
`(n,2n)`, fission with yield sampling, per-nuclide cross sections and flux maps
are not implemented or not wired. The decay clock, one explicit $(n,\gamma)$
channel and the Th-232 chain test exist. So on this page, "as the physics
demands" is ahead of "as boon-lay does it". Both are stated below.

## Simplification 1: follow one atom, not a matrix (the Lagrangian choice)

The matrix is large (the crate's design notes quote the order of thousands of
nuclides for typical reactor burnup, with sparse rows; this is an
order-of-magnitude statement from the notes, not a measurement here).
Instead, give each atom its own clocks. For the nuclide it currently is,

$$\lambda_{\text{total}} = \lambda_{\text{decay}} + \lambda_{\text{transmute}},\qquad
\lambda_{\text{decay}}=\frac{\ln 2}{T_{1/2}},\qquad
\lambda_{\text{transmute}}=\phi \sigma .$$

Draw the waiting time to the next event as $t=-\ln(1-u)/\lambda_{\text{total}}$
with $u$ uniform in $[0,1)$, pick the channel in proportion to the rates, and
move the atom to its product. The histogram of identities over many atoms
**is** the inventory.

- *What is thrown away:* nothing physical. It is the same rate equations,
  solved by sampling instead of by matrix algebra.
- *The judgement:* is the statistical noise acceptable? A rare nuclide needs
  many atoms to be seen at all.
- *The reward:* no stiffness, no matrix exponential, cost linear in the number
  of atoms, and the same walker can also diffuse (step 5).

In the code: `WoSWalker::advance_until`, whose hop is cut short if an event
fires first. See the
[transmutation deep dive](../../deep-dives/triso-atops/transmutation.html) and
the Code Review map entry
[`advance_until`](../../code-review/#crates/boon-lay/src/lagrangian_decay_simulator/lagrangian_diffusion/first_passage/depletion.rs::advance_until).

**Check status, honestly.** Four unit tests cover the sampler and the channel
logic. The deep dive records that there is **no V&V record** comparing a
depleting ensemble with a Bateman solution (gh:#539). Do not quote a number
from it yet.

## Simplification 2: only some nuclides

Most of the thousands of nuclides in a chain never matter for release. The
release models in the crate group elements by how they move
(noble gas, halogen, special metal, silver, other; see
[`ElementGroup`](../../code-review/#crates/boon-lay/src/triso_atops_fork/nuclide_model/mod.rs::from_atomic_number)),
and the MHTGR example tracks chosen nuclides.

- *The judgement:* a nuclide may be dropped if neither it nor its descendants
  matter for the quantity you want. Cs-137 and its short-lived parents matter
  for a Cs release; a nuclide that decays in seconds into something stable
  does not.
- *When it fails:* a short-lived parent that is a **mobile** gas (a precursor)
  can leave the particle before it turns into a less mobile daughter. Dropping
  it shifts where the daughter is born. `[citation needed: maintainer to
  supply a source for precursor effects in TRISO release.]`

## Simplification 3: decay only, fixed flux

Hold the flux constant (or set it to zero after shutdown). Then $A$ is
constant and, for a **pure decay chain**, the Bateman equations solve it in
closed form. The crate uses exactly this as its check
([decay rung](../../tutorials/triso-atops/decay.html)): the Th-232 chain
Ra-228, Ac-228, Th-228, Ra-224, Rn-220, Po-216, Pb-212, Bi-212 to stable
Pb-208 is walked and each identity asserted. That test checks the chain's
**order**, not the branching frequencies.

- *The judgement:* is the flux effectively constant, or effectively zero, over
  the time of interest? After shutdown, yes. During irradiation, the flux
  changes with burnup and with position in the core.
- *When it fails:* a high flux, where a nuclide is more likely to be hit than
  to decay. The crate's design notes say: if flux is low against decay times,
  transmutation matters less; if flux is high, it can reshape the whole chain.

## What we assumed

- Rate laws linear in the populations (every reaction is first order).
- The flux and cross sections are given. This page does not compute them; that
  is a neutronics problem (`outram-mc-libs`).
- Fission yields are the evaluated ENDF/B-VIII.0 values, not computed here.
- In boon-lay today: decay is real, transmutation is one explicit $(n,\gamma)$
  channel, fission yields are **not** wired into the walker.

## Next question

[Step 2](./02-recoil.md): a fission makes fragments with great energy. Where
do they stop?
