// Ported from NJOY2016 `src/acedo.f90` (subroutines `acedos`, lines 31-295,
// and `dosout`, lines 480-598).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! ACER's **dosimetry** path — upstream `acer` with `iopt = 3`.
//!
//! A dosimetry table (`class y`) is the simplest ACE class there is: no energy
//! grid of its own, no angular or energy distributions, no heating. Three
//! blocks, and each reaction carries its own grid:
//!
//! | block | JXS | contents |
//! |---|---|---|
//! | MTR | 3 | `ntr` reaction numbers |
//! | LSIG | 6 | `ntr` **1-based offsets into SIGD**, not absolute locators |
//! | SIGD | 7 | per reaction: `NR`, the interpolation table if `NR != 0`, `NE`, then `NE` energies \[MeV\] followed by `NE` cross sections |
//!
//! The input is a **PENDF**, not a raw evaluation: `acedos` searches it for
//! the material whose MF=1/MT=451 temperature matches the requested one
//! (`acedo.f90:80-104`) and reads MF=3 — and then MF=10 — from that block.
//!
//! ## Three upstream behaviours a reader of this module should know
//!
//! 1. **`NR = 1` with `INT = 2` is stored as `NR = 0`.** Lin-lin over one
//!    region is the default, so upstream writes a single zero instead of the
//!    region table (`:145-157`). Anything else is stored in full.
//! 2. **MF=3 and MF=10 store that region table differently.** The MF=3 branch
//!    writes the `NBT` values and *then* the `INT` values (`:150-153`); the
//!    MF=10 branch interleaves them as `(NBT, INT)` pairs (`:220-223`). Both
//!    are `2 * NR` words and the writer cannot tell them apart, so a consumer
//!    reading an MF=3-derived table as pairs gets nonsense. This port
//!    reproduces both, because that is what the files say.
//! 3. **MF=10 reactions get a synthetic MT**, `MT + 1000 * (10 + LFS)`
//!    (`:198`), so an isomeric production channel is distinguishable.
//!
//! ## And one that looks like a defect
//!
//! With `tempd = 0` the temperature-search loop at `:83` never executes —
//! its condition is `|temp - tempd| >= tempd/100 + 1`, which is `0 >= 1` —
//! so `za` and `awr` are never read and the table comes out with ZAID
//! `0.00y` and `AWR = 0`. Verified against NJOY2016 on 2026-09-21: asking for
//! a 0 K dosimetry table really does produce `hz = "     0.00y"`. This port
//! **does not** reproduce that; it returns an error naming the temperature,
//! because silently writing a table with no ZAID is not a behaviour worth
//! being bit-compatible with. That is the one deliberate divergence here and
//! it is recorded in
//! `verification_and_validation/acer_dosimetry_vs_njoy2016.md`.

use crate::acer::read::{AceClass, AceFileType, AceHeader, RawAceTable};
use crate::endf::records::SectionCursor;
use crate::endf::tape::Tape;
use crate::NjoyError;

/// `emev` (`acedo.f90:52`).
const EMEV: f64 = 1.0e6;
/// Boltzmann's constant \[eV/K\] — `bk` from `physics`, used for `tz = T k / 1e6`.
const BK_EV_PER_K: f64 = 8.617_333_262e-5;
/// `nmax` (`:116`) — upstream's reaction cap.
const NMAX: usize = 350;

/// Everything `acedos` needs that is not on the tape.
#[derive(Debug, Clone, Default)]
pub struct DosimetryOptions {
    /// `suff` — ZAID suffix.
    pub suffix: f64,
    /// `hk` — the 70-character comment.
    pub comment: String,
    /// `hd` — the `mm/dd/yy` body of the processing date.
    pub date: String,
    /// `mcnpx` — 13-character ZAID, `f10.3,'ny '` (`:268-272`).
    pub mcnpx: bool,
    /// The 16 `IZ` entries.
    pub iz: [i32; 16],
    /// The 16 `AW` entries.
    pub aw: [f64; 16],
}

/// A built dosimetry table, before it is turned into a writable
/// [`RawAceTable`].
#[derive(Debug, Clone)]
pub struct DosimetryAce {
    /// `matd` — the ENDF material number, carried through to `hm`.
    pub mat: i32,
    /// `za`.
    pub za: i32,
    /// `aw0` — atomic weight ratio.
    pub awr: f64,
    /// `tz` — `T k_B` \[MeV\].
    pub kt_mev: f64,
    /// The temperature the tape actually supplied \[K\].
    pub temperature_k: f64,
    /// `ntr` — reaction count.
    pub ntr: usize,
    /// NXS(1..16).
    pub nxs: [i32; 16],
    /// JXS(1..32).
    pub jxs: [i32; 32],
    /// XSS.
    pub xss: Vec<f64>,
    /// Which XSS words `dosout` writes as integers (`:520-550`).
    pub xss_is_int: Vec<bool>,
}

/// One reaction's stored block, before the locators are squeezed.
struct Reaction {
    mt: i32,
    /// `(NBT, INT)` regions, empty when upstream collapses them to `NR = 0`.
    regions: Vec<(u32, u32)>,
    /// True when the regions are stored interleaved (the MF=10 convention).
    interleaved: bool,
    energies_mev: Vec<f64>,
    sigma: Vec<f64>,
}

impl Reaction {
    /// The words this reaction contributes to SIGD, and their integer flags.
    fn emit(&self, xss: &mut Vec<f64>, is_int: &mut Vec<bool>) {
        let nr = self.regions.len();
        xss.push(nr as f64);
        is_int.push(true);
        if nr != 0 {
            if self.interleaved {
                for &(nbt, int) in &self.regions {
                    xss.push(nbt as f64);
                    xss.push(int as f64);
                    is_int.push(true);
                    is_int.push(true);
                }
            } else {
                for &(nbt, _) in &self.regions {
                    xss.push(nbt as f64);
                    is_int.push(true);
                }
                for &(_, int) in &self.regions {
                    xss.push(int as f64);
                    is_int.push(true);
                }
            }
        }
        xss.push(self.energies_mev.len() as f64);
        is_int.push(true);
        for &e in &self.energies_mev {
            xss.push(e);
            is_int.push(false);
        }
        for &s in &self.sigma {
            xss.push(s);
            is_int.push(false);
        }
    }
}

/// The MF=1/MT=451 temperature of the material block starting at `sections[i]`.
fn block_temperature(tape: &Tape, idx: usize) -> Option<f64> {
    let sec = tape.sections().get(idx)?;
    let mut cur = SectionCursor::new(&sec.rows);
    // HEAD (ZA, AWR, ...), then ENDF-6's two extra CONTs, then the one whose
    // C1 is TEMP (`acedo.f90:98`).
    let _head = cur.read_cont().ok()?;
    let _ = cur.read_cont().ok()?;
    let _ = cur.read_cont().ok()?;
    Some(cur.read_cont().ok()?.c1)
}

/// Build a dosimetry ACE table from a PENDF.
///
/// `temperature_k` selects the material block the way `acedos` does: the first
/// whose MF=1/MT=451 temperature satisfies `|T - Td| < Td/100 + 1`.
///
/// # Errors
/// [`NjoyError::EndfParse`] when `mat` is absent, when no block matches the
/// temperature, when the tape carries no MF=3 or MF=10 reaction other than
/// MT=1, or when more than `nmax = 350` reactions are found (upstream's own
/// cap, which it reports as a fatal error).
pub fn dosimetry_ace(
    tape: &Tape,
    mat: i32,
    temperature_k: f64,
    opts: &DosimetryOptions,
) -> Result<DosimetryAce, NjoyError> {
    if temperature_k == 0.0 {
        return Err(NjoyError::EndfParse(
            "acedos: a 0 K dosimetry table is refused. Upstream's temperature \
             search (acedo.f90:83) cannot run at tempd = 0, so it never reads \
             ZA or AWR and writes a table with ZAID \"0.00y\" and AWR 0; this \
             port will not produce that silently. Ask for the PENDF's own \
             temperature instead."
                .into(),
        ));
    }
    let tol = temperature_k / 100.0 + 1.0;
    // Walk the tape in file order so a multi-temperature PENDF -- which
    // repeats the whole material, MF=1 included -- is searched rather than
    // collapsed to its first block.
    let sections = tape.sections();
    let mut chosen: Option<(usize, f64)> = None;
    let mut starts: Vec<usize> = Vec::new();
    for (i, s) in sections.iter().enumerate() {
        if s.key.mat == mat && s.key.mf == 1 && s.key.mt == 451 {
            starts.push(i);
        }
    }
    if starts.is_empty() {
        return Err(NjoyError::EndfParse(format!(
            "acedos: no MF=1/MT=451 for MAT {mat}; a dosimetry run reads a PENDF"
        )));
    }
    for &i in &starts {
        if let Some(t) = block_temperature(tape, i) {
            if (t - temperature_k).abs() < tol {
                chosen = Some((i, t));
                break;
            }
        }
    }
    let (start, temp) = chosen.ok_or_else(|| {
        NjoyError::EndfParse(format!(
            "acedos: MAT {mat} has no block at {temperature_k} K \
             (tolerance {tol} K); the tape carries {:?}",
            starts
                .iter()
                .filter_map(|&i| block_temperature(tape, i))
                .collect::<Vec<_>>()
        ))
    })?;
    // The material's own HEAD carries ZA and AWR (`:89-90`).
    let head = {
        let sec = &sections[start];
        SectionCursor::new(&sec.rows).read_cont()?
    };
    let za = head.c1.round() as i32;
    let awr = head.c2;

    // Everything from this block until the next MF=1/MT=451 of this MAT.
    let end = starts
        .iter()
        .copied()
        .find(|&i| i > start)
        .unwrap_or(sections.len());

    let mut reactions: Vec<Reaction> = Vec::new();
    for s in sections[start..end].iter() {
        if s.key.mat != mat || (s.key.mf != 3 && s.key.mf != 10) || s.key.mt == 1 {
            continue;
        }
        let mut cur = SectionCursor::new(&s.rows);
        let hd = cur.read_cont()?;
        if s.key.mf == 3 {
            let t = cur.read_tab1()?;
            reactions.push(tab1_to_reaction(s.key.mt, &t, false));
        } else {
            // MF=10: `ns = max(1, N1)` subsections, each a TAB1 whose L2 is
            // LFS (`:165-198`).
            let ns = hd.n1.max(1);
            for _ in 0..ns {
                let t = cur.read_tab1()?;
                let lfs = t.head.l2;
                reactions.push(tab1_to_reaction(s.key.mt + 1000 * (10 + lfs), &t, true));
            }
        }
        if reactions.len() > NMAX {
            return Err(NjoyError::EndfParse(format!(
                "acedos: more than nmax = {NMAX} reactions for MAT {mat}"
            )));
        }
    }
    if reactions.is_empty() {
        return Err(NjoyError::EndfParse(format!(
            "acedos: MAT {mat} carries no MF=3 or MF=10 reaction other than MT=1"
        )));
    }

    // ── assemble, with LSIG already squeezed to SIGD-relative offsets ───────
    let ntr = reactions.len();
    let mut sigd_words: Vec<f64> = Vec::new();
    let mut sigd_is_int: Vec<bool> = Vec::new();
    let mut offsets: Vec<f64> = Vec::with_capacity(ntr);
    for r in &reactions {
        offsets.push(sigd_words.len() as f64 + 1.0);
        r.emit(&mut sigd_words, &mut sigd_is_int);
    }
    let mut xss: Vec<f64> = Vec::with_capacity(2 * ntr + sigd_words.len());
    let mut xss_is_int: Vec<bool> = Vec::with_capacity(xss.capacity());
    for r in &reactions {
        xss.push(r.mt as f64);
        xss_is_int.push(true);
    }
    xss.extend_from_slice(&offsets);
    xss_is_int.extend(std::iter::repeat_n(true, ntr));
    xss.extend_from_slice(&sigd_words);
    xss_is_int.extend_from_slice(&sigd_is_int);

    let mtr = 1usize;
    let lsig = mtr + ntr;
    let sigd = mtr + 2 * ntr;
    let len2 = xss.len();
    let mut nxs = [0i32; 16];
    nxs[0] = len2 as i32;
    nxs[1] = za;
    nxs[3] = ntr as i32;
    let mut jxs = [0i32; 32];
    jxs[0] = 1; // lone
    jxs[2] = mtr as i32;
    jxs[5] = lsig as i32;
    jxs[6] = sigd as i32;
    jxs[21] = len2 as i32; // `end`, JXS(22)

    let _ = opts;
    Ok(DosimetryAce {
        mat,
        za,
        awr,
        kt_mev: temp * BK_EV_PER_K / EMEV,
        temperature_k: temp,
        ntr,
        nxs,
        jxs,
        xss,
        xss_is_int,
    })
}

/// Turn one MF=3 / MF=10 TAB1 into the stored form, applying upstream's
/// `NR = 1, INT = 2` collapse (`acedo.f90:145-157`).
fn tab1_to_reaction(mt: i32, t: &crate::endf::records::Tab1, interleaved: bool) -> Reaction {
    let nr = t.interp.len();
    let int0 = t.interp.first().map(|&(_, i)| i).unwrap_or(2);
    let regions = if nr == 1 && int0 == 2 {
        Vec::new()
    } else {
        t.interp.clone()
    };
    Reaction {
        mt,
        regions,
        interleaved,
        energies_mev: t.pairs.iter().map(|&(x, _)| x / EMEV).collect(),
        sigma: t.pairs.iter().map(|&(_, y)| y).collect(),
    }
}

impl DosimetryAce {
    /// Assemble the writable table — header, NXS/JXS and XSS with the integer
    /// mask `dosout` implies.
    pub fn into_raw(self, opts: &DosimetryOptions, file_type: AceFileType) -> RawAceTable {
        let zaid_num = self.za as f64 + opts.suffix;
        let zaid = if opts.mcnpx {
            format!("{zaid_num:10.3}ny ")
        } else {
            format!("{zaid_num:9.2}y")
        };
        let date = format!("  {:<8}", opts.date);
        let mat_id = format!("   mat{:4}", self.mat);
        let comment = format!("{:<70}", opts.comment);
        RawAceTable {
            file_type,
            header: AceHeader {
                raw_text: [
                    zaid.as_bytes().to_vec(),
                    date.as_bytes().to_vec(),
                    comment.as_bytes().to_vec(),
                    mat_id.as_bytes().to_vec(),
                ],
                zaid: zaid.trim().to_string(),
                zaid_num: Some(zaid_num),
                class: AceClass::Dosimetry,
                awr: self.awr,
                kt_mev: self.kt_mev,
                date: opts.date.clone(),
                comment: opts.comment.clone(),
                mat_id: mat_id.trim().to_string(),
                iz: opts.iz,
                aw: opts.aw,
            },
            nxs: self.nxs,
            jxs: self.jxs,
            xss_is_int: Some(self.xss_is_int),
            xss: self.xss,
        }
    }

    /// Reaction `i`'s MT number.
    pub fn mt(&self, i: usize) -> i32 {
        self.xss[i].round() as i32
    }

    /// Reaction `i`'s `(energies_MeV, sigma)`, read back through the locators
    /// the same way a consumer would.
    pub fn reaction(&self, i: usize) -> (Vec<f64>, Vec<f64>) {
        let sigd = self.jxs[6] as usize;
        let off = self.xss[self.ntr + i] as usize;
        let mut p = sigd - 1 + off - 1;
        let nr = self.xss[p] as usize;
        p += 1 + 2 * nr;
        let ne = self.xss[p] as usize;
        p += 1;
        (
            self.xss[p..p + ne].to_vec(),
            self.xss[p + ne..p + 2 * ne].to_vec(),
        )
    }
}
