// SPDX-License-Identifier: GPL-3.0

//! **Writing the ACE delayed-neutron blocks: DNU, BDD, DNEDL, DNED.**
//!
//! [`super::delayed`] reads these blocks (GitHub #307); this writes them, so a
//! table this crate produces carries the delayed-neutron data NJOY's does. Found
//! missing by `examples/ace_blocks_vs_reference.rs`, which reported all four
//! blocks — and `NXS(8)` — absent from every table this crate wrote against
//! NJOY2016's own U-234/235/238.
//!
//! # Upstream is the specification
//!
//! Ported from `acefc.f90:5997-6253` (the "store delayed neutron data" block of
//! `acelod`) with the Type-1 integer typing of `:13461-13530`, NJOY2016 as held
//! in `upstream_source/NJOY2016`:
//!
//! | block | JXS | content |
//! |---|---|---|
//! | DNU | 24 | delayed ν̄_d(E), the NU block's `LNU = 2` layout |
//! | BDD | 25 | per group: decay constant **per shake**, then its probability TAB1 |
//! | DNEDL | 26 | one DNED-relative locator per group |
//! | DNED | 27 | per group: one ACE law-4 spectrum behind a `[LNW, LAW, IDAT]` header |
//!
//! `NXS(8)` is the group count. The blocks are written only when the evaluation
//! has **both** MF=1/MT=455 and MF=5/MT=455 (`:6003`); with ν̄_d but no spectra
//! upstream prints "delayed neutron data supressed" and writes none, and so does
//! this.
//!
//! # The details that decide whether the words match
//!
//! - **Decay constants go per shake**: `λ[s⁻¹] / 1e8` (`shake = 1.e8`, `:4931`).
//! - **A single lin-lin probability range is written `NR = 0`** (`:6041`).
//! - **`LF = 5` spectra carry no incident-energy dependence**, so upstream writes
//!   the one distribution **twice**, at the probability table's first and last
//!   energies (`:6187-6207`). The θ(E) TAB1 is read and discarded.
//! - **`ismooth = 1` (the default, `acer.f90:326`)** extends a *histogram*
//!   spectrum's lowest bin with a √E shape while its upper edge is above 40 eV,
//!   splitting at `0.8409 ×` the edge each time (`:6172-6186`).
//! - **CDFs are accumulated from the unrounded ENDF values** and rounded to 9
//!   figures, pdfs to 7; if the integral is off unity by more than 1e-8 both are
//!   divided by it, and for `LF = 5` a renormalised CDF inside
//!   `[0.999999997, 1.0001]` — and the last point, if below 1 — is pinned to 1.

use crate::endf::records::{SectionCursor, Tab1};
use crate::endf::tape::Tape;
use crate::NjoyError;

use super::nu::NuRepr;

const EMEV: f64 = 1.0e6;
/// `acefc.f90:4931` — `shake=1.e8`, so `λ/shake` is λ per shake.
const SHAKE: f64 = 1.0e8;

/// NJOY's `sigfig(x, n, 0)`, faithfully — `util.f90:361-393` via the crate's
/// one port of it. The 9-figure CDF needs `n = 9`, which is why this does not
/// go through `acer::build::sigfig` (fixed at the call site's `n`, same port).
fn sigfig(x: f64, n: i32) -> f64 {
    if x == 0.0 || !x.is_finite() {
        return x;
    }
    crate::mixr::mix::sigfig(x, n, 0)
}

/// The four delayed-neutron blocks, as `(value, is_integer)` words, ready to be
/// appended to XSS in order. Locators inside DNEDL/DNED are **block-relative**
/// (1-based from the start of DNED), exactly as upstream stores them, so the
/// blocks can be placed anywhere.
#[derive(Debug, Clone, Default)]
pub struct DelayedBlocks {
    /// DNU words.
    pub dnu: Vec<(f64, bool)>,
    /// BDD words.
    pub bdd: Vec<(f64, bool)>,
    /// DNEDL words (one integer locator per group).
    pub dnedl: Vec<(f64, bool)>,
    /// DNED words.
    pub dned: Vec<(f64, bool)>,
    /// `NXS(8)`, the number of precursor groups.
    pub n_groups: usize,
}

/// Build the delayed-neutron blocks for `mat`, or `Ok(None)` when upstream would
/// write none: no MF=1/MT=455, or no MF=5/MT=455 to go with it.
///
/// `ismooth` is ACER's card-7 switch; `true` is upstream's default.
///
/// # Errors
///
/// `NotPorted` for the forms upstream handles here that no held evaluation uses
/// and this port has not been checked against: `LNU = 1` delayed ν̄ (upstream's
/// spontaneous-fission branch) and energy-dependent decay constants (`LDG = 1`).
/// And an `LF` other than 1 or 5, which upstream silently skips — leaving that
/// group's spectrum empty — and which this refuses rather than reproduce.
pub fn build(tape: &Tape, mat: i32, ismooth: bool) -> Result<Option<DelayedBlocks>, NjoyError> {
    let (Some(mf1), Some(mf5)) = (tape.section(mat, 1, 455), tape.section(mat, 5, 455)) else {
        return Ok(None);
    };

    // ── DNU and the decay constants (MF=1/MT=455) ───────────────────────────
    let mut c1 = SectionCursor::new(&mf1.rows);
    let head = c1.read_cont()?; // HEAD: L1 = LDG, L2 = LNU
    let (ldg, lnu) = (head.l1, head.l2);
    if lnu != 2 {
        return Err(NjoyError::NotPorted(
            "ACE delayed blocks from an LNU=1 (polynomial) MF=1/MT=455: upstream's \
             spontaneous-fission branch (acefc.f90:5172-5191), unused by every held \
             evaluation",
        ));
    }
    if ldg != 0 {
        return Err(NjoyError::NotPorted(
            "ACE delayed blocks with energy-dependent decay constants (LDG=1): upstream \
             reads the decay-constant LIST assuming LDG=0 here",
        ));
    }
    let lambdas = c1.read_list()?.data; // NNF decay constants [1/s]
    let dnu_repr = NuRepr::from_endf(tape, mat, 455)?
        .ok_or_else(|| NjoyError::EndfParse("MF=1/MT=455 present but unreadable".into()))?;
    let dnu = typed_nu(&dnu_repr);

    // ── MF=5/MT=455: probabilities and spectra ──────────────────────────────
    let mut c5 = SectionCursor::new(&mf5.rows);
    let head5 = c5.read_cont()?;
    let ndnf = head5.n1.max(0) as usize;
    if ndnf != lambdas.len() {
        return Err(NjoyError::EndfParse(format!(
            "MF=5/MT=455 has {ndnf} precursor groups but MF=1/MT=455 gives {} decay \
             constants",
            lambdas.len()
        )));
    }

    let mut bdd: Vec<(f64, bool)> = Vec::new();
    let mut dnedl: Vec<(f64, bool)> = Vec::with_capacity(ndnf);
    let mut dned: Vec<(f64, bool)> = Vec::new();

    for (g, &lambda) in lambdas.iter().enumerate() {
        let p = c5.read_tab1()?; // p_g(E); L2 = LF
        let lf = p.head.l2;

        // BDD entry (`:6034-6056`).
        bdd.push((lambda / SHAKE, false));
        push_interp(&mut bdd, &p);
        bdd.push((p.pairs.len() as f64, true));
        for &(e, _) in &p.pairs {
            bdd.push((sigfig(e / EMEV, 7), false));
        }
        for &(_, v) in &p.pairs {
            bdd.push((sigfig(v, 7), false));
        }

        let xxmin = p.pairs.first().map_or(0.0, |x| x.0);
        let xxmax = p.pairs.last().map_or(0.0, |x| x.0);

        // DNED entry. `rel` is this group's 1-based offset within DNED.
        let rel = dned.len() + 1;
        dnedl.push((rel as f64, true));
        // Header: LNW, LAW=4, IDAT, then a flat applicability (`:6107-6115`).
        dned.push((0.0, true));
        dned.push((4.0, true));
        dned.push(((rel + 9) as f64, true));
        dned.push((0.0, true));
        dned.push((2.0, true));
        dned.push((sigfig(xxmin / EMEV, 7), false));
        dned.push((sigfig(xxmax / EMEV, 7), false));
        dned.push((1.0, false));
        dned.push((1.0, false));

        match lf {
            1 => {
                // Tabulated: a TAB2 of NE incident energies, each a TAB1.
                let t2 = c5.read_tab2()?;
                let ne = t2.head.n2.max(0) as usize;
                dned.push((0.0, true)); // NR
                dned.push((ne as f64, true)); // NE
                let e_at = dned.len();
                for _ in 0..2 * ne {
                    dned.push((0.0, false)); // energies, then locators; filled below
                }
                for ie in 0..ne {
                    let g_tab = c5.read_tab1()?;
                    dned[e_at + ie] = (sigfig(g_tab.head.c2 / EMEV, 7), false);
                    dned[e_at + ne + ie] = ((dned.len() + 1) as f64, true);
                    let iint = g_tab.interp.first().map_or(2, |&(_, i)| i);
                    push_spectrum(&mut dned, &g_tab.pairs, iint, false);
                }
            }
            5 => {
                let _theta = c5.read_tab1()?; // read and discarded, as upstream
                let g_tab = c5.read_tab1()?;
                let iint = g_tab.interp.first().map_or(2, |&(_, i)| i);
                let mut pairs = g_tab.pairs.clone();
                if ismooth && iint == 1 && g_tab.interp.len() == 1 {
                    extend_lowest_bin(&mut pairs);
                }
                // No incident-energy dependence: the same distribution twice,
                // at the probability table's first and last energies.
                dned.push((0.0, true)); // NR
                dned.push((2.0, true)); // NE
                dned.push((sigfig(xxmin / EMEV, 7), false));
                dned.push((sigfig(xxmax / EMEV, 7), false));
                let mm = pairs.len();
                let first = dned.len() + 2 + 1;
                dned.push((first as f64, true));
                dned.push(((first + 2 + 3 * mm) as f64, true));
                let spec = spectrum_words(&pairs, iint, true);
                dned.extend_from_slice(&spec);
                dned.extend_from_slice(&spec);
            }
            other => {
                return Err(NjoyError::NotPorted(if other == 0 {
                    "ACE delayed blocks: MF=5/MT=455 subsection with LF=0"
                } else {
                    "ACE delayed blocks: an MF=5/MT=455 LF other than 1 or 5 -- upstream \
                     skips it and leaves the group's spectrum empty, which this refuses \
                     to reproduce"
                }));
            }
        }
        let _ = g;
    }

    Ok(Some(DelayedBlocks {
        dnu,
        bdd,
        dnedl,
        dned,
        n_groups: ndnf,
    }))
}

/// DNU in the NU block's layout, with upstream's typing: LNU, NR, NBT, INT and
/// NE integers; energies and ν̄ reals (`:13465-13477`).
fn typed_nu(r: &NuRepr) -> Vec<(f64, bool)> {
    match r {
        NuRepr::Tabulated { nbt, int, e_mev, nu } => {
            let mut w = vec![(2.0, true), (nbt.len() as f64, true)];
            w.extend(nbt.iter().map(|&v| (v, true)));
            w.extend(int.iter().map(|&v| (v, true)));
            w.push((e_mev.len() as f64, true));
            w.extend(e_mev.iter().map(|&v| (v, false)));
            w.extend(nu.iter().map(|&v| (v, false)));
            w
        }
        NuRepr::Polynomial(c) => {
            let mut w = vec![(1.0, true), (c.len() as f64, true)];
            w.extend(c.iter().map(|&v| (v, false)));
            w
        }
    }
}

/// `NR` and, unless the table is one lin-lin range, the `(NBT, INT)` lists
/// (`:6041-6049`).
fn push_interp(w: &mut Vec<(f64, bool)>, t: &Tab1) {
    let single_linlin = t.interp.len() == 1 && t.interp[0].1 == 2;
    if single_linlin || t.interp.is_empty() {
        w.push((0.0, true));
    } else {
        w.push((t.interp.len() as f64, true));
        for &(nbt, _) in &t.interp {
            w.push((f64::from(nbt), true));
        }
        for &(_, int) in &t.interp {
            w.push((f64::from(int), true));
        }
    }
}

/// Append one law-4 outgoing distribution: `INTT, NP, E'(NP), pdf(NP), cdf(NP)`.
fn push_spectrum(w: &mut Vec<(f64, bool)>, pairs: &[(f64, f64)], iint: u32, pin: bool) {
    w.extend(spectrum_words(pairs, iint, pin));
}

/// The distribution's words. `pin` applies the `LF = 5` rule that a renormalised
/// CDF within `[0.999999997, 1.0001]`, and a last point below 1, become exactly 1
/// (`:6238-6247`); the `LF = 1` branch does not (`:6149-6158`).
fn spectrum_words(pairs: &[(f64, f64)], iint: u32, pin: bool) -> Vec<(f64, bool)> {
    let mm = pairs.len();
    let mut e = Vec::with_capacity(mm);
    let mut pdf = Vec::with_capacity(mm);
    let mut cdf = Vec::with_capacity(mm);
    let mut sumup = 0.0f64;
    for j in 0..mm {
        let (x, y) = pairs[j];
        e.push(sigfig(x / EMEV, 7));
        pdf.push(sigfig(y * EMEV, 7));
        cdf.push(sigfig(sumup, 9));
        if j + 1 < mm {
            let (x2, y2) = pairs[j + 1];
            if iint == 1 {
                sumup += (x2 - x) * y;
            } else if iint == 2 {
                sumup += (x2 - x) * (y2 + y) / 2.0;
            }
        }
    }
    if 100_000_000.0 * (sumup - 1.0).abs() > 1.0 {
        for j in 0..mm {
            pdf[j] = sigfig(pdf[j] / sumup, 7);
            cdf[j] = sigfig(cdf[j] / sumup, 9);
            if pin && (0.999_999_997..=1.000_1).contains(&cdf[j]) {
                cdf[j] = 1.0;
            }
        }
        if pin && cdf.last().is_some_and(|&c| c < 1.0) {
            *cdf.last_mut().expect("non-empty") = 1.0;
        }
    }
    let mut w = Vec::with_capacity(2 + 3 * mm);
    w.push((f64::from(iint), true));
    w.push((mm as f64, true));
    w.extend(e.into_iter().map(|v| (v, false)));
    w.extend(pdf.into_iter().map(|v| (v, false)));
    w.extend(cdf.into_iter().map(|v| (v, false)));
    w
}

/// `ismooth`'s √E extension of a histogram's lowest bin (`acefc.f90:6172-6186`):
/// while the first bin's upper edge is above 40 eV, split it at `0.8409 ×` that
/// edge, giving the lower part `√0.8409 ×` the density and the upper part
/// `(1 − 0.8409^1.5)/(1 − 0.8409) ×` it.
fn extend_lowest_bin(pairs: &mut Vec<(f64, f64)>) {
    const EX: f64 = 40.0;
    // `fx=.8409` (`acefc.f90:6183`) is a Fortran literal with NO kind suffix,
    // i.e. a SINGLE-precision constant, stored into a double (`real(kr)::fx`,
    // `:4923`). Upstream therefore splits at 0.8409000039100647, not 0.8409 --
    // 4.65e-9 relative, which compounds over the ~40 splits a keV-wide lowest
    // bin needs and left 584 of U-234's 10 920 DNED words one unit off in the
    // 7th figure until it was reproduced here. Same literal at `:7555`.
    const FX: f64 = 0.8409_f32 as f64;
    while pairs.len() >= 2 && pairs[1].0 > EX {
        let (x1, y1) = pairs[0];
        let x2_old = pairs[1].0;
        let new_x2 = FX * x2_old;
        let new_y1 = FX.sqrt() * y1;
        let new_y2 = (1.0 - FX * FX.sqrt()) * y1 / (1.0 - FX);
        pairs[0] = (x1, new_y1);
        pairs.insert(1, (new_x2, new_y2));
    }
}
