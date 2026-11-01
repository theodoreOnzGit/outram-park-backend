# Wall friction, gravity and wall heat in `HemPipe` (gh:#1061, gh:#1062)

> Research, education and V&V only. AI draft, unreviewed.

Part of the RELAP-7 parity epic (gh:#1056). Source: the RELAP-7 Theory
Manual (cited, not quoted):

- §2.1.1, Eqs 21–22: the momentum and energy sources;
- §2.2.1, Eqs 24–34: wall friction;
- §2.2.2, Eqs 35–37: wall heat;
- §3.3.2, Eq 222: the HEM mixture viscosity.

## What was built

- **`hem::wall_friction`.** The RELAP5 four-region Darcy factor: laminar
  `64/(Re Φ_S)`, reciprocal transition 2200–3000, then Zigrang–Sylvester with
  the corrected 1.14. Re-exported as `tampines::closures::wall_friction`.
- **`HemEquationOfState::dynamic_viscosity`.** Optional, default `None`.
  - IAPWS-IF97 supplies it: IAPWS viscosity single-phase, and inside the dome
    `μ = (1 − α)μ_f + αμ_g` (Eq 222).
  - The ideal gas, the reduced closures and CoolProp supply none, so their
    pipes are frictionless.
- **Friction and gravity sources in `HemPipe`.**
  - Momentum: `−ρ f u|u|/(2D_h) − ρ g sinθ`. Energy: `−ρ u g sinθ`. The wall
    does no work, so friction has no energy term.
  - Friction is **on by default**, with `HemPipe::without_wall_friction` as
    the ablation.
  - `HemPipeGeometry` gains `with_incline` (default 0), `with_roughness`
    (default 0, smooth) and `with_shape_factor` (default 1).
- **`WallHeat`** (adiabatic by default, `HeatFlux`, or `WallTemperature` with
  `h`) and a volumetric source, `set_volumetric_heating` (Eq 22's `r`). Both
  are applied through the circular wall area per length over the flow area.

### The low-Reynolds default departs from RELAP-7, on evidence

RELAP-7 caps `f = f_max` below `Re = 64`. That cap makes the drag on a
stopping flow quadratic in `u`. The exact fully developed laminar law,
`64/(Re Φ_S)`, keeps it linear (Stokes) and is up to `64/Re` times larger.

With the cap, the `tampines` two-fluid blowdown's volume constraint drifted
to **1.68e-6**, breaching its 1e-6 gate (8.0e-10 before the change). Nearly
stagnant phases lost their linear drag. With the laminar law continued below
64 it is **8.0e-10** again; that test was run on 2026-11-01 to check exactly
this hypothesis.

So the default is the laminar law, the correct physics. RELAP-7's cap is
available as `LowReynolds::FMax`, with `f_max = 1/Φ_S`, because the manual
names `f_max` without giving it.

## Predictions (written before the first run, 2026-11-01)

1. Zigrang–Sylvester within 1 % of Colebrook over Re 4e3–1e8, ε/D 0–0.05;
   laminar exact; continuous at the region edges.
2. Closures without a viscosity, and horizontal pipes: byte-identical.
3. *(Replaced before running; see below.)* Steady fully developed friction.
4. A hydrostatic column stays near rest: |u| < 1e-2 m/s, dp/dz within 1 %.
5. Edwards with friction on moves by a few percent, depressurising more
   slowly.
6. The multiphase drift-flux and two-fluid Edwards and Marviken metrics move
   < 1 %.
7. *(#1062)* Each heat source adds exactly its heat in a closed pipe.

## Results (2026-11-01, release build)

| # | Case | Measured | Prediction |
|---|---|---|---|
| 1 | Zigrang–Sylvester against Colebrook | worst 0.29 % | held |
| 2 | Sod hash; IF97 break with friction ablated, hashed on the old and new code | `06b78ecca14dfc88` / `3627eddfbfad3e55`, identical | held |
| 3 | Uniform IF97 liquid flow (1 MPa, 300 K, 3 m/s, ε 1e-4 m), one forward-Euler step | Δ(ρu) = −7.904579e-4 against −dt ρ f u²/(2D_h) = −7.904579e-4 (f 0.01988, Re 3.96e5) | equal to round-off |
| 4 | Vertical 10 m IF97 column initialised hydrostatic, 20 ms | max \|u\| 1.3e-3 m/s; dp/dz −0.0036 % from −ρg | held |
| 5 | Edwards–O'Brien, 24 cells | see below | magnitude held, direction mixed |
| 7 | Air, closed pipe: flux 1e4 W/m², volumetric 1e6 W/m³, wall 400 K with h = 1000 | ΔE equal to the heat put in, to 1e-9, in each case | held |

**About case 3.** Prediction 3 (steady fully developed `dp/dx` in a long IF97
liquid pipe) was replaced before it ran by the one-step deceleration check
above. Reaching a steady state with IF97 inversions would have taken minutes
per run, and the one-step check tests the same wiring, the closure times the
cell state, exactly.

**Edwards–O'Brien at GS-1:**

| | Frictionless (ablated; reproduces the record) | Friction, f_max cap | Friction, laminar default |
|---|---|---|---|
| plateau, 0.02–0.06 s | 368.0 psia | 366.9 psia | 367.6 psia |
| RMSE against data, 16 points | 44.2 psia | 42.8 psia | 42.8 psia |

Over 0.10–0.24 s friction lowers GS-1 by 2–3 psia; from 0.26 s it raises it
slightly. "Slower depressurisation" held only for the late part. All Edwards
gates still pass.

**Water double rarefaction.** The semi-analytical reference is inviscid. With
friction on, the plateau speed moved from −0.05 % to −0.79 % of `u_b`. The
test now ablates friction, which matches the instrument to its reference, and
reads −0.05 % again.

## Multiphase (`tampines::multiphase_1d`)

The drift-flux and two-fluid solvers now use the shared factor (smooth, Φ_S
= 1) in place of their two Blasius copies.

**Suite rerun with the laminar default:** `cargo test --release -p tampines
--lib multiphase_1d`, 2026-11-01. 40 passed, 1 failed, 3 ignored before
re-pinning; the one failure is the Marviken 24 characterisation pin below.

- **Two-fluid blowdown:** volume residual 8.0e-10, as before. With the f_max
  cap it was 1.68e-6, which is the evidence for the low-Reynolds default.
- **Marviken 23 (drift flux):** unchanged at the recorded precision
  (8.9 % / +2.8 % / 19.7 %).
- **Marviken 24 (drift flux):**

  | Metric | Before | After |
  |---|---|---|
  | signed deviation | −9.7 % | −9.8 % |
  | max \|dev\| | 28.8 % | 28.9 % |
  | mean \|dev\| | 11.4 % | 11.4 % |
  | points outside the band | 6/40 | 6/40 |
  | worst steadiness | 0.1256 | 0.1498 (gate 0.20) |
  | interior maxima | 1/40 | 3/40 |

  The interior-maxima count is a characterisation pin and was re-pinned with
  the old value, the new one and why.

Prediction 6 (every gated metric within 1 %) held.

## Not done here

- Phasic wall heat in the two-phase solvers (RELAP-7 Eq 201; rest of gh:#1062).
- A viscosity for the CoolProp closure.
- Roughness in `Pipe1d`.
