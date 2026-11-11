# Two-region non-equilibrium control volume (gh:#1092)

> Research, education and V&V only. AI draft, unreviewed.

Part of the MELCOR-style control-volume epic (gh:#1091). MELCOR 2.2's CVH
package (CVH/FL reference manual §2 and §5, cited by section and equation,
not quoted; no MELCOR source used) describes a volume holding a pool and an
atmosphere at separate temperatures, with the level taken from a
volume–altitude table and a swollen level (§5.1.3, Eqs 5-26 to 5-30). This is
a clean-room equivalent, built by generalising the wet well (gh:#1075).
Dated 2026-11-01.

## What it does

`tampines::control_volume::ControlVolume`:

- **Pool**: liquid water, mass and internal energy. IF97 region 1 by default
  (energy on the saturation line, density at the atmosphere's pressure);
  `sembawang`'s constant `c_p = 4186 J/(kg K)` is the ablation.
- **Atmosphere**: a `tampines_steam_tables::gas_mixture::GasMixture` of steam
  and N2, H2, O2, CO, CO2, Ar, closed by `GasMixtureModel` (gh:#1093; IF97
  steam by default). It has its own temperature.
- **Level**: `VolumeAltitudeTable`, piecewise-linear `V(z)` with an exact
  inverse, so a non-prismatic vessel has the right level. The atmosphere
  fills the vessel less the collapsed pool (bubbles belong to the
  atmosphere's inventory).
- **Swollen level**: `swollen_level` / `swollen_level_from_vapour_flow`,
  ported from `sembawang::bwr_mark1::swollen_level`. The vapour is what rose
  through the pool in the last step: uncondensed bubbles from submerged
  ports and flashed vapour. With the swell on (the default), a port below
  the swollen level draws liquid. `without_level_swell()` makes ports use
  the collapsed level.
- **Ports at elevations**: each port has a source (water at an enthalpy,
  plus non-condensables at a temperature) and an outflow. The outflow draws
  pool liquid below the level and the atmosphere's mixture above it (as
  `VolumePort::AtHeight`, gh:#1073). The pressure at a port is
  `p_atm + ρ g (L − z)` below the level and `p_atm` above it.
- **Inflow below the pool**: a two-phase source is split by IF97's saturation
  enthalpies at the port's pressure. The liquid joins the pool; the vapour
  and gases pass through MELCOR's bubble model with the bubble efficiency
  (CVH/FL RM Eqs 6-1 to 6-3).
- **Pool surface**: MELCOR's exchange (§5.1). Flashing of a superheated
  pool. Rain-out of a supersaturated atmosphere, solved to saturation, with
  `sembawang`'s four passes as the ablation.
- **Also**: pool heat, the immersed heat exchanger, and the pool's
  compression work on the atmosphere.
- **Bookkeeping**: water, each non-condensable and energy across the
  boundary. `step` returns a `PortTransfer` for each port.

Searched first:
- The wet well (`components/wet_well.rs`) already ported the surface, the
  bubbles, flashing and rain-out. Its free functions `exchange`,
  `bubble_efficiency`, `t_sat`, `p_sat_pa` and `steam_h` were **moved
  verbatim** to `control_volume/pool_surface.rs` and are re-exported by the
  wet well, so its API and numbers are unchanged.
- Its methods (`inject`, `bubble_through`, `surface`) were re-ported here
  for several non-condensables.
- The wet well's own N2 `c_v(T)` copy now forwards to
  `tampines_steam_tables::gas_mixture::melcor_ncg`, which has the same
  operation order.
- `sembawang::bwr_mark1::swollen_level` is public. It is ported with the
  vessel passed in as closures and checked bit for bit on `sembawang`'s side.

### Several non-condensables at the surface (limit)

MELCOR's surface correlations are ported for steam plus one gas with N2's
properties, and the volume passes the total non-condensable moles as that
gas:

- **Enters**: every gas's mole fraction, through the steam's partial
  pressure and the diffusion driving force.
- **N2's values are used for**: the diffusivity, the viscosity, the
  conductivity, the density and `c_p`.
- **Consequence**: an H2-rich atmosphere's buoyancy and diffusivity are
  misstated. `sembawang`'s original carries H2's own properties; porting
  them is a follow-up.

## Predictions (written before the first run, 2026-11-01)

In `tests/control_volume.rs` and `crates/sembawang/tests/swollen_level_vs_tampines.rs`:

1. **Closure** (two-piece vessel, 300 s): steam below the pool, H2 + N2
   through a second submerged port, a bottom drain and a top vent. Water and
   each gas close to < 1e-12 of the inventory, and energy to < 1e-10 of
   `E_in`. The pool heats 4–8 K and the pressure rises 5–15 %.
2. **Wet well as a special case**: the Unit 1 torus with N2 only, the same
   inputs and the wet well's models. Agreement to < 1e-9 relative, with
   round-off from Dalton's sum, the energy-to-temperature solve and the
   table's area.
3. **Swollen level**:
   - against `sembawang`: bit-identical on its BWR vessel (64 cases);
   - on a prismatic vessel: the hand value `z/(1 − α)` to 1e-12;
   - a port between the collapsed and the swollen level draws liquid with
     the swell and atmosphere without it.
4. **Non-prismatic level**: the table `(0,0), (1,2), (3,12), (4,20)` with
   7 m³ of pool gives `2.000 m` by hand. The volume's level equals the hand
   interpolation of its pool volume to 1e-12 m.
5. **Rain-out**: 1.5 times saturation at 350 K ends at RH = 1 within 1e-3,
   and mass and energy are conserved. The four-pass ablation ends below
   RH 0.99.

## Results (2026-11-01, release, Rust 1.99.1)

| Case | Measured | Prediction |
|---|---|---|
| 1 water / N2 / H2 closure | 1.4e-14 / 1.9e-14 / 1.9e-16 relative | < 1e-12 ✓ |
| 1 energy closure | −7.1e-16 of `E_in` (4.17e8 J) | < 1e-10 ✓ |
| 1 pool heating / pressure rise | +6.31 K / +8.64 % | 4–8 K / 5–15 % ✓ |
| 2 first 5 steps | 6.5e-15 relative (worst quantity) | round-off ✓ |
| 2 over 600 s | T_gas 2.7e-5, p 2.3e-5, steam 1.05e-4, T_pool 9.2e-11, pool mass 4.9e-9, level 3.8e-9 | < 1e-9 **✗ MISS** |
| 2 the wet well against itself, gas 1e-12 K warmer | T_gas 2.9e-5, p 2.5e-5, steam 1.14e-4, T_pool 9.5e-11, pool mass 5.3e-9, level 4.1e-9 | (control, added after the miss) |
| 3 vs `sembawang` | 64 of 64 bit-identical | bit-identical ✓ |
| 3 hand swell | α 0.0308, swollen 2.06346501 m, matching the hand value to 1e-15 m; the draw flips with the ablation | ✓ |
| 4 non-prismatic | 2.0000000009 m, matching the hand interpolation exactly | ✓ |
| 5 rain-out | RH 1.493 → 0.99997; ΔW = ΔE = 0 exactly | ✓ |
| 5 four passes (ablation) | RH 0.006, gas at 601 K | < 0.99 ✓ |

**Prediction 2 missed, and the reason was found.**

- **What happened**: the control volume and the wet well agree to
  round-off for the first steps. From step 7, once the gas first reaches
  saturation, the difference grows by about 2.5 times per step, then
  saturates near 1e-4 in the steam mass.
- **The control that explains it**: the wet well started with its gas
  warmer by 3e-15 relative differs from itself by the same amount.
- **So**: the wet well amplifies any round-off difference. ~~Its explicit
  surface step and rain-out at 1 s act on a gas held at saturation.~~
  **CORRECTED 2026-11-02 (gh:#1118, measured by
  `tests/control_volume.rs::wet_well_round_off_amplification_gh1118`):**
  the amplifier is `sembawang`'s four-pass rain-out alone. The surface
  step is not the cause, and neither is the 1 s step. The sensible-only
  surface amplifies the same (9.9e-5). Shortening the step only scales the
  bound with `dt` (1.1e-4 at 1 s, 1.2e-5 at 0.1 s). The control volume
  with the equilibrium rain-out (its default) stays at round-off, 1.2e-13,
  at 1, 0.5 and 0.25 s. The four passes leave the latent heat in the gas,
  which raises the saturated capacity by more than the mass condensed
  (about 14 times at 330 K in this gas space). The pass therefore ends far
  subsaturated, the next step's evaporation refills the gap, and the map
  from one step's excess vapour to the next has a gain above one. The
  control volume reproduces it to the wet well's own reproducibility, and
  no better is possible while both use the four passes.
- **What the test asserts**: that explanation (round-off for 5 steps, and
  no more than 10 times the wet well's self-sensitivity), not the original
  criterion. The original criterion stays recorded here as missed.
- **Follow-up**: ~~the amplification is a property of the wet well's
  explicit step.~~ **CORRECTED 2026-11-02 (gh:#1118):** it is a property of
  the four-pass rain-out, which the wet well keeps for its bit-for-bit pin
  against `sembawang`'s `Containment`. Both runs stay physical and end at
  the same state to 1e-4. **UPDATE 2026-11-02 (maintainer decision):** the
  wet well's default is now the equilibrium rain-out, with the four passes
  as its ablation. With both models at their defaults, the special case
  agrees to 6.1e-11 at worst (steam mass), which meets prediction 2's
  < 1e-9
  (`tests/control_volume.rs::the_wet_well_is_a_special_case_at_both_defaults`).
  The table row above stays as measured, on the four passes.

**What to quote:**

- Closure: round-off.
- Wet well equivalence: ~~"to round-off before saturation, and within the
  wet well's own 1e-4 round-off sensitivity after it"~~ **since 2026-11-02**
  "to 6.1e-11 at both defaults (equilibrium rain-out)". The four-pass
  figure applies only to the ablation.
- Swollen level: bit-identical to `sembawang`.

## For gh:#1094 (flow paths) and gh:#1095 (implicit network)

A flow path reads the following at each end:

- `pressure_at_elevation(z)`;
- `port_donor(i)`: what is drawn (liquid or atmosphere), with its density,
  enthalpy, temperature and composition.

It computes the flows and drives each volume with one `PortInput` for each
port: `outflow` for flow leaving, and `inflow = Source` built from the donor
for flow arriving. The receiving volume should be given the donor's actual
`PortTransfer` (water, gases, energy out), not the network's estimate, so
the network conserves what each volume conserves.

An implicit scheme can linearise by stepping clones. The wet well's vacuum
breaker becomes one such flow path.

## Not modelled

- kinetic energy and gravity in the energy equation;
- gravity in the atmosphere;
- the void's effect on what a port draws (it draws liquid at the pool's
  state);
- fog;
- ~~pool stratification;~~ **CORRECTED 2026-11-01 (gh:#1100):** an optional
  layered pool is now modelled (see "Layered pool and dry-out" below); the
  default is still one well-mixed pool;
- heat structures (the caller supplies `pool_heat`);
- aerosols;
- the liquid energy's pressure dependence.

## Layered pool and dry-out (gh:#1100, 2026-11-01)

> Research, education and V&V only. AI draft, unreviewed.

### What was added

**Layered pool** (`ControlVolume::with_pool_layers(n)`, a configuration
choice; `n = 1`, the default, runs the single-region code path unchanged):

- The pool is `n` horizontal layers, bottom first, each with its own mass and
  internal energy. The total (mass, energy) is still the state the level,
  the density, the swell and the atmosphere's volume are taken from
  (Boussinesq: layer density differences set stability, not the level).
- **Where things go.** Liquid and condensing bubbles entering at a port join
  the layer at the port's elevation (by the mass below that elevation at the
  pool's mean density; the top layer above the level). The bubble efficiency
  and the bubbles' equilibrium use that layer's temperature. A liquid
  outflow draws from the port's layer, then the layers above it, then those
  below; each gives its share of its energy and the flow work comes from the
  remaining pool by mass. ~~The surface exchange, flashing and rain-out act on
  the **top** layer (its temperature, its mass for MELCOR's Eq 5-17 limit,
  its thickness as the pool-side depth).~~ **CORRECTED 2026-11-01:** the
  surface exchange and flashing act on the surface mixed block (section
  "The surface sees the surface mixed block" below; the top layer alone made
  the exchange depend on the layer count, L9); rain-out joins the top layer. Pool heat, the immersed heat
  exchanger (per layer, implicit in each layer's temperature) and the
  compression work are shared by mass. A port donor's liquid state is the
  layer at the port.
- **Re-layering** (`layers::split_merge`): the layers are Lagrangian; when one
  holds more than twice the mean mass, or one is empty, the adjacent pair with
  the closest specific energies is merged and the heaviest layer is split in
  half (DYRESM's split/amalgamate Lagrangian layers, Imberger et al. 1978,
  not re-checked: not in the corpus). A split smears nothing.
- **Unstable stratification mixes at once** (`layers::convective_adjustment`):
  ported from `sembawang::bwr_mark1::containment::Containment::layer_pool`
  (two layers, mixed whenever the deep one is the lighter; after PPOOLEX and
  Li and Kudinov), generalised to `n` layers by the complete convective
  adjustment of a column (Rahmstorf 1993, not re-checked), with "lighter"
  decided by IF97 density at the atmosphere's pressure, not by temperature
  (so water below 4 °C stratifies correctly). Applied after the outflows and
  again at the end of the step.
- **Stable stratification conducts** (`layers::conduct`, ON by default,
  `without_pool_layer_conduction()` the ablation): molecular conduction
  `k A / Δz` between neighbouring layers, `k` MELCOR's liquid-water table
  (NCG/H2O RM §4.1.1, the one the surface uses), `A` the cross-section at the
  interface, `Δz` between the layers' mid-mass elevations, backward Euler by
  the tridiagonal algorithm, the interface energies applied antisymmetrically
  (energy to round-off). **No eddy diffusivity**: a stable, quiescent
  stratification damps turbulence, so conduction is the physically derived
  lower bound of the exchange; stirring by vents and plumes needs a model of
  the stirring, which is not here (stated). The surface temperature this
  gives is therefore an upper bound.

**Dry-out** (always on; no switch):

- A pool may hold zero mass: at construction, and by draining, flashing,
  evaporation or boiling. Removals that would leave less than 1e-9 of the
  pool (or of a layer) take all of it, so an empty pool is exactly
  `(0 kg, 0 J)` rather than a round-off residue with a meaningless
  temperature; a drained pool's last flow work comes from the atmosphere
  (the gas pushes the last liquid out), and an end-of-step sweep returns any
  energy the compression work left on an empty pool (and any pool below
  1e-9 kg) to the atmosphere. These are numerical floors, not physical
  coefficients.
- A dry pool has no surface exchange or flashing; rain-out, liquid inflow and
  condensate refill it. Its ports draw the atmosphere (the level is the
  vessel bottom). The immersed heat exchanger is uncovered (no heat). Pool
  heat into a dry pool goes to the atmosphere, as does pool heat larger than
  what turns the whole pool into saturated vapour (the pool then boils dry
  within the step); both are reported as
  `ControlVolumeStepReport::pool_heat_to_atmosphere`.
- A liquid outflow larger than the pool now takes the pool and reports what
  moved (`PortTransfer::water_out`), instead of failing; the network already
  passes what moved to the receiver.

### Predictions (written 2026-11-01 before the first run)

In `tests/control_volume_layers.rs`:

- **L1, N = 1 bit identity.** Two 200 s transients on the closure vessel
  (steam and H2/N2 below the pool, drain, vent, pool heat, heat exchanger,
  atmosphere heat and steam sink; the second with every ablation): the pool
  and gas temperatures, the pool mass, pressure, steam mass, total energy,
  collapsed and swollen levels at 50, 100, 150 and 200 s (64 values) are
  **bit-identical** (`to_bits`) to the values captured from the code
  **before** this change, both by default and with `with_pool_layers(1)`.
- **L2, closure with 8 layers**, the same transient (default mechanisms):
  water and each gas close to < 1e-12 of the inventory, energy to < 1e-10 of
  `E_in`, the layers sum to the totals to < 1e-12. The steam enters near the
  bottom and the surface cools the top, both unstable, so the pool stays
  nearly mixed: the layer spread at 300 s < 0.05 K, the mean pool
  temperature within 0.05 K of the one-layer run.
- **L3, conduction against the analytic solution.** A 1 m² prismatic pool
  1 m deep, 40 layers, the lower half at 300 K and the upper at 310 K,
  constant-`c_p` liquid, surface exchange and compression work ablated (so
  the column is insulated), 2e5 s at 200 s steps. Reference: the Fourier
  cosine series for an insulated slab with a step initial condition,
  averaged over each layer, with `α = k/(ρ c_p)` at the mean 305 K
  (`k` = 0.6194 W/(m K) from the table, `ρ` ≈ 995.0, `c_p` 4186:
  `α` ≈ 1.487e-7 m²/s, diffusion length `√(α t)` ≈ 0.17 m). Prediction: the
  largest layer error < 0.05 K (0.5 % of the 10 K step), expected near
  0.01–0.02 K (the second-order spatial error and `k(T)` varying ±1 % across
  the step); the two layers at the interface within 0.05 K of 305 K
  symmetric about it; energy to < 1e-12 relative; no mixing (the column
  stays stable). The ablation `without_pool_layer_conduction()` leaves the
  step bit for bit.
- **L4, unstable mixes.** 10 layers, the lower half at 320 K and the upper at
  300 K, one step: every layer at the same temperature (spread < 1e-9 K),
  about 310 K, energy to round-off. And by density, not temperature: 275 K
  water over 281 K water (denser above, though colder) mixes in one step.
- **L5, stable hot inflow at the top.** A 10 m² pool 1 m deep at 300 K, 10
  layers, 2 kg/s of 350 K water through a port 2 cm below the surface for
  300 s (600 kg into about 9,970 kg), default mechanisms: the surface layer
  ends at least 5 K above the bottom layer; the bottom layer within 0.01 K of
  300 K; the mean within 0.2 K of the one-layer run; the one-layer pool's
  surface (its only) temperature at least 3 K below the layered surface
  layer's; water and energy closure as L2.
- **L6, dry-out and refill.** A 1 m² × 3 m vessel, 500 kg at 330 K, a bottom
  drain at 1 kg/s until the pool is dry, then closed, 50 s dry, then 2 kg/s
  of 300 K water through the bottom port for 300 s; with one layer and with
  four. No error; the pool mass is exactly 0.0 while dry and the port draws
  the atmosphere; the drain moves 500 kg less what evaporated (0–2 kg);
  water and energy close to < 1e-12 and < 1e-10 of the throughput; after the
  refill about 600 kg at 300–301 K.
- **L7, dry-out by evaporation, by heat, and refill by rain-out.** (a) 1 kg
  at 340 K under 2 m³ of dry N2 at 450 K evaporates within the run (the
  surface exchange's own rate), with no error and closure as L6; (b) a
  0.1 kg pool given 1 MW boils dry in one step, the heat reported to the
  atmosphere, energy conserved; (c) a volume built with an empty pool and a
  supersaturated atmosphere rains out into it: pool mass > 0 after one step,
  water and energy to round-off; pool heat into a dry pool reaches the
  atmosphere.

### Changes made after the first run, and the predictions for the re-run (written before it)

The first run (2026-11-01) held L1, L3, L5 and L6 and missed parts of L2, L4
and L7 (the misses are in the results below). Two things changed before the
re-run, both recorded here because they came after seeing results:

1. **Thin layers are now amalgamated.** In L6's four-layer refill the top
   layer ended at 319.4 K over 300.0 K water: the split/merge rule as first
   written merged only to relieve a thick layer, so a layer could become
   arbitrarily thin, and the warm humid gas condensing on a thin top layer
   heated it as far as its (tiny) mass allowed. A layer's thickness is the
   model's resolution, not physics, so the rule now also merges a layer
   lighter than half the mean with its closer neighbour (DYRESM's
   amalgamation of thin layers). **Prediction for the re-run:** L6's
   four-layer refill ends with every layer within 2 K of the others; L1, L3
   and L5 unchanged (their layers never become thin: L1 has none, L3's
   masses never change, L5's top layer only grows).
2. **L7 (a) re-designed**: 1 kg in a closed 3 m³ volume cannot dry (the gas
   saturates). **Prediction:** 0.05 kg at 340 K under 3 m³ of dry N2 at
   450 K dries within 3000 s, the gas cooling 60–80 K (1.2e5 J of latent
   heat over 80 mol of N2 at `c_v` 20.8 J/(mol K)), closure to round-off.
- **L8, a volume drains dry inside a network** (written before its first
  run, after the L1–L7 re-run; in `src/cv_network/tests.rs`). Two 1 m² × 5 m
  vessels joined bottom to bottom (ports at 0 m, 0.002 m², `K` = 1): A holds
  0.2 m of water under 3 bar of N2, B holds **no** water under 1 bar (built
  with a zero pool, which before gh:#1100 was refused). 20 s at 0.02 s.
  Prediction: no error; A's gas pushes its water into B and A dries (pool
  exactly 0.0 kg) within 10 s, after which its port draws gas and the gas
  bubbles up through B's new pool; the network's water, N2 and energy
  conserved to < 1e-12 relative; B ends with 199–200 kg; the gas-space
  pressures settle with A above B by B's pool head at the port,
  `ρ g h` ≈ 1960 Pa (within 300 Pa).

### Results (2026-11-01, release, Rust 1.99.1)

Two more defects were found by these cases **after the first run** and
fixed before the numbers below (both in a 0.05 kg film split into three
layers, L7 a): (i) `layers::conduct` computed each interface's energy as
`Δt G (T_k' − T_{k+1}')`, a huge conductance times a round-off temperature
difference when layers are thin, and gave a 1e-9 kg pool 7e5 J; it now
takes the energy the layers below gave up, `−Σ C (T' − T)`, and bounds
`G Δt` at 1e6 times the smaller heat capacity (beyond which the pair relaxes
within `e^-1e6` of a step; without the bound the tridiagonal system is
singular in floating point); (ii) MELCOR's Eq 5-17 limit caps the pool's
heat by the whole pool's sensible heat while the evaporated mass leaves at
the pool's enthalpy, so a film that is nearly all evaporating was left
colder than the triple point (221 K, then a negative energy); such an
evaporation now takes the surface liquid whole. Both act only near dry-out;
L1 confirms neither touches the one-layer path.

| Case | Measured | Prediction |
|---|---|---|
| L1 one layer vs pre-change code | 64 of 64 values bit-identical, default and ablated, with and without `with_pool_layers(1)` (256 comparisons) | bit-identical ✓ |
| L2 closure, 8 layers | water 1.0e-14, gas 1.3e-14, energy 7.5e-16 of `E_in`; layer sums 9.5e-15 (mass), 4.5e-15 (energy) | < 1e-12 / 1e-10 / 1e-12 ✓ |
| L2 mean pool T | 304.6649 K vs 304.6568 K one layer (+0.008 K) | within 0.05 K ✓ |
| L2 layer spread | **4.76 K** | < 0.05 K **✗ MISS** (below) |
| L3 worst / rms layer error vs analytic slab | 1.10e-2 K / 6.7e-3 K (0.11 % of the 10 K step), `α` 1.487e-7 m²/s, `√(αt)` 0.172 m | < 0.05 K, expected 0.01–0.02 ✓ |
| L3 interface layers | 304.8050 / 305.2144 K (analytic 304.7958 / 305.2042), mean 305.0097 K | within 0.05 K of 305 K ✓ |
| L3 energy; stability; ablation | 0 (exact); column monotone; without conduction the step bit for bit | ✓ |
| L4 320 K under 300 K | one temperature, 309.9987 K, spread 1.1e-13 K, energy 1.9e-16 | ✓ |
| L4 275 K over 281 K | bottom eight layers mix to 278.75 K, top two stay 275.00 K, a stable column by IF97 density | "mixes" **✗ MISS** (below) |
| L4 277 K under 283 K | not mixed (spread 6.0 K) | (control) ✓ |
| L5 surface vs bottom layer | 318.71 K vs 299.9985 K | ≥ 5 K apart ✓ |
| L5 bottom layer | 299.9985 K (−0.0015 K, the pool surface's cooling reaching the mixed layers below the inflow is nil; the heat exchanger is off) | within 0.01 K of 300 K ✓ |
| L5 mean vs one layer; one-layer surface | 302.8279 vs 302.8383 K (−0.010 K); the one-layer surface 15.9 K below the layered one | within 0.2 K; ≥ 3 K ✓ |
| L5 closure | water 0, energy 2.5e-14 | ✓ |
| L6 one layer | dry at 500 s, pool exactly 0.0; drained 499.944 kg (0.056 kg evaporated); refilled to 600.040 kg at 300.064 K; water 3.8e-16, energy 2.5e-16 | ✓ |
| L6 four layers (first run) | top layer 319.38 K over 300.01 K | (prompted the thin-layer rule) |
| L6 four layers (re-run) | dry at 500 s; drained 499.942 kg; refilled to 600.042 kg at 300.067 K, layers 300.010 / 300.010 / 300.010 / 300.445 K (spread 0.44 K); water 1.3e-15, energy 1.9e-15 | spread < 2 K ✓ |
| L7 a, first design (1 kg) | not dry after 20,000 s: the gas saturated at 312.8 K, RH 0.997 | **✗ MISS** (design) |
| L7 a, 0.05 kg | dry at 98 s (one layer) and 119 s (three); gas 450 → 381.8 K (−68.2 K); water ≤ 6.9e-16, energy ≤ 1.1e-15 of `E0` | dry within 3000 s, −60 to −80 K ✓ |
| L7 b | 0.1 kg boiled dry by 1 MW in one step, 1.0e6 J reported to the atmosphere, energy residual −2.3e-10 J; then 1 kW into the dry pool reaches the atmosphere | ✓ |
| L7 c | an empty pool under RH 1.49 at 350 K: 0.01888 kg rained out, the pool at 349.9995 K, RH 0.99997, ΔW 0, ΔE −4.7e-10 J (one and four layers identical) | ✓ |
| L8 network: A dry | 5.46 s, pool exactly 0.0 | within 10 s ✓ |
| L8 network conservation | water 2.0e-15, N2 3.3e-16, energy 1.6e-15 (worst over 1000 steps) | < 1e-12 ✓ |
| L8 end state | A refilled to 121.1 kg, B 72.6 kg, the gases 31 kPa apart, still moving | B 190–195 kg, settled at the head **✗ MISS** (below) |

**The misses, and what they mean.**

- **L2 spread**: the steam port (0.3 m) lies in the second layer (the bottom
  layer ends at 0.19 m), so the water below the injection is not stirred and
  stays 4.76 K colder while everything above mixes. That is the physics the
  layering exists for (`sembawang`'s deep layer under the vents); the
  prediction overlooked the port's elevation. The test asserts the
  explanation (the layers at and above the injection within 0.05 K, the
  bottom colder).
- **L4 anomaly**: the merged block (5.6 °C) is near the density maximum
  (about 4 °C), so the remaining 1.85 °C water is then the lighter and
  floats: cold water over 4 °C water, as in a winter lake. Correct physics;
  the test asserts static stability by IF97 density.
- **L7 a**: a closed 3 m³ volume cannot hold 1 kg of vapour at the
  temperatures evaporation leaves it at; the case, not the volume, was wrong.
- **L8**: two gas springs and the path's inertia make a U-tube that the
  path's `K = 1` damps only slowly, so the water swung back and A refilled
  after drying, without error and conserving. The first L8 run at 300 K
  stopped on two limits outside this change: the HEM liquid choking
  (~~`Iapws97Hem::break_flow` refuses 300 K liquid from 2.31 to 1.50 bar, and
  from 2.0 to 1.5 bar or 1.2 to 1.0 bar when called directly; it succeeds at
  3 to 1 bar~~ **CORRECTED 2026-11-02, gh:#1102**: that refusal was fixed, and
  `break_flow` now returns the unchoked flux, Bernoulli to 6.7e-5, for all
  four cases (`tampines-steam-tables`
  `hem::tests::gh1102_cold_liquid_low_dp_break_flow_is_unchoked_bernoulli`).
  L8 has not been re-run with choking on. It was a defect of the break flow
  for cold subcooled liquid at low pressure, reported with gh:#1100), and
  IF97 steam's 273.16 K floor (gh:#1098). L8 ablates choking (as the manometer test does) and runs at
  350 K.

**What to quote:** one layer is the old volume bit for bit (L1); with
layers, conservation to round-off (L2, L5, L6, L7, L8); conduction across a
stable column within 0.011 K of the analytic slab after 55 h (L3); an
unstable column mixes at once by density (L4); a pool drains, boils or
evaporates dry and refills, conserving to round-off (L6–L8).

## Atmosphere reactions: H2 and CO combustion at caller-set rates (gh:#1100, 2026-11-01)

> Research, education and V&V only. AI draft, unreviewed.

> ~~Everything in this section is `tampines` code.~~ **CORRECTED
> 2026-11-01 (gh:#1103):** the atmosphere reactions are **no longer in
> `tampines`**. `AtmosphereReactions`, `ControlVolumeInputs::
> atmosphere_reactions`, `CvNetwork::set_atmosphere_reactions`,
> `CvNetwork::reaction_totals`, `ControlVolume::reaction_energy_per_mole`,
> the report's and bookkeeping's `hydrogen_burned`,
> `carbon_monoxide_burned` and `reaction_heat`, and `control_volume/
> reaction.rs` were removed; the derivation, constants and anchoring are
> `sembawang::bwr_mark1::reaction`, and `tests/control_volume_reactions.rs`
> is `crates/sembawang/tests/reaction_in_a_control_volume.rs` (R1–R6
> re-run there; results in `sembawang`'s
> `verification_and_validation/bwr_mark1/containment_network.md`). The
> record below is kept as it was run; every API name in it is historical.
> What replaced it here is the species source and sink (section "The
> reactions move to `sembawang`" below).

### What was added

`ControlVolumeInputs::atmosphere_reactions: AtmosphereReactions` (two
`MolarFlowRate`s, mol/s of fuel: `hydrogen_burn` for `H2 + ½ O2 → H2O(g)`,
`carbon_monoxide_burn` for `CO + ½ O2 → CO2`), applied inside
`ControlVolume::step` after the atmosphere heat and steam sink (step 2c), and
`CvNetwork::set_atmosphere_reactions(volume, reactions)` (held until changed,
as `set_atmosphere_steam_sink`). The burn criteria and rates are the
caller's (`sembawang`, MELCOR BUR RM §2); this side closes the stoichiometry
and the energy. The step's extents are capped by the fuel present and then,
together, by the oxygen (both scaled by one factor when oxygen limits); a
limited reactant ends exactly 0. `ControlVolumeStepReport` carries
`hydrogen_burned`, `carbon_monoxide_burned` and `reaction_heat`;
`ControlVolumeBookkeeping` their running totals; `CvNetwork::reaction_totals`
the network's.

Searched first: no reaction, combustion or formation-enthalpy code in
`tampines` or `tampines-steam-tables` (`grep -riE
"formation|combust|heat of|burn"` over both `src/`; the hits are pebble-bed
burnup, OpenFOAM's `hConst`/`hPolynomial` formation-enthalpy *field* `Hf`
with no species data, and the gas mixture itself). Elsewhere,
`dover::species::Species::enthalpy_formation` holds NIST-JANAF values for
steam, CO and CO2 rounded to 0.1 kJ/mol (−241.8, −110.5, −393.5); checked
and not reused: `tampines` does not depend on `dover`, and the rounding
alone is 1.1e-4 of the H2 heat, against a 1e-3 gate that should be met with
margin; the full-precision JANAF values are used here instead;
`gas_mixture::nasa7` keeps only the five `c_p` coefficients of GRI-Mech 3.0,
not the sixth (the enthalpy constant `a5`, which carries GRI's formation
enthalpy), and steam is IF97, not NASA-7, so the a5 route cannot serve the
steam; the formation enthalpies are taken from NIST-JANAF instead, and GRI's
`a5` is used only in the independent hand calculation (R2).

### The energy-reference derivation

Each species' energy is on its own reference: non-condensables from 0 K by
`sembawang`'s convention (below each fit's lower bound `u = c_v(T_low) T`),
steam on IF97 (triple-point liquid). With `h_i^code(T)` the code's molar
enthalpy (`u_i + R T`; steam `M_H2O h_IF97(T, 1 Pa)`, the ideal-gas limit),
every species is anchored to its NIST-JANAF `Δ_f H°(298.15 K)` (Chase 1998:
H2O(g) −241.826, CO −110.527, CO2 −393.522 kJ/mol, elements 0):

```text
c_i = Δ_f H_i°(298.15) − h_i^code(298.15),    h̃_i = h_i^code + c_i
Q   = −Σ ν_i c_i ξ        (added to the atmosphere's stored energy)
```

`c_i` shifts `u` and `h` alike, so `Σ n_i (u_i^code + c_i)` is the conserved
absolute energy and `Q` is what the stored energy gains. It is a constant per
mole (`ControlVolume::reaction_energy_per_mole`). 298.15 K, not 0 K, is the
anchor because the code's "from 0 K" values are an extrapolation below each
fit, not JANAF's `H°(T) − H°(0)` (numbers in the results).

### Predictions (written 2026-11-01 before the first run)

- **R1, heat of reaction at 298.15 K vs JANAF.** Burn a small amount (so the
  steam stays below saturation) in a dry 10 m³ volume of H2 + O2 at
  298.15 K in one step; the heat to remove to bring the products back to
  298.15 K at constant volume, plus `V Δp`, per mole, is `−Δ_r H(298.15)`.
  Predicted 241.826 kJ/mol for H2 (JANAF LHV) and 282.995 kJ/mol for CO
  (JANAF; 282.98 is the rounded value in the issue, 0.005 % away), both
  within 0.01 %: by construction the anchor is exact, and only the IF97
  steam's real-gas residual enthalpy at its partial pressure (about 500 Pa:
  `(B − T dB/dT) p`, a few J/mol, about 1e-5 of the heat) differs from the
  ideal gas. Gate: < 0.1 %.
- **R2, adiabatic constant-volume complete burn.** 1 m³, dry, 8 % H2 in air
  (O2 : N2 = 21 : 79), 300 K, 1 bar (40.09 mol: 3.21 H2, 7.74 O2, 29.15
  N2), burned to completion. Hand estimate: `Q ≈ 3.21 × 240.6 kJ ≈ 772 kJ`
  over a mean `C_v ≈ 891 J/K` gives `T_end ≈ 1170 ± 50 K` and
  `p_end = n R T/V ≈ 3.73 ± 0.2 bar`. Against an independent calculation
  written in the test (GRI-Mech 3.0 NASA-7 with its own enthalpy constants
  for H2, O2, N2 and H2O, ideal gases, bisection on `U(T)`), predicted to
  agree within 1 % of the temperature rise and 1 % on the pressure, the
  difference carried by MELCOR's N2 `c_v` against NASA-7's (about 1 % near
  1000 K) and IF97 against NASA-7 steam. Gate: 1 %.
- **R3, limiting reactant.** 10 mol H2 with 2 mol O2 asked to burn 100 mol
  in one step: O2 ends exactly 0, H2 at 6 mol and the steam up by 4 mol to
  1e-14; asked to burn more H2 than present with excess O2, H2 ends exactly
  0; both fuels O2-starved burn in the asked ratio.
- **R4, elements.** H, O, C, N (and Ar) over a 60-step transient with both
  reactions, inflows, outflows and a pool: each element's inventory less
  what crossed the boundary closes to < 1e-12 relative; energy closes with
  `reaction_heat` to < 1e-10 of the energy in.
- **R5, zero reaction is the old code bit for bit.** A 60-step transient
  (pool, steam, H2, O2, CO, CO2, Ar, N2, inflow, drain, vent, atmosphere heat
  and sink) taken on the code before the change: every value `to_bits`
  identical with the reaction input absent, and set explicitly to zero.

### Results (2026-11-01, release, Rust 1.99.1; `tests/control_volume_reactions.rs`)

| Case | Predicted | Measured | |
|---|---|---|---|
| R1 H2, `−Δ_r H(298.15)` | 241.826 kJ/mol within 0.01 % | **241.8296 kJ/mol, +1.48e-5** (the gas reached 779 K in the step) | as predicted |
| R1 CO, `−Δ_r H(298.15)` | 282.995 kJ/mol within 0.01 % | **282.9950 kJ/mol, 0.0** (no steam, no real-gas term); +5.3e-5 against the issue's rounded 282.98 | as predicted |
| R2 adiabatic burn, end T | 1170 ± 50 K; within 1 % of the rise of the hand calculation | code **1146.09 K**, hand (GRI-Mech NASA-7) **1149.77 K**: −3.68 K, **−0.43 %** of the 850 K rise | as predicted |
| R2 end p | 3.73 ± 0.2 bar; within 1 % | code **3.6675 bar**, hand **3.6793 bar**: **−0.32 %** | as predicted |
| R2 path independence | (not predicted) | 20 steps and one step end at the same T to 1e-6 K (1146.093695 K both) | |
| R2 energy | (closure) | `E − E0 − Q` = −1.2e-10 J of 8.85e5 J | |
| R3 limiting | O2 exactly 0, H2 6, steam +4 mol to 1e-14 | as predicted; a further step burns nothing; fuel-limited H2 exactly 0 and O2 8.5; both fuels starved burn 3 : 1 as asked | case corrected, below |
| R4 elements | < 1e-12 relative | H +2.9e-15, O +2.9e-15, C +3.9e-16, N −4.4e-15, Ar 8e-18 (90 mol H2 and 19.96 mol CO burned, 3.04e7 J); energy −3.0e-14 of `E_in` | as predicted |
| R5 zero reaction | bit for bit | all 72 values identical (absent, `none()`, explicit zeros) | as predicted |
| R6 network | (added) | the one-volume network burns exactly as the volume alone; water 0, gases < 1e-14 kg, energy 2.3e-10 J of 1.94e6 J against `boundary_inflow + reaction_totals` | |

**Stored-energy release per mole** on the default closure (MELCOR H2 and N2,
NASA-7 O2/CO/CO2, IF97 steam): `Q_H2 = 275 808.3 J/mol`, `Q_CO = 279 844.5
J/mol`. These are not heats of reaction (they include each species' shift
between its code reference and JANAF's); the heats are R1's.

**R2, what carries the −3.7 K.** Re-running the hand calculation with the
code's MELCOR N2 energy in place of GRI's (attribution only; it reuses the
code's N2 fit) gives 1146.05 K, 0.04 K from the code: MELCOR's N2 `c_v`
(about 1 % above NASA-7's near 1000 K) carries the difference; IF97 against
GRI steam and JANAF's against GRI's own formation enthalpy of H2O(g)
(−241.8246 kJ/mol from GRI's `a5`, 1.4 J/mol from JANAF) carry 0.04 K.

**Added checks (not in the written predictions).**
- IF97 to JANAF for water itself: IF97's
  `M_H2O (h_ig(298.15 K, 1 Pa) − h_liq(298.15 K, 1 bar)) = 44.0112 kJ/mol`
  against JANAF's `Δ_f H°(H2O,g) − Δ_f H°(H2O,l) = 44.004 kJ/mol`, **+1.6e-4**
  (7.2 J/mol). IF97's steam and liquid are consistent with the JANAF
  formation data to that level; the reactions use only the vapour anchor.
- The code's enthalpies "from 0 K" at 298.15 K against JANAF's
  `H°(298.15) − H°(0)` (default closure): H2 7571.9 vs 8468 (**−896 J/mol**,
  MELCOR's fit held at `c_v(100 K)` below 100 K, where real H2's rotational
  `c_v` falls), O2 +11.8, N2 +83.4 (MELCOR), CO +13.7, CO2 **+517.6**
  (NASA-7 held at `c_v(200 K)` below 200 K), Ar +0.4; all-NASA-7: H2 −187.1,
  N2 −1.1. **Finding, not a defect for the existing code:** the gas
  mixture's "from 0 K" is `sembawang`'s convention (stated in
  `gas_mixture::nasa7` and `melcor_ncg`), not JANAF's `H°(T) − H°(0)`. It
  is harmless for energy changes above each fit's lower bound, but it rules
  out anchoring formation enthalpies at 0 K (a 0.37 % error on the H2 burn
  from H2 alone); the reactions anchor at 298.15 K for that reason.

**Changes after the first run (case errors, not code).**
- R3, both fuels starved: the first case held 10 mol of each fuel with rates
  30 and 10 mol/s, so the fuel cap made the asked extents 10 : 10 before the
  oxygen scaled them (2 : 2 measured). The volume did what its docs say; the
  case now holds 50 mol of each and burns 3 : 1.
- R4: an assertion (not a written prediction) that more than 25 mol of CO
  burned; only 20 mol was there, and the vent took 0.04 mol of it. Now: H2
  burned 90 mol (1.5 mol/s × 60 s) and CO ran out.

**What to quote:** heat of reaction at 298.15 K within 1.5e-5 of JANAF for
H2 and exact for CO (R1); the adiabatic constant-volume burn of 8 % H2 in
air within 0.43 % of the temperature rise and 0.32 % of the pressure of an
independent GRI-Mech calculation, the difference carried by MELCOR's N2
`c_v` (R2); elements to 5e-15 (R4); zero reaction bit for bit (R5).

## The surface sees the surface mixed block, pool heat at an elevation, the heat to the atmosphere reported (gh:#1100, 2026-11-01)

> Research, education and V&V only. AI draft, unreviewed.

Three needs listed by `sembawang`'s
`verification_and_validation/bwr_mark1/containment_network.md` ("What
`tampines` would need"):

1. **The surface exchange depended on the layer count.** `sembawang`'s case A
   (relief steam into a mixed, layered torus pool) gave network `p_ww` − old
   +341.1, +347.9, +354.2, +360.1, +365.5 Pa at 1, 2, 4, 8, 16 layers. The
   suspected cause, from the code: `ControlVolume::surface_liquid` hands
   `pool_surface::exchange` the **top layer's** mass (MELCOR's Eq 5-17 limit
   on the pool's heat) and its **thickness** as the pool depth `L` in the
   pool-side natural convection (`h_free`, `X = min(D_s, L)`, the
   coefficient `∝ X^-1/4` through `Nu = 0.27 (Gr Pr)^1/4`). A thinner layer
   gives a larger coefficient and more evaporation.
2. Pool heat at an elevation (`set_pool_heat` shares by mass).
3. Each volume's `pool_heat_to_atmosphere` through `NetworkStepReport`.

**What MELCOR uses.** MELCOR's CVH holds one pool per control volume, so it
has no layer to choose: the pool-side length of its horizontal-surface
natural convection (CVH/FL RM §5.1.2, Eqs 5-20, 5-22 to 5-25, `X = min(D_s,
L)`) is that pool's depth, and Eq 5-17's limit is that pool's mass. (Section
and equation numbers as cited by the existing ports, `sembawang`'s
`pool_surface` and this crate's; the manual is private and was not re-read
for this change.) The physical reading for a layered pool is the water that
convects with the surface: the block of layers, from the top down, that is
not stably stratified against the surface water. A neutral or unstable
column below the surface convects as one, and only a stable density step
arrests the surface's convection.

### Predictions (written 2026-11-01, before any run of this change)

**L9, confirmation on the code as it stands (before any edit to
`src/`).** One volume, case A of `sembawang` (750 m² prismatic, 2.34 m of
water at 303.15 K, 2620 m³ of saturated N2 at 105 kPa, 10 kg/s of 2.7 MJ/kg
steam into a bottom port, 600 s at 1 s), `n` = 1, 2, 4, 8, 16: `p` at 600 s
rises monotonically with `n`, by +4 … +8 Pa a doubling (about +24 Pa from 1
to 16, as `sembawang`'s case A), and the surface evaporation rises with it.

**L9, after the fix.** The surface sees the surface mixed block: from the
top holding layer down, every layer whose IF97 density (at the
atmosphere's pressure) is not above the block's by more than 1e-12
relative (a round-off allowance: the convective adjustment hands the
block's top layer the energy remainder, a few ulp; 1e-12 in density is
about 3e-9 K, far below any physical stratification); its mass for Eq
5-17, its depth (level less the height of the water below it) as `L`, its
temperature as the pool's; what the surface takes (evaporation, heat,
flashing) comes from the block's layers by mass. Default ON;
`without_surface_mixed_block()` restores the top layer.

- The column is mixed (the steam enters the bottom layer, which is the
  lightest and mixes the whole column each step), so the block is the
  whole pool: `|p(n) − p(1)|` < 0.5 Pa for `n` = 2 … 16, the evaporated
  mass within 0.1 % of `n` = 1's.
- With `without_surface_mixed_block()` the numbers equal the confirmation
  run's bit for bit.
- **L1 unchanged** (the one-layer path is not touched): 64 values bit for
  bit.
- L2 to L8 keep their asserted bounds. L5 (a stable column heated at the
  top) moves by under 0.5 K: its block is the top layer plus any split half
  at the same energy.

**L10, pool heat at an elevation** (`ControlVolumeInputs::pool_heat_at`, a
list of `(elevation, power)`; `pool_heat` unchanged). A 10 m² pool 1 m deep
at 300 K in 10 layers, surface exchange ablated to sensible-only at zero
coefficient and compression work off (an insulated column), conduction off;
100 kW for 100 s at 0.9 m: all 1e7 J in the top layer (it warms by
`1e7 / (997 kg × 4180)` ≈ 2.4 K; the others bit for bit unchanged); the same
at 0.05 m: the bottom layer warms, is lighter, and the convective
adjustment mixes the whole column to the one-layer temperature within
1e-9 K. One layer: `pool_heat_at` equals `pool_heat` bit for bit. A dry pool:
the heat reaches the atmosphere and is reported, as `pool_heat`.

**N1, the network report** (`NetworkStepReport::volumes`, each volume's
`ControlVolumeStepReport`): a two-volume network, one pool given 1 MW for a
step with 0.1 kg of water: its report's `pool_heat_to_atmosphere` is the
heat less the pool's capacity, the same as the volume reports alone; the
other volume's is zero; every existing network result bit for bit (the
report is a copy, no arithmetic changes).

### Results (2026-11-01, release, Rust 1.99.1)

| Case | Measured | Prediction |
|---|---|---|
| L9 confirmation (unmodified `src/`) | `p` 105684.434, 105691.218, 105697.528, 105703.375, 105708.756 Pa at `n` = 1, 2, 4, 8, 16 (+6.8, +6.3, +5.8, +5.4 Pa a doubling); evaporated 0.663, 0.680, 0.696, 0.711, 0.725 kg | monotone, +4 … +8 Pa a doubling ✓ (the cause confirmed: the top layer's mass and thickness) |
| L9 after the fix, default | `p(n) − p(1)` below 1e-4 Pa at every `n` (105684.4344 Pa printed for all five); evaporated within 1.5e-13 of one layer's | < 0.5 Pa, < 0.1 % ✓ |
| L9 `without_surface_mixed_block` | bit-identical to the confirmation run at every `n` | bit for bit ✓ |
| L1 | 64 of 64 bit-identical, default and ablated | ✓ |
| L2–L8 | all bounds hold; L5's surface layer 318.7051 K (318.71 before), bottom 299.9985 K: its block is one layer | ✓ |
| L10 top (0.95 m) | the top layer +1.000000e7 J (302.4001 K), the other nine bit for bit | ✓ |
| L10 bottom (0.05 m) | the column mixes to within 5.7e-14 K of the one-layer 300.239986 K | < 1e-9 K ✓ |
| L10 band 0.5–1.0 m | layers 5–9 +1.9999e6 J each, layer 4 +327 J, layers 0–3 bit for bit | "shares by mass": held to 1.6e-4 (below) |
| L10 one layer; dry pool; invalid band | `pool_heat_at` = `pool_heat` bit for bit; 1e3 J to the atmosphere, reported; a band with its top below its bottom refused | ✓ |
| N1 network | two reports, in order; A's 0.1 kg film boils dry, 1e6 J to its atmosphere in the network report and alone; B's 0; closure with heat at an elevation 5.6e-16 | ✓ |

**After the run (stated):**

- **L10 band:** the band's 0.5 m maps to the layers through the pool's mean
  density (as a port finds its layer), and the layers' masses come from
  the construction's density, so 1.6e-4 of the band's water, 327 J, lies
  in layer 4. The assertion, first exact for layers 0–4, now holds layers
  0–3 bit for bit and layer 4 under 1e-3 of the band's heat; set after the
  run and labelled so in the test.
- **L4, the 4 °C anomaly, a round-off inversion:** the two 275 K layers at
  the top form one surface block, which now gives up its exchange by mass
  with the top layer taking the remainder; they ended 2e-13 K apart, the
  upper warmer, which below the density maximum is denser by about 1e-17.
  L4's stability check (strict, through a different density call) failed
  on that. It now allows 1e-12 of the density, the allowance the block
  itself uses; **a test tolerance changed after a run, flagged for the
  maintainer.** The physics asserted (static stability, the lower part
  mixed) is unchanged.

~~**A consequence found downstream (stated, open).**~~ **CORRECTED
2026-11-01** (gh:#1107): fixed by the implicit flash (option (b) below,
section "The flash is implicit in the pressure it raises"); the record as
it stood: Flashing acts on the
block too, so a mixed block above saturation flashes as a whole in one
step, as a one-layer pool does. In `sembawang`'s 30 h containment replay
this brought back post-vent `p_ww` swings (up to +12.7 % at the half-hour
samples) and an `n` dependence of the vent window (−17.2 % at 11 layers,
−8.2 % at 22). It is the step-scale relaxation cycle that `sembawang`
recorded for the one-layer pool. The two options are a flash limited to
the step, or flashing on the top layer while the exchange sees the block.
Neither is taken here; both need a decision (record:
`crates/sembawang/verification_and_validation/bwr_mark1/containment_network.md`).

**What to quote:** with layers, a mixed pool's surface exchange no longer
depends on the layer count (L9: 1e-13 against +24 Pa from 1 to 16 layers
before); one layer is untouched (L1); heat at an elevation goes where it
is put and conserves (L10, N1); each volume's report, including
`pool_heat_to_atmosphere`, reaches the network's caller (N1).

## The reactions move to `sembawang`; a species source and sink stays (gh:#1103, 2026-11-01)

> Research, education and V&V only. AI draft, unreviewed.

**Maintainer direction:** `tampines` holds thermal hydraulics, no
chemistry. The atmosphere reactions (`AtmosphereReactions`,
`set_atmosphere_reactions`, `reaction_totals`, the reaction report and
bookkeeping fields and `control_volume/reaction.rs`, commit 3f1261bd5) are
removed here and moved to `sembawang::bwr_mark1::reaction` (with
`tests/control_volume_reactions.rs`, ported). In their place,
`ControlVolumeInputs::atmosphere_species` (`AtmosphereSpeciesRates`, held on
the network by `CvNetwork::set_atmosphere_species`): a mass rate per
species, negative removing, each carried at its own code molar enthalpy at
the **start-of-step** atmosphere (`ControlVolume::
atmosphere_species_molar_enthalpies`, public, so a caller closes its own
energy balance on exactly those values). A removal beyond what the
atmosphere holds (by more than 1e-12 of it) is refused. It counts as a
boundary flow in the volume's bookkeeping and in the network's
`boundary_inflow`.

### Predictions (written 2026-11-01, before the first run)

**S1** (`tests/control_volume.rs`): a 10 m³ vessel, 2 m³ of water at 330 K,
N2 + 10 % H2 + 10 % O2 at 1 bar; for 100 s at 1 s, +0.01 kg/s steam,
+0.02 kg/s CO2, −0.002 kg/s H2, −0.016 kg/s O2: water, every gas and the
energy close against the bookkeeping to < 1e-12 relative; each step's
energy moved equals `Σ (m_i / M_i) h_i` at the start-of-step enthalpies
to 1e-12; an H2 sink of more than the atmosphere holds is refused; zero
rates leave a step bit for bit (L1's 64 values, which use none).

**N1** (extended): a species source and sink on a network volume closes
against `boundary_inflow` to < 1e-12.

### Results (2026-11-01, release, Rust 1.99.1)

| Case | Measured | Prediction |
|---|---|---|
| S1 closure | water 6.8e-13 of the 1 kg added (3e-16 of the inventory), gases 5.9e-16, energy 3.8e-16 of `E0` | < 1e-12 ✓ |
| S1 energy per step | equal to `Σ (m/M) h` at the start-of-step enthalpies exactly (0 difference) | 1e-12 ✓ |
| S1 oversized sink | refused | ✓ |
| L1 (zero rates) | 64 of 64 bit-identical | ✓ |
| N1 species on a network | closure 3.9e-16 against `boundary_inflow` | < 1e-12 ✓ |

**Changed after the first run (case error):** S1's first sinks (0.002 kg/s
H2, 0.016 kg/s O2) asked for 99 mol of H2 over 100 s from an 8 m³
atmosphere holding 29 mol. The volume refused it, as specified. The case
now uses 0.0002 and 0.0016 kg/s.

**The reactions themselves**, ported to `sembawang`, reproduce this crate's
R1–R6 record (R1 241.8296 kJ/mol, R2 1146.093695 K, R5 bit for bit),
and the containment's burn case reproduces bit for bit in peak pressure
and total H2 burned, with round-off (≤ 8 ulp) elsewhere: `sembawang`'s V&V
doc.

## The flash is implicit in the pressure it raises (gh:#1107, 2026-11-01)

> Research, education and V&V only. AI draft, unreviewed.

**The defect (gh:#1107).** Since the surface mixed block (above), flashing
acts on the whole block. In `sembawang`'s 30 h replay, after the 24 h vent,
the upper layers mix to saturation and the block flashes whole in one 1 s
step. Network − old Δ`p_ww` swings up to +12.71 % at 26.5–29.5 h and the vent
window depends on the layer count (−17.2 % at 11 layers, −8.2 % at 22). The
unlayered pool showed the same 225 s burst cycle earlier (up to +38 kPa).

### How the flash is computed here (read before any change)

`ControlVolume::surface` (and before it the wet well's `surface`, and
`sembawang`'s `Containment::pool_surface_step`, which both port) flashes a
pool above `T_s = T_sat(p_gas)` **explicitly**: `T_s` is taken at the
pressure the atmosphere has *before* the flashed vapour joins it, the flash
mass `m = (E_s − m_s u_l(T_s)) / (h_g(T_s) − u_l(T_s))` is applied in full in
that step, and only then is the pressure re-derived. The vapour raises the
pressure, so `T_sat` rises, and the pool, left at the old `T_s`, ends the
step **subcooled** against its own end-of-step pressure. The overshoot is the
loop gain

`G = (M c / h_fg) · (dT_sat/dp) · (∂p/∂m_steam)`

of the flash through the gas space. For the unlayered torus at 26 h (the
`sembawang` trace: 1.75e6 kg, 418 kg flashed, +38.8 kPa, so `∂p/∂m` ≈
93 Pa/kg; `dT_sat/dp` ≈ 2.8e-4 K/Pa at 1 atm; `M c / h_fg` ≈ 3.3e3 kg/K)
`G` ≈ 85: the explicit flash takes about 86 times the self-consistent
amount, then waits for the vent to remove it. `G` scales with the mass that
flashes, which is why the top layer alone (`1/n` of a mixed pool) hid it
and the block (or one layer) brings it back, and why the vent window moves
with `n`. `G` does not depend on `Δt`, so a shorter step shrinks each burst
but not the overshoot ratio.

The network (`CvNetwork::step`, a Picard iteration on the end-of-step
pressures) does not make the flash implicit: each iterate steps every volume
from its start state, and inside that step the flash is again taken at the
pre-flash pressure.

### What MELCOR does (CVH/FL Reference Manual, section and equation numbers only)

- **The pool's vapour is part of the equation of state, at the new
  pressure.** §5.1.3 (Eq 5-30): the vapour in the pool is set by the pool's
  total mass and enthalpy and the saturated enthalpies at the control
  volume's pressure; §2.3 and the CVT package give pressure from the
  contents. Flashing on depressurisation (which §5.1.3 names alongside
  boiling) is therefore not a separate explicit source.
- **The transfer of that vapour to the atmosphere is implicit.** §4.2.1
  (Eqs 4-26 to 4-36): bubble separation is linearised in the pool void
  fraction and eliminated into modified pressure derivatives, so it is
  solved with the pressures. §4.2.1 gives the reason in substance: as an
  explicit source it made boiling pools, at low pressure above all,
  numerically unstable (paraphrased, not quoted).
- **End-of-step consistency is enforced.** §4.3: the new pressures and
  void fractions from the full equation of state must agree with the
  linearly projected ones (0.5 % and 1 % by default), otherwise the
  linearisation point is moved and the step repeated, then cut; §4.3 also
  limits a system step to a 10 % change of pressure. §4.3 names crossing the
  saturation line as the case where a projection over the whole step fails.

So **upstream does not do what this crate does**: MELCOR evaluates the
pool's vapour at the end-of-step pressure, consistently with the vapour's
own effect on it. `sembawang`'s `pool_surface_step` and the wet well do what
this crate does (explicit), and the earlier `sembawang` comment that MELCOR
"holds bubbles up to a 0.4 void" (§5.1.3's bubbly-flow limit) concerns how
much vapour may stay in the pool, not when it is generated.

### The fix chosen, and why

**(b), the flash implicit in its own pressure response**, default ON,
ablation `ControlVolume::without_implicit_flash()` (the explicit flash, bit
for bit). The end state of the flash satisfies `T_block = T_sat(p_end)`,
`p_end` the atmosphere's pressure *after* the flashed vapour has joined it
(and after the pool volume and density have changed with it): a scalar root
in `T_s ∈ [T_sat(p_before), T_block]`, the flash mass at each trial `T_s`
given by the same energy balance as before, the pressure by the volume's own
`refresh` on a probe. This is the single-volume form of MELCOR's
end-of-step equation-of-state consistency (§4.2.1, §4.3, Eq 5-30); the
network's Picard iteration then couples it to the flows, so at convergence
the flash is implicit in the network pressures too. No cap, no limit, no
tuned constant: the root's tolerance (1e-10 K on `T_s`) is a solver control.

(a), "the exchange sees the block but only the top layer flashes", was
rejected: the rest of a superheated block would then be lighter than the
flashed top layer, mix into it at the end of the step and flash on the
next one, so the flash would relax over `n` steps. That is a time constant
set by the layer count, not by the physics, and it leaves the unlayered pool
(`n = 1`, where the top layer is the pool) exactly as unstable as before.

### Predictions (written 2026-11-01, before any run of this change)

**F1, the burst** (`tests/control_volume_flash.rs`, new): a 10 m² prismatic
vessel 5 m tall, 2 m of water at `T_sat(2.0e5 Pa)` (about 393.36 K, about
18.9 t) under 30 m³ of steam (RH 0.9 at that temperature) and N2 at
2.0e5 Pa total, gas at the pool temperature; a port at 4.9 m draws
0.1 kg/s of the atmosphere for 10 s at `Δt` = 1 s. Each draw lowers `p` by
about 0.1/35 of it, ≈ 570 Pa, so the pool is ≈ 0.09 K superheated. `G` ≈ 35
here (`∂p/∂m` ≈ 6.0e3 Pa/kg, `dT_sat/dp` ≈ 1.6e-4 K/Pa, `M c / h_fg` ≈
36 kg/K).

- *Before the fix* (the code as it stands; then `without_implicit_flash`,
  bit for bit): step 1 flashes 3.3 ± 0.7 kg; `p` **rises** in that step by
  +19 ± 5 kPa during a pure depressurisation; the pool ends it subcooled
  against its own pressure by 3.0 ± 0.8 K; no flash in steps 2–10 (the
  pool waits for the draw to undo the burst). At `Δt` = 0.5 s the first
  burst is about half as large.
- *After the fix* (default): every step flashes 0.09 ± 0.01 kg (≈ the draw
  times `G / (1 + G)`); `p` falls in every step, by about 16 ± 5 Pa a step
  (≈ 570 Pa / (1 + G)); at each step's end `|T_pool − T_sat(p)|` < 0.01 K
  (the surface exchange and compression work act after the flash, so not
  1e-10); no one-step rise of `p`.
- *`Δt` convergence, after the fix:* `p(10 s)` at `Δt` = 1, 0.5, 0.25,
  0.125 s within 5 Pa of each other, the differences shrinking with `Δt`.
  Before the fix the largest one-step rise of `p` scales with `Δt`
  (≈ 19, 10, 5, 2.5 kPa).
- *Layers:* the same case in 8 layers (a mixed column, so the block is the
  whole pool): after the fix `|p(8) − p(1)|` < 1e-4 Pa at 10 s.
- *Conservation:* water and energy close against the bookkeeping to
  < 1e-12 relative, both ways.

**Unchanged, because none of them flashes:** L1's 64 values bit for bit
(the pool stays below 306 K at 1 bar), L9 (303 K pool at 105 kPa) bit for
bit at every `n` and both block settings, L2–L10, the network tests and
the wet well (not edited). `sembawang`'s case A unchanged bit for bit, so
`p(n) − p(1)` stays below 1e-4 Pa.

**`sembawang`'s 30 h replay** (predictions in its V&V doc, written now):
nothing moves before the first flash; after the vent the swings fall to
the `without_surface_mixed_block` level, and the vent window's `n`
dependence shrinks.

### Results (2026-11-01, release, Rust 1.99.1; `tests/control_volume_flash.rs`)

**First run: a case error, recorded.** F1 as first written put the pool at
`T_sat` of the *nominal* 2.0e5 Pa, but the volume derives its own
pressure (IF97 steam, its own pool density): 196 122.5 Pa, so the pool
started about 0.6 K superheated, and step 1 flashed 26.4 kg even at
`Δt` = 0.125 s. The case now sets the pool to `T_sat` of the volume's own
initial pressure (three passes, since the pool's temperature moves the gas
volume), which is the case the predictions describe. Nothing else changed.

| Case | Measured | Prediction | Outcome |
|---|---|---|---|
| F1 explicit (unmodified `src/`; pinned, `without_implicit_flash` bit for bit) | step 1 flashes **4.335 kg**, `p` **+32.86 kPa** in that step (from 196 122.5 Pa), pool **5.07 K** subcooled against its own pressure after it; no flash in steps 2–10; `p(10 s)` 221 860.53 Pa | 3.3 ± 0.7 kg, +19 ± 5 kPa, 3.0 ± 0.8 K, no later flash | burst held; **sizes MISSED** (1.3, 1.7, 1.7 times): `∂p/∂m` 7.6 kPa/kg against the 6.0 estimated (the steam warms the gas), so `G` ≈ 47, not 35 |
| F1 implicit (default) | 0.0934 kg in step 1, then 0.0920–0.0921 kg a step; `p` −4.9 Pa in step 1, −16.1 Pa in every later step; worst `\|T_pool − T_sat(p)\|` 1.84e-3 K | 0.09 ± 0.01 kg, −16 ± 5 Pa, < 0.01 K, no rise | held |
| F1 eight layers | `p(10 s)` 7.3e-9 Pa from one layer (implicit); explicit 208 ulp apart | < 1e-4 Pa | held |
| F1 closure | water ≤ 3.2e-16, energy ≤ 3.1e-16 of the inventory, both flashes | < 1e-12 | held |
| `Δt` convergence, implicit | `p(10 s)` 195 972.44, 195 966.94, 195 964.19, 195 962.81 Pa at 1, 0.5, 0.25, 0.125 s: differences 5.50, 2.75, 1.38 Pa, **first order**; spread 9.6 Pa | within 5 Pa, shrinking | shrinking held; **"within 5 Pa" MISSED** (the step-1 transient of the explicit surface exchange, `O(Δt)`) |
| `Δt` convergence, explicit | largest one-step rise 32.9, 16.4, 8.2, 4.1 kPa; `p(10 s)` 221 860.5, 205 342.8, 197 112.1, 197 020.0 Pa | rises ≈ 19, 10, 5, 2.5 kPa | proportional to `Δt` held; 1.7 times the size, as above |
| L1, L9, L2–L10, network tests | all pass; L1's 64 values and L9 bit for bit (no flash in them) | unchanged | held |
| full suite | `cargo test --release -j 3 -p tampines --lib --tests`: 248 passed, 0 failed, 5 ignored (14 binaries) | | |

**A check set after the run, flagged for the maintainer.** The
`Δt`-convergence test first asserted the prediction (all within 5 Pa); it
failed at 9.6 Pa. It now asserts what the run shows and what convergence
means here: each halving of `Δt` halves the difference (ratio in
[0.4, 0.6]; measured 0.500, 0.500), and the spread is under 1e-4 of `p`
(4.9e-5; the explicit flash spreads 0.13 of `p`). The 5 Pa miss stays
recorded here and in the test.

**`sembawang`:** case A unchanged (+341.10 Pa at 2, 4, 8, 16 layers), 6 h
pins unchanged to the printed digits; the 30 h results are in
`crates/sembawang/verification_and_validation/bwr_mark1/containment_network.md`,
section gh:#1107.

**What to quote:** a saturated pool under depressurisation now flashes the
self-consistent amount each step (F1: 0.092 kg against one 4.3 kg burst),
stays on its own saturation line to 2e-3 K, converges first order in
`Δt`, and is independent of the layer count to 1e-8 Pa; the explicit form
is kept bit for bit as the ablation.

## Non-condensables with their own transport properties (gh:#1114, 2026-11-02)

**Before.** The control volume's pool surface passed every
non-condensable to MELCOR's surface correlations as nitrogen (total moles
in `n_n2`): N2's viscosity, conductivity, molar mass (so the density),
`c_p` and steam diffusivity. The wall-condensation kernel
(`components::wall_condensation`) already gave H2 its own properties
(MELCOR's tables, Eucken, Eq 6-2), but O2, CO, CO2 and Ar had no
representation in it.

**Searched first.**
- `wall_condensation::Atmosphere` already carries steam, N2 and H2: MELCOR
  NCG/H2O RM §4.1.2, §5.1.2 and §5.1.4 tables, the N2 power laws (§4.4
  Table 4.1, §5.4 Table 5.2), Eucken for H2 (Eq 4-1), and the steam-air and
  steam-H2 diffusivities (Eqs 6-1, 6-2) combined by Blanc's law.
- `sembawang::bwr_mark1::vapour` (since 2026-11-11 `outram_park_fork_offbeat::melcor::vapour`, gh:#1335) has Chapman–Enskog binary diffusivity
  (MELCOR RN §2.5.2, Eq 2-105) with the diffusion collision integral of
  NCG/H2O Table 6.1, and the Lennard-Jones parameters of steam, H2 and N2
  (NCG/H2O §5.2). Those three are Svehla's (NASA TR R-132, 1962), the table
  reprinted in Poling, Prausnitz & O'Connell, *The Properties of Gases and
  Liquids*, Appendix B.
- `tampines_steam_tables::gas_mixture::nasa7::cv_molar` gives `c_v(T)` for
  O2, CO, CO2 and Ar.
- No workspace code has transport properties for O2, CO, CO2 or Ar.

**The change.**
- A seven-species atmosphere, `wall_condensation::GasMixtureAtmosphere`:
  steam plus every non-condensable.
- Steam, N2 and H2 keep their present properties. O2, CO, CO2 and Ar get:
  - viscosity: Chapman–Enskog, `μ = 26.69 √(M T) / (σ² Ω_μ)` μP, with
    Svehla's σ and ε/k (O2 3.467 Å / 106.7 K, CO 3.690 / 91.7, CO2
    3.941 / 195.2, Ar 3.542 / 93.3) and Neufeld, Janzen & Aziz's (1972)
    fit for `Ω_μ`;
  - conductivity: Eucken, `(c_v + 9R/(4M)) μ` (Eq 4-1, as for H2);
  - steam diffusivity: Chapman–Enskog with steam's parameters (Eq 2-105,
    ported from `sembawang`'s `vapour`).
- Mixtures by the rules already in place: Wilke (Eq 5-2), Wassijewa with
  Mason–Saxena (Eq 4-2), Blanc's law over the non-condensables, molar
  `c_p` summed.
- `Atmosphere` (steam, N2, H2) and `pool_surface::Atmosphere` (steam, N2)
  keep their fields and delegate. The absent gases enter every sum as
  exact zeros, last, so they stay bit for bit what they were (pinned by a
  verbatim copy of the removed code).
- The control volume's pool surface passes its full composition by
  default. `ControlVolume::without_ncg_transport_properties` restores the
  N2 stand-in, bit for bit.

**Predictions (written before the code was changed).**
- **N1.** All-N2 atmospheres are bit-identical, and so is the 3-gas
  `Atmosphere` (steam/N2/H2) and `structure_surface` on it. So `sembawang`'s
  wall condensation, which passes only N2 and H2 through `WallGas`, does
  not move.
- **N2, sign and size on an H2-rich case.** Pool surface `exchange` at
  350 K pool, 330 K gas, 2 bar, steam at saturation of 330 K × 0.5, the
  non-condensables half N2, half H2 by mole, against the N2 stand-in.
  - The diffusivity rises: Blanc with `D_H2O-H2 ≈ 3.6 D_H2O-N2` gives
    ×1.56.
  - The density falls: molar mass 27 → 15 g/mol, ×0.56.
  - The mixture viscosity falls slightly (×0.9), conductivity roughly
    doubles, and `c_p` per kg rises ×1.8.
  - In the turbulent natural-convection limit the mass-transfer
    coefficient goes as `D^(2/3) (ρ/μ)^(1/3)`, so ×1.15.
  - Prediction: **evaporation rises by +10 to +30 %**, and the gas-side
    heat flow rises too.
- **N3, air-like (N2 79 %, O2 21 % of the non-condensables).** Within ±2 %
  of the N2 stand-in: O2 is close to N2 (D −3 %, ρ +2 %, μ +3 % in the
  mix).
- **N4, the closure transient (`tests/control_volume.rs` prediction 1:
  steam, N2 and H2 in, H2 about 0.6 kg).** Closures stay < 1e-12. The pool
  heating (6.31 K) and pressure rise (8.64 %) move by less than 1 % of
  themselves (H2 is a few per cent of the gas by mole there).
- **N5, downstream.** `sembawang`'s containment network passes H2 and the
  other gases through the control volume's pool surface, so its numbers
  move. Not run here (the re-bless holds sembawang); listed in the
  hand-off.

**Results (2026-11-02, release, Rust 1.99.1).** Tests:
`tests/ncg_transport_properties.rs`, the closure test's N4 extension in
`tests/control_volume.rs`, and the default pin in
`correct_physics_is_the_default`.

| Prediction | Measured | Verdict |
|---|---|---|
| N1, three-gas `Atmosphere` and `structure_surface` | 40 608 values bit-identical to the removed code (verbatim copy, bd4932732) | held |
| N1, pool `exchange` with N2 only (both entry points) | 2 400 values bit-identical | held |
| new gases' viscosity at 300 K (±3 %) | O2 −0.64 %, CO −0.66 %, CO2 +1.23 %, Ar +0.52 % | held |
| new gases' conductivity (±5 %; O2 near −5 %, Ar within 1 %) | O2 −4.59 %, CO −0.17 %, CO2 −1.04 %, Ar +0.62 % | held |
| steam diffusivity ratio to N2's (O2 0.97–1.00, CO2 0.60–0.75) | O2 0.986, CO 0.976, CO2 0.692, Ar 0.958 | held |
| **N2, half H2: evaporation +10 to +30 %** | **+7.79 %** (3.869e-3 → 4.170e-3 kg/s); gas-side heat flow +21.2 %; `T_s` −0.59 K | **missed** (below) |
| N3, air-like within ±2 % | −0.05 % | held |
| N4, closure transient (H2 7.9 % of the gas) within 1 % | pool heating 6.305419 → 6.305368 K (−8.1e-6 of itself); pressure rise 8.64313 → 8.65376 % (+1.2e-3); closures unchanged | held |
| N5, `sembawang` | not run (the re-bless holds sembawang) | open |

**Why N2 missed.** The estimate scaled only the mass-transfer
coefficient, about `D^(2/3) (ρ/μ)^(1/3)`, ×1.15. The surface's own energy
balance answers back:
- the extra evaporation cools the surface by 0.59 K, which lowers
  `p_sat(T_s)` in the driving force;
- the H2-rich gas, with roughly twice the conductivity, draws 21 % more
  heat from the surface.

About half of the coefficient's rise is absorbed that way. The test
asserts the measured +5 to +10 %, set after the run and labelled so.

**Moved in `tampines` and not predicted by name (recorded).**
`tests/control_volume_layers.rs` L1's default-mechanism goldens (captured
before gh:#1100) inject H2, so they moved: the pressure at 50 s went
103 503.555 → 103 503.305 Pa. L1 keeps them on
`without_ncg_transport_properties`, the behaviour they were captured on,
and checks the layer identity it exists for (one layer = single region,
bit for bit) at the new default. L2 now runs at the new default: one layer
304.6568 K, eight 304.6650 K, spread 4.762 K, unchanged at the digits
recorded.

**What moves downstream, by default (N5).**
- **Everything in `sembawang` that steps a control volume whose atmosphere
  holds a gas other than N2:** the containment network (H2 from the core,
  O2/N2 in the reactor building), so `containment_network_vs_containment`
  (cases B to E fingerprints), the headless 30 h fixture, the network
  baseline pin and the BSAF-figure comparisons.
- **Not moved:** `sembawang`'s own wall condensation, which goes through
  `WallGas` (steam, N2, H2), whose three-gas path is bit-identical (N1).
  Giving it O2, CO, CO2 and Ar needs `sembawang` to pass a
  `GasMixtureAtmosphere` to `structure_surface_mixture`, a `sembawang`
  change left for after the re-bless.
- **The wet well** (steam and N2) does not move.

**What to quote:** the H2-rich pool-surface change (+7.8 % evaporation at
the state above), and the new gases' handbook agreement.
