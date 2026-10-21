# From a unit release to becquerels

## The problem

A source term lists twenty nuclides, each released over its own time windows.
Krypton-89 has a half-life of 189 s. A puff lives for 1200 s by default. Which
of these do we need: twenty dispersion runs, one per nuclide, or one?

**One.** This chapter shows why that is exact rather than an approximation,
and then follows the result through decay, deposition, and the end-to-end
example. All of it is in `changi::activity`, which is **not a port**: it was
written here, and has no upstream to compare against
([`activity/mod.rs`, lines 3–18](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/activity/mod.rs#L3-L18)).
[The V&V page](./vv-and-limits.md) covers how it was checked anyway.

## Step 1: run once, with unit mass

The puff kernel is linear in mass, and puffs superpose by summation. So the
concentration from a release of `Q` is `Q` times the concentration from a
release of 1:

```text
chi(r; Q) = Q * chi(r; 1)
```

That holds identically, so the puff train is run **once with unit mass** and
the answer scaled afterwards. It is the standard idiom in consequence codes
([`chi_over_q.rs`, lines 5–21](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/activity/chi_over_q.rs#L5-L21)).

## Step 2: keep the answer resolved by travel time

Decay is the catch. A puff's surviving fraction depends on how long it has
been travelling, and that differs from puff to puff, so a single scalar
`chi/Q` cannot carry it. Ignoring decay is not an option at these scales: for
Kr-89 it overstates the far field by roughly 80×.

The fix is to keep the response **binned by puff age**. Because `puff_dt` is
required to be an integer multiple of `sim_dt`, every puff's age is exactly
`a × sim_dt` for an integer `a`. Bin `a` therefore carries travel time
*exactly*, and the decay weight is exact too
([lines 23–42](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/activity/chi_over_q.rs#L23-L42)):

```text
chi/Q (r, s, n) = sum over a of  bins[r][s][a] * exp(-lambda_n * a * sim_dt)
```

Here `r` is the receptor, `s` the release segment and `n` the nuclide. The
accumulation is in
[`dilution_factors`](../../api/changi/activity/chi_over_q/fn.dilution_factors.html)
([lines 230–371](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/activity/chi_over_q.rs#L230-L371)),
and the decay-weighted sum in
[`DilutionFactors::dilution`](../../api/changi/activity/chi_over_q/struct.DilutionFactors.html#method.dilution)
([lines 161–196](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/activity/chi_over_q.rs#L161-L196)).

Two details in `dilution_factors` are worth reading:

- **The mass is counted, not assumed.** It normalises by the mass each segment
  *actually* emitted. That stays correct even under upstream's mass-doubling
  emission policy from [the puff-train page](./puff-train.md).
- **An all-zero answer is an error.** A puff older than `puff_duration` simply
  stops existing. At 4 m/s and 1200 s that is 4.8 km, so a receptor at 10 km
  would read ~~exactly zero~~ only the negligible Gaussian tail of the
  oldest puffs (**CORRECTED 2026-10-04**: measured 1.06e-19 Bq·s/m³ at
  8000 m in the example below, against 9.6e4 at 5000 m), with no warning. The function computes the reach
  and panics if *every* receptor lies beyond it
  ([lines 44–51](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/activity/chi_over_q.rs#L44-L51)).
  A mixed set is allowed, so check
  [`DilutionFactors::reach`](../../api/changi/activity/chi_over_q/struct.DilutionFactors.html#method.reach).

## Why decay separates exactly: a transfer function

Transport with decay obeys

```text
dC/dt = K grad^2 C  -  u . grad C  -  lambda C
```

Substitute `C = exp(-lambda t) C_0`. The diffusion and advection terms are
linear and carry the factor straight through. The time derivative produces a
`-lambda C` term that cancels the decay term exactly, leaving the decay-free
equation for `C_0`. So decay contributes a multiplier that depends **only on
travel time**, never on position within the puff
([`decay_transfer.rs`, lines 9–40](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/activity/decay_transfer.rs#L9-L40)).

The control-engineering picture fits exactly: the puff is the plant, and
decay is a first-order gain on its output. `N` nuclides cost one transport
solve and `N` multiplies. A Monte Carlo decay simulation here would buy
nothing, because it would sample a distribution whose mean is this closed
form.

### Where the shared field stops being shared

Decay is uniform. **Depletion is not.** Dry and wet deposition and
gravitational settling depend on chemical and physical form, so a noble gas,
an iodine and a caesium aerosol released together do not keep the same shape
in flight. The rule is therefore **one transport field per depletion class,
one scalar per nuclide within it**: three fields for an HTR-10 source term
(noble gases, halogens, particulates), not thirty
([lines 42–61](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/activity/decay_transfer.rs#L42-L61)).

For a daughter with an ingrowing parent the gain is a Bateman solution rather
than one exponential. The common two-step case is
[`bateman_two_step`](../../api/changi/activity/decay_transfer/fn.bateman_two_step.html).
Longer chains, and daughters that change depletion class mid-flight, are
**not implemented**.

### Which decay constant

Take decay constants from `boon-lay` [(Ong, 2026)](#ref-ong2026boonlay). **Do not** use
`flexpart::decay::decay_constant`: it reproduces upstream FLEXPART's truncated
`0.693147` in place of `ln 2`, which is right for a code-to-code comparison and
wrong for physics
([`chi_over_q.rs`, lines 167–170](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/activity/chi_over_q.rs#L167-L170)).

## Step 3: dry deposition

Activity deposited per unit area is the deposition velocity times the
time-integrated air concentration at ground level:

```text
D [Bq/m^2] = v_d [m/s] * TIC(z = 0) [Bq.s/m^3]
```

The module states four limits up front
([`deposition.rs`, lines 3–29](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/activity/deposition.rs#L3-L29)):

- **Dry only.** This layer does not apply wet scavenging, so the answer is
  **not an upper bound**: rain would raise it. That is the opposite of the
  usual conservative framing, and the limit most likely to be misread.
- **Diagnostic, not depleting.** Deposited activity is not removed from the
  plume, so at long range both air concentration and deposition are
  over-predicted.
- **Evaluated at `z = 0`.** Using a breathing-height concentration
  under-predicts deposition, so the survey takes two receptor sets paired by
  index, one for air and one for the ground.
- **One velocity per group.** There is no chemical speciation.

And one deliberate refusal: **there is no default deposition velocity.**
Published values for these groups are order-of-magnitude figures that vary by
more than ten times with surface, wind, stability and particle size. Quoting
one from memory with a citation attached would be a tuned parameter wearing a
reference. The caller supplies the velocity, and the placeholder the example
uses is named `order_of_magnitude_placeholder` so nobody can mistake it for a
sourced number
([lines 30–47](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/activity/deposition.rs#L30-L47)).

## Step 4: the survey, end to end

[`survey`](../../api/changi/activity/survey/fn.survey.html) is plain
arithmetic over the dilution factors, linear in the number of nuclides
([`survey.rs`, lines 3–14](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/activity/survey.rs#L3-L14)):

```text
TIC(r, n) = sum over segments of  chi/Q(r, seg, lambda_n) * Q(n, seg)
D(r, n)   = v_d(group_n) * TIC_ground(r, n)
```

The example `site_activity_survey` runs the whole chain with a prescribed,
illustrative release (no `sembawang`, no plant value). Here is the part that
builds the two sets of dilution factors and calls the survey:

```rust,ignore
{{#include ../../../examples/site_activity_survey.rs:142:161}}
```

Run it yourself:

```bash
cargo run --release -p changi --example site_activity_survey
```

The example prints its own reading notes, and they are worth reading as part
of the lesson
([lines 213–241](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/examples/site_activity_survey.rs#L215-L243)):

- everything is linear in the release;
- Kr-88 deposits exactly zero, because it is a noble gas;
- the I-131 deposition column uses an uncited placeholder velocity;
- the near rows are low because the release is 30 m up;
- 100 m is at the edge of the Pasquill–Gifford fit range;
- an unmoved puff contributes nothing;
- a receptor beyond the puff reach reads ~~exactly zero~~ only a negligible
  Gaussian tail (**CORRECTED 2026-10-04**, measured below).

That last note applies to this example itself. The wind is Singapore's mean
surface wind, 2 m/s, taken from `puff::climatology`, and puffs live 2500 s, so
the reach is 2 × 2500 = 5000 m. The 8000 m receptor is beyond it, so its row
~~should read zero~~ reads only the Gaussian tail of the oldest puffs:
**measured 2026-10-04**, 1.06e-19 Bq·s/m³ of Kr-88 against 9.59e4 at
5000 m (`develop` `d4428668be`, one core). Not exactly zero, and not a real
far-field value either. The comment on `PUFF_LIFETIME_S` said "at 4 m/s the reach is
10 km", from when the example ran at 4 m/s, until it was corrected on 2026-10-03
([lines 45–66](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/examples/site_activity_survey.rs#L45-L68)).
Run the example and check the last row.

## And dose?

`changi` computes **no dose quantity**. Dose coefficients and dose-rate
functions live in `buangkok` (`buangkok::coefficients` and `pydoseia::dose`).
That separation is deliberate: dose is a biological quantity, and keeping it
out of the dispersion crate stops the dispersion physics being mistaken for a
health-assessment capability. This deep dive stops at Bq·s/m³ and Bq/m².

## Try it

1. Using the decay-transfer argument, show that a stable nuclide's `chi/Q` is
   just the sum of the bins. Why does
   [`DilutionFactors::dilution`](../../api/changi/activity/chi_over_q/struct.DilutionFactors.html#method.dilution)
   say that a zero decay constant has "no rounding penalty"?
2. Kr-89 (`t½` = 189 s) travels 5 km at 2 m/s. What fraction survives? Compare
   with the "roughly 80×" overstatement quoted above, which assumes a 1200 s
   puff lifetime.

<!-- references:begin -->
## References

<p class="csl-entry" id="ref-ong2026boonlay" style="padding-left: 2em; text-indent: -2em;">Ong, T. K. C. (2026). <i>boon-lay: BOmbardment Open source Nuclide simulation Laboratory Algorithm for Yields (BOON LAY)</i>. GitHub. https://github.com/theodoreOnzGit/boon-lay</p>

<!-- references:end -->
