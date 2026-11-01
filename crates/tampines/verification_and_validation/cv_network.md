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
| 4 | closure ≤ 1.7e-15; choked at step 3; ends at 1.0000 bar | held |
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
