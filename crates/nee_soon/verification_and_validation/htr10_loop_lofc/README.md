# HTR-10 primary loop and LOFC ATWS with a lumped core stand-in (gh:#1151 part A)

> Research, education and V&V only. Not for licensing, operation or safety
> decisions. AI draft (Claude), unreviewed by a human.

> **Moved 2026-11-03** from `crates/tampines/verification_and_validation/htr10_loop_lofc/` (maintainer: coupled kinetics and TH live in `nee_soon`). Loop and stand-in code stay in `crates/tampines/src/htr10_primary_loop/`; the kinetics, the coupled transient and these tests are `crates/nee_soon/src/htr10_th_transient/`. The moved tests reproduce every number below (Sec 7).

Code: `crates/tampines/src/htr10_primary_loop/` (loop, stand-in) and `crates/nee_soon/src/htr10_th_transient/` (kinetics, coupling). Inputs:
`docs/reactor-scoping/htr10-th-benchmark.md` (the cited input sheet; source
keys [T1] TECDOC-1694, [J09] Jun 2009, [C09] Chen 2009, [H06] Hu 2006,
[G02] Gao & Shi 2002, [S21] She 2021 are its own) and
`docs/reactor-scoping/htr10-th-data/`.

## 1. Predictions, written before the first LOFC run (2026-11-03)

Written after the code compiled and before any transient was run, from
hand estimates on the stand-in's own derived parameters.

**What the stand-in is.** 5 axial bed zones (fuel zone / fuel-pebble shell /
dummy pebbles each) + an unheated cone, one side-reflector node, the barrel,
the RPV and a 50 degC RCCS; all conductances and capacities derived from
TECDOC-1694 tables. It has **no in-vessel natural circulation, no top or
bottom reflector**, and only conduction-plus-radiation from the bed to the
side reflector (`8 pi k_eff H`, about 200-300 W/K over the bed with Breitbach
`k_eff` 3-6 W/(m K)).

Hand estimates:

- Bed pebble heat capacity about 24,000 pebbles x 1.13e-4 m^3 x 2.7 MJ/(m^3 K)
  = 7-8 MJ/K. Adiabatic heat-up at 3.3 MW: 0.43 K/s.
- Fuel + moderator coefficients -18.3 pcm/K: -1 $ (726 pcm) needs about 40 K,
  i.e. ~90 s at full power; the power falls as reactivity goes negative, so
  the fall is slower than that.
- Decay heat (ANS draft, saturated at 3.3 MW): about 2 % (66 kW) at 1000 s,
  1.5 % (50 kW) at 3000 s.
- Heat leaving the bed after the trip: bed-to-reflector 200-300 W/K x
  (bed mean ~430 degC minus reflector ~230 degC) = 40-60 kW. **Comparable to
  the decay heat**, so the bed barely cools.

| Quantity | Prediction (stand-in) | Measured / reference |
|---|---|---|
| Fission power to 1 % of initial | 250-450 s | 330 s ([C09] Sec 5, [C14] Tab 4) |
| Minimum total reactivity | -400 to -700 pcm, at 300-800 s | THERMIX about -0.55 $ = -400 pcm ([C09] Fig 6, calculated) |
| Return of power (re-criticality) | **later than measured: after 6000 s, quite possibly not within 3 h** | power back above 1 % at ~4210 s (digitised [C09] Fig 6); "critical again" 2520 s [H06]; 4075 s [T1] |
| First peak | if it returns within 3 h, lower and later than measured | 24.7 % at 4400 s ([C09]) |
| Long-term power | not reached | ~180 kW within 3 h ([C09]) |

**Why the prediction is "late".** Measured long-term power of ~180-200 kW
means the real core sheds ~200 kW by 3 h; the top reflector alone takes
~240 kW on average (+170 K on ~13 t of graphite in 3 h, [C09] Fig 8(c)) and
it is reached by in-vessel natural circulation ([C09] Sec 5). The stand-in
has neither, so its heat removal is a factor of 3-5 short and its
re-criticality must be late. The short-term shutdown (the 1 % time) depends
on heat capacity and coefficients, which the stand-in does carry, so that
comparison is meaningful; the long-term one is not, and is reported only to
show the size of what part B must supply.

## 2. Methodology

**What is computed.** The HTR-10 helium-circulator-trip ATWS test of
15 Oct 2003: circulator switched off at t = 0, blower baffle closed and
secondary isolated at 12 s, no rod moved ([H06] Tab 1, [C09] Sec 4).
Coupled per step (`transient.rs`): `teh-o-prke` PRKE (backward Euler) +
ANS-1978-draft decay heat + Xe-135 with feedback from the core's
start-of-step temperatures; the loop momentum balance; the core's
backward-Euler solve of 19 nodes. 0.1 s steps to 1000 s, 0.5 s after.

**Inputs (D4: as-run).** 3315 kW, RPV inlet 212 degC, 2476 kPa ([J09] Tab 2),
1.457 kg/s (Derived, sheet Sec 6). Kinetics: beta 0.726 %, Lambda 1.68e-3 s,
alpha -2.13 / -16.2 / +7.71 pcm/K ([T1], [J09] Tab 3); Keepin U-235 six
groups (**named assumption**, none published); Xe ON (one-group burnout,
`kinetics.rs` docs). Bypass 87/1/2/10 ([T1], **assumption**, D5). Coastdown:
the **measured** blower speed, [C09] Fig 4 digitised. Core: the lumped
stand-in (`lumped_core.rs`; its data table and named assumptions A1-A8 are
in the module docs; layout drawn in `crates/tampines/verification_and_validation/htr10_primary_loop/lumped_core_layout.svg`).

**Reference.** The measured relative fission power, [C09] Fig 4 (0-500 s)
and Fig 6 (0-10800 s), digitised (Sec 4). Its own markers, read from the
digitised trace: 1 % at ~330 s ([C09] text; trace 0.0139 at 300 s,
0.0056 at 400 s); power back above 1 % at ~4210 s; first peak 0.246 at
4395 s; second 0.097 at ~5610 s; plateau 0.054 (179 kW).

**Pass criterion (the one gate, fixed with the predictions).** Time to 1 %
within 330 +- 120 s (`lofc_atws_base_case_against_the_measured_trace`).
Nothing else is gated: Sec 1 explains why the long-term comparison cannot
pass with this stand-in.

## 3. Results (2026-11-03, `cargo test --release -p tampines --lib htr10_primary_loop`)

### 3.1 Initial steady state (verification)

| Quantity | Value | Check |
|---|---|---|
| Energy balance `P = Q_coolant + Q_RCCS` | residual -2e-15 (forward), -2e-14 (reversed 0.3 kg/s) | exact, `lumped_core_steady_state_balances_energy` |
| Core outlet (hot nozzle) | **637.96 degC** against measured 650 | -12 K: 90.9 kW goes to the RCCS while the 1.457 kg/s was derived assuming none |
| Power-weighted fuel / moderator / reflector | 546.1 / 508.6 / 214.8 degC | THERMIX's calculated average fuel temperature at t = 0 is ~545 degC ([C09] Fig 5, a code result, not a measurement) |
| Bed / reflector / barrel+RPV capacity | 8.59 / 67.4 / 56.7 MJ/K | |
| Bed -> reflector conductance | 221 W/K | as predicted (200-300) |
| Core dP at 1.457 kg/s; gravity head; inertance | 1043 Pa; -29.4 Pa; 54.3 1/m | |
| Circulator head at the initial state; temperature rise | 3530 Pa; 0.35 K | SG outlet set to 211.65 degC so the RPV inlet is 212 degC |
| 60 s with no trip | flow 1.457100 kg/s, power 3.315322 MW | steady to 1e-4 (`coupled_initial_state_is_steady`) |
| Equilibrium Xe worth | -2075 pcm (-2.86 $); +4.4 % inventory and -91 pcm 1 h after shutdown | derived, not compared |

### 3.2 Coastdown (`coastdown_flow_follows_speed_then_baffle_closes`)

Loop flow follows the measured speed within 0.3 % up to 12 s (W/W0 0.7748
at speed 0.7775), as [C09] Sec 4 says it should; zero once the baffle
closes. Multiplying the derived inertance by 10 (a bound on the Unknown duct
and SG lengths) moves the flow by under 1 %. By x100 it does not hold: the
time constant becomes ~1 s. Recorded so nobody repeats it.

### 3.3 LOFC ATWS, base case: prediction against result

| Quantity | Predicted (Sec 1) | Model | Measured |
|---|---|---|---|
| Fission power to 1 % | 250-450 s | **257 s** (gate 330 +- 120: **passes**) | ~330 s |
| Minimum total reactivity | -400 to -700 pcm at 300-800 s | **-547 pcm at 942 s** (later than predicted) | THERMIX -0.55 $ (calc.) |
| Power back above 1 % | after 6000 s, possibly not in 3 h | **not in 3 h**; reactivity -10 pcm at 10800 s; a 6 h run: rho = 0 at 10962 s, power back at 15038 s | ~4210 s |
| First peak | lower and later, if any | none in 3 h; 6 h run: **80.8 % at 15129 s** (higher, not lower) | 24.6 % at 4395 s |
| Mean power, last 30 min | not reached | 32 kW (decay only) | ~180 kW |

Short term, relative fission power (model / measured): 10 s 0.979 / 0.989;
20 s 0.847 / 0.905; 50 s 0.430 / 0.496; 100 s 0.147 / 0.204; 200 s 0.024 /
0.047; 300 s 0.0052 / 0.0139. **The model falls faster than the plant
throughout**, by 6 % of initial power at 20 s and a factor of ~2 by 200 s.
Not chased in this change: the candidates are the stand-in's bed capacity
(the as-run pebble count is 7 % below the 0.61 packing of the zone volumes,
assumption A1), all power deposited in the fuel (A2), and the flow after the
baffle closes being exactly zero where [C09] says "almost zero".

Long term the prediction held, and for the predicted reason: the RCCS takes
77-90 kW and the bed sheds only ~40-60 kW to the reflector, about equal to
the decay heat (64 kW at 1000 s, 31 kW at 10800 s), so the bed cools by
only ~55 K in 3 h (fuel 562 -> 507 degC; reflector 215 -> 221 degC) while
xenon adds -187 pcm. The prediction that a late peak would be *lower* was
wrong: the 80.8 % peak of the 6 h run comes from returning from a very deep
subcritical state. There is **no neutron source** in the model (**CORRECTED
2026-11-03:** there is since part B, default ON; the numbers in this
paragraph are the source-free `SRC-OFF` run, which reproduces them), so the
fission power falls with the longest precursor period for hours and the
excursion overshoots before feedback catches it. The measured return comes
at 4210 s from a far shallower state.

**What this establishes.** The loop, the coastdown, the kinetics and the
coupling run and conserve energy; the short-term shutdown is reproduced in
kind and within the gate. **It establishes nothing about re-criticality or
long-term power**: those need the R-Z core with in-vessel natural
circulation and the top/bottom reflectors (part B, after gh:#1150).

### 3.4 Sensitivity cases (`lofc_atws_sensitivity_cases`), one input changed each

| case | to 1 % | min rho | rho = 0 | power back > 1 % | first peak | mean P, last 30 min |
|---|---|---|---|---|---|---|
| BASE | 257 s | -547 pcm @ 942 s | none | none | none | 32 kW |
| BASE-6H (21600 s) | 257 s | -547 pcm @ 942 s | 10962 s | 15038 s | 80.8 % @ 15129 s | 26 kW |
| KIN-LAMBDA-H06 (1.68e-4 s) | 257 s | -544 pcm @ 943 s | none | none | none | 32 kW |
| KIN-DN-TEHO (teh-o-prke U-235 set; ENDF/B-VIII.0 since gh:#1319) | ~~258 s~~ **242 s** | ~~-547 pcm @ 942 s~~ **-518 pcm @ 954 s** | ~~none~~ **10674 s** | none | none | 32 kW |
| KIN-DN-1G (one group) | 195 s | -555 pcm @ 933 s | none | none | none | 32 kW |
| KIN-ALPHA-G02-IC | 295 s | -430 pcm @ 1075 s | none | none | none | 32 kW |
| KIN-ALPHA-G02-EQ | 352 s | -284 pcm @ 1254 s | none | none | none | 32 kW |
| XE-OFF | 258 s | -526 pcm @ 731 s | 8325 s | none | none | 32 kW |
| DH-HALF (decay heat x 0.5) | 261 s | -491 pcm @ 555 s | 6248 s | 8844 s | 59.3 % @ 8952 s | 61 kW |
| BYP-G02 | 258 s | -543 pcm @ 905 s | 10553 s | none | none | 32 kW |
| BYP-S5-CR | 257 s | -547 pcm @ 942 s | none | none | none | 32 kW |
| BYP-NOLEAK | 256 s | -585 pcm @ 1522 s | none | none | none | 32 kW |
| LOFC-SPEC (flow stops at 0) | 247 s | -549 pcm @ 931 s | none | none | none | 32 kW |
| LOFC-J09-FLOW (J09's tuned flow, no baffle) | 278 s | -449 pcm @ 474 s | 4300 s | 6033 s | 60.7 % @ 6142 s | 136 kW |
| LOFC-INERTIAL-49S (`1/(1+t/49 s)`) | 258 s | -547 pcm @ 942 s | none | none | none | 32 kW |
| TR-HU (3000 kW, 215 degC, 2.5 MPa) | 266 s | -504 pcm @ 803 s | 9263 s | none | none | 29 kW |
| TR-SPEC (3 MW, 250 degC, 2.5 MPa) | 267 s | -508 pcm @ 892 s | 10563 s | none | none | 29 kW |

**CORRECTED 2026-11-03 (gh:#1151 part B): the default kinetics changed**
(the Am-Be start-up source is ON, `neutron_source`; part B README Sec 3.5),
so this table was **re-measured** with the moved tests
(`cargo test --release -p nee_soon --lib lofc_atws_sensitivity_cases`). The
table above is the source-free record; it is reproduced exactly by the new
`SRC-OFF` and `BASE-6H-SRC-OFF` rows. With the source, only the rows that
return to power move (the others are identical):

| case | to 1 % | min rho | rho = 0 | power back > 1 % | first peak | mean P, last 30 min |
|---|---|---|---|---|---|---|
| BASE-6H | 257 s | -547 pcm @ 942 s | 10962 s | ~~15038 s~~ **12406 s** | ~~80.8 % @ 15129 s~~ **8.7 % @ 12681 s** | ~~26~~ **28 kW** |
| XE-OFF | 258 s | -526 pcm @ 731 s | 8325 s | ~~none~~ **9652 s** | ~~none~~ **10.2 % @ 9913 s** | ~~32~~ **95 kW** |
| DH-HALF | 261 s | -491 pcm @ 555 s | 6248 s | ~~8844 s~~ **7452 s** | ~~59.3 % @ 8952 s~~ **12.3 % @ 7694 s** | ~~61~~ **48 kW** |
| BYP-G02 | 258 s | -543 pcm @ 905 s | ~~10553~~ **10554 s** | none | none | 32 kW |
| LOFC-J09-FLOW | 278 s | -449 pcm @ 474 s | ~~4300~~ **4301 s** | ~~6033 s~~ **5273 s** | ~~60.7 % @ 6142 s~~ **18.9 % @ 5479 s** | ~~136~~ **103 kW** |
| TR-HU | 266 s | -504 pcm @ 803 s | 9263 s | ~~none~~ **10648 s** | ~~none~~ **5.8 % @ 10800 s** | ~~29~~ **37 kW** |

The source does what part B's Sec 1 predicted: earlier returns and much
smaller overshoots (the 80.8 % "higher, not lower" peak below was the
source-free floor at ~1e-20).

Reading it:

- **Which numbers to quote:** BASE. The cases are not alternatives to
  choose from after the fact (sheet Sec 9.2).
- **Lambda** changes nothing at this time scale (delayed-neutron
  dominated), so the [H06] conflict does not matter for the LOFC.
- **The group set matters through `sum beta_i / lambda_i`**: teh-o-prke's
  set (0.09473 s against Keepin's 0.09449 s) moves nothing; one group with
  the same sum moves the 1 % time by -62 s (the short groups carry the
  early decay). **CHANGED 2026-11-10 (gh:#1319):** teh-o-prke's U-235 set
  is now ENDF/B-VIII.0, 0.08030 s (15 % below Keepin), and KIN-DN-TEHO
  moves: the 1 % time 258 -> 242 s, the minimum 29 pcm shallower, and
  reactivity now returns to zero at 10674 s (power does not return in
  3 h). Re-measured with `cargo test --release -p nee_soon --lib
  lofc_atws_sensitivity_cases -- --nocapture`; BASE (Keepin) unchanged.
  Predicted on gh:#1319: 1 % time 10-40 s earlier (met, -16 s), minimum
  within +-10 pcm (missed, +29 pcm).
- **The coefficients matter most** in the short term (257 -> 352 s with
  G02's equilibrium core); none returns to power in 3 h.
- **Decay heat is the strongest long-term lever** among the cases, as in
  [C14] Tab 4 case 3: halving it brings the return to 8844 s. The
  stand-in's missing heat removal acts the same way.
- **LOFC-J09-FLOW returns earliest (6033 s)** because J09's tuned curve
  keeps forced flow for ~150 s without a baffle closure and so removes heat
  the as-run case never removes. It is a tuned input and stays a case.
- **The bypass split** moves little except NOLEAK, which sends 10 % more flow
  through the bed before the trip: the minimum is deeper and later.

### 3.5 Natural circulation with the baffle left open (`natural_circulation_with_the_baffle_open`)

With the baffle never closing, the flow decays with the coastdown and then
**reverses**: -0.088 kg/s at 600 s with the SG thermal centre level with the
RPV nozzles, -0.084 kg/s with it 5 m above (both elevations hypothetical;
the real one is Unknown). Reversal is the expected sign: the in-RPV gravity
head is -29 Pa (hot gas in and below the bed opposes the downward forced
flow). Preventing a primary-circuit natural circulation is what [C09] Sec 2
says the baffle is for.

### 3.6 The helical-coil SG in the loop (`helical_coil_steam_generator_in_the_loop`, long-tests)

The workspace exchanger (`htr10_illustrative_config`, illustrative UA
4.26e4 W/K) at the as-run hot inlet (637.96 degC, 1.457 kg/s) with
feedwater 1.1555 kg/s (energy balance to 440 degC / 4.0 MPa steam from
104 degC feed, the design steam state; the 30 % state is Unknown) settles to
a helium outlet of **180.64 degC against the measured 212 degC** (-31 K)
after 300 s, and 173.2 degC after the first 20 s of the LOFC (flow stopped,
SG isolated). The illustrative UA is too large for 30 % power. The 300 s
settle took 1553 s wall (0.0125 s substep): this is why the base case uses
[J09]'s measured-boundary method instead.

## 4. Data provenance (digitised for this issue)

All in `docs/reactor-scoping/htr10-th-data/`, each with a citation header,
the method and the read error. Method: the embedded raster figure extracted
at native resolution (`pdfimages -png`, no resampling); frame from its solid
axis lines; axes from the frame and printed tick labels (checked against
gridlines); curve pixels selected by colour, per pixel column; legends
masked; no interpolation. Scripts kept in the session scratchpad, not
committed.

| File | Source | Content | Read error |
|---|---|---|---|
| `lofc-blower-speed-c09-fig4.csv` | [C09] Fig 4 | measured relative blower speed, 0-500 s, 97 points | +-0.01, +-1 s |
| `lofc-power-short-c09-fig4.csv` | [C09] Fig 4 | measured relative fission power, 0-500 s, 189 points | +-0.01, or the half-width |
| `lofc-power-long-c09-fig6.csv` | [C09] Fig 6 | measured relative fission power, 0-10800 s, 543 points | +-0.006, +-20 s |
| `lofc-flow-coastdown-j09-fig6.csv` | [J09] Fig 6 | GAMMA+ input flow (TUNED), 70 points | +-0.01 kg/s |
| `lofc-thermocouples-c09-figs8-11.csv` | [C09] Figs 8-11 | 17 measured TC traces (TR, SR1-12, BR), 180 s bins | the noise band, 0.5-4.5 C (37 C in the BR2/BR4 first bin) |
| `she2021-fig8-outlet-temperature.csv` | [S21] Fig 8 A-G (CC BY) | 122 measured core-outlet markers, PANGU input power at each | +-6.3 C; flags for merged markers and power steps |
| `she2021-fig4a-inlet-flow.csv` | [S21] Fig 4(A) | inlet mass flow, one row per pixel column (2.5 d) | +-0.02 kg/s |
| `she2021-fig4b-inlet-temperature.csv` | [S21] Fig 4(B) | inlet temperature markers | +-2 C |

Checks of the digitisation against the papers' own text: [C09] 24.7 % at
4400 s (read 0.246 at 4395 s); [T1] second peak 0.1 at +1250 s (read 0.097
at +1215 s); [C09] ~180 kW at 3 h (read 179 kW); [C09] TR1&TR4 232 -> 405 C
(read 232.7 -> 405.2); [C09] SR6 minimum ~295 C (read 295-297 C).

**Not digitised:** the TECDOC-1694 and [H06] LOFC power figures (one
digitisation of the measured trace, checked at every point the texts quote,
was judged enough), and every calculated (black) curve.

## 5. Known flaws, carried openly (D5)

1. **The stand-in lacks the heat-removal paths that set re-criticality**
   (in-vessel natural circulation, top and bottom reflectors). Part B.
2. ~~**No neutron source**: subcritical power falls without a floor and the
   return overshoots. The HTR-10 source strength is Unknown here.~~
   **CORRECTED 2026-11-03 (part B):** the start-up source is cited
   (TECDOC-1382 Sec 4.2.1.5: 20 Ci Am-Be, 4.4e7 n/s) and ON by default; its
   presence at the test remains Unknown. See Sec 3.4's re-measurement.
3. Short-term power falls faster than measured (Sec 3.3); cause not found.
4. Bypass split unmeasured; [T1] split assumed (D5).
5. Six-group constants unpublished; Keepin assumed.
6. Xenon worth derived with 2200 m/s cross-section ratios and a cold
   lattice `k_inf`; spectrum effects not carried.
7. Circulator characteristic, rotor inertia, duct and SG lengths and
   elevations are Unknown; the loop carries the derived lower-bound
   inertance and zero external elevation.
8. Helium: Tab 2.8 fits in the core and the loop losses, CoolProp in the
   circulator.
9. The lumped core's helium is quasi-steady (no gas inventory); no
   transport delay in the loop legs.

## 6. Geometry drawing

`crates/tampines/verification_and_validation/htr10_primary_loop/lumped_core_layout.svg` (written by the tampines test `draw_the_lumped_core_layout` from
the assembled core): the five heated zones with their pebble counts, power
fractions and steady temperatures, the cone, and the radial bodies the
solver reads. Checked in it: zone edges 0-36-72-108-144-187 cm, power
fractions 16.7 / 20.7 / 22.2 / 20.7 / 19.6 %, reflector sub-annuli
contiguous from 0.90 to 1.675 m, brick to 1.90 m, barrel 1.91-1.94 m, RPV
2.10-2.18 m. Not checkable in it: the axial placement of the barrel and RPV
(drawn centred on the reflector span; the stand-in has no axial extent for
them beyond their heights), and the cone's stepped shape (drawn at its top
radius).

## 7. How to run

```bash
cargo test --release -p nee_soon --lib htr10_th_transient -- --nocapture   # coupled (moved 2026-11-03)
cargo test --release -p tampines --lib htr10_primary_loop -- --nocapture   # loop-only TH
```

~~13 tests~~ (since the 2026-11-03 move: 9 in nee_soon, 6 loop-only in tampines); all but the helical-coil SG test (`long-tests`, ~26 min) finish
in under 10 s together.
