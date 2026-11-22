# gh:#550 third re-measure after the S(a,b) default flip: plan and predictions (2026-11-19)

Research, education and V&V only. **Committed BEFORE any number below is
produced.** A prediction that misses stays in place; outcomes go in
`sab_reference_550.md` ("Third re-measure"). Maintainer brief, 2026-11-19: finish
the items "Not re-measured" in the second re-measure, in the order LCT-008
pitches, then the five-route routes 3 and 5, then the FHR driver's other solves.
The old-build control is the named ablation `OUTRAM_SAB_LOG_SPACED_GRID=1`
(second re-measure, "The old-build control"); no code is changed by this pass
unless said below. Nothing is tuned and no threshold is moved.

## Item 1: LCT-008 pin cell at 1.63576 and 1.75 cm, default route, 24 seeds

Protocol of `gap_240_550.md` step 1: case-1 borated water, 11 nuclides
(`--cheap-nuclides`), 5000 x [50 + 200], 293.6 K, tape-data route, node-grid
S(a,b) (the default). Seeds 1-8 are the recorded `flip_550/pitch_flip.csv`;
seeds 9-24 are new (`--seed 9`/`17`, `OUTRAM_PITCH_SEEDS=8`, the same example and
flags, so the 8-seed block is exactly reproducible per seed, `gap_240_550.md`
step 5). OpenMC 0.15.3 reference: the existing 16 seeds of
`ensemble_550/openmc_seeds.csv` (`k_gen_mean`); more OpenMC seeds only if time
allows, and then announced before reading.

What is on record going in (computed from the committed CSVs, 2026-11-19):

| pitch | OpenMC (16 seeds) | pre-flip outram, 16 seeds, minus OpenMC | flip, 8 seeds | flip 8 seeds minus OpenMC |
|---|---|---|---|---|
| 1.63576 | 1.06253 +- 0.00022 | -37 +- 38 | 1.06323 +- 0.00039 | +71 +- 45 |
| 1.75 | 1.01104 +- 0.00033 | -1 +- 37 | 1.01168 +- 0.00033 | +64 +- 47 |

At 2.40 cm the same flip moved the default route from -117 +- 39 to -10 +- 28
against OpenMC at 24 seeds, i.e. about +106 pcm; the 8-seed flip moves were
+107 +- 50 (1.63576) and +64 +- 37 (1.75). The weighted mean of the three flip
moves is about +90 +- 23 pcm. The 8-seed 2.40 value (-85) turned out to be a low
draw; the same can happen to either of these 8-seed values in either direction.

**Predictions, default route minus OpenMC at 24 seeds (sign, size, z):**

| pitch | predicted | range | z | gate (|z| <= 2) |
|---|---|---|---|---|
| 1.63576 | **+45 pcm** | -30 .. +120 | about +1.2 (sigma about 31: 24 outram seeds sem 22, OpenMC 22) | pass |
| 1.75 | **+55 pcm** | -20 .. +130 | about +1.4 (sigma about 40: outram 22, OpenMC 33) | pass |

Reasoning: pre-flip difference plus a flip move of about +80 (the three moves
above, shrunk toward their mean because each is a single noisy estimate) gives
+43 and +79; the 8-seed direct measurements (+71, +64) pull the second one
down, and I take the middle. Both are *positive*: the node grid raises k in
H-in-H2O (first re-measure: +32, +107, +64, +39 pcm).

**Refutation.** (a) A 24-seed difference outside the stated range at either
pitch. (b) Either difference negative beyond -30 pcm: the flip would then not
raise k at that pitch, against four independent pin-cell and HST-009 moves of
the same sign. (c) |z| > 2 at either pitch: that is a gate failure, reported
as one, not repaired. The probability I put on (c) at least once of two:
about 25 %, since the predicted offsets sit 1.2 to 1.4 sigma from zero.
The 24-seed mean minus the 8-seed mean (seeds 9-24 against 1-8) is predicted
-20 pcm at 1.63576 and -10 at 1.75 (regression of a 1.5-sigma high 8-seed
draw), each +-40; not a refutation condition.

**36-nuclide tier at 2.40 cm: only if time remains after items 2 and 3**, and
then 8 seeds, default route, announced as not powered: its OpenMC reference is
a single run per pitch (`data/pitch_sweep_full36_case1.csv`), so no gate is
claimed for it.

## Item 2: `icsbep_five_route_keff` routes 3 and 5

Read from the code (2026-11-19): route 3 (`--route ace --ace-dir <NJOY2016>`) and
route 5 (`--route ace --ace-dir <Rust-NJOY>`) take the H(H2O) law from
`ThermalScattering::from_ace` on `293.6K/HinH2O.ace`
(`icsbep_five_route_keff.rs`, `"ace"` arm), never from `from_tape`; Godiva and
Jemima carry no S(a,b) at all; `msre` is refused on these routes. The knob
`OUTRAM_SAB_LOG_SPACED_GRID` is read only inside `from_tape` and
`from_tape_with_options`. So the flip cannot reach routes 3 and 5.

**Prediction: knob-on and knob-off give bit-identical k, k_std and entropy per
seed (difference exactly 0 pcm, every printed digit) on HST-009, routes 3 and 5,
seeds 1-3 (5000 x [40 + 120], 2 threads).** Refutation: any difference at any
printed digit: then something on the ACE route reaches the ENDF thermal build,
and the "by construction" statement in the second re-measure (and this plan) is
wrong. Positive control, so that "identical" cannot be a dead knob: the same
binary and seed 1 on route 4 (`--route endf`, which does build through
`from_tape`) with the knob on and off must DIFFER (the second re-measure saw
+73 +- 47 pcm over 16 seeds and seed 1 differing); a route-4 control that does
not differ would void the whole item. The Rust-NJOY library (route 5) and the
NJOY2016 library (route 3) for HST-009's 12 nuclides are built in scratch (not
committed); if route 5's library cannot be built in the time, route 5 is
reported not run and the code-reading statement stands as unmeasured. Not run
at all, deliberately: Godiva, Jemima (no S(a,b), nothing to flip), the
35-nuclide lattice on routes 3 and 5 (same ACE S(a,b) path as HST-009; 35
nuclides x two libraries is hours for a result fixed by the same line).

## Item 3: `fhr_ring_rpt_endf`, the seven solves

The driver already has the knob `OUTRAM_FHR_ALL_SEED=S` (gh:#1635 item 1: one
master seed for every solve, no code change needed here; the TRISO packing seed
is a separate constant and does not move). Protocol: the whole driver (seven
solves: explicit TRISO, ring-RPT and naive-homogenised, each on the r = 3 sphere
and the cube, plus the ring-RPT CSG sphere), 4000 x [30 + 80], 600 K, one process
per seed per arm, seeds 1 and 2 (3 and 4 only if the clock allows, decided by the
clock and not by what seeds 1-2 read), default arm against
`OUTRAM_SAB_LOG_SPACED_GRID=1`. About 20 min a seed per arm on a shared 2-thread
slot, so these are the lowest-power runs of the pass: single-solve seed sd is
100-300 pcm, so sigma(new - old) at N = 2 is about 150-250 pcm.

**Predictions.** (i) Controls, exact: the CSG solve with the knob on reproduces
the second re-measure's knob-on values for seeds 1 and 2 (1.36355 +- 0.00184,
1.36228 +- 0.00172, `sab_reference_550.md`), and the default-arm CSG solve
reproduces `remeasure2_550/fhr_new.out`'s per-seed default values (to be read
from there before the comparison). A mismatch means the code has moved under
me and I stop and say so. (ii) The six delta-tracked solves: new minus old
consistent with zero, no sign predicted for any one (graphite S(a,b) with a
hotter stationary neutron temperature argues small and negative, -15 pcm; the
second re-measure's two ring-RPT graphite-pebble arms went +77 and +192, so I
do not rely on it). Seven comparisons at 2 sigma: about a 30 % chance that at
least one has |z| > 2 under zero true effect, so a lone marginal value is not
a finding. Refutation of "consistent with zero": a mean over the seven solves
beyond +-300 pcm at N = 2, or any single solve beyond 3.5 sigma. Resolution is
too poor to see anything under about 300 pcm; I say so now so that a null is
not read as a bound.

## Gates and disclosures

- Hardware for every run: Intel Xeon @ 2.10 GHz (cloud container, KVM), 4
  logical cores, 15 GB RAM, Linux 6.18, CPU only, shared with another agent.
  Two heavy processes at once, 2 threads each.
- No `review.md` target is touched: this pass changes no library function. Any
  example change is stated in the record.
- Re-pins: only if a pinned k value moves, is resolved, and is due to the flip.
  None expected from these runs (they are not pinned).
