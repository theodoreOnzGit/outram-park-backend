# Rung 3: how high? Plume rise, ground reflection, building wake

> **Research, education and V&V only.** Nothing on this page is for emergency
> planning or response, dose assessment for real people, licensing or any
> safety decision (`RESPONSIBLE_USE.md`).

> **Review status:** AI-assisted first draft, 2026-10-04, not yet reviewed by
> a human. Built from commit
> [`@@COMMIT_SHORT@@`](https://github.com/theodoreOnzGit/outram-park-backend/commit/@@COMMIT@@).
> [It doesn't tally](https://github.com/theodoreOnzGit/outram-park-backend/issues/new?title=Dispersion%20rung%203%20doesn%27t%20tally).

> **Demo:** [open this rung in the dispersion demo](../../../demos/dispersion/?rung=rise-wake): the ground-level centreline `chi/Q` with and without pyDOSEIA's plume rise, and a ground release with and without a building wake, as you move the exit velocity, stack height and building size. Every calculation runs in a background worker, so the page stays live; the demo's "What's happening here?" link opens this page.

> **Status of this rung's physics: ported, not verified against its sources,
> and not used.** Every function on this page is a port of pyDOSEIA code that
> upstream defines **but never calls**. The faithful versions are checked
> code-to-code against upstream; the corrected versions are not checked
> against anything. Nothing in the workspace's dose path applies plume rise or
> building wake. Read this rung as "what the model leaves out, and what that
> costs", not as a working capability.

## The problem

Rung 1 put the release at the stack height `H` and held it there. A real stack
emits gas at several metres per second, often warm, so the plume **keeps
rising** after it leaves the stack. And a stack that is short next to its
building releases into the building's turbulent **wake**, which drags the
plume down. **How much do these change the ground-level concentration, and
does this workspace account for them?**

---

## Step 1. The ground, once more: reflect or absorb?

**Short answer.** Rung 1's image source assumes the ground **reflects**
everything. That is exact for a non-depositing gas such as krypton, and an
over-estimate of what stays airborne for anything that sticks to the ground.

**The formula.** The image term of rung 1,
`exp(-(z + H)^2 / 2 sigma_z^2)`, is the whole of the ground model. A partly
absorbing ground would multiply it by a reflection coefficient below 1, or,
equivalently, remove material from the plume as it deposits.

**In the code.** The image term is the second exponential inside
`master_equation_single_plume` (rung 1's walk) and inside `changi`'s puff
kernel (rung 4's walk). There is **no partial-reflection option** in either
port, and **no plume depletion** anywhere in the workspace: deposition is
computed as a diagnostic beside an undepleted plume (rung 5).

**Check.** Rung 1's conservation test and rung 4's puff mass test both
integrate the kernel over the whole line in `z` and get **twice** the release,
which shows the image is present; over `z >= 0` they get exactly the release,
which shows it reflects everything.

**Predict.** A stack releases at 10 m/s into a 2 m/s wind. Does the plume's
centreline end up nearer 30 m or nearer 100 m?

---

## Step 2. Plume rise: momentum and buoyancy

**Short answer.** The gas leaves the stack with upward **momentum** (and, if
it is hot, **buoyancy**), and the wind bends it over. The plume levels off at
an **effective height** `H_e = H + dh`, and rung 1's formula is then used with
`H_e` in place of `H`.

**The formula.** pyDOSEIA writes two momentum-rise formulas (upstream cites
the AERB/NF/SG/S-1 guide, p. 44, and IAEA-TECDOC-379; neither is in the open
corpus, so neither has been checked here):

```text
neutral and unstable (A-D):
  dh = min( 1.44 D_i (W0/U)^(2/3) (x/D_i)^(1/3)  -  3 (1.5 - W0/U) D_e ,   3 D_i W0/U )

stable (E, F), with S = 8.7e-4 (E) or 1.75e-3 (F) s^-2 and Fm = W0^2 (D_i/2)^2:
  dh = 4 (Fm/S)^(1/4)              (calm)
  dh = 1.5 S^(-1/6) (Fm/U)^(1/3)   (windy)
```

`W0` is the exit velocity, `U` the wind at stack height, `D_i` and `D_e` the
inner and outer stack diameters, and `x` the distance downwind (the rise grows
as `x^(1/3)` until it reaches the cap `3 D_i W0 / U`).

**Two things upstream gets wrong or leaves open**, both kept by the faithful
port:

- **Defect D6.** The stable formula assigns `S` for class E and then
  overwrites it with class F's, and computes the calm formula and then
  overwrites it with the windy one. As it runs, it is always class F, windy.
  The port reproduces that in `plume_rise_stable_upstream` and offers
  `plume_rise_stable_both_formulas` (the class's own `S`, both formulas) as a
  labelled divergence.
- **An open question, not a confirmed defect.** The neutral formula subtracts
  the downwash term `3 (1.5 - W0/U) D_e` at every `W0/U`. When the exit
  velocity exceeds `1.5 U` that term is negative and **adds** height. Whether
  the reference applies it only for `W0 < 1.5 U` cannot be checked until the
  reference is in the corpus. With upstream's defaults it decides whether the
  cap binds (see the run below). Filed with the reference check as a gap issue
  (linked at the bottom).

**Animation.** [In the demo](../../../demos/dispersion/?rung=rise-wake&step=rise): the plume leaving the stack, rising and bending
over, with `H_e` marked; the centreline is the stack plus the rise formulas
above, so it carries their open questions (gh:#542).

**In the code.** There is no caller in the library, so the walk starts from
the example written for this rung:

> **On the [Code Review map](../../../code-review/):** start at [`main`](../../../code-review/#crates/buangkok/examples/plume_rise_and_wake.rs::main) and follow its callees (▸) to [`plume_rise_neutral_unstable`](../../../code-review/#crates/buangkok/src/pydoseia/plume_rise.rs::plume_rise_neutral_unstable). Each card opens the function's source and shows its review stamp.

```rust,ignore
{{#include ../../../../../buangkok/src/pydoseia/plume_rise.rs:31:48}}
```

**Check.** The faithful functions are in pyDOSEIA's code-to-code fixture:
group `plume_rise`, 62 cases, **exact**, recorded 2026-09-28 in
[`pydoseia-code-to-code.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/docs/pydoseia-code-to-code.md#results).
The divergence `plume_rise_stable_both_formulas` has a unit test that it
reproduces upstream for class F and differs for class E; it is **not** checked
against the guide. That is verification of a translation only.

**Use, modify, create.**

```bash
cargo run --release -p buangkok --example plume_rise_and_wake
```

**Result** (run for this lesson, `develop` `d4428668be`, one core (core 12, `-j 1`), 2026-10-04; the record is the printout, quoted
here and in the example's module doc):

- **Neutral rise with upstream's defaults is pinned at the cap**: `dh = 75.0 m`
  at every distance from 50 m to 1 km, because the downwash term adds 84 m
  (`W0 / U = 5 > 1.5`). Had the term been zero for `W0 >= 1.5 U`, the rise would
  be about 57 m at 100 m and reach the cap only beyond about 230 m (arithmetic
  from the formula, not a run). This is the open question above, and it
  matters inside the first few hundred metres.
- **Stable rise:** upstream as it runs, 29.3 m; the divergence gives class E
  116.5 m (calm) or 32.9 m (windy), class F 97.8 m or 29.3 m.
- **What the rise does to rung 1's plume:** class D, 2 m/s, ground-level
  centreline at 1 km: `chi/Q` = 3.64e-5 s/m³ at `H = 30 m`, **1.86e-7** at
  `H + dh = 105 m`, a factor of about **200** lower. Leaving rise out of the
  dose path is not a small simplification for a fast, hot release.
- **Building wake** (illustrative 1000 m², ground release, class F, 1 m/s):
  the wake value sits **at the one-third floor** at 100 and 200 m, then rises
  to 0.41 of the unwaked value at 400 m and 0.76 at 1 km.

- **Modify.** Halve the exit velocity `W0` and rerun. Predict first: does the
  cap still bind at 1 km?
- **Create.** Rung 1's example prints the peak position. Run it with `H` set to
  the effective height this example prints, and see how far the peak moves.

**Predict.** A short stack sits next to a building that is taller than it.
Is the ground-level concentration just downwind higher or lower than rung 1
predicts?

---

## Step 3. Building wake

**Short answer.** Two effects, pulling opposite ways. The wake **drags an
elevated plume down** to the ground near the building, which raises the
ground-level concentration compared with rung 1's elevated release. And it
**mixes the release over the building's cross-section** almost at once, which
lowers the concentration compared with a point release at ground level.
Gifford's formula, the one ported here, treats the release as already at
ground level and models only the second effect.

**The formula.** Gifford's wake dilution, as the port's corrected version of
upstream's unusable function (defect D5) writes it:

```text
chi/Q (wake) = 1 / ( (c A + pi sigma_y sigma_z) U ),     c = 0.5,  A = building cross-section

floored at  (1/3) chi/Q (unwaked)
```

Upstream's version cannot run: it multiplies two bound methods
(`self.sigmay`, `self.sigmaz`) as if they were numbers (a `TypeError`), and
it **multiplies** by `U` where the formula divides. The port takes the sigmas
as arguments and divides, keeping upstream's one-third floor. It is **not
checked** against the AERB guide or TECDOC-379.

**In the code.**

> **On the [Code Review map](../../../code-review/):** start at [`main`](../../../code-review/#crates/buangkok/examples/plume_rise_and_wake.rs::main) and follow its callees (▸) to [`building_wake_gifford`](../../../code-review/#crates/buangkok/src/pydoseia/plume_rise.rs::building_wake_gifford). Each card opens the function's source and shows its review stamp.

**Check.** A unit test asserts the floor (`wake_never_drops_below_a_third`).
Nothing else. Read the example's wake column as "what the formula does", not
as a verified number.

---

## What this rung leaves the chain with

- The workspace's dose path (pyDOSEIA's, and the capstone in rung 7) uses the
  release height **as given**: no rise, no wake. For an elevated hot release
  that over-states near-field ground concentrations; for a ground release next
  to a building it may over- or under-state them, depending on distance.
- The capstone uses a **ground release** with no wake, stated there as one of
  its conservative assumptions.

**Gap filed:** plume rise and building wake are unverified against their cited
sources, unused in any pathway, and carry the open downwash question:
[gh:#542](https://github.com/theodoreOnzGit/outram-park-backend/issues/542).

## Deliberate liberties

| Liberty | Whose | What it may cost |
|---|---|---|
| Effective height = release height (no rise) in every pathway | pyDOSEIA, kept | Over-states near-field ground concentration for a buoyant or fast release; size shown by the example |
| No building wake in any pathway | pyDOSEIA, kept | Near-field over- or under-statement, not measured |
| Ground reflects fully | both ports | Over-states airborne concentration of depositing species far downwind (rung 5) |

## History

> **[HISTORY: needs a source]** Briggs's plume-rise formulas (1969, 1975) and
> Gifford's building-wake model are the history here; no open-corpus source
> is available yet.

## Call-tree appendix

> **On the [Code Review map](../../../code-review/):** start at [`main`](../../../code-review/#crates/buangkok/examples/plume_rise_and_wake.rs::main). Each card opens the function's source and shows its review stamp.

**Next:** [Rung 4: starts, stops and turns, the Gaussian puff](./04-puffs.md).
