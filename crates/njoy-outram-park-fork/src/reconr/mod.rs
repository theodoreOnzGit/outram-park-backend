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
//! | 2c | Reich-Moore (LRF=3, e.g. U-235) | **this version** |
//! | 2d | Unresolved resonances (LRU=2, PURR) | future |
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

pub mod linearize;
pub mod mf1;
pub mod mf2;
pub mod rm;
pub mod slbw;

pub use mf1::MaterialInfo;
pub use mf2::{EnergyRange, LState, ResonanceFormalism, ResonanceInfo, RmLState, RmResonance, SlbwResonance};

use crate::{
    endf::{records::SectionCursor, tape::Tape, MtReaction},
    NjoyError,
};
use rm::RmSigmas;
use slbw::{channel_radius, eval_slbw_lstate};

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

/// Result of running RECONR on one material.
#[derive(Debug, Clone)]
pub struct ReconrResult {
    /// Material header from MF=1/MT=451.
    pub material: MaterialInfo,
    /// Reconstructed MF=3 sections, sorted by MT.
    pub sections: Vec<ReconrSection>,
}

impl ReconrResult {
    /// Evaluate cross section \[b\] for a reaction at energy `e` \[eV\].
    ///
    /// Uses linear interpolation on the lin-lin grid. Returns `0.0` if
    /// `mt` is not present or `e` is outside the tabulated range.
    pub fn eval_mt(&self, mt: MtReaction, e: f64) -> f64 {
        let sec = match self.sections.iter().find(|s| s.mt == mt) {
            Some(s) => s,
            None    => return 0.0,
        };
        eval_lin_lin(&sec.pairs, e)
    }
}

/// Linear interpolation on a sorted lin-lin (x,y) grid.
///
/// Returns the linearly interpolated y at `x`. Returns `0.0` if the grid is empty
/// or `x` is outside [x_min, x_max].
pub fn eval_lin_lin(pairs: &[(f64, f64)], x: f64) -> f64 {
    if pairs.is_empty() { return 0.0; }
    if x <= pairs[0].0 { return pairs[0].1; }
    if x >= pairs[pairs.len()-1].0 { return pairs[pairs.len()-1].1; }

    let idx = pairs.partition_point(|&(xi, _)| xi <= x);
    if idx == 0 { return pairs[0].1; }
    let (x0, y0) = pairs[idx - 1];
    let (x1, y1) = pairs[idx];
    if (x1 - x0).abs() < 1e-30 { return y0; }
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
/// - [`NjoyError::NotPorted`] — MF=2 contains LRF=4 (Adler-Adler) or LRF=7 (R-Matrix Limited).
pub fn reconr(tape: &Tape, config: &ReconrConfig) -> Result<ReconrResult, NjoyError> {
    let mat = config.mat;
    let eps = config.tolerance;

    // MF=1/MT=451 — material header
    let mf1_sec = tape.section(mat, 1, 451)
        .ok_or(NjoyError::SectionNotFound { mat, mf: 1, mt: 451 })?;
    let material = mf1::parse_material_info(mf1_sec)?;

    // MF=2/MT=151 — resonance parameters (may be absent for charged-particle data)
    let res_info = if let Some(mf2_sec) = tape.section(mat, 2, 151) {
        mf2::parse_resonance_info(mf2_sec)?
    } else {
        ResonanceInfo::default()
    };

    // MF=3 — background cross sections
    let mut sections: Vec<ReconrSection> = tape
        .sections()
        .iter()
        .filter(|s| s.key.mat == mat && s.key.mf == 3)
        .map(|sec| {
            let mut cur = SectionCursor::new(&sec.rows);
            let _head = cur.read_cont()?; // MF=3 HEAD: ZA, AWR, 0, 0, 0, 0
            let tab1 = cur.read_tab1()?;  // TAB1 header carries QM (c1), QI (c2)
            let pairs = linearize::linearize_tab1(&tab1.interp, &tab1.pairs, eps);
            Ok(ReconrSection {
                mt: MtReaction::from_any(sec.key.mt),
                qi: tab1.head.c2,
                pairs,
            })
        })
        .collect::<Result<Vec<_>, NjoyError>>()?;

    sections.sort_by_key(|s| i32::from(s.mt));

    // Phase 2b: add SLBW/MLBW resonance contributions
    add_resonance_contributions(&mut sections, &res_info);

    Ok(ReconrResult { material, sections })
}

/// Reconstruct **only the MF=3 background** — no MF=2 resonance contributions.
///
/// This is the fast-range path used by the MGXS bake
/// ([`crate::nuclear_data::Mgxs::collapse_from_reconr`]). Above a nuclide's WMP
/// `e_max` the incident energy is beyond the resolved-resonance region, so the
/// smooth linearised MF=3 grid *is* the cross section there. Skipping MF=2 avoids
/// reconstructing resonance formats this port does not yet handle (notably
/// ENDF/B-VIII.0's LRF=7 R-Matrix Limited), which do not affect the fast range.
///
/// Returns the same [`ReconrResult`] shape as [`reconr`] — linearised MF=3
/// sections sorted by MT — but with **no** resonance additions. Do **not** use it
/// below the resonance ceiling; there it omits the resonance cross section.
pub fn reconr_background(
    tape: &Tape,
    mat: i32,
    tolerance: f64,
) -> Result<ReconrResult, NjoyError> {
    let mf1_sec = tape
        .section(mat, 1, 451)
        .ok_or(NjoyError::SectionNotFound { mat, mf: 1, mt: 451 })?;
    let material = mf1::parse_material_info(mf1_sec)?;

    let mut sections: Vec<ReconrSection> = tape
        .sections()
        .iter()
        .filter(|s| s.key.mat == mat && s.key.mf == 3)
        .map(|sec| {
            let mut cur = SectionCursor::new(&sec.rows);
            let _head = cur.read_cont()?; // MF=3 HEAD
            let tab1 = cur.read_tab1()?; // TAB1 header carries QM (c1), QI (c2)
            let pairs = linearize::linearize_tab1(&tab1.interp, &tab1.pairs, tolerance);
            Ok(ReconrSection {
                mt: MtReaction::from_any(sec.key.mt),
                qi: tab1.head.c2,
                pairs,
            })
        })
        .collect::<Result<Vec<_>, NjoyError>>()?;
    sections.sort_by_key(|s| i32::from(s.mt));
    Ok(ReconrResult { material, sections })
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
fn add_resonance_contributions(sections: &mut Vec<ReconrSection>, res_info: &ResonanceInfo) {
    for range in res_info.resolved_slbw_ranges() {
        add_slbw_range(sections, range);
    }
    for range in res_info.resolved_rm_ranges() {
        add_rm_range(sections, range);
    }
}

/// Resonance cross-section contributions at one energy, by reaction \[b\].
#[derive(Debug, Default, Clone, Copy)]
struct RangeDelta {
    total:   f64,
    elastic: f64,
    fission: f64,
    capture: f64,
}

fn add_slbw_range(sections: &mut Vec<ReconrSection>, range: &EnergyRange) {
    if range.l_states.is_empty() { return; }

    // Hoist the per-l-state channel radius and resonance tuples out of the
    // energy loop — they do not depend on energy.
    let prepared: Vec<(u32, f64, f64, Vec<_>)> = range.l_states.iter().map(|ls| {
        let ra = channel_radius(ls.awri, range.naps, range.ap);
        let tuples: Vec<_> = ls.resonances.iter().map(|r| r.as_tuple()).collect();
        (ls.l, ls.awri, ra, tuples)
    }).collect();

    let mut halo = Vec::new();
    add_resonance_halo_energies(&mut halo, &range.l_states, range.el, range.eh);

    rebuild_range(sections, range.el, range.eh, halo, |e| {
        let mut d = RangeDelta::default();
        for (l, awri, ra, tuples) in &prepared {
            let s = eval_slbw_lstate(e, tuples, *l, range.spi, range.ap, *awri, *ra);
            d.elastic += s.elastic;
            d.capture += s.capture;
            d.fission += s.fission;
        }
        d.total = d.elastic + d.capture + d.fission;
        d
    });
}

fn add_rm_range(sections: &mut Vec<ReconrSection>, range: &EnergyRange) {
    if range.rm_l_states.is_empty() { return; }

    let mut halo = Vec::new();
    add_rm_halo_energies(&mut halo, &range.rm_l_states, range.el, range.eh);

    rebuild_range(sections, range.el, range.eh, halo, |e| {
        let mut d = RangeDelta::default();
        for ls in &range.rm_l_states {
            let s: RmSigmas = rm::eval_rm_lstate(e, ls, range.ap, range.spi, range.naps);
            d.total   += s.total;
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
fn rebuild_range(
    sections: &mut Vec<ReconrSection>,
    el: f64,
    eh: f64,
    halo: Vec<f64>,
    delta_at: impl Fn(f64) -> RangeDelta,
) {
    // Union energy grid inside [el, eh]: background energies + resonance halo.
    let mut egrid = collect_background_energies(sections, el, eh);
    egrid.extend(halo.into_iter().filter(|&e| e > el && e < eh && e > 0.0));
    egrid.sort_by(|a, b| a.partial_cmp(b).unwrap());
    egrid.dedup_by(|a, b| (*a - *b).abs() < 1e-10 * b.abs().max(1.0));
    if egrid.is_empty() { return; }

    // Evaluate the resonance contribution once per grid energy.
    let deltas: Vec<RangeDelta> = egrid.iter().map(|&e| delta_at(e)).collect();

    for mt in [MtReaction::Mt1Total, MtReaction::Mt2Elastic,
               MtReaction::Mt18Fission, MtReaction::Mt102Capture] {
        let sec = match sections.iter_mut().find(|s| s.mt == mt) {
            Some(s) => s,
            None    => continue,
        };

        // Immutable snapshot of the background for interpolation.
        let bg = sec.pairs.clone();

        // Keep points strictly outside the resonance range as-is.
        let mut new_pairs: Vec<(f64, f64)> =
            bg.iter().copied().filter(|&(e, _)| e < el || e > eh).collect();

        // Add background + resonance for every in-range grid energy.
        for (i, &e) in egrid.iter().enumerate() {
            let base = eval_lin_lin(&bg, e);
            let add = match mt {
                MtReaction::Mt1Total     => deltas[i].total,
                MtReaction::Mt2Elastic   => deltas[i].elastic,
                MtReaction::Mt18Fission  => deltas[i].fission,
                MtReaction::Mt102Capture => deltas[i].capture,
                _ => 0.0,
            };
            new_pairs.push((e, base + add));
        }

        new_pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        new_pairs.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-10 * b.0.abs().max(1.0));
        sec.pairs = new_pairs;
    }
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
    const OFFSETS: &[f64] = &[-10.0, -5.0, -2.0, -1.0, -0.5, -0.25,
                               0.0, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0];
    for ls in l_states {
        for res in &ls.resonances {
            if res.er <= 0.0 { continue; }
            let half_g = res.gt / 2.0;
            grid.push(res.er);
            for &off in OFFSETS {
                let e = res.er + off * half_g;
                if e > el && e < eh && e > 0.0 { grid.push(e); }
            }
        }
    }
}

/// Add a halo of energy points around each Reich-Moore resonance peak.
///
/// Total width is `Γ_n + Γ_γ + |Γ_fA| + |Γ_fB|`.
fn add_rm_halo_energies(grid: &mut Vec<f64>, rm_l_states: &[RmLState], el: f64, eh: f64) {
    const OFFSETS: &[f64] = &[-10.0, -5.0, -2.0, -1.0, -0.5, -0.25,
                               0.0, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0];
    for ls in rm_l_states {
        for res in &ls.resonances {
            if res.er <= 0.0 { continue; }
            let gt    = res.gn + res.gg + res.gfa.abs() + res.gfb.abs();
            let half_g = gt / 2.0;
            grid.push(res.er);
            for &off in OFFSETS {
                let e = res.er + off * half_g;
                if e > el && e < eh && e > 0.0 { grid.push(e); }
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
    Err(crate::NjoyError::NotPorted("reconr driver (physics ported — use the module API)"))
}
