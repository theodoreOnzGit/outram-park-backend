# Step 20 — The whole chain in one worked example: htgr_sim_v1, and a route for reviewing it

> **Research, education and V&V only.** Not for reactor operation, licensing, safety decisions or emergency response.
> **Review status: first draft, 2026-10-18, AI-assisted (Claude Sonnet), not yet reviewed by a human.**

**What this page is.** A walkthrough of source code. `htgr_sim_v1` is an
offline teaching model of a *published research reactor* (HTR-10). It is not
an operational tool, not a safety tool, and nothing on this page predicts what
a real plant would do. The page links no running copy of it and quotes no new
measurement: every number is one the repo already records, labelled
"recorded", with its location.

**Two questions.** First: Steps 1 to 19 each gave a ladder from the model with
the fewest assumptions to a cheaper one. *Which rung does each link of one
real chain actually stand on, and what did it assume to get there?* Second,
the maintainer's purpose for this page: *in what order should a reviewer read
the functions that implement the chain, and what should they check in each?*

**The question from the previous step.** [Step 19](./19-levels-of-fidelity.md)
named the levels. This page shows one chain with a level chosen at every link.

## 0. How to read the review notes

Each link below lists functions in **reading order** (entry point first, then
callees). For each, the Code Review link, then:

- **Check**: the equation or rule it should implement, and the step of this
  book that derives it.
- **Assumes**: what it encodes that a test cannot see.
- **Units**: what crosses its boundary.
- **Review**: what the repo's `review.md` files record. **Only
  `crates/boon-lay/src/fuel_failure/review.md` holds human review records**
  (31 entries, all dated 2026-10-13, signed `github:theodoreOnzGit`, recorded
  as `rung = 3`). Every other `review.md` beside the chain's code (the
  TRISO-ATOPS fork and its `activities`, `diffusion`, `normal_operation`,
  `release_models` and `accident` folders, `triso_atops_extensions`, `changi`
  `puff` and `activity`, `bishan`, `sembawang`, and `htgr_sim_v1`'s own
  `physics`) is the empty "created by Index fresh" note. A read of all of them found no review entry.
  "**None recorded**" below means exactly that: no review entry exists. It
  does not mean the code was found wrong. This page does not interpret what "rung 3"
  signifies.

## 1. The call path in one picture

```text
Plant::step_with_correctors            (physics/mod.rs)
 |
 |- scenario switch (first step only)
 |    DlofcAtws -> release.depressurisation_lift_off
 |    WaterIngress -> release.wash_off_plate_out
 |
 |- release.set_primary_flow
 |- release.update(sim_time, fuel stack)
 |    evaluate_stack -> evaluate_with_sic
 |      fractions_at -> end_of_irradiation_failure   (failure)
 |      normal_operation_node                        (particle release)
 |      htr10_pool_rates                             (removal rates)
 |    live_pools::step                               (coolant pools)
 |    set_uncredited_stack                           (no building)
 |
 |- vent_circulating / release_stored_noble_gas      (accident)
 |
 |- dispersion.update(sim_time, release)
 |    evaluate -> dilution_pair -> changi puff       (dispersion)
 |    survey -> deposition, decay in transit
 |
 '- tede.accumulate (dose rate via buangkok)         (indicative dose)
```

The numbered comments in `step_with_correctors` run these in this order, with
dispersion last and "a pure consumer of converged state that feeds nothing
back" (the code's own words). Start a review at
[`Plant::step_with_correctors`](../../code-review/#crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/mod.rs::Plant::step_with_correctors)
and follow the branches below.

## 2. Link A: generation (the inventory)

**Fewest assumptions (Step 1):** compute the inventory from fission yields,
burnup and transmutation.

**What htgr_sim_v1 does:** it **looks the inventory up**. The five tracked
nuclides, `Kr-85, Xe-133, I-131, Cs-137, Ag-110m`
([`TRACKED_NUCLIDES`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/fission_product_release.rs)),
take their equilibrium-core activity from the published HTR-10 table (Liu &
Cao 2002, Table 1, ORIGEN2 at 80 000 MWd/t, as the code comment cites it).
There is **no burnup, no yield, no transmutation** in the plant model. A code
comment explains the refusal: independent yields would silently give the wrong
answer for species such as Cs-137, where cumulative yield is what a source
term needs.

**Fidelity level:** a table (below the first rung of Step 1).

**Review route:**

1. [`htr10_core_inventory_bq`](../../code-review/#crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/fission_product_release.rs::htr10_core_inventory_bq).
   Check: it returns the table value, in becquerels. Units: Bq. Review: none
   recorded.
2. [`htr10_core_inventory`](../../code-review/#crates/changi/src/activity/inventory.rs::htr10_core_inventory)
   in `changi`. Check: the transcription against the table. Assumes the
   table is one equilibrium core. Review: none recorded.

## 3. Link B: release from the particle

**Fewest assumptions:** the Lagrangian random walk of
[Step 5](./05-random-walk.md), or the Eulerian solution with Ray's equations
of [Step 7](./07-ray-equations.md).

**What htgr_sim_v1 does:** neither. It runs **TRISO-ATOPS's closed forms**
(the Booth equivalent sphere for the kernel, a breakthrough model for silver
through SiC, a graphite attenuation factor), with Arrhenius diffusion
coefficients. A search of the example's source for the walk (`first_passage`,
`WoSWalker`, `lagrangian_decay`) and for FLEXPART found no use. This is
the Booth rung of [Step 6](./06-eulerian-fresco.md), not the walk.

**Temperatures.** The coefficients are exponential in temperature, so the code
refuses to substitute a wrong one. It uses the inventory-averaged kernel
temperature for the kernel, the SiC-layer mean for silver, and the
fuelled-zone matrix mean for graphite hold-up. The silver choice is a
documented departure from upstream, which passes the kernel temperature.

**Calibrated or borrowed:**

- Kernel grain size `1e-5 m`: a TRISO-ATOPS reference value for **UCO** fuel,
  not HTR-10 data. HTR-10 has UO2 kernels. It reaches Cs-137 only.
- Irradiation time 1080 full-power days, derived from the published burnup,
  mass and power.
- Parent chaining is **not** done. The code says this under-states I-131 and
  Xe-133 circulating activity.

**Review route (reading order):**

1. [`TrisoAtopsReleaseChannel::evaluate_with_sic`](../../code-review/#crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/fission_product_release.rs::TrisoAtopsReleaseChannel::evaluate_with_sic).
   Check: which temperature goes to which group; that it calls
   `normal_operation_node` once per curie and once at the real inventory.
   Units: K in, atoms/s and Bq out. Review: none recorded.
2. [`normal_operation_node`](../../code-review/#crates/boon-lay/src/triso_atops_fork/normal_operation/mod.rs::normal_operation_node)
   (the whole per-nuclide chain). Review: none recorded.
3. [`diffusion_coefficient`](../../code-review/#crates/boon-lay/src/triso_atops_fork/diffusion/mod.rs::diffusion_coefficient):
   $D = D_0 \exp(-Q/RT)$ ([Step 3](./03-layers-chemistry.md),
   [Step 6](./06-eulerian-fresco.md)). Units: m$^2$/s. Review: none recorded.
4. [`rb_fail`](../../code-review/#crates/boon-lay/src/triso_atops_fork/release_models/mod.rs::rb_fail),
   which dispatches to
   [`booth_longlived`](../../code-review/#crates/boon-lay/src/triso_atops_fork/release_models/steady_state.rs::booth_longlived),
   [`breakthrough_model`](../../code-review/#crates/boon-lay/src/triso_atops_fork/release_models/steady_state.rs::breakthrough_model)
   and `rb_fail_noble_gases`. Check against Step 6. The recorded Booth
   truncation floor `1.216e-4` over-states some metal releases (gh:#385, per
   the crate's limitations page). Review: none recorded.
5. [`attenuation_factor`](../../code-review/#crates/boon-lay/src/triso_atops_fork/release_models/steady_state.rs::attenuation_factor):
   graphite hold-up. Review: none recorded.
6. [`release_rate`](../../code-review/#crates/boon-lay/src/triso_atops_fork/activities/source_terms.rs::release_rate)
   and
   [`base_activities`](../../code-review/#crates/boon-lay/src/triso_atops_fork/activities/source_terms.rs::base_activities):
   the group table of [Step 11](./11-into-the-coolant.md). Check: gases and
   halogens get $(f_{hm} + f_{inc})\langle R/B\rangle$ only. Review: none
   recorded.

The TRISO-ATOPS port is **verified code to code** against upstream
(recorded: 14 130 cases, worst group $2.44 \times 10^{-9}$ relative, in
[`triso-atops-code-to-code.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/docs/triso-atops-code-to-code.md)).
That shows the port reproduces the Python. It does not show the Python is
right for HTR-10.

## 4. Link C: failure

**Fewest assumptions:** finite-element thermo-mechanics per particle
([Step 8](./08-pressure-stress.md)). **What it does:** the thin-shell
PANAMA-I chain of [Step 9](./09-panama.md), through boon-lay fuel failure.

[`fractions_at`](../../code-review/#crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/fission_product_release.rs::Htr10TrisoAtopsInputs::fractions_at)
builds the four TRISO-ATOPS failure fractions at the live kernel temperature:

| Fraction | Value | Source (as the code states) |
|---|---|---|
| `f_hm` | $5.0 \times 10^{-5}$ | measured free uranium, Tang et al. (2002) spec limit $3 \times 10^{-4}$ |
| `f_sic` | 0 | inside `f_hm` (bounding split) |
| `f_inc` | $5 \times 10^{-4} + \varphi_1 +$ hook | Liu & Cao design value, plus boon-lay fuel failure |
| `f_inc_sic` | $3.6 \times 10^{-5}$ | **NP-MHTGR stand-in**, no HTR-10 value |

**Bounding choice.** The measured free uranium counts defective-SiC
particles and contaminated uranium together, and the split is not published.
The code assigns all of it to `f_hm` (exposed heavy metal), which over-states
gas and iodine release by at most the unknown defective-SiC share. A test
reports the other bound.

**Where the code and earlier pages disagree.**

- **Step 11 quotes `3e-4`** as the HTR-10 design free uranium for `f_hm`,
  from `sembawang`'s KORA example. `htgr_sim_v1` uses the **measured
  $5.0 \times 10^{-5}$** and treats $3 \times 10^{-4}$ as the specification
  limit. Both are in the repo. The plant model uses the measured one. Step 11
  now says so (2026-10-18).
- **Step 9** presents $\varphi_1$ as the accident pressure-vessel mechanism.
  Here it is evaluated at *end of irradiation* with $T_B = T_{kernel} - 75$ K
  and **added on top of** the $5 \times 10^{-4}$ design value. The code's own
  comment says $\varphi_1$ alone "would silently answer a different question"
  and keeps it only because it rises with live temperature. $\varphi_2$
  (decomposition) is not routed anywhere in this chain.
- The $\varphi_1$ strength and fluence inputs are borrowed from the report's
  HTR-Module case (as Step 9 says).

**Review route:**

1. [`end_of_irradiation_failure`](../../code-review/#crates/boon-lay/src/fuel_failure/htr10/mod.rs::end_of_irradiation_failure).
   Check: Eqs (1) to (3) assembled for one particle. Review: none recorded.
2. [`released_gas_fraction`](../../code-review/#crates/boon-lay/src/fuel_failure/booth.rs::released_gas_fraction)
   ($F_d$) and
   [`oxygen_per_fission_uo2`](../../code-review/#crates/boon-lay/src/fuel_failure/oxygen.rs::oxygen_per_fission_uo2)
   (OPF). Review: none recorded. The crate records that Eq (4) has no figure
   or table to check it against.
3. [`internal_gas_pressure`](../../code-review/#crates/boon-lay/src/fuel_failure/pressure.rs::internal_gas_pressure):
   $p = (F_d F_f + \mathrm{OPF})F_b R T / [(V_f/V_k) V_m]$
   ([Step 8](./08-pressure-stress.md)). Units: Pa. Review: none recorded for
   the function; the gas constant `GAS_CONSTANT_J_PER_MOL_K` has a review
   entry.
4. [`induced_stress_with_thinning_factor`](../../code-review/#crates/boon-lay/src/fuel_failure/stress.rs::induced_stress_with_thinning_factor):
   $\sigma_t = r p \mathrm{FKOR}/(2 d_o)$. **Reviewed** (record dated
   2026-10-13, `rung = 3`).
5. [`irradiated_strength`](../../code-review/#crates/boon-lay/src/fuel_failure/strength.rs::irradiated_strength)
   and
   [`irradiated_weibull_modulus`](../../code-review/#crates/boon-lay/src/fuel_failure/strength.rs::irradiated_weibull_modulus):
   Eqs (8a) and (9a). **Reviewed** (same date).
6. [`weibull_failure_fraction`](../../code-review/#crates/boon-lay/src/fuel_failure/weibull.rs::weibull_failure_fraction):
   $\varphi_1 = 1 - \exp[-\ln 2(\sigma_t/\sigma_o)^m]$. Check the $\ln 2$.
   **Reviewed** (same date).

Recorded values for `phi_1` at end of irradiation: $2.8 \times 10^{-15}$ at
700 C, $1.2 \times 10^{-12}$ at 776 C, $1.6 \times 10^{-6}$ at 1000 C
([Step 9](./09-panama.md), from the crate's HTR-10 test docs).

## 5. Link D: coolant pools and removal

**Fewest assumptions:** the CFD species model of
[Step 13](./13-brute-force-cfd.md). **What it does:** the TRISO-ATOPS lumped
pools of [Step 14](./14-triso-atops.md), extended by boon-lay with a leak sink.

[`TrisoAtopsReleaseChannel::update`](../../code-review/#crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/fission_product_release.rs::TrisoAtopsReleaseChannel::update)
steps three pools and a cumulative leak per nuclide, exactly over each
interval.

**Not the closed-form pools.** The ported closed forms are switched off
(`hps_enabled` false, `k_plate` and `k_clean` zero in the ported constants).
The live pools carry HTR-10's published rates instead.

**Review route:**

1. [`htr10_pool_rates`](../../code-review/#crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/fission_product_release.rs::htr10_pool_rates).
   Check: $k_{clean} = \eta \times 0.05 \mathrm{h^{-1}}$;
   $k_{plate} = -\ln(1-d)\dot m / M$; $k_{leak} = 0.01/86400 \mathrm{s^{-1}}$
   ([Step 12](./12-escape-and-removal.md)). Assumes: the efficiencies and
   per-cycle deposition are Liu & Cao's "conservatively set" values. Units:
   s$^{-1}$. It panics for an element they give no value for, by design.
   Review: none recorded.
2. [`live_pools::step`](../../code-review/#crates/boon-lay/src/triso_atops_fork/activities/live_pools.rs::step):
   the exact step of $dC/dt = S - \beta C$, with $\beta = \lambda + k_{plate}
   + k_{clean} + k_{leak}$. Check: atom conservation, and equality with the
   ported closed forms from empty. It is **not a port**. Review: none
   recorded.
3. [`set_pools`](../../code-review/#crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/fission_product_release.rs::NuclideRelease::set_pools):
   converts atoms to activity. Check: activity = $\lambda \times$ atoms.

**Borrowed and bounding.** Pools open at a 20 full-power-year history. The
leak is the *specified limit*, not a measured rate. No position: one rate per
element for the whole circuit.

**Where the code and earlier pages disagree.** None on the rates. Note the
`k_plate` provenance: an earlier hand-quoted value was found to have false
provenance and deleted (gh:#399), as the code records.

## 6. Link E: the accident, and what leaves the circuit

Which scenario runs is a command: `DlofcAtws`, `WaterIngress` or the
default. The first step of a scenario applies an instantaneous change to the
pools.

1. [`depressurisation_lift_off`](../../code-review/#crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/fission_product_release.rs::TrisoAtopsReleaseChannel::depressurisation_lift_off):
   iodine and metals desorb $2.4 \times$ the circulating pool (capped by the
   plate-out pool); dust moves $\text{share} \times 10\%$ of plate-out;
   purification hold-up releases 100 % of noble gas and 10 % of iodine and
   metals. Constants live in
   [`depressurisation.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/depressurisation.rs)
   (Liu & Cao 2002 sections 4.1.1.2 to 4.1.1.4, as cited there). Review: none
   recorded.
2. [`wash_off_plate_out`](../../code-review/#crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/fission_product_release.rs::TrisoAtopsReleaseChannel::wash_off_plate_out):
   moves the steam-generator share of plate-out (Cs 73 %, Sr 80 %, I 100 %)
   into the circulating pool. Review: none recorded.
3. [`vent_circulating`](../../code-review/#crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/fission_product_release.rs::TrisoAtopsReleaseChannel::vent_circulating):
   what the relief valves and the 72-hour ventilation take from the
   circulating pool.
4. [`release_stored_noble_gas`](../../code-review/#crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/fission_product_release.rs::TrisoAtopsReleaseChannel::release_stored_noble_gas)
   with
   [`stored_gas_fraction`](../../code-review/#crates/boon-lay/src/chemistry/kernel_hydrolysis.rs::stored_gas_fraction):
   a burst when steam meets exposed kernels. The fit covers 2.8 to 1051 Pa;
   the code warns when it is extrapolated (gh:#418) and never caps it.

**Where the code and earlier pages disagree (important).**

- [Step 12](./12-escape-and-removal.md) wrote the accident release as
  $f_{vent}(A_{kernel} + A_{graphite}) + A_{circ} + x_{liftoff} A_{plate}$
  with $x_{liftoff} = 0.05$ and called that the HTR-10 stage. That formula,
  [`accident_release_curies`](../../code-review/#crates/boon-lay/src/triso_atops_fork/accident/mod.rs::accident_release_curies),
  is used by **`sembawang`** and by the plant's *static* bounding table. A
  search of `htgr_sim_v1` found **no call** to it, and no `x_liftoff`
  outside that table. The live plant instead moves atoms between pools with
  the $2.4\times$, dust and purification rules above, then vents. These are
  **two different lift-off assumptions** for one process. Step 12 now names
  which path each belongs to (2026-10-18).
- The plant does not call TRISO-ATOPS's accident release at all: it vents the
  live pools. Step 15 describes the TRISO-ATOPS accident path, which the
  digital twin does not take.

**Assumed, bounding.** Isolation of the purification system "assumed to
fail". The 2.4$\times$ desorption is Liu & Cao's, who note it "might be
overestimated by a factor of 3"; the code does not apply that reduction.

## 7. Link F: beyond the vessel, and the building

**Step 16's point:** a building can retain and filter, and a bounding case
credits none of it. **What the code does:**
[`set_uncredited_stack`](../../code-review/#crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/fission_product_release.rs::NuclideRelease::set_uncredited_stack)
sets `stack_release_rate = leak_rate` and `building_activity = 0`. The
building control volume
([`bishan::building::step`](../../code-review/#crates/bishan/src/building.rs::step))
is in the tree, off the default path (gh:#409), reachable only through
`with_building_credit`, which the code marks test-only.

Check: the default `building_credit` is off, and the stack rate equals the
leak. Review: none recorded for either. Even the credited path takes
filter capture and deposition as zero by default, citing Liu & Cao.

This is a **bounding** choice by maintainer decision (2026-09-29), not a
finding about the building.

## 8. Link G: dispersion

**Fewest assumptions:** weather fields and Lagrangian particles
([Step 17](./17-dispersion-reality.md)). **What it does:** the Gaussian
**puff** model, through `changi`, not FLEXPART and not the steady plume
([Step 18](./18-puff-and-plume.md)). The module says so in its first
paragraphs.

**Inputs that are inputs, not data:** a 40 m release height (published), 1.5 m
receptor height, eight compass sectors at 100, 500 and 1000 m, wind speed,
direction and hour set by the operator, and deposition velocities that
`changi` itself labels order-of-magnitude placeholders.

**Assumptions that bias the answer, as the code states them:**

- **No plume rise.** The 9 m/s exit velocity is recorded and unused, so the
  effective height is under-stated and ground concentration over-stated
  (conservative direction).
- **Building wake neglected**, justified by a 40 m stack against a 12 m
  building (ratio 3.3, against a 2.5 rule of thumb the code cites).
- **Default stability is not a worst case.** 3 m/s by day gives the
  ambiguous two-class regime, resolved with `.primary()` to class B. The
  doc comment records that an older comment wrongly called it neutral.
- **Wet deposition and depletion**: none in this channel.
- The puff sigmas were fitted for methane leak detection over about 0.1 to
  10 km, per the module's own note.

**Where the code and earlier pages disagree.** The Map tab can overlay a
**steady plume** from `buangkok` that is a *ground-level* release (height 0),
while the live puff releases at 40 m. The overlay's own header lists the
differences. A reader comparing the two fields compares different source
heights.

**Review route:**

1. [`AtmosphericDispersionChannel::update`](../../code-review/#crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/atmospheric_dispersion.rs::AtmosphericDispersionChannel::update)
   then `evaluate`. Check: the release rate comes from the release channel's
   stack rate, per nuclide. Review: none recorded.
2. [`dilution_factors`](../../code-review/#crates/changi/src/activity/chi_over_q.rs::dilution_factors):
   unit release, binned by travel time, so decay in transit is exact. Check:
   linearity in mass, and $\exp(-\lambda t)$ per age bin. Units: s/m$^3$.
   Review: none recorded.
3. [`gaussian_puff_concentration`](../../code-review/#crates/changi/src/puff/concentration.rs::gaussian_puff_concentration),
   [`pasquill_gifford_sigmas`](../../code-review/#crates/changi/src/puff/dispersion.rs::pasquill_gifford_sigmas)
   and
   [`stability_class`](../../code-review/#crates/changi/src/puff/stability.rs::stability_class).
   Recorded: the puff port matches its R upstream code to code
   (`docs/puff-code-to-code.md`, per the changi coverage table). Review:
   none recorded.
4. [`pathway_rate_usv_per_h`](../../code-review/#crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/dose_rate.rs::pathway_rate_usv_per_h):
   an **indicative** dose rate through `buangkok`, air pathways only, a
   semi-infinite cloud. Coefficients that are missing are reported missing,
   not zero. Review: none recorded.

## 9. What disagrees, collected

| Where | Earlier page says | Code says |
|---|---|---|
| Step 11 | HTR-10 `f_hm` is $3 \times 10^{-4}$ | the plant uses measured $5 \times 10^{-5}$; $3 \times 10^{-4}$ is the limit |
| Step 12 | accident lift-off uses $x_{liftoff} = 0.05$ | the live plant uses $2.4\times$ desorption, dust and purification rules; $x_{liftoff}$ is in `sembawang` and the static table |
| Step 9 | $\varphi_1$ is an accident mechanism | the plant adds it at end of irradiation on top of $5 \times 10^{-4}$ |
| Steps 5 to 7 | walk and Ray's equations | not used in this chain; Booth closed form |
| Map overlay | (not an earlier page) | steady plume at 0 m against the puff at 40 m |

The earlier pages carry these corrections (2026-10-18).

## 10. Recorded numbers, and what they do and do not show

The one comparison against published HTR-10 tables the code records is the
uncalibrated comparison with Liu & Cao Tables 2 and 3, **pre-#469 and "not
re-measured since 62434bb998 pending validation work"**. Recorded
model-over-published ratios
([test doc](../../code-review/#crates/outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/fission_product_release.rs::the_release_is_compared_uncalibrated_with_liu_cao_tables_2_and_3)):

| Nuclide | Source rate | Circulating (20 y) |
|---|---|---|
| Kr-85 | 0.107 | 0.107 |
| Xe-133 | 0.185 | 0.182 |
| I-131 | 0.160 | 0.227 |
| Cs-137 | 12.8 | 13.9 |
| Ag-110m | 2.05 | 2.32 |

The code's reading: gases and iodine come out 5 to 10 times low and caesium
about 13 times high, **with nothing fitted**, and the later $f_{inc}$ change
raised the gas and iodine release about elevenfold, so this table is stale.
No validation of `htgr_sim_v1` has begun, per the crate's `CLAUDE.md`, which
also says not to re-run those checks. Nothing here is a validation.

## 11. The assumptions table for the whole chain

Short codes: **Y** bounding (errs one way), **N** not bounding, **?** the
repo does not say.

| Link | Model | Level |
|---|---|---|
| A inventory | table lookup | below Step 1 |
| B release | Booth closed form | Step 6 |
| C failure | thin-shell Weibull | Step 9 |
| D pools | lumped first-order | Step 14 |
| E accident | pool moves, then vent | Step 12 |
| F building | none credited | Step 16 |
| G dispersion | Gaussian puff | Step 18 |

| Link | Assumption | Bound? | Where |
|---|---|---|---|
| A | one equilibrium core, 5 nuclides, no burnup | ? | `htr10_core_inventory_bq` |
| B | UCO grain size for UO2 kernels | ? | `TRISO_ATOPS_GRAIN_SIZE_M` |
| B | no parent in-growth | N (under-states) | `evaluate_with_sic` |
| B | Booth floor $1.216 \times 10^{-4}$ | Y (over-states some metals) | Booth series, gh:#385 |
| C | all free uranium counted as `f_hm` | Y | `fractions_at` |
| C | `f_inc_sic` from NP-MHTGR | ? | `fractions_at` |
| C | $\varphi_1$ strength from HTR-Module | ? | `htr10` module |
| D | leak 1 %/day, a limit | Y | `htr10_pool_rates` |
| D | purification 99 % and 90 % | Y ("conservatively set") | `htr10_pool_rates` |
| D | plate-out per cycle 20 to 50 % | Y ("conservatively taken") | `htr10_pool_rates` |
| D | one rate per element, no position | ? | `live_pools::step` |
| E | desorption $2.4\times$, not reduced | Y | `depressurisation_lift_off` |
| E | purification isolation fails | Y | `depressurisation_lift_off` |
| E | core gas exchanged with cavity air | Y ("errs high") | `depressurisation.rs` |
| F | no building credit | Y | `set_uncredited_stack` |
| G | no plume rise | Y | `Htr10SiteInputs` |
| G | stability from wind, class B | N | `Meteorology::default` |
| G | placeholder deposition velocities | ? | `survey` |

## Suggested review order for the whole chain

Most load-bearing first.

1. **`fractions_at`.** Every gas and iodine release scales with
   $f_{hm} + f_{inc}$, and it carries the bounding split and two borrowed
   values. The recorded Table 2 and 3 gaps are fuel-side.
2. **`htr10_pool_rates`.** It sets leak, purification and plate-out, the
   three numbers that decide what the pools hold, and all three are
   "conservative" allowances.
3. **`depressurisation_lift_off`.** In a depressurisation, the $2.4\times$
   desorption and dust terms can dominate the release from the circuit, and
   it contradicts Step 12's description.
4. **`set_uncredited_stack`.** One line that turns a bounding decision into
   the number everything downstream uses.
5. **`live_pools::step`.** Not a port, so it has no upstream to check
   against. Atom conservation is its main evidence.
6. **`normal_operation_node` and `rb_fail`.** The core of the release, verified
   code to code, so lower risk of transcription error but not of misapplied
   inputs.
7. **`end_of_irradiation_failure`.** The only function in the chain whose
   neighbours have human review records. Reviewing it completes that group.
8. **`dilution_factors` and the puff.** The last link chi/Q does not depend
   on the source, so source errors cancel there, but the bounding choices
   (no rise, placeholder deposition) live here.
9. **`stored_gas_fraction`.** Extrapolated outside its fit during water
   ingress.

## What we assumed

- The call path above is read from the source as of this draft, not run.
- A review record that does not exist is reported as none recorded.
- The recorded numbers are quoted as recorded, stale where the code says so.

## Next question

Where is the next place to cut cost, or to add physics, without losing the
ability to say which direction the answer errs? That is a maintainer's choice,
and [Step 19](./19-levels-of-fidelity.md) gives the vocabulary for it.
