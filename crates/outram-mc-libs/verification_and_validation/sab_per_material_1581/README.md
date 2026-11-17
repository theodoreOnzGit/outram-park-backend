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
file itself says a difference under about 2 sigma is not resolved); ~~an
8-seed ensemble was not run (data load 463 s per process on shared cores)~~
**CORRECTED 2026-11-15 (gh:#1583):** a 24-seed ensemble of old and new was
run, see the next section. Flagged for the maintainer rather than hidden.

### HTR-10 pebble ensemble, old vs migrated (gh:#1583)

**Question.** Are the single-seed moves above (-57, +848, +1650 pcm; boron
deltas -422 -> +483 and +623 -> +2330 pcm) only a URR random-stream re-draw
(the nuclide array got longer), or a physics change?

**Method.** `examples/htr10_pebble_delta_tracking.rs` with
`OUTRAM_HTR10_SEEDS=24 OUTRAM_HTR10_HISTORIES=300` (new in #1583; seed loop
outer; master seeds 1..24; the example's own settings, 300 x [20 inactive + 60
active], 300.15 K, delta tracking over the explicit packing, ENDF/B-VIII.0 +
crystalline-graphite and SiC S(a,b)). `BoronReading::all()` is **five** arms;
the single-seed table above showed three. ONE process per build, so the ENDF
load was paid once (505 s old, 512 s new, both running at once). **OLD** =
`d1e20f188` (develop before #1581) in a separate worktree; **NEW** =
`f26d6b1a5` (develop with #1581). Both carry the same seed-loop patch to the
example; nothing else differs in data or settings. Each seed's `k` is that
run's own estimate; the ensemble mean is the mean of 24 seed values, sem =
sd/sqrt(24), the difference has sigma = sqrt(sem_old^2 + sem_new^2). Seed 1
of the old and new builds reproduces the single-seed values of the section
above to every printed digit (natural-B atom ppm 1.68547 old, 1.68490 new), a
check that the harness is the same program.

**Prediction (written on #1583 before any run).** Every arm's new-minus-old
difference consistent with zero, |z| <= 2 on all and |z| <= 1 on at least two;
same for the boron-ablation deltas; resolution about +-500-650 pcm at N = 8
(about +-200 at N = 24, as it turned out: the seed sd was 450-830 pcm, below
the 1100 pcm used for that estimate).

| arm | old mean (sd) | new mean (sd) | new - old +- sigma | z |
|---|---|---|---|---|
| natural B, atom ppm (as specified) | 1.68887 (829 pcm) | 1.68681 (589) | -206 +- 208 pcm | -0.99 |
| natural B, weight ppm (ablation) | 1.68556 (668) | 1.68667 (565) | +111 +- 179 | +0.62 |
| no boron at all | 1.69824 (676) | 1.70078 (561) | +254 +- 179 | +1.41 |
| graphite boron only | 1.68832 (741) | 1.68796 (452) | -36 +- 177 | -0.20 |
| ppm read as elemental B-10 | 1.64348 (669) | 1.64516 (725) | +168 +- 201 | +0.84 |

sem per arm, old / new (pcm): 169/120, 136/115, 138/114, 151/92, 136/148. The
paired same-seed difference gives the same means (the two builds' URR streams
differ, so seeds are not expected to correlate).

Boron-ablation deltas (arm minus natural-atom-ppm arm; difference of arm
means over the 24 seeds):

| delta | old +- sem | new +- sem | new - old +- sigma | z |
|---|---|---|---|---|
| weight ppm - natural | -331 +- 230 | -13 +- 130 | +318 +- 264 | +1.20 |
| no boron - natural | +938 +- 186 | +1397 +- 167 | +460 +- 250 | +1.84 |
| graphite boron only - natural | -55 +- 190 | +115 +- 152 | +170 +- 244 | +0.70 |
| elemental-B-10 misread - natural | -4539 +- 222 | -4165 +- 201 | +375 +- 300 | +1.25 |

**Verdict.** The prediction held. All nine comparisons are inside |z| <= 2, so
the data are consistent with a pure URR re-draw. Four of the five arm
differences are also inside |z| <= 1 (the exception is no-boron at +1.41);
the largest delta is +1.84. Read with care:

1. This is a **null at about +-200 pcm per arm and +-250-300 pcm on a delta,
   not a proof of zero.** A physics shift under about 400-500 pcm (2 sigma)
   would not be seen.
2. Nine correlated comparisons were made; a largest |z| of 1.84 among them is
   not unusual under a zero true difference. But the three largest z (+1.84,
   +1.41, +1.25) all point the same way (new above old) and the five arm
   differences average +66 pcm. That pattern was not in the prediction and was
   not tested; it is recorded, not explained.
3. The new ensemble's seed sd (452-725 pcm) is below the old's (668-829) on
   four of five arms. No cause is claimed and no test was made.
4. The single-seed headline moves (+1650 pcm; +2330 pcm delta) were draws of
   populations with sd 450-830 pcm. For this model at 300 histories, quote
   the 24-seed mean (natural-B atom ppm `1.6868 +- 0.0012`, new), not any
   single seed. The 2026-09-16 6000-history table in the example's doc comment
   was not re-measured and is not comparable to these settings.

**Hardware.** Intel Xeon (cloud container), 4 logical cores, 15 GiB RAM, Linux
6.18, CPU only. The old and new ensembles ran concurrently on one container
(a rayon pool per process, shared cores), so per-call times (60-87 s) carry no
cross-run meaning and are not tabulated. `lscpu` read `@ 2.80GHz` when the
runs started and `@ 2.10GHz` after a later container restart, so the clock at
run time is uncertain; both strings are recorded as read.

**Raw outputs and script:** `htr10_pebble_ensemble_1583/`
(`ens_old_d1e20f188.out`, `ens_new_f26d6b1a5.out`, `analyse.py`;
`python3 -I analyse.py <old> <new>` reproduces every number here).

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

