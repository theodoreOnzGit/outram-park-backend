# Region 5 backward correlations T(p,h) and T(p,s)

> **Generated file — do not hand-edit.** Regenerate with:
>
> ```bash
> cargo test --release -p tampines-steam-tables --lib \
>   backward_eqn_chebyshev_experimental::tests::region_5
> ```
>
> Generated 2026-08-31 04:44 (UTC).

## Status

These are **experimental, non-IAPWS correlations** fitted in-house (see GitHub issue #34). IAPWS-IF97 publishes no backward equations for some of the cases covered here, so where a reference is quoted it is either an IAPWS equation already implemented in this crate or this crate's own forward equations — never a published backward-equation reference value.

Per `RESPONSIBLE_USE.md` this is AI-assisted draft material: the numbers below are measurements, **not a validation sign-off**. No human has reviewed them.

## Methodology

IAPWS-IF97 publishes **no** backward equations for Region 5, so there is no published reference to compare against. The check is therefore a round trip against this crate's own Region 5 forward equations, which are line-for-line transcriptions of the IAPWS tables.

A 60 x 60 grid is swept over the full fit domain — pressure log-spaced from 1e-4 to 50 MPa, temperature linear from 1073.15 to 2273.15 K. At each node the forward equations `h_tp_5` and `s_tp_5` supply the reference enthalpy and entropy, and the backward correlations must recover the temperature the state was generated at. The grid is deterministic, so these numbers are reproducible.

Pass criterion: the recovered temperature must match the originating temperature within the envelopes recorded in the test source, which are the measured values rounded up.

## Results

Deviation in recovered temperature over 3600 grid points:

| Correlation | max |dT| [K] | RMS dT [K] |
|---|---|---|
| T(p,h) | 2.319e-2 | 1.602e-3 |
| T(p,s) | 7.525e-4 | 2.021e-5 |

## Interpretation

Both correlations reproduce the forward equations to far better than the ~0.01 K resolution at which a Region 5 temperature is normally meaningful, so as an accelerator replacing an iterative solve they are numerically sound over the fitted box.

This says nothing about agreement with IAPWS beyond what the forward equations themselves guarantee, and it says nothing about behaviour **outside** the fit domain, where the Chebyshev polynomial is an unbounded extrapolation. Neither function clamps its input.

