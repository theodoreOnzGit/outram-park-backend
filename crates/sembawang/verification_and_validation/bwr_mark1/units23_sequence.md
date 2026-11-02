# Fukushima Daiichi Units 2 and 3: accident sequences, RCIC and HPCI (gh:#1109)

> Research, education and V&V only. AI draft, unreviewed. 2026-11-02.
> Never for operation, licensing, safety decisions or emergency response.

## What was built

- `src/bwr_mark1/units23.rs`: `Unit` (One, Two, Three), each unit's
  recorded sequence (`UNIT2_SEQUENCE`, `UNIT3_SEQUENCE`) and the plant
  readings they are checked against (`OBSERVATIONS`).
- `src/bwr_mark1/turbine_pump.rs`: RCIC and HPCI as governed,
  steam-turbine-driven pumps (shaft balance on IF97's isentropic drop),
  with L8 trip, the operators' throttled mode and carry-over.
- `Plant::for_unit(unit, mode, mechanisms)`: `Params::unit2()`/`unit3()`
  with `NetworkConfig::unit2()`/`unit3()` (or the lumped containment under
  `without_network_containment`), the unit's sequence under
  `Mode::Historical`.
- The drywell spray (`containment::spray_condensation`, both containment
  models; `Mechanisms::containment_spray`, default on, ablation
  `without_containment_spray`).
- The turbines' exhaust joins the relief valves' stream into the torus
  pool (no new snapshot field; Unit 1 sends nothing there, so its runs are
  unchanged).

## The source of every timing

One open document held in the repository: USNRC HRTD, *GE BWR/4
Advanced Technology*, ch. 7.1 "Fukushima Dai-ichi Event" (Rev 0114),
**Table 7.1-2 "Event Chronology"**, pp. 7.1-18 to 7.1-23
(`crates/kovan-literature/reactor-literature/theodore-open-corpus/nrc/nrc-hrtd-ge-bwr4-atm-ch7.1-fukushima.pdf`,
PDF pages 22–27, read locally with `pdftotext -layout`). Figures 7.1-9a to
7.1-9j draw the same chronology (checked by rendering 7.1-9a); Figure
7.1-5a "BWR 4 RCIC" gives the flow path (steam supply, trip-throttle and
control valves, turbine exhaust to the pool, pump suction from the
condensate storage tank, flow controller, test line to the tank) and no
capacities. Times are the table's "TIME ZERO PLUS" column (h:mm after the
14:46 JST scram).

### Unit 2 (inputs; "recorded" rows are never acted on)

| t (h:mm) | JST | row | model action | page |
|---|---|---|---|---|
| 00:06 | 14:52 | Unit 2 RCIC in service | RCIC start, injecting | 7.1-18 |
| 00:19 | 15:05 | Units 2 and 3 RCIC in service, cycled for level | recorded | 7.1-18 |
| 00:42 | 15:28 | Unit 2 RCIC trip on high RPV water level | **recorded: the model trips at L8 itself** | 7.1-18 |
| 00:53 | 15:39 | Unit 2 RCIC manually restarted | RCIC start, injecting | 7.1-18 |
| 00:55 | 15:41 | loss of all AC and DC power (Units 1, 2, 4) | DC lost: SRVs at spring set points; RCIC has no level control or trip | 7.1-18 |
| 44:14 | 11:00 (13 Mar) | Unit 2 aligned for containment venting | vent ordered (the disk at 0.528 MPa abs must still burst) | 7.1-21 |
| 70:39 | 13:25 (14 Mar) | Unit 2 RCIC lost due to pump failure | RCIC lost | 7.1-22 |
| 75:14 | 18:00 | car batteries used to open 1 SRV | one SRV held open | 7.1-22 |
| 77:08 | 19:54 | start of seawater injection via fire pumper | injection on (fire-engine curve, [A]) | 7.1-22 |
| 82:00 | — | end of run | [A] run length | — |

### Unit 3

| t (h:mm) | JST | row | model action | page |
|---|---|---|---|---|
| 00:19 | 15:05 | Unit 3 RCIC in service | RCIC start, injecting | 7.1-18 |
| 00:39 | 15:25 | Unit 3 RCIC trips on high RPV level | **recorded: the model trips at L8 itself** | 7.1-18 |
| 00:55 | 15:41 | station blackout, Unit 3 keeps DC | recorded (nothing modelled runs on AC) | 7.1-18 |
| 01:17 | 16:03 | Unit 3 RCIC started up for injection (157 in > TAF) | RCIC start, **throttled [A]** (no high-level trip in the next 19.5 h, so level control is inferred) | 7.1-19 |
| 20:48 | 11:36 (12 Mar) | Unit 3 RCIC lost and could not be restarted | RCIC lost | 7.1-19 |
| 21:47 | 12:35 | HPCI auto start at 120 in > TAF; operators controlled injection to prevent hi level trip | HPCI start, throttled | 7.1-20 |
| 35:56 | 02:42 (13 Mar) | HPCI tripped (battery depletion); cannot operate SRVs | HPCI lost; DC lost | 7.1-20 |
| 40:53 | 07:39 | containment sprays using a fire pumper | spray on (fire-engine curve against the drywell, [A]) | 7.1-20 |
| 41:55 | 08:41 | maximum containment pressure 78 psig; venting commenced | vent ordered | 7.1-20 |
| 42:22 | 09:08 | Unit 3 SRV opened on automobile batteries | one SRV held open | 7.1-21 |
| 42:39 | 09:25 | start of freshwater injection | injection on; **spray off [A]** (the pumper moved) | 7.1-21 |
| 46:26 | 13:12 | start of seawater injection | injection (continues) | 7.1-21 |
| 59:14 | 02:00 (14 Mar) | vent valve(s) had failed closed | vent closed **[A]** (closed by then; taken at this time) | 7.1-21 |
| 64:14 | 07:00 | "just prior to commencing venting" | vent ordered **[A]** (the time) | 7.1-21 |
| 68:15 | 11:01 | reactor building explodes | recorded | 7.1-21 |
| 70:00 | — | end of run | [A] run length | — |

**A disagreement inside the source.** Rows 20:48 (11:36), 21:22 (12:10)
and 21:47 (12:35) are 2 min short of their Japan time (14:46 + 20:48 is
11:34). Every other Unit 2 and 3 row agrees to the minute (checked by
`units23::tests::the_sequences_are_ordered_cited_and_consistent_with_their_clock_times`).
The offset column is used; recorded, not resolved.

**Not found in the table:** when Unit 2's RCIC suction moved from the tank
to the torus (26:44 says it was then on the pool); further SRV openings
(BSAF's "ADS, 6 SRVs" for Unit 3 at about 42 h is a code inference and a
check, never an input); when Unit 2's SRV closed again (80:04 implies
about 79:54; no closure is applied).

**Not in the model (stated):** torus suction (gh:#1121; `tampines` has no pool
sink; the pump takes tank water, so the torus pool gains the water the
vessel boils off: about 1400 t over Unit 2's first 26 h at 15 kg/s); Units
1 and 2's torus cooling and spray (00:39 to 00:55, on AC); the turbines'
speed dynamics; Unit 3's MOX (gh:#1122); the Unit 3 → 4 hydrogen path
(gh:#1123); Unit 2's blow-out panel.

## The pump's inputs (`params.rs`, tag and source)

| input | value | tag | source |
|---|---|---|---|
| `rcic_rated_kg_s` | U2 26.34 kg/s (95 m³/h); U3 26.94 (97 t/h) | [U] ([S]) | U2: TEPCO press material 2014-08-06 (its 30 m³/h, a fit to measured pressure, **not used**); U3: KTH EMUG 2012 |
| `hpci_rated_kg_s` | 268.1 kg/s (965 t/h) | [U] ([S]) | KTH EMUG 2012 (U3), PSI EMUG 2013 (U2) |
| `pump_efficiency`, `turbine_efficiency` | 0.7, 0.5 | [A] | not in a held source |
| `turbine_min_pa` | 0.5 MPa abs | [A] | low-steam-pressure isolation, not in a held source |
| `level_l2_above_taf_m` | 3.048 m (120 in) | [P] | HRTD Table 7.1-2 p. 7.1-20, HPCI auto start at 120 in > TAF |
| `level_l8_above_taf_m` | 5.0 m | [A] | 0.6 m above the borrowed normal level |
| `msl_nozzle_above_taf_m`, `carryover_band_m` | 6.0 m, 0.5 m | [A] | above L8; not adjusted to any level |

[S] values came from a WebSearch excerpt (2026-11-02); the primaries were
not opened.

## Methodology of the check

**The HRTD readings are the check, never an input.** The comparison is
the drywell ("containment") pressure, the vessel pressure and the level at
each time Table 7.1-2 reports one (`units23::OBSERVATIONS`, 33 rows),
against the model at the same time. "Containment pressure" in the table
is compared with the model's drywell pressure. No input was set from
these readings, and none was changed after seeing them.

Runs: `tests/units_2_and_3_sequence.rs`. The full sequences (82 h and
70 h) run on the **lumped containment** (`without_network_containment`),
which costs seconds; the network default is run over each unit's first
readings only (Unit 2 to 9 h, Unit 3 to 22 h: 44 min of wall time; its
geometry is not yet checked by a human, gh:#1110). The lumped model's
comparison is therefore an ablation's, said so wherever it is quoted.

## Predictions (written before the first run, 2026-11-02)

Hand estimates from the inputs above, not from the readings.

1. **RCIC's first L8 trip.** Unit 2's RCIC (26.3 kg/s at 20 °C) against
   about 60 MW of decay heat at 6 min: its subcooling takes about 33 MW,
   so the SRVs pass about 18 kg/s and the vessel gains about 7 kg/s;
   0.6 m to L8 over about 15 t/m is about 9 t, about 21 min. **Unit 2's
   trip at about 00:27 (range 00:15–00:45); HRTD 00:42.** Unit 3, started
   at 00:19, **about 00:40 (range 00:30–00:60); HRTD 00:39.**
2. **Unit 2 after DC is lost** runs uncontrolled; the swollen level rises to
   the steam line's nozzle (6.0 m above TAF) and holds there by carry-over.
   **The collapsed level at 07:04 between 4.5 and 6.0 m above TAF; HRTD
   reads 134 in = 3.40 m. Expected to MISS high by 1–2.5 m** (the nozzle
   is [A], and the real mechanism is not established).
3. **Unit 2's containment** has the whole decay heat in a pool of 2980 t
   with no torus-room flooding and no stratification in the lumped model
   (both carry BSAF's explanation of the low measured pressure). About
   2 TJ by 26 h against 12.5 GJ/K: the pool saturates within about 15 h.
   **Drywell at 08:39 between 0.12 and 0.20 MPa abs (HRTD 5 psig = 0.136);
   at 26:44 between 0.40 and 0.80 MPa abs (HRTD 30 psig = 0.31): MISS high
   expected.** With the vent ordered at 44:14 and the drywell above the
   disk's 0.528 MPa, **the model vents, where HRTD records Unit 2 below the
   disk throughout (78:14): MISS expected.**
4. **Unit 3's HPCI (throttled)** draws about 8.6 kg/s of steam at 7 MPa
   (0.032 kg per kg pumped) against about 13 MW of decay heat at 22 h, so
   the vessel cools down: **vessel at 26:14 between 1.0 and 4.0 MPa abs
   (HRTD 421 psig = 3.0 MPa).**
5. **After HPCI and DC are lost (35:56)** the SRVs act at their spring set
   points (Unit 1's, borrowed): **vessel at 38:14 between 7.5 and 8.4 MPa
   abs (HRTD 1070 psig = 7.48 MPa); level below TAF by 38:14.**
6. **Unit 3's containment at 21:22 between 0.30 and 0.70 MPa abs (HRTD 42
   psig = 0.39)**; the vent opens when ordered at 41:55 only if the wetwell
   is above 0.528 MPa then.

## Results (2026-11-02, lumped containment, data as committed)

Run: `cargo test --release -j 1 -p sembawang --test units_2_and_3_sequence -- --nocapture`.
Deterministic (no randomness); one run each. **All on the lumped
containment** (`without_network_containment`), an ablation: the network
default ran only the short window below.

### The sequence plays out

Every input row acts at its time (asserted by
`both_sequences_play_out_against_the_hrtd_readings`): Unit 2's RCIC from
00:06, restarted 00:53, uncontrolled after DC is lost at 00:55, lost at
70:39; its SRV held from 75:14; seawater from 77:08. Unit 3's RCIC from
00:19, throttled from 01:17, lost 20:48; HPCI throttled 21:47 to 35:56,
DC lost then; spray 40:53 to 42:39; vent ordered 41:55, closed 59:14,
reordered 64:14; injection from 42:39.

### Against the predictions

| # | quantity | predicted | model | HRTD | verdict |
|---|---|---|---|---|---|
| 1 | Unit 2 RCIC's L8 trip | 00:27 (00:15–00:45) | **00:36.3** | 00:42 | prediction in range; model 6 min early |
| 1 | Unit 3 RCIC's L8 trip | 00:40 (00:30–01:00) | **01:00.6** | 00:39 | **prediction MISSED** (0.6 min outside); model 22 min late |
| 2 | Unit 2 level at 07:04, m above TAF | 4.5–6.0 (MISS high expected) | **6.01** | 3.40 | prediction just missed (0.01 m above); **model misses HRTD by +2.6 m**, as expected |
| 3 | Unit 2 drywell at 08:39, MPa abs | 0.12–0.20 | **0.128** | 0.136 | in range; model −6 % |
| 3 | Unit 2 drywell at 26:44, MPa abs | 0.40–0.80 (MISS high expected) | **0.328** | 0.308 | **prediction MISSED low**; model +6 % on HRTD |
| 3 | Unit 2 vents at 44:14 | yes (MISS expected) | **yes, 44.25 h** | never (78:14: below the disk) | model misses HRTD, as predicted |
| 4 | Unit 3 RPV at 26:14, MPa abs | 1.0–4.0 | **2.43** | 3.00 | in range; model −19 % |
| 5 | Unit 3 RPV at 38:14, MPa abs | 7.5–8.4 | **3.67** | 7.48 | **prediction MISSED**; model −51 % |
| 5 | Unit 3 level at 38:14 | below TAF | **+6.11 m** | −2.01 m | **prediction MISSED** |
| 6 | Unit 3 drywell at 21:22, MPa abs | 0.30–0.70 | **0.240** | 0.391 | **prediction MISSED low**; model −39 % |
| 6 | Unit 3 vent opens when ordered (41:55) | if the wetwell > 0.528 | **flowing 41.93 h** (wetwell 0.65) | venting commenced 41:55 | agrees |

### Every HRTD reading in the runs

Containment = the model's drywell. Pressures MPa abs (HRTD psig + 0.1013).

| unit | t | reading | HRTD | model | model − HRTD |
|---|---|---|---|---|---|
| 2 | 07:04 | level above TAF | 3.40 m | 6.01 m | +2.61 m |
| 2 | 08:39 | RPV | 6.403 | 8.017 | +25 % |
| 2 | 08:39 | containment | 0.136 | 0.128 | −6 % |
| 2 | 26:44 | containment | 0.308 | 0.328 | +6 % |
| 2 | 42:09 | containment | 0.370 | 0.638 | +72 % |
| 2 | 68:15 | containment | 0.460 | 0.108 | −77 % (model vented at 44:14) |
| 2 | 69:44 | containment | 0.487 | 0.107 | −78 % (vented) |
| 2 | 70:39 | drywell | 0.460 | 0.107 | −77 % (vented) |
| 2 | 73:48 | RPV | 7.099 | 8.026 | +13 % |
| 2 | 80:04 | containment | 0.536 | 0.113 | −79 % (vented) |
| 2 | 81:16 | containment | 0.756 | 0.111 | −85 % (vented) |
| 3 | 01:17 | level above TAF | 3.99 m | 3.69 m | −0.30 m |
| 3 | 21:22 | containment | 0.391 | 0.240 | −39 % |
| 3 | 21:47 | level above TAF | 3.05 m | 2.42 m | −0.63 m |
| 3 | 26:14 | RPV | 3.004 | 2.434 | −19 % |
| 3 | 35:56 | RPV | 4.100 | 0.971 | −76 % |
| 3 | 37:29 | level above TAF | 0.00 m | 5.66 m | +5.66 m |
| 3 | 38:14 | RPV | 7.479 | 3.666 | −51 % |
| 3 | 38:14 | level above TAF | −2.01 m | 6.11 m | +8.12 m |
| 3 | 38:14 | containment | 0.363 | 0.535 | +47 % |
| 3 | 40:59 | level above TAF | −3.00 m | 6.13 m | +9.13 m |
| 3 | 40:59 | RPV | 7.410 | 8.165 | +10 % |
| 3 | 40:59 | containment | 0.460 | 0.595 | +29 % |
| 3 | 40:59 | torus | 0.439 | 0.599 | +36 % |
| 3 | 41:55 | containment (the maximum) | 0.639 | 0.650 | +2 % |
| 3 | 42:24 | containment | 0.632 | 0.546 | −14 % |
| 3 | 42:38 | containment | 0.536 | 0.543 | +1 % |
| 3 | 42:39 | RPV | 0.453 | 2.675 | +490 % |
| 3 | 46:14 | RPV | 0.294 | 1.034 | +252 % |
| 3 | 59:14 | containment | 0.260 | 0.111 | −57 % |
| 3 | 60:54 | containment | 0.329 | 0.114 | −65 % |
| 3 | 62:34 | containment | 0.453 | 0.122 | −73 % |
| 3 | 68:07 | containment | 0.515 | 0.114 | −78 % |

Peak drywell: Unit 2 0.704 MPa (just before its vent at 44.25 h), Unit 3
0.650 MPa (at 41.9 h). Peak cladding above 1500 K from 76.3 h (Unit 2)
and 43.6 h (Unit 3); hydrogen at the end of the runs 347 kg and 741 kg.
(TEPCO's MAAP core-damage times in the table, 76:34 and 43:54, are
analyses, not observations, and are **not** used as checks.)

**The spray's share** (`without_containment_spray`, Unit 3): the drywell
at 41:55 is 0.6501 MPa with the droplets' exchange and 0.6570 without
(−7 kPa, about 1 %). The fire-engine curve ([A]) against a 0.6 MPa drywell
gives only about 2.7 kg/s.

### Interpretation (what the disagreements say)

- **The first 26 h of Unit 2's containment agree to ±6 %**, which BSAF's
  narrative says lumped codes do *not* manage (they over-predict without
  torus-room flooding and stratification). The model gets there by
  another route that it should not be credited for: it has **no torus
  suction**, so the tank water RCIC injects (limited by carry-over to
  roughly 1–12 kg/s in the trace, on average about the boil-off) ends up
  in the pool as the steam it becomes: roughly 1400 t by 26 h by the
  estimate above (not tallied by the run), a heat capacity the real pool
  did not gain. That is the leading suspect, untested; the comparison is
  not evidence the model is right.
- **Unit 2 vents at 44:14 in the model and never in the plant.** By 42:09
  the model is 72 % high (0.638 against 0.370), above the 0.528 MPa disk,
  so the ordered vent opens; everything after it in Unit 2 is a different
  accident from the one recorded. The miss is the pool's heat-up, not the
  vent logic.
- **Unit 2's level holds at the steam line's nozzle** (6.0 m above TAF,
  [A]), 2.6 m above the reading: the carry-over hypothesis, with its
  assumed nozzle, does hold the level steady for 70 h as observed, but not
  where it was observed.
- **Unit 3's throttled HPCI cools the vessel too far and keeps it full.**
  At 35:56 the model is at 0.97 MPa against 4.10, and at 38:14 its level
  is 6.1 m above TAF where the plant had already uncovered the core. The
  throttled mode (turbine at full rated flow, the excess round the test
  line) with a proportional level band holds the level near L8 to the
  end, so when HPCI stops there is far more water to boil off. The
  table's "operators controlled injection to prevent hi level trip. HPCI
  causing RPV cooldown" is reproduced in kind (the cooldown) but not in
  degree. Neither the level band nor the turbine's flow at low pressure
  was adjusted to the readings.
- **The SRV-held depressurisations are faster than the plant's**
  (Unit 3 at 42:39: 2.7 MPa against 0.45; Unit 2's at 75:14 reaches 0.38
  MPa in an hour against 0.73 at 76:17): one held valve of Unit 1's
  throat ([A]) against vessels this model keeps fuller.
- **After its vent closes (59:14), Unit 3's model containment stays near
  0.11 MPa** while the plant repressurised to 0.52 by 68:07: the model
  vented its nitrogen and its pool does not reheat under the fire
  engine's cold injection. Not investigated further here.

**Which numbers to quote:** the table above, as the lumped ablation's,
with the torus-suction caveat on every Unit 2 containment number. None is
a validation; the network default has not been run over these windows
(cost, and its geometry awaits the human check of gh:#1110).

### Network default, over each unit's first readings

Run on a clean worktree of commit 7b3be3985 (committed `tampines`; the
working tree held another agent's uncommitted `cv_network` edits, gh:#1119,
so no network result here depends on them):
`cargo test --release -j 1 -p sembawang --test units_2_and_3_sequence -- --ignored --nocapture the_network_over`.
Wall time: Unit 2 to 9 h 456 s, Unit 3 to 22 h 2195 s (the network's cost
grows as the pool heats).

| unit | t | reading | HRTD | network | lumped |
|---|---|---|---|---|---|
| 2 | 07:04 | level above TAF | 3.40 m | 6.00 m | 6.01 m |
| 2 | 08:39 | RPV, MPa | 6.403 | 8.010 | 8.017 |
| 2 | 08:39 | containment, MPa | 0.136 | **0.133 (−2 %)** | 0.128 (−6 %) |
| 3 | 01:17 | level above TAF | 3.99 m | 3.66 m | 3.69 m |
| 3 | 21:22 | containment, MPa | 0.391 | **0.226 (−42 %)** | 0.240 (−39 %) |
| 3 | 21:47 | level above TAF | 3.05 m | 2.59 m | 2.42 m |

Drywell, network / lumped, MPa: Unit 2 at 4 h 0.1148 / 0.1122, at 9 h
0.1341 / 0.1296; Unit 3 at 22 h 0.2426 / 0.2631. Pools within 0.15 K of
each other throughout. **The two containments agree to 2–8 % over these
windows**, so the lumped comparison above is a fair stand-in for the
network's up to these times. It is not evidence beyond them, and the
network's geometry still awaits the human check (gh:#1110). The short
network smoke test (`the_network_runs_unit2_for_twenty_minutes`, 20 min of
plant time in 9.4 s) passes. Neither the network's nor the lumped
model's Unit 3 at 21:22 comes near the reading: both under-predict it by
about 40 %.
