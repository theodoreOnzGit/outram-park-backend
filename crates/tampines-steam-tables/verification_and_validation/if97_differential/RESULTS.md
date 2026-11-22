# tampines-steam-tables vs CoolProp IF97.h and rust-steam: differential comparison

gh:#1346 (under epic gh:#1336), measured **2026-11-13**. Pinned by
`tests/if97_vs_coolprop_if97.rs` (`cargo test --release -p tampines-steam-tables --test
if97_vs_coolprop_if97`, 0.2 s). Predictions written before any run: `PREDICTIONS.md`
(commit `8fdeeac53`). Provenance and regeneration: `README.md`.

> **AI-assisted draft, not human-reviewed.** Research, education and V&V only; not for
> facility operation, licensing or safety decisions (`RESPONSIBLE_USE.md`).

## Methodology

* **Upstreams, actually run.** (1) CoolProp `IF97.h` v2.4.0, CoolProp/IF97 commit
  `138ec05fd64c60167515f6a4f5448cdd19466d27`, header compiled directly (g++ 13.3, glibc 2.39,
  `-O2`, no fast-math, no FMA). (2) rust-steam v0.2.0, marciorvneto/rusteam commit
  `2b1f74dbed98bcfeaae2d68fa8ebd9d5b4e080b8`. (3) A cross-check only, not committed:
  `pip install CoolProp` 8.0.0 (`PropsSI(..., "IF97::Water")`), plus the `iapws` 1.5.5 and
  `seuif97` 2.3.8 source packages for the coefficient audit.
* **Inputs.** 3666 (T,p) points (T every 30 K 273.15-1073.15, every 100 K to 2273.15, +-1 ulp
  around 273.15, 623.15, 647.096, 863.15, 1073.15, 2273.15 K, a dense block near the critical
  point; p log-spaced 611.213 Pa-100 MPa, 50/100 MPa, p_sat(T) and B23(T) with +-1 ulp and
  +-1e-9 neighbours), 374 + 120 saturation points, 1638 (p,h), 1470 (p,s) and 784 (h,s)
  points (the (h,s) set includes 256 states generated from forward equations and 24
  saturated mixtures, so that every backward region is exercised).
* **Measure.** Relative difference `|a-b|/max(|a|,|b|)`; counts of bit-identical results and
  results within 1 ulp. Specific volume is compared as `v` against `1/rho` (CoolProp returns
  rho), so its bit-identity count carries one extra rounding. Quantities near zero
  (`h`, `s` at 273.15 K, where the IF97 reference state makes them about -41 J/kg and
  -0.15 J/(kg K)) inflate relative differences; this is why Region 1 `h`, `s`, `u` show 1e-12.
* **Point classes kept apart.** A point within 1e-8 (relative) of the saturation curve or of
  B23 (or of the saturated liquid/vapour state for (p,h), (p,s)) is a *boundary point*: the two
  codes round p_sat/B23 differently, so the side is round-off. These are counted, never mixed
  into value statistics. Values are compared only where both codes put the point in the same
  region; region mismatches are counted and listed.

## Results

### What agrees, and how well

| path | outcome (full table in the appendix) |
|---|---|
| (T,p) Region 5, vs IF97.h (705 pts x 9 quantities) | worst 8.7e-16; many bit-identical |
| (T,p) Region 2, vs IF97.h (1804 pts) | v,h,s,u,cp,cv,w,mu worst <= 1.1e-14; **lambda worst 3.1e-11** |
| (T,p) Region 1, vs IF97.h (481 pts) | worst 2.7e-12 (cv); h,s,u 1.1-1.5e-12 are cancellation near the zero of the reference state; lambda 4.6e-12 |
| (T,p) Region 3, vs IF97.h (210 pts, away from critical) | **0 bit-identical**, v 6.3e-6, h 2.0e-6, s 2.1e-6, u 2.8e-6, cv 4.1e-6, w 1.5e-5, cp 3.0e-5, mu 7.1e-6, lambda 6.2e-6 |
| (T,p) Region 3 near critical (87 pts, `|T-Tc|<10 K`, `|p-pc|<3 MPa`) | v 1.4e-2, h 3.6e-3, s 2.6e-3, mu 1.0e-2, **cp 0.98, lambda 0.85** (both at exactly T=Tc, p=pc where cp and lambda2 diverge) |
| saturation: psat(T), Tsat(p), sigma(T) | worst 6.7e-15, 7.1e-15, 3.3e-16 |
| (p,h)->T, (p,s)->T, Regions 1, 2, 3, away from saturation | worst 3.8e-15 (R1, R2), **0 (PS R2, PS R3: 217/217 and 43/43 bit-identical after the fix below)**, PH R3 7.7e-14 |
| (h,s)->T,p, Regions 1, 2, 3 | worst 6.0e-14 after the fix below (T), p bit-identical in R1, R3 |
| (p,h), (p,s) in the dome, interior | T 9.9e-15; mixture s,h,v differ 2e-4 to 8e-4 (see "upstream-side"); x 1.6e-5 |
| rust-steam vs this crate, forward Regions 1, 2, 5 | **v, w bit-identical on every point; h,s,u,cp,cv within 1.1e-15**; psat bit-identical (374/374), Tsat 2.6e-15 |
| rust-steam vs this crate, Region 3 (356 pts) | v 354/356 bit-identical; h,s,u,cp,cv,w within 1 ulp on 343-354 of 356. The largest differences are the 2 rows at T=623.15 K, p=100 MPa, where rust-steam's half-open `..100e6` bound sends the state to its Region 1 (v 1.8e-5, h 3.2e-6, cp 1.3e-3, w 1.2e-3 there). The 2-13 other rows beyond 1 ulp per quantity were **not examined** |

### Defects found and fixed (all verified against three independent sources)

A coefficient audit (every IF97.h table, tolerance 1e-15, nearest literal in this crate's
`src/`, rounding vs transposition decided by digit string) found **seven transposed digits** in
backward-equation coefficients. IF97.h, the `iapws` package and `seuif97` agree with each
other on each of them, and rust-steam agrees where it has the table (2b T(p,s)). None of the
affected functions is kovan-reviewed (`review.md` files hold no `kind = "review"` block).

| table | (I,J) | was | now | relative change of the coefficient |
|---|---|---|---|---|
| T(p,s) 2b | (0,1) | -0.406633261952**38**e3 | -0.406633261958**38**e3 | 1.5e-11 |
| T(h,s) 2b | (0,0) | 0.801496989929**459**e-1 | 0.801496989929**495**e-1 | 4.5e-14 |
| T(h,s) 2b | (1,5) | -0.186552827328**146**e3 | -0.186552827328**416**e3 | 1.4e-12 |
| T(h,s) 2c | (6,8) | 0.273918446626**997**e9 | 0.273918446626**977**e9 | 7.3e-14 |
| T(p,s) 3a | (-5,0) | -0.571527767052**397**e-4 | -0.571527767052**398**e-4 | 1.8e-15 |
| T(p,s) 3a | (1,0) | 0.38406665186**8099**e-1 | 0.38406665186**8009**e-1 | 2.3e-13 |
| p(h,s) 3a | (8,1) | -0.254741561156**755**e4 | -0.254741561156**775**e4 | 7.9e-14 |

Moved numbers (old -> new, same inputs; the pins in the test hold the new values; the new
coefficients are also pinned by `coefficient_pins_gh1346` unit tests beside each table):

| cell | n | bit-identical old -> new | worst relative difference old -> new |
|---|---:|---|---|
| PS/R2/T | 217 | 174 -> 217 | 2.7e-11 -> 0 |
| PS/R2/h | 217 | 78 -> 97 | 2.5e-11 -> 6.6e-16 |
| PS/R2/v | 217 | 96 -> 115 | 7.8e-11 -> 3.8e-16 |
| PS/R3/T | 43 | 17 -> 43 | 1.3e-14 -> 0 |
| HS/R2/T | 192 | 126 -> 173 | 4.4e-14 -> 5.7e-16 |
| HS/R2/p | 192 | 71 -> 110 | 3.5e-13 -> 2.9e-16 |
| HS/R3/T | 20 | 10 -> 13 | 6.0e-14 -> 6.0e-14 (not moved: libm class) |
| HS/R3/p | 20 | 10 -> 20 | 2.5e-15 -> 0 |

The IAPWS verification tables could not detect any of these: the 2b T(p,s) one moves T by
about 1e-11 K, far inside their 9 printed digits.

### Convention differences (not defects; deferred to the maintainer)

1. **T = 623.15 K exactly, p > p_sat.** This crate and rust-steam use Region 3 (inclusive
   `623.15..=863.15`); IF97.h uses Region 1. Both are inside the release's stated ranges, which
   overlap on this isotherm. Using the other side changes v by up to 3.4e-5, h 1.8e-5, s 1.1e-5,
   cp 1.3e-3 (17 test points). I **did not change it** (it would shift the Region 1/3 `(p,h)`
   boundary isotherm everywhere); I tried it, measured, and reverted.
2. **T = 1073.15 K exactly, p <= 50 MPa.** This crate and rust-steam: Region 5; IF97.h: Region
   2. Difference v 1.2e-4, h 2.3e-5, s 1.9e-5, cp 7.4e-4 (47 points). Not changed.
3. **Exactly on p = p_sat(T), 623.15 < T < 647.096 K.** IF97.h returns Region 3 values; this
   crate refuses (the state is two-phase, quality undetermined), which is correct and
   documented. 6 points.
4. This crate answers 158 (p,h) and 122 (p,s) points that IF97.h refuses: the
   Region 5 states, served by in-house Chebyshev fits that are **not IAPWS** (documented in
   `AGENTS.md`). IF97.h has no Region 5 backward equation.

### Where this crate is the less accurate side

* **Region 3 forward properties are not consistent with the Region 3 basic equation.**
  `v_tp_3` is the SR5 backward v(p,T) used directly, and `h,s,u,cp,cv,w` are then evaluated
  from it. IF97.h Newton-refines on the basic Helmholtz equation (tolerance 1e-14). The
  measured gap (2e-6 to 3e-5 away from the critical point, up to 1.4e-2 in v within 10 K and
  3 MPa of it) is the SR5 band, as predicted (P2). It is the same cause for the 6 (p,h) and 6
  (p,s) points whose side of the dome differs, and for the dome x difference 1.6e-5 above 16.5
  MPa. rust-steam has the identical behaviour (it is where the code came from). This is a
  physics-default question (CLAUDE.md: correct physics is the default), so it is filed, not
  changed: changing it moves every Region 3 V&V number in the crate.
* **Metastable steam, dielectric constant and the IAPWS-95 comparison (issue case 5) were not
  run**: IF97.h has no metastable or dielectric model, and the IAPWS-95 route needs the
  `outram-park-fork-coolprop` table of permissible differences.

### Where the upstream is the less accurate side

* **IF97.h mixture properties in the dome** evaluate the saturated-liquid and vapour states at
  `T = T_ph(p, h')` from the backward equation (`Y_pX`, source lines using
  `RegionOutputBackward(..., false, LIQUID)`), not at `Tsat(p)`, so they inherit the backward
  equations' 25 mK band. At p = 3.4 kPa, h = 120 kJ/kg this crate gives s = 417.94189470757
  J/(kg K); linear mixing of IF97.h's own saturation calls (CoolProp 8.0.0 `PropsSI` Q=0, Q=1)
  gives 417.9418947075679 (agreement to 14 digits), while IF97.h's `smass_phmass` gives 418.259
  (7.6e-4 away, about 23 mK of temperature at that state). Likewise s differs 1.3e-3 at a state
  with x = 1.4e-8 from IF97.h's own saturated-liquid value. Not changed here (it is upstream's).
* At exactly p = Pmin = 611.213 Pa, IF97.h returns quality 0 and a wrong s (68 % off) for
  states inside the dome (29 + 41 points, listed as the `*pmin` cells).
* rust-steam `temperature_ph` / `temperature_ps` return a Region 1 backward value for some
  Region 2 states near p = 18-25 MPa (1 of 650 and 2 of 483 compared points, relative
  difference up to 0.19 and 0.35), because it has no Region 3 backward and picks the
  first of two candidate temperatures by region test. This crate agrees with IF97.h there.
  rust-steam also refuses p = 100 MPa exactly above 623.15 K (68 points; this crate fixed the
  same half-open bound on 2026-08-11).

### Prediction scorecard (from `PREDICTIONS.md`, unedited)

| # | predicted | observed | verdict |
|---|---|---|---|
| P1 | Regions 1, 2, 5 worst < 1e-12; v,h,u,s bit-identical on > 50 % | R5 <= 8.7e-16 and R2 <= 1.1e-14 except lambda 3.1e-11; R1 up to 2.7e-12 (cv), h,s,u 1.1-1.5e-12 from reference-state cancellation, lambda 4.6e-12; bit-identity > 50 % for v,h,s,u in R2 and R5, R1 `v` 236/481, `h` 254/481 | **partly wrong**: the 1e-12 bound is exceeded in Region 1 and by lambda. Not localised to a first divergent operation; attributed to libm and summation order and the near-zero reference state, not proven by ablation |
| P2 | Region 3 not < 1e-12; v ~1e-5, h,s,u <= 1e-4, cp,cv,w <= 5e-3 away from critical, up to 5e-2 near it | v 6.3e-6, h 2.0e-6, cp 3.0e-5; near critical v 1.4e-2, cp 0.98 at Tc | right in kind and order; near-critical worse than guessed at the exact critical point |
| P3 | region assignment identical except T=1073.15 and p = p_sat | also **T = 623.15 K exactly** (17 points), and boundary points within 1e-8 of p_sat / B23 (flip side by round-off) | **miss**: I had not looked at the 623.15 K bound; found by the run |
| P4 | rust-steam bit-identical in R1, R2, R5, R3; 100 MPa refusal | R1, R2, R5: v and w bit-identical, h,s,u,cp,cv <= 1.1e-15 (not all bit-identical: `h` 298/514 in R1); R3: v 354/356 bit-identical, exceptions are the 623.15 K / 100 MPa rows above; 100 MPa refusal as predicted (68 points) | **mostly right**; "bit-identical wherever it returns" is wrong for h,s,u,cp,cv at the 1-ulp level (operation order after the `uom` wrapping), not localised further |
| P5 | psat, Tsat < 1e-12, >= 90 % bit-identical psat; sigma < 1e-12 | psat 85/374 bit-identical (6.7e-15), Tsat 62/120, sigma 226/374; all <= 7.1e-15 | accuracy right, **bit-identity prediction wrong** (22 % for psat) |
| P6 | R1, R2, R5 transport < 1e-10; R3 mu <= 1e-4; near-critical lambda >= 1e-2 | R1 mu 3.3e-14, R2 mu 6.5e-16, R5 <= 5.1e-16; lambda R2 3.1e-11 (below 1e-10); R3 mu 7.1e-6; near-critical lambda 0.85 | right |
| P7 | backward T < 1e-12 away from saturation; ~1e-4 within 25 mK of it | away: <= 3.8e-15 (R1, R2), 7.7e-14 (PH R3); within 0.05 K of saturation: 7.7e-5 (PH R1), 3.3e-5 (PS R1) | right |
| P8 | quality agrees < 1e-9 in the dome | **wrong**: x differs 1.6e-5 (R3 dome); mixture s,h,v 2e-4 to 8e-4 | **miss**, located in the upstream (see above) and the Region 3 SR5 gap |
| P9 | (h,s): panics in a minority, no silent disagreement > 1e-6 where both answer | HS dome: T 5.5e-5, p 9.3e-4, x 3.2e-3 (69 points, 2 states IF97.h answers and this crate refuses, 1 reverse); single-phase <= 6.0e-14 | **miss** for the dome: a silent disagreement up to 9.3e-4 in p, not localised this round |
| P10 | not run | not run | deferred |

## Appendix: every cell

| cell | n compared | bit-identical | within 1 ulp | worst relative difference |
|---|---:|---:|---:|---:|
| `HS/R1/T` | 109 | 69 | 77 | 3.537e-15 |
| `HS/R1/p` | 109 | 109 | 109 | 0.000e+00 |
| `HS/R2/T` | 192 | 173 | 188 | 5.684e-16 |
| `HS/R2/p` | 192 | 110 | 168 | 2.910e-16 |
| `HS/R3/T` | 20 | 13 | 13 | 5.986e-14 |
| `HS/R3/p` | 20 | 20 | 20 | 0.000e+00 |
| `HS/R4/T` | 69 | 34 | 34 | 5.477e-05 |
| `HS/R4/p` | 69 | 7 | 9 | 9.324e-04 |
| `HS/R4/x` | 69 | 16 | 25 | 3.242e-03 |
| `PH/R1/T` | 230 | 149 | 176 | 3.812e-15 |
| `PH/R1/s` | 230 | 134 | 153 | 9.155e-14 |
| `PH/R1/v` | 230 | 87 | 183 | 1.169e-14 |
| `PH/R1near_sat/T` | 38 | 10 | 12 | 7.688e-05 |
| `PH/R1near_sat/s` | 38 | 31 | 33 | 8.092e-16 |
| `PH/R1near_sat/v` | 38 | 19 | 36 | 2.082e-15 |
| `PH/R1pmin/T` | 2 | 2 | 2 | 0.000e+00 |
| `PH/R1pmin/s` | 2 | 1 | 1 | 5.699e-16 |
| `PH/R1pmin/v` | 2 | 0 | 2 | 2.168e-16 |
| `PH/R2/T` | 401 | 365 | 385 | 3.085e-15 |
| `PH/R2/s` | 401 | 260 | 354 | 1.921e-15 |
| `PH/R2/v` | 401 | 188 | 356 | 5.960e-15 |
| `PH/R2near_sat/T` | 20 | 3 | 7 | 2.653e-05 |
| `PH/R2near_sat/s` | 20 | 11 | 17 | 4.728e-15 |
| `PH/R2near_sat/v` | 20 | 6 | 13 | 1.675e-14 |
| `PH/R2pmin/T` | 19 | 18 | 18 | 1.654e-05 |
| `PH/R2pmin/s` | 19 | 7 | 14 | 3.404e-06 |
| `PH/R2pmin/v` | 19 | 13 | 18 | 1.660e-05 |
| `PH/R3/T` | 78 | 40 | 40 | 7.680e-14 |
| `PH/R3/s` | 78 | 0 | 0 | 2.332e-04 |
| `PH/R3/v` | 78 | 0 | 0 | 1.213e-03 |
| `PH/R3near_sat/T` | 6 | 3 | 3 | 7.722e-14 |
| `PH/R3near_sat/s` | 6 | 0 | 0 | 3.709e-06 |
| `PH/R3near_sat/v` | 6 | 0 | 0 | 1.427e-05 |
| `PH/R4/T` | 414 | 204 | 228 | 9.923e-15 |
| `PH/R4/s` | 414 | 0 | 0 | 7.588e-04 |
| `PH/R4/v` | 414 | 0 | 0 | 8.323e-05 |
| `PH/R4/x` | 414 | 106 | 180 | 1.560e-05 |
| `PH/R4nearedge/T` | 79 | 36 | 40 | 9.923e-15 |
| `PH/R4nearedge/s` | 79 | 0 | 0 | 1.348e-03 |
| `PH/R4nearedge/v` | 79 | 0 | 0 | 9.002e-05 |
| `PH/R4nearedge/x` | 79 | 18 | 22 | 6.557e-03 |
| `PH/R4pmin/T` | 29 | 29 | 29 | 0.000e+00 |
| `PH/R4pmin/s` | 29 | 0 | 0 | 6.829e-01 |
| `PH/R4pmin/v` | 29 | 0 | 0 | 1.469e-06 |
| `PH/R4pmin/x` | 29 | 0 | 0 | 1.000e+00 |
| `PH/region_agree` | 1322 | 1316 | 0 | 0.000e+00 |
| `PS/R1/T` | 233 | 222 | 233 | 1.846e-16 |
| `PS/R1/h` | 233 | 101 | 185 | 1.377e-15 |
| `PS/R1/v` | 233 | 95 | 221 | 2.789e-15 |
| `PS/R1near_sat/T` | 38 | 26 | 26 | 3.272e-05 |
| `PS/R1near_sat/h` | 38 | 20 | 32 | 4.119e-15 |
| `PS/R1near_sat/v` | 38 | 13 | 32 | 2.231e-15 |
| `PS/R1pmin/T` | 2 | 2 | 2 | 0.000e+00 |
| `PS/R1pmin/h` | 2 | 1 | 2 | 1.247e-16 |
| `PS/R1pmin/v` | 2 | 1 | 2 | 2.168e-16 |
| `PS/R2/T` | 217 | 217 | 217 | 0.000e+00 |
| `PS/R2/h` | 217 | 97 | 184 | 6.622e-16 |
| `PS/R2/v` | 217 | 115 | 205 | 3.801e-16 |
| `PS/R2near_sat/T` | 20 | 8 | 8 | 2.801e-05 |
| `PS/R2near_sat/h` | 20 | 11 | 19 | 3.341e-16 |
| `PS/R2near_sat/v` | 20 | 13 | 20 | 2.037e-16 |
| `PS/R2pmin/T` | 7 | 6 | 6 | 1.389e-05 |
| `PS/R2pmin/h` | 7 | 6 | 6 | 2.858e-06 |
| `PS/R2pmin/v` | 7 | 3 | 6 | 1.394e-05 |
| `PS/R3/T` | 43 | 43 | 43 | 0.000e+00 |
| `PS/R3/h` | 43 | 0 | 0 | 2.954e-04 |
| `PS/R3/v` | 43 | 0 | 0 | 1.204e-03 |
| `PS/R3near_sat/T` | 6 | 5 | 5 | 2.074e-07 |
| `PS/R3near_sat/h` | 6 | 0 | 0 | 1.991e-06 |
| `PS/R3near_sat/v` | 6 | 0 | 0 | 8.957e-06 |
| `PS/R4/T` | 442 | 234 | 265 | 9.923e-15 |
| `PS/R4/h` | 442 | 0 | 0 | 2.086e-04 |
| `PS/R4/v` | 442 | 0 | 0 | 1.425e-04 |
| `PS/R4/x` | 442 | 232 | 278 | 1.157e-05 |
| `PS/R4nearedge/T` | 81 | 36 | 40 | 9.923e-15 |
| `PS/R4nearedge/h` | 81 | 0 | 0 | 6.132e-04 |
| `PS/R4nearedge/v` | 81 | 0 | 0 | 1.448e-04 |
| `PS/R4nearedge/x` | 81 | 34 | 39 | 5.201e-03 |
| `PS/R4pmin/T` | 41 | 41 | 41 | 0.000e+00 |
| `PS/R4pmin/h` | 41 | 0 | 0 | 2.704e-01 |
| `PS/R4pmin/v` | 41 | 0 | 0 | 2.486e-07 |
| `PS/R4pmin/x` | 41 | 0 | 0 | 1.000e+00 |
| `PS/region_agree` | 1136 | 1130 | 0 | 0.000e+00 |
| `RUS/PH/T` | 650 | 645 | 646 | 1.929e-01 |
| `RUS/PS/T` | 483 | 374 | 419 | 3.503e-01 |
| `RUS/R1/cp` | 514 | 351 | 512 | 2.262e-16 |
| `RUS/R1/cv` | 514 | 332 | 514 | 2.214e-16 |
| `RUS/R1/h` | 514 | 298 | 486 | 4.021e-16 |
| `RUS/R1/s` | 514 | 351 | 514 | 2.187e-16 |
| `RUS/R1/u` | 514 | 303 | 503 | 2.701e-16 |
| `RUS/R1/v` | 514 | 514 | 514 | 0.000e+00 |
| `RUS/R1/w` | 514 | 514 | 514 | 0.000e+00 |
| `RUS/R2/cp` | 1870 | 1012 | 1817 | 4.441e-16 |
| `RUS/R2/cv` | 1870 | 607 | 1427 | 1.097e-15 |
| `RUS/R2/h` | 1870 | 928 | 1739 | 4.396e-16 |
| `RUS/R2/s` | 1870 | 1279 | 1870 | 2.220e-16 |
| `RUS/R2/u` | 1870 | 1037 | 1833 | 3.458e-16 |
| `RUS/R2/v` | 1870 | 1870 | 1870 | 0.000e+00 |
| `RUS/R2/w` | 1870 | 1870 | 1870 | 0.000e+00 |
| `RUS/R3/cp` | 356 | 232 | 354 | 1.331e-03 |
| `RUS/R3/cv` | 356 | 227 | 353 | 4.179e-04 |
| `RUS/R3/h` | 356 | 202 | 346 | 3.245e-06 |
| `RUS/R3/s` | 356 | 197 | 354 | 3.164e-06 |
| `RUS/R3/u` | 356 | 204 | 343 | 1.954e-08 |
| `RUS/R3/v` | 356 | 354 | 354 | 1.769e-05 |
| `RUS/R3/w` | 356 | 291 | 354 | 1.159e-03 |
| `RUS/R5/cp` | 752 | 460 | 751 | 3.289e-16 |
| `RUS/R5/cv` | 752 | 349 | 680 | 4.295e-16 |
| `RUS/R5/h` | 752 | 427 | 750 | 2.525e-16 |
| `RUS/R5/s` | 752 | 492 | 752 | 2.216e-16 |
| `RUS/R5/u` | 752 | 473 | 729 | 2.356e-16 |
| `RUS/R5/v` | 752 | 752 | 752 | 0.000e+00 |
| `RUS/R5/w` | 752 | 752 | 752 | 0.000e+00 |
| `RUS/SAT/Tsat` | 120 | 111 | 113 | 2.613e-15 |
| `RUS/SAT/psat` | 374 | 374 | 374 | 0.000e+00 |
| `SAT/Tsat` | 120 | 62 | 71 | 7.095e-15 |
| `SAT/psat` | 374 | 85 | 86 | 6.656e-15 |
| `SAT/sigma` | 374 | 226 | 369 | 3.252e-16 |
| `TP/R1/cp` | 481 | 233 | 420 | 2.884e-13 |
| `TP/R1/cv` | 481 | 324 | 373 | 2.740e-12 |
| `TP/R1/h` | 481 | 254 | 359 | 1.531e-12 |
| `TP/R1/lambda` | 481 | 0 | 2 | 4.634e-12 |
| `TP/R1/mu` | 481 | 23 | 62 | 3.292e-14 |
| `TP/R1/s` | 481 | 361 | 381 | 1.431e-12 |
| `TP/R1/u` | 481 | 314 | 334 | 1.151e-12 |
| `TP/R1/v` | 481 | 236 | 445 | 9.589e-15 |
| `TP/R1/w` | 481 | 355 | 415 | 1.476e-12 |
| `TP/R2/cp` | 1804 | 265 | 606 | 3.452e-15 |
| `TP/R2/cv` | 1804 | 258 | 324 | 1.094e-14 |
| `TP/R2/h` | 1804 | 853 | 1516 | 5.449e-16 |
| `TP/R2/lambda` | 1804 | 644 | 993 | 3.133e-11 |
| `TP/R2/mu` | 1804 | 629 | 1438 | 6.504e-16 |
| `TP/R2/s` | 1804 | 1228 | 1663 | 5.888e-16 |
| `TP/R2/u` | 1804 | 1260 | 1565 | 6.397e-16 |
| `TP/R2/v` | 1804 | 927 | 1708 | 4.390e-16 |
| `TP/R2/w` | 1804 | 1053 | 1676 | 4.819e-15 |
| `TP/R3/cp` | 210 | 0 | 0 | 3.034e-05 |
| `TP/R3/cv` | 210 | 0 | 0 | 4.064e-06 |
| `TP/R3/h` | 210 | 0 | 0 | 2.043e-06 |
| `TP/R3/lambda` | 210 | 0 | 0 | 6.152e-06 |
| `TP/R3/mu` | 210 | 0 | 0 | 7.129e-06 |
| `TP/R3/s` | 210 | 0 | 0 | 2.070e-06 |
| `TP/R3/u` | 210 | 0 | 0 | 2.760e-06 |
| `TP/R3/v` | 210 | 0 | 0 | 6.329e-06 |
| `TP/R3/w` | 210 | 0 | 0 | 1.453e-05 |
| `TP/R3crit/cp` | 87 | 0 | 0 | 9.820e-01 |
| `TP/R3crit/cv` | 87 | 0 | 0 | 4.450e-03 |
| `TP/R3crit/h` | 87 | 0 | 0 | 3.616e-03 |
| `TP/R3crit/lambda` | 87 | 0 | 0 | 8.538e-01 |
| `TP/R3crit/mu` | 87 | 0 | 0 | 1.028e-02 |
| `TP/R3crit/s` | 87 | 0 | 0 | 2.647e-03 |
| `TP/R3crit/u` | 87 | 0 | 0 | 3.262e-03 |
| `TP/R3crit/v` | 87 | 0 | 0 | 1.392e-02 |
| `TP/R3crit/w` | 87 | 0 | 0 | 2.390e-03 |
| `TP/R5/cp` | 705 | 193 | 377 | 5.820e-16 |
| `TP/R5/cv` | 705 | 194 | 354 | 8.725e-16 |
| `TP/R5/h` | 705 | 425 | 607 | 3.314e-16 |
| `TP/R5/lambda` | 705 | 413 | 673 | 6.597e-16 |
| `TP/R5/mu` | 705 | 158 | 520 | 5.078e-16 |
| `TP/R5/s` | 705 | 482 | 655 | 3.519e-16 |
| `TP/R5/u` | 705 | 578 | 623 | 5.010e-16 |
| `TP/R5/v` | 705 | 364 | 684 | 3.519e-16 |
| `TP/R5/w` | 705 | 548 | 702 | 2.895e-16 |
| `TP/T1073.15_other_side/cp` | 47 | 0 | 0 | 7.374e-04 |
| `TP/T1073.15_other_side/h` | 47 | 0 | 0 | 2.286e-05 |
| `TP/T1073.15_other_side/s` | 47 | 0 | 0 | 1.941e-05 |
| `TP/T1073.15_other_side/v` | 47 | 0 | 0 | 1.238e-04 |
| `TP/T623.15_other_side/cp` | 17 | 0 | 0 | 1.331e-03 |
| `TP/T623.15_other_side/h` | 17 | 0 | 0 | 1.774e-05 |
| `TP/T623.15_other_side/s` | 17 | 0 | 0 | 1.053e-05 |
| `TP/T623.15_other_side/v` | 17 | 0 | 0 | 3.436e-05 |
| `TP/region_agree` | 3351 | 3287 | 0 | 0.000e+00 |
