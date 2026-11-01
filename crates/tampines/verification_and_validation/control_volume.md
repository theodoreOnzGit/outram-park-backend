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
- pool stratification;
- heat structures (the caller supplies `pool_heat`);
- aerosols;
- the liquid energy's pressure dependence.
