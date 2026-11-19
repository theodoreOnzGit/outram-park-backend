# The 2.40 cm LCT-008 gap on the default route: plan and predictions (GitHub #550, 2026-11-19)

Research, education and V&V only. **This file is committed BEFORE any per-nuclide
swap is run.** A prediction that misses stays in place; outcomes go in
`gap_240_550.md`. Nothing below was informed by a per-nuclide cross-section
comparison of the port against the `.h5` (that comparison is done after this
commit, as a mechanism step, not as a source of these numbers).

Maintainer decision, 2026-11-19: "Chase the 2.40 cm gap".

## What is established going in (read from the records, not re-run)

- Default route (port RECONR/BROADR neutron data, ENDF-built S(a,b) on the
  calcem node grid): 2.40 cm k = 0.70568 +- 0.00024 (8 seeds), -85 +- 38 pcm vs
  OpenMC (0.70652 +- 0.00020, 32 seeds), z -2.26.
- Arm E (OpenMC `.h5` neutron data, same node-grid S(a,b)): 0.70651 +- 0.00039
  (8 seeds), -1 +- 44 vs OpenMC. Seeds 1-8 in both arms.
- Per-seed values (`flip_550/pitch_flip.csv`, `sab_reference_550/arms/E_h5_nodegrid.csv`),
  E minus default: unpaired +83 +- 46 pcm (z 1.8). The seed is shared, so the
  initial source and the first generations are shared; paired differences on
  seeds 1-8 are +34, -1, +191, +199, +112, -32, +111, +52 (mean +83, sd 85,
  sem 30, t = 2.8 on 7 df). **I computed this paired figure after writing the
  plan above, from numbers already in the repository; how much the shared seed
  correlates the two arms is itself uncertain at n = 8, so the premise check
  adds seeds rather than relying on either figure.**
- H5 of the earlier diagnosis (OpenMC tallies against the port's factors on the
  OLD log-grid baseline): f is the only factor that moves (-0.06 % relative, 2.2
  sigma); eta, p and eps agree to 5e-5. So whatever remains, if it is real,
  lives in the thermal utilisation f (B-10 44 % of thermal absorption, H-1 15 %
  per OpenMC's tally), not in resonance capture or fast fission.
- The 11-nuclide `.h5` library at `openmc-libs/lct008_pin11/h5_njoy` is NJOY2016
  RECONR/BROADR/UNRESR/PURR/ACER of the same ENDF/B-VIII.0 tapes, at 0 K and
  294 K. The `.h5` route sets the free-gas kT from the data (294 K =
  0.025334 eV) where the tape route uses 293.6 K (0.025300 eV), a 0.13 %
  difference in kT for O-16 and the other free-gas scatterers (read from
  `Nuclide::free_gas_kt`, which cites OpenMC `physics.cpp:734`).

## Plan

1. **Premise.** Is default-minus-E at 2.40 cm real? Extend both arms from seeds
   1-8 to seeds 1-24 (same protocol, 5000 x [50+200], 11 nuclides), report
   unpaired and paired.
2. **Mixing mechanism.** New example flag `lct008_pitch_sweep --hdf5-nuclides
   LIST` (with `--hdf5-dir`): the listed nuclides come from the `.h5`, every other
   nuclide of the tier from its ENDF tape. No library code is touched; the
   selector is `nuclide_from_hdf5`, reached by the example's unit test.
   (Searched first: `Nuclide::from_hdf5` and `from_endf_file_with_speed` are the
   two loaders; `lct008_pitch_sweep::load_nuclides` already chose between them
   for the WHOLE tier; no per-nuclide mixing existed in `examples/` or `src/`.)
3. **Swaps.** From the default (all tape), put one nuclide (or a group) on the
   `.h5`. A shift of 20-30 pcm cannot be resolved with 12 seeds (sigma of a
   difference to the 24-seed default arm is about 25-30 pcm unpaired), so
   groups come first where a single nuclide is predicted small, and a
   cross-section comparison against the `.h5` in the relevant energy range is
   run to decide where transport time is worth spending.
4. **Cross-section comparison** of the implicated nuclide(s), port vs `.h5`
   (and the NJOY ACE/PENDF if needed), then the upstream routine that owns any
   difference.

## Predictions, signed and sized (effect on k at 2.40 cm of moving ONE nuclide
## from the port's data to the `.h5`, all else default)

The leverage argument used throughout: f = 0.40 here, so a relative change
`d` in a nuclide's thermal absorption share `s` of the total thermal absorption
moves f, and so k, by about `-d * s` (absorber outside the fuel) in relative
terms; 0.1 % in B-10 (s = 0.44) is therefore about 44 pcm, 0.1 % in H-1 (s =
0.15) about 15 pcm. The processing differences expected between two NJOY-grade
reconstructions of one evaluation are 1e-4 to 1e-3 of the cross section at
thermal energies, so single-nuclide effects are expected at the 0-30 pcm level.

| id | nuclide | predicted effect of moving to `.h5` | reasoning |
|---|---|---|---|
| P1 | H-1 | **-5 pcm** (range -40 .. +30) | thermal capture is inside S(a,b)'s hydrogen (free capture xs above 0 is 1/v from the same tape); 15 % share, so a 1e-4 xs difference is 1.5 pcm. The `.h5` carries 294 K kT; the S(a,b) is 293.6 K in both. Sign: unknown, slightly negative because 294 K is the hotter data |
| P2 | O-16 | **0 pcm** (range -15 .. +15) | negligible absorption (4e-5 of thermal absorption); only elastic angular law and the free-gas kT differ (0.13 % in kT, about -1 pcm) |
| P3 | U-235 | **0 pcm** (range -25 .. +30) | eta agrees to 5e-5 between the codes, so thermal fission/capture ratio cannot differ by more than about 5 pcm; the unresolved-resonance range and fission spectrum differences act on a minor part of the neutron economy |
| P4 | U-238 | **0 pcm** (range -20 .. +20) | p agrees to 5e-5 between the codes (resonance capture), which bounds this swap at about 5-10 pcm in the resonance region; 0 K elastic for resonance scattering differs in construction (port: its own RECONR 0 K; `.h5`: NJOY 0 K), and 294 K versus 293.6 K broadening of the 6.67 eV resonance is the sign-negative candidate (hotter = more capture) of order -2 pcm |
| P5 | B-10 | **0 pcm** (range -30 .. +30) | the largest leverage (44 % of thermal absorption) but a 1/v tape; both routes give 1/v to 1e-4 below 1 eV. A 1e-4 difference would be 4 pcm |
| P6 | the rest (U-234, Al-27, Si-28/29/30, Mn-55) | **0 pcm** (range -20 .. +20, group) | shares of thermal absorption under 3 % each; U-234 has the largest (low-energy resonance) |

**Joint prediction.** All eleven on the `.h5` (arm E) minus default: **+30 pcm,
range -30 .. +110**, a regression towards zero from the observed +83 (the
earlier same-neutron-data comparison with the log-grid S(a,b) was B minus
baseline = +23 +- 38; the full LCT-008 lattice agreed between the routes at -8
+- 9 pcm over 96 seeds). At 24 seeds each I expect the unpaired E minus default
to read **+35 +- 25 pcm** (z about 1.4). If it comes out at +80 or more with
sigma 25, my regression-to-the-mean expectation was wrong and there is a real
route difference to find.

**What would refute the plan's framing.** (a) A single-nuclide swap giving
more than 2 sigma (about 50 pcm) while the joint is under 30: then the sum of
parts does not add and I look for a cancelling second nuclide or an
interaction. (b) E minus default under 20 pcm at 24 seeds: the premise
dissolves and the -85 pcm is a statistical fluctuation of the 8-seed default
arm (to be tested by OpenMC-minus-default at 24 seeds).

**Prior on the cause, stated now.** Statistical fluctuation of the default
arm's 8 seeds: 35 %. A real route difference carried by thermal absorption
(B-10 or H-1 cross section processing): 25 %. By the 294 K versus 293.6 K
temperature difference of the data (the `.h5` is NOT at the model's 293.6 K;
this is a data-processing option, not a defect): 15 %. By U-238 or U-235
resonance treatment (URR, DBRC 0 K): 10 %. Other: 15 %.

## Instrument and protocol

`lct008_pitch_sweep --pitches 2.40 --particles 5000 --inactive 50 --active 200
--cheap-nuclides`, `OUTRAM_PITCH_SEEDS=N` consecutive seeds in one process,
2 rayon threads, at most two such processes at a time. Statistics: unpaired
Welch and paired (shared seed) both reported; the one I quote is named in the
record. No threshold, tolerance or input is tuned.
