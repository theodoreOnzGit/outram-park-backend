# MELCOR 2.2 unit-test reference items: BUR, CAV, FDI, LHC

AI draft (not human-reviewed), 2026-11-26. Proprietary-tier source: only
equations, numbers and citations are restated, in my own symbols; no prose
is copied. "RM" = Vol. 2 Reference Manual (`vol2.txt`), "UG" = Vol. 1 Users'
Guide (`vol1.txt`, SAND2017-0455 O). Page labels are the manuals' own
footers. Every expected value below was computed with python3; the full
scripts are in
`calc/ (the py_* scripts)`
(`bur.py`, `cav.py`, `cav2.py`, `fdi.py`, `lhc.py`), run with `python3 -I`.

Code paths are relative to `/home/user/outram-park-backend/crates/sembawang/src/bwr_mark1/`.
`outram-park-fork-offbeat/src/melcor/` (aerosol, iodine, vapour, RN volume)
holds none of these four packages.

Extraction caveats (pdftotext): several equations lost glyphs. Where I
reconstructed one, the item says how (dimensional check or a second
printed form in the UG) and marks it **re-check against the PDF**.

---

## BUR — combustion

### BUR-01 Le Chatelier ignition, oxygen and inerting tests

- **Cite:** RM BUR §2.2, Eqs 2-3, 2-4, 2-5, BUR-RM-7/8. Defaults UG
  record BUR_IGN (BUR-UG-4) and BUR_IGN01 (BUR-UG-5).
- **Law:** with mole fractions `x`, ignite when all three hold:
  `x_H2 + x_CO (L_H2 / L_CO) >= L_H2`; `x_O2 >= X_O2,ig`;
  `x_H2O + x_CO2 < X_dil,ig`. Limits (dimensionless mole fractions):
  no igniters `L_H2 = 0.10`, `L_CO = 0.167`; igniters `0.07`, `0.129`;
  DCH defaults to the igniter values; `X_O2,ig = 0.05`, `X_dil,ig = 0.55`
  (same with or without igniters; DCH copies them). D2 counts as H2 mole
  for mole (BUR-RM-5/6).
- **Cases (no igniters):** `(x_H2, x_CO, x_O2, x_D)`:
  `(0.10,0,0.2,0)` → Y = 0.1, ignites (equality is inclusive);
  `(0.099,0,0.2,0)` → no; `(0.06,0.0668,0.2,0)` → Y = 0.1000000 (exact
  to float), ignites; `(0.06,0.0669,...)` → Y = 0.100059880; `(0.2,0,0.049,0)`
  → no (O2); `(0.2,0,0.2,0.55)` → no (strict `<` on diluent).
  With igniters: `(0.05,0.05)` → Y = 0.0771317829, ignites;
  `(0.03,0.07)` → Y = 0.0679844961, no.
- **Edge:** equality at `x_D = 0.55` must fail, at `Y = L_H2` must pass.
  The H2/CO ratio in this test (0.10/0.167) is NOT the completeness ratio
  of BUR-02 (0.08/0.148); a common slip is to reuse one for the other.
- **Ours:** `containment.rs:481` `bur_ignites` (no-igniter set only; no
  igniter or DCH variants). Test `the_building_ignites_by_melcors_bur_criteria`
  (`containment.rs:2154`) checks exactly these cases. Constants
  `params.rs:395-402`.
- **Discrepancies:** none in the formula. Old model (`containment.rs:1484`)
  passes only `x_CO2` as the diluent (stated: building steam condensed);
  `containment_network.rs:1438` passes steam + CO2, as the manual does.

### BUR-02 Combustion completeness (HECTR 1.5)

- **Cite:** RM §2.3, Eqs 2-6 to 2-9, BUR-RM-9/10; UG BUR_COM (BUR-UG-6)
  and C2202 (BUR-UG-19/20).
- **Law:** `Y = x_H2 + x_CO (0.08 / 0.148)` (XH2CC, XCOCC defaults);
  `CC = 1 - Y_min / Y_max`; default correlation `CC = 0` for
  `Y_max <= 0.03746`, else `CC = 23.4116 (Y_max - 0.03746)`, constrained to
  `[0, 1]` (UG C2202 states the clamp; its general form is
  `Y_min/Y_max = C(1) - C(3)(Y_max - C(2))`, defaults 1.0, 0.03746, 23.4116).
- **Cases:** Y = 0.05 → CC = 0.293581464, Y_min = 0.0353209268;
  Y = 0.06 → CC = 0.527697464, Y_min = 0.0283381522;
  Y = 0.08 → CC = 0.995929464; CC reaches 1 at Y = 0.0801738683.
  H2 5 % + CO 3 %: Y = 0.0662162162, CC = 0.673229032,
  Y_min = 0.0216375371.
- **Edge:** Y = 0.03 and Y = 0.03746 → CC = 0 exactly; Y = 0.12 → 1 (clamped).
- **Ours:** `containment.rs:489` `bur_completeness`, constants
  `params.rs:403-415`. Test `the_building_burns_by_melcors_bur_rates`
  (`containment.rs:2175`) checks 0.06 → 0.5277 (to 1e-4), 0.03 → 0, 0.12 → 1.
- **Discrepancies:** none.

### BUR-03 Base flame speed, piecewise in Y_max

- **Cite:** RM §2.4, Eqs 2-10 to 2-16, BUR-RM-10/11; UG C2200
  (BUR-UG-17/18), coefficients C2200(1..7) = 59.2, 1.792, −9.576, 15, 45,
  55, 48.58 m/s, breakpoints C2200(14..18) = 0.1, 0.2, 0.3, 0.4, 0.6.
- **Law (m/s):** `V_base = 59.2Y + 1.792` (0..0.1]; `172.88Y − 9.576`
  (0.1..0.2]; `50Y + 15` (0.2..0.3]; `−50Y + 45` (0.3..0.4];
  `−75Y + 55` (0.4..0.6]; `−64.3Y + 48.58` (0.6..1]. The UG builds each
  slope from continuity, `s_{i+1} = s_i + (C_{i+1} − C_{i+2}) / Y_i`;
  computed: 59.2 → 172.88 → 50.0 → −50.0 → −75.0 → −64.3 (the RM
  slopes are recovered exactly, so the law is continuous at every
  breakpoint).
- **Cases:** V_base(0) = 1.792; (0.05) = 4.752; (0.1) = 7.712; (0.15) =
  16.356; (0.2) = 25.0; (0.25) = 27.5; (0.3) = 30.0 (peak); (0.35) = 27.5;
  (0.4) = 25.0; (0.5) = 17.5; (0.6) = 10.0; (0.7) = 3.57; (0.75) = 0.355.
- **Edge:** V_base crosses zero at Y = 0.755520995 and is −15.72 at Y = 1.
  A mixture with Y in (0.7555, 0.95] and `x_O2 >= 0.05` passes BUR-01 yet
  has a negative flame speed.
- **Ours:** `containment.rs:498` `bur_flame_speed`. Test (`containment.rs:2178-2183`)
  checks V(0.12) = 11.1696 and continuity at 0.2.
- **Discrepancies:** in the negative-speed band, `bur_step`
  (`containment.rs:431`) sets `left_s = CDIM / V < 0`, so the burn is
  silently never run although `started` is returned `true`. The manual
  gives no guard either; what MELCOR does there is not stated. Flag for the
  maintainer: a test should pin whichever behaviour is chosen.
  The UG's printed (Y2, Y3] line repeats the (Y1, Y2] line, a typo: the
  term `(C3 − C4)/Y2` is missing; the RM and continuity both give 50.

### BUR-04 Diluent factor on flame speed

- **Cite:** RM Eqs 2-17 to 2-19, BUR-RM-10/11; UG C2200(8..13) = 1.0,
  −4.53, 5.37, 0.05, 1.0, −1.29 (BUR-UG-18).
- **Law:** `X_D = x_H2O + x_CO2`. Lean
  `C_L = max(0.05, 1 − 4.53 X_D + 5.37 X_D^2)` for Y_max <= 0.2; rich
  `C_R = max(0, 1 − 1.29 X_D)` for Y_max >= 0.3; between, linear blend
  `C_L (0.3 − Y)/0.1 + C_R (Y − 0.2)/0.1`. `V = V_base · C_dil`.
- **Cases:** (Y, X_D) → C_dil, V: (0.1, 0) → 1.0, 7.712; (0.1, 0.2) →
  0.3088, 2.3814656; (0.1, 0.4) → 0.05 (floor), 0.3856; (0.1, 0.5) →
  0.0775, 0.59768; (0.25, 0.2) → 0.5254, 14.4485; (0.35, 0.2) → 0.742,
  20.405; (0.35, 0.8) → 0.0, 0.0.
- **Edge:** the lean quadratic has its minimum 0.0446508 at X_D = 0.421788,
  below the 0.05 floor, and rises again: at the inerting limit X_D = 0.55
  it is 0.132925, so between X_D ≈ 0.42 and 0.55 more diluent gives a
  faster flame. Correct per the correlation; worth pinning.
- **Ours:** `containment.rs:510-519`. Test checks 0.3088 and 0.742.
- **Discrepancies:** none. RM Eq 2-18 is printed with a misplaced
  parenthesis; UG C2200 prints the blend unambiguously and matches the
  code's reading (the code's doc comment says so already). The RM's
  ranges overlap at Y = 0.3 (2-18 "≤ 0.3", 2-19 "0.3 ≤"); both give C_R
  there.

### BUR-05 Burn duration and default characteristic dimension

- **Cite:** RM Eq 2-20, BUR-RM-11; UG BUR_BRT field CDIM (BUR-UG-10/11):
  default −1/DEF → radius of the equal-volume sphere `(3V/4π)^(1/3)`;
  TFRAC default 0.0; CDDH, TFDH default to CDIM, TFRAC.
- **Law:** `t_comb = CDIM / V` (s), `CDIM` in m, `V` in m/s.
- **Cases:** V = 26,000 m^3 → CDIM = 18.3778589 m; 1,000 m^3 → 6.20350491 m.
  26,000 m^3, 12 % H2, no diluent: V = 11.1696 m/s, t_comb = 1.64534620 s.
  6 % H2: V = 5.344, t_comb = 3.43897060 s.
- **Edge:** V → 0 (Y_max at 0.7555 or C_R = 0 at X_D ≥ 0.775 when Y ≥ 0.3)
  gives an unbounded duration.
- **Ours:** `containment.rs:527` `bur_cdim`, used at 431. Test checks
  CDIM(26,000) = 18.378 and t_comb via the burn's remaining time.
- **Discrepancies:** none.

### BUR-06 Burn rate and species balance

- **Cite:** RM §2.5, Eqs 2-21 to 2-23 and the species paragraph,
  BUR-RM-11/12; timestep control C2201 (0.005 combustible, 0.01 diluent
  overshoot), BUR-RM-15, BUR-UG-19.
- **Law:** `YRATE = (Y(t) − Y_min) / (t0 + t_comb − t)`;
  `ΔH2 = x_H2 · YRATE · Δt / Y`, `ΔCO = x_CO · YRATE · Δt / Y` (as fractions;
  times total moles for moles), each capped at the inventory; remove
  ΔH2, ΔCO and `0.5(ΔH2 + ΔCO)` O2, add ΔH2 steam and ΔCO CO2; total mass
  and energy unchanged (formation enthalpies are in the EOS).
- **Case:** 1000 mol gas, 100 mol H2, 20 mol CO, X_D 0.1, 26,000 m^3:
  Y = 0.110810811, CC = 1 → Y_min = 0, V = 5.75529046 m/s, t_comb =
  3.19321136 s; first step Δt = 0.5 s from t0: YRATE = 0.0347019969 /s,
  ΔH2 = 15.6582181 mol, ΔCO = 3.13164362 mol, O2 used 9.39493087 mol.
  Stepping to the end with no sources returns Y = 1.0e-18 (Y_min reached,
  as the manual says it is exactly without flows).
- **Edge:** Δt > remaining time must clamp to the remaining time; zero O2
  must burn nothing.
- **Ours:** `containment.rs:425` `bur_step` (fraction burnt
  `(y − y_min)/left_s · h / y`, the same law), O2 cap at 443-447. Test
  `the_building_burns_by_melcors_bur_rates` checks a two-step 12 % burn and
  a 6 % burn ending at (1 − 0.5277)·0.06.
- **Discrepancies:** (a) the O2 cap is the code's own guard; the manual
  caps only the fuel. (b) The manual says these values are refreshed every
  step ("Y_min(t)"); the code fixes `y_min` at ignition. With no flows in
  the volume the two agree; with flows they can differ. Ambiguous in the
  RM; flag. (c) C2201 overshoot fallbacks are not implemented (no
  sub-stepping on crossing).

### BUR-07 Propagation criteria and delay

- **Cite:** RM §2.6, Eq 2-24 and the propagation inequality, BUR-RM-12/13;
  UG BUR_COM fields 3-8 (BUR-UG-6/7), BUR_BRT TFRAC (BUR-UG-11).
- **Law:** neighbour burns when `x_H2 + x_CO (L_H2/L_CO) >= L_H2` with
  (L_H2, L_CO) = upward (0.041, 0.125), horizontal (0.06, 0.138),
  downward (0.09, 0.15); allowed after `t_prp = TFRAC · t_comb`; not through
  a closed or water-covered path.
- **Cases:** 5 % H2: up yes, horizontal no, down no. 4 % H2 + 3 % CO: up
  Y = 0.04984 yes; horizontal Y = 0.0530434783 no; down Y = 0.058 no.
  `t_prp` for the 12 % building burn at TFRAC 0.5 = 0.822673099 s.
- **Edge:** TFRAC = 0 propagates at ignition; 1 only at the end.
- **Ours:** not implemented (no multi-volume propagation;
  `containment_network` burns only the building).

### BUR-08 Detonation flag

- **Cite:** RM §2.7, Eqs 2-25 to 2-27, BUR-RM-14; UG BUR_DET (BUR-UG-5).
- **Law:** warn only when `x_H2 > 0.14`, `x_O2 > 0.09` and `x_H2O < 0.30`
  (all strict); no physics change.
- **Cases:** (0.15, 0.10, 0.29) → flagged; (0.14, …) → not (strict).
- **Ours:** not implemented.

---

## CAV — core-concrete interaction

### CAV-01 Standard concrete compositions

- **Cite:** UG CAV_C0/CAV_C1, Table 2.1 (mass %), CAV-UG-10 to 12; species
  list RM App. A, CAV-RM-30/31 (CaCO3 → CaO + CO2 and Ca(OH)2 → CaO +
  H2OCHEM at initialisation).
- **Data (mass %), as extracted** (columns: CORCON basaltic | CORCON
  limestone/common sand | CORCON generic SE US | BASALT | LIMESTONE/CS | CRBR):
  SiO2 54.84 | 35.80 | 3.60 | 65 | 36 | 4; TiO2 1.05 | 0.18 | 0.12; MnO – |
  0.03 | 0.01; MgO 6.16 | 0.48 | 5.67; CaO 8.82 | 31.30 | 45.40; Na2O 1.80 |
  0.082 | 0.078; K2O 5.39 | 1.22 | 0.68; Fe2O3 6.26 | 1.44 | 1.20; Al2O3
  8.32 | 3.60 | 1.60 | 20 | 5 | 3; Cr2O3 – | 0.014 | 0.004; CO2 1.50 |
  21.15 | 35.698; H2OCHEM 2.00 | 2.00 | 2.00; CaCO3 – | – | – | 3 | 48 |
  81; Ca(OH)2 – | – | – | 8 | 8 | 8; H2OEVAP 3.86 | 2.70 | 3.94 | 4 | 3 | 4.
  The layout of the three CORCON sub-columns is garbled in the text;
  **re-check against the PDF**. The SRP type (4) has no column.
- **Derived test values** (CO2 and H2O incl. decomposition; gas moles per
  kg concrete; Zr needed to reduce all of it, kg/kg):
  CORCON basaltic: sum 100.0000, CO2 0.015000, H2O 0.058600,
  3.593626 mol/kg, 0.163912; CORCON L/CS: sum **99.9960**, 0.211500,
  0.047000, 7.414674, 0.338198; GSEUS: 100.0000, 0.356980, 0.059400,
  11.408628, 0.520370; BASALT: 0.013191, 0.059452, 3.599803; LIMESTONE/CS:
  0.211062, 0.049452, 7.540812; CRBR: 0.356167, 0.059452, 11.393031.
- **Edge:** the CORCON L/CS column sums to 99.996, not 100 (UG notes
  unnormalised fractions are accepted); a test should normalise or
  tolerate 4e-5.
- **Ours:** `params.rs:98-107`: CORCON basaltic, `CONCRETE_H2O_FRACTION
  0.0586`, `CONCRETE_CO2_FRACTION 0.0150`, matching the derived values.
  Only these two fractions are used; the oxides are lumped (as SiO2,
  `CONCRETE_OXIDE_M`, \[A\]). No test pins them against the table.
- **Discrepancies:** none for the fractions. The digest's "~3.6" and
  "~7.4 mol/kg" agree with 3.5936 and 7.4147.

### CAV-02 Standard concrete properties

- **Cite:** UG CAV_C2, Table 2.2, CAV-UG-13/14.
- **Data:** density kg/m^3 2340 | 2340 | 2340 | 2400 | 2340 | 2340;
  solidus K 1350 | 1420 | 1690 | 1350 | 1420 | 1690; liquidus K 1650 |
  1670 | 1875 | 1650 | 1670 | 1875; ablation K 1450 | 1500 | 1750 | 1450 |
  1500 | 1750; initial T 298 K and emissivity 0.6 for all. The UG says
  these are not recommended values.
- **Case/edge:** ablation temperature lies between solidus and liquidus for
  every type (assert `T_sol < T_abl < T_liq`).
- **Ours:** `params.rs:104-105` density 2340, ablation 1450 K (CORCON
  basaltic). Solidus/liquidus not used (the melting range is the
  pseudo-binary, `cavity.rs:193`, with the ablation temperature as the
  low-melting component's melting point).
- **Discrepancies:** none in the values used.

### CAV-03 Quasi-steady ablation rate

- **Cite:** RM §3.3/§3.4, CAV-RM-14/15 (no equation number): mass ablation
  rate per area = heat flux to concrete / heat of ablation; adiabatic
  floor when the debris is below `T_abl`; preheat neglected. SC2301(1) adds
  DDELH to the internal ablation enthalpy (default 0, CAV-UG-33).
- **Law:** `m'' = q'' / h_a` (kg/m^2/s); `v = q'' / (rho_c h_a)` (m/s);
  `q'' = 0` and no ablation when `T <= T_abl`.
- **Case:** h = 1000 W/m^2K, A = 21.2 m^2, T = 2400 K, T_abl = 1450 K:
  q = 20.14 MW, with our derived h_a = 1.816 MJ/kg → 11.0903084 kg/s;
  recession 2.35325e-4 m/s per MW/m^2 (84.717 cm/h per MW/m^2) at 2340 kg/m^3.
  Components of our h_a: sensible 1.147 MJ/kg, bound water 0.121008 MJ/kg,
  CO2 0.0606687 MJ/kg (match `params.rs:109-116`).
- **Edge:** T = 1400 K → zero ablation, zero gas.
- **Ours:** `cavity.rs:283-284` (and 325-326 with phases). Test
  `a_hot_pool_ablates_and_reduces_the_concrete_gases` (`cavity.rs:366`)
  checks 11.09 kg and the zero case.
- **Discrepancies:** h_a is derived here (\[A\], 1.816 MJ/kg); the RM gives
  no number (CORCON's internal value). The debris-to-concrete `h = 1000`
  is \[A\], not CORCON's gas-film/bubbling correlation (stated in the code).

### CAV-04 Slag-film resistance factor

- **Cite:** RM §3.3, CAV-RM-15 (Bradley): slag-film coefficient = 0.41 ×
  the interior-to-surface coefficient, so the series value is
  `0.41/(1 + 0.41)` of it. Default model in MELCOR is the gas film
  (GFILMBOTT/GFILMSIDE).
- **Case:** 0.41/1.41 = 0.290780142 (RM rounds to 0.29).
- **Ours:** not implemented (single `cavity_h`).

### CAV-05 Mechanistic mixing over a step (entrainment vs settling)

- **Cite:** RM §3.2.3, Eqs 3-1 to 3-6, CAV-RM-12/13. Default MIXING is
  enforced mixing (§3.2.1).
- **Law:** `dM_D/dt = ṁ_e(0) − M_D/τ_s`, `τ_s = L_M / v_settle`;
  `M_D(t) = M_D0 + (ṁ_e τ_s − M_D0)(1 − e^{−t/τ_s})`; steady `ṁ_e τ_s`;
  net move over Δt `(ṁ_e τ_s − M_D0)(1 − e^{−Δt/τ_s})`, positive limited to
  the lower layer's mass.
- **Case:** L_M = 0.5 m, v = 0.01 m/s (τ_s = 50 s), ṁ_e = 2 kg/s,
  M_D0 = 10 kg: M_D(10 s) = 26.3142322 kg (explicit Euler at 1e-4 s:
  26.3142470); M_D(50 s) = 66.8908503; steady 100; ΔM_net(10 s) = 16.3142322.
- **Edge:** M_D0 > ṁ_e τ_s gives a negative move, never more than M_D0.
- **Ours:** not implemented (one layer is MELCOR's default, `cavity.rs:6-13`).

### CAV-06 Water ingress: dryout flux, crack temperature, conduction depth, permeability

- **Cite:** RM §3.7.3, Eqs 3-8, 3-9, 3-10, 3-15, CAV-RM-19 to 22; UG
  SC2315 (CAV-UG-35/36).
- **Law:** reconstructed from the garbled Eq 3-8 by dimensional analysis
  (W/m^2 forces exponents 5 and 4; **re-check against the PDF**):
  `q_dry = C_dry [ (h_lv (ρ_l − ρ_v) g / ν_v)^5 (N k_c^2 Δe_sat / (c_p Δe_cr))^4 ]^{1/13} [α_T (T_cr − T_sat)]^{15/13}`;
  `T_cr = T_s − σ_t / (α_T E)`; `δ_cond = k_c (T_B − T_sat) / q_dry`;
  `κ = 2 μ_v q_dry / (ρ_v h_lv (ρ_l − ρ_v) g)`. Constants: ρ_l 960, ρ_v 0.59
  kg/m^3, g 9.8, ν_v 1.29e-5, N 0.1 K m^1/2, σ_t 6.77e7 Pa, E 1.25e11 Pa.
- **Case** (assumed: k_c 1.5 W/mK, c_p 1000 J/kgK, T_s = T_B = 2000 K,
  T_sat 373.15 K, α_T 1e-5 /K, h_lv 2.257e6, Δe = c_p ΔT): T_cr = 1945.84 K
  (drop 54.16 K); `q_dry / C_dry` = 450.501834 W/m^2; `δ_cond · C_dry` =
  5.41679260 m; `κ / C_dry` = 9.28329993e-13 m^2.
- **Edge:** T_cr ≤ T_sat (large σ_t/(α_T E)) makes the last bracket ≤ 0:
  no ingress; a test should assert the guard.
- **Ours:** not implemented.
- **Manual-internal issues:** `C_dry` has no value in the RM; `ν_v` is
  called "dynamic viscosity" with units m^2/s, and 1.29e-5 is the order of
  steam's *dynamic* viscosity (Pa s) at 373 K, while Eq 3-15 uses `μ_v`.
  Tests must state which they take.

### CAV-07 Melt eruption (Ricou-Spalding)

- **Cite:** RM §3.7.4, Eqs 3-12 to 3-14, CAV-RM-21/22; UG SC2315(1) EENT.
- **Law:** `j_melt = K_ent j_gas`, `K_ent = E_ent (ρ_gas / ρ_melt)^{1/2}`;
  eruption only above `j_min = κ (ρ_c − ρ_m) g / μ_g` (floating crust).
- **Case:** ρ_gas 0.3, ρ_melt 7000: E_ent 0.06 → K = 3.92792202e-4,
  j_melt at j_gas 0.05 m/s = 1.96396101e-5 m/s; E_ent 0.08 → K =
  5.23722937e-4, j_melt = 2.61861468e-5.
- **Edge:** ρ_c < ρ_m (crust lighter, the floating case) makes `j_min`
  negative, e.g. −1.5163e-4 m/s per unit C_dry for κ above, ρ_c 7500,
  ρ_m 8000, μ_g 3e-5: the criterion then always passes.
- **Ours:** not implemented.
- **Manual-internal discrepancy:** default E_ent is **0.06** in the RM
  (CAV-RM-21) but **0.08** in UG SC2315(1) (CAV-UG-35). Flag.

### CAV-08 Melt viscosity: base laws, solid fraction, two-phase multipliers

- **Cite:** RM §3.8, Eqs 3-16 to 3-21, CAV-RM-22/23; UG SC2302
  (CAV-UG-33/34): VISMOD 0 = Kunitz (default), Ramacciotti coefficients
  2.5 and 6.0 (recommended 4-8).
- **Law:** steel `μ0 = 1.076e-3 exp(3313/T) M_user` Pa s; basalt fallback
  `μ0 = 1.94e-5 exp(20950/T) M_user`; `φ = clamp((T_liq − T_ave)/(T_liq − T_sol), 0, 1)`,
  `T_ave = (T_Z + T_R)/2`, capped at 0.9 in code; Kunitz
  `μ = μ0 (1 + 0.5φ)/(1 − φ)^4`; Ramacciotti `μ = μ0 exp(2.5 C φ)`.
- **Cases:** steel μ0(1800 K) = 6.77884015e-3, (2000 K) = 5.63924653e-3;
  basalt μ0(1800) = 2.20042692, (2000) = 0.687126073. φ(T_liq 2800, T_sol
  2200, T_Z 2500, T_R 2400) = 0.583333333. Kunitz φ = 0.1/0.3/0.5/0.9 →
  1.60036580 / 4.78967097 / 20.0 / 14500.0; Ramacciotti C = 6 → 4.48168907
  / 90.0171313 / 1808.04241 / 729416.370; C = 4 → 2.71828183 / 20.0855369
  / 148.413159 / 8103.08393.
- **Edge:** φ clamps to 0 above the liquidus and to 1 below the solidus
  (then 0.9 by the code cap).
- **Ours:** not implemented (no spreading; fixed `CAVITY_AREA_M2` 21.2).

### CAV-09 Debris spreading (Huppert-type)

- **Cite:** RM §3.8, Eqs 3-22 to 3-27, CAV-RM-23/24; UG SC2303 (CAV-UG-34):
  C1 = 0.136; stop at 0.5 of radius or 0.5 of thickness as crust.
- **Law:** `dR/dt = C1 ρ g V^3 / (μ π^3 R^7)` (Eq 3-24), integrated by
  explicit Euler in MELCOR; printed closed form (Eq 3-25)
  `R^8 = R0^8 + C1 ρ g V^3 (t − t0)/(μ π^3)`; `H = V/(π R^2)`.
- **Case:** ρ 8000, g 9.8, μ 1000 Pa s, V 1 m^3, R0 2 m, t 100 s:
  `C1 ρ g V^3/(μ π^3)` = 0.343878761; Eq 3-25 as printed → R = 2.03175955 m;
  exact integral of Eq 3-24 (`R^8 = R0^8 + 8 k t`) → 2.19102506 m;
  Euler of Eq 3-24 at Δt 1e-3 s → 2.19102551 m; H = 0.0663064 m.
- **Edge:** large dR/dt (low μ, small R0) makes explicit Euler unstable:
  μ = 0.01, V = 10, R0 = 1 at Δt 1e-3 runs away to 3.4e4 m.
- **Ours:** not implemented.
- **Manual-internal discrepancy (flag):** Eq 3-25 is not the integral of
  Eq 3-24 (missing a factor 8 in the R^8 increment). Separately, from my
  recollection of Huppert (1982), the axisymmetric constant-volume
  current is `R = 0.894 (g V^3 t / (3 ν))^{1/8}`, and `0.894^8 / 3` =
  0.136013, i.e. Huppert's 0.136 belongs to `R^8 = C1 ρ g V^3 t / μ` with
  neither the 8 nor π^3 (R = 2.45563484 m in the case above). Not checked
  against Huppert's paper here; a spreading port must decide which form
  MELCOR actually runs before pinning numbers.

---

## FDI — fuel dispersal

### FDI-01 LPME stream position and floor deposit

- **Cite:** RM FDI §2.2, Eqs 2-1 to 2-6, FDI-RM-6/7.
- **Law:** `z_head = z_top − U Δt`; `z_tail = z_head + m_f /(π D0^2 ρ_f / 4)`
  (the square on D0 is lost in the text; the stream-length dimension
  requires it, **re-check**); `m_flr = m_f clamp((z_bot − z_head)/z_len, 0, 1)`;
  if the head has not reached the floor, `m_flr = min(m_f, m_cav0)`,
  `m_cav = m_f + m_cav0 − m_flr`; when ejection stops, all of `m_cav0` lands.
- **Case:** z_top 10, z_bot 2 m, U 4 m/s, Δt 1.5 s, m_f 500 kg, D0 0.1 m,
  ρ_f 8000: z_head = 4.0, z_tail = 11.9577472 m, z_len = 7.95774715 m,
  m_flr = 0. With Δt 1 s and m_cav0 120 kg: m_flr = 120, m_cav = 500.
- **Ours:** not implemented (stated, `fdi.rs:29-32`: each packet lands in
  the step it leaves).

### FDI-02 Chu breakup, modified (and original)

- **Cite:** RM Eqs 2-7 to 2-14, FDI-RM-8/9.
- **Law:** `We = ρ_c U^2 D0 / σ_f`, `r = (ρ_c/ρ_f)^{1/2}`, `τ = (U t / D0) r`;
  original `D = D0 exp(−C1 τ^0.772 We^0.246)`, `C1 = 0.171 − 0.149 r`;
  MELCOR's `D = D0 exp(−C1 τ We^0.254)`, `C1 = 0.1232 − 0.149 r`; in depth
  `D(z) = D0 e^{−Z}`, `Z = (C1 We^0.254 / D0) r (z_pool − z)`; no breakup
  once solid.
- **Case:** ρ_c 958, ρ_f 8000, U 1.8793 m/s, D0 0.1 m, σ_f 0.45 N/m:
  r = 0.346049129, We = 751.874270, We^0.254 = 5.37701053, C1 = 0.0716386797
  (original 0.119438680), Z/m = 1.33298794; D(0.5 m) = 0.0513505792,
  D(1 m) = 0.0263688198 (time form gives the same), D(2 m) =
  0.00695314658; original Chu at 1 m: 0.0204303558.
- **Edge:** modified C1 ≤ 0 when ρ_c/ρ_f ≥ 0.683674 (light debris):
  Z ≤ 0, the droplet would grow.
- **Ours:** `fdi.rs:99` `z_per_m`, used in `bottom_temperature` (`fdi.rs:110`).
  Test `debris_falls_through_the_pedestal_water_as_melcor_says`
  (`fdi.rs:152`) checks Z/m against 1.335 to 2e-3.
- **Discrepancies:** (a) the code clamps Z at 0 (`.max(0.0)`); the manual
  has no clamp. Reasonable guard; state it. (b) "Solid" is taken at Zr's
  2098 K (\[A\], stated) rather than the debris's own T_sol,f. (c) The
  test's doc comment hand numbers are slightly off: with the test's own
  U = 0.6·(2·9.81·0.5)^{1/2} = 1.87925517, We = 751.838 (comment 750.9) and
  We^0.254 = 5.37695 (comment 5.384), Z/m = 1.332972 (comment 1.335). The
  assertion passes at its 2e-3 tolerance; the comment's numbers are a doc
  defect.

### FDI-03 Bromley film boiling and the regime-transition temperature

- **Cite:** RM Eqs 2-15, 2-16, FDI-RM-9.
- **Law:** `h_FB = ½ k_g [k_g μ_g D (T_g − T_sat) / (ρ_g (ρ_c − ρ_g) g i_fg)]^{−1/4}`,
  `T_g = (T_f + T_sat)/2`; `T_TRAN` solves
  `σ (T^4 − T_c^4) = h_FB (T − T_sat)` (unit emissivity); radiation-only
  above T_TRAN, film-boiling-only below. (Classical Bromley's leading 0.62
  becomes 0.5 here.)
- **Case** (fixed film properties for a pure unit test: k_g 0.08 W/mK,
  μ_g 3.5e-5 Pa s, ρ_g 0.25, ρ_c 958 kg/m^3, i_fg 2.257e6 J/kg, D 0.01 m,
  T_sat = T_c = 373.15 K): T_f 1000 K → h_FB 198.305728, q_FB 124307.946,
  q_rad 55604.3659 W/m^2; 1500 K → 171.261459, 192985.975, 285963.310;
  2000 K → 156.238942, 254177.322, 906160.466. T_TRAN = 1315.63318 K
  (h_FB 179.084403, both fluxes 168784.038 W/m^2).
- **Edge:** T_f ≤ T_sat → zero flux.
- **Ours:** `fdi.rs:58` `bromley_h` (properties from steam correlations,
  so the fixed-property case cannot call it directly), `fdi.rs:89` `flux`
  takes `max(rad, film)`. Test checks the radiation value at 2800 K and
  film > radiation at 1000 K.
- **Discrepancies:** `max` equals the manual's switch only when the
  crossing is single (true for these properties, f(400 K) < 0 < f(3000 K));
  not asserted. The manual calls the low regime "transition film boiling";
  it is the conduction film-boiling coefficient above.

### FDI-04 Falling-droplet energy equation

- **Cite:** RM Eq 2-17, FDI-RM-9/10.
- **Law:** `ρ_f c_f (π D^3/6) U dT/dz = −q (π D^2)` → `dT/dz = −6 q / (ρ_f c_f D U)`.
  Closed form for radiation only, constant D, T_c neglected:
  `T(z)^{−3} = T0^{−3} + 18 σ z / (ρ_f c_f D U)`.
- **Case:** ρ_f 8000, c_f 500, D 0.005 m, U 2 m/s, T0 2800 K, z 1 m:
  T = 2414.18178 K (RK4 at 1 mm: 2414.18178, agreement 1e-12).
- **Edge:** depth 0 → T0 exactly.
- **Ours:** `fdi.rs:110` `bottom_temperature` (RK4, 200 steps). Test
  checks only a band (2600-2750 K) and depth 0. The closed form above is a
  sharper check (set T_c ≪ T, freeze D by starting below 2098 K).

### FDI-05 Steam generated and pool cap

- **Cite:** RM Eqs 2-18 to 2-20, FDI-RM-10.
- **Law:** `Q = H_f(T0) − H_f(T_bot)`, `H_f = Σ m_i h_i(T)`;
  `m_steam = Q / (h_sat,vap(P) − h_pool)`; pool temperature unchanged; if
  m_steam > pool mass, back-calculate H_f(T_bot) to boil exactly the pool.
- **Case:** 1000 kg at c 400 J/kgK, 2800 → 2600 K: Q = 8.0e7 J; with
  h_sat,vap 2.6755e6 and h_pool 4.19e5 J/kg: m_steam = 35.4531354 kg. Pool
  30 kg: T_bot back-calculated to 2630.7625 K.
- **Ours:** `containment.rs:739` `lpme_fall` (760-765). No direct test of
  the cap found.
- **Discrepancy:** the code uses constant specific heats
  (`c_tot · ΔT`), so a packet crossing its solidus releases no latent heat;
  the manual's H_f(T) is the component enthalpy and does.

### FDI-06 HPME airborne mass: settling, oxidation, products

- **Cite:** RM Eqs 2-21 to 2-26, FDI-RM-12/13. (The printed C1/C2 of Eq
  2-26 are truncated in the text; the solution below is derived from Eq
  2-25 and checked numerically.)
- **Law:** inert: `m(t) = (m0 − S τ_ST) e^{−t/τ_ST} + S τ_ST`; oxidising
  metal: same with `1/τ_SO = 1/τ_ST + 1/τ_OX`; product k from reactant l:
  `dm_k/dt = −m_k/τ_ST + R m_l/τ_OX + S_k`.
- **Cases:** inert m0 5, S 2 /s, τ 10 s, Δt 3 s → 8.88772669 kg.
  Zr → ZrO2 (R = 123.222/91.224), τ_ST 10, τ_OX 5 (τ_SO 3.33333333),
  m_l0 4, S_l 1, m_k0 0.5, S_k 0.2, Δt 3 s: m_l = 3.60437977, m_k =
  3.52372070 (steady part τ_ST(S_k + R S_l τ_SO/τ_OX) = 11.0050864;
  transient coefficient −0.900508638); explicit Euler at 1e-5 s gives
  3.60437941, 3.52372226.
- **Ours:** not implemented (HPME needs user transport fractions; the
  code flags u > 10 m/s and still falls by LPME, as MELCOR does without
  FDI_CVH/FDI_DEP input, `fdi.rs:7-16`).

### FDI-07 HPME switch, oxidant split and FDI sensitivity defaults

- **Cite:** RM §2.3, Eq 2-27, FDI-RM-10/14; SC table FDI-RM-20.
- **Law/data:** HPME when ejection velocity > C4602 = 10 m/s; oxidation
  share to O2 `R_O2 = W x_O2 / (W x_O2 + x_st)`, W = C4610 = 1; C4603 3700 K,
  C4604 500 K, C4605 10, C4606 0.001, C4607 1e-4 s, C4608 1000 W/m^2K,
  C4609 600 K, C4620 tolerances 1e-6 and 2e-7.
- **Cases:** (W, x_O2, x_st) (1, 0.1, 0.3) → 0.25; (1, 0, 0.3) → 0;
  (2, 0.1, 0.3) → 0.4; (1e6, 0.1, 0.3) → 0.999997.
- **Ours:** `params.rs:428` `FDI_HPME_VELOCITY = 10.0`, used at
  `containment.rs:745` (flag only). Ejection velocity `fdi.rs:50` (COR RM
  Eq 6-34, outside this range) is tested (1.8793, 4.6403 m/s). Rest not
  implemented.

---

## LHC — lower head containment (a second, catcher head)

LHC is a separate catcher plate below the RPV, not the RPV lower head.
`core.rs`'s lower head (Larson-Miller creep, Lipinski dryout) is the COR
package's and lies outside this range; nothing in our code implements LHC.

### LHC-01 Moving-boundary crust: profile, mean and growth rate

- **Cite:** RM LHC §2.5.1, Eqs 2-13 to 2-18, LHC-RM-15/16.
- **Law:** `T'' − (u/α) T' = −Q/k`, `T(0) = T_sol`, `T(δ) = T_bnd`:
  `T(z) = T_sol − (T_sol − T_bnd + Qδ/(ρc_p u)) (e^{uz/α} − 1)/(e^{uδ/α} − 1) + Qz/(ρc_p u)`;
  mean over [0, δ]:
  `T̄ = T_sol − (T_sol − T_bnd + Qδ/(ρc_p u)) [ (α/(uδ))(e^{uδ/α} − 1) − 1 ]/(e^{uδ/α} − 1) + Qδ/(2ρc_p u)`
  (derived; the printed Eq 2-18 loses glyphs, **re-check**); growth
  `u = (q_bnd − q_slr − Qδ) / (ρ_m [h_fs − c_p (T_bnd − T_sol)])`.
- **Case:** T_sol 2800, T_bnd 400 K, Q 1e6 W/m^3, ρ 8000, c_p 500, k 3,
  u 1e-5 m/s, δ 0.02 m (α 7.5e-7, uδ/α 0.266667): T(δ/2) = 1696.52371 K;
  T̄ = 1664.36819 K (numeric mean 1664.36819); ODE residual −333331 vs
  −Q/k = −333333 (finite-difference). u from q_bnd 2e5, q_slr 1e5, h_fs
  2.7e5: 6.80272109e-6 m/s.
- **Edge:** u → 0 must reduce to `T_sol − (T_sol − T_bnd) z/δ + Q z(δ − z)/(2k)`
  (1616.6667 K at mid-depth); the closed form cancels catastrophically
  there, so implementations need `expm1` and a small-u branch.
- **Ours:** not implemented.

### LHC-02 Mixture enthalpy, effective specific heat, density

- **Cite:** RM Eqs 2-1 to 2-6, LHC-RM-11/12.
- **Law:** mass-weighted h and c_p over 12 components; inside each
  component's melting window [T_m, T_m + 0.1 K], `c_p,eff = c_p + h_fus/0.1`
  and `h = h(T_m) + c_p,eff (T − T_m)`; phase density
  `ρ = 1000 Σ M_i / Σ χ_i V_m,i (1 + β_i (T_ave − 1673))`.
- **Case:** UO2-like c_p 500, h_fus 2.74e5 → c_p,eff = 2.7405e6 J/kgK.
- **Ours:** not implemented.

### LHC-03 Crust-boundary temperature initialisation

- **Cite:** RM Eqs 2-11, 2-12, LHC-RM-14.
- **Law:** series-conductance weighted means:
  `T_B = (T_pl 2k_pl/Δz_n + T_L 2k_m/H_L)/(2k_m/H_L + 2k_pl/Δz_n)`,
  `T_T = (T_bnd h_bnd + T_L 2k_m/H_L)/(2k_m/H_L + h_bnd)`.
- **Case:** k_pl 40, Δz_n 0.01, k_m 3, H_L 0.2, T_L 2800, T_pl 600 →
  T_B = 608.219178 K; h_bnd 500, T_bnd 400 → T_T = 535.849057 K.
- **Ours:** not implemented.

---

## Summary

- 26 items: BUR 8, CAV 9, FDI 7, LHC 3 (LHC, a separate catcher head, is
  wholly unimplemented).
- **Implemented and tested against the manual:** BUR-01 to BUR-06
  (`containment.rs`), CAV-01/02 (values in `params.rs`), CAV-03
  (`cavity.rs`), FDI-02, FDI-03, FDI-04 (`fdi.rs`), FDI-07's switch.
  FDI-05 is implemented without a direct test.
- **Code vs manual, to flag:** (1) BUR flame speed is negative for Y_max >
  0.7555; `bur_step` then silently skips the burn while reporting it
  started. (2) `bur_step` fixes Y_min at ignition; the RM implies it is
  refreshed each step (equal without flows). (3) LPME steam uses constant
  c_p, missing the debris's latent heat. (4) FDI solidification at Zr's
  2098 K, not the debris solidus (\[A\], stated); Z clamped at 0.
  (5) `fdi.rs` test comment hand numbers are slightly off (We 750.9 vs
  751.84, Z 1.335 vs 1.3330); assertion still passes. Aside, outside the
  manual comparison: `cavity.rs:210` takes `&dyn Fn`, a trait object the
  workspace's Rust rules forbid.
- **Manual-internal inconsistencies:** CAV Eq 3-25 is not the integral of
  Eq 3-24 (factor 8), and Huppert's 0.136 fits neither form with π^3
  (from recollection; check); E_ent default 0.06 (RM) vs 0.08 (UG);
  ν_v unit/value mismatch and no value for C_dry in water ingress; UG
  C2200 (Y2, Y3] slope typo; RM Eq 2-18 parenthesis.
- Reconstructed equations marked **re-check against the PDF**: CAV Eq 3-8,
  Table 2.1 sub-columns, FDI Eq 2-2 (D0^2), LHC Eq 2-18.
