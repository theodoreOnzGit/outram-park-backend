# Rung 6: from air and ground to dose

> **Research, education and V&V only.** A dose on this page is a property of
> a model, never a dose to a real person or population, and nothing here is
> for emergency, regulatory, occupational, medical or licensing use
> (`RESPONSIBLE_USE.md`). Dose is kept in its own crate, `buangkok`, for
> exactly that reason: so the dispersion physics is not mistaken for a
> health-assessment capability.

> **Review status:** AI-assisted first draft, 2026-10-04, not yet reviewed by
> a human. Built from commit
> [`@@COMMIT_SHORT@@`](https://github.com/theodoreOnzGit/outram-park-backend/commit/@@COMMIT@@).
> [It doesn't tally](https://github.com/theodoreOnzGit/outram-park-backend/issues/new?title=Dispersion%20rung%206%20doesn%27t%20tally).

> **Demo:** [open this rung in the dispersion demo](../../../demos/dispersion/?rung=dose): dose against distance: the capstone's recorded points, the same release under the class and wind you pick (scaled by `chi/Q`, the capstone's own arithmetic), and Liu & Cao's published Table 9 drawn solid as context. Every calculation runs in a background worker, so the page stays live; the demo's "What's happening here?" link opens this page.

## The problem

Rung 5 left us with a time-integrated air concentration (Bq·s/m³) and a
ground deposit (Bq/m²) at each receptor. **What dose does a person standing
there receive, and by which routes?** There are five, and `buangkok` carries
all five from its port of pyDOSEIA:

| Route | What irradiates | Quantity it needs |
|---|---|---|
| inhalation | activity breathed in, irradiating from inside | TIC (Bq·s/m³) × breathing rate |
| submersion (cloudshine) | the cloud around you, semi-infinite | TIC |
| plume shine | the cloud overhead, finite | the plume's shape, integrated |
| groundshine | activity deposited on the ground | Bq/m², built up and decaying |
| ingestion | activity eaten in food grown in the deposit | deposition, transfer factors, diet |

---

## Step 1. Inhalation: a committed dose per becquerel breathed

**Short answer.** Breathe in a becquerel and the body keeps it for a while,
irradiating tissue for years. The **committed** dose sums that future dose
into one number per becquerel taken in, the inhalation dose coefficient.

**The formula.**

```text
E_inh [mSv] = TIC [Bq s/m^3] * B [m^3/s] * e_inh [Sv/Bq] * 1000
```

with `B` the breathing rate (pyDOSEIA: 8400 m³/y above age 1, 1400 m³/y
below) and `e_inh` the coefficient. US EPA Federal Guidance Report 11 defines
the "committed" part (EPA-520/1-88-020, 1988, open standard corpus,
`fgr-11-epa-520-1-88-020.pdf`, PDF page 15):

> "This sum, by convention taken over tho 50-year period following intake,•
> is known as the "committed" dose."

(The quotation is as the scanned text reads, OCR artefacts included.)

**In the code.** The pathway, from pyDOSEIA's driver as the port runs it:

> **On the [Code Review map](../../../code-review/):** start at [`main`](../../../code-review/#crates/buangkok/examples/pydoseia_assessment.rs::main) and follow its callees (▸) to [`inhalation_dose`](../../../code-review/#crates/buangkok/src/pydoseia/dose.rs::inhalation_dose). Each card opens the function's source and shows its review stamp.

The function is built on a dose-rate helper so that a map and the ported
pathway share one formula, in upstream's multiplication order:

```rust,ignore
{{#include ../../../../../buangkok/src/pydoseia/dose.rs:87:127}}
```

**The coefficients.** The port ships **no** coefficient data. `buangkok::coefficients`
supplies freely usable ones: FGR-11 Table 2.1 inhalation (adult only; ten
nuclides), and FGR-15 (2025) external coefficients. Two inconsistencies are
stated in its module doc rather than hidden: FGR-15 is ICRP 103 effective
dose, FGR-11 is ICRP 26/30 committed effective dose **equivalent** for
Reference Man, so adding them mixes two weighting schemes; and a non-adult
inhalation lookup returns `None`, never a number.

**Predict.** Standing in the cloud, does the krypton you do *not* breathe
also dose you?

---

## Step 2. Submersion: standing in the cloud

**Short answer.** Yes. Gamma and beta radiation from the cloud reaches you
from all round. The simplest model puts you in a **semi-infinite** uniform
cloud, so the dose rate is the local concentration times a coefficient.

**The formula.**

```text
E_sub [mSv] = TIC [Bq s/m^3] * h_sub [Sv m^3 / (Bq s)] * 1000
```

FGR-15 (EPA 402-R-25-001, July 2025 revision, open standard corpus,
`fgr-15-epa-402-r-25-001.pdf`, PDF page 31, §3.3.2.1) describes the geometry
behind `h_sub`:

> "The air submersion exposure geometry involves an individual standing in a
> large volume of uniformly contaminated air. It is assumed that the
> individual is standing on an uncontaminated flat surface of infinite area.
> The source of radiation is a semi-infinite cloud containing a uniformly
> distributed monoenergetic photon emitter of strength (1 Bq m-3) surrounding
> a human phantom standing on the air- ground interface."

and, on the next page (PDF page 32, §3.3.2.3):

> "The dose near the air-ground interface from a semi-infinite cloud source
> has been taken to be one- half that due to an infinite cloud source"

**What the assumption costs.** A real plume is not semi-infinite. Under a
narrow plume the semi-infinite coefficient **over**-states the dose on the
centreline; under an elevated plume that has not reached the ground it
**under**-states it (the cloud overhead still shines on you while the ground
concentration is near zero). The doc comment of
[`submersion_dose_rate_msv_per_s`](../../../api/buangkok/pydoseia/dose/fn.submersion_dose_rate_msv_per_s.html)
says so.

**In the code.**

> **On the [Code Review map](../../../code-review/):** start at [`main`](../../../code-review/#crates/buangkok/examples/pydoseia_assessment.rs::main) and follow its callees (▸) to [`submersion_dose`](../../../code-review/#crates/buangkok/src/pydoseia/dose.rs::submersion_dose). Each card opens the function's source and shows its review stamp.

**Predict.** How would you compute the dose from a cloud that is **not**
around you, but 100 m overhead?

---

## Step 3. Plume shine: the finite cloud

**Short answer.** Integrate. Every volume element of the plume emits photons;
each reaches the receptor attenuated by the air between, `exp(-mu r)`, and
boosted by scattered photons (a build-up factor `1 + k mu r`). Summing over the
plume is a **triple integral** per photon line.

**The formula.**

```text
dose rate  ~  sum over lines  E * mu_a * yield  *  integral over the plume of
              C(x, y, z) * (1 + k mu r) * exp(-mu r) / (4 pi r^2)  dx dy dz
```

with `r` the distance from the volume element to the receptor and `mu`, `mu_a`
the attenuation and energy-absorption coefficients of air at the line's
energy. pyDOSEIA cites Wang, Ling and Shi (*Nucl. Eng. Des.* 231, 2004) for its
integration limits in mean free paths.

**In the code.** Down to the integrator:

> **On the [Code Review map](../../../code-review/):** start at [`per_class`](../../../code-review/#crates/buangkok/src/pydoseia/plume_shine/mod.rs::per_class) and follow its callees (▸) to [`petir_tplquad`](../../../code-review/#crates/buangkok/src/pydoseia/plume_shine/integral.rs::petir_tplquad). Each card opens the function's source and shows its review stamp.

**The fork: two integrators.** By default the triple integral runs on
`petir`'s port of GSL QAGS (`PlumeShineIntegrator::Petir`); the port of SciPy's
QUADPACK that upstream calls stays as `PlumeShineIntegrator::ScipyQuadpackReference`,
the regression reference. Both are QUADPACK `dqagse`. Measured 2026-09-28: the
petir path is **bit-identical** to the reference on all 120 fixture integrals
([integrators](../ext/driver-and-integrators.md)).

**The catch: no photon data ships.** Gamma lines (IAEA) and air attenuation
coefficients (NIST) are **not** in the repository, because their terms of use
were not established. The code-to-code test runs on **synthetic** tables. So
plume shine is a verified translation that **cannot be run on real nuclides in
this workspace** until a caller supplies those tables, and pyDOSEIA's own
unit for the result is ambiguous (µSv/h in comments, µSv per year in its
report; defect D16), which is why the port returns plain "upstream values"
rather than a dose type.

**Predict.** Iodine settles on grass and caesium on the ground. Once the cloud
has gone, does the dose stop?

---

## Step 4. Groundshine: what the cloud leaves behind

**Short answer.** No. Deposited activity keeps irradiating from the ground
until it decays or weathers away. The dose is the deposit built up over the
exposure period times a ground-surface coefficient.

**The formula.** pyDOSEIA's arithmetic, kept by the port:

```text
deposition rate   = chi/Q * (Q / year) * v_d                         [Bq m^-2 s^-1]
ground conc.      = deposition rate * (1 - exp(-lambda_e T)) / lambda_e   [Bq m^-2]
dose              = ground conc. * h_gs * 1000 * year                 [mSv]
```

`lambda_e` is decay plus, optionally, soil weathering (`Weathering`).

**An upstream convention to know about.** For an **instantaneous** release,
pyDOSEIA uses the same long-term formula with `Q` spread over a year, then
exposes for a year at the resulting concentration. That is upstream's
convention, reproduced and stated in the doc comment of
[`ground_shine_dose`](../../../api/buangkok/pydoseia/dose/fn.ground_shine_dose.html).
The capstone (rung 7) does **not** use it: it deposits the activity at once
(`A = TIC × v_d`) and integrates the decaying deposit over its own 96 h dose
window, `A h_gs (1 - exp(-lambda T)) / lambda`.

**In the code.**

> **On the [Code Review map](../../../code-review/):** start at [`main`](../../../code-review/#crates/buangkok/examples/pydoseia_assessment.rs::main) and follow its callees (▸) to [`ground_shine_dose`](../../../code-review/#crates/buangkok/src/pydoseia/dose.rs::ground_shine_dose). Each card opens the function's source and shows its review stamp.

**Progeny.** Cs-137's groundshine is mostly its daughter Ba-137m's gamma.
`buangkok::coefficients` adds Ba-137m in secular equilibrium at FGR-15's
0.944 Bq per Bq. The Liu & Cao cross-check (below) measured that **without**
it the accident external dose came out about 15× lower.

---

## Step 5. Ingestion: the food chain

**Short answer.** Deposited activity moves into crops, pasture, milk and meat;
eating them commits a dose. pyDOSEIA follows IAEA SRS 19's screening food-chain
equations, with special specific-activity models for H-3 and C-14 (IAEA
TECDOC-1616).

**In the code.** The corrected driver, which computes each nuclide on its
own:

> **On the [Code Review map](../../../code-review/):** start at [`ingestion_dose_per_nuclide`](../../../code-review/#crates/buangkok/src/pydoseia/ingestion/corrected.rs::ingestion_dose_per_nuclide). Each card opens the function's source and shows its review stamp.

**The fork, and why it matters.** Upstream's ingestion driver has the
largest cluster of defects found in the port (D8–D14 in
[`pydoseia-code-to-code.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/docs/pydoseia-code-to-code.md)):
per-element lists indexed by position in the full nuclide list (D8), a
transposed output that the driver then scrambles across nuclides (D9: in one
fixture scenario an infant's reported ingestion for one nuclide is 11 860 mSv,
almost all of it another nuclide's vegetable dose), C-14 in Bq per **year**
used as Bq/m³ (D10: 3.15e7 times too large), a fallback diet in annual amounts
multiplied by 365 (D12). The faithful port reproduces every one,
`ingestion::upstream::ingestion_dose_upstream`; the corrected per-nuclide
driver is a labelled divergence.

**The catch, again: no transfer factors ship.** SRS 19's transfer factors and
the ingestion coefficients are IAEA/ICRP copyright and not in the repository.
Ingestion is a verified translation on synthetic tables, and **not run on real
nuclides anywhere in this workspace**. The capstone does not compute it.

---

## Step 6. The checks

**Verification against pyDOSEIA** (recorded 2026-09-28,
[`pydoseia-code-to-code.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/buangkok/docs/pydoseia-code-to-code.md#results)),
on synthetic tables: `dose_inhalation`, `dose_ground_shine`,
`dose_submersion` (128 cases each), `ingestion` (96), `plume_shine_single`,
`plume_shine_long_term`, `plume_shine_met`, all **exact**. Of 43 groups, 41 are
bit-exact; the other two are limited by a pandas reduction (2 ulp) and a
four-figure text report. 26 upstream defects are recorded (D1–D26, gh:#364).
**Re-run for this lesson** (`develop` `d4428668be`, one core, 2026-10-04): `cargo test --release -p buangkok --test pydoseia_code_to_code`, **15 of 15 tests pass** in 100 s, so the 2026-09-28 record holds at this commit. The petir-against-QUADPACK
regression test (`plume_shine_petir_vs_quadpack`) also passes (2 of 2, 23 s).

**Against a published calculation: consistent, not verified.**
`crates/buangkok/tests/liu_cao_external_dose_cross_check.rs` (gh:#379) feeds
Liu and Cao's (2002) HTR-10 releases (Table 5, normal operation; Table 8, two
design-basis accidents) through `buangkok`'s plume and FGR-15 external
coefficients, sweeping the meteorology the paper does not give (classes A–F,
1/3/5 m/s), and asks whether the published dose falls inside the resulting
**external-only** band. It is not a gate; the finding is the comparison.
Recorded 2026-09-29:

| Case | Published | `buangkok` band | Reading |
|---|---|---|---|
| Normal operation, 0.5 km (Table 7) | 1.1e-4 mSv/y | 2.82e-9 to 3.38e-4 | inside |
| Normal operation, 1.5–75 km | 1.4e-4 to 7.7e-6 mSv/y | below by 1.2× to 6.5× | expected: Table 7 also has inhalation and ingestion |
| Depressurisation, 0.25 km (Table 9, whole body) | 7.7e-2 mSv | max 1.42e-3 | published 54× above the band |
| Water ingress, 0.25 km | 2.0e-1 mSv | max 3.32e-3 | published 60× above |
| Both accidents, 45–75 km | | | published inside the band |

The test's own interpretation: two unstated inputs can each account for an
order of magnitude near the site (a 50-year groundshine integration would give
about 30× for Cs-137; inhalation and ingestion are not computed), and the
published dose falls 3.85e4× from 0.25 to 75 km where no single class here
falls more than about 70×, so the paper's meteorology or deposition differs in
kind. **Consistent, not verified: no contradiction, and no tight agreement.**

**Re-run for this lesson** (`develop` `d4428668be`, one core (core 12, `-j 1`), 2026-10-04, 1.67 s): all four tests pass, and the
printed bands reproduce the record with FGR-15's 0.944 Ba-137m branching
(for example depressurisation 0.25 km band maximum 1.419e-3 mSv, water
ingress 3.311e-3 mSv).

**Validation:** none. No dose here has been compared with a measurement.

**Predict (into rung 7).** Put it all together: a source term from a fuel
model, through the plume, into these pathways. Which pathway do you expect to
dominate close to the site for a release rich in iodine and caesium?

## Deliberate liberties

| Liberty | Whose | What it may cost |
|---|---|---|
| Semi-infinite cloud for submersion | FGR-15's geometry, pyDOSEIA | Over-states centreline dose under a narrow plume; under-states under an elevated one |
| Every age above 1 year breathes the adult rate | pyDOSEIA | Child inhalation dose with an adult breathing rate |
| Iodine deposits at the aerosol velocity | pyDOSEIA | Not measured |
| Instantaneous groundshine spread over a year | pyDOSEIA | Not the capstone's method; a known convention |
| FGR-11 (ICRP 26/30) added to FGR-15 (ICRP 103) | `buangkok::coefficients` | Mixed weighting schemes; common screening practice, stated |
| No photon, transfer-factor or ingestion data shipped | data licensing | Plume shine and ingestion cannot run on real nuclides here |

## Call-tree appendix

> **On the [Code Review map](../../../code-review/):** start at [`main`](../../../code-review/#crates/buangkok/examples/pydoseia_assessment.rs::main). Each card opens the function's source and shows its review stamp.

**Next:** [Rung 7: capstone, HTR-10 air ingress from release to dose](./07-capstone.md).
