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

## Limits

- The donor's level at the port picks the phase that leaves (a point draw).
  The path's partial-submergence split sets only the rate.
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
