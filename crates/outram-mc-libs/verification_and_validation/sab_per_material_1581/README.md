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

- ~~`fhr_pebble_target_at_rest_ablation`~~ (**done 2026-11-17, gh:#1583
  item 2**, see "Item 2 ensembles" below), ~~`fhr_ring_rpt_endf`~~ (**done 2026-11-17 for its surface-tracked CSG solve only, N = 8; its six other solves are NOT re-measured, see "Item 2, last three drivers" below**),
  ~~`thermal_kernel_keff_worth`~~ (**done**, N = 6),
  ~~`htr10_fuel_zone_kinf`~~ (**done**, N = 12),
  ~~`lct008_pitch_sweep` (partial only, N = 3, not enough to conclude)~~ (**done 2026-11-17, N = 10**, see "Item 2 remainder" below),
  ~~`lct008_ace_roundtrip`~~ (**done 2026-11-17, N = 8, see below**), ~~`icsbep_five_route_keff`
  (every case)~~ (**done 2026-11-17 for the hst009 (N = 16) and msre (N = 8) cases on route `endf`; godiva and jemima carry no S(a,b) and were not run; lct008-full and the ACE routes were NOT run**), ~~`msre_n1_first_criticality`~~ (**done 2026-11-17, N = 8 + 8 replication; seeds 1-8 MISSED the gate, see below**), the nee_soon HTR-10 diagnostics
  with transport (these three and `fhr_ring_rpt_endf` are **still not
  re-measured**): each loads uranium from ENDF (14 to 36 nuclides) and the
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
elsewhere); ~~`majorant_bounds_endf`, `majorant_temperature_cover`,
`htr10_graphite_thermal_scattering_pebble_bed`~~ (**CORRECTED 2026-11-17,
gh:#1635: all three are migrated, markers removed; checked by the
`no_unmarked_nuclide_level_sab_462` scan and the results below**),
`temperature_interpolation_947`; nee_soon `data.rs` / `data_jobs.rs`.
Each file's marker now states its audit and reason.


## Item 2 ensembles: three more ENDF drivers, old vs migrated (gh:#1583, 2026-11-17)

**Prediction (written on #1583 before any run).** Every arm's new-minus-old
difference consistent with zero, |z| <= 2 on all arms and |z| <= 1 on at least
half; the within-build deltas each driver exists to measure also consistent
with their OLD value at |z| <= 2. No sign predicted. Expected resolution
(from the HTR-10 pebble ensemble) a few hundred pcm per arm at N = 8-12, so a
shift under about 700 pcm would be invisible. Any arm beyond 2 sigma would be a
finding, reported and not patched.

**Method.** Same as the HTR-10 pebble section above. OLD = `d1e20f188` (before
#1581); NEW = `7b5dd858d` (the session branch, which holds #1581). Same data,
seeds and settings in both; ALL seeds inside ONE process per build, so each
ENDF load was paid once; OLD and NEW ran at the same time with the other
drivers (up to six processes on 4 cores). sem = sample sd / sqrt(n);
sigma(new - old) = sqrt(sem_old^2 + sem_new^2); z = difference / sigma. Both
|z| <= 1 and |z| <= 2 are reported. A delta is the arm minus the first arm,
formed per seed and then averaged, per build. Script:
`ensembles_1583/analyse.py` (`python3 -I analyse.py <fz|fhr|tk|pitch> old new`
reproduces every number below), raw outputs beside it. The only code change is
a seed-loop knob in two examples (`OUTRAM_FZ_SEEDS`/`OUTRAM_FZ_SEED0` in
`htr10_fuel_zone_kinf`, `OUTRAM_PITCH_SEEDS` in `lct008_pitch_sweep`), applied
identically to both builds; unset it leaves the single-seed program as it was.
`fhr_pebble_target_at_rest_ablation` (`OUTRAM_FHR_SEEDS`) and
`thermal_kernel_keff_worth` (replica argument) already had seed loops.

### `htr10_fuel_zone_kinf` (N = 12, seeds 1-12)

2000 particles x [20 + 60], 293.15 K, default ENDF route (30P graphite,
URR + DBRC), packing seed 20260811, `CpuSingleThread`. The recorded 2026-10-05
numbers are at 10 000 x [50 + 200] and are not comparable per seed.

| arm | old mean (sd) | new mean (sd) | new - old +- sigma | z |
|---|---|---|---|---|
| heterogeneous | 1.57287 (164 pcm) | 1.57302 (279) | +15 +- 93 pcm | +0.16 |
| homogenised | 1.44863 (251) | 1.44751 (268) | -113 +- 106 | -1.06 |
| worth (hom - het) | -12424 +- 64 | -12551 +- 123 | -128 +- 138 | -0.92 |

|z| <= 2 on all three (held); |z| <= 1 on two of three (held; homogenised is
-1.06). **Verdict: consistent with a URR re-draw.** The double-heterogeneity
worth is -12.4 to -12.6 thousand pcm in both builds, against -12452 +- 126 recorded
at 10 000 x 250. Null at about +-100 pcm per arm, +-140 on the worth.

### `fhr_pebble_target_at_rest_ablation` (N = 8, seeds 1-8)

4000 x [30 + 80], 600 K, ring-RPT CSG pebble, multi-thread inside each seed.

| arm | old mean (sd) | new mean (sd) | new - old +- sigma | z |
|---|---|---|---|---|
| MOVING (free-gas target motion sampled) | 1.36384 (127 pcm) | 1.36307 (117) | -77 +- 61 pcm | -1.26 |
| AT-REST | 1.36165 (154) | 1.36154 (141) | -11 +- 74 | -0.15 |
| AT-REST - MOVING | -219 +- 64 | -153 +- 43 | +66 +- 77 | +0.86 |

|z| <= 2 on all three (held); |z| <= 1 on two of three (held). **Verdict:
consistent with a re-draw**, at about +-60-75 pcm per arm (the tightest of the
three drivers). **Separate pre-existing finding, not caused by #1581:** the
driver's own prediction (-2242 pcm, from gh:#193's table, k_MOVING 1.40546) FAILS
in BOTH builds (OLD -219 +- 71 pcm, NEW -153 +- 65 unpaired; the driver prints
"FAILED as stated" in both outputs), and MOVING k is about 1.364 in both
against 1.40546 in that table. The in-process hook therefore does not
reproduce the whole-material switch's -2242 pcm on either side of the
migration. Not investigated here and not patched.

### `thermal_kernel_keff_worth` (N = 6 replicas, master seeds 20260912-20260917)

`-- 4000 20 80 8` was requested; the run was cut after replica 5 (the
OLD process had finished replicas 0-5, NEW 0-5 plus part of 6) for time, so the
common complete set is N = 6. 600 K graphite medium, four S(a,b) emission
tabulations per replica, multi-thread.

| tabulation | old mean (sd) | new mean (sd) | new - old +- sigma | z |
|---|---|---|---|---|
| 48x16 (superseded) | 1.38445 (199 pcm) | 1.38447 (180) | +2 +- 109 pcm | +0.02 |
| 384x16 (grid only) | 1.38518 (150) | 1.38627 (134) | +109 +- 82 | +1.33 |
| 48x64 (bins only) | 1.38363 (217) | 1.38527 (178) | +163 +- 115 | +1.42 |
| 384x64 (current) | 1.38449 (199) | 1.38534 (144) | +85 +- 100 | +0.85 |

Worths against 48x16, new - old: 384x16 +108 +- 130 (z +0.82), 48x64 +161 +- 128
(+1.26), 384x64 +83 +- 160 (+0.52). |z| <= 2 on all seven (held); |z| <= 1
on four of seven (two of four arms, exactly half, so the prediction's "at
least half" holds, narrowly; three comparisons sit at 1.26-1.42). Three of
four arms and all three worths are positive and the four arms average
+90 pcm, the same new-above-old lean seen in the HTR-10 pebble ensemble; the
fuel-zone and FHR drivers above lean the other way, so it is recorded, not
explained, and no test was made. **Verdict: consistent with a re-draw at |z| <= 2,
resolution about +-100 pcm per arm at N = 6; not a proof of zero.** The
recorded 2026-10-04 worth (+107 +- 207 pcm at 720 000 histories) is
within these statistics in both builds (old +4 +- 110, new +87 +- 116).

### `lct008_pitch_sweep` (PARTIAL, N = 3: not a result)

`--cheap-nuclides` (11 tapes), pitches 1.45 and 1.63576 cm, 3000 x [30 + 100],
seeds 1-3 complete when the run was stopped for time (it was paused for about
25 minutes to let the other drivers finish, then killed). N = 3 is below the
N >= 8 required, so the sd per arm is itself uncertain (OLD 1.45 cm: sd 48 pcm
from three values, against a within-run sigma of about 190). new - old:
1.45 cm +115 +- 151 pcm (z +0.76), 1.63576 cm +75 +- 81 (z +0.94). These are
reported for completeness and **conclude nothing**; the item stays open for
this driver.

### Not done

`fhr_ring_rpt_endf`, `msre_n1_first_criticality`, `icsbep_five_route_keff`,
`lct008_ace_roundtrip` and the nee_soon transport diagnostics were not run:
the three completed drivers plus a partial pitch sweep took the whole time
budget on a shared 4-core box (each process reconstructs 8-36 nuclides, and the
sweeps ran at 3-15 minutes per seed once six processes shared the cores).

### Overall

Across the three completed drivers: 8 arm comparisons and 5 within-build
deltas, every one at |z| <= 2 (largest |z| 1.42, on the 48x64 arm). No difference inconsistent with a pure URR re-draw was found, at resolutions of
about +-60 to +-140 pcm per arm; a physics shift below about 150-300 pcm (2
sigma) would not be seen. The |z| <= 1 half-criterion held on every driver
(thermal-kernel only narrowly).

**Hardware.** Intel Xeon @ 2.10 GHz (cloud container, `lscpu` model name only;
KVM), 4 logical cores, 15 GiB RAM, Linux 6.18, CPU only, shared: OLD and NEW
and up to six processes ran at once. Per-run times (107-307 s for one
fuel-zone case, 225 s for one FHR seed) carry no cross-run meaning and are
not tabulated; whole-process wall time was 4221 s / 4049 s (fuel-zone, old /
new, including the data load).

## Item 2 remainder (gh:#1583, 2026-11-17): `lct008_pitch_sweep` to N >= 8

**Prediction (written before any run of this pass).** Same mechanism and same
criterion as above: OLD = `d1e20f188` (+ the same `OUTRAM_PITCH_SEEDS` patch),
NEW = `2d2a99ab4` (this branch). `--cheap-nuclides` (11 tapes), pitches 1.45 and
1.63576 cm, 3000 x [30 + 100], seeds 1..N with N = 10 planned (>= 8 required),
all seeds in one process per build, OLD and NEW concurrent. The 3 seeds of the
first pass are NOT reused for NEW, because NEW has moved from `7b5dd858d` to
`2d2a99ab4` (changes in `material/thermal.rs`, `source/distribution.rs`,
`physics/keff.rs`); they are re-run, and OLD seeds 1-3 are compared with the
first-pass OLD values as a determinism check (predicted bit-identical). For each
of the two pitch arms, mean(NEW) - mean(OLD) is consistent with zero, |z| <= 2
on both, and |z| <= 1 on at least one. No sign is predicted. From the first
pass the single-seed sigma is about 190-200 pcm, the seed-to-seed sd about
50-190 pcm; at N = 10 sigma(new - old) is expected about 80-110 pcm per arm,
so a shift under about 200 pcm would not be resolved. Any arm beyond 2 sigma is
a finding: reported, not patched. The partial N = 3 result above (+115 +- 151,
+75 +- 81 pcm) is not used.

**Prediction for `msre_n1_first_criticality` (written before any run of it).**
OLD = `d1e20f188` (the driver as it was, per-nuclide `with_thermal_scattering`),
NEW = `2d2a99ab4` (per-material S(a,b)); the driver already has `--seeds`, so no
code change and no patch in either build. Arm: `--mo natural` (the V&V record's
default model), default shield and dimensions, 4000 x [30 + 70], 2 threads per
process, seeds 1..N (N = 8 planned, >= 8 required), all seeds in one process
per build, OLD and NEW concurrent. Sized from the cost of the other drivers, not
from any tally; if seed 1 shows N = 8 does not fit, N is reduced equally in both
and said so. Same mechanism: the migration appends bound C-12 / C-13 / H-1
copies and re-draws the URR stream; the carbon and shield-water physics are
unchanged. mean(NEW) - mean(OLD) consistent with zero, |z| <= 2 (and |z| <= 1
expected). No sign predicted. Expected seed sd about 150-300 pcm at this
history count (the full-core model is large and loosely coupled), so
sigma(new - old) about 100-150 pcm at N = 8; shifts under about 300 pcm are not
resolvable. The recorded V&V k_eff is for a different history count and is not
compared. Beyond 2 sigma is a finding, reported and not patched.

**`msre_n1_first_criticality`: first N = 8 (seeds 1-8) MISSED the gate.**
new - old = +229 +- 91 pcm, z = +2.51 (prediction was |z| <= 2). It stands as
a miss and is not reinterpreted. Before the replication below was run, the plan
was written: a fresh, non-overlapping replication, seeds 9-16, identical
settings and builds, run as a SEPARATE process pair (so the data load is paid
again); the verdict on seeds 1-8 is not changed by it. Prediction for the
replication (same as above): |z| <= 2; a z > +2 again, or a pooled N = 16
|z| > 2, would make a real old/new difference the working hypothesis, to be
chased by reading `assign_material_sab` against the old per-nuclide path (not
by touching anything here).

### Results, item 2 remainder (2026-11-17)

Raw outputs: `ensembles_1583/pitch10_{old_d1e20f188,new_2d2a99ab4}.out`,
`msre_s1-8_*.out`, `msre_s9-16_*.out`, plus `msre_stat.awk`, the arithmetic
used for the MSRE rows (`paste old.k new.k | awk -f msre_stat.awk`, one
`seed k` pair per line, the k values being the `seed N: k = ...` lines of the
outputs; no new Python). The pitch rows were produced by the existing
`analyse.py pitch`. sem = sd / sqrt(N); sigma = sqrt(sem_old^2 + sem_new^2);
z = difference / sigma.

**`lct008_pitch_sweep`** (N = 10, seeds 1-10, 3000 x [30 + 100], 11-nuclide
tier, multi-thread, OLD and NEW concurrent):

| arm | old mean (sd) | new mean (sd) | new - old +- sigma | z |
|---|---|---|---|---|
| 1.45 cm | 1.11715 (106 pcm) | 1.11789 (208) | +75 +- 74 pcm | +1.01 |
| 1.63576 cm (LCT-008) | 1.06221 (170) | 1.06226 (172) | +4 +- 76 | +0.05 |
| delta (1.63576 - 1.45), per seed | -5493 +- 56 | -5564 +- 79 | -71 +- 97 | -0.73 |

|z| <= 2 on both arms and the delta (held); |z| <= 1 on one arm and the delta
(the prediction asked for at least one of the two arms: held). **Verdict:
consistent with a URR re-draw**, resolution about +-75 pcm per arm.
Determinism checks, both as predicted: OLD seeds 1-3 are bit-identical to the
first pass's OLD values, and NEW seeds 1-3 at `2d2a99ab4` are bit-identical to
the first pass's NEW values at `7b5dd858d`, so the intervening source changes
did not move this driver. The two arms average +39 pcm new above old, the same
sign as the earlier lean and well inside the noise.

**`msre_n1_first_criticality`** (`--mo natural`, 52 nuclides, 4000 x
[30 + 70], 2 threads, seeds 1-8, then, as a pre-declared replication, 9-16; the
data load, 374-389 s, was paid once per process):

| seeds | old mean (sd) | new mean (sd) | new - old +- sigma | z |
|---|---|---|---|---|
| 1-8 | 1.01709 (188 pcm) | 1.01938 (176) | **+229 +- 91 pcm** | **+2.51** |
| 9-16 (fresh) | 1.01827 (192) | 1.01832 (228) | +5 +- 105 | +0.05 |
| pooled 1-16 | 1.01768 (193) | 1.01885 (204) | +117 +- 70 | +1.66 |

**Verdict against the prediction: MISSED on seeds 1-8** (z = +2.51 > 2), and
that stays the recorded result of the pre-declared N = 8. The replication did
not repeat it (z = +0.05) and the pooled N = 16 is +1.66. That is consistent
with a 2.5-sigma chance excursion (about 1 in 80 two-sided, among the many
comparisons made across these ensembles) and equally with a real shift of
order +100 pcm that N = 16 cannot resolve. **Not resolved, and not declared a
pure re-draw.** The sign matches the unexplained new-above-old lean of the
HTR-10 pebble and thermal-kernel ensembles; recorded, untested. Nothing was
patched. Reading `assign_material_sab`: the lowering is equivalent by
construction to the old nuclide-level binding for this driver (the same law on
C-12 / C-13 in every material that holds them, H-1 in the shield water), so no
mechanism for a physics difference was found, only the URR re-draw.

A first launch of both builds crashed after about 20 minutes on missing Zr
tapes (the driver reads `../ace/endf/...` from the `reference-data/ace`
submodule, empty in a worktree clone). Both were relaunched with that path
linked to the main checkout's populated submodule; nothing else changed (the
link was removed afterwards).

Per-seed k (old / new), seeds 1-8: `1.01513/1.01756, 1.01757/1.02116,
1.01965/1.01835, 1.01786/1.01905, 1.01394/1.01968, 1.01610/1.02266,
1.01838/1.01893, 1.01813/1.01765`; seeds 9-16: `1.02203/1.01821,
1.01813/1.01861, 1.01641/1.01319, 1.01949/1.02053, 1.01850/1.01984,
1.01865/1.01833, 1.01640/1.01792, 1.01656/1.01993`.

**Not done:** `fhr_ring_rpt_endf` (no seed loop in the 1527-line driver; it
would need a knob, left alone), `icsbep_five_route_keff`,
`lct008_ace_roundtrip`, and the nee_soon transport diagnostics. `msre` was run
ahead of `fhr_ring_rpt_endf` because it already had `--seeds`.

**Hardware.** Intel Xeon @ 2.10 GHz (cloud container, KVM), 4 logical cores,
15.7 GiB RAM, Linux 6.18, CPU only (llvmpipe), shared: OLD and NEW concurrent
plus another agent's compiles (load average up to 7.5). Wall times carry no
cross-run meaning; an msre process was about 390 s load plus about 190-200 s
per seed.

### gh:#1617: where the -2242 pcm came from (read, not re-run)

`examples/fhr_pebble_target_at_rest_ablation.rs` takes its prediction and its
`k_MOVING = 1.40546` from `examples/fhr_ring_rpt_endf.rs` caveat 6, measured on
2026-09-12 with `OUTRAM_RINGRPT_TARGET_AT_REST=1`. That 1.40546 is the
**pre-fix** value: `tests/inelastic_channel_vs_endf.rs`
(`a_threshold_reaction_has_no_cross_section_below_its_threshold`: "before
1.40546 ... +4067 pcm vs OpenMC, after 1.36140 ... -339 pcm") and gh:#193's
closing comment record that the F-19 sub-threshold inelastic clamp defect
(fixed in `45f8d72d`, 2026-09-12) held the whole FHR pebble about +4400 pcm
high, and `fhr_ring_rpt_endf.rs` itself says "Caveats 4-6 were measured BEFORE
the residual was closed". The ablation's MOVING k today (1.363-1.364) matches
the post-fix 1.36140, not 1.40546. So the table the driver's prediction quotes
was taken on a build with the F-19 teleport defect, and the -2242 pcm is not a
like-for-like number for today's model. This is a probable cause established
from those records; it is **not** re-run, since no pre-fix build was
re-measured. ~~The prediction was not retuned and the driver was not edited. A
re-baseline would need a fresh whole-material AT-REST run at today's build.~~
**CORRECTED 2026-11-17 (gh:#1617):** the whole-material switch no longer exists
on today's build (gh:#313); the re-baseline was made by replaying it on
`60f6bb261` (-556 +/- 87 pcm, bit-identical to the per-nuclide hook), and the
driver now gates on that, see the next section.

### gh:#1617: what #193 ablated, and the prediction written before the like-for-like run (2026-11-17)

**What gh:#193 did (read from `75615b678`, 2026-09-12, and `0414bc827`).** The
-2242 pcm row came from `OUTRAM_RINGRPT_TARGET_AT_REST=1` in
`examples/fhr_ring_rpt_endf.rs`, which at that commit set
`KeffSettings::temperature_k = 0.0` for the whole run and changed nothing else:
no nuclide was edited. One seed per side, 4000 x [30 + 80], on a build that still
carried the F-19 sub-threshold inelastic defect (k_MOVING 1.40546).

**That switch no longer exists.** gh:#313 (`0414bc827`, 2026-09-30) made the CSG
kernel stop reading `KeffSettings::temperature_k` (free-gas kT is the nuclide's
data temperature); `tests/temperature_precedence.rs` pins that a changed run
temperature leaves k bit-identical. The env var in `fhr_ring_rpt_endf.rs` was
moved onto `Nuclide::with_target_at_rest` in the same commit. So **on today's
build there is no whole-material switch distinct from the per-nuclide hook**, and
no like-for-like whole-material measurement can be made on it. I will not invent
one. The comparison that CAN be made is a replay of the old switch on the last
kind of build that has it AND the post-F-19 physics: commit `60f6bb261`
(2026-09-17), which already carries the per-nuclide-hook driver. That commit's
message records the hook arm there as -556 +/- 87 pcm (paired, 8 seeds) with
MOVING seed 1 = 1.36662 +/- 0.00210, and says the env-var route was "being
confirmed separately"; no record of that confirmation exists in the tree. This
replay supplies it.

**Prediction (written before any run of the replay).** From the code trace
recorded in `60f6bb261` (zeroing the run temperature moves no pointwise cross
section; `Nuclide::sample_thermal` takes no temperature; `free_gas_kt(temp)` is
the only consumer and `with_target_at_rest` makes that same function return 0.0),
the two routes execute the same arithmetic. Therefore, at `60f6bb261`, 8 seeds:

1. SWITCH arm (`temperature_k = 0.0`, nuclides untouched) is **bit-identical
   seed by seed** to the HOOK arm. Any seed that differs refutes the trace.
2. MOVING seed 1 reproduces 1.36662 +/- 0.00210 (determinism check).
3. AT-REST - MOVING = **-556 +/- 87 pcm** (paired, the 2026-09-17 record) for
   both routes, i.e. within |z| < 2 of that record; -2242 is not reproduced
   (gap about +1700 pcm, more than 15 sigma of the ensemble error).
4. For today's build, the whole-material number would equal today's hook number
   by construction, since the routes coincide. Using only the record, I expect it
   to lie near -556 but not to equal it: `60f6bb261` to today includes #313
   (free-gas kT), URR band / DBRC / equiprobable S(a,b) sampling (`81ec018c3`)
   and per-material S(a,b) (#1581), each of which moves the MOVING arm. I have no
   physics basis to say by how much and will report the drift as measured, not as
   explained.

The stale -2242 / 1.40546 is attributed to F-19, not to the hook: the F-19
teleport put neutrons past the U-238 resonances (p +8.5 % over OpenMC), and a
target-at-rest ablation acts on the thermal/epithermal slowing-down balance that
sits next to that error. This is the same inference the earlier record drew and
is what the replay's MOVING arm and the -2242 gap test.

**Cost probe and plan.** From `ensembles_1583/fhr_new_7b5dd858d.out`: about 300 s
data build, 200-250 s per seed (up to 570 s under load), 4 logical cores shared.
Two processes at most: A = MOVING + HOOK, B = SWITCH; N = 8 seeds each.

#### gh:#1617: results of the replay (2026-11-17)

**Method.** A detached worktree at `60f6bb261`, whose driver already has the
per-nuclide-hook arm. Its only edits (`ensembles_1617/replay_driver_patch.diff`):
an arm `SWITCH` that runs the unmodified nuclides with
`KeffSettings::temperature_k = 0.0` (the gh:#193 switch, exactly as
`75615b678` applied it), an arm selector, and `k` printed to 15 digits. Same case,
4000 x [30 + 80], seeds 1-8, one process per arm group (A = MOVING + HOOK,
B = SWITCH). No retuning: the predictions above were committed (`e046bc796`)
before any of this ran.

| seed | MOVING k | HOOK k = SWITCH k | HOOK vs SWITCH (k, p, eps) |
|---|---|---|---|
| 1 | 1.366620965 | 1.360950879 | bit-identical |
| 2 | 1.365263700 | 1.356959614 | bit-identical |
| 3 | 1.366041799 | 1.365649312 | bit-identical |
| 4 | 1.366565419 | 1.358806863 | bit-identical |
| 5 | 1.366194794 | 1.360870933 | bit-identical |
| 6 | 1.367760389 | 1.362685367 | bit-identical |
| 7 | 1.366477344 | 1.359285890 | bit-identical |
| 8 | 1.365114313 | 1.360320031 | bit-identical |

(Full per-seed values with p and eps in `ensembles_1617/replay_60f6bb261_*.out`;
`ensembles_1617/analyse_1617.py` recomputes everything below.)

| quantity (8 seeds, 60f6bb261) | value |
|---|---|
| k_MOVING | 1.36625 (sd 83 pcm) |
| k_AT-REST, SWITCH arm = HOOK arm | 1.36069 (sd 263 pcm) |
| AT-REST - MOVING, paired | **-556 +/- 87 pcm** (6.4 sigma), paired sd 247 |
| AT-REST - MOVING, unpaired | -556 +/- 97 pcm (5.7 sigma) |
| p (2-group) MOVING / AT-REST | 0.4833 / 0.4841, +0.0008 +/- 0.0004 |
| eps (2-group) MOVING / AT-REST | 1.5063 / 1.5015, -0.0048 +/- 0.0007 |

**Against the predictions.** (1) Bit-identity: held, 8 of 8 seeds, k, p and eps.
(2) MOVING seed 1 = 1.36662 +/- 0.00210: held. (3) -556 +/- 87: held, to the
pcm, because the two routes are one calculation. (4) Today's whole-material
number: not measurable (the switch is gone); the hook's own value on the current
build is -153 +/- 43 pcm paired (8 seeds, `ensembles_1583/fhr_new_7b5dd858d.out`),
so the "near -556, not equal" expectation held in sign and was wrong in size: the
shift is +403 pcm, 4.2 sigma of (87, 43) combined.

**Verdict.**

| comparison | result |
|---|---|
| whole-material switch vs per-nuclide hook, same build `60f6bb261` | identical, 0 sigma, bit for bit; **no hook defect, no disagreement beyond abs(z) = 2** |
| gh:#193 -2242 pcm vs the like-for-like -556 | gap +1686 pcm; explained by F-19 (`45f8d72d`) and the 1-seed error bar, not by the hook |
| hook at `60f6bb261` (-556 +/- 87) vs hook today (-153 +/- 43) | **4.2 sigma, a finding**: the quantity moved with the build |

So gh:#1617's question ("does the hook reproduce the whole-material switch?")
is answered yes, with an exact result and not a statistical one. What remains is
a different finding: the target-motion worth itself changed between 2026-09-17
and today. The unablated arm moved 1.36625 -> 1.36307 (-318 pcm) and the
at-rest arm 1.36069 -> 1.36154 (+85 pcm). I have not bisected it. Candidates by
the commit log between the two builds: gh:#313 (`0414bc827`, free-gas kT is the
nuclide's data temperature), `81ec018c3` (URR band, DBRC, equiprobable S(a,b)
sampling), `a7e650694` (DBRC rejection loop), gh:#1581 (per-material S(a,b)).
~~Which of them, and whether the new value is the better physics, is open.~~
**CORRECTED 2026-11-17 (gh:#1636):** none of the four listed; the cause is the
default-on resonance elastic scattering (`f8dbb4951`, method switched to RVS by
`457d4ca97`), see "gh:#1636" below. The
MOVING arm today (1.36307) also sits nearer OpenMC's 1.36479 +/- 0.00067 than the
replay's 1.36625 does, so the shift is not obviously a regression.

**Not done / limits.** No bisect of the 403 pcm. No whole-material measurement on
today's build, because none exists. The replay is one commit (`60f6bb261`), 8
seeds. Reviewed functions edited: none (`examples/review.md` holds no review
record; the driver is an example).

**Hardware.** Intel Xeon @ 2.10 GHz (cloud container, KVM), 4 logical cores,
15.7 GiB RAM, Linux 6.18, CPU only; shared, load average 8-12 from other agents'
runs during the replay. Per-seed wall times were 600-1250 s and carry no
cross-run meaning; data build about 275 s per process.

## Item 2, last three drivers (gh:#1583, 2026-11-17): predictions

**Written before any ensemble run of this pass.** One cost probe preceded it:
`icsbep_five_route_keff --case hst009 --route endf --seeds 1` on the NEW build
alone (data load about 320 s, 52 s for the seed, k = 1.00128 +/- 0.00143, 2
threads, box loaded by other agents). That seed is kept as seed 1 of the NEW
ensemble; no OLD value had been seen when the predictions below were fixed.

**Design (all three drivers).** OLD = `d1e20f188`, NEW = `501c16264` plus the
`fhr_ring_rpt_endf` seed knob only (committed in this pass; the OLD build
carries the same patch). Same data, settings and seeds in both; all seeds of an
arm in ONE process per build (data load paid once); OLD and NEW run at the same
time, at most two transport processes of mine. sem = sd / sqrt(N);
sigma(new - old) = sqrt(sem_old^2 + sem_new^2); z = difference / sigma. Gate:
|z| <= 2 on every arm; |z| <= 1 also reported. A run cut for time is cut at the
same seed count in both builds, chosen by the clock and never by a result. A
miss is recorded as a miss; if an arm misses, a fresh non-overlapping
replication is planned here before it runs and then pooled, as the MSRE case
was. Nothing is tuned.

**Mechanism under test (same as the earlier passes).** #1581 appends bound
copies of H-1 (and, for MSRE, C-12 / C-13) to the nuclide array;
`transport_csg.rs` advances the URR stream by `nuclides.len()`, so models with
URR-table nuclides draw a different URR realisation. Same physics, different
draws.

**Arms and expected size.**

1. `icsbep_five_route_keff` at the acceptance size 5000 x [40 + 120], route
   `endf`, 2 threads: **hst009** (12 nuclides, URR on 5, S(a,b) H(H2O) on the
   solution and the water reflector), N = 16 seeds 1..16; then **msre** (the
   graphite unit cell; carbon S(a,b) per material), N as the cost allows
   (target 16, at least 8); **lct008-full** (35 nuclides) only if the load
   fits, otherwise reported not done. Prediction: per arm, mean(NEW) -
   mean(OLD) consistent with zero, |z| <= 2, and |z| <= 1 on at least half of
   the arms run. Expected seed sd about 150-250 pcm, so at N = 16 sigma about
   55-90 pcm; a shift under about 150 pcm would not be resolved. No sign
   predicted (earlier ensembles leaned new-above-old by +40 to +120 pcm; the
   lean is checked for, not assumed).
2. `lct008_ace_roundtrip` (11-nuclide lattice, `--purr`, ENDF / ACE / ablated
   arms), reduced size 4000 x [40 + 100] (the default 10000 x [250 + 400] is
   not affordable), N = 8 seeds from `--seed 1`, 3 threads. Prediction: each
   of the three arms consistent between builds at |z| <= 2, and |z| <= 1 on at
   least one of three. The within-build parity (ACE - ENDF) is also predicted
   consistent between builds (computed from the per-seed ENDF and ACE values).
   Expected seed sd 100-200 pcm, sigma(new - old) about 70-100 pcm per arm at
   N = 8; shifts under about 200 pcm invisible. No arm-specific shift is
   predicted.
3. `fhr_ring_rpt_endf`: the driver had no seed loop; a knob
   `OUTRAM_FHR_RING_SEEDS=N` / `OUTRAM_FHR_RING_SEED0=S` was added around its
   `OUTRAM_RINGRPT_ONLY=csg` branch ONLY (the surface-tracked ring-RPT solve,
   4000 x [30 + 80]). The default output is unchanged (knob unset: one solve at
   the default seed, no extra line). The other solves of the driver (explicit
   TRISO, ring-RPT and naive homogenised, cube and sphere, delta-tracked) are
   NOT covered: per-seed repetition of those would put the whole 1500-line
   body inside a seed loop, judged not safe to patch. N = 8 seeds (default
   seed + 0..7). Prediction: mean(NEW) - mean(OLD) consistent with zero,
   |z| <= 2; seed sd about 100-150 pcm (the FHR ablation driver, same pebble,
   same history count, gave 117-154 pcm), sigma about 50-70 pcm. A shift under
   about 150 pcm would not be seen.

### Results, item 2 last three drivers (2026-11-17)

OLD = `d1e20f188`, NEW = `501c16264` + the `fhr_ring_rpt_endf` seed knob.
Raw outputs: `ensembles_1583/hst009_{old_d1e20f188,new_501c16264}.out`,
`icsbep_msre_s1-8_*.out`, `ace_roundtrip_*.out`, `fhr_ring_csg_*.out`. The
arithmetic is `msre_stat.awk` (`paste old.k new.k | awk -f msre_stat.awk`, one
`seed k` pair per line from the `seed N: k = ...` / `SEEDK N k std` lines) and,
for the three-column roundtrip table, the same formulas in a one-off awk
command; no Python. sem = sd / sqrt(N), sigma = sqrt(sem_old^2 + sem_new^2).

**`icsbep_five_route_keff`, route `endf`, 5000 x [40 + 120], 2 threads**
(`--particles 5000 --inactive 40 --active 120`, seeds 1..N):

| case (N) | old mean (sd) | new mean (sd) | new - old +- sigma | z |
|---|---|---|---|---|
| hst009, 12 nuclides, URR on 5 (N = 16) | 1.00082 (156 pcm) | 1.00080 (152) | -2 +- 54 pcm | -0.04 |
| msre unit cell, 15 nuclides, URR on 7 (N = 8) | 1.65542 (157) | 1.65536 (118) | -6 +- 69 | -0.09 |

|z| <= 1 on both. **Verdict: consistent with a URR re-draw**, resolution about
+-55 to +-70 pcm per case. Departures from the plan, all by cost and none by
result: msre was planned for N = 16 and was cut at N = 8 in both builds (about
690-700 s per seed on this box with the other agent's four processes running,
load average about 11.8); the cut was made by the clock on the seed count, with
all eight seeds already finished in both builds. `lct008-full` (35 nuclides)
was not run (not affordable beside msre at these per-seed costs), and godiva and
jemima were not run (they carry no S(a,b), so their nuclide array did not
change). The hst009 seed 1 of the NEW build had been run once before the
predictions as the cost probe (1.00128, the same value as in this ensemble).

**`lct008_ace_roundtrip`**, `--purr --particles 4000 --inactive 40 --active 100
--seeds 8 --seed 1 --threads 2` (2 threads instead of the 3 predicted, to
stay inside the two-process allowance on a loaded box; threads do not change
the estimate):

| arm | old mean (sd) | new mean (sd) | new - old +- sigma | z |
|---|---|---|---|---|
| ENDF route | 1.00235 (128 pcm) | 1.00253 (137) | +18 +- 66 pcm | +0.27 |
| ACE route (with PURR) | 1.00189 (133) | 1.00246 (270) | +57 +- 106 | +0.54 |
| ENDF, DBRC ablated | 1.00159 (122, sem 43) | 1.00143 (171, sem 61) | -16 +- 75 | -0.21 |
| parity ACE - ENDF (per seed pair) | -46 +- 40 pcm | -6 +- 131 | +39 +- 137 | +0.29 |

All four |z| <= 1 (prediction: <= 2 on all, <= 1 on at least one of three
arms: held). **Verdict: consistent with a URR re-draw.** Within each build the
ACE route agrees with the ENDF route (old -46 +- 40 pcm, new -6 +- 131 pcm; the
driver's own verdict line reads "AGREE" for both), which is the point of the
example and is unchanged. The ablated arm prints only its mean and sem in the
driver's summary, so its row is from those two numbers (mean 1.00159 / 1.00143,
sd 122 / 171 pcm), not recomputed per seed. **Not explained, not tested:** the
NEW ACE arm's seed-to-seed sd is 270 pcm against 133 pcm in OLD (variance ratio
about 4.1 on 7 and 7 degrees of freedom, about 0.04 one-sided); seeds 4 and 5
of NEW swing the parity by +397 and -408 pcm. The ENDF arms and the ablated arm
do not show it, and a ratio like this among the many comparisons made across
these ensembles is not strong evidence, but it is on the record. Absolute k
(1.0024 at 4000 x 100) belongs to the 11-nuclide reduced-size model and is not
comparable with the 10000 x [250 + 400] record in the file's doc comment.

**`fhr_ring_rpt_endf`, surface-tracked CSG solve only**
(`OUTRAM_RINGRPT_ONLY=csg OUTRAM_FHR_RING_SEEDS=8`, seeds 1..8, 4000 x
[30 + 80], 600 K, default thread pool, one process per build):

| arm | old mean (sd) | new mean (sd) | new - old +- sigma | z |
|---|---|---|---|---|
| ring-RPT CSG pebble | 1.36384 (127 pcm) | 1.36307 (117) | -77 +- 61 pcm | -1.26 |

|z| <= 2 (prediction: held); |z| <= 1 not met on this arm (the prediction asked
for it only for arms of the other two drivers). **Read this honestly: this is
NOT independent of the earlier FHR ablation.** This solve is the same model,
settings and seeds as the MOVING arm of `fhr_pebble_target_at_rest_ablation`,
and the 8 per-seed k in each build are identical to the printed digits to that
driver's MOVING lines (OLD 1.36463, 1.36491, 1.36322, ...; NEW 1.36355,
1.36228, ...), so the number above is the same -77 +- 61 pcm (z -1.26) already
recorded for MOVING. What this run adds is that the `fhr_ring_rpt_endf` entry
point and its new seed knob reproduce that arm bit for bit in both builds
(a determinism and harness check), not a second sample. **Not covered:** the
driver's other six solves (explicit-TRISO, ring-RPT and naive-homogenised, cube
and sphere, delta tracking) were not re-measured; putting them in a seed loop
means wrapping most of a 1500-line `run()` and was not attempted. The record
for those remains the pre-migration values. The knob leaves the default output
unchanged (knob unset: one solve at the default seed, no extra line).

**Overall.** Across 6 arm comparisons and 1 within-build parity in the three
drivers (hst009, msre, three roundtrip arms, FHR CSG; the FHR row repeats an
earlier record), every |z| <= 1.26 and none exceeded 2: **no arm missed the
gate, so no replication was planned or run.** The signs are mixed (-2, -6, +18,
+57, -16, -77 pcm), so this pass does not extend the "new above old" lean of the
earlier thermal-kernel / HTR-10 / MSRE ensembles. Nulls of about +-55 to +-110
pcm per arm, not proofs of zero: a physics shift under about 110-220 pcm (2
sigma) would not be seen.

**Reproduction notes.** Both binaries were built with `-j 2` and copied under
unique names. The OLD worktree was deleted after its build, but the OLD
binaries embed its path at compile time (`CARGO_MANIFEST_DIR`), so OLD runs set
`OUTRAM_PARK_ENDF_DIR` to this checkout's `reference-data/endf` (the same
tapes) and the OLD `fhr_ring_rpt_endf` got a scratch directory shaped like the
old tree whose `reference-data/endf` is a link to the same tapes. The msre case
reads Zr tapes from `reference-data/ace/endf`, so `reference-data/ace` was
linked to the main checkout's populated submodule for the runs and the link was
removed afterwards. Hardware: Intel Xeon @ 2.10 GHz (cloud container, KVM), 4
logical cores, about 15 GiB RAM, Linux 6.18, CPU only, shared with another
agent's two long processes (load average 8-12 throughout); per-seed wall times
(52-70 s hst009 at 2 threads, about 690-705 s msre, about 5 min FHR CSG) carry
no cross-run meaning.

## gh:#1636: which change moved the FHR target-at-rest worth from -556 to -153 pcm (2026-11-17)

**Predictions written before any run of this pass.** Question: between
`60f6bb261` (2026-09-17) and the session build, the paired AT-REST - MOVING
worth moved from -556 +/- 87 pcm to -153 +/- 43 pcm. Which change did it?

**What the two ensembles already say (read, not re-run).** MOVING k went
1.36625 (sem 29 pcm, sd 83) -> 1.36307 (sem 41, sd 117): **-318 +/- 50 pcm
(6.4 sigma)**. AT-REST went 1.36069 (sem 93) -> 1.36154 (sem 50): **+85 +/- 105
pcm (0.8 sigma, not resolved)**. So the worth shift is carried by the MOVING
arm; the AT-REST arm has not been shown to move. A cause that acts on both arms
equally (cross-section details, S(a,b) data) would have moved AT-REST by about
the same -318 pcm; it did not (difference of the two shifts: 403 +/- 116 pcm).
The cause is therefore predicted to be something that acts only when the target
kT > 0, i.e. the free-gas / resonance-scattering collision path.

**Design.** Runtime legacy switches on the current build (no rebuild of old
commits), `OUTRAM_FHR_ARMS=moving` (the driver's new MOVING-only mode), seeds
1-8, 4000 x [30 + 80], one process per switch, at most two transport processes.
sem = sd/sqrt(8); sigma(a - b) = sqrt(sem_a^2 + sem_b^2); gate |z| <= 2. A
switch's worth is the MOVING k with the switch minus the MOVING k without it.
Nothing is tuned; the -556 reference is not moved.

| step | switch (on the session build) | predicted MOVING k change (legacy minus default) | basis |
|---|---|---|---|
| 0 | none: determinism check vs `ensembles_1583/fhr_new_7b5dd858d.out` | 0, bit-identical on 8 of 8 seeds | transport code unchanged since; later commits are depletion/k-search |
| 1 | `OUTRAM_MC_LEGACY_SCATTER_KERNELS=1` (pre-2026-11-11 free-gas, CM-to-lab and ladder kernels; gh:#1386) | **+300 pcm, range +100 to +450** (up: restores the old 1.366) | the only listed change that acts on kT > 0 only; carries the shift if the hypothesis holds |
| 2 | `OUTRAM_MC_LEGACY_REACTION_ORDER=1` (master: reaction order, fission-neutron order, per-collision banking; gh:#1444) | 0 +/- 60 pcm (abs(z) < 2 of zero) | draw-order and unbiased banking changes of the same physics; no kT dependence |
| 3 | `OUTRAM_MC_LEGACY_XS_DETAILS=1` (nu_fission, extrapolation, S(a,b) cutoff and total; gh:#463) | 0 +/- 60 pcm | acts on both arms, so cannot explain a MOVING-only shift |

A step-1 result inside the range would make `ScatterKernels::OpenMc` (default
physics, maintainer decision 2026-11-11) the cause; the consequence is a
documented re-baseline, never a revert. If steps 1-3 together leave more than 2
sigma of the -318 pcm unexplained, the remaining candidates (gh:#313 free-gas
kT, `81ec018c3` URR/DBRC/equiprobable S(a,b), `457d4ca97` RVS default,
gh:#1581 per-material S(a,b)) are tested by building those commits and their
parents, planned here before they run. Hardware: shared 4-core Intel Xeon
2.10 GHz, 15.7 GiB, CPU only, another agent running.

#### gh:#1636: step 4 prediction, written after steps 0-1 and before step 4 ran

Steps 0 and 1 were running when this was written (seeds 1-7 of each were in:
step 0 reproduces `ensembles_1583` seed for seed; step 1, the legacy scatter
kernels, sits at about 1.3626 against 1.3630, i.e. no shift). **The step-1
prediction (+300 pcm) is refuted**: the kernel rewrite is not the cause. Steps 2
and 3 are not run unless step 4 fails, because their predictions were null and
step 1 has already moved the suspicion elsewhere.

What I missed when predicting: at `60f6bb261`, `Nuclide` carried **no** resonance
elastic scattering table (`git grep -i dbrc 60f6bb261 -- crates/outram-mc-libs/src`
hits only `physics/scatter.rs`; `material/nuclide.rs` has none), and the FHR
driver never attached one. On the session build every nuclide with 0 K elastic
data carries one by default (RVS, upstream's method, gh:#1311; table from
`81ec018c3` / `a7e650694`), acting between 0.01 eV and 1 keV on U-238. That
acts only when the target kT > 0, so it is **in the MOVING arm and absent from
the AT-REST arm**, which is exactly the signature found above (MOVING -318 pcm,
AT-REST unresolved). Resonance scattering broadens U-238's effective resonance
capture and is known to lower k in low-enriched thermal systems.

**Step 4: price it.** New driver knob `OUTRAM_FHR_NO_RESONANCE_SCATTERING=1`
removes the table (`Nuclide::without_dbrc`) from both arms before the at-rest
hook; everything else is the session build. Seeds 1-8, 4000 x [30 + 80], both
arms, paired. Predictions, fixed now:

1. MOVING k with the table removed returns to the `60f6bb261` value:
   **1.3663 (range 1.3650-1.3680)**, i.e. +320 pcm (range +150 to +480) over
   the default MOVING arm 1.36307.
2. AT-REST with the table removed is **bit-identical per seed** to the default
   AT-REST arm (kT = 0 means the table is never consulted). A seed that differs
   refutes the mechanism claim, or shows the table alters RNG draws at kT = 0.
3. The paired worth with the table removed is within |z| <= 2 of the -556 +/- 87
   record: inside (-750, -362) pcm, centre about -480 (AT-REST 1.3615 minus
   MOVING 1.3663).

If 1 holds and 3 holds, the cause is the default-on resonance scattering
(correct physics, maintainer decision gh:#1311), and the fix is a re-baseline
of the driver's reference to the session-build value with the old one struck
through. If 1 holds only in part, the residual is attributed to the remaining
candidates (gh:#313, per-material S(a,b)) by building those commits, planned
here before they run.

#### gh:#1636: results (2026-11-17)

Raw outputs: `ensembles_1636/step{0_default,1_scatter,4_moving,4_atrest}.out`;
`ensembles_1636/analyse_1636.sh` recomputes everything (awk), its output is
`ensembles_1636/analysis_1636.txt`. Binary: the session build at `26eab0e5e`'s
parent tree plus the driver knobs (the example only), seeds 1-8, 4000 x [30 + 80].

| arm (8 seeds) | k_MOVING mean, sd, sem | change vs default MOVING | z |
|---|---|---|---|
| step 0, default | 1.36307, 117, 41 pcm | 0 (reproduces `ensembles_1583` seed for seed, to the 5 printed digits on 8 of 8) | - |
| step 1, `OUTRAM_MC_LEGACY_SCATTER_KERNELS=1` | 1.36275, 62, 22 pcm | **-32 +/- 47 pcm** unpaired (paired -32 +/- 33) | 0.7 |
| step 4, resonance scattering removed | 1.36570, 173, 61 pcm | **+263 +/- 74 pcm** unpaired (paired +263 +/- 84) | 3.6 |
| AT-REST, default = AT-REST, resonance scattering removed | 1.36154, 141, 50 pcm | **bit-identical on 8 of 8 seeds** (5 printed digits against `ensembles_1583`; full precision in `step4_atrest.out`) | - |

| worth (AT-REST - MOVING), 8 seeds | value | vs the -556 +/- 87 record (60f6bb261) |
|---|---|---|
| default build (resonance scattering on) | -153 +/- 43 pcm paired (sd 121); unpaired -153 +/- 65 | gap +403, 4.2 sigma: FAILS |
| resonance scattering removed | **-416 +/- 79 pcm** unpaired (paired -416 +/- 80, sd 225; paired sd exceeds the arms', so quote unpaired) | gap +140, 1.2 sigma of (79, 87): **HOLDS, abs(z) <= 2** |

**Against the predictions.**

- Step 1 (+300 pcm, range +100 to +450): **refuted**, measured -32 +/- 47. The
  scatter-kernel rewrite (gh:#1386) does not move this case; I had picked it as the
  only listed change that acts at kT > 0 only, and that was the wrong candidate.
- Steps 2 and 3 (null predictions): **not run**, step 4 explained the shift.
- Step 4, three predictions written after steps 0-1 and before step 4:
  (1) MOVING returns to 1.3663 (range 1.3650-1.3680): measured **1.36570**, inside
  the range (the `60f6bb261` value 1.36625 is -55 +/- 68 pcm away, 0.8 sigma);
  (2) AT-REST bit-identical per seed: **held, 8 of 8**; (3) worth inside
  (-750, -362) pcm: **-416**, inside (centre predicted -480, within 0.8 sigma).
  Accounted for: of the MOVING shift of -318 +/- 50 pcm, +263 +/- 74 is restored by
  removing the table (83 %); the remaining -55 pcm is 0.8 sigma, not resolved.

**Verdict.** The worth moved from -556 to -153 pcm because the session build
**applies resonance elastic scattering by default** and `60f6bb261` did not. That
is correct physics (CLAUDE.md "correct physics is the DEFAULT"; maintainer
decision gh:#1311 on the RVS method), a default-on change, so the fix is a
documented re-baseline, not a revert. It acts only at target kT > 0, which is why
the MOVING arm carries the shift and the AT-REST arm is untouched. The old -556
is the worth of a model without the effect (reproduced to 1.2 sigma when the
effect is removed). The driver's reference was **not moved to fit**: the default
build is judged against its own measured -153 +/- 43 (`SESSION_PCM`), and the
ablated configuration against the unchanged -556 +/- 87. The effect itself is also
the physically sensible sign: resonance up-scatter off U-238 widens its resonances
and lowers k.

**Not done / limits.** (a) The ablation removes DBRC-on and the RVS method
together; building `f8dbb4951` and its parent, and `457d4ca97` and its parent, to
split them was not done (about 5 min build plus 35 min per arm each; the
mechanism is already resolved to within 1 sigma, so I stopped). (b) No
commit-by-commit bisect of the remaining -55 pcm; it is 0.8 sigma. (c) Steps 2 and
3 not run. (d) No new physics V&V against OpenMC: whether -153 is the better
physics rests on the maintainer's RVS decision and
`tests/dbrc_single_scatter_vs_openmc.rs`, not on this study. (e) Eight seeds per
arm; AT-REST sem 50 pcm. Reviewed functions edited: none (an example only; no
`src/` change). Hardware: Intel Xeon 2.10 GHz (KVM), 4 logical cores shared by two
of my transport processes plus another agent's builds, 15.7 GiB, Linux 6.18, CPU
only; per-seed wall times 390-440 s carry no cross-run meaning.

## ACE-arm variance replication (gh:#1635, 2026-11-17): prediction

**Written before any run of this pass.** The question: is the NEW build's ACE
arm of `lct008_ace_roundtrip` (seed sd 270 pcm, seeds 1-8) really noisier than
OLD (133 pcm, F about 4.1, one-sided p about 0.04), or was that one of many
comparisons made across these ensembles?

**Design.** Fresh, non-overlapping seeds 9-16 for both builds, exactly the
settings of the seeds 1-8 run: 11 nuclides, `--purr --particles 4000
--inactive 40 --active 100 --seeds 8 --seed 9 --threads 2`, one process per
build, both builds at the same time (two transport processes). OLD =
`d1e20f188`; NEW = this branch (its `src/` and
`examples/lct008_ace_roundtrip.rs` are identical to the `501c16264` used for
seeds 1-8; checked with `git diff --stat`). Reported: per-build sd on seeds
9-16 and pooled 1-16, F = var(NEW)/var(OLD) for the ACE arm (and for the ENDF
arm and the parity as controls), and paired new - old with z (sem = sd/sqrt(N),
sigma = sqrt(sem_old^2 + sem_new^2)).

**Mechanism.** None expected. #1581 changes which URR realisation is drawn
(stream advanced by `nuclides.len()`), not the variance of the estimator; the
ACE route reads back the same tapes. A genuine variance excess in the ACE arm
only would need a mechanism specific to the ACE read-back plus the bound
S(a,b) copies, and none is known.

**Prediction.** Chance. On seeds 9-16 the NEW/OLD ACE variance ratio is near 1
(point estimate 0.5-2; F inside 0.2-4.99, the central 95 % band of F(7,7),
with probability about 0.95 under chance). Pooled 1-16 ratio about 1.5-2.5
(the seeds 1-8 excess diluted, not removed), pooled F(15,15) one-sided p above
0.05. Paired new - old ACE |z| <= 2 on 9-16. If the F on 9-16 exceeds 4.99
(one-sided p < 0.05 again, independent of the first) the "chance" reading is
rejected and a mechanism hunt is warranted. Nothing is changed in the code by
this item and nothing is tuned.

### Results, ACE-arm variance replication (gh:#1635, 2026-11-17)

Raw outputs: `ensembles_1635/ace_roundtrip_{old_d1e20f188,new_ed32c976a}_seeds9-16.out`;
seeds 1-8 are `ensembles_1583/ace_roundtrip_*.out`; arithmetic is
`ensembles_1635/stats.sh` (awk, per-seed k parsed from the `seed N: ENDF ... ACE ...`
lines; the seeds 1-8 rows reproduce the table above to the digit, which checks
the parser). Settings as predicted (`--purr --particles 4000 --inactive 40
--active 100 --seeds 8 --seed 9 --threads 2`, 11 nuclides, both builds at once,
box load average about 11-12). F = var(NEW)/var(OLD); sigma(new - old) =
sqrt(sem_old^2 + sem_new^2). k in pcm-of-k below shown as k.

| arm | seeds | old mean (sd, pcm) | new mean (sd) | F | new - old +- sigma | z |
|---|---|---|---|---|---|---|
| ACE | 1-8 | 1.00189 (133) | 1.00246 (270) | 4.10 | +57 +- 106 | +0.54 |
| ACE | **9-16** | 1.00168 (**280**) | 1.00207 (**173**) | **0.38** | +39 +- 116 | +0.33 |
| ACE | pooled 1-16 | 1.00179 (212) | 1.00227 (220) | **1.08** | +48 +- 76 | +0.63 |
| ENDF (control) | 1-8 | 1.00235 (128) | 1.00253 (137) | 1.14 | +18 +- 66 | +0.27 |
| ENDF | 9-16 | 1.00276 (200) | 1.00173 (201) | 1.00 | -103 +- 100 | -1.03 |
| ENDF | pooled 1-16 | 1.00256 (164) | 1.00213 (171) | 1.09 | -43 +- 59 | -0.72 |
| parity ACE - ENDF (pcm) | 1-8 | -46 (114) | -6 (372) | 10.6 | +39 +- 137 | +0.29 |
| parity | 9-16 | -108 (385) | +34 (311) | 0.65 | +142 +- 175 | +0.81 |
| parity | pooled 1-16 | -77 (276) | +14 (332) | 1.44 | +91 +- 108 | +0.84 |

**Verdict: the seeds 1-8 variance excess was chance.** On the fresh seeds the
ratio reversed: OLD's ACE arm has sd 280 pcm and NEW's 173 (F = 0.38, i.e.
OLD/NEW = 2.6, itself about p = 0.1 one-sided), so OLD shows the same large
spread NEW showed on 1-8. Pooled over 16 seeds the two builds have the same ACE
sd (212 vs 220 pcm, F = 1.08). Nothing in this ensemble supports a build-
specific variance difference, and the seed-to-seed sd of this ACE arm is simply
large and variable at 8 seeds (133-280 pcm across the four 8-seed samples).
The paired ACE shift is +48 +- 76 pcm (z +0.63), consistent with zero. The
within-build parity ACE - ENDF is -77 +- 69 pcm (OLD) and +14 +- 83 pcm (NEW)
over 16 seeds (sem = sd/4), both within about 1.1 sigma of zero; the driver's
own verdict line reads AGREE on seeds 9-16 for both builds.

Prediction check: ACE F on 9-16 inside 0.2-4.99: **held** (0.38); paired ACE
|z| <= 2 on 9-16: **held** (0.33); pooled F(15,15) p above 0.05: **held**
(F = 1.08, p about 0.4). **Missed:** the predicted pooled ratio "about 1.5-2.5"
came out 1.08, because the OLD build supplied the large spread rather than
only diluting NEW's; that is a closer-to-1 result than predicted, not a
contradiction of the chance reading.

**A new anomaly, reported and NOT explained: the ENDF-with-DBRC-ablated arm,
seeds 9-16.** It was a secondary arm in the plan (no prediction was written for
it beyond the earlier "consistent" record) and its per-seed values are not
printed, so this row is from the driver's own summary lines (mean +- sem over
8 seeds):

| arm | seeds | old | new | new - old +- sigma | z |
|---|---|---|---|---|---|
| ENDF, DBRC ablated | 1-8 | 1.00159 (sem 43) | 1.00143 (sem 61) | -16 +- 75 | -0.21 |
| ENDF, DBRC ablated | 9-16 | 1.00441 (sem 46) | 1.00179 (sem 46) | **-262 +- 65** | **-4.0** |

Within OLD alone the ablated mean moved from 1.00159 (seeds 1-8) to 1.00441
(seeds 9-16), +282 pcm, about 4.3 sigma between two 8-seed means (sd 122 and 130
pcm), while the ENDF and ACE arms of the same OLD process moved by +41 and -21
pcm. This is a 4-sigma result among a dozen or so comparisons in these
ensembles, so chance is not excluded but is not a comfortable reading. With
only summary means it cannot be tested here (no per-seed ablated values; a
re-run with per-seed output of that arm, or a third block of seeds 17-24, would
be the next step). Do not quote the ablated-arm difference as a physics shift
until that is done: the two builds agreed on seeds 1-8, and the shift sits in
the OLD build's second block, not in a NEW-only change. Filed as a follow-up in
the hand-off, not tuned or explained.

### gh:#1635: two majorant tests migrated off `with_thermal_scattering` (2026-11-17)

`tests/majorant_bounds_endf.rs` and `tests/majorant_temperature_cover.rs` now
load every nuclide free and associate each LEAPR law per material through
`assign_material_sab` (the #1581 API); the `#![allow(deprecated)]` markers are
removed. Methodology: the test binaries were built before (`ed32c976a`, deprecated
path) and after (this change) and run with identical settings, `--test-threads=1`.

- **`majorant_bounds_endf`: output identical before and after**, line for line
  apart from wall time (3 passed; 1127 s before, 1110 s after). Raw:
  `ensembles_1635/majorant_bounds_endf_{before,after}.out`.
- ~~**`majorant_temperature_cover`: Not re-checked yet** at this commit; runs in
  progress.~~ **CORRECTED 2026-11-17:** both runs finished, **5 passed in each;
  every printed majorant number identical before and after** (diffed with the
  build and fit wall times masked; 6680 s before, 6702 s after). Limits: the two
  `u238_*` tests printed `SKIP: reference ENDF tapes not present` in BOTH runs
  (the binaries were run from a scratch directory where the tape lookup failed),
  so this compares the three graphite/UO2 tests only; the U-238 cases hold no
  S(a,b) law and are untouched by the migration. The head of each output file
  (the first test and the `[MIX] DEFAULT` line of the second) was overwritten by
  an accidental relaunch at 15:18 and is not in the saved files; before it was
  lost it read `table worst 0.7729 (1.386e-5 eV, 1500 K)` in the before run.
  Raw: `ensembles_1635/majorant_temperature_cover_{before,after}.out`.

## gh:#1635 item 1: `htr10_graphite_thermal_scattering_pebble_bed` (2026-11-17): prediction

**Written before any run of the migrated test.** The test file built one array
`[U234, U235, U238, C0]` through `nuclides(thermal)`, attaching the graphite law
to `C0` (slot 3) with `with_thermal_scattering`; material 1 (graphite matrix)
is the only reference to slot 3. The migration loads all four slots free and
lowers the law per material with `assign_material_sab` (the
`majorant_temperature_cover` pattern), which appends a bound copy of `C0` at
slot 4 and repoints material 1 at it. The free-gas arm is unchanged (4 free
nuclides, `materials()` as before).

Predictions, each falsifiable by the before/after outputs:

1. **Free-gas k: bit-identical** (same array, same materials, same seed).
2. **Bound k: bit-identical OR a seed-level re-draw.** The array grows 4 -> 5,
   and `keff_delta` advances its URR stream by `nuclides.len()` per energy change
   (`future_seed(nuclides.len(), urr_seed)`, `src/pebble_beds/keff_delta.rs:1237`).
   If no nuclide in this model calls the URR draw (`from_core` windowed-multipole
   nuclides; `needs_urr_draw`), the stream is never consumed and the k is bit-identical.
   I do not know which holds, so both are admissible; if the bound k moves I will
   state that as a stream re-draw only after showing it is statistically
   consistent in a seed ensemble (|z| <= 2), not assume it.
3. **Nuclide-level checks** (`graphite_thermal_law_reaches_the_transport_kernel`,
   `majorant_still_bounds_total_xs_with_bound_graphite`, the shape test): the
   bound copy is a clone of the same `C0` with the same law, and the majorant is
   rebuilt on the same material set, so every printed number is **identical**
   (the unreferenced free `C0` at slot 3 is in no material, so `macro_xs_total`
   and the majorant are unchanged).
4. **Any pinned value that moves: STOP and report; no re-pin.**

### Results, gh:#1635 item 1 (2026-11-17)

`tests/htr10_graphite_thermal_scattering_pebble_bed.rs` is migrated: the new
`model(thermal)` helper loads `[U234, U235, U238, C0]` free and, for the bound
arm, lowers the graphite law with `assign_material_sab`
(`MaterialSab::for_materials_containing(&mats, 3, 0, &[])`); `matrix_nuclide`
reads the matrix's nuclide through the material, so the nuclide-level checks
also run on the bound copy that `assign_material_sab` makes (its one sanctioned
binding site). No deprecated call is left in the file and the
`#![allow(deprecated)]` marker is gone. Methodology: the test binary was built
before (`e5b0e9e3b`, deprecated path) and after (this change); both were run
`--test-threads=1 --nocapture`, once for the four live tests and once with
`--ignored` for `diagnose_thermal_spectrum_reach`. Hardware: Intel Xeon
@ 2.10 GHz KVM container, 4 logical cores, 15.7 GiB, Linux 6.18, CPU only,
shared; the test runs are single-threaded.

| quantity | before | after |
|---|---|---|
| free-gas k (300 histories, 8 + 15 cycles) | 1.93405 +/- 0.00852 | 1.93405 +/- 0.00852 |
| bound k | 1.94577 +/- 0.00635 | 1.94577 +/- 0.00635 |
| `<E'/E>` at 0.0253 eV, free gas / bound | 1.13863 +/- 0.00123 / 1.03807 +/- 0.00065 | identical |
| worst Sigma_t/Sigma_maj (200000 points) | 0.909093 at 4.850698 eV, material 0 | identical |
| diagnostic: histories reaching cutoff, mean scatters, percentiles | 17923 / 20000, 77.7, p01 3.9750 ... p90 6.3901 eV | identical |

**All four live tests and the diagnostic print identical output before and
after** (diff with the wall times masked; 4 passed / 1 ignored in both; 97.4 s
before, 96.0 s after). **Prediction check:** (1) free-gas k bit-identical: held.
(2) bound k bit-identical or a re-draw: it is **bit-identical**, so the array
growth 4 -> 5 did not move anything here (the URR stream is not consumed by this
`from_core` model; I did not separately trace that, the identical k is the
evidence). No seed ensemble was needed. (3) nuclide-level numbers identical:
held. (4) no pinned value moved, nothing re-pinned. Limit: one seed and one
packing, as the test always was; this shows the migration changes nothing in
these runs, not that the physics is validated. Raw:
`ensembles_1635/htr10_pebble_bed_{before,after,diag_before,diag_after}.out`.

## gh:#1635 item 2: nee_soon HTR-10 diagnostics, transport part (2026-11-17): prediction

**Which of the five migrated diagnostics (`00d2cc781`) transport.** Read from the
sources: `htr10_material_check`, `htr10_mat_xs` and `htr10_majorant_diagnosis`
only evaluate macroscopic cross sections or a majorant (no neutron histories);
`htr10_mgxs_genfoam` is BLOCKED by its own header (panics at assembly, gh:#475)
and cannot run. The only one with transport is
**`htr10_deterministic_vs_mc`** (explicit-TRISO full-core `run_keff_csg_hybrid`,
two passes; the k comes from pass 1). It is the one measured here.

**Knobs added to the example (defaults unchanged, applied identically to the
OLD checkout `d1e20f188`):** `OUTRAM_HTR10_SEED` (the `KeffSettings::seed`, default
20260919) and `OUTRAM_HTR10_K_ONLY` (stop after pass 1; scoring draws no random
number, so pass 1's k is the full run's k).

**Prediction, written before any run.** OLD attaches the graphite law to three
carbon slots with `with_thermal_scattering` (C12 slot 4, C13 graphite slot, ...,
lines 192/200/214) and the SiC laws likewise; NEW loads every slot free and
`assign_htr10_sab` appends bound copies after them, so the nuclide array
**lengthens**. Every nuclide here comes from ENDF with RECONR+BROADR+PURR, so URR
probability tables are live and the URR stream advances by `nuclides.len()` per
energy change: the stream **re-draws**, bit-identity is NOT expected. Materials
reference the same cross sections otherwise (each slot's role audited safe,
`00d2cc781`), so the expected shift in k is **zero within statistics**. Gate:
paired by seed, N seeds, `new - old` within |z| <= 2 of its standard error; a
bigger shift is reported, not tuned. Pass criterion for a seed: none (no
pinned value; the example prints k and compares to a deterministic solver).
If the histories needed for a usable per-seed sigma exceed the time budget, the
run is reduced (histories, rings, layers via the example's existing knobs) and
the reduction stated; k of a reduced run is a statistics instrument for the
old-vs-new comparison, not a benchmark number.

### Results, gh:#1635 item 2 (2026-11-17): NOT MEASURABLE, the example panics at OLD and NEW alike

Of the five nee_soon diagnostics migrated in `00d2cc781`, only
`htr10_deterministic_vs_mc` transports (see the prediction above). It was built
at OLD (`d1e20f188`, throwaway worktree) and NEW (this branch) and run with its
documented defaults plus `OUTRAM_HTR10_HISTORIES=100` (to shorten, had it run),
from `crates/nee_soon`, tapes present (`reference-data/endf`, 90 files, no
SKIP). **Both builds panic identically, about four minutes in, at the start of
Monte Carlo pass 1**, after the geometry is built (5746 tiles, 19410 cells, 767
universes):

```
panicked at crates/outram-mc-libs/src/material/cell_temperature.rs:178:68:
cell 2 references material 11, out of range
  ... with_cell_temperatures_and_majorants <- run_keff_csg_seq
  <- run_keff_csg_inner <- run_keff_csg_hybrid <- htr10_deterministic_vs_mc::main
```

Raw: `ensembles_1635/nee_soon_dvm_{old,new}_panic.out` (identical apart from clock
times and thread ids). So the panic is **not caused by the #1581 migration**
(OLD has it too) and **no old-vs-new k could be measured**; the prediction
(zero shift within |z| <= 2 after a URR-stream re-draw) is **untested**, neither
supported nor refuted. Diagnosis I can state: the example's `materials()` builds
exactly `mat::HOMOG_DUMMY + 1 = 11` materials (it asserts this), while the
assembled core (`assemble_explicit_triso`) has a cell filled with material
index 11, `mat::ZONE_TABLE_FIRST`, so `with_cell_temperatures` (the cell
temperature default, gh:#313, `651dcd747`, 2026-11-14) rejects it. I did not
check whether the panic predates #313 (older code may have skipped the lookup)
or which cell 2 is; I did not fix the example, because that is a rewrite of its
material list against the current core, outside this item. The other three
non-transport diagnostics (`htr10_material_check`, `htr10_mat_xs`,
`htr10_majorant_diagnosis`) evaluate cross sections or a majorant only and were
not run; `htr10_mgxs_genfoam` is blocked by its own header (gh:#475). The
`OUTRAM_HTR10_SEED` / `OUTRAM_HTR10_K_ONLY` knobs mentioned in the prediction
were reverted (never exercised past the panic); the prediction text above is
kept as written. The nee_soon `data.rs` / `data_jobs.rs` loader (full-core
HTR-10 runs, marked not migrated) was not measured either. Hardware: Intel Xeon
@ 2.10 GHz KVM container, 4 logical cores, 15.7 GiB, CPU only, two processes
concurrent.

## gh:#1635 item 2, second attempt (2026-11-17): the example fixed, prediction

**The earlier "NOT MEASURABLE" record above is superseded; its diagnosis was
right and is now acted on.** ~~`htr10_deterministic_vs_mc` ... could not be
measured~~ **CORRECTED 2026-11-17:** `htr10_deterministic_vs_mc` built
`mat::HOMOG_DUMMY + 1 = 11` materials by hand while `assemble_explicit_triso`
references up to `mat::COUNT` (11 + 24 Table 4-3 zones + ROD_B4C/STEEL/IRON).
Fix, example only: the nuclide plan is `Htr10NuclideLayout::plan` (the loader
`htr10_rmc_keff` uses) with every slot loaded FREE, the materials are
`htr10_material_set(layout, Htr10MaterialConfig::benchmark_default(293.6))` (the
one builder every other `htr10_rmc` consumer calls), and the graphite /
C-in-SiC / Si-in-SiC laws are still attached per material by
`assign_htr10_sab(.., &layout.pebble, ..)`. The UO2 and iron laws the plan names
for some slots are NOT bound (the example never bound them). The helium slot now
carries real He-3/He-4 (it was an empty material before): a change of the
example's model that applies to OLD and NEW alike. Test:
`tests::materials_cover_every_core_slot` in the example
(`materials().len() == mat::COUNT`, every component index inside the plan).

**When it broke (git, not run).** `mat::ZONE_TABLE_FIRST` and the 24 zone slots
arrived in `55e46d6f4` (2026-09-25, "HTR-10 reflector as explicit 3-D
geometry"), the rod slots (`ROD_B4C`...) later in the same series; the example's
material list and its `HOMOG_DUMMY + 1` assert were never updated (its only
later commits are the `mathf` route `0cfb3c036` and the #1581 migration
`00d2cc781`). So the example has been inconsistent with the core since
2026-09-25. Why it panics with `cell 2 references material 11` is #313
(`651dcd747`, 2026-11-14), which checks every cell's material at run start;
before that the same out-of-range index would have been hit at the first
collision in such a cell (inference from the code, NOT run on an old checkout).

**Prediction, written before any run of the fixed example.** OLD (`d1e20f188`,
same example fix, laws bound on the nuclide slots with
`with_thermal_scattering`) vs NEW (this branch, laws per material): the
per-material association lengthens the nuclide array (bound copies appended),
the URR stream advances by the array length, so histories re-draw and no
bit-identity is expected. Cross sections are the same otherwise, so the expected
difference `new - old` in `k_eff` is **zero within statistics**; gate |z| <= 2
of its standard error, a larger shift reported and not tuned. The per-seed sigma
is not known in advance (the pilot measures it); the run uses the example's
default 2000 histories x [20 inactive + 40 active], 6 rings x 4 layers, unless
the clock forces fewer, and the seed ensemble is cut by clock, never by result.
k of this reduced run is a statistics instrument, not a benchmark number.

### Results, gh:#1635 item 2, second attempt (2026-11-17): within the gate

Runs: `OUTRAM_HTR10_K_ONLY=1`, 2000 histories x [20 inactive + 40 active],
6 rings x 4 layers, seeds 20260919-20260924 on each build (six each; the
ensemble was set by clock before any result, and two container restarts
interrupted it: seeds were resumed, none re-drawn or dropped). OLD =
`d1e20f188` + the same example fix (`with_thermal_scattering` on nuclide slots);
NEW = `68b52a0b0` (laws per material).

| seed | OLD k_eff | NEW k_eff |
|---|---|---|
| 20260919 | 0.547218 ± 0.003348 | 0.550315 ± 0.002503 |
| 20260920 | 0.549320 ± 0.003738 | 0.544410 ± 0.004091 |
| 20260921 | 0.551570 ± 0.002480 | 0.543653 ± 0.003083 |
| 20260922 | 0.545045 ± 0.002890 | 0.535304 ± 0.003111 |
| 20260923 | 0.543016 ± 0.003586 | 0.546343 ± 0.003108 |
| 20260924 | 0.551063 ± 0.003573 | 0.544343 ± 0.002982 |
| **mean ± sem** | **0.547872 ± 0.001390** | **0.544061 ± 0.002011** |

The sem is from the seed-to-seed scatter; the quoted per-seed sigmas combine to
0.001347 (OLD) and 0.001299 (NEW), so the example's own error bar agrees with
the scatter on OLD and is about 1.5x small on NEW (n = 6 cannot tell these
apart: the NEW scatter is driven by seed 20260922).

**new - old = −381 ± 244 pcm, z = −1.56: inside the |z| <= 2 gate**, so the
prediction (zero within statistics) holds. The difference is not resolved; an
ensemble this size cannot exclude a shift of a few hundred pcm in either
direction, and none is claimed. k here is a statistics instrument for the
migration, not an HTR-10 benchmark number (reduced run, 293.6 K, UO2 and iron
laws unbound).

Raw: `ensembles_1635/nee_soon_dvm_{old,new}_konly_*.out` (the OLD pilot without
`OUTRAM_PARK_ENDF_DIR` printed SKIP and is not counted or kept); statistics:
`ensembles_1635/dvm_stats.sh`. Test: `tests::materials_cover_every_core_slot`.

## gh:#1644 (2026-11-17): the DBRC-ablated arm, seeds 9-16, new - old -262 +- 65 pcm: prediction

**Written before any run of this pass.** Question: is the OLD build's ablated
arm jump (1.00159 on seeds 1-8 to 1.00441 on seeds 9-16, +282 pcm, about 4.3
sigma) a property of the code or one of a dozen comparisons made across the
#1583/#1635 ensembles?

**Design.** (1) Re-run OLD (`d1e20f188`) seeds 9-16 with the UNMODIFIED driver,
same command line as the #1635 record (`--purr --particles 4000 --inactive 40
--active 100 --seeds 8 --seed 9 --threads 2`, 11 nuclides) and check that the
ablated-arm mean and sem on the `ENDF ablated` summary line reproduce the
recorded 1.00441 (46) bit-for-bit (runs are deterministic per seed). (2) Add a
print-only per-seed line for the ablated arm to the driver (NEW: committed;
OLD: patched in a detached checkout, never committed), then run per-seed
ablated k on both builds for seed blocks 1-8, 9-16 and 17-24, one process per
block per build, the same `--seeds 8` structure as the record, so that a
process-level effect (state leaking between arms or between seeds) would
appear in the same place. The NEW transport source is identical to the
`ed32c976a` used for seeds 9-16 (`git diff --stat ed32c976a HEAD` over
`crates/outram-mc-libs/src`, `crates/njoy-outram-park-fork/src`, `petir/src`,
`Cargo.lock`: empty). Statistics by awk: mean, sd, sem = sd/sqrt(N), paired
new - old with sigma = sqrt(sem_old^2 + sem_new^2), and a leave-one-out look
at the OLD 9-16 block to see whether one seed carries the jump.

**Mechanism considered, from the code (read before predicting).** The ablated
arm is the third loop in `main`, after the ENDF and ACE arms, over
`via_endf.into_iter().map(without_dbrc)`, with `seed + s` for the same seeds as
the other arms; no state is shared across `run_keff_csg` calls that I know of
(each call builds its own tallies). The only ways an arm-specific,
build-specific, block-specific jump arises with deterministic seeds are chance
in the seed draws or a non-deterministic path (thread scheduling at `--threads
2`). I know of no such path and have not yet audited one.

**Prediction.** Chance. (a) The OLD unmodified 9-16 run reproduces the recorded
mean bit-for-bit (to the printed 5 digits). If it does not, the runs are NOT
deterministic per seed and that is the finding. (b) On OLD seeds 9-16 the
per-seed ablated k are spread with sd about 130 pcm and no single seed carries
more than about half of the +282 pcm (a leave-one-out mean still above 1.0030).
(c) Block 17-24 ablated means for both builds fall back inside about
1.0010-1.0030 (the ENDF and ACE arm range; the ablated arm differs from ENDF
only by DBRC, worth about -20 pcm in the 2026-09-30 record), and the paired
new - old |z| <= 2 on 17-24 and on the pooled 24 seeds. (d) Pooled 24-seed
ablated means agree between builds within 2 sigma. **What would indicate a
defect:** a failure of (a); one or two seeds that are outliers by more than 3
sd of the other seeds; the OLD ablated arm staying high (above 1.0035) on 17-24
while the NEW one does not; or the ablated arm's per-seed values drifting with
seed order inside a process (state leaking across runs). Nothing is changed in
the transport and nothing is tuned.

### Results, gh:#1644 (2026-11-17): PARTLY RESOLVED, the OLD per-seed values are NOT obtained

Raw outputs and arithmetic: `ensembles_1644/` (`run.sh` is the exact command
line, `stats1644.sh` the awk, `stats1644_output.txt` its output). Settings as
the #1635 record: `--purr --particles 4000 --inactive 40 --active 100 --seeds 8
--threads 2`, 11 nuclides, `OUTRAM_PARK_ENDF_DIR=reference-data/endf`, blocks
`--seed 1`, `--seed 9`, `--seed 17`. Hardware: the shared 4-logical-core cloud
container used throughout this record (CPU model and RAM not captured here;
load average 8-12 from other agents, so the timings in the raw files mean
nothing and are not quoted). The OLD binary is the unpatched `d1e20f188` build
left by the #1635 session (`rt_old`); the NEW binary is this branch with the
print-only patch.

**What was NOT done, and why.** The OLD print-only patch was prepared (detached
`git archive` of `d1e20f188` plus the same six added lines) but **never built**:
the volume had about 1 GB free, a separate `target-old` would need several GB
(it cannot fit), and building the OLD source into the shared `target-njoy` was
declined by the permission layer (the brief says that directory is not safe for
different source). So there are **no OLD per-seed ablated values**; the OLD
arm below is the driver's own summary means (sem from seed scatter), as in the
original table. Items 1 and 3 of the issue are answered for NEW only, plus the
OLD block means.

**(a) Reproduction, OLD seeds 9-16, unmodified driver: BIT-FOR-BIT.** The
re-run prints `ENDF ablated : k_eff = 1.00441 +/- 0.00046`, `ablated sd 130
pcm`, and its eight `seed N: ENDF ... ACE ...` lines are **identical to the
recorded file** (`diff` empty), so the jump is deterministic in the OLD build
and is not a thread-scheduling or process artefact. NEW seeds 9-16 and 1-8
likewise reproduce the recorded ablated means, 1.00179 (46) and 1.00143 (61).

**(b) NEW per-seed ablated k** (7 decimals in the raw files; pcm = (k-1)*1e5):

| seed | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
|---|---|---|---|---|---|---|---|---|
| k | 0.99887 | 1.00283 | 0.99920 | 1.00341 | 1.00201 | 1.00062 | 1.00165 | 1.00287 |

| seed | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 |
|---|---|---|---|---|---|---|---|---|
| k | 1.00023 | 1.00040 | 1.00300 | 1.00260 | 1.00380 | 1.00109 | 1.00229 | 1.00095 |

| seed | 17 | 18 | 19 | 20 | 21 | 22 | 23 | 24 |
|---|---|---|---|---|---|---|---|---|
| k | 0.99941 | 1.00272 | 1.00132 | 0.99959 | 1.00185 | 1.00374 | 1.00167 | 1.00246 |

**(c) Block means of the ablated arm (pcm above k = 1; sem = sd/sqrt(8)):**

| block | OLD (summary mean, sem) | NEW (per-seed mean, sd, sem) | new - old +- sigma | z |
|---|---|---|---|---|
| 1-8 | +159 (43) | +143 (171, 61) | -16 +- 75 | -0.21 |
| 9-16 | **+441 (46)** | +179 (131, 46) | -262 +- 65 | -4.0 |
| 17-24 | +157 (72) | +159 (149, 53) | +2 +- 89 | +0.02 |
| pooled 24 | +252 (32) | +161 (145 per seed, 30) | -91 +- 44 | -2.07 |

(OLD pooled = unweighted mean of the three block means, sem = quadrature of the
three sems / 3. OLD ablated sd on 17-24 is 205 pcm, on 1-8 122, on 9-16 130.)

**What the blocks say.** NEW is homogeneous across blocks (143, 179, 159; no
trend, sd per seed 145 pooled). OLD blocks 1-8 and 17-24 agree with NEW to
16 and 2 pcm. Only OLD block 9-16 departs: +283 +- 62 pcm (z +4.55) from the
mean of OLD's other two blocks; the three OLD block means have chi2 = 22.9 on
2 dof against their weighted mean (p about 1e-5). The ENDF arm of OLD also
sits slightly high on 9-16 (1.00276 against 1.00235 and 1.00182, +41 and +94
pcm), the ACE arm does not (1.00168 vs 1.00189 and 1.00285).

**Prediction check.** (a) reproduces bit-for-bit: **held**. (b) one seed
carries at most half of the jump: **not testable**, no OLD per-seed values.
(c) block 17-24 ablated means fall back to 1.0010-1.0030 for both builds,
paired |z| <= 2: **held** (OLD 1.00157, NEW 1.00159, z +0.02). (d) pooled
24-seed agreement within 2 sigma: **missed marginally**, z -2.07, and entirely
because of the one OLD block. **Defect indicators:** a failed (a): no; outlier
seeds in NEW: none (all 24 seeds within -1.9 to +1.5 sd of the pooled mean);
OLD staying high on 17-24: no (it returned to 1.00157); state leaking across
seeds within a process: not seen in NEW (block means do not drift with
position in the process, and a fresh process at seed 17 gives the same level
as a process at seed 1), not testable in OLD.

**Code read (no defect found).** `without_dbrc` is `self.dbrc = None` on an
owned `Nuclide` (`src/material/nuclide.rs:1658`), the ablated arm owns its own
vector (`via_endf.into_iter()`, consumed after arm A), `run_keff_csg` is called
with `seed + s` exactly as the other arms, and the driver asserts that the
arm carries the ACE arm's URR tables and no DBRC. I found no shared mutable
state between arms and none was reported by the bit-for-bit reproduction.

**Verdict.** The jump is a **property of the OLD build at those eight seeds,
reproducible bit-for-bit, and not seen in the NEW build or in OLD's other two
blocks**. It is NOT explained: the leading candidate is chance in the
particular random streams OLD gave those seeds (per the #1581 description, not re-read here, OLD draws the URR variate from
the same stream position for every nuclide of a collision, the correlation
#1581 removed; that could fatten the tails of a per-seed k and is a hypothesis
only, untested here), but a +4.5 sigma block in one of six is not a comfortable
chance reading even after counting about a dozen comparisons. What IS
established: (i) it does not reproduce at fresh seeds 17-24 in either build,
(ii) NEW has no such block in 24 seeds, (iii) the per-build difference is
therefore **not a physics shift and must not be quoted as one**, the pooled
24-seed difference of -91 +- 44 pcm (z -2.07) being dominated by one OLD block.
**What would settle it:** OLD per-seed ablated values for seeds 9-16 (the
patched OLD binary, needing roughly 3+ GB of target space or the shared
`target-njoy` with the maintainer's go-ahead) to see whether one or two seeds
carry the +283, then OLD seeds 25-40 to see whether tail blocks like this recur
in OLD. Nothing in the transport was changed and nothing was tuned.

### gh:#1644: OLD per-seed ablated values, seeds 9-16 (2026-11-17)

Built after the maintainer approved building the print-patched OLD tree
(`d1e20f188` + the per-seed print of `e606c312d`, not committed) into the shared
target dir. Same command as `ensembles_1644/run.sh` (`--seed 9 --seeds 8`).
Raw: `ensembles_1644/old_patched_9-16.out`.

**The patch is print-only, checked:** the block summary is again
`ENDF ablated 1.00441 +/- 0.00046`, and the eight per-seed ENDF/ACE lines are
identical to `old_unpatched_9-16.out`.

| seed | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 |
|---|---|---|---|---|---|---|---|---|
| OLD ablated k | 1.00445 | 1.00567 | 1.00233 | 1.00458 | 1.00465 | 1.00320 | 1.00392 | 1.00646 |

**No single seed carries the block.** The lowest seed (11, 1.00233) is still
above OLD's 1-8 block mean (1.00159). Dropping any one seed leaves the block
+252 to +311 pcm above that mean. The within-block sd is 130 pcm, the same as
NEW's 145 pcm. So the whole block is shifted together, with ordinary scatter
around a mean about 280 pcm high.

**What this means.** The prediction "one seed carries at most half the jump"
holds trivially: no seed carries a large share. The result rules out an outlier
seed and leaves two readings:
- a ~4.5 sigma chance fluctuation of eight independent seeds all landing high;
- something correlating the OLD ablated arm across seeds 9-16 in this one
  process, which seeds 1-8 and 17-24 in their own processes do not show.

These cannot be told apart without reading how OLD's ablated arm derives its
per-seed stream, which has NOT been done here. The verdict is unchanged: not a
physics shift between builds (absent at seeds 17-24 in both builds and in all 24
NEW seeds), and still unexplained.

**Code read, OLD `lct008_ace_roundtrip` and `run_keff_csg`'s seeding (2026-11-17).**
- Each seed of each arm is a separate `run_keff_csg` call with
  `set.seed = settings.seed + s`.
- The ablated arm owns `via_endf_ablated`, which is `via_endf` consumed with
  `without_dbrc()`. Its loop shares nothing mutable with the other arms' loops.
- Per-history streams are `future_seed(hist * HIST_STRIDE, future_seed(gen * GEN_STRIDE, seed))`,
  independent of thread count and of any earlier call.
- So seed 13's ablated k is a function of (seed 13, the ablated nuclide set) and
  nothing else. Nothing in the code can couple seeds 9-16 to each other within
  the process, and the reproduction across processes confirms it.
- The OLD ENDF arm, which uses the same seed values and transport, moved only
  +41 pcm between blocks.

This removes the "correlation within the process" reading. What is left is an
unusually large chance excursion (+4.5 sigma in one of six eight-seed blocks,
from about a dozen comparisons), which no further run here can distinguish from
an unidentified effect. **Final position: not a physics difference between the
builds, and not to be quoted as one. The 9-16 block is a deterministic,
unexplained statistical excursion of the OLD ablated arm.**

## gh:#1635 region_temperature re-pin (2026-11-17): prediction

**Written before any run of the migrated test.** Maintainer confirmed
(2026-11-17) that `tests/region_temperature.rs` may be re-pinned, because
appending a bound nuclide copy lengthens the nuclide array and
`transport_csg.rs:2182` advances the URR stream by `nuclides.len()`, so every
history re-draws.

**What changes.** Only the HAND path of
`leu_cell_two_regions_match_the_low_level_path_and_doppler_lowers_k`:
`hand_nuclides()` loads all five nuclides FREE (U-235, U-238, O-16 at 1200 K,
H-1 and O-16 at 293.6 K) and the H-in-H2O LEAPR law is associated to the water
through `MaterialSab::for_materials_containing` and `assign_material_sab`. The
hand array is then 6 long (a bound H-1 copy appended, water's H-1 component
moved to index 5). The MODEL path (`Model::assemble` / `load_nuclides`,
`src/model/data.rs`) is not edited: it still binds H-1 at nuclide level inside
its own loader (keyed by law, marked and audited in #1581) and stays 5 long.
`#![allow(deprecated)]` is removed from the test file.

**Which assertions are physics (must pass unchanged).**
- Test 1 (uniform by region is the uniform model, bit for bit): untouched, LOW
  tier, no hand path; still bit-identical.
- Test 3, Doppler: `k(1200 K) - k(293.6 K) < -3 sigma`, both from the model
  path (unedited); predicted unchanged at -1920 +/- 258 pcm, bit-identical.
- Test 4, the guard (`RunError::NuclideTemperature`), and the temperature and
  nuclide-name assertions on the assembled model.
- Test 5 (delta-tracked majorants, audit, control) and
  `the_test_geometries_draw`: no nuclide-level binding in the hand path;
  unchanged.

**Which assertion is a bit-identity pin (expected to move).**
`hot_run.k_by_cycle == reference.k_by_generation` and the `k_mean` bits: model
(array of 5) against hand (array of 6). Expected NOT to be bit-identical any
more: the URR stream is keyed by array length, so the hand path draws another
realisation. Physics is unchanged (same cross sections, same law, same water
density), so the two must agree within statistics.

**Prediction for the ensemble.** The measured quantity is the hand-path k_eff
(5000 x [20 + 100], the test's own size) over a seed ensemble, OLD build (base
commit `cdd9a695e`, nuclide-level binding, array 5, where hand == model bit for
bit) against NEW build (per-material, array 6). New minus old = 0 within
statistics, no sign predicted; **gate |z| <= 2**, z the difference of means over
the unpaired combined standard error of the means. Controls: the MODEL-path k
at the same seeds is bit-identical between the two builds (the model path is
not edited), and in the OLD build model == hand at every seed. The single-run
sigma of k is about 170 pcm (0.00172 on the mean in the record above), so
N = 12 seeds resolves a mean shift to about 70 pcm per build.

**Plan, fixed before any result.** Seeds 101-112 (N = 12, decided now; cut by
the clock to a prefix of completed seeds and said so, never by result). One
process per build loads the data once and runs every seed through an env knob
(`OUTRAM_RT_SEEDS=101-112`, with `OUTRAM_RT_ENSEMBLE_ONLY=1` to skip the other
legs); the test's behaviour without the knob is unchanged. Same hardware, one
long process at a time.

**Re-pin rule, fixed before any result.** Only if |z| <= 2: the test stops
asserting model == hand bit for bit (no longer true across two array lengths),
asserts instead that model and hand agree within 3 combined sigma at seed 945
(the physics statement), and pins the new hand-path `k_mean` bits at seed 945.
The old pin is recorded struck through in the test doc and here. If |z| > 2
nothing is re-pinned and the miss is reported. No physics tolerance is loosened.

### Results, gh:#1635 region_temperature re-pin (2026-11-17): within the gate, re-pinned

**Hardware:** Intel Xeon (cloud container), 4 logical cores shared with other
runs (one long process at a time), 15 GiB RAM, Linux 6.18, CPU only; rayon 4
threads for the model path, `ThreadCount::Fixed(6)` for the hand path (k is
independent of thread count). Data: ENDF/B-VIII.0 from `reference-data/endf/`.
Timings not recorded as comparable (shared machine).

**Method (as planned above).** OLD binary built from `891f8ccea` (the knob
commit on the base code, nuclide-level binding), NEW binary from this change;
each ran `OUTRAM_RT_SEEDS=101-112 OUTRAM_RT_ENSEMBLE_ONLY=1`, 5000 x [20 + 100],
data loaded once per build, N = 12 as fixed beforehand (not cut). Statistics by
`awk` (unpaired, z = difference of means over sqrt(sem_old^2 + sem_new^2)).
Raw: `ensembles_1635/region_temperature_{old,new}.out`.

| seed | model k (old = new) | hand k OLD | hand k NEW |
|---|---|---|---|
| 101 | 1.492744 | 1.492744 | 1.493173 |
| 102 | 1.491675 | 1.491675 | 1.493670 |
| 103 | 1.494288 | 1.494288 | 1.490877 |
| 104 | 1.493806 | 1.493806 | 1.491343 |
| 105 | 1.488376 | 1.488376 | 1.490113 |
| 106 | 1.491780 | 1.491780 | 1.492666 |
| 107 | 1.490129 | 1.490129 | 1.491281 |
| 108 | 1.489914 | 1.489914 | 1.491559 |
| 109 | 1.492847 | 1.492847 | 1.493155 |
| 110 | 1.492144 | 1.492144 | 1.491629 |
| 111 | 1.492805 | 1.492805 | 1.492983 |
| 112 | 1.490075 | 1.490075 | 1.491650 |

- OLD hand: mean 1.491715, sd 0.001766, sem 0.000510.
- NEW hand: mean 1.492008, sd 0.001093, sem 0.000315.
- **new - old = +0.000293 = +29 +/- 60 pcm, z = +0.49** (gate |z| <= 2: PASS).
- Controls, as predicted: the model-path column is bit-identical between the
  builds (12 of 12 seeds), and in the OLD build model == hand at all 12 seeds.
- The NEW sd (0.00109) is smaller than the OLD (0.00177); with 11 degrees of
  freedom each the variance ratio is 2.6 (two-sided p about 0.15), consistent
  with chance. Not investigated further.

**Re-pin (rule fixed beforehand, |z| <= 2 met).** ~~model == hand bit for bit~~
**RE-PINNED 2026-11-17** to: (a) model and hand agree within 3 combined sigma
(physics statement; seed 945: 1.492429 +/- 0.001330 vs 1.492279 +/- 0.001231),
(b) hand path `k_mean` bits at seed 945 `3ff7e06041575ec6` (1.492279), reason
the nuclide array is 6 long against 5 (the URR stream advances by the array
length), gh:#1635, maintainer-confirmed. No physics assertion was loosened
(Doppler `< -3 sigma`, the guard, the majorant audit and its control, test 1 are
unchanged). `#![allow(deprecated)]` is removed from the test file.
The `1.49292 +/- 0.00172` in the test's module docs predates later changes to the
code and is not re-checked; at seed 945 on current code the model path reads
1.492429 +/- 0.001330.

## gh:#1635 item 3: the four loader files (ugraphite, dhshort, htr10) (2026-11-17): prediction

**Written before any run of a migrated binary.** The four files below each
attached the graphite law to carbon slots with the deprecated
`Nuclide::with_thermal_scattering`. They migrate to per-material binding
(`assign_material_sab`), after which the carbon slots stay free and bound
copies are appended to the nuclide array.

- `examples/common/ugraphite_common.rs` (native, runnable) and
  `dhoby-ghaut/examples/monte_carlo_web/ugraphite/model.rs` (browser): the
  five-nuclide table `[U234, U235, U238, C12, C13]` was indexed by position by
  `Mix::densities()`, `run_case`'s thermal fold and `energy_only_slowing_down`.
  The new code keys those densities by **role** (`vv::ugraphite::Role`,
  `RoleIndex`), looks the bound carbon up through the assembled materials, and
  builds the per-nuclide density vector from the roles. Array: 5 -> 7 entries.
- `dhshort/data.rs` and `htr10/model.rs` (browser-only): compile-checked only, not
  run (no browser here). dhshort already carried a free C-12 at slot 3 and a
  bound copy at slot 4; its migration builds the bound copy by
  `assign_material_sab` on the materials of `dhshort/model.rs`. htr10's
  `FuelZone::new` is given the free table and binds both arms' carbon.

Predictions for what I can run natively (`ugraphite_four_factor` main case and
sweep point, `lumped_ugraphite_kinf` one radius), each falsifiable:

1. **Input-derived output is bit-identical**: the `settings:` line, the
   inventory line, `N [atoms/b-cm]`, the cell-average `N_C/N_U`, the lump and
   cell radii, `uranium volume fraction`. These do not touch transport.
2. **With `GRAPHITE_SAB=0` (the existing explicit ablation) every k, factor and
   energy-only number is bit-identical** before and after: no law is bound, so
   the array stays 5 free nuclides and the materials do not change. This is the
   control: if it moves, the refactor changed something other than the binding.
3. **With the law on, k_inf, the factors and the energy-only `p` are NOT
   expected to be bit-identical, only equal within statistics.** The array grows
   5 -> 7 and U-235 / U-238 carry unresolved-resonance tables, whose stream is
   advanced by the array length per energy change, so histories re-draw. The
   gate for each of k, eta, f, p, epsilon (main case) and k (lumped, r = 0.3 cm)
   is |z| <= 2 on the difference of seed-ensemble means, 6 seeds per arm, `z =
   (mean_new - mean_old) / sqrt(sem_old^2 + sem_new^2)` with `sem = sd/sqrt(n)`.
   Predicted difference: 0 (the same atoms, same law, same data), with the
   usual ~5 % chance per quantity of a 2 sigma miss. Misses are reported, not
   tuned.
4. **The fold check** `fold(Sigma_a,tot)/tally` should stay near 1 within the
   binning error of the old output (the old value is the reference, not 1.0).
   If it moves materially, the role-keyed density vector is wrong (a density
   attached to the free carbon instead of the bound copy would show here and in
   the energy-only `p`).
5. dhoby's ignored native tests of the ugraphite and lumped rungs (processing
   six tapes) check structure, not statistics: predicted to pass before and
   after, once the one assertion that read `nuclides[3]` for the graphite law
   reads the bound carbon through its role.
6. **Any pinned value that moves outside these allowances: STOP and report.**

### Results, gh:#1635 item 3 (2026-11-17/18)

**What changed.** `examples/common/ugraphite_common.rs`,
`dhoby-ghaut/.../ugraphite/model.rs`, `.../dhshort/data.rs` and
`.../htr10/model.rs` no longer call `Nuclide::with_thermal_scattering`; the four
`#[allow(deprecated)]` markers are gone (the #462 scan test
`no_unmarked_nuclide_level_sab_462` passes and prints 21 files that still call
it; I did not record the count before this change). New unreviewed code in
`outram_mc_libs::vv::ugraphite`: `Role`, `RoleIndex`, `Mix::density_of`,
`bind_graphite` (unit test
`binding_graphite_moves_carbon_to_bound_copies_and_roles_follow`).
`Mix::densities` is untouched. `run_case` binds per material and reads its
densities by role; the energy-only walk of `ugraphite_four_factor` does the
same; the dhoby ugraphite/lumped `Loaded` carries a `RoleIndex` (the sigma(E)
panel's C-12 is the bound copy). dhshort builds its live universe with the bound
materials (`model::build_with`, new; `build` is unchanged). No kovan-reviewed
function was edited: none of the touched folders holds a `kind = "review"`
block (checked in `examples/common`, `examples`, `src/vv` and the dhoby
`monte_carlo_web` folders).

**Method.** Before: binaries built from the base commit (the code of
`20cf60a96`, which differs from the base only by the prediction text); after:
the migration commit; both `--release -j 2`, same machine, run sequentially (one
MC process at a time). `ugraphite_four_factor` main case
(`PARTICLES=2000 INACTIVE=15 ACTIVE=40 P_HIST=100000 THREADS=3`) and
`lumped_ugraphite_kinf` (`RADII=0.3 CONTROL=0`, same settings), seeds
20261101-20261106, 6 per arm, plus the `GRAPHITE_SAB=0` control (500 x [5+10],
seeds 777 / 778). Hardware: Intel Xeon @ 2.10 GHz, 4 logical cores (3 used per
run), 15 GiB, Linux 6.18, CPU only; the machine was shared and loaded (load
average 6-8), so wall times are not comparable and are not quoted. The
settings are far smaller than the recorded 5000 x [20+50] runs; the point is
old vs new, not the k itself. Raw: `ensembles_1635_loader/` (`*_old_*`,
`*_new_*`, `compare.out`, `run_arm.sh`, `compare.sh`).

| check | prediction | result |
|---|---|---|
| input-derived lines (settings, inventory, `N`, ratios, radii) | bit-identical | **identical** in 12 of 12 seed pairs |
| `GRAPHITE_SAB=0`, main case (every k, factor, energy-only number) | bit-identical | **identical** (diff of the two outputs, wall time masked) |
| `GRAPHITE_SAB=0`, lumped | bit-identical | **identical** (wall time masked) |

Law on, 6 seeds per arm, z = (new - old) / sqrt(sem_old^2 + sem_new^2):

| quantity | old (sd) | new (sd) | diff | z |
|---|---|---|---|---|
| main k_inf | 1.56997 (0.00278) | 1.56893 (0.00376) | -0.00104 +/- 0.00191 | -0.54 |
| main eta | 2.02810 (0.00005) | 2.02807 (0.00006) | -0.00004 | -1.11 |
| main f | 0.97470 (0.00000) | 0.97470 (0.00001) | +0.00000 | 0.00 |
| main p (3-group) | 0.71339 (0.00064) | 0.71396 (0.00224) | +0.00057 +/- 0.00095 | +0.60 |
| main epsilon | 1.11147 (0.00031) | 1.11153 (0.00070) | +0.00006 | +0.19 |
| main p (energy-only) | 0.71337 (0.00070) | 0.71376 (0.00133) | +0.00039 +/- 0.00061 | +0.63 |
| main fold/tally | 0.99999 | 0.99999 | 0 | 0.00 |
| lumped k_inf | 0.96091 (0.00238) | 0.95963 (0.00105) | -0.00129 +/- 0.00106 | -1.21 |
| lumped eta / f / p / epsilon | | | | +0.50 / -0.30 / +0.13 / -0.06 |

**Prediction check.** (1) held. (2) held: both controls are bit-identical, so
the refactor changes nothing but the binding. (3) held: all gated quantities
are inside |z| <= 2, largest |z| 1.21 (lumped k_inf); no misses to report. The
k's are not bit-identical with the law on, as predicted (array 5 -> 7, URR
stream advanced by the array length). (4) n/a, nothing pinned moved. Caveats:
6 seeds per arm resolve a difference of about 2 x 190 pcm on the main k and
2 x 110 pcm on the lumped k, so this excludes a gross error in the binding,
not a shift of tens of pcm; eta and f have a seed sd at the printed precision,
so their z is quantised and means little beyond "no movement at five digits".
The fold/tally ratio (0.99999 both) is the check that the role-keyed density
vector sits on the bound carbon: a density left on the free copy would move it
and the energy-only `p`.

**Native tests of the browser code** (the four `#[ignore]`d tests that process
tapes, `cargo test --release -p dhoby-ghaut --example monte_carlo_web --
--ignored`, 1270 s, after the migration): all four pass
(`ugraphite::histories_in_the_mixture_end_properly_and_thermalise`,
`lumped::histories_in_the_cell_end_properly`,
`dhshort::data::the_table_is_dh_keff_vvs_order` (now 11 slots: the bound copy at
10, the graphite slot unreferenced) and
`htr10::the_fuel_zone_case_runs_both_arms`). Only the htr10 test measures a k,
so only it was run before as well (one seed, 1000 x [20+60], harness sem):

| arm | record (2026-10-16) | before | after | after - before |
|---|---|---|---|---|
| heterogeneous | 1.57136 +/- 0.00088 | 1.57264 +/- 0.00375 | 1.57766 +/- 0.00384 | +0.00502 +/- 0.00537 (z +0.93) |
| homogenised | 1.44684 +/- 0.00090 | 1.44604 +/- 0.00321 | 1.45263 +/- 0.00377 | +0.00659 +/- 0.00495 (z +1.33) |

Both after-values are about 1.5 sigma above the record, both before-values
within 0.4 sigma; both arms share one seed and one packing, so the two z's are
not independent, and each difference is inside |z| <= 2. A single seed cannot
tell a re-draw from a small shift; no ensemble was run here and nothing was
tuned. The majorant audit is unchanged (worst Sigma_t/Sigma_maj 0.9091 in both
arms, before and after). The other three tests are structural; their
before-state was not run. The dhshort k_inf view and the dhoby ugraphite and
lumped Watch views (browser) are compile-checked only: `cargo check --release
--target wasm32-unknown-unknown -p dhoby-ghaut --example monte_carlo_web`
passes, as does the native `--lib --tests --examples` check of
`outram-mc-libs`, `nee_soon` and `dhoby-ghaut`.

## gh:#1635 nee_soon loader (2026-11-17): prediction

**What changes.** `nee_soon::htr10_rmc::data` and `data_jobs` no longer need
`#![allow(deprecated)]`. The loader's primary product is now `FreeSlots` (every
slot loaded FREE, each distinct law loaded once, `table_of[i]` naming slot i's
law), and the law is associated per material with `bind_tables_per_material` /
`FreeSlots::bind_per_material` (a thin loop over `MaterialSab::
for_materials_containing` and `assign_material_sab`, fraction 1 for every
material that references the slot, which is what the nuclide-level binding did).
The jobs path gains `assemble_entries_free`. The old nuclide-level entry points
(`load_htr10_nuclides`, `load_slots_with_laws`, `assemble_entries`, ...) stay for
the callers not migrated here and are now the free path plus ONE marked
nuclide-level bind (`bind_nuclide_level`), so their results are bit-identical to
before. The measured consumer, `examples/htr10_rmc_keff.rs`, is migrated to the
free path (OLD = the base commit, with `load_htr10_nuclides`).

**Prediction, written before any run.**

1. **Bit-identical, by construction and by test.**
   - The free loader (`load_slots_free_with_laws`) and the free jobs path
     (`assemble_entries_free`) give the same free nuclides bit for bit, the same
     `table_of` and the same number of tables (the pin
     `data_jobs::tests::the_free_paths_agree_and_per_material_binding_matches_the_legacy_slot`).
   - The bound copy appended by `bind_per_material` transports exactly as the
     legacy nuclide-level slot (`xs_at_energy` and `sample_thermal`
     bit-identical at 300 energies).
   - The pre-existing pin `the_jobs_path_builds_the_loaders_nuclides` still holds
     unchanged (legacy loader vs legacy jobs path), as does the `#[ignore]`d
     whole-core pin.
   - Cross sections, laws, materials and densities are the same set of numbers.
2. **Changes within statistics only, not bit-identical, in transport.** The
   default HTR-10 plan has 38 slots; ten of them carry a law that some
   material references (U-235, U-238, O-16 in the UO2 kernel; C-12 and C-13 in
   graphite and in SiC; Si-28, 29, 30 in SiC), so the migrated array has
   38 + 10 = 48 nuclides, the free originals of those ten staying unreferenced
   in place. `transport_csg.rs:2182` advances the URR stream by
   `nuclides.len()` and every HTR-10 nuclide set has URR uranium, so every
   history draws other URR numbers. Expected `k_eff` difference
   (new - old) is **zero within statistics, no sign predicted**; gate
   |z| <= 2 of the unpaired standard error of the seed-ensemble means, a
   larger value reported and not tuned.
3. **Not predicted to change:** the geometry, the majorant (built from
   materials, not from the array) and the lost-particle count (0 expected).
4. **Cost, a guess to be measured:** peak memory rises (ten more nuclide copies,
   of which U-238 and U-235 dominate) and transport time by a few percent at
   most (a longer URR loop per history). Data-processing time is unchanged: the
   same tapes and laws are processed once.

**Plan, fixed before any result.** The fastest real consumer is
`htr10_rmc_keff` (full 12-layer core, 8 rings, K only: it prints k per seed and
takes `OUTRAM_BENCH_SEEDS` seeds inside one process, so the data is processed
once per build). OLD = a git worktree of the base commit `4a8fe06fb`; NEW =
this branch; same example text apart from the load call, same seeds
(`OUTRAM_HTR10_SEED=20260917` plus `OUTRAM_BENCH_SEEDS` successors), same
histories, one long MC process at a time, 2 threads. The full-core run is slow,
so histories are reduced (1000 per cycle, 10 inactive + 30 active; k here is a
statistics instrument for the migration, not an HTR-10 benchmark number). The
ensemble size is cut by CLOCK, decided now: each build runs up to 8 seeds, and
whichever build is slower sets the number of seeds both use, stopping at 3 hours
of wall time per build; never cut by result. Statistics with awk
(`ensembles_1635_nee_soon_loader/stats.sh`).

### Results, gh:#1635 nee_soon loader (2026-11-18): within the gate

Runs: `htr10_rmc_keff`, full core, `OUTRAM_HTR10_HISTORIES=1000`,
`OUTRAM_HTR10_INACTIVE=10`, `OUTRAM_HTR10_ACTIVE=30`, one process per seed, run
one at a time (old, then new, per seed). OLD = `4a8fe06fb`; NEW = `cdd9a695e`.
Seeds 20260917-20260922 on each build: six seeds, about 19 min per run, about
2 h per build, inside the 3 h clock cut. Raw:
`ensembles_1635_nee_soon_loader/{old,new}_seed*.out`, runner `run.sh`,
statistics `stats.sh` (awk).

| seed | OLD k_eff | NEW k_eff |
|---|---|---|
| 20260917 | 1.001385 ± 0.006640 | 0.997219 ± 0.006426 |
| 20260918 | 0.991311 ± 0.007357 | 0.994329 ± 0.006267 |
| 20260919 | 0.988912 ± 0.007328 | 0.995951 ± 0.005368 |
| 20260920 | 1.005977 ± 0.006243 | 0.989836 ± 0.007195 |
| 20260921 | 0.994510 ± 0.004666 | 0.988380 ± 0.005571 |
| 20260922 | 0.994721 ± 0.005977 | 0.995702 ± 0.006347 |
| **mean ± sem** | **0.996136 ± 0.002611** | **0.993569 ± 0.001472** |

The sem comes from the seed-to-seed scatter. The quoted per-seed sigmas combine
to 0.002627 (OLD) and 0.002541 (NEW), so the reported error bars match the
scatter.

**new - old = −257 ± 300 pcm, z = −0.86: inside the |z| <= 2 gate.** The
prediction (zero within statistics) holds. At this run size, shifts of a few
hundred pcm in either direction cannot be excluded, and none is claimed. These k
values come from a reduced run used as a statistics check for the migration;
they are not an HTR-10 benchmark number.

**Not measured:** the memory and per-history URR-cost caveat from the
prediction. The ten unreferenced free originals stay in the array. The box was
shared with other agents' builds and runs, and transport time per run varied
231-592 s with load, so the timings say nothing about a cost difference.

## gh:#1635 item 1: the arms #1583 did not run (2026-11-18): prediction

**Written and committed before any run of this pass.** (The one thing done
before it is reading the drivers; no transport process has been started.) Same
design as "Item 2, last three drivers" above: OLD = `d1e20f188`, NEW = this
branch HEAD (`181fa26d8` when this was written), same data, settings and seeds
in both builds, at most two MC processes of mine at once. sem = sd / sqrt(N);
sigma(new - old) = sqrt(sem_old^2 + sem_new^2); z = (new - old) / sigma. Gate
|z| <= 2 on every comparison, |z| <= 1 also reported. A run cut for time is cut
at the same seed count in both builds, chosen by the clock, never by a result.
A miss is recorded as a miss, and a fresh non-overlapping replication is planned
before it runs and then pooled. Nothing is tuned. Raw outputs:
`ensembles_1635_item1/`. Statistics: awk only.

**Mechanism under test (unchanged).** #1581 appends bound copies of H-1 and the
carbon isotopes to the nuclide array; `transport_csg.rs` advances the URR
stream by `nuclides.len()`, so models with URR-table nuclides draw a different
URR realisation. Same physics, different draws.

**Arm 1: MSRE to the planned N = 16** (`icsbep_five_route_keff --case msre
--route endf --particles 5000 --inactive 40 --active 120 --threads 2`). Seeds
1..8 exist in `ensembles_1583/icsbep_msre_s1-8_*.out`; this pass runs seeds
9..16 on both builds. NEW seed 1 is re-run on this HEAD first and must
reproduce 1.65582 +/- 0.00113 to the printed digits; if it does not, NEW has
moved since `501c16264` and NEW seeds 1-8 are re-run and the record says so.
Prediction: the pooled N = 16 new - old is consistent with zero, |z| <= 2, no
sign predicted. The first N = 8 gave -6 +/- 69 pcm, z -0.09; the seeds 9..16
block is reported on its own as well as pooled. Resolution: seed sd about
120-160 pcm, so sigma(new - old) about 50 pcm at N = 16; a shift under about
100 pcm (2 sigma) would not be seen.

**Arm 2: `lct008-full`** (35 nuclides; same command with `--case lct008`, route
`endf`, same settings), N = 8 seeds per build (seeds 1..8) **if** one process
fits in memory (peak RSS measured on seed 1 first; the box has 15 GB shared with
at most one other of my processes). If not, it is reported not done with the
measured number. Prediction: new - old consistent with zero, |z| <= 2, no sign
predicted. Resolution: no seed sd has been recorded for this case at this size;
assuming 150-250 pcm, sigma about 75-125 pcm at N = 8, so a shift under about
150-250 pcm would not be seen.

**Arm 3: `fhr_ring_rpt_endf`'s other six solves** (explicit-TRISO cube and
sphere, ring-RPT cube and sphere, naive-homogenised cube and sphere, all
delta-tracked, 4000 x [30 + 80], 600 K). The driver has no seed override for
them (the existing `OUTRAM_FHR_RING_SEEDS` wraps only the `csg` branch), and
wrapping its 1500-line `run()` in a loop was judged unsafe. Instead one new env
knob `OUTRAM_FHR_ALL_SEED=S` sets the master seed of every solve, and each seed
is a separate process (data load and TRISO packing paid per process). Unset, the
default is unchanged: this is proved byte for byte (apart from timings) before
the ensemble. The TRISO packing seed is a separate constant and is NOT varied,
so the geometry is the same in every process. N is fixed at **4 seeds per
build** (seeds 1..4) as the minimum; seeds 5..8 are added to both builds if and
only if the clock allows, decided by the clock and not by the 1..4 results.
Prediction, per solve (six comparisons, plus the method deltas they imply):
new - old consistent with zero, |z| <= 2, no sign predicted. Resolution: seed sd
about 100-150 pcm, so sigma(new - old) about 70-105 pcm at N = 4 and 50-75 at
N = 8; a shift under about 150-200 pcm would not be seen. **Multiplicity,
stated in advance:** six comparisons at a 2-sigma gate have about a 26 % chance
of at least one |z| > 2 under the null, so a single marginal |z| between 2 and
2.5 among the six is not by itself evidence of a shift; it would be replicated
on fresh seeds before any claim is made.

### gh:#1635 item 1: results, arms 1 and 2 (2026-11-18)

OLD = `d1e20f188`, NEW = branch HEAD (`181fa26d8` + the prediction commit, plus
the `OUTRAM_FHR_ALL_SEED` knob for arm 3 only; `icsbep_five_route_keff` is
unchanged between `501c16264` and this HEAD, as the seed-1 check below shows).
Both binaries were built `--release -j 2` into the shared target directory and
copied to unique paths. `OUTRAM_PARK_ENDF_DIR` pointed at the main checkout's
`reference-data/endf` (its `../ace` is the populated submodule, where the MSRE
Zr tapes are). Raw outputs: `ensembles_1635_item1/icsbep_{msre,lct008}_*.{out,err,rss}`
(the driver prints `seed N: k = ...` on stderr, so the `.err` files hold the
per-seed lines; `.rss` is the peak `VmHWM` sampled every 30 s from `/proc`, so it
can miss a short spike). Arithmetic: `ensembles_1635_item1/stat.awk`
(= `ensembles_1583/msre_stat.awk`, unchanged), fed by `paste old new`; no
Python. sem = sd / sqrt(N), sigma = sqrt(sem_old^2 + sem_new^2).

**Hardware.** Intel Xeon @ 2.10 GHz (cloud container, KVM), 4 logical cores,
about 15.7 GB RAM, Linux 6.18, CPU only; 2 threads per MC process, two MC
processes of mine at a time, load average 3.4-4.6 (no other agent's heavy
processes seen this time). Per-seed wall times (about 345-355 s MSRE, about 73-76
s lct008-full) belong to this box under this load and carry no cross-run
meaning; the earlier record's 690-705 s MSRE seeds ran beside another agent's
four processes.

**Seed-1 reproduction check (asked for before pooling).** NEW seed 1 on this
HEAD printed `k = 1.65582 +/- 0.00113`, the same value to the printed digits as
`ensembles_1583/icsbep_msre_s1-8_new_501c16264.out`. NEW has therefore not
moved for this driver since `501c16264`; NEW seeds 1-8 were not re-run (seed 1
was run again only as the check, inside the process that ran seeds 9-16).

**Arm 1: MSRE, route `endf`, 5000 x [40 + 120], 2 threads, N = 16**

| seeds | old mean (sd) | new mean (sd) | new - old +- sigma | z |
|---|---|---|---|---|
| 1-8 (recorded 2026-11-17) | 1.65542 (157 pcm) | 1.65536 (118) | -6 +- 69 pcm | -0.09 |
| 9-16 (this pass) | 1.65558 (116) | 1.65562 (158) | +5 +- 69 | +0.07 |
| **pooled, N = 16** | 1.65550 (133, sem 33) | 1.65549 (135, sem 34) | **-1 +- 48 pcm** | **-0.01** |

Per-seed (old / new): 9 1.65568 / 1.65537; 10 1.65728 / 1.65844; 11 1.65705 /
1.65565; 12 1.65596 / 1.65649; 13 1.65516 / 1.65682; 14 1.65506 / 1.65374; 15
1.65413 / 1.65425; 16 1.65431 / 1.65424. Prediction (consistent with zero,
|z| <= 2, no sign): **held**, and |z| <= 1 on every row. Seed sd 116-158 pcm,
inside the 120-160 predicted. Resolution: sigma 48 pcm, so a shift under about
100 pcm (2 sigma) would not be seen. Peak RSS sampled 0.62 GB per process.

**Arm 2: `lct008-full`** (35 nuclides, URR on 10/35, DBRC on 35/35, S(a,b) on),
route `endf`, 5000 x [40 + 120], 2 threads, seeds 1-8 per build. It fits easily:
peak sampled RSS 0.61 GB (old) and 0.63 GB (new) per process, data load 377-378
s, 72-76 s per seed. N = 8 was fixed in the prediction and kept (the seeds are
cheap enough that a larger N was possible; it was not added after seeing the
result).

| arm | old mean (sd) | new mean (sd) | new - old +- sigma | z |
|---|---|---|---|---|
| lct008-full, N = 8 | 1.00129 (148 pcm) | 1.00134 (120) | +5 +- 67 pcm | +0.07 |

Per-seed (old / new): 1 1.00266 / 1.00338; 2 1.00068 / 1.00061; 3 1.00213 /
0.99992; 4 0.99899 / 1.00146; 5 1.00122 / 1.00074; 6 1.00185 / 1.00267; 7
0.99957 / 1.00165; 8 1.00324 / 1.00030 (`ensembles_1635_item1/lct008_pairs.txt`).
Prediction (|z| <= 2): **held**;
seed sd 120-148 pcm, a little under the 150-250 assumed, so sigma 67 pcm is
better than the 75-125 predicted. A shift under about 135 pcm would not be seen.
Absolute k (1.0013) is this run's own value at 5000 x [40 + 120] and is not
compared here with the benchmark or with the 10 000 x [250 + 400] record.

### gh:#1635 item 1, arm 3: first four seeds, a miss, and the replication plan (2026-11-18, written before seeds 5-8 were read)

**What ran.** `OUTRAM_FHR_ALL_SEED=S`, the whole `fhr_ring_rpt_endf` driver once
per seed and per build (separate processes, seven solves each, 4000 x [30 + 80],
600 K, default thread pool, two processes at a time, same box as above, about
20 min per seed per build). The knob's default is proved unchanged:
`fhr_default_csg_new.err` (knob unset) and `fhr_allseed1_csg_new.err` (knob = 1,
the default seed) are byte-identical after masking wall times, and both give
`k_eff (ring-RPT, CSG sphere) = 1.36355 +/- 0.00184`, the value recorded for
this build at `501c16264`. As a harness check the seven-solve driver's CSG solve
for seeds 1-4 reproduces, to the printed digits, the earlier CSG-only runs
(OLD 1.36463 1.36491 1.36322 1.36293; NEW 1.36355 1.36228 1.36255 1.36507).

**Seeds 1-4 (the pre-declared minimum N = 4), per solve** (statistics:
`fhr_extract.awk` pulls the seven k per run, `paste old new | awk -f fhr_stat.awk`):

| solve | old mean (sd) | new mean (sd) | new - old +- sigma | z |
|---|---|---|---|---|
| explicit TRISO, sphere | 1.36102 (80) | 1.36053 (200) | -49 +- 108 | -0.46 |
| explicit TRISO, cube | 1.40077 (180) | 1.40162 (335) | +85 +- 190 | +0.45 |
| **ring-RPT, sphere** | 1.36410 (127) | 1.36141 (107) | **-269 +- 83** | **-3.24** |
| ring-RPT, cube | 1.39862 (216) | 1.39714 (137) | -148 +- 128 | -1.16 |
| naive homogenised, sphere | 1.32871 (58) | 1.33093 (302) | +222 +- 154 | +1.44 |
| naive homogenised, cube | 1.37880 (297) | 1.37865 (96) | -15 +- 156 | -0.09 |
| ring-RPT, CSG sphere (already recorded) | 1.36392 (99) | 1.36336 (126) | -56 +- 80 | -0.70 |

**One solve misses the gate: ring-RPT, sphere, z -3.24 at N = 4.** This is above
the 2.5 that the prediction's multiplicity note set for "marginal", so it is a
miss, not an allowed chance excursion, and it is recorded as one. N = 4 is thin
(the sd of a 4-seed sd is large, and the other solves' sd vary by a factor of 4
between builds, 58 against 302 on naive-sphere), so this is neither a physics
shift nor chance yet. Nothing is tuned and no cause is claimed.

**Replication plan, fixed now, before any further seed is read.** (a) Seeds
5-8 had already been queued on both builds, when seed 1 was still running, as
the "added if the clock allows" extension; they are pooled with 1-4 into N = 8.
(b) For the miss, a fresh non-overlapping replication: seeds 9-16 on both
builds, whole driver per seed, run in order and cut by the clock at the same
seed count in both builds. The replication block (9-16, or as many as ran) is
reported on its own and then pooled into N = 16. Prediction for the replication
block, ring-RPT sphere, if the seeds 1-4 miss was a draw: |z| <= 2 on its own.
If it is a real build difference of about -270 pcm, the block shows about -270 pcm
again. Either outcome is reported as it comes; the pooled N = 16 number is the
one quoted.

### gh:#1635 item 1, arm 3: seeds 1-8 pooled (2026-11-18)

Seeds 5-8 are the extension queued before any result was read; they ran on both
builds and are pooled with 1-4. Per-seed k for all seven solves, both builds, are
in `ensembles_1635_item1/fhr_s{1..8}_{old,new}.err` (the `k_eff` lines);
`fhr_extract.awk` + `fhr_stat.awk` give the table. Seeds 5-8 per solve
(explicit-sphere / explicit-cube / rpt-sphere / rpt-cube / naive-sphere /
naive-cube / CSG), OLD then NEW, as extracted:

```text
seed 5 OLD 1.35757 1.40204 1.36196 1.39628 1.33092 1.38041 1.36607
seed 5 NEW 1.35936 1.39845 1.36235 1.39818 1.32845 1.38153 1.36422
seed 6 OLD 1.36382 1.39965 1.36560 1.39509 1.32989 1.37744 1.36291
seed 6 NEW 1.35997 1.39852 1.36697 1.39835 1.33149 1.38399 1.36255
seed 7 OLD 1.36035 1.39942 1.36171 1.39855 1.33029 1.37733 1.36379
seed 7 NEW 1.36156 1.39994 1.36673 1.39900 1.32918 1.38032 1.36296
seed 8 OLD 1.35817 1.39970 1.36433 1.39798 1.32969 1.37486 1.36225
seed 8 NEW 1.35949 1.40191 1.36338 1.39977 1.32974 1.37818 1.36137
```

| solve (N = 8) | old mean (sd) | new mean (sd) | new - old +- sigma | z |
|---|---|---|---|---|
| explicit TRISO, sphere | 1.36050 (200) | 1.36031 (149) | -19 +- 88 | -0.21 |
| explicit TRISO, cube | 1.40048 (146) | 1.40066 (264) | +18 +- 107 | +0.17 |
| ring-RPT, sphere | 1.36375 (154) | 1.36313 (250) | -62 +- 104 | -0.60 |
| ring-RPT, cube | 1.39780 (196) | 1.39798 (136) | +18 +- 84 | +0.22 |
| naive homogenised, sphere | 1.32946 (95) | 1.33032 (225) | +87 +- 86 | +1.01 |
| naive homogenised, cube | 1.37815 (254) | 1.37983 (212) | +167 +- 117 | +1.43 |
| ring-RPT, CSG sphere | 1.36384 (127) | 1.36307 (117) | -77 +- 61 | -1.26 |

All seven |z| <= 1.43 at N = 8. The CSG row is the -77 +- 61 pcm (z -1.26)
already recorded for this arm, here reproduced from the seven-solve driver; it
is not independent of that record. Block by block, the seeds 1-4 miss of ring-RPT sphere (z -3.24) does not repeat:
seeds 5-8 alone give ring-RPT sphere new - old = +146 +- 150 pcm (z +0.97), the
opposite sign. Seeds 5-8 alone also put two other solves at |z| just over 2 (ring-RPT
cube +185 +- 87 pcm, z +2.13; naive-homogenised cube +350 +- 166 pcm, z +2.10),
neither of which is out of line at N = 8. So across the two N = 4 blocks, three of
fourteen solve comparisons exceed 2 and they do not share a sign or a solve, which is
what a stream re-draw with a heavy-tailed N = 4 sem looks like; it is not a
demonstration of that. Resolution: sigma 84-117 pcm at N = 8, so a shift under about
170-230 pcm would not be seen, and the seed sd of a single solve differs between the
builds by up to a factor of 6 at N = 4 (naive-sphere 58 against 302), which small N
cannot separate from chance. The replication block (seeds 9-12) is below.
