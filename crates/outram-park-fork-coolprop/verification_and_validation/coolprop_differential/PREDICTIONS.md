# Predictions, written BEFORE any comparison run (gh:#1349, 2026-11-13)

Committed ahead of the first run so the comparison can fail. Upstream: CoolProp
`0e67fe74b30a2fe9526af9bc64ea026a96f56ebf` (2026-07-05, the commit this crate ports,
`upstream_source/README.md`), built from source as a static library with g++ 13.3,
`-O3 -DNDEBUG` from the CMake Release profile plus `-ffp-contract=off`, no `-ffast-math`,
no `-march=native` (so no FMA), glibc 2.39, x86-64.

What I read before predicting (source, not a run): this crate's `props::state_trho`,
`eos::FluidEos::{residual,ideal}_derivs`, `ancillaries::SatAncillary::evaluate`,
`vle::saturation_at_temperature`, `flash::{density_pt,solve_pt_outer}`, the README coverage
statement, and the generator `dev/gen_fluid.py` (coefficients are copied programmatically from
the fluid JSON, not typed). I have not yet read CoolProp's `FlashRoutines.cpp`.

Inputs are fixed by rule (grids below), never by what makes a number agree. Grid points are
shared bit for bit: the driver prints the `(T, rho_mass)` it fed to CoolProp (`DmassT` input),
and CoolProp's own `delta`, `tau`; the Rust side consumes those printed values.

| # | claim | predicted outcome |
|---|---|---|
| P1 | `alphar` and its five derivatives, 137 fluids, (tau, delta) grid incl. near critical | worst relative difference < 1e-12 per fluid per term, and >= 60 % of all values bit-identical. Differences come from `exp`/`pow`/`ln` (PETIR vs glibc) and from summation order. At most 3 fluids exceed 1e-13; one near-critical cell may exceed 1e-12 through cancellation in the second derivatives (I will report it with the cancellation scale, not hide it) |
| P2 | `alpha0` and derivatives | < 1e-13 worst; >= 90 % bit-identical (few transcendental calls) |
| P3 | transposed or mistyped coefficient in the generated fluid files | none (the generator copies the JSON). If any fluid is worse than 1e-10 on alphar, that is a real defect, localised by the term group |
| P4 | `(T, rho)`: p, u, h, s, cv, cp, w | < 1e-11 relative for h, s, cv, cp, w; p less tight where `1 + delta*alphar_delta` is near 0 (reported with a scale-normalised error). h and s: **a reference-state offset on some fluids** (CoolProp applies the JSON `STATES`/`ideal` offset conventions at load; the port may not) is my main worry: expect 0-15 fluids with a constant absolute offset in h and s, reported as such and not buried in a relative tolerance |
| P5 | saturation `T -> (p, rho', rho'')` | Rust Maxwell solve vs CoolProp: < 1e-8 for T <= Tc - 1 K; Rust returns None for some points within ~1 K of Tc (documented) and for fluids without ancillaries (6 of 137). The ancillary fits themselves: < 1e-13, >= 80 % bit-identical |
| P6 | PT flash | same root as CoolProp on >= 99 % of the grid; at least one fluid, in the compressed-liquid region, returns the vapour root or fails (gh:#917 class). Where the root agrees: density < 1e-6 relative (Rust brackets and bisects to a pressure tolerance), not 1e-12 |
| P7 | PH, PS round trip (T from CoolProp's own h or s) | agreement bounded by the PT flash (< 1e-6 in T), with failures concentrated in two-phase-adjacent states |
| P8 | transport mu, lambda | the Rust side has a model for 42 / 45 fluids; CoolProp has more (CS/ECS/rhosr fallbacks): the CoolProp-yes / Rust-no list is the gap inventory. Where both have a model: < 1e-10 away from the critical point; near critical (abs(T-Tc) < 5 K, abs(rho-rho_c) < 0.3 rho_c) lambda may differ at 1e-6 or worse because the enhancement uses cp, cv and drho/dp |
| P9 | any term that disagrees at > 1e-10 | I expect 0-4 (fluid, term) groups. Each is localised to the first divergent operation before any fix; a fix is a translation of the upstream routine |

Any observed number outside these is reported as found, with the localisation. None of the
bounds above will be edited after the run.

Grids (fixed now):
- Terms and (T, rho): per fluid, `tau` = 11 log-spaced values between `Tr/Tmax` and
  `Tr/Ttriple`, times `delta` in {1e-4, 0.01, 0.1, 0.3, 0.6, 0.9, 1.0, 1.1, 1.5, 2.0, 2.5,
  3.0}, plus a 7 x 7 near-critical block `tau` in {0.99, 0.995, 0.999, 1, 1.001, 1.005, 1.01}
  times `delta` in {0.9, 0.95, 0.99, 1, 1.01, 1.05, 1.1}. 181 points per fluid.
- Saturation: 30 temperatures from Ttriple to Tc - 1e-3 K, spacing refined towards Tc
  (`T = Tt + (Tc - 1e-3 - Tt) * (1 - (1-u)^3)`, u uniform on [0, 1]).
- PT: for 8 subcritical temperatures (Ttriple*1.001 to Tc - 0.01 K, uniform) the pressures
  `psat * {0.5, 0.9, 0.99, 1.01, 1.1, 2, 10, 100}` (capped at 0.9 pmax), and 6 supercritical
  temperatures (1.01 Tc to 0.95 Tmax) times 6 pressures log-spaced from 0.01 pc to 0.9 pmax.
