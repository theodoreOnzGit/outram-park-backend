# AI V&V items: MELCOR 2.2 COR package, part 1 (Vol. 2 lines 4219-11000)

AI-extracted draft, not human reviewed. Source texts: MELCOR 2.2 Vol. 2 Reference
Manual (SAND2023-10994 O), COR package, page labels `COR-RM-nn`; Vol. 1 Users' Guide
(SAND2017-0455 O), COR sensitivity coefficients, page labels `COR-UG-nn`. Equations
and numbers only; no prose copied. Every expected value below was computed with
`python3` (scripts: calc/calc_c1.py`, `c2.py`), not taken from our code.
The manual prints no numeric worked examples in this range; the one built-in
consistency check it offers (the emissivity table reproducing Eqs 2-7/2-8) is used in
COR1-01.

Code paths: `S` = `crates/sembawang/src/bwr_mark1/`, `O` =
`crates/outram-park-fork-offbeat/src/`. `O/melcor/` holds only RN/aerosol code; none of
the COR items below lives there.

---

## COR1-01 Oxidised-Zircaloy surface emissivity

**Citation.** RM §2.1.1, Eqs 2-7 to 2-10 and the interpolation table, COR-RM-28/29;
UG C1104(1..9), COR-UG-183/184.

**Model.** Linear interpolation in oxide thickness `dr` (m):
`(0, 0.325)`, `(3.8799999e-6, 0.808448)`, `(1.0e-3, 0.758642)`, constant beyond 1 mm.
If the surface's *maximum-ever* temperature `Tmax > 1500 K`, multiply by
`f = exp((1500 - Tmax)/300)`. Bounds: UG C1104(8,9) = [0.325, 0.9999] on the emissivity.

**Test cases.**
| dr (m) | Tmax (K) | expected eps |
|---|---|---|
| 0 | 1000 | 0.325000 |
| 2.0e-6 | 1000 | 0.574200 |
| 3.8799999e-6 | 1000 | 0.808448 |
| 5.0e-4 | 1000 | 0.783642 |
| 2.0e-3 | 1000 | 0.758642 |
| 2.0e-3 | 1600 | 0.543591 (= 0.758642 e^-1/3) |
| 5.0e-4 | 1800 | 0.325 (floor; raw 0.2883) |
| 2.0e-3 | 3000 | 0.325 (floor) |

Manual self-check: Eq 2-8 `0.808642 - 50 dr` gives 0.808448 at 3.88e-6 m and 0.758642 at
1 mm; Eq 2-7 `0.325 + 0.1246e6 dr` gives 0.808448 at 3.88e-6 m (continuous).

**Our code.** `S/core.rs:533` `zr_emissivity`. Test `radiation_follows_melcors_emissivities_and_conserves`
(`S/core.rs:2537`) pins *our* rounded table (0.808, 0.759), not the manual's values.

**Discrepancies.**
1. Table values rounded: 0.808 vs 0.808448, 0.759 vs 0.758642. At dr = 5e-4 m ours is
   0.783595 vs 0.783642 (-5.9e-5 relative); at dr >= 1 mm, +4.0e-4 relative.
2. Uses present temperature, not the maximum ever reached (stated in the doc comment).
3. Manual internal conflict: RM text says the *factor f* has a lower bound of 0.325 and
   Eq 2-10 bounds eps to [1e-4, 0.9999]; UG C1104(9) bounds the *emissivity* at 0.325.
   Code follows the UG. Under the RM-literal reading eps(2 mm, 3000 K) = 0.758642 x 0.325
   = 0.246559, not 0.325. A test should cite which reading it pins.

---

## COR1-02 Steel (SS/NS) and particulate-debris emissivity

**Citation.** RM §2.1.1, Eqs 2-11, 2-12, COR-RM-29; UG C1102(1..5) COR-UG-182/183,
C1103(1..5) COR-UG-183.

**Model.** Steel: `eps = clamp(0.25617 + 3.474e-4 (T - 616.4833), 1e-4, 0.9999)`, T in K
(reference = 650 F, slope 0.000193 /F x 1.8 = 3.474e-4 /K). Particulate debris:
`clamp(0.9999 + 0 x (T - 1000), 1e-4, 0.9999)` = 0.9999 at all T by default.

**Test cases (steel).** T = 300 K: 0.146224; 616.4833 K: 0.256170; 1000 K: 0.389404;
1500 K: 0.563104; 3000 K: 0.9999 (cap reached at T = 2757.33 K). Floor 1e-4 is only
reached at T = -120.6 K, so it never binds physically. Check: (650 - 32)/1.8 + 273.15 =
616.4833 K.

**Our code.** `S/core.rs:553` `steel_emissivity`; same test as COR1-01 (only checks
616.48 K). Particulate emissivity: not implemented as such (lower-plenum debris radiation
not modelled with C1103).

**Discrepancy.** Reference temperature 616.48 vs 616.4833 K: +1.146e-6 absolute in eps
(already documented in the code as a known, kept deviation).

---

## COR1-03 Two gray surfaces with an intervening gray medium

**Citation.** RM §2.1, Eqs 2-1 to 2-6, COR-RM-26/27/28.

**Model.** Radiosity `J_i`; surface resistance `q_i = A_i eps_i/(1-eps_i) (Eb_i - J_i)`;
surface-surface `q_ij = (A F)_ij tau (J_i - J_j)`; surface-medium
`q_im = A_i eps_m (J_i - Eb_m)`, `tau = 1 - eps_m`; balance `q_i = q_im + q_ij`;
`Eb = sigma T^4`, sigma = 5.670374419e-8 W/(m^2 K^4). Solve the 2x2 linear system for J1, J2.
With eps_m = 0 it reduces to
`q = (Eb1 - Eb2) / [ (1-eps1)/(A1 eps1) + 1/(AF) + (1-eps2)/(A2 eps2) ]`.

**Test cases (per m^2, A1 = A2 = 1).**
- (a) AF = 1, eps 0.8/0.5, 1500/1000 K, no medium: q = 102381.760 W (equals
  `sigma dT4/(1/e1 + 1/e2 - 1)`).
- (b) AF = 0.25, eps 0.325/0.95, 1500/560 K, no medium: **q = 45922.778 W**.
- (c) as (a) with eps_m = 0.3, T_m = 1200 K: q1 = 125565.320, q12 = 84138.172,
  q1m = 41427.147, q2 = -78770.242, q2m = 5367.931 W; energy balance
  q1 + q2 = q1m + q2m = 46795.078 W.

**Our code.** `S/core.rs:559` `gray_exchange` = `F A sigma (T1^4 - T2^4)/(1/e1 + 1/e2 - 1)`,
used by `radiation` (596), `bottom_radiation` (705), `top_radiation` (740). No medium term.
Test `the_bottom_of_the_core_radiates_to_the_lower_plenum` (`S/core.rs:2507`) checks our
formula, not the network solution.

**Discrepancy.** For F < 1 our form is not the series solution of Eqs 2-3/2-4: case (b)
gives 22486.12 W from ours vs 45922.78 W from Eqs 2-1..2-6 (factor 2.04). The two agree
only at F = 1 with equal areas. Whether MELCOR's actual A1, A2 for these paths (component
areas, which can be much larger than A_cell; §2.1.2, §2.1.4) shrink the gap was not
determined; a test should pin Eqs 2-1..2-6 directly. Steam absorption (eps_m) not modelled.

---

## COR1-04 Mean beam length and optical depth for steam emissivity

**Citation.** RM §2.1.1, Table 2-2, Eqs 2-13, 2-14, COR-RM-30/31.

**Model.** Fuel/clad: `BL = 3.5 (P - 2 R_cl)`; channel/bypass/structure surfaces:
`BL = 1.8 x {BLRC = 4 mm, BLCCB = 5.05 mm, BLCC = 14 mm}` per Table 2-2. Optical depth
`POD = P_steam BL / 1013.25` (P in Pa; atm cm results when BL is in metres, since P/1013.25 = 100 P_atm). Absorptivity
`alpha_v = eps_m(T_v, POD) (T_v/T_s)^0.45`.

**Test case.** P = 16.2 mm pitch, R_cl = 6.135 mm: BL = 3.5 x (16.2 - 12.27) mm = 13.755 mm
= 0.013755 m; at P_steam = 7.0e6 Pa, POD = 7.0e6 x 0.013755/1013.25 = 95.0259 atm cm
(= 69.085 atm x 1.3755 cm). With eps_m = 0.2, T_v = 1200, T_s = 1500: alpha_v = 0.2 x 0.8^0.45 =
0.180892. (Table 2-1 values are an image in the extracted text; not reproduced.)

**Our code.** Not implemented (no steam radiation). No test.

**Note.** The manual does not state BL's unit in Eq 2-13; atm cm comes out only with BL in m.
A test should state that reading.

---

## COR1-05 Intercell radiation exchange factor (FCELR model)

**Citation.** RM §2.1.3, Eqs 2-18, 2-20, 2-28 and limits 2-21..2-23, 2-29..2-31,
COR-RM-35/36/37.

**Model.** `alpha_i L_i = A_i/(K A_cell)`;
`AF = A_cell K (1 - e^-a1L1)(1 - e^-a2L2)`;
`(AF)_eff = 2 (A_cell K)^2/(A1 + A2) { [1 - (1 + a1L1) e^-a1L1](1 - e^-a2L2) + (1 - e^-a1L1)[1 - (1 + a2L2) e^-a2L2] }`.

**Test cases (A_cell = 1 m^2, K = 1).**
- A1 = 2, A2 = 3: AF = 0.821616, (AF)_eff = 0.502756.
- both large, A1 = 200, A2 = 300: AF = 1.000000, (AF)_eff = 0.008000 (Eq 2-29 limit
  4 (A_cell K)^2/(A1+A2) = 0.008, matches).
- both small, A1 = 1e-3, A2 = 2e-3: AF = 1.997003e-6 (Eq 2-23 limit A1A2/(K A_cell) = 2e-6,
  matches); (AF)_eff = 1.996448e-6.
- cell 1 small, cell 2 large, A1 = 1e-3, A2 = 300: AF = 9.995002e-4 (Eq 2-22 limit A1,
  matches); (AF)_eff = 6.666643e-6.

**Our code.** Not implemented (our rings exchange through fixed view factors). No test.
UG note: the FCELR/FCELA defaults are 0.1 for DEFAULT = 2.0 (0.25 legacy), COR-UG-30.

**Discrepancy in the manual's printed limits (as extracted).** Expanding Eq 2-28: both-small
gives `A1 A2/(K A_cell)` (= 2e-6 above), but Eq 2-31 as extracted reads `(1/2)(A1^2 + A2^2)/(A1 + A2)`
(= 8.33e-4); cell-1-small gives `2 K A1 A_cell/(A1 + A2)` (= 6.67e-6, matches the closed form),
while Eq 2-30 reads `K A1 A_cell/(A1 + A2)` (factor 2 lower). Possibly a text-extraction
artefact; a test should pin the closed form Eq 2-28, not the printed limits. Flag for the
maintainer to check against the PDF.

---

## COR1-06 Fuel-cladding gap conductance

**Citation.** RM §2.2.8, Eqs 2-80 to 2-83, COR-RM-56/57; UG C1101(1,2) (eps_f 0.8,
eps_c 0.325), COR-UG-182.

**Model.** `1/h_gap = 1/h_f + 1/(1/(1/h_g + 1/h_CF) + h_rad)` with `h_f = 4 k_f/R_FU`,
`h_g = k_g/dr_g`, `h_rad = 4 sigma ((T_CL + T_FU)/2)^3/(1/eps_f + 1/eps_c - 1)`; without a
control-function resistance the `1/h_CF` term is absent. (PBR variant: `h_f = 5 k_f/R`, §2.2.9.)

**Test case.** k_f = 3.0 W/(m K), R_FU = 5.2 mm, k_g = 0.3 W/(m K), dr_g = 80 um,
T_FU = 1500 K, T_CL = 1000 K: h_f = 2307.692, h_g = 3750.000, h_rad = 133.1555,
**h_gap = 1447.480 W/(m^2 K)**. Limit: dr_g -> 0 gives h_gap -> h_f.

**Our code.** `S/params.rs:3147` `fuel_to_clad_w_per_m_k`:
`1/(1/(8 pi k) + 1/(2 pi R h))` per metre, i.e. `h_f = 4k/R` (matches Eq 2-81) in series
with a constant `GAP_H` = 5000 W/(m^2 K) (`S/params.rs:65`); with k = 3, R = 5.2 mm, the
area-based h = 1578.947 W/(m^2 K). No test against Eq 2-80.

**Discrepancy.** No gap-radiation term and no k_g(T)/dr_g; gap conductance is a constant
`[A]` input. Form otherwise consistent with Eq 2-80 (h_rad = 0, h_g = GAP_H).

---

## COR1-07 Effective cladding heat capacity

**Citation.** RM §2.2.11, Eq 2-93 (with the implicit pair 2-87/2-88), COR-RM-58/61.

**Model.** `C_CL,eff = C_CL + C_FU h_gap A_f dt/(C_FU + h_gap A_f dt)`.

**Test case.** C_CL = 1000 J/K, C_FU = 5000 J/K, h_gap = 5000 W/(m^2 K), A_f = 0.5 m^2,
dt = 0.1 s: **1238.0952 J/K**. Limits: dt -> 0 gives C_CL; dt -> infinity gives
C_CL + C_FU (1e6 s: 5999.99).

**Our code.** Not used as such; `S/core.rs` (around 1605) solves the fuel/clad pair
implicitly (equivalent to Eqs 2-87/2-88 with constant G). No test.

---

## COR1-08 Forced convection (laminar developing + Dittus-Boelter, max)

**Citation.** RM §2.3.1-2.3.2, Eqs 2-99, 2-101 to 2-104, COR-RM-63/64; UG C1212, C1213,
C1214 (not re-read here; defaults as stated in the RM).

**Model.** `Nu_lam = 4.36 g_dev`, `g_dev = 1 + 0.00826/(F + 0.0011)`,
`F = (z - z0)/(D_h Re Pr)` with `z - z0` = 1000 m in the code; `Nu_turb = 0.023 Re^0.8 Pr^0.4`;
`Nu = max(Nu_lam, Nu_turb)`; `h = Nu k/D_h`.

**Test cases.** D_h = 0.012 m: Re = 5000, Pr = 1.0: g_dev = 1.000496, Nu_lam = 4.362161,
Nu_turb = 20.936488, **Nu = 20.936488**. Re = 100, Pr = 0.9: Nu_turb = 0.877859,
**Nu = Nu_lam = 4.360039**. Developing (z - z0 = 0.1 m, Re = 1000, Pr = 1): g_dev =
1.875618, Nu = 8.177696. Switch point at Pr = 1: Re = (4.36/0.023)^1.25 = 703.39.

**Our code.** `S/core.rs:1217` `steam_htc` (max(4.36, DB)); mass-transfer twin in
`diffusion_limited_rate` (`S/core.rs:1168`). g_dev omitted (stated; <= 5e-4 at core Re).
No test of `steam_htc` against Eq 2-104.

**Discrepancy.** MELCOR relaxes h with C1200 f_old = 0.5 vapour / 0.9 liquid (Eq 2-100);
not done in ours (timestep-scheme detail, not a physics constant).

---

## COR1-09 Free convection in narrow channels, max with forced

**Citation.** RM §2.3.3, Eqs 2-105, 2-106, COR-RM-64/65 (C1221, C1222).

**Model.** `Nu_lam = 0.18 Ra^(1/4) (L/D_h)^(-1/9)`, `Nu_turb = 0.065 Ra^(1/3) (L/D_h)^(-1/9)`;
free Nu = max of the two; final h = max(forced h, free h).

**Test case.** Ra = 1e6, L = 4 m, D_h = 0.012 m: Nu_lam = 2.985057, Nu_turb = 3.408737,
free Nu = 3.408737. Crossover Ra = (0.18/0.065)^12 = 2.03381e5.

**Our code.** Not implemented: `steam_htc` has forced convection only.

**Discrepancy.** At stagnant steam (w -> 0) ours floors at Nu = 4.36 (laminar forced); MELCOR
would take max(4.36 g_dev, free). With the inputs above free Nu (3.41) is below 4.36, so the
gap only opens at Ra > ~2.09e6 for L/D_h = 333 (computed: 0.065 Ra^(1/3) (333.3)^(-1/9) = 4.36 gives
Ra = 2.0926e6). Not measured in the plant model.

---

## COR1-10 Convection from particulate debris

**Citation.** RM §2.3.4, Eqs 2-107, 2-108, COR-RM-65 (C1231, C1232).

**Model.** Forced `Nu = 2.0 + 0.6 Re^(1/2) Pr^(1/3)`; free `Nu = 2.0 + 0.6 Gr^(1/4) Pr^(1/3)`;
max of the two; properties at film temperature.

**Test case.** Re = 100, Pr = 0.7: Nu = 7.327424; Gr = 1e4, Pr = 0.7: Nu = 7.327424;
Re -> 0: Nu -> 2.0.

**Our code.** Not implemented (lower-plenum debris uses quench/dryout and a fixed coefficient).

---

## COR1-11 MARCH-2 boiling curve (optional, non-default)

**Citation.** RM §2.3.6, Eqs 2-109 to 2-111, COR-RM-66 (C1241, C1242; default is the HS
package's boiling, so this is an option).

**Model.** `h = 34.5 P^(1/4) dT^1.523` for dT < 23.4 K; `h = 1.41e7 P^(1/4) dT^-2.575` for
dT >= 23.4 K; film regime adds `h_rad = sigma 0.4 (Ts^4 - Tl^4)/(Ts - Tl)`.

**Test case.** P = 7e6 Pa: dT = 10 K: 59168.91 W/(m^2 K); dT = 23.39999 K: 215977.78;
dT = 23.4 K: 216153.76 (near-continuous switch, +0.08 %); dT = 100 K: 5134.446;
h_rad(1100 K, 560 K) = 57.3655 W/(m^2 K).

**Our code.** Not implemented; covered rods use constant `BOILING_H` = 2e4 (`S/params.rs:72`, `[A]`).

---

## COR1-12 Lipinski 0-D dryout heat flux

**Citation.** RM §2.3.8, Eqs 2-114, 2-115, COR-RM-69/70; UG C1244(1..3) = 0.756,
0.089 m, 0.15, COR-UG-192.

**Model.** `q_d = 0.756 h_lv sqrt( rho_v (rho_l - rho_v) g d eps^3 (1 + lambda_c/L) / ((1 - eps)(1 + (rho_v/rho_l)^(1/4))^4) )`,
eps floored at 0.15, `lambda_c = 0.089 m x (0.5 mm/d)` (the manual says only that the
capillary head is adjusted for particle size; Eq 2-115 gives `lambda_c ~ 1/d`).

**Test cases** (rho_l = 915, rho_v = 2.67 kg/m^3, h_lv = 2.108e6 J/kg, g = 9.81, L = 1 m):
d = 0.01 m, eps = 0.4: lambda_c = 0.00445 m, **q_d = 5.309088e6 W/m^2**; d = 0.5 mm, eps = 0.4:
lambda_c = 0.089 m, q_d = 1.236103e6; ~~eps = 0.05 -> floored to 0.15: 1.024314e6~~
**CORRECTED 2026-11-26 (found by the unit test):** 1.024314e6 is for **d = 0.01 m**,
eps = 0.05 -> floored to 0.15 (`calc/calc_c2.py` computes that case); the text had
paired it with d = 0.5 mm, where the floored value is 2.384887e5. Side check of Eq 2-115 with water at 373 K (sigma 0.0589 N/m, cos = 1,
eps = 0.4, d = 0.5 mm): lambda_c = 0.1129 m, the same order as the 0.089 m default.

**Our code.** `S/core.rs:1119` `lipinski_dryout_flux`; test `lipinski_dryout_matches_a_hand_value`
(`S/core.rs:2664`) checks 5.31e6 to 2 % (uses IF97 saturation, so it is a hand check, not an
exact pin). Tighten by injecting the property values above.

---

## COR1-13 Falling-debris quench: leading edge and decay factor

**Citation.** RM §2.3.8, Eqs 2-112, 2-113, COR-RM-68; UG C1020(1) = 360 s (TSPRS),
COR-UG-181; RM §3.2.5, COR-RM-147 region.

**Model.** `z_d = z_csp - v_d (t - t_fail)`, v_d default 5 m/s.
`f(t + dt) = min[1, f(t) exp(-dt/tau_spr) + V_cor/V_LP]`; quench h applies as `f h_q`
until f < 0.01, after which only the dryout flux limits.

**Test cases.** f = 1, ten steps of 10 s, no inflow: f = 0.757465 (= e^(-100/360)).
f = 0.9, dt = 10 s, V_cor/V_LP = 0.05: f = 0.925344. Time to f = 0.01 with no inflow:
360 ln 100 = 1657.86 s. Leading edge: z_csp = 5 m, 0.4 s after failure: z_d = 3.0 m.

**Our code.** `S/core.rs:1319` (decay), `892`/`1936` (inflow term), `1338` (0.01 switch);
`QUENCH_TAU_S` = 360 s `S/params.rs:524`. No unit test of the recursion.

**Note.** The code comment at `S/params.rs:519-524` says the coefficient table lists
§3.2.5 against C1020(2) = 60 s; that is Vol. 2's table (line ~15553, outside this range).
Vol. 1 UG C1020(1) gives 360 s for solid material, agreeing with the text, so the conflict
is inside Vol. 2 only.

---

## COR1-14 Molten-pool convection: internal Rayleigh and ACOPO

**Citation.** RM §2.4.2, Eqs 2-119, 2-120, 2-126, Table 2-4, Eq 2-138, COR-RM-72/74/75/80;
UG C1280(1,1..2) = 0.3, 0.22, COR-UG-196; C1281 (time constant).

**Model.** `Ra_i = g beta Q H^5/(lambda nu^2) Pr`; transient form uses
`Q = (Q_decay + sum boundary losses)/2` (Eq 2-126); `Nu = A Ra_i^n`, `h = Nu lambda/H`.
Table 2-4: j=1 oxide->radial A=0.3, n=0.22 (ACOPO); j=2,3 oxide->interface/atmosphere
A=0.381, n=0.234 (Bonnet); j=4 metal->lower A=0.069, n=0.333, m=0.074 (Globe-Dropkin,
`Nu = A Ra^n Pr^m`); j=5,6 metal A=0.3, n=0.22. Manual quotes lambda in kW/(m K) and Q in
kW/m^3; only the ratio Q/lambda enters, so consistent SI gives the same Ra.

**Test case.** rho = 8000, mu = 4.4e-3, cp = 500, lambda = 3.0, beta = 1.05e-4, Q = 1e6 W/m^3,
H = 0.5 m: Ra_i = 2.601136e13, Nu(j=1) = 268.1990, **h = 1609.194 W/(m^2 K)**;
Nu(j=2/3) = 0.381 Ra^0.234 = 524.8959. Transient Q with decay 1 MW and losses 2 MW over 1 m^3:
1.5e6 W/m^3.

**Our code.** `S/core.rs:421` `pool_h` (j = 1 only, mean coefficient, transient Q);
test `the_plenum_pool_convects_as_melcor_says` (`S/core.rs:2649`) pins 1609.19 to 1e-4.
Already checked against the manual's form.

**Note (manual).** RM says the default inertial time constant is zero; UG C1281(1) lists
0.1 s. Our code has no inertia term.

---

## COR1-15 Molten-pool mixture viscosity and expansion coefficient

**Citation.** RM §2.4.2, Eqs 2-121, 2-122, 2-123, COR-RM-73.

**Model.** `mu_i = OMEGA exp(ALPHA/T)`; oxidic mixture `mu = ( sum x_i mu_i^(1/3) / sum x_i )^3`
(Kendall-Monroe); `beta = sum f_i v_i beta_i / sum f_i v_i (1 + beta_i (T - 1673))`.

**Test case.** x = (0.6, 0.4), mu = (4.0e-3, 6.0e-3) Pa s:
mu = (0.6 x 0.158740 + 0.4 x 0.181712)^3 = 4.735613e-3 Pa s (computed:
`(0.6*4e-3**(1/3)+0.4*6e-3**(1/3))**3`). Single material at T = 1673 K: beta_mix = beta_i.

**Our code.** Not implemented (`POOL_MU`, `POOL_BETA` constants `[A]`, `S/params.rs`).

---

## COR1-16 Parabolic Zr-steam kinetics, Urbanic-Heidrick (default)

**Citation.** RM §2.5.1, Eqs 2-156 to 2-158, COR-RM-91; §2.5.4, Eq 2-169, Table 2-6,
COR-RM-97/98; UG C1001(1..6,1) COR-UG-175, C1004(1) = 1100 K, C1004(2) = 9900 K,
COR-UG-177.

**Model.** `W_f^2 = W_i^2 + K(T_i) dt`, W in kg Zr/m^2. `K = 0` below 1100 K;
`29.6 exp(-16820/T)` for 1100 <= T <= 1853; `87.9 exp(-16610/T)` for T >= 1873 (to 9900 K);
between, linear in T between K(1853) and K(1873) (Eq 2-158 third branch). Units
(kg/m^2)^2/s.

**Test cases.** K(1099.9) = 0; K(1100) = 6.769152e-6; K(1500) = 3.993953e-4;
K(1853) = 3.381629e-3; K(1863) = 7.879247e-3 (interpolated); K(1873) = 1.237687e-2;
K(2000) = 2.173568e-2; K(2400) = 8.675792e-2. Step: W_i = 0.1, 100 s at 1500 K:
W_f = 0.2234715 kg/m^2. From zero, 1 h at 1500 K: 1.199093 kg/m^2.
Branch jump the interpolation bridges: low branch at 1873 = 3.72581e-3 vs high = 1.23769e-2.

**Our code.** `S/core.rs:1131` `oxidation_rate`, constants `S/params.rs:635-647`, integration
`S/core.rs:1523-1527` (`sqrt(W^2 + K dt)`, capped at the Zr available). Test
`the_oxidation_rate_is_continuous_and_rising` (`S/core.rs:2567`) checks monotonicity and
continuity only, **not values**. Add value pins above.

**Discrepancy.** No upper cutoff at TOXMAX = 9900 K (immaterial). K is evaluated at the
start-of-step clad temperature, as Eq 2-157 states.

---

## COR1-17 Zr-oxygen kinetics and steam/O2 combination

**Citation.** RM Table 2-6 row 3, Eqs 2-170 to 2-173, COR-RM-97/98; UG C1001(.,2),
COR-UG-175/176, and the UG's DEFAULT = 1.86 vs 2.0 table, COR-UG-30/31.

**Model.** Parabolic, Zr + O2 -> ZrO2. Two sets of O2 constants exist: RM Table 2-6 gives
`50.4 exp(-14630/T)` (1100-10000 K); UG C1001 gives, for the MELCOR 2.x defaults (DEFAULT = 2.0),
`26.7 exp(-17490/T)` for both branches (breakpoints 9998/9999 K), and lists 50.4/14630 as the
legacy DEFAULT = 1.86 values. With both oxidants `dW/dt = max(dW/dt_H2O, dW/dt_O2)` (Eq 2-171);
optional split `f_H2O = r_H2O/(r_H2O + r_O2)`, `f_O2 = 1 - f_H2O`; default consumes O2 before steam.

**Test cases.** 2.x default: K_O2(1500) = 2.304823e-4, K_O2(2000) = 4.252125e-3.
Legacy/RM table: K_O2(1500) = 2.928274e-3, K_O2(2000) = 3.354026e-2.
At W = 0.5 kg/m^2, 1500 K (dW/dt = K/(2W)): steam 3.993953e-4 kg/(m^2 s); O2 (2.x)
2.304823e-4, max = 3.993953e-4, f_H2O = 0.634084; O2 (legacy) 2.928274e-3, max = 2.928274e-3,
f_H2O = 0.120023.

**Our code.** Not implemented (no air ingress in-vessel). No test.

**Discrepancy (manual-internal).** RM Table 2-6 shows the legacy (1.86) O2 constants while the
UG says the 2.x default is 26.7/17490: a factor 12.7 in K at 1500 K. A test must say which set.

---

## COR1-18 Optional Zr oxidation correlations

**Citation.** RM §2.5.5, Eq 2-174, Tables 2-7, 2-8, COR-RM-98/99.

**Model.** `K = A1 exp(-B1/T)` for T < T1; `A2 exp(-B2/T)` for T > T2; between, linear in T
between the two end values. Steam rows (T1, T2, A1, B1, A2, B2):
Cathcart-Pawel/U-H (1853, 1873, 294.2, 20111, 87.9, 16610);
Leistikow-Schanz/Prater-Courtright (1800, 1900, 426.0, 20953, 26778.5, 26461);
Leistikow (1100, 9900, 426.01, 20953, same); Urbanic-Heidrick (1853, 1873, 29.6, 16820,
87.9, 16610); Sokolov (1763, 1783, 1291.2, 23040, 798.5, 20800); Grosse (1223, 1273,
6633.6, 25310, 72.2, 18066.6; C-P/U-H above 1673 K).

**Test cases.** Cathcart-Pawel K(1500) = 4.424985e-4; Leistikow-Schanz K(1500) =
3.655062e-4, K(1850) (interpolated) = 1.385337e-2; Sokolov K(1500) = 2.755689e-4.

**Our code.** Not implemented (U-H only). No test.

---

## COR1-19 Heat of Zr-steam reaction and its reference state

**Citation.** RM §2.5.2, Eqs 2-166, 2-167, Table 2-5, COR-RM-95/96.

**Model.** `Q(T) = Q(T0) + H_r(T) - H_p(T) - [H_r(T0) - H_p(T0)]`. Table 2-5: Zircaloy/steam
5797 kJ/kg Zr; Zircaloy/air 12065 kJ/kg Zr; SS/steam `(-249.5 X_Fe + 2442 X_Cr)/(X_Fe + X_Cr)`
kJ/kg SS; SS/air `(4869 X_Fe + 1091 X_Cr)/(X_Fe + X_Cr)`; Al/steam 15180; graphite/steam
-10850; graphite/air 9273 (CO), 3280.7 (CO2) kJ/kg.

**Test case (reference-state check).** With dHf(ZrO2) = -1100.6 kJ/mol,
dHf(H2O, g) = -241.826, dHf(H2O, l) = -285.83 kJ/mol, M_Zr = 91.224 g/mol:
vapour reference -> 6.763001 MJ/kg Zr; **liquid reference -> 5.798255 MJ/kg Zr**, within
2.2e-4 of the table's 5797 kJ/kg. The difference, 2 x 44.0 kJ/mol / 91.224 = 0.9647 MJ/kg,
is the latent heat of the two moles of water at 298 K. So the 5797 figure is consistent with
water referenced to liquid at T0 (the CVH convention); it is not a different reaction energy.

**Our code.** `S/params.rs:665` `OXIDATION_HEAT_J_PER_KG_ZR` = `O/corrosion/exotherm.rs:145`
(6.763 MJ/kg, vapour at 298.15 K), plus steam enthalpy above 298 K *vapour*
(`S/core.rs:1100`) and minus H2 heating (`S/core.rs:1540-1547`). Test
`reaction_enthalpy_is_derived_and_lands_in_the_published_band` (`O/corrosion/exotherm.rs`,
~line 280) checks 616.95 kJ/mol; nothing references Table 2-5.

**Discrepancy.** None in physics if the steam reference is consistently vapour (as coded).
A naive comparison of our 6.763 MJ/kg with MELCOR's 5.797 MJ/kg reads as +16.7 %; a test
should compare at a common reference (pin 5.798 MJ/kg from the liquid-reference derivation
against the table's 5.797).

---

## COR1-20 Stainless-steel oxidation: rate, stoichiometry, heat

**Citation.** RM §2.5.6, Eqs 2-176 to 2-179, Table 2-9, Table 2-5, COR-RM-96/100;
UG C1002(1,1) = 2.42e9, (2,1) = 4.24e4, COR-UG-176; C1004(1) = 1100 K.

**Model.** `K = 2.42e9 exp(-42400/T)` (kg SS/m^2)^2/s, 1100-9900 K; Fe + H2O -> FeO + H2,
Cr + 3/2 H2O -> 1/2 Cr2O3 + 3/2 H2, Ni + H2O -> NiO + H2, C + H2O -> CO + H2. Heat per kg SS
from Table 2-5 (Ni and C carry none).

**Test case** (X = 0.74/0.18/0.08 Fe/Cr/Ni, no C): K(1100) = 4.402883e-8, K(1500) = 1.281616e-3,
K(1700) = 3.564512e-2; steam 19.80669 mol/kg SS = 0.3568176 kg/kg; H2 0.03993030 kg/kg;
heat 277.0978 kJ/kg (air: 4129.826 kJ/kg).

**Our code.** `S/core.rs:909` `ss_oxidation_rate`, `S/core.rs:918` `ss_reaction`; constants
`S/params.rs:366-370, 471-475`. Test `the_blade_steel_reacts_as_melcor_says`
(`S/core.rs:2466`) pins these to 1e-3/2e-3. Already checked; could be tightened to 1e-6.

---

## COR1-21 Gaseous-diffusion limit on oxidation

**Citation.** RM §2.5.3, Eq 2-168, COR-RM-96/97; UG C1003(1) = 0.00548, C1003(2) = 0.00504
kg K/(Pa m^3), COR-UG-176/177.

**Model.** `dW/dt = (M/(n R)) k_c P_ox/T_f`, `T_f = (T + T_gas)/2`; k_c from the Nu
correlations with Sc for Pr and Sh for Nu; used when below the parabolic rate. For O2 the
constant is doubled internally.

**Test cases.** M_Zr/(2R) = 0.091224/(2 x 8.314462618) = 5.485863e-3 (C1003(1) = 0.00548,
-1.1e-3 relative). k_c = 0.05 m/s, P_ox = 1e5 Pa, T_f = 1500 K: rate = 1.826667e-2 kg/(m^2 s)
with 0.00548 (1.828621e-2 with the derived constant).
Steel: Fe with n = 1 gives 6.716610e-3; Fe -> Fe3O4 (n = 4/3) gives **5.037457e-3 = C1003(2)**;
our mixture-weighted constant (per kg SS: 1/(19.80669 mol/kg)/R) gives 6.072308e-3.

**Our code.** `S/core.rs:1168` `diffusion_limited_rate` (Zr); steel scaling at
`S/core.rs:1572`. Test `the_diffusion_limit_follows_melcors_equation` (`S/core.rs:2260`)
checks the Zr constant to 1e-5 absolute. No steel test.

**Discrepancy.** Steel diffusion constant: ours 6.0723e-3 vs MELCOR default 5.04e-3 (+20.5 %).
MELCOR's default corresponds to Fe -> Fe3O4 stoichiometry, while its own steel reaction set
(Eq 2-176) is Fe -> FeO; so the manual is itself internally inconsistent here. Ours follows the
reaction set. Applies only with `blade_diffusion_limit`.

---

## COR1-22 Simple B4C-steam model (MARCON 2.1B)

**Citation.** RM §2.5 intro (gates), §2.5.9, Eqs 2-197 to 2-208, COR-RM-89/105/106;
UG C1005(2) = 0.02, C1005(3) = 0.9, C1005(4) = 1500 K, COR-UG-177; C1006(1) = 1.662e5 /s,
C1006(2) = 2.26472e4 K, COR-UG-178.

**Model.** `y_CO/CO2 = (P_H2/P_H2O) exp(-3605/T + 3.427)`;
`y_CO/CH4 = (P_H2O/P_H2^3) exp(-27350/T + 30.50)`, pressures in atm;
`x_CO = 1/(1 + 1/y1 + 1/y2)`, `x_CO2 = x_CO/y1`, `x_CH4 = 1 - x_CO - x_CO2`.
Steam per mol B4C `7x_CO + 8x_CO2 + 6x_CH4`; H2 `7x_CO + 8x_CO2 + 4x_CH4`.
Energy J/kmol B4C: `Q1 = 8.238e8 - 58380 T`, `Q2 = 8.674e8 - 67060 T`, `Q3 = 1.056e8 - 61430 T`.
Rate `d(M/M0)/dt = (9.973e6/60) exp(-22647.2/T)` /s. Gates: reaction starts only after intact
blade steel < 0.9 of initial, B4C T > 1500 K, at most 0.02 of initial B4C; energy to CVH gas.

**Test cases.**
- T = 1600 K, P_H2 = P_H2O = 1 atm: y1 = 3.234502, **y2 = 6.641412e5**, x_CO = 0.7638438,
  x_CO2 = 0.2361550, x_CH4 = 1.150123e-6; steam 7.236154, H2 7.236152 mol/mol;
  Q = 7.374078e8 J/kmol = 7.374078e5 J/mol.
- T = 1600, P_H2 = 0.1, P_H2O = 10: x_CO = 0.0313316, x_CO2 = 0.9686684, steam 7.968668,
  Q = 7.591731e5 J/mol.
- T = 1200, P_H2 = 20, P_H2O = 0.1 (reducing): x_CH4 = 0.9728322, steam 6.027257,
  H2 4.081592, Q = 5.149831e4 J/mol.
- Rate: 1500 K 4.609338e-2 /s; 1600 K 1.184277e-1; 1200 K 1.057742e-3. 9.973e6/60 =
  166216.67 vs C1006(1) 1.662e5 (+1.0e-4).

**Our code.** `S/core.rs:934` `b4c_reaction`; rate and cap at `S/core.rs:1588-1594`;
constants `S/params.rs:463-468`. Test `the_b4c_reacts_as_melcor_says` (`S/core.rs:2489`).

**Discrepancies.**
1. **No 1500 K threshold (C1005(4)) and no 0.9 intact-steel gate (C1005(3))**: our B4C reacts
   whenever the node is uncovered, steam is present and the blade has not melted. At 1200 K
   the rate is 1.06e-3 /s, so the 2 % cap could be consumed in ~19 s below MELCOR's threshold.
   Impact on the plant results not measured.
2. Energy goes to the cladding node, not CVH gas (stated in the code).
3. Test doc comment says `y_CO/CH4 = 6.39e5` at 1600 K; the value is 6.641e5 (the assertion
   only bounds x_CH4 < 1e-5, so it still passes). Doc-comment arithmetic slip.

---

## COR1-23 Eutectic mixture solidus

**Citation.** RM §2.8.1-2.8.2, Eq 2-230, Table 2-12, COR-RM-120/121; UG C1011 (thresholds
1400 K / 1520 K).

**Model.** `TS_mix = sum_j sum_{i != j} f_i f_j TS_ij / sum_j sum_{i != j} f_i f_j`; TS_ij is the
mole-weighted mean of the two solidus temperatures, except for Table 2-12 pairs (molar ratio,
eutectic T): Zr-Inconel 0.76/0.24 1210 K; Zr-steel 0.76/0.24 1210; ZrO2-UO2 0.50/0.50 2800;
Zr-B4C 0.43/0.57 1900; steel-B4C 0.69/0.31 1420; Zr-AgInCd 0.67/0.33 1470; for these, the
mole-weighted mean of the eutectic T and the solidus of the component in excess of the
eutectic composition. Liquidus = solidus + 0.01 K. Early-failure thresholds: Zr-Inconel and
Zr-steel 1400 K, B4C-steel 1520 K.

**Test case** (interpretation: lever rule, eutectic fraction = deficient component / its
eutectic fraction; ZrO2 2990 K, UO2 3113 K): x_ZrO2 = 0.3: 0.6 eutectic + 0.4 excess UO2
-> 2925.2 K; x_ZrO2 = 0.5 -> 2800.0 K; x_ZrO2 = 0.8 -> 2914.0 K. Plain mole weighting would
give 3076.1 K at x = 0.3. The weighting rule's wording is terse; state the interpretation in
the test.

**Our code.** Not implemented as a function: `POOL_MELT_K` = 2800 K constant
(`S/params.rs:436`). The eutectics model is off by default in MELCOR (COR_MS), so the default
control-blade failure is at steel melting (our `SS_MELT_K` 1700 K) rather than 1520 K.

---

## COR1-24 Rod-collapse criteria and oxide-shell holdup

**Citation.** RM §3.1.3, COR-RM-134; §3.2.1, COR-RM-141/142; UG C1131(1..6) COR-UG-185,
C1132(1,2) COR-UG-185/186, COR_CCT DRZRMN = 1e-4 m COR-UG-41.

**Model.** Molten Zr held by its oxide shell if ZrO2 thickness >= 1e-5 m (DZXMN) AND
T < 2400 K (TZXMX, cladding; 2100 K for canisters, C1131(6)) AND no candling yet from that
component; steel shell: 1e-3 m and 1700 K. Rod with metal < 1e-4 m (DRZRMN) counts as
oxidised; an oxidised rod stands to 2500 K (TRDFAI), any rod fails at 3100 K (TRDMAX).

**Test cases (truth table).**
| oxide (m) | T_clad (K) | Zr molten? | held? |
|---|---|---|---|
| 2e-5 | 2300 | yes | yes |
| 2e-5 | 2400 | yes | no (T >= TZXMX) |
| 5e-6 | 2300 | yes | **no** (oxide < DZXMN) |
| canister, 2e-5 | 2150 | yes | no (TZXMX1 = 2100) |
Rod: metal 5e-5 m, T = 2499 K stands; 2500 K collapses; metal 5e-4 m, T = 3099 K stands,
3100 K collapses.

**Our code.** `S/core.rs:1757-1770` (collapse), `S/core.rs:1794` (candling gate),
constants `S/params.rs:526-548`. Covered indirectly by conservation tests only; no truth-table test.

**Discrepancies.**
1. Holdup ignores the oxide-thickness condition (DZXMN): our molten Zr never candles below
   2400 K, even with < 10 um of oxide.
2. Collapse uses fuel temperature `tf` against 2500 K; RM §3.2.1 says the *cladding* reaching
   2500 K, UG C1132(1) just "temperature". Flag which one is pinned.
3. The "no candling yet" condition is not tracked.

---

## COR1-25 Time-at-temperature fuel-rod failure

**Citation.** RM §3.2.2, Eqs 3-29, 3-30, COR-RM-143.

**Model.** `1/L(T) = A exp(B T)`, A = 2.16e-11 /s, B = 7e-3 /K (50th percentile);
`D(t) = sum dt/L(T)`; failure at D >= 1.

**Test cases.** L(2000) = 38496.70 s (10.6935 h); L(2300) = 4714.168 s; L(2500) = 1162.500 s;
L(2700) = 286.6689 s. 1 h at 2300 K then 30 min at 2400 K: D = 1.532562 (fails).

**Our code.** `S/core.rs:1759` (accumulated only while the rod is "bare"), constants
`DAMAGE_A`, `DAMAGE_B` `S/params.rs` (~line 555). No unit test.

**Discrepancy.** Ours uses the fuel temperature and accumulates only for oxidised/breached
rods; the manual says the local core-cell temperature and does not state an oxidised-only
gate. Not checked further.

---

## COR1-26 Candling secondary transport and breakthrough timestep

**Citation.** RM §3.1.3, Eq 3-13, COR-RM-134; §3.1.4, Eqs 3-14, 3-15, COR-RM-135;
UG COR_CMT (MTUOZR = 1, FUOZR = 0.2; MTZXZR = 1, FZXZR = 0; MTSXSS = 2, FSXSS = 1.0;
MTCPSS = 2, FCPSS = 0), COR-UG-41; C1141 (dt_break = 1 s, Gamma_max = 1 kg/(m s)).

**Model.** Mechanism 1: `dM_s = F1 dM_m`; mechanism 2: `dM_s = F2 (M_s,total/M_m,total) dM_m`.
Breakthrough: `dt_contact = max(dt, M_m dz/(Gamma_max A_surf))`.

**Test cases.** Mechanism 1, UO2 with 10 kg molten Zr deposited, F1 = 0.2: 2.0 kg.
Mechanism 2, F2 = 0.5, cell of origin 3 kg secondary of 12 kg total, 4 kg molten deposited:
0.5 x 3/12 x 4 = 0.5 kg.
Breakthrough: M_m = 200 kg, dz = 1 m, Gamma_max = 1, A_surf = 0.5 m: dt_contact = 400 s
vs dt = 1 s -> 400 s (note: the Eq 3-13 symbols as extracted are ambiguous about which
length multiplies M_m; state the reading).

**Our code.** `S/core.rs:1811` (`candling_uo2_fraction` = 0.2, `S/params.rs:432`). Covered by
`uo2_is_conserved_through_its_candling` (`S/core.rs:2618`), a conservation test, not a value
test. Eq 3-13 not implemented.

**Discrepancy.** Ours takes 0.2 of the molten mass *leaving* the source node; MELCOR's dM_m
is the molten mass *deposited* (refrozen) on a component. Same for a single hop to a cold node.

---

## COR1-27 Particulate-debris surface area

**Citation.** RM §3.2.1, Eq 3-28, COR-RM-142.

**Model.** `A_s = 6 V_p/D_p`; the Zr-oxidising share is the volume fraction of Zr + ZrO2 in
the particulate; steel likewise.

**Test cases.** V_p = 0.01 m^3, D = 0.01 m: 6.0 m^2. 1 kg Zr (6550 kg/m^3), D = 0.01 m:
0.0916031 m^2. V_p = 0.1 m^3, D = 2 mm: 300.0 m^2.

**Our code.** `S/core.rs:854` (plate debris), `S/core.rs:1336`, `S/core.rs:1363` (plenum
debris): `6 m_Zr/(rho_Zr D)`. No value test.

**Discrepancy (minor).** Ours counts the Zr metal volume only; MELCOR counts Zr + ZrO2 volume
(ZrO2 is ~1.56x the Zr volume it replaces), so ours under-states the area once oxide forms.

---

## Summary

- **27 items** (COR1-01 to COR1-27).
- **Already checked against the manual by a test:** COR1-14 (ACOPO `pool_h`), COR1-20 (steel
  rate/stoichiometry/heat), COR1-21 (Zr diffusion constant), COR1-22 (B4C split, energy and rate),
  COR1-12 (Lipinski, 2 % hand check). Partially: COR1-01/02 (pin our rounded constants),
  COR1-16 (monotonicity only, no values), COR1-19 (derivation tested, Table 2-5 not referenced).
- **Implemented but no manual-value test:** COR1-03 (gray exchange), COR1-06 (gap), COR1-08
  (forced convection), COR1-13 (quench recursion), COR1-16 values, COR1-24 (criteria),
  COR1-25 (TaT), COR1-26 (candling fraction), COR1-27 (debris area).
- **Not implemented:** COR1-04, 05, 07, 09, 10, 11, 15, 17, 18, 23.
- **Discrepancies found (no code changed):**
  1. Gray-body exchange form `F/(1/e1+1/e2-1)` is not the solution of Eqs 2-1..2-6 for F < 1
     (factor 2.04 in the worked case) (COR1-03).
  2. Zr emissivity table rounded (0.808/0.759 vs 0.808448/0.758642); current T, not max T; RM vs
     UG disagree on whether 0.325 floors the factor or the emissivity (COR1-01).
  3. B4C reaction lacks MELCOR's 1500 K threshold and 0.9 intact-steel gate (COR1-22); test doc
     comment's y_CO/CH4 = 6.39e5 should be 6.641e5.
  4. Steel gaseous-diffusion constant 6.07e-3 vs MELCOR default 5.04e-3 (+20.5 %); MELCOR's
     default matches Fe3O4 stoichiometry, inconsistent with its own FeO reaction (COR1-21).
  5. Oxide-shell holdup ignores the 1e-5 m oxide-thickness condition; rod collapse compares fuel
     (not cladding) temperature with 2500 K (COR1-24); TaT uses fuel T and an oxidised-only gate
     (COR1-25).
  6. No free-convection branch in `steam_htc` (COR1-09); constant gap conductance without
     radiation (COR1-06); debris area excludes ZrO2 volume (COR1-27).
  7. Not a defect but a trap for tests: MELCOR's 5797 kJ/kg Zr heat is the liquid-water-reference
     value (derived 5798.3); our 6.763 MJ/kg is the vapour-reference value (COR1-19).
  8. Manual-internal issues to check against the PDF: FCELR printed limits Eqs 2-30/2-31 do not
     follow from Eq 2-28 (COR1-05); O2 kinetics constants differ between RM and UG
     (RM Table 2-6 shows the legacy 1.86 O2 constants, UG gives 26.7/17490 for 2.x) (COR1-17);
     RM vs UG on the molten-pool inertia time constant (0 vs 0.1 s) (COR1-14).
