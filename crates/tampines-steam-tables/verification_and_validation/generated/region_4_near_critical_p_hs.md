# Near-critical Region 4 (h,s) flash: p(h,s)

> **Generated file — do not hand-edit.** Regenerate with:
>
> ```bash
> cargo test --release -p tampines-steam-tables --lib \
>   backward_eqn_chebyshev_experimental::tests::region_4
> ```
>
> Generated 2026-09-02 21:45 (UTC).

## Status

These are **experimental, non-IAPWS correlations** fitted in-house (see GitHub issue #34). IAPWS-IF97 publishes no backward equations for some of the cases covered here, so where a reference is quoted it is either an IAPWS equation already implemented in this crate or this crate's own forward equations — never a published backward-equation reference value.

Per `RESPONSIBLE_USE.md` this is AI-assisted draft material: the numbers below are measurements, **not a validation sign-off**. No human has reviewed them.

## Methodology

The reference here is IAPWS-traceable: this crate's IAPWS Region 4 saturation-pressure equation `sat_pressure_4(T_sat)`. The correlation must recover that pressure from an `(h, s)` pair alone.

Test states are generated **forwards**, which matters. Sweeping the `(h,s)` fit bounding box directly does not work: the box is a bounding box, not the valid domain — real near-critical two-phase states occupy a curved wedge inside it, and most `(h,s)` pairs drawn from the box are not two-phase states at all. So instead: pick a saturation temperature in the fitted band, take the reference pressure from `sat_pressure_4`, pick an enthalpy between the fitted saturated-liquid and saturated-vapour branches, take the matching entropy from this crate's IAPWS `(p,h)` flash `s_ph_eqm`, and confirm the state is genuinely two-phase with `x_ph_flash`.

Grid: 40 saturation temperatures across 623.15-647.04 K against 20 qualities spanning 0.05-0.95, giving 795 usable two-phase states. Qualities are kept off the exact endpoints, where the flash is ill-conditioned.

## Results

Relative error in the recovered saturation pressure:

| Statistic | median | 90th pct | 99th pct | maximum |
|---|---|---|---|---|
| relative error in p | 1.340e-6 | 7.955e-6 | 5.899e-5 | 1.141e-4 |

## Interpretation

On genuine two-phase states the correlation reproduces the IAPWS saturation pressure closely, and this is a real comparison against IAPWS rather than a self-consistency check.

**The sharp edge is the domain, not the accuracy.** `p(h,s)` is fitted in `log(p)` with coefficients of order 1e4 that cancel to give a `log(p)` of order 3 on the two-phase wedge. Off the wedge that cancellation does not happen and the exponential runs away — sampling the bounding box uniformly has produced pressures as large as 1e71 MPa. An absurd result from this function almost certainly means the input `(h,s)` pair is not a near-critical two-phase state, not that the fit is broken. Callers must establish that before calling; the function does not validate its input.

Not covered here: the companion quality correlation `x(h,s)`, which has no accuracy measurement of its own. Its lever rule is inherently ill-conditioned approaching the critical point, where `h_g - h_f` tends to zero.

