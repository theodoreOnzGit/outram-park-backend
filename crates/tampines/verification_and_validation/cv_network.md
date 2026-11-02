# Control-volume network (gh:#1095, with gh:#1094's flow paths)

> Research, education and V&V only. AI draft, unreviewed.

Part of the MELCOR-style control-volume epic (gh:#1091). It follows MELCOR
2.2 CVH/FL Reference Manual §6–7, cited and not quoted. Code:
`tampines::cv_network` (`CvNetwork`, `NetworkPath`, `NetworkEnd`,
`NetworkBoundary`, `NetworkControl`). Tests: `src/cv_network/tests.rs`.

## Method

Each step of `Δt` is a Picard iteration on the end-of-step state:

1. Each `FlowPath` advances its two velocities from their start values with
   the end pressures of the latest iterate. Those pressures are under-relaxed
   with an adaptive `ω`. At convergence the momentum equation is backward
   Euler in the pressures.
2. **Equalising limit:** a gas path never moves, in one step, more than the
   moles that bring the two atmospheres to one pressure. This is ported from
   `sembawang`'s `two_sided_equalising` downcomer cap.
3. Every volume steps from its start with its outflows, and with what each
   donor **actually** sent on the previous iterate as its inflow.
4. **Convergence:** iterate until the largest change of a path rate,
   relative to the network's largest rate, is below the tolerance (default
   1e-6, 100 iterations).

With one pass this is loose coupling.

**Exact conservation:** what was sent but not yet delivered is held as
in-transit inventory per path end, `after = before + sent − delivered`, and
the network totals count it. Boundaries are fixed-state reservoirs, and
their account is kept.

## Predictions (written before the first run, 2026-11-01)

1. A closed two-tank gas network conserves to 1e-12, equalises, and
   converges.
2. At `Δt = 1 s` it is stable and ends within 1e-6 of `Δt = 0.01 s`, and an
   early state converges at first order.
3. The manometer period matches `ω² = 2A_p(g + γpA/(ρV))/(LA)` within 3 %.
4. A vent to a boundary closes against the boundary account, and is choked
   at first.
5. Loose coupling also conserves.

## Results (2026-11-01, release)

| # | Measured | Prediction |
|---|---|---|
| 1 | water 8.9e-16, N2 2.2e-16, energy 1.6e-15; p 151 324.94 / 151 319.91 Pa; ≤ 30 iterations | held |
| 2 | stable; +1.06 % from Δt = 0.01 s at 120 s; order at 2 s non-monotone (+107 Pa, −3.2 Pa) | **missed, both halves** |
| 3 | 27.977 s against 27.969 s (+0.03 %); conservation 1e-15 | held |
| 4 | closure ≤ 1.7e-15; choked at step 3; ends at ~~1.0000 bar~~ 99 944 Pa (CORRECTED 2026-11-01, gh:#1097: the gas column to the boundary) | held |
| 5 | as 4, with one pass | held |

**Case 2, analysis of the misses:**
- At 120 s the pressures have equalised, but the gas and the pools are still
  exchanging heat. The state is a transient, not an equilibrium, so it
  legitimately depends on `Δt`. My prediction was wrong.
- The order is unreadable because the equalising limit and choking are
  switches. The bars kept are stability and a 2 % bound, both set after the
  run and labelled so.

## Corrections during the work (each recorded in the code)

1. **Negative deliveries:** an over-delivery from the last step made the
   carried inventory negative, and the receiver refused a negative inflow.
   Delivery now clamps each component at zero and leaves the deficit in
   transit.
2. **The pressure fixed point oscillated.** The residual stalled at 1e-5.
   Fix: adaptive under-relaxation of the end pressures.
3. **The first pass at `Δt = 1 s` took half a tank's gas** at the start-of-step
   choked rate. Fix: the equalising limit from `sembawang`.
4. **The residual was relative to each path's own rate,** which vanishes as
   the ends equalise, so it stalled at 6e-8. Fix: normalise by the network's
   largest rate. The default tolerance became 1e-6; it is a solver control.
5. **Conservation defect:** the in-transit update omitted the "before" term,
   so the network lost about the Picard residual per step. Fixed; every
   closure is now about 1e-15.
6. **Test design:** IF97 steam has no state below 273.16 K. Blowdowns from
   2–3 bar at 300 K cool below it, correctly. The tests run warmer, or use
   the ideal-steam ablation, with the reason stated. Filed in gh:#1098.

## Energy references across a gas flux (gh:#1098 item 2, 2026-11-01)

The gas closure measures non-condensables from 0 K and IF97 steam from the
triple-point liquid. The doubt was whether a volume or the network ever adds
an enthalpy on one reference to an inventory on another.

**Audit.** A volume's atmosphere energy is `Σ n_i u_i^{0 K}(T) +
m_s u_s^{IF97}(T, p_s)`. Each transfer handles the references as follows:

- **Atmosphere outflow** (`ControlVolume::step`, step 3): it removes one
  fraction `f` of every species and `f (U + pV)`, so each species' offset
  leaves with its own mass.
- **Inflow** (`Source`, `inject`, `bubble_through`): the energy is
  `m h_w^{IF97} + Σ n_i (u_i^{0 K} + RT)`. Bubble full interaction prices
  the gases at `u^{0 K}(T_pool)` and the vapour at IF97, and the pool gets
  the IF97 remainder.
- **Flashing, surface exchange, rain-out, the steam sink:** water only,
  IF97 on both sides. The ideal-steam and constant-`c_p` ablations are
  pinned to IF97 values at 100 °C and 0 °C. The surface correction `corr`
  closes the difference against `exchange`'s own `h_fg`.
- **The network's `source_from`:** it subtracts the gases' `u^{0 K} + RT`
  (the receiver's model, at the donor's gas temperature) from a delivery,
  and gives the rest to the water. A boundary donor prices its gases the
  same way.

No path mixes references. A secondary point, not a reference defect: if the
donor and receiver use different non-condensable `c_v` models (one
ablated), `source_from` moves that sensible-energy difference onto the
water. Energy is still conserved exactly. This does not arise with the
default models.

**Pin.** `p V = p_s V + Σ p_i V`, so `(U + pV)/m` of the mixture is exactly
the mass-weighted species enthalpies. A slice of a donor's atmosphere split
by `source_from` must therefore hand the water IF97's `h_tp_2(T, p_s)`.

**Result** (`gas_flux_keeps_each_species_on_its_own_energy_reference`,
2026-11-01, release): steam with N2, H2, O2, CO2 and Ar at 360 K gives
`h_w` = 2658314.478990 J/kg, equal to IF97 to 2.2e-16 (bar: 1e-9). Pricing
the gases from 298.15 K instead would have moved `h_w` by 2.54e6 J/kg.

## Limits

- ~~The donor's level at the port picks the phase that leaves (a point draw).
  The path's partial-submergence split sets only the rate.~~ **CORRECTED
  2026-11-02 (gh:#1117):** the donor draws both phases in the path's split;
  the point draw is the ablation (section below).
- The step is Picard, not Newton or JFNK, and convergence is linear.
- A 30-hour run has not been timed; that comes with gh:#1097.

## External sources, sinks, heats and valves (gh:#1097, 2026-11-01)

Added for the Mark I containment network in `sembawang`
(`bwr_mark1::containment_network`):

- `CvNetwork::set_source(volume, port, Source)`: a held external source
  (relief valves, a vessel leak) on a port no path uses; it enters through
  the volume's own port model (bubbles below the level).
- `set_atmosphere_heat` and `set_atmosphere_steam_sink`, through the new
  `ControlVolumeInputs::atmosphere_heat` and `atmosphere_steam_sink`: the
  caller's heat structures (convection, and condensation leaving with the
  steam's enthalpy at the atmosphere's temperature and partial pressure).
  Both explicit.
- `FlowPath::set_open_fraction` and `CvNetwork::set_path_open_fraction`:
  a valve that multiplies the area (and the choked area); at 0 the path is
  closed and its velocities reset.
- `boundary_inflow` now counts all of these and the pool heat (the pool
  heat was left out before; no test exercised it).

**Test 6** (`external_sources_sinks_heats_and_a_valve_close`, prediction
before the run: closure < 1e-12 with the valve shut and half open):
**worst closure 2.6e-15** over 300 steps of 1 s. Held.

~~A 30-hour run has not been timed; that comes with gh:#1097.~~ Timed in
`crates/sembawang/verification_and_validation/bwr_mark1/containment_network.md`.

### Vacuum breakers held for the step (gh:#1097, 2026-11-01)

**Defect found on the Mark I containment:** a vacuum breaker decided on
each Picard iterate equalised the two volumes on the first pass, was shut
on the second (the end pressures now equal), and the step converged with
no flow, so the breaker never opened at any pressure difference (the torus
reached 4.3 kPa above the drywell with 3.4 kPa breakers and nothing moved).
**Fix:** the network decides each breaker once per step from the
start-of-step pressures (`FlowPath::hold_open`) and its equalising limit
stops at the opening difference instead of at equal pressures.

**Test 7** (`a_vacuum_breaker_holds_its_opening_difference`; prediction
before the run: the difference held between 3.4 and 3.8 kPa): **MISSED
high, 3905–4064 Pa**, because the equalising limit, taken from the
start-of-step state, does not see a source's rise within the step (about
600 Pa a step in that test). Closure 9.8e-15. The bound kept (3.4–4.1 kPa)
was set after the run and is labelled so. A breaker held for a whole step is
first order in `Δt`; a lagged switch, stated.
**SUPERSEDED 2026-11-02 (gh:#1116):** the decisions now come from the
latest Picard iterate, and test 7 holds 3399.999704–3400.000890 Pa. See
the section below.

### The equalising limit uses the junction pressures (gh:#1097, 2026-11-01)

**Defect found on the Mark I containment:** the limit took the two
atmospheres' pressures, so it let the downcomers equalise the drywell with the
torus gas and ignored the 11.8 kPa of submergence over their exits. The next
iterate then drove the gas back, and the step did not converge (at 4.39 h
of the Unit 1 sequence). **Fix:** the limit stops where the path's
start-of-step driving difference vanishes: the junction pressures (with any
liquid head) less the donor gas column `ρ_g g Δz`, minus a breaker's opening
difference. Tests 1–7 still hold. The vent test's vessel now ends at
99 944 Pa instead of 1.0000 bar: the 4 m gas column to its boundary,
47 Pa, is no longer ignored.

## Switch decisions from the latest Picard iterate (gh:#1116, 2026-11-02)

**The change.**
- **Equalising limit.** It is re-taken on every Picard iterate from the
  latest iterate. That is, it is a linearised end-of-step condition, no
  longer a start-of-step one. The path may move at most the gas that brings
  the end-of-step driving difference to its target (the opening difference
  for a vacuum breaker, zero otherwise):
  `n_eq = (d0 − Δp_set) / (p_a/n_a + p_b/n_b)`, with `d0 = drive_latest +
  n_prev (p_a/n_a + p_b/n_b)`. Here `drive_latest` is the latest iterate's
  junction-pressure difference less the gas column, and `n_prev` the moles
  this path moved on that iterate. On the first iterate `n_prev = 0` and the
  latest state is the start state, so it is the old limit. At convergence,
  whenever the limit binds, the end-of-step driving difference equals
  `Δp_set`. (This is the design as written before the run. Two
  corrections, found by the tests, are below under "As built".)
- **Vacuum breaker.**
  - It starts each step as before, open if the start-of-step difference
    exceeds its set point.
  - On every iterate it also opens if `d0` (the latest difference with its
    own flow added back) exceeds the set point.
  - **Hysteresis:** once open within a step it stays open for that step. So
    the decision changes at most once per step and cannot chatter, unlike
    gh:#1097's per-iterate decision, which shut itself.
  - Staying open is safe, because the limit gives zero flow whenever `d0` is
    below the set point.
  - A breaker whose donor is a boundary is limited by its receiver alone; a
    boundary does not move. Before, it had no limit.
- **Ablation:** `CvNetwork::without_iterate_switch_decisions` keeps the
  start-of-step decisions, bit for bit.

**Predictions (written before the code was changed):**

- **S1.** The ablation reproduces the old network bit for bit (trajectory
  hashes of tests 1 and 7 taken on the code before the change).
- **S2, test 7 (breaker under a rising pressure).** After opening, every
  step ends with the difference in **3400.0–3400.5 Pa** (old: 3905–4064 Pa),
  inside the original 3.4–3.8 kPa prediction. The 0.05 kg/s source no
  longer overshoots, because the limit sees the end of the step. The
  breaker opens one step earlier (step 4, old step 5), because the opening
  can now be found within the step in which the difference crosses 3.4 kPa.
  The step it opens on ends at 3400.0–3400.5 Pa (old: 4247 Pa). Closure
  stays < 1e-12.
- **S3, a smooth case for the time order.** Two tanks 1 % apart (1.01 and
  1.00 bar), a 0.0005 m² path, no choking and the limit never binding
  (asserted from the report). The momentum is backward Euler and the volumes
  are first order, so the error at 2 s against `Δt` = 0.0125 s halves with
  `Δt`: ratio 1.8–2.2 between `Δt` = 0.2 and 0.1 s, and between 0.4 and
  0.2 s.
- **S4, test 2 re-measured.** At 120 s, `Δt` = 1 s against 0.01 s: still
  within the 2 % bar (old +1.06 %). At 2 s, the old +107 Pa (0.2 s) and
  −3.2 Pa (0.1 s): low confidence. My guess is that the 0.2 s error falls,
  because the limit no longer acts on a stale state, but the order stays
  unreadable, because choking is still a switch.
- **S5.** Tests 1, 3, 4, 5 and 6 still hold: closure < 1e-12. Test 4 still
  ends at 99 944 Pa (an equilibrium, within 1 Pa).

**As built (two corrections to the design above, each found by a test
and fixed before the results were recorded):**

1. **The receiver lags by one iterate.** A receiver in the latest iterate
   has taken only what the donor sent on the iterate *before*. So `n_prev`
   is added back at the donor's slope, and the moles of the iterate before
   at the receiver's. The design above took both at `n_prev`, and test 2 at
   `Δt` = 1 s converged on a false fixed point with no flow. The tanks
   stayed 100 kPa apart (+32 % at 120 s). The rate residual could not see
   it, and the in-transit inventory kept the closure exact.
2. **The slope is adiabatic, `γ p/n`,** with `γ = 1 + nR/C_v` from each
   volume's gas model, not the isothermal `p/n`. Gas that leaves carries its
   enthalpy, so the rest cools. With `p/n` the iterates oscillated by a
   factor of about `1 − γ ≈ −0.4` each, and `sembawang`'s relief case went
   from 2 to 36 Picard iterations. The building's breathing flow, about
   6e-5 kg/s through its openings, was being re-limited at sub-mPa drives.
   With `γ p/n` it went to 11 iterations. The converged limit does not
   depend on the slope; only the convergence rate does. The first iterate's
   limit therefore differs from the ablation's by the slope (loose coupling,
   one pass, moved; test 4 below). A path fed by a boundary is now limited
   by its receiver alone, for open paths as well as breakers, so inflow from
   a boundary cannot run past the end-of-step balance either.

**Results (2026-11-02, release, Rust 1.99.1).**

| Prediction | Old (now the ablation) → new | Verdict |
|---|---|---|
| S1 ablation = old code | test 1 (600 × 0.1 s) and test 7 (400 × 1 s) trajectory hashes identical to b4c99e210 (`iterate_switches_are_default_and_the_ablation_is_the_old_network`) | held |
| S2 test 7, after opening | 3904.5–4063.6 → **3399.999704–3400.000890 Pa** | held, except its lower edge missed by 3.0e-4 Pa (Picard tolerance) |
| S2 test 7, opening | step 5 at 4246.8 Pa → step 4 at 3399.9999928 Pa | held (one step earlier; within 7e-6 Pa of the set point) |
| S2 test 7, closure | 9.8e-15 → 9.3e-15 | held |
| S3 smooth case, order | errors at `Δt` 0.4 / 0.2 / 0.1 / 0.05 s: −75.63 / −19.12 / −3.695 / −0.961 Pa against 0.0125 s; ratios 3.96, 5.17, 3.85 | **missed** (predicted 1.8–2.2): second-order dominated, see below |
| S4 test 2, 120 s | +1.06 % → +0.138 % | held (2 % bar unchanged) |
| S4 test 2, 2 s | +107 → +58.1 Pa (0.2 s); −3.2 → −17.7 Pa (0.1 s); ratio −3.28 | held as guessed: smaller at 0.2 s, still unreadable |
| S5 test 1 | 151 324.94 / 151 319.91 → 151 310.28 / 151 310.28 Pa; iterations ≤ 30 → ≤ 34 | held |
| S5 test 4, implicit | 99 944.1 → 99 942.1 Pa | **missed** "within 1 Pa" by 1 Pa (below) |
| S5 test 4, loose coupling | 99 944.1 → 99 945.5 Pa | **missed** by 1.4 Pa: its one pass takes the adiabatic slope (As built, 2) |
| S5 tests 3, 5, 6, L8, N1 | unchanged; closures ≤ 2.9e-15 | held |

**Why the numbers moved.** The limit is now an end-of-step condition, so
it sees what happens within the step: a source's rise (test 7) and the gas's
warming from the pools (tests 1, 2 and 4). The start-of-step limit had held
each of those one step behind. In test 4 the vessel ends 2 Pa lower for
that reason, and the 1 s run of test 2 lands 7.7 times closer to the
0.01 s run.

**S3, the time order on a smooth case** (`time_order_on_a_smooth_case`; no
step choked or limited, asserted). The error at these steps is dominated by
an `h²` term, not the first-order one predicted. That is `flow_path`'s
identity (`transient_second_order_is_a_tanh_addition_identity`): a path
accelerating from rest under a nearly constant difference is second order
in this scheme, and the first-order term comes only from the difference
changing within a step. A two-term fit on the two finest steps gives
`c₁ ≈ −1.5 Pa/s` and `c₂ ≈ −354 Pa/s²`, so the first-order term would
dominate only below about 4 ms. The fit is stated but not asserted. Asserted
after the run, and labelled so: one sign, and each halving divides the
error by at least 1.8. On this case the ablation gives identical bits,
because no switch acts.

**Downstream: `sembawang`'s containment network**
(`cargo test --release -p sembawang --test containment_network_vs_containment`,
6/6 pass on both; the old decisions measured by temporarily making the
ablation the default, then restored).

| Quantity | Old decisions → new | Comment |
|---|---|---|
| A1/A2 relief: Δp_ww, Δp_dw, worst ΔT_pool | +341.1 / +6.4 Pa, 2.87e-3 K: unchanged | |
| A5 Picard iterations (asserted < 30) | 2 → 11 | the building's breathing flow, ~6e-5 kg/s, which the start-of-step limit froze at zero |
| B, 6 h of Unit 1: worst \|Δp_dw\|, \|Δp_ww\|, \|ΔT_pool\| against the old lumped `Containment` | 57.99 % → **16.51 %**, 12.00 % → **4.62 %**, 0.60 → **0.26 K** | at 4.96 h the drywell spike: 553 → 408 kPa (old model 350 kPa); the downcomers now open within the step (+1576 kg/s) |
| B, peak p_dw | 571.6 → 503.6 kPa | |
| B, half-hourly p_dw at 5.0 / 6.0 h | +2.81 % → +3.41 %, +3.98 % → +4.52 % | slightly worse between spikes |
| B, Picard iterations, cost | ≤ 35 → ≤ 105; **6.46 → 24.9 ms a step** (a 30 h run 698 s → 2688 s) | **3.9 times the cost** |
| C, D, E | fingerprints moved: p_dw +0 to −35 Pa; Cs and Xe splits by up to 1e-3 relative; burned H2 323.43 → 320.80 kg, the building's peak 402.3 → 396.3 kPa; burn starts 1678/1800 s → 1677/1799 s; conservation unchanged (~2e-14) | |
| the whole test file | 177 s → 650 s | |

The cost is the open item. The decisions now resolve flows that the lagged
limit held at zero or let overshoot, and that takes Picard iterations.
Every `sembawang` long test that runs the network gets slower and its pins
move. They were not re-run here (see the hand-off).

**What to quote:** test 7's 3399.999704–3400.000890 Pa, and the smooth
case's ratios. Test 2's 2 s errors are not an order measurement.

## Cutting the iteration count (gh:#1116 follow-up, 2026-11-02): profiled; no candidate accepted

The maintainer asked for the iteration count to be cut before acceptance.
Measured on `sembawang`'s case B (6 h of Unit 1,
`containment_network_vs_containment::the_first_hours_of_unit_1_against_the_old_containment`).
Hardware: a 4-vCPU Intel Xeon @ 2.10 GHz cloud container, 15 GB, release
build with `-j 2`. Its timing varies by about 12 % between runs of the same
code (21.9 and 24.3 ms a step for the base below).

**Profile** (temporary instrumentation, since removed):
- 21 606 network steps, 203 888 Picard iterations, a mean of 9.44.
- 75 % of the iterations are in the 11 317 steps that take more than 5.
- In those steps the largest residual belongs to the **downcomer**
  (86 920 iterates) and the **vacuum breaker** (42 886). The building's
  openings almost never hold it (3).
- **63 % of those iterations run at the under-relaxation floor, `ω` =
  0.05.**

A per-iterate trace shows the mechanism:
1. The downcomer's momentum flow is evaluated at the latest end pressures,
   which already carry its own previous-iterate flow.
2. A large flow on one iterate therefore lowers its own drive on the next,
   and the flow collapses, sometimes to zero, then jumps back. That is a
   Picard iteration on a stiff self-coupling.
3. Each collapse makes the residual grow (to about 1e11 when every rate
   drops to zero on one iterate), so `ω` halves. Once it reaches 0.05, the
   remaining corrections move 5 % a step.

The old start-of-step limit hid all this. It held the downcomer at a
constant, always-binding cap on every iterate, so the momentum flow never
decided the rate. That is also why its numbers were 3–4 times further from
the lumped model.

**Candidates.** The prediction was written before each run. Mean
iterations are over case B, and acceptance means within the Picard
tolerance of the current #1116 numbers.

| Candidate | Predicted mean | Measured mean / max / ms a step | Case B (worst \|Δp_dw\|, \|Δp_ww\|, \|ΔT_pool\|) | Verdict |
|---|---|---|---|---|
| base (#1116 as committed, d40b3eb7a) | — | 9.44 / 105 / 24.3 | 16.51 %, 4.62 %, 0.26 K | — |
| (a) don't re-take a limit that did not bind | identical (the flow is `min(raw, cap)`) | 9.44 / 105 / 23.9 | identical | held as predicted; no gain |
| (b) Aitken Δ² on a limited path's moles, every third iterate | 8–9 | **11.0** / 110 / 27.5 | identical at the printed digits | rejected (slower) |
| (c) warm start: the first iterate's cap from the previous step's limited moles | about 9 | **12.9** / 105 / 36.6 (+121 substeps) | identical at the printed digits | rejected (slower) |
| (d) solve each gas path with its own linearised end response (`n = G(n)`, bracketed secant) | ≤ 3.5 | 8.0 / **208** / 23.2 | **14.35 %, 4.87 %, 0.28 K** | rejected: moved past the tolerance; N1 fails to converge |
| (d′) as (d), donor's slope only | 5–7 | — | — | rejected: N1 and test 2 fail to converge (tampines proxies) |
| (e) `ω` doubles when the residual halves | 5–6 | — | — | rejected: four tampines network tests fail to converge (the oscillation `ω` was added for) |
| (f) residual scale: the larger of this and the previous iterate's rates | 7–8 | — | — | rejected: case B stops at 0.002 h. A flow alternating with zero reads residual 1 every iterate, never "growing", so `ω` never damps it |
| (e) + (f) | 4–5 | — | — | rejected: as (e) |

**Interpretation.** The extra iterations are the genuine implicit solve of
a stiff path's momentum. The old code never performed that solve, because
its lagged cap always bound. A fixed-point (Picard) iteration with
pressure under-relaxation converges on it only linearly, and none of the
listed accelerations changes that without either slowing down or losing
convergence. The step already states this limit ("Picard, not Newton or
JFNK"). The fix that matches the mechanism is a Newton step on the path
flows that includes each volume's real (condensing, bubbling) response.
That is a solver change of its own, not a tweak here, and is not attempted.
The candidates' temporary code is not kept.

## A partially submerged junction moves both phases (gh:#1117, 2026-11-02)

**Before.** At a path's donor end the path already split its rate by the
submerged fraction of the opening: liquid through the part below the
level, gas through the rest (`flow_path`, "MELCOR's partially submerged
junction"). But the network handed the donor volume one total outflow, and
the volume drew a single phase by its level at the opening's centre (the
point draw, `PortDraw`). A junction straddling the surface therefore moved
pure liquid, at the liquid-plus-gas rate, while the level was above the
centre, then pure gas.

**Upstream.** MELCOR's FL package gives a flow path separate pool and
atmosphere velocities. The pool and atmosphere fractions of the junction
opening come from the donor's level against the opening's elevation and
height, and each phase moves through its own fraction (MELCOR 2.2 FL
reference manual, the junction void fraction / flow-path opening section).
**Section number not re-checked:** the manual is private and not in this
workspace, and no file here cites the section. The rule as ported is the
one `flow_path` already applied to the rates; this change makes the donor
volume honour it.

**The change.**
- `PortInput::liquid_fraction: Option<f64>`: the pool-liquid mass fraction
  of a port's outflow. `None` keeps the point draw (the default for a
  caller that sets only `outflow`).
- The network passes the path's own split, `m_liq / (m_liq + m_gas)` for
  the direction's donor.
- Default on. `CvNetwork::without_two_phase_junction_draw()` restores the
  point draw, bit for bit (pinned on hashes taken before the change).

**Predictions (written before the code was changed):**
- **J1, the drain (`side_port_drain`).** A: 1.5 m of water under 2 bar;
  port at 1.0 m, opening 0.2 m; drains into an empty B at 1 bar.
  - (a) New: the liquid fraction of A's outflow is exactly 1 while A's
    level is at or above the opening's top (1.1 m), exactly 0 at or below
    its bottom (0.9 m), strictly between in the band, and non-increasing as
    the level falls.
  - (b) Old (the ablation): it jumps from 1 to 0 in one step as the level
    crosses the centre, 1.0 m.
  - (c) The two runs are bit-identical until A's level first falls below
    1.1 m.
  - (d) A's lowest level: old within 1 cm below 1.0 m (at the centre the
    port starts drawing gas, and liquid stops); new within 1 cm of 0.9 m
    (liquid keeps leaving through the covered part until the opening is
    clear). A's gas is still about 0.6 bar above B's then, so the drive
    remains.
  - (e) Water, N2 and energy close to < 1e-12 in both.
- **J2, the ablation.** Bit-identical to the pre-change code on the drain
  and on L8's network (hashes).
- **J3, L8 at the new default.** Its port at 0.0 m (opening 0.05 m)
  straddles as A drains, so L8 moves. Dry-out still within 10 s (old
  5.46 s, change < 0.1 s); closures < 1e-12; the end state (A 121.1 kg,
  B 72.6 kg) moves by a few kg.
- **J4, `sembawang` case B (not run here; the coordinator's re-bless holds
  sembawang).** Bit-identical unless a donor port straddles a surface
  with flow through it. The torus level stays 3.2–3.4 m with the downcomer
  exits well below, and I expect the drywell floor water not to reach the
  downcomer entry band (2 m above the floor) in 6 h. Low confidence on the
  drywell floor.

**Results (2026-11-02, release, Rust 1.99.1).** Tests:
`cv_network::tests::a_straddling_junction_moves_both_phases`,
`a_volume_drains_dry_inside_a_network` (L8),
`tests/control_volume.rs::a_port_outflow_splits_by_its_liquid_fraction`.

| Prediction | Measured | Verdict |
|---|---|---|
| J1(a), fraction exactly 1 at or above 1.1 m (by the end-of-step level) | 1.000000000000000 | held |
| J1(a), strictly between in the band | in (0, 1], but **244 band steps are pure liquid**: with the pressures equal (from about 25 s) the gas flows B → A on those steps | **missed** (backflow not foreseen) |
| J1(a), non-increasing in the band | it rose on 90 steps, by at most 4.2e-3, as the pressures equalised and the gas flow fluctuated | **missed** |
| J1(b), old jumps 1 → 0 at 1.0 m | largest one-step fall 1.000002 | held |
| J1(c), identical until the band | identical for 784 steps; the level first ends below 1.1 m at step 784 | held |
| J1(d), old lowest level within 1 cm below 1.0 m | 0.99951 m | held |
| J1(d), new lowest level within 1 cm of 0.9 m | **1.00818 m at 40 s** (1.0013 m at 115 s, still falling) | **missed** (below) |
| J1(e), closure | 3.3e-15 (new), 4.4e-15 (old) | held |
| J2, ablation bit for bit (drain and L8 hashes, 215434afa) | identical | held |
| J3, L8 dry-out | 5.46 → 5.5 s | held |
| J3, L8 end state "moves by a few kg" | A 121.1 → **0 kg**, B 72.6 → **194.0 kg**, the gases 31 kPa → 1893 Pa apart | **missed**; L8's own original prediction (B 190–195 kg, B's head 1911 ± 300 Pa) now **holds** |
| control volume split (0.25 of 2 kg) | water out 0.5 + 0.043059 kg of steam, N2 out 1.456941 kg, both exact to 1e-12; closure 7.5e-13 kg water; f = 1 and f = 0 equal the point draw bit for bit; f = 1.5 refused | held |
| J4, `sembawang` case B | not run (the coordinator's re-bless holds sembawang) | open |

**Why J1(d) missed.** My estimate counted only the liquid moving (A's gas
expanding, B's compressed) and left A 0.6 bar above B. But from 1.1 m
down, A's gas also passes through the uncovered part of the opening, and
the two pressures meet at about 1.40 bar by 25 s. After that the liquid
drains only under its own small head, through the shrinking covered
part. The point draw (old) instead stalls just below the centre: once
the level is below it, the port draws only gas.

**Why L8 moved (J3).** As A drains, L8's 0.05 m opening straddles A's
surface. The point draw pushed pure liquid at the path's liquid-plus-gas
rate until A's level passed the centre, then pure gas. That left the two
gas springs unbalanced, and the water swung back (the old record's
refill). Passing both phases lets A's gas through as the opening
uncovers, so the pressures settle on B's head. The refill is now asserted
on the ablation, which still produces it.

**What to quote:** L8's end state at the default (B 194.0 kg, 1893 Pa).
The old refill is quoted only as the ablation's.
