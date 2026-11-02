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

## Round-off sensitivity at saturation (gh:#1118 item 1, 2026-11-02)

**Methodology.** The torus of `tests/control_volume.rs` (10 kg/s relief
steam with 0.5 kg/s N2, the heat exchanger, a 5 kg/s outlet, 600 s) is run
twice, the twin's gas 1e-12 K warmer. The worst relative difference in
steam mass and gas temperature is compared across step sizes, across
surface models and across rain-out models. The test is
`tests/control_volume.rs::wet_well_round_off_amplification_gh1118`, and its
doc comment carries the predictions and the full table.

**Predictions (written before the run):** a stability limit of the explicit
surface step. That would mean it is gone at `dt` ≤ 0.5 s, absent with the
sensible-only surface, and not removed by the equilibrium rain-out.

**Results (release, Rust 1.99.1).** Three of the four predictions missed.

| Variant | Worst steam-mass difference |
|---|---|
| wet well, `dt` 1 / 0.5 / 0.25 / 0.1 s | 1.14e-4 / 5.7e-5 / 2.9e-5 / 1.2e-5 |
| wet well, sensible-only surface, 1 s | 9.9e-5 |
| control volume, four-pass rain-out, 1 s | 1.09e-4 |
| control volume, equilibrium rain-out, 1 / 0.5 / 0.25 s | 1.2e-13 / 1.3e-13 / 1.3e-13 |

**Interpretation.** The amplifier is the four-pass rain-out ported from
`sembawang`. Each pass condenses to saturation at the current temperature
and leaves the latent heat in the gas. That heat raises the gas's saturated
capacity by more than the mass condensed (about 14 times at 330 K). The pass
therefore ends far subsaturated, and the next step's evaporation refills the
gap. This map expands a difference at every step size, so it is not a
step-size stability limit. The bound scales with one step's excess vapour,
and so with `dt`. The control volume's equilibrium rain-out removes it.

~~**Not changed, stated.** Making the equilibrium rain-out the wet well's
default would change every number of its bit-for-bit pin against
`sembawang`'s `Containment` (`crates/sembawang/tests/wet_well_vs_tampines.rs`,
case 3b). The pin needs the four passes, because `sembawang` uses them. That
choice is left to the maintainer. The control volume (gh:#1092) is the
general model, and it already has the equilibrium rain-out on by default.~~ **SUPERSEDED 2026-11-02** (maintainer decision): the equilibrium rain-out is now the default, and the four passes, which carry the port check, are the ablation. See the next section.

## Equilibrium rain-out as the default (gh:#1118, maintainer decision 2026-11-02)

The maintainer's decision: the wet well's default becomes the control
volume's equilibrium rain-out, which bisects for the condensed mass that
leaves the gas at RH = 1 at the temperature its remaining energy gives.
`sembawang`'s four passes stay as the ablation
`WetWell::without_equilibrium_rain_out`, and that ablation carries the
bit-for-bit port check.

**Predictions (written 2026-11-02, before the code was changed):**

- **E1.** The ablation reproduces the old wet well bit for bit. Every
  number in the results table above, and case 3b's printed 305.261372 K and
  105693.96 Pa, come out the same.
- **E2.** Closure (case 1) stays at round-off with the new default: water
  < 1e-12 relative, energy < 1e-12 of the injected energy. Both rain-outs
  move mass and energy between gas and pool exactly.
- **E3.** Case 3b with the new default: the pool temperature moves by less
  than 1e-4 K from 305.261372 K, because the rain-out returns at most a few
  kg to a 1.7e6 kg pool. The gas pressure moves by 0 to +30 Pa: the gas now
  sits at saturation instead of zig-zagging below it, and a deficit of
  about 0.1 kg of steam is about 5 Pa in 2620 m³. Both of 3b's criteria
  (worst `|ΔT_pool|` < 2e-3 K, `Δp` in 150–400 Pa) still hold.
- **E4.** The control volume's special case (`tests/control_volume.rs`, its
  default equilibrium rain-out against the wet well's new default) agrees
  to < 1e-9 relative over all 600 s in all six quantities. That is the
  original prediction 2, which missed only because of the four passes'
  amplification. It is not expected bit for bit: Dalton's sum, the
  energy-to-temperature solve, the bisection's 1e-13 tolerance and the
  control volume's fixed density inside its rain-out probe all differ.
- **E5.** The round-off twin (gh:#1118 item 1) with the new default stays
  below 1e-11 in steam mass at 1 s.
- **E6.** The other wet-well cases move little:
  - Case 2's pool-mass excess still equals the steam the gas lost, to
    1e-9 kg (that is an identity).
  - Case 4's inward breaker ends within 1 Pa of its old 3400.10 Pa.
  - Case 6 is unchanged to 1e-6 (a single step with a subsaturated gas).

**Results (2026-11-02, release, Rust 1.99.1).** E1, E2, E4 and E5 held; E3
held for the pool and missed its pressure band; E6 held where it predicted,
but did not cover the outward breaker, which moved.

| Prediction | Old (four passes, now the ablation) → new default | Verdict |
|---|---|---|
| E1, ablation = old code | three 600/600/300-step trajectory hashes identical to the pre-change code (90ecc3373), `tests/wet_well.rs::four_pass_ablation_is_the_old_wet_well_bit_for_bit` | held |
| E2, closure (case 1) | water 9.5e-16 → 6.0e-16; N2 −3.0e-14 → −2.9e-14; energy 9.3e-14 → −4.2e-13 | held (< 1e-12) |
| E3, case 3b pool temperature | 305.261372 K → 305.261372 K (−6.1e-8 K) | held (< 1e-4 K) |
| E3, case 3b gas pressure | 105693.96 → 105693.95 Pa (−0.01 Pa); vs `sembawang` +349.28 → +349.27 Pa | **missed** the predicted 0 to +30 Pa: the surface warms this gas from 303 to 304.2 K, so it is mostly subsaturated and the rain-out barely acts; the 150–400 Pa criterion holds |
| E4, control volume vs wet well at both defaults | worst 1.05e-4 (four passes) → 6.1e-11 (steam; temperatures 1.1e-11, pressure 9.3e-12, the rest ≤ 3e-15) | held (< 1e-9): the original prediction 2 now holds |
| E5, round-off twin | 1.14e-4 → 2.3e-13 (steam) | held (< 1e-11) |
| E6, case 2 pool gain | 1000.00036 → 1000.000069 kg (rain-out 3.6e-4 → 6.9e-5 kg); ΔL 1.5898e-3 m unchanged | held (identity) |
| E6, case 4 inward breaker | 3400.10 Pa, 9.52 kg, 62 re-openings: unchanged | held |
| not predicted: case 4 outward breaker | 17.35 → 10.26 kg; 3240.59 → 3400.04 Pa; 2 → 232 openings | see below |
| E6, case 6 | ε 0.500000, 0.500000 kg: unchanged | held |

**Why each number moved.** The four passes over-condense: each condenses
down to saturation at the gas's current temperature, then its latent heat
warms the gas. They therefore condense about five times what the
equilibrium solve does: case 2's rain-out, 3.6e-4 against 6.9e-5 kg; the
outward breaker's first step, 1.29 against 0.25 kg. On the outward breaker,
that excess latent heat warmed the gas to 304.07 K and held the difference
at 3730 Pa. A second 8.25 kg vent followed, and the gas then cooled down to
3240.59 Pa. With the solve, the gas stays at 303.03 K and one 0.87 kg vent
reaches 3395 Pa. The warmer pool then warms the gas, and the breaker holds
at its set point, as the inward case does. **The old agreement of the
outward case with the dry-gas `(γ − 1)` estimate (3240) came from that
over-condensation, not from the overshoot predicted.** A saturated gas
expands with condensation, and the estimate does not apply to it. The 3200
to 3401 Pa assertion was not moved, and it holds for both.

**What to quote:** the new default for the wet well's own numbers. The
ablation is quoted only for the port check against `sembawang`.

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
- ~~The rain-out is sembawang's four passes, which amplify round-off at
  saturation to about 1e-4 in steam mass (gh:#1118, above). The control
  volume's equilibrium rain-out does not.~~ **CORRECTED 2026-11-02:** the
  default rain-out is the control volume's equilibrium solve. The four
  passes, which amplify round-off at saturation to about 1e-4 in steam
  mass, are the ablation `without_equilibrium_rain_out`.
- Moving sembawang onto this component is a separate decision, because its
  fixture and pins would move.
