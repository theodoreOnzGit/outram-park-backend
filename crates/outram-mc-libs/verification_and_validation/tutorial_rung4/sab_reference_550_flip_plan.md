# Flip the ENDF-route S(a,b) default to THERMR's own incident-energy grid: plan and predictions (GitHub #550, 2026-11-19)

Research, education and V&V only. **This file is committed BEFORE the default is
changed and before any number below is re-measured.** A prediction that misses
stays in place; outcomes go in `sab_reference_550.md` ("Flip and re-measure").

Maintainer decision, 2026-11-19: "Flip and re-measure". The defect and the
node-grid constructors are in `sab_reference_550.md`.

## What gets flipped (read from the code, 2026-11-19)

`ThermalScattering` in `src/material/thermal.rs` has two default ENDF-route entry
points that both end in the log-spaced body:

- `from_tape` -> `from_tape_with_grids(.., N_EMIT_GRID, N_OUTGOING)`
  (`from_endf_file`, `from_leapr` and `from_embedded_leapr_deck` all call it);
- `from_tape_with_options` -> `from_tape_with_grids_and_options(..)`
  (`from_endf_file_with_options`, the LTHR=3 mixed-elastic path of gh:#634).

Neither is in a kovan `review.md`: `src/material/review.md` holds zero
`kind = "review"` blocks (checked 2026-11-19; `njoy-outram-park-fork` is not
edited). The flip is therefore **two one-line changes** (the issue text said
one: it is two, because `from_tape` does not route through
`from_tape_with_options`). `from_tape_with_grids[_and_options]` stay as the
explicit, visible log-spaced ablation. The LEAPR-regenerated laws flip with them
(they build a tape and call `from_tape`).

## Consumers of the default ENDF thermal route (grep, 2026-11-19)

Library: `material/thermal.rs` (the constructors), `processed.rs` (shipped
processed laws and the round trip), `model/data.rs`, `vv/ugraphite.rs`,
`pebble_beds/{keff_delta,fhr_pebble}.rs`; other crates: `nee_soon`
(`htr10_rmc/data*.rs` and five examples), `dhoby-ghaut`
(`mc_web/processed_cache.rs`).

Examples (transport k): `lct008_pitch_sweep`, `lct008_keff`, `hst009_keff`,
`icsbep_five_route_keff`, `msre_n1_first_criticality`, `fhr_ring_rpt_endf`,
`fhr_pebble_target_at_rest_ablation`, `htr10_*`, `dh_keff_vv`,
`pincell_depletion_vs_openmc`. Kernel-only: `h2o_vs_njoy_thermr`,
`h2o_kernel_vs_njoy_thermr`, `graphite_vs_njoy_thermr`,
`graphite_kernel_vs_njoy_thermr`, `graphite_sab_angle_and_width_vs_njoy_thermr`,
`thermal_emission_grid_convergence`, `thermal_fixed_point_sweep`,
`thermal_kernel_keff_worth`.

Tests that build a law through the default path:
`thermal_kernel_stationary_distribution` (recorded fixed points 604.17 K
graphite, 292.96 K H2O), `thermal_laws_vs_njoy_thermr`, `thermal_standards`,
`thermal_h2o_leapr_vs_tsl`, `thermal_graphite_elastic`,
`thermal_lthr3_mixed_634`, `thermal_endf_route_xs_vs_njoy`,
`thermal_cutoff_is_tape_emax`, `light_water_thermal_scattering`,
`triso_shell_thermal_scattering`, `htr10_graphite_thermal_scattering_pebble_bed`,
`material_sab_462_transport`, `total_fast_path_matches_full`,
`speed_tier_fast_is_exact`, `majorant_bounds_endf`, `temperature_precedence`,
`elastic_xi_h1_o16`, `processed_evaluation_round_trip`, `openmc_xs_details_463`,
`openmc_notebooks/pincell`, and `thermal_emission_on_thermr_grid_550`.

Unchanged by construction: sigma_inel (the `xs_e`/`xs_sigma` grid is THERMR's own
`calcem` table in both builds; bit-identical, pinned by the 550 test), the
elastic channels, the cutoff.

## Numbers recorded with the log-spaced build, and the plan to re-measure

"Old" is read from the record named; a re-measure uses the record's own protocol
where affordable (shared 4-core VM, 2 heavy processes at a time, ~5 h).
Predictions are signed and sized **before** any run, from the 550 record (arm E
minus arm B at 2.40 cm: +92 +- 47 pcm; 1.63576: +42 +- 55; 1.75: -32 +- 51;
cross sections identical; moments of the node-grid table equal the ACE's).

| id | number | old (record) | re-measure | predicted move |
|---|---|---|---|---|
| N1 | LCT-008 pin cell k, 2.40 cm, case 1, 11 nuclides, tape data | 0.70536 +- 0.00027 (16 seeds, `pitch_sweep.md` ensemble) | 8 seeds, same protocol (5000 x [50+200]) | **+90 pcm** (range +10 .. +170); the OpenMC gap -117 +- 39 should shrink to about -30 |
| N2 | same, 1.63576 cm | 1.06216 +- 0.00031 (16) | 8 seeds | **+40 pcm** (range -40 .. +120), not resolved at 8 seeds |
| N3 | same, 1.75 cm | 1.01104 +- 0.00016 (16) | 8 seeds | **+10 pcm** (range -60 .. +80), not resolved |
| N4 | HST-009 case 1 k-eff (water-reflected solution sphere) | +80 +- 13 pcm over 128 seeds (`icsbep_remeasure_2026_11_12`, default arm) | 32 seeds, 5000 x [40+120] (sem about 26 pcm) | **+40 pcm** (range -30 .. +120): thermal solution, same sign as N1 but a harder spectrum than over-moderated 2.40 cm |
| N5 | LCT-008 full lattice case 1 k-eff (`lct008_keff`) | +172 +- 22 pcm over 32 seeds (same record) | as many seeds as the budget allows (>= 8) | **+40 pcm** (range -30 .. +110): the 1.63576 pitch of N2 is the lattice's own |
| N6 | graphite and H2O kernel widths against THERMR (`graphite_kernel_vs_njoy_thermr`, `h2o_kernel_vs_njoy_thermr`, `thermal_emission_grid_convergence`) | worst width -2.50 %, rms 1.78 % at 384 points (`thermal.rs` docs) | one run each | worst width **within +-1 %** of old, sign unknown: the 64-bin truncation (one-signed) is unchanged; the staircase error (zero-mean) goes, but node spacing 4-10 % is wider than the 384-point log spacing (about 3.6 %), so `select_table`'s statistical-interpolation variance may RISE for graphite |
| N7 | `thermal_kernel_stationary_distribution` fixed points | 604.17 +- 2.0 K graphite, 292.96 +- 1.0 K H2O | the test | within the gates, shift < 1 K for H2O (the relabelling is zero-mean about the detailed-balance fixed point); graphite uncertain, could leave its +- 2 K gate |
| N8 | `thermal_standards`, `thermal_laws_vs_njoy_thermr`, `thermal_graphite_elastic`, `triso_shell_*`, `light_water_*`, `htr10_graphite_*`, `thermal_lthr3_mixed_634` | each test's own gates | the tests | all pass without re-pinning: they gate cross sections, channels, cutoff and loose kernel bands, which the flip does not touch. A failing pin is reported with old/new |
| N9 | `material_sab_462_transport`, `processed_evaluation_round_trip`, `total_fast_path_matches_full`, `speed_tier_fast_is_exact` | bit-identity between two routes that BOTH build through the default | the tests | pass (both sides move together) |

## Not planned for re-measurement (will be listed again in the record)

Pebble-bed and graphite k values (HTR-10 pebble beds, FHR ring RPT, FHR pebble
ablation), MSRE N-1 and NC-1, `sab_per_material_1581` ensembles,
`pincell_depletion_vs_openmc`, `dh_keff_vv`, `icsbep_five_route_keff` (five
routes, multi-hour), the `nee_soon` HTR-10 RMC runs, the shipped processed laws'
sizes, and the OpenMC-notebook pin cell. Each is an hours-long ensemble; the
budget is about five hours on a shared 4-core VM. They are **not measured, and
every number in them was taken with the log-spaced build**; the record carries a
banner saying so. Graphite deserves a flag: its emission staircase is the one most
likely to move a pebble-bed k, and the sign is not predictable from the water
result.
