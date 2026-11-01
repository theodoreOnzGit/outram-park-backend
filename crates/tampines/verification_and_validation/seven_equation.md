# Seven-equation two-pressure model (gh:#1059)

> Research, education and V&V only. AI draft, unreviewed.

Part of the RELAP-7 parity epic (gh:#1056). The RELAP-7 Theory Manual
§3.1.3–3.1.11 (Eqs 115–190) and §3.2.1 (Eqs 191–198, Remarks 1–2) is
private, so it is cited by equation number only. The scheme follows the
published Saurel & Abgrall (1999), *J. Comput. Phys.* 150, 425–467, with
relaxation procedures after Saurel, Petitpas & Berry (2009), *J. Comput.
Phys.* 228, 1678–1712, and Saurel, Petitpas & Abgrall (2008), *J. Fluid
Mech.* 607, 313–350. Dated 2026-11-01.

## What was built

`tampines::multiphase_1d::seven_equation::SevenEquation1d`:

- Two pressures. A volume-fraction equation `∂α/∂t + u_int ∂α/∂x = μ Δp`.
  Phasic mass, momentum and total energy, with the non-conservative
  `p_int ∂α/∂x` and `p_int u_int ∂α/∂x` terms. Quasi-1-D area `A(x)` from
  `Pipe1d` (gh:#1058), and gravity.
- Interfacial variables (Eqs 191–196): impedance-weighted with the
  `sgn(∂α_l/∂x)` terms (the default), Baer–Nunziato, and Saurel–Abgrall.
- Relaxation (Eqs 197–198): `μ = A_int/(Z_l + Z_v)` and
  `λ = ½ μ Z_l Z_v`, with `A_int` from the gh:#1060 regime map. Each can be
  finite (the default), instantaneous (the stiff `μ, λ → ∞` operator), or
  ablated.
- Optional instantaneous thermo-chemical relaxation to saturation (the HEM
  limit). It is off by default because it is an equilibrium limit, not a
  finite-rate closure.
- The non-condensable species of gh:#1064, conserved with the vapour flux.
- Characteristic analysis:
  - `characteristic_speeds`: `u_k ± c_k`, `u_k` and `u_int`, all real;
  - `single_pressure_characteristic_speeds`: the Remark-2 reduced system,
    with roots from PETIR's companion-matrix solver;
  - `wood_sound_speed`.
- An entropy inventory with stiffened-gas entropies.
- Ablations:
  - `without_pressure_relaxation`, `without_velocity_relaxation`;
  - `without_interface_sign_terms`, `without_regime_map`;
  - `without_hllc` (Rusanov), `without_muscl`.

**Numerics.** The method is explicit finite volume with Godunov splitting:

- per-phase HLLC flux (Rusanov as the ablation);
- one face `α`, upwinded on `u_int`, used both in the phase fluxes and in
  the non-conservative terms (this is what Abgrall's criterion needs);
- MUSCL-minmod reconstruction and SSP-RK2 time stepping;
- instantaneous relaxations after each stage, finite-rate ones once per
  step.

**Reused:**

- `tampines_steam_tables::hem::{StiffenedGasHem, StiffenedGasPair}`
  (gh:#1057): the parameters and `T_sat`;
- `Pipe1d` with its `AreaProfile` (gh:#1058);
- `InterfacialExchange` (gh:#1060): its area was made public as
  `interfacial_area_concentration`;
- `petir::roots::BracketingSolver` (Brent) and
  `petir::poly::companion::roots_companion`;
- the Edwards case data in `edwards_tests.rs`, made `pub(super)`.

**Searched first:** no seven-equation, Baer–Nunziato, Saurel–Abgrall or
Kapila model exists in any crate. The KNP/HLL kernel in
`outram-foam-basic-lib` (`fvc::central_upwind`) is single-fluid and
conservative only, so it does not carry the non-conservative coupling.

## Predictions (written into the tests before each first run)

| # | Case | Prediction |
|---|---|---|
| 1 | Abgrall: uniform `p`, `u` across a void step | `|Δp|/p < 1e-10`, `|Δu| < 1e-8 m/s` |
| 2 | at rest in a 2:1 duct | `|u| < 1e-9`, `|Δp|/p < 1e-12` |
| 3 | closed tube | phase masses, species and `E` to `1e-13` |
| 4 | relaxation sweep | `E`, momentum to `1e-12`; entropy never falls; instantaneous equilibrium to `1e-6 Pa`, `1e-12 m/s`; no overshoot |
| 5 | Remark 2 | complex pair at every slip, `|Im|` ≈ the incompressible formula; real at zero slip with fastest speed `u + c_W` |
| 6 | Remark 1 (Kapila) | relaxed pulse at Wood's speed within 3 % |
| 7 | two-phase shock tube | per phase: `ρ` L1 < 2 %, star `p`, `u` within 2 % |
| 8 | water–air interface shock tube | star `u` within 2 %, `p` within 3 % |
| 9 | Ransom faucet | mean `|Δα|` < 0.03 (120 cells), < 0.02 (240); `α(4 m)` within 0.01; `u_l` within 0.3 m/s |
| 10 | species pulse | centroid / `u t` within 1 %; mass to `1e-12` |
| 11 | Edwards, HEM limit | GS-1 plateau 650–740 psia, RMSE worse than HemPipe's 44.2 psia; HEM state to `1e-6 K`, `1e-3 Pa`, `1e-9 m/s` |
| 12 | `TwoFluid1d` on Edwards | refuses within 0.05 s |

## Results (2026-11-01, release, `src/multiphase_1d/seven_equation/tests.rs`)

| # | Measured | Verdict |
|---|---|---|
| 1 | `|Δp|/p` 1.4e-11, `|Δu|` 9.3e-13 m/s; front 5 cells | met |
| 2 | `|u|` 7.1e-17 m/s, `Δp` exactly 0 | met |
| 3 | `m_v` 7.1e-16, `m_l` 2.6e-15, species 6.6e-16, `E` 2.6e-15 | met (after the fixes below) |
| 4 | `E` 3.9e-16, momentum 2.3e-17; entropy never fell; `|u_v − u_l|` 1e-31; **`|p_v − p_l|` 6.8e-5 Pa** | **1e-6 Pa bar MISSED** (below the representation floor; see below) |
| 5 | complex pair at every `w > 0`, `|Im|` within 0.05 % of the incompressible formula; **zero-slip fastest root `u + 341.8 m/s`, not `u + c_W`** | complex part met; **zero-slip expectation wrong** |
| 6 | 23.256 m/s against `c_W` = 23.650 (−1.67 %) | met (second protocol) |
| 7 | water `p*` +0.07 %, `u*` −0.05 %, L1 0.15 %; air −0.05 %, +0.06 %, 0.27 %; `α` exact | met |
| 8 | refused in the first stage | **MISSED** (pinned) |
| 9 | 0.0371 / 0.0316; `α(4 m)` 0.3875 / 0.4005 (exact 0.4011); `u_l` 0.62 / 0.48 m/s | **L1 and `u_l` MISSED**; convergence and `α(4 m)` met |
| 10 | 1.000018; mass 1.9e-16 | met (second protocol) |
| 11 | plateau **417.8 psia**, RMSE **144.7 psia**; HEM state 4.6e-7 K, 5.7e-14 m/s where the operator acted; `|p_v − p_l|` 1.16e-3 Pa | **plateau prediction MISSED**; RMSE as predicted (worse); **1e-3 Pa bar MISSED** |
| 12 | refused at 0.33 ms (vapour past its 30 K subcooling bound) | met |

### Comparison with `TwoFluid1d` and the other rungs on Edwards GS-1

| model | EOS | GS-1 plateau (psia) | RMSE vs data (psia) | status |
|---|---|---|---|---|
| data (digitised B-T-3271) | — | ~361 | — | — |
| HemPipe (tampines-steam-tables, 24 cells) | IF97 | 368.0 | 44.2 | completes 0.6 s |
| `DriftFlux1d` (24 cells) | IF97 | 354.3 | 28.2 | completes 0.6 s |
| `TwoFluid1d` (24 cells, 30 µs) | IF97 | — | — | **refused at 0.33 ms** |
| `SevenEquation1d`, HEM limit (100 cells, Rusanov) | stiffened gas (Le Métayer 2004) | 417.8 | 144.7 | completes 0.30 s |

GS-1 series, seven-equation against the data (psia):

| t (s) | 0.00 | 0.02 | 0.04 | 0.06 | 0.10 | 0.14 | 0.20 | 0.30 |
|---|---|---|---|---|---|---|---|---|
| seven-eq | 1015.3 | 465.0 | 400.4 | 388.1 | 364.7 | 272.4 | 92.2 | 14.1 |
| data | 985.0 | 350.8 | 364.0 | 367.4 | 312.2 | 296.0 | 289.0 | 190.1 |

**Interpretation.**

- The seven-equation model is the only one of the two-velocity rungs that
  marches Edwards at all. `TwoFluid1d` refuses at 0.33 ms, as its own
  record says it does from a low-void start.
- In the HEM limit the relaxation operators do what Remark 1 says: every
  cell they act on ends at `p_v = p_l`, `u_v = u_l` and
  `T_v = T_l = T_sat(p)`, to 4.6e-7 K.
- **"Recovers HEM within a stated tolerance" is met for the state, not for
  the curve.** The GS-1 curve differs from HemPipe's, for two reasons that
  are not the relaxation:
  1. **The stiffened-gas pair.** `p_sat` at the break-end temperature is
     740.7 psia, against IF97's 366, because of the gh:#1057 `q'`
     question.
  2. **No critical-flow break.** The pipe empties far too fast: GS-1 is at
     ambient by 0.28 s, while the data still read 190 psia at 0.30 s.

  An HEM pipe on the same stiffened-gas pair would be needed to separate the
  relaxation from the EOS. No such closure exists for `HemPipe`.
- My plateau prediction assumed the HEM fan stops a few per cent below the
  local `p_sat`. That is true of IF97 and of the data. With this pair the fan
  runs about 44 % below it.
- **Edwards is run with the Rusanov ablation.** HLLC is refused at 0.44 ms:
  a 1e-6 vapour trace in the break cell loses positivity beside the first
  flashing cell, where its own flux runs against `u_int`. A per-phase donor
  `α` was tried and failed the same way, and was reverted.

### Misses, and how each was handled

- **Relaxation and Edwards `|p_v − p_l|` bars (1e-6 Pa, 1e-3 Pa).** The state
  stores `α_v`, so at `α_v = 0.99` the liquid fraction loses two digits, and a
  pressure recovered as `(γ − 1) ρ (e − q) − γ p∞` carries
  `(p + γp∞) ε / α_l` of round-off (about 3e-3 Pa). The measured 6.8e-5 Pa is
  0.021 of that floor. The bars were set below what the representation can
  resolve. They are printed as missed, and the equilibrium is asserted
  against the floor.
- **Remark 2 at zero slip.** With independent velocities the quartic factors
  as `d² [(α_v ρ_l/c_v² + α_l ρ_v/c_l²) d² − (α_v ρ_l + α_l ρ_v)]`. That gives
  a harmonic-inertia speed (341.8 m/s at `α_v = 0.5`), not Wood's: Wood's
  speed needs velocity equilibrium as well, and test 6 shows it does appear
  then. The assertion checks the closed-form root.
- **Water–air interface shock tube.** `p_int` is cell-centred. At the
  material interface the water flux enters the air-side cell with
  `α_l ≈ 1`, while that cell's `p_int` is near the air pressure. The
  `1e-3 kg/m³` water trace then takes the whole water-side force and has no
  admissible relaxed pressure. A face-based interfacial pressure (Abgrall &
  Saurel 2003, the discrete equation method) is needed. The test pins the
  refusal. The two-phase shock tube with uniform `α` (test 7) is the
  shock-tube case that passes.
- **Faucet L1 and `u_l`.** Both converge (0.0371 → 0.0316) but sit above the
  predicted bars; the void at 4 m is right to 0.0006. The first run used the
  Rusanov default and gave 0.067 / 0.054. That result is why HLLC became the
  default. With `λ = 0` and instantaneous `μ` the faucet is RELAP-7's
  Remark-2 configuration, regularised only by numerical diffusion.
- **Protocol misses fixed without moving a bar** (each is in the test's doc
  comment):
  - Kapila: the first run released the pulse at rest, the left- and
    right-going waves overlapped, and the peak read −8.9 %. The pulse was
    made right-going.
  - Species: the first run lost 1.1e-5 of the pulse through the inlet tail.
    The pulse was moved away from the inlet.
  - Closed tube: the first set-up was refused, the interface limitation
    above.
- **Defects fixed in the solver by these tests:**
  - The wall flux leaked 5e-12 because the wall ghost was mirrored from the
    cell centre rather than from the reconstructed face state.
  - The finite pressure relaxation lost entropy (`ΔS/S` = −1.4e-8) with
    `p_int` frozen at the start of the step. `p_int` is now taken between
    the end pressures.
  - Its root search failed at extreme `α`. It is now bracketed between the
    start and the instantaneous equilibrium.
  - Davis wave speeds could put the HLLC contact outside `[S_L, S_R]`.
    Rusanov is now the fallback there.

### Marviken

Not run, for three reasons:

- There is no critical-flow break model. Edwards shows what the
  static-pressure outlet does to the discharge rate, and Marviken is a
  discharge-rate benchmark.
- The stiffened-gas `T_sat` offset would dominate the comparison.
- The vessel, discharge pipe and nozzle need a multi-component network that
  this solver does not have.

`TwoFluid1d` has no Marviken harness either (only drift flux does), so
there is no two-fluid result to compare against.

## How to run

```bash
cargo test --release -p tampines --lib multiphase_1d::seven_equation -- --nocapture   # ~15 s
```

The Edwards case takes about 9 s and needs no `long-tests` gate.

## Open items (for the maintainer to file)

- Face-based interfacial pressure and velocity (the discrete equation
  method), so that the water–air interface problem runs, together with
  positivity for trace phases under HLLC.
- A critical-flow break boundary for the seven-equation model.
- A stiffened-gas HEM closure for `HemPipe`, to compare the HEM limit
  code-to-code on one EOS.
- Finite-rate interfacial heat and mass transfer (RELAP-7's `Γ`).
- A Marviken harness.
