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

**The code walk.** The image term is the second exponential inside
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

**Animation.** *Not yet built* ([gh:#548](https://github.com/theodoreOnzGit/outram-park-backend/issues/548)): the plume leaving the stack, rising and bending
over, with `H_e` marked.

**The code walk.** There is no caller in the library, so the walk starts from
the example written for this rung:

<!-- code-walk: from=crates/buangkok/examples/plume_rise_and_wake.rs::main to=crates/buangkok/src/pydoseia/plume_rise.rs::plume_rise_neutral_unstable -->
<!-- generated by `kovan-cli code-walk-check --update`; edit the comment above, not this -->

Call chain from `plume_rise_and_wake.rs::main` to `plume_rise.rs::plume_rise_neutral_unstable`: 1 hop, 1 shortest chain.

- [`plume_rise_and_wake.rs::main`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_rise_and_wake.rs#L94) `fn main()`
  - [`plume_rise.rs::plume_rise_neutral_unstable`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/plume_rise.rs#L37) `pub fn plume_rise_neutral_unstable(w0: f64, x: f64, u: f64, d_i: f64, d_e: f64) -> f64` — `compute_plume_rise_neutral_unstable_cat(W0, x, U, D_i, D_e)` (classes A-D), m: the smaller of `1.44 D_i (W0/U)^(2/3) (x/D_i)^(1/3) - 3 (1.5 - W0/U) D_e` and `3 D_i W0/U`. · called at [L98](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_rise_and_wake.rs#L98)
<!-- /code-walk -->

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

**The code walk.**

<!-- code-walk: from=crates/buangkok/examples/plume_rise_and_wake.rs::main to=crates/buangkok/src/pydoseia/plume_rise.rs::building_wake_gifford -->
<!-- generated by `kovan-cli code-walk-check --update`; edit the comment above, not this -->

Call chain from `plume_rise_and_wake.rs::main` to `plume_rise.rs::building_wake_gifford`: 1 hop, 1 shortest chain.

- [`plume_rise_and_wake.rs::main`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_rise_and_wake.rs#L94) `fn main()`
  - [`plume_rise.rs::building_wake_gifford`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/plume_rise.rs#L119) `pub fn building_wake_gifford(chi_over_q_unwaked: f64, building_area_m2: f64, wind_speed_m_per_s: f64, sigma_y_m: f64, sigma_z_m: f64) -> f64` — **Divergence from upstream (D5 corrected):** Gifford's building-wake dilution factor `chi/Q = 1 / ((c A + pi sigma_y sigma_z) U)`, `c = 0.5`, floored at one third of the unwaked value, s/m^3. · called at [L134](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_rise_and_wake.rs#L134)
<!-- /code-walk -->

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

<!-- code-walk: from=crates/buangkok/examples/plume_rise_and_wake.rs::main depth=3 -->
<!-- generated by `kovan-cli code-walk-check --update`; edit the comment above, not this -->

Everything `crates/buangkok/examples/plume_rise_and_wake.rs::main` reaches in the workspace, to 3 hops: 17 functions, 0 unresolved calls. A function is expanded once; later calls to it say *(expanded elsewhere in this walk)*.

- [`plume_rise_and_wake.rs::main`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_rise_and_wake.rs#L94) `fn main()`
  - [`plume_rise.rs::plume_rise_neutral_unstable`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/plume_rise.rs#L37) `pub fn plume_rise_neutral_unstable(w0: f64, x: f64, u: f64, d_i: f64, d_e: f64) -> f64` — `compute_plume_rise_neutral_unstable_cat(W0, x, U, D_i, D_e)` (classes A-D), m: the smaller of `1.44 D_i (W0/U)^(2/3) (x/D_i)^(1/3) - 3 (1.5 - W0/U) D_e` and `3 D_i W0/U`. · called at [L98](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_rise_and_wake.rs#L98)
    - [`plume_rise.rs::pw`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/plume_rise.rs#L27) `fn pw(v: f64, e: f64) -> f64` — Python's `v ** e` on floats: C `pow`, with the exponent hidden from LLVM so it is not strength-reduced. · called at [L39](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/plume_rise.rs#L39)
  - [`plume_rise.rs::plume_rise_stable_upstream`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/plume_rise.rs#L62) `pub fn plume_rise_stable_upstream(w0: f64, u: f64, d_i: f64) -> f64` — `compute_plume_rise_stable_cat(W0, U, D_i)` **as upstream computes it** (defect D6): the stability parameter is assigned for class E and then overwritten with class F's, and the calm formula is computed and then overwritten by the windy one, so the result is always `1.5 S_F^(-1/6) (Fm/U)^(1/3)` with `S_F = 1.75e-3`. · called at [L102](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_rise_and_wake.rs#L102)
    - [`plume_rise.rs::momentum_flux`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/plume_rise.rs#L52) `pub fn momentum_flux(w0: f64, d_i: f64) -> f64` — Upstream's momentum flux parameter `Fm = W0^2 (D_i/2)^2`. · called at [L63](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/plume_rise.rs#L63)
      - `plume_rise.rs::pw` *(expanded elsewhere in this walk)* · called at [L53](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/plume_rise.rs#L53)
    - `plume_rise.rs::pw` *(expanded elsewhere in this walk)* · called at [L65](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/plume_rise.rs#L65)
  - [`plume_rise.rs::plume_rise_stable_both_formulas`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/plume_rise.rs#L92) `pub fn plume_rise_stable_both_formulas(w0: f64, u: f64, d_i: f64, class: StableClass) -> StablePlumeRise` — **Divergence from upstream (D6 corrected):** uses the stability parameter of the class asked for and returns **both** formulas upstream writes, instead of discarding the first. · called at [L103](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_rise_and_wake.rs#L103)
    - `plume_rise.rs::momentum_flux` *(expanded elsewhere in this walk)* · called at [L98](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/plume_rise.rs#L98)
    - `plume_rise.rs::pw` *(expanded elsewhere in this walk)* · called at [L104](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/plume_rise.rs#L104)
  - [`plume_rise_and_wake.rs::ground_chi_over_q`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_rise_and_wake.rs#L83) `fn ground_chi_over_q(class: StabilityClass, h_m: f64, x_m: f64, u10: f64) -> f64` · called at [L113](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_rise_and_wake.rs#L113)
    - [`dispersion.rs::dilution_single_plume_no_met`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L293) `pub fn dilution_single_plume_no_met(x: Length, geometry: PlumeGeometry, scaling: MeanSpeedScaling) -> [DilutionFactor; 6]` — Dilution factor for an **instantaneous (single-plume) release without met data**, one value per stability class A-F, s/m^3 (time-integrated concentration per Bq released, at unit wind speed times the height correction). · called at [L90](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_rise_and_wake.rs#L90)
      - `dispersion.rs::height_correction_factor` *(expanded elsewhere in this walk)* · called at [L301](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L301)
      - [`dispersion.rs::master_equation_single_plume`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L197) `pub fn master_equation_single_plume(sigma_y: Length, sigma_z: Length, speed_factor: f64, release_height: Length, receptor: Receptor) -> MasterEquationTerms` — Single (instantaneous / short-term) Gaussian plume, Hukkoo-Bapat eq. · called at [L302](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L302) · *(calls below the depth limit not shown)*
      - `dispersion.rs::sigma_y` *(expanded elsewhere in this walk)* · called at [L303](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L303)
      - `dispersion.rs::sigma_z` *(expanded elsewhere in this walk)* · called at [L304](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L304)
      - `dispersion.rs::StabilityClass::index` *(expanded elsewhere in this walk)* · called at [L309](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L309)
      - [`dispersion.rs::kqij`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L268) `fn kqij(terms: MasterEquationTerms, sumnu: f64, hours_denominator: f64) -> f64` — Frequency-weighted accumulation used by all three modes, in upstream's operation order: `KQIJ = pre * expo * SUMNU; KQIJ = (KQIJ * 3600) / (hours * 3600)`. · called at [L309](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L309) · *(calls below the depth limit not shown)*
      - [`dispersion.rs::apply_scaling`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L273) `fn apply_scaling(values: [f64; 6], scaling: MeanSpeedScaling) -> [DilutionFactor; 6]` · called at [L311](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L311) · *(calls below the depth limit not shown)*
    - [`dispersion.rs::StabilityClass::index`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L69) `pub const fn index(self) -> usize` — Zero-based index, 0 (A) to 5 (F). · called at [L90](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_rise_and_wake.rs#L90)
    - [`units.rs::DilutionFactor::seconds_per_cubic_meter`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/activity/units.rs#L53) `pub const fn seconds_per_cubic_meter(self) -> f64` — The value in seconds per cubic metre. · called at [L91](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_rise_and_wake.rs#L91)
  - [`dispersion.rs::sigma_y`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L94) `pub fn sigma_y(stability: StabilityClass, x: Length) -> Length` — Lateral plume spread `sigma_y = A_y x^0.9031`, m, for downwind distance `x`. · called at [L126](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_rise_and_wake.rs#L126)
    - `dispersion.rs::StabilityClass::index` *(expanded elsewhere in this walk)* · called at [L97](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L97)
  - [`dispersion.rs::sigma_z`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L107) `pub fn sigma_z(stability: StabilityClass, x: Length) -> Length` — Vertical plume spread `sigma_z = A_z x^q + r`, m, with three distance bands (`x < 100 m`, `100 <= x <= 1000 m`, `x > 1000 m`) exactly as upstream. · called at [L127](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_rise_and_wake.rs#L127)
    - `dispersion.rs::StabilityClass::index` *(expanded elsewhere in this walk)* · called at [L109](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L109)
  - [`dispersion.rs::height_correction_factor`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/dispersion.rs#L137) `pub fn height_correction_factor(stability: StabilityClass, release_height: Length, measurement_height: Length) -> f64` — Wind-speed correction from measurement height to release height, `(H / H_m)^p` with `p = n / (2 - n)`, `n = 0.2` (A-C), `0.25` (D), `0.5` (E-F). · called at [L129](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_rise_and_wake.rs#L129)
  - [`plume_rise.rs::building_wake_gifford`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/src/pydoseia/plume_rise.rs#L119) `pub fn building_wake_gifford(chi_over_q_unwaked: f64, building_area_m2: f64, wind_speed_m_per_s: f64, sigma_y_m: f64, sigma_z_m: f64) -> f64` — **Divergence from upstream (D5 corrected):** Gifford's building-wake dilution factor `chi/Q = 1 / ((c A + pi sigma_y sigma_z) U)`, `c = 0.5`, floored at one third of the unwaked value, s/m^3. · called at [L134](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/examples/plume_rise_and_wake.rs#L134)
<!-- /code-walk -->

**Next:** [Rung 4: starts, stops and turns, the Gaussian puff](./04-puffs.md).
