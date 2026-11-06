# Step 9 — Normal diffusion, FRESCO-style: the aggregate view, Booth and FRESCO

> **Research, education and V&V only.** Not for reactor operation, licensing, safety decisions or emergency response.
>
> **Review status: first draft, 2026-10-18, AI-assisted (Claude Sonnet), not yet reviewed by a human.**

**Back:** [Step 8, one atom at a time](./05-random-walk.md).
**The question that led here:** the random walk is honest but noisy,
with error $\propto 1/\sqrt N$. If we only want the *average* release,
can we skip the individual atoms?

**This page answers:** what is the aggregate model with the fewest
assumptions, what do we give up at each simplification, and where do
FRESCO and TRISO-ATOPS sit on that ladder?

---

## 1. Fewest assumptions: concentration in spherical layers

Stop following atoms. Follow the **concentration** $C(r,t)$ (atoms per
unit volume) on a fixed grid. This is the **Eulerian** view.

Fick's first law gives the flux, $J=-D \partial C/\partial r$.
Conservation of atoms in a spherical shell then gives, with a source
$Q$ (births, per volume per time) and decay constant $\lambda$:

$$\frac{\partial C}{\partial t}
=\frac{1}{r^{2}}\frac{\partial}{\partial r}\left(r^{2}D\frac{\partial C}{\partial r}\right)
+Q-\lambda C .$$

Where $D$ is uniform in a layer this is the form FRESCO's paper writes
(its Eq. 1, with $D(T)$ Arrhenius, $D=D_0\exp(-E/RT)$).

Each layer (kernel, buffer, IPyC, SiC, OPyC) has its own $D$. At each
interface, flux is continuous. Whether concentration is continuous, or
jumps by a partition ratio, is a modelling choice; the walk of step 8
used a ratio $K$, default 1.

This is the **same physics** as the random walk. The walk is a Monte
Carlo solution of this equation (step 8, section 1). Here we solve it
directly, so there is no statistical noise, only discretisation error.

**Cost.** A mesh in $r$, a time loop, and a linear solve each step.
The number of unknowns grows with the resolution you need near thin,
low-$D$ layers such as SiC.

---

## 2. What FRESCO does

FRESCO-II is a fuel-element release code from Forschungszentrum
Jülich. The only source this book uses for it is:

> C. Fang, C. Li, J. Cao, K. Liu, S. Fang, "Analysis of Fission
> Products' Release in Pebble-Bed High-Temperature Gas-Cooled Reactor
> Fuel Elements Using a Modified FRESCO II Numerical Model", *Science
> and Technology of Nuclear Installations* 2021, 6627789,
> [doi:10.1155/2021/6627789](https://doi.org/10.1155/2021/6627789)
> (open access, CC BY; listed in the
> [open corpus README](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/kovan-literature/reactor-literature/theodore-open-corpus/README.md)).

What follows is what that paper says FRESCO does. Anything about
FRESCO's history, validation or code that is not in that paper is
marked.

### Geometry: spheres inside spheres

The paper treats the fuel element, the TRISO particle and the graphite
grains each as a **uniform sphere**, so one equation serves all of them
(its Fig. 2). The paper's example is an HTR-PM pebble of 60 mm diameter
with about 12,000 TRISO particles. The particle in its Table 1 has a
500 µm kernel diameter, and buffer, IPyC, SiC and OPyC layers 95,
40, 35 and 40 µm thick. For an intact particle, products must cross every coating,
then the graphite matrix, before reaching the coolant. A failed
particle, and uranium contamination in the graphite, release into the
matrix directly.

### Numerics: shells, linear profiles, a tridiagonal solve

As the paper describes it (Section 4):

1. Cut the sphere into $N$ thin shells. The paper's comparison uses 39
   shells over the kernel and four coatings.
2. Take the concentration to be **linear in $r$** between shell values,
   and quadratic at the centre (zero gradient there).
3. Write conservation for each shell: change = births $-$ decay $-$
   flux out $+$ flux in, with Fick's law for the flux.
4. Step in time by replacing the future concentrations on the right
   side with a **time average** of old and new values (the paper's
   Eq. 9).
5. This gives a banded system $M C^{k+1}=E^{k}$ (its Eq. 17).
   The matrix $M$ depends only on geometry and time step, not on time.

At the outer surface the paper uses a mass-transfer condition,
$-D \partial C/\partial r=\beta(C_{rp}-C_{gr})$, with $\beta$ an
interface coefficient.

### What FRESCO simplifies

- **A 1-D sphere in place of the real geometry.** The pebble, particle
  and grain are each idealised as a sphere.
- **A piecewise-linear concentration** within a shell, which needs
  enough shells to resolve steep gradients.
- **Temperature enters only through** $D(T)$ per material.

### The paper's own finding

The authors re-derived the FRESCO-II coefficients and report an index
error (a wrong subscript on a cubed radius, their Eq. 15), and a
treatment of the right-hand side that makes the model unphysical for
decay at large time steps. They coded a corrected version (FERRA, in
Python) and compared on Cs-137 and Cs-138 for 25,400 h over 600 to
1300 °C in the paper's Tables 3 and 4. Ratios (FRESCO-II over FERRA)
for Cs-137 are 0.9993 to 1.0049. For Cs-138, the short-lived case, they
are 1.0000 up to 1100 °C, falling to 0.9993 at 1200 °C and 0.9951 at
1300 °C. FRESCO-II reads low, which the paper attributes to
underestimated interlayer transport rates.

*A discrepancy to note:* the paper's abstract says the difference for
Cs-138 "can be more than 1%", while its Table 4 ratios differ from 1 by
at most 0.5%. Both are the paper's words; the table is the data.

> ~~**[citation needed: maintainer to supply]**~~ **Partly sourced
> 2026-10-21.** FRESCO-II's origin as the paper gives it is Krohn and Finken,
> Jül-Spez-212, 1983. IAEA-TECDOC-1645 (IAEA, Vienna, 2010, reference 156)
> lists the same report as Krohn, H. et al., *FRESCO II: Ein Rechenprogramm
> zur Berechnung der Spaltproduktfreisetzung aus kugelförmigen HTR
> Brennelementen in Bestrahlungs- und Ausheizexperimenten*, Jül-Spez-212,
> Forschungszentrum Jülich, June 1983. IAEA-TECDOC-978 (1997, chapter 3, p. 57)
> calls FRESCO "the pebble version of the KFA diffusion model", widely used for
> metallic fission-product release from a spherical element. Its place in
> later code packages beyond what Fang et al. state (step 11), and its
> validation record, are still **not** described here. Both IAEA reports are
> proprietary-folder sources.

### Where boon-lay stands against it

boon-lay does **not** contain a FRESCO-style multilayer transient
solver. A finite-volume solve of multi-layer diffusion with decay and
source (section 1 above) is what FRESCO does, and what step 8's random
walk reproduces in the large-crowd limit. The nearest thing in the crate is the closed-form
Crank series used as the *reference* in the CRP-6 Case 1 check
(step 8, section 5). Anyone wanting a FRESCO-like run needs to build one.

---

## 3. Simplification 1: Booth's equivalent sphere

FRESCO needs a mesh and a solve. Can we get a closed form?

**Booth's idea** (1957, as
[the release lesson](../../deep-dives/triso-atops/release.html) cites it)
is to replace the real multi-shell particle, for one chemical group,
with a **single uniform sphere** of radius $a$ and one diffusion
coefficient. The released fraction then depends on $D$ and $a$ only
through $D'=D/a^{2}$.

For a long-lived species with uniform start and a perfect-sink surface
(a bare-kernel problem) the solution is Crank's series:

$$\frac{M_t}{M_\infty}=1-\frac{6}{\pi^{2}}\sum_{n=1}^{\infty}\frac{1}{n^{2}}\exp\left(-n^{2}\pi^{2}D't\right),$$

with the early-time form $6\sqrt{D't/\pi}-3D't$. It is
[`booth_longlived`](../../code-review/#crates/boon-lay/src/triso_atops_fork/release_models/steady_state.rs::booth_longlived).

**The judgement that makes it acceptable.** Release is controlled by
one region, the kernel or the grain, and the layers around it do not
hold the product back. The equivalent radius $a$ is then a choice, and
for special metals TRISO-ATOPS forms $a=\sqrt{2 a_{\text{grain}} r}$
([`rb_fail`](../../code-review/#crates/boon-lay/src/triso_atops_fork/release_models/mod.rs::rb_fail)).
This formula is the upstream code's; why it is a good choice is not
derived here. [citation needed: maintainer to supply]

**When it fails.** When a layer in series is the bottleneck, as SiC
is in step 4. A single sphere has no SiC. That is why TRISO-ATOPS adds
a separate breakthrough model for silver (below).

**The temperature history.** In an accident $T$ changes with time, so
$D$ does too. For one uniform material, with $D$ a function of time
only, the substitution $\tau=\int D dt$ turns the problem into the
constant-$D$ one. The code uses $\int D dt$ in place of $Dt$
([`integrate_diffusion_over_time`](../../code-review/#crates/boon-lay/src/triso_atops_fork/diffusion/mod.rs::integrate_diffusion_over_time)).

---

## 4. Simplification 2: steady state

If decay is fast, the inventory in the sphere reaches equilibrium
between birth, decay and escape. Only a **release-to-birth ratio**
$\langle R/B\rangle$ is wanted, not the history. Solving the sphere
equation with the decay sink and no time dependence gives

$$\left\langle\frac{R}{B}\right\rangle=\frac{3}{\mu}\left(\coth\mu-\frac{1}{\mu}\right),\qquad \mu=\sqrt{\frac{\lambda a^{2}}{D}}.$$

Limits: $\mu\to0$ (slow decay or fast diffusion) gives 1, everything
born gets out. $\mu\to\infty$ gives $3/\mu$, so only atoms born near the
surface escape. Code:
[`booth_shortlived_fast_diffuse`](../../code-review/#crates/boon-lay/src/triso_atops_fork/release_models/steady_state.rs::booth_shortlived_fast_diffuse).

**Judgement:** the species lives long enough to reach equilibrium but
not so long that the sphere empties. **Fails** for transients, and in
the middle ground where neither limit holds; the code name itself says
"fast diffuse".

---

## 5. Simplification 3: closed forms for barrier and graphite

- **Breakthrough** (silver through SiC): the standard time-lag
  solution for permeation through a plane barrier of thickness $a$
  around a kernel of radius $r$,
  $\text{RF}=3Dt/(ra)-a/(2r)-\ldots$ (a decaying series),
  [`breakthrough_model`](../../code-review/#crates/boon-lay/src/triso_atops_fork/release_models/steady_state.rs::breakthrough_model).
  It treats SiC as a flat slab. That is fine while $a\ll r$, and
  is an assumption, not a check.
- **Graphite attenuation:** a hold-up factor $A_f\ge1$ for the matrix,
  capped at $10^{8}$,
  [`attenuation_factor`](../../code-review/#crates/boon-lay/src/triso_atops_fork/release_models/steady_state.rs::attenuation_factor).
  Not a release fraction.
- **Noble gases and halogens:** an empirical fit for the release from
  failed particles, not a diffusion solution
  ([`rb_fail_noble_gases`](../../code-review/#crates/boon-lay/src/triso_atops_fork/release_models/steady_state.rs::rb_fail_noble_gases)).

The ladder, in one table:

| Model | Assumes | Gives | Fails when |
|---|---|---|---|
| Random walk (step 8) | Brownian atoms | histories, noisy | cost $\propto N$ |
| Multilayer transient FV (FRESCO-type) | Fick per layer | $C(r,t)$ | mesh too coarse |
| Booth sphere | one region controls | $M_t/M_\infty$ | a layer is the bottleneck |
| Steady state | equilibrium | $\langle R/B\rangle$ | transients |
| Breakthrough, attenuation | slab, saturation | rates, factors | thick layers, early time |

---

## 6. TRISO-ATOPS, and how it differs from FRESCO

TRISO-ATOPS is Idaho National Laboratory's tool (MIT licence). boon-lay
carries a Rust fork of it in
[`triso_atops_fork`](../../code-review/#crates/boon-lay/src/triso_atops_fork/mod.rs),
taken at upstream commit `de374c8`. The fork's contract is code-to-code
agreement with upstream.

| | FRESCO (per Fang et al.) | TRISO-ATOPS (this fork) |
|---|---|---|
| Method | numerical, shells + banded solve | closed-form Booth, breakthrough, attenuation |
| Unknown | $C(r,t)$ per shell | release fraction or $\langle R/B\rangle$ |
| Layers | each with its own $D$ | one equivalent sphere per chemical group |
| Cost | mesh and linear solve | a handful of series evaluations |
| Scope in the fork | not implemented | normal operation and depressurisation accident, coolant activity, 84 nuclides |

Two caveats the records carry:

- The Booth series is cut at a fixed number of terms
  (`BOOTH_SERIES_TERMS = 5000`, upstream's value). At very small
  $D't$ it floors near $1.2\times10^{-4}$, which over-states Sr, Ba and
  Eu kernel release by up to 120 times at HTR-10 normal-operation
  temperatures (gh:#385). The port keeps it, since it is a port. See
  [the release lesson](../../deep-dives/triso-atops/release.html).
- Code-to-code agreement
  ([record](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/docs/triso-atops-code-to-code.md))
  shows the port computes what upstream computes. It does not show the
  physics is right for any particular reactor.

The Booth sphere and the random walk meet on the single uniform sphere:
the walk reproduced Crank to 0.0006 at 1200 °C
([step 8](./05-random-walk.md#5-does-the-crowd-give-the-right-answer)).
That agreement is what lets the two models be called complements. It
says nothing about multilayer cases, where no quantitative record
exists yet.

---

## What we assumed

- Fickian diffusion with Arrhenius $D(T)$ in every layer (inputs from
  steps 2 to 4).
- Concentration and flux continuity at interfaces (or a stated $K$).
- For Booth: one controlling region and a chosen equivalent radius.
- For steady state: equilibrium between birth, decay and escape.
- Transmutation in transit is **not** modelled in any of these.
- Only FRESCO facts from Fang et al. 2021 are used here; others are
  unsourced and flagged.

**Next question:** the random walk is faithful and noisy; the Booth
family is cheap and coarse. Is there a way to use both? That is where
boon-lay's own contribution starts:
[step 10, Ray's equations](./07-ray-equations.md).
