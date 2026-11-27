# AI V&V items: MELCOR 2.2 CVH/FL, DCH, HS (Vol. 2 lines 17693-32045)

AI draft, not human-reviewed. Sources: MELCOR 2.2 Vol. 2 Reference Manual
(SAND2023-10994 O, cited as `CVH/FL-RM-nn`, `DCH-RM-nn`, `HS-RM-nn`,
`COR-RM-nn`) and Vol. 1 Users' Guide (SAND2017-0455 O, cited as `CVH-UG-nn`
etc.). Equations restated in my own symbols; no manual prose copied. Every
expected value below was computed with `python3 -I`; the scripts are in
calc/py_vv1.py` … `vv5.py`.
Property values used as inputs (h_fg, v_g, rho, mu, k …) are stated inputs,
not computed from steam tables, so each test is self-contained.

Not covered from the assigned range: CVT, HS conduction finite differences,
HS radiation (§2.6.2), film tracking, CVH momentum/solution strategy. They
are less unit-testable or not used by our code.

---

## SPECIAL QUESTION: MELCOR's pool swell vs `swollen_level_from_sources`

> **RESOLVED 2026-11-26:** the plant now uses `swollen_level_by_volume`,
> MELCOR's per-control-volume form, so points 1 to 4 below no longer apply.
> Points 5 to 7 (flashing, the quench-front split, dynamics) remain and are
> stated in its doc. Test: `bwr_mark1::tests::cvh_02_the_pool_swells_per_control_volume_as_melcor_does`.
> The analysis below is kept as the record of the comparison.

**(a) Vapour sources that swell a pool.** The pool's vapour content is a
state derived from the pool's enthalpy, not from a heat rate: the vapour mass
in the pool is `M_v = M_P (h_P - h_l)/(h_v - h_l)` (Eq 5-30, CVH/FL-RM-48),
capped at `M_P`. So anything that raises the pool's specific enthalpy above
saturated liquid, or lowers saturation enthalpy, makes swell: heat deposited
in the pool by any package (HS pool convection/boiling HS-RM-56/57 Eq 2-123;
COR convection to the pool), and **flashing on depressurisation**
(CVH/FL-RM-47 §5.1.3, opening paragraph; CVH-UG-9 §1.3.3). Only a fraction
`f = 1/(1 + 2 v0 Δt/Z_P)` (Eq 5-29) stays per step; the rest moves to the
atmosphere at h_v; then the mean void is forced to ≤ 0.40 (C4407(11),
CVH-UG-52). Swell is computed only for non-equilibrium volumes; equilibrium
volumes put all vapour in the atmosphere (CVH/FL-RM-41/42).

Not swelling: gas/vapour **entering a volume from a flow path below its
pool surface** (the bubble-physics model, §6.1): those bubbles "are not
considered to reside in the pool and do not contribute to pool swelling"
(CVH/FL-RM-61); without the option the material goes straight to the
acceptor's atmosphere (CVH/FL-RM-61; CVH-UG-50, C4405).

**(b) Quench vapour.** COR's quench-front heat (COR Eq 2-49, plus the
analogous fuel-pellet term) is deposited directly at the liquid interface, so
its vapour goes to the atmosphere and **does not contribute to level swell**
(COR-RM-47, §2.2.1, end of the paragraph after Eq 2-59). C1260 holds the
quench-model constants: ΔT_Q,max = 600 K (C1260(1)), immediate-quench ΔT
40 K (C1260(2)), h* = 1.5e5 W/m²K (C1260(3)), unquenched-submerged h =
125 W/m²K (C1260(4)) (COR-UG-126 table; COR-UG-194/195 §1260; COR-RM-45, -47,
-49). Heat to the pool from the *unquenched submerged* and the *quenched*
parts of a component goes to the pool normally (COR-RM-45 Eq 2-44, and
COR-RM-44 "transferred to the appropriate portion of the fluid"), so it does
swell. **Falling-debris quench** (COR §2.3.8, COR-RM-67/68, h on COR_LP,
descent 5 m/s, decay factor Eq 2-113): the manual does not say where its
vapour goes; read by default as ordinary heat to the lower-plenum pool
(hence swell in the lower-plenum CV). Not stated explicitly; flagged.

**(c) Per control volume; BWR nodalisation.** Yes. The bubble-rise model is
per CV with `Z_P` and `V_P` that CV's pool depth and swollen volume (Eqs
5-26 to 5-29), and the implicit bubble-separation linearisation is per
volume `i` (§4.2.1, Eqs 4-26 to 4-36, CVH/FL-RM-28 to -31). COR-RM-48 states
bubble separation is handled separately in each CV, so stacked CVs can each
have a pool and an atmosphere and the core can show more than one level;
COR then "bridges" pools across CV boundaries (C1270). Different CVs are
ordinarily used above and below the core plate (COR-RM-66 §2.3.7). The UG
example nodalisation has the BWR lower plenum (CV100), downcomer (CV110,
jet pumps between them) and core channel (CV120) as separate CVs, with
FL_BLK on the LP-to-channel path (FL-UG-80). COR may also use a separate
bypass CV (COR-RM-14/15).

**Does `swollen_level_from_sources` (crates/sembawang/src/bwr_mark1/mod.rs:1480)
match MELCOR?** Partly. It now agrees with MELCOR on the point the defect
was about: vapour raised in the core does not swell the lower plenum's water,
and vapour raised in one region does not void water below it. It differs in
these ways:

1. **Lower-plenum vapour.** In ours, `lower_plenum_w` is a source at z = 0
   that voids every slab above it, core included. In a MELCOR nodalisation
   with a separate LP CV, LP vapour separates in the LP CV. What reaches the
   core CV as atmosphere through the LP-to-core path enters below the core
   pool and **does not swell it** (§6.1). Only the residual void carried in
   two-phase *pool* flow reaches the core pool. So ours probably
   over-swells the core for LP sources. That is the same direction as the
   2026-11-26 defect, though much smaller.
2. **Location within one CV.** MELCOR's void depends only on the CV's total
   vapour generation. Eq 5-26 assumes generation spread uniformly over the
   swollen depth whatever the real source height. Ours integrates the local
   void `J(z)/v0`. For a source at the bottom of a uniform column, ours
   gives twice MELCOR's mean void (item CVH-02c: 3.333 m vs 2.667 m).
3. **Uniform sources below the collapsed level.** Even for uniform row
   sources, ours exceeds MELCOR by a systematic `+1.25 %` of level at mean
   void 0.1 (2.250 m vs 2.222 m on a 2 m column). Ours puts no generation
   above the collapsed level but full flux through the swollen part above
   it. The doc comment of `swollen_level_from_sources` and the test
   `the_swell_voids_only_the_water_above_each_source` (mod.rs:3415-3455)
   blame the residual on "the 1 cm grid and the half-row offset". **That is
   a doc claim the arithmetic contradicts**, and it should be corrected
   (not edited here: read-only task).
4. **Cap.** MELCOR caps the CV-mean void at 0.40. Ours caps the local void,
   which is stated in the doc comment.
5. **Flashing.** MELCOR swells on depressurisation flashing (Eq 5-30 from
   enthalpy). Ours derives swell only from heat rates `q / h_fg`, so a
   depressurisation (SRV/ADS) makes no swell. Not implemented.
6. **Quench-front vapour.** MELCOR excludes it from swell (COR-RM-47). Ours
   has no quench-front split, so the boiling of a hot node that becomes
   covered (`q_pool_rows_w`, core.rs:1663-1666) and the plate's debris heat
   (`q_pool_plate_w`, core.rs:1475-1485) all count as swell vapour. The plate
   source is the one that drove the 2026-11-26 defect. Whether MELCOR
   applies the quench model to particulate debris on the plate is not
   stated in what I read, so this is a likely difference, not a confirmed
   one.
7. **Dynamics.** MELCOR's void is a lagged state (Eq 5-29 per step, a
   time constant of about `Z_P/(2 v0)`, plus the implicit linearisation of
   §4.2.1). Ours is algebraic from the last step's heat. At steady state the
   two agree (CVH-02a shows the MELCOR scheme converges to Eq 5-26 for any
   Δt).

---

## CVH items

### CVH-01: bubble retention fraction per step
- Citation: CVH/FL §5.1.3, Eq 5-29, CVH/FL-RM-48; `v0` = C4407(1) = 0.3 m/s
  (CVH-UG-51).
- Equation: `f = 1 / (1 + 2 v0 Δt / Z_P)`; `Z_P` pool depth (m), `Δt` (s).
- Cases (`vv1.py`): v0 = 0.3. Δt = 0.1 s, Z = 2 m → **0.970873786**; Δt = 1 s,
  Z = 2 m → **0.769230769**; Δt = 10 s, Z = 0.5 m → **0.0769230769**. Edge:
  Δt → 0 gives f → 1 (1e-6 s: 0.9999997000).
- Ours: not implemented as a per-step retention (algebraic level only).
- Discrepancy: see special question, point 7.

### CVH-02: mean pool void, swollen level, 0.40 cap
- Citation: Eqs 5-26 to 5-28 (CVH/FL-RM-47/48); cap 0.40 = C4407(11)
  (CVH/FL-RM-48; CVH-UG-52, also CVH-UG-9 §1.3.3).
- Equation: with `G` the vapour volume generated per second (m³/s), the
  steady mean void is `α = G Z_P / (2 v0 V_P)`. For a uniform column of area
  `A` that is `α = (G/A)/(2 v0)`, and the swollen depth is
  `Z = Z_collapsed / (1 - min(α, 0.4))`.
- Cases: A = 20 m², collapsed depth 2 m.
  - (a) G = 3 m³/s → α = **0.25**, Z = **2.666666667 m**. Iterating the
    per-step scheme (CVH-01 plus Eq 5-28) at Δt = 0.01, 0.1, 1 s converges to
    the same 0.25 / 2.6666667 m: timestep-independent at steady state.
  - (b) Heat form: q = 50 MW, h_fg = 1.5052e6 J/kg, v_g = 0.027378 m³/kg
    (7 MPa, inputs) → G = **0.909447250 m³/s**, α = **0.0757872708**,
    Z = **2.164003954 m**.
  - Edge (cap): G = 10 → α_raw = 0.8333 → capped 0.4 → Z = **3.333333333 m**.
  - (c) Source-location contrast, ported from `swollen_level_from_sources`
    (`vv2.py`). Source at the bottom: G = 1.5 → ours **2.666667** vs MELCOR
    **2.285714**; G = 3 → ours **3.333333** vs MELCOR **2.666667**. Uniform
    over the collapsed 2 m (200 rows): G = 1.5 → ours 2.334933 vs MELCOR
    2.285714; G = 3 → ours 2.803167 vs 2.666667. At mean void 0.1 (6 rows):
    ours = 1.125 H, MELCOR = H/0.9, ratio **1.0125** exactly.
- Ours: `swollen_level` mod.rs:1437 (single-CV form; matches Eq 5-26 for
  uniform area); `swollen_level_from_sources` mod.rs:1480;
  `tampines::control_volume::level::swollen_level_from_vapour_flow`
  (level.rs:50), pinned bit-identical by
  `crates/sembawang/tests/swollen_level_vs_tampines.rs`. Tests:
  `the_swollen_level_follows_bubble_rise_and_its_cap` (mod.rs:3399) checks
  J/(2v0) and the 0.4 cap; `the_swell_voids_only_the_water_above_each_source`
  (mod.rs:3430) has a 3 cm tolerance.
- Discrepancies: `swollen_level` uses the local area at the collapsed level
  where MELCOR uses the swollen mean area V_P/Z_P (they are equal only for a
  uniform section). For the rest, see special question points 1 to 6.

### CVH-03: pool vapour mass from enthalpy
- Citation: Eq 5-30, CVH/FL-RM-48.
- Equation: `M_v = M_P (h_P - h_l)/(h_v - h_l)`, capped at M_P; h_l, h_v are
  saturated at the CV pressure. The manual gives no lower bound; a clamp at 0
  for subcooled pools is my reading.
- Cases: M_P = 1e5 kg, h_l = 1.2674e6, h_v = 2.7726e6 J/kg: h_P = 1.30e6 →
  **2165.825140 kg**; h_P = h_l → 0; edge h_P = 3.0e6 → capped at **1e5 kg**;
  h_P = 1.25e6 → raw −1155.99 (clamp to 0, assumed).
- Ours: not implemented (swell comes from q, not from enthalpy).

### CVH-04: pool-surface mass flux with non-condensables
- Citation: Eqs 5-1, 5-2, 5-5, 5-6, CVH/FL-RM-42/43.
- Equations: evaporation-positive flux `m'' = C ln[(P_A - P_wA)/(P_A - P_wl)]`
  (kg/m²s); `C = ρ_v D h_A/k (Sc/Pr)^(1/3)` with ρ_v the saturated-vapour
  density at total pressure; the inverse is
  `P_wl = P_A - (P_A - P_wA) exp(-m''/C)`, `T_l = T_sat(P_wl)`.
- Cases: P_A = 2e5, P_wA = 3e4 Pa, C = 0.01 kg/m²s: m'' = +0.002 →
  P_wl = **60815.77198 Pa**; m'' = −0.0005 → **21283.91362 Pa**; the round
  trip returns m'' exactly. C for ρ_v = 1.1291, D = 3e-5, h = 5, k = 0.03,
  Sc = 0.6, Pr = 0.7 → **5.36274091e-3**. Edge: a large condensation flux
  (−0.002 at C = 0.01) gives a negative P_wl, so the iteration must bound it.
- Ours: `pool_surface::exchange` (crates/sembawang/src/bwr_mark1/pool_surface.rs:146,
  flux at :177, C at :176). Test
  `the_surface_evaporates_and_condenses_with_the_right_signs` (signs and
  energy only, no manual numbers).
- Discrepancy: ρ_v is ideal-gas `P M/(R T_sat)` (:175), not the saturated
  steam density; that is a few % low at high pressure. C uses the convective
  h without radiation, which agrees with Eq 5-2.

### CVH-05: interface heat-transfer coefficients
- Citation: Eqs 5-18 to 5-25, CVH/FL-RM-45/46; constants C4407(2..10)
  (CVH-UG-51/52): TRAC 0.02; atmosphere turbulent 0.14 with exponent 1/3,
  laminar 0.54 with exponent 1/4; pool 0.27 with exponent 1/4 for both.
- Equations: `h = max(h_forced, h_free, k/L)`; `h_forced = 0.02 ρ c_p v`;
  `h_free,A = max(0.54 (GrPr)^¼, 0.14 (GrPr)^⅓) k/X`;
  `h_free,P = 0.27 (GrPr)^¼ k/X`; `X = min(D_s, L)`;
  `Gr = g β |ΔT| X³ (ρ/μ)²`; with radiation,
  `h* = h + σ (T_φ² + T_l²)(T_φ + T_l)` (unit emissivity and view factor).
- Cases (`vv2.py`, g = 9.80665):
  - Atmosphere: ρ = 1, μ = 2e-5, k = 0.03, c_p = 1010, β = 1/350, ΔT = 20 K,
    X = 5 m → GrPr = 1.17913292e11, laminar 1.89861 / turbulent
    **4.11907981 W/m²K**. h_forced(v = 0.5) = **10.1**. Radiation part
    (350/330 K) = **8.92244690**. h* = **19.02244690**.
  - Laminar/turbulent crossover at GrPr = (0.54/0.14)^12 = **1.08439795e7**.
  - Pool: ρ = 967, μ = 3.26e-4, k = 0.674, c_p = 4200, β = 7e-4, ΔT = 10, X = 2
    → GrPr = 9.81598e12, h_free = **161.056044**; k/L = 0.337.
- Ours: `h_free` pool_surface.rs:124 (natural only, constants match
  C4407). Forced convection is omitted (stated in the module doc).
- Discrepancy: none in the constants. Eq 5-21 forced convection is not
  implemented (stated).

### CVH-06: surface-exchange conservation and limiters
- Citation: Eqs 5-7, 5-12 to 5-17, CVH/FL-RM-44/45; 90 % condensation cap in
  C4407 (C4407(12) per CVH-UG-52).
- Statement: `ΔM_P = -ṁΔt`, `ΔE_P = -(ṁ h_f + Q_PS)Δt`, `ΔM_wA = ṁΔt`,
  `ΔE_A = -(ṁ h_g - Q_AS)Δt`. Their sum is `ΔE_P + ΔE_A = Q_RS Δt`, an exact
  identity when `ṁ = (Q_PS + Q_AS + Q_RS)/(h_g - h_f)`. Sensible heat is
  limited to `M c_p (T_φ - T_l)/Δt`; condensation to 0.9 of the atmosphere's
  vapour per step.
- Test: any inputs; assert `ΔE_P + ΔE_A - Q_RS Δt = 0` to round-off, and
  that the 90 % cap scales ṁ, Q_PS and Q_AS by one factor.
- Ours: pool_surface.rs:194-216 (Eq 5-17 and the 0.9 cap). The existing test
  checks `ṁ h_fg = Q_P + Q_A`.

### CVH-07: bubble-physics efficiencies at a submerged vent
- Citation: Eqs 6-1 to 6-3, CVH/FL-RM-61/62; C4405(1..5) = 0.01 m, 1.0,
  0.1 K, 5.0 K, 0.99 RH (CVH-UG-50).
- Equations: `ε_z = clamp((z_P - z_J - 0.01)/(1.0 h), 0, 1)`,
  `ε_T = clamp((T_sat(P_J) - T_P - 0.1)/5.0, 0, 1)`, `ε = ε_z ε_T`; P_J
  includes the static head.
- Cases: depth 1.2, h 0.5, T_sat 393.36, T_P 390.76 → (1, 0.5, **0.5**);
  depth 0.26 → ε_z **0.5**; depth 0.005 → 0; subcooling 0.06 K → ε_T 0.
- Ours: `bubble_efficiency` pool_surface.rs:229. Test
  `the_bubble_efficiency_follows_the_subcooling` (:267) uses these switch
  points. Matches.

### CVH-08: gas critical flux
- Citation: Eq 6-13, CVH/FL-RM-64; threshold for the choking test 20 m/s
  C4402(1) (CVH-UG-48); discharge coefficient default 1.0 (FL_USL).
- Equation: `G_c = ρ_d c_s,d (2/(γ+1))^((γ+1)/(2(γ-1)))`, with the multiplier
  evaluated at γ = 1.4 → **0.578703704**. The manual says it stays within 5 %
  of 0.58 over 1.1 ≤ γ ≤ 1.8: computed 0.599118 (+3.30 %) at 1.1, 0.554978
  (−4.31 %) at 1.8. Confirmed.
- Cases: N₂, 0.8 MPa, 450 K, γ 1.4: ρ = **5.98975572**, c_s =
  **432.418689 m/s**, G = **1498.89023 kg/m²s**, ṁ(Cd 0.6, 5e-4 m²) =
  **0.449667069 kg/s**. Ideal-gas steam at 7 MPa, 559 K, γ = 4.15/3.15: G =
  **9154.20397** (multiplier at 1.4) or 9239.08344 (own γ).
- Ours: `tampines::flow_path::critical_moles` (crates/tampines/src/flow_path/mod.rs:84),
  used for the wetwell vent and the drywell flange. Test
  `a_leak_to_the_atmosphere_chokes` (containment.rs:2213; 2e-3 tolerance).
  Matches.
- Discrepancy: the vessel's SRV and leak flux (mod.rs:1938, `ChokedTable`
  mod.rs:1381) use a real-steam isentropic nozzle from the steam tables,
  not Eq 6-13. At 7 MPa, Eq 6-13 gives about 9.15e3 kg/m²s, and Napier's
  rule gives about 1.02e4, so the two routes differ by roughly 10 %
  (estimate; ours not run). `mixture_flux_factor` = √(M_mix/M_H2O) is
  consistent with Eq 6-13 at fixed γ and T.

### CVH-09: two-phase critical flux and imposed velocity
- Citation: Eqs 6-15, 6-16, CVH/FL-RM-65.
- Equations: `(αρ_A + (1-α)ρ_P)/G_2φ = αρ_A/G_A + (1-α)ρ_P/G_P`;
  `v_C = G_c/(α ρ_A + (1-α) ρ_P)`, applied to both phases.
- Cases: ρ_A = 36.5, ρ_P = 740, G_A = 3000, G_P = 25000: α = 0.5 →
  G = **18591.3807**, v = **47.8850758**; α = 0.9 → **7681.52408**, v =
  **71.8907261**; limits α = 0 → 25000 (v 33.7837838), α = 1 → 3000
  (v 82.1917808).
- Ours: not implemented.

### CVH-10: liquid critical flux floor (orifice at h = 0)
- Citation: Eqs 6-14, 6-17, CVH/FL-RM-65/66. RETRAN fits (Henry-Fauske
  subcooled, Moody saturated) are used above 170 Btu/lbm; below that there
  is a linear bridge to `G_O = √(2 P ρ_P)` at h = 0. The transition
  enthalpy is kept at least 10 Btu/lbm below saturation.
- Cases: P = 7 MPa, ρ = 740 → G_O = **101784.085**; 0.1 MPa, ρ = 958 →
  **13841.9652**.
- Ours: not implemented. The failed-vessel drain (mod.rs:2021-2027) uses an
  incompressible orifice `Cd A √(2 ρ ΔP)` with **no critical-flow cap**.
  MELCOR would cap the liquid at its RETRAN critical flux. The RETRAN fits'
  coefficients are not in the manual (Ref. [21]), so a reference value
  cannot be computed from this manual.

### CVH-11: flow-path junction void fraction
- Citation: Eqs 5-31, 5-32, CVH/FL-RM-49.
- Equations: `α = (z_TJ - z_P)/(z_TJ - z_BJ)` clamped to [0, 1]; donor
  junction for co-current flow; countercurrent:
  `α_j = (ρ_A^-½ v_A α_Ad + ρ_P^-½ v_P α_Pd)/(ρ_A^-½ v_A + ρ_P^-½ v_P)`.
- Case: junction 1.0 to 1.5 m, pool at 1.2 → α = (1.5 − 1.2)/0.5 = **0.6**.
- Ours: not checked in detail (no Eq 5-31 citation found in
  `tampines/src/flow_path`).

---

## DCH items (decay heat; our `decay.rs`)

### DCH-01: neutron-capture correction G(t)
- Citation: DCH §3.2, Eq 3-5 (DCH-RM-13), G_max Table 3.1 (DCH-RM-10 to -12);
  ψ on DCH_NFA; G_max is SC3204 (DCH-UG-21).
- Equation: for t < 1e4 s, `G = 1 + (3.24e-6 + 5.23e-10 t) T^0.4 ψ`; above,
  `G = G_max(t)` from the table (1.064 at 1e4 s, 1.598 at its peak at 1e8 s).
- Cases: T = 5.05e7 s, ψ = 0.713: T^0.4 = 1205.91459; G(0) =
  **1.00278581**; G(1e4⁻) = **1.00728265**; G(1e4) = **1.064**, a +5.6 % step
  (the manual calls the step small). G_max(1.2e4) = **1.06849660** (linear
  in ln t) or 1.06848503 (log-log).
- Ours: `PlantDecay::capture_factor` decay.rs:186. Test
  `decay_heat_follows_the_ans_1979_form` checks G(0) − 1 = 2.8e-3 ± 1e-4 and
  G(1e4) = 1.064. The G_MAX table entries match Table 3.1 as far as they
  were checked (the 1e4 to 1e9 rows).
- Discrepancy: the manual says only "logarithmic interpolation" for F_i,
  and for G_max says nothing. Ours is linear in ln t. The difference at
  1.2e4 s is 1.2e-5, which is negligible.

### DCH-02: U-239 and Np-239 decay heat
- Citation: Eqs 3-2 to 3-4, DCH-RM-12/13; SC3205 defaults R = 0.526,
  E_U = 0.474 MeV, E_Np = 0.419 MeV, λ1 = 4.91e-4/s, λ2 = 3.41e-6/s
  (DCH-UG-21; an example input there shows 0.54); Q_i (SC3201): 199.0,
  210.2, 199.3 MeV (DCH-UG-20).
- Equations: `F_U = E_U R (1-e^{-λ1T}) e^{-λ1t}`;
  `F_Np = E_Np R [λ1 (1-e^{-λ2T}) e^{-λ2t} - λ2 (1-e^{-λ1T}) e^{-λ1t}]/(λ1-λ2)`;
  `P_d = Σ (P_i/Q_i) (F_U + F_Np)`, **not** multiplied by G (Eq 3-1).
- Cases (T = 5.05e7, MeV/fission): t = 0 → F_U 0.249324, F_Np 0.220394,
  sum **0.469718**; t = 1e3 → **0.372826306**; t = 1.8e4 → **0.208758555**;
  t = 1e5 → **0.157809103**. With the plant's shares (U-235 2.316/3.578,
  Pu-239 1.1114/3.578): mean 1/Q = 4.94163809e-3 (Q_eff 202.362047);
  P_d/P0 at 0 = **2.32117636e-3**, at 5 h = **1.03160923e-3**.
- Ours: `teh_o_prke::point_reactor::actinide_decay_power_after_constant_irradiation`
  (crates/teh-o-prke/src/point_reactor.rs:237), the same algebra. Params
  are params.rs:607-619. The test checks 1.03e-3 ± 0.02e-3 at 5 h. G is
  applied to the fission products only, which matches Eq 3-1.

### DCH-03: fission-product term from Table 3.1
- Citation: Eq 3-1, Table 3.1, DCH-RM-10 to -12; `F(t,T) = F(t,∞) - F(t+T,∞)`
  (SC3203 text, DCH-UG-21); log interpolation in t (DCH-RM-13).
- Case: t = 1e4 s, T = 5.05e7 s, log-log interpolation:
  F_U235 = **1.77106877**, F_Pu239 = **1.60889828**, F_U238 = **1.65815145**
  MeV/fission. With G_max = 1.064 and the plant shares,
  P_FP/P0 = **9.03175150e-3**. Adding the actinides gives
  P_wc/P0 = **1.01007351e-2**. Linear-in-ln-t interpolation gives
  1.00993212e-2 (−1.4e-4 relative).
- Ours: decay.rs takes P_FP from `teh-o-prke`'s 23-group fit of the 1978
  draft standard, not from MELCOR's 1979 tables. The doc comment states
  that the difference is not measured. **Suggested unit test:**
  `PlantDecay::fp_power_w()/P0` at 1e4 s against 9.0318e-3, with the
  tolerance set from a first measurement and recorded either way.
- Discrepancy: different data route. The size is unknown; I did not run it.

---

## HS items

### HS-01: atmosphere and pool convection regime and mixing
- Citation: Eqs 2-79 to 2-84 (HS-RM-37 to -39), Table 2.3 (HS-RM-38), pool
  Eqs 2-103 to 2-107 and Table 2.4 (HS-RM-48 to -50); C4060 limits 1.0 and
  10.0.
- Equations: natural if Re² < Gr, forced if Re² > 10 Gr; mixed
  `Nu = [(Re²/Gr - 1)/9](Nu_f - Nu_n) + Nu_n`. External rectangular:
  natural laminar 0.59 Ra^¼ (Ra < 1e9), turbulent 0.10 Ra^⅓ (Ra > 1e10),
  linear in Ra between; forced laminar 0.664 Re^½ Pr^⅓ (Re < 3e5),
  turbulent 0.037 Re^0.8 Pr^⅓ (Re > 6e5), linear between (Eq 2-91).
  Internal forced laminar Nu = 8.235 (rect) or 48/11 (cyl); turbulent
  0.023 Re^0.8 Pr^⅓.
- Cases (`vv5.py`, Pr = 0.7): Nu_nat(1e8) = **59.0**, (1e9) = **104.918485**,
  (5e9) = **163.159891**, (1e10) = **215.443469**, (1e11) = **464.158883**.
  Note the jump at Ra = 1e10: laminar 186.57 vs turbulent 215.44, which the
  transition band absorbs. Mixed at Gr = 1e10: Re = 5e4 → **184.414728**
  (natural); 2e5 → **210.830799**; 3e5 → **307.530378**; 4e5 →
  **580.550469** (forced, transition region).
- Ours: `structure_surface_mixture`
  (crates/tampines/src/components/wall_condensation.rs:540): external
  rectangular natural convection only, with the transition interpolation
  as above. Matches the natural branch.
- Discrepancy: no forced or mixed regime (stated). Transport properties
  are taken at the film temperature and density at the bulk (stated).

### HS-02: forced-convection laminar to turbulent transition
- Citation: Eqs 2-88 to 2-91, HS-RM-39/40.
- Cases: Pr = 0.7: Re = 1e5 → **186.437853**; 3e5 → **322.919834**;
  4.5e5 → **744.898905**; 6e5 → **1377.49090**; 1e6 → **2072.84934**.
- Ours: not implemented.

### HS-03: Sherwood analogy
- Citation: Eqs 2-134, 2-135, HS-RM-62; C4201 = (1, 1, 1/3, −1/3).
- Equation: `Sh = Nu Sc^⅓ Pr^-⅓`, `h_D = F_m Sh D/L_c` (F_m = 1).
- Case: Nu 150, Sc 0.55, Pr 0.72, D 3.5e-5, L 2 → Sh = **137.120164**,
  h_D = **2.39960287e-3 m/s**.
- Ours: wall_condensation.rs:576. Matches.

### HS-04: condensation with non-condensables, VPFRAC, flashing limit
- Citation: §2.7, §2.7.2 Eqs 2-136 to 2-138 (HS-RM-60 to -63); VPFRAC
  C4200 = 0.9995 (HS-UG-68); h_e C4202 = 5e5 W/m²K (HS-UG-69); no diffusion
  mass transfer above 647.2 K; condensation per cycle capped at 90 %
  (C4203(2)).
- Equation: `ṁ_c = h_D ρ_v ln[(P - P_srf)/(P - P_stm)]`, used only when
  `P_stm < VPFRAC P`, with ρ_v the vapour density at T_sat(P_tot).
- Case: h_D as in HS-03, ρ_v 1.1, P 3e5, P_stm 1.5e5, P_srf 5e4 →
  **1.34835649e-3 kg/m²s** (condensing). Evaporating, P_srf = 2.99e5 →
  **−1.32258883e-2**.
- **Manual inconsistency (report, do not pick silently):** the RM
  (HS-RM-63) prints `ṁ_e = h_e max(0, T_dew - T_srf)/h_fg` with
  `ṁ = min(ṁ_c, ṁ_e)`. The UG (HS-UG-69) prints `min(0, …)` with
  `ṁ = max(ṁ_c, ṁ_e)`. Only the UG form bounds *evaporation*, which is
  what both texts say the limit is for. With condensation positive, the RM
  form would cap condensation instead. Example (T_dew 384.5,
  T_srf 385.5): UG ṁ_e = **−0.227272727 kg/m²s**.
- Ours: wall_condensation.rs:546-578. Discrepancies:
  - Above VPFRAC, MELCOR drops the diffusion limit and lets conduction
    limit the rate. Ours clamps the steam fraction at 0.9995 and keeps the
    diffusion formula (stated in the doc comment).
  - With no non-condensables, ours returns no condensation (`condensing`
    requires `ncg_total() > 0`). MELCOR condenses, limited by conduction.
  - Ours has no evaporation (stated) and so no h_e limit.
  - ρ_v is ideal gas at T_sat(P).

### HS-05: laminar condensate-film coefficient
- Citation: §2.6.1.1, Eq 2-92 and the film Nusselt forms (HS-RM-41/42);
  C4210 = (30, 100, —, sin θ_min 0.1686289), C4211 = (0.943, 0.25)
  (HS-UG-70 to -72).
- Equations: `H_f = max(H_f,corr, k_f/δ_f)`;
  `Nu_l = 0.943 [g ρ_f (ρ_f - ρ_v) h'_fg L³ sin θ / (μ_f k_f (T_sat - T_srf))]^0.25`,
  with `h'_fg = h_fg + 0.68 c_p,f (T_f - T_srf)`.
- Case: L = 1, vertical, ρ_f 958.35, ρ_v 0.5978, k_f 0.679, μ_f 2.817e-4,
  c_p 4216, h_fg 2.2565e6, ΔT 20 K → h'_fg = **2313837.6**, Nu =
  **8100.23749**, h = **5500.06126 W/m²K**; k/δ at δ = 1e-4 m = **6790**,
  so H_f = 6790 there.
- Ours: not found in `tampines`/`sembawang` (wall condensation has no film
  resistance).

### HS-06: Rohsenow nucleate boiling and surface tension
- Citation: Eq 2-114 (HS-RM-52/53; C4180: C_sf 0.013, n 0.33, m 1.0),
  Eq 2-115 (HS-RM-53; C4000).
- Equations: `c_pl ΔT/h_fg = C_sf [q/(μ h_fg) √(σ/(g(ρ_l - ρ_v)))]^n Pr^m`,
  solved for q; `σ = 0.2358 (1 - 0.625 T_R) T_R^1.256`, `T_R = 1 - T/647.3`.
- Cases: σ(300) = **0.0717006381**, σ(373.15) = **0.0589345304**, σ(500) =
  **0.0315087367**, σ(647.3) = 0. q_nb at 1 atm (properties above, Pr_l
  1.75, σ at T_avg): ΔT 5 → **17173.5112**, 10 → **140891.067**, 20 →
  **1160908.58 W/m²**.
- Ours: not implemented. The core uses a constant `boiling_h` param.

### HS-07: CHF, minimum film boiling, Bromley, transition, radiation
- Citation: Eqs 2-116 (HS-RM-54; C4181), 2-117 (HS-RM-54; C4182), 2-118
  (HS-RM-55; C4183), 2-119 to 2-121 (HS-RM-55/56), 2-122 (HS-RM-56/57; C4184
  = 1, σ = 5.669e-8).
- Equations:
  - `q_chf = 0.18 ρ_v h_fg [σ(ρ_l-ρ_v)g/ρ_v²]^¼ [ρ_l/(ρ_l+ρ_v)]^½`.
  - `q_min = 0.09 ρ_v h_fg [σ(ρ_l-ρ_v)g/ρ_l²]^¼ [ρ_l/(ρ_l+ρ_v)]^½`, which is
    algebraically the classic `[…/(ρ_l+ρ_v)²]^¼`. Checked: the two are equal.
  - `q_film = 0.943 [ρ_v(ρ_l-ρ_v) g k_v³ (h_fg + ½ c_pv ΔT)/(μ_v L_c)]^¼ ΔT^0.75`.
  - Transition: log-log interpolation of q between (ΔT_c, q_chf) and
    (ΔT_mfilm, q_min), plus q_rad.
  - `q_rad = σ (T_s⁴ - T_pool⁴)`.
- Cases (1 atm): q_chf = **1510029.86** (σ at T_avg, ΔT 10) or 1522776.98
  (σ at T_sat; Zuber's 0.131 would give 1108588.84); q_min = **19016.1323**;
  Bromley (k_v 0.0251, μ_v 1.227e-5, c_pv 2080, L_c 0.01): ΔT 200 →
  **57955.2764**, ΔT 500 → **118710.792**; q_rad (873.15 to 373.15 K) =
  **31851.3833 W/m²**.
- Ours: Bromley exists only in the FDI form (`fdi.rs:58 bromley_h`, FDI RM,
  outside this range). HS pool boiling is not implemented.

### HS-08: fog cap (CVH)
- Citation: §5.1.4, CVH/FL-RM-48/49; C4406(1) = 0.1 kg/m³ (CVH-UG-51).
- Statement: fog above 0.1 kg/m³ of atmosphere volume rains to the pool,
  after sources and after each subcycle. Case: 5 kg of fog in 20 m³ → 3 kg
  rains out.
- Ours: tampines control_volume mentions rain-out (mod.rs:180). Not checked.

---

## Other manual inconsistencies found
- Pool-bridging thresholds: COR-RM-48 says the lower CV pool fraction must
  exceed 0.10 and the upper 0.001. The UG gives C1270(1) = 0.6 and
  C1270(2) = 0.1 (COR-UG-195). Report both.
- HS-RM-63 vs HS-UG-69 evaporation limit (HS-04).
- DCH-RM-10 calls the G step at 1e4 s small. With ψ = 0.713 it is +5.6 %;
  with ψ = 3 it would be about +3 %.

## Summary for the caller
- 22 items, each with a citation, equations, computed cases and our code
  location. Strongest unit-test candidates: CVH-02 (with its timestep
  independence check), CVH-07, CVH-08, DCH-01/02/03, HS-01, HS-03, HS-04,
  HS-06/07.
- Defects and doc issues to raise, not fixed here:
  1. The `swollen_level_from_sources` doc and test blame its residual on
     the grid. It is a systematic +1.25 % at void 0.1, from generation
     below the collapsed level.
  2. Lower-plenum vapour swells the core in ours but not in MELCOR's
     separate-CV nodalisation.
  3. Ours has no quench-front exclusion from swell (COR-RM-47).
  4. Ours has no flashing swell.
  5. The failed-vessel liquid drain has no critical-flow cap.
  6. The vessel's steam choking uses the steam tables, not Eq 6-13
     (about 10 % apart at 7 MPa, estimated).
  7. Wall condensation has no pure-steam condensation and holds the
     steam fraction at VPFRAC.
  8. P_FP comes from the 1978 23-group fit, not MELCOR's 1979 tables.
     Suggested test value: 9.0318e-3 P0 at 1e4 s.
