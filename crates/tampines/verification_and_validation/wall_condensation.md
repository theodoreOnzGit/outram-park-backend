# Wall condensation with non-condensables (gh:#1096)

> Research, education and V&V only. AI draft, unreviewed.

Dated 2026-11-01. Part of the MELCOR-style control-volume epic (gh:#1091).
MELCOR 2.2's HS Reference Manual §2.6.1 and §2.7 (private; cited by section
and equation, not quoted; no MELCOR source used) describe a heat-structure
surface facing a control-volume atmosphere: natural convection, and
condensation of steam by diffusion through the non-condensables.

## What it does

`tampines::components::wall_condensation`:

- **`structure_surface` and `Atmosphere`** were moved verbatim from
  `sembawang::bwr_mark1::pool_surface`, the operation order unchanged:
  - natural convection, `Nu = 0.59 Ra^1/4` below `Ra = 1e9`,
    `0.10 Ra^1/3` above `1e10`, linear between (Table 2.3, Eqs 2-82,
    2-85 to 2-87);
  - while condensing, the Grashof number comes from the density difference
    to the saturated gas at the wall;
  - condensation by diffusion through the non-condensables,
    `m'' = h_D ρ_v ln((P − P_srf)/(P − P_stm))` (Eqs 2-134 to 2-136);
  - MELCOR's transport properties for steam, N2 and H2, Wilke's and
    Wassijewa's mixing rules, and the steam-air and steam-H2 diffusivities.
- **`sembawang` now calls the moved code.** `pool_surface` re-exports
  `structure_surface` and `Atmosphere`, and the steam property tables.
  `sembawang` depends on `tampines`, never the reverse (maintainer, 2026-11-01).
- **Reused, not copied:**
  - the N2/H2 `c_v(T)` fits and the molar masses from
    `tampines_steam_tables::gas_mixture::melcor_ncg` (gh:#1093);
  - `p_sat_pa`, `t_sat` and `steam_h` from `tampines::components::wet_well`.
- **Coupling to `HeatStructure` (gh:#1077).** `WallCondensation::exchange`
  takes one side of a structure facing a `WallGas` (temperature, pressure,
  moles of steam, N2 and H2; `WallGas::from_partial_pressures` builds one
  from partial pressures and a volume). For each axial cell, at the
  start-of-step state, it returns:
  - the `StructureBoundary::HeatInflow` to step the structure with;
  - the condensation rate, per cell and in total, for the caller's pool or
    drain;
  - every energy flow: sensible, latent to the wall, condensate enthalpy,
    and the gas's loss.

  Condensation is limited to 90 % of the steam in a step (HS §2.7.3).

| Default | Ablation |
|---|---|
| MELCOR's natural convection plus condensation | `without_melcor_surface(h)`: a fixed coefficient, no condensation |
| IAPWS-IF97 enthalpies (`h_g` region 2 at `T_gas` and `p_steam`, `h_l` region 1 at `T_wall`) | `without_real_steam()`: the containment's constant-`c_v` convention (`sembawang`, `wet_well`) |

Searched first:
- `tampines::components::wet_well` already carried a steam-and-N2-only copy
  of `Atmosphere` (gh:#1075), with no H2 and no structure surface.
  - Its saturation and enthalpy functions are reused here.
  - Folding its `Atmosphere` onto this module needs its own bit-identity
    check, so that is left as follow-up.
- `tuas_boussinesq_solver` has Churchill–Chu (gh:#1063). That is a
  different correlation, and it is used here only as a cross-check.

## Predictions (written before the first run, 2026-11-01)

1. **Bit-for-bit.** The moved `structure_surface` reproduces `sembawang`'s
   original exactly on every state of the grid.
2. **Energy closure.** The gas's loss (sensible plus `ṁ h_g`) equals the
   wall's stored-energy gain plus the condensate's enthalpy, to < 1e-12
   relative. The latent heat exceeds the sensible heat by more than 3×.
3. **The drywell wall on `sembawang`'s inputs.** The coupling gives the
   coefficient and the condensed mass bit-identical to the containment's own
   step 3, and the latent heat to < 1e-14 relative.
4. **Hand check, no steam.** N2 at 0.1 MPa and 400 K on a 300 K wall:
   - at `L = 0.5` m, `h = 5.03` W/(m² K) (`Ra = 4.22e8`, laminar);
   - at `L = 5` m, `h = 4.46` W/(m² K) (`Ra = 4.22e11`, turbulent).

   Each within 1 %. MELCOR's laminar form is about 10 % below
   Churchill–Chu, inside ±15 %.
5. **Step limit and ablations.**
   - With about 1 g of steam, exactly 90 % condenses in the step.
   - The fixed coefficient condenses nothing.
   - The latent heat per kg on the two enthalpy conventions agrees within
     0.5 %.

## Methodology and results (2026-11-01, release, Rust 1.99.1)

### 1. Bit-for-bit against `sembawang`'s original

`crates/sembawang/tests/wall_condensation_bit_identity.rs`.

**How the pre-move outputs were kept.** The test was run first on the
unmodified `sembawang` source (branch base `8841fd46c`, the function still in
`pool_surface.rs`). It printed:
- the FNV-1a 64-bit hash of the bits of every output;
- twelve sample outputs as `f64` bit patterns.

Both were pinned **before** the code moved. After the move the grid is
evaluated through `sembawang`'s re-export and straight through `tampines`.

**The grid** has 2 646 states:
- gas temperature {300, 350, 400, 450, 500, 600, 800} K;
- wall temperature {290, 320, 350, 380, 420, 500} K;
- pressure {0.1, 0.3, 0.8} MPa;
- seven compositions: pure N2; 5 % steam; 30 % steam with 10 % H2; 60 %
  steam; 95 % steam in H2; 99.99 % steam (above `VPFRAC`); pure steam;
- heights {0.3, 5, 30} m (laminar, transitional and turbulent).

| | Result |
|---|---|
| Pre-move hash | `0xbd0af0807827d1c9` (5 292 outputs) |
| After the move, via `sembawang` | `0xbd0af0807827d1c9`: **bit-identical, held** |
| After the move, via `tampines` | `0xbd0af0807827d1c9`: **bit-identical, held** |
| States that condense | 1 050 of 2 646 |

### 2. Energy closure

`crates/tampines/tests/wall_condensation.rs`. The wall is a steel slab: 2 cm in
4 cells, `k = 50` W/(m K), `ρc = 3.9e6` J/(m³ K), 10 m × 3 axial cells of
2 m, at 320 K. The gas is at 420 K: 0.15 MPa steam, 0.12 MPa N2, 0.03 MPa
H2 (dew point 384.5 K). Height 6 m, `Δt = 1` s. The wall is stepped with the
returned boundary, and its stored energy is differenced.

| Convention | `ṁ`, kg/s | sensible, W | latent to wall, W | condensate, W | gas loss, W | closure |
|---|---|---|---|---|---|---|
| IF97 (default) | 0.341902 | 73 054 | 878 717 | 67 157 | 1 018 928 | −1.5e-14 |
| Containment (ablation) | 0.341902 | 73 054 | 880 058 | 67 052 | 1 020 164 | −8.9e-14 |

Held. Latent/sensible = 12.0. `h = 12.183` W/(m² K).

### 3. The drywell wall on `sembawang`'s inputs

`bwr_mark1::containment::tests::the_drywell_wall_through_tampines_reproduces_the_containment`.
This is a unit test inside `sembawang` because it needs the containment's
private state.

**Method.**
- From `Params::default()`, 20 kg/s of steam and 0.02 kg/s of H2 leak into
  the drywell for 1800 s at 1 s steps, with `dw_wall_hs` on.
- Every 300 s the start-of-step wall exchange is computed two ways:
  - (a) as the containment's step 3 does it;
  - (b) by `WallCondensation::new().without_real_steam()` on a one-cell slab
    of the wall's area (4000 m²) at the wall temperature, with the
    drywell's moles.
- The sensible heat is not compared. The containment exchanges it
  implicitly with the gas's capacity; the coupling does it explicitly
  through the slab's half cell. This difference is stated, not tested.

| t, s | T_dw, K | p_dw, Pa | T_wall, K | `h`, W/(m² K), (a) = (b) | condensed in 1 s, kg, (a) = (b) | latent to wall, J |
|---|---|---|---|---|---|---|
| 300 | 405.434 | 177 725.7 | 341.576 | 7.360770 | 16.071825 | 3.945816e7 |
| 600 | 416.213 | 195 565.9 | 366.040 | 6.939290 | 14.847318 | 3.524530e7 |
| 900 | 438.057 | 219 011.3 | 385.594 | 6.391867 | 10.146684 | 2.369096e7 |
| 1200 | 465.367 | 243 176.7 | 396.915 | 6.549834 | 2.162619 | 5.062751e6 |
| 1500 | 483.125 | 257 926.9 | 400.926 | 6.788241 | 0 (above the dew point) | 0 |
| 1800 | 489.058 | 265 799.5 | 402.515 | 6.968567 | 0 | 0 |

Held, and better than predicted:
- the coefficient and the condensed mass are bit-identical in all six
  states;
- the latent heat differs by **0** (predicted only to < 1e-14), because the
  two products round the same way at these states.

### 4. Hand check of the natural-convection limit (no steam)

| `L` | code `h`, W/(m² K) | pencil | `Ra` | `Nu` |
|---|---|---|---|---|
| 0.5 m | 5.02480 | 5.03 (−0.10 %) | 4.2249e8 | 84.587 |
| 5 m | 4.45744 | 4.46 (−0.06 %) | 4.2249e11 | 750.364 |

- An independent coding of the same equations in the test agrees to
  < 1e-12. No condensation.
- Churchill–Chu (Incropera Eq 9.26) gives `Nu = 94.225` at 0.5 m. MELCOR's
  form is **−10.23 %**, as predicted. This is a property of MELCOR's
  correlation, not a defect of the port.

### 5. Step limit and ablations

- **Step limit.** The first run did not reach the limit. **Instrument error,
  corrected:** it cut the steam's moles alone, which also cut its mole
  fraction to nothing, so nothing condensed. The same composition in a 1 L
  gas space now gives: limited; 6.964414e-4 kg condensed of 7.738238e-4 kg,
  which is 90 % to < 1e-15. The criterion was not changed.
- **Fixed coefficient.** Zero condensation, and the heat is
  `−heat_to_fluid` bit for bit.
- **Latent heat per kg.** IF97 gives 2 570 082 J/kg; the containment
  convention gives 2 574 004 J/kg: **+0.153 %**. Held.

### `sembawang` regression, before and after

`cargo test --release -j 2 -p sembawang --lib --tests`, the whole suite,
including the headless 30 h fixture (`headless_30h.csv`, byte for byte), the
pinned uncalibrated baseline, and the BSAF figures:

| Binary | Before (pre-move) | After |
|---|---|---|
| lib (unit tests) | 138 passed | 137 passed (two tests moved to `tampines` with the code, one added: item 3) |
| accident_release | 9 passed | 9 passed |
| bwr_mark1_vs_bsaf_figures | 5 passed | 5 passed |
| chain_handoff | 4 passed | 4 passed |
| depletion_to_source_term | 1 passed | 1 passed |
| inventory_handoff_fingerprints | 1 passed | 1 passed |
| normal_operation_pools_code_to_code | 2 passed | 2 passed |
| nuclide_names_code_to_code | 1 passed | 1 passed |
| venting_modes | 7 passed | 7 passed |
| wall_condensation_bit_identity | (pin printed, not yet pinned) | 1 passed |
| wet_well_vs_tampines | 2 passed | 2 passed |

- The fixture byte comparison
  (`the_headless_run_matches_the_committed_fixture`) and the pinned baseline
  pass unchanged after the move. No tracked file was rewritten.
- **Found on the way:** `tests/wet_well_vs_tampines.rs` did not compile on
  the branch base. It imported `Containment`, `ContainmentSnapshot`,
  `Params`, `gas` and `pool_surface` twice (E0252), left behind by the
  gh:#1075 move. The duplicate `use` lines were removed before the baseline
  run.

## Limits

- The wall temperature is the surface cell's centre. The convective heat
  goes through the half-cell resistance.
- Explicit over the step.
- No radiation, forced convection, film resistance or film evaporation.
- The enthalpy convention must match the atmosphere's own bookkeeping.
- The H2/N2 non-condensable set is MELCOR's subset as `sembawang` has it.
  O2, CO, CO2 and Ar of `GasMixture` have no transport properties here yet.

## All six non-condensables (gh:#1114, 2026-11-02)

`GasMixtureAtmosphere` and `structure_surface_mixture` give O2, CO, CO2 and
Ar their own transport properties. They use Chapman–Enskog viscosity
(Svehla's Lennard-Jones parameters, Neufeld's `Ω_μ`), Eucken conductivity
with NASA-7 `c_v`, and MELCOR's Eq 6-1 scaled by Chapman–Enskog's ratio
for the steam diffusivity. Steam, N2 and H2 keep this kernel's MELCOR
properties. `Atmosphere` and `structure_surface` (steam, N2, H2) delegate
to them, and are bit-identical to the code before (40 608 values,
`tests/ncg_transport_properties.rs`).

Methodology, predictions and results are in `control_volume.md`,
"Non-condensables with their own transport properties (gh:#1114)".
