# H in H2O S(a,b): which processing is closer to the ENDF/B-VIII.0 evaluation? (GitHub #550, started 2026-11-18)

Research, education and V&V only. **Predictions (this section) are committed
before any number below it is computed.** A prediction that misses stays in
place with its outcome beside it.

## Question

At the 2.40 cm LCT-008 pitch the outram-mc ENDF-route k is 116 pcm below
OpenMC, and swapping only the H-in-H2O S(a,b) table for NJOY2016's
`HinH2O.ace` closes it (+135 +- 35 pcm, `pitch_sweep.md`). The two tables'
cross sections agree to 1.3e-4; their sampled outgoing-energy kernels differ
(mean E' ACE minus ENDF-built: +1.1 % at 0.1 eV, -0.6 % at 1.5 eV, +2.2 % at
3.5 eV). Neither table is an oracle of the evaluation. This record integrates
the evaluation's own S(alpha,beta) directly and compares both.

## What I read first (upstream before hypothesis)

- `thermr.f90` `sig` (l. 2482-2615): d2sigma/dE'dmu = sqrt(E'/E)/(2 kT) *
  sigma_b * exp(S - beta/2) with S = ln S(alpha,beta) interpolated
  **quadratically** (`terpq`) in alpha and beta; beyond alpha_max / beta_max
  the short-collision-time (SCT) law with T_eff; for `LAT = 1` alpha and beta
  are rescaled by kT/`tevz`; for `LASYM = 0` only beta >= 0 is stored.
- `calcem` reads the tape (l. 1650-1800): `sb = B(1)/natom * ((A+1)/A)^2`.
- The tape `tsl-HinH2O.endf`, 293.6 K: header says `LAT = 1`, `LASYM = 0`,
  `LLN = 0` (S itself, not ln S), `NI = 12`, B(4) = 10.00008 eV; S(alpha)
  and S(beta) interpolation codes are `INT = 4` (ln S linear in the argument),
  which is *not* NJOY's quadratic law: the evaluation's stated law and the
  processing's law differ, so the reference is computed under both.
- Port side: `IncoherentInelasticScattering::emission(e, ..)` returns the
  `calcem` record **nearest** to `e` (`nearest_record`, documented: "ACE stores
  one block per tabulated incident energy"). outram-mc's
  `ThermalScattering::from_tape_with_grids_and_options` calls it at 384+
  **log-spaced** energies, so each table is labelled with a log-grid energy but
  carries the E' of the nearest `calcem` node (EGRID has 118 nodes, 4-10 %
  apart). NJOY's ACE stores the node tables on the node energies. The tape-built
  table in outram-mc is therefore a staircase in incident energy, where the ACE
  table is a linear interpolation between node tables.
- `HinH2O.ace` (the file OpenMC's `.h5` came from): NXS says 64 outgoing
  energies and 16 cosines per bin, equiprobable (IFENG 0); outram-mc's tape
  route builds 64 x 8 (`N_OUTGOING`, `N_COSINES`).

## Predictions (before any run)

| id | prediction | what would refute it |
|---|---|---|
| R1 | Reference at the five named energies: sigma_inel within 1e-3 of the port's `calcem` xsi; <E'> within 3e-3 of the port's node table at node energies (0.625 eV, and 0.0253 eV is a node) | a larger miss at nodes means my reference or `calcem` has a convention error, and I stop and find it before comparing tables |
| R2 | At **node** incident energies both processed tables agree with the reference to <= 3e-3 in <E'> (equiprobable discretisation limits it) and with each other to 1e-4 | any node disagreement |
| R3 | At **between-node** energies the ACE table (linear interpolation of E' between node tables) is closer to the reference than the ENDF-built one: abs(<E'>_ACE - ref)/ref <= 5e-3, abs(<E'>_ENDF-built - ref)/ref up to 2-4 % (half a node spacing, 4-10 %, times the E' sensitivity ~ 0.5-1) | ACE worse than 1 % at a between-node point, or ENDF-built better than ACE at most between-node points |
| R4 | The ENDF-built deviation is a pure incident-energy relabelling: <E'>_ENDF-built(E) / <E'>_ref(E_node(E)) = 1 within 3e-3, where E_node(E) is the node `nearest_record` picks for the log-grid point E | a residual > 1e-2 means something besides the labelling differs |
| R5 | Cosine count (8 vs 16 per bin) changes <E'> and <E'^2> of a node table by < 1e-3 and <mu> by < 5e-3 | a larger change |
| R6 | The INT = 4 (stated) law and NJOY's quadratic law give reference <E'> within 2e-3 of each other | a bigger gap, in which case "closer to the evaluation" depends on which law is the evaluation's |
| R7 | The THERMR port (`thermr::calcem`) is **not defective**: its node tables are NJOY2016's word for word (`acer_thermal_word_parity.rs`), so by the port-is-the-oracle rule nothing there changes. The defect, if any, is in outram-mc's use of `emission` at non-node energies | |
| R8 | Reviewed status: no function involved (`ThermalScattering::from_tape_with_grids_and_options`, `IncoherentInelasticScattering::emission`) carries a `kind = "review"` block (the `review.md` of `outram-mc-libs/src/material` and of `njoy-outram-park-fork/src/thermr[/calcem]` hold none) | a stamp found |
| R9 | A tape-built table on the calcem node grid (as ACE has it) reproduces the ACE table's moments to sampling error at all energies, and moves the 2.40 cm k by **+60 pcm** (range -20 .. +150, 8 seeds, so +-35 pcm statistical). Honest prior that this alone accounts for most of the +135 pcm: **35 %**. A staircase with random-signed label errors is unbiased to first order, so I do not expect it to carry all of 135 pcm | the k shift beyond the range, or the tables still differing after the grid change |
| R10 | The other two pitches (1.63576, 1.75 cm) do not move beyond statistics (shift < 2 sigma of about 60 pcm) | a resolved shift |

## Results (2026-11-18/19)

Hardware for every timing and run: Intel Xeon @ 2.10 GHz, 4 logical cores,
15 GB RAM, Linux 6.18.44, CPU only, a VM shared with other agents (load
average 9-11); the transport runs used 2 threads per process, two processes at
a time. Code: this worktree, `sab_moments_550` example, `from_tape_on_calcem_grid`.

### Instruments (committed beside this file, `sab_reference_550/`)

- `sab_reference.py` (run with `python -I`): the direct integral of the
  tape's S(alpha,beta), MF=7 MT=4 at 293.6 K, `LAT = 1`, `LASYM = 0`,
  `LLN = 0`, sb = 81.8125 b, A = 0.9991673, T_eff = 1194.334 K, 222 alpha x
  317 beta (beta_max 395.3, alpha_max 1581). Two interpolation laws: the
  evaluation's stated `INT = 4` (ln S linear in alpha and beta), and THERMR's
  quadratic `terpq` as a control. The short-collision-time law is used outside
  the box as THERMR does; below alpha_min the diffusion (`cliq`) extrapolation
  for beta <= 0.2 as THERMR does (this extrapolation is NJOY's, not stated by
  the ENDF manual: the one convention of the reference that is not the
  evaluation's own). Quadrature: Gauss-Legendre 8 points per E' panel with
  panel edges at every tabulated beta and at E, and 4 mu panels in
  `y = sqrt((1-mu)/2)`.
- **Convergence** (`reference_convergence_both_laws.csv`): three refinements
  (mu nodes x E' sub-panels = 32x1, 64x2, 128x4). 64x2 against 128x4: sigma
  within 1e-6 relative at every energy but 3.5 eV (5e-7 there: 20.684295 vs
  20.684275), `<E'>` within 3e-6, `var(E')` within 1e-6, `<mu>` within 3e-6.
  32x1 against 128x4 worst 5.3e-5 (sigma, 3.5 eV). The numbers below are the
  64x2 level; the residual is below 1e-5 and is not a limit on any
  comparison. (The reference comparison does not itself test the `cliq`
  extrapolation or the SCT branch separately; the SCT share of sigma is 0 in
  the output column `sct_frac`, and the low-alpha share was not split out.)
- `moments_550.csv` from `examples/sab_moments_550.rs`: 4e6 samples per table
  and energy, same seed, batch-means standard errors (20 batches).
  `compare.py` / `compare_output.txt` turn them into the tables below.

### The reference numbers (ENDF `INT = 4` law, 64x2)

| E [eV] | sigma_inel [b] | `<E'>` [eV] | var(E') [eV^2] | `<mu>` | cov(E',mu) | corr(E',mu) |
|---|---|---|---|---|---|---|
| 0.0253 | 51.68071 | 0.0302027 | 3.25842e-4 | 0.178360 | -9.9371e-4 | -0.0981 |
| 0.1 | 32.54008 | 0.0822861 | 1.21695e-3 | 0.343075 | 6.1741e-3 | 0.3373 |
| 0.3 | 25.14931 | 0.200474 | 8.2033e-3 | 0.459772 | 2.6434e-2 | 0.6176 |
| 0.625 | 22.14547 | 0.378181 | 3.5451e-2 | 0.533311 | 5.4127e-2 | 0.6724 |
| 1.0 | 21.48705 | 0.568762 | 8.9873e-2 | 0.572822 | 8.1814e-2 | 0.6897 |
| 1.5 | 21.10931 | 0.821684 | 0.198359 | 0.598712 | 0.117555 | 0.7140 |
| 3.5 | 20.68429 | 1.826365 | 1.04855 | 0.634897 | 0.254163 | 0.7737 |

Between-node energies (0.025, 0.08, 0.15, 0.4, 0.9) and the `calcem` nodes
(0.1035, 1.05, 3.42) are in `reference_endf_law.csv`. The NJOY-law control
(`reference_njoy_law.csv`) differs from the stated law by at most 6.9e-4 in
`<E'>` (3.5 eV; 1.4e-4 elsewhere) and by 1.9e-3 in sigma at 3.5 eV (8e-4 or
less elsewhere): the two laws are within 2e-3 of each other, so "the
evaluation" is defined to that level whichever is read. **Quote the `INT = 4`
column**: it is the law the evaluation states. The NJOY-law column is only
the control; it is the law the NJOY (and so the ACE) tables are made with.

### Both tables against the reference (`compare_output.txt`)

`<E'>` relative to the direct integral (sampling sem 2e-4 to 3e-4 relative):

| E [eV] | nearest calcem node offset | ENDF-built (384 log grid) | NJOY ACE | node grid (new) | 1536x256 log grid |
|---|---|---|---|---|---|
| 0.025 | +1.2 % | **+8.6e-3** | -1.9e-4 | -1.9e-4 | +8.6e-3 |
| 0.0253 (node) | 0 | -1.5e-4 | -1.5e-4 | -1.5e-4 | -1.7e-4 |
| 0.08 | +2.5 % | -7.0e-4 | -1.4e-4 | -1.3e-4 | +1.7e-2 |
| 0.1 | +3.5 % | **-1.12e-2** | -1.3e-5 | -1.3e-5 | +2.6e-2 |
| 0.1035 (node) | 0 | -5.3e-5 | -5.3e-5 | -5.3e-5 | -6.0e-5 |
| 0.15 | -2.8 % | +4.8e-3 | -9.1e-5 | -9.1e-5 | -2.3e-2 |
| 0.3 | +0.4 % | +1.3e-3 | -3.7e-5 | -3.8e-5 | +3.2e-3 |
| 0.4 | -2.5 % | +5.8e-3 | -1.3e-4 | -1.3e-4 | -2.2e-2 |
| 0.625 (node) | 0 | +1.5e-5 | +1.3e-5 | +1.5e-5 | +1.3e-5 |
| 0.9 | -4.4 % | +8.1e-3 | +2.1e-5 | +2.2e-5 | -3.9e-2 |
| 1.0 | -5.0 % | **+1.46e-2** | -2.1e-5 | -2.3e-5 | -4.5e-2 |
| 1.05 (node) | 0 | -1.0e-6 | -1.0e-6 | -1.0e-6 | -2.9e-6 |
| 1.5 | +3.3 % | +6.4e-3 | -2.0e-5 | -2.0e-5 | +3.1e-2 |
| 3.5 | -2.3 % | **-2.26e-2** | -6.4e-4 | -6.4e-4 | -2.3e-2 |

Also against the reference: sd(E') of every table is narrower than the
integral by 1.6e-2 at 0.025 eV falling to 2e-4 at 3.5 eV (the 64-bin
equiprobable set truncates the tails, the one-signed narrowing documented in
`thermal.rs`; the same for ENDF-built, ACE and node grid, so it is **not** a
difference between them). `<mu>` is within 9e-4 absolute for ACE and node grid
at every energy (ENDF-built 2.3e-3 worst), and cov(E',mu) within 1.1e-2 (ACE,
node) and 2.5e-2 (ENDF-built). sigma_inel of the three constructions is
within 8e-4 of the `INT = 4` integral, 1.9e-3 at 3.5 eV where the NJOY-law
integral is the match.

- **At nodes** (0.0253, 0.1035, 0.625, 1.05 eV) the ENDF-built and ACE tables
  agree with each other to 5e-6 and with the reference to 1.5e-4 at worst.
- **Between nodes** the ACE table agrees with the reference to 1.9e-4 (6.4e-4
  at 3.5 eV, which is the NJOY-law versus `INT = 4` gap, 6.9e-4); the
  ENDF-built table is off by up to 2.3 %, of either sign, by more than 4e-3 at
  8 of the 10 between-node energies and by 1.1 % or more at 3.
- **The new node-grid table is the ACE table**: its sampled moments with the
  same seed equal the ACE's to 3e-7 (`<E'>`), 3e-6 (var), 7e-6 (`<mu>`) at
  the between-node energies; the residual is the ACE's 7-figure text and its
  16 cosines.
- **Cosine count (8 against 16 per bin)** at six `calcem` nodes: `<E'>` and
  var(E') change by at most 3e-6 relative, `<mu>` by 4e-6, cov by 1e-5 (last
  block of `compare_output.txt`). It is not the cause.
- The 1536x256 table is not better: it is the same relabelled-node
  construction on more labels. It is off by 4.5 % at 1 eV, which is why arm D
  of `pitch_sweep.md` did not move k.

### Where the difference lives

1. **Not in the THERMR port.** `calcem`'s node records are NJOY2016's word for
   word (`tests/acer_thermal_word_parity.rs`), and a table made from those
   records on their own energies reproduces the ACE (above). Nothing in
   `njoy-outram-park-fork` is edited.
2. **In outram-mc's use of `IncoherentInelasticScattering::emission`.**
   `emission(e, ..)` returns the record nearest `e` by design (its doc says
   so). `ThermalScattering::from_tape_with_grids_and_options` calls it at a
   log-spaced `E` and stores the result under the label `E`. The EGRID nodes
   are 4 to 10 % apart, so a table labelled `E` carries the outgoing energies
   of an incident energy up to 5 % away; at 0.025 eV every label near 0.025
   maps to the 0.0253 node and the sampled moments are those of the node
   table to every printed digit (3.01981704e-2 both). The result is a
   staircase in incident energy where OpenMC's sampler (`sample_discrete`,
   ported) wants a table at each of its own points.
3. **Not the cosine count, not interpolation resolution, not the cross
   section, not the mean cosine.**

### The fix and its test

`ThermalScattering::from_tape_on_calcem_grid` (and `_and_options`): one
emission table per `calcem` record, at its own energy, sharing one body with
the old constructor (`from_tape_with_emission_grid`, private, with an
`EmissionGridKind` enum). Reached by
`tests/thermal_emission_on_thermr_grid_550.rs` (node-grid `<E'>` against the
direct integral at 0.1 and 1.0 eV within 5e-4: measured -2.4e-5 and -4.0e-5;
against an independent node interpolation: 0.082284 vs 0.082290; the
log-spaced table still +1.46e-2 off at 1.0 eV, pinned as the contrast; sigma
bit-identical). `lct008_pitch_sweep` gained `--sab-node-grid`.
~~**`from_tape` still builds the log-spaced table**~~ **CORRECTED 2026-11-19: flipped, see "Flip and re-measure" at the end.** (Text as written 2026-11-18:) flipping the default
moves every ENDF-route thermal number in the workspace (HST-009, LCT-008, the
pebble beds with H2O, the shipped processed laws) and each needs
re-measuring first. That is recorded here as the recommended next step, not
done. `thermal.rs` carries the caveat in the doc comment of
`from_tape_with_grids`. Checks run after the change: the three thermal test
files `thermal_endf_route_xs_vs_njoy`, `thermal_cutoff_is_tape_emax`,
`light_water_thermal_scattering` pass (7 tests), `cargo check` of
`outram-mc-libs` on `x86_64-unknown-none` without `std`, `petir`'s
`no_platform_libm` (4 tests), and `cargo check --lib --tests --examples` of
`outram-mc-libs` and `njoy-outram-park-fork`. The full test suite was not run.

### Eigenvalue arm E (OpenMC `.h5` neutron data, ENDF-built S(a,b) on THERMR's grid)

5000 x [50 + 200], case-1 borated water, 11-nuclide tier, 293.6 K, seeds 1-8
(`sab_reference_550/arms/`, driver `arms/run_arm_E.sh`; 1.75 cm has 16 seeds,
1-8 and 9-16, `arms/run_1p75_followup.sh`). `analyse_arms.py` /
`analyse_arms_output.txt`. Unpaired (Welch).

| pitch [cm] | arm | n | k |
|---|---|---|---|
| 2.40 | **E** node grid | 8 | 0.70651 +- 0.00039 |
| 2.40 | A (NJOY ACE S(a,b)) | 8 | 0.70694 +- 0.00023 |
| 2.40 | B (384 log grid) | 8 | 0.70559 +- 0.00027 |
| 2.40 | OpenMC | 32 | 0.70652 +- 0.00020 |
| 1.63576 | E | 8 | 1.06257 +- 0.00045 |
| 1.63576 | ENDF-route baseline | 16 | 1.06216 +- 0.00031 |
| 1.63576 | OpenMC | 16 | 1.06253 +- 0.00022 |
| 1.75 | E | 16 | 1.01149 +- 0.00032 |
| 1.75 | B | 8 | 1.01181 +- 0.00040 |
| 1.75 | ENDF-route baseline | 16 | 1.01104 +- 0.00016 |
| 1.75 | OpenMC | 16 | 1.01104 +- 0.00033 |

- **2.40 cm: E - B = +92 +- 47 pcm (z +1.96)**; E against the ENDF-route
  baseline +115 +- 47 (z +2.44); **E - OpenMC (32 seeds) = -1 +- 44 pcm
  (z -0.02)**, the gap closed; E - A = -43 +- 45 (z -0.94), consistent with
  the ACE table.
- **1.63576 cm**: E - baseline +42 +- 55 (z +0.75); E - OpenMC +4 +- 50.
- **1.75 cm**: E - B (same neutron data, 8 seeds of B) -32 +- 51 pcm
  (z -0.63); E - baseline +45 +- 36 (z +1.25); E - OpenMC +44 +- 46
  (z +0.96). The first 8 seeds had read +104 +- 49 (z +2.09) against the
  baseline; 16 seeds and the same-data control B say that was a fluctuation.
  I added those 8 seeds and the B control after seeing +104, so the 1.75 cm
  verdict rests on a look taken because of the first result; all numbers are
  reported.

### Predictions R1-R10 against results

| id | verdict | measured |
|---|---|---|
| R1 | **held** | sigma vs the port's tables +1.3e-4 .. +7.9e-4 (1.9e-3 at 3.5 eV, the NJOY-law difference); node `<E'>`: port 0.0302050 vs reference 0.0302027 (+7.5e-5), 0.625 eV +5.6e-6 |
| R2 | **held** | node-energy `<E'>` of both tables within 1.5e-4 of the reference, 5e-6 of each other |
| R3 | **held, sharper than predicted** | ACE within 1.9e-4 (6.4e-4 at 3.5 eV); ENDF-built up to 2.26e-2 (predicted 2-4 %) |
| R4 | **partly tested** | mechanism confirmed at 0.025 eV (ENDF-built moments equal the 0.0253 node table's to every printed digit); the quantitative form (ratio to the reference at the labelled node) was not computed because the log-grid labels are not the probe energies |
| R5 | **held** | 8 vs 16 cosines: `<E'>`, var 3e-6; `<mu>` 4e-6; cov 1e-5 |
| R6 | **held** | `<E'>` laws within 6.9e-4 (limit 2e-3) |
| R7 | **held** | node table = ACE to 3e-7 in `<E'>` |
| R8 | **held** | zero `kind = "review"` blocks in `outram-mc-libs/src/material/review.md`, `njoy-outram-park-fork/src/thermr/review.md` and `thermr/calcem/review.md`; `from_tape_with_grids_and_options` is `fn:fc92d8e9d1d2226c` in `material/kovan.toml` with no review record |
| R9 | **held** | +92 +- 47 pcm (E - B), inside +60 (-20 .. +150); closes the gap. My expectation that a zero-mean relabelling is unbiased to first order argued for less; the measured shift says it matters. How the relabelling moves k is not isolated here |
| R10 | **held after follow-up** | 1.63576 +42 +- 55; 1.75 -32 +- 51 against same-data B (see above) |

### Which table is closer to the ENDF/B-VIII.0 evaluation

NJOY's `HinH2O.ace` (and the new node-grid table, which is the same law) is
closer: within 1.9e-4 in `<E'>` everywhere that the two readings of the
evaluation agree (6.4e-4 at 3.5 eV), against up to 2.3 % for the tape-built
table with the log-spaced emission grid. At the four energies that are nodes
of both grids the two tables are the same and both are within 1.5e-4. The node
grid and the ACE are both narrower than the evaluation in sd(E') by up to
1.6 % at thermal energies (64 equiprobable bins); that is a property of the
representation, not of either table.

### Not shown

- Why the zero-mean relabelling noise moves k by about +100 pcm at the most
  over-moderated pitch. The measurement establishes that it does, and that
  fixing the labels reproduces the ACE arm (E - A -43 +- 45).
- Whether the same relabelling affects other consumers of `emission` (the
  tape-built tables of graphite and the pebble-bed demos build through the
  same constructor). Not measured.
- Pitches other than 1.63576, 1.75 and 2.40 cm; k of HST-009, the full
  LCT-008 lattice, or any pin that uses the tape-built table. All are built
  by the same construction and none was re-measured.

## Flip and re-measure (2026-11-19, maintainer decision "Flip and re-measure")

Plan and signed predictions, committed before any run: `sab_reference_550_flip_plan.md`.
Raw outputs: `flip_550/`. Research, education and V&V only.

**What changed.** `ThermalScattering::from_tape` and `from_tape_with_options`
(and so `from_endf_file*`, `from_leapr`, `from_embedded_leapr_deck`) now call
the node-grid body, `from_tape_on_calcem_grid[_and_options]`. The old build is
the explicit ablation `from_tape_log_spaced_grid` (and `from_tape_with_grids`).
The flip was two lines, not one (`from_tape` did not route through
`from_tape_with_options`). No kovan-reviewed function was touched
(`src/material/review.md` holds no review block). Pinned by
`tests/correct_physics_is_default.rs::endf_thermal_emission_is_on_the_calcem_grid_by_default`
(ordinary paths bit-identical to the node-grid build, different from the ablation).

Hardware for every run below: Intel Xeon @ 2.10 GHz, 4 logical cores, 15 GB RAM,
Linux 6.18.44, CPU only; shared VM, two heavy processes of mine at a time,
2 threads each, load average above 4 throughout, so timings are indicative only.
Code: the commit that flips the default, `--release`.

### k-effective numbers (outram-mc, ENDF tapes through RECONR/BROADR, new default)

Old values are the records named; "move" is new minus old, sigma is the
quadrature of the two sems (unpaired), z = move / sigma.

| id | case | old | new | move | z | predicted |
|---|---|---|---|---|---|---|
| N1 | LCT-008 pin cell 2.40 cm, 5000 x [50+200], 11 nuclides | 0.70536 +- 0.00027 (16 seeds) | 0.70568 +- 0.00024 (8 seeds, sd 68 pcm) | **+32 +- 36 pcm** | +0.88 | +90 (+10 .. +170): **missed low**, inside the range |
| N1' | same, outram-mc minus OpenMC (0.70653 +- 0.00029, 16 seeds) | -117 +- 39 pcm (z -2.97) | **-85 +- 38 pcm** | +32 | z -2.26 | gap to shrink to about -30: **not met**; the |z| <= 2 gate still misses | **CORRECTED 2026-11-19 (`gap_240_550.md`): 24 seeds give 0.70642 +- 0.00020, -10 +- 28 pcm (z -0.37), the gate passes; the 8-seed -85 was a fluctuation** |
| N2 | same, 1.63576 cm | 1.06216 +- 0.00031 (16) | 1.06323 +- 0.00040 (8, sd 112) | **+107 +- 50 pcm** | +2.14 | +40 (-40 .. +120): inside the range (upper end +120); sign right, size 2.7 x the central value |
| N3 | same, 1.75 cm | 1.01104 +- 0.00016 (16) | 1.01168 +- 0.00033 (8, sd 93) | **+64 +- 37 pcm** | +1.76 | +10 (-60 .. +80): inside the range, larger than central |
| N4 | HST-009 case 1, 5000 x [40+120], 32 seeds | +80 +- 13 pcm (128 seeds, 2026-11-12); +80 +- 22 (32 seeds, 2026-11-14) | **+119 +- 23 pcm** (sd 131; k 1.00119) | **+39 +- 26** (vs 128) / +39 +- 32 (vs 32) | +1.5 / +1.2 | +40 (-30 .. +120): **held** |
| N5 | LCT-008 full lattice case 1, 4000 x [120+250], 35 nuclides, 32 seeds | +172 +- 22 pcm (32 seeds, 2026-11-12) | **+148 +- 21 pcm** (sd 117) | **-24 +- 30** | -0.8 | +40 (-30 .. +110): **sign wrong**, inside the range |

Summary of the pin-cell result against what the diagnosis above suggested:
the three pitch moves are all positive (+32, +107, +64 pcm), the same sign as
arm E, and each is below 2.2 sigma on its own. At 8 seeds I cannot call any one
of them resolved; the three together average +68 pcm with sigma about 26 pcm
(treating the three as independent, 2.6 sigma), a combination chosen after
seeing the data. **The 2.40 cm gap to OpenMC is not closed on the default
(tape-data) route**: -85 +- 38 pcm (z -2.26). ~~Not closed~~ **CORRECTED 2026-11-19 (`gap_240_550.md`):** with seeds 9-24 added the default route is -10 +- 28 pcm (z -0.37) and E minus default is -15 +- 29; ~~the two routes differ in neutron data (RECONR/BROADR against NJOY ACE)~~ their cross sections, URR tables, 0 K elastic, nu-bar, fission spectrum and kT agree to 1e-6 or better, so there is no neutron-data difference. (Original text follows.) Arm E closed it (-1 +- 44) with
OpenMC's `.h5` neutron data; the two routes differ in neutron data (RECONR/BROADR
against NJOY ACE), so the E result and this one are not the same experiment,
and the flip's +32 +- 36 on tape data is smaller than E minus B (+92 +- 47)
by 60 +- 59 pcm (1.0 sigma). I do not claim the S(a,b) staircase explains the
2.40 cm miss on the default route. The seed set 1-8 is a subset of the old
16-seed ensemble's seeds, but the code and random streams differ, so the
comparison is unpaired.

### Tests and kernel gates

| id | item | old | new | verdict |
|---|---|---|---|---|
| N7 | `thermal_kernel_stationary_distribution`, c_H_in_H2O fixed point | 292.96 K, shape 1.6660 | **294.31 K**, shape 1.6557 (hot-cold +0.73 K, 2.22 sigma) | **re-pinned**; water shift +1.35 K, predicted under 1 K: **missed**. Moves from -0.22 % to +0.24 % of nominal and from -0.04 % to -0.66 % of 5/3: **further from a Maxwellian** (unflattering) |
| N7 | same, c_Graphite | 604.17 K, shape 1.6456 | **606.29 K**, shape 1.6448 (hot-cold +0.95 K, 0.76 sigma) | **re-pinned** (+2.12 K, outside the 2.0 K gate); from +0.70 % to +1.05 % above nominal: **further from nominal** (unflattering). Tolerances unchanged |
| N6 | graphite golden gate `<E'>/E` vs NJOY THERMR, 10 energies | worst +0.301 % at 0.01 eV | identical to 5 digits except 0.11157 eV (1.01175 -> 1.01173) | unchanged: THERMR's probe energies are `calcem` nodes, where the two builds agree; the staircase lives between nodes and these gates cannot see it |
| N6 | H-in-H2O golden gate, 11 energies | worst +0.098 % at 1.5 meV | identical | unchanged, same reason. The example's last assertion (a 5-sigma check that the OLD #188 defect is still visible, 0.10 % vs 0.15 % sigma) panics with the old build and the new one alike: a stale gate from before #407, not caused by this flip, and **not edited** (tightening it is a maintainer decision) |
| N8 | `thermal_standards`, `thermal_laws_vs_njoy_thermr` (8), `thermal_graphite_elastic` (8), `thermal_lthr3_mixed_634`, `thermal_endf_route_xs_vs_njoy`, `thermal_cutoff_is_tape_emax`, `light_water_thermal_scattering` (5), `triso_shell_thermal_scattering`, `htr10_graphite_thermal_scattering_pebble_bed`, `elastic_xi_h1_o16` | pass | pass, no re-pin | **held** |
| N9 | `material_sab_462_transport`, `processed_evaluation_round_trip`, `total_fast_path_matches_full`, `speed_tier_fast_is_exact`, `majorant_bounds_endf`, `temperature_precedence`, `openmc_xs_details_463`, `thermal_h2o_leapr_vs_tsl` | pass | pass | **held** |

### Not re-measured

Every number below was taken with the log-spaced build and **has not been
re-measured**; read each as pre-flip. Graphite is the case most worth a look,
because the stationary-distribution pins moved most there in relative terms.

- Pebble-bed and graphite k: HTR-10 pebble beds and `nee_soon::htr10_rmc`,
  FHR ring RPT (`fhr_ring_rpt_endf`), FHR pebble ablation, MSRE N-1 and NC-1.
- `sab_per_material_1581` ensembles, `icsbep_five_route_keff` (five routes;
  routes 4 and 5 take the LEAPR deck, which also flips), `pincell_depletion_vs_openmc`,
  `dh_keff_vv`, the `openmc_notebooks/pincell` test (not run), `dhoby-ghaut`'s processed-law cache.
- LCT-008 pin cell: the unborated points and the 36-nuclide tier; pitches other
  than the three above; and 16 seeds at each of the three (8 were run).
- The graphite/H2O kernel at between-node energies against THERMR: the
  committed golden tables have no between-node probe, so no THERMR comparison of
  the flip exists; the evidence for it is the direct integral above.
- Timings of the node-grid build against the log-spaced one.

**CORRECTED 2026-11-19 (gh:#550, "Re-measure the rest"):** most of the list
above is measured in "Second re-measure" below, which also says which items
were not reached. The list stands as the state of `33af080b9`.

## Second re-measure (2026-11-19, gh:#550, maintainer decision "Re-measure the rest")

Plan and signed predictions, committed before any run:
`sab_reference_550_remeasure2_plan.md` (two addenda, the second written
after block 1 of G1 and before block 2's output was read). Raw outputs and the
comparison script: `remeasure2_550/` (`python3 -I analyse.py NEW OLD`).
Research, education and V&V only. The LCT-008 cases were not run (another
agent owns the 2.40 cm pin-cell gap).

**The old-build control.** `from_tape` and `from_tape_with_options` now honour
`OUTRAM_SAB_LOG_SPACED_GRID=1` (named ablation; unset, empty or `0` = default).
It reaches every consumer, so an unchanged example is its own paired control.
**It is a true old-build control:** `htr10_fuel_zone_kinf` with the knob on
reproduces the 2026-11-17 record's means and sds exactly (het 1.57302, sd 279
pcm; hom 1.44751, sd 268 pcm, 12 seeds), and `fhr_pebble_target_at_rest_ablation`
(MOVING) with the knob on reproduces the record's seeds 1 and 2 to every
printed digit (1.36355 +- 0.00184, 1.36228 +- 0.00172). I stopped the FHR
ablation after those two seeds and use the record's per-seed values as the
control for seeds 1-8 (not verified for seeds 3-8).

Hardware for every run below: Intel Xeon @ 2.10 GHz (cloud container, KVM),
4 logical cores, 15 GB RAM, Linux 6.18.44, CPU only; shared with another
agent (load average 1.6 to 10 over the session), two to three processes of mine
at once, 1-2 threads each. Timings are indicative only and not tabulated.
Code: `4c29b7f58` plus the knob (this change), `--release`.

### Graphite: k moves, with their predictions

| id | case | old | new | new - old +- sigma | z | predicted |
|---|---|---|---|---|---|---|
| G1 | graphite medium k_inf, `thermal_kernel_keff_worth`, 600 K, 4000 x [20+80], 8 replicas paired by master seed, node grid minus log-spaced 384x64, block 1 (seeds 20260912-19) | 1.38590 +- 0.00068 | 1.38356 +- 0.00102 | **-234 +- 108 pcm** | -2.16 | -15 (-90 .. +60): **missed**, same sign, outside the range |
| G1 | same, block 2 (seeds 20261912-19, replication announced before reading) | 1.38403 +- 0.00039 | 1.38391 +- 0.00050 | **-12 +- 68 pcm** | -0.18 | -15 (-230 .. +200) if no effect, -230 (-450 .. -10) if real: **consistent with no effect, 1.7 sigma from block 1** (222 +- 128) |
| G1 | pooled N = 16 (pre-announced pool) | | | **-123 +- 68 pcm** | -1.80 | -15: 1.6 sigma from it; **not resolved**; negative in both blocks |
| G2 | `htr10_fuel_zone_kinf` heterogeneous, 2000 x [20+60], 12 seeds, paired with the knob | 1.57302 (sd 279) | 1.57253 (sd 314) | **-49 +- 121 pcm** (paired -49 +- 119) | -0.41 | -10 (-80 .. +60): held |
| G2 | homogenised | 1.44751 (sd 268) | 1.44731 (sd 287) | -20 +- 113 (paired -20 +- 133) | -0.17 | |
| G2 | double-heterogeneity worth (hom - het) | -12552 | -12522 | +30 +- 175 | +0.17 | unchanged within 150: held |
| G3 | `htr10_pebble_delta_tracking` natural B (atom ppm), 300 x [20+60], 12 seeds against the record's 24 | 1.68681 (sd 589, N 24) | 1.68456 (sd 573) | -225 +- 205 pcm | -1.10 | -15; resolution 250: unresolved |
| G3 | weight-ppm arm | 1.68667 | 1.68511 (sd 1021) | -156 +- 317 | -0.49 | |
| G3 | no boron | 1.70078 | 1.69998 | -80 +- 169 | -0.48 | |
| G3 | graphite boron only | 1.68796 | 1.69074 | +278 +- 222 | +1.25 | |
| G3 | ppm as elemental B-10 | 1.64516 | 1.64357 | -159 +- 207 | -0.77 | |
| G3 | paired control, seeds 1-3 with the knob, five arms | | | per-arm mean differences +112, -447, -189, +674, -280 pcm (single-seed differences swing +-1000) | | noise: **nothing is resolved**; the five-arm mean of the unpaired differences above is -68 pcm |
| G4 | FHR ring-RPT pebble, MOVING (the `fhr_ring_rpt_endf` CSG solve; per-seed identical to it in the record), 4000 x [30+80], 600 K, 8 seeds | 1.36307 (sd 117) | 1.36384 (sd 214) | **+77 +- 86 pcm** (paired +77 +- 74) | +0.89 | -10 (-90 .. +70): inside the range, sign wrong |
| G4 | AT-REST | 1.36154 (sd 141) | 1.36123 (sd 206) | -31 +- 88 | -0.35 | |
| G4 | worth AT-REST - MOVING | -153 +- 43 | **-261 +- 107** | -108 +- 115 | -0.94 | unchanged within 100: held. The driver's own gate against -153 prints HELD, 1.0 sigma |

Reading G1 to G4 together. The graphite medium (G1) is the one graphite
instrument built to resolve a kernel effect, and its two blocks disagree by
1.7 sigma (-234, -12); the pooled -123 +- 68 pcm is below 2 sigma and I do not
call it a measurement. Its sign is negative in both blocks, as the hotter
graphite stationary-distribution neutron temperature (+2.1 K) predicted, but
the size block 1 suggested is not supported by block 2. The pebble cases (G2
to G4) carry seed sds of 120 to 1000 pcm and resolve nothing under 100 to 300
pcm: G2 and G3 are consistent with zero and with the pooled G1 value; G4's
MOVING arm moved up, not down, by +77 +- 86 pcm (0.9 sigma). The FHR
seed-to-seed sd is 214 pcm against 117 in the record (variance ratio 3.3 on 7
and 7 degrees of freedom): a fluctuation I cannot exclude, and it is what
makes -261 +- 107 against -153 +- 43 unresolved. No threshold or pin was moved
for these.

### Graphite pebble with the double-heterogeneity treatments: `dh_keff_vv`

| id | case | old (knob on) | new | new - old +- sigma | z | predicted |
|---|---|---|---|---|---|---|
| G5 | delta tracking (explicit TRISO), 7200 x [15+40], 600 K, 8 draws, single thread | 1.38363 (sd 122 pcm) | 1.38307 (sd 128) | **-56 +- 62 pcm** (paired by draw -56 +- 29, z -1.93) | -0.89 | -10 (sigma 210 per single draw): inside, sign right |
| G5 | ring-RPT (OpenMC-fitted r_inner 1.4934 cm) | 1.38145 (sd 145) | 1.38337 (sd 183) | **+192 +- 82 pcm** (paired +192 +- 105, z +1.83) | **+2.33** | -10: **missed, wrong sign, 2.3 sigma** |

Draw 1 of the delta arm is the same run as the two-draw probe made first
(1.38385 both times: the program is deterministic for a fixed seed). The ring
arm's +192 pcm is the largest unflattering move of this pass. It is a single
comparison among about twenty made today, so a 2.3 sigma value is not unusual
under a zero true difference, and the delta arm on the identical pebble moves
the other way (-56), but I cannot call it noise either; the ring-RPT pebble
is the same model as G4's, whose MOVING arm also moved up (+77 +- 86). The
two graphite-pebble ring-RPT numbers together (+192 +- 82 and +77 +- 86,
independent runs) average +137 +- 59 pcm (2.3 sigma, a combination chosen after
seeing both). The graphite medium of G1 moved the other way. I cannot
reconcile these with one sign for graphite, and I do not claim to. The
reference OpenMC value for this pebble is k = 1.36510 +- 0.00063
(explicit TRISO); the delta and ring arms sit about +1800 pcm above it both
before and after the flip, a gap of another origin and not moved by it.

### Water systems and the LEAPR route

| id | case | old | new | new - old +- sigma | z | predicted |
|---|---|---|---|---|---|---|
| W2 | `icsbep_five_route_keff --case hst009 --route endf` (route 4, H(H2O) from the LEAPR deck, which also flips), 5000 x [40+120], 16 seeds | 1.00080 (sd 152, N 16, `501c16264`) | **1.00152** (sd 113) | **+73 +- 47 pcm** (paired by seed +73 +- 49) | +1.53 | +40 (-30 .. +120): **held** |
| W2 | control: the knob on, seed 1 | record seed 1 1.00128 | 1.00128 +- 0.00143 | bit-identical | | the old build is reproduced; seeds 2 and 3 with the knob: 0.99926, 0.99853; new minus knob over seeds 1-3: -99, +335, +258, mean +165 +- 134 pcm |
| W3 | `icsbep_five_route_keff --case msre --route endf` (graphite unit cell with salt, route 4, carbon S(a,b) from the LEAPR deck), 5000 x [40+120], 4 seeds against the record's 8 | 1.65536 (sd 118, N 8, `501c16264`) | 1.65511 (sd 56, N 4) | **-25 +- 50 pcm** | -0.50 | -10: held |
| W3 | same, seeds 1-4 against the record's seeds 1-4 (1.65585, sd 94) | | | -74 +- 55 unpaired (z -1.35); the same-seed "paired" figure, -74 +- 24 (z -3.1), is NOT used: the code has moved since `501c16264` (no knob-on control was run for this case) and the two builds' random streams do not correlate | | |

Both water numbers agree in sign and size with the first pass (HST-009
+39 +- 26; the pin cells +32, +107, +64): the node grid raises k in
H-in-H2O systems by tens of pcm, +73 +- 47 here.

### MSRE N-1 (graphite at 911 K, salt, H2O in the shield)

| id | case | old | new | new - old +- sigma | z | predicted |
|---|---|---|---|---|---|---|
| W1 | `msre_n1_first_criticality --mo natural --seeds 1-3 --threads 2`, 20000 x [100+200], default model (natural Mo, hot dimensions, whole TM-0728 shield, U-234/U-236); **2 of 3 seeds finished** (2852 s and 3340 s a seed on 2 shared threads) | 1.01807 +- 0.00023 (6 seeds, 2026-11-13) | seeds 1, 2: 1.016934 +- 0.000488, 1.015550 +- 0.000573; mean 1.016242 (sd 98 pcm, sem 69) | **-183 +- 73 pcm** against the 6-seed record (z -2.5) | -2.5 | -10 (-80 .. +60): **missed**, 2.5 sigma, and **not attributable to the flip** |

**Why W1 is not a measurement of the flip.** No knob-on control was run (one
seed costs about 50 min here, and the budget ended). The 2026-11-13 record is
six days and many commits old: the msre_n1 README itself records the default
re-run on a later binary (`42b372a59`, 2026-11-19, before the flip) at seed 1 =
1.019902 +- 0.000544, **+237 pcm** from the 11-13 binary's seed 1 (1.017528),
for a reason the README does not isolate. This run's seed 1 (1.016934) is -59
pcm from the 11-13 binary's and **-297 +- 73 pcm** from the later pre-flip
binary's; the code differs from both by more than the flip. The honest
statement: N-1 read 1.0162 +- 0.0007 (2 seeds) on today's binary, 183 pcm
below the 11-13 six-seed record and 297 pcm below the same-day pre-flip seed
1; a paired knob-on pair at the same seeds (about two hours) is what would
say how much of that is the flip. A lower k moves N-1 toward k_ref (1.00080),
a flattering direction, which is a reason for suspicion and not for acceptance;
and if it were all the flip it would be the largest move in this pass. The
data are in `remeasure2_550/msre_new.csv`. The README's headline
(+1726 +- 23 pcm) is not changed by this.

### Tests, the cache and items that do not move

| id | item | verdict |
|---|---|---|
| T1 | `openmc_notebooks` pincell tests (6, includes `pincell_lwr_thermal_pin_benchmark`, which reads `tsl-HinH2O.endf` through `from_endf_file`) | pass, no re-pin |
| T1 | `correct_physics_is_default` (8, includes the flip's pin), `no_unmarked_nuclide_level_sab_462` (1) | pass |
| T2 | `dhoby-ghaut` `mc_web::processed_cache` tests (6) | pass. **A defect found and fixed in the same change:** `processed_cache::CODE` (the version every cached product carries; its doc says to bump it when THERMR or LEAPR changes a product) was not bumped by the flip, so a browser would have kept serving pre-flip thermal laws as current. Bumped to `nd-2026-11-19.1`. No cache is committed (IndexedDB state per browser), so nothing was regenerated. The existing hash pin covers only neutron products (He-4, U-234, which hold no thermal data: unchanged), which is how the flip passed unnoticed; a second pin, `the_code_version_pins_the_thermal_law`, now fixes the SHA-256 of the Si-in-SiC law (MAT 43, 300.15 K, `from_tape`) as `ef7c770d...d6d1a` and asserts that the pre-flip build gives a different one (`556208e0...ac8439`) |
| P0 | `pincell_depletion_vs_openmc` | **does not move, by construction**: `ThermalScattering::from_ace` and `Nuclide::from_ace_file` (lines 125-126), no ENDF thermal build. Not run |
| P0 | `thermal_kernel_keff_worth`'s four pre-existing arms | do not move: they call `from_tape_with_grids` explicitly (the log-spaced constructor), unchanged by the flip; only the new fifth arm is the default |
| P0 | MSRE "NC-1" | no case of that name exists in the repository (searched `.rs`, `.md`, `.toml`, `.sh`); the MSRE cases are N-1 and N-3. N-1 was run (below) |

### Summary of this pass (new minus old, pcm)

Positive = the node grid raised k. Predictions are from the plan, fixed before
any run. About twenty comparisons were made, so one or two at 2 sigma are
expected under zero true difference.

| case | move | z | predicted | outcome |
|---|---|---|---|---|
| graphite medium k_inf (G1, N=16 pooled) | -123 +- 68 | -1.8 | -15 | block 1 -234 (missed), block 2 -12; pooled unresolved |
| HTR-10 fuel zone het (G2) | -49 +- 121 | -0.4 | -10 | held |
| HTR-10 pebble delta, natural B (G3) | -225 +- 205 | -1.1 | -15 | unresolved |
| FHR ring-RPT MOVING (G4) | +77 +- 86 | +0.9 | -10 | sign wrong, unresolved |
| FHR worth AT-REST - MOVING (G4) | -108 +- 115 (-261 vs -153) | -0.9 | 0 +- 100 | held |
| `dh_keff_vv` delta (G5) | -56 +- 62 | -0.9 | -10 | held |
| `dh_keff_vv` ring-RPT (G5) | +192 +- 82 | +2.3 | -10 | **missed, wrong sign** |
| HST-009 route 4 (W2) | +73 +- 47 | +1.5 | +40 | held |
| MSRE unit cell route 4 (W3) | -25 +- 50 | -0.5 | -10 | held |
| MSRE N-1 (W1) | ~~-183 +- 73 (uncontrolled)~~ | -2.5 | -10 | ~~**missed; not attributable to the flip**~~ **CORRECTED 2026-11-19 (third re-measure, below): the paired knob-on control gives the flip's own move, -297 +- 73 pcm (z -4.1, one seed pair), with +237 from other code; the earlier statement that it was not attributable to the flip was wrong** |
| processed-law cache version | `CODE` not bumped by the flip | | | defect fixed |

What the pass supports: no graphite or water system moved by an amount the
available statistics resolve, except two 2.3 to 2.5 sigma values (the ring-RPT
pebble up, MSRE N-1 down) and G1's block 1, which a replication did not
confirm. The graphite signs are mixed (G1 and G5-delta down, G4 and G5-ring
up), which fits an effect smaller than the statistics and does not fit a
single physical story. The water systems keep rising by tens of pcm
(+32, +107, +64, +39 in the first pass; +73 here). Nothing was tuned, no
tolerance moved, no pin re-set for k; the only pin added is the thermal-law
hash of the processed-law cache.

### Not re-measured, and why

- `nee_soon::htr10_rmc` (HTR-10 whole core) at the record's protocol:
  10000 x [5+135] took 2945 s of transport on 16 threads on 18 cores, about
  33 ms per history per thread, so about 6.5 h on the 2 threads available, and
  a data load of 420 s on 16 threads. A reduced pair would have a sigma near 500
  pcm and resolve nothing (the original pair already could only bound
  |LEAPR - TAPE| below 380 pcm). Reached: yes (`ThermalScattering::from_leapr`
  -> `from_tape`, `data.rs`, `data_jobs.rs`). The graphite instruments that can
  resolve something are G1 to G5 above, and they are the evidence for graphite.
  The `htr10_correct_physics_is_default` test of `nee_soon` was not run.
- ~~`icsbep_five_route_keff` routes 3 and 5~~ **MEASURED 2026-11-19, see
  "Third re-measure": bit-identical, as they cannot reach the ENDF thermal
  build.** The HST-009 / MSRE cases at the
  2026-11-17 record's 16 and 8 seeds on the knob-on side (only 3 knob-on seeds
  of HST-009 were run, as a control) remain as stated.
- ~~`fhr_ring_rpt_endf`'s other six solves (explicit TRISO, ring-RPT and naive
  homogenised, cube and sphere, delta-tracked): the driver has no seed loop
  (see `sab_per_material_1581`); its CSG solve is G4. G5 is the nearest
  delta-tracked ring-RPT pebble.~~ **CORRECTED 2026-11-19:** the driver has no
  seed LOOP, but it has had the per-process knob `OUTRAM_FHR_ALL_SEED=S`
  (gh:#1635 item 1, `sab_per_material_1581`) since 2026-11-18, which sets the
  master seed of all seven solves; the third re-measure ran the seven solves
  as a knob pair per seed, see "Third re-measure".
- The 24-seed G3 protocol (12 were run), and the FHR AT-REST arm with the knob
  on (the control for it is the 2026-11-17 record, checked only through the
  MOVING arm's seeds 1 and 2).
- `dh_keff_vv`'s older single-draw doc record (1.38647 +- 0.00211, another
  machine and commit) is not a comparison here; the paired knob-on arm is.
- Timings of the node-grid build against the log-spaced one.
- ~~LCT-008 cases, deliberately.~~ **The LCT-008 pin cell at 1.63576 and 1.75
  cm was extended to 24 seeds in the third re-measure** (2.40 cm is
  `gap_240_550.md`); the 36-nuclide tier at 2.40 cm and the 35-nuclide lattice
  were not re-measured.

## Third re-measure (2026-11-19, gh:#550, "finish the rest")

Plan and signed predictions, committed before any run:
`sab_reference_550_remeasure3_plan.md` (commit `c025f2007`). Raw outputs, run
scripts and the analysis script: `remeasure3_550/` (`python3 -I
analyse_pitches.py`). Research, education and V&V only. No library function was
changed and no example was changed (the FHR seed knob `OUTRAM_FHR_ALL_SEED`
already existed, gh:#1635 item 1); no `review.md` was touched; no tolerance,
threshold or pin was moved; nothing is re-pinned.

Hardware for every run: Intel Xeon @ 2.10 GHz (cloud container, KVM), 4 logical
cores, 15 GB RAM, Linux 6.18.44, CPU only; a VM shared with other agents (load
average 3 to 12 over the session); 2 threads per transport process (1 for the
five-route runs), two heavy processes at a time plus niced side jobs. Timings
are indicative only. Code: `c025f2007` (the plan commit; the library is that of
`6b3b64807`), `--release`.

### Item 1: LCT-008 pin cell at 1.63576 and 1.75 cm, default route, 24 seeds

Protocol as `gap_240_550.md` step 1: case-1 borated water, 11 nuclides,
5000 x [50 + 200], 293.6 K, tape data, node-grid S(a,b). Seeds 1-8 are the
recorded `flip_550/pitch_flip.csv`; seeds 9-24 are new (`remeasure3_550/def_s9.csv`,
`def_s17.csv`, one process each, both pitches per seed). **Reproducibility
check, run because the 1.63576 block of seeds 9-24 disagrees with seeds 1-8:**
seed 1 at both pitches re-run on this binary gives 1.064450 and 1.010059,
identical to the recorded values to every printed digit
(`remeasure3_550/repro_s1.csv`), so the recorded 8-seed blocks are what the code
gives, as at 2.40 cm.

**OpenMC reference.** The plan named the existing 16 OpenMC seeds and allowed
more "only if time allows, announced". I added OpenMC seeds 17-24 at both
pitches (OpenMC 0.15.3, same deck, same NJOY `.h5` library, same settings,
`remeasure3_550/openmc_extra.csv`) after the 16-vs-24 results were in, so that
the reference is 24 seeds as the outram side is; the 24-vs-24 figure is the
headline and the 24-vs-16 figure (the one the plan's z values referred to) is
shown beside it.

| pitch | outram seeds | outram k | OpenMC | outram minus OpenMC | z | predicted (range) | outcome |
|---|---|---|---|---|---|---|---|
| 1.63576 | 1-8 (recorded) | 1.06323 +- 0.00039 | 24 seeds, 1.06232 +- 0.00019 | +92 +- 44 | +2.09 | | |
| 1.63576 | 9-24 (new) | 1.06177 +- 0.00022 | | -55 +- 29 | -1.91 | | |
| **1.63576** | **1-24** | **1.06225 +- 0.00024** (sd 117) | 24 seeds | **-6 +- 31** | **-0.20** | +45 (-30 .. +120) | **inside the range, 51 pcm below the central value; gate passes** |
| 1.63576 | 1-24 | same | 16 seeds, 1.06253 +- 0.00022 | -27 +- 32 | -0.85 | +45 (-30 .. +120) | inside the range by 3 pcm, 72 pcm below the central value |
| 1.75 | 1-8 (recorded) | 1.01168 +- 0.00033 | 24 seeds, 1.01130 +- 0.00026 | +39 +- 42 | +0.93 | | |
| 1.75 | 9-24 (new) | 1.01153 +- 0.00027 | | +23 +- 37 | +0.62 | | |
| **1.75** | **1-24** | **1.01158 +- 0.00021** (sd 102) | 24 seeds | **+28 +- 33** | **+0.86** | +55 (-20 .. +130) | **held, 27 pcm below the central value; gate passes** |
| 1.75 | 1-24 | same | 16 seeds, 1.01104 +- 0.00033 | +54 +- 39 | +1.38 | +55 (-20 .. +130) | held (the plan's z, +1.4, matches) |

- Both pitches pass the |z| <= 2 gate against OpenMC, with either reference. The
  refutation conditions of the plan: (a) outside the range, no, though 1.63576 cm
  sits at the low edge against the 16-seed reference; (b) negative beyond -30,
  no (-27 and -6); (c) |z| > 2, no.
- **1.63576 cm: the 8-seed block that the first re-measure quoted (+107 +- 50
  from the log-spaced build, +71 to +92 against OpenMC) was a high draw**:
  seeds 9-24 minus seeds 1-8 is **-147 +- 45 pcm (z -3.3, a post-hoc look at a
  block difference, so not a 3.3 sigma finding)**; at 1.75 cm the same split is
  -16 +- 43. The first 8 seeds are exactly reproducible (above), so this is the
  random-number draw of those seeds, as at 2.40 cm (seeds 1-8 low there, here
  high). Quote the **24-seed values**; the 8-seed values of the first
  re-measure and of `pitch_sweep.md` ("1.06323 +- 0.00040, +107 +- 50 from the
  log-spaced build") are superseded.
- **What the flip did, with the 24 seeds, per pitch** (new minus the 2026-11-18
  log-spaced 16-seed outram values, 1.06216 and 1.01104): **+9 +- 39 pcm** at
  1.63576 cm, **+54 +- 37 pcm** at 1.75 cm, and at 2.40 cm +106 +- 36 pcm
  (`gap_240_550.md`: 0.70536 to 0.70642). The three average +56 +- 22 pcm; this
  combination was chosen after seeing the three, and per pitch only 2.40 cm and
  1.75 cm are above 1.4 sigma. The first re-measure's reading (the node grid
  raises k in H-in-H2O systems by tens of pcm) stands as a pooled statement and
  is **not** resolved at 1.63576 cm.
- My prediction missed on the central values (+45 predicted, -6 or -27
  measured, i.e. 1.6 to 2.2 of the 32 pcm sigma at 1.63576 cm; +55 predicted, +28
  measured at 1.75 cm): the model "pre-flip offset plus about +80 pcm" is too high at
  1.63576 cm. The 8-seed number I shrunk from was the high block.

**Not done:** the 36-nuclide tier at 2.40 cm (the plan made it conditional; the
clock went to the other items, and its OpenMC side is a single run per pitch, so
no gate could be claimed). Its status is unchanged: not re-measured.

### Item 2: `icsbep_five_route_keff`, routes 3 and 5

Read from the code first (plan): the `"ace"` arm builds H(H2O) with
`ThermalScattering::from_ace` on `293.6K/HinH2O.ace`, and the knob is read only in
`from_tape` / `from_tape_with_options`, so routes 3 and 5 cannot move.
**Measured**: HST-009, 5000 x [40 + 120], seeds 1-3, each route as a knob pair
(`OUTRAM_SAB_LOG_SPACED_GRID` unset against `=1`), 1 thread, same binary:

| route | seed | k, knob off (default) | k, knob on (old build) | difference |
|---|---|---|---|---|
| 3 (NJOY2016 ACE) | 1, 2, 3 | 1.001330, 1.001075, 1.001708 | 1.001330, 1.001075, 1.001708 | **0.000000 each (bit-identical, k and k_std)** |
| 5 (Rust-NJOY ACE) | 1, 2, 3 | 1.001330, 1.001075, 1.001708 | 1.001330, 1.001075, 1.001708 | **0.000000 each** |
| 4 (ENDF + LEAPR), positive control | 1 | 1.00029 +- 0.00127 | 1.00128 +- 0.00143 | **-99 pcm** (the second re-measure's seed-1 control gave -99 too; the knob is live in this binary) |

**Prediction held exactly** (difference exactly 0 on routes 3 and 5; route 4
must differ). Two things worth knowing: (i) routes 3 and 5 also agree with each
other to every digit at these seeds, as they do in the committed campaign
(`data/per_seed_keff.csv`: both read 1.000823, 1.001497, 1.001068 at seeds 1-3 for
its run size); the Rust-NJOY ACE files differ from NJOY2016's in the header
words only (`cmp` stops at byte 38 / 3), consistent with the earlier
word-for-word record; (ii) the absolute k differs from the campaign's (1.00133 vs
1.000823 at seed 1) because this run is 1 thread with the present code and a
library I assembled in scratch (below), where the campaign's run was at a
different commit; **it is not compared and not a result**, only the knob pairs
are. Libraries (not committed, scratch): route 3's from the submodule's U-234/235/
236/238 tables, the 12 HST-009 nuclides' other tables built with
`scripts/make_njoy_library.sh` on the shared NJOY2016 build (F-19, and the U-236 /
U-238 / F-19 0 K companions; Al-27, Mn-55, O-16, Si, H-1 and U-234 symlinked from
an earlier scratch library of the same recipe, `remeasure3_550/fiveroute/njoylib.log`);
route 5's from `write_ace_library` with `--thermal-leapr` as `run_all.sh` does
(`rustlib.log`). All 12 nuclides carry DBRC, 5 carry URR, S(a,b) on, in both
(each `.err`).

**Not run, and why:** Godiva and Jemima (no S(a,b); nothing for the flip to
reach); the 35-nuclide lattice on routes 3 and 5 (the same `from_ace` line as
HST-009; 35 nuclides x two libraries for a fixed result); `msre` is refused on
these routes by the driver. The statement "routes 3 and 5 do not move" is
therefore **measured on HST-009 (3 seeds, 0 pcm) and read from the code for the
rest**.

### Item 3: `fhr_ring_rpt_endf`, all seven solves

`OUTRAM_FHR_ALL_SEED=S` (existing knob), the whole driver per seed (explicit
TRISO, ring-RPT and naive-homogenised on the r = 3 sphere and the cube, plus
the ring-RPT CSG sphere), 4000 x [30 + 80], 600 K, default arm against
`OUTRAM_SAB_LOG_SPACED_GRID=1`, seeds 1-4 (3 and 4 were added by the clock
rule in the plan, before any seed-1-2 comparison was read; a fifth seed was
started and stopped unread for lack of time). Raw: `remeasure3_550/fhr/`;
arithmetic with `sab_per_material_1581/ensembles_1635_item1/fhr_extract.awk`
and `fhr_stat.awk` (old = knob on, new = default; sem = sd/sqrt(N)).

**Controls, held exactly.** The knob-on arm reproduces the 2026-11-18 record's
build on all seven solves and four seeds (e.g. means 1.36053, 1.40162, 1.36141,
1.39714, 1.33093, 1.37865, 1.36336, identical to `sab_per_material_1581`'s "new"
column), and the default arm's CSG solve gives 1.36684, 1.36561, 1.36466, 1.36265
at seeds 1-4, identical to `remeasure2_550/fhr_new.err` MOVING seeds 1-4.

| solve | old (knob on) mean (sd) | new (default) mean (sd) | new - old +- sigma (pcm) | z |
|---|---|---|---|---|
| explicit TRISO, sphere | 1.36053 (200) | 1.36110 (237) | +57 +- 155 | +0.37 |
| explicit TRISO, cube | 1.40162 (335) | 1.40167 (68) | +5 +- 171 | +0.03 |
| ring-RPT, sphere | 1.36141 (107) | 1.36285 (255) | +144 +- 138 | +1.04 |
| ring-RPT, cube | 1.39714 (137) | 1.39691 (195) | -23 +- 119 | -0.19 |
| naive homogenised, sphere | 1.33093 (302) | 1.32966 (85) | -127 +- 157 | -0.81 |
| naive homogenised, cube | 1.37865 (96) | 1.37920 (124) | +55 +- 79 | +0.70 |
| ring-RPT, CSG sphere | 1.36336 (126) | 1.36494 (177) | +158 +- 109 | +1.45 |

Mean of the seven: **+38 pcm**. **Nothing is resolved**: every |z| <= 1.5, six
of seven are inside 160 pcm, resolution at N = 4 is 80-170 pcm. The prediction
(consistent with zero, mean within +-300, no single solve beyond 3.5 sigma)
**held on the four-seed result**. Disclosure: at N = 3, a look I took when the
seed-4 runs started, the CSG solve read +291 +- 74 pcm (z +3.9), above the 3.5
sigma refutation line I had written down; the per-seed CSG differences were
+329, +333, +211, and then -242 at seed 4. The plan fixed N by the clock, and 4
is the final N, but the N = 3 excursion is recorded: it is what a single noisy
solve looks like with a staircase of 100-250 pcm per-seed sd, and the CSG solve's
8-seed value in the second re-measure (+77 +- 86) is the better estimate.
Seven comparisons: about a 30 % chance of at least one |z| > 2 under the null,
and none is above 1.5. The ring-RPT sphere and CSG solves lean positive
(+144, +158), as G4 (+77) and G5-ring (+192) did, all below 2.3 sigma; the
explicit-TRISO solves, the same graphite physics without the ring, do not
(+57, +5), and the naive homogenised sphere leans the other way (-127). I
cannot give the graphite-pebble ring-RPT a single sign from these data, and do
not claim one.

### Summary of this pass

| case | move or result | z | predicted | outcome |
|---|---|---|---|---|
| LCT-008 1.63576 cm, 24 seeds vs OpenMC 24 / 16 | -6 +- 31 / -27 +- 32 | -0.20 / -0.85 | +45 (-30 .. +120) | inside; central value missed by 51 / 72 pcm; gate passes |
| LCT-008 1.75 cm, 24 seeds vs OpenMC 24 / 16 | +28 +- 33 / +54 +- 39 | +0.86 / +1.38 | +55 (-20 .. +130) | held; gate passes |
| HST-009 route 3, knob pair, 3 seeds | 0 pcm, bit-identical | | exactly 0 | held |
| HST-009 route 5, knob pair, 3 seeds | 0 pcm, bit-identical | | exactly 0 | held |
| HST-009 route 4 control, seed 1 | -99 pcm | | must differ | held |
| FHR seven solves, 4 seeds, new minus old | +38 pcm mean of seven; extremes +158 (CSG) and -127 (naive sphere) | all <= 1.5 | consistent with zero | held (N = 3 excursion disclosed) |

What this pass adds: the LCT-008 pin cell agrees with OpenMC at all three
pitches within the gate on the default (node-grid) route, at 24 seeds; the ACE
routes 3 and 5 are not touched by the flip (measured); the FHR pebble's seven
solves show no resolved move. What it does not add: any resolution below about
100 pcm on the FHR solves; the 36-nuclide tier; the 35-nuclide lattice, MSRE N-1
(still the uncontrolled W1 of the second re-measure), `nee_soon::htr10_rmc`.
Nothing was tuned, no tolerance moved, no pin changed.

**UPDATED 2026-11-19:** MSRE N-1 and `nee_soon::htr10_rmc` were re-measured as paired runs in the second part of the third re-measure, below.

## Third re-measure, part 2 (2026-11-19, gh:#550): HTR-10 whole core and MSRE N-1, paired

Plan and signed predictions, committed before any paired run:
`sab_reference_550_remeasure3_core_plan.md` (commit `3f0b2d35f`, committed as `sab_reference_550_remeasure3_plan.md` and renamed at the merge with part 1). Raw outputs:
`remeasure3_550/`. Research, education and V&V only. Base `6b3b64807`,
`--release`, knob = `OUTRAM_SAB_LOG_SPACED_GRID=1`. Difference = default (node
grid) minus knob-on (log-spaced).

Hardware for every run: Intel Xeon @ 2.10 GHz (cloud container, KVM), 4 logical
cores, 15.7 GiB RAM, Linux 6.18.44, CPU only; the box was shared with another
agent (load average 6 to 12), so timings are indicative only.

| id | case | old (knob on) | new (default) | new - old +- sigma | z | predicted | outcome |
|---|---|---|---|---|---|---|---|
| H1 | `htr10_rmc_keff` (the `nee_soon::htr10_rmc` model, default LEAPR laws), 4000 x [5+90], seed 20260917, 14 rings x 12 layers, 1 thread per arm, two arms side by side | 0.991743 +- 0.001673 | 0.991090 +- 0.001686 | **-65 +- 237 pcm** | -0.27 | -10 (-80 .. +60) at an expected sigma of 300; refuted only outside +-600 | **held, unresolved**: inside the range, but sigma 237 cannot resolve tens of pcm |
| M1 | `msre_n1_first_criticality --mo natural --seeds 1 --threads 2`, 20000 x [100+200], default model | 1.019902 +- 0.000544 | 1.016934 +- 0.000488 (second re-measure, same code) | **-297 +- 73 pcm** | **-4.1** | -10 (-80 .. +60), and "the earlier -183 is not the flip" (knob-on should read about 1.0162) | **missed, both statements; resolved on one seed pair** |

### H1: HTR-10 whole core

The two arms ran the same commit, seed and statistics (380000 histories each,
360000 active), one thread each, as the plan fixed. Data 491.7 s (default) and
492.6 s (knob), transport 7490.9 s and 7467.1 s. Both: 0 lost, 0 stuck, 0
majorant violations. The arms' internal sd came out at 169 and 167 pcm, below
the 213 expected, so the difference's sigma is 237, not 300. The difference,
-65 +- 237 pcm (z -0.27), is inside the predicted range and far inside the
+-600 refutation band; it **bounds |flip| below about 475 pcm at 2 sigma at this
size and resolves nothing finer**. It is consistent with zero and with the -10
predicted, and equally with -300. (Against RMC 1.004288: -1320 pcm default,
-1255 knob-on; the benchmark gate is not what this pair tests.)

A second finding the pair does show: **both arms sit below the 2026-11-05
record** (0.995309 +- 0.001098, 10000 x [5+135], 16 threads): -422 +- 201 pcm
(default, z -2.1) and -357 +- 200 pcm (knob on, z -1.8). Because the knob-on arm
reproduces the pre-flip build and sits as low as the default, this is **not the
flip**. It is code drift since 2026-11-05, or the different protocol (4000 x
[5+90] against 10000 x [5+135], one thread against sixteen: the CSG hybrid
driver's thread-count independence is assumed, not tested, see the example's
doc), or a seed draw; I did not decompose it. The record 0.995309 is not
reproduced on today's code at this size, ~~and should not be quoted as the current
value until a like-for-like re-run decides which.~~ **CORRECTED 2026-11-20:** it
is reproduced within the statistics. Four seeds at this protocol average
0.993020, −229 ± 175 pcm from the record (z −1.3), and seed 20260917 is the
lowest of the four. 1 and 2 threads are bit-identical. Record:
`crates/nee_soon/verification_and_validation/htr10_record_offset_2026_11_19/`.

`htr10_correct_physics_is_default` (nee_soon) builds no data and cannot move; it
is run as a test, see the hand-off.

### M1: MSRE N-1

**The control is exact.** The knob-on seed 1 is 1.019902 +- 0.000544, to every
printed digit the value the msre_n1 README records for "the default re-run on a
later binary (`42b372a59`, 2026-11-19, before the flip)". The knob therefore
reproduces the pre-flip build on a third case, and the drift is separated:
11-13 binary seed 1 = 1.017528 -> pre-flip same-day seed 1 = 1.019902 (+237 pcm,
other code, not the flip) -> default seed 1 = 1.016934 (**-297 +- 73 pcm, the
flip**).

The default arm is the second re-measure's seed 1 (`remeasure2_550/msre_new.csv`),
not re-run: no `crates/*/src` file changed between its commit and base, and runs
are deterministic for a fixed seed and thread count (2); that is an assumption I
did not test with a repeat run. **One seed pair only**: seed 2's knob-on arm was
not run (about 55 min, over budget), so the second re-measure's seed 2 default
(1.015550) has no control. Sigma is the two runs' internal sd in quadrature; the
six-seed record's seed-to-seed sd is about 56 pcm, consistent with it.

What this says. The earlier "-183 +- 73 pcm against the 11-13 record, not
attributable to the flip" was wrong in its attribution: **the flip itself moves
N-1 by -297 +- 73 pcm (4.1 sigma) on seed 1**, more than the -183, partly offset
by +237 from other code. Both of my predictions missed: -10 (-80 .. +60), and the
attribution (knob-on about 1.0162; it read 1.0199). The direction, lower k, moves
N-1 toward k_ref 1.00080, **which flatters the model, and is a reason to look
harder and not to accept**: the node grid changes the carbon S(a,b) at 911 K
(LEAPR deck) and H(H2O) in the shield, and I did not separate the two. One
4 sigma pair among about thirty comparisons made today is possible under no
effect, so a replication (knob-on seeds 2 and 3) is what would settle it. It is
the largest flip-caused move of the #550 series (water systems +30 to +110 pcm,
graphite pebbles unresolved at 100 to 300).

**No pin moved**: nothing in the crates' tests pins the MSRE N-1 k (searched).
The msre_n1 README headline (+1726 pcm, 6 seeds, 2026-11-13) is unchanged but is
stale in two ways, the +237 other-code drift and this flip; not re-measured at
six seeds.

### Not done in this pass

- MSRE N-1 knob-on seeds 2 and 3, and the default seed 3.
- A like-for-like HTR-10 re-run at the record's protocol, to explain the -400 pcm
  against 0.995309.
- Separating the carbon and H2O contributions to M1.
