// Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Record-level plumbing for [`super::Mf6Feed`]: the flat `ddmf6` LIST view
//! that keeps `getmf6`'s in-place grid patches literally translatable, the
//! per-subsection container, the `skip6` tape walk, and the two `ismooth`
//! sqrt(E) extensions.
//!
//! Split out of `mf6_feed.rs` only to respect this crate's 1000-line file cap
//! (see its `CLAUDE.md`); every item here is `pub(super)` and belongs to
//! `getmf6` (`groupr.f90:7527-8133`).

use crate::endf::records::{SectionCursor, Tab1};
use crate::groupr::kinematics::{Cm6Point, Law1LabTable};
use crate::mixr::mix::sigfig;
use crate::NjoyError;

/// One incident-energy emission table, held exactly as the Fortran `tmp`/
/// `ddmf6` LIST record so the in-place grid patches (`:7705-7810`) translate
/// literally: `buf[k]` is `tmp(ilo + k)`.
///
/// Layout: `[C1, E, ND, NA, NW, NEP, (E'_0, c_0..), (E'_1, c_1..), ...]`,
/// stride `ncyc = NW / NEP`. The first `ND` records are discrete emission
/// lines; the rest are the continuum.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct LabTable {
    pub(super) buf: Vec<f64>,
}

impl LabTable {
    /// `tmp(ilo+1)` — the incident energy \[eV\].
    pub(super) fn e_in(&self) -> f64 {
        self.buf[1]
    }
    /// `nint(tmp(ilo+2))` — ENDF `ND`, the discrete-line count.
    pub(super) fn nd(&self) -> usize {
        self.buf[2].round().max(0.0) as usize
    }
    /// `nint(tmp(ilo+4))` — ENDF `NW`.
    pub(super) fn nw(&self) -> usize {
        self.buf[4].round().max(0.0) as usize
    }
    /// `nint(tmp(ilo+5))` — ENDF `NEP`, the record count.
    pub(super) fn nep(&self) -> usize {
        self.buf[5].round().max(0.0) as usize
    }
    /// `ncyc = nint(tmp(ilo+4)/tmp(ilo+5))` (`:7689`, `:8056`).
    pub(super) fn ncyc(&self) -> usize {
        let nep = self.nep();
        if nep == 0 {
            0
        } else {
            (self.nw() as f64 / nep as f64).round().max(0.0) as usize
        }
    }
    /// How many `(E', coeffs)` records the buffer actually holds. Normally
    /// `nep()`; smaller when the `ismooth` merge loop shrank the record
    /// without updating the header (see [`Mf6Feed::new`]'s bug note).
    pub(super) fn stored_records(&self) -> usize {
        let ncyc = self.ncyc();
        if ncyc == 0 {
            0
        } else {
            (self.buf.len().saturating_sub(6)) / ncyc
        }
    }
    /// Record `j` as `(E', coeffs)`; `coeffs` has `ncyc - 1` entries.
    pub(super) fn record(&self, j: usize) -> Cm6Point {
        let ncyc = self.ncyc();
        let base = 6 + j * ncyc;
        Cm6Point {
            ep: self.buf[base],
            coeffs: self.buf[base + 1..base + ncyc].to_vec(),
        }
    }
    /// The continuum part (records `ND ..` ), as `f6lab` wants it.
    pub(super) fn continuum(&self) -> Law1LabTable {
        let n = self.stored_records();
        Law1LabTable {
            e_in: self.e_in(),
            points: (self.nd().min(n)..n).map(|j| self.record(j)).collect(),
        }
    }
    /// The `ND` discrete emission lines (records `0 .. ND`).
    pub(super) fn discrete(&self) -> Vec<Cm6Point> {
        let n = self.stored_records();
        (0..self.nd().min(n)).map(|j| self.record(j)).collect()
    }
}

/// One MF=6 subsection for the requested particle — the Fortran
/// `iyss(iss)` (yield TAB1) / `izss(iss)` (law block) / `jloss(..)` (per
/// incident-energy tables) triple.
#[derive(Debug, Clone)]
pub(super) struct Mf6Sub {
    /// `ddmf6(iy..)`: the yield TAB1 packed in ENDF order, ready for `terpa`
    /// (`:7982`).
    pub(super) yield_packed: Vec<f64>,
    /// `nint(ddmf6(iy+3))` — ENDF `LAW`.
    pub(super) law: i32,
    /// `nint(ddmf6(iz+2))` — ENDF `LANG`.
    pub(super) lang: i32,
    /// `nint(ddmf6(iz+3))` — ENDF `LEP`.
    pub(super) lep: i32,
    /// `nint(ddmf6(iz+7))` — the first region's incident-energy interpolation
    /// code, after the `2 -> 22` forcing (`:7672-7674`, `:8010-8012`).
    pub(super) int_code: u32,
    /// `jloss(jjss(iss) .. )` — one lab table per incident energy (`ne`).
    pub(super) tables: Vec<LabTable>,
}

/// Pack a TAB1 back into the flat ENDF field order `terpa` expects:
/// `C1, C2, L1, L2, NR, NP, (NBT, INT) * NR, (x, y) * NP`.
pub(super) fn pack_tab1(t: &Tab1) -> Vec<f64> {
    let mut a = Vec::with_capacity(6 + 2 * t.interp.len() + 2 * t.pairs.len());
    a.extend_from_slice(&[
        t.head.c1,
        t.head.c2,
        t.head.l1 as f64,
        t.head.l2 as f64,
        t.head.n1 as f64,
        t.head.n2 as f64,
    ]);
    for &(nbt, int) in &t.interp {
        a.push(nbt as f64);
        a.push(int as f64);
    }
    for &(x, y) in &t.pairs {
        a.push(x);
        a.push(y);
    }
    a
}

/// Pack a lab-frame distribution back into the LIST layout `cm2lab`/`ll2lab`
/// write (`:8245-8252`, `:9057-9059`): `ND = 0`, `NA = nl - 1`,
/// `NW = np*(nl+1)`, `NEP = np`, then `np` records of `(E', y_1..y_nl)`.
pub(super) fn pack_lab_list(e_in: f64, points: &[(f64, Vec<f64>)], nl: usize) -> Vec<f64> {
    let np = points.len();
    let nw = np * (nl + 1);
    let mut buf = Vec::with_capacity(6 + nw);
    buf.extend_from_slice(&[0.0, e_in, 0.0, (nl as f64) - 1.0, nw as f64, np as f64]);
    for (ep, coeffs) in points {
        buf.push(*ep);
        for l in 0..nl {
            buf.push(coeffs.get(l).copied().unwrap_or(0.0));
        }
    }
    buf
}

/// `skip6` (`endf.f90:1437-1476`) — step the cursor past a subsection body.
pub(super) fn skip6(cur: &mut SectionCursor<'_>, law: i32) -> Result<(), NjoyError> {
    match law {
        6 => {
            cur.read_cont()?;
        }
        1 | 2 | 5 => {
            let t = cur.read_tab2()?;
            for _ in 0..t.head.n2.max(0) {
                cur.read_list()?;
            }
        }
        7 => {
            let t = cur.read_tab2()?;
            for _ in 0..t.head.n2.max(0) {
                let m = cur.read_tab2()?;
                for _ in 0..m.head.n2.max(0) {
                    cur.read_tab1()?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// `:7705-7721` — patch a two-point lin-lin secondary distribution whose first
/// bin sits below the elastic-recoil floor `E/(A+1)^2`.
///
/// # Upstream defect reproduced
/// Every index here (`ilo+7`, `ilo+8`, `ilo+9`) is written as if the record
/// stride were exactly 2, i.e. `NA = 0`. For an anisotropic record
/// (`NA > 0`, `ncyc > 2`) `tmp(ilo+8)`/`tmp(ilo+9)` are the first record's
/// *angular coefficients*, not the second record's `(E', f0)`, so the patch
/// corrupts the distribution instead of fixing it. Only `tmp(ilo+6+ncyc)` on
/// the guard line uses the real stride. Reproduced verbatim; flagged, not
/// fixed.
pub(super) fn law1_low_energy_patch(
    buf: &mut [f64],
    nn: usize,
    lep: i32,
    ncyc: usize,
    awr: f64,
    messages: &mut Vec<String>,
) {
    if nn != 2 || lep != 2 {
        return;
    }
    if buf.len() < 10 || 6 + ncyc >= buf.len() {
        return;
    }
    let floor = buf[1] / (awr + 1.0).powi(2);
    let mut j = 0;
    if buf[6 + ncyc] < floor {
        j = 1;
        buf[7] = 0.0;
        buf[8] = floor;
        buf[9] = 2.0 / buf[8];
    } else if buf[7] != 0.0 && buf[9] == 0.0 {
        j = 1;
        buf[9] = buf[7];
        buf[7] = 0.0;
    }
    if j != 0 {
        messages.push(format!(
            "getmf6: patching low-energy distribution at {:10.3e}",
            buf[1]
        ));
    }
}

/// `:7722-7763` — `ismooth` sqrt(E) extension of a **histogram** (`LEP = 1`)
/// neutron emission spectrum.
///
/// First merges leading bins while the running integral still follows
/// `E^{3/2}` (`:7729-7744`), then prepends geometric bins with the sqrt(E)
/// shape down below 40 eV (`:7748-7762`).
///
/// # Upstream defect reproduced
/// The merge loop decrements the local `nx` (`NW`) and `n` (`NEP`) but never
/// writes them back into the record header — only the *extension* loop does
/// (`:7758-7759`). If the merge loop runs and the extension loop does not (the
/// second energy is already `<= 40 eV`), the record is truncated to the merged
/// length by `l = ilo+6+nx` (`:7763`) while `NW`/`NEP` still describe the
/// pre-merge record, so `NEP` over-counts the records actually present. This
/// port reproduces that and simply never reads past the buffer
/// ([`LabTable::stored_records`]).
pub(super) fn ismooth_histogram(buf: &mut Vec<f64>, messages: &mut Vec<String>) {
    const FX: f64 = 0.8409;
    const EX: f64 = 40.0;
    let ncyc = (buf[3].round().max(0.0) as usize) + 2;
    if ncyc < 2 || buf.len() < 6 + 3 * ncyc {
        return;
    }
    let mut cx = buf[6 + ncyc] * buf[7];
    let mut nx = buf[4].round().max(0.0) as usize;
    let mut n = buf[5].round().max(0.0) as usize;
    // `:7729-7744` — merge while the sqrt(E) form still holds.
    while n > 2 && 6 + 2 * ncyc < buf.len() {
        let e1 = buf[6 + ncyc];
        let e2 = buf[6 + 2 * ncyc];
        if e1 <= 0.0 || e2 <= 0.0 {
            break;
        }
        let cxx = cx + buf[7 + ncyc] * (e2 - e1);
        let ref_ = cx / e1.powf(1.5);
        if (cxx / e2.powf(1.5) - ref_).abs() > ref_ / 50.0 {
            break;
        }
        buf[7] = (buf[7] * e1 + buf[7 + ncyc] * (e2 - e1)) / e2;
        if nx < 2 * ncyc {
            break;
        }
        for ix in 1..=(nx - 2 * ncyc) {
            buf[5 + ix + ncyc] = buf[5 + ix + 2 * ncyc];
        }
        cx = cxx;
        nx -= ncyc;
        n -= 1;
    }
    messages.push(format!(
        "getmf6: extending histogram as sqrt(E) below {:10.2e} eV for E={:10.2e} eV",
        buf[6 + ncyc],
        buf[1]
    ));
    // `:7746-7762` — prepend geometric sqrt(E) bins.
    let mut guard = 0usize;
    while buf[6 + ncyc] > EX {
        if buf.len() < 6 + ncyc + nx + 1 {
            buf.resize(6 + ncyc + nx + 1, 0.0);
        }
        for ix in (1..=nx).rev() {
            buf[5 + ncyc + ix] = sigfig(buf[5 + ix], 7, 0);
        }
        buf[6 + ncyc] = sigfig(FX * buf[6 + 2 * ncyc], 7, 0);
        let val = buf[7];
        buf[7] = sigfig(FX.sqrt() * val, 7, 0);
        buf[7 + ncyc] = sigfig((1.0 - FX * FX.sqrt()) * val / (1.0 - FX), 7, 0);
        nx += ncyc;
        n += 1;
        buf[4] = nx as f64;
        buf[5] = n as f64;
        guard += 1;
        if guard > 10_000 {
            break;
        }
    }
    buf.truncate(6 + nx);
}

/// `:7765-7810` — `ismooth` sqrt(E) extension of a **lin-lin** (`LEP = 2`)
/// neutron emission spectrum, with the `rn` renormalisation that conserves
/// the original leading-panel integral (`:7797-7809`).
pub(super) fn ismooth_linlin(buf: &mut Vec<f64>, messages: &mut Vec<String>) {
    const EX: f64 = 40.0;
    const FX: f64 = 0.50;
    let ncyc = (buf[3].round().max(0.0) as usize) + 2;
    if ncyc < 2 || buf.len() < 6 + 3 * ncyc {
        return;
    }
    let mut nx = buf[4].round().max(0.0) as usize;
    let mut n = buf[5].round().max(0.0) as usize;
    messages.push(format!(
        "getmf6: extending lin-lin as sqrt(E) below {:10.2e} eV for E={:10.2e} eV",
        buf[6 + ncyc],
        buf[1]
    ));
    let mut nn = 0usize;
    let cx = (buf[6 + ncyc] - buf[6]) * (buf[7 + ncyc] + buf[7]) / 2.0;
    let mut cxx = 0.0_f64;
    for i in 1..ncyc {
        buf[6 + i] = 0.0;
    }
    let mut guard = 0usize;
    while buf[6 + ncyc] > EX {
        nn += 1;
        if buf.len() < 6 + ncyc + nx + 1 {
            buf.resize(6 + ncyc + nx + 1, 0.0);
        }
        for ix in (ncyc..=nx).rev() {
            buf[5 + ncyc + ix] = buf[5 + ix];
        }
        buf[6 + ncyc] = sigfig(FX * buf[6 + 2 * ncyc], 6, 0);
        for i in 1..ncyc {
            buf[6 + ncyc + i] = buf[6 + 2 * ncyc + i] * (buf[6 + ncyc] / buf[6 + 2 * ncyc]).sqrt();
        }
        if nn > 1 {
            cxx += (buf[6 + 2 * ncyc] - buf[6 + ncyc]) * (buf[7 + 2 * ncyc] + buf[7 + ncyc]) / 2.0;
        }
        nx += ncyc;
        n += 1;
        buf[4] = nx as f64;
        buf[5] = n as f64;
        guard += 1;
        if guard > 10_000 {
            break;
        }
    }
    cxx += buf[6 + ncyc] * buf[7 + ncyc] / 2.0;
    if 6 + (nn + 1) * ncyc >= buf.len() {
        buf.truncate(6 + nx);
        return;
    }
    let dx = buf[6 + (nn + 1) * ncyc] - buf[6 + nn * ncyc];
    let mut rn = 1.0_f64;
    if cxx + dx * buf[7 + nn * ncyc] / 2.0 != 0.0 {
        rn = (cx - dx * buf[7 + (nn + 1) * ncyc] / 2.0) / (cxx + dx * buf[7 + nn * ncyc] / 2.0);
    }
    for i in 1..=nn {
        for j in 1..ncyc {
            buf[6 + i * ncyc + j] *= rn;
        }
    }
    buf.truncate(6 + nx);
}
