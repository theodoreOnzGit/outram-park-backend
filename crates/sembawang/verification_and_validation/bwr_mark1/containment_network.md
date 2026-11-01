# `bwr_mark1`: Unit 1's containment on the control-volume network, compared (2026-11-01, gh:#1097)

> Research, education and V&V only. AI draft, unreviewed.

A code-to-code comparison of two containment models in this workspace. It is
not a validation: neither model is compared with plant data here. Its purpose
is the evidence for one decision the maintainer has to make: whether
`bwr_mark1::Plant` should switch from `containment::Containment` to
`containment_network::ContainmentNetwork`. **The default path is unchanged**,
and no fixture was re-blessed.

## Methodology

**The two models.**

- *Old*: `containment::Containment`. Drywell, wetwell gas and pool as lumped
  inventories with ideal steam and a constant-`c_p` pool; orifice flows capped
  by the isothermal equalising amount; a fixed 2620 m³ wetwell gas space; a
  two-layer pool; MCCI, FDI, burns, fission products and aerosols.
- *Network*: `containment_network::ContainmentNetwork`. `tampines::cv_network`
  with three `ControlVolume`s (drywell, torus, reactor building), five
  `FlowPath`s (downcomers, vacuum breakers, wetwell vent, head-flange leak,
  building openings) and the environment as a boundary. It uses IF97 steam and
  liquid, a V(z) table per volume (so the pool displaces the gas), MELCOR's
  two-velocity paths with inertia and choking, and an implicit (Picard) step.
  The drywell structures and torus shell are lumped walls with the same MELCOR
  surface correlation as the old model, explicit. The module docs list what is
  mapped from the `ContainmentSnapshot` and what is not, along with every
  geometric assumption (\[A\]).

**What `tampines` gained for this** (its V&V doc,
`crates/tampines/verification_and_validation/cv_network.md`): external
sources on ports, atmosphere heat and a steam sink (the caller's structures),
valve open fractions on paths, and two defect fixes found on this model. The
first: vacuum breakers are now decided once per step from the start-of-step
pressures. Before, a breaker never opened. The second: the equalising limit
now uses the junction pressures with the liquid head and gas column. Before,
the downcomers did not converge at the submergence.

**Cases** (`crates/sembawang/tests/containment_network_vs_containment.rs`;
the predictions are in its module docs, written before the first run):

- **A.** Relief steam into the torus: 10 kg/s at 2.7 MJ/kg for 600 s at
  1 s. The old model runs with its wetwell atmosphere, `two_sided_equalising`,
  `outflow_enthalpy`, `gas_cv_t` and `dw_wall_hs`, and without pool layers or
  the shell. This is the wet-well case of `wet_well_vs_tampines.rs`.
- **B.** The Unit 1 historical sequence. `Plant::new(Mode::Historical)` runs
  with the old containment, and every `ContainmentSnapshot` is recorded and
  replayed into the network. This is open loop: the vessel never sees the
  network's drywell. The default is 6 h; `FUKUSHIMA_NETWORK_HOURS=30` gives
  the whole scenario.

**Instrument.** Pressures, temperatures, levels and path flows from both
models at the same step. The network's own ablations
(`NetworkMechanisms`) and the prismatic torus attribute the differences.

## Results

### A. Relief steam into the torus (all at 600 s)

| Network configuration | `p_ww` − old | worst \|Δ`T_pool`\| |
|---|---|---|
| prismatic torus (sembawang's 750 m²), all on | **+341.1 Pa** | 2.87e-3 K |
| torus V(z) (711 m² at the level), all on | +328.8 Pa | 2.89e-3 K |
| prismatic, without pressure work | +280.7 Pa | 2.92e-3 K |
| prismatic, constant `c_p` liquid only | +340.5 Pa | 4.27e-5 K |
| prismatic, ideal steam and constant `c_p` | +349.3 Pa | 4.24e-5 K |
| prismatic, without level swell | +341.1 Pa | — |

Other results: at rest the torus reads −8.3 Pa; `p_dw` is +6.4 Pa; neither
the downcomers nor the vacuum breakers flow; at most 2 Picard iterations.

**Each difference, with its cause:**

- **+280 Pa: the pool displaces the gas.** Injecting 6000 kg adds about 6 m³
  of water, shrinking a 2620 m³ gas space by 0.23 %. Isothermal, that is
  about 240 Pa. The old model holds its gas volume fixed. The effect depends
  on the water's volume, not on the pool's area, so the torus shape changes it
  by only −12 Pa.
- **+60 Pa: the pool's compression work heats the gas.** Measured by the
  pressure-work ablation.
- **−8.3 Pa: IF97 against ideal steam.** Both start from the same saturated
  steam mass, and IF97's real gas reads lower. The ideal-steam ablation gives
  exactly 105 000.00 Pa at rest, and +349.3 Pa at 600 s, which is the wet
  well's figure to 0.1 Pa.
- **2.87e-3 K: IF97 liquid against constant `c_p`.** This falls to 4.3e-5 K
  with constant `c_p`.
- **Level swell: no effect here.** The swell moves the draw level only, and
  every port draws the phase it already did.

**Prediction outcomes:**

- `p_ww` band: held.
- `T_pool` within 2e-3 K: **MISSED**, because the wet well I reasoned from
  ran with constant `c_p`.
- Torus band: held, but by wrong reasoning.
- No flow: held.
- Without pressure work within ±100 Pa: **MISSED**. I confused the geometric
  compression with the pressure-work term.
- Fewer than 30 iterations: held.

The pins that replace the missed bounds were set after the run and are
labelled so in the test.

### B. The Unit 1 sequence, 30 h (network / old)

| t (h) | `p_dw` (kPa) | Δ | `p_ww` (kPa) | Δ | `T_dw` (K) | `T_pool` (K) (old: mean, top) | what drives it |
|---|---|---|---|---|---|---|---|
| 1 | 107.9 / 107.8 | +0.07 % | 105.0 / 105.0 | −0.01 % | 331.9 / 331.9 | 303.15 / 303.15 | isolation condensers; heat from the RPV's insulation |
| 2 | 108.2 / 108.1 | +0.15 % | 110.4 / 108.8 | +1.48 % | 332.6 / 332.6 | 315.62 / 315.60 | relief valves |
| 3 | 110.7 / 109.2 | +1.34 % | 114.1 / 112.6 | +1.33 % | 333.3 / 333.1 | 325.45 / 325.41 | vacuum breakers open |
| 4 | 194.4 / 190.2 | +2.21 % | 181.3 / 178.4 | +1.64 % | 377.7 / 380.5 | 332.12 / 332.12 | RPV leak into the drywell; downcomers |
| 5 | 448.7 / 430.0 | +4.34 % | 452.3 / 433.4 | +4.37 % | 437.2 / 443.1 | 340.06 / 340.43, 343.21 | the vessel's blowdown at 4.956 h |
| 6 | 458.3 / 419.0 | +9.39 % | 444.9 / 422.4 | +5.33 % | 418.1 / 436.7 | 340.25 / 341.58, 345.55 | debris ex-vessel |
| 8 | 553.3 / 466.4 | +18.6 % | 539.5 / 454.7 | +18.7 % | 430.2 / 434.9 | 343.77 / 344.00, 350.46 | ex-vessel heat boils the drywell sump |
| 10 | 570.7 / 525.6 | +8.6 % | 557.2 / 513.8 | +8.4 % | 487.6 / 437.3 | 344.90 / 346.95, 356.42 | sump dry; heat into the structures |
| 14 | 582.3 / 683.9 | −14.9 % | 568.7 / 672.1 | −15.4 % | 642.8 / 443.3 | 345.60 / 354.47, 371.45 | the old model's MCCI gases |
| 18 | 618.8 / 763.3 | −18.9 % | 604.6 / 753.2 | −19.7 % | 625.7 / 445.4 | 353.14 / 359.10, 380.74 | |
| 23 | 674.1 / 805.3 | −16.3 % | 659.4 / 808.8 | −18.5 % | 599.2 / 448.9 | 364.86 / 363.47, 389.54 | |
| 24 | 116.5 / 197.1 | −40.9 % | 101.4 / 194.9 | −48.0 % | 534.9 / 432.3 | 369.17 / 364.20, 391.01 | the wetwell vent opens |
| 26 | 116.2 / 113.6 | +2.27 % | 101.3 / 101.8 | −0.53 % | 669.1 / 440.7 | 370.38 / 355.07, 373.29 | vented, near 1 atm |
| 30 | 116.8 / 113.3 | +3.06 % | 101.6 / 101.6 | +0.02 % | 593.1 / 431.9 | 373.14 / 354.96, 373.18 | |

Worst over the 30 h: |Δ`p_dw`| 60.39 % (one step at 4.956 h), |Δ`p_ww`|
54.93 % (during the vent), |Δ`T_pool`| 18.20 K.

Worst over the default 6 h: 60.39 %, 9.78 % and 1.32 K. The test pins these,
labelled as set after the run.

**Each difference, with its cause:**

1. **0–4.5 h, network 0.1 % to 4.4 % high: the same displacement as in A.**
   By 4.5 h the pool has risen 0.15 m (about 110 m³ of condensate in a 711 m²
   torus), compressing the torus gas by 4 %. The old model's 2620 m³ is
   fixed. The drywell follows through the breakers and downcomers. I
   predicted the network would be *lower*, from the old model's stratified
   pool. **MISSED**: the old pool hardly stratifies before 5 h (its mean and
   top agree to 0.05 K), and the displacement dominates.
2. **4.956 h, a one-step spike of +60 % in `p_dw` (562 against 350 kPa).**
   The vessel blows down at 2149 kg/s into the drywell. The old model adds the
   leak and then runs its downcomers within the same step. In the network, the
   downcomers' equalising limit is taken from the start-of-step state, when
   the submergence still held them shut, so no gas moves until the next step.
   This is the same first-order lag recorded for the vacuum breaker in
   `cv_network.md`. For about a minute afterwards the network's drywell stays
   40–60 kPa higher: its downcomers carry 1479 kg/s, rising from rest through
   8 m of inertia \[A\] and choking. The old orifices land on the equalising
   amount at once (`FUKUSHIMA_NETWORK_TRACE=4.955,4.961` prints it step by
   step). Physical cause: MELCOR's two-velocity, inertial paths against
   sembawang's equalising orifices. Numerical cause: the limit's start-of-step
   state.
3. **6–10 h, network up to 18.7 % high, then 11–23 h up to 19.7 % low: the
   debris on the floor.** The network has no cavity model. While the drywell
   sump holds more than its initial fill, the ex-vessel heat boils it, which
   raises steam faster than the old model's MCCI, whose heat partly ablates
   concrete. Once the sump is back to its fill, the heat goes into the lumped
   structures. These have no outer slab in the network (`dw_wall_outer` is
   unmapped), so they heat the drywell gas to 640–680 K, against 443 K in the
   old model. From about 11 h the old model's concrete gases (CO, CO2, H2,
   steam from MCCI), absent from the network, carry its pressure above the
   network's.
4. **24 h, the vent: −48 %.** Both vent once the operators order it and the
   disk bursts. The network's torus falls to 101.4 kPa within the hour; the
   old one is at 194.9 kPa at 24.0 h and 110.0 kPa at 25 h. The network flashes
   its single, well-mixed 369 K pool as the pressure falls. The old model vents
   with its `mixture_vent` steam limit and its two-layer pool. The vent itself
   (0.07 m², 60 m inertia \[A\], choked) is not the cause: both are choked
   early. **Not attributed by ablation**, which is stated as a doubt.
5. **After the vent, `T_pool` 18 K apart.** The network's single pool sits at
   saturation (373 K). The old model's *mean* is 355 K, but its top layer is
   373.2 K: the network has no pool layers. This is a model difference, not a
   defect.
6. **After the vent, `p_dw` +2–3 %.** The network's drywell sits one
   submergence head (11.8 kPa at its density) plus the gas column above the
   torus. The old model's sits 11.8 kPa above, with its fixed depth and no gas
   column.

**Timing.** The network costs 2.97 ms a step over 5 h, 3.08 ms over 6 h,
and **7.78 ms over 30 h: 840 s for the scenario**, against about 11 s for the
whole plant with the old containment. It needed one halved step at 4.9 h, 4
by 11 h, 171 by 25 h and 588 by 30 h. These are Picard steps that did not
converge in 100 iterations and were cut by `advance`, up to 6 halvings, as
MELCOR's executive cuts its step. At most 43 iterations in a step.
Prediction 8 (under 5 ms a step) **MISSED**.

## What would move if the plant switched (the decision's evidence)

Switching `Plant` to the network re-blesses `headless_30h.csv` and moves:

- every containment pressure, from the first relief (displacement: +1 % to
  +4 % before the lower head fails);
- the drywell's peak at the vessel's blowdown (sharper on the network);
- after lower-head failure, everything that depends on the ex-vessel
  mapping, **which the network does not yet carry** (cavity, MCCI gases, FDI,
  the drywell slab);
- the vent's depressurisation and the pool's temperature after it;
- `compare::the_uncalibrated_baseline_is_pinned` and the BSAF comparisons,
  through the drywell pressure.

The fission products, aerosols, iodine and burns cannot be compared yet:
the network carries none of them. **The building's hydrogen differs in
kind.** The old model's drywell passes the flange's 0.75 MPa opening
pressure (786 kPa at 21 h, 805 kPa at 23 h), so its flange leaks hydrogen
into the building. The network's drywell peaks at 674 kPa, so its flange
never opens and its building gets no hydrogen. This difference comes from
cause 3 (no MCCI gases on the network), and it would move the explosion
timing.

**Recommendation (for the maintainer, not acted on):** do not switch yet. The
network agrees with the old model where they model the same physics, and its
differences there are physical (displacement, IF97, inertia). After
lower-head failure, the gap is mapping that the old model has and the network
lacks. The cost is about 75 times the old model's, which matters for the web
demo. Prerequisites: the cavity and MCCI gases and the drywell slab on the
network; fission products carried by the paths; the start-of-step lag fixed in
`cv_network`; and the vent difference attributed.

## Units 2 and 3 by configuration (a sketch; no plant data invented)

The network reduces a unit to `Params` (volumes, areas, set points, the
snapshots' sources) plus `NetworkConfig` (shapes, elevations, sump pits,
inertial lengths). Units 2 and 3 are BWR/4s in Mark I containments, with
RCIC and HPCI instead of isolation condensers, which lives in the vessel
model and not here. A unit would be
`ContainmentNetwork::with_config(&Params::unit2(), NetworkConfig::unit2(), ..)`.
Neither constructor exists yet, because the data below is not held. It would
need, from open literature only (SAND2012-6173, the Japanese government's
June 2011 report to the IAEA, IAEA Technical Volume 1, TEPCO's public plant
data; not the restricted BSAF specification):

- drywell free volume and wetwell gas and pool volumes (the June 2011
  report's attachment gives them per unit, as it does Unit 1's 2620/1750 m³);
- torus major and minor radii, or the pool area at the normal level;
- downcomer count, area and submergence; vacuum-breaker count, area and
  set point;
- the wetwell (and, for Unit 2, the drywell) vent line's area and the
  rupture disk's pressure; the operators' vent orders (the sequence, in
  `Mode::Historical`);
- the drywell's elevation over the torus, the reactor building's free volume
  and its openings (Unit 2's blowout panel opened; Unit 3's building
  exploded);
- the drywell structures' areas and masses (SAND2012-6173's heat-structure
  inventory).

Unit 1's own \[A\] entries need the same data.

## Doubts, not settled here

- The vent's faster depressurisation on the network (cause 4) is not
  attributed by ablation.
- The start-of-step equalising limit lags by one step on breakers and
  downcomers. A limit from the latest iterate, or a Newton step, would remove
  it.
- 588 halvings by 30 h: the Picard step struggles after the vent. The
  failures were not classified (which path, which switch).
- The drywell's sump pit, the light bulb drawn as a prism, and the drywell
  floor at the torus top are \[A\]. The +90 Pa gas column they imply in the
  downcomers is physical but unverified.
- The building is a control volume open to the environment through 1 m²
  \[A\], where sembawang holds it at one atmosphere exactly.
