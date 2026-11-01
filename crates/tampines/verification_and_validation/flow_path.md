# Two-velocity flow paths (gh:#1094)

> Research, education and V&V only. AI draft, unreviewed.

Part of the MELCOR-style control-volume epic (gh:#1091). It follows MELCOR
2.2 CVH/FL Reference Manual §6, cited and not quoted, with no MELCOR source
used. Code: `tampines::flow_path` (`FlowPath`, `FlowPathGeometry`,
`FlowPathKind`, `PathEnd`, `FlowTransfer`, `critical_moles`). Tests:
`src/flow_path/tests.rs`.

## Method

- **Two velocities, pool and atmosphere,** each with its own momentum
  equation:
  - `ρ_k L dv_k/dt = ΔP_k − ½Kρ_k|v_k|v_k`;
  - `ΔP_k` is the end pressures at the junction elevations, less the gravity
    head `ρ_k g (z_to − z_from)`.
- **Time step:** semi-implicit in the velocity, with the loss linearised
  about the old velocity and the pressures held. The implicit network with
  coupled pressures is gh:#1095.
- **Phase selection:** the donor end's level against the opening sets the
  split. The submerged fraction passes liquid, and the rest passes gas.
- **Choking, on by default** (`without_choking` is the ablation):
  - gas: MELCOR's critical flow (§6.3, Eq 6-13), `critical_moles`, moved
    **verbatim** from `sembawang`, which now forwards to it;
  - liquid: IF97 HEM break flux from the donor's stagnation state.
- **Kinds:** open, check valve, and vacuum breaker (forward only, opening
  above a pressure difference).

## Predictions (written before the first run, 2026-11-01)

1. With `K = 0`, `v = ΔP t/(ρL)` exactly.
2. At steady state, `ΔP = ½Kρv²` to 1e-10.
3. The error of `v(τ)` for `v∞ tanh(t/τ)` halves with Δt (first order).
4. A 1 m rise with equal end pressures decelerates the flow at `g/L`.
5. Phase selection by level, including a half-covered opening.
6. Gas choking equals `critical_moles` exactly; liquid flow chokes.
7. A check valve never flows back; a breaker flows only above its set
   difference.

## Results (2026-11-01, release)

| # | Measured | Prediction |
|---|---|---|
| 1 | v(1 s) = 1.000000000000000 against 1.0 | held |
| 2 | v 3.162277660168 against √(2ΔP/(Kρ)), loss to 1e-10 | held |
| 3 | errors 1.771e-4 and 4.427e-5 at τ/50 and τ/100: **ratio 4.000, second order** | **missed** (good direction, not explained) |
| 4 | deceleration −g/L to 1e-12 | held |
| 5 | liquid only, gas only, and half/half, each to 1e-12 | held |
| 6 | gas 186.650459 kg/s = `critical_moles`; liquid choked at 2967.8 kg/s through 0.1 m² (7 MPa, 1.2 MJ/kg) | held |
| 7 | as predicted | held |

**Defect found and fixed during the work (case 6).** The liquid limit used
the stagnation enthalpy `h + ½v²` with the path's pre-limit velocity, which
reached 4.5e7 J/kg. The donor is a volume at rest, so its stagnation
enthalpy is now `h`.

**Bit identity with sembawang:** `sembawang`'s full suite (lib and tests)
passes with `critical_moles` and the MELCOR gas fits forwarding to
`tampines`. Its fixtures pin every output.

## Limits

- The pressures are held over a step. The two-velocity paths are not yet
  driven by control volumes (gh:#1092 and gh:#1095).
- There is no wall friction and no counter-current flow limitation.
- Interphase drag within the path is not modelled.
- The case-3 order is unexplained (filed).
