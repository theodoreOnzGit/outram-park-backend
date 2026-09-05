# p(rho,h) in Region 4, across the two-phase dome

> **Generated file — do not hand-edit.** Regenerate with:
>
> ```bash
> cargo test --release -p tampines-steam-tables --lib \
>   backward_eqn_chebyshev_experimental::tests::p_rho_h
> ```
>
> Generated 2026-09-02 21:45 (UTC).

## Status

These are **experimental, non-IAPWS correlations** fitted in-house (see GitHub issue #34). IAPWS-IF97 publishes no backward equations for some of the cases covered here, so where a reference is quoted it is either an IAPWS equation already implemented in this crate or this crate's own forward equations — never a published backward-equation reference value.

Per `RESPONSIBLE_USE.md` this is AI-assisted draft material: the numbers below are measurements, **not a validation sign-off**. No human has reviewed them.

## Methodology

Region 4 cannot be reached through the single-phase `(T,p)` flash used for the other regions, because a `(T,p)` pair on the saturation line is underdetermined without a quality. States are therefore generated with the crate's two-phase `(T,p,x)` flashes.

For each saturation temperature the reference pressure is the IAPWS `sat_pressure_4(T_sat)`; `v_tp_eqm_two_phase` and `h_tp_eqm_two_phase` give the mixture specific volume and enthalpy at a given quality, and density is the reciprocal of specific volume. The correlation must recover the saturation pressure from `(rho, h)` alone.

Grid: 50 saturation temperatures over 280-645 K against 21 qualities from 0 to 1 inclusive, so both saturation boundaries are exercised — the bubble point at x = 0 and the dew point at x = 1. The upper temperature stops short of the critical point (647.096 K), where the two phases merge and the flash degenerates.

## Results

Relative error in the recovered saturation pressure:

| Subset | n | median | 90th pct | 99th pct | maximum |
|---|---|---|---|---|---|
| all qualities | 1050 | 2.195e-4 | 1.340e-3 | 4.726e-3 | 1.773e-1 |
| bubble point (x < 0.05) | 50 | 1.145e-4 | 4.838e-2 | 1.409e-1 | 1.773e-1 |
| interior (0.05 <= x < 0.95) | 900 | 2.041e-4 | 1.174e-3 | 2.967e-3 | 4.158e-3 |
| dew point (x >= 0.95) | 100 | 5.196e-4 | 3.138e-3 | 4.689e-3 | 4.726e-3 |

Worst single state: T_sat = 280.00 K, rho = 999.86203 kg/m3, h = 28.80 kJ/kg — reference 0.000992 MPa, recovered 0.000816 MPa.

## Interpretation

The Region 4 surfaces reproduce IAPWS saturation pressure well across the bulk of the dome, and the sweep covers both saturation boundaries, so this region is no longer an untested part of the correlation set.

The worst states sit at the cold, low-pressure end of the dome, near the bubble point — the same corner that dominates the Region 1 error, and for the same reason: at low pressure a liquid-like density carries almost no information about pressure, so the inversion is ill-conditioned there. See the Region 1 conditioning report.

