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
| 3 | errors 1.771e-4 and 4.427e-5 at τ/50 and τ/100: **ratio 4.000, second order** | **missed** (good direction; ~~not explained~~ explained 2026-11-01, case 3b) |
| 3b | see below: ratios 4.001, 4.000, 4.000 at τ/2, τ, 2τ; identity to 3.8e-15; held-ΔP ramp ratio 2.000 | **held** (all four predictions) |
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

## Case 3b: why case 3 is second order (gh:#1098 item 1, 2026-11-01)

The doubt was that the ratio of 4.000 at `t = τ` might be a cancellation of
the first-order error coefficient at that one time. It is not.

**Derivation (written before the run).** Let `u = v/v∞`, `s = t/τ` and
`h = Δt/τ`. The momentum equation becomes `du/ds = 1 − u²`. The step
`(ρL/Δt + ½Kρ|v^n|) v^{n+1} = ρL/Δt v^n + ΔP` becomes, for `u ≥ 0`:

```text
u_{n+1} = (u_n + h) / (1 + h u_n)
```

This is the addition formula `tanh(a + b) = (tanh a + tanh b)/(1 + tanh a
tanh b)` with `h` in place of `tanh b`. Starting from `u_0 = 0`, the step is
therefore exact on a stretched clock:

```text
u_n = tanh(n · artanh h)
artanh(h)/h = 1 + h²/3 + h⁴/5 + …
u_n − tanh s = s sech²(s) h²/3 + O(h⁴)
```

The leading error is proportional to `h²` at every `s > 0`, and its
coefficient vanishes nowhere there.

More generally, take `u' = a − bu²` with constant `a` and `b`. Expanding
`(u + ha)/(1 + hbu)` gives `u + h(a − bu²) + h²(b²u³ − abu) + O(h³)`, which
matches `hu' + h²u''/2`. So the linearised quadratic loss is second order
for any `ΔP` held constant. The step falls to first order when `ΔP` changes
within the step and is held at its start value, which is what a network
does.

**Predictions:**

- (a) The path reproduces `v∞ tanh(n artanh h)` to 1e-12 relative.
- (b) The error matches `v∞ s sech²(s) h²/3` to 2 % at `h = 1/100`.
- (c) The error ratio lies in [3.9, 4.1] at `s` = ½, 1 and 2.
- (d) With `ΔP = ΔP₀ t/τ` held at each step's start, the self-convergence
  ratio at `s = 1` lies in [1.8, 2.2].

**Results (2026-11-01, release,
`transient_second_order_is_a_tanh_addition_identity`):**

| s | error, h = 1/50 | error, h = 1/100 | ratio | leading term, h = 1/50 |
|---|---|---|---|---|
| ½ | 1.658e-4 | 4.145e-5 | 4.001 | 1.658e-4 |
| 1 | 1.771e-4 | 4.427e-5 | 4.000 | 1.771e-4 |
| 2 | 5.958e-5 | 1.489e-5 | 4.000 | 5.959e-5 |

- The identity holds to 3.8e-15 relative, worst case.
- The ramp with held `ΔP` gives `v(τ)` = 1.415502889, 1.428029571 and
  1.434293582 at `h` = 1/50, 1/100 and 1/200, a self-convergence ratio of
  2.000: first order.

All four predictions held. The scheme was not changed.

## Limits

- The pressures are held over a step. The two-velocity paths are not yet
  driven by control volumes (gh:#1092 and gh:#1095).
- There is no wall friction and no counter-current flow limitation.
- Interphase drag within the path is not modelled.
- ~~The case-3 order is unexplained (filed).~~ **CORRECTED 2026-11-01:**
  explained in case 3b (gh:#1098 item 1). Within a network, where the
  pressures change within a step, the step is first order.
