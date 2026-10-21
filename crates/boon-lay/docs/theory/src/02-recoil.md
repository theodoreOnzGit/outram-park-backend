# Step 2 — The first micrometres: recoil and stopping

> **Research, education and V&V only.** Not for reactor operation, licensing, safety decisions or emergency response.

> **Review status: first draft, 2026-10-18, AI-assisted (Claude Sonnet), not yet reviewed by a human.**

*Previous: [Step 1](./01-generation.md) said which atoms are created. This
page says where they are when diffusion starts.*

## The question

A fission fragment is born with tens of MeV. An atom diffusing in a solid
moves a fraction of an eV at a time. **So where does a fragment end up before
diffusion even begins?** If it can leave the kernel, diffusion through the
layers is not the first barrier.

## The model with the fewest assumptions: slow down every fragment

A charged fragment loses energy in two ways:

- **electronic stopping**: it drags electrons along its path (dominant at high
  energy);
- **nuclear stopping**: it collides with whole atoms, and knocks them out of
  their sites (dominant at the end of the track).

Those knocked atoms are *recoils*, and they knock others in turn. The
**binary collision approximation** (BCA) follows each collision of a moving
atom with one lattice atom at a time, and every recoil above a cutoff energy.
The track, its recoils and the site where each stops is a *cascade*.

The assumptions are few: the material is described by its composition and
density, and the interaction is a screened-Coulomb potential. The cost is
high. boon-lay carries a copy of **RustBCA** (Drobny and Curreli, *J. Open
Source Softw.* 6(64), 3298, 2021, doi:10.21105/joss.03298, as the repo cites
it) in `boon_lay::rustbca`, for fission fragments, alphas and recoil knock-ons.

### What the repo measured (a cost probe, not a validation)

Run 2026-10-16 on HTR-10 layers unrolled flat, one thread
([record](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/verification_and_validation/rustbca_cascades/README.md)):

| particle (birth energy) | where | range µm (mean ± sem, n) | s per cascade |
|---|---|---|---|
| Sr-95 light fragment (100.47 MeV) | kernel | 10.69 ± 0.17 (16) | 1.47 |
| Xe-139 heavy fragment (68.66 MeV) | kernel | 7.41 ± 0.12 (16) | 2.00 |
| alpha (1.472 MeV) | kernel | 3.907 ± 0.033 (64) | 0.028 |
| alpha | buffer | 9.430 ± 0.012 (64) | 0.064 |

**Read these carefully.**

- The fragment ranges are labelled **illustrative** by the record. They have
  **not** been compared with measured or uncalibrated range data, because
  RustBCA's high-energy electronic stopping uses the bare projectile charge
  $Z a^2$ rather than an effective charge. Treat 7 to 11 µm as what this
  code gave, not as a validated range.
- The record's own prediction for the light fragment (4 to 9 µm) **missed**;
  the result was 10.7 µm. The predicted reason (too-strong stopping) was not
  supported.
- The cost prediction was too low as well: a fragment cascade tracks hundreds
  of thousands of recoils and takes 1.5 to 2 s on a core. That is why nobody
  runs this for every fission in every particle.
- A light fragment born 2 µm inside the kernel surface came to rest *outside*
  the kernel in 38 % of the 16 histories (n is small).

`[citation needed: maintainer to supply published fission-fragment ranges in
UO2 to compare these against.]`

## Why the buffer exists

The kernel radius is 250 µm, and the ranges above are 7 to 11 µm. Only
fragments born within roughly a range of the kernel surface can leave it, and
those cross into the **buffer**, a carbon layer about half voids
([rung 1](../../tutorials/triso-atops/triso.html)). The buffer therefore does
two jobs the page-1 table lists: it absorbs the fragments that leave the
kernel, so they do not damage the IPyC or SiC, and its voids give the released
gas somewhere to go (the free volume of step 6, taken in PANAMA-I as half the
buffer shell).

A lower density means a longer range, so the porous buffer must be thick
enough to hold what leaves the kernel. `[citation needed: maintainer to supply
the range of a fission fragment in porous carbon and the buffer-thickness
design argument.]`

## Simplification: a recoil fraction from geometry

Assume a fragment travels in a straight line of fixed length $R$ in a random
direction, born uniformly in the kernel of radius $a$. The probability that it
escapes is the fraction of the kernel volume within reach of the surface.

For $R \ll a$ that is a thin shell, and with the **direction** averaged over
the sphere the escaping fraction is about

$$f_{\text{recoil}} \approx \frac{3}{4} \frac{R}{a}.$$

It is derived here by the arithmetic below, not taken from a source.

*Arithmetic.* Near a flat surface, a point at depth $z<R$ escapes if its
direction points outward within the cone where $\cos\theta > z/R$. Over the
sphere of directions that probability is $\tfrac{1}{2}(1 - z/R)$. The volume
fraction of the shell at depth $dz$ is $3 dz/a$ (shell area $4\pi a^2$ over
volume $\tfrac{4}{3}\pi a^3$). So
$f = \int_0^R \tfrac{1}{2}(1 - z/R) \tfrac{3}{a} dz = \tfrac{3}{4} R/a$.

*Illustration only, with the table's number:* $R = 10 \mu\text{m}$ and
$a = 250 \mu\text{m}$ give $f \approx 0.75 \times 0.04 = 0.03$, about 3 %
per fragment. (This is an order-of-magnitude illustration from an illustrative
range, not a result.) The cascade record's one "2 µm inside" case, 38 % out
of 16, is a different, deliberately near-surface experiment, and is not
comparable.

- *The judgement:* you may use a single range $R$ when the spread of ranges
  (set by fragment mass and energy: light and heavy fragments differ in the
  table above, 10.7 against 7.4 µm) is small against $a$, and straggling can
  be ignored.
- *When it fails:* a small kernel (large $R/a$), a surface that is not smooth
  (a fragment can leave through a crack or a grain edge), or a material that
  stops fragments unevenly.

## What we assumed

- Fragments start uniformly in the kernel (not in the actual burnup-dependent
  power distribution).
- A fixed straight-line range, or in the full model, RustBCA's cutoff-limited
  cascade, whose stopping power is not yet validated.
- Recoil only matters for where the atom starts; after it stops, it diffuses
  (step 3).

## Next question

[Step 3](./03-layers-chemistry.md): the atom has stopped. How does it move
through the kernel, and then through every layer?
