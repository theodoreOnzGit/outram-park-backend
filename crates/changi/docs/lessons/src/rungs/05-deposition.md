# Rung 5: decay in flight, and deposition

> **Research, education and V&V only.** Nothing on this page is for emergency
> planning or response, dose assessment for real people, licensing or any
> safety decision (`RESPONSIBLE_USE.md`).

> **Review status:** AI-assisted first draft, 2026-10-04, not yet reviewed by
> a human. Built from commit
> [`@@COMMIT_SHORT@@`](https://github.com/theodoreOnzGit/outram-park-backend/commit/@@COMMIT@@).
> [It doesn't tally](https://github.com/theodoreOnzGit/outram-park-backend/issues/new?title=Dispersion%20rung%205%20doesn%27t%20tally).

> **Demo:** [open this rung in the dispersion demo](../../../demos/dispersion/?rung=deposition): the dry-deposition footprint per unit release for krypton, iodine, caesium or bromine, with a half-life slider for decay in flight. Every calculation runs in a background worker, so the page stays live; the demo's "What's happening here?" link opens this page.

## The problem

A source term lists twenty nuclides, each released over its own time windows.
Krypton-89 has a half-life of 189 s; a puff lives 1200 s by default. Some of
the twenty are noble gases that never touch the ground; some are iodine and
caesium that stick. **Do we need twenty dispersion runs, and where does the
material that leaves the air go?**

This rung is `changi::activity`, which is **not a port**: it was written in
this workspace, has no upstream, and was checked by an independent sum and by
rung 4's plume comparison instead.

---

## Step 1. One run, not twenty: linearity

**Short answer.** One. The puff kernel is linear in mass and puffs add, so the
concentration from a release `Q` is `Q` times the concentration from a release
of 1. Run the puff train **once with unit mass** and scale afterwards.

**The formula.**

```text
chi(r; Q) = Q * chi(r; 1)         so carry  chi/Q  per unit release
```

**In the code.** The end-to-end example builds the dilution factors and
then the survey:

> **On the [Code Review map](../../../code-review/):** start at [`main`](../../../code-review/#crates/changi/examples/site_activity_survey.rs::main) and follow its callees (▸) to [`dilution_factors`](../../../code-review/#crates/changi/src/activity/chi_over_q.rs::dilution_factors). Each card opens the function's source and shows its review stamp.

Two details in
[`dilution_factors`](../../../api/changi/activity/chi_over_q/fn.dilution_factors.html)
are worth reading in the source: it normalises by the mass each segment
**actually** emitted (correct even under upstream's mass-doubling policy of
rung 4), and it **panics if every receptor is beyond the puffs' reach**
(`wind speed × puff_duration`), because an all-zero answer from a puff that
simply stopped existing is a silent wrong answer.

**Predict.** Decay is the catch. Kr-89 decays while it flies. Can one
unit-mass run still serve every nuclide?

---

## Step 2. Decay in flight: an exact transfer function

**Short answer.** Yes, provided the result is kept **binned by travel time**.
Decay multiplies a parcel's activity by `exp(-lambda t)`, where `t` is how long
that parcel has been flying, and the transport does not care. So decay is a
gain on the output of the transport, applied per age bin.

**The formula.** Transport with decay obeys

```text
dC/dt = K lap(C) - u . grad(C) - lambda C
```

Substitute `C = exp(-lambda t) C_0`: the diffusion and advection terms carry
the factor straight through, and the time derivative's `-lambda C` cancels the
decay term. So `C_0` obeys the decay-free equation, and decay is a multiplier
that depends **only on travel time**. Because `puff_dt` is an integer
multiple of `sim_dt`, every puff's age is exactly `a × sim_dt`, so the bins
carry travel time exactly:

```text
chi/Q (receptor r, segment s, nuclide n) = sum over ages a of  bins[r][s][a] * exp(-lambda_n * a * sim_dt)
```

Ignoring decay is not an option at these scales: for Kr-89 it would overstate
the far field by roughly 80×. For a daughter with an ingrowing parent the gain
is a Bateman solution rather than one exponential; the two-step case is
[`bateman_two_step`](../../../api/changi/activity/decay_transfer/fn.bateman_two_step.html),
and longer chains are **not implemented**.

**Which decay constant.** From `boon-lay`'s nuclide data, never from
`flexpart::decay::decay_constant`, which reproduces FLEXPART's truncated
`0.693147` for `ln 2` (right for a code-to-code comparison, wrong for physics).

**In the code.** The decay-weighted sum:

> **On the [Code Review map](../../../code-review/):** start at [`main`](../../../code-review/#crates/changi/examples/site_activity_survey.rs::main) and follow its callees (▸) to [`DilutionFactors::dilution`](../../../code-review/#crates/changi/src/activity/chi_over_q.rs::DilutionFactors::dilution). Each card opens the function's source and shows its review stamp.

**Show the absence.** There is no "decay in transit" loop that follows atoms.
The decay *emerges* from one multiplication per age bin, which is the whole
point of the transfer-function argument. A Monte Carlo decay simulation here
would sample a distribution whose mean is this closed form.

**Check.** `crates/buangkok/tests/changi_puff_train_vs_plume.rs` (gh:#380),
Gate 1: with a constant wind and a fixed class every puff has the same
history, so `chi/Q` is a plain sum over ages of the closed-form kernel, which
the test writes without any of `dilution_factors`' code, for a stable nuclide
and a Kr-89-like one. Bound `1e-11`, from the number of terms times machine
epsilon. Recorded 2026-09-29: **worst 3.9e-16 over 84 cases.**

**Re-run for this lesson** (`develop` `d4428668be`, one core (core 12, `-j 1`), 2026-10-04): worst `changi`-against-sum **3.919e-16**
(gate 1e-11); worst time-step error against the continuum 2.23e-5, as
recorded; `chi/Q` independent of `puff_dt` to 5e-16.

**Predict.** Krypton never sticks to the ground; iodine does. If the plume
deposits iodine as it travels, is there less iodine in the air downwind, and
does this code know?

---

## Step 3. Dry deposition: a flux to the ground

**Short answer.** The deposited activity per square metre is a **deposition
velocity** times the time-integrated air concentration at ground level.

**The formula.**

```text
D [Bq/m^2] = v_d [m/s] * TIC(z = 0) [Bq s/m^3],      TIC = chi/Q * Q
```

**The two codes' velocities.** They make opposite choices, and the lesson
should say so:

| | `changi::activity::deposition` | `buangkok::pydoseia::dose::deposition_velocity_m_per_s` |
|---|---|---|
| Groups | noble gas, halogen, particulate (`DepositionGroup`) | by element symbol |
| Default value | **none, by design**: the caller supplies `v_d`; the example's value is named `order_of_magnitude_placeholder` | 0 for H, C and noble gases; 0.1 m/s for F, Cl, Br; **1000 m/d** for everything else (IAEA SRS 19's screening value, as upstream cites it) |
| Iodine | halogen group, caller's velocity | **the aerosol value**, not the halogens' 0.1 m/s (upstream behaviour, kept) |

`changi` refuses to default `v_d` because published values for these groups
vary by more than ten times with surface, wind, stability and particle size;
quoting one from memory with a citation attached would be a tuned parameter
wearing a reference.

**Four limits `changi`'s deposition states up front**
([`deposition.rs` module doc](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/activity/deposition.rs#L3-L29)):

- **Dry only.** No wet scavenging, so the answer is **not an upper bound**:
  rain would raise it. That is the opposite of the usual conservative framing.
- **Diagnostic, not depleting.** Deposited activity is **not removed** from
  the plume, so far downwind both the air concentration and the deposition are
  over-predicted.
- **Evaluated at `z = 0`**, with a second receptor set paired by index for the
  breathing height.
- **One velocity per group**, no chemical speciation.

**In the code.**

> **On the [Code Review map](../../../code-review/):** start at [`main`](../../../code-review/#crates/changi/examples/site_activity_survey.rs::main) and follow its callees (▸) to [`dry_deposition`](../../../code-review/#crates/changi/src/activity/deposition.rs::dry_deposition). Each card opens the function's source and shows its review stamp.

**Check.** Internal consistency only
([`activity-consistency-checks.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/docs/activity-consistency-checks.md)):
linearity, noble gases depositing exactly zero, the pairing of the two
receptor sets. Deposition is **outside** gh:#380's independent check. No
deposition number in this workspace has been compared with a measurement.

**Predict.** It rains. What happens to the iodine?

---

## Step 4. What is missing: wet deposition and depletion

**Short answer.** Rain washes soluble gases and particles out of the plume
(**wet deposition**), and every deposited becquerel is one fewer in the air
(**depletion**). Neither is in the dispersion chain.

- **Wet deposition** exists in the workspace only as FLEXPART's scavenging
  kernels, ported in `changi::flexpart::wet_deposition` and verified
  code-to-code against the compiled Fortran, but FLEXPART is **deferred**
  (gh:#410) and drives nothing.
- **Depletion** exists nowhere. A source-depletion factor (removing deposited
  activity from the Gaussian as it travels) would be the standard addition.

Filed as a gap: [gh:#543](https://github.com/theodoreOnzGit/outram-park-backend/issues/543).

---

## Use, modify, create

```bash
cargo run --release -p changi --example site_activity_survey
cargo run --release -p changi --example chi_over_q_survey
```

The first runs the whole chain with an illustrative release (no `sembawang`,
no plant value): dilution factors, decay, deposition, air and ground totals.
Its printed reading notes are part of the lesson: everything is linear in the
release; Kr-88 deposits exactly zero; the I-131 deposition column uses an
uncited placeholder velocity; the near rows are low because the release is
30 m up; a receptor beyond the puff reach reads only a negligible tail. That
last note said "exactly zero" until this lesson's run showed 1.06e-19 Bq·s/m³
at 8000 m; it was corrected in the example and in `chi_over_q`'s module doc.

**Run for this lesson** (`develop` `d4428668be`, one core (core 12, `-j 1`), 2026-10-04): class D, 2 m/s, release at 30 m for 3600 s,
1 Ci each of Kr-88 and I-131; air at 1.5 m, deposition at the ground:

| x | Kr-88 air (Bq·s/m³) | I-131 air (Bq·s/m³) | I-131 deposited (Bq/m²) |
|---|---|---|---|
| 100 m | 8.83 | 8.87 | 4.04e-2 |
| 500 m | 2.29e6 | 2.33e6 | 2.32e4 |
| 1 km | 1.68e6 | 1.74e6 | 1.74e4 |
| 5 km | 9.59e4 | 1.12e5 | 1.12e3 |
| 8 km | 1.06e-19 | 1.25e-19 | 1.25e-21 |

Kr-88 deposits exactly zero at every distance. The I-131 deposition column
uses the example's uncited placeholder velocity (1e-2 m/s), so it is plumbing,
not a result. The 8 km row is beyond the 5 km reach and is the Gaussian tail
of the oldest puffs: **not exactly zero**, which corrected the example's own
reading note and `chi_over_q`'s module doc in this change.

- **Modify.** Change `PUFF_LIFETIME_S` so the 8000 m receptor is inside the
  reach. Predict its row before you run.
- **Create.** Using
  [`surviving_fraction_against_stable`](../../../api/changi/activity/survey/fn.surviving_fraction_against_stable.html),
  tabulate how much Kr-89 survives at each receptor against a stable nuclide,
  and compare with `exp(-lambda x / u)`.

**Predict (into rung 6).** We now have Bq·s/m³ in the air and Bq/m² on the
ground. What dose would a person standing there receive, and by how many
routes?

## Deliberate liberties

| Liberty | What it may cost |
|---|---|
| Undepleted plume, diagnostic deposition | Over-predicts air and ground far downwind for depositing species; not measured |
| No wet deposition | Under-predicts deposition in rain; not measured |
| Caller-supplied, uncited deposition velocities | The deposition column's absolute level is a placeholder |
| Daughters beyond two-step Bateman not chained | Missing ingrowth for longer chains |
| Shared transport field per depletion class | Exact for decay; an approximation once depletion is added (one field per class, not per nuclide) |

## Call-tree appendix

> **On the [Code Review map](../../../code-review/):** start at [`main`](../../../code-review/#crates/changi/examples/site_activity_survey.rs::main). Each card opens the function's source and shows its review stamp.

**Next:** [Rung 6: from air and ground to dose](./06-dose.md).
