// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! # HTR-10 code-to-code verification against the RMC paper
//!
//! **Reference.** Li Wanlin, Yu Ganglin & Wei Chunlin, *"Research on Benchmark
//! Calculation and Analysis of HTR-10 with RMC Code"*, 7th International Topical
//! Meeting on High Temperature Reactor Technology (HTR 2014), Weihai, China,
//! 27-31 October 2014.
//!
//! This is the coupling layer's job: the paper specifies one reactor, and both
//! the Monte Carlo and the deterministic ends of this suite should reproduce it
//! from **the same** geometry and composition. That shared model lives here so
//! the two ends cannot drift apart and quietly turn a composition difference
//! into an apparent transport difference.
//!
//! # Verification status: TENTATIVE, and what is still open (2026-09-25)
//!
//! The twelve-height `k_eff` curve IS now computed against RMC (the "NOT
//! verifiable now" section below predates the TECDOC reflector model). At
//! `0454c1ad1b` (one-ball bed, superseded -- see CURRENT NUMBERS below), 10 000 x [5 + 135], one seed per height, the residual is
//! `-896 +/- 30` pcm on ENDF/B-VIII.0 and `+288 +/- 31` pcm on ENDF/B-VII.0
//! (the reference's library), and **drifts `+7` pcm/cm with loading height in
//! every arm** (gh:#218, results posted there). Treat those numbers as tentative
//! until the items below are priced or fixed. Each is an issue; none has been
//! measured unless it says so.
//!
//! **CURRENT NUMBERS (2026-09-25, two-ball cell, ENDF/B-VIII.0 + 5 thermal
//! laws, 10 000 x [5 + 135], 14 rings, 3 seeds each):** residual against RMC
//! `+1626 +/- 50` pcm at 97.98 cm, `+1646 +/- 62` at 122.47 cm, `+1958 +/- 33`
//! at 200.86 cm; slope `+3.41 +/- 0.54` pcm/cm. The model moved from BELOW
//! RMC to ABOVE it (+2231 to +2413 pcm against the one-ball model). **Nothing
//! was adjusted towards the reference; the residual is an open question**,
//! and the simplifications listed below that push `k` up are the first
//! candidates to investigate -- not to tune. Every earlier number in this
//! section predates the two-ball cell. Methodology, the twelve-height curve
//! and the sampling evidence: the V&V record
//! (`crates/outram-mc-libs/verification_and_validation/htr10_rmc/README.md`,
//! "The two-ball prism cell").
//!
//! **SUPERSEDED as "current" 2026-09-26:** those residuals predate the
//! explicit reflector (PR #327). On it (fast single-seed runs, 2000 x
//! [30 + 70], rod-steel Ni -> Fe and Fe-57 -> Fe-56 stated as assumptions),
//! the residual at the critical loading is ~~`-2365` pcm on ENDF/B-VIII.0 and
//! `-922` pcm on VII.0~~ **`-2726` pcm on ENDF/B-VIII.0 and `-1283` pcm on
//! VII.0** (CORRECTED 2026-09-27, gh:#333: matched on the paper's whole-ball
//! height; ~~`n_axial = 25` IS the paper's 123.576 cm loading~~), roughly -4000 pcm
//! from the numbers above; the drift is still there (~~`+10.2`~~ `+13.2 +/- 4.0`
//! pcm/cm on VIII.0). ~~Every "height-matched" residual in this section (and in
//! #218) used the volume-equivalent height and is low by 165-480 pcm.~~
//! **CORRECTED 2026-10-01 from Şeker & Çolak (2003) Table 3 (gh:#333):** the
//! reference rows hold `1346 N + 733` balls, which at 0.61 is **H − 0.55 cm**
//! of volume-equivalent bed (0.58 cm at N = 9, 0.48 cm at N = 20), not
//! H − 6 cm. `n_axial = 2N + 1` holds `(2N+1) x 4.899` cm, which is
//! **0.52–0.63 cm (≈ 60–75 balls) less** than the row it was matched to. So:
//! - the whole-ball-height residuals above are **low**;
//! - the earlier volume-equivalent ones were **high**;
//! - in both cases by roughly **70–250 pcm**. That magnitude is estimated from
//!   the reference's own slope, not measured, and it assumes Li's RMC model
//!   has Şeker's inventory.
//!
//! The correct match is by **ball count**. Record:
//! `crates/outram-mc-libs/verification_and_validation/htr10_rmc/fast_ablation_2026_09_26.md`.
//!
//! **Note 2026-10-01 (gh:#428).** Every residual in this section was computed
//! on a bed that is now **withdrawn**: the one-ball `core_model::assemble`,
//! then the two-ball `bed::TwoBallBed`. Both cut pebbles, and both now panic
//! if asked for. The default bed is Şeker & Çolak (2003)'s 13-ball cell
//! (`bed::SekerBed`, gh:#472), and it is compared to RMC at equal ball count
//! ([`rmc_keff_at_ball_count`]). None of the numbers above was re-measured on
//! it for this note, so none of them describes the current default.
//!
//! **Model defects, production path (`assemble_explicit_triso`):**
//! - ~~gh:#309 — one ball per hex tile clips the pebble shell: 4.76 % of all
//!   core carbon is missing while the heavy metal is exact (C/U low). Sign on
//!   `k` not predicted.~~ **FIXED 2026-09-25** by the paper's two-ball prism
//!   cell (`bed::TwoBallBed`): whole 6 cm pebbles, sampled filling fraction
//!   0.6096-0.6097, graphite restored (envelope graphite 0.4990 -> 0.5244 at
//!   122.47 cm), kernel fraction 0.998-1.000 of the paper-implied value.
//!   Worth `+2231` to `+2413` pcm (3-seed means, 98-201 cm): **k goes UP**.
//!   **SUPERSEDED 2026-10-01 (gh:#472):** the two-ball cell still cut the
//!   balls crossing the side wall and the bed top, and it is withdrawn
//!   (`OUTRAM_HTR10_TWO_BALL_CELL` now panics in `assemble_explicit_triso`).
//!   The default is `bed::SekerBed`, in which every ball is whole and is
//!   rejected at the side wall, the cone and the tube.
//! - ~~gh:#310 — the lattice drops the A-B layer offset, so axially adjacent
//!   pebbles touch and their fuel zones meet~~ **FIXED 2026-09-25**, same
//!   change: A-B stacking restored, minimum centre distance of the BUILT bed
//!   6.2102 cm (`tests::no_two_balls_of_the_built_bed_overlap`).
//! - ~~gh:#311 — only B-10 is placed~~ **FIXED 2026-09-25**: B-11 now goes in
//!   beside B-10 in every material, from the selected library, pinned by
//!   `every_boron_bearing_material_carries_natural_b11`. Its worth is priced
//!   on #311. Every k in this section predates it.
//! - ~~gh:#316 — the built core carried ~1.2 % less heavy metal~~ **FIXED
//!   2026-09-25**: the TRISO count was taken on one grid offset and the
//!   lattice built on another (8340 counted, 8240 built). Both now use one
//!   offset; built == counted is asserted. Resampled: 0.9971 +/- 0.0014 of the
//!   paper-implied kernel fraction (was 0.9875). Worth **+353 +/- 111 pcm**
//!   at 122.47 cm (three paired seeds). Every k in this section predates it.
//! - gh:#218 — the `+7` pcm/cm drift itself. ~~**Cause now evidenced
//!   (2026-09-25):** a shrunk-pebble ablation with no #309 clip and no #310
//!   axial contact (all volume fractions the paper's) changes k by
//!   `-6.88 +/- 1.48` pcm/cm across 98-201 cm, equal and opposite to the
//!   drift.~~ **CORRECTED 2026-09-25 -- NOT evidenced.** The physical fix (the
//!   two-ball cell, real 6 cm pebble) changes the slope by only
//!   `-1.63 +/- 0.94` pcm/cm (1.7 sigma, unresolved) at the same three
//!   heights; the residual still drifts `+3.41 +/- 0.54` pcm/cm (develop:
//!   `+5.04 +/- 0.77`, its per-point sems taken from the pooled seed sd).
//!   The two arms' slope changes differ by `-5.3 +/- 1.8` pcm/cm (3 sigma), so
//!   most of the ablation's `-6.88` came from what else differed in it --
//!   chiefly its 18 % smaller pebble -- not from the clip or the contact
//!   (an inference, not a separate measurement). The drift remains open. Already ruled out: data library, source convergence, cavity,
//!   bottom-reflector mirroring, UO2 law source, B-11, TRISO count; the
//!   pebble construction accounts for at most about a third of it. Open: the
//!   reflector (explicit channels, PR #327), the height convention of the
//!   reference (gh:#333).
//!
//! **Documented simplifications (not defects, each pushes `k` one way):**
//! - ~~every reflector region is TECDOC zone 22, the densest graphite in
//!   Table 4-3, and the boronated zones are not placed — raises `k`;~~
//! - ~~the control-rod boring band is solid zone-22 graphite
//!   (`OUTRAM_HTR10_BORINGS` is off: its core-height composition is unrecorded);~~
//! - ~~the core-height reflector zone map is not placed;~~
//! - rods fully withdrawn (the benchmark's B1 state); one temperature
//!   (300.15 K) everywhere.
//!
//! **CHANGED 2026-09-25 (WIP, branch `claude/htr10-reflector`, NOT yet
//! priced):** the reflector is explicit 3-D geometry
//! ([`reflector_geometry`]): the 20 coolant, 10 control-rod, 3 irradiation
//! and 7 absorber-ball channels at their own positions in solid graphite,
//! the hot gas duct, and every IAEA-TECDOC-1382 Fig. 4.10 zone with the
//! p. 242 corrections for explicit borings. The rods sit in their channels
//! at the withdrawn position with explicit B4C, steel and iron. The
//! discharge tube holds explicit whole graphite balls, with Li (2014)'s
//! rejection of balls crossing the cone or tube. What the specification does
//! not give (channel azimuths, the contents of the absorber-ball and
//! irradiation channels, the internal structure of zones 0-4, 8-16, 19-21,
//! 48, 57) is listed in [`reflector_geometry`]'s module docs, and was
//! settled by the maintainer on 2026-09-27 (gh:#330: 18 degree convention,
//! KLAK and irradiation channels empty; gh:#332: those zones as the TECDOC
//! gives them). ~~**No `k` has been computed on this geometry yet, and it has
//! not yet been drawn**~~ **CORRECTED 2026-09-27:** it has been drawn
//! (`verification_and_validation/htr10_python_plots/`) and first priced at
//! fast statistics
//! (`outram-mc-libs/verification_and_validation/htr10_rmc/fast_ablation_2026_09_26.md`).
//! It is still an AI-drafted model awaiting human review.
//!
//! **Other paths and plumbing:**
//! - gh:#308 — `assemble` (homogenised fuel) lacks the cavity, conus, bricks and
//!   annulus of the production path; do not use it for a `k` comparison. ~~It
//!   feeds `htr10_mgxs_genfoam`.~~ **CORRECTED 2026-10-01:** `assemble` is
//!   withdrawn and panics on entry (it cuts pebbles). `htr10_mgxs_genfoam`
//!   still calls it (checked: `examples/htr10_mgxs_genfoam.rs`), so that
//!   example now panics too.
//! - gh:#313 — `Cell::temperature` is never read by transport and every cell
//!   hardcodes 293.6 K; the material temperature (300.15 K) is what is used.
//! - gh:#312 — the control-rod smeared composition drops the steel sections
//!   (not exercised by the rods-out benchmark).
//!
//! # What this module can verify today, and what it cannot
//!
//! Read this before quoting anything from here. The honest scope is narrower
//! than "reproduce the paper", and the reason is in the paper itself.
//!
//! ## Verifiable now — the model's construction
//!
//! Tables 1 and 2 are **over-determined**: they state quantities that are also
//! derivable from other quantities they state. Every such closure is a genuine
//! code-to-code check that our reconstruction matches theirs, and none of them
//! needs a transport solve. [`GeometryClosure`] carries them.
//!
//! ## ~~NOT verifiable now~~ — the k-eff curve
//!
//! **CORRECTED 2026-09-27:** the blocker described below is gone. The
//! TECDOC-1382 reflector is modelled (Table 4-3 zones, then every boring
//! explicit, draft PR #327), and the curve is computed against RMC at the
//! paper's own heights (gh:#333). See the "CURRENT NUMBERS" and the
//! SUPERSEDED note below and
//! `crates/outram-mc-libs/verification_and_validation/htr10_rmc/`. The
//! original text follows, unchanged, for the record.
//!
//! The paper's Tables 3 and 4 give `k_eff` against fuel-loading height, which is
//! the headline result. **We cannot reproduce it yet, and the blocker is the
//! paper's own**:
//!
//! > *"Modeling details of reflector and structural material are referred to
//! > paper released by IAEA which is listed in reference."*
//!
//! That reference is IAEA-TECDOC-1382. The HTR-10 core is 180 cm across inside
//! roughly a metre of graphite reflector which *"house\[s\] control rods, small
//! absorber balls, helium flow channels, and irradiation channels"* — and that
//! reflector is most of the reason so small a fissile inventory reaches
//! criticality. Without it the eigenvalue is not close, and pretending otherwise
//! would be reporting a number that looks like a comparison and is not one.
//!
//! ## A defect in the reference, found while reading it
//!
//! Tables 3 and 4 are **both captioned "(vacuum)"**, while the text says
//! *"Calculations are performed for vacuum and helium."* Checking them row by
//! row:
//!
//! - the **RMC** column is byte-identical in ~~**11 of 11**~~ **12 of 12**
//!   shared rows;
//! - the **MCNP** column ~~differs in **0 of 11** — that is, in every row~~
//!   **differs in 12 of 12** rows.
//!
//! **CORRECTED 2026-10-01 (gh:#428):** Tables 3 and 4 have twelve rows, not
//! eleven (`the_reference_curve_is_monotonic_and_brackets_criticality` asserts
//! 12), and "0 of 11" inverted the count. Checked against the stored
//! [`MCNP_TABLE3_KEFF_VS_HEIGHT`] and [`MCNP_TABLE4_KEFF_VS_HEIGHT`]: no row
//! is equal.
//!
//! So the paper reports **one** RMC dataset against **two** MCNP results, and
//! one of the two captions is wrong. Consequence for anyone verifying against
//! it: there is a single RMC curve, not a vacuum/helium pair, and an attempt to
//! reproduce two would be chasing an artefact. [`RMC_KEFF_VS_HEIGHT`] carries
//! that single curve.
//!
//! **RESOLVED 2026-10-01 from Şeker & Çolak (2003), NED 222:263, Table 3**
//! (the MCNP paper Li cites): Li's Table 3 MCNP column equals Şeker's
//! **vacuum** column and Li's Table 4 MCNP column equals Şeker's **helium**
//! column, in all 12 rows to 5 d.p. (checked by eye against the stored
//! constants). So Table 4's "(vacuum)" caption is the wrong one. Which medium
//! Li's single RMC curve was run in is still not stated (gh:#333).
//!
//! **This model's coolant is helium by default since 2026-10-01**
//! (maintainer, gh:#426: "use helium, I think it is more accurate"): natural
//! helium at 300.15 K and an assumed 101.33 kPa in every coolant region.
//! Until then `mat::HELIUM` was an empty material, i.e. vacuum, which is now
//! the `OUTRAM_HTR10_VACUUM_COOLANT` ablation (`data::Coolant::Vacuum`).
//!
//! ## How close is close, for this reference
//!
//! The paper's own RMC-vs-MCNP relative differences reach ~0.9 %, and it states
//! plainly that *"model used in this calculation is constructed relatively
//! independently"*. These are indicative code-to-code numbers, **not** a
//! benchmark reference. Agreement to ~500 pcm would be a real success here;
//! agreement to 50 pcm would be suspicious and should prompt a search for a
//! coincidence rather than a celebration.

use outram_mc_libs::prelude::TrisoSpec;

pub mod bed;
pub mod reflector;
pub mod reflector_geometry;
pub mod core_model;
pub mod control_rod;
pub mod data;
pub mod materials;
pub mod plots;
pub mod keff_vs_height;

/// The paper's single RMC `k_eff` curve against fuel-loading height, Tables 3
/// and 4 (`(height_cm, k_eff)`).
///
/// One curve, not two — see the module docs on the duplicated column. ~~The MCNP
/// columns are deliberately **not** carried here: they are a second code's
/// results on a third model, and mixing them in would invite a comparison that
/// is not ours to make.~~ **CHANGED 2026-09-27 (maintainer direction: "save
/// mcnp data too since it's there, just so we have a rough gauge"):** they are
/// now carried separately, in [`MCNP_TABLE3_KEFF_VS_HEIGHT`] and
/// [`MCNP_TABLE4_KEFF_VS_HEIGHT`]. RMC stays the reference.
///
/// Heights are the paper's convention: bottom of the lowest ball to top of the
/// highest, `9.798 N + 6.0` cm (gh:#333).
///
/// **The inventory behind each height (2026-10-01).** These are exactly the
/// heights of Şeker & Çolak (2003), NED 222:263, Table 3 (the MCNP model Li
/// follows), which also gives the ball counts: `1346 N + 733` balls, of which
/// `767 N + 418` are fuel, for N = 9…20. At a 0.61 filling fraction that is
/// **H − 0.55 cm** of volume-equivalent bed (0.58 cm at N = 9, 0.48 cm at
/// N = 20). Match a model to a row **by ball count**, not by height. The
/// experiment's first criticality was 16 890 balls at 123.06 cm; Şeker's
/// N = 12 row holds 16 885. Whether Li's RMC model holds the same count is not
/// stated.
pub const RMC_KEFF_VS_HEIGHT: &[(f64, f64)] = &[
    (94.182, 0.894_693),
    (103.980, 0.937_122),
    (113.778, 0.972_163),
    (123.576, 1.004_288),
    (133.374, 1.030_738),
    (143.172, 1.056_866),
    (152.970, 1.078_448),
    (162.768, 1.097_370),
    (172.566, 1.114_775),
    (182.364, 1.133_878),
    (192.162, 1.147_570),
    (201.960, 1.162_230),
];

/// MCNP `k_eff` against loading height as printed in the paper's **Table 3**
/// (captioned "Critical result 1 (vacuum)"), `(height_cm, k_eff)`.
///
/// **A rough gauge, not a reference.** Li, Yu & Wei describe these as the
/// results *"of MCNP reported in paper listed in reference"*, i.e. MCNP on a
/// different, independently built model (*"model used in this calculation is
/// constructed relatively independently"*), with no uncertainty quoted. Use
/// them to see how far two codes on two models already spread, which bounds
/// how much agreement with RMC alone can mean: the paper's own RMC-MCNP
/// differences reach 0.95 %. Do not fit to them and do not replace RMC with
/// them.
///
/// **Their data library (2026-10-01, gh:#428).** Li's abstract says RMC and
/// MCNP both used *"continuous energy cross section based on ENDF/B-7.0"*. The
/// MCNP numbers are not Li's own runs, though. They equal Şeker & Çolak
/// (2003)'s Table 3 (see the module docs), and Şeker p.265 states:
/// *"ENDF/B-VI continuous energy cross sections are used in calculations for
/// all materials except graphite. Cross sections for graphite are taken from
/// TMCCS library."* So these MCNP columns are **ENDF/B-VI**, with TMCCS
/// graphite, and not VII.0. Only RMC's library is the VII.0 Li states.
///
/// Both Tables 3 and 4 are captioned "(vacuum)" while the text says the
/// calculations were for vacuum *and* helium, so one caption is wrong ~~and it
/// is not known which table is which~~ **CORRECTED 2026-10-01**: Şeker &
/// Çolak (2003) Table 3 shows Table 3 is vacuum and Table 4 is **helium**
/// (see the module docs). They are kept under their table numbers. Transcribed 2026-09-27;
/// `the_mcnp_columns_reproduce_the_papers_relative_differences` re-derives the
/// paper's "Re-diff" column from them.
pub const MCNP_TABLE3_KEFF_VS_HEIGHT: &[(f64, f64)] = &[
    (94.182, 0.888_56),
    (103.980, 0.930_52),
    (113.778, 0.969_98),
    (123.576, 1.003_3),
    (133.374, 1.033_66),
    (143.172, 1.058_4),
    (152.970, 1.085_12),
    (162.768, 1.102_9),
    (172.566, 1.122_37),
    (182.364, 1.139_04),
    (192.162, 1.158_46),
    (201.960, 1.171_93),
];

/// MCNP `k_eff` against loading height from the paper's **Table 4** (also
/// captioned "(vacuum)"; see [`MCNP_TABLE3_KEFF_VS_HEIGHT`] for what these are
/// and are not).
pub const MCNP_TABLE4_KEFF_VS_HEIGHT: &[(f64, f64)] = &[
    (94.182, 0.890_44),
    (103.980, 0.931_44),
    (113.778, 0.969_73),
    (123.576, 1.004_79),
    (133.374, 1.032_33),
    (143.172, 1.060_3),
    (152.970, 1.082_7),
    (162.768, 1.102_18),
    (172.566, 1.122_48),
    (182.364, 1.137_83),
    (192.162, 1.156_83),
    (201.960, 1.169_73),
];

/// Design characteristics from the paper's **Table 1**.
/// Balls per 9.798 cm layer in Şeker & Çolak (2003)'s HTR-10 model, Table 3:
/// every row holds `1346 N + 733` balls (gh:#333, gh:#472).
///
/// Source: Şeker, V., Çolak, Ü. (2003), *HTR-10 full core first criticality
/// analysis with MCNP*, Nucl. Eng. Des. 222, 263–270,
/// doi:10.1016/S0029-5493(03)00031-1, Table 3. Li, Yu & Wei (2014) tabulate
/// RMC at exactly these heights.
pub const SEKER_BALLS_PER_LAYER: usize = 1346;

/// The constant in Şeker's `1346 N + 733`: one extra basal plane. See
/// [`SEKER_BALLS_PER_LAYER`].
pub const SEKER_BALLS_EXTRA_PLANE: usize = 733;

/// **The reference loading height \[cm\] that holds `balls` balls** in Şeker &
/// Çolak (2003)'s model: `9.798 (balls - 733) / 1346 + 6.0`. It is exact at
/// the tabulated rows and linear between them.
///
/// # Why a model is matched to the reference by ball count (gh:#472, 2026-10-01)
///
/// Our Şeker bed keeps every ball whole. It holds 721 + 609 balls per layer;
/// Şeker's table implies 733 + 613. The difference was chased down: it is
/// exactly the next shells out, 12 basal balls centred at ρ = 87.209 cm and 6
/// central balls at ρ = 87.080 cm, which would cross the r = 90 cm reflector
/// by 0.21 and 0.08 cm. Keeping them reproduces Şeker's basal count exactly
/// (733) and the central one to +2 (615). No lattice offset does it under the
/// whole-ball rule. So the reference keeps balls that cross the wall: cut by
/// the core cylinder in MCNP, or kept by a tolerance.
///
/// Cut pebbles are wrong physics (maintainer, 2026-10-01), so they are not
/// added. Instead the maintainer chose to compare **at the same ball
/// inventory**: the reference at the height where its model holds as many
/// balls as ours. At N = 12 ours holds 16 681 balls, which in Şeker's model is
/// 122.09 cm, 1.49 cm below the 123.576 cm row.
#[must_use]
pub fn seker_height_for_balls(balls: usize) -> f64 {
    table1::LAYER_HEIGHT_CM * (balls as f64 - SEKER_BALLS_EXTRA_PLANE as f64)
        / SEKER_BALLS_PER_LAYER as f64
        + table1::BALL_DIAMETER_CM
}

/// RMC's `k_eff` (Li, Yu & Wei 2014) for a bed of `balls` balls: the
/// reference curve [`RMC_KEFF_VS_HEIGHT`] read at [`seker_height_for_balls`],
/// linear between rows. `None` outside the tabulated range (no
/// extrapolation). This is the comparison point for a whole-ball model; see
/// [`seker_height_for_balls`] for why.
#[must_use]
pub fn rmc_keff_at_ball_count(balls: usize) -> Option<f64> {
    keff_curve_at_height(RMC_KEFF_VS_HEIGHT, seker_height_for_balls(balls))
}

/// A tabulated `(height_cm, k_eff)` curve — [`RMC_KEFF_VS_HEIGHT`],
/// [`MCNP_TABLE3_KEFF_VS_HEIGHT`] or [`MCNP_TABLE4_KEFF_VS_HEIGHT`] — read at
/// `h_cm`, linear between rows. `None` outside the tabulated range (1e-6 cm
/// tolerance at the ends): past the ends the curves flatten and a linear
/// extension would invent reactivity, so nothing is extrapolated.
///
/// The one interpolation every HTR-10 comparison uses (added 2026-10-02,
/// gh:#501, replacing per-example copies). Read a curve at the height where
/// Şeker's model holds the built bed's ball count
/// ([`seker_height_for_balls`]), not at the built height (gh:#472).
#[must_use]
pub fn keff_curve_at_height(curve: &[(f64, f64)], h_cm: f64) -> Option<f64> {
    let (first, last) = (curve.first()?.0, curve.last()?.0);
    if h_cm < first - 1e-6 || h_cm > last + 1e-6 {
        return None;
    }
    curve.windows(2).find_map(|w| {
        let ((h0, k0), (h1, k1)) = (w[0], w[1]);
        (h0 - 1e-6..=h1 + 1e-6)
            .contains(&h_cm)
            .then(|| k0 + (h_cm - h0) / (h1 - h0) * (k1 - k0))
    })
}

pub mod table1 {
    /// Thermal power \[MW\].
    pub const THERMAL_POWER_MW: f64 = 10.0;
    /// Mean core height \[cm\].
    pub const CORE_HEIGHT_CM: f64 = 197.0;
    /// Core diameter \[cm\].
    pub const CORE_DIAMETER_CM: f64 = 180.0;
    /// Fuel-to-moderator ball ratio, as the paper writes it (`0.57/0.43`).
    pub const FUEL_BALL_FRACTION: f64 = 0.57;
    /// Moderator (graphite) ball fraction.
    pub const MODERATOR_BALL_FRACTION: f64 = 0.43;
    /// Total fuel elements in the core, from the paper's body text.
    pub const FUEL_ELEMENTS: f64 = 27_000.0;
    /// Fuel-ball loading step, i.e. one layer \[cm\] — the paper's own
    /// quantisation, *"selected as the height of a layer ... in order to avoid
    /// fractional fuel or moderator balls"*.
    pub const LAYER_HEIGHT_CM: f64 = 9.798;
    /// Ball filling fraction in the core region, from the paper's body text.
    pub const BALL_FILLING_FRACTION: f64 = 0.61;
    /// Ball diameter \[cm\] (Table 2, both fuel and moderator).
    pub const BALL_DIAMETER_CM: f64 = 6.0;
}

/// One over-determined quantity in the paper: a value the paper **states**,
/// beside the value our reconstruction **derives** from other stated quantities.
///
/// A closure is only evidence if the derivation does not use the stated value —
/// otherwise it is a tautology. Each entry below says what it was derived from.
#[derive(Debug, Clone, Copy)]
pub struct GeometryClosure {
    /// What is being closed.
    pub quantity: &'static str,
    /// What the paper states.
    pub stated: f64,
    /// What our reconstruction implies.
    pub derived: f64,
    /// Units, for reporting.
    pub units: &'static str,
    /// What the derivation used — so a reader can check it is independent of
    /// `stated`.
    pub derived_from: &'static str,
}

impl GeometryClosure {
    /// Relative difference `(derived - stated)/stated`, dimensionless.
    #[must_use]
    pub fn relative(&self) -> f64 {
        (self.derived - self.stated) / self.stated
    }
}

/// Every geometry closure the paper supports, computed from
/// [`bed::HexBedCell`] and [`TrisoSpec::HTR10_LI2014`].
#[must_use]
pub fn geometry_closures() -> Vec<GeometryClosure> {
    let cell = bed::HexBedCell::from_paper();
    let spec = TrisoSpec::HTR10_LI2014;

    vec![
        GeometryClosure {
            quantity: "layer height",
            stated: table1::LAYER_HEIGHT_CM,
            derived: bed::close_packed_layer_spacing(table1::BALL_DIAMETER_CM) * 2.0,
            units: "cm",
            derived_from: "two close-packed layers of 6 cm spheres",
        },
        GeometryClosure {
            quantity: "ball filling fraction",
            stated: table1::BALL_FILLING_FRACTION,
            derived: cell.packing_fraction(),
            units: "-",
            derived_from: "2 balls in the reconstructed hex cell",
        },
        GeometryClosure {
            quantity: "fuel elements in the core",
            stated: table1::FUEL_ELEMENTS,
            derived: cell.balls_in_core(table1::CORE_DIAMETER_CM, table1::CORE_HEIGHT_CM),
            units: "balls",
            derived_from: "the cell tiled through a 180 x 197 cm core",
        },
        GeometryClosure {
            quantity: "heavy metal per fuel ball",
            stated: 5.0,
            derived: heavy_metal_per_ball(),
            units: "g",
            derived_from: "8335 kernels, 250 um radius, 10.4 g/cm3, 17 wt%",
        },
        GeometryClosure {
            quantity: "TRISO packing fraction",
            stated: spec.packing_fraction,
            derived: 8335.0 * (spec.opyc / 2.5).powi(3),
            units: "-",
            derived_from: "8335 particles of radius 455 um in a 2.5 cm fuel zone",
        },
    ]
}

/// Heavy metal per fuel ball \[g\], from Table 2's kernel count, radius, density
/// and enrichment — none of which is the stated 5 g.
#[must_use]
pub fn heavy_metal_per_ball() -> f64 {
    use outram_mc_libs::pebble_beds::htr10::{u235_atom_fraction, RHO_UO2};
    let spec = TrisoSpec::HTR10_LI2014;
    let x5 = u235_atom_fraction();
    let m_u = x5 * 235.043_930 + (1.0 - x5) * 238.050_788;
    let m_uo2 = m_u + 2.0 * 15.994_914_6;
    let v_kernel = 4.0 / 3.0 * std::f64::consts::PI * spec.kernel.powi(3);
    8335.0 * v_kernel * RHO_UO2 * m_u / m_uo2
}

#[cfg(test)]
mod tests;
