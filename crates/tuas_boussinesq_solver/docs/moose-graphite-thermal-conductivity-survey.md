# Graphite thermal conductivity in MOOSE and the Virtual Test Bed — survey (2026-09-28)

A search of the MOOSE framework and the Virtual Test Bed (VTB) for every
graphite thermal-conductivity correlation or data table, compared against what
`tuas_boussinesq_solver` already carries in
`src/lib/boussinesq_thermophysical_properties/solid_database/nuclear_graphite.rs`.
The question behind it was whether any open MOOSE-family source gives a
graphite conductivity with a **stated validity range above 2000 K**, or at
least states the range of the correlations TUAS already uses.

**Short answer: none does.** The highest stated upper limit anywhere in open
MOOSE/VTB is **1800 K** (H-451, MOOSE `ThermalGraphiteProperties`). The one
table that runs to 2000 K (G-348, VTB HTTF deck) is a parabola evaluated past
the 1273 K top of its measured data. The A3 matrix and IG-110 correlations
TUAS uses are not given a range by any open MOOSE/VTB file. The matrix-graphite
models that do carry grade-specific ranges (`ADGraphiteMatrixThermal`, grades
`A3_3_1800`, `A3_27_1800`, `IG_110`) belong to BISON, whose source is not
public.

Companion file: [`moose-thermal-graphite-properties-lgpl.md`](moose-thermal-graphite-properties-lgpl.md)
(the current MOOSE H-451 object, verbatim).

## Method

| | |
|---|---|
| MOOSE | `git clone --depth 1 --branch next https://github.com/idaholab/moose`, HEAD `fad761c91167365c8b0c5ee96aeb2de46ea41fd5` (2026-09-27). Grepped every `*.C *.h *.i *.md *.bib *.py *.csv` for `graphite` (case-insensitive), and separately for pebble-bed closures (`pebble`, `Zehner`, `Breitbach`, `Schlunder`, `Kunii`, `ZBS`, `Tsotsas`, `Kugeler`). History from `gh api repos/idaholab/moose/commits`. |
| VTB | `git clone --depth 1 https://github.com/idaholab/virtual_test_bed`, HEAD `0c1eef21060521247b6d17583be11ce585a35928` (2026-09-27), compared against the vendored copy in `reference-data/virtual_test_bed/` (pinned `f3314028132912e5b02b9040d9e7cc60290fce4b`). Every VTB file quoted below is **byte-identical** in both. |
| Other MOOSE apps | `git ls-remote https://github.com/idaholab/<app>` for `pronghorn`, `sockeye`, `sam`, `bison`, `griffin`: all return **"Repository not found"** (private, NCRC-distributed). Their source was not examined. |
| Retrieved | 2026-09-28. |

GitHub code search was tried first but is limited to the default branch and
rate-limited, so the local clone is what the findings rest on.

## Findings

"Stated range" means a range written in the source itself. "Compared with
TUAS" uses the functions in `nuclear_graphite.rs` as of 2026-09-28.

| # | Grade | Where | Form | Stated range | Fluence | Cited source | Compared with TUAS |
|---|---|---|---|---|---|---|---|
| 1 | **H-451** (petroleum-coke reflector) | MOOSE `modules/solid_properties/src/solidproperties/ThermalGraphiteProperties.C` (current) | `k = 3.28248e-5 T^2 - 1.24890e-1 T + 1.692145e2` W/(m K), T in K | **500-1800 K** (doc page) | none | `nea2018` = NEA/NSC/R(2017)4, MHTGR-350 benchmark | **Not in TUAS.** New grade. Quadratic minimum at 1902 K (k = 50.4), so it rises past its own range. Already excerpted in the companion file. |
| 2 | **G-348** (isotropic, HTTF) | MOOSE, **historical only**: `modules/solid_properties/src/materials/ThermalGraphiteProperties.C`, added `3a6b898ad4` (2022-06-07), replaced by H-451 in `2de3e5f109` (2022-06-20) | `k = 2879.68819 T^-0.52813` W/(m K) (power-law re-fit of McEligot's parabola) | **25-1000 degC** (298-1273 K) | none | McEligot et al., *Thermal Properties of G-348 Graphite*, INL/EXT-16-38241 (2016) | **Not in TUAS.** Removed from MOOSE after 13 days; excerpted below because its header states the measured range and the uncertainty. |
| 3 | **G-348** | VTB `htgr/httf/sam_ring_model/HTTF-SS.i` block `kgraphite` | 18-point `PiecewiseLinear`, 300-2000 K | none in the deck; source says "correlation in OSU-HTTF-TECH-003-R2 Appendix (which is from INL report)" | none | OSU-HTTF-TECH-003-R2 / INL report | **Not in TUAS.** The table is **exactly** the parabola `k = 166.111 - 0.127717 T + 3.71902e-5 T^2` (fit residual 4e-4 W/(m K), measured here). Minimum at 1717 K. Above 1273 K it is the parabola extrapolated past McEligot's measured range, so reaching 2000 K here does **not** mean data exist to 2000 K. |
| 4 | **G-348** | VTB `htgr/mhtgr/mhtgr_sam/MHTGR.i` block `kgraphite` | 11-point `PiecewiseLinear`, 295.75-1274.05 K | table span only | none | none named (McEligot's temperatures) | **Not in TUAS.** These look like McEligot's measured points. Row 2's power law is -5.3 % to +7.3 % off them; row 3's parabola is -3.9 % to +1.0 % off (both measured here). |
| 5 | **A3 matrix** (A3-27, 1800 degC) | VTB `htgr/htr-pm/core-multiphysics/updated_equilibrium_core/pebble_triso.i` block `gmatrix_k` (l. 193-198) | temperature x Maxwell factor x fluence factor | none | yes | none in deck | **Identical, already in TUAS** (`nuclear_graphite_matrix_a3_thermal_conductivity_fluence_dependent` and its high-temperature sibling). |
| 6 | **IG-110** | VTB `htgr/httr/steady_state_and_null_transient/fuel_elem_steady.i` block `IG110_k` (l. 301-306); `full_core_ht_*.i` use the same quadratic as `((k) - 1)/t` so that `AnisoHeatConductionMaterial` can apply a 1 : 0.54 : 0.54 anisotropy tensor | `k = 66.32 - 4.994e-2 T + 1.712e-5 T^2` | none | none | none in deck | **Identical, already in TUAS** (`nuclear_graphite_ig_110_thermal_conductivity_unirradiated`). The 0.54 tensor is a block-homogenisation factor, not an IG-110 property. |
| 7 | **A3_3_1800, A3_27_1800, IG_110** | VTB decks `htgr/gpbr200/pebble_*/gpbr200_ss_bsht_pebble_triso.i`, `microreactors/mrad/*`, `microreactors/gcmr/*`, `microreactors/hpmr_assembly/main.i` | BISON objects (`ADGraphiteMatrixThermal`, `graphite_grade = ...`, `unirradiated_type = 'A3_27_1800'`) | inside BISON | yes (`fast_neutron_fluence`) | inside BISON | **Not reachable**: the correlations live in BISON's closed source; the decks only name the grade. The open route to the same numbers is PNNL-31427 Table 3.3, where TUAS row 5's constants were traced. |
| 8 | "graphite" constants | VTB `pbfhr/mark1/reflector/*/solid.i` (43.73, "evaluated at 650 C"); `htgr/generic-pbr/pbr.i`, `htgr/generic-pbr-tutorial/step*.i`, `htgr/pbmr400/*`, `htgr/htr-pm/*flow-fv_materials.i` (**26.0**, "From NEA report [2]"; 20.8 for channel blocks); `htgr/leu_pulse/ht_20r_leu_fl.i` (0.3014 W/(cm K)); `microreactors/gcmr/balance_of_plant/*` (40); `microreactors/hpmr_h2`, `drum_rotation` (70, "typical value for G348"); `htgr/assembly/solid.i` (15, matrix) | constants | n/a | none | as quoted | Not correlations; nothing to add. Note that **26 W/(m K)** is the solid conductivity behind VTB's ZBS bed table (row 9). |
| 9 | Pebble-bed effective k (ZBS) | VTB `htgr/generic-pbr/pbr.i` block `keff_pebble_bed` (18 points, 300-2000 K); Pronghorn `FunctorPebbleBedKappaSolid` / `PebbleBedKappaSolid` (`solid_conduction = ZBS`, `radiation = BreitbachBarthels`) in `gpbr200`, `generic-pbr-tutorial/step8.i`, `htr-pm`, `pbmr400` decks | table / closed-source object | table span only | none | "calculated from the ZBS correlation" | Table is in the workspace already (`outram_park_digital_twin_engine::htr10::zbs`). The analytic ZBS and Breitbach-Barthels objects are **Pronghorn (private)**. MOOSE `navier_stokes` holds only the fluid-side `FunctorKappaFluid` / `FunctorEffectiveFluidThermalConductivity` and Ergun/KTA drag; no solid-side bed closure. |
| 10 | other MOOSE hits | `modules/electromagnetics/test/src/functions/ElectricalContactTestFunc.C` (graphite **electrical** conductivity 73069.2 S/m); `modules/combined/examples/stochastic/thermomech/graphite_ring_thermomechanics.i` (constants 25 and 100); `modules/heat_transfer/test/.../convective_ht_side_integral.i`; `test/tests/mesh/preparedness/test.i` (dummy 3.0) | test fixtures | n/a | n/a | n/a | Not property data. |

Orphaned bibliography: `modules/solid_properties/doc/content/bib/solid_properties.bib`
(last touched `89a17d8cfe`, 2025-07-17) still lists `mceligot`, `albers`
(Albers, Batty & Kaschak, *High-Temperature Properties of Nuclear Graphite*,
HTR-2008), `parfume`, `stainsby`, `tecdoc1694`, `rochais`, `lopez_honorato` and
`xin_wang_thesis`, and no current MOOSE page cites any of them. They are
leftovers of the 2022 G-348 object (row 2) and of pebble/TRISO objects that
live in private apps.

### What this settles, and what it does not

- **No stated range above 2000 K exists anywhere in open MOOSE/VTB.** The
  above-2000 K gap in TUAS's A3 matrix conductivity (currently extrapolated
  to 3000 K in `nuclear_graphite_matrix_a3_thermal_conductivity_high_temp_*`)
  cannot be closed from these sources. It needs Gontard & Nabielek (1990)
  itself, or PNNL-31427 / Hales et al. (2020) with the measured range read
  off.
- **None of the open sources states a range for TUAS's A3 or IG-110
  correlations.** Rows 5-6 come from decks with no range and no citation.
- **Every quadratic here turns upward inside or just past its range:** H-451
  at 1902 K, G-348 at 1717 K, and **IG-110 at 1458.5 K** (inside TUAS's coded
  300-2000 K window; already recorded in `nuclear_graphite.rs`'s module doc).
  The A3 form's minimum is at 2029.9 K. Row 2 is the one monotonic form. Its
  authors re-fit the G-348 data as a power law precisely so that it would not
  turn upward, but that makes it an extrapolation choice, not data.

---

## Verbatim excerpt A — MOOSE historical G-348 object (LGPL-2.1)

This is the only open MOOSE source found that states **both** a measured range
and an uncertainty for a graphite conductivity fit. It is kept for provenance,
not recommended for HTR-10 (G-348 is the HTTF grade, not A3 or IG-110).

| | |
|---|---|
| Upstream project | MOOSE, <https://mooseframework.inl.gov> |
| Repository | <https://github.com/idaholab/moose> |
| Files | `modules/solid_properties/include/materials/ThermalGraphiteProperties.h`, `modules/solid_properties/src/materials/ThermalGraphiteProperties.C`, `modules/solid_properties/doc/content/source/materials/ThermalGraphiteProperties.md` |
| Commit read | `7e862aa7353b491ea5b00b76c4f7e9266acd93ef` (parent of `25c039275a`, "Convert material objects to user objects", 2022-06-20). Object introduced in `3a6b898ad453d158af7ab7f883b787b32fe57e39` (2022-06-07); conductivity replaced by H-451 in `2de3e5f1093012763898aa91b179303eeac19ed8` (2022-06-20). |
| Copyright | Battelle Energy Alliance, LLC — "All rights reserved, see COPYRIGHT for full restrictions", <https://github.com/idaholab/moose/blob/master/COPYRIGHT> |
| Licence | **LGPL-2.1**, <https://www.gnu.org/licenses/lgpl-2.1.html> (combinable with this GPL-3.0 workspace under LGPL-2.1 section 3) |
| Retrieved | 2026-09-28, `raw.githubusercontent.com` |

Upstream file header (`.C`, unchanged):

```cpp
//* This file is part of the MOOSE framework
//* https://www.mooseframework.org
//*
//* All rights reserved, see COPYRIGHT for full restrictions
//* https://github.com/idaholab/moose/blob/master/COPYRIGHT
//*
//* Licensed under LGPL 2.1, please see LICENSE for details
//* https://www.gnu.org/licenses/lgpl-2.1.html
```

`ThermalGraphiteProperties.h`, the conductivity doc comment:

```cpp
  /**
   * Thermal conductivity (W/m$\cdot$K) as a function of temperature (K).
   * More so than density and specific heat, thermal conductivity is extremely
   * dependent on material and manufacturing details, and no correlation is
   * provided in the review article by Baker \cite baker.
   *
   * Due to the hexagonal lattice structure typical of
   * most graphites, the thermal conductivity is about two orders of magnitude
   * higher in the planar direction than in the perpendicular direction.
   * Measurements of thermal conductivity for three different grades of nuclear
   * graphite produced by GrafTech International Holdings Inc. (PCEA, PCIB-SFG,
   * and PPEA) in the range of 25 to 900 $\degreeC$ show a difference of between
   * 15 and 25 W/m$\cdot$K between the three grades \cite albers. Experimental data is
   * provided by McEligot et. al for isotropic G-348 graphite for
   * 25 $\degree$ C $\le$ T $\le$ 1000 $\degree$ C in the form of a parabolic
   * correlation; this data is re-fit with a
   * power correlation to permit (cautionary) extrapolation to
   * higher temperatures without giving non-physical increases in thermal
   * conductivity at the local minima of the parabolic fit \cite mceligot. The
   * R$^2$ value of this fit is 0.97620. The systematic uncertainty is $\pm$2.7% while
   * the random uncertainty is $\pm$1.5%. At low temperatures, this correlation
   * slightly underpredicts the GrafTech data \cite albers.
   */
  virtual void computeThermalConductivity() override;
```

`ThermalGraphiteProperties.C`:

```cpp
void
ThermalGraphiteProperties::computeThermalConductivity()
{
  _k[_qp] = 2879.68819 * std::pow(_temperature[_qp], -0.52813);
}

void
ThermalGraphiteProperties::computeThermalConductivityDerivatives()
{
  _dk_dT[_qp] = 2879.68819 * -0.52813 * std::pow(_temperature[_qp], -1.52813);
}
```

Documentation page, "Range of Validity":

```markdown
The ThermalGraphiteProperties material is valid for estimating isobaric
specific heat over 200 K $\le$ T $\le$ 3500 K; for estimating thermal
conductivity over 25$\degree$C $\le$ T $\le$ 1000$\degree$C; and for
estimating density over 20$\degree$C $\le$ T $\le$ 2500$\degree$C,
though the error in density estimation increases with
temperature due to the assumption of a constant thermal expansion coefficient.
```

## Verbatim excerpt B — VTB G-348 tables (CC-BY-4.0)

| | |
|---|---|
| Upstream project | Virtual Test Bed (NRIC / Idaho National Laboratory, DOE NEAMS), <https://github.com/idaholab/virtual_test_bed> |
| Files | `htgr/httf/sam_ring_model/HTTF-SS.i` (last upstream commit `b82718b37687b7938ac1c9591badb97c73cd1746`); `htgr/mhtgr/mhtgr_sam/MHTGR.i` (last upstream commit `15fc4129dca7220c54a23bd851bc672022387224`; the vendored copy differs from HEAD only in one solver tolerance, not in these blocks) |
| Also vendored at | `reference-data/virtual_test_bed/` (commit `f3314028132912e5b02b9040d9e7cc60290fce4b`); the HTTF file is byte-identical, and the MHTGR block below is identical |
| Copyright / licence | Idaho National Laboratory / Battelle Energy Alliance; **CC-BY-4.0**, <https://creativecommons.org/licenses/by/4.0/> |
| Retrieved | 2026-09-28 |
| Modifications | none |

`HTTF-SS.i`, lines 362-366:

```
  [kgraphite] #G-348 graphite therm. cond; x- Temperature [K], y-Thermal conductivity [W/m-K] correlation in OSU-HTTF-TECH-003-R2 Appendix (which is from INL report)
    type = PiecewiseLinear
    x = '300      400 500     600     700  800  900  1000 1100 1200 1300 1400 1500 1600 1700 1800 1900 2000'
    y = '131.143 120.975 111.550 102.869 94.932 87.739 81.290 75.584 70.622 66.404 62.930 60.200 58.213 56.970 56.471 56.716 57.705 59.437'
  []
```

`MHTGR.i`, lines 484-488:

```
  [kgraphite] #G-348 graphite therm. cond; x- Temperature [K], y-Thermal condiuctivity [W/m-K]
    type = PiecewiseLinear
    x = '295.75 374.15 472.45 574.75 674.75 774.75 874.75 974.85 1074.45 1173.95 1274.05'
    y = '133.02 128.54 117.62 106.03 96.7 88.61 82.22 76.52 71.78 67.88 64.26'
  []
```

**Analysis (made here, not upstream's):** a least-squares quadratic through
the HTTF table gives `k = 1.66111215e2 - 1.27717421e-1 T + 3.71901832e-5 T^2`
with a maximum residual of 4.2e-4 W/(m K), so the table is that parabola
sampled every 100 K. Its minimum is at 1717 K, and from there it climbs
+5.3 % to 2000 K. That climb is the non-physical upturn the 2022 MOOSE header
(excerpt A) re-fitted to avoid.

## Recommendation for TUAS

1. **No new A3 or IG-110 correlation is available from MOOSE/VTB.** Nothing
   found changes the range statements already in `nuclear_graphite.rs`, and
   the above-2000 K A3 conductivity is still an extrapolation. Primary
   literature is the only way forward (Gontard & Nabielek 1990; PNNL-31427
   Table 3.3; Hales et al. 2020).
2. **Optional new grade: H-451.** Add it only if a consumer needs it (the
   MHTGR-350 / prismatic reflector). It does not serve HTR-10, whose reflector
   is IG-110. If added: `k = 3.28248e-5 T^2 - 1.24890e-1 T + 1.692145e2`
   W/(m K), window **500-1800 K** as stated, cp from Butland & Maddison
   polynomial 3 (already in TUAS), density 1850 kg/m^3 (MOOSE default, cited
   to NEA/NSC/R(2017)4). Source: MOOSE `ThermalGraphiteProperties`, commit
   `9952567b9a`, LGPL-2.1.
3. **Optional new grade: G-348** (HTTF), only if an HTTF consumer appears. Use
   McEligot's parabola (the HTTF table) with a window of **298-1273 K**, the
   range MOOSE's 2022 header states for the data. Do not use the table's
   2000 K end as a range.
