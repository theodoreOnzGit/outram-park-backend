# `bwr_mark1`: Unit 1's containment on the control-volume network, compared (2026-11-01, gh:#1097, gh:#1100)

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
   debris on the floor.** (gh:#1097's mapping; **since gh:#1100** the cavity,
   MCCI gases and slab are coupled and this cause is gone: see that section
   for what remains.) The network has no cavity model. While the drywell
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
   early. ~~**Not attributed by ablation**, which is stated as a doubt.~~
   **CORRECTED 2026-11-01:** attributed (gh:#1100 section): the old model's
   top layer, heated to 391 K by the fission products' decay heat, boils as
   the torus depressurises; every open-loop replay without either term
   closes the 24 h gap to within 0.3 %.
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

## gh:#1100: sembawang's cavity, FDI and drywell slab coupled onto the network (2026-11-01)

**What was coupled, reused rather than rewritten** (the physics stays in
sembawang, epic gh:#1091):

| sembawang code (reused) | MELCOR reference (cited, clean room) | how it reaches the network |
|---|---|---|
| `cavity::Cavity::{arrive, step}` (MCCI: one debris layer, ablation, Zr oxidation by the concrete's H2O and CO2, film boiling under water, radiation when dry, `cavity_phases`) | CAV RM §3.2–3.5, §3.10 | `step`'s upward heat into the drywell sump by `set_pool_heat` while water lies over the debris, else into the lumped structures; its steam, H2, CO, CO2 into the drywell atmosphere by `set_source` on a new port at the leak's height, at the ablation temperature (steam at IF97's enthalpy, region 5 above 1073 K) |
| `containment::lpme_fall` (moved out of `Containment::lpme`, arithmetic unchanged) over `fdi::Fall` | FDI RM §2.2 (Eqs 2-11 to 2-20); COR RM Eq 6-34 | the heat the falling debris gives up into the sump by `set_pool_heat`; the debris lands at the fall's temperature |
| `containment::dw_slab_step` (moved out of `Containment`, arithmetic unchanged) | the gap's conduction and radiation, a 1-D slab to the building (sembawang's own, \[A\] geometry) | on the network's lumped drywell structures, after their exchange with the atmosphere |

Ablations: `NetworkMechanisms::without_cavity` (back to gh:#1097's mapping),
`without_fdi`, `without_dw_slab`; all three on by default, pinned by
`the_coupled_physics_is_on_by_default`. Case B takes
`FUKUSHIMA_NETWORK_ABLATE=cavity,fdi,dw_slab`.

**Assumed for the coupling, stated:** water "over the debris" is what the
drywell sump holds beyond its 1 m³ fill \[A\] (a control volume cannot dry
its pool); the sump's heat is capped at what heats it to saturation and
boils that water, the rest radiated to the structures (the old model's
`boil_pedestal` caps the same boiling at its water and drops the rest);
the cavity and the fall see the start-of-step sump and drywell (explicit,
as the structures are). The FDI fall's heat warms the sump to saturation
before it boils (the old model boils at the pool's temperature, Eq 2-20):
the same energy, a different split between sensible heat and steam.

### Predictions, written before the first run

Case C (in the test's module docs, 9 to 11). Case B, the 30 h replay, all
three coupled (window: network − old as recorded in the gh:#1097 table →
predicted):

| window | gh:#1097 | predicted with the coupling | why |
|---|---|---|---|
| 0–4.9 h | +0.07 % … +4.4 % | moves by less than 0.5 % of `p_dw` | no debris; the slab takes little heat while the structures are within 50 K of it |
| 6–10 h | +18.6 % at 8 h, +8.6 % at 10 h | **falls** to within −5 % … +8 % at 8 h | the debris's heat now partly ablates concrete (`q_down`) and leaves the top by film boiling, as in the old model, instead of boiling the sump at the full decay power |
| 11–23 h | −14.9 % … −19.7 % | **rises** to within ±10 % at 14 and 18 h | the concrete gases (H2, CO, CO2, steam) now enter the drywell; the slab cools the structures, so `T_dw` falls from 640–680 K to within 40 K of the old model's 443–449 K, which works the other way but is outweighed |
| flange | never opens on the network | opens before 23 h, as the old model's does (21 h) | the drywell passes 0.75 MPa with the MCCI gases |
| 24 h, the vent | −40.9 % / −48.0 % | not attributed; sign unchanged | the vent difference is the pool's (cause 4), not the floor's |
| 26–30 h `T_pool` | 373 K against a 355 K mean | unchanged within 1 K | pool layers are still not on the network |

### Attribution runs, predictions written before they ran

Each remaining difference is attributed to `Containment` by ablating the
suspected mechanism **in the old model** (`FUKUSHIMA_OLD_ABLATE`; the plant
is recorded with it ablated, so both models see the same, ablated
snapshots) and, for the coupling itself, in the network
(`FUKUSHIMA_NETWORK_ABLATE`). Predicted:

- **Old model without `pool_layers`:** its mean `T_pool` after the vent
  rises to saturation, so |Δ`T_pool`| at 26–30 h falls from about 18 K to
  under 2 K; the 24 h Δ`p_ww` during the vent shrinks by at least half
  (an unlayered pool flashes as the network's does); before 5 h, under
  0.3 % movement (the old pool hardly stratifies then).
- **Old model without `fp_decay_follows`:** the decay heat of the fission
  products the old containment holds (airborne in the drywell, on its
  structures, in the pool), which the network does not carry, leaves the old
  model too; its pressures at 11–23 h fall by 2–6 % and the network−old gap
  there moves by as much toward zero; its pool mean falls by 1–3 K by 23 h.
- **Network without `dw_slab`:** `T_dw` at 11–23 h returns toward the
  gh:#1097 run's 600–680 K; the slab is what brings it down (sign:
  without the slab, hotter and, at fixed moles, higher pressure).

### Open-loop replay attribution, predictions written before it ran

The old-model `fp_decay_follows` ablation turned out to change the plant
itself (all decay heat then stays in the core: 2.5 times the concrete
ablated by 23 h), so it does not isolate the containment's term. Instead,
`FUKUSHIMA_OLD_REPLAY` replays the recorded snapshots, open loop, into fresh
old `Containment`s with one term removed from the snapshot, against the
same coupled network run (the plain replay is asserted bit-equal to the
recording). Predicted, network − old:

- **old without the fission products' decay heat (`q_*_w` = 0):** the
  old pool mean at 23 h falls from 363.5 K to within 5 K of the network's
  345.6 K; the 11–23 h `p_ww` gap shrinks from about −13 % to about −7 %.
- **old without `pool_layers`:** the 11–23 h gap about −5 % to −7 % (as the
  closed-loop ablation gave), the vent's −48 % at 24 h gone.
- **old without both:** the 11–23 h gap within ±3 %; `T_pool` within 3 K
  throughout, including after the vent.

### Results (2026-11-01)

**Case C, debris on the floor** (predictions 9–11 all held):

| | network | old |
|---|---|---|
| dry: concrete ablated, kg | 1258.3 | 1258.3 |
| dry: H2 / CO made, kg | 8.251 / 12.012 | 8.251 / 12.012 |
| dry: debris, K | 2774.9 | 2774.9 |
| dry: worst \|Δ`p_dw`\| | 0.03 % | |
| wet: FDI steam, kg | 27.7 | 27.6 |
| wet: concrete ablated, kg | 1240.3 | 1240.3 |
| wet: worst \|Δ`p_dw`\| | 3.46 % (not predicted; the sump boils from saturation and the 6000 kg poured displaces the drywell gas) | |

With `without_cavity().without_dw_slab()` the network reproduces gh:#1097's
6 h worsts exactly (60.39 %, 9.78 %, 1.32 K): prediction 11 held, and the
84 `bwr_mark1` lib tests, the byte-for-byte fixture among them, pass after
the `lpme_fall` and `dw_slab_step` refactors.

**Case B, the 30 h replay, coupled (default) against gh:#1097's mapping**
(network − old; `T_dw` and `T_pool` network / old):

| t (h) | Δ`p_dw` gh:#1097 → coupled | Δ`p_ww` gh:#1097 → coupled | `T_dw` (K) coupled | `T_pool` (K) coupled (old mean, top) | prediction |
|---|---|---|---|---|---|
| 3 | +1.34 → +1.30 % | +1.33 → +1.29 % | 333.0 / 333.1 | 325.45 / 325.41 | held (< 0.5 %) |
| 4 | +2.21 → +1.00 % | +1.64 → +0.36 % | 376.1 / 380.5 | 332.11 / 332.12 | **MISSED** (moved 1.2 %) |
| 5 | +4.34 → +1.07 % | +4.37 → +1.17 % | 432.7 / 443.1 | 339.78 / 340.43, 343.21 | **MISSED** (moved 3.3 %) |
| 6 | +9.39 → −5.78 % | +5.33 → −5.86 % | 405.4 / 436.7 | 339.72 / 341.58, 345.55 | |
| 8 | +18.6 → −12.89 % | +18.7 → −11.35 % | 407.9 / 434.9 | 339.65 / 344.00, 350.46 | sign held, magnitude **MISSED** (predicted −5 … +8 %) |
| 10 | +8.6 → −13.83 % | +8.4 → −14.46 % | 416.7 / 437.3 | 339.95 / 346.95, 356.42 | |
| 14 | −14.9 → −12.83 % | −15.4 → −13.31 % | 432.8 / 443.3 | 342.39 / 354.47, 371.45 | **MISSED** (predicted ±10 %); `T_dw` within 40 K held |
| 18 | −18.9 → −12.47 % | −19.7 → −13.08 % | 438.3 / 445.4 | 344.26 / 359.10, 380.74 | as 14 h |
| 23 | −16.3 → −12.11 % | −18.5 → −14.15 % | 442.1 / 448.9 | 345.55 / 363.47, 389.54 | as 14 h |
| 24 | −40.9 → −41.46 % | −48.0 → −48.00 % | 423.6 / 432.3 | 350.85 / 364.20, 391.01 | held (sign unchanged) |
| 26 | +2.27 → +1.75 % | −0.53 → −0.53 % | 415.8 / 440.7 | 353.84 / 355.07, 373.29 | **MISSED** (`T_pool` 370.38 → 353.84) |
| 30 | +3.06 → +2.21 % | +0.02 → −0.26 % | 407.2 / 431.9 | 358.74 / 354.96, 373.18 | **MISSED** (373.14 → 358.74) |

- **The coupled cavity agrees with the old one**: concrete ablated 64 565
  against 64 388 kg at 23 h (+0.27 %) and 86 746 against 86 534 kg at 30 h;
  H2 274.0 and CO 398.9 kg in both (the Zr is exhausted in both by 18 h); the
  debris within 3 K throughout. The FDI fall raised 55 kg of steam against
  the old model's 75 kg (the water over the debris at its arrival differs;
  not attributed further, 20 kg).
- **The flange still never opens on the network** (peak 711.5 kPa at 23.5 h;
  the old model opens it at about 17.5 h): prediction **MISSED**. The
  building gets no hydrogen on the network; the old model's reaches 3.73 %.
- **`T_dw`** came down from 600–680 K to within 7–27 K of the old model
  (the slab; see the attribution).
- **Worst over 30 h:** |Δ`p_dw`| 56.29 % (the 4.956 h step, cause 2, now
  lower because the slab has cooled the structures), |Δ`p_ww`| 55.00 % (the
  vent), |Δ`T_pool`| 18.31 K. **Over the default 6 h:** 56.29 %, 13.65 %,
  1.86 K, pinned in the test as set after the run (gh:#1097's 60.39 %,
  9.78 %, 1.32 K stay pinned for the ablated configuration).
- **Cost:** 1029 s for the 30 h with the network alone on the machine
  (9.5 ms a step, the replays included), 1187 s with three other runs
  beside it; gh:#1097's mapping took 840 s. 953 halved steps by 30 h
  against 588.

**Attribution runs, outcomes against the predictions above:**

- *Network without `dw_slab`* (closed loop unchanged): 4 h and 5 h return
  exactly to gh:#1097's +2.21 % and +4.34 %, so the early movement (the
  missed 0–4.9 h prediction) is the slab: it draws heat out of structures
  at 360–410 K from the start. At 11–23 h the gap becomes −8.9 %, −2.2 %,
  −0.5 % with `T_dw` 494–502 K against the old model's 443–449 K: sign held
  (hotter and higher without the slab), magnitude partly (500 K, not
  600–680 K: the cavity's ablation and film boiling now take heat that
  gh:#1097 put into the structures).
- *Old model without `fp_decay_follows`, closed loop*: **not a valid
  instrument**, so its prediction is void rather than missed: the ablation
  keeps all decay heat in the core, the debris ablates 151 t of concrete by
  23 h instead of 64 t, and the old drywell reaches 986 kPa. Replaced by the
  open-loop replay.
- *Old model without `pool_layers`, closed loop*: the vent's −48 % at 24 h
  became −0.27 % (held); the 11–23 h gap −4.2 … −6.8 % (held, roughly);
  |Δ`T_pool`| after the vent 15–20 K (**MISSED**, predicted under 2 K: the
  old unlayered pool sits at saturation, the network's below it, for want
  of the decay heat below); 2 % movement at 5 h (**MISSED**, predicted
  under 0.3 %).
- *Open-loop replays* (the same coupled network run against old
  containments replayed on the recorded snapshots; the plain replay is
  bit-equal to the recording, asserted):

| t (h) | old as run | old, no FP decay heat | old, no layers | old, neither |
|---|---|---|---|---|
| 5 | +1.17 % | +2.79 % | +1.48 % | +3.06 % |
| 6 | −5.86 % | +2.47 % | −5.01 % | +3.12 % |
| 8 | −11.35 % | +3.10 % | −9.56 % | +3.96 % |
| 10 | −14.46 % | +3.33 % | −11.51 % | +4.22 % |
| 14 | −13.31 % | +3.75 % | −5.76 % | +5.35 % |
| 18 | −13.08 % | +3.69 % | −4.24 % | +6.04 % |
| 23 | −14.15 % | +3.87 % | −6.49 % | +6.66 % |
| 24 (vent) | −48.00 % | −0.04 % | −0.28 % | +0.02 % |
| 30 | −0.26 % | −0.01 % | −0.18 % | +0.00 % |

(Δ`p_ww`, network − old.) `T_pool` at 23 h: network 345.55 K against
363.47 (as run), 344.81 (no FP heat), 366.63 (no layers), 345.18 K
(neither); at 30 h 358.74 against 354.96, 354.65, 373.17, 355.73 K.
`T_dw` at 8 h: 407.9 against 434.9 (as run), 406.4 K (no FP heat).

Outcomes: *no FP heat*: `T_pool` within 5 K at 23 h held (0.74 K); the gap
"about −7 %" **MISSED**: the decay heat carries all of it and more (it
flips to +3.9 %). *No layers*: held (−4.2 … −6.5 %, the vent gone). *Neither*:
within ±3 % **MISSED** (+5.4 … +6.7 %: I left out gh:#1097's cause 1, the
displacement, which grows with the torus level: 0.226 m by 23 h, 160 m³ of
the 2620 m³, 6.1 %); `T_pool` within 3 K held to 26 h (2.89 K), 3.01 K at
30 h (**MISSED** narrowly). The two old-model terms do not add: with the
decay heat present the layers raise the old pressure by 6–9 %, without it
by 2–3 %, because the old model puts the pool's decay heat into its top
layer (`pool_energy`, the layer above the downcomers' exits), which is how
its top reaches 389 K.

### Every remaining difference, attributed to `Containment` (coupled network, 30 h)

| window | remaining Δ (network − old) | attributed to | evidence |
|---|---|---|---|
| 0–4.5 h | +0.1 … +1.3 % | the pool displacing the torus gas (network physics; the old model's gas space is fixed) | gh:#1097 cause 1, case A |
| 4.956 h | +56 % for one step | the network's start-of-step equalising limit on the downcomers (numerical) and its inertial paths (physical) | gh:#1097 cause 2, unchanged |
| 5–23 h | −4.8 … −14.5 % | **the old containment's fission-product decay heat** (`q_noble_w`, `q_alkali_w`, `q_halogen_w`, `q_chalcogen_w` into the drywell gas, its structures and the pool) — about −17 … −18 % — which the old model's **pool layers** amplify by putting the pool's share into the top layer (a further 2–9 %), against the network's displacement (+3 … +7 %) | open-loop replays: without the decay heat +2.5 … +3.9 %, without both +3.1 … +6.7 %, which is the displacement |
| 5–10 h `T_dw` | network 21–31 K low | the same decay heat (its drywell share) | no-FP-heat replay: within 2 K |
| 11–23 h flange | network never opens | the same decay heat: without it the old drywell peaks at 680 kPa and its flange would not open either | no-FP-heat replay |
| 24 h, the vent | −48 % | the old model's top layer, heated to 391 K by that decay heat, boiling as the torus depressurises | every replay: −0.3 … +0.0 % (gh:#1097 cause 4, now attributed) |
| 26–30 h `T_pool` | network 353.8–358.7 K, old mean 355 K, old top 373 K | layers (top against mean) and decay heat (mean), partly cancelling | replays: 350.3–355.7 K without both terms, within 3 K |
| 24–30 h `p_dw` | +1.7 … +2.4 % | the network's gas column above the torus and its submergence at its own density | gh:#1097 cause 6; persists in every replay |
| after 24 h `T_dw` | network 10–25 K low | about half the decay heat (no-FP-heat replay: 10 K left), half not attributed | |
| 6–23 h drywell H2 | network up to 57 kg higher | **not attributed** (doubt): less gas pushed through the downcomers at the network's lower pressure is the leading hypothesis, untested | |
| FDI steam | 55 against 75 kg | the water over the debris when it arrives (the network's sump beyond its \[A\] fill against the old pedestal) | not tested further |

**So the remaining differences are, in order of size: the fission products'
decay heat and the pool layers (not yet on the network; together they set
the 5–23 h gap, the flange, the vent and the pool temperature), the
displacement and gas column (network physics), and the one-step
blowdown lag (numerical).**

### What remains of gh:#1100 (not done in this change)

1. **The fission products' decay heat in the volumes**, the largest
   remaining difference. The old model deposits `q_*_w` by where its
   `RnVolume`s hold each class (airborne, on structures, scrubbed in the
   pool). The network cannot place that heat until the fission products ride
   its paths: `rn_volume::RnVolume` (and the noble-gas, CO/CO2 tracers) need
   advecting with each path's transferred gas each step (the network's
   `NetworkStepReport::transfers` already give the per-path masses), plus
   the relief and leak arrivals and SPARC-90's scrubbing at the submerged
   ports. Then the decay heat couples by `set_atmosphere_heat` (airborne),
   the structures' temperature (deposited) and `set_pool_heat` (pool).
   Aerosol deposition, iodine and the building inventory come with it.
2. **The pool layers**, second. A control volume holds one pool at one
   temperature, so this cannot be expressed with the existing sources and
   sinks without misstating either the level or the surface temperature: a
   deep layer held outside the volume loses its share of the level (the
   V(z) geometry), and one held inside it shares the volume's heating. It
   needs a general addition to `tampines::control_volume` (the module's
   "Not modelled" list names pool stratification): an optional split
   elevation, inflows at ports below it into the lower layer (the bubble
   efficiency at its temperature), outflows by port elevation, the surface,
   flashing and pool heat on the upper layer, mixing on inversion, after
   `Containment::layer_pool` (and the PPOOLEX / Li–Kudinov physics it
   cites). Not started: it is a change to a shared tampines type that needs
   its own V&V.
3. **Burns in the building** (`step_burn`, BUR RM §2.3–2.5): the network's
   building would need its hydrogen and CO removed and the oxygen consumed;
   `ControlVolumeInputs` has a steam sink but no non-condensable sink, so
   this needs a general tampines addition too. It has no effect on the
   drywell or torus here (the old building is held at one atmosphere) and
   the network's building receives no hydrogen yet (the flange stays shut).
4. **The \[A\] sump fill** stays: the coupling uses the water beyond it as
   the water over the debris. A control volume that can dry its pool would
   remove the assumption.

## What would move if the plant switched (the decision's evidence)

Switching `Plant` to the network re-blesses `headless_30h.csv` and moves:

- every containment pressure, from the first relief (displacement: +1 % to
  +4 % before the lower head fails);
- the drywell's peak at the vessel's blowdown (sharper on the network);
- after lower-head failure, everything that depends on the ex-vessel
  mapping, ~~**which the network does not yet carry** (cavity, MCCI gases, FDI,
  the drywell slab)~~ (**CORRECTED 2026-11-01**: carried since gh:#1100, its
  cavity agreeing with the old one to 0.3 %; what is still missing after
  lower-head failure is the fission products' decay heat in the volumes and
  the pool layers);
- the vent's depressurisation and the pool's temperature after it;
- `compare::the_uncalibrated_baseline_is_pinned` and the BSAF comparisons,
  through the drywell pressure.

The fission products, aerosols, iodine and burns cannot be compared yet:
the network carries none of them. **The building's hydrogen differs in
kind.** The old model's drywell passes the flange's 0.75 MPa opening
pressure (786 kPa at 21 h, 805 kPa at 23 h), so its flange leaks hydrogen
into the building. The network's drywell peaks at 674 kPa, so its flange
never opens and its building gets no hydrogen. ~~This difference comes from
cause 3 (no MCCI gases on the network)~~ **CORRECTED 2026-11-01:** with the
MCCI gases coupled (gh:#1100) the network's drywell peaks at 711.5 kPa and
its flange still does not open; the open-loop replay attributes the
difference to the old containment's fission-product decay heat (without it
the old drywell peaks at 680 kPa). It would move the explosion timing.

**Recommendation (for the maintainer, not acted on):** do not switch yet. The
network agrees with the old model where they model the same physics, and its
differences there are physical (displacement, IF97, inertia). After
lower-head failure, the gap is mapping that the old model has and the network
lacks. The cost is about 75 times the old model's, which matters for the web
demo. Prerequisites: the cavity and MCCI gases and the drywell slab on the
network; fission products carried by the paths; the start-of-step lag fixed in
`cv_network`; and the vent difference attributed. (**2026-11-01, gh:#1100:**
the cavity, MCCI gases and slab are on the network and the vent difference
is attributed, to the old model's decay-heated top layer; the fission
products' decay heat in the volumes and the pool layers remain, and they
are now the largest differences.)

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

- ~~The vent's faster depressurisation on the network (cause 4) is not
  attributed by ablation.~~ **CORRECTED 2026-11-01:** attributed by the
  open-loop replays (gh:#1100 section): the old model's top layer, heated
  to 391 K by the fission products' decay heat.
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
