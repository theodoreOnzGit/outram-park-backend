# p(rho,h) across the single-phase regions

> **Generated file — do not hand-edit.** Regenerate with:
>
> ```bash
> cargo test --release -p tampines-steam-tables --lib \
>   backward_eqn_chebyshev_experimental::tests::p_rho_h
> ```
>
> Generated 2026-08-31 04:44 (UTC).

## Status

These are **experimental, non-IAPWS correlations** fitted in-house (see GitHub issue #34). IAPWS-IF97 publishes no backward equations for some of the cases covered here, so where a reference is quoted it is either an IAPWS equation already implemented in this crate or this crate's own forward equations — never a published backward-equation reference value.

Per `RESPONSIBLE_USE.md` this is AI-assisted draft material: the numbers below are measurements, **not a validation sign-off**. No human has reviewed them.

## Methodology

IAPWS-IF97 publishes no `(rho,h)` backward equations, so the reference is this crate's own forward equations. A 60 x 60 `(p, T)` grid is swept — pressure log-spaced 1e-3 to 50 MPa, temperature linear 280 to 2200 K. At each node the crate's dispatcher `region_fwd_eqn_single_phase` labels the region, `v_tp_eqm_single_phase` and `h_tp_eqm_single_phase` give specific volume and enthalpy, and density is the reciprocal of specific volume. The correlation must then recover the originating pressure from `(rho, h)` alone.

The region is **supplied** to the correlation here rather than classified, so this measures the fitted surfaces in isolation, with the statistical classifier out of the loop. Region 4 is covered separately (see the two-phase dome report), because two-phase states are not reachable through a single-phase `(T,p)` flash.

## Results

Relative error in the recovered pressure, by region:

| Region | n | median | 90th pct | 99th pct | maximum |
|---|---|---|---|---|---|
| Region1 | 299 | 5.619e-4 | 7.583e-2 | 1.838e0 | 3.185e0 |
| Region2 | 1185 | 9.788e-6 | 2.258e-5 | 6.240e-5 | 1.549e-4 |
| Region3 | 16 | 1.082e-4 | 3.247e-4 | 3.407e-4 | 3.688e-4 |
| Region5 | 2100 | 2.146e-5 | 5.249e-5 | 1.833e-4 | 1.306e-3 |

Worst single state: Region1 at T = 280.00 K, rho = 999.8620 kg/m3, h = 28.80 kJ/kg — reference 0.00100 MPa, recovered 0.00419 MPa.

## Interpretation

Regions 2, 3 and 5 recover pressure tightly. Region 1 does not, and its error is dominated by the low-pressure end rather than by liquid as such — see the Region 1 conditioning report for the breakdown by pressure decade, which is the number that should drive any decision about using this in liquid.

Region 3's sample is small (the `(p,T)` sweep grazes it), so its statistics are the least well supported here.

