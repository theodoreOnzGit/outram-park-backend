<!--
SPDX-License-Identifier: MIT
Part of Outram Park (outram-park-backend).
A fork/translation of CoolProp (https://github.com/CoolProp/CoolProp, MIT).
CoolProp is not affiliated with or endorsing this fork. See NOTICE / TRADEMARKS.md.
-->

# outram-park-fork-coolprop

A pure-Rust fork/translation of **[CoolProp](https://github.com/CoolProp/CoolProp)**
(MIT) — thermophysical properties from Helmholtz-energy-explicit equations of
state — built to OUTRAM PARK's design rules.

This is an **independent fork**, not the CoolProp project and not endorsed by
it (see `TRADEMARKS.md`).

## What's different from CoolProp

- **Enum dispatch, no trait objects.** Fluids are a `Fluid` enum; EOS term
  forms are `ResidualTerm` / `IdealTerm` enums dispatched by `match`. CoolProp's
  string-keyed lookup and backend class hierarchy become exhaustive enums.
- **Hardcoded data, no runtime JSON.** Each fluid's EOS coefficients are `const`
  Rust in `src/fluids/`, generated once from CoolProp's fluid JSON by
  `dev/gen_fluid.py`. The shipped crate reads no files — a few KB per fluid
  (full IAPWS-95 Water is ~3 KB).
- **Pure `std`, no BLAS / C dependencies** — so it also builds for Android.

## Theory

For a pure fluid the reduced Helmholtz energy is
`α(δ, τ) = α⁰(δ, τ) + αʳ(δ, τ)`, with `δ = ρ/ρ_r` and `τ = T_r/T`. All
thermodynamic properties follow from `α` and its first/second `δ`,`τ`
derivatives (Span–Wagner / IAPWS-95). The residual part is a sum of
**Power** (`n·δ^d·τ^t·exp(-δ^l)`), **Gaussian** (bell-shaped) and
**non-analytic** (critical-region) terms; the ideal part is **Lead**,
**LogTau** and **Planck–Einstein** terms.

## Status (initial port — 2026-07-08)

First vertical slice, end-to-end and verified:

- Helmholtz EOS engine: residual **Power** + **Gaussian** terms and the ideal
  **Lead / LogTau / Planck–Einstein** terms, with all first/second `δ`,`τ`
  derivatives.
- `(T, ρ)` property evaluation (`props::state_trho`): p, u, h, s, c_v, c_p,
  speed of sound.
- **Water (IAPWS-95)** hardcoded; pressure reproduces CoolProp's tabulated
  triple-line value (611.6548 Pa) to **~1×10⁻⁴** at both saturated-liquid and
  saturated-vapour points (`tests/water_reference.rs`).

Tracked follow-ups (beads `op-kbc`):

- **Non-analytic critical-region terms** — carried in the fluid data but not yet
  evaluated (a no-op, so accuracy within ~1 % of the critical point is
  degraded; unaffected elsewhere).
- `(T, p)` / `(p, h)` … **flashes** (need a density solve).
- **More fluids** via `dev/gen_fluid.py`.
- **`rfluids` verification** (CoolProp oracle) as a dev-dependency.
- **`uom`-typed public API** (internally raw `f64` SI).

## Regenerating fluid data

The CoolProp reference clone lives in the **gitignored** `reference/` (dev
only). Regenerate it and a fluid's Rust data with:

```bash
git clone --depth 1 https://github.com/CoolProp/CoolProp.git reference/CoolProp
python3 dev/gen_fluid.py Water > src/fluids/water.rs
```

## References

- CoolProp — <https://github.com/CoolProp/CoolProp> (I. Bell et al., MIT)
- Wagner & Pruß (2002), *The IAPWS Formulation 1995…* (IAPWS-95), J. Phys.
  Chem. Ref. Data 31(2)
- Span & Wagner, multiparameter Helmholtz equations of state
