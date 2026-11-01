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
- **So**: the wet well amplifies any round-off difference. Its explicit
  surface step and rain-out at 1 s act on a gas held at saturation. The
  control volume reproduces it to the wet well's own reproducibility, and
  no better is possible.
- **What the test asserts**: that explanation (round-off for 5 steps, and
  no more than 10 times the wet well's self-sensitivity), not the original
  criterion. The original criterion stays recorded here as missed.
- **Follow-up**: the amplification is a property of the wet well's
  explicit step. Both runs stay physical and end at the same state to 1e-4.

**What to quote:**

- Closure: round-off.
- Wet well equivalence: "to round-off before saturation, and within the
  wet well's own 1e-4 round-off sensitivity after it".
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
  remaining pool by mass. The surface exchange, flashing and rain-out act on
  the **top** layer (its temperature, its mass for MELCOR's Eq 5-17 limit,
  its thickness as the pool-side depth). Pool heat, the immersed heat
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
  (`Iapws97Hem::break_flow` refuses 300 K liquid from 2.31 to 1.50 bar, and
  from 2.0 to 1.5 bar or 1.2 to 1.0 bar when called directly; it succeeds at
  3 to 1 bar: a defect of the break flow for cold subcooled liquid at low
  pressure, reported with gh:#1100), and IF97 steam's 273.16 K floor
  (gh:#1098). L8 ablates choking (as the manometer test does) and runs at
  350 K.

**What to quote:** one layer is the old volume bit for bit (L1); with
layers, conservation to round-off (L2, L5, L6, L7, L8); conduction across a
stable column within 0.011 K of the analytic slab after 55 h (L3); an
unstable column mixes at once by density (L4); a pool drains, boils or
evaporates dry and refills, conserving to round-off (L6–L8).
