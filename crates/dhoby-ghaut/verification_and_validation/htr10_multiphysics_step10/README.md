# HTR-10 coupled run (workbench Steps 9 and 10): V&V record

**Status: TENTATIVE.** AI-drafted, awaiting human review. Research, education
and V&V only: not for facility operation, licensing or safety decisions.
Recorded 2026-10-05 (gh:#574).

## What is computed

Step 10 of the `dhoby-ghaut` workbench runs the Step 9 case. This record uses
the HTR-10 prefill, `crates/dhoby-ghaut/src/bin/dhoby-ghaut/mp_preset.rs`.

- **Thermal-hydraulics.** An r-z multi-channel model of the pebble bed. The bed
  is split into 40 equal-area rings and 200 axial nodes. Each ring is marched
  in the flow direction, which is downward. The march tracks enthalpy exactly,
  using the helium (p, h) flash of `outram-park-fork-coolprop` via
  `tampines::gas_phase::properties`.
- **Pebble surface.** Wakao-Funazkri film coefficient on the local Re, Pr and k
  (`tampines::pebble_bed::cht`).
- **Pebble and fuel temperatures.** Two-zone pebble conduction plus the hottest
  TRISO particle at the pebble centre (`tampines::pebble_bed::Pebble::htr10`),
  on fresh fuel (fluence 0).
- **Friction.** KTA 3102.3 bed friction per node, with the static head
  included.
- **Coupling loop.** Picard iteration over three things:
  - the flow split among rings, until every ring sees the same
    plenum-to-plenum pressure drop;
  - every helium property and temperature;
  - the lumped reactivity feedback, `rho = alpha_iso (T_bed - T_ref)`.
- **Power shape.** **PRESCRIBED**, not solved. It is the bare-cylinder mode
  `J0(2.405 r / R_e) cos(pi z / H_e)`, with one extrapolation length on the
  radius and on both ends. That length is solved so that the peak/mean power
  density equals 2.57 / 2.0. The 2.57 is Gao & Shi's equilibrium-core maximum
  power density (section 4.2) and the 2.0 is their 100 % mean (Table 2). The
  result is 107.2 cm.

### Inputs and their sources

| Input | Value | Source |
|---|---|---|
| Thermal power | 10 MW | Li, Yu & Wei 2014 Table 1 (`nee_soon::htr10_rmc::table1`) |
| Bed radius, height | 90 cm, 197 cm | same |
| Filling fraction, pebble diameter | 0.61, 6 cm | same; IAEA-TECDOC-1382 |
| Inlet helium, system pressure | 250 °C, 3.0 MPa | IAEA benchmark (`tampines` `htr10_design_point`); Gao & Shi 2002 Table 2 |
| Total flow, bed flow | 4.32 kg/s, 3.77 kg/s | Gao & Shi 2002 Table 2, Table 1 |
| Fuel-pebble fraction | 1.0 (equilibrium core) | Gao & Shi 2002 section 4.1 |
| Isothermal coefficient | -1.4e-4 /K | Chen et al. 2009 Table 1, as transcribed in `htgr_sim_v1` (not in the kovan corpus) |
| Cold reference temperature | 300.15 K | Step 5's data temperature |
| Tolerances | ring Δp spread < 1e-4, max ΔT < 0.01 K | Step 9 defaults |

Gao & Shi (2002) is proprietary. Its values were read from the transcription
in `docs/reactor-scoping/htr10-plant-data.md`, sections 7.4–7.6. That document
records that the column assignment of Table 2 was reconstructed by
monotonicity.

## Pass criteria (verification) and how they are tested

These tests run in `cargo test --release -p dhoby-ghaut --bin dhoby-ghaut`.

- **Energy.** One ring with uniform power. The bed-exit temperature from the
  (p, h) march must match an independent `T_in + P / (m c_p)`, with `c_p` from
  the (T, p) route, to within 0.5 K
  (`exit_temperature_matches_an_independent_cp_balance`). The heat carried by
  the helium equals the power to 1e-9, measured at 1.6e-15.
- **Coupling.** The HTR-10 case converges. At convergence the ring
  pressure-drop spread is below 1e-4, the centre ring carries the least flow,
  and the shape's peak/mean is 1.285 to 1e-9
  (`htr10_case_converges_with_equal_ring_pressure_drops`).
- **Shape.** The J0 and J1 series match Abramowitz & Stegun Table 9.1 to 1e-9,
  and the bare-cylinder peak/mean is 3.638 (`porous_core` tests).
- **Regression.** The summary row is pinned by
  `tests/fixtures/dhoby_ghaut_multiphysics.csv`.

## Results (2026-10-05, 40 × 200 mesh)

The run converged after 10 iterations in about 10 s. The full console is in
`console.txt`, and the node fields are in `multiphysics_fields.csv`.

| Quantity | Ours | Gao & Shi 2002, 100 % | Difference |
|---|---|---|---|
| Vessel outlet (bed exit mixed with the bypass) | 695.9 °C | 700 °C | −4.1 K |
| Bed exit, mixed | 760.9 °C | — | — |
| Maximum helium temperature | 869.3 °C | 818 °C | **+51.3 K** |
| Maximum fuel (kernel) temperature | 958.7 °C | 918.7 °C | **+40.0 K** |
| Maximum fuel-pebble surface temperature | 922.8 °C | 876.7 °C | **+46.1 K** |
| Bed pressure drop | 0.51 kPa | 1.3 kPa (bed and bottom reflector) | −0.79 kPa |
| Interstitial velocity at the inlet | 1.39 m/s | 1.5 m/s average at core inlet (section 4.4) | −0.11 m/s |
| Power-weighted mean fuel pebble temperature | 583.1 °C | — | — |
| Lumped reactivity, cold to hot | −7785 pcm | — | — |
| k relative to a cold-critical reference | 0.92777 | — | — |

### Mesh study

The peaks are **mesh dependent**: no heat crosses between rings, so finer
rings resolve a hotter centreline. The default mesh was chosen by
convergence, not by agreement.

| Rings × axial nodes | Maximum fuel | Maximum surface | Maximum helium |
|---|---|---|---|
| 5 × 40 | 931.5 °C | 896.5 °C | 848.4 °C |
| 10 × 40 | 944.8 °C | 909.0 °C | 860.3 °C |
| 20 × 160 | 955.1 °C | 919.3 °C | 866.3 °C |
| **40 × 200 (default)** | **958.7 °C** | **922.8 °C** | **869.3 °C** |
| 80 × 400 | 960.9 °C | 924.9 °C | 870.8 °C |

The first draft defaulted to 5 × 40. It reads 13 K closer to the published
maximum fuel temperature only because the coarse mesh averages the peak away.
It was replaced.

### Ablation: power shape

With a uniform power density (`--uniform-power`):

| Quantity | Uniform | Default shape |
|---|---|---|
| Maximum fuel | 849.3 °C | 958.7 °C |
| Maximum surface | 813.2 °C | 922.8 °C |
| Maximum helium | 760.9 °C | 869.3 °C |

The prescribed shape carries about 110 K of the peaks. The peaks therefore
depend mostly on an input that no neutronics solve has checked (gh:#591).

## Interpretation, and the disagreements

1. **The outlet temperature is an energy balance, not evidence.** 10 MW in
   4.32 kg/s of helium gives 695.9 °C. Gao & Shi's 700 °C is the design value.
2. **The maxima are 40–51 K hotter than Gao & Shi**, even though their
   temperatures *include* uncertainty factors and ours are nominal. Their
   factors (section 4.1) are a burnup peaking factor of 1.2, a hot-spot factor
   of 1.05 and a heat-transfer factor of 1.2. So the true nominal gap is larger
   than 40–51 K. Leading suspects, none of which has been tested:
   - **No radial heat transfer between rings.** ZBS effective conductivity
     and radiation would flatten the centreline (gh:#592).
   - **The adiabatic side wall.**
   - **The prescribed shape.** A J0 × cosine with one extrapolation length is
     not the HTR-10 equilibrium-core distribution, which peaks in the upper
     core (section 4.2 gives the initial-core maximum at Z = 90 cm).
   - **Fresh-fuel graphite conductivity** where the equilibrium core is
     irradiated. Irradiation lowers the conductivity, so this would move our
     peak *up*, not down.
3. **The pressure drop is about 40 % of the published value**, which also
   includes the bottom reflector, and we do not model that. Whether the bed
   share alone agrees is not known.
4. **k is not an eigenvalue.** It is `1 / (1 - rho)`, with rho from one
   isothermal coefficient extrapolated from 27 °C to 583 °C. Chen's value was
   evaluated for 212–650 °C, and the TECDOC-1382 values (−7.4e-5 to −9.2e-5 /K
   over 20–250 °C) differ from it by up to a factor of 2. Read the −7785 pcm
   as an order of magnitude, not a result.

## What is not modelled

See the element list on the Step 9 panel and in `mp_preset.rs::elements`:

- the OUTRAM-Foam porous solver (gh:#592);
- conduction and radiation between rings (gh:#592);
- the reflector, plenums and bypass, thermally;
- spatial neutronics, which needs Step 8's cross sections (gh:#591);
- the farrer-park structural side (gh:#593).

## Images (the crate's drawing rule)

These images are drawn from the solver's own node fields on its mesh, not
from the input constants. They are an axial section, mirrored about the axis,
with every 2nd ring line and every 3rd axial line drawn. Each comes with a
legend.

- `multiphysics_rz_power_density.png`: the prescribed shape. Checked: it is
  symmetric about mid-height, peaks on the axis, and the node averages span 1.36–2.55 W/cm³ (the 2.57 peak is a point value).
- `multiphysics_rz_helium_temperature.png`: the helium heats downward, and the
  centre runs hottest.
- `multiphysics_rz_kernel_temperature.png`: the peak is at the bottom of the
  centre ring (z = 192–197 cm from the top of the bed).

Not checked: whether the HTR-10 bed's conus and discharge tube matter. They
are not in this mesh.
