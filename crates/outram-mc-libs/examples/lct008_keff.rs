//! **LEU-COMP-THERM-008** — the B&W critical lattices, a 2.459 w/o UO₂ rod
//! array in borated water. The one benchmark left that tests **U-238 resolved
//! resonance escape at strong self-shielding**.
//!
//! # Why this exists
//!
//! The FHR ring-RPT study (`verification_and_validation/ring_rpt/`,
//! `op-mzvp.2.12`) narrowed a **+4004 pcm** disagreement against its OpenMC
//! reference down to one scalar: the fraction of neutrons that cross 0.625 eV,
//! an effective resonance integral **11 % lower here**. Production per
//! absorption agrees to 0.07 % in the thermal group and 0.02 % in the fast
//! group, so nothing that changes a reaction-rate *ratio* can be the cause.
//!
//! Three measured criticality benchmarks have since been reproduced on this
//! same data path and driver, and none of them touches that scalar:
//!
//! | benchmark | spectrum | U-238 | `p` | result |
//! |---|---|---|---|---|
//! | HEU-MET-FAST-001 (Godiva) | fast | 5 % of HM | — | +16 ± 11 pcm (256 seeds) |
//! | IEU-MET-FAST-002 (Jemima) | fast | 83 % / 99.3 % | — | +6 ± 173 pcm |
//! | HEU-SOL-THERM-009 case 1 | thermal | 5 % of HM | ≈ 0.95 | −18 ± 171 pcm |
//! | **this case** | **thermal** | **97.5 % of HM** | **≈ 0.75** | ? |
//!
//! Godiva and Jemima are fast — U-238 is present, in bulk, but no neutron is
//! slowed *through* the resolved resonances by a moderator, so there is no
//! self-shielded resonance integral to get wrong. HST-009 is thermal but its
//! fuel is 90 % HEU in solution: U-238 is 5 % of the heavy metal and the mixture
//! is homogeneous, so its resonance escape is ≈ 0.95 and essentially unshielded.
//!
//! LEU-COMP-THERM-008 is the complement of all three at once: **thermal**, with
//! U-238 at **97.5 %** of the heavy metal, in **lumped** fuel. The run measures
//! how lumped, from its own reconstruction: the fuel's `Σ_t` peaks at
//! **171.1 cm⁻¹ at 6.674 eV**, 434× its 0.39 cm⁻¹ at 5 eV, so the 1.030 cm
//! pellet is **176 mean free paths across** at the 6.67 eV resonance. The
//! resonance flux depression is as strong as it gets.
//!
//! **That is the whole point: this is the discriminator.** A result near 1.0000
//! clears U-238 resonance escape and leaves the reference deck as the only
//! surviving candidate. A result several thousand pcm high reproduces the FHR
//! residual on a *measured* configuration and hands the hunt a target that
//! depends on nobody's deck.
//!
//! # Results (2026-09-14, ENDF/B-VIII.0, 10 000 × [250 + 400] generations) — RESOLVED
//!
//! ```text
//! case 1   k_eff = 1.00201 ± 0.00063      Δk = +201 ± 63 pcm
//! case 2   k_eff = 1.00237 ± 0.00063      Δk = +237 ± 63 pcm
//! case 8   k_eff = 1.00087 ± 0.00062      Δk = +87  ± 62 pcm
//! ```
//!
//! All three independently critical configurations agree with the **measured**
//! benchmark, comfortably inside its own ±600 pcm model uncertainty. Transport
//! was ~1080 s per case.
//!
//! A small residual of order **+175 pcm** remains and *is* now resolved at this
//! statistics (σ = 63 pcm, so ~3 σ from zero). It is not noise and should not be
//! described as zero; see "what is left" below.
//!
//! # The old +2950 pcm was a geometry defect, and this file had already proved it was not the data
//!
//! Until 2026-09-14 the three cases read **+2950 / +2271 / +1713 pcm**. The
//! cause was
//! [`Geometry::cross_surface`](outram_mc_libs::geometry::geometry::Geometry::cross_surface)
//! being handed a **global** position for a surface that lives inside a
//! *translated* lattice universe, so the surface normal was computed about the
//! wrong centre. `nudge_across` then pushed the particle 1e-9 along that wrong
//! normal, which can land it back on the side it came from; the following
//! `locate` returned the cell it was leaving, and the whole next flight segment
//! was attributed to the wrong material. This benchmark is 23 lattices of
//! **cylindrical pins** — the affected class exactly.
//!
//! **Paired A/B at matched settings** (3000 × [80 + 150]), which is what makes
//! the attribution rather than the comparison to an older record:
//!
//! | | k_eff | Δk from the benchmark |
//! |---|---|---|
//! | before the fix | 1.02341 ± 0.00188 | +2341 ± 188 pcm |
//! | after the fix | 1.00037 ± 0.00189 | +37 ± 189 pcm |
//! | **difference** | | **−2304 ± 267 pcm (8.6 σ)** |
//!
//! The lower-statistics post-fix arms (+37 / +80 / +176 pcm) and the full-
//! statistics ones above agree case by case to **0.8 / 0.9 / 0.4 σ**, so the
//! two sets are one result at different precision.
//!
//! Fixed by `Geometry::cross_surface_in_frame`; diagnosis in
//! `tests/openmc_notebooks/triso.rs::triso_nested_lattice_surface_vs_delta_keff`.
//!
//! ## The spread across cases is what identified it, and this file already had that argument
//!
//! The CORRECTION below refuted a U-238 resonance-escape explanation on exactly
//! the right grounds: pitch, pellet, clad and fuel are identical in all three
//! configurations, so resonance escape `p` is the same in each and an error in
//! it **must give the same Δk**. It did not — the three spanned **1237 pcm**.
//!
//! That argument was correct, and a geometry defect is what explains a spread,
//! because the error scales with how much pin surface a neutron crosses, which
//! differs between configurations. The spread is now **150 pcm across a ~88 pcm
//! combined σ, i.e. 1.7 σ — not resolved.** The three configurations are no
//! longer distinguishable from each other, which is the shape a *data* residual
//! would have and the old numbers did not.
//!
//! **It was not the nuclear data.** Established rather than assumed — see the
//! uniform-material discriminator in `triso.rs`, where one material written
//! into both slots makes two independent trackers agree to 0.0 σ while the
//! lattice geometry is fully retained. Two further data hypotheses (the
//! majorant below its grid floor, and across the resolved-resonance region)
//! were tested and killed separately.
//!
//! ## What is left, and what it is worth comparing to
//!
//! The remaining ~+175 pcm is small, positive, and consistent across three
//! configurations. For scale, ~~Godiva's pooled residual is +214 ± 20 pcm
//! (96 seeds)~~ **CORRECTED 2026-09-18 — two errors in one sentence.**
//! `+214 ± 20` was **64** seeds, not 96 (the 96-seed study gave `+228 ± 18`
//! before the MT=91 Q-value cap and `+314 ± 21` after); and `+214` is itself
//! **superseded** by **`+16 ± 11 pcm` over 256 seeds**, after the discrete
//! inelastic angular distributions landed (`op-tm9f`). Use `+16 ± 11`.
//!
//! The comparison it was making still stands: Godiva is a *fast bare sphere*
//! with no lattice at all, and is structurally untouched by the fix above
//! (`run_keff` never calls `cross_surface`). But note the scale argument is
//! now much weaker — Godiva's residual is ~16 pcm, not ~214, so it no longer
//! provides a "comparable residual elsewhere" for this +175 pcm.
//!
//! Two residuals of similar size and sign on physically very different systems
//! is **an observation worth pricing, not a conclusion**. It is equally
//! consistent with a common small systematic, with a data-library difference,
//! and with coincidence — three configurations here cannot separate those. The
//! hunt for the fast-spectrum residual is GitHub issue #206 / `bn:op-mfs4`, and
//! establishing a like-for-like OpenMC reference on the same evaluation is its
//! first step for exactly this reason.
//!
//! **Source convergence** is adequate but not generous at these settings:
//! inactive quarters 0.9932 / 1.0010 / 0.9993 / 1.0006 on case 1, with active
//! halves 1.00183 then 1.00220. The residual drift between halves suggests more
//! inactive generations would be worth having before reading much into a
//! sub-100 pcm effect.
//!
//! **Supersedes (2026-09-11, same 10 000 × [250 + 400] settings, pre-fix):**
//! case 1 `k_eff = 1.02950 ± 0.00061`, +2950 ± 61 pcm; case 2 +2271 ± 61 pcm;
//! case 8 +1713 ± 60 pcm.
//!
//! # CORRECTION 2026-09-11 — it is NOT U-238 resonance escape
//!
//! This file originally went on to attribute the residual to **self-shielded
//! U-238 resonance absorption**, on an argument from benchmark coverage: this is
//! the only reproduced case that is thermal *and* U-238-dominated *and* lumped.
//! **That attribution is refuted by this benchmark's own other cases.**
//!
//! LEU-COMP-THERM-008 ships several **independently critical** configurations
//! that share one pin cell and differ in how the poison is supplied. Run with
//! `--case`:
//!
//! | case | soluble B-10 | poison rods | fuel pins | `Δk` |
//! |---|---|---|---|---|
//! | 1 | 1511 ppm | none | 4961 | **+2950 ± 61 pcm** |
//! | 2 | 1335.5 ppm | none | 4808 | **+2271 ± 61 pcm** |
//! | 8 | 794 ppm | 144 pyrex | 4808 | **+1713 ± 60 pcm** |
//!
//! The pitch, pellet, clad and fuel are identical in all three, so resonance
//! escape `p` is the same in each and an error in it must give the **same** `Δk`.
//! The spread is **1237 ± 86 pcm — 14σ**, monotone in the soluble boron.
//!
//! Stated as reactivity *differences*, which owes nothing to any estimate of
//! absorption shares: every pairwise difference is truly **zero**, because every
//! case is critical. This code gets them wrong by **+679 ± 86** (1→2) and
//! **+558 ± 86 pcm** (2→8). Boron is removed in both steps, and in both this
//! code says `k` rises *less* than it should — **its boron is worth too little**.
//!
//! It is not the B-10 cross section: `examples/b10_thermal_probe.rs` gives
//! 3845.9 b at 0.0253 eV against the 3835 b standard, exactly 1/v. So it is the
//! **thermal flux where the absorber sits**. See the ring-RPT V&V document,
//! Interpretation 18, for the rest of the trail.
//!
//! The magnitude agreed with the pebble quantitatively, and the prediction was
//! written down before the run: the pebble's six factors imply an effective
//! resonance integral 11 % low, which carried onto `p ≈ 0.75` predicts
//! `+3200 pcm` against the `+2950 ± 61` measured. **That agreement turned out to
//! be a coincidence** — see the correction above. It is recorded because a
//! successful quantitative prediction is exactly the kind of evidence that makes
//! a wrong attribution feel settled, and the case scan is what broke it.
//!
//! Source convergence: the mean `k` over the four quarters of the 250 inactive
//! generations ran 1.0233 / 1.0316 / 1.0286 / 1.0299 — settled after the first
//! quarter — and the two halves of the active block gave 1.02874 and 1.03025, a
//! 151 pcm difference against a 122 pcm 1σ, so 1.2σ and no drift.
//!
//! What this does **not** say is *which* step is wrong. Every cross section
//! involved is verified against NJOY2016's own PENDF — `σ_γ(E)` pointwise to
//! ±0.04 %, its resonance *shape* to 0.12 % worst over six resonances, and its
//! infinitely-dilute resonance integral to +0.00 % — and the tracking method is
//! excluded twice over (delta vs surface-tracked CSG agree to 18 pcm on the
//! pebble, and this case is surface-tracked while the pebble is delta-tracked,
//! yet both are high). So the defect is in what transport *does* with a correct
//! `σ_γ`, not in `σ_γ` itself. See `verification_and_validation/ring_rpt/` and
//! bead `op-mzvp.2.12` for the live hypothesis list.
//!
//! # Re-measured 2026-09-12 and 2026-09-13 — unchanged by this week's fixes
//!
//! Three defects landed against the thermal/nuclear-data path this week: the
//! emission-table resize (GitHub #190), the bound kernels' fixed point (#191),
//! and the one that closed the FHR pebble's +4004 pcm residual outright — #193,
//! threshold reactions carrying a cross section *below* their threshold. This
//! case was re-run against all three, at 4000 x [120 + 250] rather than the
//! recorded 10 000 x [250 + 400]:
//!
//! | case | recorded | after #190/#193 | + continuous kernel | **+ MF=6 + E' linearisation** |
//! |---|---|---|---|---|
//! | 1 | +2950 ± 61 pcm | +2693 ± 120 pcm | +2552 ± 129 pcm | **+2665 ± 128 pcm** |
//! | 2 | +2271 ± 61 pcm | +2136 ± 126 pcm | +2122 ± 121 pcm | **+2086 ± 118 pcm** |
//! | 8 | +1713 ± 60 pcm | +1787 ± 127 pcm | +1658 ± 117 pcm | **+1605 ± 114 pcm** |
//!
//! **Nothing moved.** No single step shifts a case by 2σ, and the whole of this
//! week's thermal and nuclear-data work is worth roughly **−300 pcm out of
//! +2950**. The pairwise differences — the statement that owes nothing to any
//! absorption-share estimate — survive: **+579 ± 174 pcm** (1→2) against the
//! recorded +679 ± 86, and **+481 ± 164** (2→8) against +558 ± 86. Boron is still
//! worth too little here, by about the same amount.
//!
//! # RESOLVED 2026-09-16 — the residual is gone, and the prediction that said
//! # it would be is GitHub #188's own
//!
//! GitHub #188 ("H-in-H2O incoherent-inelastic kernel transfers 2-5.5 % too
//! little energy") was closed on 2026-09-14 by replacing `equiprobable_emission`
//! with the ported `aceth.f90::acesix`. Its closing text named the direct test
//! and stated the criterion in advance:
//!
//! > re-run LEU-COMP-THERM-008 cases 1, 2 and 8, and the three `Δk` values must
//! > collapse together and toward zero
//!
//! **That test had never been run.** It was run on 2026-09-16, at the same
//! 4000 × [120 + 250] as the 2026-09-13 column, and both halves of the
//! criterion are met:
//!
//! | case | 2026-09-13 | **2026-09-16** | move |
//! |---|---|---|---|
//! | 1 | +2665 ± 128 pcm | **+157 ± 119 pcm** | −2508 |
//! | 2 | +2086 ± 118 pcm | **+1 ± 126 pcm** | −2085 |
//! | 8 | +1605 ± 114 pcm | **+124 ± 124 pcm** | −1481 |
//!
//! **Toward zero:** every case is now within **1.3 σ** of a measured critical
//! experiment, mean `|Δk|` = 94 pcm, against 20 σ before.
//!
//! **Together:** the spread across the three collapsed from **1060 pcm to
//! 156 pcm**.
//!
//! And the statement that owes nothing to any estimate of absorption shares —
//! the pairwise differences, which are *truly* zero because every case is
//! independently critical:
//!
//! | difference | before | **after** |
//! |---|---|---|
//! | 1 → 2 | +579 ± 174 pcm (3.3 σ) | **−156 ± 173 pcm (0.9 σ)** |
//! | 2 → 8 | +481 ± 164 pcm (2.9 σ) | **+123 ± 177 pcm (0.7 σ)** |
//!
//! **"Its boron is worth too little" is resolved.** That was this case's
//! load-bearing finding — the one conclusion that survived every earlier fix
//! and that no absorption-share argument could explain away. Both pairwise
//! differences are now consistent with zero.
//!
//! ## What this does and does not establish
//!
//! **Does:** the thermal path's largest known defect is closed, measured
//! against a *measured critical experiment* rather than another code, on the
//! one benchmark here whose fuel is both thermal and strongly self-shielded in
//! the U-238 resolved resonances.
//!
//! **Does not:** it does not retroactively justify the attribution history
//! above. The residual was attributed to U-238 resonance escape, that
//! attribution was **refuted** by the case scan, and the real cause was a
//! thermal scattering kernel. A quantitative prediction agreeing to 8 % (the
//! `+3200` against `+2950`) turned out to be coincidence. That sequence is
//! recorded because it is how a wrong attribution comes to feel settled, and it
//! is worth more than the number that finally came out right.
//!
//! **Does not:** say anything about DBRC or URR probability tables, both landed
//! 2026-09-16 and both **opt-in and not enabled here**. This run uses neither.
//! Pricing them on this case is the obvious next measurement now that it has
//! headroom — at ±120 pcm it can resolve an effect of a few hundred pcm.
//!
//! # DBRC and URR probability tables priced here, 2026-09-16 — both BOUNDED,
//! # neither resolved
//!
//! With the residual gone this case finally had the headroom to price the two
//! resonance treatments that landed the same day, and which a bare fast sphere
//! cannot see at all. Both are opt-in (`--dbrc`, `--urr`) and both default off,
//! so the no-flag arm reproduces the baseline above and the difference is
//! attributable to the flag alone.
//!
//! | arm | `Δk` | worth against the baseline |
//! |---|---|---|
//! | neither (baseline) | +157 ± 119 pcm | — |
//! | `--dbrc` | +197 ± 129 pcm | **+40 ± 176 (0.2 σ)** |
//! | `--urr` | +60 ± 132 pcm | **−97 ± 178 (0.5 σ)** |
//! | both | +210 ± 129 pcm | **+53 ± 176 (0.3 σ)** |
//!
//! **Every one is consistent with zero, and none of them is a measurement.**
//! A single paired run here has `σ_diff ≈ 176 pcm`, so it can only resolve an
//! effect above **~350 pcm at 2 σ**. The published DBRC value for an LWR pin
//! cell is 100–200 pcm — *below* this run's resolution. So this result **does
//! not contradict the literature**; it simply cannot see an effect that size,
//! and saying "DBRC is worth nothing here" would be reading an unresolved
//! central value as a measurement. The honest statement is: **bounded below
//! ~350 pcm at 2 σ, consistent with zero.** Resolving either needs a
//! paired-seed ensemble, which is ordinary CPU rather than new physics.
//!
//! One thing the run *does* establish beyond the bound: the URR wiring
//! generalises past the nuclide it was verified on. Tables built for **all
//! three actinides** — U-234 over `[1.500e3, 1.000e5]` eV, U-235 over
//! `[2.250e3, 2.500e4]`, U-238 over `[2.000e4, 1.490e5]` — in about 5 s total
//! at `nladr = 16`. The control test could only show U-238.
//!
//! # Re-run 2026-09-13 against the last two changes, and why it was worth doing
//!
//! The fourth column is this case re-measured after the MT=91/MT=16 continuum
//! law moved to the evaluation's own ENDF MF=6 (gh:#192) and after THERMR's E'
//! grid gained the adaptive linearisation `calcem`/`sigl` uses. **The second of
//! those changes H-in-H₂O's thermal cross section by about −1 %** (it removed a
//! quadrature excess; see `vv::njoy_golden::H2O_XS`), and this benchmark is a
//! light-water lattice, so its recorded numbers could not be assumed to still
//! hold — they had been measured before both changes.
//!
//! They do hold. Case 1 moves **+113 ± 181 pcm** against the third column
//! (0.6 σ), case 2 **−36 ± 169**, case 8 **−53 ± 163** — nothing resolved. The
//! signs are consistent with less H moderation giving slightly less
//! thermalisation, but at 0.6 σ that reading is not established by these runs.
//!
//! So the conclusion is unchanged and now rests on current code: **LCT-008's
//! residual is not #193's, not the continuum emission law's, and not the E'
//! quadrature's.** It remains the thermal-kernel family's — gh:#188 — which is
//! still open.
//!
//! The continuous outgoing-energy law is the interesting one of the three,
//! because it is the only one that touches the H-in-H₂O kernel this benchmark's
//! residual is attributed to. It moved case 1 by −141 ± 177 pcm, case 2 by
//! −14 ± 175 and case 8 by −129 ± 173 — all under 1σ. That is consistent with
//! what it did to the kernel itself: it removed only about **20 %** of the width
//! deficit against NJOY2016 (−4.91 % → −4.02 %), and the rest lives in the
//! THERMR kernel underneath rather than in how the crate samples it.
//!
//! ## Why #193 could not have touched this case, checked rather than assumed
//!
//! #193 fires only where an evaluation opens a threshold MF=3 section at a
//! **non-zero** cross section, which the endpoint clamp then propagated to zero
//! energy. Scanning every threshold section (MT=16, 51–91, 103–117) of all
//! eleven nuclides this case loads — H-1, B-10, O-16, Al-27, Si-28/29/30, Mn-55,
//! U-234/235/238 — finds exactly one non-zero opening: **O-16 MT=103 at
//! (10.244 MeV, 5e-11 b)**. That is 1.3e-11 of O-16's total cross section and
//! sits above every neutron in this spectrum anyway.
//!
//! So the defect that carried the entire FHR pebble residual has no counterpart
//! here, and this benchmark's residual stays attributed to the H-in-H₂O
//! scattering law (GitHub #188).
//!
//! ## One recorded inference does not survive these error bars
//!
//! A note on `op-rpb6` reads the three cases as tracking soluble boron at
//! "~1.7 pcm/ppm", extrapolating to "only ~+350 pcm at zero boron". That fit
//! mixes case 8 into a soluble-boron line, and case 8 is the one that swaps
//! 717 ppm of *soluble* boron for **144 lumped pyrex rods** — a different poison
//! in a different geometry. Cases 1 and 2, which differ in soluble boron alone,
//! give **3.2 ± 1.0 pcm/ppm**; adding case 8 gives 1.3. Those are not the same
//! quantity and should not be averaged, and at ±1.0 pcm/ppm on a two-point slope
//! the extrapolation to zero boron carries ±1500 pcm — it is not a measurement
//! of anything yet. Settling it needs cases 1 and 2 at the full 10 000 x
//! [250 + 400], which would put the slope at roughly ±0.3 pcm/ppm.
//!
//! # The model, and where it comes from
//!
//! The three XML files in
//! `verification_and_validation/icsbep/leu-comp-therm-008/` are the OpenMC
//! model from `mit-crpg/benchmarks` (Paul Romano's ICSBEP model collection,
//! MIT licence). They are **parsed at run time**, not transcribed: the thing
//! under test is then the sourced specification itself, and a transcription
//! error — the failure mode this entire study has been chasing — cannot be
//! introduced by this file.
//!
//! Geometry: a 7 × 7 core lattice at 24.5364 cm pitch, each tile either water
//! or one of 22 distinct 15 × 15 pin lattices at 1.63576 cm pitch, inside a
//! vacuum-bounded cylinder `r < 76.200 cm`, `|z| < 81.662 cm`. Pins are
//! `r < 0.514858` UO₂, `0.514858 – 0.602996` Al-6061 clad, water outside.
//! 3 materials; the pyrex / vicor / alumina surfaces in the file are declared
//! but used by no cell (this is the unpoisoned core).
//!
//! ## What is approximated, and by how much
//!
//! ~~Only the **Al-6061 clad's trace alloying elements** are omitted, because
//! this environment has no ENDF tape for them: Mg-24/25/26, Ti-46…50,
//! Cr-50/52/53/54, Fe-54/56/57, Cu-63/65, Zn-64/66/67/68/70 and B-11.~~
//! **CORRECTED 2026-09-20 — NOTHING IS OMITTED ANY MORE.** All 24 of those
//! tapes (plus Fe-58 and Na-23, which that list also missed) were located in
//! the local ENDF/B-VIII.0 library and committed to `reference-data/endf/`,
//! and `TAPES` now covers **all 36 nuclides** the OpenMC material cards name.
//! The "no ENDF tape for them" claim was true when written and is not true now.
//! Nothing is omitted from the fuel, the moderator, or the boron poison
//! either — B-10, the entire worth of the 1511 ppm soluble boron, is present.
//!
//! **This changes the eigenvalue**, and the `--clad-omission-bound` figures
//! below were measured against the omitted-clad model. Re-measure before
//! quoting them.
//!
//! The run prints the omitted atom density and its share of the clad. To bound
//! its worth **by measurement rather than by assertion**, pass
//! `--clad-omission-bound`: the omitted density is re-added to the clad as
//! **Mn-55**, a stronger thermal absorber than any element on that list (they
//! are structural alloying metals, which is why they are in cladding at all),
//! and the run is repeated. The resulting Δk is a conservative upper bound on
//! the omission, computed from data in hand rather than from a remembered
//! cross section.
//!
//! # The reference value
//!
//! **`k_eff = 1.0000`**, because this is a *critical* configuration. An ICSBEP
//! benchmark model is an experiment measured at delayed critical, reduced to a
//! model whose `k_eff` is 1.0000 to within the evaluated experimental
//! uncertainty — 0.1–0.6 % for a lattice of this kind. The case-specific
//! uncertainty is not in the repository and the handbook is not reachable from
//! this environment, so the reference is treated as **`1.0000 ± 0.006`**, the
//! pessimistic end of that band. That is ample for a 4 % question.
//!
//! This is reasoning from what an ICSBEP critical benchmark *is*, not from a
//! remembered number — the distinction that matters, and the reason the atom
//! densities and geometry are read from a file instead of recalled.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example lct008_keff -- [--clad-omission-bound] [--particles N]
//!                              [--dbrc] [--urr]
//! ```

//!
//! ## POOLED RESULT (2026-09-18) — 32 seeds, `OUTRAM_BENCH_SEEDS=32`
//!
//! | | dk vs benchmark |
//! |---|---|
//! | **pooled mean, 32 seeds** | **+165 pcm, sem +/-25** |
//! | seed-to-seed sd | 143 pcm — what ONE run scatters by |
//! | previously recorded (SINGLE seed) | `+201 +/- 63 pcm` |
//! | one draw taken 2026-09-18 | `+89 +/- 126 pcm` |
//!
//! **Read the configuration caveat before comparing these.** The recorded
//! `+201 +/- 63` was measured at ~4x the histories this example now runs
//! (its sigma was 63 pcm; a single draw of the CURRENT default gives 126).
//! So `+201` and `+165` are not a like-for-like pair, and the apparent 36 pcm
//! agreement is partly a coincidence of two different configurations. What IS
//! like-for-like is `+89` against `+165`: the same deck, one seed against
//! thirty-two, 0.53 sd apart.
//!
//! `+165 +/- 25` sits well inside the benchmark's own ~600 pcm band, so this
//! case AGREES. The `distance from benchmark = 6.5 sem` the example prints is
//! measured in OUR sampling error only — pooling has shrunk that until the
//! experiment's uncertainty dominates, and quoting 6.5 sem as a discrepancy
//! would be wrong.
//!
//!
//! **PROVENANCE WARNING on that band (added 2026-09-18).** The `~600 pcm` is
//! NOT a quoted ICSBEP case uncertainty. Lines ~407-409 of this file say the
//! case-specific uncertainty "is not in the repository and the handbook is not
//! reachable from this environment", and take the worst case of a 0.1-0.6 %
//! range. `+165 +/- 25` would probably survive a realistic band, but that is
//! not established. Do not publish as agreement until the real uncertainty is
//! obtained (bead filed 2026-09-18).
//! Seed count follows the workspace standard of **32** (maintainer decision
//! 2026-09-18; see `examples/godiva_keff_ensemble.rs`).

use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken, SurfaceToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::lattice::{Lattice, RectLattice};
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind, ZCylinder, ZPlane};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::physics::compute::ComputeType;
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::reactor_physics::{run_keff_reactor_physics, ReactorPhysicsConfig};
use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};
use std::collections::BTreeMap;
use std::time::Instant;

/// The benchmark is at room temperature and the model gives no `<temperature>`,
/// so OpenMC's default applies. 293.6 K is also the tabulated temperature of the
/// `H(H2O)` law, so no thermal interpolation is involved.
const TEMP_K: f64 = 293.6;

const MATERIALS_XML_CASE1: &str =
    include_str!("../verification_and_validation/icsbep/leu-comp-therm-008/materials.xml");
const GEOMETRY_XML_CASE1: &str =
    include_str!("../verification_and_validation/icsbep/leu-comp-therm-008/geometry.xml");
const MATERIALS_XML_CASE2: &str =
    include_str!("../verification_and_validation/icsbep/leu-comp-therm-008/case-2/materials.xml");
const GEOMETRY_XML_CASE2: &str =
    include_str!("../verification_and_validation/icsbep/leu-comp-therm-008/case-2/geometry.xml");
const MATERIALS_XML_CASE8: &str =
    include_str!("../verification_and_validation/icsbep/leu-comp-therm-008/case-8/materials.xml");
const GEOMETRY_XML_CASE8: &str =
    include_str!("../verification_and_validation/icsbep/leu-comp-therm-008/case-8/geometry.xml");

/// The committed case, selected by `--case`.
///
/// Every case of LEU-COMP-THERM-008 is **independently critical**, and they
/// differ in how the poison is supplied: case 1 carries 1511 ppm of *soluble*
/// boron and no poison rods; case 8 carries 794 ppm and 144 *lumped* pyrex
/// burnable-poison rods. Same lattice, same pitch, same fuel. That makes the
/// pair a **differential measurement needing no new reference**: an error in
/// thermal absorption tracks the soluble-boron worth and its distribution, while
/// an error in epithermal resonance escape does not, because `p` is set by the
/// lattice and the lattice is unchanged.
static ACTIVE_CASE: std::sync::OnceLock<u32> = std::sync::OnceLock::new();

fn materials_xml() -> &'static str {
    match ACTIVE_CASE.get().copied().unwrap_or(1) {
        2 => MATERIALS_XML_CASE2,
        8 => MATERIALS_XML_CASE8,
        _ => MATERIALS_XML_CASE1,
    }
}
fn geometry_xml() -> &'static str {
    match ACTIVE_CASE.get().copied().unwrap_or(1) {
        2 => GEOMETRY_XML_CASE2,
        8 => GEOMETRY_XML_CASE8,
        _ => GEOMETRY_XML_CASE1,
    }
}

/// Pin universes, transcribed **by hand** off the committed `geometry.xml` — two
/// or three cells each, which is inside the size a reviewer can check by eye
/// (GitHub #184's rule). Each entry is `(universe id, [(outer radius, material
/// id)], material id outside them all)`.
const PIN_SHELLS: &[(i32, &[(f64, i32)], i32)] = &[
    // all-water pin cell
    (1, &[], 1),
    // fuel rod: UO2 to 0.514858, Al-6061 clad to 0.602996, water outside
    (2, &[(0.514_858, 2), (0.602_996, 3)], 1),
    // pyrex burnable-poison rod (case 8): glass to 0.585, water outside
    (3, &[(0.585_000, 4)], 1),
];

/// Nuclides this environment has an ENDF/B-VIII.0 tape for, by the OpenMC name
/// the model uses. Anything in the model and not in this table is omitted, and
/// the omission is reported and bounded (see the module docs).
/// **The default tape set: every nuclide the OpenMC material cards name.**
///
/// Correct physics is the DEFAULT, not an opt-in (`develop`, 2026-09-20). The
/// clad's trace alloying elements are part of the model, so they are loaded
/// unless someone deliberately asks for less with `--cheap-nuclides`.
///
/// # Cost
///
/// Each tape is resonance-reconstructed and Doppler-broadened on device
/// (RECONR + BROADR); the iron and chromium evaluations alone are 8-24 MB of
/// ENDF text apiece, and reconstruction dominates the run before a single
/// neutron moves. [`TAPES_CHEAP`] exists for iterating on geometry, tallies or
/// statistics, where the clad's trace elements change nothing being looked at.
const TAPES: &[(&str, &str)] = &[
    // Everything in the cheap tier ...
    ("H1", "n-001_H_001-ENDF8.0-Beta6.endf"),
    ("B10", "n-005_B_010-ENDF8.0.endf"),
    ("O16", "n-008_O_016-ENDF8.0.endf"),
    ("U234", "n-092_U_234-ENDF8.0.endf"),
    ("U235", "n-092_U_235-ENDF8.0.endf"),
    ("U238", "n-092_U_238.endf"),
    ("Al27", "n-013_Al_027-ENDF8.0.endf"),
    ("Si28", "n-014_Si_028-ENDF8.0.endf"),
    ("Si29", "n-014_Si_029-ENDF8.0.endf"),
    ("Si30", "n-014_Si_030-ENDF8.0.endf"),
    ("Mn55", "n-025_Mn_055-ENDF8.0.endf"),
    // ... plus the Al-6061 trace alloying elements, added 2026-09-20 from the
    // local ENDF/B-VIII.0 library. Before this they were absent and the model
    // ran WITHOUT them.
    ("B11", "n-005_B_011-ENDF8.0.endf"),
    ("Na23", "n-011_Na_023-ENDF8.0.endf"),
    ("Mg24", "n-012_Mg_024-ENDF8.0.endf"),
    ("Mg25", "n-012_Mg_025-ENDF8.0.endf"),
    ("Mg26", "n-012_Mg_026-ENDF8.0.endf"),
    ("Ti46", "n-022_Ti_046-ENDF8.0.endf"),
    ("Ti47", "n-022_Ti_047-ENDF8.0.endf"),
    ("Ti48", "n-022_Ti_048-ENDF8.0.endf"),
    ("Ti49", "n-022_Ti_049-ENDF8.0.endf"),
    ("Ti50", "n-022_Ti_050-ENDF8.0.endf"),
    ("Cr50", "n-024_Cr_050-ENDF8.0.endf"),
    ("Cr52", "n-024_Cr_052-ENDF8.0.endf"),
    ("Cr53", "n-024_Cr_053-ENDF8.0.endf"),
    ("Cr54", "n-024_Cr_054-ENDF8.0.endf"),
    ("Fe54", "n-026_Fe_054-ENDF8.0.endf"),
    ("Fe56", "n-026_Fe_056-ENDF8.0.endf"),
    ("Fe57", "n-026_Fe_057-ENDF8.0.endf"),
    ("Fe58", "n-026_Fe_058-ENDF8.0.endf"),
    ("Cu63", "n-029_Cu_063-ENDF8.0.endf"),
    ("Cu65", "n-029_Cu_065-ENDF8.0.endf"),
    ("Zn64", "n-030_Zn_064-ENDF8.0.endf"),
    ("Zn66", "n-030_Zn_066-ENDF8.0.endf"),
    ("Zn67", "n-030_Zn_067-ENDF8.0.endf"),
    ("Zn68", "n-030_Zn_068-ENDF8.0.endf"),
    ("Zn70", "n-030_Zn_070-ENDF8.0.endf"),
];

/// The **cheap** subset — fuel, moderator, soluble-boron poison, and the three
/// clad nuclides the clad cannot do without. Selected with `--cheap-nuclides`.
///
/// It exercises the identical transport path at a fraction of the
/// reconstruction cost, so it is the tier to use while iterating on geometry,
/// tallies or statistics. It is **not** the default: the model it describes is
/// incomplete, and correct physics is the default here.
const TAPES_CHEAP: &[(&str, &str)] = &[
    ("H1", "n-001_H_001-ENDF8.0-Beta6.endf"),
    ("B10", "n-005_B_010-ENDF8.0.endf"),
    ("O16", "n-008_O_016-ENDF8.0.endf"),
    ("U234", "n-092_U_234-ENDF8.0.endf"),
    ("U235", "n-092_U_235-ENDF8.0.endf"),
    ("U238", "n-092_U_238.endf"),
    // ── Clad, CHEAP tier: the three the clad cannot do without ───────────
    ("Al27", "n-013_Al_027-ENDF8.0.endf"),
    ("Si28", "n-014_Si_028-ENDF8.0.endf"),
    ("Si29", "n-014_Si_029-ENDF8.0.endf"),
    ("Si30", "n-014_Si_030-ENDF8.0.endf"),
    ("Mn55", "n-025_Mn_055-ENDF8.0.endf"),
];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let clad_bound = args.iter().any(|a| a == "--clad-omission-bound");
    let six_factors = args.iter().any(|a| a == "--six-factors");
    let n_particles = arg_usize(&args, "--particles").unwrap_or(4000);
    let n_inactive = arg_usize(&args, "--inactive").unwrap_or(120);
    let n_active = arg_usize(&args, "--active").unwrap_or(250);

    eprintln!("LEU-COMP-THERM-008 — B&W critical lattices, Core XI, 2.459 w/o UO₂");
    eprintln!("  specification: mit-crpg/benchmarks OpenMC model, parsed from XML at run time");
    eprintln!(
        "  nuclide tier: {} ({} tapes){}\n",
        if tapes().len() == TAPES.len() { "FULL" } else { "CHEAP" },
        tapes().len(),
        if tapes().len() == TAPES.len() {
            " -- every nuclide the OpenMC cards name (default); slow to reconstruct"
        } else {
            " -- fuel/moderator/poison + Al,Si,Mn clad (--cheap-nuclides)"
        }
    );

    let case: u32 = arg_usize(&args, "--case").unwrap_or(1) as u32;
    assert!(
        matches!(case, 1 | 2 | 8),
        "only cases 1, 2 and 8 are committed"
    );
    let _ = ACTIVE_CASE.set(case);
    eprintln!("  case {case}");

    let spec = parse_materials(materials_xml());
    let res_opts = ResonanceOptions {
        dbrc: args.iter().any(|a| a == "--dbrc"),
        urr: args.iter().any(|a| a == "--urr"),
    };
    if res_opts.dbrc || res_opts.urr {
        eprintln!(
            "Resonance treatments requested: dbrc={} urr={} (both default OFF; a run without \
             them reproduces the recorded baseline)",
            res_opts.dbrc, res_opts.urr
        );
    }
    // `--via-ace` routes every nuclide through this workspace's own ACER and
    // ACE reader instead of straight from the reconstructed ENDF.
    let ace_scratch = args.iter().any(|a| a == "--via-ace").then(|| {
        let d = std::env::temp_dir().join(format!("lct008_ace_{}", std::process::id()));
        std::fs::create_dir_all(&d).expect("scratch dir");
        eprintln!("Nuclide route: ENDF -> ACER -> .ace -> from_ace (--via-ace)");
        d
    });
    let (nuclides, slots, omitted) = load_nuclides(&spec, res_opts, ace_scratch.clone());
    let (materials, clad_idx) = build_materials(&spec, &slots, &omitted, false);
    report_omissions(&spec, &omitted);
    // `check_geometry`'s hand-written predicate resolves materials by the model's
    // own ids through `PIN_SHELLS`, but it does assume the file lists them in id
    // order starting at water = 1, so that is pinned rather than assumed.
    let ids: Vec<i32> = materials.iter().map(|m| m.id).collect();
    assert_eq!(
        ids,
        (1..=ids.len() as i32).collect::<Vec<_>>(),
        "the model's materials are no longer listed as ids 1..n in order"
    );
    assert_eq!(clad_idx, 2, "material 3 is the Al-6061 clad");

    report_self_shielding(&materials[1], &nuclides);

    let geom = build_geometry(&materials, true);
    let volume_fraction = check_geometry(&geom, &materials);

    let settings = KeffSettings {
        n_particles,
        n_inactive,
        n_active,
        temperature_k: TEMP_K,
        compute: ComputeType::CpuMultiThread(Default::default()),
        // `--seed` makes each invocation an independent draw, so a sweep can
        // time data and transport per run rather than pooling N seeds inside
        // one process the way `OUTRAM_BENCH_SEEDS` does (2026-09-24).
        seed: arg_usize(&args, "--seed")
            .map(|v| v as u64)
            .unwrap_or(KeffSettings::default().seed),
        ..KeffSettings::default()
    };
    // The fissionable region is the whole pin array; sample the core cylinder.
    let src = SourceBox {
        lower: Position::new(-R_CORE, -R_CORE, Z_LO),
        upper: Position::new(R_CORE, R_CORE, Z_HI),
    };
    eprintln!(
        "\n  {} histories/gen, {} inactive + {} active generations",
        settings.n_particles, settings.n_inactive, settings.n_active
    );

    let t = Instant::now();
    // Seed ensemble (OUTRAM_BENCH_SEEDS, default 1 -- unchanged single-seed
    // behaviour). One run of this case scatters by far more than the effects
    // being argued about, so a single draw cannot resolve a 100-200 pcm change;
    // see outram_mc_libs::vv::pooled. `run_keff_csg` is already internally
    // multi-threaded, so seeds run sequentially and each uses every core.
    let n_seeds = outram_mc_libs::vv::bench_seeds();
    let mut ens: Vec<f64> = Vec::with_capacity(n_seeds);
    let mut result = run_keff_csg(&geom, &materials, &nuclides, src, &settings, None);
    ens.push((result.k_mean - 1.0) * 1.0e5);
    // Pool the active-phase drift across seeds. LCT-008 is the large, loosely
    // coupled lattice in this set -- the case where a dominance ratio near 1
    // makes source-convergence bias most likely, and where the single-seed
    // trace showed the largest drift of the four.
    let mut drifts: Vec<f64> =
        vec![outram_mc_libs::vv::source_convergence_drift_pcm(&result, settings.n_inactive)];
    outram_mc_libs::vv::report_transport_losses("lct008_keff seed 1", &result);
    for seed in 2..=n_seeds as u64 {
        let s = KeffSettings {
            seed,
            ..settings.clone()
        };
        let r = run_keff_csg(&geom, &materials, &nuclides, src, &s, None);
        eprintln!("    seed {seed}: k = {:.5} +/- {:.5}", r.k_mean, r.k_std);
        ens.push((r.k_mean - 1.0) * 1.0e5);
        drifts.push(outram_mc_libs::vv::source_convergence_drift_pcm(&r, s.n_inactive));
        // Deliberately NOT `result = r`. Everything downstream -- the
        // convergence trace, the printed k_eff, the V&V gate and the
        // bounded-geometry cross-check -- is sized against SEED 1, which is
        // also what the single-seed default runs. Reassigning here silently
        // repoints all of them at the last seed of the ensemble.
    }
    if n_seeds > 1 {
        let good: Vec<f64> = drifts.iter().copied().filter(|d| d.is_finite()).collect();
        if good.len() > 1 {
            let (dm, dsd, dsem) = outram_mc_libs::vv::pooled(&good);
            println!("\n  POOLED SOURCE-CONVERGENCE DRIFT ({} seeds)", good.len());
            println!("    active-half drift = {dm:+.0} +/- {dsem:.0} pcm   (seed-to-seed sd {dsd:.0})");
            if dm.abs() > 2.0 * dsem {
                println!("    => RESOLVED systematic drift: source still moving while scoring.");
            } else {
                println!("    => not resolved; consistent with statistical scatter.");
            }
        }
        let (mean, sd, sem) = outram_mc_libs::vv::pooled(&ens);
        println!("\n  ENSEMBLE LEU-COMP-THERM-008 case {case}: {n_seeds} seeds");
        println!("    pooled dk    = {mean:+.0} pcm");
        println!("    seed-to-seed sd  = {sd:.0} pcm   (what ONE run scatters by)");
        println!("    uncertainty  sem = +/-{sem:.0} pcm   (on the pooled mean)");
        println!(
            "    distance from benchmark = {:.1} sem",
            (mean / sem).abs()
        );
    }
    eprintln!("  transport: {:.1} s", t.elapsed().as_secs_f64());

    let k = &result.k_by_generation;
    let trace = |lo: usize, hi: usize| -> f64 { k[lo..hi].iter().sum::<f64>() / (hi - lo) as f64 };
    let n_in = settings.n_inactive.min(k.len());
    eprintln!(
        "  source convergence, mean k over inactive quarters: {:.4} {:.4} {:.4} {:.4}",
        trace(0, n_in / 4),
        trace(n_in / 4, n_in / 2),
        trace(n_in / 2, 3 * n_in / 4),
        trace(3 * n_in / 4, n_in),
    );
    let half = n_in + (k.len() - n_in) / 2;
    eprintln!(
        "  active halves: {:.5} then {:.5} (a drift here means more inactive generations)",
        trace(n_in, half),
        trace(half, k.len()),
    );

    println!("\n=== LEU-COMP-THERM-008 ===");
    println!("  k_eff = {:.5} ± {:.5}", result.k_mean, result.k_std);
    println!("  ICSBEP benchmark model (critical) = 1.0000 ± ~0.006");
    println!(
        "  Δk from the benchmark = {:+.0} ± {:.0} pcm",
        (result.k_mean - 1.0) * 1.0e5,
        result.k_std * 1.0e5
    );

    if six_factors {
        let src2 = SourceBox {
            lower: Position::new(-R_CORE, -R_CORE, Z_LO),
            upper: Position::new(R_CORE, R_CORE, Z_HI),
        };
        report_six_factors(
            &geom,
            &materials,
            &nuclides,
            &volume_fraction,
            &settings,
            src2,
        );
    }

    if clad_bound {
        eprintln!("\n  bounding the clad omission: omitted density re-added as Mn-55 …");
        let (bounded, _) = build_materials(&spec, &slots, &omitted, true);
        let geom2 = build_geometry(&bounded, true);
        let src2 = SourceBox {
            lower: Position::new(-R_CORE, -R_CORE, Z_LO),
            upper: Position::new(R_CORE, R_CORE, Z_HI),
        };
        let r2 = run_keff_csg(&geom2, &bounded, &nuclides, src2, &settings, None);
        println!(
            "\n  clad-omission bound: k = {:.5} ± {:.5}, Δ = {:+.0} ± {:.0} pcm",
            r2.k_mean,
            r2.k_std,
            (r2.k_mean - result.k_mean) * 1.0e5,
            (r2.k_std.powi(2) + result.k_std.powi(2)).sqrt() * 1.0e5
        );
        println!(
            "  (every omitted clad nuclide replaced by an equal atom density of Mn-55,\n   \
             which absorbs more thermal neutrons than any of them — so this is an\n   \
             upper bound on the omission, measured rather than asserted)"
        );
    }

    println!(
        "\n  This is the discriminator: the one benchmark whose fuel is both thermal\n  \
         and strongly self-shielded in the U-238 resolved resonances. Measured\n  \
         2026-09-11 at 10 000 × [250 + 400]: k = 1.02950 ± 0.00061, i.e.\n  \
         +2950 ± 61 pcm — against +4004 pcm on the FHR pebble, and against the\n  \
         +3200 pcm predicted by carrying that pebble's 11 % resonance-integral\n  \
         deficit onto p ≈ 0.75. The residual is this code's self-shielded U-238\n  \
         resonance absorption, reproduced on a measured critical experiment; it\n  \
         is not the pebble's reference deck. NOTE: the resonance attribution is\n  \
         REFUTED by --case 2 and --case 8; see the module docs."
    );
}

fn arg_usize(args: &[String], flag: &str) -> Option<usize> {
    let i = args.iter().position(|a| a == flag)?;
    args.get(i + 1)?.parse().ok()
}

// ---------------------------------------------------------------------------
// A minimal XML reader.
//
// The two model files use a strict subset of XML: elements with quoted
// attributes, no namespaces, no entities, no CDATA, and text content that is
// only whitespace-separated numbers. Parsing them here — rather than taking a
// dependency, or transcribing the numbers into Rust — keeps the specification
// the single source of truth while adding nothing to the crate's dependency
// graph.
// ---------------------------------------------------------------------------

/// Every `<tag …>` element in `xml`, as `(attributes, body)`. `body` is empty
/// for a self-closing tag. Matching requires a delimiter after the tag name, so
/// `<material` does not match `<materials`.
fn elements<'a>(xml: &'a str, tag: &str) -> Vec<(&'a str, &'a str)> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = Vec::new();
    let mut i = 0usize;
    while let Some(p) = xml[i..].find(&open) {
        let start = i + p;
        let after = start + open.len();
        let next = xml[after..].chars().next().unwrap_or('>');
        if !(next.is_whitespace() || next == '>' || next == '/') {
            i = after;
            continue;
        }
        let gt = start
            + xml[start..]
                .find('>')
                .unwrap_or_else(|| panic!("unterminated <{tag}>"));
        let raw = &xml[after..gt];
        let self_closing = raw.trim_end().ends_with('/');
        let attrs = raw.trim_end().trim_end_matches('/');
        let body = if self_closing {
            ""
        } else {
            let rest = &xml[gt + 1..];
            let e = rest
                .find(&close)
                .unwrap_or_else(|| panic!("unterminated <{tag}> element"));
            &rest[..e]
        };
        out.push((attrs, body));
        i = gt + 1;
    }
    out
}

/// The value of attribute `name` in an attribute string, or `None`.
fn attr<'a>(attrs: &'a str, name: &str) -> Option<&'a str> {
    let mut i = 0usize;
    while let Some(p) = attrs[i..].find(name) {
        let s = i + p;
        let ok_before = s == 0 || attrs[..s].ends_with(char::is_whitespace);
        let rest = attrs[s + name.len()..].trim_start();
        if ok_before && rest.starts_with('=') {
            let v = rest[1..].trim_start();
            let q = v.chars().next()?;
            if q == '"' || q == '\'' {
                let end = v[1..].find(q)? + 1;
                return Some(&v[1..end]);
            }
        }
        i = s + name.len();
    }
    None
}

/// Strip `<!-- … -->`. The geometry file comments out a surface and annotates
/// every other one; a comment body must never be read as markup.
fn strip_comments(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut rest = xml;
    while let Some(p) = rest.find("<!--") {
        out.push_str(&rest[..p]);
        match rest[p..].find("-->") {
            Some(e) => rest = &rest[p + e + 3..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

fn num<T: std::str::FromStr>(s: &str, what: &str) -> T {
    s.trim()
        .parse()
        .unwrap_or_else(|_| panic!("{what}: cannot parse {s:?}"))
}

// ---------------------------------------------------------------------------
// Materials
// ---------------------------------------------------------------------------

struct MaterialSpec {
    id: i32,
    name: String,
    nuclides: Vec<(String, f64)>,
    sab: Option<String>,
}

fn parse_materials(xml: &str) -> Vec<MaterialSpec> {
    let xml = strip_comments(xml);
    elements(&xml, "material")
        .into_iter()
        .map(|(a, body)| MaterialSpec {
            id: num(attr(a, "id").expect("material id"), "material id"),
            name: attr(a, "name").unwrap_or("").to_string(),
            nuclides: elements(body, "nuclide")
                .into_iter()
                .map(|(na, _)| {
                    (
                        attr(na, "name").expect("nuclide name").to_string(),
                        num(attr(na, "ao").expect("nuclide ao"), "nuclide ao"),
                    )
                })
                .collect(),
            sab: elements(body, "sab")
                .first()
                .and_then(|(sa, _)| attr(sa, "name"))
                .map(str::to_string),
        })
        .collect()
}

/// Reconstruct every nuclide the model asks for that this environment has a
/// tape for. Returns the nuclide array, name → slot, and the omitted set.
/// Which optional resonance-treatment physics to switch on, for pricing it.
///
/// Both default to **off**, matching every result recorded for this case
/// before 2026-09-16 — so a run without flags reproduces the baseline and the
/// difference is attributable to the flag alone.
#[derive(Debug, Clone, Copy, Default)]
struct ResonanceOptions {
    /// `--dbrc` — resonance elastic scattering (`Nuclide::with_dbrc`) on the
    /// actinides, below 1 keV (OpenMC's default limit).
    dbrc: bool,
    /// `--urr` — unresolved-resonance probability tables
    /// (`Nuclide::with_urr_probability_tables`) on the actinides.
    urr: bool,
}

/// Which tape set this run uses. **Full by default** — every nuclide the
/// OpenMC material cards name, because correct physics is the default and not
/// an opt-in. `--cheap-nuclides` drops to the 11-nuclide subset for iteration,
/// at the cost of running an incomplete model.
fn tapes() -> &'static [(&'static str, &'static str)] {
    static CHEAP: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let cheap = *CHEAP.get_or_init(|| std::env::args().any(|a| a == "--cheap-nuclides"));
    if cheap {
        TAPES_CHEAP
    } else {
        TAPES
    }
}

/// The tape to load for `name`: the active tier's entry, unless the U-238
/// ablation switch overrides it.
///
/// `OUTRAM_U238_ENDF7=1` swaps U-238 **alone** to ENDF/B-VII.0, every other
/// nuclide held at VIII.0. Resolved here rather than in the `TAPES_*` tables
/// because those are `const` and `std::env::var` is not const-callable, and it
/// wraps [`tapes()`] rather than either table directly so the ablation composes
/// with `--cheap-nuclides` instead of silently ignoring the tier.
///
/// Isolating a single nuclide is the point. The four pooled ICSBEP residuals
/// split by U-238 content, and the two U-238-heavy cases disagree in SIGN. A
/// whole-library swap cannot tell U-238 apart from U-235; this can.
///
/// **The residuals that motivated it were measured BEFORE unresolved-resonance
/// probability tables and DBRC became defaults** (workspace `CLAUDE.md`,
/// "Correct physics is the DEFAULT SETTING"). They were Godiva −55, Jemima
/// −253, HST-009 −38, LCT-008 +165 pcm over 32 seeds. Jemima and LCT-008 have
/// since moved and **a re-measure of all four is owed**; do not quote those two
/// figures as current. The reasoning above is unaffected — it turns on the
/// sign split, not the magnitudes.
fn tape_for(name: &str) -> &'static str {
    if name == "U238" && std::env::var("OUTRAM_U238_ENDF7").is_ok() {
        return "n-092_U_238-ENDF7.0.endf";
    }
    tapes().iter().find(|(n, _)| *n == name).expect("tape").1
}

fn load_nuclides(
    spec: &[MaterialSpec],
    opts: ResonanceOptions,
    ace_scratch: Option<std::path::PathBuf>,
) -> (Vec<Nuclide>, BTreeMap<String, usize>, BTreeMap<String, f64>) {
    let mut wanted: Vec<&str> = Vec::new();
    let mut omitted: BTreeMap<String, f64> = BTreeMap::new();
    for m in spec {
        for (name, ao) in &m.nuclides {
            match tapes().iter().find(|(n, _)| n == name) {
                Some(_) => {
                    if !wanted.contains(&name.as_str()) {
                        wanted.push(name);
                    }
                }
                None => *omitted.entry(name.clone()).or_insert(0.0) += ao,
            }
        }
    }

    eprintln!("Reconstructing nuclides (RECONR + BROADR @ {TEMP_K} K):");
    let t0 = Instant::now();
    let sab_needed = spec.iter().any(|m| m.sab.as_deref() == Some("c_H_in_H2O"));
    let sab = sab_needed.then(|| {
        ThermalScattering::from_endf_file(
            reference_endf("tsl-HinH2O.endf")
                .expect("H(H2O) tape")
                .to_str()
                .expect("path"),
            1, // MAT 1 — H in H2O, ENDF/B-VIII.0
            TEMP_K,
            "c_H_in_H2O",
        )
        .expect("H(H2O) S(a,b)")
    });

    let mut sab = sab;
    let mut nuclides = Vec::new();
    let mut slots = BTreeMap::new();
    for name in wanted {
        let file = tape_for(name);
        let mut n = match &ace_scratch {
            Some(dir) => load_via_ace(name, file, dir),
            None => load(name, file),
        };
        // Resonance treatments, on the actinides only -- they are where the
        // resolved and unresolved resonances that matter live, and building
        // URR tables is expensive enough not to attempt on nuclides with no
        // unresolved range.
        if name.starts_with('U') || name.starts_with("Pu") {
            if opts.dbrc {
                n = n.with_dbrc(1.0e3);
                if n.has_dbrc() {
                    eprintln!("    {name} carries DBRC below 1 keV");
                }
            }
            if opts.urr {
                let p = reference_endf(file).expect("tape");
                let tape = njoy_outram_park_fork::endf::tape::Tape::read_file(&p).expect("tape");
                let mat = tape.materials()[0];
                let t0 = Instant::now();
                n = n
                    .with_urr_probability_tables(&tape, mat, TEMP_K, 20, 16, 2000)
                    .expect("PURR");
                if let Some((lo, hi)) = n.urr_range_ev() {
                    eprintln!(
                        "    {name} carries URR probability tables over [{lo:.3e}, {hi:.3e}] eV \
                         ({:.1?})",
                        t0.elapsed()
                    );
                }
            }
        }
        if name == "H1" {
            if let Some(s) = sab.take() {
                n = n.with_thermal_scattering(s);
                eprintln!("    H1 carries c_H_in_H2O");
            }
        }
        slots.insert(name.to_string(), nuclides.len());
        nuclides.push(n);
    }
    eprintln!("Nuclear data ready in {:.1} s.", t0.elapsed().as_secs_f64());
    (nuclides, slots, omitted)
}

/// The ACE temperature in MeV, for the ACE route's ZAID header.
const KT_MEV: f64 = 8.617_333_262e-5 * TEMP_K * 1.0e-6;

/// Build this nuclide the long way round: reconstruct, broaden, **write ACE,
/// read it back**, and construct from that (`--via-ace`).
///
/// # Why the round trip rather than reading a pre-built library
///
/// There is no ACE library committed for this tier — the `reference-data/ace`
/// submodule carries uranium only, and LEU-COMP-THERM-008 is a thermal
/// lattice in water that needs H-1 and O-16. Generating in process is
/// therefore the only way to exercise this workspace's **own** ACER on the
/// benchmark, which is the point: it measures what our ACE writer and reader
/// cost and whether transport through them lands on the same `k`.
///
/// The assembly order lives in `njoy_outram_park_fork::acer::build_full` so
/// this cannot drift from `lct008_ace_roundtrip.rs` or `njoy`'s own
/// `write_ace.rs`.
///
/// The file is deleted as soon as it is read: U-235 alone is ~256 MB of
/// Type-1 ASCII, and a run of this tier writes well over a gigabyte in total.
fn load_via_ace(name: &str, file: &str, scratch: &std::path::Path) -> Nuclide {
    use njoy_outram_park_fork::broadr::broaden_result;
    use njoy_outram_park_fork::endf::tape::Tape;
    use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};

    let p = reference_endf(file).unwrap_or_else(|| panic!("missing reference tape {file}"));
    eprint!("  via ACE {name:<6} … ");
    let t0 = Instant::now();
    let tape = Tape::read_file(&p).unwrap_or_else(|e| panic!("tape({name}): {e}"));
    let mat = tape.materials()[0];
    let recon0 = reconr(
        &tape,
        &ReconrConfig { mat, tolerance: 1.0e-3, temperature: 0.0 },
    )
    .unwrap_or_else(|e| panic!("RECONR({name}): {e}"));
    let recon = broaden_result(&recon0, TEMP_K);
    let ace = njoy_outram_park_fork::acer::build_full(&tape, mat, &recon, KT_MEV, 0)
        .unwrap_or_else(|e| panic!("assemble ACE({name}): {e}"));

    let out = scratch.join(format!("{name}.ace"));
    ace.write_type1(&out)
        .unwrap_or_else(|e| panic!("write ACE({name}): {e}"));
    let bytes = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
    let raw = njoy_outram_park_fork::acer::read::read(&out)
        .unwrap_or_else(|e| panic!("read ACE({name}): {e}"));
    let n = Nuclide::from_ace(&raw, name).unwrap_or_else(|e| panic!("from_ace({name}): {e}"));
    let _ = std::fs::remove_file(&out);
    eprintln!("{:.1?}  ({:.0} MB written, read back, removed)", t0.elapsed(), bytes as f64 / 1.0e6);
    n
}

fn load(name: &str, file: &str) -> Nuclide {
    let p = reference_endf(file).unwrap_or_else(|| panic!("missing reference tape {file}"));
    eprint!("  reconstructing {name:<6} … ");
    let t0 = Instant::now();
    let n = Nuclide::from_endf_file(&p, name, TEMP_K, 1.0e-3)
        .unwrap_or_else(|e| panic!("from_endf_file({}): {e}", p.display()));
    eprintln!("{:.1?}", t0.elapsed());
    n
}

/// Build the transport materials, in the model's own order. Returns the
/// materials and the index of the clad. With `bound_omission`, the omitted
/// clad density is re-added as Mn-55 (see the module docs).
fn build_materials(
    spec: &[MaterialSpec],
    slots: &BTreeMap<String, usize>,
    omitted: &BTreeMap<String, f64>,
    bound_omission: bool,
) -> (Vec<Material>, usize) {
    let mut clad_idx = 0usize;
    let materials = spec
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let mut components: Vec<NuclideComponent> = m
                .nuclides
                .iter()
                .filter_map(|(name, ao)| {
                    slots.get(name).map(|&nuclide_idx| NuclideComponent {
                        nuclide_idx,
                        atom_density: *ao,
                    })
                })
                .collect();
            if m.name.contains("Aluminum") || m.name.contains("Cladding") {
                clad_idx = i;
                if bound_omission {
                    let extra: f64 = m
                        .nuclides
                        .iter()
                        .filter(|(n, _)| omitted.contains_key(n))
                        .map(|(_, ao)| *ao)
                        .sum();
                    let mn = slots["Mn55"];
                    for c in components.iter_mut() {
                        if c.nuclide_idx == mn {
                            c.atom_density += extra;
                        }
                    }
                }
            }
            Material {
                id: m.id,
                name: m.name.clone(),
                temperature: TEMP_K,
                components,
            }
        })
        .collect();
    (materials, clad_idx)
}

/// How optically thick the fuel pellet is where it matters — measured from this
/// run's own reconstruction, not from a recalled cross section.
///
/// The claim this benchmark rests on is that its fuel is *strongly self-shielded*
/// in the U-238 resolved resonances, which is the one regime the other three
/// reproduced benchmarks never enter. That claim is quantitative, so it is
/// measured: `Σ_t` of the fuel at the peak of the 6.67 eV resonance, scanned on
/// a fine grid across the peak, times the pellet diameter.
fn report_self_shielding(fuel: &Material, nuclides: &[Nuclide]) {
    let (mut e_peak, mut s_peak) = (0.0_f64, 0.0_f64);
    // 6.0-7.5 eV at 0.5 meV covers the resonance (Γ ≈ 25 meV) many times over.
    let mut e = 6.0_f64;
    while e <= 7.5 {
        let s = fuel.macro_xs_total(e, nuclides);
        if s > s_peak {
            s_peak = s;
            e_peak = e;
        }
        e += 5.0e-4;
    }
    let s_off = fuel.macro_xs_total(5.0, nuclides);
    eprintln!(
        "  fuel self-shielding: Σ_t peaks at {:.1} cm⁻¹ at {:.3} eV ({:.0}× the {:.2} cm⁻¹ \
         at 5 eV);\n    the {:.3} cm pellet is {:.0} mean free paths across there",
        s_peak,
        e_peak,
        s_peak / s_off,
        s_off,
        2.0 * R_FUEL,
        2.0 * R_FUEL * s_peak,
    );
}

fn report_omissions(spec: &[MaterialSpec], omitted: &BTreeMap<String, f64>) {
    if omitted.is_empty() {
        eprintln!("  every nuclide in the model has a tape — nothing omitted.");
        return;
    }
    let names: Vec<&str> = omitted.keys().map(String::as_str).collect();
    let total: f64 = omitted.values().sum();
    for m in spec {
        let sum: f64 = m.nuclides.iter().map(|(_, ao)| *ao).sum();
        let miss: f64 = m
            .nuclides
            .iter()
            .filter(|(n, _)| omitted.contains_key(n))
            .map(|(_, ao)| *ao)
            .sum();
        if miss > 0.0 {
            eprintln!(
                "  material {} \"{}\": omitted {:.3e} of {:.3e} /b·cm ({:.2} % of its atoms)",
                m.id,
                m.name,
                miss,
                sum,
                100.0 * miss / sum
            );
        }
    }
    eprintln!(
        "  omitted (no tape in this environment): {} — {:.3e} /b·cm in total",
        names.join(", "),
        total
    );
}

// ---------------------------------------------------------------------------
// Geometry
// ---------------------------------------------------------------------------

/// Core bounding cylinder [cm] — surface 30 in the model, vacuum.
const R_CORE: f64 = 76.200;
/// Core axial extent [cm] — surfaces 10 and 20, both vacuum.
const Z_LO: f64 = -81.662;
const Z_HI: f64 = 81.662;

/// The parsed model, kept so the self-check can re-derive lattice lookups by
/// its own arithmetic instead of trusting `Geometry::locate`.
struct Model {
    /// Core lattice, and each assembly lattice, by id.
    lattices: BTreeMap<i32, RectLattice>,
    /// Universe id → the lattice id filling it, for the assembly wrapper cells.
    wrapper: BTreeMap<i32, i32>,
    /// Universe id → the *universe* id filling it, for the water-tile wrapper.
    uni_fill: BTreeMap<i32, i32>,
    /// Universe id at each universe slot, in `Geometry.universes` order.
    uni_ids: Vec<i32>,
}

// The parse is done once and stashed, because `build_geometry` and
// `check_geometry` both need it and the model is immutable.
static MODEL: std::sync::OnceLock<Model> = std::sync::OnceLock::new();

fn build_geometry(materials: &[Material], reverse_rows: bool) -> Geometry {
    let xml = strip_comments(geometry_xml());

    // --- surfaces -------------------------------------------------------
    let mut surf_index: BTreeMap<i32, usize> = BTreeMap::new();
    let mut surfaces: Vec<SurfaceKind> = Vec::new();
    for (a, _) in elements(&xml, "surface") {
        let id: i32 = num(attr(a, "id").expect("surface id"), "surface id");
        let ty = attr(a, "type").expect("surface type");
        let c: Vec<f64> = attr(a, "coeffs")
            .expect("surface coeffs")
            .split_whitespace()
            .map(|t| num(t, "surface coeff"))
            .collect();
        let bc = match attr(a, "boundary") {
            Some("vacuum") => BoundaryType::Vacuum,
            Some("reflective") => BoundaryType::Reflective,
            Some(other) => panic!("unhandled boundary {other:?} on surface {id}"),
            None => BoundaryType::Transmissive,
        };
        let s = match ty {
            "z-cylinder" => SurfaceKind::ZCylinder(ZCylinder {
                x0: c[0],
                y0: c[1],
                r: c[2],
                bc,
            }),
            "z-plane" => SurfaceKind::ZPlane(ZPlane { z0: c[0], bc }),
            "sphere" => SurfaceKind::Sphere(Sphere {
                x0: c[0],
                y0: c[1],
                z0: c[2],
                r: c[3],
                bc,
            }),
            other => panic!("unhandled surface type {other:?} on surface {id}"),
        };
        surf_index.insert(id, surfaces.len());
        surfaces.push(s);
    }

    // --- lattices -------------------------------------------------------
    // Universe ids are collected first: a lattice's `<universes>` block names
    // them, and so does every cell's `universe=` attribute.
    let lat_blocks = elements(&xml, "lattice");
    let cell_blocks = elements(&xml, "cell");
    let mut uni_ids: Vec<i32> = Vec::new();
    for (a, _) in &cell_blocks {
        let u: i32 = num(attr(a, "universe").expect("cell universe"), "cell universe");
        if !uni_ids.contains(&u) {
            uni_ids.push(u);
        }
    }
    uni_ids.sort_unstable();
    let uni_index: BTreeMap<i32, usize> =
        uni_ids.iter().enumerate().map(|(i, &u)| (u, i)).collect();

    let mut lat_index: BTreeMap<i32, usize> = BTreeMap::new();
    let mut lattices: Vec<Lattice> = Vec::new();
    let mut model_lattices: BTreeMap<i32, RectLattice> = BTreeMap::new();
    for (a, body) in &lat_blocks {
        let id: i32 = num(attr(a, "id").expect("lattice id"), "lattice id");
        let dim: Vec<usize> = attr(a, "dimension")
            .expect("lattice dimension")
            .split_whitespace()
            .map(|t| num(t, "dimension"))
            .collect();
        assert_eq!(dim.len(), 2, "lattice {id} is not 2-D; not handled here");
        let (nx, ny) = (dim[0], dim[1]);
        let ll: Vec<f64> = block(body, "lower_left");
        let pitch: Vec<f64> = block(body, "pitch");
        let rows: Vec<i32> = block(body, "universes");
        assert_eq!(
            rows.len(),
            nx * ny,
            "lattice {id}: {} entries for a {nx}×{ny} grid",
            rows.len()
        );

        // OpenMC writes lattice rows from the TOP down — the first row is the
        // one at maximum y (`openmc/lattice.py`, `Lattice.universes` docs, and
        // the XML writer's row ordering). This crate's flat index runs with iy
        // increasing along +y. Reversing the rows here is the whole of that
        // conversion, and getting it wrong mirrors the core in y — a defect
        // that produces a perfectly plausible wrong k. `check_geometry` tests
        // the convention against the data itself.
        // Every lattice in this model is centred on its own frame — asserted
        // below — which is what makes the reversal a pure mirror.
        assert!(
            (ll[1] + 0.5 * ny as f64 * pitch[1]).abs() < 1.0e-9,
            "lattice {id} is not centred in y; the mirror argument in \
             `check_geometry` does not apply"
        );
        let mut universes = vec![0usize; nx * ny];
        for row in 0..ny {
            let iy = if reverse_rows { ny - 1 - row } else { row };
            for ix in 0..nx {
                let uid = rows[row * nx + ix];
                universes[nx * iy + ix] = *uni_index
                    .get(&uid)
                    .unwrap_or_else(|| panic!("lattice {id} names unknown universe {uid}"));
            }
        }
        let rect = RectLattice {
            id,
            n: [nx, ny, 1],
            lower_left: Position::new(ll[0], ll[1], 0.0),
            pitch: [pitch[0], pitch[1], 1.0],
            universes,
            outer: None,
        };
        model_lattices.insert(id, rect.clone());
        lat_index.insert(id, lattices.len());
        lattices.push(Lattice::Rect(rect));
    }

    // --- cells ----------------------------------------------------------
    let mat_index: BTreeMap<i32, usize> = materials
        .iter()
        .enumerate()
        .map(|(i, m)| (m.id, i))
        .collect();
    let mut cells: Vec<Cell> = Vec::new();
    let mut universes: Vec<Universe> = uni_ids
        .iter()
        .map(|&id| Universe {
            id,
            cell_indices: Vec::new(),
        })
        .collect();
    let mut wrapper: BTreeMap<i32, i32> = BTreeMap::new();
    let mut uni_fill: BTreeMap<i32, i32> = BTreeMap::new();
    for (a, _) in &cell_blocks {
        let id: i32 = num(attr(a, "id").expect("cell id"), "cell id");
        let uid: i32 = num(attr(a, "universe").expect("cell universe"), "cell universe");
        let region = parse_region(attr(a, "region").expect("cell region"), &surf_index, id);
        let fill = match (attr(a, "material"), attr(a, "fill")) {
            (Some(m), None) => {
                let mid: i32 = num(m, "cell material");
                CellFill::Material(
                    *mat_index
                        .get(&mid)
                        .unwrap_or_else(|| panic!("cell {id} names unknown material {mid}")),
                )
            }
            (None, Some(f)) => {
                let fid: i32 = num(f, "cell fill");
                // A fill id names a lattice if one has that id, else a universe.
                // The two id spaces are disjoint in this model, and that is
                // asserted rather than assumed.
                match (lat_index.get(&fid), uni_index.get(&fid)) {
                    (Some(_), Some(_)) => {
                        panic!("cell {id}: fill {fid} is ambiguous — both a lattice and a universe")
                    }
                    (Some(&l), None) => {
                        wrapper.insert(uid, fid);
                        CellFill::Lattice(l)
                    }
                    (None, Some(&u)) => {
                        uni_fill.insert(uid, fid);
                        CellFill::Universe(u)
                    }
                    (None, None) => panic!("cell {id}: fill {fid} is neither lattice nor universe"),
                }
            }
            (Some(_), Some(_)) => panic!("cell {id} has both a material and a fill"),
            (None, None) => panic!("cell {id} has neither a material nor a fill"),
        };
        let cell = match fill {
            CellFill::Material(m) => Cell::material(id, region, m, TEMP_K),
            other => Cell::fill(id, region, other, Position::ZERO),
        };
        universes[uni_index[&uid]].cell_indices.push(cells.len());
        cells.push(cell);
    }

    let _ = MODEL.set(Model {
        lattices: model_lattices,
        wrapper,
        uni_fill,
        uni_ids: uni_ids.clone(),
    });

    let root = uni_index[&0];
    // Only the canonical build reports; `check_geometry` builds the mirrored
    // variant a second time and there is nothing new to say about it.
    if reverse_rows {
        eprintln!(
            "\n  geometry: {} surfaces, {} cells, {} universes, {} lattices (root = universe 0)",
            surfaces.len(),
            cells.len(),
            universes.len(),
            lattices.len()
        );
    }
    Geometry {
        surfaces,
        cells,
        universes,
        lattices,
        root_universe: root,
    }
}

/// Whitespace-separated numbers inside `<tag> … </tag>`.
fn block<T: std::str::FromStr>(body: &str, tag: &str) -> Vec<T> {
    elements(body, tag)
        .first()
        .unwrap_or_else(|| panic!("missing <{tag}>"))
        .1
        .split_whitespace()
        .map(|t| num(t, tag))
        .collect()
}

/// A region in these files is a space-separated list of signed surface ids, all
/// intersected — no unions, complements or parentheses appear. Anything else is
/// rejected loudly rather than silently mis-parsed.
fn parse_region(text: &str, surf_index: &BTreeMap<i32, usize>, cell_id: i32) -> Vec<RegionToken> {
    let mut tokens = Vec::new();
    for t in text.split_whitespace() {
        assert!(
            t.chars()
                .all(|c| c.is_ascii_digit() || c == '-' || c == '+'),
            "cell {cell_id}: region token {t:?} is not a signed surface id \
             (unions/complements are not handled)"
        );
        let signed: i32 = num(t, "region surface");
        let idx = *surf_index
            .get(&signed.abs())
            .unwrap_or_else(|| panic!("cell {cell_id}: region names unknown surface {signed}"));
        let sense = if signed < 0 {
            HalfSpaceSense::Inside
        } else {
            HalfSpaceSense::Outside
        };
        let first = tokens.is_empty();
        tokens.push(RegionToken::HalfSpace {
            surface_idx: idx,
            sense,
        });
        if !first {
            tokens.push(RegionToken::Intersection);
        }
    }
    assert!(!tokens.is_empty(), "cell {cell_id}: empty region");
    tokens
}

// ---------------------------------------------------------------------------
// Self-check
// ---------------------------------------------------------------------------

/// Check the CSG descent against hand-written arithmetic at 200 000 points, and
/// settle the lattice row convention.
///
/// This is the first nested-lattice model in this crate, and two of its steps
/// are exactly the kind that produce a plausible wrong answer rather than a
/// crash: the two-level tile index arithmetic, and OpenMC's top-down
/// `<universes>` row order against this crate's bottom-up flat index. The pin
/// *maps* are data read from the same file either way, so what is verified here
/// is the machinery around them.
///
/// 1. **The row convention cannot matter for this model, and that is checked,
///    not assumed.** Every lattice here is centred on its own frame
///    (`lower_left = −½·n·pitch`, asserted while parsing) and every lattice
///    frame is itself centred — the core lattice on the origin, each assembly
///    on its core tile's centre. Reversing a centred lattice's rows therefore
///    mirrors its contents about its own mid-plane, and composing that at both
///    levels is exactly a **global mirror of the model about `y = 0`**. The only
///    other geometry is a z-cylinder on the axis and two z-planes, all
///    y-symmetric, so the two conventions give congruent models and identical
///    `k`. The check builds the model both ways and requires
///    `locate(x, y, z)` under one to equal `locate(x, −y, z)` under the other at
///    every sampled point — which both proves the mirror claim and, because the
///    two builds differ in every lattice, exercises the index arithmetic
///    against an independent transformation.
///
///    (An earlier version of this check tried to pick the convention by
///    measuring which one gave a smoother circular core boundary. It returned
///    exactly the same raggedness for both — 1.944 cm — which is not a weak
///    signal but the mirror symmetry above showing up: no metric invariant
///    under reflection can distinguish them. The convention used is still
///    OpenMC's documented top-down row order; it simply has no effect here.)
///
/// 2. **Point agreement.** For each sampled point, an independent predicate
///    computes the core tile, the assembly tile and the pin region by its own
///    arithmetic — with the pin radii read by hand off the model's surface
///    cards — and the resulting material must equal the one
///    `Geometry::locate` descends to, including `None` outside the vacuum
///    boundary.
fn check_geometry(geom: &Geometry, materials: &[Material]) -> Vec<f64> {
    let model = MODEL.get().expect("model");
    let core = model.lattices.get(&99).expect("core lattice 99");

    // --- 1. the row convention is a global y-mirror -----------------------
    {
        let mirrored = build_geometry(materials, false);
        let mut seed = 777_777_u64;
        let mut prn = move || {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((seed >> 11) as f64) / ((1u64 << 53) as f64)
        };
        const M: usize = 50_000;
        let u = Direction::new(0.0, 0.0, 1.0);
        for _ in 0..M {
            let (x, y, z) = (
                (2.0 * prn() - 1.0) * R_CORE * 1.02,
                (2.0 * prn() - 1.0) * R_CORE * 1.02,
                Z_LO - 1.0 + prn() * (Z_HI - Z_LO + 2.0),
            );
            let a = geom
                .locate(Position::new(x, y, z), u, SurfaceToken::NONE)
                .and_then(|p| p.material);
            let b = mirrored
                .locate(Position::new(x, -y, z), u, SurfaceToken::NONE)
                .and_then(|p| p.material);
            assert_eq!(
                a, b,
                "the two row conventions are not mirror images at ({x}, {y}, {z}): {a:?} vs {b:?}"
            );
        }
        eprintln!(
            "  lattice row convention: the two orders are exact y-mirrors of each other \
             ({M} points), so k cannot depend on the choice"
        );
    }

    // --- 1. point agreement ----------------------------------------------
    // Materials are listed as ids 1..n in order (asserted in `main`), so a
    // model id maps to a slot by subtracting one. `PIN_SHELLS` is the hand-read
    // transcription of each pin universe.
    let slot = |material_id: i32| -> usize { (material_id - 1) as usize };
    let shells = |uid: i32| -> &'static (i32, &'static [(f64, i32)], i32) {
        PIN_SHELLS
            .iter()
            .find(|(u, _, _)| *u == uid)
            .unwrap_or_else(|| panic!("pin universe {uid} is not in PIN_SHELLS"))
    };
    let expect = |p: Position| -> Option<usize> {
        if p.z <= Z_LO || p.z >= Z_HI || p.x * p.x + p.y * p.y >= R_CORE * R_CORE {
            return None;
        }
        // core tile
        let cx = ((p.x - core.lower_left.x) / core.pitch[0]).floor();
        let cy = ((p.y - core.lower_left.y) / core.pitch[1]).floor();
        assert!(
            cx >= 0.0 && cy >= 0.0 && cx < core.n[0] as f64 && cy < core.n[1] as f64,
            "core cylinder reaches outside the 7×7 lattice at {p:?}"
        );
        let (cx, cy) = (cx as usize, cy as usize);
        let uid = model.uni_ids[core.universes[core.n[0] * cy + cx]];
        // A core tile is either an assembly (its wrapper universe is filled by
        // a lattice) or the all-water tile (filled by pin universe 1).
        let lid = match (model.wrapper.get(&uid), model.uni_fill.get(&uid)) {
            (Some(&l), _) => l,
            (None, Some(&1)) => return Some(slot(1)), // the all-water tile
            _ => panic!("core tile {cx},{cy} is universe {uid}, which fills nothing known"),
        };
        let asm = &model.lattices[&lid];
        let x0 = core.lower_left.x + (cx as f64 + 0.5) * core.pitch[0];
        let y0 = core.lower_left.y + (cy as f64 + 0.5) * core.pitch[1];
        let (lx, ly) = (p.x - x0, p.y - y0);
        let ax = ((lx - asm.lower_left.x) / asm.pitch[0]).floor();
        let ay = ((ly - asm.lower_left.y) / asm.pitch[1]).floor();
        assert!(
            ax >= 0.0 && ay >= 0.0 && ax < asm.n[0] as f64 && ay < asm.n[1] as f64,
            "assembly {lid} does not tile its core cell at {p:?}"
        );
        let (ax, ay) = (ax as usize, ay as usize);
        let pin_uid = model.uni_ids[asm.universes[asm.n[0] * ay + ax]];
        let (_, rings, outside_mat) = shells(pin_uid);
        // pin universe 2: fuel / clad / water by radius about the pin centre
        let px = asm.lower_left.x + (ax as f64 + 0.5) * asm.pitch[0];
        let py = asm.lower_left.y + (ay as f64 + 0.5) * asm.pitch[1];
        let (dx, dy) = (lx - px, ly - py);
        let r = (dx * dx + dy * dy).sqrt();
        for &(r_out, mat_id) in rings.iter() {
            if r < r_out {
                return Some(slot(mat_id));
            }
        }
        Some(slot(*outside_mat))
    };

    let mut seed = 20_260_911_u64;
    let mut prn = move || {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((seed >> 11) as f64) / ((1u64 << 53) as f64)
    };
    const N: usize = 200_000;
    let (mut inside, mut outside) = (0usize, 0usize);
    let mut tally = vec![0usize; materials.len()];
    for _ in 0..N {
        let p = Position::new(
            (2.0 * prn() - 1.0) * R_CORE * 1.05,
            (2.0 * prn() - 1.0) * R_CORE * 1.05,
            Z_LO - 2.0 + prn() * (Z_HI - Z_LO + 4.0),
        );
        let want = expect(p);
        let got = geom
            .locate(p, Direction::new(0.0, 0.0, 1.0), SurfaceToken::NONE)
            .and_then(|path| path.material);
        assert_eq!(
            got, want,
            "CSG disagrees with the model at {p:?}: found material {got:?}, model says {want:?}"
        );
        match want {
            Some(m) => {
                inside += 1;
                tally[m] += 1;
            }
            None => outside += 1,
        }
    }
    let shares: Vec<f64> = tally.iter().map(|&t| t as f64 / inside as f64).collect();
    eprintln!(
        "  geometry check: {N} points, {inside} inside / {outside} outside; volume shares {}",
        materials
            .iter()
            .zip(&shares)
            .map(|(m, v)| format!("{} {:.1} %", m.name, 100.0 * v))
            .collect::<Vec<_>>()
            .join(" / ")
    );
    assert!(
        inside > N / 10 && outside > N / 100 && tally.iter().all(|&t| t > 0),
        "degenerate sampling"
    );
    shares
}

/// Where the 3 % actually sits, measured rather than inferred.
///
/// The LEU-COMP-THERM-008 residual has so far been *attributed* to U-238
/// resonance escape by an argument from benchmark coverage — this case is the
/// only one that is thermal AND U-238-dominated AND lumped — never by measuring
/// this case's own decomposition. This measures it.
///
/// The trick that makes it self-contained: **`k` must be 1.0000**, so whichever
/// factor is wrong is wrong by the 3 % the eigenvalue is out. `η` and `f` are
/// then checked against a one-group calculation built from *this run's own*
/// reconstructed cross sections at 0.0253 eV and the point-sampled volume
/// fractions — quantities that barely depend on transport at all — which leaves
/// `p·ε` as the residue by elimination rather than by assumption.
///
/// The one-group `f` is a **flat-flux** estimate and so an upper bound: it
/// ignores the thermal disadvantage factor, which depresses the flux in the
/// strongly absorbing fuel. `f_measured / f_flat` is therefore the disadvantage
/// factor this code produces, and for a pin lattice of this pitch it should sit
/// a few percent below 1.
fn report_six_factors(
    geom: &Geometry,
    materials: &[Material],
    nuclides: &[Nuclide],
    volume_fraction: &[f64],
    settings: &KeffSettings,
    src: SourceBox,
) {
    let cfg = ReactorPhysicsConfig {
        keff: settings.clone(),
        source_box: src,
        thermal_cutoff_ev: 0.625,
        ..Default::default()
    };
    let t = Instant::now();
    let rep = match run_keff_reactor_physics(geom, materials, nuclides, &cfg) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("  six-factor run failed: {e:?}");
            return;
        }
    };
    eprintln!("  six-factor transport: {:.1} s", t.elapsed().as_secs_f64());

    let sf = &rep.six_factors;
    let (eta2, f2, p2, eps2) = sf.two_group_openmc_convention();
    println!("\n=== LEU-COMP-THERM-008 six factors ===");
    println!("  k_eff = {:.5} ± {:.5}", rep.keff.k_mean, rep.keff.k_std);
    println!(
        "  3-group: eta {:.4}  f {:.4}  p {:.4}  eps {:.4}  P_FNL {:.4}  P_TNL {:.4}",
        sf.eta.mean, sf.f.mean, sf.p.mean, sf.epsilon.mean, sf.p_fnl.mean, sf.p_tnl.mean
    );
    println!("  2-group: eta {eta2:.4}  f {f2:.4}  p {p2:.4}  eps {eps2:.4}");
    println!(
        "  product (k_4f) {:.5}, consistency gap {:+.2} %, leakage {:.4}",
        sf.k_from_factors.mean,
        100.0 * rep.consistency_gap,
        rep.leakage_total.mean
    );

    // One-group η and f from this run's own cross sections, flat flux.
    const E_TH: f64 = 0.0253;
    let vol = volume_fraction;
    let mut sigma_a = vec![0.0_f64; materials.len()];
    let mut nu_sigma_f = vec![0.0_f64; materials.len()];
    for (i, m) in materials.iter().enumerate() {
        let x = m.macro_xs(E_TH, nuclides);
        sigma_a[i] = x.absorption;
        nu_sigma_f[i] = x.nu_fission;
    }
    // materials are ordered water(0), fuel(1), clad(2) — asserted in main.
    let denom: f64 = (0..materials.len()).map(|i| vol[i] * sigma_a[i]).sum();
    let eta_flat = nu_sigma_f[1] / sigma_a[1];
    let f_flat = vol[1] * sigma_a[1] / denom;
    println!(
        "\n  one-group check at 0.0253 eV, from this run's own sigma and the \n           point-sampled volume fractions (water {:.3} / fuel {:.3} / clad {:.3}):",
        vol[0], vol[1], vol[2]
    );
    println!(
        "    Sigma_a  water {:.5}  fuel {:.5}  clad {:.5} cm^-1;  nu*Sigma_f fuel {:.5}",
        sigma_a[0], sigma_a[1], sigma_a[2], nu_sigma_f[1]
    );
    println!(
        "    eta_flat {eta_flat:.4} vs measured {:.4}  ({:+.2} %)",
        sf.eta.mean,
        100.0 * (sf.eta.mean / eta_flat - 1.0)
    );
    println!(
        "    f_flat   {f_flat:.4} vs measured {:.4}  (disadvantage factor {:.4})",
        sf.f.mean,
        sf.f.mean / f_flat
    );

    // What p would have to be for this configuration to be critical, given the
    // OTHER factors as measured. k must be 1.0000.
    let others = sf.eta.mean * sf.f.mean * sf.epsilon.mean * sf.p_fnl.mean * sf.p_tnl.mean;
    if others > 0.0 {
        let p_required = 1.0 / others;
        println!(
            "\n    p measured {:.4}; p required for k = 1.0000 with the other five \n                 factors as measured: {p_required:.4}  ({:+.2} %)",
            sf.p.mean,
            100.0 * (sf.p.mean / p_required - 1.0)
        );
        println!(
            "    (the six factors carry a {:+.2} % consistency gap against k itself, so \n                  read this as a localisation, not a balance to the last digit)",
            100.0 * rep.consistency_gap
        );
    }
}

/// Pin radii [cm] — surfaces 1 and 2 in the model.
const R_FUEL: f64 = 0.514858;
const R_CLAD: f64 = 0.602996;
