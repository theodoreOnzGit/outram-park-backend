# p(rho,h) in Region 1: conditioning of the pressure inversion

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

## Why this report exists

Region 1 is the weakest case for `p(rho,h)`, and an aggregate statistic badly misrepresents it. The error is overwhelmingly a **low-pressure** effect rather than a liquid effect, and the difference matters: the blunt reading ("do not use this in subcooled liquid") would rule out ordinary power-cycle conditions where the correlation is in fact accurate.

## Methodology

The Region 1 subset of the standard single-phase sweep (60 x 60 over `(p, T)`; see the single-phase report for how states are generated), with the relative error in recovered pressure binned by pressure decade and expressed as a percentage.

## Results

Relative error in recovered pressure, by pressure decade:

| Pressure | n | median | 90th pct | 99th pct | maximum |
|---|---|---|---|---|---|
| 1e-3 – 1e-2 MPa | 15 | 88.2413% | 220.7433% | 265.1739% | 318.5474% |
| 1e-2 – 1e-1 MPa | 32 | 6.7719% | 20.3487% | 27.3736% | 29.3643% |
| 1e-1 – 1 MPa | 54 | 0.6994% | 1.8331% | 2.5237% | 2.7075% |
| 1 – 10 MPa | 100 | 0.0541% | 0.1727% | 0.2799% | 0.3005% |
| 10 – 100 MPa | 98 | 0.0072% | 0.0166% | 0.0284% | 0.0349% |

## Interpretation

The inversion is ill-conditioned wherever density stops responding to pressure. Liquid water is very nearly incompressible, so along a low-pressure isotherm density barely moves while pressure changes by orders of magnitude, and recovering pressure from density amplifies any error enormously. **This is a property of the state variables, not a defect in the fit — no better fit can remove it.**

Practical guidance: above roughly 1 MPa Region 1 recovers pressure to better than a few tenths of a percent, and better still above 10 MPa, which covers ordinary power-cycle liquid conditions. Below roughly 0.1 MPa it should not be used — carry pressure as a state variable there instead.

The same corner is where the Region 4 two-phase sweep has its own worst point (the 280 K bubble point, around 1e-3 MPa). That two independent sweeps agree on the location of the difficulty is corroboration that the explanation is the conditioning of the state variables rather than a local defect in one fitted surface.

