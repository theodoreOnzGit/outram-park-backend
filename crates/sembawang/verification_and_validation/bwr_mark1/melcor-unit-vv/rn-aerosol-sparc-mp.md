# AI V&V items: MELCOR 2.2 RN (release, aerosol, vapour, pool scrubbing, iodine), SPR, MP

> AI draft, not human-reviewed. Research, education and V&V only.
> Sources: MELCOR 2.2 Vol. 2 Reference Manual (SAND2023-10994 O) text lines
> RN 38538-47123, SPR 47140-47337, MP 33340-35562; Vol. 1 Users' Guide
> (SAND2017-0455 O). Page labels are the printed page footers (`RN-RM-nn` is
> the footer that ends page nn). Equations are restated in my own symbols;
> no manual prose is copied.
>
> Every expected number below was computed with `python3 -I` on 2026-11-26.
> Full scripts: calc/vvcalc_rn.py`, `ae.py`, `fm.py`, `vp.py`, `ps.py`, `mp.py`
> beside this file. Snippets show the load-bearing lines.
>
> Code paths are relative to `/home/user/outram-park-backend/crates/`.
> `offbeat/melcor/` is `outram-park-fork-offbeat/src/melcor/`;
> `bwr/` is `sembawang/src/bwr_mark1/`.

## Summary table

| ID | Item | Implemented | Manual-referenced test exists | Discrepancy |
|---|---|---|---|---|
| RN-01 | CORSOR-M Arrhenius release, all 12 classes | Xe/Cs/I2/Te row and UO2 row | yes, loose (hand values, 0.2 to 3 %) | gas constant: 0.14-0.33 % rate offset |
| RN-02 | CORSOR `A exp(BT)` | no | no | manual: K vs degC for T |
| RN-03 | Booth diffusion release fraction | no | no | manual: unit label of Q, D0 low/high, 2-13/2-14 seam |
| RN-04 | S/V multiplier, Te retention multiplier, gap failure T | no (gap: rejected with a stated reason) | no | none beyond "not built" |
| AE-01 | Gas density, mean free path, slip factor | yes | partly | manual `0.069 um` vs formula `0.0648 um` |
| AE-02 | Gravitational settling velocity | yes | yes (hand) | g = 9.80665 vs manual 9.8 |
| AE-03 | Brownian deposition velocity | yes | yes (hand) | none |
| AE-04 | Thermophoretic velocity | yes | linearity only | no absolute value pinned |
| AE-05 | Diffusiophoresis with Stefan flow | yes | one branch | none |
| AE-06 | Brownian coagulation kernel (Fuchs) | yes | yes (limits) | manual's B-4 printed inverted (code reads it right) |
| AE-07 | Gravitational and turbulent kernels | yes | symmetry only | none |
| AE-08 | Sectional aerosol area (Eq 2-102) | different (representative diameter) | no | -0.2 % to +12 % area |
| VP-01 | Vapour pressure curves and extrapolation | Cs, I2, Te | continuity + one point | Vol. 1 and Vol. 2 Cs curves differ by 5 decades; 2-108 not built |
| VP-02 | TRAP-MELT2 analytic step | yes | yes | kw from heat-mass analogy, not Eq 2-103 |
| VP-03 | Chapman-Enskog binary diffusivity | yes | yes (cross-check) | none |
| PS-01 | DF_EC (D-5, D-6) and its iodine cap (D-7) | yes | yes (D-7) | none |
| PS-02 | Globule size (D-1 to D-4) | yes | no | none |
| PS-03 | Inertial impaction (D-8 to D-11) | yes | yes (seam) | C7155(7) 4.2597e-3 vs D-11 text 4.25973e-3 |
| PS-04 | Bubble size, shape, rise and swarm velocity (E.1 to E.5) | yes | yes | none |
| PS-05 | Swarm-rise increment and overall DF (2-112 to 2-114) | 2-112, 2-113 | indirectly | 2-114 not a function |
| IO-01 | SPARC I2 equilibrium constants (C7159) vs I2 partition coefficient (Eq 2-180) | C7159 only | yes (C7159) | functional forms are inferred, and 2-180 disagrees with K1 by up to 27 % |
| MP-01 | UO2 enthalpy table | via MATPRO cp | no | matches within 2e-5 (consistent) |
| MP-02 | Zircaloy enthalpy table | via MATPRO cp | no | +109 J/kg above 1113 K (inherited interpolation bug); the manual's own 400 K row is inconsistent |
| MP-03 | Melting points, heats of fusion, densities | partly | fusion step only | Zr density 6550 vs 6500; MP page labels in code comment |
| MP-04 | Conductivity tables UO2, Zr, ZrO2 | no (constant UO2 k = 3.0) | no | the constant UO2 k is up to 2x off |
| SPR-01 | Spray droplet drag coefficient breakpoints | no (equilibrium spray) | no | none |

---

## RN-01 CORSOR-M fractional release rate

**Citation.** Vol. 2 RN §2.3.1.2, Eq (2-11), RN-RM-19. Defaults: Vol. 2 App. A
C7102, RN-RM-131 (with note on RN-RM-132); Vol. 1 RN UG §SC 7102, RN-UG-115/116.

**Equation.** Fractional release rate `fdot [1/min] = k0 exp(-Q / (R T))`,
`k0` in 1/min, `Q` in kcal/mol, `T` in K, `R = 1.987e-3 kcal/(mol K)` (stated
in the manual). Class defaults (k0 1/min, Q kcal/mol):

| class | k0 | Q | class | k0 | Q |
|---|---|---|---|---|---|
| Xe, Cs, I2, Te | 2.0e5 | 63.8 | Ce | 2.67e8 | 188.2 |
| Ba | 2.95e5 | 100.2 | La, UO2 | 1.46e7 | 143.1 |
| Ru | 1.62e6 | 152.8 | Cd, Ag | 5.95e3 | 70.8 |
| Mo | 23.15 | 44.1 | BO2, H2O, CON, CsI, CsM | 0 | 0 |

**Worked cases** (calc/vvcalc_rn.py`):

```python
R_kcal=1.987e-3
a*m.exp(-q/(R_kcal*T))        # per minute
```

| T (K) | Xe/Cs/I2/Te | Ba | Ru | Mo | Ce | La/UO2 | Cd/Ag |
|---|---|---|---|---|---|---|---|
| 1173 | 2.588340e-07 | 6.299603e-14 | 5.469161e-23 | 1.403754e-07 | 2.284279e-27 | 3.163611e-20 | 3.821038e-10 |
| 1500 | 1.010665e-04 | 7.404302e-10 | 8.804978e-17 | 8.682680e-06 | 1.008141e-19 | 2.055758e-14 | 2.871511e-07 |
| 2000 | 2.131636e-02 | 3.308024e-06 | 3.242836e-11 | 3.508553e-04 | 7.232173e-13 | 3.355964e-09 | 1.089462e-04 |
| 2400 | 3.095749e-01 | 2.211112e-04 | 1.967992e-08 | 2.230310e-03 | 1.937058e-09 | 1.355938e-06 | 2.122086e-03 |
| 2500 | 5.286599e-01 | 5.124120e-04 | 7.089947e-08 | 3.228588e-03 | 9.391118e-09 | 4.503233e-06 | 3.843024e-03 |

Per second (divide by 60), manual R: Cs at 2400 K **5.159582e-03 /s**, Cs at
1000 K **3.786558e-11 /s**, UO2 at 2500 K **7.505388e-08 /s**. Half-release
time for Cs at 2400 K: 134.342 s.

Edge cases: the zero-coefficient classes must return exactly 0 (not NaN from
`0 * exp(-0/RT)`, which is 0 here but worth pinning); Mo, with the smallest Q,
overtakes Cs below about 1100 K (1000 K: Mo 5.317472e-09 vs Cs 2.271935e-09
/min; 1100 K: Mo 3.999115e-08 vs Cs 4.208137e-08). This is a property of the
default table and a useful ordering check.

**Our implementation.** `bwr/core.rs:1201` `volatile_release_rate`
(Xe/Cs/I2 row), `bwr/core.rs:1210` `uo2_release_rate` (UO2 row);
constants `bwr/params.rs:304-309`. Tests: `volatile_release_follows_corsor_m`
(`core.rs:2234`, 2e-3 and 2 % tolerances against 3-figure hand values) and
`uo2_release_follows_corsor_m` (`core.rs:2245`, 3 % tolerance). Te is
computed by the same row but not tracked; the other classes are not
implemented.

**Discrepancy.** The code converts `Q` with `4184 J/kcal` and divides by
`R_GAS = 8.314462 J/(mol K)` (`offbeat/melcor/params.rs`). That is an effective
`R = 1.98720e-3 kcal/(mol K)`, 1.03e-4 above the manual's 1.987e-3. Because
`Q/RT` is 13 to 29 here, the rate shifts by 0.14 % (Cs, 2400 K: code
5.166677e-03 vs manual 5.159582e-03), 0.33 % (Cs, 1000 K: 3.799067e-11 vs
3.786558e-11) and 0.30 % (UO2, 2500 K: 7.527628e-08 vs 7.505388e-08). The
existing tests pass only because their tolerances (0.2 % to 3 %) are wider
than this offset. A test against the manual's own `R` would fail at 1e-6.
Whether to use the manual's `R` (code-to-code) or CODATA's (physics) is a
maintainer decision; the present tests cannot tell the two apart.

---

## RN-02 CORSOR exponential release

**Citation.** Vol. 2 RN §2.3.1.1, Eq (2-10), RN-RM-19; App. A C7101,
RN-RM-128 to RN-RM-130. Vol. 1 SC 7101, RN-UG-113/114.

**Equation.** `fdot [1/min] = A_i exp(B_i T)` for `T >= T_i`, three ranges
`i`; zero below `T_1`. Defaults: `T_i = 900, 1400, 2200` (all classes but
Te), `900, 1600, 2000` (Te). Cs: `A = 7.53e-12, 2.02e-7, 1.74e-5`, `B = 0.0142,
0.00667, 0.00460`. (Full per-class table in C7101; Xe, I2 share the
`7.02e-9, 2.02e-7, 1.74e-5 / 0.00886, 0.00667, 0.00460` row.)

**Worked cases** (Cs, `T` used directly, as the formula is written):

| T | 899 | 900 | 1399 | 1400 | 2000 | 2199 | 2200 | 2400 |
|---|---|---|---|---|---|---|---|---|
| fdot (1/min) | 0 | 2.673489e-06 | 3.194496e-03 | 2.295056e-03 | 1.255567e-01 | 4.734693e-01 | 4.321250e-01 | 1.084327e+00 |

Edge case: the correlation is **discontinuous** at the range boundaries
(1400: 3.19e-3 drops to 2.30e-3; 2200: 0.473 drops to 0.432). A test should
pin the range selection at exactly `T_i`.

**Our implementation.** Not implemented (CORSOR-M is used).

**Discrepancy (in the manuals).** Vol. 2 App. A labels the C7101
temperatures **K**, while Vol. 1 labels them **degC**; both give `B` in
**degC^-1**. The two readings differ by a factor of 6.2 at 1673.15 K (degC
reading 2.295056e-03, K reading 1.419185e-02 /min) and the K reading gives
more than 100 %/min for Cs at 2400 K. The degC reading looks like the one
intended (CORSOR's original form uses degC), but this is a judgement, not
checked against the CORSOR source report. Flag for whoever implements it.

---

## RN-03 CORSOR-Booth diffusion release

**Citation.** Vol. 2 RN §2.3.1.3, Eqs (2-12) to (2-15), RN-RM-20; (2-16) to
(2-22) RN-RM-21/22. App. A C7103 RN-RM-132, C7106/C7107 RN-RM-133. Vol. 1 SC
7103 RN-UG-116, SC 7106 RN-UG-117/118.

**Equations.**
- `D = D0 exp(-Q/(R T))`; defaults `D0 = 1e-6 m^2/s` (both low and high
  burn-up), transition burn-up `3e4 MWd/tU`, `Q = 3.814e5`, grain radius
  `a = 6e-6 m`.
- Reduced time `x = D t / a^2`. Released fraction
  `f = 6 sqrt(x/pi) - 3x` for `x < 1/pi^2`, else
  `f = 1 - (6/pi^2) exp(-pi^2 x)`.
- Other classes: diffusion rate scaled by `S_k` (C7103) and combined in series
  with gas-phase transport, `mdot_tot = 1/(1/DIFF_k + 1/mdot_k)`.

**Worked cases** (`R = 8.314462618`, `Q` read as J/mol):

| T (K) | D (m^2/s) | D/a^2 (1/s) |
|---|---|---|
| 1500 | 5.232769e-20 | 1.453547e-09 |
| 2000 | 1.094079e-16 | 3.039108e-06 |
| 2500 | 1.074580e-14 | 2.984946e-04 |

`f(x)`: x = 1e-4: 0.033551; 1e-2: 0.308514; 0.05: 0.606940; 0.2: 0.915552;
1.0: 0.999969.

Edge case, the seam at `x = 1/pi^2`: the short-time branch gives 0.773559,
the long-time branch 0.776356. The two branches **do not meet** (jump
2.8e-3), so a test must state which branch the boundary point takes.

**Our implementation.** Not implemented.

**Discrepancies (in the manuals).**
1. App. A and Vol. 1 give `Q` in "J/kg-mole". Read literally (J/kmol), `Q/RT`
   is 0.023 at 2000 K and `D/a^2` is about 2.7e4 /s, which is physically
   absurd. Only J/mol gives a plausible release time scale. The unit label
   appears wrong.
2. The text says high-burn-up `D0` is five times the low one; both defaults
   are `1.0e-6`.
3. C7103 for Mo: Vol. 2 App. A gives 1.0e-3, Vol. 1 gives 6.25e-2.

---

## RN-04 S/V multiplier, Te retention, gap failure temperature

**Citation.** Vol. 2 RN §2.3.1.4, Eq (2-23), RN-RM-22, base S/V C7104 = 422.5
1/m (RN-RM-132; Vol. 1 RN-UG-117). Te retention: RN §2.3.1 text (RN-RM-18),
C7105(1) = 0.70 cut-off, C7105(2) = 0.025 (= 1/40) (RN-RM-132; Vol. 1
RN-UG-117). Gap release: RN §2.3.2, RN-RM-24, default cladding failure
temperature 1173 K (Vol. 1 RN1_GAP00, RN-UG-41).

**Equations.**
- `fdot = fdot_CORSOR(-M) x (S/V)_structure / 422.5`.
- Te: multiply the Te rate by 0.025 while unoxidised intact cladding mass is
  above 0.70 of the intact cladding mass (metal plus oxide).
- Gap: the whole ring's gap inventory goes to the channel volume when any
  cell's cladding exceeds 1173 K (or loses its geometry).

**Worked cases.** S/V = 800 1/m: multiplier 1.893491 (doubling: 1600 1/m,
3.786982). Te at 2000 K under unoxidised cladding: 2.131636e-02 x 0.025 =
5.329090e-04 /min; at an unoxidised fraction of exactly 0.70 the multiplier
switches off (pin which side gets equality). Gap: 1172.999 K no release,
1173.0 K release of the ring.

**Our implementation.** S/V and Te retention: not implemented (stated in
`core.rs:1188-1200`). Gap holdup: checked and rejected with a measured
argument in the same doc comment (default initial gap inventory zero;
holdup moves release by under 1e-5). That argument checks out: at 1173 K the
Cs rate is 2.588340e-07 /min = 1.553e-05 per hour, matching the comment's
"1.6e-5 per hour".

**Discrepancy.** None in what is built.

---

## AE-01 Gas density, mean free path, slip correction

**Citation.** Vol. 2 RN App. B Eqs (B-3), (B-10) to (B-12), RN-RM-149; Eq
(2-41), RN-RM-38. Slip default 1.257: Vol. 1 RN1_MS00, RN-UG-50.

**Equations.**
- `rho_g = 1.21e-4 P M / T` (P Pa, M g/mol, T K, result kg/m^3).
- `lambda = (mu/rho_g) sqrt(1.89e-4 M / T)` (the extracted text prints `M T`
  under the root; only `M/T` is dimensionally and physically right, the
  constant being `pi/(2 R 1000)`).
- `Kn = 2 lambda / d`, `C_m = 1 + Kn [F_slip + 0.4 exp(-1.1/Kn)]`.

**Worked cases** (air, T 298.15 K, P 101325 Pa, M 28.97, mu 1.8e-5 Pa s):
`rho_g = 1.191285e+00` (ideal gas with CODATA R would be 1.184121e+00);
`lambda = 6.475072e-08 m`.

| d (m) | C_m |
|---|---|
| 1e-8 | 22.036557 |
| 1e-7 | 2.849367 |
| 1e-6 | 1.162794 |
| 1e-5 | 1.016278 |

Edge: `C_m -> 1` as `d -> inf` (1e-5: 1.016); free-molecular `C_m ~ 2 lambda
(1.657)/d` at small d.

**Our implementation.** `offbeat/melcor/aerosol.rs:105` `rho_gas`, `:109`
`mean_free_path`, `:113` `slip_correction`. Test
`the_maeros_kernels_reach_their_limits` (`aerosol.rs:470`) pins
`lambda = 0.066 +- 0.004 um` at mu 1.84e-5.

**Discrepancy.** None in the code. The manual's text quotes about 0.069 um
for air at 298 K, while its own Eq B-11 gives 0.0648 um (mu 1.8e-5); the
code doc comment already notes this. The constant 1.21e-4 in B-12 is 0.6 %
above the ideal-gas value; the code follows the manual (correct for
code-to-code, worth one line in the doc).

---

## AE-02 Gravitational settling velocity

**Citation.** Vol. 2 RN §2.4.2.2, unnumbered equation above (2-41), RN-RM-38;
App. B Eq (B-14), RN-RM-150.

**Equation.** `v_grav = rho_p g d^2 C_m / (18 mu chi)`, g = 9.8 m/s^2 in the
manual's nomenclature, chi = dynamic shape factor (default 1).

**Worked cases** (AE-01 air, rho_p 1000, chi 1, g 9.8):

| d (m) | v_grav (m/s) |
|---|---|
| 1e-8 | 6.665378e-08 |
| 1e-7 | 8.618455e-07 |
| 1e-6 | 3.517093e-05 |
| 1e-5 | 3.073928e-03 |

Edge: ceilings take `-v_grav`, walls 0, floors and pools `+v_grav`
(RN-RM-37 table); total kernel floored at zero.

**Our implementation.** `aerosol.rs:123` `settling_velocity`, constant
`G = 9.80665` (`aerosol.rs:56`); floor/wall/ceiling sign `aerosol.rs:404-406`.
Test: same test as AE-01 (10 um about 3.0e-3 +- 0.15e-3, at mu 1.84e-5).

**Discrepancy.** `g`: code 9.80665, manual 9.8. Relative effect +6.786e-4
(1 um: 3.519479e-05 vs 3.517093e-05). Physically the code is right; for a
code-to-code comparison it is a known, explained 7e-4 offset, above the
1e-9 code-to-code allowance.

---

## AE-03 Brownian diffusion deposition velocity

**Citation.** Vol. 2 RN §2.4.2.2, Eq (2-42), RN-RM-39. Boundary layer default
1e-5 m: Vol. 1 RN1_MS01, RN-UG-51.

**Equation.** `v_diff = k_B T C_m / (3 pi mu chi d Delta)`, Delta = 1e-5 m.
(The manual labels k_B with Stefan-Boltzmann's units; the value 1.38e-23 J/K
is Boltzmann's.)

**Worked cases** (AE-01 air, k_B = 1.380649e-23): 1e-8 m: 5.347099e-03;
1e-7: 6.913896e-05; 1e-6: 2.821482e-06; 1e-5: 2.465966e-07 m/s.

Edge: inversely proportional to Delta (halving Delta doubles it exactly).

**Our implementation.** `aerosol.rs:128` `diffusion_velocity`. Test
`fallout_and_deposition_follow_melcor` (`aerosol.rs:546`) pins 6.87e-5 m/s
at 0.1 um with mu 1.84e-5 (consistent with the 6.9139e-5 above at mu 1.8e-5).

**Discrepancy.** None.

---

## AE-04 Thermophoretic deposition velocity

**Citation.** Vol. 2 RN §2.4.2.2, Eqs (2-43), (2-44), RN-RM-39/40. Defaults
`k_gas/k_p = 0.05`, `c_t = 2.25`: Vol. 1 RN1_MS01, RN-UG-51.

**Equation.**
`v_th = 3 mu C_m (c_t Kn + k_r) gradT / [2 chi rho_g T_w (1 + 3 F_slip Kn)(1 + 2 c_t Kn + k_r)]`,
with `gradT = -q''/k_air` at the wall temperature `T_w`.

**Worked cases** (AE-01 air, gradT 1000 K/m, T_w 298.15 K):
d = 1e-7 m: **1.586491e-05 m/s**; d = 1e-6 m: **1.241725e-05 m/s**.
Edge: zero at zero gradient; linear in gradT; nearly size-independent in the
free-molecular limit.

**Our implementation.** `aerosol.rs:135` `thermophoretic_velocity`; gradient
from `q''/k_air` in `bwr/containment_fp.rs:483-495`. Test checks only zero
and linearity (`aerosol.rs:546`).

**Discrepancy.** No absolute value is pinned. The values above are the
missing test.

---

## AE-05 Diffusiophoresis with Stefan flow

**Citation.** Vol. 2 RN §2.4.2.2, Eqs (2-45), (2-46), RN-RM-40.

**Equation.** Condensing (`W >= 0`):
`v = M_s/(X_s M_s + X_nc M_nc) x W/rho_b`; evaporating (`W < 0`): `v = W/rho_s`.

**Worked cases.** W = 1e-3 kg/(m^2 s), X_s = 0.3, M_s = 0.018015, M_nc =
0.02897, rho_b = 1.0: **7.014231e-04 m/s**. W = -1e-3, rho_s = 0.1:
**-1.000000e-02 m/s**. Edge: pure steam (X_s = 1) reduces to W/rho_b. Pool
surfaces: the manual zeroes thermo- and diffusiophoresis on pools internally
(RN-RM-37, footnote 2).

**Our implementation.** `aerosol.rs:242` `diffusiophoretic_velocity`; pool
surfaces carry zero phoretic velocity (`containment_fp.rs:520`), as the
manual does. Test pins only the pure-steam branch.

**Discrepancy.** None; add the two cases above (mixed gas, evaporation).

---

## AE-06 Brownian coagulation kernel (Fuchs)

**Citation.** Vol. 2 RN App. B Eqs (B-1) to (B-12), RN-RM-149.

**Equation.** `beta = 2 pi (D_i + D_j)(gamma d_i + gamma d_j) / F_b`,
`F_b = s/(s + 2 g_ij) + 8 (D_i + D_j)/(v_ij s c_s)`, `s = d_i + d_j`,
`v = sqrt(8 k T / (pi m))`, `l = 8 D/(pi v)`,
`g = [(d + l)^3 - (d^2 + l^2)^(3/2)]/(3 d l) - d`, quadrature sums for `g_ij`,
`v_ij`.

**Worked cases** (AE-01 air, rho_p 1000, gamma = c_s = 1):

| d_i = d_j | beta (m^3/s) | ratio to continuum `8kT C_m/(3 mu)` |
|---|---|---|
| 1e-9 | 6.286067e-16 | 0.004791 |
| 1e-8 | 1.927651e-15 | 0.143440 |
| 1e-7 | 1.460572e-15 | 0.840544 |
| 1e-6 | 6.922751e-16 | 0.976251 |
| 1e-5 | 6.156271e-16 | 0.993323 |

Free-molecular limit `pi/4 (2d)^2 sqrt(2) v`: ratio 1.000000 at 1e-10 m,
0.999999 at 3e-10 m, 0.999964 at 1e-9 m. Unequal pair 0.1 um/1 um:
4.884740e-15.

**Our implementation.** `aerosol.rs:143` `brownian`, `:186` `kernel_table`.
Test `the_maeros_kernels_reach_their_limits` checks both limits (1 %, 0.5 %)
and table-vs-kernel agreement to 1e-12.

**Discrepancy.** The extracted manual prints `F` as the bracket; only its
inverse reaches both limits. The code uses the inverse and documents why.
No code-manual disagreement beyond that.

---

## AE-07 Gravitational and turbulent coagulation kernels

**Citation.** Vol. 2 RN App. B Eqs (B-13) to (B-18), RN-RM-150; Eq (2-38)
RN-RM-34; turbulent dissipation default 0.001 m^2/s^3: Vol. 1 RN1_MS01,
RN-UG-51.

**Equations.**
- `beta_g = eps_g (pi/4) c_s (gamma d_i + gamma d_j)^2 |V_Ti - V_Tj|`,
  `eps_g = 1.5 min(d_i,d_j)^2/(d_i+d_j)^2`.
- `beta_T1 = sqrt(pi eps rho_g/(120 mu)) (gamma d_i + gamma d_j)^3`.
- `beta_T2 = 0.04029 rho_g^(1/4) eps^(3/4)/mu^(5/4) (gamma d_i + gamma d_j)^2 |rho_p C_i d_i^2/chi - rho_p C_j d_j^2/chi|`.
- `beta_T = c_s sqrt(beta_T1^2 + beta_T2^2)`.

**Worked cases** (AE-01 air): 1 um/10 um: gravitational **3.579952e-15**,
turbulent **3.015398e-15** m^3/s; equal 1 um turbulent (shear only)
**1.053042e-17**. Edge: gravitational is exactly 0 for equal sizes and
symmetric.

**Our implementation.** `aerosol.rs:160` `gravitational`, `:167` `turbulent`.
Test checks zero-at-equal and symmetry only; the doc comment quotes
"gravitational 3.5e-15, turbulent 3.0e-15" at mu 1.84e-5.

**Discrepancy.** None; pin the absolute values.

---

## AE-08 Sectional aerosol surface area (Eq 2-102)

**Citation.** Vol. 2 RN §2.5.2, Eq (2-102), RN-RM-60 (derived in App. C).
Sherwood number 2 for aerosols: same page.

**Equation.** For a section of mass concentration `M` between particle masses
`m1` and `m2 = 2 m1` (log-uniform mass distribution),
`A_p = 12 pi (3/(4 pi rho))^(2/3) M / ln(m2/m1) x (m1^(-1/3) - m2^(-1/3))`.

**Worked case.** rho 1000, m1 = mass of a 1 um sphere, m2 = 2 m1, M = 1e-9
kg/m^3: **A_p = 5.357290e-06 m^2/m^3**. Same mass at the geometric-mean
particle: 5.345392e-06 (-0.22 %); at the lower-edge particle: 6.000000e-06
(+12.0 %).

**Our implementation.** `offbeat/melcor/rn_volume.rs:278-283` uses
`N_k pi d_k^2` at the section's representative diameter (Jacobson's scheme,
not MAEROS's), with `k = 2 D/d` (Sh = 2, as the manual says).

**Discrepancy.** Model choice, documented as "Discretisation is not MAEROS's"
in `aerosol.rs`. Jacobson's scheme has no section bounds: `Sections::diameter`
(`aerosol.rs:291`) is the diameter of the pivot volume `v_k = v_0 2^k`, and
all of a section's mass sits at it. Mapped onto MAEROS bounds, the area
differs from Eq 2-102 by about -0.2 % if the pivot is taken as the geometric
mean of a MAEROS section, and +12 % if it is taken as the lower bound. A test
comparing the two schemes must first state that mapping.

---

## VP-01 Fission-product vapour pressure

**Citation.** Vol. 2 RN §2.5.2, Eqs (2-106) to (2-110), RN-RM-62; App. A C7110,
RN-RM-134 to RN-RM-137. Vol. 1 SC 7110, RN-UG-119 to RN-UG-121. Molar masses
C7120, Vol. 1 RN-UG-123.

**Equations.** `log10 P[mmHg] = -A/T + B + C log10 T` per range, zero below
the first range, `C_sat = P M/(R T)`. Above the last tabulated `T_max`,
`log10 P = -A'/T + B'` with `A' = A + C log10(e) T_max`,
`B' = B + C (log10(e) + log10 T_max)` (P and dP/dT continuous).

Defaults used by our code (Vol. 1):

| class | T_i (K) | A | B | C |
|---|---|---|---|---|
| Cs (as CsOH) | 600 / 1553 | 9.4e3 (printed 9.4E4) / 6870.778 | 21.59 / 7.994503 | -3.75 / 0 |
| I2 | 273 / 387 / 457 | 3578 / 3205 / 2176.912045 | 17.72 / 23.66536 / 7.637352 | -2.51 / -5.18 / 0 |
| Te (as TeO) | 273 | 13940 | 23.51 | -3.52 |

**Worked cases** (calc/vvcalc_vp.py`, 1 mmHg = 133.322368 Pa):

| T (K) | Cs (Vol. 1, A=9.4e3) mmHg | I2 mmHg | Te mmHg | CsI mmHg |
|---|---|---|---|---|
| 500 | 0 | 1.921005e+03 | 1.347800e-14 | 0 |
| 800 | 8.982817e-02 | 8.245417e+04 | 7.347223e-05 | 8.084781e-03 |
| 1000 | 8.709636e+00 | 2.886959e+05 | 1.023293e-01 | 1.281419e+00 |
| 1500 | 2.588160e+03 | 1.534892e+06 | 1.088492e+03 | 5.174280e+02 |
| 2000 | 3.623381e+04 | 3.539131e+06 | 8.324812e+04 | 8.552625e+03 |

Continuity: Cs at 1553 K, both sides log10 P = 3.570306 (with A = 9.4e3; the
printed 9.4E4 gives -50.904903). I2 at 387 K: 1.979367 both sides; at 457 K:
2.873869. CsI at 894 K: -0.868520; at 1553 K: 2.880958.

Saturation concentration: CsOH at 1000 K **2.093670e-02 kg/m^3**; I2 at 400 K
**1.519480e+00 kg/m^3** (R = 8.314462618).

Extrapolation (Te, with Vol. 2's T_max = 10000 K): A' = -1347.165763,
B' = 7.901283; at 12000 K original 7.989615, extrapolated 8.013547 log10
mmHg; slopes match at T_max (-1.347166e-05 per K).

**Our implementation.** `offbeat/melcor/vapour.rs:168-175` ranges,
`:186` `vapour_pressure_pa`, `:200` `saturation_concentration`. Test
`vapour_pressures_are_melcors_continuous_curves` (`vapour.rs:243`): boundary
continuity to 1e-3, zero below range, Xe infinite, CsOH 8.71 +- 0.05 mmHg at
1000 K.

**Discrepancies.**
1. **The two manuals disagree on the Cs curve.** Vol. 2 App. A (RN-RM-134)
   gives Cs `600 K: A 13600, B 8.895, C 0; 1229.5 K: A 12100, B 7.675` (the
   same as CsM in Vol. 1), not the Vol. 1 CsOH curve the code uses. At
   1000 K that is 1.972423e-05 mmHg against 8.709636 mmHg, **5.6 decades**
   apart. The code follows Vol. 1 (with the documented 9.4E4 to 9.4E3
   correction). Which edition's default is current for MELCOR 2.2 is a
   question for the maintainer; the results depend on it heavily.
2. Eqs 2-108 to 2-110 (extrapolation above T_max) are not built; the last
   range is extrapolated as is. No effect for Cs and I2 (C = 0 in the last
   range). For Te (C = -3.52) the original curve peaks at 9118.76 K, and
   Vol. 2's T_max = 10000 K is above the peak, so even Eq 2-108 cannot
   prevent the decrease there. No practical effect below 9000 K.
3. Minor: Vol. 1 says the default Lennard-Jones parameters (3.617, 97) are
   water vapour's; Vol. 2 RN-RM-61 says air's. 3.617 / 97 K are air's
   Bird-Stewart-Lightfoot values.

---

## VP-02 TRAP-MELT2 analytic condensation step

**Citation.** Vol. 2 RN §2.5.2, Eqs (2-96), (2-97) RN-RM-59; (2-98) to
(2-101) RN-RM-60; Eq (2-103) RN-RM-61.

**Equations.** `alpha = sum A_i k_i / V`, `beta/alpha = sum A_i k_i C_i^s / sum A_i k_i`,
`C_a(dt) = beta/alpha - (beta/alpha - C_a0) e^(-alpha dt)`,
`M_i = M_i0 + A_i k_i (beta/alpha - C_i^s) dt - A_i k_i (beta/alpha - C_a0)(1 - e^(-alpha dt))/alpha`.

**Worked case.** V = 100 m^3, C_a0 = 5e-3 kg/m^3, surfaces (A k, C^s) =
(2.0 m^3/s, 0) and (1.0 m^3/s, 3e-3), dt = 30 s, M_i0 = 0:
alpha = 0.03 1/s, beta/alpha = 1e-3; **C_a = 2.6262786390e-03**,
**M_1 = 2.1824809074e-01 kg**, **M_2 = 1.9124045368e-02 kg**; mass balance
error 0. An explicit Euler integration with 3e5 steps gives
C_a = 2.62627644e-03 (agrees to 1e-6 relative, the Euler error). Edge: a
long step relaxes to beta/alpha; a single surface with C^s = 0 decays as
`exp(-A k t/V)`.

**Our implementation.** `vapour.rs:219` `condense`; test
`the_trap_melt_step_conserves_and_relaxes` (`vapour.rs:284`) checks
conservation, single-surface decay and the long-step limit, but at a start
already at `beta/alpha` in one case. The worked case above has non-trivial
values for all three outputs.

**Discrepancy.** Eq 2-103 builds the wall's vapour mass-transfer coefficient
from the HS package's water condensation coefficient times a diffusivity
ratio (the extracted equation is garbled; the exponent on the ratio cannot be
read). The drywell in `containment_fp.rs:468-500` uses `k = h/(rho c_p)`
(heat-mass analogy, Lewis number 1, stated in its doc). Different route,
stated in the code, not Eq 2-103.

---

## VP-03 Chapman-Enskog binary diffusivity

**Citation.** Vol. 2 RN §2.5.2, Eqs (2-104), (2-105), RN-RM-61; C7111
Lennard-Jones parameters Vol. 1 RN-UG-121/122 (Xe 4.055 A / 229 K; I2
4.982 / 550; others 3.617 / 97).

**Equation.** `D_AB [cm^2/s] = 0.0018583 sqrt(T^3 (1/M_A + 1/M_B)) / (P[atm] sigma_AB^2 Omega_D)`,
`sigma_AB = (sigma_A + sigma_B)/2`, `Omega_D(kT/eps_AB)`, `eps_AB = sqrt(eps_A eps_B)`.
Mixture: `D_k = (1 - y_k)/sum_n (y_n / D_kn)`.

**Worked cases** (Omega taken at T* = 1, where the table gives 1.439):
I2 (4.982 A, 253.8) in an air-like gas (3.617 A, 28.97) at
T = sqrt(550 x 97) = 230.977 K, 1 atm: **4.809130e-02 cm^2/s**. Xe-Xe at
229 K, 1 atm: **3.358994e-02 cm^2/s**. Edge: `D ~ 1/P` exactly; a trace in a
single gas equals the binary value.

**Our implementation.** `vapour.rs:125` `binary_diffusivity`, `:147`
`diffusivity`, collision integral table `vapour.rs:87`. Test
`vapour_diffusivity_is_chapman_enskogs` (`vapour.rs:266`) cross-checks
steam-H2 against MELCOR's NCG correlation (10 %), a table point, and the
single-gas identity.

**Discrepancy.** None. The cases above pin an absolute value at a table node
(no interpolation error).

---

## PS-01 Vent-exit condensation DF (D-5, D-6) and iodine cap (D-7)

**Citation.** Vol. 2 RN App. D Eqs (D-5), (D-6) RN-RM-155, (D-7) RN-RM-156;
§2.7.1 RN-RM-65 (no iodine scrubbing when pool iodine over the partition
exceeds the gas's).

**Equations.** `X_o = 1 - p_sat(T_p)/(p_surf + rho_l g h_p)`;
`DF_EC = max(X_o/X_i, 1)`;
`DF_EC,vap = 1 + min(H rho_v/rho_l, 1)(DF_EC - 1)`.

**Worked cases** (p_sat 108000 Pa, p_surf 3e5 Pa, rho_l 957, h_p 1.2 m,
g 9.80665): **X_o = 0.653025**. X_i = 0.2: DF_EC = **3.265127**; X_i = 0.5:
**1.306051**; X_i >= X_o: 1. D-7: DF_EC 5, H 100, ratio 1e-3: **1.4**; H 0:
1; H 1e4: 5. DF_EC 3.2650 with H 16.13 and ratio 6.6e-4: **1.024113**.

**Our implementation.** `bwr/sparc.rs:145` `Pool::x_o`;
`bwr/containment.rs:1587` `condensation_pass` (returns 1/DF_EC);
`offbeat/melcor/iodine.rs:172` `vapour_condensation_df`;
`bwr/containment_fp.rs:445` `iodine_vapour_pass` (the same cap written as a
pass fraction, plus the no-scrubbing rule). Tests:
`hydrolysis_and_cs_oh_raise_the_partition` (`iodine.rs:222`) pins D-7;
`the_swarm_scrubs_large_particles_and_passes_small_ones` (`sparc.rs:516`)
pins X_o to 2e-3.

**Discrepancy.** None.

---

## PS-02 Globule size (D-1 to D-4)

**Citation.** Vol. 2 RN App. D Eqs (D-1) to (D-3) RN-RM-154, (D-4) RN-RM-155;
C7151 RN-RM-139; Vol. 1 RN-UG-131.

**Equations.** `We = rho_l D_o V_o^2/sigma`, `V_o = Q/(pi D_o^2/4)`,
`v_n = a We^b`, `D_g,o^3 = 1.5 v_n D_o^2 sqrt(sigma/(rho_l g))`,
`D_g(x) = D_g,o (1 - x/(12 D_g,o))`. (a, b): multiple holes (3.45, 0.46),
downcomer (0.0891, 0.616), horizontal vent (0.857, 0.73).

**Worked cases** (D_o 0.515 m, Q 1.0 m^3/s, sigma 0.0588 N/m, rho_l 957,
g 9.80665): We = **1.931671e+05**. Multiple holes: v_n 9.318550e+02,
D_g,o **0.9753837 m**; downcomer: v_n 1.606988e+02, D_g,o **0.5429130 m**;
horizontal: v_n 6.190305e+03, D_g,o **1.833567 m**. Edge: D_g = 0 at
x = 12 D_g,o.

**Our implementation.** `sparc.rs:104` `Vent::globule` (coefficients match
C7151), `sparc.rs:197` `globule_diameter`. No test pins D-1 to D-3 directly
(only through the end-to-end swarm test).

**Discrepancy.** None. The globule-volume fraction `(D_g/D_g,o)^3` used for
`f_gl` is the code's reading (the manual gives the shrinkage, not the
fraction); stated in `sparc.rs`.

---

## PS-03 Inertial impaction at the vent exit (D-8 to D-11)

**Citation.** Vol. 2 RN App. D Eqs (D-8), (D-9) RN-RM-156, (D-10), (D-11)
RN-RM-157; C7155 RN-RM-140/141; Vol. 1 RN-UG-133.

**Equations.** `Stk = rho_p V_e d^2/(9 mu D_o)`; `DF_II = 1/(1 - alpha)`;
`alpha = 1.79182 (3.3437e-11)^((5.9244e-3)^Stk)` for Stk <= 0.65868, else
`1.13893 (1.4173e-6)^((4.25973e-3)^Stk)`; alpha capped at 0.99.

**Worked cases:**

| Stk | alpha | DF_II |
|---|---|---|
| 0.1 | 0.000001 | 1.000001 |
| 0.5 | 0.279881 | 1.388659 |
| 0.65868 | 0.786978 (both branches) | 4.694357 |
| 0.7 | 0.847964 | 6.577411 |
| >= 1.0 | 0.99 (cap) | 100 |

Stokes number example: rho_p 1000, V_e 10 m/s, d 5 um, mu 2e-5, D_o 0.5 m:
2.777778e-03 (DF_II = 1 to 1e-9).

**Our implementation.** `sparc.rs:208` `impaction_df`; test
`the_sparc_pieces_match_hand_values` (`sparc.rs:454`) pins the seam
(alpha 0.787) and the small-particle limit.

**Discrepancy.** App. A lists C7155(7) = 4.2597e-3; the D-11 text and the code
use 4.25973e-3. Effect: alpha at Stk 0.7 is 0.84796572 vs 0.847964 (2e-6
relative). Negligible, but a code-to-code test at 1e-9 would see it; record
which value is intended.

---

## PS-04 Swarm bubble size, shape, rise and swarm velocity (E.1 to E.5)

**Citation.** Vol. 2 RN App. E Eqs (E.1) to (E.3) RN-RM-160, (E.4), (E.5)
RN-RM-161; C7152 to C7154 RN-RM-140; Vol. 1 RN-UG-131/133.

**Equations.**
- `d_vm [cm] = 0.7 x 10^(-0.2265 + sqrt(0.0203 + 0.0313 x_nc))`.
- `a/b = 0.84107 + 1.13466 d_vm - 0.3795 d_vm^2` for 0.15 <= d_vm <= 1.3 cm;
  1 below, 1.675 above.
- `V_r = 7.876 (sigma/rho_l)^(1/4)` cm/s (cgs) for d_vm <= 0.5 cm, else
  `1.40713 d_vm^0.49275 V_r,T`.
- `V_s(x) = sqrt((Q_s + 5.33)/3.011e-3) (1 - 3.975e-4 x)` cm/s (Q_s L/s at
  mid-depth, x cm), mean of vent and surface values, at most 170 cm/s.

**Worked cases.** x_nc 0: d_vm 0.576868 cm, a/b 1.369330; x_nc 0.5:
0.642988, 1.413745; x_nc 1: **0.701059 cm, 1.450015**. Shape correlation at
its ends: 1.002730 at 0.15 cm, 1.674773 at 1.3 cm (small jumps to 1 and
1.675). Water sigma 0.0717 N/m, rho_l 996.5: V_r,T **22.938561 cm/s**;
at 0.7011 cm 27.096205 cm/s; branch factor at 0.5 cm 1.000004.
V_s: Q 0, depth 120 cm: **41.070008 cm/s**; Q 10 L/s, 300 cm: 67.099112;
Q 1000 L/s: capped at 170.

**Our implementation.** `sparc.rs:164` `bubble_axes`, `:181` `rise_velocity`,
`:190` `swarm_velocity`; test `the_sparc_pieces_match_hand_values`
(`sparc.rs:454`).

**Discrepancy.** None in these correlations. Elsewhere in App. E the code
departs on purpose and says so (`sparc.rs:29-62`): ellipse normal `(b/a)^2`
where Eq E.15 prints `(a/b)^2`; Levich exposure time; isothermal saturated
bubble (no Eqs E.6 to E.11); Stokes settling throughout (no E.17 to E.19).

---

## PS-05 Swarm-rise increment and overall DF (2-112 to 2-114)

**Citation.** Vol. 2 RN §2.7.1, Eqs (2-112), (2-113) RN-RM-66, (2-114)
RN-RM-67. XNRISE default 10: C7150(1), RN-RM-139.

**Equations.** Per step: `DF_SR,i = 1/(f_gl + (1 - f_gl)/DF_BB,i)`; overall
`DF_OV,i = DF_EC DF_II,i DF_ER,i DF_SR,i`; all sizes:
`DF_OV,part = sum m_i / sum (m_i/DF_OV,i)`.

**Worked cases.** f_gl 0.3, DF_BB 10: **2.702703**; f_gl 1: 1; f_gl 0: 10.
Masses (1, 2, 1) with DFs (1.5, 10, 1000): **DF_OV,part = 4.610065**.

**Our implementation.** 2-112 inside `sparc.rs:337` `pass_fractions` and
`:391` `iodine_pass` (`pass *= f_gl + (1 - f_gl)/df_bb`); 2-113 assembled
across `containment.rs` and `sparc.rs`; 2-114 not a standalone function (mass
is carried per section instead, which is equivalent when the inlet sizes are
tracked). End-to-end results pinned in `sparc.rs:516` and `:548`.

**Discrepancy.** None in form. The building blocks have no unit test of
their own; the two cases above give them one.

---

## IO-01 SPARC iodine equilibrium constants (C7159) vs the I2 partition coefficient (Eq 2-180)

**Citation.** C7159 coefficients: Vol. 2 App. A RN-RM-142; Vol. 1 RN-UG-137.
Iodine pool model partition coefficients: Vol. 2 RN §2.12.7.5, Eqs (2-180) to
(2-182), RN-RM-120.

**Equations.** C7159 pairs, in the code's inferred form
`K = a exp(b/T)` (water: `a exp(b/T + c/T^2)`): K1 (1.3882e-3, 3279.3),
K2 (7.7606, 1370), K3 (1.0423e-2, -7148), K4 (4.2271e-9, -1748.5),
Kw (1.56531e-13, 5462.81, -1.87376e6). Independent iodine-pool-model
partition coefficient: `log10 PC_I2 = 13.5467 - 0.0605142 T + 7.166e-5 T^2`;
`PC_I0 = 0.0238 PC_I2`; `PC_CH3I = 9.4e-4 exp(2641/T)`.

**Worked cases:**

| T (K) | K1 | K2 | K3 | K4 | Kw | PC_I2 (2-180) | K1/PC_I2 |
|---|---|---|---|---|---|---|---|
| 298.15 | 8.301977e+01 | 7.682093e+02 | 4.036413e-13 | 1.199840e-11 | 9.943148e-15 | 7.490209e+01 | 1.108377 |
| 350 | 1.627583e+01 | 3.889090e+02 | 1.407533e-11 | 2.860430e-11 | 2.138517e-13 | 1.396626e+01 | 1.165368 |
| 375 | 8.715061e+00 | 2.995834e+02 | 5.492383e-11 | 3.990912e-11 | 5.425237e-13 | 8.532229e+00 | 1.021428 |
| 400 | 5.045530e+00 | 2.384262e+02 | 1.807800e-10 | 5.341125e-11 | 1.095843e-12 | 6.406488e+00 | 0.787566 |

At 298.15 K: PC_I0 1.782670, PC_CH3I 6.608302.

**Our implementation.** `offbeat/melcor/iodine.rs:65` `constants` (C7159);
`:144` `equilibrium`, `:163` `partition`. Test
`the_iodine_equilibrium_closes` (`iodine.rs:186`) compares to textbook values
within 5 % and checks closure to 1e-10. Eq 2-180 to 2-182 (iodine pool model,
§2.12): not implemented.

**Discrepancies.**
1. The functional forms of the C7159 constants are not printed in the
   manual; the code infers `a exp(b/T)` and pins it only to textbook values
   at 298 K (5 %). This is the main unverified assumption in the iodine
   chemistry.
2. MELCOR's two iodine models disagree with each other: K1 (SPARC) differs
   from PC_I2 (iodine pool model, Eq 2-180) by +10.8 % at 298 K, +16.5 % at
   350 K, +2.1 % at 375 K and -21.2 % at 400 K. This is a good cross-check
   for a test with a stated tolerance, and a fact to record, not a defect in
   our code.
3. Minor: `iodine.rs` uses `M_I2 = 0.25381` and `M_CS = 0.13291` kg/mol; C7120
   gives 253.8008 and 132.905 (differences 3.6e-5 and 3.8e-5 relative).
   `vapour.rs` uses the C7120 values exactly.

---

## MP-01 UO2 specific enthalpy table

**Citation.** Vol. 2 MP §2.3, MP-RM-10 (enthalpy table); §4.3 cp table
MP-RM-21; §7 melting point MP-RM-50; §8 heat of fusion MP-RM-51.

**Data.** The default table is MATPRO cp integrated, with the heat of fusion
2.74e5 J/kg added over 0.01 K at 3113 K. Selected rows (J/kg):
300: 33143; 400: 58419; 600: 114638; 1000: 236492; 1500: 397973; 2000: 572782;
2500: 785450; 3000: 1079422; 3113: 1161764; 3113.01: 1435764; 3513: 1636964.

**Worked cases.** Our enthalpy (MATPRO UO2 cp, trapezoid at 1 K from 250 K,
calc/vvcalc_mp.py`) minus its 300 K value, against the table minus 33143:

| T | ours | MELCOR | rel |
|---|---|---|---|
| 400 | 25274.8 | 25276 | -5e-5 |
| 1000 | 203348.0 | 203349 | -5e-6 |
| 2000 | 539639.5 | 539639 | +1e-6 |
| 3000 | 1046297.7 | 1046279 | +1.8e-5 |
| 3113 | 1128643.3 | 1128621 | +2.0e-5 |

Our cp: 400 K 265.84, 1000 K 314.03, 2000 K 375.09 (equal to the MP cp table
to 2 decimals), 3000 K 699.77 (table 699.73), 3113 K 758.27 (table 758.23).
Edge: fusion step 1435764 - 1161764 = **274000 J/kg** exactly.

**Our implementation.** `bwr/core.rs:996` `cp_matpro`, `:1011` `Enthalpy::new`,
`:1050` `uo2_enthalpy`; cp from
`outram-park-fork-offbeat/src/materials/properties/heat_capacity.rs:568`.
Fusion `UO2_FUSION = (3113.0, 2.74e5)` at `core.rs:1048`. Test
`the_matpro_tables_are_positive_and_invert` (`core.rs:2355`) checks
positivity and inversion only; nothing compares to the MP table. (The
node heat capacity outside the debris path uses the constant
`UO2_CP = 330.0`, `params.rs:59`.)

**Discrepancy.** None of substance: agreement within 2e-5 relative (trapezoid
step and the reference temperature). The MP table is a ready-made
code-to-code reference; a test at, say, 5e-5 would pin it.

---

## MP-02 Zircaloy specific enthalpy table

**Citation.** Vol. 2 MP §2.1, MP-RM-9 (enthalpy); §4.1 cp table MP-RM-20.

**Data** (J/kg): 300: 0; 400: 21915; 640: 105110; 1090: 263960;
1093: 265275.5; 1113: 276195.5; 1173: 316935.5; 1248: 363753; 2098: 666353;
2098.01: 891353 (fusion 225000); 3598: 1425353.

**Worked cases** (ours, h(T) - h(300)):

| T | ours (as built) | ours with the interval fixed | MELCOR |
|---|---|---|---|
| 400 | 29150.0 | 29150.0 | 21915.0 |
| 640 | 105110.0 | 105110.0 | 105110.0 |
| 1090 | 263960.0 | 263960.0 | 263960.0 |
| 1093 | 265275.5 | 265275.5 | 265275.5 |
| 1113 | 276304.5 | 276195.5 | 276195.5 |
| 1248 | 363862.0 | 363753.0 | 363753.0 |
| 2098 | 666462.0 | 666353.0 | 666353.0 |

Interval 1093 to 1113 K: ours 11029.0, MELCOR 10920.0 (= the trapezoid of
502 and 590 over 20 K). Our cp jumps at 1093 K from 502.0 to 513.48.

**Our implementation.** `core.rs:1055` `zr_enthalpy` over
`heat_capacity.rs:616` `MatproZircaloy`, whose `(1090.0, 1113.0, 502.0,
590.0)` interval is documented there as upstream's off-by-one bug,
"reproduced deliberately". Test: `core.rs:2355` checks the fusion step
(2.25e5 at 2098 K) and inversion, not the table.

**Discrepancies.**
1. **Ours runs +109.0 J/kg high from 1113 K upward** (+0.04 % at 1113 K,
   +0.016 % at 2098 K). The cause is the inherited interpolation interval;
   MELCOR's table integrates the anchor points correctly. A comparison of
   our Zr enthalpy against MELCOR's table would fail above 1e-4 relative in
   1093 to 1113 K and above 1e-9 everywhere above it. Not fixed here; the
   OFFBEAT port reproduces its upstream deliberately, so this needs a
   maintainer decision (a MELCOR-faithful table in sembawang, or a
   `with_*` variant).
2. **The MELCOR table is internally inconsistent at 400 K.** Its own cp
   points (281 at 300 K per MATPRO, 302 at 400 K) integrate to 29150, which
   is also what makes its 640 K row (105110) come out; the printed 400 K row
   is 21915, 7235 J/kg low. The 400 K row looks like a typo. A test against
   that row would fail for a reason that is not ours.
3. The MP cp table starts at 273.1 K with 275 (giving 280.72 at 300 K) while
   the enthalpy table implies MATPRO's 281 at 300 K. Immaterial (+28 J/kg
   over 300 to 400 K).

---

## MP-03 Melting points, heats of fusion and densities

**Citation.** Vol. 2 MP §7 MP-RM-50, §8 MP-RM-51, §6.1 MP-RM-40, §6.2.1 to 6.2.3
MP-RM-42.

**Data.** Melting point K / heat of fusion J/kg / density kg/m^3:
Zircaloy 2098 / 2.25e5 / 6500; ZrO2 2990 / 7.07e5 / 5600; UO2 3113 / 2.74e5 /
10960; stainless steel 1700 / 2.68e5 / 7930; SS oxide 1870 / 5.98e5 / 5180;
B4C 2620 / 5.00e5 / 2520. The enthalpy tables carry the same fusion steps
(checked: Zr 225000, ZrO2 707000, UO2 274000, SS 268000, SS oxide 598000).

**Worked cases.** A test asserts `h(T_m + 0.01) - h(T_m)` equals the heat
of fusion for each of the five tables (exactly, in the table data), and that
our constants equal the MP values.

**Our implementation.** `core.rs:1045-1048` `ZR_FUSION (2098, 2.25e5)`,
`ZRO2_MELT_K 2990`, `UO2_FUSION (3113, 2.74e5)` (match);
`params.rs:574` `ZRO2_DENSITY 5600` (match); `params.rs:47` `UO2_DENSITY
10412` (95 % of 10960, stated); `params.rs:49` `ZR_DENSITY 6550`
(labelled "MATPRO, about 6550"); `params.rs:530` `ZR_MELT_K 2100` (a
relocation threshold, explained in its doc). Fusion step tested in
`core.rs:2355`.

**Discrepancies.**
1. Zircaloy density: ours 6550, MELCOR MP default 6500 (+0.77 %). Ours is
   sourced to MATPRO, not to MELCOR; worth stating which reference it means
   to follow.
2. The doc comment at `core.rs:1040` cites the MP melting and fusion
   tables as "MP-RM-49/50"; by the page footers they are on MP-RM-50 and
   MP-RM-51 (the MP contents list gives 50 and 51). Off by one page.

---

## MP-04 Thermal conductivity tables (UO2, Zircaloy, ZrO2)

**Citation.** Vol. 2 MP §5.1.1 MP-RM-30 (Zircaloy), §5.1.2 MP-RM-30/31 (ZrO2),
§5.1.3 MP-RM-31 (UO2). Linear interpolation, no extrapolation.

**Worked cases** (linear interpolation of the printed tables):

| T (K) | UO2 | Zircaloy | ZrO2 |
|---|---|---|---|
| 600 | 6.079495 | 16.085000 | 2.012000 |
| 1000 | 3.852086 | 21.928800 | 2.170000 |
| 1500 | 2.518235 | 31.511297 | 2.390000 |
| 2000 | 2.179709 | 53.414924 | 2.490000 |
| 2500 | 2.709412 | 58.400000 | 2.490000 |

Edge: below the first point (273.15 K) or above 5000 K, MELCOR does not
extrapolate; a test should assert the clamp or the error, whichever is
chosen.

**Our implementation.** Not used in the core model: `params.rs:63`
`UO2_K = 3.0` constant (tagged [A], "replace with the OFFBEAT port's UO2
conductivity"). The lower-head steel uses the Kim 304L correlation
(`core.rs:399`), not MELCOR's SS table.

**Discrepancy.** The constant 3.0 W/(m K) is 0.49x MELCOR's value at 600 K and
1.38x at 2000 K. Stated as an assumption in the code, so not a hidden defect;
recorded for completeness.

---

## SPR-01 Spray droplet drag coefficient

**Citation.** Vol. 2 SPR §2, Eqs (2-1) to (2-4) SPR-RM-6, (2-5) to (2-7)
SPR-RM-7. Vol. 1 SPR SC 3000 and 3001, SPR-UG-15/16.

**Equations.** `C_d = 27 Re^-0.84` (Re < C3000(4)), `0.271 Re^0.217` (to
C3000(8)), `2` above. The breakpoints are where the pieces meet:
`C3000(4) = (27/0.271)^(1/(0.217 + 0.84))`, `C3000(8) = (0.271/2)^(-1/0.217)`.
Mass rate (C3001): `dm/dt = -2 pi rho_g D (1 + 0.25 Re^(1/2) Sc^(1/3)) D_c ln(1 + B)`.

**Worked cases.** C3000(4) = **77.737155** (Vol. 1 prints 77.737154);
C3000(8) = **10006.443455** (Vol. 1: 10006.443). At Re = 77.737154 both
pieces give 0.697005. The rounded breakpoint 78 in the Vol. 2 text leaves a
small jump (0.695031 vs 0.697515). At Re = 10006.443, 0.271 Re^0.217 =
2.000000. Re = 1: 27; Re = 1000: 1.213303.

**Our implementation.** Not implemented. The drywell spray is an equilibrium
bound (`bwr/containment.rs:1563` `spray_condensation`, which states it
does not hold the SPR manual's droplet model).

**Discrepancy.** None (nothing built). Useful only if the droplet model is
ported.

---

## Notes on scope

- Not itemised: generalized release model (2-24 to 2-31), VANESA, hygroscopic
  (Mason) growth, flashing-jet impaction, spray washout (2-115 to 2-130),
  chemisorption, iodine pool model reactions (2-183 to 2-192), turbulent
  deposition in pipes and bends. None is implemented in the code inspected;
  they are candidates if the code grows.
- Class map (Vol. 2 Table 2.1, RN-RM-12): 17 default classes; our
  `rn_volume.rs` tracks Cs (CsOH), I2, Te with Xe as a permanent gas. Consistent
  with the class table for those members.
