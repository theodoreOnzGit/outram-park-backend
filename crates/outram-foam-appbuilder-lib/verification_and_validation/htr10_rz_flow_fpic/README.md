# HTR-10 R-Z porous gas flow coupled to the solid field: the FPIC full-power case (gh:#1150)

> **Research, education and V&V only.** Offline demonstration; not for
> reactor operation, licensing or safety decisions (`RESPONSIBLE_USE.md`).
> AI-generated draft, untrusted until a human reviews it. TECDOC-1694 is
> proprietary: only numbers with their table numbers appear here (D1).

## Predictions (written 2026-11-03, BEFORE any coupled case was run)

These were written down after reading the input sheet, the gh:#1149 part-B
record and the Tab 2.6 power map, and **before the gas-flow code existed**.
No coupled run had been made. They are hand estimates, kept unedited below;
the results section compares against them.

**How they were formed.** Gas: 4.32 kg/s at 250 deg C, c_p 5195 J/(kg K);
87 % (3.758 kg/s) through the bed. The RCCS and heads take an estimated
150-250 kW (the RPV sits near the inlet temperature; the gh:#1149 A-HELD
case put 235 kW out at a 270 deg C RPV). KTA 3102.2 at the bed-mean state
(G = 1.48 kg/(m^2 s), Re ~ 2300) gives h ~ 560 W/(m^2 K), a_v = 61 1/m,
so the pebble surface sits q'''/(h a_v) ~ 60-80 K above the gas. Inside a
pebble at the Tab 2.6 peak (2.84 W/cm^3 bed-homogenised, 527 W per pebble)
the surface-to-centre rise is ~45 K (shell ~12 K, fuel zone ~34 K, k ~ 30
W/(m K)). The centre column carries more power per unit flow than the mean
(~1.2x), so its gas leaves near 850 deg C.

| Quantity | Prediction |
|---|---|
| Mixed core outlet (hot plenum, all four streams) | **687 deg C**, band 680-692 |
| Bed gas outlet (87 % stream, mixed at the cone floor) | ~755 deg C |
| Heat to the RCCS + heads | 150-250 kW |
| Maximum fuel-centre temperature (TECDOC Sec 2.1.1.6 definition) | **950 deg C**, band 880-1020, on or near the axis (r < 20 cm), low in the core (z 150-187 cm) |
| Maximum fuel (pebble) surface temperature | ~915 deg C |
| Maximum kernel centre (TRISO step of the GeN-Foam pebble port) | a few K above the fuel-centre value |
| 22 thermocouples, residual (ours - measured) | within +/-100 K at 20 of 22; RMS ~50 K; side reflector (TC 1-12) mostly positive, 0 to +60 K, as INET's; TC 19, 21, 22 the largest misses, under-predicted by 60-150 K; inside the participants' range at >= 14 of 22 |

**Sensitivities (each changes one input; predicted before running):**

| Case | Prediction |
|---|---|
| SS-ASRUN (10.3 MW, 236 deg C, 2.94 MPa, 4.273 kg/s) | outlet ~691 deg C; inlet-dominated TCs (1, 7, 16) -10 K; core-dominated TCs and the fuel +10 K |
| BYP-G02 (86 % bed, 14 % bypass in Tab proportions) | mixed outlet unchanged (< 1 K); bed outlet +6 K; max fuel +8 K; TC 17-19 -5 K |
| BYP-NOLEAK (97/1/2/0) | bed outlet -50 K; max fuel -50 K; TC 14/15 -45 K; TC 19 +100 K |
| ZBS instead of Breitbach (D3) | max fuel -10 K; TC 6/12 +10 to +20 K |
| Carbon brick 30 W/(m K) instead of Tab 2.12, FPIC | TC 3/9 (r 167) -15 K; TC 2/8 (r 189) +5 K; RCCS duty +10 % |
| Carbon brick 30 W/(m K), the gh:#1149 A-MEAN950 stagnant case | RCCS 102.5 -> **112-120 kW**: the brick explains **less than 15 %** of the gap to htgr_sim_v1's 254.0 kW (hand series-resistance chain: brick ~14 % of the total resistance) |
| Pebble without the Maxwell TRISO correction (Tab 2.9 as the fuel-zone conductivity) | fuel centre -2 to -6 K |

(The rest of this record was written after the runs.)

**Disclosure.** Before the 5 cm runs, the unit test
`coarse_fpic_closes_the_plant_energy_balance` ran once on the Fig 2.5 mesh
(819 cells) to check the energy balance. It printed an outlet of 689.7 deg C,
a 132 kW loss and a 917 deg C maximum fuel centre. The predictions above
were written before that run and were not changed after it.

## Headline

> **CORRECTED 2026-11-03 (gh:#1173): re-measured with RPV-cavity natural
> convection, the default since then** (`model.rs`; ablation
> `without_cavity_convection`). `cargo test --release -p
> outram-foam-appbuilder-lib --test htr10_rz_flow_fpic`, 5 cm:
> outlet ~~689.27~~ **688.75 deg C**; RCCS + heads ~~141.7~~ **153.5 kW**;
> max fuel centre ~~912.6~~ **912.0 deg C**; thermocouples mean / RMS
> ~~-16.9 / 49.4 K~~ **-17.9 / 49.8 K**, inside the participants' range
> ~~22/22~~ **19/22** (TC 7, 8 and 10, the outer side reflector at
> z = 170 cm, now fall 1-1.5 K below the participants' lowest), within
> 100 K 21/22. Brick ablation: A-MEAN950 ~~102.53 -> 110.15~~ **105.07 ->
> 113.02 kW** (+7.6 %, 5.3 % of the 148.9 kW gap). Predicted before the
> run: outlet change < 0.2 K and RMS change < 0.5 K; measured -0.52 K
> (a miss: the extra 11.8 kW to the RCCS is 0.5 K of a 4.3 kg/s stream,
> which the prediction under-counted) and +0.4 K. The sensitivities
> re-measured on the same day: SS-ASRUN 693.70 deg C / RMS 50.5 K; FLOW-EB
> 693.05 / 48.6; BYP-G02 688.74 / 49.4; BYP-NOLEAK 688.83 / 64.0; ZBS
> 688.72 / 53.0; BRICK-30 687.76 / 50.3; mesh Fig 2.5 / 10 / 5 / 2.5 cm
> outlet 689.25 / 688.84 / 688.75 / 688.70 deg C, fuel centre 916.5 /
> 912.2 / 912.0 / 909.4 deg C. Every other number below is the
> radiation-only record.

On the 5 cm base mesh, at the benchmark specification (D4), the model gives (radiation only):

- a mixed core outlet of **689.3 deg C**;
- a maximum fuel-centre temperature of **912.6 deg C**, on the axis at
  z = 178 cm. INET gives 988.2; the CRP spread is 941 +/- 48 (881-1026,
  Tab 7.3).

Against the 22 measured thermocouples:

| Model | Mean residual | RMS |
|---|---|---|
| This model | **-16.9 K** | **49.4 K** |
| INET THERMIX-KONVEK | -4.2 K | 38.7 K |
| Mean of the submitted participant values | -5.4 K | 60.7 K |

- **All 22 of our values lie inside the participants' range.** INET's lie
  inside it at 21 of 22.
- 21 of 22 are within 100 K.
- **The one large miss is TC 22**, the discharge-tube wall at z = 260 cm:
  -184 K.

The comparison is reported, not gated. TECDOC-1694 gives no measurement
uncertainty (Sec 7.1.1.2).

**The brick ablation (gh:#1149 question).** In this model, replacing the
Tab 2.12 carbon-brick conductivity with htgr_sim_v1's unsourced 30 W/(m K)
moves the stagnant A-MEAN950 RCCS duty from 102.53 kW to only **110.15 kW**.
That is **5.0 %** of the 151.5 kW gap to htgr_sim_v1's 254.0 kW. **The brick
is not the main cause of that gap.** This contradicts the "most likely cause"
reading in the gh:#1149 record. The rest of the gap lies in features of the
chain that this test does not isolate (see Sensitivities).

## Methodology

**Code.**

| Module | Content |
|---|---|
| `src/htr10_rz/flow.rs` | flow regions, gas network, pressure and gas-energy solves |
| `src/htr10_rz/pebble.rs` | pebble interior |
| `src/htr10_rz/fpic.rs` | coupled driver and thermocouples |
| `SolidConductionField::set_cell_exchange` | the solid half of the coupling, new in `solid_conduction/solver.rs` |

**Tests.**

- Unit: `src/htr10_rz/tests.rs` and
  `solid_conduction/tests.rs::cell_exchange_matches_the_fin_solution`.
- Integration: `tests/htr10_rz_flow_fpic.rs` covers the base case, the
  drawings, the sensitivities, the brick ablation and the 2.5 cm mesh.

Measured 2026-11-03, `cargo test --release`, Rust 1.99.1.

### Flow: KONVEK-like, steady, 2-D R-Z (D2)

- **Mesh.** The gh:#1149 solid mesh, cell for cell: 5 cm base, 11 770 cells.
  The flow-region map is the input sheet's Fig 2.6 reading of the Tab 2.5
  regions. Void fractions are Quoted from Tab 2.5. Hydraulic diameters come
  from Tab 2.3 where it gives a hole size. For regions 2, 11, 12 and 19 they
  are **Assumed** (see the `flow_region` docs).
- **Momentum.** Darcy-Forchheimer, `-grad p + rho g e_z = R G`:
  - bed: KTA 3102.3 (Tab 2.10);
  - channels: axial only, with the ported GeN-Foam Churchill friction on the
    channel mass flux;
  - open volumes: Churchill friction on a slot with `D_h = 2w`.

  The pressure equation is symmetric and solved by DIC-CG. The resistances
  are converged by Picard iteration.
- **Why not GeN-Foam `onePhase`.** It has constant `rho`, `mu` and `c_p`, an
  isotropic drag, and is a transient PIMPLE march. Details are in the `flow`
  module docs.
- **Bypass paths, explicit, split imposed (D5).**
  - Inlet chain: entry 16, annulus 17, bottom cavity 7, region 8, rounding
    path 9, risers 10, top plenum 14.
  - Taps: the discharge tube (1 %) is fed from cavity 7 through throttle 11.
    The leakage (10 %) is fed after the rounding cells at r < 50 cm, through
    the leak gaps 19 into the hot plenum. The control rods (2 %) and the bed
    (87 %) are fed from the top plenum.
  - The 2-D domain holds regions 18, 5, 1, 11, 2, 15, 3, 4, 13, 12 and 19.
  - The outlet is the hot plenum face at r = 90 cm.
- **Gas energy.** Steady and upwind, solved by a topologically ordered sweep.
  - Bed: KTA 3102.2 (Tab 2.10), `a_v = 6(1 - eps)/d`.
  - Elsewhere: Dittus-Boelter `0.023 Re^0.8 Pr^0.4`, floored at 3.66, both
    through the ported GeN-Foam `FsForcedConvectionHtc`, with
    `a_v = 4 eps/D_h`.
  - The solid bed temperature is the **pebble surface** temperature (KTA's
    T_K).
- **Helium.** The Tab 2.8 fits at the operating pressure, in bar, with
  `c_p` = 5195.
- **Power.** The Tab 2.6 shape scaled to 10 MW. The map's own integral is
  9.984 MW.

**Two modelling changes made during development, BEFORE any result was
recorded.** Each was forced by a failure to converge, not by a comparison.

1. **Outlet back-flow accounting.** The first version clipped back-flow at
   the outlet. That broke the cell mass balance and printed a 2194 deg C
   outlet on the first pass. The outlet flux is now signed, so a back-flow
   face takes in gas at the cell temperature.
2. **No buoyancy on faces that touch an open volume.**
   - With buoyancy kept, the 250 deg C leakage entering under the 750 deg C
     core gas drove a Rayleigh-Darcy circulation in the hot plenum.
   - Under the first, laminar slot resistance (`12 mu/(rho w^2)`), the outer
     iteration fell into a 15-pass limit cycle of +/-300 K. The heat to the
     gas swung between 9.6 and 11.2 MW.
   - Under the turbulent Churchill slot resistance with 0.5 under-relaxation,
     it still wandered by about 100 K for 400 passes.
   - TECDOC-1694 Sec 2.1.1.3 calls the hot plenum the place where the
     streams *mix*. A Darcy model with buoyancy does not represent a
     turbulent mixing volume, so the term is dropped there. Buoyancy stays on
     in the bed and the channels.
   - The Churchill slot friction replaced the laminar one in the same
     change. The 0.5 under-relaxation of the gas temperature was kept.

### Coupling and convergence

Outer iteration (block Gauss-Seidel):

1. Solve the gas for the current solid field.
2. Hand `h a_v` and `T_gas` to the solid as an implicit per-cell exchange.
3. Solve the solid steady state, with its own Picard loop over `lambda(T)`
   and radiation.

The iteration stops when no solid or gas temperature changes by more than
1e-4 K.

**Base case.** It converged in **129 passes**, to 9.0e-5 K. The largest
change per pass was:

| Pass | 2 | 5 | 10 | 20 | 40 | 80 | 120 |
|---|---|---|---|---|---|---|---|
| Change, K | 43.1 | 36.5 | 34.1 | 15.9 | 2.71 | 0.257 | 6.4e-4 |

The solid needs 4 Picard passes in the last outer pass. The gas flow needs
2 Picard passes, to 8.9e-8.

**Energy.**

| Term | kW |
|---|---|
| Power | 10 000 |
| RCCS | 125.76 |
| Top head | 5.20 |
| Bottom head | 10.76 |
| Gas, solid side | 9858.28 |
| Gas, `W c_p dT` | 9858.28 |

- The plant imbalance is **9.6e-7** (gate 1e-5, every coupled solve).
- The solid imbalance is 4.9e-12.

### Pebble interior

`pebble.rs` adapts the GeN-Foam `nuclearSteadyStatePebble` port. That port
models an FHR annulus. Here it runs in the `r_core -> 0` limit (1 um) on the
HTR-10 pebble data:

| Quantity | Value | Source |
|---|---|---|
| Pebble radius | 3.0 cm | Tab 2.2/2.3 |
| Fuel-zone radius | 2.5 cm | Tab 2.2/2.3 |
| Kernel radius | 0.25 mm | Tab 2.2 |
| Coatings | 0.09 / 0.04 / 0.035 / 0.04 mm | Tab 2.2 |
| Particles per pebble | 8335 | Derived from 5 g U |
| Matrix conductivity | Tab 2.9, zero dose | Tab 2.9 |
| Layer conductivities | the port's gFHR values | not HTR-10 data |

- **Fuel centre.** Reported by the TECDOC Sec 2.1.1.6 definition: the mean of
  the central 0.3 cm zone.
- **Default physics.** The port's Maxwell TRISO homogenisation is kept as
  the default. The homogeneous Tab 2.9 reading is a sensitivity.
- **Unit check.** The limit reproduces the closed-form full sphere to 1e-9.
  At 2.84 W/cm^3 and a 1000 K surface: 526.6 W per pebble, k 37.18 W/(m K),
  shell 7.51 K, fuel zone 22.54 K, TRISO step 14.35 K.

### Thermocouple instrument (fixed before the first run)

- The model value is the mean over the cells that touch the point.
- In an open volume (void >= 0.99) the **gas** temperature is used. In every
  other cell the **solid** temperature is used.
- Both raw values are listed.
- This makes TC 21 a gas value (hot plenum) and TC 13 a mixed value
  (top-reflector flow zone and entry plenum). All the others are solid
  values.
- TC 14 and 15 ("hot helium plenum", z = 234) lie in the lower
  bottom-reflector channel zone. They are therefore solid values, and their
  gas values are listed too.

### Unit verification (new code)

| Test | Result |
|---|---|
| `cell_exchange_matches_the_fin_solution`: the implicit exchange against the closed-form fin `cosh` solution, 100 cells | max error 6.1e-3 K (estimate 4e-3 K), imbalance 3.5e-13 |
| `pebble_profile_is_the_full_sphere_limit` | shell and fuel-zone rises to 1e-9 of the closed forms |
| `gas_network_conserves_mass_and_energy`: isothermal solid at the inlet temperature | outlet = inlet to 3e-9 K, exchanged heat -0.05 W (round-off over ~1e5 W/K) |
| `gas_network_conserves_mass_and_energy`: solid at +200 K | the gas enthalpy rise equals the exchanged heat to 1e-6 |
| `coarse_fpic_closes_the_plant_energy_balance`, Fig 2.5 mesh | 180 passes, imbalance 3.5e-8 |
| `thermocouple_copy_matches_the_input_sheet_and_parses` | the embedded CSV is byte-identical to the input sheet's |

## Drawings (from the assembled mesh and gas network)

| Image | Checked | Could not check |
|---|---|---|
| `htr10_fpic_flow_region_map.png` | Every Tab 2.5 region is where the input sheet's Fig 2.6 reading puts it: bed with cone and tube; entry plenum; top flow zone; CR channels from z = -65 to 222.2 with the throttle and small plenum; risers r 140.6-148.6 from -65 to 446; lower bottom-reflector channels; hot plenum r 25-90; leak gaps r 39-50, z 325-446; rounding path; bottom annular path; throttle; bottom cavity; entry and annulus at r 197-210 below z 295 | Fig 2.6 itself (private): no overlay. Region 14 (top plenum) is only the solid model's region-43 cells (r 25-83.5, z -75 to -65). Fig 2.6's region 14 extends to z -435, which this mesh does not have |
| `htr10_fpic_flow_regions_zoom_bottom.png` (5 cm mesh) | Cone steps meet region 2 only at r 39-60. The tube (r < 25) touches the bed only at its top, z 217.5. Every channel region has at least 2 cells across | Whether the leak gaps really open into the hot plenum's bottom face (assumed from the flow description) |
| `htr10_fpic_mass_flux.png` | Bed about 2 kg/(m^2 s), uniform; the flow converges in the cone to the region-2 opening, about 10 kg/(m^2 s); the hot plenum discharges radially to r = 90; the tube carries its 1 % upward | The arrows are small; read them with the colours |
| `htr10_fpic_gas_temperature.png` | 250 deg C inlet path; heating down the bed, hottest on the axis; cold tube gas at the bed bottom; the cold leakage at the plenum floor | — |
| `htr10_fpic_solid_temperature.png` | Hottest pebbles on the axis at z about 178 cm; the bottom reflector at 790 deg C; the side reflector cooled by the risers; the RPV at 86-246 deg C | — |

## Results: FPIC base case (5 cm, benchmark specification, 87/1/2/10, Breitbach, Tab 2.12 brick)

| Quantity | Value |
|---|---|
| Mixed core outlet | **689.27 deg C** |
| Bed outlet (cone floor) | 769.94 deg C |
| Gas at the bottom cavity / leak take-off / top plenum | 246.81 / 246.72 / 278.48 deg C. The inlet gas is cooled 3 K by the RPV wall and heated 32 K in the risers |
| RCCS + heads | 141.7 kW |
| Bed-stream pressure drop | 1536 Pa |
| Hottest pebble (r 1.6, z 177.8 cm) | surface 884.6, fuel-zone surface 891.2, **fuel centre 912.6**, kernel 924.4 deg C; 432.7 W per pebble; `k_fz` 32.19 W/(m K) |
| Maximum side-reflector surface (r = 90) | 519.2 deg C. INET 511.8; CRP 577 +/- 116 |
| Maximum bottom reflector | 793.3 deg C. INET 789.3; CRP 761 +/- 75 |
| Fuel centre against Tab 7.3 | 912.6, against INET 988.2 and the CRP 941 +/- 48 (range 881-1026): -76 K from INET, inside the range |
| Fuel surface | 884.6, against INET 924.5 and the CRP 903 +/- 61 |

### The 22 thermocouples (measured: Tab 2.14; participants: Tab 7.1, all 11 columns)

| TC | Location | r | z | Cells | Instrument | Measured C | Ours C | Residual K | INET C | INET residual K | Participants min-max C | Inside |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | Side reflector | 193 | 80 | gap 38 | solid | 231.3 | 220.2 | -11.1 | 243.9 | +12.6 | 204-256 | yes |
| 2 | Side reflector | 189 | 80 | brick | solid | 249.3 | 243.0 | -6.3 | 263.5 | +14.2 | 224-335 | yes |
| 3 | Side reflector | 167 | 80 | graphite | solid | 274.3 | 280.1 | +5.8 | 291.6 | +17.3 | 260-335 | yes |
| 4 | Side reflector | 133 | 80 | graphite | solid | 289.1 | 302.8 | +13.7 | 316.4 | +27.3 | 298-357 | yes |
| 5 | Side reflector | 117 | 80 | graphite | solid | 313.3 | 338.8 | +25.5 | 349.5 | +36.2 | 329-389 | yes |
| 6 | Side reflector | 93 | 80 | inner graphite | solid | 357.7 | 399.8 | +42.1 | 410.4 | +52.7 | 379-471 | yes |
| 7 | Side reflector | 193 | 170 | gap 38 | solid | 234.9 | 220.1 | -14.8 | 247.3 | +12.4 | 217-257 | yes |
| 8 | Side reflector | 189 | 170 | brick | solid | 264.8 | 242.2 | -22.6 | 270.9 | +6.1 | 240-366 | yes |
| 9 | Side reflector | 167 | 170 | graphite | solid | 303.9 | 278.2 | -25.7 | 305.3 | +1.4 | 253-368 | yes |
| 10 | Side reflector | 133 | 170 | graphite | solid | 328.5 | 315.3 | -13.2 | 341.5 | +13.0 | 315-408 | yes |
| 11 | Side reflector | 117 | 170 | graphite | solid | 365.3 | 378.3 | +13.0 | 388.8 | +23.5 | 365-469 | yes |
| 12 | Side reflector | 93 | 170 | inner graphite | solid | 507.1 | 491.1 | -16.0 | 501.3 | -5.8 | 489-646 | yes |
| 13 | Top reflector | 60 | -40 | flow zone 44 / entry plenum (face) | mixed | 245.7 | 279.0 | +33.3 | 260.8 | +15.1 | 251-282 | yes |
| 14 | Hot He plenum | 40 | 234 | lower bottom refl. | solid (gas 797.4) | 800.2 | 769.6 | -30.6 | 769.9 | -30.3 | 720-820 | yes |
| 15 | Hot He plenum | 60 | 234 | lower bottom refl. | solid (gas 761.2) | 763.1 | 758.8 | -4.3 | 714.1 | -49.0 | 653-794 | yes |
| 16 | Bottom brick | 70 | 440 | brick / steel 2 (face) | solid | 224.1 | 248.3 | +24.2 | 239.5 | +15.4 | 240-261 | yes |
| 17 | Bottom brick | 50 | 400 | leak steel / brick (face) | solid | 245.7 | 249.3 | +3.6 | 263.7 | +18.0 | 245-440 | yes |
| 18 | Bottom brick | 50 | 370 | leak steel / brick (face) | solid | 296.2 | 255.3 | -40.9 | 295.4 | -0.8 | 251-311 | yes |
| 19 | Bottom brick | 50 | 340 | leak steel / graphite / brick (corner) | solid | 406.7 | 316.6 | -90.1 | 339.8 | -66.9 | 250-412 | yes |
| 20 | Discharge tube | 26 | 340 | graphite / brick (face) | solid | 334.2 | 295.0 | -39.2 | 316.8 | -17.4 | 254-399 | yes |
| 21 | Discharge tube | 26 | 300 | hot plenum | gas (solid medium 576.8) | 806.1 | 772.9 | -33.2 | 707.4 | -98.7 | 379-820 | yes |
| 22 | Discharge tube | 26 | 260 | bottom refl. solid 31 | solid | 881.7 | 697.8 | **-183.9** | 792.7 | -89.0 | 585-832 | yes |

**Summary statistics, all 22. No thermocouple is dropped.**

| Set | Mean residual | RMS | Max \|residual\| | Within 100 K | Inside the participants' range |
|---|---|---|---|---|---|
| **This model** | **-16.9 K** | **49.4 K** | 183.9 K (TC 22) | 21/22 | **22/22** |
| INET (Tab 2.14) | -4.2 K | 38.7 K | 98.7 K (TC 21) | 22/22 | 21/22 |
| Mean of the submitted participant values per TC | -5.4 K | 60.7 K | 210.8 K (TC 21) | 20/22 | — |
| This model, side reflector (TC 1-12) | -0.8 K | 20.0 K | — | — | — |

**Reading.**

- **Side reflector.** TC 1-12 agree with an RMS of 20 K. The inner
  reflector runs warm (TC 6 +42 K) and the outer one cool (TC 1, 7, 8, 9:
  -11 to -26 K). The radial gradient across the reflector is too steep. The
  risers and the stagnant-gap radiation shape it, and natural convection in
  the gaps is not modelled.
- **Bottom: the largest misses.** They sit near the discharge tube and the
  leak zone.
  - TC 22 (-184 K) and TC 19 (-90 K): this model cools the solid around the
    discharge tube with its 1 % stream, which enters at 247 deg C and flows
    up the tube.
  - TC 18-19 sit on the leak gaps' wall, which the 10 % cold leakage cools.
  - Both bypasses are **assumed**: the split, the leak path and the
    discharge-tube flow direction come from the flow description.
  - BYP-NOLEAK (below) moves TC 19 by +151 K. **Do not read that as evidence
    for the leakage value.** The split stays an input (D5), and TC 22 does
    not move toward the measurement in any split case.
- **Fuel.** The fuel centre (912.6) lies below INET (988.2) but inside the
  CRP range. The model has no wall channelling, and the KTA heat-transfer
  coefficient carries +/-20 % (Tab 2.10).

## Predictions against results

| Prediction | Result | |
|---|---|---|
| Outlet 687, band 680-692 deg C | 689.27 | met |
| Bed outlet ~755 | 769.9 | miss, +15 K |
| RCCS + heads 150-250 kW | 141.7 kW | miss, just low |
| Max fuel centre 950, band 880-1020, r < 20, z 150-187 | 912.6 at (1.6, 177.8) | met: in the band and in the place |
| Max surface ~915 | 884.6 | miss, -30 K |
| Kernel a few K above the centre | +11.8 K | miss. The gFHR coating conductivities (buffer 0.5 W/(m K)) give a larger TRISO step than assumed |
| TCs within +/-100 K at 20 of 22 | 21 of 22 | met |
| RMS ~50 K | 49.4 K | met |
| Side reflector mostly positive, 0 to +60 K | mean -0.8 K, -26 to +42 | miss: half are negative |
| TC 19, 21, 22 the largest misses, -60 to -150 K | TC 22 -184, TC 19 -90, TC 21 -33 | partly: 19 met, 22 beyond the band, 21 smaller |
| Inside the participants' range at >= 14 of 22 | 22 of 22 | met |
| SS-ASRUN: outlet ~691 | 694.19 | miss, +3 K |
| SS-ASRUN: inlet TCs -10 K | TC 1/7/16: -7.9 / -8.1 / -13.4 | met |
| SS-ASRUN: core and fuel +10 K | fuel +14.6 K; TC 14/15 +8.8/+7.9 | met roughly |
| BYP-G02: outlet < 1 K, bed outlet +6, fuel +8, TC 17-19 -5 K | -0.01; +5.6; +7.4; 0 / -0.2 / -1.4 | met, except TC 17-19 (smaller) |
| BYP-NOLEAK: bed outlet -50, fuel -50, TC 14/15 -45, TC 19 +100 K | -54; -66.6; -50.8/-51.5; +150.6 | sign and order met; fuel and TC 19 larger |
| ZBS: fuel -10 K; TC 6/12 +10 to +20 K | fuel +0.9; TC 6/12 +19.9/+21.4 | fuel **miss** (sign); TCs met |
| Brick 30, FPIC: TC 3/9 -15, TC 2/8 +5, RCCS +10 % | -1.4/-1.2; +26.1/+25.5; +14.6 % | **miss** on each. The brick sets the temperature drop across itself, so its outer face (r 189) warms and its inner face hardly moves |
| Brick 30, A-MEAN950: 112-120 kW, < 15 % of the gap | 110.15 kW, 5.0 % of the gap | value just below the band; conclusion met |
| Pebble homogeneous: fuel -2 to -6 K | -1.5 K | miss, smaller |

## Sensitivities (5 cm; one change each from the base)

| Case | Outer passes | Outlet C | Bed outlet C | RCCS + heads kW | Max fuel centre C | Mean / RMS residual K | Inside range |
|---|---|---|---|---|---|---|---|
| **BASE** | 129 | 689.27 | 769.94 | 141.7 | 912.6 | -16.9 / 49.4 | 22 |
| SS-ASRUN (D4: 10.3 MW, 236 C, 2.94 MPa, 4.273 kg/s) | 132 | 694.19 | 778.46 | 129.0 | 927.2 | -23.6 / 50.1 | 14 |
| FLOW-EB (4.278 kg/s) | 129 | 693.58 | 775.21 | 141.8 | 919.0 | -15.3 / 48.3 | 22 |
| BYP-G02 (D5: 86/14) | 128 | 689.26 | 775.51 | 141.9 | 920.0 | -15.8 / 49.1 | 22 |
| BYP-NOLEAK (D5: 97/1/2/0) | 135 | 689.35 | 715.99 | 139.8 | 846.0 | -10.6 / 63.8 | 11 |
| ZBS bed (D3 comparison) | 128 | 689.24 | 769.92 | 142.4 | 913.5 | -14.0 / 52.8 | 22 |
| BRICK-30 (ablation; unsourced) | 128 | 688.35 | 768.97 | 162.4 | 911.5 | -12.6 / 50.2 | 21 |
| PEBBLE Tab 2.9 homogeneous | 129 | 689.27 | 769.94 | 141.7 | 911.1 | -16.9 / 49.4 | 22 |

Per-thermocouple shifts against the base, K:

| Case | Shifts |
|---|---|
| SS-ASRUN | -3.5 (TC 12) to -13.6 (TC 17) at every structure TC; +8.8 / +7.9 at TC 14 / 15; +8.8 at TC 21; +5.4 at TC 22 |
| BYP-NOLEAK | TC 17 +49.6, TC 18 +103.5, TC 19 +150.6, TC 20 +129.5; TC 14 / 15 -51; TC 21 -50; TC 12 -26 |
| ZBS | TC 6 +19.9, TC 12 +21.4, TC 5 / 11 +10 / +9; TC 22 -10 |
| BRICK-30 | TC 1 / 2 / 7 / 8 +25 to +26; all others within 9 K |

- **SS-ASRUN** lies inside the participants' range at only 14 of 22. The
  benchmark participants ran the specification point, so that range is
  centred on it.
- **Brick ablation inside the gh:#1149 stagnant case (A-MEAN950,
  Breitbach).**

| Brick | RCCS kW | Heads kW | Bed power kW | T at r 93 / 167 / 189 / 214 cm, z 90 (deg C) |
|---|---|---|---|---|
| Tab 2.12 (5-8 W/(m K)) | **102.53** (the gh:#1149 record, reproduced) | 7.02 / 3.04 | 112.59 | 415.2 / 367.7 / 307.0 / 178.3 |
| 30 W/(m K) (htgr_sim_v1, unsourced) | **110.15** (+7.4 %) | 9.61 / 3.17 | 122.92 | 379.2 / 327.3 / 312.6 / 183.5 |

The brick carries **5.0 %** of the 151.5 kW gap to 254.0 kW. Inside this
model the brick is not the dominant resistance. The two radiating helium
gaps and the RPV-to-RCCS radiation are larger.

So the gap to htgr_sim_v1's chain must come mainly from its other features:

- annuli with no axial resistance;
- no barrel and no gap layers;
- the mean-to-surface conductance of the bed.

That is a reading, not a measurement: those features were not ablated here.
`htgr_sim_v1` was not modified or re-run; its validation is frozen.

**Mesh** (the Fig 2.5 lines split to at most h):

| h | Cells | Passes | Outlet C | Max fuel C | RMS K | TC 6 | TC 11 | TC 12 | TC 21 | TC 22 |
|---|---|---|---|---|---|---|---|---|---|---|
| Fig 2.5 | 819 | 180 | 689.70 | 917.0 | 81.0 | 395.7 | 340.4 | 488.4 | 489.8 | 721.3 |
| 10 cm | 3 729 | 138 | 689.35 | 912.7 | 46.1 | 385.1 | 357.7 | 489.7 | 771.2 | 708.4 |
| **5 cm (base)** | 11 770 | 129 | 689.27 | 912.6 | 49.4 | 399.8 | 378.3 | 491.1 | 772.9 | 697.8 |
| 2.5 cm | 41 580 | 129 | 689.23 | 909.9 | 50.5 | 408.6 | 370.9 | 498.4 | 773.3 | 695.7 |

- The outlet is converged to 0.04 K and the fuel maximum to 2.7 K between
  5 and 2.5 cm.
- **Single thermocouples still move by up to 9.4 K** between 5 and 2.5 cm
  (TC 1 -9.2, TC 7 -9.4, TC 6 +8.8, TC 12 +7.3). The steepest gradients sit in the inner reflector and
  in the 4 cm gap 38.
- The RMS moves by 1.1 K.
- On the Fig 2.5 mesh, TC 21's hot-plenum cell spans r 25-39 cm. Its gas
  is set by the cold leakage entering below it (-283 K against the 5 cm
  mesh). That mesh does not resolve the tube-plenum-leak corner.
- The 2.5 cm run takes about 3.5 min. It sits behind `long-tests`.

## The open-only gate (She 2021 Fig 8, CC BY)

**Not run here: covered by gh:#1151.**

- She 2021 Fig 8 compares PANGU with the measured core outlet temperature
  over seven steady periods of the 2003-2007 operating history.
- It is a figure only, not digitised (input sheet Sec 11).
- Its power states are not the FPIC point.
- gh:#1151 lists it as its open-only gate.

The only open-source statement that applies here is that the full-power
outlet was 700 deg C ([S21]). This model's 689.3 deg C mixed outlet follows
from the rated 4.32 kg/s flow, which is 1 % above the energy-balance flow,
and from 142 kW of losses.

## Known flaws, carried openly (D5)

- **Bypass.** The bypass split and its paths are **assumed**. The leak gaps,
  the discharge-tube stream and the throttle `D_h` are not measured or
  Unknown. They drive the largest residuals (TC 19-22).
- **Open volumes.** The plena carry no buoyancy (see Methodology). The mixed
  nodes (entry, bottom cavity, top plenum) are well mixed. The duct from the
  riser tops to the top plenum exchanges no heat.
- **Extents.** Tab 2.5 region 14 is represented by the region-43 cells only.
  Every extent is Uncertain (Fig 2.5/2.6 readings).
- **Missing transport.** No wall channelling in the bed and no gas
  conduction or dispersion. Natural convection in the stagnant helium gaps
  is still missing (gh:#1149).
- **TRISO data.** The layer conductivities are the GeN-Foam gFHR values, not
  HTR-10 data. They set the 12 K kernel step.
- **Not modelled.** The as-run power map (only the scale changes in
  SS-ASRUN). Irradiation: zero dose throughout.
