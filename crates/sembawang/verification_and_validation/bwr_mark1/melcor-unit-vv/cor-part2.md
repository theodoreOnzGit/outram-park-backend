# AI V&V items: MELCOR 2.2 COR package, second half (Vol. 2 lines 11000-17660)

Draft for human review (AI-extracted, not reviewed). Source: MELCOR 2.2 Vol. 2
Reference Manual, COR package (page labels `COR-RM-nn`, SAND2023-10994 O text as
extracted with pdftotext), and Vol. 1 Users' Guide (`COR-UG-nn`, SAND2017-0455 O).
Equations and numbers are cited; prose is paraphrased. Every expected value was
computed with python3 (full script:
calc/calc_cor2.py`;
each item repeats the lines that matter). g = 9.81 m/s^2 throughout.

Code paths are relative to `/home/user/outram-park-backend/crates/sembawang/src/bwr_mark1/`
unless stated. `crates/outram-park-fork-offbeat/src/melcor/` holds only RN-side
models (aerosol, iodine, vapour, rn_volume); none of the COR items below lives there.

Items COR2-14 to COR2-17 sit outside the assigned line range (Vol. 2 lines
< 11000) but their defaults are in the in-range coefficient table (COR-RM-243
to 248) and the caller asked for them by name; they may overlap the first-half
(COR1) extraction.

---

## COR2-01 Stainless-steel yield stress of supporting structure (SS)

- **Citation:** §5.3.1, Eq 5-16, COR-RM-165; constants are sensitivity array
  C1606 (table, COR-RM-248).
- **Form:** sigma_Y(T) = A [ (1 + (T/T1)^n)^-1 - (1 + (T2/T1)^n)^-1 ], Pa, T in K,
  A = 260e6 Pa, T1 = 800 K, T2 = 1700 K, n = 3. It is zero at T2; take it as
  zero above T2 (the bracket goes negative).
- **Worked cases:**

  ```python
  f=lambda t:1/(1+(t/800.)**3); sy=lambda T:max(0.0,260e6*(f(T)-f(1700.)))
  # 300 K -> 222437632 ; 800 K -> 105461751 ; 1000 K -> 63504079.2
  # 1273.15 K -> 27145536.4 ; 1600 K -> 4350640.04 ; 1700 K -> 0 ; 1800 K -> 0
  ```
  Edge: exactly 0 at 1700 K; clamped to 0 above.
- **Code:** `core.rs:781` `steel_yield_pa` (same form, `.max(0.0)`).
  **Test:** `core.rs:2446` `the_core_plate_yields_where_melcor_says` checks 63.5 MPa
  at 1000 K to 2e-3 and 0 at 1700 K. Tighter pin available: 63.5040792 MPa.
- **Discrepancy:** none in form or constants (constants are inline literals, not
  `Params` fields).

## COR2-02 Plate bending stress: PLATEG and PLATEB (BWR core plate)

- **Citation:** §5.2.1 Eqs 5-1 to 5-4, COR-RM-160 (K_square = 0.0513); §5.2.3
  Eq 5-13, COR-RM-163 (K_beam = 0.0833, beams without cross beams, Poisson 0.3).
  Vol. 1 COR_SS (COR-UG-51/52): THICK and SPACE have no default; AKMG = 0.0513,
  AKMB = 0.0833. PLATEB is loaded by its own weight plus particulate debris (PD)
  on it only (COR-RM-163).
- **Form:** sigma_ring = 6 K x^2 W_ring / (A_ring h^2), Pa; x = beam spacing (m),
  h = plate thickness (m), W_ring = load on the ring (N), A_ring = ring area (m^2).
  Fails when sigma >= sigma_Y(T) of COR2-01.
- **Worked cases:**

  ```python
  s=lambda K,x,h,A,W: 6*K*x*x/(A*h*h)*W
  s(0.0833,0.3,0.05,9.29,1.0e6)  # PLATEB -> 1936792.25 Pa
  s(0.0513,0.3,0.05,9.29,1.0e6)  # PLATEG -> 1192766.42 Pa
  # temperature at which PLATEB (above) reaches yield, bisection on COR2-01:
  # T_fail = 1652.93411 K
  ```
  Edge: W -> 0 gives sigma -> 0, so a plate with no debris fails only at 1700 K
  (yield -> 0).
- **Code:** `core.rs:877-880` inside `Core::one_plate_step` (`core.rs:834`);
  W = (plate mass + held UO2 + Zr) g, x = `CORE_PLATE_BEAM_M` 0.3, h =
  `CORE_PLATE_M` 0.05 (`params.rs:343-344`). **Test:** `the_core_plate_yields_where_melcor_says`
  (1.94 MPa at 1e6 N, failure between 1600 and 1690 K). The bisected 1652.93 K is
  a sharper pin.
- **Discrepancies:**
  1. MELCOR's default SS failure rule is parametric (TSFAIL at 1273.15 K, COR_SS
     ISSFAI/TSSFAI defaults, COR-UG-50/51); stress-based failure is a user
     choice. The code uses the stress-based PLATEB rule. That is a legitimate
     option, but it is not MELCOR's default, and the docs should say so. The
     plate's thickness and beam spacing are [A] values because MELCOR gives no
     defaults for THICK or SPACE.
  2. K_beam = 0.0833 is a literal at `core.rs:880`, not a `Params` field.
  3. No SS creep (see COR2-05). The doc comment says so.

## COR2-03 Column-supported plate coefficient and ligament efficiency

- **Citation:** §6.3.1 Eq 6-32 (COR-RM-180) with the manual's own example
  (nu = 0.3, x/c = 5 gives K = 0.0826); Eq 6-33 (COR-RM-180): epsilon = 1 - b/d,
  and K is divided by epsilon for holes.
- **Form:** K_support = (1 + nu) [ln(x/c) - 0.811] / (4 pi); c = column radius,
  x = column spacing.
- **Worked cases:**

  ```python
  ks=lambda nu,xc:(1+nu)*(math.log(xc)-0.811)/(4*math.pi)
  ks(0.3,5.0)   # 0.0825989713  (manual prints 0.0826)
  ks(0.3,10.0)  # 0.154305541
  # holes b = 0.01 m, d = 0.1 m: eps = 0.9, K/eps = 0.0917766348
  ```
  Edge: K reaches 0 at x/c = e^0.811 = 2.2502 and is negative below that, so the
  formula has no meaning for columns that close.
- **Code:** not implemented. No test.

## COR2-04 Edge-supported PLATE with a central hole

- **Citation:** §5.2.2 Eqs 5-5 to 5-12, COR-RM-161/162; defaults K0 = 0.206,
  K1 = 0.576 for nu = 0.3 (COR-RM-163; Vol. 1 AKM0/AKM1, COR-UG-51).
- **Form:** K0 = (3 + nu)/16, K1 = (1 + 3 nu)/(3 + nu). Innermost ring:
  sigma_1 = 6 K0 W_total / (pi h^2). Ring i > 1:
  sigma_i = 6 K0 / (pi h^2) [1 - K1 (r_{i-1}/a)^2] [1 + (r0/r_{i-1})^2] W_total.
  r0 is the radius of the failed hole, a is the plate radius, and W_total is the
  load on the unfailed annulus.
- **Worked cases:**

  ```python
  K0=(3+0.3)/16; K1=(1+0.9)/3.3   # 0.20625, 0.575757576
  6*0.206/(math.pi*0.05**2)*1e6    # ring 1, h=0.05, W=1e6 -> 157372408 Pa
  # ring i, r_{i-1}=1, a=2: r0=0 -> 134710781 Pa ; r0=1 (hole to r_{i-1}) -> 269421562 Pa
  ```
  Edge: r0 = 0 reduces to Eq 5-5/5-6 (Eq 5-9). A hole reaching r_{i-1} doubles
  the stress at that edge.
- **Code:** not implemented (Unit 1 uses PLATEB). No test.
- **Manual-internal note:** below Eq 5-6 the text states K0 = 0.125 and K1 = nu.
  That holds only for nu = -1 and contradicts the 0.206/0.576 defaults given two
  paragraphs later. Use the defaults.

## COR2-05 Larson-Miller creep rupture of supporting structure (stainless steel)

- **Citation:** §5.3.3 Eqs 5-21 to 5-23, COR-RM-166; array C1604 (-7.5e3, 8.1e4,
  16.44; table COR-RM-248, where the equation labels are swapped).
- **Form:** t_R = 10^(P_LM/T - 16.44) s; P_LM = 81000 - 7500 log10(sigma_e [Pa]);
  failure when sum(dt_i / t_R(t_i)) = 1. The sum is integrated per core cell.
- **Worked cases:**

  ```python
  tR=lambda T,s:10**((81000-7500*math.log10(s))/T-16.44)
  tR(1000.,10e6)   # 1.14815362e12 s
  tR(1200.,1.94e6) # 5.77084205e11 s (the PLATEB load of COR2-02 at 1200 K)
  tR(1100.,63.5e6) # 9899.96573 s
  ```
  Edge: at the low PLATEB stresses (~2 MPa), creep is negligible next to yield
  until T is near 1650 K. So the code's omission may matter little for Unit 1;
  a test should establish this rather than assume it.
- **Code:** not implemented. The plate doc comment at `core.rs:807` says
  "no creep". No test.
- **Discrepancy:** a missing MELCOR failure path, stated in the code.

## COR2-06 COLUMN stress, Euler buckling and steel elastic modulus

- **Citation:** §5.2.4 Eqs 5-14/5-15, COR-RM-164; §5.3.2 Eqs 5-17 to 5-20,
  COR-RM-165; E(T) constants C1605 (370e9 Pa, 1700 K, 1650 K, 3; COR-RM-248).
- **Form:** sigma = W_ring / (N pi (ro^2 - ri^2)) x (M_col,0 / M_col);
  W_buckle = pi^2 E I / l^2, I = pi/4 (ro^4 - ri^4); buckling when
  sigma > pi^2 (ro^2 + ri^2) E / (4 l^2);
  E(T) = 370e9 [ (1 + (T/1650)^3)^-1 - (1 + (1700/1650)^3)^-1 ].
- **Worked cases:**

  ```python
  E=lambda T:max(0,370e9*(1/(1+(T/1650.)**3)-1/(1+(1700/1650.)**3)))
  E(300.)  # 1.9106806e11 ; E(1000.) # 1.25909477e11 ; E(1700.) # 0
  ro,ri,N,l=0.14,0.13,10,4.0
  # Ac = 0.0848230016 m^2, I = 7.7400989e-05 m^4, W_buckle(1000 K, one column) = 6011525.48 N
  # sigma_buckle = pi^2 (ro^2+ri^2) E / (4 l^2) = 708714071 Pa (1000 K)
  # W = 2e6 N -> sigma = 23578510.1 Pa ; with M/M0 = 0.8 -> 29473137.6 Pa
  ```
- **Manual-internal note:** Eq 5-19 as extracted carries a factor N in the
  denominator (pi^2 (ro^2+ri^2) E / (4 N l^2), which gives 70871407.1 Pa here).
  Deriving it from Eqs 5-15, 5-17 and 5-18 for N columns gives no N. A test
  needs the printed PDF checked first.
- **Code:** not implemented (no CRGT columns in Unit 1's model). No test.

## COR2-07 Lower-head creep rupture, 0-D default (Larson-Miller, SA533B1)

- **Citation:** §6.3 Eqs 6-23 to 6-26, COR-RM-177/178; C1601 (-4.725e3, 4.812e4,
  7.042, 0.18) and C1600(3) = 1000 Pa (table COR-RM-248). Effective dP is the
  lower-plenum-to-cavity difference plus the debris head; the temperature is the
  mass average over the load-bearing layers (COR-RM-177).
- **Form:** t_R = 10^(P_LM/T - 7.042) s, P_LM = 4.812e4 - 4.725e3 log10(sigma_e);
  eps_pl += 0.18 dt / t_R; failure at eps_pl >= 0.18;
  sigma_e = (dP + rho_d g dz_d) Ri^2 / (Ro^2 - Ri^2); the model is skipped while
  dP_eff < 1000 Pa.
- **Worked cases:**

  ```python
  tR=lambda T,s:10**((4.812e4-4.725e3*math.log10(s))/T-7.042)
  tR(1273.,10e6)    # 59777.6564 s
  tR(1273.15,10e6)  # 59586.304 s
  tR(1000.,50e6)    # 50161.0628 s
  Ri,Ro=2.4,2.62; se=(7e6+4000*9.81*1.0)*Ri**2/(Ro**2-Ri**2)  # 36713167.7 Pa
  tR(1100.,se)      # 16184.7543 s
  # one 600 s step at t_R = 6e4 s adds 0.0018 strain
  ```
  Edge: dP_eff = 999 Pa accumulates nothing. Failure after one step needs
  dt >= t_R.
- **Code:** `core.rs:514` `HeadWall::failed` (constants `params.rs:491-494`;
  dP_eff at `core.rs:1398`/`1419`/`1445` uses `(p - p_dw).max(0) + rho_bed g h`).
  **Test:** `core.rs:2394` `the_lower_head_wall_conserves_energy_and_creeps_by_larson_miller`
  checks t_R to within 10 % of 6.0e4 s at 1273 K, 10 MPa. The exact value
  59777.66 s allows a tolerance near 1e-9.
- **Discrepancy:** the code uses the arithmetic mean of the five wall-node
  temperatures (`core.rs:518`). MELCOR uses the mass-averaged temperature, and
  this mesh's face nodes carry half mass (`core.rs:483`). The two agree for a
  linear profile and differ otherwise. Example profile 1500/1200/1000/900/800 K:
  arithmetic 1080 K, mass-weighted 1062.5 K, t_R ratio 0.5896 at 10 MPa.
  The code also has no non-load-bearing insulation layers (NINSLH). That is
  irrelevant with none input.

## COR2-08 Lower-head 1-D mechanical option: E(T), sigma_Y(T), thermal strain

- **Citation:** Eq 6-27 (C1602: 2.0e11 Pa, 1800 K, 900 K, 6), COR-RM-178;
  Eq 6-28 (C1603: 4.0e8 Pa, 1800 K, 900 K, 6), COR-RM-179; Eq 6-31 (C1600(2) =
  1e-5 1/K), COR-RM-179; enabled by C1600(1) = 1 (default 0).
- **Form:** X(T) = X0 [ (1 + (T/900)^6)^-1 - (1 + (1800/900)^6)^-1 ], clamped at 0
  above 1800 K; eps_th = 1e-5 (T - T_ref).
- **Worked cases:**

  ```python
  f=lambda t:1/(1+(t/900.)**6)
  E=lambda T:max(0,2e11*(f(T)-f(1800.))); S=lambda T:max(0,4e8*(f(T)-f(1800.)))
  # 300 K: E 1.96649104e11, sY 393298209 ; 900 K: 9.69230769e10, 193846154
  # 1273.15 K: 1.9112009e10, 38224018 ; 1800 K: 0, 0
  # eps_th(1273.15 K, T_ref 560 K) = 0.0071315
  ```
- **Code:** not implemented (the 0-D default is used). No test.

## COR2-09 Debris to lower head and to penetrations; penetration conduction

- **Citation:** §6.1 Eqs 6-1, 6-2 (COR-RM-168), 6-5 (COR-RM-169). Vol. 1 COR_LHF
  (COR-UG-143/144): HDBPN = HDBLH = HMPOLH = HMPMLH = 1000 W/(m^2 K), TPFAIL =
  1273.15 K, CDISPN = 1.0.
- **Form:** q_dh = h_dh A_h (T_d - T_hs); q_dp = h_dp (dz_d/dz_1) A_p (T_d - T_p);
  q_ph = k_p A_ph (T_p - T_hs)/dz_1.
- **Worked cases:**

  ```python
  1000*10*(2000-800)        # q_dh = 12000000 W (A_h=10 m^2)
  1000*(0.2/0.5)*0.5*(2000-900)  # q_dp = 220000 W (dz_d=0.2, dz_1=0.5, A_p=0.5)
  20*0.01*(900-800)/0.5     # q_ph = 40 W
  ```
  Edge: dz_d >= dz_1 means full coverage (the code clamps cover to 1); dz_d = 0
  gives no debris heating of the penetration.
- **Code:** `core.rs:470` `HeadWall::step_with` (inner face q = h (T_in - T_0);
  penetration lumped node with cover `(height/lp_bottom_level_m).min(1)`).
  Penetration failure at TPFAIL is in `HeadWall::failed`. **Test:** the same
  head-wall test checks energy conservation and a 33 K first-second penetration
  rise, plus the TPFAIL failure flag.
- **Discrepancies (stated in the code):** no penetration-to-head conduction
  (Eq 6-5); the penetration's heat is not taken from the debris; no lateral
  conduction (Eq 6-6); the outer face is adiabatic (see COR2-10).

## COR2-10 Lower-head outer boundary: cavity atmosphere and downward-facing pool boiling

- **Citation:** Eq 6-8 and h_ATM = C1246(1) = 10 W/(m^2 K), COR-RM-171;
  Eqs 6-9, 6-10 (C1245(1-6)), COR-RM-172; 6-11 to 6-14 (C1245(7-12)), COR-RM-173;
  6-15/6-16 (area-weighted angle average), COR-RM-173; 6-17 (transition
  interpolation), COR-RM-174.
- **Form:** theta is the inclination in degrees (0 = facing down). With
  B = rho_v^0.5 h_lv [g sigma (rho_l - rho_v)]^0.25:
  q_CHF = (0.034 + 0.0037 theta^0.656) B; q_MIN = (4.8e-4 + 8.2e-4 theta^0.407) B;
  h_FLM = 0.142 k_v [h_lv rho_v g (rho_l - rho_v)/(mu_v k_v dT)]^(1/3) (sin theta)^(1/3)
  (default), or (0.055 + 0.016 theta^0.5) k_v [same]^(1/3) when C1245(7) = 1;
  transition h = (q_MIN/dT) (dT/dT_MIN)^(ln(q_CHF/q_MIN)/ln(dT_CHF/dT_MIN)).
  The segment value is f_i = int f sin(theta) dtheta / (cos theta_{i-1} - cos theta_u).
  The pool term is only counted when T_wall > T_sat.
- **Worked cases** (saturated water near 0.1 MPa, given as test inputs: rho_l
  958.35, rho_v 0.5976 kg/m^3, h_lv 2.2565e6 J/kg, sigma 0.0589 N/m, k_v 0.0251
  W/(m K), mu_v 1.227e-5 Pa s):

  ```python
  B=0.5976**0.5*2.2565e6*(9.81*0.0589*(958.35-0.5976))**0.25   # 8460577.73
  # q_CHF: 0 deg 287659.643, 30 deg 579129.258, 90 deg 886877.615 W/m^2
  # q_MIN: 0 deg 4061.07731, 30 deg 31756.1317, 90 deg 47371.3157 W/m^2
  # h_FLM(90 deg, dT=500 K): default 155.020344 ; option 2 225.750372 ; default at 0 deg = 0
  # area-averaged over 0-30 deg: q_CHF 506426.116 W/m^2 ; h_FLM(500 K) 106.03441
  # transition with qCHF=1e6, qMIN=2e4, dTCHF=30, dTMIN=300:
  #   dT=30 -> h 33333.3333 (q=1e6) ; dT=100 -> h 1293.13747 ; dT=300 -> h 66.6666667 (q=2e4)
  # atmosphere term, h_ATM=10, A=1 m^2, 600 K vs 330 K -> 2700 W
  ```
  Edge: the transition formula reproduces q_CHF and q_MIN exactly at its end
  points. The default film coefficient is 0 at theta = 0 pointwise and positive
  only after averaging (the manual says so).
- **Code:** not implemented. `HeadWall` has an adiabatic outer face; the doc
  comment at `core.rs:375-377` states this and cites MELCOR's 10 W/(m^2 K). No test.

## COR2-11 Lower-head node melt fraction

- **Citation:** §6.2, COR-RM-175/176 (unnumbered).
- **Form:** per material node, the left and right melt fractions are
  max(previous, (E - E_mlt)/E_LHF), using the left and right temperature
  nodes' enthalpies. The node fraction is 0.5 (f_left + f_right). It never
  decreases. Melted mass enters COR at E_mlt + E_LHF, and the cell's virtual
  volume grows by f m_LH / rho_SS.
- **Worked case** (each side capped at 1 here, my assumption):

  ```python
  # E_mlt=1.0e6, E_LHF=2.7e5 J/kg; E_left=1.135e6, E_right=1.27e6
  # f_left=0.5, f_right=1.0 -> f=0.75 ; dV for 1000 kg at 7900 kg/m^3 = 0.0949367089 m^3
  ```
  Edge: refreezing does not lower f (non-decreasing).
- **Code:** not implemented ("no wall melting", stated at `core.rs:378`). No test.

## COR2-12 Debris ejection: velocity, mass per step, gating thresholds

- **Citation:** §6.4 Eqs 6-34 (COR-RM-182), 6-35 (COR-RM-183); thresholds
  C1610(1) = 0.1 melt fraction or C1610(2) = 5000 kg molten (COR-RM-182); gross
  failure at TPFAIL empties the bottom cell linearly over 1 s (COR-RM-182);
  default opening 0.1 m diameter (COR-RM-180); CDISPN = 1.0 (COR-UG-144).
- **Form:** v_ej = C_d sqrt(2 dP/rho_m + 2 g dz_d), set to 0 if the radicand is
  < 0; M_ej = rho_m A_f v_ej dt; ejected fraction = min(1, M_ej / M_available).
- **Worked cases:**

  ```python
  Af=math.pi*0.1**2/4                       # 0.00785398163 m^2
  v=1.0*math.sqrt(2*1e6/8000+2*9.81*1.0)    # 16.4201096 m/s
  8000*Af*v*1.0                             # M_ej(dt=1 s) = 1031.70592 kg; fraction of 50 t = 0.0206341183
  # dP=0, dz=1 m -> 4.42944692 m/s ; dP=-1e5 Pa, dz=0.5 m -> 0 (radicand < 0)
  ```
- **Code:** not implemented. On failure the code moves all the plenum debris
  out at once (`core.rs:1323-1324`, `mod.rs:2215-2216`). The opening sizes
  `BREACH_M2` 0.01 (`params.rs:322`) and `BREACH_DIAMETER_M` 0.1
  (`params.rs:511`) are used only for gas and water flow. No test.
- **Discrepancy:** no rate-limited ejection and no C1610 gate. The doc comment
  for `BREACH_M2` cites 7.9e-3 m^2 for MELCOR's 0.1 m opening; the exact value is
  7.85398e-3 m^2. That rounding is acceptable but could be stated exactly.

## COR2-13 Ablation of the failure opening

- **Citation:** §6.4 Eqs 6-36, 6-37 (COR-RM-183) and 6-38 (COR-RM-184). The
  K_p properties come from MELCOR's reference [38]; the manual gives no numbers.
- **Form:** h_abl = max(0.023 K_p v^0.8 / D_f^0.2, 0.0292 K_p v^0.8 / dz_h^0.2),
  K_p = k (rho/mu)^0.8 Pr^(1/3); dD_f/dt = 2 h_abl (T_d - T_m,s) /
  (rho_s [c_p,s (T_m,s - T_h,avg) + h_f,s]).
- **Worked case** (my property inputs: k 3, rho 8000, mu 4.4e-3, Pr 0.7333;
  v = 16.4201 m/s from COR2-12; D_f = 0.1 m, dz_h = 0.22 m; steel rho_s 7850,
  c_p 600, h_f 2.7e5, T_m 1700 K, T_h,avg 800 K, T_d 2800 K):

  ```python
  Kp=3*(8000/4.4e-3)**0.8*0.7333**(1/3)          # 275375.309
  # h_tube 94178.9135, h_plate 102123.139 -> h_abl 102123.139 W/(m^2 K)
  # dDf/dt = 0.0353339476 m/s
  ```
  Edge: with T_d <= T_m,s there is no growth. A negative rate should be
  clamped; the manual does not say so (inference).
- **Code:** not implemented. No test.

## COR2-14 Falling-debris quench (h, particle area, decay factor, overpressure)

- **Citation:** §2.3.8 Eqs 2-112, 2-113, COR-RM-68/69. The time constant
  tau_spr = C1020(1) = 360 s (§3.2.5 text, COR-RM-147); the molten-material
  constant is C1020(2) = 60 s (§3.1.5, COR-RM-136). Vol. 1 COR_LP (COR-UG-138):
  HDBH2O = 100 W/(m^2 K) (0 disables), PPFAIL = 2.0e7 Pa, VFALL = 1.0 m/s.
- **Form:** leading edge z_d = z_csp - v_d (t - t_fail). Area A_p = 6 V_particles /
  D_hyd (equivalent spheres of DHYPD). h is held at HDBH2O until the leading
  edge reaches the head, then multiplied by
  f(t+dt) = min[1, f(t) exp(-dt/tau_spr) + V_cor/V_LP]. Below f = 0.01 the
  dryout flux of COR2-15 limits the heat transfer. dP(LP - cavity) >= PPFAIL
  fails the head totally.
- **Worked cases:**

  ```python
  # mf=1e5 kg UO2 (10412 kg/m^3), mc=3e4 kg Zr (6550), D=0.01 m:
  V=1e5/10412+3e4/6550       # 14.1844554 m^3 ; Ap=6V/D = 8510.67324 m^2
  100*8510.67324*1500        # Q at dT=1500 K, f=1: 1.27660099e9 W
  math.exp(-1000/360)        # f after 1000 s with no inflow: 0.062176524
  360*math.log(100)          # time from f=1 to f=0.01: 1657.86127 s
  min(1,0.5*math.exp(-10/360)+0.2)  # one 10 s step, f0=0.5, Vcor/VLP=0.2: 0.686302239
  # z_d at t-t_fail=2 s from z_csp=4 m: 2 m at 1 m/s (UG default), -6 m at 5 m/s (RM text)
  ```
- **Code:** `core.rs:1319` (decay every step), `core.rs:1328-1350` (h f A_p dT,
  then Lipinski once f < 0.01), `core.rs:892`/`1936` (f += dV/V on arrival,
  capped at 1). Constants are at `params.rs:515-524`. **Test:** no unit test of the
  decay factor or the quench rate. `mod.rs:3482` `the_first_relocation_does_not_quench_the_core`
  is plant-level.
- **Discrepancies:**
  1. MELCOR holds f = 1 (no decay) until the debris front reaches the head.
     The code decays from arrival because it has no fall time (VFALL is not
     modelled). The two are equivalent only for an instantaneous fall.
  2. The PPFAIL overpressure failure (2e7 Pa) is not implemented. It is likely
     moot for Unit 1, which never approached 20 MPa.
  3. Manual-internal: the RM text gives a 5 m/s fall-speed default; the UG
     says 1.0 m/s (changed from 5). The coefficient table pairs §3.1.5 with
     C1020(1) and §3.2.5 with C1020(2), the reverse of the section texts. The
     code's `QUENCH_TAU_S` = 360 s follows the §3.2.5 text and says so.

## COR2-15 Lipinski 0-D dryout heat flux

- **Citation:** §2.3.8 Eqs 2-114, 2-115, COR-RM-69/70; C1244 = (0.756, 0.089 m,
  0.15), table COR-RM-246. The table cites the equation as (2-112), a stale label.
- **Form:** q_d = 0.756 h_lv sqrt( rho_v (rho_l - rho_v) g d eps^3 (1 + lambda_c/L) /
  ((1 - eps) [1 + (rho_v/rho_l)^(1/4)]^4) ), with eps = max(eps, 0.15). The
  manual describes the capillary head as nominally 0.089 m for 0.5 mm particles,
  adjusted for particle size, so lambda_c = 0.089 (5e-4/d). The flux applies
  over the cell cross-section.
- **Worked cases** (properties as inputs; rho_l 915.29, rho_v 2.6680 kg/m^3,
  h_lv 2.1074e6 J/kg, roughly saturated at 0.5 MPa):

  ```python
  lip(915.29,2.6680,2.1074e6,d=0.01,eps=0.4,L=1.0)    # 5306971.09 W/m^2
  lip(915.0,2.67,2.108e6,0.01,0.4,1.0)                 # 5309087.99 (the code test's props)
  lip(915.29,2.6680,2.1074e6,0.0005,0.4,0.1)           # 1627790.33
  lip(...,0.01,0.10,1.0) == lip(...,0.01,0.15,1.0)     # 1023905.29 (porosity floor)
  ```
  For reference, Eq 2-115 itself (sigma 0.0589, theta 0, eps 0.4, 1 atm) gives
  lambda_c = 0.112840641 m at d = 0.5 mm and 0.00564203204 m at 1 cm, against
  0.00445 m from the 1/d scaling.
- **Code:** `core.rs:1119` `lipinski_dryout_flux` (1/d scaling, floor 0.15).
  **Test:** `core.rs:2664` `lipinski_dryout_matches_a_hand_value` (5.31e6 within
  2 %, sqrt(d) trend, floor).
- **Discrepancy:** none. The 1/d-only scaling matches the manual's description.
  The test tolerance (2 %) could tighten if the steam-table values at 0.5 MPa
  were pinned.

## COR2-16 Molten-pool Nusselt correlations (ACOPO / Bonnet) and the stratified-pool interface

- **Citation:** §2.4.2 Eqs 2-119, 2-120 (COR-RM-72), Table 2-4 (C1280,
  COR-RM-80), Eqs 2-133 to 2-137 (COR-RM-79).
- **Form:** Nu = A Ra_i^n Pr^m. Ra_i = g beta Q H^5 Pr / (lambda nu^2), Q
  volumetric (Q/lambda is unit-consistent in kW or W). Rows of A, n, m:
  oxide-radial 0.3, 0.22, 0 (ACOPO); oxide-interface and oxide-atmosphere 0.381,
  0.234, 0 (Bonnet); metal-lower 0.069, 0.333, 0.074 (Globe-Dropkin);
  metal-radial and metal-upper 0.3, 0.22, 0. h = Nu lambda / H. Between pools,
  h_12 = h_1 h_2 / (h_1 + h_2).
- **Worked cases** (k 3, mu 4.4e-3, beta 1.05e-4, cp 500, rho 8000):

  ```python
  # Q=1e6 W/m^3, H=0.5: Ra=2.60113636e13, Nu=268.198992, h=1609.19395
  # Q=1e6, H=1.0: Ra=8.32363636e14, h=1724.69137 ; Q=2e6, H=0.5: h=1874.28224
  # Bonnet row at Q=1e6, H=0.5: Nu=524.895858, h=3149.37515
  # h1=1609, h2=500 -> h12=381.460408
  ```
- **Code:** `core.rs:421` `pool_h` (oxide-radial row only, one mean h);
  constants `params.rs:436-455`. **Test:** `core.rs:2649`
  `the_plenum_pool_convects_as_melcor_says` (1609.19 to 1e-4 plus the
  scaling laws).
- **Discrepancy:** none in the row used. Not implemented: the other rows, the
  angular profile, the Ra time constant (Eq 2-138), stratification, and the
  Stefan crust (all stated in the code).

## COR2-17 Simple B4C-steam reaction model, with the 2 % cap

- **Citation:** §2.5.9 Eqs 2-197 to 2-208, COR-RM-105/106 (rate constants C1006:
  1.662e5 1/s, 2.26472e4 K, COR-RM-243). Gating is in §2.5 (COR-RM-89/90): the
  reaction starts only after the blade sheath fails, which is when intact steel
  falls below C1005(3) = 0.9 of its initial mass. It then needs B4C above
  C1005(4) = 1500 K. At most C1005(2) = 0.02 of the initial B4C reacts. The
  reaction energy goes to CVH (the gas).
- **Form:** y_CO/CO2 = (P_H2/P_H2O) exp(-3605/T + 3.427);
  y_CO/CH4 = P_H2O/P_H2^3 exp(-27350/T + 30.50) (atm);
  x_CO = 1/(1 + 1/y_CO/CO2 + 1/y_CO/CH4), x_CO2 = x_CO/y_CO/CO2, x_CH4 = 1 - x_CO - x_CO2.
  Per mole B4C: steam 7, 8, 6 and H2 7, 8, 4. Q = 8.238e8 - 58380 T,
  8.674e8 - 67060 T, 1.056e8 - 61430 T (J/kmol). Rate d(M/M0)/dt =
  (9.973e6/60) exp(-22647.2/T).
- **Worked cases** (outputs: y_CO2, y_CH4, x_CO, x_CO2, x_CH4, steam/mol, H2/mol,
  Q J/kmol):

  ```python
  b4c(1600,1,1)      # 3.23450208, 664141.163, 0.763843847, 0.236155003, 1.15012273e-06, 7.23615385, 7.23615155, 737407806
  b4c(1600,0.1,1.0)  # 0.323450208, 664141163, 0.244399227, 0.755600772, 3.68e-10, 7.75560077, 7.75560077, 752842410
  b4c(1200,1.0,0.01) # 152.627989, 22.2682778, 0.951059606, 0.00623122672, 0.0427091674, 6.96352206, 6.87810372, 723120737
  rate(1200)=0.00105774158/s (2 % in 18.9082 s); rate(1500)=0.046093382; rate(1600)=0.118427708; rate(2000)=2.00861903
  ```
  Edges: the cap stops the reaction at 0.02 of M0. The 1200 K case shows CH4
  becoming non-negligible in hydrogen-rich gas. C1006(1) = 1.662e5 versus
  9.973e6/60 = 166216.667 differs by 1.0e-4 relative (table rounding).
- **Code:** `core.rs:934` `b4c_reaction`; rate and cap `core.rs:1586-1612`;
  constants `params.rs:459-468`. **Tests:** `core.rs:2489` `the_b4c_reacts_as_melcor_says`
  (1600 K, 1 atm each; 2e-4 abs on x, 2e-3 relative on Q, 1e-2 on rate);
  `mod.rs:2729` `the_blades_b4c_reacts_up_to_melcors_cap` (plant-level cap bound).
- **Discrepancies:**
  1. **No 1500 K threshold** (C1005(4)). The code reacts B4C at any cladding
     temperature with steam present, so at 1200 K the 2 % cap is reached in
     about 19 s. MELCOR would not react it at all below 1500 K.
  2. **No sheath-failure gate** (C1005(3) = 0.9). The code reacts B4C while the
     blade is intact (`!node.blade_gone`). MELCOR starts only after intact
     blade steel falls below 0.9 of its initial mass (by oxidation or melting).
  3. The energy goes to the cladding rather than the gas (stated in the code).

## COR2-18 Candling: secondary-material transport, oxide-shell holdup, breakthrough step

- **Citation:** §3.1.4 Eqs 3-14, 3-15 (COR-RM-135); Vol. 1 COR_CMT
  (COR-UG-40/41): MTUOZR = 1, FUOZR = 0.2, MTZXZR = 1, FZXZR = 0.0, MTSXSS = 2,
  FSXSS = 1.0, MTCPSS = 2, FCPSS = 0.0. The transport is inactive when the
  eutectics model is on. §3.1.3 (COR-RM-134): melt is held while oxide thickness
  exceeds dr_hold, T < T_breach, and no candling has occurred yet. C1131 =
  (1e-5 m, 2400 K, 0.001 m, 1700 K, 1e-5 m, 2100 K) and C1132 = (2500, 3100 K),
  table COR-RM-244. Eq 3-13: dt_contact = min(dt, M_m dz / (Gamma_max A_surf));
  C1141 = (1 s, 1 kg/(m s)) (COR-RM-134, table COR-RM-245).
- **Form:** mechanism 1, dM_s = F1 dM_m; mechanism 2, dM_s = F2 (M_s,total / M_m,total) dM_m.
- **Worked cases:**

  ```python
  0.2*100        # UO2 carried with 100 kg molten Zr: 20 kg
  1*30/100*10    # mechanism 2, F2=1, Ms=30, Mtot=100, dMm=10: 3 kg
  min(5.0,50*0.3/(1*20))  # dt_contact with Mm=50 kg, dz=0.3 m, A=20 m^2, dt=5 s: 0.75 s
  ```
- **Code:** `core.rs:1789-1830` (`candling`, `uo2_candling`); constants
  `CANDLING_UO2_FRACTION` 0.2 (`params.rs:432`), `CLAD_BREACH_K` 2400 and
  `ROD_COLLAPSE_K/MAX_K` 2500/3100 (`params.rs:540-548`). **Tests:**
  `core.rs:2585` `zircaloy_is_conserved_through_candling` and `core.rs:2618`
  `uo2_is_conserved_through_its_candling`. Both are plant-level conservation
  checks. No unit test of the 0.2 fraction on a single node.
- **Discrepancy:** the code candles a node only while T_clad >= 2400 K, in every
  step. MELCOR holds melt only if the oxide is thicker than dr_hold (I read the
  1e-5 m / 2400 K pair as Zircaloy cladding; the table does not name the pairs).
  It holds only until the first breach, after which candling continues below
  T_breach. Thin-oxide Zr therefore candles at melt (about 2098 K) in MELCOR but
  not in the code. The code also relocates within one step and refreezes whole
  (stated).

## COR2-19 Ag-In-Cd control-rod release (PWR; reference only)

- **Citation:** §4.2 Eqs 4-1 to 4-5, COR-RM-156/157.
- **Form:** with r = A2/A1, V_r = sqrt(2 g h / (1 + K_c - r^2)); K_c = 0.32 (1 - r)
  for r < 0.4712 and K_c = r above; mdot = rho_AIC A2 V_r. Head:
  sqrt(h) = sqrt(h0) - gamma t/2, gamma = r sqrt(2 g/(1 + K_c - r^2)), which is
  my reading of the extracted Eq 4-2 (it follows from dh/dt = -r V_r). h = 0
  after t_empty = 2 sqrt(h0)/gamma.
- **Worked cases:**

  ```python
  # r=0.01: Kc=0.3168, Vr(h=1)=3.86016691 m/s, gamma=0.0386016691; h0=2 m: h(50 s)=0.201755337 m, t_empty=73.2721457 s
  # mdot with rho=10170, A2=1e-4, h=1: 3.92578975 kg/s
  # r=0.5: Kc=0.5, Vr(h=1)=3.96181776
  ```
  Edge: K_c jumps from 0.169216 to 0.4712 at r = 0.4712 (manual-internal
  discontinuity).
- **Code:** not implemented (BWR, B4C blades). No test.

## COR2-20 Modified Booth fractional release (Gelbard approximation; HTGR, reference only)

- **Citation:** §9.4.2 Eqs 9-19, 9-20, COR-RM-225.
- **Form:** F = 1.0006964 (6 sqrt(D't/pi) - 3 D't) for D't < 0.155;
  F = 1 - (6/pi^2) exp(-pi^2 D't) otherwise. D' = D/a^2.
- **Worked cases** (with the exact series 1 - 6/pi^2 sum exp(-n^2 pi^2 D't)/n^2
  for comparison):

  ```python
  # D't=1e-4: 0.0335747402 (exact 0.033551375) ; 0.01: 0.308728599 (0.30851375)
  # 0.1: 0.771011028 (0.770478738) ; 0.3: 0.968525627 (0.968524535) ; 1.0: 0.999968556
  # switch: left 0.868334254, right 0.868334279 at 0.155 (continuous to 3e-8)
  ```
  Manual-internal note: the condition is printed as 0.155 on one branch and
  0.1551 on the other.
- **Code:** not implemented in sembawang or offbeat. No test.

## COR2-21 Heat-pipe condenser heat-exchanger limit (reference only)

- **Citation:** §8.5 Eqs 8-23 to 8-26, COR-RM-201/202.
- **Form:** beta = mdot c_p / (h_x,ef A_x); T_x,o = (T_wf + beta T_x,in)/(1 + beta);
  Q_x = mdot c_p (T_x,o - T_x,in); the bound as h -> inf is
  Q_lim = mdot c_p (T_wf - T_x,in).
- **Worked case:** mdot 2 kg/s, c_p 1200, hA 5000 W/K, T_wf 900 K, T_in 600 K:
  beta 0.48, T_x,o 802.702703 K, Q 486486.486 W, Q_lim 720000 W.
- **Code:** not implemented. No test.

## COR2-22 Small closures listed for completeness (not implemented)

- **Radionuclide partition factor**, Eq 3-33, COR-RM-153: f_i is the ratio of
  RN_i concentration in the metallic pool to that in the oxide pool. The
  default keeps all RN in the oxide.
- **Point-kinetics Doppler**, Eq 7-4, COR-RM-186: rho_D = chi_D ln(T_f/T_f0).
  The manual lists chi_D's unit as 1/s, which looks like a typo for a
  dimensionless coefficient.
- **Core-quench constants**, C1260 (table COR-RM-246): 600 K, 40 K (immediate
  quench below this excess), 1.5e5 W/(m^2 K), and 125 W/(m^2 K) (submerged but
  unquenched). The §2.2.2/2.2.3 equations sit before this range, and `rg C1260`
  finds nothing in sembawang or offbeat, so it is not implemented.

---

## Summary

- **22 items** (COR2-01 to COR2-22; COR2-22 bundles three small closures).
- **Implemented and tested against hand values:** 01 (yield), 02 (PLATEB
  stress), 07 (lower-head Larson-Miller), 15 (Lipinski), 16 (ACOPO), 17 (B4C
  equilibrium and rate). Most of these tests use loose tolerances (1e-2 to
  10 %); the values here allow pins near round-off.
- **Implemented but no unit-level test:** 09 (debris and penetration heat; only
  energy and a one-second rise are checked), 14 (quench decay factor and quench
  rate), 18 (candling fraction; plant-level conservation only).
- **Not implemented, no test:** 03, 04, 05, 06, 08, 10, 11, 12, 13, 19, 20, 21, 22.
- **Code-against-manual discrepancies:**
  1. B4C has no 1500 K threshold (C1005(4)) and no sheath-failure gate
     (C1005(3)), so it reacts on intact blades at any temperature (COR2-17).
  2. Lower-head creep uses the arithmetic node mean, not the mass-averaged
     temperature (COR2-07).
  3. Candling requires 2400 K every step, so there is no thin-oxide candling at
     melt and no continued candling after breach (COR2-18).
  4. The quench decay starts on arrival rather than when the debris reaches the
     head; PPFAIL is missing (COR2-14).
  5. The core plate uses the stress-based PLATEB rule, while MELCOR's default is
     TSFAIL at 1273.15 K. Its K is a literal and it has no SS creep (COR2-02,
     COR2-05).
  6. Missing ejection-rate and C1610 gating; the `BREACH_M2` doc rounds 7.854e-3
     to 7.9e-3 m^2 (COR2-12).
- **Manual-internal inconsistencies:** Eq 5-6's K0/K1 sentence; Eq 5-19's N;
  table labels for C1604, C1606, C1020 and C1244; VFALL 5 versus 1 m/s;
  Gelbard 0.155 versus 0.1551; Eq 4-4's discontinuity.
