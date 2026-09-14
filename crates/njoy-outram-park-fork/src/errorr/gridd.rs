// Ported from NJOY2016 `src/errorr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine errorr` dictionary scan, l.713-792 (`scan_reactions`).
//   - `subroutine gridd`, l.1091-1483 (`gridd`) — MF=33 / iread=0 / nstan=0 path.
//   - `subroutine merge`, l.1485-1557 (`merge`).
//   - `subroutine lumpmt`, l.1681-1768 (`lumpmt`).
//   - `subroutine uniong`, l.9534-9643 (`uniong`) — iverf>4 path.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Union energy grid and derived-cross-section coefficients for ERRORR's
//! MF=33 path — `gridd`, `merge`, `uniong`, `lumpmt` (`errorr.f90`).
//!
//! ERRORR does its covariance arithmetic on a **union grid**: the user group
//! boundaries `egn` unioned with every energy that appears in the MF=33
//! covariance records (`eni`). [`gridd`] collects `eni` and, from the
//! NC-type `LTY=0` "derivation formula" sub-subsections, the coefficient
//! table `akxy(iy, ix, k)` that expresses a *derived* reaction `ix` as a
//! linear combination of *evaluated* reactions `iy` inside derivation
//! energy range `k` (bounds `ek`). [`uniong`] then forms `un`, the union of
//! `egn` and `eni` restricted to the user range.
//!
//! **Scope of this port (honest):** `mfcov = 33`, `iread = 0` (reaction list
//! taken from the file), `nstan = 0` (no ratio-to-standard tape: an NC-type
//! sub-subsection with `LTY > 0` is an error here exactly as it is in
//! upstream with `nstan = 0`), ENDF-5/6 format (`iverf > 4`). MF=31/34/35/40
//! and the `iread = 1/2` user-list branches are not ported.
//!
//! All energies go through `sigfig(x, ndig = 6)` ([`NDIG`], `errorr.f90:116`)
//! exactly as upstream, so the union grid matches the Fortran listing to the
//! printed digits.

use crate::endf::records::SectionCursor;
use crate::endf::tape::Tape;
use crate::mixr::mix::sigfig;
use crate::NjoyError;

/// `ndig` — significant figures every ERRORR energy is rounded to
/// (`errorr.f90:116`, `integer,parameter::ndig=6`).
pub const NDIG: i32 = 6;

/// `small` / `big` — the default derivation range when the file has no
/// NC-type `LTY=0` formulas (`errorr.f90:1430-1431`).
const SMALL: f64 = 1.0e-10;
const BIG: f64 = 1.0e10;
/// `small5` — the lower bound prepended to `ek` when the first formula range
/// starts above `1e-5` eV (`errorr.f90:1412-1420`).
const SMALL5: f64 = 1.0e-5;
/// `eps` — `merge`'s relative coincidence tolerance (`errorr.f90:1495`).
const MERGE_EPS: f64 = 1.0e-5;

/// A lumped reaction (`MT` 851–870) and the component `MT`s that make it up
/// (`lump`/`lmt` in upstream, `errorr.f90:1681-1768`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LumpedReaction {
    /// The lumped `MT` (851..=870).
    pub mtl: i32,
    /// Component `MT`s whose MF=33 sections declare `MTL == mtl`, in file
    /// order.
    pub components: Vec<i32>,
}

/// The covariance reaction bookkeeping ERRORR builds from the ENDF
/// dictionary and MF=33 (`errorr.f90:713-792` + `gridd`).
#[derive(Debug, Clone, Default)]
pub struct CovarianceReactions {
    /// `mts(1:nmt1)` — covariance reactions in MF=33 file order (a lumped
    /// component that is also part of a lump is stored negated by
    /// [`lumpmt`], as upstream does).
    pub mts: Vec<i32>,
    /// `mats(1:nmt1)` — companion material of each entry (`0` = this
    /// material; `-1` marks a negated lumped component).
    pub mats: Vec<i32>,
    /// `mzap(1:nmt1)` — always `0` for MF=33.
    pub mzap: Vec<i32>,
    /// `iga(1:nga)` — the `MT`s `grpav` group-averages (every MF=33 `MT`
    /// that is neither a lump nor ignored).
    pub iga: Vec<i32>,
    /// `lump` — the lumped reactions found in the dictionary.
    pub lumps: Vec<LumpedReaction>,
    /// `mf32 == 1` — the dictionary lists an MF=32 file.
    pub mf32_present: bool,
    /// `mf33 == 1` — the dictionary lists an MF=33 file.
    pub mf33_present: bool,
}

/// Scan the material's dictionary for the covariance reactions
/// (`subroutine errorr`, `errorr.f90:713-792`, `mfcov = 33`).
///
/// Upstream reads the MF=1/MT=451 dictionary; a parsed [`Tape`] lists its
/// sections in the same file order, so the section keys are used directly.
/// Returns the reactions found; `mts`/`mats` stay empty here (they are filled
/// by [`gridd`] for ENDF-5/6 tapes, `errorr.f90:1154-1165`).
///
/// # Errors
/// [`NjoyError::EndfParse`] if the dictionary lists no MF=32/33 file
/// (`icov == 0`, upstream's "no data on file for mfcov" termination).
pub fn scan_reactions(tape: &Tape, matd: i32) -> Result<CovarianceReactions, NjoyError> {
    let mut r = CovarianceReactions::default();
    let mut icov = 0;
    for sec in tape.sections().iter().filter(|s| s.key.mat == matd) {
        let (mf, mt) = (sec.key.mf, sec.key.mt);
        if mf == 32 {
            r.mf32_present = true;
        }
        if mf == 33 {
            r.mf33_present = true;
        }
        if mf == 32 || mf == 33 {
            icov += 1;
        }
        if mf != 33 {
            continue;
        }
        // errorr.f90:750-754 — unknown MTs are ignored with a message.
        if mt == 850 || (870 < mt && mt < 875) || mt > 891 {
            continue;
        }
        if mt > 850 && mt <= 870 {
            r.lumps.push(LumpedReaction {
                mtl: mt,
                components: Vec::new(),
            });
        } else {
            r.iga.push(mt);
        }
    }
    if icov == 0 {
        return Err(NjoyError::EndfParse(format!(
            "errorr: no data on file for mfcov=33 (mat {matd})"
        )));
    }
    Ok(r)
}

/// Merge the ascending energy grid `x` into the ascending grid `y`
/// (`subroutine merge`, `errorr.f90:1485-1557`).
///
/// Every `x` is rounded to [`NDIG`] figures first; with a non-zero window
/// `(e1, e2)` only `e1 < x < e2` survive. An `x` equal to, or within
/// `1e-5` (relative) below, an existing `y` is dropped rather than
/// duplicated — the search resumes *after* the matched `y`, exactly as the
/// Fortran `j` pointer does.
///
/// # Errors
/// [`NjoyError::EndfParse`] if the merged grid is not ascending (upstream's
/// "y(j) lt y(i)" fatal check).
pub fn merge(x: &[f64], y: &mut Vec<f64>, e1: f64, e2: f64) -> Result<(), NjoyError> {
    if x.is_empty() {
        return Ok(());
    }
    // errorr.f90:1500-1513 — sigfig + window.
    let mut xs = Vec::with_capacity(x.len());
    for &xi in x {
        let xi = sigfig(xi, NDIG, 0);
        if !(e1 == 0.0 && e2 == 0.0) {
            if xi <= e1 {
                continue;
            }
            if xi >= e2 {
                break;
            }
        }
        xs.push(xi);
    }
    if y.is_empty() {
        *y = xs;
    } else {
        // errorr.f90:1522-1541 — the j pointer walks y once for all of x.
        let mut j = 0usize; // Fortran j (1-based count) before the first increment
        for xi in xs {
            loop {
                j += 1;
                if j > y.len() {
                    y.push(xi); // label 170: append, y(j) = x(i)
                    break;
                }
                let yj = y[j - 1];
                if yj < xi {
                    continue;
                }
                if yj == xi || (yj - xi).abs() <= MERGE_EPS * yj {
                    break; // label 180: coincident, x(i) dropped, j stays
                }
                y.insert(j - 1, xi); // insert x(i) at position j
                break;
            }
        }
    }
    // errorr.f90:1544-1555 — consistency check.
    for w in y.windows(2) {
        if w[0] > w[1] {
            return Err(NjoyError::EndfParse(format!(
                "errorr::merge: y={:e} lt {:e} (grid out of order)",
                w[1], w[0]
            )));
        }
    }
    Ok(())
}

/// Derived-cross-section coefficients `akxy(iy, ix, k)` over the derivation
/// energy ranges `ek(1:nek+1)` (`gridd`, `errorr.f90:1404-1480`).
///
/// `akxy(iy, ix, k)` is the coefficient of *evaluated* reaction `iy` in
/// *derived* reaction `ix` within range `k`; a reaction that is directly
/// evaluated in range `k` has `akxy(ix, ix, k) = 1` and zeros elsewhere in
/// its column. Indices are 0-based here (upstream's are 1-based).
#[derive(Debug, Clone, PartialEq)]
pub struct DerivedCoefficients {
    /// `ek(1:nek+1)` — range bounds \[eV\], ascending.
    pub ek: Vec<f64>,
    /// Number of reactions (`nmt1`).
    pub nmt1: usize,
    /// Flattened `[k][ix][iy]`.
    akxy: Vec<f64>,
}

impl DerivedCoefficients {
    /// Number of derivation ranges `nek`.
    pub fn nek(&self) -> usize {
        self.ek.len() - 1
    }

    /// `akxy(iy, ix, k)` (0-based).
    pub fn get(&self, iy: usize, ix: usize, k: usize) -> f64 {
        self.akxy[(k * self.nmt1 + ix) * self.nmt1 + iy]
    }

    fn set(&mut self, iy: usize, ix: usize, k: usize, v: f64) {
        self.akxy[(k * self.nmt1 + ix) * self.nmt1 + iy] = v;
    }

    /// Identity coefficients for `nmt1` reactions over `nek` ranges.
    fn identity(ek: Vec<f64>, nmt1: usize) -> Self {
        let nek = ek.len() - 1;
        let mut akxy = vec![0.0; nek * nmt1 * nmt1];
        for k in 0..nek {
            for i in 0..nmt1 {
                akxy[(k * nmt1 + i) * nmt1 + i] = 1.0;
            }
        }
        DerivedCoefficients { ek, nmt1, akxy }
    }

    /// Is reaction `ix` directly evaluated in every range (`akxy(ix,ix,k)
    /// != 0` for all `k`, the `isd` test of `covout`, `errorr.f90:7237-7243`)?
    pub fn is_directly_evaluated(&self, ix: usize) -> bool {
        (0..self.nek()).all(|k| self.get(ix, ix, k) != 0.0)
    }
}

/// One NC-type `LTY=0` derivation formula, staged until every `MT` is known
/// (`ak`/`el`/`eh`/`nmtr`/`imtr`, `errorr.f90:1300-1315`).
struct Formula {
    /// `imtr(ir)` — 0-based index of the derived reaction in `mts`.
    derived: usize,
    /// `el(ir)`, `eh(ir)` — range bounds (already `sigfig`'d).
    el: f64,
    eh: f64,
    /// `ak(1:2*nmtr, ir)` — `(coefficient, MT)` pairs.
    pairs: Vec<(f64, i32)>,
}

/// Output of [`gridd`].
#[derive(Debug, Clone)]
pub struct GriddResult {
    /// `eni(1:neni)` — the MF=33 energy grid (union of every NI/NC energy).
    pub eni: Vec<f64>,
    /// `ek`/`akxy` — derived-cross-section coefficients.
    pub derived: DerivedCoefficients,
}

/// Read through MF=33 and extract the covariance energy grid and the
/// derivation-coefficient table (`subroutine gridd`, `errorr.f90:1091-1483`,
/// `mfcov = 33`, `iread = 0`, `nstan = 0`).
///
/// Fills `reactions.mts`/`mats`/`mzap` in MF=33 file order (one entry per
/// section with `NL > 0`; lumped components with `NL = 0` are skipped,
/// `errorr.f90:1149`). Cross-material subsections (`MAT1 > 0`) are present
/// on file but "not wanted" under `iread = 0` (`iok = 0`), so their energies
/// do not enter the union grid (`errorr.f90:1200-1218`).
///
/// # Errors
/// - [`NjoyError::NotPorted`] for an NC-type sub-subsection with `LTY > 0`
///   (needs the `nstan` standards tape, `errorr.f90:1274-1297`).
/// - [`NjoyError::EndfParse`] for `MT1 = 0`, `LTY > 3`, a formula that
///   references an `MT` absent from MF=33, or a malformed record.
pub fn gridd(
    tape: &Tape,
    matd: i32,
    reactions: &mut CovarianceReactions,
) -> Result<GriddResult, NjoyError> {
    let mut eni: Vec<f64> = Vec::new();
    let mut ek: Vec<f64> = Vec::new();
    let mut formulas: Vec<Formula> = Vec::new();

    for sec in tape
        .sections()
        .iter()
        .filter(|s| s.key.mat == matd && s.key.mf == 33)
    {
        let mth = sec.key.mt;
        let mut cur = SectionCursor::new(&sec.rows);
        let head = cur.read_cont()?; // errorr.f90:1167
        let nsub = head.n2; // NL (errorr.f90:1181)
        if nsub == 0 {
            continue; // component of a lumped reaction (errorr.f90:1149)
        }
        // errorr.f90:1154-1165 — append this reaction.
        reactions.mts.push(mth);
        reactions.mats.push(0);
        reactions.mzap.push(0);
        let nmt = reactions.mts.len() - 1;

        for _ in 0..nsub {
            let sub = cur.read_cont()?; // errorr.f90:1187
            let mat1 = sub.l1;
            let mt1 = sub.l2;
            if mt1 == 0 {
                return Err(NjoyError::EndfParse(format!(
                    "errorr::gridd: illegal mt1=0 in MF=33/MT={mth}"
                )));
            }
            // iread=0: mat1>0 -> iok=0 (errorr.f90:1207-1208, 1224-1226).
            let iok = mat1 <= 0;
            let nc = sub.n1;
            let ni = sub.n2;

            // NC-type sub-subsections (errorr.f90:1235-1319).
            for _ in 0..nc {
                let lty_cont = cur.read_cont()?;
                let lty = lty_cont.l2;
                if lty > 3 {
                    return Err(NjoyError::EndfParse(format!(
                        "errorr::gridd: not coded for lty={lty} (MT={mth}, mat1={mat1}, mt1={mt1})"
                    )));
                }
                let list = cur.read_list()?;
                if !iok {
                    continue;
                }
                merge(&[list.head.c1, list.head.c2], &mut eni, 0.0, 0.0)?;
                let elh = sigfig(list.head.c1, NDIG, 0);
                let ehh = sigfig(list.head.c2, NDIG, 0);
                if lty != 0 {
                    // errorr.f90:1274-1297: needs nstan.
                    return Err(NjoyError::NotPorted(
                        "errorr::gridd: NC-type sub-subsection with lty>0 (ratio-to-standard, needs nstan)",
                    ));
                }
                // errorr.f90:1300-1318 — save the derivation formula.
                let n2h = list.head.n2 as usize; // number of MTs in the formula
                let n1h = list.head.n1 as usize; // 2*n2h words
                let mut pairs = Vec::with_capacity(n2h);
                for j in 0..n2h {
                    let c = list.data[2 * j];
                    let m = list.data[2 * j + 1].round() as i32;
                    pairs.push((c, m));
                }
                debug_assert_eq!(n1h, 2 * n2h);
                formulas.push(Formula {
                    derived: nmt,
                    el: elh,
                    eh: ehh,
                    pairs,
                });
                merge(&[list.head.c1, list.head.c2], &mut ek, 0.0, 0.0)?;
            }

            // NI-type sub-subsections (errorr.f90:1323-1350).
            for _ in 0..ni {
                let list = cur.read_list()?;
                if !iok {
                    continue;
                }
                let nx = list.head.n2 as usize; // NP / NE
                let lb = list.head.l2;
                if lb < 5 || lb == 8 {
                    // (E,F) pairs: E_k at data(2k-1); the last nl pairs are the
                    // second table (errorr.f90:1335-1341).
                    let nl = list.head.l1 as usize; // LT
                    let x: Vec<f64> = (0..nx).map(|i| list.data[2 * i]).collect();
                    let nx1 = nx - nl;
                    merge(&x[..nx1], &mut eni, 0.0, 0.0)?;
                    merge(&x[nx1..], &mut eni, 0.0, 0.0)?;
                } else {
                    merge(&list.data[..nx], &mut eni, 0.0, 0.0)?; // errorr.f90:1343
                    if lb != 5 {
                        // lb=6: column energies follow the nx row energies.
                        let nec = (list.head.n1 as usize - 1) / nx;
                        merge(&list.data[nx..nx + nec], &mut eni, 0.0, 0.0)?;
                    }
                }
            }
        }
    }

    // errorr.f90:1406-1420 — a default range 1e-5 .. ek(1) if needed.
    let mut neki = ek.len();
    if neki > 0 && SMALL5 + SMALL5 / 1000.0 < ek[0] {
        ek.insert(0, SMALL5);
        neki += 1;
    }
    let nmt1 = reactions.mts.len();
    if neki == 0 {
        let derived = DerivedCoefficients::identity(vec![SMALL, BIG], nmt1);
        return Ok(GriddResult { eni, derived });
    }
    let mut derived = DerivedCoefficients::identity(ek.clone(), nmt1);

    // errorr.f90:1437-1477 — reconstruct the full akxy table.
    for (irs, f) in formulas.iter().enumerate() {
        let ilo = ek.iter().position(|&e| e == f.el).ok_or_else(|| {
            NjoyError::EndfParse(format!(
                "errorr::gridd: formula {} lower bound {:e} not on ek grid",
                irs + 1,
                f.el
            ))
        })?;
        let ihi = ek.iter().position(|&e| e == f.eh).ok_or_else(|| {
            NjoyError::EndfParse(format!(
                "errorr::gridd: formula {} upper bound {:e} not on ek grid",
                irs + 1,
                f.eh
            ))
        })?;
        // Fortran: ihi is the ek index of eh, then ihi=ihi-1 -> last range.
        let ihi = ihi - 1;
        let mut jak = Vec::with_capacity(f.pairs.len());
        for &(_, ixm) in &f.pairs {
            let idx = reactions.mts.iter().position(|&m| m == ixm).ok_or_else(|| {
                NjoyError::EndfParse(format!(
                    "errorr::gridd: mt{ixm} referenced in derivation formula for range {} does not appear in mfcov",
                    irs + 1
                ))
            })?;
            jak.push(idx);
        }
        for k in ilo..=ihi {
            for (j, &(coef, _)) in f.pairs.iter().enumerate() {
                derived.set(jak[j], f.derived, k, coef);
            }
            derived.set(f.derived, f.derived, k, 0.0);
        }
    }
    Ok(GriddResult { eni, derived })
}

/// Form the union of the user's group boundaries with the MF=33 energy grid
/// (`subroutine uniong`, `errorr.f90:9534-9643`, `iverf > 4` path).
///
/// `egn` must already be `sigfig`'d to [`NDIG`] (as `egngpn` does before
/// calling `uniong`, `errorr.f90:9787-9790`); `eni` is the grid from
/// [`gridd`]. Covariance energies outside `[egn(1), egn(ngn+1)]` are
/// dropped. Returns `un(1:nunion+1)`.
///
/// # Errors
/// [`NjoyError::EndfParse`] if the union is not strictly ascending.
pub fn uniong(egn: &[f64], eni: &[f64]) -> Result<Vec<f64>, NjoyError> {
    let ngnp1 = egn.len();
    let neni = eni.len();
    let mut scr2: Vec<f64> = Vec::with_capacity(ngnp1 + neni);
    let mut j = 1usize; // 1-based pointer into eni
    let mut i = 1usize; // 1-based pointer into egn
    let mut finished_endf = false;
    if neni == 0 {
        // Degenerate: nothing to union (upstream would read garbage here).
        scr2.extend_from_slice(egn);
    } else {
        'outer: while i <= ngnp1 {
            loop {
                // label 140
                if eni[j - 1] < egn[0] {
                    // label 160
                    if j == neni {
                        i += 1;
                        continue 'outer;
                    }
                    j += 1;
                    continue;
                }
                if eni[j - 1] <= egn[i - 1] {
                    // label 150
                    scr2.push(eni[j - 1]);
                    j += 1;
                    if j <= neni && egn[i - 1] != eni[j - 2] {
                        continue;
                    }
                    if j <= neni && egn[i - 1] == eni[j - 2] {
                        i += 1;
                        continue 'outer;
                    }
                    // all endf energies used (label 170)
                    finished_endf = true;
                    break 'outer;
                }
                scr2.push(egn[i - 1]);
                i += 1;
                continue 'outer;
            }
        }
        if finished_endf {
            // label 170: remaining user energies.
            for &e in &egn[i - 1..] {
                if scr2.last() != Some(&e) {
                    scr2.push(e);
                }
            }
        }
    }
    // label 200: strict ordering check.
    for w in scr2.windows(2) {
        if w[1] <= w[0] {
            return Err(NjoyError::EndfParse(format!(
                "errorr::uniong: union energies out of order ({:e} le {:e})",
                w[1], w[0]
            )));
        }
    }
    Ok(scr2)
}

/// Fill in the component list of every lumped reaction and mark lumped
/// components in `mts` (`subroutine lumpmt`, `errorr.f90:1681-1768`).
///
/// A section whose HEAD `MTL` is in 851..=870 is a component of that lump;
/// its own entry in `mts` (if it has one) is negated and its `mats` set to
/// `-1`, exactly as upstream flags it.
pub fn lumpmt(
    tape: &Tape,
    matd: i32,
    reactions: &mut CovarianceReactions,
) -> Result<(), NjoyError> {
    if reactions.lumps.is_empty() {
        return Ok(());
    }
    for sec in tape
        .sections()
        .iter()
        .filter(|s| s.key.mat == matd && s.key.mf == 33)
    {
        let mth = sec.key.mt;
        let mut cur = SectionCursor::new(&sec.rows);
        let head = cur.read_cont()?;
        let mt1 = head.l2; // MTL
        if !(851..=870).contains(&mt1) {
            continue;
        }
        if let Some(l) = reactions.lumps.iter_mut().find(|l| l.mtl == mt1) {
            l.components.push(mth);
            if let Some(j) = reactions.mts.iter().position(|&m| m == mth) {
                reactions.mts[j] = -reactions.mts[j];
                if reactions.mats[j] == 0 {
                    reactions.mats[j] = -1;
                }
                if reactions.mats[j] > 0 {
                    reactions.mats[j] = -reactions.mats[j];
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `merge` must insert, skip coincident (exact and within 1e-5 below),
    /// window, and keep ascending order — hand-traced against the Fortran
    /// `j` pointer (`errorr.f90:1522-1541`).
    #[test]
    fn merge_inserts_skips_and_windows_like_upstream() {
        // Every grid upstream merges into has already been through sigfig
        // (with its 1.0000000000001 bias), so build `y` the same way.
        let sf = |v: &[f64]| v.iter().map(|&x| sigfig(x, NDIG, 0)).collect::<Vec<_>>();
        let mut y = sf(&[1.0, 2.0, 3.0]);
        merge(&[0.5, 2.0, 2.5, 2.999_99, 4.0], &mut y, 0.0, 0.0).unwrap();
        // 2.0 exact -> dropped; 2.99999 is within 1e-5*3.0 below 3.0 -> dropped.
        assert_eq!(y, sf(&[0.5, 1.0, 2.0, 2.5, 3.0, 4.0]));

        let mut y = Vec::new();
        merge(&[1.0, 2.0, 3.0, 4.0], &mut y, 1.5, 3.5).unwrap();
        assert_eq!(y, sf(&[2.0, 3.0]));

        // sigfig to 6 digits on the way in.
        let mut y = Vec::new();
        merge(&[1.234_567_89e6], &mut y, 0.0, 0.0).unwrap();
        assert!((y[0] - 1.234_57e6).abs() < 1e-3 * 1e6 * 1e-6);
    }

    /// `uniong` with a covariance grid that straddles the user range: energies
    /// below `egn(1)` and above `egn(ngn+1)` are dropped, interior ones are
    /// inserted, coincident ones are not duplicated.
    #[test]
    fn uniong_unions_and_clips_to_user_range() {
        let egn = [1.0, 10.0, 100.0, 1000.0];
        let eni = [0.1, 1.0, 5.0, 10.0, 50.0, 500.0, 5000.0];
        let un = uniong(&egn, &eni).unwrap();
        assert_eq!(un, vec![1.0, 5.0, 10.0, 50.0, 100.0, 500.0, 1000.0]);

        // no interior points at all
        let un = uniong(&egn, &[0.5, 2000.0]).unwrap();
        assert_eq!(un, egn.to_vec());
    }

    /// `DerivedCoefficients` identity + `is_directly_evaluated`.
    #[test]
    fn derived_coefficients_identity() {
        let d = DerivedCoefficients::identity(vec![1e-5, 1e7], 3);
        assert_eq!(d.nek(), 1);
        for i in 0..3 {
            for j in 0..3 {
                assert_eq!(d.get(j, i, 0), if i == j { 1.0 } else { 0.0 });
            }
            assert!(d.is_directly_evaluated(i));
        }
    }
}
