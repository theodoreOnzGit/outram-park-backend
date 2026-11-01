# Wet well (pressure-suppression pool) (gh:#1075)

> Research, education and V&V only. AI draft, unreviewed.

Part of the RELAP-7 parity epic (gh:#1056). The RELAP-7 Theory Manual §6.12,
Eqs 389–400 (private; cited, not quoted) describes a 0-D wet well. This is a
clean-room equivalent whose physics is **ported from `sembawang::bwr_mark1`**
(the Fukushima Unit 1 torus, MELCOR-based), not written fresh.

## What it does

`tampines::components::wet_well::WetWell`:

- **Gas space**: N2 plus steam, ideal gases at one temperature; N2 with
  MELCOR's `c_v(T)` fit.
- **Pool**: well mixed, one temperature. Its **level is a state**,
  `L_w = m_w / (ρ_w A_c)` with IF97's density (Eq 398). The gas occupies
  `A_c (L_t − L_w)` (Eq 397), so a rising pool compresses it.
- **Injection port** at elevation `z_i`: water of any phase, optionally with
  N2.
  - A two-phase injection is split by IF97's saturation enthalpies at the
    port's pressure `p_g + ρ g (L_w − z_i)` (Eqs 395, 400). The liquid joins
    the pool; the vapour goes through MELCOR's bubble model.
  - Within 5 K of saturation part of the steam passes uncondensed.
  - The submergence comes from the level.
- **Vacuum breaker**: one-way orifice `C_d A √(2 ρ ΔP)`, opening beyond its
  set point, capped at the equalising amount.
  - The direction is a required choice (`IntoGasSpace` /
    `OutOfGasSpace`), because RELAP-7's Eq 389 and sembawang's breakers pass
    gas out of the wet well, while a Mark I's building-to-torus breakers pass
    air in.
  - The external volume is a fixed-state reservoir.
- **Immersed heat exchanger**: `UA (T_pool − T_hx)`, implicit in the pool
  temperature.
- **Liquid outlet**: drawn at the pool's enthalpy (Eqs 391–392).
- **Pool surface**: MELCOR's mass and energy exchange, flashing and
  rain-out.
- **Compression work** `p ΔV` from the pool to the gas. RELAP-7 neglects it
  (the remark after Eq 390).
- **API**: `step(dt, &inputs)`, running boundary totals (`bookkeeping`), and
  getters for:
  - pressure, level, temperatures, masses and density;
  - Eq 395's reference pressure, Eq 400's pressure at an elevation and
    Eq 399's pipe-end pressure.

### Ported from where

`tampines` cannot depend on `sembawang`: it would pull the source-term chain
into the framework, and `sembawang` is meant to consume this component later.
So the logic is copied with its operation order, and each function's doc
comment names its original. `sembawang` is unchanged.

| Here | `sembawang::bwr_mark1` |
|---|---|
| `exchange` + property tables | `pool_surface::exchange` (H2 removed) |
| `bubble_efficiency`, `t_sat`, `steam_h` | `pool_surface::*` |
| bubble step | `containment::Containment::bubble_through` |
| flashing / surface / rain-out | `Containment::pool_surface_step` |
| vacuum breaker | the `ww_atmosphere` breaker branch of `Containment::step` (mirrored for inflow) |
| heat exchanger's implicit form | `Containment::ww_shell_step` |
| N2 `c_v(T)`, `u(T)` | `gas::cv_n2`, `gas::u_n2`, `ncg_u`, `ncg_c` (~~copied~~ since 2026-11-01 used from `tampines_steam_tables::gas_mixture::melcor_ncg`, where `gas` moved verbatim: gh:#1092, gh:#1098 item 4; bit-identity on a grid pinned by `tests/wet_well.rs::n2_fits_are_melcor_ncg_bit_for_bit`) |
| pool density | `sparc::Pool::rho_l` |

~~`sembawang` became a **dev-dependency** of `tampines` (non-Android, non-wasm
block) so the test can check the copies bit for bit. There is no cycle today.~~
**CORRECTED 2026-11-01** (maintainer: `sembawang` depends on `tampines`,
never the reverse): that dev-dependency is removed, and the comparison (item
3 below) moved to `crates/sembawang/tests/wet_well_vs_tampines.rs`, through
`sembawang`'s `tampines` dependency (a normal one since 2026-11-01).

### Mechanisms (all on by default) and their ablations

| Ablation | Effect |
|---|---|
| `without_surface_mass_transfer(α)` | RELAP-7 Eq 390: sensible `A_c α ΔT` only |
| `without_bubble_pass_through()` | `ε = 1` always |
| `without_nitrogen_cv_t()` | constant 20.8 J/(mol K) |
| `without_pressure_work()` | RELAP-7's omission |

`tests/wet_well.rs::every_mechanism_is_on_by_default` pins the defaults.

## Methodology

`tests/wet_well.rs`, release. The geometry is sembawang's Unit 1 torus:

- pool surface 750 m² and pool 1.74125e6 kg;
- gas space 2620 m³;
- port at the downcomers' 1.209 m submergence, opening 0.515 m;
- breaker 0.5 m², `C_d` 0.6, opening at 3.4 kPa;
- initially 105 kPa and 303.15 K, gas saturated.

The predictions were written in the test file's header before the first run.

1. **Closure.** 600 s at 1 s:
   - 20 kg/s injected, two-phase at 1.6e6 J/kg and then steam at 2.7e6 J/kg,
     with 0.5 kg/s of N2 at 400 K;
   - UA 2 MW/K against 300 K coolant;
   - 5 kg/s drawn from the pool;
   - the breaker venting out.

   Bars: water and N2 below 1e-12 of the inventory; energy
   `ΔE − (E_in − E_out)` below 1e-9 of the injected energy.
2. **Level hand check.** 1000 kg of steam into the subcooled pool, the
   surface ablated to α = 0. Prediction: the pool gains exactly 1000 kg, and
   `ΔL = Δm/(ρA) + L β ΔT = 1.339e-3 + 2.49e-4` m, within 2 %.
3. **Against sembawang** (in `crates/sembawang/tests/wet_well_vs_tampines.rs`
   since 2026-11-01).
   - (a) Ported primitives, bit for bit, over 300 surface states.
   - (b) sembawang's `Containment` stepped standalone with relief steam only
     (10 kg/s at 2.7e6 J/kg, 600 s), against this component at the same pool
     mass and gas volume.

   Prediction for (b): pool temperature within 2e-3 K, and the gas pressure
   here higher by 150–400 Pa. The rising pool compresses the gas here, but
   sembawang's gas volume is fixed.
4. **Vacuum breaker.** No flow below the set point or against the breaker's
   direction. Above the set point, the difference ends between
   `set − (γ−1)·excess` and the set point.
5. **RELAP-7.** **No open published RELAP-7 wet-well test case is held.**
   §6.12 describes the model, not a test. In its place, the RELAP-7-equivalent
   mode (sensible-only surface, constant `c_v`, no pressure work, dry N2 at
   350 K) is checked against Eq 390 in closed form:
   - implicit Euler's `ΔT_0/(1+kσ)^N` to 1e-9;
   - the exact exponential to 0.5 %;
   - Eqs 395, 399 and 400 to round-off.
6. **Pass-through.** A pool 2.6 K below saturation at the port gives
   `ε = 0.5 ± 0.02`, and `1 − ε` of the steam reaches the gas.

## Results (2026-11-01, `cargo test --release -p tampines --test wet_well`: 9 passed)

| Case | Measured | Prediction |
|---|---|---|
| 1. Water closure | 1.7e-9 kg of 1.74e6 kg (9.5e-16) | held |
| 1. N2 closure | −8.9e-11 kg of 2934 kg (−3.0e-14) | held |
| 1. Energy closure | 2.4e-3 J of 2.59e10 J injected (9.3e-14) | held |
| 1. Breaker vented / pool rise | 206 of 300 kg N2; +2.618 K | held (2.5–4.5 K) |
| 2. Level | ΔL 1.5898e-3 m against 1.588e-3 m by hand (+0.11 %) | held |
| 2. Pool mass gain | **1000.00036 kg, not 1000** | **miss** (see below) |
| 3a. Primitives | 300 surface states, 5 `ε`, 10 N2 pairs bit-identical | held |
| 3b. Pool temperature vs sembawang | 305.261372 vs 305.261414 K (−4.2e-5 K; +5.4e-6 K without pressure work) | held |
| 3b. Gas pressure vs sembawang | +349.3 Pa (105693.96 vs 105344.68) | held (150–400) |
| 4. Breaker, closed cases | zero flow, both directions | held |
| 4. Breaker inward, final ΔP | **3400.10 Pa** | **miss of the upper bound by 0.10 Pa** |
| 4. Breaker outward, final ΔP | 3240.59 Pa | held (`(γ−1)` bound 3240) |
| 5. Eq 390 vs implicit Euler | worst 3.7e-14 | held |
| 5. Eq 390 vs exact exponential | +0.44 % (first-order, `kσ = 3.8e-3`) | held |
| 5. Eqs 395, 399, 400 | within 1e-9 of `p_g` | held |
| 6. Pass-through | ε = 0.500000; 0.500000 kg to the gas; ablated: none | held (after a setup change, below) |

### Misses, as measured

- **Level, pool mass (case 2).** The pool gained 1000.00036 kg. The
  prediction overlooked that the rising pool compresses the *saturated* gas,
  and rain-out returns 3.6e-4 kg of steam to the pool. The test now asserts
  that the excess equals the steam the gas lost, to 1e-9 kg. With 90 %
  humid gas the gain is exactly 1000 kg. The level figure itself held.
- **Breaker upper bound (case 4).** The prediction placed the final
  difference at or below the set point; it ended 0.10 Pa above. The breaker
  acts at the start of a step, and the pool surface then cools the gas the
  inflow warmed, so the difference creeps back above the set point and the
  breaker reopens (62 times near it). This is operator splitting, not a
  breaker passing gas below its set point. The test now asserts, step by
  step, that the breaker flows exactly when the difference as it acts
  exceeds 3400 Pa, and records the 3400.10 Pa. The bound was not widened.
- **Pass-through setup (case 6).** The first run's setup at 105 kPa could
  not be built. The pool would have been above the surface's saturation
  temperature, because the hand estimate of the port's pressure was too low
  (it is about 1.18e5 Pa). The setup moved to 130 kPa; the prediction is
  unchanged.

## Interpretation

- The copies reproduce sembawang's surface and bubble physics exactly.
- Against sembawang's whole torus, the pool temperatures agree to 4e-5 K.
- The only systematic difference is the one predicted: the gas compression
  from the pool level, which sembawang does not model.
- Closure is at round-off.

## Limits (stated in the module)

- N2 is the only non-condensable; no H2.
- The gravity terms `(z − L_w/2) g` of Eq 393 are omitted (about 25 J/kg
  against 2.3e6 J/kg), and so is kinetic energy.
- No stratification and no shell heat structure (sembawang's `pool_layers`
  and `ww_shell`); no swell; no scrubbing.
- The liquid port only draws water out, and the vessel is prismatic.
- The energy conventions are sembawang's: constant liquid `c_p` and constant
  steam `c_v`.
- Moving sembawang onto this component is a separate decision, because its
  fixture and pins would move.
