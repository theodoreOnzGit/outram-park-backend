# Helical-coil once-through steam generator, 1-D march: V&V (gh:#1157)

> Research, education and V&V only. AI draft, unreviewed. Not for facility
> operation, licensing or safety decisions.

Code: `crates/tampines/src/components/helical_coil_otsg/` (`mod.rs` the
march and shooting, `correlations.rs` the closures with their sources,
`tests.rs` the checks below). Demo: `crates/dhoby-ghaut/examples/helical_sg_web/`.

**What this record can and cannot say.** Everything below is
*verification* (the equations are solved as stated) plus one *design-point
comparison* that is not validation, because the geometry it runs on is
partly invented. **No helical-coil boiling data were compared**: the open
candidates found (the SIET/IRIS helical-coil campaign of Santini, Cammi,
Ricotti et al., and its re-analysis in M. Colombo's PoliMi PhD thesis) could
not be reached from this session (the hosts are not on the session's proxy
allow list), so nothing was digitised and nothing was compared. That is the
largest gap in this record.

## 1. Predictions, written 2026-11-03 before any run

Written after the code compiled for the first time and before it was
executed on any case. Not edited after the runs; the results in section 2
are set against them.

| # | Check | Prediction | Why |
|---|---|---|---|
| P1 | Energy balance, `(Q_water − Q_hot)/Q_hot`, Q_hot against the set inlet | `≤ 1e-8` on every converged case | Both sides integrate the same Heun-averaged `q'` per cell, so the balance closes by construction to the shooting residual, `m_h c_p · 1e-6 K` against a duty of order 10⁵ W per tube. The check catches an implementation in which the two sides integrate different heats, not a modelling error |
| P2a | Straight limit, friction, correlation level | exactly `64/Re` (laminar) and Blasius (turbulent) at `d/D_c = 0` | Mishra–Gupta is `64/Re` for `De ≤ 1`; Gnielinski's `ξ` is Blasius plus `0.03 √(d/D_c)` |
| P2b | Straight limit, turbulent Nu, against TUAS's straight-tube Gnielinski with the same Blasius `f` and `Pr_w = Pr` | coil limit higher by `Re/(Re − 1000)`: +11.1 % at Re = 1e4, +2.04 % at 5e4, +1.01 % at 1e5 | the coil form carries `Re` where the straight form carries `Re − 1000`; TUAS's `Pr^0.666667` differs from `2/3` by ~1e-6 relative |
| P2c | Straight limit, laminar Nu | `3.66 + 0.08 √Re Pr^(1/3)`, **not** 3.66: at Re = 1000, Pr = 5, Nu = 7.99, +118 % over 3.66 | Gnielinski's laminar coil form keeps its `Re^0.5` term at zero curvature |
| P2d | Straight limit, whole march, single-phase liquid, against the `without_curvature` ablation | relative differences in friction Δp and duty fall as `(d/D_c)^0.5`; below 1e-2 at `d/D_c = 1e-4` | the only curvature terms left in turbulent flow are the `0.03 √(d/D_c)` in `ξ`, entering friction directly and Nu through `ξ` |
| P3a | Grid convergence, single-phase liquid case | observed order ≈ 2 | Heun is second order and nothing switches |
| P3b | Grid convergence, the full boiling case | observed order ≈ 1 in steam outlet temperature and Δp | the saturation onset, the dryout node, the `Re_crit` friction jump and the transition kink are located only to within a cell |
| P4 | Illustrative HTR-10-like case (`HelicalCoilGeometry::illustrative`, `OperatingPoint::illustrative`) | duty 10.5–12.5 MW; steam out 480–620 °C; helium out 150–260 °C; Δp 50–300 kPa; the tube dries out, at an equilibrium quality between 0.6 and 0.95 | rough hand estimate: outer area 183 m², `U ≈ 450 W/(m²K)` (helium film dominant), NTU on the helium side ≈ 3.7, so the helium leaves cold and the steam superheats beyond the published 440 °C, since neither `h_o` nor the geometry was sized to the design |

## 2. Results (2026-11-03, release build, toolchain 1.99.1)

All numbers from `cargo test --release -p tampines --lib helical_coil_otsg
-- --nocapture`, 200 cells unless stated.

### 2.1 What the first runs found, and what was changed (protocol, not criteria)

The first runs **missed P1 and P3**, and the misses were defects, not
tolerances:

| Run | P1 closure | P3b order (200/400/800) | Cause found | Change |
|---|---|---|---|---|
| 1 | 1.36e-5 (predicted ≤ 1e-8) | −0.84 (does not converge) | dryout snapped to a node: the march's returned hot temperature **jumps** when a guess moves the dryout node by one, so shooting closed its bracket on a discontinuity with residual ≈ 7 mK | dryout located inside its cell |
| 2 | 4.9e-6 | −6.1 | every other switch also snapped to nodes (`x = 1`, where Dougall–Rohsenow hands over to the coil correlation; the friction law and Chisholm's `C` at `Re_crit`); and a scan of the residual against the guess showed ±5 mK **noise** at 2e-7 K steps | general event location: any regime or friction-law switch is bisected inside its cell (to 1e-10 of a cell) and the cell integrated in Heun sub-steps between switches |
| 3 | 4.3e-6 | −0.84 | the noise traced to the **steam tables**: at `(T_sat, p)` the `(T, p)` region test is decided by round-off, flipping between Regions 1, 2 and 4 for pressures 0.37 Pa apart near 4.457 MPa, so the saturated-liquid conductivity's critical-enhancement term took the *vapour* `c_p` about half the time (`λ_f` 0.6187 against 0.6094 W/(m·K), +1.5 %; `λ_g` −1.3 %), and at exactly 3.9 MPa Region 4 **panicked** (`SaturatedTransport::at` crashed the first test run) | fixed at the source, `tampines-steam-tables` `thermal_conductivity::lambda_2_crit_enhancement_term_tp_two_phase_estimate`: on the saturation line below 623.15 K (within 1e-6 of `p_sat`) the caller's `x` picks Region 1 or 2. Regression test `saturated_endpoints_at_3_9_mpa_do_not_panic`. The steam-table library suite still passes (1041 passed, 0 failed, 14 ignored) and its generated V&V tables reproduced byte for byte (only their timestamps changed) |
| 4 | **6.7e-14** | **1.92** | — | — |

**Consequences of the steam-table fix elsewhere, flagged for the
maintainer.** Only `tampines::multiphase_1d` consumes the changed path
(`SaturatedTransport`). Two of its tests moved, and both were updated with
the old record struck through:

- `two_fluid_tests::all_three_ablations_reproduce_the_pre_1060_march_bit_for_bit`:
  a bit-identity pin of the demonstrator blowdown moved by up to 0.98 %
  (max |T_g − T_l|). With only the conductivity file restored to its pre-fix
  content, the current code reproduced the old bits exactly, so the move is
  the fix and nothing else. The pin was re-measured.
- `two_fluid_tests::region_4_safe_pressure_removes_the_upstream_classifier_panic`:
  it pinned 105 unguarded panics over a 10 790-pressure sweep. After the fix
  there is 1, above 16.53 MPa, where the line borders Region 3 and this fix
  does not reach. `two_fluid::region_4_safe_pressure` stays for that range,
  and its doc is corrected.

### 2.2 Against the predictions

| # | Prediction | Result | Verdict |
|---|---|---|---|
| P1 | closure ≤ 1e-8 | 6.7e-14 (default), ≤ 1.1e-10 over all seven ablations and every grid; **missed on runs 1–3** (above) | met after the fixes |
| P2a | exact straight friction limits | `64/Re` and Blasius exactly (`coil_friction_tends_to_the_straight_tube`) | met |
| P2b | Nu ratio `Re/(Re−1000)` | 1.111111, 1.020408, 1.010101 at Re = 1e4, 5e4, 1e5; matches to < 1e-5 relative | met: **the coil correlation's straight limit is the Petukhov form, 11 % above the straight-tube Gnielinski at Re = 1e4** |
| P2c | laminar limit 7.99, +118 % | 7.9859, +118.2 % | met: **Gnielinski's laminar coil form does not reduce to 3.66** |
| P2d | march gaps fall as `(d/D_c)^0.5`, < 1e-2 at 1e-4 | friction gap +3.4e-1, +9.0e-2, +3.0e-2, +9.0e-3, +6.0e-4 at `d/D_c` = 1e-1 … 1e-4, 1e-6: slopes −0.58, −0.48, −0.52 per decade (−0.5 predicted). Duty gap +2.3e-2, +6.3e-3, +2.9e-3, +1.8e-3, +7.7e-4: slower than `√r` below 1e-2, because this case's Re (4 133 to 19 354) crosses `Re_crit` and the laminar and blended Nu legs carry `r^0.194` and `r^0.9` terms | met for friction; duty met at 1e-4 (1.8e-3 < 1e-2) but its rate is not `√r`, explained |
| P3a | order ≈ 2, single-phase | first run: outlet T 2.07, **Δp 1.56 and non-monotone**. The case was not "nothing switches": Re crosses the coil `Re_crit = 7 438`, where the friction law jumps (+5.0 % at `d/D_c = 0.1`). After event location: T 2.001, Δp converged to 1e-2 Pa by 80 cells. A second case kept turbulent throughout (30 tubes, Re 12 398 to 41 956), P3a′: **T 2.001, Δp 2.009** | met after the fix; P3a′ is the clean measure of the scheme |
| P3b | order ≈ 1, boiling | **1.92** (steam out), **2.50** (Δp), **1.93** (duty); 400 vs 800 cells: 0.0022 K and 4.8e-6 relative in Δp | better than predicted, because the prediction assumed node-snapped switches, which run 4 no longer has; the first run (snapped) did not converge at all, worse than predicted |
| P4 | duty 10.5–12.5 MW, steam 480–620 °C, He out 150–260 °C, Δp 50–300 kPa, dryout at x 0.6–0.95 | duty **10.914 MW**; steam out **554.4 °C** at 4.421 MPa; helium out **211.3 °C**; Δp **79.0 kPa** (friction 61.2, gravity 12.6, acceleration 5.3); dryout at s = **27.38 m**, x_eq = **0.921**; peak inner wall **588 °C**; 20 marches to converge | all within the predicted ranges |

Grid study (P3b), steam outlet and Δp:

| cells | steam out [K] | Δp [Pa] | duty [W] | dryout s [m] |
|---|---|---|---|---|
| 25 | 826.861653 | 79035.088 | 10908112.38 | 27.35597 |
| 50 | 827.378603 | 79008.584 | 10912265.16 | 27.37430 |
| 100 | 827.504188 | 79005.062 | 10913274.12 | 27.37850 |
| 200 | 827.536603 | 79010.072 | 10913534.73 | 27.37948 |
| 400 | 827.544855 | 79012.223 | 10913601.10 | 27.37972 |
| 800 | 827.547030 | 79012.604 | 10913618.59 | 27.37980 |

**Which numbers to quote:** the 200-cell values (the default) are within
0.011 K and 3e-5 (Δp) of the 800-cell values; quote those, with the
grid error above. The run-1 to run-3 numbers do not count: those marches
carried the defects in §2.1.

### 2.3 Ablations (illustrative case)

| Ablation | duty [MW] | steam out [°C] | Δp [kPa] | peak inner wall [°C] | dryout s [m] |
|---|---|---|---|---|---|
| default (all on) | 10.9135 | 554.39 | 79.01 | 588.2 | 27.38 |
| `without_curvature` | 10.8194 | 542.77 | 57.45 | 595.6 | 26.96 |
| `without_nucleate_boiling` | 10.9060 | 553.45 | 78.89 | 587.5 | 27.42 |
| `without_subcooled_boiling` | 10.9126 | 554.28 | 78.99 | 588.2 | 27.38 |
| `without_dryout` | 10.9265 | 555.99 | 78.89 | 589.5 | — |
| `without_gravity` | 10.9123 | 554.28 | 66.26 | 588.2 | 27.38 |
| `without_acceleration` | 10.9134 | 554.40 | 73.71 | 588.3 | 27.38 |
| `without_wall_resistance` | 10.9933 | 564.31 | 79.44 | 596.9 | 27.20 |

Reading it: the helium film (`h_o = 500 W/(m²K)`) controls the duty, so the
water-side boiling closures move it by under 0.15 %; curvature carries
27 % of the pressure drop (57.5 → 79.0 kPa) and 0.9 % of the duty. The
peak wall temperature sits near the hot inlet, in the superheater, which is
why dryout barely moves it here; a water-side-limited case (higher `h_o`)
would show the dryout excursion.

### 2.4 Design-point comparison (not validation)

The published HTR-10 point (`docs/reactor-scoping/htr10-plant-data.md`):
helium 700 → 250 °C, steam 440 °C at 4.0 MPa, about 10.0 MW
(`4.3 kg/s × 5193 J/(kg·K) × 450 K` = 10.05 MW). This model, on the
illustrative geometry: helium out 211 °C (−39 K), steam 554 °C (+114 K),
duty 10.91 MW (+8.6 %). **The disagreement is recorded, not tuned away.**
It cannot count for or against the correlations: the tube size, pitch,
helix diameter, tube count and `h_o` are invented (the HTR-10 sources state
none of them), and the published duty fixes the steam outlet only together
with the unpublished geometry. What it does say: with these invented
values the exchanger has more `UA` than HTR-10's, and the peak inner wall
(588 °C) is above the published 500 °C tube design limit, so this invented
geometry would not be an acceptable HTR-10 design either.

### 2.5 Correlations: provenance and verification status

The table and per-function sources are in `correlations.rs`. In short:
Schmidt (1967), the VDI mean curvature diameter, Gnielinski (1986) laminar
and turbulent coil Nu and friction, Mishra & Gupta (1979) laminar friction,
Chen (1966) with the Edelstein et al. (1984) closed forms, Forster & Zuber
(1955), and Lockhart & Martinelli (1949) / Chisholm (1967) were each read in
an open secondary source this session (Modelica Standard Library 4.0.0;
the `fluids` and `ht` Python libraries; a repository record of Mishra &
Gupta), and four are reproduced to 1e-9 against those libraries' published
doctest values. **[U]**: the linear Nu interpolation between `Re_crit` and
2.2e4, and the subcooled-boiling extension of Chen. Reused unchanged and
**not re-checked this session**: Biasi et al. (1967) CHF and
Dougall–Rohsenow (1963), from `outram-foam-multiphase`.

### 2.5a The browser demo (checks, not V&V)

These checks used a full wasm build in headless Chromium (SwiftShader
software GL), at phone width 390×844 and at 1280×800.

- **Convergence:** 2.7 s and 4.9 s respectively, to the native numbers
  (10.91 MW, steam 554 °C).
- **Phone layout:** the panel opens folded. Controls » / « Hide, the
  + / − / Reset buttons (scale bar 100 → 20 cm after four +), and a physics
  checkbox all work.
- **Ablation:** unticking dryout re-solved to 10.93 MW / 556 °C, matching
  the native `without_dryout` row above.
- **No-lag check:** the largest gap between animation frames was 200 ms
  while the worker re-solved, against 150 ms with no solve running.
  SwiftShader painting dominates both, so no solve work reaches the UI
  thread. On a hardware GPU the gaps should be frame-sized; that was not
  measured here.
- **Cosmetic, open:** on a phone the label at the coil's top touches the
  status text.

### 2.6 Geometry drawing (the drawing rule)

`helical_coil_otsg_gh1157_geometry.png`, beside this record, written by
`cargo run --release -p dhoby-ghaut --example helical_sg_web --
--render-geometry`. Every pixel is classified by
`HelicalCoilGeometry::region_at` (distance to the assembled helix the march
integrates along: bore, wall or shell), not drawn from the named
constants. Panels: (A) the whole coil, x–z through the axis; (B) the bottom
turns zoomed, with the pitch measured between two centreline crossings;
(C) the x–y slice at mid-height; (D) the derived dimensions.

**Checked in it (2026-11-03, illustrative coil):** the y = 0 cuts of the tube
in (B) are circles 30.0 mm apart (the pitch, measured from
`centreline_point`), each a bore inside a wall annulus; the bore/wall pixel
ratio is 1.180 against 1.188 for exact circles of 14 and 19 mm (the 0.7 %
gap is the helix angle, sin θ = 0.095, plus pixelation; the test
`render::tests::the_review_image_is_drawn_from_the_assembled_geometry` holds
it within 8 %); turns do not overlap (pitch 30 mm > d_o 19 mm); the coil
starts at z = 0 and its height is 3.240 m with 108 turns, tube length
34.084 m, `D_c` = 100.91 mm, `d_i/D_c` = 0.1387; the mid-height plane cuts
the near-horizontal tube along a crescent, as a helix rising at 0.095 should.
**Not checked:** the shell around the coil (its geometry is not modelled;
the hot side is a constant `h_o`), and any geometry other than the
illustrative default.

## 3. How to re-run
## 3. How to re-run

```
cargo test --release -j 2 -p tampines --lib helical_coil_otsg -- --nocapture
cargo test --release -j 2 -p dhoby-ghaut --example helical_sg_web
cargo run  --release -j 2 -p dhoby-ghaut --example helical_sg_web -- --headless 200
cargo run  --release -j 2 -p dhoby-ghaut --example helical_sg_web -- --render-geometry
```
