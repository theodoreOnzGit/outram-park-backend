# Step 3 — Through the layers: diffusion, and why some layers trap

> **Research, education and V&V only.** Not for reactor operation, licensing, safety decisions or emergency response.

> **Review status: first draft, 2026-10-18, AI-assisted (Claude Sonnet), not yet reviewed by a human.**

*Previous: [Step 2](./02-recoil.md) put the atom at rest, mostly inside the
kernel. Next, [Step 4](./04-sic.md) looks at the one layer that matters most.*

## The question

An atom sits in the kernel. To leave the particle it must cross the kernel,
the buffer, the IPyC, the SiC and the OPyC. **How fast does it move in each,
and why do the layers differ so much?**

## The model with the fewest assumptions: diffusion with the real chemistry

At the smallest scale an atom hops between sites. The rate of hopping depends
on what is around it: which compound it forms, which defects or grain
boundaries are nearby, whether it is a gas in a bubble, whether it sticks to a
surface. A first-principles model would carry all of that: the chemical
species of each fission product, their reactions with the kernel and the
carbon, and the microstructure of each layer.

We do not have that model in this workspace. What the repo has is its
coarse-grained result: **one effective diffusion coefficient $D$ per element
per layer**, measured or fitted by others, with the chemistry folded into the
numbers. The rest of this page says what that folding hides, and marks every
claim the repo cannot support.

## The one law everything uses: Arrhenius $D(T)$

Atoms hop by thermal activation, so

$$D(T) = D_1 e^{-Q_1/RT} + D_2 e^{-Q_2/RT},$$

with $R$ the gas constant, $T$ in kelvin, and $D_1, Q_1, D_2, Q_2$ per element
and per layer. The second term matters only at high temperature. This is the
two-term form of Jiang et al. (2023), *Fission product transport in TRISO
particles and pebbles*, INL, which the walker uses
([rung 4](../../tutorials/triso-atops/layers.html), table on p. 13 of the
report). The activation energy $Q$ is the "energy cost of one hop", so a large
$Q$ means a strong temperature dependence. The fast-fluence factor on Cs in
SiC is in the code but not discussed here.

*Why exponential?* A hop needs the atom to borrow an energy $Q$ from the
lattice vibrations. The fraction of atoms with that energy is $e^{-Q/RT}$
(Boltzmann). `[citation needed: maintainer to supply a textbook reference for
Arrhenius diffusion in solids.]`

## What the numbers say (the walker's own lookup, zero fluence)

Run 2026-10-04, $D$ in m²/s, from `examples/layer_diffusion_table.rs`
([record](../../tutorials/triso-atops/layers.html)):

| Nuclide | T (°C) | kernel | buffer | PyC | SiC |
|---|---|---|---|---|---|
| Cs-137 | 1000 | 1.50e-16 | 1.0e-8 | 4.91e-17 | 4.09e-19 |
| Cs-137 | 1600 | 1.25e-13 | 1.0e-8 | 4.06e-14 | 9.23e-17 |
| Sr-90 | 1000 | 2.10e-23 | 1.0e-8 | 1.90e-14 | 4.66e-18 |
| Sr-90 | 1600 | 5.42e-17 | 1.0e-8 | 7.38e-12 | 2.46e-15 |
| Ag-110m | 1000 | 1.14e-15 | 1.0e-8 | 2.55e-15 | 5.44e-18 |
| Ag-110m | 1600 | 1.68e-13 | 1.0e-8 | 2.69e-13 | 3.64e-15 |
| Kr-85 | 1000 | 5.36e-17 | (none) | 3.34e-20 | 3.63e-23 |
| Kr-85 | 1600 | 2.50e-14 | (none) | 2.23e-16 | 1.77e-17 |

Four things stand out. These are statements about the **coefficients**, not
independent measurements.

1. **The buffer is 5 to 15 orders of magnitude faster than anything else**
   ($10^{-8}$ against $10^{-23}$ to $10^{-13}$). It is a porous space, so an
   atom moves through voids, not through solid.
2. **Strontium is almost immobile in the kernel** ($2\times10^{-23}$ at
   1000 °C) but moves freely in PyC ($1.9\times10^{-14}$): a factor of about
   $10^9$ between the kernel and the PyC. For Cs the kernel and PyC are
   comparable.
3. **The SiC is slowest of the three solids for Cs, Sr and Ag.** Step 4 quantifies it.
4. **Krypton in the buffer has no coefficient**: the lookup is a `todo!()`
   (gh:#541), so a Kr-85 walker that reaches the buffer panics. Elements other
   than Ag, Cs, Sr and Kr silently diffuse as silver, which is a placeholder
   and not a model.

## Why do the layers differ? The chemistry

Here the repo supports little, so this section says what is known and what is
not. Everything not supported is marked.

**The oxide kernel.** The kernel is UO₂. Fission products sit in it as
dissolved atoms, in precipitates, or in gas bubbles. The *chemical form* of
each is what fixes its mobility. For example, that strontium diffuses
$10^9$ times slower in the kernel than in PyC (item 2) is a coefficient the
crate carries; **why**, in terms of which compound Sr forms in UO₂, is not in
the repo. ~~`[citation needed]`~~ **Partly sourced 2026-10-21:**
IAEA-TECDOC-1645 (*High Temperature Gas Cooled Reactor Fuels and Materials*,
IAEA, Vienna, 2010, section 10.3.1, "Behavior of chemically active fission
products in fuel") says Sr, with Zr and Nb, has significant solubility in UO₂
and is easy to oxidise (SrO), whereas Cs "has low solubility and might be
oxidized to Cs2O depending on the fuel temperature and oxygen potential", and
that significant Cs release from the kernel was observed in the work it
cites. That is consistent with Sr being held in the oxide kernel better than
Cs, but it gives no diffusion coefficient, so the factor of $10^9$ is still
the crate's own. Proprietary-folder source.

**Noble gases (Kr, Xe).** They are chemically inert, so they do not bond to
anything. In the crate they form their own release group (He, Ne, Ar, Kr, Xe,
Rn) with an empirical release-to-birth correlation, and they "do not plate
out" ([`ElementGroup`](../../code-review/#crates/boon-lay/src/triso_atops_fork/nuclide_model/mod.rs::from_atomic_number)).
The idea that they move only through defects and bubbles is the textbook
picture, but the repo does not state it. ~~`[citation needed]`~~ **Partly
sourced 2026-10-21:** IAEA-TECDOC-1645 (IAEA, Vienna, 2010, section 10.3.1)
describes fission gas in a UO₂ kernel as atoms in the lattice and gas in
intragranular and intergranular bubbles, with release to the buffer by
percolation once the intergranular porosity interconnects (release of about
20 % at 4 % FIMA to about 35 % at 10 % FIMA at 1450 K in the work it cites).
That supports "defects and bubbles" for the kernel only; transport of the
gases through the PyC and SiC layers is not covered by the passage read.
Proprietary-folder source.

**Silver (and palladium).** The crate groups Ag and Pd and releases them by a
*breakthrough* model through the SiC, "the limiting barrier for Ag"
([`ElementGroup::Silver`](../../code-review/#crates/boon-lay/src/triso_atops_fork/nuclide_model/mod.rs::from_atomic_number)).
That Ag moves through SiC grain boundaries, and that Pd attacks the SiC, are
not stated in the repo as mechanisms. What the repo does state about SiC
attack is in step 4 (`grain_boundary.rs`). ~~`[citation needed]`~~ **Sourced
2026-10-21 (proprietary-folder sources), and the Ag half corrects the picture
"Ag moves along grain boundaries":**

- *Pd attack.* NUREG/CR-6844 (*TRISO-Coated Particle Fuel PIRTs*, U.S. NRC,
  2004, pp. 2-19 and 2-47) says palladium migrates to the SiC and attacks it
  at high temperature (above about 1300 °C), and is the noble metal of greatest
  concern. IAEA-TECDOC-1645 (2010, sections 8.5 and 10.3.4) discusses Pd attack
  of SiC and says the Pd–SiC reaction can be explained qualitatively from the
  Si–C–Pd phase diagram.
- *Ag in SiC.* IAEA-TECDOC-1645 (section 10.3.4) says silver is released from
  undamaged particles above about 1100 °C, that "today, the Ag migration
  mechanism remains not fully understood", and that McLean et al. showed
  grain-boundary diffusion would give a coefficient inconsistent with the one
  derived from Ag release measurements. So grain-boundary diffusion is one
  proposed and contested explanation, not an established mechanism.

**Oxygen and carbon monoxide.** This one is in the repo. When a UO₂ kernel
fissions, oxygen is freed, and the oxygen forms CO with the carbon, which adds
to the gas pressure alongside the fission gases (PANAMA-I Eqs (5a) to (5f),
oxygen released per fission, capped at $0.625$). It is a *chemistry* effect
on the pressure of step 6, not on diffusion
([`oxygen.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/oxygen.rs)).

**Air and steam.** Water or air entering hot graphite is a separate kind of
chemistry: graphite oxidation and kernel hydrolysis. The crate has three
cited rate laws and says they are not mechanistic attack models
([rung 6](../../tutorials/triso-atops/chemistry.html)).

## Interfaces: where one layer meets the next

Two neighbours with different $D$ and different chemistry can also differ in
how much the atom prefers one side. The walker handles an interface by
demanding that, left alone, atoms settle to a concentration ratio
$c_2/c_1 = K$ (the **partition ratio**). That gives the transmission
probability

$$p_{\text{transmit}} = \frac{K D_2}{D_1 + K D_2}.$$

The crate's default is $K=1$, meaning concentration is continuous and there is
no chemical segregation; a $K \neq 1$ would be the place to put chemistry
(a compound that dissolves better in one layer). The code allows it, but the
repo supplies no $K$ values
([`transmission_probability`](../../code-review/#crates/boon-lay/src/lagrangian_decay_simulator/lagrangian_diffusion/first_passage/interface.rs::transmission_probability);
the derivation is in the module doc of
[`interface.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/lagrangian_decay_simulator/lagrangian_diffusion/first_passage/interface.rs)).
Why linear in $D$ and not $\sqrt{D}$ is explained on [rung 4](../../tutorials/triso-atops/layers.html#step-2-what-happens-at-an-interface).

## The simplification ladder for this step

1. **Effective $D(T)$ per element per layer** (what the walker does). The
   chemistry is inside the coefficient.
   *Judgement:* the coefficient was measured in conditions like yours (same
   material, temperature range, burnup, fluence). *Fails:* outside that range;
   Jiang's fits were read from plotted correlations and the crate's tests
   check only the transcription.
2. **One equivalent sphere per chemical group** (Booth; step 9). The layers
   are thrown away. *Judgement:* one layer controls the release.
3. **An empirical release-to-birth ratio** for gases and halogens. *Judgement:*
   a correlation fitted to data applies to your case.

## What we assumed

- One $D(T)$ per element per layer, from the cited correlations.
- $K=1$ at every interface.
- No temperature gradient across the particle; one fluence for the whole
  particle.
- Missing chemistry (the forms of Cs and Sr, Ag in SiC, Pd attack) is marked
  above as needing a citation.

## Next question

[Step 4](./04-sic.md): the table shows SiC is slow for every element. How
slow, compared with the carbon around it, and what can make it fail?
