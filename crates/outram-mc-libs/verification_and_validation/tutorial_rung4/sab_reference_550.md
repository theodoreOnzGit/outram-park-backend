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
