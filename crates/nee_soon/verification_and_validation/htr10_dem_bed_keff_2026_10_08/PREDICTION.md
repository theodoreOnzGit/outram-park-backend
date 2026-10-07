# HTR-10 k_eff on the DEM random bed: prediction, written before the run (2026-10-08)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** Research, education and V&V only; not
> for any operational use. AI-assisted (Claude Opus 5.5 in Claude Code), not
> yet reviewed by the maintainer.

Written and committed **before** the smoke run and before the full run, so
before any k on this bed at these settings was read. The approval is option A
of the maintainer's 2026-10-08 comment on gh:#787.

## What will be run

- **Bed:** the gh:#216 bed, `reference-data/liggghts/htr10_conus_presettled_mu10_mur00.csv`
  (`GranularSystem`, µ = 0.1, µ_r = 0; 27 554 pebbles), cut to the lattice's
  **16 681** balls above the floor at N = 12. The cut is the beds view's rule:
  every pebble at or below the floor, and the lowest 16 681 above it
  (`explicit_bed::trim_to_core_balls`).
- **Identity:** `paper_fuel_assignment`, 57:43 above the floor: 9 508 fuel
  balls of 16 681. Conus and tube are all dummy.
- **Driver:** the recorded runs' own `examples/htr10_rmc_keff.rs`, with one new
  knob, `OUTRAM_HTR10_DEM_BED=<csv>`. It swaps in
  `assemble_explicit_triso_from_centres` and leaves everything else as it is:
  data, materials, bounded majorant (#589), source box, entropy mesh, and the
  reference read at the equal-ball-count height.
- **Settings:** the lattice record's (`../htr10_seker_2026_10_07_10k/RUN_PARAMETERS.md`):
  - 10 000 × [5 + 135];
  - ENDF/B-VIII.0 with 30P graphite;
  - seed 20260917;
  - 14 rings, `OUTRAM_HTR10_LAYERS=12`;
  - every ablation off: URR, DBRC and S(α,β) on, as defaults;
  - **5 threads** (the lattice record used 7).
- **Compared against:**
  - the lattice: N = 12 on VIII.0 in the 2026-10-07 record, **0.995125 ± 0.001055**;
  - RMC at the equal-ball-count height, 122.091 cm: **0.999419**. The lattice
    sits −429 ± 106 pcm from it.

## Every way the DEM-bed core differs from the lattice core, besides the packing

Measured on the built geometry by
`explicit_bed::tests::the_dem_bed_cut_to_the_lattice_ball_count_is_the_beds_view_cut`
(2026-10-08):

| | lattice, N = 12 | DEM cut | note |
|---|---|---|---|
| balls centred above the floor | 16 681 | 16 681 | equal by construction |
| fuel balls | 57:43 rule | 9 508 (57:43 rule, ordered by height) | same rule, different positions |
| **conus balls** (all dummy) | **2 065** | **2 219** | **+154 (+7.5 %) graphite balls in the conus** |
| tube balls (all dummy) | 1 936 (Şeker's ordered stack) | 238 DEM + 1 721 Şeker filler | ordered filler below the 25 cm DEM tube, with a 0.62 cm helium gap at the junction |
| bed top | top of Şeker's whole top layer, 123.576 cm | flat cut through the packing; highest ball top 125.085 cm, p99 surface 123.692 cm | the top is ragged within about one radius |
| overlaps | none | 51 781 pairs split at the bisector; lens volume 1.27e-5 of the ball volume | 1.3e-5 less graphite, negligible |
| wall penetration | none | up to 0.032 cm of a ball cap clipped by the envelope | negligible |
| near-wall structure | hex lattice, balls rejected at the 90 cm wall | the poured wall layering | part of "packing" |
| entropy mesh and source box | from the lattice bed's extent | from this bed's extent | changes the entropy trace's binning, not k |

The cut is the beds view's cut. Quantising to u16 cannot change which balls
are kept, because the step is 3.75e-5 m, the gap at the cut is 8.68e-5 m, and
the nearest centre to the floor is 5.98e-5 m away. The test asserts this.

The code has changed since the lattice record (`dbb9e26e`). The only commit
to `outram-mc-libs`, `nee_soon` or `njoy-outram-park-fork` sources that
touches transport is `7ee55f283a`, the #784 tracking events. It draws no
random number, and its traced-equals-untraced and bit-identity tests pass
unchanged. **The lattice point is not re-run here**, as the brief requires.

## Prediction

**k = 0.9960, with Δk = k_DEM − k_lattice ≈ +100 pcm. The range is −200 to +400 pcm.**
That is consistent with zero at the resolution of this comparison: σ of the
difference is √(105² + 105²) ≈ 150 pcm.

Reasoning, term by term:

1. **Packing at equal ball count and equal fill.** The two beds hold the
   same balls at the same whole-core φ: 0.5999 for the lattice and 0.5994 for
   the DEM bed in the beds view. Fuel inventory, moderator inventory and the
   moderator-to-fuel ratio are therefore the same. What changes is how
   pebbles shadow one another (the pebble-level Dancoff factor) and the
   near-wall layering. At equal φ I expect the first to be small, under
   ~200 pcm in either direction. I cannot sign it from first principles with
   any confidence, and I will not quote a literature number I have not read.
2. **The conus holds 154 more graphite balls (+7.5 %).** That is about
   17 400 cm³ of graphite replacing helium below the core, which is a better
   bottom reflector. **Sign: +.** Size: about 50–100 pcm. This scales a past
   record: replacing the conus's dummy balls with solid reflector graphite
   (about 45 points of graphite fraction) was worth about 680 pcm (the
   2026-09-18 `htr10_rmc_keff` notes), and this change is about 4 points.
3. **Top and tube.** The flat-cut top moves no balls; it only roughens the
   surface within one radius. The tube differs by 23 balls, 2 m below the
   core. Each is worth well under 50 pcm.

Taken together the prediction is a small positive shift. The sign rests on
term 2, and term 1 could reverse it.

**What would surprise me:** |Δk| > 450 pcm, which is 3σ of the difference.
That would mean random packing itself is worth hundreds of pcm at equal φ in
this core. Then, as the approval says, option B (three pours) is needed
before any of it can be called the packing's worth. One pour cannot separate
the packing from its own realisation.

**The reference.** RMC at 122.091 cm is 0.999419, so the prediction puts the
DEM bed at about **−330 pcm** from RMC, against the lattice's −429.

## Cost prediction

Measured costs on record:

| run | CPU per history |
|---|---|
| DEM-bed smoke, 2026-10-05, i9-13900K, 12 threads | 30.6 ms. **Before the #589 majorant fix**: commit `a81ecaf9e8`, 07:55 UTC, against `b208cecc7` at 10:57 UTC. |
| lattice N = 12, VIII.0, 2026-10-07, Xeon W-2195, 7 threads, five runs at once | 38.9 ms (7 771.6 s × 7 / 1.4 M). After the fix: 8 926 virtual collisions per history. |
| lattice, 2026-10-01, i9-13900K, 5 threads | 8.5–14.8 ms, before the fix. |

The fix multiplied the virtual-collision count, so the pre-fix smoke figure
is a floor.

- **Per history:** 20–60 ms of CPU.
- **Full run:** 1.4 M histories, so **8–23 CPU-h**, or **1.6–4.7 h on 5
  threads**, central estimate about 3 h.
- **Data processing:** another 2–5 min.

The smoke run below measures the cost directly and replaces this estimate.

## Addendum, 2026-10-08, after the smoke run and before the full run

The section above is unchanged. This only records the smoke run
(`logs/smoke_1000x2p3.log`): 1000 × [2 + 3], 5 threads, same build and
settings otherwise.

- **Health:** 5 000 histories with 0 lost locates, 0 stuck events and 0
  negative distances. The geometry has 22 427 tiles, 88 090 cells and 2 285
  universes. The ball count matched at 122.091 cm, where RMC reads 0.999419.
  The entropy trace was 5.463, 5.504, 5.417, 5.522, 5.486 bits (ceiling 6).
  That is sane and flat at this size; it shows nothing about convergence.
- **k = 0.995483 ± 0.007653.** At σ ≈ 770 pcm this says nothing about the
  prediction and is not a result.
- **Cost:** nuclear data 88.8 s; transport 15.5 s, which is **3.1 ms wall and
  15.5 ms CPU per history**. There were 9 025 virtual collisions per history,
  close to the lattice's 8 926. **That is below the predicted 20–60 ms.** The
  prediction came from the slower shared Xeon and the 12-thread smoke run.
- **Revised wall time for the full run:** 1.4 M × 3.1 ms ≈ **1.2 h**, if the
  per-history cost holds at full size.
