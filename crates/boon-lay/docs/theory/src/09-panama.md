# Step 7 — PANAMA fuel failure: the pressure-vessel simplification, and ours

> **Research, education and V&V only.** Not for reactor operation, licensing, safety decisions or emergency response.
> **Review status: first draft, 2026-10-18, AI-assisted (Claude Sonnet), not yet reviewed by a human.**

**The question this page answers.** A finite-element model of every particle
is too heavy. *Which parts of it can be dropped, what makes each drop
acceptable, and when does each one stop being acceptable?*

**The question from the previous step.** [Step 6](./08-pressure-stress.md)
built the reference: gas pressure in the buffer, load on a layered shell,
creep and swelling in every layer. It also showed that `farrer-park` does not
yet have a TRISO model. The cheap alternative is much older.

## A naming rule first

Two things share a story here, and they are different evidence.

- **PANAMA-I** is a report: Verfondern, K. & Nabielek, H., *The Mathematical
  Basis of the PANAMA-I Code for Modeling Pressure Vessel Failure of TRISO
  Coated Particles under Accident Conditions*, Forschungszentrum Jülich,
  HTA-IB-03/90, 1 August 1990. "PANAMA" means this report and the results it
  prints.
- **boon-lay fuel failure** is this crate's own Rust code, written by an AI
  coding agent from the report's printed equations and then reviewed. This
  project has never had the PANAMA source code, so nothing here is "PANAMA's
  result" for a case the report does not print.

The report is restricted literature. The repo carries its equations and
constants with page numbers, and no prose or figure from it
([`mod.rs`](../../code-review/#crates/boon-lay/src/fuel_failure/mod.rs::total_failure_fraction),
[`README`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/README.md)).
The deep dive [When the particle breaks](../../deep-dives/triso-atops/fuel-failure.html)
goes equation by equation; this page is about the *choices*.

## The story in one picture

Think of the particle as a balloon with a stiff, brittle skin.

```text
 kernel -> gas -> buffer pores      pressure p  (ideal gas)
                      |
                      v
           +------------------+
           |  SiC thin shell  |  hoop stress  sigma = r p / (2 d)
           +------------------+
                      |
          compare with strength distribution (Weibull)
                      |
                      v
              fraction of particles failed
```

PANAMA-I as the repo records it picks the SiC layer as *the* load-bearing
member, computes one stress from one pressure, and compares it with a
**distribution** of strengths across the particle population. The answer is a
fraction of particles, not a yes or no for one particle. It adds a second
route to failure, SiC thermal decomposition, for temperatures where the layer
does not burst but disappears.

## Simplification 1: the whole particle becomes one thin SiC shell

**Dropped relative to Step 6.** The finite-element mesh, the PyC creep and
swelling, the kernel swelling, the contact between layers, and any bending or
non-spherical shape. The only structural member left is the SiC.

**The judgement that makes it acceptable.** The SiC is the stiff, strong
layer. In an accident most of the pressure load is carried by it, so one
number, its hoop stress, is the quantity that matters for burst. (This is the
report's modelling judgement. This page has not checked it against a
first-principles model, because `farrer-park` cannot yet run one.)

**The formula.** The report's Eq (2), written for a thin spherical shell:

$$\sigma_t = \frac{r  p}{2  d_o}\left(1 + \frac{\dot v  t}{d_o}\right)$$

with $r = \left(\tfrac{1}{2}(r_a^3 + r_i^3)\right)^{1/3}$ the report's average
radius (not the arithmetic mean), $d_o$ the original thickness, and
$\dot v  t / d_o$ the corrosion thinning of Simplification 4
([`induced_stress`](../../code-review/#crates/boon-lay/src/fuel_failure/stress.rs::induced_stress)).

**Where it breaks.**

- **Thick or thin-spot layers.** The report contains no thick-wall (Lame)
  formulation, and the code offers none. The thin-shell form drifts from the
  elastic solution as $d/r$ grows. At the illustration geometry of Step 6,
  $d/r$ is $35 / 268.6 \approx 0.13$.
- **A PyC layer that cracks or debonds.** Then the load path changes and the
  SiC sees something other than the gas pressure.
- **Non-spherical particles** and local flaws are outside a one-number stress.
- **No uncertainty is carried** on the stress itself: the strength spread of
  Simplification 2 takes all of the scatter.

## Simplification 2: strength is a distribution, and one stress meets all of it

**Dropped.** A particle-by-particle comparison. Each real particle has its own
flaws and its own geometry.

**The judgement.** Brittle ceramic strength scatters, and the scatter, not the
detail of each particle, is what sets a failure *fraction*. So draw the
strength from a Weibull distribution and ask what fraction of it lies below the
stress. The report's Eq (1):

$$\varphi_1 = 1 - \exp\left[-\ln 2 \left(\frac{\sigma_t}{\sigma_o}\right)^{m}\right]$$

([`weibull_failure_fraction`](../../code-review/#crates/boon-lay/src/fuel_failure/weibull.rs::weibull_failure_fraction)).

**The `ln 2` matters.** It makes $\sigma_o$ the *median* strength: at
$\sigma_t = \sigma_o$ exactly half the particles fail, for any $m$. A stock
Weibull law uses the characteristic strength, at which 63.2 % fail. Swapping
one for the other shifts the strength scale by $(\ln 2)^{1/m}$, about 4 % at
$m = 8$, in the flattering direction, and raises no error. A test,
`median_is_the_scale_parameter`, pins the median convention.

**Where it breaks.** The failure fraction is very sensitive to the modulus
$m$: a larger $m$ gives fewer failures below the median and more above it (a
test pins both directions). So the answer is only as good as $\sigma_o$ and
$m$, which are measured on unirradiated or irradiated material and have to be
supplied or degraded (Simplification 3).

## Simplification 3: irradiation degrades strength by a fitted law

**Dropped.** A microscopic account of how fast-neutron damage changes the
SiC.

**The judgement.** The report gives two correlations, Eqs (8a) and (9a), for
the irradiated median strength and Weibull modulus as a function of
irradiation temperature $T_B$ and fast fluence $\Gamma$, with floors of
196 MPa (Eq 8b) and 2 (Eq 9b). They are attributed in the code to Allelein
(1983) ([`strength.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/strength.rs)).
The code reproduces the report's Table 1 for all 16 entries, and that fixed
the unit of $T_B$ as kelvin: kelvin reproduces 16 of 16 entries, a literal
reading in degrees C reproduces 0 of 16
([units register](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/docs/panama-i-units-and-open-questions.md)).

**Where it breaks.** These are correlations, not physics. They are good where
they were fitted. For HTR-10, two inputs are not published and are taken, by
name, from the report's HTR-Module case: the unirradiated SiC strength and
modulus, and the fast fluence. That is a borrowing, not a measurement.

## Simplification 4: the gas is ideal and the release is a closed form

**Dropped.** The diffusion calculation of Steps 8 to 10 inside the kernel, and
any real-gas equation of state.

**The judgement.** Pressure is the ideal-gas law (Step 6, Eq 3) with the
released fraction $F_d$ from a Booth-type closed form of the kernel diffusion
([`booth.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/booth.rs)),
and CO from oxygen-per-fission correlations. Corrosion of the SiC by fission
products thins the layer at a rate $\dot v$ through Eq (7), and the code
carries a thinning factor from step to step so a varying temperature history
is handled.

**Where it breaks.**

- At high gas density the ideal-gas law may over- or under-state the
  pressure [citation needed: maintainer to supply how much].
- Whether the Booth-type closed form is adequate for a rapidly changing
  temperature history is not checked on this page (Not re-checked).
- The report states Eq (4) (the $F_d$ release) and Eqs (11) to (14b) have no
  printed figure or table to check them against, and the code's docs say so.
  The checks that remain are internal and algebraic.

## Simplification 5: SiC decomposition is a second failure route

**Dropped.** A resolved mechanism for how the SiC layer decomposes.

**The judgement.** Above about 2000 C the layer fails by decomposing into
gaseous Si and graphite, not by bursting. The report models it with an
Arrhenius action integral

$$\zeta = \int k(T)  dt, \qquad \varphi_2 = 1 - \exp(-\alpha  \zeta^{\beta})$$

with $k = (375 / d_o)  \exp(-556000 / (R T))$ and two calibrations of
$(\alpha, \beta)$: $(0.693, 0.88)$ for loose particles and $(0.0001, 4)$ for
particles in a sphere ([`decomposition.rs`](../../code-review/#crates/boon-lay/src/fuel_failure/decomposition.rs::decomposition_rate_constant)).

The unit of the 375 is not printed: for $k$ to be in per second with $d_o$ in
metres, 375 must be a velocity in m/s. The code reads it so and takes a
`Length` argument for this reason.

**Where it breaks.** The $\alpha$ and $\beta$ are calibrated to experiment,
which the report says must be done. Which of the two applies to a given
particle, and how far they extrapolate, is a judgement. The report's Fig. 6
rules out one calibration for loose particles, and a test pins that; nothing
else in the report checks Eqs (11) to (14b).

A structural point carries over to any transient code: $\zeta$ carries the
history, and $\varphi_2$ is read directly from it. It is *not* accumulated by
increments. Increments of $\varphi_2$ would give a wrong answer for a history
that heats and then cools. $\varphi_1$, by contrast, is accumulated from
positive increments. The accident driver steps the two side by side
([`AccidentHistory`](../../api/boon_lay/fuel_failure/history/struct.AccidentHistory.html)).

## Simplification 6: independent mechanisms, combined by survival

**Dropped.** Any interaction between failure routes.

**The judgement.** A particle survives only if it survives every route, so
survival probabilities multiply:

$$1 - \varphi = (1 - \varphi_o)(1 - \varphi_1)(1 - \varphi_2)$$

The fractions are not added, because a particle failed twice would be counted
twice ([`total_failure_fraction`](../../code-review/#crates/boon-lay/src/fuel_failure/mod.rs::total_failure_fraction)).
The as-manufactured term $\varphi_o$ is an **input**: not modelled.

**Where it breaks.** If the mechanisms are correlated (a thinned layer is
also a weaker one, for example), independence is wrong. Not checked here whether the report's
thinning law covers this coupling (Not re-checked).

## What this gives for HTR-10, and what it does not

All values below are boon-lay fuel failure's, from the crate's HTR-10 module,
not PANAMA's, because PANAMA was never run on HTR-10
([deep dive](../../deep-dives/triso-atops/fuel-failure.html)).

Accident hold of 200 h at constant temperature, after irradiation at
$T_B = 776$ C:

| accident $T$ | $\sigma_t$ | $\varphi_1$ | $\varphi_2$ |
|---|---|---|---|
| 1200 C | 50 MPa | $4.53 \times 10^{-9}$ | about 0 |
| 1600 C | 178 MPa | $3.10 \times 10^{-5}$ | $3.4 \times 10^{-15}$ |
| 2000 C | 370 MPa | $4.79 \times 10^{-3}$ | $2.78 \times 10^{-4}$ |
| 2200 C | 536 MPa | $6.07 \times 10^{-2}$ | 0.977 |

The shape is the model's own: $\varphi_2$ overtakes $\varphi_1$ between 2000
and 2200 C, as the report says it should. **These are not compared against
measured HTR-10 failure data** (the workspace's literature has none), and the
report claims good agreement only over about 1600 to 2500 C for German
fuel. HTR-10 fuel is German-lineage, so applying the model is defensible,
but it is an extrapolation and is reported as one.

Do not read the tiny normal-operation values of $\varphi_1$ as an in-service
failure fraction. In-service failure is a defect population, which the
equations take as an input. That would answer a different question.

**CORRECTED 2026-10-18** (Step 21 review of `htgr_sim_v1`): the digital twin evaluates $\varphi_1$ at end of irradiation with $T_B = T_{kernel} - 75$ K, adds it on top of the $5 \times 10^{-4}$ design $f_{inc}$, and does not route $\varphi_2$; see [Step 21](./20-htgr-sim-v1-chain.md) section 4.

## Why each simplification is allowed

**Kinds of justification used below.** (a) negligible effect, with an estimate; (b) computationally too expensive to be practical; (c) bounding or conservative, with the side it errs on; (d) data not available. **Open** means this page cannot justify it.

Each simplification above carries a justification, collected here. The
judgement text on the page above is the argument. This table names its kind.

| Simplification | Kind | Note |
|---|---|---|
| Whole particle becomes one SiC shell | (b) and (d) | FEM too costly per particle; PyC laws absent. Error against FEM unmeasured: **open** |
| Weibull strength over the population | (b) | Per-particle flaws cannot be known, and tracking them is impractical |
| Irradiated strength from the report's fit | (d) | No microscopic model or data in the repo; HTR-10 inputs are borrowed |
| Ideal gas, closed-form release | (d), and **open** for size | Real-gas error not estimated |
| Decomposition as an Arrhenius action integral | (d) | $\alpha$, $\beta$ are calibrated by the report; nothing in the report checks Eqs (11) to (14b) |
| Mechanisms independent, combined by survival | **Open** | Correlation between mechanisms is not examined |
| Applying the model to HTR-10 | (d) | An extrapolation, reported as one |
| Keeping the PANAMA-I conservative choices | (c) | The qualification record calls PANAMA-I a conservative design model that over-predicts failure |

## What we assumed

- The SiC alone carries the pressure, as a thin shell.
- Strength is Weibull-distributed, with the median as the scale (the `ln 2`).
- Irradiation changes strength by the report's fitted correlations.
- The gas is ideal, and $F_d$ comes from a closed-form release.
- SiC decomposition is an Arrhenius action integral with a calibrated shape.
- The failure routes are independent, combined by survival.

## Next question

The report's chain gives a failure fraction for a given temperature history.
It took the fraction of gas released from the kernel, $F_d$, from a closed
form, and said nothing about the fission products that leave through
*intact* coatings. That is normal diffusion, the next rung down.
[Step 8](./05-random-walk.md) starts it one atom at a time, and
[Step 9](./06-eulerian-fresco.md) takes the aggregate, FRESCO-style view.
Who later combined failure and release in one tool, and what this crate is
trying to do, is [Step 11](./10-stacy-boon-lay.md).
