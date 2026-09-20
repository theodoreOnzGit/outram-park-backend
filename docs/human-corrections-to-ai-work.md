# Human corrections to AI-generated work

A running log of defects in AI-assisted work in this workspace that **required
human domain expertise to catch** — where the AI output was plausible, passed
its own tests, and survived AI review, but was wrong in a way only a
practitioner would see.

## Why this file exists

`RESPONSIBLE_USE.md` states that AI-assisted output is untrusted draft material
until a human reviews it. This file is the **evidence base** for that rule: the
concrete cases, so the claim is not an abstraction and so the failure *modes*
can be recognised again.

It is deliberately not a list of AI mistakes in general. Ordinary bugs get
fixed and forgotten. What belongs here is narrower and more useful:

> A defect that an AI could not have caught by working harder, because the
> verification it would have written encodes the same misunderstanding as the
> code.

That is the class that needs a human in the loop, and it is worth being able to
point at real instances of it.

**How to read an entry.** Each records what was reported (usually accurate),
what was *actually* wrong, why the AI process missed it, and what test would
have caught it. The last field is the valuable one — it converts a war story
into a standing check.

---

## 2026-08-12 — Energy-equation boundary conditions were zero-gradient on both sides

**Crates:** `tampines-steam-tables`, `outram-park-fork-coolprop`
(`TampinesSteamArray`, `OPCPFluidArray`)
**Severity:** high — silently unphysical, and it blocked every heat exchanger
built on these arrays
**Beads:** `op-289n`, `op-2qsf`, `op-3lkj`, plus the workspace-wide audit bead

### What was reported

Agents investigating why a steam generator could not be driven from its inlet
reported, correctly and with reproducible measurements:

- `TampinesSteamArray::set_inlet_enthalpy` has no effect on the solution. With a
  uniform initial field the enthalpy array is bit-identical after 30 000 `step()`
  calls, with the boundary condition set both far above and far below the initial
  state, at every `dt` from 1e-4 to 1.25e-2 s.
- Root cause: `FvMatrix::solve` returns a field with **zero-gradient boundaries**.
  `step()` captured and re-stamped the boundary-condition template for `u` and
  `p` but **not for `he`**, so `self.he = he_new` discarded the prescribed inlet
  enthalpy after every energy solve.
- `OPCPFluidArray` appeared healthy — but only because the comparison driver
  re-issued the boundary condition every step. Applied once, it failed the same
  way.

Every one of those statements is true. The fix that followed — re-stamp the `he`
boundary condition after each solve — makes the setter work.

### What was actually wrong

**The energy equation had zero-gradient boundaries on both sides.** That is a
basic CFD error, and re-stamping restores the *value* while leaving the
*formulation* wrong.

- A zero-gradient inlet gives `h_face = h_cell0`, so the advective influx is
  `mdot * h_cell0` — **the domain advects in its own enthalpy**. It is
  self-referential, and the interior field is structurally indifferent to
  whatever is actually flowing in. That is precisely the observed symptom, and it
  would have persisted in a subtler form even with the setter "fixed".
- With zero-gradient at *both* ends, there is no boundary through which energy
  can enter or leave the domain, so the global balance
  `mdot*(h_out - h_in) = Q_sources` cannot even be posed.

The correct pairing for an advection-dominated energy equation:

| Boundary | Condition | Why |
|---|---|---|
| Inlet | **fixedValue** (Dirichlet) in enthalpy, or a flux BC carrying `mdot*h_in` | The incoming fluid carries its own enthalpy; the domain does not get to choose it |
| Outlet | **`inletOutlet`** (mixed, keyed on face-flux sign) | Degenerates to zeroGradient on outflow, where the downstream state is genuinely unknown; clamps to `inletValue` on inflow, so a reversal cannot advect the interior's own enthalpy back in unbounded |

Stated more generally, and as the maintainer put it: for a heat exchanger you
need the enthalpies at all **four** terminals (hot in/out, cold in/out), and from
the **previous timestep's thermodynamic state** you always apply an
**upwind-equivalent advection boundary condition** — the upstream value is
selected by flow direction rather than defaulted.

**`tuas_boussinesq_solver` already implements exactly this**, in
`single_control_vol/boundary_condition_interactions/advection_to_bcs.rs`: it
holds both candidate upstream states from the last timestep (a zero-gradient
extrapolated boundary enthalpy and the control-volume enthalpy), switches on
`if mass_flow_from_bc_to_cv > MassRate::zero()` — BC density on inflow, CV
density on outflow — and passes the enthalpies upstream-first, with the order
flipped between the `bc_to_cv` and `cv_to_bc` paths. The both-ends-zero-gradient
failure is structurally impossible there.

**The `rhoPimpleFoam` array ports never inherited it.** They are a different
lineage — an OpenFOAM finite-volume formulation built on `FvMatrix::solve` —
and took its zero-gradient default. So the workspace contained a correct
reference implementation of the very thing that was broken, in a sibling crate,
throughout.

Plain zeroGradient at the outlet is only safe while flow never reverses, and
**this workspace reverses routinely**: Edwards & O'Brien blowdown flashes and
reverses, natural circulation reverses at start-up and stagnation, and a
counter-flow heat exchanger has its two outlets at opposite ends.

### Why the AI process missed it

The phrase "zero-gradient boundaries" appeared *verbatim* in the agent's root-cause
report and was relayed into three subsequent briefs without anyone asking whether
zero-gradient is **physically admissible at an inlet**. The investigation
anchored on the symptom — "a setter does not stick" — and every step after that
was competent work on the wrong question.

The tests were green throughout. They were also written against the same mental
model as the code: a test that asks "does the field move toward the boundary
value" confirms the setter and says nothing about whether the boundary condition
should exist in that form at all.

### Tests that would have caught it

Both are now standing requirements for any fluid array in this workspace:

1. **Global energy balance.** Assert
   `mdot*(h_out - h_in) == lateral/source power + d(stored energy)/dt`, at steady
   state **and** through a transient. This catches every boundary defect of this
   class, including ones nobody has thought of. A per-setter test only ever
   catches the defect already known about.
2. **Reversal.** Drive a flow reversal and assert the enthalpy stays bounded
   between physically available limits. A forward-flow-only test passes
   *identically* with a wrong outlet boundary condition and therefore proves
   nothing.

### Related defects surfaced by the same investigation

Found, measured, and deliberately deferred rather than silently worked around:

- **`op-1fyp`** — `fvc::div(phi, he)` is pure central differencing, giving 10-15%
  dispersive over/undershoot at an advected thermal front. Note that
  `rhoPimpleFoam/central_upwind.rs` does **not** cover this: it is a
  *Mach-weighted* KNP dissipation term, so at heat-exchanger Mach numbers the
  blend goes to zero and pure central differencing is what remains. For a boiling
  stream an enthalpy undershoot can push a `(p,h)` flash out of range and panic.
- **`op-nnqi`** — boundary `phi` is never pressure-corrected; outlet mass deficit
  -0.7035% at n=8, -0.3150% at n=16, -0.1482% at n=32. First-order truncation
  rather than a conservation bug.

---

## 2026-08-12 — An excursion overlay depicted an explosion an HTGR cannot have

**Crate:** `outram-park-digital-twin-engine` (`src/components/excursion.rs`,
formerly `explosion.rs`)
**Severity:** moderate — physically misleading in an educational tool

### What was wrong

The overlay drew a shock front, radial debris and flying fragments, escalating
from an intensity threshold corresponding to roughly **1359 °C**. TRISO fuel
survives that comfortably, and an HTGR has no explosion mechanism available at
these conditions — helium coolant, so no phase-change energy release; graphite
core; low power density; large heat capacity.

Worse, the workspace's own committed sources contradicted the artwork. Kugeler,
Nabielek & Buckthorpe (2017), EUR 28712 EN, open tier: *"no single particle
failures, nor any noticeable caesium or strontium releases, were observed during
the first few hundred hours in any of the 1600 °C heating tests"*; near 100%
retention at 1600 °C for the first hundred hours or more; SiC becomes permeable
only at 1800 °C; release quoted in **hundreds of hours**, so it is
time-accumulating rather than prompt.

So the artwork depicted destruction across a temperature band in which cited
evidence shows retention — inverting the modular HTGR's central design claim.

### Why the AI process missed it

Nothing failed. The animation was internally consistent, well-tested and
well-documented; it was simply a claim about reactor physics that nobody had
checked against the literature already sitting in the repository. Dramatic
output also *reads* as more finished, which makes it less likely to be
questioned.

### The correction, and the part that mattered

Renaming `Destructive` → `FissionProductRelease` was cosmetic. **The substantive
fix was moving the escalation threshold from 0.35 (≈1359 °C) to 1.0 (1600 °C).**
Escalating at 1359 °C in gentler wording would have repeated the original error.
The 1230-1600 °C band now draws nothing over the fuel.

### Test that would have caught it

A test asserting no failure annotation appears in a temperature range where the
cited sources demonstrate retention. Now present as
`no_release_is_drawn_where_retention_is_demonstrated` — 369 temperatures strictly
between the landmarks, every one required to stay at the warning stage.

---

## 2026-09-20 — A pebble bed is a POROUS MEDIUM, not an explicit lattice

### What the AI did

Building the HTR-10 deterministic-vs-Monte-Carlo comparison, the assistant
switched the model from `assemble()` (homogenised bed) to
`assemble_explicit_triso()` (every pebble carrying an explicit 8340-particle
TRISO lattice), on the reasoning that the explicit geometry was "the real
model" and the homogenised one a cheap approximation to be escaped.

It then discovered that a single Monte Carlo point cost **over an hour without
finishing pass 1 of 2** at only 3000 histories — 22876 tiles each carrying a
TRISO lattice — and reported this to the maintainer as a hard constraint on
whether a parameter map was feasible at all, proposing to work around it.

### The correction

> *"Pebble beds are porous media! ... We use porous media heat transfer."*

The cost crisis was self-inflicted. A pebble bed **is** a porous medium, and the
porous-media treatment is the *appropriate physical model*, not a compromise
made for speed. The explicit lattice is the special-purpose instrument — useful
for verifying a self-shielding treatment at a few anchor points — and it is not
the thing a core model should be built on.

Three consequences the assistant had not drawn:

1. **This workspace already treats the bed as porous media**, on the thermal
   side, and has for some time: `crates/tampines/src/pebble_bed/`, with
   `src/htr10/kta.rs` and `src/htr10/zbs.rs` for packed-bed friction and
   effective conductivity. The neutronics should be consistent with the
   thermal-hydraulics, not modelled on a different footing.
2. **The double heterogeneity has its own machinery here** — chord length
   sampling (`stochastic/cls.rs`, CLS/SCLS) and `dh_universe.rs` exist precisely
   so a homogenised medium keeps TRISO self-shielding without explicit geometry.
   The assistant had worked on that code earlier in the same session and did not
   connect it.
3. **The goal's three temperatures are the porous medium's PHASE temperatures.**
   "Fuel temperature, graphite temperature, moderator temperature" are not three
   arbitrary knobs — they are the solid-fuel, solid-moderator and fluid phases a
   porous-media heat-transfer model already carries separately. Reading them as
   arbitrary inputs is what made the temperature axes look like a modelling
   question rather than a coupling question.

### Why the AI process missed it

The reasoning was "explicit geometry is higher fidelity, therefore more
correct", applied without asking what the *governing model* for a pebble bed
actually is. Fidelity was treated as a scalar to be maximised rather than a
choice of formulation to be justified.

It is also the search-before-building rule failing one level up: the assistant
searched for *code* that solved its stated problem, and did not search for how
this workspace already *models the thing* — the porous-media treatment was
sitting in a sibling crate and in the CLS machinery it had itself been editing
hours earlier.

The tell was present and misread: an hour-long single point should have
prompted "is this the right model?" rather than "how do I afford this model?".
Cost blowing up by an order of magnitude is evidence about the formulation, not
merely a budget problem.

### Test that would have caught it

None would. No test can catch reaching for the wrong formulation, because every
test is written against the formulation chosen. What would have caught it is
stating the governing model and its justification before building — recurring
failure mode 1 below, which this entry is a fresh instance of.

---

## Recurring failure modes

Patterns visible across entries, worth checking against before trusting AI work
in this workspace:

1. **Symptom-level diagnosis.** The reported cause is true but shallow; fixing it
   restores behaviour while leaving the formulation wrong. *Check: can you state
   the governing equation and its boundary conditions, and justify each one
   physically?*
2. **Tests that share the code's mental model.** Green tests are not evidence
   when the same misunderstanding wrote both. *Check: is there a test of a
   conserved quantity or an invariant, as opposed to a test of an API call?*
3. **Untested regimes.** Forward flow only, design point only, no reversal, no
   off-design. *Check: does any test exercise the regime where the thing under
   test actually matters?*
4. **Not checking what already exists.** Specifying *how* to build before
   searching the workspace for the building blocks. This workspace is 40+ crates
   of ported mature codes; the prior should be "this probably exists already".
5. **Plausible-looking output resists scrutiny.** Polished artwork, confident
   prose and precise-looking numbers all reduce the chance anyone checks the
   claim underneath.
6. **Fidelity treated as a scalar to maximise.** Reaching for the most explicit
   representation available, rather than the formulation the physics of the
   system calls for. Explicit geometry is not automatically "more correct" —
   a pebble bed is a porous medium, a homogenised medium with a
   double-heterogeneity treatment is its proper model, and the explicit lattice
   is a verification instrument. *Check: can you name the governing model for
   this system and say why it is the right one, before choosing a
   discretisation? And does the rest of the workspace already model this system
   that way?*
7. **Cost blow-up read as a budget problem.** When a chosen approach becomes an
   order of magnitude more expensive than expected, that is evidence about the
   formulation, not just an obstacle to route around. *Check: before optimising
   or working around the cost, ask whether the expensive thing is the right
   thing.*
