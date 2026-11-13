# `bwr_mark1`: Unit 1's containment on the control-volume network, compared (2026-11-01, gh:#1097, gh:#1100)

> Research, education and V&V only. AI draft, unreviewed.

A code-to-code comparison of two containment models in this workspace. It is
not a validation: neither model is compared with plant data here. Its purpose
is the evidence for one decision the maintainer has to make: whether
`bwr_mark1::Plant` should switch from `containment::Containment` to
`containment_network::ContainmentNetwork`. ~~**The default path is unchanged**,
and no fixture was re-blessed.~~ **CORRECTED 2026-11-02:** the plant switched
to the network (gh:#1099) and the fixture was re-blessed; see "gh:#1099".

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
its pool; ~~still~~ **CORRECTED 2026-11-01:** since `tampines`' dry-out the
sump starts dry and all its water is over the debris, the 1 m³ fill kept
only in `NetworkConfig::unit1_gh1097`); the sump's heat is capped at what heats it to saturation and
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

(The state after the first gh:#1100 change. **Superseded 2026-11-01** for
the default configuration by the table in the next section, once the
fission products were coupled; it still describes
`without_fission_products`.)

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

**So the remaining differences are, in order of size: ~~the fission products'
decay heat and~~ the pool layers (~~not yet on the network; together they set
the 5–23 h gap, the flange, the vent and the pool temperature~~ **CORRECTED
2026-11-01:** the decay heat is now on the network, which closes the 5–23 h
gap to −3.8 … +2.5 % and opens the flange; the layers still set the vent and
the pool's mean temperature, next section), the
displacement and gas column (network physics), and the one-step
blowdown lag (numerical).**

### What remains of gh:#1100 (not done in this change)

1. ~~**The fission products' decay heat in the volumes**, the largest
   remaining difference. The old model deposits `q_*_w` by where its
   `RnVolume`s hold each class (airborne, on structures, scrubbed in the
   pool). The network cannot place that heat until the fission products ride
   its paths: `rn_volume::RnVolume` (and the noble-gas, CO/CO2 tracers) need
   advecting with each path's transferred gas each step (the network's
   `NetworkStepReport::transfers` already give the per-path masses), plus
   the relief and leak arrivals and SPARC-90's scrubbing at the submerged
   ports. Then the decay heat couples by `set_atmosphere_heat` (airborne),
   the structures' temperature (deposited) and `set_pool_heat` (pool).
   Aerosol deposition, iodine and the building inventory come with it.~~
   **CORRECTED 2026-11-01: done** (next section): `FpHold`, moved out of
   `Containment`, rides the paths' gas transfers with SPARC at the
   downcomers and quenchers, and its decay heat couples by
   `set_atmosphere_heat`, the lumped structures and `set_pool_heat`.
2. ~~**The pool layers**, second. A control volume holds one pool at one
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
   its own V&V.~~ **CORRECTED 2026-11-01: done** — `tampines` gained the
   layered pool (d870c66cb, its own V&V L1–L8) and the network uses it
   (section "the layered torus pool and dry sumps" below).
3. ~~**Burns in the building** (`step_burn`, BUR RM §2.3–2.5): the network's
   building would need its hydrogen and CO removed and the oxygen consumed;
   `ControlVolumeInputs` has a steam sink but no non-condensable sink, so
   this needs a general tampines addition too. It has no effect on the
   drywell or torus here (the old building is held at one atmosphere) and
   the network's building receives no hydrogen yet (the flange stays shut).~~
   **CORRECTED 2026-11-01: done** (section "the surface block, the decay
   heat above the exits, and burns").
4. ~~**The \[A\] sump fill** stays: the coupling uses the water beyond it as
   the water over the debris. A control volume that can dry its pool would
   remove the assumption.~~ **CORRECTED 2026-11-01: removed** — `tampines`'
   pools may be empty (d870c66cb); the sumps start dry and the cavity
   branches on `is_pool_dry()` (section below).

## gh:#1100 (continued): the fission products and their decay heat on the network (2026-11-01)

**What was coupled, reused rather than rewritten.** The old model's
fission-product code moved out of `Containment` into
`containment_fp::FpHold`, **arithmetic unchanged** (the byte-for-byte
`headless_30h.csv` fixture and `three_workers_are_one_worker` are the
evidence), and both models now call it:

| `FpHold` (moved from `Containment`) | MELCOR reference (cited, clean room) | on the network |
|---|---|---|
| `arrive` (the relief flow's scrubbed at the quenchers by SPARC-90's condensation, its iodine by the partition limit; the leaks' into the drywell) | RN RM §2.7.1, Eqs D-5 to D-7 | from the snapshot, against the torus's start-of-step pool and pressure |
| `downcomer` (condensation, impaction and swarm rise per section, I2's partition and swarm rise) | RN RM §2.7.1, Appendix F | the downcomers' mean gas mass over the step, its composition the drywell's end-of-step mixture |
| `ww_to_dw`, `vent`, `flange` | (transport in the donor's fractions) | the vacuum breakers', the vent's and the flange's mean gas mass |
| `step_rn` (condensation, agglomeration, settling, diffusion, phoresis on the drywell's floor and structures; deposition on the pool's surface; the pool's I2 partition) | RN RM §2.4 | every 10 s, the network's end-of-step volumes, the torus's V(z) area at the level, the aerosol volumes following the atmospheres |
| `decay_heat` (each location's share of the class inventory) | (sembawang's `fp_decay`) | drywell airborne by `set_atmosphere_heat`, its structures into the lumped wall, the torus's share by `set_pool_heat` |

Ablations: `NetworkMechanisms::without_fission_products` (back to the
first gh:#1100 change, bit for bit, predicted below) and
`without_fp_decay_heat` (carried, no heat: the pressures must equal the
ablation's, bit for bit, since the inventory is a pure tracer then). Both
on by default, pinned by `the_coupled_physics_is_on_by_default`.
`FUKUSHIMA_NETWORK_ABLATE` takes `fission_products` and `fp_decay_heat`.

**How the gas carries them** (stated in the module docs): a path's mean gas
mass `m` moves the fraction `m / (M + Σ out)` of its donor's gas (`M` at the
end of the step, `Σ out` everything it sent), each donor's paths in turn
from what is left; liquid carries none; the drywell sump holds none (as the
old pedestal).

### Predictions, written before the first run

**Case D** (new, in the test's module docs, 12 to 15): both models from
rest, 1800 s at 1 s, steam leaking into the drywell at 2 kg/s carrying Xe,
Cs, I2 and UO2 aerosol, the scrubbing switches and phoresis on, the class
decay powers set; the old model without its pool layers (the network has
none).

**Case B, the 30 h replay** (network − old Δ`p_ww`; the first gh:#1100
coupled run → predicted). The open-loop replays above measured what the
fission products' decay heat does to an *unlayered* old pool ("no layers"
against "neither": +1.6 % at 5 h, +8 % at 6 h, +13.5 % at 8 h, +15.7 % at
10 h, +11.1 % at 14 h, +10.3 % at 18 h, +13.2 % at 23 h). The network is
unlayered, so I predict it gains about as much:

| window | first gh:#1100 run | predicted with the fission products | why |
|---|---|---|---|
| 0–4 h | +0.07 … +1.3 % | moves by less than 0.5 % | little has left the core |
| 5 h | +1.17 % | −1.5 … +0.5 % | the decay heat of what the relief valves have brought |
| 6–10 h | −5.9 … −14.5 % | −3 … +5 % | the decay heat in the pool and the drywell gas |
| 14–23 h | −12.1 … −13.3 % | −5 … +2 % | as above; what is left is the old model's top layer (+2 … +9 %) against the displacement (+3 … +7 %) |
| 24 h, the vent | −48.0 % | −48 % ± 5 % | the old top layer (the network still has none) |
| 26–30 h | −0.5 … +0.0 % | within ±1 % | both vented |

Also predicted: `T_pool` at 23 h 345.6 → 366 ± 4 K (the old unlayered
replay's 366.6 K); at 30 h 358.7 → 371–374 K; `T_dw` at 8 h 407.9 → 433 ± 6 K
(old 434.9 K); **the flange opens on the network**, between 15 and 21 h
(peak `p_dw` about 790 kPa; the old opens at about 17.5 h), and the building
then receives hydrogen; at 23 h the network's Xe in the torus gas within
±20 % of the old model's, its Cs in the pool within ±15 %, on the drywell
structures within ±25 %; the default 6 h worsts move from 56.29 %, 13.65 %,
1.86 K to about 56 % (±1 %, the blowdown step), 4–9 % and under 1.86 K;
the cost per step up 10–30 % (the aerosol step and SPARC every 10 s).

**The building's ignition criterion** (a diagnostic, `NetworkResult::
rb_ignites`, sembawang's `bur_ignites` on the network building's mole
fractions, steam and CO2 the diluent; nothing burns): predicted **not met
within 30 h**. The old model's building peaks at 3.73 % H2, below the 4 %
of `bur_xh2ign`, and the network's building is open to the environment
through 1 m² \[A\] and holds steam.

### Results (2026-11-01)

**Case D** (predictions 12–16 all held): the network against the old model
at 1800 s: Xe in the torus gas 2.285e-3 / 2.308e-3 (−1.0 %); Cs in the pool
0.998 / 1.002 kg (−0.4 %), on the drywell's structures 0.782 / 0.797 kg
(−1.9 %), airborne 0.920 / 0.901 kg (+2.2 %); I2 in the pool 0.114 / 0.115
kg. Conservation on the network: Xe, Cs and iodine to 2e-14. The decay heat,
50.84 kW, is within 0.06 % of the same formula on the old model's inventory
(the old model's is from the end of the step, the network's from its start).
`without_fp_decay_heat` leaves every `p_dw` equal to
`without_fission_products`'s, bit for bit; with the heat `p_dw` rises by at
most 0.039 %.

**The refactor** (`FpHold` out of `Containment`): the 84 `bwr_mark1` lib
tests pass, the byte-for-byte `headless_30h.csv` fixture and
`three_workers_are_one_worker` among them. The 6 h replay with
`without_fission_products` reproduces the first gh:#1100 change's worsts
exactly (56.29 %, 13.65 %, 1.86 K; its pin passes).

**Case B, the 30 h replay** (network − old Δ`p_ww`; `T_pool` network / old
mean, top):

| t (h) | first gh:#1100 run | with the fission products | predicted | `T_pool` (K) | outcome |
|---|---|---|---|---|---|
| 3 | +1.29 % | +1.29 % | moves < 0.5 % | 325.45 / 325.41 | held |
| 4 | +0.36 % | +0.55 % | moves < 0.5 % | 332.16 / 332.12 | held |
| 5 | +1.17 % | +2.52 % | −1.5 … +0.5 % | 340.25 / 340.43, 343.21 | **MISSED** (see below) |
| 6 | −5.86 % | +0.97 % | −3 … +5 % | 341.41 / 341.58, 345.55 | held |
| 8 | −11.35 % | +1.73 % | −3 … +5 % | 343.99 / 344.00, 350.46 | held |
| 10 | −14.46 % | +0.92 % | −3 … +5 % | 347.09 / 346.95, 356.42 | held |
| 14 | −13.31 % | −3.79 % | −5 … +2 % | 354.81 / 354.47, 371.45 | held |
| 18 | −13.08 % | −1.44 % | −5 … +2 % | 361.05 / 359.10, 380.74 | held |
| 23 | −14.15 % | −3.75 % | −5 … +2 % | 366.19 / 363.47, 389.54 | held (`T_pool` 366 ± 4 K held) |
| 24 (vent) | −48.00 % | −47.88 % | −48 % ± 5 % | 371.85 / 364.20, 391.01 | held |
| 26 | −0.53 % | −0.16 % | within ±1 % | 373.17 / 355.07, 373.29 | held |
| 26.5–29 | | +0.15 … **+6.23 %** | within ±1 % | | **MISSED** (see below) |
| 30 | −0.26 % | −0.07 % | within ±1 % | 373.12 / 354.96, 373.18 | held (`T_pool` 371–374 K held) |

`Δp_dw` follows: +2.46 % at 5 h, +2.87 % at 6 h, +2.02 % at 8 h, +1.22 % at
10 h, −3.47 % at 14 h, −0.96 % at 18 h, −1.64 % at 23 h, −41.19 % at 24 h,
+2.20 % at 26 and 30 h. `T_dw` at 8 h 433.5 K against the old model's
434.9 K (predicted 433 ± 6 K: held); at 23 h 448.7 / 448.9 K.

- **The flange now opens on the network**, at 17.249 h (`p_dw` 749 985 Pa),
  against the old model's 16.824 h; peak `p_dw` 797.2 kPa (predicted
  15–21 h and about 790 kPa: held). **The building now receives hydrogen**:
  1.74 % at 23 h and 30 h, against the old model's 3.36 % and 3.73 %; its Xe
  1.18e-2 against 2.32e-2 of the core at 30 h (about half: the later opening
  and 34–39 kg less hydrogen in the network's drywell at 18–23 h, not
  attributed further).
- **The fission products agree** (network / old at 23 h): Xe in the torus
  gas 0.973 / 0.949 (+2.5 %), in the drywell 0.0137 / 0.0275; Cs in the pool
  61.86 / 62.26 kg (−0.6 %), on the drywell's structures 42.03 / 41.63 kg
  (+1.0 %), airborne 0.068 / 0.078 kg; vented by 30 h, Xe 0.986 / 0.974,
  Cs 0.058 / 0.068 kg. All within the predicted bands. The network's decay
  heat at 23 h: 17 kW into the drywell gas, 460 kW into its structures,
  1.96 MW into the torus pool.
- **The building's ignition criterion is never met** (largest `Y` 0.0194):
  held.
- **6 h worsts** (pinned after the run, labelled so): 57.77 %, 12.20 %,
  0.60 K, against 56.29 %, 13.65 %, 1.86 K without the fission products.
  Predicted about 56 %, 4–9 % and under 1.86 K: the first two **MISSED**.
  Both worsts are the one step of the vessel's blowdown at 4.956 h (cause 2:
  `p_ww` 344.9 against 392.8 kPa, the network's torus a step behind), not
  the post-debris window I had assumed; the drywell is a little warmer then
  by the decay heat of what has arrived since 3.9 h.
- **Worst over 30 h:** |Δ`p_dw`| 57.77 % (the 4.956 h step), |Δ`p_ww`|
  56.49 % (the vent), |Δ`T_pool`| 18.18 K (after the vent: the old model's
  mean against its top layer).
- **Cost:** 1251 s for the 30 h (11.6 ms a step, the three old replays
  included), against 1029 s without (9.5 ms): +22 % (predicted +10–30 %:
  held), measured with other runs beside it for about a third of the time.
  **Halved steps: 3 by 30 h, against 953** without the fission products, and
  at most 36 Picard iterations in a step against 43. Not attributed (the
  warmer pool and drywell keep the network away from whatever made the
  Picard steps fail; the failures were never classified).

**The misses:**

- *5 h, +2.52 % against −1.5 … +0.5 %:* an arithmetic sign error in my
  prediction, not in the model. The replays give the decay heat's effect on
  an unlayered old pool at 5 h as +1.6 % of its pressure (`no_pool_layers`
  +1.48 % against `neither` +3.06 %); adding the same to the network moves
  the gap **up**, to about +2.8 %, which I applied correctly at 6–23 h and
  subtracted at 5 h. The network's own gain is +1.33 % (444.3 against
  438.4 kPa without the fission products), the old unlayered model's
  +1.56 %.
- *26.5–29 h, `p_ww` up to +6.23 % against ±1 %* (**attributed
  2026-11-01**, layered-pool section: one-step flashes of the whole pool,
  not the vent): after the vent the
  network's torus, its pool at saturation (373.1 K), swings between 101.5 and
  108.5 kPa at the half-hour samples while the old model's sits at
  101.5–103.0 kPa. Not attributed. The leading hypothesis is the vent's
  check valve and its 60 m of inertia \[A\] against a pool flashing at
  saturation, which the old model's quasi-steady `mixture_vent` cannot show;
  untested. Before the fission products were coupled the network's pool sat
  at 354–359 K after the vent, subcooled, and this did not appear.

### Every remaining difference, attributed (default network with the fission products, 30 h)

(**Superseded 2026-11-01** for the default by the layered-pool section
below; this table still describes `without_pool_layers` on
`NetworkConfig::unit1_gh1097()`.)

| window | remaining Δ`p_ww` (network − old) | attributed to | evidence |
|---|---|---|---|
| 0–4.5 h | +0.3 … +1.3 % | the pool displacing the torus gas (network physics) | gh:#1097 cause 1, case A |
| 4.956 h | +57.8 % `p_dw`, −12.2 % `p_ww`, one step | the start-of-step equalising limit (numerical) and inertial paths (physical) | gh:#1097 cause 2; located by `FUKUSHIMA_NETWORK_TRACE=4.95,5.0` |
| 5–23 h | −3.8 … +2.5 % | **two terms that nearly cancel**: the displacement, +2 … +9 % (the network against the unlayered old model with its decay heat: +2.8 % at 5 h, +3.8 % at 8 h, +4.6 % at 14 h, +8.6 % at 18 h, +4.8 % at 23 h) against **the old model's pool layers**, which raise its pressure by putting the pool's decay heat into the top layer | the `no_pool_layers` replay against this run |
| 18–23 h drywell H2 | network 34–39 kg lower | not attributed (doubt): the flange opens 0.43 h later on the network and the paths differ; it was up to 57 kg *higher* before the decay heat was coupled | |
| building H2 | 1.74 % against 3.73 % at 30 h | the same, not attributed | |
| 24 h, the vent | −47.9 % | the old model's top layer, at 391 K by the decay heat, boiling as the torus depressurises | `no_pool_layers` replay: −0.05 % |
| 26.5–29 h | up to +6.2 % | ~~not attributed (above)~~ **CORRECTED 2026-11-01:** the single well-mixed pool's one-step flashes at saturation, every 225 s (next section but one); gone with the layered pool | traces at 26–26.75 h, unlayered and layered |
| 26–30 h `T_pool` | network 373.1 K, old mean 355.0 K, top 373.2 K | the layers only: the network matches the old top layer and the unlayered old pool (373.17 K) | `no_pool_layers` replay |
| 24–30 h `p_dw` | +2.2 % | the network's gas column and submergence at its own density | gh:#1097 cause 6 |
| after 24 h `T_dw` | network 10–23 K low | not attributed: the no-layers replay leaves 13 K (429.9 against 417.1 K at 30 h); the no-decay-heat replay is within 2.5 K (414.6 K), so the old model's drywell holds decay heat after the vent that the network's does not, in a share not identified | |

**So, with the fission products on the network, its only large remaining
difference from `Containment` is the old model's two-layer pool** (**CORRECTED
2026-11-01:** now carried by `tampines`' layered pool; the vent gap is
−37.6 % and the post-vent pool gap 12.4–12.9 K, next section), which
sets the vent's −48 %, the pool's mean temperature after it, and part of
the 5–23 h pressure; the rest is network physics (displacement, gas
column), the one-step blowdown lag, and the post-vent `p_ww` swings and
the building's hydrogen, which are not attributed.

### Burns: what `tampines` would need (not done; described, not hacked)

> **CORRECTED 2026-11-01:** done. `tampines` first gained the preferred
> reaction input (3f1261bd5), and the network burned through it; then, by
> maintainer direction (gh:#1103), the chemistry moved to sembawang
> (`bwr_mark1::reaction`) and `tampines` kept the "minimum" below in
> generic form, a per-species source and sink with each species at its own
> code enthalpy, plus the existing atmosphere heat. Section "the surface
> block, the decay heat above the exits, and burns" below. The text here is
> the record as written.

A burn in the network's building (MELCOR BUR RM §2.3–2.5, sembawang's
`Containment::step_burn`) converts H2 and CO with O2 into steam and CO2 and
releases the heat of combustion. The network can add steam and CO2 (a
`Source` on a building port) and heat (`set_atmosphere_heat`), but **cannot
remove a non-condensable**: `Source` rates must be ≥ 0, and
`ControlVolumeInputs` has a sink for steam only (`atmosphere_steam_sink`).
What is needed, in `tampines::control_volume` (which another change is
editing, so not touched here):

- **Preferred: a reaction input** on a volume's atmosphere, `n` mol/s of
  `H2 + ½ O2 → H2O(g)` and of `CO + ½ O2 → CO2`, applied inside the
  volume's implicit step and capped at the reactants present, with the
  energy balance closed by tampines itself from its own species references
  (the JANAF `c_v` integrals and IF97 steam), so the heat of combustion is
  consistent with the volume's energy datum. The caller (sembawang's
  `bur_ignites`, `bur_completeness`, `bur_flame_speed`, `bur_cdim`, already
  free functions) supplies only the rates.
- **Minimum: an atmosphere sink per non-condensable**, `[MassRate; 6]`,
  removing at the atmosphere's temperature with its enthalpy (as
  `atmosphere_steam_sink` does for steam), plus the existing source and heat.
  The caller must then compute the heat of combustion against tampines'
  energy references, which is where an error would hide; hence the
  preference above.
- Either needs `CvNetwork` to hold and pass it (as `set_atmosphere_steam_sink`)
  and a V&V case: an adiabatic, closed volume burning a known H2-air mixture
  to the adiabatic flame temperature and pressure (a hand calculation from
  JANAF data).

~~Until then the network reports the building's BUR ignition criterion as a
diagnostic (`rb_burn_y`, `rb_ignites`), sembawang's `bur_ignites` on the
building's composition; nothing burns.~~ **CORRECTED 2026-11-01:** the
building burns (`NetworkMechanisms::burns`, default on).

## gh:#1100 (continued): the layered torus pool and dry sumps on the network (2026-11-01)

**What was adopted** (`tampines` d870c66cb, used unchanged):

| `tampines` capability | on the network | why this way |
|---|---|---|
| `ControlVolume::with_pool_layers(n)`, after the other ablations | the torus pool in `n` Lagrangian layers, `NetworkMechanisms::pool_layers` (ON; `without_pool_layers` ablates) | the old model's two-layer pool (`Containment::layer_pool`) is the last large mapped difference |
| `without_pool_layer_conduction()` | `NetworkMechanisms::pool_layer_conduction` (ON, `tampines`' default) | molecular conduction across a stable column is the lower bound of the exchange; the old model has none ("conduction and entrainment neglected"), so it is an ablation to attribute with, not to match with |
| `pool_surface_temperature()` | `NetworkResult::t_pool_top_k` (mirrors `ContainmentResult::t_pool_top_k`); `t_pool_k` stays the energy mean | |
| `phase_states_at(z)` (the layer at `z`) | SPARC's pool side at the quenchers (the relief flow's scrubbing) and at the downcomers' exits; the surface layer for the aerosol step's pool; the relief H2's temperature without `relief_h2_enthalpy` | the old model takes its top layer for all of these; one layer, all are the pool's temperature |
| a pool may be empty; `is_pool_dry()` | `NetworkConfig::sump_water_m3` 1.0 → **0.0**: the drywell and building sump pits start dry, as the old pedestal does (`pedestal_kg` = 0 at construction); the cavity's wet/dry branch and the ablation's take `is_pool_dry()` | the \[A\] 1 m³ fill existed only because a control volume could not be empty |

**The choices, with their justification (none taken from a comparison):**

- **`n` = 11**, from `derived_pool_layers`: four layers on average across
  the downcomers' submergence, `⌈4 D / s⌉`, with the initial pool depth
  `D` = 3.237 m (the torus V(z) at 1750 m³ of sembawang's water, `pool_water_m3`
  \[P\]) and `s` = 11 800 Pa / (995 × 9.81) = 1.209 m (`vent_submergence_pa`
  \[A\]). Mean layer 0.29 m; the torus is narrow at the bottom, so the bottom
  layer is 0–0.57 m. The rule's purpose is that the water above the
  downcomers' exits, which is the old model's whole top layer, is resolved
  by several layers rather than one. It is a resolution, so the result
  must not depend on it: **22 layers are run beside 11.**
- **The quenchers' port at the torus bottom (0 m, \[A\]).** sembawang's old
  model does not put the quenchers at the downcomers' depth, as the
  network's module docs claimed (corrected there): with `pool_layers` its
  relief flow goes into the deep layer, "the relief valves' quenchers, near
  the pool's bottom" (`Containment::step`), which is all the water below the
  downcomers' exits, heated as one. A port at the bottom reproduces that:
  heat injected there rises and the convective adjustment mixes the whole
  column below any warmer water. A port higher up (the brief suggested
  about 0.3 m) leaves the water beneath it unheated (`tampines`' L2: 4.8 K
  colder below a port 0.3 m up), which is not what the old model does and
  not supported by any held datum; the quenchers' real elevation is not in
  the held sources. The downcomers' exits stay at the submergence; the
  shell's condensate drain stays above the level (the top layer).
- **Heat into a dry drywell sump goes to the structures, as the old model
  does.** The old model boils its pedestal while it holds water
  (`boil_pedestal`, its MCCI top heat and the ablation's), and otherwise
  puts the heat into `dw_wall_t`. The network already sends the cavity's
  top heat to the structures when there is no water over the debris, and
  caps the sump's heat at what heats and boils the water it holds (the
  rest to the structures), so a dry sump is given no pool heat and
  `tampines`' default (heat into a dry pool goes to the atmosphere) is not
  exercised from the start of a step. Within a step a sump can still boil
  dry under that cap (IF97 against the cap's constant `c_p`); `tampines`
  then puts the remainder into the drywell atmosphere and reports it as
  `pool_heat_to_atmosphere`, ~~which **`CvNetwork::step` does not pass back**
  (its `NetworkStepReport` has no per-volume reports), so sembawang cannot
  read or redirect it. A `tampines` addition would be needed to report it:
  the per-volume `ControlVolumeStepReport`s (or their
  `pool_heat_to_atmosphere`) in `NetworkStepReport`. Not hacked; the
  amount is bounded by the cap's mismatch.~~ **CORRECTED 2026-11-01:**
  `NetworkStepReport::volumes` now carries it, and the network reports it
  per volume (`NetworkResult::pool_heat_to_atmosphere_j`; reported, not
  redirected).
- **Where the pool's decay heat goes: by mass (stated, a difference).**
  ~~`set_pool_heat` shares the heat over the layers by mass, and `tampines`
  has no pool heat at an elevation~~ (**CORRECTED 2026-11-01:** `tampines`
  has `set_pool_heat_at`, and the default now puts it above the exits,
  `fp_pool_heat_above_exits`); the old model puts the pool's share of
  the fission products' decay heat into its top layer (`pool_energy`). The
  replays above attribute most of the old top layer's 391 K at 24 h to that
  choice. Neither is derived here: dissolved and suspended fission
  products mix with the water, which favours by mass; the old choice is
  its own.

**The earlier pins** are kept for the configuration they were measured on,
`NetworkConfig::unit1_gh1097()` (1 m³ sumps, quenchers at the exits) with
`without_pool_layers()`; `tampines`' one-layer path is bit-identical to
before d870c66cb (its L1), so they must reproduce exactly.

### Predictions, written before any run of this change

**Case A** (relief into the torus, the default network now layered, the
quenchers at the bottom, the sumps dry): the steam enters the bottom layer,
which is then the lightest and mixes the whole column each step, so the
pool stays mixed and `p_ww` − old moves by less than ±5 Pa from +341.1 /
+328.8 Pa; worst |Δ`T_pool`| within ±0.1e-3 K of 2.87e-3 K; `p_dw` − old
+6.4 Pa → +1 … +6 Pa (no 1 m³ sump to humidify the drywell: about 0.1 kg
less evaporated, −4 Pa). The pins hold.

**Case C** (debris): dry floor, every quantity within 0.1 % of its earlier
value (the 1 m³ fill took no part, only humidified); wet floor, FDI steam
within 2 % of the old model's (all the poured water is now over the debris,
as in the old pedestal, and none of it is a 323 K fill), worst |Δ`p_dw`|
3.3–3.5 %.

**Case D** (fission products, the old model without layers): the leak's
steam enters the torus at the exits and the relief's at the bottom; every
D quantity moves by less than 2 % relative and stays within its band; the
tracer leaves the pressures bit-equal (D16).

**Case B, the 6 h worsts** (the default): |Δ`p_dw`| 57.77 % ± 1 % and
|Δ`p_ww`| 12.20 % ± 1 % (both the 4.956 h blowdown step, which the pool does
not set), |Δ`T_pool`| 0.60 → 0.4–0.9 K.

**Case B, the 30 h replay, network − old Δ`p_ww`** (the fission-product run
above → predicted with layers and dry sumps, n = 11):

| window | before | predicted | why |
|---|---|---|---|
| 0–4 h | +0.07 … +1.3 % | moves by less than 0.3 % | the relief steam enters at the bottom and the column mixes; the dry drywell sump changes the drywell's humidity by little |
| 5 h | +2.52 % | moves by less than 0.5 % | as above |
| 6–10 h | +0.9 … +1.7 % | **up** by 1–4 %, to +2 … +5 % | the downcomers' steam now heats only the water above their exits, so the surface layer is hotter than the mean and the torus gas wetter; this is the part of the old layers' effect that is not the decay heat (2–3 % in the replays) |
| 14–23 h | −3.8 … −1.4 % | **up** by 1–4 %, to −2 … +2 % | as above |
| 24 h, the vent | −47.9 % | **shrinks**, to −30 … −46 % | the network's top layer, hotter than its mean, now boils as the torus depressurises, but it does not reach the old top layer's 391 K, because the pool's decay heat is shared by mass here |
| 26.5–29 h swings | up to +6.2 % | **below 3 %** | hypothesis: the swings come from the whole saturated pool flashing; with layers only the top layer can |
| 30 h | −0.07 % | within ±1 % | both vented |

Also predicted: `T_pool` (energy mean) at 26–30 h **falls** from 373.1 K to
360–370 K, so the 18 K gap to the old mean (355.0 K) shrinks to 5–15 K, the
network's surface layer at 373 ± 1 K (the old top: 373.2 K); at 23 h the
network's top layer 3–15 K above its mean (the old: 26 K); the flange opens
within ±1 h of 17.25 h; the cost per step up 10–40 %.

**Resolution (n = 22 against 11):** every window's Δ`p_ww` within 0.5 % of
n = 11's, the mean `T_pool` within 0.5 K, the top layer within 1 K, the vent
within 2 % of `p_ww`.

**Ablations (one change at a time from the new default):**

- *`without_pool_layers` (the dry sumps and the bottom quenchers kept):*
  within 0.5 % of the fission-product run in every window (the quenchers'
  depth changes the bubble efficiency only, and it is near 1 at both
  depths), so the layers carry all the movement above.
- *`without_pool_layer_conduction`:* within 0.3 % of `p_ww` everywhere, the
  top layer within 1 K (the conduction length `√(α t)` is 0.12 m over 28 h,
  half a layer).

### Results (2026-11-01, release, Rust 1.99.1)

**The earlier pins reproduce exactly** on `NetworkConfig::unit1_gh1097()`
without the layers (6 h replay): 57.77 % / 12.20 % / 0.60 K (with the
fission products), 56.29 % / 13.65 % / 1.86 K (without), 60.39 % / 9.78 % /
1.32 K (gh:#1097's mapping), and case A's 341.1 / 328.8 / 280.7 Pa and
2.87e-3 K. This confirms `tampines`' one-layer path and the refactor here
(the `pool_side` elevation, the `is_pool_dry` branch) change nothing when
the layers are off.

**Case A on the new default** (`relief_steam_on_the_layered_default`):

| network | `p_ww` − old | `p_dw` − old | worst \|Δ`T_pool`\| |
|---|---|---|---|
| prismatic, default (8 layers: its 2.34 m pool gives `⌈4 × 2.34 / 1.209⌉`) | **+360.1 Pa** | 0.0 Pa | 2.84e-3 K |
| torus, default (11 layers) | **+352.8 Pa** | 0.0 Pa | 2.85e-3 K |
| prismatic, default without the layers | +341.1 Pa | 0.0 Pa | 2.87e-3 K |
| … and the quenchers at the exits | +341.1 Pa | 0.0 Pa | 2.87e-3 K |
| prismatic, default with 1 m³ sumps | +360.1 Pa | +6.4 Pa | 2.84e-3 K |
| prismatic, default without layer conduction | +360.1 Pa | | 2.84e-3 K |

The pool stays mixed (top 305.264 K = mean). Outcomes: `p_ww` within ±5 Pa
**MISSED** (+19.0 Pa prismatic, +24.0 Pa torus), carried wholly by the
layers (not the quenchers' depth, not the conduction, not the sumps).
**Resolution check, run after this result** (prismatic, the pool mixed in
every case): 1 layer +341.1, 2 +347.9, 4 +354.2, 8 +360.1, 16 +365.5 Pa,
about +6 Pa a doubling, monotone. Explanation, from the code and supported
by that trend (the length to the −1/4 power in the coefficient), not by
an ablation inside `tampines` (not edited here): with
layers the surface exchange sees the **top layer** as the pool, its mass in
MELCOR's Eq 5-17 limit and its thickness (0.2–0.3 m) as the pool-side
natural-convection length (`tampines::control_volume::pool_surface::exchange`,
`h_free`, `x = min(d_s, l)`), where one well-mixed pool gives its whole
depth (2.3–3.2 m); a thinner length gives a larger pool-side coefficient,
more evaporation, a wetter torus gas. The old model does the same with its
top layer's 1.21 m. In a column the convective adjustment has just mixed
through, the convecting depth is the mixed block, not one layer, so this is
a resolution dependence in `tampines` (see "What `tampines` would need",
below). `p_dw` +1 … +6 Pa **MISSED** (0.0 Pa: the 1 m³ sump's evaporation
was the whole +6.4 Pa, not two thirds of it). `T_pool` within ±0.1e-3 K of
2.87e-3 K: held. At most 5 Picard iterations (2 before).

**Case C:** dry floor, every value as before to the printed digits
(1258.3 kg, 8.251 / 12.012 kg, 2774.9 K; worst |Δ`p_dw`| 0.01 %): held.
Wet floor, FDI steam 27.6 against the old model's 27.6 kg (held), worst
|Δ`p_dw`| **4.26 %** against the predicted 3.3–3.5 % (**MISSED**): the
dry sumps carry all of the movement (with 1 m³ sumps and the layers on
3.46 %, without the layers and dry sumps 4.26 %); the 1000 kg fill at
323 K was absorbing part of the debris's heat before the poured water
boiled. Not attributed further.

**Case D:** every value moved by less than 0.1 % relative (Xe in the torus
gas −0.9 %, Cs in the pool −0.4 %, on the structures −1.8 %, airborne
+2.0 % against the old model, as before; conservation 2e-14; the tracer
bit-equal, the heat raising `p_dw` by at most 0.039 %): held.

**Case B, 6 h worsts (default):** 58.11 %, 11.89 %, 0.61 K (all three
predictions held), pinned after the run, labelled so.

**Case B, 30 h, network − old Δ`p_ww`** (n = 11 is the default; `T_pool`
network mean / top, old mean / top):

| t (h) | FP run (before) | dry sumps, bottom quenchers, no layers | **default, n = 11** | n = 22 | n = 11, no conduction | predicted | `T_pool` n = 11 (K) | outcome |
|---|---|---|---|---|---|---|---|---|
| 3 | +1.29 % | +1.19 % | +1.40 % | +1.45 % | +1.40 % | moves < 0.3 % | 325.44 / 325.44; 325.41 | held (+0.11) |
| 4 | +0.55 % | +0.67 % | +0.92 % | +0.97 % | +0.92 % | moves < 0.3 % | 332.15 / 332.16; 332.12 / 332.17 | **MISSED** (+0.37) |
| 5 | +2.52 % | +2.67 % | +2.99 % | +3.03 % | +2.99 % | moves < 0.5 % | 340.23 / 342.55; 340.43 / 343.21 | held (+0.47) |
| 6 | +0.97 % | +1.25 % | +1.98 % | +2.03 % | +1.98 % | +2 … +5 % | 341.37 / 343.60; 341.58 / 345.55 | **MISSED** by 0.02 (the rise, +1.0, is inside "up 1–4") |
| 8 | +1.73 % | +2.01 % | +2.99 % | +3.03 % | +2.99 % | +2 … +5 % | 343.93 / 346.40; 344.00 / 350.46 | held |
| 10 | +0.92 % | +1.09 % | +2.37 % | +2.40 % | +2.38 % | +2 … +5 % | 347.01 / 350.28; 346.95 / 356.42 | held |
| 14 | −3.79 % | −3.82 % | −1.05 % | −1.04 % | −1.02 % | −2 … +2 % | 354.59 / 360.75; 354.47 / 371.45 | held |
| 18 | −1.44 % | −1.42 % | +1.04 % | +1.03 % | +1.09 % | −2 … +2 % | 360.38 / 368.02; 359.10 / 380.74 | held |
| 23 | −3.75 % | −3.73 % | −1.18 % | −1.23 % | −1.09 % | −2 … +2 % | 365.38 / 373.03; 363.47 / 389.54 | held |
| 24 (vent) | −47.88 % | −47.88 % | **−37.57 %** | −36.59 % | −38.61 % | −30 … −46 % | 369.73 / 377.64; 364.20 / 391.01 | held |
| 24.5 | | −25.36 % | −19.56 % | −19.51 % | −20.55 % | | 368.07 / 374.85; 359.33 / 381.60 | |
| 25 | | −7.71 % | −5.59 % | −5.22 % | −5.38 % | | 367.49 / 373.78; 356.11 / 375.30 | |
| 26 | −0.16 % | −0.14 % | +0.71 % | +0.80 % | +0.78 % | | 367.43 / 373.43; 355.07 / 373.29 | |
| 26.5–29.5 | +0.15 … +6.23 % | −0.24 … **+14.18 %** | −0.32 … +0.64 % | −0.42 … +0.66 % | −0.52 … +0.75 % | below 3 % | 367.5–367.8 / 373.2–373.4 | held |
| 30 | −0.07 % | −0.05 % | +0.44 % | +0.20 % | +0.38 % | within ±1 % | 367.90 / 373.21; 354.96 / 373.18 | held |

`Δp_dw` (n = 11): +2.94 % at 5 h, +4.06 % at 6 h, +3.25 % at 8 h, +2.64 % at
10 h, −0.78 % at 14 h, +1.48 % at 18 h, +0.93 % at 23 h, −32.15 % at 24 h,
+2.1 … +2.8 % after 26 h.

- **The vent gap shrank from −47.9 % to −37.6 %**, as predicted in sign and
  inside the band: the network's top layer is 377.6 K at 24.0 h (its mean
  369.7 K) and boils as the torus depressurises; the old top layer is at
  391.0 K, the difference the decay heat's placement (by mass here, into
  the top layer there) leaves. Against the old model **without** its
  layers (the `no_pool_layers` replay in this run) the network is now
  +19.7 % at 24 h: it vents more slowly than an unlayered old pool, as a
  layered pool should.
- **The post-vent `T_pool` gap shrank from 18.2 K to 12.4–12.9 K** (network
  mean 367.4–367.9 K against the old mean 355.0 K; predicted 5–15 K: held).
  The network's surface layer sits at 373.2–373.4 K against the old top
  layer's 373.2–373.3 K (held, within 0.2 K). What remains is the deep
  water: the old deep layer is cooler because the old model put the pool's
  decay heat into its top layer, which boiled it off after the vent; here
  it is shared by mass and the deep water keeps its share.
- **The 5–23 h gap moved toward zero and changed sign at 18 h**
  (−1.2 … +3.0 %, from −3.8 … +2.5 %): the network's surface layer is now
  2.3–7.7 K above its mean (the old one's 2.8–26 K).
- **The flange opens at 16.254 h** (17.249 h before; the old model 16.824 h;
  predicted 17.25 ± 1 h: held, by 0.005 h). The network is now 0.57 h
  early rather than 0.43 h late. The building's hydrogen at 30 h: 2.18 %
  (1.74 % before; the old model 3.73 %). Peak `p_dw` 817.9 kPa.
- **The dry sumps moved the FDI steam** to 74 kg against the old model's
  75 kg (55 kg with the 1 m³ fill, gh:#1100's unattributed 20 kg: it was
  the fill), and the concrete ablated to 65 958 against 64 388 kg at 23 h
  (+2.4 %; +0.27 % with the fill), 88 015 against 86 534 kg at 30 h; not
  attributed further.
- **Worst over 30 h** (n = 11): |Δ`p_dw`| 58.11 % (the 4.956 h step),
  |Δ`p_ww`| 44.76 % (the vent; 56.49 % before), |Δ`T_pool`| 12.94 K
  (18.18 K before).
- **Resolution** (n = 22 against 11): every window within 0.1 % of `p_ww`
  except the vent, 0.98 % (held, < 2 %); the mean `T_pool` within 0.49 K
  (held, < 0.5 K); the top layer within 1.06 K at 24 h (**MISSED** by
  0.06 K; elsewhere within 0.25 K). The vent window's residual dependence
  on `n` is the same top-layer-thickness effect as in case A.
- **Ablations:** without the layers (dry sumps and bottom quenchers kept),
  within 0.3 % of the FP run in every tabled window to 26 h (held), but the
  post-vent swings grew to +14.2 % at 26.5 h (**MISSED**, "within 0.5 % in
  every window"); without the layer conduction, within 0.25 % of the default
  except at 24 h (−38.61 against −37.57 %: **MISSED**, predicted under
  0.3 %), the top layer within 0.25 K (held). Conduction is not what sets
  the result.
- **Cost:** **MISSED** (predicted +10–40 % a step): 2805 s for the 30 h
  (26.0 ms a step, one old replay included) against 1251 s, measured with
  four runs on four cores; without conduction 2147 s, n = 22 2523 s, the
  unlayered dry-sump run 1833 s in the same conditions (so about +50 % for
  the layers, of which conduction is about half). At most 100 Picard
  iterations in a step.

**So:** with `tampines`' layered pool the network carries the old model's
last large mapped mechanism. The remaining differences in the pool are the
placement of the pool's decay heat (by mass against the old top layer),
which sets what is left of the vent gap and the post-vent mean
temperature, and the top-layer-thickness dependence of the surface exchange
(`tampines`), which sets the n-dependence at the vent.

### The 26.5–29 h post-vent swings, attributed

Two step-by-step traces, 26.00–26.75 h (`FUKUSHIMA_NETWORK_TRACE=26,26.75`,
2701 steps each), on the unlayered network (dry sumps, bottom quenchers,
`without_pool_layers`) and on the default:

| | unlayered | default (11 layers) | old model |
|---|---|---|---|
| `p_ww` range, kPa | 101.6–**140.5** | 102.4–103.4 | 101.8–103.3 |
| Δ`p_ww` network − old | −1.6 … **+38.0 %** | −0.7 … +1.4 % | |
| one-step rises over 2 kPa | **12** (one every 200–268 s, mean 225 s) | 0 (largest 0.9 kPa) | |
| pool, K (mean; top) | 373.08–373.20, one pool | 367.43–367.54; 373.42–373.44 | |
| torus steam, kg | 1426–1844 | 1460–1470 | |

**Mechanism (unlayered):** a relaxation oscillation of the single, well
mixed, saturated pool. Between bursts the pool (1.75e6 kg) warms by about
0.12 K under the decay heat and the downcomers' condensing steam, the vent
carrying 0.8–0.9 kg/s and the downcomers 1.5 kg/s. When it reaches
saturation it flashes in one 1 s step: at 26.0375 h, 418 kg of steam, the
swollen level 5.41 m for that step, the torus gas to 399 K, `p_ww` from
101.7 to 140.5 kPa (+38.8 kPa). The downcomers stop, the vacuum breakers
open (62 kg/s for a step) and the vent, choked, rises from 0.9 to 8.7 kg/s
within the step and takes about 200 s to bring the torus back, by which
time the pool, now 0.12 K subcooled, is warming again. The half-hour
samples alias this 225 s cycle: they land on a burst at some samples
(26.5 h, +14.2 %) and between bursts at others. **The vent is not the
cause** (the trace shows it responding in the step, not lagging by its
60 m of inertia, which was gh:#1100's leading hypothesis): it is the
flashing of the whole pool at once. Not tested by a vent-length ablation.

**With layers it is gone:** only the surface layer (373.4 K) reaches
saturation, and it holds about 1/11 of the water, while the deep layers sit
6 K subcooled; it boils steadily rather than flashing in bursts, and the
torus stays within 1.4 % of the old model's. The prediction (swings below
3 % with layers) held, and its hypothesis is confirmed. The cycle period
and amplitude of the unlayered pool are set by the 1 s step (the flash
completes in one step), so they are a property of a well-mixed pool under
a one-step flash, not a physical measurement.

### What `tampines` would need (described; `tampines` was not edited here)

> **CORRECTED 2026-11-01:** all three done in `tampines` and used here
> (section "the surface block, the decay heat above the exits, and burns").

1. **The surface's pool-side length should be the surface mixed block, not
   one layer.** In a layered pool `exchange` takes the top layer's mass
   (Eq 5-17's limit) and thickness (the natural-convection length). Where
   the convective adjustment has merged the top layers into one block, the
   block's mass and depth are what convects; taking one layer makes the
   coefficient depend on `n` (case A: +19 Pa at 8 layers; the vent: 1 % of
   `p_ww` between 11 and 22 layers).
2. **Pool heat at an elevation** (or into a named layer), so that a caller
   can place the decay heat of what is scrubbed where it is scrubbed;
   `set_pool_heat` shares it by mass.
3. **`NetworkStepReport` should carry each volume's
   `pool_heat_to_atmosphere`** (or its `ControlVolumeStepReport`), so a
   caller can see heat that went into a dry or boiled-dry pool's atmosphere.

## gh:#1100 (continued): the surface block, the decay heat above the exits, and burns (2026-11-01)

> Research, education and V&V only. AI draft, unreviewed.

The three `tampines` needs listed above, done in `tampines` (its V&V,
`crates/tampines/verification_and_validation/control_volume.md`, section
"The surface sees the surface mixed block"), and used here.

**1. The surface exchange's layer dependence: cause confirmed and fixed in
`tampines`.** Read from the code: `ControlVolume::surface_liquid` handed
`pool_surface::exchange` the top layer's mass (Eq 5-17's limit) and its
thickness as the pool-side length `L` (`h_free`, `X = min(D_s, L)`,
`h ∝ X^-1/4`). Confirmed directly in `tampines` before any edit (its L9,
case A on one volume): `p` at 600 s 105684.434, 105691.218, 105697.528,
105703.375, 105708.756 Pa at 1, 2, 4, 8, 16 layers, +6.8 … +5.4 Pa a
doubling, as here. MELCOR's CVH has one pool per volume, so its length is
that pool's depth and Eq 5-17's mass that pool's (CVH/FL RM §5.1.2, Eqs
5-20, 5-22 to 5-25, as cited by the ports; the manual is private and was
not re-read). The fix: the surface sees the **surface mixed block**, the
layers from the top down that are not stably stratified against the
surface water (its mass, its depth, its temperature; what it gives up
shared over its layers by mass). On the mixed pool of L9 the result is now
the one-layer result to 1e-13 at every `n`; `without_surface_mixed_block`
restores the top layer bit for bit. Here:
`NetworkMechanisms::surface_mixed_block` (ON), `without_surface_mixed_block`.

**2. The torus's decay heat where the old model puts it.** The old model
puts the torus's share of the fission products' decay heat (its gas, its
pool's surface, scrubbed in the pool) into `pool_energy()` (`Containment::
step`, step 0), which with `pool_layers` is its top layer, the water above
the downcomers' exits (`layer_pool` splits at the submergence). `tampines`
now takes heat in a band of elevations (`ElevatedPoolHeat`,
`CvNetwork::set_pool_heat_at`, shared over the band's water by mass); the
network puts that heat between the exits and the torus top
(`NetworkMechanisms::fp_pool_heat_above_exits`, ON;
`without_fp_pool_heat_above_exits` shares it over the pool by mass, as
before). One layer: the same, bit for bit.

**3. Each volume's pool heat to its atmosphere.** `NetworkStepReport::
volumes` carries every `ControlVolumeStepReport`; the network sums
`pool_heat_to_atmosphere` per volume into
`NetworkResult::pool_heat_to_atmosphere_j` (reported, not redirected).

**4. Burns.** `Containment::step_burn`'s arithmetic moved, unchanged, into
`containment::bur_step` (BUR RM §2.3–2.5), which both models now call; the
criterion stays `bur_ignites` (BUR RM §2.2). The network applies it to its
building's start-of-step gas (steam and CO2 the diluent, where the old
model, holding its building dry at one atmosphere, counts CO2 only) and
hands the moles burned per second to `tampines`
(`CvNetwork::set_atmosphere_reactions`), which caps them at the reactants
and closes the heat of reaction on its own references. **sembawang adds no
heat for a burn** (`Burn::q_j` stays a diagnostic in both models).
`NetworkMechanisms::burns` (ON), `without_burns`. Always BUR's dynamics (the
old model's default `burn_dynamics`): its ablated "explosion" removes the
fuel with no heat, which a control volume cannot express. The drywell and
torus are inerted and no path brings them oxygen, so only the building can
burn.

### Predictions, written before any run of this change

**Bit identity of the earlier configurations.** With
`without_surface_mixed_block().without_fp_pool_heat_above_exits()
.without_burns()` the network is the code before this change bit for bit
(the block ablation is `tampines`' old path; a zero reaction and a band of
zero heat do nothing at all): case A's layered values (+360.1, +352.8 Pa)
and the 6 h worsts 58.11 %, 11.89 %, 0.61 K reproduce, as do every earlier
pin on `unit1_gh1097` without layers (one layer: the block and the band are
the old code). The `bwr_mark1` lib tests (the byte-for-byte fixture,
`three_workers_are_one_worker`) pass after the `bur_step` refactor.

**Case A** (no fission products, so only the block acts): prismatic default
(8 layers) +360.1 → **+341.1 ± 0.5 Pa** (the mixed pool is now the
one-layer pool), the torus default (11) +352.8 → +328.8 ± 0.5 Pa; 2, 4, 16
layers all within 0.5 Pa of +341.1.

**Case C, D:** unchanged within 0.1 % (case C has no torus decay heat
beyond nothing; case D runs the old model unlayered and the network
default, whose torus sees D's decay heat above the exits now: Cs in the
pool within its ±15 % band, the pressures within 0.05 % of before).

**Case E (new: burns).** Both models from rest, 1 s steps, 3600 s,
`flange_leak_pa` lowered to 150 kPa and `flange_leak_m2` raised to
0.01 m² (test inputs, \[A\], so that the building receives hydrogen
within the run), 0.5 kg/s of H2 (248 mol/s) leaking into the drywell, no
steam:

- E1. Both buildings ignite. The old model near 1000 s (the drywell
  reaches 150 kPa after about 230 s, its gas then turning over in about
  500 s, the building's 1.03e6 mol in about 4200 s, to 10 % H2 in about
  400 s more); I bound it at 700–2500 s. The network ignites within ±15 %
  of the old model's time (the same displacement of the building's gas;
  its building is a volume near one atmosphere, open through 1 m²).
- E2. The first burn is complete in both (`Y` ≈ 0.10 gives a
  completeness of 1.46, clipped at 1): about 1.0e5 mol (210 kg) of H2
  burned; the network's within ±10 % of the old model's (`Burn::q_j` over
  241.8 kJ/mol), and `tampines` burns what sembawang asks to 1e-9 (oxygen
  at about 19 %, never limiting).
- E3. Duration `CDIM / V` = 18.37 m / 7.71 m/s ≈ 2.4 s in the old model;
  the network's within ±20 %.
- E4. `tampines`' heat of reaction for the burned H2 within 1 % of
  241.8 kJ/mol × the moles (its energy per mole is JANAF's to 1.5e-5 at
  298.15 K, 241.83 kJ/mol); sembawang sets no atmosphere heat on the
  building (it has none to set), so the heat is counted once.
- E5. The network's building peaks at 3.0–4.8e5 Pa (the old model's
  adiabatic bound, `q / (2.5 V)` ≈ 3.8e5 Pa above one atmosphere, less what
  the 1 m² opening vents during the 2.4 s burn).
- E6. `without_burns`: nothing burns and the building's hydrogen passes
  10 %.

**The reactions moved from `tampines` to sembawang (gh:#1103, scope
added mid-change; predicted before the first run on the moved code).**
`tampines`' `AtmosphereReactions` became `sembawang::bwr_mark1::reaction`
(the derivation, JANAF constants and 298.15 K anchoring unchanged); a burn
is now `tampines`' species sinks (H2, CO, O2) and sources (H2O, CO2) at
their start-of-step code enthalpies `h_i(T0)` plus the heat `Q − Σ ν ξ
h_i(T0)` by `set_atmosphere_heat`. Case E's fingerprint on the code
before the move (`tampines` applying the reactions; peak `p_rb`
402259.484691 Pa, `0x41188d4df052e368`; at 3601 s `p_rb` 101330.478615 Pa,
H2 burned 323.425472711 kg, building H2 fraction 0.783064633; the burns
listed in the results) is predicted to reproduce **to round-off, not bit
for bit**: the stored energy rises by `(Q − moved) + moved` against `Q`
in one addition, the extents travel as mass rates (`ξ M / Δt`, times `Δt`
in the step), and there is one more `refresh` between the heat and the
species. Ignition times and the first burn's H2 the same to the printed
digits; peak `p_rb` within 1e-9 relative; the later burns within 1e-6
(the network's Picard iteration can amplify round-off through its
switches). With no burn (case B, every pin above) the moved code is the
code before the move bit for bit (zero species rates and zero heat do
nothing at all).

**Case B, the 30 h replay** (the default network, n = 11, against the
layered run above; network − old Δ`p_ww`):

| window | layered run (before) | predicted now | why |
|---|---|---|---|
| 0–4 h | +0.9 … +1.4 % | moves by less than 0.3 % | little decay heat in the torus yet; the block changes little while the downcomers mix the water above their exits |
| 5–10 h | +2.0 … +3.0 % | moves by −1 … +2 % | the decay heat now heats only the water above the exits (about 37 % of the pool), so the surface is hotter and the torus gas wetter (+); the block sees a deeper, more massive surface water, so a smaller coefficient (−) |
| 14–23 h | −1.2 … +1.0 % | **up** by 1–5 % | the decay heat dominates as above |
| 24 h, the vent | −37.6 % | **shrinks**, to −15 … −32 % | the water above the exits, now carrying the decay heat, nears the old top layer's 391 K and boils as the torus depressurises |
| 26–30 h | −0.3 … +0.7 % | within ±1.5 % | both vented |

Also predicted: the network's surface layer at 24 h 377.6 → 383–392 K; the
post-vent mean `T_pool` 367.4–367.9 → 358–366 K (the deep water no longer
gets the decay heat, the old mean is 355.0 K); the flange opens within 1 h
of 16.25 h; **no burn in either model** (the building's `Y` peaks at
0.019–0.04, against 0.10), so the default is bit-equal to
`without_burns` in case B; the 6 h worsts (the 4.956 h blowdown step)
58.11 ± 1 %, 11.89 ± 1.5 %, |Δ`T_pool`| 0.61 → 0.4–1.5 K; resolution (n =
22 against 11): the vent within 0.3 % of `p_ww` (0.98 % before, the
top-layer effect now gone). The drywell sump's pool heat to its atmosphere:
under 1 MJ over 30 h (the cap's `c_p` mismatch only).

### Results (2026-11-01, release, Rust 1.99.1)

**Bit identity of the earlier configurations: held.** `before_block()`
(`without_surface_mixed_block().without_fp_pool_heat_above_exits()
.without_burns()`) reproduces case A's layered +360.1 / +352.8 Pa and the
6 h 58.11 % / 11.89 % / 0.61 K exactly; on `unit1_gh1097` without layers
the earlier pins reproduce exactly: 57.77 % / 12.20 % / 0.60 K,
56.29 % / 13.65 % / 1.86 K (`without_fission_products`), 60.39 % /
9.78 % / 1.32 K (gh:#1097's mapping). The `bwr_mark1` lib tests pass after
the `bur_step` refactor (below, the suite run).

**Case A, the surface block: held.** Prismatic default (8 layers)
**+341.10 Pa** (+360.1 before the block), and +341.10 Pa at 2, 4 and 16
layers (the one-layer value; before: +347.9, +354.2, +365.5); torus
default **+328.77 Pa** (+352.8 before; the one-layer +328.8). Worst
|Δ`T_pool`| 2.87e-3 K, as unlayered. The layer dependence is gone.

**Cases C and D: held**, unchanged to the printed digits (C dry: 1258.3 kg,
8.251 / 12.012 kg, 2774.9 K, 0.01 %; wet: FDI 27.6 / 27.6 kg, 4.26 %; D: Xe
in the torus gas −0.9 %, Cs in the pool −0.4 %, on the structures −1.8 %,
airborne +2.0 %, conservation 2e-14, `p_dw` up by at most 0.039 % with the
heat).

**Case E, burns** (`hydrogen_burns_in_the_building_against_the_old_containment`):

| | old model | network |
|---|---|---|
| first ignition | 1277 s | 1430 s (+12.0 %) |
| first burn: duration, H2 burned | 3 steps, 104 983 mol | 3 steps, 102 466 mol (−2.4 %) |
| second burn | 1787 s, 105 115 mol | 1678 s, 35 738 mol |
| third burn | 2225 s, 105 046 mol | 1800 s (7 steps), 22 226 mol |
| building peak pressure | held at 1 atm (bound `q/(2.5 V)`) | 402.3 kPa |
| what `tampines` burned against what sembawang asked | | 6.8e-15 (7.1e-15 before the move) |
| `without_burns` | | no burn; `Y` reaches 0.457 by 3600 s |

Outcomes: E1 held (old 1277 s inside 700–2500 s; network +12 % inside
±15 %). E2 held (−2.4 %, inside ±10 %; the asked moles burned to 7e-15).
E3 held (three 1 s steps in both, for the computed 2.38 s). **E4 MISSED,
wrong instrument:** the energy per mole burned is 275 808.27 J/mol, not
241.8 kJ/mol ±1 %, because it is the stored-energy rise on the code's own
references (`Q = −Σ ν c_i`, which includes each species' shift between
its code reference and JANAF's), not the heat of reaction. The heat of
reaction itself is R1 (241.8296 kJ/mol, +1.48e-5 of JANAF, re-run after
the move). The check replacing it, set after the run: every step's energy
per mole equals `reaction::energy_per_mole` of the building to 1e-12. That
the heat enters once is by construction (the building's only atmosphere
heat is the burn's, set in one place) and by E2. E5 held (402.3 kPa,
inside 3.0–4.8e5). E6 held. **Not predicted:** after the first burn the
network's burns are smaller and earlier (36 and 22 kmol at 248 and 370 s
after it, against the old model's 105 kmol each at 510 s spacing). The
burn pushes about half the building's gas out through the 1 m² opening at
up to 4 bar, so the building's `Y` returns to 0.10 sooner and on fewer
moles; by 1800 s its oxygen is spent (`Y` then climbs to 0.78 with no
further burn). The old building is held at one atmosphere with its oxygen
topped up by its mixing rule. This is a difference of model (a vented
volume against a fixed one), not attributed further.

**The move of the reactions to sembawang (gh:#1103): held.** Case E on
the moved code against the fingerprint taken before the move: peak `p_rb`
(`0x41188d4df052e368`), the H2 burned over the run
(`0x407436cebc794701`), `p_dw` and the drywell temperature at 3601 s are
**bit-identical**; the ignition times and the first two burns are
identical to every printed digit; the third burn's H2 differs in the 17th
digit (22 225.698955862994 against …99); at 3601 s `p_rb` differs by 8 ulp
(1.2e-15) and the building's H2 fraction by 5 ulp. The cause is the one
predicted: the stored energy now rises by `(Q − moved) + moved` (the heat,
then the species at their own enthalpies) where `tampines` added `Q` in
one operation, and the extents travel as mass rates. `without_burns` is
bit-identical. The ported R1–R6 (`tests/reaction_in_a_control_volume.rs`)
reproduce the `tampines` record: R1 H2 241.8296 kJ/mol (+1.484e-5), CO
282.9950 (0.0); R2 1146.093695 K and 3.6675 bar (−0.43 % / −0.32 % against
GRI), 20 steps against one to 1e-6; R3 exact zeros; R4 elements to
3.0e-15 (H), 3.0e-15 (O), 1.4e-16 (C), 4.3e-15 (N); R5 the 72 values bit
for bit; R6 the network volume equals the volume run alone, energy
2.3e-10 J of 1.94e6 J. The stored-energy release per mole: H2 275 808.271,
CO 279 844.461 J/mol, as before.

**Case B, 6 h worsts (default): held.** 57.99 %, 12.00 %, 0.60 K
(predicted 58.11 ± 1 %, 11.89 ± 1.5 %, 0.4–1.5 K); no burn; pinned after
the run, labelled so. The moved code gives the same 6 h run to every
printed digit.

**Case B, 30 h, network − old Δ`p_ww`** (default, n = 11; `T_pool` network
mean / top, old mean / top):

| t (h) | layered run (before) | **default now** | predicted | `T_pool` (K) | outcome |
|---|---|---|---|---|---|
| 3 | +1.40 % | +1.19 % | moves < 0.3 % | 325.45; 325.41 | held |
| 4 | +0.92 % | +0.74 % | moves < 0.3 % | 332.16; 332.12 / 332.17 | held |
| 5 | +2.99 % | +2.87 % | moves −1 … +2 % | 340.25 / 342.62; 340.43 / 343.21 | held |
| 6 | +1.98 % | +1.99 % | moves −1 … +2 % | 341.38; 341.58 / 345.55 | held |
| 8 | +2.99 % | +3.69 % | moves −1 … +2 % | 343.88 / 349.58; 344.00 / 350.46 | held |
| 10 | +2.37 % | +3.97 % | moves −1 … +2 % | 346.87; 346.95 / 356.42 | held |
| 14 | −1.05 % | +2.88 % | up 1–5 % | 354.28 / 369.21; 354.47 / 371.45 | held (+3.9) |
| 18 | +1.04 % | +5.54 % | up 1–5 % | 359.53 / 378.96; 359.10 / 380.74 | held (+4.5) |
| 23 | −1.18 % | +5.42 % | up 1–5 % | 364.00 / 387.19; 363.47 / 389.54 | **MISSED** (+6.6) |
| 24 (vent) | −37.57 % | **−17.17 %** | −15 … −32 % | 367.43 / 386.30; 364.20 / 391.01 | held |
| 24.5 | −19.56 % | −1.62 % | | 363.49; 359.33 | |
| 25 | −5.59 % | +3.07 % | | 360.73; 356.11 | |
| 26 | +0.71 % | −0.15 % | within ±1.5 % | 359.20 / 373.16; 355.07 / 373.29 | held |
| 26.5–29.5 | −0.32 … +0.64 % | **−1.27 … +12.71 %** | within ±1.5 % | 359.17–359.21 / 373.1 | **MISSED** (swings back) |
| 30 | +0.44 % | −0.10 % | within ±1.5 % | 359.23 / 373.14; 354.96 / 373.18 | held |

`Δp_dw`: +2.81 % at 5 h, +3.98 % at 6 h, +3.93 % at 8 h, +4.21 % at 10 h,
+3.09 % at 14 h, +5.92 % at 18 h, +7.55 % at 23 h, −11.01 % at 24 h,
+1.3 … +5.0 % after 25 h.

- **The vent gap halved again, −37.6 % → −17.2 %** (predicted −15 …
  −32 %): the water above the exits carries the decay heat, so the
  network's surface layer is 387.2 K at 23 h and 386.3 K at 24.0 h (old
  top 389.5 / 391.0 K; predicted 383–392 K: held) and boils as the torus
  depressurises.
- **The post-vent mean `T_pool` fell to 359.2 K** (367.4–367.9 before; old
  mean 355.0 K; predicted 358–366 K: held): the deep water no longer
  receives the decay heat. The gap to the old mean is now 4.2 K (12.4–12.9
  K before).
- **14–23 h now runs high, +2.9 … +5.5 %**, against −1.2 … +1.0 % before.
  The network's surface layer is now within 2.4 K of the old top layer
  (369.2 / 371.5 K at 14 h, 379.0 / 380.7 K at 18 h, 387.2 / 389.5 K at
  23 h; 6–16 K below it before), so the old model's hot top layer no
  longer offsets the network's displacement (+3 … +7 %, earlier
  sections). That is the expected direction; the 23 h move (+6.6 %) is
  beyond the predicted 5 %. **Not attributed yet:** the attribution runs
  below separate the block from the placement.
- **The flange opens at 15.484 h** (16.254 h before; old 16.824 h;
  predicted within 1 h of 16.25 h: held, 0.77 h early); peak `p_dw`
  874.7 kPa. The building's hydrogen 2.79 % at 23 h, 2.71 % at 30 h (old
  3.36 % / 3.73 %).
- **No burn in either model** (`Y` peaks 0.0303 network, 0.0394 old,
  against 0.10): held.
- **The drywell sump's pool heat to its atmosphere:** 52.7 kJ over 30 h
  (predicted under 1 MJ: held); torus and building 0.
- **Worst over 30 h:** |Δ`p_dw`| 57.99 % (the 4.956 h step), |Δ`p_ww`|
  30.13 %, |Δ`T_pool`| 4.69 K (12.94 K before).
- **Cost:** 1266 s for the 30 h (11.7 ms a step, one old replay included),
  with up to three other runs beside it; at most 100 Picard iterations.

### Attribution runs after the first 30 h result, predictions written before they ran

The first default 30 h run (results below) brought back post-vent swings in
`p_ww` (+12.7 % at 26.5 h, +3.8 % at 29 h, +4.7 % at 29.5 h) and moved the
23 h gap by +6.6 % (predicted 1–5 %). Hypothesis for the swings: the
surface mixed block now spans the upper layers that the vent's boiling
has mixed to saturation, and the block flashes as one in a step, as the
single pool did (the 225 s relaxation cycle above), where before only the
top layer could flash. Predicted, one change at a time from the default:

- *`without_surface_mixed_block`* (the decay heat above the exits kept):
  the post-vent swings below 3 % (as the layered run's), the vent within
  ±3 % of the default's −17.2 %, 14–23 h within ±1 % of the default.
- *`without_fp_pool_heat_above_exits`* (the block kept): 14–23 h back to
  within ±1 % of the layered run (−1.2 … +1.0 %), the vent near its
  −37.6 %; the swings present (over 3 % somewhere in 26.5–29.5 h) if the
  block is their cause.
- *n = 22* (the default): every window within 0.3 % of `p_ww` of n = 11's,
  the vent included (0.98 % before the block).

**Attribution outcomes** (30 h each, network − old Δ`p_ww`):

| t (h) | default | without the block | without the placement | n = 22 |
|---|---|---|---|---|
| 14 | +2.88 % | +3.11 % | −1.21 % | +2.81 % |
| 18 | +5.54 % | +5.69 % | +0.94 % | +5.44 % |
| 23 | +5.42 % | +5.50 % | −1.22 % | +5.29 % |
| 24 (vent) | −17.17 % | −18.04 % | −35.69 % | **−8.15 %** |
| 24.5 | −1.62 % | −3.67 % | −19.37 % | +11.27 % |
| 26.5–29.5 | −1.27 … **+12.71 %** | −0.24 … +1.24 % | −1.24 … **+8.35 %** | −1.12 … **+5.67 %** |
| 30 | −0.10 % | +0.58 % | −0.12 % | +0.39 % |
| surface layer 23 / 24 h (K) | 387.19 / 386.30 | 387.15 / 386.37 | 373.10 / 377.51 | 387.00 / 388.97 |
| post-vent mean `T_pool` (K) | 359.2 | 359.3–359.5 | 367.2–367.9 | 357.8–357.9 |
| flange (h) | 15.484 | 15.454 | 16.280 | 15.497 |

- *Without the block:* the swings below 3 % (largest +1.60 % at 26 h):
  **held**; the vent −18.04 % (within ±3 % of −17.17 %): held; 14–23 h
  within 0.25 %: held. **The block causes the post-vent swings.**
- *Without the placement:* 14–23 h −1.21 … +0.94 % (the layered run's
  −1.18 … +1.04 %): held; the vent −35.69 % (near −37.6 %): held; the
  swings present (+8.35 % at 26.5 h, +7.44 % at 28 h): held. **The
  placement carries the 14–23 h move and the vent's improvement; the
  block, with or without it, brings the swings.**
- *n = 22:* 14–23 h within 0.13 %: held; **the vent MISSED**, −8.15 %
  against −17.17 % (predicted within 0.3 %; 0.98 % before the block), and
  the swings are there too (+5.67 % at 27 h).

**What this means.** The block does what it was built for where the
column is mixed below the surface (case A, `tampines`' L9: no layer
dependence left). After the vent, though, the vent's boiling mixes the
upper layers to saturation. The block then spans them, and the flashing
(which acts on the block as the exchange does) flashes that whole block in
one 1 s step. This is the one-step flash and 225 s relaxation cycle the
unlayered pool showed (section "The 26.5–29 h post-vent swings"), now on
the upper part of the pool. Through the same mechanism the depressurisation
at 24–24.5 h depends on the layer count (the block's extent depends on
`n`). The block's physics is consistent: a saturated, mixed block does
flash as a whole. The step-scale burst is what a 1 s explicit flash of
that much water gives, as the earlier section found, not a measured
oscillation. ~~**Left open, not hacked**~~ **CORRECTED 2026-11-01**
(gh:#1107): fixed in `tampines` by the implicit flash (section gh:#1107
below); the record as it stood: (no threshold or cap was added):
the options are a flash limited to the step (MELCOR holds bubbles to a
0.4 void, `pool_surface_step`'s comment) or flashing on the top layer
while the exchange sees the block. Either is a `tampines` change that
needs its own V&V and a maintainer decision.

## gh:#1107: the flash implicit in the pressure it raises (2026-11-01)

> Research, education and V&V only. AI draft, unreviewed.

**Cause and upstream** (full record in `crates/tampines/verification_and_validation/control_volume.md`,
section "The flash is implicit in the pressure it raises"). `tampines`'
`ControlVolume::surface` flashed a pool above saturation **explicitly**: at
the pressure before the flashed vapour joins the atmosphere, in full, in one
step, so the pool ended subcooled against its own end-of-step pressure. The
overshoot is the loop gain `G = (M c / h_fg)(dT_sat/dp)(∂p/∂m)`, about 85 for
the unlayered torus at 26 h (the 418 kg, +38.8 kPa burst above), and it grows
with the mass that flashes, which the surface mixed block made the upper
part of the pool after the vent. MELCOR's CVH puts the pool's vapour in the
equation of state at the end-of-step pressure (CVH/FL RM §5.1.3, Eq 5-30),
solves the vapour's transfer to the atmosphere implicitly with the pressures
(§4.2.1, Eqs 4-26 to 4-36, introduced because the explicit form was unstable
for boiling pools) and requires end-of-step consistency (§4.3). This
model's `pool_surface_step` is explicit, as `tampines` was.

**Fix (in `tampines`):** the flash's end state satisfies
`T_block = T_sat(p_end)` with `p_end` the pressure after its own vapour
(default ON; `ControlVolume::without_implicit_flash` restores the explicit
flash bit for bit, here `NetworkMechanisms::without_implicit_flash`). No
cap, no tuned constant.

### Predictions, written before any run of this change

- **Case A** (relief steam into a 303 K torus): no flash, so bit-identical;
  `p(n) − p(1)` < 1e-4 Pa held.
- **6 h pins** (`the_first_hours_of_unit_1_against_the_old_containment`,
  default and every ablated configuration): unchanged to the printed digits
  (57.99 %, 12.00 %, 0.60 K): before 6 h the torus pool is 30–80 K below
  saturation at several bar. The one place a flash could occur early is the
  drywell sump after the 4.956 h blowdown; if it does, the worsts move by
  under 0.1 %.
- **30 h, network − old Δ`p_ww`, default (11 layers):**
  - 3–23 h: within 0.1 % of the previous row (the default above), the flange
    within 0.05 h of 15.484 h (no torus flash before the vent; a drywell-sump
    flash, if any, small);
  - 24 h (vent): between −8 % and −20 % (previous −17.17 %; the explicit
    block flash added pressure in bursts and the implicit one supplies the
    same steam smoothly, so the sign of the move is not predicted);
  - 26.5–29.5 h: every sample within ±1.6 % (the `without_surface_mixed_block`
    run's −0.24 … +1.24 %, largest +1.60 % at 26 h), against −1.27 … +12.71 %;
  - 30 h within ±1.5 %.
- **n = 22:** the vent within 2 % of `p_ww` of n = 11's (8.15 % against
  17.17 % before, 9.0 apart), and 26.5–29.5 h within ±1.6 %.
- **Cost:** within +30 % of 1266 s (the root costs a few `refresh` calls on
  steps that flash).

### Results (2026-11-01, release, Rust 1.99.1)

- **Case A: held**, unchanged: +341.10 Pa at 2, 4, 8, 16 layers and the
  default 8 (torus +328.77 Pa), so `p(n) − p(1)` < 1e-4 Pa.
- **6 h pins: held**, 57.99 % / 12.00 % / 0.60 K and every ablated
  configuration's pins pass. The default *does* flash once before 6 h
  (the drywell sump after the 4.956 h blowdown, inferred: `p_dw` at 6 h is
  2 Pa lower, 435 648 against 435 650 Pa, the first printed difference),
  inside the predicted 0.1 %. Case E's burn fingerprint (peak `p_rb`
  `0x41188d4df052e368`, H2 burned `0x407436cebc794701`) is bit-identical:
  the building does not flash.
- **The explicit ablation reproduces the run before the fix**
  (`FUKUSHIMA_NETWORK_ABLATE=implicit_flash`, 27 h): worst `|Δp_ww|`
  30.13 % at 25.39 h and `|ΔT_pool|` 4.69 K, the previous default's worsts.

**Case B, 30 h, network − old Δ`p_ww`:**

| t (h) | default before (block, explicit) | **default now (implicit)** | predicted | n = 22 before | **n = 22 now** |
|---|---|---|---|---|---|
| 3–14 | +1.19 … +3.97 % | same to the printed digits | within 0.1 % | | |
| 18 | +5.54 % | +5.54 % | within 0.1 % | +5.44 % | +5.44 % |
| 23 | +5.42 % | +5.42 % | within 0.1 % | +5.29 % | +5.29 % |
| 24 (vent) | −17.17 % | **−17.94 %** | −8 … −20 % | −8.15 % | **−10.41 %** |
| 24.5 | −1.62 % | −3.56 % | | +11.27 % | +2.30 % |
| 25 | +3.07 % | +3.67 % | | +4.80 % | +6.41 % |
| 25.5 | | +4.10 % | | +11.11 % | +4.87 % |
| 26 | −0.15 % | +1.91 % | | +0.03 % | +2.01 % |
| 26.5–29.5 | −1.27 … **+12.71 %** | **−0.12 … +0.99 %** | within ±1.6 % | −1.12 … +5.67 % | **−0.15 … +0.96 %** |
| 30 | −0.10 % | +0.36 % | within ±1.5 % | +0.39 % | +0.35 % |
| surface layer 23 / 24 h (K) | 387.19 / 386.30 | 387.19 / 386.43 | | 387.00 / 388.97 | 387.00 / 389.12 |
| post-vent mean `T_pool` (K) | 359.2 | 359.3–359.5 | | 357.8–357.9 | 358.0–358.1 |
| flange (h) | 15.484 | 15.484 | within 0.05 h | 15.497 | 15.497 |
| worst `\|Δp_ww\|` over 30 h | 30.13 % | 24.69 % (the vent) | | 28.40 % | 16.85 % |
| run time; Picard iterations at most | 1266 s; 100 | 1291 s; 35 | within +30 % | 1938 s; 100 | 1607 s; 44 |

**Step-by-step traces, 24–26.75 h** (`FUKUSHIMA_NETWORK_TRACE=24,26.75`,
9901 steps each; the half-hour samples can alias a burst cycle, so this is
the instrument for "bursts gone"):

| | explicit (ablation) | implicit (default) |
|---|---|---|
| one-step rises of `p_ww` over 2 kPa | **244** | **0** |
| largest one-step rise | 30.9 kPa (25.52 h) | 142 Pa (24.01 h, in the vent's opening) |
| Δ`p_ww` range, 25–26.75 h | −4.02 … +30.13 % | −1.92 … +5.21 % (maximum at 25.23 h, smooth) |

**Outcomes.**

- **The post-vent bursts are gone: held.** 26.5–29.5 h −0.12 … +0.99 %
  (predicted ±1.6 %; before −1.27 … +12.71 %); no one-step rise over 2 kPa
  anywhere in 24–26.75 h against 244 with the explicit flash.
- **3–23 h and the flange: held** (same to the printed digits; 15.484 h).
- **The vent: held at n = 11** (−17.94 %, inside −8 … −20 %), **MISSED for
  the layer count:** n = 22 gives −10.41 %, 7.5 points from n = 11 (9.0
  before; predicted within 2). The flash's overshoot carried only a
  fraction of the vent's `n` dependence. What remains goes with the hot
  surface layer's resolution: at 24 h the surface layer is 386.43 K at 11
  layers and 389.12 K at 22 (the old top layer 391.01 K), so the finer
  column holds a thinner, hotter top that boils harder as the torus
  depressurises. **Not attributed by ablation here**; left open.
- **24.5–26 h runs high and smooth**, +1.9 … +5.2 % (the trace's maximum
  +5.21 % at 25.23 h; n = 22 +6.41 % at 25 h). It was not predicted (no
  window was set for 24.5–26 h); the previous default's samples there were
  partly bursts (+3.07 % at 25 h), so the two are not comparable sample by
  sample. With the bursts gone this is the network's depressurisation
  against the old model's, not a numerical cycle. Not attributed.
- **Cost: held**, +2 % at n = 11 (with up to three other runs beside it)
  and −17 % at n = 22; the Picard iteration now converges in at most 35
  and 44 iterations, where the explicit flash hit the 100 cap.

## What would move if the plant switched (the decision's evidence; **it switched 2026-11-02**, gh:#1099: what moved is in that section above)

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

~~The fission products, aerosols, iodine and burns cannot be compared yet:
the network carries none of them.~~ **CORRECTED 2026-11-01:** the fission
products, aerosols and iodine are carried since gh:#1100 (continued) and
compared in cases D and B; ~~burns are still not (no non-condensable sink in
`tampines`).~~ **CORRECTED again 2026-11-01:** burns are carried and
compared in case E (section "the surface block, the decay heat above the
exits, and burns"). **The building's hydrogen differs in
kind.** The old model's drywell passes the flange's 0.75 MPa opening
pressure (786 kPa at 21 h, 805 kPa at 23 h), so its flange leaks hydrogen
into the building. The network's drywell peaks at 674 kPa, so its flange
never opens and its building gets no hydrogen. ~~This difference comes from
cause 3 (no MCCI gases on the network)~~ **CORRECTED 2026-11-01:** with the
MCCI gases coupled (gh:#1100) the network's drywell peaks at 711.5 kPa and
its flange still does not open; the open-loop replay attributes the
difference to the old containment's fission-product decay heat (without it
the old drywell peaks at 680 kPa). **CORRECTED again 2026-11-01** (gh:#1100
continued): with the fission products' decay heat on the network its
drywell peaks at 797.2 kPa and its flange opens at 17.25 h, against the old
model's 16.82 h; the building's hydrogen then reaches 1.74 % against
3.73 %, not attributed. It would move the explosion timing.

**Recommendation (for the maintainer, not acted on):** do not switch yet.
(**Superseded 2026-11-02:** the maintainer decided "switch it" on gh:#1099;
see the section above.) The
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
are now the largest differences.) (**2026-11-01, gh:#1100 continued:** the
fission products and their decay heat are on the network; the 5–23 h gap
is −3.8 … +2.5 %, the flange opens within 0.43 h of the old model's, and
the pool layers are the one large difference left, with burns, which need
a `tampines` addition.) (**2026-11-01, gh:#1100 continued, layers:** the
torus pool is layered and the sumps start dry; what is left in the pool is
where its decay heat goes and a resolution dependence of the surface
exchange in `tampines`; burns still need a `tampines` addition.)
(**2026-11-01, gh:#1100 continued, block, placement, burns:** the surface
exchange no longer depends on the layer count on a mixed pool; the torus's
decay heat goes where the old model puts it, halving the vent gap again
(−17.2 %) and the post-vent pool gap (4.2 K); the building burns by the
old model's BUR code, with the chemistry in sembawang (gh:#1103); what is
open is the block's one-step flash after the vent, which brings back
post-vent swings and an `n` dependence at the vent.)

## gh:#1099: the plant switched to the network (2026-11-02)

**Maintainer decision on gh:#1099 (2026-11-02): "switch it".** `Plant` now
builds `ContainmentNetwork` by default (`Mechanisms::network_containment`,
pinned by `bwr_mark1::tests::the_containment_network_is_the_default`); the
lumped `Containment` is the explicit ablation
`Mechanisms::without_network_containment()`, under which every result
recorded before this date is reproduced bit for bit (the 30 h fixture,
renamed `headless_30h_lumped.csv`, byte for byte). **This is now closed
loop**: the vessel and the core see the network's drywell, where every case
above was an open-loop replay of the lumped plant's snapshots.

**Results stay tentative.** The network's geometry (the torus V(z), the
drywell as a prism over a sump pit, the ports' elevations, the building) is
~~not drawn yet (gh:#1110)~~ **drawn since 2026-11-02 (gh:#1110; section
"gh:#1110" below) and awaiting a human check**, and the crate's geometry rule
keeps every result on it tentative until that check. The vent's depressurisation depends on the
torus pool's layer count (gh:#1108; −17.9 % at 11 layers against −10.4 % at
22 in the open-loop case B above), so the vent-window numbers below carry
that resolution dependence.

### Predictions, written before any closed-loop run (2026-11-02)

Against the lumped plant (its numbers in brackets):

1. **Lower-head failure 5.48 ± 0.05 h** \[5.4839 h\] **and depressurised
   4.958 ± 0.01 h** \[4.9578 h\]: both before the containment matters to
   the vessel; the core sees the drywell only through the leak's back
   pressure, and the network's drywell is within 4 % of the lumped one
   before 4.95 h (case B).
2. **H2 at 25 h within ±10 %** \[470.75 kg\]: the run is chaotic at the
   1e-9 level (`ensemble.md`), so a containment change after the head fails
   moves it by an amount no smaller than the ensemble's spread.
3. **Drywell peak 0–25 h 0.70–0.85 MPa** \[0.81295 MPa\]: open loop the
   network peaked at 0.797 MPa and its flange opened at 17.25 h.
4. **Vent within ±0.2 h of the lumped plant's; no building burn by 26 h**
   (open loop the building reached 1.74 % H2 against the lumped 3.73 %).
5. **Every bound and every fission-product balance holds** at every step of
   30 h on the network (`the_network_plant_over_30_hours`), with the lumped
   tests' tolerances.
6. **`three_workers_are_one_worker` bit-identical** with the network held in
   its worker (`ContainmentWorker`) and only snapshots and results crossing.
7. **BSAF figures** (`tests/bwr_mark1_vs_bsaf_figures.rs`, lumped in
   brackets): RPV against the 2 measured points within 0.05 MPa of
   \[3.4388 / 6.5826 MPa\] (they sit near the head's failure, where the
   vessel does not yet see the containment); drywell against the 33
   measured points mean 0.05–0.10 MPa, max 0.20–0.35 MPa \[0.0670 /
   0.2573\]; drywell inside the codes' envelope 0.55–0.80 \[0.6703\];
   H2, Xe and I envelopes within 0.1 of \[0.9885, 0.8125, 0.8542\].
8. **Cost: the 30 h run 1100–1500 s** \[10.2 s measured today for the
   lumped plant\], from the open-loop 1291 s plus the plant's 10 s.

### Results (2026-11-02, release, Rust 1.99.1; every new pin set after the run, gh:#1099)

**The ablation reproduces the old default bit for bit:** the 30 h trace
under `without_network_containment` equals the fixture committed before the
switch byte for byte (`headless_30h_lumped.csv`), and every pin recorded on
the lumped plant passes on it unchanged (`compare`'s baseline and its
earlier models, the BSAF figures, the 500 h comparisons, the conservation
tests).

**BSAF metrics, 26 h** (`compare::tests::the_network_baseline_is_pinned`):

| Quantity | BSAF band (tier) | Lumped | Network | Predicted | Outcome |
|---|---|---|---|---|---|
| lower-head failure | 7.45–17.16 h (B) | 5.4839 h (below) | **4.6894 h (below)** | 5.48 ± 0.05 h | **MISSED**, 0.79 h early |
| depressurised (< 2 MPa) | 5–12 h (A) | 4.9578 h (below) | **4.6894 h (below)** | 4.958 ± 0.01 h | **MISSED** |
| in-vessel H2 at 25 h | 400–1310 kg (C) | 470.75 kg (inside) | 417.19 kg (inside) | ±10 % | **MISSED** (−11.4 %) |
| drywell peak 0–25 h | 0.8–1.0 MPa (C) | 0.81295 MPa (inside) | **1.00367 MPa (above)** | 0.70–0.85 MPa | **MISSED** high |
| wetwell vent flowing | 23.5–24.68 h (A) | 23.7336 h (inside) | 23.7336 h (inside) | ±0.2 h | held |
| building explosion | 24.83 h (A) | never | never | never | held |

**What moved, with its cause (closed loop):**

1. **The lower head fails 0.79 h earlier, before the steam line.** On the
   lumped plant the main steam line ruptured by creep near 4.96 h and the
   head failed at 5.48 h; on the network the head fails first, at 4.69 h,
   with the vessel still at high pressure (8.20 MPa at 4.5 h), so the vessel depressurises through
   the failed head (hence the identical depressurisation time), the steam
   line never ruptures, and the FDI fall is flagged as a high-pressure
   ejection (`fdi_hpme`; still treated as low-pressure, as on the lumped
   plant). To 4 h the containments' pressures differ by at most 1.5 % (the
   pool's displacement, case B), at 4.5 h the drywell is 8 % higher; the core sees the drywell only
   through the leak's back pressure and the RPV wall's heat loss. The
   lumped plant's own 1e-9 ensemble reaches heads as early as 4.50 h
   (`ensemble.md`), so this is read as **a branch change of a chaotic
   run**, not attributed to a containment mechanism; no ablation was run.
2. **The drywell runs 4–22 % higher from 5 h to the vent** (0.446 against
   0.419 MPa at 6 h, 0.570 against 0.526 at 10 h, 0.838 against 0.763 at
   18 h, 0.982 against 0.805 at 23 h; `headless_30h.csv` against
   `headless_30h_lumped.csv`), and peaks at 1.0016 MPa at a 10-minute
   sample (1.00367 MPa at 1 s), just above BSAF's band and above the whole
   range of the lumped ensemble (0.785–0.964 MPa). Its flange opens 1.3 h
   earlier (15.33–15.5 h against 16.67–16.83 h). Partly the open-loop
   difference (case B and the gh:#1100 sections: the pool's displacement,
   IF97, the layered torus), partly the earlier head (more decay heat and
   MCCI gas ex-vessel, 552 kg of H2 and 803 kg of CO from the cavity by
   26 h); not separated by ablation.
3. **The vent** opens at the same moment (the operators' order; both disks
   burst), and the post-vent pool is hotter on the network (359.8 against
   355.1 K mean at 26 h; surface layer 374.6 K): the gh:#1108 layer-count
   dependence applies here.
4. **The building** reaches 3.9 % H2 at 26 h (lumped 3.7 %), Le Chatelier's
   Y 0.0416 (0.0394), no burn in either; noble gas to the building 0.0142
   (0.0232), vented 0.9827 (0.9733).

**BSAF figures** (`tests/bwr_mark1_vs_bsaf_figures.rs`; the tolerance gates
kept, the lumped pins kept on the ablation, the network's pinned beside them;
**no gate fails**):

| Check | Lumped | Network | Predicted | Outcome |
|---|---|---|---|---|
| measured RPV (2 points), mean / max | 3.4388 / 6.5826 MPa | 3.4177 / 6.5762 MPa | within 0.05 | held |
| measured drywell (33 points), mean / max | 0.0670 / 0.2573 MPa | **0.1028** / 0.2310 MPa | mean 0.05–0.10, max 0.20–0.35 | mean **MISSED** (worse), max held |
| inside the codes' envelope: RPV / drywell | 0.6154 / 0.6703 | 0.5714 / 0.6374 | drywell 0.55–0.80 | held |
| inside: H2 / Xe / I | 0.9885 / 0.8125 / 0.8542 | 0.9885 / 0.8333 / 0.9167 | within 0.1 | held |
| Cs on DW structures / in S/C water (Fig 2.6) | 0.8667 / 0.9663 | 0.7867 / 0.9663 | | |
| I on DW structures / in S/C water (Fig 2.6) | 0.0 / 0.0562 | 0.0 / 0.0899 | | |

The drywell against the measurement is **worse by half** in its mean
(0.0670 → 0.1028 MPa), the unflattering move of this switch, recorded and
not tuned. The two 500 h comparisons (Fig 2.3a, Fig 5.1) stay on the lumped
plant; the network's 500 h is an ignored test (several hours), **not run**.

**The other predictions.** 5 (bounds and balances at every step of 30 h):
held (`the_network_plant_over_30_hours`). 6 (`three_workers_are_one_worker`
bit-identical, the network held in its worker): held, 2000 steps; the
browser's shipped entry point likewise
(`dhoby-ghaut` `engine::tests::the_shipped_solve_entry_point_matches_the_inline_step`).
8 (cost): see the timing below.

**Re-pinned numbers, old → new** (old pins kept, valid, on the ablation):

| Pin | Lumped (kept on the ablation) | Network (new, set after the run) |
|---|---|---|
| `headless_30h.csv` | (now `headless_30h_lumped.csv`, byte-identical) | re-blessed |
| baseline: head / depressurised | 5.4839 / 4.9578 h | 4.6894 / 4.6894 h |
| baseline: H2 at 25 h / UO2 released | 470.75 / 2.672 kg | 417.19 / 4.093 kg |
| baseline: drywell peak | 0.81294607 MPa | 1.00367475 MPa |
| baseline: vent / explosion | 23.7336 h / never | 23.7336 h / never |
| BSAF figures (table above) | as above | as above |
| 26 h: Xe vented / building | 0.9733 / 0.02318 | 0.98265 / 0.014214 |
| 26 h: building Y | 0.0394 | 0.04159 |

**Not re-measured on the network (cost), stated:** the 70-row ablation
table (`compare::tests::the_ablation_table_on_the_network`, ignored, about
70 × 20 min), the 256-member ensemble and the input sensitivities
(`ensemble.md`, `input-sensitivity.md`; their examples take
`FUKUSHIMA_CONTAINMENT=lumped` to reproduce the records), the 500 h figures.
Those records describe the lumped plant.

### Timing (2026-11-02, release, Rust 1.99.1, one 4-core container, measured while a 3-job `cargo test` ran beside it, so the absolute numbers are pessimistic)

| Run | Wall time | Faster than real time |
|---|---|---|
| 30 h headless, network (`fukushima_web --headless 108000 600`) | 1403.4 s | 77× |
| 30 h headless, lumped (`--lumped-containment`) | 9.93 s | 10 900× |

Both traces matched the committed fixtures byte for byte (`headless_30h.csv`,
`headless_30h_lumped.csv`). Prediction 8 (1100–1500 s): **held**.

**Three workers against one** (`fukushima_web --bench-workers`, natively on
the existing `link::start_native` engine threads; every frame of each window
bit-identical between the two modes):

| Window | Inline | Three workers | Speed-up |
|---|---|---|---|
| steps 0–3600 (before the head fails) | 3.470 ms/step | 7.503 ms/step | **0.46** |
| steps 16 200–18 000 (4.5–5 h, the core busiest) | 18.020 ms/step | 26.238 ms/step | **0.69** |

**The three-worker split is slower natively, not faster**, the opposite of
what the split was built to buy. The cause is not measured, but the likely one
is this: the link's engine loop sleeps 3 ms when it finds no request
(`link::start_native`), which is the same order as a whole inline step, and
the containment network dominates the step, so running the core beside it
saves only the core's share. The browser's Web Workers wake on `postMessage`,
so this native number does not settle the browser's speed-up; that is **not
measured**. Left as found, not tuned; the 3 ms poll is the first thing to
test (gh:#1105).

**Re-measured after the fix (2026-11-02, gh:#1105, 09fa9f995).** The engine
thread now wakes on `send` instead of a 3 ms poll. Hardware: Intel Xeon @
2.10 GHz, 4 logical cores (3 threads used), 15 GB RAM, Linux, CPU only; the
machine was otherwise idle. The prediction, 0.95–1.15× in both windows, was
written before the change:

| Window | Inline | Three workers | Speed-up (before → after) |
|---|---|---|---|
| steps 0–3600 | 3.916 ms/step | 3.861 ms/step | 0.46 → **1.014** |
| steps 16 200–18 000 | 18.084 ms/step | 17.831 ms/step | 0.69 → **1.014** |

**Held.** Every frame is bit-identical. The poll was the whole overhead; the
split now breaks even, and no more, because the network dominates the step.

## Units 2 and 3 by configuration (gh:#1101; ~~a sketch;~~ no plant data invented)

The network reduces a unit to `Params` (volumes, areas, set points, the
snapshots' sources) plus `NetworkConfig` (shapes, elevations, sump pits,
inertial lengths). Units 2 and 3 are BWR/4s in Mark I containments, with
RCIC and HPCI instead of isolation condensers, which lives in the vessel
model and not here. A unit would be
`ContainmentNetwork::with_config(&Params::unit2(), NetworkConfig::unit2(), ..)`.
~~Neither constructor exists yet, because the data below is not held.~~
**CORRECTED 2026-11-01 (gh:#1101):** both exist now, for Unit 2 and Unit 3,
with every field marked [S], [D] or [A]. See the record below. The list that
follows was the data needed. What was found is in
`units23_open_data.md`. It would
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

### The constructors and their check (gh:#1101, 2026-11-01)

`Params::unit2()`, `Params::unit3()`, `NetworkConfig::unit2()` and
`NetworkConfig::unit3()` carry the values in `units23_open_data.md`. Its
"Step 3" table gives each field's mark and source. In brief:

| | [S] sourced | [D] derived | [A] assumed |
|---|---|---|---|
| `NetworkConfig` | torus major radius 16.75 m | building floor 646.8 m² (the volume over the torus-bottom-to-roof height, from TEPCO16's elevations and the 46 m building) | everything else, borrowed from Unit 1 |
| `Params` | power, assemblies, control rods, RPV radius and height, drywell, wetwell gas and pool volumes, design pressure, rupture disk, downcomer count, U3 building volume | inventories (power ratio), downcomer area (main-vent area ratio), drywell floor and structures ((volume ratio)^(2/3)), pool surface and torus shell (from the torus) | U2 building volume (from Unit 3), vent area, vacuum breakers, submergence, flange, initial state, every vessel and core field not listed |

**No comparison with the accident is made ~~or possible~~ here.** ~~The crate has
no RCIC or HPCI model and no Unit 2 or 3 sequence.~~ **CORRECTED 2026-11-02
(gh:#1109):** it now has both (`turbine_pump`, `units23`,
`Plant::for_unit`); the comparison with the HRTD chronology is
[`units23_sequence.md`](units23_sequence.md).

**Unit 3's rupture disk** is a sourced conflict: 0.528 or 0.549 MPa abs.
0.528 is used, because the disk is set at the PCV maximum working pressure
(AESJ4), which NISA gives as 0.427 MPa g (0.528 MPa abs). 0.549 is kept as
`params::UNIT3_RUPTURE_DISK_ANS_PA` and is not averaged in.

**Check** (`tests/units_2_and_3_network.rs`; predictions written before the
run, in its module docs):

- **G, geometry**, read from the assembled network:
  - gas space 3160.000 m³;
  - pool 2978.04 m³ (2 965 100 kg);
  - tube radius 4.3087 m. The free volume is 0.9375 of the sourced
    geometric torus (r = 4.45 m); the difference is internals, which are
    not drawn;
  - level 4.2083 m, area there 906.26 m²;
  - downcomers 0.606 m × 96 = 27.71 m², which is 1.039 of the main vents'
    26.66 m²;
  - 14 pool layers;
  - drywell top 26.94 m, building top 58.745 m.

  All predictions held. The drywell's shape and elevation are Unit 1's [A],
  so its top is not a sourced number.
- **A′, 300 s of case A's relief steam**, every mechanism on:

  | unit | ΔT_pool | p_ww | water residual | energy residual / inflow | iterations |
  |---|---|---|---|---|---|
  | 1 | +1.0590 K | 105 269 Pa | −1.2e-9 kg | −3.4e-14 | ≤ 5 |
  | 2 | +0.6224 K | 105 199 Pa | +2.3e-9 kg | −5.3e-14 | ≤ 5 |
  | 3 | Unit 2's, bit for bit | | | | |

  - The heating ratio is 0.588, the inverse ratio of the pool masses, as
    predicted.
  - There is no downcomer or breaker flow.
  - The balance is the network's own (`boundary_inflow`). The lumped walls
    are outside it.
  - The vent, disk, flange, fission products and burns are not reached in
    300 s.
- **One prediction missed**
  (`params::tests::units_2_and_3_derive_from_their_sources`):
  - The torus shell's share above the pool was estimated by hand at 0.50741
    and computed as 0.50733. The hand arithmetic used the IF97 volume share,
    not 2980/6140.
  - The constant now holds the computed value.
- **Not drawn.** The crate's geometry rule asks for images of a built
  geometry. No image of the Unit 2/3 network (V(z) tables, ports) was made
  in this change. Unit 1's network has none either. That is open.

## Doubts, not settled here

- ~~The vent's faster depressurisation on the network (cause 4) is not
  attributed by ablation.~~ **CORRECTED 2026-11-01:** attributed by the
  open-loop replays (gh:#1100 section): the old model's top layer, heated
  to 391 K by the fission products' decay heat.
- The start-of-step equalising limit lags by one step on breakers and
  downcomers. A limit from the latest iterate, or a Newton step, would remove
  it.
- 588 halvings by 30 h: the Picard step struggles after the vent. The
  failures were not classified (which path, which switch). (953 with the
  cavity coupled; **3** once the fission products' decay heat was coupled,
  2026-11-01: still not classified.)
- ~~After the vent, with the pool at saturation, the network's torus swings
  by up to 6 % at the half-hour samples (26.5–29 h); not attributed.~~
  **CORRECTED 2026-11-01:** attributed to one-step flashes of the whole
  saturated pool (a 225 s relaxation cycle, up to +38 % in one step on the
  unlayered network); with the layered pool (the default) they are gone.
- ~~The surface exchange of a layered pool depends on the layer count
  through the top layer's thickness (case A: +6 Pa a doubling; the vent:
  1 % of `p_ww` between 11 and 22 layers): a `tampines` matter, described
  in the layered-pool section.~~ **CORRECTED 2026-11-01:** fixed by the
  surface mixed block (case A: +341.10 Pa at every `n`). **New doubt:**
  after the vent the block flashes whole in one step: post-vent swings up
  to +12.7 % and the vent window 9 % apart between 11 and 22 layers
  (attribution in the block section). ~~Open.~~ **CORRECTED 2026-11-01**
  (gh:#1107): the swings are fixed by the implicit flash (26.5–29.5 h
  −0.12 … +0.99 %, no burst in the trace); the vent window is still 7.5
  points apart between 11 and 22 layers, open, not attributed.
- The drywell's sump pit, the light bulb drawn as a prism, and the drywell
  floor at the torus top are \[A\]. The +90 Pa gas column they imply in the
  downcomers is physical but unverified.
- The building is a control volume open to the environment through 1 m²
  \[A\], where sembawang holds it at one atmosphere exactly.

## gh:#1110: the assembled network, drawn (2026-11-02)

**Methodology.** `ContainmentNetwork::geometry_svg` reads back everything
from the built `CvNetwork`, never from the named constants:
- each volume's free area against elevation, from its `VolumeAltitudeTable`;
- the pool level and the swollen level;
- the torus layers' boundaries, placed as tampines places them (mass below
  at the pool's mean density);
- every port's elevation and opening height;
- every flow path, between its junction elevations.

`examples/containment_network_geometry.rs` writes one SVG per case into
`geometry/`; the PNGs are Chromium renders of those SVGs. The three
volumes share the plant datum (torus bottom 0 m). A panel's width is its
free area normalised to that volume's widest, so shapes compare within a
panel only.

`drawing::tests::the_drawing_shows_what_the_network_holds` checks, for all
four cases:
- every port and path is labelled;
- the torus has one boundary per layer, with the top one at the pool level
  (to 1e-6 m);
- the downcomer exits and quenchers sit where `port_elevations` says;
- the exits are submerged.

**Results** (release, initial state, read back):

| | Unit 1 | Unit 1, gh:#1097 config | Units 2 and 3 |
|---|---|---|---|
| torus | 0–7.683 m, 4369 m³, widest 723.5 m², pool 3.237 m, 11 layers | same | 0–8.617 m, 6138 m³, widest 906.3 m², pool 4.208 m, 14 layers |
| downcomer exits / quenchers | 2.028 m / **0.000 m** | 2.028 m / 2.028 m | 2.999 m / **0.000 m** |
| drywell | pit 6.183–7.683 m (dry), prism 200 m² to 24.718 m, 3410 m³ | 3411 m³, 0.5 m of sump water | pit 7.117 m, prism 231.3 m² to 26.939 m, 4240 m³ |
| downcomer entry / vacuum breaker (DW) | 9.683 m / 9.683 m | same | 10.617 m / 10.617 m |
| vent and vacuum breaker (WW gas) | 7.183 m | same | 8.117 m |
| head flange | 24.518 m | 24.523 m | 26.739 m |
| building | pit −1.5 m, 650 m² to 40.0 m, 26 000 m³, openings 39.0 m | same | 646.8 m² to 58.75 m, 38 000 m³, openings 57.75 m |

Units 2 and 3 draw identically (only the titles differ): their geometry
inputs are shared (`UNITS23_*`, "Step 3" of `units23_open_data.md`).

**What I checked in the images, and what stands out** (for the maintainer to
confirm or correct; nothing was changed because of it):

1. **The drywell is a 200 m² prism** (231 m² for Units 2/3) over a 1.5 m
   sump pit, not the Mark I light bulb. That is as documented ([A]); the
   drawing makes the simplification visible. Its volume is right by
   construction.
2. **The drywell floor sits at the torus top** (7.683 m), so the downcomer
   entry 2.0 m above it is 7.65 m above the exits. This is an assumed datum
   ([A]); only the downcomer's static head depends on it.
3. **The quenchers sit at the torus bottom (0 m)** in the default Unit 1
   and in Units 2/3, but at the downcomer-exit height in the gh:#1097
   configuration. That matches the configs and the issue ("quenchers at
   0 m [A]"); the two choices put the relief steam under 3.24 m and 1.21 m
   of water respectively.
4. **Exit submergence:** Unit 1 1.209 m (pool 3.237 − exit 2.028), Units 2/3
   1.209 m (4.208 − 2.999). Both are the same `vent_submergence_pa` head, as
   built.
5. **The building is a prism** whose datum (floor at 0 m, pit to −1.5 m) is
   not tied to the plant's. Its gas sees no hydrostatics that matter, so only
   the openings' and flange's heights in it are cosmetic. Units 2/3's
   58.75 m height is the configured `46 + 10.2 + 2.55` m.
6. **The torus outline** is the 64-piece circular section
   (`torus_table`), widest at its centreline. The 11 and 14 layers fill the
   pool to its surface.

**Not checked:**
- whether the real plant's elevations match the assumed datums (items 2
  and 5);
- the vent's actual elevation in the plant (drawn at the WW gas port,
  torus top − 0.5 m);
- anything below the image's resolution (the 0.0005 m² flange area is
  drawn as a line).

## gh:#1108: the vent window against the torus pool's layer count (2026-11-02)

**Methodology.** Open-loop case B
(`tests/containment_network_vs_containment.rs`,
`the_first_hours_of_unit_1_against_the_old_containment`), to 30 h, with
`FUKUSHIMA_NETWORK_LAYERS` = 11 (the derived default), 22, 44 and 88. HEAD
5c29fc691, release, Rust 1.99.1, a 4-core container running four cases at
once. Traced reruns (`FUKUSHIMA_NETWORK_TRACE=23.7,25`, 1 s steps) measure
two things that do not depend on timing: the blowdown time (vent opening
until p_ww < 200 kPa) and the vented mass from 23.7 to 25 h. I chose those
two measures and wrote the predictions on the issue before the 44- and
88-layer results existed.

**Results.**

| n | Δp_ww 24 h | 24.5 h | 25 h | 26 h | 30 h | blowdown net / old | vented 23.7–25 h |
|---|---|---|---|---|---|---|---|
| 11 | −17.94 % | −3.56 % | +3.67 % | +1.91 % | +0.36 % | 4.68 / 9.78 min | 44.86 t |
| 22 | −10.41 % | +2.30 % | +6.41 % | +2.01 % | +0.35 % | 5.08 / 9.78 min | 48.41 t |
| 44 | −6.96 % | +4.63 % | +7.35 % | +1.97 % | +0.34 % | 6.27 / 9.78 min | 49.78 t |
| 88 | −5.95 % | +5.11 % | +7.41 % | +1.94 % | +0.35 % | not run (refinement stopped at the maintainer's direction) | not run |

| Prediction | Outcome |
|---|---|
| 1. n = 11 and 22 reproduce their old values | held, exactly |
| 2. n = 44 at −6.6 ± 1.5 % | held (−6.96 %) |
| 3. n = 88 at −4.8 ± 1.5 % | held, at the edge (−5.95 %) |
| 4. top layer at 24 h rises toward 391 K | **missed**: about 388 K at vent opening at every n |
| 5. 24.5–26 h excess fixed within ±1 point | **missed**: grows with n, settling by n = 44–88 |

**Interpretation.**
- The 24 h point and the vented mass converge with n: the steps are 7.53,
  3.45 and 1.01 points, and 3.55 and 1.37 t. The 24 h difference converges
  to about **−5.5 %, not zero**. Layer resolution explains the 11-to-88
  movement, not the residual.
- The two models open the vent together and track each other for about
  2 min. Then the network reaches 200 kPa about twice as fast, while its
  wetwell gas cools by expansion from 386 to 353 K, about 35–40 K below the
  pool surface.
- The suspect is the gas-side heat input during a fast blowdown. This is not
  attributed: no ablation has been run.
- The blowdown time is **not converged at n = 44** (4.68, 5.08, 6.27 min).
  The n = 88 trace was stopped: the maintainer called the refinement enough.
- Quote n ≥ 44 for anything in the vent window, and the 24 h point as a
  phase-sensitive instrument only.

**Attribution runs** (old-model ablations, 44 layers, traced to 26 h):

| Old-model ablation | p_ww at 23.5 h, net / old | top layer at vent | blowdown to 200 kPa, net / old | Δp_ww 24 h |
|---|---|---|---|---|
| none | 856 / 814 kPa | 387.9 K | 6.27 / 9.78 min | −6.96 % |
| `ww_shell` | 1004 / 950 kPa | 399.1 K | 33.13 / 32.83 min | −4.38 % |
| `ww_atmosphere` | 1005 / 963 kPa | 397.9 K | 30.88 / 31.32 min | −7.50 % |

Both ablations **confound the comparison**. They change the old plant from
the start, so the wetwell reaches 1.0 MPa before the vent, and the 200 kPa
crossing falls in the flashing regime: the pool surface sits above
T_sat(200 kPa) ≈ 393 K. In that regime the two models agree within 0.5 min.

In the base case the crossing is set by gas expansion, and there the network
is about 2× faster. The residual (≈ −5.5 % at 24 h) therefore lives in how
much heat reaches the wetwell gas during a fast blowdown. It is **not
attributed to one mechanism**; that would need an ablation acting only after
the vent opens.

The `ww_shell` prediction (old blowdown shortens to 6–8 min) **missed**. The
`ww_atmosphere` one (it lengthens) held in direction, but neither result can
be read as an attribution.

## gh:#1120: wall condensation with every non-condensable's own properties (2026-11-02)

**Method.** The network's two wall surfaces (drywell structures, torus
shell) and the aerosol step's drywell wall coefficient called tampines'
three-gas `structure_surface`, with every non-condensable other than H2
counted as N2. They now call `structure_surface_mixture` with the volume's
own six non-condensables (tampines gh:#1114: viscosity by Chapman–Enskog
on Svehla's parameters, conductivity by Eucken on the NASA-7 `c_v`, steam
diffusivity as MELCOR's Eq 6-1 scaled by the Chapman–Enskog ratio to N2).
The old path is the ablation `NetworkMechanisms::without_ncg_wall_properties()`.

**Predictions, written before any run:**

1. The ablation reproduces the previous code bit for bit (same call, same
   arguments).
2. With only N2 and H2 present, default and ablation agree to round-off
   (relative ≤ 1e-12 on `h` and the flux). The only difference is N2 read
   directly instead of as `total − H2`.
3. With CO2 replacing half the N2 at a condensing state, the flux falls by
   5–20 %, because CO2's steam diffusivity is 0.69 of N2's and enters as
   about `D^(2/3)`. O2 replacing half the N2 moves it by under 2 %.
4. In a Unit 1 run, nothing moves before the cavity makes CO and CO2 or
   burns make O2-poor gas. After that the drywell wall condenses a few
   per cent less, and `p_dw` moves by under 0.5 %. Not measured here: no
   network regression runs until the model is checked (gh:#1110,
   maintainer).

**Results (2026-11-02, release; `the_walls_see_each_non_condensable_with_its_own_properties`):**

At 400 K, 0.4 MPa, half steam, a 350 K wall 10 m high:

| Prediction | Measured | Verdict |
|---|---|---|
| 1. The ablation is bit-identical | Same call, same arguments, so bit-identical by construction. CO2 counted as N2 under the ablation equals pure N2 exactly (`assert_eq!`) | held |
| 2. N2 + H2: default = ablation to ≤ 1e-12 | `h` 12.32387445 and flux 4.740600927e-3 kg/(m² s) on both, identical to the printed 10 digits | held |
| 3. Half CO2: flux −20 to −5 % | **+2.53 %** | **MISSED** |
| 3. Half O2: under 2 % | **+2.65 %** | **MISSED** |
| 4. The Unit 1 run | not run (no network regression until the model is checked, gh:#1110) | open |

**Why prediction 3 missed:**
- The prediction used only the diffusivity, through `h_D ∝ D^(2/3)` in the turbulent range.
- It left out the Grashof number. While condensing, Gr is set by the density difference between the gas at the wall (saturated at the wall temperature, so richer in non-condensable) and the bulk.
- A heavier non-condensable (CO2 at 44, O2 at 32 g/mol, against N2 at 28) raises that difference. The convection then more than offsets CO2's lower D.
- That is the likely cause, not a decomposition: the term-by-term split was not measured.

The test asserts:
- the N2 + H2 round-off identity;
- the ablation's bit-identity;
- that both gases move the flux by under 10 % and in the positive direction, as measured.

It does not assert the predicted bands.
