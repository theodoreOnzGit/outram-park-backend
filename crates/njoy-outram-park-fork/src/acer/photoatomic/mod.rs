// Ported from NJOY2016 `src/acepa.f90` (subroutine `acepho`, lines 30-273, and
// the writer `phoout`, lines 916-1032).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! ACER's **photo-atomic** path — upstream `acer` with `iopt = 4`.
//!
//! A photo-atomic ACE table (`class p`) is a small, fixed-shape thing with no
//! reaction list and no secondary distributions. Five blocks:
//!
//! | block | JXS | contents |
//! |---|---|---|
//! | ESZG | 1 | `nes` × 5: energy, incoherent, coherent, photoelectric, pair — **stored as natural logs** |
//! | JINC | 2 | the incoherent scattering function `S(v, Z)` at 21 fixed `v` |
//! | JCOH | 3 | 55 cumulative integrals of `(F/Z)^2`, then `F(v, Z)` at the same 55 `v` |
//! | JFLO | 4 | `4 × nflo` fluorescence words (see [`fluorescence`]) |
//! | LHNM | 5 | `nes` heating numbers \[MeV per collision\] |
//!
//! NXS is `(len2, Z, nes, nflo)` and then twelve zeros; JXS is the five
//! locators above and then twenty-seven zeros (`acepa.f90:961-963`).
//!
//! ## The logs belong to the **Type-1** path only — and that is upstream
//!
//! `acepho` leaves ESZG in linear units; `phoout` takes the log on its way out
//! (`acepa.f90:966-973`), skipping exact zeros. Two things about that are easy
//! to get wrong and both are load-bearing:
//!
//! 1. **The log is inside `phoout`'s `itype == 1` branch and nowhere else.**
//!    The Type-2 branch (`:997-1010`) writes `xss` verbatim, so **a Type-2
//!    photo-atomic file from NJOY carries the ESZG block in linear units while
//!    the Type-1 file carries logs.** That is not a reading of the code — it
//!    is measured: NJOY's own Type-2 output for the synthetic Z=6 tape starts
//!    its data at `1.0000000000001e-3`, the energy in MeV, where the Type-1
//!    file has `-6.90775527898`. Whatever one thinks of that asymmetry, a port
//!    that "fixed" it would not reproduce upstream's files, so
//!    [`PhotoatomicAce::into_raw`] takes the target container and applies the
//!    log only for [`AceFileType::Type1Ascii`].
//! 2. **The log is skipped when `iverf == -1`**, i.e. on a stand-alone
//!    `iopt = 7/8` read-and-rewrite job, because the values read back are
//!    already logs. This port gets that for free: a table read from a file is
//!    a [`RawAceTable`] already and never goes through `into_raw`.
//!
//! The ordering matters for a third reason: the heating block is built from
//! the **linear** cross sections after ESZG is complete, so a builder that
//! logged early would feed logs into the heating sum.
//!
//! ## Grid
//!
//! The energy grid is MF=23/MT=501's own grid walked by `gety1` from 1 keV to
//! 1.01e11 eV, with each point rounded to 9 significant figures and a
//! discontinuity stored as two points straddling it (`acepa.f90:104-118`).
//! Everything else — the four partial cross sections — is evaluated **on that
//! grid**, so the table is one shared grid rather than five.

pub mod fluorescence;
pub mod incoherent_heating;

use crate::acer::read::{AceClass, AceFileType, AceHeader, RawAceTable};
use crate::endf::gety1::Gety1;
use crate::endf::interp::{intega, terpa};
use crate::endf::records::SectionCursor;
use crate::endf::tape::Tape;
use crate::mixr::mix::sigfig;
use crate::NjoyError;

use fluorescence::nflo_for;
use incoherent_heating::iheat;

/// `emin` (`acepa.f90:64`) — the grid walk starts here.
const EMIN: f64 = 1.0e3;
/// `emev` (`:65`).
const EMEV: f64 = 1.0e6;
/// `emax` (`:66`) — and stops here.
const EMAX: f64 = 1.01e11;
/// `epair` (`:67`) — `2 m_e c^2` \[MeV\], the pair-production threshold whose
/// excess is deposited.
const EPAIR_MEV: f64 = 1.022;

/// `vi` (`acepa.f90:48-52`) — the 21 momentum-transfer points the incoherent
/// scattering function is tabulated at.
const VI: [f64; 21] = [
    0.0, 0.005, 0.01, 0.05, 0.1, 0.15, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.5, 2.0, 3.0,
    4.0, 5.0, 8.0,
];

/// `vc` (`acepa.f90:53-62`) — the 55 momentum-transfer points the coherent
/// form factor and its cumulative integral are tabulated at.
const VC: [f64; 55] = [
    0.0, 0.01, 0.02, 0.03, 0.04, 0.05, 0.06, 0.08, 0.10, 0.12, 0.15, 0.18, 0.20, 0.25, 0.30, 0.35,
    0.40, 0.45, 0.50, 0.55, 0.60, 0.70, 0.80, 0.90, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8,
    1.9, 2.0, 2.2, 2.4, 2.6, 2.8, 3.0, 3.2, 3.4, 3.6, 3.8, 4.0, 4.2, 4.4, 4.6, 4.8, 5.0, 5.2, 5.4,
    5.6, 5.8, 6.0,
];

/// The MF=27/MT=504 incoherent scattering function `S(x, Z)` as a TAB1, with
/// `Z` in the record's `C2` — the input [`incoherent_heating::iheat`] wants.
///
/// # Errors
/// [`NjoyError::EndfParse`] when the section is absent or does not parse.
pub fn incoherent_scattering_function(
    tape: &Tape,
    mat: i32,
) -> Result<crate::endf::records::Tab1, NjoyError> {
    let sec = tape.section(mat, 27, 504).ok_or_else(|| {
        NjoyError::EndfParse(format!(
            "no MF=27/MT=504 (incoherent scattering function) for MAT {mat}"
        ))
    })?;
    let mut cur = SectionCursor::new(&sec.rows);
    cur.read_cont()?;
    cur.read_tab1()
}

/// Everything `acepho` needs that is not on the tape.
#[derive(Debug, Clone)]
pub struct PhotoatomicOptions {
    /// `suff` — the ZAID suffix, e.g. `0.00` for `92000.00p`.
    pub suffix: f64,
    /// `hk` — the 70-character comment.
    pub comment: String,
    /// `hd` — the processing date, written as upstream's `'  '//dater(hdt)`.
    /// Pass the 8-character `mm/dd/yy` body; it is padded here.
    pub date: String,
    /// `mcnpx` — widen the ZAID field to 13 characters and write
    /// `f10.3,'pp '` instead of `f9.2,'p'` (`acepa.f90:253-257`).
    pub mcnpx: bool,
    /// The 16 `IZ`/`AW` pairs. Photoatomic tables normally carry zeros.
    pub iz: [i32; 16],
    /// The 16 `AW` values.
    pub aw: [f64; 16],
}

impl Default for PhotoatomicOptions {
    fn default() -> Self {
        PhotoatomicOptions {
            suffix: 0.0,
            comment: String::new(),
            date: String::new(),
            mcnpx: false,
            iz: [0; 16],
            aw: [0.0; 16],
        }
    }
}

/// ENDF format version, from the tape's own MF=1/MT=451
/// (`acepa.f90:82-92`): `N1 != 0` on the second CONT means ENDF-4, `N2 == 0`
/// means ENDF-5, otherwise ENDF-6.
///
/// This decides only one thing here — whether photoelectric absorption is
/// MT=602 (ENDF-4/5) or MT=522 (ENDF-6) — but getting it from the tape rather
/// than assuming 6 is the difference between reading a legacy tape and
/// silently writing a table with no photoelectric column.
fn endf_version(tape: &Tape, mat: i32) -> i32 {
    let Some(sec) = tape.section(mat, 1, 451) else {
        return 6;
    };
    let mut cur = SectionCursor::new(&sec.rows);
    if cur.read_cont().is_err() {
        return 6;
    }
    match cur.read_cont() {
        Ok(c) if c.n1 != 0 => 4,
        Ok(c) if c.n2 == 0 => 5,
        _ => 6,
    }
}

/// Evaluate a MF=23 section on the ACE grid with `gety1`'s conventions.
fn mf23_on_grid(tape: &Tape, mat: i32, mt: i32, grid: &[f64]) -> Result<Vec<f64>, NjoyError> {
    let mut out = vec![0.0f64; grid.len()];
    let Some(sec) = tape.section(mat, 23, mt) else {
        return Ok(out);
    };
    let mut cur = SectionCursor::new(&sec.rows);
    cur.read_cont()?;
    let t = cur.read_tab1()?;
    let mut g = Gety1::new(&t);
    for (i, &e) in grid.iter().enumerate() {
        out[i] = g.get(e).y;
    }
    Ok(out)
}

/// The `acepho` result before the writer takes logs — every block in linear
/// units, which is the form the heating sum is built in.
#[derive(Debug, Clone)]
pub struct PhotoatomicAce {
    /// `iza` — the ZA from MF=23/MT=501.
    pub za: i32,
    /// `aw0` — the atomic weight ratio.
    pub awr: f64,
    /// `z` — atomic number, from MF=27/MT=502's `C1 / 1000`.
    pub z: i32,
    /// `nes` — grid length.
    pub nes: usize,
    /// The energy grid in **eV**, before the division by 1e6 that puts ESZG in
    /// MeV. Retained because it is the argument every MF=23 lookup and
    /// `iheat` call is made with, and a comparison against NJOY that only
    /// sees the MeV column cannot distinguish two grids that differ by an ulp.
    pub grid_ev: Vec<f64>,
    /// `nflo` — fluorescence rows.
    pub nflo: usize,
    /// NXS(1..16).
    pub nxs: [i32; 16],
    /// JXS(1..32).
    pub jxs: [i32; 32],
    /// XSS with ESZG still linear.
    pub xss: Vec<f64>,
    /// True when the caller supplied no relaxation tape, so JFLO is zeros —
    /// upstream's `mess('acepho','no atomic relaxation data', ...)`.
    pub fluorescence_missing: bool,
}

impl PhotoatomicAce {
    /// `eszg` — the block's 1-based locator (always 1).
    pub const ESZG: usize = 1;

    /// The five ESZG columns as `(energy_MeV, incoherent, coherent,
    /// photoelectric, pair)` slices, for a caller that wants to inspect the
    /// table without knowing the layout.
    pub fn eszg(&self) -> [&[f64]; 5] {
        let n = self.nes;
        [
            &self.xss[0..n],
            &self.xss[n..2 * n],
            &self.xss[2 * n..3 * n],
            &self.xss[3 * n..4 * n],
            &self.xss[4 * n..5 * n],
        ]
    }

    /// The heating block \[MeV per collision\].
    pub fn heating(&self) -> &[f64] {
        let lhnm = self.jxs[4] as usize; // JXS(5), 1-based
        &self.xss[lhnm - 1..lhnm - 1 + self.nes]
    }
}

/// Take natural logs of the ESZG block, the way `phoout` does on its way out
/// (`acepa.f90:966-973`): every one of the `5 * nes` words, **except exact
/// zeros**, which stay zero so a threshold reaction does not become `-inf`.
pub fn to_log_eszg(xss: &mut [f64], nes: usize) {
    for v in xss.iter_mut().take(5 * nes) {
        if *v != 0.0 {
            *v = v.ln();
        }
    }
}

/// Build a photo-atomic ACE table from an ENDF photo-atomic evaluation.
///
/// `relax` is the atomic-relaxation sublibrary tape. Passing `None` reproduces
/// upstream's `nlax = 0` path: a message is not printed here, but
/// [`PhotoatomicAce::fluorescence_missing`] is set and the JFLO block is left
/// as zeros — the same table NJOY writes in that case.
///
/// # Errors
/// [`NjoyError::EndfParse`] when MF=23/MT=501 or MF=27/MT=502 or MT=504 is
/// missing, or when a record does not parse.
pub fn photoatomic_ace(
    tape: &Tape,
    mat: i32,
    opts: &PhotoatomicOptions,
    relax: Option<&Tape>,
) -> Result<PhotoatomicAce, NjoyError> {
    let iverf = endf_version(tape, mat);

    // ── the energy grid, off MF=23/MT=501 (`acepa.f90:96-120`) ──────────────
    let sec = tape.section(mat, 23, 501).ok_or_else(|| {
        NjoyError::EndfParse(format!(
            "acepho: no MF=23/MT=501 (total photon interaction) for MAT {mat}"
        ))
    })?;
    let mut cur = SectionCursor::new(&sec.rows);
    let head = cur.read_cont()?;
    let za = head.c1.round() as i32;
    let awr = head.c2;
    let t501 = cur.read_tab1()?;
    let mut g = Gety1::new(&t501);
    let mut grid: Vec<f64> = Vec::new();
    // `gety1`'s x = 0 call leaves idis = 0, which is what the first iteration
    // of the walk tests.
    let mut idis = false;
    let mut enext = EMIN;
    while enext < EMAX {
        let mut e = sigfig(enext, 9, 0);
        if idis {
            // A discontinuity is stored as two points straddling it. The
            // `gety1` call below the jump is made for its side effect on the
            // getter's own cursor -- upstream assigns its `enext`/`idis` too
            // and then overwrites both from the call above the jump
            // (`acepa.f90:108-115`), so only the cursor advance survives.
            e = sigfig(e, 9, -1);
            let _ = g.get(e);
            grid.push(e);
            e = sigfig(e, 9, 2);
        }
        let v = g.get(e);
        enext = v.xnext;
        idis = v.idis;
        grid.push(e);
        if grid.len() > 10_000_000 {
            return Err(NjoyError::EndfParse(
                "acepho: MF=23/MT=501 grid walk did not terminate".into(),
            ));
        }
    }
    let nes = grid.len();

    // ── the four partial cross sections, on that grid (`:130-148`) ──────────
    let incoh = mf23_on_grid(tape, mat, 504, &grid)?;
    let coher = mf23_on_grid(tape, mat, 502, &grid)?;
    let pairp = mf23_on_grid(tape, mat, 516, &grid)?;
    let mt_abs = if iverf >= 6 { 522 } else { 602 };
    let absor = mf23_on_grid(tape, mat, mt_abs, &grid)?;

    // Energies to MeV (`:151-153`). Every later use of an energy goes through
    // this value, including `iheat`'s argument, which upstream reaches by
    // multiplying back by 1e6 -- reproduced so the round trip is bit-identical.
    let e_mev: Vec<f64> = grid.iter().map(|&e| e / EMEV).collect();

    let iz = mat / 100;
    let nflo = nflo_for(iz);

    // ── locators (`:123-128`, `:168-172`) ───────────────────────────────────
    let eszg = 1usize;
    let jinc = eszg + 5 * nes;
    let jcoh = jinc + 21;
    let jflo = jcoh + 110;
    let lhnm = jflo + 4 * nflo;
    let len2 = lhnm + nes - 1;

    let mut xss = vec![0.0f64; len2];
    xss[0..nes].copy_from_slice(&e_mev);
    xss[nes..2 * nes].copy_from_slice(&incoh);
    xss[2 * nes..3 * nes].copy_from_slice(&coher);
    xss[3 * nes..4 * nes].copy_from_slice(&absor);
    xss[4 * nes..5 * nes].copy_from_slice(&pairp);

    // ── coherent form factors (`:175-206`) ──────────────────────────────────
    let sec27 = tape.section(mat, 27, 502).ok_or_else(|| {
        NjoyError::EndfParse(format!(
            "acepho: no MF=27/MT=502 (coherent form factor) for MAT {mat}"
        ))
    })?;
    let mut c27 = SectionCursor::new(&sec27.rows);
    let h27 = c27.read_cont()?;
    let z = (h27.c1 / 1000.0).round() as i32;
    let ff = c27.read_tab1()?;
    for i in 0..55 {
        xss[jcoh + 54 + i] = terpa(&ff.interp, &ff.pairs, VC[i]).0;
    }
    // The sampling integral is over `(F/Z)^2` against `v^2`, so the table is
    // squared in place before integrating (`:194-199`) -- `z` is an integer in
    // upstream and `z**2` an integer division of the square.
    let z2 = (z as f64) * (z as f64);
    let squared: Vec<(f64, f64)> = ff
        .pairs
        .iter()
        .map(|&(x, y)| (x * x, y * y / z2))
        .collect();
    for i in 0..55 {
        xss[jcoh - 1 + i] = intega(&ff.interp, &squared, 0.0, VC[i] * VC[i])?;
    }

    // ── incoherent scattering function, and the heating it implies (`:208-228`)
    let sec504 = tape.section(mat, 27, 504).ok_or_else(|| {
        NjoyError::EndfParse(format!(
            "acepho: no MF=27/MT=504 (incoherent scattering function) for MAT {mat}"
        ))
    })?;
    let mut c504 = SectionCursor::new(&sec504.rows);
    c504.read_cont()?;
    let sf = c504.read_tab1()?;
    for i in 0..21 {
        xss[jinc - 1 + i] = terpa(&sf.interp, &sf.pairs, VI[i]).0;
    }
    let mut heat = vec![0.0f64; nes];
    for i in 0..nes {
        let h = iheat(e_mev[i] * EMEV, &sf);
        heat[i] = incoh[i] * h.heat / EMEV;
    }

    // ── photoelectric: the whole photon energy deposits, less fluorescence ──
    for i in 0..nes {
        heat[i] += e_mev[i] * absor[i];
    }
    let mut fluorescence_missing = false;
    let mut fluor = vec![0.0f64; 4 * nflo];
    match relax {
        None => fluorescence_missing = true,
        Some(r) if nflo > 0 => {
            fluor = fluorescence::alax(r, tape, mat, iz, &e_mev, &absor, &mut heat)?;
        }
        Some(_) => {}
    }
    if 4 * nflo > 0 {
        xss[jflo - 1..jflo - 1 + 4 * nflo].copy_from_slice(&fluor);
    }

    // ── pair production deposits everything above 2 m_e c^2 (`:236-238`) ────
    for i in 0..nes {
        heat[i] += pairp[i] * (e_mev[i] - EPAIR_MEV);
    }
    // ── per-collision basis (`:241-244`) ────────────────────────────────────
    for i in 0..nes {
        let tot = incoh[i] + coher[i] + absor[i] + pairp[i];
        heat[i] /= tot;
    }
    xss[lhnm - 1..lhnm - 1 + nes].copy_from_slice(&heat);

    let mut nxs = [0i32; 16];
    nxs[0] = len2 as i32;
    nxs[1] = z;
    nxs[2] = nes as i32;
    nxs[3] = nflo as i32;
    let mut jxs = [0i32; 32];
    jxs[0] = eszg as i32;
    jxs[1] = jinc as i32;
    jxs[2] = jcoh as i32;
    jxs[3] = jflo as i32;
    jxs[4] = lhnm as i32;

    let _ = opts; // the header is assembled by `into_raw`
    Ok(PhotoatomicAce {
        grid_ev: grid,
        za,
        awr,
        z,
        nes,
        nflo,
        nxs,
        jxs,
        xss,
        fluorescence_missing,
    })
}

impl PhotoatomicAce {
    /// Assemble the writable table for one container: header, NXS/JXS, and
    /// XSS — with ESZG in natural logs for [`AceFileType::Type1Ascii`] and in
    /// **linear units** for [`AceFileType::Type2Binary`], which is what
    /// `phoout` does (see this module's header).
    ///
    /// The ZAID is `f9.2` of `ZA + suff` followed by `p` (`acepa.f90:253-257`),
    /// or `f10.3` followed by `pp ` in the mcnpx variant, and the material
    /// field is `'   mat'` then `i4`.
    pub fn into_raw(mut self, opts: &PhotoatomicOptions, file_type: AceFileType) -> RawAceTable {
        if file_type == AceFileType::Type1Ascii {
            to_log_eszg(&mut self.xss, self.nes);
        }
        let zaid_num = self.za as f64 + opts.suffix;
        let zaid = if opts.mcnpx {
            format!("{zaid_num:10.3}pp ")
        } else {
            format!("{zaid_num:9.2}p")
        };
        let date = format!("  {:<8}", opts.date);
        let mat_id = format!("   mat{:4}", self.mat_number());
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
                class: AceClass::Photoatomic,
                awr: self.awr,
                kt_mev: 0.0,
                date: opts.date.clone(),
                comment: opts.comment.clone(),
                mat_id: mat_id.trim().to_string(),
                iz: opts.iz,
                aw: opts.aw,
            },
            nxs: self.nxs,
            jxs: self.jxs,
            // `phoout` writes every photo-atomic word as `1pe20.11`
            // (`acepa.f90:966-999`), so nothing is an integer.
            xss_is_int: Some(vec![false; self.xss.len()]),
            xss: self.xss,
        }
    }

    /// The MAT this table came from, recovered from `Z` the way `acepho`
    /// derives `iz` (`matd / 100`). Photo-atomic MATs are `100 * Z`.
    fn mat_number(&self) -> i32 {
        self.z * 100
    }
}
