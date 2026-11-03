# RELAP5 geometry-101/110 heat-transfer selection: V&V record

*GitHub issue #1063 (part of #1056). Written 2026-11-01. AI draft,
unreviewed. Research, education and V&V only: not for facility operation,
licensing, or safety decisions.*

Code: `crates/tampines/src/heat_transfer/relap5_htc.rs` (dispatcher, and the
Seban–Shimazaki, Inayatov and Kazimi–Carelli forms) and
`crates/tuas_boussinesq_solver/src/lib/heat_transfer_correlations/nusselt_number_correlations/free_convection_correlations.rs`
(Churchill–Chu, Grashof, Rayleigh), with the new `NusseltCorrelation`
variants `DittusBoelter` and `ChurchillChuVerticalPlate`.

## What is checked, and what is not

This is **verification** only: each correlation is implemented as its source
paper writes it, and the dispatcher selects as the RELAP-7 Theory Manual
(§2.2.2.1–2.2.2.2, Eqs 45–51) states RELAP5 does. Nothing here is compared
against experimental data or against RELAP5 or RELAP-7 output (no code or
data from either was used). Whether the correlations represent a given
experiment is outside this record.

## Methodology

- **Reference values.** Each pin is computed by hand from the original
  paper's equation (calculator arithmetic written in the test's doc comment),
  then re-evaluated independently in IEEE double with the platform `pow`, and
  the test compares at `max_relative = 1e-10`. The code uses PETIR's
  `r_powf`, which differs from the platform `pow` by round-off only.
- **Analytic limits** are pinned exactly or near-exactly: Churchill–Chu at
  `Ra = 0` is `0.825^2 = 0.680625` for any `Pr`; as `Pr` grows without bound
  it tends to `(0.825 + 0.387 Ra^(1/6))^2`; Seban–Shimazaki at `Pe = 0` is 5.
- **Regime switches** are tested from both sides of each boundary: the
  laminar/turbulent crossover `Re* = (4.36 / 0.023 / Pr^n)^1.25`, free
  convection winning at low `Re` and high `Gr`, the `Pr = 0.1` liquid-metal
  switch, and both `P/D` clips.
- **The `uom` wrapper** is pinned against hand-formed `Re`, `Pr`, `Gr` and
  `h = Nu k / D_h` for a water-like fluid.

Correlation sources: Dittus and Boelter (1930), Univ. Calif. Publ. Eng. 2,
443–461 (see also Winterton 1998, Int. J. Heat Mass Transfer 41, 809);
Churchill and Chu (1975), Int. J. Heat Mass Transfer 18, 1323–1329;
Seban and Shimazaki (1951), Trans. ASME 73, 803–809; Inayatov (1975), Heat
Transfer Soviet Research 7(3), 84–88; Kazimi and Carelli (1976), Westinghouse
report CRBRP-ARD-0034 (the "Westinghouse correlation", as reproduced in
Todreas and Kazimi, *Nuclear Systems I*). The Kazimi–Carelli identification
is made from the coefficients and the `1.1 <= P/D <= 1.4` range, which match
the manual's Eq. 51 exactly; the manual itself does not name the source, and
the identification should be confirmed against the report.

## Predictions (written before the first run)

- Every pin agrees to better than 1e-12 relative.
- Laminar/turbulent crossover at `Re* = 703.39` for `Pr = 1` (heated) and
  `265.86` for `Pr = 7`.
- At `Re = 1e4`, `Pr = 0.1` gives Dittus–Boelter `14.512` and `Pr = 0.0999`
  gives Seban–Shimazaki `11.2747`: a 22 % drop across the switch.

## Results (2026-11-01)

Run with `cargo test --release -p tuas_boussinesq_solver --lib free_convection`
(6 passed) and `cargo test --release -p tampines --lib relap5_htc` (9 passed),
toolchain 1.99.1. All pins passed on the first run at `max_relative = 1e-10`.
To measure the actual agreement, every `1e-10` tolerance was then tightened
temporarily to `1e-15` and the tests rerun: **all still passed**, so every
pin agrees with the IEEE re-evaluation to at most `1e-15` relative (a few
ulp). The prediction (better than `1e-12`) holds. The committed tolerance
stays `1e-10`, to absorb a different platform `pow` in the reference
arithmetic.

| Quantity | Inputs | Hand (calculator) | Code | Test |
|---|---|---|---|---|
| Churchill–Chu `Nu` | `Ra = 0`, any `Pr` | `0.680625` | `0.680625` | `churchill_chu_zero_rayleigh_is_0_825_squared` |
| Churchill–Chu `Nu` | `Ra = 1e9`, `Pr = 0.71` | `122.857` | `122.85653487620696` | `churchill_chu_matches_hand_calculation` |
| Churchill–Chu `Nu` | `Ra = 1e6`, `Pr = 7` | `19.977` | `19.976669857906355` | same |
| Churchill–Chu `Nu` | `Ra = 1e6`, `Pr = 1e12` | limit `22.043025` | within `1e-6` | `churchill_chu_large_prandtl_limit` |
| Grashof | water, 10 K, 0.1 m | `2.0146e7` | `20146087.18236204` | `grashof_number_matches_hand_calculation` |
| Dittus–Boelter, heated | `Re = 1e4`, `Pr = 7` | `79.390` | `79.39022851754193` | `new_nusselt_correlation_variants_dispatch` |
| Dittus–Boelter, cooled | `Re = 1e4`, `Pr = 7` | `65.352` | `65.35175396357931` | same |
| Seban–Shimazaki | `Pe = 100` / `1000` | `5.99527` / `11.27972` | `5.995267926383743` / `11.279716078773951` | `seban_shimazaki_matches_hand_calculation` |
| Kazimi–Carelli | `P/D = 1.2`, `Pe = 100` | `5.05793` | `5.05791656017895` | `kazimi_carelli_matches_hand_calculation_and_clips` |
| Kazimi–Carelli | `P/D = 1.3`, `Pe = 500` | | `8.163613972978698` | same |
| Inayatov bundle | `P/D = 1.3`, `Re = 5e4`, `Pr = 1.5` | `201.97` | `201.9685257588318` | `rod_bundle_applies_inayatov_and_liquid_metal_forms_with_clipping` |
| `h` (geometry 101) | water-like, `u = 1 m/s`, `D_h = 0.02 m` | `4138.9 W/m^2K` | `4138.882524473211` | `wall_heat_transfer_forms_groups_and_h` |

The hand value `5.05793` for Kazimi–Carelli differs from the code's
`5.0579166` by `2.7e-6` relative: calculator rounding of `1.2^3.8` to six
figures, and inside the test's `1e-5` check against the hand value.

Regime switches, all as predicted:

| Boundary | Below | Above |
|---|---|---|
| laminar/turbulent, `Pr = 1`, `Re* = 703.39` | `Re = 700`: laminar, `4.36` | `Re = 710`: turbulent, `4.39273` |
| laminar/turbulent, `Pr = 7`, `Re* = 265.86` | `Re = 260`: laminar | `Re = 270`: turbulent, `4.41426` |
| free convection, `Re = 100`, `Pr = 7` | `Gr = 0`: laminar | `Ra = 1e6`: free convection, `19.9767` |
| liquid metal, `Re = 1e4` | `Pr = 0.0999`: `11.2747` (also with `Gr = 1e12`: no max) | `Pr = 0.1`: Dittus–Boelter, `14.5120` |
| Inayatov `P/D` clip | `None` or `0.9`: `1.1` | `2.0`: `1.6` |
| Kazimi–Carelli `P/D` clip | `1.0` or `None`: `1.1` | `1.5`: `1.4` |

## Findings about the specification

1. **Dittus–Boelter beats the laminar value deep in laminar flow.** The
   turbulent leg is evaluated at every `Re`, as the manual states, so it
   exceeds 4.36 from `Re = 703` at `Pr = 1` and from `Re = 266` at `Pr = 7`.
   This is the stated model; it is recorded, not changed.
2. **The liquid-metal switch at `Pr = 0.1` is discontinuous** (22 % at
   `Re = 1e4` above). Also the stated model.
3. **The workspace's laminar constant is 4.354, not 4.36.**
   `NusseltCorrelation::PipeConstantHeatFluxFullyDeveloped` returns 4.354
   (the exact value is `48/11 = 4.3636`). The dispatcher uses the manual's
   4.36 in its own constant; the existing variant is unchanged.

## Decisions the manual leaves open

Stated in the module docs: `|T_w - T|` in `Gr`; `T_w >= T` counts as heating;
an absent `P/D` defaults to 1.1 for the liquid-metal bundle form too; when the
free-convection length `L` is not `D_h`, the Churchill–Chu value is rescaled
by `D_h / L` so all legs compare the same `h`; published validity ranges are
documented, not enforced.
