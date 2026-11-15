# S(alpha,beta) per material: the #462 backlog migration (GitHub #1581)

> Unverified until validated; research, education and V&V only (see the
> workspace `RESPONSIBLE_USE.md`). AI-assisted draft: needs human review.

**Generated:** 2026-11-14/15, code at the worktree branch of #1581 (not pushed).
**Hardware:** Intel Xeon (cloud container), 4 logical cores shared between up to
four concurrent runs and cargo builds, 15 GiB RAM, Linux 6.18, CPU only. No
timing here is comparable across runs (timings not recorded on purpose).

## Methodology

`Nuclide::with_thermal_scattering` binds an S(a,b) law to a nuclide, so every
material referencing that nuclide gets it (#462). The replacement is
`assign_material_sab` / `MaterialSab::for_materials_containing`, which appends
a bound copy of the nuclide for the materials that name the table.

For each driver that still used the old call:

1. **Audit** which materials reference each bound entry and whether any should
   be free (the #462 defect). Result: **no LEAK in any file** (table below).
2. **Migrate** where the change is mechanical and the result can be checked.
3. **Re-measure.** A migrated model has a longer nuclide array, and
   `transport_csg.rs:2182` advances the URR stream by `nuclides.len()` (as
   OpenMC does), so a model with a URR nuclide draws another realisation.
   `Nuclide::from_endf_file` attaches URR tables by default, so **every driver
   that loads uranium from ENDF is such a model**; models on the embedded CORE
   data (no URR) are bit-identical.

Prediction, written before the runs (comment on #1581): CORE-data models
bit-identical; URR models differ by a re-draw, ensemble difference consistent
with zero within 2 sigma.

## Results

### Audit (which materials reference each bound entry)

| File | Verdict | Reason |
|---|---|---|
| dh_keff_vv, fhr_unit_cell (+ dhoby dhshort) | safe | graphite slot only in buffer, IPyC, OPyC, matrix, shell; kernel and SiC carbon use the free slot |
| fhr_pebble_target_at_rest_ablation, fhr_ring_rpt_endf | safe | C12G/C13G only in buffer, PyC1/2, matrix graphite, naive-homogenised zone (holds matrix graphite); homogenised TRISO carbon is moved to the free slot on purpose |
| htr10_pebble_delta_tracking, majorant_absorber_price, five nee_soon HTR-10 diagnostics | safe | graphite slot only in pebble graphite, reflector, bricks, bored zone, dummy pebbles; SiC slots only in SiC; rod B4C carbon is the free slot |
| htr10_fuel_zone_kinf (+ dhoby htr10) | safe | carbon only in matrix and homogenised zone; UO2 kernel has none |
| thermal_kernel_keff_worth | safe | single medium, all carbon graphite |
| lct008_keff, _pitch_sweep, _ace_roundtrip, dhoby lct008 | safe | H-1 only in the water, the only material with a `<sab>` card (cases 1, 2, 8) |
| icsbep_five_route_keff | safe | H-1 in lct008 water; in hst009 the solution and water reflector (tank has none); msre carbon only in graphite |
| msre_n1_first_criticality (+ msre_web) | safe | C-12/C-13 only in graphite compositions and end mixes; H-1 only in shield water; salt, INOR-8, basket and rod materials hold neither |
| nee_soon data.rs / data_jobs.rs | safe by slot role | U/O only in kernel; graphite, SiC and rod-metal slots are distinct roles |
| hst009 (dhoby csgrung) | safe | as hst009_keff |
| pincell test | safe | H-1 only in water |
| nuclide-level tests and probes (13 files) | safe | no material is built |

**No LEAK was found.** The old binding was correct by convention in every file.

### Re-measures (old binary vs migrated binary, same data, same seeds)

| Driver | Run | Old | Migrated | Difference | Reading |
|---|---|---|---|---|---|
| thermal pincell test (CORE data, no URR) | 600 x [40+60], 1 seed | 1.40102 +/- 0.00606 | 1.40102 +/- 0.00606 | 0 | bit-identical, as predicted |
| majorant_absorber_price (deterministic table) | no transport | table | table | 0 | output identical, line for line |
| LCT-008 case 1, full 35 nuclides, URR+DBRC | 8 seeds, 2000 x [40+60] | +76 pcm, sem 104 (sd 293) | +98 pcm, sem 122 (sd 346) | +22 +/- 160 pcm | 0.14 sigma, within the predicted 290 pcm |
| dh_keff_vv delta-tracking arm, 300 histories | 8 draws | 1.38638 +/- 0.00418 | 1.38254 +/- 0.00370 | -384 +/- 558 pcm | -0.7 sigma |

The `dh_keff_vv` single draw 1 differs by +2919 pcm (1.37280 -> 1.40199); the
8-draw ensembles show it was one draw of a population with sd about 1100 pcm,
not a shift. The LCT-008 ensembles use fewer histories than the recorded
2026-11-12 run (32 seeds, 4000 x [120+250], +172 +/- 22 pcm), so those two
numbers are not comparable; only the old-vs-migrated pair is. The recorded
values in the files are unchanged.

`htr10_pebble_delta_tracking` (HTR-10 fuel pebble k_inf, 300 x [20+60], ONE
seed per arm, so a re-draw check and **not an ensemble**; the file records no
k pin):

| arm | old | migrated | difference (sigma of the pair) |
|---|---|---|---|
| natural B, atom ppm | 1.68547 +/- 0.00600 | 1.68490 +/- 0.00628 | -57 pcm (0.07) |
| natural B, weight ppm | 1.68125 +/- 0.00620 | 1.68973 +/- 0.00725 | +848 pcm (0.9) |
| no boron | 1.69170 +/- 0.00684 | 1.70820 +/- 0.00670 | +1650 pcm (1.7) |

All three sit inside 2 sigma of the pair, so each is consistent with a
re-draw, but the largest is 1.7 sigma on one seed and the boron-ablation
deltas printed in the file's "vs natural" column moved (-422 -> +483 pcm and
+623 -> +2330 pcm). Those deltas are single-seed noise at 300 histories (the
file itself says a difference under about 2 sigma is not resolved); an
8-seed ensemble was not run (data load 463 s per process on shared cores).
Flagged for the maintainer rather than hidden.

### Not re-measured after the #1581 migration

- `fhr_pebble_target_at_rest_ablation`, `fhr_ring_rpt_endf`,
  `thermal_kernel_keff_worth`, `htr10_fuel_zone_kinf`,
  `lct008_pitch_sweep`, `lct008_ace_roundtrip`, `icsbep_five_route_keff`
  (every case), `msre_n1_first_criticality`, the nee_soon HTR-10 diagnostics
  with transport: each loads uranium from ENDF (14 to 36 nuclides) and the
  RECONR+BROADR+PURR load alone took 13 to 25 minutes per process here
  (4 shared cores), so a seed ensemble of old and new was not affordable.
  Their recorded values stand and each driver's change is the same mechanism
  measured above (nuclide-array length shifts the URR stream). The structure
  is covered by `tests/material_sab_htr10_1581.rs` and
  `tests/material_sab_462_transport.rs`.
- The dhoby-ghaut web files and the digital-twin `msre_web` example were
  migrated where mechanical and **compile only**; they cannot be run here.

### Left marked, not migrated

nuclide-level tests (13 files, nothing to attach to); `region_temperature`
(pins the hand path bit-identical to the `Model` path, which a longer array
would break); `ugraphite_common` and dhoby `ugraphite` (densities indexed by
nuclide-array position); dhoby `htr10` and `dhshort` (materials built
elsewhere); `majorant_bounds_endf`, `majorant_temperature_cover`,
`htr10_graphite_thermal_scattering_pebble_bed`,
`temperature_interpolation_947`; nee_soon `data.rs` / `data_jobs.rs`.
Each file's marker now states its audit and reason.

