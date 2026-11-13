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
- ~~The step is Picard, not Newton or JFNK, and convergence is linear.~~
  **CORRECTED 2026-11-02 (gh:#1119):** the default is a Newton step on the
  path flows (the section at the end); Picard is the ablation
  `without_newton_path_flows` and the fallback.
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

> **CAVEAT, 2026-11-02 (gh:#1119):** these case-B figures were measured with the Picard solve. That solve converges *falsely* on steps where the equalising limit binds: on the Mark I stand-in, 488 of 899 steps have a Picard downcomer flow that disagrees with the momentum equation at its own end state by up to 0.415 of the largest rate, against 2.1e-5 for Newton (§ gh:#1119 below). Read the 16.51 / 4.62 % / 0.26 K as numbers from that solver, not as the network's converged answer. Case B has not been re-run on the Newton default; that waits for the network model check (gh:#1110, maintainer).

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
convergence. The step already states this limit (~~"Picard, not Newton or
JFNK"~~ **CORRECTED 2026-11-02, gh:#1119:** Newton is now the default, see
the section at the end). The fix that matches the mechanism is a Newton
step on the path flows that includes each volume's real (condensing,
bubbling) response. That is a solver change of its own, not a tweak here,
and ~~is not attempted~~ was done in gh:#1119.
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

## A Newton step on the path flows (gh:#1119, 2026-11-02)

**Why.** Since gh:#1116 the network solves the stiff downcomer's momentum
implicitly, and the under-relaxed Picard iteration converges on it only
linearly (profile and the rejected candidates (a)–(f) are in the section
"Cutting the iteration count" above; none is repeated here).

**Searched first, for reuse.** `petir::newton_krylov` (`jfnk`,
`jfnk_preconditioned`: JFNK with GMRES on a matrix-free product) needs
`F(x)` as a pure function of `x`. The network's iterate is not one: a
receiver is stepped with what its donor sent on the *previous* iterate
(the exact-conservation design), so the iterate carries a one-iterate lag
that a pure residual does not see. `tampines::subchannel::solver::newton`
(forward-difference Jacobian by columns, after
`petir::multiroots::fdjacobian`) is private to its residual. The Newton
updates in `multiphase_1d::two_fluid` and `control_volume` are scalar or
specific to their equations. `tuas_boussinesq_solver`'s
`collection_series_and_parallel_functions` solve a pressure-flow root per
branch, not a network Jacobian. `changi`: nothing. `outram-foam-basic-lib`'s
`linear_operator` is the GMRES that `petir` already ported. **Reused:**
~~`petir::linalg::{lu_decomp, lu_solve, Matrix}` for the dense solve, and~~
**CORRECTED 2026-11-02:** the dense solve served only the first design and
is gone (revision 2 below), so nothing from `petir` remains; still reused,
the finite-difference column idea of `subchannel::solver::jacobian_triplets`.
The volume-pressure response is what `control_volume`'s own docs name:
"an implicit scheme can linearise around `pressure_at_elevation` by
stepping a clone".

**Method (the design, written before the code).**
- Unknowns: the end pressures each path is stepped with, `u` (two per
  path), as the Picard iteration already uses them. The fixed point is
  `u = P(u)`: the end pressures of the volumes after a step with the path
  flows `x(u)`.
- Jacobian `∂P/∂u = D C`:
  - `C`, per path, `∂x/∂u` by finite differences of `FlowPath::step` on a
    clone (two extra path steps a path per iterate; cheap). A path the
    equalising limit cuts gets a zero row: its flow is then set by the
    limit, not by `u`.
  - `D`, per path, the end pressures' response to its flow, by finite
    differences of **the volumes themselves**: the donor stepped again from
    the start with its outflow raised by `δ`, and the receiver with its
    inflow raised by what the donor then actually sent more. So condensing
    at a submerged exit, bubbling, flashing and the level's head are in
    `D`, as the issue asks. `D` is taken once a step, on the first iterate
    a path's `C` is non-zero (a chord Newton).
- The lag: the latest iterate's receivers carry the donor's flow of the
  iterate before. The residual is corrected for it with the receiver half
  of `D`: `P̃ = P + D_rec (x_{k−1} − x_{k−2})`.
- The update, in path-flow space (`N` unknowns, not `2N`):
  `(I − C D) Δx = C r`, `r = P̃ − u_{k−1}`, `u_k = u_{k−1} + λ (r + D Δx)`.
  `λ = 1`, halved when the residual grows (floor 1/16).
- Everything else is the Picard step unchanged: the convergence test (the
  rate residual against the tolerance), the limit, the breakers, the
  in-transit inventory.
- **Fallback:** if Newton has not converged in 30 iterates, or a volume
  errors while `D` is taken, or a solve fails, the step is re-run with
  Picard from the start state (nothing is committed before). Counted in
  `NetworkStepReport::newton_fell_back`.
- **Not used** (Picard path, bit for bit): with
  `without_iterate_switch_decisions` (the b4c99e210 pin), with one
  iteration (loose coupling), and with the ablation
  `CvNetwork::without_newton_path_flows`.

**Instrument for "within Picard tolerance".** Picard stops when the rate
change between iterates is below 1e-6 of the largest rate. With `ω` at
0.05 a small change per iterate does not mean a small distance from the
fixed point, so the two solvers can legitimately differ by more than
1e-6 of a rate. The instrument, chosen before the run: Newton at 1e-6 is
accepted when its distance from Picard at 1e-6 is no larger than Picard's
own distance from Picard at 1e-8 (the spread the tolerance itself allows),
on the quantities compared (pressures, pool temperature).

**The case-B stand-in.** `sembawang`'s case B cannot be run in this change
(another session holds `sembawang`). A stand-in in this crate,
`cv_network::tests::mark_i_proxy`: Unit 1's drywell, torus, downcomers and
vacuum breaker (sembawang's numbers, copied), 900 s at 1 s: a 20 kg/s steam
leak for 300 s, then 8 MW of drywell cooling for 300 s (the breaker opens),
then a 2 kg/s leak. Same halving driver as sembawang's `advance`.
**Picard on it, measured before the change** (4-vCPU Xeon @ 2.10 GHz,
release, `-j 1`): mean 24.56 iterations a step, max 274, 7 extra halving
sub-steps, **71.9 ms a step**, 27 steps with breaker flow, closure 4.0e-15.
It is stiffer than case B (9.44 mean).

**Predictions (written before the first Newton run).**
- **W1, ablation.** `without_newton_path_flows` is bit for bit the Picard
  default: hashes taken on the code before the change, test 1's two tanks
  (600 × 0.1 s) `0x6499f07e74895e85`, test 7's breaker (400 × 1 s)
  `0x35ec1958148a4fca`, the stand-in (900 × 1 s) `0x7bbab5d4aaf8eb46`.
  The b4c99e210 pin (`iterate_switches_are_default_and_the_ablation_is_the_old_network`)
  is unchanged.
- **W2, stand-in, iterations.** Mean 3–5 a step (from 24.56), max ≤ 30,
  no halving, no fallback.
- **W3, stand-in, cost.** 25–40 ms a step (from 71.9): a Newton iterate
  costs a Picard iterate plus two path steps a path, and `D` costs about
  two volume steps per flowing path once a step.
- **W4, stand-in, results.** Within the instrument above. Low confidence
  on the margin: I expect the worst pressure difference Newton − Picard to
  be of the same order as Picard(1e-6) − Picard(1e-8), under 100 Pa.
- **W5, the tampines network tests.** Every assertion holds unchanged
  (tests 1–7, L8, N1, S3, J1); test 1's iterations fall from ≤ 34 to
  ≤ 10. Closures stay < 1e-12 (the in-transit inventory does not depend
  on the solver).

**First run of the design above: W2 and W4 MISSED; the design was
revised three times by diagnosis.** (Measured on the stand-in, recorded
before the corrections were run; the stand-in itself had a defect, item 0.)
0. **Stand-in defect, mine:** the drywell floor was at 0 m, below the
   torus pool surface (2.33 m), so the pool ran back into the drywell
   through the downcomers. Fixed to sembawang's layout (the floor at the
   torus top). The Picard numbers above are from the defective stand-in;
   on the corrected one Picard takes **mean 18.62 iterations, max 153, one
   halving, 39.9–42.3 ms a step**, 280 steps with breaker flow, hash
   `0x15da1328b8babe3d`.
1. **The `C`-based update did nothing where it mattered.** On the
   corrected stand-in the downcomer is cut by the equalising limit on
   most iterates, so its `C` row is zero, and Newton took Picard's
   iterations exactly (mean 18.48). The limit's own convergence is slow
   because its slopes (`γ p/n`, adiabatic gas) are not the volumes'
   (steam condensing in the pool), and the receiver's half lags. **Fix:**
   in the Newton mode the limit, and the breaker's add-back, use the
   path's measured response `D` for both slopes (`limit_response`). The
   converged limit is unchanged (the driving difference meets its
   target); only the rate of getting there changes.
2. **A linearised path flow cannot cross a dead band.** At the
   downcomer's clearing (step 7) the flow ping-ponged between 0 (the drive
   below the 11.8 kPa submergence, where `∂x/∂u = 0`) and 17 kg/s, for
   30 iterates. **Fix (the method as built):** the volumes stay
   linearised by `D`; the paths are not linearised at all. Each iterate
   solves `x = Φ(x)` with the paths' own steps at the linearised pressures
   `u(x) = P̃ + D (x − x_{k−1})`, and the limit. `Φᵢ` falls as `xᵢ` rises,
   so `h = x − Φ(x)` rises with slope ≥ 1 and the root is bracketed at
   once in `[x₀ − h(x₀), x₀]`. One regula falsi (Illinois) per path,
   Gauss–Seidel over the paths (`monotone_root`). `C`, the dense solve and
   the step length are gone; path steps cost little next to volume steps.
3. **`D` taken at zero inflow.** Receivers lag, so on the first flowing
   iterate the pool's response to bubbling was taken at zero inflow, and
   the next iterates oscillated with period 2 (13 iterates). **Fix:**
   re-take `D` when its outflow or inflow base has moved by more than 10 %
   (`PathResponse::holds_at`).
4. The regula falsi's tolerance had been scaled by the previous iterate's
   largest rate (1e-7 kg/s on a clearing step), unreachable for a root of
   17 kg/s. Now relative to the root.

**As built, on the stand-in (2026-11-02, release, `-j 1`, same
hardware): mean 4.82 iterations a step, max 8, no halving, no fallback,
18.8 ms a step against Picard's 39.9 ms on the same run (2.1 times
faster).** Picard cannot converge step 7 (the downcomers clearing) at
`Δt` = 1 s even with 5000 iterations and halves it. Newton converges it in
7. So the two trajectories differ from step 7 on by one halved step: the
worst |Δp_dw| over the run is 66.3 Pa, the whole of it from that step.
**The trajectory comparison therefore cannot be the instrument** (a
time-step difference, not a solver difference). Changed after seeing
this, and stated: the instrument is **per step**. From each state on the
Newton trajectory, the same step is taken by Picard at 1e-6 and at 1e-8
(5000 iterations, no halving); steps where Picard does not converge at
full `Δt` are counted and excluded.

**Prediction W4′ (written before the per-step run):** on every step
Picard converges, Newton(1e-6) is no further from Picard(1e-8) than
Picard(1e-6) is, in both pressures and the pool temperature (Newton
converges superlinearly, so its last change bounds its error; Picard's
error at `ω` = 0.05 is about its last change divided by 0.05). The
worst Picard(1e-6) − Picard(1e-8) per step: under 1 Pa.

**W4′ MISSED (per-step run, 2026-11-02).** Worst per step: |Newton −
Picard(1e-8)| 3.75 Pa (p_dw), 3.12 Pa (p_ww), 3.8e-6 K; |Picard(1e-6) −
Picard(1e-8)| 0.025 Pa, 0.018 Pa, 5.7e-8 K. Newton is the further on 493
of 899 steps (one step skipped: Picard cannot take step 7 at full `Δt`).
All of the excess is in the leak phase (0–300 s); in the breaker and
late phases the worst |Newton − Picard(1e-8)| is 4.4e-4 and 8.9e-3 Pa.

**Diagnosis: two converged answers to one step, and only Newton's solves
the momentum equation.** Step 8, from the same start state (Newton's
history): Newton gives 23.3445 kg/s through the downcomers, unlimited,
p_dw 118 074.40 Pa, unchanged at tolerances down to 1e-11. Picard
(1e-6 and 1e-8) gives 23.432 kg/s, limited, p_dw 118 070.66 Pa.
Re-evaluating each end state (the path stepped from its start velocity
at the end pressures; the end-of-step driving difference):
- Newton: momentum 23.344493 kg/s at its own end pressures (its flow is
  23.344473; the difference is the in-transit lag), end drive +6.87 Pa,
  so the limit rightly does not bind;
- Picard: momentum **13.69 kg/s** at its own end pressures against the
  23.43 kg/s it moved, end drive 0.0001 Pa.

Picard's answer is a **false convergence**. Once the limit binds, a path's
rate no longer depends on the relaxed end pressures, so the rate residual
is met while those pressures (relaxed at `ω` = 0.05) are still far from
the end state. Tightening the tolerance cannot see it, which is why
Picard(1e-6) and Picard(1e-8) agree to 0.025 Pa. Newton has the same
blind spot in principle. It met it on its first design (a path held at
zero flow twice, item 2), and the exact-path formulation removes it,
because the paths are evaluated at pressures consistent with the
volumes' response to the flows. The criterion is not moved. The result
is that "within Picard tolerance" cannot hold where Picard is falsely
converged, and the disagreement is the finding.

**Prediction M (written before the run of the momentum-consistency
instrument):** per step, for the downcomers, the inconsistency `|x −
x_m| / max rate`, with `x_m` the momentum flow at the end state (and
`x ≤ x_m` accepted when the end drive is within 1e-3 Pa of its
target, the limit binding):
- Newton: under 1e-4 on every step;
- Picard: about 0.4 on the limited leak steps, and under 1e-4 where the
  limit does not bind.

**Result M (2026-11-02): held.** The worst inconsistency over the 899
steps Picard converges: Newton **2.1e-5**; Picard(1e-6) **0.415**, on
488 steps above 1e-4 (all in the limited leak phase). These are the same
steps as W4′'s 493.

**Results (2026-11-02, release, Rust 1.99.1, `-j 1`; a 4-vCPU Intel Xeon
@ 2.10 GHz cloud container, 15 GB; wall time varies by about 12 %
between runs of the same code).** Tests:
`cv_network::tests::newton_path_flows_on_a_mark_i_stand_in` (W1–W4′, M)
and the network suite (W5).

| Prediction | Measured | Verdict |
|---|---|---|
| W1, ablation = Picard default, bit for bit | test 1 `0x6499f07e74895e85`, test 7 `0x35ec1958148a4fca`, stand-in (first geometry) `0x7bbab5d4aaf8eb46`: identical to the code before the change; the b4c99e210 pin unchanged | held |
| W2, stand-in, mean 3–5 iterations, max ≤ 30, no halving, no fallback | **mean 4.82, max 8**, no halving, no fallback (Picard: 18.62, max 153, one halving) | held, on the design as built (the first design: 18.48, missed) |
| W3, stand-in, 25–40 ms a step | **18.2–20.5 ms** against Picard's 39.9–45.9 ms (four runs), **2.0–2.4 times faster** | missed, cheaper than predicted |
| W4, trajectory within the tolerance spread | 66 Pa, all from one step Picard must halve | instrument withdrawn (above) |
| W4′, per step, Newton no further from Picard(1e-8) than Picard(1e-6) | 3.75 Pa against 0.025 Pa; Newton the further on 493 of 899 steps | **missed**: Picard falsely converged (M) |
| M, momentum consistency, Newton < 1e-4 | 2.1e-5 (Picard 0.415) | held |
| W5, test 1 iterations ≤ 34 → ≤ 10 | ≤ 8; p 151 310.29 / 151 310.28 Pa (was 151 310.28 / 151 310.28) | held |
| W5, test 7, after opening, within 3400.0–3400.5 Pa | **3399.999498**–3400.000205 Pa (Picard: 3399.999704–3400.000890); opens at step 3 at 3399.99999967 Pa (Picard: step 4); closure 1.0e-14 | held at the test's 1e-3 Pa floor; the opening one step earlier (below) |
| W5, tests 2–6, L8, N1, S3, J1 | all pass unchanged. Test 4: 99 942.1 Pa (same); L8: B 194.017 kg, 1893.18 Pa (same at the quoted digits); S3 errors identical at the printed digits, Newton − Picard at 0.1 s −1.4e-4 Pa; J1 every quoted number unchanged | held |
| closures | ≤ 1.0e-14 everywhere (stand-in 5.3e-15) | held |

**Two pins re-based, not loosened.** gh:#1117's point-draw pin and S3's
"the switch ablation gives the same bits" were taken when Picard was the
only solver. Their intent, the ablation reproducing the old code, now
needs the Newton step ablated as well. Both are composed with
`without_newton_path_flows`, and the hashes are unchanged.

**Test 7 opens one step earlier (step 3).** It ends that step at
3399.99999967 Pa, the set point. The likely reason, **not isolated:**
the breaker's add-back now uses the measured response, and at step 3 the
difference with its own flow added back crosses 3400 Pa where the
adiabatic slopes left it under.

**Not run: `sembawang`'s case B.** This change could not build
`sembawang` (another session holds it). Case B's numbers will move, at
least where its downcomers are limited, if the false convergence is
there too (likely: its profile has the downcomers at the limit and `ω`
at its floor). They have to be re-measured with the new default, and
compared by the momentum-consistency instrument, not by distance from
the Picard numbers. A lead, **not re-checked:** candidate (d) of the
profile ("moved results past the tolerance") also solved each path
self-consistently, and may have been moving off the same false
convergence.

**What to quote:** the stand-in's iterations and cost (4.82 against
18.62 a step; about 2.1 times faster), M's consistency numbers, and
test 7's band. Not W4′'s 3.75 Pa as "Newton's error": it is Picard's.

**Suite run:** `cargo test --release -j 1 -p tampines --lib --tests`
(the full suite, long tests on): 261 passed, 0 failed, 5 ignored, in 15
binaries. The two tests added after that run started
(`the_newton_steps_scalar_solve_brackets_its_root`,
`a_failed_newton_step_falls_back_and_commits_nothing`) were run on their
own: 2 passed.

## Unit 1's quiet containment falls back from Newton on every step (gh:#1124, 2026-11-03): diagnosed, not fixed

**Measured before (gh:#1124).** `sembawang`'s three units on their
historical sequences, first 600 s at 1 s, the network containment default
(Newton on): Unit 1 costs about 25 ms a containment step with 5 iterates
on every step; Units 2 and 3 cost about 3 ms with 2. Unit 1's torus is
quiescent (no downcomer or breaker flow); Units 2 and 3 discharge SRV
steam into the pool.

**Hypothesis (gh:#1124, untested when written).** Some path carries a
tiny non-zero flow (the building's breathing flow, about 6e-5 kg/s, was
named), whose rate moves by more than the 10 % chord criterion
(`PathResponse::holds_at`) on every iterate, so `path_response` is re-taken
on every iterate.

**Predictions (copied from gh:#1124 before the run).**
1. Logging which path is re-linearised shows the same small path every
   iterate on Unit 1.
2. An absolute floor on the chord criterion, relative to the step's largest
   rate (`scale`), brings Unit 1 to about 2 iterates and about 3 ms, with
   results unchanged within the Picard/Newton tolerance.

**Method.** A temporary `sembawang` example (deleted, not committed):
`Plant::for_unit(u, Mode::Historical, Mechanisms::default())`, the core from
`p.parts()`, `ContainmentNetwork::with_config(&u.params(),
u.network_config(), NetworkMechanisms::default())` stepped with
`p.begin().containment`, then `p.finish`; containment wall time per step,
`NetworkResult::iterations`, and `p_dw`, `p_ww`, `T_pool` and the downcomer
flow at full precision each step. Temporary `eprintln!` in
`CvNetwork::step_solve` (reverted): per iterate, each path's rate, whether
its response was re-taken, its residual term, and the fallback. Release,
4-vCPU Intel Xeon @ 2.10 GHz.

**Results.**

- **Prediction 1 missed.** No path's response is re-taken after the first
  iterate of a step (`stale = false` on every later iterate). The chord
  criterion is not involved.
- **What happens instead: Newton never converges on Unit 1 and falls back
  to Picard on all 600 steps** (Units 2 and 3: 0 of 600). Each step costs
  30 failed Newton iterates and then Picard's 5; the "5 iterates" of
  gh:#1124 are Picard's, since `NetworkStepReport::iterations` counts the
  fallback's alone.
- **The path is the building opening (path 4, building to environment),
  and its flow is round-off, not breathing.** The building's pressure at
  the opening alternates between 101 300 Pa and 101 299.999 999 999 985 Pa,
  one ulp (1.455e-11 Pa) below the boundary's. One iterate drives
  -1.455e-11 kg/s (capped by the equalising limit to -3.13e-12 kg/s); the
  volume stepped with that inflow returns to exactly the boundary pressure,
  so the next iterate has no flow, and so on. The 3.13e-12 kg it moves in a
  step is below the f64 resolution of the building's 44 000 kg of air
  (`f64::EPSILON` × 44 000 kg ≈ 9.7e-12 kg), so the state cannot settle it.
- **Why the convergence test cannot pass.** The residual is each path's
  rate change over the network's largest rate, floored at an absolute
  1e-12 kg/s (`step_solve`, "Residual on the path rates"). With no other
  path flowing, the largest rate is the round-off flow itself, so the
  residual alternates between 1.0 and 3.13 (the floor) on every iterate.
  Picard ends after 5, on two consecutive zero-flow iterates, after its
  `ω` has halved at iterate 4 (the residual grew); why the relaxed
  pressure then stays on the boundary's was not traced. On Units 2 and 3 the SRV-driven
  flows make the scale large and the same toggle is 1e-12 of it.
- **Prediction 2 not tested as written** (it addresses the chord
  criterion, which is not the cause). **A different candidate was measured
  as a diagnostic and reverted:** a path's rate change counts as converged
  when its mass over the step is below the f64 resolution of the smaller
  inventory it joins, `|Δṁ| Δt ≤ ε_f64 · min(M_from, M_to)` (water plus
  every gas, a boundary infinite). With it:

  | unit | ms/step before | ms/step with the candidate | iterates (mean, max) before | with the candidate | Newton fallbacks before / with |
  |---|---|---|---|---|---|
  | 1 | 24.7 | 1.57 | 5.000, 5 (Picard's) | 2.000, 2 | 600 / 0 |
  | 2 | 2.66 | 3.0 | 2.003, 4 | 2.000, 2 | 0 / 0 |
  | 3 | 2.72 | 2.56 | 2.003, 4 | 2.000, 2 | 0 / 0 |

  Units 2 and 3's times are within the run-to-run spread seen here (2.6
  to 3.4 ms). **`p_dw`, `p_ww`, `T_pool` and the downcomer flow are bit
  for bit identical on all 600 steps of all three units** (largest
  difference 0), Unit 1's included, where the step moved from Picard to
  Newton.

~~**Not done, on purpose.** The cause found is not the hypothesised one, so
no fix is committed (gh:#1124 asks for the evidence first).~~ **SUPERSEDED
2026-11-03:** the maintainer chose "Newton only, report fallbacks"; the
fix and its results are the next section. The text below records what was
open before that choice. Open for the
maintainer: whether the round-off floor above is the right instrument
(it is derived from `f64::EPSILON` and the inventories, not from the
timing), whether it belongs in the Picard path too (it would change
Picard's iterate counts, though not, here, its results), and whether
`NetworkStepReport` should also count the failed Newton iterates so a
fallback on every step is visible to a caller.

### The fix: the round-off floor on Newton's test, and fallbacks counted (gh:#1124, 2026-11-03)

**Maintainer's choice:** "Newton only, report fallbacks". The floor above,
`|Δṁ| Δt ≤ ε_f64 · min(M_from, M_to)` (`roundoff_floor`), applies to
Newton's convergence test only; Picard, the fallback and
`without_newton_path_flows` keep the old test. `NetworkStepReport` gains
`newton_iterations` (Newton's iterates, the failed ones of a fallback
included; 0 when Newton is not used), and `sembawang`'s `NetworkResult`
carries `newton_iterations`, `newton_fallbacks` and `newton_fell_back`,
summed over its sub-steps.

**Predictions (written before the runs).**
- F1: the Picard pins are unchanged: `without_newton_path_flows` on test 1
  and test 7, the stand-in's Picard trajectory, and the gh:#1116 and
  gh:#1117 ablations.
- F2: the Newton default on `tampines`' own cases (test 1, test 7, the
  side-port drain, the Mark I stand-in) is unchanged bit for bit: each has
  real flows, so the floor (about 1e-11 kg/s or less) never decides.
- F3: `sembawang`, first 600 s, as in the diagnosis: Unit 1 to 2 iterates
  and about 1.5 ms, no fallback; Units 2 and 3 unchanged at about 3 ms;
  every `NetworkResult` field bit for bit, as the diagnostic run found for
  the four it compared.

**Results (2026-11-03, release, 4-vCPU Intel Xeon @ 2.10 GHz).**
- **F1 held.** The existing hash pins pass unchanged
  (`newton_path_flows_on_a_mark_i_stand_in`,
  `iterate_switches_are_default_and_the_ablation_is_the_old_network`, the
  gh:#1117 ablation test), in the full suite below.
- **F2 held.** Hashes of the Newton default taken before the change and
  after (`network_hash`, `drive_proxy`): test 1 `0x6fb5abade0a7d839`,
  test 7 `0x77085d148ed4668a`, the side-port drain `0xc91d30324fe9f42c`,
  the stand-in `0xed2ccaafdbf2217e` (mean 4.82 iterations, no fallback),
  identical. Printed by a temporary test, not pinned (the maintainer: no
  new pins before gh:#1110).
- **F3: the timings and counts held; "every field" missed by one ulp on
  Units 2 and 3.** The same probe as the diagnosis, every `NetworkResult`
  field compared at full precision (66 numbers a step, the fission-product
  report's included, the iteration counters and residual left out):

  | unit | ms/step before → after | iterates (mean, max) before → after | Newton fallbacks before → after | fields differing |
  |---|---|---|---|---|
  | 1 | 26.1–26.4 → 1.42–1.54 | 5.000, 5 (Picard's) → 2.000, 2 | 600 → 0 | none, on 600 of 600 steps |
  | 2 | 3.1–3.2 → 3.0–3.3 | 2.003, 4 → 2.000, 2 | 0 → 0 | `p_rb_pa` only, by 1.455e-11 Pa (one ulp, 1.4e-16 relative) |
  | 3 | 2.7–2.9 → 2.7–3.0 | 2.003, 4 → 2.000, 2 | 0 → 0 | `p_rb_pa` only, by one ulp |

  Two runs each; the before runs were deterministic (byte-identical
  output). Before, `NetworkResult` had no fallback count; the 600 is the
  diagnosis's count from the same code. **`p_dw`, `p_ww`, `T_pool` and the
  downcomer flow are bit for bit identical on all three units** (largest
  difference 0). On Units 2 and 3 the building's pressure now settles one
  ulp from where Picard's iterate left it, because Newton accepts the
  toggle on an earlier iterate; nothing else moves, so the difference does
  not grow over 600 steps. It is round-off, its cause is understood, and it
  is far below the 1e-8 relative allowance for code-to-code differences.
- **The toggle in `tampines`' own terms.** A grid, chosen before the run,
  of one quiet building-sized volume (38 000 m³ on 650 m², no pool, 1 m²
  opening 1 m below the top to a 300 K dry-air boundary): the boundary
  −2 … +2 ulps from the volume's pressure at the opening, the volume's gas
  N2 or dry air, at 300 K or 303.15 K, 5 steps each. Without the floor, 5
  of the 20 cells fell back (on 1 to 5 of the 5 steps; 30 failed iterates
  each). With it, none did, at 2 iterates a step except on two steps of
  two cells (3, 4 and 7). The cell pinned as the regression test,
  `a_round_off_toggle_converges_by_newton`, is dry air at 303.15 K with
  the boundary 2 ulps below (the closest to `sembawang`'s building): 5 of 5
  fell back before, 0 after.
- **Limit, recorded.** In the first construction tried (the tests'
  `vessel` helper: 650 m² × 60 m of dry N2 at 300 K, a 1 m² path with a
  2 m inertial length, the boundary one ulp above), the toggle moved
  1.5e-11 kg in a step against a floor of 9.9e-12 kg, and Newton still
  falls back on 5 of 5 steps with the fix. The floor catches a toggle
  below one f64 resolution of the inventory, not one of a few. Measured
  by a temporary test, not pinned.
- **The pass-through.** `containment_network_vs_containment::
  the_newton_steps_count_and_fallbacks_reach_the_network_result` steps
  Unit 1 for 10 s on the plant's own snapshots and asserts no fallback and
  `newton_iterations == iterations` at 2 iterates (it fell back on every
  step before).

