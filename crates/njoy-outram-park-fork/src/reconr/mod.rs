//! RECONR — Reconstruct pointwise cross sections from resonance parameters.
//!
//! Ported from `reconr.f90` in NJOY2016 (~5 700 lines of Fortran 90).
//!
//! RECONR converts an ENDF evaluation into a PENDF tape: all MF=3 cross sections
//! become fully pointwise (lin-lin TAB1), with resonance contributions from MF=2
//! added to the smooth background. The result is a fine-grid representation ready
//! for Doppler broadening in BROADR.
//!
//! ## Porting phases
//!
//! | Phase | Content | Status |
//! |-------|---------|--------|
//! | 2a | MF=1/MF=2 headers, linearisation, LRU=0 (H-2) | done |
//! | 2b | SLBW/MLBW resonance evaluation (Ar-37 LRU=1, LRF=2) | done |
//! | 2c | Reich-Moore (LRF=3, e.g. U-235) | done |
//! | 2d | Unresolved resonances (LRU=2): UNRESR then PURR | **next to port** (blocks σ above the resolved region — 20 keV for U-238; see `docs/porting-plan.md` §8) |
//! | 2e | R-Matrix Limited (LRF=7, via `crate::samm`) | **wired, unverified** (2026-07-07 — compiles/type-checks, not yet run against a real evaluation; see `../samm/README.md`) |
//! | 2f | Adler-Adler (LRF=4, `crate::reconr::aa`) | **wired, unverified** (2026-07-07 — zero-temperature only, matching SLBW/Reich-Moore's Doppler-deferred-to-BROADR architecture; compiles/type-checks, not yet run against a real evaluation) |
//!
//! All resolved-resonance formalisms ENDF-6 defines (LRF=1/2/3/4/7) are now
//! parsed and reconstructed by RECONR.
//!
//! ## Entry point
//!
//! ```no_run
//! use std::fs::File;
//! use njoy_outram_park_fork::{endf::tape::Tape, reconr::{reconr, ReconrConfig}};
//!
//! let tape = Tape::read(File::open("n-018_Ar_37-tendl2023.endf").unwrap()).unwrap();
//! let config = ReconrConfig { mat: 1828, tolerance: 0.001, temperature: 0.0 };
//! let result = reconr(&tape, &config).unwrap();
//! println!("Material ZA = {}", result.material.za);
//! ```

pub mod aa;
pub mod linearize;
pub mod mf1;
pub mod mf2;
pub mod rm;
pub mod urr;
pub mod rml;
pub mod slbw;

pub use mf1::MaterialInfo;
pub use mf2::{
    EnergyRange, LState, ResonanceFormalism, ResonanceInfo, RmLState, RmResonance, SlbwResonance,
};

use crate::{
    endf::{records::SectionCursor, tape::Tape, MtReaction},
    mixr::mix::sigfig,
    NjoyError,
};
use rm::RmSigmas;
use slbw::{channel_radius, eval_mlbw_lstate, eval_slbw_lstate};

// ── Public configuration and result types ─────────────────────────────────────

/// Configuration for one RECONR run on a single material.
#[derive(Debug, Clone)]
pub struct ReconrConfig {
    /// Material number (MAT) to process.
    pub mat: i32,
    /// Fractional reconstruction tolerance.
    ///
    /// Cross sections are linearised until the linear interpolation error is
    /// below this fraction of the local value. NJOY default: `0.001` (0.1%).
    pub tolerance: f64,
    /// Reconstruction temperature \[K\]. `0.0` = 0 K (no Doppler shift).
    pub temperature: f64,
}

/// One reconstructed MF=3 section, ready for Doppler broadening.
#[derive(Debug, Clone)]
pub struct ReconrSection {
    /// ENDF MF=3 breakup flag `LR` (the TAB1 header's `L2`), carried through
    /// verbatim from the evaluation.
    ///
    /// `0` for an ordinary two-body or lumped channel. A **non-zero** value on
    /// an inelastic level (MT=51-91) names a *second*, simultaneous breakup of
    /// the residual nucleus, using the same numbering as the MT it stands for:
    /// e.g. `LR=22` is `(n,n'alpha)`, `LR=32` is `(n,n'd)`, `LR=33` is
    /// `(n,n't)`. It is **not** redundant with `mt` and it is not rare on light
    /// nuclides -- in ENDF/B-VIII.0, Li-6 carries `LR=32` on 30 of its levels,
    /// Li-7 `LR=33` on 31, B-10 a mix of `LR=22/28/35`, C-12 `LR=23`. GASPR
    /// needs it to credit those breakup particles (`gaspr.f90:565-608`).
    pub lr: i32,
    /// Reaction type. Use `MtReaction::try_from(n)` or `MtReaction::from_any(n)`
    /// to convert from a raw integer if needed.
    pub mt: MtReaction,
    /// Reaction Q-value QI \[eV\] from the MF=3 TAB1 header (the energy released,
    /// or `< 0` threshold for endothermic reactions). Carried through so ACER can
    /// fill the ACE LQR block (which stores QI in MeV). `0.0` for elastic.
    pub qi: f64,
    /// Fully lin-lin (energy \[eV\], σ \[b\]) grid, sorted by energy.
    pub pairs: Vec<(f64, f64)>,
}

impl ReconrSection {
    /// The energy grid \[eV\], as its own contiguous vector.
    ///
    /// [`Self::pairs`] carries the same numbers interleaved, which is the
    /// natural Rust shape and an awkward one for array consumers: NumPy, a
    /// plotting call, or anything else that wants two columns has to
    /// transpose it. These two accessors hand over the columns directly.
    #[must_use]
    pub fn energies(&self) -> Vec<f64> {
        self.pairs.iter().map(|&(e, _)| e).collect()
    }

    /// The cross sections \[b\], aligned with [`Self::energies`].
    #[must_use]
    pub fn xs(&self) -> Vec<f64> {
        self.pairs.iter().map(|&(_, s)| s).collect()
    }

    /// Number of points on the grid.
    #[must_use]
    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    /// Whether the grid is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }
}

/// Result of running RECONR on one material.
#[derive(Debug, Clone)]
pub struct ReconrResult {
    /// Material header from MF=1/MT=451.
    pub material: MaterialInfo,
    /// Reconstructed MF=3 sections, sorted by MT.
    pub sections: Vec<ReconrSection>,
    /// Upper limit \[eV\] of the resonance region as upstream RECONR records it
    /// on the PENDF MF=2/MT=151 range record — the value BROADR reads back as
    /// its default `thnmax` (top energy for Doppler broadening). See
    /// [`ResonanceInfo::pendf_resonance_upper_limit`]. `None` when the
    /// material has no MF=2 at all (BROADR then defaults to 6.5 MeV).
    ///
    /// For U-238 this is 2e4 eV: the top of the resolved range. Broadening
    /// above it would run SIGMA1 over the unresolved region's energy-*averaged*
    /// MF=3 values, which is not what the kernel assumes — upstream never does
    /// (`broadr.f90:107-124`), and neither does
    /// [`crate::broadr::doppler_broaden_below`].
    pub resonance_upper_limit: Option<f64>,
    /// The **MF=2/MT=152** unresolved table, as ENDF rows ready to write to a
    /// PENDF — `genunr`'s output (`reconr.f90:1628-1735`). `None` when the
    /// material has no `LRU = 2` range.
    ///
    /// This is the infinitely-dilute unresolved cross section on `eunr`, the
    /// grid RECONR evaluates on; GROUPR reads it back through
    /// [`crate::groupr::urr_pendf::read_urr_from_tape`]. It is **0 K** — RECONR
    /// writes it before any broadening, and UNRESR/PURR later write their own
    /// temperature-dependent MT=152 in its place.
    ///
    /// Carried through [`crate::broadr::broaden_result`] unchanged, because
    /// BROADR does not touch MF=2 (`broadr.f90` copies it through). A broadened
    /// result therefore still carries the 0 K table, which is what upstream
    /// produces — not an oversight.
    pub unresolved_table: Option<Vec<[f64; 6]>>,
}

impl ReconrResult {
    /// Evaluate cross section \[b\] for a reaction at energy `e` \[eV\].
    ///
    /// Uses linear interpolation on the lin-lin grid. Returns `0.0` if
    /// `mt` is not present or `e` is outside the tabulated range.
    pub fn eval_mt(&self, mt: MtReaction, e: f64) -> f64 {
        let sec = match self.sections.iter().find(|s| s.mt == mt) {
            Some(s) => s,
            None => return 0.0,
        };
        // BELOW THE FIRST TABULATED POINT THE CROSS SECTION IS ZERO, NOT THE
        // FIRST POINT'S VALUE.
        //
        // An MF=3 section that begins above the evaluation's lower bound is a
        // THRESHOLD reaction, and a threshold reaction cannot occur below its
        // threshold. [`eval_lin_lin`] clamps to the endpoint, which is right for
        // the top of the grid and right for a section that spans the whole range,
        // and wrong here — it propagates the threshold value down to zero energy.
        //
        // It is wrong by more than round-off because evaluations do not all start
        // their threshold sections at zero. ENDF/B-VIII.0 F-19 opens MT=51 at
        // `(115 840 eV, 0.018129 b)` and MT=52 at `(207 460 eV, 0.0042683 b)`, so
        // the clamp gave F-19 a **constant 0.0224 b of inelastic scattering at
        // every energy below 115 keV**, all the way into the thermal range.
        //
        // That is not a small error in a cross section, it is a channel that
        // should not exist, and the kinematics make it worse: `two_body_scatter`
        // clamps a negative outgoing CM energy to zero, so a sub-threshold
        // "inelastic" collision drops the neutron to `E/(A+1)²` — a factor of 394
        // for fluorine, nearly six units of lethargy in one collision. Roughly
        // 0.6 % of F-19 collisions through the whole resonance region were
        // teleporting the neutron past it. See GitHub #193.
        // The test is `e0 > THRESHOLD_FLOOR_EV` and not simply `e < e0` so that a
        // section spanning the whole evaluation still CLAMPS at its bottom. Every
        // ENDF neutron sublibrary starts its full-range sections at 1e-5 eV, and
        // nothing thresholds below a keV, so any floor between them separates the
        // two cases; a neutron that slows below 1e-5 eV must keep seeing a finite
        // total cross section rather than a zero one and an infinite flight.
        match sec.pairs.first() {
            Some(&(e0, _)) if e < e0 && e0 > THRESHOLD_FLOOR_EV => 0.0,
            _ => eval_lin_lin(&sec.pairs, e),
        }
    }
}

/// Lowest first-abscissa \[eV\] an MF=3 section may have and still count as
/// spanning the whole evaluation rather than opening at a threshold.
///
/// ENDF neutron sublibraries start full-range sections at `1e-5 eV`, and the
/// lowest reaction thresholds in any evaluation are tens of keV, so the two
/// populations are separated by nine orders of magnitude. `1e-3 eV` sits in that
/// gap with room to spare in both directions.
pub const THRESHOLD_FLOOR_EV: f64 = 1.0e-3;

/// Linear interpolation on a sorted lin-lin (x,y) grid.
///
/// Returns the linearly interpolated y at `x`, **clamped to the endpoint values**
/// outside `[x_min, x_max]`; `0.0` only if the grid is empty. Callers that need a
/// threshold reaction to vanish below its threshold must test the first abscissa
/// themselves — [`ReconrResult::eval_mt`] does, and says why.
pub fn eval_lin_lin(pairs: &[(f64, f64)], x: f64) -> f64 {
    if pairs.is_empty() {
        return 0.0;
    }
    if x <= pairs[0].0 {
        return pairs[0].1;
    }
    if x >= pairs[pairs.len() - 1].0 {
        return pairs[pairs.len() - 1].1;
    }

    let idx = pairs.partition_point(|&(xi, _)| xi <= x);
    if idx == 0 {
        return pairs[0].1;
    }
    let (x0, y0) = pairs[idx - 1];
    let (x1, y1) = pairs[idx];
    if (x1 - x0).abs() < 1e-30 {
        return y0;
    }
    y0 + (y1 - y0) * (x - x0) / (x1 - x0)
}

// ── Main entry point ──────────────────────────────────────────────────────────

/// Reconstruct pointwise cross sections for one material.
///
/// Reads the material identified by `config.mat` from `tape`, linearises every
/// MF=3 section to lin-lin within `config.tolerance`, and adds resonance
/// contributions from MF=2 (SLBW/MLBW/Reich-Moore, Phases 2b/2c).
///
/// # Errors
///
/// - [`NjoyError::SectionNotFound`] — MF=1/MT=451 or MF=3 sections absent.
/// - [`NjoyError::NotPorted`] — MF=2 contains LRF=4 (Adler-Adler).
pub fn reconr(tape: &Tape, config: &ReconrConfig) -> Result<ReconrResult, NjoyError> {
    let mat = config.mat;
    let eps = config.tolerance;

    // MF=1/MT=451 — material header
    let mf1_sec = tape
        .section(mat, 1, 451)
        .ok_or(NjoyError::SectionNotFound {
            mat,
            mf: 1,
            mt: 451,
        })?;
    let material = mf1::parse_material_info(mf1_sec)?;

    // MF=2/MT=151 — resonance parameters (may be absent for charged-particle data)
    let res_info = if let Some(mf2_sec) = tape.section(mat, 2, 151) {
        mf2::parse_resonance_info(mf2_sec)?
    } else {
        ResonanceInfo::default()
    };

    // AWI, the incident particle's mass ratio (MF=1/MT=451, third record);
    // `lunion`'s `awin`. 1 for a neutron sublibrary.
    let awin = mf1_sec.rows.get(2).map_or(1.0, |r| r[0]);

    // MF=3 — background cross sections
    let mut sections: Vec<ReconrSection> = tape
        .sections()
        .iter()
        .filter(|s| s.key.mat == mat && s.key.mf == 3)
        .map(|sec| {
            let mut cur = SectionCursor::new(&sec.rows);
            let head = cur.read_cont()?; // MF=3 HEAD: ZA, AWR, 0, 0, 0, 0
            let mut tab1 = cur.read_tab1()?; // TAB1 header carries QM (c1), QI (c2)
            raise_threshold_to_kinematic(&mut tab1.pairs, tab1.head.c2, head.c2, awin)?;
            let mut pairs = linearize::linearize_tab1(&tab1.interp, &tab1.pairs, eps);
            shade_discontinuities(&mut pairs);
            Ok(ReconrSection {
                lr: tab1.head.l2,
                mt: MtReaction::from_any(sec.key.mt),
                qi: tab1.head.c2,
                pairs,
            })
        })
        .collect::<Result<Vec<_>, NjoyError>>()?;

    sections.sort_by_key(|s| i32::from(s.mt));

    // The LRU=2 ranges, parsed once for both phases below.
    let urr_ranges = match tape.section(mat, 2, 151) {
        Some(sec) if sec.rows.len() > 1 => crate::unresr::mf2::parse_lru2_ranges(&sec.rows[1..])?,
        _ => Vec::new(),
    };

    // Phase 2a-bis: MF=2/MT=152 -- `genunr` (reconr.f90:1628-1735).
    //
    // BUILT BEFORE ANY RESONANCE CONTRIBUTION IS ADDED, and the ordering is
    // load-bearing. `genunr` reads the evaluation's own MF=3 off the input tape
    // (`:1698`, `call findf(mata,3,0,nin)`) -- the background, with nothing
    // reconstructed into it. Building from `sections` after Phase 2b instead
    // feeds it the resolved-range reconstruction as though it were background,
    // which on U-234 overstates the stored total and elastic by 32 % and 36 %
    // at 1.5e3 eV, the energy the resolved and unresolved ranges share.
    let unresolved_table = if urr_ranges.is_empty() {
        None
    } else {
        urr::build_mt152(material.za, material.awr, &urr_ranges, &sections, 0.0, eps)?
    };

    // Phase 2a-ter: upstream's `mtr18` rule (`anlyzd`, `reconr.f90:557-561`).
    // When the evaluation carries MT=19, MT=18 is a REDUNDANT reaction:
    // `lunion` drops the tape's own MT=18 (`:1893`), `emerge` adds resonance
    // fission to MT=19 (`itype=3`, `:4760`), and MT=18 is rebuilt as the sum
    // of 19/20/21/38 (`:4886-4887`) carrying MT=19's Q (`q18`, `:1916`,
    // `:5281`). Done after `genunr`, which reads the tape's MF=3 as-is.
    let mtr18 = sections.iter().any(|s| s.mt.number() == 19);
    if mtr18 {
        sections.retain(|s| s.mt.number() != 18);
    }

    // Phase 2b: add SLBW/MLBW resonance contributions
    add_resonance_contributions(&mut sections, &res_info, eps);

    // Phase 2c: add the infinitely-dilute unresolved (LRU=2) contribution for
    // LSSF=0 ranges. Without this those ranges come back at ZERO cross
    // section; see `urr`'s module doc.
    if !urr_ranges.is_empty() {
        urr::add_unresolved_ranges(&mut sections, &urr_ranges, eps)?;
    }

    // Phase 2d: rebuild the lumped charged-particle channels MT=103-107 from
    // the discrete MT=600-849 levels, as the redundant reactions they are.
    synthesise_lumped_particle_channels(&mut sections);

    if mtr18 {
        synthesise_total_fission(&mut sections);
    }

    Ok(ReconrResult {
        material,
        sections,
        resonance_upper_limit: res_info.pendf_resonance_upper_limit(),
        unresolved_table,
    })
}

/// `lunion`'s threshold check (`reconr.f90:1913-1938`): a section whose first
/// tabulated energy lies below the kinematic threshold
/// `-Q (A+1)/A`, rounded **up** at 7 figures, has that first energy raised to
/// it, and any following energies it now overtakes are pushed up by one unit
/// each. NJOY prints `changed threshold from ... to ...` when it does this.
///
/// Measured 2026-09-26 on U-234 (ENDF/B-VIII.0): every one of the 40 discrete
/// levels' thresholds came out one unit lower in the 7th figure than NJOY's
/// table (`4.368748e-2` against `4.368749e-2` MeV for MT=51), which is exactly
/// the evaluation's own threshold against its `sigfig(thr, 7, +1)`.
fn raise_threshold_to_kinematic(
    pairs: &mut [(f64, f64)],
    qx: f64,
    awr: f64,
    awin: f64,
) -> Result<(), NjoyError> {
    let thrx = if awin != 0.0 {
        let awrx = awr / awin;
        -qx * (awrx + 1.0) / awrx
    } else {
        -qx
    };
    if thrx <= 0.0 || pairs.is_empty() {
        return Ok(());
    }
    let thrxx = sigfig(thrx, 7, 1);
    if pairs[0].0 >= thrxx {
        return Ok(());
    }
    pairs[0].0 = thrxx;
    let mut l = 0;
    while l + 1 < pairs.len() && pairs[l + 1].0 <= pairs[l].0 {
        if l > 10 {
            return Err(NjoyError::EndfParse("lunion: ill-behaved threshold.".into()));
        }
        pairs[l + 1].0 = sigfig(pairs[l].0, 7, 1);
        l += 1;
    }
    Ok(())
}

/// Rebuild MT=18 as the sum of MT=19/20/21/38, upstream's redundant `mtr=18`
/// (`reconr.f90:4886-4893`), on the union of the parts' grids, with MT=19's Q
/// (`q18`, `:1916`, written at `:5281`). Only called when MT=19 is present,
/// after the tape's own MT=18 was dropped as `lunion` drops it (`:1893`).
fn synthesise_total_fission(sections: &mut Vec<ReconrSection>) {
    let parts: Vec<&ReconrSection> = sections
        .iter()
        .filter(|s| matches!(s.mt.number(), 19 | 20 | 21 | 38))
        .collect();
    if parts.is_empty() {
        return;
    }
    let mut grid: Vec<f64> = parts
        .iter()
        .flat_map(|s| s.pairs.iter().map(|&(e, _)| e))
        .collect();
    grid.sort_by(|a, b| a.partial_cmp(b).unwrap());
    grid.dedup_by(|a, b| (*a - *b).abs() <= SAME_ENERGY_REL * b.abs().max(1.0));
    let pairs: Vec<(f64, f64)> = grid
        .iter()
        .map(|&e| (e, parts.iter().map(|s| eval_lin_lin(&s.pairs, e)).sum()))
        .collect();
    let qi = parts
        .iter()
        .find(|s| s.mt.number() == 19)
        .map(|s| s.qi)
        .unwrap_or(0.0);
    sections.retain(|s| s.mt.number() != 18);
    sections.push(ReconrSection {
        lr: 0,
        mt: MtReaction::Mt18Fission,
        qi,
        pairs,
    });
    sections.sort_by_key(|s| i32::from(s.mt));
}

/// The five lumped charged-particle channels and the discrete MF=3 level
/// ranges each one sums, for ENDF-6 (`reconr.f90:522-531`).
const LUMPED_PARTICLE_CHANNELS: [(i32, i32, i32); 5] = [
    (103, 600, 649), // (n,p)
    (104, 650, 699), // (n,d)
    (105, 700, 749), // (n,t)
    (106, 750, 799), // (n,³He)
    (107, 800, 849), // (n,α)
];

/// Build MT=103-107 as the sum of their discrete MT=600-849 levels whenever
/// the evaluation carries any of the levels (~~only when it lacks the lumped
/// section~~ -- see the correction below).
///
/// # Why this is needed
///
/// `anlyzd` (`reconr.f90:566-590`) adds MT=103, 104, 105, 106 or 107 to
/// RECONR's redundant-reaction list `mtr` as soon as *any* section in the
/// corresponding discrete range is present on the tape, and `emerge` then
/// reconstructs it as the sum of its parts. An evaluation is free to carry the
/// levels alone, and several do — so a port that only copies the MF=3 sections
/// it finds silently loses the whole channel.
///
/// # Measured
///
/// B-10 (ENDF/B-VIII.0, MAT 525) carries MT=700 but **no MT=105**. NJOY2016
/// `ac5adf5f`'s PENDF has MT=105 = 3.0068978570e-2 b at 7.6986e5 eV and
/// 2.1783660360e-1 b at 5.5909e6 eV, both equal to its MT=700 to every printed
/// digit; before this pass the crate produced no MT=105 at all. The visible
/// consequence was in GASPR: B-10's tritium production came out 3.19e-5 b
/// against NJOY's 3.01e-2 b — a factor of 940 — and its alpha production
/// 0.568 b against 1.004 b, because MT=105 on B-10 leaves ⁸Be and so counts
/// **two** alphas on top of its triton. Found by
/// `tests/gaspr_vs_njoy2016.rs`, 2026-09-17.
///
/// # Upstream recomputes it unconditionally, with Q = 0
///
/// ~~Where the lumped section **is** present it is left alone rather than
/// recomputed from the levels, even though upstream recomputes it
/// unconditionally. On B-10 the evaluation's own MT=103 already equals NJOY's
/// recomputed sum to every printed digit, so recomputing would be churn with
/// no measured gain.~~ **CORRECTED 2026-09-26** -- the gain was there, just
/// not in the cross section. `lunion` drops the tape's own lumped section
/// (`reconr.f90:1883-1887`) and `recout` writes the rebuilt one with
/// **`Q = 0`** (`scr(2)=0`, `:5280`; only MT=18 gets a Q). Keeping the tape's
/// section kept its Q: NJOY2016's U-235 ACE table stores `Q = 0` for MT=103
/// and MT=107 where this crate stored -0.8199998 and 11.1165 MeV. The
/// synthesised case was wrong the same way -- it took the ground-state
/// level's Q. Both now follow upstream.
fn synthesise_lumped_particle_channels(sections: &mut Vec<ReconrSection>) {
    for (lumped, lo, hi) in LUMPED_PARTICLE_CHANNELS {
        if !sections.iter().any(|s| (lo..=hi).contains(&s.mt.number())) {
            continue;
        }
        sections.retain(|s| s.mt.number() != lumped);
        let levels: Vec<&ReconrSection> = sections
            .iter()
            .filter(|s| (lo..=hi).contains(&s.mt.number()))
            .collect();
        if levels.is_empty() {
            continue;
        }

        let mut grid: Vec<f64> = levels
            .iter()
            .flat_map(|s| s.pairs.iter().map(|&(e, _)| e))
            .collect();
        grid.sort_by(|a, b| a.partial_cmp(b).unwrap());
        grid.dedup_by(|a, b| (*a - *b).abs() <= SAME_ENERGY_REL * b.abs().max(1.0));

        let pairs: Vec<(f64, f64)> = grid
            .iter()
            .map(|&e| (e, levels.iter().map(|s| eval_lin_lin(&s.pairs, e)).sum()))
            .collect();

        // A redundant reaction is written with Q = 0 (`recout`,
        // `reconr.f90:5280`). ~~The ground-state level's Q.~~
        let qi = 0.0;

        sections.push(ReconrSection {
            lr: 0,
            mt: MtReaction::from_any(lumped),
            qi,
            pairs,
        });
    }
    sections.sort_by_key(|s| i32::from(s.mt));
}

/// Relative energy tolerance below which two grid energies count as *the same*
/// energy — upstream's `small` in `lunion` (`reconr.f90`), used to detect a
/// tabulated discontinuity (two consecutive points at one energy).
const SAME_ENERGY_REL: f64 = 1.0e-10;

/// Lowest energy of any ENDF grid, `elow` in `rdfil2` (`reconr.f90`). A range
/// boundary sitting *at* `elow` is not shaded (there is nothing below it).
const ELOW: f64 = 1.0e-5;

/// Represent every tabulated discontinuity as two *distinct* energies.
///
/// ENDF writes a step in σ(E) as two consecutive points at the same energy.
/// Upstream RECONR never lets that reach the PENDF: `lunion` ("check ahead for
/// discontinuity", `reconr.f90`) rewrites the pair as
/// `sigfig(E,7,-1)` / `sigfig(E,7,+1)` — the energy shaded down and up by one
/// unit in the seventh significant figure (2.0e4 → 1.999999e4 / 2.000001e4) —
/// and simply drops the second point when the two σ values are identical
/// (`if (abs(srnext-sr).lt.small*sr) go to 260`).
///
/// This matters downstream: BROADR's grid walk is index-based, and a genuinely
/// duplicated energy is degenerate in it (a zero-width panel). With the two
/// sides at distinct energies a bound such as `thnmax` can fall *between* them,
/// so the lower side is broadened and the upper side copied through untouched
/// — which is exactly how upstream preserves the resolved/unresolved seam
/// (U-238, 20 keV: `op-sdbk`).
///
/// The lower point is left where it is if shading it down would cross the
/// preceding grid point (only possible for a grid finer than 1 part in 10⁷).
pub fn shade_discontinuities(pairs: &mut Vec<(f64, f64)>) {
    if pairs.len() < 2 {
        return;
    }
    let mut out: Vec<(f64, f64)> = Vec::with_capacity(pairs.len());
    let mut i = 0;
    while i < pairs.len() {
        let (e, s) = pairs[i];
        if i + 1 < pairs.len() {
            let (e2, s2) = pairs[i + 1];
            if (e2 - e).abs() <= SAME_ENERGY_REL * e.abs() {
                if (s2 - s).abs() <= SAME_ENERGY_REL * s.abs() {
                    // Identical values: not a discontinuity, drop the duplicate.
                    out.push((e, s));
                } else {
                    let mut lo = sigfig(e, 7, -1);
                    if let Some(&(prev, _)) = out.last() {
                        if lo <= prev {
                            lo = e;
                        }
                    }
                    out.push((lo, s));
                    out.push((sigfig(e2, 7, 1), s2));
                }
                i += 2;
                continue;
            }
        }
        out.push((e, s));
        i += 1;
    }
    *pairs = out;
}

/// Reconstruct **only the MF=3 background** — no MF=2 resonance contributions.
///
/// This is the fast-range path used by the MGXS bake
/// ([`crate::nuclear_data::Mgxs::collapse_from_reconr`]). Above a nuclide's WMP
/// `e_max` the incident energy is beyond the resolved-resonance region, so the
/// smooth linearised MF=3 grid *is* the cross section there. Skipping MF=2 avoids
/// the (potentially expensive, R-matrix-inversion-heavy for LRF=7) resonance
/// reconstruction entirely in a region where it wouldn't change the result anyway.
///
/// Returns the same [`ReconrResult`] shape as [`reconr`] — linearised MF=3
/// sections sorted by MT — but with **no** resonance additions. Do **not** use it
/// below the resonance ceiling; there it omits the resonance cross section.
///
/// It also does **not** run [`synthesise_lumped_particle_channels`], so an
/// evaluation carrying only the discrete MT=600-849 levels comes back with no
/// MT=103-107 here. That asymmetry with [`reconr`] is deliberate and matches
/// upstream: `genunr` reads the evaluation's own MF=3 off the input tape
/// (`reconr.f90:1698`), before `emerge` writes any redundant sum, and the
/// MT=152 writer that consumes this function needs exactly that pre-`emerge`
/// state. A caller that wants the PENDF-equivalent section list — which is
/// what HEATR, GASPR and ACER read — must use [`reconr`].
pub fn reconr_background(tape: &Tape, mat: i32, tolerance: f64) -> Result<ReconrResult, NjoyError> {
    let mf1_sec = tape
        .section(mat, 1, 451)
        .ok_or(NjoyError::SectionNotFound {
            mat,
            mf: 1,
            mt: 451,
        })?;
    let material = mf1::parse_material_info(mf1_sec)?;

    let mut sections: Vec<ReconrSection> = tape
        .sections()
        .iter()
        .filter(|s| s.key.mat == mat && s.key.mf == 3)
        .map(|sec| {
            let mut cur = SectionCursor::new(&sec.rows);
            let _head = cur.read_cont()?; // MF=3 HEAD
            let tab1 = cur.read_tab1()?; // TAB1 header carries QM (c1), QI (c2)
            let mut pairs = linearize::linearize_tab1(&tab1.interp, &tab1.pairs, tolerance);
            shade_discontinuities(&mut pairs);
            Ok(ReconrSection {
                lr: tab1.head.l2,
                mt: MtReaction::from_any(sec.key.mt),
                qi: tab1.head.c2,
                pairs,
            })
        })
        .collect::<Result<Vec<_>, NjoyError>>()?;
    sections.sort_by_key(|s| i32::from(s.mt));
    // The broadening limit only needs the MF=2 range bounds, which are cheap
    // to parse even for formalisms whose reconstruction is expensive.
    let resonance_upper_limit = tape
        .section(mat, 2, 151)
        .map(mf2::parse_resonance_info)
        .transpose()?
        .and_then(|info| info.pendf_resonance_upper_limit());
    Ok(ReconrResult {
        material,
        sections,
        resonance_upper_limit,
        // Background-only: no resonance reconstruction ran, so there is no
        // unresolved table to store either.
        unresolved_table: None,
    })
}

// ── Phase 2b: resonance contribution ─────────────────────────────────────────

/// Add SLBW/MLBW and Reich-Moore resonance contributions to the MF=3 background.
///
/// For each resolved resonance range (LRU=1), builds a dense energy grid around
/// each resonance and evaluates the cross sections. The contributions are merged
/// with the existing lin-lin background grid.
///
/// Dispatches to SLBW evaluation (LRF=1/2) or Reich-Moore evaluation (LRF=3).
/// MT mapping: elastic (MT=2), capture (MT=102), fission (MT=18), total (MT=1).
fn add_resonance_contributions(
    sections: &mut Vec<ReconrSection>,
    res_info: &ResonanceInfo,
    eps: f64,
) {
    for range in res_info.resolved_slbw_ranges() {
        add_slbw_range(sections, range, eps);
    }
    for range in res_info.resolved_rm_ranges() {
        add_rm_range(sections, range, eps);
    }
    for range in res_info.resolved_rml_ranges() {
        rml::add_rml_range(sections, range, eps);
    }
    for range in res_info.resolved_aa_ranges() {
        add_aa_range(sections, range, eps);
    }
}

/// Add Adler-Adler (LRF=4) resonance contributions.
///
/// Unlike [`add_slbw_range`]/[`add_rm_range`], `aa::eval_aa_range` takes
/// the whole range (not one l-state at a time) — see that function's doc
/// comment for why Adler-Adler doesn't decompose per-l-state the way
/// SLBW/Reich-Moore do.
fn add_aa_range(sections: &mut Vec<ReconrSection>, range: &EnergyRange, eps: f64) {
    let Some(aa) = &range.aa else { return };
    if aa.l_states.is_empty() {
        return;
    }

    let mut halo = Vec::new();
    add_aa_halo_energies(&mut halo, &aa.l_states, range.el, range.eh);

    rebuild_range(sections, range.el, range.eh, halo, eps, &[], |e| {
        let s = aa::eval_aa_range(e, aa, range.ap);
        RangeDelta {
            total: s.total,
            elastic: s.elastic,
            fission: s.fission,
            capture: s.capture,
            other: [0.0; MAX_OTHER],
        }
    });
}

/// Upper bound on the extra (non-elastic, non-fission, non-capture)
/// R-matrix-limited reaction channels carried per energy: `mmtres(10)`
/// upstream minus the three fixed slots.
pub(crate) const MAX_OTHER: usize = 7;

/// Resonance cross-section contributions at one energy, by reaction \[b\].
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct RangeDelta {
    pub(crate) total: f64,
    pub(crate) elastic: f64,
    pub(crate) fission: f64,
    pub(crate) capture: f64,
    /// Extra LRF=7 particle-pair channels (`mmtres(3..)` that are not
    /// fission), in the order of `other_mts` handed to [`rebuild_range`];
    /// unused entries stay zero.
    pub(crate) other: [f64; MAX_OTHER],
}

fn add_slbw_range(sections: &mut Vec<ReconrSection>, range: &EnergyRange, eps: f64) {
    if range.l_states.is_empty() {
        return;
    }

    // Hoist the per-l-state channel radius and resonance tuples out of the
    // energy loop — they do not depend on energy.
    let prepared: Vec<(u32, f64, f64, Vec<_>)> = range
        .l_states
        .iter()
        .map(|ls| {
            let ra = channel_radius(ls.awri, range.naps, range.ap);
            let tuples: Vec<_> = ls.resonances.iter().map(|r| r.as_tuple()).collect();
            (ls.l, ls.awri, ra, tuples)
        })
        .collect();

    let mut halo = Vec::new();
    add_resonance_halo_energies(&mut halo, &range.l_states, range.el, range.eh);

    // LRF=1 -> csslbw, LRF=2 -> csmlbw (upstream `sigma`, reconr.f90:2610-2616)
    let mlbw = matches!(range.formalism, Some(ResonanceFormalism::Mlbw));
    rebuild_range(sections, range.el, range.eh, halo, eps, &[], |e| {
        let mut d = RangeDelta::default();
        for (l, awri, ra, tuples) in &prepared {
            let s = if mlbw {
                eval_mlbw_lstate(e, tuples, *l, range.spi, range.ap, *awri, *ra)
            } else {
                eval_slbw_lstate(e, tuples, *l, range.spi, range.ap, *awri, *ra)
            };
            d.elastic += s.elastic;
            d.capture += s.capture;
            d.fission += s.fission;
        }
        d.total = d.elastic + d.capture + d.fission;
        d
    });
}

fn add_rm_range(sections: &mut Vec<ReconrSection>, range: &EnergyRange, eps: f64) {
    if range.rm_l_states.is_empty() {
        return;
    }

    let mut halo = Vec::new();
    add_rm_halo_energies(&mut halo, &range.rm_l_states, range.el, range.eh);

    rebuild_range(sections, range.el, range.eh, halo, eps, &[], |e| {
        let mut d = RangeDelta::default();
        for ls in &range.rm_l_states {
            let s: RmSigmas = rm::eval_rm_lstate(e, ls, range.ap, range.spi, range.naps);
            d.total += s.total;
            d.elastic += s.elastic;
            d.fission += s.fission;
            d.capture += s.capture;
        }
        d
    });
}

/// Rebuild the elastic/capture/fission/total sections over a resonance range.
///
/// For each of the four reactions, the new grid is the union of (a) the existing
/// background energies inside `[el, eh]` and (b) the resonance `halo` points.
/// At every grid energy the new cross section is `background(e) + resonance(e)`,
/// where `background(e)` is interpolated from the **original** (pre-merge)
/// section and `resonance(e)` comes from `delta_at`. Points outside `[el, eh]`
/// are copied through untouched.
///
/// Interpolating the background from an immutable snapshot — rather than from
/// the section as it is being modified — is essential: appending points and then
/// interpolating the half-built (unsorted) vector causes runaway accumulation.
///
/// The seed grid (background energies ∪ resonance `halo`) is then **adaptively
/// refined** ([`refine_resonance_grid`]) so linear interpolation of the
/// resonance contribution reproduces the directly-evaluated value to within
/// `eps` everywhere — this is what fills the multi-eV inter-resonance gaps the
/// fixed halo leaves, where the Lorentzian wing would otherwise be grossly
/// over-linearised (the U-238 capture-wing pedestal bug; see this crate's
/// `docs/porting-plan.md`).
pub(crate) fn rebuild_range(
    sections: &mut Vec<ReconrSection>,
    el: f64,
    eh: f64,
    halo: Vec<f64>,
    eps: f64,
    other_mts: &[i32],
    delta_at: impl Fn(f64) -> RangeDelta + Sync,
) {
    // Range-boundary nodes, shaded as upstream `rdfil2` does ("shade nodes to
    // prevent discontinuities", `reconr.f90:745-777`): the resonance
    // contribution stops abruptly at `eh`, so the top of the range is carried
    // at `sigfig(eh,7,-1)` (with resonances) and the first point beyond it at
    // `sigfig(eh,7,+1)` (background only). Likewise at `el`, unless `el` is the
    // grid floor `elow` (`if (abs(el-elow).le.small)` — nothing below it).
    let eh_lo = sigfig(eh, 7, -1);
    let eh_hi = sigfig(eh, 7, 1);
    let shade_el = (el - ELOW).abs() > SAME_ENERGY_REL * ELOW;
    let el_lo = sigfig(el, 7, -1);
    let el_hi = sigfig(el, 7, 1);
    let same = |a: f64, b: f64| (a - b).abs() <= SAME_ENERGY_REL * b.abs();

    // Union energy grid inside [el, eh]: background energies + resonance halo.
    // A background point sitting exactly on a shaded boundary moves onto the
    // in-range shaded node.
    let mut egrid: Vec<f64> = collect_background_energies(sections, el, eh)
        .into_iter()
        .map(|e| {
            if same(e, eh) {
                eh_lo
            } else if shade_el && same(e, el) {
                el_hi
            } else {
                e
            }
        })
        .collect();
    egrid.push(eh_lo);
    if shade_el {
        egrid.push(el_hi);
    }
    egrid.extend(halo.into_iter().filter(|&e| e > el && e < eh && e > 0.0));
    egrid.sort_by(|a, b| a.partial_cmp(b).unwrap());
    egrid.dedup_by(|a, b| (*a - *b).abs() < 1e-10 * b.abs().max(1.0));
    if egrid.is_empty() {
        return;
    }

    // Adaptively refine so lin-lin interpolation of the resonance contribution
    // is within `eps` everywhere; returns the (denser) grid and the resonance
    // deltas already evaluated at every point (so `delta_at` is not called again).
    let (egrid, deltas) = refine_resonance_grid(&egrid, &delta_at, eps);

    // The four fixed reactions, then the extra LRF=7 channels by MT; a
    // channel with no MF=3 section on the tape is dropped, as upstream
    // `emerge` only adds resonance terms to sections it finds
    // (reconr.f90:4755-4767).
    let targets: Vec<(MtReaction, Option<usize>)> = [
        MtReaction::Mt1Total,
        MtReaction::Mt2Elastic,
        MtReaction::Mt18Fission,
        // `itype = 3` for MT=19 too (`reconr.f90:4760`). The two never
        // coexist here: MT=18 is dropped when MT=19 is present (`mtr18`).
        MtReaction::Mt19FirstChanceFission,
        MtReaction::Mt102Capture,
    ]
    .into_iter()
    .map(|m| (m, None))
    .chain(
        other_mts
            .iter()
            .enumerate()
            .map(|(k, &mt)| (MtReaction::from_any(mt), Some(k))),
    )
    .collect();
    for (mt, other_k) in targets {
        let sec = match sections.iter_mut().find(|s| s.mt == mt) {
            Some(s) => s,
            None => continue,
        };

        // Immutable snapshot of the background for interpolation.
        let bg = sec.pairs.clone();

        // Keep points strictly outside the resonance range as-is.
        let mut new_pairs: Vec<(f64, f64)> = bg
            .iter()
            .copied()
            .filter(|&(e, _)| e < el || e > eh)
            .collect();

        // Background-only shaded nodes just outside the range (`eh_hi`, and
        // `el_lo` when `el` is shaded), unless the background already carries
        // a point there — as it does when an MF=3 discontinuity at the
        // boundary was itself shaded by `shade_discontinuities`.
        let has_point_near = |e: f64| bg.iter().any(|&(x, _)| same(x, e));
        if !has_point_near(eh_hi) {
            new_pairs.push((eh_hi, eval_lin_lin(&bg, eh_hi)));
        }
        if shade_el && !has_point_near(el_lo) {
            new_pairs.push((el_lo, eval_lin_lin(&bg, el_lo)));
        }

        // Add background + resonance for every in-range grid energy.
        for (i, &e) in egrid.iter().enumerate() {
            let base = eval_lin_lin(&bg, e);
            let add = match (mt, other_k) {
                (_, Some(k)) => deltas[i].other[k],
                (MtReaction::Mt1Total, _) => deltas[i].total,
                (MtReaction::Mt2Elastic, _) => deltas[i].elastic,
                (MtReaction::Mt18Fission, _) => deltas[i].fission,
                (MtReaction::Mt19FirstChanceFission, _) => deltas[i].fission,
                (MtReaction::Mt102Capture, _) => deltas[i].capture,
                _ => 0.0,
            };
            new_pairs.push((e, base + add));
        }

        new_pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        new_pairs.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-10 * b.0.abs().max(1.0));
        sec.pairs = new_pairs;
    }

    rebuild_total_as_sum_of_parts(sections, &egrid);
}

/// Rebuild MF=3 MT=1 as the **sum of its parts**, which is what RECONR means by
/// a redundant reaction: *"Redundant reactions are reconstructed to be the sum
/// of their parts"* (`reconr.f90:72-73`).
///
/// # Why this is not the same as `background + resonance total`
///
/// The obvious-looking assembly — take MF=3 MT=1's own background and add the
/// resonance total to it — is wrong whenever a **partial** carries an MF=3
/// background that MT=1's own section does not. Sr-88 (ENDF/B-VIII.1, MAT 3837)
/// is exactly that case: its MF=3 MT=1 and MT=2 backgrounds are identically
/// zero while MT=102 carries a `1/v` remainder, so building the total that way
/// silently dropped the capture background from it — 2.1629e-2 b at 1e-5 eV,
/// 0.24 % of the total. `bn:op-u9jp`.
///
/// It is not wrong everywhere, which is why it survived: on Si-30
/// (ENDF/B-VIII.0) the evaluation's MT=1 background already contains the
/// partials' backgrounds, so the two constructions agree to every digit and
/// this function is a no-op.
///
/// # Which sections count as parts
///
/// Mirrors `emerge`'s accumulation loop (`reconr.f90:4840-4893`). For the
/// `mtr = 1` target upstream falls straight through to label 400 with no
/// per-MT test of its own, so every MF=3 section contributes except those the
/// loop excludes before that point:
///
/// - **MT=10** (`:4845`) — the lumped `(n,continuum)` special case.
/// - **MT=46..49** (`:4847`) — reserved/derived.
/// - **MT > 200 and MT < 600** (`:4848`, `mpmin = 600` for ENDF-6) — derived
///   quantities such as `mubar`/`xi`/damage, which are not cross sections.
///
/// and, additionally, the **redundant sums themselves**, which would otherwise
/// be double-counted with the partials they stand for:
///
/// - **MT=3** (nonelastic) and **MT=4** (sum of MT=51..91).
/// - **MT=18** where the evaluation also carries MT=19..21 or 38.
/// - **MT=103..107** where the evaluation also carries the corresponding
///   discrete MT=600..849 levels. Where it does *not* — as Sr-88 does not —
///   MT=103..107 **are** the parts and are summed.
///
/// # Verified
///
/// Reproduces NJOY2016's MT=1 exactly on both materials with a committed
/// RECONR oracle: Si-30 (where it changes nothing) and Sr-88 (where it
/// restores the missing 2.1629e-2 b). See
/// `tests/reconr_mt1_is_the_sum_of_its_parts.rs`.
fn rebuild_total_as_sum_of_parts(sections: &mut [ReconrSection], egrid: &[f64]) {
    if egrid.is_empty() || !sections.iter().any(|s| s.mt == MtReaction::Mt1Total) {
        return;
    }

    let present: Vec<i32> = sections.iter().map(|s| s.mt.number()).collect();
    let has_discrete = |lo: i32, hi: i32| present.iter().any(|&m| (lo..=hi).contains(&m));
    let has_total_fission = present.iter().any(|&m| m == 18);

    // `emerge`'s exclusions (reconr.f90:4845-4848), plus the redundant sums.
    let is_part = |mt: i32| -> bool {
        if mt == 1 || mt == 3 || mt == 4 || mt == 10 {
            return false;
        }
        if (46..=49).contains(&mt) {
            return false;
        }
        if mt > 200 && mt < 600 {
            return false;
        }
        // Fission: sum MT=19/20/21/38 when the evaluation carries MT=19,
        // otherwise MT=18 -- upstream's `mtr18` rule.
        //
        // ~~Take MT=18, not MT=19/20/21/38 -- the OPPOSITE of upstream,
        // deliberately, because this crate reconstructed resonance fission on
        // MT=18 only and left MT=19..21/38 as pure background (on U-234 that
        // made summing the parts lose 3.448 b at 1e-5 eV).~~ **CORRECTED
        // 2026-09-26** -- that was the port's defect, not a reason to diverge.
        // Upstream drops the tape's MT=18 when MT=19 exists (`lunion`,
        // `reconr.f90:1893`), puts the resonance fission on MT=19 (`itype=3`,
        // `:4760`) and rebuilds MT=18 from the parts afterwards. `reconr` now
        // does all three, so by the time this runs MT=18 is absent whenever
        // MT=19 is present and `has_total_fission` selects the parts.
        if (19..=21).contains(&mt) && has_total_fission {
            return false;
        }
        if mt == 38 && has_total_fission {
            return false;
        }
        match mt {
            103 => !has_discrete(600, 649),
            104 => !has_discrete(650, 699),
            105 => !has_discrete(700, 749),
            106 => !has_discrete(750, 799),
            107 => !has_discrete(800, 849),
            _ => true,
        }
    };

    // Sum the parts on the reconstruction grid. Sections are read before the
    // total is written, so the borrow is split rather than interleaved.
    let sums: Vec<(f64, f64)> = egrid
        .iter()
        .map(|&e| {
            let total = sections
                .iter()
                .filter(|s| is_part(s.mt.number()))
                .map(|s| eval_lin_lin(&s.pairs, e))
                .sum();
            (e, total)
        })
        .collect();

    let Some(total_sec) = sections.iter_mut().find(|s| s.mt == MtReaction::Mt1Total) else {
        return;
    };
    for (e, v) in sums {
        // Replace the value at this grid energy; points outside the
        // reconstruction grid keep whatever the background gave them, exactly
        // as every other target does.
        if let Some(slot) = total_sec
            .pairs
            .iter_mut()
            .find(|(x, _)| (*x - e).abs() <= 1e-10 * e.abs().max(1.0))
        {
            slot.1 = v;
        }
    }
}

/// Safety cap on bisection depth per interval. Upstream terminates a stack by
/// the significant-figure test (`reconr.f90:2374-2379`: a panel whose
/// midpoint is not distinguishable from its ends at 9 significant figures is
/// converged), which this port reproduces; the cap only guards against a
/// pathological `delta_at`.
const RES_MAX_BISECTION_DEPTH: u32 = 64;

/// Upstream `estp` (`reconr.f90:2267`): a converged panel more than `estp`
/// times wider than the previous accepted one is split anyway ("don't allow
/// big increases in the energy step, they may be misconvergences",
/// `:2419-2421`).
const RES_STEP_INCREASE: f64 = 4.1;

/// Upstream `trange` (`reconr.f90:2270`): below 0.4999 eV the tolerances are
/// tightened five-fold (`:2390-2393`).
const RES_TIGHT_RANGE_EV: f64 = 0.4999;

/// The `resxs` convergence tolerances (`reconr.f90:2388-2407`).
#[derive(Debug, Clone, Copy)]
struct RefineTolerances {
    /// `err` — the fractional tolerance every reaction must meet.
    err: f64,
    /// `errmax` — the looser tolerance allowed when the resonance-integral
    /// criterion is met (card 4, default `10*err`, `reconr.f90:428-429`).
    errmax: f64,
    /// `errint` — the maximum resonance-integral error per grid point \[b\]
    /// (card 4, default `err/20000`, `reconr.f90:430`).
    errint: f64,
}

impl RefineTolerances {
    /// Upstream's card-4 defaults for a deck that gives only `err`.
    fn defaults(err: f64) -> Self {
        let errmax = (10.0 * err).max(err);
        Self {
            err,
            errmax,
            errint: err / 20000.0,
        }
    }
}

/// Why a panel was accepted (`reconr.f90:2398-2421`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PanelAccept {
    /// Every reaction within `err` — the step-increase rule still applies.
    Converged,
    /// Accepted by the resonance-integral check (`nmax`); upstream skips the
    /// step-increase rule for these (`go to 150`).
    Integral,
    /// Forced by the significant-figure test; nothing finer is representable.
    SigFig,
}

/// Upstream's per-panel decision (`reconr.f90:2361-2407`): the rounded
/// midpoint `xm`, its resonance contribution, and whether the panel is
/// accepted or must be split.
struct PanelTest {
    xm: f64,
    d_mid: RangeDelta,
    accept: Option<PanelAccept>,
}

/// Evaluate upstream's midpoint test on the panel `[e1, e2]` (lower `x(i)`,
/// upper `x(i-1)` in the Fortran stack).
fn test_panel(
    e1: f64,
    d1: RangeDelta,
    e2: f64,
    d2: RangeDelta,
    delta_at: &impl Fn(f64) -> RangeDelta,
    tol: RefineTolerances,
) -> PanelTest {
    let dx = e2 - e1;
    let xm0 = 0.5 * (e1 + e2);
    // Significant-figure convergence (reconr.f90:2371-2379).
    let ndig = if xm0 > 0.1 && xm0 < 1.0 { 8 } else { 9 };
    if xm0 <= sigfig(e1, ndig, 1) || xm0 >= sigfig(e2, ndig, -1) {
        return PanelTest {
            xm: xm0,
            d_mid: RangeDelta::default(),
            accept: Some(PanelAccept::SigFig),
        };
    }
    let xm = if xm0 > sigfig(e1, 7, 1) && xm0 < sigfig(e2, 7, -1) {
        sigfig(xm0, 7, 0)
    } else {
        sigfig(xm0, ndig, 0)
    };
    let d_mid = delta_at(xm);
    // Linear interpolation at the (rounded) midpoint (reconr.f90:2382-2383,
    // 2395-2396) and the per-reaction deviations; upstream tests the
    // `nsig-1` partials (elastic, fission, capture, and for LRF=7 every
    // extra particle-pair channel, reconr.f90:331-340), not the total. An
    // unused `other` slot is zero on both sides and passes trivially.
    let fr2 = (xm - e1) / dx;
    let fr1 = 1.0 - fr2;
    const NP: usize = 3 + MAX_OTHER;
    let parts: [(f64, f64, f64); NP] = std::array::from_fn(|j| match j {
        0 => (d1.elastic, d2.elastic, d_mid.elastic),
        1 => (d1.fission, d2.fission, d_mid.fission),
        2 => (d1.capture, d2.capture, d_mid.capture),
        k => (d1.other[k - 3], d2.other[k - 3], d_mid.other[k - 3]),
    });
    let dm: [f64; NP] = std::array::from_fn(|j| {
        let (lo, hi, t) = parts[j];
        (t - (fr1 * lo + fr2 * hi)).abs()
    });
    let sig: [f64; NP] = std::array::from_fn(|j| parts[j].2);
    let (errn, errm) = if e2 < RES_TIGHT_RANGE_EV {
        (tol.err / 5.0, tol.errmax / 5.0)
    } else {
        (tol.err, tol.errmax)
    };
    let not_converged = (0..NP).any(|j| dm[j] > errn * sig[j]);
    if !not_converged {
        return PanelTest {
            xm,
            d_mid,
            accept: Some(PanelAccept::Converged),
        };
    }
    if (0..NP).any(|j| dm[j] > errm * sig[j]) {
        return PanelTest {
            xm,
            d_mid,
            accept: None,
        };
    }
    // Resonance-integral check (reconr.f90:2403-2412).
    let tsti = 2.0 * tol.errint * xm / dx;
    if (0..NP).any(|j| dm[j] >= tsti) {
        return PanelTest {
            xm,
            d_mid,
            accept: None,
        };
    }
    PanelTest {
        xm,
        d_mid,
        accept: Some(PanelAccept::Integral),
    }
}

/// Sequential state of the upstream stack walk: how many points have been
/// written (`in`) and the energy of the last one (`res(1)`).
#[derive(Debug, Clone, Copy)]
struct WalkState {
    written: usize,
    last_written: f64,
}

/// Recursive refinement of one panel with upstream's full test, including
/// the step-increase rule when `state` is given (`reconr.f90:2361-2470`).
/// Interior points (and their deltas) are pushed in ascending order; the
/// caller owns the endpoints. Every accepted panel "writes" its lower end.
#[allow(clippy::too_many_arguments)]
fn refine_panel(
    e1: f64,
    d1: RangeDelta,
    e2: f64,
    d2: RangeDelta,
    delta_at: &impl Fn(f64) -> RangeDelta,
    tol: RefineTolerances,
    depth: u32,
    state: Option<&mut WalkState>,
    out_e: &mut Vec<f64>,
    out_d: &mut Vec<RangeDelta>,
    out_flag: &mut Vec<PanelAccept>,
) {
    let split = |accept: Option<PanelAccept>, state: Option<&WalkState>| -> bool {
        match accept {
            None => true,
            Some(PanelAccept::Converged) => match state {
                // reconr.f90:2419-2421: in > 3 and dx > estp*(x(i) - res(1)).
                Some(st) => {
                    st.written > 3 && (e2 - e1) > RES_STEP_INCREASE * (e1 - st.last_written)
                }
                None => false,
            },
            Some(_) => false,
        }
    };
    let t = test_panel(e1, d1, e2, d2, delta_at, tol);
    let mut state = state;
    if depth < RES_MAX_BISECTION_DEPTH && split(t.accept, state.as_deref()) {
        refine_panel(
            e1,
            d1,
            t.xm,
            t.d_mid,
            delta_at,
            tol,
            depth + 1,
            state.as_deref_mut(),
            out_e,
            out_d,
            out_flag,
        );
        out_e.push(t.xm);
        out_d.push(t.d_mid);
        refine_panel(
            t.xm,
            t.d_mid,
            e2,
            d2,
            delta_at,
            tol,
            depth + 1,
            state,
            out_e,
            out_d,
            out_flag,
        );
        return;
    }
    // Accepted: upstream writes the lower end (reconr.f90:2426-2431).
    out_flag.push(t.accept.unwrap_or(PanelAccept::SigFig));
    if let Some(st) = state {
        st.written += 1;
        st.last_written = e1;
    }
}

/// Adaptively refine `seed_grid` the way upstream `resxs` does
/// (`reconr.f90:2240-2570`): split a panel until linear interpolation of the
/// resonance contribution `delta_at` at its (7-figure-rounded) midpoint is
/// within `err` for elastic, fission and capture, unless the resonance-integral
/// check (`errmax`/`errint`) accepts it, or the significant-figure test says
/// nothing finer is representable; then, walking the grid in order, split any
/// converged panel more than `estp` times wider than the previous one.
/// Returns the refined, ascending grid together with the [`RangeDelta`]
/// already evaluated at every returned point (so callers never re-evaluate
/// `delta_at` — important since it is expensive, especially for LRF=7 which
/// runs a full R-matrix inversion per energy).
///
/// The seed windows are refined in parallel with the midpoint/integral test
/// only (that test is local to a panel), and the sequential step-increase
/// rule — which needs the previous *accepted* panel — is applied in a second,
/// ordered walk that re-tests anything it splits. The result is the grid the
/// single Fortran stack produces, in the same order.
fn refine_resonance_grid(
    seed_grid: &[f64],
    delta_at: &(impl Fn(f64) -> RangeDelta + Sync),
    eps: f64,
) -> (Vec<f64>, Vec<RangeDelta>) {
    #[cfg(not(target_arch = "wasm32"))]
    use rayon::prelude::*;
    #[cfg(target_arch = "wasm32")]
    use crate::wasm_par::*;

    let tol = RefineTolerances::defaults(eps);
    let n = seed_grid.len();

    // Phase 1: every seed window independently (`delta_at` is a pure function
    // of energy). Each window yields its left endpoint plus interior points,
    // and one acceptance flag per panel.
    let per_window: Vec<(Vec<f64>, Vec<RangeDelta>, Vec<PanelAccept>)> = (0..n - 1)
        .into_par_iter()
        .map(|i| {
            let e1 = seed_grid[i];
            let e2 = seed_grid[i + 1];
            let d1 = delta_at(e1);
            let d2 = delta_at(e2);
            let mut we: Vec<f64> = vec![e1];
            let mut wd: Vec<RangeDelta> = vec![d1];
            let mut wf: Vec<PanelAccept> = Vec::new();
            refine_panel(
                e1, d1, e2, d2, delta_at, tol, 0, None, &mut we, &mut wd, &mut wf,
            );
            (we, wd, wf)
        })
        .collect();

    let mut pts: Vec<(f64, RangeDelta)> = Vec::new();
    let mut flags: Vec<PanelAccept> = Vec::new();
    for (we, wd, wf) in per_window {
        pts.extend(we.into_iter().zip(wd));
        flags.extend(wf);
    }
    pts.push((seed_grid[n - 1], delta_at(seed_grid[n - 1])));
    debug_assert_eq!(flags.len() + 1, pts.len());

    // Phase 2: the ordered walk with the step-increase rule.
    let mut out_e: Vec<f64> = Vec::with_capacity(pts.len());
    let mut out_d: Vec<RangeDelta> = Vec::with_capacity(pts.len());
    let mut state = WalkState {
        written: 0,
        last_written: pts[0].0,
    };
    for k in 0..pts.len() - 1 {
        let (e1, d1) = pts[k];
        let (e2, d2) = pts[k + 1];
        out_e.push(e1);
        out_d.push(d1);
        let widen = flags[k] == PanelAccept::Converged
            && state.written > 3
            && (e2 - e1) > RES_STEP_INCREASE * (e1 - state.last_written);
        if widen {
            let mut ignored = Vec::new();
            refine_panel(
                e1,
                d1,
                e2,
                d2,
                delta_at,
                tol,
                0,
                Some(&mut state),
                &mut out_e,
                &mut out_d,
                &mut ignored,
            );
        } else {
            state.written += 1;
            state.last_written = e1;
        }
    }
    let (el, dl) = pts[pts.len() - 1];
    out_e.push(el);
    out_d.push(dl);

    (out_e, out_d)
}

/// Collect all energies in `[el, eh]` already present in any section's grid.
fn collect_background_energies(sections: &[ReconrSection], el: f64, eh: f64) -> Vec<f64> {
    let mut energies = Vec::new();
    for sec in sections {
        for &(e, _) in &sec.pairs {
            if e >= el && e <= eh {
                energies.push(e);
            }
        }
    }
    energies.sort_by(|a, b| a.partial_cmp(b).unwrap());
    energies.dedup();
    energies
}

/// Add a halo of energy points around each SLBW/MLBW resonance peak.
///
/// Points are placed at `E_r ± k × (Γ_tot/2)` for several values of k.
/// Negative-energy resonances are skipped (below threshold).
fn add_resonance_halo_energies(grid: &mut Vec<f64>, l_states: &[LState], el: f64, eh: f64) {
    const OFFSETS: &[f64] = &[
        -10.0, -5.0, -2.0, -1.0, -0.5, -0.25, 0.0, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0,
    ];
    for ls in l_states {
        for res in &ls.resonances {
            if res.er <= 0.0 {
                continue;
            }
            let half_g = res.gt / 2.0;
            grid.push(res.er);
            for &off in OFFSETS {
                let e = res.er + off * half_g;
                if e > el && e < eh && e > 0.0 {
                    grid.push(e);
                }
            }
        }
    }
}

/// Add a halo of energy points around each Reich-Moore resonance peak.
///
/// Total width is `Γ_n + Γ_γ + |Γ_fA| + |Γ_fB|`.
fn add_rm_halo_energies(grid: &mut Vec<f64>, rm_l_states: &[RmLState], el: f64, eh: f64) {
    const OFFSETS: &[f64] = &[
        -10.0, -5.0, -2.0, -1.0, -0.5, -0.25, 0.0, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0,
    ];
    for ls in rm_l_states {
        for res in &ls.resonances {
            if res.er <= 0.0 {
                continue;
            }
            let gt = res.gn + res.gg + res.gfa.abs() + res.gfb.abs();
            let half_g = gt / 2.0;
            grid.push(res.er);
            for &off in OFFSETS {
                let e = res.er + off * half_g;
                if e > el && e < eh && e > 0.0 {
                    grid.push(e);
                }
            }
        }
    }
}

/// Add a halo of energy points around each Adler-Adler resonance peak.
///
/// Total width proxy is `DW_total` (the total-reaction Adler-Adler width
/// parameter) when nonzero, else the largest of `DW_fission`/`DW_capture`
/// — Adler-Adler resonances always carry all three `DW`s (per the fixed
/// 12-word-per-resonance layout, see `reconr::mf2::parse_adler_adler`), so
/// this just prefers the "total" one to match SLBW/Reich-Moore's use of a
/// total-reaction width for their own halos.
fn add_aa_halo_energies(
    grid: &mut Vec<f64>,
    l_states: &[crate::reconr::mf2::AaLState],
    el: f64,
    eh: f64,
) {
    const OFFSETS: &[f64] = &[
        -10.0, -5.0, -2.0, -1.0, -0.5, -0.25, 0.0, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0,
    ];
    for ls in l_states {
        for res in &ls.resonances {
            if res.de_total <= 0.0 {
                continue;
            }
            let gt = if res.dw_total != 0.0 {
                res.dw_total.abs()
            } else {
                res.dw_fission.abs().max(res.dw_capture.abs())
            };
            let half_g = gt / 2.0;
            grid.push(res.de_total);
            for &off in OFFSETS {
                let e = res.de_total + off * half_g;
                if e > el && e < eh && e > 0.0 {
                    grid.push(e);
                }
            }
        }
    }
}

/// Run the RECONR card-input driver (NJOY module entry point).
///
/// **Status:** this module's processing physics is ported (see its `README.md`
/// and the typed API above); the NJOY *card-input driver* itself is not yet
/// ported, so this returns [`crate::NjoyError::NotPorted`]. Use the module's
/// typed API directly rather than this driver.
pub fn run() -> Result<(), crate::NjoyError> {
    Err(crate::NjoyError::NotPorted(
        "reconr driver (physics ported — use the module API)",
    ))
}
