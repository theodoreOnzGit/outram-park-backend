# pyDOSEIA port: scoping note

> ⚠️ **Research, education and V&V only.** Not for medical, occupational,
> public-health, emergency-response, licensing or regulatory use, and never a
> dose to a real person or population (workspace `RESPONSIBLE_USE.md`).

Written 2026-09-28, when the port started (maintainer request: "work on
translating pyDOSEIA into buangkok under a module ... cite properly (including
the pubmed paper) and include the usual provenance discipline. same code to
code verification discipline"). Tracking issue: see the end of this note.

## Upstream

| | |
|---|---|
| Code | <https://github.com/BiswajitSadhu/pyDOSEIA>, branch `head`, commit `dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce` (2025-08-26), cloned 2026-09-28 into the gitignored `vendor/pyDOSEIA` |
| Licence | **MIT**, Copyright (c) 2024 Dr. Biswajit Sadhu. MIT code may be ported into this GPL-3.0 crate provided the notice is kept. The notice is reproduced verbatim in [`../NOTICE`](../NOTICE) and cited in every ported file's header |
| Paper | B. Sadhu, T. Sarkar, S. Anand, K. D. Singh, D. K. Aswal, "pyDOSEIA: A Python Package for Radiological Impact Assessment during Long-term or Accidental Atmospheric Releases", *Health Physics* **130**(1) (2026) 94–110, doi:[10.1097/HP.0000000000002014](https://doi.org/10.1097/HP.0000000000002014), PMID [40622262](https://pubmed.ncbi.nlm.nih.gov/40622262/). The article is © 2025 Health Physics Society. It is cited and not reproduced, and the port was written from the **code**, not from the article |

### What upstream contains (9 163 lines of Python)

| File | Lines | Content | This pass |
|---|---:|---|---|
| `metfunc.py` | 1 173 | Met processing (Excel input, TJFD, missing and calm corrections, speed distribution), Pasquill-Gifford sigmas, height correction, single-plume and sector-averaged master equations, dilution factor for four release modes, plume rise and building wake (both marked "TO-DO") | **ported** (tranche 1); plume rise, the D3 mode and building wake added in tranche 2, the last two as labelled divergences. Not ported: the Excel reader and the plots |
| `dosefunc.py` | 1 575 | Inhalation, ground shine, submersion; ingestion (about 740 lines, including specific-activity models for H-3 and C-14 in plants, animals and fish); plume shine (about 350 lines, finite-cloud gamma integration via scipy and joblib) | ~~inhalation, ground shine, submersion **ported**; ingestion and plume shine **deferred**~~ **CHANGED 2026-09-28 (tranche 2): all ported** |
| `raddcffunc.py` | 2 271 | DCF table lookups (age brackets, absorption type, progeny correction), multi-source screening across ICRP 119 / JAERI / DOE-STD-1196 tables, half-life lookup, deposition velocity, weathering, soil density, gamma energies, attenuation coefficients, integration limits for plume shine, a point-source dose helper | ~~multi-source screening, ingestion helpers and plume-shine helpers **deferred**~~ **CHANGED 2026-09-28: all computation ported**; the table file readers are replaced by caller-supplied CSVs |
| `outputfunc.py` | 1 153 | The driver: joblib-parallel loops over distance and age, the text and CSV reports | ~~**not ported**~~ **CHANGED 2026-09-28:** the driver's computation is ported as `assessment::run_assessment` (plain loops in place of joblib) with the report's numbers; the text formatting is not |
| `main.py`, `auto_input_generator*.py`, `auto_input_v18.py` | 2 491 | CLI, the interactive input generator (INGEN), YAML config | CLI and prompts **not ported** (a Rust API and `examples/pydoseia_assessment.rs` replace them); the config's parameter structure and defaults **ported** as `config::PyDoseiaConfig`; `main.py`'s summary tables **ported** |
| `static_utils.py` | 237 | Static copies of the met functions | not ported (duplicates) |
| `test_module.py` | 262 | Upstream's own module tests | read; superseded here by the code-to-code harness |

Parallel processing (joblib) and the `pickle_it` dump are not ported: the
(distance, age) cells are independent, so a plain loop gives the same numbers
(the code-to-code driver groups compare against upstream's joblib run). The
port is pure Rust with no Python at runtime.

## Module layout in `buangkok::pydoseia`

| Rust module | Upstream functions |
|---|---|
| `met` | `file_preprocessing` (after the Excel read), `met_data_to_tjfd`, `missing_correction`, `calm_correction_factor_calc`, `speed_distribution_list` |
| `dispersion` | `sigmay`, `sigmaz`, `height_correction_factor`, `master_eq_single_plume`, `master_eq_sector_averaged_plume`, `dilution_per_sector` (three runnable modes), `synthetic_TJFD_for_single_plume` (inlined), `get_max_dilution_factor` |
| `nuclide` | `convert_half_life_to_seconds` (inside `get_nuclide_info`), the half-life branch of `find_progeny_name_and_yield_f`, `0.693 / T` |
| `dcf` | `inhalation_dcf_list`, `dcf_list_ecerman_ground_shine_include_progeny`, `dcf_list_ecerman_submersion_include_progeny`, `find_progeny_name_and_yield_f` |
| `dose` | `inhalation_dose`, `ground_shine_dose`, `submersion_dose`, `deposition_velocity_of_rad`, `apply_weathering_correction_gs` |
| `units` | new: `EffectiveDose`; `DilutionFactor` re-exported from `changi` |
| `ingestion` (tranche 2) | `ingestion_dose` and its five nested functions, `dcf_list_ingestion`, `fv_list_ecerman_ingestion`, `ingestion_weathering_correction_real`, `ingestion_weathering_correction`, `effective_surface_soil_density_rho`, `zeroing_ingestion`; submodules `food_chain`, `upstream` (faithful driver), `corrected` (divergence) |
| `plume_shine` (tranche 2) | `plumeshine_dose` and its nested integrals, `gamma_energy_abundaces`, `add_zero_energy_for_pure_beta`, `atten_coeff`, `get_k_mu_mua_MFP`, `zyx_lim_for_integral*`, `get_limit_lists_per_rad_for_all_energies`, module-level `point_source_dose` |
| `quadpack` (tranche 2) | SciPy's `__quadpack.c` `dqagse`/`dqk21`/`dqelg`/`dqpsrt` and `tplquad`'s nesting (what `plumeshine_dose` runs on) |
| `dcf_screening` (tranche 2) | `compute_max_dcf`, `get_dcfs_for_radionuclides`, `merge_dataframes_with_source_hc2`, the seven `screen_*` readers |
| `plume_rise` (tranche 2) | `compute_plume_rise_neutral_unstable_cat`, `compute_plume_rise_stable_cat` (+ D6 divergence), `building_wake_effect_gifford` (D5 divergence only) |
| `config` (tranche 2) | the YAML keys, the input generator's defaults, `__init__`'s checks |
| `assessment` (tranche 2) | `dose_calculation_script` and the `agewise_*`/`parallel_*`/`dil_fac_*` helpers, `get_max_dilution_factor`, the totals of `output_to_txt`, `main.reshape_*` and `process_plume_doses_*` |

## Licence and data decisions, per upstream table

Upstream bundles data in `library/` and `met_data/`. Each file's source was
checked before anything was ported (2026-09-28). The rule applied is
`DATA_POLICY.md` together with the crate rule "dose coefficients must be citable
and freely usable; ICRP publications are copyrighted, cite rather than
reproduce". **No upstream data file is copied into the repository.** The port
reads coefficients from user-supplied CSVs in upstream's own column layout.

| Upstream file / sheet | Content | Original source (as upstream states or as identified) | Licence position | Decision |
|---|---|---|---|---|
| `RadioToxicityMaster.xls` / `Inhalation CED Sv per Bq Public` (used by `inhalation_dose`) | age-dependent inhalation e(g), types F/M/S/V | ICRP Publication 72 values (the layout and f1 columns match the IAEA BSS / GSR Part 3 Schedule III reproduction) | ICRP data © ICRP | **not copied**; the code reads a user CSV (`InhalationDcfTable::from_csv`) |
| `RadioToxicityMaster.xls` / ingestion, `All Ci`, `FP, Ci`, `Fresh Fuel Toxicity` | ingestion e(g); inventory worksheets | ICRP 72; an unattributed inventory calculation | ICRP ©; unknown | not copied, not used |
| `Dose_ecerman_final.xlsx` / `surface_dose`, `submersion_dose` | external dose-rate coefficients, six ages | US EPA **Federal Guidance Report No. 15** (prepared by Oak Ridge National Laboratory for EPA; author list not re-checked), *External Exposure to Radionuclides in Air, Water and Soil*, EPA-402/R-19/002 (2019), Table 4-1 (ground surface) and the air-submersion table; the headers in `Dose_GSR3_updated.xlsx` quote "Table 4-1. Reference person effective dose rate coefficients for ground surface" | a US federal report, distributed freely by EPA; usable with citation | not copied in this pass (the port does not need it: the user supplies a table and the code-to-code test uses synthetic ones). Adding FGR-15 later is permitted; take it from the EPA report, not from upstream's spreadsheet, and record provenance |
| `Dose_ecerman_final.xlsx` / `eco_param`, `Dose_GSR3_updated.xlsx` / `ecological_param` | transfer factors, loss rates, deposition | IAEA Safety Reports Series No. 19 (2001), Tables VII–XI | IAEA © | not copied. ~~(ingestion deferred)~~ **2026-09-28:** ingestion ported; the caller supplies the table in upstream's column layout (`ingestion::EcoParamTable::from_csv`); the fixture uses a synthetic one |
| `Dose_ecerman_final.xlsx` / `ingestion_gsr3` | ingestion e(g) | IAEA GSR Part 3 Schedule III (ICRP 72) | IAEA / ICRP © | not copied; caller-supplied (`ingestion::IngestionDcfTable::from_csv`, with upstream's `HTO`/`OBT` rows for tritium); synthetic in the fixture |
| `Dose_ecerman_final.xlsx` / `mass_attenuation_coeff` | photon attenuation coefficients | not stated by upstream (the values look like Hubbell–Seltzer / NIST) | not established. NIST Standard Reference Data can be copyrighted under the Standard Reference Data Act (15 U.S.C. 290e), so "US-government data" is not by itself enough; the terms of this particular table were not checked (no web access to the terms page was attempted in this pass) | **not copied**; caller-supplied (`plume_shine::AttenuationTable::from_csv`); synthetic smooth curves in the fixture |
| `Dose_ecerman_final.xlsx` / `gamma_energy_radionuclide`, `gamma_energy_radionuclide.{ods,xls}` | gamma energies and yields | IAEA (2007) decay data, as upstream states | IAEA terms not established | **not copied**; caller-supplied (`plume_shine::GammaLineTable::from_csv`); synthetic lines in the fixture |
| `dcf_corr.xlsx` | decay chains and branching, used for the progeny correction | "SRS 19 based on ICRP 107", as upstream states | IAEA / ICRP © | **not copied**; the code reads user CSVs (`ProgenyChains::from_csv`) |
| `half_life/*.csv`, `radionuclides_halflife_complete.csv` | half-lives, nomenclature | ICRP 107; JAERI-Data/Code 2002-013 | ICRP ©; JAEA terms | **not copied**. The port takes decay constants as inputs, and the workspace source is `boon-lay` (`try_get_half_life`); `nuclide` only parses upstream's text formats |
| `inhalation_HC2/*`, `ingestion_public/*` | ICRP 119 Annexes F, G, H; JAERI Tables 4, 5, 7; DOE-STD-1196-2011 Table A-2 | as named | ICRP ©; JAEA (terms not established); US DOE (a public standard) | **not copied**. Screening ported (`dcf_screening`); each table is caller-supplied (`ScreeningTable::from_csv`, upstream's renamed headers); synthetic `SCR-*` tables in the fixture. DOE-STD-1196-2011 Table A-2 could be added later from the DOE standard itself, with provenance |
| `met_data/Met_data_5Yr.xlsx` | five years of hourly site met data | **no provenance stated**; it may be facility data, which `DATA_POLICY.md` forbids | unknown | **not copied and not used**; the code-to-code test generates synthetic met records |

## How dose is typed

`uom` 0.38 has no sievert (checked 2026-09-28). A dose is
`pydoseia::units::EffectiveDose`, an `f64` newtype that names its unit in
every accessor and stores millisieverts, the unit upstream computes in, so the
code-to-code comparison stays bit-exact. `AvailableEnergy` (J/kg) was rejected,
as the crate rule requires. `chi/Q` is `changi::activity::units::DilutionFactor`
(s/m^3), shared with `changi`. Lengths, speeds and activities cross the API as
`uom` quantities. Met records and coefficient tables are plain `f64`, because
they mirror upstream's spreadsheet rows, including its `999` / `9` missing-value
sentinels.

## Overlap with `changi`, and how the two fit together

`changi` already has Gaussian dispersion: `changi::puff`, a port of the R
`puff` package, and `changi::activity::chi_over_q` built on it, both with
Pasquill-Gifford sigmas. pyDOSEIA's dispersion is a **different model**: the
BARC/AERB power-law sigma fits, a 16-sector sector-averaged long-term plume,
and TJFD weighting. It is ported **inside** `buangkok` so that it can be
verified code-to-code against pyDOSEIA; it is not unified with `changi`'s. The
dose layer (`pydoseia::dose`) takes a `DilutionFactor`, so a `chi/Q` from
`changi` can feed the same pathways. That wiring, and any use in `htgr_sim_v1`,
is **not** done in this pass.

## ~~Deferred, and why~~ — **CLOSED 2026-09-28 (tranche 2)**

The first tranche deferred six items. All six are now dealt with; the
original reasons are kept, struck through, because they shaped what the
second tranche had to solve (caller-supplied tables for copyrighted data, a
port of the integrator upstream relies on).

| Item | ~~Why deferred~~ | Now |
|---|---|---|
| Ingestion | ~~Largest single pathway; needs the SRS 19 tables (IAEA ©), to be supplied by the user in a defined layout~~ | **ported** (`ingestion`), tables caller-supplied; defects D8-D14 recorded, D8-D11 with a labelled corrected driver |
| Plume shine | ~~Finite-cloud triple integration with joblib; photon data provenance not established~~ | **ported** (`plume_shine`, on a port of SciPy's QUADPACK in `quadpack`); photon tables caller-supplied |
| Multi-source DCF screening | ~~Used only for the DCF report~~ | **ported** (`dcf_screening`); still only feeds the report (D7) |
| Plume rise, building wake | ~~Marked TO-DO, never called, defective (D5, D6)~~ | neutral/unstable rise and the stable rise **ported faithfully**; D6 and D5 corrected as labelled divergences (`plume_rise`) |
| Excel reading, plots, text reports, YAML input generator, joblib driver | ~~I/O and UI, not physics~~ | the driver's computation, the report's totals and the config schema **ported**; Excel, plots, text formatting, prompts and joblib remain **not ported** (I/O and UI) |
| Single plume with met data | ~~Cannot run upstream (D3)~~ | still cannot be verified; a corrected version is provided as a labelled divergence (`dispersion::dilution_single_plume_with_met_speeds`) |

## Every upstream function, and its status (upstream `dca4cdc3`)

Status key: **ported** = faithful and code-to-code verified (group names are
those of `tests/data/pydoseia_reference.csv`); **ported (unit test)** =
faithful but only unit-tested; **divergence** = upstream cannot run or is
clearly wrong, and a corrected, labelled version is provided; **not ported** =
with the reason. Class methods are listed under their file; nested functions
under their parent.

### `metfunc.py` (class `MetFunc`)

| Function | Status | Rust |
|---|---|---|
| `__init__` | ported (the checks) | `config::PyDoseiaConfig::validate` |
| `file_preprocessing` | ported after the Excel read (tranche 1); Excel reading **not ported** (I/O) | `met::preprocess_records` |
| `speed_distribution_list` | ported (`speed_means`) | `met::speed_distribution` |
| `plot_speed_distribution` | **not ported** (plot) | — |
| `met_data_to_tjfd` | ported (`tjfd`) | `met::tjfd_from_records` |
| `synthetic_TJFD_for_single_plume` | ported (inlined; `dilution_no_met`) | `dispersion::dilution_single_plume_no_met` |
| `missing_correction` | ported (`tjfd_missing`) | `met::missing_correction` |
| `calm_correction_factor_calc` | ported with D1; corrected variant | `met::calm_correction_factors`, `..._lowest_speed_class` |
| `sigmay`, `sigmaz` | ported | `dispersion::sigma_y`, `sigma_z` |
| `master_eq_single_plume`, `master_eq_sector_averaged_plume` | ported | `dispersion::master_equation_*` |
| `dilution_per_sector` | ported, 3 modes; the 4th (single plume with met) **divergence** (D3) | `dispersion::dilution_*`, `dilution_single_plume_with_met_speeds` |
| `plot_dilution_factor` | **not ported** (plot) | — |
| `test_single_plume_glc_hukkoo` | ported as a test; its table is reproduced exactly (it never runs upstream: D23) | `tests/pydoseia_code_to_code.rs::upstream_hukkoo_self_test_table_is_reproduced_exactly` |
| `height_correction_factor` | ported | `dispersion::height_correction_factor` |
| `get_max_dilution_factor` | ported (`driver` max_chi) | `dispersion::max_dilution_factor` |
| `compute_plume_rise_neutral_unstable_cat` | ported (`plume_rise`) | `plume_rise::plume_rise_neutral_unstable` |
| `compute_plume_rise_stable_cat` | ported with D6 (`plume_rise`); corrected variant | `plume_rise::plume_rise_stable_upstream`, `plume_rise_stable_both_formulas` |
| `building_wake_effect_gifford` | **divergence** only (D5: cannot run) | `plume_rise::building_wake_gifford` |

### `dosefunc.py` (class `DoseFunc`)

| Function | Status | Rust |
|---|---|---|
| `__init__` | ported (the checks) | `config::PyDoseiaConfig::validate` |
| `inhalation_dose`, `ground_shine_dose`, `submersion_dose` | ported (tranche 1) | `dose::*` |
| `ingestion_dose` | ported with D8-D12 (`ingestion`, `driver`); corrected driver | `ingestion::upstream::ingestion_dose_upstream`, `ingestion::corrected::ingestion_dose_per_nuclide` |
| ↳ `conc_tritium_in_terrestrial_plant`, `conc_tritium_in_terrestrial_animal`, `conc_c14_in_terrestrial_plants`, `conc_c14_in_terrestrial_animal` | ported (through `ingestion`) | `ingestion::food_chain::*` |
| ↳ `conc_c14_in_fish` | **divergence** (reads an undefined `C_DIC`; never called) | `ingestion::food_chain::c14_in_fish` |
| `plumeshine_dose` | ported (`plume_shine_single`, `plume_shine_long_term`, `plume_shine_met`) | `plume_shine::per_class`, `per_sector_with_met` |
| ↳ `adgq_single_plume`, `adgq_sector_average`, `get_all_integral_stab_cat_energy_wise_for_all_rad_parallel` | ported (joblib replaced by a loop) | `plume_shine::kernel_*`, `line_integral`, `line_integrals` |
| `zeroing_ingestion` | ported: pandas 3's no-op (D14, `zeroing`) and the intended zeroing | `assessment::Zeroing`, `ingestion::zero_milk_and_meat` |

### `raddcffunc.py` (class `RaddcfFunc`, and one module function)

| Function | Status | Rust |
|---|---|---|
| `__init__` | ported (the checks) | `config` |
| `get_nuclide_info` (and nested `convert_half_life_to_seconds`) | half-life parsing and `0.693 / T` ported (tranche 1); the look-ups in upstream's ICRP 107 / JAERI / nomenclature files **not ported** (copyrighted data): the caller supplies decay constants (workspace source: `boon-lay`) and alternate names (`dcf_screening::AlternateNames`) | `nuclide::*` |
| `find_half_life_and_decay_const_radionuclides` | not ported as such (data); caller supplies the decay constants | — |
| `screen_Annex_G_...`, `screen_Table_A2_...`, `screen_Table_5_...`, `screen_Table_7_...`, `screen_Table_4_...`, `screen_AnnexF_...` | ported (`dcf_screening`) | `dcf_screening::ScreeningTable::screen` |
| `screen_Annex_H_...` | ported as what it does: nothing (D18, it always fails) | — |
| `merge_dataframes_with_source_hc2`, `get_dcfs_for_radionuclides` | ported (`dcf_screening`) | `dcf_screening::compute_max_dcf`, `assessment::dcf_report` |
| `compute_max_dcf` (first definition) | **not ported**: shadowed by the second definition, so Python never binds it | — |
| `compute_max_dcf` (second definition) | ported with D19 (`dcf_screening`) | `dcf_screening::compute_max_dcf` |
| `inhalation_dcf_list` | ported (tranche 1) | `dcf::InhalationDcfTable::lookup` |
| `dcf_list_ecerman_ground_shine`, `dcf_list_ecerman_submersion` | ported (never called upstream; identical to the parent look-up, verified through the `uncorrected` outputs of `dcf_surface`/`dcf_submersion`) | `dcf::ExternalDcfTable::lookup_exact` |
| `dcf_list_ecerman_ground_shine_include_progeny`, `..._submersion_include_progeny`, `find_progeny_name_and_yield_f` | ported (tranche 1; D4) | `dcf::external_dcf` |
| `reshape_dcfs_with_tritium` | **not ported**: never called (its call is commented out); array shaping for the report | — |
| `reshape_and_pad_dcf` | **not ported**: pads the report's DCF array for printing | — |
| `dcf_list_ingestion` | ported (`dcf_ingestion`) | `ingestion::IngestionDcfTable::lookup` |
| `fv_list_ecerman_ingestion` | ported (`eco_lookup`) | `ingestion::EcoParamTable::lookup` |
| `atten_coeff` (both definitions; identical code, the second binds) | ported (`attenuation`) | `plume_shine::AttenuationTable::air_coefficients` |
| `zyx_lim_for_integral` | ported (never called; `limits_legacy`) | `plume_shine::integration_limits_legacy` |
| `gamma_energy_abundaces` | ported with D17, D24 (`gamma_lines`) | `plume_shine::GammaLineTable::lines_for` |
| `deposition_velocity_of_rad`, `apply_weathering_correction_gs` | ported (tranche 1) | `dose::*` |
| `ingestion_weathering_correction` | ported (never called; `ingestion_weathering_unused`) | `ingestion::ingestion_weathering_correction_unused` |
| `ingestion_weathering_correction_real` | ported (through `ingestion`) | `ingestion::food_chain::effective_removal_rates` |
| `effective_surface_soil_density_rho` | ported (through `ingestion`) | `ingestion::SoilType::surface_densities` |
| `add_zero_energy_for_pure_beta` | ported (`gamma_lines`) | `plume_shine::GammaLineTable::plume_shine_lines` |
| `zyx_lim_for_integral_single_plume`, `..._sector_averaged_plume` | ported (`limits_single`, `limits_sector`) | `plume_shine::integration_limits_*` |
| `get_k_mu_mua_MFP` | ported (`attenuation`) | `plume_shine::AttenuationTable::air_coefficients` |
| `get_limit_lists_per_rad_for_all_energies` | ported (folded into the per-line integral) | `plume_shine::line_integral` |
| `point_source_dose` (method) | **not ported**: cannot run (D15) | — |
| `point_source_dose` (module function) | ported (`point_source`) | `plume_shine::point_source_dose` |

### `outputfunc.py` (class `OutputFunc`)

| Function | Status | Rust |
|---|---|---|
| `__init__` | logging **not ported**; checks in `config` | — |
| `output_to_txt` | the plant-boundary totals ported (`boundary_totals_text`, read back from the text at its printed precision); the text formatting **not ported** | `assessment::plant_boundary_totals` |
| `agewise_dcfs_inh_gs_submersion` | ported with D7, D22 (`driver_dcf_report`) | `assessment::dcf_report` |
| `agewise_dose_inh_gs_submersion` | ported (`driver`) | `assessment::pathway_doses` |
| `agewise_ingestion_dose` | ported with D13 (`driver`, `driver_raises`) | `ingestion::Receiver::from_driver_age`, `assessment::run_assessment` |
| `agewise_plume_shine_dose`, `all_dist_agewise_plume_shine_dose` | ported (the same `plumeshine_dose` per distance; not re-run in the driver groups) | `assessment::run_assessment` |
| `parallel_allage_dose_inh_gs_submersion`, `parallel_allage_dose_ingestion`, `parallel_ps_dose`, `parallel_allage_dcfs_inh_gs_submersion`, `parallel_dilfac_alldists` | ported as plain loops (joblib **not ported**) | `assessment::run_assessment` |
| `dil_fac_all_sectors_all_dist` | ported (`driver`) | `assessment::dilution_for_distance` |
| `dose_calculation_script` | ported (`driver`) | `assessment::run_assessment` |
| `simple_loop_output` | **not ported**: never called (commented out in `dose_calculation_script`), and it writes plots; its numbers are the same pathway calls | — |

### `main.py`

| Function | Status | Rust |
|---|---|---|
| `parse_arguments`, `parse_config`, `convert_none_to_str`, `main` | **not ported** (CLI, YAML I/O); replaced by the Rust API and `examples/pydoseia_assessment.rs` | `config`, `assessment` |
| `process_plume_doses_have_no_met_data`, `process_plume_doses_have_met_data` | ported (unit test: a maximum and its index) | `assessment::plume_shine_maxima` |
| `reshape_ingestion_dose_data`, `reshape_dose_data` | ported with D20, D21 (`driver_summary`) | `assessment::summary_rows` |

### `auto_input_generator.py`, `auto_input_generator_funcs_class.py`, `auto_input_v18.py`

Interactive prompts that write the YAML config: **not ported** (UI). The
parameter structure and every default that feeds a calculation are ported:
`config::PyDoseiaConfig::input_generator_defaults`,
`ingestion::IngestionParameters::INPUT_GENERATOR_DEFAULT`,
`ingestion::DietaryIntake::INPUT_GENERATOR_ADULT`/`_INFANT`. Keys that no
calculation reads are listed by `PyDoseiaConfig::unused_upstream_keys`.
Defects found by reading (not executed): D26.

### `static_utils.py`, `test_module.py`, `test/`

`static_utils.py` (class `SMetFunc`) holds static copies of four `MetFunc`
methods: **not ported** (duplicates). `test_module.py` and the `test/`
configs were read; their Hukkoo-Bapat table is ported as a test (above).

## Tracking

GitHub issue: [#363](https://github.com/theodoreOnzGit/outram-park-backend/issues/363) (pyDOSEIA port tracking issue).
Code-to-code methodology, results and defects:
[`pydoseia-code-to-code.md`](pydoseia-code-to-code.md).
