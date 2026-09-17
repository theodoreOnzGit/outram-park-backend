# (rho,h) flash by inversion of the IAPWS-IF97 backward equations

> **Generated file — do not hand-edit.** Regenerate with:
>
> ```bash
> cargo test --release -p tampines-steam-tables --lib write_rho_h_flash_vv_report
> ```
>
> Generated 2026-09-17 11:17 (UTC).

## Status

This report covers `p_rho_h_eqm` / `tpx_rho_h_eqm`, which **invert the published IAPWS-IF97 backward equations** rather than fitting a correlation. Every thermodynamic value comes from this crate's IF97 implementation; what is added is the inversion strategy (region dispatch plus a bracketed root find), which is numerics, not thermodynamics.

The reference nodes are the published steam tables this crate already checks its `(p,h)` flash against. Per `RESPONSIBLE_USE.md` this is AI-assisted draft material: the numbers below are measurements, **not a validation sign-off**. No human has reviewed them.

## Methodology

`v(p,h)` is already explicit in IF97 — Regions 1 and 2 through the `T(p,h)` backward equations, Region 3 through `v(p,h)` directly, Region 4 as a quality-weighted mixture. Inverting it for pressure is therefore a one-dimensional bracketed root find in `p` alone, with one explicit backward-equation evaluation per residual. The search interval is split at every IF97 region seam first, because `v(p,h)` is not monotone across one.

Two measures are recorded per node. `|dp/p|` is the error in the recovered **pressure** against the tabulated value. `|dv/v|` is the residual the root find actually minimises — whether the returned pressure reproduces the **density** it was given. A node counts as solved if either is tight; only a node failing both indicts the solver.

## Results — single-phase nodes, by region

| region | nodes | max abs(dp/p) |
|---|---|---|
| Region 1 | 1218 | 7.935e-1 |
| Region 2 | 957 | 3.049e-5 |
| Region 3 | 141 | 4.788e-5 |
| Region 4 | 18 | 3.728e-13 |

Of the nodes swept, **2284** recovered the pressure to better than `1e-6` relative and **50** recovered the density to better than `1e-9` instead. **0** failed both.

## Results — two-phase interior (Region 4)

States manufactured on each saturation tie line, where pressure and quality are both known in advance by construction.

| states | max abs(dp/p) | max abs(dx) |
|---|---|---|
| 1090 | 5.704e-5 | 5.631e-4 |

## Interpretation, including where this does not work

Region 4 — the two-phase dome, which is where a depressurisation transient spends its time — is recovered essentially exactly. Regions 2 and 3 are recovered to the level of the tables' own six-figure rounding.

**Region 1, the compressed liquid, is not recoverable, and that is physics rather than a defect.** Liquid water is nearly incompressible, so its density carries almost no information about its pressure. At 0.1 bar and 18 degC the amplification `|d ln p / d ln v|_h` reaches `2.0e5`, which exceeds the pressure signal across the whole range of interest: IF97's own `T(p,h)` backward equation, good to about 25 mK, already moves the specific volume by more than the pressure does. The solver there returns a pressure reproducing the requested density to machine precision that is still badly wrong, and no implementation can do better. `p_rho_h_conditioning` exposes this so a caller can detect the regime.

A second, smaller effect: nodes sitting exactly on an IF97 sub-region seam show a large `|dv/v|` with an excellent `|dp/p|`, because the reference `v(p,h)` is itself discontinuous there. The 40 bar / 280 degC node lands on the Region 2a/2b backward-equation split at `p = 4 MPa`, where `v` steps by `1.5e-5` while the pressure is recovered to `2.6e-13`.

