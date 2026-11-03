# HTR-10 thermal-hydraulic closures vs IAEA-TECDOC-1694

**Generated:** 2026-11-03 (UTC), gh:#1148 (epic gh:#1146)
**Crate version / commit:** `outram-foam-appbuilder-lib` 0.1.6, branched from `a492135b8`
**Code:** `src/htgr_closures/`
**Status:** verified (each closure against hand evaluations, gold values or an independent source). **Not validated** against any HTR-10 measurement. AI-assisted draft pending human review.

> Research, education and V&V only. Not for reactor operation, licensing or any
> safety-critical decision (`RESPONSIBLE_USE.md`).

## Sources and data policy

- **IAEA-TECDOC-1694 (2013), Chapter 2, Tables 2.8-2.13 and Fig. 2.8** (PDF
  pp. 27-29, printed pp. 16-18). Proprietary tier: an IAEA publication with
  no reuse licence, held in the maintainer's private repository as
  `iaea2013tecdoc1694`. Per maintainer decision D1 (2026-11-03) only
  coefficients and formulas enter this repository, each with its table
  number; no text is copied. Every coefficient was re-read on 2026-11-03 with
  `pdftotext -layout` and checked against the page rendered at 300-400 dpi
  (the side-reflector `F1`-`F3` signs and the Table 2.8 exponents in
  particular).
- **Kugeler, Nabielek & Buckthorpe (2017)**, EUR 28712 EN, §9.2.1 eqs.
  (108)-(109). Open (EU reuse decision 2011/833/EU).
  `crates/kovan-literature/reactor-literature/kovan-standard-open-corpus/eu-jrc/kjna28712enn.pdf`.
- **VTB generic pebble-bed deck and tutorial** (open):
  `reference-data/virtual_test_bed/htgr/generic-pbr/pbr.i`,
  `reference-data/virtual_test_bed/doc/content/htgr/generic-pbr-tutorial/step2.md`.
- **CoolProp helium** (`outram-park-fork-coolprop`, MIT upstream), used as an
  independent reference for the helium fits.

## Methodology

Every closure is checked in its own `#[test]`, whose `///` doc comment carries
the methodology and the numbers. The checks are:

| Closure | Reference | Pass criterion |
|---|---|---|
| Breitbach bed conductivity (Tab. 2.9) | Python evaluation of the printed formula, written separately from the Rust | 1e-12 relative |
| Kugeler 2017 eqs. (108)-(109) | Python evaluation | 1e-12 relative |
| Default model | `BedConductivityModel::default()` | must be `Breitbach` |
| KTA 3102.2 Nusselt number (Tab. 2.10) | Python evaluation at `Re = 1e4, Pr = 0.66, eps = 0.39` and at the HTR-10 design point | 1e-9 relative |
| KTA 3102.2 vs Wakao-Funazkri | `Nu = 2 + 1.1 Pr^(1/3) Re^0.6` at the design point | within KTA's stated +/-20 % |
| KTA 3102.3 friction and gradient (Tab. 2.10) | VTB step-2 gold values `psi = 1.983`, `3493 Pa/m` | 0.001 and 1 Pa/m |
| Helium fits (Tab. 2.8), bar reading | Python evaluation; CoolProp at 30 and 60 bar, 300-1700 K | 1e-9; 2.5 % |
| Fuel element, reflectors, carbon brick (Tab. 2.9, 2.11, 2.12) | Python evaluation | 1e-12 (2e-6 where the hand value was printed to four decimals) |
| Steels (Fig. 2.8) | Digitised points; interpolation and no extrapolation | exact at the points |
| Breitbach vs ZBS vs Kugeler | Each other, over 300-1300 K | none: the spread is recorded, not gated |

Inputs of the comparison: HTR-10 bed, `eps = 0.39`, `d = 6 cm`, graphite
emissivity 0.8, ZBS contact fraction 0.0077 and shape factor 1.25 (the
`ZbsBed::htr10()` constants); helium from the Table 2.8 fits at 30 bar; solid
from the Table 2.9 fuel-element conductivity at zero fluence; and, as a
second ZBS column, the VTB deck's `k_s = 26 W/(m K)`. Nothing is tuned.

## Results (measured 2026-11-03, release build)

### Bed conductivity

The Breitbach fit reproduces the hand values to < 1e-12: 1.9538 W/(m K) at
any temperature up to 250 deg C (the source holds `T` at 250 there), then
5.1473 (526.85 deg C), 9.3901 (800 deg C), 13.1079 (1000 deg C) and
19.5715 W/(m K) (1300 deg C). Kugeler's two branches step by +0.34 % at their
1300 deg C joint, a feature of the source's own fits.

Breitbach vs ZBS vs Kugeler (W/(m K)):

| T [K] | k_f He | k_s | Breitbach | ZBS | ZBS (k_s 26) | Kugeler | ZBS/B-1 | ZBS26/B-1 | Kug/B-1 |
|---|---|---|---|---|---|---|---|---|---|
| 300 | 0.1553 | 46.16 | 1.954 | 2.472 | 2.077 | 1.541 | +26.5 % | +6.3 % | -21.1 % |
| 400 | 0.1902 | 46.16 | 1.954 | 3.165 | 2.697 | 1.953 | +62.0 % | +38.0 % | -0.0 % |
| 500 | 0.2227 | 46.16 | 1.954 | 4.049 | 3.490 | 2.612 | +107.2 % | +78.6 % | +33.7 % |
| 600 | 0.2532 | 46.16 | 2.718 | 5.172 | 4.492 | 3.455 | +90.3 % | +65.3 % | +27.1 % |
| 700 | 0.2823 | 46.16 | 3.856 | 6.567 | 5.713 | 4.452 | +70.3 % | +48.2 % | +15.5 % |
| 800 | 0.3102 | 43.09 | 5.147 | 8.118 | 7.145 | 5.587 | +57.7 % | +38.8 % | +8.5 % |
| 900 | 0.3371 | 39.93 | 6.583 | 9.855 | 8.761 | 6.845 | +49.7 % | +33.1 % | +4.0 % |
| 1000 | 0.3631 | 37.52 | 8.156 | 11.763 | 10.520 | 8.219 | +44.2 % | +29.0 % | +0.8 % |
| 1100 | 0.3884 | 35.69 | 9.860 | 13.787 | 12.378 | 9.700 | +39.8 % | +25.5 % | -1.6 % |
| 1200 | 0.4130 | 34.32 | 11.691 | 15.875 | 14.291 | 11.281 | +35.8 % | +22.2 % | -3.5 % |
| 1300 | 0.4370 | 33.33 | 13.644 | 17.990 | 16.222 | 12.959 | +31.9 % | +18.9 % | -5.0 % |

**Breitbach-vs-ZBS spread:** over the HTR-10 operating range (523-1273 K) ZBS
is **32-107 % above** the benchmark's Breitbach fit, largest at 500-600 K where
Breitbach is held at its 250 deg C value, and +32 % at 1300 K. Breitbach and
the open Kugeler fit (same Juelich experiments) agree within +/-10 % from
800 K up but differ by 27-34 % at 500-600 K. ZBS lies above both laboratory
fits everywhere above 300 K; using `k_s = 26` instead of the unirradiated
matrix lowers the excess by 13-29 percentage points without closing it. Both
fits include irradiation, the ZBS column does not.

### `op-jvua`: the analytic ZBS against the VTB 18-point table

The ZBS module (moved unchanged from `tampines`) still measures the
model/table ratio at 0.177 (300 K) to 0.644 (2000 K) when the table is read as
W/(m K). Three new findings:

1. `pbr.i` **defines `keff_pebble_bed` and never uses it**: a grep of the deck
   finds only its definition (line 445). The earlier claim that the VTB model
   "ran with" the table is corrected in the module docs.
2. Read as a dimensionless **`k_eff/k_f`**, the table tracks this ZBS within
   6-11 % at all 18 points: `model / (table * k_f)` = 1.1064 at 300 K, minimum
   1.0615 at 900 K, 1.0947 at 2000 K (pinned by
   `vtb_table_tracks_zbs_when_read_as_k_eff_over_k_f`). With the KTA 3102.1
   helium at 60 bar instead of CoolProp, 1.060-1.131.
3. Against the laboratory fits, the table read as W/(m K) is 6.1x Breitbach at
   300 K and 3.3x at 1000 K, while the analytic ZBS is 1.3-2.1x.

**Outcome:** not fully resolved from upstream (the deck states no units), but
the evidence says the table is not a W/(m K) bed conductivity, and the
analytic ZBS is the more physical of the two. ZBS stays a labelled comparison
variant; the default is Breitbach (D3). Consequence outside this crate:
`htgr_sim_v1`'s passive decay-heat chain uses this table as its bed
`lambda_0` (`decay_heat_removal.rs`), so its bed leg is likely several times
too conductive. That number is not re-measured here (validation of
`htgr_sim_v1` is frozen by the maintainer).

### KTA 3102.2 heat transfer

Hand value at `Re = 1e4, Pr = 0.66, eps = 0.39`: `Nu = 294.742002`, matched to
< 1e-9. HTR-10 design point (4.3 kg/s through the 1.8 m bore, 6 cm pebbles,
helium at 30 bar and 748.15 K from the Table 2.8 fits): `G = 1.689793
kg/(m^2 s)`, `Re = 2685.68`, `Pr = 0.662795`, **`Nu = 123.138`, `alpha =
607.27 W/(m^2 K)`**, inside the stated validity window. Wakao gives
`Nu = 111.47`; KTA/Wakao = **1.1047**.

### KTA 3102.3 pressure drop

`psi = 1.983395` (VTB 1.983), gradient **3493.167 Pa/m** (VTB 3493). Same
digits the `tampines` copy recorded on 2026-08-11; it is now this code, and
`tampines::gas_phase::KtaBed` calls it.

### Helium (Table 2.8, pressure in bar)

At 30 bar and 523.15 K: `rho = 2.740605 kg/m^3`, `eta = 2.938851e-5 Pa s`,
`lambda = 0.229875 W/(m K)`, all equal to the hand values to < 1e-9. Against
CoolProp over 300-1700 K at 30 and 60 bar the worst differences are density
0.042 %, viscosity 1.07 %, conductivity 2.16 % (both at 300 K, 60 bar); from
500 K up, conductivity is within 0.82 %. The Pa reading the table prints gives
378 kg/m^3 at 3 MPa and 523 K, 138 times too dense: the unit is a defect in
the table (maintainer decision D5), and the code reads bar.

### Solids (Tables 2.9, 2.11, 2.12)

All match hand values (see `tecdoc1694_fits_match_hand_evaluations`):
fuel element 46.16 (450 deg C, D 0), 33.56 (1000 deg C, D 0), 22.38
(1000 deg C, D 2.5e21) W/(m K); top/bottom reflector 112.54 (20 deg C), 51.29
(1000 deg C); carbon brick 5.0-8.0 over 0-1000 deg C.

**Side reflector.** `log` is read as log10 (with ln the conductivity is
negative above 40 deg C). The dose factor `F` at zero fluence is 0.987-1.000
over 300-700 deg C and 1.156 at 1000 deg C, but 0.742 at 250, 0.020 at 200 and
-0.528 at 20 deg C: the printed fit has a pole near 130 deg C despite its
stated 0-2000 deg C range. The code **refuses it below 300 deg C** (a cut-off
chosen here from this table, not printed in the source).

### Steels (Fig. 2.8), digitised, **Uncertain**

Protocol in `structural_materials.rs`. Core barrel (SA240/SS316): 13.31 W/(m K)
at 293 K rising linearly to 23.55 at 1070 K. RPV (SA508/SA516-70): 37.74 at
290 K, peak 38.66 near 400 K, 35.40 at 700 K. Read to about +/-0.3 W/(m K).
**SA387-11 does not appear in Fig. 2.8**; no curve is provided for it.

## What is Uncertain

- The side-reflector `log` base (log10 inferred) and the 300 deg C refusal.
- The fluence unit: the table writes only "[10^21]"; per cm^2 is assumed and
  the energy threshold is not stated.
- The digitised steel curves (+/-0.3 W/(m K)).
- The Breitbach fit takes temperature only; the parametric
  Breitbach-Barthels model (`d`, `eps`, emissivity) is **not** implemented:
  no checkable source for its coefficients was available.
- The `op-jvua` `k_eff/k_f` reading of the VTB table is a hypothesis with a
  6-11 % residual.
