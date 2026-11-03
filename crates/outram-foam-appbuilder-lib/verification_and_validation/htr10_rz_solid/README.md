# HTR-10 R-Z solid conduction and radiation (gh:#1149 part B)

> **Research, education and V&V only.** Offline demonstration; not for
> reactor operation, licensing or safety decisions (`RESPONSIBLE_USE.md`).
> AI-generated draft, untrusted until a human reviews it. TECDOC-1694 is
> proprietary: only numbers with their table numbers appear here (D1).

**Code:** `src/htr10_rz/` (geometry, materials, power map, model), on the
part-A field `genfoam::thermal_hydraulics::solid_conduction`, with the
gh:#1148 closures `htgr_closures`. **Tests:** `src/htr10_rz/tests.rs` (unit
checks), `tests/htr10_rz_solid.rs` (drawings, cases, convergence,
sensitivities). Measured 2026-11-03, `cargo test --release`, Rust 1.99.1.

> **CORRECTED 2026-11-03 (gh:#1173): the default changed, every number
> below that is not re-stated here is the radiation-only record.** The RPV
> side wall now carries natural convection of the cavity air beside the
> radiation (MacGregor & Emery 1969 enclosure correlation; `model.rs`,
> ablation `without_cavity_convection`). Re-measured on the 5 cm base,
> `cargo test --release -p outram-foam-appbuilder-lib --test htr10_rz_solid`:
>
> | Case | RCCS kW (radiation only -> with convection) | in kW | hottest | RPV r 214, z 90 |
> |---|---|---|---|---|
> | A-MEAN950 Breitbach | ~~102.53~~ **105.07** (+2.5 %) | ~~112.59~~ 113.98 | 1276.0 K | ~~178.3~~ 161.6 deg C |
> | A-MEAN950 ZBS | ~~127.15~~ **130.56** | ~~139.17~~ 141.26 | 1244.5 K | 182.0 deg C |
> | A-HELD Breitbach | ~~235.40~~ **242.34** | ~~255.20~~ 260.20 | 950.0 K | 251.6 deg C |
> | A-DECAY 200 kW Breitbach | ~~183.84~~ 185.71 | 200.00 | ~~1557.9~~ **1555.1 K** | 221.7 deg C |
> | A-DECAY ZBS / Kugeler 2017 | 185.70 / 185.71 | 200.00 | ~~1433.2 / 1577.1~~ 1429.4 / 1574.0 K | 221.6 / 221.7 deg C |
> | A-MEAN950 RCCS-SHIELD | ~~94.51~~ 100.19 | 111.24 | - | 194.4 deg C |
> | A-MEAN950 HEADS-ADIABATIC | ~~111.92~~ 113.46 | 113.46 | - | 162.3 deg C |
>
> Predicted before the run (`crates/nee_soon/verification_and_validation/htr10_rz_transients/README.md`
> Sec 7): A-MEAN950 total +2 to +5 % (measured **+1.2 %**, a miss, low);
> A-DECAY hottest 5-15 K cooler (measured **-2.8 K**, a miss, smaller).
> The RPV wall cools by 17-21 K, but the core temperatures hardly move:
> the in-vessel path, not the wall, limits the duty. Against htgr_sim_v1's
> 254.0 kW the A-MEAN950 RCCS duty is now -58.6 % (Breitbach) / -48.6 %
> (ZBS); against Hu's 206 kW -49.0 % / -36.6 %.

**Headline (radiation only, before gh:#1173).** With the bed's volume-mean temperature at 950 K, stagnant
(the `htgr_sim_v1` instrument), this 2-D model carries **102.5 kW** to the
RCCS (112.6 kW in all) under Breitbach, and **127.2 kW** (139.2 kW) under
ZBS: **-60 % / -50 % against the htgr_sim_v1 chain's 254.0 kW**, and
-50 % / -38 % against Hu's 206 kW (a pre-test analysis input for normal,
forced-flow operation, not a measurement and not the same quantity). The
reflector inner face sits at 415 deg C, the same as the chain's 411.6 deg C,
so the gap is in the reflector-to-RCCS path: this model's path carries about
2.5 times less heat at the same reflector temperature. ~~The most likely
cause is the carbon brick: TECDOC Tab 2.12 gives 5-8 W/(m K), the chain
assumes 30. This was not ablated here (30 W/(m K) has no source).~~
**CORRECTED 2026-11-03 (gh:#1150, measured):** the brick was ablated in
this model (`tests/htr10_rz_flow_fpic.rs::brick_ablation_in_the_stagnant_mean950_case`).
With 30 W/(m K), A-MEAN950 moves from 102.53 to only 110.15 kW: 5.0 % of the
gap. The brick is **not** the main cause. See
`verification_and_validation/htr10_rz_flow_fpic/README.md`.

The predictions follow, unedited, then the methodology and the results.

## Predictions (written before the fine-mesh runs, 2026-11-03)

These were formed from a hand resistance chain (graphite side reflector
~80 W/(m K) over 0.9-1.675 m; carbon brick ~6.5 W/(m K) over 1.675-1.90 m;
two radiating helium gaps of ~30-45 W/(m^2 K); the RPV radiating to 50 deg C
with eps_eff 0.76) before any HTR-10 case was run. **Disclosure:** the
coarse-mesh unit test `coarse_solves_close_the_energy_balance` (Fig 2.5
mesh, 819 cells) ran once, to check the energy balance, before this file
was written down; it printed RCCS 193.1 kW for the held case and a 1559 K
maximum for the 200 kW case. The numbers below are the ones formed before
that run and are not adjusted.

| Case | Quantity | Prediction |
|---|---|---|
| A-HELD (core + cone held at 950 K, Breitbach) | RCCS side-wall duty | **280 kW**, band 200-360 kW |
| A-HELD | heads (top + bottom) | 30-80 kW together |
| A-HELD, ZBS | RCCS duty against Breitbach | within 2 % (the bed is held, so only the free discharge tube sees the bed closure) |
| A-DECAY (200 kW, FPIC shape, Breitbach) | hottest cell | **1450 K +/- 150 K**, on the axis in the upper core |
| A-DECAY | RCCS share | ~80 % of 200 kW (160 kW), heads ~40 kW |
| A-DECAY, ZBS | hottest cell | ~1160 K (centre-to-edge drop shrinks by ~1/1.6) |
| Mesh convergence (10 / 5 / 2.5 cm) | RCCS duty, hottest cell | monotone, observed order ~2, fine-mesh change < 1 % / < 5 K |
| SR-TOPBOTTOM | any | no change if no side-reflector cell is below 300 deg C; the stagnant structures should run hotter than 300 deg C at the inner reflector |
| HEADS-ADIABATIC | A-HELD RCCS duty | rises by part of the head duty; total falls |
| RCCS-SHIELD | A-HELD RCCS duty | falls by 25-40 % (the shield roughly halves eps_eff) |

### Added prediction, after the first mesh study (2026-11-03, before its run)

The first mesh study showed the held-cell instrument converging at first
order in an offset that depends on how each Fig 2.5 interval is split
(RCCS 215.9 / 222.9 / 229.0 kW at 5 / 2.5 / 1.25 cm). It was replaced by two
instruments (see Methodology): A-HELD (bed isothermal to its surface) and
A-MEAN950 (bed volume-mean 950 K, source-driven; the htgr_sim_v1
definition). The A-MEAN950 prediction below was written **after** seeing
the A-DECAY results (200 kW: hottest 1558 K, side-reflector inner face
~890 K) and is derived from them, which is stated so it is not mistaken for
an independent one:

| Case | Quantity | Prediction |
|---|---|---|
| A-MEAN950, Breitbach | power = duty for a 950 K bed mean | **140 kW**, band 110-170 kW (scaling 200 kW by (950-323)/(~1224-323)) |
| A-MEAN950, ZBS | same | ~160 kW (a better-conducting bed has a hotter surface at the same mean) |
| A-HELD (isothermal to the surface) | RCCS duty | ~235 kW (the old held cells extrapolated to zero offset) |

## Methodology

### Geometry: a cited mesh-builder input (D2)

- **Source:** `docs/reactor-scoping/htr10-th-benchmark.md` Sec 3 (TECDOC-1694
  Tab 2.3, Tab 2.4, Fig 2.5). The mesh lines are the 33 x 57 Fig 2.5 labels,
  cut to the RPV: r 0-218 cm (22 lines) and z -351 to 665 cm (40 lines).
  TECDOC `z` is used: positive is **downward** and z = 0 is the bed top.
  Each interval is split into `ceil(length / h)` equal cells, so every
  region boundary lies on a face.
- **Regions:** there are 31 model regions. They cover every Tab 2.4 region
  inside the RPV, including:
  - the pebble bed, split into the core with its cone and the discharge
    tube;
  - region 2, split into its side and top/bottom parts, because Tab 2.11
    gives them different fits.

  The regions outside the RPV are represented by the boundary conditions:
  15, 16, 17, 24, 26, 27, 35-37, 39 and 42.
- **Confidence:** every extent keeps the input sheet's **Uncertain**.
  `geometry::region_rects` labels each rectangle as Fig 2.5, Tab 2.3, or
  **Assumed**. Assumed means the sheet did not resolve the extent and this
  module filled it:
  - the barrel and gap 38 run over z -219..507. That is 726 cm against
    Tab 2.3's 737 cm;
  - the RPV top head is at z -351..-343. This is the only 8 cm Fig 2.5
    interval above the cavity. Derived height: 1016 cm, against the
    1115.5 cm total in Tab 2.3;
  - the side column runs from z -130 to 440;
  - r 148.6-190, z 446-461 is steel 1;
  - region 23 runs up to the head.
- **Derived checks** (unit tests):
  - the core plus cone volume is 5.041 m^3, against Tab 2.3's 5 m^3;
  - the FPIC map integral is 9.984 MW, as in the input sheet;
  - the mapped power is exact on every mesh to 1e-12;
  - 42 probe points land in their intended regions on the **assembled**
    mesh;
  - every region is present.

### Properties (`materials.rs`)

- **Closures:** every property comes from `htgr_closures`. Tab 2.4 solid
  fractions `f` weight both properties:
  - conductivity: `f lambda_s + (1 - f) lambda_He`;
  - heat capacity: `f rho c_s + (1 - f) rho c_He`.

  For conductivity this is the parallel-path mean. It is exact along the
  channels and an upper bound across them.
- **Bed:** `BedConductivityModel`. **Breitbach is the default (D3).** ZBS
  uses the Tab 2.9 fuel-element `ks` at zero dose and Tab 2.8 helium at
  30 bar.
- **Helium cavities:** the THERMIX-style medium is
  `lambda_He + 4 sigma T^3 delta / (2/eps - 1)`, where:
  - `eps` comes from Tab 2.13. Where Tab 2.13 is silent, 0.85 or 0.79 is
    assumed and stated in `geometry.rs`;
  - `delta` is the meshed width. It follows the barrel placement.

  Integrated across the cavity, this term gives exactly the grey two-plate
  exchange. Natural convection is not modelled.
- **Holds outside closure ranges:** properties are tabulated every 5 K over
  273-2273 K. Outside a closure's range the end value is held, and every
  region that reaches a held value is listed as a "range flag".
- **Zero fluence everywhere.** This is the initial core. The DOSIS unit is
  Unknown.

### Side reflector below 300 deg C (the gh:#1148 flag)

TECDOC-1694 gives no guidance below 300 deg C. Tab 2.11 states 0-2000 deg C,
but its printed fit contradicts that range. So no new fit is invented:

- **Base case (`SR-HOLD300`, named assumption):** hold the 300 deg C value,
  84.2 W/(m K).
- **Sensitivity (`SR-TOPBOTTOM`):** use the Tab 2.11 top/bottom-reflector
  fit, which is unirradiated. At 300 deg C it agrees with the side fit
  within 0.5 %.

The rule is exercised in the base cases. In A-MEAN950, 268 of the 11 770
cells are side-reflector cells below 300 deg C, at the cold ends of the
column; the coldest is 491 K.

### Boundaries (`model.rs`)

| Boundary | Condition |
|---|---|
| Axis | adiabatic |
| RPV side wall (r = 218) | grey radiation to the RCCS panels at **50 deg C** (Sec 2.1.1.4), eps 0.85 both (Tab 2.13); concentric cylinders (Kugeler 2017 eq. 125), panels at r = 285.4 cm (Uncertain): eps_eff = 0.7626 |
| RPV heads | radiation to 50 deg C, eps_eff = 1/(2/0.85 - 1) = 0.7391. This ignores the insulation (Unknown properties) and **over-states** the path. Bounded by `HEADS-ADIABATIC`. |

Air conduction and convection in the cavity are not modelled.

### Cases (gh:#1149 (a); solid only, no gas flow; that is gh:#1150)

- **A-MEAN950, the like-for-like.**
  - The bed generates power with the Tab 2.6 shape.
  - A secant search finds the power at which the bed's **volume-mean**
    temperature is 950 K.
  - This is the definition the `htgr_sim_v1` 254.0 kW uses: a lumped bed
    node at 950 K behind a mean-to-surface conductance, stagnant, in
    equilibrium.
- **A-HELD, the upper bound.**
  - The bed is held isothermal at 950 K out to its surface.
  - Its cells are held with the new
    `SolidConductionField::set_fixed_temperatures`, an OpenFOAM
    `setValues` elimination. Its conductivity is set to 1e6 W/(m K).
  - Why the 1e6: in the first held-cell version the held boundary sat half
    a cell inside the bed. The duty then converged at first order in an
    offset set by how each Fig 2.5 interval split:
    193.1 / 200.6 / 215.9 / 222.9 / 229.0 kW. That version is superseded.
- **A-DECAY.**
  - The bed generates a sustained **200 kW** with the Tab 2.6 shape.
  - 200 kW is about the LOFC long-term power at about 7200 s
    ([H06] Tab 1, the input sheet Sec 7.2).
  - At steady state the duty equals the source, so this case reports the
    temperatures and the split.
  - It is a conduction-only upper bound. There is no in-vessel natural
    circulation, and this is not a transient.

**Gates:**

- every solve closes its energy balance to 1e-6;
- in practice, the measured imbalance is at most 4e-9;
- the 5 cm numbers are regression-pinned to 0.1 %.

The comparisons against 254 kW and 206 kW are reported, not gated.

## Drawings (from the assembled mesh's cell lookup)

| Image | Checked | Could not check |
|---|---|---|
| `htr10_rz_region_map.png` (whole model) | Region layout against the input sheet's Fig 2.5 extents: the bed, the 6-step cone, the discharge tube and throttle; the top flow zone, plenum and core-entry plenum stack; the side column order 14 / 3 / 2 / 4 / 2 / 5; the bottom brick, the supports and the bottom cavity; the gaps, the barrel and the RPV | Fig 2.5 itself (private): no overlay was possible. The Assumed fills look plausible but are not checked against the source |
| `htr10_rz_mesh_10cm.png` | Every region boundary lies on a mesh line. The Fig 2.5 lines are subdivided with no stray lines | — |
| `htr10_rz_zoom_gaps_vessel.png` (r 150-218, z -60..240) | Brick 167.5-190, gap 38 190-194, barrel 194-197, gap 23 197-210, RPV 210-218. At least one cell across each steel layer (3 and 8 cm) at 5 cm | Whether the barrel is really at 194-197 (N5). Run as a sensitivity |
| `htr10_rz_zoom_bottom.png` | Cone steps at 7.5 cm; regions 8, 31, 28, 9, 25, 13, 11, 20, 10, 12 and 29 where the sheet places them; the riser over the steel at 440-446 | The Assumed steel at r 148.6-190, z 446-461 |
| `htr10_rz_zoom_top.png` | Top reflector 90 cm, brick 40 cm, plenum r 25-83.5, gap 38 starting at z -219, RPV head | The head position and everything above z -219 (Assumed) |
| `htr10_rz_temperature_held_950k.png`, `..._decay_200kw.png` | Hottest point on the axis in the upper core; temperature falls monotonically outward; the cavities are nearly isothermal | — |

## Results (5 cm base mesh, 11 770 cells; "in" = power generated or supplied)

| Case | RCCS kW | top / bottom head kW | in kW | hottest | reflector r 93 | brick r 180 | RPV r 214 (z 90, deg C) |
|---|---|---|---|---|---|---|---|
| **A-MEAN950 Breitbach** | **102.53** | 7.02 / 3.04 | 112.59 | 1272.8 K | 415.2 | 328.0 | 178.3 |
| A-MEAN950 ZBS | 127.15 | 8.43 / 3.59 | 139.17 | 1240.8 K | 479.6 | 369.1 | 199.8 |
| A-HELD Breitbach | 235.40 | 14.08 / 5.73 | 255.20 | 950.0 K | 670.0 | 502.2 | 270.5 |
| A-HELD ZBS | 235.47 | 14.08 / 5.75 | 255.30 | 950.0 K | 670.0 | 502.2 | 270.5 |
| A-DECAY 200 kW Breitbach | 183.84 | 11.51 / 4.65 | 200.00 | **1557.9 K** (bed mean 1172.0) | 617.2 | 451.5 | 242.8 |
| A-DECAY ZBS | 183.83 | 11.50 / 4.67 | 200.00 | 1433.2 K (mean 1100.7) | 616.1 | 451.3 | 242.7 |
| A-DECAY Kugeler 2017 | 183.83 | 11.51 / 4.65 | 200.00 | 1577.1 K (mean 1174.3) | 617.1 | 451.5 | 242.8 |

**Against the references (RCCS side wall):**

| Case | vs 254.0 kW (htgr_sim_v1) | vs 206 kW (Hu) |
|---|---|---|
| A-MEAN950 Breitbach | -59.6 % | -50.2 % |
| A-MEAN950 ZBS | -49.9 % | -38.3 % |
| A-HELD (upper bound) | -7.3 % | +14.3 % |

Which number to quote:

- **A-MEAN950** is the one to quote against the htgr_sim_v1 chain, because
  it uses the same definition.
- **A-HELD** only bounds it, because it has no bed resistance.
- Hu's 206 kW is a forced-flow, normal-operation input to a pre-test
  analysis. None of these cases measures that quantity, so it is context
  only.

**Why the gap to the chain.**

- At A-MEAN950 the reflector inner face is 415 deg C. The chain's
  reflector node is at 411.6 deg C.
- Yet this model carries 2.5 times less heat.
- So the difference lies outward of the reflector. ~~It is plausibly the
  carbon brick (Tab 2.12 5-8 W/(m K) here, an unsourced 30 W/(m K) in the
  chain).~~ **CORRECTED 2026-11-03 (gh:#1150):** ablated, the brick
  carries 5.0 % of the gap (102.53 -> 110.15 kW with 30 W/(m K)).
- Two other features of the chain may contribute:
  - its annuli conduct over their full heights with no axial resistance,
    which its own doc calls an upper bound;
  - it has no barrel or helium gap layers.
- ~~None of these has been ablated~~ Only the brick has been ablated
  (gh:#1150); the other two remain a reading of the temperature profile,
  not a measurement.

**Bed closure (D3).** The choice moves A-MEAN950 by **+24 %** (ZBS over
Breitbach). It moves the 200 kW peak by -125 K, and Kugeler moves it by
+19 K. It does not move A-HELD (+0.03 %), because there the bed is held.

## Predictions against results

| Prediction | Result | |
|---|---|---|
| Held RCCS 280 kW, band 200-360 | 215.9 kW (first held instrument, 5 cm); 235.4 kW isothermal | inside the band, below its centre |
| Heads 30-80 kW | 18-20 kW (A-HELD) | **miss**: the top brick and the long cavities insulate more than estimated |
| ZBS within 2 % (held) | +2.64 % first instrument; +0.03 % isothermal | the first miss exposed the half-cell offset defect |
| A-DECAY hottest 1450 +/- 150 K | 1557.9 K | inside the band, upper edge |
| A-DECAY RCCS share 80 % | 91.9 % (heads 16.2 kW) | **miss** (same cause as the heads) |
| A-DECAY ZBS ~1160 K | 1433 K | **miss**: ZBS exceeds Breitbach by ~32 % when hot, not 60 % |
| Convergence order ~2, fine change < 1 % / 5 K | order ~1.2; fine change 0.07 % / 0.04 K | order **miss**, magnitude met |
| SR-TOPBOTTOM: no change unless cells < 300 C | cells below 300 C exist (column ends); change +0.03 % | partly a miss: the flag is reached, and it hardly matters |
| HEADS-ADIABATIC: RCCS up, total down | +9.2 % RCCS, -0.6 % total | as predicted |
| RCCS-SHIELD -25 to -40 % | -7.8 % (A-MEAN950) | **miss**: the RPV heats by 46 K and T^4 compensates |
| (added after the results) A-MEAN950 140 kW, band 110-170 | 112.6 kW in all; 102.5 kW to the RCCS | in the band, at its edge |
| (added after the results) ZBS ~160 kW | 139.2 kW | miss, low |
| (added after the results) A-HELD isothermal ~235 kW | 235.4 kW | met (derived from results; weak evidence) |

## Mesh convergence

Every Fig 2.5 interval is split into cells of at most `h`.

| h | cells | A-MEAN950 RCCS kW | A-HELD RCCS kW | A-DECAY hottest K |
|---|---|---|---|---|
| Fig 2.5 itself | 819 | 100.42 | 229.40 | 1559.2 |
| 10 cm | 3 729 | 102.17 | 234.58 | 1557.7 |
| 5 cm (base) | 11 770 | 102.53 | 235.40 | 1557.88 |
| 2.5 cm | 41 580 | 102.70 | 235.85 | 1557.80 |
| 1.25 cm | 153 450 | 102.78 | 236.04 | 1557.76 |
| Richardson (5/2.5/1.25) | | 102.83 (order 1.22) | 236.18 (order 1.25) | 1557.72 (order 0.96) |

The 5 cm base is within 0.3 % (duty) and 0.2 K (peak) of the extrapolated
values.

**Re-measured 2026-11-03 with the cavity convection (gh:#1173)**, the
table above being the radiation-only record:

| h | cells | A-MEAN950 RCCS kW | A-HELD RCCS kW | A-DECAY hottest K |
|---|---|---|---|---|
| Fig 2.5 itself | 819 | 102.82 | 235.91 | 1556.5 |
| 10 cm | 3 729 | 104.70 | 241.47 | 1555.0 |
| 5 cm (base) | 11 770 | 105.07 | 242.34 | 1555.11 |
| 2.5 cm | 41 580 | 105.25 | 242.82 | 1555.03 |
| 1.25 cm | 153 450 | 105.33 | 243.02 | 1554.99 |
| Richardson (5/2.5/1.25) | | 105.39 (order 1.23) | 243.17 (order 1.27) | 1554.94 (order 0.94) |

The same orders and the same 0.3 % / 0.2 K distance of the 5 cm base from
the extrapolated values.

The observed order is about 1, not the scheme's 2. That fits two
features of the model:

- property kinks (the hold at 300 deg C, the Breitbach floor at 250 deg C);
- non-smooth region corners (the cone steps, the re-entrant corners).

**This has not been demonstrated.** The run is gated behind `long-tests`
(about 7 min).

## Sensitivities (5 cm; one change each)

| Case | A-MEAN950 RCCS (all) kW | A-DECAY hottest / RCCS |
|---|---|---|
| base | 102.53 (112.59) | 1557.9 K / 183.84 kW |
| SR-TOPBOTTOM | 102.56 (112.64), +0.03 % | 1557.9 / 183.84 |
| HEADS-ADIABATIC | 111.92 (111.92) | 1559.1 / 200.00 |
| RCCS-SHIELD (plate at r 283.4, eps_eff 0.4252) | 94.51 (108.18), -7.8 %; RPV 224 deg C | 1567.9 / 177.71; RPV 310 deg C |
| N5-TAB23 (barrel r 191-194) | 105.11 (114.69), +2.5 % | 1553.9 / 184.67 |
| He 29.4 bar (SS-ASRUN pressure) | 102.53, < 0.01 % | — |
| COLD-HELD 523 K (inlet-like), hold300 vs SR-TOPBOTTOM | 49.27 vs 50.10 kW, +1.7 %; 1919 side cells < 300 deg C | — |

## Known flaws, carried openly (D5)

- **Region extents.** All are Uncertain, and several are Assumed (listed
  above). Fig 2.5 was not overlaid.
- **Missing transport.**
  - There is no natural convection: neither in the helium cavities, nor
    inside the bed, ~~nor in the air cavity~~ (**CORRECTED 2026-11-03,
    gh:#1173**: the air cavity has it now, beside the radiation; the
    helium cavities and the bed still do not here; the bed has it in the
    gh:#1151 transient core).
  - ~~The air cavity also carries no air conduction.~~ **CORRECTED
    2026-11-03 (gh:#1173):** conduction across the air gap is the
    convection correlation's floor (`Nu_L >= 1`).
  - The insulation is not modelled. Its properties are Unknown.
- **Homogenisation.** The channel homogenisation is parallel-path, which is
  an upper bound across the channels. It includes no radiation inside the
  channels.
- **Steel.**
  - The steel curves were digitised (Uncertain).
  - The bottom supports use the barrel curve, which is an assumption.
- **Fluence.** Zero fluence is assumed throughout.
- **Not run.**
  - A depressurised (DLOFC) helium pressure: Tab 2.8 does not cover 1 bar.
  - ~~Any comparison against the thermocouples. Those need the FPIC with gas
    flow (gh:#1150).~~ **DONE 2026-11-03:** gh:#1150,
    `verification_and_validation/htr10_rz_flow_fpic/README.md`.
